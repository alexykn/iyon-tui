//! Rust-owned effect classification for retained-state mutations.

/// Bit-set of consequences produced by an effective retained-state change.
/// TypeScript never supplies this authority; Rust derives it from the field
/// transition and the current PERF-13 capability table.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StateEffects(u16);

impl StateEffects {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const RESOLVE_STYLE: Self = Self(1 << 0);
    pub(crate) const PAINT_SELF: Self = Self(1 << 1);
    pub(crate) const PAINT_SUBTREE: Self = Self(1 << 2);
    pub(crate) const DAMAGE: Self = Self(1 << 3);
    pub(crate) const GEOMETRY: Self = Self(1 << 4);
    pub(crate) const MEASURE_SELF: Self = Self(1 << 5);
    pub(crate) const MEASURE_ANCESTORS: Self = Self(1 << 6);
    pub(crate) const PLACE_SELF: Self = Self(1 << 7);
    pub(crate) const PLACE_DESCENDANTS: Self = Self(1 << 8);
    pub(crate) const UPDATE_CLIP: Self = Self(1 << 9);
    pub(crate) const DAMAGE_OLD: Self = Self(1 << 10);
    pub(crate) const DAMAGE_NEW: Self = Self(1 << 11);
    pub(crate) const PROJECT_CONTENT: Self = Self(1 << 12);
    pub(crate) const INTRINSIC_WIDTH: Self = Self(1 << 13);
    pub(crate) const INTRINSIC_HEIGHT: Self = Self(1 << 14);

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) fn geometry(self) -> bool {
        self.contains(Self::GEOMETRY)
    }

    pub(crate) fn intrinsic_width(self) -> bool {
        self.contains(Self::INTRINSIC_WIDTH)
    }

    pub(crate) fn intrinsic_height(self) -> bool {
        self.contains(Self::INTRINSIC_HEIGHT)
    }
}

/// Presentation properties affect the attached occurrence's resolved style;
/// style-state values conservatively repaint the descendant subtree because
/// selectors may observe inherited state.
pub(crate) fn presentation_effects(has_style_state_change: bool) -> StateEffects {
    crate::perf::inc(crate::perf::Counter::ViewStatePresentationInvalidations);
    let paint = if has_style_state_change {
        crate::perf::inc(crate::perf::Counter::ViewStateStyleStateInvalidations);
        StateEffects::PAINT_SUBTREE
    } else {
        StateEffects::PAINT_SELF
    };
    StateEffects::RESOLVE_STYLE
        .union(paint)
        .union(StateEffects::DAMAGE)
}
