use std::{cell::RefCell, rc::Rc};

use crate::TextSpan;
use crate::stream::resident::ResidentPrefix;
use crate::stream::*;

#[derive(Clone)]
struct FakeSource {
    state: Rc<RefCell<FakeState>>,
}

struct FakeState {
    nodes: Vec<StreamNode>,
    stable_through: StreamOffset,
    source_base: StreamOffset,
    source_end: StreamOffset,
    revision: StreamRevision,
    sealed: bool,
    corrupt_on_compact: bool,
    compact_revision: Option<StreamRevision>,
}

impl FakeSource {
    fn new(stable_through: u64) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeState {
                nodes: vec![
                    StreamNode::exact_text(range(0, 1), vec![TextSpan::plain("A")]),
                    StreamNode::exact_text(range(1, 2), vec![TextSpan::plain("B")]),
                    StreamNode::exact_text(range(2, 3), vec![TextSpan::plain("C")]),
                ],
                stable_through: StreamOffset::new(stable_through),
                source_base: StreamOffset::ZERO,
                source_end: StreamOffset::new(3),
                revision: StreamRevision::ZERO,
                sealed: false,
                corrupt_on_compact: false,
                compact_revision: None,
            })),
        }
    }

    fn set_stable_without_revision(&self, stable_through: u64) {
        self.state.borrow_mut().stable_through = StreamOffset::new(stable_through);
    }

    fn corrupt_on_compact(&self) {
        self.state.borrow_mut().corrupt_on_compact = true;
    }

    fn set_revision(&self, revision: StreamRevision) {
        self.state.borrow_mut().revision = revision;
    }

    fn set_source_base(&self, source_base: StreamOffset) {
        self.state.borrow_mut().source_base = source_base;
    }

    fn set_source_end(&self, source_end: StreamOffset) {
        let mut state = self.state.borrow_mut();
        state.source_end = source_end;
        state
            .nodes
            .retain(|node| node.owned_range().end <= source_end);
        state.stable_through = state.stable_through.min(source_end);
    }

    fn set_stable_through(&self, stable_through: StreamOffset) {
        self.state.borrow_mut().stable_through = stable_through;
    }

    fn set_compact_revision(&self, revision: StreamRevision) {
        self.state.borrow_mut().compact_revision = Some(revision);
    }
}

impl StreamingSource for FakeSource {
    fn snapshot(&self) -> StreamSnapshot {
        let state = self.state.borrow();
        let view = StreamView::new(state.nodes.clone()).suffix_from(state.source_base);
        StreamSnapshot {
            revision: state.revision,
            source_base: state.source_base,
            source_end: state.source_end,
            stable_through: state.stable_through,
            view,
        }
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        let mut state = self.state.borrow_mut();
        state.source_base = offset;
        if state.corrupt_on_compact {
            state.nodes[2] = StreamNode::exact_text(range(2, 3), vec![TextSpan::plain("X")]);
        }
        state.revision = state
            .compact_revision
            .take()
            .unwrap_or_else(|| state.revision.next());
    }

    fn seal(&mut self) {
        let mut state = self.state.borrow_mut();
        state.sealed = true;
        state.stable_through = StreamOffset::new(3);
        state.revision = state.revision.next();
    }

    fn is_sealed(&self) -> bool {
        self.state.borrow().sealed
    }
}

fn range(start: u64, end: u64) -> StreamRange {
    StreamRange::new(StreamOffset::new(start), StreamOffset::new(end))
}

struct OverlapSource {
    compacted: bool,
}

impl StreamingSource for OverlapSource {
    fn snapshot(&self) -> StreamSnapshot {
        let nodes = if self.compacted {
            vec![StreamNode::exact_text(
                range(0, 3),
                vec![TextSpan::plain("ABC")],
            )]
        } else {
            vec![
                StreamNode::exact_text(range(0, 1), vec![TextSpan::plain("A")]),
                StreamNode::exact_text(range(1, 2), vec![TextSpan::plain("B")]),
                StreamNode::exact_text(range(2, 3), vec![TextSpan::plain("C")]),
            ]
        };
        StreamSnapshot {
            revision: if self.compacted {
                StreamRevision::new(1)
            } else {
                StreamRevision::ZERO
            },
            source_base: StreamOffset::ZERO,
            source_end: StreamOffset::new(3),
            stable_through: StreamOffset::new(2),
            view: StreamView::new(nodes),
        }
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        assert_eq!(offset, StreamOffset::new(2));
        self.compacted = true;
    }

    fn seal(&mut self) {}

    fn is_sealed(&self) -> bool {
        false
    }
}

