//! Bounded canonical table for immutable style atoms.
//!
//! Hot ingress paths (structural style atoms, state envelope color strings,
//! theme keys) parse the same small set of color/key strings repeatedly.
//! Interning them once avoids re-allocating identical `ThemeKey` strings on
//! every materialization while keeping a single shared vocabulary.
//!
//! Ownership and reclamation: the table is bounded (FIFO eviction). Values
//! leave the table as `Arc<str>`, so evicting a table entry never
//! invalidates a still-live semantic value — live owners keep their own
//! reference. Fingerprints are unnecessary here: `HashMap` equality on the
//! string content resolves every lookup.
//!
//! The canonical process table backs stateless ingress helpers in both
//! crates. Tests construct local tables directly so global state cannot leak
//! between assertions.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

/// Default bound for the canonical table. Theme keys, color atoms, and
/// annotation names number in the dozens per application; 2048 leaves ample
/// headroom while keeping a pathological unique-value stream bounded.
pub(crate) const CANONICAL_ATOM_CAPACITY: usize = 2048;

#[derive(Debug)]
pub(crate) struct StyleAtomTable {
    entries: HashMap<String, Arc<str>>,
    /// Insertion order with the generation each entry refers to, so a stale
    /// order entry can never evict a re-interned newer generation.
    order: VecDeque<(String, Arc<str>)>,
    capacity: usize,
}

impl StyleAtomTable {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Returns the canonical shared value for `value`, inserting and
    /// evicting oldest-first when the table is full.
    pub(crate) fn intern(&mut self, value: &str) -> Arc<str> {
        if let Some(hit) = self.entries.get(value) {
            return Arc::clone(hit);
        }
        let shared: Arc<str> = Arc::from(value);
        self.order
            .push_back((value.to_owned(), Arc::clone(&shared)));
        self.entries.insert(value.to_owned(), Arc::clone(&shared));
        while self.entries.len() > self.capacity {
            let Some((oldest, generation)) = self.order.pop_front() else {
                break;
            };
            if self
                .entries
                .get(&oldest)
                .is_some_and(|current| Arc::ptr_eq(current, &generation))
            {
                self.entries.remove(&oldest);
            }
        }
        shared
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Canonical process-wide atom authority shared by every style ingress path.
fn canonical_style_atoms() -> &'static Mutex<StyleAtomTable> {
    static TABLE: OnceLock<Mutex<StyleAtomTable>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(StyleAtomTable::with_capacity(CANONICAL_ATOM_CAPACITY)))
}

/// Interns one style atom through the canonical table. This is the single
/// ingress helper for repeated `theme:`/`ansi:`/`#rrggbb`/named color strings
/// and theme keys: identical inputs share one allocation, a unique-value
/// stream stays bounded, and live `Arc` values survive eviction.
pub fn intern_style_atom(value: &str) -> Arc<str> {
    canonical_style_atoms()
        .lock()
        .map(|mut table| table.intern(value))
        .unwrap_or_else(|_| Arc::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_interns_share_one_allocation() {
        let mut table = StyleAtomTable::with_capacity(8);
        let first = table.intern("diff.addition");
        let second = table.intern("diff.addition");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn unique_value_stream_stays_bounded_and_live_values_survive() {
        let mut table = StyleAtomTable::with_capacity(8);
        let live = table.intern("live-key");
        for index in 0..100 {
            table.intern(&format!("stream-key-{index}"));
        }
        assert_eq!(table.len(), 8);
        assert_eq!(table.capacity(), 8);
        // The evicted live value is still fully usable through its own Arc.
        assert_eq!(&*live, "live-key");
        // Re-interning an evicted key returns equal content again.
        assert_eq!(&*table.intern("live-key"), "live-key");
    }
}
