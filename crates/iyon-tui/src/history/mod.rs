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
pub(crate) use model::HistoryUnitContent;
pub use model::{History, HistoryLayout};
#[cfg(test)]
pub(crate) use native::transfer_native_prefix;
pub(crate) use native::{
    NativeTransferError, NativeTransferOutcome, NativeTransferPlan, NativeTransferStatus,
    commit_native_transfer_with_content, prepare_native_transfer_with_theme_and_content,
    transfer_native_prefix_with_theme_and_content,
};
