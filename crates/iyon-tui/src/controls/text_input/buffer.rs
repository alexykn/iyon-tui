use std::ops::Range;

use super::{
    cursor::{
        cursor_for_display_col, display_col_at, logical_line_ranges, wrapped_line_index_by_start,
    },
    edit::{canonicalize, is_separator},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub(super) struct TextBuffer {
    text: String,
    cursor: usize,
    preferred_col: Option<usize>,
    kill_buffer: String,
}

impl TextBuffer {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            preferred_col: None,
            kill_buffer: String::new(),
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn cursor_bytes(&self) -> usize {
        self.cursor
    }

    pub(super) fn set_text(&mut self, text: impl AsRef<str>, multiline: bool) {
        self.text = canonicalize(text.as_ref(), multiline);
        self.cursor = self.text.len();
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(super) fn recanonicalize(&mut self, multiline: bool) {
        let cursor = self.cursor;
        self.text = canonicalize(&self.text, multiline);
        self.cursor = 0;
        self.set_cursor(cursor.min(self.text.len()));
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.preferred_col = None;
        self.kill_buffer.clear();
        self.assert_invariant();
    }

    pub(super) fn insert_text(&mut self, text: &str, multiline: bool) -> bool {
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

    pub(super) fn backspace(&mut self) -> bool {
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

    pub(super) fn delete(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_boundary(self.cursor);
        self.text.drain(self.cursor..next);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(super) fn delete_word_backward(&mut self) -> bool {
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

    pub(super) fn delete_word_forward(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_word_start(self.cursor);
        self.text.drain(self.cursor..next);
        self.preferred_col = None;
        self.assert_invariant();
        true
    }

    pub(super) fn kill_to_line_start(&mut self) -> bool {
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

    pub(super) fn yank(&mut self) -> bool {
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

    pub(super) fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.set_cursor(self.previous_boundary(self.cursor));
        self.preferred_col = None;
        true
    }

    pub(super) fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.set_cursor(self.next_boundary(self.cursor));
        self.preferred_col = None;
        true
    }

    pub(super) fn move_word_left(&mut self) -> bool {
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

    pub(super) fn move_word_right(&mut self) -> bool {
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

    pub(super) fn move_line_start(&mut self) -> bool {
        let next = self.line_start(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(super) fn move_line_end(&mut self) -> bool {
        let next = self.line_end(self.cursor);
        if next == self.cursor {
            return false;
        }
        self.set_cursor(next);
        self.preferred_col = None;
        true
    }

    pub(super) fn move_up_in_rows(&mut self, rows: &[Range<usize>]) -> bool {
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

    pub(super) fn move_down_in_rows(&mut self, rows: &[Range<usize>]) -> bool {
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

    pub(super) fn has_kill_buffer(&self) -> bool {
        !self.kill_buffer.is_empty()
    }

    pub(super) fn logical_rows(&self) -> Vec<Range<usize>> {
        logical_line_ranges(&self.text)
    }

    fn set_cursor_if_changed(&mut self, position: usize) -> bool {
        let previous = self.cursor;
        self.set_cursor(position);
        self.cursor != previous
    }

    pub(super) fn set_cursor(&mut self, position: usize) {
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
