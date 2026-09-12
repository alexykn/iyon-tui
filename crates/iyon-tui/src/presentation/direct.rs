//! Direct occurrence rendering over the derived Taffy layout.
//!
//! The occurrence document is the semantic owner. This module is the
//! renderer-driver side of that contract: a dedicated owner thread keeps the
//! thread-affine Taffy tree, accepts owned snapshots/products, and returns an
//! owned layout candidate. It never acquires `HostInner`, calls Source
//! selection, or invokes a content callback while laying out.

use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
    sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use anyhow::{Result, anyhow};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    component::{ComponentId, ControlSnapshot, EditorSnapshot},
    geometry::Rect,
    occurrence::{
        HostKind, LayerValue, NodeKey, OccurrenceSnapshot, PropertyId, PropertyValue, UiChangeSet,
    },
    physical::{PhysicalCell, grapheme_cell_width},
    presentation::{
        BorderSpec, BorderStyle, ContentMeasurement, StyleSpec, StyleStateKey, StyleStateValue,
        TextAttribute,
    },
    text::{TerminalConstraints, TerminalTextProduct, TextContent, TextRenderPolicy},
};

use super::content::HistoryMeasurementAdjustment;
use super::direct_tree::{
    ComponentGeometry, DirectContent, DirectDecoration, DirectNode, DirectNodeId, DirectTree,
};
use super::taffy::{AvailableConstraint, MeasuredSize, NodeParticipation, TaffyLayoutAdapter};

/// A measurement captured by the host before a request crosses the driver
/// boundary. The driver owns no Source/Connector state and only reads this
/// immutable product.
#[derive(Clone, Debug)]
pub(crate) struct CapturedContentMeasurement {
    pub(crate) capture_id: u64,
    pub(crate) port_id: u64,
    pub(crate) offered_width: u16,
    pub(crate) measurement: ContentMeasurement,
    pub(crate) min_content: crate::geometry::Size,
    pub(crate) max_content: crate::geometry::Size,
    pub(crate) history_adjustment: Option<HistoryMeasurementAdjustment>,
    pub(crate) semantic_contents: Option<std::sync::Arc<[TextContent]>>,
    pub(crate) terminal_policy: TextRenderPolicy,
    pub(crate) terminal_product: Option<std::sync::Arc<TerminalTextProduct>>,
}

impl PartialEq for CapturedContentMeasurement {
    fn eq(&self, other: &Self) -> bool {
        self.capture_id == other.capture_id
            && self.port_id == other.port_id
            && self.offered_width == other.offered_width
            && self.measurement == other.measurement
            && self.min_content == other.min_content
            && self.max_content == other.max_content
            && self.history_adjustment == other.history_adjustment
            && match (&self.semantic_contents, &other.semantic_contents) {
                (None, None) => true,
                (Some(left), Some(right)) => std::sync::Arc::ptr_eq(left, right),
                _ => false,
            }
            && self.terminal_policy == other.terminal_policy
            && match (&self.terminal_product, &other.terminal_product) {
                (None, None) => true,
                (Some(left), Some(right)) => std::sync::Arc::ptr_eq(left, right),
                _ => false,
            }
    }
}

/// Direct candidate output. The scene host adds physical receipt metadata.
#[derive(Clone, Debug)]
pub(crate) struct DirectLayout {
    pub(crate) tree: DirectTree,
    pub(crate) occurrence_geometry: HashMap<NodeKey, ComponentGeometry>,
    pub(crate) content_widths: HashMap<NodeKey, f32>,
    pub(crate) content_products: HashMap<NodeKey, CapturedContentMeasurement>,
    pub(crate) component_mounts: Vec<(ComponentId, Option<ComponentId>)>,
    pub(crate) history_overflow_rows: usize,
}

#[cfg(test)]
struct TestLayoutLatch {
    entered: std::sync::mpsc::Sender<()>,
    release: Arc<Mutex<std::sync::mpsc::Receiver<()>>>,
}

/// Controls the placement of the active History suffix after a native
/// transfer.  FollowEnd keeps the current suffix bottom anchored; a blocked
/// native frontier must instead pin the active rows at the top of the
/// History track so the screen does not follow rows that were not accepted.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DirectHistoryAnchor {
    FollowEnd,
    NativeFrontier,
}

enum DirectDriverCommand {
    Synchronize {
        snapshots: Vec<OccurrenceSnapshot>,
        changes: Option<Box<UiChangeSet>>,
        participation: Vec<NodeParticipation>,
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<NodeKey>,
        portal_owners: HashMap<NodeKey, NodeKey>,
        controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
        response: SyncSender<Result<()>>,
    },
    Layout {
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: Vec<NodeKey>,
        controls: HashMap<ComponentId, ControlSnapshot>,
        response: SyncSender<Result<DirectLayout>>,
        wake: Arc<dyn Fn() + Send + Sync>,
        #[cfg(test)]
        latch: Arc<Mutex<Option<TestLayoutLatch>>>,
    },
    Paint {
        layout: DirectLayout,
        theme: Arc<crate::Theme>,
        focused: Option<ComponentId>,
        graph: crate::component::MountGraph,
        response: SyncSender<Result<crate::physical::Surface>>,
        wake: Arc<dyn Fn() + Send + Sync>,
    },
    InvalidateContent {
        port_id: u64,
        response: SyncSender<Result<()>>,
    },
    InvalidateControl {
        component: ComponentId,
        response: SyncSender<Result<()>>,
    },
}

/// Send-only handle for the one concrete renderer driver. Taffy is created,
/// used, and destroyed by the worker; this handle is the only part retained
/// by the Send-safe host owner.
pub(crate) struct DirectDriverHandle {
    command: SyncSender<DirectDriverCommand>,
    join: Option<JoinHandle<()>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    pending_layout: Mutex<Option<Receiver<Result<DirectLayout>>>>,
    pending_paint: Mutex<Option<Receiver<Result<crate::physical::Surface>>>>,
    pending_synchronize: Mutex<Option<Receiver<Result<()>>>>,
    #[cfg(test)]
    layout_latch: Arc<Mutex<Option<TestLayoutLatch>>>,
}

impl DirectDriverHandle {
    pub(crate) fn start(host_id: u64) -> Result<Self> {
        let (command, receive) = sync_channel(8);
        let (ready_send, ready_receive) = sync_channel(1);
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_for_worker = Arc::clone(&shutdown);
        let join = thread::Builder::new()
            .name(format!("iyon-tui-layout-{host_id}"))
            .spawn(move || direct_driver_loop(receive, ready_send, host_id, shutdown_for_worker))
            .map_err(|error| anyhow!("direct renderer driver startup failed: {error}"))?;
        if ready_receive
            .recv()
            .map_err(|_| anyhow!("direct renderer driver exited during startup"))?
            .is_err()
        {
            let _ = join.join();
            return Err(anyhow!("direct renderer driver startup failed"));
        }
        Ok(Self {
            command,
            join: Some(join),
            shutdown,
            pending_layout: Mutex::new(None),
            pending_paint: Mutex::new(None),
            pending_synchronize: Mutex::new(None),
            #[cfg(test)]
            layout_latch: Arc::new(Mutex::new(None)),
        })
    }

    pub(crate) fn request_synchronize(
        &self,
        snapshots: Vec<OccurrenceSnapshot>,
        changes: Option<&UiChangeSet>,
        participation: Vec<NodeParticipation>,
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<NodeKey>,
        portal_owners: HashMap<NodeKey, NodeKey>,
        controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    ) -> Result<()> {
        let mut pending = self
            .pending_synchronize
            .lock()
            .map_err(|_| anyhow!("direct synchronization pending lock is poisoned"))?;
        if pending.is_some() {
            return Err(anyhow!("direct synchronization request is already pending"));
        }
        let (response, receive) = sync_channel(1);
        self.command
            .try_send(DirectDriverCommand::Synchronize {
                snapshots,
                changes: changes.map(|changes| Box::new(changes.clone())),
                participation,
                port_ids,
                roots,
                portal_owners,
                controls,
                response,
            })
            .map_err(|_| anyhow!("direct renderer driver is closed"))?;
        *pending = Some(receive);
        Ok(())
    }

    pub(crate) fn poll_synchronize(&self) -> Result<Option<()>> {
        let mut pending = self
            .pending_synchronize
            .lock()
            .map_err(|_| anyhow!("direct synchronization pending lock is poisoned"))?;
        let Some(receive) = pending.take() else {
            return Ok(Some(()));
        };
        match receive.try_recv() {
            Ok(result) => result.map(Some),
            Err(TryRecvError::Empty) => {
                *pending = Some(receive);
                Ok(None)
            }
            Err(TryRecvError::Disconnected) => {
                Err(anyhow!("direct renderer driver dropped synchronization"))
            }
        }
    }

