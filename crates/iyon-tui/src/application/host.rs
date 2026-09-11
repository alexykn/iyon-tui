//! A language-binding host for the retained native application runtime.
//!
//! `TuiHost` deliberately exposes caller-defined outputs and native snapshots,
//! not terminal events. Components remain mounted in the native `SceneHost`.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Condvar, Mutex, Weak},
    task::Poll,
    time::{Duration, Instant},
};

use anyhow::Result;

use super::content::{ContentFamily, ContentHostRegistry, HostContentPort, PreparedContentCommit};
use super::environment::{
    HostDrainReport, HostEpochs, HostFlushOutcome, TuiEnvironment, WakeDisposition,
    host_attempt_error,
};
use super::{
    frame::{
        FrameFailure, HistoryReceipt, PreparedFrame, PreparedFrameProduct, PreparedSceneProducts,
        PresentReceipt, PresentationState, SceneDisposition, UiFailureNotification,
        blocking_receive,
    },
    legacy_scene::LegacySceneAdapter,
    ui_resources::UiResourceOwner,
};
use crate::controls::text_input::{TextInputPreview, command::TextInputCommand};
use crate::presentation::factory as vf;
use crate::{
    BorderSpec, Component, ComponentCx, ComponentHandle, HistoryUnitId, InteractionResult,
    KeyStroke, Output, ScrollPane, TextInput, Theme, View,
    backend::NativeHistorySink,
    geometry::Size,
    physical::PhysicalRow,
    presentation::{ContentProvider, EmptyContentProvider},
    scene::{PreparedSceneFrame, SceneHostError},
    terminal::{TerminalBackend, TerminalEvent, termwiz::TermwizBackend},
};

const MAX_FAILURE_NOTIFICATIONS: usize = 64;

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

#[derive(Clone, Debug)]
pub struct NativeUiEvent {
    pub handle: crate::occurrence::UiHandle,
    pub mask: u32,
    pub text: Option<String>,
    pub cursor_bytes: Option<usize>,
    pub key: Option<String>,
    pub revision: Option<u64>,
}

impl NativeUiEvent {
    fn payload_bytes(&self) -> anyhow::Result<usize> {
        self.text
            .as_ref()
            .map_or(0, String::len)
            .checked_add(self.key.as_ref().map_or(0, String::len))
            .ok_or_else(|| anyhow::anyhow!("UI event payload size overflow"))
    }
}

type HostRunning = crate::application::kernel::NativeRuntime;

const INPUT_PUMP_BUDGET: usize = 32;
const UI_EVENT_QUEUE_MAX_RECORDS: usize = 4_096;
const UI_EVENT_QUEUE_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UiEventQueueLimits {
    pub max_records: usize,
    pub max_bytes: usize,
}

#[derive(Debug)]
enum UiInputAdmissionError {
    Backpressure(&'static str),
    TooLarge(&'static str),
}

impl std::fmt::Display for UiInputAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backpressure(detail) => write!(formatter, "EVENT_BACKPRESSURE: {detail}"),
            Self::TooLarge(detail) => write!(formatter, "EVENT_TOO_LARGE: {detail}"),
        }
    }
}

impl std::error::Error for UiInputAdmissionError {}

impl Default for UiEventQueueLimits {
    fn default() -> Self {
        Self {
            max_records: UI_EVENT_QUEUE_MAX_RECORDS,
            max_bytes: UI_EVENT_QUEUE_MAX_BYTES,
        }
    }
}

struct PreparedUiInputEvents {
    control: crate::occurrence::ResourceKey,
    input: HostTextInput,
    preview: TextInputPreview,
    events: Vec<NativeUiEvent>,
    bytes: usize,
}

enum UiInput<'a> {
    Key(KeyStroke),
    Paste(&'a str),
}

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

impl HostBackend {
    fn begin_history_rows(
        &mut self,
        rows: Vec<PhysicalRow>,
    ) -> Result<crate::terminal::HistoryReceipt> {
        match self {
            Self::Headless(sink) => {
                let accepted = sink.insert_history_rows(&rows)?;
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let _ = sender.send(Ok(accepted));
                Ok(receiver)
            }
            Self::Real(backend) => backend.begin_history_rows(rows),
        }
    }
}

#[derive(Clone)]
struct CloseOutcome {
    error: Option<String>,
}

struct CloseOperation {
    outcome: Mutex<Option<CloseOutcome>>,
    wake: Condvar,
}

impl CloseOperation {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            outcome: Mutex::new(None),
            wake: Condvar::new(),
        })
    }

    fn complete(&self, result: &Result<()>) -> Result<()> {
        let outcome = CloseOutcome {
            error: result.as_ref().err().map(ToString::to_string),
        };
        let mut slot = self
            .outcome
            .lock()
            .map_err(|_| anyhow::anyhow!("close operation lock is poisoned"))?;
        *slot = Some(outcome);
        self.wake.notify_all();
        Ok(())
    }

    fn wait(&self) -> Result<()> {
        let mut slot = self
            .outcome
            .lock()
            .map_err(|_| anyhow::anyhow!("close operation lock is poisoned"))?;
        while slot.is_none() {
            slot = self
                .wake
                .wait(slot)
                .map_err(|_| anyhow::anyhow!("close operation lock is poisoned"))?;
        }
        match slot.as_ref().and_then(|outcome| outcome.error.as_ref()) {
            Some(error) => Err(anyhow::anyhow!(error.clone())),
            None => Ok(()),
        }
    }
}

enum HostLifecycle {
    Open,
    Faulted,
    Closing(Arc<CloseOperation>),
    Closed(Arc<CloseOperation>),
}

struct HostLifetime {
    inner: Arc<Mutex<HostInner>>,
    environment: TuiEnvironment,
    host_id: u64,
}

#[derive(Clone, Copy)]
struct AttemptStamp {
    revision: u64,
    work_epoch: u64,
    desired_revision: u64,
}

#[derive(Clone, Copy)]
enum FinalPosition {
    Restore,
    PositionAfterFinalFrame,
}

enum HistoryWork {
    Prepared {
        plan: crate::history::NativeTransferPlan,
    },
    /// Submission has moved outside `HostInner`. The plan is owned by the
    /// local submitter until a receipt exists; retaining a second copy here
    /// would make the physical work state internally ambiguous.
    Submitting,
    InFlight {
        plan: crate::history::NativeTransferPlan,
        backend: HostBackend,
        receipt: HistoryReceipt,
    },
}

/// A close waiter must register before inspecting `HistoryWork`. The
/// generation is protected by the same mutex used by the Condvar, so the
/// predicate check and the Condvar's atomic release-and-sleep operation obey
/// the standard lost-wake-free contract. State transitions happen under
/// `HostInner`; transition owners notify only after releasing that mutex.
pub(super) struct HistoryWorkSignal {
    generation: Mutex<u64>,
    wake: Condvar,
    #[cfg(test)]
    before_wait: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}

impl HistoryWorkSignal {
    fn new() -> Self {
        Self {
            generation: Mutex::new(0),
            wake: Condvar::new(),
            #[cfg(test)]
            before_wait: Mutex::new(None),
        }
    }

    fn register(&self) -> Result<(std::sync::MutexGuard<'_, u64>, u64)> {
        let guard = self
            .generation
            .lock()
            .map_err(|_| anyhow::anyhow!("History work wait lock is poisoned"))?;
        let generation = *guard;
        Ok((guard, generation))
    }

    fn wait_for_change(
        &self,
        mut guard: std::sync::MutexGuard<'_, u64>,
        generation: u64,
    ) -> Result<()> {
        while *guard == generation {
            #[cfg(test)]
            if let Some(sender) = self.before_wait.lock().unwrap().take() {
                sender.send(()).unwrap();
            }
            guard = self
                .wake
                .wait(guard)
                .map_err(|_| anyhow::anyhow!("History work wait lock is poisoned"))?;
        }
        Ok(())
    }

    pub(super) fn notify(&self) -> Result<()> {
        let mut generation = self
            .generation
            .lock()
            .map_err(|_| anyhow::anyhow!("History work wait lock is poisoned"))?;
        *generation = generation
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("History work generation exhausted"))?;
        self.wake.notify_all();
        Ok(())
    }

    #[cfg(test)]
    fn install_before_wait_hook(&self, sender: std::sync::mpsc::Sender<()>) {
        *self.before_wait.lock().unwrap() = Some(sender);
    }
}

enum PendingPresentation {
    Bootstrap(PresentReceipt),
    Frame {
        frame: Box<PreparedFrame>,
        receipt: PresentReceipt,
    },
}

struct ClosePlan {
    operation: Arc<CloseOperation>,
    pending_presentation: Option<PendingPresentation>,
    prepare_error: Option<anyhow::Error>,
    history_error: Option<anyhow::Error>,
    environment: TuiEnvironment,
    host_id: u64,
    backend: HostBackend,
    final_position: FinalPosition,
}

struct ReceiptSettlement {
    result: Result<()>,
    continue_to_final: bool,
}

enum UiPresentationObservation {
    Closed,
    Ready,
    Failed { attempt: u64, diagnostic: String },
    Pending { in_flight: bool },
    Stalled,
}

enum HistoryWorkPoll {
    None,
    Pending,
    Progress,
    Blocked,
}

#[cfg(test)]
enum ClosePhase {
    Started,
    Joined,
}

pub(crate) struct HostInner {
    running: HostRunning,
    backend: Option<HostBackend>,
    headless_history: Vec<PhysicalRow>,
    /// The last complete logical frame. Readback and visible-state queries
    /// always use this value; a candidate is kept separately until its
    /// backend receipt succeeds.
    frame: PreparedSceneFrame,
    /// One authoritative state owns the captured scene, revisions, product
    /// plans, and physical receipt. New desired work never edits this value.
    presentation_state: PresentationState,
    /// Bootstrap output has no desired UI candidate. It is kept separate from
    /// normal frame state so the first physical write cannot fabricate a
    /// PreparedFrame without captured content/state plans.
    bootstrap_pending: bool,
    bootstrap_receipt: Option<PresentReceipt>,
    /// At most one captured physical History write may be outstanding. The
    /// backend moves into `InFlight` with that exact plan; ordinary desired
    /// mutations remain accepted in HostInner and are prepared afterwards.
    history_work: Option<HistoryWork>,
    history_sink_blocked: bool,
    /// Attempt metadata retained long enough for the environment to report a
    /// failed in-flight candidate rather than a newer pending epoch.
    failed_attempt: Option<AttemptStamp>,
    attempt_revision: u64,
    now: Instant,
    headless: bool,
    lifecycle: HostLifecycle,
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
    pub(super) content: ContentHostRegistry,
    /// Canonical React occurrence owner. `legacy_scene` is a disposable
    /// renderer projection and never mutates this document.
    pub(super) ui_resources: UiResourceOwner,
    pub(super) legacy_scene: LegacySceneAdapter,
    pub(super) ui_scene_revision: u64,
    /// One coalesced native projection frontier since the last successful
    /// adapter synchronization. Metadata-only commits remain here until a
    /// physical candidate or the lightweight visibility path consumes them.
    pending_ui_changes: Option<crate::occurrence::UiChangeSet>,
    ui_editors: std::collections::HashMap<crate::occurrence::ResourceKey, HostTextInput>,
    ui_scrolls: std::collections::HashMap<crate::occurrence::ResourceKey, HostScrollPane>,
    ui_animations: std::collections::HashMap<crate::occurrence::ResourceKey, HostViewSlot>,
    ui_events: VecDeque<NativeUiEvent>,
    /// Notification for the one asynchronous native event lane.  The queue
    /// remains owned by this host; waiters only receive an owned batch after
    /// the host guard has been released.
    ui_event_notify: Arc<tokio::sync::Notify>,
    ui_event_bytes: usize,
    ui_event_backpressure: bool,
    ui_event_limits: UiEventQueueLimits,
    deferred_terminal_input: Option<TerminalEvent>,
    scheduler_failure: Option<FrameFailure>,
    /// Bounded native failure notifications consumed by the one automatic
    /// diagnostic observer. Each preparation/receipt failure is queued at its
    /// authoritative owner, preserving distinct recoverable failures in one
    /// work epoch without retaining an unbounded log.
    failure_notifications: VecDeque<UiFailureNotification>,
    dropped_failure_notifications: u64,
    #[cfg(test)]
    ui_control_keys_visited: usize,
    #[cfg(test)]
    waiting_for_presentation_hook:
        Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    #[cfg(test)]
    close_started_hook: Option<std::sync::mpsc::Sender<ClosePhase>>,
    #[cfg(test)]
    final_backend_receipt: Option<tokio::sync::oneshot::Receiver<anyhow::Result<()>>>,
    #[cfg(test)]
    test_history_receipt: Option<crate::terminal::HistoryReceipt>,
    presentation_notify: Arc<tokio::sync::Notify>,
    history_work_notify: Arc<HistoryWorkSignal>,
}

const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<HostInner>();
    assert_send::<PreparedFrame>();
    assert_send::<NativeUiEvent>();
};

