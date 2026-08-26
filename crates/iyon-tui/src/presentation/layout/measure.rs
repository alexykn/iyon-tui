//! Backend-neutral semantic measurement used by the layout pipeline.

use std::sync::Arc;

use crate::{
    component::ComponentId,
    geometry::Size,
    perf::{self, Counter},
    presentation::{
        ir::{
            ColumnView, GridView, HangingView, PersistentSeq, RowView, RowViewportView, TextView,
            TrackSize, View, ViewKind, WidthRule,
        },
        wrap::{TextFlowMetrics, text_flow_metrics},
    },
    scene::ResolutionOverlay,
};

use super::{
    cache::{LayoutCache, MeasureKey},
    grid::{FlexMode, SpanRequirement, allocate_grid_tracks, span_extent},
    tracks::{TrackAllocation, allocate_tracks},
};

pub(super) use super::cache::WidthIntent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DecorationMetrics {
    pub(super) left_border: u16,
    pub(super) right_border: u16,
    pub(super) top_border: u16,
    pub(super) bottom_border: u16,
    pub(super) left_padding: u16,
    pub(super) right_padding: u16,
    pub(super) top_padding: u16,
    pub(super) bottom_padding: u16,
    pub(super) horizontal: u16,
    pub(super) vertical: u16,
    pub(super) inner_width: u16,
}

pub(super) fn decoration_metrics(view: &View, width: u16) -> DecorationMetrics {
    let (left_border, right_border, top_border, bottom_border) = view
        .decoration()
        .border
        .as_ref()
        .map_or((0, 0, 0, 0), |border| {
            (
                border.left_width(),
                border.right_width(),
                border.top_height(),
                border.bottom_height(),
            )
        });
    let border_width = left_border.saturating_add(right_border);
    let padding_capacity = width.saturating_sub(border_width);
    let left_padding = view
        .decoration()
        .padding
        .left
        .min(padding_capacity.saturating_sub(1));
    let right_padding = view
        .decoration()
        .padding
        .right
        .min(padding_capacity.saturating_sub(left_padding.saturating_add(1)));
    let horizontal = border_width
        .saturating_add(left_padding)
        .saturating_add(right_padding);
    let top_padding = view.decoration().padding.top;
    let bottom_padding = view.decoration().padding.bottom;
    let vertical = top_border
        .saturating_add(bottom_border)
        .saturating_add(top_padding)
        .saturating_add(bottom_padding);
    DecorationMetrics {
        left_border,
        right_border,
        top_border,
        bottom_border,
        left_padding,
        right_padding,
        top_padding,
        bottom_padding,
        horizontal,
        vertical,
        inner_width: width.saturating_sub(horizontal),
    }
}

#[derive(Debug)]
pub(super) struct MeasuredNode {
    pub(super) view: View,
    pub(super) key: MeasureKey,
    pub(super) cacheable: bool,
    pub(super) component: Option<ComponentId>,
    pub(super) component_scope: Option<ComponentId>,
    pub(super) width_capacity: u16,
    pub(super) decoration: DecorationMetrics,
    pub(super) size: Size,
    pub(super) core_size: Size,
    pub(super) kind: MeasuredKind,
}

#[derive(Debug)]
pub(super) enum MeasuredKind {
    Text {
        text: Arc<TextView>,
        metrics: TextFlowMetrics,
    },
    Spacer {
        rows: u16,
    },
    Container {
        child: Arc<MeasuredNode>,
    },
    Column {
        children: Vec<MeasuredColumnChild>,
        gap: u16,
    },
    Row {
        allocation: TrackAllocation,
        children: Vec<MeasuredRowChild>,
        gap: u16,
        vertical_align: crate::presentation::VerticalAlign,
    },
    Hanging {
        prefix_width: u16,
        prefix: Arc<MeasuredNode>,
        continuation_prefix: Arc<MeasuredNode>,
        body: Arc<MeasuredNode>,
    },
    ClampRows {
        child: Arc<MeasuredNode>,
        max_rows: u16,
        overflow: crate::presentation::OverflowIndicator,
    },
    RowViewport {
        width: u16,
        child: Arc<MeasuredNode>,
        skip_rows: u16,
        visible_height: Option<u16>,
        layout_height: Option<u16>,
        intrinsic_content_height: bool,
    },
    Grid {
        columns: TrackAllocation,
        row_tracks: PersistentSeq<TrackSize>,
        intrinsic_rows: TrackAllocation,
        cells: Vec<MeasuredGridCell>,
        row_gap: u16,
    },
}

