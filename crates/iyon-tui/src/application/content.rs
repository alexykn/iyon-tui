//! Retained content-plane identities and cold control state.
//!
//! This module deliberately stops before Source payload storage and Funnel
//! projection. It owns the PERF-13-D lifecycle graph: environment-owned
//! Sources, host-owned Ports and Connectors, desired/visible mount state, and
//! cold subscription bookkeeping.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use anyhow::{Result, anyhow};

use super::environment::WakeDisposition;
use super::host::HostInner;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContentFamily {
    Text,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextSourceKind {
    Block,
    Stream,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextFunnelKind {
    Plain,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextWrapMode {
    Word,
    Grapheme,
    NoWrap,
}

/// Immutable, Source-neutral Funnel configuration supplied by the control
/// transport. It has no active state, host, viewport, or projection cache.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostContentFunnel {
    pub family: ContentFamily,
    pub kind: TextFunnelKind,
    pub wrap: TextWrapMode,
}

impl HostContentFunnel {
    pub const fn plain(wrap: TextWrapMode) -> Self {
        Self {
            family: ContentFamily::Text,
            kind: TextFunnelKind::Plain,
            wrap,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectorLifecycle {
    Live,
    Disposing,
    Disposed,
}

#[derive(Clone, Debug)]
struct SourceSubscription {
    host: Weak<Mutex<HostInner>>,
    connector_id: u64,
    connector_generation: u32,
}

#[derive(Debug)]
struct ContentSourceRecord {
    id: u64,
    generation: u32,
    family: ContentFamily,
    kind: TextSourceKind,
    lifecycle: SourceLifecycle,
    connector_count: usize,
    subscribers: Vec<SourceSubscription>,
}

#[derive(Debug, Default)]
struct ContentSourceRegistryInner {
    next_id: u64,
    next_generation: u32,
    sources: HashMap<u64, Arc<Mutex<ContentSourceRecord>>>,
}

/// Environment-owned Source registry. The registry holds the authoritative
/// record strongly so a Source can outlive any host and can be reused by later
/// hosts in the same environment.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContentSourceRegistry {
    inner: Arc<Mutex<ContentSourceRegistryInner>>,
}

impl ContentSourceRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn create(&self, kind: TextSourceKind) -> Result<HostContentSource> {
        let mut registry = self
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        let id = registry
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source identity exhausted"))?;
        let generation = registry
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source generation exhausted"))?;
        registry.next_id = id;
        registry.next_generation = generation;
        let record = Arc::new(Mutex::new(ContentSourceRecord {
            id,
            generation,
            family: ContentFamily::Text,
            kind,
            lifecycle: SourceLifecycle::Live,
            connector_count: 0,
            subscribers: Vec::new(),
        }));
        registry.sources.insert(id, Arc::clone(&record));
        Ok(HostContentSource {
            registry: self.clone(),
            record,
        })
    }

    fn contains(&self, id: u64, record: &Arc<Mutex<ContentSourceRecord>>) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|registry| registry.sources.get(&id).cloned())
            .is_some_and(|candidate| Arc::ptr_eq(&candidate, record))
    }
}

/// A native Source identity. Payload mutation is intentionally not present in
/// PERF-13-D; PERF-13-E adds the Source storage/data ABI behind this handle.
#[derive(Clone, Debug)]
pub struct HostContentSource {
    registry: ContentSourceRegistry,
    record: Arc<Mutex<ContentSourceRecord>>,
}

impl HostContentSource {
    pub fn id(&self) -> u64 {
        self.record.lock().map(|record| record.id).unwrap_or(0)
    }

    pub fn generation(&self) -> u32 {
        self.record
            .lock()
            .map(|record| record.generation)
            .unwrap_or(0)
    }

    pub fn family(&self) -> ContentFamily {
        self.record
            .lock()
            .map(|record| record.family)
            .unwrap_or(ContentFamily::Text)
    }

    pub fn kind(&self) -> TextSourceKind {
        self.record
            .lock()
            .map(|record| record.kind)
            .unwrap_or(TextSourceKind::Stream)
    }

    pub(crate) fn same_environment(&self, registry: &ContentSourceRegistry) -> bool {
        Arc::ptr_eq(&self.registry.inner, &registry.inner)
    }

    pub fn is_live(&self) -> bool {
        self.record
            .lock()
            .is_ok_and(|record| record.lifecycle == SourceLifecycle::Live)
            && self.registry.contains(self.id(), &self.record)
    }

    pub fn dispose(&self) -> Result<()> {
        {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            if record.lifecycle == SourceLifecycle::Disposed {
                return Ok(());
            }
            if record.connector_count != 0 {
                return Err(anyhow!(
                    "SOURCE_IN_USE: Source has {} Connector membership(s)",
                    record.connector_count
                ));
            }
            record.lifecycle = SourceLifecycle::Disposed;
            record.subscribers.clear();
        }
        let mut registry = self
            .registry
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        if registry
            .sources
            .get(&self.id())
            .is_some_and(|candidate| Arc::ptr_eq(candidate, &self.record))
        {
            registry.sources.remove(&self.id());
        }
        Ok(())
    }

    fn acquire_connector(&self) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record.connector_count = record
            .connector_count
            .checked_add(1)
            .ok_or_else(|| anyhow!("Source Connector membership count exhausted"))?;
        Ok(())
    }

    fn release_connector(&self) {
        if let Ok(mut record) = self.record.lock() {
            record.connector_count = record.connector_count.saturating_sub(1);
            if record.connector_count == 0 {
                record.subscribers.clear();
            }
        }
    }

    fn subscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record.subscribers.retain(|subscriber| {
            subscriber.host.strong_count() != 0
                && !(subscriber.connector_id == connector_id
                    && subscriber.connector_generation == connector_generation)
        });
        record.subscribers.push(SourceSubscription {
            host: host.clone(),
            connector_id,
            connector_generation,
        });
        Ok(())
    }

    fn unsubscribe(&self, connector_id: u64, connector_generation: u32) {
        if let Ok(mut record) = self.record.lock() {
            record.subscribers.retain(|subscriber| {
                subscriber.connector_id != connector_id
                    || subscriber.connector_generation != connector_generation
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.record
            .lock()
            .map(|record| record.subscribers.len())
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentBinding {
    pub port_id: u64,
    pub connector_id: Option<u64>,
}

#[derive(Clone, Debug)]
struct PortRecord {
    id: u64,
    generation: u32,
    family: ContentFamily,
    lifecycle: PortLifecycle,
    host: Weak<Mutex<HostInner>>,
    connector_ids: HashSet<u64>,
    desired_mounted: bool,
    visible_mounted: bool,
    desired_connector: Option<u64>,
    visible_connector: Option<u64>,
}

#[derive(Debug)]
struct ConnectorRecord {
    id: u64,
    generation: u32,
    lifecycle: ConnectorLifecycle,
    port: Weak<Mutex<PortRecord>>,
    source: HostContentSource,
    funnel: HostContentFunnel,
    requested: bool,
    visible: bool,
    subscribed: bool,
    phase: &'static str,
    error: Option<ContentConnectorError>,
    /// Deterministic synthetic operational failure used by native/unit
    /// fixtures to exercise transactional switch fallback before projection
    /// is implemented by the later content tranche.
    activation_failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorError {
    pub code: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorStatus {
    pub phase: String,
    pub requested: bool,
    pub visible: bool,
    pub projected_source_revision: Option<u64>,
    pub error: Option<ContentConnectorError>,
}

/// Host-owned Port/Connector registries. The registry is intentionally
/// separate from Source storage: Sources are environment-owned, while ports
/// and connectors depend on one host's activation and visible frame.
#[derive(Debug)]
pub(crate) struct ContentHostRegistry {
    source_registry: ContentSourceRegistry,
    next_port_id: u64,
    next_connector_id: u64,
    next_generation: u32,
    ports: HashMap<u64, Arc<Mutex<PortRecord>>>,
    connectors: HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
}

impl ContentHostRegistry {
    pub(crate) fn new(source_registry: ContentSourceRegistry) -> Self {
        Self {
            source_registry,
            next_port_id: 0,
            next_connector_id: 0,
            next_generation: 0,
            ports: HashMap::new(),
            connectors: HashMap::new(),
        }
    }

    pub(crate) fn create_port(
        &mut self,
        host: Weak<Mutex<HostInner>>,
        family: ContentFamily,
    ) -> Result<HostContentPort> {
        self.next_port_id = self
            .next_port_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("ContentPort identity exhausted"))?;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("ContentPort generation exhausted"))?;
        let record = Arc::new(Mutex::new(PortRecord {
            id: self.next_port_id,
            generation: self.next_generation,
            family,
            lifecycle: PortLifecycle::Live,
            host: host.clone(),
            connector_ids: HashSet::new(),
            desired_mounted: false,
            visible_mounted: false,
            desired_connector: None,
            visible_connector: None,
        }));
        self.ports.insert(self.next_port_id, Arc::clone(&record));
        Ok(HostContentPort { record, host })
    }

    fn connect(
        &mut self,
        port: &Arc<Mutex<PortRecord>>,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        let (port_id, port_family, port_live) = {
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (port.id, port.family, port.lifecycle == PortLifecycle::Live)
        };
        if !port_live {
            return Err(anyhow!("PORT_DISPOSED: ContentPort is disposed"));
        }
        if self
            .ports
            .get(&port_id)
            .is_none_or(|candidate| !Arc::ptr_eq(candidate, port))
        {
            return Err(anyhow!(
                "STALE_HANDLE: ContentPort is not owned by this host"
            ));
        }
        if !source.same_environment(&self.source_registry) {
            return Err(anyhow!(
                "WRONG_ENVIRONMENT: Source belongs to a different environment"
            ));
        }
        if !source.is_live() {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if funnel.family != port_family || funnel.family != source.family() {
            return Err(anyhow!(
                "CONTENT_FAMILY_MISMATCH: ContentPort and Source/Funnel families differ"
            ));
        }
        source.acquire_connector()?;
        self.next_connector_id = match self.next_connector_id.checked_add(1) {
            Some(id) => id,
            None => {
                source.release_connector();
                return Err(anyhow!("Connector identity exhausted"));
            }
        };
        self.next_generation = match self.next_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                source.release_connector();
                return Err(anyhow!("Connector generation exhausted"));
            }
        };
        let record = Arc::new(Mutex::new(ConnectorRecord {
            id: self.next_connector_id,
            generation: self.next_generation,
            lifecycle: ConnectorLifecycle::Live,
            port: Arc::downgrade(port),
            source: source.clone(),
            funnel,
            requested: false,
            visible: false,
            subscribed: false,
            phase: "idle",
            error: None,
            activation_failure: None,
        }));
        self.connectors
            .insert(self.next_connector_id, Arc::clone(&record));
        port.lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .connector_ids
            .insert(self.next_connector_id);
        Ok(HostContentConnector {
            record,
            host: Weak::new(),
        })
    }

    pub(crate) fn validate_targets(&self, targets: &[u64]) -> Result<()> {
        let mut seen = HashSet::with_capacity(targets.len());
        for id in targets {
            if !seen.insert(*id) {
                return Err(anyhow!(
                    "DUPLICATE_CONTENT_PORT_ATTACHMENT: ContentPort {id} occurs more than once"
                ));
            }
            let Some(port) = self.ports.get(id) else {
                return Err(anyhow!(
                    "STALE_HANDLE: ContentPort {id} is not owned by this host"
                ));
            };
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            if port.lifecycle != PortLifecycle::Live {
                return Err(anyhow!("PORT_DISPOSED: ContentPort {id} is disposed"));
            }
        }
        Ok(())
    }

    pub(crate) fn set_desired(&mut self, targets: &[u64]) -> Result<()> {
        self.validate_targets(targets)?;
        let target_set = targets.iter().copied().collect::<HashSet<_>>();
        let port_ids = self.ports.keys().copied().collect::<Vec<_>>();
        for port_id in port_ids {
            let Some(port) = self.ports.get(&port_id).cloned() else {
                continue;
            };
            let desired_mounted = target_set.contains(&port_id);
            {
                let mut port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                port_state.desired_mounted = desired_mounted;
                let desired_connector = port_state.desired_connector;
                let host = port_state.host.clone();
                drop(port_state);
                if let Some(connector_id) = desired_connector {
                    self.refresh_requested_phase(connector_id, desired_mounted)?;
                    if desired_mounted {
                        self.ensure_requested_subscription(connector_id, &host)?;
                    } else {
                        self.unsubscribe_requested_if_not_visible(connector_id)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn candidate_bindings(&self, targets: &[u64]) -> Result<Vec<ContentBinding>> {
        self.validate_targets(targets)?;
        targets
            .iter()
            .map(|port_id| {
                let port = self
                    .ports
                    .get(port_id)
                    .ok_or_else(|| anyhow!("ContentPort disappeared during frame preparation"))?;
                let (desired_connector, visible_connector) = {
                    let port = port
                        .lock()
                        .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                    (port.desired_connector, port.visible_connector)
                };
                let connector_id = match desired_connector {
                    None => None,
                    Some(id) if self.connector_is_candidate_ready(id) => Some(id),
                    Some(_) => visible_connector,
                };
                Ok(ContentBinding {
                    port_id: *port_id,
                    connector_id,
                })
            })
            .collect()
    }

    pub(crate) fn commit_visible(&mut self, bindings: &[ContentBinding]) {
        let mounted = bindings
            .iter()
            .map(|binding| binding.port_id)
            .collect::<HashSet<_>>();
        let selected = bindings
            .iter()
            .map(|binding| (binding.port_id, binding.connector_id))
            .collect::<HashMap<_, _>>();
        let port_ids = self.ports.keys().copied().collect::<Vec<_>>();
        for port_id in port_ids {
            let Some(port) = self.ports.get(&port_id).cloned() else {
                continue;
            };
            let old_visible = port.lock().ok().and_then(|port| port.visible_connector);
            let next_visible = selected.get(&port_id).copied().flatten();
            if let Some(old_id) = old_visible
                && Some(old_id) != next_visible
            {
                self.set_connector_visible(old_id, false);
            }
            if let Ok(mut port_state) = port.lock() {
                port_state.visible_mounted = mounted.contains(&port_id);
                port_state.visible_connector = if port_state.visible_mounted {
                    next_visible
                } else {
                    None
                };
            }
            if let Some(next_id) = next_visible
                && mounted.contains(&port_id)
            {
                self.set_connector_visible(next_id, true);
            }
        }
        let connector_ids = self.connectors.keys().copied().collect::<Vec<_>>();
        for connector_id in connector_ids {
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                continue;
            };
            let Ok(mut state) = connector.lock() else {
                continue;
            };
            if state.lifecycle == ConnectorLifecycle::Disposed {
                continue;
            }
            let port_mounted = state
                .port
                .upgrade()
                .and_then(|port| port.lock().ok().map(|port| port.visible_mounted))
                .unwrap_or(false);
            if state.visible {
                state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                    "disposing"
                } else {
                    "active"
                };
            } else if state.lifecycle == ConnectorLifecycle::Disposing {
                state.phase = "disposing";
            } else if state.requested {
                state.phase = if state.error.is_some() && port_mounted {
                    "failed"
                } else if port_mounted {
                    "activation-pending"
                } else {
                    "waiting-for-mount"
                };
            } else {
                state.phase = "idle";
            }
        }
        self.finalize_disposed_connectors();
    }

    pub(crate) fn fail_next_activation(
        &mut self,
        connector_id: u64,
        diagnostic: String,
    ) -> Result<()> {
        if diagnostic.is_empty() {
            return Err(anyhow!("activation failure diagnostic cannot be empty"));
        }
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live {
            return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not live"));
        }
        state.activation_failure = Some(diagnostic);
        Ok(())
    }

    fn request_activation(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (
            port,
            generation,
            source,
            was_requested,
            was_selected,
            port_mounted,
            was_failed,
            activation_failure,
        ) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not activatable"));
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (was_selected, port_mounted) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    port_state.desired_connector == Some(connector_id),
                    port_state.desired_mounted,
                )
            };
            (
                port,
                state.generation,
                state.source.clone(),
                state.requested,
                was_selected,
                port_mounted,
                state.error.is_some(),
                if port_mounted {
                    state.activation_failure.take()
                } else {
                    None
                },
            )
        };
        if was_requested && was_selected && !was_failed && activation_failure.is_none() {
            return Ok(false);
        }
        let old_selected = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        if let Some(old_id) = old_selected
            && old_id != connector_id
        {
            self.clear_requested(old_id)?;
        }
        {
            let mut port_state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            port_state.desired_connector = Some(connector_id);
        }
        let activation_failed = activation_failure.is_some();
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.requested = true;
            state.error = activation_failure
                .as_ref()
                .map(|diagnostic| ContentConnectorError {
                    code: "PROJECTION_FAILED".to_owned(),
                    diagnostic: diagnostic.clone(),
                });
            state.phase = if state.error.is_some() {
                "failed"
            } else if port_mounted {
                "activation-pending"
            } else {
                "waiting-for-mount"
            };
        }
        if port_mounted {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        Ok(port_mounted && !activation_failed)
    }

    fn request_deactivation(&mut self, connector_id: u64) -> Result<bool> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (port, source, generation, was_visible, was_requested) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not active"));
            }
            (
                state
                    .port
                    .upgrade()
                    .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?,
                state.source.clone(),
                state.generation,
                state.visible,
                state.requested,
            )
        };
        if !was_requested && !was_visible {
            return Ok(false);
        }
        if port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector
            == Some(connector_id)
        {
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .desired_connector = None;
        }
        if !was_visible {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        if let Ok(mut state) = connector.lock() {
            state.requested = false;
            state.phase = if state.visible { "active" } else { "idle" };
        }
        Ok(was_visible)
    }

    fn request_connector_disposal(&mut self, connector_id: u64) -> Result<bool> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (port, source, generation, visible, desired) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle == ConnectorLifecycle::Disposed
                || state.lifecycle == ConnectorLifecycle::Disposing
            {
                return Ok(false);
            }
            state.lifecycle = ConnectorLifecycle::Disposing;
            state.requested = false;
            state.phase = "disposing";
            (
                state
                    .port
                    .upgrade()
                    .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?,
                state.source.clone(),
                state.generation,
                state.visible,
                state.port.upgrade().and_then(|port| {
                    port.lock().ok().and_then(|port| {
                        (port.desired_connector == Some(connector_id)).then_some(())
                    })
                }),
            )
        };
        if !visible {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        if desired.is_some()
            && let Ok(mut port_state) = port.lock()
        {
            port_state.desired_connector = None;
        }
        if visible {
            return Ok(true);
        }
        self.remove_connector(connector_id);
        Ok(false)
    }

    fn dispose_port(&mut self, port: &Arc<Mutex<PortRecord>>) -> Result<()> {
        let (id, desired_mounted, visible_mounted, connector_ids) = {
            let state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (
                state.id,
                state.desired_mounted,
                state.visible_mounted,
                state.connector_ids.clone(),
            )
        };
        if desired_mounted || visible_mounted {
            return Err(anyhow!(
                "PORT_MOUNTED: ContentPort {id} is still structurally mounted"
            ));
        }
        if !connector_ids.is_empty() {
            return Err(anyhow!(
                "PORT_IN_USE: ContentPort {id} still has Connector membership"
            ));
        }
        if let Ok(mut state) = port.lock() {
            state.lifecycle = PortLifecycle::Disposed;
        }
        self.ports.remove(&id);
        Ok(())
    }

    pub(crate) fn dispose_all(&mut self) {
        let connector_ids = self.connectors.keys().copied().collect::<Vec<_>>();
        for connector_id in connector_ids {
            self.remove_connector(connector_id);
        }
        for port in self.ports.values() {
            if let Ok(mut state) = port.lock() {
                state.lifecycle = PortLifecycle::Disposed;
                state.desired_mounted = false;
                state.visible_mounted = false;
                state.desired_connector = None;
                state.visible_connector = None;
            }
        }
        self.ports.clear();
    }

    fn refresh_requested_phase(&mut self, connector_id: u64, mounted: bool) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Ok(());
        }
        state.phase = if state.error.is_some() && mounted {
            "failed"
        } else if mounted {
            "activation-pending"
        } else {
            "waiting-for-mount"
        };
        Ok(())
    }

    fn ensure_requested_subscription(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, requested, visible, lifecycle) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (
                state.source.clone(),
                state.generation,
                state.requested,
                state.visible,
                state.lifecycle,
            )
        };
        if requested && !visible && lifecycle == ConnectorLifecycle::Live {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        Ok(())
    }

    fn unsubscribe_requested_if_not_visible(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        Ok(())
    }

    fn clear_requested(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.requested = false;
            if !state.visible {
                state.phase = "idle";
            }
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        Ok(())
    }

    fn subscribe_connector(
        &mut self,
        connector_id: u64,
        source: &HostContentSource,
        generation: u32,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!("STALE_HANDLE: Connector is unavailable"));
        };
        let already_subscribed = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed;
        if already_subscribed {
            return Ok(());
        }
        source.subscribe(host, connector_id, generation)?;
        connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed = true;
        Ok(())
    }

    fn unsubscribe_connector(&mut self, source: &HostContentSource, id: u64, generation: u32) {
        source.unsubscribe(id, generation);
        if let Some(connector) = self.connectors.get(&id)
            && let Ok(mut state) = connector.lock()
        {
            state.subscribed = false;
        }
    }

    fn set_connector_visible(&mut self, connector_id: u64, visible: bool) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let Ok(mut state) = connector.lock() else {
            return;
        };
        state.visible = visible;
        let source = state.source.clone();
        let generation = state.generation;
        if visible {
            state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                "disposing"
            } else {
                "active"
            };
        } else if state.lifecycle != ConnectorLifecycle::Disposing {
            state.phase = if state.error.is_some() && state.requested {
                "failed"
            } else if state.requested {
                "activation-pending"
            } else {
                "idle"
            };
        }
        drop(state);
        if !visible {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
    }

    fn remove_connector(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.remove(&connector_id) else {
            return;
        };
        if let Ok(mut state) = connector.lock() {
            state.source.unsubscribe(connector_id, state.generation);
            state.source.release_connector();
            state.lifecycle = ConnectorLifecycle::Disposed;
            state.phase = "disposed";
            state.visible = false;
            state.requested = false;
            if let Some(port) = state.port.upgrade()
                && let Ok(mut port_state) = port.lock()
            {
                port_state.connector_ids.remove(&connector_id);
                if port_state.desired_connector == Some(connector_id) {
                    port_state.desired_connector = None;
                }
                if port_state.visible_connector == Some(connector_id) {
                    port_state.visible_connector = None;
                }
            }
        }
    }

    fn finalize_disposed_connectors(&mut self) {
        let ids = self
            .connectors
            .iter()
            .filter_map(|(id, connector)| {
                let state = connector.lock().ok()?;
                (state.lifecycle == ConnectorLifecycle::Disposing && !state.visible).then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            self.remove_connector(id);
        }
    }

    pub(crate) fn connector_status(&self, id: u64) -> Result<ContentConnectorStatus> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(ContentConnectorStatus {
            phase: state.phase.to_owned(),
            requested: state.requested,
            visible: state.visible,
            projected_source_revision: None,
            error: state.error.clone(),
        })
    }

    pub(crate) fn port_status(&self, id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&id)
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {id} is unavailable"))?;
        Ok(port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .visible_mounted)
    }

    fn connector_is_candidate_ready(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_some_and(|state| {
                state.lifecycle == ConnectorLifecycle::Live
                    && state.requested
                    && state.error.is_none()
            })
    }

    pub(crate) fn connector_is_disposed(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_none_or(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }

    fn deactivate_port(&mut self, port_id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&port_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {port_id} is unavailable"))?;
        let connector_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        match connector_id {
            Some(connector_id) => self.request_deactivation(connector_id),
            None => Ok(false),
        }
    }

    fn request_connector_activation(
        &mut self,
        id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        self.request_activation(id, host)
    }

    fn request_connector_deactivation(&mut self, id: u64) -> Result<bool> {
        self.request_deactivation(id)
    }

    fn request_connector_dispose(&mut self, id: u64) -> Result<bool> {
        self.request_connector_disposal(id)
    }
}

