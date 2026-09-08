// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = c697c2a2064686fae3f39ee1c120ea7da763069faf1fdaff4434b95119a65817
// generator_blake3 = 21a374704490d608ade5c8894d0de2d01caec2c069c7f8db8839fb5a2832fe3d
// L1-05 retained-state envelope tables (§7.1).
//
// Single source of truth: the `state_property` rows in
// `tools/tui-abi/view_abi.toml`. Bit ids are dense per domain; the TS
// envelope packers write the same lanes. Hand-written readers own value
// semantics; these tables own masks, offsets, and codes.

/// Primitive wake disposition bit: the environment must drain.
pub const WAKE_SCHEDULE_ENVIRONMENT_DRAIN: u32 = 1;

pub mod geometry {
    pub const PROPERTY_COUNT: u32 = 10;
    pub const ALL_MASK: u32 = 0x3ff;
    pub const NULLABLE_MASK: u32 = 0x278;
    pub const CLEARABLE_MASK: u32 = 0x3ff;
    pub const WORD_COUNT: usize = 14;
    pub const STRING_COUNT: usize = 0;
    pub const ID_WIDTH: u32 = 0;
    pub const WIDTH_NAME: &str = "width";
    pub const WIDTH_WORD_OFFSET: u32 = 0;
    pub const WIDTH_WORD_LEN: u32 = 1;
    pub const WIDTH_STRING_OFFSET: u32 = 0;
    pub const WIDTH_STRING_LEN: u32 = 0;
    pub const ID_HEIGHT: u32 = 1;
    pub const HEIGHT_NAME: &str = "height";
    pub const HEIGHT_WORD_OFFSET: u32 = 1;
    pub const HEIGHT_WORD_LEN: u32 = 1;
    pub const HEIGHT_STRING_OFFSET: u32 = 0;
    pub const HEIGHT_STRING_LEN: u32 = 0;
    pub const ID_PADDING: u32 = 2;
    pub const PADDING_NAME: &str = "padding";
    pub const PADDING_WORD_OFFSET: u32 = 2;
    pub const PADDING_WORD_LEN: u32 = 4;
    pub const PADDING_STRING_OFFSET: u32 = 0;
    pub const PADDING_STRING_LEN: u32 = 0;
    pub const ID_MIN_WIDTH: u32 = 3;
    pub const MIN_WIDTH_NAME: &str = "minWidth";
    pub const MIN_WIDTH_WORD_OFFSET: u32 = 6;
    pub const MIN_WIDTH_WORD_LEN: u32 = 1;
    pub const MIN_WIDTH_STRING_OFFSET: u32 = 0;
    pub const MIN_WIDTH_STRING_LEN: u32 = 0;
    pub const ID_MAX_WIDTH: u32 = 4;
    pub const MAX_WIDTH_NAME: &str = "maxWidth";
    pub const MAX_WIDTH_WORD_OFFSET: u32 = 7;
    pub const MAX_WIDTH_WORD_LEN: u32 = 1;
    pub const MAX_WIDTH_STRING_OFFSET: u32 = 0;
    pub const MAX_WIDTH_STRING_LEN: u32 = 0;
    pub const ID_MIN_HEIGHT: u32 = 5;
    pub const MIN_HEIGHT_NAME: &str = "minHeight";
    pub const MIN_HEIGHT_WORD_OFFSET: u32 = 8;
    pub const MIN_HEIGHT_WORD_LEN: u32 = 1;
    pub const MIN_HEIGHT_STRING_OFFSET: u32 = 0;
    pub const MIN_HEIGHT_STRING_LEN: u32 = 0;
    pub const ID_MAX_HEIGHT: u32 = 6;
    pub const MAX_HEIGHT_NAME: &str = "maxHeight";
    pub const MAX_HEIGHT_WORD_OFFSET: u32 = 9;
    pub const MAX_HEIGHT_WORD_LEN: u32 = 1;
    pub const MAX_HEIGHT_STRING_OFFSET: u32 = 0;
    pub const MAX_HEIGHT_STRING_LEN: u32 = 0;
    pub const ID_GAP: u32 = 7;
    pub const GAP_NAME: &str = "gap";
    pub const GAP_WORD_OFFSET: u32 = 10;
    pub const GAP_WORD_LEN: u32 = 1;
    pub const GAP_STRING_OFFSET: u32 = 0;
    pub const GAP_STRING_LEN: u32 = 0;
    pub const ID_ALIGNMENT: u32 = 8;
    pub const ALIGNMENT_NAME: &str = "alignment";
    pub const ALIGNMENT_WORD_OFFSET: u32 = 11;
    pub const ALIGNMENT_WORD_LEN: u32 = 1;
    pub const ALIGNMENT_STRING_OFFSET: u32 = 0;
    pub const ALIGNMENT_STRING_LEN: u32 = 0;
    pub const ID_BORDER_EDGES: u32 = 9;
    pub const BORDER_EDGES_NAME: &str = "borderEdges";
    pub const BORDER_EDGES_WORD_OFFSET: u32 = 12;
    pub const BORDER_EDGES_WORD_LEN: u32 = 2;
    pub const BORDER_EDGES_STRING_OFFSET: u32 = 0;
    pub const BORDER_EDGES_STRING_LEN: u32 = 0;
    pub const SIZE_MODE_FIT: u32 = 1;
    pub const SIZE_MODE_FILL: u32 = 2;
    pub const ALIGN_H_START: u32 = 1;
    pub const ALIGN_H_CENTER: u32 = 2;
    pub const ALIGN_H_END: u32 = 3;
    pub const ALIGN_H_MASK: u32 = ALIGN_H_START | ALIGN_H_CENTER | ALIGN_H_END;
    pub const ALIGN_V_TOP: u32 = 1;
    pub const ALIGN_V_CENTER: u32 = 2;
    pub const ALIGN_V_BOTTOM: u32 = 3;
    pub const ALIGN_V_MASK: u32 = ALIGN_V_TOP | ALIGN_V_CENTER | ALIGN_V_BOTTOM;
    pub const ALIGN_V_SHIFT: u32 = 3;
    pub const ALIGN_WORD_MASK: u32 = ALIGN_H_MASK | (ALIGN_V_MASK << ALIGN_V_SHIFT);
    pub const EDGE_KIND_OBJECT: u32 = 0;
    pub const EDGE_KIND_ALL: u32 = 1;
    pub const EDGE_KIND_TOP_BOTTOM: u32 = 2;
    pub const EDGE_BIT_TOP: u32 = 1;
    pub const EDGE_BIT_RIGHT: u32 = 2;
    pub const EDGE_BIT_BOTTOM: u32 = 4;
    pub const EDGE_BIT_LEFT: u32 = 8;
    pub const EDGE_BITS_MASK: u32 = EDGE_BIT_TOP | EDGE_BIT_RIGHT | EDGE_BIT_BOTTOM | EDGE_BIT_LEFT;
    pub fn check_envelope(
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        for_set: bool,
        words: usize,
        strings: usize,
    ) -> Result<(), String> {
        if set_mask & !0x3ff != 0 {
            return Err("ViewState geometry envelope sets unknown properties".to_owned());
        }
        if null_mask & !set_mask != 0 {
            return Err("ViewState geometry envelope null mask escapes the set mask".to_owned());
        }
        if null_mask & !0x278 != 0 {
            return Err("ViewState geometry envelope nulls a non-nullable property".to_owned());
        }
        if clear_mask & set_mask != 0 {
            return Err("ViewState geometry envelope clears and sets the same properties".to_owned());
        }
        if for_set && clear_mask != 0 {
            return Err("ViewState geometry set envelope carries a clear mask".to_owned());
        }
        if !for_set && (set_mask != 0 || null_mask != 0) {
            return Err("ViewState geometry clear envelope carries set values".to_owned());
        }
        if clear_mask & !0x3ff != 0 {
            return Err("ViewState geometry envelope clears unknown properties".to_owned());
        }
        if clear_mask & !0x3ff != 0 {
            return Err("ViewState geometry envelope clears a non-clearable property".to_owned());
        }
        if for_set {
            if words != 14 {
                return Err("ViewState geometry envelope words lane has the wrong length".to_owned());
            }
            if strings != 0 {
                return Err("ViewState geometry envelope strings lane has the wrong length".to_owned());
            }
        } else if words != 0 || strings != 0 {
            return Err("ViewState geometry clear envelope carries value lanes".to_owned());
        }
        Ok(())
    }
}

