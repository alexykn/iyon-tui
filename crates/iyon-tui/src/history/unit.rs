//! Ordered semantic History unit representation.

use std::cell::RefCell;

use crate::{
    component::{ComponentId, ComponentRevision},
    presentation::{View, ir::ViewId},
};

use super::{FlowBoundary, HistoryUnitId};

/// Dependency key for one unit's cached presentation height.
///
/// A cached height is valid exactly when the unit's content identity and all
/// geometry-affecting dependencies match this key at the same content width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HistoryUnitLayoutKey {
    /// Component-free unit: only the semantic identity and the width matter.
    Static(ViewId),
    /// A static semantic unit containing a retained `ContentPort`. The provider
    /// revision participates so Source/Funnel/delivery changes remeasure the
    /// History flow instead of reusing the empty pre-mutation height.
    Content { view: ViewId, projection: u64 },
    /// Component-bearing unit: also every reachable component revision.
    Live {
        view: ViewId,
        dependencies: Vec<(ComponentId, ComponentRevision)>,
    },
}

#[derive(Debug, Default)]
pub(super) struct HistoryUnitLayout {
    pub(super) width: Option<u16>,
    pub(super) key: Option<HistoryUnitLayoutKey>,
    pub(super) height: Option<usize>,
}

pub(crate) struct HistoryUnit {
    pub(super) id: HistoryUnitId,
    pub(super) boundary: FlowBoundary,
    pub(super) content: HistoryUnitContent,
    pub(super) layout: RefCell<HistoryUnitLayout>,
}

pub(crate) enum HistoryUnitContent {
    Static(View),
    Live(View),
}
