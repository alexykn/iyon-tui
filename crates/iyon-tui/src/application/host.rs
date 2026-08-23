//! A language-binding host for the retained native application runtime.
//!
//! `TuiHost` deliberately exposes caller-defined outputs and native snapshots, not
//! terminal events. Components remain mounted in the same `SceneHost` used by
//! the Rust application driver.

use std::{
    collections::VecDeque,
    ops::Range,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

use crate::controls::text_input::command::TextInputCommand;
use crate::text::RewriteProjectionError;
use crate::text::{
    Alignment, Block, BlockKind, Inline, InlineContent, InlineKind, List, ListItem, ListMarker,
    LiteralText, Mark, SemanticTag, Table, TableCell, TableColumn, TableRow, TextIrError,
    TextProjectionError, TextProvenance, TextRewriter, TextRun, validate_text_projection,
    walk_rewrite_inline,
};
use crate::{
    App as TuiApp, AppCx, BorderSpec, CodeBlockLabelPolicy, Component, ComponentCx,
    ComponentHandle, History, HistoryLayout, HistoryStreamHandle, HistoryUnitId, InteractionResult,
    IntoView, KeyStroke, MarkdownOptions, MarkdownProjector, Output, Projection, ProjectionBuilder,
    Projector, Renderer, ScrollPane, Smooth, SmoothConfig, SoftBreakPolicy, StreamOffset,
    StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder, StreamingSource,
    TableColumnSizing, TaskListMarkerPolicy, TextContent, TextInput, TextRenderPolicy,
    TextRenderer, TextStream as GenericTextStream, Theme, View, WrapMode,
    backend::NativeHistorySink,
    geometry::Size,
    physical::PhysicalRow,
    scene::PreparedSceneFrame,
    terminal::{PresentReceipt, TerminalBackend, TerminalEvent, termwiz::TermwizBackend},
};

/// One caller-defined routed output produced by native interaction routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedOutput {
    pub route_id: String,
    pub payload: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostCellStyle {
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reversed: bool,
    pub strikethrough: bool,
}

#[derive(Debug)]
enum HostOutput {
    Routed(RoutedOutput),
}

struct HostState {
    body: View,
    outputs: VecDeque<RoutedOutput>,
}

fn host_init(_cx: &mut AppCx<'_, HostOutput>) -> Result<HostState> {
    Ok(HostState {
        body: View::spacer(0),
        outputs: VecDeque::new(),
    })
}

fn host_update(
    state: &mut HostState,
    action: HostOutput,
    _cx: &mut AppCx<'_, HostOutput>,
) -> Result<()> {
    match action {
        HostOutput::Routed(output) => state.outputs.push_back(output),
    }
    Ok(())
}

fn host_view(state: &HostState) -> View {
    state.body.clone()
}

type HostRunning = crate::application::kernel::RunningApp<
    HostState,
    HostOutput,
    anyhow::Error,
    fn(&mut HostState, HostOutput, &mut AppCx<'_, HostOutput>) -> Result<()>,
    fn(&HostState) -> View,
>;

#[derive(Default)]
struct HeadlessSink {
    width: u16,
    height: u16,
    history: Vec<PhysicalRow>,
}

impl NativeHistorySink for HeadlessSink {
    type Error = anyhow::Error;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        self.history.extend(rows.iter().cloned());
        Ok(rows.len())
    }
}

enum HostBackend {
    Headless(HeadlessSink),
    Real(TermwizBackend),
}

struct HostInner {
    running: HostRunning,
    backend: HostBackend,
    frame: PreparedSceneFrame,
    presentation: Option<PresentReceipt>,
    now: Instant,
    headless: bool,
    closed: bool,
}

/// A shared native TextInput value that can be mounted into one TuiHost.
#[derive(Clone)]
pub struct HostTextInput {
    state: Arc<Mutex<TextInput>>,
    component_id: Arc<Mutex<Option<u64>>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
}

#[derive(Clone)]
pub struct HostViewSlot {
    state: Arc<Mutex<ViewSlotState>>,
    component_id: Arc<Mutex<Option<u64>>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
}

/// A shared native scrolling viewport for live output.
#[derive(Clone)]
pub struct HostScrollPane {
    state: Arc<Mutex<ScrollPane>>,
    component_id: Arc<Mutex<Option<u64>>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
}

struct ViewSlotState {
    view: View,
    revision: u64,
    frames: Vec<View>,
    pending_frames: Option<Vec<View>>,
    frame_index: usize,
    interval: Duration,
    last_tick: Option<Instant>,
}

impl HostViewSlot {
    pub fn new(view: View) -> Self {
        Self {
            state: Arc::new(Mutex::new(ViewSlotState {
                view,
                revision: 0,
                frames: Vec::new(),
                pending_frames: None,
                frame_index: 0,
                interval: Duration::from_millis(480),
                last_tick: None,
            })),
            component_id: Arc::new(Mutex::new(None)),
            host: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_view(&self, view: View) -> Result<()> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("view slot lock is poisoned"))?;
            state.view = view;
            state.frames.clear();
            state.pending_frames = None;
            state.frame_index = 0;
            state.last_tick = None;
            state.revision = state.revision.saturating_add(1);
        }
        self.invalidate_host()
    }

    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    /// PERF-12 T13.1 R8: request deferred retirement of this slot's registry
    /// entry. Idempotent; a never-host-mounted slot (no component id) is a
    /// no-op. Physical reclamation happens in
    /// `RunningApp::reap_retired_components` after reconciliation proves the
    /// component unmounted.
    pub fn retire(&self) {
        let Some(raw_id) = self.component_id() else {
            return;
        };
        if let Ok(guard) = self.host.lock() {
            if let Some(weak) = guard.as_ref() {
                if let Some(inner) = weak.upgrade() {
                    if let Ok(mut inner) = inner.lock() {
                        inner.running.host_retire_component(raw_id);
                    }
                }
            }
        }
    }

    pub fn revision(&self) -> u64 {
        self.state.lock().map(|state| state.revision).unwrap_or(0)
    }

    pub fn set_animation(&self, frames: Vec<View>, interval: Duration) -> Result<()> {
        self.replace_animation(frames, interval)
    }

    /// Replace animation frames on the next cycle boundary while preserving
    /// the current frame until the native scheduler reaches frame zero.
    ///
    /// This is useful when a caller changes animation semantics without
    /// wanting a mid-cycle visual inversion. Rust retains the pending frames
    /// and applies them from the native tick path; callers do not schedule
    /// individual ticks through the binding.
    pub fn set_animation_at_cycle_boundary(
        &self,
        frames: Vec<View>,
        interval: Duration,
    ) -> Result<()> {
        if frames.is_empty() {
            return Err(anyhow::anyhow!(
                "view slot animation requires at least one frame"
            ));
        }
        let host_now = self.host_time();
        let mut invalidate = false;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("view slot lock is poisoned"))?;
            if state.frames.len() < 2 || state.interval != interval {
                let preserve_phase = !state.frames.is_empty() && state.interval == interval;
                state.frame_index = if preserve_phase {
                    state.frame_index % frames.len()
                } else {
                    0
                };
                state.view = frames[state.frame_index].clone();
                state.frames = frames;
                state.pending_frames = None;
                state.interval = interval;
                if !preserve_phase {
                    state.last_tick = Some(host_now.unwrap_or_else(Instant::now));
                }
                state.revision = state.revision.saturating_add(1);
                invalidate = true;
            } else {
                state.pending_frames = Some(frames);
            }
        }
        if invalidate {
            self.invalidate_host()
        } else {
            Ok(())
        }
    }

    fn replace_animation(&self, frames: Vec<View>, interval: Duration) -> Result<()> {
        if frames.is_empty() {
            return Err(anyhow::anyhow!(
                "view slot animation requires at least one frame"
            ));
        }
        // Read host time before taking the slot lock. Rendering holds the host
        // lock while resolving mounted components, which takes the slot lock;
        // acquiring them in the opposite order here can deadlock the runtime.
        let host_now = self.host_time();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("view slot lock is poisoned"))?;
            let preserve_phase = !state.frames.is_empty() && state.interval == interval;
            state.frame_index = if preserve_phase {
                state.frame_index % frames.len()
            } else {
                0
            };
            state.view = frames[state.frame_index].clone();
            state.frames = frames;
            state.pending_frames = None;
            state.interval = interval;
            if !preserve_phase {
                // Anchor the animation to the host clock. This keeps the
                // first scheduled tick from either catching up immediately
                // or delaying the first frame by one scheduler interval.
                state.last_tick = Some(host_now.unwrap_or_else(Instant::now));
            }
            state.revision = state.revision.saturating_add(1);
        }
        self.invalidate_host()
    }

    pub fn stop_animation(&self, view: View) -> Result<()> {
        self.set_view(view)
    }

    fn host_time(&self) -> Option<Instant> {
        self.host
            .lock()
            .ok()
            .and_then(|host| host.as_ref().and_then(Weak::upgrade))
            .and_then(|host| host.lock().ok().map(|inner| inner.now))
    }

    fn tick(&self, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.frames.len() < 2 {
            // Reset the clock so a future set_animation starts fresh
            // rather than inheriting a stale last_tick.
            state.last_tick = None;
            return false;
        }
        let Some(last) = state.last_tick else {
            // First tick with frames: start the clock now so the interval
            // is measured from the first scheduled tick, not from the
            // earlier set_animation call. This avoids catching up for the
            // scheduling delay.
            state.last_tick = Some(now);
            return true;
        };
        let due = now.duration_since(last) >= state.interval;
        if !due {
            return false;
        }
        state.last_tick = Some(now);
        state.frame_index = (state.frame_index + 1) % state.frames.len();
        if state.frame_index == 0 {
            if let Some(frames) = state.pending_frames.take() {
                state.frames = frames;
            }
        }
        state.view = state.frames[state.frame_index].clone();
        state.revision = state.revision.saturating_add(1);
        true
    }

    fn attach_host(&self, host: &Arc<Mutex<HostInner>>) -> Result<()> {
        *self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("view slot host lock is poisoned"))? =
            Some(Arc::downgrade(host));
        Ok(())
    }

    fn set_component_id(&self, id: u64) -> Result<()> {
        *self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("view slot component lock is poisoned"))? = Some(id);
        Ok(())
    }

    fn invalidate_host(&self) -> Result<()> {
        let host = self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("view slot host lock is poisoned"))?
            .clone()
            .and_then(|host| host.upgrade());
        let Some(host) = host else {
            return Ok(());
        };
        let component_id = self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("view slot component lock is poisoned"))?
            .to_owned();
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        if let Some(component_id) = component_id {
            inner.running.host_invalidate_component(component_id);
        } else {
            inner.running.invalidate_frame();
        }
        inner.advance_and_render()?;
        Ok(())
    }
}

