//! Native host runtime and its retained content/state integration.
//!
//! Rust does not expose a generic application-authoring loop here. The native
//! host owns the concrete runtime, while TypeScript supplies application
//! composition through the binding seam.

#[cfg(feature = "native-host")]
pub(crate) mod content;
#[cfg(feature = "native-host")]
pub(crate) mod environment;
#[cfg(feature = "native-host")]
pub(crate) mod host;
#[cfg(feature = "native-host")]
mod input;
#[cfg(feature = "native-host")]
mod kernel;
#[cfg(feature = "native-host")]
mod run;
#[cfg(feature = "native-host")]
mod source_store;
#[cfg(feature = "native-host")]
pub(crate) mod ui_resources;
#[cfg(feature = "native-host")]
pub(crate) mod view_state;
