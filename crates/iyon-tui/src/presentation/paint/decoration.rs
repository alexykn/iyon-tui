//! Physical border and surface decoration painting.

#[cfg(test)]
use std::cell::Cell;

use crate::{
    physical::{PhysicalStyle, Surface},
    presentation::BorderSpec,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{StyleContext, ThemeResolver};

#[cfg(test)]
thread_local! {
    static BORDER_CELLS_VISITED: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_border_work() {
    BORDER_CELLS_VISITED.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn border_work() -> usize {
    BORDER_CELLS_VISITED.with(Cell::get)
}

pub(crate) fn paint_border(
    surface: &mut Surface,
    border: &BorderSpec,
    theme: &ThemeResolver,
    inherited: PhysicalStyle,
    context: &StyleContext,
) {
    let size = (surface.width(), surface.height());
    paint_border_at(
        surface,
        border,
        theme,
        inherited,
        context,
        (0, 0),
        size,
        crate::geometry::Rect::new(0, 0, size.0, size.1),
    );
}

/// Paints a decoration at a signed origin without allocating a temporary
/// content-sized surface. A viewport uses this for a decorated ContentHost
/// whose full allocation is larger than the visible window.
pub(crate) fn paint_border_at(
    surface: &mut Surface,
    border: &BorderSpec,
    theme: &ThemeResolver,
    inherited: PhysicalStyle,
    context: &StyleContext,
    origin: (i32, i32),
    size: (u16, u16),
    clip: crate::geometry::Rect,
) {
    if size.0 == 0 || size.1 == 0 {
        return;
    }

    let style = border_style(border, theme, inherited, context);
    let edges = border.edges;
    let glyphs = &border.glyphs;
    let last_x = origin.0.saturating_add(i32::from(size.0).saturating_sub(1));
    let last_y = origin.1.saturating_add(i32::from(size.1).saturating_sub(1));
    let horizontal = visible_range(origin.0, size.0, clip.x, clip.right(), surface.width());
    let vertical = visible_range(origin.1, size.1, clip.y, clip.bottom(), surface.height());

    if edges.top
        && visible_point(origin.1, clip.y, clip.bottom(), surface.height())
        && let Some(range) = horizontal.clone()
    {
        for x in range {
            note_border_cell();
            set_cell_at(surface, x, origin.1, glyphs.top.clone(), style, clip);
        }
    }
    if edges.bottom
        && visible_point(last_y, clip.y, clip.bottom(), surface.height())
        && let Some(range) = horizontal
    {
        for x in range {
            note_border_cell();
            set_cell_at(surface, x, last_y, glyphs.bottom.clone(), style, clip);
        }
    }
    if edges.left
        && let Some(range) = vertical.clone()
    {
        for y in range {
            note_border_cell();
            set_cell_at(surface, origin.0, y, glyphs.left.clone(), style, clip);
        }
    }
    if edges.right
        && let Some(range) = vertical
    {
        for y in range {
            note_border_cell();
            set_cell_at(surface, last_x, y, glyphs.right.clone(), style, clip);
        }
    }

    if edges.top && edges.left {
        set_cell_at(
            surface,
            origin.0,
            origin.1,
            glyphs.top_left.clone(),
            style,
            clip,
        );
    }

    if edges.top
        && visible_point(origin.1, clip.y, clip.bottom(), surface.height())
        && let Some(label) = &border.top_label
    {
        let mut x = origin.0;
        for grapheme in label.graphemes(true) {
            let width = crate::physical::grapheme_cell_width(grapheme);
            if width == 0 {
                continue;
            }
            set_cell_at(surface, x, origin.1, grapheme.to_owned(), style, clip);
            x = x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
        }
    }
    if edges.top && edges.right {
        set_cell_at(
            surface,
            last_x,
            origin.1,
            glyphs.top_right.clone(),
            style,
            clip,
        );
    }
    if edges.bottom && edges.left {
        set_cell_at(
            surface,
            origin.0,
            last_y,
            glyphs.bottom_left.clone(),
            style,
            clip,
        );
    }
    if edges.bottom && edges.right {
        set_cell_at(
            surface,
            last_x,
            last_y,
            glyphs.bottom_right.clone(),
            style,
            clip,
        );
    }
}

fn border_style(
    border: &BorderSpec,
    theme: &ThemeResolver,
    inherited: PhysicalStyle,
    context: &StyleContext,
) -> PhysicalStyle {
    let mut style = border
        .color
        .as_ref()
        .map_or(inherited, |color| PhysicalStyle {
            foreground: Some(theme.resolve_color(color, context)),
            ..PhysicalStyle::default()
        });
    // Text backgrounds belong only to descendant text cells. Border cells
    // retain the backing surface background established before border paint.
    style.background = None;
    style
}

fn set_cell_at(
    surface: &mut Surface,
    x: i32,
    y: i32,
    grapheme: String,
    mut style: PhysicalStyle,
    clip: crate::geometry::Rect,
) {
    if x < i32::from(clip.x)
        || x >= i32::from(clip.right())
        || y < i32::from(clip.y)
        || y >= i32::from(clip.bottom())
        || x < 0
        || y < 0
        || x >= i32::from(surface.width())
        || y >= i32::from(surface.height())
    {
        return;
    }
    let x = x as u16;
    let y = y as u16;
    style.background = surface.get(x, y).style.background;
    let cell = surface.get_mut(x, y);
    cell.grapheme = Some(grapheme);
    cell.style = style;
    cell.painted = true;
    cell.continuation = false;
}

fn visible_range(
    origin: i32,
    len: u16,
    clip_start: u16,
    clip_end: u16,
    surface_len: u16,
) -> Option<std::ops::Range<i32>> {
    let start = origin.max(i32::from(clip_start)).max(0);
    let end = origin
        .saturating_add(i32::from(len))
        .min(i32::from(clip_end))
        .min(i32::from(surface_len));
    (start < end).then_some(start..end)
}

fn visible_point(point: i32, clip_start: u16, clip_end: u16, surface_len: u16) -> bool {
    point >= i32::from(clip_start)
        && point < i32::from(clip_end)
        && point >= 0
        && point < i32::from(surface_len)
}

#[inline]
fn note_border_cell() {
    #[cfg(test)]
    BORDER_CELLS_VISITED.with(|count| count.set(count.get().saturating_add(1)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BorderEdges, Theme, geometry::Rect};

    #[test]
    fn tall_border_edges_visit_only_the_visible_intersection() {
        let mut surface = Surface::new(20, 1);
        let theme = ThemeResolver::new(&Theme::default());
        reset_border_work();
        paint_border_at(
            &mut surface,
            &BorderSpec::plain(),
            &theme,
            PhysicalStyle::default(),
            &StyleContext::default(),
            (0, -10_000),
            (20, 20_001),
            Rect::new(0, 0, 20, 1),
        );
        assert_eq!(border_work(), 2, "only left/right edge cells intersect row");
    }

    #[test]
    fn clipped_wide_top_label_never_leaves_an_orphan_cell() {
        let mut clipped = Surface::new(3, 1);
        let theme = ThemeResolver::new(&Theme::default());
        let edges = BorderEdges::new(true, false, false, false);
        paint_border_at(
            &mut clipped,
            &BorderSpec::plain().edges(edges).top_label("🐕"),
            &theme,
            PhysicalStyle::default(),
            &StyleContext::default(),
            (-1, 0),
            (4, 1),
            Rect::new(0, 0, 3, 1),
        );

        let mut expected = Surface::new(3, 1);
        paint_border_at(
            &mut expected,
            &BorderSpec::plain().edges(edges),
            &theme,
            PhysicalStyle::default(),
            &StyleContext::default(),
            (-1, 0),
            (4, 1),
            Rect::new(0, 0, 3, 1),
        );
        assert_eq!(clipped, expected);
        assert!(crate::physical::validate_cells(clipped.row_cells(0)).is_ok());
    }
}
