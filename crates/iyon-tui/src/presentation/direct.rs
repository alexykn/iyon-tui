//! Direct occurrence rendering over the derived Taffy layout.
//!
//! The occurrence document is the semantic owner. This module is the
//! renderer-driver side of that contract: a dedicated owner thread keeps the
//! thread-affine Taffy tree, accepts owned snapshots/products, and returns an
//! owned layout candidate. It never acquires `HostInner`, calls Source
//! selection, or invokes a content callback while laying out.

use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    thread::{self, JoinHandle},
};

use anyhow::{Result, anyhow};

use crate::{
    component::ComponentId,
    geometry::Rect,
    occurrence::{
        HostKind, LayerValue, NodeKey, OccurrenceSnapshot, PropertyId, PropertyValue, UiChangeSet,
    },
    presentation::{
        BorderSpec, BorderStyle, ContentMeasurement, StyleSpec, StyleStateKey, StyleStateValue,
        TextAttribute, View, layout::LayoutTree,
    },
    text::{
        TerminalConstraints, TerminalTextProduct, TerminalTextProjector, TextContent,
        TextRenderPolicy,
    },
};

use super::content::HistoryMeasurementAdjustment;
use super::layout::{
    ChildDependency, ComponentGeometry, LayoutContent, LayoutNode, LayoutNodeId, LayoutStyle,
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
#[derive(Debug)]
pub(crate) struct DirectLayout {
    pub(crate) tree: LayoutTree,
    pub(crate) occurrence_geometry: HashMap<NodeKey, ComponentGeometry>,
    pub(crate) content_widths: HashMap<NodeKey, f32>,
    pub(crate) content_products: HashMap<NodeKey, CapturedContentMeasurement>,
    pub(crate) component_mounts: Vec<(ComponentId, Option<ComponentId>)>,
    pub(crate) history_overflow_rows: usize,
}

/// Controls the placement of the active History suffix after a native
/// transfer.  FollowEnd keeps the current suffix bottom anchored; a blocked
/// native frontier must instead pin the active rows at the top of the
/// History track so the screen does not follow rows that were not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectHistoryAnchor {
    FollowEnd,
    NativeFrontier,
}

enum DirectDriverCommand {
    Synchronize {
        snapshots: Vec<OccurrenceSnapshot>,
        changes: Option<UiChangeSet>,
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
        control_views: HashMap<ComponentId, View>,
        intrinsic_control_views: HashMap<ComponentId, View>,
        response: SyncSender<Result<DirectLayout>>,
    },
    InvalidateContent {
        port_id: u64,
        response: SyncSender<Result<()>>,
    },
    InvalidateControl {
        component: ComponentId,
        response: SyncSender<Result<()>>,
    },
    Shutdown,
}

/// Send-only handle for the one concrete renderer driver. Taffy is created,
/// used, and destroyed by the worker; this handle is the only part retained
/// by the Send-safe host owner.
pub(crate) struct DirectDriverHandle {
    command: SyncSender<DirectDriverCommand>,
    join: Option<JoinHandle<()>>,
}

