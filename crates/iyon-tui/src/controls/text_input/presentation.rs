use crate::{View, presentation::factory as vf, presentation::wrap::input_wrap_ranges};

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
        vf::border(view, border)
    }

    /// Returns the editor's immutable intrinsic content, independent of the
    /// last allocated viewport width. The direct control adapter uses this
    /// for Taffy's MinContent/MaxContent requests after layout callbacks have
    /// converted the regular view into a Fill RowViewport.
    pub(crate) fn intrinsic_view(&self) -> View {
        let text = if self.focused {
            vf::text_with_cursor(
                self.buffer.text().to_owned(),
                crate::WrapMode::WordThenGrapheme,
                crate::HorizontalAlign::Start,
                self.buffer.cursor_bytes(),
            )
        } else {
            vf::text_with_style(
                self.buffer.text().to_owned(),
                crate::WrapMode::WordThenGrapheme,
                crate::HorizontalAlign::Start,
                crate::StyleRef::default(),
            )
        };
        self.decorated(text)
    }

    pub(super) fn semantic_view(&self) -> View {
        let Some(layout_size) = self.layout_size else {
            return self.intrinsic_view();
        };

        let size = self.inner_size(layout_size);
        if size.width == 0 {
            // L1-04: the direct column factory replaces the empty closure.
            return self.decorated(vf::column(Vec::new(), 0).map_node(|node| {
                node.width = crate::presentation::ir::WidthRule::Fill;
                node.height = crate::presentation::ir::HeightRule::Fill;
            }));
        }

        let ranges = input_wrap_ranges(self.buffer.text(), size.width);
        let cursor = self.buffer.cursor_bytes();
        let cursor_row = super::cursor::wrapped_line_index_by_start(&ranges, cursor).unwrap_or(0);
        // L1-04: moved children plus the direct column factory.
        let body = vf::column(
            ranges
                .iter()
                .enumerate()
                .map(|(row_index, range)| {
                    let row_text = self.buffer.text()[range.clone()].to_owned();
                    if self.focused && row_index == cursor_row {
                        vf::text_with_cursor(
                            row_text,
                            crate::WrapMode::NoWrap,
                            crate::HorizontalAlign::Start,
                            cursor.saturating_sub(range.start),
                        )
                    } else {
                        vf::text_with_style(
                            row_text,
                            crate::WrapMode::NoWrap,
                            crate::HorizontalAlign::Start,
                            crate::StyleRef::default(),
                        )
                    }
                })
                .collect(),
            0,
        )
        .map_node(|node| node.width = crate::presentation::ir::WidthRule::Fill);
        // Border belongs on a parent. RowViewport copies from skip into its
        // own surface; a border on that node overwrites the first and last
        // copied rows.
        let skip =
            u16::try_from(self.scroll_row.min(ranges.len().saturating_sub(1))).unwrap_or(u16::MAX);
        self.decorated(vf::container(vf::row_viewport(body, skip, None)))
    }
}
