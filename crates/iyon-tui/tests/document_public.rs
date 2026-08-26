use std::convert::Infallible;

use iyon_tui::projection::ProjectionBuilder;
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{
    Alignment, Annotations, BlockKind, BreakKind, CodeBlock, FormatId, Image, LanguageId,
    LinkTarget, List, ListItem, ListMarker, LiteralText, Mark, MarkSet, NumberDelimiter,
    NumberStyle, SemanticKey, SemanticTag, SemanticValue, TextIrError, TextProjectionError,
    TextProvenance, TextRewriter, TextRun, TextVisitor, validate_text_projection, walk_block,
    walk_rewrite_block, walk_rewrite_inline,
};
use iyon_tui::text::{Table, TableCell, TableColumn, TableRow};
use iyon_tui::{
    Block, Inline, InlineContent, Projection, Projector, ProjectorExt, RawText,
    TextContent as Content,
};

fn range(start: usize, end: usize) -> StreamRange {
    StreamRange::new(
        StreamOffset::new(start as u64),
        StreamOffset::new(end as u64),
    )
}

fn exact(text: &str, start: usize) -> TextRun {
    TextRun::exact(text, range(start, start + text.len())).unwrap()
}

fn sealed_raw(source: &str) -> Projection<RawText> {
    ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(source.len() as u64),
        StreamOffset::new(source.len() as u64),
        true,
    )
    .emit(range(0, source.len()), RawText::new(source))
    .finish()
    .unwrap()
}

fn paragraph(source: &str, start: usize) -> Block {
    Block::paragraph(InlineContent::new([Inline::text(exact(source, start))]))
}

struct OuterProbe {
    source: String,
}

impl Projector<RawText> for OuterProbe {
    type Output = Content;
    type Error = Infallible;

    fn project(
        &mut self,
        input: &Projection<RawText>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let source = &self.source;
        let before = "Before\n";
        let json = "{\"ok\":true}";
        let after = "After\n";
        let mermaid = "graph TD\nA --> B\n";
        assert_eq!(source.as_str(), format!("{before}{json}{after}{mermaid}"));

        let before_end = before.len();
        let json_start = before_end;
        let json_end = json_start + json.len();
        let after_start = json_end;
        let after_end = after_start + after.len();
        let mermaid_start = after_end;

        let before_block = Block::paragraph(InlineContent::new([
            Inline::text(exact("Before", 0)),
            Inline::break_(BreakKind::Hard),
        ]));
        let json_language = LanguageId::new("json").unwrap();
        let json_block = Block::code(CodeBlock::new(
            Some(json_language),
            Some("json"),
            LiteralText::new([TextRun::exact(json, range(json_start, json_end))
                .unwrap()
                .with_annotations(
                    Annotations::new().with_tag(SemanticTag::new("example", "annotated").unwrap()),
                )]),
        ));
        let after_block = Block::paragraph(InlineContent::new([
            Inline::text(exact("After", after_start)),
            Inline::break_(BreakKind::Hard),
        ]));
        let mermaid_block = Block::code(CodeBlock::new(
            Some(LanguageId::new("mermaid").unwrap()),
            Some("mermaid"),
            LiteralText::from_exact(mermaid, range(mermaid_start, source.len())).unwrap(),
        ));

        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        output = output.emit(range(0, before_end), Content::block(before_block));
        output = output.emit(range(json_start, json_end), Content::block(json_block));
        output = output.emit(range(after_start, after_end), Content::block(after_block));
        output = output.emit(
            range(mermaid_start, source.len()),
            Content::block(mermaid_block),
        );
        Ok(output.finish().unwrap())
    }
}

struct JsonProbe;

impl TextRewriter for JsonProbe {
    type Error = Infallible;

