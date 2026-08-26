//! History → SceneHost → TermwizPresenter → ShadowTerminal fidelity tests.

use anyhow::Result;
use std::{cell::RefCell, rc::Rc};

use crate::{
    History, HistoryStreamHandle, IntoView, TextSpan, Theme, View,
    backend::NativeHistorySink,
    component::ComponentRegistry,
    geometry::Size,
    physical::PhysicalRow,
    scene::{PreparedSceneFrame, Scene, SceneHost},
    stream::{
        StreamOffset, StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder,
        StreamingSource,
    },
    terminal::termwiz::{
        TermwizPresenter, desired_surface,
        shadow::{ShadowRow, ShadowTerminal, physical_style},
    },
};

const WIDTH: u16 = 40;
const HEIGHT: u16 = 12;

const INTRO: &str = "hello stream\n";
const CODE: &str = "this_is_a_ridiculously_long_function_call();";
const EMOJI: &str = "\n🐕‍🦺 AFTER\n";
const RAW_TABLE: &str = "| A | B |\n| 1 | 2 |\n";
const GRID_TABLE: &str = "A    B\n1    2";
const PARAS: &str = "more paragraphs\n";

fn intro_end() -> u64 {
    INTRO.len() as u64
}

fn code_end() -> u64 {
    intro_end() + CODE.len() as u64
}

fn emoji_end() -> u64 {
    code_end() + EMOJI.len() as u64
}

fn table_end() -> u64 {
    emoji_end() + RAW_TABLE.len() as u64
}

fn paras_end() -> u64 {
    table_end() + PARAS.len() as u64
}

struct DifferentialHistoryTerminal {
    presenter: TermwizPresenter,
    terminal: ShadowTerminal,
    promoted_rows: Vec<PhysicalRow>,
    claimed_promoted: usize,
}

impl DifferentialHistoryTerminal {
    fn new() -> Self {
        Self {
            presenter: TermwizPresenter::new(usize::from(WIDTH), usize::from(HEIGHT)),
            terminal: ShadowTerminal::new(usize::from(WIDTH), usize::from(HEIGHT)),
            promoted_rows: Vec::new(),
            claimed_promoted: 0,
        }
    }

    fn present_frame(&mut self, frame: &PreparedSceneFrame) -> Result<()> {
        self.presenter
            .present(&mut self.terminal, desired_surface(frame))?;
        Ok(())
    }

    fn assert_tapes(&mut self, history: &History) {
        assert_eq!(
            self.terminal.implicit_wraps, 0,
            "presenter emitted autowrap"
        );
        self.terminal
            .assert_screen_matches_surface(self.presenter.presented_for_test());
        let _ = self.terminal.capture_char_frame();
        let _ = self.terminal.capture_spans();
        assert_eq!(
            self.terminal.scrollback.len() as u64,
            history.native.physical_rows_inserted,
            "shadow scrollback must match the ACK ledger"
        );
        let promoted: Vec<String> = self
            .promoted_rows
            .iter()
            .map(|row| row.plain_text().trim_end().to_string())
            .collect();
        assert_eq!(
            self.terminal.scrollback_trimmed_texts(),
            promoted,
            "shadow scrollback must match promoted physical rows"
        );
        assert_shadow_rows_match_promoted(&self.terminal.scrollback, &self.promoted_rows);
        let claimed = self.terminal.claim_scrollback();
        let newly_promoted = &self.promoted_rows[self.claimed_promoted..];
        assert_eq!(
            claimed.len(),
            newly_promoted.len(),
            "claimed scrollback must match newly promoted rows"
        );
        self.claimed_promoted = self.promoted_rows.len();
    }
}

fn assert_shadow_rows_match_promoted(scrollback: &[ShadowRow], promoted_rows: &[PhysicalRow]) {
    assert_eq!(
        scrollback.len(),
        promoted_rows.len(),
        "shadow scrollback must have one row for every promoted physical row"
    );

    for (row_index, (shadow, promoted)) in scrollback.iter().zip(promoted_rows).enumerate() {
        assert!(
            promoted.validate_cell_geometry().is_ok(),
            "promoted row {row_index} has invalid cell geometry: {:?}",
            promoted.validate_cell_geometry()
        );
        assert!(
            promoted.width() <= shadow.cells.len(),
            "promoted row {row_index} is wider than its ShadowRow: {} > {}",
            promoted.width(),
            shadow.cells.len()
        );

        let last_painted = promoted.cells().iter().rposition(|cell| cell.painted);
        for (column, cell) in promoted.cells().iter().enumerate() {
            let shadow_cell = &shadow.cells[column];
            let expected_style = if cell.painted {
                physical_style(cell.style)
            } else {
                termwiz::cell::CellAttributes::default()
            };
            assert_eq!(
                shadow_cell.attrs, expected_style,
                "style mismatch at promoted row {row_index}, column {column}"
            );

            if cell.continuation {
                assert!(
                    shadow_cell.continuation,
                    "expected continuation at promoted row {row_index}, column {column}"
                );
                assert_eq!(
                    shadow_cell.grapheme, None,
                    "continuation must not carry a grapheme at promoted row {row_index}, column {column}"
                );
                continue;
            }

            assert!(
                !shadow_cell.continuation,
                "unexpected continuation at promoted row {row_index}, column {column}"
            );
            if last_painted.is_some_and(|last| column <= last) {
                let expected_grapheme = cell.grapheme.as_deref().unwrap_or(" ");
                assert_eq!(
                    shadow_cell.grapheme.as_deref(),
                    Some(expected_grapheme),
                    "grapheme mismatch at promoted row {row_index}, column {column}"
                );
            } else {
                assert_eq!(
                    shadow_cell.grapheme, None,
                    "trailing blank cell must be cleared at promoted row {row_index}, column {column}"
                );
            }
        }
    }
}

