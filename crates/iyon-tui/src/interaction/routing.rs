use crate::{
    component::{ComponentId, ComponentRegistry, MountGraph},
    output::OutputQueue,
};

use super::{FocusState, InteractionResult, KeyStroke, MountedCapabilities};

pub(crate) fn route_paste_interceptor<A>(
    text: &str,
    focus: &FocusState,
    graph: &MountGraph,
    mut intercept: impl FnMut(ComponentId, &str) -> Option<A>,
) -> Option<A> {
    for id in routing_chain(focus, graph) {
        if let Some(action) = intercept(id, text) {
            return Some(action);
        }
    }
    None
}

pub(crate) fn route_paste(
    text: &str,
    focus: &FocusState,
    graph: &MountGraph,
    capabilities: &MountedCapabilities,
    registry: &mut ComponentRegistry,
    queue: &mut OutputQueue,
) -> InteractionResult {
    let mut cx = queue.event_cx();
    for id in routing_chain(focus, graph) {
        let Some(handler) = capabilities.get(id).and_then(|caps| caps.paste.as_ref()) else {
            continue;
        };
        let result = registry
            .with_any_mut(id, |component| handler(component, text, &mut cx))
            .unwrap_or(InteractionResult::Ignored);
        if matches!(result, InteractionResult::Consumed) {
            return result;
        }
    }
    InteractionResult::Ignored
}

pub(crate) fn route_key_local(
    key: KeyStroke,
    focus: &mut FocusState,
    graph: &MountGraph,
    capabilities: &MountedCapabilities,
    registry: &mut ComponentRegistry,
    queue: &mut OutputQueue,
) -> InteractionResult {
    let mut cx = queue.event_cx();
    let chain = routing_chain(focus, graph);

    for id in chain {
        let Some(component_capabilities) = capabilities.get(id) else {
            continue;
        };
        for capability in &component_capabilities.key_commands {
            let Some(command) = registry
                .with_any(id, |component| (capability.map)(component, key))
                .flatten()
            else {
                continue;
            };
            let result = registry
                .with_any_mut(id, |component| {
                    (capability.handle)(component, command, &mut cx)
                })
                .unwrap_or(InteractionResult::Ignored);
            if matches!(result, InteractionResult::Consumed) {
                return result;
            }
        }
    }

    if key.key() == super::Key::Tab
        && key.modifiers() == super::Modifiers::NONE
        && focus.focus_next(graph, capabilities, registry)
    {
        return InteractionResult::Consumed;
    }

    InteractionResult::Ignored
}

fn routing_chain(focus: &FocusState, graph: &MountGraph) -> Vec<ComponentId> {
    let start = focus.focused().or_else(|| focus.active_modal());
    let Some(mut current) = start else {
        return Vec::new();
    };

    let modal = focus.active_modal();
    let mut chain = Vec::new();
    loop {
        if modal.is_some_and(|modal| !super::focus::is_descendant_or_self(current, modal, graph)) {
            break;
        }
        chain.push(current);
        if modal == Some(current) {
            break;
        }
        let Some(parent) = graph.parent(current) else {
            break;
        };
        current = parent;
    }
    chain
}
