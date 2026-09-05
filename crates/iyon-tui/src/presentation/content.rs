//! Generic content inputs shared by retained layout and paint.
//!
//! The application/content registry implements this boundary. Presentation
//! code consumes only immutable measurements and prepared row products; it never
//! reaches into Source/Port/Connector lifecycle or scheduling state.

use crate::{
    Theme,
    geometry::Size,
    physical::{PhysicalRow, Surface},
};

/// Viewport row window requested during content painting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentWindow {
    pub(crate) first_row: u64,
    pub(crate) row_count: u32,
}

impl ContentWindow {
    pub(crate) const fn full(row_count: u32) -> Self {
        Self {
            first_row: 0,
            row_count,
        }
    }
}

/// Prepared projection ticket shared by measure and paint within a frame attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedProjectionTicket {
    pub(crate) port_id: u64,
    /// Connector identity is part of the prepared product.  A Port can have
    /// several Connectors with identical widths, so width is never a safe
    /// selector during paint.
    pub(crate) connector_id: Option<u64>,
    pub(crate) offered_width: u16,
    pub(crate) projection_revision: u64,
    /// Monotonic identity of the immutable prepared product retained by the
    /// candidate.  It is an identity witness only; the provider validates it
    /// while the owning product remains live and fails closed when unavailable.
    pub(crate) projection_identity: u64,
}

/// Intrinsic metrics returned by a Connector projection for one offered width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentMeasurement {
    pub(crate) intrinsic_size: Size,
    /// Whether the derived rows fit whole graphemes in the offered width.
    pub(crate) physically_complete: bool,
    /// Revision/fingerprint of the derived projection used for these metrics.
    /// Layout caches include this value in addition to the offered width.
    pub(crate) projection_revision: u64,
    /// Metric revision: changes only when intrinsic dimensions or completeness change.
    pub(crate) metric_revision: u64,
    /// Paint revision: changes when colors, theme, or delivery frontier change.
    pub(crate) paint_revision: u64,
    /// Exact Connector/product identity used to derive this measurement.
    /// These fields stay private to the presentation/content seam.
    pub(crate) connector_id: Option<u64>,
    pub(crate) projection_identity: u64,
}

impl Default for ContentMeasurement {
    fn default() -> Self {
        Self {
            intrinsic_size: Size::new(0, 0),
            physically_complete: true,
            projection_revision: 0,
            metric_revision: 0,
            paint_revision: 0,
            connector_id: None,
            projection_identity: 0,
        }
    }
}

/// Physical rows that a `ContentHost` can transfer into native History. Open
/// content may expose only a finalized prefix, and a sealed smoothed Source
/// may still expose only the delivered prefix; the unit is complete only when
/// that receipt reaches the delivered sealed end.
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
    /// theme arrives shared: providers adopt the `Arc` instead of cloning
    /// its maps, and compare by pointer before comparing by value.
    fn set_theme(&mut self, _theme: &std::sync::Arc<Theme>) {}

    fn projection_revision(&self, port_id: u64, offered_width: u16) -> u64;

    fn layout_input_revision(&self, port_id: u64, offered_width: u16) -> u64 {
        self.projection_revision(port_id, offered_width)
    }

    fn measure(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::WidthRule,
    ) -> ContentMeasurement;

    /// Direct row-window paint contract. Writes directly into `target` at
    /// `target_origin` clipped to `clip` without full offscreen surface allocation.
    fn paint_window(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (u16, u16),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    );

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

    fn paint_window(
        &self,
        _ticket: PreparedProjectionTicket,
        _window: ContentWindow,
        _target: &mut Surface,
        _target_origin: (u16, u16),
        _clip: crate::geometry::Rect,
        _style: crate::physical::PhysicalStyle,
    ) {
    }
}
