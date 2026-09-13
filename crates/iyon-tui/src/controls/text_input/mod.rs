//! A backend-neutral, stateful Unicode text editor component.
//!
//! [`TextInput`] owns editing mechanics and emits semantic outputs. It does
//! not know about application submission policy, terminal backends, or
//! geometry.

mod buffer;
pub(crate) mod command;
mod cursor;
mod edit;
mod output;
mod presentation;

#[cfg(test)]
mod tests;

pub(crate) use buffer::TextBuffer;
pub(crate) use command::TextInputCommand;

use crate::{
    BorderSpec, Component, ComponentCx, EventCx, InteractionResult, Output, geometry::Size,
    presentation::wrap::input_wrap_ranges,
};

use self::{
    command::{apply_buffer_command_to_buffer, command_for_key, handle_command},
    output::ChangeOutputs,
};

pub use output::TextChange;

/// Exact post-input text/cursor facts prepared without mutating the live
/// editor.  The host uses this small editor-local value to admit its native
/// event batch before applying input; it never clones the owning runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextInputPreview {
    pub(crate) text: String,
    pub(crate) cursor_bytes: usize,
}

/// Retained generic Unicode text editor state.
///
/// `TextInput` is intentionally constructed explicitly. Its output channels
/// are stable for the lifetime of the component and are not duplicated by
/// cloning, so this type does not implement [`Clone`].
pub struct TextInput {
    buffer: TextBuffer,
    multiline: bool,
    focused: bool,
    submitted: Output<String>,
    change_outputs: ChangeOutputs,
    layout_size: Option<Size>,
    scroll_row: usize,
    border: Option<BorderSpec>,
}

