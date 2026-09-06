//! Retained content-plane identities, Source storage, and inactive control state.
//!
//! Source storage is deliberately host-independent. This module owns the
//! PERF-13-E mutation boundary and the PERF-13-D lifecycle graph: environment-
//! owned Sources, host-owned Ports and Connectors, desired/visible mount state,
//! weak subscription bookkeeping, and the plain-text Connector projection.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::str;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

use anyhow::{Result, anyhow};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    geometry::Size,
    physical::{PhysicalCell, PhysicalRow, Surface},
    presentation::{
        ContentDirty, ContentDirtyReason, ContentMeasurement, ContentProvider, ContentWindow,
        HistoryContentRows, PreparedProjectionTicket,
    },
    projection::{Projection, ProjectionBuilder, Projector, Smooth, SmoothConfig},
    stream::{StreamOffset, StreamRange},
    text::{
        AnsiProjector, Block, DiffProjector, Inline, InlineContent, InlineKind, LiteralText,
        MarkdownOptions, MarkdownProjector, PlainTextProjector, RawText, TextContent,
        TextProjectionError, TextProvenance, TextRenderer, TextRewriter, TextRun,
        walk_rewrite_block, walk_rewrite_inline,
    },
    {AnsiColor, ColorSpec, StyleRef, StyleSpec, TextAttribute, Theme},
};

use super::environment::{EnvironmentIdentity, WakeDisposition};
use super::host::HostInner;
use super::source_store::{
    ChunkView, SourceAnnotation, StoredSource, ValidatedAnnotation, ValidatedInput,
};
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContentFamily {
    Text,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextSourceKind {
    Block,
    Stream,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextFunnelKind {
    Plain,
    Markdown,
    Diff,
    Ansi,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentDelivery {
    Immediate,
    Smooth(SmoothConfig),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextWrapMode {
    Word,
    Grapheme,
    NoWrap,
}

/// Fixed-width annotation envelope shared by the direct data ABI and the
/// native Source store. Offsets are operation-local UTF-8 byte coordinates;
/// the Source converts them to absolute coordinates while holding its mutex.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentAnnotationRecord {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub aux0: u32,
    pub aux1: u32,
}

const _: () = assert!(std::mem::size_of::<ContentAnnotationRecord>() == 32);
const _: () = assert!(std::mem::align_of::<ContentAnnotationRecord>() == 4);

/// Read-only annotation data exposed by a diagnostic Source snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentAnnotationSnapshot {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u64,
    pub end_byte: u64,
    pub payload: Vec<u8>,
    pub aux0: u32,
    pub aux1: u32,
}

/// Immutable, cheap-to-clone Source snapshot. The text/chunk storage is
/// shared by Arc; `text()` is an explicit diagnostic/materialization query and
/// is not used by the frame path.
#[derive(Clone, Debug)]
pub struct HostContentSourceSnapshot {
    pub source_id: u64,
    pub source_generation: u32,
    pub content_generation: u64,
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub sealed: bool,
    pub head_partial: bool,
    storage: Arc<StoredSource>,
}

impl HostContentSourceSnapshot {
    #[must_use]
    pub fn text(&self) -> String {
        self.storage.text()
    }

    #[must_use]
    pub fn annotations(&self) -> Vec<ContentAnnotationSnapshot> {
        self.storage
            .annotations_in_order()
            .iter()
            .map(|annotation| ContentAnnotationSnapshot {
                kind: annotation.kind,
                flags: annotation.flags,
                start_byte: annotation.start_byte,
                end_byte: annotation.end_byte,
                payload: annotation.payload.to_vec(),
                aux0: annotation.aux0,
                aux1: annotation.aux1,
            })
            .collect()
    }

    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        self.source_end.saturating_sub(self.source_base)
    }

    #[must_use]
    pub fn retained_lines(&self) -> u64 {
        self.storage.line_count() as u64
    }

    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.storage.chunk_count()
    }

    pub(crate) fn chunk_views(&self) -> Vec<ChunkView> {
        self.storage.chunk_views()
    }

    fn chunks(&self) -> impl Iterator<Item = (&[u8], u64)> {
        self.storage.iter_chunks()
    }

    fn annotations_for_projection(&self) -> &[SourceAnnotation] {
        self.storage.annotations_in_order()
    }

    fn stable_prefix(&self) -> Option<Self> {
        let storage = self.storage.stable_prefix()?;
        Some(Self {
            source_id: self.source_id,
            source_generation: self.source_generation,
            content_generation: self.content_generation,
            revision: self.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            storage: Arc::new(storage),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContentLineage {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
}

impl ContentLineage {
    fn from_snapshot(snapshot: &HostContentSourceSnapshot) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TextProjectionKey {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_revision: u64,
    width: u16,
    wrap: TextWrapMode,
    funnel_kind: TextFunnelKind,
    delivery_revision: u64,
    theme_revision: u64,
    needs_finalized_prefix: bool,
    needs_physical_rows: bool,
}

impl TextProjectionKey {
    fn revision(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn metric_revision(self, size: Size, complete: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.width.hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.funnel_kind.hash(&mut hasher);
        self.needs_finalized_prefix.hash(&mut hasher);
        size.width.hash(&mut hasher);
        size.height.hash(&mut hasher);
        complete.hash(&mut hasher);
        hasher.finish()
    }

    fn layout_input_revision(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.source_id.hash(&mut hasher);
        self.source_generation.hash(&mut hasher);
        self.source_revision.hash(&mut hasher);
        self.width.hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.funnel_kind.hash(&mut hasher);
        // Delivery can change the visible intrinsic height even when Source
        // bytes and width are unchanged. It is a layout input, while the
        // metric revision below still records whether geometry actually
        // changed after evaluation.
        self.delivery_revision.hash(&mut hasher);
        hasher.finish()
    }
}

/// Key identifying a cached semantic IR projection. The semantic IR is
/// independent of theme, width, delivery tick, and viewport.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SemanticProjectionKey {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_revision: u64,
    source_base: u64,
    source_end: u64,
    sealed: bool,
    funnel_kind: TextFunnelKind,
    hyperlinks: bool,
}

impl SemanticProjectionKey {
    fn for_snapshot(snapshot: &HostContentSourceSnapshot, funnel: HostContentFunnel) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            sealed: snapshot.sealed,
            funnel_kind: funnel.kind,
            hyperlinks: funnel.hyperlinks,
        }
    }
}

type SemanticProjectionCache = VecDeque<(SemanticProjectionKey, Arc<Projection<TextContent>>)>;

/// Resolves one semantic projection through the Connector cache, building
/// and retaining it only on a miss. Theme-only changes always hit: parsers
/// never re-run for a recolor.
fn resolve_cached_semantic(
    cache: &mut SemanticProjectionCache,
    key: SemanticProjectionKey,
    build: impl FnOnce() -> Result<Projection<TextContent>>,
) -> Result<Arc<Projection<TextContent>>> {
    if let Some(hit) = cache
        .iter()
        .find(|(candidate, _)| candidate == &key)
        .map(|(_, projection)| Arc::clone(projection))
    {
        return Ok(hit);
    }
    crate::perf::inc(crate::perf::Counter::SemanticProjectionRebuilds);
    let built = Arc::new(build()?);
    cache.retain(|(candidate, _)| candidate != &key);
    cache.push_front((key, Arc::clone(&built)));
    while cache.len() > 4 {
        cache.pop_back();
    }
    Ok(built)
}

#[derive(Clone, Debug)]
struct HostContentProjection {
    /// Monotonic product identity used by prepared tickets.  This is not an
    /// allocator address and therefore cannot suffer pointer ABA after cache
    /// eviction/reuse.
    identity: u64,
    key: TextProjectionKey,
    intrinsic_size: Size,
    physically_complete: bool,
    /// Immediate non-History projections may defer physical row lowering to
    /// the prepared-ticket window. Smooth/History products retain rows for
    /// reveal and scrollback semantics.
    rows: Option<Arc<Vec<PhysicalRow>>>,
    layout: Option<Arc<crate::presentation::layout::LayoutTree>>,
    text_geometry: Option<Arc<Mutex<crate::presentation::paint::TextGeometryCache>>>,
    /// Immutable palette captured with this projection. Deferred row-window
    /// painting must not consult the Connector's newer host theme.
    theme: Arc<Theme>,
    /// Physical rows produced by the established finalized-prefix proof.
    /// This is a distinct product from the open document rows: Markdown may
    /// render a prefix differently while its trailing block remains open.
    finalized_prefix: Option<Arc<FinalizedPrefixProduct>>,
    stable_rows: usize,
    visible_row_count: usize,
    cut: Option<(u16, u16)>,
}

/// Mutable delivery state that belongs to one smoothed Connector binding.
/// Delivery tracks time advancement, grapheme indexing, and candidate frontiers
/// without reconstructing raw Source projections or parsing syntax on pure ticks.
#[derive(Debug)]
struct ConnectorDelivery {
    smoother: Smooth,
    units: Projection<TextContent>,
    indexed_generation: u32,
    indexed_revision: u64,
    indexed_sealed: bool,
    candidate_frontier: StreamOffset,
}

impl ConnectorDelivery {
    fn new(config: SmoothConfig) -> Self {
        Self {
            smoother: Smooth::new(config),
            units: ProjectionBuilder::new(
                StreamOffset::ZERO,
                StreamOffset::ZERO,
                StreamOffset::ZERO,
                false,
            )
            .finish()
            .expect("empty grapheme projection is valid"),
            indexed_generation: u32::MAX,
            indexed_revision: u64::MAX,
            indexed_sealed: false,
            candidate_frontier: StreamOffset::ZERO,
        }
    }

    fn accept_input(&mut self, snapshot: &HostContentSourceSnapshot) -> Result<()> {
        let changed = self.indexed_generation != snapshot.source_generation
            || self.indexed_revision != snapshot.revision
            || self.indexed_sealed != snapshot.sealed;
        if !changed {
            return Ok(());
        }
        let units = source_grapheme_projection(snapshot)
            .map_err(|error| anyhow!("content smoothing input failed: {error}"))?;
        let _ = self.smoother.project(&units);
        self.units = units;
        self.indexed_generation = snapshot.source_generation;
        self.indexed_revision = snapshot.revision;
        self.indexed_sealed = snapshot.sealed;
        self.candidate_frontier = self.smoother.published_through();
        Ok(())
    }

    fn advance(&mut self, now: Instant) -> bool {
        let progressed = self.smoother.advance(now);
        if progressed {
            self.candidate_frontier = self.smoother.published_through();
        }
        progressed
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.smoother.next_wakeup()
    }

    fn published_through(&self) -> StreamOffset {
        self.smoother.published_through()
    }

    fn reveal_units(&self) -> usize {
        let published = self.published_through();
        let spans = self.units.spans();
        spans.partition_point(|span| span.source().end() <= published)
    }
}

/// Mutable execution state that belongs to one Connector binding. Funnels
/// remain immutable specifications; inactive Connectors drop this value so
/// inactive membership retains no parser, delivery, or projection work.
#[derive(Debug)]
struct ConnectorExecution {
    markdown: Option<MarkdownProjector>,
    diff: Option<DiffProjector>,
    ansi: Option<AnsiProjector>,
    parser_lineage: Option<ContentLineage>,
    delivery: Option<ConnectorDelivery>,
    /// Text lowering is connector-local just like parser and delivery state.
    /// Keeping this renderer alive makes its immutable block/edge products
    /// reusable across source appends, theme changes, and delivery ticks.
    renderer: TextRenderer,
}

impl ConnectorExecution {
    fn new(funnel: &HostContentFunnel) -> Self {
        Self {
            markdown: matches!(funnel.kind, TextFunnelKind::Markdown).then(|| {
                MarkdownProjector::new(MarkdownOptions::gfm().with_live_table_stabilization(true))
            }),
            diff: matches!(funnel.kind, TextFunnelKind::Diff).then(DiffProjector::new),
            ansi: matches!(funnel.kind, TextFunnelKind::Ansi).then(|| {
                AnsiProjector::new(crate::text::AnsiOptions {
                    hyperlinks: funnel.hyperlinks,
                })
            }),
            parser_lineage: None,
            delivery: funnel.smooth_config().map(ConnectorDelivery::new),
            renderer: content_text_renderer(),
        }
    }

    fn prepare_for_snapshot(&mut self, snapshot: &HostContentSourceSnapshot) {
        let lineage = ContentLineage::from_snapshot(snapshot);
        if self.parser_lineage == Some(lineage) {
            return;
        }
        if self.parser_lineage.is_some() {
            // Replacement/clear starts a new logical document even when the
            // retained byte range happens to have the same coordinates. Only
            // parser state is lineage-bound; Smooth keeps its existing
            // replacement policy and the renderer remains reusable.
            self.markdown = None;
            self.diff = None;
            self.ansi = None;
        }
        self.parser_lineage = Some(lineage);
    }
}

impl HostContentProjection {
    fn measurement(&self, connector_id: u64) -> ContentMeasurement {
        ContentMeasurement {
            intrinsic_size: self.intrinsic_size,
            physically_complete: self.physically_complete,
            projection_revision: self.key.revision(),
            metric_revision: self
                .key
                .metric_revision(self.intrinsic_size, self.physically_complete),
            paint_revision: self.key.revision(),
            connector_id: Some(connector_id),
            projection_identity: self.identity,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostContentSourceStats {
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub retained_bytes: u64,
    pub retained_lines: u64,
    pub chunk_count: usize,
    pub sealed: bool,
    pub head_partial: bool,
    pub accepted_bytes: u64,
    pub copied_bytes: u64,
    pub dropped_head_bytes: u64,
}

/// Result returned by every successful Source data mutation. The wake bit is
/// only a scheduler hint; native host epochs remain authoritative. A
/// post-acceptance host wake failure is reported by the environment's next
/// drain report without changing this successful result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentMutationResult {
    pub revision: u64,
    pub environment_wake_epoch: u64,
    pub schedule_environment_drain: bool,
}

const SOURCE_CHUNK_BYTES: usize = 16 * 1024;
const MAX_SOURCE_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_CONTENT_PROJECTION_ROWS: u64 = u16::MAX as u64;
const MAX_SOURCE_ANNOTATIONS: usize = 16 * 1024;
const MAX_ANNOTATION_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentProjectionFailureKind {
    LimitExceeded,
    RetentionIncompatible,
    Projection,
}

impl ContentProjectionFailureKind {
    const fn code(self) -> &'static str {
        match self {
            Self::LimitExceeded => "LIMIT_EXCEEDED",
            Self::RetentionIncompatible => "RETENTION_INCOMPATIBLE",
            Self::Projection => "PROJECTION_FAILED",
        }
    }
}

#[derive(Debug)]
struct ContentProjectionFailure {
    kind: ContentProjectionFailureKind,
    diagnostic: String,
}

impl std::fmt::Display for ContentProjectionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ContentProjectionFailure {}

static NEXT_CONTENT_PROJECTION_ID: AtomicU64 = AtomicU64::new(1);

fn next_content_projection_id() -> u64 {
    NEXT_CONTENT_PROJECTION_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("content projection identity exhausted")
}

/// Initial annotation kinds are deliberately closed and host-independent.
/// More consumer-specific kinds can be added by a generated sidecar later;
/// unknown kinds never silently enter the Source store.
pub const CONTENT_ANNOTATION_KIND_TAG: u32 = 1;
pub const CONTENT_ANNOTATION_KIND_STYLE: u32 = 2;
pub const CONTENT_ANNOTATION_KIND_ATOMIC: u32 = 3;
pub const CONTENT_ANNOTATION_KIND_POINT: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnnotationTruncationPolicy {
    Clip,
    Drop,
    Point,
}

fn projected_bounds(
    snapshot: &HostContentSourceSnapshot,
    wrap: TextWrapMode,
    offered_width: u16,
) -> (u64, u64) {
    let width = u64::from(offered_width.max(1));
    let mut rows = 0u64;
    let mut line_bytes = 0u64;
    let mut max_line_bytes = 0u64;
    for (bytes, _) in snapshot.chunks() {
        for byte in bytes {
            if *byte == b'\n' {
                max_line_bytes = max_line_bytes.max(line_bytes);
                rows = if wrap == TextWrapMode::NoWrap {
                    rows.saturating_add(1)
                } else {
                    rows.saturating_add(line_bytes.div_ceil(width).max(1))
                };
                line_bytes = 0;
            } else {
                line_bytes = line_bytes.saturating_add(1);
            }
        }
    }
    max_line_bytes = max_line_bytes.max(line_bytes);
    rows = if wrap == TextWrapMode::NoWrap {
        rows.saturating_add(1)
    } else {
        rows.saturating_add(line_bytes.div_ceil(width).max(1))
    };
    (rows, max_line_bytes)
}

fn annotation_policy(kind: u32) -> AnnotationTruncationPolicy {
    match kind {
        CONTENT_ANNOTATION_KIND_TAG | CONTENT_ANNOTATION_KIND_STYLE => {
            AnnotationTruncationPolicy::Clip
        }
        CONTENT_ANNOTATION_KIND_ATOMIC => AnnotationTruncationPolicy::Drop,
        CONTENT_ANNOTATION_KIND_POINT => AnnotationTruncationPolicy::Point,
        _ => AnnotationTruncationPolicy::Drop,
    }
}

fn source_projection(
    snapshot: &HostContentSourceSnapshot,
) -> Result<Projection<TextContent>, TextProjectionError> {
    let mut builder = ProjectionBuilder::new(
        StreamOffset::new(snapshot.source_base),
        StreamOffset::new(snapshot.source_end),
        StreamOffset::new(snapshot.source_end),
        snapshot.sealed,
    );
    for view in snapshot.chunk_views() {
        let raw = RawText::from_page_slice(view.page, view.page_start, view.len);
        let end = view.abs_start.saturating_add(u64::from(view.len));
        builder = builder.emit(
            StreamRange::new(StreamOffset::new(view.abs_start), StreamOffset::new(end)),
            TextContent::Raw(raw),
        );
    }
    builder.finish().map_err(TextProjectionError::Projection)
}

fn source_grapheme_projection(
    snapshot: &HostContentSourceSnapshot,
) -> Result<Projection<TextContent>, TextProjectionError> {
    let mut builder = ProjectionBuilder::new(
        StreamOffset::new(snapshot.source_base),
        StreamOffset::new(snapshot.source_end),
        StreamOffset::new(snapshot.source_end),
        snapshot.sealed,
    );
    let mut carry: Option<(u64, String)> = None;
    for (bytes, start) in snapshot.chunks() {
        let text = str::from_utf8(bytes).expect("Source snapshot chunks are valid UTF-8");
        let (combined_start, mut combined) = match carry.take() {
            Some((carry_start, carry_text)) => {
                let mut combined = carry_text;
                combined.push_str(text);
                (carry_start, combined)
            }
            None => (start, text.to_owned()),
        };
        let boundaries = combined
            .grapheme_indices(true)
            .map(|(offset, grapheme)| (offset, grapheme.len()))
            .collect::<Vec<_>>();
        let keep = boundaries.last().copied();
        for (offset, length) in boundaries
            .iter()
            .copied()
            .take(boundaries.len().saturating_sub(1))
        {
            let grapheme_start = combined_start.saturating_add(offset as u64);
            let grapheme_end = grapheme_start.saturating_add(length as u64);
            builder = builder.emit(
                StreamRange::new(
                    StreamOffset::new(grapheme_start),
                    StreamOffset::new(grapheme_end),
                ),
                TextContent::raw(combined[offset..offset + length].to_owned()),
            );
        }
        if let Some((offset, length)) = keep {
            let carry_start = combined_start.saturating_add(offset as u64);
            carry = Some((carry_start, combined[offset..offset + length].to_owned()));
        }
        // Drop the temporary combined buffer after preserving only the final
        // grapheme. This keeps cross-append EGC handling correct without
        // materializing the complete Source.
        combined.clear();
    }
    if let Some((start, grapheme)) = carry {
        let end = start.saturating_add(grapheme.len() as u64);
        builder = builder.emit(
            StreamRange::new(StreamOffset::new(start), StreamOffset::new(end)),
            TextContent::raw(grapheme),
        );
    }
    builder.finish().map_err(TextProjectionError::Projection)
}

fn project_semantic_snapshot(
    snapshot: &HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    execution: &mut ConnectorExecution,
) -> Result<Projection<TextContent>> {
    execution.prepare_for_snapshot(snapshot);
    let raw = source_projection(snapshot).map_err(|error| anyhow!(error.to_string()))?;
    let semantic = match funnel.kind {
        TextFunnelKind::Plain => PlainTextProjector::new()
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Markdown => execution
            .markdown
            .get_or_insert_with(|| {
                MarkdownProjector::new(MarkdownOptions::gfm().with_live_table_stabilization(true))
            })
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Diff => execution
            .diff
            .get_or_insert_with(DiffProjector::new)
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Ansi => execution
            .ansi
            .get_or_insert_with(|| {
                AnsiProjector::new(crate::text::AnsiOptions {
                    hyperlinks: funnel.hyperlinks,
                })
            })
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
    };
    if snapshot.annotations_for_projection().is_empty() {
        return Ok(semantic);
    }
    SourceAnnotationRewriter::new(&snapshot.storage)
        .into_projector()
        .project(&semantic)
        .map_err(|error| anyhow!(error.to_string()))
}

fn content_text_renderer() -> TextRenderer {
    let policy = crate::TextRenderPolicy::new()
        .with_block_gap(1)
        .with_soft_break(crate::SoftBreakPolicy::LineBreak)
        .with_table_column_sizing(crate::TableColumnSizing::Content)
        .with_table_column_gap(1)
        .with_table_row_gap(0)
        .with_task_list_marker(crate::TaskListMarkerPolicy::TaskOnly)
        .with_code_block_label(crate::CodeBlockLabelPolicy::Language)
        .with_code_block_gap(0)
        .with_code_wrap(crate::WrapMode::NoWrap);
    TextRenderer::with_policy(policy)
}

fn compile_semantic_content(
    renderer: &TextRenderer,
    semantic: &Projection<TextContent>,
    theme: &Theme,
    offered_width: u16,
    text_geometry: &mut crate::presentation::paint::TextGeometryCache,
) -> Result<(
    crate::presentation::layout::LayoutBlock,
    Arc<crate::presentation::layout::LayoutTree>,
)> {
    if semantic.spans().is_empty() {
        let view = renderer.lower_semantic_iter(std::iter::empty());
        let compiler = crate::presentation::layout::ViewCompiler::new(theme);
        let tree = Arc::new(compiler.layout_tree(
            &view,
            crate::geometry::LayoutConstraints::width_only(offered_width.max(1)),
        ));
        return Ok((
            crate::presentation::layout::LayoutBlock {
                width: 0,
                rows: Vec::new(),
                physically_complete: true,
            },
            tree,
        ));
    }
    let view = renderer.lower_semantic_iter(semantic.spans().iter().flat_map(|span| span.values()));
    let compiler = crate::presentation::layout::ViewCompiler::new(theme);
    let tree = Arc::new(compiler.layout_tree(
        &view,
        crate::geometry::LayoutConstraints::width_only(offered_width.max(1)),
    ));
    Ok((
        compiler.compile_tree_with_text_cache(&tree, text_geometry),
        tree,
    ))
}

fn layout_semantic_content(
    renderer: &TextRenderer,
    semantic: &Projection<TextContent>,
    theme: &Theme,
    offered_width: u16,
) -> (u16, bool, Arc<crate::presentation::layout::LayoutTree>) {
    let view = renderer.lower_semantic_iter(semantic.spans().iter().flat_map(|span| span.values()));
    let compiler = crate::presentation::layout::ViewCompiler::new(theme);
    let tree = Arc::new(compiler.layout_tree(
        &view,
        crate::geometry::LayoutConstraints::width_only(offered_width.max(1)),
    ));
    (tree.size.width, tree.physically_complete, tree)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RevealBoundary {
    pub(crate) revealed_height: u16,
    pub(crate) fully_revealed_rows: usize,
    pub(crate) cut: Option<(u16, u16)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityIndex {
    pub(crate) row_glyphs: Vec<Vec<u16>>,
    pub(crate) total_glyphs: usize,
}

impl VisibilityIndex {
    pub(crate) fn from_rows(rows: &[PhysicalRow]) -> Self {
        let mut row_glyphs = Vec::with_capacity(rows.len());
        let mut total_glyphs = 0;
        for row in rows {
            let mut cols = Vec::new();
            for glyph in row.glyphs() {
                if glyph.leader.painted {
                    cols.push(glyph.start as u16);
                    total_glyphs += 1;
                }
            }
            row_glyphs.push(cols);
        }
        Self {
            row_glyphs,
            total_glyphs,
        }
    }

    pub(crate) fn reveal_bounds(&self, units: usize, width: u16, height: u16) -> RevealBoundary {
        if units == 0 || width == 0 || height == 0 || self.total_glyphs == 0 {
            return RevealBoundary {
                revealed_height: 0,
                fully_revealed_rows: 0,
                cut: None,
            };
        }
        if units >= self.total_glyphs {
            return RevealBoundary {
                revealed_height: height,
                fully_revealed_rows: self.row_glyphs.len(),
                cut: None,
            };
        }
        let mut remaining = units;
        let mut last_row = 0u16;
        let mut saw_glyph = false;
        let mut fully_revealed = 0usize;
        let mut cut: Option<(u16, u16)> = None;

        for (row_idx, cols) in self.row_glyphs.iter().enumerate() {
            let row = row_idx as u16;
            if cols.is_empty() {
                fully_revealed = row_idx + 1;
                continue;
            }
            if remaining < cols.len() {
                let cut_col = cols[remaining];
                cut = Some((row, cut_col));
                if remaining > 0 {
                    saw_glyph = true;
                    last_row = row;
                }
                break;
            }
            remaining -= cols.len();
            saw_glyph = true;
            last_row = row;
            fully_revealed = row_idx + 1;
        }

        if !saw_glyph {
            return RevealBoundary {
                revealed_height: 0,
                fully_revealed_rows: 0,
                cut: None,
            };
        }

        let target_height = if let Some((cut_row, cut_col)) = cut {
            if cut_col > 0 {
                last_row.max(cut_row).saturating_add(1)
            } else {
                last_row.saturating_add(1)
            }
        } else {
            last_row.saturating_add(1)
        };

        RevealBoundary {
            revealed_height: target_height.min(height),
            fully_revealed_rows: fully_revealed,
            cut,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PreparedPaintKey {
    semantic_key: SemanticProjectionKey,
    theme_revision: u64,
    width: u16,
    needs_finalized_prefix: bool,
    needs_physical_rows: bool,
}

#[derive(Clone, Debug)]
struct PreparedPaintProduct {
    /// Width-dependent layout geometry is palette-independent and survives a
    /// theme-only repaint.  The physical rows below are the theme-resolved
    /// paint product layered on top of this retained tree.
    layout: Arc<crate::presentation::layout::LayoutTree>,
    text_geometry: Arc<Mutex<crate::presentation::paint::TextGeometryCache>>,
    rows: Option<Arc<Vec<PhysicalRow>>>,
    width: u16,
    height: u16,
    physically_complete: bool,
    visibility: Option<VisibilityIndex>,
    /// The current finalized-prefix policy is retained as an immutable row
    /// product. It is intentionally allowed to differ from the open document
    /// until the existing parser/History policy says the prefix is finalized.
    finalized_prefix: Option<Arc<FinalizedPrefixProduct>>,
}

#[derive(Clone, Debug)]
struct FinalizedPrefixProduct {
    rows: Arc<Vec<PhysicalRow>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PrefixProofKey {
    semantic: SemanticProjectionKey,
    source_end: u64,
    width: u16,
}

#[derive(Clone, Debug)]
struct PrefixProof {
    source_end: u64,
    layout: Arc<crate::presentation::layout::LayoutTree>,
    text_geometry: Arc<Mutex<crate::presentation::paint::TextGeometryCache>>,
}

type PrefixProofCache = VecDeque<(PrefixProofKey, Arc<PrefixProof>)>;

type PreparedPaintCache = VecDeque<(PreparedPaintKey, Arc<PreparedPaintProduct>)>;

fn prove_finalized_prefix(
    snapshot: &HostContentSourceSnapshot,
    semantic_key: &SemanticProjectionKey,
    funnel: HostContentFunnel,
    theme: &Theme,
    offered_width: u16,
    semantic_cache: &mut SemanticProjectionCache,
    prefix_proof_cache: &mut PrefixProofCache,
) -> Option<Arc<FinalizedPrefixProduct>> {
    // This is the pre-L11 finalized-prefix policy. It deliberately renders
    // an immutable sealed range rather than slicing the open projection; the
    // existing parser/restart behavior remains authoritative.
    let prefix = snapshot.stable_prefix()?;
    let stable_end = prefix.source_end;
    let key = PrefixProofKey {
        semantic: semantic_key.clone(),
        source_end: stable_end,
        width: offered_width.max(1),
    };
    let (proof, initial_rows) = if let Some(proof) = prefix_proof_cache
        .iter()
        .find(|(candidate, _)| candidate == &key)
        .map(|(_, proof)| Arc::clone(proof))
    {
        (proof, None)
    } else {
        let mut prefix_execution = ConnectorExecution::new(&funnel);
        let prefix_key = SemanticProjectionKey::for_snapshot(&prefix, funnel);
        let prefix_semantic = resolve_cached_semantic(semantic_cache, prefix_key, || {
            project_semantic_snapshot(&prefix, funnel, &mut prefix_execution)
        })
        .ok()?;
        let text_geometry = Arc::new(Mutex::new(
            crate::presentation::paint::TextGeometryCache::new(),
        ));
        let (compiled, layout) = {
            let mut text_geometry_guard = text_geometry.lock().ok()?;
            compile_semantic_content(
                &prefix_execution.renderer,
                &prefix_semantic,
                theme,
                offered_width,
                &mut text_geometry_guard,
            )
            .ok()?
        };
        let proof = Arc::new(PrefixProof {
            source_end: stable_end,
            layout,
            text_geometry,
        });
        if prefix_proof_cache.len() >= 8 {
            prefix_proof_cache.pop_back();
        }
        prefix_proof_cache.push_front((key, Arc::clone(&proof)));
        (proof, Some(compiled.rows))
    };
    debug_assert_eq!(proof.source_end, stable_end);
    let rows = if let Some(rows) = initial_rows {
        rows
    } else {
        let mut text_geometry = proof.text_geometry.lock().ok()?;
        crate::presentation::layout::ViewCompiler::new(theme)
            .compile_tree_with_text_cache(&proof.layout, &mut text_geometry)
            .rows
    };
    Some(Arc::new(FinalizedPrefixProduct {
        rows: Arc::new(rows),
    }))
}

fn project_text_snapshot(
    snapshot: &HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    offered_width: u16,
    needs_finalized_prefix: bool,
    theme: &Arc<Theme>,
    theme_revision: u64,
    execution: &mut ConnectorExecution,
    delivery_revision: u64,
    semantic_cache: &mut SemanticProjectionCache,
    prefix_proof_cache: &mut PrefixProofCache,
    prepared_paint_cache: &mut PreparedPaintCache,
) -> Result<HostContentProjection> {
    let key = TextProjectionKey {
        source_id: snapshot.source_id,
        source_generation: snapshot.source_generation,
        content_generation: snapshot.content_generation,
        source_revision: snapshot.revision,
        width: offered_width.max(1),
        wrap: funnel.wrap,
        funnel_kind: funnel.kind,
        delivery_revision,
        theme_revision,
        needs_finalized_prefix,
        needs_physical_rows: needs_finalized_prefix || execution.delivery.is_some(),
    };
    if snapshot.source_base == snapshot.source_end {
        return Ok(HostContentProjection {
            identity: next_content_projection_id(),
            key,
            intrinsic_size: Size::new(0, 0),
            physically_complete: true,
            rows: Some(Arc::new(Vec::new())),
            layout: None,
            text_geometry: None,
            theme: Arc::clone(theme),
            finalized_prefix: None,
            stable_rows: 0,
            visible_row_count: 0,
            cut: None,
        });
    }

    let (row_bound, max_line_bytes) = projected_bounds(snapshot, funnel.wrap, offered_width);
    if row_bound > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow::Error::new(ContentProjectionFailure {
            kind: ContentProjectionFailureKind::LimitExceeded,
            diagnostic: format!(
                "LIMIT_EXCEEDED: content projection requires {row_bound} rows, exceeding the terminal row limit"
            ),
        }));
    }
    if max_line_bytes > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow::Error::new(ContentProjectionFailure {
            kind: ContentProjectionFailureKind::LimitExceeded,
            diagnostic: format!(
                "LIMIT_EXCEEDED: content projection has a {max_line_bytes}-byte logical line, exceeding the terminal line limit"
            ),
        }));
    }

    // Semantic IR is theme-independent and layout-independent: recolors,
    // window resizes, and smooth timer delivery ticks hit the cache, while
    // source revisions or funnel kind changes rebuild.
    let semantic_key = SemanticProjectionKey::for_snapshot(snapshot, funnel);
    let paint_key = PreparedPaintKey {
        semantic_key: semantic_key.clone(),
        theme_revision,
        width: offered_width.max(1),
        needs_finalized_prefix,
        needs_physical_rows: key.needs_physical_rows,
    };
    let retain_rows = key.needs_physical_rows;

    let paint_product = if let Some(product) = prepared_paint_cache
        .iter()
        .find(|(k, _)| k == &paint_key)
        .map(|(_, p)| Arc::clone(p))
    {
        product
    } else {
        let semantic = resolve_cached_semantic(semantic_cache, semantic_key.clone(), || {
            project_semantic_snapshot(snapshot, funnel, execution)
        })?;
        let reusable_product = prepared_paint_cache
            .iter()
            .find(|(candidate, _)| {
                candidate.semantic_key == semantic_key
                    && candidate.width == offered_width.max(1)
                    && candidate.needs_finalized_prefix == needs_finalized_prefix
                    && candidate.needs_physical_rows == key.needs_physical_rows
            })
            .map(|(_, product)| Arc::clone(product));
        let (compiled, layout, text_geometry) = if let Some(product) = reusable_product {
            let layout = Arc::clone(&product.layout);
            let text_geometry = Arc::clone(&product.text_geometry);
            let compiled = if retain_rows {
                let compiler = crate::presentation::layout::ViewCompiler::new(theme);
                let mut text_geometry_guard = text_geometry
                    .lock()
                    .map_err(|_| anyhow!("text geometry cache lock is poisoned"))?;
                compiler.compile_tree_with_text_cache(&layout, &mut text_geometry_guard)
            } else {
                crate::presentation::layout::LayoutBlock {
                    width: layout.size.width,
                    rows: Vec::new(),
                    physically_complete: layout.physically_complete,
                }
            };
            (compiled, layout, text_geometry)
        } else {
            let text_geometry = Arc::new(Mutex::new(
                crate::presentation::paint::TextGeometryCache::new(),
            ));
            let (compiled, layout) = if retain_rows {
                let (compiled, layout) = {
                    let mut text_geometry_guard = text_geometry
                        .lock()
                        .map_err(|_| anyhow!("text geometry cache lock is poisoned"))?;
                    compile_semantic_content(
                        &execution.renderer,
                        &semantic,
                        theme,
                        offered_width,
                        &mut text_geometry_guard,
                    )?
                };
                (compiled, layout)
            } else {
                let (width, physically_complete, layout) =
                    layout_semantic_content(&execution.renderer, &semantic, theme, offered_width);
                (
                    crate::presentation::layout::LayoutBlock {
                        width,
                        rows: Vec::new(),
                        physically_complete,
                    },
                    layout,
                )
            };
            (compiled, layout, text_geometry)
        };
        let width = compiled.width;
        let physically_complete = compiled.physically_complete;
        let rows = retain_rows.then(|| Arc::new(compiled.rows));
        let height = layout.size.height;
        let visibility = rows.as_ref().map(|rows| VisibilityIndex::from_rows(rows));
        let finalized_prefix = if !needs_finalized_prefix {
            None
        } else if snapshot.sealed {
            Some(Arc::new(FinalizedPrefixProduct {
                rows: Arc::clone(
                    rows.as_ref()
                        .expect("History products retain physical rows"),
                ),
            }))
        } else {
            prove_finalized_prefix(
                snapshot,
                &semantic_key,
                funnel,
                theme,
                offered_width,
                semantic_cache,
                prefix_proof_cache,
            )
        };
        let product = Arc::new(PreparedPaintProduct {
            layout,
            text_geometry,
            rows,
            width,
            height,
            physically_complete,
            visibility,
            finalized_prefix,
        });
        prepared_paint_cache.retain(|(k, _)| k != &paint_key);
        prepared_paint_cache.push_front((paint_key, Arc::clone(&product)));
        while prepared_paint_cache.len() > 4 {
            prepared_paint_cache.pop_back();
        }
        product
    };

    let (intrinsic_size, visible_row_count, cut, _fully_revealed_rows) =
        if let Some(delivery) = execution.delivery.as_mut() {
            delivery.accept_input(snapshot)?;
            let reveal_units = delivery.reveal_units();
            let bounds = paint_product
                .visibility
                .as_ref()
                .expect("delivery products retain visibility rows")
                .reveal_bounds(reveal_units, paint_product.width, paint_product.height);
            (
                Size::new(paint_product.width, bounds.revealed_height),
                usize::from(bounds.revealed_height),
                bounds.cut,
                bounds.fully_revealed_rows,
            )
        } else {
            (
                Size::new(paint_product.width, paint_product.height),
                usize::from(paint_product.height),
                None,
                usize::from(paint_product.height),
            )
        };

    let stable_rows = paint_product.finalized_prefix.as_ref().map_or(0, |prefix| {
        if execution.delivery.is_some() {
            // Preserve the established row-granular Smooth policy: History
            // may transfer the finalized product incrementally as the same
            // number of open rows become fully revealed.  In particular, a
            // sealed source can still have a partial sink-visible backlog.
            prefix.rows.len().min(_fully_revealed_rows)
        } else {
            prefix.rows.len()
        }
    });

    Ok(HostContentProjection {
        identity: next_content_projection_id(),
        key,
        intrinsic_size,
        physically_complete: paint_product.physically_complete,
        rows: paint_product.rows.clone(),
        layout: Some(Arc::clone(&paint_product.layout)),
        text_geometry: Some(Arc::clone(&paint_product.text_geometry)),
        theme: Arc::clone(theme),
        finalized_prefix: paint_product.finalized_prefix.clone(),
        stable_rows,
        visible_row_count,
        cut,
    })
}

/// Applies Source annotations to semantic runs without resolving them to a
/// host-native style. Exact runs preserve source coordinates; transformed
/// runs use a deterministic proportional split when a range crosses them.
struct SourceAnnotationRewriter<'a> {
    storage: &'a StoredSource,
}

impl<'a> SourceAnnotationRewriter<'a> {
    fn new(storage: &'a StoredSource) -> Self {
        Self { storage }
    }

    fn annotate_run(&self, run: TextRun) -> Result<Vec<TextRun>, TextProjectionError> {
        let Some(source) = (match run.provenance() {
            TextProvenance::Exact(range) | TextProvenance::Derived(range) => Some(*range),
            TextProvenance::Synthetic => None,
        }) else {
            return Ok(vec![run]);
        };
        if source.is_empty() || run.text().is_empty() {
            return Ok(vec![run]);
        }
        let overlapping = self
            .storage
            .overlapping(source.start().as_u64(), source.end().as_u64());
        if overlapping.is_empty() {
            return Ok(vec![run]);
        }
        let mut cuts = vec![0usize, run.text().len()];
        for overlap in &overlapping {
            for offset in [overlap.start(), overlap.end()] {
                if offset <= source.start().as_u64() || offset >= source.end().as_u64() {
                    continue;
                }
                let local = source_local_offset(&run, source, offset);
                if run.text().is_char_boundary(local) {
                    cuts.push(local);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        if cuts.len() == 2 {
            let mut piece = run;
            piece = piece.map_annotations(|current| {
                overlapping.iter().fold(current, |current, overlap| {
                    if let Some(tag) = overlap.tag() {
                        current.with_tag(tag)
                    } else {
                        current
                    }
                })
            });
            if let Some(style) = overlapping.iter().rev().find_map(|overlap| overlap.style()) {
                piece = piece.with_style(style);
            }
            return Ok(vec![piece]);
        }
        let mut output = Vec::with_capacity(cuts.len().saturating_sub(1));
        for pair in cuts.windows(2) {
            let local_start = pair[0];
            let local_end = pair[1];
            if local_start == local_end {
                continue;
            }
            let (piece, _) = run.split_at(local_end)?;
            let (_, piece) = piece.split_at(local_start)?;
            let piece_source = source_range_for_piece(&run, source, local_start, local_end);
            let active = overlapping
                .iter()
                .filter(|overlap| {
                    overlap.end() > piece_source.start().as_u64()
                        && overlap.start() < piece_source.end().as_u64()
                })
                .collect::<Vec<_>>();
            let mut piece = piece;
            if !active.is_empty() {
                piece = piece.map_annotations(|current| {
                    active.iter().fold(current, |current, overlap| {
                        if let Some(tag) = overlap.tag() {
                            current.with_tag(tag)
                        } else {
                            current
                        }
                    })
                });
                if let Some(style) = active.iter().rev().find_map(|overlap| overlap.style()) {
                    piece = piece.with_style(style);
                }
            }
            output.push(piece);
        }
        Ok(output)
    }
}

impl TextRewriter for SourceAnnotationRewriter<'_> {
    type Error = TextProjectionError;

    fn rewrite_block(&mut self, block: Block) -> Result<Block, Self::Error> {
        walk_rewrite_block(self, block)
    }

    fn rewrite_inline(&mut self, inline: Inline) -> Result<Inline, Self::Error> {
        let InlineKind::Text(run) = inline.kind() else {
            return walk_rewrite_inline(self, inline);
        };
        let pieces = self.annotate_run(run.clone())?;
        if pieces.len() != 1 {
            // Inline content owns the vector boundary. This branch is only
            // used by rewrite_inline callers outside our inline-content hook;
            // preserve the first piece rather than duplicating an Inline.
            let Some(first) = pieces.into_iter().next() else {
                return Ok(inline);
            };
            return Ok(Inline::from_parts(
                InlineKind::Text(first),
                inline.marks().clone(),
                inline.annotations().clone(),
            ));
        }
        Ok(Inline::from_parts(
            InlineKind::Text(pieces.into_iter().next().expect("one piece")),
            inline.marks().clone(),
            inline.annotations().clone(),
        ))
    }

    fn rewrite_inline_content(
        &mut self,
        content: InlineContent,
    ) -> Result<InlineContent, Self::Error> {
        let mut output = Vec::new();
        for inline in content.items() {
            let InlineKind::Text(run) = inline.kind() else {
                output.push(self.rewrite_inline(inline.clone())?);
                continue;
            };
            for piece in self.annotate_run(run.clone())? {
                output.push(Inline::from_parts(
                    InlineKind::Text(piece),
                    inline.marks().clone(),
                    inline.annotations().clone(),
                ));
            }
        }
        Ok(InlineContent::new(output))
    }

    fn rewrite_literal(&mut self, literal: LiteralText) -> Result<LiteralText, Self::Error> {
        let mut runs = Vec::new();
        for run in literal.runs() {
            runs.extend(self.annotate_run(run.clone())?);
        }
        Ok(LiteralText::new(runs))
    }
}

fn source_local_offset(run: &TextRun, source: StreamRange, offset: u64) -> usize {
    let source_delta = offset.saturating_sub(source.start().as_u64());
    let local = match run.provenance() {
        TextProvenance::Exact(_) => source_delta,
        TextProvenance::Derived(_) => source_delta
            .saturating_mul(run.text().len() as u64)
            .checked_div(source.len().max(1))
            .unwrap_or_default(),
        TextProvenance::Synthetic => 0,
    };
    usize::try_from(local)
        .unwrap_or(run.text().len())
        .min(run.text().len())
}

fn source_range_for_piece(
    run: &TextRun,
    source: StreamRange,
    local_start: usize,
    local_end: usize,
) -> StreamRange {
    match run.provenance() {
        TextProvenance::Exact(_) => StreamRange::new(
            source.start().saturating_add(local_start as u64),
            source.start().saturating_add(local_end as u64),
        ),
        TextProvenance::Derived(_) => {
            let start = source.start().as_u64().saturating_add(
                (local_start as u64)
                    .saturating_mul(source.len())
                    .checked_div(run.text().len().max(1) as u64)
                    .unwrap_or_default(),
            );
            let end = source.start().as_u64().saturating_add(
                (local_end as u64)
                    .saturating_mul(source.len())
                    .checked_div(run.text().len().max(1) as u64)
                    .unwrap_or_default(),
            );
            StreamRange::new(
                StreamOffset::new(start.min(source.end().as_u64())),
                StreamOffset::new(end.min(source.end().as_u64())),
            )
        }
        TextProvenance::Synthetic => StreamRange::new(source.start(), source.start()),
    }
}

// ContentPort IDs cross the structural/native boundary, so they must not be
// host-local: a View built for host A must not accidentally resolve to host B's
// port with the same local slot. IDs are monotonic and never reused.
static NEXT_CONTENT_PORT_ID: AtomicU64 = AtomicU64::new(1);

/// Immutable, Source-neutral Funnel configuration supplied by the control
/// transport. It has no active state, host, viewport, or projection cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostContentFunnel {
    pub family: ContentFamily,
    pub kind: TextFunnelKind,
    pub wrap: TextWrapMode,
    pub hyperlinks: bool,
    pub delivery: ContentDelivery,
}

impl HostContentFunnel {
    #[must_use]
    pub const fn plain(wrap: TextWrapMode) -> Self {
        Self {
            family: ContentFamily::Text,
            kind: TextFunnelKind::Plain,
            wrap,
            hyperlinks: true,
            delivery: ContentDelivery::Immediate,
        }
    }

    #[must_use]
    pub const fn new(
        kind: TextFunnelKind,
        wrap: TextWrapMode,
        hyperlinks: bool,
        delivery: ContentDelivery,
    ) -> Self {
        Self {
            family: ContentFamily::Text,
            kind,
            wrap,
            hyperlinks,
            delivery,
        }
    }

    fn smooth_config(&self) -> Option<SmoothConfig> {
        match self.delivery {
            ContentDelivery::Immediate => None,
            ContentDelivery::Smooth(config) => Some(config),
        }
    }

    fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.family.hash(&mut hasher);
        self.kind.hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.hyperlinks.hash(&mut hasher);
        match self.delivery {
            ContentDelivery::Immediate => 0u8.hash(&mut hasher),
            ContentDelivery::Smooth(config) => {
                1u8.hash(&mut hasher);
                config.tick_interval().hash(&mut hasher);
                config.spring().to_bits().hash(&mut hasher);
                config.min_units_per_second().to_bits().hash(&mut hasher);
                config.max_units_per_second().to_bits().hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRetentionPolicy {
    max_bytes: Option<u64>,
    max_lines: Option<u64>,
    drop_oldest: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectorLifecycle {
    Live,
    Disposing,
    Disposed,
}

#[derive(Clone, Debug)]
struct SourceSubscriptionGroup {
    host: Weak<Mutex<HostInner>>,
    tokens: Vec<(u64, u32)>,
    wake_tokens: Vec<(u64, u32)>,
}

struct CapturedSubscriberGroup {
    host_key: usize,
    host: Weak<Mutex<HostInner>>,
    tokens: Vec<(u64, u32)>,
}

impl std::fmt::Debug for CapturedSubscriberGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapturedSubscriberGroup")
            .field("host_key", &self.host_key)
            .field("tokens", &self.tokens)
            .finish()
    }
}

#[derive(Debug)]
struct ContentSourceRecord {
    id: u64,
    generation: u32,
    content_generation: u64,
    revision: u64,
    family: ContentFamily,
    kind: TextSourceKind,
    lifecycle: SourceLifecycle,
    retention: Option<SourceRetentionPolicy>,
    storage: Arc<StoredSource>,
    copied_bytes: u64,
    dropped_head_bytes: u64,
    accepted_bytes: u64,
    connector_count: usize,
    /// Host-grouped wake subscriptions.  The host allocation pointer is only
    /// an in-process map key; each value retains a Weak host and generation-
    /// checked Connector tokens for validation when a mutation is drained.
    subscribers: HashMap<usize, SourceSubscriptionGroup>,
    /// Reused outer wake batch storage. The Source lock is released before
    /// hosts are touched, so the batch is recycled only after every eligible
    /// host has been attempted.
    subscriber_wake_scratch: Vec<CapturedSubscriberGroup>,
}

/// A Source mutation can finish after releasing the Source lock, so a host
/// wake failure cannot be returned as the mutation's ordinary `Result`:
/// doing so would make an already-installed revision look rejected to the
/// caller.  Keep one latest failure per host in an environment-owned side
/// channel instead.  The weak host reference is retained to resolve the
/// current environment host ID without taking the possibly poisoned host
/// lock, and also prevents an allocator-address reuse from misattributing a
/// stale failure to a new host.
#[derive(Debug)]
pub(crate) struct PendingSourceWakeFailure {
    pub(super) host_key: usize,
    pub(super) host: Weak<Mutex<HostInner>>,
    pub(super) revision: u64,
    pub(super) diagnostic: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SourceWakeFailureChannel {
    failures: Arc<Mutex<HashMap<usize, PendingSourceWakeFailure>>>,
}

impl SourceWakeFailureChannel {
    pub(super) fn record(
        &self,
        host: &Weak<Mutex<HostInner>>,
        revision: u64,
        diagnostic: impl Into<String>,
    ) {
        let host_key = host.as_ptr() as usize;
        let mut failures = self
            .failures
            .lock()
            // A poisoned diagnostic channel must not turn accepted Source
            // bytes into an apparent mutation rejection.  The queue contains
            // only replaceable diagnostics, so recovering its guard is safe;
            // the failure remains explicit when the environment drains it.
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failures
            .get(&host_key)
            .is_some_and(|previous| previous.revision > revision)
        {
            return;
        }
        failures.insert(
            host_key,
            PendingSourceWakeFailure {
                host_key,
                host: host.clone(),
                revision,
                diagnostic: diagnostic.into(),
            },
        );
    }

    pub(super) fn take(&self) -> Vec<PendingSourceWakeFailure> {
        let mut failures = self
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut pending = failures
            .drain()
            .map(|(_, failure)| failure)
            .collect::<Vec<_>>();
        // HashMap iteration order is deliberately unspecified. Stable error
        // ordering keeps the environment report deterministic while retaining
        // O(number of failed hosts) extraction.
        pending.sort_unstable_by_key(|failure| (failure.host_key, failure.revision));
        pending
    }
}

#[derive(Debug, Default)]
struct ContentSourceRegistryInner {
    next_id: u64,
    next_generation: u32,
    sources: HashMap<u64, Arc<Mutex<ContentSourceRecord>>>,
}

/// Environment-owned Source registry. The registry holds the authoritative
/// record strongly so a Source can outlive any host and can be reused by later
/// hosts in the same environment.
#[derive(Clone, Debug)]
pub(crate) struct ContentSourceRegistry {
    inner: Arc<Mutex<ContentSourceRegistryInner>>,
    identity: EnvironmentIdentity,
    wake_failures: SourceWakeFailureChannel,
}

impl Default for ContentSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentSourceRegistry {
    pub(crate) fn new() -> Self {
        Self::with_identity(EnvironmentIdentity::allocate())
    }

    pub(crate) fn with_identity(identity: EnvironmentIdentity) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ContentSourceRegistryInner::default())),
            identity,
            wake_failures: SourceWakeFailureChannel::default(),
        }
    }

    pub(crate) fn identity(&self) -> EnvironmentIdentity {
        self.identity
    }

    pub(super) fn record_wake_failure(
        &self,
        host: &Weak<Mutex<HostInner>>,
        revision: u64,
        diagnostic: impl Into<String>,
    ) {
        self.wake_failures.record(host, revision, diagnostic);
    }

    pub(super) fn take_wake_failures(&self) -> Vec<PendingSourceWakeFailure> {
        self.wake_failures.take()
    }

    pub(crate) fn create(&self, kind: TextSourceKind) -> Result<HostContentSource> {
        let mut registry = self
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        let id = registry
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source identity exhausted"))?;
        if id > u64::from(u32::MAX) {
            return Err(anyhow!("content Source identity exhausted"));
        }
        let generation = registry
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source generation exhausted"))?;
        registry.next_id = id;
        registry.next_generation = generation;
        let record = Arc::new(Mutex::new(ContentSourceRecord {
            id,
            generation,
            content_generation: 1,
            revision: 0,
            family: ContentFamily::Text,
            kind,
            lifecycle: SourceLifecycle::Live,
            retention: None,
            storage: Arc::new(StoredSource::empty()),
            copied_bytes: 0,
            dropped_head_bytes: 0,
            accepted_bytes: 0,
            connector_count: 0,
            subscribers: HashMap::new(),
            subscriber_wake_scratch: Vec::new(),
        }));
        registry.sources.insert(id, Arc::clone(&record));
        Ok(HostContentSource {
            registry: self.clone(),
            record,
        })
    }

    fn contains(&self, id: u64, record: &Arc<Mutex<ContentSourceRecord>>) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|registry| registry.sources.get(&id).cloned())
            .is_some_and(|candidate| Arc::ptr_eq(&candidate, record))
    }

    pub(crate) fn lookup(&self, id: u64, generation: u32) -> Result<HostContentSource> {
        let record = self
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?
            .sources
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_SOURCE: Source {id} is unavailable"))?;
        let matches = record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?
            .generation
            == generation;
        if !matches {
            return Err(anyhow!("STALE_SOURCE: Source {id} generation is stale"));
        }
        Ok(HostContentSource {
            registry: self.clone(),
            record,
        })
    }
}

/// A native environment-owned Source identity and retained UTF-8 store.
#[derive(Clone, Debug)]
pub struct HostContentSource {
    registry: ContentSourceRegistry,
    record: Arc<Mutex<ContentSourceRecord>>,
}

fn ensure_source_live(record: &ContentSourceRecord) -> Result<()> {
    if record.lifecycle != SourceLifecycle::Live {
        return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
    }
    Ok(())
}

fn next_revision(revision: u64) -> Result<u64> {
    revision
        .checked_add(1)
        .ok_or_else(|| anyhow!("Source revision exhausted"))
}

fn validate_payload_size(length: usize) -> Result<()> {
    if length > MAX_SOURCE_PAYLOAD_BYTES {
        return Err(anyhow!(
            "PAYLOAD_TOO_LARGE: Source payload exceeds the configured limit"
        ));
    }
    Ok(())
}

fn decode_annotations(
    input: &ValidatedInput<'_>,
    absolute_base: u64,
    records: &[ContentAnnotationRecord],
    payload: &[u8],
) -> Result<Vec<ValidatedAnnotation>> {
    if records.len() > MAX_SOURCE_ANNOTATIONS {
        return Err(anyhow!(
            "LIMIT_EXCEEDED: annotation count exceeds the configured limit"
        ));
    }
    if payload.len() > MAX_ANNOTATION_PAYLOAD_BYTES {
        return Err(anyhow!(
            "PAYLOAD_TOO_LARGE: annotation payload exceeds the configured limit"
        ));
    }
    let text = input.text();
    let bytes = input.bytes();
    records
        .iter()
        .map(|record| {
            let policy = match record.kind {
                CONTENT_ANNOTATION_KIND_TAG
                | CONTENT_ANNOTATION_KIND_STYLE
                | CONTENT_ANNOTATION_KIND_ATOMIC
                | CONTENT_ANNOTATION_KIND_POINT => annotation_policy(record.kind),
                _ => {
                    return Err(anyhow!(
                        "UNKNOWN_ANNOTATION_KIND: annotation kind {} is unsupported",
                        record.kind
                    ));
                }
            };
            if record.flags != 0 || record.aux0 != 0 || record.aux1 != 0 {
                return Err(anyhow!(
                    "INVALID_ANNOTATION_PAYLOAD: annotation flags or auxiliary lanes are reserved"
                ));
            }
            let start = usize::try_from(record.start_byte)
                .map_err(|_| anyhow!("INVALID_RANGE: annotation start does not fit usize"))?;
            let end = usize::try_from(record.end_byte)
                .map_err(|_| anyhow!("INVALID_RANGE: annotation end does not fit usize"))?;
            if start > end
                || end > bytes.len()
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
            {
                return Err(anyhow!(
                    "INVALID_RANGE: annotation range is not an ordered UTF-8 range"
                ));
            }
            if policy == AnnotationTruncationPolicy::Point && start != end {
                return Err(anyhow!(
                    "INVALID_RANGE: point annotations must have an empty range"
                ));
            }
            if policy != AnnotationTruncationPolicy::Point && start == end {
                return Err(anyhow!(
                    "INVALID_RANGE: non-point annotations must cover text"
                ));
            }
            let payload_end = record
                .payload_offset
                .checked_add(record.payload_length)
                .ok_or_else(|| anyhow!("INVALID_ANNOTATION_PAYLOAD: payload range overflow"))?;
            if payload_end as usize > payload.len() {
                return Err(anyhow!(
                    "INVALID_ANNOTATION_PAYLOAD: annotation payload range is outside the sidecar"
                ));
            }
            let annotation_payload = &payload[record.payload_offset as usize..payload_end as usize];
            let (tag, style) = match record.kind {
                CONTENT_ANNOTATION_KIND_TAG => (Some(decode_tag(annotation_payload)?), None),
                CONTENT_ANNOTATION_KIND_STYLE => {
                    (None, Some(decode_semantic_style(annotation_payload)?))
                }
                CONTENT_ANNOTATION_KIND_ATOMIC | CONTENT_ANNOTATION_KIND_POINT => (None, None),
                _ => unreachable!("annotation kind was validated above"),
            };
            let absolute_start = absolute_base
                .checked_add(record.start_byte as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: annotation coordinate exhausted"))?;
            let absolute_end = absolute_base
                .checked_add(record.end_byte as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: annotation coordinate exhausted"))?;
            Ok(ValidatedAnnotation {
                kind: record.kind,
                flags: record.flags,
                start_byte: absolute_start,
                end_byte: absolute_end,
                payload: payload[record.payload_offset as usize..payload_end as usize].to_vec(),
                aux0: record.aux0,
                aux1: record.aux1,
                tag,
                style,
            })
        })
        .collect()
}

fn decode_tag(payload: &[u8]) -> Result<crate::text::SemanticTag> {
    let separator = payload.iter().position(|byte| *byte == 0).ok_or_else(|| {
        anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: tag annotations require a NUL-separated namespace and name"
        )
    })?;
    if payload[separator + 1..].contains(&0) {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: tag annotation names must not contain NUL"
        ));
    }
    let namespace = str::from_utf8(&payload[..separator])
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: tag namespace is not valid UTF-8"))?;
    let name = str::from_utf8(&payload[separator + 1..])
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: tag name is not valid UTF-8"))?;
    crate::text::SemanticTag::new(namespace, name)
        .map_err(|error| anyhow!("INVALID_ANNOTATION_PAYLOAD: {error}"))
}

const STYLE_PAYLOAD_VERSION: u8 = 1;
const STYLE_FLAG_ROLE: u8 = 1 << 0;
const STYLE_FLAG_FOREGROUND: u8 = 1 << 1;
const STYLE_FLAG_BACKGROUND: u8 = 1 << 2;
const STYLE_FLAG_ATTRIBUTES: u8 = 1 << 3;

fn decode_semantic_style(payload: &[u8]) -> Result<StyleRef> {
    if payload.len() < 4 || payload[0] != STYLE_PAYLOAD_VERSION {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload version is unsupported"
        ));
    }
    let flags = payload[1];
    if flags & !0x0f != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload has reserved flags"
        ));
    }
    let presence = payload[2];
    let values = payload[3];
    if presence & !0x3f != 0 || values & !presence != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style attributes are malformed"
        ));
    }
    let mut cursor = 4usize;
    let role = if flags & STYLE_FLAG_ROLE != 0 {
        Some(read_style_string(payload, &mut cursor, "role")?)
    } else {
        None
    };
    let foreground = if flags & STYLE_FLAG_FOREGROUND != 0 {
        Some(read_style_color(payload, &mut cursor, "foreground")?)
    } else {
        None
    };
    let background = if flags & STYLE_FLAG_BACKGROUND != 0 {
        Some(read_style_color(payload, &mut cursor, "background")?)
    } else {
        None
    };
    if cursor != payload.len() {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload has trailing bytes"
        ));
    }
    let mut style = StyleSpec::new();
    if let Some(color) = foreground {
        style.set_foreground(color);
    }
    if let Some(color) = background {
        style.set_background(color);
    }
    if flags & STYLE_FLAG_ATTRIBUTES != 0 {
        for (bit, attribute) in [
            (1, TextAttribute::Bold),
            (2, TextAttribute::Dim),
            (4, TextAttribute::Italic),
            (8, TextAttribute::Underline),
            (16, TextAttribute::Reversed),
            (32, TextAttribute::Strikethrough),
        ] {
            if presence & bit != 0 {
                style.set_attribute(attribute, values & bit != 0);
            }
        }
    } else if presence != 0 || values != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style attributes flag is missing"
        ));
    }
    Ok(match role {
        Some(role) => StyleRef::themed(role, style),
        None => StyleRef::themed(crate::content::text::TEXT_THEME_KEY, style),
    })
}

