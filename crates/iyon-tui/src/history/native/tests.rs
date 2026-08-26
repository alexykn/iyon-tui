use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use super::super::*;
use super::{NativeBlockReason, NativeTransferOutcome};
use crate::history::projection::project;
use crate::{
    Component, ComponentCx, TextSpan, Theme, ThemeColor,
    backend::NativeHistorySink,
    geometry::Size,
    physical::{PhysicalColor, PhysicalRow},
    presentation::{ColorSpec, Insets, IntoView, StyleSpec, View},
    stream::{
        StreamOffset, StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder,
        StreamingSource,
    },
};

#[derive(Default)]
struct TestSink {
    rows: Vec<PhysicalRow>,
}

impl NativeHistorySink for TestSink {
    type Error = ();

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        self.rows.extend(rows.iter().cloned());
        Ok(rows.len())
    }
}

#[derive(Clone)]
struct NativeSource {
    snapshot: StreamSnapshot,
    sealed: bool,
}

impl NativeSource {
    fn from_snapshot(snapshot: StreamSnapshot, sealed: bool) -> Self {
        Self { snapshot, sealed }
    }

    fn new(text: &str, stable_through: u64, sealed: bool) -> Self {
        Self::at(0, text, stable_through, sealed)
    }

    fn at(base: u64, text: &str, stable_through: u64, sealed: bool) -> Self {
        let end = base.saturating_add(text.len() as u64);
        let snapshot = StreamSnapshotBuilder::new(
            StreamRevision::ZERO,
            StreamOffset::new(base),
            StreamOffset::new(stable_through),
            StreamOffset::new(end),
        )
        .exact_text(
            StreamRange::new(StreamOffset::new(base), StreamOffset::new(end)),
            [TextSpan::plain(text)],
        )
        .finish()
        .unwrap();
        Self::from_snapshot(snapshot, sealed)
    }
}

#[test]
fn static_native_transfer_uses_the_active_theme() {
    let mut history = crate::History::new();
    history
        .push(View::text("themed").style(StyleSpec::new().foreground(ColorSpec::theme("accent"))))
        .unwrap();
    let mut sink = TestSink::default();
    let theme = Theme::new().with_color("accent", ThemeColor::Indexed(42));
    transfer_native_prefix_with_theme(&mut history, &mut sink, 20, 20, &theme).unwrap();
    assert_eq!(
        sink.rows[0].style_at(0).unwrap().foreground,
        Some(PhysicalColor::Indexed(42))
    );
}

#[test]
fn resident_native_rows_keep_their_committed_theme() {
    let mut history = crate::History::new();
    history
        .push(View::text("resident").style(StyleSpec::new().foreground(ColorSpec::theme("accent"))))
        .unwrap();
    let mut sink = TestSink::default();
    let first = Theme::new().with_color("accent", ThemeColor::Indexed(44));
    let second = Theme::new().with_color("accent", ThemeColor::Indexed(45));
    transfer_native_prefix_with_theme(&mut history, &mut sink, 20, 20, &first).unwrap();
    transfer_native_prefix_with_theme(&mut history, &mut sink, 20, 20, &second).unwrap();
    assert_eq!(
        sink.rows[0].style_at(0).unwrap().foreground,
        Some(PhysicalColor::Indexed(44))
    );
}

#[test]
fn stream_native_transfer_uses_the_active_theme() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(6),
        StreamOffset::new(6),
    )
    .exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(6)),
        [TextSpan::styled(
            "themed",
            StyleSpec::new().foreground(ColorSpec::theme("accent")),
        )],
    )
    .finish()
    .unwrap();
    let mut history = crate::History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut sink = TestSink::default();
    let theme = Theme::new().with_color("accent", ThemeColor::Indexed(43));
    transfer_native_prefix_with_theme(&mut history, &mut sink, 20, 20, &theme).unwrap();
    assert_eq!(
        sink.rows[0].style_at(0).unwrap().foreground,
        Some(PhysicalColor::Indexed(43))
    );
}

impl StreamingSource for NativeSource {
    fn snapshot(&self) -> StreamSnapshot {
        self.snapshot.clone()
    }

    fn seal(&mut self) {
        self.sealed = true;
        self.snapshot.stable_through = self.snapshot.source_end;
        self.snapshot.revision = StreamRevision::new(self.snapshot.revision.as_u64() + 1);
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }
}

#[derive(Clone)]
struct CompactingSource {
    snapshot: StreamSnapshot,
    sealed: bool,
    compact_calls: Rc<Cell<usize>>,
}

impl CompactingSource {
    fn stabilize(&mut self) {
        self.snapshot.stable_through = self.snapshot.source_end;
        self.snapshot.revision = self.snapshot.revision.next();
    }

    fn new() -> Self {
        let snapshot = StreamSnapshotBuilder::new(
            StreamRevision::ZERO,
            StreamOffset::ZERO,
            StreamOffset::new(1),
            StreamOffset::new(2),
        )
        .exact_text(
            StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
            [TextSpan::plain("A")],
        )
        .exact_text(
            StreamRange::new(StreamOffset::new(1), StreamOffset::new(2)),
            [TextSpan::plain("B")],
        )
        .finish()
        .unwrap();
        Self {
            snapshot,
            sealed: false,
            compact_calls: Rc::new(Cell::new(0)),
        }
    }
}

impl StreamingSource for CompactingSource {
    fn snapshot(&self) -> StreamSnapshot {
        self.snapshot.clone()
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        self.compact_calls
            .set(self.compact_calls.get().saturating_add(1));
        self.snapshot.source_base = offset;
        self.snapshot.view = self.snapshot.view.suffix_from(offset);
        self.snapshot.revision = self.snapshot.revision.next();
    }

