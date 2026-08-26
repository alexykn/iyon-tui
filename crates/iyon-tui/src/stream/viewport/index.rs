use super::super::{StreamModel, StreamRevision, StreamRowAnchor, StreamingSource, compile_stream};

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static BUILD_INDEX_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StreamRowIndex {
    pub(crate) revision: StreamRevision,
    pub(crate) width: u16,
    /// Source offset this index starts at (its row 0 anchors this offset).
    pub(crate) indexed_from: super::super::StreamOffset,
    /// Source coordinate through which this index has been compiled.
    pub(crate) indexed_through: super::super::StreamOffset,
    /// Earliest semantic coordinate recompiled for this revision.
    pub(crate) semantic_changed_from: super::super::StreamOffset,
    /// Earliest visual coordinate whose rows were recompiled.
    pub(crate) visual_restart_from: super::super::StreamOffset,
    pub(crate) anchors: Vec<StreamRowAnchor>,
    /// Hard-line start of every row, parallel to `anchors`.
    pub(crate) hard_line_starts: Vec<super::super::StreamOffset>,
}

impl StreamRowIndex {
    /// Drops anchors whose checkpoint is strictly before `offset`, adjusting the
    /// indexed-from base. Used when a native sink releases resident prefix rows.
    pub(crate) fn retain_from(&mut self, offset: super::super::StreamOffset) {
        let first = self
            .anchors
            .iter()
            .position(|anchor| anchor_offset(anchor) >= offset)
            .unwrap_or(self.anchors.len());
        self.anchors.drain(..first);
        self.hard_line_starts.drain(..first);
        self.indexed_from = self.indexed_from.max(offset);
        self.semantic_changed_from = self.semantic_changed_from.max(offset);
        self.visual_restart_from = self.visual_restart_from.max(offset);
    }
}

pub(crate) fn build_index<S: StreamingSource>(
    model: &StreamModel<S>,
    width: u16,
) -> StreamRowIndex {
    build_index_from(model, super::super::StreamOffset::ZERO, width)
}

pub(crate) fn build_index_from<S: StreamingSource>(
    model: &StreamModel<S>,
    start: super::super::StreamOffset,
    width: u16,
) -> StreamRowIndex {
    #[cfg(test)]
    BUILD_INDEX_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
    crate::perf::set(
        crate::perf::Counter::StreamSemanticRestartOffset,
        start.as_u64(),
    );
    crate::perf::set(
        crate::perf::Counter::StreamVisualRestartOffset,
        start.as_u64(),
    );

    let snapshot = model.snapshot();
    let compiled = compile_stream(
        &model.semantic_view_from(start),
        width,
        snapshot.stable_through,
    );
    let anchors = compiled
        .rows
        .into_iter()
        .map(|row| row.anchor)
        .collect::<Vec<_>>();
    let hard_line_starts = model.hard_line_starts_for(&anchors, start);
    crate::perf::add(
        crate::perf::Counter::StreamRowsReindexed,
        anchors.len() as u64,
    );
    StreamRowIndex {
        revision: snapshot.revision,
        width,
        indexed_from: start,
        indexed_through: snapshot.source_end,
        semantic_changed_from: start,
        visual_restart_from: start,
        anchors,
        hard_line_starts,
    }
}

/// Rebuilds a retained index in place. The cache owner can keep the same
/// allocation for the stable prefix and replace only the damaged suffix.
pub(crate) fn reindex_in_place<S: StreamingSource>(
    model: &StreamModel<S>,
    index: &mut StreamRowIndex,
    start: super::super::StreamOffset,
    semantic_changed_from: super::super::StreamOffset,
    width: u16,
) {
    if index.width != width || index.indexed_from != start {
        *index = build_index_from(model, start, width);
        return;
    }

    let all_atomic = index
        .anchors
        .iter()
        .all(|anchor| matches!(anchor, StreamRowAnchor::Atomic { .. }));
    let atomic_prefix_is_stable = all_atomic
        && index.anchors.last().is_some_and(|anchor| {
            matches!(
                anchor,
                StreamRowAnchor::Atomic { range, .. }
                    if range.end <= semantic_changed_from
            )
        });
    let first_line_after_change = index
        .hard_line_starts
        .partition_point(|line_start| *line_start <= semantic_changed_from);
    let mut visual_restart = if atomic_prefix_is_stable {
        semantic_changed_from
    } else {
        first_line_after_change
            .checked_sub(1)
            .and_then(|position| index.hard_line_starts.get(position).copied())
            .unwrap_or(start)
    };
    visual_restart = visual_restart.max(start);
    crate::perf::set(
        crate::perf::Counter::StreamSemanticRestartOffset,
        semantic_changed_from.as_u64(),
    );
    crate::perf::set(
        crate::perf::Counter::StreamVisualRestartOffset,
        visual_restart.as_u64(),
    );

    let retained_len = index
        .hard_line_starts
        .partition_point(|line_start| *line_start < visual_restart);
    index.anchors.truncate(retained_len);
    index.hard_line_starts.truncate(retained_len);

    let compiled = compile_stream(
        &model.semantic_view_from(visual_restart),
        width,
        model.snapshot().stable_through,
    );
    index
        .anchors
        .extend(compiled.rows.into_iter().map(|row| row.anchor));
    let suffix_starts =
        model.hard_line_starts_for_from(&index.anchors[retained_len..], start, visual_restart);
    index.hard_line_starts.extend(suffix_starts);
    index.revision = model.snapshot().revision;
    index.indexed_through = model.snapshot().source_end;
    index.semantic_changed_from = semantic_changed_from;
    index.visual_restart_from = visual_restart;

    crate::perf::add(
        crate::perf::Counter::StreamStableRowsReused,
        retained_len as u64,
    );
    crate::perf::add(
        crate::perf::Counter::StreamRowsReindexed,
        index.anchors.len().saturating_sub(retained_len) as u64,
    );
}

fn anchor_offset(anchor: &StreamRowAnchor) -> super::super::StreamOffset {
    match anchor {
        StreamRowAnchor::Checkpoint(offset) => *offset,
        StreamRowAnchor::Atomic { range, .. } => range.start,
    }
}

#[cfg(test)]
pub(crate) fn reset_build_index_call_count() {
    BUILD_INDEX_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn build_index_call_count() -> usize {
    BUILD_INDEX_CALLS.with(Cell::get)
}
