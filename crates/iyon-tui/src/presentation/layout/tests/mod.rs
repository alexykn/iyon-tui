//! Retained layout and text compiler regressions.

use std::sync::Arc;

use super::*;
use crate::geometry::{LayoutConstraints, Size};
use crate::physical::{PhysicalColor, PhysicalRow, PhysicalStyle};
use crate::presentation::api::style::{
    AnsiColor, BorderEdges, BorderGlyphs, BorderSpec, BorderStyle, OverflowIndicator, TextAttribute,
};
use crate::presentation::ir::ViewKind;
use crate::presentation::ir::{Decoration, RowChild, ViewNodeParts};
use crate::presentation::{
    ColorSpec, EmptyContentProvider, GridCellSpec, GridTrack, HorizontalAlign, Insets, StyleRef,
    StyleSpec, TextSpan, ThemeKey, VerticalAlign, View, WidthRule, WrapMode,
};
use crate::{StyleSelector, Theme};

fn text(row: &PhysicalRow) -> String {
    row.plain_text()
}

fn layout_view(view: &View, width: u16, inherited: PhysicalStyle) -> Surface {
    let compiler = ViewCompiler::default();
    let tree = compiler.layout_tree(view, LayoutConstraints::width_only(width));
    ViewPainter.paint_tree_with_style(&compiler, &tree, inherited)
}

fn row_view(children: Vec<RowChild>, gap: u16) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fill,
        height: crate::presentation::ir::HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: Default::default(),
        style_facts: Default::default(),
        state_attachment: None,
        content_attachment: None,
        kind: crate::presentation::ir::ViewKind::Row(Arc::new(crate::presentation::ir::RowView {
            children: crate::presentation::ir::PersistentSeq::from_vec(children),
            gap,
            vertical_align: VerticalAlign::Top,
        })),
    })
}

fn box_view(child: View, decoration: Decoration) -> View {
    let width = child.width();
    let height = child.height();
    View::from_node(ViewNodeParts {
        width,
        height,
        decoration,
        style_states: Default::default(),
        style_facts: Default::default(),
        state_attachment: None,
        content_attachment: None,
        kind: crate::presentation::ir::ViewKind::Container(Arc::new(
            crate::presentation::ir::ContainerNode { child },
        )),
    })
}

fn background_decoration(color: ColorSpec) -> Decoration {
    Decoration {
        surface_background: Some(color),
        ..Decoration::default()
    }
}

fn background_with_padding(color: ColorSpec, padding: Insets) -> Decoration {
    Decoration {
        surface_background: Some(color),
        padding,
        ..Decoration::default()
    }
}

fn style(color: &str) -> StyleSpec {
    StyleSpec {
        foreground: Some(ColorSpec::Theme(ThemeKey::from(color))),
        ..StyleSpec::default()
    }
}

fn decorated_row_view(body: &str) -> View {
    row_view(
        vec![
            RowChild::content(crate::presentation::factory::style(
                crate::presentation::factory::wrap(
                    crate::presentation::factory::text("●"),
                    crate::WrapMode::NoWrap,
                    None,
                ),
                style("accent"),
            )),
            RowChild::flex(crate::presentation::factory::fill_width(
                crate::presentation::factory::style(
                    crate::presentation::factory::text(body),
                    style("text.default"),
                ),
            )),
        ],
        1,
    )
}

fn assert_block_shape(block: &LayoutBlock, width: u16, height: usize) {
    assert_eq!(block.width, width);
    assert_eq!(block.rows.len(), height);
}

fn assert_measurement_parity(view: &View, width: u16) {
    let measured = measure_view(view, width);
    let laid_out = super::layout_view(view, LayoutConstraints::width_only(width));
    assert_eq!(
        measured, laid_out.size,
        "standalone measurement diverged from layout at width {width}: {view:#?}",
    );
}

