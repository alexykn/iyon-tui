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
    stream::StreamRange,
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
#[derive(Clone, Copy, Debug)]
pub struct AnsiProjector {
    options: AnsiOptions,
}

impl Default for AnsiProjector {
    fn default() -> Self {
        Self::new(AnsiOptions { hyperlinks: true })
    }
}

impl AnsiProjector {
    pub const fn new(options: AnsiOptions) -> Self {
        Self { options }
    }

    pub const fn options(self) -> AnsiOptions {
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
            let block = parse_domain(&domain, self.options)?;
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

fn parse_domain(
    domain: &RawDomain,
    options: AnsiOptions,
) -> Result<super::Block, TextProjectionError> {
    let text = domain.text().as_bytes();
    let mut state = AnsiState::default();
    let mut inlines = Vec::new();
    let mut segment_start = 0usize;
    let mut cursor = 0usize;

    while cursor < text.len() {
        match text[cursor] {
            b'\r' if text.get(cursor + 1) == Some(&b'\n') => {
                // CRLF is one semantic hard break. Keep both bytes in the
                // Source range while omitting the carriage-return control
                // from the rendered inline sequence.
                push_segment(domain, segment_start..cursor, &state, options, &mut inlines)?;
                cursor += 1;
                segment_start = cursor;
            }
            b'\n' => {
                push_segment(domain, segment_start..cursor, &state, options, &mut inlines)?;
                inlines.push(Inline::break_(BreakKind::Hard));
                cursor += 1;
                segment_start = cursor;
            }
            0x1b => {
                push_segment(domain, segment_start..cursor, &state, options, &mut inlines)?;
                cursor = consume_escape(text, cursor + 1, &mut state, options);
                segment_start = cursor;
            }
            0x9b => {
                // C1 CSI is a control sequence too. It has no display
                // semantics unless it is an SGR sequence, which is parsed
                // using the same bounded parameter handling below.
                push_segment(domain, segment_start..cursor, &state, options, &mut inlines)?;
                cursor = consume_csi(text, cursor + 1, &mut state);
                segment_start = cursor;
            }
            _ => cursor += 1,
        }
    }
    push_segment(
        domain,
        segment_start..text.len(),
        &state,
        options,
        &mut inlines,
    )?;

    Ok(super::Block::paragraph(InlineContent::new(inlines)).with_origin(TextOrigin::ANSI))
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

fn consume_escape(
    bytes: &[u8],
    start: usize,
    state: &mut AnsiState,
    options: AnsiOptions,
) -> usize {
    let Some(&kind) = bytes.get(start) else {
        return bytes.len();
    };
    match kind {
        b'[' => consume_csi(bytes, start + 1, state),
        b']' => consume_osc(bytes, start + 1, state, options),
        // RIS, save/restore cursor, and every other two-byte ESC command are
        // intentionally consumed. None may become terminal output.
        _ => start.saturating_add(1).min(bytes.len()),
    }
}

fn consume_csi(bytes: &[u8], start: usize, state: &mut AnsiState) -> usize {
    let mut cursor = start;
    while let Some(&byte) = bytes.get(cursor) {
        if (0x40..=0x7e).contains(&byte) {
            if byte == b'm' {
                apply_sgr(&bytes[start..cursor], state);
            }
            return cursor.saturating_add(1);
        }
        cursor += 1;
    }
    bytes.len()
}

fn consume_osc(bytes: &[u8], start: usize, state: &mut AnsiState, options: AnsiOptions) -> usize {
    let mut cursor = start;
    let mut end = bytes.len();
    while cursor < bytes.len() {
        if bytes[cursor] == 0x07 {
            end = cursor;
            cursor += 1;
            break;
        }
        if bytes[cursor] == 0x1b && bytes.get(cursor + 1) == Some(&b'\\') {
            end = cursor;
            cursor += 2;
            break;
        }
        cursor += 1;
    }
    if options.hyperlinks {
        apply_osc(&bytes[start..end], state);
    }
    cursor.min(bytes.len())
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
