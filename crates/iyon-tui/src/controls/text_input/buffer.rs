use std::ops::Range;

use super::{
    cursor::{
        cursor_for_display_col, display_col_at, logical_line_ranges, wrapped_line_index_by_start,
    },
    edit::{canonicalize, is_separator},
};
use unicode_segmentation::UnicodeSegmentation;

use super::command::TextInputCommand;
use crate::presentation::wrap::input_wrap_ranges;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextBuffer {
    text: String,
    cursor: usize,
    preferred_col: Option<usize>,
    kill_buffer: String,
}

impl TextBuffer {
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            preferred_col: None,
            kill_buffer: String::new(),
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn cursor_bytes(&self) -> usize {
        self.cursor
    }

    pub(crate) fn set_text(&mut self, text: impl AsRef<str>, multiline: bool) {
        self.text = canonicalize(text.as_ref(), multiline);
        self.cursor = self.text.len();
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(crate) fn recanonicalize(&mut self, multiline: bool) {
        let cursor = self.cursor;
        self.text = canonicalize(&self.text, multiline);
        self.cursor = 0;
        self.set_cursor(cursor.min(self.text.len()));
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(crate) fn insert_text(&mut self, text: &str, multiline: bool) -> bool {
        let text = canonicalize(text, multiline);
        if text.is_empty() {
            return false;
        }
        self.text.insert_str(self.cursor, &text);
        self.set_cursor(self.cursor + text.len());
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let previous = self.previous_boundary(self.cursor);
        self.text.drain(previous..self.cursor);
        self.set_cursor(previous);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn delete(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_boundary(self.cursor);
        self.text.drain(self.cursor..next);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn delete_word_backward(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let previous = self.previous_word_start(self.cursor);
        self.text.drain(previous..self.cursor);
        self.set_cursor(previous);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn delete_word_forward(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_word_start(self.cursor);
        self.text.drain(self.cursor..next);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn kill_to_line_start(&mut self) -> bool {
        let start = self.line_start(self.cursor);
        if self.cursor == start {
            if self.cursor == 0 {
                return false;
            }
            let previous_end = self.previous_line_end(self.cursor);
            self.text.drain(previous_end..self.cursor);
            self.set_cursor(previous_end);
            self.preferred_col = None;
            self.assert_invariant();
            return true;
        }

        self.kill_buffer = self.text[start..self.cursor].to_string();
        self.text.drain(start..self.cursor);
        self.set_cursor(start);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn yank(&mut self) -> bool {
        if self.kill_buffer.is_empty() {
            return false;
        }
        let kill = self.kill_buffer.clone();
        self.text.insert_str(self.cursor, &kill);
        self.set_cursor(self.cursor + kill.len());
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(crate) fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.set_cursor(self.previous_boundary(self.cursor));
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.set_cursor(self.next_boundary(self.cursor));
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_word_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let next = self.previous_word_start(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_word_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_word_start(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_line_start(&mut self) -> bool {
        let next = self.line_start(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_line_end(&mut self) -> bool {
        let next = self.line_end(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(crate) fn move_up_in_rows(&mut self, rows: &[Range<usize>]) -> bool {
        let Some(index) = wrapped_line_index_by_start(rows, self.cursor) else {
            return false;
        };
        if index == 0 {
            return self.set_cursor_if_changed(0);
        }
        let current = &rows[index];
        let target = self
            .preferred_col
            .unwrap_or_else(|| display_col_at(&self.text, current.start, current.end, self.cursor));
        self.preferred_col = Some(target);
        let previous = &rows[index - 1];
        let next = cursor_for_display_col(&self.text, previous.start, previous.end, target);
        self.set_cursor_if_changed(next)
    }

    pub(crate) fn move_down_in_rows(&mut self, rows: &[Range<usize>]) -> bool {
        let Some(index) = wrapped_line_index_by_start(rows, self.cursor) else {
            return false;
        };
        if index + 1 >= rows.len() {
            return self.set_cursor_if_changed(self.text.len());
        }
        let current = &rows[index];
        let target = self
            .preferred_col
            .unwrap_or_else(|| display_col_at(&self.text, current.start, current.end, self.cursor));
        self.preferred_col = Some(target);
        let next_row = &rows[index + 1];
        let next = cursor_for_display_col(&self.text, next_row.start, next_row.end, target);
        self.set_cursor_if_changed(next)
    }

    pub(crate) fn has_kill_buffer(&self) -> bool {
        !self.kill_buffer.is_empty()
    }

    pub(crate) fn logical_rows(&self) -> Vec<Range<usize>> {
        logical_line_ranges(&self.text)
    }

    /// Applies one already-decoded editor command to the buffer. This is the
    /// authoritative command operation shared by live dispatch and the
    /// editor-local admission preview; callers own submit/output/scroll side
    /// effects.
    pub(crate) fn apply_command(
        &mut self,
        command: TextInputCommand,
        multiline: bool,
        layout_width: Option<u16>,
    ) -> bool {
        match command {
            TextInputCommand::Insert(character) => {
                self.insert_text(&character.to_string(), multiline)
            }
            TextInputCommand::Submit => false,
            TextInputCommand::InsertNewline => self.insert_text("\n", multiline),
            TextInputCommand::Backspace => self.backspace(),
            TextInputCommand::Delete => self.delete(),
            TextInputCommand::DeleteWordBackward => self.delete_word_backward(),
            TextInputCommand::DeleteWordForward => self.delete_word_forward(),
            TextInputCommand::KillToLineStart => self.kill_to_line_start(),
            TextInputCommand::Yank => self.yank(),
            TextInputCommand::MoveLeft => self.move_left(),
            TextInputCommand::MoveRight => self.move_right(),
            TextInputCommand::MoveWordLeft => self.move_word_left(),
            TextInputCommand::MoveWordRight => self.move_word_right(),
            TextInputCommand::MoveLineStart => self.move_line_start(),
            TextInputCommand::MoveLineEnd => self.move_line_end(),
            TextInputCommand::MoveUp => {
                let rows = layout_width.map_or_else(
                    || self.logical_rows(),
                    |width| input_wrap_ranges(self.text(), width),
                );
                self.move_up_in_rows(&rows)
            }
            TextInputCommand::MoveDown => {
                let rows = layout_width.map_or_else(
                    || self.logical_rows(),
                    |width| input_wrap_ranges(self.text(), width),
                );
                self.move_down_in_rows(&rows)
            }
        }
    }

    fn set_cursor_if_changed(&mut self, position: usize) -> bool {
        let previous = self.cursor;
        self.set_cursor(position);
        self.cursor != previous
    }

    pub(crate) fn set_cursor(&mut self, position: usize) {
        let position = position.min(self.text.len());
        let position = if self.text.is_char_boundary(position) {
            position
        } else {
            let mut position = position;
            while position > 0 && !self.text.is_char_boundary(position) {
                position -= 1;
            }
            position
        };
        self.cursor = position;
    }

    fn previous_boundary(&self, position: usize) -> usize {
        self.text[..position]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(start, _)| start)
    }

    fn next_boundary(&self, position: usize) -> usize {
        self.text[position..]
            .grapheme_indices(true)
            .next()
            .map_or(position, |(_, grapheme)| position + grapheme.len())
    }

    fn line_start(&self, position: usize) -> usize {
        self.text[..position]
            .rfind('\n')
            .map_or(0, |index| index + 1)
    }

    fn line_end(&self, position: usize) -> usize {
        self.text[position..]
            .find('\n')
            .map_or(self.text.len(), |index| position + index)
    }

    fn previous_line_end(&self, position: usize) -> usize {
        self.text[..position].rfind('\n').unwrap_or(position)
    }

    fn previous_word_start(&self, mut position: usize) -> usize {
        while position > 0 {
            let previous = self.previous_boundary(position);
            let grapheme = &self.text[previous..position];
            let character = grapheme.chars().next().expect("grapheme is nonempty");
            if !character.is_whitespace() {
                break;
            }
            position = previous;
        }
        if position == 0 {
            return 0;
        }

        let previous = self.previous_boundary(position);
        let initial = self.text[previous..position]
            .chars()
            .next()
            .expect("grapheme is nonempty");
        let target_separator = is_separator(initial) && !initial.is_whitespace();
        while position > 0 {
            let previous = self.previous_boundary(position);
            let character = self.text[previous..position]
                .chars()
                .next()
                .expect("grapheme is nonempty");
            if character.is_whitespace() {
                break;
            }
            let separator = is_separator(character) && !character.is_whitespace();
            if separator != target_separator {
                break;
            }
            position = previous;
        }
        position
    }

    fn next_word_start(&self, mut position: usize) -> usize {
        if position >= self.text.len() {
            return self.text.len();
        }
        let next = self.next_boundary(position);
        let first = self.text[position..next]
            .chars()
            .next()
            .expect("grapheme is nonempty");
        if first.is_whitespace() {
            while position < self.text.len() {
                let next = self.next_boundary(position);
                let character = self.text[position..next]
                    .chars()
                    .next()
                    .expect("grapheme is nonempty");
                if !character.is_whitespace() {
                    break;
                }
                position = next;
            }
            return position;
        }

        let target_separator = is_separator(first) && !first.is_whitespace();
        while position < self.text.len() {
            let next = self.next_boundary(position);
            let character = self.text[position..next]
                .chars()
                .next()
                .expect("grapheme is nonempty");
            if character.is_whitespace() {
                break;
            }
            let separator = is_separator(character) && !character.is_whitespace();
            if separator != target_separator {
                break;
            }
            position = next;
        }
        while position < self.text.len() {
            let next = self.next_boundary(position);
            let character = self.text[position..next]
                .chars()
                .next()
                .expect("grapheme is nonempty");
            if !character.is_whitespace() {
                break;
            }
            position = next;
        }
        position
    }

    pub(super) fn assert_invariant(&self) {
        assert!(self.cursor <= self.text.len());
        assert!(self.text.is_char_boundary(self.cursor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_is_deterministic() {
        assert_eq!(canonicalize("a\r\nb\tc", true), "a\nb    c");
        assert_eq!(canonicalize("a\rb\n\u{0001}", false), "a b \u{0001}");
    }

    #[test]
    fn cursor_preserves_char_boundary_after_zwj_merge() {
        let mut buffer = TextBuffer::new();
        buffer.set_text("👩💻", true);
        buffer.set_cursor("👩".len());
        assert!(buffer.insert_text("\u{200d}", true));
        assert_eq!(buffer.text(), "👩\u{200d}💻");
        assert_eq!(buffer.cursor_bytes(), "👩\u{200d}".len());
        buffer.assert_invariant();
    }

    #[test]
    fn forward_deletion_can_merge_regional_indicators_without_invalidating_cursor() {
        let mut buffer = TextBuffer::new();
        buffer.set_text("🇺x🇸", true);
        buffer.set_cursor("🇺".len());
        assert!(buffer.delete());
        assert_eq!(buffer.text(), "🇺🇸");
        assert_eq!(buffer.cursor_bytes(), "🇺".len());
        buffer.assert_invariant();
    }
}
