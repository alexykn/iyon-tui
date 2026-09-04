use std::{fmt, hash::Hash, marker::PhantomData, num::NonZeroU64, sync::atomic::AtomicU64};

use crate::id::next_nonzero_id;

static NEXT_OUTPUT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct OutputId(NonZeroU64);

impl OutputId {
    fn allocate() -> Self {
        Self(next_nonzero_id(&NEXT_OUTPUT_ID, "output id exhausted"))
    }
}

/// Opaque typed identity for a semantic output channel.
pub struct Output<T: 'static> {
    id: OutputId,
    marker: PhantomData<fn() -> T>,
}

impl<T: 'static> Output<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: OutputId::allocate(),
            marker: PhantomData,
        }
    }

    pub(super) const fn id(self) -> OutputId {
        self.id
    }
}

impl<T: 'static> Copy for Output<T> {}

impl<T: 'static> Clone for Output<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> PartialEq for Output<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: 'static> Eq for Output<T> {}

impl<T: 'static> Hash for Output<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: 'static> fmt::Debug for Output<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("Output").finish()
    }
}

impl fmt::Debug for OutputId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("OutputId").field(&self.0).finish()
    }
}
