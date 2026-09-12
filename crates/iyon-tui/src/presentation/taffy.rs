//! Occurrence-owned Taffy layout adapter.
//!
//! This is derived renderer state, not a second UI document. The adapter
//! accepts immutable occurrence snapshots, owns the high-level [`TaffyTree`],
//! and returns checked terminal geometry.  It deliberately does not expose a
//! Taffy style or accept callbacks that can mutate application state.

use std::collections::{HashMap, HashSet};

use taffy::prelude::{
    AlignContent, AlignItems, AlignSelf, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, LengthPercentage, LengthPercentageAuto, Line, Position,
    Size, Style, TaffyAuto, TaffyMaxContent, TaffyMinContent, TrackSizingFunction,
};
use taffy::style::Direction;
use taffy::style_helpers::{FromFr, FromLength, FromPercent};

use crate::occurrence::{
    Alignment, AlignmentAxis, AlignmentMode, DimensionInsets, DimensionValue, DirectionMode,
    DisplayMode, FlexDirectionMode, FlexWrapMode, GridAutoFlowMode, GridLineValue,
    GridPlacementValue, HostKind, LayerValue, LayoutMode, NodeKey, OccurrenceSnapshot,
    PositionMode, PropertyId, PropertyValue, TrackListValue, TrackMaxBound, TrackMinBound,
    TrackValue,
};

/// A terminal geometry rectangle after one accumulated-edge quantization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComputedGeometry {
    pub(crate) key: NodeKey,
    pub(crate) rect: PhysicalRect,
    pub(crate) logical: LogicalRect,
    pub(crate) logical_content_width: f32,
    pub(crate) logical_content_height: f32,
    pub(crate) logical_content_x: f32,
    pub(crate) logical_content_y: f32,
    /// Renderer hiding does not remove ownership or layout-node identity.
    pub(crate) renderer_hidden: bool,
    /// `display:none` is a semantic layout value, distinct from renderer
    /// hiding, and still retains the occurrence in derived adapter state.
    pub(crate) display_none: bool,
}

/// A checked logical geometry rectangle before physical quantization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogicalRect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaffyAdapterError {
    InvalidTaffyTree,
    InvalidInput(&'static str, NodeKey),
    NonFiniteGeometry,
    PhysicalOverflow,
    MissingNode(NodeKey),
    MissingSnapshot(NodeKey),
    DuplicateSnapshot(NodeKey),
}

/// The finite available-space constraint sent to an intrinsic leaf
/// measurement. Known dimensions are carried separately in [`MeasureRequest`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AvailableConstraint {
    Definite(f32),
    MinContent,
    MaxContent,
}

/// Participation is supplied by the owning renderer for controls whose
/// retained occurrence remains mounted while its current frame demand is
/// inactive. It is deliberately not an application callback or a content
/// registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NodeParticipation {
    pub(crate) key: NodeKey,
    pub(crate) participates: bool,
}

/// Narrow measurement request. The callback must read immutable captured
/// content context; it cannot select a connector, advance smoothing, or
/// mutate a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MeasureRequest {
    pub(crate) known_width: Option<f32>,
    pub(crate) known_height: Option<f32>,
    pub(crate) available_width: AvailableConstraint,
    pub(crate) available_height: AvailableConstraint,
    /// The integer wrapping width selected for terminal content, when a
    /// logical width is available. Zero is intentionally distinct from one.
    pub(crate) wrap_width: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MeasuredSize {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LeafContext {
    key: NodeKey,
}

#[derive(Clone, Debug)]
struct LayoutEntry {
    node: taffy::NodeId,
    renderer_hidden: bool,
    display_none: bool,
    participates: bool,
    child_list_revision: u64,
}

struct PreparedNode<'a> {
    key: NodeKey,
    snapshot: &'a OccurrenceSnapshot,
    style: PreparedStyle,
    renderer_hidden: bool,
    display_none: bool,
    participates: bool,
}

enum PreparedStyle {
    New(Style),
    Update(Style),
    Unchanged,
}

struct PreparedParent {
    key: NodeKey,
    children: Vec<NodeKey>,
    structure_revision: u64,
}

struct PreparedSync<'a> {
    nodes: Vec<PreparedNode<'a>>,
    parents: Vec<PreparedParent>,
    retirement_order: Vec<NodeKey>,
}

/// Renderer-driver-owned derived layout state.
#[derive(Debug)]
pub(crate) struct TaffyLayoutAdapter {
    tree: taffy::TaffyTree<LeafContext>,
    entries: HashMap<NodeKey, LayoutEntry>,
}

impl Default for TaffyLayoutAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TaffyLayoutAdapter {
    pub(crate) fn new() -> Self {
        let mut tree = taffy::TaffyTree::with_capacity(32);
        tree.disable_rounding();
        Self {
            tree,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn contains(&self, key: NodeKey) -> bool {
        self.entries.contains_key(&key)
    }

    /// Synchronize explicit changed-style nodes and explicit canonical parent
    /// child lists. The caller must supply every changed parent snapshot; this
    /// boundary never searches child lists to rediscover parentage.
    pub(crate) fn synchronize(
        &mut self,
        snapshots: &[OccurrenceSnapshot],
        changed_styles: &[NodeKey],
        changed_parents: &[NodeKey],
        participation: &[NodeParticipation],
        retired: &[NodeKey],
    ) -> Result<(), TaffyAdapterError> {
        let plan = self.prepare_sync(
            snapshots,
            changed_styles,
            changed_parents,
            participation,
            retired,
        )?;
        self.install_new_nodes(&plan)?;
        self.install_topology(&plan)?;
        self.install_styles_and_metadata(&plan)?;
        self.retire_nodes(&plan.retirement_order)
    }

    /// Apply a sparse occurrence frontier after initial synchronization. The
    /// supplied snapshots contain only changed leaves/parents and their
    /// affected participation facts; unchanged entries remain owned by this
    /// derived adapter and are not cloned into a second snapshot universe.
    pub(crate) fn synchronize_sparse(
        &mut self,
        snapshots: &[OccurrenceSnapshot],
        changed_styles: &[NodeKey],
        changed_parents: &[NodeKey],
        participation: &[NodeParticipation],
        retired: &[NodeKey],
    ) -> Result<(), TaffyAdapterError> {
        let plan = self.prepare_sync(
            snapshots,
            changed_styles,
            changed_parents,
            participation,
            retired,
        )?;
        self.install_new_nodes(&plan)?;
        self.install_topology(&plan)?;
        self.install_styles_and_metadata(&plan)?;
        self.retire_nodes(&plan.retirement_order)
    }

    fn entry(&self, key: NodeKey) -> Result<&LayoutEntry, TaffyAdapterError> {
        self.entries
            .get(&key)
            .ok_or(TaffyAdapterError::MissingNode(key))
    }

    /// Validate the complete sparse frontier and derive the operations that
    /// will be installed. This is the only phase that interprets occurrence
    /// snapshots; the following phases only apply this plan to Taffy.
    fn prepare_sync<'a>(
        &self,
        snapshots: &'a [OccurrenceSnapshot],
        changed_styles: &[NodeKey],
        changed_parents: &[NodeKey],
        participation: &[NodeParticipation],
        retired: &[NodeKey],
    ) -> Result<PreparedSync<'a>, TaffyAdapterError> {
        let mut snapshot_map = HashMap::with_capacity(snapshots.len());
        for snapshot in snapshots {
            if snapshot_map.insert(snapshot.key, snapshot).is_some() {
                return Err(TaffyAdapterError::DuplicateSnapshot(snapshot.key));
            }
            validate_legacy_alignment(snapshot)?;
        }
        let style_set: HashSet<NodeKey> = changed_styles.iter().copied().collect();
        let parent_set: HashSet<NodeKey> = changed_parents.iter().copied().collect();
        let retired_set: HashSet<NodeKey> = retired.iter().copied().collect();
        let mut participation_map = HashMap::with_capacity(participation.len());
        for item in participation {
            if participation_map
                .insert(item.key, item.participates)
                .is_some()
            {
                return Err(TaffyAdapterError::InvalidInput(
                    "duplicate participation entry",
                    item.key,
                ));
            }
        }

        // Changed-style and participation-only keys must carry a current
        // snapshot. This is the sparse adapter contract, not document
        // validation: the accepted occurrence summary has already validated
        // topology and retirement membership upstream.
        for key in style_set.iter().chain(participation_map.keys()) {
            if !snapshot_map.contains_key(key) {
                return Err(TaffyAdapterError::MissingSnapshot(*key));
            }
        }
        for key in &parent_set {
            if !snapshot_map.contains_key(key) {
                return Err(TaffyAdapterError::MissingSnapshot(*key));
            }
        }
        for key in &retired_set {
            if !self.entries.contains_key(key) {
                return Err(TaffyAdapterError::MissingNode(*key));
            }
        }
        let nodes = self.prepare_nodes(snapshots, &style_set, &participation_map);
        let parents =
            self.prepare_parents(changed_parents, &snapshot_map, &parent_set, &retired_set)?;
        let retirement_order = self.retirement_postorder(retired, &retired_set)?;
        Ok(PreparedSync {
            nodes,
            parents,
            retirement_order,
        })
    }

