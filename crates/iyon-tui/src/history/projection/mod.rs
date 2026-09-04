//! Width-dependent semantic projection of History into one vertical View.

use crate::{
    geometry::Size,
    perf::{self, Counter},
    physical::PhysicalRow,
    presentation::{
        ContentProvider, EmptyContentProvider, Insets, View,
        layout::{LayoutCache, measure_view_with_overlay_and_cache_and_content},
    },
    scene::{ResolutionOverlay, ResolveError, ResolveSession},
};

use super::unit::HistoryUnitLayoutKey;
use super::{
    FlowBoundary, History, HistoryUnitContent,
    native::frontier::{FrozenPhysicalRows, SpacingTransferState},
};
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
    /// Static History views may receive provider-owned resident decoration
    /// after a prefix has been accepted by native scrollback.
    view: Option<View>,
    content: PlannedContent,
}

enum PlannedContent {
    Static,
    Frozen(FrozenPhysicalRows),
    Live(View),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowItem {
    TopPadding,
    Unit(usize),
    Gap(usize),
    BottomPadding,
}

#[derive(Clone, Copy, Debug)]
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
    let mut content = EmptyContentProvider;
    let projection = project_into_session(history, size, &mut session, &mut content)?;
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
    let mut content = EmptyContentProvider;
    let projection =
        project_into_session_with_mode(history, size, &mut session, anchor, &mut content)?;
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
    content: &mut dyn ContentProvider,
) -> Result<HistoryProjectionParts, ResolveError> {
    project_into_session_with_mode(
        history,
        size,
        session,
        HistoryViewportAnchor::FollowEnd,
        content,
    )
}

/// Host projection mode includes the current `ContentHost` measurement when
/// native overflow metadata is calculated.
pub(crate) fn project_into_session_for_host(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
    anchor: HistoryViewportAnchor,
) -> Result<HistoryProjectionParts, ResolveError> {
    let mut content = EmptyContentProvider;
    project_into_session_for_host_with_content(history, size, session, anchor, &mut content)
}

pub(crate) fn project_into_session_for_host_with_content(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
    anchor: HistoryViewportAnchor,
    content: &mut dyn ContentProvider,
) -> Result<HistoryProjectionParts, ResolveError> {
    project_into_session_with_mode(history, size, session, anchor, content)
}

