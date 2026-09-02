//! Public semantic terminal-root composition.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::presentation::{ContentProvider, EmptyContentProvider, StyleFacts, StyleStates};
use crate::{History, IntoView, View};

/// The semantic root of a terminal application.
///
/// A [`Scene`] contains an optional root-level [`History`] followed by one
/// ordinary [`View`] body. It is intentionally not an [`IntoView`] value:
/// roots cannot be nested inside ordinary presentation composition.
///
/// ```text
/// let scene = Scene::new(View::text("ordinary application"));
///
/// let mut history = History::new();
/// history.push("earlier output")?;
/// let scene = Scene::with_history(history, View::text("body"));
/// ```
pub struct Scene {
    history: Option<History>,
    body: View,
    layout_body: View,
    layout_root: View,
}

impl Scene {
    /// Creates a body-only semantic root.
    pub fn new(body: impl IntoView) -> Self {
        let body = body.into_view();
        let layout_body = body.clone().fill_width().fill_height();
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
    pub fn with_history(history: History, body: impl IntoView) -> Self {
        let body = body.into_view();
        let layout_body = body.clone().fill_width().fill_height();
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
    pub fn set_body(&mut self, body: impl IntoView) {
        let body = body.into_view();
        self.layout_body = body.clone().fill_width().fill_height();
        self.layout_root = root_view(None, self.layout_body.clone());
        self.body = body;
    }

    pub(crate) fn set_history(&mut self, history: History) {
        self.history = Some(history);
    }

    pub(crate) fn next_stream_wakeup(&self) -> Option<std::time::Instant> {
        self.history.as_ref().and_then(History::next_stream_wakeup)
    }

    pub(crate) fn advance_streams(
        &mut self,
        now: std::time::Instant,
    ) -> Result<bool, crate::HistoryError> {
        self.history
            .as_mut()
            .map_or(Ok(false), |history| history.advance_streams(now))
    }
}

use crate::{
    component::{ComponentId, ComponentRegistry},
    geometry::Size,
    history::{HistoryPhysicalOverlay, HistoryViewportAnchor, project_into_session_for_host},
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
        &HashMap::new(),
    )
}

pub(crate) fn resolve_root_scene_with_anchor_and_cache_and_states(
    root: &Scene,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
    cache: &mut LayoutCache,
    states: &HashMap<u64, crate::retained_state::ViewStateSnapshot>,
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
    states: &HashMap<u64, crate::retained_state::ViewStateSnapshot>,
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
                let projection = project_into_session_for_host(
                    history,
                    Size::new(size.width, history_height),
                    &mut session,
                    anchor,
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
    states: &HashMap<u64, crate::retained_state::ViewStateSnapshot>,
) -> Result<ResolvedScene, ResolveError> {
    let mut session = ResolveSession::new(registry);
    session.set_state_snapshots(states);
    let view = session.resolve_root(view)?;
    Ok(session.finish(view))
}

/// Resolves only the content owned by one changed component. The component
/// itself remains in the retained graph; direct children are re-parented to
/// it so the caller can splice this local result into the existing MountGraph.
pub(crate) fn resolve_component_subtree(
    view: &View,
    registry: &ComponentRegistry,
    parent: ComponentId,
) -> Result<ResolvedScene, ResolveError> {
    resolve_component_subtree_with_states(view, registry, parent, &HashMap::new())
}

pub(crate) fn resolve_component_subtree_with_states(
    view: &View,
    registry: &ComponentRegistry,
    parent: ComponentId,
    states: &HashMap<u64, crate::retained_state::ViewStateSnapshot>,
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
        return Ok(ResolvedScene {
            view: layout_root.clone(),
            mounts: body.mounts,
            capabilities: body.capabilities,
            overlay: body.overlay,
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

    Ok(ResolvedScene {
        view: root_view(Some(history_view), body_view),
        mounts: crate::component::MountGraph::new(mounts),
        capabilities,
        overlay,
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
        kind: ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(children),
            gap: 0,
        })),
    })
}
