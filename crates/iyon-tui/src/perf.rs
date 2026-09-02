//! Opt-in instrumentation for the TUI performance refactor.
//!
//! This module is intentionally hidden behind the `perf-counters` feature. It
//! is a measurement seam for benchmark tooling, not ordinary framework API.

#![allow(dead_code)]

#[cfg(all(test, feature = "perf-counters"))]
use std::cell::Cell;
#[cfg(feature = "perf-counters")]
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Counter {
    ViewNodesConstructedRust,
    ViewCloneCalls,
    NapiViewNodesSeen,
    NapiViewCacheHits,
    NapiViewCacheMisses,
    NapiViewStringBytesCopied,
    ResolverNodesVisited,
    ComponentViewCalls,
    ComponentCapabilityCalls,
    MeasureNodeCalls,
    TextFlowMeasureCalls,
    PrepareNodeCalls,
    LayoutNodesEmitted,
    PaintNodesVisited,
    PaintCellsAllocated,
    PaintCacheHits,
    PaintCacheMisses,
    SurfaceCellsComposited,
    HistoryUnitsExamined,
    HistoryUnitsMeasured,
    HistoryCachedHeightHits,
    PersistentSeqNodesAllocated,
    PersistentSeqLeafClones,
    PersistentSeqBranchClones,
    ComponentGeometryNodesVisited,
    ViewStateMutationsAccepted,
    ViewStateMutationsNoop,
    ViewStatePresentationInvalidations,
    ViewStateStyleStateInvalidations,
    ViewStateIncrementalPaints,
    ViewStateDamageRects,
    ViewStateFullDamageFallbacks,
    ViewStateGeometryInvalidations,
    ViewStateGeometryRelayouts,
    ViewStateGeometryLocalPatches,
    ViewStateGeometryFullFallbacks,
    ViewStateDirtyPropagationNodes,
    DecoratedNormalizedNodes,
}

impl Counter {
    pub const COUNT: usize = Self::DecoratedNormalizedNodes as usize + 1;

    const fn index(self) -> usize {
        self as usize
    }
}

const NAMES: [&str; Counter::COUNT] = [
    "view_nodes_constructed_rust",
    "view_clone_calls",
    "napi_view_nodes_seen",
    "napi_view_cache_hits",
    "napi_view_cache_misses",
    "napi_view_string_bytes_copied",
    "resolver_nodes_visited",
    "component_view_calls",
    "component_capability_calls",
    "measure_node_calls",
    "text_flow_measure_calls",
    "prepare_node_calls",
    "layout_nodes_emitted",
    "paint_nodes_visited",
    "paint_cells_allocated",
    "paint_cache_hits",
    "paint_cache_misses",
    "surface_cells_composited",
    "history_units_examined",
    "history_units_measured",
    "history_cached_height_hits",
    "persistent_seq_nodes_allocated",
    "persistent_seq_leaf_clones",
    "persistent_seq_branch_clones",
    "component_geometry_nodes_visited",
    "view_state_mutations_accepted",
    "view_state_mutations_noop",
    "view_state_presentation_invalidations",
    "view_state_style_state_invalidations",
    "view_state_incremental_paints",
    "view_state_damage_rects",
    "view_state_full_damage_fallbacks",
    "view_state_geometry_invalidations",
    "view_state_geometry_relayouts",
    "view_state_geometry_local_patches",
    "view_state_geometry_full_fallbacks",
    "view_state_dirty_propagation_nodes",
    "decorated_normalized_nodes",
];

#[cfg(feature = "perf-counters")]
static VALUES: [AtomicU64; Counter::COUNT] = [const { AtomicU64::new(0) }; Counter::COUNT];

#[cfg(all(test, feature = "perf-counters"))]
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(all(test, feature = "perf-counters"))]
thread_local! {
    static TEST_COUNTERS_ENABLED: Cell<bool> = const { Cell::new(false) };
}

#[cfg(all(test, feature = "perf-counters"))]
pub(crate) struct TestLockGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(all(test, feature = "perf-counters"))]
impl Drop for TestLockGuard {
    fn drop(&mut self) {
        TEST_COUNTERS_ENABLED.with(|enabled| enabled.set(false));
    }
}

#[cfg(all(test, feature = "perf-counters"))]
pub(crate) fn test_lock() -> TestLockGuard {
    let lock = TEST_LOCK.lock().expect("performance test lock poisoned");
    TEST_COUNTERS_ENABLED.with(|enabled| enabled.set(true));
    TestLockGuard { _lock: lock }
}

/// A point-in-time copy of every performance counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PerfSnapshot {
    values: [u64; Counter::COUNT],
}

impl Default for PerfSnapshot {
    fn default() -> Self {
        Self {
            values: [0; Counter::COUNT],
        }
    }
}

impl PerfSnapshot {
    /// Returns one counter value by its stable machine-readable identity.
    pub fn value(self, counter: Counter) -> u64 {
        self.values[counter.index()]
    }

    /// Iterates counters in the canonical JSONL output order.
    pub fn iter(self) -> impl Iterator<Item = (&'static str, u64)> {
        NAMES.iter().copied().zip(self.values)
    }
}

/// Clears all counters.
#[inline]
pub fn reset() {
    #[cfg(feature = "perf-counters")]
    for value in &VALUES {
        value.store(0, Ordering::Relaxed);
    }
}

/// Increments one counter by one.
#[inline(always)]
pub fn inc(counter: Counter) {
    add(counter, 1);
}

/// Adds a measured amount of work to one counter.
#[inline(always)]
pub fn add(counter: Counter, amount: u64) {
    #[cfg(all(test, feature = "perf-counters"))]
    if !TEST_COUNTERS_ENABLED.with(Cell::get) {
        return;
    }
    #[cfg(feature = "perf-counters")]
    VALUES[counter.index()].fetch_add(amount, Ordering::Relaxed);
    #[cfg(not(feature = "perf-counters"))]
    let _ = (counter, amount);
}

/// Sets a counter used as a current restart/offset gauge.
#[inline(always)]
pub fn set(counter: Counter, value: u64) {
    #[cfg(all(test, feature = "perf-counters"))]
    if !TEST_COUNTERS_ENABLED.with(Cell::get) {
        return;
    }
    #[cfg(feature = "perf-counters")]
    VALUES[counter.index()].store(value, Ordering::Relaxed);
    #[cfg(not(feature = "perf-counters"))]
    let _ = (counter, value);
}

/// Reads all counters atomically.
#[inline]
pub fn snapshot() -> PerfSnapshot {
    #[cfg(feature = "perf-counters")]
    {
        let mut values = [0; Counter::COUNT];
        for (index, value) in VALUES.iter().enumerate() {
            values[index] = value.load(Ordering::Relaxed);
        }
        PerfSnapshot { values }
    }

    #[cfg(not(feature = "perf-counters"))]
    {
        PerfSnapshot::default()
    }
}