fn project_into_session_with_mode(
    history: &History,
    size: Size,
    session: &mut ResolveSession<'_>,
    anchor: HistoryViewportAnchor,
    content: &mut dyn ContentProvider,
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
                let cache_key = view.content_attachment_id().map_or_else(
                    || HistoryUnitLayoutKey::Static(view.id()),
                    |port_id| HistoryUnitLayoutKey::Content {
                        view: view.id(),
                        projection: content.projection_revision(port_id, content_width),
                    },
                );
                let height = history.prepare_unit_layout(index, content_width, cache_key.clone());
                Ok(UnitPlan {
                    boundary: unit.boundary,
                    height,
                    cache_key: Some(cache_key),
                    view: Some(content.history_view(view)),
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
                    view: None,
                    content: PlannedContent::Live(resolved),
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
    if let Some(rows) = frozen_static.clone()
        && let Some(plan) = plans.first_mut()
    {
        plan.height = Some(rows.len());
        plan.content = PlannedContent::Frozen(rows);
    }

    // Histories without a frozen native remainder can retain every unit's
    // presentation height. ContentHost revisions participate through their
    // provider projection keys.
    let retained_geometry = frozen_static.is_none() && !history.native.has_physical_rows();
    if retained_geometry {
        for (index, plan) in plans.iter_mut().enumerate() {
            if plan.height.is_none() {
                ensure_height(
                    history,
                    index,
                    plan,
                    units[index],
                    content_width,
                    overlay,
                    content,
                );
            }
        }
    }

    let retained_total = history.cached_total_flow_height();
    let total_flow_height = if retained_geometry && retained_total.is_some() {
        retained_total
            .expect("retained total checked above")
            .saturating_add(flow_overhead(history, &plans, top_padding))
    } else {
        items
            .iter()
            .copied()
            .map(|item| {
                item_height(
                    history,
                    &mut plans,
                    &units,
                    item,
                    content_width,
                    top_padding,
                    overlay,
                    content,
                )
            })
            .sum()
    };
    let protected_tail = protected_content_tail_bounds(&units, content, content_width);
    let use_cached_selection = retained_geometry && protected_tail.is_none();
    let capacity = usize::from(size.height);
    let overflow_rows = total_flow_height.saturating_sub(capacity);

    match anchor {
        HistoryViewportAnchor::FollowEnd => {
            if use_cached_selection {
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
            } else if let Some((blocker, stream)) = protected_tail {
                // An open ContentHost tail follows its semantic end, but resident
                // blockers before it form a protected band. Reserve that real flow
                // geometry first; only the remaining capacity belongs to the content
                // suffix. If the band
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
                    content,
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
                                content,
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
                        .filter(|item| content_tail_region_item(*item, stream))
                    {
                        let height = item_height(
                            history,
                            &mut plans,
                            &units,
                            item,
                            content_width,
                            top_padding,
                            overlay,
                            content,
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
                            content,
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
                        content,
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
                    content,
                );
            }
        }
        HistoryViewportAnchor::NativeFrontier => {
            if use_cached_selection {
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
                    content,
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
                    if let Some((visible, row_offset)) =
                        frozen_visible_rows(&plans[index], selected)
                        && !visible.is_empty()
                    {
                        frozen_overlay = Some(HistoryPhysicalOverlay {
                            row: rendered_row
                                .saturating_add(row_offset)
                                .min(usize::from(u16::MAX)) as u16,
                            rows: visible,
                        });
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

fn protected_content_tail_bounds(
    units: &[&super::HistoryUnit],
    content: &dyn ContentProvider,
    content_width: u16,
) -> Option<(usize, usize)> {
    let stream = units.len().checked_sub(1)?;
    let is_open_stream = match &units[stream].content {
        HistoryUnitContent::Static(view) if view.contains_content_identity() => {
            view.content_attachment_id().is_some_and(|port_id| {
                content
                    .history_rows(port_id, content_width)
                    .is_some_and(|rows| !rows.complete)
            })
        }
        _ => false,
    };
    if !is_open_stream {
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

fn content_tail_region_item(item: FlowItem, stream: usize) -> bool {
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
    content: &mut dyn ContentProvider,
) -> usize {
    items
        .iter()
        .copied()
        .filter(|item| protected_band_item(*item, blocker, stream))
        .map(|item| item_height(history, plans, units, item, width, 0, overlay, content))
        .sum()
}

fn item_height(
    history: &History,
    plans: &mut [UnitPlan],
    units: &[&super::HistoryUnit],
    item: FlowItem,
    width: u16,
    top_padding: usize,
    overlay: &ResolutionOverlay,
    content: &mut dyn ContentProvider,
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
                content,
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
    content: &mut dyn ContentProvider,
) {
    for item in items.iter().rev().copied() {
        let height = item_height(
            history,
            plans,
            units,
            item,
            width,
            top_padding,
            overlay,
            content,
        );
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
    content: &mut dyn ContentProvider,
) {
    for item in items.iter().copied() {
        if *remaining == 0 {
            break;
        }
        let height = item_height(
            history,
            plans,
            units,
            item,
            width,
            top_padding,
            overlay,
            content,
        );
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

fn ensure_height(
    history: &History,
    index: usize,
    plan: &mut UnitPlan,
    unit: &super::HistoryUnit,
    width: u16,
    overlay: &ResolutionOverlay,
    content: &mut dyn ContentProvider,
) -> usize {
    if let Some(height) = plan.height {
        perf::inc(Counter::HistoryCachedHeightHits);
        return height;
    }
    let height = match (&mut plan.content, &unit.content) {
        (PlannedContent::Static, HistoryUnitContent::Static(view)) => {
            let view = plan.view.as_ref().unwrap_or(view);
            view_height(view, width, overlay, content)
        }
        (PlannedContent::Frozen(rows), _) => rows.len(),
        (PlannedContent::Live(view), _) => view_height(view, width, overlay, content),
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
            let view = plan.view.as_ref().unwrap_or(view);
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
    }
}

fn push_spacer(children: &mut Vec<View>, rows: usize) {
    if rows == 0 {
        return;
    }
    children.push(View::spacer(rows.min(usize::from(u16::MAX)) as u16));
}

fn view_height(
    view: &View,
    width: u16,
    overlay: &ResolutionOverlay,
    content: &mut dyn ContentProvider,
) -> usize {
    perf::inc(Counter::HistoryUnitsMeasured);
    let mut cache = LayoutCache::default();
    usize::from(
        measure_view_with_overlay_and_cache_and_content(view, width, overlay, &mut cache, content)
            .height,
    )
}
