use napi::Status;
use napi::bindgen_prelude::Result;
use std::collections::HashSet;

use iyon_tui::{
    BorderEdges, BorderGlyphs, BorderSpec, ColorSpec, DiffHunk, DiffLine, DiffLineNumber,
    DiffLineOffset, DiffLineTermination, DiffRange, GridCellSpec, GridTrack, HorizontalAlign,
    Insets, IntoView, Renderer, StyleRef, StyleSpec, TextAttribute, TextSpan, VerticalAlign, View,
    WrapMode,
};

use super::ViewRuntimeHandle;
use super::{
    ALIGN_CENTER, ALIGN_END, ALIGN_START, DIFF_ADDITION, DIFF_CONTEXT, DIFF_DELETION,
    DIFF_TERMINATED, DIFF_UNTERMINATED, GRID_TRACK_CONTENT, GRID_TRACK_CONTENT_MAX,
    GRID_TRACK_FIXED, GRID_TRACK_FLEX, GRID_TRACK_FLEX_MAX, LAYOUT_CHILD_CONTENT_MAX,
    LAYOUT_CHILD_FIXED, LAYOUT_CHILD_FLEX, LAYOUT_CHILD_FLEX_MAX, LAYOUT_CHILD_NORMAL,
    OVERFLOW_ELLIPSIS, OVERFLOW_FOOTER, OVERFLOW_NONE, PACKED_BORDER_COLOR, PACKED_BORDER_EDGES,
    PACKED_BORDER_EDGES_ABSENT, PACKED_BORDER_EDGES_ALL, PACKED_BORDER_EDGES_TOP_BOTTOM,
    PACKED_BORDER_GLYPHS, PACKED_BORDER_STYLE, PACKED_BORDER_STYLE_ABSENT,
    PACKED_BORDER_STYLE_DOUBLE, PACKED_BORDER_STYLE_PLAIN, PACKED_BORDER_STYLE_ROUNDED,
    PACKED_COLOR_ANSI, PACKED_COLOR_NONE, PACKED_COLOR_STRING, PACKED_DECORATION_BACKGROUND,
    PACKED_DECORATION_BORDER, PACKED_DECORATION_FOREGROUND, PACKED_DECORATION_HEIGHT,
    PACKED_DECORATION_MAX_HEIGHT, PACKED_DECORATION_MAX_WIDTH, PACKED_DECORATION_MIN_HEIGHT,
    PACKED_DECORATION_MIN_WIDTH, PACKED_DECORATION_PADDING, PACKED_DECORATION_STATES,
    PACKED_DECORATION_STYLE, PACKED_DECORATION_WIDTH, PACKED_RULE_FILL, PACKED_RULE_FIT,
    PACKED_STYLE_BACKGROUND, PACKED_STYLE_FOREGROUND, PACKED_STYLE_THEME, PACKED_VIEW_DEF,
    PACKED_VIEW_MAGIC, PACKED_VIEW_PROTOCOL_VERSION, PACKED_VIEW_REF, VERTICAL_BOTTOM,
    VERTICAL_CENTER, VERTICAL_TOP, VIEW_BRIDGE_SCHEMA_VERSION, VIEW_KIND_CLAMP, VIEW_KIND_COLUMN,
    VIEW_KIND_COMPONENT, VIEW_KIND_CONTAINER, VIEW_KIND_CONTENT_MAX, VIEW_KIND_DECORATED,
    VIEW_KIND_DIFF, VIEW_KIND_GRID, VIEW_KIND_HANGING, VIEW_KIND_ROW, VIEW_KIND_SPACER,
    VIEW_KIND_TEXT, WRAP_GRAPHEME, WRAP_NO_WRAP, WRAP_WORD_THEN_GRAPHEME,
};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const STYLE_BOLD: u32 = 1;
const STYLE_DIM: u32 = 2;
const STYLE_ITALIC: u32 = 4;
const STYLE_UNDERLINE: u32 = 8;
const STYLE_REVERSED: u32 = 16;
const STYLE_STRIKETHROUGH: u32 = 32;
const STYLE_ALL: u32 =
    STYLE_BOLD | STYLE_DIM | STYLE_ITALIC | STYLE_UNDERLINE | STYLE_REVERSED | STYLE_STRIKETHROUGH;

