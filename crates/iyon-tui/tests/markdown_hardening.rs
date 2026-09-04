use std::fmt::Write as _;
use std::time::{Duration, Instant};

use iyon_tui::projection::{ProjectionBuilder, validate_projection_transition};
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{
    Alignment, List, ListItem, ListMarker, Mark, MarkSet, NumberDelimiter, NumberStyle,
    TextProvenance, TextRun, TextVisitor, validate_text_projection,
};
use iyon_tui::text::{Table, TableCell, TableColumn, TableRow};
use iyon_tui::{
    Block, Inline, InlineContent, MarkdownProjector, Projection, Projector, Renderer, Smooth,
    TextContent, TextRenderer,
};

fn source_projection(
    source: &str,
    stable: usize,
    sealed: bool,
    base: usize,
) -> Projection<TextContent> {
    let start = StreamOffset::new(base as u64);
    let end = StreamOffset::new((base + source.len()) as u64);
    let mut builder = ProjectionBuilder::new(
        start,
        StreamOffset::new((base + stable) as u64),
        end,
        sealed,
    );
    if source.is_empty() {
        return builder.finish().unwrap();
    }
    if stable > 0 && stable < source.len() {
        builder = builder.emit(
            StreamRange::new(start, StreamOffset::new((base + stable) as u64)),
            TextContent::raw(&source[..stable]),
        );
        builder = builder.emit(
            StreamRange::new(StreamOffset::new((base + stable) as u64), end),
            TextContent::raw(&source[stable..]),
        );
    } else {
        builder = builder.emit(StreamRange::new(start, end), TextContent::raw(source));
    }
    builder.finish().unwrap()
}

fn sealed(source: &str) -> Projection<TextContent> {
    source_projection(source, source.len(), true, 0)
}

fn segmented(source: &str, cuts: &[usize], sealed: bool) -> Projection<TextContent> {
    let end = StreamOffset::new(source.len() as u64);
    let mut boundaries = vec![0];
    boundaries.extend(
        cuts.iter()
            .copied()
            .filter(|&cut| cut > 0 && cut < source.len() && source.is_char_boundary(cut)),
    );
    boundaries.push(source.len());
    boundaries.sort_unstable();
    boundaries.dedup();
    let stable = if sealed { end } else { StreamOffset::ZERO };
    let mut builder = ProjectionBuilder::new(StreamOffset::ZERO, stable, end, sealed);
    for window in boundaries.windows(2) {
        let start = window[0];
        let finish = window[1];
        builder = builder.emit(
            StreamRange::new(
                StreamOffset::new(start as u64),
                StreamOffset::new(finish as u64),
            ),
            TextContent::raw(&source[start..finish]),
        );
    }
    builder.finish().unwrap()
}

struct TextSummary {
    text: String,
    sources: Vec<StreamRange>,
}

impl TextSummary {
    fn new() -> Self {
        Self {
            text: String::new(),
            sources: Vec::new(),
        }
    }
}

impl TextVisitor for TextSummary {
    fn visit_raw(&mut self, raw: &iyon_tui::RawText) {
        self.text.push_str(raw.text());
    }

    fn visit_text_run(&mut self, run: &TextRun) {
        self.text.push_str(run.text());
        let Some(provenance) = (match run.provenance() {
            TextProvenance::Exact(range) | TextProvenance::Derived(range) => Some(*range),
            TextProvenance::Synthetic => None,
        }) else {
            return;
        };
        if let Some(previous) = self.sources.last_mut()
            && previous.end() == provenance.start()
        {
            *previous = StreamRange::new(previous.start(), provenance.end());
        } else {
            self.sources.push(provenance);
        }
    }
}

fn signature(
    output: &Projection<TextContent>,
) -> Vec<(StreamRange, Vec<(String, Vec<StreamRange>)>)> {
    output
        .spans()
        .iter()
        .map(|span| {
            let values = span
                .values()
                .iter()
                .map(|value| {
                    let mut summary = TextSummary::new();
                    iyon_tui::text::walk_content(&mut summary, value);
                    (summary.text, summary.sources)
                })
                .collect();
            (span.source(), values)
        })
        .collect()
}

#[test]
fn sealed_markdown_is_independent_of_raw_transport_segmentation() {
    let source = "# title\n\nalpha *beta*\n\n```json\n{\"ok\":true}\n```\n\nlast\n";
    let mut one = MarkdownProjector::default();
    let expected = one.project(&sealed(source)).unwrap();
    let expected_signature = signature(&expected);
    let segmentations = [vec![], vec![1, 8, 9, 20, 31, 32, source.len()]];
    for cuts in segmentations {
        let mut projector = MarkdownProjector::default();
        let output = projector.project(&segmented(source, &cuts, true)).unwrap();
        assert_eq!(signature(&output), expected_signature);
    }
    let mut seed = 0x9e37_79b9_u64;
    let mut cuts = Vec::new();
    for _ in 0..32 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        cuts.push((seed as usize) % source.len());
    }
    let mut projector = MarkdownProjector::default();
    let output = projector.project(&segmented(source, &cuts, true)).unwrap();
    assert_eq!(signature(&output), expected_signature);
}

