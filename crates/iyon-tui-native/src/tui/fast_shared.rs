use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, ThreadId};

use iyon_tui::{
    BorderEdges, BorderGlyphs, BorderSpec, ColorSpec, DiffHunk, DiffLine, DiffLineNumber,
    DiffLineOffset, DiffLineTermination, DiffRange, GridTrack, HorizontalAlign, Insets, Renderer,
    RetainedAxis, RetainedAxisChild, RetainedAxisTrack, RetainedDecoration, RetainedGridCell,
    RetainedGridCells, RetainedSizeRule, SharedUtf8Source, StyleRef, StyleSpec, TextAttribute,
    TextSpan, VerticalAlign, View, WrapMode,
};

macro_rules! fast_perf_inc {
    ($counter:ident) => {
        #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::$counter);
    };
}

#[allow(unused_macros)]
macro_rules! fast_perf_set {
    ($counter:ident, $value:expr) => {
        #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
        iyon_tui::perf::set(iyon_tui::perf::Counter::$counter, $value as u64);
    };
}

use super::view_abi::{NativeViewRuntime, runtime_is_registered};
use super::{
    ALIGN_CENTER, ALIGN_END, ALIGN_START, DIFF_ADDITION, DIFF_CONTEXT, DIFF_DELETION,
    DIFF_TERMINATED, DIFF_UNTERMINATED, GRID_TRACK_CONTENT, GRID_TRACK_CONTENT_MAX,
    GRID_TRACK_FIXED, GRID_TRACK_FLEX, GRID_TRACK_FLEX_MAX, LAYOUT_CHILD_CONTENT_MAX,
    LAYOUT_CHILD_FIXED, LAYOUT_CHILD_FLEX, LAYOUT_CHILD_FLEX_MAX, LAYOUT_CHILD_NORMAL,
    OVERFLOW_ELLIPSIS, OVERFLOW_FOOTER, OVERFLOW_NONE, PACKED_V3_PATCH_HEIGHT,
    PACKED_V3_PATCH_MAX_HEIGHT, PACKED_V3_PATCH_MAX_WIDTH, PACKED_V3_PATCH_MIN_HEIGHT,
    PACKED_V3_PATCH_MIN_WIDTH, PACKED_V3_PATCH_PADDING, PACKED_V3_PATCH_WIDTH, VERTICAL_BOTTOM,
    VERTICAL_CENTER, VERTICAL_TOP, VIEW_BRIDGE_SCHEMA_VERSION, WRAP_GRAPHEME, WRAP_NO_WRAP,
    WRAP_WORD_THEN_GRAPHEME,
};

pub const FAST_ABI_MAGIC: u32 = 0x494f_4654;
pub const FAST_ABI_VERSION: u32 = 1;
pub const FAST_COMMAND_BYTES: usize = 256 * 1024;
pub const FAST_META_OFFSET: usize = 128 * 1024;
pub const FAST_CONTROL_WORDS: usize = 16;
pub const FAST_OP_WORDS: usize = 10;
pub const FAST_OP_BYTES: usize = FAST_OP_WORDS * 4;
pub const FAST_PAGE_BYTES: usize = 64 * 1024;
pub const FAST_PAGE_COUNT: usize = 128;
pub const FAST_MAX_OPS: usize = (FAST_META_OFFSET - FAST_CONTROL_WORDS * 4) / FAST_OP_BYTES;
const MAX_COUNT: usize = 1_000_000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const LOCAL_BIT: u32 = 0x8000_0000;
const PAGE_FREE: u8 = 0;
const PAGE_WRITING: u8 = 1;
const PAGE_SEALED: u8 = 2;
const PAGE_RETAINED: u8 = 3;
pub const FAST_OK: i32 = 0;
pub const FAST_CACHE_MISS: i32 = 1;
pub const FAST_BAD_SESSION: i32 = 2;
#[allow(dead_code)]
pub const FAST_BAD_GENERATION: i32 = 3;
pub const FAST_BAD_BATCH: i32 = 4;
pub const FAST_PAGE_STATE: i32 = 5;
#[allow(dead_code)]
pub const FAST_UNSUPPORTED: i32 = 6;
pub const FAST_INTERNAL: i32 = 7;

const OP_DEF_TEXT: u32 = 1;
const OP_DEF_DIFF: u32 = 2;
const OP_DEF_SPACER: u32 = 3;
const OP_DEF_AXIS: u32 = 4;
const OP_DEF_HANGING: u32 = 5;
const OP_DEF_GRID: u32 = 6;
const OP_DEF_CONTAINER: u32 = 7;
const OP_DEF_CLAMP: u32 = 8;
const OP_DEF_CONTENT_MAX: u32 = 9;
const OP_DEF_COMPONENT: u32 = 10;
const OP_DEF_DECORATED: u32 = 11;
const OP_DEF_SEQ_LEAF: u32 = 12;
const OP_DEF_SEQ_BRANCH: u32 = 13;
const OP_DEF_GRID_LEAF: u32 = 14;
const OP_DEF_GRID_BRANCH: u32 = 15;
const OP_PATCH_TEXT: u32 = 16;
const OP_PATCH_DECORATION: u32 = 17;
const OP_PATCH_AXIS: u32 = 18;
const OP_PATCH_GRID: u32 = 19;

#[derive(Clone)]
enum FastSlot {
    Empty,
    View(iyon_tui::WeakView),
    Sequence(Weak<FastSequence>),
    GridSequence(Weak<FastGridSequence>),
}

pub(super) struct FastSlotTable {
    pages: Vec<Option<Box<[FastSlot]>>>,
}

impl FastSlotTable {
    pub(super) fn new() -> Self {
        Self { pages: Vec::new() }
    }

    fn page_offset(reference: u32) -> (usize, usize) {
        let value = reference as usize;
        (value >> 12, value & 4095)
    }

    fn page_mut(&mut self, page_index: usize) -> &mut [FastSlot] {
        if self.pages.len() <= page_index {
            self.pages.resize_with(page_index + 1, || None);
        }
        self.pages[page_index].get_or_insert_with(|| {
            std::iter::repeat_with(|| FastSlot::Empty)
                .take(4096)
                .collect::<Vec<_>>()
                .into_boxed_slice()
        })
    }

    fn get(&self, reference: u32) -> Option<&FastSlot> {
        let (page, offset) = Self::page_offset(reference);
        self.pages.get(page)?.as_ref()?.get(offset)
    }

    fn set(&mut self, reference: u32, value: FastSlot) -> Result<(), ()> {
        if reference == 0 || reference >= LOCAL_BIT {
            return Err(());
        }
        self.page_mut((reference as usize) >> 12)[(reference as usize) & 4095] = value;
        Ok(())
    }

    fn resolve_view(&self, reference: u32) -> Option<View> {
        match self.get(reference)? {
            FastSlot::View(weak) => weak.upgrade(),
            _ => None,
        }
    }

    fn resolve_sequence(&self, reference: u32) -> Option<Arc<FastSequence>> {
        match self.get(reference)? {
            FastSlot::Sequence(weak) => weak.upgrade(),
            _ => None,
        }
    }