#[derive(Debug)]
pub(super) struct MeasuredColumnChild {
    pub(super) track: TrackSize,
    pub(super) node: Arc<MeasuredNode>,
}

#[derive(Debug)]
pub(super) struct MeasuredRowChild {
    pub(super) track_width: u16,
    pub(super) node: Arc<MeasuredNode>,
}

#[derive(Debug)]
pub(super) struct MeasuredGridCell {
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) row_span: usize,
    pub(super) column_span: usize,
    pub(super) horizontal_align: crate::presentation::HorizontalAlign,
    pub(super) vertical_align: crate::presentation::VerticalAlign,
    pub(super) node: Arc<MeasuredNode>,
}

pub(super) fn measure_node(
    view: &View,
    width: u16,
    intent: WidthIntent,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> Arc<MeasuredNode> {
    let (key, cacheable) = match view.kind() {
        ViewKind::ComponentSlot(slot) => {
            let snapshot = overlay.component(slot.id);
            (
                snapshot
                    .map(|snapshot| {
                        MeasureKey::with_component(view, snapshot.view.id(), width, intent)
                    })
                    .unwrap_or_else(|| MeasureKey::ordinary(view, width, intent)),
                snapshot.is_some_and(|snapshot| !snapshot.view.contains_component_identity()),
            )
        }
        _ => (
            MeasureKey::ordinary(view, width, intent),
            !view.contains_component_identity(),
        ),
    };
    if cacheable && let Some(measured) = cache.measured(key) {
        return measured;
    }

    let measured = Arc::new(measure_node_uncached(
        view,
        width,
        intent,
        overlay,
        component_scope,
        key,
        cacheable,
        cache,
    ));
    if cacheable {
        cache.store_measured(key, Arc::clone(&measured));
    }
    measured
}

#[allow(clippy::too_many_arguments)]
fn measure_node_uncached(
    view: &View,
    width: u16,
    intent: WidthIntent,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    key: MeasureKey,
    cacheable: bool,
    cache: &mut LayoutCache,
) -> MeasuredNode {
    perf::inc(Counter::MeasureNodeCalls);
    #[cfg(test)]
    super::record_measure_node();
    let bounds = view.decoration().bounds;
    let width_capacity = width.min(bounds.width.normalized_max());
    let decoration = decoration_metrics(view, width_capacity);
    let (component, child_scope) = match view.kind() {
        ViewKind::ComponentSlot(slot) => (Some(slot.id), Some(slot.id)),
        _ => (None, component_scope),
    };
    let kind = measure_kind(view, decoration.inner_width, overlay, child_scope, cache);
    let core_size = kind.intrinsic_size();
    let core_width = match (intent, view.width()) {
        (WidthIntent::ForceFit, _) | (_, WidthRule::Fit) => core_size.width,
        (_, WidthRule::Fill) => decoration.inner_width,
    };
    let size = Size::new(
        core_width
            .saturating_add(decoration.horizontal)
            .max(bounds.width.min)
            .min(width_capacity),
        core_size
            .height
            .saturating_add(decoration.vertical)
            .max(bounds.height.min)
            .min(bounds.height.normalized_max()),
    );
    MeasuredNode {
        view: view.clone(),
        key,
        cacheable,
        component,
        component_scope: child_scope,
        width_capacity,
        decoration,
        size,
        core_size,
        kind,
    }
}

fn measure_kind(
    view: &View,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    match view.kind() {
        ViewKind::Text(text) => {
            let metrics = text_flow_metrics(text, width);
            MeasuredKind::Text {
                text: Arc::clone(text),
                metrics,
            }
        }
        ViewKind::Spacer { rows } => MeasuredKind::Spacer { rows: *rows },
        ViewKind::Container(container) => MeasuredKind::Container {
            child: measure_node(
                &container.child,
                width,
                WidthIntent::Semantic,
                overlay,
                component_scope,
                cache,
            ),
        },
        ViewKind::Hanging(hanging) => {
            measure_hanging(hanging, width, overlay, component_scope, cache)
        }
        ViewKind::ClampRows(clamp) => {
            let child = measure_node(
                &clamp.child,
                width,
                WidthIntent::Semantic,
                overlay,
                component_scope,
                cache,
            );
            MeasuredKind::ClampRows {
                child,
                max_rows: clamp.max_rows,
                overflow: clamp.overflow.clone(),
            }
        }
        ViewKind::RowViewport(viewport) => {
            measure_viewport(viewport, width, overlay, component_scope, cache)
        }
        ViewKind::Column(column) => measure_column(column, width, overlay, component_scope, cache),
        ViewKind::Row(row) => measure_row(row, width, overlay, component_scope, cache),
        ViewKind::Grid(grid) => measure_grid(grid, width, overlay, component_scope, cache),
        ViewKind::ComponentSlot(slot) => {
            let snapshot = overlay
                .component(slot.id)
                .unwrap_or_else(|| panic!("component overlay missing {:?}", slot.id));
            MeasuredKind::Container {
                child: measure_node(
                    &snapshot.view,
                    width,
                    WidthIntent::Semantic,
                    overlay,
                    Some(slot.id),
                    cache,
                ),
            }
        }
    }
}

fn measure_hanging(
    hanging: &HangingView,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    let prefix = measure_node(
        &hanging.prefix,
        u16::MAX,
        WidthIntent::ForceFit,
        overlay,
        component_scope,
        cache,
    );
    let prefix_width = prefix.size.width;
    let body_width = width.saturating_sub(prefix_width).max(1);
    let body = measure_node(
        &hanging.body,
        body_width,
        WidthIntent::Semantic,
        overlay,
        component_scope,
        cache,
    );
    let continuation_prefix = measure_node(
        &hanging.continuation_prefix,
        prefix_width,
        WidthIntent::Semantic,
        overlay,
        component_scope,
        cache,
    );
    MeasuredKind::Hanging {
        prefix_width,
        prefix,
        continuation_prefix,
        body,
    }
}

fn measure_column(
    column: &ColumnView,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    let children = column
        .children
        .iter()
        .map(|child| MeasuredColumnChild {
            track: child.track,
            node: measure_node(
                &child.view,
                width,
                WidthIntent::Semantic,
                overlay,
                component_scope,
                cache,
            ),
        })
        .collect::<Vec<_>>();
    MeasuredKind::Column {
        children,
        gap: column.gap,
    }
}

fn measure_row(
    row: &RowView,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    let tracks = row
        .children
        .iter()
        .map(|child| child.track)
        .collect::<Vec<_>>();
    let allocation = allocate_tracks(width, row.gap, &tracks, |index, remaining| {
        measure_node(
            &row.children[index].view,
            remaining,
            WidthIntent::ForceFit,
            overlay,
            component_scope,
            cache,
        )
        .size
        .width
    });
    let children = row
        .children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let track_width = allocation.tracks[index];
            MeasuredRowChild {
                track_width,
                node: measure_node(
                    &child.view,
                    track_width,
                    WidthIntent::Semantic,
                    overlay,
                    component_scope,
                    cache,
                ),
            }
        })
        .collect::<Vec<_>>();
    MeasuredKind::Row {
        allocation,
        children,
        gap: row.gap,
        vertical_align: row.vertical_align,
    }
}

