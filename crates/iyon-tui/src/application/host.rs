//! A language-binding host for the retained native application runtime.
//!
//! `TuiHost` deliberately exposes caller-defined outputs and native snapshots, not
//! terminal events. Components remain mounted in the same `SceneHost` used by
//! the Rust application driver.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use anyhow::Result;

use crate::controls::text_input::command::TextInputCommand;

use super::content::{ContentBinding, ContentFamily, ContentHostRegistry, HostContentPort};
use super::environment::{
    HostDrainReport, HostEpochs, HostFlushOutcome, TuiEnvironment, WakeDisposition,
    host_attempt_error,
};
use super::view_state::HostViewState;
use crate::{
    App as TuiApp, AppCx, BorderSpec, Component, ComponentCx, ComponentHandle, History,
    HistoryLayout, HistoryUnitId, InteractionResult, KeyStroke, Output, ScrollPane, TextInput,
    Theme, View,
    backend::NativeHistorySink,
    geometry::Size,
    physical::PhysicalRow,
    presentation::{ContentProvider, EmptyContentProvider},
    retained_state::{StateNodeKind, ViewStateRecord, ViewStateRegistry, ViewStateSnapshot},
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
    candidate_content_bindings: Option<Vec<ContentBinding>>,
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
        if let Ok(guard) = self.host.lock()
            && let Some(weak) = guard.as_ref()
            && let Some(inner) = weak.upgrade()
            && let Ok(mut inner) = inner.lock()
        {
            inner.running.host_retire_component(raw_id);
        }
    }

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
            .map_or_else(|_| View::spacer(0), |pane| Component::view(&*pane))
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
            .map_or_else(|_| View::spacer(0), |state| state.view.clone())
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
            .map_or_else(|_| View::spacer(0), |input| input.view())
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
    inner: Arc<Mutex<HostInner>>,
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
        let frame = prepare_frame(&mut running, &mut backend, now, &HashMap::new())?;
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
                candidate_content_bindings: None,
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
        let (_, record) = inner.view_states.create(host_id)?;
        Ok(HostViewState::new(record, &self.inner))
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
        inner.running.state.body = body.clone();
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

    pub fn next_output(&self) -> Option<RoutedOutput> {
        self.lock_mut().ok()?.running.state.outputs.pop_front()
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
            .map_or(true, |inner| inner.closed || inner.running.host_exited())
    }

    pub fn poll_terminal(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.sync_real_time();
        for _ in 0..INPUT_PUMP_BUDGET {
            if inner.running.has_pending_actions() {
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
            // Do not consume input after a routed action. The caller must
            // reduce that action before later keystrokes can change focus or
            // clear the composer.
            if inner.running.has_pending_actions() {
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

    pub fn screen_rows(&self) -> Vec<String> {
        self.lock()
            .map(|inner| inner.frame.screen_lines())
            .unwrap_or_default()
    }

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
        // semantic root. Drop both the host state's body and the scene root so
        // environment-scoped weak caches can observe expiry after disposal.
        inner.running.state.body = View::spacer(0);
        inner.running.host_set_body(View::spacer(0));
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
        if self.pending_epoch == self.committed_epoch && self.running.is_dirty() {
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

    fn state_snapshots(&self) -> Result<HashMap<u64, ViewStateSnapshot>> {
        self.view_states.snapshots()
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

    fn candidate_content_bindings(&mut self) -> Result<Vec<ContentBinding>> {
        let targets = self.running.host_current_content_attachment_targets()?;
        self.content.candidate_bindings(&targets)
    }

    fn commit_visible_state_bindings(&mut self, targets: &[(u64, StateNodeKind)]) {
        self.view_states.set_visible(targets);
    }

    fn set_in_flight_state_bindings(&mut self, ids: &[u64]) {
        self.view_states.set_in_flight(ids);
    }

    fn clear_in_flight_state_bindings(&mut self) {
        self.view_states.clear_in_flight();
    }

    pub(super) fn content_port_is_mounted(&self, id: u64) -> Result<bool> {
        self.content.port_status(id)
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

    pub(super) fn dispose_view_state(
        &mut self,
        record: &Arc<Mutex<ViewStateRecord>>,
    ) -> Result<()> {
        let id = record
            .lock()
            .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?
            .id;
        let Some(owned) = self.view_states.records.get(&id) else {
            return Ok(());
        };
        if !Arc::ptr_eq(owned, record) {
            return Err(anyhow::anyhow!("ViewState belongs to a different host"));
        }
        let mut record = record
            .lock()
            .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?;
        if record.lifecycle == crate::retained_state::ViewStateLifecycle::Disposed {
            return Ok(());
        }
        if record.desired_bound || record.visible_bound || record.in_flight_bound {
            return Err(anyhow::anyhow!(
                "STATE_MOUNTED: ViewState is still attached"
            ));
        }
        record.dispose();
        drop(record);
        self.view_states.remove(id);
        Ok(())
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

    pub(super) fn mark_content_pending(&mut self) -> anyhow::Result<WakeDisposition> {
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
        if self.content_dirty {
            self.running.host_invalidate_content();
        }
        let states = match self.state_snapshots() {
            Ok(states) => states,
            Err(error) => {
                self.content.abort_candidate();
                return Err(error);
            }
        };
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
                self.failed_attempt = Some((target_epoch, target_structural_revision));
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                return Err(error);
            }
        };
        let state_ids = candidate
            .state_bindings
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let content_bindings = match self.candidate_content_bindings() {
            Ok(bindings) => bindings,
            Err(error) => {
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                return Err(error);
            }
        };
        self.set_in_flight_state_bindings(&state_ids);
        let previous_pending = self.frame_pending;
        debug_assert!(self.candidate_frame.is_none());
        self.candidate_frame = Some(candidate);
        self.candidate_epoch = Some(target_epoch);
        self.candidate_structural_revision = Some(target_structural_revision);
        self.candidate_content_bindings = Some(content_bindings);
        self.content.begin_candidate(
            self.candidate_content_bindings
                .as_deref()
                .unwrap_or_default(),
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
                Ok(result) => result.map_err(|error| {
                    host_attempt_error(
                        "backend",
                        "BACKEND_IO_FAILED",
                        true,
                        format!("terminal presentation failed: {error}"),
                    )
                })?,
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    self.presentation = Some(receipt);
                    return Ok(());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
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
                Err(error) if error.to_string().contains("terminal worker stopped") => {
                    self.closed = true;
                    return Err(host_attempt_error(
                        "backend",
                        "BACKEND_NOT_READY",
                        false,
                        error.to_string(),
                    ));
                }
                Err(error) => {
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
        let candidate = self
            .candidate_frame
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing candidate frame"))?;
        let content_bindings = self
            .candidate_content_bindings
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing candidate content bindings"))?;
        let candidate_epoch = self
            .candidate_epoch
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing candidate frame epoch"))?;
        let candidate_structural_revision = self
            .candidate_structural_revision
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing candidate structural revision"))?;
        let state_bindings = candidate.state_bindings.clone();
        self.commit_visible_state_bindings(&state_bindings);
        self.content.commit_visible(&content_bindings);
        self.content.end_candidate();
        self.clear_in_flight_state_bindings();
        self.frame = candidate;
        self.frame_pending = false;
        self.visible_structural_revision = candidate_structural_revision;
        self.visible_frame_revision = self
            .visible_frame_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("visible frame revision exhausted"))?;
        self.committed_epoch = candidate_epoch;
        if self.pending_epoch == candidate_epoch {
            self.content_dirty = false;
        }
        self.environment.complete_host(
            self.host_id,
            self.pending_epoch,
            self.committed_epoch,
            true,
            false,
        )?;
        Ok(HostFlushOutcome {
            committed: true,
            waiting_for_presentation: false,
            committed_epoch: Some(candidate_epoch),
            visible_structural_revision: Some(candidate_structural_revision),
        })
    }

    fn capture_failed_candidate(&mut self) {
        if let (Some(epoch), Some(revision)) =
            (self.candidate_epoch, self.candidate_structural_revision)
        {
            self.failed_attempt = Some((epoch, revision));
        }
    }

    fn discard_candidate_frame(&mut self) {
        self.content.abort_candidate();
        self.candidate_frame = None;
        self.candidate_epoch = None;
        self.candidate_structural_revision = None;
        self.candidate_content_bindings = None;
        self.frame_pending = false;
        self.clear_in_flight_state_bindings();
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
        let content_advanced = self.content.advance(self.now);
        if content_advanced {
            self.content_dirty = true;
            self.ensure_pending()?;
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
    states: &HashMap<u64, ViewStateSnapshot>,
) -> Result<PreparedSceneFrame> {
    let mut content = EmptyContentProvider;
    prepare_frame_with_content(running, backend, now, states, &mut content)
}

fn prepare_frame_with_content(
    running: &mut HostRunning,
    backend: &mut HostBackend,
    now: Instant,
    states: &HashMap<u64, ViewStateSnapshot>,
    content: &mut dyn ContentProvider,
) -> Result<PreparedSceneFrame> {
    content.set_theme(running.theme());
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
                let (code, retryable) = if matches!(error, SceneHostError::DidNotConverge) {
                    ("LAYOUT_DID_NOT_CONVERGE", false)
                } else {
                    ("FRAME_PREPARATION_FAILED", true)
                };
                host_attempt_error(
                    "frame",
                    code,
                    retryable,
                    format!("headless render failed: {error:?}"),
                )
            }),
        HostBackend::Real(backend) => running
            .prepare_frame_with_states(now, backend, |backend| backend.viewport(), states, content)
            .map_err(|error| {
                let (code, retryable) = if matches!(error, SceneHostError::DidNotConverge) {
                    ("LAYOUT_DID_NOT_CONVERGE", false)
                } else {
                    ("FRAME_PREPARATION_FAILED", true)
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
    use std::collections::HashMap;

    use tokio::sync::oneshot;

    use super::super::environment::TuiEnvironment;
    use super::TuiHost;
    use crate::{ColorSpec, IntoView, View, ViewStatePresentationPatch};

    #[test]
    fn desired_revision_waits_for_a_successful_frame_barrier() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let initial = host.epochs().unwrap();
        assert_eq!(initial.desired_structural_revision, 0);
        assert_eq!(initial.visible_frame_revision, 0);
        assert_eq!(initial.pending_epoch, initial.committed_epoch);

        host.set_desired_view(View::text("desired").into_view())
            .unwrap();
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
        host.set_desired_view(View::text("old").into_view())
            .unwrap();
        host.flush_pending_hosts(8, false).unwrap();
        let old_rows = host.screen_rows();

        host.fail_next_frame_for_test("injected frame preparation failure")
            .unwrap();
        host.set_desired_view(View::text("new").into_view())
            .unwrap();
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
        let view = View::text("state")
            .into_view()
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
    fn environment_requeues_in_flight_presentation_receipts() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_desired_view(View::text("receipt").into_view())
            .unwrap();
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
                super::prepare_frame(running, backend, *now, &HashMap::new()).unwrap()
            };
            let state_ids = candidate
                .state_bindings
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            let content_bindings = inner.candidate_content_bindings().unwrap();
            inner.set_in_flight_state_bindings(&state_ids);
            inner.content.begin_candidate(&content_bindings);
            inner.candidate_frame = Some(candidate);
            inner.candidate_epoch = Some(inner.pending_epoch);
            inner.candidate_structural_revision = Some(inner.desired_structural_revision);
            inner.candidate_content_bindings = Some(content_bindings);
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
    fn environment_drain_is_fair_and_shared_by_hosts() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        first
            .set_desired_view(View::text("first").into_view())
            .unwrap();
        second
            .set_desired_view(View::text("second").into_view())
            .unwrap();

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
}
