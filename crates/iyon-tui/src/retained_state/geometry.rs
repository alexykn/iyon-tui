//! Retained geometry overrides and their native effect classification.
//!
//! Geometry remains an immutable semantic View value until a ViewState is
//! attached.  These records are the host-owned sparse overlay used by the
//! candidate layout transaction; they never change semantic topology.

use crate::presentation::ir::{AxisBounds, Decoration, HeightRule, WidthRule};
use crate::presentation::{HorizontalAlign, Insets, VerticalAlign};

use super::effects::StateEffects;

/// Typed geometry values accepted by the retained-state control surface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewStateGeometryPatch {
    pub width: Option<ViewStateSizeMode>,
    pub height: Option<ViewStateSizeMode>,
    pub padding: Option<Insets>,
    pub min_width: Option<Option<u16>>,
    pub max_width: Option<Option<u16>>,
    pub min_height: Option<Option<u16>>,
    pub max_height: Option<Option<u16>>,
    pub gap: Option<u16>,
    pub alignment: Option<GeometryAlignment>,
    pub border_edges: Option<Option<crate::presentation::BorderEdges>>,
}

/// Which alignment axes a retained patch changes.  The target node kind
/// decides which axis is legal during H3 prepare.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeometryAlignment {
    pub horizontal: Option<HorizontalAlign>,
    pub vertical: Option<VerticalAlign>,
}

/// Sparse, host-owned geometry override storage.  The outer Option means
/// "this property has an override"; nullable bounds/edges retain an inner
/// Option so explicit null remains distinct from clear.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeometryOverrides {
    pub(crate) width: Option<ViewStateSizeMode>,
    pub(crate) height: Option<ViewStateSizeMode>,
    pub(crate) padding: Option<Insets>,
    pub(crate) min_width: Option<Option<u16>>,
    pub(crate) max_width: Option<Option<u16>>,
    pub(crate) min_height: Option<Option<u16>>,
    pub(crate) max_height: Option<Option<u16>>,
    pub(crate) gap: Option<u16>,
    pub(crate) alignment: Option<GeometryAlignment>,
    pub(crate) border_edges: Option<Option<crate::presentation::BorderEdges>>,
}

impl GeometryOverrides {
    pub(crate) fn apply_patch(&mut self, patch: &ViewStateGeometryPatch) -> StateEffects {
        let mut effects = StateEffects::NONE;
        if patch.width.is_some() && self.width != patch.width {
            self.width = patch.width;
            effects = effects.union(geometry_effects(GeometryProperty::Width));
        }
        if patch.height.is_some() && self.height != patch.height {
            self.height = patch.height;
            effects = effects.union(geometry_effects(GeometryProperty::Height));
        }
        if patch.padding.is_some() && self.padding != patch.padding {
            self.padding = patch.padding;
            effects = effects.union(geometry_effects(GeometryProperty::Padding));
        }
        if patch.min_width.is_some() && self.min_width != patch.min_width {
            self.min_width = patch.min_width;
            effects = effects.union(geometry_effects(GeometryProperty::MinWidth));
        }
        if patch.max_width.is_some() && self.max_width != patch.max_width {
            self.max_width = patch.max_width;
            effects = effects.union(geometry_effects(GeometryProperty::MaxWidth));
        }
        if patch.min_height.is_some() && self.min_height != patch.min_height {
            self.min_height = patch.min_height;
            effects = effects.union(geometry_effects(GeometryProperty::MinHeight));
        }
        if patch.max_height.is_some() && self.max_height != patch.max_height {
            self.max_height = patch.max_height;
            effects = effects.union(geometry_effects(GeometryProperty::MaxHeight));
        }
        if patch.gap.is_some() && self.gap != patch.gap {
            self.gap = patch.gap;
            effects = effects.union(geometry_effects(GeometryProperty::Gap));
        }
        if patch.alignment.is_some() && self.alignment != patch.alignment {
            self.alignment = patch.alignment;
            effects = effects.union(geometry_effects(GeometryProperty::Alignment));
        }
        if patch.border_edges.is_some() && self.border_edges != patch.border_edges {
            self.border_edges = patch.border_edges;
            effects = effects.union(geometry_effects(GeometryProperty::BorderEdges));
        }
        effects
    }

