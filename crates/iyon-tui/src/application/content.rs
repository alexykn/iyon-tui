//! Retained content-plane identities, Source storage, and inactive control state.
//!
//! Source storage is deliberately host-independent. This module owns the
//! PERF-13-E mutation boundary and the PERF-13-D lifecycle graph: environment-
//! owned Sources, host-owned Ports and Connectors, desired/visible mount state,
//! weak subscription bookkeeping, and the plain-text Connector projection.

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
    geometry::{Rect, Size},
    physical::{PhysicalRow, Surface},
    presentation::{ContentMeasurement, ContentProvider, HistoryContentRows},
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
}

impl TextProjectionKey {
    fn revision(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
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
    funnel_kind: TextFunnelKind,
    hyperlinks: bool,
}

impl SemanticProjectionKey {
    fn for_snapshot(
        snapshot: &HostContentSourceSnapshot,
        funnel: HostContentFunnel,
    ) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
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
    key: TextProjectionKey,
    intrinsic_size: Size,
    surface: Arc<Surface>,
    stable_rows: usize,
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
    delivery: Option<ConnectorDelivery>,
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
            delivery: funnel.smooth_config().map(ConnectorDelivery::new),
        }
    }
}

impl HostContentProjection {
    fn measurement(&self) -> ContentMeasurement {
        ContentMeasurement {
            intrinsic_size: self.intrinsic_size,
            physically_complete: self.surface.physically_complete,
            projection_revision: self.key.revision(),
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
/// only a scheduler hint; native host epochs remain authoritative.
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

fn render_semantic_surface(
    semantic: &Projection<TextContent>,
    theme: &Theme,
    offered_width: u16,
    reveal_units: Option<usize>,
) -> Result<(Size, Surface, usize)> {
    let values = semantic
        .spans()
        .iter()
        .flat_map(|span| span.values().iter().cloned())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Ok((Size::new(0, 0), Surface::new(0, 0), 0));
    }
    let renderer = content_text_renderer();
    let view = <TextRenderer as crate::content::Renderer<[TextContent]>>::render(
        &renderer,
        values.as_slice(),
    );
    let compiled =
        crate::presentation::layout::ViewCompiler::new(theme).compile(&view, offered_width.max(1));
    let width = compiled.width;
    let height = compiled.rows.len().min(usize::from(u16::MAX)) as u16;
    let mut surface = Surface::new(width, height);
    surface.physically_complete = compiled.physically_complete;
    for (row_index, row) in compiled
        .rows
        .into_iter()
        .take(usize::from(height))
        .enumerate()
    {
        let (placed, complete) = row.place(width, 0);
        surface.physically_complete &= complete;
        for (column, cell) in placed.cells().iter().enumerate() {
            if cell.painted {
                *surface.get_mut(column as u16, row_index as u16) = cell.clone();
            }
        }
    }
    if let Some(reveal_units) = reveal_units {
        let (revealed, fully_revealed) = reveal_surface(&surface, reveal_units);
        let size = Size::new(revealed.width(), revealed.height());
        return Ok((size, revealed, fully_revealed));
    }
    let full_height = usize::from(height);
    Ok((Size::new(width, height), surface, full_height))
}

fn surface_suffix(surface: &Surface, start: usize) -> Surface {
    crate::perf::inc(crate::perf::Counter::ContentSurfaceClones);
    let start = start.min(usize::from(surface.height()));
    let height = usize::from(surface.height()).saturating_sub(start);
    let mut suffix = Surface::new(surface.width(), height as u16);
    suffix.physically_complete = surface.physically_complete;
    for row in 0..height {
        for column in 0..surface.width() {
            *suffix.get_mut(column, row as u16) = surface.get(column, (start + row) as u16).clone();
        }
    }
    suffix
}

fn reveal_surface(surface: &Surface, mut units: usize) -> (Surface, usize) {
    if units == 0 || surface.width() == 0 || surface.height() == 0 {
        return (Surface::new(surface.width(), 0), 0);
    }
    crate::perf::inc(crate::perf::Counter::ContentSurfaceClones);
    let mut revealed = surface.clone();
    let mut last_row = 0u16;
    let mut saw_glyph = false;
    let mut fully_revealed = 0usize;
    for row in 0..surface.height() {
        let mut cut = None;
        for column in 0..surface.width() {
            let cell = surface.get(column, row);
            if !cell.painted || cell.continuation {
                continue;
            }
            if units == 0 {
                cut = Some(column);
                break;
            }
            units -= 1;
            saw_glyph = true;
            last_row = row;
        }
        if let Some(column) = cut {
            revealed.clear_rect(Rect::new(
                column,
                row,
                surface.width().saturating_sub(column),
                1,
            ));
            if column > 0 {
                last_row = last_row.max(row);
            }
            break;
        }
        fully_revealed = usize::from(row) + 1;
    }
    if !saw_glyph {
        return (Surface::new(surface.width(), 0), 0);
    }
    (
        revealed.crop_to(surface.width(), last_row.saturating_add(1)),
        fully_revealed,
    )
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityIndex {
    pub(crate) row_glyphs: Vec<Vec<u16>>,
    pub(crate) total_glyphs: usize,
}

impl VisibilityIndex {
    pub(crate) fn from_surface(surface: &Surface) -> Self {
        let mut row_glyphs = Vec::with_capacity(usize::from(surface.height()));
        let mut total_glyphs = 0;
        for row in 0..surface.height() {
            let mut cols = Vec::new();
            for col in 0..surface.width() {
                let cell = surface.get(col, row);
                if cell.painted && !cell.continuation {
                    cols.push(col);
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

    pub(crate) fn apply_reveal(
        &self,
        surface: &Arc<Surface>,
        units: usize,
    ) -> (Size, Arc<Surface>, usize) {
        if units == 0 || surface.width() == 0 || surface.height() == 0 || self.total_glyphs == 0 {
            let empty = Surface::new(surface.width(), 0);
            return (Size::new(surface.width(), 0), Arc::new(empty), 0);
        }
        if units >= self.total_glyphs {
            let size = Size::new(surface.width(), surface.height());
            return (size, Arc::clone(surface), self.row_glyphs.len());
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
            let empty = Surface::new(surface.width(), 0);
            return (Size::new(surface.width(), 0), Arc::new(empty), 0);
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

        let mut revealed = Surface::new(surface.width(), target_height);
        revealed.physically_complete = surface.physically_complete;
        for row in 0..target_height {
            let max_col = if let Some((cut_row, cut_col)) = cut && row == cut_row {
                cut_col
            } else if let Some((cut_row, _)) = cut && row > cut_row {
                0
            } else {
                surface.width()
            };
            for col in 0..max_col {
                let cell = surface.get(col, row);
                if cell.painted {
                    *revealed.get_mut(col, row) = cell.clone();
                }
            }
        }
        let size = Size::new(revealed.width(), revealed.height());
        (size, Arc::new(revealed), fully_revealed)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PreparedPaintKey {
    semantic_key: SemanticProjectionKey,
    theme_revision: u64,
    width: u16,
}

#[derive(Clone, Debug)]
struct PreparedPaintProduct {
    unmasked_surface: Arc<Surface>,
    visibility: VisibilityIndex,
    unmasked_stable_prefix_rows: usize,
}

type PreparedPaintCache = VecDeque<(PreparedPaintKey, Arc<PreparedPaintProduct>)>;

fn project_text_snapshot(
    snapshot: &HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    offered_width: u16,
    theme: &Theme,
    theme_revision: u64,
    execution: &mut ConnectorExecution,
    delivery_revision: u64,
    semantic_cache: &mut SemanticProjectionCache,
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
    };
    if snapshot.source_base == snapshot.source_end {
        return Ok(HostContentProjection {
            key,
            intrinsic_size: Size::new(0, 0),
            surface: Arc::new(Surface::new(0, 0)),
            stable_rows: 0,
        });
    }

    let (row_bound, max_line_bytes) = projected_bounds(snapshot, funnel.wrap, offered_width);
    if row_bound > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow!(
            "LIMIT_EXCEEDED: content projection requires {row_bound} rows, exceeding the terminal row limit"
        ));
    }
    if max_line_bytes > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow!(
            "LIMIT_EXCEEDED: content projection has a {max_line_bytes}-byte logical line, exceeding the terminal line limit"
        ));
    }

    // Semantic IR is theme-independent and layout-independent: recolors,
    // window resizes, and smooth timer delivery ticks hit the cache, while
    // source revisions or funnel kind changes rebuild.
    let semantic_key = SemanticProjectionKey::for_snapshot(snapshot, funnel);
    let paint_key = PreparedPaintKey {
        semantic_key: semantic_key.clone(),
        theme_revision,
        width: offered_width.max(1),
    };

    let paint_product = if let Some(product) = prepared_paint_cache
        .iter()
        .find(|(k, _)| k == &paint_key)
        .map(|(_, p)| Arc::clone(p))
    {
        product
    } else {
        let semantic = resolve_cached_semantic(semantic_cache, semantic_key, || {
            project_semantic_snapshot(snapshot, funnel, execution)
        })?;
        let (_unmasked_size, unmasked_surface, _) =
            render_semantic_surface(&semantic, theme, offered_width, None)?;
        let visibility = VisibilityIndex::from_surface(&unmasked_surface);
        let unmasked_stable_prefix_rows = if snapshot.sealed {
            usize::from(unmasked_surface.height())
        } else {
            snapshot
                .stable_prefix()
                .and_then(|prefix| {
                    let prefix_key = SemanticProjectionKey::for_snapshot(&prefix, funnel);
                    let mut prefix_exec = ConnectorExecution::new(&funnel);
                    resolve_cached_semantic(semantic_cache, prefix_key, || {
                        project_semantic_snapshot(&prefix, funnel, &mut prefix_exec)
                    })
                    .ok()
                    .and_then(|prefix_semantic| {
                        render_semantic_surface(&prefix_semantic, theme, offered_width, None)
                            .ok()
                            .map(|(_, prefix_surface, _)| usize::from(prefix_surface.height()))
                    })
                })
                .unwrap_or(0)
        };
        let product = Arc::new(PreparedPaintProduct {
            unmasked_surface: Arc::new(unmasked_surface),
            visibility,
            unmasked_stable_prefix_rows,
        });
        prepared_paint_cache.retain(|(k, _)| k != &paint_key);
        prepared_paint_cache.push_front((paint_key, Arc::clone(&product)));
        while prepared_paint_cache.len() > 4 {
            prepared_paint_cache.pop_back();
        }
        product
    };

    let (intrinsic_size, surface, fully_revealed_rows) =
        if let Some(delivery) = execution.delivery.as_mut() {
            delivery.accept_input(snapshot)?;
            let reveal_units = delivery.reveal_units();
            paint_product
                .visibility
                .apply_reveal(&paint_product.unmasked_surface, reveal_units)
        } else {
            (
                Size::new(
                    paint_product.unmasked_surface.width(),
                    paint_product.unmasked_surface.height(),
                ),
                Arc::clone(&paint_product.unmasked_surface),
                paint_product.visibility.row_glyphs.len(),
            )
        };

    let stable_rows = if snapshot.sealed {
        surface.height() as usize
    } else if execution.delivery.is_some() {
        paint_product
            .unmasked_stable_prefix_rows
            .min(fully_revealed_rows)
    } else {
        paint_product
            .unmasked_stable_prefix_rows
            .min(surface.height() as usize)
    };

    Ok(HostContentProjection {
        key,
        intrinsic_size,
        surface,
        stable_rows,
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
            if let Some(style) = overlapping
                .iter()
                .rev()
                .find_map(|overlap| overlap.style())
            {
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
                if let Some(style) = active
                    .iter()
                    .rev()
                    .find_map(|overlap| overlap.style())
                {
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
struct SourceSubscription {
    host: Weak<Mutex<HostInner>>,
    connector_id: u64,
    connector_generation: u32,
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
    subscribers: Vec<SourceSubscription>,
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
        }
    }

    pub(crate) fn identity(&self) -> EnvironmentIdentity {
        self.identity
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
            subscribers: Vec::new(),
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

fn capture_subscribers(record: &mut ContentSourceRecord) -> Vec<SourceSubscription> {
    record
        .subscribers
        .retain(|subscriber| subscriber.host.strong_count() != 0);
    record.subscribers.clone()
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
        let truncated = record.retention.is_some_and(|policy| policy.drop_oldest)
            || record.storage.base() != 0;
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
            record.copied_bytes = record
                .copied_bytes
                .saturating_add(input.len() as u64);
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
            record.copied_bytes = record
                .copied_bytes
                .saturating_add(input.len() as u64);
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
                && record.storage.annotations_in_order().is_empty()
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
        subscribers: Vec<SourceSubscription>,
    ) -> Result<ContentMutationResult> {
        let mut groups: Vec<(Arc<Mutex<HostInner>>, Vec<(u64, u32)>)> = Vec::new();
        for subscriber in subscribers {
            let Some(host) = subscriber.host.upgrade() else {
                continue;
            };
            if let Some((_, tokens)) = groups
                .iter_mut()
                .find(|(candidate, _)| Arc::ptr_eq(candidate, &host))
            {
                tokens.push((subscriber.connector_id, subscriber.connector_generation));
            } else {
                groups.push((
                    host,
                    vec![(subscriber.connector_id, subscriber.connector_generation)],
                ));
            }
        }

        let mut schedule_environment_drain = false;
        let mut environment_wake_epoch = 0;
        // A failed subscriber must not cancel the remaining wakes (§9.6):
        // every eligible host is attempted, and a failure is reported
        // afterwards with the accepted revision attached, never as an
        // ambiguous ordinary rejection that invites a duplicating retry.
        let mut wake_failures = 0u32;
        let mut first_wake_error: Option<anyhow::Error> = None;
        for (host, tokens) in groups {
            let wake_result = (|| {
                let mut host = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
                let mut affected = false;
                for (id, generation) in tokens {
                    affected |= host
                        .content
                        .source_subscription_is_live(id, generation, revision);
                }
                if affected {
                    let wake = host.mark_content_pending()?;
                    schedule_environment_drain |= wake.schedule_environment_drain;
                    environment_wake_epoch = host.environment_wake_epoch();
                }
                Ok(())
            })();
            if let Err(error) = wake_result {
                wake_failures += 1;
                if first_wake_error.is_none() {
                    first_wake_error = Some(error);
                }
            }
        }
        if let Some(error) = first_wake_error {
            return Err(anyhow!(
                "SOURCE_WAKE_FAILED: Source revision {revision} was accepted; \
                 {wake_failures} subscriber host(s) failed to wake: {error:#}"
            ));
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
        if !drop_oldest
            && retention_head(&record.storage, Some(retention)) > record.storage.base()
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

    fn release_connector(&self) {
        if let Ok(mut record) = self.record.lock() {
            record.connector_count = record.connector_count.saturating_sub(1);
            if record.connector_count == 0 {
                record.subscribers.clear();
            }
        }
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
        record.subscribers.retain(|subscriber| {
            subscriber.host.strong_count() != 0
                && !(Weak::ptr_eq(&subscriber.host, host)
                    && subscriber.connector_id == connector_id
                    && subscriber.connector_generation == connector_generation)
        });
        record.subscribers.push(SourceSubscription {
            host: host.clone(),
            connector_id,
            connector_generation,
        });
        Ok(())
    }

    fn unsubscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) {
        if let Ok(mut record) = self.record.lock() {
            record.subscribers.retain(|subscriber| {
                !Weak::ptr_eq(&subscriber.host, host)
                    || subscriber.connector_id != connector_id
                    || subscriber.connector_generation != connector_generation
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.record
            .lock()
            .map_or(0, |record| record.subscribers.len())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentBinding {
    pub port_id: u64,
    pub connector_id: Option<u64>,
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
    history_unit: Option<u64>,
    history_insets: crate::presentation::Insets,
    history_committed_rows: usize,
    history_committed_content_rows: usize,
    history_leading_padding_rows: usize,
    history_trailing_padding_rows: usize,
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
    committed_projection: Option<Arc<HostContentProjection>>,
    candidate_projection: Option<Arc<HostContentProjection>>,
    projected_source_revision: Option<u64>,
    projection_failure_key: Option<TextProjectionKey>,
    delivery_revision: u64,
    candidate_delivery_frontier: StreamOffset,
    committed_delivery_frontier: StreamOffset,
    execution: Option<ConnectorExecution>,
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
    /// Active due deadlines for smoothed connectors. Native ticks and wake
    /// queries inspect this structure without scanning inactive registries.
    active_deadlines: HashMap<u64, Instant>,
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
            active_deadlines: HashMap::new(),
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
            history_unit: None,
            history_insets: crate::presentation::Insets::ZERO,
            history_committed_rows: 0,
            history_committed_content_rows: 0,
            history_leading_padding_rows: 0,
            history_trailing_padding_rows: 0,
        }));
        self.ports.insert(port_id, Arc::clone(&record));
        Ok(HostContentPort { record, host })
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
                source.release_connector();
                return Err(anyhow!("Connector identity exhausted"));
            }
        };
        self.next_generation = match self.next_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                source.release_connector();
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
            phase: "idle",
            error: None,
            failed_source_revision: None,
            activation_failure: None,
            projection_cache: VecDeque::new(),
            prepared_paint_cache: VecDeque::new(),
            semantic_cache: VecDeque::new(),
            committed_projection: None,
            candidate_projection: None,
            projected_source_revision: None,
            projection_failure_key: None,
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
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn begin_projection_candidate(&mut self) {
        self.clear_candidate_projections();
    }

    /// Advances Connector-local delivery clocks without parsing or touching
    /// Source storage. A progressed smoother invalidates only its derived
    /// projection; the host frame commits the new visible frontier later.
    pub(crate) fn advance(&mut self, now: Instant) -> bool {
        if self.active_deadlines.is_empty() {
            let active_candidates: Vec<u64> = self
                .connectors
                .iter()
                .filter_map(|(&id, c)| {
                    let s = c.lock().ok()?;
                    (s.visible || s.requested).then_some(id)
                })
                .collect();
            for id in active_candidates {
                self.sync_connector_deadline(id, Some(now));
            }
        }
        if self.active_deadlines.is_empty() {
            return false;
        }
        let due_ids: Vec<u64> = self
            .active_deadlines
            .iter()
            .filter_map(|(&id, &deadline)| (deadline <= now).then_some(id))
            .collect();
        if due_ids.is_empty() {
            return false;
        }
        let mut changed = false;
        for connector_id in due_ids {
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                self.active_deadlines.remove(&connector_id);
                continue;
            };
            let Ok(mut state) = connector.lock() else {
                continue;
            };
            if !state.visible && !state.requested {
                self.active_deadlines.remove(&connector_id);
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
                self.active_deadlines.insert(connector_id, dl);
            } else {
                self.active_deadlines.remove(&connector_id);
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
            changed = true;
        }
        changed
    }

    pub(crate) fn next_wakeup(&self) -> Option<Instant> {
        self.active_deadlines.values().copied().min()
    }

    pub(crate) fn sync_connector_deadline(&mut self, connector_id: u64, now: Option<Instant>) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            self.active_deadlines.remove(&connector_id);
            return;
        };
        let Ok(mut state) = connector.lock() else {
            return;
        };
        if state.lifecycle == ConnectorLifecycle::Disposed || (!state.visible && !state.requested) {
            self.active_deadlines.remove(&connector_id);
            return;
        }
        if state.execution.is_none() && state.funnel.smooth_config().is_some() {
            state.execution = Some(ConnectorExecution::new(&state.funnel));
        }
        if let Ok(snapshot) = state.source.snapshot()
            && let Some(execution) = state.execution.as_mut()
            && let Some(delivery) = execution.delivery.as_mut()
        {
            let _ = delivery.accept_input(&snapshot);
            if !delivery.smoother.has_pending_work() {
                self.active_deadlines.remove(&connector_id);
            } else if let Some(dl) = delivery.smoother.next_wakeup() {
                self.active_deadlines.insert(connector_id, dl);
            } else if let Some(now) = now {
                self.active_deadlines.insert(connector_id, now);
            } else {
                self.active_deadlines.insert(connector_id, Instant::now());
            }
        } else {
            self.active_deadlines.remove(&connector_id);
        }
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
        let snapshot = state.source.snapshot()?;
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
        })
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
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let (source, funnel, delivery_revision) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live || (!state.requested && !state.visible) {
                return Ok(ContentMeasurement::default());
            }
            (state.source.clone(), state.funnel, state.delivery_revision)
        };
        crate::perf::inc(crate::perf::Counter::SemanticPreparations);
        let snapshot = source.snapshot()?;
        if funnel.kind == TextFunnelKind::Markdown && snapshot.source_base != 0 {
            return Err(anyhow!(
                "RETENTION_INCOMPATIBLE: Markdown requires an untruncated Source from its logical start"
            ));
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
                let measurement = projection.measurement();
                drop(state);
                self.sync_connector_deadline(connector_id, None);
                return Ok(measurement);
            }
        }

        // The snapshot owns immutable chunks; the Source lock is not held
        // while width-dependent projection allocates/compiles derived rows.
        // Execution state is Connector-local. Take it, semantic cache, and
        // prepared paint cache out while projecting so a parser/smoother can
        // mutate without holding the Connector mutex.
        let (mut execution, mut semantic_cache, mut prepared_paint_cache) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (
                state
                    .execution
                    .take()
                    .unwrap_or_else(|| ConnectorExecution::new(&funnel)),
                std::mem::take(&mut state.semantic_cache),
                std::mem::take(&mut state.prepared_paint_cache),
            )
        };
        let projection = match project_text_snapshot(
            &snapshot,
            funnel,
            offered_width,
            &self.theme,
            self.theme_revision,
            &mut execution,
            delivery_revision,
            &mut semantic_cache,
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
                    state.prepared_paint_cache = prepared_paint_cache;
                }
                return Err(error);
            }
        };
        let measurement = projection.measurement();
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if let Some(delivery) = execution.delivery.as_ref() {
            state.candidate_delivery_frontier = delivery.candidate_frontier;
        }
        state.execution = Some(execution);
        state.semantic_cache = semantic_cache;
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
        self.sync_connector_deadline(connector_id, None);
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
            return Some(projection.measurement());
        }
        if let Some(projection) = Self::cached_projection(&connector, &key) {
            return Some(projection.measurement());
        }
        connector
            .committed_projection
            .as_ref()
            .filter(|projection| projection.key.width == key.width)
            .map(|projection| projection.measurement())
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
        diagnostic: String,
    ) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        if let Ok(mut state) = connector.lock() {
            let code = if diagnostic.starts_with("LIMIT_EXCEEDED:") {
                "LIMIT_EXCEEDED"
            } else if diagnostic.starts_with("RETENTION_INCOMPATIBLE:") {
                "RETENTION_INCOMPATIBLE"
            } else {
                "PROJECTION_FAILED"
            };
            state.error = Some(ContentConnectorError {
                code: code.to_owned(),
                diagnostic,
            });
            state.failed_source_revision = Some(key.source_revision);
            state.projection_failure_key = Some(key);
            state.phase = if state.visible { "active" } else { "failed" };
        }
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
        self.prepare_connector_projection(connector_id, measurement.intrinsic_size.width)
            .unwrap_or(measurement)
    }

    fn adjust_history_measurement(
        &self,
        port_id: u64,
        mut measurement: ContentMeasurement,
    ) -> ContentMeasurement {
        let Some(committed_rows) = self
            .ports
            .get(&port_id)
            .and_then(|port| port.lock().ok())
            .map(|port| port.history_committed_content_rows)
        else {
            return measurement;
        };
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
            if self
                .prepare_activation_candidate(connector_id, offered_width)
                .unwrap_or(false)
            {
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
                            self.record_projection_failure(connector_id, key, error.to_string());
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

    fn paint_content(&self, port_id: u64, offered_width: u16) -> Option<Arc<Surface>> {
        let connector_id = self.selected_connector_id(port_id)?;
        let projection = self.connector_projection(connector_id, offered_width)?;
        let committed_rows = self
            .ports
            .get(&port_id)
            .and_then(|port| port.lock().ok())
            .map_or(0, |port| port.history_committed_content_rows);
        if committed_rows == 0 {
            return Some(Arc::clone(&projection.surface));
        }
        Some(Arc::new(surface_suffix(
            &projection.surface,
            committed_rows,
        )))
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
            .filter(|projection| projection.key.width == key.width)
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
            .filter(|projection| projection.key.width == key.width)
            .cloned()
    }

    pub(crate) fn candidate_bindings(&mut self, targets: &[u64]) -> Result<Vec<ContentBinding>> {
        // Ordinary target validation belongs to H3 prepare; Repeating it here
        // would let the fallible frame path report stale/duplicate/wrong-host
        // attachment errors after the desired publication has already been
        // accepted. The owner registry and H3 lease keep this lookup valid;
        // disappearance is an internal invariant failure instead.
        targets
            .iter()
            .map(|port_id| {
                let port = self.ports.get(port_id).cloned().ok_or_else(|| {
                    anyhow!(
                        "INTERNAL_INVARIANT: ContentPort {port_id} disappeared after H3 prepare"
                    )
                })?;
                let (desired_connector, visible_connector, port_mounted) = {
                    let port = port.lock().map_err(|_| {
                        anyhow!("INTERNAL_INVARIANT: ContentPort {port_id} lock is poisoned")
                    })?;
                    (
                        port.desired_connector,
                        port.visible_connector,
                        port.desired_mounted,
                    )
                };
                let connector_id = if let Some(selection) = self.candidate_selections.get(port_id) {
                    *selection
                } else {
                    match desired_connector {
                        None => None,
                        // Projection/activation belongs to the measure pass;
                        // this final selection step must not retry a failed
                        // candidate with an arbitrary width after layout.
                        Some(id) if port_mounted && self.connector_is_candidate_ready(id) => {
                            Some(id)
                        }
                        Some(_) => visible_connector,
                    }
                };
                Ok(ContentBinding {
                    port_id: *port_id,
                    connector_id,
                })
            })
            .collect()
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
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation candidate {connector_id} disappeared"
            ));
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.activation_failure.is_none() {
            return Ok(false);
        }
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation failure targeted a non-candidate Connector {connector_id}"
            ));
        }
        // Capture the revision and input key at the start of the failed
        // attempt. A concurrent Source mutation may commit while the host is
        // still preparing this candidate; recording the pre-attempt revision
        // and exact width prevents a second layout measurement in this same
        // frame from retrying the identical failed input.
        let snapshot = state.source.snapshot()?;
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
    pub(crate) fn begin_candidate(&mut self, bindings: &[ContentBinding]) {
        self.in_flight_connectors = bindings
            .iter()
            .filter_map(|binding| binding.connector_id)
            .collect();
    }

    /// Releases the candidate lease after its logical frame commit. A
    /// disposing Connector selected by that frame remains visible/disposing
    /// until the following removal frame, as required by transactional
    /// disposal semantics.
    pub(crate) fn end_candidate(&mut self) {
        self.in_flight_connectors.clear();
        self.clear_candidate_projections();
    }

    /// Aborts a candidate without changing visible bindings. Deferred control
    /// mutations can now finalize identities that were never made visible and
    /// inactive requested Connectors can lose provisional subscriptions.
    pub(crate) fn abort_candidate(&mut self) {
        self.clear_candidate_projections();
        let connector_ids = self.in_flight_connectors.drain().collect::<Vec<_>>();
        for connector_id in connector_ids {
            self.cleanup_aborted_candidate(connector_id);
        }
        self.finalize_disposed_connectors();
    }

    fn cleanup_aborted_candidate(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let Ok(state) = connector.lock() else {
            return;
        };
        let source = state.source.clone();
        let generation = state.generation;
        let visible = state.visible;
        let requested = state.requested;
        let port_mounted = state
            .port
            .upgrade()
            .and_then(|port| port.lock().ok().map(|port| port.desired_mounted))
            .unwrap_or(false);
        drop(state);
        if !visible && (!requested || !port_mounted) {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
    }

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
        for connector in self.connectors.values() {
            if let Ok(mut state) = connector.lock() {
                state.candidate_projection = None;
                state.candidate_delivery_frontier = state.committed_delivery_frontier;
                if !state.visible {
                    state.committed_projection = None;
                    state.projection_cache.clear();
                    state.prepared_paint_cache.clear();
                    state.semantic_cache.clear();
                    state.projected_source_revision = None;
                    state.execution = None;
                    state.delivery_revision = 0;
                    state.candidate_delivery_frontier = StreamOffset::ZERO;
                    state.committed_delivery_frontier = StreamOffset::ZERO;
                }
            }
        }
        self.active_deadlines.retain(|id, _| {
            self.connectors
                .get(id)
                .and_then(|c| c.lock().ok())
                .is_some_and(|s| s.visible || s.requested)
        });
    }

    pub(crate) fn commit_visible(&mut self, bindings: &[ContentBinding]) {
        for binding in bindings {
            if let Some(connector_id) = binding.connector_id {
                self.promote_candidate_projection(connector_id);
            }
        }
        let mounted = bindings
            .iter()
            .map(|binding| binding.port_id)
            .collect::<HashSet<_>>();
        let selected = bindings
            .iter()
            .map(|binding| (binding.port_id, binding.connector_id))
            .collect::<HashMap<_, _>>();
        let port_ids = self.ports.keys().copied().collect::<Vec<_>>();
        crate::perf::add(
            crate::perf::Counter::ContentRegistryPortScans,
            port_ids.len() as u64,
        );
        for port_id in port_ids {
            let Some(port) = self.ports.get(&port_id).cloned() else {
                continue;
            };
            let old_visible = port.lock().ok().and_then(|port| port.visible_connector);
            let next_visible = selected.get(&port_id).copied().flatten();
            if let Some(old_id) = old_visible
                && Some(old_id) != next_visible
            {
                self.set_connector_visible(old_id, false);
            }
            if let Ok(mut port_state) = port.lock() {
                port_state.visible_mounted = mounted.contains(&port_id);
                port_state.visible_connector = if port_state.visible_mounted {
                    next_visible
                } else {
                    None
                };
            }
            if let Some(next_id) = next_visible
                && mounted.contains(&port_id)
            {
                self.set_connector_visible(next_id, true);
            }
        }
        let connector_ids = self.connectors.keys().copied().collect::<Vec<_>>();
        for connector_id in connector_ids {
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                continue;
            };
            let Ok(mut state) = connector.lock() else {
                continue;
            };
            if state.lifecycle == ConnectorLifecycle::Disposed {
                continue;
            }
            let port_mounted = state
                .port
                .upgrade()
                .and_then(|port| port.lock().ok().map(|port| port.visible_mounted))
                .unwrap_or(false);
            if state.visible {
                state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                    "disposing"
                } else {
                    "active"
                };
            } else if state.lifecycle == ConnectorLifecycle::Disposing {
                state.phase = "disposing";
            } else if state.requested {
                state.phase = if state.error.is_some() && port_mounted {
                    "failed"
                } else if port_mounted {
                    "activation-pending"
                } else {
                    "waiting-for-mount"
                };
            } else {
                state.phase = "idle";
            }
        }
        self.finalize_disposed_connectors();
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
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (port, generation, source, was_requested, was_selected, port_mounted, was_failed) = {
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
            )
        };
        if was_requested && was_selected && !was_failed {
            return Ok(false);
        }
        let old_selected = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
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
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
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
        if port_mounted {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        // The request itself is not the activation/projection operation. A
        // mounted candidate is processed by candidate_bindings inside the
        // frame transaction, where injected/real operational failure can fall
        // back to the committed Connector without changing the visible frame.
        Ok(port_mounted)
    }

    fn request_deactivation(&mut self, connector_id: u64) -> Result<bool> {
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
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        if !was_requested && !was_visible && !in_flight {
            return Ok(false);
        }
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
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        if let Ok(mut state) = connector.lock() {
            state.requested = false;
            state.phase = if state.visible { "active" } else { "idle" };
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
        if !visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation);
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
            self.unsubscribe_connector(&source, connector_id, generation);
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
            state.requested = false;
            if !state.visible {
                state.phase = "idle";
            }
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
            self.unsubscribe_connector(&source, connector_id, generation);
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

    fn unsubscribe_connector(&mut self, source: &HostContentSource, id: u64, generation: u32) {
        source.unsubscribe(&self.owner_host, id, generation);
        if let Some(connector) = self.connectors.get(&id)
            && let Ok(mut state) = connector.lock()
        {
            state.subscribed = false;
        }
    }

    fn set_connector_visible(&mut self, connector_id: u64, visible: bool) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let Ok(mut state) = connector.lock() else {
            return;
        };
        state.visible = visible;
        let source = state.source.clone();
        let generation = state.generation;
        if visible {
            state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                "disposing"
            } else {
                "active"
            };
        } else {
            state.committed_projection = None;
            state.candidate_projection = None;
            state.projection_cache.clear();
            state.prepared_paint_cache.clear();
            state.semantic_cache.clear();
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
        drop(state);
        if visible {
            self.sync_connector_deadline(connector_id, None);
        } else {
            self.active_deadlines.remove(&connector_id);
            self.unsubscribe_connector(&source, connector_id, generation);
        }
    }

    fn remove_connector(&mut self, connector_id: u64) {
        self.active_deadlines.remove(&connector_id);
        let Some(connector) = self.connectors.remove(&connector_id) else {
            return;
        };
        if let Ok(mut state) = connector.lock() {
            state
                .source
                .unsubscribe(&self.owner_host, connector_id, state.generation);
            state.source.release_connector();
            state.lifecycle = ConnectorLifecycle::Disposed;
            state.phase = "disposed";
            state.visible = false;
            state.requested = false;
            if let Some(port) = state.port.upgrade()
                && let Ok(mut port_state) = port.lock()
            {
                port_state.connector_ids.remove(&connector_id);
                if port_state.desired_connector == Some(connector_id) {
                    port_state.desired_connector = None;
                }
                if port_state.visible_connector == Some(connector_id) {
                    port_state.visible_connector = None;
                }
            }
        }
    }

    fn finalize_disposed_connectors(&mut self) {
        let ids = self
            .connectors
            .iter()
            .filter_map(|(id, connector)| {
                let state = connector.lock().ok()?;
                (state.lifecycle == ConnectorLifecycle::Disposing && !state.visible).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            self.remove_connector(id);
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
        let mut state = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
        state.history_unit = Some(unit_id);
        state.history_insets = insets;
        state.history_committed_rows = 0;
        state.history_committed_content_rows = 0;
        state.history_leading_padding_rows = 0;
        state.history_trailing_padding_rows = 0;
        Ok(())
    }

    fn history_rows(&self, port_id: u64, offered_width: u16) -> Option<HistoryContentRows> {
        let port = self.ports.get(&port_id)?;
        let (insets, committed_rows, connector_id) = {
            let state = port.lock().ok()?;
            (
                state.history_insets,
                state.history_committed_rows,
                state.visible_connector.or(state.desired_connector),
            )
        };
        let connector_id = connector_id?;
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let snapshot = connector.source.snapshot().ok()?;
        let sealed = snapshot.sealed;
        drop(connector);
        let content_width =
            offered_width.saturating_sub(insets.left().saturating_add(insets.right()));
        let projection = self.connector_projection(connector_id, content_width)?;
        let content = &projection.surface;
        let surface = if sealed {
            let width = content
                .width()
                .saturating_add(insets.left())
                .saturating_add(insets.right());
            let height = content
                .height()
                .saturating_add(insets.top())
                .saturating_add(insets.bottom());
            let mut surface = Surface::new(width, height);
            surface.composite(content, insets.left(), insets.top());
            surface
        } else {
            let width = content
                .width()
                .saturating_add(insets.left())
                .saturating_add(insets.right());
            let height = usize::from(insets.top()).saturating_add(usize::from(content.height()));
            let mut surface = Surface::new(width, height as u16);
            surface.composite(content, insets.left(), insets.top());
            surface
        };
        let content_start = usize::from(insets.top());
        let content_end = content_start.saturating_add(usize::from(content.height()));
        let stable_end = if sealed {
            usize::from(surface.height())
        } else {
            projection
                .stable_rows
                .min(usize::from(content.height()))
                .saturating_add(content_start)
        };
        let start = committed_rows.min(usize::from(surface.height()));
        let end = stable_end.min(usize::from(surface.height()));
        let rows = (start < end)
            .then(|| {
                (start..end)
                    .map(|row| {
                        PhysicalRow::from_cells(
                            (0..surface.width())
                                .map(|column| surface.get(column, row as u16).clone())
                                .collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let payload_content_start = content_start.max(start).saturating_sub(start);
        let payload_content_end = content_end.min(end).saturating_sub(start);
        let leading_padding = usize::from(insets.top())
            .saturating_sub(start)
            .min(end.saturating_sub(start));
        let trailing_padding = if sealed {
            end.saturating_sub(content_end.max(start))
                .min(usize::from(insets.bottom()))
        } else {
            0
        };
        Some(HistoryContentRows {
            rows,
            complete: sealed && end >= usize::from(surface.height()),
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
        let Some(port) = self.ports.get(&port_id) else {
            return;
        };
        if let Ok(mut state) = port.lock() {
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

    pub(crate) fn clear_history_unit(&mut self, unit_id: u64) {
        for port in self.ports.values() {
            if let Ok(mut state) = port.lock()
                && state.history_unit == Some(unit_id)
            {
                state.history_unit = None;
                state.history_committed_rows = 0;
                state.history_committed_content_rows = 0;
                state.history_leading_padding_rows = 0;
                state.history_trailing_padding_rows = 0;
            }
        }
    }

    pub(crate) fn history_unit_retired(&mut self, unit_id: u64) {
        let ports = self
            .ports
            .values()
            .filter_map(|port| {
                let state = port.lock().ok()?;
                (state.history_unit == Some(unit_id))
                    .then(|| (Arc::clone(port), state.connector_ids.clone()))
            })
            .collect::<Vec<_>>();
        for (port, connector_ids) in ports {
            let port_id = if let Ok(mut state) = port.lock() {
                state.history_unit = None;
                state.history_committed_rows = 0;
                state.history_committed_content_rows = 0;
                state.history_leading_padding_rows = 0;
                state.history_trailing_padding_rows = 0;
                state.desired_mounted = false;
                state.visible_mounted = false;
                state.desired_connector = None;
                state.visible_connector = None;
                state.lifecycle = PortLifecycle::Disposed;
                state.id
            } else {
                continue;
            };
            for connector_id in connector_ids {
                self.remove_connector(connector_id);
            }
            self.ports.remove(&port_id);
        }
    }

    fn connector_is_candidate_ready(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_some_and(|state| {
                state.lifecycle == ConnectorLifecycle::Live
                    && state.requested
                    && state.error.is_none()
                    && state.candidate_projection.is_some()
            })
    }

    pub(super) fn source_subscription_is_live(
        &mut self,
        id: u64,
        generation: u32,
        source_revision: u64,
    ) -> bool {
        let Some(connector) = self.connectors.get(&id).cloned() else {
            return false;
        };
        let Ok(mut state) = connector.lock() else {
            return false;
        };
        if state.generation != generation
            || !state.subscribed
            || state.lifecycle == ConnectorLifecycle::Disposed
        {
            return false;
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
        let is_live = state.visible || state.requested || self.in_flight_connectors.contains(&id);
        drop(state);
        if is_live {
            self.sync_connector_deadline(id, None);
        }
        is_live
    }

    pub(crate) fn connector_is_disposed(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_none_or(|state| state.lifecycle == ConnectorLifecycle::Disposed)
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
        if let Some(port) = self.ports.get(&port_id).and_then(|port| port.lock().ok()) {
            port.history_committed_rows.hash(&mut hasher);
            port.history_committed_content_rows.hash(&mut hasher);
            port.history_leading_padding_rows.hash(&mut hasher);
            port.history_trailing_padding_rows.hash(&mut hasher);
        }
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

    fn paint(
        &self,
        port_id: u64,
        offered_width: u16,
        _allocated_height: u16,
    ) -> Option<Arc<Surface>> {
        self.paint_content(port_id, offered_width)
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
        let Some(port) = self.ports.get(&port_id) else {
            return view.clone();
        };
        let Some(state) = port.lock().ok() else {
            return view.clone();
        };
        let mut insets = view.decoration().padding;
        if state.history_leading_padding_rows > 0 {
            insets.top = 0;
        }
        if state.history_trailing_padding_rows > 0 {
            insets.bottom = 0;
        }
        drop(state);
        if insets == view.decoration().padding {
            return view.clone();
        }
        view.clone().padding(insets)
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
        if state.history_unit.is_none() {
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
    record: Arc<Mutex<PortRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentPort {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.record.lock().map_or(0, |record| record.id)
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.record.lock().map_or(0, |record| record.generation)
    }

    #[must_use]
    pub fn family(&self) -> ContentFamily {
        self.record
            .lock()
            .map_or(ContentFamily::Text, |record| record.family)
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.deactivate_port(self.id())?;
        if needs_frame {
            return inner.mark_content_pending();
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
        self.record
            .lock()
            .map(|record| record.visible_mounted)
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))
    }
}

#[derive(Clone, Debug)]
pub struct HostContentConnector {
    record: Arc<Mutex<ConnectorRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentConnector {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.record.lock().map_or(0, |record| record.id)
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.record.lock().map_or(0, |record| record.generation)
    }

    #[must_use]
    pub fn source_id(&self) -> u64 {
        self.record.lock().map_or(0, |record| record.source.id())
    }

    pub fn activate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner
            .content
            .request_connector_activation(self.id(), &host_weak)?;
        if needs_frame {
            return inner.mark_content_pending();
        }
        Ok(WakeDisposition::default())
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.request_connector_deactivation(self.id())?;
        if needs_frame {
            return inner.mark_content_pending();
        }
        Ok(WakeDisposition::default())
    }

    pub fn dispose(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.request_connector_dispose(self.id())?;
        if needs_frame {
            return inner.mark_content_pending();
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
        let disposed = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .lifecycle
            == ConnectorLifecycle::Disposed;
        if disposed {
            return self.record_status();
        }
        if let Some(host) = self.host.upgrade() {
            return host
                .lock()
                .map_err(|_| anyhow!("host lock is poisoned"))?
                .content
                .connector_status(self.id());
        }
        // HostInner::drop() marks retained Connector records disposed before
        // its weak owner disappears. A live record with no owner is an
        // invariant failure, not a reason to fabricate a status.
        Err(anyhow!("HOST_DISPOSED: Connector host is gone"))
    }

    pub fn visible_delivery_frontier(&self) -> Result<StreamOffset> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.committed_delivery_frontier)
    }

    pub fn candidate_delivery_frontier(&self) -> Result<StreamOffset> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.candidate_delivery_frontier)
    }

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
        })
    }

    #[must_use]
    pub fn is_disposed(&self) -> bool {
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
    use crate::View;
    use crate::application::environment::TuiEnvironment;
    use crate::application::host::TuiHost;

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
            .set_desired_view(View::native_content_host(first_port.id()).unwrap())
            .unwrap();
        first.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 1);

        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        let second_connector = second_port.connect(&source, funnel).unwrap();
        second_connector.activate().unwrap();
        assert_eq!(source.subscriber_count(), 1);
        second
            .set_desired_view(View::native_content_host(second_port.id()).unwrap())
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
    fn poisoned_subscriber_wake_reports_source_wake_failed_with_accepted_revision() {
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
            .set_desired_view(View::native_content_host(first_port.id()).unwrap())
            .unwrap();
        first.flush_pending_hosts(32, true).unwrap();

        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        let second_conn = second_port.connect(&source, funnel).unwrap();
        second_conn.activate().unwrap();
        second
            .set_desired_view(View::native_content_host(second_port.id()).unwrap())
            .unwrap();
        second.flush_pending_hosts(32, true).unwrap();

        assert_eq!(source.subscriber_count(), 2);

        // Deliberately poison first host's mutex
        let inner_clone = first.inner.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = inner_clone.lock().unwrap();
            panic!("deliberate host lock poisoning for test");
        }));

        // Now append to the source. The source revision MUST advance and be accepted,
        // while finish_mutation reports SOURCE_WAKE_FAILED with the accepted revision.
        let result = source.append_utf8(b"hello\n", &[], &[]);
        let err = result.expect_err("mutation should report wake failure when subscriber is poisoned");
        let err_str = err.to_string();
        assert!(
            err_str.contains("SOURCE_WAKE_FAILED"),
            "error must identify wake failure, got: {err_str}"
        );
        assert!(
            err_str.contains("Source revision 1 was accepted"),
            "error must report accepted revision to prevent retry duplicates, got: {err_str}"
        );

        // State is authoritative and NOT rolled back:
        let stats = source.stats().unwrap();
        assert_eq!(stats.revision, 1);
        assert_eq!(stats.accepted_bytes, 6);
        assert_eq!(source.snapshot().unwrap().text(), "hello\n");
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

        let latest = source.snapshot().unwrap();
        assert_eq!(latest.annotations().len(), 50);
        assert_eq!(latest.retained_lines(), 51); // 50 newlines + base line = 51

        // 4. Test atomic retention + annotations
        source
            .configure_retention(Some(30), None, true)
            .unwrap();
        source
            .append_utf8(b"tail-item\n", &[], &[])
            .unwrap();
        let stats = source.stats().unwrap();
        // Base advanced to keep within 30 bytes
        assert!(stats.source_base > 0);
        let snap_retained = source.snapshot().unwrap();
        assert!(snap_retained.retained_bytes() <= 30);
        // Annotations were atomically pruned/clipped with the head truncation
        for anno in snap_retained.annotations() {
            assert!(anno.end_byte > stats.source_base);
        }
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
                HostContentFunnel::plain(TextWrapMode::Word),
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

        let foreign_view = View::native_content_host(first_port.id()).unwrap();
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
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
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
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
            .unwrap();
        first.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        second
            .fail_next_activation("synthetic projection failure".to_owned())
            .unwrap();
        second.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "failed");

        host.set_desired_view(View::spacer(0)).unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "waiting-for-mount");
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "active");

        host.close().unwrap();
        source.dispose().unwrap();
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
        let binding = ContentBinding {
            port_id: port.id(),
            connector_id: Some(connector_id),
        };
        registry.begin_candidate(&[binding]);

        assert!(registry.request_deactivation(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.abort_candidate();
        assert!(registry.connectors.contains_key(&connector_id));
        assert_eq!(source.subscriber_count(), 0);

        {
            let mut state = port.record.lock().unwrap();
            state.desired_connector = Some(connector_id);
        }
        registry.begin_candidate(&[binding]);
        assert!(registry.request_connector_disposal(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.commit_visible(&[binding]);
        registry.end_candidate();
        assert!(
            registry
                .connectors
                .get(&connector_id)
                .and_then(|record| record.lock().ok())
                .is_some_and(|state| state.visible)
        );
        registry.commit_visible(&[]);
        assert!(!registry.connectors.contains_key(&connector_id));
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
    fn reveal_surface_cut_at_row_boundary_does_not_add_blank_trailing_row() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        let mut surface = Surface::new(10, 2);
        for column in 0..3 {
            let cell = crate::physical::PhysicalCell {
                grapheme: Some("a".to_owned()),
                style: crate::physical::PhysicalStyle::default(),
                painted: true,
                continuation: false,
            };
            *surface.get_mut(column, 0) = cell;
        }
        for column in 0..3 {
            let cell = crate::physical::PhysicalCell {
                grapheme: Some("b".to_owned()),
                style: crate::physical::PhysicalStyle::default(),
                painted: true,
                continuation: false,
            };
            *surface.get_mut(column, 1) = cell;
        }
        // Exactly 3 units -> reveals only the 3 cells in row 0
        let (revealed, fully_revealed) = reveal_surface(&surface, 3);
        #[cfg(feature = "perf-counters")]
        assert_eq!(
            crate::perf::snapshot().value(crate::perf::Counter::ContentSurfaceClones),
            1
        );
        assert_eq!(
            revealed.height(),
            1,
            "revealed surface should only have 1 row when all cells of row 1 are unrevealed"
        );
        assert_eq!(fully_revealed, 1);
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
        let key1 = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        registry.set_theme(&Arc::new(t2));
        let m2 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
        let key2 = registry
            .connector_projection_key(connector.id(), 20)
            .unwrap();
        assert_ne!(key1, key2, "TextProjectionKey must differ across themes");
        assert_ne!(
            m1.projection_revision, m2.projection_revision,
            "ContentMeasurement projection_revision must change across themes to invalidate paint cache"
        );
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
        let before = rebuilds();
        let m1 = registry.measure_content(port.id(), 20, crate::presentation::WidthRule::Fill);
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
        let view = View::native_content_host(port_id)
            .unwrap()
            .padding(crate::Insets::new(2, 0, 1, 0));
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
        let view = View::native_content_host(port_id).unwrap();
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
        registry.advance(t0);
        let t1 = t0 + std::time::Duration::from_secs(2);
        registry.advance(t1);

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
    fn history_transfer_tool_and_unsealed_markdown_stream_does_not_block() {
        let mut history = crate::History::new();
        // Unit 1: A completed/frozen tool call unit
        let tool_view = View::text("Tool: execute_command -> success");
        let _ = history.push(tool_view).unwrap();

        // Unit 2: An unsealed Markdown assistant stream
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        source
            .append_utf8(
                b"Assistant response line 1\nAssistant response line 2\n",
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
        let stream_view = View::native_content_host(port_id).unwrap();
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
        registry.advance(t0);
        let t1 = t0 + std::time::Duration::from_secs(2);
        registry.advance(t1);
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);

        let mut sink = LocalSink::default();
        let theme = crate::Theme::new();

        // First transfer: transfers the tool call!
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
            "tool call unit must transfer to native scrollback"
        );
        assert_eq!(
            history.len(),
            1,
            "tool call unit retired; stream unit remains"
        );

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
        registry.sync_connector_deadline(connector.id(), Some(t0));
        let _ = registry.measure_content(port_id, 40, crate::presentation::WidthRule::Fill);
        registry.advance(t0);
        registry.promote_candidate_projection(connector.id());
        assert!(registry.next_wakeup().is_some());

        let mut now = t0;
        let mut trace = Vec::new();

        // Advance over multiple ticks
        for i in 0..20 {
            now += Duration::from_millis(16);
            let progressed = registry.advance(now);
            if progressed {
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
        assert!(!trace.is_empty(), "Smoother should have progressed over 20 ticks");
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
        registry.sync_connector_deadline(connector.id(), Some(now));
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
        registry.sync_connector_deadline(connector1.id(), Some(t0));
        assert!(
            registry.active_deadlines.contains_key(&connector1.id()),
            "Smooth connector must schedule timer deadline"
        );

        // Initial measure of connector 1 establishes preparation
        let _ = registry.measure_content(port1_id, 40, crate::presentation::WidthRule::Fill);

        // Ticking advances Connector 1 without affecting Connector 2
        registry.advance(t0 + Duration::from_millis(16));
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
        registry.advance(t0);
        registry.promote_candidate_projection(connector.id());

        let rebuilds_before =
            crate::perf::snapshot().value(crate::perf::Counter::SemanticProjectionRebuilds);
        let clones_before =
            crate::perf::snapshot().value(crate::perf::Counter::ContentSurfaceClones);

        // Advance 1 tick
        let t1 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        let progressed = registry.advance(t1);
        assert!(progressed, "Advance must progress delivery");

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
        registry.advance(t0);
        registry.promote_candidate_projection(connector.id());

        // Initial committed frontier is recorded
        let initial_committed = registry
            .connector_delivery_frontier(connector.id())
            .unwrap()
            .as_u64();

        // Advance clock: candidate changes, committed does NOT change
        let t1 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        let progressed = registry.advance(t1);
        assert!(progressed);

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
        assert_eq!(committed_after_abort.as_u64(), initial_committed, "Committed remains initial");

        // Now advance and successfully promote
        let t2 = registry.next_wakeup().unwrap() + Duration::from_millis(100);
        registry.advance(t2);
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

        registry.sync_connector_deadline(connector.id(), None);
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
