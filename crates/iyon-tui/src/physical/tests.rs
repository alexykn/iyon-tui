use super::*;
use crate::geometry::Size;
use crate::physical::grapheme_cell_width;
use proptest::prelude::*;

fn leader(text: &str) -> PhysicalCell {
    PhysicalCell {
        grapheme: Some(text.to_string()),
        style: PhysicalStyle::default(),
        painted: true,
        continuation: false,
    }
}

fn continuation() -> PhysicalCell {
    PhysicalCell {
        grapheme: None,
        style: PhysicalStyle::default(),
        painted: true,
        continuation: true,
    }
}

fn row_for(text: &str) -> PhysicalRow {
    let mut cells = Vec::new();
    for grapheme in unicode_segmentation::UnicodeSegmentation::graphemes(text, true) {
        let width = grapheme_cell_width(grapheme);
        if width == 0 {
            continue;
        }
        cells.push(leader(grapheme));
        for _ in 1..width {
            cells.push(continuation());
        }
    }
    PhysicalRow::from_cells(cells)
}

#[test]
fn transparent_and_blank_cells_remain_distinct() {
    let transparent = PhysicalCell::transparent();
    let blank = PhysicalCell::blank(PhysicalStyle {
        background: Some(PhysicalColor::Indexed(1)),
        ..PhysicalStyle::default()
    });

    assert!(!transparent.painted);
    assert!(blank.painted);
    assert_ne!(transparent, blank);
}

#[test]
fn surfaces_support_zero_width_and_zero_height_geometry() {
    for (width, height) in [(0, 0), (0, 3), (4, 0)] {
        let surface = Surface::new(width, height);
        assert_eq!(surface.size, Size { width, height });
        assert_eq!(
            surface.cells.len(),
            usize::from(width) * usize::from(height)
        );
    }
}

#[test]
fn composite_preserves_transparency_and_incompleteness() {
    let mut parent = Surface::new(2, 1);
    let mut child = Surface::new(2, 1);
    child.physically_complete = false;
    child.get_mut(0, 0).grapheme = Some("x".to_string());
    child.get_mut(0, 0).painted = true;

    parent.composite(&child, 0, 0);

    assert!(!parent.physically_complete);
    assert!(parent.get(0, 0).painted);
    assert!(!parent.get(1, 0).painted);
}

#[test]
fn wide_continuation_is_occupied_but_not_text() {
    let row = row_for("界");
    assert_eq!(row.width(), 2);
    assert_eq!(row.plain_text(), "界");
    assert!(row.validate_cell_geometry().is_ok());
}

#[test]
fn placed_omits_a_wide_glyph_that_would_not_fully_fit() {
    let row = row_for("AB界");
    let (placed, complete) = row.place(3, 0);
    assert!(!complete, "界 needs two cells and must not be split");
    assert!(placed.validate_cell_geometry().is_ok());
    assert_eq!(placed.plain_text(), "AB");
    assert!(!placed.cells()[2].painted);
    assert!(!placed.cells()[2].continuation);
}

#[test]
fn clipped_row_prefix_drops_an_intersecting_wide_glyph() {
    let row = row_for("A界B");
    let clipped = row.clipped_after(2);
    assert!(clipped.validate_cell_geometry().is_ok());
    assert_eq!(clipped.plain_text(), "A");
    assert!(!clipped.cells()[1].painted);
    assert!(!clipped.cells()[2].painted);

    let complete = row.clipped_after(3);
    assert!(complete.validate_cell_geometry().is_ok());
    assert_eq!(complete.plain_text(), "A界");
}

#[test]
fn composite_clears_the_whole_glyph_when_overwriting_a_continuation() {
    let mut parent = Surface::new(2, 1);
    let wide = row_for("界");
    *parent.get_mut(0, 0) = wide.cells()[0].clone();
    *parent.get_mut(1, 0) = wide.cells()[1].clone();

    let mut child = Surface::new(1, 1);
    child.get_mut(0, 0).grapheme = Some("X".to_string());
    child.get_mut(0, 0).painted = true;
    parent.composite(&child, 1, 0);

    assert!(!parent.get(0, 0).painted, "old leader must be cleared");
    assert_eq!(parent.get(1, 0).grapheme.as_deref(), Some("X"));
    assert!(!parent.get(1, 0).continuation);
}

#[test]
fn crop_to_does_not_keep_a_truncated_leader() {
    let mut source = Surface::new(4, 1);
    let row = row_for("AB界");
    for (x, cell) in row.cells().iter().enumerate() {
        *source.get_mut(x as u16, 0) = cell.clone();
    }
    let cropped = source.crop_to(3, 1);
    assert!(!cropped.physically_complete);
    assert_eq!(cropped.get(0, 0).grapheme.as_deref(), Some("A"));
    assert_eq!(cropped.get(1, 0).grapheme.as_deref(), Some("B"));
    assert!(!cropped.get(2, 0).painted);
}

#[test]
fn cell_x_of_uses_physical_columns_not_utf8_bytes() {
    let row = row_for("💛 Age");
    let heart = grapheme_cell_width("💛");
    assert_eq!(
        row.cell_x_of("Age"),
        Some(heart + 1),
        "Age follows a {heart}-cell heart plus a space, not UTF-8 byte 5"
    );
    assert_ne!(row.cell_x_of("Age"), "💛 Age".find("Age"));
}

#[test]
fn truncated_wide_leader_fails_geometry_validation() {
    let row = PhysicalRow::from_cells(vec![leader("a")]);
    assert!(row.validate_cell_geometry().is_ok());
    let invalid = vec![leader("界")];
    assert!(super::glyph::validate_cells(&invalid).is_err());
}

const GEOMETRY_ATOMS: &[&str] = &[
    "a",
    "b",
    " ",
    "漢",
    "💛",
    "🇮🇩",
    "👩‍🔬",
    "e\u{301}",
    "👋🏻",
    "🏴󠁧󠁢󠁳󠁣󠁴󠁿",
    "☆",
    "⭐",
    "4⃣",
    "☀️",
];

fn geometry_text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(GEOMETRY_ATOMS), 0..10)
        .prop_map(|parts| parts.concat())
}

proptest! {
    #[test]
    fn physical_rows_and_crops_keep_wide_cell_geometry(
        text in geometry_text(),
        width in 1u16..24,
    ) {
        let row = row_for(&text);
        prop_assert!(row.validate_cell_geometry().is_ok());
        for glyph in row.glyphs() {
            if let Some(grapheme) = glyph.leader.grapheme.as_deref() {
                prop_assert_eq!(glyph.width, grapheme_cell_width(grapheme));
            }
        }

        let (placed, _) = row.place(width, 0);
        prop_assert!(placed.validate_cell_geometry().is_ok());
        prop_assert!(placed.occupied_width() <= usize::from(width));
        prop_assert!(placed.glyphs().all(|glyph| glyph.start + glyph.width <= placed.cells().len()));

        let mut surface = Surface::new(row.width().max(1) as u16, 1);
        for (x, cell) in row.cells().iter().enumerate() {
            if x >= usize::from(surface.width()) {
                break;
            }
            *surface.get_mut(x as u16, 0) = cell.clone();
        }
        let cropped = surface.crop_to(width, 1);
        let crop_width = usize::from(cropped.width());
        if crop_width > 0 {
            prop_assert!(super::glyph::validate_cells(&cropped.cells[..crop_width]).is_ok());
        }
        prop_assert!(cropped.width() <= width);
    }
}
