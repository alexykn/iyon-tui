use std::collections::HashSet;

use crate::physical::PhysicalRow;

use super::super::HistoryUnitId;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrozenPhysicalRows(pub(crate) Vec<PhysicalRow>);

impl FrozenPhysicalRows {
    pub(crate) fn new(rows: Vec<PhysicalRow>) -> Self {
        debug_assert!(rows.iter().all(|row| row.validate_cell_geometry().is_ok()));
        Self(rows)
    }

    pub(crate) fn as_slice(&self) -> &[PhysicalRow] {
        &self.0
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SpacingTransferState {
    Semantic,
    Frozen(FrozenPhysicalRows),
    Native,
}

impl Default for SpacingTransferState {
    fn default() -> Self {
        Self::Semantic
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrozenStaticRemainder {
    pub(crate) unit: HistoryUnitId,
    pub(crate) rows: FrozenPhysicalRows,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrozenContentRemainder {
    pub(crate) unit: HistoryUnitId,
    pub(crate) port_id: u64,
    pub(crate) rows: FrozenPhysicalRows,
    pub(crate) complete: bool,
    pub(crate) content_start: usize,
    pub(crate) content_end: usize,
    pub(crate) leading_padding: usize,
    pub(crate) trailing_padding: usize,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct NativeFrontier {
    pub(crate) physical_rows_inserted: u64,
    /// Set when a native sink reports an error after a write may have begun.
    /// Logical frontiers are never rewound; the host must perform its normal
    /// synchronization recovery before attempting another transfer.
    pub(crate) synchronization_unknown: bool,
    pub(crate) last_native_unit: Option<HistoryUnitId>,
    pub(crate) top_padding: SpacingTransferState,
    pub(crate) leading_gap: Option<SpacingTransferState>,
    pub(crate) frozen_static: Option<FrozenStaticRemainder>,
    pub(crate) frozen_content: Option<FrozenContentRemainder>,
    /// Retirements recorded by one transfer attempt. The outer adapter drains
    /// this list even when a later sink operation fails, so content ownership
    /// cannot leak after an irreversible semantic retirement.
    pub(crate) retired_units: Vec<HistoryUnitId>,
    /// Native export eligibility owned by the History frontier.  A direct
    /// occurrence may remain mounted while its shape is intentionally not a
    /// content-only physical export; that frontier must block transfer rather
    /// than manufacture a replacement View or rows.
    pub(crate) blocked_units: HashSet<HistoryUnitId>,
}

impl NativeFrontier {
    pub(crate) fn mark_synchronization_unknown(&mut self) {
        self.synchronization_unknown = true;
    }

    pub(crate) fn recover_synchronization(&mut self) {
        self.synchronization_unknown = false;
    }

    pub(crate) fn has_physical_rows(&self) -> bool {
        self.physical_rows_inserted != 0
    }

    pub(crate) fn record_physical_rows(&mut self, count: usize) {
        self.physical_rows_inserted = self.physical_rows_inserted.saturating_add(count as u64);
    }

    pub(super) fn reset_unit_state(&mut self) {
        self.leading_gap = None;
        self.frozen_static = None;
        self.frozen_content = None;
    }

    pub(crate) fn set_transfer_blocked(&mut self, unit: HistoryUnitId, blocked: bool) {
        if blocked {
            self.blocked_units.insert(unit);
        } else {
            self.blocked_units.remove(&unit);
        }
    }

    pub(super) fn blank_rows(width: u16, count: usize) -> Vec<PhysicalRow> {
        (0..count)
            .map(|_| {
                PhysicalRow::from_cells(vec![
                    crate::physical::PhysicalCell::transparent();
                    usize::from(width)
                ])
            })
            .collect()
    }
}