#[test]
fn semantic_slice_respects_resident_source_frontier_overlap() {
    let mut model = StreamModel::new(OverlapSource { compacted: false }).unwrap();
    model.refresh().unwrap();

    let combined = model.semantic_view();
    assert_eq!(
        combined
            .nodes
            .iter()
            .map(StreamNode::owned_range)
            .collect::<Vec<_>>(),
        vec![range(0, 1), range(1, 2), range(2, 3)]
    );

    let sliced = model.semantic_slice(range(1, 3)).unwrap();
    assert_eq!(
        sliced
            .nodes
            .iter()
            .map(StreamNode::owned_range)
            .collect::<Vec<_>>(),
        vec![range(1, 2), range(2, 3)]
    );
}

fn semantic_text<S: StreamingSource>(model: &StreamModel<S>) -> String {
    model
        .semantic_view()
        .nodes
        .iter()
        .filter_map(|node| match node {
            StreamNode::Text(text) | StreamNode::ContinuousText(text) => Some(
                text.runs
                    .iter()
                    .map(|run| run.display.as_str())
                    .collect::<String>(),
            ),
            StreamNode::Atomic { .. } => None,
        })
        .collect()
}

#[test]
fn non_send_source_is_accepted_and_resident_capture_is_semantic() {
    let source = FakeSource::new(2);
    let mut model = StreamModel::new(source).unwrap();
    model.refresh().unwrap();
    assert_eq!(model.resident().end(), StreamOffset::new(2));
    assert_eq!(semantic_text(&model), "ABC");

    let wide = model.compile(20);
    let narrow = model.compile(2);
    assert_eq!(
        wide.rows
            .iter()
            .map(|row| row.plain_text())
            .collect::<String>(),
        "ABC"
    );
    assert_eq!(
        narrow
            .rows
            .iter()
            .map(|row| row.plain_text())
            .collect::<String>(),
        "ABC"
    );
}

#[test]
fn resident_source_overlap_does_not_duplicate_nodes() {
    let source = FakeSource::new(2);
    let mut model = StreamModel::new(source).unwrap();
    model.refresh().unwrap();
    assert_eq!(model.semantic_view().nodes.len(), 3);
    assert_eq!(semantic_text(&model), "ABC");
}

#[test]
fn malicious_compaction_semantic_change_is_rejected() {
    let source = FakeSource::new(2);
    source.corrupt_on_compact();
    let mut model = StreamModel::new(source).unwrap();
    let before_snapshot = model.snapshot().clone();
    let before_resident_end = model.resident().end();
    let before_view = model.semantic_view();

    assert_eq!(
        model.refresh(),
        Err(StreamModelError::CompactionChangedSemanticSuffix)
    );
    assert_eq!(model.snapshot(), &before_snapshot);
    assert_eq!(model.resident().end(), before_resident_end);
    assert_eq!(model.semantic_view(), before_view);
}

#[test]
fn revision_regression_is_rejected() {
    let source = FakeSource::new(3);
    source.set_revision(StreamRevision::new(1));
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_revision(StreamRevision::ZERO);
    assert_eq!(model.refresh(), Err(StreamModelError::RevisionRegressed));
}

#[test]
fn source_base_regression_is_rejected() {
    let source = FakeSource::new(3);
    source.set_source_base(StreamOffset::new(1));
    source.set_revision(StreamRevision::new(1));
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_source_base(StreamOffset::ZERO);
    source.set_revision(StreamRevision::new(2));
    assert_eq!(model.refresh(), Err(StreamModelError::SourceBaseRegressed));
}

#[test]
fn source_end_regression_is_rejected() {
    let source = FakeSource::new(3);
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_source_end(StreamOffset::new(2));
    source.set_revision(StreamRevision::new(1));
    assert_eq!(model.refresh(), Err(StreamModelError::SourceEndRegressed));
}

#[test]
fn stability_regression_is_rejected() {
    let source = FakeSource::new(3);
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_stable_through(StreamOffset::new(2));
    source.set_revision(StreamRevision::new(1));
    assert_eq!(model.refresh(), Err(StreamModelError::StabilityRegressed));
}

#[test]
fn intra_refresh_changed_snapshot_without_revision_is_rejected_transactionally() {
    let source = FakeSource::new(2);
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_revision(StreamRevision::new(1));
    source.set_compact_revision(StreamRevision::new(1));
    let before_snapshot = model.snapshot().clone();
    let before_resident_end = model.resident().end();
    let before_view = model.semantic_view();

    assert_eq!(
        model.refresh(),
        Err(StreamModelError::ChangedWithoutRevision)
    );
    assert_eq!(model.snapshot(), &before_snapshot);
    assert_eq!(model.resident().end(), before_resident_end);
    assert_eq!(model.semantic_view(), before_view);
}

