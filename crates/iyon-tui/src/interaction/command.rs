use std::{
    any::Any,
    collections::HashMap,
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{component::ComponentId, geometry::Size, output::EventCx};

use super::{InteractionResult, KeyStroke};

type MapCommand = dyn Fn(&dyn Any, KeyStroke) -> Option<Box<dyn Any>>;
type HandleCommand =
    dyn for<'a> Fn(&mut dyn Any, Box<dyn Any>, &mut EventCx<'a>) -> InteractionResult;
type FocusChanged = dyn Fn(&mut dyn Any, bool);
type PasteHandler = dyn for<'paste, 'event> Fn(
    &mut dyn Any,
    &'paste str,
    &mut EventCx<'event>,
) -> InteractionResult;
type TickHandler = dyn for<'a> Fn(&mut dyn Any, Instant, &mut EventCx<'a>) -> bool;
type LayoutChanged = dyn Fn(&mut dyn Any, Size);

#[derive(Clone)]
pub(crate) struct KeyCommandCapability {
    pub(crate) map: Arc<MapCommand>,
    pub(crate) handle: Arc<HandleCommand>,
}

#[derive(Clone)]
pub(crate) struct TickCapability {
    pub(crate) interval: Duration,
    pub(crate) handler: Arc<TickHandler>,
}

#[derive(Clone, Default)]
pub(crate) struct ComponentCapabilities {
    pub(crate) focusable: bool,
    pub(crate) modal_scope: bool,
    pub(crate) focus_changed: Option<Arc<FocusChanged>>,
    pub(crate) paste: Option<Arc<PasteHandler>>,
    pub(crate) key_commands: Vec<KeyCommandCapability>,
    pub(crate) tick: Option<TickCapability>,
    pub(crate) layout_changed: Option<Arc<LayoutChanged>>,
    pub(crate) content_extent_changed: Option<Arc<LayoutChanged>>,
}

impl std::fmt::Debug for ComponentCapabilities {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComponentCapabilities")
            .field("focusable", &self.focusable)
            .field("modal_scope", &self.modal_scope)
            .field("key_commands", &self.key_commands.len())
            .field("paste", &self.paste.is_some())
            .field("tick", &self.tick.is_some())
            .field("layout_changed", &self.layout_changed.is_some())
            .field(
                "content_extent_changed",
                &self.content_extent_changed.is_some(),
            )
            .finish()
    }
}

/// Ephemeral capability declaration context for a mounted component.
pub struct ComponentCx<'a, C> {
    pub(crate) capabilities: &'a mut ComponentCapabilities,
    marker: PhantomData<fn(&'a C)>,
}

impl<'a, C> ComponentCx<'a, C> {
    pub(crate) fn new(capabilities: &'a mut ComponentCapabilities) -> Self {
        Self {
            capabilities,
            marker: PhantomData,
        }
    }

    /// Declares that this component may participate in host-owned focus.
    pub fn focusable(&mut self) {
        self.capabilities.focusable = true;
    }

    /// Declares this component as an interaction modal scope.
    pub fn modal_scope(&mut self) {
        self.capabilities.modal_scope = true;
    }

    /// Registers an optional focus transition callback.
    pub fn on_focus_changed(&mut self, handler: fn(&mut C, bool))
    where
        C: 'static,
    {
        self.capabilities.focus_changed = Some(Arc::new(move |component, focused| {
            let component = component
                .downcast_mut::<C>()
                .expect("component focus handler type mismatch");
            handler(component, focused);
        }));
    }

    /// Registers a borrowed paste handler for this component.
    pub fn on_paste(
        &mut self,
        handler: for<'paste, 'event> fn(
            &mut C,
            &'paste str,
            &mut EventCx<'event>,
        ) -> InteractionResult,
    ) where
        C: 'static,
    {
        self.capabilities.paste = Some(Arc::new(move |component, text, cx| {
            let component = component
                .downcast_mut::<C>()
                .expect("component paste handler type mismatch");
            handler(component, text, cx)
        }));
    }

    /// Registers a crate-private layout-size synchronization callback.
    pub(crate) fn on_layout_changed(&mut self, handler: fn(&mut C, Size))
    where
        C: 'static,
    {
        self.capabilities.layout_changed = Some(Arc::new(move |component, size| {
            let component = component
                .downcast_mut::<C>()
                .expect("component layout handler type mismatch");
            handler(component, size);
        }));
    }

    /// Registers a crate-private notification for the full intrinsic extent
    /// behind a component-owned viewport.
    pub(crate) fn on_content_extent_changed(&mut self, handler: fn(&mut C, Size))
    where
        C: 'static,
    {
        self.capabilities.content_extent_changed = Some(Arc::new(move |component, size| {
            let component = component
                .downcast_mut::<C>()
                .expect("component content extent handler type mismatch");
            handler(component, size);
        }));
    }

    /// Registers an ordered typed local key-command mapping and handler.
    pub fn key_commands<Command: 'static>(
        &mut self,
        map: fn(&C, KeyStroke) -> Option<Command>,
        handle: for<'event> fn(&mut C, Command, &mut EventCx<'event>) -> InteractionResult,
    ) where
        C: 'static,
    {
        self.capabilities.key_commands.push(KeyCommandCapability {
            map: Arc::new(move |component, key| {
                let component = component
                    .downcast_ref::<C>()
                    .expect("component key mapping type mismatch");
                map(component, key).map(|command| Box::new(command) as Box<dyn Any>)
            }),
            handle: Arc::new(move |component, command, cx| {
                let component = component
                    .downcast_mut::<C>()
                    .expect("component key command type mismatch");
                let command = *command
                    .downcast::<Command>()
                    .expect("component command type mismatch");
                handle(component, command, cx)
            }),
        });
    }

    /// Declares a mounted framework tick capability.
    pub fn tick(
        &mut self,
        interval: Duration,
        handler: for<'event> fn(&mut C, Instant, &mut EventCx<'event>) -> bool,
    ) where
        C: 'static,
    {
        assert!(
            !interval.is_zero(),
            "component tick interval must be nonzero"
        );
        self.capabilities.tick = Some(TickCapability {
            interval,
            handler: Arc::new(move |component, now, cx| {
                let component = component
                    .downcast_mut::<C>()
                    .expect("component tick type mismatch");
                handler(component, now, cx)
            }),
        });
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct MountedCapabilities {
    pub(crate) entries: HashMap<ComponentId, ComponentCapabilities>,
}

impl MountedCapabilities {
    pub(crate) fn insert(&mut self, id: ComponentId, capabilities: ComponentCapabilities) {
        self.entries.insert(id, capabilities);
    }

    pub(crate) fn get(&self, id: ComponentId) -> Option<&ComponentCapabilities> {
        self.entries.get(&id)
    }

    pub(crate) fn modal_ids<'a>(
        &'a self,
        order: impl Iterator<Item = &'a crate::component::MountNode> + 'a,
    ) -> impl Iterator<Item = ComponentId> + 'a {
        order
            .filter(|node| self.get(node.id).is_some_and(|caps| caps.modal_scope))
            .map(|node| node.id)
    }
}