fn measure_grid(
    grid: &GridView,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    let column_requirements = grid
        .cells
        .iter()
        .map(|cell| SpanRequirement {
            start: cell.column,
            span: usize::from(cell.column_span),
            preferred: measure_node(
                &cell.view,
                width,
                WidthIntent::ForceFit,
                overlay,
                component_scope,
                cache,
            )
            .size
            .width,
        })
        .collect::<Vec<_>>();
    let columns = allocate_grid_tracks(
        width,
        grid.column_gap,
        &grid.columns,
        &column_requirements,
        FlexMode::Fill,
    );
    let cells = grid
        .cells
        .iter()
        .map(|cell| {
            let cell_width = span_extent(&columns, cell.column, usize::from(cell.column_span));
            MeasuredGridCell {
                row: cell.row,
                column: cell.column,
                row_span: usize::from(cell.row_span),
                column_span: usize::from(cell.column_span),
                horizontal_align: cell.horizontal_align,
                vertical_align: cell.vertical_align,
                node: measure_node(
                    &cell.view,
                    cell_width,
                    WidthIntent::Semantic,
                    overlay,
                    component_scope,
                    cache,
                ),
            }
        })
        .collect::<Vec<_>>();
    let row_requirements = cells
        .iter()
        .map(|cell| SpanRequirement {
            start: cell.row,
            span: cell.row_span,
            preferred: cell.node.size.height,
        })
        .collect::<Vec<_>>();
    let intrinsic_rows = allocate_grid_tracks(
        u16::MAX,
        grid.row_gap,
        &grid.rows,
        &row_requirements,
        FlexMode::Intrinsic,
    );
    MeasuredKind::Grid {
        columns,
        row_tracks: grid.rows.clone(),
        intrinsic_rows,
        cells,
        row_gap: grid.row_gap,
    }
}

