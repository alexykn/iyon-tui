//! Physical History export and receipt frontier.
//!

mod boundary;
mod error;
mod id;
mod model;
mod native;
pub(crate) mod trace;

pub use boundary::FlowBoundary;
pub use error::HistoryError;
pub use id::HistoryUnitId;
pub use model::History;
#[cfg(test)]
pub(crate) use model::HistoryLayout;
pub(crate) use model::HistoryUnitContent;
pub(crate) use native::{
    NativeTransferError, NativeTransferOutcome, NativeTransferPlan, NativeTransferStatus,
    commit_native_transfer_with_content, prepare_native_transfer_with_theme_and_content,
};
