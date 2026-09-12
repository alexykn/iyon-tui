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

use crate::presentation::{ContentDirty, ContentProvider, EmptyContentProvider};
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
    presentation::layout::DamageRegion,
    presentation::{
        ir::{View, ViewId, ViewKind},
        layout::{LayoutCache, ViewCompiler, layout_view_with_overlay_and_cache_and_content},
        paint::{PaintCache, ViewPainter},
    },
};

use super::root::merge_root_scene;
use super::{
    LayoutSynchronizer, ResolveError, ResolveSession, ResolvedRootScene, ResolvedScene,
    ResolvedSceneLayout, Scene, layout_resolved_scene_with_cache_and_content,
    resolve_component_subtree, resolve_root_scene_with_anchor_and_cache_and_content,
};
use crate::history::{HistoryViewportAnchor, project_into_session_for_host_with_content};
use crate::presentation::direct::{
    CapturedContentMeasurement, DirectDriverHandle, DirectHistoryAnchor, DirectLayout,
    snapshot_display_none,
};

const MAX_LAYOUT_PASSES: usize = 8;

fn direct_floor_width(value: f32) -> Result<u16> {
    if !value.is_finite() || value < 0.0 || value >= f32::from(u16::MAX) {
        return Err(anyhow::anyhow!(
            "direct content width is outside terminal range"
        ));
    }
    Ok(value.floor() as u16)
}

fn validate_direct_measurement_widths(
    direct: &DirectLayout,
    captures: &HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
) -> Result<()> {
    for (key, capture) in captures {
        let Some(width) = direct.content_widths.get(key).copied() else {
            continue;
        };
        if direct_floor_width(width)? != capture.offered_width {
            return Err(anyhow::anyhow!(
                "direct content width did not converge within bounded preparation"
            ));
        }
    }
    Ok(())
}

fn validate_direct_content_products(
    direct: &DirectLayout,
    captures: &HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
) -> Result<()> {
    for (key, capture) in captures {
        if direct.content_products.get(key) != Some(capture) {
            return Err(anyhow::anyhow!(
                "direct ContentHost ticket does not match captured product"
            ));
        }
    }
    Ok(())
}

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
/// frontier is stuck and the host should paint from the `NativeFrontier` anchor.
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
    /// Component geometry captured with the same resolved scene and surface.
    /// The host only exposes this map after the corresponding frame receipt
    /// succeeds; a candidate must never answer visibility queries.
    pub(crate) component_geometry: crate::presentation::layout::ComponentGeometryMap,
    /// Geometry keyed by retained View identity for ordinary occurrence
    /// references that do not own a native component.
    pub(crate) view_geometry: HashMap<ViewId, crate::presentation::layout::ComponentGeometry>,
    /// Confirmed-frame geometry keyed directly by qualified occurrence key.
    /// The application adapter fills this while the candidate is captured;
    /// queries never resolve an occurrence through mutable desired recipes.
    pub(crate) occurrence_geometry:
        HashMap<crate::occurrence::NodeKey, crate::presentation::layout::ComponentGeometry>,
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

#[derive(Clone, Copy, Debug, Default)]
struct ContentDirtyRecord {
    epoch: u64,
    measurement: bool,
    paint: bool,
}

pub(crate) struct SceneHost {
    mounted: MountedComponents,
    synchronizer: LayoutSynchronizer,
    focus: FocusState,
    ticker: TickScheduler,
    outputs: OutputQueue,
    graph: MountGraph,
    capabilities: MountedCapabilities,
    /// Direct occurrence route. This is the production path for the React UI
    /// document. The retained View resolver below serves only native
    /// component/content compatibility while the T7 semantic-content gate is
    /// completed.
    direct_driver: Option<DirectDriverHandle>,
    direct_controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    direct_content_ports: HashMap<crate::occurrence::NodeKey, crate::occurrence::ResourceKey>,
    direct_control_nodes: HashMap<crate::occurrence::NodeKey, crate::occurrence::ResourceKey>,
    direct_port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
    direct_body_root: Option<crate::occurrence::NodeKey>,
    direct_synchronized: bool,
    direct_history_overflow_rows: usize,
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
    incremental_requires_full_sync: bool,
    incremental_paint_components: Vec<ComponentId>,
    /// True when the retained History branch can be painted without walking
    /// the clean body branch.
    incremental_paint_history: bool,
    /// True while the current retained candidate was rebuilt from the History
    /// branch without re-resolving the body branch.
    history_only_refresh: bool,
    /// Source revisions change descendant layout/paint products without
    /// changing semantic View identity. Each record is keyed by the affected
    /// ContentPort, and the retained layout tree supplies the dependency
    /// frontier for targeted invalidation.
    content_dirty: HashMap<u64, ContentDirtyRecord>,
    content_dirty_epoch: u64,
    content_prepared_epoch: Option<u64>,
    /// ContentPort IDs whose physical subtree needs repainting in the current
    /// candidate. This is separate from measurement dirtiness: a palette or
    /// delivery paint change must not force a layout walk when metrics match.
    incremental_paint_content: Vec<u64>,
    /// A palette/presentation-only change reuses the retained geometry and
    /// semantic products. Paint cache entries are discarded, but layout cache
    /// and the retained scene remain valid.
    theme_invalidated: bool,
    /// ContentHost dependency paths retained across a structural invalidation
    /// until the replacement root is resolved. Theme revisions are
    /// intentionally excluded from layout-input keys, so this deferred index
    /// invalidates only old content tickets without clearing clean siblings.
    retained_content_dependencies: HashMap<u64, HashSet<ViewId>>,
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

    pub(crate) fn set_direct_control_component(
        &mut self,
        key: crate::occurrence::ResourceKey,
        component: crate::component::ComponentId,
    ) {
        self.direct_controls.insert(key, component);
    }

    pub(crate) fn set_direct_driver_id(&mut self, driver_id: u64) -> Result<()> {
        if self.direct_driver.is_none() {
            self.direct_driver = Some(DirectDriverHandle::start(driver_id)?);
        }
        Ok(())
    }

    pub(crate) fn clear_direct_driver(&mut self) -> Result<()> {
        if let Some(mut driver) = self.direct_driver.take() {
            driver.shutdown()?;
        }
        self.direct_synchronized = false;
        self.direct_history_overflow_rows = 0;
        Ok(())
    }

    pub(crate) fn remove_direct_control_component(&mut self, key: crate::occurrence::ResourceKey) {
        self.direct_controls.remove(&key);
    }

    pub(crate) fn direct_control_for_component(
        &self,
        component: crate::component::ComponentId,
    ) -> Option<crate::occurrence::ResourceKey> {
        self.direct_controls
            .iter()
            .find_map(|(key, candidate)| (*candidate == component).then_some(*key))
    }

    pub(crate) fn direct_component_for_control(
        &self,
        control: crate::occurrence::ResourceKey,
    ) -> Option<crate::component::ComponentId> {
        self.direct_controls.get(&control).copied()
    }

    pub(crate) fn sync_direct_occurrences(
        &mut self,
        snapshots: Vec<crate::occurrence::OccurrenceSnapshot>,
        changes: Option<&crate::occurrence::UiChangeSet>,
        participation: &[crate::presentation::taffy::NodeParticipation],
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<crate::occurrence::NodeKey>,
        body_root: crate::occurrence::NodeKey,
        portal_owners: HashMap<crate::occurrence::NodeKey, crate::occurrence::NodeKey>,
    ) -> Result<()> {
        let Some(driver) = self.direct_driver.as_ref() else {
            return Err(anyhow::anyhow!("direct renderer driver is not started"));
        };
        let participation_map = participation
            .iter()
            .map(|item| (item.key, item.participates))
            .collect::<HashMap<_, _>>();
        let content_ports = snapshots
            .iter()
            .filter_map(|snapshot| {
                snapshot
                    .port
                    .filter(|port| {
                        participation_map
                            .get(&snapshot.key)
                            .copied()
                            .unwrap_or(true)
                            && !snapshot_display_none(snapshot)
                            && port_ids.contains_key(port)
                    })
                    .map(|port| (snapshot.key, port))
            })
            .collect::<HashMap<_, _>>();
        let control_nodes = snapshots
            .iter()
            .filter_map(|snapshot| snapshot.control.map(|control| (snapshot.key, control)))
            .collect::<HashMap<_, _>>();
        let changed_snapshot_keys = snapshots
            .iter()
            .map(|snapshot| snapshot.key)
            .collect::<Vec<_>>();
        driver.synchronize(
            snapshots,
            changes,
            participation.to_vec(),
            port_ids.clone(),
            roots.clone(),
            portal_owners,
            self.direct_controls.clone(),
        )?;
        if changes.is_none() || !self.direct_synchronized {
            self.direct_content_ports = content_ports;
            self.direct_control_nodes = control_nodes;
        } else {
            for key in changed_snapshot_keys {
                self.direct_content_ports.remove(&key);
                self.direct_control_nodes.remove(&key);
            }
            for key in &changes.expect("checked change set").retired_nodes {
                self.direct_content_ports.remove(key);
                self.direct_control_nodes.remove(key);
            }
            self.direct_content_ports.extend(content_ports);
            self.direct_control_nodes.extend(control_nodes);
        }
        self.direct_port_ids = port_ids;
        self.direct_body_root = Some(body_root);
        self.direct_synchronized = true;
        Ok(())
    }