#[test]
fn layout_stage_counters_match_semantic_nodes() {
    let row = crate::presentation::factory::row_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Fixed(3),
                crate::presentation::factory::text("two"),
            ),
            (
                crate::presentation::ir::TrackSize::Flex { min: 1 },
                crate::presentation::factory::text("three"),
            ),
        ],
        0,
        crate::presentation::VerticalAlign::Top,
    );
    let view = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Fixed(1),
                crate::presentation::factory::text("one"),
            ),
            (crate::presentation::ir::TrackSize::Flex { min: 1 }, row),
        ],
        0,
    );
    reset_layout_counters();
    let tree = super::layout_view(&view, LayoutConstraints::width_only(20));
    let counters = layout_counters();
    assert!(counters.0 <= counters.1);
    assert_eq!(counters.1, counters.2);
    assert_eq!(counters.2, tree.nodes.len());

    let hanging = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("> "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("  "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::fill_width(crate::presentation::factory::text(
            "one two three",
        )),
    ));
    reset_layout_counters();
    let hanging_tree = super::layout_view(&hanging, LayoutConstraints::width_only(8));
    let hanging_counters = layout_counters();
    assert!(hanging_counters.0 <= hanging_counters.1);
    assert!(hanging_counters.1 <= hanging_counters.2);
    assert_eq!(hanging_counters.2, hanging_tree.nodes.len());
}

#[test]
fn row_paint_lowering_matches_surface_paint_for_common_layouts() {
    let decorated = box_view(
        crate::presentation::factory::text("inside box"),
        background_with_padding(ColorSpec::ansi(4), Insets::new(1, 1, 1, 1)),
    );
    let views = [
        crate::presentation::factory::fill_width(crate::presentation::factory::text(
            "one two three",
        )),
        crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("first"),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::fill_width(crate::presentation::factory::text(
                        "second",
                    )),
                ),
            ],
            0,
        ),
        crate::presentation::factory::row_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Fixed(4),
                    crate::presentation::factory::text("left"),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    crate::presentation::factory::text("right"),
                ),
            ],
            0,
            crate::presentation::VerticalAlign::Top,
        ),
        crate::presentation::factory::row_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Fixed(10),
                    crate::presentation::factory::text("tall\nline 2\nline 3"),
                ),
                (
                    crate::presentation::ir::TrackSize::Fixed(10),
                    crate::presentation::factory::text("short"),
                ),
            ],
            0,
            crate::presentation::VerticalAlign::Top,
        ),
        crate::presentation::factory::grid(
            ([GridTrack::fixed(10), GridTrack::fixed(10)])
                .into_iter()
                .collect(),
            0,
            0,
            vec![
                (
                    crate::presentation::api::grid::GridTrack::content(),
                    vec![
                        (
                            GridCellSpec::new().row_span(2),
                            crate::presentation::factory::text("tall\nline 2\nline 3"),
                        ),
                        (
                            crate::presentation::factory::grid_cell_spec_new(),
                            crate::presentation::factory::text("short"),
                        ),
                    ],
                ),
                (
                    crate::presentation::api::grid::GridTrack::content(),
                    vec![(
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::text("next"),
                    )],
                ),
            ],
        ),
        crate::presentation::factory::row_viewport_default(
            crate::presentation::factory::column(
                vec![
                    crate::presentation::factory::text("row 0"),
                    crate::presentation::factory::text("row 1"),
                    crate::presentation::factory::text("row 2"),
                ],
                0,
            ),
            1,
        ),
        decorated,
    ];

    for view in views {
        let compiler = ViewCompiler::default();
        let tree = compiler.layout_tree(&view, LayoutConstraints::width_only(20));
        let expected = lower_surface(ViewPainter.paint_tree(&compiler, &tree));
        let (actual, complete) =
            ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &EmptyContentProvider);
        assert_eq!(actual, expected, "row lowering diverged for {view:#?}");
        assert_eq!(complete, tree.physically_complete);
    }

    // The viewport and its child are deliberately offset below a preceding
    // row.  This exercises the global-to-local clip transform used by the
    // direct row compositor rather than only the zero-origin fast path.
    let nested_viewport = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Fixed(1),
                crate::presentation::factory::text("header"),
            ),
            (
                crate::presentation::ir::TrackSize::Flex { min: 1 },
                crate::presentation::factory::fill_height(
                    crate::presentation::factory::fill_width(
                        crate::presentation::factory::row_viewport_default(
                            crate::presentation::factory::column(
                                vec![
                                    crate::presentation::factory::text("row 0"),
                                    crate::presentation::factory::text("row 1"),
                                    crate::presentation::factory::text("row 2"),
                                ],
                                0,
                            ),
                            1,
                        ),
                    ),
                ),
            ),
        ],
        0,
    );
    let compiler = ViewCompiler::default();
    let tree = compiler.layout_tree(
        &nested_viewport,
        LayoutConstraints::bounded(Size::new(20, 5)),
    );
    let expected = lower_surface(ViewPainter.paint_tree(&compiler, &tree));
    let (actual, _) =
        ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &EmptyContentProvider);
    assert_eq!(actual, expected, "offset viewport row lowering diverged");

    // Keep the semantic span lookup linear in the number of graphemes rather
    // than rescanning every span for each grapheme. Empty spans and a
    // combining mark split across style boundaries exercise the same
    // first-span selection as the canonical full compositor.
    let mut spans = vec![TextSpan::plain("")];
    for index in 0..256 {
        spans.push(TextSpan::styled(
            format!("word-{index} "),
            if index % 2 == 0 {
                StyleSpec::new().bold()
            } else {
                StyleSpec::new().italic()
            },
        ));
    }
    spans.push(TextSpan::styled("e", StyleSpec::new().bold()));
    spans.push(TextSpan::styled("\u{301}", StyleSpec::new().italic()));
    spans.push(TextSpan::plain(""));
    let styled_many =
        crate::presentation::factory::fill_width(crate::presentation::factory::styled_text(spans));
    let compiler = ViewCompiler::default();
    let tree = compiler.layout_tree(&styled_many, LayoutConstraints::width_only(20));
    let expected = lower_surface(ViewPainter.paint_tree(&compiler, &tree));
    let (actual, _) =
        ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &EmptyContentProvider);
    assert_eq!(actual, expected, "many-span geometry/style lookup diverged");

    let labeled = crate::presentation::factory::border(
        crate::presentation::factory::text("x"),
        BorderSpec::plain().top_label("🐕x"),
    );
    for width in [3, 4] {
        let compiler = ViewCompiler::default();
        let tree = compiler.layout_tree(&labeled, LayoutConstraints::bounded(Size::new(width, 3)));
        let expected = lower_surface(ViewPainter.paint_tree(&compiler, &tree));
        let (actual, _) =
            ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &EmptyContentProvider);
        assert_eq!(
            actual, expected,
            "wide border label clipping diverged at width {width}"
        );
        assert!(
            actual
                .iter()
                .all(|row| row.validate_cell_geometry().is_ok())
        );
    }
}

