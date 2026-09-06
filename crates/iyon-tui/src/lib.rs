//! Generic terminal runtime implementation.
//!
//! The Rust crate is an unpublished implementation crate for the TypeScript
//! framework and its in-tree native addon. Rust applications do not author
//! Views, controls, or renderers through this crate. The only public bridge is
//! [`binding`], a deliberately unsupported, operation-specific seam consumed by
//! `iyon-tui-native`; semantic construction and retained storage stay private.
//!
//! TypeScript callers use `@iyon/tui`. Native input routing, clocks, History,
//! layout, Unicode-safe painting, and the retained host kernel remain owned by
//! this crate.

mod application;
mod backend;
mod component;
mod content;
mod controls;
mod geometry;
mod history;
mod id;
mod interaction;
mod output;
#[cfg(feature = "perf-counters")]
#[doc(hidden)]
pub mod perf;
#[cfg(not(feature = "perf-counters"))]
mod perf;
#[cfg(feature = "perf-counters")]
#[doc(hidden)]
pub mod perf_bench;
mod physical;
mod presentation;
/// Root-coordinate projection algebra and diagnostics.
pub mod projection;
mod retained_state;
mod scene;
mod scroll;
mod scroll_command;
/// Source-rooted coordinates shared by semantic content projections.
pub mod stream;
mod terminal;
#[cfg(feature = "test-util")]
pub mod testing;
/// Complete generic semantic text IR, traversal, projectors, and renderers.
pub mod text;
mod theme;

#[cfg(feature = "native-host")]
pub use application::{
    ContentAnnotationRecord, ContentAnnotationSnapshot, ContentDelivery, ContentFamily,
    ContentMutationResult, HostCellStyle, HostCommit, HostContentConnector, HostContentFunnel,
    HostContentPort, HostContentSource, HostContentSourceSnapshot, HostContentSourceStats,
    HostDrainReport, HostEpochs, HostFrameError, HostHistory, HostScrollPane, HostTextInput,
    HostViewSlot, HostViewState, RoutedOutput, TextFunnelKind, TextSourceKind, TextWrapMode,
    TuiEnvironment, TuiHost, WakeDisposition,
};

// The application driver and presentation graph are runtime-only. Keep the
// short names available to the in-crate kernel while deliberately omitting
// them from the public crate surface.
pub(crate) use application::{App, AppCx, AppHandle, RunError, RuntimeError};

pub(crate) use component::{Component, ComponentCx, ComponentHandle};
pub use content::diff::{
    DiffHunk, DiffLine, DiffLineKind, DiffLineNumber, DiffLineOffset, DiffLineTermination,
    DiffRange, DiffValidationError,
};
pub use content::text::{
    AnsiOptions, AnsiProjector, Block, CodeBlockLabelPolicy, DiffProjector, FormatId, HeadingLevel,
    Inline, InlineContent, LanguageId, MarkdownOptions, MarkdownProjector, PlainTextProjector,
    RawText, SemanticTag, SoftBreakPolicy, TableColumnSizing, TaskListMarkerPolicy, TextContent,
    TextListKind, TextOrigin, TextPart, TextRenderPolicy, TextRole, TextSelector, TextTableSection,
    TextTaskState,
};
pub use controls::{TextChange, TextInput};
pub use history::{FlowBoundary, History, HistoryError, HistoryLayout, HistoryUnitId};
pub use interaction::{InteractionResult, Key, KeyStroke, MediaKey, ModifierKey, Modifiers};
pub use output::{EventCx, Output, OutputRouter, RouteConflict};
pub use projection::{Projection, Projector, ProjectorExt, Smooth, SmoothConfig};
#[cfg(feature = "native-host")]
#[doc(hidden)]
pub use retained_state::{
    GeometryAlignment, ViewStateGeometryPatch, ViewStateGeometryProperty,
    ViewStatePresentationPatch, ViewStatePresentationProperty, ViewStateSizeMode,
};
pub use scene::Scene;
pub use scroll::ScrollPane;
pub use theme::Theme;

pub use presentation::api::style::Insets;
#[allow(unused_imports)]
pub(crate) use presentation::api::{
    AnsiColor, BorderEdges, BorderSpec, ColorSpec, HorizontalAlign, StyleRef, StyleSelector,
    StyleSpec, StyleStateKey, StyleStateValue, TextAttribute, TextSpan, ThemeColor, WrapMode,
};
pub(crate) use presentation::api::{GridCellSpec, GridTrack};
pub(crate) use presentation::ir::View;
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
