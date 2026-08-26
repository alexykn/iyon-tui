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
                RowChild::fixed(3, View::text("abc").into_view()),
                RowChild::flex(View::text("body").fill_width().into_view()),
                RowChild::content(View::text("status").into_view()),
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
        let plain = View::text("x").into_view();
        let wrapped = plain.clone().container();
        let plain_block = compiler.compile(&plain, width);
        let wrapped_block = compiler.compile(&wrapped, width);
        assert_eq!(wrapped_block.width, plain_block.width);
        assert_eq!(wrapped_block.rows, plain_block.rows);
        assert_eq!(
            wrapped_block.physically_complete,
            plain_block.physically_complete
        );
    }

    let spacer = View::spacer(3).container();
    assert_block_shape(&compiler.compile(&spacer, 10), 0, 3);
}

#[test]
fn final_clamp_preserves_zero_width_vertical_extent() {
    let view = View::spacer(3).clamp_rows(4, OverflowIndicator::None);
    assert_block_shape(&ViewCompiler::default().compile(&view, 10), 0, 3);
}

#[test]
fn width_defaults_and_explicit_fill_are_intrinsic_and_allocated() {
    let compiler = ViewCompiler::default();
    let horizontal = View::horizontal(|row| {
        row.child("ab");
        row.child("cde");
        row.gap(1);
    });
    let vertical = View::vertical(|column| {
        column.child("ab");
        column.child("abcde");
    });

    assert_block_shape(&compiler.compile(&horizontal, 10), 6, 1);
    assert_block_shape(&compiler.compile(&horizontal.fill_width(), 10), 10, 1);
    assert_block_shape(&compiler.compile(&vertical, 10), 5, 2);
    assert_block_shape(&compiler.compile(&vertical.fill_width(), 10), 10, 2);
}

#[test]
fn hanging_preserves_body_width_policy_within_bounded_column() {
    let compiler = ViewCompiler::default();
    let fit = View::hanging(
        View::text("• ").no_wrap(),
        View::text("  ").no_wrap(),
        View::text("x").fit_width(),
    )
    .fill_width();
    let fill = View::hanging(
        View::text("• ").no_wrap(),
        View::text("  ").no_wrap(),
        View::text("x").fill_width(),
    )
    .fill_width();

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
    let fit_child = View::text("x")
        .fit_width()
        .background(ColorSpec::Ansi(1))
        .into_view();
    let fill_child = View::text("x")
        .fill_width()
        .background(ColorSpec::Ansi(1))
        .into_view();

    let fit_parent = View::vertical(|column| {
        column.child(fit_child);
    })
    .fill_width();
    let fill_parent = View::vertical(|column| {
        column.child(fill_child);
    })
    .fill_width();
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
    let fit = compiler.compile(&View::spacer(2), 10);
    let fill = compiler.compile(&View::spacer(2).fill_width(), 10);

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

    let zero_fit = compiler.compile(&View::spacer(0), 10);
    let zero_fill = compiler.compile(&View::spacer(0).fill_width(), 10);
    assert_block_shape(&zero_fit, 0, 0);
    assert_block_shape(&zero_fill, 10, 0);
}

#[test]
fn zero_width_spacers_contribute_vertical_extent_and_horizontal_height() {
    let compiler = ViewCompiler::default();
    let column = View::vertical(|column| {
        column.child("a");
        column.child(View::spacer(2));
        column.child("b");
    });
    let row = View::horizontal(|row| {
        row.child(View::spacer(3));
    });

    assert_block_shape(&compiler.compile(&column, 10), 1, 4);
    assert_block_shape(&compiler.compile(&row, 10), 0, 3);
}

#[test]
fn empty_flows_have_no_intrinsic_or_gap_geometry() {
    let compiler = ViewCompiler::default();
    let empty_horizontal = View::horizontal(|row| {
        row.gap(50);
    });
    let empty_vertical = View::vertical(|column| {
        column.gap(50);
    });
    let filled_horizontal = empty_horizontal.clone().fill_width();
    let filled_vertical = empty_vertical.clone().fill_width();

    assert_block_shape(&compiler.compile(&empty_horizontal, 10), 0, 0);
    assert_block_shape(&compiler.compile(&empty_vertical, 10), 0, 0);
    assert_block_shape(&compiler.compile(&filled_horizontal, 10), 10, 0);
    assert_block_shape(&compiler.compile(&filled_vertical, 10), 10, 0);
}

