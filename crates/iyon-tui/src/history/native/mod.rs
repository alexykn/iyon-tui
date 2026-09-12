//! Private native scrollback ownership for generic History.

pub(super) mod frontier;

use crate::{
    backend::NativeHistorySink,
    physical::PhysicalRow,
    presentation::{ContentProvider, EmptyContentProvider, layout::compile_view_with_theme},
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
    SynchronizationUnknown,
}

/// An immutable physical History write captured from one semantic frontier.
/// The rows and acknowledgement metadata remain tied to the exact unit and
/// ContentHost that produced them; a later desired mutation cannot retarget
/// acknowledgement to whichever unit happens to be current.
#[derive(Debug, Clone)]
pub(crate) struct NativeTransferPlan {
    pub(crate) rows: Vec<PhysicalRow>,
    pub(crate) requested: usize,
    operation: NativeTransferOperation,
}

#[derive(Clone, Debug)]
enum NativeTransferOperation {
    Spacing {
        leading_gap: bool,
    },
    Static {
        unit: HistoryUnitId,
    },
    Content {
        unit: HistoryUnitId,
        port_id: u64,
        complete: bool,
        content_start: usize,
        content_end: usize,
        leading_padding: usize,
        trailing_padding: usize,
    },
    FrozenStatic {
        unit: HistoryUnitId,
    },
    FrozenContent {
        unit: HistoryUnitId,
        port_id: u64,
        complete: bool,
        content_start: usize,
        content_end: usize,
        leading_padding: usize,
        trailing_padding: usize,
    },
    Retire {
        unit: HistoryUnitId,
    },
}

impl NativeTransferPlan {
    pub(crate) fn rows(&self) -> &[PhysicalRow] {
        &self.rows[..self.requested]
    }
}

/// Captures the next native History operation without touching the semantic
/// or physical frontier. Callers may therefore release the host acceptance
/// lock, submit `plan.rows()` to the terminal worker, and apply the exact
/// acknowledgement later.
pub(crate) fn prepare_native_transfer_with_theme_and_content(
    history: &History,
    width: u16,
    max_rows: usize,
    theme: &crate::Theme,
    content: &dyn ContentProvider,
) -> Option<NativeTransferPlan> {
    if max_rows == 0 || width == 0 || history.units.is_empty() {
        return None;
    }

    // Eligibility belongs to the accepted History unit.  A direct occurrence
    // may remain active for layout while its physical shape is not the
    // temporary content-only export contract; do not derive a fake transfer
    // View or silently emit a substitute row product.
    if history.native_transfer_blocked_front() {
        return None;
    }

    if let Some(rows) = spacing_rows(
        &history.native.top_padding,
        width,
        usize::from(history.layout().padding.top),
    ) && !rows.is_empty()
    {
        return Some(spacing_plan(rows, max_rows, false));
    }

    if history.native.last_native_unit.is_some()
        && matches!(
            history.units.front().expect("nonempty History").boundary,
            FlowBoundary::Default
        )
    {
        let gap = usize::from(history.layout().gap);
        let state = history
            .native
            .leading_gap
            .as_ref()
            .unwrap_or(&SpacingTransferState::Semantic);
        if let Some(rows) = spacing_rows(state, width, gap)
            && !rows.is_empty()
        {
            return Some(spacing_plan(rows, max_rows, true));
        }
    }

    if let Some(frozen) = history.native.frozen_content.as_ref() {
        return Some(content_plan(
            frozen.rows.as_slice().to_vec(),
            max_rows,
            NativeTransferOperation::FrozenContent {
                unit: frozen.unit,
                port_id: frozen.port_id,
                complete: frozen.complete,
                content_start: frozen.content_start,
                content_end: frozen.content_end,
                leading_padding: frozen.leading_padding,
                trailing_padding: frozen.trailing_padding,
            },
        ));
    }
    if let Some(frozen) = history.native.frozen_static.as_ref() {
        return Some(content_plan(
            frozen.rows.as_slice().to_vec(),
            max_rows,
            NativeTransferOperation::FrozenStatic { unit: frozen.unit },
        ));
    }

    let unit = history.units.front().expect("nonempty History");
    match &unit.content {
        HistoryUnitContent::Live(_) => None,
        HistoryUnitContent::Static(view) => {
            if let Some(transfer) = view.content_history_transfer() {
                let rows = content.history_rows(transfer.port_id, width)?;
                if rows.rows.is_empty() {
                    return rows.complete.then(|| NativeTransferPlan {
                        rows: Vec::new(),
                        requested: 0,
                        operation: NativeTransferOperation::Retire { unit: unit.id },
                    });
                }
                Some(content_plan(
                    rows.rows,
                    max_rows,
                    NativeTransferOperation::Content {
                        unit: unit.id,
                        port_id: transfer.port_id,
                        complete: rows.complete,
                        content_start: rows.content_start,
                        content_end: rows.content_end,
                        leading_padding: rows.leading_padding,
                        trailing_padding: rows.trailing_padding,
                    },
                ))
            } else if view.contains_content_identity() {
                // A composite/nested ContentHost is intentionally blocked;
                // it cannot be represented by the ContentHost product alone.
                None
            } else {
                let rows = static_rows(view, width, history.layout(), theme);
                if rows.is_empty() {
                    Some(NativeTransferPlan {
                        rows: Vec::new(),
                        requested: 0,
                        operation: NativeTransferOperation::Retire { unit: unit.id },
                    })
                } else {
                    Some(content_plan(
                        rows,
                        max_rows,
                        NativeTransferOperation::Static { unit: unit.id },
                    ))
                }
            }
        }
    }
}

