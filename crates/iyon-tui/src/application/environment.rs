//! Native environment scheduler for PERF-13's shared host wake seam.
//!
//! The environment owns pending-host fairness and the edge-trigger latch. It
//! does not own semantic UI structure, retained layout, or terminal paint.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    Arc, Condvar, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};
use std::task::Wake;
use std::time::Duration;

use super::content::{ContentSourceRegistry, HostContentSource, TextSourceKind};
use super::host::HostInner;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HostFlushOutcome {
    pub(super) committed: bool,
    pub(super) waiting_for_presentation: bool,
    pub(super) waiting_for_physical_work: bool,
    pub(super) committed_epoch: Option<u64>,
    pub(super) visible_structural_revision: Option<u64>,
}

#[derive(Debug)]
pub(super) struct HostAttemptError {
    pub(super) phase: &'static str,
    pub(super) code: &'static str,
    pub(super) retryable: bool,
    pub(super) diagnostic: String,
}

impl std::fmt::Display for HostAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for HostAttemptError {}

pub(super) fn host_attempt_error(
    phase: &'static str,
    code: &'static str,
    retryable: bool,
    diagnostic: impl Into<String>,
) -> anyhow::Error {
    anyhow::Error::new(HostAttemptError {
        phase,
        code,
        retryable,
        diagnostic: diagnostic.into(),
    })
}

/// Monotonic host/frame state shared with the runtime wake broker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostEpochs {
    pub host_id: u64,
    pub desired_structural_revision: u64,
    pub visible_structural_revision: u64,
    pub visible_frame_revision: u64,
    pub pending_epoch: u64,
    pub committed_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFrameError {
    pub host_id: u64,
    pub attempted_epoch: u64,
    pub desired_revision: u64,
    pub phase: String,
    pub code: String,
    pub retryable: bool,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCommit {
    pub host_id: u64,
    pub committed_epoch: u64,
    pub visible_structural_revision: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostDrainReport {
    pub rearm: bool,
    pub waiting_for_presentation: bool,
    pub attempted: usize,
    pub commits: Vec<HostCommit>,
    pub errors: Vec<HostFrameError>,
    pub wake_epoch: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeDisposition {
    pub schedule_environment_drain: bool,
}

/// Stable process-local identity used by Source direct-FFI calls. The
/// generation is intentionally explicit even though v1 does not recycle slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnvironmentIdentity {
    pub slot: u32,
    pub generation: u32,
}

static NEXT_ENVIRONMENT_SLOT: AtomicU64 = AtomicU64::new(1);

impl EnvironmentIdentity {
    pub(crate) fn allocate() -> Self {
        let slot = NEXT_ENVIRONMENT_SLOT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current <= u64::from(u32::MAX)).then_some(current + 1)
            })
            .unwrap_or_else(|_| panic!("environment identity exhausted"));
        Self {
            slot: u32::try_from(slot).expect("environment identity fits u32"),
            generation: 1,
        }
    }
}

/// One native environment owns the pending-host set and wake latch shared by
/// all hosts created in that environment. The queue stores IDs only; host
/// state remains authoritative in each registered host.
#[derive(Clone)]
pub struct TuiEnvironment {
    #[allow(dead_code)] // Drop of the pointee is the environment shutdown gate.
    lifetime: Arc<EnvironmentLifetime>,
}

/// Queue/service access used by the native scheduler. Unlike
/// `TuiEnvironment`, this value is not a public owner token and is never used
/// to decide when environment shutdown starts.
#[derive(Clone)]
struct EnvironmentQueue {
    inner: Arc<Mutex<EnvironmentInner>>,
    /// A host drain may release `inner` while it services a host. Keep those
    /// service turns single-threaded so the native driver and an explicit
    /// barrier cannot concurrently pop/reconcile the same pending host.
    drain_gate: Arc<Mutex<()>>,
}

/// The one real owner of environment shutdown. Every public environment
/// facade clone shares this pointee; its `Drop` therefore runs exactly once
/// after the last public/core owner releases it. The driver only receives an
/// `EnvironmentQueue` and cannot keep this lifetime alive.
struct EnvironmentLifetime {
    queue: EnvironmentQueue,
    #[cfg(test)]
    shutdown_probe: Option<std::sync::mpsc::Sender<()>>,
}

impl Drop for EnvironmentLifetime {
    fn drop(&mut self) {
        if let Ok(mut environment) = self.queue.inner.lock() {
            environment.shutdown = true;
            environment.wake.notify_all();
            environment.notify.notify_waiters();
        }
        #[cfg(test)]
        if let Some(probe) = self.shutdown_probe.take() {
            let _ = probe.send(());
        }
    }
}

