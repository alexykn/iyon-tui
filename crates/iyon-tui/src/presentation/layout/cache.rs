//! Bounded retained measurement and preparation state.
//!
//! Measurement and preparation are intentionally separate generations. The
//! layout tree is still rebuilt every pass because placement depends on its
//! parent origin and clip; only width-dependent semantic facts and
//! height-dependent allocation are retained here.

use std::{collections::HashMap, sync::Arc};

use crate::presentation::ir::ViewId;

use super::{measure::MeasuredNode, prepare::PreparedNode};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct MeasureKey {
    pub(super) view: ViewId,
    pub(super) component_view: Option<ViewId>,
    pub(super) width: u16,
    pub(super) intent: WidthIntent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum WidthIntent {
    Semantic,
    ForceFit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct PrepareKey {
    pub(super) measured: MeasureKey,
    pub(super) height_bound: Option<u16>,
}

/// Two-generation retained layout cache.
///
/// Entries are promoted from the previous generation into the current one on
/// use. Rotation happens once per host frame, never between convergence passes,
/// so all passes needed to reach a stable frame share the same working set.
#[derive(Debug, Default)]
pub(crate) struct LayoutCache {
    current_measure: HashMap<MeasureKey, Arc<MeasuredNode>>,
    previous_measure: HashMap<MeasureKey, Arc<MeasuredNode>>,
    current_prepare: HashMap<PrepareKey, Arc<PreparedNode>>,
    previous_prepare: HashMap<PrepareKey, Arc<PreparedNode>>,
}

impl LayoutCache {
    pub(crate) fn begin_epoch(&mut self) {
        std::mem::swap(&mut self.current_measure, &mut self.previous_measure);
        self.current_measure.clear();
        std::mem::swap(&mut self.current_prepare, &mut self.previous_prepare);
        self.current_prepare.clear();
    }

    pub(super) fn measured(&mut self, key: MeasureKey) -> Option<Arc<MeasuredNode>> {
        if let Some(value) = self.current_measure.get(&key) {
            return Some(Arc::clone(value));
        }
        let value = self.previous_measure.remove(&key)?;
        self.current_measure.insert(key, Arc::clone(&value));
        Some(value)
    }

    pub(super) fn store_measured(&mut self, key: MeasureKey, value: Arc<MeasuredNode>) {
        self.current_measure.insert(key, value);
    }

    pub(super) fn prepared(&mut self, key: PrepareKey) -> Option<Arc<PreparedNode>> {
        if let Some(value) = self.current_prepare.get(&key) {
            return Some(Arc::clone(value));
        }
        let value = self.previous_prepare.remove(&key)?;
        self.current_prepare.insert(key, Arc::clone(&value));
        Some(value)
    }

    pub(super) fn store_prepared(&mut self, key: PrepareKey, value: Arc<PreparedNode>) {
        self.current_prepare.insert(key, value);
    }

    #[cfg(test)]
    pub(super) fn retained_entries(&self) -> usize {
        self.current_measure.len()
            + self.previous_measure.len()
            + self.current_prepare.len()
            + self.previous_prepare.len()
    }

    #[cfg(test)]
    pub(super) fn contains_view_id(&self, view: ViewId) -> bool {
        self.current_measure
            .keys()
            .chain(self.previous_measure.keys())
            .any(|key| key.view == view)
            || self
                .current_prepare
                .keys()
                .chain(self.previous_prepare.keys())
                .any(|key| key.measured.view == view)
    }
}

impl MeasureKey {
    pub(super) fn ordinary(
        view: &crate::presentation::View,
        width: u16,
        intent: WidthIntent,
    ) -> Self {
        Self {
            view: view.id(),
            component_view: None,
            width,
            intent,
        }
    }

    pub(super) fn with_component(
        view: &crate::presentation::View,
        component_view: ViewId,
        width: u16,
        intent: WidthIntent,
    ) -> Self {
        Self {
            view: view.id(),
            component_view: Some(component_view),
            width,
            intent,
        }
    }
}
