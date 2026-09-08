//! Host-owned direct UI occurrences.
//!
//! This module is deliberately independent from React, N-API, terminal
//! backends and layout drivers.  It owns one accepted topology/property
//! document and exposes a short-lived typed commit plan to its future
//! transport boundary.

mod arena;
pub(crate) mod generated;
mod properties;
mod tree;

mod commit;

use std::collections::{HashMap, HashSet};

pub(crate) use arena::{Arena, ArenaError, HostNamespace, NodeKey, ResourceKey, UiHandle};
pub(crate) use commit::{
    AppliedUiCommit, CommitDetail, NodeRef, PreparedUiCommit, ResourceRef, UiCommit, UiOperation,
    UiOperationResult, UiRejection, UiStatus,
};
pub(crate) use generated::{
    ControlKind, EffectMask, HandleKind, HostKind, OwnershipMode, PropertyId, RootRole,
};
pub(crate) use properties::{
    Alignment, AlignmentAxis, BorderStyle, ColorValue, Edges, GlyphsValue, Insets, LayerValue,
    PropertyError, PropertyLayer, PropertyValue, SizeMode, StyleValue, TextAttributes,
};
pub(crate) use tree::{Attachments, DirtyState, Links, Occurrence, Revisions, TreeError};

const DEFAULT_ARENA_CAPACITY: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ResourceFamily {
    Port,
    Connector,
    Control,
}

