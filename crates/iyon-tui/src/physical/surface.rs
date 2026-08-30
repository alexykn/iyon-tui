//! Mutable backend-neutral physical composition surface.

use crate::{
    geometry::Size,
    perf::{self, Counter},
};

use super::glyph::{clear_glyph_covering, glyphs, place_glyphs, validate_cells, write_glyph_span};
use super::{PhysicalColor, PhysicalStyle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalCell {
    pub(crate) grapheme: Option<String>,
    pub(crate) style: PhysicalStyle,
    pub(crate) painted: bool,
    pub(crate) continuation: bool,
}

impl PhysicalCell {
    pub(crate) fn transparent() -> Self {
        Self {
            grapheme: None,
            style: PhysicalStyle::default(),
            painted: false,
            continuation: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn blank(style: PhysicalStyle) -> Self {
        Self {
            grapheme: None,
            style,
            painted: true,
            continuation: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Surface {
    pub(crate) size: Size,
    pub(crate) cells: Vec<PhysicalCell>,
    pub(crate) physically_complete: bool,
}

impl Surface {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            size: Size { width, height },
            cells: vec![PhysicalCell::transparent(); usize::from(width) * usize::from(height)],
            physically_complete: true,
        }
    }

    pub(crate) fn width(&self) -> u16 {
        self.size.width
    }

    pub(crate) fn height(&self) -> u16 {
        self.size.height
    }

    fn index(&self, x: u16, y: u16) -> usize {
        usize::from(y) * usize::from(self.width()) + usize::from(x)
    }

    fn row_range(&self, y: u16) -> std::ops::Range<usize> {
        let width = usize::from(self.width());
        let start = usize::from(y) * width;
        start..start + width
    }

    fn row_cells(&self, y: u16) -> &[PhysicalCell] {
        &self.cells[self.row_range(y)]
    }

    fn row_cells_mut(&mut self, y: u16) -> &mut [PhysicalCell] {
        let range = self.row_range(y);
        &mut self.cells[range]
    }

    pub(crate) fn get(&self, x: u16, y: u16) -> &PhysicalCell {
        &self.cells[self.index(x, y)]
    }

    pub(crate) fn get_mut(&mut self, x: u16, y: u16) -> &mut PhysicalCell {
        let index = self.index(x, y);
        &mut self.cells[index]
    }

    pub(crate) fn apply_surface_background(&mut self, color: PhysicalColor) {
        for cell in &mut self.cells {
            if !cell.painted {
                cell.style = PhysicalStyle {
                    background: Some(color),
                    ..PhysicalStyle::default()
                };
                cell.grapheme = None;
                cell.continuation = false;
                cell.painted = true;
            } else if cell.style.background.is_none() {
                cell.style.background = Some(color);
            }
        }
    }

    /// Overlay `child` at `(x, y)` using whole-glyph writes.
    ///
    /// Cell-by-cell copy can produce `leader(🐕‍🦺) | X` when a child paints
    /// only the continuation column. Before writing a glyph we clear every
    /// destination glyph that intersects its span, matching termwiz
    /// `Line::set_cell`.
    pub(crate) fn composite(&mut self, child: &Self, x: u16, y: u16) {
        if !child.physically_complete {
            self.physically_complete = false;
        }
        let origin_x = usize::from(x);
        for child_y in 0..child.height() {
            let target_y = y.saturating_add(child_y);
            if target_y >= self.height() {
                if child.row_cells(child_y).iter().any(|cell| cell.painted) {
                    self.physically_complete = false;
                }
                continue;
            }
            let child_row = child.row_cells(child_y);
            for glyph in glyphs(child_row) {
                if !glyph.leader.painted {
                    continue;
                }
                let dest_start = origin_x.saturating_add(glyph.start);
                let dest_end = dest_start.saturating_add(glyph.width);
                if dest_start >= usize::from(self.width()) || dest_end > usize::from(self.width()) {
                    self.physically_complete = false;
                    break;
                }
                let dest_row = self.row_cells_mut(target_y);
                perf::add(Counter::SurfaceCellsComposited, glyph.width as u64);
                write_glyph_span(dest_row, dest_start, child_row, glyph.start, glyph.width);
            }
            debug_assert!(validate_cells(self.row_cells(target_y)).is_ok());
        }
    }

    /// Overlay `child` at a signed vertical offset while respecting an
    /// additional physical clip. This is the incremental counterpart to the
    /// full-tree RowViewport composition path.
    pub(crate) fn composite_clipped(
        &mut self,
        child: &Self,
        x: i32,
        y: i32,
        clip: crate::geometry::Rect,
    ) {
        if !child.physically_complete {
            self.physically_complete = false;
        }
        let clip_left = i32::from(clip.x);
        let clip_top = i32::from(clip.y);
        let clip_right = clip_left.saturating_add(i32::from(clip.width));
        let clip_bottom = clip_top.saturating_add(i32::from(clip.height));
        for child_y in 0..child.height() {
            let target_y = y.saturating_add(i32::from(child_y));
            if target_y < clip_top
                || target_y >= clip_bottom
                || target_y < 0
                || target_y >= i32::from(self.height())
            {
                continue;
            }
            let child_row = child.row_cells(child_y);
            for glyph in glyphs(child_row) {
                if !glyph.leader.painted {
                    continue;
                }
                let dest_start = x.saturating_add(glyph.start as i32);
                let dest_end = dest_start.saturating_add(glyph.width as i32);
                if dest_start < clip_left
                    || dest_end > clip_right
                    || dest_start < 0
                    || dest_end > i32::from(self.width())
                {
                    continue;
                }
                let dest_row = self.row_cells_mut(target_y as u16);
                perf::add(Counter::SurfaceCellsComposited, glyph.width as u64);
                write_glyph_span(
                    dest_row,
                    dest_start as usize,
                    child_row,
                    glyph.start,
                    glyph.width,
                );
            }
            debug_assert!(validate_cells(self.row_cells(target_y as u16)).is_ok());
        }
    }

    /// Crop to `width × height` without splitting a wide glyph on the right edge.
    #[allow(dead_code)]
    pub(crate) fn crop_to(&self, width: u16, height: u16) -> Self {
        let mut cropped = Self::new(width, height);
        cropped.physically_complete = self.physically_complete;
        let copy_height = height.min(self.height());
        for y in 0..copy_height {
            if !place_glyphs(self.row_cells(y), cropped.row_cells_mut(y), 0) {
                cropped.physically_complete = false;
            }
        }
        cropped
    }

    /// Clears a rectangular region before an incremental subtree composite.
    pub(crate) fn clear_rect(&mut self, rect: crate::geometry::Rect) {
        self.clear_rect_with_background(rect, None);
    }

    /// Clears a region while restoring the nearest retained ancestor surface
    /// background. Incremental component painting must not erase a parent
    /// background merely because the changed child has transparent cells.
    pub(crate) fn clear_rect_with_background(
        &mut self,
        rect: crate::geometry::Rect,
        background: Option<PhysicalColor>,
    ) {
        let right = rect.right().min(self.width());
        let bottom = rect.bottom().min(self.height());
        for y in rect.y.min(self.height())..bottom {
            for x in rect.x.min(self.width())..right {
                let cell = if let Some(color) = background {
                    PhysicalCell {
                        grapheme: None,
                        style: PhysicalStyle {
                            background: Some(color),
                            ..PhysicalStyle::default()
                        },
                        painted: true,
                        continuation: false,
                    }
                } else {
                    PhysicalCell::transparent()
                };
                *self.get_mut(x, y) = cell;
            }
        }
    }

    /// Clear the whole glyph covering `(x, y)` so a later 1-cell write cannot
    /// leave a leader claiming a continuation that now belongs to someone else.
    pub(crate) fn clear_glyph_at(&mut self, x: u16, y: u16) {
        if y >= self.height() || x >= self.width() {
            return;
        }
        let column = usize::from(x);
        clear_glyph_covering(self.row_cells_mut(y), column);
    }
}
