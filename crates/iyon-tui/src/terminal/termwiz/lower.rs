use termwiz::{
    cell::{CellAttributes, Intensity, Underline},
    color::{AnsiColor, ColorAttribute, RgbColor},
    surface::{Change, Position, Surface},
};

use crate::{
    physical::{
        AnsiColor as IyonAnsiColor, PhysicalCell, PhysicalColor, PhysicalRow, PhysicalStyle,
        text_cell_width,
    },
    scene::PreparedSceneFrame,
};

pub(crate) fn desired_surface(frame: &PreparedSceneFrame) -> Surface {
    let width = frame.surface.width();
    let height = frame.surface.height();
    let mut desired = Surface::new(usize::from(width), usize::from(height));
    desired.add_changes(surface_changes_with_overlay(
        &frame.surface,
        frame.history_overlay.as_ref(),
    ));
    if width > 0 && height > 0 {
        desired.add_change(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(usize::from(height.saturating_sub(1))),
        });
        desired.add_change(Change::CursorVisibility(
            termwiz::surface::CursorVisibility::Hidden,
        ));
    }
    desired
}

fn surface_changes_with_overlay(
    surface: &crate::physical::Surface,
    overlay: Option<&crate::history::HistoryPhysicalOverlay>,
) -> Vec<Change> {
    let mut changes = Vec::new();
    for y in 0..surface.height() {
        changes.extend(direct_row_changes(surface, overlay, y));
    }
    changes
}

fn direct_row_changes(
    surface: &crate::physical::Surface,
    overlay: Option<&crate::history::HistoryPhysicalOverlay>,
    y: u16,
) -> Vec<Change> {
    let width = usize::from(surface.width());
    let overlay_row = overlay.and_then(|overlay| {
        let offset = usize::from(y).checked_sub(usize::from(overlay.row))?;
        overlay.rows.get(offset)
    });
    let overlay_last_written =
        overlay_row.and_then(|row| row.cells().iter().rposition(|cell| cell.painted));
    let last_written = (0..width)
        .filter(|&x| effective_cell(surface, overlay_row, overlay_last_written, x, y).painted)
        .max();
    let mut changes = vec![Change::CursorPosition {
        x: Position::Absolute(0),
        y: Position::Absolute(usize::from(y)),
    }];
    let Some(last_written) = last_written else {
        return changes;
    };

    let mut active_style: Option<CellAttributes> = None;
    let mut text = String::new();
    for x in 0..=last_written {
        let cell = effective_cell(surface, overlay_row, overlay_last_written, x, y);
        if cell.continuation {
            continue;
        }
        let style = if cell.painted {
            physical_style(cell.style)
        } else {
            CellAttributes::default()
        };
        let grapheme = if cell.painted {
            cell.grapheme.as_deref().unwrap_or(" ")
        } else {
            " "
        };
        if active_style.as_ref() != Some(&style) {
            flush_text(&mut changes, &mut text, active_style.take());
            active_style = Some(style);
        }
        text.push_str(grapheme);
    }
    flush_text(&mut changes, &mut text, active_style);
    changes
}

fn effective_cell(
    surface: &crate::physical::Surface,
    overlay_row: Option<&PhysicalRow>,
    overlay_last_written: Option<usize>,
    x: usize,
    y: u16,
) -> PhysicalCell {
    if overlay_last_written.is_some_and(|last| x <= last)
        && let Some(row) = overlay_row
    {
        return row
            .cell(x)
            .cloned()
            .filter(|cell| cell.painted)
            .unwrap_or(PhysicalCell {
                grapheme: None,
                style: PhysicalStyle::default(),
                painted: true,
                continuation: false,
            });
    }
    surface.get(x as u16, y).clone()
}

