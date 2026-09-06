use super::*;

#[test]
fn styled_spans_survive_wrapping_and_newlines() {
    let view =
        crate::presentation::factory::fill_width(crate::presentation::factory::styled_text(vec![
            TextSpan::styled(
                "abc",
                StyleSpec::new().foreground(ColorSpec::Named(AnsiColor::Green)),
            ),
            TextSpan::styled(
                "def\ngh",
                StyleSpec::new().foreground(ColorSpec::Named(AnsiColor::Red)),
            ),
        ]));
    let rows = compile_view(&view, 4).rows;
    assert_eq!(text(&rows[0]), "abcd");
    assert_eq!(text(&rows[1]), "ef");
    assert_eq!(text(&rows[2]), "gh");
    assert_eq!(
        rows[0].style_at(0).and_then(|style| style.foreground),
        Some(PhysicalColor::Named(crate::physical::AnsiColor::Green))
    );
    assert_eq!(
        rows[0].style_at(3).and_then(|style| style.foreground),
        Some(PhysicalColor::Named(crate::physical::AnsiColor::Red))
    );
    assert_eq!(
        rows[2].style_at(0).and_then(|style| style.foreground),
        Some(PhysicalColor::Named(crate::physical::AnsiColor::Red))
    );
}

#[test]
fn typed_text_style_cascades_to_physical_spans_without_rewriting_them() {
    let text = crate::presentation::factory::style(
        crate::presentation::factory::styled_text([
            TextSpan::plain("plain"),
            TextSpan::styled("bold", StyleSpec::new().bold()),
        ]),
        StyleSpec::new().foreground(ColorSpec::Ansi(1)),
    );
    let rows = compile_view(&text, 20).rows;

    assert_eq!(
        rows[0].style_at(0).and_then(|style| style.foreground),
        Some(PhysicalColor::Indexed(1))
    );
    assert!(!rows[0].style_at(0).is_some_and(|style| style.bold));
    assert_eq!(
        rows[0].style_at(5).and_then(|style| style.foreground),
        Some(PhysicalColor::Indexed(1))
    );
    assert!(rows[0].style_at(5).is_some_and(|style| style.bold));
}

#[test]
fn typed_text_wrap_and_no_wrap_preserve_existing_behavior() {
    let wrapped = crate::presentation::factory::wrap(
        crate::presentation::factory::text("abcd efgh"),
        WrapMode::WordThenGrapheme,
        None,
    );
    let grapheme = crate::presentation::factory::wrap(
        crate::presentation::factory::text("abcd efgh"),
        WrapMode::Grapheme,
        None,
    );
    let no_wrap = crate::presentation::factory::wrap(
        crate::presentation::factory::text("abcdef"),
        crate::WrapMode::NoWrap,
        None,
    );

    let ViewKind::Text(wrapped_text) = wrapped.kind() else {
        panic!("expected text view");
    };
    let ViewKind::Text(grapheme_text) = grapheme.kind() else {
        panic!("expected text view");
    };
    assert_eq!(wrapped_text.wrap, WrapMode::WordThenGrapheme);
    assert_eq!(grapheme_text.wrap, WrapMode::Grapheme);
    assert!(
        !ViewCompiler::default()
            .compile(&no_wrap, 3)
            .physically_complete
    );
}

#[test]
fn typed_text_alignment_uses_existing_text_layout() {
    for (align, expected) in [
        (HorizontalAlign::Start, "x"),
        (HorizontalAlign::Center, "  x"),
        (HorizontalAlign::End, "    x"),
    ] {
        let view = crate::presentation::factory::wrap(
            crate::presentation::factory::fill_width(crate::presentation::factory::text("x")),
            crate::WrapMode::WordThenGrapheme,
            Some(align),
        );
        let rows = compile_view(&view, 5).rows;
        assert_eq!(text(&rows[0]), expected);
    }
}

#[test]
fn ordinary_view_does_not_partially_paint_wide_grapheme() {
    let compiler = ViewCompiler::default();
    let view = crate::presentation::factory::text("漢");

    let block = compiler.compile(&view, 1);

    // Whatever the established clipped representation is,
    // it must not contain a half-painted wide grapheme.
    for row in &block.rows {
        for cell in row.cells() {
            assert!(
                !cell
                    .grapheme
                    .as_deref()
                    .is_some_and(|text| text.contains('漢'))
            );
        }
    }
}
