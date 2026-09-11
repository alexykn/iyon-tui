//! Typed native-control state used by the occurrence acceptance owner.
//!
//! This module contains no N-API values, renderer calls, or terminal I/O. It
//! reuses the existing text editor's Unicode-safe `TextBuffer` and closed
//! `TextInputCommand` implementation rather than maintaining a second editor
//! algorithm. The wire IDs are decoded into those existing commands before a
//! prepared control state is changed; raw IDs and operand vectors are never
//! retained.

use crate::controls::text_input::{TextBuffer, TextInputCommand};

use super::generated::ControlKind;
use super::{config::ControlConfig, generated};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlState {
    Editor(EditorState),
    Scroll(ScrollState),
    Animation(AnimationState),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorState {
    buffer: TextBuffer,
    edit_revision: u64,
    multiline: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollState {
    top_row: u32,
    viewport_rows: u32,
    extent_rows: u32,
    following_end: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationState {
    active_frame: u32,
    frame_count: u32,
    interval_ms: Option<u32>,
    running: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    WrongKind,
    UnknownCommand,
    InvalidOperands,
    InvalidValue,
    StaleEditRevision,
    RevisionExhausted,
}

impl ControlState {
    pub fn new(kind: ControlKind) -> Self {
        Self::new_with_config(kind, ControlConfig::default())
    }

    pub fn new_with_config(kind: ControlKind, config: ControlConfig) -> Self {
        config
            .validate_for(kind)
            .expect("validated control config must match its kind");
        match kind {
            ControlKind::Editor => Self::Editor(EditorState {
                buffer: TextBuffer::new(),
                edit_revision: 0,
                multiline: config.multiline,
            }),
            ControlKind::Scroll => Self::Scroll(ScrollState {
                top_row: 0,
                viewport_rows: 0,
                extent_rows: 0,
                following_end: true,
            }),
            ControlKind::Animation => Self::Animation(AnimationState {
                active_frame: 0,
                frame_count: 0,
                interval_ms: config.animation_interval_ms,
                // AnimationStop is a persistent command; a newly mounted
                // animation is active until that command is accepted.
                running: true,
            }),
        }
    }

    pub fn kind(&self) -> ControlKind {
        match self {
            Self::Editor(_) => ControlKind::Editor,
            Self::Scroll(_) => ControlKind::Scroll,
            Self::Animation(_) => ControlKind::Animation,
        }
    }

    /// Applies one closed command to a prepared state. Operands are checked
    /// here, before native resource state can be installed.
    pub fn apply_command(&mut self, command_id: u32, operands: &[u32]) -> Result<(), ControlError> {
        match self {
            Self::Editor(editor) => editor.apply_command(command_id, operands),
            Self::Scroll(scroll) => scroll.apply_command(command_id, operands),
            Self::Animation(animation) => animation.apply_command(command_id, operands),
        }
    }

    pub fn replace_editor(
        &mut self,
        content: &[u8],
        expected_edit_revision: u64,
    ) -> Result<(), ControlError> {
        let Self::Editor(editor) = self else {
            return Err(ControlError::WrongKind);
        };
        let text = std::str::from_utf8(content).map_err(|_| ControlError::InvalidValue)?;
        if expected_edit_revision != u64::MAX && expected_edit_revision != editor.edit_revision {
            return Err(ControlError::StaleEditRevision);
        }
        editor.edit_revision = editor
            .edit_revision
            .checked_add(1)
            .ok_or(ControlError::RevisionExhausted)?;
        editor.buffer.set_text(text, editor.multiline);
        Ok(())
    }

    pub fn editor_text(&self) -> Option<&str> {
        match self {
            Self::Editor(editor) => Some(editor.buffer.text()),
            _ => None,
        }
    }

    pub fn editor_revision(&self) -> Option<u64> {
        match self {
            Self::Editor(editor) => Some(editor.edit_revision),
            _ => None,
        }
    }

    pub fn editor_multiline(&self) -> Option<bool> {
        match self {
            Self::Editor(editor) => Some(editor.multiline),
            _ => None,
        }
    }

    pub fn scroll_top(&self) -> Option<u32> {
        match self {
            Self::Scroll(scroll) => Some(scroll.top_row),
            _ => None,
        }
    }

    pub fn animation_running(&self) -> Option<bool> {
        match self {
            Self::Animation(animation) => Some(animation.running),
            _ => None,
        }
    }

    pub fn animation_frame_count(&self) -> Option<u32> {
        match self {
            Self::Animation(animation) => Some(animation.frame_count),
            _ => None,
        }
    }

    pub fn animation_active_frame(&self) -> Option<u32> {
        match self {
            Self::Animation(animation) => Some(animation.active_frame),
            _ => None,
        }
    }

    pub fn animation_interval_ms(&self) -> Option<Option<u32>> {
        match self {
            Self::Animation(animation) => Some(animation.interval_ms),
            _ => None,
        }
    }
}

impl EditorState {
    fn apply_command(&mut self, command_id: u32, operands: &[u32]) -> Result<(), ControlError> {
        let command = decode_editor_command(command_id, operands)?;
        let changed = match command {
            TextInputCommand::Insert(character) => self
                .buffer
                .insert_text(&character.to_string(), self.multiline),
            TextInputCommand::Submit => false,
            TextInputCommand::InsertNewline => self.buffer.insert_text("\n", self.multiline),
            TextInputCommand::Backspace => self.buffer.backspace(),
            TextInputCommand::Delete => self.buffer.delete(),
            TextInputCommand::DeleteWordBackward => self.buffer.delete_word_backward(),
            TextInputCommand::DeleteWordForward => self.buffer.delete_word_forward(),
            TextInputCommand::KillToLineStart => self.buffer.kill_to_line_start(),
            TextInputCommand::Yank => self.buffer.yank(),
            TextInputCommand::MoveLeft => self.buffer.move_left(),
            TextInputCommand::MoveRight => self.buffer.move_right(),
            TextInputCommand::MoveWordLeft => self.buffer.move_word_left(),
            TextInputCommand::MoveWordRight => self.buffer.move_word_right(),
            TextInputCommand::MoveLineStart => self.buffer.move_line_start(),
            TextInputCommand::MoveLineEnd => self.buffer.move_line_end(),
            TextInputCommand::MoveUp => {
                let rows = self.buffer.logical_rows();
                self.buffer.move_up_in_rows(&rows)
            }
            TextInputCommand::MoveDown => {
                let rows = self.buffer.logical_rows();
                self.buffer.move_down_in_rows(&rows)
            }
        };
        if changed {
            self.edit_revision = self
                .edit_revision
                .checked_add(1)
                .ok_or(ControlError::RevisionExhausted)?;
        }
        Ok(())
    }
}

fn decode_editor_command(
    command_id: u32,
    operands: &[u32],
) -> Result<TextInputCommand, ControlError> {
    let descriptor =
        generated::control_command_descriptor(command_id).ok_or(ControlError::UnknownCommand)?;
    if descriptor.control_kind != ControlKind::Editor {
        return Err(ControlError::WrongKind);
    }
    if descriptor.operands.len() != operands.len() {
        return Err(ControlError::InvalidOperands);
    }
    Ok(match descriptor.name {
        "EditorInsert" => {
            let [value] = operands else {
                return Err(ControlError::InvalidOperands);
            };
            TextInputCommand::Insert(char::from_u32(*value).ok_or(ControlError::InvalidValue)?)
        }
        "EditorSubmit" => TextInputCommand::Submit,
        "EditorInsertNewline" => TextInputCommand::InsertNewline,
        "EditorBackspace" => TextInputCommand::Backspace,
        "EditorDelete" => TextInputCommand::Delete,
        "EditorDeleteWordBackward" => TextInputCommand::DeleteWordBackward,
        "EditorDeleteWordForward" => TextInputCommand::DeleteWordForward,
        "EditorKillToLineStart" => TextInputCommand::KillToLineStart,
        "EditorYank" => TextInputCommand::Yank,
        "EditorMoveLeft" => TextInputCommand::MoveLeft,
        "EditorMoveRight" => TextInputCommand::MoveRight,
        "EditorMoveWordLeft" => TextInputCommand::MoveWordLeft,
        "EditorMoveWordRight" => TextInputCommand::MoveWordRight,
        "EditorMoveLineStart" => TextInputCommand::MoveLineStart,
        "EditorMoveLineEnd" => TextInputCommand::MoveLineEnd,
        "EditorMoveUp" => TextInputCommand::MoveUp,
        "EditorMoveDown" => TextInputCommand::MoveDown,
        _ => return Err(ControlError::UnknownCommand),
    })
}

impl ScrollState {
    fn apply_command(&mut self, command_id: u32, operands: &[u32]) -> Result<(), ControlError> {
        let descriptor = generated::control_command_descriptor(command_id)
            .ok_or(ControlError::UnknownCommand)?;
        if descriptor.control_kind != ControlKind::Scroll {
            return Err(ControlError::WrongKind);
        }
        if descriptor.operands.len() != operands.len() {
            return Err(ControlError::InvalidOperands);
        }
        let page = self.viewport_rows.max(1);
        match descriptor.name {
            "ScrollLineUp" => self.top_row = self.top_row.saturating_sub(1),
            "ScrollLineDown" => {
                self.top_row = self.max_top().min(self.top_row.saturating_add(1));
            }
            "ScrollPageUp" => self.top_row = self.top_row.saturating_sub(page),
            "ScrollPageDown" => {
                self.top_row = self.max_top().min(self.top_row.saturating_add(page));
            }
            "ScrollStart" => self.top_row = 0,
            "ScrollEnd" => self.top_row = self.max_top(),
            _ => return Err(ControlError::UnknownCommand),
        }
        self.following_end = self.top_row == self.max_top();
        Ok(())
    }

    fn max_top(&self) -> u32 {
        self.extent_rows.saturating_sub(self.viewport_rows)
    }
}

impl AnimationState {
    pub(crate) fn running(&self) -> bool {
        self.running
    }

    pub fn active_frame(&self) -> u32 {
        self.active_frame
    }

    pub fn interval_ms(&self) -> Option<u32> {
        self.interval_ms
    }

    pub(crate) fn set_active_frame(&mut self, frame: usize) -> Result<(), ControlError> {
        self.active_frame = u32::try_from(frame).map_err(|_| ControlError::InvalidValue)?;
        Ok(())
    }

    fn apply_command(&mut self, command_id: u32, operands: &[u32]) -> Result<(), ControlError> {
        let descriptor = generated::control_command_descriptor(command_id)
            .ok_or(ControlError::UnknownCommand)?;
        if descriptor.control_kind != ControlKind::Animation {
            return Err(ControlError::WrongKind);
        }
        if descriptor.operands.len() != operands.len() {
            return Err(ControlError::InvalidOperands);
        }
        match descriptor.name {
            "AnimationStop" => {
                self.running = false;
                Ok(())
            }
            _ => Err(ControlError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(name: &str) -> u32 {
        generated::CONTROL_COMMAND_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.name == name)
            .expect("generated control command")
            .code
    }

    #[test]
    fn editor_commands_reuse_unicode_buffer_and_revision() {
        let mut state = ControlState::new(ControlKind::Editor);
        state
            .apply_command(command("EditorInsert"), &['a' as u32])
            .unwrap();
        state
            .apply_command(command("EditorInsert"), &['😀' as u32])
            .unwrap();
        state
            .apply_command(command("EditorBackspace"), &[])
            .unwrap();
        assert_eq!(state.editor_text(), Some("a"));
        assert_eq!(state.editor_revision(), Some(3));
        assert_eq!(
            state.apply_command(command("EditorInsert"), &[]),
            Err(ControlError::InvalidOperands)
        );
        assert_eq!(
            state.replace_editor(b"new", 2),
            Err(ControlError::StaleEditRevision)
        );
        state.replace_editor(b"new", 3).unwrap();
        assert_eq!(state.editor_text(), Some("new"));
        assert_eq!(state.editor_revision(), Some(4));
    }

    #[test]
    fn scroll_commands_are_closed_and_kind_checked() {
        let mut state = ControlState::new(ControlKind::Scroll);
        state.apply_command(command("ScrollPageDown"), &[]).unwrap();
        assert_eq!(state.scroll_top(), Some(0));
        assert_eq!(
            state.apply_command(command("EditorInsert"), &[]),
            Err(ControlError::WrongKind)
        );
        assert_eq!(
            ControlState::new(ControlKind::Editor).apply_command(command("ScrollEnd"), &[]),
            Err(ControlError::WrongKind)
        );
    }

    #[test]
    fn animation_stop_is_explicit_and_non_generic() {
        let mut state = ControlState::new(ControlKind::Animation);
        assert!(state.animation_running().unwrap());
        state.apply_command(command("AnimationStop"), &[]).unwrap();
        assert!(!state.animation_running().unwrap());
        assert_eq!(
            state.apply_command(command("AnimationStop"), &[1]),
            Err(ControlError::InvalidOperands)
        );
        assert_eq!(
            state.apply_command(command("ScrollEnd"), &[]),
            Err(ControlError::WrongKind)
        );
    }
}
