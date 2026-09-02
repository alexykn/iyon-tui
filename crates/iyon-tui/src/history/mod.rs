//! Optional ordered semantic history capability.
//!
//! History is deliberately independent from ordinary View-based applications.
//! It owns ordered semantic lifetime and layout, while the native durability
//! frontier consumes committed static/ContentHost rows. It does not own
//! terminal/backend implementation details or terminal writes.

mod boundary;
mod error;
mod id;
mod layout;
mod model;
mod native;
mod projection;
pub(crate) mod trace;
mod unit;

pub use boundary::FlowBoundary;
pub use error::HistoryError;
pub use id::HistoryUnitId;
pub use layout::HistoryLayout;
pub use model::History;
#[cfg(test)]
pub(crate) use native::transfer_native_prefix;
pub(crate) use native::{
    NativeTransferError, NativeTransferStatus, transfer_native_prefix_with_theme_and_content,
};
#[allow(unused_imports)]
pub(crate) use projection::{
    HistoryPhysicalOverlay, HistoryViewportAnchor, project_into_session_for_host,
    project_into_session_for_host_with_content,
};

pub(crate) use unit::{HistoryUnit, HistoryUnitContent};