    fn seal(&mut self) {
        self.sealed = true;
        self.snapshot.stable_through = self.snapshot.source_end;
        self.snapshot.revision = self.snapshot.revision.next();
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }
}

#[derive(Clone)]
struct GrowingSource {
    state: Rc<RefCell<GrowingState>>,
}

struct GrowingState {
    snapshot: StreamSnapshot,
    sealed: bool,
}

impl GrowingSource {
    fn new() -> Self {
        let snapshot = StreamSnapshotBuilder::new(
            StreamRevision::ZERO,
            StreamOffset::ZERO,
            StreamOffset::new(2),
            StreamOffset::new(3),
        )
        .exact_text(
            StreamRange::new(StreamOffset::ZERO, StreamOffset::new(2)),
            [TextSpan::plain("A\n")],
        )
        .exact_text(
            StreamRange::new(StreamOffset::new(2), StreamOffset::new(3)),
            [TextSpan::plain("B")],
        )
        .finish()
        .unwrap();
        Self {
            state: Rc::new(RefCell::new(GrowingState {
                snapshot,
                sealed: false,
            })),
        }
    }

    fn grow(&mut self) {
        let snapshot = StreamSnapshotBuilder::new(
            StreamRevision::new(1),
            StreamOffset::ZERO,
            StreamOffset::new(4),
            StreamOffset::new(4),
        )
        .exact_text(
            StreamRange::new(StreamOffset::ZERO, StreamOffset::new(2)),
            [TextSpan::plain("A\n")],
        )
        .exact_text(
            StreamRange::new(StreamOffset::new(2), StreamOffset::new(3)),
            [TextSpan::plain("B")],
        )
        .exact_text(
            StreamRange::new(StreamOffset::new(3), StreamOffset::new(4)),
            [TextSpan::plain("C")],
        )
        .finish()
        .unwrap();
        self.state.borrow_mut().snapshot = snapshot;
    }
}

impl StreamingSource for GrowingSource {
    fn snapshot(&self) -> StreamSnapshot {
        self.state.borrow().snapshot.clone()
    }

    fn seal(&mut self) {
        let mut state = self.state.borrow_mut();
        state.sealed = true;
        state.snapshot.stable_through = state.snapshot.source_end;
        state.snapshot.revision = state.snapshot.revision.next();
    }

    fn is_sealed(&self) -> bool {
        self.state.borrow().sealed
    }
}

struct LiveComponent;

impl Component for LiveComponent {
    fn view(&self) -> View {
        View::text("B").into_view()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

#[derive(Default)]
struct FakeSink {
    rows: Vec<PhysicalRow>,
    accepts: VecDeque<Result<usize, &'static str>>,
    calls: usize,
}

impl FakeSink {
    fn accepting(accepted: impl IntoIterator<Item = Result<usize, &'static str>>) -> Self {
        Self {
            accepts: accepted.into_iter().collect(),
            ..Self::default()
        }
    }
}

impl NativeHistorySink for FakeSink {
    type Error = &'static str;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        self.calls += 1;
        let accepted = self.accepts.pop_front().unwrap_or(Ok(rows.len()))?;
        self.rows.extend(rows[..accepted].iter().cloned());
        Ok(accepted)
    }
}

struct InvalidAckSink;

impl NativeHistorySink for InvalidAckSink {
    type Error = ();

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        Ok(rows.len() + 1)
    }
}

fn transfer(
    history: &mut History,
    sink: &mut FakeSink,
    width: u16,
    max_rows: usize,
) -> NativeTransferOutcome {
    transfer_native_prefix(history, sink, width, max_rows).unwrap()
}

#[test]
fn invalid_acknowledgement_is_rejected_without_mutation() {
    let mut history = History::new();
    history.push("A").unwrap();
    let before = history.native.clone();
    let result = transfer_native_prefix(&mut history, &mut InvalidAckSink, 8, 8);

    assert_eq!(
        result,
        Err(NativeTransferError::InvalidAcknowledgement {
            requested: 1,
            accepted: 2,
        })
    );
    assert_eq!(history.native, before);
    assert_eq!(history.native.physical_rows_inserted, 0);
    assert!(!history.native.has_physical_rows());
    assert_eq!(history.units.len(), 1);
}

#[test]
fn zero_budget_and_width_do_not_prepare_or_mutate() {
    let mut history = History::new();
    history.push("A").unwrap();
    let before = history.native.clone();
    let mut sink = FakeSink::default();

    assert_eq!(transfer(&mut history, &mut sink, 8, 0).inserted, 0);
    assert_eq!(transfer(&mut history, &mut sink, 0, 8).inserted, 0);
    assert_eq!(history.native, before);
    assert!(sink.rows.is_empty());
}

#[test]
fn static_full_ack_pops_front_and_records_predecessor() {
    let mut history = History::new();
    let id = history.push("A").unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);

    assert_eq!(outcome.inserted, 1);
    assert!(history.units.is_empty());
    assert_eq!(history.native.last_native_unit, Some(id));
    assert_eq!(sink.rows[0].plain_text(), "A");
    assert_eq!(sink.rows[0].width(), 8);
}

