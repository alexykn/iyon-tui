//! Presentation override values and effective-state derivation.

use std::collections::BTreeMap;

use crate::presentation::ir::{Decoration, HeightRule, WidthRule};

use super::geometry::{EffectiveGeometry, GeometryAlignment, GeometryOverrides};
use crate::presentation::{
    BorderGlyphs, BorderStyle, ColorSpec, StyleRef, StyleSpec, StyleStates, TextAttribute,
};

/// Typed sparse text-attribute values accepted by a presentation patch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewStateTextAttributes {
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub reversed: Option<bool>,
    pub strikethrough: Option<bool>,
}

impl ViewStateTextAttributes {
    pub(crate) fn apply_to(&self, style: &mut StyleRef) {
        for (attribute, value) in [
            (TextAttribute::Bold, self.bold),
            (TextAttribute::Dim, self.dim),
            (TextAttribute::Italic, self.italic),
            (TextAttribute::Underline, self.underline),
            (TextAttribute::Reversed, self.reversed),
            (TextAttribute::Strikethrough, self.strikethrough),
        ] {
            if let Some(value) = value {
                style.set_attribute(attribute, value);
            }
        }
    }
}

/// All presentation fields supported by PERF-13-B.
///
/// `Some(Some(value))` is an explicit override, `Some(None)` is an explicit
/// nullable semantic value, and `None` means that the retained override is
/// absent. Clear operations restore the outer `None` state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewStatePresentationPatch {
    pub foreground: Option<Option<ColorSpec>>,
    pub background: Option<Option<ColorSpec>>,
    pub border_color: Option<Option<ColorSpec>>,
    pub border_style: Option<Option<BorderStyle>>,
    pub border_glyphs: Option<Option<BorderGlyphs>>,
    pub text_attributes: ViewStateTextAttributes,
    pub style: Option<Option<StyleRef>>,
}

/// Named presentation domains accepted by `clearPresentation`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStatePresentationProperty {
    Foreground,
    Background,
    BorderColor,
    BorderStyle,
    BorderGlyphs,
    TextAttributes,
    Style,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PresentationOverrides {
    pub(crate) foreground: Option<Option<ColorSpec>>,
    pub(crate) background: Option<Option<ColorSpec>>,
    pub(crate) border_color: Option<Option<ColorSpec>>,
    pub(crate) border_style: Option<Option<BorderStyle>>,
    pub(crate) border_glyphs: Option<Option<BorderGlyphs>>,
    pub(crate) text_attributes: ViewStateTextAttributes,
    pub(crate) style: Option<Option<StyleRef>>,
}

impl PresentationOverrides {
    pub(crate) fn apply_patch(&mut self, patch: &ViewStatePresentationPatch) -> bool {
        let before = self.clone();
        if patch.foreground.is_some() {
            self.foreground = patch.foreground.clone();
        }
        if patch.background.is_some() {
            self.background = patch.background.clone();
        }
        if patch.border_color.is_some() {
            self.border_color = patch.border_color.clone();
        }
        if patch.border_style.is_some() {
            self.border_style = patch.border_style;
        }
        if patch.border_glyphs.is_some() {
            self.border_glyphs = patch.border_glyphs.clone();
        }
        apply_attribute_patch(&mut self.text_attributes, &patch.text_attributes);
        if patch.style.is_some() {
            self.style = patch.style.clone();
        }
        *self != before
    }

    pub(crate) fn clear(&mut self, properties: Option<&[ViewStatePresentationProperty]>) -> bool {
        let before = self.clone();
        let Some(properties) = properties else {
            *self = Self::default();
            return *self != before;
        };
        for property in properties {
            match property {
                ViewStatePresentationProperty::Foreground => self.foreground = None,
                ViewStatePresentationProperty::Background => self.background = None,
                ViewStatePresentationProperty::BorderColor => self.border_color = None,
                ViewStatePresentationProperty::BorderStyle => self.border_style = None,
                ViewStatePresentationProperty::BorderGlyphs => self.border_glyphs = None,
                ViewStatePresentationProperty::TextAttributes => {
                    self.text_attributes = ViewStateTextAttributes::default()
                }
                ViewStatePresentationProperty::Style => self.style = None,
            }
        }
        *self != before
    }
}

