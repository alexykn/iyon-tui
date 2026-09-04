//! Opaque stream coordinates.

/// Opaque monotonic coordinate within one stream's root source space.
///
/// Text sources conventionally use UTF-8 byte offsets. Other sources may use
/// event or record ordinals. Exact text projection APIs remain byte-specific;
/// arbitrary-coordinate values must use replacement or atomic boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct StreamOffset(pub(crate) u64);

impl StreamOffset {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(offset: u64) -> Self {
        Self(offset)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn checked_add(self, rhs: u64) -> Option<Self> {
        match self.0.checked_add(rhs) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn saturating_add(self, rhs: u64) -> Self {
        Self(self.0.saturating_add(rhs))
    }
}

/// A half-open range `[start, end)` in one stream's root coordinate space.
/// For text-specific APIs this range is a UTF-8 byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StreamRange {
    pub(crate) start: StreamOffset,
    pub(crate) end: StreamOffset,
}

impl StreamRange {
    #[must_use]
    pub const fn new(start: StreamOffset, end: StreamOffset) -> Self {
        assert!(start.0 <= end.0, "stream range start exceeds end");
        Self { start, end }
    }

    #[must_use]
    pub const fn try_new(start: StreamOffset, end: StreamOffset) -> Option<Self> {
        if start.0 <= end.0 {
            Some(Self { start, end })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn start(self) -> StreamOffset {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> StreamOffset {
        self.end
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub fn len(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }

    #[must_use]
    pub fn contains_offset(&self, offset: StreamOffset) -> bool {
        offset >= self.start && offset < self.end
    }
}
