use std::collections::HashSet;
use std::sync::{Arc, Weak};

use napi::bindgen_prelude::Result;

use iyon_tui::{
    BorderEdges, BorderGlyphs, BorderSpec, ColorSpec, DiffHunk, DiffLine, DiffLineNumber,
    DiffLineOffset, DiffLineTermination, DiffRange, GridTrack, HorizontalAlign, Insets, Renderer,
    RetainedAxis, RetainedAxisChild, RetainedAxisTrack, RetainedDecoration, RetainedGridCell,
    RetainedGridCells, RetainedSizeRule, StyleRef, StyleSpec, TextAttribute, TextSpan,
    VerticalAlign, View, WrapMode,
};

use super::ViewRuntimeHandle;
use super::{
    ALIGN_CENTER, ALIGN_END, ALIGN_START, DIFF_ADDITION, DIFF_CONTEXT, DIFF_DELETION,
    DIFF_TERMINATED, DIFF_UNTERMINATED, GRID_TRACK_CONTENT, GRID_TRACK_CONTENT_MAX,
    GRID_TRACK_FIXED, GRID_TRACK_FLEX, GRID_TRACK_FLEX_MAX, LAYOUT_CHILD_CONTENT_MAX,
    LAYOUT_CHILD_FIXED, LAYOUT_CHILD_FLEX, LAYOUT_CHILD_FLEX_MAX, LAYOUT_CHILD_NORMAL,
    OVERFLOW_ELLIPSIS, OVERFLOW_FOOTER, OVERFLOW_NONE, PACKED_V4_COLD_CLOSURE,
    PACKED_V4_DEF_GRID_CELL_BRANCH, PACKED_V4_DEF_GRID_CELL_LEAF, PACKED_V4_DEF_SEQ_BRANCH,
    PACKED_V4_DEF_SEQ_LEAF, PACKED_V4_DEF_VIEW_FULL, PACKED_V4_HAS_UTF8, PACKED_V4_OP_RENDER,
    PACKED_V4_OP_RENDER_FOREST, PACKED_V4_PATCH_ALIGN, PACKED_V4_PATCH_AXIS,
    PACKED_V4_PATCH_DECORATION, PACKED_V4_PATCH_GAP, PACKED_V4_PATCH_GRID,
    PACKED_V4_PATCH_GRID_CELLS, PACKED_V4_PATCH_HEIGHT, PACKED_V4_PATCH_MAX_HEIGHT,
    PACKED_V4_PATCH_MAX_WIDTH, PACKED_V4_PATCH_MIN_HEIGHT, PACKED_V4_PATCH_MIN_WIDTH,
    PACKED_V4_PATCH_PADDING, PACKED_V4_PATCH_SEQUENCE, PACKED_V4_PATCH_TEXT, PACKED_V4_PATCH_VIEW,
    PACKED_V4_PATCH_WIDTH, PACKED_V4_PATCH_WRAP, PACKED_V4_PROTOCOL_VERSION,
    PACKED_V4_RESET_GENERATION, PACKED_V4_SEQ_BRANCH_FACTOR, PACKED_V4_SEQ_COLUMN,
    PACKED_V4_SEQ_ROW, PACKED_V4_WIRE_LOCAL_BIT, PACKED_VIEW_MAGIC, VERTICAL_BOTTOM,
    VERTICAL_CENTER, VERTICAL_TOP, VIEW_BRIDGE_SCHEMA_VERSION, VIEW_KIND_CLAMP, VIEW_KIND_COLUMN,
    VIEW_KIND_COMPONENT, VIEW_KIND_CONTAINER, VIEW_KIND_CONTENT_MAX, VIEW_KIND_DECORATED,
    VIEW_KIND_DIFF, VIEW_KIND_GRID, VIEW_KIND_HANGING, VIEW_KIND_ROW, VIEW_KIND_SPACER,
    VIEW_KIND_TEXT, WRAP_GRAPHEME, WRAP_NO_WRAP, WRAP_WORD_THEN_GRAPHEME,
};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const PAGE_SHIFT: usize = 12;
const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
const KNOWN_FLAGS: u32 = PACKED_V4_RESET_GENERATION | PACKED_V4_COLD_CLOSURE | PACKED_V4_HAS_UTF8;
const HEADER_WORDS: usize = 12;

fn inc(counter: iyon_tui::perf::Counter) {
    #[cfg(feature = "perf-counters")]
    iyon_tui::perf::inc(counter);
}

fn add(counter: iyon_tui::perf::Counter, value: usize) {
    #[cfg(feature = "perf-counters")]
    iyon_tui::perf::add(counter, value as u64);
}
const STYLE_BOLD: u32 = 1;
const STYLE_DIM: u32 = 2;
const STYLE_ITALIC: u32 = 4;
const STYLE_UNDERLINE: u32 = 8;
const STYLE_REVERSED: u32 = 16;
const STYLE_STRIKETHROUGH: u32 = 32;
const STYLE_ALL: u32 =
    STYLE_BOLD | STYLE_DIM | STYLE_ITALIC | STYLE_UNDERLINE | STYLE_REVERSED | STYLE_STRIKETHROUGH;

fn invalid(message: impl Into<String>) -> napi::Error {
    crate::NativeError::invalid_input(message)
}

fn cache_miss(reference: u32) -> napi::Error {
    crate::NativeError::coded(
        napi::Status::InvalidArg,
        "ION_PACKED_CACHE_MISS",
        format!("packed V4 reference {reference} is not retained"),
    )
}

#[derive(Clone)]
enum PackedSlot {
    Empty,
    View { weak: iyon_tui::WeakView },
    Sequence(Weak<V4Sequence>),
    GridSequence(Weak<V4GridSequence>),
}

#[derive(Clone)]
pub(super) struct PackedSlotTable {
    pages: Vec<Option<Arc<[PackedSlot]>>>,
    count: usize,
}

impl PackedSlotTable {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            count: 0,
        }
    }

    fn reset(&mut self) {
        self.pages.clear();
        self.count = 0;
    }

    fn page_offset(reference: u32) -> (usize, usize) {
        let value = reference as usize;
        (value >> PAGE_SHIFT, value & (PAGE_SIZE - 1))
    }

    fn page_mut(&mut self, page_index: usize) -> &mut [PackedSlot] {
        if self.pages.len() <= page_index {
            self.pages.resize_with(page_index + 1, || None);
        }
        let page = self.pages[page_index].get_or_insert_with(|| {
            Arc::from(
                std::iter::repeat_with(|| PackedSlot::Empty)
                    .take(PAGE_SIZE)
                    .collect::<Vec<_>>(),
            )
        });
        Arc::make_mut(page)
    }

    fn get(&self, reference: u32) -> Option<&PackedSlot> {
        let (page, offset) = Self::page_offset(reference);
        self.pages.get(page)?.as_ref()?.get(offset)
    }

    fn set(&mut self, reference: u32, value: PackedSlot) {
        let page = reference as usize >> PAGE_SHIFT;
        let offset = reference as usize & (PAGE_SIZE - 1);
        let was_empty = matches!(self.get(reference), None | Some(PackedSlot::Empty));
        self.page_mut(page)[offset] = value;
        if was_empty && !matches!(self.get(reference), None | Some(PackedSlot::Empty)) {
            self.count += 1;
        }
    }

    fn view(&self, reference: u32) -> Option<View> {
        match self.get(reference)? {
            PackedSlot::View { weak, .. } => weak.upgrade(),
            _ => None,
        }
    }

    fn sequence(&self, reference: u32) -> Option<Arc<V4Sequence>> {
        match self.get(reference)? {
            PackedSlot::Sequence(weak) => weak.upgrade(),
            _ => None,
        }
    }

    fn grid_sequence(&self, reference: u32) -> Option<Arc<V4GridSequence>> {
        match self.get(reference)? {
            PackedSlot::GridSequence(weak) => weak.upgrade(),
            _ => None,
        }
    }

    fn slot_count(&self) -> usize {
        self.count
    }

    fn page_count(&self) -> usize {
        self.pages.iter().filter(|page| page.is_some()).count()
    }

    fn snapshot(&self) -> Self {
        self.clone()
    }
}

