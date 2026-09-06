use super::*;

#[test]
fn row_uses_track_width_for_continuations() {
    let rows = compile_view(&decorated_row_view("abcdefghijklmnop"), 10).rows;
    assert_eq!(text(&rows[0]), "● abcdefgh");
    assert_eq!(text(&rows[1]), "  ijklmnop");
}

#[test]
fn narrow_rows_never_overflow_the_surface() {
    for width in 0..=4 {
        let view = row_view(
            vec![
                RowChild::fixed(3, crate::presentation::factory::text("abc")),
                RowChild::flex(crate::presentation::factory::fill_width(
                    crate::presentation::factory::text("body"),
                )),
                RowChild::content(crate::presentation::factory::text("status")),
            ],
            2,
        );
        let block = compile_view(&view, width);
        assert!(block.width <= width);
        for row in block.rows {
            assert!(row.width() <= usize::from(width));
        }
    }
}

#[test]
fn undecorated_container_is_physical_identity_and_preserves_zero_width_height() {
    let compiler = ViewCompiler::default();
    for width in [0, 1, 5, 20] {
        let plain = crate::presentation::factory::text("x");
        let wrapped = crate::presentation::factory::container(plain.clone());
        let plain_block = compiler.compile(&plain, width);
        let wrapped_block = compiler.compile(&wrapped, width);
        assert_eq!(wrapped_block.width, plain_block.width);
        assert_eq!(wrapped_block.rows, plain_block.rows);
        assert_eq!(
            wrapped_block.physically_complete,
            plain_block.physically_complete
        );
    }

    let spacer = crate::presentation::factory::container(crate::presentation::factory::spacer(3));
    assert_block_shape(&compiler.compile(&spacer, 10), 0, 3);
}

#[test]
fn final_clamp_preserves_zero_width_vertical_extent() {
    let view = crate::presentation::factory::clamp_rows(
        crate::presentation::factory::spacer(3),
        4,
        OverflowIndicator::None,
    );
    assert_block_shape(&ViewCompiler::default().compile(&view, 10), 0, 3);
}

#[test]
fn width_defaults_and_explicit_fill_are_intrinsic_and_allocated() {
    let compiler = ViewCompiler::default();
    let horizontal = crate::presentation::factory::row_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("ab"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("cde"),
            ),
        ],
        1,
        crate::presentation::VerticalAlign::Top,
    );
    let vertical = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("ab"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("abcde"),
            ),
        ],
        0,
    );

    assert_block_shape(&compiler.compile(&horizontal, 10), 6, 1);
    assert_block_shape(
        &compiler.compile(
            &crate::presentation::factory::fill_width(horizontal.clone()),
            10,
        ),
        10,
        1,
    );
    assert_block_shape(&compiler.compile(&vertical, 10), 5, 2);
    assert_block_shape(
        &compiler.compile(
            &crate::presentation::factory::fill_width(vertical.clone()),
            10,
        ),
        10,
        2,
    );
}

#[test]
fn hanging_preserves_body_width_policy_within_bounded_column() {
    let compiler = ViewCompiler::default();
    let fit = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("• "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("  "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::fit_width(crate::presentation::factory::text("x")),
    ));
    let fill = crate::presentation::factory::fill_width(crate::presentation::factory::hanging(
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("• "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::wrap(
            crate::presentation::factory::text("  "),
            crate::WrapMode::NoWrap,
            None,
        ),
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
    ));

    let fit_tree = compiler.layout_tree(
        &fit,
        crate::geometry::LayoutConstraints::bounded(Size::new(20, 4)),
    );
    let fill_tree = compiler.layout_tree(
        &fill,
        crate::geometry::LayoutConstraints::bounded(Size::new(20, 4)),
    );
    let fit_body = *fit_tree.node(fit_tree.root).children.last().unwrap();
    let fill_body = *fill_tree.node(fill_tree.root).children.last().unwrap();

    assert_eq!(fit_tree.size.width, 20);
    assert_eq!(fit_tree.node(fit_body).rect.width, 1);
    assert_eq!(fill_tree.size.width, 20);
    assert_eq!(fill_tree.node(fill_body).rect.width, 18);
}

#[test]
fn nested_fill_does_not_change_fit_child_allocation() {
    let compiler = ViewCompiler::default();
    let fit_child = crate::presentation::factory::background(
        crate::presentation::factory::fit_width(crate::presentation::factory::text("x")),
        ColorSpec::Ansi(1),
    );
    let fill_child = crate::presentation::factory::background(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        ColorSpec::Ansi(1),
    );

    let fit_parent =
        crate::presentation::factory::fill_width(crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                fit_child,
            )],
            0,
        ));
    let fill_parent =
        crate::presentation::factory::fill_width(crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                fill_child,
            )],
            0,
        ));
    let fit = compiler.compile(&fit_parent, 8);
    let fill = compiler.compile(&fill_parent, 8);

    assert_eq!(fit.width, 8);
    assert_eq!(text(&fit.rows[0]), "x");
    assert_eq!(fill.width, 8);
    assert_eq!(text(&fill.rows[0]), "x       ");
}

