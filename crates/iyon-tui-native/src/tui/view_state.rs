//! N-API control surface for the retained-state plane.
//!
//! This module owns only envelope decoding and native-wrapper lifecycle.
//! State records, effective geometry/presentation, and host binding remain in
//! iyon-tui's `retained_state` module; no structural View or frame code is
//! implemented here. The mask envelope (set/null/clear masks plus fixed
//! value lanes) generates from the `state_property` schema rows; the readers
//! below own value semantics and terminate directly in the canonical
//! override patch structs.

use std::sync::atomic::{AtomicBool, Ordering};

use iyon_tui::binding::{
    BorderEdges, BorderGlyphs, BorderStyle, GeometryAlignment, HorizontalAlign, HostViewState,
    Insets, StyleRef, StyleSpec, TextAttribute, TextAttributeSpec, VerticalAlign,
    ViewStateGeometryPatch, ViewStateGeometryProperty, ViewStatePresentationPatch,
    ViewStatePresentationProperty, ViewStateSizeMode, WakeDisposition,
};
use napi::bindgen_prelude::Result;
use napi_derive::napi;

use super::view_state_schema::{self, geometry, presentation};
use super::{color_spec_str, ensure_alive, text_attribute};

#[napi]
pub struct NativeViewState {
    state: HostViewState,
    alive: AtomicBool,
}

#[napi]
impl NativeViewState {
    #[napi]
    pub fn dispose(&self) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        self.state
            .dispose()
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        self.alive.store(false, Ordering::Release);
        Ok(())
    }

    #[napi(js_name = "stateId")]
    pub fn state_id(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        i64::try_from(self.state.state_id())
            .map_err(|_| crate::NativeError::internal("ViewState identity exceeds i64"))
    }

    #[napi(js_name = "attachmentId")]
    pub fn attachment_id(&self) -> Result<i64> {
        self.state_id()
    }

    #[napi(js_name = "validateNodeKind")]
    pub fn validate_node_kind(&self, target_node_kind: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let target_node_kind = u32::try_from(target_node_kind).map_err(|_| {
            crate::NativeError::invalid_input("ViewState node kind must fit in u32")
        })?;
        self.state
            .validate_node_kind(target_node_kind)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    /// Applies a geometry envelope. Masks and lanes decode first; the record
    /// mutates only after the complete patch validates, so a malformed
    /// envelope never leaves a partial override behind.
    #[napi(js_name = "setGeometry")]
    pub fn set_geometry(
        &self,
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        words: Vec<u32>,
        strings: Vec<String>,
    ) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let patch = decode_geometry_envelope(set_mask, null_mask, clear_mask, &words, &strings)?;
        let wake = self
            .state
            .set_geometry(&patch)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "clearGeometry")]
    pub fn clear_geometry(
        &self,
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        clear_all: bool,
    ) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let properties = decode_geometry_clear(set_mask, null_mask, clear_mask, clear_all)?;
        let wake = self
            .state
            .clear_geometry(properties.as_deref())
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "setPresentation")]
    pub fn set_presentation(
        &self,
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        words: Vec<u32>,
        strings: Vec<String>,
    ) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let patch =
            decode_presentation_envelope(set_mask, null_mask, clear_mask, &words, &strings)?;
        let wake = self
            .state
            .set_presentation(&patch)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "clearPresentation")]
    pub fn clear_presentation(
        &self,
        set_mask: u32,
        null_mask: u32,
        clear_mask: u32,
        clear_all: bool,
    ) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let properties = decode_presentation_clear(set_mask, null_mask, clear_mask, clear_all)?;
        let wake = self
            .state
            .clear_presentation(properties.as_deref())
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    /// Dynamic style-state keys stay a separate typed operation (§7.1), not a
    /// free-form property map. Owned strings move into state storage; no enum
    /// ids are invented for application keys.
    #[napi(js_name = "setStyleState")]
    pub fn set_style_state(&self, key: String, value: String) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let wake = self
            .state
            .set_style_state(key, value)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "clearStyleState")]
    pub fn clear_style_state(&self, key: String) -> Result<u32> {
        ensure_alive(&self.alive)?;
        let wake = self
            .state
            .clear_style_state(&key)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    pub(super) fn from_host(state: HostViewState) -> Self {
        Self {
            state,
            alive: AtomicBool::new(true),
        }
    }
}

