//! Direct terminal presentation boundaries.
//!
//! Occurrence geometry is adapted by Taffy, semantic text is lowered by the
//! terminal content projector, and paint helpers resolve styles and
//! decoration. No generic semantic UI tree is retained here.

pub mod api;
pub(crate) mod content;
pub(crate) mod direct;
pub(crate) mod direct_tree;
pub(crate) mod paint;
pub(crate) mod taffy;
pub(crate) mod wrap;

#[allow(unused_imports)]
pub(crate) use api::{
    BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec,
    HorizontalAlign, Insets, OverflowIndicator, StyleRef, StyleSpec, StyleStateKey,
    StyleStateValue, TextAttribute, TextAttributeSpec, TextSpan, ThemeKey, VerticalAlign, WrapMode,
};
pub(crate) use api::{StyleFacts, StyleStates};

pub(crate) use content::{
    ContentDirty, ContentDirtyReason, ContentMeasurement, ContentMeasurementCapture,
    ContentProvider, ContentWidthRule, ContentWindow, HistoryContentRows,
    HistoryMeasurementAdjustment, PreparedProjectionTicket,
};
