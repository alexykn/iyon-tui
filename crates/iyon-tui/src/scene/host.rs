//! Direct occurrence host for controls, focus, ticks, and physical paint.
//!
//! The occurrence document and the Taffy driver are the only general layout
//! route. This host owns interaction state and turns an immutable direct
//! layout product into a terminal surface; it does not resolve semantic
//! semantic UI recipes or maintain a parallel scene/cache/index tree.

use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::Arc,
    time::Instant,
};

use anyhow::{Result, anyhow};

use crate::{
    Theme,
    component::{
        ComponentId, ComponentRegistry, ControlSnapshot, MountGraph, MountedComponents,
        TickOutcome, TickScheduler,
    },
    geometry::Size,
    interaction::{
        FocusState, InteractionResult, KeyStroke, MountedCapabilities, route_key_local,
        route_paste, route_paste_interceptor,
    },
    output::{OutputQueue, OutputRouter},
    physical::Surface,
    presentation::{
        ContentProvider,
        direct::{CapturedContentMeasurement, DirectDriverHandle, DirectHistoryAnchor},
        direct_tree::ComponentGeometryMap,
        taffy::NodeParticipation,
    },
};

#[cfg(test)]
use crate::geometry::Rect;

const MAX_LAYOUT_PASSES: usize = 8;

pub(crate) struct SceneLayoutPending;

impl std::fmt::Display for SceneLayoutPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LAYOUT_PENDING: direct layout or paint is running")
    }
}

impl std::fmt::Debug for SceneLayoutPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SceneLayoutPending")
    }
}

impl std::error::Error for SceneLayoutPending {}

struct PendingPaint {
    signature: u64,
    component_geometry: ComponentGeometryMap,
    occurrence_geometry:
        HashMap<crate::occurrence::NodeKey, crate::presentation::direct_tree::ComponentGeometry>,
}

struct PendingLayoutInput {
    signature: u64,
    captures: HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
    controls: HashMap<ComponentId, ControlSnapshot>,
}

fn layout_request_signature(
    direct_revision: u64,
    root: crate::occurrence::NodeKey,
    size: Size,
    history_anchor: DirectHistoryAnchor,
    captures: &HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
    controls: &HashMap<ComponentId, ControlSnapshot>,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    direct_revision.hash(&mut hasher);
    root.hash(&mut hasher);
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);
    history_anchor.hash(&mut hasher);
    let mut capture_keys = captures.keys().copied().collect::<Vec<_>>();
    capture_keys.sort_unstable_by_key(|key| (key.slot, key.generation));
    for key in capture_keys {
        key.hash(&mut hasher);
        let capture = &captures[&key];
        capture.port_id.hash(&mut hasher);
        capture.measurement.source_id.hash(&mut hasher);
        capture.measurement.source_generation.hash(&mut hasher);
        capture.measurement.content_generation.hash(&mut hasher);
        capture.measurement.physically_complete.hash(&mut hasher);
        capture.measurement.intrinsic_size.hash(&mut hasher);
        capture.measurement.source_base.hash(&mut hasher);
        capture.measurement.source_end.hash(&mut hasher);
        capture.measurement.sealed.hash(&mut hasher);
        capture.measurement.head_partial.hash(&mut hasher);
    }
    let mut control_keys = controls.keys().copied().collect::<Vec<_>>();
    control_keys.sort_unstable();
    for key in control_keys {
        key.hash(&mut hasher);
        format!("{:?}", &controls[&key]).hash(&mut hasher);
    }
    hasher.finish()
}

fn captures_match_sources(
    current: &HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
    pending: &HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
) -> bool {
    current.len() == pending.len()
        && current.iter().all(|(key, capture)| {
            let Some(previous) = pending.get(key) else {
                return false;
            };
            capture.port_id == previous.port_id
                && capture.measurement.source_id == previous.measurement.source_id
                && capture.measurement.source_generation == previous.measurement.source_generation
                && capture.measurement.content_generation == previous.measurement.content_generation
                && capture.measurement.source_base == previous.measurement.source_base
                && capture.measurement.source_end == previous.measurement.source_end
                && capture.measurement.sealed == previous.measurement.sealed
                && capture.measurement.head_partial == previous.measurement.head_partial
                && capture.measurement.physically_complete
                    == previous.measurement.physically_complete
                && capture.measurement.connector_id == previous.measurement.connector_id
                && (!capture.measurement.physically_complete
                    || !previous.measurement.physically_complete
                    || capture.terminal_product.as_ref().map(|product| {
                        let size = product.size();
                        (size.width(), size.height())
                    }) == previous.terminal_product.as_ref().map(|product| {
                        let size = product.size();
                        (size.width(), size.height())
                    }))
        })
}

