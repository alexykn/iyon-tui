use super::*;
use crate::geometry::Size;
use crate::physical::PhysicalStyle;
use crate::presentation::layout::{compile_bounded_view, compile_view};
use crate::{BorderEdges, BorderSpec, Component};

fn focused_view(text: &str) -> crate::presentation::View {
    let mut input = TextInput::new().multiline(true);
    input.set_text(text);
    input.focused = true;
    input.view()
}

fn has_reversed(row: &crate::physical::PhysicalRow) -> bool {
    row.cells().iter().any(|cell| cell.style.reversed)
}

#[test]
fn focused_empty_and_trailing_cursors_are_visible_reversed_cells() {
    let empty = compile_view(&focused_view(""), 20);
    assert_eq!(empty.rows.len(), 1);
    assert!(has_reversed(&empty.rows[0]));

    let trailing = compile_view(&focused_view("abc"), 20);
    assert_eq!(trailing.rows[0].cells().len(), 4);
    assert!(trailing.rows[0].cells()[3].style.reversed);
}

#[test]
fn newline_cursor_uses_the_correct_logical_line() {
    let mut before = TextInput::new().multiline(true);
    before.set_text("a\nb");
    before.set_cursor_for_test(1);
    before.focused = true;
    let before = compile_view(&before.view(), 20);
    assert!(before.rows[0].cells()[1].style.reversed);
    assert!(!has_reversed(&before.rows[1]));

    let after = compile_view(&focused_view("a\n"), 20);
    assert!(!has_reversed(&after.rows[0]));
    assert!(has_reversed(&after.rows[1]));
}

#[test]
fn cursor_style_targets_one_physical_cell_without_splitting_graphemes() {
    let mut wide_input = TextInput::new().multiline(true);
    wide_input.set_text("界");
    wide_input.set_cursor_for_test(0);
    wide_input.focused = true;
    let wide = compile_view(&wide_input.view(), 20);
    assert_eq!(wide.rows[0].cells().len(), 2);
    assert!(wide.rows[0].cells()[0].style.reversed);
    assert!(!wide.rows[0].cells()[1].style.reversed);

    let mut combining_input = TextInput::new().multiline(true);
    combining_input.set_text("e\u{301}");
    combining_input.set_cursor_for_test(0);
    combining_input.focused = true;
    let combining = compile_view(&combining_input.view(), 20);
    assert_eq!(combining.rows[0].cells().len(), 1);
    assert!(combining.rows[0].cells()[0].style.reversed);
}

#[test]
fn inside_egc_cursor_snaps_to_the_leading_edge() {
    for (text, cursor) in [
        ("👩\u{200d}💻", "👩".len()),
        ("🇺🇸", "🇺".len()),
        ("e\u{301}", "e".len()),
    ] {
        let mut input = TextInput::new().multiline(true);
        input.set_text(text);
        input.set_cursor_for_test(cursor);
        input.focused = true;
        let width = 20;
        let compiled = compile_view(&input.view(), width);
        let new_cursor = compiled
            .rows
            .iter()
            .enumerate()
            .find_map(|(row, physical)| {
                physical
                    .cells()
                    .iter()
                    .position(|cell| cell.style.reversed)
                    .map(|column| (column as u16, row as u16))
            });
        let expected = match text {
            "👩\u{200d}💻" | "🇺🇸" | "e\u{301}" => Some((0, 0)),
            _ => unreachable!(),
        };
        assert_eq!(new_cursor, expected, "text={text:?}");
    }
}

#[test]
#[should_panic(expected = "text cursor anchor is not a UTF-8 boundary")]
fn invalid_cursor_anchor_fails_loudly() {
    let view = crate::presentation::factory::cursor_at(crate::presentation::factory::text("é"), 1);
    let _ = compile_view(&view, 20);
}

#[test]
fn bounded_text_input_uses_width_for_vertical_motion_and_scroll() {
    let mut input = TextInput::new().multiline(true);
    input.set_text("abcdefghij");
    input.set_cursor_for_test(input.text().len());
    input.layout_size = Some(Size::new(4, 2));
    input.repair_scroll();
    assert_eq!(input.scroll_row, 2);

    let view = input.view();
    let compiled = compile_bounded_view(&view, Size::new(4, 2));
    assert_eq!(compiled.rows.len(), 2);
    assert_eq!(compiled.rows[0].plain_text(), "ghi");
    assert_eq!(compiled.rows[1].plain_text(), "j");

    input.set_cursor_for_test(9);
    let ranges = crate::presentation::wrap::input_wrap_ranges("abcdefghij", 4);
    input.move_up_in_rows_for_test(&ranges);
    assert_eq!(input.cursor_bytes(), 6);
}