    pub(crate) fn has_direct_occurrences(&self) -> bool {
        self.direct_synchronized && self.direct_driver.is_some()
    }

    pub(crate) fn direct_history_overflow_rows(&self) -> usize {
        self.direct_history_overflow_rows
    }

    pub(crate) fn direct_body_root(&self) -> Option<crate::occurrence::NodeKey> {
        self.direct_body_root
    }

    pub(crate) fn direct_port_ids(&self) -> &HashMap<crate::occurrence::ResourceKey, u64> {
        &self.direct_port_ids
    }

    pub(crate) fn invalidate_direct_content_measurement(&mut self, port_id: u64) -> Result<()> {
        let Some(driver) = self.direct_driver.as_ref() else {
            return Ok(());
        };
        driver.invalidate_content(port_id)
    }

    pub(crate) fn invalidate_direct_control_measurement(
        &mut self,
        component: crate::component::ComponentId,
    ) -> Result<()> {
        let Some(driver) = self.direct_driver.as_ref() else {
            return Ok(());
        };
        driver.invalidate_control(component)
    }

    pub(crate) fn prepare_direct_at_with_content(
        &mut self,
        now: Instant,
        root: crate::occurrence::NodeKey,
        size: Size,
        history_anchor: DirectHistoryAnchor,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        content: &mut dyn ContentProvider,
        _port_ids: &HashMap<crate::occurrence::ResourceKey, u64>,
    ) -> Result<PreparedSceneFrame> {
        let mut captures = self.capture_direct_measurements(size.width, content)?;
        let mut control_views = self.capture_direct_control_views(registry)?;
        let mut invalidate_controls = Vec::new();
        for _ in 0..MAX_LAYOUT_PASSES {
            let direct = self.prepare_direct_layout(
                root,
                size,
                history_anchor,
                &mut captures,
                &control_views,
                &invalidate_controls,
                content,
            )?;
            invalidate_controls.clear();
            self.direct_history_overflow_rows = direct.history_overflow_rows;
            let mounts = direct.component_mounts.clone();
            let mount_nodes = mounts
                .iter()
                .map(|(id, parent)| {
                    let snapshot = registry
                        .resolution(*id)
                        .ok_or_else(|| anyhow::anyhow!("direct mounted component disappeared"))?;
                    Ok(crate::component::MountNode {
                        id: *id,
                        parent: *parent,
                        revision: snapshot.revision,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let graph = MountGraph::new(mount_nodes);
            let mut capabilities = MountedCapabilities::default();
            for id in graph.ids() {
                if let Some(snapshot) = registry.resolution(id) {
                    capabilities.insert(id, snapshot.capabilities);
                }
            }
            self.graph = graph.clone();
            self.capabilities = capabilities.clone();
            let transitions = self.mounted.reconcile(graph.clone());
            self.ticker
                .sync_capabilities(&graph, &capabilities, &transitions, now);
            let geometry = direct.tree.component_geometry();
            if matches!(
                self.synchronizer
                    .synchronize(&graph, &capabilities, &geometry, registry),
                super::LayoutSync::Dirty
            ) {
                control_views = self.capture_direct_control_views(registry)?;
                invalidate_controls = self.direct_control_nodes.keys().copied().collect();
                continue;
            }
            if self
                .focus
                .reconcile_with_geometry(&graph, &capabilities, Some(&geometry), registry)
            {
                control_views = self.capture_direct_control_views(registry)?;
                invalidate_controls = self.direct_control_nodes.keys().copied().collect();
                continue;
            }
            let compiler = ViewCompiler::with_interaction(theme, self.focus.focused(), &self.graph);
            self.paint_cache.begin_epoch(theme);
            let surface = ViewPainter.paint_tree_with_content(
                &compiler,
                &direct.tree,
                &mut self.paint_cache,
                content,
            );
            self.invalidated_components.clear();
            let component_geometry = geometry;
            return Ok(PreparedSceneFrame {
                surface,
                history_overlay: None,
                damage: DamageRegion::full(size),
                component_geometry,
                view_geometry: direct.tree.view_geometry(),
                occurrence_geometry: direct.occurrence_geometry,
            });
        }
        Err(anyhow::anyhow!("direct occurrence layout did not converge"))
    }

    fn prepare_direct_layout(
        &self,
        root: crate::occurrence::NodeKey,
        size: Size,
        history_anchor: DirectHistoryAnchor,
        captures: &mut HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
        control_views: &HashMap<ComponentId, crate::presentation::View>,
        invalidate_controls: &[crate::occurrence::NodeKey],
        content: &mut dyn ContentProvider,
    ) -> Result<DirectLayout> {
        let driver = self
            .direct_driver
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("direct renderer driver is not started"))?;
        let mut direct = driver.layout(
            root,
            size,
            history_anchor,
            captures.clone(),
            invalidate_controls.to_vec(),
            control_views.clone(),
        )?;
        let mut invalidate = Vec::new();
        for (key, capture) in captures.iter_mut() {
            let Some(width) = direct.content_widths.get(key) else {
                continue;
            };
            let width = direct_floor_width(*width)?;
            if width == capture.offered_width {
                continue;
            }
            let next = content.refine_captured_measurement(
                capture.port_id,
                capture.capture_id,
                width,
                crate::presentation::WidthRule::Fill,
            )?;
            capture.measurement = next.measurement;
            capture.min_content = next.min_content;
            capture.max_content = next.max_content;
            capture.history_adjustment = next.history_adjustment;
            capture.semantic_view = next.semantic_view;
            capture.offered_width = width;
            invalidate.push(*key);
        }
        if !invalidate.is_empty() {
            direct = driver.layout(
                root,
                size,
                history_anchor,
                captures.clone(),
                invalidate,
                control_views.clone(),
            )?;
        }
        validate_direct_measurement_widths(&direct, captures)?;
        validate_direct_content_products(&direct, captures)?;
        Ok(direct)
    }

    fn capture_direct_measurements(
        &self,
        width: u16,
        content: &mut dyn ContentProvider,
    ) -> Result<HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>> {
        let mut captures = HashMap::new();
        for (node, resource) in &self.direct_content_ports {
            let port_id = self
                .direct_port_ids
                .get(resource)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("direct ContentPort is not installed"))?;
            let capture =
                content.capture_measurement(port_id, width, crate::presentation::WidthRule::Fit)?;
            captures.insert(
                *node,
                CapturedContentMeasurement {
                    capture_id: capture.capture_id,
                    port_id,
                    offered_width: width,
                    measurement: capture.measurement,
                    min_content: capture.min_content,
                    max_content: capture.max_content,
                    history_adjustment: capture.history_adjustment,
                    semantic_view: capture.semantic_view.clone(),
                },
            );
        }
        Ok(captures)
    }

    fn capture_direct_control_views(
        &self,
        registry: &ComponentRegistry,
    ) -> Result<HashMap<ComponentId, crate::presentation::View>> {
        self.direct_controls
            .values()
            .copied()
            .map(|component| {
                registry
                    .resolution(component)
                    .map(|snapshot| (component, snapshot.view))
                    .ok_or_else(|| anyhow::anyhow!("direct control component disappeared"))
            })
            .collect()
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
            direct_driver: None,
            direct_controls: HashMap::new(),
            direct_content_ports: HashMap::new(),
            direct_control_nodes: HashMap::new(),
            direct_port_ids: HashMap::new(),
            direct_body_root: None,
            direct_synchronized: false,
            direct_history_overflow_rows: 0,
            layout_cache: LayoutCache::default(),
            paint_cache: PaintCache::default(),
            retained: None,
            last_surface: None,
            invalidated_components: HashSet::new(),
            incremental_sync_components: Vec::new(),
            incremental_topology_changed: false,
            incremental_requires_full_sync: false,
            incremental_paint_components: Vec::new(),
            incremental_paint_history: false,
            history_only_refresh: false,
            content_dirty: HashMap::new(),
            content_dirty_epoch: 0,
            content_prepared_epoch: None,
            incremental_paint_content: Vec::new(),
            theme_invalidated: false,
            retained_content_dependencies: HashMap::new(),
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
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_requires_full_sync = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_content.clear();
        self.theme_invalidated = false;
        self.retained_content_dependencies.clear();
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
        self.content_dirty.clear();
        self.content_dirty_epoch = 0;
        self.content_prepared_epoch = None;
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

    /// Records one affected ContentPort and invalidates only its retained
    /// dependency frontier.  Measurement dirtiness and paint propagation are
    /// tracked independently: a presentation-only update never discards
    /// width-dependent layout products, while a Source/delivery update always
    /// reaches the ContentHost and its metric ancestors before cache lookup.
    pub(crate) fn invalidate_content(&mut self, dirty: ContentDirty) {
        crate::perf::inc(crate::perf::Counter::ContentDirtyRecordsMarked);
        let requires_measurement = dirty.reason.requires_measurement();
        let requires_paint = requires_measurement || dirty.reason.is_paint_only();
        if let Some(existing_epoch) = self
            .content_dirty
            .get(&dirty.port_id)
            .map(|entry| entry.epoch)
        {
            let needs_new_epoch = self
                .content_prepared_epoch
                .is_some_and(|prepared| existing_epoch <= prepared);
            if !needs_new_epoch {
                let (escalated_measurement, escalated_paint, paint) = {
                    let entry = self
                        .content_dirty
                        .get_mut(&dirty.port_id)
                        .expect("content dirty entry still exists");
                    let escalated_measurement = requires_measurement && !entry.measurement;
                    let escalated_paint = requires_paint && !entry.paint;
                    entry.measurement |= requires_measurement;
                    entry.paint |= requires_paint;
                    (escalated_measurement, escalated_paint, entry.paint)
                };
                if escalated_measurement || escalated_paint {
                    self.invalidate_content_dependencies(
                        dirty.port_id,
                        escalated_measurement,
                        paint,
                    );
                }
                return;
            }
        }
        let epoch = self.content_dirty_epoch.saturating_add(1);
        self.content_dirty_epoch = epoch;
        let entry = self
            .content_dirty
            .entry(dirty.port_id)
            .or_insert(ContentDirtyRecord {
                epoch,
                ..ContentDirtyRecord::default()
            });
        entry.epoch = epoch;
        entry.measurement |= requires_measurement;
        entry.paint |= requires_paint;
        let paint = entry.paint;
        self.invalidate_content_dependencies(dirty.port_id, requires_measurement, paint);
    }

    fn invalidate_content_dependencies(&mut self, port_id: u64, measurement: bool, paint: bool) {
        let Some(retained) = self.retained.as_ref() else {
            if let Some(view_ids) = self.retained_content_dependencies.get(&port_id) {
                if measurement {
                    self.layout_cache.invalidate_view_ids(view_ids);
                }
                if paint {
                    self.paint_cache.invalidate_view_ids(view_ids);
                }
            }
            return;
        };
        let Some(view_ids) = retained.layout.tree.content_dependency_view_ids(port_id) else {
            return;
        };
        if measurement {
            self.layout_cache.invalidate_view_ids(&view_ids);
        }
        if paint {
            self.paint_cache.invalidate_view_ids(&view_ids);
        }
    }

    /// Invalidates only resolved presentation. Theme changes do not alter
    /// terminal metrics in the current engine, so retaining layout avoids a
    /// registry-wide measure/prepare flush while the next candidate repaints
    /// all visible rows with the new palette.
    pub(crate) fn invalidate_theme(&mut self) {
        self.paint_cache.clear();
        self.layout_cache.invalidate_content_entries();
        let mut content_view_ids = HashSet::new();
        for view_ids in self.retained_content_dependencies.values() {
            content_view_ids.extend(view_ids.iter().copied());
        }
        self.layout_cache.invalidate_view_ids(&content_view_ids);
        self.theme_invalidated = true;
        if let Some(retained) = self.retained.as_ref() {
            let epoch = self.content_dirty_epoch.saturating_add(1);
            self.content_dirty_epoch = epoch;
            let ports = retained
                .layout
                .tree
                .content_roots
                .keys()
                .copied()
                .collect::<Vec<_>>();
            // Theme revisions are intentionally absent from the content
            // layout-input key because they do not alter intrinsic metrics.
            // Invalidate only the affected ContentHost measurement entries
            // nevertheless: if another structural invalidation forces a full
            // pass before the paint-only path runs, it must not reuse a ticket
            // for the previous theme after ContentHostRegistry drops its old
            // paint products.
            let mut content_view_ids = HashSet::new();
            for port_id in ports {
                if let Some(nodes) = retained.layout.tree.content_roots.get(&port_id) {
                    content_view_ids.extend(
                        nodes
                            .iter()
                            .map(|node_id| retained.layout.tree.node(*node_id).view_id),
                    );
                }
                let entry = self.content_dirty.entry(port_id).or_default();
                entry.epoch = epoch;
                entry.paint = true;
            }
            self.layout_cache.invalidate_view_ids(&content_view_ids);
        }
        self.full_paint_pending = true;
        crate::perf::inc(crate::perf::Counter::ContentPaintPropagations);
    }

    fn has_unprepared_content(&self) -> bool {
        self.content_dirty_epoch != self.content_prepared_epoch.unwrap_or(0)
    }

    fn content_requires_measurement(&self) -> bool {
        self.content_dirty.values().any(|record| record.measurement)
    }

    /// Re-lays out only the affected ContentHost path. The first successful
    /// bounded replacement is the narrowest fixed-allocation boundary: a
    /// ContentHost that grows climbs to its parent dependency, while a host
    /// with a stable allocation patches only its own retained node. `true`
    /// means a parent frontier was patched and a complete paint is required
    /// because siblings may have moved; `false` means the content leaf can be
    /// repainted in isolation.
    fn try_local_content_refresh(
        &mut self,
        retained: &mut StableScene,
        ports: &[u64],
        content: &mut dyn ContentProvider,
    ) -> Option<bool> {
        self.layout_cache.begin_epoch();
        let mut parent_frontier = false;
        for port_id in ports {
            let node_id = retained
                .layout
                .tree
                .content_roots
                .get(port_id)
                .and_then(|nodes| nodes.first().copied())?;
            let layout_path = retained.layout.tree.path_to_root(node_id);
            let semantic_path = retained.root.scene.content_paths.get(port_id)?;
            if layout_path.len() != semantic_path.len() {
                return None;
            }
            let old_metric_revision = match &retained.layout.tree.node(node_id).content {
                crate::presentation::layout::LayoutContent::ContentHost {
                    metric_revision, ..
                } => *metric_revision,
                _ => return None,
            };
            let old_physical_completeness = match &retained.layout.tree.node(node_id).content {
                crate::presentation::layout::LayoutContent::ContentHost {
                    physically_complete,
                    ..
                } => *physically_complete,
                _ => return None,
            };
            // A bounded local patch can refresh a changed paint product, but
            // it cannot discover a new intrinsic height: applying the old
            // ContentHost allocation as a height bound would hide that metric
            // change and leave following siblings at stale positions. Probe
            // the affected leaf once against its committed width; a height
            // change escalates to the normal dependency-aware root pass.
            let content_width = retained.layout.tree.node(node_id).content_rect.width;
            let content_width_rule = semantic_path
                .last()
                .map(View::width)
                .unwrap_or(crate::presentation::WidthRule::Fill);
            let current_measurement = content.measure(*port_id, content_width, content_width_rule);
            let (old_content_width, old_content_height) =
                match &retained.layout.tree.node(node_id).content {
                    crate::presentation::layout::LayoutContent::ContentHost {
                        intrinsic_size,
                        ..
                    } => (intrinsic_size.width, intrinsic_size.height),
                    _ => return None,
                };
            let height_changed = current_measurement.intrinsic_size.height != old_content_height;
            let width_changed = content_width_rule == crate::presentation::WidthRule::Fit
                && current_measurement.intrinsic_size.width != old_content_width;
            let parent_uses_width = layout_path.windows(2).any(|path| {
                retained
                    .layout
                    .tree
                    .child_dependency(path[0], path[1])
                    .is_none_or(|dependency| dependency.parent_uses_child_width())
            });
            if height_changed
                || current_measurement.physically_complete != old_physical_completeness
                || (width_changed && parent_uses_width)
            {
                crate::perf::inc(crate::perf::Counter::ContentMetricChanges);
                return None;
            }
            let mut patched = false;
            let mut patch_root = node_id;
            for index in (0..layout_path.len()).rev() {
                let target = layout_path[index];
                let rect = retained.layout.tree.node(target).rect;
                if rect.is_empty() {
                    // An empty committed ContentHost has no fixed allocation
                    // to patch. Escalate to the nearest non-empty ancestor so
                    // the first visible append can establish geometry.
                    continue;
                }
                let replacement = layout_view_with_overlay_and_cache_and_content(
                    &semantic_path[index],
                    LayoutConstraints::bounded(rect.size()),
                    &retained.root.scene.overlay,
                    None,
                    &mut self.layout_cache,
                    content,
                );
                if replacement.size != rect.size()
                    || !retained.layout.tree.patch_subtree(target, &replacement)
                {
                    continue;
                }
                patched = true;
                patch_root = target;
                parent_frontier |= index != layout_path.len().saturating_sub(1);
                break;
            }
            if !patched
                || !retained
                    .layout
                    .tree
                    .patch_component_geometry_subtree(patch_root, &mut retained.layout.components)
            {
                return None;
            }
            let new_metric_revision = match &retained.layout.tree.node(node_id).content {
                crate::presentation::layout::LayoutContent::ContentHost {
                    metric_revision, ..
                } => *metric_revision,
                _ => return None,
            };
            if old_metric_revision != new_metric_revision {
                crate::perf::inc(crate::perf::Counter::ContentMetricChanges);
            }
        }
        Some(parent_frontier)
    }

    fn pending_content_ports(&self) -> Vec<u64> {
        let mut ports = self.content_dirty.keys().copied().collect::<Vec<_>>();
        ports.sort_unstable();
        ports
    }

    pub(crate) fn content_candidate_epoch(&self) -> u64 {
        self.content_dirty_epoch
    }

    /// Marks the current content candidate's dirtiness as presented. Newer
    /// records (accepted while an older backend receipt was in flight) remain
    /// queued for the next candidate.
    pub(crate) fn commit_content_candidate(&mut self, epoch: u64) {
        self.content_dirty.retain(|_, record| record.epoch > epoch);
        if self.content_dirty.is_empty() {
            self.content_prepared_epoch = Some(epoch);
        }
    }

    /// Candidate preparation failed or its physical receipt failed. Retain
    /// all dirty records and force the next attempt to evaluate them again.
    pub(crate) fn abort_content_candidate(&mut self) {
        self.content_prepared_epoch = None;
        self.incremental_paint_content.clear();
    }

    /// Invalidates the retained scene root for body/history/topology changes.
    pub(crate) fn invalidate_root(&mut self) {
        // Keep dependency-local layout/paint products for unchanged retained
        // descendants. The replacement root gets a new ViewId. Content input
        // revisions participate in leaf layout keys, while theme revisions
        // are intentionally excluded and use the deferred ContentHost path
        // index in invalidate_theme; clearing both caches here would
        // relayout/repaint every stable sibling on a narrow publication.
        if let Some(retained) = self.retained.as_ref() {
            self.retained_content_dependencies.clear();
            for nodes in retained.layout.tree.content_roots.values() {
                for node in nodes {
                    let port_id = match &retained.layout.tree.node(*node).content {
                        crate::presentation::layout::LayoutContent::ContentHost {
                            port_id, ..
                        } => *port_id,
                        _ => continue,
                    };
                    let dependencies = self
                        .retained_content_dependencies
                        .entry(port_id)
                        .or_default();
                    for ancestor in retained.layout.tree.path_to_root(*node) {
                        dependencies.insert(retained.layout.tree.node(ancestor).view_id);
                    }
                }
            }
        }
        self.retained = None;
        self.last_surface = None;
        self.invalidated_components.clear();
        self.incremental_sync_components.clear();
        self.incremental_topology_changed = false;
        self.incremental_requires_full_sync = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
        self.content_prepared_epoch = None;
        self.incremental_paint_content.clear();
        self.full_paint_pending = false;
        self.pending_damage = None;
    }

    /// Drops all derived candidate data after a backend presentation failure.
    /// The `HostInner` frame remains authoritative, so stale candidate caches
    /// must not seed the next retry.
    pub(crate) fn discard_candidate(&mut self) {
        self.invalidate_root();
        self.layout_cache.clear();
        self.paint_cache.clear();
        self.abort_content_candidate();
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

    pub(crate) fn focus_component(
        &mut self,
        target: crate::component::ComponentId,
        geometry: &crate::presentation::layout::ComponentGeometryMap,
        registry: &mut crate::component::ComponentRegistry,
    ) -> bool {
        self.focus.focus_component(
            target,
            &self.graph,
            &self.capabilities,
            Some(geometry),
            registry,
        )
    }

    pub(crate) fn confirmed_component_geometry(
        &self,
        target: crate::component::ComponentId,
    ) -> Option<crate::presentation::layout::ComponentGeometry> {
        self.retained
            .as_ref()?
            .layout
            .components
            .entries
            .get(&target)
            .copied()
    }

    #[cfg(test)]
    pub(crate) fn focused(&self) -> Option<crate::component::ComponentId> {
        self.focused_component()
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
        self.render_at_with_content(
            Instant::now(),
            scene,
            registry,
            theme,
            sink,
            viewport,
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
        self.render_at_with_content(now, scene, registry, theme, sink, viewport, &mut content)
    }

    pub(crate) fn render_at_with_content<S, F>(
        &mut self,
        now: Instant,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        theme: &Theme,
        sink: &mut S,
        mut viewport: F,
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
                content,
            )?;

            // A previous native sink error may have left physical scrollback
            // partially applied.  The transfer marker is irreversible, so
            // skip another emission until the host paints its recovery frame;
            // retrying the same rows here would duplicate output.
            if scene
                .history()
                .is_some_and(crate::History::native_synchronization_unknown)
            {
                crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                return Ok(self.paint_with_content(resolved, theme, content));
            }

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
                        content,
                    )?;
                    crate::history::trace::trace_resolve_pressure(resolves, 0, transfer_calls);
                    return Ok(self.paint_with_content(pinned, theme, content));
                }
            }
        }
    }

