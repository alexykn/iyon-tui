//! Ordered semantic History unit representation.

use std::cell::RefCell;

use crate::{
    component::{ComponentId, ComponentRevision},
    presentation::{View, ir::ViewId},
    stream::{StreamOffset, StreamRevision},
};

use super::{ErasedHistoryStream, FlowBoundary, HistoryUnitId};

/// Dependency key for one unit's cached presentation height.
///
/// A cached height is valid exactly when the unit's content identity and all
/// geometry-affecting dependencies match this key at the same content width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HistoryUnitLayoutKey {
    /// Component-free unit: only the semantic identity and the width matter.
    Static(ViewId),
    /// Component-bearing unit: also every reachable component revision.
    Live {
        view: ViewId,
        dependencies: Vec<(ComponentId, ComponentRevision)>,
    },
    /// Streaming unit: semantic revision and source coordinates for its row index.
    Stream {
        revision: StreamRevision,
        base: StreamOffset,
        source_end: StreamOffset,
        indexed_from: StreamOffset,
        prefix_rows: usize,
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
    Stream(ErasedHistoryStream),
}
