//! Bounded allocation over measured semantic facts.

use std::sync::Arc;

use crate::{
    geometry::Size,
    perf::{self, Counter},
    presentation::ir::HeightRule,
};

use super::{
    cache::{LayoutCache, PrepareKey},
    grid::{FlexMode, SpanRequirement, allocate_grid_tracks, span_extent, track_offset},
    measure::{MeasuredKind, MeasuredNode},
    tracks::allocate_tracks,
};

#[derive(Debug)]
pub(super) struct PreparedNode {
    pub(super) measured: Arc<MeasuredNode>,
    pub(super) size: Size,
    pub(super) core_size: Size,
    pub(super) content_offset_x: u16,
    pub(super) content_offset_y: u16,
    pub(super) complete: bool,
    pub(super) kind: PreparedKind,
}

#[derive(Debug)]
pub(super) struct PreparedChild {
    pub(super) x: u16,
    pub(super) y: u16,
    pub(super) node: Arc<PreparedNode>,
}

#[derive(Debug)]
pub(super) enum PreparedKind {
    Leaf,
    Children(Vec<PreparedChild>),
    Clamp {
        child: Arc<PreparedChild>,
    },
    RowViewport {
        child: Arc<PreparedChild>,
        skip_rows: u16,
    },
}

pub(super) fn prepare_node(
    measured: &Arc<MeasuredNode>,
    height_bound: Option<u16>,
    cache: &mut LayoutCache,
) -> Arc<PreparedNode> {
    let key = PrepareKey {
        measured: measured.key,
        height_bound,
    };
    if measured.cacheable
        && let Some(prepared) = cache.prepared(key)
    {
        return prepared;
    }

    let prepared = Arc::new(prepare_node_uncached(measured, height_bound, cache));
    if measured.cacheable {
        cache.store_prepared(key, Arc::clone(&prepared));
    }
    prepared
}

fn prepare_node_uncached(
    measured: &Arc<MeasuredNode>,
    height_bound: Option<u16>,
    cache: &mut LayoutCache,
) -> PreparedNode {
    perf::inc(Counter::PrepareNodeCalls);
    #[cfg(test)]
    super::record_prepare_node();
    let view = &measured.view;
    let decoration = measured.decoration;
    let height_capacity = height_bound
        .unwrap_or(u16::MAX)
        .min(view.decoration().bounds.height.normalized_max());
    let core_height_capacity = height_capacity.saturating_sub(decoration.vertical);
    let minimum_core_height = view
        .decoration()
        .bounds
        .height
        .min
        .saturating_sub(decoration.vertical)
        .min(core_height_capacity);
    let requested_core_height = match (view.height(), height_bound) {
        (HeightRule::Fill, Some(_)) => core_height_capacity,
        _ => measured.core_size.height.min(core_height_capacity),
    }
    .max(minimum_core_height);
    let (kind, kind_size, complete) =
        prepare_kind(measured, requested_core_height, height_bound, cache);
    let core_width = measured.size.width.saturating_sub(decoration.horizontal);
    let core_height = match view.height() {
        HeightRule::Fill => requested_core_height,
        HeightRule::Fit => kind_size.height.min(requested_core_height),
    };
    PreparedNode {
        measured: Arc::clone(measured),
        size: Size::new(
            core_width
                .saturating_add(decoration.horizontal)
                .max(view.decoration().bounds.width.min)
                .min(measured.width_capacity),
            core_height
                .saturating_add(decoration.vertical)
                .max(view.decoration().bounds.height.min)
                .min(height_capacity),
        ),
        core_size: Size::new(core_width, core_height),
        content_offset_x: decoration
            .left_border
            .saturating_add(decoration.left_padding),
        content_offset_y: decoration.top_border.saturating_add(decoration.top_padding),
        complete,
        kind,
    }
}

