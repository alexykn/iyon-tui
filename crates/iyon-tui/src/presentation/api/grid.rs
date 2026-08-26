//! Closure-scoped two-dimensional grid construction.
//!
//! [`Grid`] and [`GridRow`] exist only while their construction closure runs.
//! [`View::grid`] lowers them immediately into retained presentation IR.
//! Declared columns define the initial explicit tracks; additional columns
//! required by cell placement become implicit content-sized tracks. The same
//! principle applies to rows created by row spans.
//!
//! ```
//! use iyon_tui::{GridCellSpec, GridTrack, HorizontalAlign, View};
//!
//! let view = View::grid(|grid| {
//!     grid.columns([GridTrack::content(), GridTrack::flex()]);
//!     grid.column_gap(1);
//!
//!     grid.row(|row| {
//!         row.cell("Name");
//!         row.cell("Value");
//!     });
//!
//!     grid.row(|row| {
//!         row.cell("requests");
//!         row.cell_with(
//!             GridCellSpec::new().horizontal_align(HorizontalAlign::End),
//!             "42",
//!         );
//!     });
//! });
//! # let _ = view;
//! ```
//!
//! Spanning cells occupy multiple shared tracks, including internal gaps:
//!
//! ```
//! use iyon_tui::{GridCellSpec, GridTrack, HorizontalAlign, View};
//!
//! let view = View::grid(|grid| {
//!     grid.columns([
//!         GridTrack::content(),
//!         GridTrack::content(),
//!         GridTrack::flex(),
//!     ]);
//!     grid.row(|row| {
//!         row.cell_with(
//!             GridCellSpec::new()
//!                 .column_span(2)
//!                 .horizontal_align(HorizontalAlign::Center),
//!             "Spans two columns",
//!         );
//!         row.cell("tail");
//!     });
//! });
//! # let _ = view;
//! ```

use std::{collections::HashMap, num::NonZeroU16, sync::Arc};

use super::{style::VerticalAlign, text::HorizontalAlign, view::IntoView};
use crate::presentation::ir::{GridCellView, GridView, PersistentSeq, TrackSize, View};

/// A column or row track size. The underlying layout representation is private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridTrack {
    pub(crate) track: TrackSize,
}

impl GridTrack {
    pub const fn content() -> Self {
        Self {
            track: TrackSize::Content { max: None },
        }
    }

    pub const fn content_max(max: u16) -> Self {
        Self {
            track: TrackSize::Content { max: Some(max) },
        }
    }

    pub const fn fixed(size: u16) -> Self {
        Self {
            track: TrackSize::Fixed(size),
        }
    }

    pub const fn flex() -> Self {
        Self {
            track: TrackSize::Flex { min: 1 },
        }
    }

    pub const fn flex_max(max: u16) -> Self {
        Self {
            track: TrackSize::FlexMax { min: 1, max },
        }
    }
}

/// Placement and alignment for one grid cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridCellSpec {
    column_span: NonZeroU16,
    row_span: NonZeroU16,
    horizontal_align: HorizontalAlign,
    vertical_align: VerticalAlign,
}

impl Default for GridCellSpec {
    fn default() -> Self {
        Self::new()
    }
}

impl GridCellSpec {
    pub fn new() -> Self {
        Self {
            column_span: NonZeroU16::MIN,
            row_span: NonZeroU16::MIN,
            horizontal_align: HorizontalAlign::Start,
            vertical_align: VerticalAlign::Top,
        }
    }

    pub fn column_span(self, span: u16) -> Self {
        Self {
            column_span: NonZeroU16::new(span).expect("grid column span must be at least 1"),
            ..self
        }
    }

    pub fn row_span(self, span: u16) -> Self {
        Self {
            row_span: NonZeroU16::new(span).expect("grid row span must be at least 1"),
            ..self
        }
    }

    pub fn horizontal_align(self, align: HorizontalAlign) -> Self {
        Self {
            horizontal_align: align,
            ..self
        }
    }

    pub fn vertical_align(self, align: VerticalAlign) -> Self {
        Self {
            vertical_align: align,
            ..self
        }
    }
}

/// Closure-scoped capability for constructing two-dimensional composition.
///
/// The capability is consumed by [`View::grid`](crate::View::grid); it is not a
/// retained semantic node and cannot itself be converted into a `View`.
pub struct Grid {
    columns: Vec<GridTrack>,
    rows: Vec<PendingGridRow>,
    column_gap: u16,
    row_gap: u16,
}

