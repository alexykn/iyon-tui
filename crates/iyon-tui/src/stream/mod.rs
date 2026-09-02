//! Source-rooted UTF-8 coordinates shared by semantic content projections.
//!
//! Mutable content ownership lives in the Source/Funnel/Connector/ContentPort
//! runtime. This module contains only the generic coordinate values used by
//! projection envelopes; it is not a streaming runtime or viewport.

mod coord;

pub use coord::{StreamOffset, StreamRange};
