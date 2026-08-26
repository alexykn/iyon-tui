use std::borrow::Cow;
use std::ops::Range;

use crate::perf::{self, Counter};
use crate::physical::{PhysicalStyle, grapheme_cell_width};
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation;

use crate::presentation::{
    WidthRule, WrapMode,
    ir::{TextCursorAnchor, TextView},
};

/// An atomic extended-grapheme cluster with style and optional source range.
///
/// `width` is canonical terminal-cell geometry, computed once at tokenization
/// via [`crate::physical::grapheme_cell_width`]. Wrap, measure, paint, and
/// cursor placement must use this stored value — not re-measure the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledGrapheme<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) width: usize,
    pub(crate) style: PhysicalStyle,
    pub(crate) source: Option<Range<usize>>,
}

/// A physical line of wrapped graphemes along with its display width and fit indicator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrappedLine<'a> {
    pub(crate) graphemes: Vec<StyledGrapheme<'a>>,
    pub(crate) width: usize,
    pub(crate) fits: bool,
}

impl<'a> WrappedLine<'a> {
    pub(crate) fn new(graphemes: Vec<StyledGrapheme<'a>>, target_width: usize) -> Self {
        let width = graphemes.iter().map(|g| g.width).sum();
        let fits = if graphemes.is_empty() {
            true
        } else if target_width == 0 {
            false
        } else {
            width <= target_width
        };
        Self {
            graphemes,
            width,
            fits,
        }
    }
}

/// Internal span fragment within a hard line.
#[derive(Debug, Clone)]
struct SpanFragment<'a> {
    slice: &'a str,
    style: PhysicalStyle,
    source_start: Option<usize>,
}

/// Splits styled spans into hard logical lines (at `\n`), tokenizing each hard line
/// as a unified sequence of atomic [`StyledGrapheme`] clusters across span boundaries.
pub(crate) fn styled_hard_lines<'a, I>(spans: I) -> Vec<Vec<StyledGrapheme<'a>>>
where
    I: IntoIterator<Item = (&'a str, PhysicalStyle, Option<usize>)>,
{
    let mut hard_lines_fragments: Vec<Vec<SpanFragment<'a>>> = Vec::new();
    let mut current_fragments: Vec<SpanFragment<'a>> = Vec::new();

    for (span_text, style, source_base) in spans {
        let mut byte_offset = 0usize;
        for (piece_idx, piece) in span_text.split('\n').enumerate() {
            if piece_idx > 0 {
                hard_lines_fragments.push(std::mem::take(&mut current_fragments));
                byte_offset += 1; // '\n'
            }
            if !piece.is_empty() {
                let src_start = source_base.map(|base| base + byte_offset);
                current_fragments.push(SpanFragment {
                    slice: piece,
                    style,
                    source_start: src_start,
                });
            }
            byte_offset += piece.len();
        }
    }
    hard_lines_fragments.push(current_fragments);

    hard_lines_fragments
        .into_iter()
        .map(tokenize_hard_line)
        .collect()
}

fn tokenize_hard_line<'a>(fragments: Vec<SpanFragment<'a>>) -> Vec<StyledGrapheme<'a>> {
    if fragments.is_empty() {
        return Vec::new();
    }

    if fragments.len() == 1 {
        let frag = &fragments[0];
        let mut line = Vec::new();
        for (g_rel, g_text) in frag.slice.grapheme_indices(true) {
            // Width is calculated once per EGC and stored. Later wrap/paint
            // steps must use `StyledGrapheme.width`, not re-measure the text.
            let width = grapheme_cell_width(g_text);
            let src = frag
                .source_start
                .map(|base| (base + g_rel)..(base + g_rel + g_text.len()));
            line.push(StyledGrapheme {
                text: Cow::Borrowed(g_text),
                width,
                style: frag.style,
                source: src,
            });
        }
        return line;
    }

    // Multiple fragments in this hard line. Concatenate and tokenize across boundaries.
    let mut full_text = String::new();
    let mut fragment_ranges: Vec<(Range<usize>, &SpanFragment<'a>)> =
        Vec::with_capacity(fragments.len());

    for frag in &fragments {
        let start = full_text.len();
        full_text.push_str(frag.slice);
        let end = full_text.len();
        fragment_ranges.push((start..end, frag));
    }

    let mut line = Vec::new();
    for (g_start, g_text) in full_text.grapheme_indices(true) {
        let g_end = g_start + g_text.len();
        let (start_range, start_frag) = fragment_ranges
            .iter()
            .find(|(r, _)| r.contains(&g_start))
            .unwrap();
        let (end_range, end_frag) = fragment_ranges
            .iter()
            .find(|(r, _)| r.contains(&(g_end - 1)))
            .unwrap();

        let style = start_frag.style;
        let source = match (start_frag.source_start, end_frag.source_start) {
            (Some(s_base), Some(e_base)) => {
                let s_offset = s_base + (g_start - start_range.start);
                let e_offset = e_base + (g_end - end_range.start);
                Some(s_offset..e_offset)
            }
            _ => None,
        };

        let text = if std::ptr::eq(*start_frag, *end_frag) {
            let rel_start = g_start - start_range.start;
            let rel_end = g_end - start_range.start;
            Cow::Borrowed(&start_frag.slice[rel_start..rel_end])
        } else {
            Cow::Owned(g_text.to_string())
        };

        let width = grapheme_cell_width(g_text);
        line.push(StyledGrapheme {
            text,
            width,
            style,
            source,
        });
    }

    line
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledTextFlow<'a> {
    pub(crate) width: u16,
    pub(crate) rows: Vec<WrappedLine<'a>>,
    pub(crate) cursor: Option<(usize, usize)>,
}