    fn rewrite_block(&mut self, block: Block) -> Result<Block, Self::Error> {
        let Some(code) = block.as_code_block() else {
            return walk_rewrite_block(self, block);
        };
        if code.language().map(LanguageId::as_str) != Some("json") {
            return walk_rewrite_block(self, block);
        }
        let inherited_annotations = code
            .body()
            .runs()
            .first()
            .map(|run| run.annotations().clone())
            .unwrap_or_default();
        let start = code
            .body()
            .runs()
            .first()
            .and_then(|run| match run.provenance() {
                TextProvenance::Exact(range) | TextProvenance::Derived(range) => {
                    Some(range.start())
                }
                TextProvenance::Synthetic => None,
            })
            .unwrap()
            .as_u64() as usize;
        let body = LiteralText::new([
            TextRun::exact("{", range(start, start + 1)).unwrap(),
            TextRun::synthetic("\n  "),
            TextRun::exact("\"ok\"", range(start + 1, start + 5))
                .unwrap()
                .with_annotations(inherited_annotations),
            TextRun::synthetic(" "),
            TextRun::exact(":", range(start + 5, start + 6)).unwrap(),
            TextRun::synthetic(" "),
            TextRun::exact("true", range(start + 6, start + 10)).unwrap(),
            TextRun::synthetic("\n"),
            TextRun::exact("}", range(start + 10, start + 11)).unwrap(),
        ]);
        let replacement = Block::code(CodeBlock::new(code.language().cloned(), code.info(), body));
        Ok(replacement.with_annotations(block.annotations().clone()))
    }
}

impl Projector<Content> for JsonProbe {
    type Output = Content;
    type Error = Infallible;

    fn project(
        &mut self,
        input: &Projection<Content>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            let values = span
                .values()
                .iter()
                .map(|value| match value {
                    Content::Raw(raw) => Ok(Content::Raw(raw.clone())),
                    Content::Block(block) => self.rewrite_block(block.clone()).map(Content::Block),
                })
                .collect::<Result<Vec<_>, _>>()?;
            output = if values.is_empty() {
                output.elide(span.source())
            } else {
                output.emit_many(span.source(), values)
            };
        }
        Ok(output.finish().unwrap())
    }
}

struct SyntaxProbe;

impl TextRewriter for SyntaxProbe {
    type Error = Infallible;

    fn rewrite_literal(&mut self, literal: LiteralText) -> Result<LiteralText, Self::Error> {
        let key = SemanticTag::new("syntax", "key").unwrap();
        let mut runs = Vec::new();
        for run in literal.runs() {
            if run.text().contains("ok") {
                runs.push(
                    run.clone()
                        .map_annotations(|annotations| annotations.with_tag(key.clone())),
                );
            } else {
                runs.push(run.clone());
            }
        }
        Ok(LiteralText::new(runs))
    }
}

impl Projector<Content> for SyntaxProbe {
    type Output = Content;
    type Error = Infallible;

    fn project(
        &mut self,
        input: &Projection<Content>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            let values = span
                .values()
                .iter()
                .map(|value| match value {
                    Content::Raw(raw) => Ok(Content::Raw(raw.clone())),
                    Content::Block(block) => self.rewrite_block(block.clone()).map(Content::Block),
                })
                .collect::<Result<Vec<_>, _>>()?;
            output = if values.is_empty() {
                output.elide(span.source())
            } else {
                output.emit_many(span.source(), values)
            };
        }
        Ok(output.finish().unwrap())
    }
}

struct MermaidProbe;

impl TextRewriter for MermaidProbe {
    type Error = Infallible;

    fn rewrite_block(&mut self, block: Block) -> Result<Block, Self::Error> {
        let Some(code) = block.as_code_block() else {
            return walk_rewrite_block(self, block);
        };
        if code.language().map(LanguageId::as_str) != Some("mermaid") {
            return walk_rewrite_block(self, block);
        }
        let body = code.body().text();
        let source_range = code
            .body()
            .runs()
            .first()
            .and_then(|run| match run.provenance() {
                TextProvenance::Exact(range) | TextProvenance::Derived(range) => Some(*range),
                TextProvenance::Synthetic => None,
            })
            .unwrap();
        let child = paragraph(&body, source_range.start().as_u64() as usize);
        let tag = SemanticTag::new("diagram", "mermaid").unwrap();
        Ok(Block::container([child]).with_annotations(block.annotations().clone().with_tag(tag)))
    }
}

impl Projector<Content> for MermaidProbe {
    type Output = Content;
    type Error = Infallible;