fn read_style_string(payload: &[u8], cursor: &mut usize, field: &str) -> Result<String> {
    let length = read_style_u16(payload, cursor, field)? as usize;
    let end = cursor.checked_add(length).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} length overflow")
    })?;
    let value = payload.get(*cursor..end).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} is truncated")
    })?;
    *cursor = end;
    let value = str::from_utf8(value)
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} is not UTF-8"))?;
    if value.is_empty() || value.contains('\0') || value.chars().any(char::is_whitespace) {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style {field} is not a valid name"
        ));
    }
    Ok(value.to_owned())
}

fn read_style_u16(payload: &[u8], cursor: &mut usize, field: &str) -> Result<u16> {
    let end = (*cursor).saturating_add(2);
    let bytes = payload.get(*cursor..end).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} length is truncated")
    })?;
    *cursor = end;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_style_color(payload: &[u8], cursor: &mut usize, field: &str) -> Result<ColorSpec> {
    let kind = *payload.get(*cursor).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} color is truncated")
    })?;
    *cursor += 1;
    match kind {
        1 => Ok(ColorSpec::named(read_ansi_color(payload, cursor, field)?)),
        2 => Ok(ColorSpec::ansi(read_style_byte(payload, cursor, field)?)),
        3 => Ok(ColorSpec::rgb(
            read_style_byte(payload, cursor, field)?,
            read_style_byte(payload, cursor, field)?,
            read_style_byte(payload, cursor, field)?,
        )),
        4 => Ok(ColorSpec::theme(read_style_string(payload, cursor, field)?)),
        _ => Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style {field} color kind is unknown"
        )),
    }
}

fn read_style_byte(payload: &[u8], cursor: &mut usize, field: &str) -> Result<u8> {
    let value = *payload.get(*cursor).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} color is truncated")
    })?;
    *cursor += 1;
    Ok(value)
}

fn read_ansi_color(payload: &[u8], cursor: &mut usize, field: &str) -> Result<AnsiColor> {
    let value = read_style_byte(payload, cursor, field)?;
    let color = match value {
        0 => AnsiColor::Black,
        1 => AnsiColor::Red,
        2 => AnsiColor::Green,
        3 => AnsiColor::Yellow,
        4 => AnsiColor::Blue,
        5 => AnsiColor::Magenta,
        6 => AnsiColor::Cyan,
        7 => AnsiColor::Gray,
        8 => AnsiColor::DarkGray,
        9 => AnsiColor::LightRed,
        10 => AnsiColor::LightGreen,
        11 => AnsiColor::LightYellow,
        12 => AnsiColor::LightBlue,
        13 => AnsiColor::LightMagenta,
        14 => AnsiColor::LightCyan,
        15 => AnsiColor::White,
        _ => {
            return Err(anyhow!(
                "INVALID_ANNOTATION_PAYLOAD: semantic style {field} ANSI color is unknown"
            ));
        }
    };
    Ok(color)
}

fn retention_head(storage: &StoredSource, retention: Option<SourceRetentionPolicy>) -> u64 {
    let Some(retention) = retention else {
        return storage.base();
    };
    let mut head = storage.base();
    if let Some(max_bytes) = retention.max_bytes
        && storage.end().saturating_sub(storage.base()) > max_bytes
    {
        head = head.max(storage.offset_for_max_bytes(max_bytes));
    }
    if let Some(max_lines) = retention.max_lines
        && storage.line_count() as u64 > max_lines
    {
        let keep = usize::try_from(max_lines).unwrap_or(usize::MAX);
        let index = storage.line_count().saturating_sub(keep);
        if let Some(line_start) = storage.line_entry(storage.base(), index as u64) {
            head = head.max(line_start);
        }
    }
    head
}

fn retention_would_overflow(
    storage: &StoredSource,
    retention: Option<SourceRetentionPolicy>,
    appended_bytes: usize,
    appended_newlines: usize,
) -> bool {
    let Some(policy) = retention else {
        return false;
    };
    (!policy.drop_oldest
        && policy.max_bytes.is_some_and(|limit| {
            storage
                .end()
                .saturating_sub(storage.base())
                .saturating_add(appended_bytes as u64)
                > limit
        }))
        || (!policy.drop_oldest
            && policy.max_lines.is_some_and(|limit| {
                (storage.line_count() as u64).saturating_add(appended_newlines as u64) > limit
            }))
}

fn apply_retention(
    storage: StoredSource,
    retention: Option<SourceRetentionPolicy>,
) -> Result<(StoredSource, u64)> {
    let head = retention_head(&storage, retention);
    if head == storage.base() {
        return Ok((storage, 0));
    }
    let policy = retention.expect("retention head requires a policy");
    if !policy.drop_oldest {
        return Err(anyhow!(
            "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
        ));
    }
    let (next, dropped) = storage
        .apply_truncate(storage.base(), head, storage.revision())
        .map_err(|err| anyhow!("{err}"))?;
    Ok((next, dropped))
}

fn capture_subscribers(record: &mut ContentSourceRecord) -> Vec<CapturedSubscriberGroup> {
    let mut captured = std::mem::take(&mut record.subscriber_wake_scratch);
    captured.clear();
    record
        .subscribers
        .retain(|_, group| group.host.strong_count() != 0 && !group.tokens.is_empty());
    for (&host_key, group) in record.subscribers.iter_mut() {
        let mut tokens = std::mem::take(&mut group.wake_tokens);
        tokens.clear();
        tokens.extend(group.tokens.iter().copied());
        captured.push(CapturedSubscriberGroup {
            host_key,
            host: group.host.clone(),
            tokens,
        });
    }
    captured
}

