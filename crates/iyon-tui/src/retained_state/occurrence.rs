//! Canonical physical occurrence box for retained state.
//!
//! The box is present for every state-capable layout occurrence, even when
//! its base decoration is empty. A ViewState changes this record's effective
//! values; it never creates a structural wrapper node.

use crate::presentation::api::StyleStates;
use crate::presentation::ir::{Decoration, HeightRule, WidthRule};

use super::{EffectiveGeometry, GeometryAlignment, StateNodeKind, ViewStateSnapshot};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OccurrenceBox {
    pub(crate) state_attachment: Option<u64>,
    pub(crate) node_kind: StateNodeKind,
    pub(crate) base_width: WidthRule,
    pub(crate) base_height: HeightRule,
    pub(crate) effective_width: WidthRule,
    pub(crate) effective_height: HeightRule,
    pub(crate) base_gap: Option<u16>,
    pub(crate) effective_gap: Option<u16>,
    pub(crate) base_alignment: GeometryAlignment,
    pub(crate) effective_alignment: GeometryAlignment,
    pub(crate) base_decoration: Decoration,
    pub(crate) effective_decoration: Decoration,
    pub(crate) base_style_states: StyleStates,
    pub(crate) effective_style_states: StyleStates,
}

impl OccurrenceBox {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_effective(
        state_attachment: Option<u64>,
        node_kind: StateNodeKind,
        base_width: WidthRule,
        base_height: HeightRule,
        base_gap: Option<u16>,
        base_alignment: GeometryAlignment,
        base_decoration: Decoration,
        effective: EffectiveGeometry,
        base_style_states: StyleStates,
        effective_style_states: StyleStates,
    ) -> Self {
        Self {
            state_attachment,
            node_kind,
            base_width,
            base_height,
            effective_width: effective.width,
            effective_height: effective.height,
            base_gap,
            effective_gap: effective.gap,
            base_alignment,
            effective_alignment: effective.alignment,
            base_decoration,
            effective_decoration: effective.decoration,
            base_style_states,
            effective_style_states,
        }
    }

    pub(crate) fn apply_state(&mut self, state: &ViewStateSnapshot) {
        let effective = state.effective_geometry(
            self.base_width,
            self.base_height,
            &self.base_decoration,
            self.base_gap,
            self.base_alignment,
        );
        self.effective_width = effective.width;
        self.effective_height = effective.height;
        self.effective_gap = effective.gap;
        self.effective_alignment = effective.alignment;
        self.effective_decoration = effective.decoration;
        self.effective_style_states = state.effective_style_states(&self.base_style_states);
    }
}
