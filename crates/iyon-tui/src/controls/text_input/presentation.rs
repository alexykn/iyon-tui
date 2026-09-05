use crate::{IntoView, View, presentation::wrap::input_wrap_ranges};

use super::TextInput;

impl TextInput {
    fn decorated(&self, view: View) -> View {
        let Some(border) = &self.border else {
            return view;
        };
        let border = if self.scroll_row > 0 {
            border
                .clone()
                .top_label(format!(" ↑ {} more ", self.scroll_row))
        } else {
            border.clone()
        };
        view.border(border)
    }

    pub(super) fn semantic_view(&self) -> View {
        let Some(layout_size) = self.layout_size else {
            let text =
                View::text(self.buffer.text().to_owned()).wrap(crate::WrapMode::WordThenGrapheme);
            return self.decorated(if self.focused {
                text.cursor_at(self.buffer.cursor_bytes()).into_view()
            } else {
                text.into_view()
            });
        };

        let size = self.inner_size(layout_size);
        if size.width == 0 {
            // L1-04: the direct column factory replaces the empty closure.
            return self.decorated(
                View::column_from_views(Vec::new(), 0)
                    .fill_width()
                    .fill_height(),
            );
        }

        let ranges = input_wrap_ranges(self.buffer.text(), size.width);
        let cursor = self.buffer.cursor_bytes();
        let cursor_row = super::cursor::wrapped_line_index_by_start(&ranges, cursor).unwrap_or(0);
        // L1-04: moved children plus the direct column factory.
        let body = View::column_from_views(
            ranges
                .iter()
                .enumerate()
                .map(|(row_index, range)| {
                    let row_text = self.buffer.text()[range.clone()].to_owned();
                    if self.focused && row_index == cursor_row {
                        View::text(row_text)
                            .no_wrap()
                            .cursor_at(cursor.saturating_sub(range.start))
                            .into_view()
                    } else {
                        View::text(row_text).no_wrap().into_view()
                    }
                })
                .collect(),
            0,
        )
        .fill_width();
        // Border belongs on a parent. RowViewport copies from skip into its
        // own surface; a border on that node overwrites the first and last
        // copied rows.
        let skip =
            u16::try_from(self.scroll_row.min(ranges.len().saturating_sub(1))).unwrap_or(u16::MAX);
        self.decorated(
            View::row_viewport(body, skip)
                .fill_width()
                .fill_height()
                .container(),
        )
    }
}