impl HostContentSource {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.record.lock().map_or(0, |record| record.id)
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.record.lock().map_or(0, |record| record.generation)
    }

    #[must_use]
    pub fn environment_slot(&self) -> u32 {
        self.registry.identity.slot
    }

    #[must_use]
    pub fn environment_generation(&self) -> u32 {
        self.registry.identity.generation
    }

    #[must_use]
    pub fn family(&self) -> ContentFamily {
        self.record
            .lock()
            .map_or(ContentFamily::Text, |record| record.family)
    }

    #[must_use]
    pub fn kind(&self) -> TextSourceKind {
        self.record
            .lock()
            .map_or(TextSourceKind::Stream, |record| record.kind)
    }

    fn retention_compatible(&self, funnel: TextFunnelKind) -> Result<bool> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if funnel != TextFunnelKind::Markdown {
            return Ok(true);
        }
        let truncated =
            record.retention.is_some_and(|policy| policy.drop_oldest) || record.storage.base() != 0;
        Ok(!truncated)
    }

    pub fn content_generation(&self) -> Result<u64> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        Ok(record.content_generation)
    }

    pub fn snapshot(&self) -> Result<HostContentSourceSnapshot> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        crate::perf::inc(crate::perf::Counter::SourceSnapshotsAcquired);
        let storage = Arc::clone(&record.storage);
        Ok(HostContentSourceSnapshot {
            source_id: record.id,
            source_generation: record.generation,
            content_generation: record.content_generation,
            revision: record.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            storage,
        })
    }

    pub fn stats(&self) -> Result<HostContentSourceStats> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        ensure_source_live(&record)?;
        let storage = &record.storage;
        Ok(HostContentSourceStats {
            revision: record.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            retained_bytes: storage.retained_bytes(),
            retained_lines: storage.line_count() as u64,
            chunk_count: storage.chunk_count(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            accepted_bytes: record.accepted_bytes,
            copied_bytes: record.copied_bytes,
            dropped_head_bytes: record.dropped_head_bytes,
        })
    }

    /// Appends one validated UTF-8 payload to a Stream Source. The payload is
    /// copied into immutable chunks before the Source lock is released.
    pub fn append_utf8(
        &self,
        bytes: &[u8],
        annotations: &[ContentAnnotationRecord],
        annotation_payload: &[u8],
    ) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind != TextSourceKind::Stream {
                return Err(anyhow!("INVALID_ARGUMENT: append requires a stream Source"));
            }
            if record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            validate_payload_size(bytes.len())?;
            let input = ValidatedInput::from_bytes(bytes)?;
            let base = record.storage.end();
            base.checked_add(input.len() as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: Source coordinate exhausted"))?;
            let parsed = decode_annotations(&input, base, annotations, annotation_payload)?;
            if input.is_empty() && parsed.is_empty() {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            if retention_would_overflow(
                &record.storage,
                record.retention,
                input.len(),
                input.newlines(),
            ) {
                return Err(anyhow!(
                    "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
                ));
            }
            let retention = record.retention;
            // Preflight the fallible revision arithmetic before installing
            // candidate storage: a rejection must leave bytes, annotations,
            // revision and accounting exactly as they were (§9.6).
            let revision = next_revision(record.revision)?;
            let next = record
                .storage
                .apply_append(input.text(), revision, parsed)
                .map_err(|err| anyhow!("{err}"))?;
            let (next, dropped) = apply_retention(next, retention)?;
            record.storage = Arc::new(next);
            record.revision = revision;
            record.copied_bytes = record.copied_bytes.saturating_add(input.len() as u64);
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            record.accepted_bytes = record.accepted_bytes.saturating_add(input.len() as u64);
            (revision, capture_subscribers(&mut record))
        };
        self.finish_mutation(revision, subscribers)
    }

    /// Atomically replaces a Block or Stream Source with a fresh content
    /// generation. Existing snapshots retain their old immutable storage.
    pub fn replace_utf8(
        &self,
        bytes: &[u8],
        annotations: &[ContentAnnotationRecord],
        annotation_payload: &[u8],
    ) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind == TextSourceKind::Stream && record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            validate_payload_size(bytes.len())?;
            let input = ValidatedInput::from_bytes(bytes)?;
            let parsed = decode_annotations(&input, 0, annotations, annotation_payload)?;
            let revision = next_revision(record.revision)?;
            let content_generation = record
                .content_generation
                .checked_add(1)
                .ok_or_else(|| anyhow!("Source content generation exhausted"))?;
            let next = StoredSource::empty()
                .apply_append(input.text(), revision, parsed)
                .map_err(|err| anyhow!("{err}"))?;
            let (next, dropped) = apply_retention(next, record.retention)?;
            record.storage = Arc::new(next);
            record.content_generation = content_generation;
            record.revision = revision;
            record.copied_bytes = record.copied_bytes.saturating_add(input.len() as u64);
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            record.accepted_bytes = record.accepted_bytes.saturating_add(input.len() as u64);
            (revision, capture_subscribers(&mut record))
        };
        self.finish_mutation(revision, subscribers)
    }

    pub fn clear(&self) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind == TextSourceKind::Stream && record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            if record.storage.base() == 0
                && record.storage.end() == 0
                && record.storage.annotation_count() == 0
            {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            // Preflight both fallible counters before swapping in the empty
            // root: a rejection must leave the retained bytes in place (§9.6).
            let revision = next_revision(record.revision)?;
            let content_generation = record
                .content_generation
                .checked_add(1)
                .ok_or_else(|| anyhow!("Source content generation exhausted"))?;
            record.storage = Arc::new(StoredSource::empty());
            record.content_generation = content_generation;
            record.revision = revision;
            (revision, capture_subscribers(&mut record))
        };
        self.finish_mutation(revision, subscribers)
    }

    pub fn seal(&self) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind != TextSourceKind::Stream {
                return Err(anyhow!("INVALID_ARGUMENT: seal requires a stream Source"));
            }
            if record.storage.sealed() {
                return Err(anyhow!("SOURCE_ALREADY_SEALED: Source is already sealed"));
            }
            // Preflight the revision before flipping the flag: a rejection
            // must not report a sealed Source at a stale revision (§9.6).
            let revision = next_revision(record.revision)?;
            let next = record
                .storage
                .apply_seal(record.storage.base(), record.storage.end(), revision, None)
                .map_err(|err| anyhow!("{err}"))?;
            record.storage = Arc::new(next);
            record.revision = revision;
            (revision, capture_subscribers(&mut record))
        };
        self.finish_mutation(revision, subscribers)
    }

    /// Advances the retained head without renumbering absolute coordinates.
    pub fn truncate_head(&self, offset: u64) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if offset < record.storage.base() || offset > record.storage.end() {
                return Err(anyhow!(
                    "INVALID_RANGE: Source head is outside the retained range"
                ));
            }
            if !record.storage.is_boundary(offset) {
                return Err(anyhow!(
                    "INVALID_RANGE: Source head must be a UTF-8 scalar boundary"
                ));
            }
            if offset == record.storage.base() {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            // Preflight the revision before dropping the head: a rejection
            // must not move the retained range at a stale revision (§9.6).
            let revision = next_revision(record.revision)?;
            let (next, dropped) = record
                .storage
                .apply_truncate(record.storage.base(), offset, revision)
                .map_err(|err| anyhow!("{err}"))?;
            record.storage = Arc::new(next);
            record.revision = revision;
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            (revision, capture_subscribers(&mut record))
        };
        self.finish_mutation(revision, subscribers)
    }

    fn finish_mutation(
        &self,
        revision: u64,
        mut groups: Vec<CapturedSubscriberGroup>,
    ) -> Result<ContentMutationResult> {
        let mut schedule_environment_drain = false;
        let mut environment_wake_epoch = 0;
        // A failed subscriber must not cancel the remaining wakes (§9.6):
        // every eligible host is attempted, and a failure is reported through
        // the environment's per-host error channel.  In particular, do not
        // return an ordinary mutation error after the Source revision is
        // installed; that would make a successful append look retryable to a
        // direct-FFI caller and could duplicate its bytes.
        crate::perf::add(crate::perf::Counter::ContentWakeGroups, groups.len() as u64);
        for group in &groups {
            let Some(host) = group.host.upgrade() else {
                continue;
            };
            let tokens = &group.tokens;
            let wake_result: Result<()> = (|| {
                let mut host = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
                let mut dirty = std::mem::take(&mut host.content_dirty_scratch);
                dirty.clear();
                for (id, generation) in tokens.iter().copied() {
                    let Some(dirty_item) = host
                        .content
                        .source_subscription_is_live(id, generation, revision)?
                    else {
                        continue;
                    };
                    dirty.push(dirty_item);
                }
                if !dirty.is_empty() {
                    let wake = host.mark_content_pending_batch(&dirty)?;
                    schedule_environment_drain |= wake.schedule_environment_drain;
                    environment_wake_epoch = host.environment_wake_epoch();
                }
                host.content_dirty_scratch = dirty;
                Ok(())
            })();
            if let Err(error) = wake_result {
                schedule_environment_drain = true;
                self.registry.record_wake_failure(
                    &group.host,
                    revision,
                    format!(
                        "SOURCE_WAKE_FAILED: Source revision {revision} was accepted; \
                         subscriber host wake failed: {error:#}"
                    ),
                );
            }
        }
        // Reuse the per-Source outer batch allocation on the next mutation;
        // the Source mutex is reacquired only after all host locks have been
        // released, preserving the no-Source-lock→Host-lock ordering.
        match self.record.lock() {
            Ok(mut record) => {
                for group in &mut groups {
                    if let Some(subscriber) = record.subscribers.get_mut(&group.host_key) {
                        subscriber.wake_tokens = std::mem::take(&mut group.tokens);
                    }
                }
                record.subscriber_wake_scratch = groups;
            }
            Err(_) => {
                // The Source was already accepted, so a poisoned record at
                // this cleanup point is also an environment-visible wake
                // failure, not a mutation rejection.  The captured weak
                // references identify the affected host channels without
                // touching those hosts again.  If all subscribers raced away,
                // there is no remaining host channel to report to; the next
                // Source operation will explicitly surface the poisoned
                // Source lock.
                for group in &groups {
                    schedule_environment_drain = true;
                    self.registry.record_wake_failure(
                        &group.host,
                        revision,
                        format!(
                            "SOURCE_WAKE_FAILED: Source revision {revision} was accepted; \
                             Source lock is poisoned while recycling wake storage"
                        ),
                    );
                }
            }
        }
        Ok(ContentMutationResult {
            revision,
            environment_wake_epoch,
            schedule_environment_drain,
        })
    }

    pub(crate) fn same_environment(&self, registry: &ContentSourceRegistry) -> bool {
        Arc::ptr_eq(&self.registry.inner, &registry.inner)
    }

    #[must_use]
    pub fn is_live(&self) -> bool {
        self.record
            .lock()
            .is_ok_and(|record| record.lifecycle == SourceLifecycle::Live)
            && self.registry.contains(self.id(), &self.record)
    }

    pub fn dispose(&self) -> Result<()> {
        let source_id = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            if record.lifecycle == SourceLifecycle::Disposed {
                return Ok(());
            }
            if record.connector_count != 0 {
                return Err(anyhow!(
                    "SOURCE_IN_USE: Source has {} Connector membership(s)",
                    record.connector_count
                ));
            }
            record.lifecycle = SourceLifecycle::Disposed;
            record.subscribers.clear();
            record.id
        };
        let mut registry = self
            .registry
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        if registry
            .sources
            .get(&source_id)
            .is_some_and(|candidate| Arc::ptr_eq(candidate, &self.record))
        {
            registry.sources.remove(&source_id);
        }
        Ok(())
    }

    /// Stores the creation-time retention policy used by Source mutations.
    #[doc(hidden)]
    pub fn configure_retention(
        &self,
        max_bytes: Option<u64>,
        max_lines: Option<u64>,
        drop_oldest: bool,
    ) -> Result<()> {
        if max_bytes.is_none() && max_lines.is_none()
            || max_bytes.is_some_and(|value| value == 0)
            || max_lines.is_some_and(|value| value == 0)
        {
            return Err(anyhow!(
                "INVALID_ARGUMENT: Source retention limits must be positive"
            ));
        }
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if record.connector_count != 0 {
            return Err(anyhow!(
                "SOURCE_IN_USE: Source retention cannot change while Connectors exist"
            ));
        }
        let retention = SourceRetentionPolicy {
            max_bytes,
            max_lines,
            drop_oldest,
        };
        if !drop_oldest && retention_head(&record.storage, Some(retention)) > record.storage.base()
        {
            return Err(anyhow!(
                "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
            ));
        }
        record.retention = Some(retention);
        Ok(())
    }

    fn acquire_connector(&self) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record.connector_count = record
            .connector_count
            .checked_add(1)
            .ok_or_else(|| anyhow!("Source Connector membership count exhausted"))?;
        Ok(())
    }

    fn release_connector(&self) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        record.connector_count = record.connector_count.saturating_sub(1);
        if record.connector_count == 0 {
            record.subscribers.clear();
        }
        Ok(())
    }

    fn subscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record
            .subscribers
            .retain(|_, subscriber| subscriber.host.strong_count() != 0);
        let host_key = host.as_ptr() as usize;
        let group = record
            .subscribers
            .entry(host_key)
            .or_insert_with(|| SourceSubscriptionGroup {
                host: host.clone(),
                tokens: Vec::new(),
                wake_tokens: Vec::new(),
            });
        group
            .tokens
            .retain(|token| *token != (connector_id, connector_generation));
        group.tokens.push((connector_id, connector_generation));
        Ok(())
    }

    fn unsubscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        let host_key = host.as_ptr() as usize;
        let remove_group = record.subscribers.get_mut(&host_key).is_some_and(|group| {
            group
                .tokens
                .retain(|token| *token != (connector_id, connector_generation));
            group.tokens.is_empty()
        });
        if remove_group {
            record.subscribers.remove(&host_key);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.record.lock().map_or(0, |record| {
            record
                .subscribers
                .values()
                .map(|group| group.tokens.len())
                .sum()
        })
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryPortState {
    pub(crate) history_unit: Option<u64>,
    pub(crate) history_insets: crate::presentation::Insets,
    pub(crate) history_committed_rows: usize,
    pub(crate) history_committed_content_rows: usize,
    pub(crate) history_leading_padding_rows: usize,
    pub(crate) history_trailing_padding_rows: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryTerminalAdapter {
    ports: HashMap<u64, HistoryPortState>,
    unit_ports: HashMap<u64, HashSet<u64>>,
}

impl HistoryTerminalAdapter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn bind_unit(
        &mut self,
        port_id: u64,
        unit_id: u64,
        insets: crate::presentation::Insets,
    ) {
        if let Some(Some(old_unit)) = self.ports.get(&port_id).map(|state| state.history_unit)
            && old_unit != unit_id
            && let Some(ports) = self.unit_ports.get_mut(&old_unit)
        {
            ports.remove(&port_id);
        }
        self.ports.insert(
            port_id,
            HistoryPortState {
                history_unit: Some(unit_id),
                history_insets: insets,
                history_committed_rows: 0,
                history_committed_content_rows: 0,
                history_leading_padding_rows: 0,
                history_trailing_padding_rows: 0,
            },
        );
        self.unit_ports.entry(unit_id).or_default().insert(port_id);
    }

    pub(crate) fn unit_id(&self, port_id: u64) -> Option<u64> {
        self.ports.get(&port_id).and_then(|s| s.history_unit)
    }

    pub(crate) fn committed_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_committed_rows)
    }

    pub(crate) fn committed_content_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_committed_content_rows)
    }

    pub(crate) fn leading_padding_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_leading_padding_rows)
    }

    pub(crate) fn trailing_padding_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_trailing_padding_rows)
    }

    pub(crate) fn insets(&self, port_id: u64) -> crate::presentation::Insets {
        self.ports
            .get(&port_id)
            .map_or(crate::presentation::Insets::ZERO, |s| s.history_insets)
    }

    pub(crate) fn record_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        if let Some(state) = self.ports.get_mut(&port_id) {
            state.history_committed_rows = state.history_committed_rows.saturating_add(rows);
            state.history_committed_content_rows = state
                .history_committed_content_rows
                .saturating_add(content_rows.min(rows));
            state.history_leading_padding_rows = state
                .history_leading_padding_rows
                .saturating_add(leading_padding.min(rows));
            state.history_trailing_padding_rows = state
                .history_trailing_padding_rows
                .saturating_add(trailing_padding.min(rows));
        }
    }

    pub(crate) fn clear_unit(&mut self, unit_id: u64) {
        if let Some(port_ids) = self.unit_ports.remove(&unit_id) {
            for port_id in port_ids {
                if let Some(state) = self.ports.get_mut(&port_id) {
                    if state.history_unit != Some(unit_id) {
                        continue;
                    }
                    state.history_unit = None;
                    state.history_committed_rows = 0;
                    state.history_committed_content_rows = 0;
                    state.history_leading_padding_rows = 0;
                    state.history_trailing_padding_rows = 0;
                }
            }
        }
    }

    pub(crate) fn retire_unit(&mut self, unit_id: u64) -> Vec<u64> {
        let matching = self
            .unit_ports
            .remove(&unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        for port_id in &matching {
            self.ports.remove(port_id);
        }
        matching
    }

    pub(crate) fn hash_state<H: std::hash::Hasher>(&self, port_id: u64, hasher: &mut H) {
        use std::hash::Hash;
        if let Some(state) = self.ports.get(&port_id) {
            state.history_committed_rows.hash(hasher);
            state.history_committed_content_rows.hash(hasher);
            state.history_leading_padding_rows.hash(hasher);
            state.history_trailing_padding_rows.hash(hasher);
        }
    }
}

#[derive(Clone, Debug)]
struct PortRecord {
    id: u64,
    generation: u32,
    family: ContentFamily,
    lifecycle: PortLifecycle,
    host: Weak<Mutex<HostInner>>,
    connector_ids: HashSet<u64>,
    desired_mounted: bool,
    visible_mounted: bool,
    desired_connector: Option<u64>,
    visible_connector: Option<u64>,
}

#[derive(Debug)]
struct ConnectorRecord {
    id: u64,
    generation: u32,
    lifecycle: ConnectorLifecycle,
    port: Weak<Mutex<PortRecord>>,
    source: HostContentSource,
    funnel: HostContentFunnel,
    requested: bool,
    visible: bool,
    subscribed: bool,
    /// Source membership is independent from wake subscription. This receipt
    /// bit makes disposal/retry release the Source membership exactly once.
    membership_released: bool,
    /// A post-promotion Source cleanup that could not acquire its Source lock.
    /// This status is distinct from projection/activation failure: the
    /// logical frame is already visible and the Source membership remains
    /// retained until the cleanup succeeds.
    cleanup_error: Option<Arc<ContentConnectorError>>,
    phase: &'static str,
    error: Option<ContentConnectorError>,
    /// Source revision observed at the start of the last failed candidate.
    /// A later Source revision clears the retryable error exactly once.
    failed_source_revision: Option<u64>,
    /// Deterministic synthetic operational failure used by native/unit
    /// fixtures to exercise transactional switch rollback.
    activation_failure: Option<String>,
    /// Connector-local width-dependent derived projections. Inactive connectors
    /// clear this cache; the Source remains the authoritative store.
    projection_cache: VecDeque<(TextProjectionKey, Arc<HostContentProjection>)>,
    /// Connector-local width-dependent unmasked paint cache and visibility index.
    /// Reused across delivery ticks without reparsing or fresh View lowering.
    prepared_paint_cache: PreparedPaintCache,
    /// Connector-local theme-independent semantic IR. A palette/presentation
    /// recolor reuses these products and repaints only; inactive connectors
    /// clear this cache alongside the surface products.
    semantic_cache: SemanticProjectionCache,
    prefix_proof_cache: PrefixProofCache,
    committed_projection: Option<Arc<HostContentProjection>>,
    candidate_projection: Option<Arc<HostContentProjection>>,
    projected_source_revision: Option<u64>,
    projection_failure_key: Option<TextProjectionKey>,
    /// Monotonic control revision used to keep newer requested selection
    /// changes independent from an older in-flight candidate cleanup.
    control_revision: u64,
    delivery_revision: u64,
    candidate_delivery_frontier: StreamOffset,
    committed_delivery_frontier: StreamOffset,
    execution: Option<ConnectorExecution>,
}

/// A prevalidated visible-association change.  The Arc records are captured
/// while the candidate is prepared, so receipt-time promotion never resolves
/// a handle through the live registries or builds a replacement collection.
#[derive(Debug)]
struct PreparedContentPort {
    id: u64,
    record: Arc<Mutex<PortRecord>>,
    mounted: bool,
    old_connector_id: Option<u64>,
    old_connector_index: Option<usize>,
    old_control_revision: Option<u64>,
    next_connector_id: Option<u64>,
    next_connector_index: Option<usize>,
    retry_selection: bool,
}

#[derive(Debug)]
struct PreparedContentConnector {
    id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    source: HostContentSource,
    source_id: u64,
    generation: u32,
    requested: bool,
    subscribed: bool,
    control_revision: u64,
    deadline: Option<Instant>,
    visible: bool,
    /// Immutable projection captured with this candidate. Receipt promotion
    /// must use this Arc rather than consuming whatever candidate happens to
    /// be installed after a newer delivery tick or Source revision.
    candidate_projection: Option<Arc<HostContentProjection>>,
    delivery_frontier: StreamOffset,
    delivery_input: Option<(u32, u64, bool)>,
    delivery_revision: u64,
}

#[derive(Debug)]
struct PreparedContentSource {
    id: u64,
    source: HostContentSource,
}

#[derive(Clone, Copy, Debug)]
struct PreparedContentBindingChange {
    port_index: usize,
    revision: u64,
}

#[derive(Clone, Debug)]
struct PreparedSourceCleanup {
    source: HostContentSource,
    source_id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    connector_id: u64,
    connector_generation: u32,
    unsubscribe: bool,
    error: Arc<ContentConnectorError>,
}

/// Candidate-owned content commit data.  It is deliberately separate from
/// the mutable desired Port/Connector tables: operations accepted while a
/// backend receipt is outstanding cannot consume or overwrite this plan.
#[derive(Debug)]
pub(crate) struct PreparedContentCommit {
    ports: Vec<PreparedContentPort>,
    connectors: Vec<PreparedContentConnector>,
    sources: Vec<PreparedContentSource>,
    source_cleanups: Vec<PreparedSourceCleanup>,
    binding_changes: Vec<PreparedContentBindingChange>,
}

fn remove_source_subscription_locked(
    source: &mut ContentSourceRecord,
    host: &Weak<Mutex<HostInner>>,
    connector_id: u64,
    connector_generation: u32,
) {
    let host_key = host.as_ptr() as usize;
    let remove_group = source.subscribers.get_mut(&host_key).is_some_and(|group| {
        group
            .tokens
            .retain(|token| *token != (connector_id, connector_generation));
        group.tokens.is_empty()
    });
    if remove_group {
        source.subscribers.remove(&host_key);
    }
}

fn release_source_membership_locked(source: &mut ContentSourceRecord) {
    source.connector_count = source.connector_count.saturating_sub(1);
    if source.connector_count == 0 {
        source.subscribers.clear();
    }
}

fn set_connector_visible_committed(
    active_deadlines: &mut HashMap<u64, Instant>,
    active_connectors: &mut HashSet<u64>,
    connector: &PreparedContentConnector,
    visible: bool,
    preserve_newer_control: bool,
) {
    let connector_id = connector.id;
    let mut state = connector
        .record
        .lock()
        .expect("prepared Connector lock must remain usable during visible commit");
    state.visible = visible;
    if visible {
        state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
            "disposing"
        } else {
            "active"
        };
        return;
    }
    if !preserve_newer_control {
        state.committed_projection = None;
        state.candidate_projection = None;
        state.projection_cache.clear();
        state.prepared_paint_cache.clear();
        state.semantic_cache.clear();
        state.prefix_proof_cache.clear();
        state.projected_source_revision = None;
        state.projection_failure_key = None;
        state.execution = None;
        state.delivery_revision = 0;
        state.candidate_delivery_frontier = StreamOffset::ZERO;
        state.committed_delivery_frontier = StreamOffset::ZERO;
    }
    if state.lifecycle != ConnectorLifecycle::Disposing {
        state.phase = if state.error.is_some() && state.requested {
            "failed"
        } else if state.requested {
            "activation-pending"
        } else {
            "idle"
        };
    }
    active_deadlines.remove(&connector_id);
    active_connectors.remove(&connector_id);
}