    pub(crate) fn request_layout(
        &self,
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: Vec<NodeKey>,
        controls: HashMap<ComponentId, ControlSnapshot>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(anyhow!("direct renderer driver is closed"));
        }
        let mut pending = self
            .pending_layout
            .lock()
            .map_err(|_| anyhow!("direct layout pending lock is poisoned"))?;
        if pending.is_some() {
            return Err(anyhow!("direct layout request is already pending"));
        }
        let (response, receive) = sync_channel(1);
        #[cfg(test)]
        let latch = Arc::clone(&self.layout_latch);
        self.command
            .try_send(DirectDriverCommand::Layout {
                root,
                size,
                history_anchor,
                measurements,
                invalidate,
                controls,
                response,
                wake,
                #[cfg(test)]
                latch,
            })
            .map_err(|_| anyhow!("direct renderer driver queue is full or closed"))?;
        *pending = Some(receive);
        Ok(())
    }

    pub(crate) fn poll_layout(&self) -> Result<Option<DirectLayout>> {
        let mut pending = self
            .pending_layout
            .lock()
            .map_err(|_| anyhow!("direct layout pending lock is poisoned"))?;
        let Some(receive) = pending.take() else {
            return Ok(None);
        };
        match receive.try_recv() {
            Ok(result) => result.map(Some),
            Err(TryRecvError::Empty) => {
                *pending = Some(receive);
                Ok(None)
            }
            Err(TryRecvError::Disconnected) => {
                Err(anyhow!("direct renderer driver dropped layout"))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn install_layout_latch_for_test(
        &self,
    ) -> (std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>) {
        let (entered, entered_receive) = std::sync::mpsc::channel();
        let (release, release_receive) = std::sync::mpsc::channel();
        *self
            .layout_latch
            .lock()
            .expect("layout test latch lock must remain usable") = Some(TestLayoutLatch {
            entered,
            release: Arc::new(Mutex::new(release_receive)),
        });
        (entered_receive, release)
    }

    #[cfg(test)]
    pub(crate) fn clear_layout_latch_for_test(&self) {
        *self
            .layout_latch
            .lock()
            .expect("layout test latch lock must remain usable") = None;
    }

    pub(crate) fn request_paint(
        &self,
        layout: DirectLayout,
        theme: Arc<crate::Theme>,
        focused: Option<ComponentId>,
        graph: crate::component::MountGraph,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(anyhow!("direct renderer driver is closed"));
        }
        let mut pending = self
            .pending_paint
            .lock()
            .map_err(|_| anyhow!("direct paint pending lock is poisoned"))?;
        if pending.is_some() {
            return Err(anyhow!("direct paint request is already pending"));
        }
        let (response, receive) = sync_channel(1);
        self.command
            .try_send(DirectDriverCommand::Paint {
                layout,
                theme,
                focused,
                graph,
                response,
                wake,
            })
            .map_err(|_| anyhow!("direct renderer driver queue is full or closed"))?;
        *pending = Some(receive);
        Ok(())
    }

    pub(crate) fn poll_paint(&self) -> Result<Option<crate::physical::Surface>> {
        let mut pending = self
            .pending_paint
            .lock()
            .map_err(|_| anyhow!("direct paint pending lock is poisoned"))?;
        let Some(receive) = pending.take() else {
            return Ok(None);
        };
        match receive.try_recv() {
            Ok(result) => result.map(Some),
            Err(TryRecvError::Empty) => {
                *pending = Some(receive);
                Ok(None)
            }
            Err(TryRecvError::Disconnected) => Err(anyhow!("direct renderer driver dropped paint")),
        }
    }

    pub(crate) fn invalidate_content(&self, port_id: u64) -> Result<()> {
        let (response, receive) = sync_channel(1);
        self.command
            .send(DirectDriverCommand::InvalidateContent { port_id, response })
            .map_err(|_| anyhow!("direct renderer driver is closed"))?;
        receive
            .recv()
            .map_err(|_| anyhow!("direct renderer driver dropped invalidation"))?
    }

    pub(crate) fn invalidate_control(&self, component: ComponentId) -> Result<()> {
        let (response, receive) = sync_channel(1);
        self.command
            .send(DirectDriverCommand::InvalidateControl {
                component,
                response,
            })
            .map_err(|_| anyhow!("direct renderer driver is closed"))?;
        receive
            .recv()
            .map_err(|_| anyhow!("direct renderer driver dropped invalidation"))?
    }

    pub(crate) fn shutdown(&mut self) -> Result<()> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        self.shutdown.store(true, Ordering::Release);
        let join_result = join
            .join()
            .map_err(|_| anyhow!("direct renderer driver panicked"));
        join_result
    }
}

impl Drop for DirectDriverHandle {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("direct renderer driver shutdown failed during drop: {error}");
        }
    }
}

fn direct_driver_loop(
    receive: Receiver<DirectDriverCommand>,
    ready: SyncSender<Result<()>>,
    host_id: u64,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut renderer = DirectOccurrenceRenderer::new(host_id);
    let _ = ready.send(Ok(()));
    while !shutdown.load(Ordering::Acquire) {
        let command = match receive.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(command) => command,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match command {
            DirectDriverCommand::Synchronize {
                snapshots,
                changes,
                participation,
                port_ids,
                roots,
                portal_owners,
                controls,
                response,
            } => {
                let _ = response.send(renderer.synchronize(
                    snapshots,
                    changes.as_deref(),
                    &participation,
                    port_ids,
                    roots,
                    portal_owners,
                    controls,
                ));
            }
            DirectDriverCommand::Layout {
                root,
                size,
                history_anchor,
                measurements,
                invalidate,
                controls,
                response,
                wake,
                #[cfg(test)]
                latch,
            } => {
                #[cfg(test)]
                if let Some(latch) = latch
                    .lock()
                    .expect("layout test latch lock must remain usable")
                    .as_ref()
                {
                    let _ = latch.entered.send(());
                    let _ = latch
                        .release
                        .lock()
                        .expect("layout test latch release lock must remain usable")
                        .recv();
                }
                let result = renderer.prepare(
                    root,
                    size,
                    history_anchor,
                    &measurements,
                    &invalidate,
                    &controls,
                );
                let _ = response.send(result);
                wake();
            }
            DirectDriverCommand::Paint {
                layout,
                theme,
                focused,
                graph,
                response,
                wake,
            } => {
                let result = paint_direct_layout_owned(&layout, &theme, focused, &graph);
                let _ = response.send(result);
                wake();
            }
            DirectDriverCommand::InvalidateContent { port_id, response } => {
                let _ = response.send(renderer.invalidate_content_measurement(port_id));
            }
            DirectDriverCommand::InvalidateControl {
                component,
                response,
            } => {
                let _ = response.send(renderer.invalidate_control_measurement(component));
            }
        }
    }
}

struct DirectOccurrenceRenderer {
    driver_id: u64,
    layout: TaffyLayoutAdapter,
    snapshots: HashMap<NodeKey, OccurrenceSnapshot>,
    participation: HashMap<NodeKey, bool>,
    ports: HashMap<crate::occurrence::ResourceKey, u64>,
    roots: Vec<NodeKey>,
    portal_owners: HashMap<NodeKey, NodeKey>,
    controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    synchronized: bool,
}

impl DirectOccurrenceRenderer {
    fn new(driver_id: u64) -> Self {
        Self {
            driver_id,
            layout: TaffyLayoutAdapter::new(),
            snapshots: HashMap::new(),
            participation: HashMap::new(),
            ports: HashMap::new(),
            roots: Vec::new(),
            portal_owners: HashMap::new(),
            controls: HashMap::new(),
            synchronized: false,
        }
    }

    fn synchronize(
        &mut self,
        snapshots: Vec<OccurrenceSnapshot>,
        changes: Option<&UiChangeSet>,
        participation: &[NodeParticipation],
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<NodeKey>,
        portal_owners: HashMap<NodeKey, NodeKey>,
        controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    ) -> Result<()> {
        let initial = !self.synchronized;
        let changed_keys = if initial {
            snapshots
                .iter()
                .map(|snapshot| snapshot.key)
                .collect::<Vec<_>>()
        } else {
            changes.map_or_else(
                || snapshots.iter().map(|snapshot| snapshot.key).collect(),
                |changes| {
                    let mut keys = changes.changed_nodes.clone();
                    keys.extend(changes.membership_nodes.iter().copied());
                    keys.sort_unstable_by_key(|key| (key.slot, key.generation));
                    keys.dedup();
                    keys
                },
            )
        };
        if let Some(changes) = changes {
            for key in &changes.retired_nodes {
                self.snapshots.remove(key);
            }
        }
        for snapshot in &snapshots {
            self.snapshots.insert(snapshot.key, snapshot.clone());
        }
        let retired = changes.map_or_else(Vec::new, |changes| {
            changes
                .retired_nodes
                .iter()
                .copied()
                .filter(|key| self.layout.contains(*key))
                .collect::<Vec<_>>()
        });
        let sync = if initial {
            self.layout.synchronize(
                &snapshots,
                &changed_keys,
                &changed_keys,
                participation,
                &retired,
            )
        } else {
            self.layout.synchronize_sparse(
                &snapshots,
                &changed_keys,
                &changed_keys,
                participation,
                &retired,
            )
        };
        sync.map_err(|error| anyhow!("direct Taffy synchronization failed: {error:?}"))?;
        if let Some(changes) = changes {
            for key in &changes.retired_nodes {
                self.participation.remove(key);
            }
        }
        for item in participation {
            self.participation.insert(item.key, item.participates);
        }
        self.ports = port_ids;
        self.roots = roots;
        self.portal_owners = portal_owners;
        self.controls = controls;
        self.synchronized = true;
        Ok(())
    }