#[test]
fn static_partial_ack_freezes_exact_remainder() {
    let mut history = History::new();
    let id = history.push("A\nB\nC").unwrap();
    let mut sink = FakeSink::accepting([Ok(1)]);

    let outcome = transfer(&mut history, &mut sink, 8, 3);

    assert_eq!(outcome.inserted, 1);
    assert_eq!(history.native.frozen_static.as_ref().unwrap().unit, id);
    assert_eq!(
        history
            .native
            .frozen_static
            .as_ref()
            .unwrap()
            .rows
            .as_slice()[0]
            .plain_text(),
        "B"
    );
    assert_eq!(history.units.front().unwrap().id, id);
}

#[test]
fn sink_zero_and_error_do_not_publish_static_freeze() {
    for response in [Ok(0), Err("blocked")] {
        let mut history = History::new();
        history.push("A\nB").unwrap();
        let before = history.native.clone();
        let mut sink = FakeSink::accepting([response]);

        let result = transfer_native_prefix(&mut history, &mut sink, 8, 2);
        assert!(result.is_err() || result.unwrap().inserted == 0);
        assert_eq!(history.native, before);
        assert_eq!(history.units.len(), 1);
    }
}

#[test]
fn top_padding_is_transferred_once_and_bottom_padding_is_resident() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::vertical(1), 0));
    history.push("A").unwrap();
    let mut sink = FakeSink::default();

    let first = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(first.inserted, 1);
    assert_eq!(sink.rows.len(), 1);
    assert_eq!(sink.rows[0].width(), 8);
    assert_eq!(history.units.len(), 1);

    let second = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(second.inserted, 1);
    assert!(history.units.is_empty());
    assert_eq!(sink.rows.len(), 2);

    history.push("B").unwrap();
    assert_eq!(transfer(&mut history, &mut sink, 8, 8).inserted, 1);
    assert_eq!(sink.rows.len(), 3);
}

#[test]
fn top_padding_partial_ack_freezes_exact_remainder_across_layout_change() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::new(3, 0, 0, 0), 0));
    history.push("A").unwrap();
    let mut first = FakeSink::accepting([Ok(1)]);

    transfer(&mut history, &mut first, 8, 8);
    let frozen = match history.native.top_padding {
        crate::history::native::frontier::SpacingTransferState::Frozen(ref rows) => rows.clone(),
        _ => panic!("top padding should be frozen after partial ACK"),
    };
    assert_eq!(frozen.as_slice().len(), 2);

    history.set_layout(HistoryLayout::from_parts(Insets::new(10, 0, 0, 0), 0));
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 8, 8);
    assert_eq!(second.rows, frozen.as_slice());

    let mut third = FakeSink::default();
    transfer(&mut history, &mut third, 8, 8);
    assert_eq!(
        third
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A"]
    );
}

#[test]
fn zero_height_static_retires_without_native_row() {
    let mut history = History::new();
    let zero = history.push(View::spacer(0)).unwrap();
    let successor = history.push("B").unwrap();
    let mut sink = FakeSink::accepting([Ok(0)]);

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(sink.calls, 1);
    assert!(sink.rows.is_empty());
    assert_eq!(history.native.last_native_unit, Some(zero));
    assert_eq!(history.units.front().unwrap().id, successor);
    assert_eq!(outcome.status, NativeTransferStatus::SinkBlocked);

    let mut successor_sink = FakeSink::default();
    transfer(&mut history, &mut successor_sink, 8, 8);
    assert_eq!(successor_sink.rows[0].plain_text(), "B");
}

#[test]
fn live_blocks_later_static_units() {
    let mut registry = crate::component::ComponentRegistry::new();
    let live = registry.register(LiveComponent);
    let mut history = History::new();
    history.push("A").unwrap();
    let live_unit = history.push(View::component(live)).unwrap();
    history.push("C").unwrap();
    let mut sink = FakeSink::default();

    assert_eq!(transfer(&mut history, &mut sink, 8, 8).inserted, 1);
    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert!(matches!(
        outcome.status,
        NativeTransferStatus::SemanticBlocked {
            unit,
            reason: NativeBlockReason::Live,
        } if unit == live_unit
    ));
    assert_eq!(history.units.len(), 2);
    let _ = registry;
}

#[test]
fn multiple_live_units_enforce_one_global_native_frontier() {
    let mut registry = crate::component::ComponentRegistry::new();
    let live_b = registry.register(LiveComponent);
    let live_d = registry.register(LiveComponent);
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 1));
    history.push("A").unwrap();
    let b = history.push(View::component(live_b)).unwrap();
    history.push("C").unwrap();
    let d = history.push(View::component(live_d)).unwrap();
    history.push("E").unwrap();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    transfer(&mut history, &mut sink, 8, 8);
    let projection = project(&history, &registry, Size::new(8, 5)).unwrap();
    assert!(projection.scene.mounts.ids().any(|id| id == live_b.id()));
    assert!(projection.scene.mounts.ids().any(|id| id == live_d.id()));

    let blocked_b = transfer(&mut history, &mut sink, 8, 8);
    assert!(matches!(
        blocked_b.status,
        NativeTransferStatus::SemanticBlocked {
            unit,
            reason: NativeBlockReason::Live,
        } if unit == b
    ));
    assert_eq!(history.units.front().unwrap().id, b);

    history.freeze(b, "B").unwrap();
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert!(projection.scene.mounts.ids().any(|id| id == live_d.id()));
    transfer(&mut history, &mut sink, 8, 8);
    transfer(&mut history, &mut sink, 8, 8);
    transfer(&mut history, &mut sink, 8, 8);
    transfer(&mut history, &mut sink, 8, 8);
    let blocked_d = transfer(&mut history, &mut sink, 8, 8);
    assert!(matches!(
        blocked_d.status,
        NativeTransferStatus::SemanticBlocked {
            unit,
            reason: NativeBlockReason::Live,
        } if unit == d
    ));
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "", "B", "", "C", ""]
    );
}

