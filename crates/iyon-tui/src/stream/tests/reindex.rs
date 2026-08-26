//! Incremental atomic-row reflow must equal a cold full recompile.

use crate::{
    View,
    presentation::IntoView,
    stream::{
        StreamModel, StreamOffset, StreamRange, StreamRevision, StreamRowIndex, StreamSnapshot,
        StreamSnapshotBuilder, StreamingSource, TextStream, build_index_from, reindex_in_place,
    },
};

/// Reuses the production in-place suffix reindexer while keeping the test's
/// previous index immutable for the cold-vs-incremental comparison.
fn reindex<S: StreamingSource>(
    model: &StreamModel<S>,
    previous: &StreamRowIndex,
    start: StreamOffset,
    changed_from: StreamOffset,
    width: u16,
) -> StreamRowIndex {
    let mut index = previous.clone();
    reindex_in_place(model, &mut index, start, changed_from, width);
    index
}

/// A tiny append-only source whose semantic units are indivisible atomic views
/// (the same shape the Markdown/smoother stream pipeline produces).
#[derive(Default)]
struct AtomicBlocks {
    blocks: Vec<(StreamRange, View)>,
    base: StreamOffset,
    end: StreamOffset,
    revision: StreamRevision,
    sealed: bool,
}

impl AtomicBlocks {
    fn push(&mut self, text: &str) {
        let start = self.end;
        let end = start.saturating_add(text.len() as u64);
        self.blocks
            .push((StreamRange::new(start, end), View::text(text).into_view()));
        self.end = end;
        self.revision = self.revision.next();
    }
}

impl StreamingSource for AtomicBlocks {
    fn snapshot(&self) -> StreamSnapshot {
        let mut builder = StreamSnapshotBuilder::new(self.revision, self.base, self.end, self.end);
        for (range, view) in &self.blocks {
            builder = builder
                .atomic(*range, view.clone())
                .expect("test atomic block must be component-free");
        }
        builder
            .finish()
            .expect("test atomic snapshot must be valid")
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        self.blocks.retain(|(range, _)| range.end() > offset);
        self.base = self.base.max(offset.min(self.end));
        self.revision = self.revision.next();
    }

    fn seal(&mut self) {
        self.sealed = true;
        self.revision = self.revision.next();
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }
}

#[test]
fn text_reindex_matches_cold_across_stable_prefix_compaction() {
    let mut model = StreamModel::new(TextStream::new()).unwrap();
    let mut previous = None;
    for index in 0..12 {
        let changed_from = model.snapshot().stable_through();
        let chunk = format!("row {index} {}\n", "x".repeat(220));
        model.source_mut().push(chunk);
        model.refresh().unwrap();

        let cold = build_index_from(&model, StreamOffset::ZERO, 32);
        let reindexed = match &previous {
            Some(prev) => reindex(&model, prev, StreamOffset::ZERO, changed_from, 32),
            None => build_index_from(&model, StreamOffset::ZERO, 32),
        };
        assert_eq!(
            cold.anchors, reindexed.anchors,
            "text reindex drifted at chunk {index}"
        );
        previous = Some(cold);
    }
}

#[test]
fn atomic_reindex_reuses_stable_rows_and_equals_cold() {
    for width in [6u16, 16, 40] {
        let mut model = StreamModel::new(AtomicBlocks::default()).unwrap();
        let mut previous = None;
        for index in 0..12 {
            let changed_from = model.snapshot().stable_through();
            model
                .source_mut()
                .push(&format!("block {index} that wraps over the terminal width"));
            model.refresh().unwrap();

            let cold = build_index_from(&model, StreamOffset::ZERO, width);
            let reindexed = match &previous {
                Some(prev) => reindex(&model, prev, StreamOffset::ZERO, changed_from, width),
                None => build_index_from(&model, StreamOffset::ZERO, width),
            };
            assert_eq!(
                cold.anchors, reindexed.anchors,
                "atomic reindex drifted at width={width} block={index}"
            );
            previous = Some(cold);
        }
    }

    #[cfg(feature = "perf-counters")]
    {
        let _lock = crate::perf::test_lock();
        let mut model = StreamModel::new(AtomicBlocks::default()).unwrap();
        let mut previous = None;
        for index in 0..40 {
            let changed_from = model.snapshot().stable_through();
            model.source_mut().push(&format!("block {index}"));
            model.refresh().unwrap();
            crate::perf::reset();
            let reindexed = match &previous {
                Some(prev) => reindex(&model, prev, StreamOffset::ZERO, changed_from, 20),
                None => build_index_from(&model, StreamOffset::ZERO, 20),
            };
            if index > 0 {
                let counters = crate::perf::snapshot();
                assert!(
                    counters.value(crate::perf::Counter::StreamStableRowsReused) > 0,
                    "atomic appends must reuse stable rows (block {index})"
                );
                assert!(
                    counters.value(crate::perf::Counter::StreamRowsReindexed)
                        < reindexed.anchors.len() as u64,
                    "atomic appends must reflow only a suffix (block {index})"
                );
            }
            previous = Some(reindexed);
        }

        let mut model = StreamModel::new(TextStream::new()).unwrap();
        let mut previous = None;
        for index in 0..40 {
            let changed_from = model.snapshot().stable_through();
            model
                .source_mut()
                .push(&format!("line {index}: stable append\n"));
            model.refresh().unwrap();
            crate::perf::reset();
            let reindexed = match &previous {
                Some(prev) => reindex(&model, prev, StreamOffset::ZERO, changed_from, 20),
                None => build_index_from(&model, StreamOffset::ZERO, 20),
            };
            if index > 0 {
                let counters = crate::perf::snapshot();
                assert!(
                    counters.value(crate::perf::Counter::StreamStableRowsReused) > 0,
                    "text appends must reuse stable rows (line {index})"
                );
                assert!(
                    counters.value(crate::perf::Counter::StreamRowsReindexed)
                        < reindexed.anchors.len() as u64,
                    "text appends must reflow only a suffix (line {index})"
                );
                assert!(
                    counters.value(crate::perf::Counter::StreamSemanticRestartOffset)
                        >= counters.value(crate::perf::Counter::StreamVisualRestartOffset),
                    "visual restart must not follow the semantic append offset"
                );
            }
            previous = Some(reindexed);
        }
    }
}