pub(super) struct PackedState {
    pub(super) generation: u32,
    pub(super) slots: PackedSlotTable,
}

impl PackedState {
    pub(super) fn new() -> Self {
        Self {
            generation: 0,
            slots: PackedSlotTable::new(),
        }
    }
    pub(super) fn reset_slots(&mut self) {
        self.slots.reset();
    }
    pub(super) fn slot_count(&self) -> usize {
        self.slots.slot_count()
    }
    pub(super) fn page_count(&self) -> usize {
        self.slots.page_count()
    }
}

#[derive(Clone)]
struct V4Child {
    kind: u32,
    size: u16,
    max_rows: u16,
    view: View,
}

// The child Arcs are retained deliberately so persistent-ref slots for
// descendants remain valid while their parent View is alive.
#[allow(dead_code)]
#[derive(Clone)]
enum V4Sequence {
    Leaf {
        kind: u32,
        aggregate: u32,
        items: Arc<[V4Child]>,
        retained: RetainedAxis,
    },
    Branch {
        kind: u32,
        height: u32,
        aggregate: u32,
        children: Arc<[Arc<V4Sequence>]>,
        sizes: Arc<[u32]>,
        retained: RetainedAxis,
    },
}

impl V4Sequence {
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

    fn retained_axis(&self) -> RetainedAxis {
        match self {
            Self::Leaf { retained, .. } | Self::Branch { retained, .. } => retained.clone(),
        }
    }

    fn aggregate(&self) -> u32 {
        match self {
            Self::Leaf { aggregate, .. } | Self::Branch { aggregate, .. } => *aggregate,
        }
    }
}

#[derive(Clone)]
struct V4GridCell {
    row: usize,
    column: usize,
    row_span: u16,
    column_span: u16,
    horizontal_align: HorizontalAlign,
    vertical_align: VerticalAlign,
    view: View,
}

// See V4Sequence: descendant sequence Arcs are lifetime anchors for weak
// packed slots, even when decoding otherwise reads the retained root.
#[allow(dead_code)]
#[derive(Clone)]
enum V4GridSequence {
    Leaf {
        aggregate: u32,
        items: Arc<[V4GridCell]>,
        retained: RetainedGridCells,
    },
    Branch {
        height: u32,
        aggregate: u32,
        children: Arc<[Arc<V4GridSequence>]>,
        sizes: Arc<[u32]>,
        retained: RetainedGridCells,
    },
}

impl V4GridSequence {
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
enum Staged {
    View { node_id: u64, view: View },
    Sequence(Arc<V4Sequence>),
    GridSequence(Arc<V4GridSequence>),
}

pub fn resolve_ref(generation: i64, packed_ref: i64, cache: ViewRuntimeHandle) -> Result<View> {
    inc(iyon_tui::perf::Counter::NapiV4ExactRefCalls);
    let generation =
        u32::try_from(generation).map_err(|_| invalid("packed V4 generation must fit in u32"))?;
    let packed_ref = persistent_ref(packed_ref)?;
    let (cache_generation, view) = super::with_view_runtime(&cache, |cache| {
        (
            cache.packed_v4.generation,
            cache.packed_v4.slots.view(packed_ref),
        )
    })?;
    if cache_generation != generation {
        return Err(cache_miss(packed_ref));
    }
    if let Some(view) = view {
        inc(iyon_tui::perf::Counter::NapiV4PersistentRefUpgrades);
        return Ok(view);
    }
    inc(iyon_tui::perf::Counter::NapiV4PersistentRefMisses);
    Err(cache_miss(packed_ref))
}

pub fn decode_render(words: &[u32], bytes: &[u8], cache: ViewRuntimeHandle) -> Result<View> {
    inc(iyon_tui::perf::Counter::NapiV4Transactions);
    let mut transaction = V4Transaction::new(words, bytes, cache)?;
    transaction.decode_definitions()?;
    let root = transaction.decode_operation()?;
    transaction.publish()?;
    Ok(root)
}

struct V4Transaction<'a> {
    words: &'a [u32],
    bytes: &'a [u8],
    offsets: Vec<u32>,
    retained_strings: Vec<Option<String>>,
    records_end: usize,
    cursor: usize,
    definitions: Vec<Staged>,
    refs: HashSet<u32>,
    cache: ViewRuntimeHandle,
    slots: PackedSlotTable,
    generation: u32,
    cold: bool,
    definition_count: usize,
    staged_refs: Vec<(u32, Staged)>,
}