// State IDs embed the host slot in their upper 21 bits. Allocate host slots
// process-wide because independently-created TuiEnvironments may exchange a
// detached native History before the TypeScript resolver sees its contents.
const MAX_STATE_HOST_ID: u64 = 0x001f_ffff;

static NEXT_HOST_ID: AtomicU64 = AtomicU64::new(1);

struct EnvironmentInner {
    identity: EnvironmentIdentity,
    hosts: HashMap<u64, Weak<Mutex<HostInner>>>,
    pending: VecDeque<u64>,
    pending_set: HashSet<u64>,
    queued: HashSet<u64>,
    retry_blocked: HashSet<u64>,
    waiting_for_presentation: HashSet<u64>,
    wake_latched: bool,
    wake_epoch: u64,
    wake: Arc<Condvar>,
    notify: Arc<tokio::sync::Notify>,
    shutdown: bool,
    startup_error: Option<String>,
    content_sources: ContentSourceRegistry,
    #[cfg(test)]
    last_completion_capacities: Option<([usize; 4], [usize; 4], [usize; 4])>,
    #[cfg(test)]
    driver_exit_probe: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(test)]
fn signal_driver_exit(state: &EnvironmentInner) {
    if let Some(probe) = state.driver_exit_probe.as_ref() {
        let _ = probe.send(());
    }
}

/// Weak wake state installed in a native presentation receipt. Completion
/// only re-admits the qualified host; it never retains or locks that host.
#[derive(Clone)]
pub(crate) struct ReceiptWake {
    inner: Weak<Mutex<EnvironmentInner>>,
    host_id: u64,
}

impl ReceiptWake {
    fn wake_host(&self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        let Ok(mut environment) = inner.lock() else {
            return;
        };
        if environment.shutdown || !environment.hosts.contains_key(&self.host_id) {
            return;
        }
        environment.pending_set.insert(self.host_id);
        environment.waiting_for_presentation.remove(&self.host_id);
        EnvironmentQueue::queue_host(&mut environment, self.host_id);
        environment.wake_latched = true;
        environment.wake_epoch = environment.wake_epoch.saturating_add(1);
        environment.wake.notify_all();
        environment.notify.notify_waiters();
    }
}

impl Wake for ReceiptWake {
    fn wake(self: Arc<Self>) {
        self.wake_host();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_host();
    }
}