    pub(crate) fn clear(
        &mut self,
        properties: Option<&[ViewStateGeometryProperty]>,
    ) -> StateEffects {
        let before = self.clone();
        if let Some(properties) = properties {
            for property in properties {
                match property {
                    ViewStateGeometryProperty::Width => self.width = None,
                    ViewStateGeometryProperty::Height => self.height = None,
                    ViewStateGeometryProperty::Padding => self.padding = None,
                    ViewStateGeometryProperty::MinWidth => self.min_width = None,
                    ViewStateGeometryProperty::MaxWidth => self.max_width = None,
                    ViewStateGeometryProperty::MinHeight => self.min_height = None,
                    ViewStateGeometryProperty::MaxHeight => self.max_height = None,
                    ViewStateGeometryProperty::Gap => self.gap = None,
                    ViewStateGeometryProperty::Alignment => self.alignment = None,
                    ViewStateGeometryProperty::BorderEdges => self.border_edges = None,
                }
            }
        } else {
            *self = Self::default();
        }
        effects_for_difference(&before, self)
    }

    pub(crate) fn effective(
        &self,
        width: WidthRule,
        height: HeightRule,
        decoration: &Decoration,
        gap: Option<u16>,
        alignment: GeometryAlignment,
    ) -> EffectiveGeometry {
        let mut decoration = decoration.clone();
        if let Some(padding) = self.padding {
            decoration.padding = padding;
        }
        apply_axis_bound(&mut decoration.bounds.width, self.min_width, self.max_width);
        apply_axis_bound(
            &mut decoration.bounds.height,
            self.min_height,
            self.max_height,
        );
        if let Some(edges) = &self.border_edges {
            match edges {
                Some(edges) => {
                    let border = decoration
                        .border
                        .get_or_insert_with(crate::presentation::BorderSpec::plain);
                    border.set_edges(*edges);
                }
                None => decoration.border = None,
            }
        }
        let alignment = self
            .alignment
            .map_or(alignment, |override_value| GeometryAlignment {
                horizontal: override_value.horizontal.or(alignment.horizontal),
                vertical: override_value.vertical.or(alignment.vertical),
            });
        EffectiveGeometry {
            width: self.width.map_or(width, ViewStateSizeMode::to_width_rule),
            height: self
                .height
                .map_or(height, ViewStateSizeMode::to_height_rule),
            decoration,
            gap: self.gap.or(gap),
            alignment,
        }
    }
}

fn apply_axis_bound(
    bounds: &mut AxisBounds,
    minimum: Option<Option<u16>>,
    maximum: Option<Option<u16>>,
) {
    if let Some(minimum) = minimum {
        bounds.min = minimum.unwrap_or(0);
    }
    if let Some(maximum) = maximum {
        bounds.max = maximum.unwrap_or(u16::MAX);
    }
}

fn effects_for_difference(before: &GeometryOverrides, after: &GeometryOverrides) -> StateEffects {
    let mut effects = StateEffects::NONE;
    if before.width != after.width {
        effects = effects.union(geometry_effects(GeometryProperty::Width));
    }
    if before.height != after.height {
        effects = effects.union(geometry_effects(GeometryProperty::Height));
    }
    if before.padding != after.padding {
        effects = effects.union(geometry_effects(GeometryProperty::Padding));
    }
    if before.min_width != after.min_width {
        effects = effects.union(geometry_effects(GeometryProperty::MinWidth));
    }
    if before.max_width != after.max_width {
        effects = effects.union(geometry_effects(GeometryProperty::MaxWidth));
    }
    if before.min_height != after.min_height {
        effects = effects.union(geometry_effects(GeometryProperty::MinHeight));
    }
    if before.max_height != after.max_height {
        effects = effects.union(geometry_effects(GeometryProperty::MaxHeight));
    }
    if before.gap != after.gap {
        effects = effects.union(geometry_effects(GeometryProperty::Gap));
    }
    if before.alignment != after.alignment {
        effects = effects.union(geometry_effects(GeometryProperty::Alignment));
    }
    if before.border_edges != after.border_edges {
        effects = effects.union(geometry_effects(GeometryProperty::BorderEdges));
    }
    effects
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStateGeometryProperty {
    Width,
    Height,
    Padding,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    Gap,
    Alignment,
    BorderEdges,
}

/// Candidate geometry after the immutable View base and retained overrides
/// have been combined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectiveGeometry {
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) gap: Option<u16>,
    pub(crate) alignment: GeometryAlignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStateSizeMode {
    Fit,
    Fill,
}

