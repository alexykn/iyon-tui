use std::{any::Any, cell::RefCell, collections::HashMap, fmt};

use super::{Component, ComponentHandle, ComponentId, ComponentRevision};
use crate::interaction::{ComponentCapabilities, ComponentCx};
use crate::perf::{self, Counter};
use crate::presentation::View;

trait ErasedComponent {
    fn view(&self) -> View;
    fn capabilities(&self) -> ComponentCapabilities;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<C> ErasedComponent for C
where
    C: Component,
{
    fn view(&self) -> View {
        perf::inc(Counter::ComponentViewCalls);
        Component::view(self)
    }

    fn capabilities(&self) -> ComponentCapabilities {
        perf::inc(Counter::ComponentCapabilityCalls);
        let mut capabilities = ComponentCapabilities::default();
        let mut cx = ComponentCx::new(&mut capabilities);
        Component::capabilities(self, &mut cx);
        capabilities
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ComponentSnapshot {
    pub(crate) view: View,
    pub(crate) revision: ComponentRevision,
    pub(crate) capabilities: ComponentCapabilities,
}

struct ComponentEntry {
    component: Box<dyn ErasedComponent>,
    revision: ComponentRevision,
    snapshot: RefCell<Option<ComponentSnapshot>>,
}

impl fmt::Debug for ComponentEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ComponentEntry(..)")
    }
}

/// The sole owner of retained component instances.
#[derive(Debug)]
pub(crate) struct ComponentRegistry {
    slots: HashMap<ComponentId, ComponentEntry>,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentRegistry {
    pub(crate) fn new() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }

    pub(crate) fn register<C>(&mut self, component: C) -> ComponentHandle<C>
    where
        C: Component,
    {
        let id = ComponentId::allocate();
        self.slots.insert(
            id,
            ComponentEntry {
                component: Box::new(component),
                revision: ComponentRevision::default(),
                snapshot: RefCell::new(None),
            },
        );
        ComponentHandle::new(id)
    }

    #[cfg(test)]
    pub(crate) fn contains<C>(&self, handle: ComponentHandle<C>) -> bool
    where
        C: Component,
    {
        self.slots
            .get(&handle.id())
            .is_some_and(|entry| entry.component.as_any().is::<C>())
    }

    pub(crate) fn with<C, R>(
        &self,
        handle: ComponentHandle<C>,
        f: impl FnOnce(&C) -> R,
    ) -> Option<R>
    where
        C: Component,
    {
        let entry = self.slots.get(&handle.id())?;
        let component = entry.component.as_any().downcast_ref::<C>()?;
        Some(f(component))
    }

    pub(crate) fn with_any<R>(&self, id: ComponentId, f: impl FnOnce(&dyn Any) -> R) -> Option<R> {
        let entry = self.slots.get(&id)?;
        Some(f(entry.component.as_any()))
    }

    pub(crate) fn with_any_mut<R>(
        &mut self,
        id: ComponentId,
        f: impl FnOnce(&mut dyn Any) -> R,
    ) -> Option<R> {
        let entry = self.slots.get_mut(&id)?;
        let result = f(entry.component.as_any_mut());
        entry.revision = entry.revision.increment();
        entry.snapshot.get_mut().take();
        Some(result)
    }

    #[cfg(test)]
    pub(crate) fn capabilities(&self, id: ComponentId) -> Option<ComponentCapabilities> {
        self.slots
            .get(&id)
            .map(|entry| entry.component.capabilities())
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn invalidate(&mut self, id: ComponentId) -> bool {
        let Some(entry) = self.slots.get_mut(&id) else {
            return false;
        };
        entry.revision = entry.revision.increment();
        entry.snapshot.get_mut().take();
        true
    }

    pub(crate) fn resolution(&self, id: ComponentId) -> Option<ComponentSnapshot> {
        let entry = self.slots.get(&id)?;
        {
            let snapshot = entry.snapshot.borrow();
            if snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.revision == entry.revision)
            {
                return snapshot.clone();
            }
        }

        let snapshot = ComponentSnapshot {
            view: entry.component.view(),
            revision: entry.revision,
            capabilities: entry.component.capabilities(),
        };
        *entry.snapshot.borrow_mut() = Some(snapshot.clone());
        Some(snapshot)
    }

    pub(crate) fn with_mut<C, R>(
        &mut self,
        handle: ComponentHandle<C>,
        f: impl FnOnce(&mut C) -> R,
    ) -> Option<R>
    where
        C: Component,
    {
        let entry = self.slots.get_mut(&handle.id())?;
        let component = entry.component.as_any_mut().downcast_mut::<C>()?;
        let result = f(component);
        entry.revision = entry.revision.increment();
        entry.snapshot.get_mut().take();
        Some(result)
    }

    /// PERF-12 T13.1 R8: type-erased removal for deferred component
    /// retirement (the native host bridge knows only ComponentIds by the
    /// time a retired component may physically be reclaimed).
    pub(crate) fn remove_id(&mut self, id: ComponentId) -> bool {
        self.slots.remove(&id).is_some()
    }

    pub(crate) fn remove<C>(&mut self, handle: ComponentHandle<C>) -> Option<C>
    where
        C: Component,
    {
        let entry = self.slots.get(&handle.id())?;
        if !entry.component.as_any().is::<C>() {
            return None;
        }
        self.slots
            .remove(&handle.id())?
            .component
            .into_any()
            .downcast::<C>()
            .ok()
            .map(|component| *component)
    }

    #[cfg(test)]
    pub(crate) fn revision<C>(&self, handle: ComponentHandle<C>) -> Option<ComponentRevision>
    where
        C: Component,
    {
        let entry = self.slots.get(&handle.id())?;
        entry.component.as_any().is::<C>().then_some(entry.revision)
    }
}
