use std::time::{Duration, Instant};

use crate::{
    component::{Component, ComponentRegistry, MountedComponents, TickScheduler},
    output::{Output, OutputQueue, OutputRouter},
    presentation::{IntoView, View},
    scene::resolve_scene,
};

use super::route_key_local;
use super::{FocusState, InteractionResult, Key, KeyStroke, Modifiers};

struct FocusProbe {
    focused: bool,
}

impl Component for FocusProbe {
    fn view(&self) -> View {
        View::text("focus").into_view()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_focus_changed(Self::focus_changed);
    }
}

impl FocusProbe {
    fn focus_changed(&mut self, focused: bool) {
        self.focused = focused;
    }
}

struct DynamicFocusProbe {
    focusable: bool,
    focused: bool,
}

impl Component for DynamicFocusProbe {
    fn view(&self) -> View {
        View::text("dynamic").into_view()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        if self.focusable {
            cx.focusable();
            cx.on_focus_changed(Self::focus_changed);
        }
    }
}

impl DynamicFocusProbe {
    fn focus_changed(&mut self, focused: bool) {
        self.focused = focused;
    }
}

struct TickProbe {
    enabled: bool,
    ticks: usize,
    changed: Output<usize>,
}

impl Component for TickProbe {
    fn view(&self) -> View {
        View::text(self.ticks.to_string()).into_view()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        if self.enabled {
            cx.tick(Duration::from_millis(80), Self::tick);
        }
    }
}

impl TickProbe {
    fn tick(&mut self, _now: Instant, cx: &mut crate::EventCx<'_>) -> bool {
        self.ticks += 1;
        cx.emit(self.changed, self.ticks);
        false
    }
}

struct Counter {
    value: usize,
    changed: Output<usize>,
}

#[derive(PartialEq, Eq)]
enum CounterCommand {
    Increment,
}

impl Component for Counter {
    fn view(&self) -> View {
        View::text(self.value.to_string()).into_view()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.focusable();
        cx.key_commands(Self::command, Self::handle);
    }
}

impl Counter {
    fn command(&self, key: KeyStroke) -> Option<CounterCommand> {
        (key.key() == Key::Char('+')).then_some(CounterCommand::Increment)
    }

    fn handle(
        &mut self,
        command: CounterCommand,
        cx: &mut crate::EventCx<'_>,
    ) -> InteractionResult {
        match command {
            CounterCommand::Increment => {
                self.value += 1;
                cx.emit(self.changed, self.value);
                InteractionResult::Consumed
            }
        }
    }
}

struct MismatchComponent;
struct OtherComponent;

impl MismatchComponent {
    fn command(&self, _: KeyStroke) -> Option<()> {
        None
    }

    fn handle(&mut self, _: (), _: &mut crate::EventCx<'_>) -> InteractionResult {
        InteractionResult::Ignored
    }

    fn focus_changed(&mut self, _: bool) {}
}

struct Modal {
    children: Vec<crate::ComponentHandle<FocusProbe>>,
    nested: Option<crate::ComponentHandle<Modal>>,
}

impl Component for Modal {
    fn view(&self) -> View {
        View::vertical(|column| {
            for child in &self.children {
                column.child(View::component(*child));
            }
            if let Some(nested) = self.nested {
                column.child(View::component(nested));
            }
        })
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.modal_scope();
    }
}

struct IgnoredEmitter {
    output: Output<&'static str>,
}

impl Component for IgnoredEmitter {
    fn view(&self) -> View {
        View::text("ignored").into_view()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.focusable();
        cx.key_commands(Self::command, Self::handle);
    }
}

impl IgnoredEmitter {
    fn command(&self, key: KeyStroke) -> Option<()> {
        (key.key() == Key::Char('i')).then_some(())
    }

    fn handle(&mut self, _: (), cx: &mut crate::EventCx<'_>) -> InteractionResult {
        cx.emit(self.output, "emitted");
        InteractionResult::Ignored
    }
}

struct IgnoredParent {
    child: crate::ComponentHandle<IgnoredEmitter>,
    handled: bool,
}

impl Component for IgnoredParent {
    fn view(&self) -> View {
        View::component(self.child)
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.key_commands(Self::command, Self::handle);
    }
}

impl IgnoredParent {
    fn command(&self, key: KeyStroke) -> Option<()> {
        (key.key() == Key::Char('i')).then_some(())
    }

    fn handle(&mut self, _: (), _: &mut crate::EventCx<'_>) -> InteractionResult {
        self.handled = true;
        InteractionResult::Consumed
    }
}

struct Parent {
    child: crate::ComponentHandle<FocusProbe>,
    handled: bool,
}

impl Component for Parent {
    fn view(&self) -> View {
        View::component(self.child).container()
    }