impl HostScrollPane {
    pub fn new(content: View) -> Self {
        Self {
            state: Arc::new(Mutex::new(ScrollPane::new(content))),
            component_id: Arc::new(Mutex::new(None)),
            host: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_content(&self, content: View) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane lock is poisoned"))?
            .set_content(content);
        self.invalidate_host()
    }

    pub fn follow_end(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane lock is poisoned"))?
            .follow_end();
        self.invalidate_host()
    }

    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    /// PERF-12 T13.1 R8: see `HostViewSlot::retire`.
    pub fn retire(&self) {
        let Some(raw_id) = self.component_id() else {
            return;
        };
        if let Ok(guard) = self.host.lock() {
            if let Some(weak) = guard.as_ref() {
                if let Some(inner) = weak.upgrade() {
                    if let Ok(mut inner) = inner.lock() {
                        inner.running.host_retire_component(raw_id);
                    }
                }
            }
        }
    }

    fn attach_host(&self, host: &Arc<Mutex<HostInner>>) -> Result<()> {
        *self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane host lock is poisoned"))? =
            Some(Arc::downgrade(host));
        Ok(())
    }

    fn set_component_id(&self, id: u64) -> Result<()> {
        *self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane component lock is poisoned"))? = Some(id);
        Ok(())
    }

    fn invalidate_host(&self) -> Result<()> {
        let host = self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane host lock is poisoned"))?
            .clone()
            .and_then(|host| host.upgrade());
        let Some(host) = host else {
            return Ok(());
        };
        let component_id = self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("scroll pane component lock is poisoned"))?
            .to_owned();
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        if let Some(component_id) = component_id {
            inner.running.host_invalidate_component(component_id);
        } else {
            inner.running.invalidate_frame();
        }
        inner.advance_and_render()?;
        Ok(())
    }
}

struct MountedScrollPane(HostScrollPane);

impl Component for MountedScrollPane {
    fn view(&self) -> View {
        self.0
            .state
            .lock()
            .map(|pane| Component::view(&*pane))
            .unwrap_or_else(|_| View::spacer(0))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_layout_changed(Self::on_layout_changed);
        cx.key_commands(Self::map_command, Self::handle_command);
    }
}

impl MountedScrollPane {
    fn on_layout_changed(component: &mut Self, size: Size) {
        if let Ok(mut pane) = component.0.state.lock() {
            pane.on_layout_changed(size);
        }
    }

    fn map_command(
        component: &Self,
        key: KeyStroke,
    ) -> Option<crate::scroll_command::ScrollCommand> {
        component.0.state.lock().ok()?.map_command(key)
    }

    fn handle_command(
        component: &mut Self,
        command: crate::scroll_command::ScrollCommand,
        cx: &mut crate::EventCx<'_>,
    ) -> InteractionResult {
        component
            .0
            .state
            .lock()
            .map(|mut pane| pane.handle_command(command, cx))
            .unwrap_or(InteractionResult::Ignored)
    }
}

struct MountedViewSlot(HostViewSlot);

impl Component for MountedViewSlot {
    fn view(&self) -> View {
        self.0
            .state
            .lock()
            .map(|state| state.view.clone())
            .unwrap_or_else(|_| View::spacer(0))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.tick(Duration::from_millis(16), Self::tick);
    }
}

impl MountedViewSlot {
    fn tick(component: &mut Self, now: Instant, _cx: &mut crate::EventCx<'_>) -> bool {
        component.0.tick(now)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextStreamAnnotation {
    pub namespace: String,
    pub name: String,
}

#[derive(Clone, Debug)]
struct HostSourceChunk {
    range: StreamRange,
    annotations: Vec<SemanticTag>,
    text: Arc<str>,
}

impl HostSourceChunk {
    fn pacing_atoms(&self) -> impl Iterator<Item = (StreamRange, HostPacingAtom)> + '_ {
        self.text.grapheme_indices(true).map(|(start, grapheme)| {
            let end = start + grapheme.len();
            (
                StreamRange::new(
                    self.range.start().saturating_add(start as u64),
                    self.range.start().saturating_add(end as u64),
                ),
                HostPacingAtom {
                    annotations: self.annotations.clone(),
                    source: Arc::clone(&self.text),
                    local: start..end,
                },
            )
        })
    }

    fn suffix_from(&self, offset: StreamOffset) -> Option<Self> {
        if self.range.end() <= offset {
            return None;
        }
        if offset <= self.range.start() {
            return Some(self.clone());
        }
        let local = usize::try_from(offset.as_u64().saturating_sub(self.range.start().as_u64()))
            .expect("host source chunk offset fits usize");
        assert!(
            self.text.is_char_boundary(local),
            "host source compaction must use a UTF-8 boundary"
        );
        Some(Self {
            range: StreamRange::new(offset, self.range.end()),
            annotations: self.annotations.clone(),
            text: Arc::from(&self.text[local..]),
        })
    }
}

#[derive(Clone, Debug)]
struct HostPacingAtom {
    annotations: Vec<SemanticTag>,
    source: Arc<str>,
    local: Range<usize>,
}

impl HostPacingAtom {
    fn text(&self) -> &str {
        &self.source[self.local.clone()]
    }
}

struct HostTextPipeline {
    smoother: Smooth,
    markdown: MarkdownProjector,
    renderer: TextRenderer,
    paced: Projection<HostPacingAtom>,
}

impl HostTextPipeline {
    fn new(pacing: SmoothConfig) -> Self {
        let policy = TextRenderPolicy::new()
            .with_block_gap(1)
            .with_soft_break(SoftBreakPolicy::LineBreak)
            .with_table_column_sizing(TableColumnSizing::Content)
            .with_table_column_gap(1)
            .with_table_row_gap(0)
            .with_task_list_marker(TaskListMarkerPolicy::TaskOnly)
            .with_code_block_label(CodeBlockLabelPolicy::Language)
            .with_code_block_gap(0)
            .with_code_wrap(WrapMode::NoWrap);
        let paced = ProjectionBuilder::new(
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            false,
        )
        .finish()
        .expect("empty host paced projection is valid");
        Self {
            smoother: Smooth::new(pacing),
            markdown: MarkdownProjector::new(
                MarkdownOptions::gfm().with_live_table_stabilization(true),
            ),
            renderer: TextRenderer::with_policy(policy),
            paced,
        }
    }

    fn reset(&mut self) {
        let pacing = self.smoother.config();
        *self = Self::new(pacing);
    }

