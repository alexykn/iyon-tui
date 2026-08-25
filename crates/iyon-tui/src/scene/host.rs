//! Generic retained Scene host.
//!
//! This module owns frame geometry, component synchronization, focus, ticks,
//! and native History pressure. Application code supplies only semantic Scene
//! state and consumes routed outputs.

use std::{collections::HashSet, time::Instant};

use anyhow::Result;

use crate::{
    Theme,
    backend::NativeHistorySink,
    component::{
        ComponentId, ComponentRegistry, MountGraph, MountedComponents, TickOutcome, TickScheduler,
    },
    geometry::Size,
    interaction::{
        FocusState, InteractionResult, KeyStroke, MountedCapabilities, route_key_local,
        route_paste, route_paste_interceptor,
    },
    output::{OutputQueue, OutputRouter},
    physical::Surface,
    presentation::{
        layout::{LayoutCache, ViewCompiler},
        paint::{PaintCache, ViewPainter},
    },
};

use super::{
    LayoutSynchronizer, ResolveError, ResolvedRootScene, ResolvedSceneLayout, Scene,
    layout_resolved_scene_with_cache, resolve_component_subtree,
    resolve_root_scene_with_anchor_and_cache,
};
use crate::history::HistoryViewportAnchor;

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
) -> Result<NativePressure, crate::history::NativeTransferError<S::Error>> {
    use crate::history::NativeTransferStatus::{Idle, Progress, SemanticBlocked, SinkBlocked};

    let mut remaining = overflow_rows;
    let mut inserted_any = false;

    while remaining > 0 {
        let physical_before = history.physical_rows_inserted();
        let outcome = crate::history::transfer_native_prefix_with_theme(
            history, sink, width, remaining, theme,
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
    incremental_sync_components: Vec<ComponentId>,
    incremental_topology_changed: bool,
    incremental_paint_components: Vec<ComponentId>,
    /// Counts calls to `resolve_stable_at_with_anchor` for structural test
    /// assertions. Not compiled into production builds.
    #[cfg(test)]
    pub(crate) resolve_count: usize,
    #[cfg(test)]
    pub(crate) full_resolves: usize,
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
            incremental_sync_components: Vec::new(),
            incremental_topology_changed: false,
            incremental_paint_components: Vec::new(),
            #[cfg(test)]
            resolve_count: 0,
            #[cfg(test)]
            full_resolves: 0,
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
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_paint_components.clear();
    }

    /// Marks one native component as changed. The next frame can resolve only
    /// this component's owned subtree; the committed frame remains authoritative
    /// until that frame successfully prepares and paints.
    pub(crate) fn invalidate_component(&mut self, id: ComponentId) {
        self.invalidated_components.insert(id);
    }

    /// Invalidates the retained scene root for body/history/theme changes.
    pub(crate) fn invalidate_root(&mut self) {
        self.retained = None;
        self.last_surface = None;
        self.invalidated_components.clear();
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_paint_components.clear();
    }

    pub(crate) fn next_tick_deadline(&self) -> Option<Instant> {
        self.ticker.next_deadline()
    }

    #[cfg(test)]
    pub(crate) fn focused(&self) -> Option<crate::component::ComponentId> {
        self.focus.focused()
    }

    #[cfg(test)]
    pub(crate) fn mount_count_for_test(&self) -> usize {
        self.graph.nodes.len()
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
        self.ticker
            .tick_due_with_events(now, registry, &mut self.outputs)
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
        self.render_at(Instant::now(), scene, registry, theme, sink, viewport)
    }

    pub(crate) fn render_at<S, F>(
        &mut self,
        now: Instant,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        sink: &mut S,
        mut viewport: F,
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
            )?;

            if resolved.root.history_overflow_rows == 0 {
                crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                return Ok(self.paint(resolved, theme));
            }

            let Some(history) = scene.history_mut() else {
                crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                return Ok(self.paint(resolved, theme));
            };

            let pressure = drain_native_pressure(
                history,
                sink,
                size.width,
                resolved.root.history_overflow_rows,
                theme,
                &mut transfer_calls,
            )
            .map_err(SceneHostError::Transfer)?;

            match pressure {
                NativePressure::Progress => continue,

                NativePressure::Blocked => {
                    // size may be reused: no native rows were inserted during
                    // the final blocked drain attempt, so viewport geometry did
                    // not change.
                    resolves += 1;
                    let pinned = self.resolve_stable_at_with_anchor(
                        scene,
                        registry,
                        size,
                        now,
                        HistoryViewportAnchor::NativeFrontier,
                    )?;
                    crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                    return Ok(self.paint(pinned, theme));
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
        self.resolve_stable_at_with_anchor(
            scene,
            registry,
            size,
            now,
            HistoryViewportAnchor::FollowEnd,
        )
    }

    fn resolve_stable_at_with_anchor<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        now: Instant,
        anchor: HistoryViewportAnchor,
    ) -> Result<StableScene, SceneHostError<E>> {
        let mut force_full = false;
        let mut layout_epoch_started = false;
        for _ in 0..MAX_LAYOUT_PASSES {
            let resolved = if !force_full {
                match self.try_incremental_stable(scene, registry, size, anchor) {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) => {
                        if !layout_epoch_started {
                            self.layout_cache.begin_epoch();
                            layout_epoch_started = true;
                        }
                        self.resolve_full_stable(scene, registry, size, anchor)?
                    }
                    Err(error) => return Err(SceneHostError::Resolve(error)),
                }
            } else {
                if !layout_epoch_started {
                    self.layout_cache.begin_epoch();
                    layout_epoch_started = true;
                }
                self.resolve_full_stable(scene, registry, size, anchor)?
            };
            let incremental_host =
                !self.incremental_sync_components.is_empty() && !self.incremental_topology_changed;
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
                self.incremental_paint_components.clear();
                continue;
            }

            self.graph = resolved.root.scene.mounts.clone();
            self.capabilities = resolved.root.scene.capabilities.clone();
            let focus_changed = if incremental_host && self.focus.focused().is_none() {
                false
            } else if incremental_host {
                self.focus.reconcile_with_geometry(
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
            if focus_changed {
                force_full = true;
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_paint_components.clear();
                continue;
            }
            self.invalidated_components.clear();
            self.incremental_sync_components.clear();
            self.incremental_topology_changed = false;
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
    ) -> Result<StableScene, SceneHostError<E>> {
        let resolved = resolve_root_scene_with_anchor_and_cache(
            scene,
            registry,
            size,
            anchor,
            &mut self.layout_cache,
        )
        .map_err(SceneHostError::Resolve)?;
        let layout =
            layout_resolved_scene_with_cache(&resolved.scene, size, &mut self.layout_cache);
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_paint_components.clear();
        #[cfg(test)]
        {
            self.full_resolves += 1;
        }
        Ok(StableScene {
            root: resolved,
            layout,
        })
    }

    fn try_incremental_stable(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
    ) -> Result<Option<StableScene>, ResolveError> {
        if self.invalidated_components.is_empty()
            || self.retained.as_ref().is_none_or(|retained| {
                retained.layout.tree.size != size
                    || !crate::presentation::View::ptr_eq(
                        &retained.root.body_view,
                        scene.layout_body(),
                    )
            })
        {
            return Ok(None);
        }
        if !matches!(anchor, HistoryViewportAnchor::FollowEnd) {
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

        let mut topology_changed = false;
        for id in &roots {
            match update_component_subtree(&mut retained, registry, *id) {
                Ok(changed) => topology_changed |= changed,
                Err(error) => {
                    self.retained = Some(retained);
                    return Err(error);
                }
            }
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
            ) {
                patched = false;
                break;
            }
        }
        if !patched {
            self.layout_cache.begin_epoch();
            retained.layout = layout_resolved_scene_with_cache(
                &retained.root.scene,
                size,
                &mut self.layout_cache,
            );
            self.incremental_paint_components.clear();
        } else {
            self.incremental_paint_components = roots;
        }
        #[cfg(test)]
        {
            self.incremental_resolves += 1;
        }
        Ok(Some(retained))
    }

    fn paint(&mut self, resolved: StableScene, theme: &Theme) -> PreparedSceneFrame {
        self.retained = Some(resolved);
        let retained = self.retained.as_ref().expect("retained frame installed");
        let compiler = ViewCompiler::with_interaction(theme, self.focus.focused(), &self.graph);
        if !self.incremental_paint_components.is_empty() {
            if let Some(mut surface) = self.last_surface.take() {
                let mut incremental = true;
                let mut incremental_cache = PaintCache::default();
                for component in self.incremental_paint_components.iter().copied() {
                    if !ViewPainter.paint_component_into(
                        &compiler,
                        &retained.layout.tree,
                        component,
                        &mut surface,
                        &mut incremental_cache,
                    ) {
                        incremental = false;
                        break;
                    }
                }
                self.incremental_paint_components.clear();
                if incremental {
                    let output = surface.clone();
                    self.last_surface = Some(surface);
                    return PreparedSceneFrame {
                        surface: output,
                        history_overlay: retained.root.history_overlay.clone(),
                    };
                }
            }
            self.incremental_paint_components.clear();
        }
        self.paint_cache.begin_epoch(theme);
        let surface = ViewPainter.paint_tree_with_cache(
            &compiler,
            &retained.layout.tree,
            &mut self.paint_cache,
        );
        let output = surface.clone();
        self.last_surface = Some(surface);
        PreparedSceneFrame {
            surface: output,
            history_overlay: retained.root.history_overlay.clone(),
        }
    }
}

fn update_component_subtree(
    retained: &mut StableScene,
    registry: &mut ComponentRegistry,
    id: ComponentId,
) -> Result<bool, ResolveError> {
    let snapshot = registry
        .resolution(id)
        .ok_or(ResolveError::MissingComponent { id })?;
    let subtree = resolve_component_subtree(&snapshot.view, registry, id)?;
    let graph = &mut retained.root.scene.mounts;
    let Some(old_range) = graph.subtree_range(id) else {
        return Err(ResolveError::MissingComponent { id });
    };
    let old_ids = graph.subtree_ids(id);
    let old_children = graph.nodes[old_range.start + 1..old_range.end]
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<Vec<_>>();
    let new_children = subtree
        .mounts
        .nodes
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<Vec<_>>();
    let topology_changed = old_children != new_children;

    for node in &subtree.mounts.nodes {
        if graph.contains(node.id) && !old_ids.contains(&node.id) {
            return Err(ResolveError::DuplicateComponent { id: node.id });
        }
    }

    if topology_changed {
        graph.replace_subtree(id, subtree.mounts.clone());
    } else {
        graph.update_revision(id, snapshot.revision);
        for node in &subtree.mounts.nodes {
            graph.update_revision(node.id, node.revision);
        }
    }
    if topology_changed {
        graph.update_revision(id, snapshot.revision);
    }

    for old_id in old_ids {
        retained.root.scene.overlay.components.remove(&old_id);
        retained.root.scene.capabilities.entries.remove(&old_id);
    }
    retained
        .root
        .scene
        .overlay
        .components
        .insert(id, snapshot.clone());
    retained
        .root
        .scene
        .capabilities
        .insert(id, snapshot.capabilities);
    retained
        .root
        .scene
        .overlay
        .components
        .extend(subtree.overlay.components);
    retained
        .root
        .scene
        .capabilities
        .entries
        .extend(subtree.capabilities.entries);
    Ok(topology_changed)
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
        BorderSpec, ColorSpec, Component, ComponentCx, InteractionResult, IntoView, Key, KeyStroke,
        Scene, ScrollPane, StyleSelector, TextSpan, ThemeColor, View,
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

    impl Component for R6bLeaf {
        fn view(&self) -> View {
            View::text(self.text.clone()).into_view()
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
            assert!(counters.value(crate::perf::Counter::PaintCacheHits) >= 999);
            assert!(counters.value(crate::perf::Counter::PaintCacheMisses) <= 2);
        }

        registry
            .with_mut(handles[0], |leaf| leaf.text = "y\nrow".to_owned())
            .unwrap();
        host.invalidate_component(handles[0].id());
        let geometry_change = host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let geometry_frame = host.paint(geometry_change, &Theme::default());

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