    fn prepare(
        &mut self,
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: &[NodeKey],
        controls: &HashMap<ComponentId, ControlSnapshot>,
    ) -> Result<DirectLayout> {
        if !self.synchronized {
            return Err(anyhow!("direct occurrence renderer is not synchronized"));
        }
        self.layout
            .invalidate_measurement(
                &invalidate
                    .iter()
                    .copied()
                    .filter(|key| self.layout.contains(*key))
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| anyhow!("direct Taffy measurement invalidation failed: {error:?}"))?;
        let mut controls_by_node = HashMap::new();
        for snapshot in self.snapshots.values() {
            let Some(component) = snapshot
                .control
                .and_then(|control| self.controls.get(&control).copied())
            else {
                continue;
            };
            let control = controls
                .get(&component)
                .cloned()
                .ok_or_else(|| anyhow!("direct control snapshot is missing"))?;
            controls_by_node.insert(snapshot.key, control);
        }
        let (geometries, history_overflow_rows) =
            self.layout_roots(root, size, history_anchor, measurements, &controls_by_node)?;
        self.build_layout_tree(
            root,
            size,
            &geometries,
            measurements,
            controls,
            history_overflow_rows,
        )
    }

    fn invalidate_content_measurement(&mut self, port_id: u64) -> Result<()> {
        let keys = self
            .snapshots
            .values()
            .filter_map(|snapshot| {
                snapshot
                    .port
                    .filter(|port| self.ports.get(port).copied() == Some(port_id))
                    .map(|_| snapshot.key)
            })
            .collect::<Vec<_>>();
        self.layout
            .invalidate_measurement(&keys)
            .map_err(|error| anyhow!("direct content measurement invalidation failed: {error:?}"))
    }

    fn invalidate_control_measurement(&mut self, component: ComponentId) -> Result<()> {
        let controls = self
            .controls
            .iter()
            .filter_map(|(control, candidate)| (*candidate == component).then_some(*control))
            .collect::<HashSet<_>>();
        let keys = self
            .snapshots
            .values()
            .filter(|snapshot| {
                snapshot
                    .control
                    .is_some_and(|control| controls.contains(&control))
            })
            .map(|snapshot| snapshot.key)
            .collect::<Vec<_>>();
        self.layout
            .invalidate_measurement(&keys)
            .map_err(|error| anyhow!("direct control measurement invalidation failed: {error:?}"))
    }

