//! Retained content-plane identities and cold control state.
//!
//! This module deliberately stops before Source payload storage and Funnel
//! projection. It owns the PERF-13-D lifecycle graph: environment-owned
//! Sources, host-owned Ports and Connectors, desired/visible mount state, and
//! cold subscription bookkeeping.

use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};

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

// ContentPort IDs cross the structural/native boundary, so they must not be
// host-local: a View built for host A must not accidentally resolve to host B's
// port with the same local slot. IDs are monotonic and never reused.
static NEXT_CONTENT_PORT_ID: AtomicU64 = AtomicU64::new(1);

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
struct SourceRetentionPolicy {
    max_bytes: Option<u64>,
    max_lines: Option<u64>,
    drop_oldest: bool,
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
    retention: Option<SourceRetentionPolicy>,
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
            retention: None,
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

    /// Stores creation-time retention policy for the later Source storage
    /// tranche. PERF-13-D does not mutate payloads yet, but accepting and then
    /// dropping this configuration would make the public factory dishonest.
    #[doc(hidden)]
    pub fn configure_retention(
        &self,
        max_bytes: Option<u64>,
        max_lines: Option<u64>,
        drop_oldest: bool,
    ) -> Result<()> {
        if max_bytes.is_none() && max_lines.is_none()
            || max_bytes.is_some_and(|value| value == 0)
            || max_lines.is_some_and(|value| value == 0)
        {
            return Err(anyhow!(
                "INVALID_ARGUMENT: Source retention limits must be positive"
            ));
        }
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if record.connector_count != 0 {
            return Err(anyhow!(
                "SOURCE_IN_USE: Source retention cannot change while Connectors exist"
            ));
        }
        record.retention = Some(SourceRetentionPolicy {
            max_bytes,
            max_lines,
            drop_oldest,
        });
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
                && !(Weak::ptr_eq(&subscriber.host, host)
                    && subscriber.connector_id == connector_id
                    && subscriber.connector_generation == connector_generation)
        });
        record.subscribers.push(SourceSubscription {
            host: host.clone(),
            connector_id,
            connector_generation,
        });
        Ok(())
    }

    fn unsubscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) {
        if let Ok(mut record) = self.record.lock() {
            record.subscribers.retain(|subscriber| {
                !Weak::ptr_eq(&subscriber.host, host)
                    || subscriber.connector_id != connector_id
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
    owner_host: Weak<Mutex<HostInner>>,
    next_connector_id: u64,
    next_generation: u32,
    ports: HashMap<u64, Arc<Mutex<PortRecord>>>,
    connectors: HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
    in_flight_connectors: HashSet<u64>,
}

impl ContentHostRegistry {
    pub(crate) fn new(source_registry: ContentSourceRegistry) -> Self {
        Self {
            source_registry,
            owner_host: Weak::new(),
            next_connector_id: 0,
            next_generation: 0,
            ports: HashMap::new(),
            connectors: HashMap::new(),
            in_flight_connectors: HashSet::new(),
        }
    }

    pub(crate) fn create_port(
        &mut self,
        host: Weak<Mutex<HostInner>>,
        family: ContentFamily,
    ) -> Result<HostContentPort> {
        self.owner_host = host.clone();
        let port_id = NEXT_CONTENT_PORT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| anyhow!("ContentPort identity exhausted"))?;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("ContentPort generation exhausted"))?;
        let record = Arc::new(Mutex::new(PortRecord {
            id: port_id,
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
        self.ports.insert(port_id, Arc::clone(&record));
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
                let was_mounted = port_state.desired_mounted;
                port_state.desired_mounted = desired_mounted;
                let desired_connector = port_state.desired_connector;
                let host = port_state.host.clone();
                drop(port_state);
                if let Some(connector_id) = desired_connector {
                    self.refresh_requested_phase(
                        connector_id,
                        desired_mounted,
                        !was_mounted && desired_mounted,
                    )?;
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

    pub(crate) fn candidate_bindings(&mut self, targets: &[u64]) -> Result<Vec<ContentBinding>> {
        // Ordinary target validation belongs to H3 prepare. Repeating it here
        // would let the fallible frame path report stale/duplicate/wrong-host
        // attachment errors after the desired publication has already been
        // accepted. The owner registry and H3 lease keep this lookup valid;
        // disappearance is an internal invariant failure instead.
        targets
            .iter()
            .map(|port_id| {
                let port = self.ports.get(port_id).cloned().ok_or_else(|| {
                    anyhow!(
                        "INTERNAL_INVARIANT: ContentPort {port_id} disappeared after H3 prepare"
                    )
                })?;
                let (desired_connector, visible_connector, port_mounted) = {
                    let port = port.lock().map_err(|_| {
                        anyhow!("INTERNAL_INVARIANT: ContentPort {port_id} lock is poisoned")
                    })?;
                    (
                        port.desired_connector,
                        port.visible_connector,
                        port.desired_mounted,
                    )
                };
                let connector_id = match desired_connector {
                    None => None,
                    Some(id) if port_mounted && self.prepare_activation_candidate(id)? => {
                        visible_connector
                    }
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

    /// Applies the native/unit-only operational failure hook at candidate
    /// preparation time. This keeps activation request state truthful and
    /// exercises the same old-visible fallback boundary that real projection
    /// errors will use in PERF-13-F.
    fn prepare_activation_candidate(&mut self, connector_id: u64) -> Result<bool> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation candidate {connector_id} disappeared"
            ));
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        let Some(diagnostic) = state.activation_failure.take() else {
            return Ok(false);
        };
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation failure targeted a non-candidate Connector {connector_id}"
            ));
        }
        state.error = Some(ContentConnectorError {
            code: "PROJECTION_FAILED".to_owned(),
            diagnostic,
        });
        state.phase = "failed";
        Ok(true)
    }

    /// Retains the Connector IDs referenced by a candidate frame. Control
    /// mutations accepted while a backend receipt is in flight must not
    /// destroy or detach an identity that the captured candidate still uses.
    pub(crate) fn begin_candidate(&mut self, bindings: &[ContentBinding]) {
        self.in_flight_connectors = bindings
            .iter()
            .filter_map(|binding| binding.connector_id)
            .collect();
    }

    /// Releases the candidate lease after its logical frame commit. A
    /// disposing Connector selected by that frame remains visible/disposing
    /// until the following removal frame, as required by transactional
    /// disposal semantics.
    pub(crate) fn end_candidate(&mut self) {
        self.in_flight_connectors.clear();
    }

    /// Aborts a candidate without changing visible bindings. Deferred control
    /// mutations can now finalize identities that were never made visible and
    /// cold requested Connectors can lose provisional subscriptions.
    pub(crate) fn abort_candidate(&mut self) {
        let connector_ids = self.in_flight_connectors.drain().collect::<Vec<_>>();
        for connector_id in connector_ids {
            self.cleanup_aborted_candidate(connector_id);
        }
        self.finalize_disposed_connectors();
    }

    fn cleanup_aborted_candidate(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let Ok(state) = connector.lock() else {
            return;
        };
        let source = state.source.clone();
        let generation = state.generation;
        let visible = state.visible;
        let requested = state.requested;
        let port_mounted = state
            .port
            .upgrade()
            .and_then(|port| port.lock().ok().map(|port| port.desired_mounted))
            .unwrap_or(false);
        drop(state);
        if !visible && (!requested || !port_mounted) {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
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
        if state.visible {
            return Err(anyhow!(
                "INVALID_ARGUMENT: cannot inject activation failure for a visible Connector"
            ));
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
        let (port, generation, source, was_requested, was_selected, port_mounted, was_failed) = {
            let state = connector
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
            )
        };
        if was_requested && was_selected && !was_failed {
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
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.requested = true;
            state.error = None;
            state.phase = if port_mounted {
                "activation-pending"
            } else {
                "waiting-for-mount"
            };
        }
        if port_mounted {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        // The request itself is not the activation/projection operation. A
        // mounted candidate is processed by candidate_bindings inside the
        // frame transaction, where injected/real operational failure can fall
        // back to the committed Connector without changing the visible frame.
        Ok(port_mounted)
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
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        if !was_requested && !was_visible && !in_flight {
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
        // A captured candidate may still select this Connector even though it
        // is not visible in the old frame. Keep its subscription and identity
        // until that receipt commits or aborts; otherwise the old candidate
        // can resurrect a deactivated Connector without a follow-up epoch.
        if !was_visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        if let Ok(mut state) = connector.lock() {
            state.requested = false;
            state.phase = if state.visible { "active" } else { "idle" };
        }
        Ok(was_visible || in_flight)
    }

    fn request_connector_disposal(&mut self, connector_id: u64) -> Result<bool> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let in_flight = self.in_flight_connectors.contains(&connector_id);
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
        if !visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation);
        }
        if desired.is_some()
            && let Ok(mut port_state) = port.lock()
        {
            port_state.desired_connector = None;
        }
        if visible || in_flight {
            // The captured candidate still owns a short-lived identity lease;
            // finalize only after commit/abort reconciles that candidate.
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
        self.in_flight_connectors.clear();
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

    fn refresh_requested_phase(
        &mut self,
        connector_id: u64,
        mounted: bool,
        remounted: bool,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Ok(());
        }
        if remounted && mounted {
            // A failed candidate is retryable on a real remount. Clear only
            // the old operational diagnostic here; an error from the new
            // candidate will be recorded again during frame preparation.
            state.error = None;
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
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
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
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
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
        source.unsubscribe(&self.owner_host, id, generation);
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
            state
                .source
                .unsubscribe(&self.owner_host, connector_id, state.generation);
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
        let disposed = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .lifecycle
            == ConnectorLifecycle::Disposed;
        if disposed {
            return self.record_status();
        }
        if let Some(host) = self.host.upgrade() {
            return host
                .lock()
                .map_err(|_| anyhow!("host lock is poisoned"))?
                .content
                .connector_status(self.id());
        }
        // HostInner::drop() marks retained Connector records disposed before
        // its weak owner disappears. A live record with no owner is an
        // invariant failure, not a reason to fabricate a status.
        Err(anyhow!("HOST_DISPOSED: Connector host is gone"))
    }

    fn record_status(&self) -> Result<ContentConnectorStatus> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::View;
    use crate::application::environment::TuiEnvironment;
    use crate::application::host::TuiHost;

    #[test]
    fn source_subscriptions_are_scoped_by_host() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let funnel = HostContentFunnel::plain(TextWrapMode::Word);
        let first_port = first.create_content_port(ContentFamily::Text).unwrap();
        let first_connector = first_port.connect(&source, funnel).unwrap();
        first_connector.activate().unwrap();
        assert_eq!(source.subscriber_count(), 0);
        first
            .set_desired_view(View::native_content_host(first_port.id()).unwrap())
            .unwrap();
        first.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 1);

        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        let second_connector = second_port.connect(&source, funnel).unwrap();
        second_connector.activate().unwrap();
        assert_eq!(source.subscriber_count(), 1);
        second
            .set_desired_view(View::native_content_host(second_port.id()).unwrap())
            .unwrap();
        second.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 2);

        first_connector.deactivate().unwrap();
        first.flush_pending_hosts(32, true).unwrap();
        assert_eq!(source.subscriber_count(), 1);

        first.close().unwrap();
        assert_eq!(source.subscriber_count(), 1);
        second.close().unwrap();
        assert_eq!(source.subscriber_count(), 0);
        source.dispose().unwrap();
    }

    #[test]
    fn cold_membership_blocks_source_disposal_until_connector_release() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        assert_eq!(source.subscriber_count(), 0);
        assert!(source.dispose().is_err());
        registry.remove_connector(connector.id());
        source.dispose().unwrap();
    }

    #[test]
    fn content_port_ids_cannot_alias_across_hosts() {
        let environment = TuiEnvironment::new();
        let first = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let second = TuiHost::open_in_environment(20, 4, true, environment).unwrap();
        let first_port = first.create_content_port(ContentFamily::Text).unwrap();
        let second_port = second.create_content_port(ContentFamily::Text).unwrap();
        assert_ne!(first_port.id(), second_port.id());

        let foreign_view = View::native_content_host(first_port.id()).unwrap();
        let error = second.set_desired_view(foreign_view).unwrap_err();
        assert!(error.to_string().contains("STALE_HANDLE"));

        first.close().unwrap();
        second.close().unwrap();
    }

    #[test]
    fn activation_failure_is_recorded_by_candidate_fallback() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let port = host.create_content_port(ContentFamily::Text).unwrap();
        let first = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        let second = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
            .unwrap();
        first.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();

        second
            .fail_next_activation("synthetic projection failure".to_owned())
            .unwrap();
        second.activate().unwrap();
        assert_eq!(second.status().unwrap().phase, "activation-pending");
        host.flush_pending_hosts(32, true).unwrap();
        let status = second.status().unwrap();
        assert_eq!(status.phase, "failed");
        assert!(status.requested);
        assert!(!status.visible);
        assert_eq!(
            status.error.as_ref().map(|error| error.code.as_str()),
            Some("PROJECTION_FAILED")
        );
        assert_eq!(first.status().unwrap().phase, "active");

        second.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "active");
        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn source_connection_rejects_a_different_environment() {
        let first_sources = ContentSourceRegistry::new();
        let second_sources = ContentSourceRegistry::new();
        let source = first_sources.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(second_sources);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let error = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap_err();
        assert!(error.to_string().contains("WRONG_ENVIRONMENT"));
        source.dispose().unwrap();
    }

    #[test]
    fn failed_connector_retries_on_a_port_remount() {
        let environment = TuiEnvironment::new();
        let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let port = host.create_content_port(ContentFamily::Text).unwrap();
        let first = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        let second = port
            .connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
            .unwrap();
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
            .unwrap();
        first.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        second
            .fail_next_activation("synthetic projection failure".to_owned())
            .unwrap();
        second.activate().unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "failed");

        host.set_desired_view(View::spacer(0)).unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "waiting-for-mount");
        host.set_desired_view(View::native_content_host(port.id()).unwrap())
            .unwrap();
        host.flush_pending_hosts(32, true).unwrap();
        assert_eq!(second.status().unwrap().phase, "active");

        host.close().unwrap();
        source.dispose().unwrap();
    }

    #[test]
    fn connector_status_survives_native_host_drop_as_disposed() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .unwrap();
        let connector = {
            let host = TuiHost::open_in_environment(20, 4, true, environment.clone()).unwrap();
            let port = host.create_content_port(ContentFamily::Text).unwrap();
            port.connect(&source, HostContentFunnel::plain(TextWrapMode::Word))
                .unwrap()
        };
        assert_eq!(connector.status().unwrap().phase, "disposed");
        source.dispose().unwrap();
    }

    #[test]
    fn candidate_control_keeps_in_flight_connector_identity_alive() {
        let source_registry = ContentSourceRegistry::new();
        let source = source_registry.create(TextSourceKind::Stream).unwrap();
        let mut registry = ContentHostRegistry::new(source_registry);
        let port = registry
            .create_port(Weak::new(), ContentFamily::Text)
            .unwrap();
        let connector = registry
            .connect(
                &port.record,
                &source,
                HostContentFunnel::plain(TextWrapMode::Word),
            )
            .unwrap();
        let connector_id = connector.id();
        {
            let mut state = port.record.lock().unwrap();
            state.desired_mounted = true;
            state.desired_connector = Some(connector_id);
        }
        let binding = ContentBinding {
            port_id: port.id(),
            connector_id: Some(connector_id),
        };
        registry.begin_candidate(&[binding]);

        assert!(registry.request_deactivation(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.abort_candidate();
        assert!(registry.connectors.contains_key(&connector_id));
        assert_eq!(source.subscriber_count(), 0);

        {
            let mut state = port.record.lock().unwrap();
            state.desired_connector = Some(connector_id);
        }
        registry.begin_candidate(&[binding]);
        assert!(registry.request_connector_disposal(connector_id).unwrap());
        assert!(registry.connectors.contains_key(&connector_id));
        registry.commit_visible(&[binding]);
        registry.end_candidate();
        assert!(
            registry
                .connectors
                .get(&connector_id)
                .and_then(|record| record.lock().ok())
                .is_some_and(|state| state.visible)
        );
        registry.commit_visible(&[]);
        assert!(!registry.connectors.contains_key(&connector_id));
    }
}
