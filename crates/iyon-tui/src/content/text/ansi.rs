//! Safe ANSI-to-semantic-text projection.
//!
//! ANSI is interpreted as content syntax. Supported SGR and OSC 8 sequences
//! become host-independent style/link intent on semantic text runs; control
//! sequences that could move the terminal cursor, change the window, or write
//! directly to the terminal are consumed and never reach the backend.

use std::ops::Range;

use crate::{
    AnsiColor, ColorSpec, StyleRef, StyleSpec, TextAttribute,
    projection::{Projection, ProjectionBuilder, ProjectionSpan, Projector},
    stream::{StreamOffset, StreamRange},
};

use super::source::RawDomain;
use super::{
    BreakKind, Inline, InlineContent, LinkTarget, TextContent, TextOrigin, TextProjectionError,
    validate_text_projection,
};

/// Configuration for the generic ANSI projector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnsiOptions {
    /// Interpret OSC 8 hyperlinks as semantic link marks. When false, the
    /// control sequence is still consumed but no link metadata is retained.
    pub hyperlinks: bool,
}

/// ANSI projection errors are the same structural/source errors used by the
/// canonical text projection boundary.
pub type AnsiProjectionError = TextProjectionError;

/// Converts safe ANSI display intent into the canonical text IR.
#[derive(Clone, Debug)]
pub struct AnsiProjector {
    options: AnsiOptions,
    last_base: Option<StreamOffset>,
    last_end: Option<StreamOffset>,
    completed_state: AnsiState,
    completed_inlines: Vec<Inline>,
    completed_end: StreamOffset,
}

impl Default for AnsiProjector {
    fn default() -> Self {
        Self::new(AnsiOptions { hyperlinks: true })
    }
}

impl AnsiProjector {
    #[must_use]
    pub const fn new(options: AnsiOptions) -> Self {
        Self {
            options,
            last_base: None,
            last_end: None,
            completed_state: AnsiState::EMPTY,
            completed_inlines: Vec::new(),
            completed_end: StreamOffset::ZERO,
        }
    }

    #[must_use]
    pub const fn options(&self) -> AnsiOptions {
        self.options
    }
}