    fn layout_root(
        &mut self,
        root: NodeKey,
        width: AvailableConstraint,
        height: AvailableConstraint,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        controls: &HashMap<NodeKey, ControlSnapshot>,
    ) -> Result<Vec<crate::presentation::taffy::ComputedGeometry>> {
        let mut measurement_error = None;
        let mut failed_measurement_keys = Vec::new();
        let control_fill_width = self
            .snapshots
            .values()
            .map(|snapshot| (snapshot.key, control_fills_available_width(snapshot)))
            .collect::<HashMap<_, _>>();
        #[cfg(feature = "perf-counters")]
        let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::TaffyLayoutNanos);
        let geometries = self
            .layout
            .layout(
                root,
                width,
                height,
                &mut |key, request| match measured_for_request(
                    measurements.get(&key),
                    controls.get(&key),
                    control_fill_width.get(&key).copied().unwrap_or(true),
                    request,
                ) {
                    Ok(measured) => measured,
                    Err(error) => {
                        measurement_error = Some(error);
                        failed_measurement_keys.push(key);
                        MeasuredSize::default()
                    }
                },
            )
            .map_err(|error| anyhow!("direct Taffy layout failed: {error:?}"))?;
        if let Some(error) = measurement_error {
            self.layout
                .invalidate_measurement(&failed_measurement_keys)
                .map_err(|error| anyhow!("direct Taffy retry invalidation failed: {error:?}"))?;
            return Err(error.context("direct terminal content measurement failed"));
        }
        Ok(geometries)
    }

    fn layout_roots(
        &mut self,
        body_root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        controls: &HashMap<NodeKey, ControlSnapshot>,
    ) -> Result<(Vec<crate::presentation::taffy::ComputedGeometry>, usize)> {
        let mut history_roots = Vec::new();
        let mut portal_roots = Vec::new();
        let mut body = None;
        for root in self.roots.iter().copied() {
            match self
                .snapshots
                .get(&root)
                .and_then(|snapshot| snapshot.root_role)
            {
                Some(crate::occurrence::RootRole::LegacyHistoryUnit) => history_roots.push(root),
                Some(crate::occurrence::RootRole::Portal) => portal_roots.push(root),
                Some(crate::occurrence::RootRole::Body) => body = Some(root),
                None => {}
            }
        }
        let body = body.ok_or_else(|| anyhow!("direct occurrence roots have no Body root"))?;
        if body != body_root {
            return Err(anyhow!(
                "direct Body root does not match the occurrence document"
            ));
        }
        let has_history = !history_roots.is_empty();
        let mut output = Vec::new();
        // Obtain the body's intrinsic height before placing it against the
        // terminal viewport, matching the retained root resolver's anchor.
        let body_intrinsic = self.layout_root(
            body,
            AvailableConstraint::Definite(f32::from(size.width)),
            AvailableConstraint::MaxContent,
            measurements,
            controls,
        )?;
        let body_height = body_intrinsic
            .iter()
            .find(|geometry| geometry.key == body)
            .map(|geometry| round_edge(geometry.logical.height))
            .transpose()?
            .ok_or_else(|| anyhow!("direct Body root geometry is missing"))?
            .clamp(0, i32::from(size.height)) as u16;
        let body_measurement = self.layout_root(
            body,
            AvailableConstraint::Definite(f32::from(size.width)),
            AvailableConstraint::Definite(f32::from(body_height)),
            measurements,
            controls,
        )?;
        let history_height = size.height.saturating_sub(body_height);

        let mut history_layouts = Vec::with_capacity(history_roots.len());
        let mut history_height_total = 0.0_f32;
        for root in history_roots {
            let intrinsic_geometries = self.layout_root(
                root,
                AvailableConstraint::Definite(f32::from(size.width)),
                AvailableConstraint::MaxContent,
                measurements,
                controls,
            )?;
            let root_height = intrinsic_geometries
                .iter()
                .find(|geometry| geometry.key == root)
                .map(|geometry| geometry.logical.height)
                .ok_or_else(|| anyhow!("direct History root geometry is missing"))?;
            if !root_height.is_finite() || root_height < 0.0 {
                return Err(anyhow!("direct History root height is invalid"));
            }
            let geometries = self.layout_root(
                root,
                AvailableConstraint::Definite(f32::from(size.width)),
                AvailableConstraint::Definite(root_height),
                measurements,
                controls,
            )?;
            history_height_total += root_height;
            history_layouts.push((geometries, root_height));
        }
        let history_height_total_rounded = round_edge(history_height_total)?.max(0);
        let history_offset = if history_anchor == DirectHistoryAnchor::NativeFrontier {
            0.0
        } else {
            f32::from(history_height) - history_height_total
        };
        let history_overflow_rows =
            history_overflow_rows(history_height_total_rounded, history_height);
        let mut translated = Vec::new();
        let mut history_y = history_offset;
        for (mut geometries, root_height) in history_layouts {
            for geometry in &mut geometries {
                translate_geometry_by(geometry, 0.0, history_y)?;
            }
            translated.extend(geometries);
            history_y += root_height;
        }
        output.extend(translated);

        let mut body_geometries = body_measurement;
        let body_y = if !has_history {
            i32::from(size.height.saturating_sub(body_height))
        } else {
            i32::from(history_height)
        };
        for geometry in &mut body_geometries {
            translate_geometry_by(geometry, 0.0, body_y as f32)?;
        }
        output.extend(body_geometries);

        // Portals are overlays owned by an occurrence, not additional flow
        // roots. Their local Taffy tree is constrained by the owner's content
        // box and translated to that actual owner allocation.
        for root in portal_order(&portal_roots, &self.portal_owners, &self.snapshots)? {
            let owner = self
                .portal_owners
                .get(&root)
                .copied()
                .ok_or_else(|| anyhow!("direct Portal root owner is missing"))?;
            let owner_geometry = output
                .iter()
                .find(|geometry| geometry.key == owner)
                .copied()
                .ok_or_else(|| anyhow!("direct Portal owner geometry is missing"))?;
            let owner_width = owner_geometry.logical_content_width.max(0.0);
            let geometries = self.layout_root(
                root,
                AvailableConstraint::Definite(owner_width),
                AvailableConstraint::MaxContent,
                measurements,
                controls,
            )?;
            for mut geometry in geometries {
                translate_geometry_by(
                    &mut geometry,
                    owner_geometry.logical_content_x,
                    owner_geometry.logical_content_y,
                )?;
                output.push(geometry);
            }
        }
        Ok((output, history_overflow_rows))
    }

    fn build_layout_tree(
        &self,
        root: NodeKey,
        size: crate::geometry::Size,
        geometries: &[crate::presentation::taffy::ComputedGeometry],
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        controls: &HashMap<ComponentId, ControlSnapshot>,
        history_overflow_rows: usize,
    ) -> Result<DirectLayout> {
        let geometry_map = geometries
            .iter()
            .map(|geometry| (geometry.key, geometry))
            .collect();
        let mut nodes = Vec::with_capacity(geometries.len().saturating_add(1));
        let mut occurrence_geometry = HashMap::with_capacity(geometries.len());
        let mut content_widths = HashMap::new();
        nodes.push(DirectNode {
            key: root,
            snapshot: None,
            rect: Rect::new(0, 0, size.width, size.height),
            content_rect: Rect::new(0, 0, size.width, size.height),
            content_width: size.width,
            clip_rect: Rect::new(0, 0, size.width, size.height),
            paint_origin: (0, 0),
            content_origin: (0, 0),
            component: None,
            children: Vec::new(),
            style_states: Default::default(),
            style_facts: Default::default(),
            decoration: Default::default(),
            content: DirectContent::Children,
        });
        let roots = if self.roots.is_empty() {
            vec![root]
        } else {
            self.roots.clone()
        };
        let mut root_children = Vec::with_capacity(roots.len());
        for root in roots {
            root_children.push(self.emit_node(
                root,
                Some(Rect::new(0, 0, size.width, size.height)),
                &geometry_map,
                measurements,
                controls,
                &mut occurrence_geometry,
                &mut content_widths,
                &mut nodes,
            )?);
        }
        nodes[0].children = root_children;
        let mut tree = DirectTree {
            root: DirectNodeId(0),
            nodes,
            size,
            physically_complete: measurements
                .values()
                .all(|capture| capture.measurement.physically_complete),
            parents: Vec::new(),
            content_roots: HashMap::new(),
        };
        tree.index();
        let mut component_mounts = Vec::new();
        for root in self.roots.iter().copied() {
            component_mounts.extend(self.component_mounts(root)?);
        }
        Ok(DirectLayout {
            tree,
            occurrence_geometry,
            content_widths,
            content_products: measurements.clone(),
            component_mounts,
            history_overflow_rows,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_node(
        &self,
        key: NodeKey,
        inherited_clip: Option<Rect>,
        geometries: &HashMap<NodeKey, &crate::presentation::taffy::ComputedGeometry>,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        controls: &HashMap<ComponentId, ControlSnapshot>,
        occurrence_geometry: &mut HashMap<NodeKey, ComponentGeometry>,
        content_widths: &mut HashMap<NodeKey, f32>,
        nodes: &mut Vec<DirectNode>,
    ) -> Result<DirectNodeId> {
        let snapshot = self
            .snapshots
            .get(&key)
            .ok_or_else(|| anyhow!("snapshot missing for {key:?}"))?;
        let geometry = geometries
            .get(&key)
            .ok_or_else(|| anyhow!("geometry missing for {key:?}"))?;
        let rect_info = physical_rect(geometry.rect)?;
        let rect = rect_info.rect;
        let content_info = physical_box(
            geometry.logical_content_x,
            geometry.logical_content_y,
            geometry.logical_content_width,
            geometry.logical_content_height,
        )?;
        let content_rect = content_info.rect;
        let content_width = measurements
            .get(&key)
            .map_or(geometry.logical_content_width, |capture| {
                content_width_for_layout(snapshot, geometry, capture)
            });
        let content_width_cells = floor_constraint_width(content_width)?;
        let participates = self.participation.get(&key).copied().unwrap_or(true);
        let visible = participates && !geometry.renderer_hidden && !geometry.display_none;
        let clip_rect = if visible {
            inherited_clip
                .and_then(|clip| signed_intersection(rect_info.origin, rect.size(), clip))
                .unwrap_or(Rect::new(rect.x, rect.y, 0, 0))
        } else {
            Rect::new(rect.x, rect.y, 0, 0)
        };
        occurrence_geometry.insert(
            key,
            ComponentGeometry {
                outer: rect,
                content: content_rect,
                visible: clip_rect.intersection(rect),
            },
        );
        if snapshot.kind == HostKind::ContentHost {
            content_widths.insert(key, content_width);
        }
        let component = snapshot
            .control
            .and_then(|control| self.controls.get(&control).copied());
        let control_snapshot = (snapshot.kind == HostKind::Editor)
            .then_some(component)
            .flatten()
            .and_then(|component| controls.get(&component).cloned());
        let id = DirectNodeId(nodes.len());
        let direct_content = if let Some(control) = control_snapshot {
            DirectContent::Control(control)
        } else if snapshot.kind == HostKind::ContentHost && measurements.get(&key).is_none() {
            if visible {
                return Err(anyhow!(
                    "active ContentHost measurement is missing; its owner must exclude the exported root"
                ));
            }
            DirectContent::Children
        } else {
            content(snapshot, measurements.get(&key))?
        };
        nodes.push(DirectNode {
            key,
            snapshot: Some(snapshot.clone()),
            rect,
            content_rect,
            content_width: measurements
                .get(&key)
                .map(|_| content_width_cells)
                .unwrap_or(content_rect.width),
            clip_rect,
            paint_origin: rect_info.origin,
            content_origin: content_info.origin,
            component,
            children: Vec::new(),
            style_states: style_states(snapshot),
            style_facts: Default::default(),
            decoration: decoration(snapshot),
            content: direct_content,
        });
        let mut children = Vec::with_capacity(snapshot.children.len());
        for child in snapshot.children.iter().copied() {
            children.push(self.emit_node(
                child,
                Some(clip_rect),
                geometries,
                measurements,
                controls,
                occurrence_geometry,
                content_widths,
                nodes,
            )?);
        }
        nodes[id.0].children = children;
        Ok(id)
    }

    fn component_mounts(&self, root: NodeKey) -> Result<Vec<(ComponentId, Option<ComponentId>)>> {
        let mut result = Vec::new();
        let mut stack = vec![(root, None, true)];
        while let Some((key, parent, ancestors_visible)) = stack.pop() {
            if !ancestors_visible || !self.participation.get(&key).copied().unwrap_or(true) {
                continue;
            }
            let snapshot = self
                .snapshots
                .get(&key)
                .ok_or_else(|| anyhow!("direct mount snapshot is missing"))?;
            if snapshot_display_none(snapshot) {
                continue;
            }
            let component = snapshot
                .control
                .and_then(|control| self.controls.get(&control).copied());
            let next_parent = component.or(parent);
            if let Some(component) = component {
                result.push((component, parent));
            }
            for child in snapshot.children.iter().rev().copied() {
                stack.push((child, next_parent, true));
            }
        }
        Ok(result)
    }
}

fn portal_order(
    roots: &[NodeKey],
    owners: &HashMap<NodeKey, NodeKey>,
    snapshots: &HashMap<NodeKey, OccurrenceSnapshot>,
) -> Result<Vec<NodeKey>> {
    let root_set = roots.iter().copied().collect::<HashSet<_>>();
    let ownership_index = portal_root_index(roots, snapshots)?;
    let dependencies = roots
        .iter()
        .copied()
        .filter_map(|root| {
            owners
                .get(&root)
                .and_then(|owner| ownership_index.get(owner).copied())
                .map(|owner_root| (root, owner_root))
        })
        .collect::<HashMap<_, _>>();
    let mut marks = HashMap::<NodeKey, u8>::new();
    let mut ordered = Vec::with_capacity(roots.len());
    for root in roots.iter().copied() {
        visit_portal(root, &root_set, &dependencies, &mut marks, &mut ordered)?;
    }
    Ok(ordered)
}

/// Index every node in each portal subtree once. Portal dependencies are
/// then resolved by lookup rather than by recursively searching every root
/// for every portal owner.
fn portal_root_index(
    roots: &[NodeKey],
    snapshots: &HashMap<NodeKey, OccurrenceSnapshot>,
) -> Result<HashMap<NodeKey, NodeKey>> {
    let mut ownership = HashMap::new();
    let mut stack = Vec::new();
    for root in roots.iter().rev().copied() {
        stack.push((root, root));
    }
    while let Some((key, portal_root)) = stack.pop() {
        if ownership.insert(key, portal_root).is_some() {
            return Err(anyhow!(
                "direct Portal subtree contains a duplicate occurrence"
            ));
        }
        let snapshot = snapshots
            .get(&key)
            .ok_or_else(|| anyhow!("direct Portal snapshot is missing for {key:?}"))?;
        for child in snapshot.children.iter().rev().copied() {
            stack.push((child, portal_root));
        }
    }
    Ok(ownership)
}

fn visit_portal(
    root: NodeKey,
    root_set: &HashSet<NodeKey>,
    dependencies: &HashMap<NodeKey, NodeKey>,
    marks: &mut HashMap<NodeKey, u8>,
    ordered: &mut Vec<NodeKey>,
) -> Result<()> {
    match marks.get(&root).copied() {
        Some(2) => return Ok(()),
        Some(1) => return Err(anyhow!("direct Portal ownership graph contains a cycle")),
        _ => {}
    }
    marks.insert(root, 1);
    if let Some(owner) = dependencies.get(&root).copied()
        && root_set.contains(&owner)
    {
        visit_portal(owner, root_set, dependencies, marks, ordered)?;
    }
    marks.insert(root, 2);
    ordered.push(root);
    Ok(())
}

fn measured_for_request(
    capture: Option<&CapturedContentMeasurement>,
    control: Option<&ControlSnapshot>,
    fill_available_width: bool,
    request: crate::presentation::taffy::MeasureRequest,
) -> Result<MeasuredSize> {
    if capture.is_none() {
        let Some(control) = control else {
            // A childless ordinary Box has no intrinsic content. This is a
            // valid zero-sized leaf, unlike a missing ContentHost/control
            // capture, which is rejected at tree emission.
            return Ok(MeasuredSize::default());
        };
        let mut measured = measure_control(control, request, fill_available_width)?;
        if let Some(width) = request.known_width {
            measured.width = width;
        }
        if let Some(height) = request.known_height {
            measured.height = height;
        }
        return Ok(measured);
    }
    let capture = capture.expect("content capture checked above");
    let requested_product = content_product_for_request(capture, request)?;
    let available_width = match request.available_width {
        AvailableConstraint::Definite(width) => Some(floor_constraint_width(width)?),
        AvailableConstraint::MinContent | AvailableConstraint::MaxContent => None,
    };
    let mut measured = requested_product
        .as_ref()
        .map(|product| {
            let size = product.size();
            let width = if request.known_width.is_none() {
                available_width.map_or_else(
                    || size.width(),
                    |available| product.intrinsic_size().width().min(available),
                )
            } else {
                size.width()
            };
            MeasuredSize {
                width: f32::from(width),
                height: f32::from(size.height()),
            }
        })
        .unwrap_or(MeasuredSize {
            width: capture.measurement.intrinsic_size.width.into(),
            height: capture.measurement.intrinsic_size.height.into(),
        });
    // History's owning content adapter may have irreversibly exported a
    // prefix. Apply that exact adjustment only to the captured immutable
    // product and width; ordinary semantic products retain their wrapping.
    let uses_captured_product = requested_product.as_ref().is_some_and(|product| {
        capture
            .terminal_product
            .as_ref()
            .is_some_and(|captured| std::sync::Arc::ptr_eq(product, captured))
    });
    if let Some(adjustment) = capture.history_adjustment
        && uses_captured_product
        && adjustment.projection_identity == capture.measurement.projection_identity
        && adjustment.offered_width == capture.offered_width
    {
        measured.height = (measured.height - adjustment.removed_rows as f32).max(0.0);
    }
    if requested_product.is_none() {
        measured.width = match request.known_width {
            Some(width) => width,
            None => match request.available_width {
                AvailableConstraint::Definite(width) => width,
                AvailableConstraint::MinContent => f32::from(capture.min_content.width),
                AvailableConstraint::MaxContent => f32::from(capture.max_content.width),
            },
        };
        measured.height = match request.known_height {
            Some(height) => height,
            None => match request.available_height {
                AvailableConstraint::Definite(height) => height,
                AvailableConstraint::MinContent => f32::from(capture.min_content.height),
                AvailableConstraint::MaxContent => f32::from(capture.max_content.height),
            },
        };
    }
    if let Some(width) = request.known_width {
        measured.width = width;
    }
    if let Some(height) = request.known_height {
        measured.height = height;
    }
    Ok(measured)
}

/// Resolve a pure width-specific product from the immutable semantic values
/// captured by the host. This callback never reaches Source/Connector state;
/// a failed or absent refinement uses the already selected product instead of
/// silently routing through an unrelated content owner.
fn content_product_for_request(
    capture: &CapturedContentMeasurement,
    request: crate::presentation::taffy::MeasureRequest,
) -> Result<Option<std::sync::Arc<TerminalTextProduct>>> {
    let constraints = match request.known_width {
        Some(width) => TerminalConstraints::definite(floor_constraint_width(width)?),
        None => match request.available_width {
            AvailableConstraint::Definite(width) => {
                TerminalConstraints::definite(floor_constraint_width(width)?)
            }
            AvailableConstraint::MinContent => TerminalConstraints::min_content(),
            AvailableConstraint::MaxContent => TerminalConstraints::max_content(),
        },
    };
    if matches!(
        constraints.width(),
        crate::text::TerminalWidthConstraint::Definite(width)
            if width == capture.offered_width
    ) {
        if let Some(product) = capture.terminal_product.as_ref() {
            return Ok(Some(std::sync::Arc::clone(product)));
        }
    }
    // A Taffy measurement callback may only read a captured immutable
    // realization. Rewrapping or projecting from semantic values here would
    // put expensive content work back on the layout thread and could produce
    // geometry that has no receipt-pinned product. Width misses are scheduled
    // by the ContentHost owner; the retained capture supplies its old metrics
    // until the matching width product is ready.
    Ok(None)
}

fn floor_constraint_width(value: f32) -> Result<u16> {
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow!("terminal content width is not finite"));
    }
    let value = value.floor();
    if value > f32::from(u16::MAX) {
        return Err(anyhow!("terminal content width exceeds terminal range"));
    }
    Ok(value as u16)
}