impl TextInput {
    /// Creates an empty single-line input with its own submission channel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: TextBuffer::new(),
            multiline: false,
            focused: false,
            submitted: Output::new(),
            change_outputs: ChangeOutputs::default(),
            layout_size: None,
            scroll_row: 0,
            border: None,
        }
    }

    /// Returns a copy configured for single- or multi-line editing.
    #[must_use]
    pub fn multiline(mut self, enabled: bool) -> Self {
        self.set_multiline(enabled);
        self
    }

    /// Sets the semantic border used by the input surface.
    #[must_use]
    pub fn border(mut self, border: BorderSpec) -> Self {
        self.border = Some(border);
        self
    }

    pub(crate) fn set_border(&mut self, border: BorderSpec) {
        self.border = Some(border);
    }

    /// Changes the line policy without emitting a user change event.
    pub fn set_multiline(&mut self, enabled: bool) {
        if self.multiline == enabled {
            return;
        }
        self.multiline = enabled;
        self.buffer.recanonicalize(enabled);
        self.repair_scroll();
    }

    #[must_use]
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.text().is_empty()
    }

    #[must_use]
    pub fn cursor_bytes(&self) -> usize {
        self.buffer.cursor_bytes()
    }

    pub(crate) fn preview_key(&self, key: crate::KeyStroke) -> Option<TextInputPreview> {
        let command = command_for_key(self, key)?;
        let mut buffer = self.buffer.clone();
        apply_buffer_command_to_buffer(
            &mut buffer,
            command,
            self.multiline,
            self.command_layout_width(),
        );
        Some(TextInputPreview {
            text: buffer.text().to_owned(),
            cursor_bytes: buffer.cursor_bytes(),
        })
    }

    pub(crate) fn is_submit_key(&self, key: crate::KeyStroke) -> bool {
        matches!(command_for_key(self, key), Some(TextInputCommand::Submit))
    }

    pub(crate) fn preview_paste(&self, text: &str) -> TextInputPreview {
        let mut buffer = self.buffer.clone();
        buffer.insert_text(text, self.multiline);
        TextInputPreview {
            text: buffer.text().to_owned(),
            cursor_bytes: buffer.cursor_bytes(),
        }
    }

    /// Replaces the canonical text and moves the cursor to its end.
    pub fn set_text(&mut self, text: impl AsRef<str>) {
        self.buffer.set_text(text, self.multiline);
        self.repair_scroll();
    }

    /// Clears text and editor-local kill state without emitting a change.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.repair_scroll();
    }

    /// Returns this input's stable submission channel.
    #[must_use]
    pub fn submitted(&self) -> Output<String> {
        self.submitted
    }

    /// Registers an ordered synchronous projection of user text changes.
    pub fn output_on_change<R: Send + 'static>(
        &mut self,
        project: impl for<'change> Fn(TextChange<'change>) -> R + Send + 'static,
    ) -> Output<R> {
        self.change_outputs.register(project)
    }

    fn insert_text(&mut self, text: &str) -> bool {
        let changed = self.buffer.insert_text(text, self.multiline);
        if changed {
            self.repair_scroll();
        }
        changed
    }

    fn emit_change(&self, cx: &mut EventCx<'_>) {
        self.change_outputs.emit(&self.buffer, cx);
    }

    fn can_execute(&self, command: TextInputCommand) -> bool {
        match command {
            TextInputCommand::Insert(_) | TextInputCommand::InsertNewline => true,
            TextInputCommand::Submit => true,
            TextInputCommand::Backspace => self.cursor_bytes() > 0,
            TextInputCommand::Delete => self.cursor_bytes() < self.text().len(),
            TextInputCommand::DeleteWordBackward => self.cursor_bytes() > 0,
            TextInputCommand::DeleteWordForward => self.cursor_bytes() < self.text().len(),
            TextInputCommand::KillToLineStart => self.cursor_bytes() > 0,
            TextInputCommand::Yank => self.buffer.has_kill_buffer(),
            TextInputCommand::MoveLeft => self.cursor_bytes() > 0,
            TextInputCommand::MoveRight => self.cursor_bytes() < self.text().len(),
            TextInputCommand::MoveWordLeft => self.cursor_bytes() > 0,
            TextInputCommand::MoveWordRight => self.cursor_bytes() < self.text().len(),
            TextInputCommand::MoveLineStart => self.cursor_bytes() != self.line_start(),
            TextInputCommand::MoveLineEnd => self.cursor_bytes() != self.line_end(),
            TextInputCommand::MoveUp | TextInputCommand::MoveDown => true,
        }
    }

    fn line_start(&self) -> usize {
        self.text()[..self.cursor_bytes()]
            .rfind('\n')
            .map_or(0, |index| index + 1)
    }

    fn line_end(&self) -> usize {
        self.text()[self.cursor_bytes()..]
            .find('\n')
            .map_or(self.text().len(), |index| self.cursor_bytes() + index)
    }

    fn focus_changed(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn handle_paste(&mut self, text: &str, cx: &mut EventCx<'_>) -> InteractionResult {
        if !self.insert_text(text) {
            return InteractionResult::Ignored;
        }
        self.emit_change(cx);
        InteractionResult::Consumed
    }

    pub(crate) fn command_for_key(input: &Self, key: crate::KeyStroke) -> Option<TextInputCommand> {
        command_for_key(input, key)
    }

    pub(crate) fn handle_command(
        input: &mut Self,
        command: TextInputCommand,
        cx: &mut EventCx<'_>,
    ) -> InteractionResult {
        let result = handle_command(input, command, cx);
        input.repair_scroll();
        result
    }

    pub(crate) fn layout_changed(input: &mut Self, size: Size) {
        input.layout_size = Some(size);
        input.repair_scroll();
    }

    fn inner_size(&self, size: Size) -> Size {
        let Some(border) = &self.border else {
            return size;
        };
        Size::new(
            size.width
                .saturating_sub(border.left_width().saturating_add(border.right_width())),
            size.height
                .saturating_sub(border.top_height().saturating_add(border.bottom_height())),
        )
    }

    pub(crate) fn command_layout_width(&self) -> Option<u16> {
        self.layout_size.map(|size| self.inner_size(size).width)
    }

    fn repair_scroll(&mut self) {
        let Some(layout_size) = self.layout_size else {
            return;
        };
        let size = self.inner_size(layout_size);
        if size.width == 0 || size.height == 0 {
            self.scroll_row = 0;
            return;
        }
        let ranges = input_wrap_ranges(self.text(), size.width);
        let visible = usize::from(size.height).max(1);
        let max_scroll = ranges.len().saturating_sub(visible);
        let cursor = self.cursor_bytes().min(self.text().len());
        let cursor_row = cursor::wrapped_line_index_by_start(&ranges, cursor).unwrap_or(0);
        let mut scroll = self.scroll_row.min(max_scroll);
        if cursor_row < scroll {
            scroll = cursor_row;
        }
        if cursor_row >= scroll.saturating_add(visible) {
            scroll = cursor_row + 1 - visible;
        }
        self.scroll_row = scroll.min(max_scroll);
    }

    pub(crate) fn focus_changed_callback(input: &mut Self, focused: bool) {
        input.focus_changed(focused);
    }

    pub(crate) fn paste_callback(
        input: &mut Self,
        text: &str,
        cx: &mut EventCx<'_>,
    ) -> InteractionResult {
        input.handle_paste(text, cx)
    }

    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: usize) {
        self.buffer.set_cursor(cursor);
    }
}

impl Component for TextInput {
    fn control_snapshot(&self) -> Option<crate::component::ControlSnapshot> {
        Some(crate::component::ControlSnapshot::Editor(Box::new(
            self.frame_snapshot(),
        )))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_focus_changed(Self::focus_changed_callback);
        cx.key_commands(Self::command_for_key, Self::handle_command);
        cx.on_paste(Self::paste_callback);
        cx.on_layout_changed(Self::layout_changed);
    }
}
