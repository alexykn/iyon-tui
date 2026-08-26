use std::{cell::RefCell, ops::Range, rc::Rc, sync::Arc};

use pulldown_cmark::{
    Alignment as PdAlignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd,
};

use crate::{
    projection::{Projection, ProjectionBuilder, ProjectionSpan, Projector},
    stream::{StreamOffset, StreamRange},
};

use super::markdown_options::MarkdownOptions;
use super::origin::stamp_block_origin;
use super::source::RawDomain;
use super::{
    Alignment, Block, BlockKind, BreakKind, CodeBlock, FormatId, HeadingLevel, Image, Inline,
    InlineContent, InlineKind, LanguageId, LinkTarget, List, ListItem, ListMarker, LiteralText,
    Mark, MarkSet, NumberDelimiter, NumberStyle, Table, TableCell, TableColumn, TableRow,
    TextContent, TextIrError, TextOrigin, TextProjectionError, TextRun, validate_text_projection,
};

/// Errors raised while converting CommonMark events to generic text IR.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownProjectionError {
    Text(TextProjectionError),
    Ir(TextIrError),
    InvalidSourceMap {
        context: &'static str,
    },
    InvalidNesting {
        context: &'static str,
    },
    InsufficientRestartContext {
        source_base: StreamOffset,
        required_from: StreamOffset,
    },
    ParserInvariant {
        context: &'static str,
    },
}

impl std::fmt::Display for MarkdownProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(error) => error.fmt(f),
            Self::Ir(error) => error.fmt(f),
            Self::InvalidSourceMap { context }
            | Self::InvalidNesting { context }
            | Self::ParserInvariant { context } => {
                write!(f, "Markdown projection error: {context}")
            }
            Self::InsufficientRestartContext {
                source_base,
                required_from,
            } => write!(
                f,
                "Markdown input starts at {source_base:?}, but retained parsing requires {required_from:?}"
            ),
        }
    }
}

impl std::error::Error for MarkdownProjectionError {}

impl From<TextProjectionError> for MarkdownProjectionError {
    fn from(error: TextProjectionError) -> Self {
        Self::Text(error)
    }
}

impl From<TextIrError> for MarkdownProjectionError {
    fn from(error: TextIrError) -> Self {
        Self::Ir(error)
    }
}

/// Stateful, non-temporal CommonMark-to-TextContent projector.
#[derive(Debug)]
pub struct MarkdownProjector {
    options: MarkdownOptions,
    required_restart_from: Option<StreamOffset>,
    last_stable: Option<StreamOffset>,
    checkpoints: Vec<StreamRange>,
    caches: Vec<CachedDomain>,
    #[cfg(feature = "test-util")]
    parser_invocations: usize,
    #[cfg(feature = "test-util")]
    parser_bytes: usize,
}

#[derive(Clone, Debug)]
struct CachedDomain {
    source_base: StreamOffset,
    stable_end: StreamOffset,
    prefix: String,
    spans: Vec<CachedSpan>,
    has_reference_context: bool,
}

#[derive(Clone, Debug)]
struct CachedSpan {
    source: StreamRange,
    values: Vec<TextContent>,
}

#[derive(Debug)]
struct ParsedDomain {
    projection: Projection<TextContent>,
    unstable_from: Option<StreamOffset>,
    has_reference_context: bool,
}

impl MarkdownProjector {
    pub fn new(options: MarkdownOptions) -> Self {
        Self {
            options,
            required_restart_from: None,
            last_stable: None,
            checkpoints: Vec::new(),
            caches: Vec::new(),
            #[cfg(feature = "test-util")]
            parser_invocations: 0,
            #[cfg(feature = "test-util")]
            parser_bytes: 0,
        }
    }

    pub fn options(&self) -> MarkdownOptions {
        self.options
    }

    #[cfg(feature = "test-util")]
    #[doc(hidden)]
    pub fn parser_work(&self) -> (usize, usize) {
        (self.parser_invocations, self.parser_bytes)
    }
}

impl Default for MarkdownProjector {
    fn default() -> Self {
        Self::new(MarkdownOptions::commonmark())
    }
}

impl Projector<TextContent> for MarkdownProjector {
    type Output = TextContent;
    type Error = MarkdownProjectionError;

    fn project(
        &mut self,
        input: &Projection<TextContent>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        validate_text_projection(input)?;
        if let Some(required) = self.required_restart_from {
            if input.source_base() > required {
                return Err(MarkdownProjectionError::InsufficientRestartContext {
                    source_base: input.source_base(),
                    required_from: required,
                });
            }
        }

        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.source_base(),
            input.source_end(),
            false,
        );
        let mut global_stable = self.last_stable.unwrap_or(input.source_base());
        let mut prefix_open = true;
        let mut index = 0;

