//! Host-owned `ViewState` identity, binding, lifecycle, and version registry.
//!
//! Records live directly in the registry behind the host owner; wrappers
//! carry the immutable identity plus a weak host reference instead of a
//! second lock. Every access is serialized through the host lock, so no
//! per-record mutex remains. Each record publishes an immutable shared
//! version only when its logical values change; a frame capture owns one
//! candidate overlay over the committed version table and never snapshots
//! unrelated unmounted states.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::capabilities::{StateNodeKind, validate_geometry_for_kind};
use super::capture::StateCandidateOverlay;
use super::effects::StateEffects;
use super::presentation::ViewStateSnapshot;
use super::record::{ViewStateLifecycle, ViewStateRecord};

#[derive(Debug, Default)]
pub(crate) struct ViewStateRegistry {
    records: HashMap<u64, ViewStateRecord>,
    /// Committed immutable version per live record. Published at creation and
    /// on every accepted mutation; read by frames through the candidate
    /// overlay without cloning unrelated records.
    committed: HashMap<u64, Arc<ViewStateSnapshot>>,
    /// Deduplicated dirty-state worklist. An id appears at most once no
    /// matter how many accepted mutations land before the next capture.
    dirty: HashSet<u64>,
    /// Capture epoch. Each capture drains the dirty marks it serves; marks
    /// for records outside the demanded set stay queued.
    capture_epoch: u64,
    next_id: u64,
    desired: HashSet<u64>,
    visible: HashSet<u64>,
    in_flight: HashSet<u64>,
}

/// Candidate-owned visible/in-flight transitions. The vectors are prepared
/// before a backend receipt; receipt-time promotion only applies those
/// precomputed transitions and never builds a replacement HashSet/Vec.
#[derive(Debug)]
pub(crate) struct PreparedStateCommit {
    pub(crate) visible_targets: Vec<(u64, StateNodeKind)>,
    pub(crate) removed_visible: Vec<u64>,
    pub(crate) in_flight_ids: Vec<u64>,
}

impl ViewStateRegistry {
    pub(crate) fn new() -> Self {
        Self {
            records: HashMap::new(),
            committed: HashMap::new(),
            dirty: HashSet::new(),
            capture_epoch: 0,
            next_id: 1,
            desired: HashSet::new(),
            visible: HashSet::new(),
            in_flight: HashSet::new(),
        }
    }

    pub(crate) fn create(&mut self, host_id: u64) -> anyhow::Result<u64> {
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
        let record = ViewStateRecord::new(id);
        // Publish the initial version so a newly attached or remounted state
        // resolves even when it was never mutated since creation.
        self.committed.insert(id, Arc::new(record.snapshot()));
        self.records.insert(id, record);
        Ok(id)
    }

    /// Applies one validated mutation atomically, then publishes the new
    /// immutable version and queues the id on the deduplicated dirty
    /// worklist. No-op writes return without publishing, so versions advance
    /// only when logical values change. A removed record reads as disposed:
    /// removal always follows disposal, and the wrapper outlives only the
    /// host-owned entry, never the lifecycle.
    pub(crate) fn mutate_record<F>(&mut self, id: u64, mutation: F) -> anyhow::Result<StateEffects>
    where
        F: FnOnce(&mut ViewStateRecord) -> anyhow::Result<StateEffects>,
    {
        let effects = {
            let Some(record) = self.records.get_mut(&id) else {
                return Err(anyhow::anyhow!("STATE_DISPOSED: ViewState is disposed"));
            };
            if record.lifecycle == ViewStateLifecycle::Disposed {
                return Err(anyhow::anyhow!("STATE_DISPOSED: ViewState is disposed"));
            }
            mutation(record)?
        };
        if effects.is_empty() {
            return Ok(effects);
        }
        let snapshot = self
            .records
            .get(&id)
            .map(ViewStateRecord::snapshot)
            .expect("mutated record is still owned by the registry");
        self.committed.insert(id, Arc::new(snapshot));
        self.dirty.insert(id);
        Ok(effects)
    }

