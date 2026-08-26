use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use super::{ComponentId, MountGraph, MountTransitions};
use crate::{
    component::ComponentRegistry,
    interaction::MountedCapabilities,
    output::{EventCx, OutputQueue},
};

trait TickDriver {
    fn tick(
        &mut self,
        handle: ComponentId,
        now: Instant,
        registry: &mut ComponentRegistry,
        cx: &mut EventCx<'_>,
    ) -> bool;
}

struct CapabilityTickDriver {
    callback: Arc<dyn for<'a> Fn(&mut dyn std::any::Any, Instant, &mut EventCx<'a>) -> bool>,
}

impl TickDriver for CapabilityTickDriver {
    fn tick(
        &mut self,
        handle: ComponentId,
        now: Instant,
        registry: &mut ComponentRegistry,
        cx: &mut EventCx<'_>,
    ) -> bool {
        registry
            .with_any_mut(handle, |component| (self.callback)(component, now, cx))
            .unwrap_or(false)
    }
}

struct TickRegistration {
    interval: Duration,
    next_due: Option<Instant>,
    driver: Box<dyn TickDriver>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TickOutcome {
    pub(crate) ran: bool,
    pub(crate) dirty: bool,
}

/// Private scheduler for mounted retained components.
pub(crate) struct TickScheduler {
    registrations: HashMap<ComponentId, TickRegistration>,
    mounted: HashSet<ComponentId>,
    mount_order: Vec<ComponentId>,
}

impl Default for TickScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TickScheduler {
    pub(crate) fn new() -> Self {
        Self {
            registrations: HashMap::new(),
            mounted: HashSet::new(),
            mount_order: Vec::new(),
        }
    }

    /// Synchronizes activation from the semantic mount graph.
    pub(crate) fn sync_mounts(
        &mut self,
        graph: &MountGraph,
        transitions: &MountTransitions,
        now: Instant,
    ) {
        for transition in &transitions.transitions {
            match transition {
                super::MountTransition::Mounted { id, .. } => {
                    self.mounted.insert(*id);
                    self.activate(*id, now);
                }
                super::MountTransition::Unmounted { id } => {
                    self.mounted.remove(id);
                    self.deactivate(*id);
                }
            }
        }

        let graph_ids: HashSet<_> = graph.ids().collect();
        for id in &graph_ids {
            if self.mounted.insert(*id) {
                self.activate(*id, now);
            } else if self
                .registrations
                .get(id)
                .is_some_and(|registration| registration.next_due.is_none())
            {
                self.activate(*id, now);
            }
        }
        for id in self.mounted.clone() {
            if !graph_ids.contains(&id) {
                self.mounted.remove(&id);
                self.deactivate(id);
            }
        }
        self.mount_order = graph.ids().collect();
    }

    /// Synchronizes the scheduler's current typed tick declarations with a
    /// successfully resolved mounted capability set.
    pub(crate) fn sync_capabilities(
        &mut self,
        graph: &MountGraph,
        capabilities: &MountedCapabilities,
        transitions: &MountTransitions,
        now: Instant,
    ) {
        self.sync_mounts(graph, transitions, now);

        let desired: HashMap<_, _> = graph
            .ids()
            .filter_map(|id| {
                capabilities
                    .get(id)
                    .and_then(|caps| caps.tick.clone().map(|tick| (id, tick)))
            })
            .collect();

        for (id, tick) in &desired {
            if tick.interval.is_zero() {
                continue;
            }
            let driver = Box::new(CapabilityTickDriver {
                callback: tick.handler.clone(),
            });
            match self.registrations.get_mut(id) {
                Some(registration) => {
                    let interval_changed = registration.interval != tick.interval;
                    registration.interval = tick.interval;
                    registration.driver = driver;
                    if interval_changed {
                        registration.next_due = None;
                        self.activate(*id, now);
                    } else if registration.next_due.is_none() {
                        self.activate(*id, now);
                    }
                }
                None => {
                    self.registrations.insert(
                        *id,
                        TickRegistration {
                            interval: tick.interval,
                            next_due: None,
                            driver,
                        },
                    );
                    self.activate(*id, now);
                }
            }
        }

        let stale: Vec<_> = self
            .registrations
            .iter()
            .filter(|(id, _)| !desired.contains_key(id))
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            self.registrations.remove(&id);
        }
    }

    /// Updates one already-mounted component without rescanning the complete
    /// mount graph. This is used after a topology-preserving local snapshot
    /// replacement.
    pub(crate) fn sync_component_capability(
        &mut self,
        id: ComponentId,
        capabilities: &MountedCapabilities,
        now: Instant,
    ) {
        let Some(tick) = capabilities.get(id).and_then(|caps| caps.tick.clone()) else {
            self.registrations.remove(&id);
            return;
        };
        if tick.interval.is_zero() {
            self.registrations.remove(&id);
            return;
        }
        let driver = Box::new(CapabilityTickDriver {
            callback: tick.handler.clone(),
        });
        match self.registrations.get_mut(&id) {
            Some(registration) => {
                let interval_changed = registration.interval != tick.interval;
                registration.interval = tick.interval;
                registration.driver = driver;
                if interval_changed || registration.next_due.is_none() {
                    self.activate(id, now);
                }
            }
            None => {
                self.registrations.insert(
                    id,
                    TickRegistration {
                        interval: tick.interval,
                        next_due: None,
                        driver,
                    },
                );
                self.activate(id, now);
            }
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.mount_order
            .iter()
            .filter_map(|id| self.registrations.get(id))
            .filter_map(|registration| registration.next_due)
            .min()
    }

    #[cfg(test)]
    pub(crate) fn next_timeout(&self, now: Instant, idle_timeout: Duration) -> Duration {
        self.next_deadline()
            .map(|deadline| deadline.checked_duration_since(now).unwrap_or_default())
            .unwrap_or(idle_timeout)
    }

    pub(crate) fn tick_due_with_events(
        &mut self,
        now: Instant,
        registry: &mut ComponentRegistry,
        queue: &mut OutputQueue,
    ) -> TickOutcome {
        if self.registrations.is_empty() {
            return TickOutcome {
                ran: false,
                dirty: false,
            };
        }
        let due: Vec<_> = self
            .mount_order
            .iter()
            .copied()
            .filter(|id| {
                self.registrations
                    .get(id)
                    .and_then(|registration| registration.next_due)
                    .is_some_and(|deadline| deadline <= now)
            })
            .collect();

        let mut dirty = false;
        let mut ran = false;
        let mut cx = queue.event_cx();
        for id in due {
            let Some(registration) = self.registrations.get_mut(&id) else {
                continue;
            };
            ran = true;
            dirty |= registration.driver.tick(id, now, registry, &mut cx);
            registration.next_due = Some(
                now.checked_add(registration.interval)
                    .expect("component tick deadline exhausted"),
            );
        }
        TickOutcome { ran, dirty }
    }

    fn activate(&mut self, id: ComponentId, now: Instant) {
        if let Some(registration) = self.registrations.get_mut(&id) {
            registration.next_due = Some(
                now.checked_add(registration.interval)
                    .expect("component tick deadline exhausted"),
            );
        }
    }

    fn deactivate(&mut self, id: ComponentId) {
        if let Some(registration) = self.registrations.get_mut(&id) {
            registration.next_due = None;
        }
    }
}