/// Primitive wake disposition (§7.2): one u32 bitmask, never a JSON object.
fn wake_value(wake: WakeDisposition) -> u32 {
    if wake.schedule_environment_drain {
        view_state_schema::WAKE_SCHEDULE_ENVIRONMENT_DRAIN
    } else {
        0
    }
}

fn decode_geometry_envelope(
    set_mask: u32,
    null_mask: u32,
    clear_mask: u32,
    words: &[u32],
    strings: &[String],
) -> Result<ViewStateGeometryPatch> {
    geometry::check_envelope(
        set_mask,
        null_mask,
        clear_mask,
        true,
        words.len(),
        strings.len(),
    )
    .map_err(crate::NativeError::invalid_input)?;
    let mut patch = ViewStateGeometryPatch::default();
    if set_mask & (1 << geometry::ID_WIDTH) != 0 {
        patch.width = Some(read_size_mode(
            words[geometry::WIDTH_WORD_OFFSET as usize],
            geometry::WIDTH_NAME,
        )?);
    }
    if set_mask & (1 << geometry::ID_HEIGHT) != 0 {
        patch.height = Some(read_size_mode(
            words[geometry::HEIGHT_WORD_OFFSET as usize],
            geometry::HEIGHT_NAME,
        )?);
    }
    if set_mask & (1 << geometry::ID_PADDING) != 0 {
        let base = geometry::PADDING_WORD_OFFSET as usize;
        patch.padding = Some(Insets::new(
            read_u16(words[base], &format!("{} top", geometry::PADDING_NAME))?,
            read_u16(
                words[base + 1],
                &format!("{} right", geometry::PADDING_NAME),
            )?,
            read_u16(
                words[base + 2],
                &format!("{} bottom", geometry::PADDING_NAME),
            )?,
            read_u16(words[base + 3], &format!("{} left", geometry::PADDING_NAME))?,
        ));
    }
    if set_mask & (1 << geometry::ID_MIN_WIDTH) != 0 {
        patch.min_width = Some(if null_mask & (1 << geometry::ID_MIN_WIDTH) != 0 {
            None
        } else {
            Some(read_u16(
                words[geometry::MIN_WIDTH_WORD_OFFSET as usize],
                geometry::MIN_WIDTH_NAME,
            )?)
        });
    }
    if set_mask & (1 << geometry::ID_MAX_WIDTH) != 0 {
        patch.max_width = Some(if null_mask & (1 << geometry::ID_MAX_WIDTH) != 0 {
            None
        } else {
            Some(read_u16(
                words[geometry::MAX_WIDTH_WORD_OFFSET as usize],
                geometry::MAX_WIDTH_NAME,
            )?)
        });
    }
    if set_mask & (1 << geometry::ID_MIN_HEIGHT) != 0 {
        patch.min_height = Some(if null_mask & (1 << geometry::ID_MIN_HEIGHT) != 0 {
            None
        } else {
            Some(read_u16(
                words[geometry::MIN_HEIGHT_WORD_OFFSET as usize],
                geometry::MIN_HEIGHT_NAME,
            )?)
        });
    }
    if set_mask & (1 << geometry::ID_MAX_HEIGHT) != 0 {
        patch.max_height = Some(if null_mask & (1 << geometry::ID_MAX_HEIGHT) != 0 {
            None
        } else {
            Some(read_u16(
                words[geometry::MAX_HEIGHT_WORD_OFFSET as usize],
                geometry::MAX_HEIGHT_NAME,
            )?)
        });
    }
    if set_mask & (1 << geometry::ID_GAP) != 0 {
        patch.gap = Some(read_u16(
            words[geometry::GAP_WORD_OFFSET as usize],
            geometry::GAP_NAME,
        )?);
    }
    if set_mask & (1 << geometry::ID_ALIGNMENT) != 0 {
        patch.alignment = Some(read_alignment(
            words[geometry::ALIGNMENT_WORD_OFFSET as usize],
        )?);
    }
    if set_mask & (1 << geometry::ID_BORDER_EDGES) != 0 {
        patch.border_edges = Some(if null_mask & (1 << geometry::ID_BORDER_EDGES) != 0 {
            None
        } else {
            let base = geometry::BORDER_EDGES_WORD_OFFSET as usize;
            Some(read_border_edges(words[base], words[base + 1])?)
        });
    }
    Ok(patch)
}