impl DirectDriverHandle {
    pub(crate) fn start(host_id: u64) -> Result<Self> {
        let (command, receive) = sync_channel(8);
        let (ready_send, ready_receive) = sync_channel(1);
        let join = thread::Builder::new()
            .name(format!("iyon-tui-layout-{host_id}"))
            .spawn(move || direct_driver_loop(receive, ready_send, host_id))
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
        })
    }

    pub(crate) fn synchronize(
        &self,
        snapshots: Vec<OccurrenceSnapshot>,
        changes: Option<&UiChangeSet>,
        participation: Vec<NodeParticipation>,
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<NodeKey>,
        portal_owners: HashMap<NodeKey, NodeKey>,
        controls: HashMap<crate::occurrence::ResourceKey, ComponentId>,
    ) -> Result<()> {
        let (response, receive) = sync_channel(1);
        self.command
            .send(DirectDriverCommand::Synchronize {
                snapshots,
                changes: changes.cloned(),
                participation,
                port_ids,
                roots,
                portal_owners,
                controls,
                response,
            })
            .map_err(|_| anyhow!("direct renderer driver is closed"))?;
        receive
            .recv()
            .map_err(|_| anyhow!("direct renderer driver dropped synchronization"))?
    }

    pub(crate) fn layout(
        &self,
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: Vec<NodeKey>,
        control_views: HashMap<ComponentId, View>,
    ) -> Result<DirectLayout> {
        self.layout_with_intrinsic(
            root,
            size,
            history_anchor,
            measurements,
            invalidate,
            control_views,
            HashMap::new(),
        )
    }

    pub(crate) fn layout_with_intrinsic(
        &self,
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: Vec<NodeKey>,
        control_views: HashMap<ComponentId, View>,
        intrinsic_control_views: HashMap<ComponentId, View>,
    ) -> Result<DirectLayout> {
        let (response, receive) = sync_channel(1);
        self.command
            .send(DirectDriverCommand::Layout {
                root,
                size,
                history_anchor,
                measurements,
                invalidate,
                control_views,
                intrinsic_control_views,
                response,
            })
            .map_err(|_| anyhow!("direct renderer driver is closed"))?;
        receive
            .recv()
            .map_err(|_| anyhow!("direct renderer driver dropped layout"))?
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
        let send_result = self
            .command
            .send(DirectDriverCommand::Shutdown)
            .map_err(|_| anyhow!("direct renderer driver already stopped"));
        let join_result = join
            .join()
            .map_err(|_| anyhow!("direct renderer driver panicked"));
        send_result.and(join_result)
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
) {
    let mut renderer = DirectOccurrenceRenderer::new(host_id);
    let _ = ready.send(Ok(()));
    while let Ok(command) = receive.recv() {
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
                    changes.as_ref(),
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
                control_views,
                intrinsic_control_views,
                response,
            } => {
                let _ = response.send(renderer.prepare_with_intrinsic(
                    root,
                    size,
                    history_anchor,
                    &measurements,
                    &invalidate,
                    &control_views,
                    &intrinsic_control_views,
                ));
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
            DirectDriverCommand::Shutdown => break,
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
        control_views: &HashMap<ComponentId, View>,
    ) -> Result<DirectLayout> {
        self.prepare_with_intrinsic(
            root,
            size,
            history_anchor,
            measurements,
            invalidate,
            control_views,
            &HashMap::new(),
        )
    }

    fn prepare_with_intrinsic(
        &mut self,
        root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        invalidate: &[NodeKey],
        control_views: &HashMap<ComponentId, View>,
        intrinsic_control_views: &HashMap<ComponentId, View>,
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
        let mut control_views_by_node = HashMap::new();
        let mut intrinsic_control_views_by_node = HashMap::new();
        for snapshot in self.snapshots.values() {
            let Some(component) = snapshot
                .control
                .and_then(|control| self.controls.get(&control).copied())
            else {
                continue;
            };
            let view = control_views
                .get(&component)
                .cloned()
                .ok_or_else(|| anyhow!("direct control view capture is missing"))?;
            control_views_by_node.insert(snapshot.key, view);
            if let Some(view) = intrinsic_control_views.get(&component) {
                intrinsic_control_views_by_node.insert(snapshot.key, view.clone());
            }
        }
        let (geometries, history_overflow_rows) = self.layout_roots_with_intrinsic(
            root,
            size,
            history_anchor,
            measurements,
            &control_views_by_node,
            &intrinsic_control_views_by_node,
        )?;
        self.build_layout_tree(
            root,
            size,
            &geometries,
            measurements,
            control_views,
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
        control_views: &HashMap<NodeKey, View>,
        intrinsic_control_views: &HashMap<NodeKey, View>,
    ) -> Result<Vec<crate::presentation::taffy::ComputedGeometry>> {
        let mut measurement_error = None;
        let mut failed_measurement_keys = Vec::new();
        let geometries = self
            .layout
            .layout(root, width, height, &mut |key, request| {
                match measured_for_request_with_intrinsic(
                    measurements.get(&key),
                    control_views.get(&key),
                    intrinsic_control_views.get(&key),
                    request,
                ) {
                    Ok(measured) => measured,
                    Err(error) => {
                        measurement_error = Some(error);
                        failed_measurement_keys.push(key);
                        MeasuredSize::default()
                    }
                }
            })
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
        control_views: &HashMap<NodeKey, View>,
    ) -> Result<(Vec<crate::presentation::taffy::ComputedGeometry>, usize)> {
        self.layout_roots_with_intrinsic(
            body_root,
            size,
            history_anchor,
            measurements,
            control_views,
            &HashMap::new(),
        )
    }

    fn layout_roots_with_intrinsic(
        &mut self,
        body_root: NodeKey,
        size: crate::geometry::Size,
        history_anchor: DirectHistoryAnchor,
        measurements: &HashMap<NodeKey, CapturedContentMeasurement>,
        control_views: &HashMap<NodeKey, View>,
        intrinsic_control_views: &HashMap<NodeKey, View>,
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
            control_views,
            intrinsic_control_views,
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
            control_views,
            intrinsic_control_views,
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
                control_views,
                intrinsic_control_views,
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
                control_views,
                intrinsic_control_views,
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
                control_views,
                intrinsic_control_views,
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
        control_views: &HashMap<ComponentId, View>,
        history_overflow_rows: usize,
    ) -> Result<DirectLayout> {
        let geometry_map = geometries
            .iter()
            .map(|geometry| (geometry.key, geometry))
            .collect();
        let mut nodes = Vec::with_capacity(geometries.len().saturating_add(1));
        let mut occurrence_geometry = HashMap::with_capacity(geometries.len());
        let mut content_widths = HashMap::new();
        nodes.push(LayoutNode {
            view_id: View::direct_root_id(self.driver_id),
            paint_cacheable: false,
            rect: Rect::new(0, 0, size.width, size.height),
            content_rect: Rect::new(0, 0, size.width, size.height),
            content_width: size.width,
            clip_rect: Rect::new(0, 0, size.width, size.height),
            paint_origin: (0, 0),
            content_origin: (0, 0),
            component: None,
            native_component_view: None,
            children: Vec::new(),
            child_dependencies: Vec::new(),
            style: LayoutStyle {
                component_scope: None,
                style_states: Default::default(),
                style_facts: Default::default(),
                decoration: Default::default(),
            },
            content: LayoutContent::Children,
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
                control_views,
                &mut occurrence_geometry,
                &mut content_widths,
                &mut nodes,
            )?);
        }
        nodes[0].children = root_children;
        nodes[0].child_dependencies = vec![ChildDependency::all(); nodes[0].children.len()];
        let mut tree = LayoutTree {
            root: LayoutNodeId(0),
            nodes,
            size,
            physically_complete: measurements
                .values()
                .all(|capture| capture.measurement.physically_complete),
            component_roots: HashMap::new(),
            parents: Vec::new(),
            content_roots: HashMap::new(),
            child_y_sorted: Vec::new(),
        };
        tree.index_component_roots();
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
        control_views: &HashMap<ComponentId, View>,
        occurrence_geometry: &mut HashMap<NodeKey, ComponentGeometry>,
        content_widths: &mut HashMap<NodeKey, f32>,
        nodes: &mut Vec<LayoutNode>,
    ) -> Result<LayoutNodeId> {
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
            .map(|capture| content_width_for_layout(snapshot, geometry, capture))
            .unwrap_or(geometry.logical_content_width);
        let content_width_cells = content_width.floor().clamp(0.0, f32::from(u16::MAX)) as u16;
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
        let native_component_view = (snapshot.kind == HostKind::Editor)
            .then(|| component)
            .flatten()
            .and_then(|component| control_views.get(&component).cloned());
        let id = LayoutNodeId(nodes.len());
        nodes.push(LayoutNode {
            view_id: View::direct_id(key),
            paint_cacheable: false,
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
            native_component_view,
            children: Vec::new(),
            child_dependencies: Vec::new(),
            style: LayoutStyle {
                component_scope: component,
                style_states: style_states(snapshot),
                style_facts: Default::default(),
                decoration: decoration(snapshot),
            },
            content: if snapshot.kind == HostKind::ContentHost && measurements.get(&key).is_none() {
                if visible {
                    return Err(anyhow!(
                        "active ContentHost measurement is missing; its owner must exclude the exported root"
                    ));
                }
                LayoutContent::Children
            } else {
                content(snapshot, measurements.get(&key))?
            },
        });
        let mut children = Vec::with_capacity(snapshot.children.len());
        for child in snapshot.children.iter().copied() {
            children.push(self.emit_node(
                child,
                Some(clip_rect),
                geometries,
                measurements,
                control_views,
                occurrence_geometry,
                content_widths,
                nodes,
            )?);
        }
        nodes[id.0].children = children;
        nodes[id.0].child_dependencies = vec![ChildDependency::all(); nodes[id.0].children.len()];
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
    control_view: Option<&View>,
    request: crate::presentation::taffy::MeasureRequest,
) -> Result<MeasuredSize> {
    measured_for_request_with_intrinsic(capture, control_view, None, request)
}

fn measured_for_request_with_intrinsic(
    capture: Option<&CapturedContentMeasurement>,
    control_view: Option<&View>,
    intrinsic_control_view: Option<&View>,
    request: crate::presentation::taffy::MeasureRequest,
) -> Result<MeasuredSize> {
    if capture.is_none() {
        let intrinsic_request = matches!(
            (request.known_width, request.available_width),
            (
                None,
                AvailableConstraint::MinContent | AvailableConstraint::MaxContent
            )
        );
        let view = if intrinsic_request {
            intrinsic_control_view.or(control_view)
        } else {
            control_view
        };
        let Some(view) = view else {
            // A childless ordinary Box has no intrinsic content. This is a
            // valid zero-sized leaf, unlike a missing ContentHost/control
            // capture, which is rejected at tree emission.
            return Ok(MeasuredSize::default());
        };
        let tree =
            crate::presentation::layout::layout_view(view, control_layout_constraints(request));
        let size = tree.node(tree.root).rect.size();
        let mut measured = MeasuredSize {
            width: f32::from(size.width),
            height: f32::from(size.height),
        };
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
    let mut measured = requested_product
        .as_ref()
        .map(|product| {
            let size = product.size();
            MeasuredSize {
                width: f32::from(size.width()),
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
/// silently routing through the removed View renderer.
fn content_product_for_request(
    capture: &CapturedContentMeasurement,
    request: crate::presentation::taffy::MeasureRequest,
) -> Result<Option<std::sync::Arc<TerminalTextProduct>>> {
    let Some(contents) = capture.semantic_contents.as_deref() else {
        return Ok(None);
    };
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
    let projector = TerminalTextProjector::new(capture.terminal_policy.clone());
    projector
        .project_contents(contents, constraints)
        .map(std::sync::Arc::new)
        .map(Some)
        .map_err(|error| anyhow!(error.to_string()))
}

fn floor_constraint_width(value: f32) -> Result<u16> {
    if !value.is_finite() || value < 0.0 {
        return Err(anyhow!("terminal content width is not finite"));
    }
    Ok(value.floor().min(f32::from(u16::MAX)) as u16)
}

fn control_layout_constraints(
    request: crate::presentation::taffy::MeasureRequest,
) -> crate::geometry::LayoutConstraints {
    match (request.known_width, request.available_width) {
        (Some(width), _) | (None, AvailableConstraint::Definite(width)) => {
            crate::geometry::LayoutConstraints::width_only(control_request_width(width))
        }
        (None, AvailableConstraint::MinContent | AvailableConstraint::MaxContent) => {
            crate::geometry::LayoutConstraints {
                width: crate::geometry::AxisConstraint::Unbounded,
                height: crate::geometry::AxisConstraint::Unbounded,
            }
        }
    }
}

fn control_request_width(value: f32) -> u16 {
    value.floor().clamp(0.0, f32::from(u16::MAX)) as u16
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
) -> Result<LayoutContent> {
    if snapshot.kind != HostKind::ContentHost {
        return Ok(LayoutContent::Children);
    }
    let capture = measurement.ok_or_else(|| anyhow!("ContentHost measurement is missing"))?;
    if capture.port_id == 0 {
        return Err(anyhow!("ContentPort identity is invalid"));
    }
    Ok(LayoutContent::ContentHost {
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

fn decoration(snapshot: &OccurrenceSnapshot) -> crate::presentation::ir::Decoration {
    let mut decoration = crate::presentation::ir::Decoration::default();
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
            (PropertyId::Padding, PropertyValue::Insets(insets)) => decoration.padding = *insets,
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
    use crate::occurrence::{DimensionValue, FiniteScalar, LayoutMode, NodeRef};
    use std::sync::{Arc, Mutex};

    fn direct_history_layout(heights: &[f32], viewport_height: u16) -> Result<DirectLayout> {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut commit = crate::occurrence::UiCommit::new(0);
        for (index, height) in heights.iter().copied().enumerate() {
            let ordinal = u32::try_from(index + 1).expect("History ordinal");
            commit.push(crate::occurrence::UiOperation::CreateRoot {
                local_ordinal: ordinal,
                role: crate::occurrence::RootRole::LegacyHistoryUnit,
                owner: None,
            });
            commit.push(crate::occurrence::UiOperation::SetDeclared {
                node: NodeRef::Local(ordinal),
                property: PropertyId::Height,
                value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                    FiniteScalar::new(height).expect("History height"),
                ))),
            });
        }
        let created = document
            .commit_ui(&commit)
            .expect("History roots")
            .acknowledgement
            .created;
        let history_roots = (0..heights.len())
            .map(|index| created[index].node_key().expect("History root"))
            .collect::<Vec<_>>();
        let mut snapshots = vec![document.snapshot(body).expect("body snapshot")];
        snapshots.extend(
            history_roots
                .iter()
                .copied()
                .map(|root| document.snapshot(root).expect("History snapshot")),
        );
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(7);
        let mut roots = history_roots;
        roots.push(body);
        renderer.synchronize(
            snapshots,
            None,
            &participation,
            HashMap::new(),
            roots,
            HashMap::new(),
            HashMap::new(),
        )?;
        renderer.prepare(
            body,
            crate::geometry::Size::new(20, viewport_height),
            DirectHistoryAnchor::FollowEnd,
            &HashMap::new(),
            &[],
            &HashMap::new(),
        )
    }

    #[test]
    fn direct_occurrence_layout_uses_taffy_for_row_geometry() {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut commit = crate::occurrence::UiCommit::new(0);
        for ordinal in 1..=3 {
            commit.push(crate::occurrence::UiOperation::CreateNode {
                local_ordinal: ordinal,
                kind: HostKind::Box,
            });
        }
        commit.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Existing(body.handle(namespace)),
            child: NodeRef::Local(1),
            before: None,
        });
        for ordinal in 2..=3 {
            commit.push(crate::occurrence::UiOperation::InsertBefore {
                parent: NodeRef::Local(1),
                child: NodeRef::Local(ordinal),
                before: None,
            });
            commit.push(crate::occurrence::UiOperation::SetDeclared {
                node: NodeRef::Local(ordinal),
                property: PropertyId::Width,
                value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                    FiniteScalar::new(3.0).expect("width"),
                ))),
            });
        }
        commit.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(1),
            property: PropertyId::Layout,
            value: LayerValue::Value(PropertyValue::LayoutMode(LayoutMode::Row)),
        });
        document.commit_ui(&commit).expect("accepted direct tree");
        let mut all = vec![document.snapshot(body).expect("body")];
        let mut index = 0;
        while index < all.len() {
            let children = all[index].children.clone();
            for child in children {
                all.push(document.snapshot(child).expect("child"));
            }
            index += 1;
        }
        let participation = all
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(1);
        renderer
            .synchronize(
                all,
                None,
                &participation,
                HashMap::new(),
                vec![body],
                HashMap::new(),
                HashMap::new(),
            )
            .expect("sync");
        let layout = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &HashMap::new(),
                &[],
                &HashMap::new(),
            )
            .expect("layout");
        let body_id = layout.tree.nodes[0].children[0];
        assert_eq!(layout.history_overflow_rows, 0);
        let row_id = layout.tree.nodes[body_id.0].children[0];
        let row_children = &layout.tree.nodes[row_id.0].children;
        assert_eq!(row_children.len(), 2);
        assert_eq!(layout.tree.nodes[row_children[0].0].rect.x, 0);
        assert_eq!(layout.tree.nodes[row_children[1].0].rect.x, 3);
    }

    #[test]
    fn direct_measurement_requests_use_captured_min_max_and_known_dimensions() {
        let capture = CapturedContentMeasurement {
            capture_id: 1,
            port_id: 7,
            offered_width: 20,
            measurement: ContentMeasurement {
                intrinsic_size: crate::geometry::Size::new(6, 3),
                ..ContentMeasurement::default()
            },
            min_content: crate::geometry::Size::new(2, 1),
            max_content: crate::geometry::Size::new(8, 4),
            history_adjustment: None,
            semantic_contents: None,
            terminal_policy: TextRenderPolicy::default(),
            terminal_product: None,
        };
        let min = measured_for_request(
            Some(&capture),
            None,
            crate::presentation::taffy::MeasureRequest {
                known_width: None,
                known_height: None,
                available_width: AvailableConstraint::MinContent,
                available_height: AvailableConstraint::MinContent,
                wrap_width: None,
            },
        )
        .expect("min-content measurement");
        assert_eq!(
            min,
            MeasuredSize {
                width: 2.0,
                height: 1.0
            }
        );
        let max = measured_for_request(
            Some(&capture),
            None,
            crate::presentation::taffy::MeasureRequest {
                known_width: Some(5.0),
                known_height: Some(9.0),
                available_width: AvailableConstraint::MaxContent,
                available_height: AvailableConstraint::MaxContent,
                wrap_width: None,
            },
        )
        .expect("definite measurement");
        assert_eq!(
            max,
            MeasuredSize {
                width: 5.0,
                height: 9.0
            }
        );
    }

    #[test]
    fn direct_editor_intrinsic_measurement_preserves_multiline_text() {
        let mut editor = crate::TextInput::new().multiline(true);
        editor.set_text("abc\ndefgh");
        let view = crate::Component::view(&editor);
        let block = crate::presentation::layout::compile_view(&view, 5);
        assert_eq!(
            block
                .rows
                .iter()
                .map(|row| row.plain_text())
                .collect::<Vec<_>>(),
            ["abc", "defgh"]
        );
        for available_width in [
            AvailableConstraint::MinContent,
            AvailableConstraint::MaxContent,
        ] {
            let measured = measured_for_request(
                None,
                Some(&view),
                crate::presentation::taffy::MeasureRequest {
                    known_width: None,
                    known_height: None,
                    available_width,
                    available_height: AvailableConstraint::MaxContent,
                    wrap_width: None,
                },
            )
            .expect("intrinsic editor measurement");
            assert_eq!(
                measured,
                MeasuredSize {
                    width: 5.0,
                    height: 2.0,
                }
            );
        }
        let zero_width = measured_for_request(
            None,
            Some(&view),
            crate::presentation::taffy::MeasureRequest {
                known_width: Some(0.0),
                known_height: None,
                available_width: AvailableConstraint::Definite(0.0),
                available_height: AvailableConstraint::MaxContent,
                wrap_width: Some(0),
            },
        )
        .expect("zero-width editor measurement");
        assert_eq!(zero_width.width, 0.0);
        assert_eq!(zero_width.height, 2.0);

        crate::controls::text_input::TextInput::layout_changed(
            &mut editor,
            crate::geometry::Size::new(20, 4),
        );
        let allocated_view = crate::Component::view(&editor);
        let intrinsic_view = editor.intrinsic_view();
        let allocated_block = crate::presentation::layout::compile_view(&allocated_view, 5);
        assert_eq!(
            allocated_block
                .rows
                .iter()
                .map(|row| row.plain_text())
                .collect::<Vec<_>>(),
            ["abc", "defgh"]
        );
        let allocated_intrinsic = measured_for_request_with_intrinsic(
            None,
            Some(&allocated_view),
            Some(&intrinsic_view),
            crate::presentation::taffy::MeasureRequest {
                known_width: None,
                known_height: None,
                available_width: AvailableConstraint::MaxContent,
                available_height: AvailableConstraint::MaxContent,
                wrap_width: None,
            },
        )
        .expect("allocated editor intrinsic measurement");
        assert_eq!(allocated_intrinsic.width, 5.0);

        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut mount = crate::occurrence::UiCommit::new(0);
        mount.push(crate::occurrence::UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(crate::occurrence::UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::Editor,
        });
        mount.push(crate::occurrence::UiOperation::CreateControl {
            local_ordinal: 3,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(2)),
        });
        mount.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Existing(body.handle(namespace)),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(1),
            property: PropertyId::Layout,
            value: LayerValue::Value(PropertyValue::LayoutMode(LayoutMode::Row)),
        });
        mount.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(2),
            property: PropertyId::Width,
            value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Auto)),
        });
        let created = document
            .commit_ui(&mount)
            .expect("direct Editor occurrence")
            .acknowledgement
            .created;
        let wrapper_node = created[0].node_key().expect("wrapper node");
        let editor_node = created[1].node_key().expect("Editor node");
        let control = created[2].resource_key().expect("Editor control");
        let snapshots = vec![
            document.snapshot(body).expect("body snapshot"),
            document.snapshot(wrapper_node).expect("wrapper snapshot"),
            document.snapshot(editor_node).expect("Editor snapshot"),
        ];
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let component = ComponentId::from_raw(1);
        let mut renderer = DirectOccurrenceRenderer::new(8);
        renderer
            .synchronize(
                snapshots,
                None,
                &participation,
                HashMap::new(),
                vec![body],
                HashMap::new(),
                HashMap::from([(control, component)]),
            )
            .expect("direct Editor synchronization");
        let direct = renderer
            .prepare_with_intrinsic(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &HashMap::new(),
                &[],
                &HashMap::from([(component, allocated_view)]),
                &HashMap::from([(component, intrinsic_view)]),
            )
            .expect("direct Editor intrinsic layout");
        assert_eq!(direct.occurrence_geometry[&editor_node].outer.width, 5);
    }

    #[test]
    fn direct_semantic_measurement_can_grow_at_a_narrower_width() {
        let capture = CapturedContentMeasurement {
            capture_id: 1,
            port_id: 7,
            offered_width: 8,
            measurement: ContentMeasurement {
                intrinsic_size: crate::geometry::Size::new(8, 1),
                ..ContentMeasurement::default()
            },
            min_content: crate::geometry::Size::new(1, 8),
            max_content: crate::geometry::Size::new(8, 1),
            history_adjustment: None,
            semantic_contents: Some(vec![TextContent::raw("abcdefgh")].into()),
            terminal_policy: TextRenderPolicy::default(),
            terminal_product: None,
        };
        let measured = measured_for_request(
            Some(&capture),
            None,
            crate::presentation::taffy::MeasureRequest {
                known_width: Some(4.0),
                known_height: None,
                available_width: AvailableConstraint::Definite(4.0),
                available_height: AvailableConstraint::MaxContent,
                wrap_width: Some(4),
            },
        )
        .expect("narrow terminal measurement");
        assert_eq!(measured.height, 2.0);
    }

    #[test]
    fn direct_semantic_measurement_ignores_unmatched_history_adjustment() {
        let capture = CapturedContentMeasurement {
            capture_id: 1,
            port_id: 7,
            offered_width: 8,
            measurement: ContentMeasurement {
                intrinsic_size: crate::geometry::Size::new(8, 1),
                projection_identity: 7,
                ..ContentMeasurement::default()
            },
            min_content: crate::geometry::Size::new(1, 8),
            max_content: crate::geometry::Size::new(8, 1),
            history_adjustment: Some(HistoryMeasurementAdjustment {
                projection_identity: 99,
                offered_width: 8,
                removed_rows: 1,
            }),
            semantic_contents: Some(vec![TextContent::raw("abcdefgh")].into()),
            terminal_policy: TextRenderPolicy::default(),
            terminal_product: None,
        };
        let measured = measured_for_request(
            Some(&capture),
            None,
            crate::presentation::taffy::MeasureRequest {
                known_width: Some(4.0),
                known_height: None,
                available_width: AvailableConstraint::Definite(4.0),
                available_height: AvailableConstraint::MaxContent,
                wrap_width: Some(4),
            },
        )
        .expect("measurement");
        assert_eq!(measured.height, 2.0);
    }

    #[test]
    fn terminal_projection_errors_reach_the_direct_measurement_boundary() {
        let contents: std::sync::Arc<[TextContent]> =
            vec![TextContent::raw("x\n".repeat(usize::from(u16::MAX) + 1))].into();
        let capture = CapturedContentMeasurement {
            capture_id: 1,
            port_id: 7,
            offered_width: 1,
            measurement: ContentMeasurement::default(),
            min_content: crate::geometry::Size::new(1, 1),
            max_content: crate::geometry::Size::new(1, 1),
            history_adjustment: None,
            semantic_contents: Some(contents),
            terminal_policy: TextRenderPolicy::default(),
            terminal_product: None,
        };
        let error = measured_for_request(
            Some(&capture),
            None,
            crate::presentation::taffy::MeasureRequest {
                known_width: Some(1.0),
                known_height: None,
                available_width: AvailableConstraint::Definite(1.0),
                available_height: AvailableConstraint::MaxContent,
                wrap_width: Some(1),
            },
        )
        .expect_err("an overflowing terminal product must fail measurement");
        let message = error.to_string();
        assert!(
            message.contains("terminal") && message.contains("exceeds u16"),
            "unexpected terminal projection error: {message}"
        );
    }

    #[test]
    fn direct_driver_owns_taffy_until_explicit_shutdown() {
        let mut driver = DirectDriverHandle::start(0xfeed).expect("driver startup");
        driver.shutdown().expect("driver shutdown");
    }

    #[test]
    fn signed_geometry_is_clipped_before_terminal_conversion() {
        let box_rect = physical_box(-2.0, -1.0, 5.0, 4.0).expect("signed box");
        assert_eq!(box_rect.rect, Rect::new(0, 0, 5, 4));
        assert_eq!(box_rect.origin, (-2, -1));
        assert_eq!(
            signed_intersection(
                box_rect.origin,
                box_rect.rect.size(),
                Rect::new(0, 0, 20, 20)
            ),
            Some(Rect::new(0, 0, 3, 3))
        );
        assert!(physical_box(0.0, 0.0, -1.0, 2.0).is_err());
    }

    #[test]
    fn failed_content_measurement_dirties_taffy_before_retry_and_paint() {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut commit = crate::occurrence::UiCommit::new(0);
        commit.push(crate::occurrence::UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        commit.push(crate::occurrence::UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        commit.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Existing(body.handle(namespace)),
            child: NodeRef::Local(1),
            before: None,
        });
        commit.push(crate::occurrence::UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(crate::occurrence::ResourceRef::Local(2)),
        });
        let ack = document.commit_ui(&commit).expect("content occurrence");
        let node = ack.acknowledgement.created[0].node_key().expect("node");
        let port = ack.acknowledgement.created[1].resource_key().expect("port");
        let mut snapshots = vec![document.snapshot(body).expect("body")];
        snapshots.push(document.snapshot(node).expect("content"));
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(3);
        renderer
            .synchronize(
                snapshots,
                None,
                &participation,
                HashMap::from([(port, 9)]),
                vec![body],
                HashMap::new(),
                HashMap::new(),
            )
            .expect("direct synchronization");
        let overflowing_contents: std::sync::Arc<[TextContent]> =
            vec![TextContent::raw("x\n".repeat(usize::from(u16::MAX) + 1))].into();
        let mut captures = HashMap::new();
        captures.insert(
            node,
            CapturedContentMeasurement {
                capture_id: 1,
                port_id: 9,
                offered_width: 20,
                measurement: ContentMeasurement {
                    ..ContentMeasurement::default()
                },
                min_content: crate::geometry::Size::new(1, 1),
                max_content: crate::geometry::Size::new(1, 1),
                history_adjustment: None,
                semantic_contents: Some(overflowing_contents),
                terminal_policy: TextRenderPolicy::default(),
                terminal_product: None,
            },
        );
        let error = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &captures,
                &[],
                &HashMap::new(),
            )
            .expect_err("projection failure must reject the direct candidate");
        assert!(error.to_string().contains("terminal content measurement"));

        let valid_contents: std::sync::Arc<[TextContent]> =
            vec![TextContent::raw("recovered")].into();
        let valid_product = std::sync::Arc::new(
            TerminalTextProjector::new(TextRenderPolicy::default())
                .project_contents(valid_contents.as_ref(), TerminalConstraints::definite(20))
                .expect("valid terminal product"),
        );
        captures.insert(
            node,
            CapturedContentMeasurement {
                capture_id: 2,
                port_id: 9,
                offered_width: 20,
                measurement: ContentMeasurement {
                    intrinsic_size: crate::geometry::Size::new(9, 1),
                    physically_complete: true,
                    connector_id: Some(9),
                    projection_identity: 2,
                    ..ContentMeasurement::default()
                },
                min_content: crate::geometry::Size::new(9, 1),
                max_content: crate::geometry::Size::new(9, 1),
                history_adjustment: None,
                semantic_contents: Some(valid_contents),
                terminal_policy: TextRenderPolicy::default(),
                terminal_product: Some(valid_product.clone()),
            },
        );
        let layout = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &captures,
                &[],
                &HashMap::new(),
            )
            .expect("valid retry must recompute the dirtied content leaf");
        assert!(layout.tree.physically_complete);
        let content_node = layout.tree.content_roots[&9][0];
        assert!(layout.tree.node(content_node).content_rect.height > 0);

        struct ProductProvider {
            product: std::sync::Arc<TerminalTextProduct>,
        }

        impl crate::presentation::ContentProvider for ProductProvider {
            fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
                1
            }

            fn measure(
                &mut self,
                _port_id: u64,
                _offered_width: u16,
                _width_rule: crate::presentation::WidthRule,
            ) -> ContentMeasurement {
                ContentMeasurement {
                    intrinsic_size: crate::geometry::Size::new(9, 1),
                    physically_complete: true,
                    connector_id: Some(9),
                    projection_identity: 2,
                    ..ContentMeasurement::default()
                }
            }

            fn paint_window(
                &self,
                _ticket: crate::presentation::PreparedProjectionTicket,
                window: crate::presentation::ContentWindow,
                target: &mut crate::physical::Surface,
                target_origin: (u16, u16),
                clip: crate::geometry::Rect,
                style: crate::physical::PhysicalStyle,
            ) {
                self.product
                    .paint_window(
                        &crate::Theme::default(),
                        style,
                        target,
                        (i32::from(target_origin.0), i32::from(target_origin.1)),
                        clip,
                        crate::text::TerminalRowWindow::new(
                            usize::try_from(window.first_row).expect("row index"),
                            usize::try_from(window.row_count).expect("row count"),
                        ),
                    )
                    .expect("captured product paints");
            }
        }

        let mut paint_cache = crate::presentation::paint::PaintCache::default();
        let compiler = crate::presentation::layout::ViewCompiler::default();
        let surface = crate::presentation::paint::ViewPainter.paint_tree_with_content(
            &compiler,
            &layout.tree,
            &mut paint_cache,
            &ProductProvider {
                product: valid_product,
            },
        );
        let row = crate::physical::PhysicalRow::from_cells(surface.row_cells(3).to_vec());
        assert_eq!(row.plain_text(), "recovered");
    }

    #[test]
    fn direct_history_overflow_budget_covers_no_under_exact_and_over_capacity() -> Result<()> {
        for (heights, viewport, expected) in [
            (&[][..], 4, 0),
            (&[1.5][..], 4, 0),
            (&[2.0][..], 2, 0),
            (&[2.6, 1.9][..], 4, 1),
        ] {
            let layout = direct_history_layout(heights, viewport)?;
            assert_eq!(layout.history_overflow_rows, expected);
        }
        Ok(())
    }

    #[test]
    fn direct_history_roots_share_fractional_edge_rounding() -> Result<()> {
        let layout = direct_history_layout(&[0.6, 0.6], 2)?;
        let history_nodes = layout
            .tree
            .nodes
            .iter()
            .filter(|node| node.view_id != View::direct_root_id(7))
            .collect::<Vec<_>>();
        assert_eq!(layout.history_overflow_rows, 0);
        assert_eq!(history_nodes[0].rect.y, 1);
        assert_eq!(history_nodes[0].rect.height, 0);
        assert_eq!(history_nodes[1].rect.y, 1);
        assert_eq!(history_nodes[1].rect.height, 1);
        Ok(())
    }

    #[test]
    fn direct_row_respects_declared_flex_shrink_across_width_and_height() -> Result<()> {
        let body = NodeKey {
            slot: 70,
            generation: 1,
        };
        let row = NodeKey {
            slot: 71,
            generation: 1,
        };
        let first = NodeKey {
            slot: 72,
            generation: 1,
        };
        let second = NodeKey {
            slot: 73,
            generation: 1,
        };
        let scalar = |value| crate::occurrence::FiniteScalar::new(value).expect("finite value");
        let length = |value| {
            LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(scalar(
                value,
            ))))
        };
        let snapshot = |key, children, root_role, properties| OccurrenceSnapshot {
            key,
            kind: HostKind::Box,
            root_role,
            children,
            port: None,
            control: None,
            hidden: false,
            subscriptions: 0,
            history_action: None,
            properties,
            style_states: Vec::new(),
            structure_revision: 1,
            geometry_revision: 1,
            presentation_revision: 0,
            interaction_revision: 0,
        };
        let snapshots = vec![
            snapshot(
                body,
                vec![row],
                Some(crate::occurrence::RootRole::Body),
                Vec::new(),
            ),
            snapshot(
                row,
                vec![first, second],
                None,
                vec![
                    (
                        PropertyId::Layout,
                        LayerValue::Value(PropertyValue::LayoutMode(LayoutMode::Row)),
                    ),
                    (PropertyId::Width, length(6.0)),
                    (PropertyId::Height, length(2.0)),
                ],
            ),
            snapshot(
                first,
                Vec::new(),
                None,
                vec![
                    (PropertyId::Width, length(5.0)),
                    (PropertyId::Height, length(2.0)),
                    (
                        PropertyId::FlexShrink,
                        LayerValue::Value(PropertyValue::Scalar(scalar(0.0))),
                    ),
                ],
            ),
            snapshot(
                second,
                Vec::new(),
                None,
                vec![
                    (PropertyId::Width, length(5.0)),
                    (PropertyId::Height, length(2.0)),
                ],
            ),
        ];
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(70);
        renderer.synchronize(
            snapshots,
            None,
            &participation,
            HashMap::new(),
            vec![body],
            HashMap::new(),
            HashMap::new(),
        )?;
        let layout = renderer.prepare(
            body,
            crate::geometry::Size::new(8, 4),
            DirectHistoryAnchor::FollowEnd,
            &HashMap::new(),
            &[],
            &HashMap::new(),
        )?;
        let first_rect = layout.occurrence_geometry[&first].outer;
        let second_rect = layout.occurrence_geometry[&second].outer;
        assert_eq!(first_rect.width, 5);
        assert_eq!(second_rect.width, 1);
        assert_eq!(first_rect.height, 2);
        assert_eq!(second_rect.height, 2);
        assert_eq!(first_rect.x + first_rect.width, second_rect.x);
        Ok(())
    }

    #[test]
    fn direct_content_paint_ticket_uses_resolved_width_for_explicit_and_fill() -> Result<()> {
        struct TicketProvider {
            tickets: Arc<Mutex<Vec<crate::presentation::PreparedProjectionTicket>>>,
        }
        impl crate::presentation::ContentProvider for TicketProvider {
            fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
                1
            }

            fn measure(
                &mut self,
                _port_id: u64,
                _offered_width: u16,
                _width_rule: crate::presentation::WidthRule,
            ) -> ContentMeasurement {
                ContentMeasurement::default()
            }

            fn paint_window(
                &self,
                ticket: crate::presentation::PreparedProjectionTicket,
                _window: crate::presentation::ContentWindow,
                _target: &mut crate::physical::Surface,
                _target_origin: (u16, u16),
                _clip: crate::geometry::Rect,
                _style: crate::physical::PhysicalStyle,
            ) {
                self.tickets.lock().unwrap().push(ticket);
            }
        }

        let scalar = |value| crate::occurrence::FiniteScalar::new(value).expect("finite value");
        let length = |value| {
            LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(scalar(
                value,
            ))))
        };
        let snapshot = |key, children, root_role, properties| OccurrenceSnapshot {
            key,
            kind: HostKind::Box,
            root_role,
            children,
            port: None,
            control: None,
            hidden: false,
            subscriptions: 0,
            history_action: None,
            properties,
            style_states: Vec::new(),
            structure_revision: 1,
            geometry_revision: 1,
            presentation_revision: 0,
            interaction_revision: 0,
        };
        let render_case = |parent_properties, content_properties| -> Result<DirectLayout> {
            let body = NodeKey {
                slot: 80,
                generation: 1,
            };
            let parent = NodeKey {
                slot: 81,
                generation: 1,
            };
            let content = NodeKey {
                slot: 82,
                generation: 1,
            };
            let snapshots = vec![
                snapshot(
                    body,
                    vec![parent],
                    Some(crate::occurrence::RootRole::Body),
                    Vec::new(),
                ),
                snapshot(parent, vec![content], None, parent_properties),
                {
                    let mut value = snapshot(content, Vec::new(), None, content_properties);
                    value.kind = HostKind::ContentHost;
                    value
                },
            ];
            let participation = snapshots
                .iter()
                .map(|snapshot| NodeParticipation {
                    key: snapshot.key,
                    participates: true,
                })
                .collect::<Vec<_>>();
            let mut renderer = DirectOccurrenceRenderer::new(80);
            renderer.synchronize(
                snapshots,
                None,
                &participation,
                HashMap::new(),
                vec![body],
                HashMap::new(),
                HashMap::new(),
            )?;
            let capture = CapturedContentMeasurement {
                capture_id: 1,
                port_id: 9,
                offered_width: 80,
                measurement: ContentMeasurement {
                    intrinsic_size: crate::geometry::Size::new(8, 1),
                    physically_complete: true,
                    projection_revision: 1,
                    metric_revision: 1,
                    paint_revision: 1,
                    connector_id: Some(5),
                    projection_identity: 9,
                },
                min_content: crate::geometry::Size::new(8, 1),
                max_content: crate::geometry::Size::new(8, 1),
                history_adjustment: None,
                semantic_contents: Some(vec![TextContent::raw("abcdefgh")].into()),
                terminal_policy: TextRenderPolicy::default(),
                terminal_product: None,
            };
            renderer.prepare(
                body,
                crate::geometry::Size::new(80, 6),
                DirectHistoryAnchor::FollowEnd,
                &HashMap::from([(content, capture)]),
                &[],
                &HashMap::new(),
            )
        };

        let explicit = render_case(
            vec![
                (PropertyId::Width, length(8.0)),
                (PropertyId::Height, length(2.0)),
            ],
            vec![(PropertyId::Width, length(4.0))],
        )?;
        let padded_fill = render_case(
            vec![
                (PropertyId::Width, length(8.0)),
                (
                    PropertyId::Padding,
                    LayerValue::Value(PropertyValue::Insets(crate::presentation::Insets::new(
                        0, 2, 0, 2,
                    ))),
                ),
            ],
            vec![(
                PropertyId::Width,
                LayerValue::Value(PropertyValue::SizeMode(crate::occurrence::SizeMode::Fill)),
            )],
        )?;
        let cases = [explicit, padded_fill];
        for layout in cases {
            let content = layout
                .tree
                .nodes
                .iter()
                .find(|node| {
                    node.view_id
                        == View::direct_id(NodeKey {
                            slot: 82,
                            generation: 1,
                        })
                })
                .expect("ContentHost layout node");
            let logical_width = layout.occurrence_geometry[&NodeKey {
                slot: 82,
                generation: 1,
            }]
                .content
                .width;
            assert_eq!(content.content_width, logical_width);
            let tickets = Arc::new(Mutex::new(Vec::new()));
            let provider = TicketProvider {
                tickets: Arc::clone(&tickets),
            };
            let theme = crate::Theme::new();
            let compiler = crate::presentation::layout::ViewCompiler::new(&theme);
            let mut cache = crate::presentation::paint::PaintCache::default();
            cache.begin_epoch(&theme);
            let _ = crate::presentation::paint::ViewPainter.paint_tree_with_content(
                &compiler,
                &layout.tree,
                &mut cache,
                &provider,
            );
            assert!(!tickets.lock().unwrap().is_empty());
            assert!(
                tickets
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|ticket| ticket.offered_width == logical_width)
            );
        }
        Ok(())
    }

    #[test]
    fn direct_layout_rejects_a_missing_body_root_role() {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut snapshot = document.snapshot(body).expect("body snapshot");
        snapshot.root_role = None;
        let participation = vec![NodeParticipation {
            key: body,
            participates: true,
        }];
        let mut renderer = DirectOccurrenceRenderer::new(4);
        renderer
            .synchronize(
                vec![snapshot],
                None,
                &participation,
                HashMap::new(),
                vec![body],
                HashMap::new(),
                HashMap::new(),
            )
            .expect("direct synchronization");
        let error = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &HashMap::new(),
                &[],
                &HashMap::new(),
            )
            .expect_err("missing Body role must be explicit");
        assert!(error.to_string().contains("no Body root"));
    }

    #[test]
    fn direct_native_frontier_anchor_places_active_history_at_the_top() {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut commit = crate::occurrence::UiCommit::new(0);
        commit.push(crate::occurrence::UiOperation::CreateRoot {
            local_ordinal: 1,
            role: crate::occurrence::RootRole::LegacyHistoryUnit,
            owner: None,
        });
        commit.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(1),
            property: PropertyId::Height,
            value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                FiniteScalar::new(2.0).expect("height"),
            ))),
        });
        let history = document
            .commit_ui(&commit)
            .expect("History root")
            .acknowledgement
            .created[0]
            .node_key()
            .expect("History key");
        let snapshots = vec![
            document.snapshot(body).expect("body snapshot"),
            document.snapshot(history).expect("History snapshot"),
        ];
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(5);
        renderer
            .synchronize(
                snapshots,
                None,
                &participation,
                HashMap::new(),
                vec![history, body],
                HashMap::new(),
                HashMap::new(),
            )
            .expect("direct synchronization");
        let follow_end = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::FollowEnd,
                &HashMap::new(),
                &[],
                &HashMap::new(),
            )
            .expect("FollowEnd layout");
        let native_frontier = renderer
            .prepare(
                body,
                crate::geometry::Size::new(20, 4),
                DirectHistoryAnchor::NativeFrontier,
                &HashMap::new(),
                &[],
                &HashMap::new(),
            )
            .expect("NativeFrontier layout");
        let history_rect = |layout: &DirectLayout| {
            layout
                .tree
                .nodes
                .iter()
                .find(|node| node.view_id == View::direct_id(history))
                .expect("History node")
                .rect
        };
        assert_eq!(history_rect(&follow_end).y, 2);
        assert_eq!(history_rect(&native_frontier).y, 0);
    }

    #[test]
    fn direct_nested_portal_order_follows_descendant_owner_dependency() -> Result<()> {
        let namespace = crate::occurrence::HostNamespace::allocate().expect("namespace");
        let mut document = crate::occurrence::OccurrenceDocument::new(namespace);
        let body = document.body_root();
        let mut commit = crate::occurrence::UiCommit::new(0);
        commit.push(crate::occurrence::UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        commit.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Existing(body.handle(namespace)),
            child: NodeRef::Local(1),
            before: None,
        });
        commit.push(crate::occurrence::UiOperation::CreateRoot {
            local_ordinal: 2,
            role: crate::occurrence::RootRole::Portal,
            owner: Some(NodeRef::Existing(body.handle(namespace))),
        });
        commit.push(crate::occurrence::UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Box,
        });
        commit.push(crate::occurrence::UiOperation::InsertBefore {
            parent: NodeRef::Local(2),
            child: NodeRef::Local(3),
            before: None,
        });
        commit.push(crate::occurrence::UiOperation::CreateRoot {
            local_ordinal: 4,
            role: crate::occurrence::RootRole::Portal,
            owner: Some(NodeRef::Local(3)),
        });
        commit.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(2),
            property: PropertyId::Height,
            value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                FiniteScalar::new(2.0).expect("outer height"),
            ))),
        });
        commit.push(crate::occurrence::UiOperation::SetDeclared {
            node: NodeRef::Local(4),
            property: PropertyId::Height,
            value: LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                FiniteScalar::new(1.0).expect("inner height"),
            ))),
        });
        let created = document
            .commit_ui(&commit)
            .expect("nested portals")
            .acknowledgement
            .created;
        let body_child = created[0].node_key().expect("body child");
        let outer = created[1].node_key().expect("outer portal");
        let outer_child = created[2].node_key().expect("outer child");
        let inner = created[3].node_key().expect("inner portal");
        let snapshots = [
            document.snapshot(body).expect("body snapshot"),
            document.snapshot(body_child).expect("body child snapshot"),
            document.snapshot(outer).expect("outer snapshot"),
            document
                .snapshot(outer_child)
                .expect("outer child snapshot"),
            document.snapshot(inner).expect("inner snapshot"),
        ]
        .into_iter()
        .collect::<Vec<_>>();
        let participation = snapshots
            .iter()
            .map(|snapshot| NodeParticipation {
                key: snapshot.key,
                participates: true,
            })
            .collect::<Vec<_>>();
        let mut renderer = DirectOccurrenceRenderer::new(6);
        renderer.synchronize(
            snapshots,
            None,
            &participation,
            HashMap::new(),
            vec![inner, outer, body],
            HashMap::from([(outer, body), (inner, outer_child)]),
            HashMap::new(),
        )?;
        let (geometries, _) = renderer.layout_roots(
            body,
            crate::geometry::Size::new(20, 4),
            DirectHistoryAnchor::FollowEnd,
            &HashMap::new(),
            &HashMap::new(),
        )?;
        let outer_position = geometries
            .iter()
            .position(|geometry| geometry.key == outer)
            .expect("outer portal geometry");
        let inner_position = geometries
            .iter()
            .position(|geometry| geometry.key == inner)
            .expect("inner portal geometry");
        assert!(outer_position < inner_position);
        Ok(())
    }
}