#[cfg(feature = "perf-counters")]
#[test]
fn row_paint_prunes_disjoint_column_children_before_allocating_rows() {
    let _lock = crate::perf::test_lock();
    let view = crate::presentation::factory::column(
        (0..4_096)
            .map(|index| crate::presentation::factory::text(format!("row {index}")))
            .collect(),
        0,
    );
    let compiler = ViewCompiler::default();
    let tree = compiler.layout_tree(&view, LayoutConstraints::bounded(Size::new(20, 1)));
    crate::perf::reset();
    let (rows, _) =
        ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &EmptyContentProvider);
    let counters = crate::perf::snapshot();
    eprintln!(
        "row-paint-prune counters: visited={} allocated_cells={}",
        counters.value(crate::perf::Counter::PaintNodesVisited),
        counters.value(crate::perf::Counter::PaintCellsAllocated),
    );

    assert_eq!(rows.len(), 1);
    assert!(
        counters.value(crate::perf::Counter::PaintNodesVisited) < 16,
        "offscreen siblings must be pruned before row painting, counters={counters:?}"
    );
    assert!(
        counters.value(crate::perf::Counter::PaintCellsAllocated) < 256,
        "small viewport must allocate only row-sized surfaces, counters={counters:?}"
    );
}

#[cfg(feature = "perf-counters")]
#[test]
fn retained_layout_cache_reuses_warm_measurement_and_prepare() {
    let _lock = crate::perf::test_lock();
    let view = crate::presentation::factory::column(
        (0..10_000)
            .map(|index| crate::presentation::factory::text(format!("stable-{index}")))
            .collect(),
        0,
    );
    let overlay = crate::scene::ResolutionOverlay::default();
    let mut cache = LayoutCache::default();

    cache.begin_epoch();
    crate::perf::reset();
    let first = layout_view_with_overlay_and_cache(
        &view,
        LayoutConstraints::width_only(80),
        &overlay,
        &mut cache,
    );
    let first_counters = crate::perf::snapshot();

    cache.begin_epoch();
    crate::perf::reset();
    let second = layout_view_with_overlay_and_cache(
        &view,
        LayoutConstraints::width_only(80),
        &overlay,
        &mut cache,
    );
    let second_counters = crate::perf::snapshot();

    assert_eq!(first, second);
    assert!(first_counters.value(crate::perf::Counter::TextFlowMeasureCalls) > 0);
    assert_eq!(
        second_counters.value(crate::perf::Counter::TextFlowMeasureCalls),
        0
    );
    assert!(
        second_counters.value(crate::perf::Counter::MeasureNodeCalls)
            < first_counters.value(crate::perf::Counter::MeasureNodeCalls)
    );
    assert_eq!(
        second_counters.value(crate::perf::Counter::PrepareNodeCalls),
        0
    );
    assert!(cache.retained_entries() > 0);
}

