//! Immutable editor capture used by the direct occurrence presenter.

use crate::component::EditorSnapshot;

use super::TextInput;

impl TextInput {
    pub(crate) fn frame_snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: self.buffer.text().to_owned().into(),
            cursor_bytes: self.buffer.cursor_bytes(),
            focused: self.focused,
            multiline: self.multiline,
            scroll_row: self.scroll_row,
            border: self.border.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_capture_contains_only_concrete_frame_facts() {
        let mut input = TextInput::new().multiline(true);
        input.set_text("hello\nworld");
        let capture = input.frame_snapshot();

        assert_eq!(capture.text, "hello\nworld".into());
        assert_eq!(capture.cursor_bytes, "hello\nworld".len());
        assert!(capture.multiline);
        assert!(!capture.focused);
        assert_eq!(capture.scroll_row, 0);
        assert!(capture.border.is_none());
    }
}
