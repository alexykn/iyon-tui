use std::collections::HashMap;

use crate::{Component, ComponentHandle, KeyStroke, component::ComponentId};

use super::host::RoutedOutput;

#[derive(Default)]
pub(crate) struct GlobalBindings {
    bindings: HashMap<KeyStroke, Box<dyn Fn() -> RoutedOutput>>,
}

impl GlobalBindings {
    pub(crate) fn bind(&mut self, key: KeyStroke, factory: impl Fn() -> RoutedOutput + 'static) {
        self.bindings.insert(key, Box::new(factory));
    }

    pub(crate) fn output(&self, key: KeyStroke) -> Option<RoutedOutput> {
        self.bindings.get(&key).map(|factory| factory())
    }
}

#[derive(Default)]
pub(crate) struct PasteInterceptors {
    interceptors: HashMap<ComponentId, Box<dyn Fn(String) -> RoutedOutput>>,
}

impl PasteInterceptors {
    pub(crate) fn intercept<C>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> RoutedOutput + 'static,
    ) where
        C: Component,
    {
        self.interceptors.insert(component.id(), Box::new(map));
    }

    /// PERF-12 T13.1 R8: ID-based counterpart of `remove` for host-side
    /// deferred component retirement.
    pub(crate) fn remove_id(&mut self, component: ComponentId) -> bool {
        self.interceptors.remove(&component).is_some()
    }

    pub(crate) fn output(&self, component: ComponentId, text: &str) -> Option<RoutedOutput> {
        self.interceptors
            .get(&component)
            .map(|map| map(text.to_owned()))
    }
}