impl ViewStateSizeMode {
    fn to_width_rule(self) -> WidthRule {
        match self {
            Self::Fit => WidthRule::Fit,
            Self::Fill => WidthRule::Fill,
        }
    }

    fn to_height_rule(self) -> HeightRule {
        match self {
            Self::Fit => HeightRule::Fit,
            Self::Fill => HeightRule::Fill,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeometryProperty {
    Width,
    Height,
    Padding,
    MinWidth,
    MaxWidth,
    MinHeight,
    MaxHeight,
    Gap,
    Alignment,
    BorderEdges,
}

fn geometry_effects(property: GeometryProperty) -> StateEffects {
    let mut effects = StateEffects::GEOMETRY
        .union(StateEffects::PAINT_SUBTREE)
        .union(StateEffects::DAMAGE_OLD)
        .union(StateEffects::DAMAGE_NEW);
    match property {
        GeometryProperty::Width
        | GeometryProperty::Padding
        | GeometryProperty::MinWidth
        | GeometryProperty::MaxWidth
        | GeometryProperty::BorderEdges => {
            effects = effects
                .union(StateEffects::PROJECT_CONTENT)
                .union(StateEffects::MEASURE_SELF)
                .union(StateEffects::MEASURE_ANCESTORS)
                .union(StateEffects::PLACE_SELF)
                .union(StateEffects::PLACE_DESCENDANTS)
                .union(StateEffects::UPDATE_CLIP);
        }
        GeometryProperty::Height | GeometryProperty::MinHeight | GeometryProperty::MaxHeight => {
            effects = effects
                .union(StateEffects::MEASURE_SELF)
                .union(StateEffects::MEASURE_ANCESTORS)
                .union(StateEffects::PLACE_SELF)
                .union(StateEffects::PLACE_DESCENDANTS)
                .union(StateEffects::UPDATE_CLIP);
        }
        GeometryProperty::Gap => {
            effects = effects
                .union(StateEffects::MEASURE_SELF)
                .union(StateEffects::MEASURE_ANCESTORS)
                .union(StateEffects::PLACE_DESCENDANTS);
        }
        GeometryProperty::Alignment => {
            effects = effects
                .union(StateEffects::PLACE_SELF)
                .union(StateEffects::PLACE_DESCENDANTS);
        }
    }
    let intrinsic = match property {
        GeometryProperty::Width | GeometryProperty::MinWidth | GeometryProperty::MaxWidth => {
            // Width changes can alter descendant wrapping and therefore
            // height as well as the immediate intrinsic width.
            StateEffects::INTRINSIC_WIDTH.union(StateEffects::INTRINSIC_HEIGHT)
        }
        GeometryProperty::Height | GeometryProperty::MinHeight | GeometryProperty::MaxHeight => {
            StateEffects::INTRINSIC_HEIGHT
        }
        GeometryProperty::Padding | GeometryProperty::Gap | GeometryProperty::BorderEdges => {
            StateEffects::INTRINSIC_WIDTH.union(StateEffects::INTRINSIC_HEIGHT)
        }
        GeometryProperty::Alignment => StateEffects::NONE,
    };
    effects.union(intrinsic)
}
