use iyon_tui::projection::ProjectionBuilder;
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::BlockKind;
use iyon_tui::{
    Block, HeadingLevel, MarkdownOptions, MarkdownProjector, PlainTextProjector, Projection,
    Projector, TextContent, TextOrigin,
};

fn range(start: usize, end: usize) -> StreamRange {
    StreamRange::new(
        StreamOffset::new(start as u64),
        StreamOffset::new(end as u64),
    )
}

fn raw(source: &str, sealed: bool) -> Projection<TextContent> {
    let end = StreamOffset::new(source.len() as u64);
    ProjectionBuilder::new(StreamOffset::ZERO, end, end, sealed)
        .emit(
            StreamRange::new(StreamOffset::ZERO, end),
            TextContent::raw(source),
        )
        .finish()
        .unwrap()
}

fn blocks(output: &Projection<TextContent>) -> Vec<&Block> {
    output
        .spans()
        .iter()
        .flat_map(iyon_tui::projection::ProjectionSpan::values)
        .filter_map(|value| match value {
            TextContent::Block(block) => Some(block),
            TextContent::Raw(_) => None,
        })
        .collect()
}

#[test]
fn markdown_heading_and_nested_inlines_claim_markdown_origin() {
    let mut projector = MarkdownProjector::default();
    let output = projector.project(&raw("# Hello\n", true)).unwrap();
    let heading = blocks(&output)[0];
    assert!(
        matches!(heading.kind(), BlockKind::Heading { level, .. } if *level == HeadingLevel::H1)
    );
    assert_eq!(heading.origin(), Some(TextOrigin::MARKDOWN));
    let BlockKind::Heading { content, .. } = heading.kind() else {
        panic!("expected heading");
    };
    assert_eq!(content.items()[0].origin(), Some(TextOrigin::MARKDOWN));
}

#[test]
fn markdown_preserves_existing_semantic_barriers() {
    let markdown = "# markdown\n";
    let app = "app\n";
    let tail = "tail\n";
    let md_end = markdown.len();
    let app_end = md_end + app.len();
    let tail_end = app_end + tail.len();
    let application = Block::heading(HeadingLevel::H1, "application");
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(tail_end as u64),
        StreamOffset::new(tail_end as u64),
        true,
    )
    .emit(range(0, md_end), TextContent::raw(markdown))
    .emit(
        range(md_end, app_end),
        TextContent::block(application.clone()),
    )
    .emit(range(app_end, tail_end), TextContent::raw(tail))
    .finish()
    .unwrap();

    let mut projector = MarkdownProjector::default();
    let output = projector.project(&input).unwrap();
    let values = blocks(&output);
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].origin(), Some(TextOrigin::MARKDOWN));
    assert!(matches!(values[0].kind(), BlockKind::Heading { .. }));
    assert_eq!(values[1], &application);
    assert_eq!(values[1].origin(), None);
    assert_eq!(values[2].origin(), Some(TextOrigin::MARKDOWN));
}

#[test]
fn markdown_stamps_origin_through_nested_list_structure() {
    let mut projector = MarkdownProjector::default();
    let output = projector.project(&raw("- **hello**\n", true)).unwrap();
    let list = blocks(&output)[0];
    assert!(matches!(list.kind(), BlockKind::List(_)));
    assert_eq!(list.origin(), Some(TextOrigin::MARKDOWN));
    let item = &list.as_list().unwrap().items()[0];
    assert_eq!(item.origin(), Some(TextOrigin::MARKDOWN));
    let paragraph = &item.blocks()[0];
    assert_eq!(paragraph.origin(), Some(TextOrigin::MARKDOWN));
    let BlockKind::Paragraph(content) = paragraph.kind() else {
        panic!("expected paragraph");
    };
    assert_eq!(content.items()[0].origin(), Some(TextOrigin::MARKDOWN));
}

#[test]
fn markdown_table_origin_when_tables_are_enabled() {
    let source = "| a | b |\n|---|---|\n| c | d |\n";
    let mut projector = MarkdownProjector::new(MarkdownOptions::commonmark().with_tables(true));
    let output = projector.project(&raw(source, true)).unwrap();
    let table = blocks(&output)
        .into_iter()
        .find(|block| matches!(block.kind(), BlockKind::Table(_)))
        .unwrap();
    assert_eq!(table.origin(), Some(TextOrigin::MARKDOWN));
    let BlockKind::Table(table) = table.kind() else {
        panic!("expected table");
    };
    assert_eq!(table.rows()[0].origin(), Some(TextOrigin::MARKDOWN));
    assert_eq!(
        table.rows()[0].cells()[0].origin(),
        Some(TextOrigin::MARKDOWN)
    );
}

#[test]
fn plain_text_claims_raw_domains_and_preserves_barriers() {
    let hello = "hello";
    let app = "app";
    let tail = "tail";
    let hello_end = hello.len();
    let app_end = hello_end + app.len();
    let tail_end = app_end + tail.len();
    let application = Block::paragraph("application");
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(tail_end as u64),
        StreamOffset::new(tail_end as u64),
        true,
    )
    .emit(range(0, hello_end), TextContent::raw(hello))
    .emit(
        range(hello_end, app_end),
        TextContent::block(application.clone()),
    )
    .emit(range(app_end, tail_end), TextContent::raw(tail))
    .finish()
    .unwrap();

    let mut projector = PlainTextProjector::new();
    let output = projector.project(&input).unwrap();
    let values = blocks(&output);
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].origin(), Some(TextOrigin::PLAIN_TEXT));
    let BlockKind::Paragraph(content) = values[0].kind() else {
        panic!("expected paragraph");
    };
    assert_eq!(content.items()[0].origin(), Some(TextOrigin::PLAIN_TEXT));
    assert_eq!(values[1], &application);
    assert_eq!(values[1].origin(), None);
    assert_eq!(values[2].origin(), Some(TextOrigin::PLAIN_TEXT));
}
