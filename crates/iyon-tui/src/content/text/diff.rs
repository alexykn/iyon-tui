//! Unified-diff input as canonical semantic text.
//!
//! Diff syntax is interpreted into semantic line roles and host-independent
//! theme references. It is deliberately a text projector rather than a
//! second renderer or a terminal escape path.

use std::ops::Range;

use crate::{
    StyleRef,
    projection::{Projection, ProjectionBuilder, ProjectionSpan, Projector},
    stream::StreamRange,
};

use super::source::RawDomain;
use super::{
    Annotations, BreakKind, Inline, InlineContent, SemanticTag, TextContent, TextOrigin,
    TextProjectionError, TextRun, validate_text_projection,
};

/// Diff projector errors are source/projection validation errors. Malformed
/// diff syntax is retained as styled metadata/plain lines instead of making a
/// frame fail.
pub type DiffProjectionError = TextProjectionError;

/// Converts unified diff text into the canonical semantic text IR.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiffProjector;

impl DiffProjector {
    pub const fn new() -> Self {
        Self
    }
}

impl Projector<TextContent> for DiffProjector {
    type Output = TextContent;
    type Error = DiffProjectionError;

    fn project(
        &mut self,
        input: &Projection<TextContent>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        validate_text_projection(input)?;
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        let mut index = 0;
        while index < input.spans().len() {
            let span = &input.spans()[index];
            if !is_raw_span(span) {
                output = output.emit_many(span.source(), span.values().iter().cloned());
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            while index < input.spans().len() && is_raw_span(&input.spans()[index]) {
                index += 1;
            }
            let domain = RawDomain::from_spans(&input.spans()[start..index])?;
            let block = parse_domain(&domain)?;
            output = output.emit(
                StreamRange::new(domain.source_base(), domain.source_end()),
                TextContent::Block(block),
            );
        }
        output.finish().map_err(TextProjectionError::Projection)
    }
}

fn is_raw_span(span: &ProjectionSpan<TextContent>) -> bool {
    span.values().len() == 1 && matches!(span.values()[0], TextContent::Raw(_))
}

fn parse_domain(domain: &RawDomain) -> Result<super::Block, TextProjectionError> {
    let text = domain.text();
    let mut inlines = Vec::new();
    let mut in_hunk = false;
    for range in line_ranges(text) {
        let content_end = if range.end > range.start && text.as_bytes()[range.end - 1] == b'\n' {
            range.end - 1
        } else {
            range.end
        };
        let content_end = if content_end > range.start && text.as_bytes()[content_end - 1] == b'\r'
        {
            content_end - 1
        } else {
            content_end
        };
        let line = &text[range.start..content_end];
        let (style_key, tag_name, body_start) = if line.starts_with("@@ ") {
            in_hunk = true;
            ("diff.header", "header", range.start)
        } else if line.starts_with(r"\ No newline") {
            ("diff.meta", "meta", range.start)
        } else if in_hunk && line.starts_with('+') && !line.starts_with("+++") {
            ("diff.addition", "addition", range.start + 1)
        } else if in_hunk && line.starts_with('-') && !line.starts_with("---") {
            ("diff.deletion", "deletion", range.start + 1)
        } else if in_hunk && line.starts_with(' ') {
            ("diff.context", "context", range.start + 1)
        } else {
            ("diff.meta", "meta", range.start)
        };
        let tag = SemanticTag::new("diff", tag_name).map_err(TextProjectionError::Ir)?;
        let style = StyleRef::theme(style_key);
        let body_range = body_start.min(content_end)..content_end;
        if body_range.start > range.start {
            let marker = &text[range.start..body_range.start];
            inlines.push(Inline::text(
                TextRun::synthetic(marker)
                    .with_annotations(Annotations::new().with_tag(tag.clone()))
                    .with_style(style.clone()),
            ));
        }
        if body_range.start < body_range.end {
            for run in domain.exact_runs(body_range)? {
                inlines.push(Inline::text(
                    run.with_annotations(Annotations::new().with_tag(tag.clone()))
                        .with_style(style.clone()),
                ));
            }
        }
        if range.end > content_end {
            inlines.push(Inline::break_(BreakKind::Hard));
        }
    }
    Ok(super::Block::paragraph(InlineContent::new(inlines)).with_origin(TextOrigin::DIFF))
}

fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, character) in text.char_indices() {
        if character != '\n' {
            continue;
        }
        ranges.push(start..offset + 1);
        start = offset + 1;
    }
    if start < text.len() {
        ranges.push(start..text.len());
    }
    ranges
}
