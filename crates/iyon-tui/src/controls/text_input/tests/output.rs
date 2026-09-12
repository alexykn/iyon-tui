use super::super::{TextInput, TextInputCommand, command::handle_command};
use crate::InteractionResult;
use crate::output::{OutputQueue, OutputRouter};

#[derive(Debug, PartialEq, Eq)]
struct SmallProjection {
    length: usize,
    cursor: usize,
}

#[test]
fn submission_is_stable_owned_and_does_not_clear_or_emit_change() {
    let mut input = TextInput::new();
    let submitted = input.submitted();
    let changed = input.output_on_change(|change| change.text().to_owned());
    assert_eq!(submitted, input.submitted());
    assert_ne!(submitted, TextInput::new().submitted());

    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        assert_eq!(
            handle_command(&mut input, TextInputCommand::Insert('x'), &mut cx),
            InteractionResult::Consumed
        );
        assert_eq!(
            handle_command(&mut input, TextInputCommand::Submit, &mut cx),
            InteractionResult::Consumed
        );
    }

    let mut router = OutputRouter::<String>::new();
    router
        .route(submitted, |text| format!("submitted:{text}"))
        .unwrap();
    router
        .route(changed, |text| format!("changed:{text}"))
        .unwrap();
    assert_eq!(
        router.drain(&mut queue).unwrap(),
        vec!["changed:x", "submitted:x"]
    );
    assert_eq!(input.text(), "x");
}

#[test]
fn empty_submit_is_a_valid_event_without_a_change_projection() {
    let mut input = TextInput::new();
    let submitted = input.submitted();
    let changed = input.output_on_change(|change| change.text().to_owned());
    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        assert_eq!(
            handle_command(&mut input, TextInputCommand::Submit, &mut cx),
            InteractionResult::Consumed
        );
        input.focus_changed(true);
    }
    let mut router = OutputRouter::<String>::new();
    router.route(submitted, |text| text).unwrap();
    router
        .route(changed, |text| format!("changed:{text}"))
        .unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec![String::new()]);
    assert!(input.is_empty());
}

#[test]
fn change_projectors_are_borrowed_ordered_and_can_return_non_clone_values() {
    let mut input = TextInput::new();
    let projected = input.output_on_change(|change| SmallProjection {
        length: change.text().len(),
        cursor: change.cursor_bytes(),
    });
    let length = input.output_on_change(|change| change.text().chars().count());
    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        handle_command(&mut input, TextInputCommand::Insert('😀'), &mut cx);
    }

    let mut router = OutputRouter::<String>::new();
    router
        .route(projected, |value| {
            format!("{}:{}", value.length, value.cursor)
        })
        .unwrap();
    router
        .route(length, |value| format!("chars:{value}"))
        .unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec!["4:4", "chars:1"]);
}

#[test]
fn programmatic_mutation_and_cursor_movement_emit_no_change() {
    let mut input = TextInput::new();
    let changed = input.output_on_change(|change| change.text().to_owned());
    input.set_text("text");
    input.clear();
    input.set_multiline(true);

    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        handle_command(&mut input, TextInputCommand::MoveRight, &mut cx);
        handle_command(&mut input, TextInputCommand::Submit, &mut cx);
    }
    let mut router = OutputRouter::<String>::new();
    router.route(changed, |value| value).unwrap();
    assert!(router.drain(&mut queue).unwrap().is_empty());
}