    fn capabilities(&self, cx: &mut super::ComponentCx<'_, Self>) {
        cx.key_commands(Self::command, Self::handle);
    }
}

impl Parent {
    fn command(&self, key: KeyStroke) -> Option<()> {
        (key.key() == Key::Char('p')).then_some(())
    }

    fn handle(&mut self, _: (), _: &mut crate::EventCx<'_>) -> InteractionResult {
        self.handled = true;
        InteractionResult::Consumed
    }
}

fn scene_for(view: View, registry: &ComponentRegistry) -> crate::scene::ResolvedScene {
    resolve_scene(&view, registry).expect("test scene resolves")
}

fn mount(scene: &crate::scene::ResolvedScene) -> MountedComponents {
    let mut mounted = MountedComponents::default();
    mounted.reconcile(scene.mounts.clone());
    mounted
}

#[test]
fn focus_traversal_is_cyclic_and_not_index_owned() {
    let mut registry = ComponentRegistry::new();
    let a = registry.register(FocusProbe { focused: false });
    let b = registry.register(FocusProbe { focused: false });
    let view = View::horizontal(|row| {
        row.child(View::component(a));
        row.child(View::component(b));
    });
    let scene = scene_for(view, &registry);
    let mounted = mount(&scene);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    assert_eq!(focus.focused(), Some(a.id()));
    assert!(focus.focus_next(mounted.current(), &scene.capabilities, &mut registry));
    assert_eq!(focus.focused(), Some(b.id()));
    assert!(focus.focus_next(mounted.current(), &scene.capabilities, &mut registry));
    assert_eq!(focus.focused(), Some(a.id()));
    assert_eq!(registry.with(a, |probe| probe.focused), Some(true));
    assert!(!registry.with(b, |probe| probe.focused).unwrap());
}

#[test]
fn singleton_focus_traversal_is_ignored_without_blur() {
    let mut registry = ComponentRegistry::new();
    let only = registry.register(FocusProbe { focused: false });
    let scene = scene_for(View::component(only), &registry);
    let mounted = mount(&scene);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    assert_eq!(focus.focused(), Some(only.id()));
    assert!(registry.with(only, |probe| probe.focused).unwrap());

    assert!(!focus.focus_next(mounted.current(), &scene.capabilities, &mut registry));
    assert!(!focus.focus_previous(mounted.current(), &scene.capabilities, &mut registry));
    assert_eq!(focus.focused(), Some(only.id()));
    assert!(registry.with(only, |probe| probe.focused).unwrap());
}

#[test]
fn typed_local_command_mutates_and_emits_only_before_later_drain() {
    let mut registry = ComponentRegistry::new();
    let output = Output::new();
    let counter = registry.register(Counter {
        value: 0,
        changed: output,
    });
    let scene = scene_for(View::component(counter), &registry);
    let mounted = mount(&scene);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);

    let mut queue = OutputQueue::new();
    assert_eq!(
        route_key_local(
            KeyStroke::new(Key::Char('+')),
            &mut focus,
            mounted.current(),
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    assert_eq!(registry.with(counter, |counter| counter.value), Some(1));

    let mut router = OutputRouter::new();
    router.route(output, |value| value).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec![1]);
}

#[test]
fn ignored_handler_can_emit_before_ancestor_consumes() {
    let mut registry = ComponentRegistry::new();
    let output = Output::new();
    let child = registry.register(IgnoredEmitter { output });
    let parent = registry.register(IgnoredParent {
        child,
        handled: false,
    });
    let scene = scene_for(View::component(parent), &registry);
    let mounted = mount(&scene);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);

    let mut queue = OutputQueue::new();
    assert_eq!(
        route_key_local(
            KeyStroke::new(Key::Char('i')),
            &mut focus,
            mounted.current(),
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    assert!(registry.with(parent, |parent| parent.handled).unwrap());
    let mut router = OutputRouter::new();
    router.route(output, |value| value).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec!["emitted"]);
}

#[test]
fn ignored_focused_component_bubbles_to_ancestor_but_not_sibling() {
    let mut registry = ComponentRegistry::new();
    let child = registry.register(FocusProbe { focused: false });
    let parent = registry.register(Parent {
        child,
        handled: false,
    });
    let scene = scene_for(View::component(parent), &registry);
    let mounted = mount(&scene);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    // The child is the only focusable node and therefore receives the focus.
    assert_eq!(focus.focused(), Some(child.id()));

    let mut queue = OutputQueue::new();
    assert_eq!(
        route_key_local(
            KeyStroke::new(Key::Char('p')),
            &mut focus,
            mounted.current(),
            &scene.capabilities,
            &mut registry,
            &mut queue,
        ),
        InteractionResult::Consumed
    );
    assert!(registry.with(parent, |parent| parent.handled).unwrap());
}

