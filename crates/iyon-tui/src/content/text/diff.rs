//! Unified-diff input as canonical semantic text.
//!
//! Diff syntax is interpreted into semantic line roles and host-independent
//! theme references. It is deliberately a text projector rather than a
//! second renderer or a terminal escape path.

use std::ops::Range;

use crate::{
    StyleRef,
    projection::{Projection, ProjectionBuilder, ProjectionSpan, Projector},
    stream::{StreamOffset, StreamRange},
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
#[derive(Clone, Debug, Default)]
pub struct DiffProjector {
    last_base: Option<StreamOffset>,
    last_end: Option<StreamOffset>,
    in_hunk: bool,
    completed_inlines: Vec<Inline>,
    completed_end: StreamOffset,
}

impl DiffProjector {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_base: None,
            last_end: None,
            in_hunk: false,
            completed_inlines: Vec::new(),
            completed_end: StreamOffset::ZERO,
        }
    }

    fn parse_diff_domain(
        &mut self,
        domain: &RawDomain,
        is_sealed: bool,
    ) -> Result<super::Block, TextProjectionError> {
        let resume_offset = if self.completed_end > domain.source_base() {
            usize::try_from(self.completed_end.as_u64().saturating_sub(domain.source_base().as_u64()))
                .unwrap_or(0)
        } else {
            0
        };

        if resume_offset > domain.len() {
            self.in_hunk = false;
            self.completed_inlines.clear();
            self.completed_end = domain.source_base();
        }

        let parse_slice = if self.completed_end > domain.source_base() && resume_offset < domain.len() {
            domain.suffix(resume_offset)?
        } else if self.completed_end <= domain.source_base() {
            domain.clone()
        } else {
            domain.suffix(domain.len())?
        };

        let text = parse_slice.text();
        let ranges = line_ranges(text);
        let mut trailing_inlines = Vec::new();

        for range in ranges {
            let has_newline = range.end > range.start && text.as_bytes()[range.end - 1] == b'\n';
            let is_completed = has_newline || is_sealed;

            let inlines = parse_single_diff_line(
                &parse_slice,
                text,
                range.clone(),
                &mut self.in_hunk,
            )?;

            if is_completed {
                self.completed_inlines.extend(inlines);
                self.completed_end = parse_slice.source_base().saturating_add(range.end as u64);
            } else {
                trailing_inlines.extend(inlines);
            }
        }

        let mut all_inlines = self.completed_inlines.clone();
        all_inlines.extend(trailing_inlines);
        Ok(super::Block::paragraph(InlineContent::new(all_inlines)).with_origin(TextOrigin::DIFF))
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
        let is_continuation = self.last_base == Some(input.source_base())
            && self.last_end.is_some_and(|end| end <= input.source_end())
            && self.completed_end >= input.source_base();

        if !is_continuation {
            self.last_base = Some(input.source_base());
            self.in_hunk = false;
            self.completed_inlines.clear();
            self.completed_end = input.source_base();
        }
        self.last_end = Some(input.source_end());

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
            let block = self.parse_diff_domain(&domain, input.is_sealed())?;
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

fn parse_single_diff_line(
    domain: &RawDomain,
    text: &str,
    range: Range<usize>,
    in_hunk: &mut bool,
) -> Result<Vec<Inline>, TextProjectionError> {
    let mut inlines = Vec::new();
    let content_end = if range.end > range.start && text.as_bytes()[range.end - 1] == b'\n' {
        range.end - 1
    } else {
        range.end
    };
    let content_end = if content_end > range.start && text.as_bytes()[content_end - 1] == b'\r' {
        content_end - 1
    } else {
        content_end
    };
    let line = &text[range.start..content_end];
    let (style_key, tag_name, body_start) = if line.starts_with("@@ ") {
        *in_hunk = true;
        ("diff.header", "header", range.start)
    } else if line.starts_with(r"\ No newline") {
        ("diff.meta", "meta", range.start)
    } else if *in_hunk && line.starts_with('+') && !line.starts_with("+++") {
        ("diff.addition", "addition", range.start + 1)
    } else if *in_hunk && line.starts_with('-') && !line.starts_with("---") {
        ("diff.deletion", "deletion", range.start + 1)
    } else if *in_hunk && line.starts_with(' ') {
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
    Ok(inlines)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_projection(text: &str, sealed: bool) -> Projection<TextContent> {
        let b = StreamOffset::ZERO;
        let e = StreamOffset::new(text.len() as u64);
        ProjectionBuilder::new(b, e, e, sealed)
            .emit(StreamRange::new(b, e), TextContent::raw(text))
            .finish()
            .unwrap()
    }

    #[test]
    fn incremental_diff_matches_one_shot() {
        let diff_text = "\
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 context line
-old line
+new line 1
+new line 2
 trailing context
";
        let mut one_shot = DiffProjector::new();
        let expected = one_shot.project(&raw_projection(diff_text, true)).unwrap();

        // Feed line by line incrementally
        let mut incremental = DiffProjector::new();
        let mut accumulated = String::new();
        let mut last_proj = None;
        for line in diff_text.lines() {
            accumulated.push_str(line);
            accumulated.push('\n');
            let is_final = accumulated.len() == diff_text.len();
            let proj = incremental
                .project(&raw_projection(&accumulated, is_final))
                .unwrap();
            last_proj = Some(proj);
        }

        let actual = last_proj.expect("at least one projection");
        assert_eq!(actual.spans().len(), expected.spans().len());
        for (a, b) in actual.spans().iter().zip(expected.spans()) {
            assert_eq!(a.source(), b.source());
            assert_eq!(a.values(), b.values());
        }
    }

    #[test]
    fn unsealed_partial_line_resumes_correctly() {
        let mut incremental = DiffProjector::new();
        // First chunk has partial line
        let p1 = incremental
            .project(&raw_projection("@@ -1,1 +1,1 @@\n+hell", false))
            .unwrap();
        assert_eq!(p1.spans().len(), 1);

        // Second chunk completes the line and adds another
        let p2 = incremental
            .project(&raw_projection("@@ -1,1 +1,1 @@\n+hello world\n-goodbye\n", true))
            .unwrap();

        let mut one_shot = DiffProjector::new();
        let expected = one_shot
            .project(&raw_projection("@@ -1,1 +1,1 @@\n+hello world\n-goodbye\n", true))
            .unwrap();

        assert_eq!(p2.spans().len(), expected.spans().len());
        for (a, b) in p2.spans().iter().zip(expected.spans()) {
            assert_eq!(a.source(), b.source());
            assert_eq!(a.values(), b.values());
        }
    }
}