fn decode_geometry_clear(
    set_mask: u32,
    null_mask: u32,
    clear_mask: u32,
    clear_all: bool,
) -> Result<Option<Vec<ViewStateGeometryProperty>>> {
    geometry::check_envelope(set_mask, null_mask, clear_mask, false, 0, 0)
        .map_err(crate::NativeError::invalid_input)?;
    if clear_all {
        return Ok(None);
    }
    let mut properties = Vec::new();
    for id in 0..geometry::PROPERTY_COUNT {
        if clear_mask & (1 << id) == 0 {
            continue;
        }
        properties.push(match id {
            geometry::ID_WIDTH => ViewStateGeometryProperty::Width,
            geometry::ID_HEIGHT => ViewStateGeometryProperty::Height,
            geometry::ID_PADDING => ViewStateGeometryProperty::Padding,
            geometry::ID_MIN_WIDTH => ViewStateGeometryProperty::MinWidth,
            geometry::ID_MAX_WIDTH => ViewStateGeometryProperty::MaxWidth,
            geometry::ID_MIN_HEIGHT => ViewStateGeometryProperty::MinHeight,
            geometry::ID_MAX_HEIGHT => ViewStateGeometryProperty::MaxHeight,
            geometry::ID_GAP => ViewStateGeometryProperty::Gap,
            geometry::ID_ALIGNMENT => ViewStateGeometryProperty::Alignment,
            geometry::ID_BORDER_EDGES => ViewStateGeometryProperty::BorderEdges,
            _ => {
                return Err(crate::NativeError::invalid_input(
                    "ViewState geometry envelope clears unknown properties",
                ));
            }
        });
    }
    Ok(Some(properties))
}

fn decode_presentation_envelope(
    set_mask: u32,
    null_mask: u32,
    clear_mask: u32,
    words: &[u32],
    strings: &[String],
) -> Result<ViewStatePresentationPatch> {
    presentation::check_envelope(
        set_mask,
        null_mask,
        clear_mask,
        true,
        words.len(),
        strings.len(),
    )
    .map_err(crate::NativeError::invalid_input)?;
    let mut patch = ViewStatePresentationPatch::default();
    if set_mask & (1 << presentation::ID_FOREGROUND) != 0 {
        patch.foreground = Some(if null_mask & (1 << presentation::ID_FOREGROUND) != 0 {
            None
        } else {
            Some(color_spec_str(
                &strings[presentation::FOREGROUND_STRING_OFFSET as usize],
            )?)
        });
    }
    if set_mask & (1 << presentation::ID_BACKGROUND) != 0 {
        patch.background = Some(if null_mask & (1 << presentation::ID_BACKGROUND) != 0 {
            None
        } else {
            Some(color_spec_str(
                &strings[presentation::BACKGROUND_STRING_OFFSET as usize],
            )?)
        });
    }
    if set_mask & (1 << presentation::ID_BORDER_COLOR) != 0 {
        patch.border_color = Some(if null_mask & (1 << presentation::ID_BORDER_COLOR) != 0 {
            None
        } else {
            Some(color_spec_str(
                &strings[presentation::BORDER_COLOR_STRING_OFFSET as usize],
            )?)
        });
    }
    if set_mask & (1 << presentation::ID_BORDER_STYLE) != 0 {
        patch.border_style = Some(if null_mask & (1 << presentation::ID_BORDER_STYLE) != 0 {
            None
        } else {
            Some(read_border_style(
                words[presentation::BORDER_STYLE_WORD_OFFSET as usize],
            )?)
        });
    }
    if set_mask & (1 << presentation::ID_BORDER_GLYPHS) != 0 {
        patch.border_glyphs = Some(if null_mask & (1 << presentation::ID_BORDER_GLYPHS) != 0 {
            None
        } else {
            let base = presentation::BORDER_GLYPHS_STRING_OFFSET as usize;
            Some(read_border_glyphs(&strings[base..base + 8])?)
        });
    }
    if set_mask & (1 << presentation::ID_TEXT_ATTRIBUTES) != 0 {
        let base = presentation::TEXT_ATTRIBUTES_WORD_OFFSET as usize;
        patch.text_attributes = read_text_attributes(words[base], words[base + 1])?;
    }
    if set_mask & (1 << presentation::ID_STYLE) != 0 {
        patch.style = Some(if null_mask & (1 << presentation::ID_STYLE) != 0 {
            None
        } else {
            let word_base = presentation::STYLE_WORD_OFFSET as usize;
            let string_base = presentation::STYLE_STRING_OFFSET as usize;
            Some(read_style(
                &strings[string_base],
                &strings[string_base + 1],
                &strings[string_base + 2],
                words[word_base],
                words[word_base + 1],
            )?)
        });
    }
    Ok(patch)
}

