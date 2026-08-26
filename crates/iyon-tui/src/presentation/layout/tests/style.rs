use super::*;

#[test]
fn local_view_facts_match_only_their_own_node() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.role", "heading"),
        StyleSpec::new().bold(),
    );
    let parent = View::vertical(|column| {
        column.child(View::text("x").style(StyleRef::theme("probe")));
    })
    .into_view()
    .style_fact("test.role", "heading");
    let surface = {
        let compiler = ViewCompiler::new(&theme);
        let tree = compiler.layout_tree(&parent, LayoutConstraints::width_only(1));
        ViewPainter.paint_tree(&compiler, &tree)
    };
    assert!(!surface.get(0, 0).style.bold);
}

#[test]
fn physical_style_inherits_after_local_fact_resolution() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.role", "heading"),
        StyleSpec::new().bold(),
    );
    let parent = View::vertical(|column| {
        column.child("x");
    })
    .style(StyleRef::theme("probe"))
    .into_view()
    .style_fact("test.role", "heading");
    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&parent, LayoutConstraints::width_only(1));
    let surface = ViewPainter.paint_tree(&compiler, &tree);
    assert!(surface.get(0, 0).style.bold);
}

#[test]
fn view_facts_do_not_leak_into_spans() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.role", "heading"),
        StyleSpec::new().bold(),
    );
    let view = View::styled_text([TextSpan::styled("x", StyleRef::theme("probe"))])
        .into_view()
        .style_fact("test.role", "heading");
    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&view, LayoutConstraints::width_only(1));
    let surface = ViewPainter.paint_tree(&compiler, &tree);
    assert!(!surface.get(0, 0).style.bold);
}

#[test]
fn span_facts_compose_with_inherited_state() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.mode", "warning").and_state("test.role", "strong"),
        StyleSpec::new().bold(),
    );
    let view = View::styled_text([
        TextSpan::styled("x", StyleRef::theme("probe")).style_fact("test.role", "strong")
    ])
    .style_state("test.mode", "warning")
    .into_view();
    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&view, LayoutConstraints::width_only(1));
    let surface = ViewPainter.paint_tree(&compiler, &tree);
    assert!(surface.get(0, 0).style.bold);
}

#[test]
fn local_fact_shadows_same_key_state_but_descendant_sees_state_again() {
    let theme = Theme::new()
        .with_style_variant(
            "probe",
            StyleSelector::state("test.kind", "parent"),
            StyleSpec::new().italic(),
        )
        .with_style_variant(
            "probe",
            StyleSelector::state("test.kind", "child"),
            StyleSpec::new().attribute(TextAttribute::Italic, false),
        );
    let child = View::vertical(|column| {
        column.child("c");
        column.child(View::vertical(|nested| {
            nested.child(View::text("g").style(StyleRef::theme("probe")));
        }));
    })
    .style(StyleRef::theme("probe"))
    .style_fact("test.kind", "child");
    let root = View::vertical(|column| {
        column.child(child);
    })
    .style_state("test.kind", "parent");

    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&root, LayoutConstraints::width_only(1));
    let surface = ViewPainter.paint_tree(&compiler, &tree);

    assert!(!surface.get(0, 0).style.italic);
    assert!(surface.get(0, 1).style.italic);
}

#[test]
fn span_fact_overrides_parent_fact_resolved_physical_style_without_leaking() {
    let theme = Theme::new()
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "heading"),
            StyleSpec::new().attribute(TextAttribute::Bold, false),
        )
        .with_style_variant(
            "probe",
            StyleSelector::state("test.role", "strong"),
            StyleSpec::new().bold(),
        );
    let view = View::styled_text([
        TextSpan::styled("plain", StyleRef::theme("probe")),
        TextSpan::styled("strong", StyleRef::theme("probe")).style_fact("test.role", "strong"),
    ])
    .style(StyleRef::theme("probe"))
    .into_view()
    .style_fact("test.role", "heading");

    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&view, LayoutConstraints::width_only(11));
    let surface = ViewPainter.paint_tree(&compiler, &tree);

    assert!(!surface.get(0, 0).style.bold);
    assert!(surface.get(5, 0).style.bold);
}

#[test]
fn ancestor_and_child_text_styles_cascade_to_physical_text() {
    let child = View::text("x").foreground(ColorSpec::Ansi(2)).into_view();
    let view = box_view(
        child,
        Decoration {
            text_style: StyleSpec::new().foreground(ColorSpec::Ansi(1)).into(),
            ..Decoration::default()
        },
    );

    let surface = layout_view(&view, 1, PhysicalStyle::default());
    assert_eq!(
        surface.get(0, 0).style.foreground,
        Some(PhysicalColor::Indexed(2))
    );
}

