//! Private presentation boundaries for semantic construction and lowering.
//!
//! `api` is the curated semantic construction facade. `ir` is the private
//! retained semantic tree. `layout` owns ordinary semantic compilation into
//! physical rows, `paint` resolves styles and decoration, and `wrap` owns
//! Unicode wrapping. Generic stream provenance is a sibling `stream` subsystem.

pub(crate) mod api;
pub(crate) mod content;
pub(crate) mod ir;
pub(crate) mod layout;
pub(crate) mod paint;
pub(crate) mod wrap;

#[allow(unused_imports)]
pub(crate) use api::{
    BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec, Grid,
    GridCellSpec, GridRow, GridTrack, Horizontal, HorizontalAlign, Insets, IntoView,
    OverflowIndicator, StyleRef, StyleSpec, Text, TextAttribute, TextAttributeSpec, TextSpan,
    ThemeKey, Vertical, VerticalAlign, View, WrapMode,
};
pub(crate) use api::{StyleFacts, StyleStates};

// Retained IR types remain private implementation details of the semantic
// layout engine.
pub(crate) use content::{ContentMeasurement, ContentProvider, EmptyContentProvider};
pub(crate) use ir::WidthRule;
