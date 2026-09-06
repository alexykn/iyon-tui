use super::*;

#[test]
fn local_view_facts_match_only_their_own_node() {
    let theme = Theme::new().with_style_variant(
        "probe",
        StyleSelector::state("test.role", "heading"),
        StyleSpec::new().bold(),
    );
    let parent = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::style(
                crate::presentation::factory::text("x"),
                StyleRef::theme("probe"),
            ),
        )],
        0,
    );
    let parent = crate::presentation::factory::style_fact(parent, "test.role", "heading");
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
    let parent = crate::presentation::factory::style(
        crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("x"),
            )],
            0,
        ),
        StyleRef::theme("probe"),
    );
    let parent = crate::presentation::factory::style_fact(parent, "test.role", "heading");
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
    let view = crate::presentation::factory::styled_text([TextSpan::styled(
        "x",
        StyleRef::theme("probe"),
    )]);
    let view = crate::presentation::factory::style_fact(view, "test.role", "heading");
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
    let view = crate::presentation::factory::styled_text([TextSpan::styled(
        "x",
        StyleRef::theme("probe"),
    )
    .style_fact("test.role", "strong")]);
    let view = crate::presentation::factory::style_state(view, "test.mode", "warning");
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
    let child = crate::presentation::factory::style(
        crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::text("c"),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    crate::presentation::factory::column_specs(
                        vec![(
                            crate::presentation::ir::TrackSize::Content { max: None },
                            crate::presentation::factory::style(
                                crate::presentation::factory::text("g"),
                                StyleRef::theme("probe"),
                            ),
                        )],
                        0,
                    ),
                ),
            ],
            0,
        ),
        StyleRef::theme("probe"),
    );
    let child = crate::presentation::factory::style_fact(child, "test.kind", "child");
    let root = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            child,
        )],
        0,
    );
    let root = crate::presentation::factory::style_state(root, "test.kind", "parent");

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
    let view = crate::presentation::factory::style(
        crate::presentation::factory::styled_text([
            TextSpan::styled("plain", StyleRef::theme("probe")),
            TextSpan::styled("strong", StyleRef::theme("probe")).style_fact("test.role", "strong"),
        ]),
        StyleRef::theme("probe"),
    );
    let view = crate::presentation::factory::style_fact(view, "test.role", "heading");

    let compiler = ViewCompiler::new(&theme);
    let tree = compiler.layout_tree(&view, LayoutConstraints::width_only(11));
    let surface = ViewPainter.paint_tree(&compiler, &tree);

    assert!(!surface.get(0, 0).style.bold);
    assert!(surface.get(5, 0).style.bold);
}

#[test]
fn ancestor_and_child_text_styles_cascade_to_physical_text() {
    let child = crate::presentation::factory::foreground(
        crate::presentation::factory::text("x"),
        ColorSpec::Ansi(2),
    );
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
    let child = crate::presentation::factory::text_attribute(
        crate::presentation::factory::styled_text(vec![
            TextSpan::plain("a"),
            TextSpan::styled("b", StyleSpec::new().bold()),
        ]),
        TextAttribute::Bold,
        false,
    );
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
    let view = crate::presentation::factory::background(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        ColorSpec::Ansi(1),
    );
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
    let view = crate::presentation::factory::background(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        ColorSpec::ansi(1),
    );
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
    let view = crate::presentation::factory::style(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        StyleSpec::new().background(ColorSpec::ansi(1)),
    );
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
    assert_eq!(surface.get(3, 0).style.background, None);
}

