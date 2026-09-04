use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::{
    Component, ComponentHandle, History, Output, OutputRouter, RouteConflict, Theme,
    component::ComponentRegistry, scene::Scene,
};

use super::{
    handle::AppHandle,
    input::{GlobalBindings, PasteInterceptors},
    timer::{TimerHandle, TimerQueue},
};

/// Borrow-scoped application capabilities available during initialization and
/// action updates.
pub(crate) struct AppCxParts<'a, Action> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) components: &'a mut ComponentRegistry,
    pub(crate) outputs: &'a mut OutputRouter<Action>,
    pub(crate) timers: &'a mut TimerQueue<Action>,
    pub(crate) theme: &'a mut Theme,
    pub(crate) global_bindings: &'a mut GlobalBindings<Action>,
    pub(crate) paste_interceptors: &'a mut PasteInterceptors<Action>,
    pub(crate) deferred_pastes: &'a mut VecDeque<String>,
    pub(crate) exit_requested: &'a mut bool,
    pub(crate) handle: &'a AppHandle<Action>,
}

pub struct AppCx<'a, Action> {
    scene: &'a mut Scene,
    components: &'a mut ComponentRegistry,
    outputs: &'a mut OutputRouter<Action>,
    timers: &'a mut TimerQueue<Action>,
    theme: &'a mut Theme,
    global_bindings: &'a mut GlobalBindings<Action>,
    paste_interceptors: &'a mut PasteInterceptors<Action>,
    deferred_pastes: &'a mut VecDeque<String>,
    exit_requested: &'a mut bool,
    handle: &'a AppHandle<Action>,
    now: Instant,
}

impl<'a, Action> AppCx<'a, Action> {
    pub(crate) fn new(parts: AppCxParts<'a, Action>, now: Instant) -> Self {
        let AppCxParts {
            scene,
            components,
            outputs,
            timers,
            global_bindings,
            theme,
            paste_interceptors,
            deferred_pastes,
            exit_requested,
            handle,
        } = parts;
        Self {
            scene,
            components,
            outputs,
            timers,
            theme,
            global_bindings,
            paste_interceptors,
            deferred_pastes,
            exit_requested,
            handle,
            now,
        }
    }

    /// Registers one retained component and returns its typed identity.
    pub fn register<C>(&mut self, component: C) -> ComponentHandle<C>
    where
        C: Component,
    {
        self.components.register(component)
    }

    /// Provides immutable closure-scoped access to a registered component.
    pub fn with_component<C, R>(
        &self,
        handle: ComponentHandle<C>,
        access: impl FnOnce(&C) -> R,
    ) -> Option<R>
    where
        C: Component,
    {
        self.components.with(handle, access)
    }

    /// Provides mutable closure-scoped access to a registered component.
    pub fn with_component_mut<C, R>(
        &mut self,
        handle: ComponentHandle<C>,
        access: impl FnOnce(&mut C) -> R,
    ) -> Option<R>
    where
        C: Component,
    {
        self.components.with_mut(handle, access)
    }

    /// Removes a registered component after application code has released all
    /// semantic references to it.
    pub fn remove_component<C>(&mut self, handle: ComponentHandle<C>) -> Option<C>
    where
        C: Component,
    {
        let component = self.components.remove(handle);
        if component.is_some() {
            self.paste_interceptors.remove(handle);
        }
        component
    }

    /// Routes one typed component output into an application action.
    pub fn route<T: 'static>(
        &mut self,
        output: Output<T>,
        map: impl Fn(T) -> Action + 'static,
    ) -> Result<(), RouteConflict> {
        self.outputs.route(output, map)
    }

    /// Removes an application route for one typed component output.
    pub fn remove_route<T: 'static>(&mut self, output: Output<T>) -> bool {
        self.outputs.remove(output)
    }

    /// Returns the persistent root History, when this App was configured with
    /// one.
    #[must_use]
    pub fn history(&self) -> Option<&History> {
        self.scene.history()
    }

    /// Returns mutable access to the persistent root History, when configured.
    pub fn history_mut(&mut self) -> Option<&mut History> {
        self.scene.history_mut()
    }

    /// Returns the active application theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Returns mutable access to the active application theme.
    pub fn theme_mut(&mut self) -> &mut Theme {
        self.theme
    }

    /// Returns the driver's logical time for deterministic application policy.
    #[must_use]
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Returns an Action-only handle targeting this application's ingress.
    #[must_use]
    pub fn handle(&self) -> AppHandle<Action> {
        self.handle.clone()
    }

    /// Replaces the application-global Action factory for one exact key.
    ///
    /// The factory runs only after focused and ancestor Component routing,
    /// including framework focus traversal, returns `Ignored`.
    pub fn bind_key(&mut self, key: crate::KeyStroke, action: impl Fn() -> Action + 'static) {
        self.global_bindings.bind(key, action);
    }

    /// Removes an application-global key binding.
    pub fn unbind_key(&mut self, key: crate::KeyStroke) -> bool {
        self.global_bindings.unbind(key)
    }

    /// Registers a component-scoped paste-to-Action mapping.
    ///
    /// The first matching Component in the ordinary focused/modal routing
    /// chain wins; background Components outside that chain cannot intercept.
    pub fn intercept_paste<C>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> Action + 'static,
    ) where
        C: Component,
    {
        self.paste_interceptors.intercept(component, map);
    }

    /// Removes a component-scoped paste interceptor.
    pub fn remove_paste_interceptor<C>(&mut self, component: ComponentHandle<C>) -> bool
    where
        C: Component,
    {
        self.paste_interceptors.remove(component)
    }

    /// Queues text for ordinary focused-component paste routing after this
    /// update returns. Interceptors are deliberately bypassed.
    pub fn forward_paste(&mut self, text: impl Into<String>) {
        self.deferred_pastes.push_back(text.into());
    }

    /// Schedules one non-recurring Action.
    pub fn schedule_after(&mut self, delay: Duration, action: Action) -> TimerHandle {
        self.timers.schedule(self.now, delay, action)
    }

    /// Cancels a pending timer. A fired or already-cancelled timer returns
    /// `false`.
    pub fn cancel_timer(&mut self, handle: TimerHandle) -> bool {
        self.timers.cancel(handle)
    }

    /// Requests semantic application exit after the current update returns.
    pub fn exit(&mut self) {
        *self.exit_requested = true;
    }
}
