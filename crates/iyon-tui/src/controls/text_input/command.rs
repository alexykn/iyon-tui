use crate::{EventCx, InteractionResult, Key, KeyStroke, Modifiers};

use super::{TextBuffer, TextInput};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextInputCommand {
    Insert(char),
    Submit,
    InsertNewline,
    Backspace,
    Delete,
    DeleteWordBackward,
    DeleteWordForward,
    KillToLineStart,
    Yank,
    MoveLeft,
    MoveRight,
    MoveWordLeft,
    MoveWordRight,
    MoveLineStart,
    MoveLineEnd,
    MoveUp,
    MoveDown,
}

pub(crate) fn command_for_key(input: &TextInput, stroke: KeyStroke) -> Option<TextInputCommand> {
    let key = stroke.key();
    let modifiers = stroke.modifiers();
    let none = Modifiers::NONE;

    let command = match key {
        Key::Char('\u{0002}') if modifiers == none => TextInputCommand::MoveLeft,
        Key::Char('\u{0006}') if modifiers == none => TextInputCommand::MoveRight,
        Key::Char('\u{0010}') if modifiers == none => TextInputCommand::MoveUp,
        Key::Char('\u{000e}') if modifiers == none => TextInputCommand::MoveDown,
        Key::Char('\n' | '\r')
            if input.is_multiline() && (modifiers == none || modifiers == Modifiers::CONTROL) =>
        {
            TextInputCommand::InsertNewline
        }
        Key::Char('j' | 'm') if modifiers == Modifiers::CONTROL && input.is_multiline() => {
            TextInputCommand::InsertNewline
        }
        Key::Left if modifiers == none => TextInputCommand::MoveLeft,
        Key::Right if modifiers == none => TextInputCommand::MoveRight,
        Key::Left if is_word_modifier(modifiers) => TextInputCommand::MoveWordLeft,
        Key::Right if is_word_modifier(modifiers) => TextInputCommand::MoveWordRight,
        Key::Char('b') if modifiers == Modifiers::ALT => TextInputCommand::MoveWordLeft,
        Key::Char('f') if modifiers == Modifiers::ALT => TextInputCommand::MoveWordRight,
        Key::Home if modifiers == none => TextInputCommand::MoveLineStart,
        Key::End if modifiers == none => TextInputCommand::MoveLineEnd,
        Key::Char('a') if modifiers == Modifiers::CONTROL => TextInputCommand::MoveLineStart,
        Key::Char('e') if modifiers == Modifiers::CONTROL => TextInputCommand::MoveLineEnd,
        Key::Up if modifiers == none => TextInputCommand::MoveUp,
        Key::Down if modifiers == none => TextInputCommand::MoveDown,
        Key::Char('p') if modifiers == Modifiers::CONTROL => TextInputCommand::MoveUp,
        Key::Char('n') if modifiers == Modifiers::CONTROL => TextInputCommand::MoveDown,
        Key::Backspace if modifiers == none => TextInputCommand::Backspace,
        Key::Char('h') if modifiers == Modifiers::CONTROL => TextInputCommand::Backspace,
        Key::Backspace if is_word_modifier(modifiers) => TextInputCommand::DeleteWordBackward,
        Key::Delete if modifiers == none => TextInputCommand::Delete,
        Key::Delete if is_word_modifier(modifiers) => TextInputCommand::DeleteWordForward,
        Key::Char('u') if modifiers == Modifiers::CONTROL => TextInputCommand::KillToLineStart,
        Key::Char('y') if modifiers == Modifiers::CONTROL => TextInputCommand::Yank,
        Key::Enter if modifiers == none => TextInputCommand::Submit,
        Key::Enter if modifiers == Modifiers::SHIFT && input.is_multiline() => {
            TextInputCommand::InsertNewline
        }
        Key::Char(character) if can_insert_character(character, modifiers) => {
            TextInputCommand::Insert(character)
        }
        _ => return None,
    };

    if input.can_execute(command) {
        Some(command)
    } else {
        None
    }
}

pub(crate) fn handle_command(
    input: &mut TextInput,
    command: TextInputCommand,
    cx: &mut EventCx<'_>,
) -> InteractionResult {
    if command == TextInputCommand::Submit {
        cx.emit(input.submitted, input.buffer.text().to_owned());
        return InteractionResult::Consumed;
    }
    let changed = apply_buffer_command(input, command);

    if changed {
        input.emit_change(cx);
        InteractionResult::Consumed
    } else {
        InteractionResult::Ignored
    }
}

pub(crate) fn apply_buffer_command(input: &mut TextInput, command: TextInputCommand) -> bool {
    let layout_width = input.command_layout_width();
    let changed =
        apply_buffer_command_to_buffer(&mut input.buffer, command, input.multiline, layout_width);
    input.repair_scroll();
    changed
}

pub(crate) fn apply_buffer_command_to_buffer(
    buffer: &mut TextBuffer,
    command: TextInputCommand,
    multiline: bool,
    layout_width: Option<u16>,
) -> bool {
    buffer.apply_command(command, multiline, layout_width)
}

fn is_word_modifier(modifiers: Modifiers) -> bool {
    modifiers == Modifiers::CONTROL || modifiers == Modifiers::ALT
}

fn can_insert_character(character: char, modifiers: Modifiers) -> bool {
    if character.is_control() {
        return false;
    }
    if modifiers.contains(Modifiers::SUPER)
        || modifiers.contains(Modifiers::HYPER)
        || modifiers.contains(Modifiers::META)
    {
        return false;
    }
    if modifiers.contains(Modifiers::CONTROL) {
        return modifiers.contains(Modifiers::ALT);
    }
    true
}