impl<'a> V4Transaction<'a> {
    fn new(words: &'a [u32], bytes: &'a [u8], cache: ViewRuntimeHandle) -> Result<Self> {
        if words.len() < HEADER_WORDS {
            return Err(invalid("packed V4 header is truncated"));
        }
        if words[0] != PACKED_VIEW_MAGIC {
            return Err(invalid("unknown packed V4 magic"));
        }
        if words[1] != PACKED_V4_PROTOCOL_VERSION {
            return Err(invalid("unsupported packed V4 protocol version"));
        }
        if words[2] != VIEW_BRIDGE_SCHEMA_VERSION {
            return Err(invalid("packed V4 bridge schema mismatch"));
        }
        let generation = words[3];
        let flags = words[4];
        if flags & !KNOWN_FLAGS != 0 {
            return Err(invalid("packed V4 flags contain unknown mandatory bits"));
        }
        let has_utf8 = flags & PACKED_V4_HAS_UTF8 != 0;
        let used_words =
            usize::try_from(words[5]).map_err(|_| invalid("packed V4 word count is invalid"))?;
        if used_words < HEADER_WORDS || used_words > words.len() {
            return Err(invalid("packed V4 word count is out of bounds"));
        }
        let used_bytes =
            usize::try_from(words[6]).map_err(|_| invalid("packed V4 byte count is invalid"))?;
        if used_bytes > bytes.len() {
            return Err(invalid("packed V4 byte count is out of bounds"));
        }
        if has_utf8 != (used_bytes != 0) {
            return Err(invalid("packed V4 UTF-8 flag does not match byte count"));
        }
        let root_count =
            usize::try_from(words[7]).map_err(|_| invalid("packed V4 root count is invalid"))?;
        if root_count != 1 {
            return Err(invalid("packed V4 requires exactly one root"));
        }
        let definition_count = usize::try_from(words[8])
            .map_err(|_| invalid("packed V4 definition count is invalid"))?;
        let records_end =
            usize::try_from(words[9]).map_err(|_| invalid("packed V4 records end is invalid"))?;
        let string_count =
            usize::try_from(words[10]).map_err(|_| invalid("packed V4 string count is invalid"))?;
        let offsets_start = usize::try_from(words[11])
            .map_err(|_| invalid("packed V4 offset-table start is invalid"))?;
        if records_end < HEADER_WORDS || records_end > offsets_start || offsets_start > used_words {
            return Err(invalid(
                "packed V4 record and offset-table bounds are invalid",
            ));
        }
        if offsets_start.checked_add(string_count + 1) != Some(used_words) {
            return Err(invalid("packed V4 offset-table length is invalid"));
        }
        if offsets_start != records_end {
            return Err(invalid("packed V4 records must end at the offset table"));
        }
        if used_bytes == 0 && string_count != 0 {
            return Err(invalid("packed V4 empty byte lane has string entries"));
        }
        let byte_prefix = &bytes[..used_bytes];
        add(iyon_tui::perf::Counter::NapiV4BytesBorrowed, used_bytes);
        let all_utf8 = std::str::from_utf8(byte_prefix)
            .map_err(|_| invalid("packed V4 byte lane is not valid UTF-8"))?;
        inc(iyon_tui::perf::Counter::NapiV4Utf8Validations);
        let raw_offsets = &words[offsets_start..used_words];
        if raw_offsets.first().copied() != Some(0) {
            return Err(invalid("packed V4 offset table must start at zero"));
        }
        let mut previous = 0usize;
        for (index, raw) in raw_offsets.iter().copied().enumerate() {
            let current =
                usize::try_from(raw).map_err(|_| invalid("packed V4 string offset is invalid"))?;
            if current < previous || current > used_bytes || !all_utf8.is_char_boundary(current) {
                return Err(invalid("packed V4 string offsets are invalid"));
            }
            if index > 0 && current == previous {
                return Err(invalid("packed V4 string entries must be non-empty"));
            }
            previous = current;
        }
        if previous != used_bytes {
            return Err(invalid(
                "packed V4 final string offset does not match byte count",
            ));
        }
        let cold = flags & PACKED_V4_COLD_CLOSURE != 0;
        if cold != (flags & PACKED_V4_RESET_GENERATION != 0) {
            return Err(invalid("packed V4 reset and cold-closure flags must agree"));
        }
        let (cache_generation, slots) = super::with_view_runtime(&cache, |cache| {
            (cache.packed_v4.generation, cache.packed_v4.slots.snapshot())
        })?;
        if flags & PACKED_V4_RESET_GENERATION == 0 && generation != cache_generation {
            return Err(cache_miss(0));
        }
        if flags & PACKED_V4_RESET_GENERATION != 0 {
            let expected = cache_generation
                .checked_add(1)
                .ok_or_else(|| invalid("packed V4 generation exhausted"))?;
            if generation != expected {
                return Err(invalid(
                    "packed V4 reset generation is not the next generation",
                ));
            }
        }
        Ok(Self {
            words: &words[..used_words],
            bytes: byte_prefix,
            offsets: raw_offsets.to_vec(),
            retained_strings: vec![None; string_count],
            records_end,
            cursor: HEADER_WORDS,
            definitions: Vec::with_capacity(definition_count),
            refs: HashSet::new(),
            cache,
            slots,
            generation,
            cold,
            definition_count,
            staged_refs: Vec::with_capacity(definition_count),
        })
    }

    fn decode_definitions(&mut self) -> Result<()> {
        for _ in 0..self.definition_count {
            let start = self.cursor;
            let tag = self.word("definition tag")?;
            let record_words = self.count("definition length")?;
            if record_words < 3 || start.checked_add(record_words).is_none() {
                return Err(invalid("packed V4 definition length is invalid"));
            }
            let end = start + record_words;
            if end > self.records_end {
                return Err(invalid("packed V4 definition exceeds record section"));
            }
            let raw_reference = self.word("definition ref")?;
            let reference = persistent_ref(raw_reference as i64)?;
            if !self.refs.insert(reference) {
                return Err(invalid("packed V4 duplicate persistent ref"));
            }
            let value = match tag {
                PACKED_V4_DEF_VIEW_FULL => {
                    inc(iyon_tui::perf::Counter::NapiV4FullViewsBuilt);
                    self.decode_full(end)?
                }
                PACKED_V4_PATCH_VIEW => {
                    inc(iyon_tui::perf::Counter::NapiV4ViewsPatched);
                    self.decode_patch(end)?
                }
                PACKED_V4_DEF_SEQ_LEAF => {
                    inc(iyon_tui::perf::Counter::NapiV4SeqNodesBuilt);
                    self.decode_sequence_leaf(end)?
                }
                PACKED_V4_DEF_SEQ_BRANCH => {
                    inc(iyon_tui::perf::Counter::NapiV4SeqNodesBuilt);
                    self.decode_sequence_branch(end)?
                }
                PACKED_V4_DEF_GRID_CELL_LEAF => {
                    inc(iyon_tui::perf::Counter::NapiV4SeqNodesBuilt);
                    self.decode_grid_sequence_leaf(end)?
                }
                PACKED_V4_DEF_GRID_CELL_BRANCH => {
                    inc(iyon_tui::perf::Counter::NapiV4SeqNodesBuilt);
                    self.decode_grid_sequence_branch(end)?
                }
                _ => return Err(invalid(format!("unknown packed V4 definition tag {tag}"))),
            };
            if self.cursor != end {
                return Err(invalid(
                    "packed V4 definition length does not match payload",
                ));
            }
            self.definitions.push(value.clone());
            self.staged_refs.push((reference, value));
        }
        Ok(())
    }