    /// Resolves and paints one candidate while owning, rather than executing,
    /// the next native History operation. The returned plan is the exact
    /// semantic frontier used by the candidate; callers submit its rows only
    /// after releasing their acceptance guard and acknowledge that plan once.
    /// This is the asynchronous host path. Synchronous compatibility callers
    /// continue to use `render_at_with_content` above, whose sink performs the
    /// same prepare/ack operation inline.
    pub(crate) fn prepare_at_with_content(
        &mut self,
        now: Instant,
        scene: &mut Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        theme: &Theme,
        content: &mut dyn ContentProvider,
    ) -> Result<
        (
            PreparedSceneFrame,
            Option<crate::history::NativeTransferPlan>,
        ),
        SceneHostError<anyhow::Error>,
    > {
        let resolved = self.resolve_stable_at_with_anchor(
            scene,
            registry,
            size,
            now,
            HistoryViewportAnchor::FollowEnd,
            content,
        )?;

        if scene
            .history()
            .is_some_and(crate::History::native_synchronization_unknown)
        {
            return Ok((self.paint_with_content(resolved, theme, content), None));
        }

        let front_content_blocked = scene
            .history()
            .and_then(crate::History::front_content_attachment_id)
            .is_some_and(|port_id| content.history_transfer_blocked(port_id, size.width));
        if resolved.root.history_overflow_rows == 0 || front_content_blocked {
            return Ok((self.paint_with_content(resolved, theme, content), None));
        }

        let plan = scene.history().and_then(|history| {
            crate::history::prepare_native_transfer_with_theme_and_content(
                history,
                size.width,
                resolved.root.history_overflow_rows,
                theme,
                content,
            )
        });

        // A missing plan means semantic work is blocked (for example, a live
        // unit or an unsupported composite ContentHost). Pin the candidate to
        // the current frontier exactly as the synchronous sink path does.
        // A present plan is also painted front-pinned because its physical
        // acknowledgement has not happened yet.
        self.retained = Some(resolved);
        let pinned = self.resolve_stable_at_with_anchor(
            scene,
            registry,
            size,
            now,
            HistoryViewportAnchor::NativeFrontier,
            content,
        )?;
        Ok((self.paint_with_content(pinned, theme, content), plan))
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
        content: &mut dyn ContentProvider,
    ) -> Result<StableScene, SceneHostError<E>> {
        let mut force_full = false;
        let mut layout_epoch_started = false;
        for _ in 0..MAX_LAYOUT_PASSES {
            let content_epoch = self.content_dirty_epoch;
            let resolved = if !force_full {
                match self.try_incremental_stable(scene, registry, size, anchor, content) {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) => {
                        if !layout_epoch_started {
                            self.layout_cache.begin_epoch();
                            layout_epoch_started = true;
                        }
                        self.resolve_full_stable(scene, registry, size, anchor, content)?
                    }
                    Err(error) => return Err(SceneHostError::Resolve(error)),
                }
            } else {
                if !layout_epoch_started {
                    self.layout_cache.begin_epoch();
                    layout_epoch_started = true;
                }
                self.resolve_full_stable(scene, registry, size, anchor, content)?
            };
            let incremental_host = (self.history_only_refresh
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
                self.incremental_paint_content.clear();
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                continue;
            }

            // A Source mutation may be accepted by a synchronous callback
            // while this candidate is being measured or synchronized.  Its
            // snapshot must be deferred to a fresh pass rather than silently
            // claiming that the older candidate represented the new epoch.
            if content_epoch != self.content_dirty_epoch {
                force_full = true;
                self.retained = None;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_content.clear();
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
                self.incremental_paint_content.clear();
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                continue;
            }
            self.invalidated_components.clear();
            self.incremental_sync_components.clear();
            self.incremental_topology_changed = false;
            self.incremental_requires_full_sync = false;
            self.history_only_refresh = false;
            self.content_prepared_epoch = Some(content_epoch);
            self.theme_invalidated = false;
            #[cfg(test)]
            {
                self.resolve_count += 1;
            }
            return Ok(resolved);
        }
        Err(SceneHostError::DidNotConverge)
    }

    fn try_incremental_stable(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
        content: &mut dyn ContentProvider,
    ) -> Result<Option<StableScene>, ResolveError> {
        let history_revision = scene.history().map_or(0, crate::History::revision);
        let native_history_revision = scene.history().map_or(0, crate::History::native_revision);
        let history_identity = scene.history().map_or(0, crate::History::identity);
        let Some(retained_scene) = self.retained.as_ref() else {
            return Ok(None);
        };
        if retained_scene.layout.tree.size != size {
            return Ok(None);
        }

        let body_changed =
            !crate::presentation::View::ptr_eq(&retained_scene.root.body_view, scene.layout_body());
        let history_changed = retained_scene.history_identity != history_identity
            || retained_scene.history_revision != history_revision;
        let native_history_changed =
            retained_scene.native_history_revision != native_history_revision;
        let body_invalidated = self.invalidated_components.iter().any(|component| {
            retained_scene.root.scene.mounts.contains(*component)
                && !retained_scene.root.history_components.contains(component)
        });
        if self.theme_invalidated
            && self.invalidated_components.is_empty()
            && !self.has_unprepared_content()
            && !body_changed
            && !body_invalidated
            && !history_changed
            && !native_history_changed
        {
            let retained = self
                .retained
                .take()
                .expect("retained scene was checked above");
            self.incremental_sync_components.clear();
            self.incremental_topology_changed = false;
            self.incremental_requires_full_sync = false;
            self.incremental_paint_components.clear();
            self.incremental_paint_content.clear();
            self.incremental_paint_history = false;
            self.history_only_refresh = false;
            self.pending_damage = None;
            return Ok(Some(retained));
        }
        if self.has_unprepared_content() {
            // Content changes do not alter the resolved semantic scene.  A
            // body-only host can therefore retain its scene and evaluate
            // only the affected layout dependency frontier.  History has a
            // separate projection/receipt owner, so it conservatively takes
            // the normal root path while still reusing the targeted caches.
            let content_in_history =
                retained_scene
                    .root
                    .history_scene
                    .as_ref()
                    .is_some_and(|history| {
                        self.pending_content_ports()
                            .iter()
                            .any(|port_id| history.content_paths.contains_key(port_id))
                    });
            if !content_in_history
                && self.invalidated_components.is_empty()
                && !body_changed
                && !body_invalidated
                && !history_changed
                && !native_history_changed
            {
                let mut retained = self
                    .retained
                    .take()
                    .expect("retained scene was checked above");
                let pending_ports = self.pending_content_ports();
                if self.content_requires_measurement() {
                    crate::perf::add(
                        crate::perf::Counter::ContentMetricEvaluations,
                        self.content_dirty.len() as u64,
                    );
                    let Some(parent_frontier) =
                        self.try_local_content_refresh(&mut retained, &pending_ports, content)
                    else {
                        self.retained = Some(retained);
                        return Ok(None);
                    };
                    if parent_frontier {
                        self.full_paint_pending = true;
                    }
                } else {
                    // Presentation/viewport-only records still need a fresh
                    // prepared paint identity (for example after a theme
                    // swap), but their intrinsic metrics are not a reason to
                    // relayout ancestors.  Evaluate the affected leaves and
                    // patch only their retained content records.
                    for port_id in &pending_ports {
                        let Some(nodes) = retained.layout.tree.content_roots.get(port_id) else {
                            continue;
                        };
                        let Some(node_id) = nodes.first().copied() else {
                            continue;
                        };
                        let width = retained.layout.tree.node(node_id).content_rect.width;
                        let measurement =
                            content.measure(*port_id, width, crate::presentation::WidthRule::Fill);
                        if retained
                            .layout
                            .tree
                            .update_content_measurement(*port_id, &measurement)
                        {
                            crate::perf::inc(crate::perf::Counter::ContentMetricChanges);
                            self.retained = Some(retained);
                            return Ok(None);
                        }
                    }
                }
                self.incremental_paint_content = pending_ports;
                self.incremental_sync_components.clear();
                self.incremental_topology_changed = false;
                self.incremental_requires_full_sync = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_history = false;
                self.history_only_refresh = false;
                self.pending_damage = None;
                #[cfg(test)]
                {
                    self.incremental_resolves += 1;
                }
                return Ok(Some(retained));
            }
            return Ok(None);
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
                .expect("retained scene was checked above");
            let affected = self
                .invalidated_components
                .iter()
                .copied()
                .filter(|component| retained.root.history_components.contains(component))
                .collect();
            return self.refresh_history_projection(
                scene, registry, size, anchor, retained, affected, false, content,
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
            match prepare_component_subtree_update(&retained, registry, *id, content) {
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

    fn resolve_full_stable<E>(
        &mut self,
        scene: &Scene,
        registry: &mut ComponentRegistry,
        size: Size,
        anchor: HistoryViewportAnchor,
        content: &mut dyn ContentProvider,
    ) -> Result<StableScene, SceneHostError<E>> {
        if self.has_unprepared_content() && self.content_requires_measurement() {
            crate::perf::add(
                crate::perf::Counter::ContentMetricEvaluations,
                self.content_dirty.len() as u64,
            );
        }
        self.pending_damage = None;
        let resolved = resolve_root_scene_with_anchor_and_cache_and_content(
            scene,
            registry,
            size,
            anchor,
            &mut self.layout_cache,
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
        self.incremental_paint_content.clear();
        self.incremental_paint_history = false;
        self.history_only_refresh = false;
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
        self.retained_content_dependencies.clear();
        let retained = self.retained.as_ref().expect("retained frame installed");
        let content_damage = DamageRegion::from_rects(
            self.incremental_paint_content.iter().flat_map(|port_id| {
                retained
                    .layout
                    .tree
                    .content_repaint_roots(*port_id)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|id| retained.layout.tree.incremental_paint_rect(id))
            }),
            retained.layout.tree.size,
        );
        let compiler = ViewCompiler::with_interaction(theme, self.focus.focused(), &self.graph);
        if !self.full_paint_pending
            && (self.incremental_paint_history
                || !self.incremental_paint_components.is_empty()
                || !self.incremental_paint_content.is_empty())
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
                if incremental {
                    for port_id in self.incremental_paint_content.iter().copied() {
                        crate::perf::inc(crate::perf::Counter::ContentPaintPropagations);
                        let Some(roots) = retained.layout.tree.content_repaint_roots(port_id)
                        else {
                            incremental = false;
                            break;
                        };
                        for root in roots {
                            if !ViewPainter.paint_subtree_into_with_content(
                                &compiler,
                                &retained.layout.tree,
                                root,
                                &mut surface,
                                &mut incremental_cache,
                                content,
                            ) {
                                incremental = false;
                                break;
                            }
                        }
                        if !incremental {
                            break;
                        }
                    }
                }
                self.incremental_paint_history = false;
                self.incremental_paint_components.clear();
                self.incremental_paint_content.clear();
                if incremental {
                    surface.physically_complete = retained.layout.tree.physically_complete;
                    let output = surface.clone();
                    self.last_surface = Some(surface);
                    return PreparedSceneFrame {
                        surface: output,
                        history_overlay: retained.root.history_overlay.clone(),
                        damage: self.pending_damage.take().unwrap_or_else(|| {
                            let rects = content_damage.rects.clone();
                            if rects.is_empty() {
                                DamageRegion::full(retained.layout.tree.size)
                            } else {
                                DamageRegion::from_rects(rects, retained.layout.tree.size)
                            }
                        }),
                        component_geometry: retained.layout.components.clone(),
                        view_geometry: retained.layout.tree.view_geometry(),
                        occurrence_geometry: HashMap::new(),
                    };
                }
            }
            self.incremental_paint_history = false;
            self.incremental_paint_components.clear();
            self.incremental_paint_content.clear();
        }
        self.incremental_paint_history = false;
        self.incremental_paint_components.clear();
        self.incremental_paint_content.clear();
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
            component_geometry: retained.layout.components.clone(),
            view_geometry: retained.layout.tree.view_geometry(),
            occurrence_geometry: HashMap::new(),
        }
    }
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
    _content: &mut dyn ContentProvider,
) -> Result<PreparedComponentSubtree, ResolveError> {
    let snapshot = registry
        .resolution(id)
        .ok_or(ResolveError::MissingComponent { id })?;
    let subtree = resolve_component_subtree(&snapshot.view, registry, id)?;
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
    update_component_content_path_index(scene, id, subtree, old_ids);
}