    fn prepare_nodes<'a>(
        &self,
        snapshots: &'a [OccurrenceSnapshot],
        style_set: &HashSet<NodeKey>,
        participation_map: &HashMap<NodeKey, bool>,
    ) -> Vec<PreparedNode<'a>> {
        snapshots
            .iter()
            .map(|snapshot| {
                let entry = self.entries.get(&snapshot.key);
                let participates = participation_map
                    .get(&snapshot.key)
                    .copied()
                    .or_else(|| entry.map(|entry| entry.participates))
                    .unwrap_or(true);
                let display_none = matches!(display_for(snapshot), DisplayMode::None);
                let is_new = entry.is_none();
                let needs_style = is_new
                    || style_set.contains(&snapshot.key)
                    || participation_map.contains_key(&snapshot.key)
                    || entry.is_none_or(|entry| {
                        entry.renderer_hidden != snapshot.hidden
                            || entry.display_none != display_none
                    });
                PreparedNode {
                    key: snapshot.key,
                    snapshot,
                    style: if is_new {
                        PreparedStyle::New(style_for(snapshot, participates))
                    } else if needs_style {
                        PreparedStyle::Update(style_for(snapshot, participates))
                    } else {
                        PreparedStyle::Unchanged
                    },
                    renderer_hidden: snapshot.hidden,
                    display_none,
                    participates,
                }
            })
            .collect()
    }

    fn prepare_parents(
        &self,
        changed_parents: &[NodeKey],
        snapshots: &HashMap<NodeKey, &OccurrenceSnapshot>,
        parent_set: &HashSet<NodeKey>,
        retired_set: &HashSet<NodeKey>,
    ) -> Result<Vec<PreparedParent>, TaffyAdapterError> {
        let mut parents = Vec::with_capacity(changed_parents.len());
        for parent in changed_parents {
            let snapshot = snapshots
                .get(parent)
                .expect("changed parent snapshot was validated");
            if self
                .entries
                .get(parent)
                .is_some_and(|entry| entry.child_list_revision == snapshot.structure_revision)
            {
                continue;
            }
            for child in &snapshot.children {
                if !self.entries.contains_key(child) && !snapshots.contains_key(child) {
                    return Err(TaffyAdapterError::MissingNode(*child));
                }
                if let Some(old_parent) = self.current_parent_if_live(*child)?
                    && old_parent != *parent
                    && !parent_set.contains(&old_parent)
                    && !retired_set.contains(&old_parent)
                {
                    return Err(TaffyAdapterError::MissingSnapshot(old_parent));
                }
            }
            parents.push(PreparedParent {
                key: *parent,
                children: snapshot.children.clone(),
                structure_revision: snapshot.structure_revision,
            });
        }
        Ok(parents)
    }

    fn current_parent_if_live(&self, key: NodeKey) -> Result<Option<NodeKey>, TaffyAdapterError> {
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        let node = entry.node;
        self.tree
            .parent(node)
            .map(|parent| {
                self.tree
                    .get_node_context(parent)
                    .map(|context| context.key)
                    .ok_or(TaffyAdapterError::InvalidTaffyTree)
            })
            .transpose()
    }

    fn install_new_nodes(&mut self, plan: &PreparedSync<'_>) -> Result<(), TaffyAdapterError> {
        for prepared in &plan.nodes {
            let PreparedStyle::New(style) = &prepared.style else {
                continue;
            };
            let taffy_node = self
                .tree
                .new_leaf_with_context(style.clone(), LeafContext { key: prepared.key })
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
            self.entries.insert(
                prepared.key,
                LayoutEntry {
                    node: taffy_node,
                    renderer_hidden: prepared.renderer_hidden,
                    display_none: prepared.display_none,
                    participates: prepared.participates,
                    child_list_revision: prepared.snapshot.structure_revision,
                },
            );
        }
        Ok(())
    }

    fn install_topology(&mut self, plan: &PreparedSync<'_>) -> Result<(), TaffyAdapterError> {
        // Sever every affected old edge first. This is what makes a sparse
        // cross-parent move independent of changed-parent iteration order.
        // Retiring subtrees are included so removing each retired child does
        // not repeatedly scan its old parent's shrinking child list. Reverse
        // postorder detaches ancestors before descendants are marked dirty.
        for key in plan
            .parents
            .iter()
            .map(|parent| parent.key)
            .chain(plan.retirement_order.iter().rev().copied())
        {
            let node = self.entry(key)?.node;
            self.tree
                .set_children(node, &[])
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        }
        for parent in &plan.parents {
            let parent_node = self.entry(parent.key)?.node;
            let child_nodes = parent
                .children
                .iter()
                .map(|key| self.entry(*key).map(|entry| entry.node))
                .collect::<Result<Vec<_>, _>>()?;
            self.tree
                .set_children(parent_node, &child_nodes)
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
            self.entries
                .get_mut(&parent.key)
                .ok_or(TaffyAdapterError::MissingNode(parent.key))?
                .child_list_revision = parent.structure_revision;
        }
        Ok(())
    }

    fn install_styles_and_metadata(
        &mut self,
        plan: &PreparedSync<'_>,
    ) -> Result<(), TaffyAdapterError> {
        for prepared in &plan.nodes {
            let PreparedStyle::Update(style) = &prepared.style else {
                continue;
            };
            let node = self.entry(prepared.key)?.node;
            if self
                .tree
                .style(node)
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?
                != style
            {
                self.tree
                    .set_style(node, style.clone())
                    .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
            }
        }
        for prepared in &plan.nodes {
            let entry = self
                .entries
                .get_mut(&prepared.key)
                .ok_or(TaffyAdapterError::MissingNode(prepared.key))?;
            entry.renderer_hidden = prepared.renderer_hidden;
            entry.display_none = prepared.display_none;
            entry.participates = prepared.participates;
        }
        Ok(())
    }

    fn retire_nodes(&mut self, retirement_order: &[NodeKey]) -> Result<(), TaffyAdapterError> {
        for key in retirement_order {
            let entry = self
                .entries
                .remove(key)
                .ok_or(TaffyAdapterError::MissingNode(*key))?;
            self.tree
                .remove(entry.node)
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        }
        Ok(())
    }

    fn retirement_postorder(
        &self,
        retired: &[NodeKey],
        retired_set: &HashSet<NodeKey>,
    ) -> Result<Vec<NodeKey>, TaffyAdapterError> {
        let mut visited = HashSet::new();
        let mut output = Vec::with_capacity(retired.len());
        for root in retired {
            if visited.contains(root) {
                continue;
            }
            let mut stack = vec![(*root, false)];
            while let Some((current, expanded)) = stack.pop() {
                if visited.contains(&current) {
                    continue;
                }
                if expanded {
                    visited.insert(current);
                    output.push(current);
                    continue;
                }
                let node = self.entry(current)?.node;
                stack.push((current, true));
                let children = self
                    .tree
                    .children(node)
                    .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
                for child in children.into_iter().rev() {
                    let child_key = self
                        .tree
                        .get_node_context(child)
                        .ok_or(TaffyAdapterError::InvalidTaffyTree)?
                        .key;
                    if retired_set.contains(&child_key) && !visited.contains(&child_key) {
                        stack.push((child_key, false));
                    }
                }
            }
        }
        Ok(output)
    }

    /// Invalidate intrinsic measurement for a changed Source/control product
    /// without changing semantic style or topology. The product revision is
    /// owned by the captured leaf context upstream; the adapter only marks
    /// the affected leaf (and Taffy's dependency ancestors) dirty.
    pub(crate) fn invalidate_measurement(
        &mut self,
        keys: &[NodeKey],
    ) -> Result<(), TaffyAdapterError> {
        for key in keys {
            let node = self.entry(*key)?.node;
            self.tree
                .mark_dirty(node)
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        }
        Ok(())
    }

    /// Layout one synchronized root under finite terminal constraints.
    pub(crate) fn layout(
        &mut self,
        root: NodeKey,
        width: AvailableConstraint,
        height: AvailableConstraint,
        measure: &mut impl FnMut(NodeKey, MeasureRequest) -> MeasuredSize,
    ) -> Result<Vec<ComputedGeometry>, TaffyAdapterError> {
        let root_node = self.entry(root)?.node;
        let available = Size {
            width: available_space(width)?,
            height: available_space(height)?,
        };
        let mut measure_error = None;
        let mut failed_nodes = Vec::new();
        self.tree
            .compute_layout_with_measure(
                root_node,
                available,
                |known, available, node, context, _| {
                    let context = context.expect("every adapter node has a LeafContext");
                    let key = context.key;
                    let width_available = match available_constraint(available.width) {
                        Ok(value) => value,
                        Err(error) => {
                            measure_error = Some(error);
                            AvailableConstraint::MinContent
                        }
                    };
                    let height_available = match available_constraint(available.height) {
                        Ok(value) => value,
                        Err(error) => {
                            measure_error = Some(error);
                            AvailableConstraint::MinContent
                        }
                    };
                    if known
                        .width
                        .is_some_and(|value| !value.is_finite() || value < 0.0)
                        || known
                            .height
                            .is_some_and(|value| !value.is_finite() || value < 0.0)
                    {
                        measure_error = Some(TaffyAdapterError::NonFiniteGeometry);
                        failed_nodes.push(node);
                        return Size::ZERO;
                    }
                    let request = MeasureRequest {
                        known_width: known.width,
                        known_height: known.height,
                        available_width: width_available,
                        available_height: height_available,
                        wrap_width: known
                            .width
                            .or_else(|| available.width.into_option())
                            .and_then(|value| match wrap_width(value) {
                                Ok(value) => Some(value),
                                Err(error) => {
                                    measure_error = Some(error);
                                    None
                                }
                            }),
                    };
                    if measure_error.is_some() {
                        failed_nodes.push(node);
                        return Size::ZERO;
                    }
                    let measured = measure(key, request);
                    match checked_measure(measured) {
                        Ok(measured) => Size {
                            width: known.width.unwrap_or(measured.width),
                            height: known.height.unwrap_or(measured.height),
                        },
                        Err(error) => {
                            measure_error = Some(error);
                            failed_nodes.push(node);
                            Size::ZERO
                        }
                    }
                },
            )
            .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        if let Some(error) = measure_error {
            for node in failed_nodes {
                self.tree
                    .mark_dirty(node)
                    .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
            }
            return Err(error);
        }

        let mut output = Vec::new();
        self.collect_geometry(root, 0.0, 0.0, &mut output)?;
        Ok(output)
    }

    /// Layout a direct root against a definite viewport height while keeping
    /// the root's ordinary intrinsic style for later max-content probes. The
    /// occurrence root is the boundary that establishes percentage heights;
    /// its temporary viewport size is restored before returning.
    pub(crate) fn layout_with_root_height(
        &mut self,
        root: NodeKey,
        width: AvailableConstraint,
        height: f32,
        measure: &mut impl FnMut(NodeKey, MeasureRequest) -> MeasuredSize,
    ) -> Result<Vec<ComputedGeometry>, TaffyAdapterError> {
        if !height.is_finite() || height < 0.0 {
            return Err(TaffyAdapterError::NonFiniteGeometry);
        }
        let node = self.entry(root)?.node;
        let mut style = self
            .tree
            .style(node)
            .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?
            .clone();
        let original = style.clone();
        style.size.height = Dimension::length(height);
        self.tree
            .set_style(node, style)
            .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        let result = self.layout(
            root,
            width,
            AvailableConstraint::Definite(height),
            measure,
        );
        self.tree
            .set_style(node, original)
            .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
        result
    }

    fn collect_geometry(
        &self,
        key: NodeKey,
        parent_x: f32,
        parent_y: f32,
        output: &mut Vec<ComputedGeometry>,
    ) -> Result<(), TaffyAdapterError> {
        let mut stack = vec![(key, parent_x, parent_y)];
        while let Some((key, parent_x, parent_y)) = stack.pop() {
            let entry = self.entry(key)?;
            let node = entry.node;
            let layout = self.tree.unrounded_layout(node);
            let x = parent_x + layout.location.x;
            let y = parent_y + layout.location.y;
            let right = x + layout.size.width;
            let bottom = y + layout.size.height;
            let logical = LogicalRect {
                x,
                y,
                width: layout.size.width,
                height: layout.size.height,
            };
            output.push(ComputedGeometry {
                key,
                rect: round_rect(logical, right, bottom)?,
                logical,
                logical_content_width: layout.content_box_width(),
                logical_content_height: layout.content_box_height(),
                logical_content_x: x + layout.border.left + layout.padding.left,
                logical_content_y: y + layout.border.top + layout.padding.top,
                renderer_hidden: entry.renderer_hidden,
                display_none: entry.display_none,
            });
            let children = self
                .tree
                .children(node)
                .map_err(|_| TaffyAdapterError::InvalidTaffyTree)?;
            for child in children.into_iter().rev() {
                let context = self
                    .tree
                    .get_node_context(child)
                    .ok_or(TaffyAdapterError::InvalidTaffyTree)?;
                stack.push((context.key, x, y));
            }
        }
        Ok(())
    }
}