#[cfg(feature = "perf-counters")]
#[test]
fn retained_layout_cache_reuses_unaffected_shared_path() {
    let _lock = crate::perf::test_lock();
    let shared = crate::presentation::factory::column(
        (0..1_000)
            .map(|index| crate::presentation::factory::text(format!("shared-{index}")))
            .collect(),
        0,
    );
    let original = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            shared.clone(),
        )],
        0,
    );
    let changed = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                shared.clone(),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("changed"),
            ),
        ],
        0,
    );
    let overlay = crate::scene::ResolutionOverlay::default();
    let mut cache = LayoutCache::default();

    cache.begin_epoch();
    crate::perf::reset();
    let original_tree = layout_view_with_overlay_and_cache(
        &original,
        LayoutConstraints::width_only(80),
        &overlay,
        &mut cache,
    );
    let original_counters = crate::perf::snapshot();

    cache.begin_epoch();
    crate::perf::reset();
    let changed_tree = layout_view_with_overlay_and_cache(
        &changed,
        LayoutConstraints::width_only(80),
        &overlay,
        &mut cache,
    );
    let changed_counters = crate::perf::snapshot();

    assert!(changed_tree.nodes.len() > original_tree.nodes.len());
    assert!(
        changed_counters.value(crate::perf::Counter::TextFlowMeasureCalls)
            < original_counters.value(crate::perf::Counter::TextFlowMeasureCalls)
    );
    assert!(
        changed_counters.value(crate::perf::Counter::MeasureNodeCalls)
            < original_counters.value(crate::perf::Counter::MeasureNodeCalls) / 10
    );
    assert!(changed_counters.value(crate::perf::Counter::LayoutNodesEmitted) > 1_000);
}

#[cfg(feature = "perf-counters")]
#[test]
fn retained_layout_cache_rotates_out_old_view_id_working_sets() {
    let _lock = crate::perf::test_lock();
    let overlay = crate::scene::ResolutionOverlay::default();
    let mut cache = LayoutCache::default();
    let mut first_entries = None;
    let mut first_view_id = None;

    for generation in 0..6 {
        cache.begin_epoch();
        let view = crate::presentation::factory::column(
            (0..100)
                .map(|index| crate::presentation::factory::text(format!("{generation}-{index}")))
                .collect(),
            0,
        );
        if generation == 0 {
            first_view_id = Some(view.id());
        }
        let _ = layout_view_with_overlay_and_cache(
            &view,
            LayoutConstraints::width_only(80),
            &overlay,
            &mut cache,
        );
        first_entries.get_or_insert(cache.retained_entries());
    }

    let working_set = first_entries.expect("at least one cache generation");
    assert!(cache.retained_entries() <= working_set.saturating_mul(2));
    assert!(!cache.contains_view_id(first_view_id.expect("first view generation")));
}