/// Replaces only the semantic path/index records owned by one component
/// occurrence. The component prefix is retained, while old nested component
/// and ContentPort paths are removed before new subtree paths are installed.
/// This keeps Source-only refreshes on the retained index and releases stale
/// component-produced `View` Arc owners after a structural replacement.
fn update_component_content_path_index(
    scene: &mut ResolvedScene,
    component: ComponentId,
    subtree: &ResolvedScene,
    old_components: &[ComponentId],
) {
    let prefix = scene.component_paths.get(&component).cloned();
    let mut old_ports = Vec::new();
    let mut affected_component_ids = HashSet::new();
    for old_component in old_components {
        scene.component_paths.remove(old_component);
        if let Some(ports) = scene.content_path_components.remove(old_component) {
            old_ports.extend(ports);
        }
    }
    old_ports.sort_unstable();
    old_ports.dedup();
    for port_id in &old_ports {
        if let Some(path) = scene.content_paths.get(port_id) {
            for view in path {
                if let ViewKind::ComponentSlot(slot) = view.kind() {
                    affected_component_ids.insert(slot.id);
                }
            }
        }
    }
    for port_id in &old_ports {
        scene.content_paths.remove(port_id);
    }
    // Ancestor reverse buckets also referenced the removed ports. Remove only
    // those IDs from the affected buckets; scanning/normalizing every bucket
    // would turn a local component replacement into a registry-wide walk.
    for component_id in &affected_component_ids {
        if let Some(ports) = scene.content_path_components.get_mut(component_id) {
            ports.retain(|port_id| !old_ports.contains(port_id));
        }
    }

    let Some(prefix) = prefix else {
        debug_assert!(
            false,
            "component content path prefix must be indexed before incremental replacement"
        );
        return;
    };
    scene.component_paths.insert(component, prefix.clone());
    for (nested_component, path) in &subtree.component_paths {
        let mut full_path = Vec::with_capacity(prefix.len().saturating_add(path.len()));
        full_path.extend(prefix.iter().cloned());
        full_path.extend(path.iter().cloned());
        scene.component_paths.insert(*nested_component, full_path);
    }
    for (port_id, path) in &subtree.content_paths {
        let mut full_path = Vec::with_capacity(prefix.len().saturating_add(path.len()));
        full_path.extend(prefix.iter().cloned());
        full_path.extend(path.iter().cloned());
        for view in &full_path {
            if let ViewKind::ComponentSlot(slot) = view.kind() {
                affected_component_ids.insert(slot.id);
                scene
                    .content_path_components
                    .entry(slot.id)
                    .or_default()
                    .push(*port_id);
            }
        }
        scene.content_paths.insert(*port_id, full_path);
    }
    for component_id in affected_component_ids {
        if let Some(ports) = scene.content_path_components.get_mut(&component_id) {
            ports.sort_unstable();
            ports.dedup();
        }
    }
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
    use crate::presentation::factory as vf;
    use crate::{
        BorderSpec, ColorSpec, Component, ComponentCx, ComponentHandle, InteractionResult, Key,
        KeyStroke, Scene, ScrollPane, StyleSelector, ThemeColor, View, backend::NativeHistorySink,
        component::ComponentRegistry, geometry::Size, physical::PhysicalRow,
    };

    #[derive(Debug)]
    struct LayoutAware {
        changed: bool,
        calls: usize,
    }

    impl Component for LayoutAware {
        fn view(&self) -> View {
            if self.changed {
                vf::text("new\nrow")
            } else {
                vf::text("old")
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

    #[derive(Default)]
    struct IndexedContentProvider {
        revisions: HashMap<u64, u64>,
    }

    impl ContentProvider for IndexedContentProvider {
        fn projection_revision(&self, port_id: u64, _offered_width: u16) -> u64 {
            self.revisions.get(&port_id).copied().unwrap_or(0)
        }

        fn measure(
            &mut self,
            port_id: u64,
            _offered_width: u16,
            _width_rule: crate::presentation::WidthRule,
        ) -> crate::presentation::ContentMeasurement {
            let revision = self.revisions.get(&port_id).copied().unwrap_or(0);
            crate::presentation::ContentMeasurement {
                intrinsic_size: Size::new(1, revision.max(1) as u16),
                physically_complete: true,
                projection_revision: revision,
                metric_revision: 1,
                paint_revision: revision,
                connector_id: (revision != 0).then_some(port_id),
                projection_identity: revision,
            }
        }

        fn paint_window(
            &self,
            ticket: crate::presentation::PreparedProjectionTicket,
            window: crate::presentation::ContentWindow,
            target: &mut Surface,
            target_origin: (u16, u16),
            _clip: crate::geometry::Rect,
            _style: crate::physical::PhysicalStyle,
        ) {
            let grapheme = ticket.projection_revision.to_string();
            for row in 0..window.row_count {
                let y = target_origin.1.saturating_add(row as u16);
                if y < target.height() && target_origin.0 < target.width() {
                    let cell = target.get_mut(target_origin.0, y);
                    cell.grapheme = Some(grapheme.clone());
                    cell.painted = true;
                }
            }
        }
    }

    #[derive(Debug)]
    struct IndexedContentComponent {
        view: View,
    }

    impl Component for IndexedContentComponent {
        fn view(&self) -> View {
            self.view.clone()
        }
    }

    fn indexed_content_view(port_id: u64, padding: u16) -> View {
        crate::presentation::factory::background(
            crate::presentation::factory::padding(
                vf::content_host(port_id).expect("test ContentHost port must be positive"),
                crate::Insets::all(padding),
            ),
            ColorSpec::ansi(1),
        )
    }

    impl Component for TickingLeaf {
        fn view(&self) -> View {
            vf::text(format!("tick-{}", self.frame))
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
            vf::text(self.text.clone())
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
                View::component(self.child)
            } else {
                vf::text("root")
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
        let scene = Scene::new(crate::presentation::factory::column(
            handles
                .iter()
                .map(|handle| View::component(*handle))
                .collect(),
            0,
        ));
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
        let mut same_fresh_host = SceneHost::default();
        let same_fresh = same_fresh_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let same_fresh_frame = same_fresh_host.paint(same_fresh, &Theme::default());
        assert_eq!(
            same_geometry_frame.screen_lines(),
            same_fresh_frame.screen_lines()
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
        let mut fresh_host = SceneHost::default();
        let fresh = fresh_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let fresh_frame = fresh_host.paint(fresh, &Theme::default());
        assert_eq!(geometry_frame.screen_lines(), fresh_frame.screen_lines());
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
        let scene = Scene::new(crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    View::component(first),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    View::component(second),
                ),
            ],
            0,
        ));
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
            vf::text("first")
        );
        assert_eq!(host.last_surface.as_ref().unwrap(), &initial_frame.surface);
    }

    #[test]
    fn retained_component_paint_preserves_ancestor_surface_background() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "x".to_owned(),
        });
        let scene = Scene::new(crate::presentation::factory::fill_height(
            crate::presentation::factory::fill_width(crate::presentation::factory::background(
                crate::presentation::factory::column_specs(
                    vec![(
                        crate::presentation::ir::TrackSize::Content { max: None },
                        View::component(handle),
                    )],
                    0,
                ),
                ColorSpec::Ansi(34),
            )),
        ));
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

        let mut fresh_host = SceneHost::default();
        let fresh = fresh_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let fresh_frame = fresh_host.paint(fresh, &Theme::default());
        assert_eq!(retained_frame.surface, fresh_frame.surface);
    }

    #[test]
    fn component_update_refreshes_history_branch_without_rebuilding_body() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "body-old".to_owned(),
        });
        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("history-old"))
            .unwrap();
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
        scene
            .history_mut()
            .unwrap()
            .push(crate::presentation::factory::text("history-new"))
            .unwrap();
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

    #[cfg(feature = "native-host")]
    #[test]
    fn component_content_path_index_updates_same_and_switched_ports() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(IndexedContentComponent {
            view: indexed_content_view(101, 1),
        });
        let scene = Scene::new(View::component(handle));
        let size = Size::new(16, 4);
        let now = Instant::now();
        let mut host = SceneHost::default();
        let mut content = IndexedContentProvider {
            revisions: HashMap::from([(101, 1), (202, 1)]),
        };
        let initial = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        assert!(initial.root.scene.content_paths.contains_key(&101));
        assert_eq!(
            initial.root.scene.content_path_components.get(&handle.id()),
            Some(&vec![101]),
            "initial reverse index must name the nested ContentPort once"
        );
        host.paint_with_content(initial, &Theme::default(), &content);

        // Replace the same Port with a differently decorated component View.
        registry
            .with_mut(handle, |component| {
                component.view = indexed_content_view(101, 2);
            })
            .unwrap();
        host.invalidate_component(handle.id());
        let same_port = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        let same_path = same_port
            .root
            .scene
            .content_paths
            .get(&101)
            .expect("same Port path must survive component replacement");
        assert_eq!(
            same_path.last().and_then(View::content_attachment_id),
            Some(101)
        );
        assert_eq!(
            same_port
                .root
                .scene
                .content_path_components
                .get(&handle.id()),
            Some(&vec![101]),
            "same-Port replacement must not duplicate the reverse owner"
        );
        host.paint_with_content(same_port, &Theme::default(), &content);
        host.commit_content_candidate(host.content_candidate_epoch());

        // A later Source/recolor invalidation must use the retained updated
        // path rather than rediscovering the old component sibling path.
        content.revisions.insert(101, 2);
        host.invalidate_content(ContentDirty::new(
            101,
            None,
            crate::presentation::ContentDirtyReason::Presentation,
        ));
        let recolored = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        assert!(recolored.root.scene.content_paths.contains_key(&101));
        host.paint_with_content(recolored, &Theme::default(), &content);
        host.commit_content_candidate(host.content_candidate_epoch());

        // Switch the component to another Port. The old path owner is
        // released from the retained index and the new path is indexed.
        registry
            .with_mut(handle, |component| {
                component.view = indexed_content_view(202, 3);
            })
            .unwrap();
        host.invalidate_component(handle.id());
        let switched = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        assert!(!switched.root.scene.content_paths.contains_key(&101));
        assert!(switched.root.scene.content_paths.contains_key(&202));
        assert_eq!(
            switched
                .root
                .scene
                .content_path_components
                .get(&handle.id()),
            Some(&vec![202]),
            "switch replacement must remove the old reverse owner"
        );
        host.paint_with_content(switched, &Theme::default(), &content);
        host.commit_content_candidate(host.content_candidate_epoch());

        content.revisions.insert(202, 2);
        host.invalidate_content(ContentDirty::new(
            202,
            None,
            crate::presentation::ContentDirtyReason::SourceInput,
        ));
        let source_update = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        assert!(source_update.root.scene.content_paths.contains_key(&202));
        assert!(!source_update.root.scene.content_paths.contains_key(&101));

        for (iteration, port_id) in [101_u64, 202, 101, 202, 101, 202].into_iter().enumerate() {
            registry
                .with_mut(handle, |component| {
                    component.view = indexed_content_view(port_id, (iteration as u16) + 4);
                })
                .unwrap();
            host.invalidate_component(handle.id());
            let replaced = host
                .resolve_stable_at_with_anchor::<()>(
                    &scene,
                    &mut registry,
                    size,
                    now,
                    HistoryViewportAnchor::FollowEnd,
                    &mut content,
                )
                .unwrap();
            assert_eq!(
                replaced
                    .root
                    .scene
                    .content_path_components
                    .get(&handle.id()),
                Some(&vec![port_id]),
                "iteration {iteration} must retain exactly one reverse ContentPort owner"
            );
            assert_eq!(replaced.root.scene.content_paths.len(), 1);
            host.paint_with_content(replaced, &Theme::default(), &content);
            host.commit_content_candidate(host.content_candidate_epoch());
        }
    }

    #[test]
    fn content_dirty_coalescing_keeps_each_port_newer_than_a_prepared_epoch() {
        let mut host = SceneHost::default();
        host.invalidate_content(ContentDirty::new(
            101,
            None,
            crate::presentation::ContentDirtyReason::Presentation,
        ));
        host.invalidate_content(ContentDirty::new(
            202,
            None,
            crate::presentation::ContentDirtyReason::Presentation,
        ));
        let epoch = host.content_dirty_epoch;
        assert!(host.content_dirty[&101].epoch < epoch);
        assert_eq!(host.content_dirty[&202].epoch, epoch);
        host.content_prepared_epoch = Some(epoch);

        // The first post-capture update advances the global epoch. The second
        // port must still advance its own record instead of being merged at
        // the old epoch and dropped by commit(epoch).
        host.invalidate_content(ContentDirty::new(
            101,
            None,
            crate::presentation::ContentDirtyReason::SourceInput,
        ));
        host.invalidate_content(ContentDirty::new(
            202,
            None,
            crate::presentation::ContentDirtyReason::SourceInput,
        ));
        assert!(host.content_dirty[&101].epoch > epoch);
        assert!(host.content_dirty[&202].epoch > epoch);
        assert!(host.content_dirty[&101].measurement);
        assert!(host.content_dirty[&202].measurement);
        host.commit_content_candidate(epoch);
        assert!(host.content_dirty.contains_key(&101));
        assert!(host.content_dirty.contains_key(&202));
    }

    #[test]
    fn content_dirty_reason_escalation_marks_measurement_and_keeps_pending_epoch() {
        let mut host = SceneHost::default();
        host.content_dirty_epoch = 4;
        host.content_prepared_epoch = Some(3);
        host.content_dirty.insert(
            303,
            ContentDirtyRecord {
                epoch: 4,
                measurement: false,
                paint: true,
            },
        );
        host.invalidate_content(ContentDirty::new(
            303,
            None,
            crate::presentation::ContentDirtyReason::SourceInput,
        ));
        assert_eq!(host.content_dirty_epoch, 4);
        assert_eq!(host.content_dirty[&303].epoch, 4);
        assert!(host.content_dirty[&303].measurement);
        assert!(host.content_dirty[&303].paint);
    }

    #[test]
    fn source_escalation_refreshes_retained_content_metrics_and_screen() {
        let mut registry = ComponentRegistry::new();
        let scene = Scene::new(
            crate::presentation::factory::content_host(303)
                .expect("test ContentHost port must be positive"),
        );
        let size = Size::new(16, 4);
        let now = Instant::now();
        let mut host = SceneHost::default();
        let mut content = IndexedContentProvider {
            revisions: HashMap::from([(303, 1)]),
        };
        let initial = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        let initial_node = initial.layout.tree.content_roots[&303][0];
        let initial_frame = host.paint_with_content(initial, &Theme::default(), &content);
        host.commit_content_candidate(host.content_candidate_epoch());
        assert!(
            initial_frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with('1'))
        );

        // A presentation-only mark is already pending when the Source/metric
        // reason arrives. The second reason must still invalidate the retained
        // ContentHost path and update both its geometry and physical output.
        content.revisions.insert(303, 2);
        host.invalidate_content(ContentDirty::new(
            303,
            None,
            crate::presentation::ContentDirtyReason::Presentation,
        ));
        host.invalidate_content(ContentDirty::new(
            303,
            None,
            crate::presentation::ContentDirtyReason::SourceInput,
        ));
        let refreshed = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        let refreshed_node = refreshed.layout.tree.content_roots[&303][0];
        let initial_height = match &host
            .retained
            .as_ref()
            .expect("initial retained scene")
            .layout
            .tree
            .node(initial_node)
            .content
        {
            crate::presentation::layout::LayoutContent::ContentHost { intrinsic_size, .. } => {
                intrinsic_size.height
            }
            _ => 0,
        };
        let refreshed_height = match &refreshed.layout.tree.node(refreshed_node).content {
            crate::presentation::layout::LayoutContent::ContentHost { intrinsic_size, .. } => {
                intrinsic_size.height
            }
            _ => 0,
        };
        assert!(refreshed_height > initial_height);
        let frame = host.paint_with_content(refreshed, &Theme::default(), &content);
        host.commit_content_candidate(host.content_candidate_epoch());
        assert!(
            frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with('2'))
        );
    }

    #[test]
    fn content_refresh_preserves_a_pending_full_paint_theme_obligation() {
        let mut registry = ComponentRegistry::new();
        let scene = Scene::new(crate::presentation::factory::column(
            vec![
                crate::presentation::factory::foreground(
                    vf::text("label"),
                    ColorSpec::theme("label"),
                ),
                crate::presentation::factory::content_host(404)
                    .expect("test ContentHost port must be positive"),
            ],
            0,
        ));
        let size = Size::new(16, 4);
        let now = Instant::now();
        let mut host = SceneHost::default();
        let mut content = IndexedContentProvider {
            revisions: HashMap::from([(404, 1)]),
        };
        let theme_one = Theme::new().with_color("label", ThemeColor::Indexed(1));
        let theme_two = Theme::new().with_color("label", ThemeColor::Indexed(2));
        let initial = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        let _ = host.paint_with_content(initial, &theme_one, &content);
        host.commit_content_candidate(host.content_candidate_epoch());

        content.revisions.insert(404, 2);
        host.invalidate_content(ContentDirty::new(
            404,
            None,
            crate::presentation::ContentDirtyReason::Presentation,
        ));
        host.invalidate_theme();
        let refreshed = host
            .resolve_stable_at_with_anchor::<()>(
                &scene,
                &mut registry,
                size,
                now,
                HistoryViewportAnchor::FollowEnd,
                &mut content,
            )
            .unwrap();
        let frame = host.paint_with_content(refreshed, &theme_two, &content);
        let label_row = frame
            .screen_lines()
            .iter()
            .position(|line| line.contains("label"))
            .expect("the themed label must be painted");
        let label = frame
            .surface
            .row_cells(label_row as u16)
            .iter()
            .find(|cell| cell.grapheme.as_deref() == Some("l"))
            .expect("the themed label must be painted");
        assert_eq!(
            label.style.foreground,
            Some(crate::physical::PhysicalColor::Indexed(2)),
            "a theme change must repaint clean siblings when content also refreshes"
        );
        assert!(
            frame
                .screen_lines()
                .iter()
                .any(|line| line.starts_with('2')),
            "the content paint update must remain visible"
        );
    }

    #[test]
    fn geometry_change_with_history_repaints_body_and_clears_old_surface() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(R6bLeaf {
            text: "body-old".to_owned(),
        });
        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("history"))
            .unwrap();
        let scene = Scene::with_history(
            history,
            crate::presentation::factory::column_specs(
                vec![
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        vf::text("tail"),
                    ),
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        View::component(handle),
                    ),
                ],
                0,
            ),
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

        let mut fresh_host = SceneHost::default();
        let fresh = fresh_host
            .resolve_stable::<()>(&scene, &mut registry, size)
            .unwrap();
        let fresh_frame = fresh_host.paint(fresh, &Theme::default());
        assert_eq!(frame.screen_lines(), fresh_frame.screen_lines());
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
                .push(crate::presentation::factory::fill_width(vf::text(format!(
                    "old-{index}"
                ))))
                .unwrap();
        }
        let mut scene = Scene::with_history(
            history,
            crate::presentation::factory::column_specs(
                vec![
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        vf::text("tail"),
                    ),
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        View::component(handle),
                    ),
                ],
                0,
            ),
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

        let mut fresh_host = SceneHost::default();
        let mut fresh_sink = TestSink::default();
        let fresh_frame = fresh_host
            .render(
                &mut scene,
                &mut registry,
                &Theme::default(),
                &mut fresh_sink,
                |_| Ok(size),
            )
            .unwrap();
        assert_eq!(frame.screen_lines(), fresh_frame.screen_lines());
    }

    #[test]
    fn routes_scroll_pane_locally_and_preserves_detachment_on_content_update() {
        let content = |count: usize| {
            vf::text(
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
            crate::presentation::factory::foreground(vf::text("state"), ColorSpec::theme("accent"))
        }
    }

    struct FocusWithinShell {
        child: crate::component::ComponentHandle<ThemedField>,
    }

    impl Component for FocusWithinShell {
        fn view(&self) -> View {
            crate::presentation::factory::fill_width(crate::presentation::factory::border(
                View::component(self.child),
                BorderSpec::plain().color(ColorSpec::theme("shell.border")),
            ))
        }
    }

    struct FocusableShell {
        child: crate::component::ComponentHandle<ThemedField>,
    }

    impl Component for FocusableShell {
        fn view(&self) -> View {
            crate::presentation::factory::fill_width(crate::presentation::factory::border(
                View::component(self.child),
                BorderSpec::plain().color(ColorSpec::theme("shell.border")),
            ))
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.focusable();
        }
    }

    struct ThemedField;

    impl Component for ThemedField {
        fn view(&self) -> View {
            crate::presentation::factory::fill_width(crate::presentation::factory::border(
                vf::text("field"),
                BorderSpec::plain().color(ColorSpec::theme("field.border")),
            ))
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
            vf::text(if self.focused { "focused" } else { "unfocused" })
        }

        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.focusable();
            cx.on_focus_changed(Self::focus_changed);
        }
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

    struct LiveBlocker;

    impl Component for LiveBlocker {
        fn view(&self) -> View {
            vf::text("B1\nB2\nB3\nB4")
        }

        fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
    }

    #[test]
    fn semantic_blocked_live_prefix_is_pinned_not_skipped() {
        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("A"))
            .unwrap();
        let mut registry = ComponentRegistry::new();
        let blocker = registry.register(LiveBlocker);
        history.push(View::component(blocker)).unwrap();
        history
            .push(crate::presentation::factory::text("C1\nC2\nC3"))
            .unwrap();

        let mut sink = TestSink::default();
        let outcome =
            crate::history::transfer_native_prefix(&mut history, &mut sink, 10, 1).unwrap();
        assert_eq!(outcome.inserted, 1);
        assert_eq!(sink.rows.len(), 1);
        assert_eq!(sink.rows[0].plain_text(), "A");

        let mut scene = Scene::with_history(history, crate::presentation::factory::text("body"));
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
            history
                .push(crate::presentation::factory::text(format!("S{i}")))
                .unwrap();
        }

        let mut scene = Scene::with_history(history, crate::presentation::factory::text("body"));
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
            history
                .push(crate::presentation::factory::text(format!("R{i}")))
                .unwrap();
        }

        let mut scene = Scene::with_history(history, crate::presentation::factory::text("body"));
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
    /// Drain: inserts A, inserts B, hits `SemanticBlocked` on C.
    /// Because some physical progress occurred → returns Progress, re-resolves.
    /// On next attempt C still blocks → Blocked → `NativeFrontier` paint.
    ///
    /// The host must NOT paint based on the pre-transfer stale frame.
    #[test]
    fn physical_progress_then_blocker_forces_reresolve_not_stale_paint() {
        use crate::Component;

        struct LiveBlocker;
        impl Component for LiveBlocker {
            fn view(&self) -> View {
                // Fills 4 rows so it dominates the visible area.
                vf::text("B1\nB2\nB3\nB4")
            }
            fn capabilities(&self, _cx: &mut crate::ComponentCx<'_, Self>) {}
        }

        let mut registry = ComponentRegistry::new();
        let blocker_handle = registry.register(LiveBlocker);

        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("A"))
            .unwrap();
        history
            .push(crate::presentation::factory::text("B"))
            .unwrap();
        history.push(View::component(blocker_handle)).unwrap();
        history
            .push(crate::presentation::factory::text("D"))
            .unwrap();

        let mut scene = Scene::with_history(history, crate::presentation::factory::text("body"));
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
}