fn apply_attribute_patch(target: &mut ViewStateTextAttributes, patch: &ViewStateTextAttributes) {
    if patch.bold.is_some() {
        target.bold = patch.bold;
    }
    if patch.dim.is_some() {
        target.dim = patch.dim;
    }
    if patch.italic.is_some() {
        target.italic = patch.italic;
    }
    if patch.underline.is_some() {
        target.underline = patch.underline;
    }
    if patch.reversed.is_some() {
        target.reversed = patch.reversed;
    }
    if patch.strikethrough.is_some() {
        target.strikethrough = patch.strikethrough;
    }
}

/// Immutable frame-time copy of one retained state record. Geometry and
/// presentation revisions remain separate for effect classification and
/// cache validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ViewStateSnapshot {
    pub(crate) id: u64,
    pub(crate) geometry: GeometryOverrides,
    pub(crate) presentation: PresentationOverrides,
    pub(crate) style_states: BTreeMap<String, String>,
    pub(crate) revision: u64,
    pub(crate) geometry_revision: u64,
    pub(crate) presentation_revision: u64,
}

impl ViewStateSnapshot {
    pub(crate) fn effective_geometry(
        &self,
        width: WidthRule,
        height: HeightRule,
        decoration: &Decoration,
        gap: Option<u16>,
        alignment: GeometryAlignment,
    ) -> EffectiveGeometry {
        let mut effective = self
            .geometry
            .effective(width, height, decoration, gap, alignment);
        effective.decoration = self.effective_decoration(&effective.decoration);
        effective
    }

    pub(crate) fn effective_decoration(&self, base: &Decoration) -> Decoration {
        let mut decoration = base.clone();
        if let Some(style) = &self.presentation.style {
            decoration.text_style = match style {
                Some(style) if style.is_themed() => style.clone(),
                Some(style) => {
                    let mut merged = decoration.text_style.clone();
                    merged.overlay(style);
                    merged
                }
                None => StyleRef::direct(StyleSpec::new()),
            };
        }
        if let Some(foreground) = &self.presentation.foreground {
            decoration.text_style.set_foreground(foreground.clone());
        }
        if let Some(background) = &self.presentation.background {
            decoration.surface_background = background.clone();
        }
        if let Some(border) = decoration.border.as_mut() {
            if let Some(color) = &self.presentation.border_color {
                border.set_color(color.clone());
            }
            if let Some(style) = self.presentation.border_style {
                border.set_style(style.unwrap_or(BorderStyle::Plain));
            }
            if let Some(glyphs) = &self.presentation.border_glyphs {
                match glyphs {
                    Some(glyphs) => border.set_glyphs(glyphs.clone()),
                    None => border.reset_glyphs_for_style(),
                }
            }
        }
        self.presentation
            .text_attributes
            .apply_to(&mut decoration.text_style);
        decoration
    }

    pub(crate) fn effective_style_states(&self, base: &StyleStates) -> StyleStates {
        let mut states = base.clone();
        for (key, value) in &self.style_states {
            states.set(key.clone(), value.clone());
        }
        states
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{BorderSpec, IntoView, View};

    #[test]
    fn explicit_null_and_clear_are_distinct() {
        let mut overrides = PresentationOverrides::default();
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(1)));
        assert!(overrides.apply_patch(&patch));
        assert_eq!(overrides.foreground, Some(Some(ColorSpec::ansi(1))));

        let mut null_patch = ViewStatePresentationPatch::default();
        null_patch.foreground = Some(None);
        assert!(overrides.apply_patch(&null_patch));
        assert_eq!(overrides.foreground, Some(None));

        assert!(overrides.clear(Some(&[ViewStatePresentationProperty::Foreground])));
        assert_eq!(overrides.foreground, None);
    }

    #[test]
    fn effective_presentation_preserves_base_and_applies_sparse_override() {
        let base = View::text("x")
            .background(ColorSpec::ansi(2))
            .foreground(ColorSpec::ansi(3))
            .border(BorderSpec::plain())
            .into_view();
        let mut record = super::super::record::ViewStateRecord::new(1);
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(4)));
        patch.text_attributes.bold = Some(true);
        assert!(!record.apply_presentation(&patch).is_empty());
        let snapshot = record.snapshot();
        let effective = snapshot.effective_decoration(base.decoration());
        assert_eq!(
            effective.surface_background,
            base.decoration().surface_background
        );
        assert_eq!(
            effective.text_style.local_foreground(),
            Some(&ColorSpec::ansi(4))
        );
        assert_eq!(
            effective.text_style.attribute_value(TextAttribute::Bold),
            Some(true)
        );
    }
}
