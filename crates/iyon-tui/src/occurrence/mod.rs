//! Host-owned direct UI occurrences.
//!
//! This module is deliberately independent from React, N-API, terminal
//! backends and layout drivers.  It owns one accepted topology/property
//! document and exposes a short-lived typed commit plan to its future
//! transport boundary.

pub(crate) mod arena;
pub(crate) mod generated;
pub(crate) mod properties;
pub(crate) mod tree;

pub(crate) mod commit;
pub(crate) mod config;
pub(crate) mod control;

use std::collections::{HashMap, HashSet};

pub(crate) use arena::Arena;
pub use arena::{ArenaError, HostNamespace, NodeKey, ResourceKey, UiHandle};
pub(crate) use commit::UiChangeSet;
pub use commit::{
    CommitDetail, FunnelSpec, NodeRef, ResourceRef, UiAcknowledgement, UiCommit, UiOperation,
    UiOperationResult, UiRejection,
};
pub use config::{ConfigError, ControlConfig, RootConfig};
pub use control::{AnimationState, ControlError, ControlState, EditorState, ScrollState};
pub use generated::{ControlKind, HandleKind, HostKind, OwnershipMode, PropertyId, RootRole};
pub use properties::{
    Alignment, AlignmentAxis, ColorValue, Edges, GlyphsValue, LayerValue, LayoutMode,
    PropertyError, PropertyLayer, PropertyValue, SizeMode, StyleValue, TextAttributes,
};
pub(crate) use tree::{Attachments, Occurrence};