fn spacing_plan(rows: Vec<PhysicalRow>, max_rows: usize, leading_gap: bool) -> NativeTransferPlan {
    NativeTransferPlan {
        requested: rows.len().min(max_rows),
        rows,
        operation: NativeTransferOperation::Spacing { leading_gap },
    }
}

fn content_plan(
    rows: Vec<PhysicalRow>,
    max_rows: usize,
    operation: NativeTransferOperation,
) -> NativeTransferPlan {
    NativeTransferPlan {
        requested: rows.len().min(max_rows),
        rows,
        operation,
    }
}

/// Applies an acknowledgement to the exact previously captured operation.
/// This function performs no terminal I/O; it only advances native/content
/// ownership after the sink has confirmed the physical prefix.
pub(crate) fn commit_native_transfer_with_content(
    history: &mut History,
    plan: NativeTransferPlan,
    accepted: usize,
    content: &mut dyn ContentProvider,
) -> Result<NativeTransferOutcome, NativeTransferError<anyhow::Error>> {
    if accepted > plan.requested {
        return Err(NativeTransferError::InvalidAcknowledgement {
            requested: plan.requested,
            accepted,
        });
    }
    if history.native.synchronization_unknown {
        return Err(NativeTransferError::SynchronizationUnknown);
    }

    let requested = plan.requested;
    if matches!(plan.operation, NativeTransferOperation::Retire { .. }) {
        let NativeTransferOperation::Retire { unit } = plan.operation else {
            unreachable!();
        };
        if history.units.front().is_none_or(|front| front.id != unit) {
            return Err(NativeTransferError::InvalidAcknowledgement {
                requested: 0,
                accepted,
            });
        }
        retire_front(history);
        return Ok(finalize_transfer(
            history,
            content,
            outcome(0, 0, NativeTransferStatus::Progress),
        ));
    }
    if accepted == 0 {
        return Ok(outcome(requested, 0, NativeTransferStatus::SinkBlocked));
    }

    let operation = plan.operation;
    match operation {
        NativeTransferOperation::Spacing { leading_gap } => {
            let state = if accepted == plan.rows.len() {
                SpacingTransferState::Native
            } else {
                SpacingTransferState::Frozen(FrozenPhysicalRows::new(
                    plan.rows[accepted..].to_vec(),
                ))
            };
            if leading_gap {
                history.native.leading_gap = Some(state);
            } else {
                history.native.top_padding = state;
            }
        }
        NativeTransferOperation::Static { unit } => {
            verify_front_unit(history, unit)?;
            cross_zero_spacing(history);
            if accepted == plan.rows.len() {
                retire_front(history);
            } else {
                history.native.frozen_static = Some(FrozenStaticRemainder {
                    unit,
                    rows: FrozenPhysicalRows::new(plan.rows[accepted..].to_vec()),
                });
            }
        }
        NativeTransferOperation::Content {
            unit,
            port_id,
            complete,
            content_start,
            content_end,
            leading_padding,
            trailing_padding,
        }
        | NativeTransferOperation::FrozenContent {
            unit,
            port_id,
            complete,
            content_start,
            content_end,
            leading_padding,
            trailing_padding,
        } => {
            verify_front_unit(history, unit)?;
            let accepted_content = accepted
                .saturating_sub(content_start)
                .min(content_end.saturating_sub(content_start));
            let accepted_leading = accepted.min(leading_padding);
            let accepted_trailing = accepted.saturating_sub(content_end).min(trailing_padding);
            content.history_rows_committed(
                port_id,
                accepted,
                accepted_content,
                accepted_leading,
                accepted_trailing,
            );
            cross_zero_spacing(history);
            if accepted == plan.rows.len() {
                history.native.frozen_content = None;
                if complete {
                    retire_front(history);
                }
            } else {
                let next_start = content_start.saturating_sub(accepted);
                let next_end = content_end.saturating_sub(accepted);
                let next_trailing =
                    trailing_padding.saturating_sub(accepted.saturating_sub(content_end));
                history.native.frozen_content = Some(FrozenContentRemainder {
                    unit,
                    port_id,
                    rows: FrozenPhysicalRows::new(plan.rows[accepted..].to_vec()),
                    complete,
                    content_start: next_start,
                    content_end: next_end,
                    leading_padding: leading_padding.saturating_sub(accepted),
                    trailing_padding: next_trailing,
                });
            }
        }
        NativeTransferOperation::FrozenStatic { unit } => {
            verify_front_unit(history, unit)?;
            if accepted == plan.rows.len() {
                retire_front(history);
            } else {
                history.native.frozen_static = Some(FrozenStaticRemainder {
                    unit,
                    rows: FrozenPhysicalRows::new(plan.rows[accepted..].to_vec()),
                });
            }
        }
        NativeTransferOperation::Retire { .. } => unreachable!(),
    }
    Ok(finalize_transfer(
        history,
        content,
        outcome(requested, accepted, NativeTransferStatus::Progress),
    ))
}