    fn resolve_grid_sequence(&self, reference: u32) -> Option<Arc<FastGridSequence>> {
        match self.get(reference)? {
            FastSlot::GridSequence(weak) => weak.upgrade(),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct FastChild {
    kind: u32,
    size: u16,
    max_rows: u16,
    view: View,
}

#[allow(dead_code)]
#[derive(Clone)]
enum FastSequence {
    Leaf {
        kind: u32,
        aggregate: u32,
        items: Arc<[FastChild]>,
        retained: RetainedAxis,
    },
    Branch {
        kind: u32,
        height: u32,
        aggregate: u32,
        children: Arc<[Arc<FastSequence>]>,
        sizes: Arc<[u32]>,
        retained: RetainedAxis,
    },
}

impl FastSequence {
    fn kind(&self) -> u32 {
        match self {
            Self::Leaf { kind, .. } | Self::Branch { kind, .. } => *kind,
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::Leaf { .. } => 0,
            Self::Branch { height, .. } => *height,
        }
    }

    fn length(&self) -> u32 {
        match self {
            Self::Leaf { items, .. } => items.len() as u32,
            Self::Branch { sizes, .. } => sizes.last().copied().unwrap_or(0),
        }
    }

    fn aggregate(&self) -> u32 {
        match self {
            Self::Leaf { aggregate, .. } | Self::Branch { aggregate, .. } => *aggregate,
        }
    }

    fn retained(&self) -> RetainedAxis {
        match self {
            Self::Leaf { retained, .. } | Self::Branch { retained, .. } => retained.clone(),
        }
    }
}

#[derive(Clone)]
struct FastGridCell {
    row: usize,
    column: usize,
    row_span: u16,
    column_span: u16,
    horizontal_align: HorizontalAlign,
    vertical_align: VerticalAlign,
    view: View,
}

#[allow(dead_code)]
#[derive(Clone)]
enum FastGridSequence {
    Leaf {
        aggregate: u32,
        items: Arc<[FastGridCell]>,
        retained: RetainedGridCells,
    },
    Branch {
        height: u32,
        aggregate: u32,
        children: Arc<[Arc<FastGridSequence>]>,
        sizes: Arc<[u32]>,
        retained: RetainedGridCells,
    },
}

impl FastGridSequence {
    fn height(&self) -> u32 {
        match self {
            Self::Leaf { .. } => 0,
            Self::Branch { height, .. } => *height,
        }
    }

    fn length(&self) -> u32 {
        match self {
            Self::Leaf { items, .. } => items.len() as u32,
            Self::Branch { sizes, .. } => sizes.last().copied().unwrap_or(0),
        }
    }

    fn aggregate(&self) -> u32 {
        match self {
            Self::Leaf { aggregate, .. } | Self::Branch { aggregate, .. } => *aggregate,
        }
    }

    fn retained(&self) -> RetainedGridCells {
        match self {
            Self::Leaf { retained, .. } | Self::Branch { retained, .. } => retained.clone(),
        }
    }
}

#[derive(Clone)]
enum FastStaged {
    View { node_id: u64, view: View },
    Sequence(Arc<FastSequence>),
    GridSequence(Arc<FastGridSequence>),
}

struct FastPage {
    bytes: Box<[u8]>,
    state: AtomicU8,
    retained: AtomicUsize,
    used: AtomicUsize,
}

impl FastPage {
    fn new() -> Self {
        Self {
            bytes: vec![0; FAST_PAGE_BYTES].into_boxed_slice(),
            state: AtomicU8::new(PAGE_FREE),
            retained: AtomicUsize::new(0),
            used: AtomicUsize::new(0),
        }
    }

    fn ptr(&self) -> usize {
        self.bytes.as_ptr() as usize
    }

    fn reclaim(&self) {
        if self.retained.load(Ordering::Acquire) == 0 {
            let _ = self.state.compare_exchange(
                PAGE_SEALED,
                PAGE_FREE,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            let _ = self.state.compare_exchange(
                PAGE_RETAINED,
                PAGE_FREE,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if self.state.load(Ordering::Acquire) == PAGE_FREE {
                self.used.store(0, Ordering::Release);
            }
        }
    }
}

struct FastUtf8Slice {
    page: Arc<FastPage>,
    start: usize,
    len: usize,
}

impl FastUtf8Slice {
    fn new(page: Arc<FastPage>, start: usize, len: usize) -> Result<Arc<Self>, FastError> {
        let end = start.checked_add(len).ok_or_else(FastError::batch)?;
        let bytes = page.bytes.get(start..end).ok_or_else(FastError::batch)?;
        std::str::from_utf8(bytes).map_err(|_| FastError::batch())?;
        page.retained.fetch_add(1, Ordering::AcqRel);
        page.state.store(PAGE_RETAINED, Ordering::Release);
        fast_perf_inc!(FastPagesRetained);
        Ok(Arc::new(Self { page, start, len }))
    }
}

impl Drop for FastUtf8Slice {
    fn drop(&mut self) {
        self.page.retained.fetch_sub(1, Ordering::AcqRel);
        if self.page.retained.load(Ordering::Acquire) == 0 {
            fast_perf_inc!(FastPagesReleased);
            let _ = self.page.state.compare_exchange(
                PAGE_RETAINED,
                PAGE_SEALED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}

impl SharedUtf8Source for FastUtf8Slice {
    fn as_str(&self) -> &str {
        let bytes = &self.page.bytes[self.start..self.start + self.len];
        std::str::from_utf8(bytes).expect("validated retained UTF-8 page slice")
    }
}

struct FastControl {
    generation: u32,
    sequence: u32,
    op_count: usize,
    root: u32,
    page_id: usize,
    byte_used: usize,
    meta_offset: usize,
    meta_used: usize,
    flags: u32,
}

struct FastError {
    status: i32,
    detail: u32,
}

impl FastError {
    const fn batch() -> Self {
        Self {
            status: FAST_BAD_BATCH,
            detail: 0,
        }
    }
    const fn page() -> Self {
        Self {
            status: FAST_PAGE_STATE,
            detail: 0,
        }
    }
    const fn cache_miss() -> Self {
        Self {
            status: FAST_CACHE_MISS,
            detail: 0,
        }
    }
}

pub struct FastSession {
    host_addr: usize,
    runtime: usize,
    owner_thread: ThreadId,
    closed: AtomicBool,
    generation: u32,
    sequence: u32,
    command: Box<[u8]>,
    pages: Vec<Arc<FastPage>>,
    staged: Vec<Option<FastStaged>>,
    publications: Vec<(u32, FastStaged)>,
    last_status: i32,
    last_detail: u32,
}

impl Drop for FastSession {
    fn drop(&mut self) {
        self.close();
        self.unregister();
    }
}

impl FastSession {
    pub fn new(host: &mut iyon_tui::TuiHost, runtime: *mut NativeViewRuntime) -> Self {
        Self {
            host_addr: host as *mut iyon_tui::TuiHost as usize,
            runtime: runtime as usize,
            owner_thread: thread::current().id(),
            closed: AtomicBool::new(false),
            generation: 0,
            sequence: 0,
            command: vec![0; FAST_COMMAND_BYTES].into_boxed_slice(),
            pages: (0..FAST_PAGE_COUNT)
                .map(|_| Arc::new(FastPage::new()))
                .collect(),
            staged: Vec::new(),
            publications: Vec::new(),
            last_status: FAST_OK,
            last_detail: 0,
        }
    }

    #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
    fn update_page_gauges(&self) {
        let mut live_pages = 0usize;
        let mut live_payload = 0usize;
        for page in &self.pages {
            if page.state.load(Ordering::Acquire) != PAGE_FREE {
                live_pages += 1;
                live_payload += page.used.load(Ordering::Acquire);
            }
        }
        fast_perf_set!(FastLivePageBytes, live_pages * FAST_PAGE_BYTES);
        fast_perf_set!(FastLivePayloadBytes, live_payload);
    }

    pub fn descriptor(&self) -> serde_json::Value {
        serde_json::json!({
            "runtime_ptr": self.runtime as usize as u64,
            "host_ptr": self.host_addr as u64,
            "magic": FAST_ABI_MAGIC,
            "version": FAST_ABI_VERSION,
            "schema_version": VIEW_BRIDGE_SCHEMA_VERSION,
            "control_words": FAST_CONTROL_WORDS,
            "op_words": FAST_OP_WORDS,
            "command_bytes": FAST_COMMAND_BYTES,
            "meta_offset": FAST_META_OFFSET,
            "max_ops": FAST_MAX_OPS,
            "page_bytes": FAST_PAGE_BYTES,
            "command_ptr": self.command.as_ptr() as usize as u64,
            "pages": self.pages.iter().enumerate().map(|(id, page)| serde_json::json!({
                "id": id,
                "ptr": page.ptr() as u64,
                "bytes": FAST_PAGE_BYTES,
            })).collect::<Vec<_>>(),
            "commit_ptr": iyon_fast_commit_v1 as *const () as usize as u64,
            "acquire_ptr": iyon_fast_acquire_utf8_page_v1 as *const () as usize as u64,
            "release_ptr": iyon_fast_release_client_page_v1 as *const () as usize as u64,
            "render_ref_ptr": iyon_fast_render_ref_v1 as *const () as usize as u64,
        })
    }

    pub fn register(&mut self) -> Result<(), ()> {
        if !runtime_is_registered(self.runtime) {
            return Err(());
        }
        let runtime = unsafe { (self.runtime as *mut NativeViewRuntime).as_mut() }.ok_or(())?;
        if !runtime.valid_on_owner_thread() {
            return Err(());
        }
        let pointer = self as *mut Self as usize;
        if runtime.fast_sessions.contains_key(&self.host_addr) {
            return Err(());
        }
        runtime.fast_sessions.insert(self.host_addr, pointer);
        Ok(())
    }

    pub fn unregister(&self) {
        if !runtime_is_registered(self.runtime) {
            return;
        }
        let Some(runtime) = (unsafe { (self.runtime as *mut NativeViewRuntime).as_mut() }) else {
            return;
        };
        if runtime.fast_sessions.get(&self.host_addr) == Some(&(self as *const Self as usize)) {
            runtime.fast_sessions.remove(&self.host_addr);
            runtime.fast_slots.remove(&self.host_addr);
        }
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    fn assert_thread(&self) -> Result<(), FastError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(FastError {
                status: FAST_BAD_SESSION,
                detail: 3,
            });
        }
        if thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(FastError {
                status: FAST_BAD_SESSION,
                detail: 1,
            })
        }
    }

    fn runtime_mut(&self) -> Result<&'static mut NativeViewRuntime, FastError> {
        self.assert_thread()?;
        let runtime =
            unsafe { (self.runtime as *mut NativeViewRuntime).as_mut() }.ok_or(FastError {
                status: FAST_BAD_SESSION,
                detail: 4,
            })?;
        if !runtime.valid_on_owner_thread() {
            return Err(FastError {
                status: FAST_BAD_SESSION,
                detail: 5,
            });
        }
        Ok(runtime)
    }

    fn control(&self) -> Result<FastControl, FastError> {
        if self.command.len() < FAST_CONTROL_WORDS * 4 {
            return Err(FastError::batch());
        }
        let word = |index: usize| read_u32(&self.command, index * 4).ok_or_else(FastError::batch);
        let magic = word(0)?;
        let version = word(1)?;
        let schema = word(13)?;
        if magic != FAST_ABI_MAGIC
            || version != FAST_ABI_VERSION
            || schema != VIEW_BRIDGE_SCHEMA_VERSION
        {
            return Err(FastError::batch());
        }
        let op_count = word(4)? as usize;
        let meta_offset = word(8)? as usize;
        let meta_used = word(9)? as usize;
        let control = FastControl {
            generation: word(2)?,
            sequence: word(3)?,
            op_count,
            root: word(5)?,
            page_id: word(6)? as usize,
            byte_used: word(7)? as usize,
            meta_offset,
            meta_used,
            flags: word(10)?,
        };
        if control.op_count > FAST_MAX_OPS
            || control.meta_offset != FAST_META_OFFSET
            || control.meta_used > FAST_COMMAND_BYTES - FAST_META_OFFSET
            || control.op_count * FAST_OP_BYTES > FAST_META_OFFSET - FAST_CONTROL_WORDS * 4
            || control.sequence == 0
        {
            return Err(FastError::batch());
        }
        Ok(control)
    }

    fn acquire_page(&mut self) -> i32 {
        if self.assert_thread().is_err() {
            return FAST_BAD_SESSION;
        }
        for page in &self.pages {
            page.reclaim();
        }
        for (id, page) in self.pages.iter().enumerate() {
            if page
                .state
                .compare_exchange(PAGE_FREE, PAGE_WRITING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
                self.update_page_gauges();
                return id as i32;
            }
        }
        FAST_PAGE_STATE
    }

    fn release_page(&mut self, page_id: usize) -> i32 {
        if self.assert_thread().is_err() {
            return FAST_BAD_SESSION;
        }
        let Some(page) = self.pages.get(page_id) else {
            return FAST_PAGE_STATE;
        };
        if page.retained.load(Ordering::Acquire) != 0 {
            return FAST_PAGE_STATE;
        }
        page.used.store(0, Ordering::Release);
        page.state.store(PAGE_FREE, Ordering::Release);
        #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
        self.update_page_gauges();
        FAST_OK
    }

    fn commit(&mut self) -> i32 {
        fast_perf_inc!(FastTransactions);
        self.last_detail = 0;
        let status = self
            .commit_inner()
            .map(|_| FAST_OK)
            .unwrap_or_else(|error| {
                self.last_detail = error.detail;
                error.status
            });
        if status != FAST_OK {
            if status == FAST_CACHE_MISS {
                fast_perf_inc!(FastStatusCacheMiss);
            } else {
                fast_perf_inc!(FastStatusInvalid);
            }
            self.staged.clear();
            self.publications.clear();
            if let Ok(control) = self.control() {
                if let Some(page) = self.pages.get(control.page_id) {
                    if page.retained.load(Ordering::Acquire) == 0 {
                        page.state.store(PAGE_FREE, Ordering::Release);
                    }
                }
            }
        }
        self.last_status = status;
        write_u32(&mut self.command, 11 * 4, status as u32);
        write_u32(&mut self.command, 12 * 4, self.last_detail);
        status
    }

    fn commit_inner(&mut self) -> Result<(), FastError> {
        self.assert_thread()?;
        let control = self.control()?;
        if control.sequence <= self.sequence {
            return Err(FastError::batch());
        }
        if control.generation != self.generation {
            let expected = self
                .generation
                .checked_add(1)
                .ok_or_else(FastError::batch)?;
            if control.generation != expected || control.flags & 1 == 0 {
                return Err(FastError::cache_miss());
            }
            self.runtime_mut()?.fast_slots.remove(&self.host_addr);
            // A FastShared generation reset only invalidates transport-local
            // refs. The environment semantic NodeId cache is retained so V3,
            // V4, direct decode, and generated calls can recover the same View.
            self.generation = control.generation;
        } else if control.flags & 1 != 0 {
            return Err(FastError::batch());
        }
        let page = if control.byte_used == 0 {
            None
        } else {
            let page = self
                .pages
                .get(control.page_id)
                .ok_or_else(FastError::page)?;
            if page.state.load(Ordering::Acquire) != PAGE_WRITING {
                return Err(FastError::page());
            }
            if control.byte_used > FAST_PAGE_BYTES {
                return Err(FastError::batch());
            }
            Some(Arc::clone(page))
        };
        let meta_start = control.meta_offset;
        let meta_end = meta_start
            .checked_add(control.meta_used)
            .ok_or_else(FastError::batch)?;
        if meta_end > self.command.len() {
            return Err(FastError::batch());
        }
        self.staged.clear();
        self.staged.reserve(control.op_count);
        self.publications.clear();
        let mut previous_destination = 0;
        for index in 0..control.op_count {
            let op_start = FAST_CONTROL_WORDS * 4 + index * FAST_OP_BYTES;
            let op = FastOp::read(&self.command, op_start).ok_or_else(FastError::batch)?;
            fast_perf_inc!(FastOpsRead);
            if op.dst != 0 {
                if op.dst <= previous_destination || op.dst >= LOCAL_BIT {
                    return Err(FastError::batch());
                }
                previous_destination = op.dst;
            }
            let value = self.decode_op(op, page.as_ref(), meta_start, meta_end)?;
            if op.dst != 0 {
                self.publications.push((op.dst, value.clone()));
            }
            self.staged.push(Some(value));
        }
        let root = self.resolve_view(control.root)?;
        if self.staged.len() != control.op_count || root.is_none() {
            return Err(FastError::batch());
        }
        if let Some(page) = page.as_ref() {
            std::str::from_utf8(&page.bytes[..control.byte_used])
                .map_err(|_| FastError::batch())?;
            page.used.store(control.byte_used, Ordering::Release);
            page.state.store(PAGE_SEALED, Ordering::Release);
        }
        #[cfg(all(feature = "perf-counters", not(feature = "perf-packed-timing")))]
        self.update_page_gauges();
        self.validate_publications()?;
        self.publish_publications()?;
        let host = self.host_addr as *mut iyon_tui::TuiHost;
        if host.is_null() {
            return Err(FastError {
                status: FAST_BAD_SESSION,
                detail: 2,
            });
        }
        // The host pointer is stable because NativeTuiHost stores TuiHost in a
        // Box. The C ABI is thread-affine and this method has already checked
        // the owner thread.
        let result = unsafe { (&mut *host).render(root.expect("validated fast root")) };
        result.map_err(|_| FastError {
            status: FAST_INTERNAL,
            detail: 3,
        })?;
        self.sequence = control.sequence;
        self.staged.clear();
        self.publications.clear();
        Ok(())
    }

    fn validate_publications(&self) -> Result<(), FastError> {
        let mut transaction_node_ids = Vec::new();
        for (_, value) in &self.publications {
            let FastStaged::View { node_id, view } = value else {
                continue;
            };
            if *node_id == 0 || *node_id > MAX_SAFE_INTEGER {
                return Err(FastError::batch());
            }
            if transaction_node_ids
                .iter()
                .any(|existing| existing == node_id)
            {
                return Err(FastError::batch());
            }
            transaction_node_ids.push(*node_id);
            if let Some(existing) = self
                .runtime_mut()?
                .nodes
                .get(node_id)
                .and_then(iyon_tui::WeakView::upgrade)
                && existing != *view
            {
                return Err(FastError::batch());
            }
        }
        Ok(())
    }

    fn publish_publications(&mut self) -> Result<(), FastError> {
        let runtime = self.runtime_mut()?;
        for (reference, value) in self.publications.drain(..) {
            fast_perf_inc!(FastPublications);
            match value {
                FastStaged::View { node_id, view } => {
                    fast_perf_inc!(FastViewsBuilt);
                    if node_id != 0 {
                        runtime
                            .publish_bulk(node_id, view.clone())
                            .map_err(|_| FastError::batch())?;
                    }
                    runtime
                        .fast_slots_for(self.host_addr)
                        .set(reference, FastSlot::View(view.downgrade()))
                        .map_err(|_| FastError::batch())?;
                }
                FastStaged::Sequence(sequence) => {
                    fast_perf_inc!(FastSeqNodesBuilt);
                    runtime
                        .fast_slots_for(self.host_addr)
                        .set(reference, FastSlot::Sequence(Arc::downgrade(&sequence)))
                        .map_err(|_| FastError::batch())?;
                }
                FastStaged::GridSequence(sequence) => {
                    fast_perf_inc!(FastSeqNodesBuilt);
                    runtime
                        .fast_slots_for(self.host_addr)
                        .set(reference, FastSlot::GridSequence(Arc::downgrade(&sequence)))
                        .map_err(|_| FastError::batch())?;
                }
            }
        }
        Ok(())
    }

    fn resolve_view(&self, wire: u32) -> Result<Option<View>, FastError> {
        fast_perf_inc!(FastRefsResolved);
        if wire & LOCAL_BIT != 0 {
            let index = (wire & !LOCAL_BIT) as usize;
            return match self.staged.get(index).and_then(Option::as_ref) {
                Some(FastStaged::View { view, .. }) => Ok(Some(view.clone())),
                _ => Err(FastError::batch()),
            };
        }
        if wire == 0 || wire >= LOCAL_BIT {
            return Err(FastError::cache_miss());
        }
        Ok(self
            .runtime_mut()?
            .fast_slots_for(self.host_addr)
            .resolve_view(wire))
    }

    fn resolve_sequence(&self, wire: u32) -> Result<Arc<FastSequence>, FastError> {
        fast_perf_inc!(FastRefsResolved);
        if wire & LOCAL_BIT != 0 {
            let index = (wire & !LOCAL_BIT) as usize;
            return match self.staged.get(index).and_then(Option::as_ref) {
                Some(FastStaged::Sequence(sequence)) => Ok(Arc::clone(sequence)),
                _ => Err(FastError::batch()),
            };
        }
        if wire == 0 || wire >= LOCAL_BIT {
            return Err(FastError::cache_miss());
        }
        self.runtime_mut()?
            .fast_slots_for(self.host_addr)
            .resolve_sequence(wire)
            .ok_or_else(FastError::cache_miss)
    }

    fn resolve_grid_sequence(&self, wire: u32) -> Result<Arc<FastGridSequence>, FastError> {
        fast_perf_inc!(FastRefsResolved);
        if wire & LOCAL_BIT != 0 {
            let index = (wire & !LOCAL_BIT) as usize;
            return match self.staged.get(index).and_then(Option::as_ref) {
                Some(FastStaged::GridSequence(sequence)) => Ok(Arc::clone(sequence)),
                _ => Err(FastError::batch()),
            };
        }
        if wire == 0 || wire >= LOCAL_BIT {
            return Err(FastError::batch());
        }
        self.runtime_mut()?
            .fast_slots_for(self.host_addr)
            .resolve_grid_sequence(wire)
            .ok_or_else(FastError::cache_miss)
    }

    fn decode_op(
        &self,
        op: FastOp,
        page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        match op.opcode {
            OP_DEF_TEXT => self.decode_text(op, page, meta_start, meta_end),
            OP_DEF_DIFF => self.decode_diff(op, page, meta_start, meta_end),
            OP_DEF_SPACER => Ok(FastStaged::View {
                node_id: op.node_id(),
                view: View::spacer(u16::try_from(op.a).map_err(|_| FastError::batch())?),
            }),
            OP_DEF_AXIS => {
                let sequence = self.resolve_sequence(op.b)?;
                let gap = u16::try_from(op.a).map_err(|_| FastError::batch())?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: sequence
                        .retained()
                        .into_view(gap)
                        .retain_transport_payload(sequence),
                })
            }
            OP_DEF_HANGING => {
                let prefix = self.resolve_view(op.a)?.ok_or_else(FastError::cache_miss)?;
                let continuation = self.resolve_view(op.b)?.ok_or_else(FastError::cache_miss)?;
                let body = self.resolve_view(op.c)?.ok_or_else(FastError::cache_miss)?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: View::from_retained_hanging(prefix, continuation, body)
                        .map_err(|_| FastError::batch())?,
                })
            }
            OP_DEF_GRID => self.decode_grid(op, meta_start, meta_end),
            OP_DEF_CONTAINER => Ok(FastStaged::View {
                node_id: op.node_id(),
                view: View::from_retained_container(
                    self.resolve_view(op.a)?.ok_or_else(FastError::cache_miss)?,
                ),
            }),
            OP_DEF_CLAMP => {
                let child = self.resolve_view(op.a)?.ok_or_else(FastError::cache_miss)?;
                let overflow = self.read_overflow(op.b, op.d, page, meta_start, meta_end)?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: View::from_retained_clamp(child, op.c16()?, overflow),
                })
            }
            OP_DEF_CONTENT_MAX => Ok(FastStaged::View {
                node_id: op.node_id(),
                view: View::from_retained_clamp(
                    self.resolve_view(op.a)?.ok_or_else(FastError::cache_miss)?,
                    op.b16()?,
                    iyon_tui::OverflowIndicator::None,
                ),
            }),
            OP_DEF_COMPONENT => Ok(FastStaged::View {
                node_id: op.node_id(),
                view: View::from_retained_component(op.a64()),
            }),
            OP_DEF_DECORATED => {
                let child = self
                    .resolve_view(op.base)?
                    .ok_or_else(FastError::cache_miss)?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: self.read_decoration(
                        child,
                        op.a as usize,
                        op.b as usize,
                        page,
                        meta_start,
                        meta_end,
                    )?,
                })
            }
            OP_DEF_SEQ_LEAF => self.decode_sequence_leaf(op, meta_start, meta_end),
            OP_DEF_SEQ_BRANCH => self.decode_sequence_branch(op, meta_start, meta_end),
            OP_DEF_GRID_LEAF => self.decode_grid_leaf(op, meta_start, meta_end),
            OP_DEF_GRID_BRANCH => self.decode_grid_branch(op, meta_start, meta_end),
            OP_PATCH_TEXT => {
                let base = self
                    .resolve_view(op.base)?
                    .ok_or_else(FastError::cache_miss)?;
                let wrap = (op.a & 1 != 0).then(|| decode_wrap(op.b)).transpose()?;
                let align = (op.a & 2 != 0).then(|| decode_align(op.c)).transpose()?;
                if wrap.is_none() && align.is_none() {
                    return Err(FastError::batch());
                }
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: base.with_text_layout_patch(wrap, align),
                })
            }
            OP_PATCH_DECORATION => {
                let base = self
                    .resolve_view(op.base)?
                    .ok_or_else(FastError::cache_miss)?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: self.read_decoration_patch(
                        base,
                        op.a as usize,
                        op.b as usize,
                        page,
                        meta_start,
                        meta_end,
                    )?,
                })
            }
            OP_PATCH_AXIS => {
                let base = self
                    .resolve_view(op.base)?
                    .ok_or_else(FastError::cache_miss)?;
                let sequence = self.resolve_sequence(op.b)?;
                let gap = if op.a & 1 != 0 {
                    op.c16()?
                } else {
                    base.retained_axis_gap().ok_or_else(FastError::batch)?
                };
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: base
                        .patch_retained_axis(sequence.retained(), gap)
                        .retain_transport_payload(sequence),
                })
            }
            OP_PATCH_GRID => {
                let base = self
                    .resolve_view(op.base)?
                    .ok_or_else(FastError::cache_miss)?;
                let sequence = self.resolve_grid_sequence(op.a)?;
                Ok(FastStaged::View {
                    node_id: op.node_id(),
                    view: base
                        .patch_retained_grid(sequence.retained())
                        .map_err(|_| FastError::batch())?
                        .retain_transport_payload(sequence),
                })
            }
            _ => Err(FastError::batch()),
        }
    }

    fn payload<'a>(
        &self,
        op: FastOp,
        command: &'a [u8],
        meta_start: usize,
        meta_end: usize,
    ) -> Result<MetaReader<'a>, FastError> {
        let start = (op.a as usize)
            .checked_mul(4)
            .ok_or_else(FastError::batch)?;
        let length = (op.b as usize)
            .checked_mul(4)
            .ok_or_else(FastError::batch)?;
        let end = start.checked_add(length).ok_or_else(FastError::batch)?;
        if start < meta_start || end > meta_end {
            return Err(FastError::batch());
        }
        Ok(MetaReader::new(&command[start..end]))
    }

    fn decode_text(
        &self,
        op: FastOp,
        page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let wrap = decode_wrap(reader.word()?)?;
        let align = decode_align(reader.word()?)?;
        let count = reader.count()?;
        let mut spans = Vec::with_capacity(count);
        for _ in 0..count {
            let (offset, length) = reader.string_ref()?;
            let style = self.read_style(&mut reader, page)?;
            let span = if length == 0 {
                TextSpan::styled(String::new(), style)
            } else {
                let page = page.ok_or_else(FastError::page)?;
                let source =
                    FastUtf8Slice::new(Arc::clone(page), offset as usize, length as usize)?;
                TextSpan::from_shared_utf8(source, style)
            };
            spans.push(span);
        }
        reader.finish()?;
        Ok(FastStaged::View {
            node_id: op.node_id(),
            view: View::from_retained_text(spans, wrap, align),
        })
    }

    fn decode_diff(
        &self,
        op: FastOp,
        page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let hunk_count = reader.count()?;
        let mut hunks = Vec::with_capacity(hunk_count);
        for _ in 0..hunk_count {
            let old_range = DiffRange::new(DiffLineOffset::new(reader.u64()?), reader.u64()?)
                .map_err(|_| FastError::batch())?;
            let new_range = DiffRange::new(DiffLineOffset::new(reader.u64()?), reader.u64()?)
                .map_err(|_| FastError::batch())?;
            let line_count = reader.count()?;
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                let kind = reader.word()?;
                let text = self.read_string_ref(&mut reader, page)?;
                let termination = match reader.word()? {
                    DIFF_TERMINATED => DiffLineTermination::Terminated,
                    DIFF_UNTERMINATED => DiffLineTermination::Unterminated,
                    _ => return Err(FastError::batch()),
                };
                let line = match kind {
                    DIFF_CONTEXT => DiffLine::context(
                        DiffLineNumber::new(reader.positive_u64()?).ok_or_else(FastError::batch)?,
                        DiffLineNumber::new(reader.positive_u64()?).ok_or_else(FastError::batch)?,
                        text,
                    ),
                    DIFF_ADDITION => DiffLine::addition(
                        DiffLineNumber::new(reader.positive_u64()?).ok_or_else(FastError::batch)?,
                        text,
                    ),
                    DIFF_DELETION => DiffLine::deletion(
                        DiffLineNumber::new(reader.positive_u64()?).ok_or_else(FastError::batch)?,
                        text,
                    ),
                    _ => return Err(FastError::batch()),
                };
                lines.push(line.with_termination(termination));
            }
            hunks.push(DiffHunk::new(old_range, new_range, lines).map_err(|_| FastError::batch())?);
        }
        reader.finish()?;
        Ok(FastStaged::View {
            node_id: op.node_id(),
            view: iyon_tui::DiffRenderer::new().render(hunks.as_slice()),
        })
    }

    fn decode_grid(
        &self,
        op: FastOp,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let columns = self.read_tracks(&mut reader)?;
        let rows = self.read_tracks(&mut reader)?;
        let cells = self.resolve_grid_sequence(reader.word()?)?;
        let column_gap = reader.u16()?;
        let row_gap = reader.u16()?;
        reader.finish()?;
        Ok(FastStaged::View {
            node_id: op.node_id(),
            view: cells
                .retained()
                .into_view(columns, rows, column_gap, row_gap)
                .retain_transport_payload(cells),
        })
    }

    fn decode_sequence_leaf(
        &self,
        op: FastOp,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let kind = reader.word()?;
        if kind != super::PACKED_V3_SEQ_ROW && kind != super::PACKED_V3_SEQ_COLUMN {
            return Err(FastError::batch());
        }
        let count = reader.count()?;
        if count > 32 {
            return Err(FastError::batch());
        }
        let aggregate = reader.word()?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let child_kind = reader.word()?;
            let size = reader.u16()?;
            let max_rows = reader.u16()?;
            let view = self
                .resolve_view(reader.word()?)?
                .ok_or_else(FastError::cache_miss)?;
            validate_child(child_kind, size, max_rows, kind)?;
            items.push(FastChild {
                kind: child_kind,
                size,
                max_rows,
                view,
            });
        }
        reader.finish()?;
        let retained = RetainedAxis::leaf(
            kind == super::PACKED_V3_SEQ_ROW,
            items
                .iter()
                .map(|item| RetainedAxisChild {
                    track: retained_track(item.kind, item.size, item.max_rows),
                    view: item.view.clone(),
                })
                .collect(),
        );
        if u32::from(retained.aggregate_flags()) != aggregate {
            return Err(FastError::batch());
        }
        Ok(FastStaged::Sequence(Arc::new(FastSequence::Leaf {
            kind,
            aggregate,
            items: items.into(),
            retained,
        })))
    }

    fn decode_sequence_branch(
        &self,
        op: FastOp,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let kind = reader.word()?;
        let height = reader.word()?;
        let count = reader.count()?;
        if count == 0 || count > 32 {
            return Err(FastError::batch());
        }
        let aggregate = reader.word()?;
        let mut children = Vec::with_capacity(count);
        let mut sizes = Vec::with_capacity(count);
        let mut previous = 0;
        for _ in 0..count {
            let size = reader.word()?;
            if size <= previous {
                return Err(FastError::batch());
            }
            previous = size;
            sizes.push(size);
            children.push(self.resolve_sequence(reader.word()?)?);
        }
        reader.finish()?;
        if children.iter().any(|child| {
            child.kind() != kind || child.height() + 1 != height || child.length() == 0
        }) || sizes.last().copied().unwrap_or(0)
            != children.iter().map(|child| child.length()).sum::<u32>()
            || aggregate
                != children
                    .iter()
                    .fold(0, |flags, child| flags | child.aggregate())
        {
            return Err(FastError::batch());
        }
        let retained = RetainedAxis::branch(
            kind == super::PACKED_V3_SEQ_ROW,
            children.iter().map(|child| child.retained()).collect(),
        )
        .map_err(|_| FastError::batch())?;
        Ok(FastStaged::Sequence(Arc::new(FastSequence::Branch {
            kind,
            height,
            aggregate,
            children: children.into(),
            sizes: sizes.into(),
            retained,
        })))
    }

    fn decode_grid_leaf(
        &self,
        op: FastOp,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let count = reader.count()?;
        if count > 32 {
            return Err(FastError::batch());
        }
        let aggregate = reader.word()?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(FastGridCell {
                row: reader.count()?,
                column: reader.count()?,
                row_span: reader.positive_u16()?,
                column_span: reader.positive_u16()?,
                horizontal_align: decode_align(reader.word()?)?,
                vertical_align: decode_vertical(reader.word()?)?,
                view: self
                    .resolve_view(reader.word()?)?
                    .ok_or_else(FastError::cache_miss)?,
            });
        }
        reader.finish()?;
        let retained = RetainedGridCells::leaf(
            items
                .iter()
                .map(|item| RetainedGridCell {
                    row: item.row,
                    column: item.column,
                    row_span: item.row_span,
                    column_span: item.column_span,
                    horizontal_align: item.horizontal_align,
                    vertical_align: item.vertical_align,
                    view: item.view.clone(),
                })
                .collect(),
        );
        if u32::from(retained.aggregate_flags()) != aggregate {
            return Err(FastError::batch());
        }
        Ok(FastStaged::GridSequence(Arc::new(FastGridSequence::Leaf {
            aggregate,
            items: items.into(),
            retained,
        })))
    }

    fn decode_grid_branch(
        &self,
        op: FastOp,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<FastStaged, FastError> {
        let mut reader = self.payload(op, &self.command, meta_start, meta_end)?;
        let height = reader.word()?;
        let count = reader.count()?;
        if count == 0 || count > 32 {
            return Err(FastError::batch());
        }
        let aggregate = reader.word()?;
        let mut children = Vec::with_capacity(count);
        let mut sizes = Vec::with_capacity(count);
        let mut previous = 0;
        for _ in 0..count {
            let size = reader.word()?;
            if size <= previous {
                return Err(FastError::batch());
            }
            previous = size;
            sizes.push(size);
            children.push(self.resolve_grid_sequence(reader.word()?)?);
        }
        reader.finish()?;
        if children
            .iter()
            .any(|child| child.height() + 1 != height || child.length() == 0)
            || sizes.last().copied().unwrap_or(0)
                != children.iter().map(|child| child.length()).sum::<u32>()
            || aggregate
                != children
                    .iter()
                    .fold(0, |flags, child| flags | child.aggregate())
        {
            return Err(FastError::batch());
        }
        let retained =
            RetainedGridCells::branch(children.iter().map(|child| child.retained()).collect())
                .map_err(|_| FastError::batch())?;
        Ok(FastStaged::GridSequence(Arc::new(
            FastGridSequence::Branch {
                height,
                aggregate,
                children: children.into(),
                sizes: sizes.into(),
                retained,
            },
        )))
    }

    fn read_tracks(&self, reader: &mut MetaReader<'_>) -> Result<Vec<GridTrack>, FastError> {
        let count = reader.count()?;
        let mut tracks = Vec::with_capacity(count);
        for _ in 0..count {
            let kind = reader.word()?;
            let value = reader.u16()?;
            let track = match kind {
                GRID_TRACK_CONTENT if value == 0 => GridTrack::content(),
                GRID_TRACK_CONTENT_MAX => GridTrack::content_max(value),
                GRID_TRACK_FIXED => GridTrack::fixed(value),
                GRID_TRACK_FLEX if value == 0 => GridTrack::flex(),
                GRID_TRACK_FLEX_MAX => GridTrack::flex_max(value),
                _ => return Err(FastError::batch()),
            };
            tracks.push(track);
        }
        Ok(tracks)
    }

    fn read_string_ref(
        &self,
        reader: &mut MetaReader<'_>,
        page: Option<&Arc<FastPage>>,
    ) -> Result<String, FastError> {
        let (offset, length) = reader.string_ref()?;
        let page = page.ok_or_else(FastError::page)?;
        let end = (offset as usize)
            .checked_add(length as usize)
            .ok_or_else(FastError::batch)?;
        let bytes = page
            .bytes
            .get(offset as usize..end)
            .ok_or_else(FastError::batch)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| FastError::batch())
    }

    fn read_style(
        &self,
        reader: &mut MetaReader<'_>,
        page: Option<&Arc<FastPage>>,
    ) -> Result<StyleRef, FastError> {
        let flags = reader.word()?;
        if flags & !7 != 0 {
            return Err(FastError::batch());
        }
        let theme = if flags & 1 != 0 {
            Some(self.read_string_ref(reader, page)?)
        } else {
            None
        };
        let foreground = if flags & 2 != 0 {
            Some(self.read_color(reader, page)?)
        } else {
            None
        };
        let background = if flags & 4 != 0 {
            Some(self.read_color(reader, page)?)
        } else {
            None
        };
        let present = reader.word()?;
        let truth = reader.word()?;
        if present & !63 != 0 || truth & !present != 0 {
            return Err(FastError::batch());
        }
        let mut style = StyleSpec::new();
        if let Some(color) = foreground {
            style = style.foreground(color);
        }
        if let Some(color) = background {
            style = style.background(color);
        }
        for (bit, attribute) in [
            (1, TextAttribute::Bold),
            (2, TextAttribute::Dim),
            (4, TextAttribute::Italic),
            (8, TextAttribute::Underline),
            (16, TextAttribute::Reversed),
            (32, TextAttribute::Strikethrough),
        ] {
            if present & bit != 0 {
                style = style.attribute(attribute, truth & bit != 0);
            }
        }
        Ok(match theme {
            Some(theme) => StyleRef::themed(theme, style),
            None => StyleRef::direct(style),
        })
    }

    fn read_color(
        &self,
        reader: &mut MetaReader<'_>,
        page: Option<&Arc<FastPage>>,
    ) -> Result<ColorSpec, FastError> {
        match reader.word()? {
            1 => super::decode_color_string(&self.read_string_ref(reader, page)?)
                .map_err(|_| FastError::batch()),
            2 => Ok(ColorSpec::ansi(
                u8::try_from(reader.word()?).map_err(|_| FastError::batch())?,
            )),
            _ => Err(FastError::batch()),
        }
    }

    fn read_border(
        &self,
        reader: &mut MetaReader<'_>,
        page: Option<&Arc<FastPage>>,
    ) -> Result<BorderSpec, FastError> {
        let flags = reader.word()?;
        if flags & !15 != 0 {
            return Err(FastError::batch());
        }
        let glyphs = if flags & 1 != 0 {
            Some([
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
                self.read_string_ref(reader, page)?,
            ])
        } else {
            None
        };
        let color = if flags & 2 != 0 {
            Some(self.read_color(reader, page)?)
        } else {
            None
        };
        let style = if flags & 4 != 0 {
            Some(reader.word()?)
        } else {
            None
        };
        let edges = if flags & 8 != 0 {
            Some(reader.word()?)
        } else {
            None
        };
        let mut spec = match style.unwrap_or(1) {
            1 => BorderSpec::plain(),
            2 => BorderSpec::rounded(),
            3 => BorderSpec::double(),
            _ => return Err(FastError::batch()),
        };
        if let Some(glyphs) = glyphs {
            spec = BorderSpec::custom(
                BorderGlyphs::new(
                    &glyphs[0], &glyphs[1], &glyphs[2], &glyphs[3], &glyphs[4], &glyphs[5],
                    &glyphs[6], &glyphs[7],
                )
                .map_err(|_| FastError::batch())?,
            );
        }
        if edges == Some(2) {
            spec = spec.edges(BorderEdges::TOP_BOTTOM);
        }
        if edges.is_some_and(|value| value != 1 && value != 2) {
            return Err(FastError::batch());
        }
        if let Some(color) = color {
            spec = spec.color(color);
        }
        Ok(spec)
    }

    fn read_decoration(
        &self,
        child: View,
        offset: usize,
        length: usize,
        page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<View, FastError> {
        let mut reader = self.payload_from_offset(offset, length, meta_start, meta_end)?;
        let decoration = self.read_decoration_value(&mut reader, page)?;
        reader.finish()?;
        Ok(View::from_retained_decoration(child, decoration))
    }

    fn read_decoration_patch(
        &self,
        base: View,
        offset: usize,
        length: usize,
        _page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<View, FastError> {
        let mut reader = self.payload_from_offset(offset, length, meta_start, meta_end)?;
        let flags = reader.word()?;
        let known = PACKED_V3_PATCH_PADDING
            | PACKED_V3_PATCH_WIDTH
            | PACKED_V3_PATCH_HEIGHT
            | PACKED_V3_PATCH_MIN_WIDTH
            | PACKED_V3_PATCH_MAX_WIDTH
            | PACKED_V3_PATCH_MIN_HEIGHT
            | PACKED_V3_PATCH_MAX_HEIGHT;
        if flags == 0 || flags & !known != 0 {
            return Err(FastError::batch());
        }
        let padding = if flags & PACKED_V3_PATCH_PADDING != 0 {
            Some(Insets::new(
                reader.u16()?,
                reader.u16()?,
                reader.u16()?,
                reader.u16()?,
            ))
        } else {
            None
        };
        let width = if flags & PACKED_V3_PATCH_WIDTH != 0 {
            decode_size_rule(reader.word()?)?
        } else {
            None
        };
        let height = if flags & PACKED_V3_PATCH_HEIGHT != 0 {
            decode_size_rule(reader.word()?)?
        } else {
            None
        };
        let min_width = (flags & PACKED_V3_PATCH_MIN_WIDTH != 0)
            .then(|| reader.u16())
            .transpose()?;
        let max_width = (flags & PACKED_V3_PATCH_MAX_WIDTH != 0)
            .then(|| reader.u16())
            .transpose()?;
        let min_height = (flags & PACKED_V3_PATCH_MIN_HEIGHT != 0)
            .then(|| reader.u16())
            .transpose()?;
        let max_height = (flags & PACKED_V3_PATCH_MAX_HEIGHT != 0)
            .then(|| reader.u16())
            .transpose()?;
        reader.finish()?;
        Ok(View::from_retained_decoration(
            base,
            RetainedDecoration {
                padding,
                background: None,
                foreground: None,
                border: None,
                style: StyleRef::direct(StyleSpec::new()),
                style_states: Vec::new(),
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
            },
        ))
    }

    fn read_decoration_value(
        &self,
        reader: &mut MetaReader<'_>,
        page: Option<&Arc<FastPage>>,
    ) -> Result<RetainedDecoration, FastError> {
        let flags = reader.word()?;
        if flags & !0x0fff != 0 || flags & 16 == 0 {
            return Err(FastError::batch());
        }
        let padding = if flags & 1 != 0 {
            Some(Insets::new(
                reader.u16()?,
                reader.u16()?,
                reader.u16()?,
                reader.u16()?,
            ))
        } else {
            None
        };
        let background = if flags & 2 != 0 {
            Some(self.read_color(reader, page)?)
        } else {
            None
        };
        let foreground = if flags & 4 != 0 {
            Some(self.read_color(reader, page)?)
        } else {
            None
        };
        let border = if flags & 8 != 0 {
            Some(self.read_border(reader, page)?)
        } else {
            None
        };
        let style = self.read_style(reader, page)?;
        let mut style_states = Vec::new();
        if flags & 32 != 0 {
            for _ in 0..reader.count()? {
                let key = self.read_string_ref(reader, page)?;
                let value = self.read_string_ref(reader, page)?;
                if key.is_empty() || value.is_empty() {
                    return Err(FastError::batch());
                }
                style_states.push((key, value));
            }
        }
        let width = if flags & 64 != 0 {
            decode_size_rule(reader.word()?)?
        } else {
            None
        };
        let height = if flags & 128 != 0 {
            decode_size_rule(reader.word()?)?
        } else {
            None
        };
        let min_width = (flags & 256 != 0).then(|| reader.u16()).transpose()?;
        let max_width = (flags & 512 != 0).then(|| reader.u16()).transpose()?;
        let min_height = (flags & 1024 != 0).then(|| reader.u16()).transpose()?;
        let max_height = (flags & 2048 != 0).then(|| reader.u16()).transpose()?;
        Ok(RetainedDecoration {
            padding,
            background,
            foreground,
            border,
            style,
            style_states,
            width,
            height,
            min_width,
            max_width,
            min_height,
            max_height,
        })
    }

    fn read_overflow(
        &self,
        offset: u32,
        length: u32,
        page: Option<&Arc<FastPage>>,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<iyon_tui::OverflowIndicator, FastError> {
        let mut reader =
            self.payload_from_offset(offset as usize, length as usize, meta_start, meta_end)?;
        let kind = reader.word()?;
        let overflow = match kind {
            OVERFLOW_NONE => iyon_tui::OverflowIndicator::None,
            OVERFLOW_ELLIPSIS => iyon_tui::OverflowIndicator::Ellipsis {
                style: self.read_style(&mut reader, page)?,
            },
            OVERFLOW_FOOTER => iyon_tui::OverflowIndicator::Footer {
                prefix: self.read_string_ref(&mut reader, page)?,
                style: self.read_style(&mut reader, page)?,
            },
            _ => return Err(FastError::batch()),
        };
        reader.finish()?;
        Ok(overflow)
    }

    fn payload_from_offset(
        &self,
        offset: usize,
        length: usize,
        meta_start: usize,
        meta_end: usize,
    ) -> Result<MetaReader<'_>, FastError> {
        let start = offset.checked_mul(4).ok_or_else(FastError::batch)?;
        let end = start
            .checked_add(length.checked_mul(4).ok_or_else(FastError::batch)?)
            .ok_or_else(FastError::batch)?;
        if start < meta_start || end > meta_end {
            return Err(FastError::batch());
        }
        Ok(MetaReader::new(&self.command[start..end]))
    }
}

#[derive(Clone, Copy)]
struct FastOp {
    opcode: u32,
    dst: u32,
    base: u32,
    node_low: u32,
    node_high: u32,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}

impl FastOp {
    fn read(bytes: &[u8], offset: usize) -> Option<Self> {
        let mut values = [0; FAST_OP_WORDS];
        for (index, value) in values.iter_mut().enumerate() {
            *value = read_u32(bytes, offset + index * 4)?;
        }
        Some(Self {
            opcode: values[0] & 0xffff,
            dst: values[1],
            base: values[2],
            node_low: values[3],
            node_high: values[4],
            a: values[5],
            b: values[6],
            c: values[7],
            d: values[8],
        })
    }

    fn node_id(self) -> u64 {
        (u64::from(self.node_high) << 32) | u64::from(self.node_low)
    }
    fn a64(self) -> u64 {
        (u64::from(self.b) << 32) | u64::from(self.a)
    }
    fn b16(self) -> Result<u16, FastError> {
        u16::try_from(self.b).map_err(|_| FastError::batch())
    }
    fn c16(self) -> Result<u16, FastError> {
        u16::try_from(self.c).map_err(|_| FastError::batch())
    }
}

struct MetaReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> MetaReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    fn word(&mut self) -> Result<u32, FastError> {
        let value = read_u32(self.bytes, self.cursor).ok_or_else(FastError::batch)?;
        self.cursor += 4;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, FastError> {
        u16::try_from(self.word()?).map_err(|_| FastError::batch())
    }
    fn positive_u16(&mut self) -> Result<u16, FastError> {
        let value = self.u16()?;
        if value == 0 {
            Err(FastError::batch())
        } else {
            Ok(value)
        }
    }
    fn count(&mut self) -> Result<usize, FastError> {
        let value = self.word()? as usize;
        if value > MAX_COUNT {
            Err(FastError::batch())
        } else {
            Ok(value)
        }
    }
    fn u64(&mut self) -> Result<u64, FastError> {
        let low = u64::from(self.word()?);
        let high = u64::from(self.word()?);
        Ok(low | (high << 32))
    }
    fn positive_u64(&mut self) -> Result<u64, FastError> {
        let value = self.u64()?;
        if value == 0 || value > MAX_SAFE_INTEGER {
            Err(FastError::batch())
        } else {
            Ok(value)
        }
    }
    fn string_ref(&mut self) -> Result<(u32, u32), FastError> {
        Ok((self.word()?, self.word()?))
    }
    fn finish(&self) -> Result<(), FastError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(FastError::batch())
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    let end = offset + 4;
    if let Some(target) = bytes.get_mut(offset..end) {
        target.copy_from_slice(&value.to_le_bytes());
    }
}

fn retained_track(kind: u32, size: u16, max_rows: u16) -> RetainedAxisTrack {
    match kind {
        LAYOUT_CHILD_NORMAL => RetainedAxisTrack::Content,
        LAYOUT_CHILD_FIXED => RetainedAxisTrack::Fixed(size),
        LAYOUT_CHILD_FLEX => RetainedAxisTrack::Flex,
        LAYOUT_CHILD_FLEX_MAX => RetainedAxisTrack::FlexMax(max_rows),
        LAYOUT_CHILD_CONTENT_MAX => RetainedAxisTrack::ContentMax(max_rows),
        _ => RetainedAxisTrack::Content,
    }
}

fn validate_child(
    kind: u32,
    size: u16,
    max_rows: u16,
    sequence_kind: u32,
) -> Result<(), FastError> {
    let valid = match kind {
        LAYOUT_CHILD_NORMAL | LAYOUT_CHILD_FLEX => size == 0 && max_rows == 0,
        LAYOUT_CHILD_FIXED => max_rows == 0,
        LAYOUT_CHILD_FLEX_MAX | LAYOUT_CHILD_CONTENT_MAX => {
            sequence_kind == super::PACKED_V3_SEQ_COLUMN && size == 0
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(FastError::batch())
    }
}

fn decode_wrap(value: u32) -> Result<WrapMode, FastError> {
    match value {
        WRAP_WORD_THEN_GRAPHEME => Ok(WrapMode::WordThenGrapheme),
        WRAP_GRAPHEME => Ok(WrapMode::Grapheme),
        WRAP_NO_WRAP => Ok(WrapMode::NoWrap),
        _ => Err(FastError::batch()),
    }
}
fn decode_align(value: u32) -> Result<HorizontalAlign, FastError> {
    match value {
        ALIGN_START => Ok(HorizontalAlign::Start),
        ALIGN_CENTER => Ok(HorizontalAlign::Center),
        ALIGN_END => Ok(HorizontalAlign::End),
        _ => Err(FastError::batch()),
    }
}
fn decode_vertical(value: u32) -> Result<VerticalAlign, FastError> {
    match value {
        VERTICAL_TOP => Ok(VerticalAlign::Top),
        VERTICAL_CENTER => Ok(VerticalAlign::Center),
        VERTICAL_BOTTOM => Ok(VerticalAlign::Bottom),
        _ => Err(FastError::batch()),
    }
}
fn decode_size_rule(value: u32) -> Result<Option<RetainedSizeRule>, FastError> {
    match value {
        1 => Ok(Some(RetainedSizeRule::Fit)),
        2 => Ok(Some(RetainedSizeRule::Fill)),
        _ => Err(FastError::batch()),
    }
}

fn session_ptr(
    runtime_pointer: *mut NativeViewRuntime,
    host_pointer: *mut iyon_tui::TuiHost,
) -> *mut FastSession {
    let Some(runtime) = (unsafe { runtime_pointer.as_mut() }) else {
        return std::ptr::null_mut();
    };
    if !runtime.valid_on_owner_thread() || host_pointer.is_null() {
        return std::ptr::null_mut();
    }
    runtime
        .fast_sessions
        .get(&(host_pointer as usize))
        .copied()
        .map(|pointer| pointer as *mut FastSession)
        .unwrap_or(std::ptr::null_mut())
}

pub fn render_ref(
    runtime_pointer: *mut NativeViewRuntime,
    host_pointer: *mut iyon_tui::TuiHost,
    generation: u32,
    reference: u32,
) -> i32 {
    let session = session_ptr(runtime_pointer, host_pointer);
    if session.is_null() {
        return FAST_BAD_SESSION;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let session = unsafe { &mut *session };
        if session.assert_thread().is_err() {
            return FAST_BAD_SESSION;
        }
        if session.generation != generation {
            return FAST_CACHE_MISS;
        }
        let Some(view) = session.resolve_view(reference).ok().flatten() else {
            return FAST_CACHE_MISS;
        };
        match unsafe { (&mut *host_pointer).render(view) } {
            Ok(()) => FAST_OK,
            Err(_) => FAST_INTERNAL,
        }
    }));
    result.unwrap_or(FAST_INTERNAL)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_fast_commit_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut iyon_tui::TuiHost,
) -> i32 {
    let session = session_ptr(runtime, host);
    if session.is_null() {
        return FAST_BAD_SESSION;
    }
    catch_unwind(AssertUnwindSafe(|| unsafe { (&mut *session).commit() })).unwrap_or(FAST_INTERNAL)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_fast_acquire_utf8_page_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut iyon_tui::TuiHost,
) -> i32 {
    let session = session_ptr(runtime, host);
    if session.is_null() {
        return FAST_BAD_SESSION;
    }
    catch_unwind(AssertUnwindSafe(|| unsafe {
        (&mut *session).acquire_page()
    }))
    .unwrap_or(FAST_INTERNAL)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_fast_release_client_page_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut iyon_tui::TuiHost,
    page_id: u32,
) -> i32 {
    let session = session_ptr(runtime, host);
    if session.is_null() {
        return FAST_BAD_SESSION;
    }
    catch_unwind(AssertUnwindSafe(|| unsafe {
        (&mut *session).release_page(page_id as usize)
    }))
    .unwrap_or(FAST_INTERNAL)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_fast_render_ref_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut iyon_tui::TuiHost,
    generation: u32,
    reference: u32,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        render_ref(runtime, host, generation, reference)
    }))
    .unwrap_or(FAST_INTERNAL)
}
