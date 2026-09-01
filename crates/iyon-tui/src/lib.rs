//! Semantic terminal UI construction.
//!
//! [`View`] is an owned backend-neutral presentation value. [`Component`] adds
//! retained mounted state, [`History`] owns ordered historical/live/stream
//! content, and [`Scene`] is the terminal semantic root.
//!
//! Semantic text is claimed by a source projector and then lowered to [`View`]:
//!
//! ```text
//! Raw TextContent
//!     ↓ source projector (Markdown, PlainText, future formats)
//! semantic TextContent
//!     ↓ optional semantic rewrites
//! TextRenderer
//!     ↓
//! View
//!     ↓
//! layout / Theme / paint
//! ```
//!
//! Semantic diffs lower through the same View pipeline:
//!
//! ```text
//! DiffHunk / DiffLine
//!     ↓
//! DiffRenderer
//!     ↓
//! View
//!     ↓
//! layout / Theme / paint
//! ```
//!
//! ```
//! use iyon_tui::{
//!     DiffHunk, DiffLine, DiffLineNumber, DiffLineOffset, DiffRange, DiffRenderer, Renderer,
//! };
//!
//! let range = DiffRange::new(DiffLineOffset::new(0), 1).unwrap();
//! let line = DiffLine::context(
//!     DiffLineNumber::new(1).unwrap(),
//!     DiffLineNumber::new(1).unwrap(),
//!     "unchanged",
//! );
//! let hunk = DiffHunk::new(range, range, [line]).unwrap();
//! let view = DiffRenderer::new().render(&hunk);
//! # let _ = view;
//! ```
//!
//! Markdown is one projector, not the text model. [`MarkdownOptions::gfm`]
//! enables the supported GFM extensions (tables, strikethrough, and task lists);
//! [`MarkdownOptions::default`] remains strict CommonMark. [`TextRenderer`] is
//! source-format independent: it emits structure and semantic identity, never
//! application paint. [`TextSelector`] themes semantic roles and generated
//! parts. Origin specialization is optional. Terminal geometry remains below
//! the renderer in the View pipeline.
//!
//! Styling has three channels:
//!
//! - inherited runtime/application context (style state)
//! - node-local semantic identity (roles, parts, annotations)
//! - resolved physical paint, which inherits normally
//!
//! A heading can therefore be presented as `plain` while nested strong text
//! remains bold: semantic identity is local, physical style inherits.
//!
//! ```
//! use iyon_tui::{
//!     CodeBlockLabelPolicy, ColorSpec, MarkdownOptions, MarkdownProjector, StyleSpec,
//!     TaskListMarkerPolicy, TextPart, TextRenderPolicy, TextRenderer, TextSelector, Theme,
//! };
//!
//! let theme = Theme::new()
//!     .with_text_style(
//!         TextSelector::heading(),
//!         StyleSpec::new().foreground(ColorSpec::theme("heading")),
//!     )
//!     .with_text_style(
//!         TextSelector::part(TextPart::CodeLabel),
//!         StyleSpec::new().dim(),
//!     );
//!
//! let markdown = MarkdownProjector::new(MarkdownOptions::gfm());
//!
//! let renderer = TextRenderer::with_policy(
//!     TextRenderPolicy::new()
//!         .with_task_list_marker(TaskListMarkerPolicy::TaskOnly)
//!         .with_code_block_label(CodeBlockLabelPolicy::Language),
//! );
//! # let _ = (theme, markdown, renderer);
//! ```
//!
//! A basic History stream uses [`TextStream`] and the lifecycle
//! `push_stream -> update_stream -> seal_stream`; an open stream must remain
//! the History tail. History does not know Markdown or other text formats.
//!
//! Semantic plain text uses the same shape with [`PlainTextProjector`]. Custom
//! semantic transformations can be composed through [`ProjectorExt`], while
//! [`text::TextRewriter::into_projector`] is the
//! envelope-preserving adapter for ordinary IR rewrites. Nested literal portals
//! and `CodeBlock::language` leave room for future projectors without adding
//! format-specific machinery here.
//!
//! [`Smooth`] is optional temporal publication control: `Projection<T> ->
//! Smooth<T> -> next projector`. Its pacing granularity is determined by the
//! upstream spans and values; it is not required by Markdown.
//!
//! Advanced compiler and protocol machinery is organized under [`text`],
//! [`projection`], and [`stream`]. Root coordinates are source coordinates,
//! never terminal positions. Applications do not perform terminal geometry;
//! renderers lower semantics to `View`, and the View pipeline owns layout.
//!
//! ```compile_fail
//! use iyon_tui::presentation::ir::ViewKind;
//! ```
//!
//! ```compile_fail
//! use iyon_tui::{IntoView, View};
//!
//! let view = View::text("x").into_view();
//! let _ = view.kind;
//! ```
//!
//! ```compile_fail
//! use iyon_tui::{Decoration, RowChild, WidthRule};
//! ```
//!
//! ```compile_fail
//! use iyon_tui::View;
//!
//! let _ = View::text("x").container().no_wrap();
//! ```
//!
//! ```compile_fail
//! use iyon_tui::Horizontal;
//!
//! let _ = Horizontal::new();
//! ```
//!
//! ```compile_fail
//! use iyon_tui::Grid;
//!
//! let _ = Grid::new();
//! ```

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
/// StreamingSource protocol, snapshots, and provenance-aware stream values.
pub mod stream;
mod terminal;
#[cfg(feature = "test-util")]
pub mod testing;
/// Complete generic semantic text IR, traversal, projectors, and renderers.
pub mod text;
mod theme;

