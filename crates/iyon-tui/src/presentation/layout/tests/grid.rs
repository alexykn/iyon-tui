use super::*;
use crate::geometry::Rect;
use crate::presentation::{GridCellSpec, GridTrack, HorizontalAlign, Insets, VerticalAlign};

fn tree(view: &View, width: u16) -> LayoutTree {
    ViewCompiler::default().layout_tree(view, LayoutConstraints::width_only(width))
}

fn child_rects(view: &View, width: u16) -> Vec<Rect> {
    let laid_out = tree(view, width);
    let root = laid_out.node(laid_out.root);
    root.children
        .iter()
        .map(|id| laid_out.node(*id).rect)
        .collect()
}

fn text_at(view: &View, width: u16, needle: &str) -> (u16, u16) {
    let block = compile_view(view, width);
    for (y, row) in block.rows.iter().enumerate() {
        if let Some(x) = row.cell_x_of(needle) {
            return (x as u16, y as u16);
        }
    }
    panic!(
        "did not find {needle:?} in {:?}",
        block
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn shared_columns_align_across_rows() {
    let view = crate::presentation::factory::grid(
        ([GridTrack::content(), GridTrack::content()])
            .into_iter()
            .collect(),
        0,
        0,
        vec![
            (
                crate::presentation::api::grid::GridTrack::content(),
                vec![
                    (
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::text("a"),
                    ),
                    (
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::text("long-long"),
                    ),
                ],
            ),
            (
                crate::presentation::api::grid::GridTrack::content(),
                vec![
                    (
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::text("longer"),
                    ),
                    (
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::text("x"),
                    ),
                ],
            ),
        ],
    );
    let (x1, _) = text_at(&view, 40, "long-long");
    let (x2, _) = text_at(&view, 40, "x");
    assert_eq!(x1, x2);
    let rects = child_rects(&view, 40);
    assert_eq!(rects[1].x, rects[3].x);
}

#[test]
fn fixed_content_flex_consume_width() {
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::grid(
        ([GridTrack::fixed(3), GridTrack::content(), GridTrack::flex()])
            .into_iter()
            .collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("abc"),
                ),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("12345"),
                ),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::fill_width(crate::presentation::factory::text(
                        "flex",
                    )),
                ),
            ],
        )],
    ));
    let rects = child_rects(&view, 20);
    assert_eq!(
        rects
            .iter()
            .map(|rect| (rect.x, rect.width))
            .collect::<Vec<_>>(),
        vec![(0, 3), (3, 5), (8, 12)],
        "rects={rects:?} tree_size={:?}",
        tree(&view, 20).size
    );
}

#[test]
fn wrapping_flex_cell_contributes_row_height() {
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::grid(
        ([GridTrack::content(), GridTrack::flex()])
            .into_iter()
            .collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("a"),
                ),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("word word word word"),
                ),
            ],
        )],
    ));
    let block = compile_view(&view, 12);
    assert!(block.rows.len() > 1);
}

#[test]
fn fit_child_keeps_intrinsic_width_fill_uses_cell() {
    let fit = crate::presentation::factory::grid(
        ([GridTrack::fixed(10)]).into_iter().collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                crate::presentation::factory::grid_cell_spec_new(),
                crate::presentation::factory::text("hi"),
            )],
        )],
    );
    let fill = crate::presentation::factory::grid(
        ([GridTrack::fixed(10)]).into_iter().collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                crate::presentation::factory::grid_cell_spec_new(),
                crate::presentation::factory::fill_width(crate::presentation::factory::text("hi")),
            )],
        )],
    );
    assert_eq!(child_rects(&fit, 20)[0].width, 2);
    assert_eq!(child_rects(&fill, 20)[0].width, 10);
}

#[test]
fn horizontal_alignment_places_the_child_view() {
    let view = crate::presentation::factory::grid(
        ([
            GridTrack::fixed(8),
            GridTrack::fixed(8),
            GridTrack::fixed(8),
        ])
        .into_iter()
        .collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (
                    GridCellSpec::new().horizontal_align(HorizontalAlign::Start),
                    crate::presentation::factory::text("x"),
                ),
                (
                    GridCellSpec::new().horizontal_align(HorizontalAlign::Center),
                    crate::presentation::factory::text("x"),
                ),
                (
                    GridCellSpec::new().horizontal_align(HorizontalAlign::End),
                    crate::presentation::factory::text("x"),
                ),
            ],
        )],
    );
    let rects = child_rects(&view, 24);
    assert_eq!(rects[0].x, 0);
    assert_eq!(rects[1].x, 8 + 3);
    assert_eq!(rects[2].x, 16 + 7);
}

