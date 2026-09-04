//! Test-only independent terminal interpreter.
//!
//! `ShadowTerminal` executes termwiz `Change` sequences against an in-memory
//! screen and scrollback tape without reusing Iyon's internal `Surface` composition
//! or `PhysicalRow::placed()`.
//!
//! It acts as an independent test oracle to verify that:
//! 1. Native scrollback is created exclusively through bottom-row CRLF.
//! 2. Model-only `ScrollRegionUp` is never emitted over the wire.
//! 3. Presenter differential state matches actual terminal tape state.

use std::time::Duration;
use termwiz::{
    cell::CellAttributes,
    surface::{Change, CursorVisibility, Position, Surface},
    terminal::{ScreenSize, Terminal, TerminalWaker},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::physical::grapheme_cell_width;

pub(crate) fn physical_style(style: crate::physical::PhysicalStyle) -> CellAttributes {
    super::lower::physical_style(style)
}

pub(crate) const SYNC_BEGIN: &str = "\x1b[?2026h";
pub(crate) const SYNC_END: &str = "\x1b[?2026l";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShadowCell {
    pub(crate) grapheme: Option<String>,
    pub(crate) attrs: CellAttributes,
    pub(crate) continuation: bool,
}

impl ShadowCell {
    pub(crate) fn blank() -> Self {
        Self {
            grapheme: None,
            attrs: CellAttributes::default(),
            continuation: false,
        }
    }

    pub(crate) fn blank_with_attrs(attrs: CellAttributes) -> Self {
        Self {
            grapheme: None,
            attrs,
            continuation: false,
        }
    }

    pub(crate) fn text(&self) -> &str {
        if self.continuation {
            ""
        } else {
            self.grapheme.as_deref().unwrap_or(" ")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShadowRow {
    pub(crate) cells: Vec<ShadowCell>,
}

impl ShadowRow {
    pub(crate) fn blank(width: usize) -> Self {
        Self {
            cells: vec![ShadowCell::blank(); width],
        }
    }

    pub(crate) fn text(&self) -> String {
        let mut s = String::new();
        for cell in &self.cells {
            if cell.continuation {
                continue;
            }
            if let Some(ref g) = cell.grapheme {
                s.push_str(g);
            } else {
                s.push(' ');
            }
        }
        s
    }

    pub(crate) fn trimmed_text(&self) -> String {
        self.text().trim_end().to_string()
    }

    pub(crate) fn clear_glyph_at(&mut self, col: usize) {
        if col >= self.cells.len() {
            return;
        }
        if self.cells[col].continuation {
            let mut leader = col;
            while leader > 0 && self.cells[leader].continuation {
                leader -= 1;
            }
            let attrs = self.cells[leader].attrs.clone();
            self.cells[leader] = ShadowCell::blank_with_attrs(attrs.clone());
            let mut curr = leader + 1;
            while curr < self.cells.len() && self.cells[curr].continuation {
                self.cells[curr] = ShadowCell::blank_with_attrs(attrs.clone());
                curr += 1;
            }
        } else if let Some(ref g) = self.cells[col].grapheme {
            let width = grapheme_cell_width(g);
            let attrs = self.cells[col].attrs.clone();
            self.cells[col] = ShadowCell::blank_with_attrs(attrs.clone());
            for c in 1..width {
                if col + c < self.cells.len() && self.cells[col + c].continuation {
                    self.cells[col + c] = ShadowCell::blank_with_attrs(attrs.clone());
                }
            }
        }
    }
}

/// One native-scrollback commit, analogous to `OpenTUI` `external_output` events.
///
/// `OpenCode`'s test renderer records styled snapshots as they are published above
/// the split footer. Iyon publishes by full-screen CRLF, so a commit here is one
/// displaced screen row that entered terminal scrollback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShadowScrollbackCommit {
    pub(crate) row: ShadowRow,
    pub(crate) width: usize,
    pub(crate) trailing_newline: bool,
}

impl ShadowScrollbackCommit {
    pub(crate) fn text(&self) -> String {
        self.row.trimmed_text()
    }
}

/// Styled span inside a captured frame, analogous to `OpenTUI` `CapturedSpan`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapturedSpan {
    pub(crate) text: String,
    pub(crate) attrs: CellAttributes,
    pub(crate) width: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapturedLine {
    pub(crate) spans: Vec<CapturedSpan>,
}

/// Visible-screen capture, analogous to `OpenTUI` `captureSpans()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapturedFrame {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cursor: (usize, usize),
    pub(crate) lines: Vec<CapturedLine>,
}

#[derive(Clone, Debug)]
pub(crate) struct ShadowTerminal {
    pub(crate) width: usize,
    pub(crate) height: usize,

    pub(crate) screen: Vec<ShadowRow>,
    pub(crate) scrollback: Vec<ShadowRow>,

    pub(crate) cursor_x: usize,
    pub(crate) cursor_y: usize,

    pub(crate) attrs: CellAttributes,
    pub(crate) cursor_visible: CursorVisibility,

    pub(crate) sync_output_active: bool,

    pub(crate) implicit_wraps: usize,

    pub(crate) fail_next_render: bool,

    claimed_through: usize,
}

impl ShadowTerminal {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            screen: (0..height).map(|_| ShadowRow::blank(width)).collect(),
            scrollback: Vec::new(),
            cursor_x: 0,
            cursor_y: 0,
            attrs: CellAttributes::default(),
            cursor_visible: CursorVisibility::Visible,
            sync_output_active: false,
            implicit_wraps: 0,
            fail_next_render: false,
            claimed_through: 0,
        }
    }

    pub(crate) fn fail_next_render(&mut self) {
        self.fail_next_render = true;
    }

    pub(crate) fn linefeed(&mut self) {
        if self.height == 0 {
            return;
        }
        if self.cursor_y + 1 < self.height {
            self.cursor_y += 1;
            return;
        }

        // Cursor is on bottom row. Displaced row enters scrollback in order.
        let displaced = self.screen.remove(0);
        self.scrollback.push(displaced);
        self.screen.push(ShadowRow::blank(self.width));
        self.cursor_y = self.height.saturating_sub(1);
    }

    pub(crate) fn write_text(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let sync_begin = SYNC_BEGIN.as_bytes();
        let sync_end = SYNC_END.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(sync_begin) {
                self.sync_output_active = true;
                i += sync_begin.len();
            } else if bytes[i..].starts_with(sync_end) {
                self.sync_output_active = false;
                i += sync_end.len();
            } else if bytes[i] == b'\r' {
                self.cursor_x = 0;
                i += 1;
            } else if bytes[i] == b'\n' {
                self.linefeed();
                i += 1;
            } else {
                let start = i;
                while i < bytes.len() {
                    if bytes[i] == b'\r'
                        || bytes[i] == b'\n'
                        || bytes[i..].starts_with(sync_begin)
                        || bytes[i..].starts_with(sync_end)
                    {
                        break;
                    }
                    i += 1;
                }
                let printable_run = &text[start..i];
                for grapheme in printable_run.graphemes(true) {
                    self.write_grapheme(grapheme);
                }
            }
        }
    }

    pub(crate) fn write_grapheme(&mut self, grapheme: &str) {
        let width = grapheme_cell_width(grapheme);
        if width == 0 {
            return;
        }

        if self.width == 0 || self.height == 0 {
            return;
        }

        if self.cursor_x + width > self.width {
            self.implicit_wraps += 1;
            self.cursor_x = 0;
            self.linefeed();
        }

        let y = self.cursor_y;
        if y >= self.height {
            return;
        }

        // Clear existing glyphs covering target cell span
        for col in self.cursor_x..((self.cursor_x + width).min(self.width)) {
            self.screen[y].clear_glyph_at(col);
        }

        // Write leader
        if self.cursor_x < self.width {
            self.screen[y].cells[self.cursor_x] = ShadowCell {
                grapheme: Some(grapheme.to_string()),
                attrs: self.attrs.clone(),
                continuation: false,
            };
        }

        // Write continuations
        for c in 1..width {
            let col = self.cursor_x + c;
            if col < self.width {
                self.screen[y].cells[col] = ShadowCell {
                    grapheme: None,
                    attrs: self.attrs.clone(),
                    continuation: true,
                };
            }
        }

        self.cursor_x += width;
    }

    pub(crate) fn clear_to_end_of_line(&mut self) {
        if self.cursor_y < self.height && self.width > 0 {
            let row = &mut self.screen[self.cursor_y];
            for col in self.cursor_x..self.width {
                row.clear_glyph_at(col);
                row.cells[col] = ShadowCell {
                    grapheme: None,
                    attrs: self.attrs.clone(),
                    continuation: false,
                };
            }
        }
    }

    pub(crate) fn clear_to_end_of_screen(&mut self) {
        if self.cursor_y < self.height && self.width > 0 {
            let row = &mut self.screen[self.cursor_y];
            for col in self.cursor_x..self.width {
                row.clear_glyph_at(col);
                row.cells[col] = ShadowCell {
                    grapheme: None,
                    attrs: self.attrs.clone(),
                    continuation: false,
                };
            }
            for y in (self.cursor_y + 1)..self.height {
                let row = &mut self.screen[y];
                for col in 0..self.width {
                    row.cells[col] = ShadowCell {
                        grapheme: None,
                        attrs: self.attrs.clone(),
                        continuation: false,
                    };
                }
            }
        }
    }

    pub(crate) fn interpret_change(&mut self, change: &Change) {
        match change {
            Change::CursorPosition { x, y } => {
                match x {
                    Position::Absolute(col) => self.cursor_x = *col,
                    Position::Relative(delta) => {
                        if *delta >= 0 {
                            self.cursor_x = self.cursor_x.saturating_add(*delta as usize);
                        } else {
                            self.cursor_x = self.cursor_x.saturating_sub((-*delta) as usize);
                        }
                    }
                    Position::EndRelative(offset) => {
                        self.cursor_x = self.width.saturating_sub(*offset);
                    }
                }
                match y {
                    Position::Absolute(row) => self.cursor_y = *row,
                    Position::Relative(delta) => {
                        if *delta >= 0 {
                            self.cursor_y = self.cursor_y.saturating_add(*delta as usize);
                        } else {
                            self.cursor_y = self.cursor_y.saturating_sub((-*delta) as usize);
                        }
                    }
                    Position::EndRelative(offset) => {
                        self.cursor_y = self.height.saturating_sub(*offset);
                    }
                }
            }
            Change::AllAttributes(attrs) => {
                self.attrs = attrs.clone();
            }
            Change::Text(text) => {
                self.write_text(text);
            }
            Change::ClearToEndOfLine(_) => {
                self.clear_to_end_of_line();
            }
            Change::ClearToEndOfScreen(_) => {
                self.clear_to_end_of_screen();
            }
            Change::CursorVisibility(vis) => {
                self.cursor_visible = *vis;
            }
            Change::ScrollRegionUp { .. } => {
                panic!(
                    "Change::ScrollRegionUp received in terminal render; ScrollRegionUp is model-only and must never be emitted to the real terminal"
                );
            }
            Change::ScrollRegionDown { .. } => {
                panic!(
                    "Change::ScrollRegionDown received in terminal render; ScrollRegionDown must never be emitted to the real terminal"
                );
            }
            unsupported => {
                panic!("unsupported Change variant in ShadowTerminal: {unsupported:?}");
            }
        }
    }

    pub(crate) fn resize(&mut self, new_width: usize, new_height: usize) {
        if (self.width, self.height) == (new_width, new_height) {
            return;
        }
        self.width = new_width;
        self.height = new_height;

        self.screen.truncate(new_height);
        while self.screen.len() < new_height {
            self.screen.push(ShadowRow::blank(new_width));
        }

        for row in &mut self.screen {
            row.cells.truncate(new_width);
            while row.cells.len() < new_width {
                row.cells.push(ShadowCell::blank());
            }
        }

        self.cursor_x = self.cursor_x.min(new_width.saturating_sub(1));
        self.cursor_y = self.cursor_y.min(new_height.saturating_sub(1));
    }

    pub(crate) fn scrollback_texts(&self) -> Vec<String> {
        self.scrollback.iter().map(|row| row.text()).collect()
    }

    pub(crate) fn scrollback_trimmed_texts(&self) -> Vec<String> {
        self.scrollback
            .iter()
            .map(|row| row.trimmed_text())
            .collect()
    }

    pub(crate) fn screen_texts(&self) -> Vec<String> {
        self.screen.iter().map(|row| row.text()).collect()
    }

    pub(crate) fn screen_trimmed_texts(&self) -> Vec<String> {
        self.screen.iter().map(|row| row.trimmed_text()).collect()
    }

    /// Visible screen as text, analogous to `OpenTUI` `captureCharFrame()`.
    pub(crate) fn capture_char_frame(&self) -> String {
        self.screen_trimmed_texts().join("\n")
    }

    /// Visible screen as styled spans plus cursor, analogous to `OpenTUI` `captureSpans()`.
    pub(crate) fn capture_spans(&self) -> CapturedFrame {
        CapturedFrame {
            cols: self.width,
            rows: self.height,
            cursor: (self.cursor_x, self.cursor_y),
            lines: self.screen.iter().map(capture_line).collect(),
        }
    }

    /// Native scrollback commits since the last claim, analogous to `OpenTUI` `externalOutput.take()`.
    pub(crate) fn claim_scrollback(&mut self) -> Vec<ShadowScrollbackCommit> {
        let commits = self.scrollback[self.claimed_through..]
            .iter()
            .cloned()
            .map(|row| ShadowScrollbackCommit {
                row,
                width: self.width,
                trailing_newline: true,
            })
            .collect();
        self.claimed_through = self.scrollback.len();
        commits
    }

    /// Join newly claimed scrollback rows with newlines, analogous to `externalOutput.takeText()`.
    pub(crate) fn claim_scrollback_text(&mut self) -> String {
        self.claim_scrollback()
            .iter()
            .map(ShadowScrollbackCommit::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn assert_screen_matches_surface(&self, expected: &Surface) {
        assert_eq!(
            (self.width, self.height),
            expected.dimensions(),
            "dimensions mismatch"
        );
        let lines = expected.screen_lines();
        for (y, line) in lines.iter().enumerate() {
            for x in 0..self.width {
                let actual = &self.screen[y].cells[x];
                let expected_cell = line.get_cell(x);
                if actual.continuation {
                    assert!(
                        expected_cell.is_none(),
                        "expected None for continuation cell at ({x}, {y}), got {expected_cell:?}"
                    );
                } else {
                    let expected_cell = expected_cell
                        .unwrap_or_else(|| panic!("expected cell at ({x}, {y}) but got None"));
                    let actual_str = actual.text();
                    let expected_str = expected_cell.str();
                    assert_eq!(
                        actual_str, expected_str,
                        "text mismatch at ({x}, {y}): shadow={actual_str:?}, surface={expected_str:?}"
                    );
                    assert_eq!(
                        actual.attrs,
                        *expected_cell.attrs(),
                        "attrs mismatch at ({x}, {y}): shadow={:?}, surface={:?}",
                        actual.attrs,
                        expected_cell.attrs()
                    );
                }
            }
        }
    }
}

fn capture_line(row: &ShadowRow) -> CapturedLine {
    let mut spans: Vec<CapturedSpan> = Vec::new();
    let mut x = 0;
    while x < row.cells.len() {
        let cell = &row.cells[x];
        if cell.continuation {
            x += 1;
            continue;
        }
        let text = cell.text().to_string();
        let width = cell
            .grapheme
            .as_deref()
            .map_or(1, grapheme_cell_width)
            .max(1);
        if let Some(last) = spans.last_mut()
            && last.attrs == cell.attrs
        {
            last.text.push_str(&text);
            last.width += width;
            x += width;
            continue;
        }
        spans.push(CapturedSpan {
            text,
            attrs: cell.attrs.clone(),
            width,
        });
        x += width;
    }
    CapturedLine { spans }
}

impl Terminal for ShadowTerminal {
    fn set_raw_mode(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn set_cooked_mode(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn enter_alternate_screen(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn exit_alternate_screen(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn get_screen_size(&mut self) -> termwiz::Result<ScreenSize> {
        Ok(ScreenSize {
            rows: self.height,
            cols: self.width,
            xpixel: 0,
            ypixel: 0,
        })
    }

    fn set_screen_size(&mut self, size: ScreenSize) -> termwiz::Result<()> {
        self.resize(size.cols, size.rows);
        Ok(())
    }

    fn render(&mut self, changes: &[Change]) -> termwiz::Result<()> {
        if self.fail_next_render {
            self.fail_next_render = false;
            return Err(std::io::Error::other("shadow terminal render failure").into());
        }
        for change in changes {
            self.interpret_change(change);
        }
        Ok(())
    }

    fn flush(&mut self) -> termwiz::Result<()> {
        Ok(())
    }

    fn poll_input(
        &mut self,
        _wait: Option<Duration>,
    ) -> termwiz::Result<Option<termwiz::input::InputEvent>> {
        Ok(None)
    }

    fn waker(&self) -> TerminalWaker {
        panic!("ShadowTerminal does not support waker")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_crlf_appends_displaced_rows_to_scrollback() {
        let mut shadow = ShadowTerminal::new(4, 3);
        shadow.write_text("1111\r\n");
        shadow.write_text("2222\r\n");
        shadow.write_text("3333\r\n");

        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["1111"]);
        assert_eq!(shadow.screen_trimmed_texts(), vec!["2222", "3333", ""]);

        shadow.write_text("4444\r\n");
        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["1111", "2222"]);
        assert_eq!(shadow.screen_trimmed_texts(), vec!["3333", "4444", ""]);

        shadow.write_text("5555\r\n");
        assert_eq!(
            shadow.scrollback_trimmed_texts(),
            vec!["1111", "2222", "3333"]
        );
        assert_eq!(shadow.screen_trimmed_texts(), vec!["4444", "5555", ""]);
    }

    #[test]
    #[should_panic(expected = "ScrollRegionUp is model-only")]
    fn shadow_terminal_rejects_scroll_region_up_in_real_render() {
        let mut shadow = ShadowTerminal::new(4, 4);
        shadow
            .render(&[Change::ScrollRegionUp {
                first_row: 0,
                region_size: 4,
                scroll_count: 1,
            }])
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "ScrollRegionDown must never be emitted")]
    fn shadow_terminal_rejects_scroll_region_down_in_real_render() {
        let mut shadow = ShadowTerminal::new(4, 4);
        shadow
            .render(&[Change::ScrollRegionDown {
                first_row: 0,
                region_size: 4,
                scroll_count: 1,
            }])
            .unwrap();
    }

    #[test]
    fn shadow_terminal_handles_wide_glyphs_and_continuation_cells() {
        let mut shadow = ShadowTerminal::new(6, 2);
        shadow.write_text("⭐B");
        assert_eq!(shadow.screen[0].cells[0].grapheme.as_deref(), Some("⭐"));
        assert!(!shadow.screen[0].cells[0].continuation);
        assert!(shadow.screen[0].cells[1].continuation);
        assert_eq!(shadow.screen[0].cells[2].grapheme.as_deref(), Some("B"));
        assert!(!shadow.screen[0].cells[2].continuation);
        assert_eq!(shadow.implicit_wraps, 0);
    }

    #[test]
    fn shadow_terminal_simulates_io_render_failure() {
        let mut shadow = ShadowTerminal::new(4, 2);
        shadow.fail_next_render();
        assert!(shadow.render(&[Change::Text("fail".into())]).is_err());
        assert!(shadow.render(&[Change::Text("ok".into())]).is_ok());
    }

    #[test]
    fn shadow_terminal_raw_texts_and_trimmed_texts() {
        let mut shadow = ShadowTerminal::new(4, 2);
        shadow.write_text("hi\r\n");
        shadow.write_text("bye\r\n");
        assert_eq!(shadow.scrollback_texts(), vec!["hi  "]);
        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["hi"]);
        assert_eq!(shadow.screen_texts(), vec!["bye ", "    "]);
        assert_eq!(shadow.screen_trimmed_texts(), vec!["bye", ""]);
    }

    #[test]
    fn capture_char_frame_matches_visible_screen() {
        let mut shadow = ShadowTerminal::new(4, 3);
        shadow.write_text("AB\r\nCD\r\n");
        assert_eq!(shadow.capture_char_frame(), "AB\nCD\n");
        let frame = shadow.capture_spans();
        assert_eq!(frame.cols, 4);
        assert_eq!(frame.rows, 3);
        assert_eq!(frame.cursor, (0, 2));
        assert_eq!(frame.lines[0].spans[0].text.trim_end(), "AB");
    }

    #[test]
    fn claim_scrollback_returns_only_new_native_rows() {
        let mut shadow = ShadowTerminal::new(4, 2);
        shadow.write_text("aa\r\n");
        shadow.write_text("bb\r\n");
        let first = shadow.claim_scrollback();
        assert_eq!(
            first
                .iter()
                .map(ShadowScrollbackCommit::text)
                .collect::<Vec<_>>(),
            vec!["aa"]
        );

        shadow.write_text("cc\r\n");
        assert_eq!(shadow.claim_scrollback_text(), "bb");
        assert_eq!(shadow.claim_scrollback_text(), "");
    }
}
