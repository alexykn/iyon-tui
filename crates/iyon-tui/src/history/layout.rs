use crate::presentation::Insets;

/// Semantic layout configuration for one History flow.
///
/// This describes reflowable resident presentation; native durability state
/// remains private to History.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryLayout {
    pub(crate) padding: Insets,
    pub(crate) gap: u16,
}

impl HistoryLayout {
    /// Starts with default padding and gap; configure with fluent setters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            padding: Insets::ZERO,
            gap: 0,
        }
    }

    #[must_use]
    pub const fn from_parts(padding: Insets, gap: u16) -> Self {
        Self { padding, gap }
    }

    #[must_use]
    pub const fn with_padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub const fn with_gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    #[must_use]
    pub const fn padding(self) -> Insets {
        self.padding
    }

    #[must_use]
    pub const fn gap(self) -> u16 {
        self.gap
    }
}
