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
    ColorSpec, HorizontalAlign, Insets, IntoView, StyleRef, StyleSpec, TextSpan, ThemeKey,
    VerticalAlign, View, WidthRule, WrapMode,
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
            RowChild::content(View::text("●").no_wrap().style(style("accent")).into_view()),
            RowChild::flex(
                View::text(body)
                    .style(style("text.default"))
                    .fill_width()
                    .into_view(),
            ),
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
    let view = View::vertical(|column| {
        column.fixed(1, View::text("one"));
        column.flex(View::horizontal(|row| {
            row.fixed(3, View::text("two"));
            row.flex(View::text("three"));
        }));
    });
    reset_layout_counters();
    let tree = super::layout_view(&view, LayoutConstraints::width_only(20));
    let counters = layout_counters();
    assert!(counters.0 <= counters.1);
    assert_eq!(counters.1, counters.2);
    assert_eq!(counters.2, tree.nodes.len());

    let hanging = View::hanging(
        View::text("> ").no_wrap(),
        View::text("  ").no_wrap(),
        View::text("one two three").fill_width(),
    )
    .fill_width();
    reset_layout_counters();
    let hanging_tree = super::layout_view(&hanging, LayoutConstraints::width_only(8));
    let hanging_counters = layout_counters();
    assert!(hanging_counters.0 <= hanging_counters.1);
    assert!(hanging_counters.1 <= hanging_counters.2);
    assert_eq!(hanging_counters.2, hanging_tree.nodes.len());
}

#[cfg(feature = "perf-counters")]
#[test]
fn retained_layout_cache_reuses_warm_measurement_and_prepare() {
    let _lock = crate::perf::test_lock();
    let view = View::vertical(|column| {
        for index in 0..10_000 {
            column.child(View::text(format!("stable-{index}")));
        }
    });
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
    let shared = View::vertical(|column| {
        for index in 0..1_000 {
            column.child(View::text(format!("shared-{index}")));
        }
    });
    let original = View::vertical(|column| {
        column.child(shared.clone());
    });
    let changed = View::vertical(|column| {
        column.child(shared.clone());
        column.child(View::text("changed"));
    });
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
        let view = View::vertical(|column| {
            for index in 0..100 {
                column.child(View::text(format!("{generation}-{index}")));
            }
        });
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
        View::text("text").into_view(),
        View::spacer(2),
        View::text("wrapped content").container(),
        View::vertical(|column| {
            column.fixed(1, View::text("fixed").fill_height());
            column.child(View::text("content"));
            column.flex(View::text("flex").fill_height());
            column.flex_max(4, View::text("flex max").fill_height());
        }),
        View::horizontal(|row| {
            row.fixed(3, View::text("fixed"));
            row.child(View::text("content"));
            row.flex(View::text("flex").fill_width());
        }),
        View::hanging(
            View::text("> ").no_wrap(),
            View::text("  ").no_wrap(),
            View::text("hanging body").fill_width(),
        ),
        View::text("clamped content").clamp_rows(2, OverflowIndicator::None),
        View::row_viewport(View::text("viewport content").into_view(), 1),
        View::text("decorated")
            .into_view()
            .fill_width()
            .fill_height()
            .min_width(2)
            .max_width(30)
            .min_height(1)
            .max_height(8)
            .padding(Insets::all(1))
            .border(BorderSpec::plain()),
    ];

    for view in &views {
        for width in 0..=40 {
            assert_measurement_parity(view, width);
        }
    }
}

#[test]
fn flex_max_intrinsic_height_respects_its_cap() {
    let view = View::vertical(|column| {
        column.fixed(1, View::text("header"));
        column.flex_max(
            16,
            View::text((1..=40).map(|row| format!("{row}\n")).collect::<String>()),
        );
    });
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
    View::vertical(|_| {})
}

#[test]
fn hanging_view_repeats_continuation_prefix_while_body_wraps() {
    let view = View::hanging(
        View::text("10. ").no_wrap(),
        View::text("    ").no_wrap(),
        View::text("one two three").fill_width(),
    )
    .fill_width();
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
    let view = View::hanging(
        View::text("10. ").no_wrap(),
        View::text("    ").no_wrap(),
        View::text("body").fill_width(),
    )
    .fill_width();
    let block = ViewCompiler::default().compile(&view, 3);

    assert!(!block.physically_complete);
    assert!(!block.rows.is_empty());
}

#[test]
fn hanging_view_preserves_prefix_and_continuation_styles() {
    let marker = View::text("* ").no_wrap().foreground(ColorSpec::ansi(3));
    let continuation = View::text("  ").no_wrap().foreground(ColorSpec::ansi(3));
    let view = View::hanging(marker, continuation, View::text("one two").fill_width()).fill_width();
    let rows = ViewCompiler::default().compile(&view, 6).rows;

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].style_at(0), rows[1].style_at(0));
    assert_eq!(rows[0].style_at(1), rows[1].style_at(1));
}