pub use application::{
    App, AppClosed, AppCx, AppHandle, AppSendError, RunError, RuntimeError, TimerHandle,
};

#[cfg(feature = "native-host")]
pub use application::{
    ContentFamily, HostCellStyle, HostCommit, HostContentConnector, HostContentFunnel,
    HostContentPort, HostContentSource, HostDrainReport, HostEpochs, HostFrameError, HostHistory,
    HostScrollPane, HostTextInput, HostTextStream, HostViewSlot, HostViewState, RoutedOutput,
    TextFunnelKind, TextSourceKind, TextStreamAnnotation, TextStreamPresentation, TextWrapMode,
    TuiEnvironment, TuiHost, WakeDisposition,
};

pub use component::{Component, ComponentCx, ComponentHandle};
pub use content::Renderer;
pub use content::diff::{
    DiffHunk, DiffLine, DiffLineKind, DiffLineNumber, DiffLineOffset, DiffLineTermination,
    DiffRange, DiffRenderer, DiffValidationError,
};
pub use content::text::{
    Block, CodeBlockLabelPolicy, HeadingLevel, Inline, InlineContent, MarkdownOptions,
    MarkdownProjector, PlainTextProjector, RawText, SoftBreakPolicy, TableColumnSizing,
    TaskListMarkerPolicy, TextContent, TextListKind, TextOrigin, TextPart, TextRenderPolicy,
    TextRenderer, TextRole, TextSelector, TextTableSection, TextTaskState,
};
pub use controls::{TextChange, TextInput};
pub use history::{
    FlowBoundary, History, HistoryError, HistoryLayout, HistoryStreamHandle, HistoryUnitId,
};
pub use interaction::{InteractionResult, Key, KeyStroke, MediaKey, ModifierKey, Modifiers};
pub use output::{EventCx, Output, OutputRouter, RouteConflict};
pub use projection::{Projection, Projector, ProjectorExt, Smooth, SmoothConfig};
#[cfg(feature = "native-host")]
#[doc(hidden)]
pub use retained_state::{
    GeometryAlignment, ViewStateGeometryPatch, ViewStateGeometryProperty,
    ViewStatePresentationPatch, ViewStatePresentationProperty, ViewStateSizeMode,
    ViewStateTextAttributes,
};
pub use scene::Scene;
pub use scroll::ScrollPane;
pub use theme::Theme;

pub use presentation::api::{
    AnsiColor, BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec,
    Grid, GridCellSpec, GridRow, GridTrack, Horizontal, HorizontalAlign, Insets, IntoView,
    OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue, Text,
    TextAttribute, TextAttributeSpec, TextSpan, ThemeColor, ThemeKey, Vertical, VerticalAlign,
    View, WrapMode,
};
#[cfg(feature = "native-host")]
#[doc(hidden)]
pub use presentation::ir::RetainedPathStep;

#[cfg(feature = "native-host")]
pub use presentation::ir::WeakView;
pub use stream::{StreamPane, TextStream};

// Internal modules and unit tests may use the short names without making
// protocol/compiler machinery part of the external crate-root vocabulary.
#[allow(unused_imports)]
pub(crate) use content::text::{
    Alignment, Annotations, BlockKind, BreakKind, CodeBlock, FormatId, Image, InlineKind,
    LanguageId, LinkTarget, List, ListItem, ListMarker, LiteralText, Mark, MarkSet,
    NumberDelimiter, NumberStyle, SemanticKey, SemanticTag, SemanticValue, TextIrError,
    TextProjectionError, TextProvenance, TextRewriter, TextRun, TextVisitor, validate_text_content,
    validate_text_projection, walk_block, walk_content, walk_inline, walk_inline_content,
    walk_literal, walk_rewrite_block, walk_rewrite_blocks, walk_rewrite_content,
    walk_rewrite_inline, walk_rewrite_inline_content, walk_rewrite_literal,
};
#[allow(unused_imports)]
pub(crate) use projection::{
    ProjectionBuilder, ProjectionRelationError, ProjectionSpan, ProjectionTransitionError,
    ProjectionValidationError, SmoothConfigError, Then, ThenError, validate_projection_relation,
    validate_projection_transition,
};
#[allow(unused_imports)]
pub(crate) use stream::{
    ProjectedText, ProjectedTextBuilder, ProjectedValidationError, StreamError, StreamOffset,
    StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder, StreamValidationError,
    StreamingSource,
};

/// Small, application-oriented import set.
pub mod prelude {
    pub use crate::{
        App, AppCx, Block, Component, ComponentCx, DiffHunk, DiffLine, DiffLineKind,
        DiffLineNumber, DiffLineOffset, DiffLineTermination, DiffRange, DiffRenderer, EventCx,
        History, HistoryLayout, Inline, InlineContent, IntoView, MarkdownProjector, Output,
        PlainTextProjector, Projection, Projector, ProjectorExt, Renderer, Scene, ScrollPane,
        Smooth, StreamPane, TextContent, TextInput, TextOrigin, TextRenderPolicy, TextRenderer,
        TextSelector, TextStream, Theme, View,
    };
}
