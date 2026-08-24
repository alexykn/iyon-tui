//! Opt-in instrumentation for the TUI performance refactor.
//!
//! This module is intentionally hidden behind the `perf-counters` feature. It
//! is a measurement seam for benchmark tooling, not ordinary framework API.

#![allow(dead_code)]

#[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Counter {
    ViewNodesConstructedRust,
    ViewNodesDeepCopied,
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
    StreamSourceNodesExamined,
    StreamRowsReindexed,
    StreamStableRowsReused,
    StreamSemanticRestartOffset,
    StreamVisualRestartOffset,
    PackedEncoderNodesVisited,
    PackedEncoderRefRecords,
    PackedEncoderDefRecords,
    PackedEncoderWordsUsed,
    PackedEncoderStrings,
    PackedEncoderStringBytes,
    PackedEncoderBufferGrows,
    PackedEncoderRefPacketHits,
    PackedEncoderCacheResyncs,
    PackedEncoderColdRetries,
    NapiPackedNodesSeen,
    NapiPackedRefHits,
    NapiPackedRefMisses,
    NapiPackedDefsDecoded,
    NapiPackedWordsRead,
    NapiPackedStringBytesCopied,
    NapiV3Transactions,
    NapiV3ExactRefCalls,
    NapiV3PersistentRefUpgrades,
    NapiV3PersistentRefMisses,
    NapiV3LocalRefResolves,
    NapiV3FullViewsBuilt,
    NapiV3ViewsPatched,
    NapiV3SeqNodesBuilt,
    NapiV3SeqNodesReused,
    NapiV3CacheLockAcquisitions,
    NapiV3CachePublications,
    NapiV3WordsRead,
    NapiV3BytesRead,
    NapiV3Utf8Validations,
    NapiV3HostMutations,
    NapiV4Transactions,
    NapiV4ExactRefCalls,
    NapiV4PersistentRefUpgrades,
    NapiV4PersistentRefMisses,
    NapiV4LocalRefResolves,
    NapiV4FullViewsBuilt,
    NapiV4ViewsPatched,
    NapiV4SeqNodesBuilt,
    NapiV4SeqNodesReused,
    NapiV4CacheLockAcquisitions,
    NapiV4CachePublications,
    NapiV4WordsRead,
    NapiV4BytesBorrowed,
    NapiV4BytesCopiedToRetained,
    NapiV4Utf8Validations,
    NapiV4HostMutations,
    PersistentSeqFlattenCalls,
    PersistentSeqNodesAllocated,
    PersistentSeqLeafClones,
    PersistentSeqBranchClones,
    PersistentSeqItemsIteratedDuringPatch,
    FastTransactions,
    FastOpsRead,
    FastRefsResolved,
    FastSeqNodesBuilt,
    FastViewsBuilt,
    FastPublications,
    FastPagesRetained,
    FastPagesReleased,
    FastLivePageBytes,
    FastLivePayloadBytes,
    FastStatusCacheMiss,
    FastStatusInvalid,
}

impl Counter {
    pub const COUNT: usize = Self::FastStatusInvalid as usize + 1;

    const fn index(self) -> usize {
        self as usize
    }
}

const NAMES: [&str; Counter::COUNT] = [
    "view_nodes_constructed_rust",
    "view_nodes_deep_copied",
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
    "stream_source_nodes_examined",
    "stream_rows_reindexed",
    "stream_stable_rows_reused",
    "stream_semantic_restart_offset",
    "stream_visual_restart_offset",
    "packed_encoder_nodes_visited",
    "packed_encoder_ref_records",
    "packed_encoder_def_records",
    "packed_encoder_words_used",
    "packed_encoder_strings",
    "packed_encoder_string_bytes",
    "packed_encoder_buffer_grows",
    "packed_encoder_ref_packet_hits",
    "packed_encoder_cache_resyncs",
    "packed_encoder_cold_retries",
    "napi_packed_nodes_seen",
    "napi_packed_ref_hits",
    "napi_packed_ref_misses",
    "napi_packed_defs_decoded",
    "napi_packed_words_read",
    "napi_packed_string_bytes_copied",
    "napi_v3_transactions",
    "napi_v3_exact_ref_calls",
    "napi_v3_persistent_ref_upgrades",
    "napi_v3_persistent_ref_misses",
    "napi_v3_local_ref_resolves",
    "napi_v3_full_views_built",
    "napi_v3_views_patched",
    "napi_v3_seq_nodes_built",
    "napi_v3_seq_nodes_reused",
    "napi_v3_cache_lock_acquisitions",
    "napi_v3_cache_publications",
    "napi_v3_words_read",
    "napi_v3_bytes_read",
    "napi_v3_utf8_validations",
    "napi_v3_host_mutations",
    "napi_v4_transactions",
    "napi_v4_exact_ref_calls",
    "napi_v4_persistent_ref_upgrades",
    "napi_v4_persistent_ref_misses",
    "napi_v4_local_ref_resolves",
    "napi_v4_full_views_built",
    "napi_v4_views_patched",
    "napi_v4_seq_nodes_built",
    "napi_v4_seq_nodes_reused",
    "napi_v4_cache_lock_acquisitions",
    "napi_v4_cache_publications",
    "napi_v4_words_read",
    "napi_v4_bytes_borrowed",
    "napi_v4_bytes_copied_to_retained",
    "napi_v4_utf8_validations",
    "napi_v4_host_mutations",
    "persistent_seq_flatten_calls",
    "persistent_seq_nodes_allocated",
    "persistent_seq_leaf_clones",
    "persistent_seq_branch_clones",
    "persistent_seq_items_iterated_during_patch",
    "fast_transactions",
    "fast_ops_read",
    "fast_refs_resolved",
    "fast_seq_nodes_built",
    "fast_views_built",
    "fast_publications",
    "fast_pages_retained",
    "fast_pages_released",
    "fast_live_page_bytes",
    "fast_live_payload_bytes",
    "fast_status_cache_miss",
    "fast_status_invalid",
];

#[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
static VALUES: [AtomicU64; Counter::COUNT] = [const { AtomicU64::new(0) }; Counter::COUNT];

#[cfg(all(test, feature = "perf-counters"))]
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(all(test, feature = "perf-counters"))]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().expect("performance test lock poisoned")
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
    #[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
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
    #[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
    VALUES[counter.index()].fetch_add(amount, Ordering::Relaxed);
    #[cfg(any(not(feature = "perf-counters"), feature = "perf-timing"))]
    let _ = (counter, amount);
}

/// Sets a counter used as a current restart/offset gauge.
#[inline(always)]
pub fn set(counter: Counter, value: u64) {
    #[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
    VALUES[counter.index()].store(value, Ordering::Relaxed);
    #[cfg(any(not(feature = "perf-counters"), feature = "perf-timing"))]
    let _ = (counter, value);
}

/// Reads all counters atomically.
#[inline]
pub fn snapshot() -> PerfSnapshot {
    #[cfg(all(feature = "perf-counters", not(feature = "perf-timing")))]
    {
        let mut values = [0; Counter::COUNT];
        for (index, value) in VALUES.iter().enumerate() {
            values[index] = value.load(Ordering::Relaxed);
        }
        PerfSnapshot { values }
    }

    #[cfg(any(not(feature = "perf-counters"), feature = "perf-timing"))]
    {
        PerfSnapshot::default()
    }
}