fn invalid(message: impl Into<String>) -> napi::Error {
    crate::NativeError::invalid_input(message)
}

fn cache_miss(id: u64) -> napi::Error {
    crate::NativeError::coded(
        Status::InvalidArg,
        "ION_PACKED_CACHE_MISS",
        format!("packed reference points to expired node id {id}"),
    )
}

fn inc(counter: iyon_tui::perf::Counter) {
    #[cfg(feature = "perf-counters")]
    iyon_tui::perf::inc(counter);
    #[cfg(not(feature = "perf-counters"))]
    let _ = counter;
}

fn add(counter: iyon_tui::perf::Counter, amount: usize) {
    #[cfg(feature = "perf-counters")]
    iyon_tui::perf::add(counter, amount as u64);
    #[cfg(not(feature = "perf-counters"))]
    let _ = (counter, amount);
}

pub(super) fn decode_one(
    words: &[u32],
    strings: &[String],
    cache: ViewRuntimeHandle,
) -> Result<View> {
    let mut transaction = PackedTransaction::new(words, strings, cache)?;
    let roots = transaction.decode_roots()?;
    if roots.len() != 1 {
        return Err(invalid("packed render requires exactly one root"));
    }
    Ok(roots.into_iter().next().expect("one root was validated"))
}

struct PackedTransaction<'a> {
    words: &'a [u32],
    strings: &'a [String],
    cursor: usize,
    cache: ViewRuntimeHandle,
    active: HashSet<u64>,
}

impl<'a> PackedTransaction<'a> {
    fn new(words: &'a [u32], strings: &'a [String], cache: ViewRuntimeHandle) -> Result<Self> {
        if words.len() < 5 {
            return Err(invalid("packed transaction header is truncated"));
        }
        if words[0] != PACKED_VIEW_MAGIC {
            return Err(invalid("unknown packed View transaction magic"));
        }
        if words[1] != PACKED_VIEW_PROTOCOL_VERSION {
            return Err(invalid("unsupported packed View transaction version"));
        }
        if words[2] != VIEW_BRIDGE_SCHEMA_VERSION {
            return Err(invalid(
                "packed View schema version does not match the direct bridge",
            ));
        }
        let used_words = usize::try_from(words[3])
            .map_err(|_| invalid("packed transaction used word count is invalid"))?;
        if used_words < 5 || used_words > words.len() {
            return Err(invalid(
                "packed transaction used word count is out of bounds",
            ));
        }
        let root_count = usize::try_from(words[4])
            .map_err(|_| invalid("packed transaction root count is invalid"))?;
        if root_count == 0 {
            return Err(invalid("packed transaction requires at least one root"));
        }
        Ok(Self {
            words: &words[..used_words],
            strings,
            cursor: 5,
            cache,
            active: HashSet::new(),
        })
    }

    fn decode_roots(&mut self) -> Result<Vec<View>> {
        let root_count = usize::try_from(self.words[4]).expect("validated root count");
        let mut roots = Vec::with_capacity(root_count);
        for _ in 0..root_count {
            roots.push(self.decode_node()?);
        }
        if self.cursor != self.words.len() {
            return Err(invalid("packed TUI transaction has trailing words"));
        }
        Ok(roots)
    }

