use super::NativeTuiHost;
use crate::NativeError;
use iyon_tui::{
    AnsiColor, ColorSpec, DiffHunk, DiffLine, DiffLineNumber, DiffLineOffset, DiffLineTermination,
    DiffRange, DiffRenderer, GridCellSpec, GridTrack, HorizontalAlign, Insets, IntoView, Renderer,
    RetainedPathStep, StyleRef, StyleSpec, TextAttribute, TextSpan, VerticalAlign, View, WrapMode,
};
use napi::Env;
use napi_derive::napi;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CStr;
use std::slice;
use std::str;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
use super::fast_shared;
#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
use super::{packed_v3, packed_v4};

#[path = "../generated/view_abi_types.rs"]
mod generated_types;
pub use generated_types::AxisChildInputV1;

// The generated ABI keeps the host handle opaque. It is the stable N-API
// NativeTuiHost allocation, not the movable inner TuiHost value.
pub(super) type NativeHost = NativeTuiHost;

mod generated_exports {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/view_abi_exports.rs"
    ));
}

#[path = "../generated/view_abi_table.rs"]
mod generated_table;

const ABI_MAGIC: u32 = 1_230_589_774;
const ABI_VERSION: u32 = 1;
const SEMANTIC_VERSION: u32 = 1;
const FAST_INVALID: u32 = 0x8000_0001;
const FAST_CACHE_MISS: u32 = 0x8000_0004;
const FAST_FALLBACK: u32 = 0x8000_0005;
const FAST_INTERNAL: u32 = 0x8000_0006;

// PERF-12 §74/T12: the status detail side channel uses the top two bits for
// the failure kind and the remaining bits for a child ordinal where needed.
// Child ordinals are bounded by the retained transport caps, so they fit in
// the 30-bit payload. Base-ref failures carry no payload.
const STATUS_DETAIL_CHILD_INDEX: u32 = 0x4000_0000;
const STATUS_DETAIL_BASE_REF: u32 = 0x8000_0000;

/// PERF-12 §55 maintenance tuning: bounded candidate budget processed per
/// maintenance call, and the weak-cache metadata growth that triggers the
/// threshold-backstop full sweep. Final values are benchmark decisions.
const SCAVENGE_BATCH_BUDGET: u64 = 256;
const FULL_SWEEP_METADATA_GROWTH_THRESHOLD: u64 = 4096;
const HOST_STATUS_OK: i32 = 0;
const HOST_STATUS_CACHE_MISS: i32 = 1;
const HOST_STATUS_INVALID: i32 = -1;
const HOST_STATUS_INTERNAL: i32 = -3;

const PATCH_PADDING: u32 = 4;
const PATCH_WIDTH: u32 = 8;
const PATCH_HEIGHT: u32 = 16;
const PATCH_MIN_WIDTH: u32 = 32;
const PATCH_MAX_WIDTH: u32 = 64;
const PATCH_MIN_HEIGHT: u32 = 128;
const PATCH_MAX_HEIGHT: u32 = 256;
const PATCH_MASK: u32 = PATCH_PADDING
    | PATCH_WIDTH
    | PATCH_HEIGHT
    | PATCH_MIN_WIDTH
    | PATCH_MAX_WIDTH
    | PATCH_MIN_HEIGHT
    | PATCH_MAX_HEIGHT;

// PERF-12 T10 (§36): grid track words use the schema kind codes in the low
// byte (1=content, 2=contentMax, 3=fixed, 4=flex, 5=flexMax) with the u16
// size/max at bits 8..24. Align packs hold horizontal in the low half and
// vertical in the high half; span packs hold column_span low / row_span high.
const GRID_TRACK_CONTENT_WORD: u32 = 1;
const GRID_TRACK_CONTENT_MAX_WORD: u32 = 2;
const GRID_TRACK_FIXED_WORD: u32 = 3;
const GRID_TRACK_FLEX_WORD: u32 = 4;
const GRID_TRACK_FLEX_MAX_WORD: u32 = 5;

#[repr(C)]
pub(super) struct FastStatusCell {
    pub(super) code: AtomicU32,
    pub(super) detail: AtomicU32,
}

impl FastStatusCell {
    fn new() -> Self {
        Self {
            code: AtomicU32::new(0),
            detail: AtomicU32::new(0),
        }
    }