impl Drop for HostInner {
    fn drop(&mut self) {
        // Host-bound handles such as History can keep the inner Arc alive
        // after the public TuiHost wrapper is dropped. Release content
        // memberships before unregistering the host so environment-owned
        // Sources cannot retain stale Connector leases.
        self.content.dispose_all();
        if let Err(error) = self.ui_resources.close() {
            eprintln!("native UI resource cleanup failed: {error}");
        }
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
    running: bool,
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
                running: true,
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

    pub(crate) fn frame_index(&self) -> Result<usize> {
        self.state
            .lock()
            .map(|state| state.frame_index)
            .map_err(|_| anyhow::anyhow!("UI animation lock is poisoned"))
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

    fn set_animation_running(&self, running: bool) -> Result<()> {
        // Keep this separate from `interval`: a zero-duration public slot is
        // a valid animation that advances on every scheduler tick, not a
        // stopped sentinel.
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("view slot lock is poisoned"))?;
        if state.running != running {
            state.running = running;
            if running {
                state.last_tick = None;
            }
        }
        Ok(())
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
        if !state.running || state.frames.len() < 2 {
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
        drop(inner);
        render_host_after_mutation(&host)
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
        drop(inner);
        render_host_after_mutation(&host)
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
        drop(inner);
        render_host_after_mutation(&host)
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

/// Native retained interaction host used by language bindings.
#[derive(Clone)]
pub struct TuiHost {
    pub(crate) inner: Arc<Mutex<HostInner>>,
    /// Counts only public host owners. Internal queue/receipt references keep
    /// `inner` alive but never acquire authority to close the host.
    #[allow(dead_code)] // Drop of the pointee owns final host teardown.
    lifetime: Arc<HostLifetime>,
}

impl TuiHost {
    pub fn open(width: u16, height: u16, headless: bool) -> Result<Self> {
        let namespace = crate::occurrence::HostNamespace::allocate()
            .ok_or_else(|| anyhow::anyhow!("UI host namespace exhausted"))?;
        Self::open_in_environment_with_ui(width, height, headless, TuiEnvironment::new(), namespace)
    }

    /// Opens a host in an existing native environment. Hosts sharing this
    /// environment share one pending-host queue and wake latch.
    pub fn open_in_environment(
        width: u16,
        height: u16,
        headless: bool,
        environment: TuiEnvironment,
    ) -> Result<Self> {
        let namespace = crate::occurrence::HostNamespace::allocate()
            .ok_or_else(|| anyhow::anyhow!("UI host namespace exhausted"))?;
        Self::open_in_environment_with_ui(width, height, headless, environment, namespace)
    }

    pub fn open_in_environment_with_ui(
        width: u16,
        height: u16,
        headless: bool,
        environment: TuiEnvironment,
        ui_namespace: crate::occurrence::HostNamespace,
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
        let frame = prepare_frame(&mut running, &mut backend, now)?;
        let inner =
            Arc::new(Mutex::new(HostInner {
                running,
                backend: Some(backend),
                headless_history: Vec::new(),
                frame,
                presentation_state: PresentationState::Idle,
                bootstrap_pending: true,
                bootstrap_receipt: None,
                history_work: None,
                history_sink_blocked: false,
                failed_attempt: None,
                attempt_revision: 0,
                now,
                headless,
                lifecycle: HostLifecycle::Open,
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
                content: ContentHostRegistry::new(environment.content_source_registry().map_err(
                    |error| anyhow::anyhow!("content environment setup failed: {error}"),
                )?),
                ui_resources: UiResourceOwner::new(ui_namespace, environment.clone()),
                legacy_scene: LegacySceneAdapter::default(),
                ui_scene_revision: 0,
                pending_ui_changes: None,
                ui_editors: std::collections::HashMap::new(),
                ui_scrolls: std::collections::HashMap::new(),
                ui_animations: std::collections::HashMap::new(),
                ui_events: VecDeque::new(),
                ui_event_notify: Arc::new(tokio::sync::Notify::new()),
                ui_event_bytes: 0,
                ui_event_backpressure: false,
                ui_event_limits: UiEventQueueLimits::default(),
                deferred_terminal_input: None,
                scheduler_failure: None,
                failure_notifications: VecDeque::new(),
                dropped_failure_notifications: 0,
                #[cfg(test)]
                ui_control_keys_visited: 0,
                #[cfg(test)]
                waiting_for_presentation_hook: None,
                #[cfg(test)]
                close_started_hook: None,
                #[cfg(test)]
                final_backend_receipt: None,
                #[cfg(test)]
                test_history_receipt: None,
                presentation_notify: Arc::new(tokio::sync::Notify::new()),
                history_work_notify: Arc::new(HistoryWorkSignal::new()),
            }));
        let host_id = environment.register_host(&inner)?;
        let mut host = inner
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        host.host_id = host_id;
        host.content.set_owner_host(Arc::downgrade(&inner));
        if let Err(error) = host.present_frame() {
            drop(host);
            environment.unregister_host(host_id);
            return Err(error);
        }
        drop(host);
        Ok(Self {
            lifetime: Arc::new(HostLifetime {
                inner: Arc::clone(&inner),
                environment: environment.clone(),
                host_id,
            }),
            inner,
        })
    }

    pub fn ui_namespace(&self) -> Result<u32> {
        Ok(self.lock()?.ui_resources.namespace.get())
    }

    pub fn ui_body_handle(&self) -> Result<crate::occurrence::UiHandle> {
        self.lock()?
            .ui_resources
            .body_handle()
            .map_err(anyhow::Error::msg)
    }

    pub fn ui_history_unit_identity(
        &self,
        handle: crate::occurrence::UiHandle,
    ) -> Result<Option<u64>> {
        let inner = self.lock()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: occurrence belongs to another host"
            ));
        }
        let key = handle
            .node_key()
            .ok_or_else(|| anyhow::anyhow!("HISTORY_UNSUPPORTED: requires an occurrence"))?;
        Ok(inner
            .ui_resources
            .history_unit(key)
            .map(|unit| unit.id.value()))
    }

    pub fn ui_content_visible(&self) -> Result<bool> {
        let inner = self.lock()?;
        let revision = inner.ui_resources.document.as_ref().map_or(
            0,
            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
        );
        if inner.visible_structural_revision < revision {
            return Ok(false);
        }
        inner.content.ui_content_visible(&inner.ui_resources)
    }

    pub fn ui_port_mounted(&self, handle: crate::occurrence::UiHandle) -> Result<bool> {
        let inner = self.lock()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: resource belongs to another host"
            ));
        }
        let key = handle
            .resource_key()
            .ok_or_else(|| anyhow::anyhow!("WRONG_KIND: expected a UI Port"))?;
        if key.kind != crate::occurrence::HandleKind::Port {
            return Err(anyhow::anyhow!("WRONG_KIND: expected a UI Port"));
        }
        if !inner.ui_resources.resource_is_live(key) {
            return Err(anyhow::anyhow!("STALE_HANDLE: UI Port is unavailable"));
        }
        inner.content.ui_port_mounted(key)
    }

    pub fn ui_connector_status(
        &self,
        handle: crate::occurrence::UiHandle,
    ) -> Result<super::content::ContentConnectorStatus> {
        let inner = self.lock()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: resource belongs to another host"
            ));
        }
        let key = handle
            .resource_key()
            .ok_or_else(|| anyhow::anyhow!("WRONG_KIND: expected a UI Connector"))?;
        if key.kind != crate::occurrence::HandleKind::Connector {
            return Err(anyhow::anyhow!("WRONG_KIND: expected a UI Connector"));
        }
        if !inner.ui_resources.resource_is_live(key) {
            return Err(anyhow::anyhow!("STALE_HANDLE: UI Connector is unavailable"));
        }
        inner.content.ui_connector_status(&inner.ui_resources, key)
    }

    pub fn focus_ui(&self, handle: crate::occurrence::UiHandle) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: occurrence belongs to another host"
            ));
        }
        let key = handle
            .node_key()
            .ok_or_else(|| anyhow::anyhow!("FOCUS_UNSUPPORTED: focus requires an occurrence"))?;
        let snapshot = inner
            .ui_resources
            .document_snapshot(key)
            .map_err(anyhow::Error::msg)?;
        if snapshot.hidden {
            return Err(anyhow::anyhow!(
                "FOCUS_UNAVAILABLE: occurrence is hidden in the desired frame"
            ));
        }
        let demanded = inner
            .ui_resources
            .document
            .as_ref()
            .expect("open UI resource owner retains document")
            .demanded_node(key, |control| {
                inner
                    .ui_resources
                    .control_state(control)
                    .and_then(crate::occurrence::ControlState::animation_active_frame)
                    .map(|frame| frame as usize)
            })
            .map_err(|error| {
                anyhow::anyhow!("FOCUS_UNAVAILABLE: occurrence demand lookup failed: {error:?}")
            })?
            .0;
        if !demanded {
            return Err(anyhow::anyhow!(
                "FOCUS_UNAVAILABLE: occurrence is hidden or inactive in the desired frame"
            ));
        }
        let control = snapshot.control.ok_or_else(|| {
            anyhow::anyhow!("FOCUS_UNSUPPORTED: occurrence has no native control")
        })?;
        let component = inner
            .ui_editors
            .get(&control)
            .and_then(HostTextInput::component_id)
            .or_else(|| {
                inner
                    .ui_scrolls
                    .get(&control)
                    .and_then(HostScrollPane::component_id)
            })
            .or_else(|| {
                inner
                    .ui_animations
                    .get(&control)
                    .and_then(HostViewSlot::component_id)
            })
            .ok_or_else(|| anyhow::anyhow!("FOCUS_UNAVAILABLE: control is not mounted"))?;
        let geometry = inner.frame.component_geometry.clone();
        if geometry
            .entries
            .get(&crate::component::ComponentId::from_raw(component))
            .and_then(|entry| entry.visible)
            .is_none()
        {
            return Err(anyhow::anyhow!(
                "FOCUS_UNAVAILABLE: control is not visible in the confirmed frame"
            ));
        }
        if inner.running.host_focused_component() == Some(component) {
            return Ok(());
        }
        if inner.running.host_focus_component(component, &geometry) {
            inner.ensure_pending()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "FOCUS_UNAVAILABLE: control is not visible in the confirmed frame"
            ))
        }
    }

    pub fn ui_visible_geometry(
        &self,
        handle: crate::occurrence::UiHandle,
    ) -> Result<Option<(u16, u16, u16, u16)>> {
        let inner = self.lock()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: occurrence belongs to another host"
            ));
        }
        let key = handle
            .node_key()
            .ok_or_else(|| anyhow::anyhow!("GEOMETRY_UNSUPPORTED: requires an occurrence"))?;
        let snapshot = inner
            .ui_resources
            .document_snapshot(key)
            .map_err(anyhow::Error::msg)?;
        if snapshot.hidden {
            return Ok(None);
        }
        let demanded = inner
            .ui_resources
            .document
            .as_ref()
            .expect("open UI resource owner retains document")
            .demanded_node(key, |control| {
                inner
                    .ui_resources
                    .control_state(control)
                    .and_then(crate::occurrence::ControlState::animation_active_frame)
                    .map(|frame| frame as usize)
            })
            .map_err(|error| {
                anyhow::anyhow!("GEOMETRY_UNAVAILABLE: occurrence demand lookup failed: {error:?}")
            })?
            .0;
        if !demanded {
            return Ok(None);
        }
        let Some(control) = snapshot.control else {
            return Ok(inner
                .frame
                .occurrence_geometry
                .get(&key)
                .and_then(|geometry| geometry.visible)
                .map(|rect| (rect.x, rect.y, rect.width, rect.height)));
        };
        let component = inner
            .ui_editors
            .get(&control)
            .and_then(HostTextInput::component_id)
            .or_else(|| {
                inner
                    .ui_scrolls
                    .get(&control)
                    .and_then(HostScrollPane::component_id)
            })
            .or_else(|| {
                inner
                    .ui_animations
                    .get(&control)
                    .and_then(HostViewSlot::component_id)
            });
        let Some(component) = component else {
            return Ok(None);
        };
        Ok(inner
            .frame
            .component_geometry
            .entries
            .get(&crate::component::ComponentId::from_raw(component))
            .and_then(|geometry| geometry.visible)
            .map(|rect| (rect.x, rect.y, rect.width, rect.height)))
    }

    pub fn drain_ui_events(&self) -> Result<Vec<NativeUiEvent>> {
        let mut inner = self.lock_mut()?;
        Ok(inner.take_ui_event_batch())
    }

    /// Lowers the host event admission bounds for a deterministic embedding
    /// or test. Bounds never grow after host construction, and an occupied
    /// queue cannot be configured below its current owned occupancy.
    pub fn set_ui_event_limits(&self, max_records: usize, max_bytes: usize) -> Result<()> {
        let limits = UiEventQueueLimits {
            max_records,
            max_bytes,
        };
        if limits.max_records == 0 || limits.max_bytes == 0 {
            return Err(anyhow::anyhow!("UI event queue limits must be positive"));
        }
        let mut inner = self.lock_mut()?;
        if limits.max_records > inner.ui_event_limits.max_records
            || limits.max_bytes > inner.ui_event_limits.max_bytes
        {
            return Err(anyhow::anyhow!("UI event queue limits may only decrease"));
        }
        let records = inner.ui_events.len();
        let bytes = inner.ui_event_bytes;
        if limits.max_records < records || limits.max_bytes < bytes {
            return Err(anyhow::anyhow!(
                "UI event queue limits are below current occupancy"
            ));
        }
        inner.ui_event_limits = limits;
        Ok(())
    }

    /// Waits for and takes one owned event batch.  Notification registration
    /// precedes queue inspection so an event arriving between those steps
    /// cannot be stranded.  No host guard, JS value, or borrowed payload is
    /// held across the await.
    pub async fn wait_for_ui_events(&self) -> Result<Option<Vec<NativeUiEvent>>> {
        loop {
            let notification = {
                let inner = self.lock()?;
                Arc::clone(&inner.ui_event_notify)
            };
            let notified = notification.notified();
            tokio::pin!(notified);

            let batch = {
                let mut inner = self.lock_mut()?;
                if inner.is_closed() || !inner.ui_resources.is_open() {
                    return Ok(None);
                }
                if inner.ui_events.is_empty() {
                    None
                } else {
                    Some(inner.take_ui_event_batch())
                }
            };
            if let Some(batch) = batch {
                return Ok(Some(batch));
            }
            notified.await;
        }
    }

    /// Waits for the next native scheduler/content/presentation failure.
    /// Registration precedes queue inspection, so a failure published between
    /// those steps cannot be lost. The observer consumes an owned bounded
    /// notification; it never drives work or scans failed resources.
    pub async fn wait_for_ui_failure(&self) -> Result<Option<UiFailureNotification>> {
        loop {
            let notification = self.presentation_notification()?;
            let notified = notification.notified();
            tokio::pin!(notified);
            let failure = {
                let mut inner = self.lock_mut()?;
                if matches!(
                    inner.lifecycle,
                    HostLifecycle::Closing(_) | HostLifecycle::Closed(_)
                ) || !inner.ui_resources.is_open()
                {
                    return Ok(None);
                }
                if inner.dropped_failure_notifications != 0 {
                    let dropped = std::mem::take(&mut inner.dropped_failure_notifications);
                    let mut overflow = inner
                        .failure_notifications
                        .front()
                        .expect("overflow must retain a bounded failure queue")
                        .clone();
                    overflow.phase = "host".to_owned();
                    overflow.code = "LIMIT_EXCEEDED".to_owned();
                    overflow.retryable = false;
                    overflow.diagnostic = format!(
                        "native diagnostic observer fell behind: {dropped} failure notifications were dropped before this retained attempt"
                    );
                    Some(overflow)
                } else {
                    inner.failure_notifications.pop_front()
                }
            };
            if let Some(failure) = failure {
                return Ok(Some(failure));
            }
            notified.await;
        }
    }

    pub fn fail_ui_connector_for_test(
        &self,
        handle: crate::occurrence::UiHandle,
        diagnostic: String,
    ) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if handle.host_namespace != inner.ui_resources.namespace.get()
            || handle.kind != crate::occurrence::HandleKind::Connector
        {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: UI Connector belongs to another host"
            ));
        }
        let connector_key = handle
            .resource_key()
            .ok_or_else(|| anyhow::anyhow!("STALE_HANDLE: invalid UI Connector handle"))?;
        let owner_key = inner
            .ui_resources
            .resource_port_key(connector_key)
            .or_else(|| inner.content.ui_connector_owner_key(connector_key))
            .unwrap_or(connector_key);
        inner.content.fail_ui_connector(owner_key, diagnostic)?;
        // The test seam models a failure on the next actual candidate
        // preparation. Re-admit the host after installing the injection so
        // the native scheduler, rather than a caller-side flush, observes it.
        inner.ensure_pending()
    }

    pub fn fail_next_ui_connector_for_test(&self, diagnostic: String) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.content.fail_next_ui_connector_for_test(diagnostic)?;
        inner.ensure_pending()
    }

    pub fn close_ui_state(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        if inner.is_closed() {
            return Err(anyhow::anyhow!("HOST_DISPOSED: host is closed"));
        }
        let result = inner.ui_resources.close().map_err(anyhow::Error::msg);
        // Root/UI-state disposal is a terminal observation for UI barriers,
        // even when the native terminal host remains available for unrelated
        // work. Never leave an observer waiting for a queue drain that can no
        // longer make the occurrence document visible.
        inner.presentation_notify.notify_waiters();
        inner.ui_event_notify.notify_waiters();
        result
    }

    #[cfg(test)]
    pub(crate) fn push_history_unit_for_test(&self, view: View) -> Result<HistoryUnitId> {
        let unit = {
            let mut inner = self.lock_mut()?;
            let unit = inner
                .running
                .scene_history_mut()
                .ok_or_else(|| anyhow::anyhow!("host history is unavailable"))?
                .push(view)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            inner.running.invalidate_frame();
            unit
        };
        render_host_after_mutation(&self.inner)?;
        Ok(unit)
    }

    pub fn commit_ui(
        &self,
        batch: crate::occurrence::UiCommit,
        sources: &[crate::application::content::HostContentSource],
    ) -> std::result::Result<crate::occurrence::UiOperationResult, crate::occurrence::UiRejection>
    {
        let (result, wakes) = {
            let mut inner = self.lock_mut().map_err(|error| {
                crate::occurrence::UiRejection::internal(
                    0,
                    crate::occurrence::CommitDetail::Invariant,
                    error.to_string(),
                )
            })?;
            if inner.is_closed() {
                return Err(crate::occurrence::UiRejection::internal(
                    0,
                    crate::occurrence::CommitDetail::Invariant,
                    "HOST_DISPOSED: host is closed",
                ));
            }
            let mut existing_history_units = HashMap::new();
            if let Some(history) = inner.running.scene_history() {
                for operation in batch.operations() {
                    let crate::occurrence::UiOperation::CreateRoot {
                        local_ordinal,
                        role: crate::occurrence::RootRole::LegacyHistoryUnit,
                        ..
                    } = operation
                    else {
                        continue;
                    };
                    let Some(raw_identity) = batch.root_config(*local_ordinal).unit_identity else {
                        continue;
                    };
                    let Some(identity) = HistoryUnitId::from_value(raw_identity) else {
                        continue;
                    };
                    if let Some(is_live) = history.unit_is_live(identity) {
                        existing_history_units.insert(identity, is_live);
                    }
                }
            }
            let prepared =
                inner
                    .ui_resources
                    .prepare_commit(batch, sources, &existing_history_units)?;
            let current_revision = inner.ui_resources.document.as_ref().map_or(
                0,
                crate::occurrence::OccurrenceDocument::accepted_ui_revision,
            );
            let provisional = inner
                .reserve_ui_change_capacity(prepared.changes())
                .map_err(|error| {
                    crate::occurrence::UiRejection::internal(
                        current_revision,
                        crate::occurrence::CommitDetail::Capacity,
                        error.to_string(),
                    )
                })?;
            let output = match inner.ui_resources.apply_prepared_commit(prepared) {
                Ok(output) => output,
                Err(rejection) => return Err(rejection),
            };
            let crate::application::ui_resources::UiCommitOutput {
                result,
                wakes,
                changes,
            } = output;
            if let Some(frontier) = provisional {
                debug_assert!(inner.pending_ui_changes.is_none());
                if changes.has_work() {
                    inner.pending_ui_changes = Some(frontier);
                }
            }
            inner.queue_ui_changes(changes);
            if result.acknowledgement.wake_flags != 0
                && let Err(error) = inner.mark_pending()
            {
                // The UI and Source transaction is already authoritative.
                // Retain the scheduler failure for the native barrier/error
                // lane instead of reporting a false desired-state rejection.
                debug_assert!(inner.scheduler_failure.is_some());
                let _ = error;
            }
            (result, wakes)
        };
        crate::application::content::HostContentSource::finish_prepared_wakes(wakes);
        Ok(result)
    }

    /// Creates a host-owned `ContentPort`. Source/Funnel identity remains
    /// separate from the structural attachment; plain content projection is
    /// prepared only when the port is mounted and selected.
    pub fn create_content_port(&self, family: ContentFamily) -> Result<HostContentPort> {
        let mut inner = self.lock_mut()?;
        if inner.is_closed() {
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
        inner.ensure_open()?;
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
    #[cfg(test)]
    pub(crate) fn set_test_view(&self, body: View) -> Result<WakeDisposition> {
        let mut inner = self.lock_mut()?;
        if inner.is_closed() {
            return Err(anyhow::anyhow!("host is closed"));
        }
        let content_targets = inner.running.host_content_attachment_targets(&body)?;
        inner.content.validate_targets(&content_targets)?;
        let next_revision = inner
            .desired_structural_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("desired structural revision exhausted"))?;
        inner.content.set_desired(&content_targets)?;
        inner.running.host_set_body(body);
        inner.desired_structural_revision = next_revision;
        inner.mark_pending()
    }

    /// Synchronously attempts the current host's pending frame. This is the
    /// explicit host visibility barrier.
    pub fn flush_pending(&self) -> Result<()> {
        let (result, environment, host_id, history_signal) = {
            let mut inner = self.lock_mut()?;
            let result = inner.flush_for_environment(true, true);
            (
                result,
                inner.environment.clone(),
                inner.host_id,
                inner.history_work_signal(),
            )
        };
        history_signal.notify()?;
        let (outcome, _, _) = result?;
        if !outcome.waiting_for_physical_work {
            return Ok(());
        }
        let report = environment.drain_pending_for(32, true, Some(host_id))?;
        if let Some(error) = report.errors.first() {
            return Err(anyhow::anyhow!("{}: {}", error.code, error.diagnostic));
        }
        // Headless compatibility receipts are already resolved; the worker
        // wake path will perform this second queue turn for real terminals.
        let report = environment.drain_pending_for(32, true, Some(host_id))?;
        if let Some(error) = report.errors.first() {
            return Err(anyhow::anyhow!("{}: {}", error.code, error.diagnostic));
        }
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

    /// Waits for a native presentation barrier without requiring a JS frame
    /// pump. Queue service and receipt ownership remain native; the async
    /// caller only observes the settled revision and never receives a
    /// terminal buffer or host lock.
    pub async fn wait_for_ui_presentation(
        &self,
        target_revision: u64,
        content_visible: bool,
    ) -> Result<()> {
        let mut admitted_failure_attempt = None;
        loop {
            // Register before inspecting state. A receipt can complete in the
            // interval between those operations; the pinned notification then
            // observes the host-local outcome instead of losing the wake.
            let notification = self.presentation_notification()?;
            let notified = notification.notified();
            tokio::pin!(notified);

            let observation = self.observe_ui_presentation(target_revision, content_visible)?;
            match observation {
                UiPresentationObservation::Closed => {
                    return Err(anyhow::anyhow!("HOST_DISPOSED: host is closed"));
                }
                UiPresentationObservation::Ready => return Ok(()),
                UiPresentationObservation::Failed {
                    attempt,
                    diagnostic,
                } => {
                    if admitted_failure_attempt == Some(attempt) {
                        // `ensure_pending` below only admits the retry. The
                        // failed state remains authoritative until the driver
                        // starts that newer attempt, so do not return the old
                        // diagnostic before it has had a chance to run.
                        notified.await;
                        continue;
                    }
                    if admitted_failure_attempt.is_some() {
                        return Err(anyhow::anyhow!(diagnostic));
                    }
                    // Admit exactly one retry for this observed failure. The
                    // attempt stamp, rather than a boolean, distinguishes the
                    // old failure from the result of this retry.
                    self.lock_mut()?.ensure_pending()?;
                    admitted_failure_attempt = Some(attempt);
                    notified.await;
                }
                UiPresentationObservation::Pending { in_flight } => {
                    if !in_flight {
                        self.lock_mut()?.ensure_pending()?;
                    }
                    notified.await;
                }
                UiPresentationObservation::Stalled => {
                    return Err(anyhow::anyhow!(
                        "FRAME_BARRIER_STALLED: native work remains pending but no host is runnable"
                    ));
                }
            }
        }
    }

    fn observe_ui_presentation(
        &self,
        target_revision: u64,
        content_visible: bool,
    ) -> Result<UiPresentationObservation> {
        let inner = self.lock()?;
        if inner.is_closed() {
            return Ok(UiPresentationObservation::Closed);
        }
        if !inner.ui_resources.is_open() {
            return Ok(UiPresentationObservation::Closed);
        }
        let content_is_visible = if content_visible {
            inner.content.ui_content_visible(&inner.ui_resources)?
        } else {
            true
        };
        if inner.visible_structural_revision >= target_revision && content_is_visible {
            return Ok(UiPresentationObservation::Ready);
        }
        if let Some(failure) = &inner.scheduler_failure {
            return Ok(UiPresentationObservation::Failed {
                attempt: inner.attempt_revision,
                diagnostic: failure.diagnostic.clone(),
            });
        }
        if let PresentationState::Failed(failure) = &inner.presentation_state {
            return Ok(UiPresentationObservation::Failed {
                attempt: inner
                    .failed_attempt
                    .map_or(inner.attempt_revision, |attempt| attempt.revision),
                diagnostic: failure.diagnostic.clone(),
            });
        }
        let in_flight =
            inner.bootstrap_receipt.is_some() || inner.presentation_state.is_in_flight();
        if inner.pending_epoch != inner.committed_epoch || in_flight {
            return Ok(UiPresentationObservation::Pending { in_flight });
        }
        Ok(UiPresentationObservation::Stalled)
    }

    pub(super) fn presentation_notification(&self) -> Result<std::sync::Arc<tokio::sync::Notify>> {
        Ok(self.lock()?.presentation_notification())
    }

    pub fn create_text_input(&self, multiline: bool) -> Result<HostTextInput> {
        let input = HostTextInput::new(multiline);
        input.attach_host(&self.inner)?;
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        let handle = inner.running.host_register(MountedTextInput(input.clone()));
        input.set_component_id(handle.raw_id())?;
        Ok(input)
    }

    pub fn bind_key(&self, key: KeyStroke, route_id: impl Into<String>) -> Result<()> {
        let route_id = route_id.into();
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner.running.host_bind_key(key, move || RoutedOutput {
            route_id: route_id.clone(),
            payload: None,
        });
        Ok(())
    }

    pub fn exit(&self) -> Result<()> {
        close_host_inner_with_position(&self.inner, FinalPosition::PositionAfterFinalFrame)
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

    /// Services only native deadlines for the environment driver. Terminal
    /// input remains owned by the terminal input reader; this path never
    /// invokes JavaScript and does not manufacture a frame when no deadline
    /// is registered.
    pub(super) fn service_native_deadline(&self) -> Result<u64> {
        let mut inner = self.lock_mut()?;
        let signal = inner.history_work_signal();
        let result = inner.service_native_deadline_inner();
        drop(inner);
        signal.notify()?;
        result
    }

    pub fn route_text_input(
        &self,
        input: &HostTextInput,
        route_id: impl Into<String>,
    ) -> Result<()> {
        let output = input.submitted()?;
        let route_id = route_id.into();
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner
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
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner
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
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner
            .running
            .host_intercept_paste(handle, move |text| RoutedOutput {
                route_id: route_id.clone(),
                payload: Some(text),
            });
        Ok(())
    }

    /// Routes paste for an accepted React Editor occurrence through the
    /// existing native router.  The occurrence handle is validated while the
    /// host lock is held; callback routing remains native and global-before-
    /// local, just like the legacy HostTextInput entrypoint.
    pub fn intercept_ui_paste(
        &self,
        handle: crate::occurrence::UiHandle,
        route_id: impl Into<String>,
    ) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        if handle.host_namespace != inner.ui_resources.namespace.get() {
            return Err(anyhow::anyhow!(
                "STALE_HANDLE: occurrence belongs to another host"
            ));
        }
        let key = handle
            .node_key()
            .ok_or_else(|| anyhow::anyhow!("PASTE_UNSUPPORTED: requires an occurrence"))?;
        let snapshot = inner
            .ui_resources
            .document_snapshot(key)
            .map_err(anyhow::Error::msg)?;
        let control = snapshot
            .control
            .ok_or_else(|| anyhow::anyhow!("PASTE_UNSUPPORTED: occurrence is not an Editor"))?;
        let input = inner
            .ui_editors
            .get(&control)
            .ok_or_else(|| anyhow::anyhow!("PASTE_UNAVAILABLE: Editor is not mounted"))?;
        let id = input
            .component_id()
            .ok_or_else(|| anyhow::anyhow!("PASTE_UNAVAILABLE: Editor is not mounted"))?;
        let route_id = route_id.into();
        inner.running.host_intercept_paste(
            ComponentHandle::<MountedTextInput>::from_raw_id(id),
            move |text| RoutedOutput {
                route_id: route_id.clone(),
                payload: Some(text),
            },
        );
        Ok(())
    }

    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner.running.host_set_theme(theme);
        drop(inner);
        render_host_after_mutation(&self.inner)
    }

    pub fn dispatch_key(&self, key: KeyStroke) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner.ensure_ui_event_capacity()?;
        inner.apply_key_input(key)?;
        drop(inner);
        render_host_after_mutation(&self.inner)
    }

    pub fn dispatch_paste(&self, text: &str) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner.ensure_ui_event_capacity()?;
        inner.apply_paste_input(text)?;
        drop(inner);
        render_host_after_mutation(&self.inner)
    }

    pub fn forward_paste(&self, text: &str) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner
            .running
            .host_forward_paste(text.to_owned())
            .map_err(|error| anyhow::anyhow!("paste forward failed: {error:?}"))?;
        drop(inner);
        render_host_after_mutation(&self.inner)
    }

    pub fn resize(&self, width: u16, height: u16) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!("terminal size must be positive"));
        }
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        if let Some(HostBackend::Headless(sink)) = inner.backend.as_mut() {
            sink.width = width;
            sink.height = height;
        }
        inner.running.invalidate_frame();
        inner.sync_real_time();
        drop(inner);
        render_host_after_mutation(&self.inner)
    }

    pub fn advance_time(&self, duration: Duration) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.ensure_open()?;
        inner.now += duration;
        drop(inner);
        render_host_after_mutation(&self.inner)
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
        self.lock().map_or(true, |inner| {
            inner.is_closed() || inner.running.host_exited()
        })
    }

    pub fn poll_terminal(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.sync_real_time();
        for _ in 0..INPUT_PUMP_BUDGET {
            if inner.running.has_pending_outputs() {
                break;
            }
            inner.ensure_ui_event_capacity()?;
            let event = match inner.deferred_terminal_input.take() {
                Some(event) => Some(event),
                None => match inner.backend.as_mut() {
                    None => None,
                    Some(HostBackend::Headless(_)) => None,
                    Some(HostBackend::Real(backend)) => backend.try_next_event()?,
                },
            };
            let Some(event) = event else {
                break;
            };
            let result = match &event {
                TerminalEvent::Key(key) => inner.apply_key_input(*key),
                TerminalEvent::Paste(text) => inner.apply_paste_input(text),
                TerminalEvent::Resize => {
                    inner.running.invalidate_frame();
                    Ok(())
                }
            };
            if let Err(error) = result {
                if is_event_backpressure(&error) {
                    inner.deferred_terminal_input = Some(event);
                }
                return Err(error);
            }
            // Do not consume input after a routed output. The caller must
            // reduce that output before later keystrokes can change focus or
            // clear the composer.
            if inner.running.has_pending_outputs() {
                break;
            }
        }
        drop(inner);
        render_host_after_mutation(&self.inner)
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
            .map(|inner| {
                if let Some(HostBackend::Headless(sink)) = inner.backend.as_ref() {
                    return sink.history.iter().map(PhysicalRow::plain_text).collect();
                }
                inner
                    .headless_history
                    .iter()
                    .map(PhysicalRow::plain_text)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn close(&self) -> Result<()> {
        let result = close_host_inner_with_position(&self.inner, FinalPosition::Restore);
        if result.is_err() {
            // A poisoned HostInner cannot safely be recovered by teardown;
            // still remove its environment queue identity. The backend
            // remains owned by the poisoned inner and its Drop path performs
            // best-effort restoration when the last owner leaves.
            self.lifetime
                .environment
                .unregister_host(self.lifetime.host_id);
        }
        result
    }

    #[cfg(test)]
    pub fn fail_next_frame_for_test(&self, diagnostic: impl Into<String>) -> Result<()> {
        self.lock_mut()
            .map(|mut inner| inner.fail_next_frame = Some(diagnostic.into()))
    }

    #[cfg(test)]
    pub(crate) fn mark_backend_stopped_for_test(&self) -> Result<()> {
        let mut inner = self.lock_mut()?;
        inner.mark_faulted();
        Ok(())
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

fn close_host_inner(host: &Arc<Mutex<HostInner>>) -> Result<()> {
    close_host_inner_with_position(host, FinalPosition::Restore)
}

/// Completes a mutation without keeping HostInner across physical History
/// submission. The first pass performs logical preparation; the environment
/// then starts/polls the worker-owned physical receipt outside the acceptance
/// guard and services the same host until its bounded work is settled.
fn render_host_after_mutation(host: &Arc<Mutex<HostInner>>) -> Result<()> {
    let (result, environment, host_id, history_signal) = {
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        let result = inner.advance_and_render();
        (
            result,
            inner.environment.clone(),
            inner.host_id,
            inner.history_work_signal(),
        )
    };
    history_signal.notify()?;
    result?;
    let mut report = environment.drain_pending_for(32, true, Some(host_id))?;
    // A headless backend completes its worker-compatible receipt
    // synchronously. Give the same queue turn that a worker wake would have
    // provided so synchronous compatibility callers still observe a settled
    // History prefix without ever waiting under HostInner.
    if report.waiting_for_presentation {
        let follow_up = environment.drain_pending_for(32, true, Some(host_id))?;
        report.errors.extend(follow_up.errors);
    }
    if let Some(error) = report.errors.first() {
        return Err(anyhow::anyhow!("{}: {}", error.code, error.diagnostic));
    }
    Ok(())
}

fn close_host_inner_with_position(
    host: &Arc<Mutex<HostInner>>,
    final_position: FinalPosition,
) -> Result<()> {
    let (operation, owner) = {
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        match &inner.lifecycle {
            HostLifecycle::Open | HostLifecycle::Faulted => {
                let operation = CloseOperation::new();
                inner.lifecycle = HostLifecycle::Closing(Arc::clone(&operation));
                inner.presentation_notify.notify_waiters();
                #[cfg(test)]
                if let Some(hook) = inner.close_started_hook.as_ref() {
                    let _ = hook.send(ClosePhase::Started);
                }
                (operation, true)
            }
            HostLifecycle::Closing(operation) | HostLifecycle::Closed(operation) => {
                #[cfg(test)]
                if let Some(hook) = inner.close_started_hook.as_ref() {
                    let _ = hook.send(ClosePhase::Joined);
                }
                (Arc::clone(operation), false)
            }
        }
    };
    if !owner {
        return operation.wait();
    }

    let history_error = settle_history_work_for_close(host).err();
    let result = prepare_close(host, &operation, final_position).and_then(|mut plan| {
        plan.history_error = history_error;
        settle_close(host, plan)
    });
    if let Err(error) = operation.complete(&result)
        && result.is_ok()
    {
        return Err(error);
    }
    result
}

/// Joins the one owned physical History operation before close detaches the
/// backend. Submission itself is never waited while HostInner is held; a
/// close caller waits on the worker receipt outside the acceptance lock, then
/// applies the captured acknowledgement under that lock.
fn settle_history_work_for_close(host: &Arc<Mutex<HostInner>>) -> Result<()> {
    loop {
        // Register the wait before reading the predicate. A submitter only
        // publishes a transition after dropping HostInner, so this lock order
        // cannot deadlock with the acceptance mutex.
        let signal = {
            let inner = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
            Arc::clone(&inner.history_work_notify)
        };
        let (guard, generation) = signal.register()?;
        let in_flight = {
            let mut inner = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
            match inner.history_work.take() {
                None => return Ok(()),
                Some(HistoryWork::Prepared { plan }) => {
                    inner.history_work = Some(HistoryWork::Prepared { plan });
                    return Ok(());
                }
                Some(HistoryWork::Submitting) => None,
                Some(HistoryWork::InFlight {
                    plan,
                    backend,
                    receipt,
                }) => Some((plan, backend, receipt)),
            }
        };
        let Some((plan, backend, receipt)) = in_flight else {
            signal.wait_for_change(guard, generation)?;
            continue;
        };
        drop(guard);
        let receipt_result = receipt.blocking_recv();
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        inner.backend = Some(backend);
        let result = match receipt_result {
            Ok(accepted) => {
                let HostInner {
                    running, content, ..
                } = &mut *inner;
                if let Some(history) = running.scene_history_mut() {
                    crate::history::commit_native_transfer_with_content(
                        history, plan, accepted, content,
                    )
                    .map_err(|error| anyhow::anyhow!("History acknowledgement failed: {error:?}"))
                } else {
                    Err(anyhow::anyhow!("History transfer has no scene History"))
                }
            }
            Err(error) => Err(anyhow::anyhow!(
                "terminal History transfer failed during close: {error}"
            )),
        };
        if result.is_err() {
            inner.fail_history_transfer();
        } else {
            inner.content.end_candidate();
            if matches!(
                result.as_ref(),
                Ok(outcome)
                    if outcome.inserted == 0
                        && matches!(
                            outcome.status,
                            crate::history::NativeTransferStatus::SinkBlocked
                        )
            ) {
                inner.history_sink_blocked = true;
            }
        }
        inner.history_work = None;
        let signal = Arc::clone(&inner.history_work_notify);
        drop(inner);
        signal.notify()?;
        return result.map(|_| ());
    }
}

fn settle_history_plan_with_backend(
    host: &Arc<Mutex<HostInner>>,
    backend: &mut HostBackend,
    plan: crate::history::NativeTransferPlan,
) -> Result<crate::history::NativeTransferOutcome> {
    #[cfg(test)]
    let test_receipt = host
        .lock()
        .map_err(|error| anyhow::anyhow!("host lock is poisoned: {error}"))?
        .test_history_receipt
        .take();
    #[cfg(not(test))]
    let test_receipt = None;
    let receipt = match test_receipt
        .map_or_else(|| backend.begin_history_rows(plan.rows().to_vec()), Ok)
    {
        Ok(receipt) => receipt,
        Err(error) => {
            let mut inner = host
                .lock()
                .map_err(|lock_error| anyhow::anyhow!(
                    "terminal History submission failed during close: {error}; candidate cleanup failed: host lock is poisoned: {lock_error}"
                ))?;
            inner.fail_history_transfer();
            return Err(anyhow::anyhow!(
                "terminal History submission failed during close: {error}"
            ));
        }
    };
    let receipt_result = match receipt.blocking_recv() {
        Ok(result) => result.map_err(|error| {
            anyhow::anyhow!("terminal History transfer failed during close: {error}")
        }),
        Err(_) => Err(anyhow::anyhow!("terminal History reply lost during close")),
    };
    let mut inner = host
        .lock()
        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
    let result = receipt_result.and_then(|accepted| {
        let HostInner {
            running, content, ..
        } = &mut *inner;
        let Some(history) = running.scene_history_mut() else {
            return Err(anyhow::anyhow!("History transfer has no scene History"));
        };
        crate::history::commit_native_transfer_with_content(history, plan, accepted, content)
            .map_err(|error| anyhow::anyhow!("History acknowledgement failed: {error:?}"))
    });
    if result.is_err() {
        inner.fail_history_transfer();
    } else {
        inner.content.end_candidate();
    }
    result
}

fn prepare_close(
    host: &Arc<Mutex<HostInner>>,
    operation: &Arc<CloseOperation>,
    final_position: FinalPosition,
) -> Result<ClosePlan> {
    let (pending_presentation, prepare_error, environment, host_id, backend) = {
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        let mut prepare_error = None;
        let pending_presentation = if let Some(receipt) = inner.bootstrap_receipt.take() {
            Some(PendingPresentation::Bootstrap(receipt))
        } else {
            match std::mem::replace(&mut inner.presentation_state, PresentationState::Idle) {
                PresentationState::InFlight { frame, receipt } => {
                    Some(PendingPresentation::Frame {
                        frame: Box::new(frame),
                        receipt,
                    })
                }
                PresentationState::Prepared(frame) => {
                    if frame.content().is_some() {
                        inner.content.abort_candidate();
                        inner.running.host_abort_content_candidate();
                    }
                    None
                }
                PresentationState::Completing { frame }
                    if matches!(final_position, FinalPosition::PositionAfterFinalFrame) =>
                {
                    inner.presentation_state = PresentationState::Completing { frame };
                    if let Err(error) = inner.commit_frame() {
                        prepare_error = Some(error);
                    }
                    None
                }
                PresentationState::Completing { frame } => {
                    if frame.content().is_some() {
                        inner.content.abort_candidate();
                        inner.running.host_abort_content_candidate();
                    }
                    None
                }
                PresentationState::Idle
                | PresentationState::Failed(_)
                | PresentationState::Closed => None,
            }
        };
        let environment = inner.environment.clone();
        let host_id = inner.host_id;
        if matches!(final_position, FinalPosition::PositionAfterFinalFrame) {
            inner.running.host_exit();
        }
        if let Some(HistoryWork::Prepared { .. }) = inner.history_work.take() {
            // The captured rows were never submitted. Close owns no
            // physical acknowledgement to settle in this branch.
            inner.content.abort_candidate();
            inner.running.host_abort_content_candidate();
        }
        let backend = inner
            .backend
            .take()
            .ok_or_else(|| anyhow::anyhow!("open host has no backend ownership"))?;
        (
            pending_presentation,
            prepare_error,
            environment,
            host_id,
            backend,
        )
    };

    Ok(ClosePlan {
        operation: Arc::clone(operation),
        pending_presentation,
        prepare_error,
        history_error: None,
        environment,
        host_id,
        backend,
        final_position,
    })
}

fn settle_close(host: &Arc<Mutex<HostInner>>, plan: ClosePlan) -> Result<()> {
    let mut plan = plan;
    plan.environment.cancel_host_scheduling(plan.host_id);
    let pending_presentation = plan.pending_presentation.take();
    let receipt_settlement = match settle_receipt(
        host,
        pending_presentation,
        matches!(plan.final_position, FinalPosition::PositionAfterFinalFrame),
    ) {
        Ok(settlement) => settlement,
        Err(error) => ReceiptSettlement {
            result: Err(error),
            continue_to_final: false,
        },
    };
    let mut final_frame = None;
    let mut final_prepare_error = None;
    if plan.prepare_error.is_none()
        && receipt_settlement.continue_to_final
        && matches!(plan.final_position, FinalPosition::PositionAfterFinalFrame)
    {
        let mut advanced = false;
        loop {
            let candidate = match host.lock() {
                Ok(mut inner) => {
                    let candidate = if advanced {
                        inner.prepare_candidate_frame_with_backend(&mut plan.backend)
                    } else {
                        inner.prepare_final_candidate(&mut plan.backend)
                    };
                    let (candidate, history_plan) = match candidate {
                        Ok((frame, history_plan)) => (Ok(frame), history_plan),
                        Err(error) => (Err(error), None),
                    };
                    if history_plan.is_some()
                        && let Ok(frame) = candidate.as_ref()
                    {
                        if let Some(content) = frame.content() {
                            inner.content.begin_prepared_candidate(content);
                        }
                        inner.discard_prepared_candidate(frame, true);
                    }
                    (candidate, history_plan)
                }
                Err(error) => (Err(anyhow::anyhow!("host lock is poisoned: {error}")), None),
            };
            match candidate {
                (Ok(_frame), Some(history_plan)) => {
                    match settle_history_plan_with_backend(host, &mut plan.backend, history_plan) {
                        Ok(outcome)
                            if outcome.inserted == 0
                                && matches!(
                                    outcome.status,
                                    crate::history::NativeTransferStatus::SinkBlocked
                                ) =>
                        {
                            // The sink acknowledged no rows and the native
                            // frontier therefore remains unchanged. Retain
                            // the semantic prefix and let the next
                            // candidate use the existing blocked-history
                            // front-pinning path instead of resubmitting the
                            // same captured rows.
                            match host.lock() {
                                Ok(mut inner) => inner.history_sink_blocked = true,
                                Err(error) => {
                                    final_prepare_error =
                                        Some(anyhow::anyhow!("host lock is poisoned: {error}"));
                                    break;
                                }
                            }
                            advanced = true;
                        }
                        Ok(_) => {
                            advanced = true;
                        }
                        Err(error) => {
                            final_prepare_error = Some(error);
                            break;
                        }
                    }
                }
                (Ok(frame), None) => {
                    match host.lock() {
                        Ok(mut inner) => {
                            inner.content.begin_prepared_candidate(
                                frame.content().expect("final scene has content plan"),
                            );
                        }
                        Err(error) => {
                            final_prepare_error =
                                Some(anyhow::anyhow!("host lock is poisoned: {error}"));
                            break;
                        }
                    }
                    final_frame = Some(frame);
                    break;
                }
                (Err(error), _) => {
                    final_prepare_error = Some(error);
                    break;
                }
            }
        }
    }
    let physical_failure = receipt_settlement.result.is_err() || plan.history_error.is_some();
    let (restore_result, headless_history) =
        settle_backend(host, plan.backend, plan.final_position, final_frame);
    let cleanup_result = finalize_close(host, &plan.operation, physical_failure, headless_history);
    // Unregister at lifecycle finalization rather than waiting for the last
    // transient `HostInner` Arc. The latter may be retained by History or a
    // receipt-owned handle, but it must not leave a runnable environment
    // identity behind after the public host has closed.
    plan.environment.unregister_host(plan.host_id);
    let mut failures = Vec::new();
    if let Err(error) = receipt_settlement.result {
        failures.push(("presentation receipt", error));
    }
    if let Some(error) = plan.history_error {
        failures.push(("History transfer", error));
    }
    if let Some(error) = plan.prepare_error {
        failures.push(("close preparation", error));
    }
    if let Some(error) = final_prepare_error {
        failures.push(("final exit frame preparation", error));
    }
    if let Err(error) = restore_result {
        failures.push(("terminal restoration", error));
    }
    if let Err(error) = cleanup_result {
        failures.push(("owner cleanup", error));
    }
    combine_close_failures(failures)
}

fn combine_close_failures(failures: Vec<(&'static str, anyhow::Error)>) -> Result<()> {
    match failures.as_slice() {
        [] => Ok(()),
        [(label, error)] => Err(anyhow::anyhow!("{label}: {error}")),
        _ => Err(anyhow::anyhow!(
            "close failures: {}",
            failures
                .into_iter()
                .map(|(label, error)| format!("{label}: {error}"))
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}

fn settle_receipt(
    host: &Arc<Mutex<HostInner>>,
    pending_presentation: Option<PendingPresentation>,
    promote_successful_frame: bool,
) -> Result<ReceiptSettlement> {
    let settlement = match pending_presentation {
        Some(PendingPresentation::Frame { frame, receipt }) => match receipt.blocking_recv() {
            Ok(()) if promote_successful_frame => {
                let mut inner = host
                    .lock()
                    .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
                inner.presentation_state = PresentationState::Completing { frame: *frame };
                match inner.commit_frame() {
                    Ok(_) => ReceiptSettlement {
                        result: Ok(()),
                        continue_to_final: true,
                    },
                    Err(error) => ReceiptSettlement {
                        result: Err(error),
                        continue_to_final: false,
                    },
                }
            }
            Ok(()) => ReceiptSettlement {
                result: Ok(()),
                continue_to_final: false,
            },
            Err(error) => {
                let mut inner = host
                    .lock()
                    .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
                inner.physical_sync_unknown = true;
                ReceiptSettlement {
                    result: Err(error),
                    continue_to_final: false,
                }
            }
        },
        Some(PendingPresentation::Bootstrap(receipt)) => match receipt.blocking_recv() {
            Ok(()) => ReceiptSettlement {
                result: Ok(()),
                continue_to_final: promote_successful_frame,
            },
            Err(error) => ReceiptSettlement {
                result: Err(error),
                continue_to_final: false,
            },
        },
        None => ReceiptSettlement {
            result: Ok(()),
            continue_to_final: true,
        },
    };
    Ok(settlement)
}

fn settle_backend(
    host: &Arc<Mutex<HostInner>>,
    mut backend: HostBackend,
    final_position: FinalPosition,
    final_frame: Option<PreparedFrame>,
) -> (Result<()>, Vec<PhysicalRow>) {
    // Final positioning/restoration is physical I/O and deliberately happens
    // outside the host guard. A second close joins this same plan.
    let frame_result = final_frame.map_or(Ok(()), |frame| {
        settle_final_frame(host, &mut backend, frame)
    });
    let output_result = frame_result.and_then(|()| match final_position {
        FinalPosition::Restore => Ok(()),
        FinalPosition::PositionAfterFinalFrame => match &mut backend {
            HostBackend::Headless(sink) => {
                sink.history.extend(confirmed_final_rows(host)?);
                Ok(())
            }
            HostBackend::Real(backend) => backend.position_after_final_frame(),
        },
    });
    // Restoration is required even when presentation or final positioning
    // failed. Do not defer it to Drop, which cannot report a second failure.
    let restore_result = match &mut backend {
        HostBackend::Real(backend) => ignore_terminal_shutdown_error(backend.restore()),
        HostBackend::Headless(_) => Ok(()),
    };
    let result = match (output_result, restore_result) {
        (Err(output), Err(restore)) => Err(anyhow::anyhow!(
            "terminal output failed: {output}; restoration failed: {restore}"
        )),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    };
    let history = match backend {
        HostBackend::Headless(sink) => sink.history,
        HostBackend::Real(_) => Vec::new(),
    };
    (result, history)
}

fn finalize_close(
    host: &Arc<Mutex<HostInner>>,
    operation: &Arc<CloseOperation>,
    receipt_failed: bool,
    headless_history: Vec<PhysicalRow>,
) -> Result<()> {
    {
        let mut inner = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        inner.running.host_set_body(vf::spacer(0));
        inner.running.host_clear_retained_views();
        if receipt_failed {
            inner.physical_sync_unknown = true;
        }
        let presentation =
            std::mem::replace(&mut inner.presentation_state, PresentationState::Idle);
        if let PresentationState::Prepared(frame) | PresentationState::Completing { frame } =
            presentation
        {
            if frame.content().is_some() {
                inner.content.abort_candidate();
                inner.running.host_abort_content_candidate();
            }
        }
        inner.content.dispose_all();
        inner.headless_history = headless_history;
        let ui_cleanup = inner.ui_resources.close().map_err(anyhow::Error::msg);
        inner.presentation_state = PresentationState::Closed;
        inner.lifecycle = HostLifecycle::Closed(Arc::clone(operation));
        inner.presentation_notify.notify_waiters();
        inner.ui_event_notify.notify_waiters();
        ui_cleanup
    }
}

fn settle_final_frame(
    host: &Arc<Mutex<HostInner>>,
    backend: &mut HostBackend,
    frame: PreparedFrame,
) -> Result<()> {
    #[cfg(test)]
    let test_receipt = host
        .lock()
        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?
        .final_backend_receipt
        .take();
    #[cfg(not(test))]
    let test_receipt = None;
    let receipt_result = if let Some(receipt) = test_receipt {
        Some(blocking_receive(receipt))
    } else if !frame.is_no_output() {
        let scene = frame
            .scene()
            .ok_or_else(|| anyhow::anyhow!("output frame has no captured scene"))?;
        if let HostBackend::Real(backend) = backend {
            let receipt = match backend.begin_frame(scene) {
                Ok(receipt) => receipt,
                Err(error) => {
                    let presentation = host_attempt_error(
                        "backend",
                        "BACKEND_IO_FAILED",
                        false,
                        format!("terminal presentation failed during final exit frame: {error}"),
                    );
                    return match discard_final_candidate(host, &frame) {
                        Ok(()) => Err(presentation),
                        Err(cleanup) => combine_close_failures(vec![
                            ("terminal presentation", presentation),
                            ("final candidate cleanup", cleanup),
                        ]),
                    };
                }
            };
            Some(blocking_receive(receipt))
        } else {
            None
        }
    } else {
        None
    };
    if let Some(Err(error)) = receipt_result {
        let presentation = host_attempt_error(
            "backend",
            "BACKEND_IO_FAILED",
            false,
            format!("terminal presentation failed during final exit frame: {error}"),
        );
        return match discard_final_candidate(host, &frame) {
            Ok(()) => Err(presentation),
            Err(cleanup) => combine_close_failures(vec![
                ("terminal presentation", presentation),
                ("final candidate cleanup", cleanup),
            ]),
        };
    }
    let mut inner = host
        .lock()
        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
    inner.presentation_state = PresentationState::Completing { frame };
    inner.commit_frame().map(|_| ())
}

fn discard_final_candidate(host: &Arc<Mutex<HostInner>>, frame: &PreparedFrame) -> Result<()> {
    let mut inner = host
        .lock()
        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
    inner.physical_sync_unknown = true;
    if frame.content().is_some() {
        inner.content.abort_candidate();
        inner.running.host_abort_content_candidate();
    }
    inner.running.host_discard_candidate();
    Ok(())
}

fn confirmed_final_rows(host: &Arc<Mutex<HostInner>>) -> Result<Vec<PhysicalRow>> {
    let inner = host
        .lock()
        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
    let width = usize::from(inner.frame.surface.width());
    if width == 0 {
        return Ok(Vec::new());
    }
    Ok(inner
        .frame
        .surface
        .cells
        .chunks(width)
        .map(|row| PhysicalRow::from_cells(row.to_vec()))
        .filter(|row| !row.plain_text().is_empty())
        .collect())
}

fn physical_color(color: crate::physical::PhysicalColor) -> String {
    match color {
        crate::physical::PhysicalColor::Default => "default".to_owned(),
        crate::physical::PhysicalColor::Named(color) => format!("{color:?}"),
        crate::physical::PhysicalColor::Indexed(value) => format!("ansi:{value}"),
        crate::physical::PhysicalColor::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

fn is_event_backpressure(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<UiInputAdmissionError>()
        .is_some_and(|error| matches!(error, UiInputAdmissionError::Backpressure(_)))
}

fn ignore_terminal_shutdown_error(result: Result<()>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if crate::terminal::is_terminal_worker_stopped(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

impl Drop for HostLifetime {
    fn drop(&mut self) {
        if close_host_inner(&self.inner).is_err() {
            self.environment.unregister_host(self.host_id);
        }
    }
}

impl HostInner {
    pub(super) fn is_closed(&self) -> bool {
        !matches!(self.lifecycle, HostLifecycle::Open)
    }

    fn ensure_open(&self) -> Result<()> {
        if self.is_closed() {
            return Err(anyhow::anyhow!("HOST_DISPOSED: host is closed"));
        }
        Ok(())
    }

    fn mark_faulted(&mut self) {
        self.lifecycle = HostLifecycle::Faulted;
        self.presentation_notify.notify_waiters();
    }

    fn publish_failure_notification(&mut self, failure: &FrameFailure) {
        if self.failure_notifications.len() >= MAX_FAILURE_NOTIFICATIONS {
            self.failure_notifications.pop_front();
            self.dropped_failure_notifications =
                self.dropped_failure_notifications.saturating_add(1);
        }
        self.failure_notifications
            .push_back(UiFailureNotification::from(failure));
        self.presentation_notify.notify_waiters();
    }

    fn publish_attempt_failure(
        &mut self,
        phase: &'static str,
        code: &'static str,
        retryable: bool,
        attempted_ui_revision: u64,
        attempted_work_epoch: u64,
        diagnostic: String,
    ) {
        self.publish_failure_notification(&FrameFailure {
            phase,
            code,
            attempted_ui_revision,
            attempted_work_epoch,
            retryable,
            diagnostic,
        });
    }

    #[cfg(test)]
    fn install_test_final_receipt(
        &mut self,
        receipt: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
    ) {
        self.final_backend_receipt = Some(receipt);
    }

    #[cfg(test)]
    fn install_test_history_receipt(&mut self, receipt: crate::terminal::HistoryReceipt) {
        self.test_history_receipt = Some(receipt);
    }

    pub(super) fn service_native_deadline_inner(&mut self) -> Result<u64> {
        if self.headless || self.is_closed() {
            return Ok(u64::MAX);
        }
        self.sync_real_time();
        let deadline = [self.running.next_deadline(), self.content.next_wakeup()]
            .into_iter()
            .flatten()
            .min();
        let Some(deadline) = deadline else {
            return Ok(u64::MAX);
        };
        let wait_ms = deadline
            .saturating_duration_since(self.now)
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
            .max(1);
        if deadline <= self.now {
            self.advance_and_render()?;
            return Ok(1);
        }
        Ok(wait_ms)
    }

    pub(super) fn flush_for_environment(
        &mut self,
        wait_for_presentation: bool,
        force_retry: bool,
    ) -> Result<(HostFlushOutcome, u64, u64)> {
        if force_retry {
            self.history_sink_blocked = false;
        }
        let mut outcome = self.flush_pending_frame()?;
        #[cfg(test)]
        if outcome.waiting_for_presentation
            && let Some((entered, release)) = self.waiting_for_presentation_hook.take()
        {
            let _ = entered.send(());
            let _ = release.recv();
        }
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

    /// Returns the completion signal while the caller still owns the host
    /// guard. The caller must drop that guard before invoking notify; this
    /// preserves the signal-before-host lock order used by close.
    pub(super) fn history_work_signal(&self) -> Arc<HistoryWorkSignal> {
        Arc::clone(&self.history_work_notify)
    }

    pub(super) fn presentation_notification(&self) -> std::sync::Arc<tokio::sync::Notify> {
        Arc::clone(&self.presentation_notify)
    }

    pub(super) fn environment_error_epochs(&self) -> (u64, u64, u64) {
        let (attempted_epoch, desired_revision) = self.failed_attempt.map_or(
            (self.pending_epoch, self.desired_structural_revision),
            |attempt| (attempt.work_epoch, attempt.desired_revision),
        );
        (attempted_epoch, desired_revision, self.pending_epoch)
    }

    #[cfg(test)]
    pub(crate) fn ui_history_len(&self) -> usize {
        self.running.scene_history().map_or(0, crate::History::len)
    }

    fn candidate_content_commit(&mut self) -> Result<PreparedContentCommit> {
        // H3 already validated the complete attachment list before desired
        // acceptance. The content commit plan needs only the changed-record
        // set populated by that acceptance and the current candidate measure;
        // unchanged visible bindings are not copied into another table.
        self.content.prepare_content_commit()
    }

    #[cfg(test)]
    pub(crate) fn install_test_in_flight(
        &mut self,
        scene: PreparedSceneFrame,
        receipt: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
    ) -> Result<()> {
        let content = self.candidate_content_commit()?;
        let frame = PreparedFrame::with_scene(
            PreparedSceneProducts {
                scene,
                content,
                content_dirty_epoch: self.running.host_content_candidate_epoch(),
            },
            SceneDisposition::Submit,
            self.visible_frame_revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("visible frame revision exhausted"))?,
            self.ui_resources.document.as_ref().map_or(
                0,
                crate::occurrence::OccurrenceDocument::accepted_ui_revision,
            ),
            self.pending_epoch,
            self.desired_structural_revision,
        );
        self.content
            .begin_prepared_candidate(frame.content().expect("scene has content plan"));
        self.presentation_state = PresentationState::InFlight {
            frame,
            receipt: PresentReceipt::from_receiver(
                receipt,
                self.environment.receipt_wake(self.host_id),
            ),
        };
        self.bootstrap_pending = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn prepare_test_candidate(&mut self) -> Result<PreparedSceneFrame> {
        self.content.begin_projection_candidate();
        prepare_frame_with_content(
            &mut self.running,
            self.backend
                .as_mut()
                .expect("open host must own its terminal backend"),
            self.now,
            &mut self.content,
        )
    }

    #[cfg(test)]
    fn install_test_bootstrap_receipt(
        &mut self,
        receipt: tokio::sync::oneshot::Receiver<anyhow::Result<()>>,
    ) {
        self.bootstrap_pending = false;
        self.bootstrap_receipt = Some(PresentReceipt::from_receiver(
            receipt,
            self.environment.receipt_wake(self.host_id),
        ));
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
        let wake = self.admit_pending()?;
        self.presentation_notify.notify_waiters();
        Ok(wake)
    }

    fn admit_pending(&mut self) -> anyhow::Result<WakeDisposition> {
        match self.environment.mark_host_pending(self.host_id) {
            Ok(wake) => {
                self.scheduler_failure = None;
                Ok(wake)
            }
            Err(error) => {
                let failure = FrameFailure {
                    phase: "scheduler",
                    code: "ENVIRONMENT_WAKE_FAILED",
                    attempted_ui_revision: self.ui_resources.document.as_ref().map_or(
                        0,
                        crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                    ),
                    attempted_work_epoch: self.pending_epoch,
                    retryable: true,
                    diagnostic: error.to_string(),
                };
                self.scheduler_failure = Some(failure.clone());
                self.publish_failure_notification(&failure);
                Err(error)
            }
        }
    }

    pub(super) fn environment_wake_epoch(&self) -> u64 {
        self.environment.wake_epoch()
    }

    fn ensure_pending(&mut self) -> anyhow::Result<()> {
        if self.pending_epoch == self.committed_epoch {
            let _ = self.mark_pending()?;
        } else {
            let _ = self.admit_pending()?;
        }
        Ok(())
    }

    fn queue_ui_changes(&mut self, changes: crate::occurrence::UiChangeSet) {
        if changes.is_empty() {
            return;
        }
        if let Some(pending) = self.pending_ui_changes.as_mut() {
            pending.merge(changes);
        } else {
            self.pending_ui_changes = Some(changes);
        }
    }

    fn reserve_ui_change_capacity(
        &mut self,
        changes: &crate::occurrence::UiChangeSet,
    ) -> Result<Option<crate::occurrence::UiChangeSet>> {
        let Some(pending) = self.pending_ui_changes.as_mut() else {
            let mut provisional = crate::occurrence::UiChangeSet::default();
            provisional
                .changed_nodes
                .try_reserve(changes.changed_nodes.len())
                .map_err(|_| anyhow::anyhow!("pending occurrence node frontier capacity"))?;
            provisional
                .retired_nodes
                .try_reserve(changes.retired_nodes.len())
                .map_err(|_| anyhow::anyhow!("pending occurrence retirement capacity"))?;
            provisional
                .history_roots
                .try_reserve(changes.history_roots.len())
                .map_err(|_| anyhow::anyhow!("pending History-root frontier capacity"))?;
            provisional
                .changed_resources
                .try_reserve(changes.changed_resources.len())
                .map_err(|_| anyhow::anyhow!("pending UI resource frontier capacity"))?;
            provisional
                .resource_ports
                .try_reserve(changes.resource_ports.len())
                .map_err(|_| anyhow::anyhow!("pending resource association capacity"))?;
            provisional
                .membership_nodes
                .try_reserve(changes.membership_nodes.len())
                .map_err(|_| anyhow::anyhow!("pending membership frontier capacity"))?;
            return Ok(Some(provisional));
        };
        pending
            .changed_nodes
            .try_reserve(changes.changed_nodes.len())
            .map_err(|_| anyhow::anyhow!("pending occurrence node frontier capacity"))?;
        pending
            .retired_nodes
            .try_reserve(changes.retired_nodes.len())
            .map_err(|_| anyhow::anyhow!("pending occurrence retirement capacity"))?;
        pending
            .history_roots
            .try_reserve(changes.history_roots.len())
            .map_err(|_| anyhow::anyhow!("pending History-root frontier capacity"))?;
        pending
            .changed_resources
            .try_reserve(changes.changed_resources.len())
            .map_err(|_| anyhow::anyhow!("pending UI resource frontier capacity"))?;
        pending
            .resource_ports
            .try_reserve(changes.resource_ports.len())
            .map_err(|_| anyhow::anyhow!("pending resource association capacity"))?;
        pending
            .membership_nodes
            .try_reserve(changes.membership_nodes.len())
            .map_err(|_| anyhow::anyhow!("pending membership frontier capacity"))?;
        Ok(None)
    }

    fn prepare_candidate_frame(
        &mut self,
    ) -> Result<(PreparedFrame, Option<crate::history::NativeTransferPlan>)> {
        let mut backend = self
            .backend
            .take()
            .expect("open host must own its terminal backend");
        let result = self.prepare_candidate_frame_with_backend(&mut backend);
        self.backend = Some(backend);
        result
    }

    fn prepare_metadata_candidate(&mut self) -> PreparedFrame {
        let changes = self
            .pending_ui_changes
            .as_ref()
            .expect("metadata candidate admission has an accepted UI frontier");
        debug_assert!(!changes.physical_work);
        debug_assert!(!self.physical_sync_unknown);
        debug_assert!(!self.content_dirty);
        debug_assert!(!self.content.has_pending_source_cleanup());
        debug_assert!(!self.running.is_dirty());
        let ui_revision = self
            .ui_resources
            .document
            .as_ref()
            .expect("open UI resource owner retains document")
            .accepted_ui_revision();
        debug_assert_eq!(ui_revision, changes.ui_revision);
        PreparedFrame::metadata(
            self.visible_frame_revision,
            ui_revision,
            self.pending_epoch,
            ui_revision,
        )
    }

    fn advance_runtime_for_candidate(&mut self, admit_wakes: bool) -> Result<bool> {
        self.sync_pending_ui_scene_before_tick()?;
        let content_dirty = self.content.advance(self.now).map_err(|error| {
            host_attempt_error(
                "content",
                "CONTENT_SCHEDULER_FAILED",
                true,
                format!("content delivery advance failed: {error}"),
            )
        })?;
        for dirty in content_dirty {
            if admit_wakes {
                self.mark_content_pending(dirty)?;
            } else {
                self.running.host_invalidate_content(dirty);
                self.content_dirty = true;
            }
        }
        let status = self.running.advance_ready(self.now).map_err(|error| {
            host_attempt_error(
                "frame",
                "FRAME_PREPARATION_FAILED",
                true,
                format!("host update failed: {error:?}"),
            )
        })?;
        let dirty = status.dirty;
        for component_id in status.changed_components {
            let Some(key) = self.legacy_scene.control_for_component(component_id) else {
                continue;
            };
            let Some(slot) = self.ui_animations.get(&key) else {
                continue;
            };
            let frame = slot.frame_index()?;
            self.ui_resources.set_native_animation_frame(key, frame)?;
            self.sync_native_animation_frame(key)?;
        }
        if admit_wakes && dirty {
            self.ensure_pending()?;
        }
        Ok(dirty)
    }

    fn sync_pending_ui_scene_before_tick(&mut self) -> Result<()> {
        let revision = self.ui_resources.document.as_ref().map_or(
            0,
            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
        );
        if revision == 0 || revision == self.ui_scene_revision {
            return Ok(());
        }
        let should_sync = self
            .pending_ui_changes
            .as_ref()
            .is_none_or(|changes| changes.physical_work);
        if !should_sync {
            return Ok(());
        }
        if let Err(error) = self.sync_ui_scene() {
            let ui_revision = self.ui_resources.document.as_ref().map_or(
                0,
                crate::occurrence::OccurrenceDocument::accepted_ui_revision,
            );
            self.record_failed_frame(&error, "frame", ui_revision, self.pending_epoch);
            return Err(error);
        }
        Ok(())
    }

    fn sync_native_animation_frame(&mut self, key: crate::occurrence::ResourceKey) -> Result<()> {
        let document = self
            .ui_resources
            .document
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("UI resource owner has no occurrence document"))?;
        let node = document
            .control_owner(key)
            .ok_or_else(|| anyhow::anyhow!("native animation control has no owner node"))?;
        let changes = crate::occurrence::UiChangeSet {
            changed_nodes: vec![node],
            membership_nodes: vec![node],
            physical_work: true,
            ..crate::occurrence::UiChangeSet::default()
        };
        let changed_content_ports = self
            .legacy_scene
            .synchronize(&self.ui_resources, &mut self.content, Some(&changes))
            .map_err(|error| {
                host_attempt_error(
                    "frame",
                    "FRAME_PREPARATION_FAILED",
                    true,
                    format!("occurrence animation synchronization failed: {error}"),
                )
            })?;
        for port_id in changed_content_ports {
            self.running
                .host_invalidate_content(crate::presentation::ContentDirty::new(
                    port_id,
                    None,
                    crate::presentation::ContentDirtyReason::SelectionLifecycle,
                ));
        }
        let body = self.legacy_scene.body(&self.ui_resources)?;
        self.running.host_set_body(body);
        Ok(())
    }

    fn prepare_final_candidate(
        &mut self,
        backend: &mut HostBackend,
    ) -> Result<(PreparedFrame, Option<crate::history::NativeTransferPlan>)> {
        if self.pending_epoch == self.committed_epoch {
            self.pending_epoch = self
                .pending_epoch
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("host pending epoch exhausted"))?;
        }
        self.scheduler_failure = None;
        self.failed_attempt = None;
        self.attempt_revision = self
            .attempt_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("host attempt revision exhausted"))?;

        self.advance_runtime_for_candidate(false)?;
        self.prepare_candidate_frame_with_backend(backend)
    }

    /// Starts the one captured History write after the caller has released
    /// HostInner. The worker command is asynchronous; no terminal wait or
    /// backend mutex is held by the acceptance/scheduler lock.
    pub(super) fn start_history_work(host: &Arc<Mutex<HostInner>>) -> Result<(u64, u64)> {
        let (plan, mut backend, host_id, environment, wake) = {
            let mut inner = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
            let plan = match inner.history_work.take() {
                Some(HistoryWork::Prepared { plan })
                    if matches!(
                        inner.lifecycle,
                        HostLifecycle::Open | HostLifecycle::Faulted
                    ) =>
                {
                    plan
                }
                Some(work) => {
                    inner.history_work = Some(work);
                    return Ok((inner.pending_epoch, inner.committed_epoch));
                }
                None => return Ok((inner.pending_epoch, inner.committed_epoch)),
            };
            let Some(backend) = inner.backend.take() else {
                inner.history_work = Some(HistoryWork::Prepared { plan });
                return Err(anyhow::anyhow!("open host has no backend ownership"));
            };
            let host_id = inner.host_id;
            let environment = inner.environment.clone();
            let wake = Arc::clone(&inner.history_work_notify);
            inner.history_work = Some(HistoryWork::Submitting);
            (plan, backend, host_id, environment, wake)
        };

        let rows = plan.rows().to_vec();
        #[cfg(test)]
        let test_receipt = {
            let mut inner = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
            inner.test_history_receipt.take()
        };
        #[cfg(not(test))]
        let test_receipt = None;
        let receipt_result = test_receipt.map_or_else(|| backend.begin_history_rows(rows), Ok);
        let mut inner = match host.lock() {
            Ok(inner) => inner,
            Err(error) => {
                wake.notify()?;
                return Err(anyhow::anyhow!("host lock is poisoned: {error}"));
            }
        };
        match receipt_result {
            Ok(receipt) => {
                inner.history_work = Some(HistoryWork::InFlight {
                    plan,
                    backend,
                    receipt: HistoryReceipt::from_receiver(
                        receipt,
                        environment.receipt_wake(host_id),
                    ),
                });
            }
            Err(error) => {
                inner.backend = Some(backend);
                inner.fail_history_transfer();
                inner.history_work = None;
                let diagnostic = format!("terminal History submission failed: {error}");
                let ui_revision = inner.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                let work_epoch = inner.pending_epoch;
                inner.publish_attempt_failure(
                    "backend",
                    "HISTORY_TRANSFER_FAILED",
                    true,
                    ui_revision,
                    work_epoch,
                    diagnostic.clone(),
                );
                drop(inner);
                wake.notify()?;
                return Err(host_attempt_error(
                    "backend",
                    "HISTORY_TRANSFER_FAILED",
                    true,
                    diagnostic,
                ));
            }
        }
        inner.presentation_notify.notify_waiters();
        let epochs = (inner.pending_epoch, inner.committed_epoch);
        drop(inner);
        wake.notify()?;
        Ok(epochs)
    }

    fn poll_history_work(&mut self) -> Result<HistoryWorkPoll> {
        let work = match self.history_work.take() {
            Some(work @ HistoryWork::Prepared { .. }) => {
                self.history_work = Some(work);
                return Ok(HistoryWorkPoll::Pending);
            }
            Some(work @ HistoryWork::Submitting) => {
                self.history_work = Some(work);
                return Ok(HistoryWorkPoll::Pending);
            }
            Some(HistoryWork::InFlight {
                plan,
                backend,
                receipt,
            }) => (plan, backend, receipt),
            None => return Ok(HistoryWorkPoll::None),
        };
        let (plan, backend, mut receipt) = work;
        match receipt.poll() {
            std::task::Poll::Pending => {
                self.history_work = Some(HistoryWork::InFlight {
                    plan,
                    backend,
                    receipt,
                });
                Ok(HistoryWorkPoll::Pending)
            }
            std::task::Poll::Ready(Ok(accepted)) => {
                self.backend = Some(backend);
                let result = self
                    .running
                    .scene_history_mut()
                    .ok_or_else(|| anyhow::anyhow!("History transfer has no scene History"))
                    .and_then(|history| {
                        crate::history::commit_native_transfer_with_content(
                            history,
                            plan,
                            accepted,
                            &mut self.content,
                        )
                        .map_err(|error| match error {
                            crate::history::NativeTransferError::InvalidAcknowledgement {
                                requested,
                                accepted,
                            } => anyhow::anyhow!(
                                "invalid History acknowledgement: accepted {accepted} of {requested}"
                            ),
                            crate::history::NativeTransferError::SynchronizationUnknown => {
                                anyhow::anyhow!("History synchronization is already unknown")
                            }
                            crate::history::NativeTransferError::Sink(error) => error,
                        })
                    });
                match result {
                    Ok(outcome)
                        if outcome.inserted == 0
                            && matches!(
                                outcome.status,
                                crate::history::NativeTransferStatus::SinkBlocked
                            ) =>
                    {
                        self.content.end_candidate();
                        self.history_sink_blocked = true;
                        Ok(HistoryWorkPoll::Blocked)
                    }
                    Ok(_) => {
                        self.content.end_candidate();
                        self.history_sink_blocked = false;
                        Ok(HistoryWorkPoll::Progress)
                    }
                    Err(error) => {
                        self.fail_history_transfer();
                        let diagnostic = error.to_string();
                        let ui_revision = self.ui_resources.document.as_ref().map_or(
                            0,
                            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                        );
                        self.publish_attempt_failure(
                            "backend",
                            "HISTORY_TRANSFER_FAILED",
                            true,
                            ui_revision,
                            self.pending_epoch,
                            diagnostic.clone(),
                        );
                        Err(host_attempt_error(
                            "backend",
                            "HISTORY_TRANSFER_FAILED",
                            true,
                            diagnostic,
                        ))
                    }
                }
            }
            std::task::Poll::Ready(Err(error)) => {
                self.backend = Some(backend);
                self.fail_history_transfer();
                let diagnostic = format!("terminal History transfer failed: {error}");
                let ui_revision = self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                self.publish_attempt_failure(
                    "backend",
                    "HISTORY_TRANSFER_FAILED",
                    true,
                    ui_revision,
                    self.pending_epoch,
                    diagnostic.clone(),
                );
                Err(host_attempt_error(
                    "backend",
                    "HISTORY_TRANSFER_FAILED",
                    true,
                    diagnostic,
                ))
            }
        }
    }

    fn prepare_candidate_frame_with_backend(
        &mut self,
        backend: &mut HostBackend,
    ) -> Result<(PreparedFrame, Option<crate::history::NativeTransferPlan>)> {
        let target_epoch = self.pending_epoch;
        if let Err(error) = self.sync_ui_scene() {
            let ui_revision = self.ui_resources.document.as_ref().map_or(
                0,
                crate::occurrence::OccurrenceDocument::accepted_ui_revision,
            );
            self.record_failed_frame(&error, "frame", ui_revision, target_epoch);
            return Err(error);
        }
        let target_structural_revision = self.desired_structural_revision;
        self.content.begin_projection_candidate();
        let size = match backend {
            HostBackend::Headless(sink) => Size::new(sink.width, sink.height),
            HostBackend::Real(backend) => backend.viewport()?,
        };
        let (mut candidate, history_plan) =
            match self
                .running
                .prepare_frame_for_history(self.now, size, &mut self.content)
            {
                Ok(candidate) => candidate,
                Err(error) => {
                    // SceneHost may have staged derived layout/surface state before
                    // a late preparation error. Keep the HostInner frame as the
                    // sole visible authority and rebuild the candidate on retry.
                    self.note_physical_sync_failure(&error);
                    let ui_revision = self.ui_resources.document.as_ref().map_or(
                        0,
                        crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                    );
                    self.record_failed_frame(&error, "frame", ui_revision, target_epoch);
                    self.content.abort_candidate();
                    self.running.host_discard_candidate();
                    return Err(error);
                }
            };
        let history_plan = (!self.history_sink_blocked)
            .then_some(history_plan)
            .flatten();
        candidate.occurrence_geometry = self
            .legacy_scene
            .occurrence_geometry(&candidate.view_geometry);
        let content_failure = match self.content.ui_content_failure() {
            Ok(failure) => failure,
            Err(error) => {
                let failure =
                    host_attempt_error("content", "INTERNAL_INVARIANT", false, error.to_string());
                let ui_revision = self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                self.record_failed_frame(&failure, "content", ui_revision, target_epoch);
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                return Err(failure);
            }
        };
        if let Some(diagnostic) = content_failure {
            let error = host_attempt_error("content", "PROJECTION_FAILED", true, diagnostic);
            self.record_failed_frame(
                &error,
                "content",
                self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                ),
                target_epoch,
            );
            self.content.abort_candidate();
            self.running.host_discard_candidate();
            return Err(error);
        }
        let content_commit = match self.candidate_content_commit() {
            Ok(commit) => commit,
            Err(error) => {
                let ui_revision = self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                self.record_failed_frame(&error, "frame", ui_revision, target_epoch);
                self.content.abort_candidate();
                self.running.host_discard_candidate();
                self.running.host_abort_content_candidate();
                return Err(error);
            }
        };
        let no_output = candidate.surface == self.frame.surface && !self.physical_sync_unknown;
        let physical_frame_id = if no_output {
            self.visible_frame_revision
        } else {
            self.visible_frame_revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("visible frame revision exhausted"))?
        };
        Ok((
            PreparedFrame::with_scene(
                PreparedSceneProducts {
                    scene: candidate,
                    content: content_commit,
                    content_dirty_epoch: self.running.host_content_candidate_epoch(),
                },
                if no_output {
                    SceneDisposition::NoOutput
                } else {
                    SceneDisposition::Submit
                },
                physical_frame_id,
                self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                ),
                target_epoch,
                target_structural_revision,
            ),
            history_plan,
        ))
    }

    fn render(&mut self) -> Result<HostFlushOutcome> {
        if self.presentation_state.is_in_flight() || self.bootstrap_receipt.is_some() {
            return Ok(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            });
        }
        let (prepared, history_plan) = if self.can_prepare_metadata_candidate() {
            (self.prepare_metadata_candidate(), None)
        } else {
            self.prepare_candidate_frame()?
        };
        if let Some(plan) = history_plan {
            if let Some(content) = prepared.content() {
                self.content.begin_prepared_candidate(content);
            }
            self.discard_prepared_candidate(&prepared, true);
            self.history_work = Some(HistoryWork::Prepared { plan });
            if self.pending_epoch == self.committed_epoch {
                self.ensure_pending()?;
            }
            return Ok(HostFlushOutcome {
                waiting_for_physical_work: true,
                ..HostFlushOutcome::default()
            });
        }
        if let Some(content) = prepared.content() {
            self.content.begin_prepared_candidate(content);
        }
        self.presentation_state = PresentationState::prepared(prepared);
        if let Err(error) = self.present_frame() {
            self.presentation_notify.notify_waiters();
            self.capture_failed_candidate();
            self.discard_candidate_frame();
            // `prepare_frame` clears the kernel dirty bit before the backend
            // handoff. Restore the retry obligation when that handoff fails;
            // otherwise the next flush could mistake the unchanged epoch for
            // a successful no-op and silently lose the desired frame.
            self.running.host_discard_candidate();
            return Err(error);
        }
        if self.bootstrap_pending
            || self.bootstrap_receipt.is_some()
            || self.presentation_state.is_in_flight()
        {
            return Ok(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            });
        }
        self.commit_frame()
    }

    fn can_prepare_metadata_candidate(&self) -> bool {
        let Some(changes) = self.pending_ui_changes.as_ref() else {
            return false;
        };
        !changes.physical_work
            && !self.physical_sync_unknown
            && !self.content_dirty
            && !self.content.has_pending_source_cleanup()
            && !self.running.is_dirty()
            && !self.bootstrap_pending
            && self.bootstrap_receipt.is_none()
            && !self.presentation_state.is_in_flight()
    }

    fn sync_ui_scene(&mut self) -> Result<()> {
        let revision = self.ui_resources.document.as_ref().map_or(
            0,
            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
        );
        if revision == 0 || revision == self.ui_scene_revision {
            return Ok(());
        }
        let changes = self.pending_ui_changes.take();
        let sync_result = (|| {
            self.sync_ui_controls(changes.as_ref())?;
            let changed_content_ports = self
                .legacy_scene
                .synchronize(&self.ui_resources, &mut self.content, changes.as_ref())
                .map_err(|error| {
                    host_attempt_error(
                        "frame",
                        "FRAME_PREPARATION_FAILED",
                        true,
                        format!("occurrence renderer synchronization failed: {error}"),
                    )
                })?;
            for port_id in changed_content_ports {
                self.running
                    .host_invalidate_content(crate::presentation::ContentDirty::new(
                        port_id,
                        None,
                        crate::presentation::ContentDirtyReason::SelectionLifecycle,
                    ));
            }
            let history_units = self
                .legacy_scene
                .history_units(&self.ui_resources, changes.as_ref())?;
            self.running
                .host_sync_ui_history(history_units, changes.as_ref(), &mut self.content)
                .map_err(|error| {
                    host_attempt_error("frame", "FRAME_PREPARATION_FAILED", true, error.to_string())
                })?;
            self.sync_ui_control_values(changes.as_ref())?;
            Ok(())
        })();
        if let Err(error) = sync_result {
            self.pending_ui_changes = changes;
            return Err(error);
        }
        let body = self.legacy_scene.body(&self.ui_resources)?;
        self.running.host_set_body(body);
        self.ui_scene_revision = revision;
        self.desired_structural_revision = revision;
        Ok(())
    }

    fn sync_ui_controls(&mut self, changes: Option<&crate::occurrence::UiChangeSet>) -> Result<()> {
        let Some(changes) = changes else {
            let keys = self
                .ui_resources
                .controls
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for key in keys {
                self.sync_ui_control(key, false)?;
            }
            return Ok(());
        };
        for key in changes.changed_resources.iter().copied() {
            if key.kind == crate::occurrence::HandleKind::Control {
                self.sync_ui_control(key, true)?;
            }
        }
        Ok(())
    }

    fn sync_ui_control(
        &mut self,
        key: crate::occurrence::ResourceKey,
        sync_editor_value: bool,
    ) -> Result<()> {
        crate::perf::inc(crate::perf::Counter::UiControlKeysVisited);
        #[cfg(test)]
        {
            self.ui_control_keys_visited = self.ui_control_keys_visited.saturating_add(1);
        }
        let Some(state) = self.ui_resources.control_state(key).cloned() else {
            self.retire_ui_control(key);
            return Ok(());
        };
        match state.kind() {
            crate::occurrence::ControlKind::Editor => {
                self.sync_ui_editor(key, &state, sync_editor_value)
            }
            crate::occurrence::ControlKind::Scroll => self.sync_ui_scroll(key),
            crate::occurrence::ControlKind::Animation => self.sync_ui_animation(key),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_ui_control_keys_visited(&self) -> usize {
        self.ui_control_keys_visited
    }

    fn retire_ui_control(&mut self, key: crate::occurrence::ResourceKey) {
        if let Some(input) = self.ui_editors.remove(&key)
            && let Some(component_id) = input.component_id()
        {
            self.running.host_retire_component(component_id);
        }
        if let Some(pane) = self.ui_scrolls.remove(&key)
            && let Some(component_id) = pane.component_id()
        {
            self.running.host_retire_component(component_id);
        }
        if let Some(slot) = self.ui_animations.remove(&key)
            && let Some(component_id) = slot.component_id()
        {
            self.running.host_retire_component(component_id);
        }
        self.legacy_scene.remove_control_component(key);
    }

    fn sync_ui_editor(
        &mut self,
        key: crate::occurrence::ResourceKey,
        state: &crate::occurrence::ControlState,
        sync_value: bool,
    ) -> Result<()> {
        let multiline = state
            .editor_multiline()
            .expect("Editor control has editor state");
        if !self.ui_editors.contains_key(&key) {
            let input = HostTextInput::new(multiline);
            let component = self.running.host_register(MountedTextInput(input.clone()));
            input.set_component_id(component.raw_id())?;
            self.ui_editors.insert(key, input);
            self.legacy_scene
                .set_control_component(key, component.raw_id());
        }
        if sync_value
            && let Some(input) = self.ui_editors.get(&key)
            && let Some(text) = state.editor_text()
        {
            let changed = {
                let mut input_state = input
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("UI editor lock is poisoned"))?;
                if input_state.text() == text {
                    false
                } else {
                    input_state.set_text(text);
                    true
                }
            };
            if changed && let Some(component) = input.component_id() {
                self.running.host_invalidate_component(component);
            }
        }
        Ok(())
    }

    fn sync_ui_scroll(&mut self, key: crate::occurrence::ResourceKey) -> Result<()> {
        if self.ui_scrolls.contains_key(&key) {
            return Ok(());
        }
        let pane = HostScrollPane::new(vf::spacer(0));
        let component = self.running.host_register(MountedScrollPane(pane.clone()));
        pane.set_component_id(component.raw_id())?;
        self.legacy_scene
            .set_control_component(key, component.raw_id());
        self.ui_scrolls.insert(key, pane);
        Ok(())
    }

    fn sync_ui_animation(&mut self, key: crate::occurrence::ResourceKey) -> Result<()> {
        if self.ui_animations.contains_key(&key) {
            return Ok(());
        }
        let slot = HostViewSlot::new(vf::spacer(0));
        let component = self.running.host_register(MountedViewSlot(slot.clone()));
        slot.set_component_id(component.raw_id())?;
        self.legacy_scene
            .set_control_component(key, component.raw_id());
        self.ui_animations.insert(key, slot);
        Ok(())
    }

    fn sync_ui_control_values(
        &mut self,
        changes: Option<&crate::occurrence::UiChangeSet>,
    ) -> Result<()> {
        let document = self
            .ui_resources
            .document
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("UI resource owner has no occurrence document"))?;
        let mut controls = changes.map_or_else(
            || self.ui_resources.controls.keys().copied().collect(),
            |changes| {
                changes
                    .changed_resources
                    .iter()
                    .filter(|key| key.kind == crate::occurrence::HandleKind::Control)
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
            },
        );
        if let Some(changes) = changes {
            for node in changes.changed_nodes.iter().copied() {
                let mut cursor = Some(node);
                while let Some(current) = cursor {
                    if let Some(control) = document.control_for_node(current) {
                        controls.insert(control);
                    }
                    cursor = document.parent_of(current);
                }
            }
        }
        let controls = controls
            .into_iter()
            .filter_map(|key| {
                self.ui_resources
                    .controls
                    .get(&key)
                    .cloned()
                    .map(|state| (key, state))
            })
            .collect::<Vec<_>>();
        let controls_changed = !controls.is_empty();
        for (key, state) in controls {
            let Some(node) = document.nodes_for_control(key).first().copied() else {
                continue;
            };
            let snapshot = self
                .ui_resources
                .document_snapshot(node)
                .map_err(anyhow::Error::msg)?;
            let children = self.legacy_scene.children_for(&snapshot)?;
            match state {
                crate::occurrence::ControlState::Scroll(_) => {
                    if let Some(pane) = self.ui_scrolls.get(&key) {
                        pane.state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("UI scroll lock is poisoned"))?
                            .set_content(vf::column(children, 0));
                    }
                }
                crate::occurrence::ControlState::Animation(animation) => {
                    if let Some(slot) = self.ui_animations.get(&key) {
                        slot.set_animation_running(animation.running())?;
                        let mut state = slot
                            .state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("UI animation lock is poisoned"))?;
                        state.frames = children;
                        if state.frames.is_empty() {
                            state.view = vf::spacer(0);
                            state.frame_index = 0;
                            state.last_tick = None;
                        } else {
                            let active = animation.active_frame() as usize % state.frames.len();
                            state.frame_index = active;
                            state.view = state.frames[active].clone();
                            state.interval = Duration::from_millis(u64::from(
                                animation.interval_ms().unwrap_or(0),
                            ));
                            state.last_tick =
                                if state.frames.len() > 1 && state.interval > Duration::ZERO {
                                    Some(self.now)
                                } else {
                                    None
                                };
                        }
                    }
                }
                crate::occurrence::ControlState::Editor(_) => {}
            }
        }
        if controls_changed {
            self.running.invalidate_frame();
        }
        Ok(())
    }

    fn dispatch_key_input(&mut self, key: KeyStroke) -> Result<()> {
        self.apply_key_input(key)?;
        self.advance_and_render()
    }

    fn apply_key_input(&mut self, key: KeyStroke) -> Result<()> {
        if self.running.input_disabled() {
            self.running
                .dispatch_key(key)
                .map_err(|error| anyhow::anyhow!("key dispatch failed: {error:?}"))?;
            return Ok(());
        }
        let admission = self
            .prepare_ui_input_events(UiInput::Key(key))?
            .map(|pending| self.reserve_ui_input_events(pending))
            .transpose()?;
        let result = self
            .running
            .dispatch_key(key)
            .map_err(|error| anyhow::anyhow!("key dispatch failed: {error:?}"))?;
        if admission.is_some() && result == InteractionResult::Ignored {
            return Err(anyhow::anyhow!(
                "UI editor preview diverged during key routing"
            ));
        }
        if let Some(admission) = admission {
            self.finish_ui_input_events(admission)?;
        }
        Ok(())
    }

    fn dispatch_paste_input(&mut self, text: &str) -> Result<()> {
        self.apply_paste_input(text)?;
        self.advance_and_render()
    }

    fn apply_paste_input(&mut self, text: &str) -> Result<()> {
        let route = self.running.prepare_paste_route(text);
        match route {
            crate::application::kernel::PasteDispatchOutcome::Disabled => return Ok(()),
            crate::application::kernel::PasteDispatchOutcome::Intercepted(output) => {
                self.running.queue_intercepted_paste(output);
                return Ok(());
            }
            crate::application::kernel::PasteDispatchOutcome::Local => {}
        }
        let admission = self
            .prepare_ui_input_events(UiInput::Paste(text))?
            .map(|pending| self.reserve_ui_input_events(pending))
            .transpose()?;
        let result = self
            .running
            .dispatch_paste_local(text)
            .map_err(|error| anyhow::anyhow!("paste dispatch failed: {error:?}"))?;
        if admission.is_some() && result == InteractionResult::Ignored {
            return Err(anyhow::anyhow!(
                "UI editor preview diverged during paste routing"
            ));
        }
        if let Some(admission) = admission {
            self.finish_ui_input_events(admission)?;
        }
        Ok(())
    }

    fn prepare_ui_input_events(
        &self,
        ui_input: UiInput<'_>,
    ) -> Result<Option<PreparedUiInputEvents>> {
        let Some(focused_component) = self.running.host_focused_component() else {
            return Ok(None);
        };
        let Some(control) = self.legacy_scene.control_for_component(focused_component) else {
            return Ok(None);
        };
        let Some(input) = self.ui_editors.get(&control) else {
            return Ok(None);
        };
        let state = input
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("UI editor lock is poisoned"))?;
        let before_text = state.text().to_owned();
        let before_cursor = state.cursor_bytes();
        let (preview, key_text, is_enter) = match ui_input {
            UiInput::Key(key) => (
                state.preview_key(key),
                Some(format!("{:?}", key.key())),
                state.is_submit_key(key),
            ),
            UiInput::Paste(text) => (Some(state.preview_paste(text)), None, false),
        };
        let Some(preview) = preview else {
            return Ok(None);
        };
        let text_changed = before_text != preview.text;
        let cursor_changed = before_cursor != preview.cursor_bytes;
        let mut mask = 0u32;
        if text_changed {
            mask |= 2 | 8;
        }
        if text_changed || cursor_changed {
            mask |= 4;
        }
        if cursor_changed {
            mask |= 16;
        }
        if is_enter {
            mask |= 1 | 32;
        }
        let Some(document) = self.ui_resources.document.as_ref() else {
            return Ok(None);
        };
        let mut targets = Vec::new();
        for node in document.nodes_for_control(control) {
            let snapshot = self
                .ui_resources
                .document_snapshot(node)
                .map_err(anyhow::Error::msg)?;
            let node_mask = mask & (snapshot.subscriptions as u32);
            if node_mask != 0 {
                targets.push((node.handle(self.ui_resources.namespace), node_mask));
            }
        }
        if targets.is_empty() {
            return Ok(None);
        }
        let mut events = Vec::new();
        events
            .try_reserve(targets.len())
            .map_err(|_| anyhow::anyhow!("UI event batch capacity reservation failed"))?;
        for (handle, node_mask) in targets {
            events.push(NativeUiEvent {
                handle,
                mask: node_mask,
                text: Some(preview.text.clone()),
                cursor_bytes: Some(preview.cursor_bytes),
                key: key_text.clone(),
                revision: None,
            });
        }
        let bytes = events.iter().try_fold(0usize, |total, event| {
            total
                .checked_add(event.payload_bytes()?)
                .ok_or_else(|| anyhow::anyhow!("UI event payload size overflow"))
        })?;
        Ok(Some(PreparedUiInputEvents {
            control,
            input: input.clone(),
            preview,
            events,
            bytes,
        }))
    }

    fn ensure_ui_event_capacity(&self) -> Result<()> {
        if self.ui_event_backpressure
            || self.ui_events.len() >= self.ui_event_limits.max_records
            || self.ui_event_bytes >= self.ui_event_limits.max_bytes
        {
            return Err(anyhow::Error::new(UiInputAdmissionError::Backpressure(
                "native UI event queue is full",
            )));
        }
        Ok(())
    }

    fn reserve_ui_input_events(
        &mut self,
        pending: PreparedUiInputEvents,
    ) -> Result<PreparedUiInputEvents> {
        let records = pending.events.len();
        let queued_records = self
            .ui_events
            .len()
            .checked_add(records)
            .ok_or_else(|| anyhow::anyhow!("UI event record accounting overflow"))?;
        let queued_bytes = self
            .ui_event_bytes
            .checked_add(pending.bytes)
            .ok_or_else(|| anyhow::anyhow!("UI event byte accounting overflow"))?;
        if records > self.ui_event_limits.max_records
            || pending.bytes > self.ui_event_limits.max_bytes
        {
            return Err(anyhow::Error::new(UiInputAdmissionError::TooLarge(
                "one native UI event batch cannot fit configured limits",
            )));
        }
        if self.ui_event_backpressure
            || queued_records > self.ui_event_limits.max_records
            || queued_bytes > self.ui_event_limits.max_bytes
        {
            self.ui_event_backpressure = true;
            return Err(anyhow::Error::new(UiInputAdmissionError::Backpressure(
                "native UI event queue is full",
            )));
        }
        if self.ui_events.try_reserve(records).is_err() {
            self.ui_event_backpressure = true;
            return Err(anyhow::Error::new(UiInputAdmissionError::Backpressure(
                "native UI event queue capacity reservation failed",
            )));
        }
        Ok(pending)
    }

    fn take_ui_event_batch(&mut self) -> Vec<NativeUiEvent> {
        let events: Vec<_> = self.ui_events.drain(..).collect();
        self.ui_event_bytes = 0;
        self.ui_event_backpressure = false;
        events
    }

    fn finish_ui_input_events(&mut self, mut pending: PreparedUiInputEvents) -> Result<()> {
        let state = pending
            .input
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("UI editor lock is poisoned"))?;
        let matches_preview = state.text() == pending.preview.text
            && state.cursor_bytes() == pending.preview.cursor_bytes;
        if !matches_preview {
            return Err(anyhow::anyhow!(
                "UI editor preview diverged during input dispatch"
            ));
        }
        let revision = self
            .ui_resources
            .control_state(pending.control)
            .and_then(crate::occurrence::ControlState::editor_revision);
        for event in &mut pending.events {
            event.revision = revision;
        }
        drop(state);
        self.ui_event_bytes = self
            .ui_event_bytes
            .checked_add(pending.bytes)
            .expect("accepted UI event byte accounting overflow");
        self.ui_events.extend(pending.events);
        self.ui_event_notify.notify_waiters();
        Ok(())
    }

    fn present_frame(&mut self) -> Result<()> {
        self.poll_bootstrap_receipt()?;
        if self.bootstrap_receipt.is_some() {
            return Ok(());
        }
        if self.bootstrap_pending {
            if let HostBackend::Real(backend) = self
                .backend
                .as_mut()
                .expect("open host must own its terminal backend")
            {
                match backend.begin_frame(&self.frame) {
                    Ok(receipt) => {
                        self.bootstrap_receipt = Some(PresentReceipt::from_receiver(
                            receipt,
                            self.environment.receipt_wake(self.host_id),
                        ));
                    }
                    Err(error) if crate::terminal::is_terminal_worker_stopped(&error) => {
                        self.mark_faulted();
                        self.physical_sync_unknown = true;
                        let failure = host_attempt_error(
                            "backend",
                            "BACKEND_NOT_READY",
                            false,
                            error.to_string(),
                        );
                        let ui_revision = self.ui_resources.document.as_ref().map_or(
                            0,
                            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                        );
                        self.record_failed_frame(
                            &failure,
                            "backend",
                            ui_revision,
                            self.pending_epoch,
                        );
                        return Err(failure);
                    }
                    Err(error) => {
                        self.physical_sync_unknown = true;
                        let failure = host_attempt_error(
                            "backend",
                            "BACKEND_IO_FAILED",
                            true,
                            error.to_string(),
                        );
                        let ui_revision = self.ui_resources.document.as_ref().map_or(
                            0,
                            crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                        );
                        self.record_failed_frame(
                            &failure,
                            "backend",
                            ui_revision,
                            self.pending_epoch,
                        );
                        return Err(failure);
                    }
                }
            }
            self.bootstrap_pending = false;
            self.poll_bootstrap_receipt()?;
            return Ok(());
        }

        let state = std::mem::replace(&mut self.presentation_state, PresentationState::Idle);
        match state {
            PresentationState::Prepared(frame) => {
                if frame.is_no_output() {
                    self.presentation_state = PresentationState::Completing { frame };
                } else if let HostBackend::Real(backend) = self
                    .backend
                    .as_mut()
                    .expect("open host must own its terminal backend")
                {
                    let scene = frame
                        .scene()
                        .ok_or_else(|| anyhow::anyhow!("output frame has no captured scene"))?;
                    match backend.begin_frame(scene) {
                        Ok(receipt) => {
                            self.presentation_state = PresentationState::InFlight {
                                frame,
                                receipt: PresentReceipt::from_receiver(
                                    receipt,
                                    self.environment.receipt_wake(self.host_id),
                                ),
                            };
                            self.poll_in_flight_receipt()?;
                        }
                        Err(error) if crate::terminal::is_terminal_worker_stopped(&error) => {
                            self.mark_faulted();
                            self.physical_sync_unknown = true;
                            let failure = host_attempt_error(
                                "backend",
                                "BACKEND_NOT_READY",
                                false,
                                error.to_string(),
                            );
                            self.record_failed_frame(
                                &failure,
                                "backend",
                                frame.ui_revision,
                                frame.work_epoch,
                            );
                            return Err(failure);
                        }
                        Err(error) => {
                            self.physical_sync_unknown = true;
                            let failure = host_attempt_error(
                                "backend",
                                "BACKEND_IO_FAILED",
                                true,
                                error.to_string(),
                            );
                            self.record_failed_frame(
                                &failure,
                                "backend",
                                frame.ui_revision,
                                frame.work_epoch,
                            );
                            return Err(failure);
                        }
                    }
                } else {
                    self.presentation_state = PresentationState::Completing { frame };
                }
            }
            PresentationState::InFlight { frame, receipt } => {
                self.presentation_state = PresentationState::InFlight { frame, receipt };
                self.poll_in_flight_receipt()?;
            }
            state @ PresentationState::Completing { .. }
            | state @ PresentationState::Failed(_)
            | state @ PresentationState::Idle
            | state @ PresentationState::Closed => {
                self.presentation_state = state;
            }
        }
        Ok(())
    }

    fn poll_in_flight_receipt(&mut self) -> Result<()> {
        let state = std::mem::replace(&mut self.presentation_state, PresentationState::Idle);
        let PresentationState::InFlight { frame, mut receipt } = state else {
            self.presentation_state = state;
            return Ok(());
        };
        match receipt.poll() {
            Poll::Pending => {
                self.presentation_state = PresentationState::InFlight { frame, receipt };
                Ok(())
            }
            Poll::Ready(Ok(())) => {
                self.presentation_state = PresentationState::Completing { frame };
                Ok(())
            }
            Poll::Ready(Err(error)) => {
                self.physical_sync_unknown = true;
                let diagnostic = error.to_string();
                let failure = host_attempt_error(
                    "backend",
                    "BACKEND_IO_FAILED",
                    true,
                    format!("terminal presentation failed: {diagnostic}"),
                );
                self.record_failed_frame(&failure, "backend", frame.ui_revision, frame.work_epoch);
                Err(failure)
            }
        }
    }

    fn poll_bootstrap_receipt(&mut self) -> Result<()> {
        let Some(mut receipt) = self.bootstrap_receipt.take() else {
            return Ok(());
        };
        match receipt.poll() {
            Poll::Ready(Ok(())) => Ok(()),
            Poll::Ready(Err(error)) => {
                self.physical_sync_unknown = true;
                let failure = host_attempt_error(
                    "backend",
                    "BACKEND_IO_FAILED",
                    true,
                    format!("terminal presentation failed: {error}"),
                );
                let ui_revision = self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                self.record_failed_frame(&failure, "backend", ui_revision, self.pending_epoch);
                Err(failure)
            }
            Poll::Pending => {
                self.bootstrap_receipt = Some(receipt);
                Ok(())
            }
        }
    }

    fn commit_frame(&mut self) -> Result<HostFlushOutcome> {
        // All normal preconditions are checked before entering the
        // environment-owned completion authority. The environment mutex then
        // remains held across content/state/frame promotion and its queue
        // completion, so no independently poisonable lock is reacquired after
        // visible authority changes.
        let candidate = self
            .presentation_state
            .frame()
            .ok_or_else(|| anyhow::anyhow!("missing prepared presentation frame"))?;
        let next_visible_frame_revision = candidate.physical_frame_id();
        if candidate.is_no_output() {
            if next_visible_frame_revision != self.visible_frame_revision {
                return Err(anyhow::anyhow!(
                    "metadata candidate does not reference the confirmed physical frame"
                ));
            }
        } else if next_visible_frame_revision
            != self
                .visible_frame_revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("visible frame revision exhausted"))?
        {
            return Err(anyhow::anyhow!(
                "output candidate physical frame identity is not monotonic"
            ));
        }
        let candidate_epoch = candidate.work_epoch;
        let candidate_ui_revision = candidate.ui_revision;
        let candidate_structural_revision = candidate.structural_revision;
        let environment = self.environment.clone();
        environment.with_host_completion(self.host_id, candidate_epoch, true, false, || {
            let state = std::mem::replace(&mut self.presentation_state, PresentationState::Idle);
            let candidate = match state {
                PresentationState::Prepared(frame) | PresentationState::Completing { frame } => {
                    frame
                }
                PresentationState::InFlight { .. } => {
                    return Err(anyhow::anyhow!(
                        "cannot complete a frame while its receipt is still in flight"
                    ));
                }
                PresentationState::Idle
                | PresentationState::Failed(_)
                | PresentationState::Closed => {
                    return Err(anyhow::anyhow!("missing prepared presentation frame"));
                }
            };
            let deferred_source_cleanup = if let Some(content) = candidate.content() {
                match self.content.commit_prepared(content) {
                    Ok(deferred_source_cleanup) => deferred_source_cleanup,
                    Err(error) => {
                        let (code, retryable) = error
                            .downcast_ref::<super::environment::HostAttemptError>()
                            .map_or(("FRAME_PREPARATION_FAILED", true), |failure| {
                                (failure.code, failure.retryable)
                            });
                        let failure = FrameFailure {
                            phase: "frame",
                            code,
                            attempted_ui_revision: candidate_ui_revision,
                            attempted_work_epoch: candidate_epoch,
                            retryable,
                            diagnostic: error.to_string(),
                        };
                        self.publish_failure_notification(&failure);
                        self.failed_attempt = Some(AttemptStamp {
                            revision: self.attempt_revision,
                            work_epoch: candidate_epoch,
                            desired_revision: candidate_structural_revision,
                        });
                        self.presentation_state =
                            PresentationState::Completing { frame: candidate };
                        return Err(error);
                    }
                }
            } else {
                false
            };
            if candidate.content().is_some() {
                let content_dirty_epoch = candidate
                    .content_dirty_epoch()
                    .expect("scene product has a content epoch");
                self.content.end_candidate();
                self.running
                    .host_commit_content_candidate(content_dirty_epoch);
            }
            let block_for_deferred_cleanup =
                deferred_source_cleanup && self.pending_epoch == candidate_epoch;
            // A successful recovery frame re-establishes screen state, but it
            // cannot establish which rows an errored History receipt may have
            // accepted. Keep History's replay barrier until an explicit owner
            // performs a native resynchronization; clearing it here would
            // duplicate the uncertain suffix.
            self.physical_sync_unknown = false;
            match candidate.product {
                PreparedFrameProduct::Scene { products, .. }
                | PreparedFrameProduct::NoOutput { products, .. } => {
                    self.frame = products.scene;
                }
                PreparedFrameProduct::Metadata { .. } => {}
            }
            self.presentation_state = PresentationState::Idle;
            self.presentation_notify.notify_waiters();
            self.visible_structural_revision = candidate_structural_revision;
            self.visible_frame_revision = next_visible_frame_revision;
            self.committed_epoch = candidate_epoch;
            if self
                .pending_ui_changes
                .as_ref()
                .is_some_and(|changes| changes.ui_revision <= candidate_ui_revision)
            {
                self.pending_ui_changes = None;
            }
            if self.pending_epoch == candidate_epoch {
                self.content_dirty = false;
            }
            Ok((
                HostFlushOutcome {
                    committed: true,
                    waiting_for_presentation: false,
                    waiting_for_physical_work: false,
                    committed_epoch: Some(candidate_epoch),
                    visible_structural_revision: Some(candidate_structural_revision),
                },
                self.pending_epoch,
                block_for_deferred_cleanup,
            ))
        })
    }

    fn capture_failed_candidate(&mut self) {
        if let Some(frame) = self.presentation_state.frame() {
            self.failed_attempt = Some(AttemptStamp {
                revision: self.attempt_revision,
                work_epoch: frame.work_epoch,
                desired_revision: frame.structural_revision,
            });
        } else if let PresentationState::Failed(failure) = &self.presentation_state {
            self.failed_attempt = Some(AttemptStamp {
                revision: self.attempt_revision,
                work_epoch: failure.attempted_work_epoch,
                desired_revision: self.desired_structural_revision,
            });
        }
    }

    /// Reconciles a candidate whose backend receipt has already completed
    /// before any newer dirty work is advanced or prepared.  Commit failures
    /// intentionally retain the exact candidate so an explicit retry can
    /// rerun its preflight against the same state/content/frontier plan.
    fn retry_existing_candidate(&mut self) -> Result<Option<HostFlushOutcome>> {
        if self.presentation_state.is_in_flight() {
            let outcome = self.poll_presentation()?;
            if outcome
                .as_ref()
                .is_some_and(|outcome| outcome.committed || outcome.waiting_for_presentation)
            {
                return Ok(outcome);
            }
        }
        if matches!(
            self.presentation_state,
            PresentationState::Completing { .. }
        ) {
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

    /// One failure owner for every asynchronous/close History receipt path.
    /// The confirmed semantic prefix is left untouched, while both the host
    /// physical marker and History's replay barrier are recorded.
    fn fail_history_transfer(&mut self) {
        self.content.abort_candidate();
        self.running.host_abort_content_candidate();
        self.running
            .host_mark_native_history_synchronization_unknown();
        self.physical_sync_unknown = true;
    }

    fn record_failed_frame(
        &mut self,
        error: &anyhow::Error,
        phase: &'static str,
        ui_revision: u64,
        work_epoch: u64,
    ) {
        let (code, retryable) = error
            .downcast_ref::<super::environment::HostAttemptError>()
            .map_or(("FRAME_PREPARATION_FAILED", true), |failure| {
                (failure.code, failure.retryable)
            });
        let failure = FrameFailure {
            phase,
            code,
            attempted_ui_revision: ui_revision,
            attempted_work_epoch: work_epoch,
            retryable,
            diagnostic: error.to_string(),
        };
        self.publish_failure_notification(&failure);
        self.presentation_state = PresentationState::Failed(failure);
        self.failed_attempt = Some(AttemptStamp {
            revision: self.attempt_revision,
            work_epoch,
            desired_revision: self.desired_structural_revision,
        });
    }

    fn discard_candidate_frame(&mut self) {
        let has_content_candidate = self
            .presentation_state
            .frame()
            .is_some_and(|frame| frame.content().is_some());
        if has_content_candidate {
            self.content.abort_candidate();
        }
        if !matches!(
            self.presentation_state,
            PresentationState::Closed | PresentationState::Failed(_)
        ) {
            self.presentation_state = PresentationState::Idle;
            self.presentation_notify.notify_waiters();
        }
        if has_content_candidate {
            self.running.host_abort_content_candidate();
        }
    }

    fn discard_prepared_candidate(&mut self, frame: &PreparedFrame, retain_content: bool) {
        if frame.content().is_some() && !retain_content {
            self.content.abort_candidate();
            self.running.host_abort_content_candidate();
        }
        if retain_content {
            self.running.host_abort_content_candidate();
        }
        self.running.host_discard_candidate();
    }

    fn poll_presentation(&mut self) -> Result<Option<HostFlushOutcome>> {
        if self.bootstrap_receipt.is_none() && !self.presentation_state.is_in_flight() {
            return Ok(None);
        }
        if let Err(error) = self.present_frame() {
            self.capture_failed_candidate();
            self.discard_candidate_frame();
            self.running.host_discard_candidate();
            self.presentation_notify.notify_waiters();
            return Err(error);
        }
        self.presentation_notify.notify_waiters();
        if self.bootstrap_receipt.is_some() || self.presentation_state.is_in_flight() {
            return Ok(Some(HostFlushOutcome {
                committed: false,
                waiting_for_presentation: true,
                ..HostFlushOutcome::default()
            }));
        }
        if matches!(
            self.presentation_state,
            PresentationState::Completing { .. }
        ) {
            return self.commit_frame().map(Some);
        }
        Ok(Some(HostFlushOutcome::default()))
    }

    fn finish_presentation_blocking(&mut self) -> Result<HostFlushOutcome> {
        if let Some(receipt) = self.bootstrap_receipt.take() {
            if let Err(error) = receipt.blocking_recv() {
                self.physical_sync_unknown = true;
                let failure = host_attempt_error(
                    "backend",
                    "BACKEND_IO_FAILED",
                    true,
                    format!("terminal presentation failed: {error}"),
                );
                let ui_revision = self.ui_resources.document.as_ref().map_or(
                    0,
                    crate::occurrence::OccurrenceDocument::accepted_ui_revision,
                );
                self.record_failed_frame(&failure, "backend", ui_revision, self.pending_epoch);
                return Err(failure);
            }
        }
        let presentation = std::mem::replace(&mut self.presentation_state, PresentationState::Idle);
        if let PresentationState::InFlight { frame, receipt } = presentation {
            if let Err(error) = receipt.blocking_recv() {
                self.physical_sync_unknown = true;
                let failure = host_attempt_error(
                    "backend",
                    "BACKEND_IO_FAILED",
                    true,
                    format!("terminal presentation failed: {error}"),
                );
                self.record_failed_frame(&failure, "backend", frame.ui_revision, frame.work_epoch);
                return Err(failure);
            }
            self.presentation_state = PresentationState::Completing { frame };
        } else {
            self.presentation_state = presentation;
        }
        if matches!(
            self.presentation_state,
            PresentationState::Completing { .. }
        ) {
            return self.commit_frame();
        }
        self.flush_pending_frame()
    }

    fn flush_pending_frame(&mut self) -> Result<HostFlushOutcome> {
        if self.is_closed() {
            return Err(anyhow::anyhow!("host is closed"));
        }
        match self.poll_history_work()? {
            HistoryWorkPoll::Pending => {
                return Ok(HostFlushOutcome {
                    waiting_for_physical_work: true,
                    ..HostFlushOutcome::default()
                });
            }
            HistoryWorkPoll::None | HistoryWorkPoll::Progress | HistoryWorkPoll::Blocked => {}
        }
        if let Some(failure) = self.scheduler_failure.clone() {
            return Err(host_attempt_error(
                failure.phase,
                failure.code,
                failure.retryable,
                failure.diagnostic,
            ));
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
        self.attempt_revision = self
            .attempt_revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("host attempt revision exhausted"))?;
        #[cfg(test)]
        if let Some(diagnostic) = self.fail_next_frame.take() {
            let error = host_attempt_error("frame", "FRAME_PREPARATION_FAILED", true, diagnostic);
            let ui_revision = self.ui_resources.document.as_ref().map_or(
                0,
                crate::occurrence::OccurrenceDocument::accepted_ui_revision,
            );
            self.record_failed_frame(&error, "frame", ui_revision, self.pending_epoch);
            return Err(error);
        }
        let status_dirty = self.advance_runtime_for_candidate(true)?;

        if (self.bootstrap_receipt.is_some() || self.presentation_state.is_in_flight())
            && let Some(outcome) = self.poll_presentation()?
            && (outcome.committed || outcome.waiting_for_presentation)
        {
            return Ok(outcome);
        }
        // The bootstrap frame completed. Continue below so a desired
        // epoch accepted before that receipt is prepared now.

        if status_dirty {
            return self.render();
        }

        if self.bootstrap_pending || self.bootstrap_receipt.is_some() {
            if let Err(error) = self.present_frame() {
                self.capture_failed_candidate();
                self.discard_candidate_frame();
                self.running.host_discard_candidate();
                return Err(error);
            }
            if self.bootstrap_receipt.is_some() {
                return Ok(HostFlushOutcome {
                    committed: false,
                    waiting_for_presentation: true,
                    ..HostFlushOutcome::default()
                });
            }
        }

        if matches!(self.presentation_state, PresentationState::Prepared(_)) {
            if let Err(error) = self.present_frame() {
                self.capture_failed_candidate();
                self.discard_candidate_frame();
                self.running.host_discard_candidate();
                return Err(error);
            }
            if self.presentation_state.is_in_flight() {
                return Ok(HostFlushOutcome {
                    committed: false,
                    waiting_for_presentation: true,
                    ..HostFlushOutcome::default()
                });
            }
            if matches!(
                self.presentation_state,
                PresentationState::Completing { .. }
            ) {
                return self.commit_frame();
            }
        }

        // A receipt may have completed while visible commit was blocked by a
        // poisoned prepared record. Keep that exact candidate for retry; do
        // not start a new preparation pass over an uncommitted frame.
        if matches!(
            self.presentation_state,
            PresentationState::Completing { .. }
        ) {
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
        if matches!(self.backend, Some(HostBackend::Real(_))) {
            self.now = Instant::now();
        }
    }
}

fn prepare_frame(
    running: &mut HostRunning,
    backend: &mut HostBackend,
    now: Instant,
) -> Result<PreparedSceneFrame> {
    let mut content = EmptyContentProvider;
    prepare_frame_with_content(running, backend, now, &mut content)
}

fn prepare_frame_with_content(
    running: &mut HostRunning,
    backend: &mut HostBackend,
    now: Instant,
    content: &mut dyn ContentProvider,
) -> Result<PreparedSceneFrame> {
    content.set_theme(running.theme_shared());
    match backend {
        HostBackend::Headless(sink) => running
            .prepare_frame(
                now,
                sink,
                |sink| Ok(Size::new(sink.width, sink.height)),
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
            .prepare_frame(
                now,
                backend,
                super::super::terminal::backend::TerminalBackend::viewport,
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
    use std::time::Duration;
    use std::{
        future::{Future, poll_fn},
        sync::{Arc, Barrier, mpsc},
        task::Poll,
    };

    use crate::physical::PhysicalRow;
    use crate::presentation::factory as vf;
    use tokio::sync::oneshot;

    use super::super::environment::TuiEnvironment;
    use super::{
        ClosePhase, HostTextInput, MAX_FAILURE_NOTIFICATIONS, NativeUiEvent, PresentationState,
        RoutedOutput, TuiHost,
    };
    use crate::occurrence::{HostKind, NodeRef, OwnershipMode, ResourceRef, UiCommit, UiOperation};
    use crate::{Key, KeyStroke, View};

    fn install_delayed_candidate(host: &TuiHost) -> oneshot::Sender<anyhow::Result<()>> {
        let (sender, receiver) = oneshot::channel();
        let mut inner = host.inner.lock().unwrap();
        let candidate = {
            let super::HostInner {
                running,
                backend,
                now,
                ..
            } = &mut *inner;
            super::prepare_frame(
                running,
                backend
                    .as_mut()
                    .expect("test host must own its terminal backend"),
                *now,
            )
            .unwrap()
        };
        inner.install_test_in_flight(candidate, receiver).unwrap();
        sender
    }

    fn accept_ui_box(host: &TuiHost) -> crate::occurrence::UiHandle {
        let body = host.ui_body_handle().unwrap();
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        let result = host.commit_ui(batch, &[]).unwrap();
        assert!(result.acknowledgement.wake_flags != 0);
        body
    }

    fn accept_ui_literal(host: &TuiHost, text: &[u8]) -> (crate::occurrence::UiHandle, u64) {
        let body = host.ui_body_handle().unwrap();
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(crate::occurrence::ResourceRef::Local(2)),
        });
        batch.push(UiOperation::ReplaceLiteral {
            port: crate::occurrence::ResourceRef::Local(2),
            content_format: 1,
            content: text.to_vec(),
            annotations: Vec::new(),
        });
        let result = host.commit_ui(batch, &[]).unwrap();
        assert!(result.acknowledgement.wake_flags != 0);
        (body, result.acknowledgement.accepted_ui_revision)
    }

    fn set_test_history(host: &TuiHost, views: &[View]) -> anyhow::Result<()> {
        let mut history = crate::History::new();
        for view in views {
            history.push(view.clone())?;
        }
        let mut inner = host.inner.lock().unwrap();
        inner.running.host_set_history(history);
        inner.running.invalidate_frame();
        inner.ensure_pending()
    }

    #[test]
    fn native_text_input_routes_local_paste_and_submit() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let input = host.create_text_input(false).unwrap();
        host.route_text_input(&input, "submit").unwrap();
        let input_view = vf::native_component(input.component_id().unwrap());
        host.set_test_view(input_view).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

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
        host.set_test_view(vf::native_component(input.component_id().unwrap()))
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

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
        host.set_test_view(vf::text("unfocused")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

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
    fn native_global_key_binding_precedes_local_component_input() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let input = host.create_text_input(false).unwrap();
        let key = KeyStroke::new(Key::Char('q'));
        host.bind_key(key, "global").unwrap();
        host.set_test_view(vf::native_component(input.component_id().unwrap()))
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        host.dispatch_key(key).unwrap();
        assert_eq!(input.text().unwrap(), "");
        assert_eq!(
            host.next_output(),
            Some(RoutedOutput {
                route_id: "global".to_owned(),
                payload: None,
            })
        );
        host.dispatch_key(KeyStroke::new(Key::Char('u'))).unwrap();
        assert_eq!(input.text().unwrap(), "u");
        assert_eq!(host.next_output(), None);

        host.set_test_view(vf::text("unfocused")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
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
    fn desired_revision_waits_for_a_successful_frame_barrier() {
        let host = TuiHost::open(20, 4, true).unwrap();
        let initial = host.epochs().unwrap();
        assert_eq!(initial.desired_structural_revision, 0);
        assert_eq!(initial.visible_frame_revision, 0);
        assert_eq!(initial.pending_epoch, initial.committed_epoch);

        host.set_test_view(vf::text("desired")).unwrap();
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
    fn metadata_only_candidate_completes_without_a_second_terminal_write() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_test_view(vf::text("same")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let first = host.epochs().unwrap();

        // The desired revision advances even though the captured physical
        // surface is unchanged. The NoOutput path must publish metadata and
        // the structural barrier without manufacturing terminal bytes.
        host.set_test_view(vf::text("same")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let second = host.epochs().unwrap();
        assert!(second.visible_structural_revision > first.visible_structural_revision);
        assert_eq!(second.visible_frame_revision, first.visible_frame_revision);
        assert_eq!(second.pending_epoch, second.committed_epoch);
        host.close().unwrap();
    }

    #[test]
    fn failed_frame_keeps_old_visible_state_and_explicit_retry_recovers() {
        let host = TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).unwrap();
        host.set_test_view(vf::text("old")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let old_rows = host.screen_rows();

        host.fail_next_frame_for_test("injected frame preparation failure")
            .unwrap();
        host.set_test_view(vf::text("new")).unwrap();
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
            host.set_test_view(crate::presentation::factory::style(
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
        let host = TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).unwrap();
        host.set_test_view(vf::text("receipt")).unwrap();
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
                super::prepare_frame(
                    running,
                    backend
                        .as_mut()
                        .expect("test host must own its terminal backend"),
                    *now,
                )
                .unwrap()
            };
            inner.install_test_in_flight(candidate, receiver).unwrap();
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
    fn blocking_completion_preserves_older_confirmed_candidate() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        host.set_test_view(vf::text("older")).unwrap();
        let captured_revision = host.epochs().unwrap().desired_structural_revision;
        let sender = install_delayed_candidate(&host);
        sender.send(Ok(())).unwrap();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.present_frame().unwrap();
            assert!(matches!(
                inner.presentation_state,
                PresentationState::Completing { .. }
            ));
        }

        host.set_test_view(vf::text("newer")).unwrap();
        let completed = host
            .inner
            .lock()
            .unwrap()
            .finish_presentation_blocking()
            .unwrap();
        assert!(completed.committed);
        let visible = host.epochs().unwrap();
        assert_eq!(visible.visible_structural_revision, captured_revision);
        assert!(visible.pending_epoch > visible.committed_epoch);
        assert!(host.screen_rows().iter().any(|row| row.contains("older")));

        host.flush_pending_hosts(8, true).unwrap();
        assert!(host.screen_rows().iter().any(|row| row.contains("newer")));
        host.close().unwrap();
    }

    #[test]
    fn production_environment_settles_two_visibility_waiters_without_a_caller_drain() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let sender = install_delayed_candidate(&host);
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        host.inner.lock().unwrap().waiting_for_presentation_hook = Some((waiting_tx, release_rx));
        host.set_test_view(vf::text("delayed")).unwrap();
        waiting_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("production driver must observe the delayed receipt");
        release_tx.send(()).unwrap();
        let target = host.epochs().unwrap().desired_structural_revision;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let first_host = host.clone();
        let second_host = host.clone();
        runtime.block_on(async move {
            let mut first = Box::pin(first_host.wait_for_ui_presentation(target, false));
            let mut second = Box::pin(second_host.wait_for_ui_presentation(target, false));
            poll_fn(|context| match first.as_mut().poll(context) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(result) => panic!("first waiter settled before receipt: {result:?}"),
            })
            .await;
            poll_fn(|context| match second.as_mut().poll(context) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(result) => panic!("second waiter settled before receipt: {result:?}"),
            })
            .await;
            sender.send(Ok(())).unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                assert!(first.await.is_ok());
                assert!(second.await.is_ok());
            })
            .await
            .expect("both visibility waiters must settle after the receipt");
        });
        assert_eq!(
            host.epochs().unwrap().pending_epoch,
            host.epochs().unwrap().committed_epoch
        );
        host.close().unwrap();
    }

    #[test]
    fn production_receipt_sender_drop_settles_visibility_barrier() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let sender = install_delayed_candidate(&host);
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        host.inner.lock().unwrap().waiting_for_presentation_hook = Some((waiting_tx, release_rx));
        host.set_test_view(vf::text("sender-drop")).unwrap();
        waiting_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("production driver must observe the delayed receipt");
        release_tx.send(()).unwrap();
        let target = host.epochs().unwrap().desired_structural_revision;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let waiter_host = host.clone();
        let waiter =
            runtime.spawn(async move { waiter_host.wait_for_ui_presentation(target, false).await });
        runtime.block_on(tokio::task::yield_now());
        drop(sender);
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), waiter)
                .await
                .expect("sender drop must wake the native barrier")
                .expect("barrier task must not panic")
        });
        assert!(result.is_ok(), "sender drop must settle through recovery");
        host.close().unwrap();
    }

    #[test]
    fn deferred_terminal_paste_resumes_once_after_event_drain() {
        let host = TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Editor,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 2,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(host.ui_body_handle().unwrap()),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::SetSubscriptions {
            node: NodeRef::Local(1),
            mask_low: 8,
            mask_high: 0,
        });
        mount.push(UiOperation::ReplaceEditorContent {
            control: crate::occurrence::ResourceRef::Local(2),
            content: b"a".to_vec(),
            expected_edit_revision: u64::MAX,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        host.focus_ui(mounted.acknowledgement.created[0]).unwrap();
        host.set_ui_event_limits(2, 4).unwrap();
        host.dispatch_paste("b").unwrap();
        let control = mounted.acknowledgement.created[1].resource_key().unwrap();
        let input = host.inner.lock().unwrap().ui_editors[&control].clone();
        // Seed the owning slot at the already-dequeued backend boundary, then
        // exercise the real poll/admission/requeue path without a terminal.
        host.inner.lock().unwrap().deferred_terminal_input =
            Some(crate::terminal::TerminalEvent::Paste("c".to_owned()));
        assert!(super::is_event_backpressure(
            &host.poll_terminal().unwrap_err()
        ));
        assert_eq!(input.text().unwrap(), "ab");
        assert!(host.inner.lock().unwrap().deferred_terminal_input.is_some());
        let first = host.drain_ui_events().unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].text.as_deref(), Some("ab"));
        host.poll_terminal().unwrap();
        assert_eq!(input.text().unwrap(), "abc");
        assert!(host.inner.lock().unwrap().deferred_terminal_input.is_none());
        let second = host.drain_ui_events().unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text.as_deref(), Some("abc"));
        host.poll_terminal().unwrap();
        assert!(host.drain_ui_events().unwrap().is_empty());
        assert_eq!(input.text().unwrap(), "abc");
        host.close().unwrap();
    }

    #[test]
    fn confirmed_occurrence_geometry_survives_newer_delayed_receipt() {
        let host = TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::Editor,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 3,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(2)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(UiOperation::ReplaceEditorContent {
            control: crate::occurrence::ResourceRef::Local(3),
            content: b"A".to_vec(),
            expected_edit_revision: u64::MAX,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let box_handle = mounted.acknowledgement.created[0];
        let first_geometry = host.ui_visible_geometry(box_handle).unwrap().unwrap();

        let mut replacement = UiCommit::new(1);
        replacement.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(box_handle),
            property: crate::occurrence::PropertyId::Padding,
            value: crate::occurrence::LayerValue::Value(crate::occurrence::PropertyValue::Insets(
                crate::Insets::all(1),
            )),
        });
        host.commit_ui(replacement, &[]).unwrap();

        let (sender, receiver) = oneshot::channel();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.sync_ui_scene().unwrap();
            let mut candidate = {
                let super::HostInner {
                    running,
                    backend,
                    now,
                    ..
                } = &mut *inner;
                super::prepare_frame(
                    running,
                    backend
                        .as_mut()
                        .expect("test host must own its terminal backend"),
                    *now,
                )
                .unwrap()
            };
            candidate.occurrence_geometry = inner
                .legacy_scene
                .occurrence_geometry(&candidate.view_geometry);
            inner.install_test_in_flight(candidate, receiver).unwrap();
        }

        // The confirmed frame still owns A while B is prepared and in flight.
        assert_eq!(
            host.ui_visible_geometry(box_handle).unwrap().unwrap(),
            first_geometry
        );

        let mut superseding = UiCommit::new(2);
        superseding.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(box_handle),
            property: crate::occurrence::PropertyId::Padding,
            value: crate::occurrence::LayerValue::Value(crate::occurrence::PropertyValue::Insets(
                crate::Insets::ZERO,
            )),
        });
        host.commit_ui(superseding, &[]).unwrap();
        assert_eq!(
            host.ui_visible_geometry(box_handle).unwrap().unwrap(),
            first_geometry
        );

        sender.send(Ok(())).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let second_geometry = host.ui_visible_geometry(box_handle).unwrap().unwrap();
        assert_ne!(second_geometry, first_geometry);

        let mut retire = UiCommit::new(3);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(box_handle),
        });
        host.commit_ui(retire, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        assert!(host.ui_visible_geometry(box_handle).is_err());

        let mut reuse = UiCommit::new(4);
        reuse.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        reuse.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::Box,
        });
        reuse.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        reuse.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(2),
            before: None,
        });
        let reused = host.commit_ui(reuse, &[]).unwrap();
        let reused_handle = reused
            .acknowledgement
            .created
            .iter()
            .find(|handle| handle.slot == box_handle.slot)
            .copied()
            .expect("replacement allocation must reuse the retired slot");
        assert_ne!(reused_handle.generation, box_handle.generation);
        assert!(host.ui_visible_geometry(box_handle).is_err());
        host.close().unwrap();
    }

    #[test]
    fn ui_event_waiter_wakes_for_owned_batch_and_ui_close() {
        let host = TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let waiter_host = host.clone();
        let waiter = runtime.spawn(async move { waiter_host.wait_for_ui_events().await });
        runtime.block_on(tokio::task::yield_now());
        {
            let mut inner = host.inner.lock().unwrap();
            let namespace = inner.ui_resources.namespace.get();
            let event = NativeUiEvent {
                handle: crate::occurrence::UiHandle {
                    host_namespace: namespace,
                    slot: 1,
                    generation: 1,
                    kind: crate::occurrence::HandleKind::Node,
                },
                mask: 2,
                text: Some("owned text".to_owned()),
                cursor_bytes: Some(3),
                key: Some("x".to_owned()),
                revision: Some(7),
            };
            let bytes = event.payload_bytes().unwrap();
            inner.ui_event_bytes = bytes;
            inner.ui_events.push_back(event);
            inner.ui_event_notify.notify_waiters();
        }
        let batch = runtime.block_on(waiter).unwrap().unwrap().unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].text.as_deref(), Some("owned text"));
        assert_eq!(batch[0].key.as_deref(), Some("x"));
        assert_eq!(batch[0].revision, Some(7));

        let close_waiter_host = host.clone();
        let close_waiter =
            runtime.spawn(async move { close_waiter_host.wait_for_ui_events().await });
        runtime.block_on(tokio::task::yield_now());
        host.close_ui_state().unwrap();
        assert!(runtime.block_on(close_waiter).unwrap().unwrap().is_none());
        host.close().unwrap();
    }

    #[test]
    fn native_failure_waiter_reports_distinct_attempts_and_wakes_on_close() {
        let host = TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        host.fail_next_frame_for_test("first native failure")
            .unwrap();
        host.set_test_view(vf::text("failure observer")).unwrap();
        let first_waiter_host = host.clone();
        let first_waiter =
            runtime.spawn(async move { first_waiter_host.wait_for_ui_failure().await });
        runtime.block_on(tokio::task::yield_now());
        let first_report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(first_report.errors.len(), 1);
        let first = runtime.block_on(first_waiter).unwrap().unwrap().unwrap();
        assert_eq!(first.phase, "frame");
        assert_eq!(first.code, "FRAME_PREPARATION_FAILED");
        assert_eq!(first.diagnostic, "first native failure");
        assert!(first.retryable);

        // A second recoverable failure in the same pending work epoch is a
        // distinct notification. The observer must not deduplicate by epoch
        // and silently lose the later diagnostic.
        host.fail_next_frame_for_test("second native failure")
            .unwrap();
        let second_waiter_host = host.clone();
        let second_waiter =
            runtime.spawn(async move { second_waiter_host.wait_for_ui_failure().await });
        runtime.block_on(tokio::task::yield_now());
        let second_report = host.flush_pending_hosts(8, true).unwrap();
        assert_eq!(second_report.errors.len(), 1);
        let second = runtime.block_on(second_waiter).unwrap().unwrap().unwrap();
        assert_eq!(second.phase, "frame");
        assert_eq!(second.code, "FRAME_PREPARATION_FAILED");
        assert_eq!(second.diagnostic, "second native failure");
        assert_eq!(second.attempted_work_epoch, first.attempted_work_epoch);
        assert_ne!(second.diagnostic, first.diagnostic);

        {
            let mut inner = host.lock_mut().unwrap();
            for _ in 0..MAX_FAILURE_NOTIFICATIONS + 2 {
                inner.publish_attempt_failure(
                    "frame",
                    "FRAME_PREPARATION_FAILED",
                    true,
                    second.attempted_ui_revision,
                    second.attempted_work_epoch,
                    "bounded observer failure".to_owned(),
                );
            }
            assert_eq!(inner.failure_notifications.len(), MAX_FAILURE_NOTIFICATIONS);
        }
        let overflow = runtime
            .block_on(host.wait_for_ui_failure())
            .unwrap()
            .unwrap();
        assert_eq!(overflow.code, "LIMIT_EXCEEDED");
        assert!(overflow.diagnostic.contains("2 failure notifications"));
        for _ in 0..MAX_FAILURE_NOTIFICATIONS {
            let retained = runtime
                .block_on(host.wait_for_ui_failure())
                .unwrap()
                .unwrap();
            assert_eq!(retained.diagnostic, "bounded observer failure");
        }

        let close_waiter_host = host.clone();
        let close_waiter =
            runtime.spawn(async move { close_waiter_host.wait_for_ui_failure().await });
        runtime.block_on(tokio::task::yield_now());
        host.close_ui_state().unwrap();
        assert!(runtime.block_on(close_waiter).unwrap().unwrap().is_none());
        host.close().unwrap();
    }

    #[test]
    fn native_ui_adapter_switches_keep_qualified_identity_and_bounded_owners() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(24, 4, true, environment.clone()).unwrap();
        let source_a = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        let source_b = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source_a.append_utf8(b"A", &[], &[]).unwrap();
        source_b.append_utf8(b"B", &[], &[]).unwrap();

        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        create.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 3,
            source_index: 0,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::OccurrenceOwned,
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 4,
            source_index: 1,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::OccurrenceOwned,
        });
        create.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(host.ui_body_handle().unwrap()),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(ResourceRef::Local(2)),
        });
        create.push(UiOperation::SelectConnector {
            port: ResourceRef::Local(2),
            connector: Some(ResourceRef::Local(3)),
        });
        let created = host
            .commit_ui(create, &[source_a.clone(), source_b.clone()])
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let node = created
            .acknowledgement
            .created
            .iter()
            .find(|handle| handle.kind == crate::occurrence::HandleKind::Node)
            .copied()
            .expect("content occurrence acknowledgement");
        let port = created
            .acknowledgement
            .created
            .iter()
            .find(|handle| handle.kind == crate::occurrence::HandleKind::Port)
            .copied()
            .expect("content Port acknowledgement");
        let connectors = created
            .acknowledgement
            .created
            .iter()
            .filter(|handle| handle.kind == crate::occurrence::HandleKind::Connector)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(connectors.len(), 2);
        let connector_a = connectors[0];
        let connector_b = connectors[1];
        let port_key = port.resource_key().unwrap();
        let connector_a_key = connector_a.resource_key().unwrap();
        let connector_b_key = connector_b.resource_key().unwrap();

        {
            let inner = host.inner.lock().unwrap();
            assert_eq!(inner.content.test_ui_adapter_count(), 1);
            assert_eq!(
                inner
                    .content
                    .ui_connector_status(&inner.ui_resources, connector_a_key)
                    .unwrap()
                    .visible,
                true
            );
            assert_eq!(source_a.subscriber_count(), 1);
            assert_eq!(source_b.subscriber_count(), 0);
        }

        let mut select_b = UiCommit::new(1);
        select_b.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: Some(ResourceRef::Existing(connector_b)),
        });
        host.commit_ui(select_b, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        {
            let inner = host.inner.lock().unwrap();
            assert_eq!(inner.content.test_ui_adapter_count(), 1);
            let status = inner
                .content
                .ui_connector_status(&inner.ui_resources, connector_b_key)
                .unwrap();
            assert!(status.requested);
            assert!(status.visible);
            assert_eq!(
                inner
                    .content
                    .test_ui_confirmed_connector(port_key)
                    .unwrap()
                    .0,
                connector_b_key
            );
        }

        // A failed switch preserves B's confirmed product while retaining at
        // most one current failed adapter. The identity comparison is
        // qualified by HandleKind, so a Port key can never be promoted as a
        // Connector key.
        host.fail_next_ui_connector_for_test("failed A".to_owned())
            .unwrap();
        let mut select_a = UiCommit::new(2);
        select_a.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: Some(ResourceRef::Existing(connector_a)),
        });
        host.commit_ui(select_a, &[]).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let failure_waiter_host = host.clone();
        let failure_waiter =
            runtime.spawn(async move { failure_waiter_host.wait_for_ui_failure().await });
        runtime.block_on(tokio::task::yield_now());
        let failed_report = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(failed_report.errors.len(), 1);
        assert_eq!(failed_report.errors[0].code, "PROJECTION_FAILED");
        let failure = runtime.block_on(failure_waiter).unwrap().unwrap().unwrap();
        assert_eq!(failure.phase, "content");
        assert_eq!(failure.code, "PROJECTION_FAILED");
        assert_eq!(
            failure.attempted_ui_revision,
            failed_report.errors[0].desired_revision
        );
        {
            let inner = host.inner.lock().unwrap();
            let failed = inner
                .content
                .ui_connector_status(&inner.ui_resources, connector_a_key)
                .unwrap();
            assert!(failed.requested);
            assert!(!failed.visible);
            assert_eq!(
                failed.error.as_ref().map(|error| error.code.as_str()),
                Some("PROJECTION_FAILED")
            );
            let confirmed = inner
                .content
                .ui_connector_status(&inner.ui_resources, connector_b_key)
                .unwrap();
            assert!(confirmed.visible);
            assert_eq!(inner.content.test_ui_adapter_count(), 2);
        }
        source_a.append_utf8(b" recovered", &[], &[]).unwrap();
        let recovery_report = host.flush_pending_hosts(8, true).unwrap();
        assert!(recovery_report.errors.is_empty());
        host.flush_pending_hosts(8, true).unwrap();
        {
            let inner = host.inner.lock().unwrap();
            assert_eq!(inner.content.test_ui_adapter_count(), 1);
            let recovered = inner
                .content
                .ui_connector_status(&inner.ui_resources, connector_a_key)
                .unwrap();
            assert!(recovered.visible);
            assert_eq!(
                inner
                    .content
                    .test_ui_confirmed_connector(port_key)
                    .unwrap()
                    .0,
                connector_a_key
            );
        }

        // Successful A/B/A churn must retire each superseded execution
        // adapter at its receipt instead of growing the host registry.
        let mut switch_b_again = UiCommit::new(3);
        switch_b_again.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: Some(ResourceRef::Existing(connector_b)),
        });
        host.commit_ui(switch_b_again, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let mut switch_a_again = UiCommit::new(4);
        switch_a_again.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: Some(ResourceRef::Existing(connector_a)),
        });
        host.commit_ui(switch_a_again, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        assert_eq!(
            host.inner.lock().unwrap().content.test_ui_adapter_count(),
            1
        );
        assert!(source_a.dispose().is_err());
        assert!(source_b.dispose().is_err());

        // Retiring the occurrence unmounts the Port, releases both accepted
        // Source memberships and removes the final derived adapter/Port.
        let mut retire = UiCommit::new(5);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(node),
        });
        host.commit_ui(retire, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        {
            let inner = host.inner.lock().unwrap();
            assert_eq!(inner.content.test_ui_adapter_count(), 0);
            assert!(
                inner
                    .content
                    .test_ui_confirmed_connector(port_key)
                    .is_none()
            );
            assert_eq!(inner.content.test_port_count(), 0);
            assert_eq!(inner.content.test_connector_count(), 0);
        }
        assert_eq!(source_a.subscriber_count(), 0);
        assert_eq!(source_b.subscriber_count(), 0);
        host.close().unwrap();
        source_a.dispose().unwrap();
        source_b.dispose().unwrap();
    }

    #[test]
    fn failed_auto_preparation_explicit_barrier_waits_for_a_new_attempt() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        host.fail_next_frame_for_test("first preparation fails")
            .unwrap();
        host.set_test_view(vf::text("retry-success")).unwrap();
        let target = host.epochs().unwrap().desired_structural_revision;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(host.wait_for_ui_presentation(target, false))
            .unwrap();
        assert!(host.inner.lock().unwrap().attempt_revision >= 2);
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("retry-success"))
        );
        host.close().unwrap();
    }

    #[test]
    fn failed_presentation_marks_physical_sync_unknown_until_recovery_frame() {
        let host = TuiHost::open(20, 4, true).unwrap();
        host.set_test_view(vf::text("receipt-failure")).unwrap();
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
                super::prepare_frame(
                    running,
                    backend
                        .as_mut()
                        .expect("test host must own its terminal backend"),
                    *now,
                )
                .unwrap()
            };
            inner.install_test_in_flight(candidate, receiver).unwrap();
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
    fn receipt_completion_between_poll_and_sleep_is_not_lost() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let sender = install_delayed_candidate(&host);
        let (waiting_tx, waiting_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        host.inner.lock().unwrap().waiting_for_presentation_hook = Some((waiting_tx, release_rx));
        host.set_test_view(vf::text("race")).unwrap();

        let drain_environment = environment.clone();
        let drain = std::thread::spawn(move || drain_environment.drain_pending(8, false));
        waiting_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("drain must poll the receipt before sleeping bookkeeping");
        sender.send(Ok(())).unwrap();
        release_tx.send(()).unwrap();
        let waiting = drain.join().unwrap().unwrap();
        assert!(waiting.waiting_for_presentation);

        let committed = environment.drain_pending(8, false).unwrap();
        assert_eq!(committed.errors, []);
        assert_eq!(committed.commits.len(), 1);
        host.close().unwrap();
    }

    #[test]
    fn close_gates_ingress_joins_callers_and_waits_for_physical_receipt() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let sender = install_delayed_candidate(&host);
        host.set_test_view(vf::text("pending-close")).unwrap();
        let target = host.epochs().unwrap().desired_structural_revision;
        let body_handle = host.ui_body_handle().unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let waiter_host = host.clone();
        let mut waiter = Box::pin(waiter_host.wait_for_ui_presentation(target, false));
        runtime.block_on(poll_fn(|context| match waiter.as_mut().poll(context) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(result) => panic!("visibility waiter settled before close: {result:?}"),
        }));

        let (close_started_tx, close_started_rx) = mpsc::channel();
        host.inner.lock().unwrap().close_started_hook = Some(close_started_tx);
        let first_host = host.clone();
        let (first_done_tx, first_done_rx) = mpsc::channel();
        let first = std::thread::spawn(move || {
            first_done_tx.send(first_host.close()).unwrap();
        });
        close_started_rx
            .recv_timeout(Duration::from_secs(2))
            .map(|phase| assert!(matches!(phase, ClosePhase::Started)))
            .expect("close must publish its ingress gate before waiting");

        // The sibling is created and serviced while the first close still
        // waits for its receipt. The shared environment remains usable.
        let peer = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        peer.set_test_view(vf::text("unrelated-peer")).unwrap();
        let peer_report = environment.drain_pending(8, false).unwrap();
        assert!(peer_report.errors.is_empty());
        assert!(
            peer.screen_rows()
                .iter()
                .any(|row| row.contains("unrelated-peer"))
        );

        let second_host = host.clone();
        let (second_done_tx, second_done_rx) = mpsc::channel();
        let second = std::thread::spawn(move || {
            second_done_tx.send(second_host.close()).unwrap();
        });
        close_started_rx
            .recv_timeout(Duration::from_secs(2))
            .map(|phase| assert!(matches!(phase, ClosePhase::Joined)))
            .expect("second close caller must join while first close is pending");
        assert!(
            host.set_test_view(vf::text("rejected-after-close"))
                .is_err(),
            "new host commands must be rejected during close"
        );
        let mut ui_batch = UiCommit::new(0);
        ui_batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        ui_batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body_handle),
            child: NodeRef::Local(1),
            before: None,
        });
        assert!(
            host.commit_ui(ui_batch, &[]).is_err(),
            "UI occurrence commits must be rejected after close admission"
        );
        let waiter_result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), &mut waiter)
                .await
                .expect("close must wake the pending visibility waiter")
        });
        assert!(
            waiter_result.is_err(),
            "pending visibility waiters must reject on close"
        );
        assert!(
            first_done_rx
                .recv_timeout(Duration::from_millis(20))
                .is_err(),
            "close must retain the receipt-owned frame until the sender resolves"
        );
        assert!(
            second_done_rx
                .recv_timeout(Duration::from_millis(20))
                .is_err(),
            "a joining close must retain the shared outcome until the sender resolves"
        );

        sender.send(Ok(())).unwrap();
        assert!(
            first_done_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        assert!(
            second_done_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        second.join().unwrap();
        first.join().unwrap();
        peer.close().unwrap();
    }

    #[test]
    fn history_transfer_submits_captured_rows_outside_host_acceptance_guard() {
        let host = TuiHost::open_in_environment(20, 2, true, TuiEnvironment::new_manual()).unwrap();
        host.push_history_unit_for_test(vf::text("one")).unwrap();
        host.push_history_unit_for_test(vf::text("two")).unwrap();
        host.push_history_unit_for_test(vf::text("three")).unwrap();

        let rows = host.native_history_rows();
        assert_eq!(rows.iter().filter(|row| *row == "one").count(), 1);
        assert!(!rows.iter().any(|row| row == "two"));
        assert!(host.screen_rows().iter().any(|row| row.contains("three")));
        host.close().unwrap();
    }

    #[test]
    fn blocked_history_receipt_does_not_hold_host_acceptance_guard() {
        let host = TuiHost::open_in_environment(20, 2, true, TuiEnvironment::new_manual()).unwrap();
        set_test_history(
            &host,
            &[vf::text("one"), vf::text("two"), vf::text("three")],
        )
        .unwrap();

        host.set_test_view(vf::text("initial")).unwrap();

        let (sender, receiver) = oneshot::channel::<anyhow::Result<usize>>();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.install_test_history_receipt(receiver);
            let outcome = inner.flush_for_environment(false, true).unwrap();
            assert!(outcome.0.waiting_for_physical_work);
        }
        super::HostInner::start_history_work(&host.inner).unwrap();

        // A desired mutation and a confirmed-frame query can proceed while
        // the captured physical receipt remains unresolved.
        let before = host.epochs().unwrap().desired_structural_revision;
        host.set_test_view(vf::text("newer")).unwrap();
        assert!(host.epochs().unwrap().desired_structural_revision > before);
        assert_eq!(host.screen_rows().len(), 2);

        let close_host = host.clone();
        let close = std::thread::spawn(move || close_host.close());
        std::thread::sleep(Duration::from_millis(10));
        assert!(!close.is_finished());
        sender.send(Ok(1)).unwrap();
        assert!(close.join().unwrap().is_ok());
    }

    #[test]
    fn failed_history_receipt_keeps_confirmed_prefix_and_blocks_suffix_replay() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 2, true, environment.clone()).unwrap();
        set_test_history(
            &host,
            &[vf::text("one"), vf::text("two"), vf::text("three")],
        )
        .unwrap();

        host.set_test_view(vf::text("screen")).unwrap();

        let (sender, receiver) = oneshot::channel::<anyhow::Result<usize>>();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.install_test_history_receipt(receiver);
            let outcome = inner.flush_for_environment(false, true).unwrap();
            assert!(outcome.0.waiting_for_physical_work);
        }
        super::HostInner::start_history_work(&host.inner).unwrap();
        sender
            .send(Err(anyhow::anyhow!("simulated History receipt failure")))
            .unwrap();

        let host_id = host.inner.lock().unwrap().host_id;
        let report = environment
            .drain_pending_for(8, false, Some(host_id))
            .unwrap();
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code, "HISTORY_TRANSFER_FAILED");
        let before_retry = host.native_history_rows();
        {
            let inner = host.inner.lock().unwrap();
            assert!(inner.physical_sync_unknown);
            assert!(
                inner
                    .running
                    .scene_history()
                    .is_some_and(crate::History::native_synchronization_unknown)
            );
        }

        // A recovery frame may restore the screen, but it cannot prove which
        // suffix the failed native receipt accepted. It must therefore not
        // submit that suffix a second time.
        host.flush_pending_hosts(8, true).unwrap();
        assert_eq!(host.native_history_rows(), before_retry);
        assert!(
            host.inner
                .lock()
                .unwrap()
                .running
                .scene_history()
                .is_some_and(crate::History::native_synchronization_unknown)
        );
        host.close().unwrap();
    }

    #[test]
    fn history_work_signal_handles_completion_before_condvar_wait() {
        let signal = Arc::new(super::HistoryWorkSignal::new());
        let (registered_tx, registered_rx) = mpsc::channel();
        let (before_wait_tx, before_wait_rx) = mpsc::channel();
        signal.install_before_wait_hook(before_wait_tx);

        let waiter_signal = Arc::clone(&signal);
        let waiter = std::thread::spawn(move || {
            let (guard, generation) = waiter_signal.register().unwrap();
            registered_tx.send(()).unwrap();
            waiter_signal.wait_for_change(guard, generation)
        });

        registered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter must register before inspecting the predicate");
        before_wait_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter must hold the registered guard before Condvar::wait");

        // The notifier runs on a second thread and must acquire the same
        // generation mutex. It therefore cannot publish completion until the
        // waiter atomically releases that mutex while entering Condvar::wait.
        let notifier_signal = Arc::clone(&signal);
        let notifier = std::thread::spawn(move || notifier_signal.notify());
        notifier.join().unwrap().unwrap();
        waiter.join().unwrap().unwrap();
    }

    #[test]
    fn inline_exit_settles_final_history_plan_before_positioning() {
        let host = TuiHost::open_in_environment(20, 2, true, TuiEnvironment::new_manual()).unwrap();
        set_test_history(
            &host,
            &[vf::text("one"), vf::text("two"), vf::text("three")],
        )
        .unwrap();

        host.set_test_view(vf::text("final")).unwrap();
        host.exit().unwrap();

        let rows = host.native_history_rows();
        assert_eq!(rows.iter().filter(|row| *row == "one").count(), 1);
        assert!(rows.iter().any(|row| row == "final"));
    }

    #[test]
    fn inline_exit_zero_progress_history_receipt_does_not_resubmit() {
        let host = TuiHost::open_in_environment(20, 2, true, TuiEnvironment::new_manual()).unwrap();
        set_test_history(
            &host,
            &[vf::text("one"), vf::text("two"), vf::text("three")],
        )
        .unwrap();

        host.set_test_view(vf::text("final")).unwrap();

        let (sender, receiver) = oneshot::channel::<anyhow::Result<usize>>();
        host.inner
            .lock()
            .unwrap()
            .install_test_history_receipt(receiver);
        let exit_host = host.clone();
        let exit = std::thread::spawn(move || exit_host.exit());
        sender.send(Ok(0)).unwrap();
        assert!(exit.join().unwrap().is_ok());

        let rows = host.native_history_rows();
        assert_eq!(rows.iter().filter(|row| *row == "one").count(), 1);
        assert!(!rows.iter().any(|row| row == "two"));
        assert!(rows.iter().any(|row| row == "final"));
    }

    #[test]
    fn backend_fault_close_reuses_teardown_and_closes_accepted_ui_resources() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let body = accept_ui_box(&host);
        let retained_inner = Arc::clone(&host.inner);

        // This seam enters the same Faulted lifecycle used when the terminal
        // worker reports BACKEND_NOT_READY. Faulted is not a completed close:
        // the real close owner must still run the shared teardown plan.
        host.mark_backend_stopped_for_test().unwrap();
        assert!(host.close().is_ok());
        assert!(host.ui_body_handle().is_err());
        assert!(
            retained_inner
                .lock()
                .unwrap()
                .ui_resources
                .document
                .is_none()
        );
        assert!(
            host.close().is_ok(),
            "subsequent close observes the stored outcome"
        );
        let _ = body;
        drop(retained_inner);
    }

    #[test]
    fn inline_exit_uses_close_plan_and_releases_ui_resources_with_extra_owner() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let sender = install_delayed_candidate(&host);
        let (body, desired_revision) = accept_ui_literal(&host, b"final-output");
        let retained_inner = Arc::clone(&host.inner);

        let exit_host = host.clone();
        let (exit_done_tx, exit_done_rx) = mpsc::channel();
        let exit = std::thread::spawn(move || {
            exit_done_tx.send(exit_host.exit()).unwrap();
        });
        assert!(
            exit_done_rx
                .recv_timeout(Duration::from_millis(20))
                .is_err(),
            "inline exit must settle the older delayed receipt first"
        );
        sender.send(Ok(())).unwrap();
        assert!(
            exit_done_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        exit.join().unwrap();
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("final-output"))
        );
        assert_eq!(
            host.epochs().unwrap().visible_structural_revision,
            desired_revision
        );
        assert!(
            host.native_history_rows()
                .iter()
                .any(|row| row.contains("final-output")),
            "headless final rows must remain readable after backend detachment"
        );
        assert!(host.ui_body_handle().is_err());
        assert!(
            retained_inner
                .lock()
                .unwrap()
                .ui_resources
                .document
                .is_none()
        );
        assert!(
            host.close().is_ok(),
            "close must observe the completed exit"
        );
        let _ = body;
        drop(retained_inner);
    }

    #[test]
    fn inline_exit_backend_result_error_does_not_promote_final_candidate() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        host.set_test_view(vf::text("confirmed")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let before = host.epochs().unwrap();
        let before_rows = host.screen_rows();
        host.set_test_view(vf::text("final-failure")).unwrap();
        let (sender, receiver) = oneshot::channel();
        host.inner
            .lock()
            .unwrap()
            .install_test_final_receipt(receiver);

        let exit_host = host.clone();
        let exit = std::thread::spawn(move || exit_host.exit());
        sender
            .send(Err(anyhow::anyhow!("simulated final frame failure")))
            .unwrap();
        let result = exit.join().unwrap();
        assert!(
            result.is_err(),
            "backend result errors must fail inline exit"
        );
        let after = host.epochs().unwrap();
        assert_eq!(after.visible_frame_revision, before.visible_frame_revision);
        assert_eq!(host.screen_rows(), before_rows);
        assert!(host.ui_body_handle().is_err());
        let repeated_close = host.close();
        assert!(repeated_close.is_err());
        assert!(
            repeated_close
                .unwrap_err()
                .to_string()
                .contains("simulated final frame failure")
        );
    }

    #[test]
    fn sparse_control_sync_keeps_unrelated_control_identity_and_cursor() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(32, 8, true, environment).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Editor,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 2,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Editor,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 4,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(3)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 5,
            kind: HostKind::Box,
        });
        for child in [1, 3, 5] {
            mount.push(UiOperation::InsertBefore {
                parent: NodeRef::Existing(body),
                child: NodeRef::Local(child),
                before: None,
            });
        }
        mount.push(UiOperation::ReplaceEditorContent {
            control: crate::occurrence::ResourceRef::Local(2),
            content: b"a".to_vec(),
            expected_edit_revision: u64::MAX,
        });
        mount.push(UiOperation::ReplaceEditorContent {
            control: crate::occurrence::ResourceRef::Local(4),
            content: b"b".to_vec(),
            expected_edit_revision: u64::MAX,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let first_control = mounted.acknowledgement.created[1].resource_key().unwrap();
        let second_node = mounted.acknowledgement.created[2];
        let second_control = mounted.acknowledgement.created[3].resource_key().unwrap();
        let second_input = host
            .inner
            .lock()
            .unwrap()
            .ui_editors
            .get(&second_control)
            .unwrap()
            .clone();
        let second_component_id = second_input.component_id().unwrap();
        let before_control_visits = host.inner.lock().unwrap().test_ui_control_keys_visited();

        host.focus_ui(second_node).unwrap();
        host.dispatch_key(KeyStroke::new(Key::Char('z'))).unwrap();
        assert_eq!(second_input.text().unwrap(), "bz");
        assert_eq!(second_input.cursor_bytes().unwrap(), 2);

        let mut update = UiCommit::new(1);
        update.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(mounted.acknowledgement.created[4]),
            property: crate::occurrence::PropertyId::Background,
            value: crate::occurrence::LayerValue::Value(crate::occurrence::PropertyValue::Color(
                crate::occurrence::ColorValue::ansi(2),
            )),
        });
        update.push(UiOperation::ControlCommand {
            control: crate::occurrence::ResourceRef::Existing(mounted.acknowledgement.created[1]),
            command_id: 1,
            operands: vec!['q' as u32],
        });
        host.commit_ui(update, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let after_control_visits = host.inner.lock().unwrap().test_ui_control_keys_visited();
        assert_eq!(
            after_control_visits.saturating_sub(before_control_visits),
            1,
            "a one-control update must not inspect the unrelated control registry"
        );

        let inner = host.inner.lock().unwrap();
        assert_eq!(
            inner
                .ui_editors
                .get(&second_control)
                .and_then(HostTextInput::component_id),
            Some(second_component_id),
            "unrelated control must retain its derived component identity"
        );
        drop(inner);
        assert_eq!(second_input.text().unwrap(), "bz");
        assert_eq!(second_input.cursor_bytes().unwrap(), 2);

        // The untouched control is still focused and usable after a sparse
        // update to the first control and an unrelated Box property.
        host.dispatch_key(KeyStroke::new(Key::Char('y'))).unwrap();
        assert_eq!(second_input.text().unwrap(), "bzy");
        assert_eq!(second_input.cursor_bytes().unwrap(), 3);
        let first_text = host
            .inner
            .lock()
            .unwrap()
            .ui_editors
            .get(&first_control)
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(first_text, "aq");
        host.close().unwrap();
    }

    #[test]
    fn native_animation_progress_survives_unrelated_input_frames() {
        let host = TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Animation,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 2,
            kind: crate::occurrence::ControlKind::Animation,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        for local_ordinal in [3, 4] {
            mount.push(UiOperation::CreateNode {
                local_ordinal,
                kind: HostKind::Box,
            });
            mount.push(UiOperation::InsertBefore {
                parent: NodeRef::Local(1),
                child: NodeRef::Local(local_ordinal),
                before: None,
            });
        }
        mount.push(UiOperation::CreateNode {
            local_ordinal: 5,
            kind: HostKind::Editor,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 6,
            kind: crate::occurrence::ControlKind::Editor,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(5)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(5),
            before: None,
        });
        mount.push(UiOperation::ReplaceEditorContent {
            control: ResourceRef::Local(6),
            content: b"seed".to_vec(),
            expected_edit_revision: u64::MAX,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();

        let animation_control = mounted.acknowledgement.created[1].resource_key().unwrap();
        let editor_node = mounted.acknowledgement.created[4];
        let (before_control_visits, before_active_frame) = {
            let inner = host.inner.lock().unwrap();
            (
                inner.test_ui_control_keys_visited(),
                inner
                    .ui_resources
                    .control_state(animation_control)
                    .and_then(crate::occurrence::ControlState::animation_active_frame),
            )
        };
        assert_eq!(before_active_frame, Some(0));

        host.focus_ui(editor_node).unwrap();
        for _ in 0..40 {
            host.dispatch_key(KeyStroke::new(Key::Char('x'))).unwrap();
            host.advance_time(Duration::from_millis(1)).unwrap();
        }

        let inner = host.inner.lock().unwrap();
        assert_eq!(
            inner
                .ui_resources
                .control_state(animation_control)
                .and_then(crate::occurrence::ControlState::animation_active_frame),
            Some(1),
            "the native animation must advance while unrelated input keeps producing frames"
        );
        assert_eq!(
            inner
                .ui_animations
                .get(&animation_control)
                .unwrap()
                .frame_index()
                .unwrap(),
            1
        );
        assert_eq!(
            inner.test_ui_control_keys_visited(),
            before_control_visits,
            "unchanged UI revisions must not rescan native controls"
        );
        let editor = inner.ui_editors.values().next().expect("mounted editor");
        assert_eq!(editor.text().unwrap(), format!("seed{}", "x".repeat(40)));
        drop(inner);

        // Capture an older native frame before accepting the stop command.
        // Its unresolved receipt must be reconciled before newer UI work is
        // allowed to mutate the scene or run another animation tick.
        let receipt = install_delayed_candidate(&host);

        let mut stop = UiCommit::new(mounted.acknowledgement.accepted_ui_revision);
        stop.push(UiOperation::ControlCommand {
            control: ResourceRef::Existing(mounted.acknowledgement.created[1]),
            command_id: 512,
            operands: Vec::new(),
        });
        let stopped = host.commit_ui(stop, &[]).unwrap();

        let waiting = host.flush_pending_hosts(8, false).unwrap();
        assert!(waiting.waiting_for_presentation);
        assert_eq!(
            host.inner
                .lock()
                .unwrap()
                .ui_resources
                .control_state(animation_control)
                .and_then(crate::occurrence::ControlState::animation_active_frame),
            Some(1),
            "an unresolved older receipt must not advance the animation"
        );

        receipt.send(Ok(())).unwrap();
        let committed = host.flush_pending_hosts(8, false).unwrap();
        assert!(
            committed
                .commits
                .iter()
                .any(|commit| { commit.host_id == host.epochs().unwrap().host_id })
        );
        host.advance_time(Duration::from_millis(20)).unwrap();
        host.advance_time(Duration::from_millis(20)).unwrap();
        assert_eq!(
            host.inner
                .lock()
                .unwrap()
                .ui_resources
                .control_state(animation_control)
                .and_then(crate::occurrence::ControlState::animation_active_frame),
            Some(1),
            "a stopped animation must remain stopped on later ticks"
        );

        let mut retire = UiCommit::new(stopped.acknowledgement.accepted_ui_revision);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(mounted.acknowledgement.created[0]),
        });
        host.commit_ui(retire, &[]).unwrap();
        host.advance_time(Duration::from_millis(20)).unwrap();
        assert!(
            !host
                .inner
                .lock()
                .unwrap()
                .ui_animations
                .contains_key(&animation_control)
        );
        host.close().unwrap();
    }

    #[test]
    fn rejected_ui_commit_does_not_publish_a_pending_projection_frontier() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(24, 6, true, environment).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let node = mounted.acknowledgement.created[0];
        let visible = host.epochs().unwrap();

        let mut noop = UiCommit::new(1);
        noop.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 0,
            mask_high: 0,
        });
        let noop_result = host.commit_ui(noop, &[]).unwrap();
        assert_eq!(noop_result.acknowledgement.accepted_ui_revision, 1);
        assert_eq!(host.epochs().unwrap(), visible);
        assert!(host.inner.lock().unwrap().pending_ui_changes.is_none());

        let mut rejected = UiCommit::new(0);
        rejected.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        let rejection = host.commit_ui(rejected, &[]).unwrap_err();
        assert_eq!(
            rejection.detail,
            crate::occurrence::CommitDetail::StaleRevision
        );
        assert_eq!(host.epochs().unwrap(), visible);

        // Leave a legitimate metadata frontier unpresented, reject another
        // stale request, then accept a second metadata update. The rejected
        // requests must not manufacture a revision-zero pending summary or
        // clear the real frontier.
        let mut first_metadata = UiCommit::new(1);
        first_metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(first_metadata, &[]).unwrap();
        let pending = host.epochs().unwrap();
        assert_ne!(pending.pending_epoch, pending.committed_epoch);

        let mut rejected_while_pending = UiCommit::new(0);
        rejected_while_pending.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 3,
            mask_high: 0,
        });
        assert_eq!(
            host.commit_ui(rejected_while_pending, &[])
                .unwrap_err()
                .detail,
            crate::occurrence::CommitDetail::StaleRevision
        );
        let mut second_metadata = UiCommit::new(2);
        second_metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 3,
            mask_high: 0,
        });
        host.commit_ui(second_metadata, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let settled = host.epochs().unwrap();
        assert!(settled.visible_structural_revision >= 2);
        assert_eq!(
            settled.visible_frame_revision,
            visible.visible_frame_revision
        );
        assert_eq!(settled.pending_epoch, settled.committed_epoch);
        host.close().unwrap();
    }

    #[test]
    fn metadata_waits_for_older_receipt_then_promotes_without_layout() {
        let host = TuiHost::open_in_environment(24, 6, true, TuiEnvironment::new_manual()).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let node = mounted.acknowledgement.created[0];

        let sender = install_delayed_candidate(&host);
        host.inner.lock().unwrap().mark_pending().unwrap();
        let mut metadata = UiCommit::new(1);
        metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(metadata, &[]).unwrap();
        sender.send(Ok(())).unwrap();
        let old = host.flush_pending_hosts(8, false).unwrap();
        assert!(
            old.commits
                .iter()
                .any(|commit| { commit.host_id == host.epochs().unwrap().host_id })
        );
        let old_visible = host.epochs().unwrap();

        crate::presentation::layout::reset_layout_counters();
        host.flush_pending_hosts(8, true).unwrap();
        let metadata_visible = host.epochs().unwrap();
        assert!(
            metadata_visible.visible_structural_revision > old_visible.visible_structural_revision
        );
        assert_eq!(
            metadata_visible.visible_frame_revision,
            old_visible.visible_frame_revision
        );
        assert_eq!(
            metadata_visible.pending_epoch,
            metadata_visible.committed_epoch
        );
        assert_eq!(crate::presentation::layout::layout_counters(), (0, 0, 0));
        host.close().unwrap();
    }

    #[test]
    fn metadata_frontier_yields_to_newer_literal_source_work() {
        let host = TuiHost::open_in_environment(24, 6, true, TuiEnvironment::new_manual()).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: crate::occurrence::OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(crate::occurrence::ResourceRef::Local(2)),
        });
        mount.push(UiOperation::ReplaceLiteral {
            port: crate::occurrence::ResourceRef::Local(2),
            content_format: 1,
            content: b"old-source".to_vec(),
            annotations: Vec::new(),
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let node = mounted.acknowledgement.created[0];
        let port = mounted.acknowledgement.created[1];
        let before = host.epochs().unwrap();

        let mut metadata = UiCommit::new(1);
        metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(metadata, &[]).unwrap();
        let mut source_work = UiCommit::new(2);
        source_work.push(UiOperation::ReplaceLiteral {
            port: crate::occurrence::ResourceRef::Existing(port),
            content_format: 1,
            content: b"new-source".to_vec(),
            annotations: Vec::new(),
        });
        host.commit_ui(source_work, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let after = host.epochs().unwrap();
        assert!(after.visible_frame_revision > before.visible_frame_revision);
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("new-source"))
        );
        assert!(
            !host
                .screen_rows()
                .iter()
                .any(|row| row.contains("old-source"))
        );
        host.close().unwrap();
    }

    #[test]
    fn unknown_physical_sync_bypasses_metadata_shortcut() {
        let host = TuiHost::open_in_environment(24, 6, true, TuiEnvironment::new_manual()).unwrap();
        let body = host.ui_body_handle().unwrap();
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        let mounted = host.commit_ui(mount, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let node = mounted.acknowledgement.created[0];
        let before = host.epochs().unwrap();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.physical_sync_unknown = true;
        }
        crate::presentation::layout::reset_layout_counters();
        let mut metadata = UiCommit::new(1);
        metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(metadata, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let after = host.epochs().unwrap();
        assert!(after.visible_frame_revision > before.visible_frame_revision);
        assert!(crate::presentation::layout::layout_counters().0 > 0);
        host.close().unwrap();
    }

    #[test]
    fn final_public_host_owners_close_once_while_inner_has_a_transient_owner() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let retained_inner = Arc::clone(&host.inner);
        let first_host = host.clone();
        let second_host = host.clone();
        let barrier = Arc::new(Barrier::new(3));
        let first_barrier = Arc::clone(&barrier);
        let first = std::thread::spawn(move || {
            first_barrier.wait();
            drop(first_host);
        });
        let second_barrier = Arc::clone(&barrier);
        let second = std::thread::spawn(move || {
            second_barrier.wait();
            drop(second_host);
        });
        barrier.wait();
        drop(host);
        first.join().unwrap();
        second.join().unwrap();
        assert!(retained_inner.lock().unwrap().is_closed());
        drop(retained_inner);
    }

    #[test]
    fn occurrence_history_partial_output_failure_preserves_confirmed_prefix_without_replay() {
        let environment = TuiEnvironment::new_manual();
        let host = TuiHost::open_in_environment(20, 2, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(super::super::content::TextSourceKind::Stream)
            .unwrap();
        source.append_utf8(b"one\ntwo\nthree", &[], &[]).unwrap();
        source.seal().unwrap();

        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: crate::occurrence::RootRole::LegacyHistoryUnit,
            owner: None,
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 3,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(2)),
        });
        mount.push(UiOperation::CreateConnector {
            local_ordinal: 4,
            source_index: 0,
            port: ResourceRef::Local(3),
            ownership: OwnershipMode::OccurrenceOwned,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(2),
            port: Some(ResourceRef::Local(3)),
        });
        mount.push(UiOperation::SelectConnector {
            port: ResourceRef::Local(3),
            connector: Some(ResourceRef::Local(4)),
        });
        host.commit_ui(mount, std::slice::from_ref(&source))
            .unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let history_root = {
            let inner = host.inner.lock().unwrap();
            inner
                .ui_resources
                .history_roots()
                .first()
                .copied()
                .expect("accepted History root")
                .handle(inner.ui_resources.namespace)
        };
        let mut freeze = UiCommit::new(1);
        freeze.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(history_root),
            action_id: 1,
        });
        host.commit_ui(freeze, &[]).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let (sender, receiver) = oneshot::channel::<Result<(), anyhow::Error>>();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.sync_ui_scene().unwrap();
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
                    backend
                        .as_mut()
                        .expect("test host must own its terminal backend"),
                    *now,
                    content,
                )
                .unwrap()
            };
            let history_prefix = inner
                .backend
                .as_ref()
                .and_then(|backend| match backend {
                    super::HostBackend::Headless(sink) => sink.history.first(),
                    super::HostBackend::Real(_) => None,
                })
                .map(PhysicalRow::plain_text);
            assert_eq!(history_prefix.as_deref(), Some("one"));
            inner.install_test_in_flight(candidate, receiver).unwrap();
            inner.mark_pending().unwrap();
        }

        sender
            .send(Err(anyhow::anyhow!("simulated partial screen output")))
            .unwrap();
        let failed = host.flush_pending_hosts(8, false).unwrap();
        assert_eq!(failed.errors.len(), 1);
        assert_eq!(failed.errors[0].code, "BACKEND_IO_FAILED");
        assert_eq!(
            host.native_history_rows()
                .iter()
                .filter(|row| *row == "one")
                .count(),
            1
        );

        host.flush_pending_hosts(8, true).unwrap();
        let recovered_rows = host.native_history_rows();
        assert_eq!(recovered_rows.iter().filter(|row| *row == "one").count(), 1);
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn failed_bootstrap_receipt_reports_backend_error_without_candidate_state() {
        let host = TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).unwrap();
        let (sender, receiver) = oneshot::channel::<Result<(), anyhow::Error>>();
        {
            let mut inner = host.inner.lock().unwrap();
            // Model a real backend bootstrap receipt: the initial frame is
            // submitted directly, so no candidate frame or state commit is
            // installed before the receipt is observed.
            inner.install_test_bootstrap_receipt(receiver);
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
        assert!(matches!(
            host.inner.lock().unwrap().presentation_state,
            PresentationState::Idle | PresentationState::Failed(_)
        ));

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
    fn delayed_content_receipt_preserves_newer_source_work_for_next_candidate() {
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(vf::content_host(port.id()).unwrap())
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
                    backend
                        .as_mut()
                        .expect("test host must own its terminal backend"),
                    *now,
                    content,
                )
                .unwrap()
            };
            inner.install_test_in_flight(candidate, receiver).unwrap();
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
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(crate::presentation::factory::row_specs(
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
        peer.set_test_view(vf::text("peer")).unwrap();
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
        let environment = TuiEnvironment::new_manual();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        first.set_test_view(vf::text("first")).unwrap();
        second.set_test_view(vf::text("second")).unwrap();

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
        let environment = TuiEnvironment::new_manual();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        first.set_test_view(vf::text("poisoned")).unwrap();
        second.set_test_view(vf::text("healthy")).unwrap();
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

        let environment = TuiEnvironment::new_manual();
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
            host.set_test_view(vf::content_host(port.id()).unwrap())
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
        host.set_test_view(vf::text("settled")).unwrap();
        host.flush_pending_hosts(8, true).unwrap();
        let settled_rows = host.screen_rows();
        {
            let mut inner = host.inner.lock().unwrap();
            inner.desired_structural_revision = u64::MAX;
        }

        let result = host.set_test_view(vf::text("never"));
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
    fn source_wake_repaints_only_the_affected_content_port() {
        #[cfg(feature = "perf-counters")]
        let _perf_lock = crate::perf::test_lock();
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(vf::content_host(port.id()).unwrap())
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
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(crate::presentation::factory::fill_width(
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
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(crate::presentation::factory::fill_width(
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
        let environment = TuiEnvironment::new_manual();
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
        host.set_test_view(crate::presentation::factory::fill_width(
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
            let environment = TuiEnvironment::new_manual();
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
            host.set_test_view(crate::presentation::factory::fill_width(
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
        host.set_test_view(vf::content_host(port.id()).unwrap())
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
        host.set_test_view(content.clone()).unwrap();
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
        host.set_test_view(first).unwrap();
        assert!(host.flush_pending_hosts(8, true).unwrap().errors.is_empty());

        // Structural publication followed by a theme change must not reuse a
        // detached retained tree's old ContentHost measurement/paint ticket.
        host.set_test_view(vf::column_specs(
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
        host.set_test_view(vf::column_specs(
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
