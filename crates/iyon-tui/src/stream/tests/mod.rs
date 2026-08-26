use crate::physical::PhysicalRow;
use crate::presentation::WidthRule;
use crate::presentation::layout::ViewCompiler;

mod append;
mod compile;
mod coord;
mod model;
mod projected;
mod reindex;

mod snapshot;
#[cfg(test)]
mod temporal;
use crate::stream::append::append_only_text_stable_frontier;
use crate::stream::*;
use crate::{ColorSpec, HorizontalAlign, IntoView, StyleSpec, TextSpan, ThemeKey, View, WrapMode};

fn assert_rows_equivalent(left: &[PhysicalRow], right: &[PhysicalRow]) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.plain_text().trim_end(), right.plain_text().trim_end());
        let left_text = left
            .cells()
            .iter()
            .filter_map(|cell| {
                cell.grapheme
                    .as_ref()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| (text, cell.style))
            })
            .collect::<Vec<_>>();
        let right_text = right
            .cells()
            .iter()
            .filter_map(|cell| {
                cell.grapheme
                    .as_ref()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| (text, cell.style))
            })
            .collect::<Vec<_>>();
        assert_eq!(left_text, right_text);
    }
}

#[test]
fn stream_offset_ordering() {
    let o0 = StreamOffset::new(0);
    let o1 = StreamOffset::new(10);
    let o2 = StreamOffset::new(20);
    assert!(o0 < o1);
    assert!(o1 < o2);
    assert_eq!(o0.saturating_add(10), o1);
}

#[test]
fn stream_range_helpers() {
    let r = StreamRange::new(StreamOffset::new(5), StreamOffset::new(15));
    assert_eq!(r.len(), 10);
    assert!(!r.is_empty());
    assert!(r.contains_offset(StreamOffset::new(5)));
    assert!(r.contains_offset(StreamOffset::new(14)));
    assert!(!r.contains_offset(StreamOffset::new(15)));
    assert!(!r.contains_offset(StreamOffset::new(4)));
}

#[test]
fn snapshot_validation_rejects_invalid_visible_source_length() {
    let node = StreamNode::projected_text(ProjectedText {
        content_range: StreamRange::new(StreamOffset::new(0), StreamOffset::new(100)),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![ProjectedTextRun {
            display: "abc".to_string(),
            style: StyleSpec::default().into(),
            style_facts: Default::default(),
            owned: StreamRange::new(StreamOffset::new(0), StreamOffset::new(100)),
            exact_visible: Some(StreamRange::new(
                StreamOffset::new(0),
                StreamOffset::new(100),
            )),
        }],
    });

    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(100),
        stable_through: StreamOffset::new(100),
        view: StreamView::new(vec![node]),
    };

    assert!(snapshot.validate().is_err());
}

#[test]
fn snapshot_validation_normal_exact_and_hard_newline() {
    // Normal Exact: visible = "abc", text_range = 0..3, terminator = None, owned = 0..3
    let node1 = StreamNode::exact_text(
        StreamRange::new(StreamOffset::new(0), StreamOffset::new(3)),
        vec![TextSpan::plain("abc")],
    );
    assert_eq!(
        node1.owned_range(),
        StreamRange::new(StreamOffset::new(0), StreamOffset::new(3))
    );

    // Exact with HardNewline: visible = "def", text_range = 3..6, terminator = HardNewline, owned = 3..7
    let node2 = StreamNode::exact_line(
        StreamRange::new(StreamOffset::new(3), StreamOffset::new(6)),
        vec![TextSpan::plain("def")],
        true,
    );
    assert_eq!(
        node2.owned_range(),
        StreamRange::new(StreamOffset::new(3), StreamOffset::new(7))
    );

    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(7),
        stable_through: StreamOffset::new(7),
        view: StreamView::new(vec![node1, node2]),
    };
    assert!(snapshot.validate().is_ok());
}

#[test]
fn compile_stream_empty_hard_newline_row_produces_one_physical_blank_row() {
    // Empty hard-newline row: visible = "", text_range = 2..2, terminator = HardNewline, owned = 2..3
    let node = StreamNode::exact_line(
        StreamRange::new(StreamOffset::new(2), StreamOffset::new(2)),
        Vec::new(),
        true,
    );
    assert_eq!(
        node.owned_range(),
        StreamRange::new(StreamOffset::new(2), StreamOffset::new(3))
    );

    let view = StreamView::new(vec![node]);
    let compiled = compile_stream(&view, 80, StreamOffset::new(3));
    assert_eq!(compiled.rows.len(), 1);
    assert_eq!(compiled.transferable_prefix_rows, 1);
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(3))
    );
}

