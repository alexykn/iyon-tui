//! Two-dimensional grid placement and normalization.
//!
//! Owned grid records lower directly into retained presentation IR through
//! the internal factory seam; no closure-scoped authoring builder is exposed.
//! Declared columns define the initial explicit tracks; additional columns
//! required by cell placement become implicit content-sized tracks. The same
//! principle applies to rows created by row spans.
//!
//! Grid placement and normalization are private runtime algorithms. The
//! TypeScript facade supplies semantic grid records; the native binding moves
//! validated records into the retained grid product without exposing a Rust
//! authoring builder.

use std::{collections::HashMap, num::NonZeroU16, sync::Arc};

use super::{style::VerticalAlign, text::HorizontalAlign};
use crate::presentation::ir::{GridCellView, GridView, PersistentSeq, TrackSize, View, ViewKind};

/// A column or row track size. The underlying layout representation is private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridTrack {
    pub(crate) track: TrackSize,
}

impl GridTrack {
    #[must_use]
    pub(crate) const fn content() -> Self {
        Self {
            track: TrackSize::Content { max: None },
        }
    }

    #[must_use]
    pub(crate) const fn content_max(max: u16) -> Self {
        Self {
            track: TrackSize::Content { max: Some(max) },
        }
    }

    #[must_use]
    pub(crate) const fn fixed(size: u16) -> Self {
        Self {
            track: TrackSize::Fixed(size),
        }
    }

    #[must_use]
    pub(crate) const fn flex() -> Self {
        Self {
            track: TrackSize::Flex { min: 1 },
        }
    }

    #[must_use]
    pub(crate) const fn flex_max(max: u16) -> Self {
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
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            column_span: NonZeroU16::MIN,
            row_span: NonZeroU16::MIN,
            horizontal_align: HorizontalAlign::Start,
            vertical_align: VerticalAlign::Top,
        }
    }

    #[must_use]
    pub(crate) fn column_span(self, span: u16) -> Self {
        Self {
            column_span: NonZeroU16::new(span).expect("grid column span must be at least 1"),
            ..self
        }
    }

    #[must_use]
    pub(crate) fn row_span(self, span: u16) -> Self {
        Self {
            row_span: NonZeroU16::new(span).expect("grid row span must be at least 1"),
            ..self
        }
    }

    #[must_use]
    pub(crate) fn horizontal_align(self, align: HorizontalAlign) -> Self {
        Self {
            horizontal_align: align,
            ..self
        }
    }

    #[must_use]
    pub(crate) fn vertical_align(self, align: VerticalAlign) -> Self {
        Self {
            vertical_align: align,
            ..self
        }
    }
}

