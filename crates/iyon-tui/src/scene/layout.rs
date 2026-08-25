use std::collections::HashMap;

use crate::{
    component::{ComponentId, ComponentRegistry, MountGraph},
    geometry::{LayoutConstraints, Size},
    interaction::MountedCapabilities,
    presentation::layout::{
        ComponentGeometryMap, LayoutCache, LayoutTree, layout_view_with_overlay_and_cache,
        layout_view_with_overlay_and_cache_in_scope,
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
    let tree = layout_view_with_overlay_and_cache(
        &scene.view,
        LayoutConstraints::bounded(size),
        &scene.overlay,
        cache,
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
    ) -> bool {
        let Some(component_root) = self.components.roots.get(&component).copied() else {
            return false;
        };
        let Some(child) = self.tree.node(component_root).children.first().copied() else {
            return false;
        };
        let old_size = self.tree.node(child).rect.size();
        let replacement = layout_view_with_overlay_and_cache_in_scope(
            view,
            LayoutConstraints::width_only(old_size.width),
            overlay,
            Some(component),
            cache,
        );
        if replacement.size != old_size
            || !self.tree.patch_component_subtree(component, &replacement)
        {
            return false;
        }
        self.components = self.tree.component_geometry();
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
        let mut dirty = false;
        for node in &graph.nodes {
            dirty |= self.synchronize_component(node.id, capabilities, geometry, registry)
                == LayoutSync::Dirty;
        }
        if dirty {
            LayoutSync::Dirty
        } else {
            LayoutSync::Stable
        }
    }

    /// Synchronizes only one retained component's layout callback. This is
    /// the R6b path for a topology-preserving local scope update.
    pub(crate) fn synchronize_component(
        &mut self,
        id: ComponentId,
        capabilities: &MountedCapabilities,
        geometry: &ComponentGeometryMap,
        registry: &mut ComponentRegistry,
    ) -> LayoutSync {
        let Some(handler) = capabilities
            .get(id)
            .and_then(|caps| caps.layout_changed.as_ref())
            .cloned()
        else {
            self.delivered.remove(&id);
            return LayoutSync::Stable;
        };
        let Some(entry) = geometry.entries.get(&id) else {
            unreachable!("mounted component has no layout geometry");
        };
        let size = entry.content.size();
        if self.delivered.get(&id).copied() == Some(size) {
            return LayoutSync::Stable;
        }
        self.delivered.insert(id, size);
        registry.with_any_mut(id, |component| handler(component, size));
        LayoutSync::Dirty
    }
}