fn available_space(constraint: AvailableConstraint) -> Result<AvailableSpace, TaffyAdapterError> {
    match constraint {
        AvailableConstraint::Definite(value) if value.is_finite() && value >= 0.0 => {
            Ok(AvailableSpace::Definite(value))
        }
        AvailableConstraint::Definite(_) => Err(TaffyAdapterError::NonFiniteGeometry),
        AvailableConstraint::MinContent => Ok(AvailableSpace::MinContent),
        AvailableConstraint::MaxContent => Ok(AvailableSpace::MaxContent),
    }
}

fn available_constraint(
    available: AvailableSpace,
) -> Result<AvailableConstraint, TaffyAdapterError> {
    match available {
        AvailableSpace::Definite(value) if value.is_finite() && value >= 0.0 => {
            Ok(AvailableConstraint::Definite(value))
        }
        AvailableSpace::Definite(_) => Err(TaffyAdapterError::NonFiniteGeometry),
        AvailableSpace::MinContent => Ok(AvailableConstraint::MinContent),
        AvailableSpace::MaxContent => Ok(AvailableConstraint::MaxContent),
    }
}

fn checked_measure(value: MeasuredSize) -> Result<MeasuredSize, TaffyAdapterError> {
    if !value.width.is_finite()
        || !value.height.is_finite()
        || value.width < 0.0
        || value.height < 0.0
    {
        return Err(TaffyAdapterError::NonFiniteGeometry);
    }
    Ok(value)
}

fn wrap_width(value: f32) -> Result<u32, TaffyAdapterError> {
    if !value.is_finite() || value < 0.0 {
        return Err(TaffyAdapterError::NonFiniteGeometry);
    }
    let value = value.floor();
    // `u32::MAX as f32` rounds to 2^32, so equality is already outside the
    // representable integer range.
    if value >= u32::MAX as f32 {
        return Err(TaffyAdapterError::PhysicalOverflow);
    }
    Ok(value as u32)
}

fn round_rect(
    rect: LogicalRect,
    right: f32,
    bottom: f32,
) -> Result<PhysicalRect, TaffyAdapterError> {
    let edges = [rect.x, rect.y, right, bottom];
    if edges.iter().any(|value| !value.is_finite()) {
        return Err(TaffyAdapterError::NonFiniteGeometry);
    }
    let [x, y, right, bottom] = edges.map(|value| f64::from(value).round());
    if [x, y, right, bottom]
        .iter()
        .any(|value| *value < f64::from(i32::MIN) || *value > f64::from(i32::MAX))
    {
        return Err(TaffyAdapterError::PhysicalOverflow);
    }
    let x = x as i32;
    let y = y as i32;
    let right = right as i32;
    let bottom = bottom as i32;
    Ok(PhysicalRect {
        x,
        y,
        width: right
            .checked_sub(x)
            .ok_or(TaffyAdapterError::PhysicalOverflow)?,
        height: bottom
            .checked_sub(y)
            .ok_or(TaffyAdapterError::PhysicalOverflow)?,
    })
}