        while index < input.spans().len() {
            if !is_raw_span(&input.spans()[index]) {
                let span = &input.spans()[index];
                output = output.emit_many(span.source(), span.values().iter().cloned());
                if prefix_open && span.source().end() <= input.stable_through() {
                    global_stable = global_stable.max(span.source().end());
                } else {
                    prefix_open = false;
                }
                index += 1;
                continue;
            }

            let start = index;
            index += 1;
            while index < input.spans().len() && is_raw_span(&input.spans()[index]) {
                index += 1;
            }
            let domain = RawDomain::from_spans(&input.spans()[start..index])?;
            let domain_closed = input.is_sealed() || index < input.spans().len();
            let mut parsed = self.parse_domain(&domain, domain_closed)?;
            if !input.is_sealed() && has_open_reference_definition_prefix(domain.text()) {
                parsed.unstable_from = Some(
                    parsed
                        .unstable_from
                        .map_or(domain.source_base(), |from| from.min(domain.source_base())),
                );
            }
            for span in parsed.projection.spans() {
                output = output.emit_many(span.source(), span.values().iter().cloned());
            }

            let barrier_stable = index < input.spans().len()
                && input.spans()[index].source().end() <= input.stable_through();
            let domain_stable = if input.is_sealed() && domain.source_end() == input.source_end() {
                domain.source_end()
            } else if barrier_stable {
                domain.source_end()
            } else {
                self.stable_prefix_end(input.stable_through(), &domain, &parsed)
            };
            if prefix_open && domain_stable >= global_stable {
                global_stable = domain_stable;
            } else {
                prefix_open = false;
            }

            self.update_cache(&domain, &parsed, domain_stable, input.is_sealed());
        }

        if input.is_sealed() {
            global_stable = input.source_end();
        }
        global_stable = global_stable
            .min(input.source_end())
            .max(input.source_base());

        let provisional = output.finish().map_err(TextProjectionError::from)?;
        global_stable = snap_stable_to_span_boundary(
            global_stable,
            input.source_base(),
            input.source_end(),
            provisional.spans(),
        );
        let mut final_builder = ProjectionBuilder::new(
            input.source_base(),
            global_stable,
            input.source_end(),
            input.is_sealed(),
        );
        self.checkpoints.clear();
        for span in provisional.spans() {
            if !span.values().is_empty() {
                self.checkpoints.push(span.source());
            }
            final_builder = final_builder.emit_many(span.source(), span.values().iter().cloned());
        }
        self.last_stable = Some(global_stable);
        final_builder
            .finish()
            .map_err(TextProjectionError::from)
            .map_err(Into::into)
    }

    fn restart_from(&self, output_from: StreamOffset) -> StreamOffset {
        let block_restart = self
            .checkpoints
            .iter()
            .find(|range| range.contains_offset(output_from) || range.start() == output_from)
            .map_or(output_from, |range| range.start());
        self.required_restart_from
            .map_or(block_restart, |required| block_restart.min(required))
            .min(output_from)
    }
}

impl MarkdownProjector {
    fn stable_prefix_end(
        &mut self,
        input_stable: StreamOffset,
        domain: &RawDomain,
        parsed: &ParsedDomain,
    ) -> StreamOffset {
        let last_semantic_start = open_block_start(parsed.projection.spans(), domain.source_base());
        if last_semantic_start == domain.source_base() {
            return domain.source_base();
        }

        let limit = parsed
            .unstable_from
            .unwrap_or(input_stable)
            .min(input_stable)
            .min(last_semantic_start);
        let Some(candidate) = parsed
            .projection
            .spans()
            .iter()
            .map(|span| span.source().end())
            .filter(|end| *end <= limit)
            .max()
        else {
            return domain.source_base();
        };
        if candidate == domain.source_base() {
            return candidate;
        }

        let prefix_parsed = if let Some(cache) = self
            .caches
            .iter()
            .find(|cache| {
                cache.source_base == domain.source_base()
                    && !cache.has_reference_context
                    && cache.stable_end <= candidate
                    && domain
                        .text_prefix(cache.stable_end)
                        .is_some_and(|prefix| prefix == cache.prefix)
            })
            .cloned()
        {
            let local_cache_end = usize::try_from(
                cache
                    .stable_end
                    .as_u64()
                    .saturating_sub(domain.source_base().as_u64()),
            )
            .unwrap_or(usize::MAX);
            if cache.stable_end == candidate {
                cached_projection(&cache).ok()
            } else {
                let Ok(suffix) = domain.suffix(local_cache_end) else {
                    return domain.source_base();
                };
                let suffix_end =
                    usize::try_from(candidate.as_u64().saturating_sub(cache.stable_end.as_u64()))
                        .unwrap_or(usize::MAX);
                let Ok(suffix) = suffix.prefix(suffix_end) else {
                    return domain.source_base();
                };
                self.parse_uncached(&suffix, true)
                    .ok()
                    .and_then(|parsed| prepend_cached(&cache, parsed.projection).ok())
            }
        } else {
            let Ok(prefix) = domain.prefix(
                usize::try_from(
                    candidate
                        .as_u64()
                        .saturating_sub(domain.source_base().as_u64()),
                )
                .unwrap_or(usize::MAX),
            ) else {
                return domain.source_base();
            };
            self.parse_uncached(&prefix, true)
                .ok()
                .map(|parsed| parsed.projection)
        };
        let Some(prefix_parsed) = prefix_parsed else {
            return domain.source_base();
        };
        let mut expected = ProjectionBuilder::new(domain.source_base(), candidate, candidate, true);
        for span in parsed
            .projection
            .spans()
            .iter()
            .take_while(|span| span.source().end() <= candidate)
        {
            expected = expected.emit_many(span.source(), span.values().iter().cloned());
        }
        let Ok(expected) = expected.finish() else {
            return domain.source_base();
        };
        if prefix_parsed == expected {
            candidate
        } else {
            domain.source_base()
        }
    }