pub mod presentation {
    pub const PROPERTY_COUNT: u32 = 7;
    pub const ALL_MASK: u32 = 0x7f;
    pub const NULLABLE_MASK: u32 = 0x5f;
    pub const CLEARABLE_MASK: u32 = 0x7f;
    pub const WORD_COUNT: usize = 5;
    pub const STRING_COUNT: usize = 14;
    pub const ID_FOREGROUND: u32 = 0;
    pub const FOREGROUND_NAME: &str = "foreground";
    pub const FOREGROUND_WORD_OFFSET: u32 = 0;
    pub const FOREGROUND_WORD_LEN: u32 = 0;
    pub const FOREGROUND_STRING_OFFSET: u32 = 0;
    pub const FOREGROUND_STRING_LEN: u32 = 1;
    pub const ID_BACKGROUND: u32 = 1;
    pub const BACKGROUND_NAME: &str = "background";
    pub const BACKGROUND_WORD_OFFSET: u32 = 0;
    pub const BACKGROUND_WORD_LEN: u32 = 0;
    pub const BACKGROUND_STRING_OFFSET: u32 = 1;
    pub const BACKGROUND_STRING_LEN: u32 = 1;
    pub const ID_BORDER_COLOR: u32 = 2;
    pub const BORDER_COLOR_NAME: &str = "borderColor";
    pub const BORDER_COLOR_WORD_OFFSET: u32 = 0;
    pub const BORDER_COLOR_WORD_LEN: u32 = 0;
    pub const BORDER_COLOR_STRING_OFFSET: u32 = 2;
    pub const BORDER_COLOR_STRING_LEN: u32 = 1;
    pub const ID_BORDER_STYLE: u32 = 3;
    pub const BORDER_STYLE_NAME: &str = "borderStyle";
    pub const BORDER_STYLE_WORD_OFFSET: u32 = 0;
    pub const BORDER_STYLE_WORD_LEN: u32 = 1;
    pub const BORDER_STYLE_STRING_OFFSET: u32 = 3;
    pub const BORDER_STYLE_STRING_LEN: u32 = 0;
    pub const ID_BORDER_GLYPHS: u32 = 4;
    pub const BORDER_GLYPHS_NAME: &str = "borderGlyphs";
    pub const BORDER_GLYPHS_WORD_OFFSET: u32 = 1;
    pub const BORDER_GLYPHS_WORD_LEN: u32 = 0;
    pub const BORDER_GLYPHS_STRING_OFFSET: u32 = 3;
    pub const BORDER_GLYPHS_STRING_LEN: u32 = 8;
    pub const ID_TEXT_ATTRIBUTES: u32 = 5;
    pub const TEXT_ATTRIBUTES_NAME: &str = "textAttributes";
    pub const TEXT_ATTRIBUTES_WORD_OFFSET: u32 = 1;
    pub const TEXT_ATTRIBUTES_WORD_LEN: u32 = 2;
    pub const TEXT_ATTRIBUTES_STRING_OFFSET: u32 = 11;
    pub const TEXT_ATTRIBUTES_STRING_LEN: u32 = 0;
    pub const ID_STYLE: u32 = 6;
    pub const STYLE_NAME: &str = "style";
    pub const STYLE_WORD_OFFSET: u32 = 3;
    pub const STYLE_WORD_LEN: u32 = 2;
    pub const STYLE_STRING_OFFSET: u32 = 11;
    pub const STYLE_STRING_LEN: u32 = 3;
    pub const BORDER_STYLE_PLAIN: u32 = 1;
    pub const BORDER_STYLE_ROUNDED: u32 = 2;
    pub const BORDER_STYLE_DOUBLE: u32 = 3;
    pub const TEXT_ATTR_BIT_BOLD: u32 = 1;
    pub const TEXT_ATTR_BIT_DIM: u32 = 2;
    pub const TEXT_ATTR_BIT_ITALIC: u32 = 4;
    pub const TEXT_ATTR_BIT_UNDERLINE: u32 = 8;
    pub const TEXT_ATTR_BIT_REVERSED: u32 = 16;
    pub const TEXT_ATTR_BIT_STRIKETHROUGH: u32 = 32;
    pub const TEXT_ATTR_MASK: u32 = TEXT_ATTR_BIT_BOLD | TEXT_ATTR_BIT_DIM | TEXT_ATTR_BIT_ITALIC | TEXT_ATTR_BIT_UNDERLINE | TEXT_ATTR_BIT_REVERSED | TEXT_ATTR_BIT_STRIKETHROUGH;
    pub fn check_envelope(
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        for_set: bool,
        words: usize,
        strings: usize,
    ) -> Result<(), String> {
        if set_mask & !0x7f != 0 {
            return Err("ViewState presentation envelope sets unknown properties".to_owned());
        }
        if null_mask & !set_mask != 0 {
            return Err("ViewState presentation envelope null mask escapes the set mask".to_owned());
        }
        if null_mask & !0x5f != 0 {
            return Err("ViewState presentation envelope nulls a non-nullable property".to_owned());
        }
        if clear_mask & set_mask != 0 {
            return Err("ViewState presentation envelope clears and sets the same properties".to_owned());
        }
        if for_set && clear_mask != 0 {
            return Err("ViewState presentation set envelope carries a clear mask".to_owned());
        }
        if !for_set && (set_mask != 0 || null_mask != 0) {
            return Err("ViewState presentation clear envelope carries set values".to_owned());
        }
        if clear_mask & !0x7f != 0 {
            return Err("ViewState presentation envelope clears unknown properties".to_owned());
        }
        if clear_mask & !0x7f != 0 {
            return Err("ViewState presentation envelope clears a non-clearable property".to_owned());
        }
        if for_set {
            if words != 5 {
                return Err("ViewState presentation envelope words lane has the wrong length".to_owned());
            }
            if strings != 14 {
                return Err("ViewState presentation envelope strings lane has the wrong length".to_owned());
            }
        } else if words != 0 || strings != 0 {
            return Err("ViewState presentation clear envelope carries value lanes".to_owned());
        }
        Ok(())
    }
}

