//! Generic semantic streaming sources and local stream panes.
//!
//! This namespace owns the `StreamingSource` protocol, source snapshots,
//! coordinate/provenance machinery, and width-independent stream builders. It
//! intentionally does not own Markdown, History ordering, native transfer, or
//! terminal geometry.

mod append;
mod text;
pub use text::TextStream;
mod compile;
mod coord;
mod model;
mod node;
mod projected;
mod resident;
mod snapshot;
mod source;
mod transfer;
mod validate;
mod viewport;

mod pane;

#[cfg(test)]
mod tests;

pub(crate) use compile::{
    CompiledStream, StreamAtomicId, StreamRowAnchor, StreamRowTransfer, compile_stream,
};
pub use coord::{StreamOffset, StreamRange, StreamRevision};
pub use model::StreamModelError as StreamError;
pub(crate) use model::{StreamModel, StreamModelError};
pub(crate) use node::{StreamNode, StreamView};

#[cfg(test)]
pub(crate) use node::StreamSliceError;
pub use pane::StreamPane;
#[cfg(test)]
pub(crate) use projected::{ExactTerminator, ProjectedTextRun};
pub use projected::{ProjectedHanging, ProjectedText, ProjectedTextBuilder};
pub(crate) use projected::{ProjectedTextLayout, projected_atoms};
pub use snapshot::{StreamSnapshot, StreamSnapshotBuilder};
pub use source::StreamingSource;
pub(crate) use transfer::{
    FrozenPhysicalRows, StreamPartialTransfer, StreamTransferPayload, plan_stream_transfer,
};
pub use validate::{ProjectedValidationError, StreamValidationError};
pub(crate) use viewport::{StreamRowIndex, build_index_from, reindex_in_place, window_view};

#[cfg(test)]
pub(crate) use viewport::{build_index_call_count, reset_build_index_call_count};