    fn parse_uncached(
        &mut self,
        domain: &RawDomain,
        sealed: bool,
    ) -> Result<ParsedDomain, MarkdownProjectionError> {
        #[cfg(feature = "test-util")]
        {
            self.parser_invocations = self.parser_invocations.saturating_add(1);
            self.parser_bytes = self.parser_bytes.saturating_add(domain.len());
        }
        parse_domain_uncached(domain, self.options, sealed)
    }

    fn parse_domain(
        &mut self,
        domain: &RawDomain,
        sealed: bool,
    ) -> Result<ParsedDomain, MarkdownProjectionError> {
        if let Some(cache) = self
            .caches
            .iter()
            .find(|cache| {
                cache.source_base == domain.source_base()
                    && cache.stable_end <= domain.source_end()
                    && domain
                        .text_prefix(cache.stable_end)
                        .is_some_and(|prefix| prefix == cache.prefix)
            })
            .cloned()
        {
            if cache.stable_end > domain.source_base() {
                if cache.stable_end == domain.source_end() && !cache.has_reference_context {
                    return Ok(ParsedDomain {
                        projection: cached_projection(&cache)?,
                        unstable_from: None,
                        has_reference_context: false,
                    });
                }
                if cache.has_reference_context {
                    return self.parse_uncached(domain, sealed);
                }
                let local = usize::try_from(
                    cache
                        .stable_end
                        .as_u64()
                        .saturating_sub(domain.source_base().as_u64()),
                )
                .map_err(|_| MarkdownProjectionError::InvalidSourceMap {
                    context: "cache restart offset",
                })?;
                let suffix = domain.suffix(local)?;
                let parsed = self.parse_uncached(&suffix, sealed)?;
                return Ok(ParsedDomain {
                    projection: prepend_cached(&cache, parsed.projection)?,
                    unstable_from: parsed.unstable_from,
                    has_reference_context: parsed.has_reference_context,
                });
            }
        }
        self.parse_uncached(domain, sealed)
    }

    fn update_cache(
        &mut self,
        domain: &RawDomain,
        parsed: &ParsedDomain,
        stable_end: StreamOffset,
        sealed: bool,
    ) {
        let stable_end = stable_end.min(domain.source_end());
        let prefix = domain
            .text_prefix(stable_end)
            .unwrap_or_default()
            .to_owned();
        let spans = parsed
            .projection
            .spans()
            .iter()
            .filter(|span| span.source().end() <= stable_end)
            .map(|span| CachedSpan {
                source: span.source(),
                values: span.values().to_vec(),
            })
            .collect();
        let cached = CachedDomain {
            source_base: domain.source_base(),
            stable_end,
            prefix,
            spans,
            has_reference_context: parsed.has_reference_context,
        };
        if let Some(existing) = self
            .caches
            .iter_mut()
            .find(|cache| cache.source_base == cached.source_base)
        {
            *existing = cached;
        } else {
            self.caches.push(cached);
        }
        if parsed.has_reference_context || parsed.unstable_from.is_some() {
            self.required_restart_from = Some(
                self.required_restart_from
                    .unwrap_or(domain.source_base())
                    .min(domain.source_base()),
            );
        }
        let _ = sealed;
    }
}

fn cached_projection(
    cache: &CachedDomain,
) -> Result<Projection<TextContent>, MarkdownProjectionError> {
    let mut builder =
        ProjectionBuilder::new(cache.source_base, cache.stable_end, cache.stable_end, true);
    for span in &cache.spans {
        builder = builder.emit_many(span.source, span.values.clone());
    }
    builder
        .finish()
        .map_err(TextProjectionError::from)
        .map_err(Into::into)
}

fn prepend_cached(
    cache: &CachedDomain,
    suffix: Projection<TextContent>,
) -> Result<Projection<TextContent>, MarkdownProjectionError> {
    let mut builder = ProjectionBuilder::new(
        cache.source_base,
        suffix.stable_through(),
        suffix.source_end(),
        suffix.is_sealed(),
    );
    let mut cursor = cache.source_base;
    let cached = cache
        .spans
        .iter()
        .map(|span| (span.source, span.values.as_slice()));
    let extra = suffix
        .spans()
        .iter()
        .map(|span| (span.source(), span.values()));
    for (source, values) in cached.chain(extra) {
        if source.end() <= cursor {
            continue;
        }
        let start = source.start().max(cursor);
        if start > cursor {
            builder = builder.elide(StreamRange::new(cursor, start));
        }
        builder = builder.emit_many(
            StreamRange::new(start, source.end()),
            values.iter().cloned(),
        );
        cursor = source.end();
    }
    if cursor < suffix.source_end() {
        builder = builder.elide(StreamRange::new(cursor, suffix.source_end()));
    }
    builder
        .finish()
        .map_err(TextProjectionError::from)
        .map_err(Into::into)
}

fn is_raw_span(span: &ProjectionSpan<TextContent>) -> bool {
    span.values().len() == 1 && matches!(span.values()[0], TextContent::Raw(_))
}