impl TuiEnvironment {
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_driver(true)
    }

    fn new_with_driver(start_driver: bool) -> Self {
        let identity = EnvironmentIdentity::allocate();
        let wake = Arc::new(Condvar::new());
        let notify = Arc::new(tokio::sync::Notify::new());
        let content_sources = ContentSourceRegistry::with_identity(identity);
        let startup_error = content_sources.executor_startup_error();
        let inner = Arc::new(Mutex::new(EnvironmentInner {
            identity,
            hosts: HashMap::new(),
            pending: VecDeque::new(),
            pending_set: HashSet::new(),
            queued: HashSet::new(),
            retry_blocked: HashSet::new(),
            waiting_for_presentation: HashSet::new(),
            wake_latched: false,
            wake_epoch: 0,
            wake: Arc::clone(&wake),
            notify: Arc::clone(&notify),
            shutdown: false,
            startup_error,
            content_sources,
            #[cfg(test)]
            last_completion_capacities: None,
            #[cfg(test)]
            driver_exit_probe: None,
        }));
        let queue = EnvironmentQueue {
            inner: Arc::clone(&inner),
            drain_gate: Arc::new(Mutex::new(())),
        };
        let environment = Self {
            lifetime: Arc::new(EnvironmentLifetime {
                queue: queue.clone(),
                #[cfg(test)]
                shutdown_probe: None,
            }),
        };
        if start_driver {
            let startup = std::thread::Builder::new()
                .name("iyon-native-environment".to_owned())
                .spawn(move || native_driver_loop(queue));
            if let Err(error) = startup
                && let Ok(mut state) = inner.lock()
            {
                state.shutdown = true;
                state.startup_error =
                    Some(format!("native environment driver start failed: {error}"));
                state.wake.notify_all();
                state.notify.notify_waiters();
            }
        }
        environment
    }

    /// Builds an environment without a background driver for deterministic
    /// owner-level tests. Production construction always starts the shared
    /// scheduler above; tests that manually drain the queue opt in here.
    #[cfg(any(test, feature = "test-util"))]
    pub fn new_manual() -> Self {
        Self::new_with_driver(false)
    }

    #[cfg(test)]
    pub(crate) fn new_with_shutdown_probes(
        shutdown_probe: std::sync::mpsc::Sender<()>,
        driver_exit_probe: std::sync::mpsc::Sender<()>,
    ) -> Self {
        let mut environment = Self::new_with_driver(true);
        Arc::get_mut(&mut environment.lifetime)
            .expect("environment lifetime has one owner during construction")
            .shutdown_probe = Some(shutdown_probe);
        environment
            .lifetime
            .queue
            .inner
            .lock()
            .expect("environment queue is healthy during construction")
            .driver_exit_probe = Some(driver_exit_probe);
        environment
    }

    #[must_use]
    pub fn environment_slot(&self) -> u32 {
        self.lifetime.queue.environment_slot()
    }

    #[must_use]
    pub fn environment_generation(&self) -> u32 {
        self.lifetime.queue.environment_generation()
    }

    pub fn lookup_content_source(
        &self,
        source_slot: u64,
        source_generation: u32,
    ) -> anyhow::Result<HostContentSource> {
        self.lifetime
            .queue
            .lookup_content_source(source_slot, source_generation)
    }

    pub fn create_content_source(&self, kind: TextSourceKind) -> anyhow::Result<HostContentSource> {
        self.lifetime.queue.create_content_source(kind)
    }

    pub(super) fn content_source_registry(&self) -> anyhow::Result<ContentSourceRegistry> {
        self.lifetime.queue.content_source_registry()
    }

    pub(super) fn wake_epoch(&self) -> u64 {
        self.lifetime.queue.wake_epoch()
    }

    pub(super) fn wake_notification(&self) -> anyhow::Result<Arc<tokio::sync::Notify>> {
        self.lifetime.queue.wake_notification()
    }

    pub(crate) fn receipt_wake(&self, host_id: u64) -> ReceiptWake {
        self.lifetime.queue.receipt_wake(host_id)
    }

    pub(super) fn register_host(&self, host: &Arc<Mutex<HostInner>>) -> anyhow::Result<u64> {
        self.lifetime.queue.register_host(host)
    }

    pub(super) fn unregister_host(&self, host_id: u64) {
        self.lifetime.queue.unregister_host(host_id);
    }

    pub(super) fn cancel_host_scheduling(&self, host_id: u64) {
        self.lifetime.queue.cancel_host_scheduling(host_id);
    }

    pub(super) fn mark_host_pending(&self, host_id: u64) -> anyhow::Result<WakeDisposition> {
        self.lifetime.queue.mark_host_pending(host_id)
    }

    /// Requeues a worker completion even when the host was marked as waiting
    /// for a physical receipt. Content/layout completions are not receipts and
    /// must be allowed to run the host's next short transition.
    pub(super) fn mark_host_ready(&self, host_id: u64) -> anyhow::Result<()> {
        self.lifetime.queue.mark_host_ready(host_id)
    }

    pub(super) fn with_host_completion<R>(
        &self,
        host_id: u64,
        committed_epoch: u64,
        requeue_if_pending: bool,
        waiting_for_presentation: bool,
        commit: impl FnOnce() -> anyhow::Result<(R, u64, bool)>,
    ) -> anyhow::Result<R> {
        self.lifetime.queue.with_host_completion(
            host_id,
            committed_epoch,
            requeue_if_pending,
            waiting_for_presentation,
            commit,
        )
    }

    pub fn drain_pending(
        &self,
        budget: usize,
        force_retry: bool,
    ) -> anyhow::Result<HostDrainReport> {
        self.lifetime.queue.drain_pending(budget, force_retry)
    }

    pub(super) fn drain_pending_for(
        &self,
        budget: usize,
        force_retry: bool,
        preferred_host_id: Option<u64>,
    ) -> anyhow::Result<HostDrainReport> {
        self.lifetime
            .queue
            .drain_pending_for(budget, force_retry, preferred_host_id)
    }

    #[cfg(test)]
    pub(super) fn take_last_completion_capacities(
        &self,
    ) -> Option<([usize; 4], [usize; 4], [usize; 4])> {
        self.lifetime.queue.take_last_completion_capacities()
    }
}

