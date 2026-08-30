//! N-API control surface for the retained-state plane.
//!
//! This module owns only boundary parsing and native-wrapper lifecycle. State
//! records, effective presentation, and host binding remain in iyon-tui's
//! retained_state module; no structural View or frame code is implemented here.

use std::sync::atomic::{AtomicBool, Ordering};

use iyon_tui::{
    BorderGlyphs, BorderStyle, HostViewState, StyleRef, ViewStatePresentationPatch,
    ViewStatePresentationProperty, ViewStateTextAttributes,
};
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use serde_json::Value;

use super::{color_spec, ensure_alive, lower_style_spec};

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

    #[napi(js_name = "setPresentation")]
    pub fn set_presentation(&self, value: Value) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let patch = parse_presentation_patch(&value)?;
        let wake = self
            .state
            .set_presentation(&patch)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "clearPresentation")]
    pub fn clear_presentation(&self, properties: Option<Vec<String>>) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let properties = properties
            .as_deref()
            .map(parse_presentation_properties)
            .transpose()?;
        let wake = self
            .state
            .clear_presentation(properties.as_deref())
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "setStyleState")]
    pub fn set_style_state(&self, key: String, value: String) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let wake = self
            .state
            .set_style_state(key, value)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        Ok(wake_value(wake))
    }

    #[napi(js_name = "clearStyleState")]
    pub fn clear_style_state(&self, key: String) -> Result<Value> {
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

fn wake_value(wake: iyon_tui::WakeDisposition) -> Value {
    serde_json::json!({
        "schedule_environment_drain": wake.schedule_environment_drain,
    })
}

fn parse_presentation_patch(value: &Value) -> Result<ViewStatePresentationPatch> {
    let object = value.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("ViewState presentation patch must be an object")
    })?;
    let mut patch = ViewStatePresentationPatch::default();
    for (key, value) in object {
        match key.as_str() {
            "foreground" => patch.foreground = Some(parse_nullable_color(value)?),
            "background" => patch.background = Some(parse_nullable_color(value)?),
            "borderColor" => patch.border_color = Some(parse_nullable_color(value)?),
            "borderStyle" => patch.border_style = Some(parse_nullable_border_style(value)?),
            "borderGlyphs" => patch.border_glyphs = Some(parse_nullable_border_glyphs(value)?),
            "textAttributes" => patch.text_attributes = parse_text_attributes(value)?,
            "style" => patch.style = Some(parse_nullable_style(value)?),
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown ViewState presentation property `{other}`"
                )));
            }
        }
    }
    Ok(patch)
}

fn parse_nullable_color(value: &Value) -> Result<Option<iyon_tui::ColorSpec>> {
    if value.is_null() {
        return Ok(None);
    }
    color_spec(value).map(Some)
}

fn parse_nullable_border_style(value: &Value) -> Result<Option<BorderStyle>> {
    if value.is_null() {
        return Ok(None);
    }
    match value.as_str() {
        Some("plain") => Ok(Some(BorderStyle::Plain)),
        Some("rounded") => Ok(Some(BorderStyle::Rounded)),
        Some("double") => Ok(Some(BorderStyle::Double)),
        _ => Err(crate::NativeError::invalid_input(
            "ViewState borderStyle must be plain, rounded, double, or null",
        )),
    }
}

fn parse_nullable_border_glyphs(value: &Value) -> Result<Option<BorderGlyphs>> {
    if value.is_null() {
        return Ok(None);
    }
    let object = value.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("ViewState borderGlyphs must be an object or null")
    })?;
    let field = |name: &str| {
        object.get(name).and_then(Value::as_str).ok_or_else(|| {
            crate::NativeError::invalid_input(format!(
                "ViewState border glyph `{name}` must be a string"
            ))
        })
    };
    BorderGlyphs::new(
        field("top")?,
        field("right")?,
        field("bottom")?,
        field("left")?,
        field("topLeft")?,
        field("topRight")?,
        field("bottomLeft")?,
        field("bottomRight")?,
    )
    .map(Some)
    .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

fn parse_text_attributes(value: &Value) -> Result<ViewStateTextAttributes> {
    let object = value.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("ViewState textAttributes must be an object")
    })?;
    let mut attributes = ViewStateTextAttributes::default();
    for (name, value) in object {
        let enabled = value.as_bool().ok_or_else(|| {
            crate::NativeError::invalid_input(format!(
                "ViewState text attribute `{name}` must be boolean"
            ))
        })?;
        match name.as_str() {
            "bold" => attributes.bold = Some(enabled),
            "dim" => attributes.dim = Some(enabled),
            "italic" => attributes.italic = Some(enabled),
            "underline" => attributes.underline = Some(enabled),
            "reversed" => attributes.reversed = Some(enabled),
            "strikethrough" => attributes.strikethrough = Some(enabled),
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown ViewState text attribute `{other}`"
                )));
            }
        }
    }
    Ok(attributes)
}

fn parse_nullable_style(value: &Value) -> Result<Option<StyleRef>> {
    if value.is_null() {
        return Ok(None);
    }
    let object = value.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("ViewState style must be an object or null")
    })?;
    let style = lower_style_spec(value)?;
    Ok(Some(match object.get("theme").and_then(Value::as_str) {
        Some(theme) => StyleRef::themed(theme, style),
        None => StyleRef::direct(style),
    }))
}

fn parse_presentation_properties(
    properties: &[String],
) -> Result<Vec<ViewStatePresentationProperty>> {
    let mut parsed = Vec::with_capacity(properties.len());
    for property in properties {
        let value = match property.as_str() {
            "foreground" => ViewStatePresentationProperty::Foreground,
            "background" => ViewStatePresentationProperty::Background,
            "borderColor" => ViewStatePresentationProperty::BorderColor,
            "borderStyle" => ViewStatePresentationProperty::BorderStyle,
            "borderGlyphs" => ViewStatePresentationProperty::BorderGlyphs,
            "textAttributes" => ViewStatePresentationProperty::TextAttributes,
            "style" => ViewStatePresentationProperty::Style,
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown ViewState clear property `{other}`"
                )));
            }
        };
        if parsed.contains(&value) {
            return Err(crate::NativeError::invalid_input(format!(
                "duplicate ViewState clear property `{property}`"
            )));
        }
        parsed.push(value);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_typed_presentation_patch() {
        let patch = parse_presentation_patch(&json!({
            "foreground": "ansi:3",
            "background": null,
            "textAttributes": {"bold": true},
            "borderStyle": "rounded",
        }))
        .unwrap();
        assert!(patch.foreground.is_some());
        assert_eq!(patch.background, Some(None));
        assert_eq!(patch.text_attributes.bold, Some(true));
        assert_eq!(patch.border_style, Some(Some(BorderStyle::Rounded)));
    }

    #[test]
    fn rejects_unknown_patch_fields() {
        assert!(parse_presentation_patch(&json!({"padding": 1})).is_err());
    }
}