impl ResourceFamily {
    const fn handle_kind(self) -> HandleKind {
        match self {
            Self::Port => HandleKind::Port,
            Self::Connector => HandleKind::Connector,
            Self::Control => HandleKind::Control,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceRecord {
    pub(crate) family: ResourceFamily,
    pub(crate) ownership: OwnershipMode,
    pub(crate) owner: Option<NodeKey>,
    pub(crate) port: Option<ResourceKey>,
    pub(crate) selected: Option<ResourceKey>,
    pub(crate) source_index: Option<u32>,
    pub(crate) content_family: Option<u32>,
    pub(crate) control_kind: Option<ControlKind>,
}

impl ResourceRecord {
    fn port(ownership: OwnershipMode, owner: Option<NodeKey>, content_family: u32) -> Self {
        Self {
            family: ResourceFamily::Port,
            ownership,
            owner,
            port: None,
            selected: None,
            source_index: None,
            content_family: Some(content_family),
            control_kind: None,
        }
    }

    fn connector(ownership: OwnershipMode, port: ResourceKey, source_index: u32) -> Self {
        Self {
            family: ResourceFamily::Connector,
            ownership,
            owner: None,
            port: Some(port),
            selected: None,
            source_index: Some(source_index),
            content_family: None,
            control_kind: None,
        }
    }

    fn control(
        ownership: OwnershipMode,
        owner: Option<NodeKey>,
        control_kind: ControlKind,
    ) -> Self {
        Self {
            family: ResourceFamily::Control,
            ownership,
            owner,
            port: None,
            selected: None,
            source_index: None,
            content_family: None,
            control_kind: Some(control_kind),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HandleError {
    WrongHost,
    WrongKind,
    Invalid,
    Stale,
}

pub(crate) struct OccurrenceDocument {
    namespace: HostNamespace,
    nodes: Arena<Occurrence>,
    ports: Arena<ResourceRecord>,
    connectors: Arena<ResourceRecord>,
    controls: Arena<ResourceRecord>,
    port_owners: HashMap<ResourceKey, NodeKey>,
    control_owners: HashMap<ResourceKey, NodeKey>,
    connectors_by_port: HashMap<ResourceKey, HashSet<ResourceKey>>,
    ports_by_selected_connector: HashMap<ResourceKey, HashSet<ResourceKey>>,
    portals_by_owner: HashMap<NodeKey, HashSet<NodeKey>>,
    roots: HashSet<NodeKey>,
    body_root: NodeKey,
    accepted_ui_revision: u64,
    arena_capacity: usize,
}

impl OccurrenceDocument {
    pub(crate) fn new(namespace: HostNamespace) -> Self {
        Self::try_new(namespace).expect("a fresh occurrence document must be constructible")
    }

    pub(crate) fn try_new(namespace: HostNamespace) -> Result<Self, ArenaError> {
        let mut nodes = Arena::new();
        let body_keys = nodes.preview_node_keys(1)?;
        let body_root = body_keys[0];
        nodes.insert_reserved_node(
            body_root,
            Occurrence::new(HostKind::Box, Some(RootRole::Body)),
        );
        let mut roots = HashSet::new();
        roots.insert(body_root);
        Ok(Self {
            namespace,
            nodes,
            ports: Arena::new(),
            connectors: Arena::new(),
            controls: Arena::new(),
            port_owners: HashMap::new(),
            control_owners: HashMap::new(),
            connectors_by_port: HashMap::new(),
            ports_by_selected_connector: HashMap::new(),
            portals_by_owner: HashMap::new(),
            roots,
            body_root,
            accepted_ui_revision: 0,
            arena_capacity: DEFAULT_ARENA_CAPACITY,
        })
    }

    pub(crate) fn namespace(&self) -> HostNamespace {
        self.namespace
    }

    pub(crate) fn accepted_ui_revision(&self) -> u64 {
        self.accepted_ui_revision
    }

    pub(crate) fn body_root(&self) -> NodeKey {
        self.body_root
    }

    pub(crate) fn body_handle(&self) -> UiHandle {
        self.body_root.handle(self.namespace)
    }

    pub(crate) fn live_node_count(&self) -> usize {
        self.nodes.live_count()
    }

    pub(crate) fn live_resource_count(&self, family: ResourceFamily) -> usize {
        match family {
            ResourceFamily::Port => self.ports.live_count(),
            ResourceFamily::Connector => self.connectors.live_count(),
            ResourceFamily::Control => self.controls.live_count(),
        }
    }

    fn node_key(&self, handle: UiHandle) -> Result<NodeKey, HandleError> {
        if handle.host_namespace != self.namespace.get() {
            return Err(HandleError::WrongHost);
        }
        if handle.kind != HandleKind::Node {
            return Err(HandleError::WrongKind);
        }
        let key = handle.node_key().ok_or(HandleError::WrongKind)?;
        self.nodes
            .get(key.slot, key.generation)
            .map(|_| key)
            .map_err(|error| match error {
                ArenaError::InvalidKey => HandleError::Invalid,
                ArenaError::StaleKey => HandleError::Stale,
                ArenaError::Capacity => HandleError::Invalid,
            })
    }

    fn resource_key(
        &self,
        handle: UiHandle,
        family: ResourceFamily,
    ) -> Result<ResourceKey, HandleError> {
        if handle.host_namespace != self.namespace.get() {
            return Err(HandleError::WrongHost);
        }
        let expected_kind = family.handle_kind();
        if handle.kind != expected_kind {
            return Err(HandleError::WrongKind);
        }
        let key = handle.resource_key().ok_or(HandleError::WrongKind)?;
        self.resource_record(key)
            .map(|_| key)
            .map_err(|error| match error {
                ArenaError::InvalidKey => HandleError::Invalid,
                ArenaError::StaleKey => HandleError::Stale,
                ArenaError::Capacity => HandleError::Invalid,
            })
    }

    fn resource_record(&self, key: ResourceKey) -> Result<&ResourceRecord, ArenaError> {
        match key.kind {
            HandleKind::Port => self.ports.get(key.slot, key.generation),
            HandleKind::Connector => self.connectors.get(key.slot, key.generation),
            HandleKind::Control => self.controls.get(key.slot, key.generation),
            HandleKind::Node => Err(ArenaError::InvalidKey),
        }
    }

    fn resource_arena_mut(&mut self, family: ResourceFamily) -> &mut Arena<ResourceRecord> {
        match family {
            ResourceFamily::Port => &mut self.ports,
            ResourceFamily::Connector => &mut self.connectors,
            ResourceFamily::Control => &mut self.controls,
        }
    }

    fn reserve_node_keys(&mut self, count: usize) -> Result<Vec<NodeKey>, ArenaError> {
        self.nodes.preview_node_keys(count)
    }

    fn reserve_resource_keys(
        &mut self,
        family: ResourceFamily,
        count: usize,
    ) -> Result<Vec<ResourceKey>, ArenaError> {
        self.resource_arena_mut(family)
            .preview_resource_keys(count, family.handle_kind())
    }
}