#[test]
fn standalone_measurement_matches_width_only_layout() {
    let views = vec![
        crate::presentation::factory::text("text"),
        crate::presentation::factory::spacer(2),
        crate::presentation::factory::container(crate::presentation::factory::text(
            "wrapped content",
        )),
        crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Fixed(1),
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "fixed",
                    )),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("content"),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "flex",
                    )),
                ),
                (
                    crate::presentation::ir::TrackSize::FlexMax { min: 1, max: 4 },
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "flex max",
                    )),
                ),
            ],
            0,
        ),
        crate::presentation::factory::row_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Fixed(3),
                    crate::presentation::factory::text("fixed"),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("content"),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    crate::presentation::factory::fill_width(crate::presentation::factory::text(
                        "flex",
                    )),
                ),
            ],
            0,
            crate::presentation::VerticalAlign::Top,
        ),
        crate::presentation::factory::hanging(
            crate::presentation::factory::wrap(
                crate::presentation::factory::text("> "),
                crate::WrapMode::NoWrap,
                None,
            ),
            crate::presentation::factory::wrap(
                crate::presentation::factory::text("  "),
                crate::WrapMode::NoWrap,
                None,
            ),
            crate::presentation::factory::fill_width(crate::presentation::factory::text(
                "hanging body",
            )),
        ),
        crate::presentation::factory::clamp_rows(
            crate::presentation::factory::text("clamped content"),
            2,
            OverflowIndicator::None,
        ),
        crate::presentation::factory::row_viewport_default(
            crate::presentation::factory::text("viewport content"),
            1,
        ),
        crate::presentation::factory::border(
            crate::presentation::factory::padding(
                crate::presentation::factory::max_height(
                    crate::presentation::factory::min_height(
                        crate::presentation::factory::max_width(
                            crate::presentation::factory::min_width(
                                crate::presentation::factory::fill_height(
                                    crate::presentation::factory::fill_width(
                                        crate::presentation::factory::text("decorated"),
                                    ),
                                ),
                                2,
                            ),
                            30,
                        ),
                        1,
                    ),
                    8,
                ),
                Insets::all(1),
            ),
            BorderSpec::plain(),
        ),
    ];

    for view in &views {
        for width in 0..=40 {
            assert_measurement_parity(view, width);
        }
    }
}

#[test]
fn flex_max_intrinsic_height_respects_its_cap() {
    let view = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Fixed(1),
                crate::presentation::factory::text("header"),
            ),
            (
                crate::presentation::ir::TrackSize::FlexMax { min: 1, max: 16 },
                crate::presentation::factory::text(
                    (1..=40).map(|row| format!("{row}\n")).collect::<String>(),
                ),
            ),
        ],
        0,
    );
    assert_eq!(ViewCompiler::default().compile(&view, 20).rows.len(), 17);
}

#[test]
fn capped_flex_redistributes_capacity_to_uncapped_tracks() {
    let allocation = crate::presentation::layout::tracks::allocate_tracks(
        20,
        0,
        &[
            crate::presentation::ir::TrackSize::FlexMax { min: 1, max: 5 },
            crate::presentation::ir::TrackSize::Flex { min: 1 },
        ],
        |_, _| 0,
    );
    assert_eq!(allocation.tracks, [5, 15]);
}

#[test]
fn capped_flex_tracks_leave_only_intentional_slack() {
    let allocation = crate::presentation::layout::tracks::allocate_tracks(
        20,
        0,
        &[
            crate::presentation::ir::TrackSize::FlexMax { min: 1, max: 3 },
            crate::presentation::ir::TrackSize::FlexMax { min: 1, max: 4 },
        ],
        |_, _| 0,
    );
    assert_eq!(allocation.tracks, [3, 4]);
}

fn empty_vertical() -> View {
    crate::presentation::factory::column(vec![], 0)
}

#[test]
fn hanging_view_repeats_continuation_prefix_while_body_wraps() {
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("10. "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("    "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::fill_width(crate::presentation::factory::text(
            "one two three",
        )),
    ));
    let block = ViewCompiler::default().compile(&view, 12);

    assert_eq!(
        block
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>(),
        ["10. one two ", "    three"]
    );
    assert!(block.rows.iter().all(|row| row.width() == 12));
    assert!(block.physically_complete);
}