struct PendingGridRow {
    track: GridTrack,
    cells: Vec<PendingGridCell>,
}

struct PendingGridCell {
    spec: GridCellSpec,
    view: View,
}

impl Grid {
    pub(super) fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            column_gap: 0,
            row_gap: 0,
        }
    }

    pub fn columns(&mut self, columns: impl IntoIterator<Item = GridTrack>) -> &mut Self {
        self.columns = columns.into_iter().collect();
        self
    }

    pub fn column_gap(&mut self, gap: u16) -> &mut Self {
        self.column_gap = gap;
        self
    }

    pub fn row_gap(&mut self, gap: u16) -> &mut Self {
        self.row_gap = gap;
        self
    }

    pub fn row(&mut self, build: impl FnOnce(&mut GridRow)) -> &mut Self {
        self.row_with(GridTrack::content(), build)
    }

    pub fn row_with(&mut self, track: GridTrack, build: impl FnOnce(&mut GridRow)) -> &mut Self {
        let mut row = GridRow { cells: Vec::new() };
        build(&mut row);
        self.rows.push(PendingGridRow {
            track,
            cells: row.cells,
        });
        self
    }

    pub(super) fn into_grid_view(self) -> GridView {
        lower_grid(self)
    }
}

/// Closure-scoped capability for constructing one grid source row.
pub struct GridRow {
    cells: Vec<PendingGridCell>,
}

impl GridRow {
    pub fn cell(&mut self, child: impl IntoView) -> &mut Self {
        self.cell_with(GridCellSpec::new(), child)
    }

    pub fn cell_with(&mut self, spec: GridCellSpec, child: impl IntoView) -> &mut Self {
        self.cells.push(PendingGridCell {
            spec,
            view: child.into_view(),
        });
        self
    }
}

fn lower_grid(grid: Grid) -> GridView {
    let mut columns: Vec<TrackSize> = grid.columns.into_iter().map(|track| track.track).collect();
    let mut occupied_until = vec![0usize; columns.len()];
    let mut rows = Vec::with_capacity(grid.rows.len());
    let mut cells = Vec::new();

    for (row_index, pending) in grid.rows.into_iter().enumerate() {
        rows.push(pending.track.track);
        for cell in pending.cells {
            let column_span = usize::from(cell.spec.column_span.get());
            let row_span = usize::from(cell.spec.row_span.get());
            let column = place_cell(row_index, column_span, &mut columns, &mut occupied_until);
            for occupied in occupied_until.iter_mut().skip(column).take(column_span) {
                *occupied = row_index.saturating_add(row_span);
            }
            cells.push(GridCellView {
                row: row_index,
                column,
                row_span: cell.spec.row_span.get(),
                column_span: cell.spec.column_span.get(),
                horizontal_align: cell.spec.horizontal_align,
                vertical_align: cell.spec.vertical_align,
                view: cell.view,
            });
        }
    }

    let needed_rows = occupied_until.iter().copied().max().unwrap_or(rows.len());
    while rows.len() < needed_rows {
        rows.push(TrackSize::Content { max: None });
    }

    debug_assert_grid_non_overlapping(&columns, &rows, &cells);

    let cell_indices = Arc::new(
        cells
            .iter()
            .enumerate()
            .map(|(index, cell)| ((cell.row, cell.column), index))
            .collect::<HashMap<_, _>>(),
    );
    GridView {
        columns: PersistentSeq::from_vec(columns),
        rows: PersistentSeq::from_vec(rows),
        column_gap: grid.column_gap,
        row_gap: grid.row_gap,
        cells: PersistentSeq::from_vec(cells),
        cell_indices,
    }
}

fn place_cell(
    row: usize,
    column_span: usize,
    columns: &mut Vec<TrackSize>,
    occupied_until: &mut Vec<usize>,
) -> usize {
    debug_assert!(column_span >= 1);
    let mut column = 0usize;
    loop {
        ensure_columns(column.saturating_add(column_span), columns, occupied_until);
        if (column..column.saturating_add(column_span)).all(|index| occupied_until[index] <= row) {
            return column;
        }
        column = column.saturating_add(1);
    }
}

