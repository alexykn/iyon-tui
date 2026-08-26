use iyon_tui::projection::{ProjectionBuilder, validate_projection_transition};
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{BlockKind, Mark, validate_text_projection};
use iyon_tui::{
    MarkdownOptions, MarkdownProjector, Projection, Projector, TextContent, TextOrigin,
};

fn iyon_gfm() -> MarkdownOptions {
    MarkdownOptions::gfm().with_live_table_stabilization(true)
}

fn input(s: &str, stable: usize, sealed: bool) -> Projection<TextContent> {
    let b = StreamOffset::ZERO;
    let e = StreamOffset::new(s.len() as u64);
    let st = StreamOffset::new(stable as u64);
    ProjectionBuilder::new(b, st, e, sealed)
        .emit(StreamRange::new(b, e), TextContent::raw(s))
        .finish()
        .unwrap()
}

fn kinds(p: &Projection<TextContent>) -> Vec<String> {
    p.spans()
        .iter()
        .flat_map(|s| s.values())
        .filter_map(|v| match v {
            TextContent::Block(b) => Some(format!("{:?}", b.kind())),
            _ => None,
        })
        .collect()
}

fn blocks(p: &Projection<TextContent>) -> Vec<&iyon_tui::Block> {
    p.spans()
        .iter()
        .flat_map(|span| span.values())
        .filter_map(|value| match value {
            TextContent::Block(block) => Some(block),
            TextContent::Raw(_) => None,
        })
        .collect()
}

fn one_shot_equals_incremental(options: MarkdownOptions, source: &str, cuts: &[usize]) {
    let mut fresh = MarkdownProjector::new(options);
    let sealed = fresh.project(&input(source, source.len(), true)).unwrap();
    let mut incremental = MarkdownProjector::new(options);
    let mut previous = None;
    for &end in cuts {
        if end == 0 || end > source.len() || !source.is_char_boundary(end) {
            continue;
        }
        let next = incremental
            .project(&input(&source[..end], end, false))
            .unwrap();
        validate_text_projection(&next).unwrap();
        if let Some(previous) = &previous {
            validate_projection_transition(previous, &next).unwrap();
        }
        previous = Some(next);
    }
    let finished = incremental
        .project(&input(source, source.len(), true))
        .unwrap();
    assert_eq!(finished, sealed);
}

#[test]
fn setext_stays_mutable_until_seal() {
    let mut m = MarkdownProjector::default();
    let a = m.project(&input("Foo\n", 4, false)).unwrap();
    assert!(a.stable_through().as_u64() < 4);
    let b = m.project(&input("Foo\n---\n", 8, false)).unwrap();
    assert!(kinds(&b).iter().any(|k| k.contains("Heading")));
    let c = m.project(&input("Foo\n---\n", 8, true)).unwrap();
    assert_eq!(c.stable_through(), c.source_end());
}

#[test]
fn references_are_parsed_and_incremental_seal_converges() {
    let source = "[foo]\n\n[foo]: /url\n";
    let mut m = MarkdownProjector::default();
    let first = m.project(&input("[foo]\n\n", 7, false)).unwrap();
    assert!(first.stable_through().as_u64() < 7);
    let final_one = m.project(&input(source, source.len(), true)).unwrap();
    let mut fresh = MarkdownProjector::default();
    let batch = fresh.project(&input(source, source.len(), true)).unwrap();
    assert_eq!(final_one, batch);
}

#[test]
fn incremental_and_cached_parses_preserve_origin_metadata() {
    let source = "# Hello\n\n- **hello**\n";
    let mut incremental = MarkdownProjector::default();
    let _ = incremental
        .project(&input("# Hello\n\n", 9, false))
        .unwrap();
    let chunked = incremental
        .project(&input(source, source.len(), true))
        .unwrap();
    let mut fresh = MarkdownProjector::default();
    let batch = fresh.project(&input(source, source.len(), true)).unwrap();
    assert_eq!(chunked, batch);
    for projection in [&chunked, &batch] {
        for span in projection.spans() {
            for value in span.values() {
                let TextContent::Block(block) = value else {
                    continue;
                };
                assert_eq!(block.origin(), Some(TextOrigin::MARKDOWN));
                if let BlockKind::List(list) = block.kind() {
                    assert_eq!(list.items()[0].origin(), Some(TextOrigin::MARKDOWN));
                }
            }
        }
    }
}

