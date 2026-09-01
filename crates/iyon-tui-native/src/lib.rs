#![recursion_limit = "512"]

//! Native bindings for the generic Iyon terminal UI framework.
//!
//! Application/session bindings live in the application repository. This
//! crate exports only the generic TUI bridge and its framework load probe.

mod content_ffi;
mod error;
mod sync;
mod tui;

pub(crate) use error::NativeError;
pub use sync::native_version;
pub use tui::tui_smoke;