    fn project(&mut self, input: &Projection<HostPacingAtom>) -> Result<Projection<TextContent>> {
        let previous_end = self.paced.source_end();
        let reset =
            self.paced.source_base() != input.source_base() || previous_end > input.source_end();
        let from = if reset {
            input.source_base()
        } else {
            previous_end
        };
        let delta = self.smoother.project_incremental(input, from);
        if reset {
            self.paced = delta;
        } else {
            for span in delta.spans() {
                self.paced
                    .append_span_many(span.source(), span.values().iter().cloned())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            }
            self.paced
                .set_envelope(delta.stable_through(), delta.is_sealed());
        }
        let paced = &self.paced;
        let raw = paced.map_ref(|atom| TextContent::raw(atom.text()));
        let markdown = self
            .markdown
            .project(&raw)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let mut pipe_tables = PipeTableRewriter;
        let markdown = pipe_tables
            .project(&markdown)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        AnnotationRewriter::new(paced)
            .into_projector()
            .project(&markdown)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn advance(&mut self, now: Instant) -> bool {
        self.smoother.advance(now)
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.smoother.next_wakeup()
    }

    fn restart_from(&self, offset: StreamOffset) -> StreamOffset {
        self.markdown.restart_from(offset)
    }
}

struct AnnotationMap(Vec<(StreamRange, Vec<SemanticTag>)>);

impl AnnotationMap {
    fn new(projection: &Projection<HostPacingAtom>) -> Self {
        let mut ranges: Vec<(StreamRange, Vec<SemanticTag>)> = Vec::new();
        for span in projection.spans() {
            let Some(atom) = span.values().first() else {
                continue;
            };
            if let Some((last, annotations)) = ranges.last_mut()
                && *annotations == atom.annotations
                && last.end() == span.source().start()
            {
                *last = StreamRange::new(last.start(), span.source().end());
            } else {
                ranges.push((span.source(), atom.annotations.clone()));
            }
        }
        Self(ranges)
    }
}

struct AnnotationRewriter {
    map: AnnotationMap,
}

impl AnnotationRewriter {
    fn new(paced: &Projection<HostPacingAtom>) -> Self {
        Self {
            map: AnnotationMap::new(paced),
        }
    }

    fn apply_annotations(run: TextRun, annotations: &[SemanticTag]) -> TextRun {
        run.map_annotations(|current| {
            annotations
                .iter()
                .cloned()
                .fold(current, |current, tag| current.with_tag(tag))
        })
    }

    fn annotate_run(&self, run: TextRun) -> Result<Vec<TextRun>, TextIrError> {
        let range = match run.provenance() {
            TextProvenance::Exact(range) | TextProvenance::Derived(range) => *range,
            TextProvenance::Synthetic => return Ok(vec![run]),
        };
        if range.is_empty() {
            return Ok(vec![run]);
        }
        let mut result = Vec::new();
        let derived = matches!(run.provenance(), TextProvenance::Derived(_));
        let mut remaining = run;
        let mut cursor = range.start();
        while !remaining.text().is_empty() {
            let Some((map_range, annotations)) = self
                .map
                .0
                .iter()
                .find(|(candidate, _)| candidate.contains_offset(cursor))
            else {
                return Ok(vec![remaining]);
            };
            let end = map_range.end().min(range.end());
            let source_delta = end.as_u64().saturating_sub(cursor.as_u64());
            let source_span = range.len().max(1);
            let mut length = if !derived {
                usize::try_from(source_delta).expect("stream range fits usize")
            } else {
                usize::try_from(
                    source_delta
                        .saturating_mul(range.len())
                        .checked_div(source_span)
                        .unwrap_or_default(),
                )
                .expect("derived stream range fits usize")
            };
            length = length.min(remaining.text().len());
            while length > 0 && !remaining.text().is_char_boundary(length) {
                length -= 1;
            }
            if length == 0 && end < range.end() {
                // A transformed run may begin with a multi-byte grapheme whose
                // source range is smaller than the display text. Keep it whole
                // rather than asking TextRun to split at an invalid boundary.
                result.push(Self::apply_annotations(remaining, annotations));
                break;
            }
            let (piece, rest) = if length == remaining.text().len() {
                (remaining, None)
            } else {
                let (left, right) = remaining.split_at(length)?;
                (left, Some(right))
            };
            result.push(Self::apply_annotations(piece, annotations));
            cursor = end;
            let Some(next) = rest else { break };
            remaining = next;
        }
        Ok(result)
    }
}

impl TextRewriter for AnnotationRewriter {
    type Error = TextIrError;

    fn rewrite_inline(&mut self, inline: Inline) -> Result<Inline, Self::Error> {
        let InlineKind::Text(run) = inline.kind().clone() else {
            return walk_rewrite_inline(self, inline);
        };
        let Some(run) = self.annotate_run(run)?.into_iter().next() else {
            return Ok(inline);
        };
        Ok(Inline::new(InlineKind::Text(run))
            .with_marks(inline.marks().clone())
            .with_annotations(inline.annotations().clone()))
    }

    fn rewrite_inline_content(
        &mut self,
        content: InlineContent,
    ) -> Result<InlineContent, Self::Error> {
        let mut items = Vec::new();
        for inline in content.items() {
            let InlineKind::Text(run) = inline.kind() else {
                items.push(self.rewrite_inline(inline.clone())?);
                continue;
            };
            for run in self.annotate_run(run.clone())? {
                items.push(
                    Inline::new(InlineKind::Text(run))
                        .with_marks(inline.marks().clone())
                        .with_annotations(inline.annotations().clone()),
                );
            }
        }
        Ok(InlineContent::new(items))
    }

    fn rewrite_literal(&mut self, literal: LiteralText) -> Result<LiteralText, Self::Error> {
        let mut runs = Vec::new();
        for run in literal.runs() {
            runs.extend(self.annotate_run(run.clone())?);
        }
        Ok(LiteralText::new(runs))
    }
}

struct PipeTableRewriter;

impl Projector<TextContent> for PipeTableRewriter {
    type Output = TextContent;
    type Error = RewriteProjectionError<TextIrError>;

    fn project(
        &mut self,
        input: &Projection<TextContent>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let mut output = input.rebuild();
        let spans = input.spans();
        for (index, span) in spans.iter().enumerate() {
            let following = input.is_sealed()
                || spans[index + 1..]
                    .iter()
                    .any(|later| !later.values().is_empty());
            let values = span
                .values()
                .iter()
                .cloned()
                .enumerate()
                .map(|(value_index, value)| {
                    let closed = following || value_index + 1 < span.values().len();
                    rewrite_pipe_content(value, closed).map_err(RewriteProjectionError::Rewrite)
                })
                .collect::<Result<Vec<_>, _>>()?;
            output = output.emit_many(span.source(), values);
        }
        let output = output.finish().map_err(|error| {
            RewriteProjectionError::Invalid(TextProjectionError::Projection(error))
        })?;
        validate_text_projection(&output).map_err(RewriteProjectionError::Invalid)?;
        Ok(output)
    }
}

fn rewrite_pipe_content(content: TextContent, closed: bool) -> Result<TextContent, TextIrError> {
    match content {
        TextContent::Raw(_) => Ok(content),
        TextContent::Block(block) => Ok(TextContent::Block(rewrite_pipe_block(block, closed)?)),
    }
}

fn rewrite_pipe_block(block: Block, closed: bool) -> Result<Block, TextIrError> {
    match block.kind() {
        BlockKind::Paragraph(content) if closed => {
            if let Some(table) = pipe_table_from_paragraph(content) {
                return Ok(Block::table(table).with_annotations(block.annotations().clone()));
            }
            Ok(block)
        }
        BlockKind::BlockQuote { blocks } => {
            let rewritten = rewrite_pipe_blocks(blocks, closed)?;
            if rewritten.as_slice() == blocks.as_ref() {
                return Ok(block);
            }
            Ok(Block::block_quote(rewritten).with_annotations(block.annotations().clone()))
        }
        BlockKind::Container { blocks } => {
            let rewritten = rewrite_pipe_blocks(blocks, closed)?;
            if rewritten.as_slice() == blocks.as_ref() {
                return Ok(block);
            }
            Ok(Block::container(rewritten).with_annotations(block.annotations().clone()))
        }
        BlockKind::List(list) => {
            let mut changed = false;
            let mut items = Vec::with_capacity(list.items().len());
            for (index, item) in list.items().iter().enumerate() {
                let blocks =
                    rewrite_pipe_blocks(item.blocks(), index + 1 < list.items().len() || closed)?;
                changed |= blocks.as_slice() != item.blocks();
                items.push(
                    ListItem::new(blocks)
                        .with_annotations(item.annotations().clone())
                        .with_checked(item.checked()),
                );
            }
            if !changed {
                return Ok(block);
            }
            Ok(Block::list(List::new(list.marker(), list.tight(), items))
                .with_annotations(block.annotations().clone()))
        }
        _ => Ok(block),
    }
}

fn rewrite_pipe_blocks(blocks: &[Block], trailing_closed: bool) -> Result<Vec<Block>, TextIrError> {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            rewrite_pipe_block(block.clone(), index + 1 < blocks.len() || trailing_closed)
        })
        .collect()
}

