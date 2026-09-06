//! Public semantic terminal-root composition.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::presentation::{ContentProvider, EmptyContentProvider, StyleFacts, StyleStates};
use crate::{History, View};

/// The semantic root of a terminal application.
///
/// A [`Scene`] contains an optional root-level [`History`] followed by one
/// ordinary [`View`] body. It is intentionally not a nested composition value:
/// roots cannot be nested inside ordinary presentation composition.
///
/// ```text
/// let scene = Scene::new(crate::presentation::factory::text("ordinary application"));
///
/// let mut history = History::new();
/// history.push(crate::presentation::factory::text("earlier output"))?;
/// let scene = Scene::with_history(history, crate::presentation::factory::text("body"));
/// ```
pub struct Scene {
    history: Option<History>,
    body: View,
    layout_body: View,
    layout_root: View,
}

impl Scene {
    /// Creates a body-only semantic root.
    pub fn new(body: View) -> Self {
        let layout_body = body.clone().map_node(|node| {
            node.width = crate::presentation::ir::WidthRule::Fill;
            node.height = crate::presentation::ir::HeightRule::Fill;
        });
        let layout_root = root_view(None, layout_body.clone());
        Self {
            history: None,
            layout_body,
            layout_root,
            body,
        }
    }

    /// Creates a semantic root with one root-level History and an ordinary
    /// body below it.
    pub fn with_history(history: History, body: View) -> Self {
        let layout_body = body.clone().map_node(|node| {
            node.width = crate::presentation::ir::WidthRule::Fill;
            node.height = crate::presentation::ir::HeightRule::Fill;
        });
        let layout_root = root_view(None, layout_body.clone());
        Self {
            history: Some(history),
            layout_body,
            layout_root,
            body,
        }
    }

    /// Returns the optional root-level History.
    pub fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }

    pub(crate) fn layout_body(&self) -> &View {
        &self.layout_body
    }

    pub(crate) fn layout_root(&self) -> &View {
        &self.layout_root
    }

    /// Returns mutable access to the optional root-level History.
    pub fn history_mut(&mut self) -> Option<&mut History> {
        self.history.as_mut()
    }

    /// Returns the ordinary body View.
    pub fn body(&self) -> &View {
        &self.body
    }

    /// Replaces the ordinary body View.
    pub fn set_body(&mut self, body: View) {
        self.layout_body = body.clone().map_node(|node| {
            node.width = crate::presentation::ir::WidthRule::Fill;
            node.height = crate::presentation::ir::HeightRule::Fill;
        });
        self.layout_root = root_view(None, self.layout_body.clone());
        self.body = body;
    }

    pub(crate) fn set_history(&mut self, history: History) {
        self.history = Some(history);
    }
}

use crate::{
    component::{ComponentId, ComponentRegistry},
    geometry::Size,
    history::{
        HistoryPhysicalOverlay, HistoryViewportAnchor, project_into_session_for_host_with_content,
    },
    presentation::{
        ir::{
            ColumnChild, ColumnView, HeightRule, PersistentSeq, TrackSize, ViewKind, ViewNodeParts,
            WidthRule,
        },
        layout::{LayoutCache, measure_view_with_overlay_and_cache_and_content},
    },
};

use super::{ResolveError, ResolveSession, ResolvedScene};

/// Private resolved root state used by the host/layout pipeline.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedRootScene {
    pub(crate) scene: ResolvedScene,
    /// Independently resolved body branch retained for body-local updates.
    pub(crate) body_scene: ResolvedScene,
    /// Independently resolved History branch retained for history-local
    /// projection updates.
    pub(crate) history_scene: Option<ResolvedScene>,
    /// Component identities resolved from the root-level History.
    pub(crate) history_components: HashSet<ComponentId>,
    /// The independently resolved body root used to decide whether a local
    /// component update can avoid rebuilding the history/root wrapper.
    pub(crate) body_view: View,
    pub(crate) history_overlay: Option<HistoryPhysicalOverlay>,
    pub(crate) history_overflow_rows: usize,
    pub(crate) history_height: u16,
    pub(crate) body_height: u16,
}