#[test]
fn vertical_alignment_places_the_child_view() {
    let view = crate::presentation::factory::grid(
        ([
            GridTrack::fixed(1),
            GridTrack::fixed(1),
            GridTrack::fixed(1),
        ])
        .into_iter()
        .collect(),
        0,
        0,
        vec![(
            GridTrack::fixed(5),
            vec![
                (
                    GridCellSpec::new().vertical_align(VerticalAlign::Top),
                    crate::presentation::factory::text("x"),
                ),
                (
                    GridCellSpec::new().vertical_align(VerticalAlign::Center),
                    crate::presentation::factory::text("x"),
                ),
                (
                    GridCellSpec::new().vertical_align(VerticalAlign::Bottom),
                    crate::presentation::factory::text("x"),
                ),
            ],
        )],
    );
    let rects = child_rects(&view, 8);
    assert_eq!(rects[0].y, 0);
    assert_eq!(rects[1].y, 2);
    assert_eq!(rects[2].y, 4);
}

#[test]
fn column_span_area_includes_internal_gap() {
    let view = crate::presentation::factory::grid(
        ([GridTrack::fixed(3), GridTrack::fixed(4)])
            .into_iter()
            .collect(),
        1,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                GridCellSpec::new().column_span(2),
                crate::presentation::factory::wrap(
                    crate::presentation::factory::fill_width(crate::presentation::factory::text(
                        "abcdefgh",
                    )),
                    crate::WrapMode::NoWrap,
                    None,
                ),
            )],
        )],
    );
    assert_eq!(child_rects(&view, 20)[0].width, 8);
}

#[test]
fn row_span_area_includes_internal_gap() {
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::grid(
        ([GridTrack::flex()]).into_iter().collect(),
        0,
        1,
        vec![
            (
                GridTrack::fixed(2),
                vec![(
                    GridCellSpec::new().row_span(2),
                    crate::presentation::factory::fill_height(
                        crate::presentation::factory::fill_width(
                            crate::presentation::factory::spacer(1),
                        ),
                    ),
                )],
            ),
            (GridTrack::fixed(3), vec![]),
        ],
    ));
    assert_eq!(child_rects(&view, 10)[0].height, 6);
}

#[test]
fn spanning_cell_grows_content_columns() {
    let view = crate::presentation::factory::grid(
        ([GridTrack::content(), GridTrack::content()])
            .into_iter()
            .collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                GridCellSpec::new().column_span(2),
                crate::presentation::factory::text("abcdefghijkl"),
            )],
        )],
    );
    let size = measure_view(&view, 40);
    assert!(size.width >= 12);
    assert_eq!(child_rects(&view, 40)[0].width, size.width);
}

#[test]
fn spanning_cell_grows_content_rows() {
    let view = crate::presentation::factory::grid(
        vec![],
        0,
        1,
        vec![
            (
                crate::presentation::api::grid::GridTrack::content(),
                vec![(
                    GridCellSpec::new().row_span(2),
                    crate::presentation::factory::spacer(5),
                )],
            ),
            (crate::presentation::api::grid::GridTrack::content(), vec![]),
        ],
    );
    let size = measure_view(&view, 10);
    assert!(size.height >= 5);
}

#[test]
fn nested_grid_measures() {
    let inner = crate::presentation::factory::grid(
        ([GridTrack::content(), GridTrack::content()])
            .into_iter()
            .collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("ab"),
                ),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("cd"),
                ),
            ],
        )],
    );
    let outer = crate::presentation::factory::grid(
        vec![],
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (crate::presentation::factory::grid_cell_spec_new(), inner),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("z"),
                ),
            ],
        )],
    );
    let block = compile_view(&outer, 20);
    assert!(block.rows[0].plain_text().contains("ab"));
    assert!(block.rows[0].plain_text().contains("cd"));
    assert!(block.rows[0].plain_text().contains("z"));
}