pub(crate) fn text_flow<'a>(
    text: &TextView,
    hard_lines: Vec<Vec<StyledGrapheme<'a>>>,
    source: Option<&str>,
    max_width: u16,
    inherited_width: WidthRule,
) -> StyledTextFlow<'a> {
    let intrinsic_width = hard_lines
        .iter()
        .map(|line| line.iter().map(|grapheme| grapheme.width).sum::<usize>())
        .max()
        .unwrap_or(0);
    let cursor_needs_cell = text.cursor.is_some_and(|anchor| {
        !hard_lines.iter().flatten().any(|grapheme| {
            grapheme
                .source
                .as_ref()
                .is_some_and(|range| range.start == anchor.byte_offset)
        })
    });
    let intrinsic_width = intrinsic_width + usize::from(cursor_needs_cell);
    let width = match inherited_width {
        WidthRule::Fit => intrinsic_width.min(usize::from(max_width)) as u16,
        WidthRule::Fill => max_width,
    };
    let mut rows = if source.is_some() && text.cursor.is_some() && text.wrap != WrapMode::NoWrap {
        wrap_input_styled_lines(&hard_lines, width)
    } else {
        wrap_styled_lines(&hard_lines, width, text.wrap)
    };
    let cursor = text.cursor.and_then(|anchor| {
        source.map(|source| {
            assert!(
                anchor.byte_offset <= source.len(),
                "text cursor anchor exceeds source length"
            );
            assert!(
                source.is_char_boundary(anchor.byte_offset),
                "text cursor anchor is not a UTF-8 boundary"
            );
            cursor_position(source, anchor, usize::from(width), &mut rows)
        })
    });
    StyledTextFlow {
        width,
        rows,
        cursor,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextFlowMetrics {
    pub(crate) width: u16,
    pub(crate) row_count: u16,
    pub(crate) fits: bool,
}

pub(crate) fn text_flow_metrics(text: &TextView, width: u16) -> TextFlowMetrics {
    perf::inc(Counter::TextFlowMeasureCalls);
    let mut source_offset = 0usize;
    let hard_lines = styled_hard_lines(text.spans.iter().map(|span| {
        let base = Some(source_offset);
        source_offset += span.text().len();
        (span.text(), PhysicalStyle::default(), base)
    }));
    let source = text
        .spans
        .iter()
        .map(|span| span.text())
        .collect::<String>();
    let flow = text_flow(
        text,
        hard_lines,
        text.cursor.map(|_| source.as_str()),
        width,
        WidthRule::Fit,
    );
    TextFlowMetrics {
        width: flow.width,
        row_count: flow.rows.len().max(1) as u16,
        fits: flow.rows.iter().all(|row| row.fits),
    }
}

/// Generic grapheme-aware line-wrapping kernel.
///
/// Wraps pre-split hard lines of [`StyledGrapheme`]s to fit within `width` cells.
/// Extended grapheme clusters are never split internally.
pub(crate) fn wrap_styled_lines<'a>(
    hard_lines: &[Vec<StyledGrapheme<'a>>],
    width: u16,
    mode: WrapMode,
) -> Vec<WrappedLine<'a>> {
    let width = usize::from(width);
    let mut output = Vec::new();

    for line in hard_lines {
        if mode == WrapMode::NoWrap || width == 0 {
            output.push(WrappedLine::new(line.clone(), width));
            continue;
        }

        if line.is_empty() {
            output.push(WrappedLine::new(Vec::new(), width));
            continue;
        }

        if mode == WrapMode::Grapheme {
            for row in wrap_graphemes_exact(line, width) {
                output.push(WrappedLine::new(row, width));
            }
            continue;
        }

        // WrapMode::WordThenGrapheme
        for row in wrap_line_word_then_grapheme(line, width) {
            output.push(WrappedLine::new(row, width));
        }
    }

    output
}

