use std::collections::HashMap;

use crate::{Component, ComponentHandle, KeyStroke, component::ComponentId};

pub(crate) struct GlobalBindings<Action> {
    bindings: HashMap<KeyStroke, Box<dyn Fn() -> Action>>,
}

impl<Action> Default for GlobalBindings<Action> {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }
}

impl<Action> GlobalBindings<Action> {
    pub(crate) fn bind(&mut self, key: KeyStroke, action: impl Fn() -> Action + 'static) {
        self.bindings.insert(key, Box::new(action));
    }

    pub(crate) fn unbind(&mut self, key: KeyStroke) -> bool {
        self.bindings.remove(&key).is_some()
    }

    pub(crate) fn action(&self, key: KeyStroke) -> Option<Action> {
        self.bindings.get(&key).map(|factory| factory())
    }
}

pub(crate) struct PasteInterceptors<Action> {
    interceptors: HashMap<ComponentId, Box<dyn Fn(String) -> Action>>,
}

impl<Action> Default for PasteInterceptors<Action> {
    fn default() -> Self {
        Self {
            interceptors: HashMap::new(),
        }
    }
}

impl<Action> PasteInterceptors<Action> {
    pub(crate) fn intercept<C>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> Action + 'static,
    ) where
        C: Component,
    {
        self.interceptors.insert(component.id(), Box::new(map));
    }

    pub(crate) fn remove<C>(&mut self, component: ComponentHandle<C>) -> bool
    where
        C: Component,
    {
        self.interceptors.remove(&component.id()).is_some()
    }

    /// PERF-12 T13.1 R8: ID-based counterpart of `remove` for host-side
    /// deferred component retirement.
    pub(crate) fn remove_id(&mut self, component: ComponentId) -> bool {
        self.interceptors.remove(&component).is_some()
    }

    pub(crate) fn action(&self, component: ComponentId, text: &str) -> Option<Action> {
        self.interceptors
            .get(&component)
            .map(|map| map(text.to_owned()))
    }
}
