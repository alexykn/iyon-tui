//! Generic semantic stream model and resident/source coordination.

use super::{
    StreamOffset, StreamRange, StreamRowAnchor, StreamSnapshot, StreamView, StreamingSource,
    compile::{CompiledStream, compile_stream_with_theme},
    node::{StreamSliceError, semantic_slice_nodes},
    resident::ResidentPrefix,
    validate::StreamValidationError,
};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamModelError {
    Validation(StreamValidationError),
    RevisionRegressed,
    SourceBaseRegressed,
    SourceEndRegressed,
    StabilityRegressed,
    ChangedWithoutRevision,
    SourceBeforeResident,
    CompactionChangedCoordinates,
    CompactionChangedSemanticSuffix,
    SourceNotSealed,
    UnstableAfterSeal,
}

impl std::fmt::Display for StreamModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "stream model error: {self:?}")
    }
}

impl std::error::Error for StreamModelError {}

pub(crate) struct StreamModel<S: StreamingSource> {
    source: S,
    resident: ResidentPrefix,
    current: StreamSnapshot,
    semantic_changed_from: StreamOffset,
}

impl<S: StreamingSource> StreamModel<S> {
    pub(crate) fn new(source: S) -> Result<Self, StreamModelError> {
        let current = source.snapshot();
        current.validate().map_err(StreamModelError::Validation)?;
        Ok(Self {
            resident: ResidentPrefix::new(current.source_base),
            source,
            semantic_changed_from: current.source_base,
            current,
        })
    }

    pub(crate) fn source(&self) -> &S {
        &self.source
    }

    pub(crate) fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    pub(crate) fn snapshot(&self) -> &StreamSnapshot {
        &self.current
    }

    #[cfg(test)]
    pub(crate) fn resident(&self) -> &ResidentPrefix {
        &self.resident
    }

    pub(crate) fn semantic_base(&self) -> StreamOffset {
        self.resident.base()
    }

    pub(crate) fn semantic_changed_from(&self) -> StreamOffset {
        self.semantic_changed_from
    }

    pub(crate) fn refresh(&mut self) -> Result<(), StreamModelError> {
        let observed = self.source.snapshot();
        Self::validate_transition(&self.current, &observed)?;
        if observed.source_base > self.resident.end() {
            return Err(StreamModelError::SourceBeforeResident);
        }

        // Append-only sources may expose a newly stable partition of the same
        // semantic prefix. A source-end increase therefore damages only the
        // prior unstable frontier. If the prior snapshot was fully stable,
        // same-length replacement is treated conservatively as damage from the
        // source base; this keeps generic sources correct as well.
        let semantic_changed_from = if observed.source_end > self.current.source_end
            || self.current.stable_through < self.current.source_end
        {
            self.current.stable_through.max(self.current.source_base)
        } else {
            self.current.source_base
        };
        let resident_end = self.resident.end();
        let mut staged_nodes = Vec::new();
        let mut captured_end = resident_end;
        for node in observed.view.suffix_from(resident_end).nodes {
            crate::perf::inc(crate::perf::Counter::StreamSourceNodesExamined);
            if node.owned_range().end > observed.stable_through {
                break;
            }
            captured_end = node.owned_range().end;
            staged_nodes.push(node);
        }

        let next = if captured_end == resident_end {
            observed
        } else {
            let before_suffix = observed.view.suffix_from(captured_end);
            let before_end = observed.source_end;
            let before_stable = observed.stable_through;
            self.source.compact_before(captured_end);
            let compacted = self.source.snapshot();
            Self::validate_transition(&observed, &compacted)?;
            if compacted.source_base > captured_end {
                return Err(StreamModelError::SourceBeforeResident);
            }
            if compacted.source_end != before_end || compacted.stable_through != before_stable {
                return Err(StreamModelError::CompactionChangedCoordinates);
            }
            if compacted.view.suffix_from(captured_end) != before_suffix {
                return Err(StreamModelError::CompactionChangedSemanticSuffix);
            }
            compacted
        };

        if next.source_base > captured_end {
            return Err(StreamModelError::SourceBeforeResident);
        }
        for node in staged_nodes {
            self.resident.push(node);
        }
        self.current = next;
        self.semantic_changed_from = semantic_changed_from;
        Ok(())
    }

    pub(crate) fn seal(&mut self) -> Result<(), StreamModelError> {
        self.source.seal();
        let observed = self.source.snapshot();
        Self::validate_transition(&self.current, &observed)?;
        if observed.source_base > self.resident.end() {
            return Err(StreamModelError::SourceBeforeResident);
        }
        if !self.source.is_sealed() {
            return Err(StreamModelError::SourceNotSealed);
        }
        if observed.stable_through != observed.source_end {
            return Err(StreamModelError::UnstableAfterSeal);
        }

        self.refresh()?;
        if self.resident.end() != self.current.source_end {
            return Err(StreamModelError::UnstableAfterSeal);
        }
        Ok(())
    }

    pub(crate) fn semantic_view(&self) -> StreamView {
        self.combined_view()
    }

