//! Source-format-independent semantic diff content and rendering.

mod model;

pub use model::{
    DiffHunk, DiffLine, DiffLineKind, DiffLineNumber, DiffLineOffset, DiffLineTermination,
    DiffRange,
};