pub(crate) fn checked_round_edge(value: f32) -> Result<i32, TaffyAdapterError> {
    if !value.is_finite() {
        return Err(TaffyAdapterError::NonFiniteGeometry);
    }
    let value = f64::from(value).round();
    if value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(TaffyAdapterError::PhysicalOverflow);
    }
    Ok(value as i32)
}

fn display_for(snapshot: &OccurrenceSnapshot) -> DisplayMode {
    match property(snapshot, PropertyId::Display) {
        Some(LayerValue::Value(PropertyValue::Display(mode))) => *mode,
        _ => match property(snapshot, PropertyId::Layout) {
            Some(LayerValue::Value(PropertyValue::LayoutMode(LayoutMode::Grid))) => {
                DisplayMode::Grid
            }
            _ => DisplayMode::Flex,
        },
    }
}

fn validate_legacy_alignment(snapshot: &OccurrenceSnapshot) -> Result<(), TaffyAdapterError> {
    let Some(LayerValue::Value(PropertyValue::Alignment(alignment))) =
        property(snapshot, PropertyId::Alignment)
    else {
        return Ok(());
    };
    if alignment
        .horizontal
        .is_some_and(|axis| axis != crate::occurrence::AlignmentAxis::Start)
    {
        return Err(TaffyAdapterError::InvalidInput(
            "alignment.horizontal is unsupported in the M1 terminal adapter; text alignment remains content-owned",
            snapshot.key,
        ));
    }
    let Some(vertical) = alignment.vertical else {
        return Ok(());
    };
    if matches!(
        vertical,
        crate::occurrence::AlignmentAxis::Start | crate::occurrence::AlignmentAxis::Top
    ) {
        return Ok(());
    }
    let layout_mode = match property(snapshot, PropertyId::Layout) {
        Some(LayerValue::Value(PropertyValue::LayoutMode(mode))) => *mode,
        _ => LayoutMode::Box,
    };
    if snapshot.kind != HostKind::Box || layout_mode != LayoutMode::Row {
        return Err(TaffyAdapterError::InvalidInput(
            "alignment.vertical is unsupported in the M1 terminal adapter except for Box rows",
            snapshot.key,
        ));
    }
    Ok(())
}

fn style_for(snapshot: &OccurrenceSnapshot, participates: bool) -> Style {
    let mut style = Style::default();
    let layout_mode = match property(snapshot, PropertyId::Layout) {
        Some(LayerValue::Value(PropertyValue::LayoutMode(mode))) => *mode,
        _ => LayoutMode::Box,
    };
    // Renderer hiding is a current-frame fact, not control participation.
    // Both independently suppress layout while their occurrence identity and
    // any explicit semantic display mode remain retained.
    style.display = if !participates || snapshot.hidden {
        Display::None
    } else {
        match display_for(snapshot) {
            DisplayMode::Flex => Display::Flex,
            DisplayMode::Grid => Display::Grid,
            DisplayMode::None => Display::None,
        }
    };
    style.direction = direction_for(snapshot);
    style.flex_direction = flex_direction_for(snapshot, layout_mode);
    style.flex_wrap = flex_wrap_for(snapshot);
    apply_dimensions(&mut style, snapshot);
    if snapshot.root_role.is_some() {
        style.size.width = Dimension::percent(1.0);
    }
    if matches!(
        property(snapshot, PropertyId::Width),
        None | Some(LayerValue::Unset | LayerValue::Null)
            | Some(LayerValue::Value(PropertyValue::SizeMode(
                crate::occurrence::SizeMode::Fit,
            )))
            | Some(LayerValue::Value(PropertyValue::Dimension(DimensionValue::Auto)))
    ) {
        style.max_size.width = Dimension::percent(1.0);
        style.min_size.width = Dimension::length(0.0);
    }
    // Concrete controls are occurrence leaves. Their native component view is
    // painted inside this allocation, so a control without explicit geometry
    // must participate in its parent's width and retain one terminal row
    // instead of being treated as a zero-sized Taffy leaf.
    if snapshot.control.is_some() {
        if matches!(
            property(snapshot, PropertyId::Width),
            None | Some(LayerValue::Unset | LayerValue::Null)
        ) {
            style.size.width = Dimension::percent(1.0);
        }
        if matches!(
            property(snapshot, PropertyId::Height),
            None | Some(LayerValue::Unset | LayerValue::Null)
        ) {
            style.min_size.height = Dimension::length(1.0);
        }
    }
    apply_flex_values(&mut style, snapshot);
    apply_alignment(&mut style, snapshot);
    apply_grid(&mut style, snapshot);
    apply_border(&mut style, snapshot);
    style
}

fn direction_for(snapshot: &OccurrenceSnapshot) -> Direction {
    match property(snapshot, PropertyId::Direction) {
        Some(LayerValue::Value(PropertyValue::Direction(DirectionMode::Rtl))) => Direction::Rtl,
        _ => Direction::Ltr,
    }
}

fn flex_direction_for(snapshot: &OccurrenceSnapshot, layout_mode: LayoutMode) -> FlexDirection {
    match property(snapshot, PropertyId::FlexDirection) {
        Some(LayerValue::Value(PropertyValue::FlexDirection(direction))) => match direction {
            FlexDirectionMode::Row => FlexDirection::Row,
            FlexDirectionMode::Column => FlexDirection::Column,
            FlexDirectionMode::RowReverse => FlexDirection::RowReverse,
            FlexDirectionMode::ColumnReverse => FlexDirection::ColumnReverse,
        },
        _ => match layout_mode {
            LayoutMode::Column | LayoutMode::Box => FlexDirection::Column,
            LayoutMode::Row | LayoutMode::Grid => FlexDirection::Row,
        },
    }
}

fn flex_wrap_for(snapshot: &OccurrenceSnapshot) -> FlexWrap {
    match property(snapshot, PropertyId::FlexWrap) {
        Some(LayerValue::Value(PropertyValue::FlexWrap(wrap))) => match wrap {
            FlexWrapMode::NoWrap => FlexWrap::NoWrap,
            FlexWrapMode::Wrap => FlexWrap::Wrap,
            FlexWrapMode::WrapReverse => FlexWrap::WrapReverse,
        },
        _ => FlexWrap::NoWrap,
    }
}

fn apply_dimensions(style: &mut Style, snapshot: &OccurrenceSnapshot) {
    style.size = Size {
        width: size_dimension(property(snapshot, PropertyId::Width)),
        height: size_dimension(property(snapshot, PropertyId::Height)),
    };
    style.min_size = Size {
        width: u16_dimension(property(snapshot, PropertyId::MinWidth)),
        height: u16_dimension(property(snapshot, PropertyId::MinHeight)),
    };
    style.max_size = Size {
        width: u16_dimension(property(snapshot, PropertyId::MaxWidth)),
        height: u16_dimension(property(snapshot, PropertyId::MaxHeight)),
    };
    style.padding = old_padding(property(snapshot, PropertyId::Padding));
    style.margin = margin_insets(property(snapshot, PropertyId::Margin));
    style.inset = dimension_insets(property(snapshot, PropertyId::Inset));
    style.position = matches!(
        property(snapshot, PropertyId::Position),
        Some(LayerValue::Value(PropertyValue::Position(
            PositionMode::Absolute
        )))
    )
    .then_some(Position::Absolute)
    .unwrap_or(Position::Relative);
}

fn apply_flex_values(style: &mut Style, snapshot: &OccurrenceSnapshot) {
    let old_gap = match property(snapshot, PropertyId::Gap) {
        Some(LayerValue::Value(PropertyValue::U16(value))) => f32::from(*value),
        _ => 0.0,
    };
    style.gap = Size {
        width: length_percentage(property(snapshot, PropertyId::ColumnGap))
            .unwrap_or(LengthPercentage::length(old_gap)),
        height: length_percentage(property(snapshot, PropertyId::RowGap))
            .unwrap_or(LengthPercentage::length(old_gap)),
    };
    style.flex_grow = scalar(property(snapshot, PropertyId::FlexGrow)).unwrap_or(0.0);
    style.flex_shrink = scalar(property(snapshot, PropertyId::FlexShrink)).unwrap_or(1.0);
    style.flex_basis = dimension(property(snapshot, PropertyId::FlexBasis));
}