/// Resolves a semantic root by resolving the body once and the root-level
/// History independently, then merging both resolution domains in visual order.
#[allow(dead_code)]
pub(crate) fn resolve_root_scene(
    scene: &Scene,
    registry: &ComponentRegistry,
    size: Size,
) -> Result<ResolvedRootScene, ResolveError> {
    resolve_root_scene_with_anchor(scene, registry, size, HistoryViewportAnchor::FollowEnd)
}

pub(crate) fn resolve_root_scene_with_anchor(
    root: &Scene,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
) -> Result<ResolvedRootScene, ResolveError> {
    let mut cache = LayoutCache::default();
    resolve_root_scene_with_anchor_and_cache(root, registry, size, anchor, &mut cache)
}

pub(crate) fn resolve_root_scene_with_anchor_and_cache(
    root: &Scene,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
    cache: &mut LayoutCache,
) -> Result<ResolvedRootScene, ResolveError> {
    resolve_root_scene_with_anchor_and_cache_and_states(
        root,
        registry,
        size,
        anchor,
        cache,
        &crate::retained_state::StateFrameView::empty(),
    )
}

pub(crate) fn resolve_root_scene_with_anchor_and_cache_and_states(
    root: &Scene,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
    cache: &mut LayoutCache,
    states: &crate::retained_state::StateFrameView<'_>,
) -> Result<ResolvedRootScene, ResolveError> {
    let mut content = EmptyContentProvider;
    resolve_root_scene_with_anchor_and_cache_and_states_and_content(
        root,
        registry,
        size,
        anchor,
        cache,
        states,
        &mut content,
    )
}

pub(crate) fn resolve_root_scene_with_anchor_and_cache_and_states_and_content(
    root: &Scene,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
    cache: &mut LayoutCache,
    states: &crate::retained_state::StateFrameView<'_>,
    content: &mut dyn ContentProvider,
) -> Result<ResolvedRootScene, ResolveError> {
    let body_scene = resolve_branch(root.layout_body(), registry, states)?;
    let body_height = measure_view_with_overlay_and_cache_and_content(
        &body_scene.view,
        size.width,
        &body_scene.overlay,
        cache,
        content,
    )
    .height
    .min(size.height);
    let history_height = root
        .history
        .as_ref()
        .map_or(0, |_| size.height.saturating_sub(body_height));

    let body_view = body_scene.view.clone();
    let (history_scene, history_overlay, history_overflow_rows, history_components) =
        match root.history.as_ref() {
            Some(history) => {
                let mut session = ResolveSession::new(registry);
                session.set_state_snapshots(states);
                let projection = project_into_session_for_host_with_content(
                    history,
                    Size::new(size.width, history_height),
                    &mut session,
                    anchor,
                    content,
                )?;
                let history_scene = session.finish(projection.view);
                let history_components = history_scene.mounts.ids().collect();
                (
                    Some(history_scene),
                    projection.frozen_overlay,
                    projection.overflow_rows,
                    history_components,
                )
            }
            None => (None, None, 0, HashSet::new()),
        };
    let scene = merge_root_scene(
        history_scene.clone(),
        body_scene.clone(),
        root.layout_root(),
    )?;

    Ok(ResolvedRootScene {
        scene,
        body_scene,
        history_scene,
        history_components,
        body_view,
        history_overlay,
        history_overflow_rows,
        history_height,
        body_height,
    })
}

fn resolve_branch(
    view: &View,
    registry: &ComponentRegistry,
    states: &crate::retained_state::StateFrameView<'_>,
) -> Result<ResolvedScene, ResolveError> {
    let mut session = ResolveSession::new(registry);
    session.set_state_snapshots(states);
    let view = session.resolve_root(view)?;
    Ok(session.finish(view))
}

/// Resolves only the content owned by one changed component. The component
/// itself remains in the retained graph; direct children are re-parented to
/// it so the caller can splice this local result into the existing `MountGraph`.
pub(crate) fn resolve_component_subtree(
    view: &View,
    registry: &ComponentRegistry,
    parent: ComponentId,
) -> Result<ResolvedScene, ResolveError> {
    resolve_component_subtree_with_states(
        view,
        registry,
        parent,
        &crate::retained_state::StateFrameView::empty(),
    )
}

pub(crate) fn resolve_component_subtree_with_states(
    view: &View,
    registry: &ComponentRegistry,
    parent: ComponentId,
    states: &crate::retained_state::StateFrameView<'_>,
) -> Result<ResolvedScene, ResolveError> {
    let mut resolved = resolve_branch(view, registry, states)?;
    resolved.mounts.reparent_roots(parent);
    Ok(resolved)
}