#[test]
fn hanging_view_marks_prefix_too_wide_as_incomplete_without_panicking() {
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("10. "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("    "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::fill_width(crate::presentation::factory::text("body")),
    ));
    let block = ViewCompiler::default().compile(&view, 3);

    assert!(!block.physically_complete);
    assert!(!block.rows.is_empty());
}

#[test]
fn hanging_view_preserves_prefix_and_continuation_styles() {
    let marker = crate::presentation::factory::foreground(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("* "),
            crate::WrapMode::NoWrap,
            None,
        ),
        ColorSpec::ansi(3),
    );
    let continuation = crate::presentation::factory::foreground(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("  "),
            crate::WrapMode::NoWrap,
            None,
        ),
        ColorSpec::ansi(3),
    );
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        marker,
        continuation,
        crate::presentation::factory::fill_width(crate::presentation::factory::text("one two")),
    ));
    let rows = ViewCompiler::default().compile(&view, 6).rows;

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].style_at(0), rows[1].style_at(0));
    assert_eq!(rows[0].style_at(1), rows[1].style_at(1));
}

#[test]
fn bounded_vertical_tracks_allocate_multiple_flex_children() {
    let view = crate::presentation::factory::fill_height(crate::presentation::factory::fill_width(
        crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Fixed(2),
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "header",
                    )),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "body",
                    )),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "tail",
                    )),
                ),
                (
                    crate::presentation::ir::TrackSize::Fixed(1),
                    crate::presentation::factory::fill_height(crate::presentation::factory::text(
                        "footer",
                    )),
                ),
            ],
            0,
        ),
    ));
    let compiler = ViewCompiler::default();
    let tree = compiler.layout_tree(
        &view,
        crate::geometry::LayoutConstraints::bounded(Size::new(20, 10)),
    );
    assert!(tree.validate());
    let root = tree.node(tree.root);
    let children = root.children.clone();
    assert_eq!(children.len(), 4);
    assert_eq!(tree.node(children[0]).rect.y, 0);
    assert_eq!(tree.node(children[0]).rect.height, 2);
    assert_eq!(tree.node(children[1]).rect.y, 2);
    assert_eq!(tree.node(children[1]).rect.height, 4);
    assert_eq!(tree.node(children[2]).rect.y, 6);
    assert_eq!(tree.node(children[2]).rect.height, 3);
    assert_eq!(tree.node(children[3]).rect.y, 9);
    assert_eq!(tree.node(children[3]).rect.height, 1);
}

#[test]
fn unbounded_column_treats_flex_as_intrinsic_after_fixed_tracks() {
    let view = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Fixed(3),
                crate::presentation::factory::fill_height(crate::presentation::factory::text(
                    "header",
                )),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("content"),
            ),
            (
                crate::presentation::ir::TrackSize::Flex { min: 1 },
                crate::presentation::factory::fill_height(crate::presentation::factory::text(
                    "body\nline\nthree",
                )),
            ),
        ],
        0,
    );
    let tree = ViewCompiler::default()
        .layout_tree(&view, crate::geometry::LayoutConstraints::width_only(20));
    let root = tree.node(tree.root);
    assert_eq!(root.rect.height, 7);
    assert_eq!(tree.node(root.children[0]).rect.height, 3);
    assert_eq!(tree.node(root.children[1]).rect.height, 1);
    assert_eq!(tree.node(root.children[2]).rect.height, 3);
}

#[test]
fn fit_row_respects_fixed_track_and_fill_width_content() {
    let view = crate::presentation::factory::fit_width(row_view(
        vec![
            RowChild::fixed(
                5,
                crate::presentation::factory::fill_width(crate::presentation::factory::text(
                    "fixed",
                )),
            ),
            RowChild::content(crate::presentation::factory::fill_width(
                crate::presentation::factory::text("x"),
            )),
        ],
        0,
    ));
    let tree = ViewCompiler::default()
        .layout_tree(&view, crate::geometry::LayoutConstraints::width_only(20));
    let root = tree.node(tree.root);
    assert_eq!(root.rect.width, 6);
    assert_eq!(tree.node(root.children[0]).rect.width, 5);
    assert_eq!(tree.node(root.children[1]).rect.width, 1);
}