fn measure_control(
    control: &ControlSnapshot,
    request: crate::presentation::taffy::MeasureRequest,
    fill_available_width: bool,
) -> Result<MeasuredSize> {
    let ControlSnapshot::Editor(editor) = control else {
        return Ok(MeasuredSize::default());
    };
    let border_width = editor.border.as_ref().map_or(0, |border| {
        border.left_width().saturating_add(border.right_width())
    });
    let border_height = editor.border.as_ref().map_or(0, |border| {
        border.top_height().saturating_add(border.bottom_height())
    });
    let min_content_width =
        editor_intrinsic_width(&editor.text, true).saturating_add(usize::from(editor.focused));
    let max_content_width =
        editor_intrinsic_width(&editor.text, false).saturating_add(usize::from(editor.focused));
    let intrinsic_width = match request.available_width {
        AvailableConstraint::MinContent => min_content_width,
        AvailableConstraint::MaxContent => max_content_width,
        AvailableConstraint::Definite(_) => max_content_width,
    };
    let intrinsic_width = intrinsic_width
        .saturating_add(usize::from(border_width))
        .min(usize::from(u16::MAX));
    let requested_width = match request.known_width {
        Some(width) => floor_constraint_width(width)?,
        None => match request.available_width {
            AvailableConstraint::Definite(width) if fill_available_width => {
                floor_constraint_width(width)?
            }
            AvailableConstraint::Definite(width) => {
                intrinsic_width.min(usize::from(floor_constraint_width(width)?)) as u16
            }
            AvailableConstraint::MinContent => min_content_width
                .saturating_add(usize::from(border_width))
                .min(usize::from(u16::MAX)) as u16,
            AvailableConstraint::MaxContent => max_content_width
                .saturating_add(usize::from(border_width))
                .min(usize::from(u16::MAX)) as u16,
        },
    };
    let inner_width = requested_width.saturating_sub(border_width);
    let rows = if editor.multiline {
        crate::presentation::wrap::input_wrap_ranges(&editor.text, inner_width).len()
    } else {
        1
    };
    let mut measured = MeasuredSize {
        width: if request.known_width.is_some() {
            request.known_width.expect("checked known width")
        } else {
            f32::from(requested_width)
        },
        height: (rows.saturating_add(usize::from(border_height))) as f32,
    };
    if let Some(height) = request.known_height {
        measured.height = height;
    }
    Ok(measured)
}

fn editor_intrinsic_width(text: &str, min_content: bool) -> usize {
    text.split('\n')
        .map(|line| {
            if min_content {
                line.split_whitespace()
                    .map(|word| {
                        word.graphemes(true)
                            .map(crate::physical::grapheme_cell_width)
                            .sum::<usize>()
                    })
                    .max()
                    .unwrap_or(0)
            } else {
                line.graphemes(true)
                    .map(crate::physical::grapheme_cell_width)
                    .sum::<usize>()
            }
        })
        .max()
        .unwrap_or(0)
}

fn control_fills_available_width(snapshot: &OccurrenceSnapshot) -> bool {
    !matches!(
        property(snapshot, PropertyId::Width),
        Some(LayerValue::Value(PropertyValue::SizeMode(
            crate::occurrence::SizeMode::Fit,
        ))) | Some(LayerValue::Value(PropertyValue::Dimension(
            crate::occurrence::DimensionValue::Auto,
        )))
    )
}

fn content_width_for_layout(
    snapshot: &OccurrenceSnapshot,
    geometry: &crate::presentation::taffy::ComputedGeometry,
    capture: &CapturedContentMeasurement,
) -> f32 {
    let width_is_fit = matches!(
        property(snapshot, PropertyId::Width),
        None | Some(LayerValue::Unset | LayerValue::Null)
            | Some(LayerValue::Value(PropertyValue::SizeMode(
                crate::occurrence::SizeMode::Fit,
            )))
            | Some(LayerValue::Value(PropertyValue::Dimension(
                crate::occurrence::DimensionValue::Auto,
            )))
    );
    if width_is_fit {
        f32::from(capture.measurement.intrinsic_size.width)
            .min(geometry.logical_content_width.max(0.0))
    } else {
        geometry.logical_content_width.max(0.0)
    }
}

fn property<'a>(snapshot: &'a OccurrenceSnapshot, id: PropertyId) -> Option<&'a LayerValue> {
    snapshot
        .properties
        .iter()
        .find_map(|(property, value)| (*property == id).then_some(value))
}

fn content(
    snapshot: &OccurrenceSnapshot,
    measurement: Option<&CapturedContentMeasurement>,
) -> Result<DirectContent> {
    if snapshot.kind != HostKind::ContentHost {
        return Ok(DirectContent::Children);
    }
    let capture = measurement.ok_or_else(|| anyhow!("ContentHost measurement is missing"))?;
    if capture.port_id == 0 {
        return Err(anyhow!("ContentPort identity is invalid"));
    }
    Ok(DirectContent::ContentHost {
        port_id: capture.port_id,
        connector_id: capture.measurement.connector_id,
        projection_revision: capture.measurement.projection_revision,
        metric_revision: capture.measurement.metric_revision,
        paint_revision: capture.measurement.paint_revision,
        projection_identity: capture.measurement.projection_identity,
        physically_complete: capture.measurement.physically_complete,
        intrinsic_size: capture.measurement.intrinsic_size,
    })
}