/// Fallback hard breaking between extended grapheme clusters.
fn wrap_graphemes_exact<'a>(
    line: &[StyledGrapheme<'a>],
    width: usize,
) -> Vec<Vec<StyledGrapheme<'a>>> {
    let mut output = Vec::new();
    let mut current = Vec::new();
    let mut used = 0usize;

    for grapheme in line {
        if used > 0 && used.saturating_add(grapheme.width) > width {
            output.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(grapheme.clone());
        used = used.saturating_add(grapheme.width);
    }

    if !current.is_empty() {
        output.push(current);
    }

    output
}

/// Word-wrapping with UAX #14 break opportunities, falling back to grapheme-level
/// hard breaks when words exceed the available width.
fn wrap_line_word_then_grapheme<'a>(
    line: &[StyledGrapheme<'a>],
    width: usize,
) -> Vec<Vec<StyledGrapheme<'a>>> {
    if line.is_empty() {
        return vec![Vec::new()];
    }

    let mut full_text = String::new();
    let mut grapheme_byte_ends: Vec<usize> = Vec::with_capacity(line.len());

    for g in line {
        full_text.push_str(g.text.as_ref());
        grapheme_byte_ends.push(full_text.len());
    }

    let mut can_break_after = vec![false; line.len()];
    for (byte_offset, opportunity) in linebreaks(&full_text) {
        if opportunity == BreakOpportunity::Allowed || opportunity == BreakOpportunity::Mandatory {
            if let Ok(idx) = grapheme_byte_ends.binary_search(&byte_offset) {
                if idx < line.len() {
                    can_break_after[idx] = true;
                }
            }
        }
    }

    let mut output = Vec::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        let line_start = cursor;
        let mut used_width = 0usize;
        let mut last_legal_break: Option<usize> = None;

        while cursor < line.len() {
            let g = &line[cursor];

            if used_width + g.width <= width {
                used_width += g.width;
                if can_break_after[cursor] {
                    last_legal_break = Some(cursor + 1);
                }
                cursor += 1;
            } else {
                break;
            }
        }

        if cursor == line.len() {
            output.push(line[line_start..cursor].to_vec());
            break;
        }

        if let Some(break_at) = last_legal_break
            && break_at > line_start
        {
            output.push(line[line_start..break_at].to_vec());
            cursor = break_at;
        } else if cursor > line_start {
            output.push(line[line_start..cursor].to_vec());
        } else {
            // A single grapheme is wider than the available width (e.g. width=1, CJK/emoji width=2).
            // Do not split the grapheme cluster. Emit it on its own row.
            output.push(vec![line[cursor].clone()]);
            cursor += 1;
        }
    }

    output
}

