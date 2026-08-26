//! Physical text painting and metadata lowering.
//!
//! Semantic wrapping and cursor geometry are owned by `presentation::wrap`.
//! This module consumes that result and lowers allocated text leaves into
//! backend-neutral physical rows and cells.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    perf::{self, Counter},
    physical::{PhysicalCell, PhysicalRow, PhysicalStyle, Surface, grapheme_cell_width},
    presentation::{HorizontalAlign, WidthRule, ir::TextView},
};

use crate::presentation::{
    layout::ViewCompiler,
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
                .map(|span| span.text())
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
