use crate::stream::projected::projected_checkpoint_is_legal;

#[test]

fn exact_fragments_share_one_egc_barrier() {
    let text = ProjectedText {
        content_range: StreamRange::new(StreamOffset::ZERO, StreamOffset::new(3)),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![
            ProjectedTextRun {
                display: "e".into(),
                style: StyleSpec::default().into(),
                style_facts: Default::default(),
                owned: StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
                exact_visible: Some(StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1))),
            },
            ProjectedTextRun {
                display: "\u{301}".into(),
                style: StyleSpec::default().into(),
                style_facts: Default::default(),
                owned: StreamRange::new(StreamOffset::new(1), StreamOffset::new(3)),
                exact_visible: Some(StreamRange::new(StreamOffset::new(1), StreamOffset::new(3))),
            },
        ],
    };
    assert!(
        std::panic::catch_unwind(|| {
            StreamView::new(vec![StreamNode::projected_text(text)])
                .suffix_from(StreamOffset::new(1));
        })
        .is_err()
    );
}

use unicode_segmentation::UnicodeSegmentation;

use crate::physical::PhysicalColor;
use crate::physical::PhysicalRow;
use crate::presentation::layout::ViewCompiler;
use crate::presentation::paint::StyleContext;
use crate::presentation::{
    HorizontalAlign, StyleFacts, StyleRef, StyleSpec, TextSpan, ThemeKey, WidthRule, WrapMode,
};
use crate::stream::*;
use crate::{StyleSelector, StyleStateKey, StyleStateValue, Theme, ThemeColor};

fn range(start: u64, end: u64) -> StreamRange {
    StreamRange::new(StreamOffset::new(start), StreamOffset::new(end))
}

fn stateful_theme() -> Theme {
    Theme::new()
        .with_color("accent", ThemeColor::Indexed(2))
        .with_color_variant(
            "accent",
            StyleSelector::state("kind", "warning"),
            ThemeColor::Indexed(1),
        )
}

fn warning_context() -> StyleContext {
    let mut states = crate::presentation::StyleStates::default();
    states.set(
        StyleStateKey::from_static("kind"),
        StyleStateValue::from_static("warning"),
    );
    StyleContext {
        inherited_states: states,
        ..StyleContext::default()
    }
}