#[derive(Clone, Debug)]
pub struct HostContentPort {
    record: Arc<Mutex<PortRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentPort {
    pub fn id(&self) -> u64 {
        self.record.lock().map(|record| record.id).unwrap_or(0)
    }

    pub fn generation(&self) -> u32 {
        self.record
            .lock()
            .map(|record| record.generation)
            .unwrap_or(0)
    }

    pub fn family(&self) -> ContentFamily {
        self.record
            .lock()
            .map(|record| record.family)
            .unwrap_or(ContentFamily::Text)
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.deactivate_port(self.id())?;
        if needs_frame {
            return inner.mark_pending();
        }
        Ok(WakeDisposition::default())
    }

    pub fn connect(
        &self,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        inner
            .content
            .connect(&self.record, source, funnel)
            .map(|mut connector| {
                connector.host = host_weak;
                connector
            })
    }

    pub fn dispose(&self) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .dispose_port(&self.record)
    }

    pub fn is_mounted(&self) -> Result<bool> {
        self.record
            .lock()
            .map(|record| record.visible_mounted)
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))
    }
}

#[derive(Clone, Debug)]
pub struct HostContentConnector {
    record: Arc<Mutex<ConnectorRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentConnector {
    pub fn id(&self) -> u64 {
        self.record.lock().map(|record| record.id).unwrap_or(0)
    }

