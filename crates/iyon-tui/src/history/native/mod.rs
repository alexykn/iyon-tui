//! Private native scrollback ownership for generic History.

pub(super) mod frontier;

use crate::{
    backend::NativeHistorySink,
    physical::PhysicalRow,
    presentation::{
        ContentProvider, EmptyContentProvider, HistoryContentRows, layout::compile_view_with_theme,
    },
};

use super::{FlowBoundary, History, HistoryUnitContent, HistoryUnitId};
pub(super) use frontier::NativeFrontier;
use frontier::{
    FrozenContentRemainder, FrozenPhysicalRows, FrozenStaticRemainder, SpacingTransferState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeBlockReason {
    Live,
    ContentHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeTransferStatus {
    /// The native frontier advanced; this may retire semantic-only stream
    /// content without inserting a physical row.
    Progress,
    Idle,
    SinkBlocked,
    SemanticBlocked {
        unit: HistoryUnitId,
        reason: NativeBlockReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeTransferOutcome {
    pub(crate) requested: usize,
    pub(crate) inserted: usize,
    pub(crate) status: NativeTransferStatus,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NativeTransferError<E> {
    Sink(E),
    InvalidAcknowledgement { requested: usize, accepted: usize },
}

#[cfg(test)]
pub(crate) fn transfer_native_prefix<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    width: u16,
    max_rows: usize,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    transfer_native_prefix_with_theme(history, sink, width, max_rows, &crate::Theme::default())
}

pub(crate) fn transfer_native_prefix_with_theme<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    width: u16,
    max_rows: usize,
    theme: &crate::Theme,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let mut content = EmptyContentProvider;
    transfer_native_prefix_with_theme_and_content(
        history,
        sink,
        width,
        max_rows,
        theme,
        &mut content,
    )
}

pub(crate) fn transfer_native_prefix_with_theme_and_content<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    width: u16,
    max_rows: usize,
    theme: &crate::Theme,
    content: &mut dyn ContentProvider,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let before = history
        .units
        .iter()
        .map(|unit| unit.id.value())
        .collect::<std::collections::HashSet<_>>();
    let outcome = transfer_native_prefix_inner(history, sink, width, max_rows, theme, content)?;
    let after = history
        .units
        .iter()
        .map(|unit| unit.id.value())
        .collect::<std::collections::HashSet<_>>();
    for unit_id in before.difference(&after) {
        content.history_unit_retired(*unit_id);
    }
    history.native.record_physical_rows(outcome.inserted);
    // Native promotion changes the display frontier even when it inserts no
    // physical rows (for example, retiring a zero-row stream/unit). Keep that
    // revision separate from semantic History revision so retained SceneHost
    // frames can refresh the History branch without rebuilding the body.
    if outcome.inserted > 0 || matches!(outcome.status, NativeTransferStatus::Progress) {
        history.bump_native_revision();
    }
    Ok(outcome)
}

fn transfer_native_prefix_inner<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    width: u16,
    max_rows: usize,
    theme: &crate::Theme,
    content: &mut dyn ContentProvider,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    if max_rows == 0 || width == 0 || history.units.is_empty() {
        return Ok(outcome(0, 0, NativeTransferStatus::Idle));
    }

    if let Some(rows) = prepare_top_padding(history, width) {
        return transfer_spacing(&mut history.native.top_padding, sink, rows, max_rows);
    }

    let gap_was_uninitialized = history.native.leading_gap.is_none();
    if let Some(rows) = prepare_leading_gap(history, width) {
        let result = transfer_spacing(
            history.native.leading_gap.as_mut().expect("gap state"),
            sink,
            rows,
            max_rows,
        );
        if gap_was_uninitialized
            && result
                .as_ref()
                .map_or(true, |outcome| outcome.inserted == 0)
        {
            history.native.leading_gap = None;
        }
        return result;
    }

    if history.native.frozen_content.is_some() {
        return transfer_frozen_content(history, sink, max_rows, content);
    }

    if history.native.frozen_static.is_some() {
        return transfer_frozen_static(history, sink, max_rows);
    }

    let unit_id = history.units.front().expect("nonempty History").id;
    match &history.units.front().expect("nonempty History").content {
        HistoryUnitContent::Live(_) => Ok(outcome(
            0,
            0,
            NativeTransferStatus::SemanticBlocked {
                unit: unit_id,
                reason: NativeBlockReason::Live,
            },
        )),
        HistoryUnitContent::Static(view) if view.contains_content_identity() => {
            let Some(port_id) = view.content_attachment_id() else {
                return Ok(outcome(
                    0,
                    0,
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit_id,
                        reason: NativeBlockReason::ContentHost,
                    },
                ));
            };
            let Some(rows) = content.history_rows(port_id, width) else {
                return Ok(outcome(
                    0,
                    0,
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit_id,
                        reason: NativeBlockReason::ContentHost,
                    },
                ));
            };
            if rows.rows.is_empty() {
                if rows.complete {
                    retire_front(history);
                    return transfer_native_prefix_inner(
                        history, sink, width, max_rows, theme, content,
                    );
                }
                return Ok(outcome(
                    0,
                    0,
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit_id,
                        reason: NativeBlockReason::ContentHost,
                    },
                ));
            }
            transfer_content(history, sink, port_id, rows, max_rows, content)
        }
        HistoryUnitContent::Static(view) => {
            let rows = static_rows(view, width, history.layout(), theme);
            if rows.is_empty() {
                retire_front(history);
                return transfer_native_prefix_inner(
                    history, sink, width, max_rows, theme, content,
                );
            }
            transfer_static(history, sink, rows, max_rows)
        }
    }
}