    fn project(
        &mut self,
        input: &Projection<Content>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            let values = span
                .values()
                .iter()
                .map(|value| match value {
                    Content::Raw(raw) => Ok(Content::Raw(raw.clone())),
                    Content::Block(block) => self.rewrite_block(block.clone()).map(Content::Block),
                })
                .collect::<Result<Vec<_>, _>>()?;
            output = if values.is_empty() {
                output.elide(span.source())
            } else {
                output.emit_many(span.source(), values)
            };
        }
        Ok(output.finish().unwrap())
    }
}

#[test]
fn public_ir_supports_generic_composition_and_nested_portals() {
    let source = "Before\n{\"ok\":true}After\ngraph TD\nA --> B\n";
    let raw = sealed_raw(source);
    validate_text_projection(&raw.clone().map(Content::Raw)).unwrap();

    let mut pipeline = OuterProbe {
        source: source.to_owned(),
    }
    .then(JsonProbe)
    .then(MermaidProbe)
    .then(SyntaxProbe);
    let output = pipeline.project(&raw).unwrap();
    validate_text_projection(&output).unwrap();

    assert_eq!(output.source_base(), StreamOffset::ZERO);
    assert_eq!(output.source_end(), StreamOffset::new(source.len() as u64));
    assert!(output.is_sealed());

    let json = output.spans()[1].values()[0].clone();
    let Content::Block(json) = json else {
        panic!("expected JSON block")
    };
    let code = json.as_code_block().unwrap();
    assert!(
        code.body()
            .runs()
            .iter()
            .any(|run| matches!(run.provenance(), TextProvenance::Synthetic))
    );
    let key_run = code
        .body()
        .runs()
        .iter()
        .find(|run| run.text().contains("ok"))
        .unwrap();
    assert!(
        key_run
            .annotations()
            .tags()
            .iter()
            .any(|tag| tag.name() == "key")
    );
    assert!(
        key_run
            .annotations()
            .tags()
            .iter()
            .any(|tag| tag.namespace() == "example" && tag.name() == "annotated")
    );

    let mermaid = output.spans()[3].values()[0].clone();
    let Content::Block(mermaid) = mermaid else {
        panic!("expected Mermaid block")
    };
    assert!(mermaid.as_container().is_some());
    assert!(
        mermaid
            .annotations()
            .tags()
            .iter()
            .any(|tag| tag.name() == "mermaid")
    );
}

#[test]
fn structured_blocks_are_hard_barriers_for_front_end_projectors() {
    struct ClaimFirst;
    impl Projector<RawText> for ClaimFirst {
        type Output = Content;
        type Error = Infallible;
        fn project(
            &mut self,
            input: &Projection<RawText>,
        ) -> Result<Projection<Content>, Self::Error> {
            let mut output = ProjectionBuilder::new(
                input.source_base(),
                input.stable_through(),
                input.source_end(),
                input.is_sealed(),
            );
            output = output.emit(range(0, 5), Content::block(paragraph("first", 0)));
            output = output.emit(
                range(5, 10),
                Content::block(Block::code(CodeBlock::new(
                    Some(LanguageId::new("json").unwrap()),
                    None::<&str>,
                    LiteralText::from_exact("block", range(5, 10)).unwrap(),
                ))),
            );
            output = output.emit(range(10, 16), Content::raw("second"));
            Ok(output.finish().unwrap())
        }
    }
    struct ParseRaw;
    impl Projector<Content> for ParseRaw {
        type Output = Content;
        type Error = Infallible;
        fn project(
            &mut self,
            input: &Projection<Content>,
        ) -> Result<Projection<Content>, Self::Error> {
            let mut output = ProjectionBuilder::new(
                input.source_base(),
                input.stable_through(),
                input.source_end(),
                input.is_sealed(),
            );
            for span in input.spans() {
                for value in span.values() {
                    output = output.emit(
                        span.source(),
                        match value {
                            Content::Raw(raw) => Content::block(paragraph(
                                raw.text(),
                                span.source().start().as_u64() as usize,
                            )),
                            Content::Block(block) => Content::Block(block.clone()),
                        },
                    );
                }
            }
            Ok(output.finish().unwrap())
        }
    }
    let mut pipeline = ClaimFirst.then(ParseRaw);
    let output = pipeline.project(&sealed_raw("firstblocksecond")).unwrap();
    assert!(
        matches!(&output.spans()[1].values()[0], Content::Block(block) if block.as_code_block().is_some())
    );
    assert!(matches!(output.spans()[2].values()[0], Content::Block(_)));
}