    /// Captures one candidate overlay over the committed version table. The
    /// overlay owns immutable versions for changed-through-epoch ids and for
    /// newly demanded (desired-but-not-yet-visible) ids; clean unchanged ids
    /// are read from the committed table through the frame view. Unmounted
    /// records are never visited, and their dirty marks stay queued until a
    /// mount demands them.
    pub(crate) fn capture_candidate(&mut self) -> StateCandidateOverlay {
        self.capture_epoch = self.capture_epoch.saturating_add(1);
        let epoch = self.capture_epoch;
        let demanded_set: HashSet<u64> = self
            .desired
            .iter()
            .chain(self.visible.iter())
            .chain(self.in_flight.iter())
            .copied()
            .collect();
        let mut demanded: Vec<u64> = demanded_set.iter().copied().collect();
        demanded.sort_unstable();
        let mut touched = HashMap::new();
        for id in &demanded {
            let changed = self.dirty.contains(id);
            let newly_demanded = self.desired.contains(id) && !self.visible.contains(id);
            if !changed && !newly_demanded {
                continue;
            }
            if let Some(version) = self.committed.get(id) {
                touched.insert(*id, Arc::clone(version));
            }
        }
        self.dirty.retain(|id| !demanded_set.contains(id));
        StateCandidateOverlay::new(epoch, demanded, touched)
    }

    pub(crate) fn committed_table(&self) -> &HashMap<u64, Arc<ViewStateSnapshot>> {
        &self.committed
    }

    pub(crate) fn record(&self, id: u64) -> Option<&ViewStateRecord> {
        self.records.get(&id)
    }

    /// Single-pass identity plus kind validation. The previous two-pass shape
    /// locked every record twice; records are host-owned now, so one lookup
    /// per target validates liveness and kind geometry together.
    pub(crate) fn validate_targets(&self, targets: &[(u64, StateNodeKind)]) -> anyhow::Result<()> {
        for (id, kind) in targets {
            let Some(record) = self.records.get(id) else {
                return Err(anyhow::anyhow!("ViewState identity is unavailable: {id}"));
            };
            if record.lifecycle == ViewStateLifecycle::Disposed {
                return Err(anyhow::anyhow!("ViewState is disposed: {id}"));
            }
            validate_geometry_for_kind(*kind, &record.geometry)?;
        }
        Ok(())
    }

    pub(crate) fn set_desired(&mut self, targets: &[(u64, StateNodeKind)]) -> anyhow::Result<()> {
        self.validate_targets(targets)?;
        let next = targets.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
        for id in self.desired.difference(&next).copied().collect::<Vec<_>>() {
            self.set_bound(&id, false, true, None);
        }
        for (id, kind) in targets {
            self.set_bound(id, true, true, Some(*kind));
        }
        self.desired = next;
        Ok(())
    }

    /// Two-phase binding preparation: every candidate id validates live
    /// before any binding mutates, so a failed commit never installs a
    /// partial or ghost binding set.
    pub(crate) fn set_visible(&mut self, targets: &[(u64, StateNodeKind)]) -> anyhow::Result<()> {
        let prepared = self.prepare_visible(targets)?;
        self.commit_visible_prepared(&prepared);
        Ok(())
    }

    /// Prepares visible transitions and reserves the only shared-set growth
    /// required by the eventual commit. The returned table is independent of
    /// newer desired operations accepted while the backend receipt is pending.
    pub(crate) fn prepare_visible(
        &mut self,
        targets: &[(u64, StateNodeKind)],
    ) -> anyhow::Result<PreparedStateCommit> {
        self.validate_targets(targets)?;
        let mut target_ids = HashSet::with_capacity(targets.len());
        for (id, _) in targets {
            target_ids.insert(*id);
        }
        let mut removed_visible = self
            .visible
            .iter()
            .filter(|id| !target_ids.contains(id))
            .copied()
            .collect::<Vec<_>>();
        removed_visible.sort_unstable();
        let added_visible = targets
            .iter()
            .filter(|(id, _)| !self.visible.contains(id))
            .count();
        self.visible.reserve(added_visible);
        Ok(PreparedStateCommit {
            visible_targets: targets.to_vec(),
            removed_visible,
            in_flight_ids: Vec::new(),
        })
    }

