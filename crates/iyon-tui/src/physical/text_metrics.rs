//! Canonical terminal-cell geometry.
//!
//! Unicode grapheme identity (UAX #29) and terminal cell occupancy are different
//! concepts. The framework segments source into extended grapheme clusters first, then
//! asks for a width **once**. Every later layer — wrap, Grid measurement, paint,
//! clipping, cursor, stream compile — must consume that stored width rather than
//! re-querying a different crate.
//!
//! The baseline is termwiz's grapheme-oriented `grapheme_column_width`, because
//! The framework lowers through termwiz. A hand-maintained emoji range table cannot track
//! variation selectors, ZWJ sequences, regional indicators, or Unicode version.
//! Residual disagreement with a particular TTY (iTerm Unicode 9+ widths, etc.)
//! is a terminal-policy mismatch, diagnosed by the opt-in width probe — not
//! special-cased here.
//!
//! Zero-width clusters (a combining mark that survived as its own EGC) allocate
//! no physical cell. `row_from_graphemes` skips them; they must not become
//! leaders.

use unicode_segmentation::UnicodeSegmentation;

/// Terminal cells occupied by a single extended grapheme cluster.
///
/// `grapheme` must be one EGC. Callers that already stored this value on a
/// `StyledGrapheme` must keep using the stored value rather than calling this
/// again, except in debug validation.
pub(crate) fn grapheme_cell_width(grapheme: &str) -> usize {
    if grapheme.is_empty() {
        return 0;
    }
    debug_assert_eq!(
        grapheme.graphemes(true).count(),
        1,
        "grapheme_cell_width requires a single extended grapheme cluster, got {grapheme:?}"
    );
    termwiz::cell::grapheme_column_width(grapheme, None)
}

/// Sum of [`grapheme_cell_width`] over each EGC in `text`.
///
/// Use this for whole strings that have not yet been segmented into styled
/// graphemes. Once a `StyledGrapheme.width` exists, add those stored widths.
pub(crate) fn text_cell_width(text: &str) -> usize {
    text.graphemes(true).map(grapheme_cell_width).sum()
}

#[cfg(test)]
mod tests {
    use super::{grapheme_cell_width, text_cell_width};

    /// Corpus used to lock the framework metric to termwiz, including the sequences
    /// that `unicode-width` under-counts (keycaps, VS-16, ZWJ, flags).
    const CORPUS: &[&str] = &[
        "a",
        " ",
        "☆",
        "⭐",
        "☀︎",
        "☀️",
        "4⃣",
        "4️⃣",
        "🥇",
        "🕹️",
        "🗡️",
        "☕",
        "漢",
        "e\u{301}",
        "🇮🇩",
        "🇩🇰",
        "👩‍🔬",
        "🐕‍🦺",
        "👨‍👩‍👧‍👦",
        "🏳️‍🌈",
        "👋🏻",
        "🏴󠁧󠁢󠁳󠁣󠁴󠁿",
    ];

    #[test]
    fn iyon_metric_matches_termwiz_for_each_corpus_grapheme() {
        for sample in CORPUS {
            let clusters: Vec<_> =
                unicode_segmentation::UnicodeSegmentation::graphemes(*sample, true).collect();
            assert_eq!(clusters.len(), 1, "{sample:?} must be one EGC");
            assert_eq!(
                grapheme_cell_width(sample),
                termwiz::cell::grapheme_column_width(sample, None),
                "The framework and termwiz must agree on {sample:?}"
            );
        }
    }

    #[test]
    fn standalone_zero_width_clusters_occupy_no_cells() {
        // A combining acute that is its own cluster is not a physical leader.
        let mark = "\u{301}";
        assert_eq!(grapheme_cell_width(mark), 0);
        assert_eq!(text_cell_width(mark), 0);
    }

    #[test]
    fn text_cell_width_sums_clusters() {
        assert_eq!(text_cell_width("ab"), 2);
        assert_eq!(
            text_cell_width("💛 Age"),
            grapheme_cell_width("💛") + 1 + 1 + 1 + 1
        );
    }
}