#[test]
fn text_provenance_and_raw_validation_are_explicit() {
    let run = TextRun::exact("é", range(4, 6)).unwrap();
    let (left, right) = run.split_at(2).unwrap();
    assert_eq!(left.provenance(), &TextProvenance::Exact(range(4, 6)));
    assert_eq!(right.text(), "");
    let raw = RawText::new("héllo");
    assert_eq!(raw.exact_slice(range(10, 16), 1..3).unwrap().text(), "é");
    assert!(raw.exact_slice(range(10, 16), 0..2).is_err());
    assert_eq!(
        TextRun::exact("é", range(0, 1)),
        Err(TextIrError::InvalidExactLength {
            text_len: 2,
            range_len: 1
        })
    );
    assert_eq!(
        TextRun::synthetic("é").split_at(1),
        Err(TextIrError::NotCharBoundary)
    );

    let derived = TextRun::derived("decoded", range(0, 2));
    let (left, right) = derived.split_at(3).unwrap();
    assert_eq!(left.provenance(), &TextProvenance::Derived(range(0, 2)));
    assert_eq!(right.provenance(), &TextProvenance::Derived(range(0, 2)));

    let bad_raw = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(1),
        StreamOffset::new(1),
        true,
    )
    .emit(range(0, 1), Content::raw("é"))
    .finish()
    .unwrap();
    assert!(matches!(
        validate_text_projection(&bad_raw),
        Err(TextProjectionError::RawByteLengthMismatch { .. })
    ));

    let mixed = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(1),
        StreamOffset::new(1),
        true,
    )
    .emit_many(
        range(0, 1),
        [Content::raw("x"), Content::block(paragraph("x", 0))],
    )
    .finish()
    .unwrap();
    assert!(matches!(
        validate_text_projection(&mixed),
        Err(TextProjectionError::RawMustBeSoleValue { .. })
    ));
}

#[test]
fn annotations_marks_and_persistent_rewriting_are_canonical() {
    let strong = Mark::Strong;
    let emphasis = Mark::Emphasis;
    let a = MarkSet::new([strong.clone(), emphasis.clone()]).unwrap();
    let b = MarkSet::new([emphasis, strong]).unwrap();
    assert_eq!(a, b);
    let link_a = Mark::Link(LinkTarget::new("/a", Some("A")));
    let link_b = Mark::Link(LinkTarget::new("/b", Some("B")));
    assert_eq!(
        MarkSet::new([link_a, link_b]),
        Err(TextIrError::DuplicateLinkMark)
    );

    let tag = SemanticTag::new("example", "annotated").unwrap();
    let key = SemanticKey::new("json", "indent").unwrap();
    let annotations = Annotations::new()
        .with_tag(tag.clone())
        .with_tag(tag.clone())
        .with_property(key.clone(), SemanticValue::Integer(2))
        .with_property(key.clone(), 4i64);
    assert_eq!(annotations.tags().len(), 1);
    assert_eq!(annotations.property(&key), Some(&SemanticValue::Integer(4)));

    let unchanged = paragraph("same", 0);
    let changed = Block::code(CodeBlock::new(
        None,
        None::<&str>,
        LiteralText::from_exact("x", range(0, 1)).unwrap(),
    ));
    let root = Block::container([unchanged.clone(), changed.clone()]);
    let mut rewriter = JsonProbe;
    let rewritten = rewriter.rewrite_block(root.clone()).unwrap();
    assert!(rewritten.ptr_eq(&root));

    let json = Block::code(CodeBlock::new(
        Some(LanguageId::new("json").unwrap()),
        None::<&str>,
        LiteralText::from_exact("x", range(0, 1)).unwrap(),
    ));
    let root = Block::container([unchanged.clone(), json]);
    let rewritten = rewriter.rewrite_block(root).unwrap();
    let children = rewritten.as_container().unwrap();
    assert!(children[0].ptr_eq(&unchanged));
    assert!(!rewritten.ptr_eq(&Block::container([])));
}

