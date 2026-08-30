//! Canonical physical occurrence box for retained state.
//!
//! The box is present for every state-capable layout occurrence, even when
//! its base decoration is empty. A ViewState changes this record's effective
//! values; it never creates a structural wrapper node.

use crate::presentation::api::StyleStates;
use crate::presentation::ir::Decoration;

use super::ViewStateSnapshot;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OccurrenceBox {
    pub(crate) state_attachment: Option<u64>,
    pub(crate) base_decoration: Decoration,
    pub(crate) effective_decoration: Decoration,
    pub(crate) base_style_states: StyleStates,
    pub(crate) effective_style_states: StyleStates,
}

impl OccurrenceBox {
    pub(crate) fn from_effective(
        state_attachment: Option<u64>,
        base_decoration: Decoration,
        effective_decoration: Decoration,
        base_style_states: StyleStates,
        effective_style_states: StyleStates,
    ) -> Self {
        Self {
            state_attachment,
            base_decoration,
            effective_decoration,
            base_style_states,
            effective_style_states,
        }
    }

    pub(crate) fn new(
        state_attachment: Option<u64>,
        base_decoration: Decoration,
        base_style_states: StyleStates,
        state: Option<&ViewStateSnapshot>,
    ) -> Self {
        let (effective_decoration, effective_style_states) = match state {
            Some(state) => (
                state.effective_decoration(&base_decoration),
                state.effective_style_states(&base_style_states),
            ),
            None => (base_decoration.clone(), base_style_states.clone()),
        };
        Self::from_effective(
            state_attachment,
            base_decoration,
            effective_decoration,
            base_style_states,
            effective_style_states,
        )
    }

    pub(crate) fn apply_state(&mut self, state: &ViewStateSnapshot) {
        self.effective_decoration = state.effective_decoration(&self.base_decoration);
        self.effective_style_states = state.effective_style_states(&self.base_style_states);
    }
}