#[test]
fn leading_gap_partial_ack_freezes_spacing_across_layout_change() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 3));
    history.push("A").unwrap();
    history.push("B").unwrap();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    let mut partial = FakeSink::accepting([Ok(1)]);
    assert_eq!(transfer(&mut history, &mut partial, 8, 8).inserted, 1);
    assert!(matches!(
        history.native.leading_gap,
        Some(crate::history::native::frontier::SpacingTransferState::Frozen(_))
    ));
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 8));
    let Some(crate::history::native::frontier::SpacingTransferState::Frozen(rows)) =
        history.native.leading_gap.as_ref()
    else {
        panic!("expected frozen gap")
    };
    assert_eq!(rows.as_slice().len(), 2);
}

#[test]
fn zero_top_padding_is_crossed_before_partial_static_content() {
    let mut history = History::new();
    history.push("A0\nA1").unwrap();
    let mut first = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut first, 8, 8);
    assert!(matches!(
        history.native.top_padding,
        crate::history::native::frontier::SpacingTransferState::Native
    ));

    history.set_layout(HistoryLayout::from_parts(Insets::new(3, 0, 0, 0), 0));
    let registry = crate::component::ComponentRegistry::new();
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert!(projection.frozen_overlay.is_some());
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 8, 8);
    assert_eq!(
        second
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A1"]
    );
}

#[test]
fn zero_top_padding_remains_crossed_after_full_transfer_and_append() {
    let mut history = History::new();
    history.push("A").unwrap();
    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 8, 8);
    history.set_layout(HistoryLayout::from_parts(Insets::new(3, 0, 0, 0), 0));
    history.push("B").unwrap();

    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
}

#[test]
fn zero_default_gap_is_crossed_before_partial_static_content() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B0\nB1").unwrap();
    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 8, 8);
    let mut partial = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut partial, 8, 8);
    assert!(matches!(
        history.native.leading_gap,
        Some(crate::history::native::frontier::SpacingTransferState::Native)
    ));

    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 3));
    let registry = crate::component::ComponentRegistry::new();
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert!(projection.frozen_overlay.is_some());
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 8, 8);
    assert_eq!(
        second
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["B1"]
    );
}

#[test]
fn zero_default_gap_is_crossed_before_partial_stream_content() {
    let mut history = History::new();
    history.push("A").unwrap();
    history
        .push_stream(NativeSource::new("B0\nB1", 5, true))
        .unwrap();
    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 8, 8);
    let mut partial = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut partial, 8, 8);
    assert!(matches!(
        history.native.leading_gap,
        Some(crate::history::native::frontier::SpacingTransferState::Native)
    ));

    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 3));
    let registry = crate::component::ComponentRegistry::new();
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    let rows =
        crate::presentation::layout::compile_bounded_view(&projection.scene.view, Size::new(8, 1))
            .rows;
    assert_eq!(rows[0].plain_text(), "B1");
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 8, 8);
    assert_eq!(
        second
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["B1"]
    );
}

#[test]
fn stream_checkpoint_ack_advances_only_after_sink_ack() {
    let mut history = History::new();
    let handle = history
        .push_stream(NativeSource::new("A\nB", 3, true))
        .unwrap();
    let mut sink = FakeSink::accepting([Ok(1)]);

    let outcome = transfer(&mut history, &mut sink, 8, 1);
    assert_eq!(outcome.inserted, 1);
    let state = history.native.stream.as_ref().unwrap();
    assert!(state.committed_through > StreamOffset::ZERO);
    assert_eq!(state.unit, handle.unit());
    assert_eq!(history.units.front().unwrap().id, handle.unit());
    let registry = crate::component::ComponentRegistry::new();
    let rows = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 1))
            .unwrap()
            .scene
            .view,
        Size::new(8, 1),
    )
    .rows;
    assert_eq!(rows[0].plain_text(), "B");
}

#[test]
fn partially_native_open_stream_grows_seals_and_blocks_successor_until_retired() {
    let source = GrowingSource::new();
    let mut history = History::new();
    let handle = history.push_stream(source).unwrap();
    let mut sink = FakeSink::accepting([Ok(1), Ok(1), Ok(1)]);

    transfer(&mut history, &mut sink, 8, 1);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A"]
    );
    let registry = crate::component::ComponentRegistry::new();
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert_eq!(
        crate::presentation::layout::compile_bounded_view(&projection.scene.view, Size::new(8, 1))
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["B"]
    );
    assert_eq!(history.units.front().unwrap().id, handle.unit());

    history.update_stream(handle, GrowingSource::grow).unwrap();
    let projection = project(&history, &registry, Size::new(8, 2)).unwrap();
    assert_eq!(
        crate::presentation::layout::compile_bounded_view(&projection.scene.view, Size::new(8, 2))
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["B", "C"]
    );

    transfer(&mut history, &mut sink, 8, 1);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let projection = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert_eq!(
        crate::presentation::layout::compile_bounded_view(&projection.scene.view, Size::new(8, 1))
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["C"]
    );
    let mut probe = FakeSink::accepting([Ok(0)]);
    let outcome = transfer(&mut history, &mut probe, 8, 8);
    assert_eq!(outcome.requested, 1);
    assert_eq!(outcome.inserted, 0);
    assert_eq!(outcome.status, NativeTransferStatus::SinkBlocked);

    history.seal_stream(handle).unwrap();
    history.push("D").unwrap();
    let before_c = history.units.front().unwrap().id;
    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(history.native.last_native_unit, Some(before_c));
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B", "C", "D"]
    );
}