fn apply_alignment(style: &mut Style, snapshot: &OccurrenceSnapshot) {
    let (old_align_items, old_justify_content) = old_alignment(snapshot, style.flex_direction);
    style.align_items = alignment_items(property(snapshot, PropertyId::AlignItems))
        .or(old_align_items)
        .or(Some(AlignItems::START));
    style.align_self = alignment_self(property(snapshot, PropertyId::AlignSelf));
    style.align_content = alignment_content(property(snapshot, PropertyId::AlignContent))
        .or(Some(AlignContent::START));
    style.justify_content = alignment_content(property(snapshot, PropertyId::JustifyContent))
        .or(old_justify_content)
        .or(Some(taffy::prelude::JustifyContent::START));
    style.justify_items = alignment_items(property(snapshot, PropertyId::JustifyItems))
        .or(Some(taffy::prelude::JustifyItems::START));
    style.justify_self = alignment_self(property(snapshot, PropertyId::JustifySelf));
}

fn apply_grid(style: &mut Style, snapshot: &OccurrenceSnapshot) {
    style.grid_template_columns =
        template_tracks(property(snapshot, PropertyId::GridTemplateColumns));
    style.grid_template_rows = template_tracks(property(snapshot, PropertyId::GridTemplateRows));
    style.grid_auto_columns = track_values(property(snapshot, PropertyId::GridAutoColumns));
    style.grid_auto_rows = track_values(property(snapshot, PropertyId::GridAutoRows));
    style.grid_auto_flow = match property(snapshot, PropertyId::GridAutoFlow) {
        Some(LayerValue::Value(PropertyValue::GridAutoFlow(flow))) => match flow {
            GridAutoFlowMode::Row => GridAutoFlow::Row,
            GridAutoFlowMode::Column => GridAutoFlow::Column,
            GridAutoFlowMode::RowDense => GridAutoFlow::RowDense,
            GridAutoFlowMode::ColumnDense => GridAutoFlow::ColumnDense,
        },
        _ => GridAutoFlow::Row,
    };
    style.grid_column = placement(property(snapshot, PropertyId::GridColumn));
    style.grid_row = placement(property(snapshot, PropertyId::GridRow));
}

fn apply_border(style: &mut Style, snapshot: &OccurrenceSnapshot) {
    let Some(LayerValue::Value(PropertyValue::Edges(edges))) =
        property(snapshot, PropertyId::BorderEdges)
    else {
        return;
    };
    style.border = taffy::geometry::Rect {
        top: LengthPercentage::length(if edges.top { 1.0 } else { 0.0 }),
        right: LengthPercentage::length(if edges.right { 1.0 } else { 0.0 }),
        bottom: LengthPercentage::length(if edges.bottom { 1.0 } else { 0.0 }),
        left: LengthPercentage::length(if edges.left { 1.0 } else { 0.0 }),
    };
}

fn property<'a>(snapshot: &'a OccurrenceSnapshot, id: PropertyId) -> Option<&'a LayerValue> {
    snapshot
        .properties
        .iter()
        .find_map(|(property, value)| (*property == id).then_some(value))
}

fn dimension(value: Option<&LayerValue>) -> Dimension {
    match value {
        Some(LayerValue::Value(PropertyValue::Dimension(value))) => match value {
            DimensionValue::Length(value) => Dimension::length(value.get()),
            DimensionValue::Percent(value) => Dimension::percent(value.get()),
            DimensionValue::Auto => Dimension::auto(),
        },
        _ => Dimension::auto(),
    }
}

fn size_dimension(value: Option<&LayerValue>) -> Dimension {
    match value {
        Some(LayerValue::Value(PropertyValue::SizeMode(mode))) => match mode {
            crate::occurrence::SizeMode::Fit => Dimension::auto(),
            crate::occurrence::SizeMode::Fill => Dimension::percent(1.0),
        },
        _ => dimension(value),
    }
}

fn u16_dimension(value: Option<&LayerValue>) -> Dimension {
    match value {
        Some(LayerValue::Value(PropertyValue::U16(value))) => Dimension::length(f32::from(*value)),
        Some(LayerValue::Null | LayerValue::Unset) | None => Dimension::auto(),
        _ => Dimension::auto(),
    }
}

fn length_percentage(value: Option<&LayerValue>) -> Option<LengthPercentage> {
    match value {
        Some(LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(value)))) => {
            Some(LengthPercentage::length(value.get()))
        }
        Some(LayerValue::Value(PropertyValue::Dimension(DimensionValue::Percent(value)))) => {
            Some(LengthPercentage::percent(value.get()))
        }
        _ => None,
    }
}

fn dimension_auto(value: DimensionValue) -> LengthPercentageAuto {
    match value {
        DimensionValue::Length(value) => LengthPercentageAuto::length(value.get()),
        DimensionValue::Percent(value) => LengthPercentageAuto::percent(value.get()),
        DimensionValue::Auto => LengthPercentageAuto::auto(),
    }
}

fn dimension_insets(value: Option<&LayerValue>) -> taffy::geometry::Rect<LengthPercentageAuto> {
    let value = match value {
        Some(LayerValue::Value(PropertyValue::Dimensions(value))) => *value,
        _ => DimensionInsets {
            top: DimensionValue::Auto,
            right: DimensionValue::Auto,
            bottom: DimensionValue::Auto,
            left: DimensionValue::Auto,
        },
    };
    taffy::geometry::Rect {
        top: dimension_auto(value.top),
        right: dimension_auto(value.right),
        bottom: dimension_auto(value.bottom),
        left: dimension_auto(value.left),
    }
}

fn margin_insets(value: Option<&LayerValue>) -> taffy::geometry::Rect<LengthPercentageAuto> {
    let value = match value {
        Some(LayerValue::Value(PropertyValue::Dimensions(value))) => *value,
        _ => DimensionInsets {
            top: DimensionValue::Length(crate::occurrence::FiniteScalar::new(0.0).unwrap()),
            right: DimensionValue::Length(crate::occurrence::FiniteScalar::new(0.0).unwrap()),
            bottom: DimensionValue::Length(crate::occurrence::FiniteScalar::new(0.0).unwrap()),
            left: DimensionValue::Length(crate::occurrence::FiniteScalar::new(0.0).unwrap()),
        },
    };
    taffy::geometry::Rect {
        top: dimension_auto(value.top),
        right: dimension_auto(value.right),
        bottom: dimension_auto(value.bottom),
        left: dimension_auto(value.left),
    }
}

fn old_padding(value: Option<&LayerValue>) -> taffy::geometry::Rect<LengthPercentage> {
    let value = match value {
        Some(LayerValue::Value(PropertyValue::Insets(value))) => *value,
        _ => return taffy::geometry::Rect::zero(),
    };
    taffy::geometry::Rect {
        top: LengthPercentage::length(f32::from(value.top())),
        right: LengthPercentage::length(f32::from(value.right())),
        bottom: LengthPercentage::length(f32::from(value.bottom())),
        left: LengthPercentage::length(f32::from(value.left())),
    }
}

fn scalar(value: Option<&LayerValue>) -> Option<f32> {
    match value {
        Some(LayerValue::Value(PropertyValue::Scalar(value))) => Some(value.get()),
        _ => None,
    }
}

fn alignment_items(value: Option<&LayerValue>) -> Option<AlignItems> {
    alignment(value).map(|value| match value {
        AlignmentMode::Start => AlignItems::START,
        AlignmentMode::End => AlignItems::END,
        AlignmentMode::Center => AlignItems::CENTER,
        AlignmentMode::Stretch => AlignItems::STRETCH,
        AlignmentMode::Baseline => AlignItems::BASELINE,
        AlignmentMode::SpaceBetween | AlignmentMode::SpaceEvenly | AlignmentMode::SpaceAround => {
            AlignItems::STRETCH
        }
    })
}

fn alignment_self(value: Option<&LayerValue>) -> Option<AlignSelf> {
    alignment_items(value)
}

fn alignment_content(value: Option<&LayerValue>) -> Option<AlignContent> {
    alignment(value).map(|value| match value {
        AlignmentMode::Start => AlignContent::START,
        AlignmentMode::End => AlignContent::END,
        AlignmentMode::Center => AlignContent::CENTER,
        AlignmentMode::Stretch => AlignContent::STRETCH,
        AlignmentMode::Baseline => AlignContent::STRETCH,
        AlignmentMode::SpaceBetween => AlignContent::SPACE_BETWEEN,
        AlignmentMode::SpaceEvenly => AlignContent::SPACE_EVENLY,
        AlignmentMode::SpaceAround => AlignContent::SPACE_AROUND,
    })
}