fn snap_stable_to_span_boundary<T>(
    stable: StreamOffset,
    source_base: StreamOffset,
    source_end: StreamOffset,
    spans: &[ProjectionSpan<T>],
) -> StreamOffset {
    let stable = stable.min(source_end).max(source_base);
    if stable == source_base
        || stable == source_end
        || spans.iter().any(|span| span.source.end() == stable)
    {
        return stable;
    }
    if let Some(end) = spans
        .iter()
        .map(|span| span.source.end())
        .filter(|end| *end <= stable)
        .max()
    {
        return end;
    }
    spans
        .iter()
        .find(|span| span.source.start() <= stable && stable < span.source.end())
        .map_or(source_base, |span| span.source.end().min(source_end))
}

fn parse_domain_uncached(
    domain: &RawDomain,
    options: MarkdownOptions,
    sealed: bool,
) -> Result<ParsedDomain, MarkdownProjectionError> {
    let broken = Rc::new(RefCell::new(Vec::<Range<usize>>::new()));
    let broken_for_callback = Rc::clone(&broken);
    let callback = move |link: pulldown_cmark::BrokenLink<'_>| {
        broken_for_callback.borrow_mut().push(link.span);
        None
    };
    let parser =
        Parser::new_with_broken_link_callback(domain.text(), options.pulldown(), Some(callback));
    let has_reference_context = parser.reference_definitions().iter().next().is_some();
    let mut builder = Builder::new(domain, sealed, options.live_table_stabilization());
    for (event, range) in parser.into_offset_iter() {
        builder.event(event, range)?;
    }
    let projection = builder.finish()?;
    let unstable_from = broken
        .borrow()
        .iter()
        .map(|range| {
            let root_start = domain.source_base().saturating_add(range.start as u64);
            projection
                .spans()
                .iter()
                .find(|span| !span.values().is_empty() && span.source().contains_offset(root_start))
                .map_or(domain.source_base(), |span| span.source().start())
        })
        .min();
    Ok(ParsedDomain {
        projection,
        unstable_from,
        has_reference_context,
    })
}

#[derive(Debug)]
struct Builder<'a> {
    domain: &'a RawDomain,
    sealed: bool,
    stabilize_live_tables: bool,
    frames: Vec<Frame>,
    root: Vec<OwnedBlock>,
    marks: Vec<Mark>,
}

#[derive(Debug)]
enum Frame {
    Paragraph {
        source: Range<usize>,
        heading: Option<HeadingLevel>,
        content: Vec<Inline>,
    },
    BlockQuote {
        source: Range<usize>,
        blocks: Vec<Block>,
    },
    List {
        source: Range<usize>,
        marker: ListMarker,
        tight: bool,
        items: Vec<(Range<usize>, ListItem)>,
    },
    Item {
        source: Range<usize>,
        blocks: Vec<Block>,
        inline: Vec<Inline>,
        checked: Option<bool>,
    },
    Code {
        source: Range<usize>,
        info: Option<String>,
        body: Vec<(String, Range<usize>)>,
    },
    Html {
        source: Range<usize>,
        body: Vec<(String, Range<usize>)>,
    },
    Table {
        source: Range<usize>,
        columns: Vec<TableColumn>,
        header_rows: usize,
        rows: Vec<TableRow>,
    },
    Row {
        source: Range<usize>,
        cells: Vec<TableCell>,
    },
    Cell {
        source: Range<usize>,
        content: Vec<Inline>,
    },
    Image {
        source: Range<usize>,
        destination: String,
        title: Option<String>,
        alt: Vec<Inline>,
    },
    Root,
}

#[derive(Debug)]
struct OwnedBlock {
    range: Range<usize>,
    block: Block,
}

impl<'a> Builder<'a> {
    fn new(domain: &'a RawDomain, sealed: bool, stabilize_live_tables: bool) -> Self {
        Self {
            domain,
            sealed,
            stabilize_live_tables,
            frames: vec![Frame::Root],
            root: Vec::new(),
            marks: Vec::new(),
        }
    }