#[test]
fn sequential_streams_transfer_in_history_order_with_unit_local_offsets() {
    let mut history = History::new();
    let _stream_a = history
        .push_stream(NativeSource::at(50, "A", 51, true))
        .unwrap();
    history.push("B").unwrap();
    let _stream_c = history
        .push_stream(NativeSource::at(1_000, "C", 1_001, true))
        .unwrap();
    let d = history.push("D").unwrap();
    let stream_e = history
        .push_stream(NativeSource::at(9_000, "E", 9_001, false))
        .unwrap();
    let mut sink = FakeSink::default();

    let mut blocked = None;
    for _ in 0..20 {
        let outcome = transfer(&mut history, &mut sink, 8, 8);
        if matches!(outcome.status, NativeTransferStatus::SemanticBlocked { .. }) {
            blocked = Some(outcome.status);
            break;
        }
    }

    assert!(matches!(
        blocked,
        Some(NativeTransferStatus::SemanticBlocked {
            unit,
            reason: NativeBlockReason::StreamBlocked,
        }) if unit == stream_e.unit()
    ));
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B", "C", "D", "E"]
    );
    assert_eq!(history.units.len(), 1);
    assert_eq!(history.units.front().unwrap().id, stream_e.unit());
    assert_eq!(history.native.last_native_unit, Some(d));
    assert_eq!(
        history.native.stream.as_ref().unwrap().committed_through,
        StreamOffset::new(9_001)
    );
}

#[test]
fn atomic_partial_ack_freezes_and_drains_exact_physical_rows() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(3)),
        View::vertical(|column| {
            column.children(["A0", "A1", "A2"]);
        }),
    )
    .unwrap()
    .exact_text(
        StreamRange::new(StreamOffset::new(3), StreamOffset::new(4)),
        [TextSpan::plain("Z")],
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut first = FakeSink::accepting([Ok(1)]);
    assert_eq!(transfer(&mut history, &mut first, 8, 1).inserted, 1);
    assert!(matches!(
        history.native.stream.as_ref().unwrap().partial,
        Some(crate::stream::StreamPartialTransfer::FrozenAtomic {
            committed_rows: 1,
            ..
        })
    ));

    let mut second = FakeSink::default();
    assert_eq!(transfer(&mut history, &mut second, 8, 8).inserted, 2);
    assert!(history.native.stream.as_ref().unwrap().partial.is_none());
    assert!(history.native.stream.as_ref().unwrap().committed_through >= StreamOffset::new(3));
}

#[test]
fn frozen_atomic_remainder_survives_resize_and_layout_change_without_reflow() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(3)),
        View::vertical(|column| {
            column.children(["A0", "A1", "A2"]);
        }),
    )
    .unwrap()
    .finish()
    .unwrap();
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(1), 0));
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut first = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut first, 8, 8);
    let frozen = match &history.native.stream.as_ref().unwrap().partial {
        Some(crate::stream::StreamPartialTransfer::FrozenAtomic { rows, .. }) => rows.clone(),
        _ => panic!("expected FrozenAtomic remainder"),
    };
    assert_eq!(frozen.as_slice().len(), 3);

    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    assert!(matches!(
        &history.native.stream.as_ref().unwrap().partial,
        Some(crate::stream::StreamPartialTransfer::FrozenAtomic { rows, .. })
            if rows == &frozen
    ));
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 12, 8);
    assert_eq!(second.rows, frozen.as_slice()[1..]);
    assert!(history.units.is_empty());
}

#[test]
fn stable_zero_height_atomic_advances_without_a_fake_row() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(2),
        StreamOffset::new(2),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
        View::spacer(0),
    )
    .unwrap()
    .exact_text(
        StreamRange::new(StreamOffset::new(1), StreamOffset::new(2)),
        [TextSpan::plain("Z")],
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 1);
    assert_eq!(sink.rows[0].plain_text(), "Z");
    assert!(history.units.is_empty());
}

#[test]
fn unstable_zero_height_atomic_blocks_later_rows() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(2),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
        View::spacer(0),
    )
    .unwrap()
    .exact_text(
        StreamRange::new(StreamOffset::new(1), StreamOffset::new(2)),
        [TextSpan::plain("Z")],
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    let handle = history
        .push_stream(NativeSource::from_snapshot(snapshot, false))
        .unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert!(matches!(
        outcome.status,
        NativeTransferStatus::SemanticBlocked {
            unit,
            reason: NativeBlockReason::StreamBlocked,
        } if unit == handle.unit()
    ));
    assert!(sink.rows.is_empty());
    assert_eq!(history.units.len(), 1);
}

#[test]
fn sealed_empty_stream_retires_without_a_sink_write() {
    let mut history = History::new();
    history.push_stream(NativeSource::new("", 0, true)).unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 0);
    assert!(sink.rows.is_empty());
    assert!(history.units.is_empty());
}

#[test]
fn sealed_empty_stream_with_nonzero_base_retires() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::new(50),
        StreamOffset::new(50),
        StreamOffset::new(50),
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    assert!(history.units.is_empty());
    assert!(sink.rows.is_empty());
}

#[test]
fn stream_with_nonzero_base_transfers_from_its_semantic_origin() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::new(50),
        StreamOffset::new(52),
        StreamOffset::new(52),
    )
    .exact_text(
        StreamRange::new(StreamOffset::new(50), StreamOffset::new(52)),
        [TextSpan::plain("AB")],
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["AB"]
    );
    assert!(history.units.is_empty());
}

