//! Narrow, deliberately unsupported cross-crate seam for `iyon-tui-native`.
//!
//! This module contains no implementation graph: every item is a re-export of
//! exactly one runtime operation or passive type the native crate links
//! against. The set is pinned by `bun run check:tui-binding` and must not
//! grow without a handoff amendment. In particular it never carries the
//! fluent View DSL or public renderer
//! projector extension ecosystem, or user callbacks into the hot pipeline.
//!
//! Lane layout follows handoff §5.3: validated structural inputs produce
//! retained nodes, validated state operations produce override records, and
//! validated content ingress drives Source storage. Only the host frame
//! coordinator reads products from all three together.

// STRUCTURE: validated kind + immutable fields + resolved retained handles
// → canonical retained nodes and persistent derivations.
#[cfg(feature = "native-host")]
pub use crate::content::diff::lower_diff_hunks;
pub use crate::content::diff::{
    DiffHunk, DiffLine, DiffLineNumber, DiffLineOffset, DiffLineTermination, DiffRange,
};
pub use crate::content::text::{FormatId, LanguageId, SemanticTag, TextOrigin};
pub use crate::presentation::api::grid::{GridCellSpec, GridTrack};
pub use crate::presentation::api::style::{
    AnsiColor, BorderEdges, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec, Insets,
    OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
    TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
};
#[cfg(feature = "native-host")]
pub use crate::presentation::api::text::NativeTextPage;
pub use crate::presentation::api::text::{HorizontalAlign, TextSpan, WrapMode};
#[cfg(feature = "native-host")]
pub use crate::presentation::api::view::NativeCommonPatch;
pub use crate::presentation::binding::{
    grid_cell_spec_column_span, grid_cell_spec_horizontal_align, grid_cell_spec_new,
    grid_cell_spec_row_span, grid_cell_spec_vertical_align, grid_track_content,
    grid_track_content_max, grid_track_fixed, grid_track_flex, grid_track_flex_max,
    text_span_plain, text_span_styled,
};
#[cfg(feature = "native-host")]
pub use crate::presentation::binding::{
    view_clamp_rows, view_downgrade, view_hanging, view_native_axis_from_children,
    view_native_axis_set_child, view_native_axis_splice, view_native_component,
    view_native_container, view_native_content_host, view_native_grid_final,
    view_native_grid_set_cell, view_native_patched, view_native_replace_at_path,
    view_native_state_attachment_id, view_native_state_attachment_ids, view_native_state_capable,
    view_native_text_final, view_native_with_content_attachment, view_native_with_state_attachment,
    view_spacer, view_styled_text, view_text, view_text_plain, view_try_replace_retained_children,
    view_try_retained_child, view_try_with_text_layout_patch, view_try_with_text_layout_patch_path,
    view_try_with_text_layout_patch_path_with_nodes, view_upgrade,
};
pub use crate::presentation::ir::View;
#[cfg(feature = "native-host")]
pub use crate::presentation::ir::{RetainedPathStep, WeakView};
pub use crate::theme::Theme;

// STATE: validated property operations + retained state identity → canonical
// sparse override records. Geometry/presentation patch vocabulary only;
// effect classification stays inside the core.
// CONTENT: validated borrowed bytes/records and immutable funnel config →
// Source storage and Connector control.
pub use crate::projection::SmoothConfig;

// OCCURRENCE UI acceptance seam. The native addon owns byte decoding and
// source qualification; the core owns typed operations and transactional
// desired-state installation. This does not expose renderer/View transport.
#[cfg(feature = "native-host")]
pub use crate::application::ui_resources::{SourceIdentity, UiResourceOwner};
pub use crate::occurrence::generated::{
    HandleKind, UI_ACK_HEADER_WORDS, UI_ACK_WORDS_PER_CREATED_HANDLE, UI_BATCH_HEADER_WORDS,
    UI_BATCH_MAGIC, UI_BATCH_VERSION, UI_HANDLE_WORDS, UiOpcode, ValueKind, property_descriptor,
    value_encoding, value_encoding_form,
};
pub use crate::occurrence::{
    Alignment, AlignmentAxis, AnimationState, ColorValue, CommitDetail, ConfigError, ControlConfig,
    ControlError, ControlKind, ControlState, Edges, EditorState, FunnelSpec, GlyphsValue, HostKind,
    HostNamespace, LayerValue, NodeKey, NodeRef, OccurrenceDocument, OwnershipMode, PropertyId,
    PropertyLayer, PropertyValue, ResourceKey, ResourceRef, RootConfig, RootRole, ScrollState,
    SizeMode, StyleValue, TextAttributes, UiAcknowledgement, UiCommit, UiHandle, UiOperation,
    UiOperationResult, UiRejection,
};

// HOST: desired publication, barriers, and native control integration.
pub use crate::content::text::{TextPart, TextRole, TextSelector};
pub use crate::controls::TextInput;
pub use crate::history::{History, HistoryLayout};
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
pub use crate::application::host::{
    HostCellStyle, HostHistory, HostScrollPane, HostTextInput, HostViewSlot, TuiHost,
};
#[cfg(feature = "native-host")]
pub use crate::application::view_state::HostViewState;
#[cfg(feature = "native-host")]
pub use crate::retained_state::geometry::{
    GeometryAlignment, ViewStateGeometryPatch, ViewStateGeometryProperty, ViewStateSizeMode,
};
#[cfg(feature = "native-host")]
pub use crate::retained_state::presentation::{
    ViewStatePresentationPatch, ViewStatePresentationProperty,
};

// Measurement seam. The native crate reports through these only; counters
// themselves remain core-owned behind the same feature gate.
#[cfg(feature = "perf-counters")]
pub use crate::perf::{Counter, add, inc, reset, snapshot};

// Style atom seam. Native ingress interns repeated color/key strings through
// the canonical core table instead of re-allocating per materialization.
#[cfg(feature = "native-host")]
pub use crate::theme::intern_style_atom;