pub(crate) fn merge_root_scene(
    history: Option<ResolvedScene>,
    body: ResolvedScene,
    layout_root: &View,
) -> Result<ResolvedScene, ResolveError> {
    let Some(history) = history else {
        let root_view = layout_root.clone();
        let mut content_paths = HashMap::with_capacity(body.content_paths.len());
        for (port_id, path) in body.content_paths {
            let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
            root_path.push(root_view.clone());
            root_path.extend(path);
            content_paths.insert(port_id, root_path);
        }
        let mut component_paths = HashMap::with_capacity(body.component_paths.len());
        for (component_id, path) in body.component_paths {
            let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
            root_path.push(root_view.clone());
            root_path.extend(path);
            component_paths.insert(component_id, root_path);
        }
        let content_path_components = body.content_path_components;
        return Ok(ResolvedScene {
            view: root_view,
            mounts: body.mounts,
            capabilities: body.capabilities,
            overlay: body.overlay,
            content_paths,
            component_paths,
            content_path_components,
        });
    };

    ensure_disjoint_mounts(&history, &body)?;
    let history_view = history.view;
    let body_view = body.view;
    let mut mounts = history.mounts.to_nodes();
    mounts.extend(body.mounts.to_nodes());
    let mut capabilities = history.capabilities;
    capabilities.entries.extend(body.capabilities.entries);
    let mut overlay = history.overlay;
    overlay.components.extend(body.overlay.components);
    let root_view = root_view(Some(history_view), body_view);
    let mut content_paths = HashMap::with_capacity(
        history
            .content_paths
            .len()
            .saturating_add(body.content_paths.len()),
    );
    for (port_id, path) in history.content_paths {
        let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
        root_path.push(root_view.clone());
        root_path.extend(path);
        content_paths.insert(port_id, root_path);
    }
    for (port_id, path) in body.content_paths {
        let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
        root_path.push(root_view.clone());
        root_path.extend(path);
        content_paths.insert(port_id, root_path);
    }
    let mut component_paths = HashMap::with_capacity(
        history
            .component_paths
            .len()
            .saturating_add(body.component_paths.len()),
    );
    for (component_id, path) in history.component_paths {
        let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
        root_path.push(root_view.clone());
        root_path.extend(path);
        component_paths.insert(component_id, root_path);
    }
    for (component_id, path) in body.component_paths {
        let mut root_path = Vec::with_capacity(path.len().saturating_add(1));
        root_path.push(root_view.clone());
        root_path.extend(path);
        component_paths.insert(component_id, root_path);
    }
    let mut content_path_components = history.content_path_components;
    for (component_id, ports) in body.content_path_components {
        content_path_components
            .entry(component_id)
            .or_default()
            .extend(ports);
    }
    for ports in content_path_components.values_mut() {
        ports.sort_unstable();
        ports.dedup();
    }

    Ok(ResolvedScene {
        view: root_view,
        mounts: crate::component::MountGraph::new(mounts),
        capabilities,
        overlay,
        content_paths,
        component_paths,
        content_path_components,
    })
}

fn ensure_disjoint_mounts(
    history: &ResolvedScene,
    body: &ResolvedScene,
) -> Result<(), ResolveError> {
    let body_ids = body.mounts.ids().collect::<HashSet<ComponentId>>();
    for id in history.mounts.ids() {
        if body_ids.contains(&id) {
            return Err(ResolveError::DuplicateComponent { id });
        }
    }
    Ok(())
}

fn root_view(history: Option<View>, body: View) -> View {
    let mut children = Vec::with_capacity(usize::from(history.is_some()) + 1);
    if let Some(history) = history {
        children.push(ColumnChild {
            track: TrackSize::Flex { min: 0 },
            view: history,
        });
    }
    children.push(ColumnChild {
        track: TrackSize::Content { max: None },
        view: body,
    });
    View::from_node(ViewNodeParts {
        width: WidthRule::Fill,
        height: HeightRule::Fill,
        decoration: Default::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        state_attachment: None,
        content_attachment: None,
        kind: ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(children),
            gap: 0,
        })),
    })
}
