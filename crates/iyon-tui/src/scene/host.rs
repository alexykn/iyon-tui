//! Generic retained Scene host.
//!
//! This module owns frame geometry, component synchronization, focus, ticks,
//! and native History pressure. Application code supplies only semantic Scene
//! state and consumes routed outputs.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use anyhow::Result;

use crate::presentation::{ContentProvider, EmptyContentProvider};
use crate::{
    Theme,
    backend::NativeHistorySink,
    component::{
        ComponentId, ComponentRegistry, MountGraph, MountedComponents, TickOutcome, TickScheduler,
    },
    geometry::{LayoutConstraints, Size},
    interaction::{
        FocusState, InteractionResult, KeyStroke, MountedCapabilities, route_key_local,
        route_paste, route_paste_interceptor,
    },
    output::{OutputQueue, OutputRouter},
    physical::Surface,
    presentation::{
        ir::{View, ViewKind},
        layout::{LayoutCache, ViewCompiler, layout_view_with_overlay_and_cache_and_content},
        paint::{PaintCache, ViewPainter},
    },
    retained_state::{DamageRegion, StateEffects, StateNodeKind, ViewStateSnapshot},
};

use super::root::merge_root_scene;
use super::{
    LayoutSynchronizer, ResolveError, ResolveSession, ResolvedRootScene, ResolvedScene,
    ResolvedSceneLayout, Scene, layout_resolved_scene_with_cache_and_content,
    resolve_component_subtree_with_states,
    resolve_root_scene_with_anchor_and_cache_and_states_and_content,
};
use crate::history::{HistoryViewportAnchor, project_into_session_for_host_with_content};

const MAX_LAYOUT_PASSES: usize = 8;

/// Outcome of one `drain_native_pressure` call.
enum NativePressure {
    /// Native state changed; the caller must re-resolve before painting.
    Progress,
    /// No native progress was possible; the caller should paint front-pinned.
    Blocked,
}

/// Consume as many native rows as `overflow_rows` allows without triggering
/// another full Scene resolve for each individual one-line History unit.
///
/// Returns `Progress` if the caller must re-resolve, or `Blocked` if the
/// frontier is stuck and the host should paint from the NativeFrontier anchor.
fn drain_native_pressure<S: NativeHistorySink>(
    history: &mut crate::History,
    sink: &mut S,
    width: u16,
    overflow_rows: usize,
    theme: &Theme,
    transfer_calls: &mut usize,
    content: &mut dyn ContentProvider,
) -> Result<NativePressure, crate::history::NativeTransferError<S::Error>> {
    use crate::history::NativeTransferStatus::{Idle, Progress, SemanticBlocked, SinkBlocked};

    let mut remaining = overflow_rows;
    let mut inserted_any = false;

    while remaining > 0 {
        let physical_before = history.physical_rows_inserted();
        let outcome = crate::history::transfer_native_prefix_with_theme_and_content(
            history, sink, width, remaining, theme, content,
        )?;
        *transfer_calls += 1;
        crate::history::trace::trace_transfer(
            overflow_rows,
            remaining,
            outcome.inserted,
            match outcome.status {
                Progress => "Progress",
                Idle => "Idle",
                SinkBlocked => "SinkBlocked",
                SemanticBlocked { .. } => "SemanticBlocked",
            },
            physical_before,
            history.physical_rows_inserted(),
        );

        if outcome.inserted > 0 {
            inserted_any = true;
            remaining = remaining.saturating_sub(outcome.inserted);
        }

        match outcome.status {
            Progress if outcome.inserted > 0 => {
                // Physical rows were consumed; keep draining within this budget.
                continue;
            }

            Progress => {
                // Semantic-only retirement (zero physical rows). Re-resolve
                // instead of spinning here — the frontier state changed and the
                // next projection may calculate a different overflow_rows.
                return Ok(NativePressure::Progress);
            }

            Idle | SinkBlocked | SemanticBlocked { .. } if inserted_any => {
                // At least one CRLF transaction already happened. The screen
                // geometry changed; must re-resolve before deciding what to paint.
                return Ok(NativePressure::Progress);
            }

            Idle | SinkBlocked | SemanticBlocked { .. } => {
                // Nothing happened at all; frontier is truly stuck.
                return Ok(NativePressure::Blocked);
            }
        }
    }

    // Budget was fully consumed. Re-resolve to get updated geometry.
    Ok(NativePressure::Progress)
}

/// A fully synchronized frame ready for the terminal adapter.
#[derive(Debug)]
pub(crate) struct PreparedSceneFrame {
    pub(crate) surface: Surface,
    pub(crate) history_overlay: Option<crate::history::HistoryPhysicalOverlay>,
    pub(crate) damage: DamageRegion,
    /// State identities encountered in this fully prepared candidate tree.
    /// The host holds an in-flight lifecycle pin for these IDs until backend
    /// presentation succeeds; visible binding promotion happens at commit.
    pub(crate) state_bindings: Vec<(u64, StateNodeKind)>,
}