fn pipe_table_from_paragraph(content: &InlineContent) -> Option<Table> {
    let lines = pipe_lines(content)?;
    let mut rows: Vec<Vec<String>> = lines
        .iter()
        .map(|line| split_pipe_cells(line))
        .collect::<Option<_>>()?;
    let width = rows.first()?.len();
    if width < 2 {
        return None;
    }
    for row in &mut rows {
        row.truncate(width);
        while row.len() < width {
            row.push(String::new());
        }
    }
    let (header_rows, alignments, body) = if rows.len() >= 3 && is_pipe_delimiter_row(&rows[1]) {
        (
            1,
            rows[1].iter().map(pipe_alignment).collect(),
            std::iter::once(rows[0].clone())
                .chain(rows[2..].iter().cloned())
                .collect::<Vec<_>>(),
        )
    } else if rows.iter().any(|row| is_pipe_delimiter_row(row)) {
        return None;
    } else if rows.len() >= 2 {
        (0, vec![Alignment::Start; width], rows)
    } else {
        return None;
    };
    let range = pipe_covering_range(content);
    let columns = alignments.into_iter().map(TableColumn::new);
    let table_rows = body.into_iter().map(|cells| {
        TableRow::new(
            cells
                .into_iter()
                .map(|cell| TableCell::text(pipe_cell_run(cell, range)))
                .collect::<Vec<_>>(),
        )
    });
    Table::new(None::<Vec<Block>>, columns, header_rows, table_rows).ok()
}

fn pipe_lines(content: &InlineContent) -> Option<Vec<String>> {
    let mut lines = vec![String::new()];
    let mut last_exact_end = None;
    for inline in content.items() {
        match inline.kind() {
            InlineKind::Break(_) => {
                last_exact_end = None;
                lines.push(String::new());
            }
            InlineKind::Text(run) => {
                if inline.marks().contains(&Mark::Code) {
                    return None;
                }
                match run.provenance() {
                    TextProvenance::Derived(_) => return None,
                    TextProvenance::Exact(range) => {
                        if last_exact_end.is_some_and(|end| range.start() > end)
                            && run.text().contains('|')
                        {
                            return None;
                        }
                        last_exact_end = Some(range.end());
                    }
                    TextProvenance::Synthetic => last_exact_end = None,
                }
                lines.last_mut()?.push_str(run.text());
            }
            _ => return None,
        }
    }
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    (!lines.is_empty() && !lines.iter().any(|line| line.trim().is_empty())).then_some(lines)
}

fn split_pipe_cells(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if line.len() < 3
        || !line.starts_with('|')
        || !line.ends_with('|')
        || line.contains('\\')
        || line.contains('`')
    {
        return None;
    }
    Some(
        line[1..line.len() - 1]
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect(),
    )
}

fn is_pipe_delimiter_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let core = cell.trim_matches(':');
            !core.is_empty() && core.chars().all(|character| character == '-')
        })
}

fn pipe_alignment(cell: &String) -> Alignment {
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Alignment::Center,
        (false, true) => Alignment::End,
        _ => Alignment::Start,
    }
}

fn pipe_covering_range(content: &InlineContent) -> Option<StreamRange> {
    let mut start: Option<StreamOffset> = None;
    let mut end: Option<StreamOffset> = None;
    for inline in content.items() {
        let Some(run) = inline.as_text() else {
            continue;
        };
        let range = match run.provenance() {
            TextProvenance::Exact(range) | TextProvenance::Derived(range) => *range,
            TextProvenance::Synthetic => continue,
        };
        start = Some(start.map_or(range.start(), |value| value.min(range.start())));
        end = Some(end.map_or(range.end(), |value| value.max(range.end())));
    }
    Some(StreamRange::new(start?, end?))
}

fn pipe_cell_run(text: String, range: Option<StreamRange>) -> TextRun {
    match range {
        Some(range) => TextRun::derived(text, range),
        None => TextRun::synthetic(text),
    }
}

struct HostStreamState {
    /// Plain streams use the generic PERF-5 source directly. This keeps the
    /// append path out of the host's Markdown/annotation adapter and lets
    /// StreamModel own stable-prefix promotion and source compaction.
    plain: Option<GenericTextStream>,
    source_chunks: Vec<HostSourceChunk>,
    source_base: StreamOffset,
    received_end: StreamOffset,
    revision: StreamRevision,
    sealed: bool,
    pipeline: Option<HostTextPipeline>,
    pacing_input: Projection<HostPacingAtom>,
    semantic: Option<Projection<TextContent>>,
    presentation: TextStreamPresentation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStreamPresentation {
    pub insets: crate::Insets,
    pacing: SmoothConfig,
}

impl TextStreamPresentation {
    pub const fn new(insets: crate::Insets) -> Self {
        Self {
            insets,
            pacing: SmoothConfig::new(),
        }
    }

    pub fn with_pacing(mut self, pacing: SmoothConfig) -> Self {
        self.pacing = pacing;
        self
    }

    pub const fn pacing(self) -> SmoothConfig {
        self.pacing
    }
}

impl Default for HostStreamState {
    fn default() -> Self {
        let pacing_input = ProjectionBuilder::new(
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            false,
        )
        .finish()
        .expect("empty host pacing projection is valid");
        Self {
            plain: Some(GenericTextStream::new()),
            source_chunks: Vec::new(),
            source_base: StreamOffset::ZERO,
            received_end: StreamOffset::ZERO,
            revision: StreamRevision::ZERO,
            sealed: false,
            pipeline: None,
            pacing_input,
            semantic: None,
            presentation: TextStreamPresentation::new(crate::Insets::ZERO),
        }
    }
}

/// A mutable native stream shared by a History unit and its language binding.
#[derive(Clone)]
pub struct HostTextStream {
    state: Arc<Mutex<HostStreamState>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
    handle: Arc<Mutex<Option<HistoryStreamHandle<HostStreamSource>>>>,
}

impl HostTextStream {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HostStreamState::default())),
            host: Arc::new(Mutex::new(None)),
            handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_markdown() -> Self {
        Self::with_markdown_presentation(TextStreamPresentation::new(crate::Insets::ZERO))
    }

    pub fn with_markdown_presentation(presentation: TextStreamPresentation) -> Self {
        let stream = Self::new();
        if let Ok(mut state) = stream.state.lock() {
            state.presentation = presentation;
            state.plain = None;
            state.pipeline = Some(HostTextPipeline::new(presentation.pacing()));
            state
                .refresh_semantic()
                .expect("empty host semantic stream is valid");
        }
        stream
    }

    pub fn append(
        &self,
        text: impl AsRef<str>,
        annotations: &[TextStreamAnnotation],
    ) -> Result<()> {
        let text = text.as_ref();
        if text.is_empty() {
            return Ok(());
        }
        let annotations = annotations
            .iter()
            .map(|annotation| {
                SemanticTag::new(annotation.namespace.clone(), annotation.name.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))
            })
            .collect::<Result<Vec<_>>>()?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("stream lock is poisoned"))?;
            if state.sealed {
                return Err(anyhow::anyhow!("stream is already sealed"));
            }
            if state.pipeline.is_none() && annotations.is_empty() && state.plain.is_some() {
                state
                    .plain
                    .as_mut()
                    .expect("plain stream is present when the host has no adapter")
                    .push(text);
                state.sync_plain_metadata();
            } else {
                if state.pipeline.is_none() && state.plain.is_some() {
                    state.switch_plain_to_annotated_source()?;
                }
                let range = StreamRange::new(
                    state.received_end,
                    state.received_end.saturating_add(text.len() as u64),
                );
                let chunk = HostSourceChunk {
                    range,
                    annotations,
                    text: Arc::from(text),
                };
                for (atom_range, atom) in chunk.pacing_atoms() {
                    state
                        .pacing_input
                        .append_span(atom_range, atom)
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                }
                state.source_chunks.push(chunk);
                state.received_end = range.end();
                state.revision = state.revision.next();
                state.refresh_semantic()?;
            }
        }
        self.render_host()
    }

    pub fn update(&self, text: impl Into<String>) -> Result<()> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("stream lock is poisoned"))?;
            if state.sealed {
                return Err(anyhow::anyhow!("stream is already sealed"));
            }
            let text = text.into();
            if state.pipeline.is_none() {
                state.plain = Some(GenericTextStream::from_text(text));
                state.sync_plain_metadata();
                state.source_chunks.clear();
            } else {
                if let Some(pipeline) = &mut state.pipeline {
                    pipeline.reset();
                }
                state.plain = None;
                state.source_chunks.clear();
                state.source_base = StreamOffset::ZERO;
                state.received_end = StreamOffset::new(text.len() as u64);
                if !text.is_empty() {
                    let chunk = HostSourceChunk {
                        range: StreamRange::new(StreamOffset::ZERO, state.received_end),
                        annotations: Vec::new(),
                        text: Arc::from(text.as_str()),
                    };
                    state.source_chunks.push(chunk.clone());
                }
                state.rebuild_pacing_input()?;
                state.revision = state.revision.next();
                state.refresh_semantic()?;
            }
        }
        self.render_host()
    }

    pub fn seal(&self) -> Result<()> {
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("stream lock is poisoned"))?;
            if state.sealed {
                return Err(anyhow::anyhow!("stream is already sealed"));
            }
            state.sealed = true;
            if state.plain.is_some() {
                state
                    .plain
                    .as_mut()
                    .expect("plain stream is present when sealing")
                    .seal();
                state.sync_plain_metadata();
            } else {
                let source_end = state.received_end;
                state.pacing_input.set_envelope(source_end, true);
                state.revision = state.revision.next();
                state.refresh_semantic()?;
            }
        }
        self.render_host()
    }

    pub fn snapshot_json(
        &self,
    ) -> Result<(String, u64, bool, Vec<(Vec<TextStreamAnnotation>, String)>)> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("stream lock is poisoned"))?;
        let segments = state.segments_json();
        Ok((
            state.source_text(),
            state.revision.as_u64(),
            state.sealed,
            segments,
        ))
    }

    pub fn attach(&self, history: &mut History) -> Result<()> {
        let handle = history
            .push_stream(HostStreamSource {
                state: self.state.clone(),
            })
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        *self
            .handle
            .lock()
            .map_err(|_| anyhow::anyhow!("stream handle lock is poisoned"))? = Some(handle);
        Ok(())
    }

    pub fn seal_history(&self, history: &mut History) -> Result<()> {
        let handle = self
            .handle
            .lock()
            .map_err(|_| anyhow::anyhow!("stream handle lock is poisoned"))?
            .as_ref()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("stream is not attached to History"))?;
        history
            .seal_stream(handle)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn attach_host(&self, host: &Arc<Mutex<HostInner>>) -> Result<()> {
        *self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("stream host lock is poisoned"))? =
            Some(Arc::downgrade(host));
        Ok(())
    }

    fn render_host(&self) -> Result<()> {
        let host = self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("stream host lock is poisoned"))?
            .clone()
            .and_then(|host| host.upgrade());
        if let Some(host) = host {
            let mut inner = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
            if let Some(handle) = self
                .handle
                .lock()
                .map_err(|_| anyhow::anyhow!("stream handle lock is poisoned"))?
                .as_ref()
                .copied()
            {
                inner
                    .running
                    .scene_history_mut()
                    .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
                    .refresh_stream(handle)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            }
            inner.running.invalidate_frame();
            inner.advance_and_render()?;
        }
        Ok(())
    }
}

