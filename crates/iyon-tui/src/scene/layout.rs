use std::collections::HashMap;

use crate::presentation::{ContentProvider, EmptyContentProvider};
use crate::{
    component::{ComponentId, ComponentRegistry, MountGraph},
    geometry::{LayoutConstraints, Size},
    interaction::MountedCapabilities,
    presentation::layout::{
        ComponentGeometryMap, LayoutCache, LayoutTree,
        layout_view_with_overlay_and_cache_and_content,
        layout_view_with_overlay_and_cache_in_scope_and_content,
    },
};

use super::ResolvedScene;

/// Ephemeral geometry derived from a resolved semantic scene.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedSceneLayout {
    pub(crate) tree: LayoutTree,
    pub(crate) components: ComponentGeometryMap,
}

#[cfg(any(test, feature = "perf-counters"))]
pub(crate) fn layout_resolved_scene(scene: &ResolvedScene, size: Size) -> ResolvedSceneLayout {
    let mut cache = LayoutCache::default();
    layout_resolved_scene_with_cache(scene, size, &mut cache)
}

pub(crate) fn layout_resolved_scene_with_cache(
    scene: &ResolvedScene,
    size: Size,
    cache: &mut LayoutCache,
) -> ResolvedSceneLayout {
    let mut content = EmptyContentProvider;
    layout_resolved_scene_with_cache_and_content(scene, size, cache, &mut content)
}

pub(crate) fn layout_resolved_scene_with_cache_and_content(
    scene: &ResolvedScene,
    size: Size,
    cache: &mut LayoutCache,
    content: &mut dyn ContentProvider,
) -> ResolvedSceneLayout {
    let tree = layout_view_with_overlay_and_cache_and_content(
        &scene.view,
        LayoutConstraints::bounded(size),
        &scene.overlay,
        None,
        cache,
        content,
    );
    let components = tree.component_geometry();
    ResolvedSceneLayout { tree, components }
}

impl ResolvedSceneLayout {
    /// Re-lays out one component content root and patches it in place when its
    /// measured shape/geometry is unchanged. A geometry change returns false
    /// so the caller can perform the authoritative full layout pass.
    pub(crate) fn patch_component_with_cache(
        &mut self,
        component: ComponentId,
        view: &crate::presentation::View,
        overlay: &super::ResolutionOverlay,
        cache: &mut LayoutCache,
        content: &mut dyn ContentProvider,
    ) -> bool {
        let Some(component_root) = self.components.roots.get(&component).copied() else {
            return false;
        };
        let Some(child) = self.tree.node(component_root).children.first().copied() else {
            return false;
        };
        let old_size = self.tree.node(child).rect.size();
        let replacement = layout_view_with_overlay_and_cache_in_scope_and_content(
            view,
            LayoutConstraints::width_only(old_size.width),
            overlay,
            Some(component),
            cache,
            content,
        );
        let shape_changed = replacement.size != old_size;
        let tree_patched =
            !shape_changed && self.tree.patch_component_subtree(component, &replacement);
        if !tree_patched {
            return false;
        }
        if !self
            .tree
            .patch_component_geometry(component, &mut self.components)
        {
            return false;
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayoutSync {
    Stable,
    Dirty,
}

#[derive(Debug, Default)]
pub(crate) struct LayoutSynchronizer {
    delivered: HashMap<ComponentId, Size>,
    delivered_content_extents: HashMap<ComponentId, Size>,
}

impl LayoutSynchronizer {
    pub(crate) fn synchronize(
        &mut self,
        graph: &MountGraph,
        capabilities: &MountedCapabilities,
        geometry: &ComponentGeometryMap,
        registry: &mut ComponentRegistry,
    ) -> LayoutSync {
        self.delivered.retain(|id, _| graph.contains(*id));
        self.delivered_content_extents
            .retain(|id, _| graph.contains(*id));
        let mut dirty = false;
        for node in graph.iter() {
            dirty |= self.synchronize_component(node.id, capabilities, geometry, registry)
                == LayoutSync::Dirty;
        }
        if dirty {
            LayoutSync::Dirty
        } else {
            LayoutSync::Stable
        }
    }

    /// Synchronizes one retained component's layout and content-extent
    /// callbacks. The two notifications have independent delivery revisions
    /// because a viewport can keep the same allocation while its full content
    /// extent changes.
    pub(crate) fn synchronize_component(
        &mut self,
        id: ComponentId,
        capabilities: &MountedCapabilities,
        geometry: &ComponentGeometryMap,
        registry: &mut ComponentRegistry,
    ) -> LayoutSync {
        let Some(entry) = geometry.entries.get(&id) else {
            unreachable!("mounted component has no layout geometry");
        };
        let size = entry.content.size();
        let layout_handler = capabilities
            .get(id)
            .and_then(|caps| caps.layout_changed.as_ref())
            .cloned();
        let extent_handler = capabilities
            .get(id)
            .and_then(|caps| caps.content_extent_changed.as_ref())
            .cloned();
        let mut dirty = false;

        if let Some(handler) = layout_handler {
            if self.delivered.get(&id).copied() != Some(size) {
                self.delivered.insert(id, size);
                registry.with_any_mut(id, |component| handler(component, size));
                dirty = true;
            }
        } else {
            self.delivered.remove(&id);
        }

        let extent = geometry.content_extents.get(&id).copied();
        if let Some(handler) = extent_handler {
            if let Some(extent) = extent {
                if self.delivered_content_extents.get(&id).copied() != Some(extent) {
                    self.delivered_content_extents.insert(id, extent);
                    registry.with_any_mut(id, |component| handler(component, extent));
                    dirty = true;
                }
            } else {
                self.delivered_content_extents.remove(&id);
            }
        } else {
            self.delivered_content_extents.remove(&id);
        }

        if dirty {
            LayoutSync::Dirty
        } else {
            LayoutSync::Stable
        }
    }
}
