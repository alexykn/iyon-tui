//! Semantic construction facade.
//!
//! Consumers construct the canonical owned View IR through these types
//! without depending on retained structural implementation details.

mod composition;
mod grid;
pub(super) mod style;
pub(super) mod text;
mod view;

pub use super::ir::View;
pub use composition::{Horizontal, Vertical};
pub use grid::{Grid, GridCellSpec, GridRow, GridTrack};
pub use style::{
    AnsiColor, BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec,
    Insets, OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
    TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
};
pub(crate) use style::{StyleFacts, StyleStates};
pub use text::{HorizontalAlign, Text, TextSpan, WrapMode};
pub use view::IntoView;