fn ensure_columns(needed: usize, columns: &mut Vec<TrackSize>, occupied_until: &mut Vec<usize>) {
    while columns.len() < needed {
        columns.push(TrackSize::Content { max: None });
    }
    while occupied_until.len() < columns.len() {
        occupied_until.push(0);
    }
}

fn debug_assert_grid_non_overlapping(
    columns: &[TrackSize],
    rows: &[TrackSize],
    cells: &[GridCellView],
) {
    let mut occupied = vec![vec![false; columns.len()]; rows.len()];
    for cell in cells {
        debug_assert!(cell.column_span >= 1, "cell span >= 1");
        debug_assert!(cell.row_span >= 1, "cell span >= 1");
        let column_end = cell.column.saturating_add(usize::from(cell.column_span));
        let row_end = cell.row.saturating_add(usize::from(cell.row_span));
        debug_assert!(cell.column < columns.len(), "cell start within columns");
        debug_assert!(cell.row < rows.len(), "cell start within rows");
        debug_assert!(column_end <= columns.len(), "cell end <= columns");
        debug_assert!(row_end <= rows.len(), "cell end <= rows");
        for occupied_row in occupied[cell.row..row_end].iter_mut() {
            for slot in occupied_row[cell.column..column_end].iter_mut() {
                debug_assert!(!*slot, "overlapping grid cells");
                *slot = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::View;
    use crate::presentation::ir::{HeightRule, ViewKind, WidthRule};

    fn grid_ir(view: &View) -> &GridView {
        let ViewKind::Grid(grid) = view.kind() else {
            panic!("expected grid view");
        };
        grid
    }

    #[test]
    fn default_grid_is_empty_fit() {
        let view = View::grid(|_| {});
        assert_eq!(view.width(), WidthRule::Fit);
        assert_eq!(view.height(), HeightRule::Fit);
        let grid = grid_ir(&view);
        assert!(grid.columns.is_empty());
        assert!(grid.rows.is_empty());
        assert!(grid.cells.is_empty());
        assert_eq!(grid.column_gap, 0);
        assert_eq!(grid.row_gap, 0);
    }

    #[test]
    fn explicit_tracks_lower_to_retained_sizes() {
        let view = View::grid(|grid| {
            grid.columns([GridTrack::content(), GridTrack::fixed(4), GridTrack::flex()]);
            grid.row(|_| {});
            grid.row_with(GridTrack::fixed(3), |_| {});
        });
        let grid = grid_ir(&view);
        assert_eq!(
            grid.columns.iter().copied().collect::<Vec<_>>(),
            vec![
                TrackSize::Content { max: None },
                TrackSize::Fixed(4),
                TrackSize::Flex { min: 1 },
            ]
        );
        assert_eq!(
            grid.rows.iter().copied().collect::<Vec<_>>(),
            vec![TrackSize::Content { max: None }, TrackSize::Fixed(3),]
        );
    }

    #[test]
    fn last_columns_declaration_wins() {
        let view = View::grid(|grid| {
            grid.columns([GridTrack::fixed(1)]);
            grid.columns([GridTrack::content(), GridTrack::flex()]);
        });
        assert_eq!(
            grid_ir(&view).columns.iter().copied().collect::<Vec<_>>(),
            vec![TrackSize::Content { max: None }, TrackSize::Flex { min: 1 }]
        );
    }

    #[test]
    fn basic_auto_placement_fills_rows_in_source_order() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell("A");
                row.cell("B");
            });
            grid.row(|row| {
                row.cell("C");
                row.cell("D");
            });
        });
        let cells = &grid_ir(&view).cells;
        assert_eq!(cells.len(), 4);
        assert_eq!((cells[0].row, cells[0].column), (0, 0));
        assert_eq!((cells[1].row, cells[1].column), (0, 1));
        assert_eq!((cells[2].row, cells[2].column), (1, 0));
        assert_eq!((cells[3].row, cells[3].column), (1, 1));
        assert_eq!(text(&cells[0].view), "A");
        assert_eq!(text(&cells[3].view), "D");
    }

    #[test]
    fn column_span_skips_occupied_columns_on_the_next_row() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell_with(GridCellSpec::new().column_span(2), "A");
                row.cell("B");
            });
            grid.row(|row| {
                row.cell("C");
                row.cell("D");
                row.cell("E");
            });
        });
        let grid = grid_ir(&view);
        assert_eq!(grid.columns.len(), 3);
        assert_eq!(
            (
                grid.cells[0].row,
                grid.cells[0].column,
                grid.cells[0].column_span
            ),
            (0, 0, 2)
        );
        assert_eq!((grid.cells[1].row, grid.cells[1].column), (0, 2));
        assert_eq!((grid.cells[2].row, grid.cells[2].column), (1, 0));
        assert_eq!((grid.cells[3].row, grid.cells[3].column), (1, 1));
        assert_eq!((grid.cells[4].row, grid.cells[4].column), (1, 2));
    }

    #[test]
    fn row_span_occupancy_skips_the_occupied_column() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell_with(GridCellSpec::new().row_span(2), "A");
                row.cell("B");
            });
            grid.row(|row| {
                row.cell("C");
            });
        });
        let cells = &grid_ir(&view).cells;
        assert_eq!(
            (cells[0].row, cells[0].column, cells[0].row_span),
            (0, 0, 2)
        );
        assert_eq!((cells[1].row, cells[1].column), (0, 1));
        assert_eq!((cells[2].row, cells[2].column), (1, 1));
        assert_eq!(text(&cells[2].view), "C");
    }

    #[test]
    fn combined_row_and_column_span_occupies_a_block() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell_with(GridCellSpec::new().column_span(2).row_span(2), "A");
                row.cell("B");
            });
            grid.row(|row| {
                row.cell("C");
            });
        });
        let grid = grid_ir(&view);
        assert_eq!(
            (
                grid.cells[0].row,
                grid.cells[0].column,
                grid.cells[0].row_span,
                grid.cells[0].column_span
            ),
            (0, 0, 2, 2)
        );
        assert_eq!((grid.cells[1].row, grid.cells[1].column), (0, 2));
        assert_eq!((grid.cells[2].row, grid.cells[2].column), (1, 2));
    }

    #[test]
    fn implicit_columns_extend_declared_tracks() {
        let view = View::grid(|grid| {
            grid.columns([GridTrack::fixed(3)]);
            grid.row(|row| {
                row.cell("A");
                row.cell("B");
                row.cell("C");
            });
        });
        let grid = grid_ir(&view);
        assert_eq!(grid.columns.len(), 3);
        assert_eq!(grid.columns[0], TrackSize::Fixed(3));
        assert_eq!(grid.columns[1], TrackSize::Content { max: None });
        assert_eq!(grid.columns[2], TrackSize::Content { max: None });
    }

    #[test]
    fn implicit_rows_cover_row_spans() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell_with(GridCellSpec::new().row_span(3), "A");
            });
        });
        let grid = grid_ir(&view);
        assert_eq!(grid.rows.len(), 3);
        assert!(
            grid.rows
                .iter()
                .all(|track| *track == TrackSize::Content { max: None })
        );
    }

    #[test]
    fn component_identity_is_owned_once_by_the_cell() {
        let child = View::native_component(1);
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell(child);
            });
        });
        assert!(view.contains_component_identity());
        let cloned = view.clone();
        let ViewKind::Grid(original) = view.kind() else {
            panic!("expected grid");
        };
        let ViewKind::Grid(clone) = cloned.kind() else {
            panic!("expected grid");
        };
        assert_eq!(original.cells.len(), 1);
        assert_eq!(clone.cells.len(), 1);
        assert_eq!(original.cells[0].view, clone.cells[0].view);
    }

    #[test]
    fn source_order_is_preserved_in_retained_cells() {
        let view = View::grid(|grid| {
            grid.row(|row| {
                row.cell_with(GridCellSpec::new().column_span(2), "first");
                row.cell("second");
            });
            grid.row(|row| {
                row.cell("third");
            });
        });
        let labels: Vec<_> = grid_ir(&view)
            .cells
            .iter()
            .map(|cell| text(&cell.view))
            .collect();
        assert_eq!(labels, ["first", "second", "third"]);
    }

    #[test]
    #[should_panic(expected = "grid column span must be at least 1")]
    fn zero_column_span_is_a_programmer_error() {
        let _ = GridCellSpec::new().column_span(0);
    }

    #[test]
    #[should_panic(expected = "grid row span must be at least 1")]
    fn zero_row_span_is_a_programmer_error() {
        let _ = GridCellSpec::new().row_span(0);
    }

    fn text(view: &View) -> &str {
        let ViewKind::Text(text) = view.kind() else {
            panic!("expected text");
        };
        text.spans[0].text()
    }
}