#[test]
fn spacer_has_zero_intrinsic_width_and_preserves_height() {
    let compiler = ViewCompiler::default();
    let fit = compiler.compile(&crate::presentation::factory::spacer(2), 10);
    let fill = compiler.compile(
        &crate::presentation::factory::fill_width(crate::presentation::factory::spacer(2)),
        10,
    );

    assert_block_shape(&fit, 0, 2);
    assert!(fit.physically_complete);
    assert!(
        fit.rows
            .iter()
            .all(|row| row.cells().iter().all(|cell| !cell.painted))
    );
    assert_block_shape(&fill, 10, 2);
    assert!(fill.physically_complete);
    assert!(
        fill.rows
            .iter()
            .all(|row| row.cells().iter().all(|cell| !cell.painted))
    );

    let zero_fit = compiler.compile(&crate::presentation::factory::spacer(0), 10);
    let zero_fill = compiler.compile(
        &crate::presentation::factory::fill_width(crate::presentation::factory::spacer(0)),
        10,
    );
    assert_block_shape(&zero_fit, 0, 0);
    assert_block_shape(&zero_fill, 10, 0);
}

#[test]
fn zero_width_spacers_contribute_vertical_extent_and_horizontal_height() {
    let compiler = ViewCompiler::default();
    let column = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("a"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::spacer(2),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("b"),
            ),
        ],
        0,
    );
    let row = crate::presentation::factory::row_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::spacer(3),
        )],
        0,
        crate::presentation::VerticalAlign::Top,
    );

    assert_block_shape(&compiler.compile(&column, 10), 1, 4);
    assert_block_shape(&compiler.compile(&row, 10), 0, 3);
}

#[test]
fn empty_flows_have_no_intrinsic_or_gap_geometry() {
    let compiler = ViewCompiler::default();
    let empty_horizontal = crate::presentation::factory::row_specs::<View>(
        vec![],
        50,
        crate::presentation::VerticalAlign::Top,
    );
    let empty_vertical = crate::presentation::factory::column(vec![], 50);
    let filled_horizontal = crate::presentation::factory::fill_width(empty_horizontal.clone());
    let filled_vertical = crate::presentation::factory::fill_width(empty_vertical.clone());

    assert_block_shape(&compiler.compile(&empty_horizontal, 10), 0, 0);
    assert_block_shape(&compiler.compile(&empty_vertical, 10), 0, 0);
    assert_block_shape(&compiler.compile(&filled_horizontal, 10), 10, 0);
    assert_block_shape(&compiler.compile(&filled_vertical, 10), 10, 0);
}

#[test]
fn one_child_gap_has_no_geometry() {
    let compiler = ViewCompiler::default();
    let vertical = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text("x"),
        )],
        0,
    );
    let vertical_gap = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text("x"),
        )],
        50,
    );
    let horizontal = crate::presentation::factory::row_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text("x"),
        )],
        0,
        crate::presentation::VerticalAlign::Top,
    );
    let horizontal_gap = crate::presentation::factory::row_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text("x"),
        )],
        50,
        crate::presentation::VerticalAlign::Top,
    );

    let vertical = compiler.compile(&vertical, 10);
    let vertical_gap = compiler.compile(&vertical_gap, 10);
    let horizontal = compiler.compile(&horizontal, 10);
    let horizontal_gap = compiler.compile(&horizontal_gap, 10);
    assert_eq!(vertical.width, vertical_gap.width);
    assert_eq!(vertical.rows, vertical_gap.rows);
    assert_eq!(horizontal.width, horizontal_gap.width);
    assert_eq!(horizontal.rows, horizontal_gap.rows);
}

#[test]
fn gaps_are_counted_between_all_semantic_children() {
    let compiler = ViewCompiler::default();
    let vertical = crate::presentation::factory::column_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("a"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("b"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("c"),
            ),
        ],
        2,
    );
    let horizontal = crate::presentation::factory::row_specs(
        vec![
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("a"),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::spacer(1),
            ),
            (
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("c"),
            ),
        ],
        2,
        crate::presentation::VerticalAlign::Top,
    );

    assert_block_shape(&compiler.compile(&vertical, 10), 1, 7);
    assert_block_shape(&compiler.compile(&horizontal, 10), 6, 1);
}