impl EnvironmentQueue {
    pub(crate) fn identity(&self) -> EnvironmentIdentity {
        self.inner
            .lock()
            .map(|environment| environment.identity)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn environment_slot(&self) -> u32 {
        self.identity().slot
    }

    #[must_use]
    pub fn environment_generation(&self) -> u32 {
        self.identity().generation
    }

    pub fn lookup_content_source(
        &self,
        source_slot: u64,
        source_generation: u32,
    ) -> anyhow::Result<HostContentSource> {
        self.content_source_registry()?
            .lookup(source_slot, source_generation)
    }

    pub(super) fn wake_epoch(&self) -> u64 {
        self.inner
            .lock()
            .map_or(0, |environment| environment.wake_epoch)
    }

    pub(super) fn wake_notification(&self) -> anyhow::Result<Arc<tokio::sync::Notify>> {
        self.inner
            .lock()
            .map(|environment| Arc::clone(&environment.notify))
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))
    }

    pub(crate) fn receipt_wake(&self, host_id: u64) -> ReceiptWake {
        ReceiptWake {
            inner: Arc::downgrade(&self.inner),
            host_id,
        }
    }

    pub fn create_content_source(&self, kind: TextSourceKind) -> anyhow::Result<HostContentSource> {
        self.inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?
            .content_sources
            .create(kind)
    }

    pub(super) fn content_source_registry(&self) -> anyhow::Result<ContentSourceRegistry> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?
            .content_sources
            .clone())
    }

    pub(super) fn register_host(&self, host: &Arc<Mutex<HostInner>>) -> anyhow::Result<u64> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if let Some(error) = &environment.startup_error {
            return Err(anyhow::anyhow!(error.clone()));
        }
        let id = NEXT_HOST_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current <= MAX_STATE_HOST_ID).then_some(current + 1)
            })
            .map_err(|_| anyhow::anyhow!("host identity exhausted"))?;
        environment.hosts.insert(id, Arc::downgrade(host));
        Ok(id)
    }

    pub(super) fn unregister_host(&self, host_id: u64) {
        let Ok(mut environment) = self.inner.lock() else {
            return;
        };
        environment.hosts.remove(&host_id);
        environment.pending_set.remove(&host_id);
        environment.retry_blocked.remove(&host_id);
        environment.waiting_for_presentation.remove(&host_id);
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
        environment.wake.notify_all();
        environment.notify.notify_waiters();
    }

    pub(super) fn cancel_host_scheduling(&self, host_id: u64) {
        let Ok(mut environment) = self.inner.lock() else {
            return;
        };
        environment.pending_set.remove(&host_id);
        environment.retry_blocked.remove(&host_id);
        environment.waiting_for_presentation.remove(&host_id);
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
        environment.wake.notify_all();
        environment.notify.notify_waiters();
    }

    fn queue_host(environment: &mut EnvironmentInner, host_id: u64) {
        if !environment.pending_set.contains(&host_id)
            || environment.retry_blocked.contains(&host_id)
            || !environment.queued.insert(host_id)
        {
            return;
        }
        environment.pending.push_back(host_id);
    }

    fn prioritize_host(environment: &mut EnvironmentInner, host_id: u64) {
        if !environment.pending_set.contains(&host_id)
            || environment.retry_blocked.contains(&host_id)
        {
            return;
        }
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        environment.pending.push_front(host_id);
        environment.queued.insert(host_id);
    }

    pub(super) fn mark_host_pending(&self, host_id: u64) -> anyhow::Result<WakeDisposition> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if let Some(error) = &environment.startup_error {
            return Err(anyhow::anyhow!(error.clone()));
        }
        if !environment.hosts.contains_key(&host_id) {
            return Ok(WakeDisposition::default());
        }
        let schedule = !environment.wake_latched;
        let next_wake_epoch = if schedule {
            Some(
                environment
                    .wake_epoch
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("environment wake epoch exhausted"))?,
            )
        } else {
            None
        };
        environment.wake_latched = true;
        if let Some(next_wake_epoch) = next_wake_epoch {
            environment.wake_epoch = next_wake_epoch;
        }
        environment.retry_blocked.remove(&host_id);
        // A newly accepted epoch supersedes the old waiting edge. It must be
        // queued even when the prior attempt was waiting for an async worker
        // or physical receipt; otherwise a UI mutation accepted during that
        // wait has no driver wake of its own.
        environment.waiting_for_presentation.remove(&host_id);
        environment.pending_set.insert(host_id);
        Self::queue_host(&mut environment, host_id);
        Self::prioritize_host(&mut environment, host_id);
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        Ok(WakeDisposition {
            schedule_environment_drain: schedule,
        })
    }

    pub(super) fn mark_host_ready(&self, host_id: u64) -> anyhow::Result<()> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Ok(());
        }
        environment.pending_set.insert(host_id);
        environment.retry_blocked.remove(&host_id);
        environment.waiting_for_presentation.remove(&host_id);
        Self::queue_host(&mut environment, host_id);
        environment.wake_latched = true;
        environment.wake_epoch = environment
            .wake_epoch
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("environment wake epoch exhausted"))?;
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        Ok(())
    }

    pub(super) fn complete_host(
        &self,
        host_id: u64,
        pending_epoch: u64,
        committed_epoch: u64,
        newer_epoch: bool,
        requeue_if_pending: bool,
        waiting_for_presentation: bool,
    ) -> anyhow::Result<()> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Ok(());
        }
        Self::complete_host_locked(
            &mut environment,
            host_id,
            pending_epoch,
            committed_epoch,
            newer_epoch,
            requeue_if_pending,
            waiting_for_presentation,
            false,
        );
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        Ok(())
    }

    /// Holds the environment authority across one Host's final visible
    /// promotion. Queue capacity is reserved while the mutex is held, the
    /// caller's commit closure runs without reacquiring this lock, and the
    /// completion bookkeeping is applied before the guard is released. Thus
    /// no independently poisonable environment lock is taken after visible
    /// mutation and no other Host can consume the reserved queue capacity.
    pub(super) fn with_host_completion<R>(
        &self,
        host_id: u64,
        committed_epoch: u64,
        requeue_if_pending: bool,
        waiting_for_presentation: bool,
        commit: impl FnOnce() -> anyhow::Result<(R, u64, bool)>,
    ) -> anyhow::Result<R> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Err(anyhow::anyhow!(
                "host is no longer registered with its environment"
            ));
        }
        #[cfg(test)]
        let before_capacity = [
            environment.pending.capacity(),
            environment.pending_set.capacity(),
            environment.queued.capacity(),
            environment.retry_blocked.capacity(),
        ];
        if requeue_if_pending && !waiting_for_presentation {
            // A newer operation accepted during a delayed receipt already
            // normally queued this Host. Reserve defensively for both that
            // race and a post-promotion deferred cleanup retry, whose epoch
            // is returned by the commit closure only after visible mutation.
            environment.pending.reserve(1);
            environment.queued.reserve(1);
            environment.pending_set.reserve(1);
            environment.retry_blocked.reserve(1);
        }
        #[cfg(test)]
        let reserved_capacity = [
            environment.pending.capacity(),
            environment.pending_set.capacity(),
            environment.queued.capacity(),
            environment.retry_blocked.capacity(),
        ];
        let result = commit();
        let outcome = match result {
            Ok((value, completion_pending_epoch, block_for_retry)) => {
                Self::complete_host_locked(
                    &mut environment,
                    host_id,
                    completion_pending_epoch,
                    committed_epoch,
                    false,
                    requeue_if_pending,
                    waiting_for_presentation,
                    block_for_retry,
                );
                Ok(value)
            }
            Err(error) => Err(error),
        };
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        #[cfg(test)]
        {
            let after_capacity = [
                environment.pending.capacity(),
                environment.pending_set.capacity(),
                environment.queued.capacity(),
                environment.retry_blocked.capacity(),
            ];
            environment.last_completion_capacities =
                Some((before_capacity, reserved_capacity, after_capacity));
        }
        outcome
    }

    #[cfg(test)]
    pub(super) fn take_last_completion_capacities(
        &self,
    ) -> Option<([usize; 4], [usize; 4], [usize; 4])> {
        self.inner.lock().ok()?.last_completion_capacities
    }

    fn complete_host_locked(
        environment: &mut EnvironmentInner,
        host_id: u64,
        pending_epoch: u64,
        committed_epoch: u64,
        newer_epoch: bool,
        requeue_if_pending: bool,
        waiting_for_presentation: bool,
        block_for_retry: bool,
    ) {
        if block_for_retry && pending_epoch == committed_epoch {
            // Deferred Source cleanup is observable pending work, but must
            // not continuously requeue a host while the same Source remains
            // poisoned.  Explicit retry, a new Source mutation, or another
            // independent host operation removes this block.
            environment.pending_set.insert(host_id);
            environment.retry_blocked.insert(host_id);
            environment.waiting_for_presentation.remove(&host_id);
            environment.queued.remove(&host_id);
            environment.pending.retain(|id| *id != host_id);
        } else if pending_epoch == committed_epoch {
            environment.pending_set.remove(&host_id);
            environment.retry_blocked.remove(&host_id);
            environment.waiting_for_presentation.remove(&host_id);
            environment.queued.remove(&host_id);
            environment.pending.retain(|id| *id != host_id);
        } else if waiting_for_presentation && !newer_epoch && !environment.queued.contains(&host_id)
        {
            environment.pending_set.insert(host_id);
            environment.waiting_for_presentation.insert(host_id);
            environment.pending.retain(|id| *id != host_id);
        } else {
            environment.pending_set.insert(host_id);
            environment.waiting_for_presentation.remove(&host_id);
            if requeue_if_pending {
                Self::queue_host(environment, host_id);
            } else if !environment.queued.contains(&host_id) {
                environment.queued.remove(&host_id);
                environment.pending.retain(|id| *id != host_id);
            }
        }
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
    }

    fn block_host(&self, host_id: u64) -> anyhow::Result<()> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Ok(());
        }
        environment.pending_set.insert(host_id);
        environment.retry_blocked.insert(host_id);
        environment.waiting_for_presentation.remove(&host_id);
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
        Ok(())
    }

    fn requeue_after_new_epoch(&self, host_id: u64) -> anyhow::Result<()> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Ok(());
        }
        environment.pending_set.insert(host_id);
        environment.retry_blocked.remove(&host_id);
        environment.waiting_for_presentation.remove(&host_id);
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        environment.wake_latched = true;
        Self::queue_host(&mut environment, host_id);
        environment.wake.notify_all();
        environment.notify.notify_waiters();
        Ok(())
    }

    /// Fairly drains pending hosts. Automatic drains leave failed hosts
    /// retry-blocked; an explicit barrier passes `force_retry = true`.
    pub fn drain_pending(
        &self,
        budget: usize,
        force_retry: bool,
    ) -> anyhow::Result<HostDrainReport> {
        self.drain_pending_for(budget, force_retry, None)
    }

    pub(super) fn drain_pending_for(
        &self,
        budget: usize,
        force_retry: bool,
        preferred_host_id: Option<u64>,
    ) -> anyhow::Result<HostDrainReport> {
        let _drain_guard = self
            .drain_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("environment drain gate is poisoned"))?;
        let mut report = HostDrainReport::default();
        let budget = budget.max(1);
        let mut candidates = Vec::new();
        let mut source_wake_errors = Vec::new();
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        report.wake_epoch = environment.wake_epoch;
        // Source payload acceptance happens before host wake delivery.  Wake
        // failures therefore arrive through the Source registry's side
        // channel and must be surfaced independently of the pending-host
        // queue.  Resolve the weak host by identity only: a poisoned host
        // mutex must not prevent its error from reaching the environment
        // channel, and pointer equality prevents allocator-address reuse
        // from retargeting a stale failure.
        for failure in environment.content_sources.take_wake_failures() {
            let host_id = environment
                .hosts
                .iter()
                .find(|(_, candidate)| {
                    candidate.as_ptr() as usize == failure.host_key
                        && Weak::ptr_eq(candidate, &failure.host)
                })
                .map(|(host_id, _)| *host_id);
            let Some(host_id) = host_id else {
                // The subscriber disappeared before the next environment
                // drain.  There is no surviving host error channel to notify;
                // dropping this stale diagnostic is the membership-race
                // policy, not a mutation rejection or a lost healthy wake.
                continue;
            };
            source_wake_errors.push(HostFrameError {
                host_id,
                attempted_epoch: 0,
                desired_revision: 0,
                phase: "content".to_owned(),
                code: "SOURCE_WAKE_FAILED".to_owned(),
                retryable: false,
                diagnostic: failure.diagnostic,
            });
        }
        if force_retry {
            if let Some(host_id) = preferred_host_id {
                environment.retry_blocked.remove(&host_id);
                Self::queue_host(&mut environment, host_id);
                Self::prioritize_host(&mut environment, host_id);
            } else {
                let blocked = environment.retry_blocked.drain().collect::<Vec<_>>();
                for host_id in blocked {
                    Self::queue_host(&mut environment, host_id);
                }
                let waiting = environment
                    .pending_set
                    .iter()
                    .copied()
                    .filter(|host_id| {
                        !environment.retry_blocked.contains(host_id)
                            && !environment.queued.contains(host_id)
                    })
                    .collect::<Vec<_>>();
                for host_id in waiting {
                    Self::queue_host(&mut environment, host_id);
                }
            }
        }
        while candidates.len() < budget {
            let Some(host_id) = environment.pending.pop_front() else {
                break;
            };
            environment.queued.remove(&host_id);
            if environment.retry_blocked.contains(&host_id) {
                continue;
            }
            candidates.push(host_id);
        }
        drop(environment);

        for host_id in candidates {
            report.attempted += 1;
            let weak = {
                let environment = self
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
                environment.hosts.get(&host_id).cloned()
            };
            let Some(weak) = weak else {
                self.unregister_host(host_id);
                continue;
            };
            let Some(host_arc) = weak.upgrade() else {
                self.unregister_host(host_id);
                continue;
            };
            let mut host = match host_arc.lock() {
                Ok(host) => host,
                Err(_) => {
                    // A poisoned host must not abort the entire fair drain:
                    // `candidates` already owns the remaining IDs, and
                    // returning here would silently drop those pending
                    // hosts. Report this identity as a blocked attempt and
                    // continue draining unrelated hosts; explicit retry can
                    // still re-attempt the poisoned owner without an
                    // automatic spin loop.
                    report.errors.push(HostFrameError {
                        host_id,
                        attempted_epoch: 0,
                        desired_revision: 0,
                        phase: "host".to_owned(),
                        code: "HOST_LOCK_POISONED".to_owned(),
                        retryable: false,
                        diagnostic: "host lock is poisoned".to_owned(),
                    });
                    self.block_host(host_id)?;
                    continue;
                }
            };
            let history_signal = host.history_work_signal();
            let queued_epoch = host.environment_pending_epoch()?;
            let result = host.flush_for_environment(force_retry, force_retry);
            let waiting_for_physical_work = result
                .as_ref()
                .is_ok_and(|(outcome, _, _)| outcome.waiting_for_physical_work);
            if waiting_for_physical_work {
                // Logical preparation has captured the exact rows but must
                // not submit them while HostInner is held. Start the worker
                // command after releasing the acceptance guard; the receipt
                // wake re-admits this host fairly when the command settles.
                drop(host);
                match HostInner::start_history_work(&host_arc) {
                    Ok((pending_epoch, committed_epoch)) => {
                        report.waiting_for_presentation = true;
                        self.complete_host(
                            host_id,
                            pending_epoch,
                            committed_epoch,
                            pending_epoch != queued_epoch,
                            true,
                            false,
                        )?;
                    }
                    Err(error) => {
                        let failure = error.downcast_ref::<HostAttemptError>();
                        let (attempted_epoch, desired_revision, pending_epoch) = host_arc
                            .lock()
                            .map(|host| host.environment_error_epochs())
                            .unwrap_or((0, 0, queued_epoch));
                        report.errors.push(HostFrameError {
                            host_id,
                            attempted_epoch,
                            desired_revision,
                            phase: failure.map_or_else(
                                || "runtime".to_owned(),
                                |failure| failure.phase.to_owned(),
                            ),
                            code: failure.map_or_else(
                                || "INTERNAL_INVARIANT".to_owned(),
                                |failure| failure.code.to_owned(),
                            ),
                            retryable: failure.is_some_and(|failure| failure.retryable),
                            diagnostic: error.to_string(),
                        });
                        if pending_epoch != queued_epoch {
                            self.requeue_after_new_epoch(host_id)?;
                        } else {
                            self.block_host(host_id)?;
                        }
                    }
                }
                continue;
            }
            match result {
                Ok((outcome, pending_epoch, committed_epoch)) => {
                    report.waiting_for_presentation |= outcome.waiting_for_presentation;
                    if outcome.committed
                        && let (Some(committed_epoch), Some(visible_structural_revision)) =
                            (outcome.committed_epoch, outcome.visible_structural_revision)
                    {
                        report.commits.push(HostCommit {
                            host_id,
                            committed_epoch,
                            visible_structural_revision,
                        });
                    }
                    // Keep the host lock held through the environment queue
                    // update. A concurrent producer must not advance the host
                    // epoch between the frame result and this reconciliation.
                    if !outcome.committed {
                        self.complete_host(
                            host_id,
                            pending_epoch,
                            committed_epoch,
                            pending_epoch != queued_epoch,
                            !outcome.waiting_for_presentation,
                            outcome.waiting_for_presentation,
                        )?;
                    }
                }
                Err(error) => {
                    let failure = error.downcast_ref::<HostAttemptError>();
                    let (attempted_epoch, desired_revision, pending_epoch) =
                        host.environment_error_epochs();
                    report.errors.push(HostFrameError {
                        host_id,
                        attempted_epoch,
                        desired_revision,
                        phase: failure.map_or_else(
                            || "runtime".to_owned(),
                            |failure| failure.phase.to_owned(),
                        ),
                        code: failure.map_or_else(
                            || "INTERNAL_INVARIANT".to_owned(),
                            |failure| failure.code.to_owned(),
                        ),
                        retryable: failure.is_some_and(|failure| failure.retryable),
                        diagnostic: error.to_string(),
                    });
                    let has_new_epoch = pending_epoch != queued_epoch;
                    if has_new_epoch {
                        self.requeue_after_new_epoch(host_id)?;
                    } else {
                        self.block_host(host_id)?;
                    }
                }
            }
            drop(host);
            history_signal.notify()?;
        }

        // Append Source wake failures after host attempts. If a failed host
        // was already pending for unrelated work, the host-lock diagnostic
        // above is still useful, but the post-acceptance Source diagnostic
        // must remain the latest per-host error observed by the TypeScript
        // channel rather than being overwritten by that older attempt.
        report.errors.extend(source_wake_errors);
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        report.rearm = !environment.pending.is_empty();
        if !report.rearm {
            // Recheck under the same lock used by mark_host_pending so a
            // mutation cannot be lost between the empty check and latch
            // clear. Blocked work remains discoverable but is not runnable.
            environment.wake_latched = false;
            report.rearm = !environment.pending.is_empty();
            if report.rearm {
                environment.wake_latched = true;
            }
        }
        report.wake_epoch = environment.wake_epoch;
        Ok(report)
    }
}