#[test]
fn final_foreground_api_inherits_to_descendant_text() {
    let view = crate::presentation::factory::foreground(
        crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("hello"),
            )],
            0,
        ),
        ColorSpec::ansi(1),
    );
    let surface = layout_view(&view, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.foreground,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn strikethrough_inherits_and_can_be_cancelled_or_reenabled_by_a_span() {
    let inherited = crate::presentation::factory::text_attribute(
        crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text("x"),
            )],
            0,
        ),
        TextAttribute::Strikethrough,
        true,
    );
    let cancelled = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text_attribute(
                crate::presentation::factory::text("x"),
                TextAttribute::Strikethrough,
                false,
            ),
        )],
        0,
    );
    let cancelled =
        crate::presentation::factory::text_attribute(cancelled, TextAttribute::Strikethrough, true);
    let reenabled = crate::presentation::factory::text_attribute(
        crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::styled_text([TextSpan::styled(
                    "x",
                    StyleSpec::new().strikethrough(),
                )]),
            )],
            0,
        ),
        TextAttribute::Strikethrough,
        false,
    );

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
    let inherited_bold_cancelled = crate::presentation::factory::text_attribute(
        crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::text_attribute(
                    crate::presentation::factory::text("x"),
                    TextAttribute::Bold,
                    false,
                ),
            )],
            0,
        ),
        TextAttribute::Bold,
        true,
    );
    let child_bold = crate::presentation::factory::column_specs(
        vec![(
            crate::presentation::ir::TrackSize::Content { max: None },
            crate::presentation::factory::text_attribute(
                crate::presentation::factory::text("x"),
                TextAttribute::Bold,
                true,
            ),
        )],
        0,
    );
    let child_bold =
        crate::presentation::factory::text_attribute(child_bold, TextAttribute::Bold, false);

    let cancelled = layout_view(&inherited_bold_cancelled, 1, PhysicalStyle::default());
    let overridden = layout_view(&child_bold, 1, PhysicalStyle::default());
    assert!(!cancelled.get(0, 0).style.bold);
    assert!(overridden.get(0, 0).style.bold);
}

#[test]
fn final_span_style_remains_more_specific_than_node_foreground() {
    let view = crate::presentation::factory::foreground(
        crate::presentation::factory::styled_text([
            TextSpan::plain("a"),
            TextSpan::styled("b", StyleSpec::new().foreground(ColorSpec::ansi(2))),
        ]),
        ColorSpec::ansi(1),
    );
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
    let empty = crate::presentation::factory::column(vec![], 0);
    let background = crate::presentation::factory::background(empty.clone(), ColorSpec::ansi(1));
    let padding = crate::presentation::factory::padding(empty.clone(), 1);
    let border = crate::presentation::factory::border(empty.clone(), BorderSpec::plain());
    let combined = crate::presentation::factory::background(
        crate::presentation::factory::border(
            crate::presentation::factory::padding(empty, 1),
            BorderSpec::plain(),
        ),
        ColorSpec::ansi(1),
    );

    assert_block_shape(&compiler.compile(&background, 10), 0, 0);
    assert_block_shape(&compiler.compile(&padding, 10), 2, 2);
    assert_block_shape(&compiler.compile(&border, 10), 2, 2);
    assert_block_shape(&compiler.compile(&combined, 10), 4, 4);
}

#[test]
fn final_border_api_preserves_surface_background_and_border_color() {
    let view = crate::presentation::factory::border(
        crate::presentation::factory::background(
            crate::presentation::factory::column(vec![], 0),
            ColorSpec::ansi(1),
        ),
        BorderSpec::plain().color(ColorSpec::ansi(2)),
    );
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
    let padded_then_clamped = crate::presentation::factory::clamp_rows(
        crate::presentation::factory::padding(crate::presentation::factory::text("x"), 1),
        1,
        OverflowIndicator::None,
    );
    let clamped_then_padded = crate::presentation::factory::padding(
        crate::presentation::factory::clamp_rows(
            crate::presentation::factory::text("x"),
            1,
            OverflowIndicator::None,
        ),
        1,
    );
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
    let view = crate::presentation::factory::fill_width(crate::presentation::factory::border(
        crate::presentation::factory::text("x"),
        BorderSpec::plain().top_label("界界界"),
    ));
    let row = &ViewCompiler::default().compile(&view, 5).rows[0];
    assert_eq!(row.cell(0).unwrap().grapheme.as_deref(), Some("界"));
    assert!(row.cell(1).unwrap().continuation);
    assert_eq!(row.cell(2).unwrap().grapheme.as_deref(), Some("界"));
    assert!(row.cell(3).unwrap().continuation);
    assert_eq!(row.cell(4).unwrap().grapheme.as_deref(), Some("┐"));
}