fn remove_prepared_connector_committed(
    connectors: &mut HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
    connector: &PreparedContentConnector,
) {
    let mut state = connector
        .record
        .lock()
        .expect("prepared Connector lock must remain usable before removal");
    if state.lifecycle != ConnectorLifecycle::Disposing || state.visible {
        return;
    }
    let _owned = connectors
        .remove(&connector.id)
        .expect("prepared Connector must still be owned at commit");
    let port = state.port.upgrade();
    state.lifecycle = ConnectorLifecycle::Disposed;
    state.phase = "disposed";
    state.visible = false;
    state.requested = false;
    state.subscribed = false;
    state.cleanup_error = None;
    if let Some(port) = port {
        let mut port_guard = port
            .lock()
            .expect("prepared ContentPort lock must remain usable during removal");
        port_guard.connector_ids.remove(&connector.id);
        if port_guard.desired_connector == Some(connector.id) {
            port_guard.desired_connector = None;
        }
        if port_guard.visible_connector == Some(connector.id) {
            port_guard.visible_connector = None;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorError {
    pub code: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorStatus {
    pub phase: String,
    pub requested: bool,
    pub visible: bool,
    pub projected_source_revision: Option<u64>,
    pub error: Option<ContentConnectorError>,
    pub cleanup_pending: bool,
    pub cleanup_error: Option<ContentConnectorError>,
}

/// Host-owned Port/Connector registries. The registry is intentionally
/// separate from Source storage: Sources are environment-owned, while ports
/// and connectors depend on one host's activation and visible frame.
#[derive(Debug)]
pub(crate) struct ContentHostRegistry {
    source_registry: ContentSourceRegistry,
    owner_host: Weak<Mutex<HostInner>>,
    theme: Arc<Theme>,
    theme_revision: u64,
    next_connector_id: u64,
    next_generation: u32,
    ports: HashMap<u64, Arc<Mutex<PortRecord>>>,
    connectors: HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
    in_flight_connectors: HashSet<u64>,
    /// Candidate selection/projection is discarded on frame abort and
    /// promoted only after the backend receipt commits.
    candidate_selections: HashMap<u64, Option<u64>>,
    /// Desired association changes accepted outside a candidate. They are
    /// moved into `candidate_binding_changes` at attempt start and remain
    /// independent from newer changes while a receipt is in flight.
    pending_binding_changes: HashSet<u64>,
    pending_binding_revisions: HashMap<u64, u64>,
    next_binding_revision: u64,
    candidate_binding_changes: HashSet<u64>,
    candidate_binding_revisions: HashMap<u64, u64>,
    /// Active due deadlines for smoothed connectors. Native ticks and wake
    /// queries inspect this structure without scanning inactive registries.
    active_deadlines: HashMap<u64, Instant>,
    /// Connector IDs eligible for deadline synchronization.  This recovery
    /// index is maintained with lifecycle transitions, so an empty deadline
    /// map never requires walking every inactive Connector.
    active_connectors: HashSet<u64>,
    /// Reused worklists for the active/deadline indexes. They contain only
    /// currently eligible Connector IDs and never require a registry scan.
    active_sync_scratch: Vec<u64>,
    due_connector_scratch: Vec<u64>,
    /// Last host-supplied clock. Source/control wakes may arrive without an
    /// explicit `now`; once a host has advanced, those wakes must stay on the
    /// same timeline rather than falling back to wall time.
    authoritative_clock: Option<Instant>,
    /// Connector/Port records touched while preparing the current candidate.
    /// Candidate cleanup and visible promotion consume these sets instead of
    /// scanning unrelated inactive registry entries.
    candidate_touched_connectors: HashSet<u64>,
    candidate_touched_ports: HashSet<u64>,
    /// Source association cleanup that could not acquire its Source lock
    /// after a successful logical frame promotion.  Membership/subscription
    /// stays retained until a later candidate can release it safely.
    pending_source_cleanups: Vec<PreparedSourceCleanup>,
    pending_source_cleanup_ids: HashSet<u64>,
    #[cfg(test)]
    test_poison_source_after_first_cleanup: Option<u64>,
    /// Attempt-local immutable Source captures.  The map is populated lazily
    /// by the first demanded Connector and shared by all later key/history
    /// lookups in that candidate.  Interior mutability keeps the read-only
    /// provider revision queries on the existing seam without making the map a
    /// second Source authority.
    candidate_source_snapshots: RefCell<HashMap<u64, HostContentSourceSnapshot>>,
    candidate_capture_active: bool,
    candidate_commit_prepared: bool,
    history_adapter: HistoryTerminalAdapter,
}

impl ContentHostRegistry {
    pub(crate) fn new(source_registry: ContentSourceRegistry) -> Self {
        Self {
            source_registry,
            owner_host: Weak::new(),
            theme: Arc::new(Theme::new()),
            theme_revision: 0,
            next_connector_id: 0,
            next_generation: 0,
            ports: HashMap::new(),
            connectors: HashMap::new(),
            in_flight_connectors: HashSet::new(),
            candidate_selections: HashMap::new(),
            pending_binding_changes: HashSet::new(),
            pending_binding_revisions: HashMap::new(),
            next_binding_revision: 0,
            candidate_binding_changes: HashSet::new(),
            candidate_binding_revisions: HashMap::new(),
            active_deadlines: HashMap::new(),
            active_connectors: HashSet::new(),
            active_sync_scratch: Vec::new(),
            due_connector_scratch: Vec::new(),
            authoritative_clock: None,
            candidate_touched_connectors: HashSet::new(),
            candidate_touched_ports: HashSet::new(),
            pending_source_cleanups: Vec::new(),
            pending_source_cleanup_ids: HashSet::new(),
            #[cfg(test)]
            test_poison_source_after_first_cleanup: None,
            candidate_source_snapshots: RefCell::new(HashMap::new()),
            candidate_capture_active: false,
            candidate_commit_prepared: false,
            history_adapter: HistoryTerminalAdapter::new(),
        }
    }

    fn touch_connector(&mut self, connector_id: u64) {
        if self.candidate_commit_prepared {
            return;
        }
        self.candidate_touched_connectors.insert(connector_id);
    }

    fn touch_port(&mut self, port_id: u64) {
        if self.candidate_commit_prepared {
            return;
        }
        self.candidate_touched_ports.insert(port_id);
    }

    fn mark_binding_change(&mut self, port_id: u64) {
        self.next_binding_revision = self
            .next_binding_revision
            .checked_add(1)
            .expect("ContentPort binding revision exhausted");
        self.pending_binding_revisions
            .insert(port_id, self.next_binding_revision);
        if self.candidate_commit_prepared {
            self.pending_binding_changes.insert(port_id);
        } else if self.candidate_capture_active {
            self.candidate_binding_changes.insert(port_id);
            self.candidate_binding_revisions
                .insert(port_id, self.next_binding_revision);
        } else {
            self.pending_binding_changes.insert(port_id);
        }
    }

    pub(crate) fn create_port(
        &mut self,
        host: Weak<Mutex<HostInner>>,
        family: ContentFamily,
    ) -> Result<HostContentPort> {
        self.owner_host = host.clone();
        let port_id = NEXT_CONTENT_PORT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| anyhow!("ContentPort identity exhausted"))?;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("ContentPort generation exhausted"))?;
        let record = Arc::new(Mutex::new(PortRecord {
            id: port_id,
            generation: self.next_generation,
            family,
            lifecycle: PortLifecycle::Live,
            host: host.clone(),
            connector_ids: HashSet::new(),
            desired_mounted: false,
            visible_mounted: false,
            desired_connector: None,
            visible_connector: None,
        }));
        self.ports.insert(port_id, Arc::clone(&record));
        Ok(HostContentPort {
            id: port_id,
            generation: self.next_generation,
            family,
            record,
            host,
        })
    }

    fn connect(
        &mut self,
        port: &Arc<Mutex<PortRecord>>,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        let (port_id, port_family, port_live) = {
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (port.id, port.family, port.lifecycle == PortLifecycle::Live)
        };
        if !port_live {
            return Err(anyhow!("PORT_DISPOSED: ContentPort is disposed"));
        }
        if self
            .ports
            .get(&port_id)
            .is_none_or(|candidate| !Arc::ptr_eq(candidate, port))
        {
            return Err(anyhow!(
                "STALE_HANDLE: ContentPort is not owned by this host"
            ));
        }
        if !source.same_environment(&self.source_registry) {
            return Err(anyhow!(
                "WRONG_ENVIRONMENT: Source belongs to a different environment"
            ));
        }
        if !source.is_live() {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if !source.retention_compatible(funnel.kind)? {
            return Err(anyhow!(
                "RETENTION_INCOMPATIBLE: Markdown requires an untruncated Source from its logical start"
            ));
        }
        if funnel.family != port_family || funnel.family != source.family() {
            return Err(anyhow!(
                "CONTENT_FAMILY_MISMATCH: ContentPort and Source/Funnel families differ"
            ));
        }
        source.acquire_connector()?;
        self.next_connector_id = match self.next_connector_id.checked_add(1) {
            Some(id) => id,
            None => {
                source
                    .release_connector()
                    .expect("Connector rollback must release Source membership");
                return Err(anyhow!("Connector identity exhausted"));
            }
        };
        self.next_generation = match self.next_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                source
                    .release_connector()
                    .expect("Connector rollback must release Source membership");
                return Err(anyhow!("Connector generation exhausted"));
            }
        };
        let record = Arc::new(Mutex::new(ConnectorRecord {
            id: self.next_connector_id,
            generation: self.next_generation,
            lifecycle: ConnectorLifecycle::Live,
            port: Arc::downgrade(port),
            source: source.clone(),
            funnel,
            requested: false,
            visible: false,
            subscribed: false,
            membership_released: false,
            cleanup_error: None,
            phase: "idle",
            error: None,
            failed_source_revision: None,
            activation_failure: None,
            projection_cache: VecDeque::new(),
            prepared_paint_cache: VecDeque::new(),
            semantic_cache: VecDeque::new(),
            prefix_proof_cache: VecDeque::new(),
            committed_projection: None,
            candidate_projection: None,
            projected_source_revision: None,
            projection_failure_key: None,
            control_revision: 0,
            delivery_revision: 0,
            candidate_delivery_frontier: StreamOffset::ZERO,
            committed_delivery_frontier: StreamOffset::ZERO,
            execution: None,
        }));
        self.connectors
            .insert(self.next_connector_id, Arc::clone(&record));
        port.lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .connector_ids
            .insert(self.next_connector_id);
        Ok(HostContentConnector {
            id: self.next_connector_id,
            generation: self.next_generation,
            source_id: source.id(),
            record,
            host: Weak::new(),
        })
    }

    pub(crate) fn validate_targets(&self, targets: &[u64]) -> Result<()> {
        let mut seen = HashSet::with_capacity(targets.len());
        for id in targets {
            if !seen.insert(*id) {
                return Err(anyhow!(
                    "DUPLICATE_CONTENT_PORT_ATTACHMENT: ContentPort {id} occurs more than once"
                ));
            }
            let Some(port) = self.ports.get(id) else {
                return Err(anyhow!(
                    "STALE_HANDLE: ContentPort {id} is not owned by this host"
                ));
            };
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            if port.lifecycle != PortLifecycle::Live {
                return Err(anyhow!("PORT_DISPOSED: ContentPort {id} is disposed"));
            }
        }
        Ok(())
    }

    pub(crate) fn set_desired(&mut self, targets: &[u64]) -> Result<()> {
        self.validate_targets(targets)?;
        let target_set = targets.iter().copied().collect::<HashSet<_>>();
        let port_ids = self.ports.keys().copied().collect::<Vec<_>>();
        crate::perf::add(
            crate::perf::Counter::ContentRegistryPortScans,
            port_ids.len() as u64,
        );
        for port_id in port_ids {
            let Some(port) = self.ports.get(&port_id).cloned() else {
                continue;
            };
            let desired_mounted = target_set.contains(&port_id);
            {
                let mut port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                let was_mounted = port_state.desired_mounted;
                port_state.desired_mounted = desired_mounted;
                let desired_connector = port_state.desired_connector;
                let host = port_state.host.clone();
                drop(port_state);
                if was_mounted != desired_mounted {
                    self.mark_binding_change(port_id);
                    self.touch_port(port_id);
                }
                if let Some(connector_id) = desired_connector {
                    self.refresh_requested_phase(
                        connector_id,
                        desired_mounted,
                        !was_mounted && desired_mounted,
                    )?;
                    if desired_mounted {
                        self.ensure_requested_subscription(connector_id, &host)?;
                    } else {
                        self.unsubscribe_requested_if_not_visible(connector_id)?;
                        let visible = self
                            .connectors
                            .get(&connector_id)
                            .and_then(|connector| connector.lock().ok())
                            .is_some_and(|state| state.visible);
                        if !visible {
                            self.active_deadlines.remove(&connector_id);
                            self.active_connectors.remove(&connector_id);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn begin_projection_candidate(&mut self) {
        self.candidate_capture_active = true;
        self.candidate_commit_prepared = false;
        self.candidate_binding_changes.clear();
        self.candidate_binding_changes
            .extend(self.pending_binding_changes.iter().copied());
        self.candidate_binding_revisions.clear();
        for port_id in &self.candidate_binding_changes {
            if let Some(revision) = self.pending_binding_revisions.get(port_id) {
                self.candidate_binding_revisions.insert(*port_id, *revision);
            }
        }
        self.candidate_source_snapshots.borrow_mut().clear();
        self.candidate_touched_connectors.extend(
            self.pending_source_cleanups
                .iter()
                .map(|cleanup| cleanup.connector_id),
        );
        self.clear_candidate_projections();
    }

    /// Advances Connector-local delivery clocks without parsing or touching
    /// Source storage. A progressed smoother invalidates only its derived
    /// projection; the host frame commits the new visible frontier later.
    pub(crate) fn advance(&mut self, now: Instant) -> Result<Vec<ContentDirty>> {
        self.authoritative_clock = Some(now);
        if self.active_deadlines.is_empty() {
            let mut active_candidates = std::mem::take(&mut self.active_sync_scratch);
            active_candidates.clear();
            active_candidates.extend(self.active_connectors.iter().copied());
            for id in active_candidates.drain(..) {
                self.sync_connector_deadline(id, Some(now))?;
            }
            self.active_sync_scratch = active_candidates;
        }
        if self.active_deadlines.is_empty() {
            return Ok(Vec::new());
        }
        let mut due_ids = std::mem::take(&mut self.due_connector_scratch);
        due_ids.clear();
        due_ids.extend(
            self.active_deadlines
                .iter()
                .filter_map(|(&id, &deadline)| (deadline <= now).then_some(id)),
        );
        crate::perf::add(
            crate::perf::Counter::ContentDueConnectors,
            due_ids.len() as u64,
        );
        if due_ids.is_empty() {
            self.due_connector_scratch = due_ids;
            return Ok(Vec::new());
        }
        let mut changed = Vec::new();
        for connector_id in due_ids.iter().copied() {
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
                continue;
            };
            let mut state = match connector.lock() {
                Ok(state) => state,
                Err(_) => {
                    // A poisoned Connector must not remain in the due index:
                    // otherwise every native tick retries the same failed
                    // lock forever without producing a report. Remove its
                    // clock membership before returning the typed scheduler
                    // failure; an explicit readiness/control signal can
                    // re-admit it after the owner repairs the record.
                    self.active_deadlines.remove(&connector_id);
                    self.active_connectors.remove(&connector_id);
                    return Err(anyhow!(
                        "Connector lock is poisoned during delivery advance"
                    ));
                }
            };
            if !state.visible && !state.requested {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
                continue;
            }
            let progressed = state
                .execution
                .as_mut()
                .and_then(|execution| execution.delivery.as_mut())
                .is_some_and(|delivery| delivery.advance(now));
            let next_dl = state
                .execution
                .as_ref()
                .and_then(|execution| execution.delivery.as_ref())
                .and_then(|delivery| {
                    if !delivery.smoother.has_pending_work() {
                        None
                    } else {
                        delivery.smoother.next_wakeup().or(Some(now))
                    }
                });
            if let Some(dl) = next_dl {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, dl);
            } else {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
            }
            if !progressed {
                continue;
            }
            if let Some(delivery) = state.execution.as_ref().and_then(|e| e.delivery.as_ref()) {
                state.candidate_delivery_frontier = delivery.candidate_frontier;
            }
            state.delivery_revision = state
                .delivery_revision
                .checked_add(1)
                .expect("Connector delivery revision exhausted");
            state.candidate_projection = None;
            // Delivery ticks do not clear projection_cache or prepared_paint_cache.
            let port_id = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id;
            changed.push(ContentDirty::new(
                port_id,
                Some(connector_id),
                ContentDirtyReason::DeliveryVisibility,
            ));
        }
        self.due_connector_scratch = due_ids;
        Ok(changed)
    }

    pub(crate) fn next_wakeup(&self) -> Option<Instant> {
        self.active_deadlines.values().copied().min()
    }

    pub(crate) fn sync_connector_deadline(
        &mut self,
        connector_id: u64,
        now: Option<Instant>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle == ConnectorLifecycle::Disposed || (!state.visible && !state.requested) {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_mounted = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned during deadline sync"))?
            .desired_mounted;
        if !state.visible && (!state.requested || !port_mounted) {
            // A requested Connector may remain a cold binding while its Port
            // is unmounted, but it must not index a delivery clock or perform
            // source/smoothing work until the destination is resident again.
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        if state.funnel.smooth_config().is_none() {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        if state.execution.is_none() && state.funnel.smooth_config().is_some() {
            state.execution = Some(ConnectorExecution::new(&state.funnel));
        }
        let snapshot = self.source_snapshot_for(&state.source)?;
        if let Some(execution) = state.execution.as_mut()
            && let Some(delivery) = execution.delivery.as_mut()
        {
            let clock_now = now
                .or(self.authoritative_clock)
                .unwrap_or_else(Instant::now);
            delivery.smoother.ensure_clock(clock_now);
            delivery.accept_input(&snapshot)?;
            delivery.smoother.ensure_clock(clock_now);
            if !delivery.smoother.has_pending_work() {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
            } else if let Some(dl) = delivery.smoother.next_wakeup() {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, dl);
            } else if let Some(now) = now.or(self.authoritative_clock) {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, now);
            } else {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, Instant::now());
            }
        } else {
            self.active_deadlines.remove(&connector_id);
            if state.funnel.smooth_config().is_none() || (!state.visible && !state.requested) {
                self.active_connectors.remove(&connector_id);
            }
        }
        Ok(())
    }

    pub(crate) fn connector_delivery_frontier(&self, id: u64) -> Result<StreamOffset> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.committed_delivery_frontier)
    }

    pub(crate) fn connector_candidate_delivery_frontier(&self, id: u64) -> Result<StreamOffset> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.candidate_delivery_frontier)
    }

    fn connector_projection_key(&self, connector_id: u64, width: u16) -> Result<TextProjectionKey> {
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        let snapshot = self.source_snapshot_for(&state.source)?;
        Ok(TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            width: width.max(1),
            wrap: state.funnel.wrap,
            funnel_kind: state.funnel.kind,
            delivery_revision: state.delivery_revision,
            theme_revision: self.theme_revision,
            needs_finalized_prefix: self.history_adapter.unit_id(port_id).is_some(),
            needs_physical_rows: self.history_adapter.unit_id(port_id).is_some()
                || state.funnel.smooth_config().is_some(),
        })
    }

    fn source_snapshot_for(&self, source: &HostContentSource) -> Result<HostContentSourceSnapshot> {
        if !self.candidate_capture_active {
            return source.snapshot();
        }
        if let Some(snapshot) = self
            .candidate_source_snapshots
            .borrow()
            .get(&source.id())
            .cloned()
        {
            return Ok(snapshot);
        }
        let snapshot = source.snapshot()?;
        self.candidate_source_snapshots
            .borrow_mut()
            .insert(source.id(), snapshot.clone());
        Ok(snapshot)
    }

    fn connector_revision(&self, connector_id: u64, offered_width: u16) -> u64 {
        let Ok(key) = self.connector_projection_key(connector_id, offered_width) else {
            return 0;
        };
        let Some(connector) = self.connectors.get(&connector_id) else {
            return 0;
        };
        let Ok(state) = connector.lock() else {
            return 0;
        };
        let port_mounted = state
            .port
            .upgrade()
            .and_then(|port| port.lock().ok().map(|port| port.desired_mounted))
            .unwrap_or(false);
        let projection_ready = state
            .candidate_projection
            .as_ref()
            .is_some_and(|projection| projection.key == key)
            || state
                .committed_projection
                .as_ref()
                .is_some_and(|projection| projection.key == key)
            || Self::cached_projection(&state, &key).is_some();

        // The layout cache is keyed before ContentProvider::measure can
        // prepare a projection. Include Connector identity and readiness so a
        // cache entry from another Connector, an unmounted occurrence, or an
        // evicted width projection cannot commit a measured node that the
        // painter has no corresponding derived rows for.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        connector_id.hash(&mut hasher);
        key.hash(&mut hasher);
        projection_ready.hash(&mut hasher);
        state.requested.hash(&mut hasher);
        state.visible.hash(&mut hasher);
        state.error.is_some().hash(&mut hasher);
        port_mounted.hash(&mut hasher);
        hasher.finish()
    }

    fn selected_connector_id(&self, port_id: u64) -> Option<u64> {
        if let Some(selection) = self.candidate_selections.get(&port_id) {
            return *selection;
        }
        let (desired, visible, desired_mounted) = {
            let port = self.ports.get(&port_id)?.lock().ok()?;
            (
                port.desired_connector,
                port.visible_connector,
                port.desired_mounted,
            )
        };
        if desired_mounted {
            if let Some(desired) = desired {
                let failed = self
                    .connectors
                    .get(&desired)
                    .and_then(|connector| connector.lock().ok())
                    .is_some_and(|state| state.error.is_some() && !state.visible);
                if !failed {
                    return Some(desired);
                }
            }
            visible
        } else {
            None
        }
    }

    fn cached_projection(
        state: &ConnectorRecord,
        key: &TextProjectionKey,
    ) -> Option<Arc<HostContentProjection>> {
        state
            .projection_cache
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, projection)| Arc::clone(projection))
    }

    fn prepare_connector_projection(
        &mut self,
        connector_id: u64,
        offered_width: u16,
    ) -> Result<ContentMeasurement> {
        self.touch_connector(connector_id);
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let (source, funnel, delivery_revision, needs_finalized_prefix, needs_physical_rows) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live || (!state.requested && !state.visible) {
                return Ok(ContentMeasurement::default());
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let port_id = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id;
            (
                state.source.clone(),
                state.funnel,
                state.delivery_revision,
                self.history_adapter.unit_id(port_id).is_some(),
                self.history_adapter.unit_id(port_id).is_some()
                    || state.funnel.smooth_config().is_some(),
            )
        };
        crate::perf::inc(crate::perf::Counter::SemanticPreparations);
        let snapshot = self.source_snapshot_for(&source)?;
        if funnel.kind == TextFunnelKind::Markdown && snapshot.source_base != 0 {
            return Err(anyhow::Error::new(ContentProjectionFailure {
                kind: ContentProjectionFailureKind::RetentionIncompatible,
                diagnostic: "RETENTION_INCOMPATIBLE: Markdown requires an untruncated Source from its logical start"
                    .to_owned(),
            }));
        }
        let key = TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            width: offered_width.max(1),
            wrap: funnel.wrap,
            funnel_kind: funnel.kind,
            delivery_revision,
            theme_revision: self.theme_revision,
            needs_finalized_prefix,
            needs_physical_rows,
        };
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.error.is_some() && state.projection_failure_key == Some(key) {
                return Err(anyhow!(
                    "PROJECTION_RETRY_BLOCKED: Connector projection is already failed for this input"
                ));
            }
            if let Some(projection) = Self::cached_projection(&state, &key) {
                state.candidate_projection = Some(Arc::clone(&projection));
                // A successful cache hit is still a successful preparation
                // for this input. Clear an older failure recorded for a
                // different width/revision so status does not remain stale
                // after the Connector has recovered without recompiling.
                state.error = None;
                state.failed_source_revision = None;
                state.projection_failure_key = None;
                let measurement = projection.measurement(connector_id);
                drop(state);
                self.sync_connector_deadline(connector_id, None)?;
                return Ok(measurement);
            }
        }

        // The snapshot owns immutable chunks; the Source lock is not held
        // while width-dependent projection allocates/compiles derived rows.
        // Execution state is Connector-local. Take it, semantic cache, and
        // prepared paint cache out while projecting so a parser/smoother can
        // mutate without holding the Connector mutex.
        let (mut execution, mut semantic_cache, mut prefix_proof_cache, mut prepared_paint_cache) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (
                state
                    .execution
                    .take()
                    .unwrap_or_else(|| ConnectorExecution::new(&funnel)),
                std::mem::take(&mut state.semantic_cache),
                std::mem::take(&mut state.prefix_proof_cache),
                std::mem::take(&mut state.prepared_paint_cache),
            )
        };
        let projection = match project_text_snapshot(
            &snapshot,
            funnel,
            offered_width,
            needs_finalized_prefix,
            &self.theme,
            self.theme_revision,
            &mut execution,
            delivery_revision,
            &mut semantic_cache,
            &mut prefix_proof_cache,
            &mut prepared_paint_cache,
        ) {
            Ok(projection) => Arc::new(projection),
            Err(error) => {
                // A failed candidate must not retain partially advanced
                // delivery/parser state. The next eligible revision or
                // explicit retry starts from a clean Connector execution.
                // The semantic cache holds only immutable completed
                // products, so it is always safe to restore.
                if let Ok(mut state) = connector.lock() {
                    state.semantic_cache = semantic_cache;
                    state.prefix_proof_cache = prefix_proof_cache;
                    state.prepared_paint_cache = prepared_paint_cache;
                }
                return Err(error);
            }
        };
        let measurement = projection.measurement(connector_id);
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if let Some(delivery) = execution.delivery.as_ref() {
            state.candidate_delivery_frontier = delivery.candidate_frontier;
        }
        state.execution = Some(execution);
        state.semantic_cache = semantic_cache;
        state.prefix_proof_cache = prefix_proof_cache;
        state.prepared_paint_cache = prepared_paint_cache;
        state
            .projection_cache
            .retain(|(candidate, _)| candidate != &key);
        state
            .projection_cache
            .push_front((key, Arc::clone(&projection)));
        while state.projection_cache.len() > 4 {
            state.projection_cache.pop_back();
        }
        state.candidate_projection = Some(Arc::clone(&projection));
        state.error = None;
        state.failed_source_revision = None;
        state.projection_failure_key = None;
        drop(state);
        self.sync_connector_deadline(connector_id, None)?;
        Ok(measurement)
    }

    fn projection_measurement(
        &self,
        connector_id: u64,
        offered_width: u16,
    ) -> Option<ContentMeasurement> {
        let key = self
            .connector_projection_key(connector_id, offered_width)
            .ok()?;
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        if let Some(projection) = connector
            .candidate_projection
            .as_ref()
            .filter(|projection| projection.key == key)
        {
            return Some(projection.measurement(connector_id));
        }
        if let Some(projection) = Self::cached_projection(&connector, &key) {
            return Some(projection.measurement(connector_id));
        }
        connector
            .committed_projection
            .as_ref()
            .filter(|projection| projection.key == key)
            .map(|projection| projection.measurement(connector_id))
    }

    fn projection_failure_is_recorded(&self, connector_id: u64, key: TextProjectionKey) -> bool {
        self.connectors
            .get(&connector_id)
            .and_then(|connector| connector.lock().ok())
            .is_some_and(|state| state.error.is_some() && state.projection_failure_key == Some(key))
    }

    fn record_projection_failure(
        &mut self,
        connector_id: u64,
        key: TextProjectionKey,
        error: &anyhow::Error,
    ) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let mut state = connector
            .lock()
            .expect("Connector lock must remain usable while recording a failure");
        let code = error
            .downcast_ref::<ContentProjectionFailure>()
            .map_or(ContentProjectionFailureKind::Projection.code(), |failure| {
                failure.kind.code()
            });
        state.error = Some(ContentConnectorError {
            code: code.to_owned(),
            diagnostic: error.to_string(),
        });
        state.failed_source_revision = Some(key.source_revision);
        state.projection_failure_key = Some(key);
        state.phase = if state.visible { "active" } else { "failed" };
    }

    fn record_connector_operating_failure(
        &mut self,
        connector_id: u64,
        error: anyhow::Error,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: Connector {connector_id} disappeared while recording an operating failure"
            ));
        };
        let mut state = connector.lock().map_err(|_| {
            anyhow!("Connector lock is poisoned while recording an operating failure")
        })?;
        state.error = Some(ContentConnectorError {
            code: "CONTENT_OPERATING_FAILED".to_owned(),
            diagnostic: error.to_string(),
        });
        state.failed_source_revision = None;
        state.projection_failure_key = None;
        state.phase = if state.visible { "active" } else { "failed" };
        Ok(())
    }

    fn refine_fit_measurement(
        &mut self,
        connector_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::WidthRule,
        measurement: ContentMeasurement,
    ) -> ContentMeasurement {
        if width_rule != crate::presentation::WidthRule::Fit
            || measurement.intrinsic_size.width == 0
            || measurement.intrinsic_size.width >= offered_width.max(1)
        {
            return measurement;
        }
        // The first probe already compiled the semantic product at the
        // offered width. When its natural width is smaller, no line wrapped
        // in that product, so re-keying the prepared ticket is equivalent to
        // recompiling at the natural width. Keep one canonical paint product
        // and avoid a second full semantic/layout/paint compilation for the
        // common width-fit ContentHost path.
        if let Some(measurement) = self.rekey_fit_projection(
            connector_id,
            offered_width,
            measurement.intrinsic_size.width,
        ) {
            return measurement;
        }
        match self.prepare_connector_projection(connector_id, measurement.intrinsic_size.width) {
            Ok(measurement) => measurement,
            Err(error) => {
                let _ = self.record_connector_operating_failure(connector_id, error);
                measurement
            }
        }
    }

    fn rekey_fit_projection(
        &mut self,
        connector_id: u64,
        offered_width: u16,
        intrinsic_width: u16,
    ) -> Option<ContentMeasurement> {
        let connector = self.connectors.get(&connector_id).cloned()?;
        let mut state = connector.lock().ok()?;
        let projection = state.candidate_projection.as_ref()?.clone();
        if projection.key.width != offered_width.max(1)
            || projection.intrinsic_size.width != intrinsic_width
            || intrinsic_width == 0
        {
            return None;
        }
        let mut key = projection.key;
        key.width = intrinsic_width.max(1);
        if key == projection.key {
            return Some(projection.measurement(connector_id));
        }
        let mut rekeyed = (*projection).clone();
        rekeyed.identity = next_content_projection_id();
        rekeyed.key = key;
        let rekeyed = Arc::new(rekeyed);
        state
            .projection_cache
            .retain(|(candidate, _)| candidate != &key);
        state
            .projection_cache
            .push_front((key, Arc::clone(&rekeyed)));
        while state.projection_cache.len() > 4 {
            state.projection_cache.pop_back();
        }
        state.candidate_projection = Some(Arc::clone(&rekeyed));
        Some(rekeyed.measurement(connector_id))
    }

    fn adjust_history_measurement(
        &self,
        port_id: u64,
        mut measurement: ContentMeasurement,
    ) -> ContentMeasurement {
        let committed_rows = self.history_adapter.committed_content_rows(port_id);
        if committed_rows == 0 {
            return measurement;
        }
        measurement.intrinsic_size.height = measurement
            .intrinsic_size
            .height
            .saturating_sub(u16::try_from(committed_rows).unwrap_or(u16::MAX));
        measurement
    }

    fn measure_content(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::WidthRule,
    ) -> ContentMeasurement {
        self.touch_port(port_id);
        let Some(port) = self.ports.get(&port_id).cloned() else {
            return ContentMeasurement::default();
        };
        let (desired, visible, desired_mounted) = {
            let Ok(port) = port.lock() else {
                return ContentMeasurement::default();
            };
            (
                port.desired_connector,
                port.visible_connector,
                port.desired_mounted,
            )
        };
        let measurement = if !desired_mounted {
            self.candidate_selections.insert(port_id, None);
            ContentMeasurement::default()
        } else {
            let Some(connector_id) = desired else {
                self.candidate_selections.insert(port_id, None);
                return ContentMeasurement::default();
            };

            // Keep the native/unit failure fixture on the same candidate-rollback
            // boundary as real projection failures.
            let activation_failed =
                match self.prepare_activation_candidate(connector_id, offered_width) {
                    Ok(failed) => failed,
                    Err(error) => {
                        if self
                            .record_connector_operating_failure(connector_id, error)
                            .is_err()
                        {
                            return ContentMeasurement::default();
                        }
                        false
                    }
                };
            if activation_failed {
                let rollback = visible.and_then(|id| {
                    self.prepare_connector_projection(id, offered_width)
                        .ok()
                        .map(|measurement| {
                            self.refine_fit_measurement(id, offered_width, width_rule, measurement)
                        })
                });
                self.candidate_selections.insert(port_id, visible);
                rollback.unwrap_or_default()
            } else {
                match self.prepare_connector_projection(connector_id, offered_width) {
                    Ok(measurement) => {
                        self.candidate_selections
                            .insert(port_id, Some(connector_id));
                        self.refine_fit_measurement(
                            connector_id,
                            offered_width,
                            width_rule,
                            measurement,
                        )
                    }
                    Err(error) => {
                        if let Ok(key) = self.connector_projection_key(connector_id, offered_width)
                            && !self.projection_failure_is_recorded(connector_id, key)
                        {
                            self.record_projection_failure(connector_id, key, &error);
                        }
                        let rollback = visible.and_then(|id| {
                            self.prepare_connector_projection(id, offered_width)
                                .ok()
                                .or_else(|| self.projection_measurement(id, offered_width))
                                .map(|measurement| {
                                    self.refine_fit_measurement(
                                        id,
                                        offered_width,
                                        width_rule,
                                        measurement,
                                    )
                                })
                        });
                        self.candidate_selections.insert(port_id, visible);
                        rollback.unwrap_or_default()
                    }
                }
            }
        };
        self.adjust_history_measurement(port_id, measurement)
    }

    fn paint_window_direct(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (u16, u16),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        // The ticket is the product selected during preparation.  Never
        // substitute the newest same-width projection: a Source append,
        // delivery tick, Connector switch, or theme change may have created
        // another candidate while this frame is still being painted.
        let Some(projection) = self.projection_for_ticket(ticket) else {
            return;
        };
        if !projection.physically_complete {
            target.physically_complete = false;
        }
        let committed_rows = self.history_adapter.committed_content_rows(ticket.port_id);
        let max_visible = projection.visible_row_count.saturating_sub(committed_rows);
        let retained_visible_len = projection.rows.as_ref().map_or(max_visible, |rows| {
            rows.len().saturating_sub(committed_rows).min(max_visible)
        });

        let start_offset = match usize::try_from(window.first_row) {
            Ok(v) => v,
            Err(_) => return,
        };
        if start_offset >= retained_visible_len || window.row_count == 0 {
            return;
        }
        let row_count = usize::try_from(window.row_count).unwrap_or(usize::MAX);
        let end_offset = (start_offset.saturating_add(row_count)).min(retained_visible_len);
        let generated_rows;
        let window_slice: &[PhysicalRow] = if let Some(rows) = projection.rows.as_ref() {
            let available_rows = if committed_rows >= rows.len() {
                &[][..]
            } else {
                &rows[committed_rows..]
            };
            &available_rows[start_offset..end_offset]
        } else {
            let first_row = committed_rows.saturating_add(start_offset);
            let source_row_count = end_offset.saturating_sub(start_offset);
            let compiler = crate::presentation::layout::ViewCompiler::new(&projection.theme);
            let layout = projection
                .layout
                .as_ref()
                .expect("deferred projection must retain its prepared layout");
            let text_geometry = projection
                .text_geometry
                .as_ref()
                .expect("deferred projection must retain text geometry");
            let Ok(mut text_geometry) = text_geometry.lock() else {
                target.physically_complete = false;
                return;
            };
            let (rows, physically_complete) = crate::presentation::paint::ViewPainter::default()
                .paint_tree_row_range_with_text_cache(
                    &compiler,
                    layout,
                    u16::try_from(first_row).unwrap_or(u16::MAX),
                    u16::try_from(source_row_count).unwrap_or(u16::MAX),
                    &mut text_geometry,
                );
            if !physically_complete {
                target.physically_complete = false;
            }
            generated_rows = rows;
            generated_rows.as_slice()
        };

        let clip_left = i32::from(clip.x);
        let clip_top = i32::from(clip.y);
        let clip_right = clip_left.saturating_add(i32::from(clip.width));
        let clip_bottom = clip_top.saturating_add(i32::from(clip.height));
        let target_width = i32::from(target.width());
        let target_height = i32::from(target.height());

        for (i, row) in window_slice.iter().enumerate() {
            let target_y = i32::from(target_origin.1).saturating_add(i as i32);
            if target_y < clip_top
                || target_y >= clip_bottom
                || target_y < 0
                || target_y >= target_height
            {
                continue;
            }

            let projection_row_idx = committed_rows + start_offset + i;
            let max_col = if let Some((cut_row, cut_col)) = projection.cut
                && projection_row_idx == usize::from(cut_row)
            {
                usize::from(cut_col)
            } else if let Some((cut_row, _)) = projection.cut
                && projection_row_idx > usize::from(cut_row)
            {
                0
            } else {
                usize::from(ticket.offered_width.max(1))
            };

            if max_col == 0 {
                continue;
            }

            let dest_origin_x = i32::from(target_origin.0);
            let src_cells = row.cells();
            for glyph in row.glyphs() {
                if !glyph.leader.painted {
                    continue;
                }
                if glyph.start >= max_col {
                    break;
                }
                let dest_start = dest_origin_x.saturating_add(glyph.start as i32);
                let dest_end = dest_start.saturating_add(glyph.width as i32);
                if dest_start < clip_left
                    || dest_end > clip_right
                    || dest_start < 0
                    || dest_end > target_width
                {
                    continue;
                }

                let dest_row = target.row_cells_mut(target_y as u16);
                crate::perf::add(
                    crate::perf::Counter::SurfaceCellsComposited,
                    glyph.width as u64,
                );
                crate::physical::write_glyph_span(
                    dest_row,
                    dest_start as usize,
                    src_cells,
                    glyph.start,
                    glyph.width,
                );

                for col in (dest_start as usize)..(dest_end as usize) {
                    let cell = &mut dest_row[col];
                    if cell.painted {
                        if cell.style.foreground.is_none() {
                            cell.style.foreground = style.foreground;
                        }
                        if cell.style.background.is_none() {
                            cell.style.background = style.background;
                        }
                        cell.style.bold |= style.bold;
                        cell.style.dim |= style.dim;
                        cell.style.italic |= style.italic;
                        cell.style.underline |= style.underline;
                        cell.style.reversed |= style.reversed;
                        cell.style.strikethrough |= style.strikethrough;
                    }
                }
            }
            debug_assert!(
                crate::physical::validate_cells(target.row_cells(target_y as u16)).is_ok()
            );
        }
    }

    fn projection_for_ticket(
        &self,
        ticket: PreparedProjectionTicket,
    ) -> Option<Arc<HostContentProjection>> {
        let connector_id = ticket.connector_id?;
        if ticket.projection_identity == 0 {
            return None;
        }
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let matches = |projection: &Arc<HostContentProjection>| {
            projection.identity == ticket.projection_identity
                && projection.key.width == ticket.offered_width.max(1)
                && projection.key.revision() == ticket.projection_revision
        };
        if connector.candidate_projection.as_ref().is_some_and(matches) {
            return connector.candidate_projection.as_ref().cloned();
        }
        if connector.committed_projection.as_ref().is_some_and(matches) {
            return connector.committed_projection.as_ref().cloned();
        }
        connector
            .projection_cache
            .iter()
            .find(|(_, projection)| matches(projection))
            .map(|(_, projection)| Arc::clone(projection))
    }

    fn connector_projection(
        &self,
        connector_id: u64,
        offered_width: u16,
    ) -> Option<Arc<HostContentProjection>> {
        let key = self
            .connector_projection_key(connector_id, offered_width)
            .ok()?;
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        if let Some(projection) = connector
            .candidate_projection
            .as_ref()
            .filter(|projection| projection.key == key)
        {
            // The candidate owns the immutable Source snapshot captured for
            // this frame. A concurrent Source revision is left for the next
            // host epoch rather than mixing snapshots during paint.
            return Some(Arc::clone(projection));
        }
        if let Some(projection) = Self::cached_projection(&connector, &key) {
            return Some(projection);
        }
        connector
            .committed_projection
            .as_ref()
            .filter(|projection| projection.key == key)
            .cloned()
    }

    fn prepare_connector_commit(
        &self,
        connector_id: u64,
        visible: bool,
    ) -> Result<PreparedContentConnector> {
        let record = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: Connector {connector_id} disappeared during candidate preparation"
                )
            })?;
        let (
            source,
            source_id,
            generation,
            requested,
            subscribed,
            control_revision,
            deadline,
            candidate_projection,
            delivery_frontier,
            delivery_input,
            delivery_revision,
        ) = {
            let state = record
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned during candidate preparation"))?;
            if state.lifecycle == ConnectorLifecycle::Disposed {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: Connector {connector_id} was disposed during candidate preparation"
                ));
            }
            let deadline = state
                .execution
                .as_ref()
                .and_then(|execution| execution.delivery.as_ref())
                .and_then(|delivery| {
                    delivery
                        .smoother
                        .has_pending_work()
                        .then(|| delivery.smoother.next_wakeup())
                        .flatten()
                });
            let delivery_input = state.execution.as_ref().and_then(|execution| {
                execution.delivery.as_ref().map(|delivery| {
                    (
                        delivery.indexed_generation,
                        delivery.indexed_revision,
                        delivery.indexed_sealed,
                    )
                })
            });
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let _port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (
                state.source.clone(),
                state.source.id(),
                state.generation,
                state.requested,
                state.subscribed,
                state.control_revision,
                deadline,
                state.candidate_projection.clone(),
                state.candidate_delivery_frontier,
                delivery_input,
                state.delivery_revision,
            )
        };
        Ok(PreparedContentConnector {
            id: connector_id,
            record,
            source,
            source_id,
            generation,
            requested,
            subscribed,
            control_revision,
            deadline,
            visible,
            candidate_projection,
            delivery_frontier,
            delivery_input,
            delivery_revision,
        })
    }

    /// Captures only the changed Port/Connector records needed to promote one
    /// prepared content candidate. This is the only place where the live
    /// Port/Connector maps are resolved for the commit. Receipt-time code
    /// consumes the resulting Arc-backed plan and performs no handle lookup,
    /// validity rediscovery, or temporary registry-wide scan.
    pub(crate) fn prepare_content_commit(&mut self) -> Result<PreparedContentCommit> {
        let mut changed_port_ids = self
            .candidate_binding_changes
            .iter()
            .copied()
            .collect::<Vec<_>>();
        changed_port_ids.sort_unstable();
        let changed_port_count = changed_port_ids.len();
        let mut ports = Vec::with_capacity(changed_port_count);
        let mut changed_ports = Vec::with_capacity(changed_port_count);
        for port_id in changed_port_ids {
            let port = self.ports.get(&port_id).cloned().ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} disappeared during candidate preparation"
                )
            })?;
            let (mounted, desired_connector) = {
                let state = port.lock().map_err(|_| {
                    anyhow!("ContentPort lock is poisoned during candidate preparation")
                })?;
                (state.desired_mounted, state.desired_connector)
            };
            let next_connector = if mounted {
                self.candidate_selections
                    .get(&port_id)
                    .copied()
                    .unwrap_or(desired_connector)
            } else {
                None
            };
            changed_ports.push((port_id, mounted, next_connector));
        }
        let mut connector_ids = HashSet::with_capacity(changed_port_count * 2);
        let mut add_port = |port_id: u64, mounted: bool, next_id: Option<u64>| -> Result<()> {
            let record = self.ports.get(&port_id).cloned().ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} disappeared during candidate preparation"
                )
            })?;
            let (old_id, port_live) = {
                let state = record.lock().map_err(|_| {
                    anyhow!("ContentPort lock is poisoned during candidate preparation")
                })?;
                (
                    state.visible_connector,
                    state.lifecycle == PortLifecycle::Live,
                )
            };
            if !port_live {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} became disposed during candidate preparation"
                ));
            }
            if !mounted && next_id.is_some() {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: unmounted ContentPort {port_id} has a visible candidate"
                ));
            }
            let old_control_revision = if let Some(id) = old_id {
                let connector = self.connectors.get(&id).cloned().ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: visible Connector {id} disappeared during candidate preparation"
                        )
                    })?;
                let control_revision = connector
                    .lock()
                    .map_err(|_| {
                        anyhow!("Connector lock is poisoned during candidate preparation")
                    })?
                    .control_revision;
                Some(control_revision)
            } else {
                None
            };
            if let Some(id) = old_id {
                connector_ids.insert(id);
            }
            if let Some(id) = next_id {
                let connector = self.connectors.get(&id).cloned().ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: candidate Connector {id} disappeared during preparation"
                        )
                    })?;
                let belongs = connector
                    .lock()
                    .map_err(|_| {
                        anyhow!("Connector lock is poisoned during candidate preparation")
                    })?
                    .port
                    .upgrade()
                    .is_some_and(|owner| Arc::ptr_eq(&owner, &record));
                if !belongs {
                    return Err(anyhow!(
                        "INTERNAL_INVARIANT: Connector {id} does not belong to ContentPort {port_id}"
                    ));
                }
                if Some(id) != old_id
                    && !self.connector_is_candidate_ready(id)?
                    && !self.in_flight_connectors.contains(&id)
                {
                    return Err(anyhow!(
                        "INTERNAL_INVARIANT: Connector {id} was not prepared for visible commit"
                    ));
                }
                connector_ids.insert(id);
            }
            self.candidate_touched_ports.insert(port_id);
            ports.push(PreparedContentPort {
                id: port_id,
                record,
                mounted,
                old_connector_id: old_id,
                old_connector_index: None,
                old_control_revision,
                next_connector_id: next_id,
                next_connector_index: None,
                retry_selection: false,
            });
            Ok(())
        };

        for (port_id, mounted, next_connector) in changed_ports {
            add_port(port_id, mounted, next_connector)?;
        }

        // A content-only candidate has no binding change, but its touched
        // Connector still owns the prepared projection/frontier that must be
        // promoted by the receipt rather than discarded by cleanup.
        connector_ids.extend(self.candidate_touched_connectors.iter().copied());
        let mut connector_ids = connector_ids.into_iter().collect::<Vec<_>>();
        connector_ids.sort_unstable();
        let mut connectors = Vec::with_capacity(connector_ids.len());
        let visible_connector_ids = ports
            .iter()
            .filter(|port| port.mounted)
            .filter_map(|port| port.next_connector_id)
            .collect::<HashSet<_>>();
        let hidden_connector_ids = ports
            .iter()
            .filter_map(|port| {
                port.old_connector_id
                    .filter(|id| Some(*id) != port.next_connector_id)
            })
            .collect::<HashSet<_>>();
        for connector_id in connector_ids {
            self.candidate_touched_connectors.insert(connector_id);
            let visible = if visible_connector_ids.contains(&connector_id) {
                true
            } else if hidden_connector_ids.contains(&connector_id) {
                false
            } else {
                self.connectors
                    .get(&connector_id)
                    .ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: touched Connector {connector_id} disappeared during candidate preparation"
                        )
                    })?
                    .lock()
                    .map_err(|_| {
                        anyhow!(
                            "Connector lock is poisoned during candidate preparation"
                        )
                    })?
                    .visible
            };
            connectors.push(self.prepare_connector_commit(connector_id, visible)?);
        }
        // Keep Connector records in the same Source-ID order used by the
        // merged lock plan. Port records retain direct prepared indexes, so
        // receipt-time association never searches this vector.
        connectors.sort_unstable_by_key(|connector| (connector.source_id, connector.id));
        // Ensure every prepared visible deadline has an owned slot before the
        // backend receipt. Receipt-time promotion only updates/removes these
        // existing entries; newer control operations may consume capacity
        // without making the old candidate depend on spare shared space.
        for connector in &connectors {
            if connector.visible
                && let Some(deadline) = connector.deadline
            {
                self.active_connectors.insert(connector.id);
                self.active_deadlines.insert(connector.id, deadline);
            }
        }
        let mut sources = Vec::with_capacity(connectors.len());
        for connector in &connectors {
            sources.push(PreparedContentSource {
                id: connector.source_id,
                source: connector.source.clone(),
            });
        }
        sources.sort_unstable_by_key(|source| source.id);
        sources.dedup_by_key(|source| source.id);
        // Preflight the merged Source lock plan in global Source-ID order.
        // Receipt-time code uses the same candidate-owned order and never
        // deduplicates Source Arcs with a linear pointer scan.
        for source in &sources {
            let _guard = source.source.record.lock().map_err(|_| {
                anyhow!("content Source lock is poisoned during candidate preparation")
            })?;
        }
        let mut source_cleanups = Vec::with_capacity(connectors.len());
        for connector in &connectors {
            // Keep one candidate-owned cleanup slot for every Connector that
            // this candidate hides. A newer disposal can arrive while the
            // receipt is pending even when the Connector was already
            // unsubscribed at capture; the commit then recomputes membership
            // release from the current lifecycle without rediscovering the
            // record through a registry scan.
            if !connector.visible {
                source_cleanups.push(PreparedSourceCleanup {
                    source: connector.source.clone(),
                    source_id: connector.source_id,
                    record: connector.record.clone(),
                    connector_id: connector.id,
                    connector_generation: connector.generation,
                    unsubscribe: connector.subscribed,
                    error: Arc::new(ContentConnectorError {
                        code: "SOURCE_CLEANUP_PENDING".to_owned(),
                        diagnostic: format!(
                            "Source {} cleanup for Connector {} was deferred",
                            connector.source_id, connector.id
                        ),
                    }),
                });
            }
        }
        source_cleanups.sort_unstable_by_key(|cleanup| (cleanup.source_id, cleanup.connector_id));
        // A Source lock can become poisoned after preparation but before the
        // logical frame receipt commits.  Reserve the deferred-cleanup
        // capacity now so conservative post-promotion retention cannot make
        // receipt completion fallible through vector/table growth.
        self.pending_source_cleanups.reserve(source_cleanups.len());
        self.pending_source_cleanup_ids
            .reserve(source_cleanups.len());

        let connector_indexes = connectors
            .iter()
            .enumerate()
            .map(|(index, connector)| (connector.id, index))
            .collect::<HashMap<_, _>>();
        for port in &mut ports {
            port.old_connector_index = port.old_connector_id.map(|id| {
                *connector_indexes
                    .get(&id)
                    .expect("prepared old Connector must be indexed")
            });
            port.next_connector_index = port.next_connector_id.map(|id| {
                *connector_indexes
                    .get(&id)
                    .expect("prepared next Connector must be indexed")
            });
            let desired = port
                .record
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned during candidate preparation"))?
                .desired_connector;
            let active_desired = desired.filter(|desired_id| {
                connector_indexes
                    .get(desired_id)
                    .is_some_and(|index| connectors[*index].requested || connectors[*index].visible)
            });
            port.retry_selection = port.next_connector_id != active_desired;
        }
        let mut binding_changes = Vec::with_capacity(self.candidate_binding_changes.len());
        for (port_index, port) in ports.iter().enumerate() {
            let revision = self
                .candidate_binding_revisions
                .get(&port.id)
                .copied()
                .unwrap_or(0);
            binding_changes.push(PreparedContentBindingChange {
                port_index,
                revision,
            });
        }
        crate::perf::add(
            crate::perf::Counter::ContentCandidateRecordsPrepared,
            (ports.len() + connectors.len() + sources.len()) as u64,
        );
        self.candidate_commit_prepared = true;
        Ok(PreparedContentCommit {
            ports,
            connectors,
            sources,
            source_cleanups,
            binding_changes,
        })
    }

    /// Applies the native/unit-only operational failure hook at candidate
    /// preparation time. This keeps activation request state truthful and
    /// exercises the same old-visible rollback boundary that real projection
    /// errors will use in PERF-13-F.
    fn prepare_activation_candidate(
        &mut self,
        connector_id: u64,
        offered_width: u16,
    ) -> Result<bool> {
        self.touch_connector(connector_id);
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation candidate {connector_id} disappeared"
            ));
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            if state.activation_failure.is_none() {
                return Ok(false);
            }
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation failure targeted a non-candidate Connector {connector_id}"
            ));
        }
        if state.error.is_some() && state.projection_failure_key.is_some() {
            // One candidate may measure the same Connector at several widths
            // (for example an unconstrained probe followed by the committed
            // width). Preserve the failed attempt across those probes instead
            // of consuming a synthetic failure once and accidentally selecting
            // the Connector on a later width pass in the same frame.
            return Ok(true);
        }
        if state.activation_failure.is_none() {
            return Ok(false);
        }
        // Capture the revision and input key at the start of the failed
        // attempt. A concurrent Source mutation may commit while the host is
        // still preparing this candidate; recording the pre-attempt revision
        // and exact width prevents a second layout measurement in this same
        // frame from retrying the identical failed input.
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        let snapshot = self.source_snapshot_for(&state.source)?;
        let key = TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            width: offered_width.max(1),
            wrap: state.funnel.wrap,
            funnel_kind: state.funnel.kind,
            delivery_revision: state.delivery_revision,
            theme_revision: self.theme_revision,
            needs_finalized_prefix: self.history_adapter.unit_id(port_id).is_some(),
            needs_physical_rows: self.history_adapter.unit_id(port_id).is_some()
                || state.funnel.smooth_config().is_some(),
        };
        let attempted_source_revision = snapshot.revision;
        let diagnostic = state
            .activation_failure
            .take()
            .expect("activation failure was checked above");
        state.error = Some(ContentConnectorError {
            code: "PROJECTION_FAILED".to_owned(),
            diagnostic,
        });
        state.failed_source_revision = Some(attempted_source_revision);
        state.projection_failure_key = Some(key);
        state.phase = "failed";
        Ok(true)
    }

    /// Retains the Connector IDs referenced by a candidate frame. Control
    /// mutations accepted while a backend receipt is in flight must not
    /// destroy or detach an identity that the captured candidate still uses.
    pub(crate) fn begin_prepared_candidate(&mut self, plan: &PreparedContentCommit) {
        self.in_flight_connectors.clear();
        for connector in &plan.connectors {
            self.in_flight_connectors.insert(connector.id);
        }
    }

    /// Releases the candidate lease after its logical frame commit. A
    /// disposing Connector selected by that frame remains visible/disposing
    /// until the following removal frame, as required by transactional
    /// disposal semantics.
    pub(crate) fn end_candidate(&mut self) {
        self.in_flight_connectors.clear();
        // Candidate projection cleanup is staged in PreparedContentCommit and
        // runs before this receipt-time lease release.  Keeping this method
        // to constant-time ownership flags avoids a post-receipt registry scan
        // and cannot consume newer desired operations.
        self.candidate_selections.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.candidate_source_snapshots.borrow_mut().clear();
    }

    /// Aborts a candidate without changing visible bindings. Deferred control
    /// mutations can now finalize identities that were never made visible and
    /// inactive requested Connectors can lose provisional subscriptions.
    pub(crate) fn abort_candidate(&mut self) {
        self.clear_candidate_projections();
        let connector_ids = self.in_flight_connectors.drain().collect::<Vec<_>>();
        for connector_id in connector_ids.iter().copied() {
            self.cleanup_aborted_candidate(connector_id);
        }
        let touched = self
            .candidate_touched_connectors
            .iter()
            .copied()
            .collect::<Vec<_>>();
        self.finalize_disposed_connectors(&touched);
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.candidate_source_snapshots.borrow_mut().clear();
    }

    fn cleanup_aborted_candidate(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let state = connector
            .lock()
            .expect("Connector lock must remain usable while aborting a candidate");
        let source = state.source.clone();
        let generation = state.generation;
        let visible = state.visible;
        let requested = state.requested;
        let port_mounted = state.port.upgrade().is_some_and(|port| {
            port.lock()
                .expect("ContentPort lock must remain usable while aborting a candidate")
                .desired_mounted
        });
        drop(state);
        if !visible && (!requested || !port_mounted) {
            self.unsubscribe_connector(&source, connector_id, generation)
                .expect("aborted candidate Source unsubscribe must be valid");
        }
    }

    #[cfg(test)]
    fn promote_candidate_projection(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        if let Ok(mut state) = connector.lock()
            && let Some(projection) = state.candidate_projection.take()
        {
            state.projected_source_revision = Some(projection.key.source_revision);
            state.committed_delivery_frontier = state.candidate_delivery_frontier;
            state.committed_projection = Some(projection);
        }
    }

    fn clear_candidate_projections(&mut self) {
        self.candidate_selections.clear();
        for connector_id in self.candidate_touched_connectors.iter() {
            let Some(connector) = self.connectors.get(connector_id).cloned() else {
                continue;
            };
            let mut state = connector
                .lock()
                .expect("Connector lock must remain usable during candidate cleanup");
            state.candidate_projection = None;
            state.candidate_delivery_frontier = state.committed_delivery_frontier;
            if !state.visible {
                state.committed_projection = None;
                state.projection_cache.clear();
                state.prepared_paint_cache.clear();
                state.semantic_cache.clear();
                state.prefix_proof_cache.clear();
                state.projected_source_revision = None;
                state.execution = None;
                state.delivery_revision = 0;
                state.candidate_delivery_frontier = StreamOffset::ZERO;
                state.committed_delivery_frontier = StreamOffset::ZERO;
            }
        }
        for connector_id in &self.candidate_touched_connectors {
            let active = self.connectors.get(connector_id).is_some_and(|connector| {
                let state = connector
                    .lock()
                    .expect("Connector lock must remain usable during candidate cleanup");
                state.visible || state.requested
            });
            if !active {
                self.active_deadlines.remove(connector_id);
                self.active_connectors.remove(connector_id);
            }
        }
    }

    fn defer_source_cleanup(&mut self, cleanup: &PreparedSourceCleanup) {
        // `prepare_content_commit` reserves both candidate-owned tables
        // before the backend handoff.  Receipt-time deferral therefore keeps
        // the old Source membership without growing a shared table.
        if self.pending_source_cleanup_ids.insert(cleanup.connector_id) {
            self.pending_source_cleanups.push(cleanup.clone());
        }
    }

    fn finish_source_cleanup(&mut self, connector_id: u64) {
        if self.pending_source_cleanup_ids.remove(&connector_id) {
            self.pending_source_cleanups
                .retain(|cleanup| cleanup.connector_id != connector_id);
        }
    }

    /// Promotes a candidate association using only the records captured by
    /// `PreparedContentCommit`. All potentially poisonable locks are checked
    /// before the first visible mutation. Receipt-time work uses the
    /// candidate's indexed records and preallocated vectors; it does not build
    /// guard arrays, resolve handles, scan registries, or grow shared
    /// association tables. Returns `true` when a Source cleanup was deferred
    /// after logical promotion and therefore requires another host candidate.
    pub(crate) fn commit_prepared(&mut self, plan: &PreparedContentCommit) -> Result<bool> {
        // Preflight every independently poisonable record without retaining a
        // guard collection across the backend receipt. Host serialization plus
        // this complete pass ensures the body below has no ordinary fallible
        // operation after the first visible mutation.
        for port in &plan.ports {
            let _guard = port
                .record
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned during visible commit"))?;
        }
        for connector in &plan.connectors {
            let _guard = connector
                .record
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned during visible commit"))?;
        }
        // Source records are shared and independently mutable.  Preflight the
        // merged candidate-owned lock plan before any visible association
        // mutation.  Cleanup itself is intentionally deferred until after the
        // logical promotion: a Source that becomes poisoned between this
        // check and cleanup must retain its old membership/subscription rather
        // than partially tearing down an old-visible Connector.
        for source in &plan.sources {
            let _guard =
                source.source.record.lock().map_err(|_| {
                    anyhow!("content Source lock is poisoned during visible commit")
                })?;
        }

        // --- all ordinary pre-promotion fallibility ends above this line ---
        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            if connector.visible {
                if let Some(projection) = connector.candidate_projection.as_ref() {
                    state.projected_source_revision = Some(projection.key.source_revision);
                    // The projection and frontier belong to the prepared
                    // candidate, not to the mutable Connector slot. A newer
                    // delivery tick may have cleared/replaced that slot while
                    // this receipt was outstanding; publishing the current
                    // slot would skip the captured frame or expose newer work
                    // through an older receipt.
                    state.committed_delivery_frontier = connector.delivery_frontier;
                    state.committed_projection = Some(Arc::clone(projection));
                } else if state.committed_delivery_frontier == StreamOffset::ZERO
                    && connector.delivery_frontier > StreamOffset::ZERO
                {
                    // A content-only plan can carry a frontier without a new
                    // projection. Preserve the captured first visible
                    // frontier, but never inspect/publish newer candidate
                    // state here.
                    state.committed_delivery_frontier = connector.delivery_frontier;
                }
            }
        }

        for port in &plan.ports {
            if let Some(old_index) = port.old_connector_index
                && Some(old_index) != port.next_connector_index
            {
                let old = &plan.connectors[old_index];
                let preserve_newer_control = port.old_control_revision.is_some_and(|revision| {
                    old.record
                        .lock()
                        .expect("prepared old Connector lock must remain usable")
                        .control_revision
                        != revision
                });
                set_connector_visible_committed(
                    &mut self.active_deadlines,
                    &mut self.active_connectors,
                    old,
                    false,
                    preserve_newer_control,
                );
            }
            let mut state = port
                .record
                .lock()
                .expect("prepared ContentPort lock must remain usable after preflight");
            state.visible_mounted = port.mounted;
            state.visible_connector = if port.mounted {
                port.next_connector_id
            } else {
                None
            };
            drop(state);
            if let Some(next_index) = port.next_connector_index {
                set_connector_visible_committed(
                    &mut self.active_deadlines,
                    &mut self.active_connectors,
                    &plan.connectors[next_index],
                    true,
                    false,
                );
            }
        }
        for change in &plan.binding_changes {
            let port = &plan.ports[change.port_index];
            let unresolved = {
                let state = port
                    .record
                    .lock()
                    .expect("prepared ContentPort lock must remain usable after preflight");
                let mounted_unresolved = state.desired_mounted != state.visible_mounted;
                let selection_unresolved =
                    state.desired_connector != state.visible_connector && port.retry_selection;
                mounted_unresolved || selection_unresolved
            };
            if !unresolved
                && self.pending_binding_revisions.get(&port.id).copied() == Some(change.revision)
            {
                self.pending_binding_changes.remove(&port.id);
                self.pending_binding_revisions.remove(&port.id);
            }
        }

        // Deadline membership is promoted from the same prepared connector
        // records. Newer desired operations retain their own pending epoch;
        // no current Source lookup is needed at receipt time.
        for connector in &plan.connectors {
            let state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            let current_delivery_input = state.execution.as_ref().and_then(|execution| {
                execution.delivery.as_ref().map(|delivery| {
                    (
                        delivery.indexed_generation,
                        delivery.indexed_revision,
                        delivery.indexed_sealed,
                    )
                })
            });
            let current_delivery_revision = state.delivery_revision;
            let current_control_revision = state.control_revision;
            drop(state);
            if connector.visible {
                if current_delivery_input != connector.delivery_input
                    || current_delivery_revision != connector.delivery_revision
                    || current_control_revision != connector.control_revision
                {
                    // A newer Source wake advanced this Connector while the
                    // old receipt was outstanding. Its newer deadline/index
                    // is already authoritative and must not be overwritten
                    // by the old candidate's schedule.
                    continue;
                }
                if let Some(deadline) = connector.deadline {
                    if let Some(current) = self.active_deadlines.get_mut(&connector.id) {
                        *current = deadline;
                    }
                } else {
                    self.active_deadlines.remove(&connector.id);
                    self.active_connectors.remove(&connector.id);
                }
            } else {
                self.active_deadlines.remove(&connector.id);
                self.active_connectors.remove(&connector.id);
            }
        }

        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            if state.lifecycle == ConnectorLifecycle::Disposed {
                continue;
            }
            if state.visible {
                state.cleanup_error = None;
                state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                    "disposing"
                } else {
                    "active"
                };
            } else if state.lifecycle == ConnectorLifecycle::Disposing {
                state.phase = "disposing";
            } else if state.requested {
                state.phase = if state.error.is_some() {
                    "failed"
                } else if connector.visible {
                    "activation-pending"
                } else {
                    "waiting-for-mount"
                };
            } else {
                state.phase = "idle";
            }
        }
        // A Connector reactivated while its older cleanup was pending no
        // longer needs that cleanup.  It was included in this candidate via
        // the pending ID table, so cancel the stale deferred entry without a
        // registry scan.
        for connector in &plan.connectors {
            if connector.visible {
                self.finish_source_cleanup(connector.id);
            }
        }

        // Source association cleanup is deliberately after logical
        // promotion.  If a Source becomes poisoned in this narrow window,
        // retain both its membership and wake subscription and retry from a
        // later candidate; do not report the already-promoted frame as an
        // aborted commit or partially tear down an old-visible Connector.
        #[cfg(test)]
        let mut source_cleanup_completed = false;
        for cleanup in &plan.source_cleanups {
            #[cfg(test)]
            if source_cleanup_completed
                && self.test_poison_source_after_first_cleanup == Some(cleanup.source_id)
            {
                self.test_poison_source_after_first_cleanup = None;
                let source_record = cleanup.source.record.clone();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _guard = source_record.lock().unwrap();
                    panic!("intentional post-promotion Source cleanup failure");
                }));
            }
            let Ok(mut state) = cleanup.record.lock() else {
                self.defer_source_cleanup(cleanup);
                continue;
            };
            let unsubscribe = cleanup.unsubscribe && !state.requested && state.subscribed;
            let release_membership =
                !state.membership_released && state.lifecycle == ConnectorLifecycle::Disposing;
            if !unsubscribe && !release_membership {
                state.cleanup_error = None;
                drop(state);
                self.finish_source_cleanup(cleanup.connector_id);
                continue;
            }
            let Ok(mut source) = cleanup.source.record.lock() else {
                state.cleanup_error = Some(Arc::clone(&cleanup.error));
                drop(state);
                self.defer_source_cleanup(cleanup);
                continue;
            };
            if unsubscribe {
                remove_source_subscription_locked(
                    &mut source,
                    &self.owner_host,
                    cleanup.connector_id,
                    cleanup.connector_generation,
                );
                state.subscribed = false;
            }
            if release_membership {
                release_source_membership_locked(&mut source);
                state.membership_released = true;
            }
            drop(source);
            state.cleanup_error = None;
            drop(state);
            self.finish_source_cleanup(cleanup.connector_id);
            #[cfg(test)]
            {
                source_cleanup_completed = true;
            }
        }
        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable during cleanup");
            let captured_projection_is_current = match (
                connector.candidate_projection.as_ref(),
                state.candidate_projection.as_ref(),
            ) {
                (Some(captured), Some(current)) => Arc::ptr_eq(captured, current),
                (None, None) => true,
                _ => false,
            };
            if captured_projection_is_current
                && state.control_revision == connector.control_revision
                && state.delivery_revision == connector.delivery_revision
            {
                state.candidate_projection = None;
                state.candidate_delivery_frontier = state.committed_delivery_frontier;
                if !state.visible {
                    state.committed_projection = None;
                    state.projection_cache.clear();
                    state.prepared_paint_cache.clear();
                    state.semantic_cache.clear();
                    state.prefix_proof_cache.clear();
                    state.projected_source_revision = None;
                    state.execution = None;
                    state.delivery_revision = 0;
                    state.candidate_delivery_frontier = StreamOffset::ZERO;
                    state.committed_delivery_frontier = StreamOffset::ZERO;
                }
            }
            let active = state.visible || state.requested;
            if !active {
                self.active_deadlines.remove(&connector.id);
                self.active_connectors.remove(&connector.id);
            }
        }
        for connector in &plan.connectors {
            if self.pending_source_cleanup_ids.contains(&connector.id) {
                continue;
            }
            remove_prepared_connector_committed(&mut self.connectors, connector);
        }
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_commit_prepared = false;
        Ok(!self.pending_source_cleanup_ids.is_empty())
    }

    pub(crate) fn fail_next_activation(
        &mut self,
        connector_id: u64,
        diagnostic: String,
    ) -> Result<()> {
        if diagnostic.is_empty() {
            return Err(anyhow!("activation failure diagnostic cannot be empty"));
        }
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live {
            return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not live"));
        }
        if state.visible {
            return Err(anyhow!(
                "INVALID_ARGUMENT: cannot inject activation failure for a visible Connector"
            ));
        }
        state.activation_failure = Some(diagnostic);
        Ok(())
    }

    fn request_activation(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (
            port,
            generation,
            source,
            was_requested,
            was_selected,
            port_mounted,
            was_failed,
            smooth,
        ) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not activatable"));
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (was_selected, port_mounted) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    port_state.desired_connector == Some(connector_id),
                    port_state.desired_mounted,
                )
            };
            (
                port,
                state.generation,
                state.source.clone(),
                state.requested,
                was_selected,
                port_mounted,
                state.error.is_some(),
                state.funnel.smooth_config().is_some(),
            )
        };
        if was_requested && was_selected && !was_failed {
            return Ok(false);
        }
        let old_selected = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        self.touch_port(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id,
        );
        if let Some(old_id) = old_selected
            && old_id != connector_id
        {
            self.clear_requested(old_id)?;
        }
        {
            let mut port_state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            port_state.desired_connector = Some(connector_id);
        }
        self.mark_binding_change(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id,
        );
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = true;
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
            state.phase = if port_mounted {
                "activation-pending"
            } else {
                "waiting-for-mount"
            };
        }
        if smooth && port_mounted {
            self.active_connectors.insert(connector_id);
        } else {
            // Immediate delivery has no native deadline, and a smooth
            // Connector selected before its Port is mounted is cold. Keep
            // this index reserved for mounted connectors whose clock can
            // actually advance so unrelated host work cannot trigger parser
            // or delivery work for a cold destination.
            self.active_connectors.remove(&connector_id);
        }
        if port_mounted {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        // The request itself is not the activation/projection operation. A
        // A mounted candidate is processed during content measurement inside
        // the frame transaction, where injected/real operational failure can
        // fall back to the committed Connector without changing the visible
        // frame.
        Ok(port_mounted)
    }

    fn request_deactivation(&mut self, connector_id: u64) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (port, source, generation, was_visible, was_requested, was_selected, visible_connector) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not active"));
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (was_selected, visible_connector) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    port_state.desired_connector == Some(connector_id),
                    port_state.visible_connector,
                )
            };
            (
                port,
                state.source.clone(),
                state.generation,
                state.visible,
                state.requested,
                was_selected,
                visible_connector,
            )
        };
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        self.touch_port(port_id);
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        if !was_requested && !was_visible && !in_flight {
            return Ok(false);
        }
        self.mark_binding_change(port_id);
        if was_selected
            && port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .desired_connector
                == Some(connector_id)
        {
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .desired_connector = None;
        }
        // A captured candidate may still select this Connector even though it
        // is not visible in the old frame. Keep its subscription and identity
        // until that receipt commits or aborts; otherwise the old candidate
        // can resurrect a deactivated Connector without a follow-up epoch.
        if !was_visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        state.control_revision = state
            .control_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
        {
            state.requested = false;
            state.phase = if state.visible { "active" } else { "idle" };
        }
        if !was_visible {
            self.active_connectors.remove(&connector_id);
        }
        // A failed switch keeps the old visible Connector (rollback)
        // while the requested candidate remains selected. Deactivating that
        // candidate must still schedule the removal of the rolled-back visible;
        // otherwise the port would stay visibly active with no requested
        // Connector and no pending epoch to remove it.
        Ok(was_visible
            || in_flight
            || (was_requested && was_selected && visible_connector.is_some()))
    }

    fn request_connector_disposal(&mut self, connector_id: u64) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        let (port, source, generation, visible, desired, visible_connector) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle == ConnectorLifecycle::Disposed
                || state.lifecycle == ConnectorLifecycle::Disposing
            {
                return Ok(false);
            }
            state.lifecycle = ConnectorLifecycle::Disposing;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = false;
            state.phase = "disposing";
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (desired, visible_connector) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    (port_state.desired_connector == Some(connector_id)).then_some(()),
                    port_state.visible_connector,
                )
            };
            (
                port,
                state.source.clone(),
                state.generation,
                state.visible,
                desired,
                visible_connector,
            )
        };
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        self.touch_port(port_id);
        self.mark_binding_change(port_id);
        if !visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation)?;
            self.active_connectors.remove(&connector_id);
        }
        if desired.is_some()
            && let Ok(mut port_state) = port.lock()
        {
            port_state.desired_connector = None;
        }
        if visible || in_flight {
            // The captured candidate still owns a short-lived identity lease;
            // finalize only after commit/abort reconciles that candidate.
            return Ok(true);
        }
        let removes_visible_rollback = desired.is_some() && visible_connector.is_some();
        self.remove_connector(connector_id);
        Ok(removes_visible_rollback)
    }

    fn dispose_port(&mut self, port: &Arc<Mutex<PortRecord>>) -> Result<()> {
        let (id, desired_mounted, visible_mounted, connector_ids) = {
            let state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (
                state.id,
                state.desired_mounted,
                state.visible_mounted,
                state.connector_ids.clone(),
            )
        };
        if desired_mounted || visible_mounted {
            return Err(anyhow!(
                "PORT_MOUNTED: ContentPort {id} is still structurally mounted"
            ));
        }
        if !connector_ids.is_empty() {
            return Err(anyhow!(
                "PORT_IN_USE: ContentPort {id} still has Connector membership"
            ));
        }
        if let Ok(mut state) = port.lock() {
            state.lifecycle = PortLifecycle::Disposed;
        }
        self.ports.remove(&id);
        Ok(())
    }

    pub(crate) fn dispose_all(&mut self) {
        self.in_flight_connectors.clear();
        let connector_ids = self.connectors.keys().copied().collect::<Vec<_>>();
        for connector_id in connector_ids {
            self.remove_connector(connector_id);
        }
        for port in self.ports.values() {
            if let Ok(mut state) = port.lock() {
                state.lifecycle = PortLifecycle::Disposed;
                state.desired_mounted = false;
                state.visible_mounted = false;
                state.desired_connector = None;
                state.visible_connector = None;
            }
        }
        self.ports.clear();
        self.active_connectors.clear();
        self.active_sync_scratch.clear();
        self.due_connector_scratch.clear();
        self.candidate_selections.clear();
        self.pending_binding_changes.clear();
        self.pending_binding_revisions.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.pending_source_cleanups.clear();
        self.pending_source_cleanup_ids.clear();
        #[cfg(test)]
        {
            self.test_poison_source_after_first_cleanup = None;
        }
        self.candidate_source_snapshots.borrow_mut().clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.history_adapter = HistoryTerminalAdapter::new();
    }

    fn refresh_requested_phase(
        &mut self,
        connector_id: u64,
        mounted: bool,
        remounted: bool,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Ok(());
        }
        if remounted && mounted {
            // A failed candidate is retryable on a real remount. Clear only
            // the old operational diagnostic here; an error from the new
            // candidate will be recorded again during frame preparation.
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
        }
        if !mounted && !state.visible {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
        }
        state.phase = if state.error.is_some() && mounted {
            "failed"
        } else if mounted {
            "activation-pending"
        } else {
            "waiting-for-mount"
        };
        Ok(())
    }

    fn ensure_requested_subscription(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, requested, visible, lifecycle) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (
                state.source.clone(),
                state.generation,
                state.requested,
                state.visible,
                state.lifecycle,
            )
        };
        if requested && !visible && lifecycle == ConnectorLifecycle::Live {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        Ok(())
    }

    fn unsubscribe_requested_if_not_visible(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        Ok(())
    }

    fn clear_requested(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = false;
            if !state.visible {
                state.phase = "idle";
            }
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        Ok(())
    }

    fn subscribe_connector(
        &mut self,
        connector_id: u64,
        source: &HostContentSource,
        generation: u32,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!("STALE_HANDLE: Connector is unavailable"));
        };
        let already_subscribed = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed;
        if already_subscribed {
            return Ok(());
        }
        source.subscribe(host, connector_id, generation)?;
        connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed = true;
        Ok(())
    }

    fn unsubscribe_connector(
        &mut self,
        source: &HostContentSource,
        id: u64,
        generation: u32,
    ) -> Result<()> {
        source.unsubscribe(&self.owner_host, id, generation)?;
        if let Some(connector) = self.connectors.get(&id) {
            connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?
                .subscribed = false;
        }
        Ok(())
    }

    fn set_connector_visible_record(
        &mut self,
        connector_id: u64,
        connector: &Arc<Mutex<ConnectorRecord>>,
        visible: bool,
        synchronize_deadline: bool,
        preserve_newer_control: bool,
    ) -> Result<()> {
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned during visibility update"))?;
        state.visible = visible;
        let source = state.source.clone();
        let generation = state.generation;
        if visible {
            state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                "disposing"
            } else {
                "active"
            };
        } else if !preserve_newer_control {
            state.committed_projection = None;
            state.candidate_projection = None;
            state.projection_cache.clear();
            state.prepared_paint_cache.clear();
            state.semantic_cache.clear();
            state.prefix_proof_cache.clear();
            state.projected_source_revision = None;
            state.projection_failure_key = None;
            state.execution = None;
            state.delivery_revision = 0;
            state.candidate_delivery_frontier = StreamOffset::ZERO;
            state.committed_delivery_frontier = StreamOffset::ZERO;
        }
        if !visible && state.lifecycle != ConnectorLifecycle::Disposing {
            state.phase = if state.error.is_some() && state.requested {
                "failed"
            } else if state.requested {
                "activation-pending"
            } else {
                "idle"
            };
        }
        let requested = state.requested;
        drop(state);
        if visible {
            if synchronize_deadline {
                self.sync_connector_deadline(connector_id, None)?;
            }
        } else {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            if !requested {
                source.unsubscribe(&self.owner_host, connector_id, generation)?;
                connector
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned after unsubscribe"))?
                    .subscribed = false;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn set_connector_visible(&mut self, connector_id: u64, visible: bool) {
        self.touch_connector(connector_id);
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        self.set_connector_visible_record(connector_id, &connector, visible, visible, false)
            .expect("test connector visibility update must succeed");
    }

    fn remove_connector(&mut self, connector_id: u64) {
        self.finish_source_cleanup(connector_id);
        self.active_deadlines.remove(&connector_id);
        self.active_connectors.remove(&connector_id);
        let Some(connector) = self.connectors.remove(&connector_id) else {
            return;
        };
        let mut state = connector
            .lock()
            .expect("Connector lock must remain usable during removal");
        let source = state.source.clone();
        let generation = state.generation;
        let subscribed = state.subscribed;
        let membership_released = state.membership_released;
        let mut source_guard = source
            .record
            .lock()
            .expect("connector Source lock must remain usable during removal");
        if subscribed {
            remove_source_subscription_locked(
                &mut source_guard,
                &self.owner_host,
                connector_id,
                generation,
            );
            state.subscribed = false;
        }
        if !membership_released {
            release_source_membership_locked(&mut source_guard);
            state.membership_released = true;
        }
        drop(source_guard);
        state.lifecycle = ConnectorLifecycle::Disposed;
        state.phase = "disposed";
        state.visible = false;
        state.requested = false;
        state.cleanup_error = None;
        let port = state.port.upgrade();
        drop(state);
        if let Some(port) = port {
            let mut port_state = port
                .lock()
                .expect("ContentPort lock must remain usable during removal");
            port_state.connector_ids.remove(&connector_id);
            if port_state.desired_connector == Some(connector_id) {
                port_state.desired_connector = None;
            }
            if port_state.visible_connector == Some(connector_id) {
                port_state.visible_connector = None;
            }
        }
    }

    fn finalize_disposed_connectors(&mut self, candidate_ids: &[u64]) {
        for id in candidate_ids.iter().copied() {
            let removable = self.connectors.get(&id).is_some_and(|connector| {
                let state = connector
                    .lock()
                    .expect("Connector lock must remain usable during finalization");
                state.lifecycle == ConnectorLifecycle::Disposing && !state.visible
            });
            if removable {
                self.remove_connector(id);
            }
        }
    }

    pub(crate) fn connector_status(&self, id: u64) -> Result<ContentConnectorStatus> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(ContentConnectorStatus {
            phase: state.phase.to_owned(),
            requested: state.requested,
            visible: state.visible,
            projected_source_revision: state.projected_source_revision,
            error: state.error.clone(),
            cleanup_pending: state.cleanup_error.is_some(),
            cleanup_error: state.cleanup_error.as_deref().cloned(),
        })
    }

    pub(crate) fn port_status(&self, id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&id)
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {id} is unavailable"))?;
        Ok(port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .visible_mounted)
    }

    fn dirty_for_port(&self, port_id: u64, reason: ContentDirtyReason) -> ContentDirty {
        ContentDirty::new(port_id, None, reason)
    }

    fn dirty_for_connector(
        &self,
        connector_id: u64,
        reason: ContentDirtyReason,
    ) -> Result<ContentDirty> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let port_id = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        Ok(ContentDirty::new(port_id, Some(connector_id), reason))
    }

    pub(crate) fn set_history_unit(
        &mut self,
        port_id: u64,
        unit_id: u64,
        insets: crate::presentation::Insets,
    ) -> Result<()> {
        let port = self
            .ports
            .get(&port_id)
            .ok_or_else(|| anyhow!("INTERNAL_INVARIANT: ContentPort {port_id} disappeared"))?;
        drop(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?,
        );
        self.history_adapter.bind_unit(port_id, unit_id, insets);
        Ok(())
    }

    fn history_rows(&self, port_id: u64, offered_width: u16) -> Option<HistoryContentRows> {
        let port = self.ports.get(&port_id)?;
        let connector_id = {
            let state = port.lock().ok()?;
            state.visible_connector.or(state.desired_connector)
        }?;
        let insets = self.history_adapter.insets(port_id);
        let committed_rows = self.history_adapter.committed_rows(port_id);
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let snapshot = self.source_snapshot_for(&connector.source).ok()?;
        let sealed = snapshot.sealed;
        drop(connector);
        let content_width =
            offered_width.saturating_sub(insets.left().saturating_add(insets.right()));
        let projection = self.connector_projection(connector_id, content_width)?;
        // History consumes the separately proved finalized-prefix product,
        // not a row-count slice of the open document.  An open Markdown tail
        // can be reinterpreted as more bytes arrive, so slicing
        // `projection.rows` would export rows that were never finalized (and
        // could disagree with the sealed-prefix rendering).
        let content_rows = projection
            .finalized_prefix
            .as_ref()
            .map_or(&[][..], |prefix| &prefix.rows[..]);
        let top_padding = usize::from(insets.top());
        // History is irreversible.  Only rows that have crossed the same
        // finalized/delivered frontier used by the screen may be transferred;
        // a sealed Source can still have Smooth backlog.  In particular, do
        // not treat sealing as permission to export the unmasked tail.
        let transferable_content_rows = projection.stable_rows.min(content_rows.len());
        let complete_content = sealed && transferable_content_rows >= content_rows.len();
        let bottom_padding = complete_content
            .then_some(usize::from(insets.bottom()))
            .unwrap_or(0);
        let total_height = top_padding
            .saturating_add(transferable_content_rows)
            .saturating_add(bottom_padding);
        let content_start = top_padding;
        let content_end = content_start.saturating_add(content_rows.len());
        let stable_end = content_start
            .saturating_add(transferable_content_rows)
            .saturating_add(bottom_padding);
        let start = committed_rows.min(total_height);
        let end = stable_end.min(total_height);

        let rows = if start < end {
            (start..end)
                .map(|row_idx| {
                    if row_idx < top_padding || row_idx >= content_end {
                        let cells = vec![PhysicalCell::transparent(); usize::from(offered_width)];
                        PhysicalRow::from_cells(cells)
                    } else if row_idx - top_padding < transferable_content_rows {
                        let content_row = &content_rows[row_idx - top_padding];
                        let placed = content_row.placed(offered_width, insets.left());
                        // Finalized-prefix rows are whole compiled rows.  A
                        // Smooth cut belongs to the open paint product and
                        // must never be applied to this independent History
                        // product.
                        placed
                    } else {
                        PhysicalRow::from_cells(vec![
                            PhysicalCell::transparent();
                            usize::from(offered_width)
                        ])
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let payload_content_start = content_start.max(start).saturating_sub(start);
        let payload_content_end = content_end.min(end).saturating_sub(start);
        let leading_padding = top_padding
            .saturating_sub(start)
            .min(end.saturating_sub(start));
        let trailing_padding = if sealed {
            end.saturating_sub(content_end.max(start))
                .min(bottom_padding)
        } else {
            0
        };
        Some(HistoryContentRows {
            rows,
            complete: complete_content && end >= total_height,
            content_start: payload_content_start.min(payload_content_end),
            content_end: payload_content_end.max(payload_content_start),
            leading_padding,
            trailing_padding,
        })
    }

    pub(crate) fn history_rows_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        self.history_adapter.record_committed(
            port_id,
            rows,
            content_rows,
            leading_padding,
            trailing_padding,
        );
    }

    pub(crate) fn clear_history_unit(&mut self, unit_id: u64) {
        self.history_adapter.clear_unit(unit_id);
    }

    pub(crate) fn history_unit_retired(&mut self, unit_id: u64) {
        let ports = self.history_adapter.retire_unit(unit_id);
        for port_id in ports {
            self.pending_binding_changes.remove(&port_id);
            self.pending_binding_revisions.remove(&port_id);
            self.candidate_binding_changes.remove(&port_id);
            self.candidate_binding_revisions.remove(&port_id);
            self.candidate_touched_ports.remove(&port_id);
            let connector_ids = if let Some(port) = self.ports.remove(&port_id) {
                if let Ok(mut state) = port.lock() {
                    state.desired_mounted = false;
                    state.visible_mounted = false;
                    state.desired_connector = None;
                    state.visible_connector = None;
                    state.lifecycle = PortLifecycle::Disposed;
                    state.connector_ids.clone()
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            };
            for connector_id in connector_ids {
                self.candidate_touched_connectors.remove(&connector_id);
                self.remove_connector(connector_id);
            }
        }
    }

    fn connector_is_candidate_ready(&self, id: u64) -> Result<bool> {
        let Some(connector) = self.connectors.get(&id) else {
            return Ok(false);
        };
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.lifecycle == ConnectorLifecycle::Live
            && state.requested
            && state.error.is_none()
            && state.candidate_projection.is_some())
    }

    pub(super) fn source_subscription_is_live(
        &mut self,
        id: u64,
        generation: u32,
        source_revision: u64,
    ) -> Result<Option<ContentDirty>> {
        let Some(connector) = self.connectors.get(&id).cloned() else {
            return Ok(None);
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.generation != generation
            || !state.subscribed
            || state.lifecycle == ConnectorLifecycle::Disposed
        {
            return Ok(None);
        }
        if state.error.is_some()
            && state.requested
            && !state.visible
            && state
                .failed_source_revision
                .is_some_and(|failed_revision| source_revision > failed_revision)
        {
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
            state.phase = "activation-pending";
        }
        let in_flight = self.in_flight_connectors.contains(&id);
        let cleanup_pending = self.pending_source_cleanup_ids.contains(&id);
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let (port_id, port_mounted) = {
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (port.id, port.desired_mounted)
        };
        let is_live = state.visible
            || (state.requested && port_mounted)
            || in_flight
            // A Source mutation is an independent readiness signal for a
            // deferred cleanup. It admits exactly one retry candidate; a
            // persistently poisoned Source is then blocked by the normal
            // environment failure path rather than spinning on every tick.
            || cleanup_pending;
        drop(state);
        if is_live {
            self.sync_connector_deadline(id, None)?;
        }
        Ok(is_live.then_some(ContentDirty::new(
            port_id,
            Some(id),
            ContentDirtyReason::SourceInput,
        )))
    }

    pub(crate) fn connector_is_disposed(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_none_or(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }

    #[cfg(test)]
    pub(crate) fn poison_connector_for_test(&self, id: u64) -> Result<()> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = connector
                .lock()
                .expect("Connector must be healthy before poison");
            panic!("intentional Connector lock poison");
        }));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn clear_connector_poison_for_test(&self, id: u64) -> Result<()> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        connector.clear_poison();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn poison_source_after_first_cleanup_for_test(&mut self, source_id: u64) {
        self.test_poison_source_after_first_cleanup = Some(source_id);
    }

    #[cfg(test)]
    pub(crate) fn clear_source_poison_for_test(&self, source: &HostContentSource) {
        source.record.clear_poison();
    }

    #[cfg(test)]
    pub(crate) fn pending_source_cleanup_count(&self) -> usize {
        self.pending_source_cleanups.len()
    }

    pub(crate) fn has_pending_source_cleanup(&self) -> bool {
        !self.pending_source_cleanup_ids.is_empty()
    }

    fn deactivate_port(&mut self, port_id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&port_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {port_id} is unavailable"))?;
        let connector_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        match connector_id {
            Some(connector_id) => self.request_deactivation(connector_id),
            None => Ok(false),
        }
    }

    fn request_connector_activation(
        &mut self,
        id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        self.request_activation(id, host)
    }

    fn request_connector_deactivation(&mut self, id: u64) -> Result<bool> {
        self.request_deactivation(id)
    }

    fn request_connector_dispose(&mut self, id: u64) -> Result<bool> {
        self.request_connector_disposal(id)
    }
}

impl ContentProvider for ContentHostRegistry {
    fn set_theme(&mut self, theme: &Arc<Theme>) {
        if Arc::ptr_eq(&self.theme, theme) || *self.theme == **theme {
            return;
        }
        self.theme = Arc::clone(theme);
        self.theme_revision = self
            .theme_revision
            .checked_add(1)
            .expect("content theme revision exhausted");
        for connector in self.connectors.values() {
            if let Ok(mut state) = connector.lock() {
                state.projection_cache.clear();
                state.candidate_projection = None;
            }
        }
    }

    fn projection_revision(&self, port_id: u64, offered_width: u16) -> u64 {
        let connector_revision = self
            .selected_connector_id(port_id)
            .map_or(0, |connector_id| {
                self.connector_revision(connector_id, offered_width)
            });
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        connector_revision.hash(&mut hasher);
        self.theme_revision.hash(&mut hasher);
        self.history_adapter.hash_state(port_id, &mut hasher);
        hasher.finish()
    }

    fn layout_input_revision(&self, port_id: u64, offered_width: u16) -> u64 {
        let connector_id = self.selected_connector_id(port_id);
        let connector_revision = connector_id
            .and_then(|id| self.connector_projection_key(id, offered_width).ok())
            .map_or(0, |key| key.layout_input_revision());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        connector_revision.hash(&mut hasher);
        self.history_adapter.hash_state(port_id, &mut hasher);
        hasher.finish()
    }

    fn measure(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::WidthRule,
    ) -> ContentMeasurement {
        self.measure_content(port_id, offered_width, width_rule)
    }

    fn paint_window(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (u16, u16),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        self.paint_window_direct(ticket, window, target, target_origin, clip, style);
    }

    fn history_rows(&self, port_id: u64, offered_width: u16) -> Option<HistoryContentRows> {
        self.history_rows(port_id, offered_width)
    }

    fn history_rows_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        self.history_rows_committed(
            port_id,
            rows,
            content_rows,
            leading_padding,
            trailing_padding,
        );
    }

    fn history_view(&self, view: &crate::presentation::View) -> crate::presentation::View {
        let Some(port_id) = view.content_attachment_id() else {
            return view.clone();
        };
        let mut insets = view.decoration().padding;
        if self.history_adapter.leading_padding_rows(port_id) > 0 {
            insets.top = 0;
        }
        if self.history_adapter.trailing_padding_rows(port_id) > 0 {
            insets.bottom = 0;
        }
        if insets == view.decoration().padding {
            return view.clone();
        }
        // History has already acknowledged the removed padding rows. Apply
        // the replacement decoration as one final semantic root instead of
        // replaying a modifier chain during every projection.
        view.clone()
            .map_node(|node| node.decoration.padding = insets)
    }

    fn history_unit_retired(&mut self, unit_id: u64) {
        self.history_unit_retired(unit_id);
    }

    fn history_transfer_blocked(&self, port_id: u64, offered_width: u16) -> bool {
        let Some(port) = self.ports.get(&port_id) else {
            return true;
        };
        let Some(state) = port.lock().ok() else {
            return true;
        };
        if self.history_adapter.unit_id(port_id).is_none() {
            return false;
        }
        if state
            .visible_connector
            .or(state.desired_connector)
            .is_none()
        {
            return true;
        }
        drop(state);
        self.history_rows(port_id, offered_width)
            .is_none_or(|rows| rows.rows.is_empty() && !rows.complete)
    }
}