fn outcome(
    requested: usize,
    inserted: usize,
    status: NativeTransferStatus,
) -> NativeTransferOutcome {
    NativeTransferOutcome {
        requested,
        inserted,
        status,
    }
}

fn spacing_rows(
    state: &SpacingTransferState,
    width: u16,
    semantic_count: usize,
) -> Option<Vec<PhysicalRow>> {
    match state {
        SpacingTransferState::Native => None,
        SpacingTransferState::Frozen(rows) => Some(rows.as_slice().to_vec()),
        SpacingTransferState::Semantic => {
            (semantic_count > 0).then(|| NativeFrontier::blank_rows(width, semantic_count))
        }
    }
}

fn prepare_top_padding(history: &mut History, width: u16) -> Option<Vec<PhysicalRow>> {
    spacing_rows(
        &history.native.top_padding,
        width,
        usize::from(history.layout().padding.top),
    )
}

fn prepare_leading_gap(history: &mut History, width: u16) -> Option<Vec<PhysicalRow>> {
    if history.native.last_native_unit.is_none() {
        return None;
    }
    let unit = history.units.front().expect("nonempty History");
    if !matches!(unit.boundary, FlowBoundary::Default) {
        return None;
    }
    if history.native.leading_gap.is_none() {
        if history.layout().gap == 0 {
            return None;
        }
        history.native.leading_gap = Some(SpacingTransferState::Semantic);
    }
    let state = history.native.leading_gap.as_ref().expect("gap state");
    spacing_rows(state, width, usize::from(history.layout().gap))
}

struct InsertAck {
    requested: usize,
    accepted: usize,
}

fn insert_prefix<S: NativeHistorySink>(
    sink: &mut S,
    rows: &[PhysicalRow],
    max_rows: usize,
) -> Result<InsertAck, NativeTransferError<S::Error>> {
    let requested = rows.len().min(max_rows);
    let accepted = sink
        .insert_history_rows(&rows[..requested])
        .map_err(NativeTransferError::Sink)?;
    if accepted > requested {
        return Err(NativeTransferError::InvalidAcknowledgement {
            requested,
            accepted,
        });
    }
    Ok(InsertAck {
        requested,
        accepted,
    })
}