#[test]
fn explicit_border_constructor_uses_rounded_glyphs() {
    let view = crate::presentation::factory::border(
        crate::presentation::factory::text("x"),
        BorderSpec::rounded().color(ColorSpec::ansi(2)),
    );
    let rows = ViewCompiler::default().compile(&view, 5).rows;
    assert!(text(&rows[0]).starts_with('╭'));
    assert_eq!(
        rows[0].style_at(0).and_then(|style| style.foreground),
        Some(PhysicalColor::Indexed(2))
    );
}

#[test]
fn explicit_style_properties_merge_without_losing_fields() {
    let view = crate::presentation::factory::style(
        crate::presentation::factory::text_attribute(
            crate::presentation::factory::foreground(
                crate::presentation::factory::text("x"),
                ColorSpec::ansi(1),
            ),
            TextAttribute::Bold,
            true,
        ),
        StyleSpec::new().italic(),
    );

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
    let view = crate::presentation::factory::fill_width(box_view(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        decoration,
    ));
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
    let view = crate::presentation::factory::fill_width(box_view(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        decoration,
    ));
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
    let view = crate::presentation::factory::fill_width(box_view(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        decoration,
    ));
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
    let view = crate::presentation::factory::fill_width(box_view(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        decoration,
    ));
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
    let view = crate::presentation::factory::fill_width(box_view(
        crate::presentation::factory::text("x"),
        decoration,
    ));

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
    let view = crate::presentation::factory::style(
        crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
        StyleSpec::new().background(ColorSpec::Ansi(2)),
    );
    let surface = layout_view(&view, 4, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(2))
    );
    assert!(!surface.get(3, 0).painted);
}

#[test]
fn explicit_text_background_wins_over_surface_background() {
    let view = crate::presentation::factory::style(
        crate::presentation::factory::background(
            crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
            ColorSpec::Ansi(1),
        ),
        StyleSpec::new().background(ColorSpec::Ansi(2)),
    );
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
        crate::presentation::factory::text("x"),
        background_decoration(ColorSpec::Ansi(2)),
    );
    let outer = crate::presentation::factory::fill_width(box_view(
        child,
        background_decoration(ColorSpec::Ansi(1)),
    ));
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
    let child = box_view(crate::presentation::factory::text("x"), {
        let mut decoration = Decoration::default();
        decoration.padding = Insets::all(1);
        decoration
    });
    let outer = crate::presentation::factory::fill_width(box_view(
        child,
        background_decoration(ColorSpec::Ansi(1)),
    ));
    let surface = layout_view(&outer, 5, PhysicalStyle::default());

    assert_eq!(
        surface.get(0, 0).style.background,
        Some(PhysicalColor::Indexed(1))
    );
}

#[test]
fn surface_background_does_not_enter_text_style_cascade() {
    let view = crate::presentation::factory::background(
        crate::presentation::factory::text("x"),
        ColorSpec::Ansi(1),
    );
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
        crate::presentation::factory::fill_width(crate::presentation::factory::text("a")),
        crate::presentation::factory::column(
            vec![crate::presentation::factory::fill_width(
                crate::presentation::factory::text("a"),
            )],
            0,
        ),
        row_view(
            vec![RowChild::content(crate::presentation::factory::text("a"))],
            0,
        ),
        crate::presentation::factory::fill_width(crate::presentation::factory::spacer(1)),
        crate::presentation::factory::clamp_rows(
            crate::presentation::factory::fill_width(crate::presentation::factory::text("a")),
            1,
            OverflowIndicator::None,
        ),
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
        crate::presentation::factory::fill_width(crate::presentation::factory::spacer(1)),
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
    let child =
        crate::presentation::factory::fill_width(crate::presentation::factory::styled_text(vec![
            TextSpan::styled(
                "x",
                StyleSpec {
                    background: Some(ColorSpec::Ansi(2)),
                    ..StyleSpec::default()
                },
            ),
        ]));
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
        crate::presentation::factory::text("漢"),
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