fn measure_viewport(
    viewport: &RowViewportView,
    width: u16,
    overlay: &ResolutionOverlay,
    component_scope: Option<ComponentId>,
    cache: &mut LayoutCache,
) -> MeasuredKind {
    let child = measure_node(
        &viewport.child,
        width,
        WidthIntent::Semantic,
        overlay,
        component_scope,
        cache,
    );
    MeasuredKind::RowViewport {
        width,
        child,
        skip_rows: viewport.skip_rows,
        visible_height: viewport.visible_height,
        layout_height: viewport.layout_height,
        intrinsic_content_height: viewport.intrinsic_content_height,
    }
}

impl MeasuredKind {
    pub(super) fn intrinsic_size(&self) -> Size {
        match self {
            Self::Text { metrics, .. } => Size::new(metrics.width, metrics.row_count),
            Self::Spacer { rows } => Size::new(0, *rows),
            Self::Container { child } => child.size,
            Self::Hanging {
                prefix_width, body, ..
            } => Size::new(
                prefix_width.saturating_add(body.size.width),
                body.size.height.max(1),
            ),
            Self::ClampRows {
                child, max_rows, ..
            } => Size::new(child.size.width, child.size.height.min(*max_rows)),
            Self::RowViewport {
                width,
                child,
                skip_rows,
                visible_height,
                intrinsic_content_height,
                ..
            } => Size::new(
                *width,
                if *intrinsic_content_height {
                    child.size.height
                } else {
                    visible_height.unwrap_or_else(|| child.size.height.saturating_sub(*skip_rows))
                },
            ),
            Self::Column { children, gap } => {
                let width = children
                    .iter()
                    .map(|child| child.node.size.width)
                    .max()
                    .unwrap_or(0);
                let height = children
                    .iter()
                    .map(|child| track_intrinsic_height(child.track, child.node.size.height))
                    .map(usize::from)
                    .sum::<usize>()
                    .saturating_add(usize::from(column_gap(*gap, children.len())));
                Size::new(width, height.min(usize::from(u16::MAX)) as u16)
            }
            Self::Row {
                allocation,
                children,
                gap,
                ..
            } => Size::new(
                allocation
                    .tracks
                    .iter()
                    .copied()
                    .sum::<u16>()
                    .saturating_add(column_gap(*gap, allocation.tracks.len())),
                children
                    .iter()
                    .map(|child| child.node.size.height)
                    .max()
                    .unwrap_or(0),
            ),
            Self::Grid {
                columns,
                intrinsic_rows,
                ..
            } => Size::new(
                allocation_extent(columns),
                allocation_extent(intrinsic_rows),
            ),
        }
    }

    pub(super) fn is_clamp(&self) -> bool {
        matches!(self, Self::ClampRows { .. })
    }
}

fn track_intrinsic_height(track: TrackSize, height: u16) -> u16 {
    match track {
        TrackSize::Fixed(value) => value,
        TrackSize::Content { max } => max.map_or(height, |value| height.min(value)),
        TrackSize::Flex { .. } => height,
        TrackSize::FlexMax { max, .. } => height.min(max),
    }
}

fn column_gap(gap: u16, count: usize) -> u16 {
    gap.saturating_mul(count.saturating_sub(1).min(usize::from(u16::MAX)) as u16)
}

fn allocation_extent(allocation: &TrackAllocation) -> u16 {
    let tracks = allocation
        .tracks
        .iter()
        .map(|track| usize::from(*track))
        .sum::<usize>();
    let gaps =
        usize::from(allocation.gap).saturating_mul(allocation.tracks.len().saturating_sub(1));
    tracks.saturating_add(gaps).min(usize::from(u16::MAX)) as u16
}
