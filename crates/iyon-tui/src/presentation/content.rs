//! Generic content inputs shared by retained layout and paint.
//!
//! The application/content registry implements this boundary. Presentation
//! code consumes only immutable measurements and derived surfaces; it never
//! reaches into Source/Port/Connector lifecycle or scheduling state.

use crate::{
    Theme,
    geometry::Size,
    physical::{PhysicalRow, Surface},
};

/// Intrinsic metrics returned by a Connector projection for one offered width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentMeasurement {
    pub(crate) intrinsic_size: Size,
    /// Whether the derived rows fit whole graphemes in the offered width.
    pub(crate) physically_complete: bool,
    /// Revision/fingerprint of the derived projection used for these metrics.
    /// Layout caches include this value in addition to the offered width.
    pub(crate) projection_revision: u64,
}

impl Default for ContentMeasurement {
    fn default() -> Self {
        Self {
            intrinsic_size: Size::new(0, 0),
            physically_complete: true,
            projection_revision: 0,
        }
    }
}

/// Physical rows that a `ContentHost` can transfer into native History. Open
/// content may expose only a stable prefix; sealed content marks its rows as
/// complete so the semantic unit can be retired after receipt.
#[derive(Debug)]
pub(crate) struct HistoryContentRows {
    pub(crate) rows: Vec<PhysicalRow>,
    pub(crate) complete: bool,
    /// Content-row range within `rows`; padding rows may surround it.
    pub(crate) content_start: usize,
    pub(crate) content_end: usize,
    pub(crate) leading_padding: usize,
    pub(crate) trailing_padding: usize,
}

/// Presentation-facing content provider. Implementations may prepare/reuse a
/// Connector projection while measuring, but painting only reads the prepared
/// result. No method owns or mutates Source bytes or viewport state.
pub(crate) trait ContentProvider {
    /// Supplies the host theme before the candidate layout/paint pass. The
    /// content provider stores only semantic/theme data, never host-native IDs.
    fn set_theme(&mut self, _theme: &Theme) {}

    fn projection_revision(&self, port_id: u64, offered_width: u16) -> u64;

    fn measure(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::WidthRule,
    ) -> ContentMeasurement;

    fn paint(
        &self,
        port_id: u64,
        offered_width: u16,
        allocated_height: u16,
    ) -> Option<std::sync::Arc<Surface>>;

    /// Returns committed/candidate physical rows for a History `ContentHost`.
    /// Open content may return only a stable prefix; `complete` is true when
    /// the returned suffix reaches the sealed end of the unit.
    fn history_rows(&self, _port_id: u64, _offered_width: u16) -> Option<HistoryContentRows> {
        None
    }

    /// Records rows accepted by the native scrollback sink. The provider
    /// advances its own source-rooted frontier only after this receipt.
    fn history_rows_committed(
        &mut self,
        _port_id: u64,
        _rows: usize,
        _content_rows: usize,
        _leading_padding: usize,
        _trailing_padding: usize,
    ) {
    }

    /// Returns the resident History view for a `ContentHost`. Providers may
    /// remove decoration already accepted by native History while preserving
    /// the caller's body occurrence unchanged.
    fn history_view(&self, view: &crate::presentation::View) -> crate::presentation::View {
        view.clone()
    }

    /// Releases a History-backed `ContentHost` after its rows have been
    /// accepted by the native scrollback sink.
    fn history_unit_retired(&mut self, _unit_id: u64) {}

    /// True when the front resident History `ContentHost` has no transferable
    /// rows for the offered width and should remain follow-end anchored.
    fn history_transfer_blocked(&self, _port_id: u64, _offered_width: u16) -> bool {
        false
    }
}

/// Empty provider used by generic layout/paint callers and existing tests that
/// do not mount a retained `ContentPort`.
#[derive(Default)]
pub(crate) struct EmptyContentProvider;

impl ContentProvider for EmptyContentProvider {
    fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
        0
    }

    fn measure(
        &mut self,
        _port_id: u64,
        _offered_width: u16,
        _width_rule: crate::presentation::WidthRule,
    ) -> ContentMeasurement {
        ContentMeasurement::default()
    }

    fn paint(
        &self,
        _port_id: u64,
        _offered_width: u16,
        _allocated_height: u16,
    ) -> Option<std::sync::Arc<Surface>> {
        None
    }
}