#[test]
fn projected_runs_resolve_theme_variants_from_effective_context() {
    let projected = ProjectedText {
        content_range: range(0, 1),
        terminator: ExactTerminator::None,
        width: WidthRule::Fill,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![ProjectedTextRun {
            display: "x".into(),
            style: StyleSpec::new()
                .foreground(crate::ColorSpec::theme("accent"))
                .into(),
            style_facts: Default::default(),
            owned: range(0, 1),
            exact_visible: Some(range(0, 1)),
        }],
    };
    let compiler = ViewCompiler::new(&stateful_theme());
    let (_, rows) = compiler.compile_projected_text_with_metadata_and_context(
        &projected,
        10,
        &warning_context(),
    );
    assert_eq!(
        rows[0].row.cell(0).unwrap().style.foreground,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn projected_hanging_prefix_resolves_theme_variants_from_effective_context() {
    let projected = ProjectedText {
        content_range: range(0, 1),
        terminator: ExactTerminator::None,
        width: WidthRule::Fill,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Hanging {
            body_column: 2,
            prefix: "> ".into(),
            prefix_style: StyleSpec::new()
                .foreground(crate::ColorSpec::theme("accent"))
                .into(),
            prefix_facts: Default::default(),
            prefix_source: range(0, 0),
            show_prefix: true,
        },
        runs: vec![ProjectedTextRun {
            display: "x".into(),
            style: StyleSpec::default().into(),
            style_facts: Default::default(),
            owned: range(0, 1),
            exact_visible: Some(range(0, 1)),
        }],
    };
    let compiler = ViewCompiler::new(&stateful_theme());
    let (_, rows) = compiler.compile_projected_text_with_metadata_and_context(
        &projected,
        10,
        &warning_context(),
    );
    assert_eq!(
        rows[0].row.cell(0).unwrap().style.foreground,
        Some(PhysicalColor::Indexed(1))
    );
}

fn text(row: &PhysicalRow) -> String {
    row.plain_text()
}

fn style(color: &str) -> StyleSpec {
    StyleSpec {
        foreground: Some(crate::ColorSpec::Theme(ThemeKey::from(color))),
        ..StyleSpec::default()
    }
}

fn fact(key: &str, value: &str) -> StyleFacts {
    let mut facts = StyleFacts::default();
    facts.set(key, value);
    facts
}

#[test]
fn identity_preserves_fact_boundaries_when_merging_runs() {
    let separate = ProjectedText::identity_with_terminator(
        range(0, 2),
        ExactTerminator::None,
        [
            TextSpan::plain("a").style_fact("role", "strong"),
            TextSpan::plain("b").style_fact("role", "link"),
        ],
    );
    assert_eq!(separate.runs.len(), 2);

    let merged = ProjectedText::identity_with_terminator(
        range(0, 2),
        ExactTerminator::None,
        [
            TextSpan::plain("a").style_fact("role", "strong"),
            TextSpan::plain("b").style_fact("role", "strong"),
        ],
    );
    assert_eq!(merged.runs.len(), 1);
    assert_eq!(merged.runs[0].style_facts, fact("role", "strong"));
}

#[test]
fn projected_run_facts_resolve_against_theme_variants() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("role", "strong"),
        StyleSpec::new().bold(),
    );
    let projected = ProjectedText::builder(range(0, 1))
        .run_with_facts(
            "x",
            range(0, 1),
            Some(range(0, 1)),
            StyleRef::theme("probe"),
            fact("role", "strong"),
        )
        .finish()
        .expect("valid projected text");
    let compiler = ViewCompiler::new(&theme);
    let (_, rows) = compiler.compile_projected_text_with_metadata(&projected, 1);
    assert!(rows[0].row.cell(0).unwrap().style.bold);
}

#[test]
fn projected_slices_and_suffix_preserve_run_facts_and_provenance() {
    let projected = ProjectedText::builder(range(0, 6))
        .run_with_facts(
            "abcdef",
            range(0, 6),
            Some(range(0, 6)),
            StyleRef::default(),
            fact("test.role", "strong"),
        )
        .finish()
        .expect("valid projected text");

    let prefix =
        crate::stream::projected::slice_projected_text_to(&projected, StreamOffset::new(3))
            .expect("legal prefix slice");
    let suffix = crate::stream::projected::slice_projected_text(&projected, StreamOffset::new(3));
    let (StreamNode::Text(suffix_text) | StreamNode::ContinuousText(suffix_text)) =
        &StreamView::new(vec![StreamNode::projected_text(projected)])
            .suffix_from(StreamOffset::new(3))
            .nodes[0]
    else {
        panic!("expected projected suffix");
    };

    assert_eq!(prefix.runs[0].display, "abc");
    assert_eq!(prefix.runs[0].owned, range(0, 3));
    assert_eq!(prefix.runs[0].exact_visible, Some(range(0, 3)));
    assert_eq!(prefix.runs[0].style_facts, fact("test.role", "strong"));
    assert_eq!(suffix.runs[0].display, "def");
    assert_eq!(suffix.runs[0].owned, range(3, 6));
    assert_eq!(suffix.runs[0].exact_visible, Some(range(3, 6)));
    assert_eq!(suffix.runs[0].style_facts, fact("test.role", "strong"));
    assert_eq!(suffix_text.runs, suffix.runs);
    assert_eq!(suffix_text.content_range, range(3, 6));
}

#[test]
fn hanging_prefix_facts_do_not_leak_into_body() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.part", "marker"),
        StyleSpec::new().bold(),
    );
    let projected = ProjectedText::builder(range(0, 1))
        .run_with_facts(
            "x",
            range(0, 1),
            Some(range(0, 1)),
            StyleRef::theme("probe"),
            StyleFacts::default(),
        )
        .with_hanging(
            ProjectedHanging::new(2, range(0, 0), "> ")
                .with_style(StyleRef::theme("probe"))
                .with_style_facts(fact("test.part", "marker")),
        )
        .finish()
        .expect("valid projected text");

    let (_, rows) = ViewCompiler::new(&theme).compile_projected_text_with_metadata(&projected, 3);
    assert!(rows[0].row.cell(0).unwrap().style.bold);
    assert!(rows[0].row.cell(1).unwrap().style.bold);
    assert!(!rows[0].row.cell(2).unwrap().style.bold);
}

