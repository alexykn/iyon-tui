use crate::presentation::HorizontalAlign;
use crate::presentation::api::style::VerticalAlign;
use crate::presentation::ir::{Decoration, HeightRule, WidthRule};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeometryAlignment {
    pub(crate) horizontal: Option<HorizontalAlign>,
    pub(crate) vertical: Option<VerticalAlign>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectiveGeometry {
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) gap: Option<u16>,
    pub(crate) alignment: GeometryAlignment,
}
