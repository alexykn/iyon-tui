//! Host integration for the retained-state plane.
//!
//! The state record/effective presentation lives under `retained_state`; this
//! file only adapts mutation wakes and owner teardown to `HostInner`.
//!
//! Wrappers carry the immutable state identity plus a weak host reference.
//! Records are owned directly by the host registry, so every mutation,
//! validation, and disposal serializes through the host lock and no
//! per-record lock remains.

use std::sync::{Mutex, Weak};

use anyhow::Result;

use crate::retained_state::{
    StateEffects, StateNodeKind, ViewStateGeometryPatch, ViewStateGeometryProperty,
    ViewStatePresentationPatch, ViewStatePresentationProperty,
};

use super::environment::WakeDisposition;
use super::host::HostInner;

/// Native host-owned retained geometry/presentation state exposed to Node-API.
#[derive(Clone)]
pub struct HostViewState {
    id: u64,
    host: Weak<Mutex<HostInner>>,
}

impl HostViewState {
    pub(super) fn new(id: u64, host: &std::sync::Arc<Mutex<HostInner>>) -> Self {
        Self {
            id,
            host: std::sync::Arc::downgrade(host),
        }
    }

    /// The immutable identity assigned at creation. Reading it needs no lock:
    /// the id never changes for the life of the wrapper.
    #[must_use]
    pub fn state_id(&self) -> u64 {
        self.id
    }

    pub fn validate_node_kind(&self, node_kind: u32) -> Result<()> {
        let kind = semantic_state_node_kind(node_kind)?;
        let Some(host) = self.host.upgrade() else {
            return Err(anyhow::anyhow!("TUI host is disposed"));
        };
        let host = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        host.validate_view_state_kind(self.id, kind)
    }

    pub fn set_geometry(&self, patch: &ViewStateGeometryPatch) -> Result<WakeDisposition> {
        self.mutate(|record| record.apply_geometry(patch))
    }

    pub fn clear_geometry(
        &self,
        properties: Option<&[ViewStateGeometryProperty]>,
    ) -> Result<WakeDisposition> {
        self.mutate(|record| record.clear_geometry(properties))
    }

    pub fn set_presentation(&self, patch: &ViewStatePresentationPatch) -> Result<WakeDisposition> {
        self.mutate(|record| Ok(record.apply_presentation(patch)))
    }

    pub fn clear_presentation(
        &self,
        properties: Option<&[ViewStatePresentationProperty]>,
    ) -> Result<WakeDisposition> {
        self.mutate(|record| Ok(record.clear_presentation(properties)))
    }

    pub fn set_style_state(&self, key: String, value: String) -> Result<WakeDisposition> {
        if key.is_empty() || value.is_empty() {
            return Err(anyhow::anyhow!(
                "ViewState style state key and value cannot be empty"
            ));
        }
        self.mutate(|record| Ok(record.set_style_state(key, value)))
    }

    pub fn clear_style_state(&self, key: &str) -> Result<WakeDisposition> {
        if key.is_empty() {
            return Err(anyhow::anyhow!("ViewState style state key cannot be empty"));
        }
        self.mutate(|record| Ok(record.clear_style_state(key)))
    }

    pub fn dispose(&self) -> Result<()> {
        let Some(host) = self.host.upgrade() else {
            // The owner registry is gone with the host; there is no retained
            // entry left to invalidate, so disposal is a no-op.
            return Ok(());
        };
        let mut host = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        host.dispose_view_state(self.id)
    }

    fn mutate<F>(&self, mutation: F) -> Result<WakeDisposition>
    where
        F: FnOnce(&mut crate::retained_state::ViewStateRecord) -> Result<StateEffects>,
    {
        let Some(host) = self.host.upgrade() else {
            return Err(anyhow::anyhow!("TUI host is disposed"));
        };
        let mut host = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        if host.is_closed() {
            return Err(anyhow::anyhow!("TUI host is disposed"));
        }
        let effects = host.mutate_view_state(self.id, mutation)?;
        if effects.is_empty() {
            return Ok(WakeDisposition::default());
        }
        host.invalidate_state(self.id, effects)
    }
}

fn semantic_state_node_kind(node_kind: u32) -> Result<StateNodeKind> {
    match node_kind {
        0 => Ok(StateNodeKind::Text),
        1 => Ok(StateNodeKind::Column),
        2 => Ok(StateNodeKind::Spacer),
        3 => Ok(StateNodeKind::Row),
        4 => Ok(StateNodeKind::Column),
        5 => Ok(StateNodeKind::Grid),
        6 => Ok(StateNodeKind::Hanging),
        7 => Ok(StateNodeKind::Container),
        8 | 9 => Ok(StateNodeKind::ClampRows),
        10 => Ok(StateNodeKind::ComponentSlot),
        12 => Ok(StateNodeKind::ContentHost),
        _ => Err(anyhow::anyhow!(
            "unknown semantic ViewState node kind: {node_kind}"
        )),
    }
}
