//! Generic retained local scrolling for arbitrary semantic Views.

use crate::{
    Component, ComponentCx, InteractionResult, KeyStroke, View,
    geometry::Size,
    presentation::IntoView,
    presentation::layout::measure_view,
    scroll_command::{ScrollCommand, map_scroll_key},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollMode {
    FollowEnd,
    Detached { top_row: usize },
}

/// A focusable retained viewport for arbitrary semantic content.
///
/// The pane receives its width and height from framework layout. It keeps a
/// visual-row position only; semantic content anchoring belongs to the content
/// and Surface owners rather than this generic viewport.
pub struct ScrollPane {
    content: View,
    mode: ScrollMode,
    layout_size: Option<Size>,
    /// Native layout reports the full extent behind a `RowViewport`. Keeping
    /// this hint here lets `ContentHost` content use the same controller without
    /// making `ContentPort` own scroll state.
    content_extent: Option<Size>,
}

impl ScrollPane {
    pub fn new(content: impl IntoView) -> Self {
        let content = content.into_view();
        assert!(
            !content.contains_component_identity(),
            "ScrollPane content cannot contain Component identity"
        );
        Self {
            content,
            mode: ScrollMode::FollowEnd,
            layout_size: None,
            content_extent: None,
        }
    }

    pub fn set_content(&mut self, content: impl IntoView) {
        let content = content.into_view();
        assert!(
            !content.contains_component_identity(),
            "ScrollPane content cannot contain Component identity"
        );
        self.content = content;
        self.content_extent = None;
        self.repair_detached();
    }

    pub fn scroll_up(&mut self, rows: usize) -> bool {
        self.move_by(rows, false)
    }

    pub fn scroll_down(&mut self, rows: usize) -> bool {
        self.move_by(rows, true)
    }

    pub fn page_up(&mut self) -> bool {
        self.scroll_up(self.layout_size.map_or(0, |size| usize::from(size.height)))
    }

    pub fn page_down(&mut self) -> bool {
        self.scroll_down(self.layout_size.map_or(0, |size| usize::from(size.height)))
    }

    pub fn scroll_to_start(&mut self) {
        self.mode = ScrollMode::Detached { top_row: 0 };
        self.repair_detached();
    }

    pub fn follow_end(&mut self) {
        self.mode = ScrollMode::FollowEnd;
    }

    pub fn is_following_end(&self) -> bool {
        matches!(self.mode, ScrollMode::FollowEnd)
    }

    fn move_by(&mut self, rows: usize, down: bool) -> bool {
        let Some(size) = self.layout_size else {
            return false;
        };
        if size.width == 0 || size.height == 0 || rows == 0 {
            return false;
        }
        let total = self.content_height(size.width);
        let viewport = usize::from(size.height);
        let max_top = total.saturating_sub(viewport);
        let top = self.top_row(total, viewport);
        let target = if down {
            top.saturating_add(rows).min(max_top)
        } else {
            top.saturating_sub(rows)
        };
        if target == top {
            if down && target == max_top && !self.is_following_end() {
                self.mode = ScrollMode::FollowEnd;
                return true;
            }
            return false;
        }
        if down && target == max_top {
            self.mode = ScrollMode::FollowEnd;
        } else {
            self.mode = ScrollMode::Detached { top_row: target };
        }
        true
    }

    fn top_row(&self, total: usize, viewport: usize) -> usize {
        match self.mode {
            ScrollMode::FollowEnd => total.saturating_sub(viewport),
            ScrollMode::Detached { top_row } => top_row.min(total.saturating_sub(viewport)),
        }
    }

    fn content_height(&self, width: u16) -> usize {
        self.content_extent.map_or_else(
            || usize::from(measure_view(&self.content, width.max(1)).height),
            |extent| usize::from(extent.height),
        )
    }

    fn repair_detached(&mut self) {
        let Some(size) = self.layout_size else {
            return;
        };
        let max_top = self
            .content_height(size.width)
            .saturating_sub(usize::from(size.height));
        if let ScrollMode::Detached { top_row } = &mut self.mode {
            *top_row = (*top_row).min(max_top);
        }
    }

    pub(crate) fn on_layout_changed(&mut self, size: Size) {
        if self.layout_size == Some(size) {
            return;
        }
        self.layout_size = Some(size);
        self.repair_detached();
    }

    pub(crate) fn on_content_extent_changed(&mut self, extent: Size) {
        if self.content_extent == Some(extent) {
            return;
        }
        self.content_extent = Some(extent);
        self.repair_detached();
    }

    pub(crate) fn map_command(
        &self,
        key: KeyStroke,
    ) -> Option<crate::scroll_command::ScrollCommand> {
        map_scroll_key(key)
    }

    pub(crate) fn handle_command(
        &mut self,
        command: ScrollCommand,
        _cx: &mut crate::EventCx<'_>,
    ) -> InteractionResult {
        match command {
            ScrollCommand::LineUp => {
                self.scroll_up(1);
            }
            ScrollCommand::LineDown => {
                self.scroll_down(1);
            }
            ScrollCommand::PageUp => {
                self.page_up();
            }
            ScrollCommand::PageDown => {
                self.page_down();
            }
            ScrollCommand::Start => self.scroll_to_start(),
            ScrollCommand::End => self.follow_end(),
        }
        InteractionResult::Consumed
    }
}

impl Component for ScrollPane {
    fn view(&self) -> View {
        let Some(size) = self.layout_size else {
            return self.content.clone().fill_width().fill_height();
        };
        // An empty initial content view can give the slot a zero geometry. Keep
        // the intrinsic content visible in that state so a later retained
        // update can remeasure the pane instead of being permanently clipped.
        if size.width == 0 || size.height == 0 {
            return self.content.clone().fill_width().fill_height();
        }
        let top = self.top_row(self.content_height(size.width), usize::from(size.height));
        View::row_viewport(self.content.clone(), top.min(usize::from(u16::MAX)) as u16)
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_layout_changed(Self::on_layout_changed);
        cx.key_commands(Self::map_command, Self::handle_command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntoView, presentation::layout::compile_bounded_view};

    fn content(count: usize) -> View {
        View::text(
            (1..=count)
                .map(|row| format!("row {row}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .into_view()
    }

    fn rendered(pane: &ScrollPane) -> Vec<String> {
        let view = Component::view(pane);
        compile_bounded_view(&view, Size::new(12, 5))
            .rows
            .into_iter()
            .map(|row| row.plain_text())
            .collect()
    }

    #[test]
    fn follows_end_and_detaches_from_later_content() {
        let mut pane = ScrollPane::new(content(20));
        pane.on_layout_changed(Size::new(12, 5));
        let first = rendered(&pane);
        assert!(first[0].contains("row 16"));

        assert!(pane.scroll_up(5));
        assert!(!pane.is_following_end());
        assert!(rendered(&pane)[0].contains("row 11"));
        pane.set_content(content(30));
        assert!(rendered(&pane)[0].contains("row 11"));

        pane.follow_end();
        assert!(pane.is_following_end());
        assert!(rendered(&pane)[0].contains("row 26"));
    }

    #[test]
    fn allocation_growth_is_not_fixed_by_the_previous_viewport_height() {
        let mut pane = ScrollPane::new(content(20));
        pane.on_layout_changed(Size::new(12, 1));
        let one_row = compile_bounded_view(&Component::view(&pane), Size::new(12, 1));
        assert_eq!(one_row.rows.len(), 1);
        assert!(one_row.rows[0].plain_text().contains("row 20"));

        pane.on_layout_changed(Size::new(12, 8));
        let eight_rows = compile_bounded_view(&Component::view(&pane), Size::new(12, 8));
        assert_eq!(eight_rows.rows.len(), 8);
        assert!(eight_rows.rows[0].plain_text().contains("row 13"));
    }

    #[test]
    fn end_resumes_following_and_resize_repairs_the_window() {
        let mut pane = ScrollPane::new(content(20));
        pane.on_layout_changed(Size::new(12, 5));
        pane.scroll_to_start();
        assert!(!pane.is_following_end());
        pane.scroll_down(100);
        assert!(pane.is_following_end());

        pane.on_layout_changed(Size::new(12, 8));
        let resized = rendered(&pane);
        assert!(resized[0].contains("row 13"));
    }
}