/// A fully synchronized occurrence/Taffy frame ready for the terminal
/// adapter. All fields are derived from the same candidate.
#[derive(Debug)]
pub(crate) struct PreparedSceneFrame {
    pub(crate) surface: Surface,
    pub(crate) component_geometry: ComponentGeometryMap,
    pub(crate) occurrence_geometry:
        HashMap<crate::occurrence::NodeKey, crate::presentation::direct_tree::ComponentGeometry>,
}

impl PreparedSceneFrame {
    pub(crate) fn screen_lines(&self) -> Vec<String> {
        (0..self.surface.height())
            .map(|y| {
                (0..self.surface.width())
                    .map(|x| self.surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                    .collect()
            })
            .collect()
    }
}

pub(crate) struct SceneHost {
    mounted: MountedComponents,
    focus: FocusState,
    ticker: TickScheduler,
    outputs: OutputQueue,
    graph: MountGraph,
    capabilities: MountedCapabilities,
    direct_driver: Option<DirectDriverHandle>,
    direct_controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    direct_content_ports: HashMap<crate::occurrence::NodeKey, crate::occurrence::ResourceKey>,
    direct_control_nodes: HashMap<crate::occurrence::NodeKey, crate::occurrence::ResourceKey>,
    direct_port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
    direct_body_root: Option<crate::occurrence::NodeKey>,
    direct_synchronized: bool,
    direct_history_overflow_rows: usize,
    direct_delivered_layout: HashMap<ComponentId, Size>,
    direct_delivered_content_extents: HashMap<ComponentId, Size>,
    invalidated_components: HashSet<ComponentId>,
    content_candidate_epoch: Option<u64>,
    async_wake: Arc<dyn Fn() + Send + Sync>,
    pending_layout_signature: Option<u64>,
    pending_paint: Option<PendingPaint>,
    pending_layout_input: Option<PendingLayoutInput>,
    pending_layout_refined: bool,
    direct_revision: u64,
    pending_content_invalidations: HashSet<u64>,
    pending_control_invalidations: HashSet<ComponentId>,
    pending_sync_revision: Option<u64>,
}

impl Default for SceneHost {
    fn default() -> Self {
        Self {
            mounted: MountedComponents::default(),
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
            direct_delivered_layout: HashMap::new(),
            direct_delivered_content_extents: HashMap::new(),
            invalidated_components: HashSet::new(),
            content_candidate_epoch: None,
            async_wake: Arc::new(|| {}),
            pending_layout_signature: None,
            pending_paint: None,
            pending_layout_input: None,
            pending_layout_refined: false,
            direct_revision: 0,
            pending_content_invalidations: HashSet::new(),
            pending_control_invalidations: HashSet::new(),
            pending_sync_revision: None,
        }
    }
}

impl SceneHost {
    pub(crate) fn is_mounted(&self, id: ComponentId) -> bool {
        self.graph.contains(id)
    }

    pub(crate) fn set_direct_control_component(
        &mut self,
        key: crate::occurrence::ResourceKey,
        component: ComponentId,
    ) {
        self.direct_controls.insert(key, component);
    }

    pub(crate) fn remove_direct_control_component(&mut self, key: crate::occurrence::ResourceKey) {
        self.direct_controls.remove(&key);
    }

    pub(crate) fn set_direct_driver_id(&mut self, driver_id: u64) -> Result<()> {
        if self.direct_driver.is_none() {
            self.direct_driver = Some(DirectDriverHandle::start(driver_id)?);
        }
        Ok(())
    }

    pub(crate) fn set_async_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.async_wake = wake;
    }

