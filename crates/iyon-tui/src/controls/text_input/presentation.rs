//! Immutable editor capture used by the direct occurrence presenter.

use crate::component::EditorSnapshot;

use super::TextInput;

impl TextInput {
    pub(crate) fn frame_snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            text: self.buffer.text().to_owned(),
            cursor_bytes: self.buffer.cursor_bytes(),
            focused: self.focused,
            multiline: self.multiline,
            scroll_row: self.scroll_row,
            border: self.border.clone(),
        }
    }
}