fn finalize_transfer(
    history: &mut History,
    content: &mut dyn ContentProvider,
    mut outcome: NativeTransferOutcome,
) -> NativeTransferOutcome {
    let mut retired_units = std::mem::take(&mut history.native.retired_units);
    let retired = !retired_units.is_empty();
    for unit_id in retired_units.drain(..) {
        content.history_unit_retired(unit_id.value());
    }
    history.native.retired_units = retired_units;
    if retired && outcome.inserted == 0 && !matches!(outcome.status, NativeTransferStatus::Progress)
    {
        outcome.status = NativeTransferStatus::Progress;
    }
    history.native.record_physical_rows(outcome.inserted);
    if outcome.inserted > 0 || retired || matches!(outcome.status, NativeTransferStatus::Progress) {
        history.bump_native_revision();
    }
    outcome
}

fn verify_front_unit(
    history: &History,
    unit: HistoryUnitId,
) -> Result<(), NativeTransferError<anyhow::Error>> {
    if history.units.front().is_none_or(|front| front.id != unit) {
        return Err(NativeTransferError::InvalidAcknowledgement {
            requested: 0,
            accepted: 0,
        });
    }
    Ok(())
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
    if history.native.synchronization_unknown {
        return Err(NativeTransferError::SynchronizationUnknown);
    }
    let result = transfer_native_prefix_inner(history, sink, width, max_rows, theme, content);
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(
            error @ (NativeTransferError::Sink(_)
            | NativeTransferError::InvalidAcknowledgement { .. }),
        ) => {
            // A sink may have performed a partial physical write before
            // returning its error. Do not rewind accepted logical rows or
            // claim that the previous physical screen remains intact.
            history.native.mark_synchronization_unknown();
            return Err(error);
        }
        Err(error @ NativeTransferError::SynchronizationUnknown) => return Err(error),
    };
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
    let Some(plan) =
        prepare_native_transfer_with_theme_and_content(history, width, max_rows, theme, content)
    else {
        let status = history
            .units
            .front()
            .map(|unit| match &unit.content {
                _ if history.native.blocked_units.contains(&unit.id) => {
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit.id,
                        reason: NativeBlockReason::ContentHost,
                    }
                }
                HistoryUnitContent::Live(_) => NativeTransferStatus::SemanticBlocked {
                    unit: unit.id,
                    reason: NativeBlockReason::Live,
                },
                HistoryUnitContent::Static(view) if view.contains_content_identity() => {
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit.id,
                        reason: NativeBlockReason::ContentHost,
                    }
                }
                HistoryUnitContent::Static(_) => NativeTransferStatus::Idle,
            })
            .unwrap_or(NativeTransferStatus::Idle);
        return Ok(outcome(0, 0, status));
    };
    if plan.requested == 0 {
        return map_plan_result(commit_native_transfer_with_content(
            history, plan, 0, content,
        ));
    }
    let requested = plan.requested;
    let accepted = sink
        .insert_history_rows(plan.rows())
        .map_err(NativeTransferError::Sink)?;
    if accepted > requested {
        return Err(NativeTransferError::InvalidAcknowledgement {
            requested,
            accepted,
        });
    }
    map_plan_result(commit_native_transfer_with_content(
        history, plan, accepted, content,
    ))
}

fn map_plan_result<E>(
    result: Result<NativeTransferOutcome, NativeTransferError<anyhow::Error>>,
) -> Result<NativeTransferOutcome, NativeTransferError<E>> {
    result.map_err(|error| match error {
        NativeTransferError::InvalidAcknowledgement {
            requested,
            accepted,
        } => NativeTransferError::InvalidAcknowledgement {
            requested,
            accepted,
        },
        NativeTransferError::SynchronizationUnknown => NativeTransferError::SynchronizationUnknown,
        NativeTransferError::Sink(error) => {
            unreachable!("physical transfer plan cannot produce sink error: {error}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_history_frontier_does_not_prepare_a_replacement_plan() {
        let mut history = History::new();
        let unit = history
            .push(crate::presentation::factory::text("already-owned"))
            .expect("History unit");
        history.set_native_transfer_blocked(unit, true);
        assert!(
            prepare_native_transfer_with_theme_and_content(
                &history,
                20,
                4,
                &crate::Theme::new(),
                &EmptyContentProvider,
            )
            .is_none()
        );
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
    history.native.retired_units.push(unit.id);
    // The outer transfer adapter records one native revision for the whole
    // successful receipt. A zero-row retirement is carried through
    // `retired_units` so it still invalidates the History branch without
    // double-bumping a transfer that also accepted physical rows.
    history.native.last_native_unit = Some(unit.id);
    history.native.reset_unit_state();
}