#[test]
fn empty_background_does_not_create_geometry() {
    let view = crate::presentation::factory::background(empty_vertical(), ColorSpec::Ansi(1));
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (0, 0));
    assert!(surface.cells.is_empty());
}

#[test]
fn empty_padding_creates_geometry_and_background_paints_it() {
    let view = crate::presentation::factory::background(
        crate::presentation::factory::padding(empty_vertical(), Insets::all(1)),
        ColorSpec::Ansi(1),
    );
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (2, 2));
    assert!(surface.cells.iter().all(|cell| cell.painted));
    assert!(
        surface
            .cells
            .iter()
            .all(|cell| cell.style.background == Some(PhysicalColor::Indexed(1)))
    );
}

#[test]
fn empty_border_geometry_is_safe_at_tiny_widths() {
    let compiler = ViewCompiler::default();
    let view = crate::presentation::factory::border(
        empty_vertical(),
        BorderSpec {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::plain(),
            top_label: None,
        },
    );
    let surface = layout_view(&view, 10, PhysicalStyle::default());
    assert_eq!((surface.width(), surface.height()), (2, 2));
    assert!(surface.cells.iter().all(|cell| cell.painted));

    for width in 0..=2 {
        let _ = compiler.compile(&view, width);
    }
}

#[test]
fn empty_padding_and_border_add_their_outer_geometry() {
    let view = crate::presentation::factory::border(
        crate::presentation::factory::padding(empty_vertical(), Insets::all(1)),
        BorderSpec {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::plain(),
            top_label: None,
        },
    );
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (4, 4));
}

#[test]
fn empty_border_and_background_compose_without_changing_geometry() {
    let view = crate::presentation::factory::border(
        crate::presentation::factory::background(empty_vertical(), ColorSpec::Ansi(1)),
        BorderSpec {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::plain(),
            top_label: None,
        },
    );
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (2, 2));
    assert!(
        surface
            .cells
            .iter()
            .all(|cell| cell.style.background == Some(PhysicalColor::Indexed(1)))
    );
}

#[test]
fn fixed_track_preserves_parent_width_and_child_sizing_intent() {
    let compiler = ViewCompiler::default();
    let fit_child = crate::presentation::factory::background(
        crate::presentation::factory::fit_width(crate::presentation::factory::text("x")),
        ColorSpec::Ansi(1),
    );
    let fill_child = crate::presentation::factory::background(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        ColorSpec::Ansi(1),
    );
    let fit = crate::presentation::factory::row_specs(
        vec![(crate::presentation::ir::TrackSize::Fixed(5), fit_child)],
        0,
        crate::presentation::VerticalAlign::Top,
    );
    let fill = crate::presentation::factory::row_specs(
        vec![(crate::presentation::ir::TrackSize::Fixed(5), fill_child)],
        0,
        crate::presentation::VerticalAlign::Top,
    );

    let fit = compiler.compile(&fit, 10);
    let fill = compiler.compile(&fill, 10);
    assert_eq!(fit.width, 5);
    assert_eq!(fill.width, 5);
    assert_eq!(text(&fit.rows[0]), "x");
    assert_eq!(text(&fill.rows[0]), "x    ");
}

#[test]
fn clamp_and_container_preserve_zero_width_vertical_extent() {
    let compiler = ViewCompiler::default();
    let spacer = crate::presentation::factory::spacer(3);
    let container = box_view(spacer.clone(), Decoration::default());
    let clamped = crate::presentation::factory::clamp_rows(spacer, 4, OverflowIndicator::None);

    assert_block_shape(&compiler.compile(&container, 10), 0, 3);
    assert_block_shape(&compiler.compile(&clamped, 10), 0, 3);
}

#[test]
fn clamp_zero_rows_remains_safe_for_empty_child() {
    let compiler = ViewCompiler::default();
    let view = crate::presentation::factory::clamp_rows(
        crate::presentation::factory::column(vec![], 0),
        0,
        OverflowIndicator::None,
    );
    assert_block_shape(&compiler.compile(&view, 10), 0, 0);
}

#[test]
fn clamp_emits_indicator() {
    let view = crate::presentation::factory::clamp_rows(
        crate::presentation::factory::text("one two three four"),
        2,
        crate::presentation::api::style::OverflowIndicator::Ellipsis {
            style: StyleSpec::default().into(),
        },
    );
    let rows = compile_view(&view, 4).rows;
    assert_eq!(rows.len(), 2);
    assert!(text(&rows[1]).contains('…'));
}