pub(crate) fn row_changes(row: &PhysicalRow, y: usize, clear_tail: bool) -> Vec<Change> {
    debug_assert!(
        row.validate_cell_geometry().is_ok(),
        "termwiz lowering requires a wide-cell-valid PhysicalRow: {:?}",
        row.validate_cell_geometry()
    );
    let last_written = row.cells().iter().rposition(|cell| cell.painted);
    let mut changes = vec![Change::CursorPosition {
        x: Position::Absolute(0),
        y: Position::Absolute(y),
    }];

    if clear_tail {
        changes.push(Change::AllAttributes(CellAttributes::default()));
        changes.push(Change::ClearToEndOfLine(ColorAttribute::Default));
    }

    let Some(last_written) = last_written else {
        return changes;
    };

    let mut active_style: Option<CellAttributes> = None;
    let mut text = String::new();
    for cell in &row.cells()[..=last_written] {
        if cell.continuation {
            continue;
        }

        let style = if cell.painted {
            physical_style(cell.style)
        } else {
            CellAttributes::default()
        };
        let grapheme = if cell.painted {
            cell.grapheme.as_deref().unwrap_or(" ")
        } else {
            " "
        };

        if active_style.as_ref() != Some(&style) {
            flush_text(&mut changes, &mut text, active_style.take());
            active_style = Some(style);
        }
        text.push_str(grapheme);
    }
    flush_text(&mut changes, &mut text, active_style);
    debug_assert_eq!(
        iyon_leader_cell_width(row, last_written),
        termwiz_emitted_cell_width(&changes),
        "Iyon leader widths must match termwiz widths of emitted Change::Text"
    );
    changes
}

fn iyon_leader_cell_width(row: &PhysicalRow, last_written: usize) -> usize {
    row.glyphs()
        .filter(|glyph| glyph.start <= last_written)
        .map(|glyph| glyph.width)
        .sum()
}

fn termwiz_emitted_cell_width(changes: &[Change]) -> usize {
    changes
        .iter()
        .map(|change| match change {
            Change::Text(text) => text_cell_width(text),
            _ => 0,
        })
        .sum()
}

fn flush_text(changes: &mut Vec<Change>, text: &mut String, style: Option<CellAttributes>) {
    let Some(style) = style else {
        return;
    };
    changes.push(Change::AllAttributes(style));
    if !text.is_empty() {
        changes.push(Change::Text(std::mem::take(text)));
    }
}

pub(crate) fn physical_style(value: PhysicalStyle) -> CellAttributes {
    let mut attributes = CellAttributes::default();
    attributes.set_foreground(value.foreground.map_or(ColorAttribute::Default, color));
    attributes.set_background(value.background.map_or(ColorAttribute::Default, color));
    // Termwiz has one intensity field. Iyon's canonical styles do not rely on
    // simultaneous bold and dim; bold wins deterministically if both appear.
    attributes.set_intensity(if value.bold {
        Intensity::Bold
    } else if value.dim {
        Intensity::Half
    } else {
        Intensity::Normal
    });
    attributes.set_italic(value.italic);
    attributes.set_underline(if value.underline {
        Underline::Single
    } else {
        Underline::None
    });
    attributes.set_reverse(value.reversed);
    attributes.set_strikethrough(value.strikethrough);
    attributes
}

fn color(value: PhysicalColor) -> ColorAttribute {
    match value {
        PhysicalColor::Default => ColorAttribute::Default,
        PhysicalColor::Named(value) => ColorAttribute::PaletteIndex(ansi_color(value) as u8),
        PhysicalColor::Indexed(value) => ColorAttribute::PaletteIndex(value),
        PhysicalColor::Rgb { r, g, b } => {
            ColorAttribute::TrueColorWithDefaultFallback(RgbColor::new_8bpc(r, g, b).into())
        }
    }
}