#[test]
fn bounded_row_vertical_alignment_uses_extra_height() {
    let view = crate::presentation::factory::fill_height(crate::presentation::factory::fill_width(
        crate::presentation::factory::row_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("x"),
            )],
            0,
            crate::presentation::VerticalAlign::Bottom,
        ),
    ));
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(5, 3));
    assert!(block.rows[0].plain_text().is_empty());
    assert!(block.rows[1].plain_text().is_empty());
    assert_eq!(block.rows[2].plain_text(), "x");
}

#[test]
fn clamp_does_not_mask_impossible_wide_grapheme() {
    let view = crate::presentation::factory::clamp_rows(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("漢")),
        1,
        OverflowIndicator::None,
    );
    assert!(
        !ViewCompiler::default()
            .compile(&view, 1)
            .physically_complete
    );
}

#[test]
fn nowrap_paint_clips_whole_graphemes_and_never_emits_a_partial_wide_cell() {
    let view = crate::presentation::factory::wrap(
        crate::presentation::factory::text("ABC界D"),
        crate::WrapMode::NoWrap,
        None,
    );
    let block = compile_view(&view, 4);
    assert!(block.rows[0].validate_cell_geometry().is_ok());
    assert_eq!(block.rows[0].plain_text(), "ABC");
    assert!(!block.physically_complete);
    assert!(block.rows[0].occupied_width() <= 4);
}

#[test]
fn bounded_compiler_preserves_fit_height_inside_fixed_track() {
    let view =
        crate::presentation::factory::fill_height(crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Fixed(3),
                crate::presentation::factory::text("x"),
            )],
            0,
        ));
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(10, 3));
    assert_eq!(block.rows.len(), 3);
    assert_eq!(block.rows[0].plain_text(), "x");
    assert!(block.rows[1].plain_text().is_empty());
}

#[test]
fn view_bounds_apply_to_fit_and_fill_outer_dimensions() {
    let fit = crate::presentation::factory::min_width(crate::presentation::factory::text("x"), 5);
    let fit_block = crate::presentation::layout::compile_bounded_view(&fit, Size::new(20, 20));
    assert_eq!(fit_block.width, 5);

    let fill = crate::presentation::factory::max_height(
        crate::presentation::factory::fill_height(crate::presentation::factory::max_width(
            crate::presentation::factory::fill_width(crate::presentation::factory::text(
                "abcdefgh",
            )),
            4,
        )),
        3,
    );
    let fill_block = crate::presentation::layout::compile_bounded_view(&fill, Size::new(20, 20));
    assert_eq!(fill_block.width, 4);
    assert_eq!(fill_block.rows.len(), 3);
}

#[test]
fn view_width_bounds_change_wrapping_and_height() {
    let view =
        crate::presentation::factory::max_width(crate::presentation::factory::text("abcdefgh"), 4);
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(20, 20));
    assert_eq!(block.width, 4);
    assert_eq!(block.rows.len(), 2);
}

#[test]
fn view_bounds_normalize_contradictions_and_respect_hard_capacity() {
    let contradictory = crate::presentation::factory::max_height(
        crate::presentation::factory::min_height(crate::presentation::factory::text("x"), 4),
        2,
    );
    let block =
        crate::presentation::layout::compile_bounded_view(&contradictory, Size::new(20, 10));
    assert_eq!(block.rows.len(), 4);

    let constrained =
        crate::presentation::factory::min_height(crate::presentation::factory::text("x"), 5);
    let block = crate::presentation::layout::compile_bounded_view(&constrained, Size::new(20, 3));
    assert_eq!(block.rows.len(), 3);
}

#[test]
fn view_height_bounds_include_padding_and_border() {
    let view = crate::presentation::factory::max_height(
        crate::presentation::factory::border(
            crate::presentation::factory::padding(crate::presentation::factory::text("x"), 1),
            BorderSpec::plain(),
        ),
        5,
    );
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(20, 20));
    assert_eq!(block.rows.len(), 5);
}

mod flow;
mod grid;
mod style;
mod text;