    fn event(
        &mut self,
        event: Event<'_>,
        range: Range<usize>,
    ) -> Result<(), MarkdownProjectionError> {
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(end) => self.end(end),
            Event::Text(text) => self.text(text.as_ref(), range),
            Event::Code(text) => self.inline_text(text.as_ref(), range, true),
            Event::InlineHtml(text) => self.inline_raw(text.as_ref(), range),
            Event::Html(text) => self.html(text.as_ref(), range),
            Event::SoftBreak => self.push_inline(Inline::break_(BreakKind::Soft)),
            Event::HardBreak => self.push_inline(Inline::break_(BreakKind::Hard)),
            Event::Rule => self.add_block(range, Block::thematic_break()),
            Event::TaskListMarker(checked) => {
                let Some(Frame::Item { checked: slot, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "task marker",
                    });
                };
                *slot = Some(checked);
                Ok(())
            }
            Event::InlineMath(_) | Event::DisplayMath(_) | Event::FootnoteReference(_) => {
                Err(MarkdownProjectionError::ParserInvariant {
                    context: "disabled parser event",
                })
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>) -> Result<(), MarkdownProjectionError> {
        match tag {
            Tag::Paragraph => {
                if self
                    .frames
                    .last()
                    .is_some_and(|frame| matches!(frame, Frame::Item { .. }))
                {
                    if let Some(Frame::List { tight, .. }) = self
                        .frames
                        .iter_mut()
                        .rev()
                        .find(|frame| matches!(frame, Frame::List { .. }))
                    {
                        *tight = false;
                    }
                }
                self.frames.push(Frame::Paragraph {
                    source: range,
                    heading: None,
                    content: Vec::new(),
                });
            }
            Tag::Heading { level, .. } => self.frames.push(Frame::Paragraph {
                source: range,
                heading: Some(HeadingLevel::new(level as u8)?),
                content: Vec::new(),
            }),
            Tag::BlockQuote(_) => self.frames.push(Frame::BlockQuote {
                source: range,
                blocks: Vec::new(),
            }),
            Tag::List(start) => {
                let marker = list_marker(start, self.domain, range.start)?;
                self.frames.push(Frame::List {
                    source: range,
                    marker,
                    tight: true,
                    items: Vec::new(),
                });
            }
            Tag::Item => self.frames.push(Frame::Item {
                source: range,
                blocks: Vec::new(),
                inline: Vec::new(),
                checked: None,
            }),
            Tag::CodeBlock(kind) => self.frames.push(Frame::Code {
                source: range,
                info: match kind {
                    CodeBlockKind::Fenced(info) if !info.trim().is_empty() => {
                        Some(info.to_string())
                    }
                    _ => None,
                },
                body: Vec::new(),
            }),
            Tag::HtmlBlock => self.frames.push(Frame::Html {
                source: range,
                body: Vec::new(),
            }),
            Tag::Table(columns) => self.frames.push(Frame::Table {
                source: range,
                columns: columns
                    .into_iter()
                    .map(convert_alignment)
                    .map(TableColumn::new)
                    .collect(),
                header_rows: 0,
                rows: Vec::new(),
            }),
            Tag::TableHead => {
                let Some(Frame::Table { header_rows, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "table head",
                    });
                };
                *header_rows += 1;
                self.frames.push(Frame::Row {
                    source: range,
                    cells: Vec::new(),
                });
            }
            Tag::TableRow => self.frames.push(Frame::Row {
                source: range,
                cells: Vec::new(),
            }),
            Tag::TableCell => self.frames.push(Frame::Cell {
                source: range,
                content: Vec::new(),
            }),
            Tag::Emphasis => self.marks.push(Mark::Emphasis),
            Tag::Strong => self.marks.push(Mark::Strong),
            Tag::Strikethrough => self.marks.push(Mark::Strikethrough),
            Tag::Link {
                dest_url, title, ..
            } => self.marks.push(Mark::Link(LinkTarget::new(
                dest_url.to_string(),
                (!title.is_empty()).then(|| title.to_string()),
            ))),
            Tag::Image {
                dest_url, title, ..
            } => self.frames.push(Frame::Image {
                source: range,
                destination: dest_url.to_string(),
                title: (!title.is_empty()).then(|| title.to_string()),
                alt: Vec::new(),
            }),
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_)
            | Tag::Superscript
            | Tag::Subscript => {
                return Err(MarkdownProjectionError::ParserInvariant {
                    context: "disabled parser tag",
                });
            }
        }
        Ok(())
    }

    fn end(&mut self, end: TagEnd) -> Result<(), MarkdownProjectionError> {
        match end {
            TagEnd::Paragraph | TagEnd::Heading(_) => {
                let Frame::Paragraph {
                    source,
                    heading,
                    content,
                } = self.pop_frame()?
                else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "paragraph",
                    });
                };
                let block = match heading {
                    Some(level) => Block::heading(level, InlineContent::new(content)),
                    None => Block::paragraph(InlineContent::new(content)),
                };
                self.add_block(source, block)
            }
            TagEnd::BlockQuote(_) => {
                let Frame::BlockQuote { source, blocks } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "blockquote",
                    });
                };
                self.add_block(source, Block::block_quote(blocks))
            }
            TagEnd::List(_) => {
                let Frame::List {
                    source,
                    marker,
                    tight,
                    items,
                } = self.pop_frame()?
                else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "list" });
                };
                // Root lists are one Block per item so a closed item can leave
                // the last-span (unstable) position while later items grow.
                if matches!(self.frames.last(), Some(Frame::Root)) {
                    for (index, (item_source, item)) in items.into_iter().enumerate() {
                        self.add_block(
                            item_source,
                            Block::list(List::new(marker_for_item(marker, index), tight, [item])),
                        )?;
                    }
                    return Ok(());
                }
                self.add_block(
                    source,
                    Block::list(List::new(
                        marker,
                        tight,
                        items.into_iter().map(|(_, item)| item),
                    )),
                )
            }
            TagEnd::Item => {
                let Frame::Item {
                    source,
                    mut blocks,
                    inline,
                    checked,
                } = self.pop_frame()?
                else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "item" });
                };
                if !inline.is_empty() {
                    blocks.insert(0, Block::paragraph(InlineContent::new(inline)));
                }
                let item = ListItem::new(blocks).with_checked(checked);
                let Some(Frame::List { items, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "item parent",
                    });
                };
                items.push((source, item));
                Ok(())
            }
            TagEnd::CodeBlock => {
                let Frame::Code { source, info, body } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "code block",
                    });
                };
                let mut runs = Vec::new();
                for (text, local) in body {
                    runs.extend(self.text_runs(text, local)?);
                }
                let language = info
                    .as_deref()
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| LanguageId::new(value).ok());
                self.add_block(
                    source,
                    Block::code(CodeBlock::new(language, info, LiteralText::new(runs))),
                )
            }
            TagEnd::HtmlBlock => {
                let Frame::Html { source, body } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "html block",
                    });
                };
                let mut runs = Vec::new();
                for (text, local) in body {
                    runs.extend(self.text_runs(text, local)?);
                }
                self.add_block(
                    source,
                    Block::raw(FormatId::new("html")?, LiteralText::new(runs)),
                )
            }
            TagEnd::TableCell => {
                let Frame::Cell { source, content } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "cell" });
                };
                let Some(Frame::Row { cells, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "cell parent",
                    });
                };
                cells.push(TableCell::plain([Block::paragraph(InlineContent::new(
                    content,
                ))]));
                let _ = source;
                Ok(())
            }
            TagEnd::TableRow => {
                let Frame::Row { source, cells } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "row" });
                };
                let Some(Frame::Table { rows, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "row parent",
                    });
                };
                rows.push(TableRow::new(cells));
                let _ = source;
                Ok(())
            }
            TagEnd::TableHead => {
                let Frame::Row { source, cells } = self.pop_frame()? else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "table head row",
                    });
                };
                let Some(Frame::Table { rows, .. }) = self.frames.last_mut() else {
                    return Err(MarkdownProjectionError::InvalidNesting {
                        context: "table head parent",
                    });
                };
                rows.push(TableRow::new(cells));
                let _ = source;
                Ok(())
            }
            TagEnd::Table => {
                let Frame::Table {
                    source,
                    columns,
                    header_rows,
                    rows,
                } = self.pop_frame()?
                else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "table" });
                };
                if !self.table_is_closed(&source) {
                    let block = self.raw_table_paragraph(source.clone())?;
                    return self.add_block(source, block);
                }
                // GFM body rows may be ragged (spec Example 204): pad short
                // rows and drop extra cells before the generic Table, which
                // requires a rectangular column schema. Do not relax Table.
                let rows = normalize_gfm_table_rows(columns.len(), rows);
                let table = Table::new(None::<Vec<Block>>, columns, header_rows, rows)?;
                self.add_block(source, Block::table(table))
            }
            TagEnd::Emphasis => self.pop_mark(Mark::Emphasis),
            TagEnd::Strong => self.pop_mark(Mark::Strong),
            TagEnd::Strikethrough => self.pop_mark(Mark::Strikethrough),
            TagEnd::Link => self.pop_link(),
            TagEnd::Image => {
                let Frame::Image {
                    source,
                    destination,
                    title,
                    alt,
                } = self.pop_frame()?
                else {
                    return Err(MarkdownProjectionError::InvalidNesting { context: "image" });
                };
                let mut image =
                    Inline::image(Image::new(destination, title, InlineContent::new(alt)));
                if !self.marks.is_empty() {
                    image = image.with_marks(MarkSet::new(self.marks.clone())?);
                }
                self.push_inline(image)?;
                let _ = source;
                Ok(())
            }
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_)
            | TagEnd::Superscript
            | TagEnd::Subscript => Err(MarkdownProjectionError::ParserInvariant {
                context: "unsupported end tag",
            }),
        }
    }

    fn text(&mut self, text: &str, range: Range<usize>) -> Result<(), MarkdownProjectionError> {
        if let Some(Frame::Code { body, .. }) = self.frames.last_mut() {
            body.push((text.to_owned(), range));
            return Ok(());
        }
        if let Some(Frame::Html { body, .. }) = self.frames.last_mut() {
            body.push((text.to_owned(), range));
            return Ok(());
        }
        self.inline_text(text, range, false)
    }

    fn inline_text(
        &mut self,
        text: &str,
        range: Range<usize>,
        code: bool,
    ) -> Result<(), MarkdownProjectionError> {
        let mut active = self.marks.clone();
        if code {
            active.push(Mark::Code);
        }
        let marks = MarkSet::new(active)?;
        let runs = if self.domain.source_slice(range.clone())? == text {
            self.domain.exact_runs(range)?
        } else {
            vec![self.domain.derived_run(Arc::<str>::from(text), range)?]
        };
        for run in runs {
            self.push_inline(Inline::from_parts(
                InlineKind::Text(run),
                marks.clone(),
                Default::default(),
            ))?;
        }
        Ok(())
    }

    fn inline_raw(
        &mut self,
        text: &str,
        range: Range<usize>,
    ) -> Result<(), MarkdownProjectionError> {
        let mut inline = Inline::raw(
            FormatId::new("html")?,
            LiteralText::new(self.text_runs(text.to_owned(), range)?),
        );
        if !self.marks.is_empty() {
            inline = inline.with_marks(MarkSet::new(self.marks.clone())?);
        }
        self.push_inline(inline)
    }

    fn html(&mut self, text: &str, range: Range<usize>) -> Result<(), MarkdownProjectionError> {
        let Some(Frame::Html { body, .. }) = self.frames.last_mut() else {
            return Err(MarkdownProjectionError::InvalidNesting {
                context: "HTML event",
            });
        };
        body.push((text.to_owned(), range));
        Ok(())
    }

    fn push_inline(&mut self, inline: Inline) -> Result<(), MarkdownProjectionError> {
        match self.frames.last_mut() {
            Some(Frame::Image { alt, .. }) => alt.push(inline),
            Some(Frame::Cell { content, .. }) => content.push(inline),
            Some(Frame::Paragraph { content, .. }) => content.push(inline),
            Some(Frame::Item {
                inline: content, ..
            }) => content.push(inline),
            _ => {
                return Err(MarkdownProjectionError::InvalidNesting {
                    context: "inline parent",
                });
            }
        }
        Ok(())
    }

    fn add_block(
        &mut self,
        range: Range<usize>,
        block: Block,
    ) -> Result<(), MarkdownProjectionError> {
        match self.frames.last_mut() {
            Some(Frame::BlockQuote { blocks, .. }) => blocks.push(block),
            Some(Frame::Item { blocks, .. }) => blocks.push(block),
            Some(Frame::Root) => self.root.push(OwnedBlock { range, block }),
            _ => {
                return Err(MarkdownProjectionError::InvalidNesting {
                    context: "block parent",
                });
            }
        }
        Ok(())
    }

    fn pop_frame(&mut self) -> Result<Frame, MarkdownProjectionError> {
        self.frames
            .pop()
            .filter(|frame| !matches!(frame, Frame::Root))
            .ok_or(MarkdownProjectionError::InvalidNesting {
                context: "empty frame stack",
            })
    }

    fn pop_mark(&mut self, expected: Mark) -> Result<(), MarkdownProjectionError> {
        (self.marks.pop() == Some(expected)).then_some(()).ok_or(
            MarkdownProjectionError::InvalidNesting {
                context: "mark stack",
            },
        )
    }

    fn pop_link(&mut self) -> Result<(), MarkdownProjectionError> {
        self.marks
            .pop()
            .filter(|mark| matches!(mark, Mark::Link(_)))
            .map(|_| ())
            .ok_or(MarkdownProjectionError::InvalidNesting {
                context: "link stack",
            })
    }

    fn text_runs(
        &self,
        text: String,
        range: Range<usize>,
    ) -> Result<Vec<TextRun>, MarkdownProjectionError> {
        if self.domain.source_slice(range.clone())? == text {
            Ok(self.domain.exact_runs(range)?)
        } else {
            Ok(vec![
                self.domain.derived_run(Arc::<str>::from(text), range)?,
            ])
        }
    }

    fn table_is_closed(&self, source: &Range<usize>) -> bool {
        // Sealed input is final: emit whatever GFM structure pulldown produced.
        if self.sealed {
            return true;
        }
        // Generic GFM follows pulldown. A pipe-less line can still be a short
        // body row (spec Example 202). The live table stabilizer is a separate
        // product heuristic, enabled only via MarkdownOptions.
        if !self.stabilize_live_tables {
            return true;
        }
        let Some(table_source) = self.domain.text().get(source.start..source.end) else {
            return false;
        };
        let after = self.domain.text().get(source.end..).unwrap_or("");
        following_line_closes_table(table_source, after)
    }

    fn raw_table_paragraph(&self, source: Range<usize>) -> Result<Block, MarkdownProjectionError> {
        let text = self.domain.source_slice(source.clone())?;
        let content_end = if text.ends_with('\n') {
            source.end.saturating_sub(1)
        } else {
            source.end
        };
        let mut inlines = Vec::new();
        let mut segment_start = source.start;
        for (offset, character) in self.domain.text()[source.start..content_end].char_indices() {
            if character != '\n' {
                continue;
            }
            let abs = source.start + offset;
            if segment_start < abs {
                for run in self.domain.exact_runs(segment_start..abs)? {
                    inlines.push(Inline::text(run));
                }
            }
            inlines.push(Inline::break_(BreakKind::Hard));
            segment_start = abs + character.len_utf8();
        }
        if segment_start < content_end {
            for run in self.domain.exact_runs(segment_start..content_end)? {
                inlines.push(Inline::text(run));
            }
        }
        Ok(Block::paragraph(InlineContent::new(inlines)))
    }

    fn finish(self) -> Result<Projection<TextContent>, MarkdownProjectionError> {
        if self.frames.len() != 1 || !self.marks.is_empty() {
            return Err(MarkdownProjectionError::InvalidNesting {
                context: "unclosed parser frame",
            });
        }
        let mut builder = ProjectionBuilder::new(
            self.domain.source_base(),
            self.domain.source_end(),
            self.domain.source_end(),
            true,
        );
        let mut cursor = 0;
        for owned in self.root {
            let end = owned.range.end.max(owned.range.start);
            let start = owned.range.start.max(cursor);
            if start > cursor {
                builder = builder.elide(StreamRange::new(
                    self.domain.source_base().saturating_add(cursor as u64),
                    self.domain.source_base().saturating_add(start as u64),
                ));
            }
            if start >= end {
                continue;
            }
            builder = builder.emit(
                StreamRange::new(
                    self.domain.source_base().saturating_add(start as u64),
                    self.domain.source_base().saturating_add(end as u64),
                ),
                TextContent::Block(stamp_block_origin(owned.block, &TextOrigin::MARKDOWN)),
            );
            cursor = end;
        }
        if cursor < self.domain.len() {
            builder = builder.elide(StreamRange::new(
                self.domain.source_base().saturating_add(cursor as u64),
                self.domain.source_end(),
            ));
        }
        builder
            .finish()
            .map_err(TextProjectionError::from)
            .map_err(Into::into)
    }
}

