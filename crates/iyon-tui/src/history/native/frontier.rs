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
    pub(crate) last_native_unit: Option<HistoryUnitId>,
    pub(crate) top_padding: SpacingTransferState,
    pub(crate) leading_gap: Option<SpacingTransferState>,
    pub(crate) frozen_static: Option<FrozenStaticRemainder>,
    pub(crate) frozen_content: Option<FrozenContentRemainder>,
}

impl NativeFrontier {
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