    fn decode_node(&mut self) -> Result<View> {
        inc(iyon_tui::perf::Counter::NapiPackedNodesSeen);
        let start = self.cursor;
        let opcode = self.word("opcode")?;
        let record_words = usize::try_from(self.word("record length")?)
            .map_err(|_| invalid("packed record length is invalid"))?;
        if record_words < 4 || start.checked_add(record_words).is_none() {
            return Err(invalid("packed record length is invalid"));
        }
        let end = start + record_words;
        if end > self.words.len() || self.cursor > end {
            return Err(invalid("packed record extends beyond transaction"));
        }
        let id = self.node_id()?;
        if opcode == PACKED_VIEW_REF {
            if record_words != 4 {
                return Err(invalid("packed REF record length is invalid"));
            }
            if self.active.contains(&id) {
                return Err(invalid(format!("packed cyclic reference for node id {id}")));
            }
            let view = self.cached(id)?;
            if self.cursor != end {
                return Err(invalid("packed REF record has trailing payload"));
            }
            return Ok(view);
        }
        if opcode != PACKED_VIEW_DEF || record_words < 5 {
            return Err(invalid(format!("unknown packed View opcode {opcode}")));
        }
        if !self.active.insert(id) {
            return Err(invalid(format!(
                "packed cyclic definition for node id {id}"
            )));
        }
        let result = self.decode_definition(end);
        self.active.remove(&id);
        let candidate = result?;
        if self.cursor != end {
            return Err(invalid(
                "packed DEF record length does not match its payload",
            ));
        }

        let existing = super::with_view_runtime(&self.cache, |cache| cache.live_cached_view(id))?;
        let view = if let Some(existing) = existing {
            if existing != candidate {
                return Err(invalid(format!(
                    "packed NodeId {id} changed semantic identity"
                )));
            }
            existing
        } else {
            candidate
        };
        super::with_view_runtime(&self.cache, |cache| {
            cache.record_decoded_semantic_view(id, &view)
        })
        .and_then(|recorded| {
            recorded.map_err(|_| invalid(format!("packed NodeId {id} changed semantic identity")))
        })?;
        inc(iyon_tui::perf::Counter::NapiPackedDefsDecoded);
        Ok(view)
    }

    fn decode_definition(&mut self, end: usize) -> Result<View> {
        let kind = self.word("view kind")?;
        match kind {
            VIEW_KIND_TEXT => self.decode_text(),
            VIEW_KIND_DIFF => self.decode_diff(),
            VIEW_KIND_SPACER => Ok(View::spacer(self.u16("spacer rows")?)),
            VIEW_KIND_ROW => self.decode_axis(end, true),
            VIEW_KIND_COLUMN => self.decode_axis(end, false),
            VIEW_KIND_HANGING => Ok(View::hanging(
                self.decode_node()?,
                self.decode_node()?,
                self.decode_node()?,
            )),
            VIEW_KIND_GRID => self.decode_grid(),
            VIEW_KIND_CONTAINER => Ok(self.decode_node()?.container()),
            VIEW_KIND_CLAMP => {
                let max_rows = self.u16("clamp maxRows")?;
                let overflow = self.decode_overflow()?;
                Ok(self.decode_node()?.clamp_rows(max_rows, overflow))
            }
            VIEW_KIND_CONTENT_MAX => {
                let max_rows = self.u16("contentMax maxRows")?;
                Ok(self
                    .decode_node()?
                    .clamp_rows(max_rows, iyon_tui::OverflowIndicator::None))
            }
            VIEW_KIND_COMPONENT => Ok(View::native_component(
                self.positive_safe("component handle")?,
            )),
            VIEW_KIND_DECORATED => {
                let view = self.decode_node()?;
                self.decode_decoration(view)
            }
            other => Err(invalid(format!("unknown packed View kind {other}"))),
        }
    }

    fn decode_text(&mut self) -> Result<View> {
        let wrap = decode_wrap(self.word("text wrap")?)?;
        let align = decode_horizontal_align(self.word("text alignment")?)?;
        let span_count = self.count("text span count")?;
        let mut spans = Vec::with_capacity(span_count);
        for _ in 0..span_count {
            let text_index = self.word("text string index")?;
            let text = self.string(text_index)?.to_owned();
            let has_style = self.word("text style presence")?;
            let style = if has_style == 0 {
                StyleRef::direct(StyleSpec::new())
            } else if has_style == 1 {
                self.decode_style_ref()?
            } else {
                return Err(invalid("packed text style presence must be 0 or 1"));
            };
            spans.push(TextSpan::styled(text, style));
        }
        Ok(View::styled_text(spans)
            .wrap(wrap)
            .text_align(align)
            .into_view())
    }