fn has_open_reference_definition_prefix(text: &str) -> bool {
    if text.ends_with('\n') {
        return false;
    }
    let line = text.rsplit('\n').next().unwrap_or(text).trim_start();
    line.starts_with('[')
}

// A GFM table is still open while the following line could be another row.
// Caching it as stable splits the next row into a paragraph; a later one-shot
// re-parse of the suffix merges them and breaks compaction identity.
fn open_block_start(
    spans: &[ProjectionSpan<TextContent>],
    domain_base: StreamOffset,
) -> StreamOffset {
    let mut last_nonempty = None;
    for span in spans {
        if !span.values().is_empty() {
            last_nonempty = Some(span);
        }
    }
    let Some(last) = last_nonempty else {
        return domain_base;
    };
    let mut open_start = last.source().start();
    if !span_can_extend_table(last) {
        return open_start;
    }
    for span in spans.iter().rev() {
        if span.values().is_empty() {
            continue;
        }
        if span.source().end() > last.source().start() {
            continue;
        }
        if span_can_extend_table(span) {
            open_start = span.source().start();
            continue;
        }
        break;
    }
    open_start
}

fn span_can_extend_table(span: &ProjectionSpan<TextContent>) -> bool {
    span.values().iter().any(|value| match value {
        TextContent::Block(block) => is_pipe_paragraph(block),
        TextContent::Raw(_) => false,
    })
}