impl HostStreamState {
    fn sync_plain_metadata(&mut self) {
        let Some(plain) = self.plain.as_ref() else {
            return;
        };
        self.source_base = plain.source_base();
        self.received_end = plain.source_end();
        self.revision = plain.revision();
    }

    fn switch_plain_to_annotated_source(&mut self) -> Result<()> {
        let Some(plain) = self.plain.take() else {
            return Ok(());
        };
        self.source_base = plain.source_base();
        self.received_end = plain.source_end();
        self.revision = plain.revision();
        self.source_chunks.clear();
        let text = plain.retained_text();
        if !text.is_empty() {
            self.source_chunks.push(HostSourceChunk {
                range: StreamRange::new(self.source_base, self.received_end),
                annotations: Vec::new(),
                text: Arc::from(text),
            });
        }
        self.rebuild_pacing_input()
    }

    fn source_text(&self) -> String {
        if let Some(plain) = &self.plain {
            return plain.retained_text().to_owned();
        }
        self.source_chunks
            .iter()
            .map(|chunk| chunk.text.as_ref())
            .collect()
    }

    fn segments_json(&self) -> Vec<(Vec<TextStreamAnnotation>, String)> {
        if self.plain.is_some() {
            return Vec::new();
        }
        let mut segments: Vec<(Vec<TextStreamAnnotation>, String)> = Vec::new();
        for chunk in &self.source_chunks {
            let atom = chunk;
            let annotations = atom
                .annotations
                .iter()
                .map(|tag| TextStreamAnnotation {
                    namespace: tag.namespace().to_owned(),
                    name: tag.name().to_owned(),
                })
                .collect::<Vec<_>>();
            if let Some((previous, text)) = segments.last_mut()
                && *previous == annotations
            {
                text.push_str(&atom.text);
            } else {
                segments.push((annotations, atom.text.to_string()));
            }
        }
        segments
    }

    fn rebuild_pacing_input(&mut self) -> Result<()> {
        let mut builder = ProjectionBuilder::new(
            self.source_base,
            self.received_end,
            self.received_end,
            self.sealed,
        );
        for chunk in &self.source_chunks {
            for (range, atom) in chunk.pacing_atoms() {
                builder = builder.emit(range, atom);
            }
        }
        self.pacing_input = builder
            .finish()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    fn refresh_semantic(&mut self) -> Result<()> {
        if let Some(pipeline) = &mut self.pipeline {
            self.semantic = Some(pipeline.project(&self.pacing_input)?);
        }
        Ok(())
    }

    fn advance(&mut self, now: Instant) -> bool {
        let Some(pipeline) = &mut self.pipeline else {
            return false;
        };
        if !pipeline.advance(now) {
            return false;
        }
        self.revision = self.revision.next();
        self.refresh_semantic().is_ok()
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.pipeline
            .as_ref()
            .and_then(HostTextPipeline::next_wakeup)
    }

    fn snapshot(&self) -> StreamSnapshot {
        if let Some(plain) = &self.plain {
            return plain.snapshot();
        }
        let source_end = self.received_end;
        let Some(semantic) = &self.semantic else {
            let range = StreamRange::new(self.source_base, source_end);
            let builder = StreamSnapshotBuilder::new(
                self.revision,
                self.source_base,
                if self.sealed {
                    source_end
                } else {
                    self.source_base
                },
                source_end,
            );
            return if self.source_text().is_empty() {
                builder
                    .exact_text(range, [])
                    .finish()
                    .expect("host plain snapshot is valid")
            } else {
                builder
                    .atomic(range, View::text(self.source_text()).into_view())
                    .expect("host plain stream coverage is valid")
                    .finish()
                    .expect("host plain snapshot is valid")
            };
        };
        let visible: Vec<_> = semantic
            .spans()
            .iter()
            .filter(|span| !span.values().is_empty())
            .collect();
        let mut builder = StreamSnapshotBuilder::new(
            self.revision,
            semantic.source_base(),
            semantic.stable_through(),
            semantic.source_end(),
        );
        if visible.is_empty() {
            if semantic.source_base() != semantic.source_end() {
                builder = builder
                    .atomic(
                        StreamRange::new(semantic.source_base(), semantic.source_end()),
                        View::spacer(0).into_view(),
                    )
                    .expect("host empty semantic coverage is valid");
            }
        } else {
            for (index, span) in visible.iter().enumerate() {
                let start = if index == 0 {
                    semantic.source_base()
                } else {
                    visible[index - 1].source().end()
                };
                let end = if index + 1 == visible.len() {
                    span.source().end()
                } else {
                    span.source().end()
                };
                let gap = host_chunk_bottom_gap(
                    span.values(),
                    visible.get(index + 1).map(|next| next.values()),
                    self.pipeline
                        .as_ref()
                        .expect("semantic pipeline exists")
                        .renderer
                        .policy()
                        .block_gap(),
                    semantic.is_sealed(),
                );
                let view = Renderer::render(
                    &self
                        .pipeline
                        .as_ref()
                        .expect("semantic pipeline exists")
                        .renderer,
                    span.values(),
                )
                .padding(crate::Insets::new(
                    self.presentation.insets.top(),
                    self.presentation.insets.right(),
                    gap + self.presentation.insets.bottom(),
                    self.presentation.insets.left(),
                ));
                builder = builder
                    .atomic(StreamRange::new(start, end), view.into_view())
                    .expect("host semantic coverage is valid");
            }
        }
        if let Some(last) = visible.last()
            && last.source().end() < semantic.source_end()
        {
            builder = builder
                .atomic(
                    StreamRange::new(last.source().end(), semantic.source_end()),
                    View::spacer(0).into_view(),
                )
                .expect("host semantic trailing coverage is valid");
        }
        builder.finish().expect("host semantic snapshot is valid")
    }
}

fn host_chunk_bottom_gap(
    current: &[TextContent],
    next: Option<&[TextContent]>,
    block_gap: u16,
    sealed: bool,
) -> u16 {
    let current_list = current.last().and_then(|value| match value {
        TextContent::Block(block) => block.as_list(),
        TextContent::Raw(_) => None,
    });
    let next_list = next.and_then(|values| {
        values.first().and_then(|value| match value {
            TextContent::Block(block) => block.as_list(),
            TextContent::Raw(_) => None,
        })
    });
    if let (Some(left), Some(right)) = (current_list, next_list)
        && same_host_list_kind(left.marker(), right.marker())
    {
        return if left.tight() && right.tight() {
            0
        } else {
            block_gap
        };
    }
    if next.is_none() {
        return if current_list.is_some() && sealed {
            block_gap
        } else {
            0
        };
    }
    block_gap
}

fn same_host_list_kind(left: ListMarker, right: ListMarker) -> bool {
    match (left, right) {
        (ListMarker::Bullet, ListMarker::Bullet) => true,
        (
            ListMarker::Ordered {
                style: left_style,
                delimiter: left_delimiter,
                ..
            },
            ListMarker::Ordered {
                style: right_style,
                delimiter: right_delimiter,
                ..
            },
        ) => left_style == right_style && left_delimiter == right_delimiter,
        _ => false,
    }
}

#[derive(Clone)]
struct HostStreamSource {
    state: Arc<Mutex<HostStreamState>>,
}

impl StreamingSource for HostStreamSource {
    fn snapshot(&self) -> StreamSnapshot {
        let state = self.state.lock().expect("host stream lock is poisoned");
        state.snapshot()
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        let mut state = self.state.lock().expect("host stream lock is poisoned");
        if state.plain.is_some() {
            state
                .plain
                .as_mut()
                .expect("plain stream is present while compacting")
                .compact_before(offset);
            state.sync_plain_metadata();
            return;
        }
        let target = offset.min(state.received_end);
        if target <= state.source_base {
            return;
        }
        let restart = state
            .pipeline
            .as_ref()
            .map_or(target, |pipeline| pipeline.restart_from(target))
            .max(state.source_base);
        state.source_chunks = state
            .source_chunks
            .iter()
            .filter_map(|chunk| chunk.suffix_from(restart))
            .collect();
        state.source_base = restart;
        // Keep the pipeline's published suffix across compaction. Its next
        // incremental project sees the new source base and reconstructs the
        // retained paced suffix; resetting the smoother here would publish
        // only its first grapheme and regress the semantic source end.
        state
            .rebuild_pacing_input()
            .expect("host stream compaction must rebuild a valid source projection");
        state.revision = state.revision.next();
        state
            .refresh_semantic()
            .expect("host stream compaction must preserve projection coverage");
    }