    pub fn generation(&self) -> u32 {
        self.record
            .lock()
            .map(|record| record.generation)
            .unwrap_or(0)
    }

    pub fn source_id(&self) -> u64 {
        self.record
            .lock()
            .map(|record| record.source.id())
            .unwrap_or(0)
    }

    pub fn activate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner
            .content
            .request_connector_activation(self.id(), &host_weak)?;
        if needs_frame {
            return inner.mark_pending();
        }
        Ok(WakeDisposition::default())
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.request_connector_deactivation(self.id())?;
        if needs_frame {
            return inner.mark_pending();
        }
        Ok(WakeDisposition::default())
    }

    pub fn dispose(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let needs_frame = inner.content.request_connector_dispose(self.id())?;
        if needs_frame {
            return inner.mark_pending();
        }
        Ok(WakeDisposition::default())
    }

    /// Injects one deterministic operational failure for a native/unit
    /// fixture. It is not part of the TypeScript content API; projection
    /// failures in later tranches use the same candidate-fallback state.
    pub fn fail_next_activation(&self, diagnostic: String) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .fail_next_activation(self.id(), diagnostic)
    }

    pub fn status(&self) -> Result<ContentConnectorStatus> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .connector_status(self.id())
            .or_else(|_| {
                let state = self
                    .record
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                Ok(ContentConnectorStatus {
                    phase: if state.lifecycle == ConnectorLifecycle::Disposed {
                        "disposed".to_owned()
                    } else {
                        state.phase.to_owned()
                    },
                    requested: state.requested,
                    visible: state.visible,
                    projected_source_revision: None,
                    error: state.error.clone(),
                })
            })
    }

    pub fn is_disposed(&self) -> bool {
        self.record
            .lock()
            .is_ok_and(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }
}

impl Drop for HostContentConnector {
    fn drop(&mut self) {
        // Explicit disposal owns semantic release. A dropped wrapper is not a
        // hidden lifecycle operation; host teardown calls dispose_all instead.
    }
}