#[test]
fn atomic_wide_grapheme_at_width_one_is_committable_when_stable() {
    let view = StreamView::atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new("漢".len() as u64)),
        View::text("漢").into_view(),
    );

    let compiled = compile_stream(&view, 1, StreamOffset::new("漢".len() as u64));
    assert_eq!(compiled.transferable_prefix_rows, 1);
    assert!(matches!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Atomic { .. }
    ));
}

#[test]
fn atomic_wide_grapheme_at_width_two_is_committable() {
    let view = StreamView::atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new("漢".len() as u64)),
        View::text("漢").into_view(),
    );

    let compiled = compile_stream(&view, 2, StreamOffset::new("漢".len() as u64));
    assert_eq!(compiled.transferable_prefix_rows, 1);
}

#[test]
fn atomic_view_uses_the_ordinary_view_style_compiler() {
    let view = View::text("atomic")
        .style(StyleSpec::new().bold())
        .into_view();
    let stream = StreamView::atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(6)),
        view.clone(),
    );
    let compiled = compile_stream(&stream, 20, StreamOffset::new(6));
    let ordinary = ViewCompiler::default().compile(&view, 20);

    assert_eq!(
        compiled
            .rows
            .iter()
            .map(|row| row.physical.clone())
            .collect::<Vec<_>>(),
        ordinary.rows
    );
    assert!(
        compiled.rows[0]
            .physical
            .cell(0)
            .is_some_and(|cell| cell.style.bold)
    );
}

#[test]
fn atomic_final_semantic_view_uses_the_same_ordinary_compiler() {
    let view = View::text("atomic")
        .bold()
        .padding(1)
        .container()
        .background(ColorSpec::ansi(1));
    let stream = StreamView::atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(6)),
        view.clone(),
    );
    let compiled = compile_stream(&stream, 20, StreamOffset::new(6));
    let ordinary = ViewCompiler::default().compile(&view, 20);

    assert_eq!(
        compiled
            .rows
            .iter()
            .map(|row| row.physical.clone())
            .collect::<Vec<_>>(),
        ordinary.rows
    );
}

#[test]
fn atomic_nested_wide_grapheme_propagates_incompleteness() {
    // Atomic(Box(Column(Text("before"), Text("漢")))) at width 1
    let inner = View::column(
        vec![
            View::text("before").into_view(),
            View::text("漢").into_view(),
        ],
        0,
    );
    let boxed = inner.container();
    let compiler = ViewCompiler::default();
    let layout = compiler.compile(&boxed, 1);
    // Physical incompleteness from "漢" must propagate through Column -> Box layout
    assert!(!layout.physically_complete);

    let view = StreamView::atomic(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(10)),
        boxed,
    );

    let compiled = compile_stream(&view, 1, StreamOffset::new(10));
    assert_eq!(compiled.transferable_prefix_rows, compiled.rows.len());
    for row in &compiled.rows {
        assert!(matches!(row.transfer, StreamRowTransfer::Atomic { .. }));
    }
}

#[test]
fn append_only_text_frontier() {
    let text = "hello 🌍";
    let base = StreamOffset::new(10);

    // Open: leaves trailing grapheme cluster out of stable frontier
    let open_frontier = append_only_text_stable_frontier(text, base, false);
    // "hello " is 6 bytes. 🌍 is 4 bytes at offset 6.
    assert_eq!(open_frontier, StreamOffset::new(16));

    // Sealed: covers all bytes
    let sealed_frontier = append_only_text_stable_frontier(text, base, true);
    assert_eq!(sealed_frontier, StreamOffset::new(10 + text.len() as u64));
}

#[test]
fn exact_text_accumulates_source_offsets_across_multiple_spans_and_preserves_styles() {
    let span1 = TextSpan::plain("hello "); // 6 bytes (0..6)
    let span2 = TextSpan::styled("world", StyleSpec::new().bold()); // 5 bytes (6..11)

    let view = StreamView::exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(11)),
        vec![span1, span2],
    );

    let compiled = compile_stream(&view, 20, StreamOffset::new(11));
    assert_eq!(compiled.rows.len(), 1);
    assert_eq!(compiled.transferable_prefix_rows, 1);

    // Physical cells preserve text and bold style on "world".
    let line = &compiled.rows[0];
    assert_eq!(line.plain_text(), "hello world");
    assert!(line.cell(6).is_some_and(|cell| cell.style.bold));

    // Row commit offset is the accumulated exact end (11)
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(11))
    );
}