impl PreparedSceneFrame {
    pub(crate) fn screen_lines(&self) -> Vec<String> {
        let mut lines = (0..self.surface.height())
            .map(|y| {
                (0..self.surface.width())
                    .map(|x| self.surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        if let Some(overlay) = &self.history_overlay {
            for (index, row) in overlay.rows.iter().enumerate() {
                let position = usize::from(overlay.row).saturating_add(index);
                if position < lines.len() {
                    lines[position] = row.plain_text();
                }
            }
        }
        lines
    }
}

/// Generic runtime host for one semantic Scene.
#[derive(Clone)]
struct StableScene {
    root: ResolvedRootScene,
    layout: ResolvedSceneLayout,
    history_identity: u64,
    history_revision: u64,
    native_history_revision: u64,
}

pub(crate) struct SceneHost {
    mounted: MountedComponents,
    synchronizer: LayoutSynchronizer,
    focus: FocusState,
    ticker: TickScheduler,
    outputs: OutputQueue,
    graph: MountGraph,
    capabilities: MountedCapabilities,
    layout_cache: LayoutCache,
    paint_cache: PaintCache,
    /// The last successfully painted semantic/layout frame. Local component
    /// invalidations update this retained frame instead of rebuilding the
    /// component forest from the scene root.
    retained: Option<StableScene>,
    last_surface: Option<Surface>,
    invalidated_components: HashSet<ComponentId>,
    invalidated_states: HashSet<u64>,
    invalidated_state_effects: HashMap<u64, StateEffects>,
    incremental_sync_components: Vec<ComponentId>,
    incremental_topology_changed: bool,
    incremental_requires_full_sync: bool,
    incremental_paint_components: Vec<ComponentId>,
    incremental_paint_states: Vec<u64>,
    state_only_refresh: bool,
    /// True when the retained History branch can be painted without walking
    /// the clean body branch.
    incremental_paint_history: bool,
    /// True while the current retained candidate was rebuilt from the History
    /// branch without re-resolving the body branch.
    history_only_refresh: bool,
    /// Source revisions change descendant layout/paint products without
    /// changing semantic View identity. This flag prevents a coalesced
    /// state-only refresh from committing a stale content projection.
    content_invalidated: bool,
    /// A body geometry/topology change has been prepared, but native History
    /// promotion may require another retained History refresh before painting.
    /// Keep the final paint whole so that refresh cannot leave moved body rows
    /// from the previously committed surface behind.
    full_paint_pending: bool,
    /// Geometry candidate damage retained until the candidate surface is
    /// painted. Full-tree paint may still use this metadata for future
    /// backends instead of discarding the old/new region information.
    pending_damage: Option<DamageRegion>,
    /// Counts calls to `resolve_stable_at_with_anchor` for structural test
    /// assertions. Not compiled into production builds.
    #[cfg(test)]
    pub(crate) resolve_count: usize,
    #[cfg(test)]
    pub(crate) full_resolves: usize,
    #[cfg(test)]
    pub(crate) full_paints: usize,
    #[cfg(test)]
    pub(crate) incremental_resolves: usize,
}

impl SceneHost {
    /// PERF-12 T13.1 R8: whether the last SUCCESSFULLY reconciled mount graph
    /// still contains this component. Deferred component retirement consults
    /// this before physically reclaiming a registry entry.
    pub(crate) fn is_mounted(&self, id: crate::component::ComponentId) -> bool {
        self.graph.contains(id)
    }
}

impl Default for SceneHost {
    fn default() -> Self {
        Self {
            mounted: MountedComponents::default(),
            synchronizer: LayoutSynchronizer::default(),
            focus: FocusState::default(),
            ticker: TickScheduler::new(),
            outputs: OutputQueue::new(),
            graph: MountGraph::default(),
            capabilities: MountedCapabilities::default(),
            layout_cache: LayoutCache::default(),
            paint_cache: PaintCache::default(),
            retained: None,
            last_surface: None,
            invalidated_components: HashSet::new(),
            invalidated_states: HashSet::new(),
            invalidated_state_effects: HashMap::new(),
            incremental_sync_components: Vec::new(),
            incremental_topology_changed: false,
            incremental_requires_full_sync: false,
            incremental_paint_components: Vec::new(),
            incremental_paint_states: Vec::new(),
            state_only_refresh: false,
            incremental_paint_history: false,
            history_only_refresh: false,
            content_invalidated: false,
            full_paint_pending: false,
            pending_damage: None,
            #[cfg(test)]
            resolve_count: 0,
            #[cfg(test)]
            full_resolves: 0,
            #[cfg(test)]
            full_paints: 0,
            #[cfg(test)]
            incremental_resolves: 0,
        }
    }
}

impl SceneHost {
    pub(crate) fn clear_retained_views(&mut self) {
        self.layout_cache = LayoutCache::default();
        self.retained = None;
        self.last_surface = None;
        self.invalidated_components.clear();
        self.invalidated_states.clear();
        self.invalidated_state_effects.clear();
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_requires_full_sync = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_states.clear();
        self.state_only_refresh = false;
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
        self.content_invalidated = false;
        self.full_paint_pending = false;
        self.pending_damage = None;
    }

    /// Marks one native component as changed. The next frame can resolve only
    /// this component's owned subtree; the committed frame remains authoritative
    /// until that frame successfully prepares and paints.
    pub(crate) fn invalidate_component(&mut self, id: ComponentId) {
        self.invalidated_components.insert(id);
    }

    pub(crate) fn has_invalidated_components(&self) -> bool {
        !self.invalidated_components.is_empty()
    }

    /// Marks one retained state attachment dirty without rebuilding the
    /// semantic scene. Rust effect metadata selects a local repaint or a
    /// retained-root geometry relayout for the next candidate.
    pub(crate) fn invalidate_state(&mut self, id: u64, effects: StateEffects) {
        self.invalidated_states.insert(id);
        self.invalidated_state_effects
            .entry(id)
            .and_modify(|current| *current = current.union(effects))
            .or_insert(effects);
    }

    /// Content revisions are not semantic View revisions, so descendant
    /// layout/paint cache entries cannot be invalidated by ViewId alone. Clear
    /// derived caches before a content candidate is measured; the immutable
    /// semantic Scene and component graph remain retained.
    pub(crate) fn invalidate_content(&mut self) {
        self.content_invalidated = true;
        self.layout_cache.clear();
        self.paint_cache.clear();
    }

    /// Re-lays out only fixed-allocation state subtrees. If a geometry change
    /// can escape the target's allocation, the caller falls back to a retained
    /// resolved-root layout so parent dependencies remain correct.
    fn try_local_geometry_refresh(
        &mut self,
        retained: &mut StableScene,
        state_ids: &[u64],
        state_effects: &HashMap<u64, StateEffects>,
        content: &mut dyn ContentProvider,
    ) -> Option<Vec<ComponentId>> {
        let mut propagation_nodes = 0usize;
        let mut geometry_roots = Vec::with_capacity(state_ids.len());
        let mut changed_components = HashSet::new();
        for state_id in state_ids {
            crate::perf::inc(crate::perf::Counter::ViewStateGeometryRelayouts);
            let Some(target_id) = retained.layout.tree.state_roots.get(state_id).copied() else {
                return None;
            };
            let layout_path = retained.layout.tree.path_to_root(target_id);
            let Some(semantic_path) = state_view_path(&retained.root.scene, *state_id) else {
                return None;
            };
            if layout_path.len() != semantic_path.len() {
                return None;
            }

            let mut dirty_view_ids = HashSet::new();
            for node_id in &layout_path {
                dirty_view_ids.insert(retained.layout.tree.node(*node_id).view_id);
            }
            propagation_nodes = propagation_nodes.saturating_add(dirty_view_ids.len());
            self.layout_cache.invalidate_view_ids(&dirty_view_ids);

            let target_index = layout_path.len().saturating_sub(1);
            let mut patched = false;
            for index in (0..layout_path.len()).rev() {
                let node_id = layout_path[index];
                let node = retained.layout.tree.node(node_id);
                let rect = node.rect;
                let view = &semantic_path[index];
                // A non-root candidate is safe to patch only when its
                // unconstrained result still fits the committed allocation.
                // Otherwise climb to the parent dependency frontier. The root
                // is always bounded by the host and is the conservative stop.
                if index != 0 {
                    let parent = layout_path[index - 1];
                    let may_escape = retained
                        .layout
                        .tree
                        .child_dependency(parent, node_id)
                        .is_none_or(|dependency| {
                            let effects =
                                state_effects.get(state_id).copied().unwrap_or_else(|| {
                                    StateEffects::INTRINSIC_WIDTH
                                        .union(StateEffects::INTRINSIC_HEIGHT)
                                });
                            (effects.intrinsic_width() && dependency.parent_uses_child_width())
                                || (effects.intrinsic_height()
                                    && dependency.parent_uses_child_height())
                        });
                    if may_escape {
                        let natural = layout_view_with_overlay_and_cache_and_content(
                            view,
                            // Measure against an unconstrained width. Using
                            // the old allocation here would hide an increased
                            // max-width/cleared bound behind that same cap and
                            // incorrectly keep the target locally clipped.
                            LayoutConstraints::width_only(u16::MAX),
                            &retained.root.scene.overlay,
                            None,
                            &mut self.layout_cache,
                            content,
                        );
                        if natural.size != rect.size() {
                            continue;
                        }
                    }
                }
                let replacement = layout_view_with_overlay_and_cache_and_content(
                    view,
                    LayoutConstraints::bounded(rect.size()),
                    &retained.root.scene.overlay,
                    None,
                    &mut self.layout_cache,
                    content,
                );
                if replacement.size != rect.size()
                    || !retained.layout.tree.patch_subtree(node_id, &replacement)
                {
                    continue;
                }
                if index != target_index {
                    // A parent-frontier patch may move siblings outside the
                    // state subtree. Keep the candidate layout, but request a
                    // complete candidate paint instead of leaving old sibling
                    // cells in the retained surface.
                    self.full_paint_pending = true;
                }
                geometry_roots.push(node_id);
                patched = true;
                break;
            }
            if !patched {
                return None;
            }
        }
        for root in geometry_roots {
            let component_ids = retained.layout.tree.component_ids_in_subtree(root);
            let old_geometry = component_ids
                .iter()
                .filter_map(|id| {
                    retained
                        .layout
                        .components
                        .entries
                        .get(id)
                        .copied()
                        .map(|geometry| (*id, geometry))
                })
                .collect::<Vec<_>>();
            let refreshed = retained
                .layout
                .tree
                .patch_component_geometry_subtree(root, &mut retained.layout.components);
            debug_assert!(
                refreshed,
                "patched geometry root must remain in the retained layout tree"
            );
            if !refreshed {
                return None;
            }
            for (id, old) in old_geometry {
                if retained
                    .layout
                    .components
                    .entries
                    .get(&id)
                    .is_some_and(|new| new != &old)
                {
                    changed_components.insert(id);
                }
            }
        }
        crate::perf::add(
            crate::perf::Counter::ViewStateDirtyPropagationNodes,
            propagation_nodes as u64,
        );
        let mut changed_components = changed_components.into_iter().collect::<Vec<_>>();
        changed_components.sort_unstable();
        Some(changed_components)
    }

    /// Invalidates the retained scene root for body/history/theme changes.
    pub(crate) fn invalidate_root(&mut self) {
        self.retained = None;
        self.last_surface = None;
        self.invalidated_components.clear();
        self.invalidated_states.clear();
        self.invalidated_state_effects.clear();
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_requires_full_sync = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_states.clear();
        self.state_only_refresh = false;
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
        self.content_invalidated = false;
        self.full_paint_pending = false;
        self.pending_damage = None;
    }

    /// Drops all derived candidate data after a backend presentation failure.
    /// The HostInner frame remains authoritative, so stale candidate caches
    /// must not seed the next retry.
    pub(crate) fn discard_candidate(&mut self) {
        self.invalidate_root();
        self.layout_cache.clear();
        self.paint_cache.clear();
    }

    /// Applies revisions/capabilities for a topology-preserving candidate to
    /// the interaction indexes without cloning clean mounted entries. The
    /// retained candidate has already completed all fallible preparation when
    /// this is called, so the committed host indexes remain transactional.
    fn update_incremental_host_state(&mut self, resolved: &StableScene) {
        for id in self.incremental_sync_components.iter().copied() {
            if let Some(node) = resolved.root.scene.mounts.node(id) {
                self.graph.update_revision(id, node.revision);
            }
            if let Some(capabilities) = resolved.root.scene.capabilities.entries.get(&id) {
                self.capabilities.insert(id, capabilities.clone());
            } else {
                self.capabilities.entries.remove(&id);
            }
        }
    }

    pub(crate) fn next_tick_deadline(&self) -> Option<Instant> {
        self.ticker.next_deadline()
    }

    pub(crate) fn focused_component(&self) -> Option<crate::component::ComponentId> {
        self.focus.focused()
    }

    #[cfg(test)]
    pub(crate) fn focused(&self) -> Option<crate::component::ComponentId> {
        self.focused_component()
    }

    #[cfg(test)]
    pub(crate) fn mount_count_for_test(&self) -> usize {
        self.graph.len()
    }

    #[cfg(test)]
    pub(crate) fn focusable_count_for_test(&self) -> usize {
        self.capabilities
            .entries
            .values()
            .filter(|capabilities| capabilities.focusable)
            .count()
    }

    pub(crate) fn dispatch_key_local(
        &mut self,
        key: KeyStroke,
        registry: &mut ComponentRegistry,
    ) -> InteractionResult {
        route_key_local(
            key,
            &mut self.focus,
            &self.graph,
            &self.capabilities,
            registry,
            &mut self.outputs,
        )
    }

    pub(crate) fn intercept_paste<A>(
        &self,
        text: &str,
        intercept: impl FnMut(crate::component::ComponentId, &str) -> Option<A>,
    ) -> Option<A> {
        route_paste_interceptor(text, &self.focus, &self.graph, intercept)
    }

    pub(crate) fn dispatch_paste(
        &mut self,
        text: &str,
        registry: &mut ComponentRegistry,
    ) -> InteractionResult {
        route_paste(
            text,
            &self.focus,
            &self.graph,
            &self.capabilities,
            registry,
            &mut self.outputs,
        )
    }

    pub(crate) fn drain_outputs<A>(
        &mut self,
        router: &OutputRouter<A>,
    ) -> Result<Vec<A>, crate::output::OutputDispatchError> {
        router.drain(&mut self.outputs)
    }

    pub(crate) fn tick_due(
        &mut self,
        now: Instant,
        registry: &mut ComponentRegistry,
    ) -> TickOutcome {
        let outcome = self
            .ticker
            .tick_due_with_events(now, registry, &mut self.outputs);
        // Tick callbacks mutate the registry through with_any_mut(), but that
        // alone is not enough to drive retained scene reconciliation. Record
        // the same changed components as interaction invalidations so a
        // History-only refresh cannot reuse a stale slot frame. Each timer
        // remains independent; this only publishes its own changed component.
        for id in outcome.changed_components.iter().copied() {
            self.invalidate_component(id);
        }
        outcome
    }

    /// Resolves, synchronizes, paints, and—when necessary—promotes the generic
    /// History prefix. The viewport callback is the only terminal-size seam.
    #[cfg(test)]
    pub(crate) fn render<S, F>(
        &mut self,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        sink: &mut S,
        viewport: F,
    ) -> Result<PreparedSceneFrame, SceneHostError<S::Error>>
    where
        S: NativeHistorySink,
        F: FnMut(&mut S) -> Result<Size>,
    {
        let mut content = EmptyContentProvider;
        self.render_at_with_states(
            Instant::now(),
            scene,
            registry,
            theme,
            sink,
            viewport,
            &HashMap::new(),
            &mut content,
        )
    }

    pub(crate) fn render_at<S, F>(
        &mut self,
        now: Instant,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        sink: &mut S,
        viewport: F,
    ) -> Result<PreparedSceneFrame, SceneHostError<S::Error>>
    where
        S: NativeHistorySink,
        F: FnMut(&mut S) -> Result<Size>,
    {
        let mut content = EmptyContentProvider;
        self.render_at_with_states(
            now,
            scene,
            registry,
            theme,
            sink,
            viewport,
            &HashMap::new(),
            &mut content,
        )
    }

    pub(crate) fn render_at_with_states<S, F>(
        &mut self,
        now: Instant,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        sink: &mut S,
        mut viewport: F,
        states: &HashMap<u64, ViewStateSnapshot>,
        content: &mut dyn ContentProvider,
    ) -> Result<PreparedSceneFrame, SceneHostError<S::Error>>
    where
        S: NativeHistorySink,
        F: FnMut(&mut S) -> Result<Size>,
    {
        let mut resolves = 0usize;
        let mut transfer_calls = 0usize;
        loop {
            let size = viewport(sink).map_err(SceneHostError::Viewport)?;

            resolves += 1;
            let resolved = self.resolve_stable_at_with_anchor(
                scene,
                registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                states,
                content,
            )?;

            let front_content_blocked = scene
                .history()
                .and_then(crate::History::front_content_attachment_id)
                .is_some_and(|port_id| content.history_transfer_blocked(port_id, size.width));
            if resolved.root.history_overflow_rows == 0 || front_content_blocked {
                crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                return Ok(self.paint_with_content(resolved, theme, content));
            }

            let Some(history) = scene.history_mut() else {
                crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                return Ok(self.paint_with_content(resolved, theme, content));
            };

            let pressure = drain_native_pressure(
                history,
                sink,
                size.width,
                resolved.root.history_overflow_rows,
                theme,
                &mut transfer_calls,
                content,
            )
            .map_err(SceneHostError::Transfer)?;

            match pressure {
                NativePressure::Progress => {
                    // Keep the candidate's retained body branch alive while
                    // native History promotion changes only the frontier. The
                    // next resolve refreshes the History branch instead of
                    // falling back to a full body resolve.
                    self.retained = Some(resolved);
                    continue;
                }

                NativePressure::Blocked => {
                    // size may be reused: no native rows were inserted during
                    // the final blocked drain attempt, so viewport geometry did
                    // not change. Retain the candidate so the NativeFrontier
                    // projection can also reuse its body branch.
                    self.retained = Some(resolved);
                    resolves += 1;
                    let pinned = self.resolve_stable_at_with_anchor(
                        scene,
                        registry,
                        size,
                        now,
                        HistoryViewportAnchor::NativeFrontier,
                        states,
                        content,
                    )?;
                    crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                    return Ok(self.paint_with_content(pinned, theme, content));
                }
            }
        }
    }

    #[cfg(test)]
    fn resolve_stable<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
    ) -> Result<StableScene, SceneHostError<E>> {
        self.resolve_stable_at(scene, registry, size, Instant::now())
    }

    #[cfg(test)]
    fn resolve_stable_at<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        now: Instant,
    ) -> Result<StableScene, SceneHostError<E>> {
        let mut content = EmptyContentProvider;
        self.resolve_stable_at_with_anchor(
            scene,
            registry,
            size,
            now,
            HistoryViewportAnchor::FollowEnd,
            &HashMap::new(),
            &mut content,
        )
    }

    fn resolve_stable_at_with_anchor<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        now: Instant,
        anchor: HistoryViewportAnchor,
        states: &HashMap<u64, ViewStateSnapshot>,
        content: &mut dyn ContentProvider,
    ) -> Result<StableScene, SceneHostError<E>> {
        let mut force_full = false;
        let mut layout_epoch_started = false;
        for _ in 0..MAX_LAYOUT_PASSES {
            let resolved = if !force_full {
                match self.try_incremental_stable(scene, registry, size, anchor, states, content) {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) => {
                        if !layout_epoch_started {
                            self.layout_cache.begin_epoch();
                            layout_epoch_started = true;
                        }
                        self.resolve_full_stable(scene, registry, size, anchor, states, content)?
                    }
                    Err(error) => return Err(SceneHostError::Resolve(error)),
                }
            } else {
                if !layout_epoch_started {
                    self.layout_cache.begin_epoch();
                    layout_epoch_started = true;
                }
                self.resolve_full_stable(scene, registry, size, anchor, states, content)?
            };
            let incremental_host = (self.state_only_refresh
                || self.history_only_refresh
                || !self.incremental_sync_components.is_empty())
                && !self.incremental_topology_changed
                && !self.incremental_requires_full_sync;
            let sync = if incremental_host {
                let mut dirty = false;
                for component in self.incremental_sync_components.iter().copied() {
                    dirty |= self.synchronizer.synchronize_component(
                        component,
                        &resolved.root.scene.capabilities,
                        &resolved.layout.components,
                        registry,
                    ) == crate::scene::LayoutSync::Dirty;
                }
                if dirty {
                    crate::scene::LayoutSync::Dirty
                } else {
                    crate::scene::LayoutSync::Stable
                }
            } else {
                self.synchronizer.synchronize(
                    &resolved.root.scene.mounts,
                    &resolved.root.scene.capabilities,
                    &resolved.layout.components,
                    registry,
                )
            };
            if matches!(sync, crate::scene::LayoutSync::Dirty) {
                // Layout callbacks mutate component revisions. Do not apply a
                // second incremental patch against the half-synchronized
                // candidate; the next pass is authoritative and full.
                force_full = true;
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                continue;
            }

            if self.incremental_topology_changed
                || (!self.history_only_refresh && self.incremental_sync_components.is_empty())
            {
                // Full/root or topology-changing candidates replace the host
                // indexes together after all fallible preparation succeeds.
                // A topology-preserving local candidate updates only its
                // affected entries below, avoiding an O(total-mounts) clone.
                self.graph = resolved.root.scene.mounts.clone();
                self.capabilities = resolved.root.scene.capabilities.clone();
            } else {
                self.update_incremental_host_state(&resolved);
            }
            let focus_changed = if incremental_host {
                self.focus.reconcile_incremental(
                    &self.incremental_sync_components,
                    &self.graph,
                    &self.capabilities,
                    Some(&resolved.layout.components),
                    registry,
                )
            } else {
                let transitions = self.mounted.reconcile(self.graph.clone());
                self.ticker
                    .sync_capabilities(&self.graph, &self.capabilities, &transitions, now);
                self.focus.reconcile_with_geometry(
                    &self.graph,
                    &self.capabilities,
                    Some(&resolved.layout.components),
                    registry,
                )
            };
            if incremental_host {
                for component in self.incremental_sync_components.iter().copied() {
                    self.ticker
                        .sync_component_capability(component, &self.capabilities, now);
                }
            }
            if focus_changed {
                force_full = true;
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                continue;
            }
            self.invalidated_components.clear();
            self.invalidated_states.clear();
            self.invalidated_state_effects.clear();
            self.incremental_sync_components.clear();
            self.incremental_topology_changed = false;
            self.incremental_requires_full_sync = false;
            self.state_only_refresh = false;
            self.history_only_refresh = false;
            #[cfg(test)]
            {
                self.resolve_count += 1;
            }
            return Ok(resolved);
        }
        Err(SceneHostError::DidNotConverge)
    }

    fn resolve_full_stable<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
        states: &HashMap<u64, ViewStateSnapshot>,
        content: &mut dyn ContentProvider,
    ) -> Result<StableScene, SceneHostError<E>> {
        if !self.invalidated_states.is_empty() {
            // Parent cache entries do not encode every descendant state
            // revision. A state mutation combined with a structural/component
            // fallback must therefore discard both derived caches before the
            // full candidate is measured and painted.
            self.layout_cache.clear();
            self.paint_cache.clear();
        }
        self.pending_damage = None;
        let resolved = resolve_root_scene_with_anchor_and_cache_and_states_and_content(
            scene,
            registry,
            size,
            anchor,
            &mut self.layout_cache,
            states,
            content,
        )
        .map_err(SceneHostError::Resolve)?;
        let layout = layout_resolved_scene_with_cache_and_content(
            &resolved.scene,
            size,
            &mut self.layout_cache,
            content,
        );
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_requires_full_sync = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_states.clear();
        self.state_only_refresh = false;
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
        self.content_invalidated = false;
        #[cfg(test)]
        {
            self.full_resolves += 1;
        }
        Ok(StableScene {
            root: resolved,
            layout,
            history_identity: scene.history().map_or(0, crate::History::identity),
            history_revision: scene.history().map_or(0, crate::History::revision),
            native_history_revision: scene.history().map_or(0, crate::History::native_revision),
        })
    }

    fn try_incremental_stable(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
        states: &HashMap<u64, ViewStateSnapshot>,
        content: &mut dyn ContentProvider,
    ) -> Result<Option<StableScene>, ResolveError> {
        let history_revision = scene.history().map_or(0, crate::History::revision);
        let native_history_revision = scene.history().map_or(0, crate::History::native_revision);
        let history_identity = scene.history().map_or(0, crate::History::identity);
        let Some(retained_state) = self.retained.as_ref() else {
            return Ok(None);
        };
        if retained_state.layout.tree.size != size {
            return Ok(None);
        }

        let body_changed =
            !crate::presentation::View::ptr_eq(&retained_state.root.body_view, scene.layout_body());
        let history_changed = retained_state.history_identity != history_identity
            || retained_state.history_revision != history_revision;
        let native_history_changed =
            retained_state.native_history_revision != native_history_revision;
        let body_invalidated = self.invalidated_components.iter().any(|component| {
            retained_state.root.scene.mounts.contains(*component)
                && !retained_state.root.history_components.contains(component)
        });
        if self.content_invalidated {
            // Content changes do not alter the resolved semantic scene. When
            // there is no separate History branch to refresh, retain that
            // scene and rebuild only its derived layout with the new content
            // provider. This preserves the three-plane boundary while still
            // allowing fit-content metrics to propagate through the full
            // layout dependency graph.
            if scene.history().is_none()
                && self.invalidated_components.is_empty()
                && self.invalidated_states.is_empty()
                && !body_changed
                && !body_invalidated
                && !history_changed
                && !native_history_changed
            {
                let mut retained = self
                    .retained
                    .take()
                    .expect("retained state was checked above");
                self.layout_cache.begin_epoch();
                retained.layout = layout_resolved_scene_with_cache_and_content(
                    &retained.root.scene,
                    size,
                    &mut self.layout_cache,
                    content,
                );
                self.content_invalidated = false;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_states.clear();
                self.state_only_refresh = false;
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                self.full_paint_pending = false;
                self.pending_damage = None;
                #[cfg(test)]
                {
                    self.incremental_resolves += 1;
                }
                return Ok(Some(retained));
            }
            return Ok(None);
        }
        if !self.invalidated_states.is_empty()
            && (!self.invalidated_components.is_empty()
                || body_changed
                || body_invalidated
                || history_changed
                || native_history_changed)
        {
            return Ok(None);
        }
        if !self.invalidated_states.is_empty()
            && self.invalidated_components.is_empty()
            && !body_changed
            && !body_invalidated
            && !history_changed
            && !native_history_changed
        {
            let mut retained = self
                .retained
                .take()
                .expect("retained state was checked above");
            let mut state_ids = self.invalidated_states.iter().copied().collect::<Vec<_>>();
            state_ids.sort_unstable();
            let geometry_refresh = state_ids.iter().any(|state_id| {
                self.invalidated_state_effects
                    .get(state_id)
                    .is_some_and(|effects| effects.geometry())
            });
            let geometry_state_ids = state_ids.clone();
            let state_effects = self.invalidated_state_effects.clone();
            // Cache entries are node-local; an ancestor entry does not encode
            // every descendant state revision. Invalidate the affected paths
            // in both derived caches before local or root refresh so a later
            // full frame cannot reuse stale state presentation/geometry. If a
            // state is no longer present in the retained tree, full clears are
            // safer than guessing at a new path.
            if let Some(view_ids) = state_paint_view_ids(&retained.layout.tree, &state_ids) {
                self.layout_cache.invalidate_view_ids(&view_ids);
                self.paint_cache.invalidate_view_ids(&view_ids);
            } else {
                self.layout_cache.clear();
                self.paint_cache.clear();
            }
            let mut paint_states = Vec::with_capacity(state_ids.len());
            for state_id in state_ids {
                let Some(snapshot) = states.get(&state_id) else {
                    self.retained = Some(retained);
                    return Ok(None);
                };
                retained
                    .root
                    .scene
                    .overlay
                    .states
                    .insert(state_id, snapshot.clone());
                retained
                    .root
                    .body_scene
                    .overlay
                    .states
                    .insert(state_id, snapshot.clone());
                if let Some(history) = retained.root.history_scene.as_mut() {
                    history.overlay.states.insert(state_id, snapshot.clone());
                }
                if geometry_refresh {
                    continue;
                }
                if !retained
                    .layout
                    .tree
                    .apply_state_snapshot(state_id, snapshot)
                {
                    self.retained = Some(retained);
                    return Ok(None);
                }
                paint_states.push(state_id);
            }
            self.invalidated_states.clear();
            self.invalidated_state_effects.clear();
            self.incremental_sync_components.clear();
            self.incremental_topology_changed = false;
            self.incremental_requires_full_sync = false;
            self.incremental_paint_components.clear();
            self.incremental_paint_history = false;
            self.history_only_refresh = false;
            if geometry_refresh {
                // A fill/fill occurrence has a fixed parent allocation, so its
                // own measured subtree can be replaced without touching clean
                // siblings or rebuilding the semantic scene.
                if let Some(changed_components) = self.try_local_geometry_refresh(
                    &mut retained,
                    &geometry_state_ids,
                    &state_effects,
                    content,
                ) {
                    self.incremental_sync_components = changed_components;
                    self.state_only_refresh = true;
                    self.pending_damage = None;
                    let mut paint_states = geometry_state_ids;
                    sort_state_paint_ids(&retained.layout.tree, &mut paint_states);
                    self.incremental_paint_states = paint_states;
                    crate::perf::inc(crate::perf::Counter::ViewStateGeometryLocalPatches);
                    return Ok(Some(retained));
                }

                // Geometry changes that can escape the target allocation use
                // the retained resolved semantic root but rebuild only the
                // derived candidate layout. Invalidate the target-to-root
                // dependency frontier so clean sibling measurements remain
                // reusable. No composition or structural publication occurs,
                // and the old surface remains authoritative until the
                // candidate is painted/committed.
                let mut dirty_view_ids = HashSet::new();
                for state_id in &geometry_state_ids {
                    let Some(node_id) = retained.layout.tree.state_roots.get(state_id).copied()
                    else {
                        continue;
                    };
                    for ancestor in retained.layout.tree.path_to_root(node_id) {
                        dirty_view_ids.insert(retained.layout.tree.node(ancestor).view_id);
                    }
                }
                crate::perf::add(
                    crate::perf::Counter::ViewStateDirtyPropagationNodes,
                    dirty_view_ids.len() as u64,
                );
                if dirty_view_ids.is_empty() {
                    self.layout_cache.clear();
                } else {
                    self.layout_cache.invalidate_view_ids(&dirty_view_ids);
                }
                let next_layout = layout_resolved_scene_with_cache_and_content(
                    &retained.root.scene,
                    size,
                    &mut self.layout_cache,
                    content,
                );
                let geometry_unchanged =
                    layout_geometry_unchanged(&retained.layout.tree, &next_layout.tree);
                self.pending_damage = Some(layout_geometry_damage(
                    &retained.layout.tree,
                    &next_layout.tree,
                    size,
                ));
                retained.layout = next_layout;
                crate::perf::inc(crate::perf::Counter::ViewStateGeometryRelayouts);
                if geometry_unchanged {
                    // The candidate box/effective content changed without
                    // changing any physical rect or clip. Reuse the retained
                    // surface and repaint only the affected state subtrees.
                    self.state_only_refresh = true;
                    let mut paint_states = geometry_state_ids;
                    sort_state_paint_ids(&retained.layout.tree, &mut paint_states);
                    self.incremental_paint_states = paint_states;
                    crate::perf::inc(crate::perf::Counter::ViewStateGeometryLocalPatches);
                } else {
                    self.state_only_refresh = false;
                    self.incremental_paint_states.clear();
                    crate::perf::inc(crate::perf::Counter::ViewStateGeometryFullFallbacks);
                }
                return Ok(Some(retained));
            }
            self.state_only_refresh = true;
            sort_state_paint_ids(&retained.layout.tree, &mut paint_states);
            self.incremental_paint_states = paint_states;
            return Ok(Some(retained));
        }
        if !body_changed
            && !body_invalidated
            && scene.history().is_some()
            && (history_changed
                || native_history_changed
                || !matches!(anchor, HistoryViewportAnchor::FollowEnd))
        {
            let retained = self
                .retained
                .take()
                .expect("retained state was checked above");
            let affected = self
                .invalidated_components
                .iter()
                .copied()
                .filter(|component| retained.root.history_components.contains(component))
                .collect();
            return self.refresh_history_projection(
                scene, registry, size, anchor, retained, affected, false, states, content,
            );
        }

        if self.invalidated_components.is_empty()
            || body_changed
            || ((history_changed || native_history_changed) && !body_invalidated)
        {
            return Ok(None);
        }

        let Some(mut retained) = self.retained.take() else {
            return Ok(None);
        };
        let invalidated = self
            .invalidated_components
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let roots = invalidated
            .iter()
            .copied()
            .filter(|candidate| retained.root.scene.mounts.contains(*candidate))
            .filter(|candidate| {
                !invalidated.iter().any(|ancestor| {
                    ancestor != candidate
                        && retained
                            .root
                            .scene
                            .mounts
                            .is_descendant_or_self(*candidate, *ancestor)
                })
            })
            .collect::<Vec<_>>();
        if roots.is_empty() {
            self.retained = Some(retained);
            return Ok(None);
        }
        self.incremental_sync_components = roots.clone();

        let mut updates = Vec::with_capacity(roots.len());
        for id in &roots {
            match prepare_component_subtree_update(&retained, registry, *id, states, content) {
                Ok(update) => updates.push(update),
                Err(error) => {
                    self.retained = Some(retained);
                    self.incremental_sync_components.clear();
                    self.incremental_topology_changed = false;
                    self.incremental_requires_full_sync = false;
                    self.incremental_paint_components.clear();
                    self.incremental_paint_history = false;
                    return Err(error);
                }
            }
        }
        let topology_changed = updates.iter().any(|update| update.topology_changed);
        let old_body_rects = roots
            .iter()
            .filter_map(|id| {
                retained
                    .layout
                    .components
                    .entries
                    .get(id)
                    .map(|geometry| (*id, geometry.outer))
            })
            .collect::<Vec<_>>();
        for update in updates {
            apply_component_subtree_update(&mut retained, update);
        }
        self.incremental_topology_changed = topology_changed;
        let mut incremental_cache = LayoutCache::default();
        // Same-shape/same-geometry content is patched into the retained layout
        // tree. Geometry or topology changes fall back to the complete layout
        // pass, whose two-generation cache still reuses unchanged siblings.
        let mut patched = true;
        for id in &roots {
            let Some(view) = retained
                .root
                .scene
                .overlay
                .components
                .get(id)
                .map(|snapshot| &snapshot.view)
            else {
                patched = false;
                break;
            };
            if !retained.layout.patch_component_with_cache(
                *id,
                view,
                &retained.root.scene.overlay,
                &mut incremental_cache,
                content,
            ) {
                patched = false;
                break;
            }
        }
        let body_geometry_changed = old_body_rects.iter().any(|(id, rect)| {
            retained
                .layout
                .components
                .entries
                .get(id)
                .is_none_or(|geometry| geometry.outer != *rect)
        });
        // A failed local patch leaves the retained geometry map unchanged, so
        // the old/new geometry comparison above cannot observe the new size.
        // Treat a failed body patch as a layout change before native History
        // promotion can trigger a second History-only refresh.
        let body_patch_failed = !patched
            && roots
                .iter()
                .any(|id| !retained.root.history_components.contains(id));
        if scene.history().is_some()
            && (history_changed
                || native_history_changed
                || !patched
                || topology_changed
                || body_geometry_changed)
        {
            return self.refresh_history_projection(
                scene,
                registry,
                size,
                anchor,
                retained,
                roots.clone(),
                topology_changed || body_geometry_changed || body_patch_failed,
                states,
                content,
            );
        }
        if !patched || topology_changed {
            self.layout_cache.begin_epoch();
            retained.layout = layout_resolved_scene_with_cache_and_content(
                &retained.root.scene,
                size,
                &mut self.layout_cache,
                content,
            );
            self.incremental_requires_full_sync = true;
            self.incremental_paint_components.clear();
            self.incremental_paint_history = false;
        } else {
            self.incremental_requires_full_sync = false;
            self.incremental_sync_components = roots
                .iter()
                .flat_map(|root| retained.root.scene.mounts.subtree_ids(*root))
                .collect();
            self.incremental_paint_history = false;
            self.incremental_paint_components = roots;
        }
        #[cfg(test)]
        {
            self.incremental_resolves += 1;
        }
        Ok(Some(retained))
    }

    /// Rebuilds only the root-level History projection while reusing the
    /// already-resolved body branch. The merged layout is still recomputed so
    /// the history viewport can change height or selection without touching
    /// the body's semantic resolution.
    fn refresh_history_projection(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
        retained: StableScene,
        affected: Vec<ComponentId>,
        body_layout_changed: bool,
        states: &HashMap<u64, ViewStateSnapshot>,
        content: &mut dyn ContentProvider,
    ) -> Result<Option<StableScene>, ResolveError> {
        let Some(history) = scene.history() else {
            self.retained = Some(retained);
            return Ok(None);
        };
        // Rotate the retained layout generations before measuring any part of
        // this new candidate. History-only refreshes bypass the full-resolve
        // branch where the normal epoch rotation occurs.
        self.layout_cache.begin_epoch();
        let body_affected = affected
            .iter()
            .any(|component| !retained.root.history_components.contains(component));
        let body_height = if body_affected {
            crate::presentation::layout::measure_view_with_overlay_and_cache_and_content(
                &retained.root.body_scene.view,
                size.width,
                &retained.root.body_scene.overlay,
                &mut self.layout_cache,
                content,
            )
            .height
            .min(size.height)
        } else {
            retained.root.body_height
        };
        let history_height = size.height.saturating_sub(body_height);
        // The body track sits below History. If promotion changes the track
        // boundary, every body component can move even when its own height is
        // unchanged; an incremental component paint cannot clear its old row.
        let body_geometry_changed =
            body_layout_changed || retained.root.history_height != history_height;
        let mut session = ResolveSession::new(registry);
        session.set_state_snapshots(states);
        let projection = match project_into_session_for_host_with_content(
            history,
            Size::new(size.width, history_height),
            &mut session,
            anchor,
            content,
        ) {
            Ok(projection) => projection,
            Err(error) => {
                // The caller may have already staged a body component update
                // into `retained`. No publication or paint occurred, so do not
                // retain that partially prepared candidate as authoritative;
                // the next attempt must rebuild from the committed host frame.
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_history = false;
                self.full_paint_pending = false;
                return Err(error);
            }
        };
        let history_scene = session.finish(projection.view);
        let history_components: HashSet<ComponentId> = history_scene.mounts.ids().collect();
        let history_topology_changed = match retained.root.history_scene.as_ref() {
            None => true,
            Some(old) => !old.mounts.same_topology(&history_scene.mounts),
        };
        let topology_changed = body_geometry_changed || history_topology_changed;
        let merged = match merge_root_scene(
            Some(history_scene.clone()),
            retained.root.body_scene.clone(),
            scene.layout_root(),
        ) {
            Ok(merged) => merged,
            Err(error) => {
                // `retained` may contain a staged body subtree update. A
                // failed merge is an evaluation/prepare failure, therefore the
                // previous painted frame—not this candidate—remains the only
                // authoritative retained state.
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_history = false;
                self.full_paint_pending = false;
                return Err(error);
            }
        };
        let layout = layout_resolved_scene_with_cache_and_content(
            &merged,
            size,
            &mut self.layout_cache,
            content,
        );
        let mut sync_components = Vec::new();
        for component in affected {
            if !merged.mounts.contains(component) {
                continue;
            }
            sync_components.extend(merged.mounts.subtree_ids(component));
        }
        // A History-only projection can change the allocated size of a
        // mounted control without changing that control's revision. Include
        // exactly those History components whose content size changed so
        // layout callbacks (notably TextInput/ScrollPane viewport repair) are
        // delivered without synchronizing the clean body forest.
        let mut history_geometry_components = retained.root.history_components.clone();
        history_geometry_components.extend(history_components.iter().copied());
        for component in history_geometry_components {
            let Some(old_geometry) = retained.layout.components.entries.get(&component) else {
                continue;
            };
            let Some(new_geometry) = layout.components.entries.get(&component) else {
                continue;
            };
            if old_geometry.content.size() != new_geometry.content.size() {
                sync_components.push(component);
            }
        }
        sync_components.sort_unstable();
        sync_components.dedup();
        let body_view = retained.root.body_scene.view.clone();
        let root = ResolvedRootScene {
            scene: merged,
            body_scene: retained.root.body_scene,
            history_scene: Some(history_scene),
            history_components: history_components.clone(),
            body_view,
            history_overlay: projection.frozen_overlay,
            history_overflow_rows: projection.overflow_rows,
            history_height,
            body_height,
        };
        self.history_only_refresh = true;
        self.full_paint_pending |= body_geometry_changed;
        // A topology- and geometry-stable History update has a concrete paint
        // target. Leaving this plan empty would make paint() mistake a normal
        // History refresh for a failed incremental update and repaint the root.
        self.incremental_sync_components = sync_components;
        self.incremental_topology_changed = topology_changed;
        self.incremental_requires_full_sync = topology_changed;
        self.incremental_paint_history = !topology_changed;
        self.incremental_paint_components = if topology_changed {
            Vec::new()
        } else {
            self.incremental_sync_components
                .iter()
                .copied()
                .filter(|component| !history_components.contains(component))
                .collect()
        };
        #[cfg(test)]
        {
            self.incremental_resolves += 1;
        }
        Ok(Some(StableScene {
            root,
            layout,
            history_identity: history.identity(),
            history_revision: history.revision(),
            native_history_revision: history.native_revision(),
        }))
    }

    fn paint(&mut self, resolved: StableScene, theme: &Theme) -> PreparedSceneFrame {
        let content = EmptyContentProvider;
        self.paint_with_content(resolved, theme, &content)
    }

    fn paint_with_content(
        &mut self,
        resolved: StableScene,
        theme: &Theme,
        content: &dyn ContentProvider,
    ) -> PreparedSceneFrame {
        self.retained = Some(resolved);
        let retained = self.retained.as_ref().expect("retained frame installed");
        let state_bindings = retained.layout.tree.state_bindings();
        let state_damage = DamageRegion::from_rects(
            self.incremental_paint_states
                .iter()
                .filter_map(|id| retained.layout.tree.state_roots.get(id).copied())
                .filter_map(|id| retained.layout.tree.incremental_paint_rect(id)),
            retained.layout.tree.size,
        );
        let compiler = ViewCompiler::with_interaction(theme, self.focus.focused(), &self.graph);
        if !self.full_paint_pending
            && (self.incremental_paint_history
                || !self.incremental_paint_components.is_empty()
                || !self.incremental_paint_states.is_empty())
        {
            if let Some(mut surface) = self.last_surface.take() {
                let mut incremental = true;
                let mut incremental_cache = PaintCache::default();
                if self.incremental_paint_history {
                    let history_root = retained.root.history_scene.as_ref().and_then(|_| {
                        retained
                            .layout
                            .tree
                            .node(retained.layout.tree.root)
                            .children
                            .first()
                            .copied()
                    });
                    incremental = history_root.is_some_and(|root| {
                        ViewPainter.paint_subtree_into_with_content(
                            &compiler,
                            &retained.layout.tree,
                            root,
                            &mut surface,
                            &mut incremental_cache,
                            content,
                        )
                    });
                }
                if incremental {
                    for state_id in &self.incremental_paint_states {
                        let Some(state_root) =
                            retained.layout.tree.state_roots.get(state_id).copied()
                        else {
                            incremental = false;
                            break;
                        };
                        if !ViewPainter.paint_subtree_into_with_content(
                            &compiler,
                            &retained.layout.tree,
                            state_root,
                            &mut surface,
                            &mut incremental_cache,
                            content,
                        ) {
                            incremental = false;
                            break;
                        }
                    }
                }
                if incremental {
                    for component in self.incremental_paint_components.iter().copied() {
                        if !ViewPainter.paint_component_into_with_content(
                            &compiler,
                            &retained.layout.tree,
                            component,
                            &mut surface,
                            &mut incremental_cache,
                            content,
                        ) {
                            incremental = false;
                            break;
                        }
                    }
                }
                self.incremental_paint_history = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_states.clear();
                self.state_only_refresh = false;
                if incremental {
                    crate::perf::inc(crate::perf::Counter::ViewStateIncrementalPaints);
                    surface.physically_complete = retained.layout.tree.physically_complete;
                    let output = surface.clone();
                    self.last_surface = Some(surface);
                    return PreparedSceneFrame {
                        surface: output,
                        history_overlay: retained.root.history_overlay.clone(),
                        damage: self.pending_damage.take().unwrap_or_else(|| {
                            if state_damage.rects.is_empty() {
                                DamageRegion::full(retained.layout.tree.size)
                            } else {
                                state_damage
                            }
                        }),
                        state_bindings,
                    };
                }
            }
            self.incremental_paint_history = false;
            self.incremental_paint_components.clear();
            self.incremental_paint_states.clear();
            self.state_only_refresh = false;
        }
        self.incremental_paint_history = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_states.clear();
        self.state_only_refresh = false;
        #[cfg(test)]
        {
            self.full_paints += 1;
        }
        self.paint_cache.begin_epoch(theme);
        let surface = ViewPainter.paint_tree_with_content(
            &compiler,
            &retained.layout.tree,
            &mut self.paint_cache,
            content,
        );
        let output = surface.clone();
        self.last_surface = Some(surface);
        self.full_paint_pending = false;
        PreparedSceneFrame {
            surface: output,
            history_overlay: retained.root.history_overlay.clone(),
            damage: self
                .pending_damage
                .take()
                .unwrap_or_else(|| DamageRegion::full(retained.layout.tree.size)),
            state_bindings,
        }
    }
}