/// Composer wrap: same [`wrap_styled_lines`] kernel as output, minus one
/// column reserved for the caret.
///
/// A wrap that would start the next row with a single space/tab instead uses
/// that reserved column so the composer does not grow a whitespace-only row.
pub(crate) fn wrap_input_styled_lines<'a>(
    hard_lines: &[Vec<StyledGrapheme<'a>>],
    width: u16,
) -> Vec<WrappedLine<'a>> {
    let wrap_width = width.saturating_sub(1).max(1);
    let mut rows = Vec::new();
    for line in hard_lines {
        let mut wrapped = wrap_styled_lines(
            std::slice::from_ref(line),
            wrap_width,
            WrapMode::WordThenGrapheme,
        );
        // Only merge overflow space inside one hard line. Indent after `\n`
        // must stay on that logical line.
        attach_caret_column_space(&mut wrapped, usize::from(wrap_width));
        rows.extend(wrapped);
    }
    rows
}

fn attach_caret_column_space<'a>(rows: &mut Vec<WrappedLine<'a>>, wrap_width: usize) {
    let mut index = 1;
    while index < rows.len() {
        if rows[index - 1].graphemes.is_empty() {
            index += 1;
            continue;
        }
        let overflow = rows[index]
            .graphemes
            .first()
            .is_some_and(|grapheme| matches!(grapheme.text.as_ref(), " " | "\t"));
        if !overflow {
            index += 1;
            continue;
        }
        let space = rows[index].graphemes.remove(0);
        rows[index - 1].graphemes.push(space);
        rows[index - 1] =
            WrappedLine::new(std::mem::take(&mut rows[index - 1].graphemes), wrap_width);
        if rows[index].graphemes.is_empty() {
            rows.remove(index);
            continue;
        }
        rows[index] = WrappedLine::new(std::mem::take(&mut rows[index].graphemes), wrap_width);
        index += 1;
    }
}

/// Byte ranges of composer visual rows, derived from wrapped grapheme sources.
pub(crate) fn input_wrap_ranges(text: &str, width: u16) -> Vec<Range<usize>> {
    let hard = styled_hard_lines([(text, PhysicalStyle::default(), Some(0))]);
    let wrapped = wrap_input_styled_lines(&hard, width);
    let mut ranges = Vec::with_capacity(wrapped.len());
    let mut cursor = 0usize;
    for row in &wrapped {
        if cursor < text.len() && text.as_bytes()[cursor] == b'\n' {
            cursor += 1;
        }
        if row.graphemes.is_empty() {
            ranges.push(cursor..cursor);
            continue;
        }
        let start = row
            .graphemes
            .iter()
            .find_map(|grapheme| grapheme.source.as_ref().map(|range| range.start))
            .unwrap_or(cursor);
        let end = row
            .graphemes
            .iter()
            .rev()
            .find_map(|grapheme| grapheme.source.as_ref().map(|range| range.end))
            .unwrap_or(start);
        ranges.push(start..end);
        cursor = end;
    }
    if ranges.is_empty() {
        ranges.push(0..0);
    }
    ranges
}

fn cursor_position(
    source: &str,
    anchor: TextCursorAnchor,
    max_columns: usize,
    rows: &mut Vec<WrappedLine<'_>>,
) -> (usize, usize) {
    let max_columns = max_columns.max(1);
    let caret = anchor.byte_offset;
    let _ = source;

    // Caret x is the sum of already-stored grapheme widths. A caret that sits
    // in a gap (the newline between hard lines, or a wrap boundary) belongs at
    // the end of the preceding row, not at column 0 of the next grapheme.
    let mut last_end = (0usize, 0usize);
    for (row_index, row) in rows.iter().enumerate() {
        let mut column = 0usize;
        for grapheme in &row.graphemes {
            let Some(range) = grapheme.source.as_ref() else {
                column = column.saturating_add(grapheme.width);
                last_end = (row_index, column);
                continue;
            };
            if caret < range.start {
                return place_caret(last_end.0, last_end.1, max_columns, rows);
            }
            if caret < range.end {
                // Inside an EGC: snap to the leading edge so the terminal never
                // bisects a cluster.
                return place_caret(row_index, column, max_columns, rows);
            }
            column = column.saturating_add(grapheme.width);
            last_end = (row_index, column);
        }
        last_end = (row_index, column);
    }

    place_caret(last_end.0, last_end.1, max_columns, rows)
}

