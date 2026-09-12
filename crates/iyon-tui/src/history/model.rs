//! Physical History ownership and native-export frontier.
//!
//! History no longer stores semantic Views or resolves a layout tree. The
//! occurrence document owns resident UI roots; this value only tracks the
//! typed physical export units and their receipt-safe native frontier.

use std::{cell::Cell, collections::VecDeque, sync::atomic::AtomicU64};

use crate::{id::next_nonzero_id, physical::PhysicalRow};

use super::{FlowBoundary, HistoryError, HistoryUnitId, native::NativeFrontier};

static NEXT_HISTORY_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryLayout {
    pub(crate) padding: crate::Insets,
    pub(crate) gap: u16,
}

impl HistoryLayout {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            padding: crate::Insets::ZERO,
            gap: 0,
        }
    }
    #[must_use]
    pub const fn from_parts(padding: crate::Insets, gap: u16) -> Self {
        Self { padding, gap }
    }
    #[must_use]
    pub const fn with_padding(mut self, padding: crate::Insets) -> Self {
        self.padding = padding;
        self
    }
    #[must_use]
    pub const fn with_gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }
    #[must_use]
    pub const fn padding(self) -> crate::Insets {
        self.padding
    }
    #[must_use]
    pub const fn gap(self) -> u16 {
        self.gap
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HistoryUnitContent {
    Content {
        port_id: u64,
        padding: crate::Insets,
    },
    StaticRows(Vec<PhysicalRow>),
    Blocked,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HistoryUnit {
    pub(crate) id: HistoryUnitId,
    pub(crate) boundary: FlowBoundary,
    pub(crate) content: HistoryUnitContent,
    pub(crate) live: bool,
}

/// Native physical export state. Resident semantic content remains owned by
/// occurrence/content registries until a confirmed native receipt retires it.
pub struct History {
    pub(super) units: VecDeque<HistoryUnit>,
    identity: u64,
    layout: HistoryLayout,
    revision: Cell<u64>,
    native_revision: Cell<u64>,
    pub(super) native: NativeFrontier,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self {
            units: VecDeque::new(),
            identity: next_nonzero_id(&NEXT_HISTORY_ID, "history identity exhausted").get(),
            layout: HistoryLayout::new(),
            revision: Cell::new(0),
            native_revision: Cell::new(0),
            native: NativeFrontier::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub(crate) fn contains_unit(&self, id: HistoryUnitId) -> bool {
        self.units.iter().any(|unit| unit.id == id)
    }
    pub(crate) fn native_has_physical_rows(&self) -> bool {
        self.native.has_physical_rows()
    }
    pub(crate) fn set_native_transfer_blocked(&mut self, id: HistoryUnitId, blocked: bool) {
        self.native.set_transfer_blocked(id, blocked);
    }
    pub(crate) fn native_transfer_blocked_front(&self) -> bool {
        self.units.front().is_some_and(|unit| {
            self.native.blocked_units.contains(&unit.id)
                || matches!(unit.content, HistoryUnitContent::Blocked)
        })
    }
    pub(crate) fn native_transfer_semantically_blocked_front(&self) -> bool {
        self.native_transfer_blocked_front() || self.units.front().is_some_and(|unit| unit.live)
    }
    pub(crate) fn unit_is_live(&self, id: HistoryUnitId) -> Option<bool> {
        self.units
            .iter()
            .find_map(|unit| (unit.id == id).then_some(unit.live))
    }
    pub(crate) fn unit_ids(&self) -> impl Iterator<Item = HistoryUnitId> + '_ {
        self.units.iter().map(|unit| unit.id)
    }

    pub(crate) fn push_content_with_identity(
        &mut self,
        id: HistoryUnitId,
        port_id: u64,
        padding: crate::Insets,
        boundary: FlowBoundary,
        live: bool,
        blocked: bool,
    ) -> Result<(), HistoryError> {
        if self.contains_unit(id) {
            return Err(HistoryError::DuplicateUnit { unit: id });
        }
        self.units.push_back(HistoryUnit {
            id,
            boundary,
            content: if blocked {
                HistoryUnitContent::Blocked
            } else {
                HistoryUnitContent::Content { port_id, padding }
            },
            live,
        });
        self.bump_revision();
        Ok(())
    }

    pub(crate) fn replace_content(
        &mut self,
        id: HistoryUnitId,
        port_id: u64,
        padding: crate::Insets,
        blocked: bool,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(id)?;
        if !self.units[index].live {
            return Err(HistoryError::UnitNotLive { unit: id });
        }
        self.units[index].content = if blocked {
            HistoryUnitContent::Blocked
        } else {
            HistoryUnitContent::Content { port_id, padding }
        };
        self.bump_revision();
        Ok(())
    }

    pub(crate) fn freeze_content(
        &mut self,
        id: HistoryUnitId,
        port_id: u64,
        padding: crate::Insets,
        blocked: bool,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(id)?;
        if !self.units[index].live {
            return Err(HistoryError::UnitNotLive { unit: id });
        }
        self.units[index].live = false;
        self.units[index].content = if blocked {
            HistoryUnitContent::Blocked
        } else {
            HistoryUnitContent::Content { port_id, padding }
        };
        self.bump_revision();
        Ok(())
    }

    pub(crate) fn retire_unit(&mut self, id: HistoryUnitId) -> Result<(), HistoryError> {
        let index = self.index_of(id)?;
        self.units.remove(index);
        self.native.blocked_units.remove(&id);
        self.bump_revision();
        Ok(())
    }

    pub(crate) fn front_content_attachment_id(&self) -> Option<u64> {
        match self.units.front().map(|unit| &unit.content) {
            Some(HistoryUnitContent::Content { port_id, .. }) => Some(*port_id),
            _ => None,
        }
    }

    pub fn layout(&self) -> HistoryLayout {
        self.layout
    }
    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }
    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub(crate) fn native_revision(&self) -> u64 {
        self.native_revision.get()
    }
    pub(crate) fn bump_native_revision(&self) {
        self.native_revision
            .set(self.native_revision.get().wrapping_add(1));
    }
    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }
    pub(crate) fn physical_rows_inserted(&self) -> u64 {
        self.native.physical_rows_inserted
    }
    pub(crate) fn native_synchronization_unknown(&self) -> bool {
        self.native.synchronization_unknown
    }
    pub(crate) fn mark_native_synchronization_unknown(&mut self) {
        self.native.mark_synchronization_unknown();
    }
    pub(crate) fn recover_native_synchronization(&mut self) {
        self.native.recover_synchronization();
    }
    pub fn set_layout(&mut self, layout: HistoryLayout) {
        if self.layout != layout {
            self.layout = layout;
            self.bump_revision();
        }
    }
    #[must_use]
    pub fn with_layout(mut self, layout: HistoryLayout) -> Self {
        self.set_layout(layout);
        self
    }

    fn index_of(&self, id: HistoryUnitId) -> Result<usize, HistoryError> {
        self.units
            .iter()
            .position(|unit| unit.id == id)
            .ok_or(HistoryError::UnitNotFound { unit: id })
    }
}