#[test]
fn direct_projected_and_static_lowering_preserve_fact_styling() {
    let theme = Theme::new()
        .with_style_variant(
            "probe",
            StyleSelector::state("test.part", "marker"),
            StyleSpec::new().italic(),
        )
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "strong"),
            StyleSpec::new().bold(),
        )
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "link"),
            StyleSpec::new().underline(),
        );
    let projected = ProjectedText::builder(range(0, 2))
        .run_with_facts(
            "a",
            range(0, 1),
            Some(range(0, 1)),
            StyleRef::theme("probe"),
            fact("test.role", "strong"),
        )
        .run_with_facts(
            "b",
            range(1, 2),
            Some(range(1, 2)),
            StyleRef::theme("probe"),
            fact("test.role", "link"),
        )
        .with_hanging(
            ProjectedHanging::new(2, range(0, 0), "> ")
                .with_style(StyleRef::theme("probe"))
                .with_style_facts(fact("test.part", "marker")),
        )
        .finish()
        .expect("valid projected text");
    let compiler = ViewCompiler::new(&theme);
    let (_, direct_rows) = compiler.compile_projected_text_with_metadata(&projected, 8);
    let static_view =
        StreamView::new(vec![StreamNode::projected_text(projected)]).into_static_view();
    let static_block = compiler.compile(&static_view, 8);

    let direct_rows = direct_rows
        .into_iter()
        .map(|compiled| compiled.row)
        .collect::<Vec<_>>();
    super::assert_rows_equivalent(&direct_rows, &static_block.rows);
}

#[test]
fn cross_run_egc_uses_first_contributor_style_and_facts() {
    let theme = Theme::new()
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "first"),
            StyleSpec::new().bold(),
        )
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "second"),
            StyleSpec::new().italic(),
        );
    let projected = ProjectedText::builder(range(0, 3))
        .run_with_facts(
            "e",
            range(0, 1),
            Some(range(0, 1)),
            StyleRef::theme("probe"),
            fact("test.role", "first"),
        )
        .run_with_facts(
            "\u{301}",
            range(1, 3),
            Some(range(1, 3)),
            StyleRef::theme("probe"),
            fact("test.role", "second"),
        )
        .finish()
        .expect("valid projected text");
    let atoms = crate::stream::projected_atoms(&projected);
    assert_eq!(atoms.len(), 1);
    assert_eq!(atoms[0].display, "e\u{301}");
    assert_eq!(atoms[0].style_facts, fact("test.role", "first"));

    let (_, rows) = ViewCompiler::new(&theme).compile_projected_text_with_metadata(&projected, 2);
    let cell = rows[0].row.cell(0).unwrap();
    assert!(cell.style.bold);
    assert!(!cell.style.italic);
}

fn hanging_text(
    content_range: StreamRange,
    prefix_source: StreamRange,
    show_prefix: bool,
) -> ProjectedText {
    ProjectedText {
        content_range,
        terminator: ExactTerminator::None,
        width: WidthRule::Fill,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Hanging {
            body_column: 2,
            prefix: "- ".to_string(),
            prefix_style: StyleSpec::default().into(),
            prefix_facts: Default::default(),
            prefix_source,
            show_prefix,
        },
        runs: vec![ProjectedTextRun {
            display: "item".to_string(),
            style: StyleSpec::default().into(),
            style_facts: Default::default(),
            owned: StreamRange::new(prefix_source.end, content_range.end),
            exact_visible: Some(StreamRange::new(prefix_source.end, content_range.end)),
        }],
    }
}

