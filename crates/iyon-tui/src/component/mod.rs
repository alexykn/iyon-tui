mod capability;
mod graph;
mod id;
mod mount;
mod registry;
mod revision;
mod slot;
mod tick;

#[cfg(test)]
mod mount_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tick_tests;

use crate::presentation::View;

pub use capability::ComponentCx;
pub(crate) use graph::{MountGraph, MountNode};
pub use id::ComponentHandle;
pub(crate) use id::ComponentId;
pub(crate) use mount::{MountTransition, MountTransitions, MountedComponents};
pub(crate) use registry::{ComponentRegistry, ComponentSnapshot};
pub(crate) use revision::ComponentRevision;
pub(crate) use tick::{TickOutcome, TickScheduler};

/// Public retained-state rendering and capability declaration contract.
pub trait Component: 'static {
    fn view(&self) -> View;

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>)
    where
        Self: Sized,
    {
    }
}