#[test]
fn previous_layout_height_does_not_cap_multiline_growth() {
    let mut input = TextInput::new().multiline(true);
    input.layout_size = Some(Size::new(20, 1));
    input.set_text("a\nb\nc\nd\ne");
    input.set_cursor_for_test(0);
    input.repair_scroll();
    assert_eq!(input.scroll_row, 0);
    let compiled = compile_view(&input.view(), 20);
    assert_eq!(compiled.rows.len(), 5);
    assert_eq!(compiled.rows[0].plain_text(), "a");
    assert_eq!(compiled.rows[4].plain_text(), "e");

    let mut bordered = TextInput::new()
        .multiline(true)
        .border(BorderSpec::plain().edges(BorderEdges::TOP_BOTTOM));
    bordered.layout_size = Some(Size::new(20, 3));
    bordered.set_text("a\nb\nc\nd\ne");
    bordered.set_cursor_for_test(0);
    bordered.repair_scroll();
    assert_eq!(bordered.scroll_row, 0);
    let compiled = compile_view(&bordered.view(), 20);
    assert_eq!(compiled.rows.len(), 7);
}

#[test]
fn bordered_empty_text_input_keeps_caret_inside_inner_rows() {
    let mut input = TextInput::new()
        .multiline(true)
        .border(BorderSpec::plain().edges(BorderEdges::TOP_BOTTOM));
    input.layout_size = Some(Size::new(20, 3));
    input.focused = true;
    input.repair_scroll();
    assert_eq!(input.scroll_row, 0);

    let compiled = compile_bounded_view(&input.view(), Size::new(20, 3));
    assert_eq!(compiled.rows.len(), 3);
    let cursor_row = compiled
        .rows
        .iter()
        .enumerate()
        .find_map(|(row, physical)| has_reversed(physical).then_some(row))
        .expect("focused empty input should render a reversed caret");
    assert_eq!(cursor_row, 1);
}

#[test]
fn bordered_bounded_text_input_scrolls_with_inner_height_and_keeps_cursor_visible() {
    let mut input = TextInput::new()
        .multiline(true)
        .border(BorderSpec::plain().edges(BorderEdges::TOP_BOTTOM));
    input.layout_size = Some(Size::new(5, 5));
    input.focused = true;
    input.insert_text("abcdefghijklm");

    assert_eq!(input.scroll_row, 1);

    let compiled = compile_bounded_view(&input.view(), Size::new(5, 5));
    assert_eq!(compiled.rows.len(), 5);
    let cursor_row = compiled
        .rows
        .iter()
        .enumerate()
        .find_map(|(row, physical)| has_reversed(physical).then_some(row))
        .expect("focused input should render a reversed caret");
    assert!((1..4).contains(&cursor_row));
}

#[test]
fn bounded_text_input_recomputes_after_resize_and_zero_size_is_safe() {
    let mut input = TextInput::new().multiline(true);
    input.set_text("abcdefgh");
    input.set_cursor_for_test(input.text().len());
    input.layout_size = Some(Size::new(4, 2));
    input.repair_scroll();
    assert_eq!(input.scroll_row, 1);

    input.layout_size = Some(Size::new(6, 2));
    input.repair_scroll();
    assert_eq!(input.scroll_row, 0);
    let _ = compile_view(&input.view(), 6);

    input.layout_size = Some(Size::new(0, 0));
    input.repair_scroll();
    assert_eq!(input.scroll_row, 0);
    let _ = compile_view(&input.view(), 0);
}

#[test]
fn unfocused_text_matches_ordinary_semantic_text() {
    let input = TextInput::new();
    let input_rows = compile_view(&input.view(), 20).rows;
    let ordinary_rows = compile_view(&crate::presentation::factory::text(""), 20).rows;
    assert_eq!(input_rows, ordinary_rows);
    assert!(
        input_rows
            .iter()
            .flat_map(|row| row.cells())
            .all(|cell| cell.style == PhysicalStyle::default())
    );
}
