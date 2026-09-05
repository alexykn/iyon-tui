//! Frame-lifetime candidate overlay over the committed state version table.
//!
//! The registry keeps one committed immutable version per live state record.
//! A frame capture owns exactly the versions the candidate needs beyond what
//! the committed table already serves: changed versions drained through the
//! captured epoch, plus newly demanded (desired-but-not-yet-visible) versions
//! for structural mounts and remounts. Readers look through the overlay first
//! and fall back to the committed table, so a paint-only patch on one
//! attachment never visits or snapshots unrelated unmounted states.
//!
//! This is a frame-lifetime mechanism, not a second mutable state authority.
//! Old `Arc` versions survive only while a visible or in-flight frame needs
//! them; commit folds touched entries into the owner table by construction
//! because the overlay shares the same immutable `Arc` values.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::presentation::ViewStateSnapshot;

/// Changed and newly demanded state versions captured for one frame attempt.
#[derive(Clone, Debug, Default)]
pub(crate) struct StateCandidateOverlay {
    /// Capture epoch. Dirty marks at or below this epoch were drained; newer
    /// marks stay queued for the next attempt.
    epoch: u64,
    /// Sorted attachment identities demanded by the candidate frame
    /// (desired ∪ visible ∪ in-flight at capture time).
    demanded: Vec<u64>,
    /// Immutable versions for changed-through-epoch and newly demanded ids.
    /// Clean unchanged ids are served from the committed table instead.
    touched: HashMap<u64, std::sync::Arc<ViewStateSnapshot>>,
}

impl StateCandidateOverlay {
    pub(crate) fn new(
        epoch: u64,
        demanded: Vec<u64>,
        touched: HashMap<u64, std::sync::Arc<ViewStateSnapshot>>,
    ) -> Self {
        Self {
            epoch,
            demanded,
            touched,
        }
    }

    pub(crate) fn empty_singleton() -> &'static Self {
        static EMPTY: OnceLock<StateCandidateOverlay> = OnceLock::new();
        EMPTY.get_or_init(StateCandidateOverlay::default)
    }

    #[must_use]
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub(crate) fn demanded(&self) -> &[u64] {
        &self.demanded
    }

    #[must_use]
    pub(crate) fn touched_len(&self) -> usize {
        self.touched.len()
    }

    #[must_use]
    pub(crate) fn contains_touched(&self, id: u64) -> bool {
        self.touched.contains_key(&id)
    }
}

/// Borrowed frame-time lookup: one candidate overlay over the committed
/// version table. Reuses the existing map-lookup shape; values are immutable
/// shared versions, so clean reads cost a pointer dereference and touched
/// reads cost an `Arc` dereference with no map or decoration copies.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StateFrameView<'a> {
    committed: &'a HashMap<u64, std::sync::Arc<ViewStateSnapshot>>,
    overlay: &'a StateCandidateOverlay,
}

impl<'a> StateFrameView<'a> {
    pub(crate) fn new(
        committed: &'a HashMap<u64, std::sync::Arc<ViewStateSnapshot>>,
        overlay: &'a StateCandidateOverlay,
    ) -> Self {
        Self { committed, overlay }
    }

    /// Empty view for paths with no retained state (tests, stateless frames).
    /// Returns an owned `'static` view so call sites keep the familiar
    /// `&StateFrameView::empty()` shape.
    pub(crate) fn empty() -> StateFrameView<'static> {
        static COMMITTED: OnceLock<HashMap<u64, std::sync::Arc<ViewStateSnapshot>>> =
            OnceLock::new();
        StateFrameView {
            committed: COMMITTED.get_or_init(HashMap::new),
            overlay: StateCandidateOverlay::empty_singleton(),
        }
    }

    /// Reads through the candidate overlay, then the committed table.
    /// Returns the shared version so branch overlays can retain it with a
    /// reference-count bump instead of cloning maps and decorations.
    /// Dereferences to `&ViewStateSnapshot` at use sites.
    #[must_use]
    pub(crate) fn get(&self, id: &u64) -> Option<&'a std::sync::Arc<ViewStateSnapshot>> {
        self.overlay
            .touched
            .get(id)
            .or_else(|| self.committed.get(id))
    }

    /// Attachment identities demanded at capture time. Branch overlays
    /// populate exactly these ids; unmounted records are never visited.
    #[must_use]
    pub(crate) fn demanded(&self) -> &[u64] {
        self.overlay.demanded()
    }

    #[must_use]
    pub(crate) fn overlay_epoch(&self) -> u64 {
        self.overlay.epoch()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_view_resolves_nothing() {
        let view = StateFrameView::empty();
        assert!(view.get(&1).is_none());
        assert!(view.demanded().is_empty());
    }

    #[test]
    fn overlay_shadows_committed_without_copying_clean_entries() {
        let mut committed = HashMap::new();
        committed.insert(1, std::sync::Arc::new(ViewStateSnapshot::default()));
        committed.insert(2, std::sync::Arc::new(ViewStateSnapshot::default()));
        let mut changed = ViewStateSnapshot::default();
        changed.revision = 7;
        let mut touched = HashMap::new();
        touched.insert(2, std::sync::Arc::new(changed));
        let overlay = StateCandidateOverlay::new(3, vec![1, 2], touched);
        let view = StateFrameView::new(&committed, &overlay);
        assert_eq!(view.get(&1).unwrap().revision, 0);
        assert_eq!(view.get(&2).unwrap().revision, 7);
        assert!(view.get(&3).is_none());
        assert_eq!(view.overlay_epoch(), 3);
    }
}
