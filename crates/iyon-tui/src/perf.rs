//! Opt-in instrumentation for the TUI performance refactor.
//!
//! This module is intentionally hidden behind the `perf-counters` feature. It
//! is a measurement seam for benchmark tooling, not ordinary framework API.

#[cfg(all(test, feature = "perf-counters"))]
use std::cell::Cell;
#[cfg(feature = "perf-counters")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "perf-counters")]
use std::time::Instant;

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Counter {
    ComponentCapabilityCalls,
    SurfaceCellsComposited,
    SourceSnapshotsAcquired,
    ContentRegistryPortScans,
    SemanticProjectionRebuilds,
    ContentWakeGroups,
    ContentDueConnectors,
    ContentCandidateRecordsPrepared,
    ContentDemandNodesVisited,
    ContentOwnerNodesVisited,
    UiControlKeysVisited,
    // These stage timing variants are only observed by ScopedTimer in
    // perf-counters builds. Their names remain in the canonical counter lane
    // so benchmarks can distinguish stage ownership without changing the
    // default addon surface.
    FramePrepareNanos,
    RuntimeAdvanceNanos,
    FramePresentNanos,
    FrameCommitNanos,
    DirectCaptureNanos,
    DirectRefinementNanos,
    ContentProjectionNanos,
    TaffyLayoutNanos,
    TaffyLayoutPasses,
    DirectPaintNanos,
}

impl Counter {
    pub const COUNT: usize = Self::DirectPaintNanos as usize + 1;

    const fn index(self) -> usize {
        self as usize
    }
}

const NAMES: [&str; Counter::COUNT] = [
    "component_capability_calls",
    "surface_cells_composited",
    "source_snapshots_acquired",
    "content_registry_port_scans",
    "semantic_projection_rebuilds",
    "content_wake_groups",
    "content_due_connectors",
    "content_candidate_records_prepared",
    "content_demand_nodes_visited",
    "content_owner_nodes_visited",
    "ui_control_keys_visited",
    "frame_prepare_nanos",
    "runtime_advance_nanos",
    "frame_present_nanos",
    "frame_commit_nanos",
    "direct_capture_nanos",
    "direct_refinement_nanos",
    "content_projection_nanos",
    "taffy_layout_nanos",
    "taffy_layout_passes",
    "direct_paint_nanos",
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
    #[must_use]
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

/// Accumulates elapsed time for one actual native-owned stage.
///
/// This is deliberately scoped to the performance feature. The default build
/// pays neither for an `Instant` nor for a timing branch, and benchmark output
/// can identify the owner of each measured stage instead of inferring native
/// work from a JavaScript wall-clock interval.
#[cfg(feature = "perf-counters")]
pub(crate) struct ScopedTimer {
    counter: Counter,
    started: Instant,
}

#[cfg(feature = "perf-counters")]
impl ScopedTimer {
    #[must_use]
    pub(crate) fn new(counter: Counter) -> Self {
        Self {
            counter,
            started: Instant::now(),
        }
    }
}

#[cfg(feature = "perf-counters")]
impl Drop for ScopedTimer {
    fn drop(&mut self) {
        add(
            self.counter,
            u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        );
    }
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
