use super::super::{TextInput, TextInputCommand, command::command_for_key};
use crate::output::OutputQueue;
use crate::{InteractionResult, Key, KeyStroke, Modifiers};

fn key(key: Key) -> KeyStroke {
    KeyStroke::new(key)
}

fn modified(key: Key, modifiers: Modifiers) -> KeyStroke {
    KeyStroke::with_modifiers(key, modifiers)
}

#[test]
fn default_keymap_preserves_framework_keys_and_application_shortcuts() {
    let mut input = TextInput::new().multiline(true);
    input.set_text("text");
    assert!(command_for_key(&input, key(Key::Tab)).is_none());
    assert!(command_for_key(&input, modified(Key::Tab, Modifiers::SHIFT)).is_none());
    assert!(command_for_key(&input, key(Key::Escape)).is_none());
    assert!(command_for_key(&input, modified(Key::Char('c'), Modifiers::CONTROL)).is_none());
    assert_eq!(
        command_for_key(&input, modified(Key::Char('b'), Modifiers::ALT)),
        Some(TextInputCommand::MoveWordLeft)
    );
    let mut start = TextInput::new().multiline(true);
    start.set_text("text");
    start.set_cursor_for_test(0);
    assert_eq!(
        command_for_key(&start, modified(Key::Char('f'), Modifiers::ALT)),
        Some(TextInputCommand::MoveWordRight)
    );
    assert_eq!(
        command_for_key(&input, modified(Key::Char('u'), Modifiers::CONTROL)),
        Some(TextInputCommand::KillToLineStart)
    );
    assert_eq!(
        command_for_key(&input, key(Key::Char('\u{0002}'))),
        Some(TextInputCommand::MoveLeft)
    );
    assert_eq!(
        command_for_key(&input, modified(Key::Char('j'), Modifiers::CONTROL)),
        Some(TextInputCommand::InsertNewline)
    );
    assert!(input.buffer.kill_to_line_start());
    assert_eq!(
        command_for_key(&input, modified(Key::Char('y'), Modifiers::CONTROL)),
        Some(TextInputCommand::Yank)
    );
}

#[test]
fn multiline_enter_and_single_line_enter_have_distinct_meanings() {
    let single = TextInput::new();
    assert_eq!(
        command_for_key(&single, modified(Key::Enter, Modifiers::SHIFT)),
        None
    );
    assert_eq!(
        command_for_key(&single, key(Key::Enter)),
        Some(TextInputCommand::Submit)
    );

    let multi = TextInput::new().multiline(true);
    assert_eq!(
        command_for_key(&multi, key(Key::Enter)),
        Some(TextInputCommand::Submit)
    );
    assert_eq!(
        command_for_key(&multi, modified(Key::Enter, Modifiers::SHIFT)),
        Some(TextInputCommand::InsertNewline)
    );

    for stroke in [
        modified(Key::Char('j'), Modifiers::CONTROL),
        modified(Key::Char('m'), Modifiers::CONTROL),
        key(Key::Char('\n')),
        key(Key::Char('\r')),
    ] {
        assert_eq!(command_for_key(&single, stroke), None);
        assert_eq!(
            command_for_key(&multi, stroke),
            Some(TextInputCommand::InsertNewline)
        );
    }
}

#[test]
fn altgr_printable_characters_insert_without_becoming_control_commands() {
    let input = TextInput::new();
    assert_eq!(
        command_for_key(
            &input,
            modified(Key::Char('€'), Modifiers::CONTROL | Modifiers::ALT),
        ),
        Some(TextInputCommand::Insert('€'))
    );
    assert!(command_for_key(&input, modified(Key::Char('x'), Modifiers::SUPER)).is_none());
}

#[test]
fn command_execution_consumes_real_edits_but_not_boundary_noops() {
    let mut input = TextInput::new();
    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        assert_eq!(
            super::super::command::handle_command(&mut input, TextInputCommand::MoveLeft, &mut cx,),
            InteractionResult::Ignored
        );
        assert_eq!(
            super::super::command::handle_command(
                &mut input,
                TextInputCommand::Insert('a'),
                &mut cx,
            ),
            InteractionResult::Consumed
        );
    }
    assert_eq!(input.text(), "a");
    assert!(queue.is_empty());
}
