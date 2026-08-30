//! Host-owned ViewState identity, binding, and lifecycle registry.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use super::presentation::ViewStateSnapshot;
use super::record::{ViewStateLifecycle, ViewStateRecord};

#[derive(Debug, Default)]
pub(crate) struct ViewStateRegistry {
    pub(crate) records: HashMap<u64, Arc<Mutex<ViewStateRecord>>>,
    next_id: u64,
    desired: HashSet<u64>,
    visible: HashSet<u64>,
    in_flight: HashSet<u64>,
}

impl ViewStateRegistry {
    pub(crate) fn new() -> Self {
        Self {
            records: HashMap::new(),
            next_id: 1,
            desired: HashSet::new(),
            visible: HashSet::new(),
            in_flight: HashSet::new(),
        }
    }

    pub(crate) fn create(
        &mut self,
        host_id: u64,
    ) -> anyhow::Result<(u64, Arc<Mutex<ViewStateRecord>>)> {
        // State identities are carried through the structural native View as
        // one scalar. Keep the host namespace and local slot in a safe u64 so
        // a state from another host cannot alias this host's slot.
        let local_id = self.next_id;
        if host_id == 0 || host_id > 0x001f_ffff || local_id == 0 || local_id > u32::MAX as u64 {
            return Err(anyhow::anyhow!("ViewState identity exhausted"));
        }
        let id = (host_id << 32) | local_id;
        self.next_id = local_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("ViewState identity exhausted"))?;
        let record = Arc::new(Mutex::new(ViewStateRecord::new(id)));
        self.records.insert(id, Arc::clone(&record));
        Ok((id, record))
    }

    pub(crate) fn snapshots(&self) -> anyhow::Result<HashMap<u64, ViewStateSnapshot>> {
        self.records
            .iter()
            .map(|(id, record)| {
                record
                    .lock()
                    .map(|record| (*id, record.snapshot()))
                    .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))
            })
            .collect()
    }

    pub(crate) fn validate_ids(&self, ids: &[u64]) -> anyhow::Result<()> {
        for id in ids {
            let Some(record) = self.records.get(id) else {
                return Err(anyhow::anyhow!("ViewState identity is unavailable: {id}"));
            };
            let record = record
                .lock()
                .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?;
            if record.lifecycle == ViewStateLifecycle::Disposed {
                return Err(anyhow::anyhow!("ViewState is disposed: {id}"));
            }
        }
        Ok(())
    }

    pub(crate) fn set_desired(&mut self, ids: &[u64]) -> anyhow::Result<()> {
        self.validate_ids(ids)?;
        let next = ids.iter().copied().collect::<HashSet<_>>();
        for id in self.desired.difference(&next) {
            self.set_bound(id, false, true);
        }
        for id in &next {
            self.set_bound(id, true, true);
        }
        self.desired = next;
        Ok(())
    }

    pub(crate) fn set_visible(&mut self, ids: &[u64]) {
        debug_assert!(ids.iter().all(|id| self.records.contains_key(id)));
        let next = ids.iter().copied().collect::<HashSet<_>>();
        for id in self.visible.difference(&next) {
            self.set_bound(id, false, false);
        }
        for id in &next {
            self.set_bound(id, true, false);
        }
        self.visible = next;
    }

    pub(crate) fn set_in_flight(&mut self, ids: &[u64]) {
        debug_assert!(ids.iter().all(|id| self.desired.contains(id)));
        let next = ids.iter().copied().collect::<HashSet<_>>();
        for id in self.in_flight.difference(&next) {
            self.set_in_flight_bound(id, false);
        }
        for id in &next {
            self.set_in_flight_bound(id, true);
        }
        self.in_flight = next;
    }

    pub(crate) fn clear_in_flight(&mut self) {
        for id in self.in_flight.iter().copied().collect::<Vec<_>>() {
            self.set_in_flight_bound(&id, false);
        }
        self.in_flight.clear();
    }

    pub(crate) fn is_bound(&self, id: u64) -> anyhow::Result<bool> {
        let Some(record) = self.records.get(&id) else {
            return Err(anyhow::anyhow!("ViewState identity is unavailable: {id}"));
        };
        let record = record
            .lock()
            .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?;
        if record.lifecycle == ViewStateLifecycle::Disposed {
            return Err(anyhow::anyhow!("ViewState is disposed: {id}"));
        }
        Ok(record.desired_bound || record.visible_bound || record.in_flight_bound)
    }

    pub(crate) fn remove(&mut self, id: u64) {
        self.desired.remove(&id);
        self.visible.remove(&id);
        self.in_flight.remove(&id);
        self.records.remove(&id);
    }

    pub(crate) fn clear_bindings(&mut self) {
        for id in self
            .desired
            .iter()
            .chain(self.visible.iter())
            .copied()
            .collect::<HashSet<_>>()
        {
            self.set_bound(&id, false, true);
            self.set_bound(&id, false, false);
        }
        self.desired.clear();
        self.visible.clear();
        self.clear_in_flight();
    }

    pub(crate) fn dispose_all(&mut self) {
        self.clear_bindings();
        for record in self.records.values() {
            if let Ok(mut record) = record.lock() {
                record.lifecycle = ViewStateLifecycle::Disposed;
            }
        }
        self.records.clear();
    }

    fn set_bound(&self, id: &u64, bound: bool, desired: bool) {
        if let Some(record) = self.records.get(id)
            && let Ok(mut record) = record.lock()
        {
            if desired {
                record.desired_bound = bound;
            } else {
                record.visible_bound = bound;
            }
        }
    }

    fn set_in_flight_bound(&self, id: &u64, bound: bool) {
        if let Some(record) = self.records.get(id)
            && let Ok(mut record) = record.lock()
        {
            record.in_flight_bound = bound;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_allocates_generationally_unique_monotonic_ids() {
        let mut registry = ViewStateRegistry::new();
        let (first, _) = registry.create(1).unwrap();
        let (second, _) = registry.create(1).unwrap();
        assert_eq!((first, second), (0x1_0000_0001, 0x1_0000_0002));
        assert_eq!(registry.records.len(), 2);
    }
}