#[test]
fn projected_checkpoint_never_splits_cross_run_egcs() {
    for parts in [
        vec!["a", "\u{301}"],
        vec!["👩", "\u{200d}", "💻"],
        vec!["🇺", "🇸"],
        vec!["e", "\u{301}"],
    ] {
        let checkpoint = StreamOffset::new(parts[0].len() as u64);
        let mut cursor = 0;
        let runs = parts
            .into_iter()
            .enumerate()
            .map(|(_index, display)| {
                let start = cursor;
                cursor += display.len() as u64;
                ProjectedTextRun {
                    display: display.to_owned(),
                    style: StyleSpec::new().bold().italic().into(),
                    style_facts: Default::default(),
                    owned: StreamRange::new(StreamOffset::new(start), StreamOffset::new(cursor)),
                    exact_visible: Some(StreamRange::new(
                        StreamOffset::new(start),
                        StreamOffset::new(cursor),
                    )),
                }
            })
            .collect::<Vec<_>>();
        let text = ProjectedText {
            content_range: StreamRange::new(StreamOffset::ZERO, StreamOffset::new(cursor)),
            terminator: ExactTerminator::None,
            width: WidthRule::Fit,
            wrap: WrapMode::WordThenGrapheme,
            align: HorizontalAlign::Start,
            layout: ProjectedTextLayout::Plain,
            runs,
        };
        let view = StreamView::new(vec![StreamNode::projected_text(text)]);
        assert!(std::panic::catch_unwind(|| view.suffix_from(checkpoint)).is_err());
    }
}

#[test]
fn exact_text_combining_mark_across_spans() {
    // 'e' (1 byte) + combining acute (2 bytes) = 3 bytes
    let span1 = TextSpan::plain("e");
    let span2 = TextSpan::plain("\u{0301}");

    let view = StreamView::exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(3)),
        vec![span1, span2],
    );

    let compiled = compile_stream(&view, 20, StreamOffset::new(3));
    assert_eq!(compiled.rows.len(), 1);
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(3))
    );
}

#[test]
fn exact_text_zwj_split_across_spans() {
    // 👩 (4 bytes) + ZWJ (3 bytes) + 💻 (4 bytes) = 11 bytes
    let span1 = TextSpan::plain("👩");
    let span2 = TextSpan::plain("\u{200D}");
    let span3 = TextSpan::plain("💻");

    let view = StreamView::exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(11)),
        vec![span1, span2, span3],
    );

    let compiled = compile_stream(&view, 20, StreamOffset::new(11));
    assert_eq!(compiled.rows.len(), 1);
    assert_eq!(compiled.rows[0].width(), 2);
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(11))
    );
}

#[test]
fn annotated_style_transition_preserves_muted_and_italic() {
    let span_annotated = TextSpan::styled(
        "reasoning\n",
        StyleSpec::new()
            .foreground(ColorSpec::Theme(ThemeKey::from("muted")))
            .italic()
            .dim(),
    );
    let span_text = TextSpan::plain("answer");

    let view = StreamView::exact_text(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(16)),
        vec![span_annotated, span_text],
    );

    let compiled = compile_stream(&view, 20, StreamOffset::new(16));
    assert_eq!(compiled.rows.len(), 2);

    // Row 0 has annotated styling (italic & dim)
    assert!(
        compiled.rows[0]
            .cell(0)
            .is_some_and(|cell| cell.style.italic)
    );
    assert!(compiled.rows[0].cell(0).is_some_and(|cell| cell.style.dim));
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(10))
    ); // "reasoning\n"

    // Row 1 is plain text
    assert_eq!(
        compiled.rows[1].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(16))
    ); // "reasoning\nanswer"
}

#[test]
fn compile_stream_exact_text_matches_view_compiler_identically() {
    let compiler = ViewCompiler::default();

    let cases = vec![
        (
            "plain wrapped text",
            vec![TextSpan::plain("hello world from exact stream")],
        ),
        (
            "styled spans",
            vec![
                TextSpan::plain("plain "),
                TextSpan::styled("bold italic", StyleSpec::new().bold().italic()),
            ],
        ),
        (
            "combining mark across spans",
            vec![TextSpan::plain("e"), TextSpan::plain("\u{0301}")],
        ),
        (
            "ZWJ emoji across spans",
            vec![
                TextSpan::plain("👩"),
                TextSpan::plain("\u{200D}"),
                TextSpan::plain("💻"),
            ],
        ),
        (
            "hard newline",
            vec![TextSpan::plain("line 1\nline 2\nline 3")],
        ),
    ];

    for width in [1u16, 2, 3, 10, 80] {
        for (label, spans) in &cases {
            let text_view = View::styled_text(spans.clone()).into_view();
            let layout_block = compiler.compile(&text_view, width);

            let total_len = spans.iter().map(|s| s.text().len() as u64).sum();
            let stream_view = StreamView::exact_text(
                StreamRange::new(StreamOffset::ZERO, StreamOffset::new(total_len)),
                spans.clone(),
            );
            let compiled_stream = compile_stream(&stream_view, width, StreamOffset::new(total_len));

            // If the entire row cannot fit within width (e.g. 2-cell wide emoji in width 1),
            // layout_text correctly clips the wide grapheme while stream compilation retains
            // the physical line marked as fits: false (non-committable).
            if !compiled_stream.rows.is_empty()
                && compiled_stream.rows[0].width() > usize::from(width)
            {
                continue;
            }

            assert_eq!(
                layout_block.rows.len(),
                compiled_stream.rows.len(),
                "Row count mismatch for '{}' at width {}",
                label,
                width
            );

            let physical = compiled_stream
                .rows
                .iter()
                .map(|row| row.physical.clone())
                .collect::<Vec<_>>();
            assert_rows_equivalent(&layout_block.rows, &physical);
        }
    }
}