/// GFM spec Example 204: missing body cells are empty; extra body cells are dropped.
/// Header/delimiter column count is already `schema_width` from pulldown.
fn normalize_gfm_table_rows(schema_width: usize, rows: Vec<TableRow>) -> Vec<TableRow> {
    rows.into_iter()
        .map(|row| {
            let annotations = row.annotations().clone();
            let mut cells: Vec<_> = row.cells().iter().cloned().collect();
            cells.truncate(schema_width);
            while cells.len() < schema_width {
                cells.push(empty_gfm_table_cell());
            }
            TableRow::new(cells).with_annotations(annotations)
        })
        .collect()
}

fn empty_gfm_table_cell() -> TableCell {
    TableCell::plain([Block::paragraph(InlineContent::new(Vec::new()))])
}

/// Live-table closer, not GFM.
///
/// GFM keeps a table open across a pipe-less body line. This heuristic closes
/// once the next line is blank or does not start with `|`, so streaming text
/// tables stay raw until they can align once.
fn following_line_closes_table(table_source: &str, after: &str) -> bool {
    let next_line = if table_source.ends_with('\n') {
        after
    } else if let Some(rest) = after.strip_prefix('\n') {
        rest
    } else {
        return false;
    };
    if next_line.is_empty() {
        return false;
    }
    if next_line.starts_with('\n') {
        return true;
    }
    let line = next_line.lines().next().unwrap_or(next_line);
    !line.trim_start().starts_with('|')
}

