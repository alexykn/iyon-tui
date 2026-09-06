//! Generic standalone application kernel.
//!
//! [`App`] owns application state and framework runtime state while [`AppCx`]
//! exposes the narrow capabilities available to `init` and `update`.
//! Components continue to handle local interaction; their typed outputs are
//! routed into the application's action queue.

mod app;
#[cfg(feature = "native-host")]
pub(crate) mod content;
mod context;
#[cfg(feature = "native-host")]
pub(crate) mod environment;
mod handle;
#[cfg(feature = "native-host")]
pub(crate) mod host;
mod input;
mod kernel;
mod run;
#[cfg(feature = "native-host")]
mod source_store;
#[cfg(test)]
#[path = "tests/driver.rs"]
mod test_driver;
mod timer;
#[cfg(feature = "native-host")]
pub(crate) mod view_state;

#[cfg(test)]
mod tests;

pub use app::App;
pub use context::AppCx;
#[cfg(test)]
pub use handle::AppHandle;
