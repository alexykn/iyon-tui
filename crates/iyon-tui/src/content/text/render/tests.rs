use super::{Renderer, TextRenderer};
use crate::content::text::{
    Annotations, Block, CodeBlock, FormatId, HeadingLevel, Image, Inline, InlineContent,
    LanguageId, LinkTarget, List, ListItem, LiteralText, Mark, MarkdownProjector, SemanticTag,
    Table, TableCell, TableColumn, TableRow, TextContent, TextOrigin, TextPart, TextRole,
    TextSelector, TextTableSection, TextTaskState,
};
use crate::physical::PhysicalStyle;
use crate::presentation::layout::compile_view_with_theme;
use crate::{Projector, StyleSpec, TextAttribute, Theme, View};

fn style_at(view: &View, theme: &Theme, needle: &str) -> PhysicalStyle {
    let painted = compile_view_with_theme(view, 80, theme);
    for row in &painted.rows {
        if let Some(index) = row.cell_x_of(needle) {
            return row.style_at(index).expect("painted cell");
        }
    }
    panic!(
        "did not find {needle:?} in {:?}",
        painted
            .rows
            .iter()
            .map(|row| row.plain_text())
            .collect::<Vec<_>>()
    );
}

fn render(block: &Block) -> View {
    TextRenderer::new().render(block)
}

fn strong(text: &str) -> Inline {
    Inline::text(text).strong()
}

fn emphasis(text: &str) -> Inline {
    Inline::text(text).emphasis()
}

#[test]
fn framework_defaults_reach_rendered_text() {
    let theme = Theme::new();
    assert!(style_at(&render(&Block::heading(HeadingLevel::H1, "H")), &theme, "H").bold);
    assert!(style_at(&render(&Block::heading(HeadingLevel::H1, "H")), &theme, "H").underline);
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([strong("S")]))),
            &theme,
            "S"
        )
        .bold
    );
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([emphasis("E")]))),
            &theme,
            "E"
        )
        .italic
    );
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([
                Inline::text("U").underline()
            ]))),
            &theme,
            "U"
        )
        .underline
    );
    let link = Inline::text("L")
        .with_link(LinkTarget::new("https://example", None::<&str>))
        .unwrap();
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([link]))),
            &theme,
            "L"
        )
        .underline
    );
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([
                Inline::text("X").strikethrough()
            ]))),
            &theme,
            "X"
        )
        .strikethrough
    );
}

#[test]
fn hand_built_and_markdown_headings_match_until_origin_specialization() {
    let mut markdown = MarkdownProjector::default();
    let parsed = markdown
        .project(
            &crate::projection::ProjectionBuilder::new(
                crate::stream::StreamOffset::ZERO,
                crate::stream::StreamOffset::new(8),
                crate::stream::StreamOffset::new(8),
                true,
            )
            .emit(
                crate::stream::StreamRange::new(
                    crate::stream::StreamOffset::ZERO,
                    crate::stream::StreamOffset::new(8),
                ),
                TextContent::raw("# Hello\n"),
            )
            .finish()
            .unwrap(),
        )
        .unwrap();
    let TextContent::Block(markdown_heading) = &parsed.spans()[0].values()[0] else {
        panic!("expected heading");
    };
    let hand = Block::heading(HeadingLevel::H1, "Hello");
    let empty = Theme::new();
    assert_eq!(
        style_at(&render(markdown_heading), &empty, "Hello"),
        style_at(&render(&hand), &empty, "Hello")
    );

    let origin_theme = Theme::new().with_text_style(
        TextSelector::heading().origin(TextOrigin::MARKDOWN),
        StyleSpec::new().italic(),
    );
    assert!(
        !style_at(&render(&hand), &origin_theme, "Hello").italic,
        "hand-built heading must ignore markdown origin rules"
    );
    assert!(style_at(&render(markdown_heading), &origin_theme, "Hello").italic);
}

#[test]
fn application_heading_plain_clears_framework_defaults_but_keeps_inner_marks() {
    let theme = Theme::new().with_text_style(TextSelector::heading(), StyleSpec::plain());
    let heading = Block::heading(
        HeadingLevel::H1,
        InlineContent::new([
            Inline::text("plain "),
            Inline::text("inner")
                .strong()
                .with_link(LinkTarget::new("https://example", None::<&str>))
                .unwrap(),
        ]),
    );
    let view = render(&heading);
    let plain = style_at(&view, &theme, "plain");
    assert!(!plain.bold);
    assert!(!plain.underline);
    let inner = style_at(&view, &theme, "inner");
    assert!(inner.bold);
    assert!(inner.underline);
}

#[test]
fn strong_and_link_conjunction_matches_only_when_both_marks_are_present() {
    let theme = Theme::new().with_text_style(
        TextSelector::strong().and_role(TextRole::Link),
        StyleSpec::new().reversed(),
    );
    let both = Inline::text("both")
        .strong()
        .with_link(LinkTarget::new("https://example", None::<&str>))
        .unwrap();
    let strong_only = strong("strong");
    let link_only = Inline::text("link")
        .with_link(LinkTarget::new("https://example", None::<&str>))
        .unwrap();
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([both]))),
            &theme,
            "both"
        )
        .reversed
    );
    assert!(
        !style_at(
            &render(&Block::paragraph(InlineContent::new([strong_only]))),
            &theme,
            "strong"
        )
        .reversed
    );
    assert!(
        !style_at(
            &render(&Block::paragraph(InlineContent::new([link_only]))),
            &theme,
            "link"
        )
        .reversed
    );
}