#[test]
fn open_empty_stream_with_nonzero_base_remains_resident() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::new(50),
        StreamOffset::new(50),
        StreamOffset::new(50),
    )
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, false))
        .unwrap();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(history.units.len(), 1);
}

#[test]
fn compacted_non_send_stream_transfers_only_the_uncommitted_suffix() {
    let source = CompactingSource::new();
    let compact_calls = source.compact_calls.clone();
    let mut history = History::new();
    let handle = history.push_stream(source).unwrap();
    history.refresh_stream(handle).unwrap();
    assert_eq!(compact_calls.get(), 1);

    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 8, 8);
    history
        .update_stream(handle, CompactingSource::stabilize)
        .unwrap();
    transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    assert_eq!(compact_calls.get(), 2);
}

#[test]
fn open_empty_stream_remains_resident() {
    let mut history = History::new();
    history
        .push_stream(NativeSource::new("", 0, false))
        .unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert!(matches!(
        outcome.status,
        NativeTransferStatus::SemanticBlocked {
            reason: NativeBlockReason::StreamBlocked,
            ..
        }
    ));
    assert_eq!(history.units.len(), 1);
}

#[test]
fn native_predecessor_keeps_default_gap_in_resident_projection() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 1));
    history.push("A").unwrap();
    history.push("B").unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 8, 8);
    let rows = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 2))
            .unwrap()
            .scene
            .view,
        Size::new(8, 2),
    )
    .rows;
    assert_eq!(
        rows.iter().map(PhysicalRow::plain_text).collect::<Vec<_>>(),
        ["", "B"]
    );

    transfer(&mut history, &mut sink, 8, 8);
    let rows = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 2))
            .unwrap()
            .scene
            .view,
        Size::new(8, 2),
    )
    .rows;
    assert_eq!(
        rows.iter().map(PhysicalRow::plain_text).collect::<Vec<_>>(),
        ["B", ""]
    );
}

#[test]
fn attach_to_previous_does_not_create_frontier_gap() {
    let mut history = History::new();
    history.push("A").unwrap();
    history
        .push_with_boundary("B", FlowBoundary::AttachToPrevious)
        .unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 8, 8);

    let rows = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 1))
            .unwrap()
            .scene
            .view,
        Size::new(8, 1),
    )
    .rows;
    assert_eq!(
        rows.iter().map(PhysicalRow::plain_text).collect::<Vec<_>>(),
        ["B"]
    );
}

#[test]
fn frozen_static_remainder_is_exposed_as_a_physical_overlay() {
    let mut history = History::new();
    history.push("A\nB\nC").unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let mut sink = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut sink, 8, 3);

    let projection = project(&history, &registry, Size::new(8, 2)).unwrap();
    let overlay = projection
        .frozen_overlay
        .expect("frozen remainder should be overlaid");
    assert_eq!(
        overlay
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["B", "C"]
    );
}

#[test]
fn partial_static_rows_conserve_and_survive_resize_and_layout_change() {
    let mut history = History::new();
    history.push("A\nB\nC").unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let expected = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 3))
            .unwrap()
            .scene
            .view,
        Size::new(8, 3),
    )
    .rows;
    let mut sink = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut sink, 8, 8);
    let projection = project(&history, &registry, Size::new(8, 3)).unwrap();
    assert_eq!(sink.rows, expected[..1]);
    assert_eq!(projection.frozen_overlay.unwrap().rows, expected[1..]);

    let frozen = history.native.frozen_static.as_ref().unwrap().rows.clone();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    let mut remainder_sink = FakeSink::default();
    transfer(&mut history, &mut remainder_sink, 10, 8);
    assert_eq!(remainder_sink.rows, frozen.as_slice());
}

#[test]
fn partial_atomic_rows_conserve_exact_physical_presentation() {
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(3)),
        View::vertical(|column| {
            column.children(["A", "B", "C"]);
        }),
    )
    .unwrap()
    .finish()
    .unwrap();
    let mut history = History::new();
    history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let expected = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(8, 3))
            .unwrap()
            .scene
            .view,
        Size::new(8, 3),
    )
    .rows;
    let mut sink = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut sink, 8, 8);
    let projection = project(&history, &registry, Size::new(8, 3)).unwrap();
    assert_eq!(sink.rows, expected[..1]);
    assert_eq!(projection.frozen_overlay.unwrap().rows, expected[1..]);
}

#[test]
fn static_physical_rows_match_bounded_view_rows() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    history.push("wide").unwrap();
    let mut sink = FakeSink::default();
    transfer(&mut history, &mut sink, 10, 4);

    let expected = crate::presentation::layout::compile_bounded_view(
        &View::text("wide").into_view(),
        Size::new(6, 1),
    )
    .rows[0]
        .clone();
    assert_eq!(sink.rows[0].width(), 10);
    assert_eq!(sink.rows[0].cell(2), expected.cell(0));
}