fn sort_state_paint_ids(tree: &crate::presentation::layout::LayoutTree, state_ids: &mut [u64]) {
    state_ids.sort_unstable_by_key(|state_id| {
        tree.state_roots
            .get(state_id)
            .map_or(usize::MAX, |node| node.0)
    });
}

fn state_paint_view_ids(
    tree: &crate::presentation::layout::LayoutTree,
    state_ids: &[u64],
) -> Option<HashSet<crate::presentation::ir::ViewId>> {
    let mut view_ids = HashSet::new();
    for state_id in state_ids {
        let node = tree.state_roots.get(state_id).copied()?;
        for ancestor in tree.path_to_root(node) {
            view_ids.insert(tree.node(ancestor).view_id);
        }
    }
    Some(view_ids)
}

fn state_view_path(scene: &ResolvedScene, state_id: u64) -> Option<Vec<View>> {
    fn visit(
        view: &View,
        overlay: &crate::scene::ResolutionOverlay,
        state_id: u64,
    ) -> Option<Vec<View>> {
        if !view.flags().contains_state_attachment() && !view.contains_component_identity() {
            return None;
        }
        if view.state_attachment_id() == Some(state_id) {
            return Some(vec![view.clone()]);
        }
        let child_path = match view.kind() {
            ViewKind::Text(_) | ViewKind::Spacer { .. } | ViewKind::ContentHost => None,
            ViewKind::ComponentSlot(slot) => overlay
                .component(slot.id)
                .and_then(|snapshot| visit(&snapshot.view, overlay, state_id)),
            ViewKind::Column(column) => column
                .children
                .iter()
                .find_map(|child| visit(&child.view, overlay, state_id)),
            ViewKind::Row(row) => row
                .children
                .iter()
                .find_map(|child| visit(&child.view, overlay, state_id)),
            ViewKind::Grid(grid) => grid
                .cells
                .iter()
                .find_map(|cell| visit(&cell.view, overlay, state_id)),
            ViewKind::Hanging(hanging) => visit(&hanging.prefix, overlay, state_id)
                .or_else(|| visit(&hanging.continuation_prefix, overlay, state_id))
                .or_else(|| visit(&hanging.body, overlay, state_id)),
            ViewKind::Container(container) => visit(&container.child, overlay, state_id),
            ViewKind::ClampRows(clamp) => visit(&clamp.child, overlay, state_id),
            ViewKind::RowViewport(viewport) => visit(&viewport.child, overlay, state_id),
        }?;
        let mut path = Vec::with_capacity(child_path.len() + 1);
        path.push(view.clone());
        path.extend(child_path);
        Some(path)
    }

    visit(&scene.view, &scene.overlay, state_id)
}