#[test]
fn ancestor_block_roles_flatten_onto_nested_heading_but_not_inner_strong() {
    let theme = Theme::new()
        .with_text_style(
            TextSelector::heading()
                .and_role(TextRole::BlockQuote)
                .and_role(TextRole::List)
                .and_role(TextRole::ListItem),
            StyleSpec::new().reversed(),
        )
        .with_text_style(
            TextSelector::strong()
                .and_role(TextRole::BlockQuote)
                .and_role(TextRole::List)
                .and_role(TextRole::ListItem),
            StyleSpec::new().dim(),
        );
    let heading = Block::heading(
        HeadingLevel::H2,
        InlineContent::new([Inline::text("head "), strong("inner")]),
    );
    let quoted = Block::block_quote([Block::list(List::bulleted([ListItem::new([heading])]))]);
    let view = render(&quoted);
    assert!(style_at(&view, &theme, "head").reversed);
    assert!(
        !style_at(&view, &theme, "inner").dim,
        "ancestor block roles must not leak onto inner Strong spans"
    );
}

#[test]
fn block_annotation_does_not_become_a_child_span_fact() {
    let tag = SemanticTag::new("app", "note").unwrap();
    let theme = Theme::new()
        .with_text_style(
            TextSelector::paragraph().and_annotation(&tag),
            StyleSpec::new().dim(),
        )
        .with_text_style(
            TextSelector::strong().and_annotation(&tag),
            StyleSpec::new().reversed(),
        );
    let paragraph = Block::paragraph(InlineContent::new([strong("inner")]))
        .with_annotations(Annotations::new().with_tag(tag));
    let view = render(&paragraph);
    let inner = style_at(&view, &theme, "inner");
    assert!(inner.dim, "physical style may inherit from the paragraph");
    assert!(
        !inner.reversed,
        "the block tag must not be a Strong span fact"
    );
}

#[test]
fn run_annotation_styles_the_exact_span() {
    let annotation = SemanticTag::new("example", "annotated").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::annotation(&annotation),
        StyleSpec::new().dim().italic(),
    );
    let tagged = crate::TextRun::from("think")
        .map_annotations(|annotations| annotations.with_tag(annotation.clone()));
    let paragraph = Block::paragraph(InlineContent::new([
        Inline::text(tagged),
        Inline::text(" plain"),
    ]));
    let view = render(&paragraph);
    let tagged = style_at(&view, &theme, "think");
    assert!(tagged.dim);
    assert!(tagged.italic);
    assert!(!style_at(&view, &theme, "plain").dim);
}

#[test]
fn annotation_and_strong_conjunction_uses_span_local_facts() {
    let annotation = SemanticTag::new("example", "annotated").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::strong().and_annotation(&annotation),
        StyleSpec::new().reversed(),
    );
    let run = crate::TextRun::from("both")
        .map_annotations(|annotations| annotations.with_tag(annotation.clone()));
    let both = Inline::text(run).strong();
    let strong_only = strong("only");
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([both]))),
            &theme,
            "both"
        )
        .reversed
    );
    assert!(
        !style_at(
            &render(&Block::paragraph(InlineContent::new([strong_only]))),
            &theme,
            "only"
        )
        .reversed
    );
}

#[test]
fn nearest_origin_wins_for_nested_generic_ir() {
    let theme = Theme::new().with_text_style(
        TextSelector::heading().origin(TextOrigin::MARKDOWN),
        StyleSpec::new().italic(),
    );
    let inherited = Block::container([Block::heading(HeadingLevel::H1, "Hello")])
        .with_origin(TextOrigin::MARKDOWN);
    assert!(style_at(&render(&inherited), &theme, "Hello").italic);

    let overridden =
        Block::container([Block::heading(HeadingLevel::H1, "Hello")
            .with_origin(TextOrigin::new("asciidoc").unwrap())])
        .with_origin(TextOrigin::MARKDOWN);
    assert!(!style_at(&render(&overridden), &theme, "Hello").italic);
}

#[test]
fn checked_task_scalar_reaches_child_strong_span() {
    let theme = Theme::new().with_text_style(
        TextSelector::strong().task_state(TextTaskState::Checked),
        StyleSpec::new().reversed(),
    );
    let checked = Block::list(List::bulleted([ListItem::task(
        InlineContent::new([strong("done")]),
        true,
    )]));
    let unchecked = Block::list(List::bulleted([ListItem::task(
        InlineContent::new([strong("todo")]),
        false,
    )]));
    assert!(style_at(&render(&checked), &theme, "done").reversed);
    assert!(!style_at(&render(&unchecked), &theme, "todo").reversed);
}