fn is_pipe_paragraph(block: &Block) -> bool {
    let BlockKind::Paragraph(content) = block.kind() else {
        return false;
    };
    let mut text = String::new();
    for inline in content.iter() {
        match inline.kind() {
            InlineKind::Text(run) => text.push_str(run.text()),
            InlineKind::Break(_) => text.push('\n'),
            _ => return false,
        }
    }
    text.lines().any(|line| line.trim_start().starts_with('|'))
}

fn convert_alignment(alignment: PdAlignment) -> Alignment {
    match alignment {
        PdAlignment::None => Alignment::Default,
        PdAlignment::Left => Alignment::Start,
        PdAlignment::Center => Alignment::Center,
        PdAlignment::Right => Alignment::End,
    }
}

fn marker_for_item(marker: ListMarker, index: usize) -> ListMarker {
    match marker {
        ListMarker::Bullet => ListMarker::Bullet,
        ListMarker::Ordered {
            start,
            style,
            delimiter,
        } => ListMarker::Ordered {
            start: start.saturating_add(index as u64),
            style,
            delimiter,
        },
    }
}

fn list_marker(
    start: Option<u64>,
    domain: &RawDomain,
    offset: usize,
) -> Result<ListMarker, MarkdownProjectionError> {
    let Some(start) = start else {
        return Ok(ListMarker::Bullet);
    };
    let source = domain
        .text()
        .get(offset..)
        .ok_or(MarkdownProjectionError::InvalidSourceMap {
            context: "ordered list marker",
        })?;
    let delimiter = source
        .lines()
        .next()
        .and_then(|line| {
            line.find(['.', ')'])
                .and_then(|index| line.as_bytes().get(index))
        })
        .map_or(NumberDelimiter::Period, |character| {
            if *character == b')' {
                NumberDelimiter::Paren
            } else {
                NumberDelimiter::Period
            }
        });
    Ok(ListMarker::Ordered {
        start,
        style: NumberStyle::Decimal,
        delimiter,
    })
}

#[allow(dead_code)]
fn _pulldown_options(options: MarkdownOptions) -> Options {
    options.pulldown()
}
