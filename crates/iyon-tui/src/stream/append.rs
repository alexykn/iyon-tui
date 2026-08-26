//! Generic append-only stability helpers.

use unicode_segmentation::UnicodeSegmentation;

use super::coord::StreamOffset;

/// Conservative stability helper for plain append-only text.
///
/// When sealed, the entire text is stable.
/// An open source ending in a newline is stable through that hard-line boundary.
/// Otherwise, hold back the trailing extended grapheme cluster so that partial
/// combining sequences are never committed before completion.
pub(crate) fn append_only_text_stable_frontier(
    source: &str,
    base: StreamOffset,
    sealed: bool,
) -> StreamOffset {
    if sealed || source.is_empty() || source.ends_with('\n') {
        return base.saturating_add(source.len() as u64);
    }

    let mut last_offset = 0;
    for (offset, _grapheme) in source.grapheme_indices(true) {
        last_offset = offset;
    }

    base.saturating_add(last_offset as u64)
}