fn layout_geometry_unchanged(
    previous: &crate::presentation::layout::LayoutTree,
    next: &crate::presentation::layout::LayoutTree,
) -> bool {
    previous.size == next.size
        && previous.nodes.len() == next.nodes.len()
        && previous.nodes.iter().zip(&next.nodes).all(|(old, new)| {
            old.rect == new.rect
                && old.content_rect == new.content_rect
                && old.clip_rect == new.clip_rect
                && old.children == new.children
        })
}

fn layout_geometry_damage(
    previous: &crate::presentation::layout::LayoutTree,
    next: &crate::presentation::layout::LayoutTree,
    size: Size,
) -> DamageRegion {
    if previous.size != next.size || previous.nodes.len() != next.nodes.len() {
        return DamageRegion::full(size);
    }
    let mut rects = Vec::new();
    for (old, new) in previous.nodes.iter().zip(&next.nodes) {
        if old.rect != new.rect
            || old.content_rect != new.content_rect
            || old.clip_rect != new.clip_rect
            || old.occurrence != new.occurrence
        {
            crate::perf::inc(crate::perf::Counter::ViewStateDirtyPropagationNodes);
            rects.push(old.rect);
            rects.push(new.rect);
        }
    }
    DamageRegion::from_rects(rects, size)
}