    fn decode_diff(&mut self) -> Result<View> {
        let hunk_count = self.count("diff hunk count")?;
        let mut hunks = Vec::with_capacity(hunk_count);
        for _ in 0..hunk_count {
            let old_range = self.diff_range()?;
            let new_range = self.diff_range()?;
            let line_count = self.count("diff line count")?;
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                let kind = self.word("diff line kind")?;
                let text_index = self.word("diff line text")?;
                let text = self.string(text_index)?.to_owned();
                let termination = match self.word("diff line termination")? {
                    DIFF_TERMINATED => DiffLineTermination::Terminated,
                    DIFF_UNTERMINATED => DiffLineTermination::Unterminated,
                    other => return Err(invalid(format!("unknown diff termination {other}"))),
                };
                let line = match kind {
                    DIFF_CONTEXT => {
                        DiffLine::context(self.diff_line_number()?, self.diff_line_number()?, text)
                    }
                    DIFF_ADDITION => DiffLine::addition(self.diff_line_number()?, text),
                    DIFF_DELETION => DiffLine::deletion(self.diff_line_number()?, text),
                    other => return Err(invalid(format!("unknown diff line kind {other}"))),
                };
                lines.push(line.with_termination(termination));
            }
            hunks.push(
                DiffHunk::new(old_range, new_range, lines).map_err(|e| invalid(e.to_string()))?,
            );
        }
        Ok(iyon_tui::DiffRenderer::new().render(hunks.as_slice()))
    }

    fn decode_axis(&mut self, _end: usize, horizontal: bool) -> Result<View> {
        let gap = self.u16("axis gap")?;
        let child_count = self.count("axis child count")?;
        let mut children = Vec::with_capacity(child_count);
        for _ in 0..child_count {
            let kind = self.word("layout child kind")?;
            let size = self.u16("layout child size")?;
            let max_rows = self.u16("layout child maxRows")?;
            let view = self.decode_node()?;
            match kind {
                LAYOUT_CHILD_NORMAL | LAYOUT_CHILD_FLEX if size == 0 && max_rows == 0 => {}
                LAYOUT_CHILD_FIXED if max_rows == 0 => {}
                LAYOUT_CHILD_FLEX_MAX if !horizontal && size == 0 => {}
                LAYOUT_CHILD_CONTENT_MAX if !horizontal && size == 0 => {}
                LAYOUT_CHILD_FLEX_MAX | LAYOUT_CHILD_CONTENT_MAX if horizontal => {
                    return Err(invalid("vertical-only layout child used in a row"));
                }
                _ => return Err(invalid("packed layout child fields are invalid")),
            }
            children.push((kind, size, max_rows, view));
        }
        if horizontal {
            Ok(View::horizontal(|row| {
                row.gap(gap);
                for (kind, size, _max_rows, view) in children {
                    match kind {
                        LAYOUT_CHILD_NORMAL => row.child(view),
                        LAYOUT_CHILD_FIXED => row.fixed(size, view),
                        LAYOUT_CHILD_FLEX => row.flex(view),
                        _ => unreachable!(),
                    };
                }
            }))
        } else {
            Ok(View::vertical(|column| {
                column.gap(gap);
                for (kind, size, max_rows, view) in children {
                    match kind {
                        LAYOUT_CHILD_NORMAL => column.child(view),
                        LAYOUT_CHILD_FIXED => column.fixed(size, view),
                        LAYOUT_CHILD_FLEX => column.flex(view),
                        LAYOUT_CHILD_FLEX_MAX => column.flex_max(max_rows, view),
                        LAYOUT_CHILD_CONTENT_MAX => column.content_max(max_rows, view),
                        _ => unreachable!(),
                    };
                }
            }))
        }
    }

    fn decode_grid(&mut self) -> Result<View> {
        let column_count = self.count("grid column count")?;
        let mut columns = Vec::with_capacity(column_count);
        for _ in 0..column_count {
            columns.push(self.grid_track()?);
        }
        let row_count = self.count("grid row count")?;
        let mut rows = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            let track = self.grid_track()?;
            let cell_count = self.count("grid cell count")?;
            let mut cells = Vec::with_capacity(cell_count);
            for _ in 0..cell_count {
                let spec = GridCellSpec::new()
                    .column_span(self.positive_u16("grid columnSpan")?)
                    .row_span(self.positive_u16("grid rowSpan")?)
                    .horizontal_align(decode_horizontal_align(
                        self.word("grid horizontal alignment")?,
                    )?)
                    .vertical_align(decode_vertical_align(
                        self.word("grid vertical alignment")?,
                    )?);
                cells.push((spec, self.decode_node()?));
            }
            rows.push((track, cells));
        }
        let column_gap = self.u16("grid columnGap")?;
        let row_gap = self.u16("grid rowGap")?;
        Ok(View::grid(|grid| {
            grid.columns(columns);
            grid.column_gap(column_gap);
            grid.row_gap(row_gap);
            for (track, cells) in rows {
                grid.row_with(track, |row| {
                    for (spec, view) in cells {
                        row.cell_with(spec, view);
                    }
                });
            }
        }))
    }

    fn decode_overflow(&mut self) -> Result<iyon_tui::OverflowIndicator> {
        match self.word("overflow kind")? {
            OVERFLOW_NONE => Ok(iyon_tui::OverflowIndicator::None),
            OVERFLOW_ELLIPSIS => Ok(iyon_tui::OverflowIndicator::Ellipsis {
                style: self.decode_style_ref()?,
            }),
            OVERFLOW_FOOTER => {
                let prefix_index = self.word("overflow footer prefix")?;
                let prefix = self.string(prefix_index)?.to_owned();
                Ok(iyon_tui::OverflowIndicator::Footer {
                    prefix,
                    style: self.decode_style_ref()?,
                })
            }
            other => Err(invalid(format!("unknown overflow kind {other}"))),
        }
    }

    fn decode_decoration(&mut self, mut view: View) -> Result<View> {
        let flags = self.word("decoration flags")?;
        let known = PACKED_DECORATION_PADDING
            | PACKED_DECORATION_BACKGROUND
            | PACKED_DECORATION_FOREGROUND
            | PACKED_DECORATION_BORDER
            | PACKED_DECORATION_STYLE
            | PACKED_DECORATION_STATES
            | PACKED_DECORATION_WIDTH
            | PACKED_DECORATION_HEIGHT
            | PACKED_DECORATION_MIN_WIDTH
            | PACKED_DECORATION_MAX_WIDTH
            | PACKED_DECORATION_MIN_HEIGHT
            | PACKED_DECORATION_MAX_HEIGHT;
        if flags & !known != 0 || flags & PACKED_DECORATION_STYLE == 0 {
            return Err(invalid("packed decoration flags are invalid"));
        }
        if flags & PACKED_DECORATION_PADDING != 0 {
            view = view.padding(Insets::new(
                self.u16("padding top")?,
                self.u16("padding right")?,
                self.u16("padding bottom")?,
                self.u16("padding left")?,
            ));
        }
        if flags & PACKED_DECORATION_BACKGROUND != 0 {
            view = view.background(self.color()?);
        }
        if flags & PACKED_DECORATION_FOREGROUND != 0 {
            view = view.foreground(self.color()?);
        }
        if flags & PACKED_DECORATION_BORDER != 0 {
            view = view.border(self.border()?);
        }
        view = view.style(self.decode_style_ref()?);
        if flags & PACKED_DECORATION_STATES != 0 {
            for _ in 0..self.count("style state count")? {
                let key_index = self.word("style state key")?;
                let value_index = self.word("style state value")?;
                let key = self.string(key_index)?.to_owned();
                let value = self.string(value_index)?.to_owned();
                view = view.style_state(key, value);
            }
        }
        if flags & PACKED_DECORATION_WIDTH != 0 {
            view = apply_width(view, self.rule("width rule")?, true)?;
        }
        if flags & PACKED_DECORATION_HEIGHT != 0 {
            view = apply_width(view, self.rule("height rule")?, false)?;
        }
        if flags & PACKED_DECORATION_MIN_WIDTH != 0 {
            view = view.min_width(self.u16("minWidth")?);
        }
        if flags & PACKED_DECORATION_MAX_WIDTH != 0 {
            view = view.max_width(self.u16("maxWidth")?);
        }
        if flags & PACKED_DECORATION_MIN_HEIGHT != 0 {
            view = view.min_height(self.u16("minHeight")?);
        }
        if flags & PACKED_DECORATION_MAX_HEIGHT != 0 {
            view = view.max_height(self.u16("maxHeight")?);
        }
        Ok(view)
    }

    fn decode_style_ref(&mut self) -> Result<StyleRef> {
        let flags = self.word("style flags")?;
        let known = PACKED_STYLE_THEME | PACKED_STYLE_FOREGROUND | PACKED_STYLE_BACKGROUND;
        if flags & !known != 0 {
            return Err(invalid("packed style flags are invalid"));
        }
        let theme = if flags & PACKED_STYLE_THEME != 0 {
            let index = self.word("theme string index")?;
            Some(self.string(index)?.to_owned())
        } else {
            None
        };
        let foreground = if flags & PACKED_STYLE_FOREGROUND != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let background = if flags & PACKED_STYLE_BACKGROUND != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let present = self.word("style attribute presence")?;
        let truth = self.word("style attribute truth")?;
        if present & !STYLE_ALL != 0 || truth & !present != 0 {
            return Err(invalid("packed style attribute masks are invalid"));
        }
        let mut style = StyleSpec::new();
        if let Some(color) = foreground {
            style = style.foreground(color);
        }
        if let Some(color) = background {
            style = style.background(color);
        }
        for (bit, attribute) in [
            (STYLE_BOLD, TextAttribute::Bold),
            (STYLE_DIM, TextAttribute::Dim),
            (STYLE_ITALIC, TextAttribute::Italic),
            (STYLE_UNDERLINE, TextAttribute::Underline),
            (STYLE_REVERSED, TextAttribute::Reversed),
            (STYLE_STRIKETHROUGH, TextAttribute::Strikethrough),
        ] {
            if present & bit != 0 {
                style = style.attribute(attribute, truth & bit != 0);
            }
        }
        Ok(match theme {
            Some(name) => StyleRef::themed(name, style),
            None => StyleRef::direct(style),
        })
    }

    fn color(&mut self) -> Result<ColorSpec> {
        match self.word("color tag")? {
            PACKED_COLOR_STRING => {
                let index = self.word("color string index")?;
                super::decode_color_string(self.string(index)?)
            }
            PACKED_COLOR_ANSI => Ok(ColorSpec::ansi(
                u8::try_from(self.word("ANSI color")?)
                    .map_err(|_| invalid("ANSI color must fit in u8"))?,
            )),
            PACKED_COLOR_NONE => Err(invalid("packed required color cannot be none")),
            other => Err(invalid(format!("unknown packed color tag {other}"))),
        }
    }

    fn border(&mut self) -> Result<BorderSpec> {
        let flags = self.word("border flags")?;
        let known =
            PACKED_BORDER_GLYPHS | PACKED_BORDER_COLOR | PACKED_BORDER_STYLE | PACKED_BORDER_EDGES;
        if flags & !known != 0 {
            return Err(invalid("packed border flags are invalid"));
        }
        let glyphs = if flags & PACKED_BORDER_GLYPHS != 0 {
            let mut values = Vec::with_capacity(8);
            for _ in 0..8 {
                let index = self.word("border glyph string index")?;
                values.push(self.string(index)?.to_owned());
            }
            Some(values)
        } else {
            None
        };
        let color = if flags & PACKED_BORDER_COLOR != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let style = if flags & PACKED_BORDER_STYLE != 0 {
            Some(self.word("border style")?)
        } else {
            None
        };
        let edges = if flags & PACKED_BORDER_EDGES != 0 {
            Some(self.word("border edges")?)
        } else {
            None
        };
        let mut spec = match style.unwrap_or(PACKED_BORDER_STYLE_PLAIN) {
            PACKED_BORDER_STYLE_PLAIN => BorderSpec::plain(),
            PACKED_BORDER_STYLE_ROUNDED => BorderSpec::rounded(),
            PACKED_BORDER_STYLE_DOUBLE => BorderSpec::double(),
            PACKED_BORDER_STYLE_ABSENT => BorderSpec::plain(),
            other => return Err(invalid(format!("unknown packed border style {other}"))),
        };
        if let Some(values) = glyphs {
            spec = BorderSpec::custom(
                BorderGlyphs::new(
                    &values[0], &values[1], &values[2], &values[3], &values[4], &values[5],
                    &values[6], &values[7],
                )
                .map_err(|e| invalid(e.to_string()))?,
            );
        }
        match edges.unwrap_or(PACKED_BORDER_EDGES_ALL) {
            PACKED_BORDER_EDGES_ALL => {}
            PACKED_BORDER_EDGES_TOP_BOTTOM => spec = spec.edges(BorderEdges::TOP_BOTTOM),
            PACKED_BORDER_EDGES_ABSENT => {}
            other => return Err(invalid(format!("unknown packed border edges {other}"))),
        }
        if let Some(color) = color {
            spec = spec.color(color);
        }
        Ok(spec)
    }

    fn grid_track(&mut self) -> Result<GridTrack> {
        let kind = self.word("grid track kind")?;
        let value = self.u16("grid track value")?;
        match kind {
            GRID_TRACK_CONTENT if value == 0 => Ok(GridTrack::content()),
            GRID_TRACK_CONTENT_MAX => Ok(GridTrack::content_max(value)),
            GRID_TRACK_FIXED => Ok(GridTrack::fixed(value)),
            GRID_TRACK_FLEX if value == 0 => Ok(GridTrack::flex()),
            GRID_TRACK_FLEX_MAX => Ok(GridTrack::flex_max(value)),
            _ => Err(invalid("packed grid track fields are invalid")),
        }
    }

    fn diff_range(&mut self) -> Result<DiffRange> {
        let start = self.safe_nonnegative("diff range start")?;
        let count = self.safe_nonnegative("diff range count")?;
        DiffRange::new(DiffLineOffset::new(start), count).map_err(|e| invalid(e.to_string()))
    }

    fn diff_line_number(&mut self) -> Result<DiffLineNumber> {
        DiffLineNumber::new(self.positive_safe("diff line number")?)
            .ok_or_else(|| invalid("diff line number must be >= 1"))
    }

    fn cached(&mut self, id: u64) -> Result<View> {
        let view = super::with_view_runtime(&self.cache, |cache| {
            cache.nodes.get(&id).and_then(iyon_tui::WeakView::upgrade)
        })?;
        if let Some(view) = view {
            inc(iyon_tui::perf::Counter::NapiPackedRefHits);
            return Ok(view);
        }
        super::with_view_runtime(&self.cache, |cache| {
            cache.nodes.remove(&id);
        })?;
        inc(iyon_tui::perf::Counter::NapiPackedRefMisses);
        Err(cache_miss(id))
    }

    fn node_id(&mut self) -> Result<u64> {
        let low = u64::from(self.word("node id low")?);
        let high = u64::from(self.word("node id high")?);
        let value = (high << 32) | low;
        if value == 0 || value > MAX_SAFE_INTEGER {
            return Err(invalid("packed node id must be a positive safe integer"));
        }
        Ok(value)
    }

    fn positive_safe(&mut self, name: &str) -> Result<u64> {
        let value = self.safe_nonnegative(name)?;
        if value == 0 {
            return Err(invalid(format!("{name} must be positive")));
        }
        Ok(value)
    }

    fn safe_nonnegative(&mut self, name: &str) -> Result<u64> {
        let low = u64::from(self.word(&format!("{name} low"))?);
        let high = u64::from(self.word(&format!("{name} high"))?);
        let value = (high << 32) | low;
        if value > MAX_SAFE_INTEGER {
            return Err(invalid(format!("{name} must be a safe integer")));
        }
        Ok(value)
    }

    fn string(&mut self, index: u32) -> Result<&'a str> {
        let value = self
            .strings
            .get(index as usize)
            .ok_or_else(|| invalid("packed string index is out of range"))?;
        add(
            iyon_tui::perf::Counter::NapiPackedStringBytesCopied,
            value.len(),
        );
        Ok(value)
    }

    fn count(&mut self, name: &str) -> Result<usize> {
        let value = self.word(name)? as usize;
        if value > 1_000_000 {
            return Err(invalid(format!("{name} is unreasonably large")));
        }
        Ok(value)
    }

    fn u16(&mut self, name: &str) -> Result<u16> {
        u16::try_from(self.word(name)?).map_err(|_| invalid(format!("{name} must fit in u16")))
    }

    fn positive_u16(&mut self, name: &str) -> Result<u16> {
        let value = self.u16(name)?;
        if value == 0 {
            return Err(invalid(format!("{name} must be positive")));
        }
        Ok(value)
    }

    fn rule(&mut self, name: &str) -> Result<u32> {
        let value = self.word(name)?;
        if value == PACKED_RULE_FIT || value == PACKED_RULE_FILL {
            Ok(value)
        } else {
            Err(invalid(format!("invalid {name}")))
        }
    }

    fn word(&mut self, name: &str) -> Result<u32> {
        let value = *self
            .words
            .get(self.cursor)
            .ok_or_else(|| invalid(format!("packed transaction is missing {name}")))?;
        self.cursor += 1;
        inc(iyon_tui::perf::Counter::NapiPackedWordsRead);
        Ok(value)
    }
}