fn alignment(value: Option<&LayerValue>) -> Option<AlignmentMode> {
    match value {
        Some(LayerValue::Value(PropertyValue::AlignmentMode(value))) => Some(*value),
        _ => None,
    }
}

fn old_alignment(
    snapshot: &OccurrenceSnapshot,
    direction: FlexDirection,
) -> (Option<AlignItems>, Option<taffy::prelude::JustifyContent>) {
    let Some(LayerValue::Value(PropertyValue::Alignment(Alignment {
        horizontal,
        vertical,
    }))) = property(snapshot, PropertyId::Alignment)
    else {
        return (None, None);
    };
    let horizontal = horizontal.and_then(horizontal_alignment);
    let vertical = vertical.and_then(vertical_alignment);
    if matches!(direction, FlexDirection::Row | FlexDirection::RowReverse) {
        (vertical, horizontal.and_then(axis_content))
    } else {
        (horizontal, vertical.and_then(axis_content))
    }
}

fn horizontal_alignment(axis: AlignmentAxis) -> Option<AlignItems> {
    match axis {
        AlignmentAxis::Start => Some(AlignItems::START),
        AlignmentAxis::Center => Some(AlignItems::CENTER),
        AlignmentAxis::End => Some(AlignItems::END),
        AlignmentAxis::Top | AlignmentAxis::Bottom => None,
    }
}

fn vertical_alignment(axis: AlignmentAxis) -> Option<AlignItems> {
    match axis {
        AlignmentAxis::Start | AlignmentAxis::Top => Some(AlignItems::START),
        AlignmentAxis::Center => Some(AlignItems::CENTER),
        AlignmentAxis::End | AlignmentAxis::Bottom => Some(AlignItems::END),
    }
}

fn axis_content(value: AlignItems) -> Option<taffy::prelude::JustifyContent> {
    Some(match value.keyword() {
        taffy::style::AlignItemsKeyword::Start => taffy::prelude::JustifyContent::START,
        taffy::style::AlignItemsKeyword::End => taffy::prelude::JustifyContent::END,
        taffy::style::AlignItemsKeyword::Center => taffy::prelude::JustifyContent::CENTER,
        _ => taffy::prelude::JustifyContent::START,
    })
}

fn track_values(value: Option<&LayerValue>) -> Vec<TrackSizingFunction> {
    let Some(LayerValue::Value(PropertyValue::Tracks(TrackListValue(values)))) = value else {
        return Vec::new();
    };
    values
        .iter()
        .map(|value| match value {
            TrackValue::Length(value) => TrackSizingFunction::from_length(value.get()),
            TrackValue::Percent(value) => TrackSizingFunction::from_percent(value.get()),
            TrackValue::Fr(value) => TrackSizingFunction::from_fr(value.get()),
            TrackValue::Auto => TrackSizingFunction::AUTO,
            TrackValue::MinContent => <TrackSizingFunction as TaffyMinContent>::MIN_CONTENT,
            TrackValue::MaxContent => <TrackSizingFunction as TaffyMaxContent>::MAX_CONTENT,
            TrackValue::MinMax { min, max } => TrackSizingFunction {
                min: min_track_sizing(*min),
                max: max_track_sizing(*max),
            },
        })
        .collect()
}

fn template_tracks(value: Option<&LayerValue>) -> Vec<taffy::style::GridTemplateComponent<String>> {
    track_values(value)
        .into_iter()
        .map(taffy::style::GridTemplateComponent::Single)
        .collect()
}

fn min_track_sizing(value: TrackMinBound) -> taffy::style::MinTrackSizingFunction {
    match value {
        TrackMinBound::Length(value) => {
            taffy::style::MinTrackSizingFunction::from_length(value.get())
        }
        TrackMinBound::Percent(value) => {
            taffy::style::MinTrackSizingFunction::from_percent(value.get())
        }
        TrackMinBound::Auto => taffy::style::MinTrackSizingFunction::AUTO,
        TrackMinBound::MinContent => {
            <taffy::style::MinTrackSizingFunction as TaffyMinContent>::MIN_CONTENT
        }
        TrackMinBound::MaxContent => {
            <taffy::style::MinTrackSizingFunction as TaffyMaxContent>::MAX_CONTENT
        }
    }
}

fn max_track_sizing(value: TrackMaxBound) -> taffy::style::MaxTrackSizingFunction {
    match value {
        TrackMaxBound::Length(value) => {
            taffy::style::MaxTrackSizingFunction::from_length(value.get())
        }
        TrackMaxBound::Percent(value) => {
            taffy::style::MaxTrackSizingFunction::from_percent(value.get())
        }
        TrackMaxBound::Fr(value) => taffy::style::MaxTrackSizingFunction::from_fr(value.get()),
        TrackMaxBound::Auto => taffy::style::MaxTrackSizingFunction::AUTO,
        TrackMaxBound::MinContent => {
            <taffy::style::MaxTrackSizingFunction as TaffyMinContent>::MIN_CONTENT
        }
        TrackMaxBound::MaxContent => {
            <taffy::style::MaxTrackSizingFunction as TaffyMaxContent>::MAX_CONTENT
        }
    }
}

fn placement(value: Option<&LayerValue>) -> Line<GridPlacement> {
    let Some(LayerValue::Value(PropertyValue::GridPlacement(GridPlacementValue { start, end }))) =
        value
    else {
        return Line {
            start: GridPlacement::Auto,
            end: GridPlacement::Auto,
        };
    };
    Line {
        start: grid_line(*start),
        end: grid_line(*end),
    }
}

