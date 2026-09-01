//! Host integration for the retained-state plane.
//!
//! The state record/effective presentation lives under `retained_state`; this
//! file only adapts mutation wakes and owner teardown to `HostInner`.

use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;

use crate::retained_state::{
    StateEffects, StateNodeKind, ViewStateGeometryPatch, ViewStateGeometryProperty,
    ViewStateLifecycle, ViewStatePresentationPatch, ViewStatePresentationProperty, ViewStateRecord,
};

use super::environment::WakeDisposition;
use super::host::HostInner;

/// Native host-owned retained geometry/presentation state exposed to Node-API.
#[derive(Clone)]
pub struct HostViewState {
    pub(super) record: Arc<Mutex<ViewStateRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostViewState {
    pub(super) fn new(record: Arc<Mutex<ViewStateRecord>>, host: &Arc<Mutex<HostInner>>) -> Self {
        Self {
            record,
            host: Arc::downgrade(host),
        }
    }

    pub fn state_id(&self) -> u64 {
        self.record.lock().map(|record| record.id).unwrap_or(0)
    }

    pub fn validate_node_kind(&self, node_kind: u32) -> Result<()> {
        let kind = semantic_state_node_kind(node_kind)?;
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?;
        ensure_live(&record.lifecycle)?;
        crate::retained_state::validate_geometry_for_kind(kind, &record.geometry)
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
            self.record
                .lock()
                .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?
                .dispose();
            return Ok(());
        };
        let mut host = host
            .lock()
            .map_err(|_| anyhow::anyhow!("host lock is poisoned"))?;
        host.dispose_view_state(&self.record)
    }

    fn mutate<F>(&self, mutation: F) -> Result<WakeDisposition>
    where
        F: FnOnce(&mut ViewStateRecord) -> Result<StateEffects>,
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
        let effects = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow::anyhow!("ViewState lock is poisoned"))?;
            ensure_live(&record.lifecycle)?;
            mutation(&mut record)?
        };
        if effects.is_empty() {
            return Ok(WakeDisposition::default());
        }
        host.invalidate_state(self.state_id(), effects)
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

fn ensure_live(lifecycle: &ViewStateLifecycle) -> Result<()> {
    if *lifecycle == ViewStateLifecycle::Disposed {
        return Err(anyhow::anyhow!("STATE_DISPOSED: ViewState is disposed"));
    }
    Ok(())
}