struct CountingVisitor {
    blocks: usize,
    inlines: usize,
    raws: usize,
    runs: usize,
    literals: usize,
}
impl TextVisitor for CountingVisitor {
    fn visit_block(&mut self, block: &Block) {
        self.blocks += 1;
        walk_block(self, block);
    }
    fn visit_inline(&mut self, inline: &Inline) {
        self.inlines += 1;
        iyon_tui::text::walk_inline(self, inline);
    }
    fn visit_raw(&mut self, _raw: &RawText) {
        self.raws += 1;
    }
    fn visit_text_run(&mut self, _run: &TextRun) {
        self.runs += 1;
    }
    fn visit_literal(&mut self, literal: &LiteralText) {
        self.literals += 1;
        iyon_tui::text::walk_literal(self, literal);
    }
}

#[test]
fn sequence_rewriting_and_inline_structural_sharing_are_public() {
    struct Splitter;

    impl TextRewriter for Splitter {
        type Error = Infallible;

        fn rewrite_inline_content(
            &mut self,
            content: InlineContent,
        ) -> Result<InlineContent, Self::Error> {
            let mut items = Vec::new();
            for inline in content.iter() {
                if inline
                    .as_text()
                    .is_some_and(|run| run.text() == "hello @alex")
                {
                    items.push(Inline::text(TextRun::exact("hello ", range(0, 6)).unwrap()));
                    items.push(Inline::text(TextRun::exact("@alex", range(6, 11)).unwrap()));
                } else {
                    items.push(inline.clone());
                }
            }
            Ok(InlineContent::new(items))
        }
    }

    let unchanged = Inline::text(TextRun::exact("tail", range(11, 15)).unwrap());
    let mention_paragraph = Block::paragraph(InlineContent::new([
        Inline::text(TextRun::exact("hello @alex", range(0, 11)).unwrap()),
        unchanged.clone(),
    ]));
    let mut splitter = Splitter;
    let rewritten = splitter.rewrite_block(mention_paragraph.clone()).unwrap();
    let BlockKind::Paragraph(content) = rewritten.kind() else {
        panic!("expected paragraph")
    };
    assert_eq!(content.len(), 3);
    assert!(content.items()[2].ptr_eq(&unchanged));
    assert_eq!(content.items()[0].as_text().unwrap().text(), "hello ");
    assert_eq!(content.items()[1].as_text().unwrap().text(), "@alex");

    struct AnnotateTarget;
    impl TextRewriter for AnnotateTarget {
        type Error = Infallible;
        fn rewrite_inline(&mut self, inline: Inline) -> Result<Inline, Self::Error> {
            if inline.as_text().is_some_and(|run| run.text() == "target") {
                Ok(inline.map_annotations(|annotations| {
                    annotations.with_tag(SemanticTag::new("probe", "changed").unwrap())
                }))
            } else {
                walk_rewrite_inline(self, inline)
            }
        }
    }

    let first = Inline::text(TextRun::exact("keep", range(0, 4)).unwrap());
    let second = Inline::text(TextRun::exact("target", range(4, 10)).unwrap());
    let sibling = paragraph("sibling", 10);
    let root = Block::container([
        Block::paragraph(InlineContent::new([first.clone(), second])),
        sibling.clone(),
    ]);
    let mut annotator = AnnotateTarget;
    let rewritten = annotator.rewrite_block(root).unwrap();
    let children = rewritten.as_container().unwrap();
    assert!(children[1].ptr_eq(&sibling));
    let BlockKind::Paragraph(content) = children[0].kind() else {
        panic!("expected paragraph")
    };
    assert!(content.items()[0].ptr_eq(&first));
    assert!(
        content.items()[1]
            .annotations()
            .contains_tag(&SemanticTag::new("probe", "changed").unwrap())
    );
}