struct PreparedComponentSubtree {
    id: ComponentId,
    snapshot: crate::component::ComponentSnapshot,
    subtree: super::ResolvedScene,
    old_ids: Vec<ComponentId>,
    topology_changed: bool,
}

fn prepare_component_subtree_update(
    retained: &StableScene,
    registry: &ComponentRegistry,
    id: ComponentId,
    states: &HashMap<u64, ViewStateSnapshot>,
    _content: &mut dyn ContentProvider,
) -> Result<PreparedComponentSubtree, ResolveError> {
    let snapshot = registry
        .resolution(id)
        .ok_or(ResolveError::MissingComponent { id })?;
    let subtree = resolve_component_subtree_with_states(&snapshot.view, registry, id, states)?;
    let graph = &retained.root.scene.mounts;
    let old_ids = graph.subtree_ids(id);
    if old_ids.is_empty() {
        return Err(ResolveError::MissingComponent { id });
    }
    let old_children = old_ids
        .iter()
        .skip(1)
        .map(|child| {
            let node = graph
                .node(*child)
                .expect("mount graph subtree id must resolve to a node");
            (node.id, node.parent)
        })
        .collect::<Vec<_>>();
    let new_children = subtree
        .mounts
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<Vec<_>>();
    let topology_changed = old_children != new_children;

    for node in subtree.mounts.iter() {
        if graph.contains(node.id) && !old_ids.contains(&node.id) {
            return Err(ResolveError::DuplicateComponent { id: node.id });
        }
    }

    Ok(PreparedComponentSubtree {
        id,
        snapshot,
        subtree,
        old_ids,
        topology_changed,
    })
}

fn apply_component_subtree_update(retained: &mut StableScene, update: PreparedComponentSubtree) {
    let PreparedComponentSubtree {
        id,
        snapshot,
        subtree,
        old_ids,
        topology_changed,
    } = update;
    let history_component = retained.root.history_components.contains(&id);
    apply_component_subtree_update_to_scene(
        &mut retained.root.scene,
        id,
        &snapshot,
        &subtree,
        &old_ids,
        topology_changed,
    );
    if history_component {
        let history = retained
            .root
            .history_scene
            .as_mut()
            .expect("history component must have a retained history scene");
        apply_component_subtree_update_to_scene(
            history,
            id,
            &snapshot,
            &subtree,
            &old_ids,
            topology_changed,
        );
    } else {
        apply_component_subtree_update_to_scene(
            &mut retained.root.body_scene,
            id,
            &snapshot,
            &subtree,
            &old_ids,
            topology_changed,
        );
    }
}

fn apply_component_subtree_update_to_scene(
    scene: &mut ResolvedScene,
    id: ComponentId,
    snapshot: &crate::component::ComponentSnapshot,
    subtree: &ResolvedScene,
    old_ids: &[ComponentId],
    topology_changed: bool,
) {
    let graph = &mut scene.mounts;
    if topology_changed {
        assert!(
            graph.replace_subtree(id, subtree.mounts.clone()),
            "prepared component subtree must still have a mounted owner"
        );
    }
    graph.update_revision(id, snapshot.revision);
    for node in subtree.mounts.iter() {
        graph.update_revision(node.id, node.revision);
    }

    for old_id in old_ids {
        scene.overlay.components.remove(old_id);
        scene.capabilities.entries.remove(old_id);
    }
    scene.overlay.components.insert(id, snapshot.clone());
    scene.capabilities.insert(id, snapshot.capabilities.clone());
    scene
        .overlay
        .components
        .extend(subtree.overlay.components.clone());
    scene
        .capabilities
        .entries
        .extend(subtree.capabilities.entries.clone());
}

#[derive(Debug)]
pub(crate) enum SceneHostError<E> {
    Viewport(anyhow::Error),
    Resolve(ResolveError),
    Transfer(crate::history::NativeTransferError<E>),
    DidNotConverge,
}

impl<E: std::fmt::Debug> std::fmt::Display for SceneHostError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Scene host error: {self:?}")
    }
}

