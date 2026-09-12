//! Generic terminal runtime implementation.
//!
//! The Rust crate is an unpublished implementation crate for the TypeScript
//! framework and its in-tree native addon. Rust applications do not author
//! semantic UI, controls, or renderers through this crate. The only public bridge is
//! [`binding`], a deliberately unsupported, operation-specific seam consumed by
//! `iyon-tui-native`; semantic construction and retained storage stay private.
//!
//! TypeScript callers use `@iyon/tui`. Native input routing, clocks, History,
//! layout, Unicode-safe painting, and the retained host kernel remain owned by
//! this crate.

pub(crate) mod application;
mod backend;
mod component;
pub(crate) mod content;
pub(crate) mod controls;
mod geometry;
pub(crate) mod history;
mod id;
pub(crate) mod interaction;
pub(crate) mod occurrence;
pub(crate) mod output;
#[cfg(feature = "perf-counters")]
#[doc(hidden)]
pub(crate) mod perf;
#[cfg(not(feature = "perf-counters"))]
mod perf;
mod physical;
pub(crate) mod presentation;
// Projection, source coordinates, and semantic text stay implementation-only
// even though their public item declarations are reused by the binding and
// by in-crate unit tests. Rust callers author against the TypeScript facade.
pub(crate) mod projection;
pub(crate) mod scene;
/// Source-rooted coordinates shared by semantic content projections.
mod stream;
mod terminal;
/// Complete generic semantic text IR, traversal, projectors, and renderers.
mod text;
pub(crate) mod theme;

pub(crate) use component::{Component, ComponentCx, ComponentHandle};
#[cfg(test)]
pub(crate) use content::diff::{DiffLineNumber, DiffLineOffset, DiffRange};
#[cfg(test)]
pub(crate) use content::text::{
    Block, HeadingLevel, Inline, InlineContent, MarkdownOptions, MarkdownProjector,
    PlainTextProjector, RawText, TextOrigin, TextSelector,
};
pub(crate) use content::text::{
    CodeBlockLabelPolicy, SoftBreakPolicy, TableColumnSizing, TaskListMarkerPolicy, TextContent,
    TextRenderPolicy,
};
pub(crate) use controls::TextInput;
#[cfg(test)]
pub(crate) use history::HistoryLayout;
pub(crate) use history::{History, HistoryUnitId};
pub(crate) use interaction::{InteractionResult, Key, KeyStroke, Modifiers};
pub(crate) use output::{EventCx, Output, OutputRouter, RouteConflict};
#[cfg(test)]
pub(crate) use projection::{Projection, Projector, ProjectorExt, Smooth, SmoothConfig};
pub(crate) use theme::Theme;

pub(crate) use presentation::api::style::Insets;
#[allow(unused_imports)]
pub(crate) use presentation::api::{
    AnsiColor, BorderEdges, BorderSpec, ColorSpec, HorizontalAlign, StyleRef, StyleSelector,
    StyleSpec, StyleStateKey, StyleStateValue, TextAttribute, TextSpan, ThemeColor, WrapMode,
};
// Internal modules may use the short names without making implementation
// machinery part of the external crate-root vocabulary.
#[allow(unused_imports)]
pub(crate) use content::text::{
    Alignment, Annotations, BlockKind, BreakKind, CodeBlock, Image, InlineKind, LinkTarget, List,
    ListItem, ListMarker, LiteralText, Mark, MarkSet, NumberDelimiter, NumberStyle, SemanticKey,
    SemanticValue, TextIrError, TextProjectionError, TextProvenance, TextRewriter, TextRun,
    TextVisitor, validate_text_content, validate_text_projection, walk_block, walk_content,
    walk_inline, walk_inline_content, walk_literal, walk_rewrite_block, walk_rewrite_blocks,
    walk_rewrite_content, walk_rewrite_inline, walk_rewrite_inline_content, walk_rewrite_literal,
};
#[allow(unused_imports)]
pub(crate) use projection::{
    ProjectionBuilder, ProjectionRelationError, ProjectionSpan, ProjectionTransitionError,
    ProjectionValidationError, SmoothConfigError, Then, ThenError, validate_projection_relation,
    validate_projection_transition,
};

/// Narrow, deliberately unsupported cross-crate seam for the in-tree native
/// binding (`iyon-tui-native`). `pub` only because the native crate must link
/// to it — not an invitation to author UI against Rust. The export set is
/// exactly the runtime operations and passive types native callers need, and
/// is pinned by `bun run check:tui-binding`. See the pre-V5 handoff §5.
pub mod binding;