fn ansi_color(value: IyonAnsiColor) -> AnsiColor {
    match value {
        IyonAnsiColor::Black => AnsiColor::Black,
        IyonAnsiColor::Red => AnsiColor::Maroon,
        IyonAnsiColor::Green => AnsiColor::Green,
        IyonAnsiColor::Yellow => AnsiColor::Olive,
        IyonAnsiColor::Blue => AnsiColor::Navy,
        IyonAnsiColor::Magenta => AnsiColor::Purple,
        IyonAnsiColor::Cyan => AnsiColor::Teal,
        IyonAnsiColor::Gray => AnsiColor::Silver,
        IyonAnsiColor::DarkGray => AnsiColor::Grey,
        IyonAnsiColor::LightRed => AnsiColor::Red,
        IyonAnsiColor::LightGreen => AnsiColor::Lime,
        IyonAnsiColor::LightYellow => AnsiColor::Yellow,
        IyonAnsiColor::LightBlue => AnsiColor::Blue,
        IyonAnsiColor::LightMagenta => AnsiColor::Fuchsia,
        IyonAnsiColor::LightCyan => AnsiColor::Aqua,
        IyonAnsiColor::White => AnsiColor::White,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_style_maps_colors_and_normalizes_intensity() {
        let attributes = physical_style(PhysicalStyle {
            foreground: Some(PhysicalColor::Indexed(4)),
            background: Some(PhysicalColor::Rgb { r: 1, g: 2, b: 3 }),
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            reversed: true,
            strikethrough: true,
        });
        assert_eq!(attributes.foreground(), ColorAttribute::PaletteIndex(4));
        assert_eq!(
            attributes.background(),
            ColorAttribute::TrueColorWithDefaultFallback(RgbColor::new_8bpc(1, 2, 3).into())
        );
        assert_eq!(attributes.intensity(), Intensity::Bold);
        assert!(attributes.italic());
        assert_eq!(attributes.underline(), Underline::Single);
        assert!(attributes.reverse());
        assert!(attributes.strikethrough());
        assert!(!physical_style(PhysicalStyle::default()).strikethrough());

        assert_eq!(
            physical_style(PhysicalStyle {
                dim: true,
                ..PhysicalStyle::default()
            })
            .intensity(),
            Intensity::Half
        );
    }

    #[test]
    fn wide_continuations_are_not_emitted_as_separate_text() {
        let row = PhysicalRow::from_cells(vec![
            PhysicalCell {
                grapheme: Some("界".to_string()),
                style: PhysicalStyle::default(),
                painted: true,
                continuation: false,
            },
            PhysicalCell {
                grapheme: None,
                style: PhysicalStyle::default(),
                painted: true,
                continuation: true,
            },
            PhysicalCell {
                grapheme: Some("!".to_string()),
                style: PhysicalStyle::default(),
                painted: true,
                continuation: false,
            },
        ]);
        let mut surface = Surface::new(4, 1);
        surface.add_changes(row_changes(&row, 0, true));
        assert_eq!(surface.screen_chars_to_string(), "界! \n");
    }

    #[test]
    fn nowrap_iyon_row_does_not_wrap_under_termwiz() {
        let samples = [
            "4⃣",
            "4️⃣",
            "☀️",
            "🕹️",
            "🗡️",
            "⭐",
            "☆",
            "🇮🇩",
            "🇩🇰",
            "👩‍🔬",
            "🐕‍🦺",
            "👨‍👩‍👧‍👦",
            "e\u{301}",
            "漢",
            "👋🏻",
            "🏴󠁧󠁢󠁳󠁣󠁴󠁿",
        ];
        for sample in samples {
            let row = crate::presentation::paint::row_from_string(sample, PhysicalStyle::default());
            assert!(
                row.validate_cell_geometry().is_ok(),
                "{sample:?}: {:?}",
                row.validate_cell_geometry()
            );
            assert_eq!(
                row.occupied_width(),
                termwiz::cell::grapheme_column_width(sample, None),
                "{sample:?}"
            );

            let width = row.occupied_width().max(1);
            let mut surface = Surface::new(width, 2);
            surface.add_changes(vec![
                Change::CursorPosition {
                    x: Position::Absolute(0),
                    y: Position::Absolute(1),
                },
                Change::Text("K".into()),
            ]);
            surface.add_changes(row_changes(&row, 0, false));
            let screen = surface.screen_chars_to_string();
            let canary = screen.lines().nth(1).unwrap_or("");
            assert!(
                canary.starts_with('K'),
                "{sample:?} must not wrap onto the canary row under termwiz; screen={screen:?}"
            );
            let changes = row_changes(&row, 0, false);
            let last_written = row.cells().iter().rposition(|cell| cell.painted).unwrap();
            assert_eq!(
                iyon_leader_cell_width(&row, last_written),
                termwiz_emitted_cell_width(&changes),
                "{sample:?} Iyon leaders must occupy the same cells as emitted termwiz text"
            );
        }
    }
}