impl NativeHistorySink for DifferentialHistoryTerminal {
    type Error = anyhow::Error;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        let accepted = self.presenter.insert_history(&mut self.terminal, rows)?;
        self.promoted_rows.extend_from_slice(&rows[..accepted]);
        Ok(accepted)
    }
}

#[derive(Clone)]
struct StagedStream {
    snapshot: Rc<RefCell<StreamSnapshot>>,
    sealed: Rc<RefCell<bool>>,
}

impl StagedStream {
    fn from_snapshot(snapshot: StreamSnapshot) -> Self {
        Self {
            snapshot: Rc::new(RefCell::new(snapshot)),
            sealed: Rc::new(RefCell::new(false)),
        }
    }

    fn replace(&self, snapshot: StreamSnapshot) {
        *self.snapshot.borrow_mut() = snapshot;
    }
}

impl StreamingSource for StagedStream {
    fn snapshot(&self) -> StreamSnapshot {
        self.snapshot.borrow().clone()
    }

    fn seal(&mut self) {
        *self.sealed.borrow_mut() = true;
        let mut snapshot = self.snapshot.borrow_mut();
        snapshot.revision = StreamRevision::new(snapshot.revision().as_u64() + 1);
        snapshot.stable_through = snapshot.source_end;
    }

    fn is_sealed(&self) -> bool {
        *self.sealed.borrow()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StreamStage {
    Intro,
    Code,
    Emoji,
    OpenTable,
    ClosedTable,
    Paragraphs,
}

fn stream_snapshot(revision: u64, stage: StreamStage) -> StreamSnapshot {
    let source_end = match stage {
        StreamStage::Intro => intro_end(),
        StreamStage::Code => code_end(),
        StreamStage::Emoji => emoji_end(),
        StreamStage::OpenTable | StreamStage::ClosedTable => table_end(),
        StreamStage::Paragraphs => paras_end(),
    };
    let stable_through = match stage {
        StreamStage::OpenTable => emoji_end(),
        _ => source_end,
    };
    let mut builder = StreamSnapshotBuilder::new(
        StreamRevision::new(revision),
        StreamOffset::ZERO,
        StreamOffset::new(stable_through),
        StreamOffset::new(source_end),
    )
    .exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(intro_end())),
        [TextSpan::plain(INTRO)],
    );
    if stage >= StreamStage::Code {
        builder = builder
            .atomic(
                StreamRange::new(
                    StreamOffset::new(intro_end()),
                    StreamOffset::new(code_end()),
                ),
                View::text(CODE).no_wrap().into_view(),
            )
            .unwrap();
    }
    if stage >= StreamStage::Emoji {
        builder = builder.exact_text(
            StreamRange::new(
                StreamOffset::new(code_end()),
                StreamOffset::new(emoji_end()),
            ),
            [TextSpan::plain(EMOJI)],
        );
    }
    if stage >= StreamStage::OpenTable {
        let table = if stage >= StreamStage::ClosedTable {
            View::text(GRID_TABLE).into_view()
        } else {
            View::text(RAW_TABLE).into_view()
        };
        builder = builder
            .atomic(
                StreamRange::new(
                    StreamOffset::new(emoji_end()),
                    StreamOffset::new(table_end()),
                ),
                table,
            )
            .unwrap();
    }
    if stage >= StreamStage::Paragraphs {
        builder = builder.exact_text(
            StreamRange::new(
                StreamOffset::new(table_end()),
                StreamOffset::new(paras_end()),
            ),
            [TextSpan::plain(PARAS)],
        );
    }
    builder.finish().unwrap()
}