fn decode_presentation_clear(
    set_mask: u32,
    null_mask: u32,
    clear_mask: u32,
    clear_all: bool,
) -> Result<Option<Vec<ViewStatePresentationProperty>>> {
    presentation::check_envelope(set_mask, null_mask, clear_mask, false, 0, 0)
        .map_err(crate::NativeError::invalid_input)?;
    if clear_all {
        return Ok(None);
    }
    let mut properties = Vec::new();
    for id in 0..presentation::PROPERTY_COUNT {
        if clear_mask & (1 << id) == 0 {
            continue;
        }
        properties.push(match id {
            presentation::ID_FOREGROUND => ViewStatePresentationProperty::Foreground,
            presentation::ID_BACKGROUND => ViewStatePresentationProperty::Background,
            presentation::ID_BORDER_COLOR => ViewStatePresentationProperty::BorderColor,
            presentation::ID_BORDER_STYLE => ViewStatePresentationProperty::BorderStyle,
            presentation::ID_BORDER_GLYPHS => ViewStatePresentationProperty::BorderGlyphs,
            presentation::ID_TEXT_ATTRIBUTES => ViewStatePresentationProperty::TextAttributes,
            presentation::ID_STYLE => ViewStatePresentationProperty::Style,
            _ => {
                return Err(crate::NativeError::invalid_input(
                    "ViewState presentation envelope clears unknown properties",
                ));
            }
        });
    }
    Ok(Some(properties))
}

fn read_size_mode(word: u32, what: &str) -> Result<ViewStateSizeMode> {
    match word {
        geometry::SIZE_MODE_FIT => Ok(ViewStateSizeMode::Fit),
        geometry::SIZE_MODE_FILL => Ok(ViewStateSizeMode::Fill),
        _ => Err(crate::NativeError::invalid_input(format!(
            "ViewState {what} must be fit or fill"
        ))),
    }
}

fn read_u16(word: u32, what: &str) -> Result<u16> {
    u16::try_from(word)
        .map_err(|_| crate::NativeError::invalid_input(format!("ViewState {what} must fit in u16")))
}

fn read_alignment(word: u32) -> Result<GeometryAlignment> {
    if word & !geometry::ALIGN_WORD_MASK != 0 {
        return Err(crate::NativeError::invalid_input(
            "ViewState alignment contains unknown bits",
        ));
    }
    let horizontal = match word & geometry::ALIGN_H_MASK {
        0 => None,
        geometry::ALIGN_H_START => Some(HorizontalAlign::Start),
        geometry::ALIGN_H_CENTER => Some(HorizontalAlign::Center),
        geometry::ALIGN_H_END => Some(HorizontalAlign::End),
        _ => {
            return Err(crate::NativeError::invalid_input(
                "ViewState horizontal alignment is invalid",
            ));
        }
    };
    let vertical = match (word >> geometry::ALIGN_V_SHIFT) & geometry::ALIGN_V_MASK {
        0 => None,
        geometry::ALIGN_V_TOP => Some(VerticalAlign::Top),
        geometry::ALIGN_V_CENTER => Some(VerticalAlign::Center),
        geometry::ALIGN_V_BOTTOM => Some(VerticalAlign::Bottom),
        _ => {
            return Err(crate::NativeError::invalid_input(
                "ViewState vertical alignment is invalid",
            ));
        }
    };
    if horizontal.is_none() && vertical.is_none() {
        return Err(crate::NativeError::invalid_input(
            "ViewState alignment must specify an axis",
        ));
    }
    Ok(GeometryAlignment {
        horizontal,
        vertical,
    })
}

fn read_border_edges(kind: u32, bits: u32) -> Result<BorderEdges> {
    if bits & !geometry::EDGE_BITS_MASK != 0 {
        return Err(crate::NativeError::invalid_input(
            "ViewState borderEdges contains unknown bits",
        ));
    }
    match kind {
        geometry::EDGE_KIND_ALL if bits == 0 => Ok(BorderEdges::ALL),
        geometry::EDGE_KIND_TOP_BOTTOM if bits == 0 => Ok(BorderEdges::TOP_BOTTOM),
        geometry::EDGE_KIND_OBJECT => Ok(BorderEdges::new(
            bits & geometry::EDGE_BIT_TOP != 0,
            bits & geometry::EDGE_BIT_RIGHT != 0,
            bits & geometry::EDGE_BIT_BOTTOM != 0,
            bits & geometry::EDGE_BIT_LEFT != 0,
        )),
        _ => Err(crate::NativeError::invalid_input(
            "ViewState borderEdges kind is invalid",
        )),
    }
}

