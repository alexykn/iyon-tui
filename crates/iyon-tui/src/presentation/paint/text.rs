//! Physical text painting and metadata lowering.
//!
//! Semantic wrapping and cursor geometry are owned by `presentation::wrap`.
//! This module consumes that result and lowers allocated text leaves into
//! backend-neutral physical rows and cells.

#[cfg(test)]
use std::cell::Cell;
use std::{borrow::Cow, collections::HashMap, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    perf::{self, Counter},
    physical::{PhysicalCell, PhysicalRow, PhysicalStyle, Surface, grapheme_cell_width},
    presentation::{HorizontalAlign, WidthRule, ir::TextView},
};

use crate::presentation::{
    layout::{LayoutNodeId, ViewCompiler},
    paint::StyleContext,
    wrap::{StyledGrapheme, styled_hard_lines, text_flow},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledTextRow {
    pub(crate) row: PhysicalRow,
    pub(crate) source_end: Option<usize>,
    pub(crate) cursor_column: Option<usize>,
    pub(crate) fits: bool,
    pub(crate) width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextGeometryGrapheme {
    pub(crate) text: String,
    pub(crate) width: usize,
    pub(crate) source: Option<Range<usize>>,
    pub(crate) span_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextGeometryRow {
    pub(crate) graphemes: Vec<TextGeometryGrapheme>,
    pub(crate) cursor_column: Option<usize>,
    pub(crate) width: usize,
    pub(crate) fits: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextGeometry {
    pub(crate) width: u16,
    pub(crate) rows: Vec<TextGeometryRow>,
}

pub(crate) type TextGeometryCache = HashMap<LayoutNodeId, TextGeometry>;

#[cfg(test)]
thread_local! {
    static TEXT_GEOMETRY_BUILDS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_text_geometry_builds() {
    TEXT_GEOMETRY_BUILDS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn text_geometry_builds() -> usize {
    TEXT_GEOMETRY_BUILDS.with(Cell::get)
}

impl ViewCompiler<'_> {
    pub(crate) fn paint_text(
        &self,
        text: &TextView,
        max_width: u16,
        width_rule: WidthRule,
        inherited: PhysicalStyle,
        context: &StyleContext,
    ) -> Surface {
        let (width, rows) =
            self.compile_text_with_metadata(text, max_width, width_rule, inherited, true, context);
        let all_fit = rows.iter().all(|row| row.fits);
        let height = rows.len().max(1) as u16;
        perf::add(
            Counter::PaintCellsAllocated,
            u64::from(width) * u64::from(height),
        );
        let mut surface = Surface::new(width, height);
        surface.physically_complete = all_fit;
        for (y, row) in rows.into_iter().enumerate() {
            let line_width = row.width;
            let offset = match text.align {
                HorizontalAlign::Start => 0,
                HorizontalAlign::Center => usize::from(width).saturating_sub(line_width) / 2,
                HorizontalAlign::End => usize::from(width).saturating_sub(line_width),
            };
            // Place whole glyph spans. Skipping a leader that does not fit and
            // then copying its continuation produces an orphan continuation.
            let (placed, complete) = row.row.place(width, offset as u16);
            if !complete {
                surface.physically_complete = false;
            }
            for (x, source) in placed.cells().iter().enumerate() {
                if source.painted {
                    *surface.get_mut(x as u16, y as u16) = source.clone();
                }
            }
            if let Some(column) = row.cursor_column {
                let x = offset.saturating_add(column);
                if x < usize::from(width) {
                    let marker_style = row
                        .row
                        .cell(column)
                        .or_else(|| row.row.cells().last())
                        .map_or(inherited, |cell| cell.style);
                    // Reverse occupies one cell; clear any wide glyph covering it.
                    surface.clear_glyph_at(x as u16, y as u16);
                    let cell = surface.get_mut(x as u16, y as u16);
                    if !cell.painted {
                        *cell = PhysicalCell {
                            grapheme: Some(" ".to_owned()),
                            style: marker_style,
                            painted: true,
                            continuation: false,
                        };
                    }
                    cell.style.reversed = true;
                }
            }
        }
        surface
    }

    pub(crate) fn compile_text_with_metadata(
        &self,
        text: &TextView,
        max_width: u16,
        width_rule: WidthRule,
        inherited: PhysicalStyle,
        track_source: bool,
        context: &StyleContext,
    ) -> (u16, Vec<CompiledTextRow>) {
        let mut relative_source = 0usize;
        let spans = text.spans.iter().map(|span| {
            let base = if track_source {
                let current = relative_source;
                relative_source += span.text().len();
                Some(current)
            } else {
                None
            };
            (
                span.text(),
                self.theme.resolve_text_style(
                    inherited,
                    &span.style,
                    &context.with_local_facts(&span.style_facts),
                ),
                base,
            )
        });
        let hard_lines = styled_hard_lines(spans);
        let source = text.cursor.map(|anchor| {
            let source = text
                .spans
                .iter()
                .map(super::super::api::text::TextSpan::text)
                .collect::<String>();
            validate_cursor_anchor(&source, anchor);
            source
        });
        let flow = text_flow(text, hard_lines, source.as_deref(), max_width, width_rule);
        let crate::presentation::wrap::StyledTextFlow {
            width,
            rows: flow_rows,
            cursor,
        } = flow;
        let rows = flow_rows
            .into_iter()
            .enumerate()
            .map(|(row_index, w_line)| CompiledTextRow {
                row: row_from_graphemes(&w_line.graphemes),
                source_end: w_line
                    .graphemes
                    .last()
                    .and_then(|g| g.source.as_ref())
                    .map(|r| r.end),
                cursor_column: cursor
                    .and_then(|(cursor_row, column)| (cursor_row == row_index).then_some(column)),
                fits: w_line.fits,
                width: w_line.width,
            })
            .collect();
        (width, rows)
    }

    /// Computes width-dependent row geometry once while retaining semantic
    /// span styles/facts for later theme resolution.  A theme recolor can
    /// therefore reuse this product and only rebuild PhysicalStyle cells.
    pub(crate) fn compile_text_geometry(
        &self,
        text: &TextView,
        max_width: u16,
        width_rule: WidthRule,
    ) -> TextGeometry {
        #[cfg(test)]
        TEXT_GEOMETRY_BUILDS.with(|count| count.set(count.get().saturating_add(1)));
        let mut relative_source = 0usize;
        let hard_lines = styled_hard_lines(text.spans.iter().map(|span| {
            let base = relative_source;
            relative_source += span.text().len();
            (span.text(), PhysicalStyle::default(), Some(base))
        }));
        let source = text.cursor.map(|anchor| {
            let source = text
                .spans
                .iter()
                .map(super::super::api::text::TextSpan::text)
                .collect::<String>();
            validate_cursor_anchor(&source, anchor);
            source
        });
        let flow = text_flow(text, hard_lines, source.as_deref(), max_width, width_rule);
        let crate::presentation::wrap::StyledTextFlow {
            width,
            rows: flow_rows,
            cursor,
        } = flow;
        let span_ranges = text_span_ranges(text);
        let mut span_cursor = 0usize;
        let rows = flow_rows
            .into_iter()
            .enumerate()
            .map(|(row_index, line)| TextGeometryRow {
                graphemes: line
                    .graphemes
                    .into_iter()
                    .map(|grapheme| {
                        let span_index = semantic_span_index_for_source(
                            &span_ranges,
                            grapheme.source.as_ref().map(|range| range.start),
                            &mut span_cursor,
                        );
                        TextGeometryGrapheme {
                            text: grapheme.text.into_owned(),
                            width: grapheme.width,
                            source: grapheme.source,
                            span_index,
                        }
                    })
                    .collect(),
                cursor_column: cursor
                    .and_then(|(cursor_row, column)| (cursor_row == row_index).then_some(column)),
                width: line.width,
                fits: line.fits,
            })
            .collect();
        TextGeometry { width, rows }
    }

    /// Paints one logical text row without allocating the text leaf's full
    /// height.  Wrapping still computes the complete row index (the retained
    /// layout product needs that geometry), but physical cells are materialized
    /// only for the requested row.
    pub(crate) fn paint_text_row(
        &self,
        text: &TextView,
        max_width: u16,
        width_rule: WidthRule,
        inherited: PhysicalStyle,
        context: &StyleContext,
        row_index: usize,
    ) -> (u16, Option<PhysicalRow>, bool) {
        let (width, rows) =
            self.compile_text_with_metadata(text, max_width, width_rule, inherited, true, context);
        let Some(row) = rows.get(row_index) else {
            return (width, None, true);
        };
        let (placed, complete) = self.paint_compiled_text_row(text, width, row, inherited);
        (width, Some(placed), complete)
    }

    pub(crate) fn paint_compiled_text_row(
        &self,
        text: &TextView,
        width: u16,
        row: &CompiledTextRow,
        inherited: PhysicalStyle,
    ) -> (PhysicalRow, bool) {
        let offset = match text.align {
            HorizontalAlign::Start => 0,
            HorizontalAlign::Center => usize::from(width).saturating_sub(row.width) / 2,
            HorizontalAlign::End => usize::from(width).saturating_sub(row.width),
        };
        let (placed, complete) = row.row.place(width, offset as u16);
        let mut surface = Surface::new(width, 1);
        for (column, cell) in placed.cells().iter().enumerate() {
            if cell.painted {
                *surface.get_mut(column as u16, 0) = cell.clone();
            }
        }
        if let Some(column) = row.cursor_column {
            let target = offset.saturating_add(column);
            if target < usize::from(width) {
                let marker_style = placed
                    .cell(target)
                    .or_else(|| placed.cells().last())
                    .map_or(inherited, |cell| cell.style);
                surface.clear_glyph_at(target as u16, 0);
                let cell = surface.get_mut(target as u16, 0);
                if !cell.painted {
                    *cell = PhysicalCell {
                        grapheme: Some(" ".to_owned()),
                        style: marker_style,
                        painted: true,
                        continuation: false,
                    };
                }
                cell.style.reversed = true;
            }
        }
        (PhysicalRow::from_cells(surface.cells), row.fits && complete)
    }

    pub(crate) fn paint_text_geometry_row(
        &self,
        text: &TextView,
        geometry: &TextGeometry,
        row: &TextGeometryRow,
        inherited: PhysicalStyle,
        context: &StyleContext,
    ) -> (PhysicalRow, bool) {
        let styled = row
            .graphemes
            .iter()
            .map(|grapheme| {
                let style = grapheme
                    .span_index
                    .and_then(|index| text.spans.get(index))
                    .map_or(inherited, |span| {
                        self.theme.resolve_text_style(
                            inherited,
                            &span.style,
                            &context.with_local_facts(&span.style_facts),
                        )
                    });
                StyledGrapheme {
                    text: Cow::Borrowed(grapheme.text.as_str()),
                    width: grapheme.width,
                    style,
                    source: grapheme.source.clone(),
                }
            })
            .collect::<Vec<_>>();
        let physical = row_from_graphemes(&styled);
        let offset = match text.align {
            HorizontalAlign::Start => 0,
            HorizontalAlign::Center => usize::from(geometry.width).saturating_sub(row.width) / 2,
            HorizontalAlign::End => usize::from(geometry.width).saturating_sub(row.width),
        };
        let (placed, complete) = physical.place(geometry.width, offset as u16);
        if row.cursor_column.is_none() {
            return (placed, row.fits && complete);
        }
        let mut surface = Surface::new(geometry.width, 1);
        for (column, cell) in placed.cells().iter().enumerate() {
            if cell.painted {
                *surface.get_mut(column as u16, 0) = cell.clone();
            }
        }
        if let Some(column) = row.cursor_column {
            let target = offset.saturating_add(column);
            if target < usize::from(geometry.width) {
                let marker_style = placed
                    .cell(target)
                    .or_else(|| placed.cells().last())
                    .map_or(inherited, |cell| cell.style);
                surface.clear_glyph_at(target as u16, 0);
                let cell = surface.get_mut(target as u16, 0);
                if !cell.painted {
                    *cell = PhysicalCell {
                        grapheme: Some(" ".to_owned()),
                        style: marker_style,
                        painted: true,
                        continuation: false,
                    };
                }
                cell.style.reversed = true;
            }
        }
        (PhysicalRow::from_cells(surface.cells), row.fits && complete)
    }
}

fn text_span_ranges(text: &TextView) -> Vec<(usize, usize)> {
    let mut start = 0usize;
    text.spans
        .iter()
        .map(|span| {
            let end = start.saturating_add(span.text().len());
            let range = (start, end);
            start = end;
            range
        })
        .collect()
}

fn semantic_span_index_for_source(
    ranges: &[(usize, usize)],
    source: Option<usize>,
    cursor: &mut usize,
) -> Option<usize> {
    let source = source?;
    while *cursor < ranges.len() && source >= ranges[*cursor].1 {
        *cursor += 1;
    }
    ranges
        .get(*cursor)
        .and_then(|&(start, end)| (source >= start && source < end).then_some(*cursor))
}

fn validate_cursor_anchor(text: &str, anchor: crate::presentation::ir::TextCursorAnchor) {
    assert!(
        anchor.byte_offset <= text.len(),
        "text cursor anchor exceeds source length"
    );
    assert!(
        text.is_char_boundary(anchor.byte_offset),
        "text cursor anchor is not a UTF-8 boundary"
    );
}

pub(crate) fn row_from_string(text: &str, style: PhysicalStyle) -> PhysicalRow {
    let graphemes = text
        .graphemes(true)
        .map(|text| StyledGrapheme {
            text: Cow::Owned(text.to_string()),
            width: grapheme_cell_width(text),
            style,
            source: None,
        })
        .collect::<Vec<_>>();
    row_from_graphemes(&graphemes)
}

pub(crate) fn row_from_graphemes(graphemes: &[StyledGrapheme<'_>]) -> PhysicalRow {
    // Trust `StyledGrapheme.width`. Recomputing from text here would reintroduce
    // a second metric between wrap and the physical buffer.
    let mut cells = Vec::new();
    for grapheme in graphemes {
        if grapheme.width == 0 {
            continue;
        }
        cells.push(PhysicalCell {
            grapheme: Some(grapheme.text.to_string()),
            style: grapheme.style,
            painted: true,
            continuation: false,
        });
        for _ in 1..grapheme.width {
            cells.push(PhysicalCell {
                grapheme: None,
                style: grapheme.style,
                painted: true,
                continuation: true,
            });
        }
    }
    let row = PhysicalRow::from_cells(cells);
    debug_assert!(row.validate_cell_geometry().is_ok());
    row
}
