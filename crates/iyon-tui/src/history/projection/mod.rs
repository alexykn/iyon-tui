//! Width-dependent semantic projection of History into one vertical View.

use std::sync::Arc;

use crate::{
    geometry::Size,
    perf::{self, Counter},
    physical::PhysicalRow,
    presentation::{Insets, View, layout::measure_view_with_overlay},
    scene::{ResolutionOverlay, ResolveError, ResolveSession},
    stream::StreamRowIndex,
};

use super::unit::HistoryUnitLayoutKey;
use super::{FlowBoundary, History, HistoryUnitContent, native::frontier::SpacingTransferState};
use crate::stream::FrozenPhysicalRows;
#[cfg(test)]
use crate::{component::ComponentRegistry, scene::ResolvedScene};

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub(crate) struct HistoryProjection {
    pub(crate) scene: ResolvedScene,
    pub(crate) frozen_overlay: Option<HistoryPhysicalOverlay>,
}

pub(crate) struct HistoryProjectionParts {
    pub(crate) view: View,
    pub(crate) frozen_overlay: Option<HistoryPhysicalOverlay>,
    pub(crate) overflow_rows: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryPhysicalOverlay {
    pub(crate) row: u16,
    pub(crate) rows: Vec<PhysicalRow>,
}

struct UnitPlan {
    boundary: FlowBoundary,
    height: Option<usize>,
    cache_key: Option<HistoryUnitLayoutKey>,
    content: PlannedContent,
}

enum PlannedContent {
    Static,
    Frozen(FrozenPhysicalRows),
    Live(View),
    Stream {
        index: Option<Arc<StreamRowIndex>>,
        start: crate::stream::StreamOffset,
        prefix: Option<FrozenPhysicalRows>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowItem {
    TopPadding,
    Unit(usize),
    Gap(usize),
    BottomPadding,
}

#[derive(Clone, Copy)]
struct Selected {
    offset: usize,
    height: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum HistoryViewportAnchor {
    #[default]
    FollowEnd,
    NativeFrontier,
}

#[cfg(test)]
pub(crate) fn project(
    history: &History,
    registry: &ComponentRegistry,
    size: Size,
) -> Result<HistoryProjection, ResolveError> {
    let mut session = ResolveSession::new(registry);
    let projection = project_into_session(history, size, &mut session)?;
    let scene = session.finish(projection.view);
    Ok(HistoryProjection {
        scene,
        frozen_overlay: projection.frozen_overlay,
    })
}

#[cfg(test)]
pub(crate) fn project_with_anchor(
    history: &History,
    registry: &ComponentRegistry,
    size: Size,
    anchor: HistoryViewportAnchor,
) -> Result<HistoryProjection, ResolveError> {
    let mut session = ResolveSession::new(registry);
    let projection = project_into_session_with_mode(history, size, &mut session, false, anchor)?;
    let scene = session.finish(projection.view);
    Ok(HistoryProjection {
        scene,
        frozen_overlay: projection.frozen_overlay,
    })
}

#[cfg(test)]
pub(crate) fn project_into_session(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
) -> Result<HistoryProjectionParts, ResolveError> {
    project_into_session_with_mode(
        history,
        size,
        session,
        false,
        HistoryViewportAnchor::FollowEnd,
    )
}

/// Host projection mode eagerly measures retained streams so native overflow
/// metadata accounts for sealed streams that are still resident in History.
pub(crate) fn project_into_session_for_host(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
    anchor: HistoryViewportAnchor,
) -> Result<HistoryProjectionParts, ResolveError> {
    project_into_session_with_mode(history, size, session, true, anchor)
}

fn project_into_session_with_mode(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
    eager_stream_overflow: bool,
    anchor: HistoryViewportAnchor,
) -> Result<HistoryProjectionParts, ResolveError> {
    let layout = history.layout();
    let content_width = size
        .width
        .saturating_sub(layout.padding.left.saturating_add(layout.padding.right));
    let units = history.units().collect::<Vec<_>>();
    let mut plans = units
        .iter()
        .enumerate()
        .map(|(index, unit)| match &unit.content {
            HistoryUnitContent::Static(view) => {
                let cache_key = HistoryUnitLayoutKey::Static(view.id());
                let height = history.prepare_unit_layout(index, content_width, cache_key.clone());
                Ok(UnitPlan {
                    boundary: unit.boundary,
                    height,
                    cache_key: Some(cache_key),
                    content: PlannedContent::Static,
                })
            }
            HistoryUnitContent::Live(view) => {
                let (resolved, dependencies) = session.resolve_root_with_dependencies(view)?;
                let cache_key = HistoryUnitLayoutKey::Live {
                    view: view.id(),
                    dependencies,
                };
                let height = history.prepare_unit_layout(index, content_width, cache_key.clone());
                Ok(UnitPlan {
                    boundary: unit.boundary,
                    height,
                    cache_key: Some(cache_key),
                    content: PlannedContent::Live(resolved),
                })
            }
            HistoryUnitContent::Stream(stream) => {
                let (start, prefix) = stream_projection_state(history, unit.id);
                let cache_key = HistoryUnitLayoutKey::Stream {
                    revision: stream.revision(),
                    base: stream.semantic_base(),
                    source_end: stream.source_end(),
                    indexed_from: start,
                    prefix_rows: prefix.as_ref().map_or(0, |rows| rows.as_slice().len()),
                };
                let height = history.prepare_unit_layout(index, content_width, cache_key.clone());
                Ok(UnitPlan {
                    boundary: unit.boundary,
                    height,
                    cache_key: Some(cache_key),
                    content: PlannedContent::Stream {
                        index: None,
                        start,
                        prefix,
                    },
                })
            }
        })
        .collect::<Result<Vec<_>, ResolveError>>()?;
    let overlay = session.overlay();

    let mut selected_units = vec![None; plans.len()];
    let mut selected_items = Vec::new();
    let mut remaining = usize::from(size.height);
    let top_padding = resident_top_padding(history);
    let items = flow_items(history, &plans);

    let frozen_static = frozen_static_rows(history);
    if let Some(rows) = frozen_static.clone() {
        if let Some(plan) = plans.first_mut() {
            plan.height = Some(rows.len());
            plan.content = PlannedContent::Frozen(rows);
        }
    }

    // Histories without a frozen native remainder can retain every unit's
    // presentation height, including stream revisions. Selection still uses
    // the protected-band/native paths below, but overflow geometry no longer
    // walks and prepares the entire stream-bearing flow on every update.
    let retained_geometry = frozen_static.is_none() && !history.native.has_physical_rows();
    if retained_geometry {
        for (index, plan) in plans.iter_mut().enumerate() {
            let lazy_sealed_stream = matches!(
                &units[index].content,
                HistoryUnitContent::Stream(stream)
                    if stream.is_sealed() && !eager_stream_overflow
            );
            if plan.height.is_none() && !lazy_sealed_stream {
                ensure_height(history, index, plan, units[index], content_width, overlay);
            }
        }
    }

    let retained_total = history.cached_total_flow_height();
    let (total_flow_height, has_unmeasured_stream) =
        if retained_geometry && retained_total.is_some() {
            (
                retained_total
                    .expect("retained total checked above")
                    .saturating_add(flow_overhead(history, &plans, top_padding)),
                false,
            )
        } else {
            items
                .iter()
                .copied()
                .map(|item| {
                    item_height_for_overflow(
                        history,
                        &mut plans,
                        &units,
                        item,
                        content_width,
                        top_padding,
                        eager_stream_overflow,
                        overlay,
                    )
                })
                .fold(
                    (0usize, false),
                    |(height, unknown), (item_height, item_unknown)| {
                        (height.saturating_add(item_height), unknown || item_unknown)
                    },
                )
        };
    let no_streams = retained_geometry
        && plans
            .iter()
            .all(|plan| !matches!(&plan.content, PlannedContent::Stream { .. }));
    let capacity = usize::from(size.height);
    let overflow_rows = total_flow_height.saturating_sub(capacity).max(usize::from(
        has_unmeasured_stream && total_flow_height >= capacity,
    ));

    match anchor {
        HistoryViewportAnchor::FollowEnd => {
            if no_streams {
                select_end_following_cached(
                    history,
                    &plans,
                    &units,
                    top_padding,
                    &mut remaining,
                    &mut selected_units,
                    &mut selected_items,
                    overlay,
                );
            } else if let Some((blocker, stream)) = protected_stream_bounds(&units) {
                // An open Stream tail follows its semantic end, but resident blockers
                // before it form a protected band. Reserve that real flow geometry first;
                // only the remaining capacity belongs to the Stream suffix. If the band
                // itself does not fit, retain the ordinary end-follow overflow behavior.
                let protected_height = protected_band_height(
                    history,
                    &mut plans,
                    &units,
                    &items,
                    blocker,
                    stream,
                    content_width,
                    overlay,
                );
                if protected_height <= remaining {
                    for item in items
                        .iter()
                        .copied()
                        .filter(|item| protected_band_item(*item, blocker, stream))
                    {
                        take_full(
                            item,
                            item_height(
                                history,
                                &mut plans,
                                &units,
                                item,
                                content_width,
                                top_padding,
                                overlay,
                            ),
                            &mut remaining,
                            &mut selected_units,
                            &mut selected_items,
                        );
                    }
                    for item in items
                        .iter()
                        .rev()
                        .copied()
                        .filter(|item| stream_region_item(*item, stream))
                    {
                        let height = item_height(
                            history,
                            &mut plans,
                            &units,
                            item,
                            content_width,
                            top_padding,
                            overlay,
                        );
                        take_selected(
                            item,
                            height,
                            &mut remaining,
                            &mut selected_units,
                            &mut selected_items,
                        );
                        if remaining == 0 {
                            break;
                        }
                    }
                    for item in items
                        .iter()
                        .rev()
                        .copied()
                        .filter(|item| preceding_item(*item, blocker))
                    {
                        let height = item_height(
                            history,
                            &mut plans,
                            &units,
                            item,
                            content_width,
                            top_padding,
                            overlay,
                        );
                        take_selected(
                            item,
                            height,
                            &mut remaining,
                            &mut selected_units,
                            &mut selected_items,
                        );
                        if remaining == 0 {
                            break;
                        }
                    }
                } else {
                    select_end_following(
                        history,
                        &mut plans,
                        &units,
                        &items,
                        content_width,
                        top_padding,
                        &mut remaining,
                        &mut selected_units,
                        &mut selected_items,
                        overlay,
                    );
                }
            } else {
                select_end_following(
                    history,
                    &mut plans,
                    &units,
                    &items,
                    content_width,
                    top_padding,
                    &mut remaining,
                    &mut selected_units,
                    &mut selected_items,
                    overlay,
                );
            }
        }
        HistoryViewportAnchor::NativeFrontier => {
            if no_streams {
                select_native_frontier_cached(
                    history,
                    &plans,
                    &units,
                    top_padding,
                    &mut remaining,
                    &mut selected_units,
                    &mut selected_items,
                );
            } else {
                select_native_frontier(
                    history,
                    &mut plans,
                    &units,
                    &items,
                    content_width,
                    top_padding,
                    &mut remaining,
                    &mut selected_units,
                    &mut selected_items,
                    overlay,
                );
            }
        }
    }
    let mut children = Vec::new();
    let slack = remaining;
    let native_anchored = history.native.has_physical_rows();
    if !native_anchored {
        push_spacer(&mut children, slack);
    }
    let mut rendered_row = if native_anchored { 0 } else { slack };
    let mut frozen_overlay = None;
    for item in items {
        match item {
            FlowItem::Unit(index) => {
                if let Some(selected) = selected_units[index] {
                    ensure_stream_index(&mut plans[index], units[index], content_width);
                    if let Some((visible, row_offset)) =
                        frozen_visible_rows(&plans[index], selected)
                    {
                        if !visible.is_empty() {
                            frozen_overlay = Some(HistoryPhysicalOverlay {
                                row: rendered_row
                                    .saturating_add(row_offset)
                                    .min(usize::from(u16::MAX))
                                    as u16,
                                rows: visible,
                            });
                        }
                    }
                    children.push(unit_view(&plans[index], units[index], selected, overlay));
                    rendered_row = rendered_row.saturating_add(selected.height);
                } else if let PlannedContent::Live(view) = &plans[index].content {
                    children.push(View::row_viewport_with_height(view.clone(), 0, Some(0)));
                }
            }
            FlowItem::TopPadding | FlowItem::Gap(_) | FlowItem::BottomPadding => {
                if let Some((_, selected)) = selected_items
                    .iter()
                    .find(|(candidate, _)| *candidate == item)
                {
                    push_spacer(&mut children, selected.height);
                    rendered_row = rendered_row.saturating_add(selected.height);
                }
            }
        }
    }
    if native_anchored {
        push_spacer(&mut children, slack);
    }

    crate::history::trace::trace_projection(
        size.width,
        size.height,
        0,
        size.height,
        match anchor {
            HistoryViewportAnchor::FollowEnd => "FollowEnd",
            HistoryViewportAnchor::NativeFrontier => "NativeFrontier",
        },
        history.native.physical_rows_inserted,
        history.native.last_native_unit.map(|id| id.value()),
        history.units.len(),
        history
            .native
            .stream
            .as_ref()
            .map(|state| state.committed_through.as_u64()),
        total_flow_height,
        overflow_rows,
        slack,
    );

    let mut root = View::vertical(|column| {
        column.children(children);
    });
    root = root.fill_width().fill_height().padding(Insets::new(
        0,
        layout.padding.right,
        0,
        layout.padding.left,
    ));
    Ok(HistoryProjectionParts {
        view: root,
        frozen_overlay,
        overflow_rows,
    })
}

fn protected_stream_bounds(units: &[&super::HistoryUnit]) -> Option<(usize, usize)> {
    let stream = units.len().checked_sub(1)?;
    let HistoryUnitContent::Stream(stream_content) = &units[stream].content else {
        return None;
    };
    if stream_content.is_sealed() {
        return None;
    }
    let blocker =
        (0..stream).find(|index| matches!(&units[*index].content, HistoryUnitContent::Live(_)))?;
    Some((blocker, stream))
}

fn protected_band_item(item: FlowItem, blocker: usize, stream: usize) -> bool {
    match item {
        FlowItem::Unit(index) | FlowItem::Gap(index) => blocker <= index && index < stream,
        FlowItem::TopPadding | FlowItem::BottomPadding => false,
    }
}

fn stream_region_item(item: FlowItem, stream: usize) -> bool {
    match item {
        FlowItem::BottomPadding => true,
        FlowItem::Gap(index) | FlowItem::Unit(index) => index == stream,
        FlowItem::TopPadding => false,
    }
}

fn preceding_item(item: FlowItem, blocker: usize) -> bool {
    match item {
        FlowItem::TopPadding => true,
        FlowItem::Unit(index) | FlowItem::Gap(index) => index < blocker,
        FlowItem::BottomPadding => false,
    }
}

fn protected_band_height(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    items: &[FlowItem],
    blocker: usize,
    stream: usize,
    width: u16,
    overlay: &ResolutionOverlay,
) -> usize {
    items
        .iter()
        .copied()
        .filter(|item| protected_band_item(*item, blocker, stream))
        .map(|item| item_height(history, plans, units, item, width, 0, overlay))
        .sum()
}

fn item_height_for_overflow(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    item: FlowItem,
    width: u16,
    top_padding: usize,
    eager_stream_overflow: bool,
    overlay: &ResolutionOverlay,
) -> (usize, bool) {
    match item {
        FlowItem::TopPadding => (top_padding, false),
        FlowItem::BottomPadding => (usize::from(history.layout().padding.bottom), false),
        FlowItem::Gap(index) => (resident_gap(history, index, &plans[index]), false),
        FlowItem::Unit(index) => {
            perf::inc(Counter::HistoryUnitsExamined);
            match (&plans[index].content, &units[index].content) {
                (PlannedContent::Static, HistoryUnitContent::Static(_)) => (
                    ensure_height(
                        history,
                        index,
                        &mut plans[index],
                        units[index],
                        width,
                        overlay,
                    ),
                    false,
                ),
                (PlannedContent::Frozen(rows), _) => (rows.len(), false),
                (PlannedContent::Live(_), _) => (
                    ensure_height(
                        history,
                        index,
                        &mut plans[index],
                        units[index],
                        width,
                        overlay,
                    ),
                    false,
                ),
                (PlannedContent::Stream { .. }, HistoryUnitContent::Stream(stream))
                    if eager_stream_overflow || !stream.is_sealed() =>
                {
                    (
                        ensure_height(
                            history,
                            index,
                            &mut plans[index],
                            units[index],
                            width,
                            overlay,
                        ),
                        false,
                    )
                }
                (
                    PlannedContent::Stream {
                        index: row_index,
                        prefix,
                        ..
                    },
                    _,
                ) => {
                    if let Some(height) = plans[index].height {
                        return (height, false);
                    }
                    let prefix_height = prefix.as_ref().map_or(0, |rows| rows.as_slice().len());
                    let known = row_index.as_ref().map_or(0, |index| index.anchors.len());
                    (prefix_height.saturating_add(known), row_index.is_none())
                }
                _ => unreachable!("History overflow plan does not match its unit"),
            }
        }
    }
}

fn item_height(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    item: FlowItem,
    width: u16,
    top_padding: usize,
    overlay: &ResolutionOverlay,
) -> usize {
    match item {
        FlowItem::TopPadding => top_padding,
        FlowItem::BottomPadding => usize::from(history.layout().padding.bottom),
        FlowItem::Gap(index) => resident_gap(history, index, &plans[index]),
        FlowItem::Unit(index) => {
            perf::inc(Counter::HistoryUnitsExamined);
            ensure_height(
                history,
                index,
                &mut plans[index],
                units[index],
                width,
                overlay,
            )
        }
    }
}

fn take_full(
    item: FlowItem,
    height: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
) {
    if height == 0 {
        return;
    }
    debug_assert!(*remaining >= height);
    let selected = Selected { offset: 0, height };
    match item {
        FlowItem::Unit(index) => selected_units[index] = Some(selected),
        _ => selected_items.push((item, selected)),
    }
    *remaining = (*remaining).saturating_sub(height);
}

fn flow_overhead(history: &History, plans: &[UnitPlan], top_padding: usize) -> usize {
    let mut overhead = top_padding.saturating_add(usize::from(history.layout().padding.bottom));
    for (index, plan) in plans.iter().enumerate() {
        if has_predecessor_gap(history, index, plan) {
            overhead = overhead.saturating_add(resident_gap(history, index, plan));
        }
    }
    overhead
}

fn select_end_following_cached(
    history: &History,
    plans: &[UnitPlan],
    units: &[&super::HistoryUnit],
    top_padding: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
    overlay: &ResolutionOverlay,
) {
    take_selected(
        FlowItem::BottomPadding,
        usize::from(history.layout().padding.bottom),
        remaining,
        selected_units,
        selected_items,
    );
    for index in (0..units.len()).rev() {
        let height = history
            .unit_height(index)
            .expect("retained static/live unit must have a cached height");
        perf::inc(Counter::HistoryUnitsExamined);
        if let PlannedContent::Live(view) = &plans[index].content
            && flexible_height(view, overlay)
            && *remaining > 0
            && height > *remaining
        {
            take_bounded(index, *remaining, remaining, selected_units);
        } else {
            take_selected(
                FlowItem::Unit(index),
                height,
                remaining,
                selected_units,
                selected_items,
            );
        }
        if *remaining == 0 {
            break;
        }
        if has_predecessor_gap(history, index, &plans[index]) {
            take_selected(
                FlowItem::Gap(index),
                resident_gap(history, index, &plans[index]),
                remaining,
                selected_units,
                selected_items,
            );
        }
        if *remaining == 0 {
            break;
        }
    }
    if *remaining > 0 {
        take_selected(
            FlowItem::TopPadding,
            top_padding,
            remaining,
            selected_units,
            selected_items,
        );
    }
}

fn select_native_frontier_cached(
    history: &History,
    plans: &[UnitPlan],
    units: &[&super::HistoryUnit],
    top_padding: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
) {
    take_front_selected(
        FlowItem::TopPadding,
        top_padding,
        remaining,
        selected_units,
        selected_items,
    );
    for index in 0..units.len() {
        if *remaining == 0 {
            return;
        }
        if has_predecessor_gap(history, index, &plans[index]) {
            take_front_selected(
                FlowItem::Gap(index),
                resident_gap(history, index, &plans[index]),
                remaining,
                selected_units,
                selected_items,
            );
        }
        if *remaining == 0 {
            return;
        }
        let height = history
            .unit_height(index)
            .expect("retained static/live unit must have a cached height");
        perf::inc(Counter::HistoryUnitsExamined);
        take_front_selected(
            FlowItem::Unit(index),
            height,
            remaining,
            selected_units,
            selected_items,
        );
    }
    if *remaining > 0 {
        take_front_selected(
            FlowItem::BottomPadding,
            usize::from(history.layout().padding.bottom),
            remaining,
            selected_units,
            selected_items,
        );
    }
}

fn take_front_selected(
    item: FlowItem,
    height: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
) {
    if height == 0 || *remaining == 0 {
        return;
    }
    let visible = (*remaining).min(height);
    let selected = Selected {
        offset: 0,
        height: visible,
    };
    match item {
        FlowItem::Unit(index) => selected_units[index] = Some(selected),
        _ => selected_items.push((item, selected)),
    }
    *remaining = (*remaining).saturating_sub(visible);
}

fn select_end_following(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    items: &[FlowItem],
    width: u16,
    top_padding: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
    overlay: &ResolutionOverlay,
) {
    for item in items.iter().rev().copied() {
        let height = item_height(history, plans, units, item, width, top_padding, overlay);
        if let FlowItem::Unit(index) = item
            && let PlannedContent::Live(view) = &plans[index].content
            && flexible_height(view, overlay)
            && *remaining > 0
            && height > *remaining
        {
            take_bounded(index, *remaining, remaining, selected_units);
        } else {
            take_selected(item, height, remaining, selected_units, selected_items);
        }
        if *remaining == 0 {
            break;
        }
    }
}

fn select_native_frontier(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    items: &[FlowItem],
    width: u16,
    top_padding: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
    overlay: &ResolutionOverlay,
) {
    for item in items.iter().copied() {
        if *remaining == 0 {
            break;
        }
        let height = item_height(history, plans, units, item, width, top_padding, overlay);
        if height == 0 {
            continue;
        }
        let visible = (*remaining).min(height);
        let selected = Selected {
            offset: 0,
            height: visible,
        };
        match item {
            FlowItem::Unit(index) => selected_units[index] = Some(selected),
            _ => selected_items.push((item, selected)),
        }
        *remaining = (*remaining).saturating_sub(visible);
    }
}

fn take_bounded(
    index: usize,
    height: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
) {
    if height == 0 {
        return;
    }
    selected_units[index] = Some(Selected { offset: 0, height });
    *remaining = 0;
}

fn flexible_height(view: &View, overlay: &ResolutionOverlay) -> bool {
    if view.height() == crate::presentation::ir::HeightRule::Fill {
        return true;
    }
    match view.kind() {
        crate::presentation::ir::ViewKind::Column(column) => column.children.iter().any(|child| {
            matches!(
                child.track,
                crate::presentation::ir::TrackSize::Flex { .. }
                    | crate::presentation::ir::TrackSize::FlexMax { .. }
            )
        }),
        crate::presentation::ir::ViewKind::Grid(grid) => grid.rows.iter().any(|track| {
            matches!(
                track,
                crate::presentation::ir::TrackSize::Flex { .. }
                    | crate::presentation::ir::TrackSize::FlexMax { .. }
            )
        }),
        crate::presentation::ir::ViewKind::Container(container) => {
            flexible_height(&container.child, overlay)
        }
        crate::presentation::ir::ViewKind::ComponentSlot(slot) => overlay
            .component(slot.id)
            .is_some_and(|snapshot| flexible_height(&snapshot.view, overlay)),
        _ => false,
    }
}

fn frozen_visible_rows(plan: &UnitPlan, selected: Selected) -> Option<(Vec<PhysicalRow>, usize)> {
    let (rows, prefix_len) = match &plan.content {
        PlannedContent::Frozen(rows) => (rows, rows.as_slice().len()),
        PlannedContent::Stream {
            prefix: Some(rows), ..
        } => (rows, rows.as_slice().len()),
        _ => return None,
    };
    let end = selected.offset.saturating_add(selected.height);
    let visible_end = end.min(prefix_len);
    let visible_start = selected.offset.min(visible_end);
    (visible_start < visible_end).then(|| {
        (
            rows.as_slice()[visible_start..visible_end].to_vec(),
            visible_start.saturating_sub(selected.offset),
        )
    })
}

fn stream_projection_state(
    history: &History,
    unit: super::HistoryUnitId,
) -> (crate::stream::StreamOffset, Option<FrozenPhysicalRows>) {
    let semantic_base = history
        .units
        .iter()
        .find(|candidate| candidate.id == unit)
        .and_then(|candidate| match &candidate.content {
            HistoryUnitContent::Stream(stream) => Some(stream.semantic_base()),
            _ => None,
        })
        .unwrap_or(crate::stream::StreamOffset::ZERO);
    let Some(state) = history
        .native
        .stream
        .as_ref()
        .filter(|state| state.unit == unit)
    else {
        return (semantic_base, None);
    };
    match &state.partial {
        Some(crate::stream::StreamPartialTransfer::FrozenAtomic {
            source_end,
            rows,
            committed_rows,
            ..
        }) => (
            *source_end,
            Some(FrozenPhysicalRows::new(
                rows.as_slice()[*committed_rows..].to_vec(),
            )),
        ),
        None => (state.committed_through, None),
    }
}

fn resident_top_padding(history: &History) -> usize {
    match &history.native.top_padding {
        SpacingTransferState::Semantic => usize::from(history.layout().padding.top),
        SpacingTransferState::Frozen(rows) => rows.as_slice().len(),
        SpacingTransferState::Native => 0,
    }
}

fn frozen_static_rows(history: &History) -> Option<FrozenPhysicalRows> {
    history
        .native
        .frozen_static
        .as_ref()
        .map(|frozen| frozen.rows.clone())
}

fn has_predecessor_gap(history: &History, index: usize, plan: &UnitPlan) -> bool {
    if !matches!(plan.boundary, FlowBoundary::Default) {
        return false;
    }
    if index > 0 {
        return true;
    }
    let Some(_) = history.native.last_native_unit else {
        return false;
    };
    !matches!(
        history.native.leading_gap,
        Some(SpacingTransferState::Native)
    )
}

fn resident_gap(history: &History, index: usize, _plan: &UnitPlan) -> usize {
    if index > 0 {
        return usize::from(history.layout().gap);
    }
    match history.native.leading_gap.as_ref() {
        Some(SpacingTransferState::Frozen(rows)) => rows.as_slice().len(),
        Some(SpacingTransferState::Native) => 0,
        Some(SpacingTransferState::Semantic) | None => usize::from(history.layout().gap),
    }
}

fn flow_items(history: &History, plans: &[UnitPlan]) -> Vec<FlowItem> {
    let mut items = vec![FlowItem::TopPadding];
    for (index, plan) in plans.iter().enumerate() {
        if has_predecessor_gap(history, index, plan) {
            items.push(FlowItem::Gap(index));
        }
        items.push(FlowItem::Unit(index));
    }
    items.push(FlowItem::BottomPadding);
    items
}

fn ensure_stream_index(plan: &mut UnitPlan, unit: &super::HistoryUnit, width: u16) {
    let PlannedContent::Stream { index, start, .. } = &mut plan.content else {
        return;
    };
    if index.is_some() {
        return;
    }
    let HistoryUnitContent::Stream(stream) = &unit.content else {
        unreachable!("stream plan does not match its unit");
    };
    *index = Some(stream.prepare_from(*start, width));
}

fn ensure_height(
    history: &History,
    index: usize,
    plan: &mut UnitPlan,
    unit: &super::HistoryUnit,
    width: u16,
    overlay: &ResolutionOverlay,
) -> usize {
    if let Some(height) = plan.height {
        perf::inc(Counter::HistoryCachedHeightHits);
        return height;
    }
    let height = match (&mut plan.content, &unit.content) {
        (PlannedContent::Static, HistoryUnitContent::Static(view)) => {
            view_height(view, width, overlay)
        }
        (PlannedContent::Frozen(rows), _) => rows.len(),
        (PlannedContent::Live(view), _) => view_height(view, width, overlay),
        (
            PlannedContent::Stream {
                index,
                start,
                prefix,
            },
            HistoryUnitContent::Stream(stream),
        ) => {
            if index.is_none() {
                perf::inc(Counter::HistoryUnitsMeasured);
            }
            let prepared = index.get_or_insert_with(|| stream.prepare_from(*start, width));
            prefix.as_ref().map_or(0, |rows| rows.as_slice().len()) + prepared.anchors.len()
        }
        _ => unreachable!("History projection plan does not match its unit"),
    };
    plan.height = Some(height);
    if plan.cache_key.is_some() {
        history.record_unit_height(index, height);
    }
    height
}

fn take_selected(
    item: FlowItem,
    height: usize,
    remaining: &mut usize,
    selected_units: &mut [Option<Selected>],
    selected_items: &mut Vec<(FlowItem, Selected)>,
) {
    if height == 0 || *remaining == 0 {
        return;
    }
    let visible = (*remaining).min(height);
    let selected = Selected {
        offset: height.saturating_sub(visible),
        height: visible,
    };
    match item {
        FlowItem::Unit(index) => selected_units[index] = Some(selected),
        _ => selected_items.push((item, selected)),
    }
    *remaining = (*remaining).saturating_sub(visible);
}

fn unit_view(
    plan: &UnitPlan,
    unit: &super::HistoryUnit,
    selected: Selected,
    overlay: &ResolutionOverlay,
) -> View {
    match &plan.content {
        PlannedContent::Static => {
            let HistoryUnitContent::Static(view) = &unit.content else {
                unreachable!("static plan must match static unit")
            };
            View::row_viewport_with_height(
                view.clone(),
                selected.offset.min(usize::from(u16::MAX)) as u16,
                Some(selected.height.min(usize::from(u16::MAX)) as u16),
            )
        }
        PlannedContent::Frozen(_) => {
            View::spacer(selected.height.min(usize::from(u16::MAX)) as u16)
        }
        PlannedContent::Live(view) => {
            if selected.offset == 0
                && selected.height < plan.height.unwrap_or(selected.height)
                && flexible_height(view, overlay)
            {
                View::bounded_row_viewport(
                    view.clone(),
                    selected.height.min(usize::from(u16::MAX)) as u16,
                )
            } else {
                View::row_viewport_with_height(
                    view.clone(),
                    selected.offset.min(usize::from(u16::MAX)) as u16,
                    Some(selected.height.min(usize::from(u16::MAX)) as u16),
                )
            }
        }
        PlannedContent::Stream { index, prefix, .. } => {
            let HistoryUnitContent::Stream(stream) = &unit.content else {
                unreachable!("stream plan must match stream unit")
            };
            let index = index
                .as_ref()
                .expect("selected stream must have a prepared index");
            let prefix_len = prefix.as_ref().map_or(0, |rows| rows.as_slice().len());
            let end = selected.offset.saturating_add(selected.height);
            let prefix_end = end.min(prefix_len);
            let prefix_start = selected.offset.min(prefix_end);
            let prefix_height = prefix_end.saturating_sub(prefix_start);
            let semantic_offset = selected.offset.saturating_sub(prefix_len);
            let semantic_height = selected.height.saturating_sub(prefix_height);
            let mut children = Vec::new();
            if prefix_height > 0 {
                children.push(View::spacer(prefix_height.min(usize::from(u16::MAX)) as u16));
            }
            if semantic_height > 0 {
                children.push(stream.window_view(
                    index,
                    semantic_offset,
                    semantic_height.min(usize::from(u16::MAX)) as u16,
                ));
            }
            View::column(children, 0)
        }
    }
}

fn push_spacer(children: &mut Vec<View>, rows: usize) {
    if rows == 0 {
        return;
    }
    children.push(View::spacer(rows.min(usize::from(u16::MAX)) as u16));
}

fn view_height(view: &View, width: u16, overlay: &ResolutionOverlay) -> usize {
    perf::inc(Counter::HistoryUnitsMeasured);
    usize::from(measure_view_with_overlay(view, width, overlay).height)
}
