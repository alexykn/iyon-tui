//! Projected-text stream lowering.

use std::borrow::Cow;

use crate::{
    physical::{PhysicalRow, PhysicalStyle},
    presentation::{
        ir::WidthRule,
        layout::{CompiledTextRow, ViewCompiler, row_from_graphemes, row_from_string},
        paint::StyleContext,
        wrap::{StyledGrapheme, wrap_styled_lines},
    },
    stream::{ProjectedText, ProjectedTextLayout, projected_atoms},
};

impl ViewCompiler<'_> {
    pub(crate) fn compile_projected_text_with_metadata(
        &self,
        text: &ProjectedText,
        max_width: u16,
    ) -> (u16, Vec<CompiledTextRow>) {
        self.compile_projected_text_with_metadata_and_context(
            text,
            max_width,
            &StyleContext::default(),
        )
    }

    pub(crate) fn compile_projected_text_with_metadata_and_context(
        &self,
        text: &ProjectedText,
        max_width: u16,
        context: &StyleContext,
    ) -> (u16, Vec<CompiledTextRow>) {
        self.compile_projected_text_with_style(text, max_width, PhysicalStyle::default(), context)
    }

    fn compile_projected_text_with_style(
        &self,
        text: &ProjectedText,
        max_width: u16,
        inherited: PhysicalStyle,
        context: &StyleContext,
    ) -> (u16, Vec<CompiledTextRow>) {
        if let ProjectedTextLayout::Hanging {
            body_column,
            prefix,
            prefix_style,
            prefix_facts,
            show_prefix,
            ..
        } = &text.layout
        {
            let body_start = text
                .runs
                .first()
                .map_or(text.content_range.end, |run| run.owned.start);
            let body = ProjectedText {
                content_range: crate::stream::StreamRange::new(body_start, text.content_range.end),
                terminator: text.terminator,
                width: WidthRule::Fill,
                wrap: text.wrap,
                align: text.align,
                layout: ProjectedTextLayout::Plain,
                runs: text.runs.clone(),
            };
            let body_width = max_width.saturating_sub(*body_column).max(1);
            let base_context = context.for_descendant();
            let (_, body_rows) =
                self.compile_projected_text_with_style(&body, body_width, inherited, &base_context);
            let prefix_width = crate::physical::text_cell_width(prefix);
            let mut rows = Vec::with_capacity(body_rows.len());
            for (index, mut row) in body_rows.into_iter().enumerate() {
                let indent = if index == 0 && *show_prefix {
                    prefix.clone()
                } else {
                    " ".repeat(usize::from(*body_column))
                };
                let prefix_style = if index == 0 && *show_prefix {
                    self.theme.resolve_text_style(
                        inherited,
                        prefix_style,
                        &context.for_descendant().with_local_facts(prefix_facts),
                    )
                } else {
                    inherited
                };
                let prefix_row = row_from_string(&indent, prefix_style);
                let mut cells = prefix_row.cells().to_vec();
                cells.extend_from_slice(row.row.cells());
                row.row = PhysicalRow::from_cells(cells);
                row.width = row.width.saturating_add(if index == 0 && *show_prefix {
                    prefix_width
                } else {
                    usize::from(*body_column)
                });
                row.fits = row.width <= usize::from(max_width);
                row.source_end = row.source_end.map(|end| {
                    end + (body_start.as_u64() - text.content_range.start.as_u64()) as usize
                });
                rows.push(row);
            }
            return (max_width, rows);
        }

        let hard_lines =
            projected_hard_lines(&self.theme, text, inherited, &context.for_descendant());
        let intrinsic_width = hard_lines
            .iter()
            .map(|line| line.iter().map(|atom| atom.width).sum::<usize>())
            .max()
            .unwrap_or(0);
        let width = match text.width {
            WidthRule::Fit => intrinsic_width.min(usize::from(max_width)) as u16,
            WidthRule::Fill => max_width,
        };
        let rows = wrap_styled_lines(&hard_lines, width, text.wrap)
            .into_iter()
            .map(|w_line| CompiledTextRow {
                row: row_from_graphemes(&w_line.graphemes),
                source_end: w_line
                    .graphemes
                    .last()
                    .and_then(|g| g.source.as_ref())
                    .map(|r| r.end),
                cursor_column: None,
                fits: w_line.fits,
                width: w_line.width,
            })
            .collect();
        (width, rows)
    }
}

fn projected_hard_lines(
    theme: &crate::presentation::paint::ThemeResolver,
    text: &ProjectedText,
    inherited: PhysicalStyle,
    context: &StyleContext,
) -> Vec<Vec<StyledGrapheme<'static>>> {
    let mut hard_lines = vec![Vec::new()];
    for atom in projected_atoms(text) {
        let mapped = StyledGrapheme {
            text: Cow::Owned(atom.display.clone()),
            width: crate::physical::text_cell_width(atom.display.as_str()),
            style: theme.resolve_text_style(
                inherited,
                &atom.style,
                &context.with_local_facts(&atom.style_facts),
            ),
            source: Some(
                (atom.owned.start.as_u64() - text.content_range.start.as_u64()) as usize
                    ..(atom.owned.end.as_u64() - text.content_range.start.as_u64()) as usize,
            ),
        };
        if atom.display == "\r\n" {
            // unicode-segmentation treats CRLF as one grapheme cluster, while
            // the text IR's hard-line contract treats LF as the line break and
            // CR as a zero-width carriage-return control. Split the cluster so
            // projected content matches ordinary TextView newline semantics.
            let mut carriage_return = mapped.clone();
            carriage_return.text = Cow::Owned("\r".to_owned());
            carriage_return.width = 0;
            let source_start = atom.owned.start.as_u64() - text.content_range.start.as_u64();
            carriage_return.source =
                Some(source_start as usize..source_start.saturating_add(1) as usize);
            hard_lines
                .last_mut()
                .expect("stream has a hard line")
                .push(carriage_return);
            hard_lines.push(Vec::new());
        } else if atom.display == "\n" {
            hard_lines.push(Vec::new());
        } else {
            hard_lines
                .last_mut()
                .expect("stream has a hard line")
                .push(mapped);
        }
    }
    hard_lines
}