fn lines(prefix: &str, count: usize) -> String {
    (1..=count)
        .map(|i| format!("{prefix}{i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn body(rows: usize) -> View {
    View::text(lines("body", rows)).into_view()
}

fn render_step(
    host: &mut SceneHost,
    scene: &mut Scene,
    registry: &mut ComponentRegistry,
    diff: &mut DifferentialHistoryTerminal,
) {
    let frame = host
        .render(scene, registry, &Theme::default(), diff, |_| {
            Ok(Size::new(WIDTH, HEIGHT))
        })
        .unwrap();
    diff.present_frame(&frame).unwrap();
    diff.assert_tapes(scene.history().expect("history"));
}

fn advance_stream(
    scene: &mut Scene,
    handle: HistoryStreamHandle<StagedStream>,
    revision: u64,
    stage: StreamStage,
) {
    scene
        .history_mut()
        .unwrap()
        .update_stream(handle, |source| {
            source.replace(stream_snapshot(revision, stage));
        })
        .unwrap();
}

fn stream_committed_through(history: &History) -> u64 {
    history
        .native
        .stream
        .as_ref()
        .map(|state| state.committed_through.as_u64())
        .unwrap_or(0)
}

#[test]
fn history_presenter_shadow_tape_remains_contiguous() {
    let mut history = History::new();
    history.push(lines("H", 3)).unwrap();
    let mut scene = Scene::with_history(history, body(2));
    let mut registry = ComponentRegistry::new();
    let mut host = SceneHost::default();
    let mut diff = DifferentialHistoryTerminal::new();

    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert_eq!(scene.history().unwrap().native.physical_rows_inserted, 0);

    scene.history_mut().unwrap().push(lines("N", 20)).unwrap();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert!(scene.history().unwrap().native.has_physical_rows());

    scene.set_body(body(6));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    scene.set_body(body(2));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    let source = StagedStream::from_snapshot(stream_snapshot(0, StreamStage::Intro));
    let handle = scene.history_mut().unwrap().push_stream(source).unwrap();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    advance_stream(&mut scene, handle, 1, StreamStage::Code);
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    advance_stream(&mut scene, handle, 2, StreamStage::Emoji);
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    let before_table = stream_committed_through(scene.history().unwrap());
    advance_stream(&mut scene, handle, 3, StreamStage::OpenTable);
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert!(
        stream_committed_through(scene.history().unwrap()) <= emoji_end(),
        "open table must not enter native History"
    );
    assert!(
        before_table <= emoji_end(),
        "native prefix before the table must remain at or before the emoji frontier"
    );
    assert!(
        diff.promoted_rows
            .iter()
            .all(|row| !row.plain_text().contains("A    B")),
        "Grid shape must not appear while the table is still open"
    );

    advance_stream(&mut scene, handle, 4, StreamStage::ClosedTable);
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    advance_stream(&mut scene, handle, 5, StreamStage::Paragraphs);
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    scene.history_mut().unwrap().seal_stream(handle).unwrap();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    scene.set_body(body(6));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    scene.set_body(body(2));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
}

#[test]
fn body_shrink_does_not_create_gap_above_resident_history_on_shadow() {
    let mut history = History::new();
    history.push(lines("R", 16)).unwrap();
    let mut scene = Scene::with_history(history, body(6));
    let mut registry = ComponentRegistry::new();
    let mut host = SceneHost::default();
    let mut diff = DifferentialHistoryTerminal::new();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert!(scene.history().unwrap().native.has_physical_rows());

    scene.set_body(body(2));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    let frame = diff.terminal.capture_char_frame();
    let first_history = frame.lines().next().unwrap_or("");
    assert!(
        first_history.starts_with('R'),
        "first History viewport row should be resident content, got {first_history:?}\n{frame}"
    );
}

#[test]
fn stream_growth_consumes_bottom_slack_before_native_transfer_on_shadow() {
    let mut history = History::new();
    history.push(lines("R", 16)).unwrap();
    let mut scene = Scene::with_history(history, body(6));
    let mut registry = ComponentRegistry::new();
    let mut host = SceneHost::default();
    let mut diff = DifferentialHistoryTerminal::new();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);

    scene.set_body(body(2));
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    let inserted = scene.history().unwrap().native.physical_rows_inserted;

    scene.history_mut().unwrap().push("grow1").unwrap();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert_eq!(
        scene.history().unwrap().native.physical_rows_inserted,
        inserted,
        "bottom slack must absorb a new row before another native insert"
    );
}

#[test]
fn unstable_stream_prefix_cannot_cross_native_history() {
    let text = lines("U", 20);
    let snapshot = StreamSnapshotBuilder::new(
        StreamRevision::ZERO,
        StreamOffset::ZERO,
        StreamOffset::new(50),
        StreamOffset::new(100),
    )
    .atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(100)),
        View::text(text).into_view(),
    )
    .unwrap()
    .finish()
    .unwrap();
    let source = StagedStream::from_snapshot(snapshot);
    let mut history = History::new();
    let handle = history.push_stream(source).unwrap();
    let mut scene = Scene::with_history(history, body(2));
    let mut registry = ComponentRegistry::new();
    let mut host = SceneHost::default();
    let mut diff = DifferentialHistoryTerminal::new();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert_eq!(
        scene.history().unwrap().native.physical_rows_inserted,
        0,
        "unstable Atomic must not transfer even when the View fits"
    );

    scene
        .history_mut()
        .unwrap()
        .update_stream(handle, |source| {
            let mut snapshot = source.snapshot();
            snapshot.revision = StreamRevision::new(1);
            snapshot.stable_through = snapshot.source_end;
            source.replace(snapshot);
        })
        .unwrap();
    render_step(&mut host, &mut scene, &mut registry, &mut diff);
    assert!(scene.history().unwrap().native.has_physical_rows());
}
