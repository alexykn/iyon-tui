//! Backend-neutral physical presentation vocabulary.
//!
//! This layer contains resolved cell styles, mutable composition surfaces, and
//! immutable rows. It has no knowledge of semantic views, streams, themes, or
//! terminal backends.
//!
//! Terminal cell occupancy is defined here ([`grapheme_cell_width`]) so wrap,
//! paint, and lowering cannot disagree about how many columns a grapheme uses.

mod glyph;
mod row;
mod style;
mod surface;
mod text_metrics;

#[cfg(test)]
mod tests;

pub(crate) use glyph::{validate_cells, write_glyph_span};
pub(crate) use row::PhysicalRow;
pub(crate) use style::{AnsiColor, PhysicalColor, PhysicalStyle};
pub(crate) use surface::{PhysicalCell, Surface};
pub(crate) use text_metrics::{grapheme_cell_width, text_cell_width};