fn snapshot(text: ProjectedText) -> StreamSnapshot {
    let range = text.owned_range();
    StreamSnapshot {
        revision: StreamRevision::ZERO,
        source_base: range.start,
        source_end: range.end,
        stable_through: range.end,
        view: StreamView::new(vec![StreamNode::projected_text(text)]),
    }
}

#[test]
fn projected_egc_spans_exact_run_boundaries_and_history_ownership() {
    let compiler = ViewCompiler::default();
    let projected = ProjectedText {
        content_range: range(0, 12),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![
            ProjectedTextRun {
                display: "a".to_string(),
                style: style("bold").into(),
                style_facts: Default::default(),
                owned: range(0, 3),
                exact_visible: Some(range(2, 3)),
            },
            ProjectedTextRun {
                display: "\u{301} rest".to_string(),
                style: style("text.default").into(),
                style_facts: Default::default(),
                owned: range(3, 12),
                exact_visible: Some(range(5, 12)),
            },
        ],
    };
    let (_, rows) = compiler.compile_projected_text_with_metadata(&projected, 1);
    assert_eq!(text(&rows[0].row), "a\u{301}");
    assert_eq!(rows[0].source_end, Some(7));

    let view = StreamView::new(vec![StreamNode::projected_text(projected)]);
    let compiled = crate::stream::compile_stream(&view, 1, crate::stream::StreamOffset::new(12));
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(7))
    );
}

#[test]
fn projected_egc_spans_zwj_run_boundaries_without_splitting() {
    let first = "👩";
    let second = "\u{200d}💻x";
    let split = first.len() as u64;
    let zwj_boundary = split + "\u{200d}".len() as u64;
    let end = split + second.len() as u64;
    let projected = ProjectedText {
        content_range: range(0, end),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![
            ProjectedTextRun {
                display: first.to_string(),
                style: style("bold").into(),
                style_facts: Default::default(),
                owned: range(0, split),
                exact_visible: Some(range(0, split)),
            },
            ProjectedTextRun {
                display: second.to_string(),
                style: style("italic").into(),
                style_facts: Default::default(),
                owned: range(split, end),
                exact_visible: Some(range(split, end)),
            },
        ],
    };
    let (_, rows) = ViewCompiler::default().compile_projected_text_with_metadata(&projected, 20);
    assert_eq!(rows.len(), 1);
    assert_eq!(text(&rows[0].row), format!("{first}{second}"));
    assert!(
        second
            .grapheme_indices(true)
            .any(|(start, _)| start == "\u{200d}".len())
    );
    assert!(!projected_checkpoint_is_legal(
        &projected,
        StreamOffset::new(zwj_boundary)
    ));
    assert!(
        std::panic::catch_unwind(|| {
            StreamView::new(vec![StreamNode::projected_text(projected)])
                .suffix_from(StreamOffset::new(zwj_boundary));
        })
        .is_err()
    );
}

#[test]
fn hanging_initial_and_suffix_nodes_validate_and_compile() {
    let initial = hanging_text(range(0, 6), range(0, 2), true);
    assert!(snapshot(initial.clone()).validate().is_ok());

    let suffix = StreamView::new(vec![StreamNode::projected_text(initial)])
        .suffix_from(StreamOffset::new(2));
    let (StreamNode::Text(suffix_text) | StreamNode::ContinuousText(suffix_text)) =
        &suffix.nodes[0]
    else {
        panic!("expected projected suffix");
    };
    let ProjectedTextLayout::Hanging {
        prefix_source,
        show_prefix,
        ..
    } = &suffix_text.layout
    else {
        panic!("expected hanging suffix");
    };
    assert_eq!(*prefix_source, range(0, 2));
    assert!(!show_prefix);
    assert!(snapshot(suffix_text.clone()).validate().is_ok());

    let compiled = crate::stream::compile_stream(&suffix, 20, StreamOffset::new(6));
    assert_eq!(compiled.rows[0].plain_text(), "  item");
}