    fn decode_operation(&mut self) -> Result<View> {
        let start = self.cursor;
        let tag = self.word("operation tag")?;
        let length = self.count("operation length")?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| invalid("packed V4 operation length overflow"))?;
        if end > self.records_end || length < 3 {
            return Err(invalid("packed V4 operation is out of bounds"));
        }
        let root_count = self.count("operation root count")?;
        if tag == PACKED_V4_OP_RENDER && root_count != 1 {
            return Err(invalid(
                "packed V4 render operation requires exactly one root",
            ));
        }
        if tag == PACKED_V4_OP_RENDER_FOREST && root_count == 0 {
            return Err(invalid(
                "packed V4 forest operation requires at least one root",
            ));
        }
        if tag != PACKED_V4_OP_RENDER && tag != PACKED_V4_OP_RENDER_FOREST {
            return Err(invalid("unknown packed V4 operation"));
        }
        if tag == PACKED_V4_OP_RENDER_FOREST && root_count != 1 {
            return Err(invalid(
                "packed V4 forest operation requires a forest-capable host boundary",
            ));
        }
        let mut roots = Vec::with_capacity(root_count);
        for _ in 0..root_count {
            roots.push(self.resolve_next_view("operation root")?);
        }
        if self.cursor != end || end != self.records_end {
            return Err(invalid("packed V4 operation has trailing words"));
        }
        roots
            .into_iter()
            .next()
            .ok_or_else(|| invalid("packed V4 render requires a root"))
    }

    fn decode_full(&mut self, _end: usize) -> Result<Staged> {
        let node_id = self.node_id()?;
        let view = match self.word("view kind")? {
            VIEW_KIND_TEXT => self.decode_text()?,
            VIEW_KIND_DIFF => self.decode_diff()?,
            VIEW_KIND_SPACER => View::spacer(self.u16("spacer rows")?),
            VIEW_KIND_ROW | VIEW_KIND_COLUMN => match self.decode_axis()? {
                Staged::View { view, .. } => view,
                Staged::Sequence(_) | Staged::GridSequence(_) => {
                    return Err(invalid("packed V4 axis did not decode to a View"));
                }
            },
            VIEW_KIND_HANGING => View::from_retained_hanging(
                self.resolve_next_view("hanging prefix")?,
                self.resolve_next_view("hanging continuation")?,
                self.resolve_next_view("hanging body")?,
            )
            .map_err(invalid)?,
            VIEW_KIND_GRID => match self.decode_grid()? {
                Staged::View { view, .. } => view,
                Staged::Sequence(_) | Staged::GridSequence(_) => {
                    return Err(invalid("packed V4 grid did not decode to a View"));
                }
            },
            VIEW_KIND_CONTAINER => {
                View::from_retained_container(self.resolve_next_view("container child")?)
            }
            VIEW_KIND_CLAMP => {
                let max_rows = self.u16("clamp maxRows")?;
                let overflow = self.decode_overflow()?;
                View::from_retained_clamp(
                    self.resolve_next_view("clamp child")?,
                    max_rows,
                    overflow,
                )
            }
            VIEW_KIND_CONTENT_MAX => {
                let max_rows = self.u16("contentMax maxRows")?;
                View::from_retained_clamp(
                    self.resolve_next_view("contentMax child")?,
                    max_rows,
                    iyon_tui::OverflowIndicator::None,
                )
            }
            VIEW_KIND_COMPONENT => {
                View::from_retained_component(self.positive_safe("component handle")?)
            }
            VIEW_KIND_DECORATED => {
                let child = self.resolve_next_view("decorated child")?;
                self.decode_decoration(child)?
            }
            other => return Err(invalid(format!("unknown packed V4 View kind {other}"))),
        };
        Ok(Staged::View { node_id, view })
    }

    fn decode_patch(&mut self, _end: usize) -> Result<Staged> {
        let node_id = self.node_id()?;
        let base = self.resolve_next_view("patch base")?;
        let kind = self.word("patch kind")?;
        let mask = self.word("patch mask")?;
        let view = match kind {
            PACKED_V4_PATCH_TEXT => {
                if mask == 0 || mask & !(PACKED_V4_PATCH_WRAP | PACKED_V4_PATCH_ALIGN) != 0 {
                    return Err(invalid("packed V4 text patch mask is invalid"));
                }
                let wrap = if mask & PACKED_V4_PATCH_WRAP != 0 {
                    Some(decode_wrap(self.word("patch wrap")?)?)
                } else {
                    None
                };
                let align = if mask & PACKED_V4_PATCH_ALIGN != 0 {
                    Some(decode_horizontal_align(self.word("patch align")?)?)
                } else {
                    None
                };
                let (base_wrap, base_align) = base
                    .retained_text_layout()
                    .ok_or_else(|| invalid("packed V4 text patch base is not text"))?;
                if wrap.is_none_or(|value| value == base_wrap)
                    && align.is_none_or(|value| value == base_align)
                {
                    return Err(invalid("packed V4 text patch is unchanged"));
                }
                base.with_text_layout_patch(wrap, align)
            }
            PACKED_V4_PATCH_DECORATION => {
                if mask == 0
                    || mask
                        & !(PACKED_V4_PATCH_PADDING
                            | PACKED_V4_PATCH_WIDTH
                            | PACKED_V4_PATCH_HEIGHT
                            | PACKED_V4_PATCH_MIN_WIDTH
                            | PACKED_V4_PATCH_MAX_WIDTH
                            | PACKED_V4_PATCH_MIN_HEIGHT
                            | PACKED_V4_PATCH_MAX_HEIGHT)
                        != 0
                {
                    return Err(invalid("packed V4 decoration patch mask is invalid"));
                }
                let padding = if mask & PACKED_V4_PATCH_PADDING != 0 {
                    Some(Insets::new(
                        self.u16("patch padding top")?,
                        self.u16("patch padding right")?,
                        self.u16("patch padding bottom")?,
                        self.u16("patch padding left")?,
                    ))
                } else {
                    None
                };
                let width = if mask & PACKED_V4_PATCH_WIDTH != 0 {
                    Some(match self.word("patch width")? {
                        1 => RetainedSizeRule::Fit,
                        2 => RetainedSizeRule::Fill,
                        _ => return Err(invalid("packed V4 patch width rule is invalid")),
                    })
                } else {
                    None
                };
                let height = if mask & PACKED_V4_PATCH_HEIGHT != 0 {
                    Some(match self.word("patch height")? {
                        1 => RetainedSizeRule::Fit,
                        2 => RetainedSizeRule::Fill,
                        _ => return Err(invalid("packed V4 patch height rule is invalid")),
                    })
                } else {
                    None
                };
                let min_width = (mask & PACKED_V4_PATCH_MIN_WIDTH != 0)
                    .then(|| self.u16("patch minWidth"))
                    .transpose()?;
                let max_width = (mask & PACKED_V4_PATCH_MAX_WIDTH != 0)
                    .then(|| self.u16("patch maxWidth"))
                    .transpose()?;
                let min_height = (mask & PACKED_V4_PATCH_MIN_HEIGHT != 0)
                    .then(|| self.u16("patch minHeight"))
                    .transpose()?;
                let max_height = (mask & PACKED_V4_PATCH_MAX_HEIGHT != 0)
                    .then(|| self.u16("patch maxHeight"))
                    .transpose()?;
                View::from_retained_decoration(
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
                )
            }
            PACKED_V4_PATCH_GRID => {
                if mask != PACKED_V4_PATCH_GRID_CELLS {
                    return Err(invalid("packed V4 grid patch mask is invalid"));
                }
                let sequence = self.resolve_next_grid_sequence("grid patch cell sequence")?;
                base.patch_retained_grid(sequence.retained())
                    .map_err(invalid)?
                    .retain_transport_payload(sequence.clone())
            }
            PACKED_V4_PATCH_AXIS => {
                if mask & !(PACKED_V4_PATCH_GAP | PACKED_V4_PATCH_SEQUENCE) != 0
                    || mask & PACKED_V4_PATCH_SEQUENCE == 0
                {
                    return Err(invalid("packed V4 axis patch mask is invalid"));
                }
                let horizontal = base
                    .retained_axis_horizontal()
                    .ok_or_else(|| invalid("packed V4 axis patch base is not an axis"))?;
                let sequence = self.resolve_next_sequence("axis patch sequence")?;
                if sequence.kind()
                    != if horizontal {
                        PACKED_V4_SEQ_ROW
                    } else {
                        PACKED_V4_SEQ_COLUMN
                    }
                {
                    return Err(invalid("packed V4 axis patch sequence kind is invalid"));
                }
                let gap = if mask & PACKED_V4_PATCH_GAP != 0 {
                    self.u16("axis patch gap")?
                } else {
                    base.retained_axis_gap()
                        .ok_or_else(|| invalid("packed V4 axis patch base gap is unavailable"))?
                };
                base.patch_retained_axis(sequence.retained_axis(), gap)
                    .retain_transport_payload(sequence.clone())
            }
            _ => return Err(invalid("unknown packed V4 patch kind")),
        };
        Ok(Staged::View { node_id, view })
    }

    fn resolve_next_view(&mut self, name: &str) -> Result<View> {
        let reference = self.word(name)?;
        self.resolve_view(reference)
    }

    fn resolve_next_sequence(&mut self, name: &str) -> Result<Arc<V4Sequence>> {
        let reference = self.word(name)?;
        self.resolve_sequence(reference)
    }

    fn resolve_next_grid_sequence(&mut self, name: &str) -> Result<Arc<V4GridSequence>> {
        let reference = self.word(name)?;
        self.resolve_grid_sequence(reference)
    }

    fn decode_sequence_leaf(&mut self, _end: usize) -> Result<Staged> {
        let sequence_kind = self.word("sequence kind")?;
        if sequence_kind != PACKED_V4_SEQ_ROW && sequence_kind != PACKED_V4_SEQ_COLUMN {
            return Err(invalid("packed V4 sequence leaf kind is invalid"));
        }
        let count = self.count("sequence leaf count")?;
        add(
            iyon_tui::perf::Counter::PersistentSeqItemsIteratedDuringPatch,
            count,
        );
        if count > PACKED_V4_SEQ_BRANCH_FACTOR as usize {
            return Err(invalid("packed V4 sequence leaf exceeds branch factor"));
        }
        let aggregate = self.word("sequence aggregate")?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let child_kind = self.word("sequence child kind")?;
            let size = self.u16("sequence child size")?;
            let max_rows = self.u16("sequence child maxRows")?;
            let view = self.resolve_next_view("sequence child view")?;
            validate_sequence_child(child_kind, size, max_rows, sequence_kind)?;
            items.push(V4Child {
                kind: child_kind,
                size,
                max_rows,
                view,
            });
        }
        let horizontal = sequence_kind == PACKED_V4_SEQ_ROW;
        let retained = RetainedAxis::leaf(
            horizontal,
            items
                .iter()
                .map(|item| RetainedAxisChild {
                    track: retained_track(item.kind, item.size, item.max_rows),
                    view: item.view.clone(),
                })
                .collect(),
        );
        if aggregate != u32::from(retained.aggregate_flags()) {
            return Err(invalid("packed V4 sequence leaf aggregate is invalid"));
        }
        Ok(Staged::Sequence(Arc::new(V4Sequence::Leaf {
            kind: sequence_kind,
            aggregate,
            items: items.into(),
            retained,
        })))
    }

    fn decode_sequence_branch(&mut self, _end: usize) -> Result<Staged> {
        let kind = self.word("sequence kind")?;
        if kind != PACKED_V4_SEQ_ROW && kind != PACKED_V4_SEQ_COLUMN {
            return Err(invalid("packed V4 sequence branch kind is invalid"));
        }
        let height = self.word("sequence height")?;
        let count = self.count("sequence branch count")?;
        add(
            iyon_tui::perf::Counter::PersistentSeqItemsIteratedDuringPatch,
            count,
        );
        if count == 0 || count > PACKED_V4_SEQ_BRANCH_FACTOR as usize {
            return Err(invalid("packed V4 sequence branch count is invalid"));
        }
        let aggregate = self.word("sequence aggregate")?;
        let mut children = Vec::with_capacity(count);
        let mut sizes = Vec::with_capacity(count);
        let mut previous = 0;
        for _ in 0..count {
            let size = self.word("sequence cumulative size")?;
            if size <= previous {
                return Err(invalid("packed V4 sequence cumulative sizes are invalid"));
            }
            previous = size;
            sizes.push(size);
            children.push(self.resolve_next_sequence("sequence child")?);
        }
        if children.iter().any(|child| {
            child.kind() != kind || child.length() == 0 || child.height() + 1 != height
        }) {
            return Err(invalid(
                "packed V4 sequence child kind, height, or size is invalid",
            ));
        }
        if sizes.last().copied().unwrap_or(0)
            != children.iter().map(|child| child.length()).sum::<u32>()
        {
            return Err(invalid("packed V4 sequence length aggregate is invalid"));
        }
        let calculated_aggregate = children
            .iter()
            .fold(0, |flags, child| flags | child.aggregate());
        if aggregate != calculated_aggregate {
            return Err(invalid("packed V4 sequence branch aggregate is invalid"));
        }
        let retained = RetainedAxis::branch(
            kind == PACKED_V4_SEQ_ROW,
            children
                .iter()
                .map(|child| child.retained_axis())
                .collect::<Vec<_>>(),
        )
        .map_err(invalid)?;
        Ok(Staged::Sequence(Arc::new(V4Sequence::Branch {
            kind,
            height,
            aggregate,
            children: children.into(),
            sizes: sizes.into(),
            retained,
        })))
    }

    fn decode_grid_sequence_leaf(&mut self, _end: usize) -> Result<Staged> {
        let count = self.count("grid sequence leaf count")?;
        add(
            iyon_tui::perf::Counter::PersistentSeqItemsIteratedDuringPatch,
            count,
        );
        if count > PACKED_V4_SEQ_BRANCH_FACTOR as usize {
            return Err(invalid(
                "packed V4 grid sequence leaf exceeds branch factor",
            ));
        }
        let aggregate = self.word("grid sequence aggregate")?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let row = self.count("grid cell row")?;
            let column = self.count("grid cell column")?;
            let column_span = self.positive_u16("grid cell columnSpan")?;
            let row_span = self.positive_u16("grid cell rowSpan")?;
            let horizontal_align =
                decode_horizontal_align(self.word("grid cell horizontal align")?)?;
            let vertical_align = decode_vertical_align(self.word("grid cell vertical align")?)?;
            let view = self.resolve_next_view("grid cell view")?;
            items.push(V4GridCell {
                row,
                column,
                row_span,
                column_span,
                horizontal_align,
                vertical_align,
                view,
            });
        }
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
        if aggregate != u32::from(retained.aggregate_flags()) {
            return Err(invalid("packed V4 grid sequence leaf aggregate is invalid"));
        }
        Ok(Staged::GridSequence(Arc::new(V4GridSequence::Leaf {
            aggregate,
            items: items.into(),
            retained,
        })))
    }

    fn decode_grid_sequence_branch(&mut self, _end: usize) -> Result<Staged> {
        let height = self.word("grid sequence height")?;
        let count = self.count("grid sequence branch count")?;
        add(
            iyon_tui::perf::Counter::PersistentSeqItemsIteratedDuringPatch,
            count,
        );
        if count == 0 || count > PACKED_V4_SEQ_BRANCH_FACTOR as usize {
            return Err(invalid("packed V4 grid sequence branch count is invalid"));
        }
        let aggregate = self.word("grid sequence aggregate")?;
        let mut children = Vec::with_capacity(count);
        let mut sizes = Vec::with_capacity(count);
        let mut previous = 0;
        for _ in 0..count {
            let size = self.word("grid sequence cumulative size")?;
            if size <= previous {
                return Err(invalid(
                    "packed V4 grid sequence cumulative sizes are invalid",
                ));
            }
            previous = size;
            sizes.push(size);
            children.push(self.resolve_next_grid_sequence("grid sequence child")?);
        }
        if children
            .iter()
            .any(|child| child.length() == 0 || child.height() + 1 != height)
        {
            return Err(invalid(
                "packed V4 grid sequence child height or size is invalid",
            ));
        }
        if sizes.last().copied().unwrap_or(0)
            != children.iter().map(|child| child.length()).sum::<u32>()
        {
            return Err(invalid(
                "packed V4 grid sequence length aggregate is invalid",
            ));
        }
        let calculated = children
            .iter()
            .fold(0, |flags, child| flags | child.aggregate());
        if aggregate != calculated {
            return Err(invalid(
                "packed V4 grid sequence branch aggregate is invalid",
            ));
        }
        let retained =
            RetainedGridCells::branch(children.iter().map(|child| child.retained()).collect())
                .map_err(invalid)?;
        Ok(Staged::GridSequence(Arc::new(V4GridSequence::Branch {
            height,
            aggregate,
            children: children.into(),
            sizes: sizes.into(),
            retained,
        })))
    }

    fn decode_axis(&mut self) -> Result<Staged> {
        let gap = self.u16("axis gap")?;
        let sequence = self.resolve_next_sequence("axis sequence")?;
        if sequence.kind() != PACKED_V4_SEQ_ROW && sequence.kind() != PACKED_V4_SEQ_COLUMN {
            return Err(invalid("packed V4 axis sequence kind is invalid"));
        }
        let view = sequence
            .retained_axis()
            .into_view(gap)
            .retain_transport_payload(sequence.clone());
        Ok(Staged::View { node_id: 0, view })
    }

    fn decode_grid(&mut self) -> Result<Staged> {
        let column_count = self.count("grid column count")?;
        let mut columns = Vec::with_capacity(column_count);
        for _ in 0..column_count {
            columns.push(self.grid_track()?);
        }
        let row_count = self.count("grid row count")?;
        let mut rows = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            rows.push(self.grid_track()?);
        }
        let cells = self.resolve_next_grid_sequence("grid cell sequence")?;
        let column_gap = self.u16("grid columnGap")?;
        let row_gap = self.u16("grid rowGap")?;
        let view = cells
            .retained()
            .into_view(columns, rows, column_gap, row_gap)
            .retain_transport_payload(cells.clone());
        Ok(Staged::View { node_id: 0, view })
    }

    fn decode_text(&mut self) -> Result<View> {
        let wrap = decode_wrap(self.word("text wrap")?)?;
        let align = decode_horizontal_align(self.word("text align")?)?;
        let count = self.count("text span count")?;
        let mut spans = Vec::with_capacity(count);
        for _ in 0..count {
            let text = self.string()?;
            let style = self.decode_style()?;
            spans.push(TextSpan::styled(text, style));
        }
        Ok(View::from_retained_text(spans, wrap, align))
    }

    fn decode_diff(&mut self) -> Result<View> {
        let hunk_count = self.count("diff hunk count")?;
        let mut hunks = Vec::with_capacity(hunk_count);
        for _ in 0..hunk_count {
            let old_range = DiffRange::new(
                DiffLineOffset::new(self.safe_nonnegative("old range start")?),
                self.safe_nonnegative("old range count")?,
            )
            .map_err(|e| invalid(e.to_string()))?;
            let new_range = DiffRange::new(
                DiffLineOffset::new(self.safe_nonnegative("new range start")?),
                self.safe_nonnegative("new range count")?,
            )
            .map_err(|e| invalid(e.to_string()))?;
            let line_count = self.count("diff line count")?;
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                let kind = self.word("diff line kind")?;
                let text = self.string()?;
                let termination = match self.word("diff line termination")? {
                    DIFF_TERMINATED => DiffLineTermination::Terminated,
                    DIFF_UNTERMINATED => DiffLineTermination::Unterminated,
                    _ => return Err(invalid("invalid diff line termination")),
                };
                let line = match kind {
                    DIFF_CONTEXT => {
                        DiffLine::context(self.diff_line_number()?, self.diff_line_number()?, text)
                    }
                    DIFF_ADDITION => DiffLine::addition(self.diff_line_number()?, text),
                    DIFF_DELETION => DiffLine::deletion(self.diff_line_number()?, text),
                    _ => return Err(invalid("invalid diff line kind")),
                };
                lines.push(line.with_termination(termination));
            }
            hunks.push(
                DiffHunk::new(old_range, new_range, lines).map_err(|e| invalid(e.to_string()))?,
            );
        }
        Ok(iyon_tui::DiffRenderer::new().render(hunks.as_slice()))
    }

    fn decode_overflow(&mut self) -> Result<iyon_tui::OverflowIndicator> {
        match self.word("overflow kind")? {
            OVERFLOW_NONE => Ok(iyon_tui::OverflowIndicator::None),
            OVERFLOW_ELLIPSIS => Ok(iyon_tui::OverflowIndicator::Ellipsis {
                style: self.decode_style()?,
            }),
            OVERFLOW_FOOTER => Ok(iyon_tui::OverflowIndicator::Footer {
                prefix: self.string()?,
                style: self.decode_style()?,
            }),
            _ => Err(invalid("invalid packed V4 overflow kind")),
        }
    }

    fn decode_decoration(&mut self, view: View) -> Result<View> {
        let flags = self.word("decoration flags")?;
        if flags & !0x0fff != 0 || flags & 16 == 0 {
            return Err(invalid("packed V4 decoration style is required"));
        }
        let padding = if flags & 1 != 0 {
            Some(Insets::new(
                self.u16("padding top")?,
                self.u16("padding right")?,
                self.u16("padding bottom")?,
                self.u16("padding left")?,
            ))
        } else {
            None
        };
        let background = if flags & 2 != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let foreground = if flags & 4 != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let border = if flags & 8 != 0 {
            Some(self.border()?)
        } else {
            None
        };
        let style = self.decode_style()?;
        let mut style_states = Vec::new();
        if flags & 32 != 0 {
            let count = self.count("style state count")?;
            style_states.reserve(count);
            for _ in 0..count {
                let key = self.string()?;
                let value = self.string()?;
                if key.is_empty() || value.is_empty() {
                    return Err(invalid(
                        "packed V4 style state key and value cannot be empty",
                    ));
                }
                style_states.push((key, value));
            }
        }
        let width = if flags & 64 != 0 {
            Some(match self.word("width rule")? {
                1 => RetainedSizeRule::Fit,
                2 => RetainedSizeRule::Fill,
                _ => return Err(invalid("packed V4 width rule is invalid")),
            })
        } else {
            None
        };
        let height = if flags & 128 != 0 {
            Some(match self.word("height rule")? {
                1 => RetainedSizeRule::Fit,
                2 => RetainedSizeRule::Fill,
                _ => return Err(invalid("packed V4 height rule is invalid")),
            })
        } else {
            None
        };
        let min_width = (flags & 256 != 0)
            .then(|| self.u16("min width"))
            .transpose()?;
        let max_width = (flags & 512 != 0)
            .then(|| self.u16("max width"))
            .transpose()?;
        let min_height = (flags & 1024 != 0)
            .then(|| self.u16("min height"))
            .transpose()?;
        let max_height = (flags & 2048 != 0)
            .then(|| self.u16("max height"))
            .transpose()?;
        Ok(View::from_retained_decoration(
            view,
            RetainedDecoration {
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
            },
        ))
    }

    fn decode_style(&mut self) -> Result<StyleRef> {
        let flags = self.word("style flags")?;
        if flags & !(1 | 2 | 4) != 0 {
            return Err(invalid("packed V4 style flags are invalid"));
        }
        let theme = if flags & 1 != 0 {
            Some(self.string()?)
        } else {
            None
        };
        let foreground = if flags & 2 != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let background = if flags & 4 != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let present = self.word("style attributes")?;
        let truth = self.word("style truth")?;
        if present & !STYLE_ALL != 0 || truth & !present != 0 {
            return Err(invalid("packed V4 style masks are invalid"));
        }
        let mut style = StyleSpec::new();
        if let Some(color) = foreground {
            style = style.foreground(color);
        }
        if let Some(color) = background {
            style = style.background(color);
        }
        for (bit, attribute) in [
            (STYLE_BOLD, TextAttribute::Bold),
            (STYLE_DIM, TextAttribute::Dim),
            (STYLE_ITALIC, TextAttribute::Italic),
            (STYLE_UNDERLINE, TextAttribute::Underline),
            (STYLE_REVERSED, TextAttribute::Reversed),
            (STYLE_STRIKETHROUGH, TextAttribute::Strikethrough),
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

    fn color(&mut self) -> Result<ColorSpec> {
        match self.word("color kind")? {
            1 => {
                let value = self.string()?;
                super::decode_color_string(&value)
            }
            2 => Ok(ColorSpec::ansi(
                u8::try_from(self.word("ansi color")?)
                    .map_err(|_| invalid("ANSI color must fit in u8"))?,
            )),
            _ => Err(invalid("invalid packed V4 color")),
        }
    }

    fn border(&mut self) -> Result<BorderSpec> {
        let flags = self.word("border flags")?;
        if flags & !0x0f != 0 {
            return Err(invalid("packed V4 border flags are invalid"));
        }
        let glyphs = if flags & 1 != 0 {
            Some([
                self.string()?,
                self.string()?,
                self.string()?,
                self.string()?,
                self.string()?,
                self.string()?,
                self.string()?,
                self.string()?,
            ])
        } else {
            None
        };
        let color = if flags & 2 != 0 {
            Some(self.color()?)
        } else {
            None
        };
        let style = if flags & 4 != 0 {
            Some(self.word("border style")?)
        } else {
            None
        };
        let edges = if flags & 8 != 0 {
            Some(self.word("border edges")?)
        } else {
            None
        };
        let mut spec = match style.unwrap_or(1) {
            1 => BorderSpec::plain(),
            2 => BorderSpec::rounded(),
            3 => BorderSpec::double(),
            _ => return Err(invalid("invalid packed V4 border style")),
        };
        if let Some(values) = glyphs {
            spec = BorderSpec::custom(
                BorderGlyphs::new(
                    &values[0], &values[1], &values[2], &values[3], &values[4], &values[5],
                    &values[6], &values[7],
                )
                .map_err(|e| invalid(e.to_string()))?,
            );
        }
        if edges == Some(2) {
            spec = spec.edges(BorderEdges::TOP_BOTTOM);
        }
        if edges.is_some_and(|value| value != 1 && value != 2) {
            return Err(invalid("invalid packed V4 border edges"));
        }
        if let Some(color) = color {
            spec = spec.color(color);
        }
        Ok(spec)
    }

    fn grid_track(&mut self) -> Result<GridTrack> {
        let kind = self.word("grid track kind")?;
        let value = self.u16("grid track value")?;
        match kind {
            GRID_TRACK_CONTENT if value == 0 => Ok(GridTrack::content()),
            GRID_TRACK_CONTENT_MAX => Ok(GridTrack::content_max(value)),
            GRID_TRACK_FIXED => Ok(GridTrack::fixed(value)),
            GRID_TRACK_FLEX if value == 0 => Ok(GridTrack::flex()),
            GRID_TRACK_FLEX_MAX => Ok(GridTrack::flex_max(value)),
            _ => Err(invalid("invalid packed V4 grid track")),
        }
    }

    fn resolve_view(&self, word: u32) -> Result<View> {
        match self.resolve(word)? {
            Staged::View { view, .. } => Ok(view),
            Staged::Sequence(_) | Staged::GridSequence(_) => {
                Err(invalid("packed V4 expected a View reference"))
            }
        }
    }
    fn resolve_sequence(&self, word: u32) -> Result<Arc<V4Sequence>> {
        match self.resolve(word)? {
            Staged::Sequence(sequence) => Ok(sequence),
            _ => Err(invalid("packed V4 expected an axis sequence reference")),
        }
    }

    fn resolve_grid_sequence(&self, word: u32) -> Result<Arc<V4GridSequence>> {
        match self.resolve(word)? {
            Staged::GridSequence(sequence) => Ok(sequence),
            _ => Err(invalid("packed V4 expected a grid sequence reference")),
        }
    }

    fn resolve(&self, word: u32) -> Result<Staged> {
        if word & PACKED_V4_WIRE_LOCAL_BIT != 0 {
            let index = (word & !PACKED_V4_WIRE_LOCAL_BIT) as usize;
            inc(iyon_tui::perf::Counter::NapiV4LocalRefResolves);
            return self
                .definitions
                .get(index)
                .cloned()
                .ok_or_else(|| invalid("packed V4 local reference is forward or out of range"));
        }
        if self.cold {
            return Err(invalid(
                "packed V4 cold closure contains a persistent reference",
            ));
        }
        let reference = persistent_ref(word as i64)?;
        if let Some(view) = self.slots.view(reference) {
            inc(iyon_tui::perf::Counter::NapiV4PersistentRefUpgrades);
            return Ok(Staged::View { node_id: 0, view });
        }
        if let Some(sequence) = self.slots.sequence(reference) {
            inc(iyon_tui::perf::Counter::NapiV4SeqNodesReused);
            inc(iyon_tui::perf::Counter::NapiV4PersistentRefUpgrades);
            return Ok(Staged::Sequence(sequence));
        }
        if let Some(sequence) = self.slots.grid_sequence(reference) {
            inc(iyon_tui::perf::Counter::NapiV4SeqNodesReused);
            inc(iyon_tui::perf::Counter::NapiV4PersistentRefUpgrades);
            return Ok(Staged::GridSequence(sequence));
        }
        inc(iyon_tui::perf::Counter::NapiV4PersistentRefMisses);
        Err(cache_miss(reference))
    }

    fn publish(&mut self) -> Result<()> {
        // The snapshot is only needed during decode. Release its shared page
        // Arcs before mutating the live table so publication does not clone a
        // 4096-slot page merely because validation used a read snapshot.
        self.slots.reset();
        let cache = super::runtime_from_handle(&self.cache)?;

        // Validate every semantic identity before changing either cache. This
        // keeps publication atomic even when a malicious or stale transaction
        // conflicts with an existing NodeId.
        let mut transaction_node_ids = HashSet::new();
        for (_, value) in &self.staged_refs {
            let Staged::View { node_id, view } = value else {
                continue;
            };
            if *node_id == 0 {
                continue;
            }
            if !transaction_node_ids.insert(*node_id) {
                return Err(invalid(format!(
                    "packed V4 duplicate NodeId {node_id} in transaction",
                )));
            }
            if let Some(existing) = cache
                .nodes
                .get(node_id)
                .and_then(iyon_tui::WeakView::upgrade)
                && !existing.eq(view)
            {
                return Err(invalid(format!(
                    "packed V4 NodeId {node_id} changed semantic identity",
                )));
            }
        }

        if self.cold {
            cache.packed_v4.slots.reset();
            cache.packed_v4.generation = self.generation;
        }
        for (reference, value) in self.staged_refs.drain(..) {
            inc(iyon_tui::perf::Counter::NapiV4CachePublications);
            match value {
                Staged::View { node_id, view } => {
                    if node_id != 0 {
                        cache
                            .publish_bulk(node_id, view.clone())
                            .map_err(|_| invalid("packed V4 native View publication failed"))?;
                    }
                    cache.packed_v4.slots.set(
                        reference,
                        PackedSlot::View {
                            weak: view.downgrade(),
                        },
                    );
                }
                Staged::Sequence(sequence) => {
                    cache
                        .packed_v4
                        .slots
                        .set(reference, PackedSlot::Sequence(Arc::downgrade(&sequence)));
                }
                Staged::GridSequence(sequence) => {
                    cache.packed_v4.slots.set(
                        reference,
                        PackedSlot::GridSequence(Arc::downgrade(&sequence)),
                    );
                }
            }
        }
        Ok(())
    }

    fn string(&mut self) -> Result<String> {
        let reference = self.word("string reference")? as usize;
        if reference == 0 {
            return Ok(String::new());
        }
        let index = reference - 1;
        let (start, end) = {
            let start = *self
                .offsets
                .get(index)
                .ok_or_else(|| invalid("packed V4 string reference is out of bounds"))?
                as usize;
            let end = *self
                .offsets
                .get(index + 1)
                .ok_or_else(|| invalid("packed V4 string reference is out of bounds"))?
                as usize;
            (start, end)
        };
        let source = self
            .retained_strings
            .get_mut(index)
            .ok_or_else(|| invalid("packed V4 string reference is out of bounds"))?;
        if source.is_none() {
            let bytes = &self.bytes[start..end];
            // SAFETY: the complete lane and every offset were validated in
            // V4Transaction::new before any record can access a StringRef.
            let value = unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned();
            add(
                iyon_tui::perf::Counter::NapiV4BytesCopiedToRetained,
                bytes.len(),
            );
            *source = Some(value);
        }
        Ok(source
            .as_ref()
            .expect("string source was initialized")
            .clone())
    }
    fn node_id(&mut self) -> Result<u64> {
        let low = self.word("NodeId low")? as u64;
        let high = self.word("NodeId high")? as u64;
        let value = (high << 32) | low;
        if value == 0 || value > MAX_SAFE_INTEGER {
            return Err(invalid("packed V4 NodeId is not a positive safe integer"));
        }
        Ok(value)
    }
    fn positive_safe(&mut self, name: &str) -> Result<u64> {
        let value = self.safe_nonnegative(name)?;
        if value == 0 {
            return Err(invalid(format!("{name} must be positive")));
        }
        Ok(value)
    }
    fn safe_nonnegative(&mut self, name: &str) -> Result<u64> {
        let low = self.word(name)? as u64;
        let high = self.word(name)? as u64;
        let value = (high << 32) | low;
        if value > MAX_SAFE_INTEGER {
            return Err(invalid(format!("{name} must fit in safe integer")));
        }
        Ok(value)
    }
    fn diff_line_number(&mut self) -> Result<DiffLineNumber> {
        DiffLineNumber::new(self.positive_safe("diff line number")?)
            .ok_or_else(|| invalid("diff line number must be positive"))
    }
    fn u16(&mut self, name: &str) -> Result<u16> {
        u16::try_from(self.word(name)?).map_err(|_| invalid(format!("{name} must fit in u16")))
    }
    fn positive_u16(&mut self, name: &str) -> Result<u16> {
        let value = self.u16(name)?;
        if value == 0 {
            return Err(invalid(format!("{name} must be positive")));
        }
        Ok(value)
    }
    fn count(&mut self, name: &str) -> Result<usize> {
        let value = self.word(name)? as usize;
        if value > 1_000_000 {
            return Err(invalid(format!("{name} is too large")));
        }
        Ok(value)
    }
    fn word(&mut self, name: &str) -> Result<u32> {
        let value = *self
            .words
            .get(self.cursor)
            .ok_or_else(|| invalid(format!("packed V4 transaction is missing {name}")))?;
        self.cursor += 1;
        inc(iyon_tui::perf::Counter::NapiV4WordsRead);
        Ok(value)
    }
}

fn retained_track(kind: u32, size: u16, max_rows: u16) -> RetainedAxisTrack {
    match kind {
        LAYOUT_CHILD_NORMAL => RetainedAxisTrack::Content,
        LAYOUT_CHILD_FIXED => RetainedAxisTrack::Fixed(size),
        LAYOUT_CHILD_FLEX => RetainedAxisTrack::Flex,
        LAYOUT_CHILD_FLEX_MAX => RetainedAxisTrack::FlexMax(max_rows),
        LAYOUT_CHILD_CONTENT_MAX => RetainedAxisTrack::ContentMax(max_rows),
        _ => unreachable!("sequence child was validated before retained conversion"),
    }
}

fn validate_sequence_child(kind: u32, size: u16, max_rows: u16, sequence_kind: u32) -> Result<()> {
    let valid = match kind {
        LAYOUT_CHILD_NORMAL | LAYOUT_CHILD_FLEX => size == 0 && max_rows == 0,
        LAYOUT_CHILD_FIXED => max_rows == 0,
        LAYOUT_CHILD_FLEX_MAX | LAYOUT_CHILD_CONTENT_MAX => {
            sequence_kind == PACKED_V4_SEQ_COLUMN && size == 0
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("packed V4 sequence child fields are invalid"))
    }
}

fn persistent_ref(value: i64) -> Result<u32> {
    let value =
        u32::try_from(value).map_err(|_| invalid("packed V4 persistent ref must fit u32"))?;
    if value == 0 || value >= PACKED_V4_WIRE_LOCAL_BIT {
        return Err(invalid("packed V4 persistent ref is out of range"));
    }
    Ok(value)
}
fn decode_wrap(value: u32) -> Result<WrapMode> {
    match value {
        WRAP_WORD_THEN_GRAPHEME => Ok(WrapMode::WordThenGrapheme),
        WRAP_GRAPHEME => Ok(WrapMode::Grapheme),
        WRAP_NO_WRAP => Ok(WrapMode::NoWrap),
        _ => Err(invalid("invalid packed V4 wrap mode")),
    }
}
fn decode_horizontal_align(value: u32) -> Result<HorizontalAlign> {
    match value {
        ALIGN_START => Ok(HorizontalAlign::Start),
        ALIGN_CENTER => Ok(HorizontalAlign::Center),
        ALIGN_END => Ok(HorizontalAlign::End),
        _ => Err(invalid("invalid packed V4 horizontal alignment")),
    }
}
fn decode_vertical_align(value: u32) -> Result<VerticalAlign> {
    match value {
        VERTICAL_TOP => Ok(VerticalAlign::Top),
        VERTICAL_CENTER => Ok(VerticalAlign::Center),
        VERTICAL_BOTTOM => Ok(VerticalAlign::Bottom),
        _ => Err(invalid("invalid packed V4 vertical alignment")),
    }
}