    fn seal(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if !state.sealed {
                state.sealed = true;
                if state.plain.is_some() {
                    state
                        .plain
                        .as_mut()
                        .expect("plain stream is present while sealing")
                        .seal();
                    state.sync_plain_metadata();
                } else {
                    let source_end = state.received_end;
                    state.pacing_input.set_envelope(source_end, true);
                    state.revision = state.revision.next();
                    state
                        .refresh_semantic()
                        .expect("host stream sealing must preserve projection coverage");
                }
            }
        }
    }

    fn is_sealed(&self) -> bool {
        self.state.lock().map(|state| state.sealed).unwrap_or(true)
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.state.lock().ok().and_then(|state| state.next_wakeup())
    }

    fn advance(&mut self, now: Instant) -> bool {
        self.state
            .lock()
            .map(|mut state| state.advance(now))
            .unwrap_or(false)
    }
}

impl HostTextInput {
    pub fn new(multiline: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(TextInput::new().multiline(multiline))),
            component_id: Arc::new(Mutex::new(None)),
            host: Arc::new(Mutex::new(None)),
        }
    }

    pub fn text(&self) -> Result<String> {
        Ok(self.lock()?.text().to_owned())
    }

    pub fn cursor_bytes(&self) -> Result<usize> {
        Ok(self.lock()?.cursor_bytes())
    }

    pub fn set_text(&self, value: impl AsRef<str>) -> Result<()> {
        self.lock()?.set_text(value);
        self.render_host()
    }

    pub fn clear(&self) -> Result<()> {
        self.lock()?.clear();
        self.render_host()
    }

    pub fn set_border(&self, border: BorderSpec) -> Result<()> {
        self.lock()?.set_border(border);
        self.render_host()
    }

    pub fn submitted(&self) -> Result<Output<String>> {
        Ok(self.lock()?.submitted())
    }

    pub fn set_multiline(&self, enabled: bool) -> Result<()> {
        self.lock()?.set_multiline(enabled);
        self.render_host()
    }

    pub fn is_multiline(&self) -> Result<bool> {
        Ok(self.lock()?.is_multiline())
    }

    pub fn view(&self) -> Result<View> {
        Ok(self.lock()?.view())
    }

    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    fn set_component_id(&self, id: u64) -> Result<()> {
        *self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("text input component lock is poisoned"))? = Some(id);
        Ok(())
    }

    fn attach_host(&self, host: &Arc<Mutex<HostInner>>) -> Result<()> {
        *self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("text input host lock is poisoned"))? =
            Some(Arc::downgrade(host));
        Ok(())
    }

    fn render_host(&self) -> Result<()> {
        let host = self
            .host
            .lock()
            .map_err(|_| anyhow::anyhow!("text input host lock is poisoned"))?
            .clone()
            .and_then(|host| host.upgrade());
        let Some(host) = host else {
            return Ok(());
        };
        let component_id = self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("text input component lock is poisoned"))?
            .to_owned();
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        if let Some(component_id) = component_id {
            inner.running.host_invalidate_component(component_id);
        } else {
            inner.running.invalidate_frame();
        }
        inner.advance_and_render()?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, TextInput>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("text input lock is poisoned"))
    }
}

struct MountedTextInput(HostTextInput);

impl Component for MountedTextInput {
    fn view(&self) -> View {
        self.0
            .lock()
            .map(|input| input.view())
            .unwrap_or_else(|_| View::spacer(0))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_focus_changed(mounted_focus_changed);
        cx.key_commands(mounted_command_for_key, mounted_handle_command);
        cx.on_paste(mounted_paste);
        cx.on_layout_changed(mounted_layout_changed);
    }
}

fn mounted_command_for_key(
    component: &MountedTextInput,
    key: KeyStroke,
) -> Option<TextInputCommand> {
    component
        .0
        .lock()
        .ok()
        .and_then(|input| TextInput::command_for_key(&input, key))
}

fn mounted_handle_command(
    component: &mut MountedTextInput,
    command: TextInputCommand,
    cx: &mut crate::EventCx<'_>,
) -> InteractionResult {
    component
        .0
        .lock()
        .map(|mut input| TextInput::handle_command(&mut input, command, cx))
        .unwrap_or(InteractionResult::Ignored)
}

fn mounted_paste(
    component: &mut MountedTextInput,
    text: &str,
    cx: &mut crate::EventCx<'_>,
) -> InteractionResult {
    component
        .0
        .lock()
        .map(|mut input| TextInput::paste_callback(&mut input, text, cx))
        .unwrap_or(InteractionResult::Ignored)
}

fn mounted_focus_changed(component: &mut MountedTextInput, focused: bool) {
    if let Ok(mut input) = component.0.lock() {
        TextInput::focus_changed_callback(&mut input, focused);
    }
}

fn mounted_layout_changed(component: &mut MountedTextInput, size: Size) {
    if let Ok(mut input) = component.0.lock() {
        TextInput::layout_changed(&mut input, size);
    }
}

/// A handle to the History owned by a TuiHost.
#[derive(Clone)]
pub struct HostHistory {
    host: Arc<Mutex<HostInner>>,
}

