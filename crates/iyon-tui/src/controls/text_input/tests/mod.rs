use std::ops::Range;

use crate::component::ComponentRegistry;
use crate::interaction::route_key_local;
use crate::interaction::{FocusState, route_paste};
use crate::output::{OutputQueue, OutputRouter};
use crate::scene::resolve_scene;
use crate::{
    Component, ComponentHandle, EventCx, InteractionResult, Key, KeyStroke, Modifiers, Output, View,
};

use super::{TextInput, buffer::TextBuffer, command::command_for_key};

mod buffer;
mod command;
mod output;
mod presentation;

fn key(key: Key) -> KeyStroke {
    KeyStroke::new(key)
}

fn modified(key: Key, modifiers: Modifiers) -> KeyStroke {
    KeyStroke::with_modifiers(key, modifiers)
}

#[derive(Debug)]
struct PasteParent {
    child: ComponentHandle<TextInput>,
    received: Output<usize>,
    modal: bool,
}

impl Component for PasteParent {
    fn view(&self) -> View {
        View::component(self.child)
    }

    fn capabilities(&self, cx: &mut crate::ComponentCx<'_, Self>) {
        if self.modal {
            cx.modal_scope();
        }
        cx.on_paste(Self::paste);
    }
}

impl PasteParent {
    fn paste(&mut self, text: &str, cx: &mut EventCx<'_>) -> InteractionResult {
        cx.emit(self.received, text.len());
        InteractionResult::Consumed
    }
}

struct FocusablePassive;

impl Component for FocusablePassive {
    fn view(&self) -> View {
        crate::presentation::factory::text("passive")
    }

    fn capabilities(&self, cx: &mut crate::ComponentCx<'_, Self>) {
        cx.focusable();
    }
}

struct PassiveParent {
    child: ComponentHandle<FocusablePassive>,
    received: Output<usize>,
}

impl Component for PassiveParent {
    fn view(&self) -> View {
        View::component(self.child)
    }

    fn capabilities(&self, cx: &mut crate::ComponentCx<'_, Self>) {
        cx.on_paste(Self::paste);
    }
}

impl PassiveParent {
    fn paste(&mut self, text: &str, cx: &mut EventCx<'_>) -> InteractionResult {
        cx.emit(self.received, text.len());
        InteractionResult::Consumed
    }
}

fn rows(ranges: impl IntoIterator<Item = Range<usize>>) -> Vec<Range<usize>> {
    ranges.into_iter().collect()
}

#[test]
fn mounted_text_input_paste_is_one_consumed_change_event() {
    let mut input = TextInput::new().multiline(true);
    let changed = input.output_on_change(|change| change.text().to_owned());
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(input);
    let scene = resolve_scene(&View::component(handle), &registry).unwrap();
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    let mut queue = OutputQueue::new();
    assert_eq!(
        route_paste(
            "a\r\nb",
            &focus,
            &scene.mounts,
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    let mut router = OutputRouter::<String>::new();
    router.route(changed, |text| text).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec!["a\nb"]);
}

#[test]
fn paste_bubbles_to_ancestor_but_not_when_focused_input_consumes() {
    let mut registry = ComponentRegistry::new();
    let passive = registry.register(FocusablePassive);
    let parent_output = Output::new();
    let parent = registry.register(PassiveParent {
        child: passive,
        received: parent_output,
    });
    let scene = resolve_scene(&View::component(parent), &registry).unwrap();
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    let mut queue = OutputQueue::new();
    assert_eq!(
        route_paste(
            "ignored by parent",
            &focus,
            &scene.mounts,
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    let mut router = OutputRouter::<usize>::new();
    router.route(parent_output, |length| length).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec![17]);
}

#[test]
fn mounted_command_emits_deferred_output_after_key_dispatch() {
    let mut input = TextInput::new();
    let changed = input.output_on_change(|change| change.text().to_owned());
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(input);
    let scene = resolve_scene(&View::component(handle), &registry).unwrap();
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    let mut queue = OutputQueue::new();
    assert_eq!(
        route_key_local(
            key(Key::Char('x')),
            &mut focus,
            &scene.mounts,
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    assert!(!queue.is_empty());
    let mut router = OutputRouter::<String>::new();
    router.route(changed, |text| text).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec!["x"]);
}

#[test]
fn modal_paste_containment_excludes_background_components() {
    let mut registry = ComponentRegistry::new();
    let background_output = Output::new();
    let background_child = registry.register(TextInput::new());
    let background = registry.register(PasteParent {
        child: background_child,
        received: background_output,
        modal: false,
    });
    let modal_input = registry.register(TextInput::new());
    let modal_output = Output::new();
    let modal = registry.register(PasteParent {
        child: modal_input,
        received: modal_output,
        modal: true,
    });
    let scene = resolve_scene(
        &crate::presentation::factory::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    View::component(background),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    View::component(modal),
                ),
            ],
            0,
        ),
        &registry,
    )
    .unwrap();
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    let mut queue = OutputQueue::new();
    route_paste(
        "modal",
        &focus,
        &scene.mounts,
        &scene.capabilities,
        &mut registry,
        &mut queue,
    );
    let mut router = OutputRouter::<usize>::new();
    router.route(background_output, |length| length).unwrap();
    router.route(modal_output, |length| length).unwrap();
    assert!(router.drain(&mut queue).unwrap().is_empty());
}