#[test]
fn span_style_overrides_node_and_explicit_false_cascades() {
    let child = View::styled_text(vec![
        TextSpan::plain("a"),
        TextSpan::styled("b", StyleSpec::new().bold()),
    ])
    .text_attribute(TextAttribute::Bold, false)
    .into_view();
    let view = box_view(
        child,
        Decoration {
            text_style: StyleSpec::new().bold().into(),
            ..Decoration::default()
        },
    );

    let surface = layout_view(&view, 2, PhysicalStyle::default());
    assert!(!surface.get(0, 0).style.bold);
    assert!(surface.get(1, 0).style.bold);
}

#[test]
fn surface_background_paints_text_backing_and_transparent_tail() {
    let view = View::text("x")
        .fill_width()
        .background(ColorSpec::Ansi(1))
        .into_view();
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(
        surface.get(3, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert!(surface.get(3, 0).painted);
}

#[test]
fn final_surface_background_api_paints_text_and_tail() {
    let view = View::text("x")
        .fill_width()
        .background(ColorSpec::ansi(1))
        .into_view();
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(
        surface.get(3, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn final_text_style_background_only_paints_text_cells() {
    let view = View::text("x")
        .fill_width()
        .style(StyleSpec::new().background(ColorSpec::ansi(1)))
        .into_view();
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(surface.get(3, 0).style.background, None);
}

#[test]
fn final_foreground_api_inherits_to_descendant_text() {
    let view = View::vertical(|column| {
        column.child("hello");
    })
    .foreground(ColorSpec::ansi(1));
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.foreground,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn strikethrough_inherits_and_can_be_cancelled_or_reenabled_by_a_span() {
    let inherited = View::vertical(|column| {
        column.child("x");
    })
    .strikethrough();
    let cancelled = View::vertical(|column| {
        column.child(View::text("x").text_attribute(TextAttribute::Strikethrough, false));
    })
    .strikethrough();
    let reenabled = View::vertical(|column| {
        column.child(View::styled_text([TextSpan::styled(
            "x",
            StyleSpec::new().strikethrough(),
        )]));
    })
    .text_attribute(TextAttribute::Strikethrough, false)
    .into_view();

    assert!(
        layout_view(&inherited, 1, PhysicalStyle::default())
            .get(0, 0)
            .style
            .strikethrough
    );
    assert!(
        !layout_view(&cancelled, 1, PhysicalStyle::default())
            .get(0, 0)
            .style
            .strikethrough
    );
    assert!(
        layout_view(&reenabled, 1, PhysicalStyle::default())
            .get(0, 0)
            .style
            .strikethrough
    );
}

#[test]
fn final_attribute_api_supports_false_and_specific_child_override() {
    let inherited_bold_cancelled = View::vertical(|column| {
        column.child(View::text("x").text_attribute(TextAttribute::Bold, false));
    })
    .bold();
    let child_bold = View::vertical(|column| {
        column.child(View::text("x").bold());
    })
    .text_attribute(TextAttribute::Bold, false);

    let cancelled = layout_view(&inherited_bold_cancelled, 1, PhysicalStyle::default());
    let overridden = layout_view(&child_bold, 1, PhysicalStyle::default());
    assert!(!cancelled.get(0, 0).style.bold);
    assert!(overridden.get(0, 0).style.bold);
}

#[test]
fn final_span_style_remains_more_specific_than_node_foreground() {
    let view = View::styled_text([
        TextSpan::plain("a"),
        TextSpan::styled("b", StyleSpec::new().foreground(ColorSpec::ansi(2))),
    ])
    .foreground(ColorSpec::ansi(1))
    .into_view();
    let surface = layout_view(&view, 2, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.foreground,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(
        surface.get(1, 0).style.foreground,
        Some(PhysicalColor::Indexed(2))
    );
}

#[test]
fn final_empty_properties_use_existing_geometry_rules() {
    let compiler = ViewCompiler::default();
    let empty = View::vertical(|_| {});
    let background = empty.clone().background(ColorSpec::ansi(1));
    let padding = empty.clone().padding(1);
    let border = empty.clone().border(BorderSpec::plain());
    let combined = empty
        .padding(1)
        .border(BorderSpec::plain())
        .background(ColorSpec::ansi(1));

    assert_block_shape(&compiler.compile(&background, 10), 0, 0);
    assert_block_shape(&compiler.compile(&padding, 10), 2, 2);
    assert_block_shape(&compiler.compile(&border, 10), 2, 2);
    assert_block_shape(&compiler.compile(&combined, 10), 4, 4);
}

#[test]
fn final_border_api_preserves_surface_background_and_border_color() {
    let view = View::vertical(|_| {})
        .background(ColorSpec::ansi(1))
        .border(BorderSpec::plain().color(ColorSpec::ansi(2)));
    let surface = layout_view(&view, 10, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.foreground,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn final_structural_order_affects_clamp_geometry() {
    let padded_then_clamped = View::text("x")
        .padding(1)
        .clamp_rows(1, OverflowIndicator::None);
    let clamped_then_padded = View::text("x")
        .clamp_rows(1, OverflowIndicator::None)
        .padding(1);
    let compiler = ViewCompiler::default();

    assert_eq!(compiler.compile(&padded_then_clamped, 10).rows.len(), 1);
    assert_eq!(compiler.compile(&clamped_then_padded, 10).rows.len(), 3);
}

#[test]
fn border_glyphs_enforce_one_cell_semantics() {
    assert!(BorderGlyphs::new("─", "│", "─", "│", "┌", "┐", "└", "┘").is_ok());
    assert!(BorderGlyphs::new("e\u{301}", "│", "─", "│", "┌", "┐", "└", "┘",).is_ok());
    let error = BorderGlyphs::new("界", "│", "─", "│", "┌", "┐", "└", "┘").unwrap_err();
    assert_eq!(error.field, "top");
    assert_eq!(error.width, 2);
}

#[test]
fn border_labels_use_display_width_and_clip_to_the_top_edge() {
    let view = View::text("x")
        .border(BorderSpec::plain().top_label("界界界"))
        .fill_width()
        .into_view();
    let row = &ViewCompiler::default().compile(&view, 5).rows[0];
    assert_eq!(row.cell(0).unwrap().grapheme.as_deref(), Some("界"));
    assert!(row.cell(1).unwrap().continuation);
    assert_eq!(row.cell(2).unwrap().grapheme.as_deref(), Some("界"));
    assert!(row.cell(3).unwrap().continuation);
    assert_eq!(row.cell(4).unwrap().grapheme.as_deref(), Some("┐"));
}

#[test]
fn explicit_border_constructor_uses_rounded_glyphs() {
    let view = View::text("x").border(BorderSpec::rounded().color(ColorSpec::ansi(2)));
    let rows = ViewCompiler::default().compile(&view.into_view(), 5).rows;
    assert!(text(&rows[0]).starts_with('╭'));
    assert_eq!(
        rows[0].style_at(0).and_then(|style| style.foreground),
        Some(PhysicalColor::Indexed(2))
    );
}

#[test]
fn explicit_style_properties_merge_without_losing_fields() {
    let view = View::text("x")
        .foreground(ColorSpec::ansi(1))
        .bold()
        .style(StyleSpec::new().italic())
        .into_view();

    assert_eq!(
        view.decoration().text_style.foreground,
        Some(ColorSpec::ansi(1))
    );
    assert_eq!(view.decoration().text_style.attributes.bold, Some(true));
    assert_eq!(view.decoration().text_style.attributes.italic, Some(true));
}

#[test]
fn explicit_border_color_preserves_surface_background() {
    let mut decoration = background_decoration(ColorSpec::Ansi(1));
    decoration.border = Some(BorderSpec {
        style: BorderStyle::Plain,
        color: Some(ColorSpec::Ansi(2)),
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let view = box_view(View::text("x").fill_width().into_view(), decoration).fill_width();
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    let border = surface.get(0, 1).style;
    assert_eq!(border.foreground, Some(PhysicalColor::Indexed(2)));
    assert_eq!(border.background, Some(PhysicalColor::Indexed(1)));
}

#[test]
fn implicit_border_color_preserves_surface_background_and_inherits_foreground() {
    let mut decoration = background_decoration(ColorSpec::Ansi(1));
    decoration.text_style = StyleSpec::new().foreground(ColorSpec::Ansi(2)).into();
    decoration.border = Some(BorderSpec {
        style: BorderStyle::Plain,
        color: None,
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let view = box_view(View::text("x").fill_width().into_view(), decoration).fill_width();
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    let border = surface.get(0, 1).style;
    assert_eq!(border.foreground, Some(PhysicalColor::Indexed(2)));
    assert_eq!(border.background, Some(PhysicalColor::Indexed(1)));
}

#[test]
fn text_background_does_not_leak_into_border() {
    let mut decoration = Decoration::default();
    decoration.text_style = StyleSpec::new().background(ColorSpec::Ansi(2)).into();
    decoration.border = Some(BorderSpec {
        style: BorderStyle::Plain,
        color: None,
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let view = box_view(View::text("x").fill_width().into_view(), decoration).fill_width();
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(1, 1).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(surface.get(0, 1).style.background, None);
}

#[test]
fn surface_and_text_backgrounds_coexist_across_border_and_content() {
    let mut decoration = background_decoration(ColorSpec::Ansi(1));
    decoration.text_style = StyleSpec::new().background(ColorSpec::Ansi(2)).into();
    decoration.border = Some(BorderSpec {
        style: BorderStyle::Plain,
        color: None,
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let view = box_view(View::text("x").fill_width().into_view(), decoration).fill_width();
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(1, 1).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(
        surface.get(0, 1).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(
        surface.get(4, 1).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn border_painting_preserves_tiny_width_geometry() {
    let mut decoration = background_decoration(ColorSpec::Ansi(1));
    decoration.border = Some(BorderSpec {
        style: BorderStyle::Plain,
        color: Some(ColorSpec::Ansi(2)),
        edges: BorderEdges::ALL,
        glyphs: BorderGlyphs::plain(),
        top_label: None,
    });
    let view = box_view(View::text("x").into_view(), decoration).fill_width();

    for width in [0, 1, 2, 3, 10] {
        let block = compile_view(&view, width);
        assert!(block.width <= width);
        assert!(
            block
                .rows
                .iter()
                .all(|row| row.width() <= usize::from(width))
        );
    }
}

#[test]
fn text_background_only_paints_text_cells() {
    let view = View::text("x")
        .fill_width()
        .style(StyleSpec::new().background(ColorSpec::Ansi(2)))
        .into_view();
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert!(!surface.get(3, 0).painted);
}

#[test]
fn explicit_text_background_wins_over_surface_background() {
    let view = View::text("x")
        .fill_width()
        .background(ColorSpec::Ansi(1))
        .style(StyleSpec::new().background(ColorSpec::Ansi(2)))
        .into_view();
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(
        surface.get(3, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn nested_surface_backgrounds_preserve_child_region() {
    let child = box_view(
        View::text("x").into_view(),
        background_decoration(ColorSpec::Ansi(2)),
    );
    let outer = box_view(child, background_decoration(ColorSpec::Ansi(1))).fill_width();
    let surface = layout_view(&outer, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(
        surface.get(3, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn transparent_padding_shows_ancestor_surface_background() {
    let child = box_view(View::text("x").into_view(), {
        let mut decoration = Decoration::default();
        decoration.padding = Insets::all(1);
        decoration
    });
    let outer = box_view(child, background_decoration(ColorSpec::Ansi(1))).fill_width();
    let surface = layout_view(&outer, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn surface_background_does_not_enter_text_style_cascade() {
    let view = View::text("x").background(ColorSpec::Ansi(1)).into_view();
    let resolved = ViewCompiler::default().theme.resolve_text_style(
        PhysicalStyle::default(),
        &view.decoration().text_style,
        &crate::presentation::paint::StyleContext::default(),
    );
    assert_eq!(resolved.background, None);
}

#[test]
fn default_decoration_keeps_core_tails_transparent() {
    let views = [
        View::text("a").fill_width().into_view(),
        View::column(vec![View::text("a").fill_width().into_view()], 0),
        row_view(vec![RowChild::content(View::text("a").into_view())], 0),
        View::spacer(1).fill_width(),
        View::text("a")
            .fill_width()
            .clamp_rows(1, OverflowIndicator::None),
    ];

    for (index, view) in views.into_iter().enumerate() {
        let surface = layout_view(&view, 4, PhysicalStyle::default());
        if index == 3 {
            assert!(surface.cells.iter().all(|cell| !cell.painted));
        } else {
            assert!(surface.cells.iter().any(|cell| cell.painted));
            assert!(!surface.get(3, 0).painted);
        }
    }
}

#[test]
fn decorated_shell_paints_through_transparent_core() {
    let view = box_view(
        View::spacer(1).fill_width(),
        background_decoration(ColorSpec::Ansi(1)),
    );
    let surface = layout_view(&view, 3, PhysicalStyle::default());

    assert!(surface.get(0, 0).painted);
    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn explicit_child_paint_wins_over_outer_background() {
    let child = View::styled_text(vec![TextSpan::styled(
        "x",
        StyleSpec {
            background: Some(ColorSpec::Ansi(2)),
            ..StyleSpec::default()
        },
    )])
    .fill_width()
    .into_view();
    let view = box_view(child, background_decoration(ColorSpec::Ansi(1)));
    let surface = layout_view(&view, 3, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert_eq!(
        surface.get(2, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn decoration_preserves_physical_incompleteness() {
    let view = box_view(
        View::text("漢").into_view(),
        background_decoration(ColorSpec::Ansi(1)),
    );
    let compiler = ViewCompiler::default();

    assert!(!compiler.compile(&view, 1).physically_complete);
    assert!(compiler.compile(&view, 2).physically_complete);
}

#[test]
fn box_background_covers_padding_and_row_gap() {
    let view = box_view(
        decorated_row_view("body"),
        background_with_padding(ColorSpec::Theme(ThemeKey::from("panel")), Insets::all(1)),
    );
    let rows = compile_view(&view, 12).rows;
    assert!(rows.iter().all(|row| {
        row.cells()
            .iter()
            .any(|cell| cell.style.background.is_some())
    }));
}
