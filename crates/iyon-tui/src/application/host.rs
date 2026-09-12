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
    ui_resources::UiResourceOwner,
};
use crate::controls::text_input::{TextInputPreview, command::TextInputCommand};
use crate::{
    BorderSpec, Component, ComponentCx, ComponentHandle, HistoryUnitId, InteractionResult,
    KeyStroke, Output, TextInput, Theme,
    backend::NativeHistorySink,
    geometry::Size,
    physical::{PhysicalRow, Surface},
    presentation::ContentProvider,
    scene::PreparedSceneFrame,
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
    /// Canonical React occurrence owner. The direct renderer consumes its
    /// immutable snapshots and never mutates this document.
    pub(super) ui_resources: UiResourceOwner,
    pub(super) ui_scene_revision: u64,
    /// One coalesced native projection frontier since the last successful
    /// adapter synchronization. Metadata-only commits remain here until a
    /// physical candidate or the lightweight visibility path consumes them.
    pending_ui_changes: Option<crate::occurrence::UiChangeSet>,
    ui_editors: std::collections::HashMap<crate::occurrence::ResourceKey, HostTextInput>,
    /// Native control registrations own only concrete control state. Their
    /// children and geometry remain occurrence-owned products.
    ui_scrolls: std::collections::HashMap<crate::occurrence::ResourceKey, u64>,
    ui_animations: std::collections::HashMap<crate::occurrence::ResourceKey, HostAnimation>,
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
        if let Err(error) = self.running.host_clear_direct_driver() {
            eprintln!("direct renderer shutdown failed: {error}");
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
struct HostAnimation {
    state: Arc<Mutex<AnimationClock>>,
    component_id: Arc<Mutex<Option<u64>>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct AnimationClock {
    active_frame: u32,
    frame_count: u32,
    interval: Duration,
    last_tick: Option<Instant>,
    running: bool,
}

impl HostAnimation {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AnimationClock {
                running: true,
                ..Default::default()
            })),
            component_id: Arc::new(Mutex::new(None)),
        }
    }

    fn frame_index(&self) -> Result<usize> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation lock is poisoned"))?
            .active_frame as usize)
    }

    fn component_id(&self) -> Option<u64> {
        self.component_id.lock().ok().and_then(|id| *id)
    }

    fn set_component_id(&self, id: u64) -> Result<()> {
        *self
            .component_id
            .lock()
            .map_err(|_| anyhow::anyhow!("animation component lock is poisoned"))? = Some(id);
        Ok(())
    }

    fn configure(
        &self,
        frame_count: u32,
        active_frame: u32,
        interval: Duration,
        running: bool,
        now: Instant,
    ) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation lock is poisoned"))?;
        state.frame_count = frame_count;
        state.active_frame = if frame_count == 0 {
            0
        } else {
            active_frame % frame_count
        };
        state.interval = interval;
        state.running = running;
        state.last_tick = (running && frame_count > 1 && !interval.is_zero()).then_some(now);
        Ok(())
    }

    fn tick(&self, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if !state.running || state.frame_count < 2 || state.interval.is_zero() {
            state.last_tick = None;
            return false;
        }
        let Some(last) = state.last_tick else {
            state.last_tick = Some(now);
            return false;
        };
        if now.duration_since(last) < state.interval {
            return false;
        }
        state.last_tick = Some(now);
        state.active_frame = (state.active_frame + 1) % state.frame_count;
        true
    }
}

struct MountedAnimation(HostAnimation);
impl Component for MountedAnimation {
    fn control_snapshot(&self) -> Option<crate::component::ControlSnapshot> {
        let state = self.0.state.lock().ok()?;
        Some(crate::component::ControlSnapshot::Animation(
            crate::component::AnimationSnapshot {
                active_frame: state.active_frame,
                frame_count: state.frame_count,
                running: state.running,
            },
        ))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.tick(Duration::from_millis(16), Self::tick);
    }
}
impl MountedAnimation {
    fn tick(component: &mut Self, now: Instant, _cx: &mut crate::EventCx<'_>) -> bool {
        component.0.tick(now)
    }
}

#[derive(Default)]
struct MountedScroll {
    viewport: Size,
    extent: Size,
}

