//! Generic content inputs shared by retained layout and paint.
//!
//! The application/content registry implements this boundary. Presentation
//! code consumes only immutable measurements and derived surfaces; it never
//! reaches into Source/Port/Connector lifecycle or scheduling state.

use crate::{geometry::Size, physical::Surface};

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

/// Presentation-facing content provider. Implementations may prepare/reuse a
/// Connector projection while measuring, but painting only reads the prepared
/// result. No method owns or mutates Source bytes or viewport state.
pub(crate) trait ContentProvider {
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
}

/// Empty provider used by generic layout/paint callers and existing tests that
/// do not mount a retained ContentPort.
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
