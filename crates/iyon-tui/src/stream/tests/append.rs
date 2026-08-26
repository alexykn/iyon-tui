use crate::stream::StreamOffset;
use crate::stream::append::append_only_text_stable_frontier;

#[test]
fn open_append_frontier_withholds_trailing_egc() {
    assert_eq!(
        append_only_text_stable_frontier("e\u{301}", StreamOffset::ZERO, false),
        StreamOffset::ZERO
    );
    assert_eq!(
        append_only_text_stable_frontier("e\u{301}", StreamOffset::ZERO, true),
        StreamOffset::new(3)
    );
}

#[test]
fn open_append_frontier_commits_a_completed_hard_line() {
    assert_eq!(
        append_only_text_stable_frontier("line\n", StreamOffset::ZERO, false),
        StreamOffset::new(5)
    );
}
