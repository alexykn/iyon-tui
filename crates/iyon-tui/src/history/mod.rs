//! Optional ordered semantic history capability.
//!
//! History is deliberately independent from ordinary View-based applications.
//! It owns ordered semantic lifetime, semantic layout, and the private native
//! durability frontier for callers that explicitly use native history. It does
//! not own terminal/backend implementation details or terminal writes.

mod boundary;
mod error;
mod id;
mod layout;
mod model;
mod native;
mod projection;
mod stream;
pub(crate) mod trace;
mod unit;

#[cfg(test)]
mod tests;

pub use boundary::FlowBoundary;
pub use error::HistoryError;
pub use id::HistoryUnitId;
pub use layout::HistoryLayout;
pub use model::History;
#[cfg(test)]
pub(crate) use native::transfer_native_prefix;
pub(crate) use native::{
    NativeTransferError, NativeTransferStatus, transfer_native_prefix_with_theme,
};
#[allow(unused_imports)]
pub(crate) use projection::{
    HistoryPhysicalOverlay, HistoryViewportAnchor, project_into_session_for_host,
    project_into_session_for_host_with_content,
};

#[cfg(test)]
pub(crate) use projection::project_with_anchor;
pub(crate) use stream::ErasedHistoryStream;
pub use stream::HistoryStreamHandle;
pub(crate) use unit::{HistoryUnit, HistoryUnitContent};