fn transfer_spacing<S: NativeHistorySink>(
    state: &mut SpacingTransferState,
    sink: &mut S,
    rows: Vec<PhysicalRow>,
    max_rows: usize,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let ack = insert_prefix(sink, &rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }
    if ack.accepted == rows.len() {
        *state = SpacingTransferState::Native;
    } else {
        *state =
            SpacingTransferState::Frozen(FrozenPhysicalRows::new(rows[ack.accepted..].to_vec()));
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn transfer_static<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    rows: Vec<PhysicalRow>,
    max_rows: usize,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let ack = insert_prefix(sink, &rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }
    if ack.accepted == rows.len() {
        cross_zero_spacing(history);
        retire_front(history);
    } else {
        cross_zero_spacing(history);
        history.native.frozen_static = Some(FrozenStaticRemainder {
            unit: history.units.front().expect("static unit").id,
            rows: FrozenPhysicalRows::new(rows[ack.accepted..].to_vec()),
        });
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn transfer_content<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    port_id: u64,
    payload: HistoryContentRows,
    max_rows: usize,
    content: &mut dyn ContentProvider,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let unit_id = history.units.front().expect("content unit").id;
    let ack = insert_prefix(sink, &payload.rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }
    let accepted_content = ack
        .accepted
        .saturating_sub(payload.content_start)
        .min(payload.content_end.saturating_sub(payload.content_start));
    let accepted_leading = ack.accepted.min(payload.leading_padding);
    let accepted_trailing = ack
        .accepted
        .saturating_sub(payload.content_end)
        .min(payload.trailing_padding);
    content.history_rows_committed(
        port_id,
        ack.accepted,
        accepted_content,
        accepted_leading,
        accepted_trailing,
    );
    cross_zero_spacing(history);
    if ack.accepted < payload.rows.len() {
        let content_start = payload.content_start.saturating_sub(ack.accepted);
        let content_end = payload.content_end.saturating_sub(ack.accepted);
        let trailing_padding = payload
            .trailing_padding
            .saturating_sub(ack.accepted.saturating_sub(payload.content_end));
        history.native.frozen_content = Some(FrozenContentRemainder {
            unit: unit_id,
            port_id,
            rows: FrozenPhysicalRows::new(payload.rows[ack.accepted..].to_vec()),
            complete: payload.complete,
            content_start,
            content_end,
            leading_padding: payload.leading_padding.saturating_sub(ack.accepted),
            trailing_padding,
        });
    } else if payload.complete {
        retire_front(history);
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn transfer_frozen_content<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    max_rows: usize,
    content: &mut dyn ContentProvider,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let frozen = history
        .native
        .frozen_content
        .as_ref()
        .expect("frozen content");
    let port_id = frozen.port_id;
    let content_start = frozen.content_start;
    let content_end = frozen.content_end;
    let complete = frozen.complete;
    let leading_padding = frozen.leading_padding;
    let trailing_padding = frozen.trailing_padding;
    let rows = frozen.rows.as_slice().to_vec();
    let ack = insert_prefix(sink, &rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }
    let accepted_content = ack
        .accepted
        .saturating_sub(content_start)
        .min(content_end.saturating_sub(content_start));
    let accepted_leading = ack.accepted.min(leading_padding);
    let accepted_trailing = ack
        .accepted
        .saturating_sub(content_end)
        .min(trailing_padding);
    content.history_rows_committed(
        port_id,
        ack.accepted,
        accepted_content,
        accepted_leading,
        accepted_trailing,
    );
    if ack.accepted == rows.len() {
        history.native.frozen_content = None;
        if complete {
            retire_front(history);
        }
    } else if let Some(frozen) = history.native.frozen_content.as_mut() {
        frozen.rows = FrozenPhysicalRows::new(rows[ack.accepted..].to_vec());
        frozen.content_start = content_start.saturating_sub(ack.accepted);
        frozen.content_end = content_end.saturating_sub(ack.accepted);
        frozen.leading_padding = leading_padding.saturating_sub(ack.accepted);
        frozen.trailing_padding =
            trailing_padding.saturating_sub(ack.accepted.saturating_sub(content_end));
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn transfer_frozen_static<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    max_rows: usize,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let frozen = history
        .native
        .frozen_static
        .as_ref()
        .expect("frozen static");
    let rows = frozen.rows.as_slice();
    let ack = insert_prefix(sink, rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }
    if ack.accepted == rows.len() {
        retire_front(history);
    } else {
        let remainder = rows[ack.accepted..].to_vec();
        history.native.frozen_static.as_mut().unwrap().rows = FrozenPhysicalRows::new(remainder);
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn static_rows(
    view: &crate::presentation::View,
    width: u16,
    layout: super::HistoryLayout,
    theme: &crate::Theme,
) -> Vec<PhysicalRow> {
    let content_width =
        width.saturating_sub(layout.padding.left.saturating_add(layout.padding.right));
    compile_view_with_theme(view, content_width, theme)
        .rows
        .into_iter()
        .map(|row| row.placed(width, layout.padding.left))
        .collect()
}

fn cross_zero_spacing(history: &mut History) {
    if matches!(history.native.top_padding, SpacingTransferState::Semantic)
        && history.layout().padding.top == 0
    {
        history.native.top_padding = SpacingTransferState::Native;
    }
    if history.native.last_native_unit.is_none() {
        return;
    }
    let unit = history.units.front().expect("nonempty History");
    if !matches!(unit.boundary, FlowBoundary::Default) {
        return;
    }
    if history.layout().gap != 0 {
        return;
    }
    match history.native.leading_gap {
        None | Some(SpacingTransferState::Semantic) => {
            history.native.leading_gap = Some(SpacingTransferState::Native)
        }
        Some(SpacingTransferState::Frozen(_)) | Some(SpacingTransferState::Native) => {}
    }
}

fn retire_front(history: &mut History) {
    cross_zero_spacing(history);
    let unit = history
        .units
        .pop_front()
        .expect("retiring nonempty History");
    // A zero-row unit can be retired through the recursive transfer path
    // without any accepted physical row and therefore without a Progress
    // outcome at the outer call. Record that frontier transition explicitly.
    history.bump_native_revision();
    history.native.last_native_unit = Some(unit.id);
    history.native.reset_unit_state();
}