/// Internal two-dimensional placement input. The canonical factory consumes
/// owned columns, rows, and cells directly; this record is not a retained
/// semantic node and cannot itself be converted into a `View`.
fn lower_grid_parts(
    mut columns: Vec<TrackSize>,
    column_gap: u16,
    row_gap: u16,
    rows_in: Vec<(TrackSize, Vec<(GridCellSpec, View)>)>,
) -> GridView {
    let mut occupied_until = vec![0usize; columns.len()];
    let mut rows = Vec::with_capacity(rows_in.len());
    let mut cells = Vec::new();

    for (row_index, (track, pending_cells)) in rows_in.into_iter().enumerate() {
        rows.push(track);
        for (spec, view) in pending_cells {
            let column_span = usize::from(spec.column_span.get());
            let row_span = usize::from(spec.row_span.get());
            let column = place_cell(row_index, column_span, &mut columns, &mut occupied_until);
            for occupied in occupied_until.iter_mut().skip(column).take(column_span) {
                *occupied = row_index.saturating_add(row_span);
            }
            cells.push(GridCellView {
                row: row_index,
                column,
                row_span: spec.row_span.get(),
                column_span: spec.column_span.get(),
                horizontal_align: spec.horizontal_align,
                vertical_align: spec.vertical_align,
                view,
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
        column_gap,
        row_gap,
        cells: PersistentSeq::from_vec(cells),
        cell_indices,
    }
}

impl View {
    /// Direct grid construction for built-in producers: moves parsed rows
    /// into final storage through the shared placement algorithm with
    /// exactly one root.
    pub(crate) fn grid_from_parts(
        columns: Vec<GridTrack>,
        column_gap: u16,
        row_gap: u16,
        rows: Vec<(GridTrack, Vec<(GridCellSpec, View)>)>,
    ) -> Self {
        Self::new_kind(ViewKind::Grid(Arc::new(lower_grid_parts(
            columns.into_iter().map(|track| track.track).collect(),
            column_gap,
            row_gap,
            rows.into_iter()
                .map(|(track, cells)| (track.track, cells))
                .collect(),
        ))))
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

    fn grid_ir(view: &View) -> &GridView {
        let ViewKind::Grid(grid) = view.kind() else {
            panic!("expected grid view");
        };
        grid
    }

    fn text(view: &View) -> &str {
        let ViewKind::Text(text) = view.kind() else {
            panic!("expected text");
        };
        text.spans[0].text()
    }

    #[test]
    fn basic_auto_placement_fills_rows_in_source_order() {
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![
                (
                    GridTrack::content(),
                    vec![
                        (GridCellSpec::new(), crate::presentation::factory::text("A")),
                        (GridCellSpec::new(), crate::presentation::factory::text("B")),
                    ],
                ),
                (
                    GridTrack::content(),
                    vec![
                        (GridCellSpec::new(), crate::presentation::factory::text("C")),
                        (GridCellSpec::new(), crate::presentation::factory::text("D")),
                    ],
                ),
            ],
        );
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
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![
                (
                    GridTrack::content(),
                    vec![
                        (
                            GridCellSpec::new().column_span(2),
                            crate::presentation::factory::text("A"),
                        ),
                        (GridCellSpec::new(), crate::presentation::factory::text("B")),
                    ],
                ),
                (
                    GridTrack::content(),
                    vec![
                        (GridCellSpec::new(), crate::presentation::factory::text("C")),
                        (GridCellSpec::new(), crate::presentation::factory::text("D")),
                        (GridCellSpec::new(), crate::presentation::factory::text("E")),
                    ],
                ),
            ],
        );
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
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![
                (
                    GridTrack::content(),
                    vec![
                        (
                            GridCellSpec::new().row_span(2),
                            crate::presentation::factory::text("A"),
                        ),
                        (GridCellSpec::new(), crate::presentation::factory::text("B")),
                    ],
                ),
                (
                    GridTrack::content(),
                    vec![(GridCellSpec::new(), crate::presentation::factory::text("C"))],
                ),
            ],
        );
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
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![
                (
                    GridTrack::content(),
                    vec![
                        (
                            GridCellSpec::new().column_span(2).row_span(2),
                            crate::presentation::factory::text("A"),
                        ),
                        (GridCellSpec::new(), crate::presentation::factory::text("B")),
                    ],
                ),
                (
                    GridTrack::content(),
                    vec![(GridCellSpec::new(), crate::presentation::factory::text("C"))],
                ),
            ],
        );
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
        let view = crate::presentation::factory::grid(
            vec![GridTrack::fixed(3)],
            0,
            0,
            vec![(
                GridTrack::content(),
                vec![
                    (GridCellSpec::new(), crate::presentation::factory::text("A")),
                    (GridCellSpec::new(), crate::presentation::factory::text("B")),
                    (GridCellSpec::new(), crate::presentation::factory::text("C")),
                ],
            )],
        );
        let grid = grid_ir(&view);
        assert_eq!(grid.columns.len(), 3);
        assert_eq!(grid.columns[0], TrackSize::Fixed(3));
        assert_eq!(grid.columns[1], TrackSize::Content { max: None });
        assert_eq!(grid.columns[2], TrackSize::Content { max: None });
    }

    #[test]
    fn implicit_rows_cover_row_spans() {
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![(
                GridTrack::content(),
                vec![(
                    GridCellSpec::new().row_span(3),
                    crate::presentation::factory::text("A"),
                )],
            )],
        );
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
        let child = crate::presentation::factory::native_component(1);
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![(GridTrack::content(), vec![(GridCellSpec::new(), child)])],
        );
        assert!(view.contains_component_identity());
        let cloned = view.clone();
        let original = grid_ir(&view);
        let clone = grid_ir(&cloned);
        assert_eq!(original.cells.len(), 1);
        assert_eq!(clone.cells.len(), 1);
        assert_eq!(original.cells[0].view, clone.cells[0].view);
    }

    #[test]
    fn source_order_is_preserved_in_retained_cells() {
        let view = crate::presentation::factory::grid(
            vec![],
            0,
            0,
            vec![
                (
                    GridTrack::content(),
                    vec![
                        (
                            GridCellSpec::new().column_span(2),
                            crate::presentation::factory::text("first"),
                        ),
                        (
                            GridCellSpec::new(),
                            crate::presentation::factory::text("second"),
                        ),
                    ],
                ),
                (
                    GridTrack::content(),
                    vec![(
                        GridCellSpec::new(),
                        crate::presentation::factory::text("third"),
                    )],
                ),
            ],
        );
        let labels: Vec<_> = grid_ir(&view)
            .cells
            .iter()
            .map(|cell| text(&cell.view))
            .collect();
        assert_eq!(labels, ["first", "second", "third"]);
    }
}