fn place_caret(
    mut row: usize,
    mut column: usize,
    max_columns: usize,
    rows: &mut Vec<WrappedLine<'_>>,
) -> (usize, usize) {
    if column >= max_columns {
        row = row.saturating_add(1);
        column = 0;
    }
    while rows.len() <= row {
        rows.push(WrappedLine::new(
            Vec::new(),
            max_columns.saturating_sub(1).max(1),
        ));
    }
    if let Some(wrapped) = rows.get_mut(row) {
        wrapped.width = wrapped.width.max(column.saturating_add(1));
        wrapped.fits = wrapped.width <= max_columns;
    }
    (row, column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::View;
    use crate::physical::PhysicalColor;
    use crate::presentation::{IntoView, layout::compile_view};
    fn fg(color: PhysicalColor) -> PhysicalStyle {
        PhysicalStyle {
            foreground: Some(color),
            ..PhysicalStyle::default()
        }
    }

    fn to_strings(rows: &[WrappedLine]) -> Vec<String> {
        rows.iter()
            .map(|row| {
                row.graphemes
                    .iter()
                    .map(|g| g.text.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn input_wrap_attaches_overflow_space_to_the_adjacent_word() {
        assert_eq!(input_wrap_ranges("one two", 4), vec![0..4, 4..7]);
    }

    #[test]
    fn input_wrap_keeps_leading_whitespace_on_its_logical_line() {
        let ranges = input_wrap_ranges("one\n  two", 4);
        assert_eq!(ranges.first().cloned(), Some(0..3));
        assert!(
            ranges[1..]
                .iter()
                .all(|range| range.start >= 4 && range.end <= 9),
            "indent after a hard newline must stay on that logical line: {ranges:?}"
        );
        assert_eq!(ranges.last().map(|range| range.end), Some(9));
    }

    #[test]
    fn input_wrap_uses_termwiz_width_not_unicode_width() {
        // VS-16 sun is 2 cells in termwiz and 1 in unicode-width. Composer
        // wrap must follow termwiz so two suns at width 3 (caret reserved)
        // cannot share a row.
        let sun = "☀️";
        assert_eq!(grapheme_cell_width(sun), 2);
        let source = format!("{sun}{sun}");
        let ranges = input_wrap_ranges(&source, 3);
        assert_eq!(ranges.len(), 2, "{ranges:?}");
        assert_eq!(ranges[0], 0..sun.len());
        assert_eq!(ranges[1], sun.len()..source.len());
    }

    #[test]
    fn no_wrap_cursor_movement_does_not_rewrap_the_row() {
        let text = "it jump now ";
        let compiled = [2, 7, 11]
            .into_iter()
            .map(|cursor| {
                compile_view(
                    &View::text(text).no_wrap().cursor_at(cursor).into_view(),
                    12,
                )
            })
            .collect::<Vec<_>>();

        let plain_rows = compiled
            .iter()
            .map(|view| {
                view.rows
                    .iter()
                    .map(|row| row.plain_text())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert!(plain_rows.windows(2).all(|rows| rows[0] == rows[1]));
        assert_eq!(plain_rows[0], vec!["it jump now "]);

        let reversed_columns = compiled
            .iter()
            .map(|view| {
                view.rows[0]
                    .cells()
                    .iter()
                    .enumerate()
                    .filter_map(|(column, cell)| cell.style.reversed.then_some(column))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(reversed_columns, vec![vec![2], vec![7], vec![11]]);
    }

    #[test]
    fn plain_words_wrap_at_spaces() {
        let hard = styled_hard_lines(vec![(
            "hello world whatever",
            PhysicalStyle::default(),
            None,
        )]);
        let wrapped = wrap_styled_lines(&hard, 12, WrapMode::WordThenGrapheme);
        assert_eq!(to_strings(&wrapped), vec!["hello world ", "whatever"]);
        assert!(wrapped.iter().all(|r| r.fits));
    }

    #[test]
    fn long_word_hard_breaks_at_graphemes() {
        let hard = styled_hard_lines(vec![("abcdefghij", PhysicalStyle::default(), None)]);
        let wrapped = wrap_styled_lines(&hard, 4, WrapMode::WordThenGrapheme);
        assert_eq!(to_strings(&wrapped), vec!["abcd", "efgh", "ij"]);
        assert!(wrapped.iter().all(|r| r.fits));
    }

    #[test]
    fn emoji_widths_match_termwiz() {
        for sample in [
            "🥇",
            "🏅",
            "4⃣",
            "5⃣",
            "☀️",
            "🌡️",
            "⚠️",
            "☁️",
            "🌤️",
            "⭐",
            "🍿",
            "🌿",
            "🕹️",
            "🗡️",
            "✈️",
            "🗓️",
            "🏷️",
            "👩‍🔬",
            "🐕‍🦺",
            "🇮🇩",
            "🇵🇪",
            "✔️",
            "⚖️",
            "☕",
            "⭐",
        ] {
            let hard = styled_hard_lines(vec![(sample, PhysicalStyle::default(), None)]);
            assert_eq!(hard.len(), 1, "{sample:?}");
            assert_eq!(hard[0].len(), 1, "{sample:?} is one grapheme");
            assert_eq!(
                hard[0][0].width,
                termwiz::cell::grapheme_column_width(sample, None),
                "{sample:?} must use the canonical termwiz cell width"
            );
        }
    }

    #[test]
    fn medal_and_keycap_reserve_the_same_columns() {
        let medal = styled_hard_lines(vec![("🥇Inception", PhysicalStyle::default(), None)]);
        let keycap = styled_hard_lines(vec![("4⃣Mad Max", PhysicalStyle::default(), None)]);
        assert_eq!(
            medal[0][0].width,
            termwiz::cell::grapheme_column_width("🥇", None)
        );
        assert_eq!(
            keycap[0][0].width,
            termwiz::cell::grapheme_column_width("4⃣", None)
        );
        let medal_text = medal[0][1].text.as_ref();
        let keycap_text = keycap[0][1].text.as_ref();
        assert_eq!(medal_text.chars().next(), Some('I'));
        assert_eq!(keycap_text.chars().next(), Some('M'));
    }

    #[test]
    fn zwj_and_flag_clusters_stay_one_wide_glyph() {
        for sample in ["👩‍🔬", "🐕‍🦺", "🇮🇩", "🇯🇵"] {
            let hard = styled_hard_lines(vec![(sample, PhysicalStyle::default(), None)]);
            assert_eq!(hard[0].len(), 1, "{sample} must be one cluster");
            assert_eq!(hard[0][0].width, 2, "{sample} must be two cells");
        }
    }

    #[test]
    fn text_presentation_stars_stay_narrow() {
        let hard = styled_hard_lines(vec![("☆", PhysicalStyle::default(), None)]);
        assert_eq!(hard[0][0].width, 1, "outline star is text presentation");
    }

    #[test]
    fn wide_emoji_never_split() {
        let hard = styled_hard_lines(vec![("😀😁😂😃", PhysicalStyle::default(), None)]);
        let wrapped = wrap_styled_lines(&hard, 3, WrapMode::WordThenGrapheme);
        assert_eq!(to_strings(&wrapped), vec!["😀", "😁", "😂", "😃"]);
        assert!(wrapped.iter().all(|r| r.fits));
    }

    #[test]
    fn combining_characters_never_split() {
        let hard = styled_hard_lines(vec![(
            "e\u{301}e\u{301}e\u{301}",
            PhysicalStyle::default(),
            None,
        )]);
        let wrapped = wrap_styled_lines(&hard, 2, WrapMode::WordThenGrapheme);
        assert_eq!(to_strings(&wrapped), vec!["e\u{301}e\u{301}", "e\u{301}"]);
        assert!(wrapped.iter().all(|r| r.fits));
    }

    #[test]
    fn preserve_hard_newlines() {
        let hard = styled_hard_lines(vec![(
            "line1\nline2\n\nline3",
            PhysicalStyle::default(),
            None,
        )]);
        let wrapped = wrap_styled_lines(&hard, 80, WrapMode::WordThenGrapheme);
        assert_eq!(to_strings(&wrapped), vec!["line1", "line2", "", "line3"]);
        assert!(wrapped.iter().all(|r| r.fits));
    }

    #[test]
    fn combining_mark_across_styled_spans_merges_into_one_grapheme() {
        let style_a = fg(PhysicalColor::Indexed(1));
        let style_b = fg(PhysicalColor::Indexed(4));
        let hard = styled_hard_lines(vec![("e", style_a, Some(0)), ("\u{301}", style_b, Some(1))]);
        assert_eq!(hard.len(), 1);
        assert_eq!(hard[0].len(), 1, "must become ONE StyledGrapheme");
        let g = &hard[0][0];
        assert_eq!(g.text.as_ref(), "e\u{301}");
        assert_eq!(g.width, 1);
        assert_eq!(g.style, style_a, "style follows base codepoint");
        assert_eq!(g.source, Some(0..3), "source spans entire cluster");
    }

    #[test]
    fn zwj_sequence_across_spans_merges_into_one_grapheme() {
        let style_a = fg(PhysicalColor::Indexed(2));
        let style_b = fg(PhysicalColor::Indexed(3));
        let style_c = fg(PhysicalColor::Indexed(5));
        // Woman health worker: 👩 + ZWJ + ⚕ + variation selector 16
        let hard = styled_hard_lines(vec![
            ("👩", style_a, Some(10)),
            ("\u{200D}", style_b, Some(14)),
            ("⚕\u{FE0F}", style_c, Some(17)),
        ]);
        assert_eq!(hard.len(), 1);
        assert_eq!(
            hard[0].len(),
            1,
            "ZWJ sequence across spans is one atomic cluster"
        );
        let g = &hard[0][0];
        assert_eq!(g.text.as_ref(), "👩\u{200D}⚕\u{FE0F}");
        assert_eq!(g.width, 2);
        assert_eq!(g.style, style_a, "style follows base codepoint");
        assert_eq!(g.source, Some(10..23), "source spans full ZWJ sequence");
    }

    #[test]
    fn oversized_grapheme_marks_fits_false() {
        let hard = styled_hard_lines(vec![("漢字", PhysicalStyle::default(), Some(0))]);
        // Target width = 1 cell, but each CJK char is 2 cells wide.
        let wrapped = wrap_styled_lines(&hard, 1, WrapMode::WordThenGrapheme);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(wrapped[0].graphemes[0].text.as_ref(), "漢");
        assert_eq!(wrapped[0].width, 2);
        assert!(
            !wrapped[0].fits,
            "2-cell grapheme on 1-cell width does not fit"
        );
        assert_eq!(wrapped[1].graphemes[0].text.as_ref(), "字");
        assert_eq!(wrapped[1].width, 2);
        assert!(!wrapped[1].fits);
    }

    #[test]
    fn nowrap_keeps_the_line_as_one_row_even_when_it_overflows() {
        let hard = styled_hard_lines(vec![("ABC🐕‍🦺DEF", PhysicalStyle::default(), None)]);
        let wrapped = wrap_styled_lines(&hard, 4, WrapMode::NoWrap);
        assert_eq!(wrapped.len(), 1);
        assert!(!wrapped[0].fits);
        assert!(wrapped[0].width > 4);
    }
}