#[derive(Clone, Debug)]
pub struct HostContentPort {
    id: u64,
    generation: u32,
    family: ContentFamily,
    record: Arc<Mutex<PortRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentPort {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn family(&self) -> ContentFamily {
        self.family
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_port(self.id(), ContentDirtyReason::SelectionLifecycle);
        let needs_frame = inner.content.deactivate_port(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn connect(
        &self,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        inner
            .content
            .connect(&self.record, source, funnel)
            .map(|mut connector| {
                connector.host = host_weak;
                connector
            })
    }

    pub fn dispose(&self) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .dispose_port(&self.record)
    }

    pub fn is_mounted(&self) -> Result<bool> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .port_status(self.id)
    }
}

#[derive(Clone, Debug)]
pub struct HostContentConnector {
    id: u64,
    generation: u32,
    source_id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentConnector {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn source_id(&self) -> u64 {
        self.source_id
    }

    pub fn activate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner
            .content
            .request_connector_activation(self.id(), &host_weak)?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner.content.request_connector_deactivation(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn dispose(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner.content.request_connector_dispose(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    /// Injects one deterministic operational failure for a native/unit
    /// fixture. It is not part of the TypeScript content API; real projection
    /// failures use the same candidate-rollback state.
    pub fn fail_next_activation(&self, diagnostic: String) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .fail_next_activation(self.id(), diagnostic)
    }

    pub fn status(&self) -> Result<ContentConnectorStatus> {
        if let Some(host) = self.host.upgrade() {
            let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
            return match inner.content.connector_status(self.id) {
                Ok(status) => Ok(status),
                Err(error) => {
                    // A disposed Connector is removed from the live registry
                    // but detached handles retain its final record. Read that
                    // record only while the owning Host lock is held so live
                    // status never races an in-flight commit.
                    let state = self
                        .record
                        .lock()
                        .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                    if state.lifecycle == ConnectorLifecycle::Disposed {
                        Ok(ContentConnectorStatus {
                            phase: "disposed".to_owned(),
                            requested: state.requested,
                            visible: state.visible,
                            projected_source_revision: state.projected_source_revision,
                            error: state.error.clone(),
                            cleanup_pending: state.cleanup_error.is_some(),
                            cleanup_error: state.cleanup_error.as_deref().cloned(),
                        })
                    } else {
                        Err(error)
                    }
                }
            };
        }
        // HostInner::drop() marks retained Connector records disposed before
        // its weak owner disappears. A live record with no owner is an
        // invariant failure, not a reason to fabricate a status.
        self.record_status()
    }

    pub fn visible_delivery_frontier(&self) -> Result<StreamOffset> {
        let Some(host) = self.host.upgrade() else {
            return self.record_frontier(false);
        };
        let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        match inner.content.connector_delivery_frontier(self.id) {
            Ok(frontier) => Ok(frontier),
            Err(error) => {
                let state = self
                    .record
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                (state.lifecycle == ConnectorLifecycle::Disposed)
                    .then_some(state.committed_delivery_frontier)
                    .ok_or(error)
            }
        }
    }

    pub fn candidate_delivery_frontier(&self) -> Result<StreamOffset> {
        let Some(host) = self.host.upgrade() else {
            return self.record_frontier(true);
        };
        let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        match inner.content.connector_candidate_delivery_frontier(self.id) {
            Ok(frontier) => Ok(frontier),
            Err(error) => {
                let state = self
                    .record
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                (state.lifecycle == ConnectorLifecycle::Disposed)
                    .then_some(state.candidate_delivery_frontier)
                    .ok_or(error)
            }
        }
    }

    fn record_frontier(&self, candidate: bool) -> Result<StreamOffset> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if candidate {
            Ok(state.candidate_delivery_frontier)
        } else {
            Ok(state.committed_delivery_frontier)
        }
    }

    /// Final detached-handle readback is valid only after the owning HostInner
    /// has gone away; while the host is live, `status` routes through its
    /// serialized registry owner.
    fn record_status(&self) -> Result<ContentConnectorStatus> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(ContentConnectorStatus {
            phase: if state.lifecycle == ConnectorLifecycle::Disposed {
                "disposed".to_owned()
            } else {
                state.phase.to_owned()
            },
            requested: state.requested,
            visible: state.visible,
            projected_source_revision: state.projected_source_revision,
            error: state.error.clone(),
            cleanup_pending: state.cleanup_error.is_some(),
            cleanup_error: state.cleanup_error.as_deref().cloned(),
        })
    }

    #[must_use]
    pub fn is_disposed(&self) -> bool {
        if let Some(host) = self.host.upgrade() {
            return host
                .lock()
                .ok()
                .is_none_or(|inner| inner.content.connector_is_disposed(self.id));
        }
        self.record
            .lock()
            .is_ok_and(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }
}

impl Drop for HostContentConnector {
    fn drop(&mut self) {
        // Explicit disposal owns semantic release. A dropped wrapper is not a
        // hidden lifecycle operation; host teardown calls dispose_all instead.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeColor;
    use crate::application::environment::TuiEnvironment;
    use crate::application::host::TuiHost;
    use crate::presentation::factory as vf;

    /// Forced-boundary Source: accepted bytes installed at revision 1, then
    /// the revision counter pinned to its limit. The returned source always
    /// holds `"keep\n"` at revision 1 before pinning.
    fn revision_pinned_source() -> HostContentSource {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"keep\n", &[], &[]).unwrap();
        source
            .record
            .lock()
            .map(|mut record| record.revision = u64::MAX)
            .unwrap();
        source
    }

    fn tag_annotation(payload: &[u8]) -> ContentAnnotationRecord {
        ContentAnnotationRecord {
            kind: CONTENT_ANNOTATION_KIND_TAG,
            flags: 0,
            start_byte: 0,
            end_byte: 4,
            payload_offset: 0,
            payload_length: payload.len() as u32,
            aux0: 0,
            aux1: 0,
        }
    }

    #[test]
    fn exhausted_source_revision_rejects_append_without_installing_anything() {
        // Counter-exhaustion atomicity (§9.6): the revision preflight runs
        // before candidate storage is installed, so a rejection leaves
        // bytes, annotations, revision and accounting exactly as they were.
        // A normal "not accepted" report after a partial install would invite
        // a duplicating retry.
        let source = revision_pinned_source();
        let payload = b"exhaustion\x00probe";
        let result = source.append_utf8(b"atomic\n", &[tag_annotation(payload)], payload);
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );

        let stats = source.stats().unwrap();
        assert_eq!(stats.revision, u64::MAX, "revision must not advance");
        assert_eq!(stats.accepted_bytes, 5, "accepted bytes must not move");
        assert_eq!(
            stats.copied_bytes, 5,
            "setup accounting must not move on rejection"
        );
        assert_eq!(stats.source_end, 5, "rejected bytes must not be installed");
        assert_eq!(
            source.snapshot().unwrap().text(),
            "keep\n",
            "no unaccepted bytes may be observable"
        );
    }

    #[test]
    fn exhausted_annotation_sequence_rejects_batch_without_accounting() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        source
            .record
            .lock()
            .map(|mut record| {
                Arc::make_mut(&mut record.storage).next_seqno = u64::MAX - 1;
            })
            .unwrap();

        let records = [
            ContentAnnotationRecord {
                kind: CONTENT_ANNOTATION_KIND_POINT,
                start_byte: 0,
                end_byte: 0,
                ..ContentAnnotationRecord::default()
            },
            ContentAnnotationRecord {
                kind: CONTENT_ANNOTATION_KIND_POINT,
                start_byte: 1,
                end_byte: 1,
                ..ContentAnnotationRecord::default()
            },
        ];
        let error = source.append_utf8(b"ab", &records, &[]).unwrap_err();
        assert!(
            error.to_string().contains("annotation sequence exhausted"),
            "unexpected sequence error: {error:#}"
        );
        let stats = source.stats().unwrap();
        assert_eq!(stats.source_end, 0);
        assert_eq!(stats.accepted_bytes, 0);
        assert_eq!(stats.copied_bytes, 0);
        assert_eq!(stats.revision, 0);
        assert!(source.snapshot().unwrap().annotations().is_empty());

        // One identity remains available and is accepted exactly once after
        // the rejected full batch.
        let one = records[..1].to_vec();
        source.append_utf8(b"a", &one, &[]).unwrap();
        let stats = source.stats().unwrap();
        assert_eq!(stats.source_end, 1);
        assert_eq!(stats.accepted_bytes, 1);
        assert_eq!(stats.copied_bytes, 1);
        assert_eq!(source.snapshot().unwrap().annotations().len(), 1);
    }

