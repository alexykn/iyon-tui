//! Typed semantic output channels and deferred event routing.
//!
//! This module owns the generic, backend-neutral boundary between component
//! event emission and application action mapping. It intentionally does not
//! know about components, UI layout, input, or runtime state.

mod event;
mod handle;
mod router;

#[cfg(test)]
mod tests;

pub use event::EventCx;
pub(crate) use event::OutputQueue;
pub use handle::Output;
pub(crate) use router::OutputDispatchError;
pub use router::{OutputRouter, RouteConflict};