    /// Prepares a complete frame state transition, including the in-flight
    /// lifecycle pin. All allocations and validation occur before receipt.
    pub(crate) fn prepare_candidate(
        &mut self,
        targets: &[(u64, StateNodeKind)],
    ) -> anyhow::Result<PreparedStateCommit> {
        let mut prepared = self.prepare_visible(targets)?;
        prepared.in_flight_ids = targets.iter().map(|(id, _)| *id).collect();
        self.set_in_flight(&prepared.in_flight_ids);
        Ok(prepared)
    }

    /// Applies the prevalidated visible table without allocating. The visible
    /// set capacity was reserved by `prepare_visible`, and no other operation
    /// mutates visible membership while a candidate receipt is outstanding.
    pub(crate) fn commit_visible_prepared(&mut self, prepared: &PreparedStateCommit) {
        for id in &prepared.removed_visible {
            self.set_bound(id, false, false, None);
            self.visible.remove(id);
        }
        for (id, kind) in &prepared.visible_targets {
            self.set_bound(id, true, false, Some(*kind));
            self.visible.insert(*id);
        }
    }

    /// Commits a complete frame state transition without constructing any
    /// receipt-time collections.
    pub(crate) fn commit_prepared(&mut self, prepared: &PreparedStateCommit) {
        self.commit_visible_prepared(prepared);
        self.clear_in_flight_prepared(&prepared.in_flight_ids);
    }

    /// Clears exactly the in-flight IDs captured by one candidate. HashSet
    /// removal does not grow or allocate, and newer desired state remains
    /// independent in its own table.
    pub(crate) fn clear_in_flight_prepared(&mut self, ids: &[u64]) {
        for id in ids {
            self.set_in_flight_bound(id, false);
            self.in_flight.remove(id);
        }
    }

    pub(crate) fn set_in_flight(&mut self, ids: &[u64]) {
        debug_assert!(ids.iter().all(|id| self.desired.contains(id)));
        let next = ids.iter().copied().collect::<HashSet<_>>();
        for id in self
            .in_flight
            .difference(&next)
            .copied()
            .collect::<Vec<_>>()
        {
            self.set_in_flight_bound(&id, false);
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
        if record.lifecycle == ViewStateLifecycle::Disposed {
            return Err(anyhow::anyhow!("ViewState is disposed: {id}"));
        }
        Ok(record.desired_bound || record.visible_bound || record.in_flight_bound)
    }

    /// Disposes one record by identity. Unknown ids stay a no-op so repeated
    /// disposal is idempotent; bound records (desired, visible, or in-flight)
    /// are rejected so a prepared frame's pins survive until commit or an
    /// explicit candidate discard.
    pub(crate) fn dispose(&mut self, id: u64) -> anyhow::Result<()> {
        let Some(record) = self.records.get_mut(&id) else {
            return Ok(());
        };
        if record.lifecycle == ViewStateLifecycle::Disposed {
            return Ok(());
        }
        if record.desired_bound || record.visible_bound || record.in_flight_bound {
            return Err(anyhow::anyhow!(
                "STATE_MOUNTED: ViewState is still attached"
            ));
        }
        record.dispose();
        self.remove(id);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: u64) {
        self.desired.remove(&id);
        self.visible.remove(&id);
        self.in_flight.remove(&id);
        self.dirty.remove(&id);
        self.committed.remove(&id);
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
            self.set_bound(&id, false, true, None);
            self.set_bound(&id, false, false, None);
        }
        self.desired.clear();
        self.visible.clear();
        self.clear_in_flight();
    }

    pub(crate) fn dispose_all(&mut self) {
        self.clear_bindings();
        for record in self.records.values_mut() {
            record.dispose();
        }
        self.dirty.clear();
        self.committed.clear();
        self.records.clear();
    }