pub(crate) fn snapshot_display_none(snapshot: &OccurrenceSnapshot) -> bool {
    snapshot.properties.iter().any(|(property, value)| {
        *property == PropertyId::Display
            && matches!(
                value,
                LayerValue::Value(PropertyValue::Display(crate::occurrence::DisplayMode::None))
            )
    })
}

fn style_states(snapshot: &OccurrenceSnapshot) -> crate::presentation::StyleStates {
    let mut states = crate::presentation::StyleStates::default();
    for (key, value) in &snapshot.style_states {
        states.set(
            StyleStateKey::new(key.clone()),
            StyleStateValue::new(value.clone()),
        );
    }
    states
}

fn decoration(snapshot: &OccurrenceSnapshot) -> DirectDecoration {
    let mut decoration = DirectDecoration::default();
    let mut direct = StyleSpec::new();
    let mut border_style = None;
    let mut border_edges = None;
    let mut border_color = None;
    let mut border_glyphs = None;
    for (property, layer) in &snapshot.properties {
        let LayerValue::Value(value) = layer else {
            continue;
        };
        match (property, value) {
            (PropertyId::Foreground, PropertyValue::Color(color)) => {
                direct.set_foreground(color.clone())
            }
            (PropertyId::BorderStyle, PropertyValue::BorderStyle(style)) => {
                border_style = Some(*style)
            }
            (PropertyId::BorderEdges, PropertyValue::Edges(edges)) => border_edges = Some(*edges),
            (PropertyId::BorderColor, PropertyValue::Color(color)) => {
                border_color = Some(color.clone())
            }
            (PropertyId::BorderGlyphs, PropertyValue::Glyphs(glyphs)) => {
                border_glyphs = Some(glyphs.clone())
            }
            (PropertyId::TextAttributes, PropertyValue::TextAttributes(attributes)) => {
                for attribute in [
                    TextAttribute::Bold,
                    TextAttribute::Dim,
                    TextAttribute::Italic,
                    TextAttribute::Underline,
                    TextAttribute::Reversed,
                    TextAttribute::Strikethrough,
                ] {
                    if let Some(enabled) = attributes.attribute_value(attribute) {
                        direct.set_attribute(attribute, enabled);
                    }
                }
            }
            (PropertyId::Style, PropertyValue::Style(style)) => {
                decoration.text_style = style.clone()
            }
            (PropertyId::Background, PropertyValue::Color(color)) => {
                decoration.surface_background = Some(color.clone())
            }
            (PropertyId::Padding, PropertyValue::Insets(_)) => {}
            _ => {}
        }
    }
    decoration.text_style.overlay(&direct);
    if border_style.is_some()
        || border_edges.is_some()
        || border_color.is_some()
        || border_glyphs.is_some()
    {
        let mut border = match border_style.unwrap_or(BorderStyle::Plain) {
            BorderStyle::Plain => BorderSpec::plain(),
            BorderStyle::Rounded => BorderSpec::rounded(),
            BorderStyle::Double => BorderSpec::double(),
        };
        if let Some(edges) = border_edges {
            border.set_edges(edges);
        }
        if let Some(color) = border_color {
            border.set_color(Some(color));
        }
        if let Some(glyphs) = border_glyphs {
            border.set_glyphs(glyphs);
        }
        decoration.border = Some(border);
    }
    decoration
}

/// Paints one immutable occurrence layout directly into the physical target.
/// Ordinary boxes contribute only their occurrence-owned decoration and
/// children; ContentHosts use the captured provider ticket and native editor
/// controls use their concrete immutable snapshot.
pub(crate) fn paint_direct_layout(
    layout: &DirectLayout,
    theme: &crate::Theme,
    content: &dyn crate::presentation::ContentProvider,
    focused: Option<ComponentId>,
    graph: &crate::component::MountGraph,
) -> Result<crate::physical::Surface> {
    #[cfg(feature = "perf-counters")]
    let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::DirectPaintNanos);
    let mut surface =
        crate::physical::Surface::new(layout.tree.size.width, layout.tree.size.height);
    let resolver = crate::presentation::paint::ThemeResolver::new(theme);
    paint_direct_node(
        &layout.tree,
        layout.tree.root,
        &resolver,
        content,
        focused,
        graph,
        &mut surface,
        crate::physical::PhysicalStyle::default(),
        crate::presentation::paint::StyleContext::default(),
        Rect::new(0, 0, layout.tree.size.width, layout.tree.size.height),
    )?;
    if !layout.tree.physically_complete {
        surface.physically_complete = false;
    }
    Ok(surface)
}

/// Paint implementation used by the layout owner thread. Every ContentHost
/// lookup resolves against the immutable capture carried by `DirectLayout`;
/// it cannot reach a live Source, Connector, or HostInner while the worker is
/// painting.
fn paint_direct_layout_owned(
    layout: &DirectLayout,
    theme: &crate::Theme,
    focused: Option<ComponentId>,
    graph: &crate::component::MountGraph,
) -> Result<crate::physical::Surface> {
    let content = CapturedContentProvider {
        captures: &layout.content_products,
        theme,
    };
    let mut surface =
        crate::physical::Surface::new(layout.tree.size.width, layout.tree.size.height);
    let resolver = crate::presentation::paint::ThemeResolver::new(theme);
    paint_direct_node(
        &layout.tree,
        layout.tree.root,
        &resolver,
        &content,
        focused,
        graph,
        &mut surface,
        crate::physical::PhysicalStyle::default(),
        crate::presentation::paint::StyleContext::default(),
        Rect::new(0, 0, layout.tree.size.width, layout.tree.size.height),
    )?;
    if !layout.tree.physically_complete {
        surface.physically_complete = false;
    }
    Ok(surface)
}

struct CapturedContentProvider<'a> {
    captures: &'a HashMap<NodeKey, CapturedContentMeasurement>,
    theme: &'a crate::Theme,
}

