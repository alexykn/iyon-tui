//! Physical History transfer and receipt ownership.
//!
//! The native frontier is independent from occurrence layout. It captures
//! exact rows, submits them outside the host lock, and advances only after the
//! matching sink receipt is installed.

pub(super) mod frontier;

use crate::{
    backend::NativeHistorySink,
    physical::PhysicalRow,
    presentation::{ContentProvider, EmptyContentProvider},
};

use super::model::HistoryUnit;
use super::{History, HistoryUnitContent, HistoryUnitId};
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

pub(crate) fn prepare_native_transfer_with_theme_and_content(
    history: &History,
    width: u16,
    max_rows: usize,
    _theme: &crate::Theme,
    content: &dyn ContentProvider,
) -> Option<NativeTransferPlan> {
    if max_rows == 0
        || width == 0
        || history.units.is_empty()
        || history.native_transfer_blocked_front()
    {
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
        && history
            .units
            .front()
            .is_some_and(|unit| unit.boundary == super::FlowBoundary::Default)
    {
        let state = history
            .native
            .leading_gap
            .as_ref()
            .unwrap_or(&SpacingTransferState::Semantic);
        if let Some(rows) = spacing_rows(state, width, usize::from(history.layout().gap))
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
        HistoryUnitContent::Blocked => None,
        HistoryUnitContent::StaticRows(rows) => {
            if rows.is_empty() {
                Some(NativeTransferPlan {
                    rows: Vec::new(),
                    requested: 0,
                    operation: NativeTransferOperation::Retire { unit: unit.id },
                })
            } else {
                Some(content_plan(
                    rows.clone(),
                    max_rows,
                    NativeTransferOperation::Static { unit: unit.id },
                ))
            }
        }
        HistoryUnitContent::Content { port_id, .. } => {
            let rows = content.history_rows(*port_id, width)?;
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
                    port_id: *port_id,
                    complete: rows.complete,
                    content_start: rows.content_start,
                    content_end: rows.content_end,
                    leading_padding: rows.leading_padding,
                    trailing_padding: rows.trailing_padding,
                },
            ))
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
    if let NativeTransferOperation::Retire { unit } = plan.operation {
        verify_front_unit(history, unit)?;
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
    match plan.operation {
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
        NativeTransferOperation::Static { unit }
        | NativeTransferOperation::FrozenStatic { unit } => {
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
                let consumed_trailing = accepted.saturating_sub(content_end);
                history.native.frozen_content = Some(FrozenContentRemainder {
                    unit,
                    port_id,
                    rows: FrozenPhysicalRows::new(plan.rows[accepted..].to_vec()),
                    complete,
                    content_start: content_start.saturating_sub(accepted),
                    content_end: content_end.saturating_sub(accepted),
                    leading_padding: leading_padding.saturating_sub(accepted),
                    trailing_padding: trailing_padding.saturating_sub(consumed_trailing),
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
    mut result: NativeTransferOutcome,
) -> NativeTransferOutcome {
    let retired = std::mem::take(&mut history.native.retired_units);
    let had_retirements = !retired.is_empty();
    for id in retired {
        content.history_unit_retired(id.value());
    }
    if had_retirements
        && result.inserted == 0
        && !matches!(result.status, NativeTransferStatus::Progress)
    {
        result.status = NativeTransferStatus::Progress;
    }
    history.native.record_physical_rows(result.inserted);
    if result.inserted > 0
        || had_retirements
        || matches!(result.status, NativeTransferStatus::Progress)
    {
        history.bump_native_revision();
    }
    result
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
    match result {
        Err(
            error @ (NativeTransferError::Sink(_)
            | NativeTransferError::InvalidAcknowledgement { .. }),
        ) => {
            // A backend error may follow an unknown amount of physical output.
            // Keep the confirmed logical prefix and disable automatic replay
            // until the owner performs explicit physical recovery.
            history.mark_native_synchronization_unknown();
            Err(error)
        }
        result => result,
    }
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
            .map(|unit| {
                if history.native.blocked_units.contains(&unit.id)
                    || matches!(unit.content, HistoryUnitContent::Blocked)
                {
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit.id,
                        reason: NativeBlockReason::ContentHost,
                    }
                } else if unit.live {
                    NativeTransferStatus::SemanticBlocked {
                        unit: unit.id,
                        reason: NativeBlockReason::Live,
                    }
                } else {
                    NativeTransferStatus::Idle
                }
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
        NativeTransferError::Sink(_) => {
            unreachable!("physical transfer plan cannot produce sink error")
        }
    })
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

fn cross_zero_spacing(history: &mut History) {
    if matches!(history.native.top_padding, SpacingTransferState::Semantic)
        && history.layout().padding.top() == 0
    {
        history.native.top_padding = SpacingTransferState::Native;
    }
    if history.native.last_native_unit.is_none()
        || history
            .units
            .front()
            .is_none_or(|unit| unit.boundary != super::FlowBoundary::Default)
        || history.layout().gap() != 0
    {
        return;
    }
    if matches!(
        history.native.leading_gap,
        None | Some(SpacingTransferState::Semantic)
    ) {
        history.native.leading_gap = Some(SpacingTransferState::Native);
    }
}

fn retire_front(history: &mut History) {
    cross_zero_spacing(history);
    let unit = history
        .units
        .pop_front()
        .expect("retiring nonempty History");
    history.native.retired_units.push(unit.id);
    history.native.last_native_unit = Some(unit.id);
    history.native.reset_unit_state();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BudgetSink {
        budgets: Vec<Result<usize, &'static str>>,
        rows: Vec<PhysicalRow>,
    }

    impl NativeHistorySink for BudgetSink {
        type Error = &'static str;

        fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
            let accepted = match self.budgets.remove(0) {
                Ok(accepted) => accepted,
                Err(error) => return Err(error),
            };
            assert!(accepted <= rows.len());
            self.rows.extend(rows[..accepted].iter().cloned());
            Ok(accepted)
        }
    }

    fn row(text: &str) -> PhysicalRow {
        PhysicalRow::from_cells(vec![crate::physical::PhysicalCell {
            grapheme: Some(text.to_owned()),
            style: crate::physical::PhysicalStyle::default(),
            painted: true,
            continuation: false,
        }])
    }

    fn static_history() -> History {
        let mut history = History::new();
        history.units.push_back(HistoryUnit {
            id: HistoryUnitId::allocate(),
            boundary: super::super::FlowBoundary::Default,
            content: HistoryUnitContent::StaticRows(vec![row("a"), row("b"), row("c")]),
            live: false,
        });
        history
    }

    #[test]
    fn blocked_history_frontier_does_not_prepare_a_replacement_plan() {
        let mut history = History::new();
        let unit = HistoryUnitId::allocate();
        history
            .push_content_with_identity(
                unit,
                1,
                crate::Insets::ZERO,
                super::super::FlowBoundary::Default,
                false,
                true,
            )
            .expect("History unit");
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

    #[test]
    fn live_history_frontier_remains_semantically_blocked() {
        let mut history = History::new();
        history
            .push_content_with_identity(
                HistoryUnitId::allocate(),
                1,
                crate::Insets::ZERO,
                super::super::FlowBoundary::Default,
                true,
                false,
            )
            .expect("live History unit");
        assert!(history.native_transfer_semantically_blocked_front());
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

    #[test]
    fn partial_and_zero_receipts_resume_from_the_confirmed_prefix() {
        let mut history = static_history();
        let mut sink = BudgetSink {
            budgets: vec![Ok(1), Ok(0), Ok(2)],
            rows: Vec::new(),
        };

        let first =
            transfer_native_prefix(&mut history, &mut sink, 20, 8).expect("first native receipt");
        assert_eq!(first.inserted, 1);
        let zero =
            transfer_native_prefix(&mut history, &mut sink, 20, 8).expect("zero native receipt");
        assert_eq!(zero.inserted, 0);
        assert!(matches!(zero.status, NativeTransferStatus::SinkBlocked));
        let remainder = transfer_native_prefix(&mut history, &mut sink, 20, 8)
            .expect("remainder native receipt");
        assert_eq!(remainder.inserted, 2);
        assert_eq!(
            sink.rows
                .iter()
                .map(PhysicalRow::plain_text)
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert!(history.is_empty());
    }

    #[test]
    fn failed_native_receipt_disables_suffix_replay() {
        let mut history = static_history();
        let mut sink = BudgetSink {
            budgets: vec![Err("simulated physical failure")],
            rows: Vec::new(),
        };
        let result = transfer_native_prefix(&mut history, &mut sink, 20, 8);
        assert!(matches!(result, Err(NativeTransferError::Sink(_))));
        assert!(history.native_synchronization_unknown());
        let result = transfer_native_prefix(&mut history, &mut sink, 20, 8);
        assert!(matches!(
            result,
            Err(NativeTransferError::SynchronizationUnknown)
        ));
        assert!(sink.rows.is_empty());
    }
}