impl HostHistory {
    pub fn layout(&self) -> Result<HistoryLayout> {
        let inner = self.lock()?;
        inner
            .running
            .scene_history()
            .map(History::layout)
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))
    }

    pub fn set_layout(&self, layout: HistoryLayout) -> Result<()> {
        self.lock_mut()?
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .set_layout(layout);
        Ok(())
    }

    pub fn push(&self, view: View) -> Result<HistoryUnitId> {
        let mut inner = self.lock_mut()?;
        let unit = inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .push(view)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(unit)
    }

    pub fn freeze(&self, unit: u64, view: View) -> Result<()> {
        let unit = HistoryUnitId::from_value(unit)
            .ok_or_else(|| anyhow::anyhow!("history unit id must be non-zero"))?;
        let mut inner = self.lock_mut()?;
        inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .freeze(unit, view)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(())
    }

    pub fn discard_live(&self, unit: u64) -> Result<()> {
        let unit = HistoryUnitId::from_value(unit)
            .ok_or_else(|| anyhow::anyhow!("history unit id must be non-zero"))?;
        let mut inner = self.lock_mut()?;
        inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .discard_live(unit)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(())
    }

    pub fn push_stream(&self, stream: &HostTextStream) -> Result<()> {
        let mut inner = self.lock_mut()?;
        stream.attach_host(&self.host)?;
        stream.attach(
            inner
                .running
                .scene_history_mut()
                .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?,
        )?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(())
    }

    pub fn seal_stream(&self, stream: &HostTextStream) -> Result<()> {
        let mut inner = self.lock_mut()?;
        let history = inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?;
        stream.seal_history(history)?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HostInner>> {
        self.host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))
    }

    fn lock_mut(&self) -> Result<std::sync::MutexGuard<'_, HostInner>> {
        self.lock()
    }
}

/// Native retained interaction host used by language bindings.
#[derive(Clone)]
pub struct TuiHost {
    inner: Arc<Mutex<HostInner>>,
}

// The host is the single owner of the retained native runtime. All access to
// its non-Send component registry and routing tables is serialized through
// `inner`; no component or callback is exposed to the async bridge.
unsafe impl Send for TuiHost {}
unsafe impl Sync for TuiHost {}

impl TuiHost {
    pub fn open(width: u16, height: u16, headless: bool) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!("terminal size must be positive"));
        }
        let backend = if headless {
            HostBackend::Headless(HeadlessSink {
                width,
                height,
                ..HeadlessSink::default()
            })
        } else {
            HostBackend::Real(TermwizBackend::enter()?)
        };
        let app = TuiApp::new(
            host_init as fn(&mut AppCx<'_, HostOutput>) -> Result<HostState>,
            host_update as fn(&mut HostState, HostOutput, &mut AppCx<'_, HostOutput>) -> Result<()>,
            host_view as fn(&HostState) -> View,
        )
        .with_theme(Theme::new())
        .with_history(History::new());
        let now = Instant::now();
        let mut running = app
            .start(now)
            .map_err(|error| anyhow::anyhow!("host init failed: {error:?}"))?;
        let mut backend = backend;
        let frame = prepare_frame(&mut running, &mut backend, now)?;
        let mut inner = HostInner {
            running,
            backend,
            frame,
            presentation: None,
            now,
            headless,
            closed: false,
        };
        inner.present_frame()?;
        let inner = Arc::new(Mutex::new(inner));
        Ok(Self { inner })
    }

    pub fn history(&self) -> HostHistory {
        HostHistory {
            host: Arc::clone(&self.inner),
        }
    }

    pub fn create_text_input(&self, multiline: bool) -> Result<HostTextInput> {
        let input = HostTextInput::new(multiline);
        input.attach_host(&self.inner)?;
        let mut inner = self.lock_mut()?;
        let handle = inner.running.host_register(MountedTextInput(input.clone()));
        input.set_component_id(handle.raw_id())?;
        Ok(input)
    }

    pub fn create_view_slot(&self, view: View) -> Result<HostViewSlot> {
        let slot = HostViewSlot::new(view);
        slot.attach_host(&self.inner)?;
        let mut inner = self.lock_mut()?;
        let handle = inner.running.host_register(MountedViewSlot(slot.clone()));
        slot.set_component_id(handle.raw_id())?;
        Ok(slot)
    }

    pub fn create_scroll_pane(&self, view: View) -> Result<HostScrollPane> {
        let pane = HostScrollPane::new(view);
        pane.attach_host(&self.inner)?;
        let mut inner = self.lock_mut()?;
        let handle = inner.running.host_register(MountedScrollPane(pane.clone()));
        pane.set_component_id(handle.raw_id())?;
        Ok(pane)
    }

    pub fn bind_key(&self, key: KeyStroke, route_id: impl Into<String>) -> Result<()> {
        let route_id = route_id.into();
        self.lock_mut()?.running.host_bind_key(key, move || {
            HostOutput::Routed(RoutedOutput {
                route_id: route_id.clone(),
                payload: None,
            })
        });
        Ok(())
    }

    pub fn exit(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            return Ok(());
        }

        // Complete any earlier render before preparing the final frame.
        super::run::wait_for_present_blocking(&mut inner.presentation)?;
        inner.running.host_exit();
        inner.advance_and_render()?;
        super::run::wait_for_present_blocking(&mut inner.presentation)?;
        let final_rows = {
            let width = usize::from(inner.frame.surface.width());
            if width == 0 {
                Vec::new()
            } else {
                inner
                    .frame
                    .surface
                    .cells
                    .chunks(width)
                    .map(|row| PhysicalRow::from_cells(row.to_vec()))
                    .filter(|row| !row.plain_text().is_empty())
                    .collect::<Vec<_>>()
            }
        };

        let result = match &mut inner.backend {
            HostBackend::Headless(sink) => {
                sink.history.extend(final_rows);
                Ok(())
            }
            HostBackend::Real(backend) => {
                backend.position_after_final_frame()?;
                ignore_terminal_shutdown_error(backend.restore())
            }
        };
        inner.closed = true;
        result
    }

    pub fn next_wake_ms(&self) -> u64 {
        let Ok(inner) = self.lock() else {
            return 80;
        };
        match inner.running.next_deadline() {
            Some(deadline) => deadline
                .saturating_duration_since(inner.now)
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
                .max(1),
            None => 16,
        }
    }

    pub fn route_text_input(
        &self,
        input: &HostTextInput,
        route_id: impl Into<String>,
    ) -> Result<()> {
        let output = input.submitted()?;
        let route_id = route_id.into();
        self.lock_mut()?
            .running
            .host_route(output, move |text| {
                HostOutput::Routed(RoutedOutput {
                    route_id: route_id.clone(),
                    payload: Some(text),
                })
            })
            .map_err(|_| anyhow::anyhow!("output route already exists"))?;
        Ok(())
    }

    pub fn route_text_input_output(
        &self,
        output: Output<String>,
        route_id: impl Into<String>,
    ) -> Result<()> {
        let route_id = route_id.into();
        self.lock_mut()?
            .running
            .host_route(output, move |text| {
                HostOutput::Routed(RoutedOutput {
                    route_id: route_id.clone(),
                    payload: Some(text),
                })
            })
            .map_err(|_| anyhow::anyhow!("output route already exists"))?;
        Ok(())
    }

    pub fn intercept_paste(
        &self,
        input: &HostTextInput,
        route_id: impl Into<String>,
    ) -> Result<()> {
        let id = input
            .component_id()
            .ok_or_else(|| anyhow::anyhow!("text input is not mounted"))?;
        let handle = ComponentHandle::<MountedTextInput>::from_raw_id(id);
        let route_id = route_id.into();
        self.lock_mut()?
            .running
            .host_intercept_paste(handle, move |text| {
                HostOutput::Routed(RoutedOutput {
                    route_id: route_id.clone(),
                    payload: Some(text),
                })
            });
        Ok(())
    }

    pub fn render(&self, body: View) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.running.state.body = body.clone();
        inner.running.host_set_body(body);
        inner.advance_and_render()
    }

    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.running.host_set_theme(theme);
        Ok(())
    }

    pub fn set_history(&self, history: History) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.running.host_set_history(history);
        Ok(())
    }

    pub fn dispatch_key(&self, key: KeyStroke) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner
            .running
            .dispatch_key(key)
            .map_err(|error| anyhow::anyhow!("key dispatch failed: {error:?}"))?;
        inner.advance_and_render()
    }

    pub fn dispatch_paste(&self, text: &str) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner
            .running
            .dispatch_paste(text)
            .map_err(|error| anyhow::anyhow!("paste dispatch failed: {error:?}"))?;
        inner.advance_and_render()
    }

    pub fn forward_paste(&self, text: &str) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner
            .running
            .host_forward_paste(text.to_owned())
            .map_err(|error| anyhow::anyhow!("paste forward failed: {error:?}"))?;
        inner.advance_and_render()
    }

    pub fn resize(&self, width: u16, height: u16) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!("terminal size must be positive"));
        }
        let mut inner = self.lock_mut()?;
        if let HostBackend::Headless(sink) = &mut inner.backend {
            sink.width = width;
            sink.height = height;
        }
        inner.running.invalidate_frame();
        inner.sync_real_time();
        inner.advance_and_render()
    }

    pub fn advance_time(&self, duration: Duration) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.now += duration;
        inner.advance_and_render()
    }

    pub fn next_output(&self) -> Option<RoutedOutput> {
        self.lock_mut().ok()?.running.state.outputs.pop_front()
    }

    /// Compatibility alias for bindings that used the pre-generic name.
    pub fn next_action(&self) -> Option<RoutedOutput> {
        self.next_output()
    }

    pub fn style_at(&self, row: u16, column: u16) -> Option<HostCellStyle> {
        let inner = self.lock().ok()?;
        if row >= inner.frame.surface.height() || column >= inner.frame.surface.width() {
            return None;
        }
        let style = inner.frame.surface.get(column, row).style;
        Some(HostCellStyle {
            foreground: style.foreground.map(physical_color),
            background: style.background.map(physical_color),
            bold: style.bold,
            dim: style.dim,
            italic: style.italic,
            underline: style.underline,
            reversed: style.reversed,
            strikethrough: style.strikethrough,
        })
    }

    pub fn cell_x_of_text(&self, row: u16, needle: &str) -> Option<u16> {
        let inner = self.lock().ok()?;
        if row >= inner.frame.surface.height() {
            return None;
        }
        if needle.is_empty() {
            return Some(0);
        }
        for start in 0..inner.frame.surface.width() {
            if inner.frame.surface.get(start, row).continuation {
                continue;
            }
            let mut candidate = String::new();
            for column in start..inner.frame.surface.width() {
                let cell = inner.frame.surface.get(column, row);
                if cell.continuation {
                    continue;
                }
                candidate.push_str(cell.grapheme.as_deref().unwrap_or(" "));
                if candidate == needle {
                    return Some(start);
                }
                if !needle.starts_with(&candidate) {
                    break;
                }
            }
        }
        None
    }

    pub fn exited(&self) -> bool {
        self.lock()
            .map(|inner| inner.closed || inner.running.host_exited())
            .unwrap_or(true)
    }

    pub fn poll_terminal(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        let event = match &mut inner.backend {
            HostBackend::Headless(_) => None,
            HostBackend::Real(backend) => backend.try_next_event()?,
        };
        inner.sync_real_time();
        match event {
            Some(TerminalEvent::Key(key)) => {
                inner
                    .running
                    .dispatch_key(key)
                    .map_err(|error| anyhow::anyhow!("key dispatch failed: {error:?}"))?;
                inner.advance_and_render()
            }
            Some(TerminalEvent::Paste(text)) => {
                inner
                    .running
                    .dispatch_paste(&text)
                    .map_err(|error| anyhow::anyhow!("paste dispatch failed: {error:?}"))?;
                inner.advance_and_render()
            }
            Some(TerminalEvent::Resize) => {
                inner.running.invalidate_frame();
                inner.advance_and_render()
            }
            None => inner.advance_and_render(),
        }
    }

    /// Run the native interaction driver until a caller-defined routed output
    /// or exit is available. Terminal input, component ticks, stream wakeups,
    /// and rendering stay on the Rust side of the boundary.
    pub async fn wait_for_output(&self) -> Result<Option<RoutedOutput>> {
        loop {
            if self.exited() {
                return Ok(None);
            }

            // A headless host is deterministic for explicit `advance_time`, but
            // an asynchronous event wait is a real-time driver just like the
            // terminal backend. Refresh its clock before polling timers.
            if let Ok(mut inner) = self.lock_mut()
                && inner.headless
            {
                inner.now = Instant::now();
            }
            self.poll_terminal()?;
            if let Some(output) = self.next_output() {
                return Ok(Some(output));
            }

            let wait_ms = self.next_wake_ms().min(16).max(1);
            super::run::wait_for_deadline(Some(Instant::now() + Duration::from_millis(wait_ms)))
                .await;
        }
    }

    /// Compatibility alias for bindings that used the pre-generic name.
    pub async fn wait_for_action(&self) -> Result<Option<RoutedOutput>> {
        self.wait_for_output().await
    }

    pub fn screen_rows(&self) -> Vec<String> {
        self.lock()
            .map(|inner| inner.frame.screen_lines())
            .unwrap_or_default()
    }

    pub fn native_history_rows(&self) -> Vec<String> {
        self.lock()
            .ok()
            .and_then(|inner| match &inner.backend {
                HostBackend::Headless(sink) => {
                    Some(sink.history.iter().map(PhysicalRow::plain_text).collect())
                }
                HostBackend::Real(_) => Some(Vec::new()),
            })
            .unwrap_or_default()
    }

    pub fn close(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            return Ok(());
        }
        super::run::wait_for_present_blocking(&mut inner.presentation)?;
        // Closing a host is also the ownership boundary for its retained
        // semantic root. Drop both the host state's body and the scene root so
        // environment-scoped weak caches can observe expiry after disposal.
        inner.running.state.body = View::spacer(0);
        inner.running.host_set_body(View::spacer(0));
        inner.running.host_clear_retained_views();
        inner.closed = true;
        if let HostBackend::Real(backend) = &mut inner.backend {
            ignore_terminal_shutdown_error(backend.restore())?;
        }
        Ok(())
    }

    pub fn is_headless(&self) -> bool {
        self.lock().map(|inner| inner.headless).unwrap_or(true)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HostInner>> {
        self.inner
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))
    }

    fn lock_mut(&self) -> Result<std::sync::MutexGuard<'_, HostInner>> {
        self.lock()
    }
}

