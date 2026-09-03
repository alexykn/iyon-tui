use super::*;
use proptest::prelude::*;

#[test]
fn graphemes_move_and_delete_atomically() {
    for text in ["a\u{301}b", "👩\u{200d}💻x", "🇺🇸x", "👍🏽x", "東京x"] {
        let mut buffer = TextBuffer::new();
        buffer.set_text(text, true);
        let end = buffer.cursor_bytes();
        assert!(buffer.move_left());
        assert!(buffer.move_left());
        assert!(buffer.move_right());
        assert!(buffer.backspace());
        assert!(buffer.text().is_char_boundary(buffer.cursor_bytes()));
        assert!(buffer.cursor_bytes() <= end);
        buffer.assert_invariant();
    }
}

#[test]
fn zwj_insertion_preserves_char_boundary_after_cluster_merge() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("👩💻", true);
    buffer.set_cursor("👩".len());
    assert!(buffer.insert_text("\u{200d}", true));
    assert_eq!(buffer.text(), "👩\u{200d}💻");
    assert_eq!(buffer.cursor_bytes(), "👩\u{200d}".len());
    buffer.assert_invariant();
}

#[test]
fn forward_delete_removes_one_extended_grapheme() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("👩\u{200d}💻x", true);
    buffer.set_cursor(0);
    assert!(buffer.delete());
    assert_eq!(buffer.text(), "x");
}

#[test]
fn word_navigation_and_deletion_share_separator_runs() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("hello world", true);
    assert!(buffer.move_word_left());
    assert_eq!(&buffer.text()[..buffer.cursor_bytes()], "hello ");
    assert!(buffer.delete_word_backward());
    assert_eq!(buffer.text(), "world");

    buffer.set_text("naïve café 東京 foo.bar", true);
    assert!(buffer.move_word_left());
    assert!(buffer.delete_word_backward());
    buffer.assert_invariant();
}

#[test]
fn kill_and_yank_preserve_the_line_editing_shape() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("one\ntwo", true);
    buffer.set_cursor(6);
    assert!(buffer.kill_to_line_start());
    assert_eq!(buffer.text(), "one\no");
    assert!(buffer.yank());
    assert_eq!(buffer.text(), "one\ntwo");

    buffer.set_text("one\ntwo", true);
    buffer.set_cursor(4);
    assert!(buffer.kill_to_line_start());
    assert_eq!(buffer.text(), "onetwo");
}

#[test]
fn vertical_movement_preserves_display_column_over_short_rows() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("abcdef\nxy\n123456", true);
    buffer.set_cursor(4);
    assert!(buffer.move_down_in_rows(&buffer.logical_rows()));
    assert_eq!(buffer.cursor_bytes(), "abcdef\nxy".len());
    assert!(buffer.move_down_in_rows(&buffer.logical_rows()));
    assert_eq!(buffer.cursor_bytes(), "abcdef\nxy\n".len() + 4);
    assert!(buffer.move_up_in_rows(&buffer.logical_rows()));
    assert_eq!(buffer.cursor_bytes(), "abcdef\nxy".len());
    assert!(buffer.move_up_in_rows(&buffer.logical_rows()));
    assert_eq!(buffer.cursor_bytes(), 4);
}

#[test]
fn synthetic_soft_wrap_rows_use_the_same_vertical_algorithm() {
    let mut buffer = TextBuffer::new();
    buffer.set_text("abcdef", true);
    buffer.set_cursor(5);
    let wrapped = rows([0..3, 3..6]);
    assert!(buffer.move_up_in_rows(&wrapped));
    assert_eq!(buffer.cursor_bytes(), 2);
    assert!(buffer.move_down_in_rows(&wrapped));
    assert_eq!(buffer.cursor_bytes(), 5);
}

#[test]
fn vertical_motion_uses_stored_cell_widths_not_utf8_bytes() {
    // 💛 occupies two cells. Treating the caret's UTF-8 offset as a display
    // column would land four cells into the next line instead of two.
    let mut buffer = TextBuffer::new();
    buffer.set_text("💛ab\ncdefg", true);
    buffer.set_cursor("💛".len());
    assert!(buffer.move_down_in_rows(&buffer.logical_rows()));
    assert_eq!(
        buffer.cursor_bytes(),
        "💛ab\ncd".len(),
        "display column 2 is the leading edge of 'e', not UTF-8 byte 4"
    );
}

proptest! {
    #[test]
    fn cursor_invariant_survives_unicode_edit_sequences(
        initial in prop::collection::vec(any::<char>(), 0..32)
            .prop_map(|chars| chars.into_iter().collect::<String>()),
        operations in prop::collection::vec(0u8..=9, 0..64),
    ) {
        let mut buffer = TextBuffer::new();
        buffer.set_text(&initial, true);
        for operation in operations {
            match operation {
                0 => { buffer.insert_text("e\\u{301}", true); }
                1 => { buffer.insert_text("👩\\u{200d}💻", true); }
                2 => { buffer.insert_text("界", true); }
                3 => { buffer.backspace(); }
                4 => { buffer.delete(); }
                5 => { buffer.move_left(); }
                6 => { buffer.move_right(); }
                7 => { buffer.move_word_left(); }
                8 => { buffer.move_word_right(); }
                9 => { buffer.kill_to_line_start(); }
                _ => unreachable!(),
            }
            buffer.assert_invariant();
        }
    }
}

#[test]
fn multiline_policy_canonicalizes_text_without_sanitizing_controls() {
    let mut single = TextBuffer::new();
    single.set_text("a\r\nb\tc\u{0001}", false);
    assert_eq!(single.text(), "a b    c\u{0001}");

    let mut multi = TextBuffer::new();
    multi.set_text("a\r\nb\tc\u{0001}", true);
    assert_eq!(multi.text(), "a\nb    c\u{0001}");
}