impl crate::presentation::ContentProvider for CapturedContentProvider<'_> {
    fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
        0
    }

    fn measure(
        &mut self,
        _port_id: u64,
        _offered_width: u16,
        _width_rule: crate::presentation::ContentWidthRule,
    ) -> ContentMeasurement {
        ContentMeasurement::default()
    }

    fn paint_window(
        &self,
        ticket: crate::presentation::PreparedProjectionTicket,
        window: crate::presentation::ContentWindow,
        target: &mut crate::physical::Surface,
        target_origin: (u16, u16),
        clip: Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        self.paint_window_signed(
            ticket,
            window,
            target,
            (i32::from(target_origin.0), i32::from(target_origin.1)),
            clip,
            style,
        );
    }

    fn paint_window_signed(
        &self,
        ticket: crate::presentation::PreparedProjectionTicket,
        window: crate::presentation::ContentWindow,
        target: &mut crate::physical::Surface,
        target_origin: (i32, i32),
        clip: Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        let capture = self.captures.values().find(|capture| {
            capture.port_id == ticket.port_id
                && capture.offered_width == ticket.offered_width
                && capture.measurement.connector_id == ticket.connector_id
                && capture.measurement.projection_identity == ticket.projection_identity
                && capture.measurement.projection_revision == ticket.projection_revision
        });
        let Some(product) = capture.and_then(|capture| capture.terminal_product.as_ref()) else {
            target.physically_complete = false;
            return;
        };
        let first_row = usize::try_from(window.first_row)
            .unwrap_or(usize::MAX)
            .saturating_add(
                capture
                    .and_then(|capture| capture.history_adjustment)
                    .map_or(0, |adjustment| adjustment.removed_rows),
            );
        let row_count = usize::try_from(window.row_count)
            .unwrap_or(usize::MAX)
            .min(product.rows().len().saturating_sub(first_row));
        if row_count == 0 {
            return;
        }
        if product
            .paint_window(
                self.theme,
                style,
                target,
                target_origin,
                clip,
                crate::text::TerminalRowWindow::new(first_row, row_count),
            )
            .is_err()
        {
            target.physically_complete = false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_direct_node(
    tree: &DirectTree,
    id: DirectNodeId,
    resolver: &crate::presentation::paint::ThemeResolver,
    content: &dyn crate::presentation::ContentProvider,
    focused: Option<ComponentId>,
    graph: &crate::component::MountGraph,
    target: &mut crate::physical::Surface,
    inherited: crate::physical::PhysicalStyle,
    inherited_context: crate::presentation::paint::StyleContext,
    inherited_clip: Rect,
) -> Result<()> {
    let node = tree.node(id);
    let scope =
        crate::presentation::paint::StyleContext::for_scope(node.component, focused, Some(graph));
    let context = inherited_context.enter_node(&node.style_states, &node.style_facts, scope);
    let resolved = resolver.resolve_text_style(inherited, &node.decoration.text_style, &context);
    let clip = inherited_clip
        .intersection(node.clip_rect)
        .unwrap_or(Rect::new(node.rect.x, node.rect.y, 0, 0));

    if let Some(background) = &node.decoration.surface_background {
        let color = resolver.resolve_color(background, &context);
        let left = node.paint_origin.0.max(i32::from(clip.x)).max(0);
        let top = node.paint_origin.1.max(i32::from(clip.y)).max(0);
        let right = node
            .paint_origin
            .0
            .saturating_add(i32::from(node.rect.width))
            .min(i32::from(clip.right()))
            .min(i32::from(target.width()));
        let bottom = node
            .paint_origin
            .1
            .saturating_add(i32::from(node.rect.height))
            .min(i32::from(clip.bottom()))
            .min(i32::from(target.height()));
        for y in top..bottom {
            for x in left..right {
                target.get_mut(x as u16, y as u16).style.background = Some(color);
            }
        }
    }

    match &node.content {
        DirectContent::Children
        | DirectContent::Control(ControlSnapshot::Scroll(_) | ControlSnapshot::Animation(_)) => {
            for child in node.children.iter().copied() {
                paint_direct_node(
                    tree,
                    child,
                    resolver,
                    content,
                    focused,
                    graph,
                    target,
                    resolved,
                    context.clone(),
                    clip,
                )?;
            }
        }
        DirectContent::ContentHost {
            port_id,
            connector_id,
            projection_revision,
            projection_identity,
            ..
        } => {
            let ticket = crate::presentation::PreparedProjectionTicket {
                port_id: *port_id,
                connector_id: *connector_id,
                offered_width: node.content_width,
                projection_revision: *projection_revision,
                projection_identity: *projection_identity,
            };
            content.paint_window_signed(
                ticket,
                crate::presentation::ContentWindow::full(u32::from(node.content_rect.height)),
                target,
                node.content_origin,
                clip,
                resolved,
            );
        }
        DirectContent::Control(ControlSnapshot::Editor(editor)) => {
            if let Some(border) = &editor.border {
                crate::presentation::paint::paint_border_at(
                    target,
                    border,
                    resolver,
                    resolved,
                    &context,
                    node.content_origin,
                    (node.content_rect.width, node.content_rect.height),
                    clip,
                );
            }
            paint_editor(editor, node, resolved, target, clip);
        }
    }
    if let Some(border) = &node.decoration.border {
        crate::presentation::paint::paint_border_at(
            target,
            border,
            resolver,
            resolved,
            &context,
            node.paint_origin,
            (node.rect.width, node.rect.height),
            clip,
        );
    }
    Ok(())
}

fn paint_editor(
    editor: &EditorSnapshot,
    node: &DirectNode,
    style: crate::physical::PhysicalStyle,
    target: &mut crate::physical::Surface,
    clip: Rect,
) {
    let (origin, size) =
        editor
            .border
            .as_ref()
            .map_or((node.content_origin, node.content_rect.size()), |border| {
                (
                    (
                        node.content_origin
                            .0
                            .saturating_add(i32::from(border.left_width())),
                        node.content_origin
                            .1
                            .saturating_add(i32::from(border.top_height())),
                    ),
                    crate::geometry::Size::new(
                        node.content_rect.width.saturating_sub(
                            border.left_width().saturating_add(border.right_width()),
                        ),
                        node.content_rect.height.saturating_sub(
                            border.top_height().saturating_add(border.bottom_height()),
                        ),
                    ),
                )
            });
    let ranges = crate::presentation::wrap::input_wrap_ranges(&editor.text, size.width);
    let first = editor.scroll_row;
    let cursor_row = if editor.focused {
        ranges
            .iter()
            .enumerate()
            .find(|(_, range)| {
                editor.cursor_bytes >= range.start && editor.cursor_bytes < range.end
            })
            .map(|(index, _)| index)
            .or_else(|| {
                ranges
                    .iter()
                    .enumerate()
                    .rfind(|(_, range)| editor.cursor_bytes == range.end)
                    .map(|(index, _)| index)
            })
    } else {
        None
    };
    for (row_index, range) in ranges.iter().enumerate().skip(first) {
        let y = origin
            .1
            .saturating_add(i32::try_from(row_index - first).unwrap_or(i32::MAX));
        if y < i32::from(clip.y)
            || y >= i32::from(clip.bottom())
            || y < 0
            || y >= i32::from(target.height())
            || row_index.saturating_sub(first) >= usize::from(size.height)
        {
            continue;
        }
        let mut x = origin.0;
        let text = &editor.text[range.clone()];
        for (offset, grapheme) in text.grapheme_indices(true) {
            let cell_width = grapheme_cell_width(grapheme);
            if cell_width == 0 {
                continue;
            }
            let grapheme_start = range.start.saturating_add(offset);
            let grapheme_end = grapheme_start.saturating_add(grapheme.len());
            let cursor = cursor_row == Some(row_index)
                && editor.cursor_bytes >= grapheme_start
                && editor.cursor_bytes < grapheme_end;
            let mut cell_style = style;
            if cursor {
                cell_style.reversed = !cell_style.reversed;
            }
            let end = x.saturating_add(i32::try_from(cell_width).unwrap_or(i32::MAX));
            if x >= i32::from(clip.x)
                && end <= i32::from(clip.right())
                && x >= 0
                && end <= i32::from(target.width())
            {
                target.clear_glyph_at(x as u16, y as u16);
                *target.get_mut(x as u16, y as u16) = PhysicalCell {
                    grapheme: Some(grapheme.to_owned()),
                    style: cell_style,
                    painted: true,
                    continuation: false,
                };
                for continuation in 1..cell_width {
                    *target.get_mut((x as usize + continuation) as u16, y as u16) = PhysicalCell {
                        grapheme: None,
                        style,
                        painted: true,
                        continuation: true,
                    };
                }
            }
            x = end;
        }
        if cursor_row == Some(row_index)
            && editor.cursor_bytes == range.end
            && x < i32::from(clip.right())
            && x >= i32::from(clip.x)
            && x >= 0
            && x < i32::from(target.width())
        {
            let mut cell_style = style;
            cell_style.reversed = !cell_style.reversed;
            *target.get_mut(x as u16, y as u16) = PhysicalCell {
                grapheme: Some(" ".to_owned()),
                style: cell_style,
                painted: true,
                continuation: false,
            };
        }
    }
}

fn translate_geometry_by(
    geometry: &mut crate::presentation::taffy::ComputedGeometry,
    x: f32,
    y: f32,
) -> Result<()> {
    geometry.logical.x += x;
    geometry.logical.y += y;
    geometry.logical_content_x += x;
    geometry.logical_content_y += y;
    let x = round_edge(geometry.logical.x)?;
    let top = round_edge(geometry.logical.y)?;
    let right = round_edge(geometry.logical.x + geometry.logical.width)?;
    let bottom = round_edge(geometry.logical.y + geometry.logical.height)?;
    geometry.rect = crate::presentation::taffy::PhysicalRect {
        x,
        y: top,
        width: right
            .checked_sub(x)
            .ok_or_else(|| anyhow!("direct geometry width underflow"))?,
        height: bottom
            .checked_sub(top)
            .ok_or_else(|| anyhow!("direct geometry height underflow"))?,
    };
    Ok(())
}

fn round_edge(value: f32) -> Result<i32> {
    super::taffy::checked_round_edge(value)
        .map_err(|error| anyhow!("direct geometry edge conversion failed: {error:?}"))
}

fn history_overflow_rows(total_height: i32, available_height: u16) -> usize {
    total_height
        .saturating_sub(i32::from(available_height))
        .max(0) as usize
}

#[derive(Clone, Copy)]
struct DirectRect {
    rect: Rect,
    origin: (i32, i32),
}

fn physical_rect(rect: crate::presentation::taffy::PhysicalRect) -> Result<DirectRect> {
    physical_box(
        rect.x as f32,
        rect.y as f32,
        rect.width as f32,
        rect.height as f32,
    )
}

fn physical_box(x: f32, y: f32, width: f32, height: f32) -> Result<DirectRect> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return Err(anyhow!("direct geometry is not finite"));
    }
    if width < 0.0 || height < 0.0 {
        return Err(anyhow!("direct geometry extent is negative"));
    }
    let origin_x = round_edge(x)?;
    let origin_y = round_edge(y)?;
    let right = round_edge(x + width)?;
    let bottom = round_edge(y + height)?;
    let allocated_width = right
        .checked_sub(origin_x)
        .ok_or_else(|| anyhow!("direct geometry width underflow"))?;
    let allocated_height = bottom
        .checked_sub(origin_y)
        .ok_or_else(|| anyhow!("direct geometry height underflow"))?;
    let terminal_max = i32::from(u16::MAX);
    if origin_x > terminal_max
        || origin_y > terminal_max
        || right > terminal_max
        || bottom > terminal_max
    {
        return Err(anyhow!("direct geometry exceeds terminal range"));
    }
    if allocated_width > i32::from(u16::MAX) || allocated_height > i32::from(u16::MAX) {
        return Err(anyhow!("direct geometry exceeds terminal range"));
    }
    Ok(DirectRect {
        rect: Rect::new(
            origin_x.max(0) as u16,
            origin_y.max(0) as u16,
            allocated_width as u16,
            allocated_height as u16,
        ),
        origin: (origin_x, origin_y),
    })
}