fn physical_color(color: crate::physical::PhysicalColor) -> String {
    match color {
        crate::physical::PhysicalColor::Default => "default".to_owned(),
        crate::physical::PhysicalColor::Named(color) => format!("{color:?}"),
        crate::physical::PhysicalColor::Indexed(value) => format!("ansi:{value}"),
        crate::physical::PhysicalColor::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

fn ignore_terminal_shutdown_error(result: Result<()>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().contains("terminal worker stopped") => Ok(()),
        Err(error) => Err(error),
    }
}

impl Drop for TuiHost {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            let _ = self.close();
        }
    }
}

impl HostInner {
    fn render(&mut self) -> Result<()> {
        self.frame = prepare_frame(&mut self.running, &mut self.backend, self.now)?;
        self.present_frame()
    }

    fn present_frame(&mut self) -> Result<()> {
        if let Some(mut receipt) = self.presentation.take() {
            match receipt.try_recv() {
                Ok(result) => result
                    .map_err(|error| anyhow::anyhow!("terminal presentation failed: {error}"))?,
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    self.presentation = Some(receipt);
                    return Ok(());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    return Err(anyhow::anyhow!("terminal presentation reply lost"));
                }
            }
        }
        if let HostBackend::Real(backend) = &mut self.backend {
            match backend.begin_frame(&self.frame) {
                Ok(receipt) => self.presentation = Some(receipt),
                Err(error) if error.to_string().contains("terminal worker stopped") => {
                    self.closed = true;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn advance_and_render(&mut self) -> Result<()> {
        let status = self
            .running
            .advance_ready(self.now)
            .map_err(|error| anyhow::anyhow!("host update failed: {error:?}"))?;
        if status.dirty {
            self.render()?;
        }
        Ok(())
    }

    fn sync_real_time(&mut self) {
        if matches!(self.backend, HostBackend::Real(_)) {
            self.now = Instant::now();
        }
    }
}

fn prepare_frame(
    running: &mut HostRunning,
    backend: &mut HostBackend,
    now: Instant,
) -> Result<PreparedSceneFrame> {
    match backend {
        HostBackend::Headless(sink) => running
            .prepare_frame(now, sink, |sink| Ok(Size::new(sink.width, sink.height)))
            .map_err(|error| anyhow::anyhow!("headless render failed: {error:?}")),
        HostBackend::Real(backend) => {
            let frame = running
                .prepare_frame(now, backend, |backend| backend.viewport())
                .map_err(|error| anyhow::anyhow!("terminal render failed: {error:?}"))?;
            Ok(frame)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostTextStream, TuiHost};

    #[test]
    fn host_stream_keeps_append_chunks_as_source_spans() {
        let stream = HostTextStream::with_markdown();
        stream.append("hello", &[]).unwrap();
        stream.append(" world\n", &[]).unwrap();

        let state = stream.state.lock().unwrap();
        assert_eq!(state.source_chunks.len(), 2);
        assert_eq!(state.pacing_input.spans().len(), 12);
        assert_eq!(state.source_text(), "hello world\n");
    }

    #[test]
    fn host_runtime_character_markdown_compaction_preserves_resident_coordinates() {
        let host = TuiHost::open(80, 24, true).unwrap();
        let stream = HostTextStream::with_markdown();
        host.history().push_stream(&stream).unwrap();
        let document = "# heading\n\nThis is **markdown**.\n\n- first\n- second\n\n```rust\nfn main() {}\n```\n";
        for character in document.chars() {
            stream
                .append(character.to_string(), &[])
                .unwrap_or_else(|error| panic!("append {character:?} failed: {error}"));
        }
        host.close().unwrap();
    }
}