    #[test]
    fn drop_oldest_retention_keeps_suffix_after_multibyte_chunk_end() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        source.configure_retention(Some(5), None, true).unwrap();

        source.append_utf8("abcé".as_bytes(), &[], &[]).unwrap();
        source.append_utf8(b"tail", &[], &[]).unwrap();

        let stats = source.stats().unwrap();
        assert_eq!(stats.source_base, 5);
        assert_eq!(stats.source_end, 9);
        assert_eq!(stats.retained_bytes, 4);
        assert_eq!(stats.dropped_head_bytes, 5);
        assert!(stats.head_partial);
        assert_eq!(source.snapshot().unwrap().text(), "tail");
    }

    #[test]
    fn exhausted_source_revision_rejects_replace_without_touching_storage() {
        // Replace preflights the revision before installing the fresh root,
        // so the old storage survives the rejection intact.
        let source = revision_pinned_source();
        let result = source.replace_utf8(b"new\n", &[], &[]);
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );
        assert_eq!(source.snapshot().unwrap().text(), "keep\n");
        assert_eq!(source.stats().unwrap().revision, u64::MAX);
    }

    #[test]
    fn exhausted_source_revision_rejects_clear_without_touching_storage() {
        // Clear also preflights the revision before swapping in the empty
        // root: rejection leaves the retained bytes in place.
        let source = revision_pinned_source();
        let result = source.clear();
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );
        assert_eq!(source.snapshot().unwrap().text(), "keep\n");
    }

    #[test]
    fn exhausted_content_generation_rejects_clear_without_touching_storage() {
        // Clear preflights both revision and generation before swapping in
        // the empty root: rejection leaves the retained bytes in place.
        let source = revision_pinned_source();
        source
            .record
            .lock()
            .map(|mut record| {
                record.revision = 1;
                record.content_generation = u64::MAX;
            })
            .unwrap();
        let result = source.clear();
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source content generation exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );
        assert_eq!(
            source.snapshot().unwrap().text(),
            "keep\n",
            "rejected clear must not drop retained bytes"
        );
        assert_eq!(source.stats().unwrap().revision, 1);
    }

    #[test]
    fn exhausted_source_revision_rejects_seal_without_setting_sealed() {
        // Seal preflights the revision before flipping the flag: a rejected
        // seal leaves the Source unsealed at the pinned revision.
        let source = revision_pinned_source();
        let result = source.seal();
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );
        let snapshot = source.snapshot().unwrap();
        assert!(
            !snapshot.sealed,
            "rejected seal must not flip the sealed flag"
        );
        assert_eq!(stats_revision(&source), u64::MAX);
    }

    #[test]
    fn exhausted_source_revision_rejects_truncate_without_moving_head() {
        // Truncate preflights the revision before dropping the head: a
        // rejected truncate leaves the retained range where it was.
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"abcdefgh\n", &[], &[]).unwrap();
        source
            .record
            .lock()
            .map(|mut record| record.revision = u64::MAX)
            .unwrap();
        let result = source.truncate_head(3);
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("Source revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );
        let stats = source.stats().unwrap();
        assert_eq!(
            stats.source_base, 0,
            "rejected truncate must not advance the head"
        );
        assert_eq!(stats.revision, u64::MAX);
    }

    fn stats_revision(source: &HostContentSource) -> u64 {
        source.stats().map(|stats| stats.revision).unwrap_or(0)
    }

    #[test]
    fn source_subscriptions_are_scoped_by_host() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let funnel = HostContentFunnel::plain(TextWrapMode::Word);
        let first_port = first.create_content_port(ContentFamily::Text).unwrap();
        let first_connector = first_port.connect(&source, funnel).unwrap();
        first_connector.activate().unwrap();
        assert_eq!(source.subscriber_count(), 0);
        first
            .set_desired_view(vf::content_host(first_port.id()).unwrap())
            .unwrap();
        first.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 1);

        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        let second_connector = second_port.connect(&source, funnel).unwrap();
        second_connector.activate().unwrap();
        assert_eq!(source.subscriber_count(), 1);
        second
            .set_desired_view(vf::content_host(second_port.id()).unwrap())
            .unwrap();
        second.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 2);

        first_connector.deactivate().unwrap();
        first.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 1);

        first.close().unwrap();
        assert_eq!(source.subscriber_count(), 1);
        second.close().unwrap();
        assert_eq!(source.subscriber_count(), 0);
        source.dispose().unwrap();
    }

    #[test]
    fn poisoned_subscriber_wake_reports_accepted_revision_and_healthy_drain() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let funnel = HostContentFunnel::plain(TextWrapMode::Word);

        let first_port = first.create_content_port(ContentFamily::Text).unwrap();
        let first_conn = first_port.connect(&source, funnel).unwrap();
        first_conn.activate().unwrap();
        first
            .set_desired_view(vf::content_host(first_port.id()).unwrap())
            .unwrap();
        first.flush_pending_hosts(32, true).unwrap();

        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        let second_conn = second_port.connect(&source, funnel).unwrap();
        second_conn.activate().unwrap();
        second
            .set_desired_view(vf::content_host(second_port.id()).unwrap())
            .unwrap();
        second.flush_pending_hosts(32, true).unwrap();

        assert_eq!(source.subscriber_count(), 2);
        let first_host_id = first.epochs().unwrap().host_id;

        // Deliberately poison first host's mutex
        let inner_clone = first.inner.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = inner_clone.lock().unwrap();
            panic!("deliberate host lock poisoning for test");
        }));

        // The Source revision is accepted even though one subscriber cannot
        // be locked.  The wake failure is delivered through the environment
        // report instead of the mutation result, so direct FFI callers still
        // receive the authoritative revision and drain hint.
        let mutation = source.append_utf8(b"hello\n", &[], &[]).unwrap();
        assert_eq!(mutation.revision, 1);
        assert!(mutation.schedule_environment_drain);

        // The healthy subscriber still receives the wake and presents the
        // accepted bytes in the same fair drain that reports the failed host.
        let report = second.flush_pending_hosts(32, false).unwrap();
        assert!(report.errors.iter().any(|error| {
            error.host_id == first_host_id
                && error.code == "SOURCE_WAKE_FAILED"
                && error.diagnostic.contains("Source revision 1 was accepted")
        }));
        assert!(
            report
                .commits
                .iter()
                .any(|commit| commit.host_id == second.epochs().unwrap().host_id),
            "healthy subscriber must commit despite a failed wake"
        );
        assert!(
            second.screen_rows().iter().any(|row| row.contains("hello")),
            "healthy subscriber must present the accepted Source bytes"
        );
        let idle = second.flush_pending_hosts(32, false).unwrap();
        assert_eq!(
            idle.attempted, 0,
            "a reported wake failure must not spin drains"
        );
        assert!(idle.errors.is_empty());
        assert!(!idle.rearm);

        // State is authoritative and NOT rolled back:
        let stats = source.stats().unwrap();
        assert_eq!(stats.revision, 1);
        assert_eq!(stats.accepted_bytes, 6);
        assert_eq!(source.snapshot().unwrap().text(), "hello\n");
    }

    #[test]
    fn stale_source_wake_error_is_dropped_after_subscriber_membership_ends() {
        let environment = TuiEnvironment::new();
        let failed = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let healthy = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let funnel = HostContentFunnel::plain(TextWrapMode::Word);

        for host in [&failed, &healthy] {
            let port = host.create_content_port(ContentFamily::Text).unwrap();
            let connector = port.connect(&source, funnel).unwrap();
            connector.activate().unwrap();
            host.set_desired_view(vf::content_host(port.id()).unwrap())
                .unwrap();
            host.flush_pending_hosts(32, true).unwrap();
        }
        assert_eq!(source.subscriber_count(), 2);

        let failed_inner = failed.inner.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = failed_inner.lock().unwrap();
            panic!("deliberate membership-race host poison");
        }));

        let mutation = source.append_utf8(b"stale\n", &[], &[]).unwrap();
        assert_eq!(mutation.revision, 1);
        assert!(mutation.schedule_environment_drain);

        // Dropping the poisoned host removes its Source membership before the
        // healthy host drains.  Its weak error channel must not be retargeted
        // or surfaced after the owner is gone.
        drop(failed_inner);
        drop(failed);
        assert_eq!(source.subscriber_count(), 1);
        let report = healthy.flush_pending_hosts(32, false).unwrap();
        assert!(
            report
                .errors
                .iter()
                .all(|error| error.code != "SOURCE_WAKE_FAILED"),
            "a stale failed-host channel must be discarded after membership ends"
        );
        assert!(
            healthy
                .screen_rows()
                .iter()
                .any(|row| row.contains("stale")),
            "membership cleanup must not affect the healthy subscriber"
        );
        assert!(
            !report.rearm,
            "stale failure cleanup must not spin the drain"
        );

        healthy.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn stop_condition_persistent_sharing_across_repeated_appends() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();

        // 1. Snapshot taken before appends
        let snap0 = source.snapshot().unwrap();
        assert_eq!(snap0.text(), "");
        assert_eq!(snap0.retained_bytes(), 0);

        // 2. Perform repeated tiny appends with annotations
        let mut snapshots = Vec::new();
        for i in 0..50 {
            let payload = format!("item {i:03}\n");
            let anno_payload = format!("tag-{i}\x00val-{i}");
            let records = [ContentAnnotationRecord {
                kind: CONTENT_ANNOTATION_KIND_TAG,
                flags: 0,
                start_byte: 0,
                end_byte: payload.len() as u32,
                payload_offset: 0,
                payload_length: anno_payload.len() as u32,
                aux0: 0,
                aux1: 0,
            }];
            source
                .append_utf8(payload.as_bytes(), &records, anno_payload.as_bytes())
                .unwrap();
            if i % 10 == 0 {
                snapshots.push(source.snapshot().unwrap());
            }
        }

        // 3. Old snapshots remain completely unchanged
        assert_eq!(snap0.text(), "");
        assert_eq!(snap0.annotations().len(), 0);

        let snap_i0 = &snapshots[0];
        assert_eq!(snap_i0.text(), "item 000\n");
        assert_eq!(snap_i0.annotations().len(), 1);
        let snap_i0_storage = Arc::clone(&snap_i0.storage);
        assert!(
            Arc::ptr_eq(&snap_i0.storage, &snap_i0_storage),
            "native Snapshot storage must be retained by Arc ownership"
        );

        let latest = source.snapshot().unwrap();
        assert_eq!(latest.annotations().len(), 50);
        assert_eq!(latest.retained_lines(), 51); // 50 newlines + base line = 51
        // 4. Test atomic retention + annotations
        source.configure_retention(Some(30), None, true).unwrap();
        source.append_utf8(b"tail-item\n", &[], &[]).unwrap();
        let stats = source.stats().unwrap();
        // Base advanced to keep within 30 bytes
        assert!(stats.source_base > 0);
        let snap_retained = source.snapshot().unwrap();
        assert!(snap_retained.retained_bytes() <= 30);
        // Annotations were atomically pruned/clipped with the head truncation
        for anno in snap_retained.annotations() {
            assert!(anno.end_byte > stats.source_base);
        }
        source
            .replace_utf8(b"replacement generation\n", &[], &[])
            .unwrap();
        assert_eq!(snap_i0.text(), "item 000\n");
        assert_eq!(snap_i0.annotations().len(), 1);
        assert!(
            Arc::ptr_eq(&snap_i0.storage, &snap_i0_storage),
            "Source replacement must not detach an old native Snapshot owner"
        );
        assert_eq!(Arc::strong_count(&snap_i0.storage), 2);
        drop(snap_i0_storage);
        assert_eq!(Arc::strong_count(&snap_i0.storage), 1);
    }

    #[test]
    fn inactive_membership_blocks_source_disposal_until_connector_release() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        assert_eq!(source.subscriber_count(), 0);
        assert!(source.dispose().is_err());
        registry.remove_connector(connector.id());
        source.dispose().unwrap();
    }

    #[test]
    fn content_port_ids_cannot_alias_across_hosts() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let first_port = first.create_content_port(ContentFamily::Text).unwrap();
        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        assert_ne!(first_port.id(), second_port.id());

        let foreign_view = vf::content_host(first_port.id()).unwrap();
        let error = second.set_desired_view(foreign_view).unwrap_err();
        assert!(error.to_string().contains("STALE_HANDLE"));

        first.close().unwrap();
        second.close().unwrap();
    }

    #[test]
    fn activation_failure_is_recorded_by_candidate_rollback() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let port = host.create_content_port(ContentFamily::Text).unwrap();
        let first = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        let second = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        first.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();

        second
            .fail_next_activation("synthetic projection failure".to_owned())
            .unwrap();
        second.activate().unwrap();
        assert_eq!(second.status().unwrap().phase, "activation-pending");
        host.flush_pending_hosts(32, true).unwrap();
        let status = second.status().unwrap();
        assert_eq!(status.phase, "failed");
        assert!(status.requested);
        assert!(!status.visible);
        assert_eq!(
            status.error.as_ref().map(|error| error.code.as_str()),
            Some("PROJECTION_FAILED")
        );
        assert_eq!(first.status().unwrap().phase, "active");

        second.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "active");
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn source_connection_rejects_a_different_environment() {
        let first_sources = ContentSourceRegistry::new();
        let second_sources = ContentSourceRegistry::new();
        let source = first_sources.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(second_sources);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let error = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap_err();
        assert!(error.to_string().contains("WRONG_ENVIRONMENT"));
        source.dispose().unwrap();
    }

    #[test]
    fn failed_connector_retries_on_a_port_remount() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let port = host.create_content_port(ContentFamily::Text).unwrap();
        let first = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        let second = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        first.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        second
            .fail_next_activation("synthetic projection failure".to_owned())
            .unwrap();
        second.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "failed");

        host.set_desired_view(vf::spacer(0)).unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "waiting-for-mount");
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "active");

        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn mounting_unactivated_connector_does_not_requeue_forever() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"not active", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();

        // Mounting is intentionally distinct from activation. The candidate
        // may commit the destination mount while leaving the unrequested
        // Connector cold, but it must not keep re-queuing the unresolved
        // desired selection on every drain.
        registry.set_desired(&[port.id()]).unwrap();
        registry.begin_projection_candidate();
        let measurement =
            registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        assert_eq!(measurement.connector_id, None);
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();

        let port_state = port.record.lock().unwrap();
        assert!(port_state.visible_mounted);
        assert_eq!(port_state.visible_connector, None);
        drop(port_state);
        assert!(registry.pending_binding_changes.is_empty());
        assert_eq!(
            registry
                .connectors
                .get(&connector.id())
                .unwrap()
                .lock()
                .unwrap()
                .phase,
            "idle"
        );
    }

    #[test]
    fn connector_status_survives_native_host_drop_as_disposed() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let connector = {
            let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
            let port = host.create_content_port(ContentFamily::Text).unwrap();
            port.connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
                .unwrap()
        };
        assert_eq!(connector.status().unwrap().phase, "disposed");
        assert_eq!(
            connector.visible_delivery_frontier().unwrap(),
            StreamOffset::ZERO
        );
        assert_eq!(
            connector.candidate_delivery_frontier().unwrap(),
            StreamOffset::ZERO
        );
        assert!(connector.is_disposed());
        source.dispose().unwrap();
    }

    #[test]
    fn disposed_connector_handle_keeps_status_and_frontiers_while_host_lives() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let port = host.create_content_port(ContentFamily::Text).unwrap();
        let connector = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        connector.dispose().unwrap();

        let status = connector.status().unwrap();
        assert_eq!(status.phase, "disposed");
        assert!(!status.requested);
        assert!(!status.visible);
        assert_eq!(
            connector.visible_delivery_frontier().unwrap(),
            StreamOffset::ZERO
        );
        assert_eq!(
            connector.candidate_delivery_frontier().unwrap(),
            StreamOffset::ZERO
        );
        assert!(connector.is_disposed());
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn candidate_control_keeps_in_flight_connector_identity_alive() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let connector_id = connector.id();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector_id);
        }
        registry.in_flight_connectors.insert(connector_id);

        assert!(registry.request_deactivation(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.abort_candidate();
        assert!(registry.connectors.contains_key(&connector_id));
        assert_eq!(source.subscriber_count(), 0);

        {
            let mut state = port.record.lock().unwrap();
            state.desired_connector = Some(connector_id);
        }
        registry.in_flight_connectors.insert(connector_id);
        assert!(registry.request_connector_disposal(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.candidate_binding_changes.insert(port.id());
        registry
            .candidate_selections
            .insert(port.id(), Some(connector_id));
        let plan = registry.prepare_content_commit().unwrap();
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();
        assert!(
            registry
                .connectors
                .get(&connector_id)
                .and_then(|record| record.lock().ok())
                .is_some_and(|state| state.visible)
        );
        registry.candidate_binding_changes.insert(port.id());
        registry.candidate_selections.insert(port.id(), None);
        let plan = registry.prepare_content_commit().unwrap();
        registry.commit_prepared(&plan).unwrap();
        assert!(!registry.connectors.contains_key(&connector_id));
    }

    #[test]
    fn newer_disposal_releases_membership_after_an_older_hidden_plan() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let connector_id = connector.id();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector_id);
            port_state.visible_connector = Some(connector_id);
        }
        {
            let record = registry.connectors.get(&connector_id).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let host = Weak::new();
        registry
            .subscribe_connector(connector_id, &source, connector.generation(), &host)
            .unwrap();
        assert_eq!(source.subscriber_count(), 1);

        registry.begin_projection_candidate();
        assert!(registry.request_deactivation(connector_id).unwrap());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);

        // Disposal is newer than the hidden/deactivation plan. The old plan
        // must still remove the identity, but it must also release the Source
        // membership exactly once even though that release was not present at
        // the plan's original capture boundary.
        assert!(registry.request_connector_disposal(connector_id).unwrap());
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();
        assert!(!registry.connectors.contains_key(&connector_id));
        assert_eq!(source.subscriber_count(), 0);
        source.dispose().unwrap();
    }

    #[test]
    fn newer_disposal_releases_already_unsubscribed_membership() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let connector_id = connector.id();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector_id);
            port_state.visible_connector = Some(connector_id);
        }
        {
            let record = registry.connectors.get(&connector_id).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
            // The Connector remains a Source member even when its wake
            // subscription has already been removed.
            assert!(!state.subscribed);
        }

        registry.begin_projection_candidate();
        assert!(registry.request_deactivation(connector_id).unwrap());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);
        assert!(registry.request_connector_disposal(connector_id).unwrap());
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();
        assert!(!registry.connectors.contains_key(&connector_id));
        source.dispose().unwrap();
    }

    #[test]
    fn source_cleanup_failure_after_first_receipt_is_exactly_once() {
        let source_registry = ContentSourceRegistry::new();
        let first_source = source_registry.create(TextSourceKind::Stream).unwrap();
        let second_source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let first_port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let second_port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let first = registry
            .connect(
                &first_port.record,
                &first_source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let second = registry
            .connect(
                &second_port.record,
                &second_source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        for (port, connector, source) in [
            (&first_port, &first, &first_source),
            (&second_port, &second, &second_source),
        ] {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_connector = Some(connector.id());
            drop(port_state);
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
            drop(state);
            registry
                .subscribe_connector(connector.id(), source, connector.generation(), &Weak::new())
                .unwrap();
        }
        assert_eq!(first_source.subscriber_count(), 1);
        assert_eq!(second_source.subscriber_count(), 1);

        registry.begin_projection_candidate();
        assert!(registry.request_deactivation(first.id()).unwrap());
        assert!(registry.request_deactivation(second.id()).unwrap());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);

        // Poison the second Source after preparation. Commit preflight must
        // fail before any Source cleanup or visible association mutation;
        // retrying the same plan must preserve both old-visible memberships
        // and must not release either membership twice.
        let second_record = second_source.record.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = second_record.lock().unwrap();
            panic!("intentional second Source cleanup failure");
        }));
        let error = registry.commit_prepared(&plan).unwrap_err();
        assert!(error.to_string().contains("Source lock is poisoned"));
        assert_eq!(first_source.subscriber_count(), 1);
        second_record.clear_poison();
        assert_eq!(second_source.subscriber_count(), 1);
        for (port, connector) in [(&first_port, &first), (&second_port, &second)] {
            assert_eq!(
                port.record.lock().unwrap().visible_connector,
                Some(connector.id()),
                "failed Source preflight must preserve the old visible binding"
            );
            assert!(connector.record.lock().unwrap().subscribed);
        }
        assert!(
            first_source.dispose().is_err(),
            "membership must remain until connector removal"
        );

        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();
        assert_eq!(first_source.subscriber_count(), 0);
        assert_eq!(second_source.subscriber_count(), 0);
        registry.remove_connector(first.id());
        registry.remove_connector(second.id());
        assert!(registry.connectors.is_empty());
        first_source.dispose().unwrap();
        second_source.dispose().unwrap();
    }

    #[test]
    fn post_promotion_source_cleanup_failure_retains_membership_for_retry() {
        let source_registry = ContentSourceRegistry::new();
        let first_source = source_registry.create(TextSourceKind::Stream).unwrap();
        let second_source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let first_port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let second_port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let first = registry
            .connect(
                &first_port.record,
                &first_source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let second = registry
            .connect(
                &second_port.record,
                &second_source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        for (port, connector, source) in [
            (&first_port, &first, &first_source),
            (&second_port, &second, &second_source),
        ] {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_connector = Some(connector.id());
            drop(port_state);
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
            drop(state);
            registry
                .subscribe_connector(connector.id(), source, connector.generation(), &Weak::new())
                .unwrap();
        }

        registry.begin_projection_candidate();
        assert!(registry.request_deactivation(first.id()).unwrap());
        assert!(registry.request_deactivation(second.id()).unwrap());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);
        // The test hook poisons the second Source only after the first cleanup
        // has succeeded, reproducing the race window after logical promotion.
        registry.poison_source_after_first_cleanup_for_test(second_source.id());
        assert!(registry.commit_prepared(&plan).unwrap());

        assert_eq!(first_source.subscriber_count(), 0);
        let second_record = second_source.record.clone();
        second_record.clear_poison();
        assert_eq!(second_source.subscriber_count(), 1);
        assert_eq!(
            first_port.record.lock().unwrap().visible_connector,
            None,
            "logical promotion must complete even when cleanup is deferred"
        );
        assert_eq!(
            second_port.record.lock().unwrap().visible_connector,
            None,
            "logical promotion must complete even when cleanup is deferred"
        );
        assert!(registry.pending_source_cleanup_ids.contains(&second.id()));
        assert!(registry.connectors.contains_key(&second.id()));
        registry.end_candidate();

        // The first Connector has no deferred Source operation; explicit
        // removal releases its retained membership exactly once. The second
        // Connector remains owned until its deferred cleanup succeeds.
        registry.remove_connector(first.id());
        assert!(first_source.dispose().is_ok());
        registry.begin_projection_candidate();
        let retry = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&retry);
        assert!(!registry.commit_prepared(&retry).unwrap());
        registry.end_candidate();
        assert_eq!(second_source.subscriber_count(), 0);
        registry.remove_connector(second.id());
        assert!(registry.connectors.is_empty());
        second_source.dispose().unwrap();
    }

    #[test]
    fn prepared_content_commit_preserves_newer_requested_selection() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"old\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let first = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let second = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.visible_mounted = true;
            state.desired_connector = Some(first.id());
            state.visible_connector = Some(first.id());
        }
        registry
            .connectors
            .get(&first.id())
            .unwrap()
            .lock()
            .unwrap()
            .requested = true;
        registry
            .connectors
            .get(&first.id())
            .unwrap()
            .lock()
            .unwrap()
            .visible = true;

        registry.begin_projection_candidate();
        registry
            .prepare_connector_projection(first.id(), 20)
            .unwrap();
        registry.candidate_binding_changes.insert(port.id());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);
        #[cfg(feature = "perf-counters")]
        let prepared_records =
            crate::perf::snapshot().value(crate::perf::Counter::ContentCandidateRecordsPrepared);

        // A newer request arrives while the old candidate is in flight. The
        // plan owns the old binding and must not be consumed or rewritten.
        assert!(
            registry
                .request_activation(second.id(), &Weak::new())
                .unwrap()
        );
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();

        let port_state = port.record.lock().unwrap();
        assert_eq!(port_state.visible_connector, Some(first.id()));
        assert_eq!(port_state.desired_connector, Some(second.id()));
        drop(port_state);
        assert!(
            registry
                .connectors
                .get(&first.id())
                .unwrap()
                .lock()
                .unwrap()
                .visible
        );
        assert!(
            registry
                .connectors
                .get(&second.id())
                .unwrap()
                .lock()
                .unwrap()
                .requested
        );
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::ContentCandidateRecordsPrepared),
                prepared_records,
                "newer desired work must not expand the delayed receipt's candidate plan"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentRegistryPortScans),
                0,
                "delayed content commit must not scan the Port registry"
            );
        }
    }

    #[test]
    fn prepared_many_records_preserve_newer_pending_binding_without_receipt_growth() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        const CONNECTOR_COUNT: usize = 128;
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"shared", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let mut ports = Vec::with_capacity(CONNECTOR_COUNT);
        let mut connectors = Vec::with_capacity(CONNECTOR_COUNT);
        for _ in 0..CONNECTOR_COUNT {
            let port = registry
                .create_port(Weak::new(), ContentFamily::Text)
                .unwrap();
            let connector = registry
                .connect(
                    &port.record,
                    &source,
                    HostContentFunnel::plain(TextWrapMode::Word),
                )
                .unwrap();
            {
                let mut port_state = port.record.lock().unwrap();
                port_state.desired_mounted = true;
                port_state.visible_mounted = true;
                port_state.desired_connector = Some(connector.id());
                port_state.visible_connector = Some(connector.id());
            }
            {
                let record = registry.connectors.get(&connector.id()).unwrap();
                let mut state = record.lock().unwrap();
                state.requested = true;
                state.visible = true;
            }
            ports.push(port);
            connectors.push(connector);
        }

        registry.begin_projection_candidate();
        for connector in &connectors {
            registry
                .prepare_connector_projection(connector.id(), 20)
                .unwrap();
        }
        for port in &ports {
            registry.candidate_binding_changes.insert(port.id());
        }
        let plan = registry.prepare_content_commit().unwrap();
        assert_eq!(plan.ports.len(), CONNECTOR_COUNT);
        assert_eq!(plan.connectors.len(), CONNECTOR_COUNT);
        assert_eq!(
            plan.sources.len(),
            1,
            "all connectors share one Source lock entry"
        );
        let plan_capacities = (
            plan.ports.capacity(),
            plan.connectors.capacity(),
            plan.sources.capacity(),
            plan.binding_changes.capacity(),
        );
        #[cfg(feature = "perf-counters")]
        let prepared_records =
            crate::perf::snapshot().value(crate::perf::Counter::ContentCandidateRecordsPrepared);
        registry.begin_prepared_candidate(&plan);

        // Accept newer control work while this large plan is held. The old
        // receipt must keep its captured association, while the newer pending
        // revision remains available for the next candidate without expanding
        // the old plan.
        let first_port_id = ports[0].id();
        registry.request_deactivation(connectors[0].id()).unwrap();
        registry.commit_prepared(&plan).unwrap();
        registry.end_candidate();
        let first_state = ports[0].record.lock().unwrap();
        assert_eq!(first_state.desired_connector, None);
        assert_eq!(first_state.visible_connector, Some(connectors[0].id()));
        drop(first_state);
        assert!(registry.pending_binding_changes.contains(&first_port_id));
        assert_eq!(
            (
                plan.ports.capacity(),
                plan.connectors.capacity(),
                plan.sources.capacity(),
                plan.binding_changes.capacity(),
            ),
            plan_capacities,
            "receipt commit must not grow candidate-owned record tables"
        );
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::ContentCandidateRecordsPrepared),
                prepared_records,
                "newer pending work must not grow the captured large plan: {counters:?}"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentRegistryPortScans),
                0,
                "receipt commit must not scan the registry: {counters:?}"
            );
        }
    }

    #[test]
    fn prepared_content_commit_rejects_a_poisoned_record_before_swapping() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"poison\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let connector_id = connector.id();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        registry.begin_projection_candidate();
        registry
            .prepare_connector_projection(connector.id(), 20)
            .unwrap();
        registry.candidate_binding_changes.insert(port.id());
        let plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&plan);
        let record = registry.connectors.get(&connector.id()).unwrap().clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = record.lock().unwrap();
            panic!("intentional connector poison");
        }));
        let error = registry.commit_prepared(&plan).unwrap_err();
        assert!(error.to_string().contains("Connector lock is poisoned"));
        assert_eq!(
            port.record.lock().unwrap().visible_connector,
            Some(connector_id),
            "poison validation must happen before visible association swaps"
        );
    }

    #[test]
    fn prepared_content_commit_preserves_a_newer_delivery_tick() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"a long enough stream for delivery pacing\n", &[], &[])
            .unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Commit an initial candidate so the delayed receipt below exercises
        // a real F0 < F1 < F2 progression rather than relying on ZERO as an
        // uninitialized sentinel.
        registry.begin_projection_candidate();
        registry
            .prepare_connector_projection(connector.id(), 32)
            .unwrap();
        registry.candidate_binding_changes.insert(port.id());
        let initial = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        assert!(initial > StreamOffset::ZERO);
        let initial_plan = registry.prepare_content_commit().unwrap();
        registry.begin_prepared_candidate(&initial_plan);
        registry.commit_prepared(&initial_plan).unwrap();
        registry.end_candidate();
        assert_eq!(
            registry
                .connector_delivery_frontier(connector.id())
                .unwrap(),
            initial
        );

        // Prepare F1, then hold its receipt while a newer delivery tick
        // advances the mutable Connector state to F2 and clears its current
        // candidate projection. The prepared plan must retain the old Arc.
        registry.begin_projection_candidate();
        let deadline = registry
            .next_wakeup()
            .expect("smooth content must have a deadline");
        let _ = registry
            .advance(deadline + std::time::Duration::from_millis(16))
            .unwrap();
        let first_newer = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        assert!(first_newer > initial);
        registry
            .prepare_connector_projection(connector.id(), 32)
            .unwrap();
        registry.candidate_binding_changes.insert(port.id());
        let delayed_plan = registry.prepare_content_commit().unwrap();
        let captured_identity = delayed_plan
            .connectors
            .iter()
            .find(|prepared| prepared.id == connector.id())
            .and_then(|prepared| prepared.candidate_projection.as_ref())
            .map(|projection| projection.identity)
            .expect("delayed plan must own F1 projection");
        registry.begin_prepared_candidate(&delayed_plan);
        let next_deadline = registry
            .next_wakeup()
            .expect("newer smooth content must have a deadline");
        let _ = registry
            .advance(next_deadline + std::time::Duration::from_millis(16))
            .unwrap();
        let newer = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        assert!(newer > first_newer);

        // A Source replacement arrives before the old receipt. It must not
        // rewrite the old plan or make its F1 frontier disappear.
        let before_replace = source.stats().unwrap().revision;
        source
            .replace_utf8(b"replacement source payload\n", &[], &[])
            .unwrap();
        assert!(source.stats().unwrap().revision > before_replace);
        // Consume the replacement wake and prepare its newer product while
        // the old receipt is still outstanding. Its source generation and
        // delivery coordinate may reset independently of F1; the old commit
        // must leave this exact newer product/deadline untouched.
        registry
            .sync_connector_deadline(
                connector.id(),
                Some(next_deadline + std::time::Duration::from_millis(32)),
            )
            .unwrap();
        registry
            .prepare_connector_projection(connector.id(), 32)
            .unwrap();
        let replacement_state = registry
            .connectors
            .get(&connector.id())
            .unwrap()
            .lock()
            .unwrap();
        let replacement_identity = replacement_state
            .candidate_projection
            .as_ref()
            .map(|projection| projection.identity)
            .expect("replacement wake must install a newer product");
        let replacement_frontier = replacement_state.candidate_delivery_frontier;
        let replacement_deadline = registry.active_deadlines.get(&connector.id()).copied();
        drop(replacement_state);
        assert_ne!(replacement_identity, captured_identity);

        registry.commit_prepared(&delayed_plan).unwrap();
        registry.end_candidate();
        assert!(newer > initial);
        assert_eq!(
            registry
                .connector_delivery_frontier(connector.id())
                .unwrap(),
            first_newer,
            "an older receipt must publish exactly its captured F1 frontier"
        );
        assert_eq!(
            registry
                .connector_candidate_delivery_frontier(connector.id())
                .unwrap(),
            newer,
            "newer F2 delivery progress must remain pending"
        );
        let after_replacement = registry
            .connectors
            .get(&connector.id())
            .unwrap()
            .lock()
            .unwrap();
        assert_eq!(
            after_replacement
                .candidate_projection
                .as_ref()
                .map(|projection| projection.identity),
            Some(replacement_identity),
            "older receipt must not consume the replacement candidate"
        );
        assert_eq!(
            after_replacement.candidate_delivery_frontier, replacement_frontier,
            "replacement coordinate must not be compared to old-source F1"
        );
        assert_eq!(
            registry.active_deadlines.get(&connector.id()).copied(),
            replacement_deadline,
            "newer replacement deadline must remain pending"
        );
        drop(after_replacement);
        assert_eq!(
            registry
                .connectors
                .get(&connector.id())
                .unwrap()
                .lock()
                .unwrap()
                .committed_projection
                .as_ref()
                .map(|projection| projection.identity),
            Some(captured_identity),
            "receipt must retain the candidate-owned F1 projection"
        );
    }

    #[test]
    fn history_binding_requires_prefix_product_on_a_warm_projection() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(
                b"# Header\n\nParagraph line 1\nParagraph line 2\n\nMore text\n",
                &[],
                &[],
            )
            .unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.visible_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Warm an ordinary open projection without any History binding.
        registry.begin_projection_candidate();
        let ordinary =
            registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        assert!(ordinary.intrinsic_size.height > 0);
        registry.promote_candidate_projection(connector.id());
        assert!(
            registry
                .connectors
                .get(&connector.id())
                .unwrap()
                .lock()
                .unwrap()
                .committed_projection
                .as_ref()
                .and_then(|projection| projection.finalized_prefix.as_ref())
                .is_none()
        );

        // Binding/rebinding the same warm Connector must change the cache key
        // and prepare the existing finalized-prefix policy without new Source
        // bytes or a whole registry/cache flush.
        registry
            .set_history_unit(port.id(), 1, crate::Insets::ZERO)
            .unwrap();
        registry.begin_projection_candidate();
        let history = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        assert!(history.intrinsic_size.height > 0);
        let candidate = registry
            .connectors
            .get(&connector.id())
            .unwrap()
            .lock()
            .unwrap()
            .candidate_projection
            .as_ref()
            .cloned()
            .expect("History-bound warm Connector must prepare a candidate");
        assert!(candidate.key.needs_finalized_prefix);
        assert!(candidate.finalized_prefix.is_some());
        registry.promote_candidate_projection(connector.id());
        let first_rows = registry.history_rows(port.id(), 20).expect("History rows");
        assert!(first_rows.complete || !first_rows.rows.is_empty());

        registry
            .set_history_unit(port.id(), 2, crate::Insets::ZERO)
            .unwrap();
        registry.begin_projection_candidate();
        registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let rebound = registry
            .connectors
            .get(&connector.id())
            .unwrap()
            .lock()
            .unwrap()
            .candidate_projection
            .as_ref()
            .cloned()
            .expect("rebound History unit must retain prefix policy");
        assert!(rebound.key.needs_finalized_prefix);
        assert!(rebound.finalized_prefix.is_some());
    }

    #[test]
    fn history_rows_complete_is_true_when_payload_reaches_sealed_end() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"one\ntwo\nthree\n", &[], &[]).unwrap();
        source.seal().unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        registry
            .set_history_unit(port.id(), 1, crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let _ = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let rows = registry.history_rows(port.id(), 20).unwrap();
        assert!(!rows.rows.is_empty(), "rows should contain lines");
        assert!(
            rows.complete,
            "rows.complete MUST be true when returning all rows of sealed unit"
        );
    }

    #[test]
    fn history_rows_leading_padding_is_preserved_for_open_streams() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"line 1\nline 2\n", &[], &[]).unwrap();
        // note: NOT sealed!
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        registry
            .set_history_unit(port.id(), 1, crate::Insets::new(2, 0, 0, 0))
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let _ = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let rows = registry.history_rows(port.id(), 20).unwrap();
        assert_eq!(
            rows.leading_padding, 2,
            "open stream must report leading padding so history_view can strip top padding"
        );
    }

    #[test]
    fn history_unit_retirement_disposes_and_removes_internal_port() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        registry
            .set_history_unit(port_id, 42, crate::Insets::ZERO)
            .unwrap();
        assert!(registry.ports.contains_key(&port_id));
        registry.history_unit_retired(42);
        assert!(
            !registry.ports.contains_key(&port_id),
            "retired History port must be removed from ContentHostRegistry to prevent memory leaks"
        );
        assert!(!registry.connectors.contains_key(&connector.id()));
    }

    #[test]
    fn theme_change_invalidates_content_measurement_projection_revision() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"hello world\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        registry
            .set_history_unit(port.id(), 1, crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let t1 = Theme::new().with_color("accent", ThemeColor::Indexed(1));
        let t2 = Theme::new().with_color("accent", ThemeColor::Indexed(2));
        registry.set_theme(&Arc::new(t1));
        let m1 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let first_layout = registry
            .connectors
            .get(&connector.id())
            .and_then(|record| record.lock().ok())
            .and_then(|state| {
                state
                    .prepared_paint_cache
                    .iter()
                    .find(|(key, _)| key.width == 20)
                    .map(|(_, product)| Arc::clone(&product.layout))
            })
            .expect("first measurement must retain a layout product");
        let key1 = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        registry.set_theme(&Arc::new(t2));
        let m2 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let second_layout = registry
            .connectors
            .get(&connector.id())
            .and_then(|record| record.lock().ok())
            .and_then(|state| {
                state
                    .prepared_paint_cache
                    .iter()
                    .find(|(key, _)| {
                        key.width == 20 && key.theme_revision == registry.theme_revision
                    })
                    .map(|(_, product)| Arc::clone(&product.layout))
            })
            .expect("recolor must retain a replacement paint product");
        assert!(
            Arc::ptr_eq(&first_layout, &second_layout),
            "theme-only repaint must reuse width-dependent layout geometry"
        );
        let key2 = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        assert_ne!(key1, key2, "TextProjectionKey must differ across themes");
        assert_ne!(
            m1.projection_revision, m2.projection_revision,
            "ContentMeasurement projection_revision must change across themes to invalidate paint cache"
        );
        assert_eq!(
            m1.metric_revision, m2.metric_revision,
            "palette-only changes must preserve the metric revision"
        );
        assert_ne!(
            m1.paint_revision, m2.paint_revision,
            "palette-only changes must advance the paint revision"
        );
    }

    #[test]
    fn prepared_paint_cache_separates_immediate_and_smooth_row_demands() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"shared cache rows must stay visible\n", &[], &[])
            .unwrap();
        let snapshot = source.snapshot().unwrap();
        let immediate = HostContentFunnel::plain(TextWrapMode::Word);
        let smooth = HostContentFunnel::new(
            TextFunnelKind::Plain,
            TextWrapMode::Word,
            true,
            ContentDelivery::Smooth(SmoothConfig::default()),
        );
        let theme = Arc::new(Theme::new());

        let mut semantic_cache = SemanticProjectionCache::new();
        let mut prefix_cache = PrefixProofCache::new();
        let mut prepared_cache = PreparedPaintCache::new();
        let mut immediate_execution = ConnectorExecution::new(&immediate);
        let immediate_projection = project_text_snapshot(
            &snapshot,
            immediate,
            40,
            false,
            &theme,
            0,
            &mut immediate_execution,
            0,
            &mut semantic_cache,
            &mut prefix_cache,
            &mut prepared_cache,
        )
        .unwrap();
        assert!(
            immediate_projection.rows.is_none(),
            "immediate non-History projections may defer physical rows"
        );

        let mut smooth_execution = ConnectorExecution::new(&smooth);
        let smooth_projection = project_text_snapshot(
            &snapshot,
            smooth,
            40,
            false,
            &theme,
            0,
            &mut smooth_execution,
            0,
            &mut semantic_cache,
            &mut prefix_cache,
            &mut prepared_cache,
        )
        .unwrap();
        assert!(
            smooth_projection.rows.is_some(),
            "smooth projections require visibility rows"
        );

        let mut reverse_semantic_cache = SemanticProjectionCache::new();
        let mut reverse_prefix_cache = PrefixProofCache::new();
        let mut reverse_prepared_cache = PreparedPaintCache::new();
        let mut smooth_first = ConnectorExecution::new(&smooth);
        let smooth_first_projection = project_text_snapshot(
            &snapshot,
            smooth,
            40,
            false,
            &theme,
            0,
            &mut smooth_first,
            0,
            &mut reverse_semantic_cache,
            &mut reverse_prefix_cache,
            &mut reverse_prepared_cache,
        )
        .unwrap();
        assert!(smooth_first_projection.rows.is_some());
        let mut immediate_after_smooth = ConnectorExecution::new(&immediate);
        let immediate_after_smooth_projection = project_text_snapshot(
            &snapshot,
            immediate,
            40,
            false,
            &theme,
            0,
            &mut immediate_after_smooth,
            0,
            &mut reverse_semantic_cache,
            &mut reverse_prefix_cache,
            &mut reverse_prepared_cache,
        )
        .unwrap();
        assert!(immediate_after_smooth_projection.rows.is_none());
    }

    #[test]
    fn deferred_ticket_paints_with_its_captured_theme_after_recolor() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"captured palette\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_mounted = true;
            port_state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        let theme_a = Arc::new(Theme::new().with_style(
            crate::content::text::TEXT_THEME_KEY,
            StyleSpec::new().foreground(ColorSpec::ansi(1)),
        ));
        let theme_b = Arc::new(Theme::new().with_style(
            crate::content::text::TEXT_THEME_KEY,
            StyleSpec::new().foreground(ColorSpec::ansi(4)),
        ));
        registry.set_theme(&theme_a);
        let measurement_a =
            registry.measure_content(port.id(), 40, crate::presentation::WidthRule::Fill);
        let old_projection = registry
            .connectors
            .get(&connector.id())
            .and_then(|record| record.lock().ok())
            .and_then(|state| state.candidate_projection.clone())
            .expect("initial measurement must prepare a projection");
        let old_ticket = PreparedProjectionTicket {
            port_id: port.id(),
            connector_id: Some(connector.id()),
            offered_width: 40,
            projection_revision: old_projection.key.revision(),
            projection_identity: old_projection.identity,
        };
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            record.lock().unwrap().committed_projection = Some(old_projection);
        }

        registry.set_theme(&theme_b);
        let _measurement_b =
            registry.measure_content(port.id(), 40, crate::presentation::WidthRule::Fill);
        let mut target = Surface::new(40, 1);
        registry.paint_window_direct(
            old_ticket,
            ContentWindow {
                first_row: 0,
                row_count: 1,
            },
            &mut target,
            (0, 0),
            crate::geometry::Rect::new(0, 0, 40, 1),
            crate::physical::PhysicalStyle::default(),
        );
        assert_eq!(
            target.get(0, 0).style.foreground,
            Some(crate::physical::PhysicalColor::Indexed(1)),
            "an old prepared ticket must retain theme A after host recolors to B"
        );
        assert!(measurement_a.intrinsic_size.height >= 1);
    }

    #[test]
    fn theme_recolor_repaints_without_reparsing_semantic_content() {
        use crate::TextFunnelKind;
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"# Title\n\nsome *emphasis* text\n", &[], &[])
            .unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        registry
            .set_history_unit(port.id(), 1, crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let rebuilds =
            || crate::perf::snapshot().value(crate::perf::Counter::SemanticProjectionRebuilds);
        let t1 = Theme::new().with_color("accent", ThemeColor::Indexed(1));
        let t2 = Theme::new().with_color("accent", ThemeColor::Indexed(2));
        registry.set_theme(&Arc::new(t1));
        crate::presentation::paint::reset_text_geometry_builds();
        let before = rebuilds();
        let m1 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let geometry_after_first = crate::presentation::paint::text_geometry_builds();
        assert!(
            geometry_after_first > 0,
            "first content preparation must build text row geometry"
        );
        if cfg!(feature = "perf-counters") {
            assert!(
                rebuilds() > before,
                "first projection must run the semantic parsers"
            );
        }
        // A palette-only recolor invalidates the painted surface but must
        // reuse the cached semantic IR: no parser runs again.
        registry.set_theme(&Arc::new(t2));
        let after_recolor = rebuilds();
        let m2 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        assert_eq!(
            crate::presentation::paint::text_geometry_builds(),
            geometry_after_first,
            "theme-only repaint must reuse cached text wrapping geometry"
        );
        if cfg!(feature = "perf-counters") {
            assert_eq!(
                rebuilds(),
                after_recolor,
                "theme recolor must not reparse semantic content"
            );
        }
        assert_ne!(
            m1.projection_revision, m2.projection_revision,
            "recolor must still invalidate the paint cache"
        );
        // Resizing the viewport changes layout width but must reuse cached semantic IR:
        // no parser runs again.
        let _m3 = registry.measure_content(port.id(), 40, crate::presentation::WidthRule::Fill);
        if cfg!(feature = "perf-counters") {
            assert_eq!(
                rebuilds(),
                after_recolor,
                "viewport resize must not reparse semantic content"
            );
        }
        // New source bytes change the semantic key and rebuild exactly.
        source.append_utf8(b"more text\n", &[], &[]).unwrap();
        registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        if cfg!(feature = "perf-counters") {
            assert!(
                rebuilds() > after_recolor,
                "source changes must rebuild semantic content"
            );
        }
    }

    #[derive(Default)]
    struct LocalSink {
        rows: Vec<crate::physical::PhysicalRow>,
    }

    impl crate::backend::NativeHistorySink for LocalSink {
        type Error = ();
        fn insert_history_rows(
            &mut self,
            rows: &[crate::physical::PhysicalRow],
        ) -> Result<usize, Self::Error> {
            self.rows.extend(rows.iter().cloned());
            Ok(rows.len())
        }
    }

    #[derive(Debug)]
    struct BudgetSink {
        budgets: VecDeque<usize>,
        rows: Vec<crate::physical::PhysicalRow>,
    }

    impl BudgetSink {
        fn new(budgets: impl IntoIterator<Item = usize>) -> Self {
            Self {
                budgets: budgets.into_iter().collect(),
                rows: Vec::new(),
            }
        }
    }

    impl crate::backend::NativeHistorySink for BudgetSink {
        type Error = ();

        fn insert_history_rows(
            &mut self,
            rows: &[crate::physical::PhysicalRow],
        ) -> Result<usize, Self::Error> {
            let accepted = self
                .budgets
                .pop_front()
                .unwrap_or(rows.len())
                .min(rows.len());
            self.rows.extend(rows[..accepted].iter().cloned());
            Ok(accepted)
        }
    }

    struct PartialErrorSink {
        rows: Vec<PhysicalRow>,
        fail: bool,
    }

    impl crate::backend::NativeHistorySink for PartialErrorSink {
        type Error = &'static str;

        fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
            if self.fail {
                let accepted = rows.len().min(1);
                self.rows.extend(rows[..accepted].iter().cloned());
                return Err("simulated partial native write");
            }
            self.rows.extend(rows.iter().cloned());
            Ok(rows.len())
        }
    }

    struct SuccessThenErrorSink {
        rows: Vec<PhysicalRow>,
        calls: usize,
    }

    impl crate::backend::NativeHistorySink for SuccessThenErrorSink {
        type Error = &'static str;

        fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
            self.calls += 1;
            if self.calls == 1 {
                let accepted = rows.len().min(1);
                self.rows.extend(rows[..accepted].iter().cloned());
                return Ok(accepted);
            }
            if let Some(row) = rows.first() {
                self.rows.push(row.clone());
            }
            Err("simulated later native write failure")
        }
    }

    #[test]
    fn native_sink_failure_marks_synchronization_unknown_without_rewinding_history() {
        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("one\ntwo"))
            .unwrap();
        let mut sink = PartialErrorSink {
            rows: Vec::new(),
            fail: true,
        };
        let error = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            8,
            &Theme::new(),
            &mut crate::presentation::EmptyContentProvider,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::history::NativeTransferError::Sink("simulated partial native write")
        ));
        assert!(history.native_synchronization_unknown());
        let rows_after_failure = sink.rows.len();
        let retry = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            8,
            &Theme::new(),
            &mut crate::presentation::EmptyContentProvider,
        )
        .unwrap_err();
        assert!(matches!(
            retry,
            crate::history::NativeTransferError::SynchronizationUnknown
        ));
        assert_eq!(sink.rows.len(), rows_after_failure);
        history.recover_native_synchronization();
        sink.fail = false;
        let _ = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            8,
            &Theme::new(),
            &mut crate::presentation::EmptyContentProvider,
        )
        .unwrap();
    }

    #[test]
    fn history_marker_preserves_prior_acknowledged_rows_after_later_failure() {
        let mut history = crate::History::new();
        history
            .push(crate::presentation::factory::text("one\ntwo\nthree"))
            .unwrap();
        let mut sink = SuccessThenErrorSink {
            rows: Vec::new(),
            calls: 0,
        };
        let first = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            1,
            &Theme::new(),
            &mut crate::presentation::EmptyContentProvider,
        )
        .unwrap();
        assert_eq!(first.inserted, 1);
        assert_eq!(history.physical_rows_inserted(), 1);
        let _ = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            1,
            &Theme::new(),
            &mut crate::presentation::EmptyContentProvider,
        )
        .unwrap_err();
        assert!(history.native_synchronization_unknown());
        assert_eq!(
            history.physical_rows_inserted(),
            1,
            "a later failed receipt must not rewind or overcount the earlier acknowledgement"
        );
    }

    fn sealed_markdown_history_rows(text: &str, width: u16) -> Vec<PhysicalRow> {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(text.as_bytes(), &[], &[]).unwrap();
        source.seal().unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        registry
            .set_history_unit(port.id(), 1, crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        registry.measure_content(port.id(), width, crate::presentation::WidthRule::Fill);
        registry
            .history_rows(port.id(), width)
            .map_or_else(Vec::new, |rows| rows.rows)
    }

    #[test]
    fn finalized_prefix_rows_match_sealed_baseline_at_each_append_and_partial_receipt() {
        // The open projection and the sealed prefix intentionally use
        // different Markdown parser states.  Compare the complete physical
        // row products (including styles and wide-cell geometry), not merely
        // whether either side is empty.  One-row acknowledgements and a zero
        // receipt then prove that History transfers this exact product once.
        let fixtures = [
            "~~~rust\nlet value = 1;\n~~~\nopen paragraph",
            "1. first\n2. second\n3. third\nopen paragraph",
            "Title\n=====\nopen paragraph",
            "[label][ref]\n\n[ref]: https://example.test\nopen paragraph",
            "| left | right |\n| --- | --- |\n| a | b |\nopen paragraph",
            "open paragraph without a trailing newline",
        ];
        let width = 40;
        let mut exercised_partial_receipt = false;

        for fixture in fixtures {
            let source_registry = ContentSourceRegistry::new();
            let source = source_registry.create(TextSourceKind::Stream).unwrap();
            let mut registry = ContentHostRegistry::new(source_registry);
            let port = registry
                .create_port(Weak::new(), ContentFamily::Text)
                .unwrap();
            let port_id = port.id();
            let connector = registry
                .connect(
                    &port.record,
                    &source,
                    HostContentFunnel::new(
                        TextFunnelKind::Markdown,
                        TextWrapMode::Word,
                        true,
                        ContentDelivery::Immediate,
                    ),
                )
                .unwrap();
            let view = vf::content_host(port_id).unwrap();
            let mut history = crate::History::new();
            let unit_id = history.push(view).unwrap();
            registry
                .set_history_unit(port_id, unit_id.value(), crate::Insets::ZERO)
                .unwrap();
            {
                let mut state = port.record.lock().unwrap();
                state.desired_mounted = true;
                state.desired_connector = Some(connector.id());
                state.visible_mounted = true;
                state.visible_connector = Some(connector.id());
            }
            {
                let record = registry.connectors.get(&connector.id()).unwrap();
                let mut state = record.lock().unwrap();
                state.requested = true;
                state.visible = true;
            }

            let bytes = fixture.as_bytes();
            let mut cursor = 0;
            let mut final_rows = Vec::new();
            for (next, _) in fixture.char_indices().skip(1) {
                source.append_utf8(&bytes[cursor..next], &[], &[]).unwrap();
                cursor = next;
                registry.measure_content(port_id, width, crate::presentation::WidthRule::Fill);
                let actual = registry.history_rows(port_id, width).unwrap();
                let prefix = source.snapshot().unwrap().stable_prefix();
                let expected = prefix
                    .as_ref()
                    .map(|prefix| sealed_markdown_history_rows(&prefix.text(), width))
                    .unwrap_or_default();
                assert_eq!(
                    actual.rows, expected,
                    "finalized rows diverged for fixture {fixture:?} at byte prefix {cursor}"
                );
                final_rows = actual.rows;
            }
            if cursor < bytes.len() {
                source.append_utf8(&bytes[cursor..], &[], &[]).unwrap();
                registry.measure_content(port_id, width, crate::presentation::WidthRule::Fill);
                let actual = registry.history_rows(port_id, width).unwrap();
                let prefix = source.snapshot().unwrap().stable_prefix();
                let expected = prefix
                    .as_ref()
                    .map(|prefix| sealed_markdown_history_rows(&prefix.text(), width))
                    .unwrap_or_default();
                assert_eq!(
                    actual.rows, expected,
                    "final append mismatch for {fixture:?}"
                );
                final_rows = actual.rows;
            }

            if !final_rows.is_empty() {
                let mut sink = BudgetSink::new([1, 0, usize::MAX]);
                let first = crate::history::transfer_native_prefix_with_theme_and_content(
                    &mut history,
                    &mut sink,
                    width,
                    usize::MAX,
                    &Theme::new(),
                    &mut registry,
                )
                .unwrap();
                assert_eq!(first.inserted, 1, "first receipt must accept one row");
                let blocked = crate::history::transfer_native_prefix_with_theme_and_content(
                    &mut history,
                    &mut sink,
                    width,
                    usize::MAX,
                    &Theme::new(),
                    &mut registry,
                )
                .unwrap();
                assert_eq!(blocked.inserted, 0, "zero receipt must not advance History");
                assert_eq!(
                    registry.history_adapter.committed_rows(port_id),
                    1,
                    "zero receipt must not advance the committed row frontier"
                );
                let _ = crate::history::transfer_native_prefix_with_theme_and_content(
                    &mut history,
                    &mut sink,
                    width,
                    usize::MAX,
                    &Theme::new(),
                    &mut registry,
                )
                .unwrap();
                assert_eq!(
                    sink.rows, final_rows,
                    "partial receipt duplicated/lost rows"
                );
                exercised_partial_receipt = true;
            }
        }
        assert!(
            exercised_partial_receipt,
            "fixtures must expose a stable row"
        );
    }

    #[test]
    fn history_partial_and_zero_receipts_preserve_frozen_rows_across_resize() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"first\nsecond\nthird\n", &[], &[])
            .unwrap();
        source.seal().unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let view = vf::content_host(port_id).unwrap();
        let mut history = crate::History::new();
        let unit_id = history.push(view).unwrap();
        registry
            .set_history_unit(port_id, unit_id.value(), crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        registry.measure_content(port_id, 12, crate::presentation::WidthRule::Fill);

        // Accept one content row, then a zero receipt must not advance any
        // content/frontier counters or manufacture a new remainder.
        let mut sink = BudgetSink::new([1, 0, 1, 1]);
        let theme = crate::Theme::new();
        let first = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            12,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert_eq!(first.inserted, 1);
        assert_eq!(registry.history_adapter.committed_rows(port_id), 1);
        let blocked = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            12,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert_eq!(blocked.inserted, 0);
        assert!(matches!(
            blocked.status,
            crate::history::NativeTransferStatus::SinkBlocked
        ));
        assert_eq!(registry.history_adapter.committed_rows(port_id), 1);

        // The remainder is frozen from this exact width; changing width
        // cannot regenerate or duplicate the already acknowledged prefix.
        let before_resize = sink.rows.clone();
        let third = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert_eq!(third.inserted, 1);
        assert_eq!(registry.history_adapter.committed_rows(port_id), 2);
        assert_eq!(sink.rows.len(), before_resize.len() + 1);

        while !history.is_empty() {
            crate::history::transfer_native_prefix_with_theme_and_content(
                &mut history,
                &mut sink,
                20,
                10,
                &theme,
                &mut registry,
            )
            .unwrap();
        }
        assert!(!sink.rows.is_empty());
        assert!(!registry.ports.contains_key(&port_id));
    }

    #[test]
    fn prepared_content_ticket_never_selects_a_newer_same_width_projection() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"first\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        {
            let mut port_state = port.record.lock().unwrap();
            port_state.desired_mounted = true;
            port_state.desired_connector = Some(connector.id());
            port_state.visible_mounted = true;
            port_state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        let first = registry
            .prepare_connector_projection(connector.id(), 20)
            .unwrap();
        let ticket = PreparedProjectionTicket {
            port_id: port.id(),
            connector_id: first.connector_id,
            offered_width: 20,
            projection_revision: first.projection_revision,
            projection_identity: first.projection_identity,
        };

        source.append_utf8(b"second\n", &[], &[]).unwrap();
        let second = registry
            .prepare_connector_projection(connector.id(), 20)
            .unwrap();
        assert_ne!(first.projection_identity, second.projection_identity);
        let selected = registry
            .projection_for_ticket(ticket)
            .expect("the prepared first product remains pinned in cache");
        assert_eq!(selected.identity, first.projection_identity);
    }

    #[test]
    fn candidate_reuses_one_source_snapshot_for_revision_queries() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"before\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();

        registry.begin_projection_candidate();
        let before = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        source.append_utf8(b"after\n", &[], &[]).unwrap();
        let during = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        assert_eq!(before.source_revision, during.source_revision);
        assert_eq!(
            registry.candidate_source_snapshots.borrow().len(),
            1,
            "shared Source captures are keyed once per candidate"
        );
        registry.abort_candidate();
    }

    #[test]
    fn history_content_transfer_retires_unit_and_strips_padding() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(b"line 1\nline 2\n", &[], &[]).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let mut history = crate::History::new();
        let view = crate::presentation::factory::padding(
            vf::content_host(port_id).unwrap(),
            crate::Insets::new(2, 0, 1, 0),
        );
        let unit_id = history.push(view.clone()).unwrap();
        registry
            .set_history_unit(port_id, unit_id.value(), crate::Insets::new(2, 0, 1, 0))
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }
        let _ = registry.measure_content(port_id, 20, crate::presentation::WidthRule::Fill);

        // While open: transfer stable prefix
        let mut sink = LocalSink::default();
        let theme = crate::Theme::new();
        let outcome = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(outcome.inserted > 0);
        // Verify resident view has top padding stripped:
        let resident_view = registry.history_view(&view);
        assert_eq!(resident_view.decoration().padding.top, 0);

        // Now seal the stream
        source.seal().unwrap();
        let _ = registry.measure_content(port_id, 20, crate::presentation::WidthRule::Fill);
        let outcome = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            20,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            matches!(
                outcome.status,
                crate::history::NativeTransferStatus::Progress
            ) || outcome.inserted > 0
        );
        assert!(
            history.is_empty(),
            "sealed History unit must be retired immediately upon transferring its final rows"
        );
        assert!(
            !registry.ports.contains_key(&port_id),
            "retired History unit port must be cleaned up from ContentHostRegistry"
        );
    }

    #[test]
    fn history_rows_transfer_unsealed_markdown_stream_with_smoothing() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(
                b"# Header\n\nParagraph line 1\nParagraph line 2\n\nMore text\n",
                &[],
                &[],
            )
            .unwrap();

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        let mut history = crate::History::new();
        let view = vf::content_host(port_id).unwrap();
        let unit_id = history.push(view.clone()).unwrap();
        registry
            .set_history_unit(port_id, unit_id.value(), crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Seed smoother with initial measurement
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);

        // Advance smoother enough to reveal all rows
        let t0 = std::time::Instant::now();
        registry.advance(t0).unwrap();
        let t1 = t0 + std::time::Duration::from_secs(2);
        registry.advance(t1).unwrap();

        let measurement =
            registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        assert!(measurement.intrinsic_size.height > 0);

        // While open: transfer stable prefix
        let mut sink = LocalSink::default();
        let theme = crate::Theme::new();
        let outcome = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            40,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            outcome.inserted > 0,
            "open Markdown stream with completed paragraphs must have stable transferable rows"
        );

        // Now seal the stream and finish transfer
        source.seal().unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let outcome = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            40,
            20,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            matches!(
                outcome.status,
                crate::history::NativeTransferStatus::Progress
                    | crate::history::NativeTransferStatus::Idle
            ) || outcome.inserted > 0
        );
        assert!(
            history.is_empty(),
            "sealed Markdown History unit must be retired"
        );
    }

    #[test]
    fn smooth_history_matches_finalized_rows_through_ticks_and_receipts() {
        use std::time::Duration;

        let fixture = "# Header\n\nfirst line\nsecond line\n\nthird line\nopen tail";
        let width = 40;
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source.append_utf8(fixture.as_bytes(), &[], &[]).unwrap();
        let open_prefix = source
            .snapshot()
            .unwrap()
            .stable_prefix()
            .map(|prefix| prefix.text())
            .unwrap_or_default();
        let open_baseline = sealed_markdown_history_rows(&open_prefix, width);
        assert!(
            open_baseline.len() >= 2,
            "multirow fixture must expose a finalized baseline"
        );

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        let mut history = crate::History::new();
        let unit_id = history.push(vf::content_host(port_id).unwrap()).unwrap();
        registry
            .set_history_unit(port_id, unit_id.value(), crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        let mut now = Instant::now();
        let mut observed_rows = 0;
        for _ in 0..8 {
            registry.measure_content(port_id, width, crate::presentation::WidthRule::Fill);
            let rows = registry.history_rows(port_id, width).unwrap();
            let committed = registry.history_adapter.committed_content_rows(port_id);
            let end = committed.saturating_add(rows.rows.len());
            assert!(end <= open_baseline.len());
            assert_eq!(rows.rows, open_baseline[committed..end]);
            observed_rows = observed_rows.max(rows.rows.len());
            let Some(deadline) = registry.next_wakeup() else {
                break;
            };
            now = deadline + Duration::from_millis(250);
            registry.advance(now).unwrap();
        }
        assert!(
            observed_rows >= 2,
            "Smooth should expose multiple finalized rows incrementally"
        );

        let available = registry.history_rows(port_id, width).unwrap();
        let available_count = available.rows.len();
        let mut sink = BudgetSink::new([1, 0, usize::MAX]);
        let first = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            width,
            usize::MAX,
            &Theme::new(),
            &mut registry,
        )
        .unwrap();
        assert_eq!(first.inserted, 1);
        let committed_after_first = registry.history_adapter.committed_content_rows(port_id);
        assert_eq!(committed_after_first, 1);
        let blocked = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            width,
            usize::MAX,
            &Theme::new(),
            &mut registry,
        )
        .unwrap();
        assert_eq!(blocked.inserted, 0);
        assert_eq!(
            registry.history_adapter.committed_content_rows(port_id),
            committed_after_first
        );
        let _ = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            width,
            usize::MAX,
            &Theme::new(),
            &mut registry,
        )
        .unwrap();
        assert_eq!(
            sink.rows,
            open_baseline[..available_count.min(open_baseline.len())]
        );

        source.seal().unwrap();
        let sealed_baseline = sealed_markdown_history_rows(fixture, width);
        for _ in 0..8 {
            now += Duration::from_secs(1);
            registry.advance(now).unwrap();
            registry.measure_content(port_id, width, crate::presentation::WidthRule::Fill);
            let committed = registry.history_adapter.committed_content_rows(port_id);
            if let Some(rows) = registry.history_rows(port_id, width) {
                let end = committed.saturating_add(rows.rows.len());
                assert!(end <= sealed_baseline.len());
                assert_eq!(rows.rows, sealed_baseline[committed..end]);
                if !rows.rows.is_empty() {
                    crate::history::transfer_native_prefix_with_theme_and_content(
                        &mut history,
                        &mut sink,
                        width,
                        usize::MAX,
                        &Theme::new(),
                        &mut registry,
                    )
                    .unwrap();
                }
            }
            if history.is_empty() {
                break;
            }
        }
        assert!(
            history.is_empty(),
            "sealed Smooth History must eventually drain"
        );
        assert_eq!(sink.rows, sealed_baseline);
    }

    #[test]
    fn open_markdown_finalized_prefix_matches_current_policy() {
        for input in [
            "```rust\nlet value = 1;\n",
            "[label]\n\n[label]: https://example.test\n",
            "| left | right |\n| --- | --- |\n| a | b |\n",
            "- first\n\n- second\n",
        ] {
            let source_registry = ContentSourceRegistry::new();
            let source = source_registry.create(TextSourceKind::Stream).unwrap();
            source.append_utf8(input.as_bytes(), &[], &[]).unwrap();
            let mut registry = ContentHostRegistry::new(source_registry);
            let port = registry
                .create_port(Weak::new(), ContentFamily::Text)
                .unwrap();
            let connector = registry
                .connect(
                    &port.record,
                    &source,
                    HostContentFunnel::new(
                        TextFunnelKind::Markdown,
                        TextWrapMode::Word,
                        true,
                        ContentDelivery::Immediate,
                    ),
                )
                .unwrap();
            let unit_view = vf::content_host(port.id()).unwrap();
            let mut history = crate::History::new();
            let unit_id = history.push(unit_view).unwrap();
            registry
                .set_history_unit(port.id(), unit_id.value(), crate::Insets::ZERO)
                .unwrap();
            {
                let mut state = port.record.lock().unwrap();
                state.desired_mounted = true;
                state.desired_connector = Some(connector.id());
                state.visible_mounted = true;
                state.visible_connector = Some(connector.id());
            }
            {
                let record = registry.connectors.get(&connector.id()).unwrap();
                let mut state = record.lock().unwrap();
                state.requested = true;
                state.visible = true;
            }
            registry.measure_content(port.id(), 40, crate::presentation::WidthRule::Fill);
            let rows = registry.history_rows(port.id(), 40).unwrap();
            let expected_prefix_end = source
                .snapshot()
                .unwrap()
                .stable_prefix()
                .map(|prefix| prefix.source_end);
            assert_eq!(rows.complete, false, "open Source cannot complete History");
            assert_eq!(
                rows.rows.is_empty(),
                expected_prefix_end.is_none(),
                "History must follow the current finalized-prefix policy: {input:?}"
            );
        }
    }

    #[test]
    fn history_transfer_static_and_unsealed_markdown_stream_does_not_block() {
        let mut history = crate::History::new();
        // Unit 1: A completed/frozen static unit
        let static_view = vf::text("Command output: success");
        let _ = history.push(static_view).unwrap();

        // Unit 2: An unsealed Markdown text stream
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"Output line 1\nOutput line 2\n", &[], &[])
            .unwrap();

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    true,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        let stream_view = vf::content_host(port_id).unwrap();
        let unit_id = history.push(stream_view).unwrap();
        registry
            .set_history_unit(port_id, unit_id.value(), crate::Insets::ZERO)
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Advance smoother
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let t0 = std::time::Instant::now();
        registry.advance(t0).unwrap();
        let t1 = t0 + std::time::Duration::from_secs(2);
        registry.advance(t1).unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);

        let mut sink = LocalSink::default();
        let theme = crate::Theme::new();

        // First transfer: transfers the static unit.
        let outcome1 = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            40,
            1,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            outcome1.inserted > 0,
            "static unit must transfer to native scrollback"
        );
        assert_eq!(history.len(), 1, "static unit retired; stream unit remains");

        // Second transfer: transfers the stable rows of the unsealed stream!
        let outcome2 = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            40,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            outcome2.inserted > 0,
            "open stream stable prefix must transfer without blocking"
        );

        // Seal and complete
        source.seal().unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let outcome3 = crate::history::transfer_native_prefix_with_theme_and_content(
            &mut history,
            &mut sink,
            40,
            10,
            &theme,
            &mut registry,
        )
        .unwrap();
        assert!(
            matches!(
                outcome3.status,
                crate::history::NativeTransferStatus::Progress
                    | crate::history::NativeTransferStatus::Idle
            ) || outcome3.inserted > 0
        );
        assert!(history.is_empty(), "history completely drained");
    }

    #[test]
    fn delivery_trace_parity_and_monotonicity() {
        use std::time::Duration;

        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let config =
            SmoothConfig::try_from_parts(Duration::from_millis(16), 2.0, 10.0, 100.0).unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Smooth(config),
                ),
            )
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Before any text is appended, no wakeup is scheduled
        assert_eq!(registry.next_wakeup(), None);

        // Append text: 50 characters
        source
            .append_utf8(
                b"01234567890123456789012345678901234567890123456789",
                &[],
                &[],
            )
            .unwrap();
        let t0 = Instant::now();
        registry
            .sync_connector_deadline(connector.id(), Some(t0))
            .unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        registry.advance(t0).unwrap();
        registry.promote_candidate_projection(connector.id());
        assert!(registry.next_wakeup().is_some());

        let mut now = t0;
        let mut trace = Vec::new();

        // Advance over multiple ticks
        for i in 0..20 {
            now += Duration::from_millis(16);
            let progressed = registry.advance(now).unwrap();
            if !progressed.is_empty() {
                let candidate = registry
                    .connector_candidate_delivery_frontier(connector.id())
                    .unwrap();
                let committed = registry
                    .connector_delivery_frontier(connector.id())
                    .unwrap();
                trace.push((i, candidate.as_u64(), committed.as_u64()));
                // Promote candidate simulating successful frame presentation
                let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
                registry.promote_candidate_projection(connector.id());
                let new_committed = registry
                    .connector_delivery_frontier(connector.id())
                    .unwrap();
                assert_eq!(new_committed, candidate);
            }
        }

        // Verify trace is strictly non-decreasing (monotonic progress)
        assert!(
            !trace.is_empty(),
            "Smoother should have progressed over 20 ticks"
        );
        for window in trace.windows(2) {
            assert!(
                window[1].1 >= window[0].1,
                "Candidate delivery frontier must be monotonic: {} >= {}",
                window[1].1,
                window[0].1
            );
        }

        // Seal the source: completed stream publishes fully and cancels deadlines
        source.seal().unwrap();
        registry
            .sync_connector_deadline(connector.id(), Some(now))
            .unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        registry.promote_candidate_projection(connector.id());

        let final_frontier = registry
            .connector_delivery_frontier(connector.id())
            .unwrap();
        assert_eq!(
            final_frontier.as_u64(),
            50,
            "All 50 characters should be revealed after seal"
        );
        assert_eq!(
            registry.next_wakeup(),
            None,
            "No active deadlines remain when stream is fully drained"
        );
    }

    #[test]
    fn two_connectors_independent_delivery_on_same_source() {
        use std::time::Duration;

        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);

        // Port & Connector 1: Smooth
        let port1 = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port1_id = port1.id();
        let connector1 = registry
            .connect(
                &port1.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        {
            let mut state = port1.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector1.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector1.id());
        }
        {
            let record = registry.connectors.get(&connector1.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Port & Connector 2: Immediate
        let port2 = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port2_id = port2.id();
        let connector2 = registry
            .connect(
                &port2.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        {
            let mut state = port2.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector2.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector2.id());
        }
        {
            let record = registry.connectors.get(&connector2.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Append text
        source
            .append_utf8(b"Hello world from multi-connector test!\n", &[], &[])
            .unwrap();
        let text_len = 39u64;

        // Connector 2 (Immediate) measurement immediately covers all rows/bytes
        let m2 = registry.measure_content(port2_id, 40, crate::presentation::WidthRule::Fill);
        assert!(m2.intrinsic_size.height > 0);
        registry.promote_candidate_projection(connector2.id());

        // Connector 2 has no active deadline in registry
        assert!(
            !registry.active_deadlines.contains_key(&connector2.id()),
            "Immediate connector must not schedule timer deadlines"
        );

        // Connector 1 (Smooth) should have an active deadline
        let t0 = Instant::now();
        registry
            .sync_connector_deadline(connector1.id(), Some(t0))
            .unwrap();
        assert!(
            registry.active_deadlines.contains_key(&connector1.id()),
            "Smooth connector must schedule timer deadline"
        );

        // Initial measure of connector 1 establishes preparation
        let _ = registry.measure_content(port1_id, 40, crate::presentation::WidthRule::Fill);

        // Ticking advances Connector 1 without affecting Connector 2
        registry.advance(t0 + Duration::from_millis(16)).unwrap();
        let _ = registry.measure_content(port1_id, 40, crate::presentation::WidthRule::Fill);
        registry.promote_candidate_projection(connector1.id());

        let frontier1 = registry
            .connector_delivery_frontier(connector1.id())
            .unwrap();
        assert!(
            frontier1.as_u64() < text_len,
            "Smooth connector reveals incrementally, got {} < {}",
            frontier1.as_u64(),
            text_len
        );

        // Connector 2 remains completely immediate and unaffected
        let m2_after = registry.measure_content(port2_id, 40, crate::presentation::WidthRule::Fill);
        assert_eq!(m2.intrinsic_size, m2_after.intrinsic_size);
    }

    #[test]
    fn native_ticks_perform_zero_parser_and_zero_surface_clones() {
        use std::time::Duration;

        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();

        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(
                b"# Header\n\nSome paragraph text that spans multiple words and lines.\n",
                &[],
                &[],
            )
            .unwrap();

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Markdown,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        // Initial measurement and preparation
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let t0 = registry.next_wakeup().unwrap();
        registry.advance(t0).unwrap();
        registry.promote_candidate_projection(connector.id());

        let rebuilds_before =
            crate::perf::snapshot().value(crate::perf::Counter::SemanticProjectionRebuilds);
        let clones_before =
            crate::perf::snapshot().value(crate::perf::Counter::ContentSurfaceClones);

        // Advance 1 tick
        let t1 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        let progressed = registry.advance(t1).unwrap();
        assert!(!progressed.is_empty(), "Advance must progress delivery");

        // Measure / project next frame
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        registry.promote_candidate_projection(connector.id());

        let rebuilds_after =
            crate::perf::snapshot().value(crate::perf::Counter::SemanticProjectionRebuilds);
        let clones_after =
            crate::perf::snapshot().value(crate::perf::Counter::ContentSurfaceClones);

        if cfg!(feature = "perf-counters") {
            assert_eq!(
                rebuilds_after, rebuilds_before,
                "Native delivery tick must perform zero semantic parser rebuilds"
            );
            assert_eq!(
                clones_after, clones_before,
                "Native delivery tick must perform zero whole-surface clones"
            );
        }
    }

    #[test]
    fn visible_frontier_commit_separated_from_execution_progress() {
        use std::time::Duration;

        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"Line one of test text\nLine two of test text\n", &[], &[])
            .unwrap();

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let port_id = port.id();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let t0 = registry.next_wakeup().unwrap();
        registry.advance(t0).unwrap();
        registry.promote_candidate_projection(connector.id());

        // Initial committed frontier is recorded
        let initial_committed = registry
            .connector_delivery_frontier(connector.id())
            .unwrap()
            .as_u64();

        // Advance clock: candidate changes, committed does NOT change
        let t1 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        let progressed = registry.advance(t1).unwrap();
        assert!(!progressed.is_empty());

        let candidate_before_promote = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        let committed_before_promote = registry
            .connector_delivery_frontier(connector.id())
            .unwrap();
        assert!(
            candidate_before_promote.as_u64() > initial_committed,
            "Candidate frontier must advance"
        );
        assert_eq!(
            committed_before_promote.as_u64(),
            initial_committed,
            "Committed frontier must NOT advance before promotion"
        );

        // Simulate frame abortion: clear candidate projections
        registry.clear_candidate_projections();
        let candidate_after_abort = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        let committed_after_abort = registry
            .connector_delivery_frontier(connector.id())
            .unwrap();
        assert_eq!(
            candidate_after_abort, committed_after_abort,
            "Candidate must roll back to committed on abort"
        );
        assert_eq!(
            committed_after_abort.as_u64(),
            initial_committed,
            "Committed remains initial"
        );

        // Now advance and successfully promote
        let t2 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        registry.advance(t2).unwrap();
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        let candidate_ready = registry
            .connector_candidate_delivery_frontier(connector.id())
            .unwrap();
        registry.promote_candidate_projection(connector.id());
        let committed_promoted = registry
            .connector_delivery_frontier(connector.id())
            .unwrap();
        assert_eq!(
            committed_promoted, candidate_ready,
            "Committed frontier matches candidate after promotion"
        );
        assert!(committed_promoted.as_u64() > candidate_before_promote.as_u64());
    }

    #[test]
    fn cold_and_disposed_connectors_clean_up_deadlines() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(b"Some text for deadline test\n", &[], &[])
            .unwrap();

        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::new(
                    TextFunnelKind::Plain,
                    TextWrapMode::Word,
                    false,
                    ContentDelivery::Smooth(SmoothConfig::default()),
                ),
            )
            .unwrap();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector.id());
            state.visible_mounted = true;
            state.visible_connector = Some(connector.id());
        }
        {
            let record = registry.connectors.get(&connector.id()).unwrap();
            let mut state = record.lock().unwrap();
            state.requested = true;
            state.visible = true;
        }

        registry
            .sync_connector_deadline(connector.id(), None)
            .unwrap();
        assert!(
            registry.next_wakeup().is_some(),
            "Active smoothed connector has a deadline"
        );

        // Unmount connector (set invisible)
        registry.set_connector_visible(connector.id(), false);
        assert_eq!(
            registry.next_wakeup(),
            None,
            "Invisible connector deadline must be removed"
        );

        // Remount connector
        registry.set_connector_visible(connector.id(), true);
        assert!(
            registry.next_wakeup().is_some(),
            "Remounted connector deadline must be restored"
        );

        // Dispose connector
        registry.remove_connector(connector.id());
        assert_eq!(
            registry.next_wakeup(),
            None,
            "Disposed connector deadline must be removed"
        );
    }
}