#[test]
fn table_header_bold_comes_from_theme_not_renderer() {
    let table = Table::new(
        None::<Vec<Block>>,
        [TableColumn::start(), TableColumn::start()],
        1,
        [
            TableRow::new([TableCell::text("Head"), TableCell::text("A")]),
            TableRow::new([TableCell::text("Body"), TableCell::text("B")]),
        ],
    )
    .unwrap();
    let view = render(&Block::table(table.clone()));
    let empty = Theme::new();
    assert!(style_at(&view, &empty, "Head").bold);
    assert!(!style_at(&view, &empty, "Body").bold);

    let override_theme = Theme::new().with_text_style(
        TextSelector::table_row().table_section(TextTableSection::Header),
        StyleSpec::plain(),
    );
    assert!(!style_at(&render(&Block::table(table)), &override_theme, "Head").bold);
}

#[test]
fn language_and_syntax_annotation_select_code_runs() {
    let keyword = SemanticTag::new("syntax", "keyword").unwrap();
    let rust = LanguageId::new("rust").unwrap();
    let python = LanguageId::new("python").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::annotation(&keyword).language(&rust),
        StyleSpec::new().reversed(),
    );
    let tagged =
        LiteralText::new([crate::TextRun::from("let")
            .map_annotations(|annotations| annotations.with_tag(keyword))]);
    let rust_block = Block::code(CodeBlock::new(Some(rust), Some("rust"), tagged.clone()));
    let python_block = Block::code(CodeBlock::new(Some(python), Some("python"), tagged));
    assert!(style_at(&render(&rust_block), &theme, "let").reversed);
    assert!(!style_at(&render(&python_block), &theme, "let").reversed);
}

#[test]
fn raw_inline_format_selector_matches() {
    let html = FormatId::new("html").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::role(TextRole::RawInline).format(&html),
        StyleSpec::new().dim(),
    );
    let inline = Inline::raw(html, LiteralText::from("<br>"));
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([inline]))),
            &theme,
            "<br>"
        )
        .dim
    );
}

#[test]
fn generated_parts_are_independently_selectable() {
    let quote_theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::QuoteMarker),
        StyleSpec::new().reversed(),
    );
    let quoted = Block::block_quote([Block::paragraph("body")]);
    let quote_view = render(&quoted);
    assert!(style_at(&quote_view, &quote_theme, ">").reversed);
    assert!(!style_at(&quote_view, &quote_theme, "body").reversed);

    let list_theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::ListMarker),
        StyleSpec::new().reversed(),
    );
    let list = Block::list(List::bulleted([ListItem::paragraph("item")]));
    let list_view = render(&list);
    assert!(style_at(&list_view, &list_theme, "-").reversed);
    assert!(!style_at(&list_view, &list_theme, "item").reversed);

    let task_theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::TaskMarker).task_state(TextTaskState::Checked),
        StyleSpec::new().reversed(),
    );
    let task = Block::list(List::bulleted([ListItem::task("done", true)]));
    assert!(style_at(&render(&task), &task_theme, "[").reversed);

    let rule_theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::ThematicRule),
        StyleSpec::new().dim(),
    );
    assert!(style_at(&render(&Block::thematic_break()), &rule_theme, "─").dim);

    let image_theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::ImageFallback),
        StyleSpec::new().italic(),
    );
    let image = Inline::image(Image::new(
        "src",
        None::<&str>,
        InlineContent::new([Inline::text("alt")]),
    ));
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([image]))),
            &image_theme,
            "alt"
        )
        .italic
    );
}

#[test]
fn any_selector_applies_the_reserved_text_base_style() {
    let theme = Theme::new().with_text_style(TextSelector::any(), StyleSpec::new().dim());
    assert!(style_at(&render(&Block::paragraph("para")), &theme, "para").dim);
    assert!(
        style_at(
            &render(&Block::heading(HeadingLevel::H2, "head")),
            &theme,
            "head"
        )
        .dim
    );
    assert!(
        style_at(
            &render(&Block::paragraph(InlineContent::new([strong("S")]))),
            &theme,
            "S"
        )
        .dim
    );
    let raw = TextRenderer::new().render(&TextContent::raw("raw"));
    assert!(style_at(&raw, &theme, "raw").dim);
}

#[test]
fn local_view_style_still_wins_over_framework_and_application_rules() {
    let heading = render(&Block::heading(HeadingLevel::H1, "Hello"))
        .style(StyleSpec::new().attribute(TextAttribute::Bold, false));
    let style = style_at(&heading, &Theme::new(), "Hello");
    assert!(!style.bold);
    assert!(style.underline);
}

#[test]
fn superscript_subscript_and_small_caps_emit_without_framework_paint() {
    let marks = [
        Inline::text("sup").with_mark(Mark::Superscript).unwrap(),
        Inline::text("sub").with_mark(Mark::Subscript).unwrap(),
        Inline::text("sc").with_mark(Mark::SmallCaps).unwrap(),
    ];
    let view = render(&Block::paragraph(InlineContent::new(marks)));
    let theme = Theme::new();
    assert!(!style_at(&view, &theme, "sup").bold);
    assert!(!style_at(&view, &theme, "sub").italic);
    assert!(!style_at(&view, &theme, "sc").underline);
}