#[test]
fn styled_wide_static_rows_conserve_exact_physical_cells_across_partial_transfer() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(1), 0));
    history
        .push(
            View::styled_text([TextSpan::styled(
                "界e\u{301}\nZ",
                StyleSpec::new()
                    .foreground(ColorSpec::ansi(1))
                    .background(ColorSpec::ansi(2))
                    .bold(),
            )])
            .into_view(),
        )
        .unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let expected = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(10, 2))
            .unwrap()
            .scene
            .view,
        Size::new(10, 2),
    )
    .rows;
    assert!(expected[0].cells().iter().any(|cell| cell.continuation));
    assert!(
        expected[0]
            .cells()
            .iter()
            .any(|cell| cell.style.foreground.is_some())
    );
    assert!(
        expected[0]
            .cells()
            .iter()
            .any(|cell| cell.style.background.is_some())
    );
    assert!(expected[0].cells().iter().any(|cell| cell.style.bold));

    let mut first = FakeSink::accepting([Ok(1)]);
    transfer(&mut history, &mut first, 10, 8);
    let projection = project(&history, &registry, Size::new(10, 2)).unwrap();
    assert_eq!(first.rows, expected[..1]);
    assert_eq!(projection.frozen_overlay.unwrap().rows, expected[1..]);

    let frozen = history.native.frozen_static.as_ref().unwrap().rows.clone();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    let mut second = FakeSink::default();
    transfer(&mut history, &mut second, 12, 8);
    assert_eq!(second.rows, frozen.as_slice());
}

#[test]
fn stream_physical_rows_match_resident_rows_with_horizontal_padding() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    history
        .push_stream(NativeSource::new("abcdefghijkl", 12, true))
        .unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let expected = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(10, 2))
            .unwrap()
            .scene
            .view,
        Size::new(10, 2),
    )
    .rows;
    let mut sink = FakeSink::accepting([Ok(1)]);

    transfer(&mut history, &mut sink, 10, 8);
    assert_eq!(sink.rows, expected[..1]);
}

#[test]
fn stream_and_projection_agree_when_padding_leaves_no_content_width() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::horizontal(2), 0));
    history
        .push_stream(NativeSource::new("abc", 3, true))
        .unwrap();
    let registry = crate::component::ComponentRegistry::new();
    let expected = crate::presentation::layout::compile_bounded_view(
        &project(&history, &registry, Size::new(3, 3))
            .unwrap()
            .scene
            .view,
        Size::new(3, 3),
    )
    .rows;
    let mut sink = FakeSink::default();

    transfer(&mut history, &mut sink, 3, 8);
    assert_eq!(sink.rows, expected);
}

#[derive(Clone)]
struct RevisableTableSource {
    state: Rc<RefCell<RevisableTableState>>,
}

struct RevisableTableState {
    snapshot: StreamSnapshot,
    sealed: bool,
}

const TABLE_PREFIX: &str = "intro\n";
const RAW_TABLE: &str = "| A | B |\n| 1 | 2 |\n";
const GRID_TABLE: &str = "A    B\n1    2";

fn table_start() -> u64 {
    TABLE_PREFIX.len() as u64
}

fn table_end() -> u64 {
    table_start() + RAW_TABLE.len() as u64
}

fn table_snapshot(revision: u64, closed: bool) -> StreamSnapshot {
    let start = StreamOffset::new(table_start());
    let end = StreamOffset::new(table_end());
    let stable = if closed {
        end
    } else {
        StreamOffset::new(table_start())
    };
    let table = if closed {
        View::text(GRID_TABLE).into_view()
    } else {
        View::text(RAW_TABLE).into_view()
    };
    StreamSnapshotBuilder::new(
        StreamRevision::new(revision),
        StreamOffset::ZERO,
        stable,
        end,
    )
    .exact_text(
        StreamRange::new(StreamOffset::ZERO, start),
        [TextSpan::plain(TABLE_PREFIX)],
    )
    .atomic(StreamRange::new(start, end), table)
    .unwrap()
    .finish()
    .unwrap()
}

impl RevisableTableSource {
    fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(RevisableTableState {
                snapshot: table_snapshot(0, false),
                sealed: false,
            })),
        }
    }

    fn close_table(&mut self) {
        let mut state = self.state.borrow_mut();
        state.snapshot = table_snapshot(state.snapshot.revision.as_u64() + 1, true);
    }
}

impl StreamingSource for RevisableTableSource {
    fn snapshot(&self) -> StreamSnapshot {
        self.state.borrow().snapshot.clone()
    }

    fn seal(&mut self) {
        let mut state = self.state.borrow_mut();
        state.sealed = true;
        state.snapshot.stable_through = state.snapshot.source_end;
        state.snapshot.revision = state.snapshot.revision.next();
    }

    fn is_sealed(&self) -> bool {
        self.state.borrow().sealed
    }
}

#[test]
fn native_history_does_not_transfer_a_range_that_can_still_change_shape() {
    let mut history = History::new();
    let handle = history.push_stream(RevisableTableSource::new()).unwrap();
    let mut sink = TestSink::default();
    transfer_native_prefix(&mut history, &mut sink, 40, 40).unwrap();

    let committed = history
        .native
        .stream
        .as_ref()
        .map(|state| state.committed_through)
        .unwrap_or(StreamOffset::ZERO);
    assert!(
        committed.as_u64() <= table_start(),
        "open table source {committed:?} must not enter native History"
    );
    assert!(
        sink.rows
            .iter()
            .all(|row| !row.plain_text().contains("A    B")),
        "Grid shape must not appear before the table range is stable"
    );
    let frozen_prefix = sink.rows.clone();

    history
        .update_stream(handle, |source| source.close_table())
        .unwrap();
    transfer_native_prefix(&mut history, &mut sink, 40, 40).unwrap();

    assert_eq!(
        &sink.rows[..frozen_prefix.len()],
        frozen_prefix.as_slice(),
        "already-transferred native rows must not change when the suffix is reinterpreted"
    );
    assert!(
        sink.rows
            .iter()
            .any(|row| row.plain_text().contains("A    B")),
        "once stable, the table range may transfer as its final Grid shape"
    );
}