#[test]
fn focus_change_callbacks_advance_component_revision() {
    let mut registry = ComponentRegistry::new();
    let a = registry.register(FocusProbe { focused: false });
    let b = registry.register(FocusProbe { focused: false });
    let scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(a));
            column.child(View::component(b));
        }),
        &registry,
    );
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(&scene.mounts, &scene.capabilities, None, &mut registry);
    assert_eq!(registry.revision(a).unwrap().value(), 1);
    focus.focus_next(&scene.mounts, &scene.capabilities, &mut registry);
    assert_eq!(registry.revision(a).unwrap().value(), 2);
    assert_eq!(registry.revision(b).unwrap().value(), 1);
}

#[test]
fn removed_focused_component_receives_blur_from_retained_handler() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(FocusProbe { focused: false });
    let second = registry.register(FocusProbe { focused: false });
    let first_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(first));
        }),
        &registry,
    );
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(
        &first_scene.mounts,
        &first_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(registry.revision(first).unwrap().value(), 1);

    let second_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(second));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &second_scene.mounts,
        &second_scene.capabilities,
        None,
        &mut registry,
    );

    assert!(!registry.with(first, |probe| probe.focused).unwrap());
    assert_eq!(registry.revision(first).unwrap().value(), 2);
    assert!(registry.with(second, |probe| probe.focused).unwrap());
}

#[test]
fn losing_focusability_blurs_using_the_previous_capability() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(DynamicFocusProbe {
        focusable: true,
        focused: false,
    });
    let second = registry.register(FocusProbe { focused: false });
    let first_scene = scene_for(View::component(first), &registry);
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(
        &first_scene.mounts,
        &first_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(registry.revision(first).unwrap().value(), 1);

    registry.with_mut(first, |probe| probe.focusable = false);
    let second_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(first));
            column.child(View::component(second));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &second_scene.mounts,
        &second_scene.capabilities,
        None,
        &mut registry,
    );

    assert!(!registry.with(first, |probe| probe.focused).unwrap());
    assert_eq!(registry.revision(first).unwrap().value(), 3);
    assert!(registry.with(second, |probe| probe.focused).unwrap());
}

#[test]
fn mounted_tick_capabilities_emit_after_the_tick_dispatch_returns() {
    let mut registry = ComponentRegistry::new();
    let output = Output::new();
    let probe = registry.register(TickProbe {
        enabled: true,
        ticks: 0,
        changed: output,
    });
    let scene = scene_for(View::component(probe), &registry);
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(scene.mounts.clone());
    let mut scheduler = TickScheduler::new();
    let start = Instant::now();
    scheduler.sync_capabilities(mounted.current(), &scene.capabilities, &transitions, start);
    let mut queue = OutputQueue::new();
    let outcome = scheduler.tick_due_with_events(
        start + Duration::from_millis(80),
        &mut registry,
        &mut queue,
    );
    assert!(outcome.ran);
    assert!(!outcome.dirty);
    assert_eq!(registry.with(probe, |probe| probe.ticks), Some(1));

    let mut router = OutputRouter::new();
    router.route(output, |value| value).unwrap();
    assert_eq!(router.drain(&mut queue).unwrap(), vec![1]);

    registry.with_mut(probe, |probe| probe.enabled = false);
    let disabled = scene_for(View::component(probe), &registry);
    let transitions = mounted.reconcile(disabled.mounts.clone());
    scheduler.sync_capabilities(
        mounted.current(),
        &disabled.capabilities,
        &transitions,
        start + Duration::from_millis(100),
    );
    assert!(
        !scheduler
            .tick_due_with_events(start + Duration::from_secs(1), &mut registry, &mut queue,)
            .dirty
    );
}

#[test]
fn modal_focus_is_contained_and_restored_in_nested_order() {
    let mut registry = ComponentRegistry::new();
    let underlying = registry.register(FocusProbe { focused: false });
    let first_child = registry.register(FocusProbe { focused: false });
    let second_child = registry.register(FocusProbe { focused: false });
    let nested_child = registry.register(FocusProbe { focused: false });
    let first = registry.register(Modal {
        children: vec![first_child, second_child],
        nested: None,
    });
    let second = registry.register(Modal {
        children: vec![nested_child],
        nested: None,
    });

    let first_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(first));
        }),
        &registry,
    );
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(
        &first_scene.mounts,
        &first_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(first_child.id()));
    focus.focus_next(
        &first_scene.mounts,
        &first_scene.capabilities,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(second_child.id()));

    registry.with_mut(first, |modal| modal.nested = Some(second));
    let nested_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(first));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &nested_scene.mounts,
        &nested_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.active_modal(), Some(second.id()));
    assert_eq!(focus.focused(), Some(nested_child.id()));

    registry.with_mut(first, |modal| modal.nested = None);
    let restored_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(first));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &restored_scene.mounts,
        &restored_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.active_modal(), Some(first.id()));
    assert_eq!(focus.focused(), Some(second_child.id()));

    let background_scene = scene_for(View::component(underlying), &registry);
    focus.reconcile_with_geometry(
        &background_scene.mounts,
        &background_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(underlying.id()));
}

