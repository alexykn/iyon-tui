//! Narrow, deliberately unsupported cross-crate seam for `iyon-tui-native`.
//!
//! This module contains no implementation graph: every item is a re-export of
//! exactly one runtime operation or passive type the native crate links
//! against. The set is pinned by `bun run check:tui-binding` and must not
//! grow without a handoff amendment. In particular it never carries the
//! fluent View DSL, `IntoView` as an authoring promise, a public renderer or
//! projector extension ecosystem, or user callbacks into the hot pipeline.
//!
//! Lane layout follows handoff §5.3: validated structural inputs produce
//! retained nodes, validated state operations produce override records, and
//! validated content ingress drives Source storage. Only the host frame
//! coordinator reads products from all three together.

// STRUCTURE: validated kind + immutable fields + resolved retained handles
// → canonical retained nodes and persistent derivations.
pub use crate::text::{FormatId, LanguageId, SemanticTag, TextOrigin};
pub use crate::{
    AnsiColor, BorderEdges, BorderGlyphs, BorderSpec, ColorSpec, DiffHunk, DiffLine,
    DiffLineNumber, DiffLineOffset, DiffLineTermination, DiffRange, DiffRenderer, GridCellSpec,
    GridTrack, HorizontalAlign, Insets, IntoView, OverflowIndicator, Renderer, StyleRef, StyleSpec,
    TextAttribute, TextSpan, Theme, ThemeColor, VerticalAlign, View, WrapMode,
};

// STATE: validated property operations + retained state identity → canonical
// sparse override records. Geometry/presentation patch vocabulary only;
// effect classification stays inside the core.
pub use crate::{BorderStyle, StyleSelector};

// CONTENT: validated borrowed bytes/records and immutable funnel config →
// Source storage and Connector control.
pub use crate::SmoothConfig;

// HOST: desired publication, barriers, and native control integration.
pub use crate::{
    History, HistoryLayout, Key, KeyStroke, Modifiers, Output, TextInput, TextPart, TextRole,
    TextSelector,
};

// Native-host seam. These mirror the `native-host` gates on the crate root:
// the native crate always enables the feature, while featureless core builds
// must not see host integration vocabulary.
#[cfg(feature = "native-host")]
pub use crate::{
    ContentAnnotationRecord, ContentDelivery, ContentFamily, ContentMutationResult,
    GeometryAlignment, HostCellStyle, HostContentConnector, HostContentFunnel, HostContentPort,
    HostContentSource, HostHistory, HostScrollPane, HostTextInput, HostViewSlot, HostViewState,
    RetainedPathStep, TextFunnelKind, TextSourceKind, TextWrapMode, TuiEnvironment, TuiHost,
    ViewStateGeometryPatch, ViewStateGeometryProperty, ViewStatePresentationPatch,
    ViewStatePresentationProperty, ViewStateSizeMode, ViewStateTextAttributes, WakeDisposition,
    WeakView,
};

// Measurement seam. The native crate reports through these only; counters
// themselves remain core-owned behind the same feature gate.
#[cfg(feature = "perf-counters")]
pub use crate::perf::{Counter, add, inc, reset, snapshot};