#[test]
fn one_child_gap_has_no_geometry() {
    let compiler = ViewCompiler::default();
    let vertical = View::vertical(|column| {
        column.child("x");
    });
    let vertical_gap = View::vertical(|column| {
        column.child("x");
        column.gap(50);
    });
    let horizontal = View::horizontal(|row| {
        row.child("x");
    });
    let horizontal_gap = View::horizontal(|row| {
        row.child("x");
        row.gap(50);
    });

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
    let vertical = View::vertical(|column| {
        column.child("a");
        column.child("b");
        column.child("c");
        column.gap(2);
    });
    let horizontal = View::horizontal(|row| {
        row.child("a");
        row.child(View::spacer(1));
        row.child("c");
        row.gap(2);
    });

    assert_block_shape(&compiler.compile(&vertical, 10), 1, 7);
    assert_block_shape(&compiler.compile(&horizontal, 10), 6, 1);
}

#[test]
fn empty_background_does_not_create_geometry() {
    let view = empty_vertical().background(ColorSpec::Ansi(1));
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (0, 0));
    assert!(surface.cells.is_empty());
}

#[test]
fn empty_padding_creates_geometry_and_background_paints_it() {
    let view = empty_vertical()
        .padding(Insets::all(1))
        .background(ColorSpec::Ansi(1));
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
    let view = empty_vertical().border(BorderSpec {
        style: BorderStyle::Plain,
        color: None,
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let surface = layout_view(&view, 10, PhysicalStyle::default());
    assert_eq!((surface.width(), surface.height()), (2, 2));
    assert!(surface.cells.iter().all(|cell| cell.painted));

    for width in 0..=2 {
        let _ = compiler.compile(&view, width);
    }
}

#[test]
fn empty_padding_and_border_add_their_outer_geometry() {
    let view = empty_vertical().padding(Insets::all(1)).border(BorderSpec {
        style: BorderStyle::Plain,
        color: None,
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!((surface.width(), surface.height()), (4, 4));
}

#[test]
fn empty_border_and_background_compose_without_changing_geometry() {
    let view = empty_vertical()
        .background(ColorSpec::Ansi(1))
        .border(BorderSpec {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::plain(),
            top_label: None,
        });
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
    let fit_child = View::text("x")
        .fit_width()
        .background(ColorSpec::Ansi(1))
        .into_view();
    let fill_child = View::text("x")
        .fill_width()
        .background(ColorSpec::Ansi(1))
        .into_view();
    let fit = View::horizontal(|row| {
        row.fixed(5, fit_child);
    });
    let fill = View::horizontal(|row| {
        row.fixed(5, fill_child);
    });

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
    let spacer = View::spacer(3);
    let container = box_view(spacer.clone(), Decoration::default());
    let clamped = spacer.clamp_rows(4, OverflowIndicator::None);

    assert_block_shape(&compiler.compile(&container, 10), 0, 3);
    assert_block_shape(&compiler.compile(&clamped, 10), 0, 3);
}

#[test]
fn clamp_zero_rows_remains_safe_for_empty_child() {
    let compiler = ViewCompiler::default();
    let view = View::vertical(|_| {}).clamp_rows(0, OverflowIndicator::None);
    assert_block_shape(&compiler.compile(&view, 10), 0, 0);
}

#[test]
fn clamp_emits_indicator() {
    let view = View::text("one two three four").clamp_rows(
        2,
        crate::presentation::api::style::OverflowIndicator::Ellipsis {
            style: StyleSpec::default().into(),
        },
    );
    let rows = compile_view(&view, 4).rows;
    assert_eq!(rows.len(), 2);
    assert!(text(&rows[1]).contains('…'));
}