#[test]
fn grid_inside_row_and_row_inside_grid() {
    let nested_grid = crate::presentation::factory::grid(
        vec![GridTrack::content(), GridTrack::flex()],
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("a"),
                ),
                (
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("b"),
                ),
            ],
        )],
    );
    let grid_in_row =
        crate::presentation::factory::fill_width(crate::presentation::factory::row_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("L"),
                ),
                (
                    crate::presentation::ir::TrackSize::Flex { min: 1 },
                    nested_grid,
                ),
            ],
            0,
            crate::presentation::VerticalAlign::Top,
        ));
    assert!(
        compile_view(&grid_in_row, 20).rows[0]
            .plain_text()
            .contains("ab")
    );

    let row_in_grid = crate::presentation::factory::fill_width(crate::presentation::factory::grid(
        ([GridTrack::flex()]).into_iter().collect(),
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                crate::presentation::factory::grid_cell_spec_new(),
                crate::presentation::factory::row_specs(
                    vec![
                        (
                            crate::presentation::ir::TrackSize::Content { max: None },
                            crate::presentation::factory::text("x"),
                        ),
                        (
                            crate::presentation::ir::TrackSize::Content { max: None },
                            crate::presentation::factory::text("y"),
                        ),
                    ],
                    0,
                    crate::presentation::VerticalAlign::Top,
                ),
            )],
        )],
    ));
    assert!(
        compile_view(&row_in_grid, 20).rows[0]
            .plain_text()
            .contains("xy")
    );
}

#[test]
fn padding_and_border_use_inner_width() {
    let view = crate::presentation::factory::border(
        crate::presentation::factory::padding(
            crate::presentation::factory::fill_width(crate::presentation::factory::grid(
                ([GridTrack::flex()]).into_iter().collect(),
                0,
                0,
                vec![(
                    crate::presentation::api::grid::GridTrack::content(),
                    vec![(
                        crate::presentation::factory::grid_cell_spec_new(),
                        crate::presentation::factory::fill_width(
                            crate::presentation::factory::text("hello"),
                        ),
                    )],
                )],
            )),
            Insets::horizontal(2),
        ),
        BorderSpec::plain(),
    );
    let laid_out = tree(&view, 20);
    let root = laid_out.node(laid_out.root);
    let child = laid_out.node(root.children[0]);
    assert!(child.rect.x >= 3);
    assert!(child.rect.width <= 20 - 6);
}

#[test]
fn bounds_apply_through_generic_measure() {
    let view = crate::presentation::factory::max_width(
        crate::presentation::factory::grid(
            ([GridTrack::content()]).into_iter().collect(),
            0,
            0,
            vec![(
                crate::presentation::api::grid::GridTrack::content(),
                vec![(
                    crate::presentation::factory::grid_cell_spec_new(),
                    crate::presentation::factory::text("hello-world"),
                )],
            )],
        ),
        6,
    );
    let size = measure_view(&view, 40);
    assert_eq!(size.width, 6);
}

#[test]
fn style_state_inherits_into_cells() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("mode", "x"),
        StyleSpec::new().bold(),
    );
    let view = crate::presentation::factory::grid(
        vec![],
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                crate::presentation::factory::grid_cell_spec_new(),
                crate::presentation::factory::style(
                    crate::presentation::factory::text("A"),
                    StyleRef::theme("probe"),
                ),
            )],
        )],
    );
    let view = crate::presentation::factory::style_state(view, "mode", "x");
    let compiler = ViewCompiler::new(&theme);
    let laid_out = compiler.layout_tree(&view, LayoutConstraints::width_only(4));
    let surface = ViewPainter.paint_tree(&compiler, &laid_out);
    assert!(surface.get(0, 0).style.bold);
}

#[test]
fn self_only_facts_do_not_enter_cells() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.role", "heading"),
        StyleSpec::new().bold(),
    );
    let view = crate::presentation::factory::grid(
        vec![],
        0,
        0,
        vec![(
            crate::presentation::api::grid::GridTrack::content(),
            vec![(
                crate::presentation::factory::grid_cell_spec_new(),
                crate::presentation::factory::style(
                    crate::presentation::factory::text("A"),
                    StyleRef::theme("probe"),
                ),
            )],
        )],
    );
    let view = crate::presentation::factory::style_fact(view, "test.role", "heading");
    let compiler = ViewCompiler::new(&theme);
    let laid_out = compiler.layout_tree(&view, LayoutConstraints::width_only(4));
    let surface = ViewPainter.paint_tree(&compiler, &laid_out);
    assert!(!surface.get(0, 0).style.bold);
}

#[test]
fn row_span_fixed_then_content_absorbs_remainder() {
    let view = crate::presentation::factory::grid(
        vec![],
        0,
        1,
        vec![
            (
                GridTrack::fixed(1),
                vec![(
                    GridCellSpec::new().row_span(2),
                    crate::presentation::factory::spacer(5),
                )],
            ),
            (crate::presentation::api::grid::GridTrack::content(), vec![]),
        ],
    );
    let rects = child_rects(&view, 10);
    assert_eq!(rects[0].height, 5);
    let size = measure_view(&view, 10);
    assert_eq!(size.height, 5);
}