#[test]
fn intra_refresh_revision_regression_is_rejected_transactionally() {
    let source = FakeSource::new(2);
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_revision(StreamRevision::new(2));
    source.set_compact_revision(StreamRevision::new(1));
    let before_snapshot = model.snapshot().clone();
    let before_resident_end = model.resident().end();
    let before_view = model.semantic_view();

    assert_eq!(model.refresh(), Err(StreamModelError::RevisionRegressed));
    assert_eq!(model.snapshot(), &before_snapshot);
    assert_eq!(model.resident().end(), before_resident_end);
    assert_eq!(model.semantic_view(), before_view);
}

#[test]
fn changed_snapshot_without_revision_is_rejected() {
    let source = FakeSource::new(1);
    let mut model = StreamModel::new(source.clone()).unwrap();
    source.set_stable_without_revision(2);
    assert_eq!(
        model.refresh(),
        Err(StreamModelError::ChangedWithoutRevision)
    );
}

#[test]
fn sealing_captures_every_whole_node() {
    let source = FakeSource::new(2);
    let mut model = StreamModel::new(source).unwrap();
    model.seal().unwrap();
    assert_eq!(model.snapshot().stable_through, model.snapshot().source_end);
    assert_eq!(model.resident().end(), model.snapshot().source_end);
}

#[test]
fn text_stream_preserves_graphemes_across_appends_before_promotion() {
    let mut model = StreamModel::new(TextStream::new()).unwrap();
    model.source_mut().push("e");
    model.refresh().unwrap();
    model.source_mut().push("\u{301}");
    model.refresh().unwrap();
    assert_eq!(model.resident().end(), StreamOffset::ZERO);

    model.source_mut().push("x");
    model.refresh().unwrap();
    assert_eq!(model.resident().end(), StreamOffset::new(3));
    assert_eq!(model.source().retained_text(), "x");
    assert_eq!(semantic_text(&model), "e\u{301}x");
}

#[test]
fn text_stream_promotes_completed_lines_and_releases_source_bytes() {
    let mut model = StreamModel::new(TextStream::new()).unwrap();
    model.source_mut().push("first line\n");
    model.refresh().unwrap();

    assert_eq!(model.resident().end(), StreamOffset::new(11));
    assert_eq!(model.source().retained_text(), "");

    model.source_mut().push("open tail");
    model.refresh().unwrap();
    assert_eq!(model.source().source_base(), StreamOffset::new(19));
    assert_eq!(model.source().retained_text(), "l");
}

#[test]
fn text_stream_model_seal_captures_and_compacts_without_losing_content() {
    let mut model = StreamModel::new(TextStream::from_text("hello")).unwrap();
    model.seal().unwrap();

    assert_eq!(model.source().source_base(), StreamOffset::new(5));
    assert_eq!(model.resident().end(), StreamOffset::new(5));
    assert_eq!(
        model
            .semantic_view()
            .nodes
            .iter()
            .filter_map(|node| match node {
                StreamNode::Text(text) | StreamNode::ContinuousText(text) => Some(
                    text.runs
                        .iter()
                        .map(|run| run.display.as_str())
                        .collect::<String>(),
                ),
                StreamNode::Atomic { .. } => None,
            })
            .collect::<String>(),
        "hello"
    );
}

#[test]
fn resident_release_does_not_split_a_node() {
    let source = FakeSource::new(3);
    let mut model = StreamModel::new(source).unwrap();
    model.refresh().unwrap();
    assert_eq!(
        model.release_resident_through(StreamOffset::new(1)),
        StreamOffset::new(1)
    );
    assert_eq!(model.resident().end(), StreamOffset::new(3));
    assert_eq!(semantic_text(&model), "BC");
}

#[test]
fn resident_release_keeps_a_node_whole_when_offset_is_inside_it() {
    let mut resident = ResidentPrefix::new(StreamOffset::ZERO);
    resident.push(StreamNode::exact_text(
        range(0, 1),
        vec![TextSpan::plain("A")],
    ));
    resident.push(StreamNode::exact_text(
        range(1, 3),
        vec![TextSpan::plain("B")],
    ));
    resident.push(StreamNode::exact_text(
        range(3, 4),
        vec![TextSpan::plain("C")],
    ));

    assert_eq!(
        resident.release_through(StreamOffset::new(2)),
        StreamOffset::new(1)
    );
    assert_eq!(resident.nodes().count(), 2);
    assert_eq!(
        resident.release_through(StreamOffset::new(3)),
        StreamOffset::new(3)
    );
    assert_eq!(resident.nodes().count(), 1);
}
