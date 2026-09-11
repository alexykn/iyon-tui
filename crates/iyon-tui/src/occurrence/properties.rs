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

/// A validated finite f32 scalar. The stored bits are canonical: NaNs and
/// infinities are rejected and negative zero is represented as positive zero.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FiniteScalar(u32);

impl FiniteScalar {
    pub fn new(value: f32) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let value = if value == 0.0 { 0.0 } else { value };
        Some(Self(value.to_bits()))
    }

    pub fn from_bits(bits: u32) -> Option<Self> {
        Self::new(f32::from_bits(bits))
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DimensionValue {
    Length(FiniteScalar),
    Percent(FiniteScalar),
    Auto,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DisplayMode {
    Flex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DirectionMode {
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FlexDirectionMode {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FlexWrapMode {
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionMode {
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AlignmentMode {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
    SpaceBetween,
    SpaceEvenly,
    SpaceAround,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GridAutoFlowMode {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TrackValue {
    Length(FiniteScalar),
    Percent(FiniteScalar),
    Fr(FiniteScalar),
    Auto,
    MinContent,
    MaxContent,
    MinMax {
        min: TrackMinBound,
        max: TrackMaxBound,
    },
}

/// Finite minimum side of a `minmax()` track. Flex fractions are not legal
/// here; keeping that fact in the type prevents a renderer fallback for an
/// impossible recursive value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TrackMinBound {
    Auto,
    Length(FiniteScalar),
    Percent(FiniteScalar),
    MinContent,
    MaxContent,
}

/// Finite maximum side of a `minmax()` track. Unlike the minimum side, the
/// maximum may be a flex fraction.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TrackMaxBound {
    Auto,
    Length(FiniteScalar),
    Percent(FiniteScalar),
    Fr(FiniteScalar),
    MinContent,
    MaxContent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TrackListValue(pub Vec<TrackValue>);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GridLineValue {
    Auto,
    Line(i32),
    Span(u16),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridPlacementValue {
    pub start: GridLineValue,
    pub end: GridLineValue,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DimensionInsets {
    pub top: DimensionValue,
    pub right: DimensionValue,
    pub bottom: DimensionValue,
    pub left: DimensionValue,
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
    Dimension(DimensionValue),
    Scalar(FiniteScalar),
    Display(DisplayMode),
    Direction(DirectionMode),
    FlexDirection(FlexDirectionMode),
    FlexWrap(FlexWrapMode),
    Position(PositionMode),
    AlignmentMode(AlignmentMode),
    GridAutoFlow(GridAutoFlowMode),
    Dimensions(DimensionInsets),
    Tracks(TrackListValue),
    GridPlacement(GridPlacementValue),
}

impl PropertyValue {
    pub(crate) const fn value_kind(&self) -> ValueKind {
        match self {
            // Width and height retain their historical `fit`/`fill` Rust
            // values while the wire's Dimension kind also admits finite
            // length and percentage forms.
            Self::SizeMode(_) => ValueKind::Dimension,
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
            Self::Dimension(_) => ValueKind::Dimension,
            Self::Scalar(_) => ValueKind::F32,
            Self::Display(_) => ValueKind::Display,
            Self::Direction(_) => ValueKind::Direction,
            Self::FlexDirection(_) => ValueKind::FlexDirection,
            Self::FlexWrap(_) => ValueKind::FlexWrap,
            Self::Position(_) => ValueKind::Position,
            Self::AlignmentMode(_) => ValueKind::AlignmentMode,
            Self::GridAutoFlow(_) => ValueKind::GridAutoFlow,
            Self::Dimensions(_) => ValueKind::InsetsF32,
            Self::Tracks(_) => ValueKind::TrackList,
            Self::GridPlacement(_) => ValueKind::GridPlacement,
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

    pub(crate) fn effective_values(&self) -> Vec<(PropertyId, LayerValue)> {
        PropertyId::ALL
            .iter()
            .map(|property| (*property, self.effective(*property)))
            .collect()
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