fn apply_width(view: View, rule: u32, horizontal: bool) -> Result<View> {
    match (horizontal, rule) {
        (true, PACKED_RULE_FIT) => Ok(view.fit_width()),
        (true, PACKED_RULE_FILL) => Ok(view.fill_width()),
        (false, PACKED_RULE_FIT) => Ok(view.fit_height()),
        (false, PACKED_RULE_FILL) => Ok(view.fill_height()),
        _ => Err(invalid("invalid packed width/height rule")),
    }
}

fn decode_wrap(value: u32) -> Result<WrapMode> {
    match value {
        WRAP_WORD_THEN_GRAPHEME => Ok(WrapMode::WordThenGrapheme),
        WRAP_GRAPHEME => Ok(WrapMode::Grapheme),
        WRAP_NO_WRAP => Ok(WrapMode::NoWrap),
        other => Err(invalid(format!("unknown packed wrap mode {other}"))),
    }
}

fn decode_horizontal_align(value: u32) -> Result<HorizontalAlign> {
    match value {
        ALIGN_START => Ok(HorizontalAlign::Start),
        ALIGN_CENTER => Ok(HorizontalAlign::Center),
        ALIGN_END => Ok(HorizontalAlign::End),
        other => Err(invalid(format!(
            "unknown packed horizontal alignment {other}"
        ))),
    }
}

fn decode_vertical_align(value: u32) -> Result<VerticalAlign> {
    match value {
        VERTICAL_TOP => Ok(VerticalAlign::Top),
        VERTICAL_CENTER => Ok(VerticalAlign::Center),
        VERTICAL_BOTTOM => Ok(VerticalAlign::Bottom),
        other => Err(invalid(format!(
            "unknown packed vertical alignment {other}"
        ))),
    }
}
