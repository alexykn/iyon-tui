//! Private monotonic native-history ownership for generic History.

pub(super) mod frontier;

#[cfg(test)]
mod tests;

use crate::{
    backend::NativeHistorySink,
    physical::PhysicalRow,
    presentation::layout::compile_view_with_theme,
    stream::{
        CompiledStream, FrozenPhysicalRows, StreamPartialTransfer, StreamTransferPayload,
        plan_stream_transfer,
    },
};

use super::{FlowBoundary, History, HistoryUnitContent, HistoryUnitId};
pub(super) use frontier::NativeFrontier;
use frontier::{FrozenStaticRemainder, SpacingTransferState, StreamFrontierState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeBlockReason {
    Live,
    StreamBlocked,
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
    let outcome = transfer_native_prefix_inner(history, sink, width, max_rows, theme)?;
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
        HistoryUnitContent::Static(view) if view.contains_content_identity() => Ok(outcome(
            0,
            0,
            NativeTransferStatus::SemanticBlocked {
                unit: unit_id,
                reason: NativeBlockReason::ContentHost,
            },
        )),
        HistoryUnitContent::Static(view) => {
            let rows = static_rows(view, width, history.layout(), theme);
            if rows.is_empty() {
                retire_front(history);
                return transfer_native_prefix_inner(history, sink, width, max_rows, theme);
            }
            transfer_static(history, sink, rows, max_rows)
        }
        HistoryUnitContent::Stream(_) => transfer_stream(history, sink, width, max_rows, theme),
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

fn transfer_stream<S: NativeHistorySink>(
    history: &mut History,
    sink: &mut S,
    width: u16,
    max_rows: usize,
    theme: &crate::Theme,
) -> Result<NativeTransferOutcome, NativeTransferError<S::Error>> {
    let unit = history.units.front().expect("stream unit");
    let unit_id = unit.id;
    let (starting_cursor, starting_partial) = history
        .native
        .stream
        .as_ref()
        .map_or((stream_semantic_base(history, unit_id), None), |state| {
            (state.committed_through, state.partial.clone())
        });
    let start = match starting_partial.as_ref() {
        Some(StreamPartialTransfer::FrozenAtomic { source_end, .. }) => *source_end,
        None => starting_cursor,
    };
    let (mut compiled, sealed, source_end) = {
        let stream = match &unit.content {
            HistoryUnitContent::Stream(stream) => stream,
            _ => unreachable!("stream frontier must match stream unit"),
        };
        (
            stream.compile_from(
                start,
                width.saturating_sub(
                    history
                        .layout()
                        .padding
                        .left
                        .saturating_add(history.layout().padding.right),
                ),
                theme,
            ),
            stream.is_sealed(),
            stream.source_end(),
        )
    };
    place_stream_rows(&mut compiled, width, history.layout());
    if starting_partial.is_none()
        && compiled
            .zero_row_prefix
            .is_some_and(|offset| offset > starting_cursor)
    {
        let next = compiled.zero_row_prefix.expect("checked zero-row prefix");
        cross_zero_spacing(history);
        let HistoryUnitContent::Stream(stream) = &mut history.units.front_mut().unwrap().content
        else {
            unreachable!("stream frontier must match stream unit")
        };
        stream.release_resident_through(next);
        let state = history.native.stream.get_or_insert(StreamFrontierState {
            unit: unit_id,
            committed_through: starting_cursor,
            partial: None,
        });
        state.committed_through = next;
        // The recursive call may ultimately report SemanticBlocked with zero
        // physical rows, so the outer transfer status cannot carry this
        // semantic frontier transition on its own.
        history.bump_native_revision();
        return transfer_native_prefix_inner(history, sink, width, max_rows, theme);
    }
    let plan = plan_stream_transfer(
        &compiled,
        max_rows,
        starting_partial.as_ref(),
        starting_cursor,
    );
    let rows = payload_rows(&compiled, &plan.payload);
    if rows.is_empty() {
        if sealed && starting_partial.is_none() && starting_cursor >= source_end {
            retire_front(history);
            return Ok(outcome(0, 0, NativeTransferStatus::Progress));
        }
        return Ok(outcome(
            0,
            0,
            NativeTransferStatus::SemanticBlocked {
                unit: unit_id,
                reason: NativeBlockReason::StreamBlocked,
            },
        ));
    }

    let ack = insert_prefix(sink, &rows, max_rows)?;
    if ack.accepted == 0 {
        return Ok(outcome(ack.requested, 0, NativeTransferStatus::SinkBlocked));
    }

    let accepted_plan = plan_stream_transfer(
        &compiled,
        ack.accepted,
        starting_partial.as_ref(),
        starting_cursor,
    );
    let new_cursor = accepted_plan.next_committed_through;
    let new_partial = accepted_plan.next_partial;
    if new_cursor > starting_cursor {
        let HistoryUnitContent::Stream(stream) = &mut history.units.front_mut().unwrap().content
        else {
            unreachable!("stream frontier must match stream unit")
        };
        stream.release_resident_through(new_cursor);
    }
    cross_zero_spacing(history);
    let state = history.native.stream.get_or_insert(StreamFrontierState {
        unit: unit_id,
        committed_through: starting_cursor,
        partial: starting_partial,
    });
    state.committed_through = new_cursor;
    state.partial = new_partial;
    if sealed && state.partial.is_none() && new_cursor >= source_end {
        retire_front(history);
    }
    Ok(outcome(
        ack.requested,
        ack.accepted,
        NativeTransferStatus::Progress,
    ))
}

fn payload_rows(compiled: &CompiledStream, payload: &StreamTransferPayload) -> Vec<PhysicalRow> {
    match payload {
        StreamTransferPayload::Compiled { start, len } => compiled.rows
            [*start..start.saturating_add(*len)]
            .iter()
            .map(|row| row.physical.clone())
            .collect(),
        StreamTransferPayload::Frozen { rows } => rows.as_slice().to_vec(),
    }
}

fn place_stream_rows(compiled: &mut CompiledStream, width: u16, layout: super::HistoryLayout) {
    for row in &mut compiled.rows {
        row.physical = row.physical.placed(width, layout.padding.left);
    }
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

fn stream_semantic_base(history: &History, unit: HistoryUnitId) -> crate::stream::StreamOffset {
    match &history.units.front().expect("stream unit").content {
        HistoryUnitContent::Stream(stream) if history.units.front().unwrap().id == unit => {
            stream.semantic_base()
        }
        _ => unreachable!("stream frontier must match stream unit"),
    }
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
