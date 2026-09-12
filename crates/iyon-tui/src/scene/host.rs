//! Direct occurrence host for controls, focus, ticks, and physical paint.
//!
//! The occurrence document and the Taffy driver are the only general layout
//! route. This host owns interaction state and turns an immutable direct
//! layout product into a terminal surface; it does not resolve semantic
//! semantic UI recipes or maintain a parallel scene/cache/index tree.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use anyhow::{Result, anyhow};

use crate::{
    Theme,
    component::{
        ComponentId, ComponentRegistry, ControlSnapshot, MountGraph, MountedComponents,
        TickOutcome, TickScheduler,
    },
    geometry::{Rect, Size},
    interaction::{
        FocusState, InteractionResult, KeyStroke, MountedCapabilities, route_key_local,
        route_paste, route_paste_interceptor,
    },
    output::{OutputQueue, OutputRouter},
    physical::{PhysicalCell, PhysicalStyle, Surface, grapheme_cell_width},
    presentation::{
        ContentDirty, ContentProvider, StyleFacts, StyleStates,
        direct::{
            CapturedContentMeasurement, DirectDriverHandle, DirectHistoryAnchor, DirectLayout,
            paint_direct_layout,
        },
        direct_tree::ComponentGeometryMap,
        taffy::NodeParticipation,
    },
};

const MAX_LAYOUT_PASSES: usize = 8;

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
    invalidated_components: HashSet<ComponentId>,
    content_candidate_epoch: Option<u64>,
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
            invalidated_components: HashSet::new(),
            content_candidate_epoch: None,
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

    pub(crate) fn clear_direct_driver(&mut self) -> Result<()> {
        if let Some(mut driver) = self.direct_driver.take() {
            driver.shutdown()?;
        }
        self.direct_synchronized = false;
        self.direct_history_overflow_rows = 0;
        Ok(())
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
        driver.synchronize(
            snapshots,
            changes,
            participation.to_vec(),
            port_ids.clone(),
            roots,
            portal_owners,
            self.direct_controls.clone(),
        )?;
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
        self.direct_driver
            .as_ref()
            .map_or(Ok(()), |driver| driver.invalidate_content(port_id))
    }

    pub(crate) fn invalidate_direct_control_measurement(
        &mut self,
        component: ComponentId,
    ) -> Result<()> {
        self.direct_driver
            .as_ref()
            .map_or(Ok(()), |driver| driver.invalidate_control(component))
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
        let driver = self
            .direct_driver
            .as_ref()
            .ok_or_else(|| anyhow!("direct renderer driver is not started"))?;
        let mut captures = self.capture_direct_measurements(size.width, content)?;
        let mut control_snapshots = self.capture_direct_controls(registry)?;
        let mut invalidate_controls = Vec::new();
        for _ in 0..MAX_LAYOUT_PASSES {
            let mut direct = driver.layout_with_intrinsic(
                root,
                size,
                history_anchor,
                captures.clone(),
                invalidate_controls.clone(),
                control_snapshots.clone(),
                control_snapshots.clone(),
            )?;
            invalidate_controls.clear();
            let mut refined_content = Vec::new();
            for (key, capture) in &mut captures {
                let Some(width) = direct.content_widths.get(key).copied() else {
                    continue;
                };
                if !width.is_finite() || width < 0.0 {
                    return Err(anyhow!("direct content width is not finite"));
                }
                let width = width.floor().min(f32::from(u16::MAX)) as u16;
                if width == capture.offered_width {
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
            if !refined_content.is_empty() {
                direct = driver.layout_with_intrinsic(
                    root,
                    size,
                    history_anchor,
                    captures.clone(),
                    refined_content,
                    control_snapshots.clone(),
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
            let transitions = self.mounted.reconcile(graph.clone());
            self.ticker
                .sync_capabilities(&graph, &capabilities, &transitions, now);
            let geometry = direct.tree.component_geometry();
            if self
                .focus
                .reconcile_with_geometry(&graph, &capabilities, Some(&geometry), registry)
            {
                control_snapshots = self.capture_direct_controls(registry)?;
                invalidate_controls = self.direct_control_nodes.keys().copied().collect();
                continue;
            }
            let surface =
                paint_direct_layout(&direct, theme, content, self.focus.focused(), &graph)?;
            self.invalidated_components.clear();
            return Ok(PreparedSceneFrame {
                surface,
                component_geometry: geometry,
                occurrence_geometry: direct.occurrence_geometry,
            });
        }
        Err(anyhow!("direct occurrence layout did not converge"))
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

    pub(crate) fn invalidate_component(&mut self, id: ComponentId) {
        self.invalidated_components.insert(id);
    }
    pub(crate) fn has_invalidated_components(&self) -> bool {
        !self.invalidated_components.is_empty()
    }
    pub(crate) fn invalidate_content(&mut self, _dirty: ContentDirty) {}
    pub(crate) fn invalidate_root(&mut self) {}
    pub(crate) fn invalidate_theme(&mut self) {}
    pub(crate) fn discard_candidate(&mut self) {}
    pub(crate) fn clear_retained_views(&mut self) {}

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
