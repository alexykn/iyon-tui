//! Finite style and text value types shared by native boundaries.

pub mod style;
pub mod text;
pub use style::{
    AnsiColor, BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec,
    Insets, OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
    TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
};
pub(crate) use style::{StyleFacts, StyleStates};
pub(crate) use text::{HorizontalAlign, TextSpan, WrapMode};