    pub(crate) fn semantic_view_from(&self, offset: StreamOffset) -> StreamView {
        let frontier = self.resident.end();
        let source_end = self.current.source_end;
        let mut nodes = Vec::new();

        if offset < frontier {
            let end = frontier.min(source_end);
            if offset < end {
                nodes.extend(
                    semantic_slice_nodes(
                        self.resident.nodes_from(offset),
                        StreamRange::new(offset, end),
                    )
                    .expect("resident stream suffix must be sliceable")
                    .nodes,
                );
            }
        }

        let current_start = offset.max(frontier);
        if current_start < source_end {
            nodes.extend(
                semantic_slice_nodes(
                    self.current.view.nodes.iter(),
                    StreamRange::new(current_start, source_end),
                )
                .expect("current stream suffix must be sliceable")
                .nodes,
            );
        }
        StreamView::new(nodes)
    }

    /// Hard-line starts for a sequence of row anchors, sharing one break scan.
    pub(crate) fn hard_line_starts_for(
        &self,
        anchors: &[StreamRowAnchor],
        indexed_from: StreamOffset,
    ) -> Vec<StreamOffset> {
        self.hard_line_starts_for_from(anchors, indexed_from, indexed_from)
    }

    pub(crate) fn hard_line_starts_for_from(
        &self,
        anchors: &[StreamRowAnchor],
        indexed_from: StreamOffset,
        scan_from: StreamOffset,
    ) -> Vec<StreamOffset> {
        let breaks = self.hard_line_breaks(scan_from);
        anchors
            .iter()
            .map(|anchor| {
                let offset = match anchor {
                    StreamRowAnchor::Checkpoint(offset) => *offset,
                    StreamRowAnchor::Atomic { range, .. } => range.start,
                };
                breaks
                    .iter()
                    .copied()
                    .take_while(|start| *start <= offset)
                    .last()
                    .unwrap_or(indexed_from)
            })
            .collect()
    }

    /// Monotonic ascending hard-line start offsets (including the indexed
    /// source coordinate). Newline atoms and hard-newline terminators advance
    /// the break; atomic node starts are breaks to avoid reflowing them.
    fn hard_line_breaks(&self, indexed_from: StreamOffset) -> Vec<StreamOffset> {
        let mut breaks = vec![indexed_from];
        for node in self.semantic_view_from(indexed_from).nodes {
            match node {
                super::StreamNode::Text(text) | super::StreamNode::ContinuousText(text) => {
                    for atom in super::projected::projected_atoms(&text) {
                        if atom.display == "\n" && atom.owned.end >= indexed_from {
                            breaks.push(atom.owned.end);
                        }
                    }
                    let owned_end = text.owned_range().end;
                    if owned_end > text.content_range.end && owned_end >= indexed_from {
                        breaks.push(owned_end);
                    }
                }
                super::StreamNode::Atomic { range, .. } if range.start >= indexed_from => {
                    breaks.push(range.start)
                }
                super::StreamNode::Atomic { .. } => {}
            }
        }
        breaks.sort_unstable();
        breaks.dedup();
        breaks
    }

    pub(crate) fn semantic_slice(
        &self,
        range: StreamRange,
    ) -> Result<StreamView, StreamSliceError> {
        let frontier = self.resident.end();
        let mut nodes = Vec::new();

        if range.start() < frontier {
            let end = range.end().min(frontier);
            if range.start() < end {
                nodes.extend(
                    semantic_slice_nodes(
                        self.resident.nodes_from(range.start()),
                        StreamRange::new(range.start(), end),
                    )?
                    .nodes,
                );
            }
        }

        if range.end() > frontier {
            let start = range.start().max(frontier);
            if start < range.end() {
                nodes.extend(
                    semantic_slice_nodes(
                        self.current.view.nodes.iter(),
                        StreamRange::new(start, range.end()),
                    )?
                    .nodes,
                );
            }
        }

        Ok(StreamView::new(nodes))
    }

    #[cfg(test)]
    pub(crate) fn compile(&self, width: u16) -> CompiledStream {
        compile_stream_with_theme(
            &self.combined_view(),
            width,
            self.current.stable_through,
            &crate::Theme::default(),
        )
    }

    pub(crate) fn compile_from(
        &self,
        offset: StreamOffset,
        width: u16,
        theme: &crate::Theme,
    ) -> CompiledStream {
        compile_stream_with_theme(
            &self.semantic_view_from(offset),
            width,
            self.current.stable_through,
            theme,
        )
    }

    pub(crate) fn release_resident_through(&mut self, offset: StreamOffset) -> StreamOffset {
        self.resident.release_through(offset)
    }

    fn combined_view(&self) -> StreamView {
        let mut nodes = self.resident.view().nodes;
        nodes.extend(self.current.view.suffix_from(self.resident.end()).nodes);
        StreamView::new(nodes)
    }

    fn validate_transition(
        previous: &StreamSnapshot,
        next: &StreamSnapshot,
    ) -> Result<(), StreamModelError> {
        next.validate().map_err(StreamModelError::Validation)?;
        if next.revision < previous.revision {
            return Err(StreamModelError::RevisionRegressed);
        }
        if next.source_base < previous.source_base {
            return Err(StreamModelError::SourceBaseRegressed);
        }
        if next.source_end < previous.source_end {
            return Err(StreamModelError::SourceEndRegressed);
        }
        if next.stable_through < previous.stable_through {
            return Err(StreamModelError::StabilityRegressed);
        }
        if next.revision == previous.revision
            && (next.source_base != previous.source_base
                || next.source_end != previous.source_end
                || next.stable_through != previous.stable_through
                || next.view != previous.view)
        {
            return Err(StreamModelError::ChangedWithoutRevision);
        }
        Ok(())
    }
}

impl From<StreamValidationError> for StreamModelError {
    fn from(error: StreamValidationError) -> Self {
        Self::Validation(error)
    }
}
