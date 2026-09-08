//! A language-binding host for the retained native application runtime.
//!
//! `TuiHost` deliberately exposes caller-defined outputs and native snapshots,
//! not terminal events. Components remain mounted in the native `SceneHost`.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use anyhow::Result;

use super::content::{ContentFamily, ContentHostRegistry, HostContentPort, PreparedContentCommit};
use super::environment::{
    HostDrainReport, HostEpochs, HostFlushOutcome, TuiEnvironment, WakeDisposition,
    host_attempt_error,
};
use super::view_state::HostViewState;
use crate::controls::text_input::command::TextInputCommand;
use crate::presentation::factory as vf;
use crate::{
    BorderSpec, Component, ComponentCx, ComponentHandle, History, HistoryLayout, HistoryUnitId,
    InteractionResult, KeyStroke, Output, ScrollPane, TextInput, Theme, View,
    backend::NativeHistorySink,
    geometry::Size,
    physical::PhysicalRow,
    presentation::{ContentProvider, EmptyContentProvider},
    retained_state::{
        StateCandidateOverlay, StateFrameView, StateNodeKind, ViewStateLifecycle, ViewStateRecord,
        ViewStateRegistry,
    },
    scene::{PreparedSceneFrame, SceneHostError},
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

type HostRunning = crate::application::kernel::NativeRuntime;

const INPUT_PUMP_BUDGET: usize = 32;

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

pub(super) struct HostInner {
    running: HostRunning,
    backend: HostBackend,
    /// The last complete logical frame. Readback and visible-state queries
    /// always use this value; a candidate is kept separately until its
    /// backend receipt succeeds.
    frame: PreparedSceneFrame,
    candidate_frame: Option<PreparedSceneFrame>,
    presentation: Option<PresentReceipt>,
    /// True while a frame still needs to be handed to the terminal worker.
    /// The initial bootstrap frame uses this flag without a candidate frame.
    frame_pending: bool,
    /// Epoch/revision captured when `candidate_frame` was prepared. New work
    /// accepted while its presentation is in flight must remain pending after
    /// this exact frame commits.
    candidate_epoch: Option<u64>,
    candidate_structural_revision: Option<u64>,
    /// Candidate content bindings are captured with the candidate frame so a
    /// control mutation accepted while presentation is in flight cannot be
    /// promoted into that older frame by accident.
    candidate_content_commit: Option<PreparedContentCommit>,
    candidate_state_commit: Option<crate::retained_state::PreparedStateCommit>,
    candidate_content_dirty_epoch: Option<u64>,
    /// Attempt metadata retained long enough for the environment to report a
    /// failed in-flight candidate rather than a newer pending epoch.
    failed_attempt: Option<(u64, u64)>,
    now: Instant,
    headless: bool,
    closed: bool,
    environment: TuiEnvironment,
    host_id: u64,
    desired_structural_revision: u64,
    visible_structural_revision: u64,
    visible_frame_revision: u64,
    pending_epoch: u64,
    committed_epoch: u64,
    /// A Source/control mutation requires content-derived layout/paint cache
    /// invalidation. It remains set until the corresponding frame commits.
    content_dirty: bool,
    /// Physical terminal/History state may be unknown after a sink or
    /// presentation failure. The next successful candidate is a recovery
    /// frame; this marker is never cleared by logical candidate rollback.
    physical_sync_unknown: bool,
    /// Reused per-host affected content worklist used to coalesce one Source
    /// wake group into one pending epoch without allocating a temporary fanout
    /// vector for every mutation.
    pub(super) content_dirty_scratch: Vec<crate::presentation::ContentDirty>,
    #[cfg(test)]
    fail_next_frame: Option<String>,
    view_states: ViewStateRegistry,
    pub(super) content: ContentHostRegistry,
}

impl Drop for HostInner {
    fn drop(&mut self) {
        // Host-bound handles such as History can keep the inner Arc alive
        // after the public TuiHost wrapper is dropped. Release content
        // memberships before unregistering the host so environment-owned
        // Sources cannot retain stale Connector leases.
        self.content.dispose_all();
        // Always unregister at the final owner boundary so weak environment
        // entries cannot leave a stale pending host or latched wake behind.
        self.environment.unregister_host(self.host_id);
    }
}

/// A shared native `TextInput` value that can be mounted into one `TuiHost`.
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
    #[must_use]
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

    #[must_use]
    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    /// PERF-12 T13.1 R8: request deferred retirement of this slot's registry
    /// entry. Idempotent; a never-host-mounted slot (no component id) is a
    /// no-op. Physical reclamation happens in
    /// `NativeRuntime::reap_retired_components` after reconciliation proves the
    /// component unmounted.
    pub fn retire(&self) {
        let Some(raw_id) = self.component_id() else {
            return;
        };
        if let Ok(guard) = self.host.lock()
            && let Some(weak) = guard.as_ref()
            && let Some(inner) = weak.upgrade()
            && let Ok(mut inner) = inner.lock()
        {
            inner.running.host_retire_component(raw_id);
        }
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.lock().map_or(0, |state| state.revision)
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
        if state.frame_index == 0
            && let Some(frames) = state.pending_frames.take()
        {
            state.frames = frames;
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
    #[must_use]
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

    #[must_use]
    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    /// PERF-12 T13.1 R8: see `HostViewSlot::retire`.
    pub fn retire(&self) {
        let Some(raw_id) = self.component_id() else {
            return;
        };
        if let Ok(guard) = self.host.lock()
            && let Some(weak) = guard.as_ref()
            && let Some(inner) = weak.upgrade()
            && let Ok(mut inner) = inner.lock()
        {
            inner.running.host_retire_component(raw_id);
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
            .map_or_else(|_| vf::spacer(0), |pane| Component::view(&*pane))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_layout_changed(Self::on_layout_changed);
        cx.on_content_extent_changed(Self::on_content_extent_changed);
        cx.key_commands(Self::map_command, Self::handle_command);
    }
}

impl MountedScrollPane {
    fn on_layout_changed(component: &mut Self, size: Size) {
        if let Ok(mut pane) = component.0.state.lock() {
            pane.on_layout_changed(size);
        }
    }

    fn on_content_extent_changed(component: &mut Self, extent: Size) {
        if let Ok(mut pane) = component.0.state.lock() {
            pane.on_content_extent_changed(extent);
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
            .map_or(InteractionResult::Ignored, |mut pane| {
                pane.handle_command(command, cx)
            })
    }
}

struct MountedViewSlot(HostViewSlot);

impl Component for MountedViewSlot {
    fn view(&self) -> View {
        self.0
            .state
            .lock()
            .map_or_else(|_| vf::spacer(0), |state| state.view.clone())
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

impl HostTextInput {
    #[must_use]
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

    #[must_use]
    pub fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    /// Requests deferred retirement of the host-registered input component.
    /// The registry keeps it alive until a successful scene reconciliation
    /// proves that the component is no longer mounted.
    pub fn retire(&self) {
        let Some(raw_id) = self.component_id() else {
            return;
        };
        if let Ok(guard) = self.host.lock()
            && let Some(weak) = guard.as_ref()
            && let Some(inner) = weak.upgrade()
            && let Ok(mut inner) = inner.lock()
        {
            inner.running.host_retire_component(raw_id);
        }
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
            .map_or_else(|_| vf::spacer(0), |input| input.view())
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
        .map_or(InteractionResult::Ignored, |mut input| {
            TextInput::handle_command(&mut input, command, cx)
        })
}

fn mounted_paste(
    component: &mut MountedTextInput,
    text: &str,
    cx: &mut crate::EventCx<'_>,
) -> InteractionResult {
    component
        .0
        .lock()
        .map_or(InteractionResult::Ignored, |mut input| {
            TextInput::paste_callback(&mut input, text, cx)
        })
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

/// A handle to the History owned by a `TuiHost`.
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
        let mut inner = self.lock_mut()?;
        let history = inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?;
        if history.layout() == layout {
            return Ok(());
        }
        history.set_layout(layout);
        inner.running.invalidate_frame();
        inner.advance_and_render()
    }

    pub fn push(&self, view: View) -> Result<HistoryUnitId> {
        let mut inner = self.lock_mut()?;
        let body = inner.running.scene_body().clone();
        let state_targets = inner
            .running
            .host_state_attachment_targets_with_history_view(&body, &view)?;
        inner.validate_state_targets(&state_targets)?;
        let content_targets = inner
            .running
            .host_content_attachment_targets_with_history_view(&body, &view)?;
        inner.content.validate_targets(&content_targets)?;
        let unit = inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .push(view.clone())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        if let Some(port_id) = view.content_attachment_id() {
            inner
                .content
                .set_history_unit(port_id, unit.value(), view.decoration().padding)?;
        }
        inner.set_desired_state_bindings(&state_targets)?;
        inner.content.set_desired(&content_targets)?;
        inner.running.invalidate_frame();
        inner.advance_and_render()?;
        Ok(unit)
    }

    pub fn freeze(&self, unit: u64, view: View) -> Result<()> {
        let unit = HistoryUnitId::from_value(unit)
            .ok_or_else(|| anyhow::anyhow!("history unit id must be non-zero"))?;
        let mut inner = self.lock_mut()?;
        let body = inner.running.scene_body().clone();
        let history_views = inner
            .running
            .scene_history()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .state_views_with_replacement(unit, &view)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let state_targets = inner
            .running
            .host_state_attachment_targets_for_history_views(&body, history_views)?;
        inner.validate_state_targets(&state_targets)?;
        let content_views = inner
            .running
            .scene_history()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .content_views_with_replacement(unit, &view)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let content_targets = inner
            .running
            .host_content_attachment_targets_for_history_views(&body, content_views)?;
        inner.content.validate_targets(&content_targets)?;
        inner
            .running
            .scene_history_mut()
            .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
            .freeze(unit, view.clone())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        inner.content.clear_history_unit(unit.value());
        if let Some(port_id) = view.content_attachment_id() {
            inner
                .content
                .set_history_unit(port_id, unit.value(), view.decoration().padding)?;
        }
        inner.set_desired_state_bindings(&state_targets)?;
        inner.content.set_desired(&content_targets)?;
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
        inner.content.clear_history_unit(unit.value());
        inner.refresh_desired_state_bindings()?;
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
    pub(crate) inner: Arc<Mutex<HostInner>>,
}

// The host is the single owner of the retained native runtime. All access to
// its non-Send component registry and routing tables is serialized through
// `inner`; no component or callback is exposed to the async boundary.
unsafe impl Send for TuiHost {}
unsafe impl Sync for TuiHost {}

impl TuiHost {
    pub fn open(width: u16, height: u16, headless: bool) -> Result<Self> {
        Self::open_in_environment(width, height, headless, TuiEnvironment::new())
    }

    /// Opens a host in an existing native environment. Hosts sharing this
    /// environment share one pending-host queue and wake latch.
    pub fn open_in_environment(
        width: u16,
        height: u16,
        headless: bool,
        environment: TuiEnvironment,
    ) -> Result<Self> {
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
        let now = Instant::now();
        let mut running = HostRunning::new();
        let mut backend = backend;
        let frame = prepare_frame(&mut running, &mut backend, now, &StateFrameView::empty())?;
        let inner =
            Arc::new(Mutex::new(HostInner {
                running,
                backend,
                frame,
                candidate_frame: None,
                presentation: None,
                frame_pending: true,
                candidate_epoch: None,
                candidate_structural_revision: None,
                candidate_content_commit: None,
                candidate_state_commit: None,
                candidate_content_dirty_epoch: None,
                failed_attempt: None,
                now,
                headless,
                closed: false,
                environment: environment.clone(),
                host_id: 0,
                desired_structural_revision: 0,
                visible_structural_revision: 0,
                visible_frame_revision: 0,
                pending_epoch: 0,
                committed_epoch: 0,
                content_dirty: false,
                physical_sync_unknown: false,
                content_dirty_scratch: Vec::new(),
                #[cfg(test)]
                fail_next_frame: None,
                view_states: ViewStateRegistry::new(),
                content: ContentHostRegistry::new(environment.content_source_registry().map_err(
                    |error| anyhow::anyhow!("content environment setup failed: {error}"),
                )?),
            }));
        let host_id = environment.register_host(&inner)?;
        let mut host = inner
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        host.host_id = host_id;
        if let Err(error) = host.present_frame() {
            drop(host);
            environment.unregister_host(host_id);
            return Err(error);
        }
        drop(host);
        Ok(Self { inner })
    }

    #[must_use]
    pub fn history(&self) -> HostHistory {
        HostHistory {
            host: Arc::clone(&self.inner),
        }
    }

    pub fn create_view_state(&self) -> Result<HostViewState> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            return Err(anyhow::anyhow!("host is closed"));
        }
        let host_id = inner.host_id;
        let id = inner.view_states.create(host_id)?;
        Ok(HostViewState::new(id, &self.inner))
    }

    /// Creates a host-owned `ContentPort`. Source/Funnel identity remains
    /// separate from the structural attachment; plain content projection is
    /// prepared only when the port is mounted and selected.
    pub fn create_content_port(&self, family: ContentFamily) -> Result<HostContentPort> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            return Err(anyhow::anyhow!("HOST_DISPOSED: host is closed"));
        }
        inner
            .content
            .create_port(Arc::downgrade(&self.inner), family)
    }

    /// Invalidates all host-owned content identities during owner teardown.
    /// This is the explicit owner-death cascade; individual dispose methods
    /// remain strict while the host is live.
    pub fn dispose_content_resources(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.content.dispose_all();
        Ok(())
    }

    /// Returns the authoritative desired/visible revisions and host epochs.
    pub fn epochs(&self) -> Result<HostEpochs> {
        Ok(self.lock()?.epochs())
    }

    /// Accepts a desired structural root without preparing or presenting a
    /// frame. The returned wake disposition is an edge-trigger hint only; the
    /// environment queue and host epochs remain authoritative.
    pub fn set_desired_view(&self, body: View) -> Result<WakeDisposition> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            return Err(anyhow::anyhow!("host is closed"));
        }
        let state_targets = inner.running.host_state_attachment_targets(&body)?;
        let state_ids = state_targets.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let mut unique_state_ids = HashSet::with_capacity(state_ids.len());
        if state_ids.iter().any(|id| !unique_state_ids.insert(*id)) {
            return Err(anyhow::anyhow!(
                "DUPLICATE_VIEW_STATE_ATTACHMENT: duplicate state attachment"
            ));
        }
        inner.validate_state_targets(&state_targets)?;
        let content_targets = inner.running.host_content_attachment_targets(&body)?;
        inner.content.validate_targets(&content_targets)?;
        let next_revision = inner
            .desired_structural_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("desired structural revision exhausted"))?;
        inner.set_desired_state_bindings(&state_targets)?;
        inner.content.set_desired(&content_targets)?;
        inner.running.host_set_body(body);
        inner.desired_structural_revision = next_revision;
        inner.mark_pending()
    }

    /// Synchronously attempts the current host's pending frame. This is the
    /// explicit host visibility barrier.
    pub fn flush_pending(&self) -> Result<()> {
        self.lock_mut()?.flush_for_environment(true).map(|_| ())
    }

    /// Clears desired/visible retained-state binding flags before wrapper
    /// disposal during Tui owner teardown.
    pub fn clear_view_state_bindings(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.clear_state_bindings();
        Ok(())
    }

    /// Fairly drains all pending hosts in this native environment. Automatic
    /// callers use `force_retry = false`; explicit barriers force one retry of
    /// retry-blocked hosts.
    pub fn flush_pending_hosts(
        &self,
        budget: usize,
        force_retry: bool,
    ) -> anyhow::Result<HostDrainReport> {
        let (environment, host_id) = {
            let inner = self.lock()?;
            (inner.environment.clone(), inner.host_id)
        };
        environment.drain_pending_for(budget, force_retry, Some(host_id))
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
        self.lock_mut()?
            .running
            .host_bind_key(key, move || RoutedOutput {
                route_id: route_id.clone(),
                payload: None,
            });
        Ok(())
    }

    pub fn exit(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            let host_id = inner.host_id;
            let environment = inner.environment.clone();
            drop(inner);
            environment.unregister_host(host_id);
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
            HostBackend::Real(backend) => match backend.position_after_final_frame() {
                Ok(()) => ignore_terminal_shutdown_error(backend.restore()),
                Err(error) => Err(error),
            },
        };
        if result.is_ok() {
            let host_id = inner.host_id;
            let environment = inner.environment.clone();
            inner.dispose_view_states();
            inner.content.dispose_all();
            inner.closed = true;
            drop(inner);
            environment.unregister_host(host_id);
        }
        result
    }

    #[must_use]
    pub fn next_wake_ms(&self) -> u64 {
        let Ok(inner) = self.lock() else {
            return 80;
        };
        let deadline = [inner.running.next_deadline(), inner.content.next_wakeup()]
            .into_iter()
            .flatten()
            .min();
        match deadline {
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
            .host_route(output, move |text| RoutedOutput {
                route_id: route_id.clone(),
                payload: Some(text),
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
            .host_route(output, move |text| RoutedOutput {
                route_id: route_id.clone(),
                payload: Some(text),
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
            .host_intercept_paste(handle, move |text| RoutedOutput {
                route_id: route_id.clone(),
                payload: Some(text),
            });
        Ok(())
    }

    pub fn render(&self, body: View) -> Result<()> {
        self.set_desired_view(body)?;
        self.flush_pending()
    }

    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.running.host_set_theme(theme);
        inner.advance_and_render()
    }

    pub fn validate_history(&self, history: &History) -> Result<()> {
        let inner = self.lock()?;
        if inner.closed {
            return Err(anyhow::anyhow!("host is closed"));
        }
        let body = inner.running.scene_body().clone();
        let state_targets = inner
            .running
            .host_state_attachment_targets_for_history(&body, history)?;
        inner.validate_state_targets(&state_targets)?;
        let content_targets = inner
            .running
            .host_content_attachment_targets_for_history(&body, history)?;
        inner.content.validate_targets(&content_targets)
    }

    pub fn set_history(&self, history: History) -> Result<()> {
        let mut inner = self.lock_mut()?;
        let body = inner.running.scene_body().clone();
        let state_targets = inner
            .running
            .host_state_attachment_targets_for_history(&body, &history)?;
        inner.validate_state_targets(&state_targets)?;
        let content_targets = inner
            .running
            .host_content_attachment_targets_for_history(&body, &history)?;
        inner.content.validate_targets(&content_targets)?;
        inner.running.host_set_history(history);
        inner.set_desired_state_bindings(&state_targets)?;
        inner.content.set_desired(&content_targets)?;
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

    #[must_use]
    pub fn next_output(&self) -> Option<RoutedOutput> {
        self.lock_mut().ok()?.running.next_output()
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

    #[must_use]
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

    #[must_use]
    pub fn exited(&self) -> bool {
        self.lock()
            .map_or(true, |inner| inner.closed || inner.running.host_exited())
    }

    pub fn poll_terminal(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.sync_real_time();
        for _ in 0..INPUT_PUMP_BUDGET {
            if inner.running.has_pending_outputs() {
                break;
            }
            let event = match &mut inner.backend {
                HostBackend::Headless(_) => None,
                HostBackend::Real(backend) => backend.try_next_event()?,
            };
            let Some(event) = event else {
                break;
            };
            match event {
                TerminalEvent::Key(key) => {
                    inner
                        .running
                        .dispatch_key(key)
                        .map_err(|error| anyhow::anyhow!("key dispatch failed: {error:?}"))?;
                }
                TerminalEvent::Paste(text) => {
                    inner
                        .running
                        .dispatch_paste(&text)
                        .map_err(|error| anyhow::anyhow!("paste dispatch failed: {error:?}"))?;
                }
                TerminalEvent::Resize => inner.running.invalidate_frame(),
            }
            // Do not consume input after a routed output. The caller must
            // reduce that output before later keystrokes can change focus or
            // clear the composer.
            if inner.running.has_pending_outputs() {
                break;
            }
        }
        inner.advance_and_render()
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

    #[must_use]
    pub fn screen_rows(&self) -> Vec<String> {
        self.lock()
            .map(|inner| inner.frame.screen_lines())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn native_history_rows(&self) -> Vec<String> {
        self.lock()
            .ok()
            .map(|inner| match &inner.backend {
                HostBackend::Headless(sink) => {
                    sink.history.iter().map(PhysicalRow::plain_text).collect()
                }
                HostBackend::Real(_) => Vec::new(),
            })
            .unwrap_or_default()
    }

    pub fn close(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if inner.closed {
            let host_id = inner.host_id;
            let environment = inner.environment.clone();
            drop(inner);
            environment.unregister_host(host_id);
            return Ok(());
        }
        if let Err(error) = super::run::wait_for_present_blocking(&mut inner.presentation) {
            // A lost presentation reply means this host can no longer make
            // progress. Retire it from the environment before returning so a
            // dropped host cannot leave a stale pending ID/latch behind.
            let host_id = inner.host_id;
            let environment = inner.environment.clone();
            inner.dispose_view_states();
            inner.content.dispose_all();
            inner.closed = true;
            let restore = if let HostBackend::Real(backend) = &mut inner.backend {
                ignore_terminal_shutdown_error(backend.restore())
            } else {
                Ok(())
            };
            drop(inner);
            environment.unregister_host(host_id);
            return match restore {
                Ok(()) => Err(error),
                Err(restore_error) => Err(anyhow::anyhow!(
                    "terminal presentation reply lost: {error}; host restore failed: {restore_error}"
                )),
            };
        }
        // Closing a host is also the ownership boundary for its retained
        // semantic root. Replace the scene root so environment-scoped weak
        // caches can observe expiry after disposal.
        inner.running.host_set_body(vf::spacer(0));
        inner.running.host_clear_retained_views();
        inner.dispose_view_states();
        inner.content.dispose_all();
        inner.closed = true;
        let result = if let HostBackend::Real(backend) = &mut inner.backend {
            ignore_terminal_shutdown_error(backend.restore())
        } else {
            Ok(())
        };
        let host_id = inner.host_id;
        let environment = inner.environment.clone();
        drop(inner);
        environment.unregister_host(host_id);
        result
    }

    #[cfg(test)]
    pub fn fail_next_frame_for_test(&self, diagnostic: impl Into<String>) -> Result<()> {
        self.lock_mut()
            .map(|mut inner| inner.fail_next_frame = Some(diagnostic.into()))
    }

    /// Poisons this host's owner lock for cross-crate failure-injection tests.
    /// The hook is available only to the in-tree test-util feature and never
    /// crosses the normal native or TypeScript host surface.
    #[cfg(any(test, feature = "test-util"))]
    pub fn poison_lock_for_test(&self) {
        let inner = Arc::clone(&self.inner);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = inner.lock().expect("host must be healthy before poisoning");
            panic!("intentional host lock poisoning for test");
        }));
    }

    #[must_use]
    pub fn is_headless(&self) -> bool {
        self.lock().map_or(true, |inner| inner.headless)
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
        Err(error) if crate::terminal::is_terminal_worker_stopped(&error) => Ok(()),
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
    pub(super) fn flush_for_environment(
        &mut self,
        wait_for_presentation: bool,
    ) -> Result<(HostFlushOutcome, u64, u64)> {
        let mut outcome = self.flush_pending_frame()?;
        if wait_for_presentation && outcome.waiting_for_presentation {
            outcome = self.finish_presentation_blocking()?;
        }
        Ok((outcome, self.pending_epoch, self.committed_epoch))
    }

    pub(super) fn environment_pending_epoch(&mut self) -> Result<u64> {
        if self.pending_epoch == self.committed_epoch
            && (self.running.is_dirty() || self.content.has_pending_source_cleanup())
        {
            self.ensure_pending()?;
        }
        Ok(self.pending_epoch)
    }

    pub(super) fn environment_error_epochs(&self) -> (u64, u64, u64) {
        let (attempted_epoch, desired_revision) = self
            .failed_attempt
            .unwrap_or((self.pending_epoch, self.desired_structural_revision));
        (attempted_epoch, desired_revision, self.pending_epoch)
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed
    }

    /// Captures the frame candidate overlay. Only demanded attachments
    /// (desired ∪ visible ∪ in-flight) contribute versions; unrelated
    /// unmounted records are never visited or cloned.
    fn capture_state_candidate(&mut self) -> StateCandidateOverlay {
        self.view_states.capture_candidate()
    }

    pub(super) fn mutate_view_state<F>(
        &mut self,
        id: u64,
        mutation: F,
    ) -> Result<crate::retained_state::StateEffects>
    where
        F: FnOnce(&mut ViewStateRecord) -> Result<crate::retained_state::StateEffects>,
    {
        self.view_states.mutate_record(id, mutation)
    }

    pub(super) fn validate_view_state_kind(&self, id: u64, kind: StateNodeKind) -> Result<()> {
        let Some(record) = self.view_states.record(id) else {
            return Err(anyhow::anyhow!("STATE_DISPOSED: ViewState is disposed"));
        };
        if record.lifecycle == ViewStateLifecycle::Disposed {
            return Err(anyhow::anyhow!("STATE_DISPOSED: ViewState is disposed"));
        }
        crate::retained_state::validate_geometry_for_kind(kind, &record.geometry)
    }

    fn validate_state_targets(&self, targets: &[(u64, StateNodeKind)]) -> Result<()> {
        self.view_states.validate_targets(targets)
    }

    fn set_desired_state_bindings(&mut self, targets: &[(u64, StateNodeKind)]) -> Result<()> {
        self.view_states.set_desired(targets)
    }

    fn refresh_desired_state_bindings(&mut self) -> Result<()> {
        let targets = self.running.host_current_state_attachment_targets()?;
        self.set_desired_state_bindings(&targets)?;
        let content_targets = self.running.host_current_content_attachment_targets()?;
        self.content.set_desired(&content_targets)
    }

    fn candidate_content_commit(&mut self) -> Result<PreparedContentCommit> {
        // H3 already validated the complete attachment list before desired
        // acceptance. The content commit plan needs only the changed-record
        // set populated by that acceptance and the current candidate measure;
        // unchanged visible bindings are not copied into another table.
        self.content.prepare_content_commit()
    }

    /// Prepares the visible/in-flight state tables before backend submission.
    /// The returned candidate owns every allocation needed by receipt-time
    /// state promotion.
    fn candidate_state_commit(
        &mut self,
        targets: &[(u64, StateNodeKind)],
    ) -> Result<crate::retained_state::PreparedStateCommit> {
        self.view_states.prepare_candidate(targets)
    }

    fn clear_in_flight_state_bindings(&mut self) {
        let candidate_present = self.candidate_frame.is_some();
        let candidate_shape_matches = candidate_present == self.candidate_epoch.is_some()
            && candidate_present == self.candidate_structural_revision.is_some()
            && candidate_present == self.candidate_content_dirty_epoch.is_some()
            && candidate_present == self.candidate_content_commit.is_some()
            && candidate_present == self.candidate_state_commit.is_some();
        if !candidate_shape_matches {
            panic!("candidate frame and commit metadata must be present together");
        }
        // The initial real-terminal bootstrap receipt has no candidate frame
        // or state plan. Its failed receipt still reaches this cleanup path,
        // but there is no in-flight state to clear.
        if !candidate_present {
            return;
        }
        let prepared = self
            .candidate_state_commit
            .as_ref()
            .expect("candidate state commit must exist for a candidate frame");
        self.view_states
            .clear_in_flight_prepared(&prepared.in_flight_ids);
    }

    pub(super) fn invalidate_state(
        &mut self,
        id: u64,
        effects: crate::retained_state::StateEffects,
    ) -> Result<WakeDisposition> {
        if !self.view_states.is_bound(id)? {
            return Ok(WakeDisposition::default());
        }
        self.running.host_invalidate_state(id, effects);
        self.mark_pending()
    }

    fn clear_state_bindings(&mut self) {
        self.view_states.clear_bindings();
    }

    pub(super) fn dispose_view_state(&mut self, id: u64) -> Result<()> {
        // Unknown identities stay a no-op so repeated disposal is idempotent.
        if self.view_states.record(id).is_none() {
            return Ok(());
        }
        // The host namespace rides in the high bits of every state identity,
        // so a record from another host cannot alias this host's slot.
        if id >> 32 != self.host_id {
            return Err(anyhow::anyhow!("ViewState belongs to a different host"));
        }
        self.view_states.dispose(id)
    }

    fn dispose_view_states(&mut self) {
        self.view_states.dispose_all();
    }

    fn epochs(&self) -> HostEpochs {
        HostEpochs {
            host_id: self.host_id,
            desired_structural_revision: self.desired_structural_revision,
            visible_structural_revision: self.visible_structural_revision,
            visible_frame_revision: self.visible_frame_revision,
            pending_epoch: self.pending_epoch,
            committed_epoch: self.committed_epoch,
        }
    }

    pub(super) fn mark_content_pending(
        &mut self,
        dirty: crate::presentation::ContentDirty,
    ) -> anyhow::Result<WakeDisposition> {
        self.mark_content_pending_batch(std::slice::from_ref(&dirty))
    }

    pub(super) fn mark_content_pending_batch(
        &mut self,
        dirty: &[crate::presentation::ContentDirty],
    ) -> anyhow::Result<WakeDisposition> {
        for item in dirty {
            self.running.host_invalidate_content(*item);
        }
        self.content_dirty = true;
        self.mark_pending()
    }

    pub(super) fn mark_pending(&mut self) -> anyhow::Result<WakeDisposition> {
        self.pending_epoch = self
            .pending_epoch
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("host pending epoch exhausted"))?;
        self.environment.mark_host_pending(self.host_id)
    }

    pub(super) fn environment_wake_epoch(&self) -> u64 {
        self.environment.wake_epoch()
    }

    fn ensure_pending(&mut self) -> anyhow::Result<()> {
        if self.pending_epoch == self.committed_epoch {
            let _ = self.mark_pending()?;
        } else {
            let _ = self.environment.mark_host_pending(self.host_id)?;
        }
        Ok(())
    }

    fn render(&mut self) -> Result<HostFlushOutcome> {
        if self.presentation.is_some() {
            return Ok(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            });
        }
        let target_epoch = self.pending_epoch;
        let target_structural_revision = self.desired_structural_revision;
        self.content.begin_projection_candidate();
        // One candidate overlay over the committed version table replaces the
        // old whole-registry snapshot. Failed preparation keeps the committed
        // versions untouched: the overlay owns its `Arc` pins, and the scene
        // candidate is discarded without merging anything back.
        let overlay = self.capture_state_candidate();
        let states = StateFrameView::new(self.view_states.committed_table(), &overlay);
        let candidate = match prepare_frame_with_content(
            &mut self.running,
            &mut self.backend,
            self.now,
            &states,
            &mut self.content,
        ) {
            Ok(candidate) => candidate,
            Err(error) => {
                // SceneHost may have staged derived layout/surface state before
                // a late preparation error. Keep the HostInner frame as the
                // sole visible authority and rebuild the candidate on retry.
                self.note_physical_sync_failure(&error);
                self.failed_attempt = Some((target_epoch, target_structural_revision));
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                return Err(error);
            }
        };
        let state_commit = match self.candidate_state_commit(&candidate.state_bindings) {
            Ok(commit) => commit,
            Err(error) => {
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                self.running.host_abort_content_candidate();
                return Err(error);
            }
        };
        let content_commit = match self.candidate_content_commit() {
            Ok(commit) => commit,
            Err(error) => {
                self.view_states
                    .clear_in_flight_prepared(&state_commit.in_flight_ids);
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                self.running.host_abort_content_candidate();
                return Err(error);
            }
        };
        let previous_pending = self.frame_pending;
        debug_assert!(self.candidate_frame.is_none());
        self.candidate_frame = Some(candidate);
        self.candidate_epoch = Some(target_epoch);
        self.candidate_structural_revision = Some(target_structural_revision);
        self.candidate_content_dirty_epoch = Some(self.running.host_content_candidate_epoch());
        self.candidate_content_commit = Some(content_commit);
        self.candidate_state_commit = Some(state_commit);
        self.content.begin_prepared_candidate(
            self.candidate_content_commit
                .as_ref()
                .expect("content commit plan must be present before in-flight pin"),
        );
        self.frame_pending = true;
        if let Err(error) = self.present_frame() {
            self.capture_failed_candidate();
            self.discard_candidate_frame();
            self.frame_pending = previous_pending;
            // `prepare_frame` clears the kernel dirty bit before the backend
            // handoff. Restore the retry obligation when that handoff fails;
            // otherwise the next flush could mistake the unchanged epoch for
            // a successful no-op and silently lose the desired frame.
            self.running.host_discard_candidate();
            return Err(error);
        }
        if self.frame_pending || self.presentation.is_some() {
            return Ok(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            });
        }
        self.commit_frame()
    }

    fn present_frame(&mut self) -> Result<()> {
        if let Some(mut receipt) = self.presentation.take() {
            match receipt.try_recv() {
                Ok(result) => {
                    if let Err(error) = result {
                        self.physical_sync_unknown = true;
                        return Err(host_attempt_error(
                            "backend",
                            "BACKEND_IO_FAILED",
                            true,
                            format!("terminal presentation failed: {error}"),
                        ));
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    self.presentation = Some(receipt);
                    return Ok(());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    self.physical_sync_unknown = true;
                    return Err(host_attempt_error(
                        "backend",
                        "BACKEND_NOT_READY",
                        false,
                        "terminal presentation reply lost",
                    ));
                }
            }
        }
        if !self.frame_pending {
            return Ok(());
        }
        let frame = self.candidate_frame.as_ref().unwrap_or(&self.frame);
        if let HostBackend::Real(backend) = &mut self.backend {
            match backend.begin_frame(frame) {
                Ok(receipt) => {
                    self.presentation = Some(receipt);
                    self.frame_pending = false;
                }
                Err(error) if crate::terminal::is_terminal_worker_stopped(&error) => {
                    self.closed = true;
                    self.physical_sync_unknown = true;
                    return Err(host_attempt_error(
                        "backend",
                        "BACKEND_NOT_READY",
                        false,
                        error.to_string(),
                    ));
                }
                Err(error) => {
                    self.physical_sync_unknown = true;
                    return Err(host_attempt_error(
                        "backend",
                        "BACKEND_IO_FAILED",
                        true,
                        error.to_string(),
                    ));
                }
            }
        } else {
            self.frame_pending = false;
        }
        Ok(())
    }

    fn commit_frame(&mut self) -> Result<HostFlushOutcome> {
        // All normal preconditions are checked before entering the
        // environment-owned completion authority. The environment mutex then
        // remains held across content/state/frame promotion and its queue
        // completion, so no independently poisonable lock is reacquired after
        // visible authority changes.
        let next_visible_frame_revision = self
            .visible_frame_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("visible frame revision exhausted"))?;
        if self.candidate_frame.is_none() {
            return Err(anyhow::anyhow!("missing candidate frame"));
        }
        let candidate_epoch = self
            .candidate_epoch
            .ok_or_else(|| anyhow::anyhow!("missing candidate frame epoch"))?;
        let candidate_structural_revision = self
            .candidate_structural_revision
            .ok_or_else(|| anyhow::anyhow!("missing candidate structural revision"))?;
        let content_dirty_epoch = self
            .candidate_content_dirty_epoch
            .ok_or_else(|| anyhow::anyhow!("missing candidate content epoch"))?;
        if self.candidate_state_commit.is_none() {
            return Err(anyhow::anyhow!("missing candidate state commit"));
        }
        let environment = self.environment.clone();
        environment.with_host_completion(self.host_id, candidate_epoch, true, false, || {
            let deferred_source_cleanup = {
                let content_commit = self
                    .candidate_content_commit
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("missing candidate content commit"))?;
                match self.content.commit_prepared(content_commit) {
                    Ok(deferred_source_cleanup) => deferred_source_cleanup,
                    Err(error) => {
                        // Preserve the failed candidate's coordinates for the
                        // error channel even if newer desired work is
                        // accepted before the explicit retry.
                        self.failed_attempt =
                            Some((candidate_epoch, candidate_structural_revision));
                        return Err(error);
                    }
                }
            };
            let candidate = self
                .candidate_frame
                .take()
                .expect("candidate frame must remain present during commit");
            self.candidate_content_commit
                .take()
                .expect("candidate content commit must remain present during commit");
            let state_commit = self
                .candidate_state_commit
                .take()
                .expect("candidate state commit must remain present during commit");
            self.view_states.commit_prepared(&state_commit);
            self.content.end_candidate();
            self.running
                .host_commit_content_candidate(content_dirty_epoch);
            self.candidate_epoch = None;
            self.candidate_structural_revision = None;
            self.candidate_content_dirty_epoch = None;
            self.candidate_state_commit = None;
            let block_for_deferred_cleanup =
                deferred_source_cleanup && self.pending_epoch == candidate_epoch;
            if self.physical_sync_unknown {
                self.running.host_recover_native_history_synchronization();
                self.physical_sync_unknown = false;
            }
            self.frame = candidate;
            self.frame_pending = false;
            self.visible_structural_revision = candidate_structural_revision;
            self.visible_frame_revision = next_visible_frame_revision;
            self.committed_epoch = candidate_epoch;
            if self.pending_epoch == candidate_epoch {
                self.content_dirty = false;
            }
            Ok((
                HostFlushOutcome {
                    committed: true,
                    waiting_for_presentation: false,
                    committed_epoch: Some(candidate_epoch),
                    visible_structural_revision: Some(candidate_structural_revision),
                },
                self.pending_epoch,
                block_for_deferred_cleanup,
            ))
        })
    }

    fn capture_failed_candidate(&mut self) {
        if let (Some(epoch), Some(revision)) =
            (self.candidate_epoch, self.candidate_structural_revision)
        {
            self.failed_attempt = Some((epoch, revision));
        }
    }

    /// Reconciles a candidate whose backend receipt has already completed
    /// before any newer dirty work is advanced or prepared.  Commit failures
    /// intentionally retain the exact candidate so an explicit retry can
    /// rerun its preflight against the same state/content/frontier plan.
    fn retry_existing_candidate(&mut self) -> Result<Option<HostFlushOutcome>> {
        let candidate_present = self.candidate_frame.is_some();
        let candidate_shape_matches = candidate_present == self.candidate_epoch.is_some()
            && candidate_present == self.candidate_structural_revision.is_some()
            && candidate_present == self.candidate_content_dirty_epoch.is_some()
            && candidate_present == self.candidate_content_commit.is_some()
            && candidate_present == self.candidate_state_commit.is_some();
        if !candidate_shape_matches {
            return Err(anyhow::anyhow!(
                "candidate frame and commit metadata must be present together"
            ));
        }

        if self.presentation.is_some() {
            let outcome = self.poll_presentation()?;
            if outcome
                .as_ref()
                .is_some_and(|outcome| outcome.committed || outcome.waiting_for_presentation)
            {
                return Ok(outcome);
            }
        }
        if self.candidate_frame.is_some() {
            if self.frame_pending {
                return Err(anyhow::anyhow!(
                    "candidate frame is pending backend submission"
                ));
            }
            return self.commit_frame().map(Some);
        }
        Ok(None)
    }

    fn note_physical_sync_failure(&mut self, error: &anyhow::Error) {
        if error
            .downcast_ref::<super::environment::HostAttemptError>()
            .is_some_and(|failure| failure.code == "HISTORY_TRANSFER_FAILED")
        {
            self.physical_sync_unknown = true;
        }
    }

    fn discard_candidate_frame(&mut self) {
        // Validate the candidate shape before clearing any ownership.  The
        // absent/absent pair is the legitimate initial bootstrap state;
        // every other partial pair is an internal invariant failure.
        self.clear_in_flight_state_bindings();
        self.content.abort_candidate();
        self.candidate_frame = None;
        self.candidate_epoch = None;
        self.candidate_structural_revision = None;
        self.candidate_content_commit = None;
        self.candidate_content_dirty_epoch = None;
        self.frame_pending = false;
        self.candidate_state_commit = None;
        self.running.host_abort_content_candidate();
    }

    fn poll_presentation(&mut self) -> Result<Option<HostFlushOutcome>> {
        if self.presentation.is_none() {
            return Ok(None);
        }
        if let Err(error) = self.present_frame() {
            self.capture_failed_candidate();
            self.discard_candidate_frame();
            self.running.host_discard_candidate();
            if !self.closed {
                self.ensure_pending()?;
            }
            return Err(error);
        }
        if self.presentation.is_some() {
            return Ok(Some(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            }));
        }
        if self.candidate_epoch.is_some() {
            return self.commit_frame().map(Some);
        }
        Ok(Some(HostFlushOutcome::default()))
    }

    fn finish_presentation_blocking(&mut self) -> Result<HostFlushOutcome> {
        loop {
            let result = super::run::wait_for_present_blocking(&mut self.presentation);
            if let Err(error) = result {
                self.physical_sync_unknown = true;
                self.capture_failed_candidate();
                self.discard_candidate_frame();
                self.running.host_discard_candidate();
                if !self.closed {
                    self.ensure_pending()?;
                }
                return Err(host_attempt_error(
                    "backend",
                    "BACKEND_IO_FAILED",
                    true,
                    format!("terminal presentation failed: {error}"),
                ));
            }
            let outcome = if self.candidate_epoch.is_some() {
                self.commit_frame()?
            } else {
                self.frame_pending = false;
                self.flush_pending_frame()?
            };
            if !outcome.waiting_for_presentation {
                return Ok(outcome);
            }
        }
    }

    fn flush_pending_frame(&mut self) -> Result<HostFlushOutcome> {
        if self.closed {
            return Err(anyhow::anyhow!("host is closed"));
        }
        if self.pending_epoch == self.committed_epoch && self.content.has_pending_source_cleanup() {
            // Deferred Source cleanup is blocked after its successful logical
            // promotion. An explicit barrier or a newly queued Source wake
            // admits one retry candidate; persistent poison is then reported
            // and blocked by the environment rather than requeued forever.
            self.ensure_pending()?;
        }

        // A completed receipt with an uncommitted candidate is an older
        // accepted frame, not a fresh preparation opportunity. Reconcile it
        // before advancing newer content/state work: beginning a new
        // projection pass would clear or overwrite the candidate and could
        // expose newer desired data through the older receipt.
        if let Some(outcome) = self.retry_existing_candidate()? {
            if outcome.committed || outcome.waiting_for_presentation {
                return Ok(outcome);
            }
        }

        self.failed_attempt = None;
        #[cfg(test)]
        if let Some(diagnostic) = self.fail_next_frame.take() {
            return Err(host_attempt_error(
                "frame",
                "FRAME_PREPARATION_FAILED",
                true,
                diagnostic,
            ));
        }
        let content_dirty = self.content.advance(self.now).map_err(|error| {
            host_attempt_error(
                "content",
                "CONTENT_SCHEDULER_FAILED",
                true,
                format!("content delivery advance failed: {error}"),
            )
        })?;
        for dirty in content_dirty {
            self.mark_content_pending(dirty)?;
        }
        let status = self.running.advance_ready(self.now).map_err(|error| {
            host_attempt_error(
                "frame",
                "FRAME_PREPARATION_FAILED",
                true,
                format!("host update failed: {error:?}"),
            )
        })?;
        if status.dirty && self.running.host_has_invalidated_components() {
            self.refresh_desired_state_bindings()?;
        }
        if status.dirty {
            self.ensure_pending()?;
        }

        if self.presentation.is_some()
            && !self.frame_pending
            && let Some(outcome) = self.poll_presentation()?
            && (outcome.committed || outcome.waiting_for_presentation)
        {
            return Ok(outcome);
        }
        // The bootstrap frame completed. Continue below so a desired
        // epoch accepted before that receipt is prepared now.

        if status.dirty {
            return self.render();
        }

        if self.frame_pending {
            if let Err(error) = self.present_frame() {
                self.capture_failed_candidate();
                self.discard_candidate_frame();
                self.running.host_discard_candidate();
                if !self.closed {
                    self.ensure_pending()?;
                }
                return Err(error);
            }
            if self.presentation.is_some() {
                return Ok(HostFlushOutcome {
                    committed: false,
                    waiting_for_presentation: true,
                    ..HostFlushOutcome::default()
                });
            }
            if self.candidate_epoch.is_some() {
                return self.commit_frame();
            }
        }

        // A receipt may have completed while visible commit was blocked by a
        // poisoned prepared record. Keep that exact candidate for retry; do
        // not start a new preparation pass over an uncommitted frame.
        if self.candidate_epoch.is_some() && self.candidate_frame.is_some() {
            return self.commit_frame();
        }

        if self.pending_epoch != self.committed_epoch {
            // Content control and Source mutations do not invalidate the
            // semantic kernel. They still require a real candidate frame so
            // Connector projection, measurement, viewport handling, and paint
            // observe the latest content before the epoch is committed.
            return self.render();
        }
        Ok(HostFlushOutcome::default())
    }

    fn advance_and_render(&mut self) -> Result<()> {
        self.flush_pending_frame().map(|_| ())
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
    states: &StateFrameView<'_>,
) -> Result<PreparedSceneFrame> {
    let mut content = EmptyContentProvider;
    prepare_frame_with_content(running, backend, now, states, &mut content)
}

fn prepare_frame_with_content(
    running: &mut HostRunning,
    backend: &mut HostBackend,
    now: Instant,
    states: &StateFrameView<'_>,
    content: &mut dyn ContentProvider,
) -> Result<PreparedSceneFrame> {
    content.set_theme(running.theme_shared());
    match backend {
        HostBackend::Headless(sink) => running
            .prepare_frame_with_states(
                now,
                sink,
                |sink| Ok(Size::new(sink.width, sink.height)),
                states,
                content,
            )
            .map_err(|error| {
                let (code, retryable) = match error {
                    SceneHostError::DidNotConverge => ("LAYOUT_DID_NOT_CONVERGE", false),
                    SceneHostError::Transfer(_) => ("HISTORY_TRANSFER_FAILED", true),
                    _ => ("FRAME_PREPARATION_FAILED", true),
                };
                host_attempt_error(
                    "frame",
                    code,
                    retryable,
                    format!("headless render failed: {error:?}"),
                )
            }),
        HostBackend::Real(backend) => running
            .prepare_frame_with_states(
                now,
                backend,
                super::super::terminal::backend::TerminalBackend::viewport,
                states,
                content,
            )
            .map_err(|error| {
                let (code, retryable) = match error {
                    SceneHostError::DidNotConverge => ("LAYOUT_DID_NOT_CONVERGE", false),
                    SceneHostError::Transfer(_) => ("HISTORY_TRANSFER_FAILED", true),
                    _ => ("FRAME_PREPARATION_FAILED", true),
                };
                host_attempt_error(
                    "frame",
                    code,
                    retryable,
                    format!("terminal render failed: {error:?}"),
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use crate::presentation::factory as vf;
    use tokio::sync::oneshot;

    use super::super::environment::TuiEnvironment;
    use super::{RoutedOutput, TuiHost};
    use crate::{
        ColorSpec, Insets, Key, KeyStroke, ViewStateGeometryPatch, ViewStatePresentationPatch,
        retained_state::StateFrameView,
    };

    #[test]
    fn native_text_input_routes_local_paste_and_submit() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let input = host.create_text_input(false).unwrap();
        host.route_text_input(&input, "submit").unwrap();
        let input_view = vf::native_component(input.component_id().unwrap());
        host.render(input_view).unwrap();

        host.dispatch_paste("typed").unwrap();
        assert_eq!(input.text().unwrap(), "typed");
        assert_eq!(host.next_output(), None);

        host.dispatch_key(KeyStroke::new(Key::Enter)).unwrap();
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "submit".to_owned(),
                payload: Some("typed".to_owned()),
            })
        );
        host.close().unwrap();
    }

    #[test]
    fn native_paste_interceptor_precedes_local_component_paste() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let input = host.create_text_input(false).unwrap();
        host.intercept_paste(&input, "intercepted").unwrap();
        host.render(vf::native_component(input.component_id().unwrap()))
            .unwrap();

        host.dispatch_paste("raw").unwrap();
        assert_eq!(input.text().unwrap(), "");
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "intercepted".to_owned(),
                payload: Some("raw".to_owned()),
            })
        );
        host.forward_paste("forwarded").unwrap();
        assert_eq!(input.text().unwrap(), "forwarded");
        assert_eq!(host.next_output(), None);
        host.close().unwrap();
    }

    #[test]
    fn native_routed_outputs_preserve_fifo_order() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let first = KeyStroke::new(Key::Char('a'));
        let second = KeyStroke::new(Key::Char('b'));
        host.bind_key(first, "first").unwrap();
        host.bind_key(second, "second").unwrap();
        host.render(vf::text("unfocused")).unwrap();

        host.dispatch_key(first).unwrap();
        host.dispatch_key(second).unwrap();
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "first".to_owned(),
                payload: None,
            })
        );
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "second".to_owned(),
                payload: None,
            })
        );
        assert_eq!(host.next_output(), None);
        host.close().unwrap();
    }

    #[test]
    fn native_global_key_fallback_preserves_local_component_precedence() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let input = host.create_text_input(false).unwrap();
        let key = KeyStroke::new(Key::Char('q'));
        host.bind_key(key, "global").unwrap();
        host.render(vf::native_component(input.component_id().unwrap()))
            .unwrap();

        host.dispatch_key(key).unwrap();
        assert_eq!(input.text().unwrap(), "q");
        assert_eq!(host.next_output(), None);

        host.render(vf::text("unfocused")).unwrap();
        host.dispatch_key(key).unwrap();
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "global".to_owned(),
                payload: None,
            })
        );
        host.close().unwrap();
    }

    #[test]
    fn native_view_slot_ticks_from_the_host_deadline() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let slot = host.create_view_slot(vf::text("first")).unwrap();
        host.render(vf::native_component(slot.component_id().unwrap()))
            .unwrap();
        slot.set_animation(
            vec![vf::text("first"), vf::text("second")],
            std::time::Duration::from_millis(16),
        )
        .unwrap();
        let before = slot.revision();

        host.advance_time(std::time::Duration::from_millis(32))
            .unwrap();
        assert!(slot.revision() > before);
        assert!(host.screen_rows().iter().any(|row| row.contains("second")));
        host.close().unwrap();
    }

    #[test]
    fn desired_revision_waits_for_a_successful_frame_barrier() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let initial = host.epochs().unwrap();
        assert_eq!(initial.desired_structural_revision, 0);
        assert_eq!(initial.visible_frame_revision, 0);
        assert_eq!(initial.pending_epoch, initial.committed_epoch);

        host.set_desired_view(vf::text("desired")).unwrap();
        let pending = host.epochs().unwrap();
        assert_eq!(pending.desired_structural_revision, 1);
        assert_eq!(pending.visible_frame_revision, 0);
        assert_ne!(pending.pending_epoch, pending.committed_epoch);

        let report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(report.errors, []);
        let visible = host.epochs().unwrap();
        assert_eq!(visible.visible_frame_revision, 1);
        assert_eq!(visible.pending_epoch, visible.committed_epoch);
        assert!(host.screen_rows().iter().any(|row| row.contains("desired")));
        host.close().unwrap();
    }

    #[test]
    fn failed_frame_keeps_old_visible_state_and_explicit_retry_recovers() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_desired_view(vf::text("old")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let old_rows = host.screen_rows();

        host.fail_next_frame_for_test("injected frame preparation failure")
            .unwrap();
        host.set_desired_view(vf::text("new")).unwrap();
        let failed = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(failed.errors.len(), 1);
        assert_eq!(host.screen_rows(), old_rows);
        let pending = host.epochs().unwrap();
        assert_eq!(pending.desired_structural_revision, 2);
        assert_eq!(pending.visible_frame_revision, 1);
        assert_ne!(pending.pending_epoch, pending.committed_epoch);

        let retried = host.flush_pending_hosts(8, true).unwrap();
        assert!(retried.errors.is_empty());
        let visible = host.epochs().unwrap();
        assert_eq!(visible.visible_frame_revision, 2);
        assert_eq!(visible.pending_epoch, visible.committed_epoch);
        assert!(host.screen_rows().iter().any(|row| row.contains("new")));
        host.close().unwrap();
    }

    #[test]
    fn presentation_state_repaints_without_measurement_or_semantic_republication() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let state = host.create_view_state().unwrap();
        let view = vf::text("state")
            .native_with_state_attachment(state.state_id())
            .unwrap();
        host.set_desired_view(view).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let before = host.epochs().unwrap();
        crate::presentation::layout::reset_layout_counters();

        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(6)));
        state.set_presentation(&patch).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let counters = crate::presentation::layout::layout_counters();
        let after = host.epochs().unwrap();
        assert_eq!(counters.0, 0, "presentation state must not measure");
        assert_eq!(
            after.desired_structural_revision,
            before.desired_structural_revision
        );
        assert!((0..4).any(|row| {
            host.style_at(row, 0)
                .and_then(|style| style.foreground)
                .as_deref()
                == Some("ansi:6")
        }));
        host.close().unwrap();
    }

    #[test]
    fn structural_publication_invalidates_retained_state_dependency_paths() {
        let host = TuiHost::open(20, 8, true).unwrap();
        let state = host.create_view_state().unwrap();
        let child = vf::text("child")
            .native_with_state_attachment(state.state_id())
            .unwrap();
        let stable = vf::column(vec![child], 0);
        host.set_desired_view(vf::column(vec![stable.clone(), vf::text("before")], 0))
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let mut patch = ViewStateGeometryPatch::default();
        patch.padding = Some(Insets::all(1));
        state.set_geometry(&patch).unwrap();

        // Publishing a new root in the same pending epoch used to clear the
        // state dirty worklist while retaining the stable ancestor's cached
        // measurement. The fresh suffix makes this the exact structural
        // publication path rather than a state-only repaint.
        host.set_desired_view(vf::column(vec![stable, vf::text("after")], 0))
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let child_row = host
            .screen_rows()
            .into_iter()
            .find(|row| row.contains("child"))
            .expect("state-attached child remains visible");
        assert_eq!(
            child_row.find("child"),
            Some(1),
            "the retained state geometry must survive the root publication"
        );
        host.close().unwrap();
    }

    #[test]
    fn component_slot_replacement_carries_captured_state_versions() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let state = host.create_view_state().unwrap();
        let slot = host.create_view_slot(vf::text("old")).unwrap();
        let slot_view = vf::native_component(slot.component_id().unwrap());
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(2)));
        state.set_presentation(&patch).unwrap();

        host.set_desired_view(slot_view).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        slot.set_view(
            vf::text("new")
                .native_with_state_attachment(state.state_id())
                .unwrap(),
        )
        .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let new_row = host
            .screen_rows()
            .iter()
            .position(|row| row.contains("new"))
            .expect("replacement view is visible");
        assert_eq!(
            host.style_at(new_row as u16, 0)
                .and_then(|style| style.foreground),
            Some("ansi:2".to_owned()),
            "incremental component replacement must preserve the captured state"
        );
        host.close().unwrap();
    }

    #[test]
    fn failed_frame_retains_old_state_versions_until_retry() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let state = host.create_view_state().unwrap();
        let view = vf::text("state")
            .native_with_state_attachment(state.state_id())
            .unwrap();
        host.set_desired_view(view).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let mut first = ViewStatePresentationPatch::default();
        first.foreground = Some(Some(ColorSpec::ansi(6)));
        state.set_presentation(&first).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let foreground_at = |host: &TuiHost| {
            (0..4).any(|row| {
                host.style_at(row, 0)
                    .and_then(|style| style.foreground)
                    .as_deref()
                    == Some("ansi:6")
            })
        };
        assert!(foreground_at(&host));

        // The injected failure fires before capture, so the failed attempt
        // must leave the old version visible with the newer desired revision
        // still pending; the retry then captures and commits the new version.
        let mut second = ViewStatePresentationPatch::default();
        second.foreground = Some(Some(ColorSpec::ansi(1)));
        state.set_presentation(&second).unwrap();
        host.fail_next_frame_for_test("injected state frame failure")
            .unwrap();
        let failed = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(failed.errors.len(), 1);
        assert!(
            foreground_at(&host),
            "failed frame must keep the old state version visible"
        );

        let retried = host.flush_pending_hosts(8, true).unwrap();
        assert!(retried.errors.is_empty());
        assert!(
            (0..4).any(|row| {
                host.style_at(row, 0)
                    .and_then(|style| style.foreground)
                    .as_deref()
                    == Some("ansi:1")
            }),
            "retry must commit the newer state version"
        );
        host.close().unwrap();
    }

    #[test]
    fn identical_themes_resolve_identically_across_hosts() {
        use crate::{StyleRef, StyleSelector, StyleSpec, Theme, ThemeColor};
        // Duplicate variants exercise declaration-order determinism: the
        // last write wins on both hosts independently.
        let theme = Theme::new()
            .with_color("accent", ThemeColor::Indexed(1))
            .with_color_variant(
                "accent",
                StyleSelector::state("mode", "error"),
                ThemeColor::Indexed(2),
            )
            .with_color_variant(
                "accent",
                StyleSelector::state("mode", "error"),
                ThemeColor::Indexed(3),
            )
            .with_style(
                "emphasis",
                StyleSpec::new()
                    .foreground(crate::ColorSpec::theme("accent"))
                    .bold(),
            );
        let first = TuiHost::open(20, 4, true).unwrap();
        let second = TuiHost::open(20, 4, true).unwrap();
        first.set_theme(theme.clone()).unwrap();
        second.set_theme(theme).unwrap();
        for host in [&first, &second] {
            host.set_desired_view(crate::presentation::factory::style(
                vf::text("parity"),
                StyleRef::theme("emphasis"),
            ))
            .unwrap();
            host.flush_pending_hosts(8, true).unwrap();
        }
        assert_eq!(first.screen_rows(), second.screen_rows());
        for row in 0..4 {
            for column in 0..6 {
                let left = first.style_at(row, column).map(|style| style.foreground);
                let right = second.style_at(row, column).map(|style| style.foreground);
                assert_eq!(left, right, "style diverged at {row}:{column}");
            }
        }
        assert!(
            first
                .style_at(3, 0)
                .and_then(|style| style.foreground)
                .as_deref()
                == Some("ansi:1"),
            "themed foreground must resolve through the shared table"
        );
        first.close().unwrap();
        second.close().unwrap();
    }

    #[test]
    fn environment_requeues_in_flight_presentation_receipts() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_desired_view(vf::text("receipt")).unwrap();
        let (sender, receiver) = oneshot::channel();
        {
            let mut inner = host.inner.lock().unwrap();
            let candidate = {
                let super::HostInner {
                    running,
                    backend,
                    now,
                    ..
                } = &mut *inner;
                super::prepare_frame(running, backend, *now, &StateFrameView::empty()).unwrap()
            };
            let state_commit = inner
                .candidate_state_commit(&candidate.state_bindings)
                .unwrap();
            let content_commit = inner.candidate_content_commit().unwrap();
            inner.content.begin_prepared_candidate(&content_commit);
            inner.candidate_frame = Some(candidate);
            inner.candidate_epoch = Some(inner.pending_epoch);
            inner.candidate_structural_revision = Some(inner.desired_structural_revision);
            inner.candidate_content_dirty_epoch =
                Some(inner.running.host_content_candidate_epoch());
            inner.candidate_content_commit = Some(content_commit);
            inner.candidate_state_commit = Some(state_commit);
            inner.frame_pending = false;
            inner.presentation = Some(receiver);
        }

        let waiting = host.flush_pending_hosts(8, false).unwrap();
        assert!(waiting.waiting_for_presentation);
        assert!(!waiting.rearm);
        sender.send(Ok(())).unwrap();
        let committed = host.flush_pending_hosts(8, false).unwrap();
        assert!(
            committed
                .commits
                .iter()
                .any(|commit| commit.host_id == host.epochs().unwrap().host_id)
        );
        assert!(host.epochs().unwrap().pending_epoch == host.epochs().unwrap().committed_epoch);
        host.close().unwrap();
    }

    #[test]
    fn failed_presentation_marks_physical_sync_unknown_until_recovery_frame() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_desired_view(vf::text("receipt-failure")).unwrap();
        let (sender, receiver) = oneshot::channel();
        {
            let mut inner = host.inner.lock().unwrap();
            let candidate = {
                let super::HostInner {
                    running,
                    backend,
                    now,
                    ..
                } = &mut *inner;
                super::prepare_frame(running, backend, *now, &StateFrameView::empty()).unwrap()
            };
            let state_commit = inner
                .candidate_state_commit(&candidate.state_bindings)
                .unwrap();
            let content_commit = inner.candidate_content_commit().unwrap();
            inner.content.begin_prepared_candidate(&content_commit);
            inner.candidate_frame = Some(candidate);
            inner.candidate_epoch = Some(inner.pending_epoch);
            inner.candidate_structural_revision = Some(inner.desired_structural_revision);
            inner.candidate_content_dirty_epoch =
                Some(inner.running.host_content_candidate_epoch());
            inner.candidate_content_commit = Some(content_commit);
            inner.candidate_state_commit = Some(state_commit);
            inner.frame_pending = false;
            inner.presentation = Some(receiver);
        }
        sender
            .send(Err(anyhow::anyhow!("simulated partial presentation")))
            .unwrap();
        let report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code, "BACKEND_IO_FAILED");
        {
            let inner = host.inner.lock().unwrap();
            assert!(inner.physical_sync_unknown);
        }
        host.flush_pending_hosts(8, true).unwrap();
        assert!(!host.inner.lock().unwrap().physical_sync_unknown);
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("receipt-failure"))
        );
        host.close().unwrap();
    }

    #[test]
    fn failed_bootstrap_receipt_reports_backend_error_without_candidate_state() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let (sender, receiver) = oneshot::channel::<Result<(), anyhow::Error>>();
        {
            let mut inner = host.inner.lock().unwrap();
            // Model a real backend bootstrap receipt: the initial frame is
            // submitted directly, so no candidate frame or state commit is
            // installed before the receipt is observed.
            inner.frame_pending = false;
            inner.presentation = Some(receiver);
            inner.mark_pending().unwrap();
        }
        sender
            .send(Err(anyhow::anyhow!(
                "simulated bootstrap presentation failure"
            )))
            .unwrap();

        let report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code, "BACKEND_IO_FAILED");
        assert_eq!(report.errors[0].phase, "backend");
        assert!(
            report.errors[0]
                .diagnostic
                .contains("simulated bootstrap presentation failure")
        );
        assert!(host.inner.lock().unwrap().candidate_state_commit.is_none());

        // The failed bootstrap is retryable through the normal explicit
        // barrier and no candidate-shape panic occurs on either path.
        let retried = host.flush_pending_hosts(8, true).unwrap();
        assert!(retried.errors.is_empty());
        assert_eq!(
            host.epochs().unwrap().pending_epoch,
            host.epochs().unwrap().committed_epoch
        );
        host.close().unwrap();
    }

    #[test]
    fn poisoned_content_commit_keeps_state_content_and_frame_authority_unchanged() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(24, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"before", &[], &[]).unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::new(
                    super::super::content::TextFunnelKind::Markdown,
                    super::super::content::TextWrapMode::Word,
                    true,
                    super::super::content::ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        let connector_id = connector.id();
        connector.activate().unwrap();
        let state = host.create_view_state().unwrap();
        let body = vf::content_host(port.id())
            .unwrap()
            .native_with_state_attachment(state.state_id())
            .unwrap();
        host.set_desired_view(body).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let before_rows = host.screen_rows();
        let before_epochs = host.epochs().unwrap();

        source.append_utf8(b"-new", &[], &[]).unwrap();
        let (sender, receiver) = oneshot::channel::<Result<(), anyhow::Error>>();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.content.begin_projection_candidate();
            let candidate = {
                let super::HostInner {
                    running,
                    backend,
                    now,
                    content,
                    ..
                } = &mut *inner;
                super::prepare_frame_with_content(
                    running,
                    backend,
                    *now,
                    &StateFrameView::empty(),
                    content,
                )
                .unwrap()
            };
            let state_commit = inner
                .candidate_state_commit(&candidate.state_bindings)
                .unwrap();
            let content_commit = inner.candidate_content_commit().unwrap();
            inner.content.begin_prepared_candidate(&content_commit);
            inner.candidate_frame = Some(candidate);
            inner.candidate_epoch = Some(inner.pending_epoch);
            inner.candidate_structural_revision = Some(inner.desired_structural_revision);
            inner.candidate_content_dirty_epoch =
                Some(inner.running.host_content_candidate_epoch());
            inner.candidate_content_commit = Some(content_commit);
            inner.candidate_state_commit = Some(state_commit);
            inner.frame_pending = false;
            inner.presentation = Some(receiver);
            inner
                .content
                .poison_connector_for_test(connector_id)
                .unwrap();
        }
        sender.send(Ok(())).unwrap();
        let report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code, "INTERNAL_INVARIANT");
        let after_error = host.epochs().unwrap();
        assert_eq!(
            after_error.visible_frame_revision,
            before_epochs.visible_frame_revision
        );
        assert_eq!(
            after_error.visible_structural_revision,
            before_epochs.visible_structural_revision
        );
        assert_eq!(host.screen_rows(), before_rows);
        assert!(port.is_mounted().unwrap());
        assert!(
            state.dispose().is_err(),
            "visible state must remain bound after poison"
        );

        // A Source wake also encounters the poisoned Connector while the old
        // candidate is retained. Repair it, then accept newer structural,
        // state, and Source work. The next flush must reconcile the old
        // candidate first rather than beginning a new preparation pass over
        // it.
        let poisoned_wake = source.append_utf8(b"-poisoned-wake", &[], &[]).unwrap();
        assert!(poisoned_wake.schedule_environment_drain);
        {
            let inner = host.inner.lock().unwrap();
            inner
                .content
                .clear_connector_poison_for_test(connector_id)
                .unwrap();
        }
        let newer_state = host.create_view_state().unwrap();
        let newer_content = vf::content_host(port.id())
            .unwrap()
            .native_with_state_attachment(newer_state.state_id())
            .unwrap();
        let newer_body = vf::column(vec![vf::text("new-root"), newer_content], 0);
        host.set_desired_view(newer_body).unwrap();
        let mut newer_patch = ViewStatePresentationPatch::default();
        newer_patch.foreground = Some(Some(ColorSpec::ansi(6)));
        newer_state.set_presentation(&newer_patch).unwrap();
        source.append_utf8(b"-newer", &[], &[]).unwrap();

        let old_retry = host.flush_pending_hosts(8, true).unwrap();
        assert!(
            old_retry
                .commits
                .iter()
                .any(|commit| commit.host_id == host.epochs().unwrap().host_id)
        );
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("before-new"))
        );

        let pending_after_old = host.epochs().unwrap();
        assert!(
            pending_after_old.pending_epoch > pending_after_old.committed_epoch,
            "newer desired/state/Source work must remain pending after old retry"
        );

        let newer = host.flush_pending_hosts(8, true).unwrap();
        assert!(newer.errors.is_empty());
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("new-root")),
            "newer desired root must be prepared after the old candidate commits"
        );
        let settled = host.epochs().unwrap();
        assert_eq!(settled.pending_epoch, settled.committed_epoch);
        assert_eq!(
            connector.status().unwrap().projected_source_revision,
            Some(4),
            "newer Source work must be projected after the retained candidate retry"
        );
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn delayed_content_receipt_preserves_newer_source_work_for_next_candidate() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(24, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"before", &[], &[]).unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::new(
                    super::super::content::TextFunnelKind::Markdown,
                    super::super::content::TextWrapMode::Word,
                    true,
                    super::super::content::ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        connector.activate().unwrap();
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        source.append_utf8(b"-candidate", &[], &[]).unwrap();
        let (sender, receiver) = oneshot::channel::<Result<(), anyhow::Error>>();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.content.begin_projection_candidate();
            let candidate = {
                let super::HostInner {
                    running,
                    backend,
                    now,
                    content,
                    ..
                } = &mut *inner;
                super::prepare_frame_with_content(
                    running,
                    backend,
                    *now,
                    &StateFrameView::empty(),
                    content,
                )
                .unwrap()
            };
            let state_commit = inner
                .candidate_state_commit(&candidate.state_bindings)
                .unwrap();
            let content_commit = inner.candidate_content_commit().unwrap();
            inner.content.begin_prepared_candidate(&content_commit);
            inner.candidate_frame = Some(candidate);
            inner.candidate_epoch = Some(inner.pending_epoch);
            inner.candidate_structural_revision = Some(inner.desired_structural_revision);
            inner.candidate_content_dirty_epoch =
                Some(inner.running.host_content_candidate_epoch());
            inner.candidate_content_commit = Some(content_commit);
            inner.candidate_state_commit = Some(state_commit);
            inner.frame_pending = false;
            inner.presentation = Some(receiver);
        }

        // This mutation is accepted while the old candidate's backend receipt
        // is outstanding. Its dirty epoch must survive the old commit and
        // force a fresh Source snapshot/product on the next candidate.
        source.append_utf8(b"-new", &[], &[]).unwrap();
        sender.send(Ok(())).unwrap();
        let committed_old = host.flush_pending_hosts(8, false).unwrap();
        let epochs_after_old = host.epochs().unwrap();
        assert!(
            committed_old
                .commits
                .iter()
                .any(|commit| commit.host_id == host.epochs().unwrap().host_id)
        );
        assert!(
            committed_old.rearm,
            "newer Source work must keep the environment queue rearmed"
        );
        assert!(
            epochs_after_old.pending_epoch > epochs_after_old.committed_epoch,
            "the newer Source epoch must remain pending after the old receipt"
        );
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("-candidate"))
        );
        assert!(!host.screen_rows().iter().any(|row| row.contains("-new")));

        host.flush_pending_hosts(8, true).unwrap();
        assert!(host.screen_rows().iter().any(|row| row.contains("-new")));
        assert_eq!(
            connector.status().unwrap().projected_source_revision,
            Some(3),
            "the newer Source revision must be promoted by its own candidate"
        );
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn host_source_cleanup_failure_preserves_membership_until_retry() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(24, 6, true, environment.clone()).unwrap();
        let first_source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        let second_source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        first_source.append_utf8(b"first", &[], &[]).unwrap();
        second_source.append_utf8(b"second", &[], &[]).unwrap();
        let first_port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let second_port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let first = first_port
            .connect(
                &first_source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        let second = second_port
            .connect(
                &second_source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        first.activate().unwrap();
        second.activate().unwrap();
        host.set_desired_view(crate::presentation::factory::row_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    vf::content_host(first_port.id()).unwrap(),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    vf::content_host(second_port.id()).unwrap(),
                ),
            ],
            0,
            crate::presentation::VerticalAlign::Top,
        ))
        .unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        assert!(first.status().unwrap().visible);
        assert!(second.status().unwrap().visible);
        assert_eq!(first_source.subscriber_count(), 1);
        assert_eq!(second_source.subscriber_count(), 1);

        first.deactivate().unwrap();
        second.deactivate().unwrap();
        {
            let mut inner = host.inner.lock().unwrap();
            inner
                .content
                .poison_source_after_first_cleanup_for_test(second_source.id());
        }
        let committed = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(committed.errors, []);
        assert_eq!(committed.commits.len(), 1);
        assert!(!committed.rearm, "a poisoned Source cleanup must not spin");
        let (_, reserved_capacity, after_capacity) = environment
            .take_last_completion_capacities()
            .expect("completion capacity sample must be available");
        assert_eq!(
            reserved_capacity, after_capacity,
            "deferred cleanup completion must not grow environment tables after promotion"
        );
        let pending_status = second.status().unwrap();
        assert!(pending_status.cleanup_pending);
        assert_eq!(
            pending_status
                .cleanup_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("SOURCE_CLEANUP_PENDING")
        );
        assert_eq!(
            host.inner
                .lock()
                .unwrap()
                .content
                .pending_source_cleanup_count(),
            1,
            "cleanup failure must become an explicit pending retry"
        );
        assert_eq!(first_source.subscriber_count(), 0);
        let peer = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        peer.set_desired_view(vf::text("peer")).unwrap();
        let fair = host.flush_pending_hosts(8, false).unwrap();
        assert!(
            fair.commits
                .iter()
                .any(|commit| commit.host_id == peer.epochs().unwrap().host_id),
            "a deferred-cleanup Host must not starve an unrelated Host"
        );
        assert!(!fair.rearm);
        peer.close().unwrap();
        // A poisoned Source cannot be read until the test clears its poison;
        // the retained token is then observable and membership remains live.
        {
            let inner = host.inner.lock().unwrap();
            inner.content.clear_source_poison_for_test(&second_source);
        }
        assert_eq!(second_source.subscriber_count(), 1);
        assert!(
            second_source.dispose().is_err(),
            "deferred cleanup must retain Source membership"
        );
        assert!(!first.status().unwrap().visible);
        assert!(!second.status().unwrap().visible);

        let recovered = host.flush_pending_hosts(8, true).unwrap();
        assert_eq!(recovered.commits.len(), 1);
        assert!(!recovered.rearm);
        assert_eq!(
            host.inner
                .lock()
                .unwrap()
                .content
                .pending_source_cleanup_count(),
            0
        );
        assert_eq!(second_source.subscriber_count(), 0);
        assert!(!second.status().unwrap().cleanup_pending);
        first.dispose().unwrap();
        second.dispose().unwrap();
        assert!(first_source.dispose().is_ok());
        assert!(second_source.dispose().is_ok());
        host.close().unwrap();
    }

    #[test]
    fn environment_drain_is_fair_and_shared_by_hosts() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        first.set_desired_view(vf::text("first")).unwrap();
        second.set_desired_view(vf::text("second")).unwrap();

        let first_batch = first.flush_pending_hosts(1, false).unwrap();
        assert_eq!(first_batch.attempted, 1);
        assert!(first_batch.rearm);
        let second_batch = first.flush_pending_hosts(1, false).unwrap();
        assert_eq!(second_batch.attempted, 1);
        assert!(!second_batch.rearm);
        assert!(first.screen_rows().iter().any(|row| row.contains("first")));
        assert!(
            second
                .screen_rows()
                .iter()
                .any(|row| row.contains("second"))
        );
        first.close().unwrap();
        second.close().unwrap();
    }

    #[test]
    fn poisoned_host_does_not_drop_unrelated_pending_hosts_from_fair_drain() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        first.set_desired_view(vf::text("poisoned")).unwrap();
        second.set_desired_view(vf::text("healthy")).unwrap();
        let first_host_id = first.epochs().unwrap().host_id;
        let second_host_id = second.epochs().unwrap().host_id;
        {
            let first_inner = first.inner.clone();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = first_inner.lock().unwrap();
                panic!("intentional fair-drain host poison");
            }));
        }

        let report = second.flush_pending_hosts(8, false).unwrap();
        assert!(
            report.errors.iter().any(|error| {
                error.code == "HOST_LOCK_POISONED" && error.host_id == first_host_id
            })
        );
        assert!(
            report
                .commits
                .iter()
                .any(|commit| commit.host_id == second_host_id),
            "a poisoned host must not discard unrelated pending work"
        );
        assert!(
            second
                .screen_rows()
                .iter()
                .any(|row| row.contains("healthy"))
        );
    }

    /// Opens two headless hosts sharing one environment/Source registry and
    /// subscribes one live connector on each host to a fresh stream Source.
    /// Returns `(healthy_first, healthy_second, source)`; callers poison one
    /// host to simulate a post-acceptance wake failure.
    fn mounted_subscribed_pair() -> (
        TuiHost,
        TuiHost,
        crate::application::content::HostContentSource,
    ) {
        use crate::application::content::{
            ContentFamily, HostContentFunnel, TextSourceKind, TextWrapMode,
        };

        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        for host in [&first, &second] {
            let port = host.create_content_port(ContentFamily::Text).unwrap();
            let connector = port
                .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
                .unwrap();
            host.set_desired_view(vf::content_host(port.id()).unwrap())
                .unwrap();
            host.flush_pending_hosts(8, true).unwrap();
            connector.activate().unwrap();
        }
        assert_eq!(
            source.subscriber_count(),
            2,
            "both hosts must hold live Source subscriptions before poisoning"
        );
        (first, second, source)
    }

    /// Poisons a host's frame mutex, simulating a subscriber that becomes
    /// unavailable after Source acceptance (lock poison surfaces exactly
    /// where `finish_mutation` reports post-acceptance wake errors).
    fn poison_host(host: &TuiHost) {
        let inner = std::sync::Arc::clone(&host.inner);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = inner.lock().unwrap();
            panic!("intentional host poison for post-acceptance wake fixture");
        }));
        assert!(result.is_err(), "poisoning panic must unwind");
        assert!(inner.is_poisoned(), "host mutex must be poisoned");
    }

    #[test]
    fn post_acceptance_wake_failure_keeps_revision_authoritative() {
        // Source post-acceptance wake failure (§9.6). The poisoned subscriber
        // is woken last, so the healthy host is already marked pending when
        // the failure surfaces. The installed revision stays authoritative
        // and observable, and the environment report carries the failed
        // host diagnostic instead of an ambiguous ordinary rejection that
        // would invite a duplicating retry.
        let (first, second, source) = mounted_subscribed_pair();
        let second_host_id = second.epochs().unwrap().host_id;
        poison_host(&second);
        let revision_before = source.snapshot().unwrap().revision;
        let epoch_before = first.epochs().unwrap().pending_epoch;

        let mutation = source.append_utf8(b"wake-auth\n", &[], &[]).unwrap();
        assert_eq!(mutation.revision, revision_before + 1);
        assert!(mutation.schedule_environment_drain);

        let report = first.flush_pending_hosts(8, false).unwrap();
        assert!(report.errors.iter().any(|error| {
            error.host_id == second_host_id
                && error.code == "SOURCE_WAKE_FAILED"
                && error
                    .diagnostic
                    .contains(&format!("revision {}", revision_before + 1))
        }));
        assert!(
            report
                .commits
                .iter()
                .any(|commit| commit.host_id == first.epochs().unwrap().host_id),
            "healthy host must continue through the failed subscriber wake"
        );

        let snapshot = source.snapshot().unwrap();
        assert_eq!(
            snapshot.revision,
            revision_before + 1,
            "accepted revision must be installed despite the wake failure"
        );
        assert!(
            snapshot.text().contains("wake-auth"),
            "accepted bytes must be readable after the wake failure"
        );
        assert_ne!(
            first.epochs().unwrap().pending_epoch,
            epoch_before,
            "hosts woken before the failure must stay marked pending"
        );
        first.close().unwrap();
        drop(second);
    }

    #[test]
    fn post_acceptance_wake_failure_still_wakes_later_subscribers() {
        // Source post-acceptance wake failure (§9.6 "continue handling other
        // eligible hosts", L1-12 "no lost wake"). The poisoned subscriber is
        // woken first: every remaining host must still be woken, and the
        // failure still reports the accepted revision.
        let (first, second, source) = mounted_subscribed_pair();
        let first_host_id = first.epochs().unwrap().host_id;
        poison_host(&first);
        let revision_before = source.snapshot().unwrap().revision;
        let epoch_before = second.epochs().unwrap().pending_epoch;

        let mutation = source.append_utf8(b"wake-auth\n", &[], &[]).unwrap();
        assert_eq!(mutation.revision, revision_before + 1);
        assert!(mutation.schedule_environment_drain);

        let report = second.flush_pending_hosts(8, false).unwrap();
        assert!(report.errors.iter().any(|error| {
            error.host_id == first_host_id
                && error.code == "SOURCE_WAKE_FAILED"
                && error
                    .diagnostic
                    .contains(&format!("revision {}", revision_before + 1))
        }));
        assert!(
            report
                .commits
                .iter()
                .any(|commit| commit.host_id == second.epochs().unwrap().host_id),
            "healthy host must continue after the first subscriber wake fails"
        );

        let snapshot = source.snapshot().unwrap();
        assert_eq!(
            snapshot.revision,
            revision_before + 1,
            "accepted revision must be installed despite the wake failure"
        );
        assert_ne!(
            second.epochs().unwrap().pending_epoch,
            epoch_before,
            "a failed subscriber must not cancel the remaining wakes"
        );
        second.close().unwrap();
        drop(first);
    }

    #[test]
    fn exhausted_desired_revision_rejects_publication_without_mutation() {
        // L1-00 step 8: counter-exhaustion atomicity for the structural
        // lane. The revision preflight runs before any state/content
        // binding or body install, so rejection leaves the visible frame
        // and the pending pipeline exactly as they were.
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_desired_view(vf::text("settled")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let settled_rows = host.screen_rows();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.desired_structural_revision = u64::MAX;
        }

        let result = host.set_desired_view(vf::text("never"));
        let message = format!("{:?}", result.unwrap_err());
        assert!(
            message.contains("desired structural revision exhausted"),
            "exhaustion must report as exhaustion, got: {message}"
        );

        host.flush_pending_hosts(8, true).unwrap();
        assert_eq!(
            host.screen_rows(),
            settled_rows,
            "rejected publication must not disturb the visible frame"
        );
        assert_eq!(
            host.epochs().unwrap().desired_structural_revision,
            u64::MAX,
            "rejected publication must not consume the revision"
        );
        host.close().unwrap();
    }

    #[test]
    fn state_patch_leaves_desired_structural_revision_untouched() {
        // L1-00 step 8: state/structural patch distinction. A retained-state
        // patch travels the state lane only: no desired-revision bump and no
        // structural republication. A structural publication consumes exactly
        // one desired revision.
        let host = TuiHost::open(20, 4, true).unwrap();
        let state = host.create_view_state().unwrap();
        let view = vf::text("lane")
            .native_with_state_attachment(state.state_id())
            .unwrap();
        host.set_desired_view(view).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let baseline = host.epochs().unwrap();
        assert_eq!(baseline.desired_structural_revision, 1);

        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(6)));
        state.set_presentation(&patch).unwrap();
        let after_state_patch = host.epochs().unwrap();
        assert_eq!(
            after_state_patch.desired_structural_revision, baseline.desired_structural_revision,
            "state patch must not consume a structural revision"
        );

        host.set_desired_view(vf::text("lane")).unwrap();
        let after_structural = host.epochs().unwrap();
        assert_eq!(
            after_structural.desired_structural_revision,
            baseline.desired_structural_revision + 1,
            "structural publication must consume exactly one revision"
        );
        host.close().unwrap();
    }

    #[test]
    fn source_wake_repaints_only_the_affected_content_port() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(32, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        connector.activate().unwrap();
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        source.append_utf8(b"target\n", &[], &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        assert!(host.screen_rows().iter().any(|row| row.contains("target")));
        assert_eq!(
            connector.status().unwrap().projected_source_revision,
            Some(1),
            "a content-only receipt must promote the prepared Source revision"
        );
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::GlobalCacheClears),
                0,
                "an isolated Source wake must not clear every host cache"
            );
            assert!(
                counters.value(crate::perf::Counter::ContentDirtyRecordsMarked) > 0,
                "the wake must carry a typed affected-ID dirty record"
            );
            assert!(
                counters.value(crate::perf::Counter::ContentMetricEvaluations) > 0,
                "Source input must be evaluated for actual metrics"
            );
        }
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn content_metric_growth_reflows_following_siblings() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(32, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"first", &[], &[]).unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        connector.activate().unwrap();
        host.set_desired_view(crate::presentation::factory::fill_width(
            crate::presentation::factory::column_specs(
                vec![
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        vf::content_host(port.id()).unwrap(),
                    ),
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        vf::text("following"),
                    ),
                ],
                0,
            ),
        ))
        .unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let before = host.screen_rows();
        let before_first = before
            .iter()
            .position(|row| row.contains("first"))
            .expect("content must be visible before growth");
        let before_following = before
            .iter()
            .position(|row| row.contains("following"))
            .expect("following sibling must be visible before growth");

        source.append_utf8(b"\nsecond", &[], &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let after = host.screen_rows();
        let after_first = after
            .iter()
            .position(|row| row.contains("first"))
            .expect("content must remain visible after growth");
        let after_following = after
            .iter()
            .position(|row| row.contains("following"))
            .expect("following sibling must remain visible after growth");
        assert_eq!(
            after_following, before_following,
            "root follow-end anchoring must preserve the following sibling position"
        );
        assert_eq!(
            after_first + 1,
            before_first,
            "content metric growth must reflow the content occurrence"
        );
        assert!(after.iter().any(|row| row.contains("second")));
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn content_dirty_work_is_local_to_one_of_many_ports() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(32, 6, true, environment.clone()).unwrap();
        let first_source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        let second_source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        first_source.append_utf8(b"first", &[], &[]).unwrap();
        second_source.append_utf8(b"second", &[], &[]).unwrap();
        let first_port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let second_port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let first_connector = first_port
            .connect(
                &first_source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        let second_connector = second_port
            .connect(
                &second_source,
                super::super::content::HostContentFunnel::plain(
                    super::super::content::TextWrapMode::Word,
                ),
            )
            .unwrap();
        first_connector.activate().unwrap();
        second_connector.activate().unwrap();
        host.set_desired_view(crate::presentation::factory::fill_width(
            crate::presentation::factory::column_specs(
                vec![
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        crate::presentation::factory::fill_width(
                            vf::content_host(first_port.id()).unwrap(),
                        ),
                    ),
                    (
                        crate::presentation::ir::TrackSize::Content { max: None },
                        crate::presentation::factory::fill_width(
                            vf::content_host(second_port.id()).unwrap(),
                        ),
                    ),
                ],
                0,
            ),
        ))
        .unwrap();
        host.flush_pending_hosts(16, true).unwrap();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();
        first_source.append_utf8(b"-update", &[], &[]).unwrap();
        host.flush_pending_hosts(16, true).unwrap();
        let rows = host.screen_rows();
        assert!(rows.iter().any(|row| row.contains("first-update")));
        assert!(rows.iter().any(|row| row.contains("second")));
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::GlobalCacheClears),
                0,
                "isolated content changes must not globally flush caches"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentPaintPropagations),
                1,
                "one Source update must schedule one content paint root"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentPathIndexNodesVisited),
                0,
                "local content refresh must use the retained semantic path index"
            );
            assert!(
                counters.value(crate::perf::Counter::MeasureNodeCalls) < 6,
                "unrelated content nodes should remain cache hits"
            );
        }
        host.close().unwrap();
        first_source.dispose().unwrap();
        second_source.dispose().unwrap();
    }

    #[test]
    fn one_of_hundreds_of_ports_has_constant_targeted_prepare_work() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        const PORT_COUNT: usize = 512;
        let environment = TuiEnvironment::new();
        let host =
            TuiHost::open_in_environment(32, PORT_COUNT as u16, true, environment.clone()).unwrap();
        let mut sources = Vec::with_capacity(PORT_COUNT);
        let mut ports = Vec::with_capacity(PORT_COUNT);
        let mut connectors = Vec::with_capacity(PORT_COUNT);
        for _ in 0..PORT_COUNT {
            let source = environment
                .create_content_source(super::super::content::TextSourceKind::Stream)
                .unwrap();
            source.append_utf8(b"x", &[], &[]).unwrap();
            let port = host
                .create_content_port(super::super::content::ContentFamily::Text)
                .unwrap();
            let connector = port
                .connect(
                    &source,
                    super::super::content::HostContentFunnel::plain(
                        super::super::content::TextWrapMode::Word,
                    ),
                )
                .unwrap();
            connector.activate().unwrap();
            sources.push(source);
            ports.push(port);
            connectors.push(connector);
        }
        host.set_desired_view(crate::presentation::factory::fill_width(
            crate::presentation::factory::column(
                ports
                    .iter()
                    .map(|port| {
                        crate::presentation::factory::fill_width(
                            vf::content_host(port.id()).unwrap(),
                        )
                    })
                    .collect(),
                0,
            ),
        ))
        .unwrap();
        host.flush_pending_hosts(PORT_COUNT, true).unwrap();
        #[cfg(feature = "perf-counters")]
        crate::perf::reset();

        // The first fixed-width leaf changes while the other 511 ports stay
        // resident and unchanged. Its Source has a distinct subscription,
        // so wake, measure, placement and commit records should scale with
        // the one affected occurrence rather than the registry size.
        sources[0].append_utf8(b"-", &[], &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        assert!(host.screen_rows().iter().any(|row| row.contains("x-")));
        #[cfg(feature = "perf-counters")]
        {
            let counters = crate::perf::snapshot();
            assert_eq!(
                counters.value(crate::perf::Counter::ContentRegistryPortScans),
                0,
                "a local Source wake must not scan every registered Port"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentWakeGroups),
                1,
                "one distinct Source subscription group must wake once"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentDirtyRecordsMarked),
                1,
                "only the changed ContentPort receives a dirty record"
            );
            assert!(
                counters.value(crate::perf::Counter::ContentCandidateRecordsPrepared)
                    < (PORT_COUNT / 2) as u64,
                "prepared commit records must remain changed-record data"
            );
            assert!(
                counters.value(crate::perf::Counter::MeasureNodeCalls) < (PORT_COUNT / 2) as u64,
                "fixed-width content mutation must not rematerialize the full layout"
            );
            assert!(
                counters.value(crate::perf::Counter::LayoutNodesEmitted) < (PORT_COUNT / 2) as u64,
                "placement work must remain bounded to the affected path"
            );
            assert_eq!(
                counters.value(crate::perf::Counter::ContentPaintPropagations),
                1,
                "one affected ContentHost must receive one paint propagation"
            );
        }
        host.close().unwrap();
        drop(connectors);
        drop(ports);
        for source in sources {
            source.dispose().unwrap();
        }
    }

    #[test]
    fn content_path_index_keeps_first_middle_and_last_updates_bounded() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();

        const SIZES: [usize; 3] = [8, 64, 512];
        const MAX_LOCAL_MEASURE_NODES: u64 = 32;
        const MAX_LOCAL_LAYOUT_NODES: u64 = 32;
        const MAX_LOCAL_COMMIT_RECORDS: u64 = 8;

        for port_count in SIZES {
            let environment = TuiEnvironment::new();
            let host = TuiHost::open_in_environment(
                32,
                u16::try_from(port_count).expect("test size fits terminal height"),
                true,
                environment.clone(),
            )
            .unwrap();
            let mut sources = Vec::with_capacity(port_count);
            let mut ports = Vec::with_capacity(port_count);
            let mut connectors = Vec::with_capacity(port_count);
            for _ in 0..port_count {
                let source = environment
                    .create_content_source(super::super::content::TextSourceKind::Stream)
                    .unwrap();
                source.append_utf8(b"x", &[], &[]).unwrap();
                let port = host
                    .create_content_port(super::super::content::ContentFamily::Text)
                    .unwrap();
                let connector = port
                    .connect(
                        &source,
                        super::super::content::HostContentFunnel::plain(
                            super::super::content::TextWrapMode::Word,
                        ),
                    )
                    .unwrap();
                connector.activate().unwrap();
                sources.push(source);
                ports.push(port);
                connectors.push(connector);
            }
            host.set_desired_view(crate::presentation::factory::fill_width(
                crate::presentation::factory::column(
                    ports
                        .iter()
                        .map(|port| {
                            crate::presentation::factory::fill_width(
                                vf::content_host(port.id()).unwrap(),
                            )
                        })
                        .collect(),
                    0,
                ),
            ))
            .unwrap();
            host.flush_pending_hosts(port_count, true).unwrap();

            for index in [0, port_count / 2, port_count - 1] {
                #[cfg(feature = "perf-counters")]
                crate::perf::reset();
                sources[index].append_utf8(b"-", &[], &[]).unwrap();
                host.flush_pending_hosts(8, true).unwrap();
                assert!(
                    host.screen_rows().iter().any(|row| row.contains("x-")),
                    "updated port {index} should be visible in size {port_count}"
                );
                #[cfg(feature = "perf-counters")]
                {
                    let counters = crate::perf::snapshot();
                    assert_eq!(
                        counters.value(crate::perf::Counter::ContentPathIndexNodesVisited),
                        0,
                        "semantic path lookup scanned siblings for size {port_count}, index {index}: {counters:?}"
                    );
                    assert!(
                        counters.value(crate::perf::Counter::MeasureNodeCalls)
                            <= MAX_LOCAL_MEASURE_NODES,
                        "local measure exceeded bound for size {port_count}, index {index}: {counters:?}"
                    );
                    assert!(
                        counters.value(crate::perf::Counter::LayoutNodesEmitted)
                            <= MAX_LOCAL_LAYOUT_NODES,
                        "local placement exceeded bound for size {port_count}, index {index}: {counters:?}"
                    );
                    assert!(
                        counters.value(crate::perf::Counter::ContentCandidateRecordsPrepared)
                            <= MAX_LOCAL_COMMIT_RECORDS,
                        "local commit preparation exceeded bound for size {port_count}, index {index}: {counters:?}"
                    );
                    assert_eq!(
                        counters.value(crate::perf::Counter::ContentRegistryPortScans),
                        0,
                        "local update scanned the Port registry for size {port_count}, index {index}: {counters:?}"
                    );
                }
            }
            host.close().unwrap();
            drop(connectors);
            drop(ports);
            for source in sources {
                source.dispose().unwrap();
            }
        }
    }

    #[test]
    fn theme_recolor_refreshes_content_paint_without_rebuilding_layout() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(32, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"# heading\n", &[], &[]).unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::new(
                    super::super::content::TextFunnelKind::Markdown,
                    super::super::content::TextWrapMode::Word,
                    true,
                    super::super::content::ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        connector.activate().unwrap();
        host.set_desired_view(vf::content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let first_theme = crate::Theme::new().with_text_style(
            crate::TextSelector::heading().level(crate::HeadingLevel::H1),
            crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(1)),
        );
        let second_theme = crate::Theme::new().with_text_style(
            crate::TextSelector::heading().level(crate::HeadingLevel::H1),
            crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(2)),
        );
        host.set_theme(first_theme).unwrap();
        let row = (0..4)
            .find(|row| host.screen_rows()[*row as usize].contains("heading"))
            .expect("content heading must be visible");
        let first = host.style_at(row, 0).and_then(|style| style.foreground);
        host.set_theme(second_theme).unwrap();
        let second = host.style_at(row, 0).and_then(|style| style.foreground);
        assert_ne!(
            first, second,
            "theme-only content repaint must resolve new styles"
        );
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn structural_theme_and_source_order_preserves_fresh_content_tickets() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(32, 5, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"# heading\n", &[], &[]).unwrap();
        let port = host
            .create_content_port(super::super::content::ContentFamily::Text)
            .unwrap();
        let connector = port
            .connect(
                &source,
                super::super::content::HostContentFunnel::new(
                    super::super::content::TextFunnelKind::Markdown,
                    super::super::content::TextWrapMode::Word,
                    true,
                    super::super::content::ContentDelivery::Immediate,
                ),
            )
            .unwrap();
        connector.activate().unwrap();
        let content = vf::content_host(port.id()).unwrap();
        let red_theme = crate::Theme::new().with_text_style(
            crate::TextSelector::heading().level(crate::HeadingLevel::H1),
            crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(1)),
        );
        host.set_desired_view(content.clone()).unwrap();
        host.set_theme(red_theme).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());
        let heading_row = host
            .screen_rows()
            .iter()
            .position(|row| row.contains("heading"))
            .expect("heading must be visible in the direct content view");
        let heading_column = host.screen_rows()[heading_row]
            .find("heading")
            .expect("heading glyph must be present") as u16;
        assert_eq!(
            host.style_at(heading_row as u16, heading_column)
                .and_then(|style| style.foreground),
            Some("ansi:1".to_owned())
        );
        let green_theme = crate::Theme::new().with_text_style(
            crate::TextSelector::heading().level(crate::HeadingLevel::H1),
            crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(2)),
        );
        host.set_theme(green_theme).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());
        assert_eq!(
            host.style_at(heading_row as u16, heading_column)
                .and_then(|style| style.foreground),
            Some("ansi:2".to_owned())
        );
        // Keep the ContentHost and its immediate ancestor identities stable
        // across each structural root; only the sibling changes.
        let stable_content = vf::style(
            vf::column(vec![content.clone()], 0),
            crate::StyleRef::theme("probe"),
        );
        let first = vf::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    stable_content.clone(),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    vf::text("tail-a"),
                ),
            ],
            0,
        );
        host.set_desired_view(first).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());

        // Structural publication followed by a theme change must not reuse a
        // detached retained tree's old ContentHost measurement/paint ticket.
        host.set_desired_view(vf::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    stable_content.clone(),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    vf::text("tail-b"),
                ),
            ],
            0,
        ))
        .unwrap();
        source.append_utf8(b"-two", &[], &[]).unwrap();
        let blue_theme = crate::Theme::new()
            .with_style(
                "probe",
                crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(4)),
            )
            .with_text_style(
                crate::TextSelector::heading().level(crate::HeadingLevel::H1),
                crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(1)),
            );
        host.set_theme(blue_theme).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());
        let heading_row = host
            .screen_rows()
            .iter()
            .position(|row| row.contains("heading"))
            .expect("heading must be visible after structural/theme refresh");
        let heading_column = host.screen_rows()[heading_row]
            .find("heading")
            .expect("heading glyph must be present") as u16;
        assert_eq!(
            host.style_at(heading_row as u16, heading_column)
                .and_then(|style| style.foreground),
            Some("ansi:1".to_owned())
        );
        assert!(host.screen_rows().iter().any(|row| row.contains("-two")));

        // Reverse order: source preparation first, then a structural root
        // replacement and theme update, must likewise use fresh products.
        source.append_utf8(b"-three", &[], &[]).unwrap();
        host.set_desired_view(vf::column_specs(
            vec![
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    stable_content.clone(),
                ),
                (
                    crate::presentation::ir::TrackSize::Content { max: None },
                    vf::text("tail-c"),
                ),
            ],
            0,
        ))
        .unwrap();
        let red_theme = crate::Theme::new()
            .with_style(
                "probe",
                crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(4)),
            )
            .with_text_style(
                crate::TextSelector::heading().level(crate::HeadingLevel::H1),
                crate::StyleSpec::new().foreground(crate::ColorSpec::ansi(2)),
            );
        host.set_theme(red_theme).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());
        assert!(host.screen_rows().iter().any(|row| row.contains("-three")));
        let heading_row = host
            .screen_rows()
            .iter()
            .position(|row| row.contains("heading"))
            .expect("heading must remain visible after reverse-order refresh");
        let heading_column = host.screen_rows()[heading_row]
            .find("heading")
            .expect("heading glyph must remain present") as u16;
        assert_eq!(
            host.style_at(heading_row as u16, heading_column)
                .and_then(|style| style.foreground),
            Some("ansi:2".to_owned())
        );
        host.close().unwrap();
        source.dispose().unwrap();
    }
}
