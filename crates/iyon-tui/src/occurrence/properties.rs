use std::fmt;

use super::generated::{EffectMask, HostKind, PropertyId, ValueKind, property_descriptor};
pub use crate::presentation::api::style::{
    BorderEdges as Edges, BorderGlyphs as GlyphsValue, BorderStyle, ColorSpec as ColorValue,
    Insets, StyleRef as StyleValue, TextAttributeSpec as TextAttributes,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SizeMode {
    Fit,
    Fill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutMode {
    Box,
    Row,
    Column,
    Grid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlignmentAxis {
    Start,
    Center,
    End,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Alignment {
    pub horizontal: Option<AlignmentAxis>,
    pub vertical: Option<AlignmentAxis>,
}

impl Alignment {
    pub const fn new(horizontal: Option<AlignmentAxis>, vertical: Option<AlignmentAxis>) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyValue {
    SizeMode(SizeMode),
    LayoutMode(LayoutMode),
    U16(u16),
    Insets(Insets),
    Alignment(Alignment),
    Edges(Edges),
    Color(ColorValue),
    BorderStyle(BorderStyle),
    Glyphs(GlyphsValue),
    TextAttributes(TextAttributes),
    Style(StyleValue),
}

impl PropertyValue {
    pub(crate) const fn value_kind(&self) -> ValueKind {
        match self {
            Self::SizeMode(_) => ValueKind::SizeMode,
            Self::LayoutMode(_) => ValueKind::LayoutMode,
            Self::U16(_) => ValueKind::U16,
            Self::Insets(_) => ValueKind::Insets,
            Self::Alignment(_) => ValueKind::Alignment,
            Self::Edges(_) => ValueKind::Edges,
            Self::Color(_) => ValueKind::Color,
            Self::BorderStyle(_) => ValueKind::BorderStyle,
            Self::Glyphs(_) => ValueKind::Glyphs,
            Self::TextAttributes(_) => ValueKind::TextAttributes,
            Self::Style(_) => ValueKind::Style,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayerValue {
    Unset,
    Null,
    Value(PropertyValue),
}

impl Default for LayerValue {
    fn default() -> Self {
        Self::Unset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyLayer {
    Declared,
    Override,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PropertyChange {
    pub(crate) changed: bool,
    pub(crate) effective_changed: bool,
    pub(crate) effects: EffectMask,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyError {
    UnsupportedKind {
        property: PropertyId,
        node_kind: HostKind,
    },
    NullNotAllowed(PropertyId),
    ValueKindMismatch {
        property: PropertyId,
        expected: ValueKind,
        actual: ValueKind,
    },
    NotClearable(PropertyId),
}

impl fmt::Display for PropertyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind {
                property,
                node_kind,
            } => write!(
                formatter,
                "property {property:?} is not legal on {node_kind:?}"
            ),
            Self::NullNotAllowed(property) => {
                write!(formatter, "property {property:?} does not accept null")
            }
            Self::ValueKindMismatch {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "property {property:?} expects {expected:?}, got {actual:?}"
            ),
            Self::NotClearable(property) => write!(
                formatter,
                "property {property:?} cannot be reset by this schema"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PropertyLayers {
    declared: Vec<LayerValue>,
    overrides: Vec<LayerValue>,
}

impl Default for PropertyLayers {
    fn default() -> Self {
        Self {
            declared: vec![LayerValue::Unset; super::generated::PROPERTY_COUNT],
            overrides: vec![LayerValue::Unset; super::generated::PROPERTY_COUNT],
        }
    }
}

impl PropertyLayers {
    pub(crate) fn apply(
        &mut self,
        layer: PropertyLayer,
        node_kind: HostKind,
        property: PropertyId,
        value: LayerValue,
    ) -> Result<PropertyChange, PropertyError> {
        let descriptor = property_descriptor(property);
        if !descriptor.legal_kinds.contains(&node_kind) {
            return Err(PropertyError::UnsupportedKind {
                property,
                node_kind,
            });
        }
        validate_value(descriptor.value_kind, property, &value, descriptor.nullable)?;
        let index = property.index();
        let old_value = match layer {
            PropertyLayer::Declared => &self.declared[index],
            PropertyLayer::Override => &self.overrides[index],
        };
        if *old_value == value {
            return Ok(PropertyChange::default());
        }
        let old_effective = self.effective(property);
        match layer {
            PropertyLayer::Declared => self.declared[index] = value,
            PropertyLayer::Override => self.overrides[index] = value,
        }
        let new_effective = self.effective(property);
        Ok(PropertyChange {
            changed: true,
            effective_changed: old_effective != new_effective,
            effects: descriptor.effects,
        })
    }

    pub(crate) fn reset_declared(
        &mut self,
        node_kind: HostKind,
        property: PropertyId,
    ) -> Result<PropertyChange, PropertyError> {
        let descriptor = property_descriptor(property);
        if !descriptor.legal_kinds.contains(&node_kind) {
            return Err(PropertyError::UnsupportedKind {
                property,
                node_kind,
            });
        }
        if !descriptor.clearable {
            return Err(PropertyError::NotClearable(property));
        }
        self.apply(
            PropertyLayer::Declared,
            node_kind,
            property,
            LayerValue::Unset,
        )
    }

    pub(crate) fn clear_override(
        &mut self,
        node_kind: HostKind,
        property: PropertyId,
    ) -> Result<PropertyChange, PropertyError> {
        let descriptor = property_descriptor(property);
        if !descriptor.legal_kinds.contains(&node_kind) {
            return Err(PropertyError::UnsupportedKind {
                property,
                node_kind,
            });
        }
        if !descriptor.clearable {
            return Err(PropertyError::NotClearable(property));
        }
        self.apply(
            PropertyLayer::Override,
            node_kind,
            property,
            LayerValue::Unset,
        )
    }

    pub(crate) fn declared(&self, property: PropertyId) -> &LayerValue {
        &self.declared[property.index()]
    }

    pub(crate) fn override_value(&self, property: PropertyId) -> &LayerValue {
        &self.overrides[property.index()]
    }

    pub(crate) fn effective(&self, property: PropertyId) -> LayerValue {
        match self.override_value(property) {
            LayerValue::Unset => self.declared(property).clone(),
            value => value.clone(),
        }
    }
}

fn validate_value(
    expected: ValueKind,
    property: PropertyId,
    value: &LayerValue,
    nullable: bool,
) -> Result<(), PropertyError> {
    match value {
        LayerValue::Unset => Ok(()),
        LayerValue::Null if nullable => Ok(()),
        LayerValue::Null => Err(PropertyError::NullNotAllowed(property)),
        LayerValue::Value(value) if value.value_kind() == expected => Ok(()),
        LayerValue::Value(value) => Err(PropertyError::ValueKindMismatch {
            property,
            expected,
            actual: value.value_kind(),
        }),
    }
}
