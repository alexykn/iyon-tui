//! Narrow, deliberately unsupported cross-crate seam for `iyon-tui-native`.
//!
//! This module contains no implementation graph: every item is a re-export of
//! exactly one runtime operation or passive type the native crate links
//! against. The set is pinned by `bun run check:tui-binding` and must not
//! grow without a handoff amendment. In particular it never carries the
//! fluent View DSL or public renderer
//! projector extension ecosystem, or user callbacks into the hot pipeline.
//!
//! Lane layout follows the direct-occurrence UI schema and the existing
//! content payload contract. Only the host frame coordinator reads products
//! from both together.

// STRUCTURE: validated kind + immutable fields + resolved retained handles
// → canonical retained nodes and persistent derivations.
#[cfg(feature = "native-host")]
pub use crate::content::diff::lower_diff_hunks;
pub use crate::content::diff::{
    DiffHunk, DiffLine, DiffLineNumber, DiffLineOffset, DiffLineTermination, DiffRange,
};
pub use crate::content::text::{FormatId, LanguageId, SemanticTag, TextOrigin};
pub use crate::presentation::api::style::{
    AnsiColor, BorderEdges, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec, Insets,
    OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
    TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
};
#[cfg(feature = "native-host")]
pub use crate::presentation::api::text::NativeTextPage;
pub use crate::presentation::api::text::{HorizontalAlign, TextSpan, WrapMode};
pub use crate::theme::Theme;

// CONTENT: validated borrowed bytes/records and immutable funnel config →
// Source storage and Connector control.
pub use crate::projection::SmoothConfig;

// OCCURRENCE UI acceptance seam. The native addon owns byte decoding and
// source qualification; the core owns typed operations and transactional
// desired-state installation. This does not expose renderer/View transport.
#[cfg(feature = "native-host")]
pub use crate::application::ui_resources::{SourceIdentity, UiCommitOutput, UiResourceOwner};
pub use crate::occurrence::generated::{
    HandleKind, UI_ACK_HEADER_WORDS, UI_ACK_WORDS_PER_CREATED_HANDLE, UI_BATCH_HEADER_WORDS,
    UI_BATCH_MAGIC, UI_BATCH_VERSION, UI_HANDLE_WORDS, UiOpcode, ValueKind, property_descriptor,
    value_encoding, value_encoding_form,
};
pub use crate::occurrence::{
    Alignment, AlignmentAxis, AlignmentMode, AnimationState, ColorValue, CommitDetail, ConfigError,
    ControlConfig, ControlError, ControlKind, ControlState, DimensionInsets, DimensionValue,
    DirectionMode, DisplayMode, Edges, EditorState, FiniteScalar, FlexDirectionMode, FlexWrapMode,
    FunnelSpec, GlyphsValue, GridAutoFlowMode, GridLineValue, GridPlacementValue, HostKind,
    HostNamespace, LayerValue, LayoutMode, NodeKey, NodeRef, OccurrenceDocument, OwnershipMode,
    PositionMode, PropertyId, PropertyLayer, PropertyValue, ResourceKey, ResourceRef, RootConfig,
    RootRole, ScrollState, SizeMode, StyleValue, TextAttributes, TrackListValue, TrackMaxBound,
    TrackMinBound, TrackValue, UiAcknowledgement, UiCommit, UiHandle, UiOperation,
    UiOperationResult, UiRejection,
};

// HOST: desired publication, barriers, and native control integration.
pub use crate::content::text::{TextPart, TextRole, TextSelector};
pub use crate::controls::TextInput;
pub use crate::interaction::{Key, KeyStroke, Modifiers};
pub use crate::output::Output;
// Native-host seam. These mirror the `native-host` gates on the crate root:
// the native crate always enables the feature, while featureless core builds
// must not see host integration vocabulary.
#[cfg(feature = "native-host")]
pub use crate::application::content::{
    ContentAnnotationRecord, ContentDelivery, ContentFamily, ContentMutationResult,
    HostContentConnector, HostContentFunnel, HostContentPort, HostContentSource,
    SourceInstallDisposition, TextFunnelKind, TextSourceKind, TextWrapMode,
};
#[cfg(feature = "native-host")]
pub use crate::application::environment::{TuiEnvironment, WakeDisposition};
#[cfg(feature = "native-host")]
pub use crate::application::host::{HostCellStyle, HostTextInput, TuiHost};

// Measurement seam. The native crate reports through these only; counters
// themselves remain core-owned behind the same feature gate.
#[cfg(feature = "perf-counters")]
pub use crate::perf::{Counter, add, inc, reset, snapshot};

// Style atom seam. Native ingress interns repeated color/key strings through
// the canonical core table instead of re-allocating per materialization.
#[cfg(feature = "native-host")]
pub use crate::theme::intern_style_atom;