#[test]
fn removing_a_modal_hierarchy_unwinds_to_background_focus() {
    let mut registry = ComponentRegistry::new();
    let background_a = registry.register(FocusProbe { focused: false });
    let background_b = registry.register(FocusProbe { focused: false });
    let first_child = registry.register(FocusProbe { focused: false });
    let nested_child = registry.register(FocusProbe { focused: false });
    let first = registry.register(Modal {
        children: vec![first_child],
        nested: None,
    });
    let nested = registry.register(Modal {
        children: vec![nested_child],
        nested: None,
    });

    let background_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(background_a));
            column.child(View::component(background_b));
        }),
        &registry,
    );
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(
        &background_scene.mounts,
        &background_scene.capabilities,
        None,
        &mut registry,
    );
    focus.focus_next(
        &background_scene.mounts,
        &background_scene.capabilities,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(background_b.id()));

    let first_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(background_a));
            column.child(View::component(background_b));
            column.child(View::component(first));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &first_scene.mounts,
        &first_scene.capabilities,
        None,
        &mut registry,
    );
    registry.with_mut(first, |modal| modal.nested = Some(nested));

    let nested_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(background_a));
            column.child(View::component(background_b));
            column.child(View::component(first));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &nested_scene.mounts,
        &nested_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(nested_child.id()));

    let removed_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(background_a));
            column.child(View::component(background_b));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &removed_scene.mounts,
        &removed_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(background_b.id()));
    assert!(focus.modal_restore_is_empty());
}

#[test]
fn replacing_a_modal_does_not_leave_stale_restore_frames() {
    let mut registry = ComponentRegistry::new();
    let underlying = registry.register(FocusProbe { focused: false });
    let first_child = registry.register(FocusProbe { focused: false });
    let first_other = registry.register(FocusProbe { focused: false });
    let second_child = registry.register(FocusProbe { focused: false });
    let first = registry.register(Modal {
        children: vec![first_child, first_other],
        nested: None,
    });
    let second = registry.register(Modal {
        children: vec![second_child],
        nested: None,
    });

    let first_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(first));
        }),
        &registry,
    );
    let mut focus = FocusState::default();
    focus.reconcile_with_geometry(
        &first_scene.mounts,
        &first_scene.capabilities,
        None,
        &mut registry,
    );
    focus.focus_next(
        &first_scene.mounts,
        &first_scene.capabilities,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(first_other.id()));

    let replacement_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(second));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &replacement_scene.mounts,
        &replacement_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(second_child.id()));

    let background_scene = scene_for(View::component(underlying), &registry);
    focus.reconcile_with_geometry(
        &background_scene.mounts,
        &background_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(underlying.id()));

    let reopened_scene = scene_for(
        View::vertical(|column| {
            column.child(View::component(underlying));
            column.child(View::component(first));
        }),
        &registry,
    );
    focus.reconcile_with_geometry(
        &reopened_scene.mounts,
        &reopened_scene.capabilities,
        None,
        &mut registry,
    );
    assert_eq!(focus.focused(), Some(first_child.id()));
}

#[test]
#[should_panic(expected = "component key mapping type mismatch")]
fn erased_key_mapping_mismatch_fails_loudly() {
    let mut capabilities = super::ComponentCapabilities::default();
    let mut cx = super::ComponentCx::<MismatchComponent>::new(&mut capabilities);
    cx.key_commands(MismatchComponent::command, MismatchComponent::handle);
    let _ = (capabilities.key_commands[0].map)(&OtherComponent, KeyStroke::new(Key::Enter));
}

#[test]
#[should_panic(expected = "component focus handler type mismatch")]
fn erased_focus_handler_mismatch_fails_loudly() {
    let mut capabilities = super::ComponentCapabilities::default();
    let mut cx = super::ComponentCx::<MismatchComponent>::new(&mut capabilities);
    cx.on_focus_changed(MismatchComponent::focus_changed);
    let handler = capabilities.focus_changed.as_ref().unwrap().clone();
    handler(&mut OtherComponent, true);
}

#[test]
fn modifiers_are_value_types_without_backend_state() {
    let stroke = KeyStroke::with_modifiers(Key::Enter, Modifiers::SHIFT | Modifiers::ALT);
    assert_eq!(stroke.key(), Key::Enter);
    assert_eq!(stroke.modifiers(), Modifiers::SHIFT | Modifiers::ALT);
}