const DEFAULT_ARENA_CAPACITY: usize = 1_048_576;

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OccurrenceDocument>();
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceFamily {
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
pub struct ResourceRecord {
    pub(crate) family: ResourceFamily,
    pub(crate) ownership: OwnershipMode,
    pub(crate) owner: Option<NodeKey>,
    pub(crate) port: Option<ResourceKey>,
    pub(crate) selected: Option<ResourceKey>,
    pub(crate) source_index: Option<u32>,
    pub(crate) content_family: Option<u32>,
    pub(crate) control_kind: Option<ControlKind>,
}

/// Immutable renderer-facing occurrence data.  This is intentionally a
/// value snapshot: the legacy adapter never retains a reference into the
/// mutable document while a terminal frame is being prepared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OccurrenceSnapshot {
    pub(crate) key: NodeKey,
    pub(crate) kind: HostKind,
    pub(crate) root_role: Option<RootRole>,
    pub(crate) children: Vec<NodeKey>,
    pub(crate) port: Option<ResourceKey>,
    pub(crate) control: Option<ResourceKey>,
    pub(crate) hidden: bool,
    pub(crate) subscriptions: u64,
    pub(crate) history_action: Option<u32>,
    pub(crate) properties: Vec<(PropertyId, LayerValue)>,
    pub(crate) style_states: Vec<(String, String)>,
    pub(crate) structure_revision: u64,
    pub(crate) geometry_revision: u64,
    pub(crate) presentation_revision: u64,
    pub(crate) interaction_revision: u64,
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
pub enum HandleError {
    WrongHost,
    WrongKind,
    Invalid,
    Stale,
}

pub struct OccurrenceDocument {
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
    pub fn new(namespace: HostNamespace) -> Self {
        Self::try_new(namespace).expect("a fresh occurrence document must be constructible")
    }

    pub fn try_new(namespace: HostNamespace) -> Result<Self, ArenaError> {
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

    pub fn namespace(&self) -> HostNamespace {
        self.namespace
    }

    pub fn accepted_ui_revision(&self) -> u64 {
        self.accepted_ui_revision
    }

    pub fn body_root(&self) -> NodeKey {
        self.body_root
    }

    pub fn body_handle(&self) -> UiHandle {
        self.body_root.handle(self.namespace)
    }

    pub fn live_node_count(&self) -> usize {
        self.nodes.live_count()
    }

    pub fn live_resource_count(&self, family: ResourceFamily) -> usize {
        match family {
            ResourceFamily::Port => self.ports.live_count(),
            ResourceFamily::Connector => self.connectors.live_count(),
            ResourceFamily::Control => self.controls.live_count(),
        }
    }

    #[must_use]
    pub fn resource_is_live(&self, key: ResourceKey) -> bool {
        self.resource_record(key).is_ok()
    }

    pub(crate) fn snapshot(&self, key: NodeKey) -> Result<OccurrenceSnapshot, HandleError> {
        let occurrence = self
            .nodes
            .get(key.slot, key.generation)
            .map_err(|error| match error {
                ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                ArenaError::StaleKey => HandleError::Stale,
            })?;
        let mut children = Vec::with_capacity(occurrence.links.child_count as usize);
        let mut child = occurrence.links.first_child;
        while let Some(key) = child {
            let record = self
                .nodes
                .get(key.slot, key.generation)
                .map_err(|error| match error {
                    ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                    ArenaError::StaleKey => HandleError::Stale,
                })?;
            children.push(key);
            child = record.links.next_sibling;
            if children.len() > occurrence.links.child_count as usize {
                return Err(HandleError::Invalid);
            }
        }
        Ok(OccurrenceSnapshot {
            key,
            kind: occurrence.kind,
            root_role: occurrence.root_role,
            children,
            port: occurrence.attachments.port,
            control: occurrence.attachments.control,
            hidden: occurrence.renderer_hidden,
            subscriptions: occurrence.subscriptions,
            history_action: occurrence.history_action,
            properties: occurrence.properties.effective_values(),
            style_states: Self::effective_style_states(occurrence),
            structure_revision: occurrence.revisions.structure,
            geometry_revision: occurrence.revisions.geometry,
            presentation_revision: occurrence.revisions.presentation,
            interaction_revision: occurrence.revisions.interaction,
        })
    }

    fn effective_style_states(occurrence: &Occurrence) -> Vec<(String, String)> {
        let mut states = occurrence.style_states.clone();
        for (key, value) in &occurrence.style_overrides {
            states.insert(key.clone(), value.clone());
        }
        states.into_iter().collect()
    }

    pub(crate) fn selected_connector(&self, port: ResourceKey) -> Option<ResourceKey> {
        self.resource_record(port)
            .ok()
            .and_then(|record| record.selected)
    }

    pub(crate) fn nodes_for_control(&self, control: ResourceKey) -> Vec<NodeKey> {
        self.nodes
            .iter()
            .filter_map(|(slot, generation, occurrence)| {
                (occurrence.attachments.control == Some(control))
                    .then_some(NodeKey { slot, generation })
            })
            .collect()
    }

    pub(crate) fn nodes_for_port(&self, port: ResourceKey) -> Vec<NodeKey> {
        self.nodes
            .iter()
            .filter_map(|(slot, generation, occurrence)| {
                (occurrence.attachments.port == Some(port)).then_some(NodeKey { slot, generation })
            })
            .collect()
    }

    pub(crate) fn port_owner(&self, port: ResourceKey) -> Option<NodeKey> {
        self.port_owners.get(&port).copied()
    }

    pub(crate) fn control_owner(&self, control: ResourceKey) -> Option<NodeKey> {
        self.control_owners.get(&control).copied()
    }

    pub(crate) fn ports_under_nodes(
        &self,
        roots: &[NodeKey],
    ) -> Result<(Vec<ResourceKey>, usize), HandleError> {
        let mut stack = roots.to_vec();
        let mut seen = HashSet::new();
        let mut ports = Vec::new();
        let mut visited = 0usize;
        while let Some(key) = stack.pop() {
            if !seen.insert(key) {
                continue;
            }
            let occurrence =
                self.nodes
                    .get(key.slot, key.generation)
                    .map_err(|error| match error {
                        ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                        ArenaError::StaleKey => HandleError::Stale,
                    })?;
            visited = visited.saturating_add(1);
            if let Some(port) = occurrence.attachments.port {
                ports.push(port);
            }
            let mut child = occurrence.links.first_child;
            while let Some(child_key) = child {
                stack.push(child_key);
                child = self
                    .nodes
                    .get(child_key.slot, child_key.generation)
                    .map_err(|error| match error {
                        ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                        ArenaError::StaleKey => HandleError::Stale,
                    })?
                    .links
                    .next_sibling;
            }
            if let Some(portals) = self.portals_by_owner.get(&key) {
                stack.extend(portals.iter().copied());
            }
        }
        ports.sort_unstable_by_key(|key| (key.kind as u32, key.slot, key.generation));
        ports.dedup();
        Ok((ports, visited))
    }

    pub(crate) fn demanded_node<F>(
        &self,
        node: NodeKey,
        active_animation: F,
    ) -> Result<(bool, usize), HandleError>
    where
        F: Fn(ResourceKey) -> Option<usize>,
    {
        let mut current = node;
        let mut visited = 0usize;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(current) {
                return Err(HandleError::Invalid);
            }
            let occurrence = self
                .nodes
                .get(current.slot, current.generation)
                .map_err(|error| match error {
                    ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                    ArenaError::StaleKey => HandleError::Stale,
                })?;
            visited = visited.saturating_add(1);
            if occurrence.renderer_hidden {
                return Ok((false, visited));
            }
            if let Some(parent) = occurrence.links.parent {
                let parent_record = self.nodes.get(parent.slot, parent.generation).map_err(
                    |error| match error {
                        ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                        ArenaError::StaleKey => HandleError::Stale,
                    },
                )?;
                if parent_record.kind == HostKind::Animation {
                    let active = parent_record
                        .attachments
                        .control
                        .and_then(&active_animation);
                    if self.child_at(parent, active.unwrap_or(0))? != Some(current) {
                        return Ok((false, visited));
                    }
                }
                current = parent;
                continue;
            }
            if let Some(owner) = occurrence.root_owner {
                current = owner;
                continue;
            }
            if occurrence.root_role.is_some() {
                return Ok((true, visited));
            }
            return Err(HandleError::Invalid);
        }
    }

    fn child_at(&self, parent: NodeKey, index: usize) -> Result<Option<NodeKey>, HandleError> {
        let parent_record = self
            .nodes
            .get(parent.slot, parent.generation)
            .map_err(|error| match error {
                ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                ArenaError::StaleKey => HandleError::Stale,
            })?;
        let mut child = parent_record.links.first_child;
        let mut position = 0usize;
        while let Some(key) = child {
            if position == index {
                return Ok(Some(key));
            }
            let child_record =
                self.nodes
                    .get(key.slot, key.generation)
                    .map_err(|error| match error {
                        ArenaError::InvalidKey | ArenaError::Capacity => HandleError::Invalid,
                        ArenaError::StaleKey => HandleError::Stale,
                    })?;
            position = position.saturating_add(1);
            if position > parent_record.links.child_count as usize {
                return Err(HandleError::Invalid);
            }
            child = child_record.links.next_sibling;
        }
        Ok(None)
    }

    pub(crate) fn control_for_node(&self, node: NodeKey) -> Option<ResourceKey> {
        self.nodes
            .get(node.slot, node.generation)
            .ok()
            .and_then(|occurrence| occurrence.attachments.control)
    }

    pub(crate) fn parent_of(&self, node: NodeKey) -> Option<NodeKey> {
        self.nodes
            .get(node.slot, node.generation)
            .ok()
            .and_then(|occurrence| occurrence.links.parent)
    }

    pub(crate) fn root_owner(&self, node: NodeKey) -> Option<NodeKey> {
        self.nodes
            .get(node.slot, node.generation)
            .ok()
            .and_then(|occurrence| occurrence.root_owner)
    }

    pub(crate) fn resource_port(&self, key: ResourceKey) -> Option<ResourceKey> {
        match key.kind {
            HandleKind::Port => Some(key),
            HandleKind::Connector => self.resource_record(key).ok()?.port,
            HandleKind::Control | HandleKind::Node => None,
        }
    }

    pub(crate) fn portal_roots(&self) -> Vec<NodeKey> {
        let mut roots = self
            .roots
            .iter()
            .copied()
            .filter(|key| {
                self.nodes
                    .get(key.slot, key.generation)
                    .is_ok_and(|record| record.root_role == Some(RootRole::Portal))
            })
            .collect::<Vec<_>>();
        roots.sort_unstable_by_key(|key| (key.slot, key.generation));
        roots
    }

    pub(crate) fn history_roots(&self) -> Vec<NodeKey> {
        let mut roots = self
            .roots
            .iter()
            .copied()
            .filter(|key| {
                self.nodes
                    .get(key.slot, key.generation)
                    .is_ok_and(|record| record.root_role == Some(RootRole::LegacyHistoryUnit))
            })
            .collect::<Vec<_>>();
        roots.sort_unstable_by_key(|key| (key.slot, key.generation));
        roots
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
