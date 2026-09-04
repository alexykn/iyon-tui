use super::origin::stamp_block_origin;
use super::{
    Block, BreakKind, Inline, InlineContent, TextContent, TextOrigin, TextProjectionError,
    validate_text_projection,
};
use crate::projection::{Projection, ProjectionBuilder, Projector};
use crate::stream::{StreamOffset, StreamRange};

/// A projector that claims each consecutive Raw domain as literal prose.
#[derive(Clone, Debug, Default)]
pub struct PlainTextProjector;

impl PlainTextProjector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Projector<TextContent> for PlainTextProjector {
    type Output = TextContent;
    type Error = TextProjectionError;

    fn project(
        &mut self,
        input: &Projection<TextContent>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        validate_text_projection(input)?;
        let mut builder = ProjectionBuilder::new(
            input.source_base(),
            input.source_base(),
            input.source_end(),
            false,
        );
        let mut index = 0;
        while index < input.spans().len() {
            let span = &input.spans()[index];
            let is_raw =
                span.values().len() == 1 && matches!(span.values()[0], TextContent::Raw(_));
            if !is_raw {
                builder = builder.emit_many(span.source(), span.values().iter().cloned());
                index += 1;
                continue;
            }

            let start = index;
            index += 1;
            while index < input.spans().len()
                && input.spans()[index].values().len() == 1
                && matches!(input.spans()[index].values()[0], TextContent::Raw(_))
            {
                index += 1;
            }
            let domain_spans = &input.spans()[start..index];
            let domain = RawDomain::from_spans(domain_spans)?;
            let block = stamp_block_origin(literal_paragraph(&domain)?, &TextOrigin::PLAIN_TEXT);
            let source = StreamRange::new(domain.source_base(), domain.source_end());
            builder = builder.emit(source, TextContent::Block(block));
        }

        let stable = plain_stability(input, &mut builder)?;
        // The helper above only computes the frontier; rebuilding keeps the
        // builder API as the sole projection construction boundary.
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            stable,
            input.source_end(),
            input.is_sealed(),
        );
        let provisional = builder.finish().map_err(TextProjectionError::Projection)?;
        for span in provisional.spans() {
            output = output.emit_many(span.source(), span.values().iter().cloned());
        }
        output.finish().map_err(TextProjectionError::Projection)
    }
}

fn literal_paragraph(domain: &RawDomain) -> Result<Block, TextProjectionError> {
    let mut inlines = Vec::new();
    let mut segment_start = 0;
    for (offset, character) in domain.text().char_indices() {
        if character != '\n' {
            continue;
        }
        if segment_start < offset {
            inlines.extend(exact_runs(domain, segment_start..offset)?);
        }
        inlines.push(Inline::break_(BreakKind::Hard));
        segment_start = offset + character.len_utf8();
    }
    if segment_start < domain.len() {
        inlines.extend(exact_runs(domain, segment_start..domain.len())?);
    }
    Ok(Block::paragraph(InlineContent::new(inlines)))
}

fn exact_runs(
    domain: &RawDomain,
    local: std::ops::Range<usize>,
) -> Result<Vec<Inline>, TextProjectionError> {
    Ok(domain
        .exact_runs(local)?
        .into_iter()
        .map(Inline::text)
        .collect())
}

fn plain_stability(
    input: &Projection<TextContent>,
    _builder: &mut ProjectionBuilder<TextContent>,
) -> Result<StreamOffset, TextProjectionError> {
    if input.is_sealed() {
        return Ok(input.source_end());
    }
    let mut stable = input.source_base();
    let mut index = 0;
    while index < input.spans().len() {
        let span = &input.spans()[index];
        let raw = span.values().len() == 1 && matches!(span.values()[0], TextContent::Raw(_));
        if raw {
            index += 1;
            while index < input.spans().len()
                && input.spans()[index].values().len() == 1
                && matches!(input.spans()[index].values()[0], TextContent::Raw(_))
            {
                index += 1;
            }
            let domain_end = input.spans()[index - 1].source().end();
            let barrier_is_stable = index < input.spans().len()
                && input.spans()[index].source().end() <= input.stable_through();
            if input.stable_through() >= domain_end && barrier_is_stable {
                stable = domain_end;
                continue;
            }
            break;
        }
        if span.source().end() > input.stable_through() {
            break;
        }
        stable = span.source().end();
        index += 1;
    }
    Ok(stable)
}

use super::source::RawDomain;