impl Projector<TextContent> for AnsiProjector {
    type Output = TextContent;
    type Error = AnsiProjectionError;

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
            self.completed_state = AnsiState::EMPTY;
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
            let block = self.parse_ansi_domain(&domain, input.is_sealed())?;
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

#[derive(Clone, Debug, Default)]
struct AnsiState {
    foreground: Option<ColorSpec>,
    background: Option<ColorSpec>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    reversed: bool,
    strikethrough: bool,
    link: Option<LinkTarget>,
}

impl AnsiState {
    const EMPTY: Self = Self {
        foreground: None,
        background: None,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        reversed: false,
        strikethrough: false,
        link: None,
    };

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn style(&self) -> StyleSpec {
        let mut style = StyleSpec::new();
        if let Some(color) = &self.foreground {
            style.set_foreground(color.clone());
        }
        if let Some(color) = &self.background {
            style.set_background(color.clone());
        }
        style.set_attribute(TextAttribute::Bold, self.bold);
        style.set_attribute(TextAttribute::Dim, self.dim);
        style.set_attribute(TextAttribute::Italic, self.italic);
        style.set_attribute(TextAttribute::Underline, self.underline);
        style.set_attribute(TextAttribute::Reversed, self.reversed);
        style.set_attribute(TextAttribute::Strikethrough, self.strikethrough);
        style
    }
}

impl AnsiProjector {
    fn parse_ansi_domain(
        &mut self,
        domain: &RawDomain,
        is_sealed: bool,
    ) -> Result<super::Block, TextProjectionError> {
        let resume_offset = if self.completed_end > domain.source_base() {
            usize::try_from(
                self.completed_end
                    .as_u64()
                    .saturating_sub(domain.source_base().as_u64()),
            )
            .unwrap_or(0)
        } else {
            0
        };

        if resume_offset > domain.len() {
            self.completed_state = AnsiState::EMPTY;
            self.completed_inlines.clear();
            self.completed_end = domain.source_base();
        }

        let parse_slice =
            if self.completed_end > domain.source_base() && resume_offset < domain.len() {
                domain.suffix(resume_offset)?
            } else if self.completed_end <= domain.source_base() {
                domain.clone()
            } else {
                domain.suffix(domain.len())?
            };

        let text = parse_slice.text().as_bytes();
        let mut state = self.completed_state.clone();
        let mut trailing_inlines = Vec::new();
        let mut segment_start = 0usize;
        let mut cursor = 0usize;

        while cursor < text.len() {
            match text[cursor] {
                b'\r' if text.get(cursor + 1) == Some(&b'\n') => {
                    push_segment(
                        &parse_slice,
                        segment_start..cursor,
                        &state,
                        self.options,
                        &mut trailing_inlines,
                    )?;
                    trailing_inlines.push(Inline::break_(BreakKind::Hard));
                    cursor += 2;
                    segment_start = cursor;
                    self.completed_inlines.append(&mut trailing_inlines);
                    self.completed_end = parse_slice.source_base().saturating_add(cursor as u64);
                    self.completed_state = state.clone();
                }
                b'\n' => {
                    push_segment(
                        &parse_slice,
                        segment_start..cursor,
                        &state,
                        self.options,
                        &mut trailing_inlines,
                    )?;
                    trailing_inlines.push(Inline::break_(BreakKind::Hard));
                    cursor += 1;
                    segment_start = cursor;
                    self.completed_inlines.append(&mut trailing_inlines);
                    self.completed_end = parse_slice.source_base().saturating_add(cursor as u64);
                    self.completed_state = state.clone();
                }
                0x1b => {
                    let mut next_state = state.clone();
                    if let Some(new_cursor) =
                        try_consume_escape(text, cursor + 1, &mut next_state, self.options)
                    {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        state = next_state;
                        cursor = new_cursor;
                        segment_start = cursor;
                        self.completed_inlines.append(&mut trailing_inlines);
                        self.completed_end =
                            parse_slice.source_base().saturating_add(cursor as u64);
                        self.completed_state = state.clone();
                    } else if is_sealed {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        cursor = text.len();
                        segment_start = cursor;
                    } else {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        break;
                    }
                }
                0xc2 if text.get(cursor + 1) == Some(&0x9b) => {
                    let mut next_state = state.clone();
                    if let Some(new_cursor) = try_consume_csi(text, cursor + 2, &mut next_state) {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        state = next_state;
                        cursor = new_cursor;
                        segment_start = cursor;
                        self.completed_inlines.append(&mut trailing_inlines);
                        self.completed_end =
                            parse_slice.source_base().saturating_add(cursor as u64);
                        self.completed_state = state.clone();
                    } else if is_sealed {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        cursor = text.len();
                        segment_start = cursor;
                    } else {
                        push_segment(
                            &parse_slice,
                            segment_start..cursor,
                            &state,
                            self.options,
                            &mut trailing_inlines,
                        )?;
                        break;
                    }
                }
                _ => cursor += 1,
            }
        }

        if segment_start < text.len() && (cursor == text.len() || is_sealed) {
            push_segment(
                &parse_slice,
                segment_start..text.len(),
                &state,
                self.options,
                &mut trailing_inlines,
            )?;
        }

        if is_sealed {
            self.completed_inlines.append(&mut trailing_inlines);
            self.completed_end = domain.source_end();
            self.completed_state = state;
            Ok(
                super::Block::paragraph(InlineContent::new(self.completed_inlines.clone()))
                    .with_origin(TextOrigin::ANSI),
            )
        } else {
            let mut all_inlines = self.completed_inlines.clone();
            all_inlines.extend(trailing_inlines);
            Ok(super::Block::paragraph(InlineContent::new(all_inlines))
                .with_origin(TextOrigin::ANSI))
        }
    }
}

fn push_segment(
    domain: &RawDomain,
    range: Range<usize>,
    state: &AnsiState,
    options: AnsiOptions,
    output: &mut Vec<Inline>,
) -> Result<(), TextProjectionError> {
    if range.start == range.end {
        return Ok(());
    }
    let style = StyleRef::themed(super::style::TEXT_THEME_KEY, state.style());
    for run in domain.exact_runs(range)? {
        let run = run.with_style(style.clone());
        let inline = match (options.hyperlinks, state.link.clone()) {
            (true, Some(link)) => Inline::text(run)
                .with_link(link)
                .map_err(TextProjectionError::Ir)?,
            _ => Inline::text(run),
        };
        output.push(inline);
    }
    Ok(())
}

fn try_consume_escape(
    bytes: &[u8],
    start: usize,
    state: &mut AnsiState,
    options: AnsiOptions,
) -> Option<usize> {
    let &kind = bytes.get(start)?;
    match kind {
        b'[' => try_consume_csi(bytes, start + 1, state),
        b']' => try_consume_osc(bytes, start + 1, state, options),
        // RIS, save/restore cursor, and every other two-byte ESC command are
        // intentionally consumed. None may become terminal output.
        _ => Some(start + 1),
    }
}

fn try_consume_csi(bytes: &[u8], start: usize, state: &mut AnsiState) -> Option<usize> {
    let mut cursor = start;
    while let Some(&byte) = bytes.get(cursor) {
        if (0x40..=0x7e).contains(&byte) {
            if byte == b'm' {
                apply_sgr(&bytes[start..cursor], state);
            }
            return Some(cursor + 1);
        }
        cursor += 1;
    }
    None
}

fn try_consume_osc(
    bytes: &[u8],
    start: usize,
    state: &mut AnsiState,
    options: AnsiOptions,
) -> Option<usize> {
    let mut cursor = start;
    let mut end = None;
    let mut new_cursor = None;
    while cursor < bytes.len() {
        if bytes[cursor] == 0x07 {
            end = Some(cursor);
            new_cursor = Some(cursor + 1);
            break;
        }
        if bytes[cursor] == 0x1b && bytes.get(cursor + 1) == Some(&b'\\') {
            end = Some(cursor);
            new_cursor = Some(cursor + 2);
            break;
        }
        cursor += 1;
    }
    let end = end?;
    let new_cursor = new_cursor?;
    if options.hyperlinks {
        apply_osc(&bytes[start..end], state);
    }
    Some(new_cursor)
}

fn apply_osc(bytes: &[u8], state: &mut AnsiState) {
    let Ok(value) = std::str::from_utf8(bytes) else {
        return;
    };
    let mut fields = value.splitn(3, ';');
    if fields.next() != Some("8") || fields.next().is_none() {
        return;
    }
    let Some(uri) = fields.next() else {
        return;
    };
    state.link = if uri.is_empty() {
        None
    } else {
        Some(LinkTarget::new(uri, None::<&str>))
    };
}

fn apply_sgr(bytes: &[u8], state: &mut AnsiState) {
    let params = if bytes.is_empty() {
        vec![0]
    } else {
        bytes
            .split(|byte| *byte == b';' || *byte == b':')
            .map(|part| {
                if part.is_empty() {
                    0
                } else {
                    std::str::from_utf8(part)
                        .ok()
                        .and_then(|value| value.parse::<u16>().ok())
                        .unwrap_or(u16::MAX)
                }
            })
            .collect()
    };
    let mut index = 0;
    while let Some(&parameter) = params.get(index) {
        match parameter {
            0 => state.reset(),
            1 => state.bold = true,
            2 => state.dim = true,
            3 => state.italic = true,
            4 => state.underline = true,
            7 => state.reversed = true,
            9 => state.strikethrough = true,
            22 => {
                state.bold = false;
                state.dim = false;
            }
            23 => state.italic = false,
            24 => state.underline = false,
            27 => state.reversed = false,
            29 => state.strikethrough = false,
            30..=37 => {
                state.foreground = Some(ColorSpec::named(ansi_color((parameter - 30) as u8, false)))
            }
            39 => state.foreground = None,
            40..=47 => {
                state.background = Some(ColorSpec::named(ansi_color((parameter - 40) as u8, false)))
            }
            49 => state.background = None,
            90..=97 => {
                state.foreground = Some(ColorSpec::named(ansi_color((parameter - 90) as u8, true)))
            }
            100..=107 => {
                state.background = Some(ColorSpec::named(ansi_color((parameter - 100) as u8, true)))
            }
            38 | 48 => {
                let foreground = parameter == 38;
                if let Some((color, consumed)) = extended_color(&params[index + 1..]) {
                    if foreground {
                        state.foreground = Some(color);
                    } else {
                        state.background = Some(color);
                    }
                    index += consumed;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

fn extended_color(params: &[u16]) -> Option<(ColorSpec, usize)> {
    match params.first().copied()? {
        2 if params.len() >= 4 && params[1..4].iter().all(|value| *value <= 255) => Some((
            ColorSpec::rgb(params[1] as u8, params[2] as u8, params[3] as u8),
            4,
        )),
        5 if params.get(1).is_some_and(|value| *value <= 255) => {
            Some((ColorSpec::ansi(params[1] as u8), 2))
        }
        _ => None,
    }
}

fn ansi_color(value: u8, bright: bool) -> AnsiColor {
    const NORMAL: [AnsiColor; 8] = [
        AnsiColor::Black,
        AnsiColor::Red,
        AnsiColor::Green,
        AnsiColor::Yellow,
        AnsiColor::Blue,
        AnsiColor::Magenta,
        AnsiColor::Cyan,
        AnsiColor::Gray,
    ];
    const BRIGHT: [AnsiColor; 8] = [
        AnsiColor::DarkGray,
        AnsiColor::LightRed,
        AnsiColor::LightGreen,
        AnsiColor::LightYellow,
        AnsiColor::LightBlue,
        AnsiColor::LightMagenta,
        AnsiColor::LightCyan,
        AnsiColor::White,
    ];
    if bright {
        BRIGHT[usize::from(value.min(7))]
    } else {
        NORMAL[usize::from(value.min(7))]
    }
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
    fn incremental_ansi_matches_one_shot() {
        let text = "\x1b[1mBold\x1b[0m \x1b[31mRed\x1b[32mGreen\x1b[0m\n\x1b]8;;https://example.com\x07Link\x1b]8;;\x07\n";
        let mut one_shot = AnsiProjector::default();
        let expected = one_shot.project(&raw_projection(text, true)).unwrap();

        let mut incremental = AnsiProjector::default();
        let mut accumulated = String::new();
        let mut last_proj = None;
        for ch in text.chars() {
            accumulated.push(ch);
            let is_final = accumulated.len() == text.len();
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
    fn incomplete_escape_at_boundary_held_and_completed() {
        let mut incremental = AnsiProjector::default();

        // Chunk 1 ends in incomplete CSI "\x1b[3"
        let p1 = incremental
            .project(&raw_projection("Hello \x1b[3", false))
            .unwrap();
        // The incomplete escape is not leaked as text
        let block1 = match &p1.spans()[0].values()[0] {
            TextContent::Block(b) => b,
            _ => panic!("expected block"),
        };
        let crate::text::BlockKind::Paragraph(inline_content1) = block1.kind() else {
            panic!("expected paragraph block");
        };
        let inlines1 = inline_content1.items();
        assert_eq!(inlines1.len(), 1);
        let run1 = match inlines1[0].kind() {
            crate::text::InlineKind::Text(r) => r,
            _ => panic!("expected text run"),
        };
        assert_eq!(run1.text(), "Hello ");

        // Chunk 2 completes the escape with "1mWorld\n"
        let p2 = incremental
            .project(&raw_projection("Hello \x1b[31mWorld\n", true))
            .unwrap();
        let mut one_shot = AnsiProjector::default();
        let expected = one_shot
            .project(&raw_projection("Hello \x1b[31mWorld\n", true))
            .unwrap();

        assert_eq!(p2.spans().len(), expected.spans().len());
        for (a, b) in p2.spans().iter().zip(expected.spans()) {
            assert_eq!(a.source(), b.source());
            assert_eq!(a.values(), b.values());
        }
    }

    #[test]
    fn incomplete_escape_at_stream_seal_is_dropped_safely() {
        let mut projector = AnsiProjector::default();
        // Ends with incomplete escape, but stream is sealed
        let p = projector
            .project(&raw_projection("Safe text\x1b[3", true))
            .unwrap();
        let block = match &p.spans()[0].values()[0] {
            TextContent::Block(b) => b,
            _ => panic!("expected block"),
        };
        let crate::text::BlockKind::Paragraph(inline_content) = block.kind() else {
            panic!("expected paragraph block");
        };
        let inlines = inline_content.items();
        assert_eq!(inlines.len(), 1);
        let run = match inlines[0].kind() {
            crate::text::InlineKind::Text(r) => r,
            _ => panic!("expected text run"),
        };
        assert_eq!(run.text(), "Safe text");
    }

    #[test]
    fn unsafe_control_sequences_never_leak() {
        let mut projector = AnsiProjector::default();
        // Cursor move, clear screen, private modes
        let input = "clean\x1b[2J\x1b[H\x1b[?25ltext\x1b[?1049h\n";
        let p = projector.project(&raw_projection(input, true)).unwrap();
        let block = match &p.spans()[0].values()[0] {
            TextContent::Block(b) => b,
            _ => panic!("expected block"),
        };
        let crate::text::BlockKind::Paragraph(inline_content) = block.kind() else {
            panic!("expected paragraph block");
        };
        let inlines = inline_content.items();
        let mut text_output = String::new();
        for inline in inlines {
            if let crate::text::InlineKind::Text(r) = inline.kind() {
                text_output.push_str(r.text());
            }
        }
        assert_eq!(text_output, "cleantext");
        assert!(!text_output.contains('\x1b'));
    }
}