#[test]
fn hanging_prefix_that_cannot_fit_does_not_block_stream_transfer() {
    let mut projected = hanging_text(range(0, 6), range(0, 2), true);
    let ProjectedTextLayout::Hanging { body_column, .. } = &mut projected.layout else {
        unreachable!();
    };
    *body_column = 3;
    let view = StreamView::new(vec![StreamNode::projected_text(projected)]);
    let compiled = crate::stream::compile_stream(&view, 3, StreamOffset::new(6));
    assert!(!compiled.rows.is_empty());
    assert_eq!(compiled.transferable_prefix_rows, compiled.rows.len());
    assert!(
        compiled
            .rows
            .iter()
            .all(|row| matches!(row.transfer, StreamRowTransfer::Checkpoint(_)))
    );
}

#[test]
fn hanging_suffix_inside_prefix_source_is_rejected() {
    let text = hanging_text(range(0, 6), range(0, 2), true);
    let result = std::panic::catch_unwind(|| {
        StreamView::new(vec![StreamNode::projected_text(text)]).suffix_from(StreamOffset::new(1));
    });
    assert!(result.is_err());
}

#[test]
fn list_continuation_empty_prefix_at_content_start_validates() {
    let text = hanging_text(range(4, 8), range(4, 4), false);
    assert!(snapshot(text).validate().is_ok());
}

#[test]
fn hanging_width_diagnostic_is_reachable_and_structural_errors_are_distinct() {
    let mut text = hanging_text(range(0, 6), range(0, 2), true);
    let ProjectedTextLayout::Hanging { body_column, .. } = &mut text.layout else {
        unreachable!();
    };
    *body_column = 3;
    assert_eq!(
        snapshot(text).validate(),
        Err(StreamValidationError::Projected(
            ProjectedValidationError::HangingWidthMismatch
        ))
    );
}

#[test]
fn projected_replacement_remains_one_indivisible_atom() {
    let projected = ProjectedText {
        content_range: range(0, 9),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![
            ProjectedTextRun {
                display: "foo".to_string(),
                style: StyleSpec::default().into(),
                style_facts: Default::default(),
                owned: range(0, 3),
                exact_visible: Some(range(0, 3)),
            },
            ProjectedTextRun {
                display: "    ".to_string(),
                style: StyleSpec::default().into(),
                style_facts: Default::default(),
                owned: range(3, 6),
                exact_visible: None,
            },
            ProjectedTextRun {
                display: "bar".to_string(),
                style: StyleSpec::default().into(),
                style_facts: Default::default(),
                owned: range(6, 9),
                exact_visible: Some(range(6, 9)),
            },
        ],
    };
    let (_, rows) = ViewCompiler::default().compile_projected_text_with_metadata(&projected, 1);
    assert!(rows.iter().any(|row| text(&row.row) == "    "));
    assert!(
        std::panic::catch_unwind(|| {
            StreamView::new(vec![StreamNode::projected_text(projected)])
                .suffix_from(StreamOffset::new(4));
        })
        .is_err()
    );
}

#[test]
fn projected_egc_boundaries_still_expose_independent_checkpoints() {
    let projected = ProjectedText {
        content_range: range(0, 2),
        terminator: ExactTerminator::None,
        width: WidthRule::Fit,
        wrap: WrapMode::WordThenGrapheme,
        align: HorizontalAlign::Start,
        layout: ProjectedTextLayout::Plain,
        runs: vec![
            ProjectedTextRun {
                display: "a".to_string(),
                style: style("bold").into(),
                style_facts: Default::default(),
                owned: range(0, 1),
                exact_visible: Some(range(0, 1)),
            },
            ProjectedTextRun {
                display: "b".to_string(),
                style: style("italic").into(),
                style_facts: Default::default(),
                owned: range(1, 2),
                exact_visible: Some(range(1, 2)),
            },
        ],
    };
    let compiled = crate::stream::compile_stream(
        &StreamView::new(vec![StreamNode::projected_text(projected)]),
        1,
        crate::stream::StreamOffset::new(2),
    );
    assert_eq!(
        compiled.rows[0].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(1))
    );
    assert_eq!(
        compiled.rows[1].transfer,
        StreamRowTransfer::Checkpoint(StreamOffset::new(2))
    );
}