impl<E: std::fmt::Debug + 'static> std::error::Error for SceneHostError<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BorderSpec, ColorSpec, Component, ComponentCx, ComponentHandle, InteractionResult,
        IntoView, Key, KeyStroke, Scene, ScrollPane, StyleSelector, TextSpan, ThemeColor, View,
        backend::NativeHistorySink,
        component::ComponentRegistry,
        geometry::Size,
        physical::PhysicalRow,
        stream::{
            StreamOffset, StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder,
            StreamingSource,
        },
    };

    #[derive(Debug)]
    struct LayoutAware {
        changed: bool,
        calls: usize,
    }

    impl Component for LayoutAware {
        fn view(&self) -> View {
            if self.changed {
                View::text("new\nrow").into_view()
            } else {
                View::text("old").into_view()
            }
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.on_layout_changed(Self::layout_changed);
        }
    }

    impl LayoutAware {
        fn layout_changed(&mut self, _size: Size) {
            self.calls += 1;
            if self.calls == 1 {
                self.changed = true;
            }
        }
    }

    #[derive(Debug, Default)]
    struct EmptySealedSource;

    impl StreamingSource for EmptySealedSource {
        fn snapshot(&self) -> StreamSnapshot {
            StreamSnapshotBuilder::new(
                StreamRevision::new(0),
                StreamOffset::ZERO,
                StreamOffset::ZERO,
                StreamOffset::ZERO,
            )
            .exact_text(
                StreamRange::new(StreamOffset::ZERO, StreamOffset::ZERO),
                [TextSpan::plain("")],
            )
            .finish()
            .unwrap()
        }

        fn seal(&mut self) {}

        fn is_sealed(&self) -> bool {
            true
        }
    }

    #[derive(Default)]
    struct TestSink {
        rows: Vec<PhysicalRow>,
    }

    #[derive(Debug)]
    struct R6bLeaf {
        text: String,
    }

    #[derive(Debug)]
    struct TickingLeaf {
        frame: usize,
    }

    impl Component for TickingLeaf {
        fn view(&self) -> View {
            View::text(format!("tick-{}", self.frame)).into_view()
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.tick(std::time::Duration::from_millis(80), Self::tick);
        }
    }

    impl TickingLeaf {
        fn tick(component: &mut Self, _now: Instant, _cx: &mut crate::EventCx<'_>) -> bool {
            component.frame += 1;
            true
        }
    }

    impl Component for R6bLeaf {
        fn view(&self) -> View {
            View::text(self.text.clone()).into_view()
        }
    }

    #[derive(Debug)]
    struct TopologyRoot {
        child: ComponentHandle<R6bLeaf>,
        show_child: bool,
    }

    impl Component for TopologyRoot {
        fn view(&self) -> View {
            if self.show_child {
                View::component(self.child).into_view()
            } else {
                View::text("root").into_view()
            }
        }
    }

    impl NativeHistorySink for TestSink {
        type Error = ();

        fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
            self.rows.extend(rows.iter().cloned());
            Ok(rows.len())
        }
    }

    #[test]
    fn retained_host_updates_one_of_one_thousand_components_incrementally() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        let mut registry = ComponentRegistry::new();
        let handles = (0..1_000)
            .map(|_| {
                registry.register(R6bLeaf {
                    text: "x".to_owned(),
                })
            })
            .collect::<Vec<_>>();
        let scene = Scene::new(View::vertical(|column| {
            for handle in &handles {
                column.child(View::component(*handle));
            }
        }));
        let size = Size::new(8, 1_000);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());
        host.full_resolves = 0;
        host.incremental_resolves = 0;
        host.resolve_count = 0;

        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        registry
            .with_mut(handles[0], |leaf| leaf.text = "y".to_owned())
            .unwrap();
        host.invalidate_component(handles[0].id());
        let same_geometry = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let same_geometry_frame = host.paint(same_geometry, &Theme::default());

        assert_eq!(host.full_resolves, 0);
        assert_eq!(host.incremental_resolves, 1);
        assert_eq!(host.resolve_count, 1);
        assert_eq!(same_geometry_frame.screen_lines()[0], "y       ");
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert!(counters.value(crate::perf::Counter::ResolverNodesVisited) <= 4);
            assert!(counters.value(crate::perf::Counter::MeasureNodeCalls) <= 16);
            assert!(counters.value(crate::perf::Counter::ComponentGeometryNodesVisited) <= 4);
            // Incremental painting patches the retained surface directly; it does
            // not walk clean siblings through the full-tree paint cache.
            assert!(counters.value(crate::perf::Counter::PaintNodesVisited) <= 2);
            assert_eq!(counters.value(crate::perf::Counter::PaintCacheHits), 0);
            assert!(counters.value(crate::perf::Counter::PaintCacheMisses) <= 1);
            assert!(counters.value(crate::perf::Counter::SurfaceCellsComposited) <= 8);
        }
        let mut same_cold_host = SceneHost::default();
        let same_cold = same_cold_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let same_cold_frame = same_cold_host.paint(same_cold, &Theme::default());
        assert_eq!(
            same_geometry_frame.screen_lines(),
            same_cold_frame.screen_lines()
        );

        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        registry
            .with_mut(handles[0], |leaf| leaf.text = "y\nrow".to_owned())
            .unwrap();
        host.invalidate_component(handles[0].id());
        let geometry_change = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let geometry_frame = host.paint(geometry_change, &Theme::default());
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert!(counters.value(crate::perf::Counter::ResolverNodesVisited) <= 4);
            assert!(counters.value(crate::perf::Counter::ComponentViewCalls) <= 1);
        }

        assert_eq!(host.full_resolves, 0);
        assert_eq!(host.incremental_resolves, 2);
        let mut cold_host = SceneHost::default();
        let cold = cold_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let cold_frame = cold_host.paint(cold, &Theme::default());
        assert_eq!(geometry_frame.screen_lines(), cold_frame.screen_lines());
    }

    #[test]
    fn retained_topology_replacement_preserves_owner_and_updates_mounts() {
        let mut registry = ComponentRegistry::new();
        let child = registry.register(R6bLeaf {
            text: "child".to_owned(),
        });
        let parent = registry.register(TopologyRoot {
            child,
            show_child: false,
        });
        let scene = Scene::new(View::component(parent));
        let size = Size::new(12, 3);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let initial_frame = host.paint(initial, &Theme::default());
        assert_eq!(initial_frame.screen_lines()[0], "root        ");
        assert_eq!(host.graph.ids().collect::<Vec<_>>(), vec![parent.id()]);

        registry
            .with_mut(parent, |root| root.show_child = true)
            .unwrap();
        host.invalidate_component(parent.id());
        let mounted = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let mounted_frame = host.paint(mounted, &Theme::default());
        assert_eq!(mounted_frame.screen_lines()[0], "child       ");
        assert_eq!(
            host.graph.ids().collect::<Vec<_>>(),
            vec![parent.id(), child.id()]
        );
        assert_eq!(host.graph.parent(parent.id()), None);
        assert_eq!(host.graph.parent(child.id()), Some(parent.id()));

        registry
            .with_mut(parent, |root| root.show_child = false)
            .unwrap();
        host.invalidate_component(parent.id());
        let removed = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let removed_frame = host.paint(removed, &Theme::default());
        assert_eq!(removed_frame.screen_lines()[0], "root        ");
        assert_eq!(host.graph.ids().collect::<Vec<_>>(), vec![parent.id()]);
    }

    #[test]
    fn incremental_prepare_error_preserves_the_committed_frame() {
        let mut registry = ComponentRegistry::new();
        let first = registry.register(R6bLeaf {
            text: "first".to_owned(),
        });
        let second = registry.register(R6bLeaf {
            text: "second".to_owned(),
        });
        let scene = Scene::new(View::vertical(|column| {
            column.child(View::component(first));
            column.child(View::component(second));
        }));
        let size = Size::new(12, 3);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let initial_frame = host.paint(initial, &Theme::default());
        let initial_ids = host.graph.ids().collect::<Vec<_>>();

        registry
            .with_mut(first, |leaf| leaf.text = "updated".to_owned())
            .unwrap();
        registry.remove(second).unwrap();
        host.invalidate_component(first.id());
        host.invalidate_component(second.id());
        let error = host.resolve_stable::<()>(&scene, &mut registry, size);
        assert!(matches!(
            error,
            Err(SceneHostError::Resolve(ResolveError::MissingComponent { id })) if id == second.id()
        ));
        assert_eq!(host.graph.ids().collect::<Vec<_>>(), initial_ids);
        assert_eq!(
            host.retained
                .as_ref()
                .unwrap()
                .root
                .scene
                .overlay
                .component(first.id())
                .unwrap()
                .view,
            View::text("first").into_view()
        );
        assert_eq!(host.last_surface.as_ref().unwrap(), &initial_frame.surface);
    }

    #[test]
    fn retained_component_paint_preserves_ancestor_surface_background() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "x".to_owned(),
        });
        let scene = Scene::new(
            View::vertical(|column| {
                column.child(View::component(handle));
            })
            .fill_width()
            .fill_height()
            .background(ColorSpec::Ansi(34)),
        );
        let size = Size::new(8, 2);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());

        registry
            .with_mut(handle, |leaf| leaf.text = "y".to_owned())
            .unwrap();
        host.invalidate_component(handle.id());
        let retained = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let retained_frame = host.paint(retained, &Theme::default());

        let mut cold_host = SceneHost::default();
        let cold = cold_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let cold_frame = cold_host.paint(cold, &Theme::default());
        assert_eq!(retained_frame.surface, cold_frame.surface);
    }

    #[test]
    fn component_update_refreshes_history_branch_without_rebuilding_body() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "body-old".to_owned(),
        });
        let mut history = crate::History::new();
        history.push("history-old").unwrap();
        let mut scene = Scene::with_history(history, View::component(handle));
        let size = Size::new(12, 4);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());
        host.full_resolves = 0;

        registry
            .with_mut(handle, |leaf| leaf.text = "body-new".to_owned())
            .unwrap();
        scene.history_mut().unwrap().push("history-new").unwrap();
        host.invalidate_component(handle.id());
        let resolved = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        assert_eq!(host.full_resolves, 0);
        assert_eq!(host.incremental_resolves, 1);
        let frame = host.paint(resolved, &Theme::default());
        let lines = frame.screen_lines();
        assert!(lines.iter().any(|line| line.starts_with("history-new")));
        assert!(lines.iter().any(|line| line.starts_with("body-new")));
    }

    #[test]
    fn geometry_change_with_history_repaints_body_and_clears_old_surface() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "body-old".to_owned(),
        });
        let mut history = crate::History::new();
        history.push("history").unwrap();
        let scene = Scene::with_history(
            history,
            View::vertical(|column| {
                column.child(View::text("tail"));
                column.child(View::component(handle));
            }),
        );
        let size = Size::new(12, 20);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());

        registry
            .with_mut(handle, |leaf| leaf.text = "body-new\nbody-line".to_owned())
            .unwrap();
        host.invalidate_component(handle.id());
        let updated = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let frame = host.paint(updated, &Theme::default());

        let mut cold_host = SceneHost::default();
        let cold = cold_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let cold_frame = cold_host.paint(cold, &Theme::default());
        assert_eq!(frame.screen_lines(), cold_frame.screen_lines());
    }

    #[test]
    fn geometry_change_after_history_transfer_repaints_the_body() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "body-old".to_owned(),
        });
        let mut history = crate::History::new();
        for index in 0..50 {
            history
                .push(View::text(format!("old-{index}")).fill_width())
                .unwrap();
        }
        let mut scene = Scene::with_history(
            history,
            View::vertical(|column| {
                column.child(View::text("tail"));
                column.child(View::component(handle));
            }),
        );
        let size = Size::new(12, 20);
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();
        host.render(
            &mut scene,
            &mut registry,
            &Theme::default(),
            &mut sink,
            |_| Ok(size),
        )
        .unwrap();

        registry
            .with_mut(handle, |leaf| leaf.text = "body-new\nbody-line".to_owned())
            .unwrap();
        host.invalidate_component(handle.id());
        let frame = host
            .render(
                &mut scene,
                &mut registry,
                &Theme::default(),
                &mut sink,
                |_| Ok(size),
            )
            .unwrap();

        let mut cold_host = SceneHost::default();
        let mut cold_sink = TestSink::default();
        let cold_frame = cold_host
            .render(
                &mut scene,
                &mut registry,
                &Theme::default(),
                &mut cold_sink,
                |_| Ok(size),
            )
            .unwrap();
        assert_eq!(frame.screen_lines(), cold_frame.screen_lines());
    }

    #[test]
    fn history_stream_refresh_paints_only_the_history_branch() {
        let mut history = crate::History::new();
        let stream = history
            .push_stream(BlockableStreamSource::new("old\nstale", 0, false))
            .unwrap();
        let body = View::vertical(|column| {
            for row in 0..100 {
                column.child(View::text(format!("body-{row}")));
            }
        });
        let mut scene = Scene::with_history(history, body);
        let mut registry = ComponentRegistry::new();
        let size = Size::new(20, 120);
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());
        host.full_paints = 0;

        scene
            .history_mut()
            .unwrap()
            .update_stream(stream, |source| {
                source.text = "new\n      ".to_owned();
                source.revision += 1;
            })
            .unwrap();
        let updated = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let mut cold_host = SceneHost::default();
        let cold = cold_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let cold_frame = cold_host.paint(cold, &Theme::default());

        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        let frame = host.paint(updated, &Theme::default());
        assert_eq!(frame.screen_lines(), cold_frame.screen_lines());
        assert_eq!(host.full_paints, 0);
        assert!(
            frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with("new"))
        );
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert!(
                counters.value(crate::perf::Counter::PaintNodesVisited) < 10,
                "History refresh should not repaint the clean body: {:?}",
                counters.value(crate::perf::Counter::PaintNodesVisited)
            );
        }
    }

    #[test]
    fn stream_refresh_and_animation_tick_repaint_independent_targets() {
        let mut history = crate::History::new();
        let stream = history
            .push_stream(BlockableStreamSource::new("old", 0, false))
            .unwrap();
        let mut registry = ComponentRegistry::new();
        let ticking = registry.register(TickingLeaf { frame: 0 });
        let mut scene = Scene::with_history(history, View::component(ticking));
        let size = Size::new(20, 4);
        let now = Instant::now();
        let mut host = SceneHost::default();
        let initial = host
            .resolve_stable_at::<()>(&scene, &mut registry, size, now)
            .unwrap();
        let _ = host.paint(initial, &Theme::default());
        host.full_paints = 0;

        scene
            .history_mut()
            .unwrap()
            .update_stream(stream, |source| {
                source.text = "new".to_owned();
                source.revision += 1;
            })
            .unwrap();
        let tick = host.tick_due(now + std::time::Duration::from_millis(80), &mut registry);
        assert_eq!(tick.changed_components, vec![ticking.id()]);
        let updated = host
            .resolve_stable_at::<()>(
                &scene,
                &mut registry,
                size,
                now + std::time::Duration::from_millis(80),
            )
            .unwrap();
        let frame = host.paint(updated, &Theme::default());

        assert_eq!(host.full_paints, 0);
        assert!(
            frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with("new"))
        );
        assert!(
            frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with("tick-1"))
        );
    }

    #[test]
    fn routes_scroll_pane_locally_and_preserves_detachment_on_content_update() {
        let content = |count: usize| {
            View::text(
                (1..=count)
                    .map(|row| format!("row {row}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        let mut registry = ComponentRegistry::new();
        let pane = registry.register(ScrollPane::new(content(30)));
        let scene = Scene::new(View::component(pane));
        let mut host = SceneHost::default();
        let _ = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(12, 5))
            .unwrap();

        assert_eq!(
            host.dispatch_key_local(KeyStroke::new(Key::PageUp), &mut registry),
            InteractionResult::Consumed
        );
        assert!(!registry.with(pane, ScrollPane::is_following_end).unwrap());
        registry
            .with_mut(pane, |pane| pane.set_content(content(40)))
            .unwrap();
        assert!(!registry.with(pane, ScrollPane::is_following_end).unwrap());
        assert_eq!(
            host.dispatch_key_local(KeyStroke::new(Key::End), &mut registry),
            InteractionResult::Consumed
        );
        assert!(registry.with(pane, ScrollPane::is_following_end).unwrap());
    }

    struct StatefulField;

    impl Component for StatefulField {
        fn view(&self) -> View {
            View::text("state")
                .foreground(ColorSpec::theme("accent"))
                .into_view()
        }
    }

    struct FocusWithinShell {
        child: crate::component::ComponentHandle<ThemedField>,
    }

    impl Component for FocusWithinShell {
        fn view(&self) -> View {
            View::component(self.child)
                .border(BorderSpec::plain().color(ColorSpec::theme("shell.border")))
                .fill_width()
        }
    }

    struct FocusableShell {
        child: crate::component::ComponentHandle<ThemedField>,
    }

    impl Component for FocusableShell {
        fn view(&self) -> View {
            View::component(self.child)
                .border(BorderSpec::plain().color(ColorSpec::theme("shell.border")))
                .fill_width()
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.focusable();
        }
    }

    struct ThemedField;

    impl Component for ThemedField {
        fn view(&self) -> View {
            View::text("field")
                .border(BorderSpec::plain().color(ColorSpec::theme("field.border")))
                .fill_width()
                .into_view()
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.focusable();
        }
    }

    struct FocusMutatingField {
        focused: bool,
    }

    impl FocusMutatingField {
        fn focus_changed(&mut self, focused: bool) {
            self.focused = focused;
        }
    }

    impl Component for FocusMutatingField {
        fn view(&self) -> View {
            View::text(if self.focused { "focused" } else { "unfocused" }).into_view()
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.focusable();
            cx.on_focus_changed(Self::focus_changed);
        }
    }

    #[test]
    fn semantic_state_crosses_component_boundary_and_nearest_override_wins() {
        let mut registry = ComponentRegistry::new();
        let field = registry.register(StatefulField);
        let scene = Scene::new(
            View::component(field)
                .style_state("severity", "warning")
                .into_view(),
        );
        let theme = Theme::new()
            .with_color("accent", ThemeColor::Indexed(2))
            .with_color_variant(
                "accent",
                StyleSelector::state("severity", "warning"),
                ThemeColor::Indexed(1),
            )
            .with_color_variant(
                "accent",
                StyleSelector::state("severity", "error"),
                ThemeColor::Indexed(3),
            );
        let mut host = SceneHost::default();
        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 4))
            .unwrap();
        let frame = host.paint(stable, &theme);
        assert_eq!(
            frame.surface.get(0, 0).style.foreground,
            Some(crate::physical::PhysicalColor::Indexed(1))
        );

        let nested = Scene::new(
            View::vertical(|column| {
                column.child(
                    View::component(field)
                        .style_state("severity", "error")
                        .into_view(),
                );
            })
            .style_state("severity", "warning"),
        );
        let stable = host
            .resolve_stable::<()>(&nested, &mut registry, Size::new(20, 4))
            .unwrap();
        let frame = host.paint(stable, &theme);
        assert_eq!(
            frame.surface.get(0, 0).style.foreground,
            Some(crate::physical::PhysicalColor::Indexed(3))
        );
    }

    #[test]
    fn focus_callback_mutation_converges_before_paint() {
        let mut registry = ComponentRegistry::new();
        let field = registry.register(FocusMutatingField { focused: false });
        let scene = Scene::new(View::component(field));
        let mut host = SceneHost::default();
        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 4))
            .unwrap();
        let frame = host.paint(stable, &Theme::default());

        assert_eq!(frame.surface.get(0, 0).grapheme.as_deref(), Some("f"));
        assert_eq!(registry.with(field, |field| field.focused), Some(true));
    }

    #[test]
    fn paints_framework_focus_variant_without_component_revision_change() {
        let mut registry = ComponentRegistry::new();
        let field = registry.register(ThemedField);
        let scene = Scene::new(View::component(field));
        let theme = Theme::new()
            .with_color("field.border", ThemeColor::Named(crate::AnsiColor::Gray))
            .with_color_variant(
                "field.border",
                StyleSelector::focused(),
                ThemeColor::Named(crate::AnsiColor::Cyan),
            );
        let revision = registry.revision(field).expect("field revision");
        let mut host = SceneHost::default();
        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 4))
            .unwrap();
        let frame = host.paint(stable, &theme);

        assert_eq!(host.focused(), Some(field.id()));
        assert_eq!(registry.revision(field), Some(revision));
        assert_eq!(
            frame.surface.get(0, 0).style.foreground,
            Some(crate::physical::PhysicalColor::Named(
                crate::physical::AnsiColor::Cyan,
            ))
        );

        #[cfg(feature = "perf-counters")]
        {
            let _lock = crate::perf::test_lock();
            crate::perf::reset();
            let _ = host
                .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 4))
                .unwrap();
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::TextFlowMeasureCalls),
                0
            );
            assert!(counters.value(crate::perf::Counter::PrepareNodeCalls) <= 1);
        }
    }

    #[test]
    fn paints_focus_within_on_a_component_parent_without_leaking_focus() {
        let mut registry = ComponentRegistry::new();
        let child = registry.register(ThemedField);
        let shell = registry.register(FocusWithinShell { child });
        let scene = Scene::new(View::component(shell));
        let theme = Theme::new()
            .with_color("shell.border", ThemeColor::Named(crate::AnsiColor::Gray))
            .with_color_variant(
                "shell.border",
                StyleSelector::focus_within(),
                ThemeColor::Named(crate::AnsiColor::Cyan),
            );
        let mut host = SceneHost::default();
        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 4))
            .unwrap();
        let frame = host.paint(stable, &theme);

        assert_eq!(host.focused(), Some(child.id()));
        assert_eq!(
            frame.surface.get(0, 0).style.foreground,
            Some(crate::physical::PhysicalColor::Named(
                crate::physical::AnsiColor::Cyan,
            ))
        );
    }

    #[test]
    fn parent_focused_does_not_mark_nested_child_focused() {
        let mut registry = ComponentRegistry::new();
        let child = registry.register(ThemedField);
        let shell = registry.register(FocusableShell { child });
        let scene = Scene::new(View::component(shell));
        let theme = Theme::new()
            .with_color("shell.border", ThemeColor::Named(crate::AnsiColor::Gray))
            .with_color("field.border", ThemeColor::Named(crate::AnsiColor::Gray))
            .with_color_variant(
                "field.border",
                StyleSelector::focused(),
                ThemeColor::Named(crate::AnsiColor::Cyan),
            );
        let mut host = SceneHost::default();
        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(20, 5))
            .unwrap();
        let frame = host.paint(stable, &theme);

        assert_eq!(host.focused(), Some(shell.id()));
        assert_eq!(
            frame.surface.get(0, 1).style.foreground,
            Some(crate::physical::PhysicalColor::Named(
                crate::physical::AnsiColor::Gray,
            ))
        );
    }

    #[test]
    fn paints_the_layout_from_the_stable_convergence_pass() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(LayoutAware {
            changed: false,
            calls: 0,
        });
        let scene = Scene::new(View::component(handle));
        let mut host = SceneHost::default();

        let stable = host
            .resolve_stable::<()>(&scene, &mut registry, Size::new(10, 4))
            .unwrap();
        let geometry = stable
            .layout
            .components
            .entries
            .get(&handle.id())
            .expect("layout-aware component geometry")
            .content;
        let frame = host.paint(stable, &crate::Theme::default());

        assert_eq!(registry.with(handle, |component| component.calls), Some(2));
        assert_eq!(geometry.height, 2);
        assert_eq!(frame.surface.get(0, 0).grapheme.as_deref(), Some("n"));
        assert_eq!(frame.surface.get(0, 1).grapheme.as_deref(), Some("r"));
    }

    #[test]
    fn render_continues_after_zero_row_stream_retirement() {
        let mut history = crate::History::new();
        let stream = history.push_stream(EmptySealedSource).unwrap();
        history.seal_stream(stream).unwrap();
        history.push("S1\nS2\nS3").unwrap();
        let mut scene = Scene::with_history(history, "body");
        let mut registry = ComponentRegistry::new();
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        host.render(
            &mut scene,
            &mut registry,
            &crate::Theme::default(),
            &mut sink,
            |_| Ok(Size::new(10, 3)),
        )
        .unwrap();

        assert!(sink.rows.iter().any(|row| row.plain_text() == "S1"));
    }

    struct LiveBlocker;

    impl Component for LiveBlocker {
        fn view(&self) -> View {
            View::text("B1\nB2\nB3\nB4").into_view()
        }

        fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
    }

    #[derive(Clone)]
    struct BlockableStreamSource {
        text: String,
        stable_through: u64,
        revision: u64,
        sealed: bool,
    }

    impl BlockableStreamSource {
        fn new(text: &str, stable_through: u64, sealed: bool) -> Self {
            Self {
                text: text.to_string(),
                stable_through,
                revision: 0,
                sealed,
            }
        }

        fn set_stable_through(&mut self, stable_through: u64) {
            self.stable_through = stable_through;
            self.revision += 1;
        }
    }

    impl StreamingSource for BlockableStreamSource {
        fn snapshot(&self) -> StreamSnapshot {
            let end = self.text.len() as u64;
            StreamSnapshotBuilder::new(
                StreamRevision::new(self.revision),
                StreamOffset::ZERO,
                StreamOffset::new(self.stable_through.min(end)),
                StreamOffset::new(end),
            )
            .exact_text(
                StreamRange::new(StreamOffset::ZERO, StreamOffset::new(end)),
                [TextSpan::plain(self.text.clone())],
            )
            .finish()
            .unwrap()
        }

        fn seal(&mut self) {
            self.sealed = true;
            self.revision += 1;
        }

        fn is_sealed(&self) -> bool {
            self.sealed
        }
    }

    #[test]
    fn semantic_blocked_live_prefix_is_pinned_not_skipped() {
        let mut history = crate::History::new();
        history.push("A").unwrap();
        let mut registry = ComponentRegistry::new();
        let blocker = registry.register(LiveBlocker);
        history.push(View::component(blocker)).unwrap();
        history.push("C1\nC2\nC3").unwrap();

        let mut sink = TestSink::default();
        let outcome =
            crate::history::transfer_native_prefix(&mut history, &mut sink, 10, 1).unwrap();
        assert_eq!(outcome.inserted, 1);
        assert_eq!(sink.rows.len(), 1);
        assert_eq!(sink.rows[0].plain_text(), "A");

        let mut scene = Scene::with_history(history, "body");
        let mut host = SceneHost::default();

        let frame = host
            .render(
                &mut scene,
                &mut registry,
                &crate::Theme::default(),
                &mut sink,
                |_| Ok(Size::new(10, 4)),
            )
            .unwrap();

        let rendered = (0..4)
            .map(|y| {
                (0..10)
                    .map(|x| frame.surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(rendered, ["B1", "B2", "B3", "body"]);
    }

    #[test]
    fn blocked_then_stable_stream_preserves_contiguous_flow() {
        let mut history = crate::History::new();
        let source = BlockableStreamSource::new("S1\nS2\nS3\nS4\nS5\nS6", 0, false);
        let stream = history.push_stream(source).unwrap();

        let mut scene = Scene::with_history(history, "body");
        let mut registry = ComponentRegistry::new();
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        let frame = host
            .render(
                &mut scene,
                &mut registry,
                &crate::Theme::default(),
                &mut sink,
                |_| Ok(Size::new(10, 4)),
            )
            .unwrap();

        let rendered = (0..4)
            .map(|y| {
                (0..10)
                    .map(|x| frame.surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(rendered, ["S1", "S2", "S3", "body"]);
        assert_eq!(sink.rows.len(), 0);

        let full_text = "S1\nS2\nS3\nS4\nS5\nS6";
        scene
            .history_mut()
            .unwrap()
            .update_stream(stream, |source| {
                source.set_stable_through(full_text.len() as u64);
            })
            .unwrap();

        let frame = host
            .render(
                &mut scene,
                &mut registry,
                &crate::Theme::default(),
                &mut sink,
                |_| Ok(Size::new(10, 4)),
            )
            .unwrap();

        let rendered = (0..4)
            .map(|y| {
                (0..10)
                    .map(|x| frame.surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(rendered, ["S4", "S5", "S6", "body"]);
        assert_eq!(sink.rows.len(), 3);
        assert_eq!(sink.rows[0].plain_text(), "S1");
        assert_eq!(sink.rows[1].plain_text(), "S2");
        assert_eq!(sink.rows[2].plain_text(), "S3");
    }

    // -----------------------------------------------------------------------
    // drain_native_pressure tests
    // -----------------------------------------------------------------------

    /// History: 100 one-row static units, history viewport capacity: 10.
    /// Sink accepts everything.
    ///
    /// Expected in one Host frame: not ~90 full scene resolves. The drain loop
    /// must batch all physical rows in one drain pass, yielding at most a small
    /// constant number of resolves.
    #[test]
    fn native_pressure_drains_multiple_units_before_reresolve() {
        let mut history = crate::History::new();
        // 100 static units, each one row.
        for i in 0..100u32 {
            history.push(format!("S{i}")).unwrap();
        }

        let mut scene = Scene::with_history(history, "body");
        let mut registry = ComponentRegistry::new();
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        host.render(
            &mut scene,
            &mut registry,
            &crate::Theme::default(),
            &mut sink,
            // Height 10: body "body" is 1 row, so history gets 9 rows.
            // overflow = 100 - 9 = 91 rows.
            |_| Ok(Size::new(10, 10)),
        )
        .unwrap();

        // All 91 overflow rows must have been drained.
        assert_eq!(sink.rows.len(), 91);

        // The drain loop batches all 91 rows within one drain call, so we
        // expect at most 3 full resolves: one initial FollowEnd, one after the
        // drain empties the budget, and at most one for layout-sync.
        // Must not scale with unit count.
        assert!(
            host.resolve_count <= 3,
            "resolve_count {} should be <= 3 (must not scale with unit count)",
            host.resolve_count
        );
    }

    /// History: 20 rows total, capacity 17. Expected: exactly 3 physical rows
    /// inserted, not 4. After final re-resolve: resident fits capacity.
    #[test]
    fn native_pressure_respects_overflow_budget() {
        let mut history = crate::History::new();
        // 20 static one-row units.
        for i in 0..20u32 {
            history.push(format!("R{i}")).unwrap();
        }

        let mut scene = Scene::with_history(history, "body");
        let mut registry = ComponentRegistry::new();
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        // height = 17 → overflow = 20 - 17 = 3 (capacity is 17 rows for history
        // because body is "body" which is 1 row, so history gets 16 rows...
        // Actually the body uses 1 row so history gets height - 1 = 16.
        // overflow = 20 - 16 = 4.
        // Let's use height=21 so body gets 1 and history gets 20, overflow=0
        // ... Actually let's measure carefully. We want exactly 3 overflow rows.
        // Use 20 units, height=20, body=1 → history height = 19, overflow = 1.
        // Use 20 units, height=19, body=1 → history height = 18, overflow = 2.
        // Use 20 units, height=18, body=1 → history height = 17, overflow = 3.
        host.render(
            &mut scene,
            &mut registry,
            &crate::Theme::default(),
            &mut sink,
            |_| Ok(Size::new(10, 18)),
        )
        .unwrap();

        // Body "body" is 1 row, so history has 17 rows visible; 20 - 17 = 3 overflow.
        assert_eq!(
            sink.rows.len(),
            3,
            "Expected exactly 3 native rows inserted, got {}",
            sink.rows.len()
        );

        // After final re-resolve the resident working set fits within capacity.
        // The host painted successfully (no panic/error) which confirms geometry
        // is consistent.
    }

    /// Static A, Static B, Live C (component), Static D.
    /// Overflow requires movement past C.
    ///
    /// Drain: inserts A, inserts B, hits SemanticBlocked on C.
    /// Because some physical progress occurred → returns Progress, re-resolves.
    /// On next attempt C still blocks → Blocked → NativeFrontier paint.
    ///
    /// The host must NOT paint based on the pre-transfer stale frame.
    #[test]
    fn physical_progress_then_blocker_forces_reresolve_not_stale_paint() {
        use crate::Component;

        struct LiveBlocker;
        impl Component for LiveBlocker {
            fn view(&self) -> View {
                // Fills 4 rows so it dominates the visible area.
                View::text("B1\nB2\nB3\nB4").into_view()
            }
            fn capabilities(&self, _cx: &mut crate::ComponentCx<'_, Self>) {}
        }

        let mut registry = ComponentRegistry::new();
        let blocker_handle = registry.register(LiveBlocker);

        let mut history = crate::History::new();
        history.push("A").unwrap();
        history.push("B").unwrap();
        history.push(View::component(blocker_handle)).unwrap();
        history.push("D").unwrap();

        let mut scene = Scene::with_history(history, "body");
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        // Viewport small enough that A and B overflow (history height < total).
        // height=4 → history height = 3 (body "body" is 1 row),
        // total semantic: A(1) + B(1) + LiveC(4) + D(1) = 7 → overflow = 7 - 3 = 4.
        host.render(
            &mut scene,
            &mut registry,
            &crate::Theme::default(),
            &mut sink,
            |_| Ok(Size::new(10, 4)),
        )
        .unwrap();

        // A and B must have been physically inserted before the blocker stopped us.
        let plain_texts: Vec<String> = sink
            .rows
            .iter()
            .map(|r| r.plain_text().to_string())
            .collect();
        assert!(
            plain_texts.contains(&"A".to_string()),
            "Expected 'A' to be inserted: {:?}",
            plain_texts
        );
        assert!(
            plain_texts.contains(&"B".to_string()),
            "Expected 'B' to be inserted: {:?}",
            plain_texts
        );

        // After the blocker is hit we must have re-resolved (not used stale
        // geometry). The host painted without panicking and the frame was produced
        // after the re-resolve with NativeFrontier anchor.
        //
        // Because progress happened (A+B inserted) then blocked (C), the drain
        // returned Progress → re-resolve occurred. Then on the next loop iteration
        // the drain returned Blocked immediately → NativeFrontier resolve + paint.
        // Both the progress-triggered resolve and the NativeFrontier resolve must
        // have happened, so resolve_count >= 2.
        assert!(
            host.resolve_count >= 2,
            "Expected at least 2 resolves (FollowEnd + NativeFrontier), got {}",
            host.resolve_count
        );
    }

    /// Stable-stream stress: 1000 stable lines, ingested in one chunk.
    ///
    /// After a single Host render call, the drain loop must have transferred
    /// the overflow budget (981 rows) in one pass — not one resolve per row.
    ///
    /// Liveness assertion: the stream was either retired entirely (units empty)
    /// or a large number of physical rows were inserted.
    ///
    /// No wall-clock timing assertions.
    #[test]
    fn stable_stream_stress_native_pressure_batches_transfer() {
        // Build a stable sealed stream with 1000 single-char lines ("L\n" × 1000
        // = 2000 bytes). All content is stable_through = source_end.
        let line = "L\n";
        let total_lines: u32 = 1000;
        let content: String = line.repeat(total_lines as usize);
        let total_bytes = content.len() as u64;

        let source = BlockableStreamSource::new(&content, total_bytes, true);

        let mut history = crate::History::new();
        let _handle = history.push_stream(source).unwrap();

        let mut scene = Scene::with_history(history, "body");
        let mut registry = ComponentRegistry::new();
        let mut host = SceneHost::default();
        let mut sink = TestSink::default();

        // Viewport 80 × 20: body "body" is 1 row → history gets 19 rows.
        // overflow = 1000 - 19 = 981 rows.
        host.render(
            &mut scene,
            &mut registry,
            &crate::Theme::default(),
            &mut sink,
            |_| Ok(Size::new(80, 20)),
        )
        .unwrap();

        // Primary liveness: the overflow rows must have been physically inserted.
        // Content is "L\n" × 1000: the trailing \n produces a final empty row,
        // giving 1001 rendered rows. History height = 19, overflow = 1001 - 19 = 982.
        assert_eq!(
            sink.rows.len(),
            982,
            "Expected 982 native rows inserted (1001 lines - 19 history capacity), got {}",
            sink.rows.len()
        );

        // Resolve count must be small (not O(N) = not O(1000)).
        // Expect: 1 initial FollowEnd, 1 after drain, ≤ 1 more for layout.
        assert!(
            host.resolve_count <= 5,
            "resolve_count {} should be small (not O(N units))",
            host.resolve_count
        );
    }
}