    fn set_bound(&mut self, id: &u64, bound: bool, desired: bool, kind: Option<StateNodeKind>) {
        if let Some(record) = self.records.get_mut(id) {
            if desired {
                record.desired_bound = bound;
                record.desired_kind = if bound { kind } else { None };
            } else {
                record.visible_bound = bound;
                record.visible_kind = if bound { kind } else { None };
            }
        }
    }

    fn set_in_flight_bound(&mut self, id: &u64, bound: bool) {
        if let Some(record) = self.records.get_mut(id) {
            record.in_flight_bound = bound;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::ColorSpec;
    use crate::retained_state::ViewStatePresentationPatch;

    fn accept_presentation(registry: &mut ViewStateRegistry, id: u64, color: u8) {
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(color)));
        let effects = registry
            .mutate_record(id, |record| Ok(record.apply_presentation(&patch)))
            .expect("mutation is accepted");
        assert!(!effects.is_empty());
    }

    fn noop_presentation(registry: &mut ViewStateRegistry, id: u64, color: u8) {
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(color)));
        let effects = registry
            .mutate_record(id, |record| Ok(record.apply_presentation(&patch)))
            .expect("mutation runs");
        assert!(effects.is_empty());
    }

    #[test]
    fn registry_allocates_generationally_unique_monotonic_ids() {
        let mut registry = ViewStateRegistry::new();
        let first = registry.create(1).unwrap();
        let second = registry.create(1).unwrap();
        assert_eq!((first, second), (0x1_0000_0001, 0x1_0000_0002));
        assert_eq!(registry.records.len(), 2);
    }

    #[test]
    fn creation_publishes_an_initial_version_without_dirtying() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        assert!(registry.committed_table().contains_key(&id));
        assert!(registry.dirty.is_empty());
    }

    #[test]
    fn dirty_marks_deduplicate_and_capture_drains_once() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        registry.set_desired(&[(id, StateNodeKind::Text)]).unwrap();
        registry.set_visible(&[(id, StateNodeKind::Text)]).unwrap();
        accept_presentation(&mut registry, id, 1);
        accept_presentation(&mut registry, id, 2);
        accept_presentation(&mut registry, id, 3);
        assert_eq!(registry.dirty.len(), 1);
        let overlay = registry.capture_candidate();
        assert_eq!(overlay.demanded(), &[id]);
        assert!(overlay.contains_touched(id));
        assert_eq!(overlay.touched_len(), 1);
        assert!(registry.dirty.is_empty());
        let second = registry.capture_candidate();
        assert_eq!(second.touched_len(), 0);
        assert!(second.epoch() > overlay.epoch());
        // A repeated identical override publishes no new version and
        // schedules no work, even though the override stays stored.
        noop_presentation(&mut registry, id, 3);
        assert!(registry.dirty.is_empty());
        let third = registry.capture_candidate();
        assert_eq!(third.touched_len(), 0);
    }

    #[test]
    fn capture_ignores_unmounted_states() {
        let mut registry = ViewStateRegistry::new();
        let bound = registry.create(1).unwrap();
        let loose = registry.create(1).unwrap();
        registry
            .set_desired(&[(bound, StateNodeKind::Text)])
            .unwrap();
        registry
            .set_visible(&[(bound, StateNodeKind::Text)])
            .unwrap();
        // Mutate the bound record and configure the unmounted one.
        accept_presentation(&mut registry, bound, 4);
        accept_presentation(&mut registry, loose, 5);
        let overlay = registry.capture_candidate();
        // One paint-state mutation visits exactly its own attachment.
        assert_eq!(overlay.demanded(), &[bound]);
        assert_eq!(overlay.touched_len(), 1);
        assert!(overlay.contains_touched(bound));
        // The unmounted mark stays queued and its version stays current.
        assert!(registry.dirty.contains(&loose));
        assert_eq!(
            registry
                .committed_table()
                .get(&loose)
                .expect("version exists")
                .presentation
                .foreground,
            Some(Some(ColorSpec::ansi(5)))
        );
    }

    #[test]
    fn newly_demanded_states_capture_without_prior_mutation() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        // Configure while unmounted, then demand without any new mutation.
        accept_presentation(&mut registry, id, 6);
        // Simulate the unmounted window: the dirty mark is queued but the
        // record is not demanded yet.
        let idle = registry.capture_candidate();
        assert_eq!(idle.touched_len(), 0);
        registry.set_desired(&[(id, StateNodeKind::Text)]).unwrap();
        let overlay = registry.capture_candidate();
        assert!(overlay.contains_touched(id));
        assert_eq!(
            overlay.touched_len(),
            1,
            "mount captures the newly demanded version"
        );
    }

    #[test]
    fn captured_overlay_pins_old_versions_across_later_mutations() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        registry.set_desired(&[(id, StateNodeKind::Text)]).unwrap();
        registry.set_visible(&[(id, StateNodeKind::Text)]).unwrap();
        accept_presentation(&mut registry, id, 1);
        let overlay = registry.capture_candidate();
        // A failed frame holds `overlay` while a newer desired revision lands.
        accept_presentation(&mut registry, id, 2);
        assert_eq!(overlay.touched_len(), 1);
        let view = super::super::capture::StateFrameView::new(registry.committed_table(), &overlay);
        // The pinned candidate still reads the old version; live reads see
        // the newer desired revision.
        assert_eq!(
            view.get(&id)
                .expect("pinned version")
                .presentation
                .foreground,
            Some(Some(ColorSpec::ansi(1)))
        );
        assert_eq!(
            registry
                .committed_table()
                .get(&id)
                .expect("committed version")
                .presentation
                .foreground,
            Some(Some(ColorSpec::ansi(2)))
        );
    }

    #[test]
    fn clear_after_override_reveals_base() {
        use crate::retained_state::ViewStatePresentationProperty;
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        registry.set_desired(&[(id, StateNodeKind::Text)]).unwrap();
        registry.set_visible(&[(id, StateNodeKind::Text)]).unwrap();
        accept_presentation(&mut registry, id, 3);
        let effects = registry
            .mutate_record(id, |record| {
                Ok(record.clear_presentation(Some(&[ViewStatePresentationProperty::Foreground])))
            })
            .expect("clear is accepted");
        assert!(!effects.is_empty());
        let overlay = registry.capture_candidate();
        assert!(overlay.contains_touched(id));
        assert_eq!(
            registry
                .committed_table()
                .get(&id)
                .expect("version exists")
                .presentation
                .foreground,
            None
        );
    }

    #[test]
    fn dispose_rejects_bound_records_and_stays_idempotent() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        registry.set_desired(&[(id, StateNodeKind::Text)]).unwrap();
        assert!(registry.dispose(id).is_err());
        registry.set_in_flight(&[id]);
        assert!(registry.dispose(id).is_err());
        registry.clear_in_flight();
        registry.clear_bindings();
        registry.dispose(id).unwrap();
        // No stale version or dirty mark survives disposal.
        assert!(!registry.committed_table().contains_key(&id));
        assert!(!registry.dirty.contains(&id));
        assert!(registry.record(id).is_none());
        // Unknown and repeated disposal are idempotent.
        registry.dispose(id).unwrap();
        registry.dispose(0x1_ffff_ffff).unwrap();
        // Monotonic ids never revive the disposed identity.
        let next = registry.create(1).unwrap();
        assert_ne!(next, id);
        let overlay = registry.capture_candidate();
        assert!(!overlay.contains_touched(id));
    }

    #[test]
    fn visible_bindings_validate_before_commit() {
        let mut registry = ViewStateRegistry::new();
        let id = registry.create(1).unwrap();
        let before = registry.visible.clone();
        assert!(
            registry
                .set_visible(&[(id, StateNodeKind::Text), (77, StateNodeKind::Text)])
                .is_err()
        );
        assert_eq!(registry.visible, before);
        assert!(!registry.visible.contains(&id));
    }
}