    fn record(&self, code: u32, detail: u32) -> u32 {
        self.detail.store(detail, Ordering::Release);
        self.code.store(code, Ordering::Release);
        code
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeViewKindTag {
    View = 1,
}

struct NativeViewSlot {
    node_id: u64,
    weak: iyon_tui::WeakView,
    leased: Option<View>,
    js_lease_count: u32,
    kind: NativeViewKindTag,
}

/// Dense paged backing for NativeRef slots (PERF-12 §52–§54).
///
/// Replaces the former `HashMap<u32, NativeViewSlot>` for the hottest handle
/// lookup: page = ref >> NATIVE_REF_PAGE_BITS, offset = ref & mask, so common
/// resolution pays vector bounds + page pointer + slot index instead of a hash.
/// Refs are monotonic and never recycled inside one runtime generation (§53),
/// so pages carry no ABA generation state. A page is physical metadata only:
/// when its live count reaches zero the page allocation is dropped (§54) while
/// the outer directory keeps its high-water length; a stale JS ref into a
/// dropped page is simply a cache miss.
const NATIVE_REF_PAGE_BITS: u32 = 12;

/// Outcome of applying the shared semantic identity rules (PERF-12 §24)
/// to one candidate publication.
#[derive(Debug)]
enum SemanticIdentityMatch {
    /// A live NativeRef already maps to this exact View.
    SameLiveWithRef(View),
    /// The weak cache holds this exact live View but no live NativeRef
    /// (for example a decode-only transport saw it first).
    SameLiveWithoutRef,
    /// A different live View owns this NodeId: impossible identity conflict.
    Conflict,
    /// No live View for this NodeId; stale metadata was cleared.
    Fresh,
}

/// Lease mode requested from the central semantic publication helper
/// (PERF-12 handoff §24). `Leased` publications return a caller-owned lease
/// (generated constructors); `Weak` publications only record semantic identity
/// without keeping the View alive (bulk/decode-style transports).
pub(super) enum PublicationLease {
    Leased,
    Weak,
}

#[derive(Default)]
struct NativeRefPage {
    slots: Box<[Option<NativeViewSlot>]>,
    live: u32,
}

#[derive(Default)]
struct NativeRefTable<const PAGE_BITS: u32 = NATIVE_REF_PAGE_BITS> {
    pages: Vec<Option<Box<NativeRefPage>>>,
    len: usize,
    pages_freed: u64,
}

impl<const PAGE_BITS: u32> NativeRefTable<PAGE_BITS> {
    fn page_slot(reference: u32) -> Option<(usize, usize)> {
        let index = reference as usize;
        if index == 0 {
            return None;
        }
        Some((index >> PAGE_BITS, index & ((1 << PAGE_BITS) - 1)))
    }

    fn get(&self, reference: &u32) -> Option<&NativeViewSlot> {
        let (page, offset) = Self::page_slot(*reference)?;
        self.pages.get(page)?.as_ref()?.slots.get(offset)?.as_ref()
    }

    fn get_mut(&mut self, reference: &u32) -> Option<&mut NativeViewSlot> {
        let (page, offset) = Self::page_slot(*reference)?;
        self.pages
            .get_mut(page)?
            .as_mut()?
            .slots
            .get_mut(offset)?
            .as_mut()
    }

    fn contains_key(&self, reference: &u32) -> bool {
        self.get(reference).is_some()
    }

    fn len(&self) -> usize {
        self.len
    }

    fn insert(&mut self, reference: u32, slot: NativeViewSlot) -> Option<NativeViewSlot> {
        let (page_index, offset) = Self::page_slot(reference)?;
        if page_index >= self.pages.len() {
            self.pages.resize_with(page_index + 1, || None);
        }
        if self.pages[page_index].is_none() {
            let page_size = 1usize << PAGE_BITS;
            let mut slots: Vec<Option<NativeViewSlot>> = Vec::with_capacity(page_size);
            slots.resize_with(page_size, || None);
            self.pages[page_index] = Some(Box::new(NativeRefPage {
                slots: slots.into_boxed_slice(),
                live: 0,
            }));
        }
        let page = self.pages[page_index]
            .as_mut()
            .expect("page just allocated");
        let replaced = page.slots[offset].replace(slot);
        if replaced.is_none() {
            page.live += 1;
            self.len += 1;
        }
        replaced
    }

    fn remove(&mut self, reference: &u32) -> Option<NativeViewSlot> {
        let (page_index, offset) = Self::page_slot(*reference)?;
        let page = self.pages.get_mut(page_index)?.as_mut()?;
        let removed = page.slots[offset].take()?;
        page.live -= 1;
        self.len -= 1;
        if page.live == 0 {
            self.pages[page_index] = None;
            self.pages_freed += 1;
        }
        Some(removed)
    }

    /// Number of pages currently holding at least one live slot.
    fn pages(&self) -> usize {
        self.pages.iter().filter(|page| page.is_some()).count()
    }

    fn pages_freed(&self) -> u64 {
        self.pages_freed
    }

    fn values(&self) -> impl Iterator<Item = &NativeViewSlot> {
        self.pages
            .iter()
            .flatten()
            .flat_map(|page| page.slots.iter().flatten())
    }

    fn iter(&self) -> impl Iterator<Item = (u32, &NativeViewSlot)> {
        self.pages
            .iter()
            .enumerate()
            .flat_map(|(page_index, page)| {
                let base = (page_index << PAGE_BITS) as u32;
                page.iter()
                    .flat_map(|page| page.slots.iter().enumerate())
                    .filter_map(move |(offset, slot)| {
                        slot.as_ref().map(|slot| (base | offset as u32, slot))
                    })
            })
    }
}

// PathRefs occupy a disjoint valid-handle range so a ViewRef can never be
// accepted as a path handle (and vice versa).
const PATH_ROOT_REF: u32 = 0x4000_0001;
const BUILDER_REF_START: u32 = 0x7ffe_0001;
const BUILDER_REF_LIMIT: u32 = 0x7fff_0001;
const EDIT_TXN_REF_START: u32 = BUILDER_REF_LIMIT;
const PATH_REF_LIMIT: u32 = BUILDER_REF_START;
const EDIT_TXN_REF_LIMIT: u32 = 0x8000_0000;
const MAX_PATH_DEPTH: u32 = 128;
const MAX_EDIT_COUNT: u32 = 256;
const MAX_TXN_STAGED_OBJECTS: usize = 4_096;
const MAX_NEW_TEXT_BYTES: u32 = 16 * 1024 * 1024;
const STYLE_ATOM_REF_START: u32 = 0x6000_0001;
const STYLE_ATOM_REF_LIMIT: u32 = 0x7000_0000;
const STYLE_REF_START: u32 = 0x7000_0001;
const STYLE_REF_LIMIT: u32 = BUILDER_REF_START;
const STYLE_ATTRIBUTE_BITS: u32 = 0x3f;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PathKey {
    parent: u32,
    kind: u32,
    expected_view_kind: u32,
    selector: u32,
}

#[derive(Clone, Copy, Debug)]
struct PathNode {
    parent: u32,
    step: RetainedPathStep,
    depth: u32,
}

#[derive(Clone)]
struct TextLayoutEdit {
    path_ref: u32,
    path_depth: u32,
    // IDs are ordered from changed leaf toward the changed root. Unused
    // entries are zero and are never interpreted by the transaction.
    node_ids: [u64; 5],
    wrap: WrapMode,
    align: HorizontalAlign,
}

struct EditTrieNode {
    step: Option<RetainedPathStep>,
    node_id: Option<u64>,
    edit: Option<TextLayoutEdit>,
    children: Vec<usize>,
}

struct EditTxn {
    base_root_ref: u32,
    base_view: View,
    expected_edit_count: u32,
    staged_text_bytes: u32,
    edits: Vec<TextLayoutEdit>,
}

struct AxisBuilder {
    horizontal: bool,
    expected_children: u32,
    children: Vec<(u32, View)>,
}

struct StagedPublicationEntry {
    node_id: u64,
    view: View,
    reference: u32,
}

struct StagedPublication {
    entries: Vec<StagedPublicationEntry>,
    next_native_ref: u32,
}

#[repr(C)]
pub(super) struct NativeViewRuntime {
    pub magic: u32,
    pub abi_version: u32,
    pub semantic_version: u32,
    pub alive: AtomicU32,
    pub(super) status: FastStatusCell,
    owner_thread: ThreadId,
    // The semantic cache is deliberately owned by the environment runtime,
    // not by a transport or host. All direct, packed, FastShared, and
    // generated paths publish through this map.
    pub(super) nodes: HashMap<u64, iyon_tui::WeakView>,
    slots: NativeRefTable,
    node_refs: HashMap<u64, u32>,
    path_nodes: HashMap<u32, PathNode>,
    path_keys: HashMap<PathKey, u32>,
    builders: HashMap<u32, AxisBuilder>,
    edit_txns: HashMap<u32, EditTxn>,
    style_atoms: HashMap<u32, String>,
    styles: HashMap<u32, StyleRef>,
    next_style_atom_ref: u32,
    next_style_ref: u32,
    next_native_ref: u32,
    next_path_ref: u32,
    next_builder_ref: u32,
    next_edit_txn_ref: u32,
    stale_removals: u64,
    release_batches: u64,
    released_refs: u64,
    // PERF-12 §56 weak-cache maintenance counters. Plain u64 field increments
    // are compile-time-cheap; no scans or atomics sit on the hot path and the
    // authoritative timing build performs no extra work beyond one add.
    scavenge_queue: VecDeque<u32>,
    semantic_cache_expired_seen: u64,
    semantic_cache_full_sweeps: u64,
    semantic_cache_entries_removed: u64,
    native_ref_expired_slots_removed: u64,
    scavenge_processed: u64,
    nodes_inserted_since_full_sweep: u64,
    pub(super) generation: u32,
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    pub(super) packed_v3: packed_v3::PackedState,
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    pub(super) packed_v4: packed_v4::PackedState,
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    pub(super) fast_slots: HashMap<usize, fast_shared::FastSlotTable>,
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    pub(super) fast_sessions: HashMap<usize, usize>,
}

impl NativeViewRuntime {
    pub(super) fn new() -> Self {
        Self {
            magic: ABI_MAGIC,
            abi_version: ABI_VERSION,
            semantic_version: SEMANTIC_VERSION,
            alive: AtomicU32::new(1),
            status: FastStatusCell::new(),
            owner_thread: std::thread::current().id(),
            nodes: HashMap::new(),
            slots: NativeRefTable::default(),
            node_refs: HashMap::new(),
            path_nodes: HashMap::from([(
                PATH_ROOT_REF,
                PathNode {
                    parent: 0,
                    step: RetainedPathStep::new(0, 0, 0),
                    depth: 0,
                },
            )]),
            path_keys: HashMap::new(),
            builders: HashMap::new(),
            edit_txns: HashMap::new(),
            style_atoms: HashMap::new(),
            styles: HashMap::new(),
            next_style_atom_ref: STYLE_ATOM_REF_START,
            next_style_ref: STYLE_REF_START,
            next_native_ref: 1,
            next_path_ref: PATH_ROOT_REF + 1,
            next_builder_ref: BUILDER_REF_LIMIT - 1,
            next_edit_txn_ref: EDIT_TXN_REF_LIMIT - 1,
            stale_removals: 0,
            release_batches: 0,
            released_refs: 0,
            scavenge_queue: VecDeque::new(),
            semantic_cache_expired_seen: 0,
            semantic_cache_full_sweeps: 0,
            semantic_cache_entries_removed: 0,
            native_ref_expired_slots_removed: 0,
            scavenge_processed: 0,
            nodes_inserted_since_full_sweep: 0,
            generation: 1,
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            packed_v3: packed_v3::PackedState::new(),
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            packed_v4: packed_v4::PackedState::new(),
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            fast_slots: HashMap::new(),
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            fast_sessions: HashMap::new(),
        }
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    pub(super) fn fast_slots_for(&mut self, host_addr: usize) -> &mut fast_shared::FastSlotTable {
        self.fast_slots
            .entry(host_addr)
            .or_insert_with(fast_shared::FastSlotTable::new)
    }

    pub(super) fn valid_on_owner_thread(&self) -> bool {
        self.magic == ABI_MAGIC
            && self.abi_version == ABI_VERSION
            && self.semantic_version == SEMANTIC_VERSION
            && self.alive.load(Ordering::Acquire) != 0
            && self.owner_thread == std::thread::current().id()
    }

    pub(super) fn diagnostic_counts(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
        let leased_slots = self
            .slots
            .values()
            .filter(|slot| slot.js_lease_count > 0)
            .count();
        (
            self.nodes.len(),
            self.slots.len(),
            leased_slots,
            self.path_nodes.len(),
            self.builders.len(),
            self.edit_txns.len(),
            self.style_atoms.len(),
            self.styles.len(),
        )
    }

    fn allocate_path_ref(&mut self) -> Option<u32> {
        while self.next_path_ref < PATH_REF_LIMIT {
            let candidate = self.next_path_ref;
            self.next_path_ref = self.next_path_ref.wrapping_add(1);
            if candidate != 0 && !self.path_nodes.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn path_root(&mut self) -> u32 {
        PATH_ROOT_REF
    }

    fn path_child(
        &mut self,
        parent: u32,
        kind: u32,
        expected_view_kind: u32,
        selector: u32,
    ) -> Result<u32, u32> {
        if !is_valid_path_ref(parent) {
            return Err(FAST_INVALID);
        }
        let Some(parent_node) = self.path_nodes.get(&parent).copied() else {
            return Err(FAST_CACHE_MISS);
        };
        if !(1..=9).contains(&kind)
            || !(1..=8).contains(&expected_view_kind)
            || !path_step_matches_kind(kind, expected_view_kind)
            || parent_node.depth >= MAX_PATH_DEPTH
            || selector > 1_000_000
        {
            return Err(FAST_INVALID);
        }
        let key = PathKey {
            parent,
            kind,
            expected_view_kind,
            selector,
        };
        if let Some(reference) = self.path_keys.get(&key).copied() {
            return Ok(reference);
        }
        let reference = self.allocate_path_ref().ok_or(FAST_FALLBACK)?;
        self.path_keys.insert(key, reference);
        self.path_nodes.insert(
            reference,
            PathNode {
                parent,
                step: RetainedPathStep::new(kind, expected_view_kind, selector),
                depth: parent_node.depth + 1,
            },
        );
        Ok(reference)
    }

    fn path_steps(&self, reference: u32) -> Result<Vec<RetainedPathStep>, u32> {
        if !is_valid_path_ref(reference) {
            return Err(FAST_INVALID);
        }
        let Some(node) = self.path_nodes.get(&reference).copied() else {
            return Err(FAST_CACHE_MISS);
        };
        let mut steps = Vec::with_capacity(node.depth as usize);
        let mut current = reference;
        while current != PATH_ROOT_REF {
            let Some(node) = self.path_nodes.get(&current).copied() else {
                return Err(FAST_CACHE_MISS);
            };
            steps.push(node.step);
            current = node.parent;
        }
        steps.reverse();
        Ok(steps)
    }

    fn allocate_builder_ref(&mut self) -> Option<u32> {
        while self.next_builder_ref >= BUILDER_REF_START {
            let candidate = self.next_builder_ref;
            self.next_builder_ref = self.next_builder_ref.saturating_sub(1);
            if candidate != 0 && !self.builders.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn begin_axis_builder(&mut self, axis_kind: u32, expected_children: u32) -> Result<u32, u32> {
        if !(AXIS_KIND_ROW..=AXIS_KIND_COLUMN).contains(&axis_kind)
            || expected_children > MAX_AXIS_CHILD_COUNT
        {
            return Err(FAST_INVALID);
        }
        let reference = self.allocate_builder_ref().ok_or(FAST_FALLBACK)?;
        self.builders.insert(
            reference,
            AxisBuilder {
                horizontal: axis_kind == AXIS_KIND_ROW,
                expected_children,
                children: Vec::with_capacity(expected_children as usize),
            },
        );
        Ok(reference)
    }

    fn push_axis_builder(&mut self, builder_ref: u32, track_word: u32, child_ref: u32) -> i32 {
        if !is_valid_builder_ref(builder_ref) {
            return -1;
        }
        if track_word != 0 && !(1..=5).contains(&(track_word & 0xff)) {
            return -1;
        }
        let Some((expected, current)) = self
            .builders
            .get(&builder_ref)
            .map(|builder| (builder.expected_children, builder.children.len()))
        else {
            return 1;
        };
        if current as u32 >= expected {
            return 2;
        }
        let Ok((child, _)) = self.resolve_ref(child_ref) else {
            return 1;
        };
        let Some(builder) = self.builders.get_mut(&builder_ref) else {
            return 1;
        };
        builder.children.push((track_word, child));
        0
    }

    fn finish_axis_builder(
        &mut self,
        builder_ref: u32,
        node_id: u64,
        gap: u32,
    ) -> Result<u32, u32> {
        if !is_valid_builder_ref(builder_ref) {
            return Err(FAST_INVALID);
        }
        let Some(builder) = self.builders.remove(&builder_ref) else {
            return Err(FAST_CACHE_MISS);
        };
        if builder.children.len() as u32 != builder.expected_children {
            return Err(FAST_INVALID);
        }
        let gap = u16::try_from(gap).map_err(|_| FAST_INVALID)?;
        let view = View::native_axis_from_children(builder.horizontal, gap, builder.children)
            .map_err(|_| FAST_INVALID)?;
        self.publish(node_id, view)
    }

    fn abort_axis_builder(&mut self, builder_ref: u32) -> i32 {
        if !is_valid_builder_ref(builder_ref) {
            return -1;
        }
        if self.builders.remove(&builder_ref).is_some() {
            0
        } else {
            1
        }
    }

    fn allocate_edit_txn_ref(&mut self) -> Option<u32> {
        while self.next_edit_txn_ref >= EDIT_TXN_REF_START {
            let candidate = self.next_edit_txn_ref;
            self.next_edit_txn_ref = self.next_edit_txn_ref.saturating_sub(1);
            if candidate != 0 && !self.edit_txns.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn begin_edit_txn(&mut self, base_root_ref: u32, expected_edit_count: u32) -> Result<u32, u32> {
        if !is_valid_view_ref(base_root_ref)
            || expected_edit_count == 0
            || expected_edit_count > MAX_EDIT_COUNT
        {
            return Err(FAST_INVALID);
        }
        let Ok((base_view, _)) = self.resolve_ref(base_root_ref) else {
            return Err(FAST_CACHE_MISS);
        };
        let reference = self.allocate_edit_txn_ref().ok_or(FAST_FALLBACK)?;
        self.edit_txns.insert(
            reference,
            EditTxn {
                base_root_ref,
                base_view,
                expected_edit_count,
                staged_text_bytes: 0,
                edits: Vec::with_capacity(expected_edit_count as usize),
            },
        );
        Ok(reference)
    }

    fn add_text_layout_edit(
        &mut self,
        txn_ref: u32,
        path_ref: u32,
        path_depth: u32,
        node_ids: [u64; 5],
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> i32 {
        if !is_valid_edit_txn_ref(txn_ref) {
            return -1;
        }
        if path_depth > 4 || path_depth > MAX_PATH_DEPTH {
            return 2;
        }
        let Ok(steps) = self.path_steps(path_ref) else {
            return 1;
        };
        if steps.len() != path_depth as usize {
            return -1;
        }
        let Some(txn) = self.edit_txns.get_mut(&txn_ref) else {
            return 1;
        };
        if txn.edits.len() as u32 >= txn.expected_edit_count
            || txn.edits.len() as u32 >= MAX_EDIT_COUNT
            || txn.staged_text_bytes > MAX_NEW_TEXT_BYTES
            || (txn.edits.len() + path_depth as usize + 1) > MAX_TXN_STAGED_OBJECTS
        {
            return 2;
        }
        if txn
            .edits
            .iter()
            .any(|edit| edit.path_ref == path_ref && edit.path_depth == path_depth)
        {
            return -1;
        }
        txn.edits.push(TextLayoutEdit {
            path_ref,
            path_depth,
            node_ids,
            wrap,
            align,
        });
        0
    }

    fn build_edit_trie(&self, txn: &EditTxn) -> Result<Vec<EditTrieNode>, u32> {
        if txn.edits.is_empty() {
            return Err(FAST_INVALID);
        }
        let mut trie = vec![EditTrieNode {
            step: None,
            node_id: None,
            edit: None,
            children: Vec::new(),
        }];
        for edit in &txn.edits {
            let steps = self.path_steps(edit.path_ref)?;
            if steps.len() != edit.path_depth as usize || edit.path_depth > 4 {
                return Err(FAST_INVALID);
            }
            let root_id = edit.node_ids[edit.path_depth as usize];
            if root_id == 0 {
                return Err(FAST_INVALID);
            }
            set_trie_node_id(&mut trie[0], root_id)?;
            let mut current = 0;
            for (index, step) in steps.iter().copied().enumerate() {
                let child = trie[current]
                    .children
                    .iter()
                    .copied()
                    .find(|candidate| trie[*candidate].step == Some(step));
                let child = if let Some(child) = child {
                    child
                } else {
                    if trie.len() >= MAX_TXN_STAGED_OBJECTS {
                        return Err(FAST_FALLBACK);
                    }
                    let child = trie.len();
                    trie.push(EditTrieNode {
                        step: Some(step),
                        node_id: None,
                        edit: None,
                        children: Vec::new(),
                    });
                    trie[current].children.push(child);
                    child
                };
                let node_id = edit.node_ids[edit.path_depth as usize - index - 1];
                if node_id == 0 {
                    return Err(FAST_INVALID);
                }
                set_trie_node_id(&mut trie[child], node_id)?;
                current = child;
            }
            if trie[current].edit.is_some() || !trie[current].children.is_empty() {
                return Err(FAST_INVALID);
            }
            trie[current].edit = Some(edit.clone());
        }
        Ok(trie)
    }

    fn stage_edit_trie(
        &self,
        view: View,
        trie: &[EditTrieNode],
        index: usize,
        staged: &mut Vec<(u64, View)>,
    ) -> Result<View, u32> {
        let node = &trie[index];
        if let Some(edit) = node.edit.as_ref() {
            if !node.children.is_empty() || node.node_id.is_none() {
                return Err(FAST_INVALID);
            }
            let patched = view
                .try_with_text_layout_patch(Some(edit.wrap), Some(edit.align))
                .map_err(|_| FAST_INVALID)?;
            staged.push((node.node_id.unwrap(), patched.clone()));
            return Ok(patched);
        }
        if node.children.is_empty() || node.node_id.is_none() {
            return Err(FAST_INVALID);
        }
        let mut patched = view.clone();
        for &child_index in &node.children {
            let step = trie[child_index].step.ok_or(FAST_INVALID)?;
            let child = view.try_retained_child(step).map_err(|_| FAST_INVALID)?;
            let rebuilt = self.stage_edit_trie(child, trie, child_index, staged)?;
            patched = patched
                .try_replace_retained_child(step, rebuilt)
                .map_err(|_| FAST_INVALID)?;
        }
        staged.push((node.node_id.unwrap(), patched.clone()));
        Ok(patched)
    }

    /// Validates all logical publication failures and reserves the NativeRefs
    /// without exposing them. The returned plan is committed only after the
    /// host accepts the new root, so a host error cannot leave published refs
    /// for an uninstalled View.
    fn prepare_staged_publication(
        &mut self,
        staged: Vec<(u64, View)>,
    ) -> Result<StagedPublication, u32> {
        let mut unique = HashSet::with_capacity(staged.len());
        let mut planned_refs = HashSet::with_capacity(staged.len());
        let mut next_native_ref = self.next_native_ref;
        let mut entries = Vec::with_capacity(staged.len());

        for (node_id, view) in staged {
            if node_id == 0 || !unique.insert(node_id) {
                return Err(FAST_INVALID);
            }
            if let Some(existing) = self
                .nodes
                .get(&node_id)
                .and_then(iyon_tui::WeakView::upgrade)
                && existing != view
            {
                return Err(FAST_INVALID);
            }

            let reference = if let Some(reference) = self.node_refs.get(&node_id).copied() {
                match self.resolve_ref(reference) {
                    Ok((existing, _)) if existing != view => return Err(FAST_INVALID),
                    Ok(_) => reference,
                    Err(FAST_CACHE_MISS) => {
                        self.node_refs.remove(&node_id);
                        reserve_staged_ref(&self.slots, &mut planned_refs, &mut next_native_ref)?
                    }
                    Err(error) => return Err(error),
                }
            } else {
                reserve_staged_ref(&self.slots, &mut planned_refs, &mut next_native_ref)?
            };
            entries.push(StagedPublicationEntry {
                node_id,
                view,
                reference,
            });
        }

        if entries.is_empty() {
            return Err(FAST_INVALID);
        }
        Ok(StagedPublication {
            entries,
            next_native_ref,
        })
    }

    /// Commits a previously prepared plan. All semantic error conditions were
    /// checked before host installation; this phase only installs the plan's
    /// already-reserved entries and cannot return a recoverable ABI status.
    fn commit_staged_publication(&mut self, publication: StagedPublication) -> u32 {
        let root_ref = publication
            .entries
            .last()
            .map(|entry| entry.reference)
            .unwrap_or(0);
        let last_index = publication.entries.len().saturating_sub(1);
        self.next_native_ref = publication.next_native_ref;
        for (index, entry) in publication.entries.into_iter().enumerate() {
            let is_root = index == last_index;
            if self.node_refs.get(&entry.node_id) == Some(&entry.reference) {
                if is_root {
                    let _ = self.ensure_lease(entry.reference, entry.view);
                }
                continue;
            }
            self.install_semantic_view(entry.node_id, entry.view, entry.reference, is_root);
        }
        root_ref
    }

    fn allocate_style_atom_ref(&mut self) -> Option<u32> {
        while self.next_style_atom_ref < STYLE_ATOM_REF_LIMIT {
            let candidate = self.next_style_atom_ref;
            self.next_style_atom_ref = self.next_style_atom_ref.wrapping_add(1);
            if !self.style_atoms.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn style_atom(&mut self, value: &str) -> Result<u32, u32> {
        if value.is_empty() || value.len() > 4096 {
            return Err(FAST_INVALID);
        }
        if let Some((reference, _)) = self
            .style_atoms
            .iter()
            .find(|(_, candidate)| candidate.as_str() == value)
        {
            return Ok(*reference);
        }
        let reference = self.allocate_style_atom_ref().ok_or(FAST_FALLBACK)?;
        self.style_atoms.insert(reference, value.to_owned());
        Ok(reference)
    }

    fn allocate_style_ref(&mut self) -> Option<u32> {
        while self.next_style_ref < STYLE_REF_LIMIT {
            let candidate = self.next_style_ref;
            self.next_style_ref = self.next_style_ref.wrapping_add(1);
            if !self.styles.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn style(&mut self, style: StyleRef) -> Result<u32, u32> {
        if let Some((reference, _)) = self
            .styles
            .iter()
            .find(|(_, candidate)| **candidate == style)
        {
            return Ok(*reference);
        }
        let reference = self.allocate_style_ref().ok_or(FAST_FALLBACK)?;
        self.styles.insert(reference, style);
        Ok(reference)
    }

    fn style_for_ref(&self, reference: u32) -> Result<StyleRef, u32> {
        if reference == 0 {
            return Ok(StyleRef::default());
        }
        if !(STYLE_REF_START..STYLE_REF_LIMIT).contains(&reference) {
            return Err(FAST_INVALID);
        }
        self.styles.get(&reference).cloned().ok_or(FAST_CACHE_MISS)
    }

    fn style_atom_value(&self, reference: u32) -> Result<&str, u32> {
        if !(STYLE_ATOM_REF_START..STYLE_ATOM_REF_LIMIT).contains(&reference) {
            return Err(FAST_INVALID);
        }
        self.style_atoms
            .get(&reference)
            .map(String::as_str)
            .ok_or(FAST_CACHE_MISS)
    }

    fn allocate_ref(&mut self) -> Option<u32> {
        while self.next_native_ref < PATH_ROOT_REF {
            let candidate = self.next_native_ref;
            self.next_native_ref = self.next_native_ref.wrapping_add(1);
            if candidate != 0 && !self.slots.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn ensure_lease(&mut self, reference: u32, view: View) -> Result<(), u32> {
        let Some(slot) = self.slots.get_mut(&reference) else {
            return Err(FAST_CACHE_MISS);
        };
        if slot.kind != NativeViewKindTag::View {
            return Err(FAST_INVALID);
        }
        if slot.js_lease_count == 0 {
            slot.leased = Some(view);
            slot.js_lease_count = 1;
        } else if slot.leased.is_none() {
            slot.leased = Some(view);
        }
        Ok(())
    }

    fn acquire_lease(&mut self, reference: u32, view: View) -> Result<(), u32> {
        let Some(slot) = self.slots.get_mut(&reference) else {
            return Err(FAST_CACHE_MISS);
        };
        if slot.kind != NativeViewKindTag::View {
            return Err(FAST_INVALID);
        }
        let Some(count) = slot.js_lease_count.checked_add(1) else {
            return Err(FAST_FALLBACK);
        };
        if slot.leased.is_none() {
            slot.leased = Some(view);
        }
        slot.js_lease_count = count;
        Ok(())
    }

    fn resolve_ref(&mut self, reference: u32) -> Result<(View, bool), u32> {
        let Some(slot) = self.slots.get_mut(&reference) else {
            return Err(FAST_CACHE_MISS);
        };
        if slot.kind != NativeViewKindTag::View {
            return Err(FAST_INVALID);
        }
        if let Some(view) = slot.leased.clone() {
            return Ok((view, true));
        }
        let Some(view) = slot.weak.upgrade() else {
            let node_id = slot.node_id;
            self.node_refs.remove(&node_id);
            self.slots.remove(&reference);
            self.native_ref_expired_slots_removed += 1;
            self.semantic_cache_expired_seen += 1;
            return Err(FAST_CACHE_MISS);
        };
        Ok((view, false))
    }

    fn consult_semantic_identity(
        &mut self,
        node_id: u64,
        view: &View,
    ) -> Result<SemanticIdentityMatch, u32> {
        if let Some(reference) = self.node_refs.get(&node_id).copied() {
            match self.resolve_ref(reference) {
                Ok((existing, _)) => {
                    return Ok(if existing == *view {
                        SemanticIdentityMatch::SameLiveWithRef(existing)
                    } else {
                        SemanticIdentityMatch::Conflict
                    });
                }
                Err(FAST_CACHE_MISS) => {
                    self.node_refs.remove(&node_id);
                    self.semantic_cache_expired_seen += 1;
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(existing) = self
            .nodes
            .get(&node_id)
            .and_then(iyon_tui::WeakView::upgrade)
        {
            if existing == *view {
                return Ok(SemanticIdentityMatch::SameLiveWithoutRef);
            }
            return Ok(SemanticIdentityMatch::Conflict);
        }
        // Clear the expired weak entry so post-maintenance metadata stays
        // proportional to live semantic state; the insert below would only
        // overwrite it anyway.
        if self.nodes.remove(&node_id).is_some() {
            self.semantic_cache_expired_seen += 1;
            self.semantic_cache_entries_removed += 1;
        }
        Ok(SemanticIdentityMatch::Fresh)
    }

    /// Single installation path for a freshly allocated NativeRef. Shared by
    /// direct publication and staged (host-atomic) publication commits.
    fn install_semantic_view(&mut self, node_id: u64, view: View, reference: u32, leased: bool) {
        self.nodes_inserted_since_full_sweep += 1;
        let weak = view.downgrade();
        self.nodes.insert(node_id, weak.clone());
        self.node_refs.insert(node_id, reference);
        self.slots.insert(
            reference,
            NativeViewSlot {
                node_id,
                weak,
                leased: leased.then_some(view),
                js_lease_count: u32::from(leased),
                kind: NativeViewKindTag::View,
            },
        );
    }

    /// The central semantic publication helper (PERF-12 handoff §24). Every
    /// transport that mints or re-associates a NativeRef for a semantic NodeId
    /// must route through this function so there is exactly one set of
    /// identity rules: validate NodeId, reject impossible identity conflicts,
    /// maintain NodeId -> WeakView, allocate/associate the NativeRef, apply
    /// the requested lease mode, and keep diagnostics on the shared runtime.
    fn publish_semantic_view(
        &mut self,
        node_id: u64,
        view: View,
        lease: PublicationLease,
    ) -> Result<u32, u32> {
        if node_id == 0 {
            return Err(FAST_INVALID);
        }
        match self.consult_semantic_identity(node_id, &view)? {
            SemanticIdentityMatch::Conflict => return Err(FAST_INVALID),
            SemanticIdentityMatch::SameLiveWithRef(existing) => {
                let reference = self.node_refs.get(&node_id).copied().ok_or(FAST_INTERNAL)?;
                match lease {
                    PublicationLease::Leased => {
                        // Every generated constructor returns a caller-owned
                        // lease, including re-materialization of an existing
                        // semantic NodeId. Do not collapse that lease with an
                        // already-live owner.
                        self.acquire_lease(reference, existing)?;
                    }
                    PublicationLease::Weak => {}
                }
                return Ok(reference);
            }
            SemanticIdentityMatch::SameLiveWithoutRef | SemanticIdentityMatch::Fresh => {}
        }
        let reference = self.allocate_ref().ok_or(FAST_FALLBACK)?;
        self.install_semantic_view(
            node_id,
            view,
            reference,
            matches!(lease, PublicationLease::Leased),
        );
        Ok(reference)
    }

    fn publish(&mut self, node_id: u64, view: View) -> Result<u32, u32> {
        self.publish_semantic_view(node_id, view, PublicationLease::Leased)
    }

    // Bulk V2/V3/V4 and FastShared definitions do not represent a live JS
    // backing, so they receive a weak-only lease. The generated path can
    // reacquire the same NativeRef later through the semantic NodeId cache.
    pub(super) fn publish_bulk(&mut self, node_id: u64, view: View) -> Result<u32, u32> {
        self.publish_semantic_view(node_id, view, PublicationLease::Weak)
    }

    fn ref_for_node_id(&mut self, node_id: u64) -> Result<u32, u32> {
        if node_id == 0 {
            return Err(FAST_INVALID);
        }
        if let Some(reference) = self.node_refs.get(&node_id).copied() {
            match self.resolve_ref(reference) {
                Ok((view, has_lease)) => {
                    if has_lease {
                        self.acquire_lease(reference, view)?;
                    } else {
                        self.ensure_lease(reference, view)?;
                    }
                    return Ok(reference);
                }
                Err(_) => {
                    self.node_refs.remove(&node_id);
                }
            }
        }
        let Some(weak) = self.nodes.get(&node_id).cloned() else {
            return Err(FAST_CACHE_MISS);
        };
        let Some(view) = weak.upgrade() else {
            self.nodes.remove(&node_id);
            return Err(FAST_CACHE_MISS);
        };
        self.publish(node_id, view)
    }

    fn abort_all_edit_txns(&mut self) {
        self.edit_txns.clear();
        self.builders.clear();
    }

    /// Live View for a NodeId from the shared semantic cache, if any.
    pub(super) fn live_cached_view(&self, node_id: u64) -> Option<View> {
        self.nodes
            .get(&node_id)
            .and_then(iyon_tui::WeakView::upgrade)
    }

    /// Drops the cached weak entry for a NodeId. Decode-style transports call
    /// this on a confirmed cache miss before re-decoding; at that point any
    /// remaining entry is expired by definition.
    pub(super) fn drop_cached_entry(&mut self, node_id: u64) {
        self.nodes.remove(&node_id);
    }

    /// Shared insertion rules for decode-style transports (Direct N-API
    /// decoder, packed decoder): same identity rules as publication without
    /// minting a NativeRef. An identical live View deduplicates, a conflicting
    /// live View is rejected as an impossible semantic identity, and expired
    /// entries are replaced. Applies the shared size-based retain cleanup so
    /// every transport benefits from one metadata-bounding rule.
    pub(super) fn record_decoded_semantic_view(
        &mut self,
        node_id: u64,
        view: &View,
    ) -> Result<(), u32> {
        if let Some(existing) = self
            .nodes
            .get(&node_id)
            .and_then(iyon_tui::WeakView::upgrade)
            && existing != *view
        {
            return Err(FAST_INVALID);
        }
        self.nodes.insert(node_id, view.downgrade());
        if self.nodes.len() > 4096 && self.nodes.len() % 256 == 0 {
            self.nodes.retain(|_, weak| weak.upgrade().is_some());
        }
        Ok(())
    }

    /// PERF-12 §55 periodic maintenance: process a bounded budget of
    /// scavenging candidates (zero-lease refs whose View has since expired),
    /// then apply the threshold backstop: once weak-cache metadata growth
    /// since the last full sweep exceeds the threshold, run one full
    /// expired-weak sweep. Amortized cost is O(1) per released ref with
    /// bounded slack, keeping post-maintenance metadata at
    /// O(live semantic state + bounded sweep slack).
    fn maintain_bounded(&mut self) {
        let mut processed = 0u64;
        while processed < SCAVENGE_BATCH_BUDGET {
            let Some(reference) = self.scavenge_queue.pop_front() else {
                break;
            };
            processed += 1;
            let expired = self
                .slots
                .get(&reference)
                .is_some_and(|slot| slot.js_lease_count == 0 && slot.weak.upgrade().is_none());
            if !expired {
                continue;
            }
            if let Some(slot) = self.slots.remove(&reference) {
                self.native_ref_expired_slots_removed += 1;
                if self.node_refs.get(&slot.node_id) == Some(&reference) {
                    self.node_refs.remove(&slot.node_id);
                }
            }
        }
        self.scavenge_processed += processed;
        if self.nodes_inserted_since_full_sweep >= FULL_SWEEP_METADATA_GROWTH_THRESHOLD {
            self.prune_expired();
        }
    }

    /// Explicit maintenance entrypoint for tests/benchmarks (§88). `full=true`
    /// performs the complete expired-weak sweep; otherwise only the bounded
    /// candidate budget is processed.
    pub(super) fn maintain(&mut self, full: bool) {
        if full {
            self.prune_expired();
        } else {
            self.maintain_bounded();
        }
    }

    /// Full expired-weak sweep (§55 threshold backstop and the explicit
    /// maintenance hook). Post-sweep weak/slot metadata is O(live + slack).
    pub(super) fn prune_expired(&mut self) {
        self.semantic_cache_full_sweeps += 1;
        self.nodes_inserted_since_full_sweep = 0;
        let expired_nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, weak)| (weak.upgrade().is_none()).then_some(*node_id))
            .collect::<Vec<_>>();
        for node_id in expired_nodes {
            self.nodes.remove(&node_id);
            self.semantic_cache_entries_removed += 1;
            self.stale_removals = self.stale_removals.saturating_add(1);
            if let Some(reference) = self.node_refs.remove(&node_id) {
                let unleased = self
                    .slots
                    .get(&reference)
                    .is_some_and(|slot| slot.js_lease_count == 0);
                if unleased && self.slots.remove(&reference).is_some() {
                    self.native_ref_expired_slots_removed += 1;
                    self.stale_removals = self.stale_removals.saturating_add(1);
                }
            }
        }
        let expired_refs = self
            .slots
            .iter()
            .filter_map(|(reference, slot)| {
                (slot.js_lease_count == 0 && slot.weak.upgrade().is_none()).then_some(reference)
            })
            .collect::<Vec<_>>();
        for reference in expired_refs {
            if let Some(slot) = self.slots.remove(&reference) {
                self.native_ref_expired_slots_removed += 1;
                self.stale_removals = self.stale_removals.saturating_add(1);
                if self.node_refs.get(&slot.node_id) == Some(&reference) {
                    self.node_refs.remove(&slot.node_id);
                }
            }
        }
        // Expired entries discovered by the sweep are not re-scanned by the
        // candidate queue; drop any queued duplicates lazily via the normal
        // bounded path (they resolve to absent slots and cost nothing).
    }

    fn release_many(&mut self, refs: *const u32, used_count: u32) -> Result<i32, i32> {
        self.release_batches = self.release_batches.saturating_add(1);
        self.released_refs = self.released_refs.saturating_add(u64::from(used_count));
        // Process candidates from earlier batches first: a View that expired
        // after its release is reclaimed here without waiting for a lookup.
        self.maintain_bounded();
        for index in 0..used_count as usize {
            let reference = unsafe { refs.add(index).read() };
            let remove_slot = self
                .slots
                .get_mut(&reference)
                .map(|slot| {
                    slot.js_lease_count = slot.js_lease_count.saturating_sub(1);
                    if slot.js_lease_count == 0 {
                        slot.leased = None;
                    }
                    slot.js_lease_count == 0 && slot.weak.upgrade().is_none()
                })
                .unwrap_or(false);
            if remove_slot {
                if let Some(slot) = self.slots.remove(&reference) {
                    self.native_ref_expired_slots_removed += 1;
                    if self.node_refs.get(&slot.node_id) == Some(&reference) {
                        self.node_refs.remove(&slot.node_id);
                    }
                }
            } else if self
                .slots
                .get(&reference)
                .is_some_and(|slot| slot.js_lease_count == 0)
            {
                // §55 release path: enqueue the zero-lease ref as a scavenging
                // candidate so later weak expiry is reclaimed without waiting
                // for a lookup on the same ref. Candidates are processed by
                // the next maintenance pass, not this one.
                self.scavenge_queue.push_back(reference);
            }
        }
        Ok(used_count as i32)
    }
}

pub(super) type ViewRuntimeHandle = Arc<NativeViewRuntime>;

static RUNTIME_HANDLES: OnceLock<Mutex<HashMap<usize, ViewRuntimeHandle>>> = OnceLock::new();

fn runtime_handles() -> &'static Mutex<HashMap<usize, ViewRuntimeHandle>> {
    RUNTIME_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn runtime_handle_for_env(env: &Env) -> napi::Result<ViewRuntimeHandle> {
    let env_key = env.raw() as usize;
    let mut handles = runtime_handles()
        .lock()
        .map_err(|_| NativeError::internal("native View ABI runtime registry is poisoned"))?;
    if let Some(runtime) = handles.get(&env_key) {
        return Ok(Arc::clone(runtime));
    }
    let runtime = Arc::new(NativeViewRuntime::new());
    let cleanup_key = env_key;
    let cleanup_runtime = Arc::clone(&runtime);
    env.add_env_cleanup_hook(cleanup_key, move |_| {
        cleanup_runtime.alive.store(0, Ordering::Release);
        if let Some(registry) = RUNTIME_HANDLES.get()
            && let Ok(mut handles) = registry.lock()
        {
            handles.remove(&cleanup_key);
        }
    })?;
    handles.insert(env_key, Arc::clone(&runtime));
    Ok(runtime)
}

pub(super) fn runtime_ptr_for_env(env: &Env) -> napi::Result<*mut NativeViewRuntime> {
    let runtime = runtime_handle_for_env(env)?;
    Ok(Arc::as_ptr(&runtime) as *mut NativeViewRuntime)
}

pub(super) fn runtime_environment_count() -> i64 {
    RUNTIME_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .map(|handles| handles.len() as i64)
        .unwrap_or(0)
}

pub(super) fn runtime_is_registered(pointer: usize) -> bool {
    RUNTIME_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .is_some_and(|handles| {
            handles
                .values()
                .any(|runtime| Arc::as_ptr(runtime) as usize == pointer)
        })
}

pub(super) fn runtime_from_handle(
    handle: &ViewRuntimeHandle,
) -> napi::Result<&'static mut NativeViewRuntime> {
    let runtime = unsafe { (Arc::as_ptr(handle) as *mut NativeViewRuntime).as_mut() }
        .ok_or_else(|| NativeError::internal("native View runtime pointer is null"))?;
    if !runtime.valid_on_owner_thread() {
        return Err(NativeError::coded(
            napi::Status::Closing,
            "ION_VIEW_RUNTIME_INVALID",
            "native View runtime is disposed or called from the wrong thread",
        ));
    }
    Ok(runtime)
}

pub(super) fn publish_decoded_view(
    handle: &ViewRuntimeHandle,
    node_id: u64,
    view: View,
) -> napi::Result<u32> {
    let runtime = runtime_from_handle(handle)?;
    runtime.publish(node_id, view).map_err(|status| {
        NativeError::invalid_input(format!(
            "decoded View publication failed with status 0x{status:x}"
        ))
    })
}

pub(super) fn runtime_for_env(env: &Env) -> napi::Result<*mut NativeViewRuntime> {
    runtime_ptr_for_env(env)
}

/// Host disposal must discard any transaction or builder that could otherwise
/// retain strong staged Views until environment teardown. These handles are
/// runtime-scoped (the ABI begin calls intentionally have no host argument), so
/// clearing the uncommitted sets is the conservative lifecycle boundary.
pub(super) fn abort_all_edit_txns(pointer: *mut NativeViewRuntime) {
    let Ok(runtime) = runtime_mut(pointer) else {
        return;
    };
    runtime.abort_all_edit_txns();
}

#[napi(js_name = "tuiViewAbiBootstrap")]
pub fn bootstrap(env: Env, prune_expired: Option<bool>) -> napi::Result<Value> {
    let runtime = runtime_for_env(&env)?;
    if prune_expired.unwrap_or(false) {
        unsafe { &mut *runtime }.prune_expired();
    }
    let diagnostics = unsafe { &*runtime }.diagnostic_counts();
    let runtime_state = unsafe { &*runtime };
    let live_weak_upgrades = prune_expired
        .unwrap_or(false)
        .then(|| {
            runtime_state
                .nodes
                .values()
                .filter(|weak| weak.upgrade().is_some())
                .count()
        })
        .unwrap_or(0);
    let diagnostics = serde_json::json!({
        "semantic_cache_entries": diagnostics.0,
        "native_ref_slots": diagnostics.1,
        "leased_slots": diagnostics.2,
        "path_nodes": diagnostics.3,
        "builders": diagnostics.4,
        "edit_transactions": diagnostics.5,
        "style_atoms": diagnostics.6,
        "styles": diagnostics.7,
        "fast_slot_tables": 0,
        "fast_slots": 0,
        "stale_removals": runtime_state.stale_removals,
        "release_batches": runtime_state.release_batches,
        "released_refs": runtime_state.released_refs,
        "live_weak_upgrades": live_weak_upgrades,
        // PERF-12 §56 weak-cache maintenance counters.
        "semantic_cache_expired_seen": runtime_state.semantic_cache_expired_seen,
        "semantic_cache_full_sweeps": runtime_state.semantic_cache_full_sweeps,
        "semantic_cache_entries_removed": runtime_state.semantic_cache_entries_removed,
        "native_ref_unleased_live_slots": diagnostics.1.saturating_sub(diagnostics.2),
        "native_ref_expired_slots_removed": runtime_state.native_ref_expired_slots_removed,
        "native_ref_pages": runtime_state.slots.pages(),
        "native_ref_pages_freed": runtime_state.slots.pages_freed(),
        "node_ref_map_entries": runtime_state.node_refs.len(),
        "scavenge_queue_len": runtime_state.scavenge_queue.len(),
        "scavenge_processed": runtime_state.scavenge_processed,
        "nodes_inserted_since_full_sweep": runtime_state.nodes_inserted_since_full_sweep,
        "generation": runtime_state.generation,
        "alive": runtime_state.alive.load(Ordering::Acquire) != 0,
    });
    Ok(serde_json::json!({
        "runtime_ptr": runtime as usize as u64,
        "abi_name": generated_types::ABI_NAME,
        "diagnostics": diagnostics,
        "abi_version": generated_types::ABI_VERSION,
        "semantic_version": generated_types::SEMANTIC_SCHEMA_VERSION,
        "schema_blake3": generated_types::SCHEMA_BLAKE3,
        "generator_blake3": generated_types::GENERATOR_BLAKE3,
        "generation": unsafe { (*runtime).generation },
        "fast_view_abi": cfg!(feature = "fast-view-abi"),
        "function_count": generated_table::FUNCTION_COUNT,
        "functions": {
            "runtimeNoop": generated_exports::iyon_runtime_noop_v1 as *const () as usize as u64,
            "viewStatusDetail": generated_exports::iyon_view_status_detail_v1 as *const () as usize as u64,
            "viewRenderRef": generated_exports::iyon_view_render_ref_v1 as *const () as usize as u64,
            "hostRenderRef": generated_exports::iyon_host_render_ref_v1 as *const () as usize as u64,
            "viewSpacerCreate": generated_exports::iyon_view_spacer_create_v1 as *const () as usize as u64,
            "viewTextLayoutPatchRoot": generated_exports::iyon_view_text_layout_patch_root_v1 as *const () as usize as u64,
            "viewCommonPatchRoot": generated_exports::iyon_view_common_patch_root_v1 as *const () as usize as u64,
            "viewAxisCreateBuffer": generated_exports::iyon_view_axis_create_buffer_v1 as *const () as usize as u64,
            "viewRowCreate0": generated_exports::iyon_view_row_create_0_v1 as *const () as usize as u64,
            "viewRowCreate1": generated_exports::iyon_view_row_create_1_v1 as *const () as usize as u64,
            "viewRowCreate2": generated_exports::iyon_view_row_create_2_v1 as *const () as usize as u64,
            "viewRowCreate3": generated_exports::iyon_view_row_create_3_v1 as *const () as usize as u64,
            "viewRowCreate4": generated_exports::iyon_view_row_create_4_v1 as *const () as usize as u64,
            "viewColumnCreate0": generated_exports::iyon_view_column_create_0_v1 as *const () as usize as u64,
            "viewColumnCreate1": generated_exports::iyon_view_column_create_1_v1 as *const () as usize as u64,
            "viewColumnCreate2": generated_exports::iyon_view_column_create_2_v1 as *const () as usize as u64,
            "viewColumnCreate3": generated_exports::iyon_view_column_create_3_v1 as *const () as usize as u64,
            "viewColumnCreate4": generated_exports::iyon_view_column_create_4_v1 as *const () as usize as u64,
            "axisBuilderBegin": generated_exports::iyon_axis_builder_begin_v1 as *const () as usize as u64,
            "axisBuilderPush": generated_exports::iyon_axis_builder_push_v1 as *const () as usize as u64,
            "axisBuilderFinish": generated_exports::iyon_axis_builder_finish_v1 as *const () as usize as u64,
            "axisBuilderAbort": generated_exports::iyon_axis_builder_abort_v1 as *const () as usize as u64,
            "viewAxisSetChild": generated_exports::iyon_view_axis_set_child_v1 as *const () as usize as u64,
            "viewAxisSpliceBuffer": generated_exports::iyon_view_axis_splice_buffer_v1 as *const () as usize as u64,
            "viewGridSetCell": generated_exports::iyon_view_grid_set_cell_v1 as *const () as usize as u64,
            "viewGridCreateBuffer": generated_exports::iyon_view_grid_create_buffer_v1 as *const () as usize as u64,
            "viewDiffCreateBuffer": generated_exports::iyon_view_diff_create_buffer_v1 as *const () as usize as u64,
            "viewAxisSetChildPath": generated_exports::iyon_view_axis_set_child_path_v1 as *const () as usize as u64,
            "viewGridSetCellPath": generated_exports::iyon_view_grid_set_cell_path_v1 as *const () as usize as u64,
            "viewReleaseMany": generated_exports::iyon_view_release_many_v1 as *const () as usize as u64,
            "viewRefForNodeId": generated_exports::iyon_view_ref_for_node_id_v1 as *const () as usize as u64,
            "pathRoot": generated_exports::iyon_path_root_v1 as *const () as usize as u64,
            "pathChild": generated_exports::iyon_path_child_v1 as *const () as usize as u64,
            "viewTextLayoutPatchPath": generated_exports::iyon_view_text_layout_patch_path_v1 as *const () as usize as u64,
            "viewTextLayoutPatchPathD1": generated_exports::iyon_view_text_layout_patch_path_d1_v1 as *const () as usize as u64,
            "viewTextLayoutPatchPathD2": generated_exports::iyon_view_text_layout_patch_path_d2_v1 as *const () as usize as u64,
            "viewTextLayoutPatchPathD3": generated_exports::iyon_view_text_layout_patch_path_d3_v1 as *const () as usize as u64,
            "viewTextLayoutPatchPathD4": generated_exports::iyon_view_text_layout_patch_path_d4_v1 as *const () as usize as u64,
            "editTxnBegin": generated_exports::iyon_edit_txn_begin_v1 as *const () as usize as u64,
            "editTxnAddTextLayout": generated_exports::iyon_edit_txn_add_text_layout_v1 as *const () as usize as u64,
            "editTxnCommitRender": generated_exports::iyon_edit_txn_commit_render_v1 as *const () as usize as u64,
            "editTxnAbort": generated_exports::iyon_edit_txn_abort_v1 as *const () as usize as u64,
            "styleAtomCreateCstring": generated_exports::iyon_style_atom_create_cstring_v1 as *const () as usize as u64,
            "styleCreateBits": generated_exports::iyon_style_create_bits_v1 as *const () as usize as u64,
            "viewTextCreateCstring": generated_exports::iyon_view_text_create_cstring_v1 as *const () as usize as u64,
            "viewTextCreateUtf8": generated_exports::iyon_view_text_create_utf8_v1 as *const () as usize as u64,
            "viewTextCreateUtf82": generated_exports::iyon_view_text_create_utf8_2_v1 as *const () as usize as u64,
            "viewTextCreateUtf83": generated_exports::iyon_view_text_create_utf8_3_v1 as *const () as usize as u64,
            "viewTextCreateUtf84": generated_exports::iyon_view_text_create_utf8_4_v1 as *const () as usize as u64,
            "viewTextCreateCstring2": generated_exports::iyon_view_text_create_cstring_2_v1 as *const () as usize as u64,
            "viewTextCreateCstring3": generated_exports::iyon_view_text_create_cstring_3_v1 as *const () as usize as u64,
            "viewTextCreateCstring4": generated_exports::iyon_view_text_create_cstring_4_v1 as *const () as usize as u64,
        },
    }))
}

/// PERF-12 §88/§89 explicit maintenance hook for tests and benchmarks.
/// `full = true` performs the complete expired-weak sweep (threshold-backstop
/// equivalent); otherwise only the bounded scavenge-candidate budget runs.
#[napi(js_name = "tuiViewAbiMaintain")]
pub fn tui_view_abi_maintain(env: Env, full: Option<bool>) -> napi::Result<Value> {
    let runtime = runtime_for_env(&env)?;
    unsafe { &mut *runtime }.maintain(full.unwrap_or(false));
    let state = unsafe { &*runtime };
    Ok(serde_json::json!({
        "full": full.unwrap_or(false),
        "semantic_cache_entries": state.nodes.len(),
        "native_ref_slots": state.slots.len(),
        "scavenge_queue_len": state.scavenge_queue.len(),
        "scavenge_processed": state.scavenge_processed,
        "semantic_cache_full_sweeps": state.semantic_cache_full_sweeps,
    }))
}

/// PERF-12 §89 memory diagnostic snapshot. Expensive live-count scans run only
/// when `count_live` is requested; timing samples must call this with
/// `count_live = false`.
#[napi(js_name = "tuiViewRuntimeMemorySnapshot")]
pub fn tui_view_runtime_memory_snapshot(env: Env, count_live: Option<bool>) -> napi::Result<Value> {
    let runtime = runtime_for_env(&env)?;
    let state = unsafe { &*runtime };
    let count_live = count_live.unwrap_or(false);
    let semantic_cache_live = if count_live {
        state
            .nodes
            .values()
            .filter(|weak| weak.upgrade().is_some())
            .count()
    } else {
        0
    };
    let leased_slots = if count_live {
        state
            .slots
            .values()
            .filter(|slot| slot.js_lease_count > 0)
            .count()
    } else {
        0
    };
    let unleased_live_slots = if count_live {
        state
            .slots
            .values()
            .filter(|slot| slot.js_lease_count == 0 && slot.weak.upgrade().is_some())
            .count()
    } else {
        0
    };
    Ok(serde_json::json!({
        "semantic_cache_entries": state.nodes.len(),
        "semantic_cache_live": semantic_cache_live,
        "native_ref_slots": state.slots.len(),
        "native_ref_pages": state.slots.pages(),
        "native_ref_pages_freed": state.slots.pages_freed(),
        "leased_slots": leased_slots,
        "unleased_live_slots": unleased_live_slots,
        "node_ref_entries": state.node_refs.len(),
        "path_nodes": state.path_nodes.len(),
        "path_keys": state.path_keys.len(),
        "builders": state.builders.len(),
        "edit_txns": state.edit_txns.len(),
        "style_refs": state.styles.len(),
        // Retained text/style payload byte accounting is not tracked by the
        // current runtime; reported as null rather than a misleading zero.
        "string_bytes": null,
        "scavenge_queue": state.scavenge_queue.len(),
        "scavenge_processed": state.scavenge_processed,
        "semantic_cache_expired_seen": state.semantic_cache_expired_seen,
        "semantic_cache_full_sweeps": state.semantic_cache_full_sweeps,
        "semantic_cache_entries_removed": state.semantic_cache_entries_removed,
        "native_ref_expired_slots_removed": state.native_ref_expired_slots_removed,
        "nodes_inserted_since_full_sweep": state.nodes_inserted_since_full_sweep,
        "generation": state.generation,
        "alive": state.alive.load(Ordering::Acquire) != 0,
    }))
}

fn runtime_mut(pointer: *mut NativeViewRuntime) -> Result<&'static mut NativeViewRuntime, u32> {
    let runtime = unsafe { pointer.as_mut() }.ok_or(FAST_INVALID)?;
    if !runtime.valid_on_owner_thread() {
        return Err(FAST_INVALID);
    }
    Ok(runtime)
}

pub(super) fn view_for_ref(pointer: *mut NativeViewRuntime, reference: u32) -> Result<View, u32> {
    let runtime = runtime_mut(pointer)?;
    runtime.resolve_ref(reference).map(|(view, _)| view)
}

fn node_id(low: u32, high: u32) -> Result<u64, u32> {
    if high > 0x001f_ffff || (high == 0 && low == 0) {
        return Err(FAST_INVALID);
    }
    Ok((u64::from(high) << 32) | u64::from(low))
}

fn record_result(runtime: &NativeViewRuntime, result: u32) -> u32 {
    runtime.status.record(result, 0)
}

fn record_result_with_detail(runtime: &NativeViewRuntime, result: u32, detail: u32) -> u32 {
    runtime.status.record(result, detail)
}

fn child_status_detail(index: usize) -> u32 {
    STATUS_DETAIL_CHILD_INDEX | (index as u32 & 0x3fff_ffff)
}

fn base_status_detail() -> u32 {
    STATUS_DETAIL_BASE_REF
}

fn record_child_cache_miss(runtime: &NativeViewRuntime, index: usize) -> u32 {
    record_result_with_detail(runtime, FAST_CACHE_MISS, child_status_detail(index))
}

fn record_base_cache_miss(runtime: &NativeViewRuntime) -> u32 {
    record_result_with_detail(runtime, FAST_CACHE_MISS, base_status_detail())
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_status_detail_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return 0;
    };
    runtime.status.detail.load(Ordering::Acquire)
}

fn record_host_status(runtime: &NativeViewRuntime, status: i32) -> i32 {
    runtime.status.record(status as u32, 0);
    status
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn runtime_noop_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    record_result(runtime, 1)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_render_ref_impl(
    runtime: *mut NativeViewRuntime,
    base: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = match runtime.resolve_ref(base) {
        Ok(_) => base,
        Err(error) => error,
    };
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn host_render_ref_impl(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    base: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return HOST_STATUS_INVALID;
    };
    let status = {
        let Some(host) = (unsafe { host.as_ref() }) else {
            return record_host_status(runtime, HOST_STATUS_INVALID);
        };
        if !host.alive.load(Ordering::Acquire) {
            return record_host_status(runtime, HOST_STATUS_INVALID);
        }
        let Ok((view, _)) = runtime.resolve_ref(base) else {
            return record_host_status(runtime, HOST_STATUS_CACHE_MISS);
        };
        match host.host.render(view) {
            Ok(()) => HOST_STATUS_OK,
            Err(_) => HOST_STATUS_INTERNAL,
        }
    };
    record_host_status(runtime, status)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_ref_for_node_id_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    let result = match runtime.ref_for_node_id(node_id) {
        Ok(reference) => reference,
        Err(error) => error,
    };
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn path_root_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let reference = runtime.path_root();
    record_result(runtime, reference)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn path_child_impl(
    runtime: *mut NativeViewRuntime,
    parent_path_ref: u32,
    step_kind: u32,
    expected_view_kind: u32,
    selector: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = runtime
        .path_child(parent_path_ref, step_kind, expected_view_kind, selector)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

fn publish_text_path(
    runtime: &mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    node_ids: &[(u32, u32)],
    wrap: u32,
    align: u32,
) -> u32 {
    if path_depth > 4 || node_ids.len() != path_depth as usize + 1 {
        return FAST_INVALID;
    }
    if !is_valid_view_ref(base_root_ref) {
        return FAST_INVALID;
    }
    let steps = match runtime.path_steps(path_ref) {
        Ok(steps) => steps,
        Err(error) => return error,
    };
    if steps.len() != path_depth as usize {
        return FAST_INVALID;
    }
    let Ok(wrap) = decode_wrap(wrap) else {
        return FAST_INVALID;
    };
    let Ok(align) = decode_align(align) else {
        return FAST_INVALID;
    };
    let mut decoded_ids = Vec::with_capacity(node_ids.len());
    for &(low, high) in node_ids {
        let Ok(node_id) = node_id(low, high) else {
            return FAST_INVALID;
        };
        decoded_ids.push(node_id);
    }
    let Ok((base_view, _)) = runtime.resolve_ref(base_root_ref) else {
        return FAST_CACHE_MISS;
    };
    let Ok((root, views)) =
        base_view.try_with_text_layout_patch_path_with_nodes(&steps, wrap, align)
    else {
        return FAST_INVALID;
    };
    if views.len() != decoded_ids.len() || views.last() != Some(&root) {
        return FAST_INVALID;
    }
    if let Err(error) = validate_path_publication(runtime, &decoded_ids, &views) {
        return error;
    }
    let mut root_ref = 0;
    let last_index = views.len().saturating_sub(1);
    for (index, (node_id, view)) in decoded_ids.into_iter().zip(views).enumerate() {
        let result = if index == last_index {
            runtime.publish(node_id, view)
        } else {
            runtime.publish_bulk(node_id, view)
        };
        match result {
            Ok(reference) => root_ref = reference,
            Err(error) => return error,
        }
    }
    root_ref
}

fn validate_path_publication(
    runtime: &mut NativeViewRuntime,
    node_ids: &[u64],
    views: &[View],
) -> Result<(), u32> {
    let mut unique = std::collections::HashSet::with_capacity(node_ids.len());
    for (node_id, view) in node_ids.iter().copied().zip(views) {
        if !unique.insert(node_id) {
            return Err(FAST_INVALID);
        }
        if let Some(existing) = runtime
            .nodes
            .get(&node_id)
            .and_then(iyon_tui::WeakView::upgrade)
            && existing != *view
        {
            return Err(FAST_INVALID);
        }
        if let Some(reference) = runtime.node_refs.get(&node_id).copied()
            && let Ok((existing, _)) = runtime.resolve_ref(reference)
            && existing != *view
        {
            return Err(FAST_INVALID);
        }
    }
    if runtime.next_native_ref >= PATH_ROOT_REF.saturating_sub(node_ids.len() as u32) {
        return Err(FAST_FALLBACK);
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    if path_depth > 4 {
        return record_result(runtime, FAST_INVALID);
    }
    let node_ids = [
        (target_node_id_low, target_node_id_high),
        (ancestor0_node_id_low, ancestor0_node_id_high),
        (ancestor1_node_id_low, ancestor1_node_id_high),
        (ancestor2_node_id_low, ancestor2_node_id_high),
        (ancestor3_node_id_low, ancestor3_node_id_high),
    ];
    let result = publish_text_path(
        runtime,
        base_root_ref,
        path_ref,
        path_depth,
        &node_ids[..path_depth as usize + 1],
        wrap,
        align,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d1_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = publish_text_path(
        runtime,
        base_root_ref,
        path_ref,
        1,
        &[
            (target_node_id_low, target_node_id_high),
            (ancestor0_node_id_low, ancestor0_node_id_high),
        ],
        wrap,
        align,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d2_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = publish_text_path(
        runtime,
        base_root_ref,
        path_ref,
        2,
        &[
            (target_node_id_low, target_node_id_high),
            (ancestor0_node_id_low, ancestor0_node_id_high),
            (ancestor1_node_id_low, ancestor1_node_id_high),
        ],
        wrap,
        align,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d3_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = publish_text_path(
        runtime,
        base_root_ref,
        path_ref,
        3,
        &[
            (target_node_id_low, target_node_id_high),
            (ancestor0_node_id_low, ancestor0_node_id_high),
            (ancestor1_node_id_low, ancestor1_node_id_high),
            (ancestor2_node_id_low, ancestor2_node_id_high),
        ],
        wrap,
        align,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d4_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = publish_text_path(
        runtime,
        base_root_ref,
        path_ref,
        4,
        &[
            (target_node_id_low, target_node_id_high),
            (ancestor0_node_id_low, ancestor0_node_id_high),
            (ancestor1_node_id_low, ancestor1_node_id_high),
            (ancestor2_node_id_low, ancestor2_node_id_high),
            (ancestor3_node_id_low, ancestor3_node_id_high),
        ],
        wrap,
        align,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_begin_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    expected_edit_count: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = runtime
        .begin_edit_txn(base_root_ref, expected_edit_count)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_add_text_layout_impl(
    runtime: *mut NativeViewRuntime,
    txn_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return -1;
    };
    if path_depth > 4 {
        return record_host_status(runtime, 2);
    }
    let ids = [
        (target_node_id_low, target_node_id_high),
        (ancestor0_node_id_low, ancestor0_node_id_high),
        (ancestor1_node_id_low, ancestor1_node_id_high),
        (ancestor2_node_id_low, ancestor2_node_id_high),
        (ancestor3_node_id_low, ancestor3_node_id_high),
    ];
    let mut node_ids = [0_u64; 5];
    for index in 0..=path_depth as usize {
        let Ok(id) = node_id(ids[index].0, ids[index].1) else {
            return record_host_status(runtime, -1);
        };
        node_ids[index] = id;
    }
    let Ok(wrap) = decode_wrap(wrap) else {
        return record_host_status(runtime, -1);
    };
    let Ok(align) = decode_align(align) else {
        return record_host_status(runtime, -1);
    };
    let status = runtime.add_text_layout_edit(txn_ref, path_ref, path_depth, node_ids, wrap, align);
    record_host_status(runtime, status)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_commit_render_impl(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    txn_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Some(host) = (unsafe { host.as_ref() }) else {
        return record_result(runtime, FAST_INVALID);
    };
    if !is_valid_edit_txn_ref(txn_ref) {
        return record_result(runtime, FAST_INVALID);
    }
    if !host.alive.load(Ordering::Acquire) {
        runtime.edit_txns.remove(&txn_ref);
        return record_result(runtime, FAST_INVALID);
    }
    let Some(txn) = runtime.edit_txns.remove(&txn_ref) else {
        return record_result(runtime, FAST_CACHE_MISS);
    };
    if txn.base_root_ref == 0 || txn.edits.is_empty() {
        return record_result(runtime, FAST_INVALID);
    }
    let trie = match runtime.build_edit_trie(&txn) {
        Ok(trie) => trie,
        Err(error) => return record_result(runtime, error),
    };
    let mut staged = Vec::with_capacity(trie.len());
    let root = match runtime.stage_edit_trie(txn.base_view, &trie, 0, &mut staged) {
        Ok(root) => root,
        Err(error) => return record_result(runtime, error),
    };
    if staged.is_empty() || staged.last().map(|(_, view)| view != &root).unwrap_or(true) {
        return record_result(runtime, FAST_INVALID);
    }
    let publication = match runtime.prepare_staged_publication(staged) {
        Ok(publication) => publication,
        Err(error) => return record_result(runtime, error),
    };
    if host.host.render(root).is_err() {
        return record_result(runtime, FAST_INTERNAL);
    }
    let result = runtime.commit_staged_publication(publication);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_abort_impl(
    runtime: *mut NativeViewRuntime,
    txn_ref: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return -1;
    };
    if !is_valid_edit_txn_ref(txn_ref) {
        return record_host_status(runtime, -1);
    }
    let status = if runtime.edit_txns.remove(&txn_ref).is_some() {
        0
    } else {
        1
    };
    record_host_status(runtime, status)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_spacer_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    rows: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: if this NodeId already owns a live
    // semantic View (cross-transport or recovery path), return its ref
    // without consuming payload or child refs.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok(rows) = u16::try_from(rows) else {
        return FAST_INVALID;
    };
    let result = match runtime.publish(node_id, View::spacer(rows)) {
        Ok(reference) => reference,
        Err(error) => error,
    };
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_root_impl(
    runtime: *mut NativeViewRuntime,
    base: u32,
    node_id_low: u32,
    node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref without
    // resolving the base or consuming scalars.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok(wrap) = decode_wrap(wrap) else {
        return FAST_INVALID;
    };
    let Ok(align) = decode_align(align) else {
        return FAST_INVALID;
    };
    let Ok((base_view, _)) = runtime.resolve_ref(base) else {
        return record_base_cache_miss(runtime);
    };
    let Ok(patched) = base_view.try_with_text_layout_patch(Some(wrap), Some(align)) else {
        return FAST_INVALID;
    };
    let result = match runtime.publish(node_id, patched) {
        Ok(reference) => reference,
        Err(error) => error,
    };
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_common_patch_root_impl(
    runtime: *mut NativeViewRuntime,
    base: u32,
    node_id_low: u32,
    node_id_high: u32,
    mask: u32,
    padding_tr: u32,
    padding_bl: u32,
    width_rule: u32,
    height_rule: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
    decoration_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref without
    // resolving the base or consuming scalar arguments.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    if mask == 0 || mask & !PATCH_MASK != 0 {
        return FAST_INVALID;
    }
    // PERF-12 T9: decoration_ref is part of the legacy patch surface but is
    // not consumed by any mask branch; 0 means absent and must not fail.
    if decoration_ref != 0 && runtime.resolve_ref(decoration_ref).is_err() {
        return FAST_CACHE_MISS;
    }
    let Ok((base_view, _)) = runtime.resolve_ref(base) else {
        return record_base_cache_miss(runtime);
    };
    let mut patched = base_view;
    if mask & PATCH_PADDING != 0 {
        patched = patched.padding(Insets::new(
            (padding_tr & 0xffff) as u16,
            (padding_tr >> 16) as u16,
            (padding_bl & 0xffff) as u16,
            (padding_bl >> 16) as u16,
        ));
    }
    if mask & PATCH_WIDTH != 0 {
        patched = match width_rule {
            1 => patched.fit_width(),
            2 => patched.fill_width(),
            _ => return FAST_INVALID,
        };
    }
    if mask & PATCH_HEIGHT != 0 {
        patched = match height_rule {
            1 => patched.fit_height(),
            2 => patched.fill_height(),
            _ => return FAST_INVALID,
        };
    }
    if mask & PATCH_MIN_WIDTH != 0 {
        let Ok(value) = u16::try_from(min_width) else {
            return FAST_INVALID;
        };
        patched = patched.min_width(value);
    }
    if mask & PATCH_MAX_WIDTH != 0 {
        let Ok(value) = u16::try_from(max_width) else {
            return FAST_INVALID;
        };
        patched = patched.max_width(value);
    }
    if mask & PATCH_MIN_HEIGHT != 0 {
        let Ok(value) = u16::try_from(min_height) else {
            return FAST_INVALID;
        };
        patched = patched.min_height(value);
    }
    if mask & PATCH_MAX_HEIGHT != 0 {
        let Ok(value) = u16::try_from(max_height) else {
            return FAST_INVALID;
        };
        patched = patched.max_height(value);
    }
    let result = match runtime.publish(node_id, patched) {
        Ok(reference) => reference,
        Err(error) => error,
    };
    record_result(runtime, result)
}

const AXIS_KIND_ROW: u32 = 1;
const AXIS_KIND_COLUMN: u32 = 2;
const MAX_AXIS_CHILD_COUNT: u32 = 524_288;

#[derive(Clone, Copy, Debug)]
struct StatusFailure {
    code: u32,
    detail: u32,
}

impl From<u32> for StatusFailure {
    fn from(code: u32) -> Self {
        Self { code, detail: 0 }
    }
}

fn resolve_axis_children(
    runtime: &mut NativeViewRuntime,
    children: *const AxisChildInputV1,
    used_child_count: u32,
) -> Result<Vec<(u32, View)>, StatusFailure> {
    if used_child_count > MAX_AXIS_CHILD_COUNT {
        return Err(FAST_FALLBACK.into());
    }
    if used_child_count == 0 {
        return Ok(Vec::new());
    }
    if children.is_null() {
        return Err(FAST_INVALID.into());
    }
    let inputs = unsafe { std::slice::from_raw_parts(children, used_child_count as usize) };
    inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            if input.track_word != 0 {
                let kind = input.track_word & 0xff;
                if !(1..=5).contains(&kind) {
                    return Err(FAST_INVALID.into());
                }
            }
            runtime
                .resolve_ref(input.child_ref)
                .map(|(view, _)| (input.track_word, view))
                .map_err(|code| StatusFailure {
                    code,
                    detail: if code == FAST_CACHE_MISS {
                        child_status_detail(index)
                    } else {
                        0
                    },
                })
        })
        .collect()
}

fn publish_structural_path(
    runtime: &mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    node_id_pairs: &[(u32, u32)],
    axis_index: Option<usize>,
    track_word: u32,
    grid_row: Option<usize>,
    grid_column: Option<usize>,
    child_ref: u32,
) -> u32 {
    if path_depth > 4 || node_id_pairs.len() != path_depth as usize + 1 {
        return FAST_INVALID;
    }
    if !is_valid_view_ref(base_root_ref) || !is_valid_view_ref(child_ref) {
        return FAST_INVALID;
    }
    let steps = match runtime.path_steps(path_ref) {
        Ok(steps) if steps.len() == path_depth as usize => steps,
        Ok(_) => return FAST_INVALID,
        Err(error) => return error,
    };
    let mut node_ids = Vec::with_capacity(node_id_pairs.len());
    for &(low, high) in node_id_pairs {
        let Ok(id) = node_id(low, high) else {
            return FAST_INVALID;
        };
        node_ids.push(id);
    }
    let Ok((base_view, _)) = runtime.resolve_ref(base_root_ref) else {
        return FAST_CACHE_MISS;
    };
    let Ok((child, _)) = runtime.resolve_ref(child_ref) else {
        return FAST_CACHE_MISS;
    };
    let Ok((root, views)) = base_view.native_replace_at_path(
        &steps,
        axis_index,
        track_word,
        grid_row,
        grid_column,
        child,
    ) else {
        return FAST_INVALID;
    };
    if views.len() != node_ids.len() || views.last() != Some(&root) {
        return FAST_INVALID;
    }
    if let Err(error) = validate_path_publication(runtime, &node_ids, &views) {
        return error;
    }
    let publication =
        match runtime.prepare_staged_publication(node_ids.into_iter().zip(views).collect()) {
            Ok(publication) => publication,
            Err(error) => return error,
        };
    runtime.commit_staged_publication(publication)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    axis_kind: u32,
    gap: u32,
    children: *const AxisChildInputV1,
    _children_capacity_bytes: usize,
    used_child_count: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first (see create_small_axis).
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok(gap) = u16::try_from(gap) else {
        return FAST_INVALID;
    };
    // PERF-12 §68/§116 note: count-vs-capacity validation for this
    // constructor is enforced by the generated export layer
    // (generated_buffer_used rejects used_child_count entries that do not
    // fit inside children_capacity_bytes), so the implementation can rely
    // on the slice below being in bounds.
    let children = match resolve_axis_children(runtime, children, used_child_count) {
        Ok(children) => children,
        Err(error) => return record_result_with_detail(runtime, error.code, error.detail),
    };
    let horizontal = match axis_kind {
        AXIS_KIND_ROW => true,
        AXIS_KIND_COLUMN => false,
        _ => return FAST_INVALID,
    };
    let Ok(view) = View::native_axis_from_children(horizontal, gap, children) else {
        return FAST_INVALID;
    };
    let result = runtime.publish(node_id, view).unwrap_or_else(|error| error);
    record_result(runtime, result)
}

fn create_small_axis(
    pointer: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    axis_kind: u32,
    gap: u32,
    children: &[(u32, u32)],
) -> u32 {
    let Ok(runtime) = runtime_mut(pointer) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId short-circuits before
    // any child-ref resolution, so stale children of an already-built node
    // cannot fail an otherwise-known construction.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok(gap) = u16::try_from(gap) else {
        return FAST_INVALID;
    };
    let mut resolved = Vec::with_capacity(children.len());
    for (index, &(track_word, child_ref)) in children.iter().enumerate() {
        if track_word != 0 && !(1..=5).contains(&(track_word & 0xff)) {
            return record_result(runtime, FAST_INVALID);
        }
        let Ok((child, _)) = runtime.resolve_ref(child_ref) else {
            return record_child_cache_miss(runtime, index);
        };
        resolved.push((track_word, child));
    }
    let horizontal = match axis_kind {
        AXIS_KIND_ROW => true,
        AXIS_KIND_COLUMN => false,
        _ => return record_result(runtime, FAST_INVALID),
    };
    let view = match View::native_axis_from_children(horizontal, gap, resolved) {
        Ok(view) => view,
        Err(_) => return record_result(runtime, FAST_INVALID),
    };
    let result = runtime.publish(node_id, view).unwrap_or_else(|error| error);
    record_result(runtime, result)
}

macro_rules! define_small_axis_constructor {
    ($name:ident, $axis_kind:expr, [$($track:ident, $child:ident),* $(,)?]) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "Rust" fn $name(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            $($track: u32, $child: u32,)*) -> u32 {
            create_small_axis(
                runtime,
                node_id_low,
                node_id_high,
                $axis_kind,
                gap,
                &[$(($track, $child)),*],
            )
        }
    };
}

define_small_axis_constructor!(view_row_create_0_impl, AXIS_KIND_ROW, []);
define_small_axis_constructor!(view_row_create_1_impl, AXIS_KIND_ROW, [track0, child0]);
define_small_axis_constructor!(
    view_row_create_2_impl,
    AXIS_KIND_ROW,
    [track0, child0, track1, child1]
);
define_small_axis_constructor!(
    view_row_create_3_impl,
    AXIS_KIND_ROW,
    [track0, child0, track1, child1, track2, child2]
);
define_small_axis_constructor!(
    view_row_create_4_impl,
    AXIS_KIND_ROW,
    [
        track0, child0, track1, child1, track2, child2, track3, child3
    ]
);
define_small_axis_constructor!(view_column_create_0_impl, AXIS_KIND_COLUMN, []);
define_small_axis_constructor!(
    view_column_create_1_impl,
    AXIS_KIND_COLUMN,
    [track0, child0]
);
define_small_axis_constructor!(
    view_column_create_2_impl,
    AXIS_KIND_COLUMN,
    [track0, child0, track1, child1]
);
define_small_axis_constructor!(
    view_column_create_3_impl,
    AXIS_KIND_COLUMN,
    [track0, child0, track1, child1, track2, child2]
);
define_small_axis_constructor!(
    view_column_create_4_impl,
    AXIS_KIND_COLUMN,
    [
        track0, child0, track1, child1, track2, child2, track3, child3
    ]
);

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_begin_impl(
    runtime: *mut NativeViewRuntime,
    axis_kind: u32,
    expected_children: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = runtime
        .begin_axis_builder(axis_kind, expected_children)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_push_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    track_word: u32,
    child_ref: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return -1;
    };
    let status = runtime.push_axis_builder(builder_ref, track_word, child_ref);
    record_host_status(runtime, status)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_finish_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return record_result(runtime, FAST_INVALID);
    };
    let result = runtime
        .finish_axis_builder(builder_ref, node_id, gap)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_abort_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return -1;
    };
    let status = runtime.abort_axis_builder(builder_ref);
    record_host_status(runtime, status)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_set_child_impl(
    runtime: *mut NativeViewRuntime,
    base_axis_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    child_index: u32,
    track_word: u32,
    child_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without resolving the base or consuming edit arguments.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok((base, _)) = runtime.resolve_ref(base_axis_ref) else {
        return record_base_cache_miss(runtime);
    };
    let Ok((child, _)) = runtime.resolve_ref(child_ref) else {
        return record_child_cache_miss(runtime, 0);
    };
    let Ok(patched) = base.native_axis_set_child(child_index as usize, track_word, child) else {
        return FAST_INVALID;
    };
    let result = runtime
        .publish(node_id, patched)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_splice_buffer_impl(
    runtime: *mut NativeViewRuntime,
    base_axis_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    index: u32,
    remove_count: u32,
    children: *const AxisChildInputV1,
    _children_capacity_bytes: usize,
    used_child_count: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without resolving the base or consuming edit arguments.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok((base, _)) = runtime.resolve_ref(base_axis_ref) else {
        return record_base_cache_miss(runtime);
    };
    let inserted = match resolve_axis_children(runtime, children, used_child_count) {
        Ok(inserted) => inserted,
        Err(error) => return record_result_with_detail(runtime, error.code, error.detail),
    };
    let Ok(patched) = base.native_axis_splice(index as usize, remove_count as usize, inserted)
    else {
        return FAST_INVALID;
    };
    let result = runtime
        .publish(node_id, patched)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_set_cell_impl(
    runtime: *mut NativeViewRuntime,
    base_grid_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    row: u32,
    column: u32,
    child_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without resolving the base or consuming edit arguments.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok((base, _)) = runtime.resolve_ref(base_grid_ref) else {
        return record_base_cache_miss(runtime);
    };
    let Ok((child, _)) = runtime.resolve_ref(child_ref) else {
        return record_child_cache_miss(runtime, 0);
    };
    let Ok(patched) = base.native_grid_set_cell(row as usize, column as usize, child) else {
        return FAST_INVALID;
    };
    let result = runtime
        .publish(node_id, patched)
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

/// PERF-12 T10 (§36): parses the flat u32 word buffer describing a new
/// grid and constructs it through the semantic builder. Every read is
/// bounds-checked; the buffer must be consumed exactly.
fn parse_and_build_grid(
    words: &[u32],
    resolve_child: &mut dyn FnMut(u32) -> Result<View, u32>,
    column_gap: u16,
    row_gap: u16,
) -> Result<View, u32> {
    let mut cursor = 0usize;
    let mut next_word = || -> Result<u32, u32> {
        let value = *words.get(cursor).ok_or(FAST_INVALID)?;
        cursor += 1;
        Ok(value)
    };
    let decode_track = |word: u32| -> Result<GridTrack, u32> {
        let kind = word & 0xff;
        let raw_amount = word >> 8;
        if raw_amount > u16::MAX as u32 {
            return Err(FAST_INVALID);
        }
        let amount = raw_amount as u16;
        match kind {
            GRID_TRACK_CONTENT_WORD if amount == 0 => Ok(GridTrack::content()),
            GRID_TRACK_FLEX_WORD if amount == 0 => Ok(GridTrack::flex()),
            GRID_TRACK_FIXED_WORD => Ok(GridTrack::fixed(
                u16::try_from(amount).map_err(|_| FAST_INVALID)?,
            )),
            GRID_TRACK_CONTENT_MAX_WORD => Ok(GridTrack::content_max(
                u16::try_from(amount).map_err(|_| FAST_INVALID)?,
            )),
            GRID_TRACK_FLEX_MAX_WORD => Ok(GridTrack::flex_max(
                u16::try_from(amount).map_err(|_| FAST_INVALID)?,
            )),
            _ => Err(FAST_INVALID),
        }
    };
    let column_track_count = next_word()?;
    if column_track_count > words.len() as u32 {
        return Err(FAST_INVALID);
    }
    let mut column_tracks = Vec::with_capacity(column_track_count as usize);
    for _ in 0..column_track_count {
        column_tracks.push(decode_track(next_word()?)?);
    }
    let row_count = next_word()?;
    if row_count > words.len() as u32 {
        return Err(FAST_INVALID);
    }
    // Two-phase parse: validate the whole layout (including child refs)
    // before constructing anything, so a malformed tail cannot leave partial
    // work behind.
    let mut parsed_rows: Vec<(GridTrack, Vec<(GridCellSpec, View)>)> =
        Vec::with_capacity(row_count as usize);
    for _ in 0..row_count {
        let row_track = decode_track(next_word()?)?;
        let cell_count = next_word()?;
        if cell_count > words.len() as u32 {
            return Err(FAST_INVALID);
        }
        let mut cells = Vec::with_capacity(cell_count as usize);
        for _ in 0..cell_count {
            let child_ref = next_word()?;
            let span_pack = next_word()?;
            let align_pack = next_word()?;
            let view = resolve_child(child_ref)?;
            let column_span = (span_pack & 0xffff) as u16;
            let row_span = (span_pack >> 16) as u16;
            if column_span == 0 || row_span == 0 {
                return Err(FAST_INVALID);
            }
            let spec = GridCellSpec::new()
                .column_span(column_span)
                .row_span(row_span)
                .horizontal_align(match align_pack & 0xffff {
                    1 => HorizontalAlign::Start,
                    2 => HorizontalAlign::Center,
                    3 => HorizontalAlign::End,
                    _ => return Err(FAST_INVALID),
                })
                .vertical_align(match align_pack >> 16 {
                    1 => VerticalAlign::Top,
                    2 => VerticalAlign::Center,
                    3 => VerticalAlign::Bottom,
                    _ => return Err(FAST_INVALID),
                });
            cells.push((spec, view));
        }
        parsed_rows.push((row_track, cells));
    }
    if cursor != words.len() {
        return Err(FAST_INVALID);
    }
    Ok(View::grid(|grid| {
        grid.columns(column_tracks);
        grid.column_gap(column_gap);
        grid.row_gap(row_gap);
        for (track, cells) in parsed_rows {
            grid.row_with(track, |row| {
                for (spec, view) in &cells {
                    row.cell_with(*spec, view.clone());
                }
            });
        }
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    column_gap: u32,
    row_gap: u32,
    words: *const u32,
    _words_capacity_bytes: usize,
    used_word_count: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing or resolving any buffered word.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    // PERF-12 §68/§116 note: count-vs-capacity validation is enforced by
    // the generated export layer before this implementation runs.
    if used_word_count == 0 || words.is_null() {
        return record_result(runtime, FAST_INVALID);
    }
    let Ok(column_gap) = u16::try_from(column_gap) else {
        return FAST_INVALID;
    };
    let Ok(row_gap) = u16::try_from(row_gap) else {
        return FAST_INVALID;
    };
    let slice = unsafe { slice::from_raw_parts(words, used_word_count as usize) };
    let mut child_index = 0usize;
    let mut detail = 0;
    let outcome = parse_and_build_grid(
        slice,
        &mut |child_ref: u32| {
            let index = child_index;
            child_index += 1;
            runtime
                .resolve_ref(child_ref)
                .map(|(view, _)| view)
                .map_err(|error| {
                    if error == FAST_CACHE_MISS {
                        detail = child_status_detail(index);
                    }
                    error
                })
        },
        column_gap,
        row_gap,
    );
    match outcome {
        Ok(view) => match runtime.publish(node_id, view) {
            Ok(reference) => record_result(runtime, reference),
            Err(error) => record_result(runtime, error),
        },
        Err(error) => record_result_with_detail(runtime, error, detail),
    }
}

/// PERF-12 T11 (§41): parses the framed words+bytes payload describing a new
/// Diff view and constructs it through the semantic `DiffRenderer` lowering
/// used by the Direct decoder. Every read is bounds-checked; the word buffer
/// must be consumed exactly and byte lengths must sum to the byte buffer.
fn parse_and_build_diff(words: &[u32], bytes: &[u8]) -> Result<View, u32> {
    // Canonical enum codes shared with the bridge schema and Direct decoder.
    const DIFF_LINE_CONTEXT: u32 = 1;
    const DIFF_LINE_ADDITION: u32 = 2;
    const DIFF_LINE_DELETION: u32 = 3;
    const DIFF_TERMINATED: u32 = 1;
    const DIFF_UNTERMINATED: u32 = 2;
    let mut word_cursor = 0usize;
    let mut next_word = || -> Result<u32, u32> {
        let value = *words.get(word_cursor).ok_or(FAST_INVALID)?;
        word_cursor += 1;
        Ok(value)
    };
    // JS numbers are safe integers below 2^53, so the high word of any
    // coordinate carried by a canonical producer fits in 21 bits.
    let coordinate = |low: u32, high: u32| -> Result<u64, u32> {
        if high > 0x001f_ffff {
            return Err(FAST_INVALID);
        }
        Ok((u64::from(high) << 32) | u64::from(low))
    };
    let line_number =
        |raw: u64| -> Result<DiffLineNumber, u32> { DiffLineNumber::new(raw).ok_or(FAST_INVALID) };
    let hunk_count = next_word()? as usize;
    if hunk_count > words.len() {
        return Err(FAST_INVALID);
    }
    let mut hunks = Vec::with_capacity(hunk_count);
    let mut byte_cursor = 0usize;
    for _ in 0..hunk_count {
        let old_start = coordinate(next_word()?, next_word()?)?;
        let old_count = coordinate(next_word()?, next_word()?)?;
        let new_start = coordinate(next_word()?, next_word()?)?;
        let new_count = coordinate(next_word()?, next_word()?)?;
        if old_start > u64::from(u32::MAX)
            || old_count > u64::from(u32::MAX)
            || new_start > u64::from(u32::MAX)
            || new_count > u64::from(u32::MAX)
        {
            return Err(FAST_INVALID);
        }
        let old_range =
            DiffRange::new(DiffLineOffset::new(old_start), old_count).map_err(|_| FAST_INVALID)?;
        let new_range =
            DiffRange::new(DiffLineOffset::new(new_start), new_count).map_err(|_| FAST_INVALID)?;
        let line_count = next_word()? as usize;
        if line_count > words.len() {
            return Err(FAST_INVALID);
        }
        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let meta = next_word()?;
            let kind = meta & 0xffff;
            let termination = match meta >> 16 {
                DIFF_TERMINATED => DiffLineTermination::Terminated,
                DIFF_UNTERMINATED => DiffLineTermination::Unterminated,
                _ => return Err(FAST_INVALID),
            };
            let old_line_raw = coordinate(next_word()?, next_word()?)?;
            let new_line_raw = coordinate(next_word()?, next_word()?)?;
            let text_bytes = next_word()? as usize;
            let text_end = byte_cursor.checked_add(text_bytes).ok_or(FAST_INVALID)?;
            if text_end > bytes.len() {
                return Err(FAST_INVALID);
            }
            let text = str::from_utf8(&bytes[byte_cursor..text_end])
                .map_err(|_| FAST_INVALID)?
                .to_owned();
            byte_cursor = text_end;
            let line = match kind {
                DIFF_LINE_CONTEXT => {
                    DiffLine::context(line_number(old_line_raw)?, line_number(new_line_raw)?, text)
                }
                DIFF_LINE_ADDITION => {
                    if old_line_raw != 0 {
                        return Err(FAST_INVALID);
                    }
                    DiffLine::addition(line_number(new_line_raw)?, text)
                }
                DIFF_LINE_DELETION => {
                    if new_line_raw != 0 {
                        return Err(FAST_INVALID);
                    }
                    DiffLine::deletion(line_number(old_line_raw)?, text)
                }
                _ => return Err(FAST_INVALID),
            };
            lines.push(line.with_termination(termination));
        }
        hunks.push(DiffHunk::new(old_range, new_range, lines).map_err(|_| FAST_INVALID)?);
    }
    if word_cursor != words.len() || byte_cursor != bytes.len() {
        return Err(FAST_INVALID);
    }
    Ok(DiffRenderer::new().render(hunks.as_slice()))
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_diff_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    words: *const u32,
    _words_capacity_bytes: usize,
    used_word_count: u32,
    bytes: *const u8,
    _bytes_capacity_bytes: usize,
    used_byte_count: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing or resolving any buffered payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    if used_word_count == 0 || words.is_null() || bytes.is_null() {
        return record_result(runtime, FAST_INVALID);
    }
    let words = unsafe { slice::from_raw_parts(words, used_word_count as usize) };
    let bytes = if used_byte_count == 0 {
        &[] as &[u8]
    } else {
        unsafe { slice::from_raw_parts(bytes, used_byte_count as usize) }
    };
    match parse_and_build_diff(words, bytes) {
        Ok(view) => match runtime.publish(node_id, view) {
            Ok(reference) => record_result(runtime, reference),
            Err(error) => record_result(runtime, error),
        },
        Err(error) => record_result(runtime, error),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_set_child_path_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    axis_index: u32,
    track_word: u32,
    child_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    if path_depth > 4 {
        return record_result(runtime, FAST_INVALID);
    }
    let ids = [
        (target_node_id_low, target_node_id_high),
        (ancestor0_node_id_low, ancestor0_node_id_high),
        (ancestor1_node_id_low, ancestor1_node_id_high),
        (ancestor2_node_id_low, ancestor2_node_id_high),
        (ancestor3_node_id_low, ancestor3_node_id_high),
    ];
    let result = publish_structural_path(
        runtime,
        base_root_ref,
        path_ref,
        path_depth,
        &ids[..path_depth as usize + 1],
        Some(axis_index as usize),
        track_word,
        None,
        None,
        child_ref,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_set_cell_path_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    grid_row: u32,
    grid_column: u32,
    child_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    if path_depth > 4 {
        return record_result(runtime, FAST_INVALID);
    }
    let ids = [
        (target_node_id_low, target_node_id_high),
        (ancestor0_node_id_low, ancestor0_node_id_high),
        (ancestor1_node_id_low, ancestor1_node_id_high),
        (ancestor2_node_id_low, ancestor2_node_id_high),
        (ancestor3_node_id_low, ancestor3_node_id_high),
    ];
    let result = publish_structural_path(
        runtime,
        base_root_ref,
        path_ref,
        path_depth,
        &ids[..path_depth as usize + 1],
        None,
        0,
        Some(grid_row as usize),
        Some(grid_column as usize),
        child_ref,
    );
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_release_many_impl(
    runtime: *mut NativeViewRuntime,
    refs: *const u32,
    _refs_capacity_bytes: usize,
    used_ref_count: u32,
) -> i32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return -1;
    };
    runtime.release_many(refs, used_ref_count).unwrap_or(-1)
}

fn reserve_staged_ref(
    slots: &NativeRefTable,
    planned: &mut HashSet<u32>,
    next: &mut u32,
) -> Result<u32, u32> {
    while *next < PATH_ROOT_REF {
        let candidate = *next;
        *next = (*next).saturating_add(1);
        if candidate != 0 && !slots.contains_key(&candidate) && planned.insert(candidate) {
            return Ok(candidate);
        }
    }
    Err(FAST_FALLBACK)
}

fn is_valid_view_ref(reference: u32) -> bool {
    (1..PATH_ROOT_REF).contains(&reference)
}

fn is_valid_path_ref(reference: u32) -> bool {
    reference == PATH_ROOT_REF || (PATH_ROOT_REF..PATH_REF_LIMIT).contains(&reference)
}

fn is_valid_builder_ref(reference: u32) -> bool {
    (BUILDER_REF_START..BUILDER_REF_LIMIT).contains(&reference)
}

fn is_valid_edit_txn_ref(reference: u32) -> bool {
    (EDIT_TXN_REF_START..EDIT_TXN_REF_LIMIT).contains(&reference)
}

fn set_trie_node_id(node: &mut EditTrieNode, node_id: u64) -> Result<(), u32> {
    match node.node_id {
        Some(existing) if existing != node_id => Err(FAST_INVALID),
        Some(_) => Ok(()),
        None => {
            node.node_id = Some(node_id);
            Ok(())
        }
    }
}

fn path_step_matches_kind(step_kind: u32, expected_view_kind: u32) -> bool {
    match step_kind {
        1 => expected_view_kind == 6,
        2 => expected_view_kind == 7,
        3 => expected_view_kind == 8,
        4 => expected_view_kind == 3,
        5 => expected_view_kind == 2,
        6 => expected_view_kind == 4,
        7..=9 => expected_view_kind == 5,
        _ => false,
    }
}

fn style_from_bits(
    runtime: &NativeViewRuntime,
    flags: u32,
    attribute_present: u32,
    attribute_true: u32,
    foreground_ref: u32,
    background_ref: u32,
    theme_atom_ref: u32,
) -> Result<StyleRef, u32> {
    if flags != 0
        || attribute_present & !STYLE_ATTRIBUTE_BITS != 0
        || attribute_true & !attribute_present != 0
    {
        return Err(FAST_INVALID);
    }
    let mut local = StyleSpec::new();
    for (bit, attribute) in [
        (1, TextAttribute::Bold),
        (2, TextAttribute::Dim),
        (4, TextAttribute::Italic),
        (8, TextAttribute::Underline),
        (16, TextAttribute::Reversed),
        (32, TextAttribute::Strikethrough),
    ] {
        if attribute_present & bit != 0 {
            local = local.attribute(attribute, attribute_true & bit != 0);
        }
    }
    if foreground_ref != 0 {
        local = local.foreground(parse_color_atom(runtime.style_atom_value(foreground_ref)?)?);
    }
    if background_ref != 0 {
        local = local.background(parse_color_atom(runtime.style_atom_value(background_ref)?)?);
    }
    if theme_atom_ref == 0 {
        Ok(StyleRef::direct(local))
    } else {
        let theme = runtime.style_atom_value(theme_atom_ref)?;
        Ok(StyleRef::themed(
            theme.strip_prefix("theme:").unwrap_or(theme),
            local,
        ))
    }
}

fn parse_color_atom(value: &str) -> Result<ColorSpec, u32> {
    if let Some(theme) = value.strip_prefix("theme:") {
        return Ok(ColorSpec::theme(theme));
    }
    if let Some(ansi) = value.strip_prefix("ansi:") {
        return ansi
            .parse::<u8>()
            .map(ColorSpec::ansi)
            .map_err(|_| FAST_INVALID);
    }
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(FAST_INVALID);
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| FAST_INVALID)?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| FAST_INVALID)?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| FAST_INVALID)?;
        return Ok(ColorSpec::rgb(r, g, b));
    }
    let color = match value.to_ascii_lowercase().as_str() {
        "black" => AnsiColor::Black,
        "red" => AnsiColor::Red,
        "green" => AnsiColor::Green,
        "yellow" => AnsiColor::Yellow,
        "blue" => AnsiColor::Blue,
        "magenta" => AnsiColor::Magenta,
        "cyan" => AnsiColor::Cyan,
        "gray" => AnsiColor::Gray,
        "darkgray" => AnsiColor::DarkGray,
        "lightred" => AnsiColor::LightRed,
        "lightgreen" => AnsiColor::LightGreen,
        "lightyellow" => AnsiColor::LightYellow,
        "lightblue" => AnsiColor::LightBlue,
        "lightmagenta" => AnsiColor::LightMagenta,
        "lightcyan" => AnsiColor::LightCyan,
        "white" => AnsiColor::White,
        _ => return Err(FAST_INVALID),
    };
    Ok(ColorSpec::named(color))
}

fn text_view_from_spans(spans: Vec<TextSpan>, wrap: u32, align: u32) -> Result<View, u32> {
    let wrap = decode_wrap(wrap).map_err(|_| FAST_INVALID)?;
    let align = decode_align(align).map_err(|_| FAST_INVALID)?;
    Ok(View::styled_text(spans)
        .wrap(wrap)
        .text_align(align)
        .into_view())
}

fn text_view_from_owned(text: String, style: StyleRef, wrap: u32, align: u32) -> Result<View, u32> {
    text_view_from_spans(vec![TextSpan::styled(text, style)], wrap, align)
}

fn cstring_to_owned(pointer: *const std::ffi::c_char, maximum_bytes: u32) -> Result<String, u32> {
    let bytes = unsafe { CStr::from_ptr(pointer) }.to_bytes();
    if bytes.len() > maximum_bytes as usize {
        return Err(FAST_FALLBACK);
    }
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| FAST_INVALID)
}

fn cstring_text_spans(
    runtime: &NativeViewRuntime,
    inputs: &[(*const std::ffi::c_char, u32)],
) -> Result<Vec<TextSpan>, u32> {
    inputs
        .iter()
        .map(|(pointer, style_ref)| {
            let text = cstring_to_owned(*pointer, MAX_NEW_TEXT_BYTES)?;
            let style = runtime.style_for_ref(*style_ref)?;
            Ok(TextSpan::styled(text, style))
        })
        .collect()
}

fn publish_cstring_text(
    runtime: &mut NativeViewRuntime,
    node_id: u64,
    inputs: &[(*const std::ffi::c_char, u32)],
    wrap: u32,
    align: u32,
) -> Result<u32, u32> {
    let spans = cstring_text_spans(runtime, inputs)?;
    let view = text_view_from_spans(spans, wrap, align)?;
    runtime.publish(node_id, view)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn style_atom_create_cstring_impl(
    runtime: *mut NativeViewRuntime,
    value: *const std::ffi::c_char,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(value) = (unsafe { CStr::from_ptr(value) }).to_str() else {
        return record_result(runtime, FAST_INVALID);
    };
    let result = runtime.style_atom(value).unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn style_create_bits_impl(
    runtime: *mut NativeViewRuntime,
    flags: u32,
    attribute_present: u32,
    attribute_true: u32,
    foreground_ref: u32,
    background_ref: u32,
    theme_atom_ref: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let result = style_from_bits(
        runtime,
        flags,
        attribute_present,
        attribute_true,
        foreground_ref,
        background_ref,
        theme_atom_ref,
    )
    .and_then(|style| runtime.style(style))
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text: *const std::ffi::c_char,
    style_ref: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let Ok(text) = cstring_to_owned(text, MAX_NEW_TEXT_BYTES) else {
        return record_result(runtime, FAST_INVALID);
    };
    let result = runtime
        .style_for_ref(style_ref)
        .and_then(|style| text_view_from_owned(text, style, wrap, align))
        .and_then(|view| runtime.publish(node_id, view))
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    _bytes_capacity: usize,
    used_bytes: u32,
    style_ref: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let bytes = if used_bytes == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(bytes, used_bytes as usize) }
    };
    let Ok(text) = str::from_utf8(bytes).map(str::to_owned) else {
        return record_result(runtime, FAST_INVALID);
    };
    let result = runtime
        .style_for_ref(style_ref)
        .and_then(|style| text_view_from_owned(text, style, wrap, align))
        .and_then(|view| runtime.publish(node_id, view))
        .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

fn utf8_text_spans(
    runtime: &NativeViewRuntime,
    bytes: *const u8,
    used_bytes: u32,
    span_bytes: &[u32],
    style_refs: &[u32],
) -> Result<Vec<TextSpan>, u32> {
    if span_bytes.len() != style_refs.len() {
        return Err(FAST_INVALID);
    }
    let total = span_bytes.iter().try_fold(0usize, |total, length| {
        total.checked_add(*length as usize).ok_or(FAST_INVALID)
    })?;
    if total != used_bytes as usize || total > MAX_NEW_TEXT_BYTES as usize {
        return Err(FAST_INVALID);
    }
    let bytes = if total == 0 {
        &[]
    } else {
        if bytes.is_null() {
            return Err(FAST_INVALID);
        }
        unsafe { slice::from_raw_parts(bytes, total) }
    };
    let mut offset = 0usize;
    span_bytes
        .iter()
        .zip(style_refs)
        .map(|(length, style_ref)| {
            let end = offset.checked_add(*length as usize).ok_or(FAST_INVALID)?;
            let text = str::from_utf8(&bytes[offset..end])
                .map(str::to_owned)
                .map_err(|_| FAST_INVALID)?;
            offset = end;
            let style = runtime.style_for_ref(*style_ref)?;
            Ok(TextSpan::styled(text, style))
        })
        .collect()
}

fn publish_utf8_text(
    runtime: &mut NativeViewRuntime,
    node_id: u64,
    bytes: *const u8,
    used_bytes: u32,
    span_bytes: &[u32],
    style_refs: &[u32],
    wrap: u32,
    align: u32,
) -> Result<u32, u32> {
    let spans = utf8_text_spans(runtime, bytes, used_bytes, span_bytes, style_refs)?;
    let view = text_view_from_spans(spans, wrap, align)?;
    runtime.publish(node_id, view)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_2_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    _bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_utf8_text(
        runtime,
        node_id,
        bytes,
        used_bytes,
        &[span0_bytes, span1_bytes],
        &[style0, style1],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_3_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    _bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    span2_bytes: u32,
    style2: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_utf8_text(
        runtime,
        node_id,
        bytes,
        used_bytes,
        &[span0_bytes, span1_bytes, span2_bytes],
        &[style0, style1, style2],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_4_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    _bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    span2_bytes: u32,
    style2: u32,
    span3_bytes: u32,
    style3: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_utf8_text(
        runtime,
        node_id,
        bytes,
        used_bytes,
        &[span0_bytes, span1_bytes, span2_bytes, span3_bytes],
        &[style0, style1, style2, style3],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_2_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const std::ffi::c_char,
    style0: u32,
    text1: *const std::ffi::c_char,
    style1: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_cstring_text(
        runtime,
        node_id,
        &[(text0, style0), (text1, style1)],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_3_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const std::ffi::c_char,
    style0: u32,
    text1: *const std::ffi::c_char,
    style1: u32,
    text2: *const std::ffi::c_char,
    style2: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_cstring_text(
        runtime,
        node_id,
        &[(text0, style0), (text1, style1), (text2, style2)],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_4_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const std::ffi::c_char,
    style0: u32,
    text1: *const std::ffi::c_char,
    style1: u32,
    text2: *const std::ffi::c_char,
    style2: u32,
    text3: *const std::ffi::c_char,
    style3: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let Ok(runtime) = runtime_mut(runtime) else {
        return FAST_INVALID;
    };
    let Ok(node_id) = node_id(node_id_low, node_id_high) else {
        return FAST_INVALID;
    };
    // PERF-12 §23 semantic-cache-first: a live NodeId returns its ref
    // without parsing any payload.
    match runtime.ref_for_node_id(node_id) {
        Ok(reference) => return record_result(runtime, reference),
        Err(FAST_CACHE_MISS) => {}
        Err(error) => return record_result(runtime, error),
    }
    let result = publish_cstring_text(
        runtime,
        node_id,
        &[
            (text0, style0),
            (text1, style1),
            (text2, style2),
            (text3, style3),
        ],
        wrap,
        align,
    )
    .unwrap_or_else(|error| error);
    record_result(runtime, result)
}

fn decode_wrap(value: u32) -> Result<WrapMode, ()> {
    match value {
        1 => Ok(WrapMode::WordThenGrapheme),
        2 => Ok(WrapMode::Grapheme),
        3 => Ok(WrapMode::NoWrap),
        _ => Err(()),
    }
}

fn decode_align(value: u32) -> Result<HorizontalAlign, ()> {
    match value {
        1 => Ok(HorizontalAlign::Start),
        2 => Ok(HorizontalAlign::Center),
        3 => Ok(HorizontalAlign::End),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AxisChildInputV1, FAST_CACHE_MISS, FAST_INVALID, GRID_TRACK_CONTENT_WORD, MAX_EDIT_COUNT,
        NativeRefTable, NativeViewKindTag, NativeViewRuntime, NativeViewSlot, PATH_ROOT_REF,
        STATUS_DETAIL_CHILD_INDEX, generated_exports, is_valid_builder_ref, is_valid_edit_txn_ref,
    };
    use iyon_tui::{GridTrack, IntoView, TextSpan, View};
    use std::ffi::CString;
    use std::time::Instant;

    fn slot_for(node_id: u64) -> NativeViewSlot {
        let view = View::spacer(1);
        let weak = view.downgrade();
        NativeViewSlot {
            node_id,
            weak,
            leased: Some(view),
            js_lease_count: 1,
            kind: NativeViewKindTag::View,
        }
    }

    #[test]
    fn native_ref_table_maps_refs_across_pages() {
        // Page-boundary and cross-page coverage for the dense table (§52).
        let mut table = NativeRefTable::<10>::default();
        assert_eq!(table.len(), 0);
        assert!(!table.contains_key(&1));
        let page_span = [
            1u32,
            2,
            (1 << 10) - 1,
            1 << 10,
            (1 << 10) + 1,
            (1 << 12) + 7,
        ];
        for &reference in &page_span {
            assert!(
                table
                    .insert(reference, slot_for(u64::from(reference)))
                    .is_none()
            );
        }
        assert_eq!(table.len(), page_span.len());
        for &reference in &page_span {
            assert_eq!(
                table.get(&reference).map(|slot| slot.node_id),
                Some(u64::from(reference))
            );
            assert!(table.contains_key(&reference));
        }
        assert!(table.get(&0).is_none());
        assert!(table.get(&(1 << 20)).is_none());
        // Replace semantics match HashMap::insert.
        assert!(table.insert((1 << 10) + 1, slot_for(u64::MAX)).is_some());
        assert_eq!(
            table.get(&((1 << 10) + 1)).map(|slot| slot.node_id),
            Some(u64::MAX)
        );
        assert_eq!(table.len(), page_span.len());
        // Removal drops empty pages but keeps the directory high-water (§54).
        for &reference in &page_span {
            assert!(table.remove(&reference).is_some());
        }
        assert_eq!(table.len(), 0);
        assert!(table.remove(&1).is_none());
        assert_eq!(table.iter().count(), 0);
    }

    #[test]
    fn native_ref_table_iter_matches_hashmap_semantics() {
        let mut table = NativeRefTable::<12>::default();
        let mut reference = std::collections::HashMap::new();
        for id in 1u32..=5_000u32 {
            let node = u64::from(id * 3 + 1);
            table
                .insert(id * 7, slot_for(node))
                .map(|_| ())
                .unwrap_or_default();
            reference.insert(id * 7, node);
        }
        assert_eq!(table.len(), reference.len());
        let mut seen: Vec<(u32, u64)> = table
            .iter()
            .map(|(key, slot)| (key, slot.node_id))
            .collect();
        seen.sort_unstable();
        let mut expected: Vec<(u32, u64)> = reference.into_iter().collect();
        expected.sort_unstable();
        assert_eq!(seen, expected);
        // Expired-weak slots are still physical entries until removed; iter must
        // surface them exactly like the former HashMap did.
        assert!(table.values().count() == seen.len());
    }

    /// Representation benchmark backing the PERF-12 T2 decision (§52/§88).
    /// Not a timing gate: prints measured ns/lookup so the chosen
    /// representation and page size are recorded by measurement. Run with:
    /// cargo test -p iyon-native --release -- --ignored --nocapture native_ref_table_representation
    #[test]
    #[ignore = "representation benchmark; run in release with --ignored --nocapture"]
    fn native_ref_table_representation_benchmark() {
        const LIVE_SLOTS: u32 = 8_192;
        const LOOKUPS: u32 = 4_000_000;
        let mut state = 0x9e37_79b9_u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            1 + (state % LIVE_SLOTS)
        };
        let keys: Vec<u32> = (0..LIVE_SLOTS).map(|_| next()).collect();
        let mut hash_map = std::collections::HashMap::new();
        let mut paged_10 = NativeRefTable::<10>::default();
        let mut paged_12 = NativeRefTable::<12>::default();
        for &key in &keys {
            hash_map.insert(key, slot_for(u64::from(key)));
            paged_10.insert(key, slot_for(u64::from(key)));
            paged_12.insert(key, slot_for(u64::from(key)));
        }
        let lookup_keys: Vec<u32> = (0..LOOKUPS).map(|_| next()).collect();
        fn measure(label: &str, lookup_keys: &[u32], elapsed_ns: u128) {
            println!(
                "{label}: {elapsed_ns} ns total, {:.2} ns/lookup",
                elapsed_ns as f64 / lookup_keys.len() as f64
            );
        }
        let started = Instant::now();
        let mut hash_checksum = 0u64;
        for &key in &lookup_keys {
            if let Some(slot) = hash_map.get(&key) {
                hash_checksum = hash_checksum.wrapping_add(slot.node_id);
            }
        }
        measure("hash_map     ", &lookup_keys, started.elapsed().as_nanos());
        let started = Instant::now();
        let mut paged10_checksum = 0u64;
        for &key in &lookup_keys {
            if let Some(slot) = paged_10.get(&key) {
                paged10_checksum = paged10_checksum.wrapping_add(slot.node_id);
            }
        }
        measure("paged_10_bits", &lookup_keys, started.elapsed().as_nanos());
        let started = Instant::now();
        let mut paged12_checksum = 0u64;
        for &key in &lookup_keys {
            if let Some(slot) = paged_12.get(&key) {
                paged12_checksum = paged12_checksum.wrapping_add(slot.node_id);
            }
        }
        measure("paged_12_bits", &lookup_keys, started.elapsed().as_nanos());
        assert_eq!(hash_checksum, paged10_checksum);
        assert_eq!(hash_checksum, paged12_checksum);
    }

    fn runtime() -> NativeViewRuntime {
        NativeViewRuntime::new()
    }

    #[test]
    fn t12_stale_child_status_detail_precedes_parent_publication() {
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let stale_child = 0x7fff_fe00;
        let result = unsafe {
            generated_exports::iyon_view_row_create_1_v1(pointer, 91, 0, 0, 0, stale_child)
        };
        assert_eq!(result, FAST_CACHE_MISS);
        assert_eq!(
            unsafe { generated_exports::iyon_view_status_detail_v1(pointer) },
            STATUS_DETAIL_CHILD_INDEX
        );
        assert_eq!(runtime.ref_for_node_id(91), Err(FAST_CACHE_MISS));
        assert_eq!(runtime.slots.len(), 0);
    }

    #[test]
    fn generated_spacer_publish_lookup_and_release_share_the_semantic_cache() {
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let reference = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 7, 0, 2) };
        assert!(reference < 0x8000_0000);
        assert_eq!(
            unsafe { generated_exports::iyon_view_render_ref_v1(pointer, reference) },
            reference
        );
        assert_eq!(
            unsafe { generated_exports::iyon_view_ref_for_node_id_v1(pointer, 7, 0) },
            reference
        );
        assert_eq!(
            unsafe { generated_exports::iyon_view_release_many_v1(pointer, &reference, 4, 1) },
            1
        );
        assert_eq!(
            unsafe { generated_exports::iyon_view_release_many_v1(pointer, &reference, 4, 1) },
            1
        );
        assert_eq!(
            unsafe { generated_exports::iyon_view_render_ref_v1(pointer, reference) },
            FAST_CACHE_MISS
        );
    }

    #[test]
    fn bulk_publication_reuses_the_environment_native_ref_table() {
        let mut runtime = runtime();
        let view = View::spacer(3);
        let bulk_ref = runtime.publish_bulk(41, view.clone()).expect("bulk ref");
        assert_eq!(runtime.ref_for_node_id(41), Ok(bulk_ref));
        assert_eq!(runtime.resolve_ref(bulk_ref), Ok((view, true)));
    }

    #[test]
    fn generated_text_string_variants_preserve_unicode_and_embedded_nul() {
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let cstring = CString::new("héllo ✓").expect("cstring text has no NUL");
        let cstring_ref = unsafe {
            generated_exports::iyon_view_text_create_cstring_v1(
                pointer,
                501,
                0,
                cstring.as_ptr(),
                0,
                1,
                1,
            )
        };
        let expected_cstring = View::styled_text([TextSpan::plain("héllo ✓")]).into_view();
        assert_eq!(
            runtime.resolve_ref(cstring_ref).map(|(view, _)| view),
            Ok(expected_cstring)
        );

        let bytes = b"left\0right";
        let buffer_ref = unsafe {
            generated_exports::iyon_view_text_create_utf8_v1(
                pointer,
                502,
                0,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
                0,
                1,
                1,
            )
        };
        let expected_buffer = View::styled_text([TextSpan::plain("left\0right")]).into_view();
        assert_eq!(
            runtime.resolve_ref(buffer_ref).map(|(view, _)| view),
            Ok(expected_buffer)
        );

        let spans = [b"left\0".as_slice(), "right\0✓".as_bytes()].concat();
        let multi_ref = unsafe {
            generated_exports::iyon_view_text_create_utf8_2_v1(
                pointer,
                503,
                0,
                spans.as_ptr(),
                spans.len(),
                spans.len() as u32,
                5,
                0,
                (spans.len() - 5) as u32,
                0,
                1,
                1,
            )
        };
        let expected_multi =
            View::styled_text([TextSpan::plain("left\0"), TextSpan::plain("right\0✓")]).into_view();
        assert_eq!(
            runtime.resolve_ref(multi_ref).map(|(view, _)| view),
            Ok(expected_multi)
        );
    }

    #[test]
    fn native_axis_builders_and_small_constructors_publish_immutable_views() {
        let mut runtime = runtime();
        let child_a = View::spacer(1);
        let child_b = View::spacer(2);
        let child_a_ref = runtime.publish_bulk(1, child_a.clone()).expect("child a");
        let child_b_ref = runtime.publish_bulk(2, child_b.clone()).expect("child b");
        let pointer = &mut runtime as *mut NativeViewRuntime;

        let builder = unsafe { generated_exports::iyon_axis_builder_begin_v1(pointer, 2, 2) };
        assert!(is_valid_builder_ref(builder));
        assert_eq!(
            unsafe {
                generated_exports::iyon_axis_builder_push_v1(pointer, builder, 0, child_a_ref)
            },
            0
        );
        assert_eq!(
            unsafe {
                generated_exports::iyon_axis_builder_push_v1(pointer, builder, 0, child_b_ref)
            },
            0
        );
        let built_ref =
            unsafe { generated_exports::iyon_axis_builder_finish_v1(pointer, builder, 3, 0, 1) };
        let expected = View::native_axis_from_children(false, 1, vec![(0, child_a), (0, child_b)])
            .expect("expected native axis");
        assert_eq!(
            runtime.resolve_ref(built_ref).map(|(view, _)| view),
            Ok(expected)
        );

        let small_ref = unsafe {
            generated_exports::iyon_view_row_create_1_v1(pointer, 4, 0, 0, 0, child_a_ref)
        };
        assert!(runtime.resolve_ref(small_ref).is_ok());
        assert_eq!(
            unsafe { generated_exports::iyon_axis_builder_abort_v1(pointer, builder) },
            1
        );
    }

    // --- PERF-12 §111 native slot lease invariants -----------------------------

    fn lease_count(runtime: &NativeViewRuntime, reference: u32) -> Option<u32> {
        runtime
            .slots
            .get(&reference)
            .map(|slot| slot.js_lease_count)
    }

    #[test]
    fn new_constructor_returns_lease_count_one() {
        let mut runtime = runtime();
        let reference = runtime.publish(11, View::spacer(2)).expect("publish");
        assert_eq!(lease_count(&runtime, reference), Some(1));
        assert!(runtime.resolve_ref(reference).is_ok());
    }

    #[test]
    fn child_temp_lease_stays_live_until_root_completes() {
        let mut runtime = runtime();
        let child_view = View::spacer(1);
        let child_ref = runtime.publish(21, child_view.clone()).expect("child");
        // Parent construction resolves the child ref safely while the temp
        // lease is still held.
        assert!(runtime.resolve_ref(child_ref).is_ok());
        let parent_view = View::spacer(3);
        let parent_ref = runtime.publish(22, parent_view).expect("parent");
        // The root now exists; releasing the child temp lease leaves the child
        // View alive through the parent's strong ownership (unleased-live).
        assert_eq!(runtime.release_many(&child_ref, 1), Ok(1));
        assert_eq!(lease_count(&runtime, child_ref), Some(0));
        assert!(runtime.resolve_ref(child_ref).is_ok());
        assert_eq!(lease_count(&runtime, parent_ref), Some(1));
    }

    #[test]
    fn batch_release_drops_child_temp_leases() {
        let mut runtime = runtime();
        let refs = [
            runtime.publish(31, View::spacer(1)).expect("a"),
            runtime.publish(32, View::spacer(1)).expect("b"),
            runtime.publish(33, View::spacer(1)).expect("c"),
        ];
        for reference in refs {
            scratch_release(&mut runtime, &[reference]);
        }
        // No other owner holds these Views, so the release path removes the
        // slots outright; with a live owner they would drop to lease 0 instead
        // (see child_temp_lease_stays_live_until_root_completes).
        for reference in refs {
            assert!(runtime.slots.get(&reference).is_none());
        }
    }

    fn scratch_release(runtime: &mut NativeViewRuntime, refs: &[u32]) {
        let mut buffer = [0u32; 16];
        for (index, reference) in refs.iter().enumerate() {
            buffer[index] = *reference;
        }
        assert_eq!(
            runtime.release_many(buffer.as_ptr(), refs.len() as u32),
            Ok(refs.len() as i32)
        );
    }

    #[test]
    fn root_lease_transfers_to_boundary_after_new_install() {
        let mut runtime = runtime();
        let old_root = runtime.publish(41, View::spacer(1)).expect("old root");
        // Boundary protocol (§18): keep previousRef leased, materialize the
        // next root, acquire its boundary lease, then release the old root.
        let new_root = runtime.publish(42, View::spacer(2)).expect("new root");
        runtime
            .ensure_lease(new_root, View::spacer(2))
            .expect("boundary lease");
        assert_eq!(runtime.release_many(&old_root, 1), Ok(1));
        // The old root's only owner was the boundary lease, so its slot is
        // fully reclaimed; the new root keeps exactly the boundary lease.
        assert!(runtime.slots.get(&old_root).is_none());
        assert_eq!(lease_count(&runtime, new_root), Some(1));
    }

    #[test]
    fn failed_host_install_retains_old_root() {
        let mut runtime = runtime();
        let old_root = runtime.publish(51, View::spacer(1)).expect("old root");
        // A failed host install releases only the failed candidate; the old
        // boundary lease must be untouched.
        let candidate = runtime.publish(52, View::spacer(2)).expect("candidate");
        assert_eq!(runtime.release_many(&candidate, 1), Ok(1));
        assert_eq!(lease_count(&runtime, old_root), Some(1));
        assert!(runtime.resolve_ref(old_root).is_ok());
    }

    #[test]
    fn failed_transaction_releases_every_new_temp_lease() {
        let mut runtime = runtime();
        let temps = [
            runtime.publish(61, View::spacer(1)).expect("t1"),
            runtime
                .publish(62, View::text("failed-tx").into_view())
                .expect("t2"),
            runtime.publish(63, View::spacer(4)).expect("t3"),
        ];
        scratch_release(&mut runtime, &temps);
        for reference in temps {
            assert!(runtime.slots.get(&reference).is_none());
        }
    }

    #[test]
    fn stale_unleased_weak_slot_returns_cache_miss() {
        let mut runtime = runtime();
        let view = View::spacer(1);
        let reference = runtime.publish_bulk(71, view.clone()).expect("bulk");
        drop(view); // no lease owns it; the weak entry is now stale metadata
        assert_eq!(runtime.resolve_ref(reference), Err(FAST_CACHE_MISS));
        assert!(runtime.slots.get(&reference).is_none());
    }

    #[test]
    fn slot_metadata_scavenged_after_weak_expiry() {
        let mut runtime = runtime();
        // Live-but-unleased slot whose View later expires: release enqueues a
        // scavenge candidate and bounded maintenance reclaims the metadata.
        let view = View::spacer(1);
        let live_reference = runtime.publish_bulk(81, view.clone()).expect("live bulk");
        assert_eq!(runtime.release_many(&live_reference, 1), Ok(1));
        // Still alive at release time: the slot stays as unleased-live state
        // and the ref sits in the scavenge queue.
        assert!(runtime.slots.get(&live_reference).is_some());
        assert_eq!(runtime.scavenge_queue.len(), 1);
        drop(view);
        // The next maintenance pass reclaims the expired metadata (§55).
        runtime.maintain(false);
        assert!(runtime.slots.get(&live_reference).is_none());
        assert!(runtime.scavenge_processed >= 1);
        assert_eq!(runtime.node_refs.len(), 0);

        // Transient churn self-cleans slots inline on release; the expired
        // weak-cache entries remain as bounded slack until the full sweep
        // removes them (§55 invariant demonstrated end to end).
        for id in 82..82 + 64u64 {
            let transient = View::spacer(1);
            let reference = runtime.publish(id, transient.clone()).expect("transient");
            drop(transient);
            assert_eq!(runtime.release_many(&reference, 1), Ok(1));
        }
        assert_eq!(runtime.slots.len(), 0);
        // 64 transient entries plus the expired bulk entry from node 81 whose
        // slot was reclaimed by the bounded pass but whose weak-cache entry
        // waits for the full sweep: exactly the "bounded slack" model.
        assert_eq!(runtime.nodes.len(), 65);
        assert_eq!(runtime.node_refs.len(), 0);
        runtime.prune_expired();
        assert!(runtime.semantic_cache_full_sweeps >= 1);
        assert!(runtime.semantic_cache_entries_removed >= 64);
        assert_eq!(runtime.nodes.len(), 0);
        assert_eq!(runtime.node_refs.len(), 0);
    }

    #[test]
    fn repeated_node_id_lookups_acquire_independent_leases() {
        let mut runtime = runtime();
        let view = View::spacer(3);
        let reference = runtime.publish_bulk(41, view.clone()).expect("bulk ref");
        assert_eq!(runtime.ref_for_node_id(41), Ok(reference));
        assert_eq!(runtime.ref_for_node_id(41), Ok(reference));
        assert_eq!(
            runtime
                .slots
                .get(&reference)
                .map(|slot| slot.js_lease_count),
            Some(2)
        );

        assert_eq!(runtime.release_many(&reference, 1), Ok(1));
        assert_eq!(
            runtime
                .slots
                .get(&reference)
                .map(|slot| slot.js_lease_count),
            Some(1)
        );
        assert_eq!(runtime.release_many(&reference, 1), Ok(1));
        assert_eq!(
            runtime
                .slots
                .get(&reference)
                .map(|slot| slot.js_lease_count),
            Some(0)
        );
    }

    #[test]
    fn generated_text_and_common_patches_publish_new_node_ids() {
        let mut runtime = runtime();
        let base = runtime
            .publish(1, View::text("hello").into_view())
            .expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let patched = unsafe {
            generated_exports::iyon_view_text_layout_patch_root_v1(pointer, base, 2, 0, 3, 2)
        };
        assert!(patched < 0x8000_0000);
        let common = unsafe {
            generated_exports::iyon_view_common_patch_root_v1(
                pointer, patched, 3, 0, 32, 0, 0, 0, 0, 4, 20, 0, 24, base,
            )
        };
        assert!(common < 0x8000_0000);
        assert_ne!(base, patched);
        assert_ne!(patched, common);
    }

    #[test]
    fn path_refs_are_interned_and_depth_specialization_rebuilds_only_the_path() {
        let mut runtime = runtime();
        let base_view = View::vertical(|column| {
            column.child(View::text("hello"));
        })
        .into_view();
        let base = runtime.publish(1, base_view).expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let root = unsafe { generated_exports::iyon_path_root_v1(pointer) };
        let path = unsafe { generated_exports::iyon_path_child_v1(pointer, root, 4, 3, 0) };
        assert_eq!(
            unsafe { generated_exports::iyon_path_child_v1(pointer, root, 4, 3, 0) },
            path
        );
        let patched = unsafe {
            generated_exports::iyon_view_text_layout_patch_path_d1_v1(
                pointer, base, path, 2, 0, 3, 0, 3, 2,
            )
        };
        assert!(patched < 0x8000_0000);
        assert_ne!(patched, base);
        assert!(
            runtime
                .nodes
                .get(&2)
                .and_then(iyon_tui::WeakView::upgrade)
                .is_some()
        );
        assert!(
            runtime
                .nodes
                .get(&3)
                .and_then(iyon_tui::WeakView::upgrade)
                .is_some()
        );
        assert!(runtime.resolve_ref(patched).is_ok());
        let generic = unsafe {
            generated_exports::iyon_view_text_layout_patch_path_v1(
                pointer, base, path, 1, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 3, 2,
            )
        };
        assert!(generic < 0x8000_0000);
        assert!(
            runtime
                .nodes
                .get(&4)
                .and_then(iyon_tui::WeakView::upgrade)
                .is_some()
        );
        assert!(
            runtime
                .nodes
                .get(&5)
                .and_then(iyon_tui::WeakView::upgrade)
                .is_some()
        );
    }

    #[test]
    fn stale_path_base_returns_cache_miss_then_recovers_once() {
        let mut runtime = runtime();
        let base = runtime
            .publish(
                1,
                View::vertical(|column| {
                    column.child(View::text("hello"));
                })
                .into_view(),
            )
            .expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let root = unsafe { generated_exports::iyon_path_root_v1(pointer) };
        let path = unsafe { generated_exports::iyon_path_child_v1(pointer, root, 4, 3, 0) };
        assert_eq!(runtime.release_many(&base, 1), Ok(1));
        assert_eq!(
            unsafe {
                generated_exports::iyon_view_text_layout_patch_path_d1_v1(
                    pointer, base, path, 2, 0, 3, 0, 3, 2,
                )
            },
            FAST_CACHE_MISS
        );
        let recovered = runtime
            .publish(
                1,
                View::vertical(|column| {
                    column.child(View::text("hello"));
                })
                .into_view(),
            )
            .expect("recovered base ref");
        let patched = unsafe {
            generated_exports::iyon_view_text_layout_patch_path_d1_v1(
                pointer, recovered, path, 2, 0, 3, 0, 3, 2,
            )
        };
        assert!(patched < 0x8000_0000);
        assert!(runtime.nodes.contains_key(&2));
        assert!(runtime.nodes.contains_key(&3));
    }

    #[test]
    fn edit_transaction_builds_one_shared_ancestor_for_two_text_edits() {
        let mut runtime = runtime();
        let base_view = View::vertical(|column| {
            column.child(View::text("left"));
            column.child(View::text("right"));
        })
        .into_view();
        let base = runtime.publish(1, base_view).expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let path_root = unsafe { generated_exports::iyon_path_root_v1(pointer) };
        let path0 = unsafe { generated_exports::iyon_path_child_v1(pointer, path_root, 4, 3, 0) };
        let path1 = unsafe { generated_exports::iyon_path_child_v1(pointer, path_root, 4, 3, 1) };
        let txn = unsafe { generated_exports::iyon_edit_txn_begin_v1(pointer, base, 2) };
        assert!(is_valid_edit_txn_ref(txn));
        assert_eq!(
            unsafe {
                generated_exports::iyon_edit_txn_add_text_layout_v1(
                    pointer, txn, path0, 1, 11, 0, 21, 0, 21, 0, 21, 0, 21, 0, 3, 2,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                generated_exports::iyon_edit_txn_add_text_layout_v1(
                    pointer, txn, path1, 1, 12, 0, 21, 0, 21, 0, 21, 0, 21, 0, 3, 2,
                )
            },
            0
        );
        let transaction = runtime.edit_txns.get(&txn).expect("transaction");
        let trie = runtime.build_edit_trie(transaction).expect("trie");
        assert_eq!(trie.len(), 3, "root plus two changed leaves");
        let mut staged = Vec::new();
        let root = runtime
            .stage_edit_trie(transaction.base_view.clone(), &trie, 0, &mut staged)
            .expect("staged root");
        assert_eq!(staged.len(), 3, "shared root is rebuilt once");
        assert_eq!(staged.last().map(|(_, view)| view), Some(&root));
        assert!(staged.iter().any(|(id, _)| *id == 11));
        assert!(staged.iter().any(|(id, _)| *id == 12));
        assert_eq!(staged.last().map(|(id, _)| *id), Some(21));
    }

    #[test]
    fn edit_transaction_abort_and_limits_leave_no_staged_state() {
        let mut runtime = runtime();
        let base = runtime
            .publish(1, View::text("base").into_view())
            .expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        assert_eq!(
            unsafe { generated_exports::iyon_edit_txn_begin_v1(pointer, base, 0) },
            FAST_INVALID
        );
        assert_eq!(
            unsafe { generated_exports::iyon_edit_txn_begin_v1(pointer, base, MAX_EDIT_COUNT + 1) },
            FAST_INVALID
        );
        let txn = unsafe { generated_exports::iyon_edit_txn_begin_v1(pointer, base, 1) };
        assert!(is_valid_edit_txn_ref(txn));
        assert_eq!(
            unsafe { generated_exports::iyon_edit_txn_abort_v1(pointer, txn) },
            0
        );
        assert!(!runtime.edit_txns.contains_key(&txn));
        assert_eq!(
            unsafe { generated_exports::iyon_edit_txn_abort_v1(pointer, txn) },
            1
        );
        assert_eq!(
            unsafe { generated_exports::iyon_edit_txn_begin_v1(pointer, PATH_ROOT_REF, 1) },
            FAST_INVALID
        );
    }

    #[test]
    fn generated_axis_and_grid_edits_copy_persistent_sequences() {
        let mut runtime = runtime();
        let axis = View::vertical(|column| {
            for index in 0..2_048 {
                column.child(View::text(format!("axis-{index}")));
            }
        })
        .into_view();
        let child = View::text("replacement").into_view();
        let base_axis = runtime.publish(1, axis.clone()).expect("axis base ref");
        let child_ref = runtime.publish(2, child.clone()).expect("child ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let replaced = unsafe {
            generated_exports::iyon_view_axis_set_child_v1(
                pointer, base_axis, 3, 0, 1_337, 0, child_ref,
            )
        };
        assert!(replaced < 0x8000_0000);
        assert_eq!(runtime.resolve_ref(base_axis), Ok((axis, true)));

        let inserted = [AxisChildInputV1 {
            track_word: 0,
            child_ref,
        }];
        let spliced = unsafe {
            generated_exports::iyon_view_axis_splice_buffer_v1(
                pointer,
                base_axis,
                4,
                0,
                1_000,
                0,
                inserted.as_ptr(),
                core::mem::size_of_val(&inserted),
                1,
            )
        };
        assert!(spliced < 0x8000_0000);

        let grid = View::grid(|grid| {
            grid.columns([GridTrack::fixed(12)]);
            grid.row(|row| {
                row.cell(View::text("grid-cell"));
            });
        })
        .into_view();
        let base_grid = runtime.publish(5, grid.clone()).expect("grid base ref");
        let grid_replaced = unsafe {
            generated_exports::iyon_view_grid_set_cell_v1(pointer, base_grid, 6, 0, 0, 0, child_ref)
        };
        assert!(grid_replaced < 0x8000_0000);
        assert_eq!(runtime.resolve_ref(base_grid), Ok((grid, true)));

        let path_grid_replaced = unsafe {
            generated_exports::iyon_view_grid_set_cell_path_v1(
                pointer,
                base_grid,
                PATH_ROOT_REF,
                0,
                8,
                0,
                1,
                0,
                1,
                0,
                1,
                0,
                1,
                0,
                0,
                0,
                child_ref,
            )
        };
        assert!(path_grid_replaced < 0x8000_0000);

        let path_root = unsafe { generated_exports::iyon_path_root_v1(pointer) };
        let path_replaced = unsafe {
            generated_exports::iyon_view_axis_set_child_path_v1(
                pointer, base_axis, path_root, 0, 7, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1_000, 0, child_ref,
            )
        };
        assert!(path_replaced < 0x8000_0000);
    }

    #[test]
    fn axis_buffer_rejects_count_larger_than_buffer_bytes_perf12_t8() {
        // PERF-12 §68/§116: used_child_count must fit inside the actual
        // borrowed buffer length; a mismatched count is rejected before any
        // dereference instead of reading out of bounds.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let child = runtime
            .publish(500_000, View::spacer(1))
            .expect("child ref");
        let mut scratch = [0u32; 8];
        scratch[1] = child;
        scratch[3] = child;
        let children = scratch.as_ptr() as *const AxisChildInputV1;
        let result = unsafe {
            generated_exports::iyon_view_axis_create_buffer_v1(
                pointer, 500_001, 0, 2, 0, children, 16, 4,
            )
        };
        assert!(result >= 0x8000_0000);
        assert!(!runtime.nodes.contains_key(&500_001));
        // A matching count on the same buffer shape still validates.
        let ok = unsafe {
            generated_exports::iyon_view_axis_create_buffer_v1(
                pointer, 500_002, 0, 2, 0, children, 16, 2,
            )
        };
        assert!(ok < 0x8000_0000);
    }

    #[test]
    fn grid_create_buffer_builds_and_consults_cache_perf12_t10() {
        // PERF-12 §36: a new grid materializes through one borrowed word
        // buffer; a live NodeId short-circuits without parsing (§23).
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let child_a = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 600, 0, 1) };
        let child_b = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 601, 0, 2) };
        assert!(child_a < 0x8000_0000 && child_b < 0x8000_0000);
        // One column track (content), one row (content track) with two cells.
        let words: [u32; 11] = [
            1,
            GRID_TRACK_CONTENT_WORD,
            1,
            GRID_TRACK_CONTENT_WORD,
            2,
            child_a,
            1 | (1 << 16),
            1 | (1 << 16),
            child_b,
            2 | (1 << 16),
            3 | (2 << 16),
        ];
        let grid = unsafe {
            generated_exports::iyon_view_grid_create_buffer_v1(
                pointer,
                700,
                0,
                1,
                2,
                words.as_ptr(),
                words.len() * 4,
                words.len() as u32,
            )
        };
        assert!(grid < 0x8000_0000);
        assert_eq!(grid, unsafe {
            generated_exports::iyon_view_ref_for_node_id_v1(pointer, 700, 0)
        });
        // §23 consult: same NodeId with garbage words must return the cache.
        let again = unsafe {
            generated_exports::iyon_view_grid_create_buffer_v1(
                pointer,
                700,
                0,
                9,
                9,
                words.as_ptr(),
                8,
                2,
            )
        };
        assert_eq!(again, grid);
        // Truncated buffer must be rejected before construction.
        let truncated = unsafe {
            generated_exports::iyon_view_grid_create_buffer_v1(
                pointer,
                701,
                0,
                1,
                1,
                words.as_ptr(),
                40,
                10,
            )
        };
        assert_eq!(truncated, FAST_INVALID);
        // Amount-bearing bits on a marker-only track are malformed, not
        // silently discarded by the packed parser.
        let malformed_track = [1, GRID_TRACK_CONTENT_WORD | (1 << 8), 0];
        let malformed = unsafe {
            generated_exports::iyon_view_grid_create_buffer_v1(
                pointer,
                702,
                0,
                0,
                0,
                malformed_track.as_ptr(),
                malformed_track.len() * 4,
                malformed_track.len() as u32,
            )
        };
        assert_eq!(malformed, FAST_INVALID);
    }

    #[test]
    fn text_constructors_consult_semantic_cache_first_perf12_t11() {
        // PERF-12 §23 on the T11 payload lanes: a live NodeId returns its
        // cached ref without parsing any text payload, on both the cstring
        // and exact-byte families.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let original = unsafe {
            generated_exports::iyon_view_text_create_cstring_v1(
                pointer,
                850,
                0,
                c"styled text".as_ptr() as *const std::ffi::c_char,
                0,
                1,
                1,
            )
        };
        assert!(original < 0x8000_0000);
        // Same NodeId with garbage payload arguments must return the cache.
        let again = unsafe {
            generated_exports::iyon_view_text_create_cstring_v1(
                pointer,
                850,
                0,
                c"different".as_ptr() as *const std::ffi::c_char,
                0,
                3,
                2,
            )
        };
        assert_eq!(again, original);
        let bytes = b"\xf0\x90\x80\x80"; // U+10000
        let utf8_again = unsafe {
            generated_exports::iyon_view_text_create_utf8_v1(
                pointer,
                850,
                0,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
                0,
                1,
                1,
            )
        };
        assert_eq!(utf8_again, original);
    }

    #[test]
    fn diff_create_buffer_builds_validates_and_consults_cache_perf12_t11() {
        // PERF-12 §41: a new diff materializes through one borrowed
        // words+bytes call and validates semantic coordinates exactly like
        // the Direct decoder; a live NodeId short-circuits without parsing.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        // Hunk: -1,2 +1,2 with context/deletion/unterminated-addition.
        // Words: [hunk_count, old_start(lo,hi), old_count(lo,hi),
        //         new_start(lo,hi), new_count(lo,hi), line_count,
        //   per line: meta, old(lo,hi), new(lo,hi), byte_length]
        const DELETION: u32 = 3 | (1 << 16);
        const ADDITION_UNTERMINATED: u32 = 2 | (2 << 16);
        const CONTEXT: u32 = 1 | (1 << 16);
        let words: [u32; 28] = [
            1,
            0,
            0,
            2,
            0, // old range start 0 count 2
            0,
            0,
            2,
            0, // new range start 0 count 2
            3,
            CONTEXT,
            1,
            0,
            1,
            0,
            4, // "same"
            DELETION,
            2,
            0,
            0,
            0,
            7, // "removed"
            ADDITION_UNTERMINATED,
            0,
            0,
            2,
            0,
            5, // "added"
        ];
        let bytes = b"sameremovedadded";
        assert_eq!(
            7 + 5 + 4,
            bytes.len(),
            "byte lengths must frame the payload exactly"
        );
        let diff = unsafe {
            generated_exports::iyon_view_diff_create_buffer_v1(
                pointer,
                800,
                0,
                words.as_ptr(),
                words.len() * 4,
                words.len() as u32,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
            )
        };
        assert!(diff < 0x8000_0000);
        assert_eq!(diff, unsafe {
            generated_exports::iyon_view_ref_for_node_id_v1(pointer, 800, 0)
        });
        // §23 consult: same NodeId with garbage payload returns the cache.
        let again = unsafe {
            generated_exports::iyon_view_diff_create_buffer_v1(
                pointer,
                800,
                0,
                words.as_ptr(),
                8,
                2,
                bytes.as_ptr(),
                0,
                0,
            )
        };
        assert_eq!(again, diff);
        // Truncated word buffer must be rejected before construction.
        let truncated = unsafe {
            generated_exports::iyon_view_diff_create_buffer_v1(
                pointer,
                801,
                0,
                words.as_ptr(),
                words.len() * 4,
                (words.len() - 1) as u32,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
            )
        };
        assert_eq!(truncated, FAST_INVALID);
        // Coordinate mismatch against the declared ranges is invalid exactly
        // like Direct's DiffHunk validation.
        let mut mismatched = words;
        mismatched[11] = 5; // context claims old line 5, hunk expects 1
        let mismatch = unsafe {
            generated_exports::iyon_view_diff_create_buffer_v1(
                pointer,
                802,
                0,
                mismatched.as_ptr(),
                mismatched.len() * 4,
                mismatched.len() as u32,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
            )
        };
        assert_eq!(mismatch, FAST_INVALID);
        // Byte length overflow past the buffer is rejected.
        let mut overlong = words;
        overlong[11] = u32::MAX;
        let overlong_status = unsafe {
            generated_exports::iyon_view_diff_create_buffer_v1(
                pointer,
                803,
                0,
                overlong.as_ptr(),
                overlong.len() * 4,
                overlong.len() as u32,
                bytes.as_ptr(),
                bytes.len(),
                bytes.len() as u32,
            )
        };
        assert_eq!(overlong_status, FAST_INVALID);
    }

    #[test]
    fn axis_and_grid_edits_consult_semantic_cache_first_perf12_t10() {
        // PERF-12 §23 on the T10 edit primitives: a live NodeId returns the
        // cached ref without resolving the base or child refs.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let base_child =
            unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 800, 0, 1) };
        let child = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 801, 0, 1) };
        let base = unsafe {
            generated_exports::iyon_view_column_create_1_v1(pointer, 802, 0, 0, 0, base_child)
        };
        let edited = unsafe {
            generated_exports::iyon_view_axis_set_child_v1(pointer, base, 900, 0, 0, 0, child)
        };
        assert!(edited < 0x8000_0000);
        // Live NodeId + stale base and child: consult must win.
        let again = unsafe {
            generated_exports::iyon_view_axis_set_child_v1(
                pointer,
                0x7fff_fe00,
                900,
                0,
                5,
                5,
                0x7fff_fe01,
            )
        };
        assert_eq!(again, edited);
        // Splice path: live NodeId wins over a stale base before any parse.
        let spliced_again = unsafe {
            generated_exports::iyon_view_axis_splice_buffer_v1(
                pointer,
                0x7fff_fe02,
                900,
                0,
                0,
                0,
                std::ptr::null(),
                0,
                0,
            )
        };
        assert_eq!(spliced_again, edited);
        // Grid path: build a real grid, then re-request via stale args.
        let cell = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 802, 0, 1) };
        let words: [u32; 8] = [
            1,
            GRID_TRACK_CONTENT_WORD,
            1,
            GRID_TRACK_CONTENT_WORD,
            1,
            cell,
            1 | (1 << 16),
            1 | (1 << 16),
        ];
        let grid = unsafe {
            generated_exports::iyon_view_grid_create_buffer_v1(
                pointer,
                950,
                0,
                0,
                0,
                words.as_ptr(),
                words.len() * 4,
                words.len() as u32,
            )
        };
        assert!(grid < 0x8000_0000);
        let cell_edit = unsafe {
            generated_exports::iyon_view_grid_set_cell_v1(
                pointer,
                0x7fff_fe03,
                950,
                0,
                9,
                9,
                0x7fff_fe04,
            )
        };
        assert_eq!(cell_edit, grid);
    }

    #[test]
    fn text_layout_patch_consults_semantic_cache_and_clones_payload_perf12_t9() {
        // PERF-12 §23/§38: a wrap/align-only patch clones the base's retained
        // text payload under the new NodeId, and a live NodeId short-circuits
        // without resolving the base ref.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let base = unsafe {
            generated_exports::iyon_view_text_create_cstring_v1(
                pointer,
                100,
                0,
                c"hello".as_ptr(),
                0,
                1,
                1,
            )
        };
        assert!(base < 0x8000_0000);
        // WrapMode::Grapheme = 2, HorizontalAlign::Center = 2 (schema codes).
        let patched = unsafe {
            generated_exports::iyon_view_text_layout_patch_root_v1(pointer, base, 101, 0, 2, 2)
        };
        assert!(patched < 0x8000_0000);
        assert_ne!(patched, base);
        assert_eq!(patched, unsafe {
            generated_exports::iyon_view_ref_for_node_id_v1(pointer, 101, 0)
        });
        // Cache-first consult: same NodeId with a stale base must return the
        // cached ref instead of failing on the base resolution.
        let again = unsafe {
            generated_exports::iyon_view_text_layout_patch_root_v1(
                pointer,
                0x7fff_ff00,
                101,
                0,
                3,
                3,
            )
        };
        assert_eq!(again, patched);
    }

    #[test]
    fn common_patch_applies_scalar_mask_and_accepts_absent_decoration_ref_perf12_t9() {
        // PERF-12 T9: the common scalar patch applies padding + width to the
        // base view; decoration_ref = 0 means absent and must not fail.
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let base = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 200, 0, 1) };
        assert!(base < 0x8000_0000);
        // PATCH_PADDING = 4 | PATCH_WIDTH = 8; padding_tr packs top|right<<16.
        const MASK: u32 = 4 | 8;
        let patched = unsafe {
            generated_exports::iyon_view_common_patch_root_v1(
                pointer,
                base,
                201,
                0,
                MASK,
                3 | (4 << 16),
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        };
        assert!(patched < 0x8000_0000);
        assert_eq!(patched, unsafe {
            generated_exports::iyon_view_ref_for_node_id_v1(pointer, 201, 0)
        });
        // Cache-first consult: live NodeId wins over a stale base ref.
        let again = unsafe {
            generated_exports::iyon_view_common_patch_root_v1(
                pointer,
                0x7fff_ff00,
                201,
                0,
                MASK,
                0,
                0,
                2,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        };
        assert_eq!(again, patched);
        // An empty mask stays invalid.
        let invalid = unsafe {
            generated_exports::iyon_view_common_patch_root_v1(
                pointer, base, 202, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            )
        };
        assert_eq!(invalid, FAST_INVALID);
    }

    #[test]
    fn constructor_consults_semantic_cache_before_building_perf12_s23() {
        // PERF-12 §23: a live NodeId short-circuits construction, returning
        // the cached ref without consuming child refs (which may be stale).
        let mut runtime = runtime();
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let first = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 7, 0, 3) };
        assert!(first < 0x8000_0000);
        // Same NodeId, different payload and a stale child ref on the axis:
        // the cache-first consult must win before either is inspected.
        let again = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 7, 0, 9) };
        assert_eq!(again, first);
        // Axis path: build a column normally, then re-request the same
        // NodeId through a stale child ref - cache-first must win.
        let child = unsafe { generated_exports::iyon_view_spacer_create_v1(pointer, 8, 0, 1) };
        assert!(child < 0x8000_0000);
        let built =
            unsafe { generated_exports::iyon_view_column_create_1_v1(pointer, 9, 0, 0, 0, child) };
        assert!(built < 0x8000_0000);
        let recovered = unsafe {
            generated_exports::iyon_view_column_create_1_v1(pointer, 9, 0, 0, 0, 0x7fff_ff00)
        };
        assert_eq!(recovered, built);
        assert_eq!(built, unsafe {
            generated_exports::iyon_view_ref_for_node_id_v1(pointer, 9, 0)
        });
    }

    #[test]
    fn path_validation_rejects_wrong_parent_kind_and_preserves_publication() {
        let mut runtime = runtime();
        let base = runtime
            .publish(
                1,
                View::vertical(|column| {
                    column.child(View::text("hello"));
                })
                .into_view(),
            )
            .expect("base ref");
        let pointer = &mut runtime as *mut NativeViewRuntime;
        let root = unsafe { generated_exports::iyon_path_root_v1(pointer) };
        let path = unsafe { generated_exports::iyon_path_child_v1(pointer, root, 4, 3, 0) };
        let invalid = unsafe {
            generated_exports::iyon_view_text_layout_patch_path_d1_v1(
                pointer, base, path, 9, 0, 1, 0, 3, 2,
            )
        };
        assert!(invalid >= 0x8000_0000);
        assert!(!runtime.nodes.contains_key(&9));
        assert_eq!(
            unsafe { generated_exports::iyon_path_child_v1(pointer, root, 4, 1, 0) },
            FAST_INVALID
        );
        assert!(base < super::PATH_ROOT_REF);
        assert_eq!(
            unsafe { generated_exports::iyon_path_child_v1(pointer, base, 4, 3, 0) },
            FAST_INVALID
        );
        assert_eq!(
            unsafe {
                generated_exports::iyon_view_text_layout_patch_path_d1_v1(
                    pointer, base, base, 2, 0, 3, 0, 3, 2,
                )
            },
            FAST_INVALID
        );
        assert_eq!(
            runtime
                .status
                .code
                .load(std::sync::atomic::Ordering::Acquire),
            FAST_INVALID
        );
    }
}