fn signed_intersection(
    origin: (i32, i32),
    size: crate::geometry::Size,
    clip: Rect,
) -> Option<Rect> {
    let left = origin.0.max(i32::from(clip.x)).max(0);
    let top = origin.1.max(i32::from(clip.y)).max(0);
    let right = origin
        .0
        .saturating_add(i32::from(size.width))
        .min(i32::from(clip.right()));
    let bottom = origin
        .1
        .saturating_add(i32::from(size.height))
        .min(i32::from(clip.bottom()));
    (left < right && top < bottom).then(|| {
        Rect::new(
            left as u16,
            top as u16,
            (right - left) as u16,
            (bottom - top) as u16,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LatchRelease(Option<std::sync::mpsc::Sender<()>>);

    impl LatchRelease {
        fn release(&mut self) {
            if let Some(release) = self.0.take() {
                let _ = release.send(());
            }
        }
    }

    impl Drop for LatchRelease {
        fn drop(&mut self) {
            self.release();
        }
    }

    fn editor_node(width: u16, height: u16) -> DirectNode {
        DirectNode {
            key: NodeKey {
                slot: 1,
                generation: 1,
            },
            snapshot: None,
            rect: Rect::new(0, 0, width, height),
            content_rect: Rect::new(0, 0, width, height),
            content_width: width,
            clip_rect: Rect::new(0, 0, width, height),
            paint_origin: (0, 0),
            content_origin: (0, 0),
            component: None,
            children: Vec::new(),
            style_states: Default::default(),
            style_facts: Default::default(),
            decoration: Default::default(),
            content: DirectContent::Children,
        }
    }

    fn editor(text: &str, cursor_bytes: usize, focused: bool) -> EditorSnapshot {
        EditorSnapshot {
            text: text.to_owned().into(),
            cursor_bytes,
            focused,
            multiline: true,
            scroll_row: 0,
            border: None,
        }
    }

    #[test]
    fn unfocused_editor_has_no_caret_and_soft_wrap_boundary_has_one() {
        let mut target = crate::physical::Surface::new(4, 2);
        let node = editor_node(4, 2);
        paint_editor(
            &editor("abc", 3, false),
            &node,
            crate::physical::PhysicalStyle::default(),
            &mut target,
            Rect::new(0, 0, 4, 2),
        );
        assert!(target.cells.iter().all(|cell| !cell.style.reversed));

        let mut target = crate::physical::Surface::new(4, 2);
        paint_editor(
            &editor("abcd", 3, true),
            &node,
            crate::physical::PhysicalStyle::default(),
            &mut target,
            Rect::new(0, 0, 4, 2),
        );
        let reversed = target
            .cells
            .iter()
            .filter(|cell| cell.style.reversed)
            .count();
        assert_eq!(reversed, 1, "soft-wrap boundary must paint one caret");
        assert_eq!(target.get(0, 0).grapheme.as_deref(), Some("a"));
        assert_eq!(target.get(1, 0).grapheme.as_deref(), Some("b"));
        assert_eq!(target.get(2, 0).grapheme.as_deref(), Some("c"));
        assert!(target.get(0, 1).style.reversed);
    }

    #[test]
    fn wide_editor_caret_reverses_only_the_grapheme_leader() {
        let mut target = crate::physical::Surface::new(3, 1);
        let node = editor_node(3, 1);
        paint_editor(
            &editor("界", 0, true),
            &node,
            crate::physical::PhysicalStyle::default(),
            &mut target,
            Rect::new(0, 0, 3, 1),
        );
        assert!(target.get(0, 0).style.reversed);
        assert!(!target.get(1, 0).style.reversed);
        assert!(target.get(1, 0).continuation);
    }

    #[test]
    fn editor_measurement_distinguishes_min_and_max_content() {
        let control = ControlSnapshot::Editor(Box::new(EditorSnapshot {
            text: "long word".to_owned().into(),
            cursor_bytes: 0,
            focused: false,
            multiline: true,
            scroll_row: 0,
            border: None,
        }));
        let request = |available_width| crate::presentation::taffy::MeasureRequest {
            known_width: None,
            known_height: None,
            available_width,
            available_height: AvailableConstraint::MaxContent,
            wrap_width: None,
        };
        assert_eq!(
            measure_control(&control, request(AvailableConstraint::MinContent), false)
                .expect("min-content measurement")
                .width,
            4.0
        );
        assert_eq!(
            measure_control(&control, request(AvailableConstraint::MaxContent), false)
                .expect("max-content measurement")
                .width,
            9.0
        );
        assert_eq!(
            measure_control(
                &control,
                request(AvailableConstraint::Definite(20.0)),
                false,
            )
            .expect("fit available measurement")
            .width,
            9.0
        );
        assert_eq!(
            measure_control(&control, request(AvailableConstraint::Definite(20.0)), true,)
                .expect("fill available measurement")
                .width,
            20.0
        );
    }

    #[test]
    fn bordered_editor_body_is_inset_and_zero_size_is_safe() {
        let mut target = crate::physical::Surface::new(4, 3);
        let mut node = editor_node(4, 3);
        let mut snapshot = editor("a", 0, false);
        snapshot.border = Some(crate::BorderSpec::plain());
        paint_editor(
            &snapshot,
            &node,
            crate::physical::PhysicalStyle::default(),
            &mut target,
            Rect::new(0, 0, 4, 3),
        );
        assert_eq!(target.get(1, 1).grapheme.as_deref(), Some("a"));
        assert_eq!(target.get(0, 0).grapheme, None);

        node.content_rect = Rect::new(0, 0, 0, 0);
        let mut empty = crate::physical::Surface::new(0, 0);
        paint_editor(
            &snapshot,
            &node,
            crate::physical::PhysicalStyle::default(),
            &mut empty,
            Rect::new(0, 0, 0, 0),
        );
    }

    #[test]
    fn direct_editor_paint_includes_native_border_and_label() {
        let mut root = editor_node(6, 3);
        let mut snapshot = editor("a", 0, false);
        snapshot.border = Some(crate::BorderSpec::plain().top_label("in"));
        root.content = DirectContent::Control(ControlSnapshot::Editor(Box::new(snapshot)));
        let mut tree = DirectTree {
            root: DirectNodeId(0),
            nodes: vec![
                DirectNode {
                    key: NodeKey {
                        slot: 0,
                        generation: 1,
                    },
                    snapshot: None,
                    rect: Rect::new(0, 0, 6, 3),
                    content_rect: Rect::new(0, 0, 6, 3),
                    content_width: 6,
                    clip_rect: Rect::new(0, 0, 6, 3),
                    paint_origin: (0, 0),
                    content_origin: (0, 0),
                    component: None,
                    children: vec![DirectNodeId(1)],
                    style_states: Default::default(),
                    style_facts: Default::default(),
                    decoration: Default::default(),
                    content: DirectContent::Children,
                },
                root,
            ],
            size: crate::geometry::Size::new(6, 3),
            physically_complete: true,
            content_roots: HashMap::new(),
            parents: Vec::new(),
        };
        tree.index();
        let layout = DirectLayout {
            tree,
            occurrence_geometry: HashMap::new(),
            content_widths: HashMap::new(),
            content_products: HashMap::new(),
            component_mounts: Vec::new(),
            history_overflow_rows: 0,
        };
        let surface = paint_direct_layout(
            &layout,
            &crate::Theme::default(),
            &crate::presentation::content::EmptyContentProvider,
            None,
            &crate::component::MountGraph::default(),
        )
        .expect("direct editor paint");
        assert_eq!(surface.get(0, 1).grapheme.as_deref(), Some("│"));
        assert_eq!(surface.get(1, 1).grapheme.as_deref(), Some("a"));
        assert_eq!(surface.get(0, 0).grapheme.as_deref(), Some("i"));
        assert_eq!(surface.get(1, 0).grapheme.as_deref(), Some("n"));
    }

    #[test]
    fn physical_box_rejects_positive_origin_outside_terminal_range() {
        assert!(physical_box(f32::from(u16::MAX) + 1.0, 0.0, 1.0, 1.0).is_err());
    }

    #[test]
    fn layout_request_is_nonblocking_while_real_driver_job_is_latched() {
        let driver = DirectDriverHandle::start(7).expect("direct driver startup");
        let root = NodeKey {
            slot: 1,
            generation: 1,
        };
        let snapshot = OccurrenceSnapshot {
            key: root,
            kind: HostKind::Box,
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
        driver
            .request_synchronize(
                vec![snapshot],
                None,
                vec![NodeParticipation {
                    key: root,
                    participates: true,
                }],
                HashMap::new(),
                vec![root],
                HashMap::new(),
                HashMap::new(),
            )
            .expect("direct synchronization request");
        while driver
            .poll_synchronize()
            .expect("direct synchronization poll")
            .is_none()
        {
            std::thread::yield_now();
        }
        let (entered, release) = driver.install_layout_latch_for_test();
        let mut release = LatchRelease(Some(release));
        driver
            .request_layout(
                root,
                crate::geometry::Size::new(4, 2),
                DirectHistoryAnchor::FollowEnd,
                HashMap::new(),
                Vec::new(),
                HashMap::new(),
                Arc::new(|| {}),
            )
            .expect("layout request admission");
        entered
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("layout worker entered latch");
        assert!(driver.poll_layout().expect("layout poll").is_none());
        release.release();
        let layout = loop {
            if let Some(layout) = driver.poll_layout().expect("layout completion poll") {
                break layout;
            }
            std::thread::yield_now();
        };
        driver.clear_layout_latch_for_test();
        assert_eq!(layout.tree.size, crate::geometry::Size::new(4, 2));
    }
}
