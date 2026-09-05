//! Typed theme and border input decoder for the native N-API surface.
//!
//! This replaces the `lower_theme`/`lower_text_selector`/`lower_border`
//! `serde_json::Value` walks. Shapes decode through `serde` into validated
//! DTOs that terminate directly in the canonical records (`Theme` via the
//! single-sort batch assembler, `BorderSpec`), so no second theme-like model
//! exists. Accepted shapes match the previous walks exactly: color strings
//! (`theme:`/`ansi:`/`#rrggbb`/named), `{type: "ansi", value}` color
//! objects, `{type: "default"}` theme colors, sparse style objects, full
//! text-selector fields (roles, parts, annotations, language, origin,
//! format, focus, states), and the TextInput border object. Malformed
//! containers now fail closed instead of being silently ignored.

use std::collections::BTreeMap;

use serde::Deserialize;

use iyon_tui::binding::{
    BorderEdges, BorderGlyphs, BorderSpec, ColorSpec, FormatId, LanguageId, SemanticTag,
    StyleSelector, StyleSpec, TextOrigin, TextPart, TextRole, Theme, ThemeColor, ThemeKey,
};

use super::{color_spec_str, text_attribute};

type NativeResult<T> = napi::bindgen_prelude::Result<T>;

fn invalid_input(message: impl Into<String>) -> napi::Error {
    crate::NativeError::invalid_input(message)
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum AnsiMarker {
    Ansi,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DefaultMarker {
    Default,
}

/// A color in a theme table: default marker, indexed ANSI object, or any
/// string form (named, `ansi:`, `#rrggbb`). `{type: "theme"}` references and
/// unknown object shapes are rejected exactly as before.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ThemeColorDto {
    Text(String),
    Indexed { r#type: AnsiMarker, value: u8 },
    Default { r#type: DefaultMarker },
}

impl ThemeColorDto {
    fn build(self) -> NativeResult<ThemeColor> {
        match self {
            Self::Default { .. } => Ok(ThemeColor::Default),
            Self::Indexed { value, .. } => Ok(ThemeColor::Indexed(value)),
            Self::Text(text) => match color_spec_str(&text)? {
                ColorSpec::Theme(_) => Err(invalid_input(
                    "theme colors cannot reference another theme color",
                )),
                ColorSpec::Named(color) => Ok(ThemeColor::Named(color)),
                ColorSpec::Ansi(value) => Ok(ThemeColor::Indexed(value)),
                ColorSpec::Rgb { r, g, b } => Ok(ThemeColor::Rgb { r, g, b }),
            },
        }
    }
}

/// A color inside a style value: any string form or `{type: "ansi", value}`.
/// `{type: "default"}` has no meaning on a sparse style and is rejected.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum StyleColorDto {
    Text(String),
    Indexed { r#type: AnsiMarker, value: u8 },
}

impl StyleColorDto {
    fn build(self) -> NativeResult<ColorSpec> {
        match self {
            Self::Text(text) => color_spec_str(&text),
            Self::Indexed { value, .. } => Ok(ColorSpec::ansi(value)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct StyleDto {
    #[serde(default)]
    foreground: Option<StyleColorDto>,
    #[serde(default)]
    background: Option<StyleColorDto>,
    #[serde(default)]
    attributes: BTreeMap<String, bool>,
}

impl StyleDto {
    fn build(self) -> NativeResult<StyleSpec> {
        let mut style = StyleSpec::new();
        if let Some(color) = self.foreground {
            style = style.foreground(color.build()?);
        }
        if let Some(color) = self.background {
            style = style.background(color.build()?);
        }
        for (name, enabled) in &self.attributes {
            let attribute = text_attribute(name)
                .ok_or_else(|| invalid_input(format!("unknown text attribute `{name}`")))?;
            style = style.attribute(attribute, *enabled);
        }
        Ok(style)
    }
}

#[derive(Debug, Deserialize)]
struct VariantDto<T> {
    selector: SelectorDto,
    value: T,
}

#[derive(Debug, Deserialize)]
struct SelectorDto {
    #[serde(default)]
    focused: bool,
    #[serde(default, rename = "focusWithin")]
    focus_within: bool,
    #[serde(default)]
    states: BTreeMap<String, String>,
}

impl SelectorDto {
    fn build(self) -> StyleSelector {
        let mut selector = StyleSelector::default();
        if self.focused {
            selector = selector.and_focused();
        }
        if self.focus_within {
            selector = selector.and_focus_within();
        }
        for (key, value) in self.states {
            selector = selector.and_state(key, value);
        }
        selector
    }
}

#[derive(Debug, Deserialize)]
struct ColorEntryDto {
    #[serde(default)]
    base: Option<ThemeColorDto>,
    #[serde(default)]
    variants: Vec<VariantDto<ThemeColorDto>>,
}

#[derive(Debug, Deserialize)]
struct StyleEntryDto {
    #[serde(default)]
    base: Option<StyleDto>,
    #[serde(default)]
    variants: Vec<VariantDto<StyleDto>>,
}

#[derive(Debug, Deserialize)]
struct TextAnnotationDto {
    namespace: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct TextSelectorDto {
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    parts: Vec<String>,
    #[serde(default)]
    annotations: Vec<TextAnnotationDto>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    focused: bool,
    #[serde(default, rename = "focusWithin")]
    focus_within: bool,
    #[serde(default)]
    states: BTreeMap<String, String>,
}

impl TextSelectorDto {
    fn build(self) -> NativeResult<iyon_tui::binding::TextSelector> {
        use iyon_tui::binding::TextSelector;
        let mut selector = TextSelector::any();
        for role in &self.roles {
            selector = selector.and_role(text_role(role)?);
        }
        for part in &self.parts {
            selector = selector.and_part(text_part(part)?);
        }
        for annotation in &self.annotations {
            let tag = SemanticTag::new(annotation.namespace.as_str(), annotation.name.as_str())
                .map_err(|error| invalid_input(error.to_string()))?;
            selector = selector.and_annotation(&tag);
        }
        if let Some(language) = &self.language {
            let language = LanguageId::new(language.as_str())
                .map_err(|error| invalid_input(error.to_string()))?;
            selector = selector.language(&language);
        }
        if let Some(origin) = &self.origin {
            let origin = TextOrigin::new(origin.as_str())
                .map_err(|error| invalid_input(error.to_string()))?;
            selector = selector.origin(origin);
        }
        if let Some(format) = &self.format {
            let format =
                FormatId::new(format.as_str()).map_err(|error| invalid_input(error.to_string()))?;
            selector = selector.format(&format);
        }
        if self.focused {
            selector = selector.and_focused();
        }
        if self.focus_within {
            selector = selector.and_focus_within();
        }
        for (key, value) in self.states {
            selector = selector.and_state(key, value);
        }
        Ok(selector)
    }
}

fn text_role(value: &str) -> NativeResult<TextRole> {
    let role = match value {
        "paragraph" => TextRole::Paragraph,
        "heading" => TextRole::Heading,
        "blockQuote" => TextRole::BlockQuote,
        "list" => TextRole::List,
        "listItem" => TextRole::ListItem,
        "codeBlock" => TextRole::CodeBlock,
        "table" => TextRole::Table,
        "tableRow" => TextRole::TableRow,
        "tableCell" => TextRole::TableCell,
        "thematicBreak" => TextRole::ThematicBreak,
        "rawBlock" => TextRole::RawBlock,
        "container" => TextRole::Container,
        "strong" => TextRole::Strong,
        "emphasis" => TextRole::Emphasis,
        "strikethrough" => TextRole::Strikethrough,
        "underline" => TextRole::Underline,
        "superscript" => TextRole::Superscript,
        "subscript" => TextRole::Subscript,
        "smallCaps" => TextRole::SmallCaps,
        "inlineCode" => TextRole::InlineCode,
        "link" => TextRole::Link,
        "image" => TextRole::Image,
        "rawInline" => TextRole::RawInline,
        _ => {
            return Err(invalid_input(format!(
                "unknown text selector role `{value}`"
            )));
        }
    };
    Ok(role)
}

fn text_part(value: &str) -> NativeResult<TextPart> {
    let part = match value {
        "listMarker" => TextPart::ListMarker,
        "taskMarker" => TextPart::TaskMarker,
        "quoteMarker" => TextPart::QuoteMarker,
        "codeLabel" => TextPart::CodeLabel,
        "tableRule" => TextPart::TableRule,
        "thematicRule" => TextPart::ThematicRule,
        "imageFallback" => TextPart::ImageFallback,
        _ => {
            return Err(invalid_input(format!(
                "unknown text selector part `{value}`"
            )));
        }
    };
    Ok(part)
}

#[derive(Debug, Deserialize)]
struct TextStyleDto {
    selector: TextSelectorDto,
    value: StyleDto,
}

#[derive(Debug, Deserialize)]
pub(super) struct ThemeDto {
    #[serde(default)]
    colors: BTreeMap<String, ColorEntryDto>,
    #[serde(default)]
    styles: BTreeMap<String, StyleEntryDto>,
    #[serde(default, rename = "textStyles")]
    text_styles: Vec<TextStyleDto>,
}

impl ThemeDto {
    fn build(
        self,
    ) -> NativeResult<(
        Vec<(
            ThemeKey,
            Option<ThemeColor>,
            Vec<(StyleSelector, ThemeColor)>,
        )>,
        Vec<(ThemeKey, Option<StyleSpec>, Vec<(StyleSelector, StyleSpec)>)>,
        Vec<(iyon_tui::binding::TextSelector, StyleSpec)>,
    )> {
        // BTreeMap iteration is key-sorted, matching the previous
        // `serde_json::Map` walk, so declaration numbers are stable.
        let mut colors = Vec::with_capacity(self.colors.len());
        for (key, entry) in self.colors {
            let key = ThemeKey::from(intern(&key));
            let base = entry.base.map(ThemeColorDto::build).transpose()?;
            let mut variants = Vec::with_capacity(entry.variants.len());
            for variant in entry.variants {
                variants.push((variant.selector.build(), variant.value.build()?));
            }
            colors.push((key, base, variants));
        }
        let mut styles = Vec::with_capacity(self.styles.len());
        for (key, entry) in self.styles {
            let key = ThemeKey::from(intern(&key));
            let base = entry.base.map(StyleDto::build).transpose()?;
            let mut variants = Vec::with_capacity(entry.variants.len());
            for variant in entry.variants {
                variants.push((variant.selector.build(), variant.value.build()?));
            }
            styles.push((key, base, variants));
        }
        let mut text_styles = Vec::with_capacity(self.text_styles.len());
        for entry in self.text_styles {
            text_styles.push((entry.selector.build()?, entry.value.build()?));
        }
        Ok((colors, styles, text_styles))
    }
}

fn intern(value: &str) -> std::sync::Arc<str> {
    iyon_tui::binding::intern_style_atom(value)
}

/// Decodes a theme payload into canonical tables. Variant lists compile in
/// one batch with a single sort per entry; duplicate selectors resolve
/// exactly as the sequential setters (last write wins, newest order).
pub(super) fn decode_theme(value: serde_json::Value) -> NativeResult<Theme> {
    let dto: ThemeDto = serde_json::from_value(value)
        .map_err(|error| invalid_input(format!("theme must decode: {error}")))?;
    let (colors, styles, text_styles) = dto.build()?;
    Ok(Theme::assemble_batched(colors, styles, text_styles))
}

#[derive(Debug, Deserialize)]
pub(super) struct BorderDto {
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    edges: Option<String>,
    #[serde(default)]
    color: Option<StyleColorDto>,
    #[serde(default)]
    glyphs: Option<BorderGlyphsDto>,
}

#[derive(Debug, Deserialize)]
struct BorderGlyphsDto {
    top: String,
    right: String,
    bottom: String,
    left: String,
    #[serde(rename = "topLeft")]
    top_left: String,
    #[serde(rename = "topRight")]
    top_right: String,
    #[serde(rename = "bottomLeft")]
    bottom_left: String,
    #[serde(rename = "bottomRight")]
    bottom_right: String,
}

/// Decodes the TextInput border option shared with the theme pipeline.
pub(super) fn build_border_spec(value: serde_json::Value) -> NativeResult<BorderSpec> {
    let dto: BorderDto = serde_json::from_value(value)
        .map_err(|error| invalid_input(format!("border must decode: {error}")))?;
    let mut spec = match dto.style.as_deref().unwrap_or("plain") {
        "plain" => BorderSpec::plain(),
        "rounded" => BorderSpec::rounded(),
        "double" => BorderSpec::double(),
        other => {
            return Err(invalid_input(format!("unknown border style `{other}`")));
        }
    };
    let top_bottom = match dto.edges.as_deref().unwrap_or("all") {
        "all" => false,
        "topBottom" => true,
        other => {
            return Err(invalid_input(format!("unknown border edges `{other}`")));
        }
    };
    if top_bottom {
        spec = spec.edges(BorderEdges::TOP_BOTTOM);
    }
    // Resolve the color once: custom glyphs replace the spec (dropping the
    // earlier color exactly as the previous walk did), so it applies again
    // below.
    let color = dto.color.map(StyleColorDto::build).transpose()?;
    if let Some(color) = color.clone() {
        spec = spec.color(color);
    }
    if let Some(glyphs) = dto.glyphs {
        spec = BorderSpec::custom(
            BorderGlyphs::new(
                glyphs.top,
                glyphs.right,
                glyphs.bottom,
                glyphs.left,
                glyphs.top_left,
                glyphs.top_right,
                glyphs.bottom_left,
                glyphs.bottom_right,
            )
            .map_err(|error| invalid_input(error.to_string()))?,
        );
        if top_bottom {
            spec = spec.edges(BorderEdges::TOP_BOTTOM);
        }
        if let Some(color) = color {
            spec = spec.color(color);
        }
    }
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iyon_tui::binding::{StyleSelector as PublicSelector, Theme as PublicTheme};
    use serde_json::json;

    fn theme_fixture() -> serde_json::Value {
        json!({
            "colors": {
                "accent": {
                    "base": {"type": "default"},
                    "variants": [
                        {"selector": {"focused": true}, "value": "red"},
                        {"selector": {"states": {"mode": "error"}}, "value": "#010203"},
                        {"selector": {"focused": true}, "value": {"type": "ansi", "value": 9}}
                    ]
                },
                "plain": {"base": "ansi:4"}
            },
            "styles": {
                "emphasis": {
                    "base": {"foreground": "theme:accent", "attributes": {"bold": true}},
                    "variants": [
                        {"selector": {"focusWithin": true, "states": {"mode": "x"}}, "value": {"background": "#ffffff"}}
                    ]
                }
            },
            "textStyles": [
                {
                    "selector": {
                        "roles": ["link", "heading"],
                        "parts": ["codeLabel"],
                        "annotations": [{"namespace": "a", "name": "b"}],
                        "language": "rust",
                        "origin": "markdown",
                        "format": "gfm",
                        "focused": true,
                        "states": {"k": "v"}
                    },
                    "value": {"attributes": {"italic": true, "bold": false}}
                }
            ]
        })
    }

    fn expected_fixture_theme() -> PublicTheme {
        use iyon_tui::binding::{StyleSpec as PublicStyle, TextSelector as PublicTextSelector};
        PublicTheme::new()
            .with_color("accent", ThemeColor::Default)
            .with_color_variant(
                "accent",
                PublicSelector::focused(),
                ThemeColor::Named(iyon_tui::binding::AnsiColor::Red),
            )
            .with_color_variant(
                "accent",
                PublicSelector::state("mode", "error"),
                ThemeColor::Rgb { r: 1, g: 2, b: 3 },
            )
            .with_color_variant("accent", PublicSelector::focused(), ThemeColor::Indexed(9))
            .with_color("plain", ThemeColor::Indexed(4))
            .with_style(
                "emphasis",
                PublicStyle::new()
                    .foreground(ColorSpec::theme("accent"))
                    .attribute(iyon_tui::binding::TextAttribute::Bold, true),
            )
            .with_style_variant(
                "emphasis",
                PublicSelector::focus_within().and_state("mode", "x"),
                PublicStyle::new().background(ColorSpec::rgb(0xff, 0xff, 0xff)),
            )
            .with_text_style(
                PublicTextSelector::role(iyon_tui::binding::TextRole::Link)
                    .and_role(iyon_tui::binding::TextRole::Heading)
                    .and_part(iyon_tui::binding::TextPart::CodeLabel)
                    .and_annotation(
                        &iyon_tui::binding::SemanticTag::new("a", "b").expect("valid tag"),
                    )
                    .language(&iyon_tui::binding::LanguageId::new("rust").expect("valid language"))
                    .origin(iyon_tui::binding::TextOrigin::new("markdown").expect("valid origin"))
                    .format(&iyon_tui::binding::FormatId::new("gfm").expect("valid format"))
                    .and_focused()
                    .and_state("k", "v"),
                PublicStyle::new()
                    .attribute(iyon_tui::binding::TextAttribute::Italic, true)
                    .attribute(iyon_tui::binding::TextAttribute::Bold, false),
            )
    }

    #[test]
    fn theme_dto_matches_sequential_construction() {
        assert_eq!(
            decode_theme(theme_fixture()).expect("decodes"),
            expected_fixture_theme()
        );
    }

    #[test]
    fn theme_dto_rejects_malformed_leaves() {
        // Unknown color object shapes have no meaning.
        assert!(
            decode_theme(json!({"colors": {"a": {"base": {"type": "named", "value": "red"}}}}))
                .is_err()
        );
        assert!(
            decode_theme(json!({"colors": {"a": {"base": {"type": "theme", "key": "b"}}}}))
                .is_err()
        );
        // Bare `{value}` without a type marker is not an ANSI color.
        assert!(decode_theme(json!({"colors": {"a": {"base": {"value": 3}}}})).is_err());
        // Unknown attributes, roles, and parts are rejected.
        assert!(
            decode_theme(json!({"styles": {"a": {"base": {"attributes": {"blink": true}}}}}))
                .is_err()
        );
        assert!(
            decode_theme(json!({"textStyles": [{"selector": {"roles": ["bogus"]}, "value": {}}]}))
                .is_err()
        );
        assert!(
            decode_theme(json!({"textStyles": [{"selector": {"parts": ["bogus"]}, "value": {}}]}))
                .is_err()
        );
        // An empty selector is valid and selects everything.
        assert!(decode_theme(json!({"textStyles": [{"selector": {}, "value": {}}]})).is_ok());
    }

    #[test]
    fn border_dto_matches_lower_border_shapes() {
        use iyon_tui::binding::BorderGlyphs as PublicGlyphs;
        assert_eq!(
            build_border_spec(json!({})).expect("decodes"),
            BorderSpec::plain()
        );
        let custom = build_border_spec(json!({
            "style": "rounded",
            "edges": "topBottom",
            "color": "ansi:2",
            "glyphs": {
                "top": "─", "right": "│", "bottom": "─", "left": "│",
                "topLeft": "╭", "topRight": "╮", "bottomLeft": "╰", "bottomRight": "╯"
            }
        }))
        .expect("custom border decodes");
        assert_eq!(
            custom,
            BorderSpec::custom(
                PublicGlyphs::new("─", "│", "─", "│", "╭", "╮", "╰", "╯").expect("valid glyphs")
            )
            .edges(BorderEdges::TOP_BOTTOM)
            .color(ColorSpec::ansi(2))
        );
        assert!(build_border_spec(json!({"style": "wavy"})).is_err());
        assert!(build_border_spec(json!({"edges": "sides"})).is_err());
    }
}
