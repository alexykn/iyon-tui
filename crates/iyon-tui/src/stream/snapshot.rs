//! Width-independent stream snapshots and public construction.

use super::{
    StreamOffset, StreamRange, StreamRevision, StreamValidationError,
    node::{StreamNode, StreamView},
    projected::ProjectedText,
};
use crate::{TextSpan, View};

/// Width-independent snapshot of the current source-owned semantic stream.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamSnapshot {
    pub(crate) revision: StreamRevision,
    /// Earliest source represented by this snapshot; source retention, not consumer ownership.
    pub(crate) source_base: StreamOffset,
    pub(crate) source_end: StreamOffset,
    pub(crate) stable_through: StreamOffset,
    pub(crate) view: StreamView,
}

impl StreamSnapshot {
    pub fn builder(revision: StreamRevision, range: StreamRange) -> StreamSnapshotBuilder {
        StreamSnapshotBuilder::from_range(revision, range)
    }

    pub fn revision(&self) -> StreamRevision {
        self.revision
    }

    pub fn source_base(&self) -> StreamOffset {
        self.source_base
    }

    pub fn source_end(&self) -> StreamOffset {
        self.source_end
    }

    pub fn stable_through(&self) -> StreamOffset {
        self.stable_through
    }

    #[cfg(feature = "test-util")]
    #[doc(hidden)]
    pub fn view_for_test(&self) -> View {
        self.view.clone().into_static_view()
    }
}

/// Validated construction boundary for source snapshots.
#[derive(Debug, Clone)]
pub struct StreamSnapshotBuilder {
    revision: StreamRevision,
    source_base: StreamOffset,
    stable_through: StreamOffset,
    source_end: StreamOffset,
    nodes: Vec<StreamNode>,
}

impl StreamSnapshotBuilder {
    pub fn from_range(revision: StreamRevision, range: StreamRange) -> Self {
        Self {
            revision,
            source_base: range.start(),
            stable_through: range.start(),
            source_end: range.end(),
            nodes: Vec::new(),
        }
    }

    pub fn stable_through(mut self, offset: StreamOffset) -> Self {
        self.stable_through = offset;
        self
    }

    pub fn fully_stable(mut self) -> Self {
        self.stable_through = self.source_end;
        self
    }

    pub fn new(
        revision: StreamRevision,
        source_base: StreamOffset,
        stable_through: StreamOffset,
        source_end: StreamOffset,
    ) -> Self {
        Self {
            revision,
            source_base,
            stable_through,
            source_end,
            nodes: Vec::new(),
        }
    }

    pub fn exact_text(
        mut self,
        range: StreamRange,
        spans: impl IntoIterator<Item = TextSpan>,
    ) -> Self {
        self.nodes
            .push(StreamNode::exact_text(range, spans.into_iter().collect()));
        self
    }

    pub(crate) fn continuous_exact_text(
        mut self,
        range: StreamRange,
        spans: impl IntoIterator<Item = TextSpan>,
    ) -> Self {
        self.nodes.push(StreamNode::continuous_exact_text(
            range,
            spans.into_iter().collect(),
        ));
        self
    }

    pub fn exact_line(
        mut self,
        range: StreamRange,
        spans: impl IntoIterator<Item = TextSpan>,
    ) -> Self {
        self.nodes.push(StreamNode::exact_line(
            range,
            spans.into_iter().collect(),
            true,
        ));
        self
    }

    pub fn projected_text(mut self, text: ProjectedText) -> Self {
        self.nodes.push(StreamNode::projected_text(text));
        self
    }

    pub fn atomic(mut self, range: StreamRange, view: View) -> Result<Self, StreamValidationError> {
        if view.contains_component_identity() {
            return Err(StreamValidationError::AtomicContainsComponent);
        }
        self.nodes.push(StreamNode::atomic(range, view));
        Ok(self)
    }

    pub fn finish(self) -> Result<StreamSnapshot, StreamValidationError> {
        let snapshot = StreamSnapshot {
            revision: self.revision,
            source_base: self.source_base,
            source_end: self.source_end,
            stable_through: self.stable_through,
            view: StreamView::new(self.nodes),
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}
