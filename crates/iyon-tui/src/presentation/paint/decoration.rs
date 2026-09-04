//! Physical border and surface decoration painting.

use crate::{
    physical::{PhysicalStyle, Surface},
    presentation::{BorderSpec, WidthRule, WrapMode, ir::TextView, layout::ViewCompiler},
};

use super::{StyleContext, ThemeResolver};

pub(crate) fn paint_border(
    surface: &mut Surface,
    border: &BorderSpec,
    theme: &ThemeResolver,
    inherited: PhysicalStyle,
    context: &StyleContext,
) {
    if surface.width() == 0 || surface.height() == 0 {
        return;
    }

    let style = border_style(border, theme, inherited, context);
    let edges = border.edges;
    let glyphs = &border.glyphs;
    let last_x = surface.width().saturating_sub(1);
    let last_y = surface.height().saturating_sub(1);

    if edges.top {
        for x in 0..surface.width() {
            set_cell(surface, x, 0, glyphs.top.clone(), style);
        }
    }
    if edges.bottom {
        for x in 0..surface.width() {
            set_cell(surface, x, last_y, glyphs.bottom.clone(), style);
        }
    }
    if edges.left {
        for y in 0..surface.height() {
            set_cell(surface, 0, y, glyphs.left.clone(), style);
        }
    }
    if edges.right {
        for y in 0..surface.height() {
            set_cell(surface, last_x, y, glyphs.right.clone(), style);
        }
    }

    if edges.top && edges.left {
        set_cell(surface, 0, 0, glyphs.top_left.clone(), style);
    }

    if edges.top
        && let Some(label) = &border.top_label
    {
        let mut text = TextView::plain(label.clone());
        text.wrap = WrapMode::NoWrap;
        let painted = ViewCompiler::with_resolver(theme).paint_text(
            &text,
            surface.width(),
            WidthRule::Fill,
            style,
            context,
        );
        for x in 0..surface.width() {
            let label_cell = painted.get(x, 0);
            if !label_cell.painted {
                continue;
            }
            let mut cell = label_cell.clone();
            cell.style.background = surface.get(x, 0).style.background;
            *surface.get_mut(x, 0) = cell;
        }
    }
    if edges.top && edges.right {
        set_cell(surface, last_x, 0, glyphs.top_right.clone(), style);
    }
    if edges.bottom && edges.left {
        set_cell(surface, 0, last_y, glyphs.bottom_left.clone(), style);
    }
    if edges.bottom && edges.right {
        set_cell(surface, last_x, last_y, glyphs.bottom_right.clone(), style);
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
        .map(|color| PhysicalStyle {
            foreground: Some(theme.resolve_color(color, context)),
            ..PhysicalStyle::default()
        })
        .unwrap_or(inherited);
    // Text backgrounds belong only to descendant text cells. Border cells
    // retain the backing surface background established before border paint.
    style.background = None;
    style
}

fn set_cell(surface: &mut Surface, x: u16, y: u16, grapheme: String, mut style: PhysicalStyle) {
    if x >= surface.width() || y >= surface.height() {
        return;
    }
    style.background = surface.get(x, y).style.background;
    let cell = surface.get_mut(x, y);
    cell.grapheme = Some(grapheme);
    cell.style = style;
    cell.painted = true;
    cell.continuation = false;
}