#[test]
fn stable_no_wrap_code_does_not_pin_following_stream_text() {
    let long_code = "this_is_a_ridiculously_long_function_call_that_must_not_wrap();";
    let code_len = long_code.len() as u64;
    let after = "AFTER";
    let after_len = after.len() as u64;
    let total_len = code_len + after_len;

    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(total_len),
        StreamOffset::new(total_len),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(code_len)),
        View::text(long_code).no_wrap().into_view(),
    )
    .unwrap()
    .exact_text(
        StreamRange::new(StreamOffset::new(code_len), StreamOffset::new(total_len)),
        [TextSpan::plain(after)],
    )
    .finish()
    .unwrap();

    let mut history = History::new();
    let stream_id = history
        .push_stream(NativeSource::from_snapshot(snapshot, true))
        .unwrap()
        .unit();

    let mut sink = FakeSink::default();
    let width = 8;
    let mut iterations = 0;
    while !history.units.is_empty() && iterations < 100 {
        iterations += 1;
        let outcome = transfer_native_prefix(&mut history, &mut sink, width, 1).unwrap();
        if outcome.status == NativeTransferStatus::Idle
            || outcome.status == NativeTransferStatus::SinkBlocked
        {
            break;
        }
    }

    assert!(
        iterations < 100,
        "transfer loop must terminate without pinning"
    );
    assert!(
        history.units.is_empty(),
        "sealed stream unit must retire once fully transferred"
    );
    assert_eq!(
        history.native.last_native_unit,
        Some(stream_id),
        "retired stream unit must be recorded in native frontier"
    );
    assert!(
        sink.rows.len() >= 2,
        "sink must contain both clipped code row and after row, got {:?}",
        sink.rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        sink.rows[0].plain_text(),
        &long_code[..8],
        "first row must be clipped to width {width}"
    );
    assert!(
        sink.rows.iter().any(|r| r.plain_text().contains("AFTER")),
        "sink must contain AFTER"
    );
}

#[test]
fn native_row_ledger_starts_at_zero() {
    let history = History::new();
    assert_eq!(history.native.physical_rows_inserted, 0);
    assert!(!history.native.has_physical_rows());
}

#[test]
fn full_static_ack_increments_native_row_ledger() {
    let mut history = History::new();
    history.push("A\nB\nC").unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 3);
    assert_eq!(history.native.physical_rows_inserted, 3);
    assert!(history.native.has_physical_rows());
}

#[test]
fn partial_ack_counts_only_accepted_rows() {
    let mut history = History::new();
    history.push("A\nB\nC").unwrap();
    let mut sink = FakeSink::accepting([Ok(1), Ok(2)]);

    let first = transfer(&mut history, &mut sink, 8, 3);
    assert_eq!(first.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 1);
    assert!(history.native.has_physical_rows());

    let second = transfer(&mut history, &mut sink, 8, 3);
    assert_eq!(second.inserted, 2);
    assert_eq!(history.native.physical_rows_inserted, 3);
    assert!(history.native.has_physical_rows());
}

#[test]
fn zero_row_semantic_progress_does_not_start_native_history() {
    let mut history = History::new();
    let zero = history.push(View::spacer(0)).unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 0);
    assert_eq!(history.native.last_native_unit, Some(zero));
    assert_eq!(history.native.physical_rows_inserted, 0);
    assert!(!history.native.has_physical_rows());
}

#[test]
fn zero_height_top_padding_crossing_does_not_count_as_physical() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 0));
    let zero = history.push(View::spacer(0)).unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 0);
    assert_eq!(history.native.last_native_unit, Some(zero));
    assert_eq!(
        history.native.top_padding,
        crate::history::native::frontier::SpacingTransferState::Native
    );
    assert_eq!(history.native.physical_rows_inserted, 0);
    assert!(!history.native.has_physical_rows());
}

#[test]
fn invalid_ack_does_not_mutate_native_row_ledger() {
    let mut history = History::new();
    history.push("A").unwrap();
    let before = history.native.clone();
    let result = transfer_native_prefix(&mut history, &mut InvalidAckSink, 8, 8);

    assert_eq!(
        result,
        Err(NativeTransferError::InvalidAcknowledgement {
            requested: 1,
            accepted: 2,
        })
    );
    assert_eq!(history.native, before);
    assert_eq!(history.native.physical_rows_inserted, 0);
    assert!(!history.native.has_physical_rows());
    assert_eq!(history.units.len(), 1);
}

#[test]
fn sink_error_does_not_advance_native_row_ledger() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B").unwrap();
    let mut sink = FakeSink::accepting([Ok(1), Err("sink error")]);

    let first = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(first.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 1);
    assert!(history.native.has_physical_rows());

    let before = history.native.clone();
    let result = transfer_native_prefix(&mut history, &mut sink, 8, 8);
    assert_eq!(result, Err(NativeTransferError::Sink("sink error")));
    assert_eq!(history.native, before);
    assert_eq!(history.native.physical_rows_inserted, 1);
    assert!(history.native.has_physical_rows());
    assert_eq!(history.units.len(), 1);
}

#[test]
fn recursive_zero_height_skips_count_ledger_exactly_once() {
    let mut history = History::new();
    history.push(View::spacer(0)).unwrap();
    history.push(View::spacer(0)).unwrap();
    history.push("A\nB").unwrap();
    let mut sink = FakeSink::default();

    let outcome = transfer(&mut history, &mut sink, 8, 8);
    assert_eq!(outcome.inserted, 2);
    assert_eq!(history.native.physical_rows_inserted, 2);
    assert!(history.native.has_physical_rows());
    assert!(history.units.is_empty());
}