impl Component for MountedScroll {
    fn control_snapshot(&self) -> Option<crate::component::ControlSnapshot> {
        Some(crate::component::ControlSnapshot::Scroll(
            crate::component::ScrollSnapshot {
                viewport_rows: u32::from(self.viewport.height),
                extent_rows: u32::from(self.extent.height),
                top_row: 0,
                following_end: true,
            },
        ))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.focusable();
        cx.on_layout_changed(Self::layout_changed);
        cx.on_content_extent_changed(Self::extent_changed);
    }
}

impl MountedScroll {
    fn layout_changed(scroll: &mut Self, size: Size) {
        scroll.viewport = size;
    }

    fn extent_changed(scroll: &mut Self, size: Size) {
        scroll.extent = size;
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

    pub(crate) fn frame_snapshot(&self) -> Result<crate::component::EditorSnapshot> {
        Ok(self.lock()?.frame_snapshot())
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
            inner.running.host_invalidate_component(component_id)?;
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
    fn control_snapshot(&self) -> Option<crate::component::ControlSnapshot> {
        self.0
            .frame_snapshot()
            .ok()
            .map(|snapshot| crate::component::ControlSnapshot::Editor(Box::new(snapshot)))
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
        let running = HostRunning::new();
        let frame = PreparedSceneFrame {
            surface: Surface::new(width, height),
            component_geometry: Default::default(),
            occurrence_geometry: HashMap::new(),
        };
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
        host.running.host_set_direct_driver_id(host_id)?;
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
            .or_else(|| inner.ui_scrolls.get(&control).copied())
            .or_else(|| {
                inner
                    .ui_animations
                    .get(&control)
                    .and_then(HostAnimation::component_id)
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
            .or_else(|| inner.ui_scrolls.get(&control).copied())
            .or_else(|| {
                inner
                    .ui_animations
                    .get(&control)
                    .and_then(HostAnimation::component_id)
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
        let driver_cleanup = inner.running.host_clear_direct_driver();
        let ui_cleanup = inner.ui_resources.close().map_err(anyhow::Error::msg);
        inner.presentation_state = PresentationState::Closed;
        inner.lifecycle = HostLifecycle::Closed(Arc::clone(operation));
        inner.presentation_notify.notify_waiters();
        inner.ui_event_notify.notify_waiters();
        match (driver_cleanup, ui_cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(driver), Ok(())) => Err(driver),
            (Ok(()), Err(ui)) => Err(ui),
            (Err(driver), Err(ui)) => Err(anyhow::anyhow!(
                "direct renderer shutdown failed: {driver}; UI cleanup failed: {ui}"
            )),
        }
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
        self.running
            .scene_history()
            .map_or(0, crate::history::History::len)
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
        let mut backend = self
            .backend
            .take()
            .expect("open host must own its terminal backend");
        let result = self
            .prepare_candidate_frame_with_backend(&mut backend)
            .and_then(|(frame, _)| match frame.product {
                PreparedFrameProduct::Scene { products, .. }
                | PreparedFrameProduct::NoOutput { products, .. } => Ok(products.scene),
                PreparedFrameProduct::Metadata { .. } => {
                    Err(anyhow::anyhow!("test candidate did not prepare a scene"))
                }
            });
        self.backend = Some(backend);
        result
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
            self.running.host_invalidate_content(*item)?;
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

    /// Re-admits a host after an owned worker completion. The callback only
    /// performs this short epoch/queue transition; worker code never holds
    /// the host lock while parsing, projecting, laying out, or painting.
    pub(super) fn wake_async_work(&mut self) {
        if matches!(self.lifecycle, HostLifecycle::Open | HostLifecycle::Faulted) {
            self.pending_epoch = self.pending_epoch.saturating_add(1);
            let _ = self.environment.mark_host_ready(self.host_id);
            self.presentation_notify.notify_waiters();
        }
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
        #[cfg(feature = "perf-counters")]
        let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::RuntimeAdvanceNanos);
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
                self.running.host_invalidate_content(dirty)?;
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
            let Some(key) = self.running.host_direct_control_for_component(component_id) else {
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
            .content
            .sync_ui_resources(&self.ui_resources, Some(&changes))?;
        for port_id in changed_content_ports {
            self.running
                .host_invalidate_content(crate::presentation::ContentDirty::new(
                    port_id,
                    None,
                    crate::presentation::ContentDirtyReason::SelectionLifecycle,
                ))?;
            self.content_dirty = true;
        }
        self.sync_direct_occurrences(Some(&changes))?;
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
                let result = result.and_then(|outcome| {
                    let direct_changed = outcome.inserted > 0
                        || matches!(
                            outcome.status,
                            crate::history::NativeTransferStatus::Progress
                        );
                    if direct_changed && self.running.host_has_direct_occurrences() {
                        self.sync_direct_occurrences(None)?;
                        let port_ids = self.running.host_direct_port_ids();
                        for port_id in port_ids.values().copied() {
                            self.running.host_invalidate_content(
                                crate::presentation::ContentDirty::new(
                                    port_id,
                                    None,
                                    crate::presentation::ContentDirtyReason::SelectionLifecycle,
                                ),
                            )?;
                        }
                    }
                    Ok(outcome)
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
        #[cfg(feature = "perf-counters")]
        let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::FramePrepareNanos);
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
        let direct_root = self
            .ui_resources
            .document
            .as_ref()
            .map(crate::occurrence::OccurrenceDocument::body_root);
        let direct_port_ids = self
            .ui_resources
            .ports
            .iter()
            .filter_map(|key| self.content.ui_port_id(*key).map(|id| (*key, id)))
            .collect::<HashMap<_, _>>();
        let front_content_blocked = self
            .running
            .host_native_history_front_content_port()
            .is_some_and(|port_id| self.content.history_transfer_blocked(port_id, size.width));
        let direct_history_anchor = if self.running.host_native_history_anchored()
            && (self.history_sink_blocked
                || self.running.host_native_history_blocked()
                || front_content_blocked)
        {
            crate::presentation::direct::DirectHistoryAnchor::NativeFrontier
        } else {
            crate::presentation::direct::DirectHistoryAnchor::FollowEnd
        };
        let (candidate, history_plan) = match self.running.prepare_frame_for_history(
            self.now,
            size,
            &mut self.content,
            direct_root,
            &direct_port_ids,
            direct_history_anchor,
        ) {
            Ok(candidate) => candidate,
            Err(error) => {
                if error
                    .downcast_ref::<super::content::ContentProjectionPending>()
                    .is_some()
                {
                    self.content.abort_candidate();
                    self.running.host_discard_candidate();
                    return Err(error);
                }
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
        // A host may receive theme/input work before its first accepted
        // occurrence commit. There is no physical candidate to prepare until
        // the direct occurrence tree is synchronized.
        if !self.running.host_has_direct_occurrences()
            && self
                .ui_resources
                .document
                .as_ref()
                .is_none_or(|document| document.accepted_ui_revision() == 0)
        {
            self.running.clear_dirty();
            return Ok(HostFlushOutcome::default());
        }
        let (prepared, history_plan) = if self.can_prepare_metadata_candidate() {
            (self.prepare_metadata_candidate(), None)
        } else {
            match self.prepare_candidate_frame() {
                Ok(candidate) => candidate,
                Err(error)
                    if error
                        .downcast_ref::<super::content::ContentProjectionPending>()
                        .is_some() =>
                {
                    return Ok(HostFlushOutcome {
                        waiting_for_presentation: true,
                        ..HostFlushOutcome::default()
                    });
                }
                Err(error) => return Err(error),
            }
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
                .content
                .sync_ui_resources(&self.ui_resources, changes.as_ref())
                .map_err(|error| {
                    host_attempt_error(
                        "frame",
                        "FRAME_PREPARATION_FAILED",
                        true,
                        format!("content resource synchronization failed: {error}"),
                    )
                })?;
            for port_id in changed_content_ports {
                self.running
                    .host_invalidate_content(crate::presentation::ContentDirty::new(
                        port_id,
                        None,
                        crate::presentation::ContentDirtyReason::SelectionLifecycle,
                    ))?;
            }
            self.sync_direct_occurrences(changes.as_ref())?;
            if !self.ui_resources.history_roots().is_empty()
                || changes.as_ref().is_some_and(|changes| {
                    !changes.history_roots.is_empty() || !changes.retired_nodes.is_empty()
                })
            {
                let history_units = self.direct_history_units(changes.as_ref())?;
                self.running
                    .host_sync_ui_history(history_units, changes.as_ref(), &mut self.content)
                    .map_err(|error| {
                        host_attempt_error(
                            "frame",
                            "FRAME_PREPARATION_FAILED",
                            true,
                            error.to_string(),
                        )
                    })?;
            }
            self.sync_ui_control_values(changes.as_ref())?;
            Ok(())
        })();
        if let Err(error) = sync_result {
            self.pending_ui_changes = changes;
            return Err(error);
        }
        self.ui_scene_revision = revision;
        self.desired_structural_revision = revision;
        Ok(())
    }

    fn direct_history_units(
        &self,
        changes: Option<&crate::occurrence::UiChangeSet>,
    ) -> Result<Vec<super::kernel::HistoryUnitRecipe>> {
        let roots = changes.map_or_else(
            || self.ui_resources.history_roots(),
            |changes| changes.history_roots.clone(),
        );
        roots
            .into_iter()
            .filter(|root| !self.running.host_ui_history_exported(*root))
            .map(|root| {
                let config = self
                    .ui_resources
                    .root_config(root)
                    .ok_or_else(|| anyhow::anyhow!("History root config is missing"))?;
                let unit = self.ui_resources.history_unit(root).ok_or_else(|| {
                    anyhow::anyhow!("History root native unit identity is missing")
                })?;
                let snapshot = self
                    .ui_resources
                    .document_snapshot(root)
                    .map_err(anyhow::Error::msg)?;
                let padding = snapshot
                    .properties
                    .iter()
                    .find_map(|(property, value)| {
                        (*property == crate::occurrence::PropertyId::Padding).then_some(value)
                    })
                    .and_then(|value| match value {
                        crate::occurrence::LayerValue::Value(
                            crate::occurrence::PropertyValue::Insets(insets),
                        ) => Some(*insets),
                        _ => None,
                    })
                    .unwrap_or(crate::Insets::ZERO);
                let (port_id, supported) = self.history_content_target(root)?;
                Ok(super::kernel::HistoryUnitRecipe {
                    root,
                    port_id,
                    padding,
                    native_transfer_allowed: supported,
                    unit_identity: unit.id,
                    status: unit.status,
                    flow_boundary: if config.flow_boundary == 1 {
                        crate::history::FlowBoundary::AttachToPrevious
                    } else {
                        crate::history::FlowBoundary::Default
                    },
                })
            })
            .collect()
    }

    /// Returns the one physical ContentHost export target for a History root.
    /// A one-child Box wrapper is only an occurrence-owned physical shell;
    /// arbitrary nested UI recipes and controls are intentionally blocked.
    fn history_content_target(&self, key: crate::occurrence::NodeKey) -> Result<(u64, bool)> {
        let snapshot = self
            .ui_resources
            .document_snapshot(key)
            .map_err(anyhow::Error::msg)?;
        let properties_supported = snapshot.properties.iter().all(|(property, value)| {
            !matches!(value, crate::occurrence::LayerValue::Value(_))
                || *property == crate::occurrence::PropertyId::Padding
                || (snapshot.kind == crate::occurrence::HostKind::Box
                    && *property == crate::occurrence::PropertyId::Layout)
        });
        if !properties_supported {
            return Ok((0, false));
        }
        match snapshot.kind {
            crate::occurrence::HostKind::ContentHost => {
                let Some(port) = snapshot.port else {
                    return Ok((0, false));
                };
                Ok((self.content.ui_port_id(port).unwrap_or(0), true))
            }
            crate::occurrence::HostKind::Box if snapshot.children.len() == 1 => {
                self.history_content_target(snapshot.children[0])
            }
            _ => Ok((0, false)),
        }
    }

    fn sync_direct_occurrences(
        &mut self,
        changes: Option<&crate::occurrence::UiChangeSet>,
    ) -> Result<()> {
        let animation_changed = changes.is_some_and(|changes| {
            changes.changed_nodes.iter().any(|key| {
                self.ui_resources
                    .document_snapshot(*key)
                    .is_ok_and(|snapshot| snapshot.kind == crate::occurrence::HostKind::Animation)
            })
        });
        let snapshots = if self.running.host_has_direct_occurrences()
            && changes.is_some()
            && !animation_changed
        {
            self.ui_resources
                .render_snapshot_delta(changes.expect("checked direct change set"))
        } else {
            self.ui_resources.render_snapshots()
        }
        .map_err(|error| {
            host_attempt_error("frame", "FRAME_PREPARATION_FAILED", true, error.to_string())
        })?;
        let document = self
            .ui_resources
            .document
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("UI resource owner has no occurrence document"))?;
        let participation = snapshots
            .iter()
            .map(|snapshot| {
                document
                    .demanded_node(snapshot.key, |control| {
                        self.ui_resources
                            .control_state(control)
                            .and_then(crate::occurrence::ControlState::animation_active_frame)
                            .map(|frame| frame as usize)
                    })
                    .map(
                        |(participates, _)| crate::presentation::taffy::NodeParticipation {
                            key: snapshot.key,
                            participates,
                        },
                    )
                    .map_err(|error| anyhow::anyhow!("occurrence demand lookup failed: {error:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let port_ids = self
            .ui_resources
            .ports
            .iter()
            .filter_map(|key| self.content.ui_port_id(*key).map(|id| (*key, id)))
            .collect::<HashMap<_, _>>();
        let mut roots = document
            .history_roots()
            .into_iter()
            .filter(|root| !self.running.host_ui_history_exported(*root))
            .collect::<Vec<_>>();
        roots.push(document.body_root());
        let portal_owners = document
            .portal_roots()
            .into_iter()
            .filter_map(|root| document.root_owner(root).map(|owner| (root, owner)))
            .collect::<HashMap<_, _>>();
        roots.extend(document.portal_roots());
        let body_root = document.body_root();
        self.running
            .host_sync_direct_occurrences(
                snapshots,
                changes,
                &participation,
                port_ids,
                roots,
                body_root,
                portal_owners,
            )
            .map_err(|error| {
                host_attempt_error(
                    "frame",
                    "FRAME_PREPARATION_FAILED",
                    true,
                    format!("direct occurrence synchronization failed: {error}"),
                )
            })
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
        if let Some(component_id) = self.ui_scrolls.remove(&key) {
            self.running.host_retire_component(component_id);
        }
        if let Some(slot) = self.ui_animations.remove(&key)
            && let Some(component_id) = slot.component_id()
        {
            self.running.host_retire_component(component_id);
        }
        self.running.host_remove_direct_control_component(key);
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
            self.running
                .host_set_direct_control_component(key, component.raw_id());
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
                self.running.host_invalidate_component(component)?;
            }
        }
        Ok(())
    }

    fn sync_ui_scroll(&mut self, key: crate::occurrence::ResourceKey) -> Result<()> {
        if self.ui_scrolls.contains_key(&key) {
            return Ok(());
        }
        let component = self.running.host_register(MountedScroll::default());
        self.running
            .host_set_direct_control_component(key, component.raw_id());
        self.ui_scrolls.insert(key, component.raw_id());
        Ok(())
    }

    fn sync_ui_animation(&mut self, key: crate::occurrence::ResourceKey) -> Result<()> {
        if self.ui_animations.contains_key(&key) {
            return Ok(());
        }
        let slot = HostAnimation::new();
        let component = self.running.host_register(MountedAnimation(slot.clone()));
        slot.set_component_id(component.raw_id())?;
        self.running
            .host_set_direct_control_component(key, component.raw_id());
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
            let child_count = document
                .snapshot(node)
                .map_err(|error| anyhow::anyhow!("animation control snapshot failed: {error:?}"))?
                .children
                .len()
                .try_into()
                .unwrap_or(u32::MAX);
            match state {
                crate::occurrence::ControlState::Scroll(_) => {}
                crate::occurrence::ControlState::Animation(animation) => {
                    if let Some(slot) = self.ui_animations.get(&key) {
                        slot.configure(
                            child_count,
                            animation.active_frame(),
                            Duration::from_millis(u64::from(animation.interval_ms().unwrap_or(0))),
                            animation.running(),
                            self.now,
                        )?;
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
        let Some(control) = self
            .running
            .host_direct_control_for_component(focused_component)
        else {
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
        #[cfg(feature = "perf-counters")]
        let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::FramePresentNanos);
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
        #[cfg(feature = "perf-counters")]
        let _perf_timer = crate::perf::ScopedTimer::new(crate::perf::Counter::FrameCommitNanos);
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
                    // A native compatibility test/helper may complete a
                    // scene-only candidate while the direct occurrence driver
                    // owns the confirmed geometry. Do not erase that exact
                    // occurrence-keyed product merely because the legacy
                    // candidate has no occurrence metadata.
                    let mut scene = products.scene;
                    if scene.occurrence_geometry.is_empty()
                        && !self.frame.occurrence_geometry.is_empty()
                    {
                        scene.occurrence_geometry = self.frame.occurrence_geometry.clone();
                    }
                    self.frame = scene;
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