#[test]
#[ignore = "local release-mode characterization probe"]
fn layout_performance_probe() {
    use std::time::Instant;

    let long_text = (0..80)
        .map(|index| format!("line {index}: a moderately long generic sentence"))
        .collect::<Vec<_>>()
        .join("\n");
    let cases = vec![
        (
            "simple_text",
            crate::presentation::factory::text("hello world"),
        ),
        (
            "wrapped_text",
            crate::presentation::factory::fill_width(crate::presentation::factory::text(
                "one two three four five six seven eight nine ten",
            )),
        ),
        (
            "nested_row_column",
            crate::presentation::factory::column_specs(
                vec![
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        crate::presentation::factory::row_specs(
                            vec![
                                (
                                    crate::presentation::ir::TrackSize::Content { max: None },
                                    crate::presentation::factory::text("left"),
                                ),
                                (
                                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                                    crate::presentation::factory::fill_width(
                                        crate::presentation::factory::text("right"),
                                    ),
                                ),
                            ],
                            0,
                            crate::presentation::VerticalAlign::Top,
                        ),
                    ),
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        crate::presentation::factory::fill_width(
                            crate::presentation::factory::text("body"),
                        ),
                    ),
                ],
                0,
            ),
        ),
        (
            "long_text",
            crate::presentation::factory::fill_width(crate::presentation::factory::text(
                &long_text,
            )),
        ),
        (
            "decorated_text",
            crate::presentation::factory::background(
                crate::presentation::factory::border(
                    crate::presentation::factory::padding(
                        crate::presentation::factory::fill_width(
                            crate::presentation::factory::text("decorated message with decoration"),
                        ),
                        Insets::horizontal(1),
                    ),
                    BorderSpec::rounded(),
                ),
                ColorSpec::ansi(4),
            ),
        ),
        (
            "hanging",
            crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
                crate::presentation::factory::wrap(
                    crate::presentation::factory::text("10. "),
                    crate::WrapMode::NoWrap,
                    None,
                ),
                crate::presentation::factory::wrap(
                    crate::presentation::factory::text("    "),
                    crate::WrapMode::NoWrap,
                    None,
                ),
                crate::presentation::factory::fill_width(crate::presentation::factory::text(
                    "one two three four five six",
                )),
            )),
        ),
        (
            "bounded_row_viewport",
            crate::presentation::factory::row_viewport_default(
                crate::presentation::factory::text(&long_text),
                20,
            ),
        ),
        (
            "scene_body",
            crate::presentation::factory::column_specs(
                vec![(
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("body"),
                )],
                0,
            ),
        ),
    ];
    let iterations = 100;

    for (name, view) in cases {
        for width in [40, 80, 120, 160] {
            let width_start = Instant::now();
            for _ in 0..iterations {
                std::hint::black_box(
                    ViewCompiler::default()
                        .layout_tree(&view, LayoutConstraints::width_only(width)),
                );
            }
            let width_elapsed = width_start.elapsed();

            for height in [10, 24, 50] {
                let bounded_start = Instant::now();
                for _ in 0..iterations {
                    std::hint::black_box(
                        ViewCompiler::default().layout_tree(
                            &view,
                            LayoutConstraints::bounded(Size::new(width, height)),
                        ),
                    );
                }
                let bounded_elapsed = bounded_start.elapsed();

                let paint_start = Instant::now();
                for _ in 0..iterations {
                    std::hint::black_box(
                        ViewCompiler::default()
                            .compile_bounded_for_probe(&view, Size::new(width, height)),
                    );
                }
                let paint_elapsed = paint_start.elapsed();
                println!(
                    "{name:32} width={width:3} height={height:2} width_only={:?} bounded={:?} paint={:?}",
                    width_elapsed, bounded_elapsed, paint_elapsed
                );
            }
        }
    }
}

impl ViewCompiler<'_> {
    fn compile_bounded_for_probe(&self, view: &View, size: Size) -> LayoutBlock {
        let tree = self.layout_tree(view, LayoutConstraints::bounded(size));
        let surface = ViewPainter.paint_tree(self, &tree);
        LayoutBlock {
            width: surface.width(),
            rows: (0..surface.height())
                .map(|y| {
                    PhysicalRow::from_cells(
                        (0..surface.width())
                            .map(|x| surface.get(x, y).clone())
                            .collect(),
                    )
                })
                .collect(),
            physically_complete: tree.physically_complete && surface.physically_complete,
        }
    }
}