fn semantic(output: &Projection<TextContent>) -> Vec<TextContent> {
    output
        .spans()
        .iter()
        .flat_map(|span| span.values().iter().cloned())
        .collect()
}

#[test]
fn restart_includes_reference_context_and_supports_exact_compaction() {
    let source = "[foo]: /target\n\none\n\ntwo\n\n[foo]\n";
    let mut projector = MarkdownProjector::default();
    let output = projector
        .project(&source_projection(source, source.len(), false, 0))
        .unwrap();
    let reference = StreamOffset::new((source.len() - 5) as u64);
    let restart = projector.restart_from(reference);
    assert!(restart <= reference);
    assert_eq!(restart, StreamOffset::ZERO);

    let retained = &source[restart.as_u64() as usize..];
    let compacted = source_projection(retained, retained.len(), true, restart.as_u64() as usize);
    let mut resumed = projector;
    let resumed_output = resumed.project(&compacted).unwrap();
    let mut fresh = MarkdownProjector::default();
    let fresh_output = fresh.project(&sealed(source)).unwrap();
    assert_eq!(resumed_output, fresh_output);
    assert_eq!(output.source_base(), StreamOffset::ZERO);
}

#[test]
fn stable_closed_blocks_are_published_inside_a_raw_domain() {
    let source = (0..100)
        .map(|index| format!("block {index}\n\n"))
        .collect::<String>()
        + "tail";
    let stable = source.len() - 4;
    let mut projector = MarkdownProjector::default();
    let output = projector
        .project(&source_projection(source.as_str(), stable, false, 0))
        .unwrap();
    assert!(output.stable_through().as_u64() > 0);
    assert!(output.stable_through().as_u64() <= stable as u64);
    validate_text_projection(&output).unwrap();
}

#[test]
fn unresolved_references_remain_mutable_until_sealed_resolution() {
    let mut projector = MarkdownProjector::default();
    let mut previous = None;
    let stable_prefix = "[foo]\n\n";
    for tail in ["[foo]: /a", "[foo]: /b", "[foo]: /c"] {
        let source = format!("{stable_prefix}{tail}");
        let next = projector
            .project(&source_projection(&source, stable_prefix.len(), false, 0))
            .unwrap();
        validate_text_projection(&next).unwrap();
        if let Some(previous) = &previous {
            validate_projection_transition(previous, &next).unwrap();
        }
        previous = Some(next);
    }
    let final_source = "[foo]\n\n[foo]: /b\n";
    let final_output = projector
        .project(&source_projection(
            final_source,
            final_source.len(),
            true,
            0,
        ))
        .unwrap();
    let mut fresh = MarkdownProjector::default();
    assert_eq!(final_output, fresh.project(&sealed(final_source)).unwrap());
}

#[test]
fn nonzero_root_coordinates_are_preserved() {
    let source = "é\n\nnext";
    let mut projector = MarkdownProjector::default();
    let output = projector
        .project(&source_projection(source, source.len(), true, 1000))
        .unwrap();
    assert_eq!(output.source_base(), StreamOffset::new(1000));
    let run = semantic(&output)
        .into_iter()
        .find_map(|value| match value {
            TextContent::Block(block) => match block.kind() {
                iyon_tui::text::BlockKind::Paragraph(content) => content
                    .items()
                    .iter()
                    .find_map(|inline| inline.as_text().cloned()),
                _ => None,
            },
            TextContent::Raw(_) => None,
        })
        .unwrap();
    assert_eq!(
        run.provenance(),
        &TextProvenance::Exact(StreamRange::new(
            StreamOffset::new(1000),
            StreamOffset::new(1002)
        ))
    );
}

#[test]
fn markdown_composes_after_smooth_without_special_streaming_api() {
    let source = "first\n\nsecond\n";
    let open = source_projection(source, 0, false, 0);
    let sealed_input = source_projection(source, source.len(), true, 0);
    let mut smooth = Smooth::default();
    let mut markdown = MarkdownProjector::default();
    let mut previous = None;
    for now in [
        Instant::now(),
        Instant::now() + Duration::from_millis(100),
        Instant::now() + Duration::from_secs(1),
    ] {
        let _ = smooth.advance(now);
        let published = smooth.project(&open).unwrap();
        let output = markdown.project(&published).unwrap();
        validate_text_projection(&output).unwrap();
        if let Some(previous) = &previous {
            validate_projection_transition(previous, &output).unwrap();
        }
        previous = Some(output);
    }
    let flushed = smooth.project(&sealed_input).unwrap();
    let final_output = markdown.project(&flushed).unwrap();
    let mut fresh = MarkdownProjector::default();
    assert_eq!(final_output, fresh.project(&sealed_input).unwrap());
}