fn read_border_style(word: u32) -> Result<BorderStyle> {
    match word {
        presentation::BORDER_STYLE_PLAIN => Ok(BorderStyle::Plain),
        presentation::BORDER_STYLE_ROUNDED => Ok(BorderStyle::Rounded),
        presentation::BORDER_STYLE_DOUBLE => Ok(BorderStyle::Double),
        _ => Err(crate::NativeError::invalid_input(
            "ViewState borderStyle must be plain, rounded, double, or null",
        )),
    }
}

fn read_border_glyphs(strings: &[String]) -> Result<BorderGlyphs> {
    let [
        top,
        right,
        bottom,
        left,
        top_left,
        top_right,
        bottom_left,
        bottom_right,
    ] = strings
    else {
        return Err(crate::NativeError::invalid_input(
            "ViewState borderGlyphs lane has the wrong length",
        ));
    };
    BorderGlyphs::new(
        top,
        right,
        bottom,
        left,
        top_left,
        top_right,
        bottom_left,
        bottom_right,
    )
    .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

fn read_text_attributes(presence: u32, values: u32) -> Result<TextAttributeSpec> {
    if presence & !presentation::TEXT_ATTR_MASK != 0 {
        return Err(crate::NativeError::invalid_input(
            "ViewState text attributes presence contains unknown bits",
        ));
    }
    if values & !presentation::TEXT_ATTR_MASK != 0 {
        return Err(crate::NativeError::invalid_input(
            "ViewState text attributes values contain unknown bits",
        ));
    }
    if values & !presence != 0 {
        return Err(crate::NativeError::invalid_input(
            "ViewState text attribute values escape their presence mask",
        ));
    }
    let mut spec = TextAttributeSpec::new();
    for (bit, attribute) in [
        (presentation::TEXT_ATTR_BIT_BOLD, TextAttribute::Bold),
        (presentation::TEXT_ATTR_BIT_DIM, TextAttribute::Dim),
        (presentation::TEXT_ATTR_BIT_ITALIC, TextAttribute::Italic),
        (
            presentation::TEXT_ATTR_BIT_UNDERLINE,
            TextAttribute::Underline,
        ),
        (
            presentation::TEXT_ATTR_BIT_REVERSED,
            TextAttribute::Reversed,
        ),
        (
            presentation::TEXT_ATTR_BIT_STRIKETHROUGH,
            TextAttribute::Strikethrough,
        ),
    ] {
        if presence & bit != 0 {
            spec = spec.attribute(attribute, values & bit != 0);
        }
    }
    Ok(spec)
}

fn read_style(
    theme: &str,
    foreground: &str,
    background: &str,
    attr_presence: u32,
    attr_values: u32,
) -> Result<StyleRef> {
    let mut style = StyleSpec::new();
    if !foreground.is_empty() {
        style = style.foreground(color_spec_str(foreground)?);
    }
    if !background.is_empty() {
        style = style.background(color_spec_str(background)?);
    }
    let attributes = read_text_attributes(attr_presence, attr_values)?;
    let named = [
        ("bold", attributes.attribute_value(TextAttribute::Bold)),
        ("dim", attributes.attribute_value(TextAttribute::Dim)),
        ("italic", attributes.attribute_value(TextAttribute::Italic)),
        (
            "underline",
            attributes.attribute_value(TextAttribute::Underline),
        ),
        (
            "reversed",
            attributes.attribute_value(TextAttribute::Reversed),
        ),
        (
            "strikethrough",
            attributes.attribute_value(TextAttribute::Strikethrough),
        ),
    ];
    for (name, enabled) in named {
        if let Some(enabled) = enabled {
            let attribute = text_attribute(name).ok_or_else(|| {
                crate::NativeError::internal("state schema text attribute has no native kind")
            })?;
            style = style.attribute(attribute, enabled);
        }
    }
    Ok(if theme.is_empty() {
        StyleRef::direct(style)
    } else {
        StyleRef::themed(theme, style)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iyon_tui::binding::{ColorSpec, StyleSpec, TextAttribute};

    fn geometry_lanes() -> (Vec<u32>, Vec<String>) {
        (
            vec![0; geometry::WORD_COUNT],
            vec![String::new(); geometry::STRING_COUNT],
        )
    }

    fn presentation_lanes() -> (Vec<u32>, Vec<String>) {
        (
            vec![0; presentation::WORD_COUNT],
            vec![String::new(); presentation::STRING_COUNT],
        )
    }

    #[test]
    fn schema_ids_are_stable_per_domain() {
        // Bit ids are protocol: pin a sample so schema edits stay explicit.
        assert_eq!(geometry::ID_WIDTH, 0);
        assert_eq!(geometry::ID_BORDER_EDGES, 9);
        assert_eq!(presentation::ID_FOREGROUND, 0);
        assert_eq!(presentation::ID_STYLE, 6);
        assert_eq!(geometry::WORD_COUNT, 14);
        assert_eq!(presentation::STRING_COUNT, 14);
    }

    #[test]
    fn decodes_full_geometry_envelope() {
        let (mut words, strings) = geometry_lanes();
        words[geometry::WIDTH_WORD_OFFSET as usize] = geometry::SIZE_MODE_FILL;
        words[geometry::HEIGHT_WORD_OFFSET as usize] = geometry::SIZE_MODE_FIT;
        let base = geometry::PADDING_WORD_OFFSET as usize;
        words[base..base + 4].copy_from_slice(&[1, 2, 3, 4]);
        words[geometry::MIN_WIDTH_WORD_OFFSET as usize] = 10;
        words[geometry::GAP_WORD_OFFSET as usize] = 2;
        words[geometry::ALIGNMENT_WORD_OFFSET as usize] =
            geometry::ALIGN_H_CENTER | (geometry::ALIGN_V_BOTTOM << geometry::ALIGN_V_SHIFT);
        let edge_base = geometry::BORDER_EDGES_WORD_OFFSET as usize;
        words[edge_base] = geometry::EDGE_KIND_OBJECT;
        words[edge_base + 1] = geometry::EDGE_BIT_TOP | geometry::EDGE_BIT_LEFT;
        let set_mask = (1 << geometry::ID_WIDTH)
            | (1 << geometry::ID_HEIGHT)
            | (1 << geometry::ID_PADDING)
            | (1 << geometry::ID_MIN_WIDTH)
            | (1 << geometry::ID_GAP)
            | (1 << geometry::ID_ALIGNMENT)
            | (1 << geometry::ID_BORDER_EDGES);
        // maxWidth arrives as semantic null while minWidth carries a value.
        let null_mask = 1 << geometry::ID_MAX_WIDTH;
        let set_mask = set_mask | (1 << geometry::ID_MAX_WIDTH);
        let patch =
            decode_geometry_envelope(set_mask, null_mask, 0, &words, &strings).expect("decodes");
        assert_eq!(patch.width, Some(ViewStateSizeMode::Fill));
        assert_eq!(patch.height, Some(ViewStateSizeMode::Fit));
        assert_eq!(patch.padding, Some(Insets::new(1, 2, 3, 4)));
        assert_eq!(patch.min_width, Some(Some(10)));
        assert_eq!(patch.max_width, Some(None));
        assert_eq!(patch.min_height, None);
        assert_eq!(patch.gap, Some(2));
        assert_eq!(
            patch.alignment,
            Some(GeometryAlignment {
                horizontal: Some(HorizontalAlign::Center),
                vertical: Some(VerticalAlign::Bottom),
            })
        );
        assert_eq!(
            patch.border_edges,
            Some(Some(BorderEdges::new(true, false, false, true)))
        );
    }

    #[test]
    fn decodes_presentation_envelope_with_null_and_style() {
        let (mut words, mut strings) = presentation_lanes();
        strings[presentation::FOREGROUND_STRING_OFFSET as usize] = "ansi:3".to_owned();
        strings[presentation::BORDER_COLOR_STRING_OFFSET as usize] = "#010203".to_owned();
        words[presentation::BORDER_STYLE_WORD_OFFSET as usize] = presentation::BORDER_STYLE_ROUNDED;
        let attr_base = presentation::TEXT_ATTRIBUTES_WORD_OFFSET as usize;
        words[attr_base] = presentation::TEXT_ATTR_BIT_BOLD | presentation::TEXT_ATTR_BIT_ITALIC;
        words[attr_base + 1] = presentation::TEXT_ATTR_BIT_BOLD;
        let style_word = presentation::STYLE_WORD_OFFSET as usize;
        let style_string = presentation::STYLE_STRING_OFFSET as usize;
        strings[style_string] = "diff.addition".to_owned();
        strings[style_string + 1] = "red".to_owned();
        strings[style_string + 2] = String::new();
        words[style_word] = presentation::TEXT_ATTR_BIT_BOLD;
        words[style_word + 1] = presentation::TEXT_ATTR_BIT_BOLD;
        let set_mask = (1 << presentation::ID_FOREGROUND)
            | (1 << presentation::ID_BACKGROUND)
            | (1 << presentation::ID_BORDER_COLOR)
            | (1 << presentation::ID_BORDER_STYLE)
            | (1 << presentation::ID_TEXT_ATTRIBUTES)
            | (1 << presentation::ID_STYLE);
        let null_mask = 1 << presentation::ID_BACKGROUND;
        let patch = decode_presentation_envelope(set_mask, null_mask, 0, &words, &strings)
            .expect("decodes");
        assert_eq!(patch.foreground, Some(Some(ColorSpec::ansi(3))));
        assert_eq!(patch.background, Some(None));
        assert_eq!(patch.border_color, Some(Some(ColorSpec::rgb(1, 2, 3))));
        assert_eq!(patch.border_style, Some(Some(BorderStyle::Rounded)));
        assert_eq!(
            patch.text_attributes,
            TextAttributeSpec::new()
                .attribute(TextAttribute::Bold, true)
                .attribute(TextAttribute::Italic, false)
        );
        let Some(Some(style)) = patch.style else {
            panic!("style must decode");
        };
        assert_eq!(
            style,
            StyleRef::themed(
                "diff.addition",
                StyleSpec::new()
                    .foreground(ColorSpec::named(iyon_tui::binding::AnsiColor::Red))
                    .attribute(TextAttribute::Bold, true)
            )
        );
    }

    #[test]
    fn rejects_envelope_header_violations() {
        let (words, strings) = geometry_lanes();
        // Unknown set bit.
        assert!(decode_geometry_envelope(1 << 31, 0, 0, &words, &strings).is_err());
        // Null mask escapes the set mask.
        assert!(
            decode_geometry_envelope(
                1 << geometry::ID_MIN_WIDTH,
                1 << geometry::ID_GAP,
                0,
                &words,
                &strings
            )
            .is_err()
        );
        // Null on a non-nullable property (gap).
        assert!(
            decode_geometry_envelope(
                1 << geometry::ID_GAP,
                1 << geometry::ID_GAP,
                0,
                &words,
                &strings
            )
            .is_err()
        );
        // Clear and set intersect.
        assert!(
            decode_geometry_envelope(
                1 << geometry::ID_GAP,
                0,
                1 << geometry::ID_GAP,
                &words,
                &strings
            )
            .is_err()
        );
        // Short lanes.
        assert!(decode_geometry_envelope(0, 0, 0, &words[..5], &strings).is_err());
        // Clear path carries set values.
        assert!(decode_geometry_clear(1, 0, 0, false).is_err());
        // Malformed scalar still fails after a valid header: bad size mode.
        let (mut words, strings) = geometry_lanes();
        words[geometry::WIDTH_WORD_OFFSET as usize] = 9;
        assert!(decode_geometry_envelope(1 << geometry::ID_WIDTH, 0, 0, &words, &strings).is_err());
    }

    #[test]
    fn rejects_unknown_value_bits_and_values_outside_presence() {
        let (mut words, strings) = geometry_lanes();
        words[geometry::ALIGNMENT_WORD_OFFSET as usize] = geometry::ALIGN_H_START | (1 << 31);
        assert!(
            decode_geometry_envelope(1 << geometry::ID_ALIGNMENT, 0, 0, &words, &strings).is_err()
        );

        let edge_base = geometry::BORDER_EDGES_WORD_OFFSET as usize;
        words[edge_base] = geometry::EDGE_KIND_OBJECT;
        words[edge_base + 1] = geometry::EDGE_BITS_MASK | (1 << 31);
        assert!(
            decode_geometry_envelope(1 << geometry::ID_BORDER_EDGES, 0, 0, &words, &strings)
                .is_err()
        );

        let (mut words, strings) = presentation_lanes();
        let attr_base = presentation::TEXT_ATTRIBUTES_WORD_OFFSET as usize;
        words[attr_base] = presentation::TEXT_ATTR_MASK | (1 << 31);
        assert!(
            decode_presentation_envelope(
                1 << presentation::ID_TEXT_ATTRIBUTES,
                0,
                0,
                &words,
                &strings
            )
            .is_err()
        );
        words[attr_base] = presentation::TEXT_ATTR_BIT_BOLD;
        words[attr_base + 1] = 1 << 31;
        assert!(
            decode_presentation_envelope(
                1 << presentation::ID_TEXT_ATTRIBUTES,
                0,
                0,
                &words,
                &strings
            )
            .is_err()
        );
        words[attr_base + 1] = presentation::TEXT_ATTR_BIT_ITALIC;
        assert!(
            decode_presentation_envelope(
                1 << presentation::ID_TEXT_ATTRIBUTES,
                0,
                0,
                &words,
                &strings
            )
            .is_err()
        );

        let style_base = presentation::STYLE_WORD_OFFSET as usize;
        words[style_base] = presentation::TEXT_ATTR_BIT_BOLD;
        words[style_base + 1] = presentation::TEXT_ATTR_BIT_ITALIC;
        assert!(
            decode_presentation_envelope(1 << presentation::ID_STYLE, 0, 0, &words, &strings)
                .is_err()
        );
    }

    #[test]
    fn absent_and_null_value_lanes_are_not_decoded() {
        let (mut words, strings) = geometry_lanes();
        words[geometry::WIDTH_WORD_OFFSET as usize] = geometry::SIZE_MODE_FIT;
        words[geometry::ALIGNMENT_WORD_OFFSET as usize] = u32::MAX;
        let edge_base = geometry::BORDER_EDGES_WORD_OFFSET as usize;
        words[edge_base] = u32::MAX;
        words[edge_base + 1] = u32::MAX;
        let patch = decode_geometry_envelope(1 << geometry::ID_WIDTH, 0, 0, &words, &strings)
            .expect("absent geometry lanes are ignored");
        assert_eq!(patch.width, Some(ViewStateSizeMode::Fit));
        let patch = decode_geometry_envelope(
            1 << geometry::ID_BORDER_EDGES,
            1 << geometry::ID_BORDER_EDGES,
            0,
            &words,
            &strings,
        )
        .expect("null geometry lane is ignored");
        assert_eq!(patch.border_edges, Some(None));

        let (mut words, mut strings) = presentation_lanes();
        strings[presentation::FOREGROUND_STRING_OFFSET as usize] = "#aé000".to_owned();
        strings[presentation::BORDER_COLOR_STRING_OFFSET as usize] = "#000é0".to_owned();
        let style_base = presentation::STYLE_WORD_OFFSET as usize;
        words[style_base] = u32::MAX;
        words[style_base + 1] = u32::MAX;
        let border_style = presentation::BORDER_STYLE_WORD_OFFSET as usize;
        words[border_style] = presentation::BORDER_STYLE_ROUNDED;
        let patch = decode_presentation_envelope(
            1 << presentation::ID_BORDER_STYLE,
            0,
            0,
            &words,
            &strings,
        )
        .expect("absent presentation lanes are ignored");
        assert_eq!(patch.border_style, Some(Some(BorderStyle::Rounded)));

        let patch = decode_presentation_envelope(
            1 << presentation::ID_FOREGROUND,
            1 << presentation::ID_FOREGROUND,
            0,
            &words,
            &strings,
        )
        .expect("null presentation lane is ignored");
        assert_eq!(patch.foreground, Some(None));
        let patch = decode_presentation_envelope(
            1 << presentation::ID_STYLE,
            1 << presentation::ID_STYLE,
            0,
            &words,
            &strings,
        )
        .expect("null nested style lane is ignored");
        assert_eq!(patch.style, Some(None));
    }

    #[test]
    fn clear_distinguishes_all_from_list_and_empty() {
        assert_eq!(
            decode_geometry_clear(0, 0, 0, true).expect("clear all"),
            None
        );
        assert_eq!(
            decode_presentation_clear(0, 0, 0, true).expect("clear all"),
            None
        );
        let mask = (1 << geometry::ID_WIDTH) | (1 << geometry::ID_GAP);
        assert_eq!(
            decode_geometry_clear(0, 0, mask, false).expect("clear list"),
            Some(vec![
                ViewStateGeometryProperty::Width,
                ViewStateGeometryProperty::Gap
            ])
        );
        assert_eq!(
            decode_geometry_clear(0, 0, 0, false).expect("explicit empty clear"),
            Some(Vec::new())
        );
    }
}
