//! Backend-neutral component interaction vocabulary and routing internals.

mod command;
mod focus;
mod key;
mod result;
mod routing;

pub use command::ComponentCx;
pub(crate) use command::{ComponentCapabilities, MountedCapabilities};
pub(crate) use focus::FocusState;
pub use key::{Key, KeyStroke, MediaKey, ModifierKey, Modifiers};
pub use result::InteractionResult;
pub(crate) use routing::{route_key_local, route_paste, route_paste_interceptor};
