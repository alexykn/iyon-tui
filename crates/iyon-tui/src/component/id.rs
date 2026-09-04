use std::{fmt, hash::Hash, marker::PhantomData, num::NonZeroU64, sync::atomic::AtomicU64};

use crate::id::next_nonzero_id;

static NEXT_COMPONENT_ID: AtomicU64 = AtomicU64::new(1);

/// Opaque identity for a retained component registration.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ComponentId(NonZeroU64);

impl ComponentId {
    pub(crate) fn allocate() -> Self {
        Self(next_nonzero_id(
            &NEXT_COMPONENT_ID,
            "component id exhausted",
        ))
    }

    pub(crate) const fn value(self) -> u64 {
        self.0.get()
    }

    pub(crate) const fn from_raw(value: u64) -> Self {
        let Some(value) = NonZeroU64::new(value) else {
            panic!("component id must be non-zero");
        };
        Self(value)
    }
}

impl fmt::Debug for ComponentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ComponentId")
            .field(&self.value())
            .finish()
    }
}

/// Typed, non-owning identity for a component in a registry.
pub struct ComponentHandle<C> {
    id: ComponentId,
    marker: PhantomData<fn() -> C>,
}

impl<C> Copy for ComponentHandle<C> {}

impl<C> Clone for ComponentHandle<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> fmt::Debug for ComponentHandle<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ComponentHandle")
    }
}

impl<C> PartialEq for ComponentHandle<C> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<C> Eq for ComponentHandle<C> {}

impl<C> Hash for ComponentHandle<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<C> ComponentHandle<C> {
    pub(crate) const fn id(self) -> ComponentId {
        self.id
    }

    pub(crate) const fn from_id(id: ComponentId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    pub(crate) const fn from_raw_id(id: u64) -> Self {
        Self::from_id(ComponentId::from_raw(id))
    }

    pub(crate) const fn new(id: ComponentId) -> Self {
        Self::from_id(id)
    }

    /// Returns the opaque identity used by a native host to refer to this
    /// mounted component from a semantic View.
    #[must_use]
    pub fn raw_id(self) -> u64 {
        self.id.value()
    }
}
