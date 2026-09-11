//! Semantic construction facade.
//!
//! Consumers construct the canonical owned View IR through these types
//! without depending on retained structural implementation details.

#[doc(hidden)]
pub mod grid;
pub mod style;
pub mod text;
#[doc(hidden)]
pub mod view;

pub(crate) use super::ir::View;
pub(crate) use grid::{GridCellSpec, GridTrack};
pub use style::{
    AnsiColor, BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec,
    Insets, OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
    TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
};
pub(crate) use style::{StyleFacts, StyleStates};
pub(crate) use text::{HorizontalAlign, TextSpan, WrapMode};