#[test]
fn gfm_incremental_matches_one_shot_for_table_task_and_strike() {
    let source =
        "| Item | State |\n| --- | --- |\n| ~~old~~ | active |\n\n- [x] complete\n- [ ] pending\n";
    one_shot_equals_incremental(
        MarkdownOptions::gfm(),
        source,
        &[
            1,
            source.find("State").unwrap(),
            source.find("---").unwrap(),
            source.find("old").unwrap(),
            source.find("complete").unwrap(),
            source.len(),
        ],
    );
    let mut projector = MarkdownProjector::new(MarkdownOptions::gfm());
    let output = projector
        .project(&input(source, source.len(), true))
        .unwrap();
    let table = blocks(&output)
        .into_iter()
        .find_map(|block| match block.kind() {
            BlockKind::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();
    assert_eq!(table.header_rows(), 1);
    let list = blocks(&output)
        .into_iter()
        .find_map(|block| block.as_list())
        .unwrap();
    assert_eq!(list.items()[0].checked(), Some(true));
    let struck = blocks(&output).into_iter().any(|block| match block.kind() {
        BlockKind::Table(table) => table.rows().iter().any(|row| {
            row.cells().iter().any(|cell| {
                cell.blocks().iter().any(|block| match block.kind() {
                    BlockKind::Paragraph(content) => content.iter().any(|inline| {
                        inline
                            .marks()
                            .marks()
                            .iter()
                            .any(|mark| matches!(mark, Mark::Strikethrough))
                    }),
                    _ => false,
                })
            })
        }),
        _ => false,
    });
    assert!(struck);
}

#[test]
fn gfm_table_streaming_keeps_stable_prefix_honest() {
    let stages = [
        "| A",
        "| A | B",
        "| A | B |",
        "| A | B |\n|---|---|",
        "| A | B |\n|---|---|\n| 1 | 2 |",
    ];
    let final_source = stages[stages.len() - 1];
    let mut projector = MarkdownProjector::new(iyon_gfm());
    let mut previous = None;
    for stage in stages {
        let next = projector
            .project(&input(stage, stage.len(), false))
            .unwrap();
        validate_text_projection(&next).unwrap();
        if let Some(previous) = &previous {
            validate_projection_transition(previous, &next).unwrap();
        }
        let restart = projector.restart_from(next.stable_through());
        assert!(restart <= next.stable_through());
        previous = Some(next);
    }
    let live = previous.as_ref().expect("unsealed table stages");
    assert!(
        blocks(live)
            .iter()
            .all(|block| !matches!(block.kind(), BlockKind::Table(_))),
        "unsealed GFM tables stay raw pipe paragraphs until a closer or seal"
    );
    let sealed = projector
        .project(&input(final_source, final_source.len(), true))
        .unwrap();
    let mut fresh = MarkdownProjector::new(MarkdownOptions::gfm());
    let batch = fresh
        .project(&input(final_source, final_source.len(), true))
        .unwrap();
    assert_eq!(sealed, batch);
    assert!(
        blocks(&sealed)
            .iter()
            .any(|block| matches!(block.kind(), BlockKind::Table(_)))
    );
}

#[test]
fn gfm_table_after_list_stays_one_table_while_streaming() {
    let source = "- item\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\n> done\n";
    let mut incremental = MarkdownProjector::new(iyon_gfm());
    for (index, ch) in source.char_indices() {
        let end = index + ch.len_utf8();
        let next = incremental
            .project(&input(&source[..end], end, false))
            .unwrap();
        validate_text_projection(&next).unwrap();
    }
    let live = incremental
        .project(&input(source, source.len(), false))
        .unwrap();
    let mut fresh = MarkdownProjector::new(iyon_gfm());
    let oneshot = fresh.project(&input(source, source.len(), false)).unwrap();
    assert_eq!(
        kinds(&live),
        kinds(&oneshot),
        "incremental table after a list must match one-shot before seal"
    );
    let sealed = incremental
        .project(&input(source, source.len(), true))
        .unwrap();
    let mut sealed_fresh = MarkdownProjector::new(MarkdownOptions::gfm());
    let batch = sealed_fresh
        .project(&input(source, source.len(), true))
        .unwrap();
    assert_eq!(sealed, batch);
}

#[test]
fn thematic_break_inside_blockquote_is_nested_not_overlapping() {
    let source = "> quote\n>\n> ---\n";
    let mut projector = MarkdownProjector::new(MarkdownOptions::gfm());
    let output = projector
        .project(&input(source, source.len(), true))
        .unwrap();
    let blocks = blocks(&output);
    assert_eq!(blocks.len(), 1, "{:?}", kinds(&output));
    match blocks[0].kind() {
        BlockKind::BlockQuote { blocks: inner } => {
            assert!(
                inner
                    .iter()
                    .any(|block| matches!(block.kind(), BlockKind::ThematicBreak)),
                "{inner:?}"
            );
        }
        other => panic!("expected blockquote, got {other:?}"),
    }
}

#[test]
fn live_markdown_guide_prefixes_stay_valid() {
    let sources = [
        concat!(
            "What is Markdown?\n\n",
            "---\n\n",
            "```markdown\n",
            "# H1 Heading\n",
            "## H2 Heading\n",
            "### H3 Heading\n\n",
            "Emphasis\n\n",
            "- Bold → **bold**\n",
            "- Italic → *italic*\n",
            "-",
        ),
        concat!(
            "> This is a blockquote.\n",
            "> It can span multiple lines.\n",
            ">\n",
            "> > And it can be nested.\n\n",
            "> Tip: remember bold vs italic.\n\n",
            "---\n\n",
            "- First item\n",
            "- Second item\n\n",
            "  - Nested item\n",
            "- Third item\n",
        ),
        "> quote\n>\n> ---\n",
        "- Bold → **bold**\n- Italic → *italic*\n-",
        "*italic*\n**bold**\n-",
        "```markdown\n# H1 Heading\n",
    ];
    for source in sources {
        let mut projector = MarkdownProjector::new(MarkdownOptions::gfm());
        let mut previous = None;
        for end in 1..=source.len() {
            if !source.is_char_boundary(end) {
                continue;
            }
            let next = projector
                .project(&input(&source[..end], end, false))
                .unwrap_or_else(|error| {
                    panic!(
                        "prefix {end} of {source:?} failed: {error:?}\n{:?}",
                        &source[..end]
                    )
                });
            validate_text_projection(&next).unwrap();
            if let Some(previous) = &previous {
                validate_projection_transition(previous, &next).unwrap();
            }
            previous = Some(next);
        }
    }
}

fn project_gfm(source: &str, sealed: bool) -> Projection<TextContent> {
    MarkdownProjector::new(MarkdownOptions::gfm())
        .project(&input(source, source.len(), sealed))
        .unwrap()
}

fn first_table(p: &Projection<TextContent>) -> &iyon_tui::text::Table {
    blocks(p)
        .into_iter()
        .find_map(|block| match block.kind() {
            BlockKind::Table(table) => Some(table),
            _ => None,
        })
        .expect("expected a table")
}

#[test]
fn gfm_example_203_mismatched_delimiter_is_not_a_table() {
    // Spec: header and delimiter must have the same cell count.
    let source = "| abc | def |\n| --- |\n| bar |\n";
    let output = project_gfm(source, true);
    assert!(
        blocks(&output)
            .iter()
            .all(|block| !matches!(block.kind(), BlockKind::Table(_))),
        "mismatched delimiter is not a GFM table: {:?}",
        kinds(&output)
    );
}

#[test]
fn gfm_example_204_pads_short_body_rows_and_truncates_long_ones() {
    let source = "| abc | def |\n| --- | --- |\n| bar |\n| bar | baz | boo |\n";
    let output = project_gfm(source, true);
    let table = first_table(&output);
    assert_eq!(table.columns().len(), 2);
    let widths: Vec<usize> = table.rows().iter().map(|row| row.cells().len()).collect();
    assert_eq!(widths, vec![2, 2, 2], "every body row is schema-wide");
}

#[test]
fn gfm_example_202_pipe_less_line_stays_a_short_table_row() {
    // Spec: a pipe-less line is still a table body row, padded with empty cells.
    let source = "| abc | def |\n| --- | --- |\nbar\n";
    let output = project_gfm(source, true);
    let table = first_table(&output);
    assert_eq!(table.columns().len(), 2);
    assert_eq!(table.rows().len(), 2);
    assert_eq!(table.rows()[1].cells().len(), 2);
}

#[test]
fn live_table_stabilization_keeps_unsealed_tables_raw() {
    let source = "| abc | def |\n| --- | --- |\n| 1 | 2 |\n";
    let live = MarkdownProjector::new(MarkdownOptions::gfm().with_live_table_stabilization(true))
        .project(&input(source, source.len(), false))
        .unwrap();
    assert!(
        blocks(&live)
            .iter()
            .all(|block| !matches!(block.kind(), BlockKind::Table(_))),
        "Live table policy keeps an unclosed table as raw pipes: {:?}",
        kinds(&live)
    );
    let strict = project_gfm(source, false);
    assert!(
        blocks(&strict)
            .iter()
            .any(|block| matches!(block.kind(), BlockKind::Table(_))),
        "strict GFM emits a table as soon as pulldown does: {:?}",
        kinds(&strict)
    );
}
