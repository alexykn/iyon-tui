//! Host-owned retained-state records and lifecycle revisions.

use std::collections::BTreeMap;

use super::capabilities::{StateNodeKind, validate_geometry_for_kind};
use super::effects::{StateEffects, presentation_effects};
use super::geometry::{GeometryOverrides, ViewStateGeometryPatch, ViewStateGeometryProperty};
use super::presentation::{
    PresentationOverrides, ViewStatePresentationPatch, ViewStatePresentationProperty,
    ViewStateSnapshot,
};

/// Host-owned mutable state record. The host keeps the `Arc` while the native
/// wrapper is live, so wrapper GC cannot invalidate a mounted occurrence.
#[derive(Debug)]
pub(crate) struct ViewStateRecord {
    pub(crate) id: u64,
    pub(crate) lifecycle: ViewStateLifecycle,
    pub(crate) desired_bound: bool,
    pub(crate) desired_kind: Option<StateNodeKind>,
    pub(crate) visible_bound: bool,
    pub(crate) visible_kind: Option<StateNodeKind>,
    /// Keeps a prepared frame's attachment alive while its backend receipt is
    /// outstanding, even if a newer desired revision supersedes it.
    pub(crate) in_flight_bound: bool,
    pub(crate) geometry: GeometryOverrides,
    pub(crate) presentation: PresentationOverrides,
    pub(crate) style_states: BTreeMap<String, String>,
    pub(crate) revision: u64,
    pub(crate) geometry_revision: u64,
    pub(crate) presentation_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewStateLifecycle {
    Live,
    Disposed,
}

impl ViewStateRecord {
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            lifecycle: ViewStateLifecycle::Live,
            desired_bound: false,
            desired_kind: None,
            visible_bound: false,
            visible_kind: None,
            in_flight_bound: false,
            geometry: GeometryOverrides::default(),
            presentation: PresentationOverrides::default(),
            style_states: BTreeMap::new(),
            revision: 0,
            geometry_revision: 0,
            presentation_revision: 0,
        }
    }

    pub(crate) fn snapshot(&self) -> ViewStateSnapshot {
        ViewStateSnapshot {
            id: self.id,
            geometry: self.geometry.clone(),
            presentation: self.presentation.clone(),
            style_states: self.style_states.clone(),
            revision: self.revision,
            geometry_revision: self.geometry_revision,
            presentation_revision: self.presentation_revision,
        }
    }

    pub(crate) fn apply_geometry(
        &mut self,
        patch: &ViewStateGeometryPatch,
    ) -> anyhow::Result<StateEffects> {
        let mut next = self.geometry.clone();
        let effects = next.apply_patch(patch);
        if effects.is_empty() {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return Ok(StateEffects::NONE);
        }
        if let Some(kind) = self.desired_kind.or(self.visible_kind) {
            validate_geometry_for_kind(kind, &next)?;
        }
        self.geometry = next;
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        crate::perf::inc(crate::perf::Counter::ViewStateGeometryInvalidations);
        self.revision = self.revision.saturating_add(1);
        self.geometry_revision = self.geometry_revision.saturating_add(1);
        Ok(effects)
    }

    pub(crate) fn clear_geometry(
        &mut self,
        properties: Option<&[ViewStateGeometryProperty]>,
    ) -> anyhow::Result<StateEffects> {
        let mut next = self.geometry.clone();
        let effects = next.clear(properties);
        if effects.is_empty() {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return Ok(StateEffects::NONE);
        }
        if let Some(kind) = self.desired_kind.or(self.visible_kind) {
            validate_geometry_for_kind(kind, &next)?;
        }
        self.geometry = next;
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        crate::perf::inc(crate::perf::Counter::ViewStateGeometryInvalidations);
        self.revision = self.revision.saturating_add(1);
        self.geometry_revision = self.geometry_revision.saturating_add(1);
        Ok(effects)
    }

    pub(crate) fn apply_presentation(
        &mut self,
        patch: &ViewStatePresentationPatch,
    ) -> StateEffects {
        let changed = self.presentation.apply_patch(patch);
        if !changed {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return StateEffects::NONE;
        }
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        self.revision = self.revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        presentation_effects(false)
    }

    pub(crate) fn clear_presentation(
        &mut self,
        properties: Option<&[ViewStatePresentationProperty]>,
    ) -> StateEffects {
        let changed = self.presentation.clear(properties);
        if !changed {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return StateEffects::NONE;
        }
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        self.revision = self.revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        presentation_effects(false)
    }

    pub(crate) fn set_style_state(&mut self, key: String, value: String) -> StateEffects {
        if self.style_states.get(&key) == Some(&value) {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return StateEffects::NONE;
        }
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        self.style_states.insert(key, value);
        self.revision = self.revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        presentation_effects(true)
    }

    pub(crate) fn clear_style_state(&mut self, key: &str) -> StateEffects {
        if self.style_states.remove(key).is_none() {
            crate::perf::inc(crate::perf::Counter::ViewStateMutationsNoop);
            return StateEffects::NONE;
        }
        crate::perf::inc(crate::perf::Counter::ViewStateMutationsAccepted);
        self.revision = self.revision.saturating_add(1);
        self.presentation_revision = self.presentation_revision.saturating_add(1);
        presentation_effects(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::ColorSpec;

    #[test]
    fn repeated_presentation_assignment_is_a_noop() {
        let mut record = ViewStateRecord::new(1);
        let mut patch = ViewStatePresentationPatch::default();
        patch.foreground = Some(Some(ColorSpec::ansi(1)));
        assert!(!record.apply_presentation(&patch).is_empty());
        let revision = record.revision;
        assert!(record.apply_presentation(&patch).is_empty());
        assert_eq!(record.revision, revision);
    }
}