#[test]
fn bounded_vertical_tracks_allocate_multiple_flex_children() {
    let view = View::vertical(|column| {
        column.fixed(2, View::text("header").fill_height());
        column.flex(View::text("body").fill_height());
        column.flex(View::text("tail").fill_height());
        column.fixed(1, View::text("footer").fill_height());
    })
    .fill_width()
    .fill_height();
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
    let view = View::vertical(|column| {
        column.fixed(3, View::text("header").fill_height());
        column.child(View::text("content"));
        column.flex(View::text("body\nline\nthree").fill_height());
    });
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
    let view = row_view(
        vec![
            RowChild::fixed(5, View::text("fixed").fill_width().into_view()),
            RowChild::content(View::text("x").fill_width().into_view()),
        ],
        0,
    )
    .fit_width();
    let tree = ViewCompiler::default()
        .layout_tree(&view, crate::geometry::LayoutConstraints::width_only(20));
    let root = tree.node(tree.root);
    assert_eq!(root.rect.width, 6);
    assert_eq!(tree.node(root.children[0]).rect.width, 5);
    assert_eq!(tree.node(root.children[1]).rect.width, 1);
}

#[test]
fn bounded_row_vertical_alignment_uses_extra_height() {
    let view = View::horizontal(|row| {
        row.child(View::text("x"));
        row.vertical_align(crate::presentation::VerticalAlign::Bottom);
    })
    .fill_width()
    .fill_height();
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(5, 3));
    assert!(block.rows[0].plain_text().is_empty());
    assert!(block.rows[1].plain_text().is_empty());
    assert_eq!(block.rows[2].plain_text(), "x");
}

#[test]
fn clamp_does_not_mask_impossible_wide_grapheme() {
    let view = View::text("漢")
        .fill_width()
        .clamp_rows(1, OverflowIndicator::None);
    assert!(
        !ViewCompiler::default()
            .compile(&view, 1)
            .physically_complete
    );
}

#[test]
fn nowrap_paint_clips_whole_graphemes_and_never_emits_a_partial_wide_cell() {
    let view = View::text("ABC界D").no_wrap().into_view();
    let block = compile_view(&view, 4);
    assert!(block.rows[0].validate_cell_geometry().is_ok());
    assert_eq!(block.rows[0].plain_text(), "ABC");
    assert!(!block.physically_complete);
    assert!(block.rows[0].occupied_width() <= 4);
}

#[test]
fn bounded_compiler_preserves_fit_height_inside_fixed_track() {
    let view = View::vertical(|column| {
        column.fixed(3, View::text("x"));
    })
    .fill_height();
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(10, 3));
    assert_eq!(block.rows.len(), 3);
    assert_eq!(block.rows[0].plain_text(), "x");
    assert!(block.rows[1].plain_text().is_empty());
}

#[test]
fn view_bounds_apply_to_fit_and_fill_outer_dimensions() {
    let fit = View::text("x").into_view().min_width(5);
    let fit_block = crate::presentation::layout::compile_bounded_view(&fit, Size::new(20, 20));
    assert_eq!(fit_block.width, 5);

    let fill = View::text("abcdefgh")
        .into_view()
        .fill_width()
        .max_width(4)
        .fill_height()
        .max_height(3);
    let fill_block = crate::presentation::layout::compile_bounded_view(&fill, Size::new(20, 20));
    assert_eq!(fill_block.width, 4);
    assert_eq!(fill_block.rows.len(), 3);
}

#[test]
fn view_width_bounds_change_wrapping_and_height() {
    let view = View::text("abcdefgh").into_view().max_width(4);
    let block = crate::presentation::layout::compile_bounded_view(&view, Size::new(20, 20));
    assert_eq!(block.width, 4);
    assert_eq!(block.rows.len(), 2);
}

#[test]
fn view_bounds_normalize_contradictions_and_respect_hard_capacity() {
    let contradictory = View::text("x").into_view().min_height(4).max_height(2);
    let block =
        crate::presentation::layout::compile_bounded_view(&contradictory, Size::new(20, 10));
    assert_eq!(block.rows.len(), 4);

    let constrained = View::text("x").into_view().min_height(5);
    let block = crate::presentation::layout::compile_bounded_view(&constrained, Size::new(20, 3));
    assert_eq!(block.rows.len(), 3);
}

#[test]
fn view_height_bounds_include_padding_and_border() {
    let view = View::text("x")
        .into_view()
        .padding(1)
        .border(BorderSpec::plain())
        .max_height(5);
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
        ("simple_text", View::text("hello world").into_view()),
        (
            "wrapped_text",
            View::text("one two three four five six seven eight nine ten")
                .fill_width()
                .into_view(),
        ),
        (
            "nested_row_column",
            View::vertical(|column| {
                column.child(View::horizontal(|row| {
                    row.child(View::text("left"));
                    row.flex(View::text("right").fill_width());
                }));
                column.child(View::text("body").fill_width());
            }),
        ),
        ("long_text", View::text(&long_text).fill_width().into_view()),
        (
            "decorated_text",
            View::text("decorated message with decoration")
                .fill_width()
                .padding(Insets::horizontal(1))
                .border(BorderSpec::rounded())
                .background(ColorSpec::ansi(4))
                .into_view(),
        ),
        (
            "hanging",
            View::hanging(
                View::text("10. ").no_wrap(),
                View::text("    ").no_wrap(),
                View::text("one two three four five six").fill_width(),
            )
            .fill_width(),
        ),
        (
            "bounded_row_viewport",
            View::row_viewport(View::text(&long_text).into_view(), 20),
        ),
        (
            "scene_body",
            View::vertical(|column| {
                column.child(View::text("body"));
            }),
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