fn grid_line(value: GridLineValue) -> GridPlacement {
    match value {
        GridLineValue::Auto => GridPlacement::Auto,
        GridLineValue::Line(value) => taffy::style_helpers::line(value as i16),
        GridLineValue::Span(value) => taffy::style_helpers::span(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::occurrence::HostKind;

    fn snapshot(
        key: NodeKey,
        children: Vec<NodeKey>,
        properties: Vec<(PropertyId, LayerValue)>,
    ) -> OccurrenceSnapshot {
        OccurrenceSnapshot {
            key,
            kind: HostKind::Box,
            root_role: None,
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
        }
    }

    #[test]
    fn fractional_siblings_round_accumulated_edges_without_a_gap() {
        let root = NodeKey {
            slot: 1,
            generation: 1,
        };
        let first = NodeKey {
            slot: 2,
            generation: 1,
        };
        let second = NodeKey {
            slot: 3,
            generation: 1,
        };
        let scalar = |value| crate::occurrence::FiniteScalar::new(value).unwrap();
        let root_properties = vec![
            (
                PropertyId::Layout,
                LayerValue::Value(PropertyValue::LayoutMode(LayoutMode::Row)),
            ),
            (
                PropertyId::Width,
                LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(scalar(
                    10.0,
                )))),
            ),
            (
                PropertyId::Height,
                LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(scalar(
                    1.0,
                )))),
            ),
        ];
        let child_properties = vec![(
            PropertyId::Width,
            LayerValue::Value(PropertyValue::Dimension(DimensionValue::Percent(scalar(
                0.33333334,
            )))),
        )];
        let snapshots = vec![
            snapshot(root, vec![first, second], root_properties),
            snapshot(first, Vec::new(), child_properties.clone()),
            snapshot(second, Vec::new(), child_properties),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&snapshots, &[root, first, second], &[root], &[], &[])
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize {
            width: 0.0,
            height: 0.0,
        };
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(10.0),
                AvailableConstraint::Definite(1.0),
                &mut measure,
            )
            .unwrap();
        let first_rect = geometry.iter().find(|item| item.key == first).unwrap().rect;
        let second_rect = geometry
            .iter()
            .find(|item| item.key == second)
            .unwrap()
            .rect;
        assert_eq!(first_rect.x + first_rect.width, second_rect.x);

        let updated = snapshot(
            first,
            Vec::new(),
            vec![(
                PropertyId::FlexGrow,
                LayerValue::Value(PropertyValue::Scalar(scalar(1.0))),
            )],
        );
        adapter
            .synchronize(&[updated], &[first], &[], &[], &[])
            .unwrap();
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(10.0),
                AvailableConstraint::Definite(1.0),
                &mut measure,
            )
            .unwrap();
        assert!(geometry.iter().any(|item| item.key == first));
    }

    #[test]
    fn floor_wrap_width_preserves_zero_and_fractional_discrepancy() {
        assert_eq!(wrap_width(-0.5), Err(TaffyAdapterError::NonFiniteGeometry));
        assert_eq!(wrap_width(0.99), Ok(0));
        assert_eq!(wrap_width(3.99), Ok(3));
        assert_eq!(
            round_rect(
                LogicalRect {
                    x: i32::MAX as f32,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                i32::MAX as f32,
                0.0,
            ),
            Err(TaffyAdapterError::PhysicalOverflow)
        );
    }

    #[test]
    fn parent_only_reorder_uses_explicit_final_list_without_parent_search() {
        let root = NodeKey {
            slot: 10,
            generation: 1,
        };
        let first = NodeKey {
            slot: 11,
            generation: 1,
        };
        let second = NodeKey {
            slot: 12,
            generation: 1,
        };
        let initial = vec![
            snapshot(root, vec![first, second], Vec::new()),
            snapshot(first, Vec::new(), Vec::new()),
            snapshot(second, Vec::new(), Vec::new()),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&initial, &[root, first, second], &[root], &[], &[])
            .unwrap();
        let mut reordered = snapshot(root, vec![second, first], Vec::new());
        reordered.structure_revision = 2;
        adapter
            .synchronize(&[reordered.clone()], &[], &[root], &[], &[])
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize::default();
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(10.0),
                AvailableConstraint::Definite(1.0),
                &mut measure,
            )
            .unwrap();
        let keys = geometry.iter().map(|item| item.key).collect::<Vec<_>>();
        assert_eq!(&keys[1..], &[second, first]);
        assert!(
            !adapter
                .tree
                .dirty(adapter.entry(root).unwrap().node)
                .unwrap()
        );
        adapter
            .synchronize(&[reordered], &[], &[root], &[], &[])
            .unwrap();
        assert!(
            !adapter
                .tree
                .dirty(adapter.entry(root).unwrap().node)
                .unwrap()
        );
    }

    #[test]
    fn sparse_frontier_rejects_unnamed_style_and_participation_changes() {
        let key = NodeKey {
            slot: 13,
            generation: 1,
        };
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(
                &[snapshot(key, Vec::new(), Vec::new())],
                &[key],
                &[],
                &[],
                &[],
            )
            .unwrap();
        let missing_snapshot = adapter.synchronize(&[], &[key], &[], &[], &[]);
        assert_eq!(
            missing_snapshot,
            Err(TaffyAdapterError::MissingSnapshot(key))
        );
        let missing_participation_snapshot = adapter.synchronize(
            &[],
            &[],
            &[],
            &[NodeParticipation {
                key,
                participates: false,
            }],
            &[],
        );
        assert_eq!(
            missing_participation_snapshot,
            Err(TaffyAdapterError::MissingSnapshot(key))
        );
    }

    #[test]
    fn keyed_preparation_scales_new_nodes_and_unchanged_parent_is_a_noop() {
        let root = NodeKey {
            slot: 100,
            generation: 1,
        };
        let children = (0..128)
            .map(|offset| NodeKey {
                slot: 101 + offset,
                generation: 1,
            })
            .collect::<Vec<_>>();
        let mut snapshots = vec![snapshot(root, children.clone(), Vec::new())];
        snapshots.extend(
            children
                .iter()
                .map(|key| snapshot(*key, Vec::new(), Vec::new())),
        );
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&snapshots, &children, &[root], &[], &[])
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize {
            width: 1.0,
            height: 1.0,
        };
        adapter
            .layout(
                root,
                AvailableConstraint::Definite(128.0),
                AvailableConstraint::Definite(1.0),
                &mut measure,
            )
            .unwrap();
        assert!(
            !adapter
                .tree
                .dirty(adapter.entry(root).unwrap().node)
                .unwrap()
        );

        let unchanged_parent = snapshot(root, children.clone(), Vec::new());
        adapter
            .synchronize(&[unchanged_parent], &[], &[root], &[], &[])
            .unwrap();
        assert!(
            !adapter
                .tree
                .dirty(adapter.entry(root).unwrap().node)
                .unwrap()
        );

        let presentation_only = snapshot(root, children, Vec::new());
        adapter
            .synchronize(&[presentation_only], &[root], &[], &[], &[])
            .unwrap();
        assert!(
            !adapter
                .tree
                .dirty(adapter.entry(root).unwrap().node)
                .unwrap()
        );
    }

    #[test]
    fn sparse_cross_parent_move_requires_and_installs_both_final_lists() {
        let root = NodeKey {
            slot: 20,
            generation: 1,
        };
        let old_parent = NodeKey {
            slot: 21,
            generation: 1,
        };
        let new_parent = NodeKey {
            slot: 22,
            generation: 1,
        };
        let child = NodeKey {
            slot: 23,
            generation: 1,
        };
        let initial = vec![
            snapshot(root, vec![old_parent, new_parent], Vec::new()),
            snapshot(old_parent, vec![child], Vec::new()),
            snapshot(new_parent, Vec::new(), Vec::new()),
            snapshot(child, Vec::new(), Vec::new()),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(
                &initial,
                &[root, old_parent, new_parent, child],
                &[root, old_parent, new_parent],
                &[],
                &[],
            )
            .unwrap();
        adapter
            .synchronize(
                &[
                    snapshot(old_parent, Vec::new(), Vec::new()),
                    snapshot(new_parent, vec![child], Vec::new()),
                ],
                &[],
                &[old_parent, new_parent],
                &[],
                &[],
            )
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize::default();
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(10.0),
                AvailableConstraint::Definite(2.0),
                &mut measure,
            )
            .unwrap();
        assert!(geometry.iter().any(|item| item.key == child));
    }

    #[test]
    fn retirement_is_postorder_and_generation_reuse_has_no_stale_edge() {
        let root = NodeKey {
            slot: 30,
            generation: 1,
        };
        let old_child = NodeKey {
            slot: 31,
            generation: 1,
        };
        let new_child = NodeKey {
            slot: 31,
            generation: 2,
        };
        let initial = vec![
            snapshot(root, vec![old_child], Vec::new()),
            snapshot(old_child, Vec::new(), Vec::new()),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&initial, &[root, old_child], &[root], &[], &[])
            .unwrap();
        adapter
            .synchronize(
                &[
                    {
                        let mut next_root = snapshot(root, vec![new_child], Vec::new());
                        next_root.structure_revision = 2;
                        next_root
                    },
                    snapshot(new_child, Vec::new(), Vec::new()),
                ],
                &[new_child],
                &[root],
                &[],
                &[old_child],
            )
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize::default();
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(4.0),
                AvailableConstraint::Definite(1.0),
                &mut measure,
            )
            .unwrap();
        assert!(geometry.iter().any(|item| item.key == new_child));
        assert!(!geometry.iter().any(|item| item.key == old_child));
    }

    #[test]
    fn retirement_uses_explicit_members_and_rescues_a_moved_descendant() {
        let root = NodeKey {
            slot: 32,
            generation: 1,
        };
        let old_parent = NodeKey {
            slot: 33,
            generation: 1,
        };
        let new_parent = NodeKey {
            slot: 34,
            generation: 1,
        };
        let rescued = NodeKey {
            slot: 35,
            generation: 1,
        };
        let doomed = NodeKey {
            slot: 36,
            generation: 1,
        };
        let initial = vec![
            snapshot(root, vec![old_parent, new_parent], Vec::new()),
            snapshot(old_parent, vec![rescued, doomed], Vec::new()),
            snapshot(new_parent, Vec::new(), Vec::new()),
            snapshot(rescued, Vec::new(), Vec::new()),
            snapshot(doomed, Vec::new(), Vec::new()),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(
                &initial,
                &[root, old_parent, new_parent, rescued, doomed],
                &[root, old_parent, new_parent],
                &[],
                &[],
            )
            .unwrap();
        let mut final_root = snapshot(root, vec![new_parent], Vec::new());
        final_root.structure_revision = 2;
        let mut final_new_parent = snapshot(new_parent, vec![rescued], Vec::new());
        final_new_parent.structure_revision = 2;
        adapter
            .synchronize(
                &[final_root, final_new_parent],
                &[],
                &[root, new_parent],
                &[],
                &[old_parent, doomed],
            )
            .unwrap();
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize {
            width: 1.0,
            height: 1.0,
        };
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::Definite(4.0),
                AvailableConstraint::Definite(2.0),
                &mut measure,
            )
            .unwrap();
        assert!(geometry.iter().any(|item| item.key == rescued));
        assert!(!geometry.iter().any(|item| item.key == doomed));
        assert!(!geometry.iter().any(|item| item.key == old_parent));
    }

    #[test]
    fn measurement_invalidation_changes_same_style_leaf_result() {
        use std::cell::Cell;

        let root = NodeKey {
            slot: 40,
            generation: 1,
        };
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(
                &[snapshot(root, Vec::new(), Vec::new())],
                &[root],
                &[],
                &[],
                &[],
            )
            .unwrap();
        let measured_width = Cell::new(2.0);
        let mut measure = |_: NodeKey, _: MeasureRequest| MeasuredSize {
            width: measured_width.get(),
            height: 1.0,
        };
        let first = adapter
            .layout(
                root,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut measure,
            )
            .unwrap();
        measured_width.set(5.0);
        adapter.invalidate_measurement(&[root]).unwrap();
        let second = adapter
            .layout(
                root,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut measure,
            )
            .unwrap();
        assert_ne!(first[0].logical.width, second[0].logical.width);
    }

    #[test]
    fn hidden_update_publishes_current_metadata() {
        let key = NodeKey {
            slot: 50,
            generation: 1,
        };
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&[snapshot(key, vec![], vec![])], &[key], &[], &[], &[])
            .unwrap();
        let mut hidden = snapshot(key, vec![], vec![]);
        hidden.hidden = true;
        adapter
            .synchronize(&[hidden], &[key], &[], &[], &[])
            .unwrap();
        let geometry = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(
            geometry[0].renderer_hidden,
            "captured metadata must reflect the current hidden state"
        );
        assert_eq!(geometry[0].rect.width, 0);

        let revealed = snapshot(key, vec![], vec![]);
        adapter
            .synchronize(&[revealed], &[key], &[], &[], &[])
            .unwrap();
        let geometry = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(!geometry[0].renderer_hidden);
        assert!(
            geometry[0].rect.width > 0,
            "unhide must not require a participation hint"
        );

        let initially_hidden_key = NodeKey {
            slot: 56,
            generation: 1,
        };
        let mut initially_hidden = snapshot(initially_hidden_key, vec![], vec![]);
        initially_hidden.hidden = true;
        adapter
            .synchronize(&[initially_hidden], &[initially_hidden_key], &[], &[], &[])
            .unwrap();
        let hidden_geometry = adapter
            .layout(
                initially_hidden_key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert_eq!(hidden_geometry[0].rect.width, 0);
        adapter
            .synchronize(
                &[snapshot(initially_hidden_key, vec![], vec![])],
                &[initially_hidden_key],
                &[],
                &[],
                &[],
            )
            .unwrap();
        let revealed_geometry = adapter
            .layout(
                initially_hidden_key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(revealed_geometry[0].rect.width > 0);

        let inactive_key = NodeKey {
            slot: 57,
            generation: 1,
        };
        adapter
            .synchronize(
                &[snapshot(inactive_key, vec![], vec![])],
                &[inactive_key],
                &[],
                &[NodeParticipation {
                    key: inactive_key,
                    participates: false,
                }],
                &[],
            )
            .unwrap();
        let inactive_geometry = adapter
            .layout(
                inactive_key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert_eq!(inactive_geometry[0].rect.width, 0);
        adapter
            .synchronize(
                &[snapshot(inactive_key, vec![], vec![])],
                &[inactive_key],
                &[],
                &[],
                &[],
            )
            .unwrap();
        let still_inactive_geometry = adapter
            .layout(
                inactive_key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert_eq!(still_inactive_geometry[0].rect.width, 0);
        adapter
            .synchronize(
                &[snapshot(inactive_key, vec![], vec![])],
                &[inactive_key],
                &[],
                &[NodeParticipation {
                    key: inactive_key,
                    participates: true,
                }],
                &[],
            )
            .unwrap();
        let active_geometry = adapter
            .layout(
                inactive_key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(active_geometry[0].rect.width > 0);

        let mut display_none = snapshot(
            key,
            vec![],
            vec![(
                PropertyId::Display,
                LayerValue::Value(PropertyValue::Display(DisplayMode::None)),
            )],
        );
        display_none.hidden = false;
        adapter
            .synchronize(
                &[display_none],
                &[key],
                &[],
                &[NodeParticipation {
                    key,
                    participates: true,
                }],
                &[],
            )
            .unwrap();
        let geometry = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(!geometry[0].renderer_hidden);
        assert!(geometry[0].display_none);
        assert_eq!(geometry[0].rect.width, 0);

        let flex = snapshot(
            key,
            vec![],
            vec![(
                PropertyId::Display,
                LayerValue::Value(PropertyValue::Display(DisplayMode::Flex)),
            )],
        );
        adapter
            .synchronize(
                &[flex],
                &[key],
                &[],
                &[NodeParticipation {
                    key,
                    participates: true,
                }],
                &[],
            )
            .unwrap();
        let geometry = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 2.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert!(!geometry[0].display_none);
        assert!(geometry[0].rect.width > 0);
    }

    #[test]
    fn content_wrap_width_is_the_inner_box_not_scrollable_extent() {
        let key = NodeKey {
            slot: 51,
            generation: 1,
        };
        let properties = vec![
            (
                PropertyId::Width,
                LayerValue::Value(PropertyValue::Dimension(DimensionValue::Length(
                    crate::occurrence::FiniteScalar::new(10.0).unwrap(),
                ))),
            ),
            (
                PropertyId::Padding,
                LayerValue::Value(PropertyValue::Insets(crate::presentation::Insets::new(
                    1, 1, 1, 1,
                ))),
            ),
        ];
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&[snapshot(key, vec![], properties)], &[key], &[], &[], &[])
            .unwrap();
        let geometry = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 30.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert_eq!(geometry[0].logical.width, 10.0);
        assert_eq!(
            geometry[0].logical_content_width, 8.0,
            "overflow content must not widen the final wrapping constraint"
        );
    }

    #[test]
    fn failed_measurement_cannot_be_reused_as_successful_zero_geometry() {
        let key = NodeKey {
            slot: 52,
            generation: 1,
        };
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(&[snapshot(key, vec![], vec![])], &[key], &[], &[], &[])
            .unwrap();
        let invalid = adapter.layout(
            key,
            AvailableConstraint::MaxContent,
            AvailableConstraint::MaxContent,
            &mut |_, _| MeasuredSize {
                width: f32::NAN,
                height: 1.0,
            },
        );
        assert_eq!(invalid, Err(TaffyAdapterError::NonFiniteGeometry));
        let recovered = adapter
            .layout(
                key,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize {
                    width: 5.0,
                    height: 1.0,
                },
            )
            .unwrap();
        assert_eq!(
            recovered[0].logical.width, 5.0,
            "failed candidate cache must not become a successful empty layout"
        );
    }

    #[test]
    fn subtree_retirement_needs_only_the_surviving_parent_snapshot() {
        let root = NodeKey {
            slot: 53,
            generation: 1,
        };
        let parent = NodeKey {
            slot: 54,
            generation: 1,
        };
        let leaf = NodeKey {
            slot: 55,
            generation: 1,
        };
        let mut adapter = TaffyLayoutAdapter::new();
        adapter
            .synchronize(
                &[
                    snapshot(root, vec![parent], vec![]),
                    snapshot(parent, vec![leaf], vec![]),
                    snapshot(leaf, vec![], vec![]),
                ],
                &[root, parent, leaf],
                &[root, parent],
                &[],
                &[],
            )
            .unwrap();
        adapter
            .synchronize(
                &[snapshot(root, vec![], vec![])],
                &[],
                &[root],
                &[],
                &[parent, leaf],
            )
            .unwrap();
        let geometry = adapter
            .layout(
                root,
                AvailableConstraint::MaxContent,
                AvailableConstraint::MaxContent,
                &mut |_, _| MeasuredSize::default(),
            )
            .unwrap();
        assert_eq!(
            geometry.iter().map(|item| item.key).collect::<Vec<_>>(),
            vec![root]
        );
    }
}