#[test]
fn public_visitor_reaches_nested_lists_tables_images_and_literal_portals() {
    let text = Inline::text(exact("cell", 0));
    let image = Inline::image(Image::new(
        "img",
        None::<&str>,
        InlineContent::new([text.clone()]),
    ));
    let cell_paragraph = Block::paragraph(InlineContent::new([image]));
    let item = ListItem::new([cell_paragraph.clone()]).with_checked(Some(true));
    let list = Block::new(BlockKind::List(List::new(
        ListMarker::Ordered {
            start: 1,
            style: NumberStyle::Decimal,
            delimiter: NumberDelimiter::Period,
        },
        true,
        [item],
    )));
    let table = Block::new(BlockKind::Table(
        Table::new(
            None::<Vec<Block>>,
            [TableColumn::new(Alignment::Start)],
            1,
            [TableRow::new([TableCell::plain([cell_paragraph])])],
        )
        .unwrap(),
    ));
    let raw = Block::new(BlockKind::RawBlock {
        format: FormatId::new("html").unwrap(),
        body: LiteralText::from_exact("x", range(0, 1)).unwrap(),
    });
    let root = Block::container([list, table, raw]);
    let mut visitor = CountingVisitor {
        blocks: 0,
        inlines: 0,
        raws: 0,
        runs: 0,
        literals: 0,
    };
    visitor.visit_block(&root);
    visitor.visit_content(&Content::raw("raw"));
    assert!(visitor.blocks >= 6);
    assert_eq!(visitor.inlines, 4);
    assert_eq!(visitor.raws, 1);
    assert_eq!(visitor.runs, 3);
    assert_eq!(visitor.literals, 1);
}

#[test]
fn table_invariants_are_validated_at_construction() {
    let column = TableColumn::new(Alignment::Start);
    assert!(matches!(
        Table::new(None::<Vec<Block>>, [column], 1, []),
        Err(TextIrError::InvalidTableHeaderRows { .. })
    ));

    assert!(
        Table::new(
            None::<Vec<Block>>,
            [
                TableColumn::new(Alignment::Start),
                TableColumn::new(Alignment::End)
            ],
            0,
            [TableRow::new([TableCell::plain([])])],
        )
        .is_ok()
    );

    assert!(matches!(
        Table::new(
            None::<Vec<Block>>,
            [TableColumn::new(Alignment::Start)],
            0,
            [TableRow::new([TableCell::new(
                [],
                None,
                std::num::NonZeroU16::MIN,
                std::num::NonZeroU16::new(2).unwrap(),
            )])],
        ),
        Err(TextIrError::TableCellDoesNotFit { .. })
    ));

    assert!(matches!(
        Table::new(
            None::<Vec<Block>>,
            [TableColumn::new(Alignment::Start)],
            0,
            [TableRow::new([TableCell::new(
                [],
                None,
                std::num::NonZeroU16::new(2).unwrap(),
                std::num::NonZeroU16::MIN,
            )])],
        ),
        Err(TextIrError::TableSpanExceedsRows { .. })
    ));
}

#[test]
fn probe_projectors_preserve_multi_value_span_segmentation() {
    let first = Block::paragraph(InlineContent::empty());
    let second = Block::thematic_break();
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        true,
    )
    .emit_many(
        range(0, 4),
        [
            Content::block(first.clone()),
            Content::block(second.clone()),
        ],
    )
    .finish()
    .unwrap();
    let mut probes = JsonProbe.then(SyntaxProbe).then(MermaidProbe);
    let output = probes.project(&input).unwrap();
    validate_text_projection(&output).unwrap();
    assert_eq!(output.spans().len(), 1);
    assert_eq!(output.spans()[0].source(), range(0, 4));
    assert_eq!(output.spans()[0].values().len(), 2);
    let Content::Block(output_first) = &output.spans()[0].values()[0] else {
        panic!("expected block")
    };
    let Content::Block(output_second) = &output.spans()[0].values()[1] else {
        panic!("expected block")
    };
    assert!(output_first.ptr_eq(&first));
    assert!(output_second.ptr_eq(&second));
}

#[test]
fn no_op_rewriting_preserves_large_persistent_trees() {
    let blocks = (0..10_000)
        .map(|index| paragraph("x", index * 2))
        .collect::<Vec<_>>();
    let root = Block::container(blocks);
    let mut rewriter = JsonProbe;
    let rewritten = rewriter.rewrite_block(root.clone()).unwrap();
    assert!(rewritten.ptr_eq(&root));
}
