//! Native environment scheduler for PERF-13's shared host wake seam.
//!
//! The environment owns pending-host fairness and the edge-trigger latch. It
//! does not own semantic View structure, retained state, or terminal paint.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, Weak};

use super::host::HostInner;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HostFlushOutcome {
    pub(super) committed: bool,
    pub(super) waiting_for_presentation: bool,
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
pub struct HostDrainReport {
    pub rearm: bool,
    pub attempted: usize,
    pub committed_hosts: Vec<u64>,
    pub errors: Vec<HostFrameError>,
    pub wake_epoch: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeDisposition {
    pub schedule_environment_drain: bool,
}

/// One native environment owns the pending-host set and wake latch shared by
/// all hosts created in that environment. The queue stores IDs only; host
/// state remains authoritative in each registered host.
#[derive(Clone)]
pub struct TuiEnvironment {
    inner: Arc<Mutex<EnvironmentInner>>,
}

// The host runtime already exposes `TuiHost` as a serialized Send/Sync
// boundary. Environment operations never access a host without taking its
// mutex; the unsafe markers make the shared pending-host registry usable by
// the same N-API boundary without moving component callbacks out of the host
// lock's ownership model.
unsafe impl Send for TuiEnvironment {}
unsafe impl Sync for TuiEnvironment {}

struct EnvironmentInner {
    next_host_id: u64,
    hosts: HashMap<u64, Weak<Mutex<HostInner>>>,
    pending: VecDeque<u64>,
    pending_set: HashSet<u64>,
    queued: HashSet<u64>,
    retry_blocked: HashSet<u64>,
    wake_latched: bool,
    wake_epoch: u64,
}

impl TuiEnvironment {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EnvironmentInner {
                next_host_id: 1,
                hosts: HashMap::new(),
                pending: VecDeque::new(),
                pending_set: HashSet::new(),
                queued: HashSet::new(),
                retry_blocked: HashSet::new(),
                wake_latched: false,
                wake_epoch: 0,
            })),
        }
    }

    pub(super) fn register_host(&self, host: &Arc<Mutex<HostInner>>) -> anyhow::Result<u64> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        let id = environment.next_host_id;
        environment.next_host_id = environment
            .next_host_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("host identity exhausted"))?;
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
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
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
        environment.pending_set.insert(host_id);
        Self::queue_host(&mut environment, host_id);
        Ok(WakeDisposition {
            schedule_environment_drain: schedule,
        })
    }

    pub(super) fn complete_host(
        &self,
        host_id: u64,
        pending_epoch: u64,
        committed_epoch: u64,
        requeue_if_pending: bool,
    ) -> anyhow::Result<()> {
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        if !environment.hosts.contains_key(&host_id) {
            return Ok(());
        }
        if pending_epoch == committed_epoch {
            environment.pending_set.remove(&host_id);
            environment.retry_blocked.remove(&host_id);
            environment.queued.remove(&host_id);
            environment.pending.retain(|id| *id != host_id);
        } else {
            environment.pending_set.insert(host_id);
            if requeue_if_pending {
                Self::queue_host(&mut environment, host_id);
            } else {
                environment.queued.remove(&host_id);
                environment.pending.retain(|id| *id != host_id);
            }
        }
        if environment.pending.is_empty() {
            environment.wake_latched = false;
        }
        Ok(())
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
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
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
        environment.queued.remove(&host_id);
        environment.pending.retain(|id| *id != host_id);
        environment.wake_latched = true;
        Self::queue_host(&mut environment, host_id);
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
        let mut report = HostDrainReport::default();
        let budget = budget.max(1);
        let mut candidates = Vec::new();
        let mut environment = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("environment lock is poisoned"))?;
        report.wake_epoch = environment.wake_epoch;
        if force_retry {
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
            if let Some(host_id) = preferred_host_id {
                Self::prioritize_host(&mut environment, host_id);
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
            let Some(host) = weak.upgrade() else {
                self.unregister_host(host_id);
                continue;
            };
            let mut attempted_epoch = 0;
            let result = host
                .lock()
                .map_err(|_| anyhow::anyhow!("host lock is poisoned"))
                .and_then(|mut host| {
                    attempted_epoch = host.environment_pending_epoch();
                    host.flush_for_environment()
                });
            match result {
                Ok((outcome, pending_epoch, committed_epoch)) => {
                    if outcome.committed {
                        report.committed_hosts.push(host_id);
                    }
                    self.complete_host(
                        host_id,
                        pending_epoch,
                        committed_epoch,
                        !outcome.waiting_for_presentation,
                    )?;
                }
                Err(error) => {
                    let host = host
                        .lock()
                        .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
                    let failure = error.downcast_ref::<HostAttemptError>();
                    let (pending_epoch, desired_revision) = host.environment_error_epochs();
                    report.errors.push(HostFrameError {
                        host_id,
                        attempted_epoch: pending_epoch,
                        desired_revision,
                        phase: failure
                            .map_or_else(|| "frame".to_owned(), |failure| failure.phase.to_owned()),
                        code: failure.map_or_else(
                            || "FRAME_PREPARATION_FAILED".to_owned(),
                            |failure| failure.code.to_owned(),
                        ),
                        retryable: failure.is_none_or(|failure| failure.retryable),
                        diagnostic: error.to_string(),
                    });
                    let has_new_epoch = pending_epoch != attempted_epoch;
                    drop(host);
                    if has_new_epoch {
                        self.requeue_after_new_epoch(host_id)?;
                    } else {
                        self.block_host(host_id)?;
                    }
                }
            }
        }

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