    #[cfg(test)]
    pub(crate) fn install_layout_latch_for_test(
        &self,
    ) -> Result<(std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>)> {
        self.direct_driver
            .as_ref()
            .ok_or_else(|| anyhow!("direct renderer driver is not started"))
            .map(DirectDriverHandle::install_layout_latch_for_test)
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_latch_for_test(&self) {
        if let Some(driver) = self.direct_driver.as_ref() {
            driver.clear_layout_latch_for_test();
        }
    }

    pub(crate) fn clear_direct_driver(&mut self) -> Result<()> {
        if let Some(mut driver) = self.take_direct_driver() {
            driver.shutdown()?;
        }
        Ok(())
    }

    pub(crate) fn take_direct_driver(&mut self) -> Option<DirectDriverHandle> {
        let driver = self.direct_driver.take();
        self.direct_synchronized = false;
        self.direct_revision = self.direct_revision.saturating_add(1);
        self.pending_layout_signature = None;
        self.pending_paint = None;
        self.pending_layout_input = None;
        self.pending_layout_refined = false;
        self.pending_content_invalidations.clear();
        self.pending_control_invalidations.clear();
        self.pending_sync_revision = None;
        self.direct_history_overflow_rows = 0;
        self.direct_delivered_layout.clear();
        self.direct_delivered_content_extents.clear();
        driver
    }

    pub(crate) fn direct_control_for_component(
        &self,
        component: ComponentId,
    ) -> Option<crate::occurrence::ResourceKey> {
        self.direct_controls
            .iter()
            .find_map(|(key, value)| (*value == component).then_some(*key))
    }

    pub(crate) fn direct_component_for_control(
        &self,
        control: crate::occurrence::ResourceKey,
    ) -> Option<ComponentId> {
        self.direct_controls.get(&control).copied()
    }

    pub(crate) fn sync_direct_occurrences(
        &mut self,
        sync_revision: u64,
        snapshots: Vec<crate::occurrence::OccurrenceSnapshot>,
        changes: Option<&crate::occurrence::UiChangeSet>,
        participation: &[NodeParticipation],
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<crate::occurrence::NodeKey>,
        body_root: crate::occurrence::NodeKey,
        portal_owners: HashMap<crate::occurrence::NodeKey, crate::occurrence::NodeKey>,
    ) -> Result<()> {
        let driver = self
            .direct_driver
            .as_ref()
            .ok_or_else(|| anyhow!("direct renderer driver is not started"))?;
        let synchronization_complete = if let Some(pending_revision) = self.pending_sync_revision {
            match driver.poll_synchronize() {
                Ok(None) => return Err(anyhow::Error::new(SceneLayoutPending)),
                Ok(Some(())) if pending_revision == sync_revision => {
                    self.pending_sync_revision = None;
                    true
                }
                Ok(Some(())) => {
                    self.pending_sync_revision = None;
                    false
                }
                Err(error) => {
                    self.pending_sync_revision = None;
                    return Err(error);
                }
            }
        } else {
            false
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
                            && !crate::presentation::direct::snapshot_display_none(snapshot)
                            && port_ids.contains_key(port)
                    })
                    .map(|port| (snapshot.key, port))
            })
            .collect::<HashMap<_, _>>();
        let control_nodes = snapshots
            .iter()
            .filter_map(|snapshot| snapshot.control.map(|control| (snapshot.key, control)))
            .collect::<HashMap<_, _>>();
        if !synchronization_complete {
            driver.request_synchronize(
                snapshots,
                changes,
                participation.to_vec(),
                port_ids.clone(),
                roots,
                portal_owners,
                self.direct_controls.clone(),
            )?;
            self.pending_sync_revision = Some(sync_revision);
            return Err(anyhow::Error::new(SceneLayoutPending));
        }
        if changes.is_none() || !self.direct_synchronized {
            self.direct_content_ports = content_ports;
            self.direct_control_nodes = control_nodes;
        } else {
            let changed = changes.expect("checked change set");
            for key in changed
                .changed_nodes
                .iter()
                .chain(changed.membership_nodes.iter())
                .chain(changed.retired_nodes.iter())
            {
                self.direct_content_ports.remove(key);
                self.direct_control_nodes.remove(key);
            }
            self.direct_content_ports.extend(content_ports);
            self.direct_control_nodes.extend(control_nodes);
        }
        self.direct_port_ids = port_ids;
        self.direct_body_root = Some(body_root);
        self.direct_synchronized = true;
        self.direct_revision = self.direct_revision.saturating_add(1);
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
        self.pending_content_invalidations.insert(port_id);
        Ok(())
    }

    pub(crate) fn invalidate_direct_control_measurement(
        &mut self,
        component: ComponentId,
    ) -> Result<()> {
        self.pending_control_invalidations.insert(component);
        Ok(())
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
        if self.direct_driver.is_none() {
            return Err(anyhow!("direct renderer driver is not started"));
        }
        let mut captures = {
            #[cfg(feature = "perf-counters")]
            let _perf_timer =
                crate::perf::ScopedTimer::new(crate::perf::Counter::DirectCaptureNanos);
            self.capture_direct_measurements(size.width, content)?
        };
        let mut control_snapshots = self.capture_direct_controls(registry)?;
        let current_signature = layout_request_signature(
            self.direct_revision,
            root,
            size,
            history_anchor,
            &captures,
            &control_snapshots,
        );
        let mut invalidate_content = self
            .pending_layout_input
            .as_ref()
            .filter(|pending| pending.signature != current_signature)
            .map_or_else(Vec::new, |_| captures.keys().copied().collect());
        let reuse_pending_captures = self.pending_layout_input.as_ref().is_some_and(|pending| {
            pending.signature == current_signature
                || (self.pending_layout_refined
                    && captures_match_sources(&captures, &pending.captures))
        });
        if reuse_pending_captures {
            let pending = self
                .pending_layout_input
                .as_ref()
                .expect("pending layout input exists when reuse is selected");
            // A Fit probe may be refined to the actual allocated width before
            // the worker request. Reuse that immutable request capture on the
            // next queue turn instead of reconstructing a new attempt with a
            // fresh capture id/fit width and rejecting the ready result.
            captures = pending.captures.clone();
            control_snapshots = pending.controls.clone();
        }
        let signature = layout_request_signature(
            self.direct_revision,
            root,
            size,
            history_anchor,
            &captures,
            &control_snapshots,
        );
        if let Some(pending) = self.pending_paint.take() {
            let driver = self
                .direct_driver
                .as_ref()
                .expect("direct renderer driver checked above");
            match driver.poll_paint()? {
                None => {
                    self.pending_paint = Some(pending);
                    return Err(anyhow::Error::new(SceneLayoutPending));
                }
                Some(surface) if pending.signature == signature => {
                    self.invalidated_components.clear();
                    return Ok(PreparedSceneFrame {
                        surface,
                        component_geometry: pending.component_geometry,
                        occurrence_geometry: pending.occurrence_geometry,
                    });
                }
                Some(_) => {
                    // The immutable paint finished for an older control or
                    // content capture. Drop it and prepare a fresh request;
                    // no stale completion can become a visible candidate.
                }
            }
        }
        let mut invalidate_controls = Vec::new();
        for _ in 0..MAX_LAYOUT_PASSES {
            let mut invalidate = invalidate_content.clone();
            invalidate.extend(invalidate_controls.iter().copied());
            let mut direct = self.layout_or_pending(
                root,
                size,
                history_anchor,
                captures.clone(),
                invalidate,
                control_snapshots.clone(),
            )?;
            invalidate_content.clear();
            invalidate_controls.clear();
            let mut refined_content = Vec::new();
            #[cfg(feature = "perf-counters")]
            let _refinement_timer =
                crate::perf::ScopedTimer::new(crate::perf::Counter::DirectRefinementNanos);
            for (key, capture) in &mut captures {
                if capture.measurement.projection_identity == 0 {
                    // Explicit loading captures have no width realization to
                    // refine. Keeping their bounded placeholder metrics lets
                    // unrelated controls complete without repeatedly
                    // resubmitting a width-zero pseudo-product.
                    continue;
                }
                let Some(width) = direct.content_widths.get(key).copied() else {
                    continue;
                };
                if !width.is_finite() || width < 0.0 {
                    return Err(anyhow!("direct content width is not finite"));
                }
                let width = width.floor().min(f32::from(u16::MAX)) as u16;
                if width == capture.offered_width && capture.measurement.physically_complete {
                    continue;
                }
                let next = content.refine_captured_measurement(
                    capture.port_id,
                    capture.capture_id,
                    width,
                    crate::presentation::ContentWidthRule::Fill,
                )?;
                capture.measurement = next.measurement;
                capture.min_content = next.min_content;
                capture.max_content = next.max_content;
                capture.history_adjustment = next.history_adjustment;
                capture.semantic_contents = next.semantic_contents;
                capture.terminal_policy = next.terminal_policy;
                capture.terminal_product = next.terminal_product;
                capture.offered_width = width;
                refined_content.push(*key);
            }
            #[cfg(feature = "perf-counters")]
            drop(_refinement_timer);
            if !refined_content.is_empty() {
                self.pending_layout_refined = true;
                if let Some(pending) = self.pending_layout_input.as_mut() {
                    pending.captures = captures.clone();
                }
                direct = self.layout_or_pending(
                    root,
                    size,
                    history_anchor,
                    captures.clone(),
                    refined_content,
                    control_snapshots.clone(),
                )?;
            }
            self.direct_history_overflow_rows = direct.history_overflow_rows;
            let mounts = direct.component_mounts.clone();
            let mount_nodes = mounts
                .iter()
                .map(|(id, parent)| {
                    let snapshot = registry
                        .resolution(*id)
                        .ok_or_else(|| anyhow!("direct mounted component disappeared"))?;
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
            self.direct_delivered_layout
                .retain(|id, _| self.graph.contains(*id));
            self.direct_delivered_content_extents
                .retain(|id, _| self.graph.contains(*id));
            let transitions = self.mounted.reconcile(graph.clone());
            self.ticker
                .sync_capabilities(&graph, &capabilities, &transitions, now);
            let geometry = direct.tree.component_geometry();
            if self.synchronize_direct_control_feedback(&geometry, registry)? {
                control_snapshots = self.capture_direct_controls(registry)?;
                invalidate_controls = self.direct_control_nodes.keys().copied().collect();
                continue;
            }
            if self
                .focus
                .reconcile_with_geometry(&graph, &capabilities, Some(&geometry), registry)
            {
                control_snapshots = self.capture_direct_controls(registry)?;
                invalidate_controls = self.direct_control_nodes.keys().copied().collect();
                continue;
            }
            let signature = layout_request_signature(
                self.direct_revision,
                root,
                size,
                history_anchor,
                &captures,
                &control_snapshots,
            );
            let component_geometry = geometry;
            let occurrence_geometry = direct.occurrence_geometry.clone();
            let driver = self
                .direct_driver
                .as_ref()
                .expect("direct renderer driver checked above");
            driver.request_paint(
                direct,
                Arc::new(theme.clone()),
                self.focus.focused(),
                graph,
                Arc::clone(&self.async_wake),
            )?;
            self.pending_paint = Some(PendingPaint {
                signature,
                component_geometry,
                occurrence_geometry,
            });
            return Err(anyhow::Error::new(SceneLayoutPending));
        }
        Err(anyhow!("direct occurrence layout did not converge"))
    }

    fn layout_or_pending(
        &mut self,
        root: crate::occurrence::NodeKey,
        size: Size,
        history_anchor: DirectHistoryAnchor,
        measurements: HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>,
        invalidate: Vec<crate::occurrence::NodeKey>,
        controls: HashMap<ComponentId, ControlSnapshot>,
    ) -> Result<crate::presentation::direct::DirectLayout> {
        let mut invalidate = invalidate;
        let content_ports = self.pending_content_invalidations.clone();
        invalidate.extend(
            self.direct_content_ports
                .iter()
                .filter_map(|(node, resource)| {
                    self.direct_port_ids
                        .get(resource)
                        .filter(|port_id| content_ports.contains(port_id))
                        .map(|_| *node)
                }),
        );
        let control_components = self.pending_control_invalidations.clone();
        invalidate.extend(
            self.direct_control_nodes
                .iter()
                .filter_map(|(node, resource)| {
                    self.direct_controls
                        .get(resource)
                        .filter(|component| control_components.contains(component))
                        .map(|_| *node)
                }),
        );
        invalidate.sort_unstable_by_key(|key| (key.slot, key.generation));
        invalidate.dedup();
        let signature = layout_request_signature(
            self.direct_revision,
            root,
            size,
            history_anchor,
            &measurements,
            &controls,
        );
        let driver = self
            .direct_driver
            .as_ref()
            .expect("direct renderer driver checked above");
        if self.pending_layout_signature == Some(signature) {
            return match driver.poll_layout() {
                Ok(Some(layout)) => {
                    self.pending_layout_signature = None;
                    self.pending_layout_input = None;
                    self.pending_layout_refined = false;
                    Ok(layout)
                }
                Ok(None) => Err(anyhow::Error::new(SceneLayoutPending)),
                Err(error) => {
                    self.pending_layout_signature = None;
                    self.pending_layout_input = None;
                    self.pending_layout_refined = false;
                    Err(error)
                }
            };
        }
        if self.pending_layout_signature.is_some() {
            match driver.poll_layout() {
                Ok(None) => return Err(anyhow::Error::new(SceneLayoutPending)),
                Ok(Some(_)) => {}
                Err(error) => {
                    self.pending_layout_signature = None;
                    self.pending_layout_input = None;
                    self.pending_layout_refined = false;
                    return Err(error);
                }
            }
            self.pending_layout_signature = None;
            self.pending_layout_input = None;
        }
        let pending_input = PendingLayoutInput {
            signature,
            captures: measurements.clone(),
            controls: controls.clone(),
        };
        driver.request_layout(
            root,
            size,
            history_anchor,
            measurements,
            invalidate,
            controls,
            Arc::clone(&self.async_wake),
        )?;
        self.pending_content_invalidations.clear();
        self.pending_control_invalidations.clear();
        self.pending_layout_signature = Some(signature);
        self.pending_layout_input = Some(pending_input);
        Err(anyhow::Error::new(SceneLayoutPending))
    }

    fn capture_direct_measurements(
        &self,
        width: u16,
        content: &mut dyn ContentProvider,
    ) -> Result<HashMap<crate::occurrence::NodeKey, CapturedContentMeasurement>> {
        self.direct_content_ports
            .iter()
            .map(|(node, resource)| {
                let port_id = self
                    .direct_port_ids
                    .get(resource)
                    .copied()
                    .ok_or_else(|| anyhow!("direct ContentPort is not installed"))?;
                let capture = content.capture_measurement(
                    port_id,
                    width,
                    crate::presentation::ContentWidthRule::Fit,
                )?;
                Ok((
                    *node,
                    CapturedContentMeasurement {
                        capture_id: capture.capture_id,
                        port_id,
                        offered_width: width,
                        measurement: capture.measurement,
                        min_content: capture.min_content,
                        max_content: capture.max_content,
                        history_adjustment: capture.history_adjustment,
                        semantic_contents: capture.semantic_contents,
                        terminal_policy: capture.terminal_policy,
                        terminal_product: capture.terminal_product,
                    },
                ))
            })
            .collect()
    }

    fn capture_direct_controls(
        &self,
        registry: &ComponentRegistry,
    ) -> Result<HashMap<ComponentId, ControlSnapshot>> {
        self.direct_controls
            .values()
            .copied()
            .map(|component| {
                let snapshot = registry
                    .resolution(component)
                    .ok_or_else(|| anyhow!("direct control component disappeared"))?;
                let control = snapshot
                    .control
                    .ok_or_else(|| anyhow!("direct control has no concrete snapshot"))?;
                Ok((component, control))
            })
            .collect()
    }

    /// Delivers the physical candidate geometry to controls that explicitly
    /// requested it. A changed callback mutates the mounted control state, so
    /// the caller must recapture the immutable control snapshots and run a
    /// bounded layout pass before painting this candidate.
    fn synchronize_direct_control_feedback(
        &mut self,
        geometry: &ComponentGeometryMap,
        registry: &mut ComponentRegistry,
    ) -> Result<bool> {
        let mut dirty = false;
        for id in self.graph.ids() {
            let entry = geometry
                .entries
                .get(&id)
                .ok_or_else(|| anyhow!("mounted component has no direct geometry"))?;
            let size = entry.content.size();
            let layout_handler = self
                .capabilities
                .get(id)
                .and_then(|caps| caps.layout_changed.as_ref())
                .cloned();
            if let Some(handler) = layout_handler {
                if self.direct_delivered_layout.get(&id).copied() != Some(size) {
                    self.direct_delivered_layout.insert(id, size);
                    registry
                        .with_any_mut(id, |component| handler(component, size))
                        .ok_or_else(|| {
                            anyhow!("mounted component disappeared during layout feedback")
                        })?;
                    dirty = true;
                }
            } else {
                self.direct_delivered_layout.remove(&id);
            }

            let extent = geometry.content_extents.get(&id).copied();
            let extent_handler = self
                .capabilities
                .get(id)
                .and_then(|caps| caps.content_extent_changed.as_ref())
                .cloned();
            if let Some(handler) = extent_handler {
                if let Some(extent) = extent {
                    if self.direct_delivered_content_extents.get(&id).copied() != Some(extent) {
                        self.direct_delivered_content_extents.insert(id, extent);
                        registry
                            .with_any_mut(id, |component| handler(component, extent))
                            .ok_or_else(|| {
                                anyhow!("mounted component disappeared during extent feedback")
                            })?;
                        dirty = true;
                    }
                } else {
                    self.direct_delivered_content_extents.remove(&id);
                }
            } else {
                self.direct_delivered_content_extents.remove(&id);
            }
        }
        Ok(dirty)
    }

    pub(crate) fn invalidate_component(&mut self, id: ComponentId) {
        self.invalidated_components.insert(id);
    }
    pub(crate) fn has_invalidated_components(&self) -> bool {
        !self.invalidated_components.is_empty()
    }
    pub(crate) fn focus_component(
        &mut self,
        id: ComponentId,
        geometry: &ComponentGeometryMap,
        registry: &mut ComponentRegistry,
    ) -> bool {
        self.focus.focus_component(
            id,
            &self.graph,
            &self.capabilities,
            Some(geometry),
            registry,
        )
    }
    pub(crate) fn focused_component(&self) -> Option<ComponentId> {
        self.focus.focused()
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
        mut intercept: impl FnMut(ComponentId, &str) -> Option<A>,
    ) -> Option<A> {
        route_paste_interceptor(text, &self.focus, &self.graph, |id, value| {
            intercept(id, value)
        })
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
    pub(crate) fn drain_outputs<A>(&mut self, router: &OutputRouter<A>) -> Result<Vec<A>>
    where
        A: Send + 'static,
    {
        router
            .drain(&mut self.outputs)
            .map_err(|error| anyhow!(error.to_string()))
    }
    pub(crate) fn tick_due(
        &mut self,
        now: Instant,
        registry: &mut ComponentRegistry,
    ) -> TickOutcome {
        self.ticker
            .tick_due_with_events(now, registry, &mut self.outputs)
    }
    pub(crate) fn next_tick_deadline(&self) -> Option<Instant> {
        self.ticker.next_deadline()
    }
    pub(crate) fn content_candidate_epoch(&self) -> Option<u64> {
        self.content_candidate_epoch
    }
    pub(crate) fn commit_content_candidate(&mut self, epoch: u64) {
        self.content_candidate_epoch = Some(epoch);
    }
    pub(crate) fn abort_content_candidate(&mut self) {
        self.content_candidate_epoch = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Component, ComponentCx, component::MountNode};

    #[derive(Default)]
    struct GeometryAware {
        layouts: Vec<Size>,
        extents: Vec<Size>,
    }

    impl Component for GeometryAware {
        fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
            cx.on_layout_changed(Self::layout_changed);
            cx.on_content_extent_changed(Self::extent_changed);
        }
    }

    impl GeometryAware {
        fn layout_changed(component: &mut Self, size: Size) {
            component.layouts.push(size);
        }

        fn extent_changed(component: &mut Self, size: Size) {
            component.extents.push(size);
        }
    }

    fn geometry(size: Size, extent: Size, id: ComponentId) -> ComponentGeometryMap {
        ComponentGeometryMap {
            entries: HashMap::from([(
                id,
                crate::presentation::direct_tree::ComponentGeometry {
                    outer: Rect::new(0, 0, size.width, size.height),
                    content: Rect::new(0, 0, size.width, size.height),
                    visible: Some(Rect::new(0, 0, size.width, size.height)),
                },
            )]),
            content_extents: HashMap::from([(id, extent)]),
        }
    }

    #[test]
    fn direct_candidate_delivers_layout_and_extent_once_per_value() {
        let mut registry = ComponentRegistry::new();
        let handle = registry.register(GeometryAware::default());
        let id = handle.id();
        let mut host = SceneHost::default();
        host.graph = MountGraph::new(vec![MountNode {
            id,
            parent: None,
            revision: Default::default(),
        }]);
        let snapshot = registry
            .resolution(id)
            .expect("geometry-aware component remains registered");
        host.capabilities.insert(id, snapshot.capabilities);

        assert!(
            host.synchronize_direct_control_feedback(
                &geometry(Size::new(8, 2), Size::new(8, 12), id),
                &mut registry,
            )
            .expect("first candidate feedback")
        );
        assert!(
            !host
                .synchronize_direct_control_feedback(
                    &geometry(Size::new(8, 2), Size::new(8, 12), id),
                    &mut registry,
                )
                .expect("unchanged candidate feedback")
        );
        assert!(
            host.synchronize_direct_control_feedback(
                &geometry(Size::new(7, 2), Size::new(8, 13), id),
                &mut registry,
            )
            .expect("changed candidate feedback")
        );

        let facts = registry
            .with(handle, |component| {
                (component.layouts.clone(), component.extents.clone())
            })
            .expect("geometry-aware component remains registered");
        assert_eq!(facts.0, vec![Size::new(8, 2), Size::new(7, 2)]);
        assert_eq!(facts.1, vec![Size::new(8, 12), Size::new(8, 13)]);
    }

    #[test]
    fn layout_signature_ignores_per_attempt_capture_ids() {
        let key = crate::occurrence::NodeKey {
            slot: 1,
            generation: 1,
        };
        let capture = CapturedContentMeasurement {
            capture_id: 1,
            port_id: 7,
            offered_width: 8,
            measurement: crate::presentation::ContentMeasurement::default(),
            min_content: Size::new(1, 1),
            max_content: Size::new(8, 2),
            history_adjustment: None,
            semantic_contents: None,
            terminal_policy: crate::text::TextRenderPolicy::default(),
            terminal_product: None,
        };
        let mut next = capture.clone();
        next.capture_id = 2;
        let first = HashMap::from([(key, capture)]);
        let second = HashMap::from([(key, next)]);
        assert_eq!(
            layout_request_signature(
                1,
                key,
                Size::new(8, 2),
                DirectHistoryAnchor::FollowEnd,
                &first,
                &HashMap::new(),
            ),
            layout_request_signature(
                1,
                key,
                Size::new(8, 2),
                DirectHistoryAnchor::FollowEnd,
                &second,
                &HashMap::new(),
            )
        );
        let mut replacement = second.clone();
        replacement
            .get_mut(&key)
            .expect("capture exists")
            .measurement
            .source_end = 9;
        assert_ne!(
            layout_request_signature(
                1,
                key,
                Size::new(8, 2),
                DirectHistoryAnchor::FollowEnd,
                &first,
                &HashMap::new(),
            ),
            layout_request_signature(
                1,
                key,
                Size::new(8, 2),
                DirectHistoryAnchor::FollowEnd,
                &replacement,
                &HashMap::new(),
            )
        );
    }

    #[test]
    fn direct_host_keeps_input_invalidation_nonblocking_during_layout() {
        let mut host = SceneHost::default();
        host.set_direct_driver_id(17)
            .expect("direct driver startup");
        let root = crate::occurrence::NodeKey {
            slot: 1,
            generation: 1,
        };
        let snapshot = crate::occurrence::OccurrenceSnapshot {
            key: root,
            kind: crate::occurrence::HostKind::Box,
            root_role: Some(crate::occurrence::RootRole::Body),
            children: Vec::new(),
            port: None,
            control: None,
            hidden: false,
            subscriptions: 0,
            history_action: None,
            properties: Vec::new(),
            style_states: Vec::new(),
            structure_revision: 1,
            geometry_revision: 1,
            presentation_revision: 0,
            interaction_revision: 0,
        };
        let synchronize = host.sync_direct_occurrences(
            1,
            vec![snapshot.clone()],
            None,
            &[NodeParticipation {
                key: root,
                participates: true,
            }],
            HashMap::new(),
            vec![root],
            root,
            HashMap::new(),
        );
        assert!(synchronize.is_err(), "initial sync is asynchronous");
        for _ in 0..1000 {
            let synchronize = host.sync_direct_occurrences(
                1,
                vec![snapshot.clone()],
                None,
                &[NodeParticipation {
                    key: root,
                    participates: true,
                }],
                HashMap::new(),
                vec![root],
                root,
                HashMap::new(),
            );
            if synchronize.is_ok() {
                break;
            }
            std::thread::yield_now();
        }
        assert!(host.has_direct_occurrences());

        let (entered, release) = host
            .install_layout_latch_for_test()
            .expect("layout latch installation");
        let mut registry = ComponentRegistry::new();
        let mut content = crate::presentation::content::EmptyContentProvider;
        let prepare = host.prepare_direct_at_with_content(
            Instant::now(),
            root,
            Size::new(8, 2),
            DirectHistoryAnchor::FollowEnd,
            &mut registry,
            &Theme::default(),
            &mut content,
            &HashMap::new(),
        );
        assert!(prepare.is_err(), "layout request is asynchronous");
        entered
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("layout worker entered latch");
        host.invalidate_direct_control_measurement(ComponentId::from_raw(1))
            .expect("input invalidation must not wait for layout");
        let prepare = host.prepare_direct_at_with_content(
            Instant::now(),
            root,
            Size::new(8, 2),
            DirectHistoryAnchor::FollowEnd,
            &mut registry,
            &Theme::default(),
            &mut content,
            &HashMap::new(),
        );
        assert!(prepare.is_err(), "latched layout remains pending");
        release.send(()).expect("release layout worker");
        host.clear_layout_latch_for_test();
        let mut prepared = false;
        for _ in 0..1000 {
            let prepare = host.prepare_direct_at_with_content(
                Instant::now(),
                root,
                Size::new(8, 2),
                DirectHistoryAnchor::FollowEnd,
                &mut registry,
                &Theme::default(),
                &mut content,
                &HashMap::new(),
            );
            if prepare.is_ok() {
                prepared = true;
                break;
            }
            std::thread::yield_now();
        }
        assert!(prepared, "layout and paint complete after release");
        host.clear_direct_driver().expect("direct driver shutdown");
    }
}