#[test]
fn renderer_preserves_generic_list_table_and_image_semantics() {
    let paragraph =
        |text: &str| Block::paragraph(InlineContent::new([Inline::text(TextRun::synthetic(text))]));
    let list = List::new(
        ListMarker::Ordered {
            start: 1,
            style: NumberStyle::LowerAlpha,
            delimiter: NumberDelimiter::TwoParens,
        },
        true,
        [ListItem::new([paragraph("item")])],
    );
    let caption = [paragraph("caption")];
    let table = Table::new(
        Some(caption),
        [TableColumn::new(Alignment::Center)],
        1,
        [TableRow::new([TableCell::plain([paragraph("cell")])])],
    )
    .unwrap();
    let alt =
        Inline::text(TextRun::synthetic("alt")).with_marks(MarkSet::new([Mark::Strong]).unwrap());
    let image = Inline::image(iyon_tui::text::Image::new(
        "image",
        None::<&str>,
        InlineContent::new([alt]),
    ))
    .with_marks(MarkSet::new([Mark::Emphasis]).unwrap());
    let block = Block::paragraph(InlineContent::new([image]));
    let renderer = TextRenderer::new();
    let list_view = renderer.render(&TextContent::block(Block::list(list)));
    let list_text = format!("{list_view:?}");
    assert!(list_text.contains("(a) "));
    let table_view = renderer.render(&TextContent::block(Block::table(table)));
    let table_text = format!("{table_view:?}");
    assert!(table_text.contains("caption"));
    assert!(table_text.contains("cell"));
    let image_view = renderer.render(&TextContent::block(block));
    let image_text = format!("{image_view:?}");
    assert!(image_text.contains("alt"));
}

#[test]
fn smooth_markdown_final_result_is_chunk_invariant() {
    let source = "ASCII\n\né\n\n中\n\na\u{301}\n\n👩‍💻\n";
    let mut one = MarkdownProjector::default();
    let expected = one.project(&sealed(source)).unwrap();
    for split in 0..=source.len() {
        if !source.is_char_boundary(split) {
            continue;
        }
        let mut projector = MarkdownProjector::default();
        let first = source_projection(&source[..split], split, false, 0);
        let _ = projector.project(&first).unwrap();
        let output = projector.project(&sealed(source)).unwrap();
        assert_eq!(output, expected);

        let mut chunked = MarkdownProjector::default();
        let mut previous = None;
        for end in source
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
        {
            let snapshot = source_projection(&source[..end], end, false, 0);
            let next = chunked.project(&snapshot).unwrap();
            validate_text_projection(&next).unwrap();
            if let Some(previous) = &previous {
                validate_projection_transition(previous, &next).unwrap();
            }
            previous = Some(next);
        }
        let chunked_final = chunked.project(&sealed(source)).unwrap();
        assert_eq!(chunked_final, expected);
    }
}

#[test]
#[cfg(feature = "test-util")]
fn stable_cache_reuses_closed_prefixes() {
    let mut projector = MarkdownProjector::default();
    let mut source = String::new();
    for index in 0..1000 {
        let _ = write!(source, "block {index}\n\n");
        let output = projector
            .project(&source_projection(&source, source.len(), false, 0))
            .unwrap();
        validate_text_projection(&output).unwrap();
    }
    let (invocations, bytes) = projector.parser_work();
    println!(
        "1000-block parser work: invocations={invocations}, bytes={bytes}, source={}",
        source.len()
    );
    assert!(invocations > 0);
    assert!(bytes < source.len().saturating_mul(100));
}

#[test]
fn renderer_accepts_all_number_styles() {
    let text = |value: &str| {
        Block::paragraph(InlineContent::new([Inline::text(TextRun::synthetic(
            value,
        ))]))
    };
    let render_marker = |start, style, delimiter| {
        let list = List::new(
            ListMarker::Ordered {
                start,
                style,
                delimiter,
            },
            true,
            [ListItem::new([text("x")])],
        );
        format!(
            "{:?}",
            TextRenderer::new().render(&TextContent::block(Block::list(list)))
        )
    };
    assert!(render_marker(9, NumberStyle::Decimal, NumberDelimiter::Period).contains("9. "));
    assert!(render_marker(9, NumberStyle::Decimal, NumberDelimiter::Paren).contains("9) "));
    assert!(render_marker(9, NumberStyle::Decimal, NumberDelimiter::TwoParens).contains("(9) "));
    assert!(render_marker(1, NumberStyle::LowerAlpha, NumberDelimiter::Paren).contains("a) "));
    assert!(render_marker(27, NumberStyle::UpperAlpha, NumberDelimiter::Paren).contains("AA) "));
    assert!(render_marker(9, NumberStyle::LowerRoman, NumberDelimiter::Paren).contains("ix) "));
    assert!(render_marker(9, NumberStyle::UpperRoman, NumberDelimiter::Paren).contains("IX) "));
}
