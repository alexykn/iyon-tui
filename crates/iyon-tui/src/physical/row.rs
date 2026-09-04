//! Immutable final physical rows.

use super::PhysicalCell;
#[cfg(any(test, feature = "test-util"))]
use super::PhysicalStyle;
#[cfg(any(test, feature = "test-util"))]
use super::glyph::cell_x_of;
use super::glyph::{CellGeometryError, PhysicalGlyph, glyphs, place_glyphs, validate_cells};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalRow {
    cells: Vec<PhysicalCell>,
}

impl PhysicalRow {
    pub(crate) fn from_cells(cells: Vec<PhysicalCell>) -> Self {
        let row = Self { cells };
        debug_assert!(
            row.validate_cell_geometry().is_ok(),
            "PhysicalRow constructed with invalid wide-cell geometry: {:?}",
            row.validate_cell_geometry()
        );
        row
    }

    /// Place this row into a `width`-cell destination starting at `left`.
    ///
    /// Wide glyphs that would not fully fit are omitted entirely. Cell slicing
    /// (`cells[..copy_len]`) can keep a leader without its continuation.
    pub(crate) fn placed(&self, width: u16, left: u16) -> Self {
        self.place(width, left).0
    }

    pub(crate) fn place(&self, width: u16, left: u16) -> (Self, bool) {
        let mut cells = vec![PhysicalCell::transparent(); usize::from(width)];
        let complete = place_glyphs(&self.cells, &mut cells, usize::from(left));
        (Self { cells }, complete)
    }

    pub(crate) fn empty() -> Self {
        Self::from_cells(Vec::new())
    }

    pub(crate) fn width(&self) -> usize {
        self.cells.len()
    }

    /// Columns through the last painted cell, including left padding if this
    /// row was `placed` into a wider destination.
    pub(crate) fn occupied_width(&self) -> usize {
        self.cells
            .iter()
            .rposition(|cell| cell.painted)
            .map_or(0, |index| index + 1)
    }

    pub(crate) fn cells(&self) -> &[PhysicalCell] {
        &self.cells
    }

    pub(crate) fn cell(&self, index: usize) -> Option<&PhysicalCell> {
        self.cells.get(index)
    }

    pub(crate) fn glyphs(&self) -> impl Iterator<Item = PhysicalGlyph<'_>> {
        glyphs(&self.cells)
    }

    pub(crate) fn validate_cell_geometry(&self) -> Result<(), CellGeometryError> {
        validate_cells(&self.cells)
    }

    pub(crate) fn plain_text(&self) -> String {
        let last_painted = self
            .cells
            .iter()
            .rposition(|cell| cell.painted && !cell.continuation);
        let Some(last_painted) = last_painted else {
            return String::new();
        };

        self.cells[..=last_painted]
            .iter()
            .filter_map(|cell| {
                if cell.continuation {
                    None
                } else {
                    Some(cell.grapheme.as_deref().unwrap_or(" "))
                }
            })
            .collect()
    }

    #[cfg(any(test, feature = "test-util"))]
    pub(crate) fn style_at(&self, index: usize) -> Option<PhysicalStyle> {
        self.cell(index).map(|cell| cell.style)
    }

    #[cfg(any(test, feature = "test-util"))]
    pub(crate) fn cell_x_of(&self, needle: &str) -> Option<usize> {
        cell_x_of(&self.cells, needle)
    }
}