// --- Fix 3: contiguity validation tests ---

#[test]
fn contiguity_rejects_gap_between_nodes() {
    // node 1 = 0..3, node 2 = 10..13 -- gap at 3..10
    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(13),
        stable_through: StreamOffset::new(13),
        view: StreamView::new(vec![
            StreamNode::exact_text(
                StreamRange::new(StreamOffset::new(0), StreamOffset::new(3)),
                vec![TextSpan::plain("abc")],
            ),
            StreamNode::exact_text(
                StreamRange::new(StreamOffset::new(10), StreamOffset::new(13)),
                vec![TextSpan::plain("def")],
            ),
        ]),
    };
    assert!(snapshot.validate().is_err());
}

#[test]
fn contiguity_rejects_trailing_uncovered_source() {
    // source_end = 10, only node = 0..5
    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(10),
        stable_through: StreamOffset::new(10),
        view: StreamView::new(vec![StreamNode::exact_text(
            StreamRange::new(StreamOffset::new(0), StreamOffset::new(5)),
            vec![TextSpan::plain("hello")],
        )]),
    };
    assert!(snapshot.validate().is_err());
}

#[test]
fn contiguity_rejects_leading_gap() {
    // source_base = 5, first node starts at 6
    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::new(5),
        source_end: StreamOffset::new(9),
        stable_through: StreamOffset::new(9),
        view: StreamView::new(vec![StreamNode::exact_text(
            StreamRange::new(StreamOffset::new(6), StreamOffset::new(9)),
            vec![TextSpan::plain("abc")],
        )]),
    };
    assert!(snapshot.validate().is_err());
}

#[test]
fn contiguity_accepts_typed_newline_chain() {
    // Exact: visible 0..3, HardNewline, owned 0..4
    // Next Exact: visible 4..7, None, owned 4..7
    // source_end = 7
    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(7),
        stable_through: StreamOffset::new(7),
        view: StreamView::new(vec![
            StreamNode::exact_line(
                StreamRange::new(StreamOffset::new(0), StreamOffset::new(3)),
                vec![TextSpan::plain("abc")],
                true,
            ),
            StreamNode::exact_text(
                StreamRange::new(StreamOffset::new(4), StreamOffset::new(7)),
                vec![TextSpan::plain("def")],
            ),
        ]),
    };
    assert!(snapshot.validate().is_ok());
}

#[test]
fn contiguity_accepts_atomic_then_exact() {
    // Atomic 0..9, Exact 9..14, source_end = 14
    let snapshot = StreamSnapshot {
        revision: StreamRevision::new(1),
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(14),
        stable_through: StreamOffset::new(14),
        view: StreamView::new(vec![
            StreamNode::atomic(
                StreamRange::new(StreamOffset::new(0), StreamOffset::new(9)),
                View::text("bold").into_view(),
            ),
            StreamNode::exact_text(
                StreamRange::new(StreamOffset::new(9), StreamOffset::new(14)),
                vec![TextSpan::plain("plain")],
            ),
        ]),
    };
    assert!(snapshot.validate().is_ok());
}

#[test]
fn contiguity_empty_stream_valid_iff_base_equals_end() {
    let valid = StreamSnapshot {
        revision: StreamRevision::ZERO,
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::ZERO,
        stable_through: StreamOffset::ZERO,
        view: StreamView::empty(),
    };
    assert!(valid.validate().is_ok());

    let invalid = StreamSnapshot {
        revision: StreamRevision::ZERO,
        source_base: StreamOffset::ZERO,
        source_end: StreamOffset::new(5),
        stable_through: StreamOffset::new(5),
        view: StreamView::empty(),
    };
    assert!(invalid.validate().is_err());
}
