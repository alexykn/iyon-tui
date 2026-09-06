use iyon_tui::projection::ProjectionBuilder;
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{BlockKind, Mark, TextProvenance};
use iyon_tui::{
    MarkdownOptions, MarkdownProjector, Projection, Projector, RawText, TextContent, TextOrigin,
};

fn projection(source: &str, sealed: bool) -> Projection<TextContent> {
    let end = StreamOffset::new(source.len() as u64);
    ProjectionBuilder::new(StreamOffset::ZERO, end, end, sealed)
        .emit(
            StreamRange::new(StreamOffset::ZERO, end),
            TextContent::Raw(RawText::new(source)),
        )
        .finish()
        .unwrap()
}

fn values(output: &Projection<TextContent>) -> Vec<&TextContent> {
    output
        .spans()
        .iter()
        .flat_map(iyon_tui::projection::ProjectionSpan::values)
        .collect()
}

#[test]
fn markdown_converts_common_blocks_and_exact_code_body() {
    let source = "# Title\n\n**hello** _world_\n\n```json\n{\"ok\":true}\n```\n";
    let mut projector = MarkdownProjector::new(MarkdownOptions::commonmark());
    let output = projector.project(&projection(source, true)).unwrap();
    assert!(output.is_sealed());
    assert_eq!(output.stable_through(), output.source_end());
    assert!(values(&output).iter().any(|value| matches!(value, TextContent::Block(block) if matches!(block.kind(), BlockKind::Heading { .. }))));
    let code = values(&output)
        .into_iter()
        .find_map(|value| match value {
            TextContent::Block(block) => block.as_code_block(),
            TextContent::Raw(_) => None,
        })
        .unwrap();
    assert_eq!(code.language().unwrap().as_str(), "json");
    assert_eq!(code.body().text(), "{\"ok\":true}\n");
    assert!(matches!(
        code.body().runs()[0].provenance(),
        TextProvenance::Exact(_)
    ));
}

#[test]
fn markdown_extensions_are_independent() {
    let source = "- [x] done\n\n~~gone~~\n\n| a | b |\n|---|---|\n| c | d |\n";
    for options in [
        MarkdownOptions::commonmark(),
        MarkdownOptions::commonmark().with_task_lists(true),
        MarkdownOptions::commonmark().with_strikethrough(true),
        MarkdownOptions::commonmark().with_tables(true),
        MarkdownOptions::commonmark()
            .with_task_lists(true)
            .with_strikethrough(true)
            .with_tables(true),
    ] {
        let mut projector = MarkdownProjector::new(options);
        projector.project(&projection(source, true)).unwrap();
    }
}

fn gfm_fixture() -> &'static str {
    "| Item | State |\n| --- | --- |\n| ~~old~~ | active |\n\n- [x] complete\n- [ ] pending\n"
}

#[test]
fn gfm_preset_maps_tables_strikethrough_and_tasks() {
    assert_eq!(MarkdownOptions::default(), MarkdownOptions::commonmark());
    assert!(MarkdownOptions::gfm().tables());
    assert!(MarkdownOptions::gfm().strikethrough());
    assert!(MarkdownOptions::gfm().task_lists());

    let mut projector = MarkdownProjector::new(MarkdownOptions::gfm());
    let output = projector.project(&projection(gfm_fixture(), true)).unwrap();
    let blocks: Vec<_> = values(&output)
        .into_iter()
        .filter_map(|value| match value {
            TextContent::Block(block) => Some(block),
            TextContent::Raw(_) => None,
        })
        .collect();

    let table = blocks
        .iter()
        .find_map(|block| match block.kind() {
            BlockKind::Table(table) => Some((block, table)),
            _ => None,
        })
        .expect("GFM table");
    assert_eq!(table.0.origin(), Some(TextOrigin::MARKDOWN));
    assert_eq!(table.1.header_rows(), 1);
    assert_eq!(table.1.columns().len(), 2);
    assert_eq!(
        table.1.columns()[0].alignment(),
        iyon_tui::text::Alignment::Default
    );
    assert_eq!(
        table.1.columns()[1].alignment(),
        iyon_tui::text::Alignment::Default
    );

    let body_cell = &table.1.rows()[1].cells()[0];
    let BlockKind::Paragraph(content) = body_cell.blocks()[0].kind() else {
        panic!("expected paragraph cell");
    };
    assert!(
        content.iter().any(|inline| {
            inline.as_text().is_some_and(|run| run.text() == "old")
                && inline
                    .marks()
                    .marks()
                    .iter()
                    .any(|mark| matches!(mark, Mark::Strikethrough))
        }),
        "GFM strikethrough must map to Mark::Strikethrough"
    );

    let lists: Vec<_> = blocks.iter().filter_map(|block| block.as_list()).collect();
    assert_eq!(
        lists.len(),
        2,
        "each completed task item is its own stable list"
    );
    assert_eq!(lists[0].items()[0].checked(), Some(true));
    assert_eq!(lists[1].items()[0].checked(), Some(false));
    assert_eq!(lists[0].items()[0].origin(), Some(TextOrigin::MARKDOWN));
}

#[test]
fn commonmark_does_not_claim_gfm_extensions() {
    let mut projector = MarkdownProjector::new(MarkdownOptions::commonmark());
    let output = projector.project(&projection(gfm_fixture(), true)).unwrap();
    let blocks: Vec<_> = values(&output)
        .into_iter()
        .filter_map(|value| match value {
            TextContent::Block(block) => Some(block),
            TextContent::Raw(_) => None,
        })
        .collect();
    assert!(
        blocks
            .iter()
            .all(|block| !matches!(block.kind(), BlockKind::Table(_))),
        "CommonMark must not emit a Table"
    );
    assert!(blocks.iter().all(|block| match block.kind() {
        BlockKind::List(list) => list.items().iter().all(|item| item.checked().is_none()),
        _ => true,
    }));
    assert!(blocks.iter().all(|block| match block.kind() {
        BlockKind::Paragraph(content) => content.iter().all(|inline| {
            !inline
                .marks()
                .marks()
                .iter()
                .any(|mark| matches!(mark, Mark::Strikethrough))
        }),
        _ => true,
    }));
}

#[test]
fn plain_projector_claims_raw_domains_and_respects_barriers() {
    let source = "one\n\ntwo";
    let mut projector = iyon_tui::PlainTextProjector::new();
    let output = projector.project(&projection(source, true)).unwrap();
    assert_eq!(values(&output).len(), 1);
    assert!(
        matches!(values(&output)[0], TextContent::Block(block) if matches!(block.kind(), BlockKind::Paragraph(_)))
    );
}