impl Default for TuiEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

fn native_driver_loop(queue: EnvironmentQueue) {
    let inner = Arc::clone(&queue.inner);
    loop {
        let Ok(state) = inner.lock() else {
            return;
        };
        if state.shutdown {
            #[cfg(test)]
            signal_driver_exit(&state);
            return;
        }
        drop(state);
        let hosts = inner
            .lock()
            .map(|state| state.hosts.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut next_wake_ms = u64::MAX;
        for weak_host in hosts {
            let Some(host) = weak_host.upgrade() else {
                continue;
            };
            if let Ok(mut host) = host.lock() {
                let history_signal = host.history_work_signal();
                let wake_result = host.service_native_deadline_inner();
                drop(host);
                if history_signal.notify().is_err() {
                    return;
                }
                if let Ok(wake_ms) = wake_result {
                    next_wake_ms = next_wake_ms.min(wake_ms);
                }
            }
        }
        let report = queue.drain_pending(32, false);
        if report.is_err() {
            // A poisoned environment/host is blocked by the normal drain
            // policy. Keep the driver alive for a later explicit retry or a
            // new accepted wake; never spin on the same failed work.
            next_wake_ms = next_wake_ms.min(100);
        }
        let should_run_again = report.as_ref().is_ok_and(|report| report.rearm);
        if should_run_again {
            continue;
        }
        let timeout = if next_wake_ms == u64::MAX {
            None
        } else {
            Some(Duration::from_millis(next_wake_ms.max(1).min(1000)))
        };
        let Ok(state) = inner.lock() else {
            return;
        };
        if state.shutdown {
            #[cfg(test)]
            signal_driver_exit(&state);
            return;
        }
        if !state.pending.is_empty() {
            continue;
        }
        let wake = Arc::clone(&state.wake);
        if let Some(timeout) = timeout {
            let _ = wake.wait_timeout(state, timeout);
        } else {
            drop(wake.wait(state));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier, mpsc};

    use super::TuiEnvironment;

    #[test]
    fn final_environment_owners_shutdown_driver_after_concurrent_drop() {
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (driver_exit_tx, driver_exit_rx) = mpsc::channel();
        let environment = TuiEnvironment::new_with_shutdown_probes(shutdown_tx, driver_exit_tx);
        let barrier = Arc::new(Barrier::new(3));
        let first_environment = environment.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = std::thread::spawn(move || {
            first_barrier.wait();
            drop(first_environment);
        });
        let second_environment = environment.clone();
        let second_barrier = Arc::clone(&barrier);
        let second = std::thread::spawn(move || {
            second_barrier.wait();
            drop(second_environment);
        });
        barrier.wait();
        drop(environment);
        first.join().unwrap();
        second.join().unwrap();
        shutdown_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("environment lifetime must signal shutdown");
        driver_exit_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("environment driver must signal exit");
    }
}