fn prepare_kind(
    measured: &Arc<MeasuredNode>,
    requested_core_height: u16,
    height_bound: Option<u16>,
    cache: &mut LayoutCache,
) -> (PreparedKind, Size, bool) {
    match &measured.kind {
        MeasuredKind::Text { metrics, .. } => (
            PreparedKind::Leaf,
            Size::new(metrics.width, metrics.row_count),
            metrics.fits && metrics.row_count <= requested_core_height,
        ),
        MeasuredKind::Spacer { rows } => (
            PreparedKind::Leaf,
            Size::new(0, (*rows).min(requested_core_height)),
            true,
        ),
        MeasuredKind::Container { child } => {
            let prepared = prepare_node(child, Some(requested_core_height), cache);
            let size = prepared.size;
            let complete = prepared.complete;
            (
                PreparedKind::Children(vec![PreparedChild {
                    x: 0,
                    y: 0,
                    node: prepared,
                }]),
                size,
                complete,
            )
        }
        MeasuredKind::ClampRows {
            child, max_rows, ..
        } => {
            let prepared = prepare_node(child, None, cache);
            let size = Size::new(prepared.size.width, prepared.size.height.min(*max_rows));
            let complete = prepared.complete;
            (
                PreparedKind::Clamp {
                    child: Arc::new(PreparedChild {
                        x: 0,
                        y: 0,
                        node: prepared,
                    }),
                },
                size,
                complete,
            )
        }
        MeasuredKind::RowViewport {
            width: _,
            child,
            skip_rows,
            visible_height,
            layout_height,
            intrinsic_content_height,
        } => {
            let prepared = prepare_node(child, *layout_height, cache);
            let child_height = prepared.size.height;
            let height = if *intrinsic_content_height {
                let remaining = child_height.saturating_sub(*skip_rows);
                if height_bound.is_some() {
                    requested_core_height.min(remaining)
                } else {
                    child_height
                }
            } else {
                visible_height.unwrap_or_else(|| child_height.saturating_sub(*skip_rows))
            };
            let complete = prepared.complete;
            (
                PreparedKind::RowViewport {
                    child: Arc::new(PreparedChild {
                        x: 0,
                        y: 0,
                        node: prepared,
                    }),
                    skip_rows: *skip_rows,
                },
                Size::new(measured.decoration.inner_width, height),
                complete,
            )
        }
        MeasuredKind::Column { children, gap } => {
            let tracks = children.iter().map(|child| child.track).collect::<Vec<_>>();
            let allocation = allocate_tracks(requested_core_height, *gap, &tracks, |index, _| {
                children[index].node.size.height
            });
            let mut prepared_children = Vec::with_capacity(children.len());
            let mut y = 0;
            let mut complete = true;
            for (index, child) in children.iter().enumerate() {
                let track = allocation.tracks[index];
                let prepared = prepare_node(&child.node, Some(track), cache);
                if child.node.size.height > track && !child.node.kind.is_clamp() {
                    complete = false;
                }
                complete &= prepared.complete;
                prepared_children.push(PreparedChild {
                    x: 0,
                    y,
                    node: prepared,
                });
                y = y.saturating_add(track).saturating_add(allocation.gap);
            }
            let used_height = allocation
                .tracks
                .iter()
                .map(|track| usize::from(*track))
                .sum::<usize>()
                .saturating_add(usize::from(allocation.gap) * tracks.len().saturating_sub(1));
            (
                PreparedKind::Children(prepared_children),
                Size::new(
                    measured.decoration.inner_width,
                    used_height.min(usize::from(u16::MAX)) as u16,
                ),
                complete,
            )
        }
        MeasuredKind::Row {
            children,
            allocation: _,
            gap,
            vertical_align,
        } => {
            let row_height = if measured.view.height() == HeightRule::Fill {
                requested_core_height
            } else {
                measured.core_size.height.min(requested_core_height)
            };
            let mut prepared_children = Vec::with_capacity(children.len());
            let mut x = 0;
            let mut complete = true;
            for child in children {
                let child_height = child.node.size.height.min(row_height);
                let y = match vertical_align {
                    crate::presentation::VerticalAlign::Top => 0,
                    crate::presentation::VerticalAlign::Center => {
                        row_height.saturating_sub(child_height) / 2
                    }
                    crate::presentation::VerticalAlign::Bottom => {
                        row_height.saturating_sub(child_height)
                    }
                };
                let prepared = prepare_node(&child.node, Some(row_height), cache);
                complete &= prepared.complete;
                prepared_children.push(PreparedChild {
                    x,
                    y,
                    node: prepared,
                });
                x = x.saturating_add(child.track_width).saturating_add(*gap);
            }
            (
                PreparedKind::Children(prepared_children),
                Size::new(measured.core_size.width, row_height),
                complete,
            )
        }
        MeasuredKind::Hanging {
            prefix_width,
            prefix,
            continuation_prefix,
            body,
        } => {
            if requested_core_height == 0 {
                return (
                    PreparedKind::Children(Vec::new()),
                    Size::new(measured.decoration.inner_width, 0),
                    true,
                );
            }
            let body = prepare_node(body, Some(requested_core_height), cache);
            let row_height = body.size.height.max(1).min(requested_core_height);
            let prefix_node = prepare_node(prefix, Some(1), cache);
            let prefix_complete = prefix_node.complete;
            let mut children = vec![PreparedChild {
                x: 0,
                y: 0,
                node: prefix_node,
            }];
            let mut complete =
                body.complete && prefix_complete && *prefix_width < measured.decoration.inner_width;
            for row in 1..row_height {
                let continuation = prepare_node(continuation_prefix, Some(1), cache);
                complete &= continuation.complete;
                children.push(PreparedChild {
                    x: 0,
                    y: row,
                    node: continuation,
                });
            }
            children.push(PreparedChild {
                x: *prefix_width,
                y: 0,
                node: body,
            });
            (
                PreparedKind::Children(children),
                Size::new(
                    measured.decoration.inner_width,
                    if measured.view.height() == HeightRule::Fill {
                        requested_core_height
                    } else {
                        row_height
                    },
                ),
                complete,
            )
        }
        MeasuredKind::Grid {
            columns,
            row_tracks,
            cells,
            row_gap,
            ..
        } => {
            let row_requirements = cells
                .iter()
                .map(|cell| SpanRequirement {
                    start: cell.row,
                    span: cell.row_span,
                    preferred: cell.node.size.height,
                })
                .collect::<Vec<_>>();
            let rows = allocate_grid_tracks(
                requested_core_height,
                *row_gap,
                row_tracks,
                &row_requirements,
                FlexMode::Fill,
            );
            let mut prepared_children = Vec::with_capacity(cells.len());
            let mut complete = true;
            for cell in cells {
                let area_x = track_offset(columns, cell.column);
                let area_y = track_offset(&rows, cell.row);
                let area_width = span_extent(columns, cell.column, cell.column_span);
                let area_height = span_extent(&rows, cell.row, cell.row_span);
                if cell.node.size.height > area_height && !cell.node.kind.is_clamp() {
                    complete = false;
                }
                let prepared = prepare_node(&cell.node, Some(area_height), cache);
                complete &= prepared.complete;
                let extra_x = area_width.saturating_sub(prepared.size.width);
                let extra_y = area_height.saturating_sub(prepared.size.height);
                let x = area_x.saturating_add(match cell.horizontal_align {
                    crate::presentation::HorizontalAlign::Start => 0,
                    crate::presentation::HorizontalAlign::Center => extra_x / 2,
                    crate::presentation::HorizontalAlign::End => extra_x,
                });
                let y = area_y.saturating_add(match cell.vertical_align {
                    crate::presentation::VerticalAlign::Top => 0,
                    crate::presentation::VerticalAlign::Center => extra_y / 2,
                    crate::presentation::VerticalAlign::Bottom => extra_y,
                });
                prepared_children.push(PreparedChild {
                    x,
                    y,
                    node: prepared,
                });
            }
            let used_width = span_extent(columns, 0, columns.tracks.len());
            let used_height = span_extent(&rows, 0, rows.tracks.len());
            (
                PreparedKind::Children(prepared_children),
                Size::new(used_width, used_height),
                complete,
            )
        }
    }
}
