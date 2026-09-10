use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use super::config::{ConfigError, ControlConfig, RootConfig};
use super::{
    OccurrenceDocument, ResourceFamily, ResourceRecord,
    arena::{NodeKey, ResourceKey, UiHandle},
    generated::{
        ControlKind, EFFECT_CONTENT_PROJECTION, EFFECT_INTERACTION_RUNTIME, EFFECT_LAYOUT_INPUT,
        EFFECT_PRESENTATION, EFFECT_STRUCTURE_GUARD, EffectMask, HandleKind, HostKind,
        OwnershipMode, PropertyId, RootRole, property_descriptor,
    },
    properties::{LayerValue, PropertyLayer, PropertyLayers},
    tree::{Occurrence, TreeDraft, TreeError, TreePlan},
};

const STATUS_OK: u32 = 0;
const STATUS_REJECTED: u32 = 0x8000_0000;
const FAILED_RECORD_NONE: u32 = u32::MAX;
const WAKE_DRAIN: u32 = 1;

#[derive(Clone, Debug, Default)]
pub struct UiCommit {
    expected_ui_revision: u64,
    operations: Vec<UiOperation>,
    funnel_specs: HashMap<u32, FunnelSpec>,
    control_configs: HashMap<u32, ControlConfig>,
    root_configs: HashMap<u32, RootConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunnelSpec {
    pub kind: u32,
    pub wrap: u32,
    pub hyperlinks: bool,
    pub smooth: bool,
}

impl UiCommit {
    pub fn new(expected_ui_revision: u64) -> Self {
        Self {
            expected_ui_revision,
            operations: Vec::new(),
            funnel_specs: HashMap::new(),
            control_configs: HashMap::new(),
            root_configs: HashMap::new(),
        }
    }

    pub fn push(&mut self, operation: UiOperation) {
        self.operations.push(operation);
    }

    pub fn with_operations(expected_ui_revision: u64, operations: Vec<UiOperation>) -> Self {
        Self {
            expected_ui_revision,
            operations,
            funnel_specs: HashMap::new(),
            control_configs: HashMap::new(),
            root_configs: HashMap::new(),
        }
    }

    pub fn expected_ui_revision(&self) -> u64 {
        self.expected_ui_revision
    }

    pub fn operations(&self) -> &[UiOperation] {
        &self.operations
    }

    pub fn set_funnel_for_connector(&mut self, local_ordinal: u32, funnel: FunnelSpec) {
        self.funnel_specs.insert(local_ordinal, funnel);
    }

    #[must_use]
    pub fn funnel_for_connector(&self, local_ordinal: u32) -> FunnelSpec {
        self.funnel_specs
            .get(&local_ordinal)
            .copied()
            .unwrap_or(FunnelSpec {
                kind: 0,
                wrap: 0,
                hyperlinks: true,
                smooth: false,
            })
    }

    pub fn set_control_config(&mut self, local_ordinal: u32, config: ControlConfig) {
        self.control_configs.insert(local_ordinal, config);
    }

    #[must_use]
    pub fn control_config(&self, local_ordinal: u32) -> ControlConfig {
        self.control_configs
            .get(&local_ordinal)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_root_config(&mut self, local_ordinal: u32, config: RootConfig) {
        self.root_configs.insert(local_ordinal, config);
    }

    #[must_use]
    pub fn root_config(&self, local_ordinal: u32) -> RootConfig {
        self.root_configs
            .get(&local_ordinal)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeRef {
    Existing(UiHandle),
    Local(u32),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceRef {
    Existing(UiHandle),
    Local(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiOperation {
    CreateNode {
        local_ordinal: u32,
        kind: HostKind,
    },
    CreateRoot {
        local_ordinal: u32,
        role: RootRole,
        owner: Option<NodeRef>,
    },
    InsertBefore {
        parent: NodeRef,
        child: NodeRef,
        before: Option<NodeRef>,
    },
    Detach {
        parent: NodeRef,
        child: NodeRef,
    },
    RetireSubtree {
        root: NodeRef,
    },
    RetireRoot {
        root: NodeRef,
    },
    AttachPort {
        node: NodeRef,
        port: Option<ResourceRef>,
    },
    AttachControl {
        node: NodeRef,
        control: Option<ResourceRef>,
    },
    CreatePort {
        local_ordinal: u32,
        content_family: u32,
        ownership: OwnershipMode,
        owner: Option<NodeRef>,
    },
    CreateConnector {
        local_ordinal: u32,
        source_index: u32,
        port: ResourceRef,
        ownership: OwnershipMode,
    },
    SelectConnector {
        port: ResourceRef,
        connector: Option<ResourceRef>,
    },
    DisposePort {
        port: ResourceRef,
    },
    DisposeConnector {
        connector: ResourceRef,
    },
    SetLiteralFunnel {
        port: ResourceRef,
        kind: u32,
        wrap: u32,
        hyperlinks: bool,
        smooth: bool,
    },
    ReplaceLiteral {
        port: ResourceRef,
        content_format: u32,
        content: Vec<u8>,
        annotations: Vec<u8>,
    },
    CreateControl {
        local_ordinal: u32,
        kind: super::generated::ControlKind,
        ownership: OwnershipMode,
        owner: Option<NodeRef>,
    },
    DisposeControl {
        control: ResourceRef,
    },
    HistoryAction {
        root: NodeRef,
        action_id: u32,
    },
    SetDeclared {
        node: NodeRef,
        property: PropertyId,
        value: LayerValue,
    },
    ResetDeclared {
        node: NodeRef,
        property: PropertyId,
    },
    SetOverride {
        node: NodeRef,
        property: PropertyId,
        value: LayerValue,
    },
    ClearOverride {
        node: NodeRef,
        property: PropertyId,
    },
    SetHidden {
        node: NodeRef,
        hidden: bool,
    },
    SetStyleState {
        node: NodeRef,
        layer: u32,
        key: String,
        value: String,
    },
    ClearStyleState {
        node: NodeRef,
        layer: u32,
        key: String,
    },
    SetSubscriptions {
        node: NodeRef,
        mask_low: u32,
        mask_high: u32,
    },
    ControlCommand {
        control: ResourceRef,
        command_id: u32,
        operands: Vec<u32>,
    },
    ReplaceEditorContent {
        control: ResourceRef,
        content: Vec<u8>,
        expected_edit_revision: u64,
    },
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiStatus {
    Accepted = STATUS_OK,
    Rejected = STATUS_REJECTED,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitDetail {
    Malformed = 1,
    StaleRevision = 2,
    StaleHandle = 3,
    WrongHost = 4,
    WrongKind = 5,
    InvalidTopology = 6,
    InUseDisposal = 7,
    Capacity = 8,
    InvalidProperty = 9,
    Unsupported = 10,
    Invariant = 11,
}

impl CommitDetail {
    const fn code(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiAcknowledgement {
    pub status: UiStatus,
    pub accepted_ui_revision: u64,
    pub created_count: u32,
    pub failed_record: u32,
    pub detail_code: u32,
    pub wake_flags: u32,
    pub created: Box<[UiHandle]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiOperationResult {
    pub acknowledgement: UiAcknowledgement,
    /// The exact eight-word acknowledgement header followed by four words per
    /// created handle.  This storage is prepared before authoritative apply.
    pub words: Box<[u32]>,
}

pub type AppliedUiCommit = UiOperationResult;

impl UiOperationResult {
    fn accepted(revision: u64, created: Vec<UiHandle>, wake_flags: u32) -> Self {
        Self::new(
            UiStatus::Accepted,
            revision,
            created,
            FAILED_RECORD_NONE,
            0,
            wake_flags,
        )
    }

    pub fn rejected(revision: u64, failed_record: u32, detail: CommitDetail) -> Self {
        Self::new(
            UiStatus::Rejected,
            revision,
            Vec::new(),
            failed_record,
            detail.code(),
            0,
        )
    }

    fn new(
        status: UiStatus,
        accepted_ui_revision: u64,
        created: Vec<UiHandle>,
        failed_record: u32,
        detail_code: u32,
        wake_flags: u32,
    ) -> Self {
        let created = created.into_boxed_slice();
        let created_count =
            u32::try_from(created.len()).expect("prepared acknowledgement count fits u32");
        let word_count = 8usize
            .checked_add(
                created
                    .len()
                    .checked_mul(4)
                    .expect("prepared acknowledgement size fits usize"),
            )
            .expect("prepared acknowledgement size fits usize");
        let mut words = Vec::with_capacity(word_count);
        words.push(status as u32);
        words.push(
            u32::try_from(accepted_ui_revision & u64::from(u32::MAX))
                .expect("masked UI revision fits u32"),
        );
        words
            .push(u32::try_from(accepted_ui_revision >> 32).expect("shifted UI revision fits u32"));
        words.push(created_count);
        words.push(failed_record);
        words.push(detail_code);
        words.push(wake_flags);
        words.push(0);
        for handle in &created {
            words.extend_from_slice(&[
                handle.host_namespace,
                handle.slot,
                handle.generation,
                handle.kind as u32,
            ]);
        }
        debug_assert_eq!(words.len(), word_count);
        Self {
            acknowledgement: UiAcknowledgement {
                status,
                accepted_ui_revision,
                created_count,
                failed_record,
                detail_code,
                wake_flags,
                created,
            },
            words: words.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiRejection {
    pub result: UiOperationResult,
    pub detail: CommitDetail,
    pub message: String,
}

impl UiRejection {
    pub fn internal(revision: u64, detail: CommitDetail, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            result: UiOperationResult::rejected(revision, u32::MAX, detail),
            detail,
            message,
        }
    }
}

impl fmt::Display for UiRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "UI commit rejected ({:?}, record {}): {}",
            self.detail,
            if self.result.acknowledgement.failed_record == FAILED_RECORD_NONE {
                "none".to_owned()
            } else {
                self.result.acknowledgement.failed_record.to_string()
            },
            self.message
        )
    }
}

#[derive(Debug)]
struct CommitIssue {
    detail: CommitDetail,
    message: String,
}

impl CommitIssue {
    fn new(detail: CommitDetail, message: impl Into<String>) -> Self {
        Self {
            detail,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]

enum LocalFamily {
    Node,
    Port,
    Connector,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigTarget {
    Control(ControlKind),
    Root(RootRole),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalObject {
    Node(NodeKey),
    Resource(ResourceKey),
}

struct ResourceDraft<'a> {
    document: &'a OccurrenceDocument,
    edits: HashMap<ResourceKey, ResourceRecord>,
    created: Vec<ResourceKey>,
    created_set: HashSet<ResourceKey>,
    created_connectors_by_port: HashMap<ResourceKey, HashSet<ResourceKey>>,
    retired: Vec<ResourceKey>,
    retired_set: HashSet<ResourceKey>,
    owner_changes: HashMap<ResourceKey, Option<NodeKey>>,
    resource_ports: HashMap<ResourceKey, Option<ResourceKey>>,
}

pub struct PreparedUiCommit {
    expected_ui_revision: u64,
    next_ui_revision: u64,
    pub(crate) changed: bool,
    pub(crate) effects: EffectMask,
    pub(crate) physical_work: bool,
    tree: TreePlan,
    resources: ResourcePlan,
    roots_added: Vec<NodeKey>,
    roots_removed: Vec<NodeKey>,
    pub(crate) history_roots: Vec<HistoryRootSummary>,
    result: UiOperationResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryRootSummary {
    pub(crate) root: NodeKey,
    pub(crate) action: Option<u32>,
    pub(crate) has_live_control: bool,
}

/// Native-only summary of one accepted occurrence commit.  It is the bridge
/// between the validated commit plan and derived renderer projections; it is
/// not a replayable operation log or a second semantic protocol.
#[derive(Clone, Debug, Default)]
pub(crate) struct UiChangeSet {
    pub(crate) ui_revision: u64,
    pub(crate) changed_nodes: Vec<NodeKey>,
    pub(crate) retired_nodes: Vec<NodeKey>,
    pub(crate) history_roots: Vec<NodeKey>,
    pub(crate) changed_resources: Vec<ResourceKey>,
    pub(crate) resource_ports: Vec<(ResourceKey, Option<ResourceKey>)>,
    pub(crate) membership_nodes: Vec<NodeKey>,
    pub(crate) effects: EffectMask,
    pub(crate) physical_work: bool,
}

impl UiChangeSet {
    pub(crate) fn is_empty(&self) -> bool {
        self.changed_nodes.is_empty()
            && self.retired_nodes.is_empty()
            && self.history_roots.is_empty()
            && self.changed_resources.is_empty()
            && self.resource_ports.is_empty()
            && self.membership_nodes.is_empty()
            && !self.physical_work
    }

    pub(crate) fn has_work(&self) -> bool {
        !self.is_empty()
    }

    pub(crate) fn merge(&mut self, next: UiChangeSet) {
        self.ui_revision = next.ui_revision;
        self.changed_nodes.extend(next.changed_nodes);
        self.retired_nodes.extend(next.retired_nodes);
        self.history_roots.extend(next.history_roots);
        self.changed_resources.extend(next.changed_resources);
        self.resource_ports.extend(next.resource_ports);
        self.membership_nodes.extend(next.membership_nodes);
        self.effects = self.effects.union(next.effects);
        self.physical_work |= next.physical_work;
        self.changed_nodes
            .sort_unstable_by_key(|key| (key.slot, key.generation));
        self.changed_nodes.dedup();
        self.retired_nodes
            .sort_unstable_by_key(|key| (key.slot, key.generation));
        self.retired_nodes.dedup();
        self.history_roots
            .sort_unstable_by_key(|key| (key.slot, key.generation));
        self.history_roots.dedup();
        self.changed_resources
            .sort_unstable_by_key(|key| (key.kind as u32, key.slot, key.generation));
        self.changed_resources.dedup();
        self.resource_ports
            .sort_unstable_by_key(|(key, _)| (key.kind as u32, key.slot, key.generation));
        self.resource_ports.dedup_by_key(|(key, _)| *key);
        self.membership_nodes
            .sort_unstable_by_key(|key| (key.slot, key.generation));
        self.membership_nodes.dedup();
        self.changed_nodes
            .retain(|key| !self.retired_nodes.contains(key));
        self.membership_nodes
            .retain(|key| !self.retired_nodes.contains(key));
        self.history_roots
            .retain(|key| !self.retired_nodes.contains(key));
    }
}

impl PreparedUiCommit {
    #[must_use]
    pub fn acknowledgement(&self) -> &UiAcknowledgement {
        &self.result.acknowledgement
    }

    #[must_use]
    pub fn retired_resource_keys(&self) -> Vec<ResourceKey> {
        self.resources.retired.clone()
    }

    #[must_use]
    pub fn retired_node_keys(&self) -> Vec<NodeKey> {
        self.tree.retired.clone()
    }

    pub(crate) fn added_root_keys(&self) -> &[NodeKey] {
        &self.roots_added
    }

    pub(crate) fn change_set(&self) -> UiChangeSet {
        let mut changed_nodes = self.tree.changed_nodes.iter().copied().collect::<Vec<_>>();
        changed_nodes.sort_unstable_by_key(|key| (key.slot, key.generation));
        let mut changed_resources = self.resources.changed_keys();
        changed_resources.sort_unstable_by_key(|key| (key.kind as u32, key.slot, key.generation));
        changed_resources.dedup();
        let mut resource_ports = self
            .resources
            .resource_ports
            .iter()
            .map(|(key, port)| (*key, *port))
            .collect::<Vec<_>>();
        resource_ports.sort_unstable_by_key(|(key, _)| (key.kind as u32, key.slot, key.generation));
        UiChangeSet {
            ui_revision: self.next_ui_revision,
            changed_nodes,
            retired_nodes: self.tree.retired.clone(),
            history_roots: self
                .history_roots
                .iter()
                .map(|summary| summary.root)
                .collect(),
            physical_work: self.physical_work || !changed_resources.is_empty(),
            changed_resources,
            resource_ports,
            membership_nodes: self.tree.membership_nodes.iter().copied().collect(),
            effects: self.effects,
        }
    }

    pub(crate) fn history_root_summaries(&self) -> &[HistoryRootSummary] {
        &self.history_roots
    }
}

struct ResourcePlan {
    edits: HashMap<ResourceKey, ResourceRecord>,
    created: Vec<ResourceKey>,
    retired: Vec<ResourceKey>,
    retired_set: HashSet<ResourceKey>,
    owner_changes: HashMap<ResourceKey, Option<NodeKey>>,
    resource_ports: HashMap<ResourceKey, Option<ResourceKey>>,
    connector_buckets: HashMap<ResourceKey, HashSet<ResourceKey>>,
    connector_touched: HashSet<ResourceKey>,
    selected_buckets: HashMap<ResourceKey, HashSet<ResourceKey>>,
    selected_touched: HashSet<ResourceKey>,
}

impl ResourcePlan {
    fn changed_keys(&self) -> Vec<ResourceKey> {
        self.edits
            .keys()
            .copied()
            .chain(self.created.iter().copied())
            .chain(self.retired.iter().copied())
            .collect()
    }
}

impl<'a> ResourceDraft<'a> {
    fn new(document: &'a OccurrenceDocument) -> Self {
        Self {
            document,
            edits: HashMap::new(),
            created: Vec::new(),
            created_set: HashSet::new(),
            created_connectors_by_port: HashMap::new(),
            retired: Vec::new(),
            retired_set: HashSet::new(),
            owner_changes: HashMap::new(),
            resource_ports: HashMap::new(),
        }
    }

    fn reserve(&mut self, additional: usize) -> Result<(), CommitIssue> {
        self.edits
            .try_reserve(additional)
            .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "resource overlay capacity"))?;
        self.created
            .try_reserve(additional)
            .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "resource creation capacity"))?;
        self.created_set
            .try_reserve(additional)
            .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "resource index capacity"))?;
        self.created_connectors_by_port
            .try_reserve(additional)
            .map_err(|_| {
                CommitIssue::new(CommitDetail::Capacity, "Connector dependency capacity")
            })?;
        self.retired.try_reserve(additional).map_err(|_| {
            CommitIssue::new(CommitDetail::Capacity, "resource retirement capacity")
        })?;
        self.retired_set.try_reserve(additional).map_err(|_| {
            CommitIssue::new(CommitDetail::Capacity, "resource retire index capacity")
        })?;
        self.owner_changes
            .try_reserve(additional)
            .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "resource owner capacity"))?;
        self.resource_ports.try_reserve(additional).map_err(|_| {
            CommitIssue::new(CommitDetail::Capacity, "resource association capacity")
        })?;
        Ok(())
    }

    fn insert_created(
        &mut self,
        key: ResourceKey,
        record: ResourceRecord,
    ) -> Result<(), CommitIssue> {
        if self.edits.contains_key(&key) {
            return Err(CommitIssue::new(
                CommitDetail::Invariant,
                "duplicate provisional resource key",
            ));
        }
        self.created.push(key);
        self.created_set.insert(key);
        if let Some(port) = record.port {
            self.created_connectors_by_port
                .entry(port)
                .or_default()
                .insert(key);
        }
        self.edits.insert(key, record);
        if key.kind == HandleKind::Connector {
            self.resource_ports.insert(key, self.edits[&key].port);
        }
        Ok(())
    }

    fn read(&self, key: ResourceKey) -> Result<&ResourceRecord, CommitIssue> {
        if self.retired_set.contains(&key) {
            return Err(CommitIssue::new(
                CommitDetail::StaleHandle,
                "resource was retired earlier in this commit",
            ));
        }
        self.edits
            .get(&key)
            .or_else(|| self.document.resource_record(key).ok())
            .ok_or_else(|| CommitIssue::new(CommitDetail::StaleHandle, "resource handle is stale"))
    }

    fn edit(&mut self, key: ResourceKey) -> Result<&mut ResourceRecord, CommitIssue> {
        if self.retired_set.contains(&key) {
            return Err(CommitIssue::new(
                CommitDetail::StaleHandle,
                "resource was retired earlier in this commit",
            ));
        }
        if !self.edits.contains_key(&key) {
            let record = self
                .document
                .resource_record(key)
                .map_err(|_| {
                    CommitIssue::new(CommitDetail::StaleHandle, "resource handle is stale")
                })?
                .clone();
            if key.kind == HandleKind::Connector {
                self.resource_ports.insert(key, record.port);
            }
            self.edits.insert(key, record);
        }
        self.edits
            .get_mut(&key)
            .ok_or_else(|| CommitIssue::new(CommitDetail::Invariant, "resource overlay lost key"))
    }

    fn owner_of(&self, key: ResourceKey) -> Result<Option<NodeKey>, CommitIssue> {
        if let Some(owner) = self.owner_changes.get(&key) {
            return Ok(*owner);
        }
        match key.kind {
            HandleKind::Port => Ok(self.document.port_owners.get(&key).copied()),
            HandleKind::Control => Ok(self.document.control_owners.get(&key).copied()),
            HandleKind::Connector => Ok(self.read(key)?.owner),
            HandleKind::Node => Err(CommitIssue::new(
                CommitDetail::WrongKind,
                "node key is not a resource",
            )),
        }
    }

    fn set_owner(&mut self, key: ResourceKey, owner: Option<NodeKey>) -> Result<bool, CommitIssue> {
        let previous = self.owner_of(key)?;
        if previous == owner {
            return Ok(false);
        }
        if owner.is_some() && previous.is_some() {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "resource already has a final owner",
            ));
        }
        self.edit(key)?.owner = owner;
        self.owner_changes.insert(key, owner);
        Ok(true)
    }

    fn retire(&mut self, key: ResourceKey) -> Result<bool, CommitIssue> {
        let record = self.read(key)?.clone();
        if self.created_set.contains(&key) {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "a provisional resource cannot be created and retired in one commit",
            ));
        }
        self.retired.try_reserve(1).map_err(|_| {
            CommitIssue::new(CommitDetail::Capacity, "resource retirement capacity")
        })?;
        self.retired_set.try_reserve(1).map_err(|_| {
            CommitIssue::new(CommitDetail::Capacity, "resource retire index capacity")
        })?;
        if self.retired_set.insert(key) {
            self.retired.push(key);
            if key.kind == HandleKind::Connector {
                self.resource_ports.insert(key, record.port);
            }
            if matches!(
                record.family,
                ResourceFamily::Port | ResourceFamily::Control
            ) {
                self.owner_changes.insert(key, None);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn is_retired(&self, key: ResourceKey) -> bool {
        self.retired_set.contains(&key)
    }

    fn detach_occurrence_resources(
        &mut self,
        attachments: super::Attachments,
    ) -> Result<(), CommitIssue> {
        if let Some(port) = attachments.port {
            self.detach_port_from_occurrence(port)?;
        }
        if let Some(control) = attachments.control {
            self.set_owner(control, None)?;
            if self.read(control)?.ownership == OwnershipMode::OccurrenceOwned {
                self.retire(control)?;
            }
        }
        Ok(())
    }

    fn detach_port_from_occurrence(&mut self, port: ResourceKey) -> Result<(), CommitIssue> {
        let record = self.read(port)?.clone();
        let connectors = self.connectors_for_port(port)?;
        if record.ownership == OwnershipMode::OccurrenceOwned {
            for connector in &connectors {
                if self.read(*connector)?.ownership == OwnershipMode::OccurrenceOwned {
                    self.retire(*connector)?;
                }
            }
            self.edit(port)?.selected = None;
            self.set_owner(port, None)?;
            self.retire(port)?;
        } else {
            self.set_owner(port, None)?;
        }
        Ok(())
    }

    fn connectors_for_port(&self, port: ResourceKey) -> Result<Vec<ResourceKey>, CommitIssue> {
        let mut keys = Vec::new();
        let indexed = self.document.connectors_by_port.get(&port);
        let created = self.created_connectors_by_port.get(&port);
        keys.try_reserve(
            indexed
                .map_or(0, HashSet::len)
                .saturating_add(created.map_or(0, HashSet::len)),
        )
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "Connector dependency capacity"))?;
        if let Some(indexed) = indexed {
            for key in indexed {
                if self.retired_set.contains(key) {
                    continue;
                }
                if let Some(record) = self.edits.get(key) {
                    if record.port == Some(port) {
                        keys.push(*key);
                    }
                } else {
                    keys.push(*key);
                }
            }
        }
        if let Some(created) = created {
            for key in created {
                if !self.retired_set.contains(key) {
                    keys.push(*key);
                }
            }
        }
        Ok(keys)
    }

    fn attach_port(
        &mut self,
        tree: &mut TreeDraft<'_>,
        node: NodeKey,
        port: Option<ResourceKey>,
    ) -> Result<bool, CommitIssue> {
        if let Some(port) = port {
            if port.kind != HandleKind::Port {
                return Err(CommitIssue::new(
                    CommitDetail::WrongKind,
                    "ATTACH_PORT requires a Port handle",
                ));
            }
            self.read(port)?;
            if let Some(owner) = self.owner_of(port)? {
                if owner != node {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "Port is attached to another occurrence",
                    ));
                }
            }
        }
        let current = tree
            .read(node)
            .map_err(|error| CommitIssue::new(CommitDetail::InvalidTopology, error.to_string()))?
            .attachments
            .port;
        if current == port {
            return Ok(false);
        }
        if let Some(current) = current {
            self.detach_port_from_occurrence(current)?;
        }
        if let Some(port) = port {
            self.set_owner(port, Some(node))?;
        }
        tree.edit(node)
            .map_err(|error| CommitIssue::new(CommitDetail::InvalidTopology, error.to_string()))?
            .attachments
            .port = port;
        Ok(true)
    }

    fn attach_control(
        &mut self,
        tree: &mut TreeDraft<'_>,
        node: NodeKey,
        control: Option<ResourceKey>,
    ) -> Result<bool, CommitIssue> {
        if let Some(control) = control {
            if control.kind != HandleKind::Control {
                return Err(CommitIssue::new(
                    CommitDetail::WrongKind,
                    "ATTACH_CONTROL requires a Control handle",
                ));
            }
            self.read(control)?;
            if let Some(owner) = self.owner_of(control)? {
                if owner != node {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "Control is attached to another occurrence",
                    ));
                }
            }
        }
        let current = tree
            .read(node)
            .map_err(|error| CommitIssue::new(CommitDetail::InvalidTopology, error.to_string()))?
            .attachments
            .control;
        if current == control {
            return Ok(false);
        }
        if let Some(current) = current {
            self.set_owner(current, None)?;
            if self.read(current)?.ownership == OwnershipMode::OccurrenceOwned {
                self.retire(current)?;
            }
        }
        if let Some(control) = control {
            self.set_owner(control, Some(node))?;
        }
        tree.edit(node)
            .map_err(|error| CommitIssue::new(CommitDetail::InvalidTopology, error.to_string()))?
            .attachments
            .control = control;
        Ok(true)
    }

    fn select_connector(
        &mut self,
        port: ResourceKey,
        connector: Option<ResourceKey>,
    ) -> Result<bool, CommitIssue> {
        if port.kind != HandleKind::Port {
            return Err(CommitIssue::new(
                CommitDetail::WrongKind,
                "SELECT_CONNECTOR requires a Port handle",
            ));
        }
        self.read(port)?;
        if let Some(connector) = connector {
            if connector.kind != HandleKind::Connector {
                return Err(CommitIssue::new(
                    CommitDetail::WrongKind,
                    "SELECT_CONNECTOR requires a Connector handle",
                ));
            }
            if self.read(connector)?.port != Some(port) {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "Connector is bound to another Port",
                ));
            }
        }
        let current = self.read(port)?.selected;
        if current == connector {
            return Ok(false);
        }
        self.edit(port)?.selected = connector;
        Ok(true)
    }

    fn dispose_port(&mut self, port: ResourceKey) -> Result<bool, CommitIssue> {
        let record = self.read(port)?;
        if record.owner.is_some() || self.owner_of(port)?.is_some() || record.selected.is_some() {
            return Err(CommitIssue::new(
                CommitDetail::InUseDisposal,
                "Port is still attached or has a selected Connector",
            ));
        }
        if self.connector_uses_port(port) {
            return Err(CommitIssue::new(
                CommitDetail::InUseDisposal,
                "Port is still referenced by a Connector",
            ));
        }
        self.retire(port)
    }

    fn dispose_connector(&mut self, connector: ResourceKey) -> Result<bool, CommitIssue> {
        if connector.kind != HandleKind::Connector {
            return Err(CommitIssue::new(
                CommitDetail::WrongKind,
                "DISPOSE_CONNECTOR requires a Connector handle",
            ));
        }
        self.read(connector)?;
        if self.port_selects_connector(connector) {
            return Err(CommitIssue::new(
                CommitDetail::InUseDisposal,
                "Connector is selected by a Port",
            ));
        }
        self.retire(connector)
    }

    fn dispose_control(&mut self, control: ResourceKey) -> Result<bool, CommitIssue> {
        if control.kind != HandleKind::Control {
            return Err(CommitIssue::new(
                CommitDetail::WrongKind,
                "DISPOSE_CONTROL requires a Control handle",
            ));
        }
        if self.owner_of(control)?.is_some() {
            return Err(CommitIssue::new(
                CommitDetail::InUseDisposal,
                "Control is still attached",
            ));
        }
        self.retire(control)
    }

    fn connector_uses_port(&self, port: ResourceKey) -> bool {
        if let Some(indexed) = self.document.connectors_by_port.get(&port) {
            for key in indexed {
                if self.retired_set.contains(key) {
                    continue;
                }
                if self
                    .edits
                    .get(key)
                    .is_none_or(|record| record.port == Some(port))
                {
                    return true;
                }
            }
        }
        self.created_connectors_by_port
            .get(&port)
            .is_some_and(|keys| keys.iter().any(|key| !self.retired_set.contains(key)))
    }

    fn port_selects_connector(&self, connector: ResourceKey) -> bool {
        if let Some(indexed) = self.document.ports_by_selected_connector.get(&connector) {
            for key in indexed {
                if self.retired_set.contains(key) {
                    continue;
                }
                if self
                    .edits
                    .get(key)
                    .is_none_or(|record| record.selected == Some(connector))
                {
                    return true;
                }
            }
        }
        self.edits.iter().any(|(key, record)| {
            key.kind == HandleKind::Port
                && !self.retired_set.contains(key)
                && !self
                    .document
                    .ports_by_selected_connector
                    .get(&connector)
                    .is_some_and(|values| values.contains(key))
                && record.selected == Some(connector)
        })
    }

    fn validate_connector_binding(&self, record: &ResourceRecord) -> Result<(), CommitIssue> {
        let port = record.port.ok_or_else(|| {
            CommitIssue::new(CommitDetail::InvalidTopology, "Connector has no bound Port")
        })?;
        if port.kind != HandleKind::Port {
            return Err(CommitIssue::new(
                CommitDetail::WrongKind,
                "Connector binding does not name a Port",
            ));
        }
        let port_record = self.read(port)?;
        if record.ownership == OwnershipMode::OccurrenceOwned
            && (port_record.ownership != OwnershipMode::OccurrenceOwned
                || self.owner_of(port)?.is_none())
        {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "occurrence-owned Connector must follow an attached occurrence-owned Port",
            ));
        }
        Ok(())
    }

    fn validate_port_selection(
        &self,
        port: ResourceKey,
        connector: ResourceKey,
    ) -> Result<(), CommitIssue> {
        let connector_record = self.read(connector)?;
        if connector_record.family != ResourceFamily::Connector
            || connector_record.port != Some(port)
        {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "Port selection does not match the Connector binding",
            ));
        }
        Ok(())
    }

    fn validate(&self, tree: &TreeDraft<'_>) -> Result<(), CommitIssue> {
        for (key, record) in &self.edits {
            if self.retired_set.contains(key) {
                continue;
            }
            if record.family == ResourceFamily::Connector {
                self.validate_connector_binding(record)?;
            }
            if record.family == ResourceFamily::Port {
                if let Some(connector) = record.selected {
                    self.validate_port_selection(*key, connector)?;
                }
            }
            if let Some(owner) = record.owner {
                let owner_record = tree.read(owner).map_err(|error| {
                    CommitIssue::new(CommitDetail::InvalidTopology, error.to_string())
                })?;
                let attached = match key.kind {
                    HandleKind::Port => owner_record.attachments.port == Some(*key),
                    HandleKind::Control => owner_record.attachments.control == Some(*key),
                    _ => true,
                };
                if !attached {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "resource owner index disagrees with occurrence attachment",
                    ));
                }
            }
        }
        for port in self
            .retired
            .iter()
            .copied()
            .filter(|key| key.kind == HandleKind::Port)
        {
            if self
                .connectors_for_port(port)?
                .into_iter()
                .any(|connector| !self.retired_set.contains(&connector))
            {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "Connector remains bound to a retired Port",
                ));
            }
        }
        for key in tree.edits.keys().copied() {
            if tree.is_retired(key) {
                continue;
            }
            let record = tree.read(key).map_err(|error| {
                CommitIssue::new(CommitDetail::InvalidTopology, error.to_string())
            })?;
            for attachment in [record.attachments.port, record.attachments.control]
                .into_iter()
                .flatten()
            {
                let resource = self.read(attachment)?;
                if self.owner_of(attachment)? != Some(key) || resource.owner != Some(key) {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "occurrence attachment disagrees with resource owner index",
                    ));
                }
            }
        }
        Ok(())
    }

    fn into_plan(mut self) -> ResourcePlan {
        self.created.sort_unstable_by(|left, right| {
            (left.kind as u32, left.slot).cmp(&(right.kind as u32, right.slot))
        });
        ResourcePlan {
            edits: self.edits,
            created: self.created,
            retired: self.retired,
            retired_set: self.retired_set,
            owner_changes: self.owner_changes,
            resource_ports: self.resource_ports,
            connector_buckets: HashMap::new(),
            connector_touched: HashSet::new(),
            selected_buckets: HashMap::new(),
            selected_touched: HashSet::new(),
        }
    }
}

struct CommitDraft<'a> {
    document: &'a OccurrenceDocument,
    local_objects: BTreeMap<u32, LocalObject>,
    tree: TreeDraft<'a>,
    resources: ResourceDraft<'a>,
    roots_added: Vec<NodeKey>,
    roots_removed: Vec<NodeKey>,
    effects: EffectMask,
    property_initials: HashMap<NodeKey, PropertyLayers>,
    style_state_initials: HashMap<(NodeKey, String), Option<String>>,
    interaction_initials: HashMap<NodeKey, (bool, u64)>,
    /// True when the accepted change can alter native output or native
    /// resources.  Subscription presence and a declared value hidden by an
    /// override intentionally do not set this bit.
    physical_work: bool,
}

impl<'a> CommitDraft<'a> {
    fn apply_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::CreateNode { .. } => self.apply_structure_operation(operation),
            UiOperation::CreateRoot { .. } => self.apply_structure_operation(operation),
            UiOperation::HistoryAction { .. } => self.apply_structure_operation(operation),
            UiOperation::InsertBefore { .. } => self.apply_structure_operation(operation),
            UiOperation::Detach { .. } => self.apply_structure_operation(operation),
            UiOperation::RetireSubtree { .. } => self.apply_structure_operation(operation),
            UiOperation::RetireRoot { .. } => self.apply_structure_operation(operation),
            UiOperation::AttachPort { .. } => self.apply_resource_operation(operation),
            UiOperation::AttachControl { .. } => self.apply_resource_operation(operation),
            UiOperation::CreatePort { .. } => self.apply_resource_operation(operation),
            UiOperation::CreateConnector { .. } => self.apply_resource_operation(operation),
            UiOperation::SelectConnector { .. } => self.apply_resource_operation(operation),
            UiOperation::DisposePort { .. } => self.apply_resource_operation(operation),
            UiOperation::DisposeConnector { .. } => self.apply_resource_operation(operation),
            UiOperation::SetLiteralFunnel { .. } => self.apply_resource_operation(operation),
            UiOperation::ReplaceLiteral { .. } => self.apply_resource_operation(operation),
            UiOperation::CreateControl { .. } => self.apply_resource_operation(operation),
            UiOperation::DisposeControl { .. } => self.apply_resource_operation(operation),
            UiOperation::SetDeclared { .. } => self.apply_property_operation(operation),
            UiOperation::ResetDeclared { .. } => self.apply_property_operation(operation),
            UiOperation::SetOverride { .. } => self.apply_property_operation(operation),
            UiOperation::ClearOverride { .. } => self.apply_property_operation(operation),
            UiOperation::SetStyleState { .. } => self.apply_style_operation(operation),
            UiOperation::ClearStyleState { .. } => self.apply_style_operation(operation),
            UiOperation::SetHidden { .. } => self.apply_interaction_operation(operation),
            UiOperation::SetSubscriptions { .. } => self.apply_interaction_operation(operation),
            UiOperation::ControlCommand { .. } => self.apply_control_operation(operation),
            UiOperation::ReplaceEditorContent { .. } => self.apply_control_operation(operation),
        }
    }

    fn apply_structure_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::CreateNode { .. } => {
                self.physical_work = true;
                self.effects = self
                    .effects
                    .union(EFFECT_STRUCTURE_GUARD)
                    .union(EFFECT_LAYOUT_INPUT);
                Ok(true)
            }

            UiOperation::CreateRoot {
                local_ordinal,
                role,
                owner,
            } => {
                let root = local_node_object(&self.local_objects, *local_ordinal)?;
                let owner = owner
                    .as_ref()
                    .map(|value| {
                        resolve_node_ref(self.document, &self.tree, &self.local_objects, value)
                    })
                    .transpose()?;
                if *role == RootRole::Portal && owner.is_none() {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "Portal roots require an owning occurrence",
                    ));
                }
                self.tree.set_root_owner(root, owner).map_err(tree_issue)?;
                self.roots_added.push(root);
                self.physical_work = true;
                self.effects = self
                    .effects
                    .union(EFFECT_STRUCTURE_GUARD)
                    .union(EFFECT_LAYOUT_INPUT);
                Ok(true)
            }

            UiOperation::HistoryAction { root, action_id } => {
                let root = resolve_node_ref(self.document, &self.tree, &self.local_objects, root)?;
                if *action_id != 1 && *action_id != 2 {
                    return Err(CommitIssue::new(
                        CommitDetail::Unsupported,
                        "History action is not supported by the current host",
                    ));
                }
                let record = self.tree.edit(root).map_err(tree_issue)?;
                if record.root_role != Some(RootRole::LegacyHistoryUnit) {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "History action requires a LegacyHistoryUnit root",
                    ));
                }
                if record.history_action == Some(*action_id) {
                    return Ok(false);
                }
                if *action_id == 2 {
                    if self.document.history_roots().last().copied() != Some(root) {
                        return Err(CommitIssue::new(
                            CommitDetail::InvalidTopology,
                            "History discard requires the live tail root",
                        ));
                    }
                    record.history_action = Some(*action_id);
                    self.retire_root(root)?;
                    return Ok(true);
                }
                record.history_action = Some(*action_id);
                self.tree.mark_changed(root);
                self.physical_work = true;
                self.effects = self.effects.union(EFFECT_STRUCTURE_GUARD);
                Ok(true)
            }

            UiOperation::InsertBefore {
                parent,
                child,
                before,
            } => {
                let parent =
                    resolve_node_ref(self.document, &self.tree, &self.local_objects, parent)?;
                let child =
                    resolve_node_ref(self.document, &self.tree, &self.local_objects, child)?;
                let before = before
                    .as_ref()
                    .map(|value| {
                        resolve_node_ref(self.document, &self.tree, &self.local_objects, value)
                    })
                    .transpose()?;
                let previous_parent = self.tree.read(child).map_err(tree_issue)?.links.parent;
                let changed = self
                    .tree
                    .insert_before(parent, child, before)
                    .map_err(tree_issue)?;
                if changed {
                    self.physical_work = true;
                    let keys = previous_parent
                        .into_iter()
                        .chain(std::iter::once(parent))
                        .chain(std::iter::once(child))
                        .chain(before);
                    mark_structure(&mut self.tree, keys, &mut self.effects)?;
                }
                Ok(changed)
            }

            UiOperation::Detach { parent, child } => {
                let parent =
                    resolve_node_ref(self.document, &self.tree, &self.local_objects, parent)?;
                let child =
                    resolve_node_ref(self.document, &self.tree, &self.local_objects, child)?;
                let changed = self.tree.detach(parent, child).map_err(tree_issue)?;
                if changed {
                    self.physical_work = true;
                    mark_structure(&mut self.tree, [parent, child], &mut self.effects)?;
                }
                Ok(changed)
            }

            UiOperation::RetireSubtree { root } => {
                let root = resolve_node_ref(self.document, &self.tree, &self.local_objects, root)?;
                if self
                    .tree
                    .read(root)
                    .map_err(tree_issue)?
                    .root_role
                    .is_some()
                {
                    return Err(tree_issue(TreeError::ProtectedRoot(root)));
                }
                let members = self.tree.retirement_keys(root).map_err(tree_issue)?;
                let attachments = members
                    .iter()
                    .map(|key| self.tree.read(*key).map(|record| record.attachments))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(tree_issue)?;
                let parent = self.tree.read(root).map_err(tree_issue)?.links.parent;
                self.tree
                    .retire_subtree_with_keys(root, &members)
                    .map_err(tree_issue)?;
                self.physical_work = true;
                for attachment in attachments {
                    self.resources.detach_occurrence_resources(attachment)?;
                }
                self.roots_removed
                    .extend(members.iter().copied().filter(|member| {
                        self.tree
                            .read_any(*member)
                            .is_ok_and(|record| record.root_role.is_some())
                    }));
                if let Some(parent) = parent {
                    mark_structure(&mut self.tree, [parent], &mut self.effects)?;
                }
                Ok(true)
            }

            UiOperation::RetireRoot { root } => {
                let root = resolve_node_ref(self.document, &self.tree, &self.local_objects, root)?;
                let root_role = self.tree.read(root).map_err(tree_issue)?.root_role;
                if root_role.is_none() || root_role == Some(RootRole::Body) {
                    return Err(tree_issue(TreeError::ProtectedRoot(root)));
                }
                self.retire_root(root)
            }

            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }

    fn retire_root(&mut self, root: NodeKey) -> Result<bool, CommitIssue> {
        let members = self.tree.retirement_keys(root).map_err(tree_issue)?;
        let attachments = members
            .iter()
            .map(|key| self.tree.read(*key).map(|record| record.attachments))
            .collect::<Result<Vec<_>, _>>()
            .map_err(tree_issue)?;
        let parent = self.tree.read(root).map_err(tree_issue)?.links.parent;
        self.tree
            .retire_subtree_with_keys(root, &members)
            .map_err(tree_issue)?;
        self.physical_work = true;
        for attachment in attachments {
            self.resources.detach_occurrence_resources(attachment)?;
        }
        self.roots_removed
            .extend(members.iter().copied().filter(|member| {
                self.tree
                    .read_any(*member)
                    .is_ok_and(|record| record.root_role.is_some())
            }));
        if let Some(parent) = parent {
            mark_structure(&mut self.tree, [parent], &mut self.effects)?;
        }
        Ok(true)
    }

    fn apply_resource_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::AttachPort { node, port } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let port = port
                    .as_ref()
                    .map(|value| {
                        resolve_resource_ref(
                            self.document,
                            &self.resources,
                            &self.local_objects,
                            value,
                            ResourceFamily::Port,
                        )
                    })
                    .transpose()?;
                let changed = self.resources.attach_port(&mut self.tree, node, port)?;
                if changed {
                    self.physical_work = true;
                    mark_interaction(&mut self.tree, node, &mut self.effects)?;
                }
                Ok(changed)
            }

            UiOperation::AttachControl { node, control } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let control = control
                    .as_ref()
                    .map(|value| {
                        resolve_resource_ref(
                            self.document,
                            &self.resources,
                            &self.local_objects,
                            value,
                            ResourceFamily::Control,
                        )
                    })
                    .transpose()?;
                let changed = self
                    .resources
                    .attach_control(&mut self.tree, node, control)?;
                if changed {
                    self.physical_work = true;
                    mark_interaction(&mut self.tree, node, &mut self.effects)?;
                }
                Ok(changed)
            }

            UiOperation::CreatePort {
                local_ordinal,
                owner,
                ..
            } => {
                let port =
                    local_resource_object(&self.local_objects, *local_ordinal, HandleKind::Port)?;
                if let Some(owner) = owner {
                    let owner =
                        resolve_node_ref(self.document, &self.tree, &self.local_objects, owner)?;
                    self.resources
                        .attach_port(&mut self.tree, owner, Some(port))?;
                    mark_interaction(&mut self.tree, owner, &mut self.effects)?;
                }
                self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                self.physical_work = true;
                Ok(true)
            }

            UiOperation::CreateConnector {
                local_ordinal,
                port,
                ..
            } => {
                let connector = local_resource_object(
                    &self.local_objects,
                    *local_ordinal,
                    HandleKind::Connector,
                )?;
                let port = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    port,
                    ResourceFamily::Port,
                )?;
                self.resources.edit(connector)?.port = Some(port);
                self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                self.physical_work = true;
                Ok(true)
            }

            UiOperation::SelectConnector { port, connector } => {
                let port = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    port,
                    ResourceFamily::Port,
                )?;
                let connector = connector
                    .as_ref()
                    .map(|value| {
                        resolve_resource_ref(
                            self.document,
                            &self.resources,
                            &self.local_objects,
                            value,
                            ResourceFamily::Connector,
                        )
                    })
                    .transpose()?;
                let changed = self.resources.select_connector(port, connector)?;
                if changed {
                    self.physical_work = true;
                    self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                }
                Ok(changed)
            }

            UiOperation::DisposePort { port } => {
                let port = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    port,
                    ResourceFamily::Port,
                )?;
                let changed = self.resources.dispose_port(port)?;
                if changed {
                    self.physical_work = true;
                    self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                }
                Ok(changed)
            }

            UiOperation::DisposeConnector { connector } => {
                let connector = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    connector,
                    ResourceFamily::Connector,
                )?;
                let changed = self.resources.dispose_connector(connector)?;
                if changed {
                    self.physical_work = true;
                    self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                }
                Ok(changed)
            }

            UiOperation::SetLiteralFunnel { port, .. } => {
                let port = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    port,
                    ResourceFamily::Port,
                )?;
                let record = self.resources.read(port)?;
                if record.ownership != OwnershipMode::OccurrenceOwned {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "literal Funnel requires an occurrence-owned Port",
                    ));
                }
                self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                self.physical_work = true;
                Ok(true)
            }

            UiOperation::ReplaceLiteral {
                port,
                content_format,
                content,
                annotations,
            } => {
                let port = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    port,
                    ResourceFamily::Port,
                )?;
                let record = self.resources.read(port)?;
                if record.ownership != OwnershipMode::OccurrenceOwned {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "literal replacement requires an occurrence-owned Port",
                    ));
                }
                if *content_format != 1 {
                    return Err(CommitIssue::new(
                        CommitDetail::Unsupported,
                        "literal content format is unsupported",
                    ));
                }
                std::str::from_utf8(content).map_err(|_| {
                    CommitIssue::new(CommitDetail::Malformed, "literal content is not UTF-8")
                })?;
                let _ = annotations;
                self.effects = self.effects.union(EFFECT_CONTENT_PROJECTION);
                self.physical_work = true;
                Ok(true)
            }

            UiOperation::CreateControl {
                local_ordinal,
                owner,
                ..
            } => {
                let control = local_resource_object(
                    &self.local_objects,
                    *local_ordinal,
                    HandleKind::Control,
                )?;
                if let Some(owner) = owner {
                    let owner =
                        resolve_node_ref(self.document, &self.tree, &self.local_objects, owner)?;
                    self.resources
                        .attach_control(&mut self.tree, owner, Some(control))?;
                    mark_interaction(&mut self.tree, owner, &mut self.effects)?;
                }
                self.effects = self.effects.union(EFFECT_INTERACTION_RUNTIME);
                self.physical_work = true;
                Ok(true)
            }

            UiOperation::DisposeControl { control } => {
                let control = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    control,
                    ResourceFamily::Control,
                )?;
                let changed = self.resources.dispose_control(control)?;
                if changed {
                    self.physical_work = true;
                    self.effects = self.effects.union(EFFECT_INTERACTION_RUNTIME);
                }
                Ok(changed)
            }

            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }

    fn apply_property_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::SetDeclared {
                node,
                property,
                value,
            } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let kind = self.tree.read(node).map_err(tree_issue)?.kind;
                capture_property_initial(&self.tree, node, &mut self.property_initials)?;
                let change = self
                    .tree
                    .edit(node)
                    .map_err(tree_issue)?
                    .properties
                    .apply(PropertyLayer::Declared, kind, *property, value.clone())
                    .map_err(property_issue)?;
                let _ = change;
                Ok(false)
            }

            UiOperation::ResetDeclared { node, property } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let kind = self.tree.read(node).map_err(tree_issue)?.kind;
                capture_property_initial(&self.tree, node, &mut self.property_initials)?;
                let change = self
                    .tree
                    .edit(node)
                    .map_err(tree_issue)?
                    .properties
                    .reset_declared(kind, *property)
                    .map_err(property_issue)?;
                let _ = change;
                Ok(false)
            }

            UiOperation::SetOverride {
                node,
                property,
                value,
            } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let kind = self.tree.read(node).map_err(tree_issue)?.kind;
                capture_property_initial(&self.tree, node, &mut self.property_initials)?;
                let change = self
                    .tree
                    .edit(node)
                    .map_err(tree_issue)?
                    .properties
                    .apply(PropertyLayer::Override, kind, *property, value.clone())
                    .map_err(property_issue)?;
                let _ = change;
                Ok(false)
            }

            UiOperation::ClearOverride { node, property } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let kind = self.tree.read(node).map_err(tree_issue)?.kind;
                capture_property_initial(&self.tree, node, &mut self.property_initials)?;
                let change = self
                    .tree
                    .edit(node)
                    .map_err(tree_issue)?
                    .properties
                    .clear_override(kind, *property)
                    .map_err(property_issue)?;
                let _ = change;
                Ok(false)
            }

            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }

    fn apply_style_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::SetStyleState {
                node,
                layer,
                key,
                value,
            } => {
                if *layer > 1
                    || key.is_empty()
                    || key.contains('\0')
                    || value.is_empty()
                    || value.contains('\0')
                {
                    return Err(CommitIssue::new(
                        CommitDetail::Malformed,
                        "style state layer/value is invalid",
                    ));
                }
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                if *layer == 0 {
                    capture_style_state_initial(
                        &self.tree,
                        node,
                        key,
                        &mut self.style_state_initials,
                    )?;
                }
                let record = self.tree.edit(node).map_err(tree_issue)?;
                let states = if *layer == 0 {
                    &mut record.style_states
                } else {
                    &mut record.style_overrides
                };
                if states.get(key) == Some(value) {
                    return Ok(false);
                }
                states.insert(key.clone(), value.clone());
                if *layer == 0 {
                    return Ok(false);
                }
                mark_style_state(&mut self.tree, node, &mut self.effects)?;
                Ok(true)
            }

            UiOperation::ClearStyleState { node, layer, key } => {
                if *layer > 1 || key.is_empty() || key.contains('\0') {
                    return Err(CommitIssue::new(
                        CommitDetail::Malformed,
                        "style state layer/key is invalid",
                    ));
                }
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                if *layer == 0 {
                    capture_style_state_initial(
                        &self.tree,
                        node,
                        key,
                        &mut self.style_state_initials,
                    )?;
                }
                let record = self.tree.edit(node).map_err(tree_issue)?;
                let states = if *layer == 0 {
                    &mut record.style_states
                } else {
                    &mut record.style_overrides
                };
                if states.remove(key).is_none() {
                    return Ok(false);
                }
                if *layer == 0 {
                    return Ok(false);
                }
                mark_style_state(&mut self.tree, node, &mut self.effects)?;
                Ok(true)
            }

            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }

    fn apply_interaction_operation(
        &mut self,
        operation: &UiOperation,
    ) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::SetHidden { node, hidden } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                capture_interaction_initial(&self.tree, node, &mut self.interaction_initials)?;
                let record = self.tree.edit(node).map_err(tree_issue)?;
                if record.renderer_hidden == *hidden {
                    return Ok(false);
                }
                record.renderer_hidden = *hidden;
                Ok(false)
            }

            UiOperation::SetSubscriptions {
                node,
                mask_low,
                mask_high,
            } => {
                let node = resolve_node_ref(self.document, &self.tree, &self.local_objects, node)?;
                let mask = u64::from(*mask_low) | (u64::from(*mask_high) << 32);
                capture_interaction_initial(&self.tree, node, &mut self.interaction_initials)?;
                let record = self.tree.edit(node).map_err(tree_issue)?;
                if record.subscriptions == mask {
                    return Ok(false);
                }
                record.subscriptions = mask;
                Ok(false)
            }

            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }

    fn apply_control_operation(&mut self, operation: &UiOperation) -> Result<bool, CommitIssue> {
        match operation {
            UiOperation::ControlCommand {
                control,
                command_id,
                operands,
            } => {
                let _ = command_id;
                let _ = operands;
                let _ = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    control,
                    ResourceFamily::Control,
                )?;
                self.effects = self.effects.union(EFFECT_INTERACTION_RUNTIME);
                Ok(true)
            }

            UiOperation::ReplaceEditorContent {
                control,
                content,
                expected_edit_revision: _,
            } => {
                let _ = resolve_resource_ref(
                    self.document,
                    &self.resources,
                    &self.local_objects,
                    control,
                    ResourceFamily::Control,
                )?;
                std::str::from_utf8(content).map_err(|_| {
                    CommitIssue::new(CommitDetail::Malformed, "editor content is not UTF-8")
                })?;
                self.effects = self.effects.union(EFFECT_INTERACTION_RUNTIME);
                Ok(true)
            }
            _ => unreachable!("operation dispatched to the wrong draft phase"),
        }
    }
}

struct FinalizedDraft {
    local_objects: BTreeMap<u32, LocalObject>,
    tree: TreePlan,
    resources: ResourcePlan,
    roots_added: Vec<NodeKey>,
    roots_removed: Vec<NodeKey>,
    history_roots: Vec<HistoryRootSummary>,
    effects: EffectMask,
    physical_work: bool,
    changed: bool,
}

impl<'a> CommitDraft<'a> {
    fn finalize(mut self, mut changed: bool) -> Result<FinalizedDraft, CommitIssue> {
        let (property_changed, property_physical) = finalize_property_changes(
            &mut self.tree,
            std::mem::take(&mut self.property_initials),
            &mut self.effects,
        )?;
        changed |= property_changed;
        self.physical_work |= property_physical;
        let style_changed = finalize_style_state_changes(
            &mut self.tree,
            std::mem::take(&mut self.style_state_initials),
            &mut self.effects,
        )?;
        changed |= style_changed;
        self.physical_work |= style_changed;
        let (interaction_changed, interaction_physical, membership_nodes) =
            finalize_interaction_changes(
                &mut self.tree,
                std::mem::take(&mut self.interaction_initials),
                &mut self.effects,
            )?;
        self.tree.membership_nodes.extend(membership_nodes);
        changed |= interaction_changed;
        self.physical_work |= interaction_physical;
        self.tree.validate().map_err(tree_issue)?;
        validate_root_roles(self.document, &self.tree)?;
        let history_roots = history_root_summaries(&self.tree)?;
        self.resources.validate(&self.tree)?;
        for object in self.local_objects.values() {
            match object {
                LocalObject::Node(key) if self.tree.is_retired(*key) => {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "a local occurrence cannot be created and retired in one commit",
                    ));
                }
                LocalObject::Resource(key) if self.resources.is_retired(*key) => {
                    return Err(CommitIssue::new(
                        CommitDetail::InvalidTopology,
                        "a local resource cannot be created and disposed in one commit",
                    ));
                }
                _ => {}
            }
        }
        Ok(FinalizedDraft {
            local_objects: self.local_objects,
            tree: self.tree.into_plan(),
            resources: self.resources.into_plan(),
            roots_added: self.roots_added,
            roots_removed: self.roots_removed,
            history_roots,
            effects: self.effects,
            physical_work: self.physical_work,
            changed,
        })
    }
}

fn rejection_at_revision(revision: u64, index: Option<usize>, issue: CommitIssue) -> UiRejection {
    let failed_record = index
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(FAILED_RECORD_NONE);
    let result = UiOperationResult::rejected(revision, failed_record, issue.detail);
    UiRejection {
        result,
        detail: issue.detail,
        message: issue.message,
    }
}

fn collect_local_creations(
    operations: &[UiOperation],
) -> Result<(BTreeMap<u32, LocalFamily>, usize, usize, usize, usize), (usize, CommitIssue)> {
    let mut local_specs = BTreeMap::new();
    let mut counts = [0usize; 4];
    for (index, operation) in operations.iter().enumerate() {
        let Some((ordinal, family)) = local_creation(operation) else {
            continue;
        };
        if ordinal == 0 {
            return Err((
                index,
                CommitIssue::new(
                    CommitDetail::Malformed,
                    "local creation ordinals start at one",
                ),
            ));
        }
        if local_specs.insert(ordinal, family).is_some() {
            return Err((
                index,
                CommitIssue::new(
                    CommitDetail::Malformed,
                    "local creation ordinal is declared more than once",
                ),
            ));
        }
        counts[local_family_index(family)] += 1;
    }
    for (position, ordinal) in local_specs.keys().enumerate() {
        let expected = u32::try_from(position + 1).map_err(|_| {
            (
                operations.len().saturating_sub(1),
                CommitIssue::new(
                    CommitDetail::Malformed,
                    "local creation ordinal count exceeds the wire range",
                ),
            )
        })?;
        if *ordinal != expected {
            let index = operations
                .iter()
                .position(|operation| {
                    local_creation(operation).is_some_and(|(candidate, _)| candidate == *ordinal)
                })
                .unwrap_or_else(|| operations.len().saturating_sub(1));
            return Err((
                index,
                CommitIssue::new(
                    CommitDetail::Malformed,
                    "local creation ordinals must be dense starting at one",
                ),
            ));
        }
    }
    Ok((local_specs, counts[0], counts[1], counts[2], counts[3]))
}

fn local_node_object(
    local_objects: &BTreeMap<u32, LocalObject>,
    ordinal: u32,
) -> Result<NodeKey, CommitIssue> {
    match local_objects.get(&ordinal) {
        Some(LocalObject::Node(key)) => Ok(*key),
        Some(LocalObject::Resource(_)) => Err(CommitIssue::new(
            CommitDetail::WrongKind,
            "local reference does not name an occurrence",
        )),
        None => Err(CommitIssue::new(
            CommitDetail::Malformed,
            "local occurrence ordinal was not declared",
        )),
    }
}

fn local_resource_object(
    local_objects: &BTreeMap<u32, LocalObject>,
    ordinal: u32,
    expected_kind: HandleKind,
) -> Result<ResourceKey, CommitIssue> {
    match local_objects.get(&ordinal) {
        Some(LocalObject::Resource(key)) if key.kind == expected_kind => Ok(*key),
        Some(LocalObject::Resource(_)) => Err(CommitIssue::new(
            CommitDetail::WrongKind,
            "local resource ordinal has the wrong kind",
        )),
        Some(LocalObject::Node(_)) => Err(CommitIssue::new(
            CommitDetail::WrongKind,
            "local reference does not name a resource",
        )),
        None => Err(CommitIssue::new(
            CommitDetail::Malformed,
            "local resource ordinal was not declared",
        )),
    }
}

fn provisional_resource_key(
    reference: &ResourceRef,
    local_objects: &BTreeMap<u32, LocalObject>,
    document: &OccurrenceDocument,
) -> Result<ResourceKey, CommitIssue> {
    match reference {
        ResourceRef::Existing(handle) => document
            .resource_key(*handle, ResourceFamily::Port)
            .map_err(|error| handle_issue(error, "Port reference")),
        ResourceRef::Local(ordinal) => {
            local_resource_object(local_objects, *ordinal, HandleKind::Port)
        }
    }
}

fn resolve_node_ref(
    document: &OccurrenceDocument,
    tree: &TreeDraft<'_>,
    local_objects: &BTreeMap<u32, LocalObject>,
    reference: &NodeRef,
) -> Result<NodeKey, CommitIssue> {
    let key = match reference {
        NodeRef::Existing(handle) => document
            .node_key(*handle)
            .map_err(|error| handle_issue(error, "occurrence reference"))?,
        NodeRef::Local(ordinal) => local_node_object(local_objects, *ordinal)?,
    };
    tree.read(key).map(|_| key).map_err(tree_issue)
}

fn resolve_resource_ref(
    document: &OccurrenceDocument,
    resources: &ResourceDraft<'_>,
    local_objects: &BTreeMap<u32, LocalObject>,
    reference: &ResourceRef,
    family: ResourceFamily,
) -> Result<ResourceKey, CommitIssue> {
    let key = match reference {
        ResourceRef::Existing(handle) => document
            .resource_key(*handle, family)
            .map_err(|error| handle_issue(error, "resource reference"))?,
        ResourceRef::Local(ordinal) => {
            local_resource_object(local_objects, *ordinal, family.handle_kind())?
        }
    };
    resources.read(key).map(|_| key)
}

fn handle_issue(error: super::HandleError, context: &str) -> CommitIssue {
    let detail = match error {
        super::HandleError::WrongHost => CommitDetail::WrongHost,
        super::HandleError::WrongKind => CommitDetail::WrongKind,
        super::HandleError::Invalid | super::HandleError::Stale => CommitDetail::StaleHandle,
    };
    CommitIssue::new(detail, format!("{context} is invalid"))
}

fn tree_issue(error: TreeError) -> CommitIssue {
    let detail = if matches!(error, TreeError::Capacity) {
        CommitDetail::Capacity
    } else {
        CommitDetail::InvalidTopology
    };
    CommitIssue::new(detail, error.to_string())
}

fn property_issue(error: super::PropertyError) -> CommitIssue {
    CommitIssue::new(CommitDetail::InvalidProperty, error.to_string())
}

fn mark_structure<I>(
    tree: &mut TreeDraft<'_>,
    keys: I,
    effects: &mut EffectMask,
) -> Result<(), CommitIssue>
where
    I: IntoIterator<Item = NodeKey>,
{
    let mut seen = HashSet::new();
    for key in keys {
        if !seen.insert(key) {
            continue;
        }
        let record = tree.edit(key).map_err(tree_issue)?;
        record.revisions.structure =
            record.revisions.structure.checked_add(1).ok_or_else(|| {
                CommitIssue::new(CommitDetail::Invariant, "structure revision exhausted")
            })?;
        record.dirty.effects = record
            .dirty
            .effects
            .union(EFFECT_STRUCTURE_GUARD)
            .union(EFFECT_LAYOUT_INPUT);
    }
    *effects = effects
        .union(EFFECT_STRUCTURE_GUARD)
        .union(EFFECT_LAYOUT_INPUT);
    Ok(())
}

fn capture_property_initial(
    tree: &TreeDraft<'_>,
    node: NodeKey,
    initials: &mut HashMap<NodeKey, PropertyLayers>,
) -> Result<(), CommitIssue> {
    if initials.contains_key(&node) {
        return Ok(());
    }
    let properties = tree.read(node).map_err(tree_issue)?.properties.clone();
    initials.insert(node, properties);
    Ok(())
}

fn capture_style_state_initial(
    tree: &TreeDraft<'_>,
    node: NodeKey,
    key: &str,
    initials: &mut HashMap<(NodeKey, String), Option<String>>,
) -> Result<(), CommitIssue> {
    let entry = (node, key.to_owned());
    if initials.contains_key(&entry) {
        return Ok(());
    }
    let value = tree
        .read(node)
        .map_err(tree_issue)?
        .style_states
        .get(key)
        .cloned();
    initials.insert(entry, value);
    Ok(())
}

fn capture_interaction_initial(
    tree: &TreeDraft<'_>,
    node: NodeKey,
    initials: &mut HashMap<NodeKey, (bool, u64)>,
) -> Result<(), CommitIssue> {
    if initials.contains_key(&node) {
        return Ok(());
    }
    let record = tree.read(node).map_err(tree_issue)?;
    initials.insert(node, (record.renderer_hidden, record.subscriptions));
    Ok(())
}

fn finalize_property_changes(
    tree: &mut TreeDraft<'_>,
    initials: HashMap<NodeKey, PropertyLayers>,
    effects: &mut EffectMask,
) -> Result<(bool, bool), CommitIssue> {
    let mut changed = false;
    let mut physical = false;
    let mut domain_changes: HashMap<NodeKey, (bool, bool)> = HashMap::new();
    let mut effective_effects: HashMap<NodeKey, EffectMask> = HashMap::new();
    for (node, before) in initials {
        if tree.is_retired(node) {
            continue;
        }
        let after = tree.read(node).map_err(tree_issue)?.properties.clone();
        for property in PropertyId::ALL {
            let layer_changed = before.declared(*property) != after.declared(*property)
                || before.override_value(*property) != after.override_value(*property);
            if !layer_changed {
                continue;
            }
            changed = true;
            // A masked declaration still advances the accepted UI revision.
            // Retain the node in the native frontier so a later effective
            // reveal can reuse this coalesced dependency instead of
            // rediscovering the write from the whole document.
            tree.mark_changed(node);
            let descriptor = property_descriptor(*property);
            let domains = domain_changes.entry(node).or_default();
            if descriptor.domain == "geometry" {
                domains.0 = true;
            } else {
                domains.1 = true;
            }
            if before.effective(*property) != after.effective(*property) {
                physical = true;
                tree.mark_changed(node);
                let entry = effective_effects.entry(node).or_default();
                *entry = entry.union(descriptor.effects);
            }
        }
    }
    for (node, (geometry, presentation)) in domain_changes {
        let record = tree.edit(node).map_err(tree_issue)?;
        if geometry {
            record.revisions.geometry =
                record.revisions.geometry.checked_add(1).ok_or_else(|| {
                    CommitIssue::new(CommitDetail::Invariant, "geometry revision exhausted")
                })?;
        }
        if presentation {
            record.revisions.presentation = record
                .revisions
                .presentation
                .checked_add(1)
                .ok_or_else(|| {
                    CommitIssue::new(CommitDetail::Invariant, "presentation revision exhausted")
                })?;
        }
        if let Some(node_effects) = effective_effects.get(&node).copied() {
            record.dirty.effects = record.dirty.effects.union(node_effects);
            *effects = effects.union(node_effects);
        }
    }
    Ok((changed, physical))
}

fn finalize_style_state_changes(
    tree: &mut TreeDraft<'_>,
    initials: HashMap<(NodeKey, String), Option<String>>,
    effects: &mut EffectMask,
) -> Result<bool, CommitIssue> {
    let mut changed_nodes = HashSet::new();
    for ((node, key), initial) in initials {
        if tree.is_retired(node) {
            continue;
        }
        let current = tree
            .read(node)
            .map_err(tree_issue)?
            .style_states
            .get(&key)
            .cloned();
        if current != initial {
            changed_nodes.insert(node);
            tree.mark_changed(node);
        }
    }
    for node in changed_nodes.iter().copied() {
        mark_style_state(tree, node, effects)?;
    }
    Ok(!changed_nodes.is_empty())
}

fn finalize_interaction_changes(
    tree: &mut TreeDraft<'_>,
    initials: HashMap<NodeKey, (bool, u64)>,
    effects: &mut EffectMask,
) -> Result<(bool, bool, HashSet<NodeKey>), CommitIssue> {
    let mut changed_nodes = HashSet::new();
    let mut membership_nodes = HashSet::new();
    let mut physical = false;
    for (node, (hidden, subscriptions)) in initials {
        if tree.is_retired(node) {
            continue;
        }
        let (hidden_changed, subscriptions_changed) = {
            let record = tree.read(node).map_err(tree_issue)?;
            (
                record.renderer_hidden != hidden,
                record.subscriptions != subscriptions,
            )
        };
        if hidden_changed || subscriptions_changed {
            changed_nodes.insert(node);
            tree.mark_changed(node);
            physical |= hidden_changed;
            if hidden_changed {
                membership_nodes.insert(node);
            }
        }
    }
    for node in changed_nodes.iter().copied() {
        mark_interaction(tree, node, effects)?;
    }
    Ok((!changed_nodes.is_empty(), physical, membership_nodes))
}

fn mark_style_state(
    tree: &mut TreeDraft<'_>,
    node: NodeKey,
    effects: &mut EffectMask,
) -> Result<(), CommitIssue> {
    tree.mark_changed(node);
    let record = tree.edit(node).map_err(tree_issue)?;
    record.revisions.presentation =
        record
            .revisions
            .presentation
            .checked_add(1)
            .ok_or_else(|| {
                CommitIssue::new(CommitDetail::Invariant, "presentation revision exhausted")
            })?;
    record.dirty.effects = record.dirty.effects.union(EFFECT_PRESENTATION);
    *effects = effects.union(EFFECT_PRESENTATION);
    Ok(())
}

fn mark_interaction(
    tree: &mut TreeDraft<'_>,
    node: NodeKey,
    effects: &mut EffectMask,
) -> Result<(), CommitIssue> {
    tree.mark_changed(node);
    let record = tree.edit(node).map_err(tree_issue)?;
    record.revisions.interaction =
        record.revisions.interaction.checked_add(1).ok_or_else(|| {
            CommitIssue::new(CommitDetail::Invariant, "interaction revision exhausted")
        })?;
    record.dirty.effects = record.dirty.effects.union(EFFECT_INTERACTION_RUNTIME);
    *effects = effects.union(EFFECT_INTERACTION_RUNTIME);
    Ok(())
}

fn validate_root_roles(
    document: &OccurrenceDocument,
    tree: &TreeDraft<'_>,
) -> Result<(), CommitIssue> {
    let mut candidates = tree
        .edits
        .keys()
        .copied()
        .filter(|key| !tree.is_retired(*key))
        .collect::<HashSet<_>>();
    let affected_owners = tree
        .edits
        .keys()
        .copied()
        .chain(tree.retired.iter().copied())
        .collect::<Vec<_>>();
    for owner in affected_owners {
        candidates.extend(tree.portal_dependents(owner));
    }

    for key in candidates {
        if tree.is_retired(key) {
            continue;
        }
        let record = tree.read(key).map_err(tree_issue)?;
        let Some(role) = record.root_role else {
            continue;
        };
        if role == RootRole::Body && key != document.body_root {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "only the host body may use the Body root role",
            ));
        }
        if role == RootRole::Portal && record.root_owner.is_none() {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "Portal roots require an owner",
            ));
        }
        let mut cursor = record.root_owner;
        let mut seen = HashSet::new();
        while let Some(owner) = cursor {
            if owner == key || !seen.insert(owner) {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "root ownership graph contains a cycle",
                ));
            }
            let owner_record = tree.read(owner).map_err(tree_issue)?;
            cursor = owner_record.root_owner.or(owner_record.links.parent);
        }
    }
    Ok(())
}

fn history_root_summaries(tree: &TreeDraft<'_>) -> Result<Vec<HistoryRootSummary>, CommitIssue> {
    // Descendant edits are already marked by the draft's changed frontier.
    // Walk only those keys to their owning root; an ordinary leaf update must
    // not rescan every child of every History unit just to rediscover which
    // unit changed.  A final-content scan is retained below only for an
    // actual Freeze action, where the whole final subtree is the contract.
    let mut root_set = HashSet::new();
    for key in tree.changed_keys() {
        if tree.is_retired(key) {
            continue;
        }
        let mut cursor = Some(key);
        let mut seen = HashSet::new();
        while let Some(current) = cursor {
            if !seen.insert(current) {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "History root ownership links contain a cycle",
                ));
            }
            let record = tree.read(current).map_err(tree_issue)?;
            if record.root_role == Some(RootRole::LegacyHistoryUnit) {
                root_set.insert(current);
                break;
            }
            cursor = record.root_owner.or(record.links.parent);
        }
    }
    let mut roots = root_set.into_iter().collect::<Vec<_>>();
    roots.sort_unstable_by_key(|key| (key.slot, key.generation));
    let mut summaries = Vec::with_capacity(roots.len());
    for root in roots {
        let record = tree.read(root).map_err(tree_issue)?;
        let has_live_control = if record.history_action == Some(1) {
            // Freeze is the one operation whose validity depends on the
            // complete final subtree, including unchanged descendants.
            history_subtree_has_control(tree, root)?
        } else {
            false
        };
        if record.history_action == Some(1) && has_live_control {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "History freeze final content cannot contain a live control",
            ));
        }
        summaries.push(HistoryRootSummary {
            root,
            action: record.history_action,
            has_live_control,
        });
    }
    Ok(summaries)
}

fn history_subtree_has_control(tree: &TreeDraft<'_>, root: NodeKey) -> Result<bool, CommitIssue> {
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert(node) {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "History root child links contain a cycle",
            ));
        }
        let record = tree.read(node).map_err(tree_issue)?;
        if record.attachments.control.is_some() {
            return Ok(true);
        }
        let mut child = record.links.first_child;
        while let Some(child_key) = child {
            if tree.is_retired(child_key) {
                child = tree
                    .read_any(child_key)
                    .map_err(tree_issue)?
                    .links
                    .next_sibling;
                continue;
            }
            stack.push(child_key);
            child = tree.read(child_key).map_err(tree_issue)?.links.next_sibling;
        }
    }
    Ok(false)
}

fn apply_tree_plan(
    arena: &mut super::Arena<Occurrence>,
    portals_by_owner: &mut HashMap<NodeKey, HashSet<NodeKey>>,
    plan: TreePlan,
) {
    let TreePlan {
        mut edits,
        created,
        retired,
        retired_set,
        changed_nodes: _,
        membership_nodes: _,
        portal_buckets,
        portal_touched,
    } = plan;
    for (owner, portals) in portal_buckets {
        assert!(
            portals_by_owner.insert(owner, portals).is_none(),
            "prepared portal reverse-index bucket already exists"
        );
    }
    for key in created {
        let value = edits
            .remove(&key)
            .expect("every reserved occurrence has a prepared record");
        if value.root_role == Some(RootRole::Portal) {
            if let Some(owner) = value.root_owner {
                portals_by_owner
                    .get_mut(&owner)
                    .expect("prepared portal reverse-index reservation")
                    .insert(key);
            }
        }
        arena.insert_reserved_node(key, value);
    }
    for (key, value) in edits {
        if retired_set.contains(&key) {
            continue;
        }
        let previous = arena
            .get(key.slot, key.generation)
            .expect("prepared occurrence edit still addresses a live slot");
        if previous.root_role == Some(RootRole::Portal) && previous.root_owner != value.root_owner {
            if let Some(owner) = previous.root_owner {
                remove_portal_owner(portals_by_owner, owner, key, &portal_touched);
            }
            if let Some(owner) = value.root_owner {
                portals_by_owner
                    .get_mut(&owner)
                    .expect("prepared portal reverse-index reservation")
                    .insert(key);
            }
        }
        *arena
            .get_mut(key.slot, key.generation)
            .expect("prepared occurrence edit still addresses a live slot") = value;
    }
    for key in retired {
        let value = arena
            .get(key.slot, key.generation)
            .expect("prepared occurrence retirement still addresses a live slot");
        if value.root_role == Some(RootRole::Portal) {
            if let Some(owner) = value.root_owner {
                remove_portal_owner(portals_by_owner, owner, key, &portal_touched);
            }
        }
        let _ = arena
            .remove(key.slot, key.generation)
            .expect("prepared occurrence retirement still addresses a live slot");
    }
    cleanup_reverse_buckets(portals_by_owner, &portal_touched);
}

fn remove_portal_owner(
    portals_by_owner: &mut HashMap<NodeKey, HashSet<NodeKey>>,
    owner: NodeKey,
    portal: NodeKey,
    touched: &HashSet<NodeKey>,
) {
    if let Some(portals) = portals_by_owner.get_mut(&owner) {
        portals.remove(&portal);
        if portals.is_empty() && !touched.contains(&owner) {
            portals_by_owner.remove(&owner);
        }
    }
}

fn apply_resource_plan(
    ports: &mut super::Arena<ResourceRecord>,
    connectors: &mut super::Arena<ResourceRecord>,
    controls: &mut super::Arena<ResourceRecord>,
    port_owners: &mut HashMap<ResourceKey, NodeKey>,
    control_owners: &mut HashMap<ResourceKey, NodeKey>,
    connectors_by_port: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    ports_by_selected_connector: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    plan: ResourcePlan,
) {
    let ResourcePlan {
        mut edits,
        created,
        retired,
        retired_set,
        owner_changes,
        resource_ports: _,
        connector_buckets,
        connector_touched,
        selected_buckets,
        selected_touched,
    } = plan;
    for (port, connectors) in connector_buckets {
        assert!(
            connectors_by_port.insert(port, connectors).is_none(),
            "prepared Connector reverse-index bucket already exists"
        );
    }
    for (connector, ports) in selected_buckets {
        assert!(
            ports_by_selected_connector
                .insert(connector, ports)
                .is_none(),
            "prepared selection reverse-index bucket already exists"
        );
    }
    for key in created {
        let value = edits
            .remove(&key)
            .expect("every reserved resource has a prepared record");
        index_resource_add(key, &value, connectors_by_port, ports_by_selected_connector);
        match key.kind {
            HandleKind::Port => ports.insert_reserved_resource(key, value),
            HandleKind::Connector => connectors.insert_reserved_resource(key, value),
            HandleKind::Control => controls.insert_reserved_resource(key, value),
            HandleKind::Node => panic!("resource plan cannot contain a node key"),
        }
    }
    for (key, value) in edits {
        if retired_set.contains(&key) {
            continue;
        }
        let previous = match key.kind {
            HandleKind::Port => ports
                .get(key.slot, key.generation)
                .expect("prepared Port edit still addresses a live slot"),
            HandleKind::Connector => connectors
                .get(key.slot, key.generation)
                .expect("prepared Connector edit still addresses a live slot"),
            HandleKind::Control => controls
                .get(key.slot, key.generation)
                .expect("prepared control edit still addresses a live slot"),
            HandleKind::Node => panic!("resource plan cannot contain a node key"),
        };
        index_resource_replace(
            key,
            previous,
            &value,
            connectors_by_port,
            ports_by_selected_connector,
            &connector_touched,
            &selected_touched,
        );
        match key.kind {
            HandleKind::Port => {
                *ports
                    .get_mut(key.slot, key.generation)
                    .expect("prepared Port edit still addresses a live slot") = value
            }
            HandleKind::Connector => {
                *connectors
                    .get_mut(key.slot, key.generation)
                    .expect("prepared Connector edit still addresses a live slot") = value
            }
            HandleKind::Control => {
                *controls
                    .get_mut(key.slot, key.generation)
                    .expect("prepared control edit still addresses a live slot") = value
            }
            HandleKind::Node => panic!("resource plan cannot contain a node key"),
        }
    }
    for key in retired {
        let value = match key.kind {
            HandleKind::Port => ports
                .get(key.slot, key.generation)
                .expect("prepared Port retirement still addresses a live slot"),
            HandleKind::Connector => connectors
                .get(key.slot, key.generation)
                .expect("prepared Connector retirement still addresses a live slot"),
            HandleKind::Control => controls
                .get(key.slot, key.generation)
                .expect("prepared control retirement still addresses a live slot"),
            HandleKind::Node => panic!("resource plan cannot contain a node key"),
        };
        index_resource_remove(
            key,
            value,
            connectors_by_port,
            ports_by_selected_connector,
            &connector_touched,
            &selected_touched,
        );
        match key.kind {
            HandleKind::Port => {
                let _ = ports
                    .remove(key.slot, key.generation)
                    .expect("prepared Port retirement still addresses a live slot");
            }
            HandleKind::Connector => {
                let _ = connectors
                    .remove(key.slot, key.generation)
                    .expect("prepared Connector retirement still addresses a live slot");
            }
            HandleKind::Control => {
                let _ = controls
                    .remove(key.slot, key.generation)
                    .expect("prepared control retirement still addresses a live slot");
            }
            HandleKind::Node => panic!("resource plan cannot contain a node key"),
        }
    }
    for (key, owner) in owner_changes {
        match key.kind {
            HandleKind::Port => {
                port_owners.remove(&key);
                if let Some(owner) = owner {
                    port_owners.insert(key, owner);
                }
            }
            HandleKind::Control => {
                control_owners.remove(&key);
                if let Some(owner) = owner {
                    control_owners.insert(key, owner);
                }
            }
            HandleKind::Connector | HandleKind::Node => {}
        }
    }
    cleanup_reverse_buckets(connectors_by_port, &connector_touched);
    cleanup_reverse_buckets(ports_by_selected_connector, &selected_touched);
}

fn index_resource_add(
    key: ResourceKey,
    value: &ResourceRecord,
    connectors_by_port: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    ports_by_selected_connector: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
) {
    match key.kind {
        HandleKind::Connector => {
            if let Some(port) = value.port {
                connectors_by_port
                    .get_mut(&port)
                    .expect("prepared Connector reverse-index reservation")
                    .insert(key);
            }
        }
        HandleKind::Port => {
            if let Some(connector) = value.selected {
                ports_by_selected_connector
                    .get_mut(&connector)
                    .expect("prepared Port reverse-index reservation")
                    .insert(key);
            }
        }
        HandleKind::Control | HandleKind::Node => {}
    }
}

fn index_resource_remove(
    key: ResourceKey,
    value: &ResourceRecord,
    connectors_by_port: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    ports_by_selected_connector: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    connector_touched: &HashSet<ResourceKey>,
    selected_touched: &HashSet<ResourceKey>,
) {
    match key.kind {
        HandleKind::Connector => {
            if let Some(port) = value.port {
                remove_reverse_index(connectors_by_port, port, key, connector_touched);
            }
        }
        HandleKind::Port => {
            if let Some(connector) = value.selected {
                remove_reverse_index(
                    ports_by_selected_connector,
                    connector,
                    key,
                    selected_touched,
                );
            }
        }
        HandleKind::Control | HandleKind::Node => {}
    }
}

fn index_resource_replace(
    key: ResourceKey,
    previous: &ResourceRecord,
    next: &ResourceRecord,
    connectors_by_port: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    ports_by_selected_connector: &mut HashMap<ResourceKey, HashSet<ResourceKey>>,
    connector_touched: &HashSet<ResourceKey>,
    selected_touched: &HashSet<ResourceKey>,
) {
    match key.kind {
        HandleKind::Connector if previous.port != next.port => {
            index_resource_remove(
                key,
                previous,
                connectors_by_port,
                ports_by_selected_connector,
                connector_touched,
                selected_touched,
            );
            index_resource_add(key, next, connectors_by_port, ports_by_selected_connector);
        }
        HandleKind::Port if previous.selected != next.selected => {
            index_resource_remove(
                key,
                previous,
                connectors_by_port,
                ports_by_selected_connector,
                connector_touched,
                selected_touched,
            );
            index_resource_add(key, next, connectors_by_port, ports_by_selected_connector);
        }
        _ => {}
    }
}

fn reserve_reverse_index<K, V, I>(
    index: &mut HashMap<K, HashSet<V>>,
    relations: I,
) -> Result<(HashMap<K, HashSet<V>>, HashSet<K>), CommitIssue>
where
    K: Copy + Eq + std::hash::Hash,
    V: Copy + Eq + std::hash::Hash,
    I: IntoIterator<Item = (K, V)>,
{
    let relations = relations.into_iter();
    let mut grouped = HashMap::new();
    grouped
        .try_reserve(relations.size_hint().0)
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "reverse dependency index"))?;
    for (dependency, value) in relations {
        grouped
            .entry(dependency)
            .or_insert_with(HashSet::new)
            .insert(value);
    }

    index
        .try_reserve(grouped.len())
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "reverse dependency index"))?;
    let mut new_buckets = HashMap::new();
    new_buckets
        .try_reserve(grouped.len())
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "reverse dependency index"))?;
    let mut touched = HashSet::new();
    touched
        .try_reserve(grouped.len())
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "reverse dependency index"))?;
    for (dependency, bucket) in grouped {
        touched.insert(dependency);
        if let Some(keys) = index.get_mut(&dependency) {
            keys.try_reserve(bucket.len()).map_err(|_| {
                CommitIssue::new(CommitDetail::Capacity, "reverse dependency index")
            })?;
        } else {
            new_buckets.insert(dependency, bucket);
        }
    }
    Ok((new_buckets, touched))
}

fn remove_reverse_index<K, V>(
    index: &mut HashMap<K, HashSet<V>>,
    dependency: K,
    key: V,
    touched: &HashSet<K>,
) where
    K: Eq + std::hash::Hash,
    V: Eq + std::hash::Hash,
{
    if let Some(keys) = index.get_mut(&dependency) {
        keys.remove(&key);
        if keys.is_empty() && !touched.contains(&dependency) {
            index.remove(&dependency);
        }
    }
}

fn cleanup_reverse_buckets<K, V>(index: &mut HashMap<K, HashSet<V>>, touched: &HashSet<K>)
where
    K: Copy + Eq + std::hash::Hash,
    V: Eq + std::hash::Hash,
{
    for dependency in touched {
        if index.get(dependency).is_some_and(HashSet::is_empty) {
            index.remove(dependency);
        }
    }
}

fn local_creation(operation: &UiOperation) -> Option<(u32, LocalFamily)> {
    match operation {
        UiOperation::CreateNode { local_ordinal, .. }
        | UiOperation::CreateRoot { local_ordinal, .. } => {
            Some((*local_ordinal, LocalFamily::Node))
        }
        UiOperation::CreatePort { local_ordinal, .. } => Some((*local_ordinal, LocalFamily::Port)),
        UiOperation::CreateConnector { local_ordinal, .. } => {
            Some((*local_ordinal, LocalFamily::Connector))
        }
        UiOperation::CreateControl { local_ordinal, .. } => {
            Some((*local_ordinal, LocalFamily::Control))
        }
        _ => None,
    }
}

fn validate_commit_configs(
    batch: &UiCommit,
    local_specs: &BTreeMap<u32, LocalFamily>,
) -> Result<(), CommitIssue> {
    let mut targets = HashMap::new();
    targets
        .try_reserve(batch.operations.len())
        .map_err(|_| CommitIssue::new(CommitDetail::Capacity, "config target capacity"))?;
    for operation in &batch.operations {
        let target = match operation {
            UiOperation::CreateControl {
                local_ordinal,
                kind,
                ..
            } => Some((*local_ordinal, ConfigTarget::Control(*kind))),
            UiOperation::CreateRoot {
                local_ordinal,
                role,
                ..
            } => Some((*local_ordinal, ConfigTarget::Root(*role))),
            _ => None,
        };
        if let Some((ordinal, target)) = target {
            targets.insert(ordinal, target);
        }
    }
    for (&ordinal, config) in &batch.control_configs {
        let Some(LocalFamily::Control) = local_specs.get(&ordinal) else {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "control config must target a local Control creation",
            ));
        };
        let Some(ConfigTarget::Control(kind)) = targets.get(&ordinal).copied() else {
            return Err(CommitIssue::new(
                CommitDetail::Invariant,
                "control config creation is missing its kind",
            ));
        };
        config.validate_for(kind).map_err(config_issue)?;
    }
    for &ordinal in batch.root_configs.keys() {
        let Some(LocalFamily::Node) = local_specs.get(&ordinal) else {
            return Err(CommitIssue::new(
                CommitDetail::InvalidTopology,
                "root config must target a local root creation",
            ));
        };
        let Some(ConfigTarget::Root(role)) = targets.get(&ordinal).copied() else {
            return Err(CommitIssue::new(
                CommitDetail::Invariant,
                "root config creation is missing its role",
            ));
        };
        batch
            .root_config(ordinal)
            .validate_for(role)
            .map_err(config_issue)?;
    }
    Ok(())
}

fn config_issue(error: ConfigError) -> CommitIssue {
    CommitIssue::new(
        match error {
            ConfigError::WrongKind => CommitDetail::InvalidTopology,
            ConfigError::InvalidValue => CommitDetail::InvalidProperty,
        },
        "typed UI config does not match its creation kind",
    )
}

const fn local_family_index(family: LocalFamily) -> usize {
    match family {
        LocalFamily::Node => 0,
        LocalFamily::Port => 1,
        LocalFamily::Connector => 2,
        LocalFamily::Control => 3,
    }
}

fn assign_local_objects(
    specifications: &BTreeMap<u32, LocalFamily>,
    node_keys: Vec<NodeKey>,
    port_keys: Vec<ResourceKey>,
    connector_keys: Vec<ResourceKey>,
    control_keys: Vec<ResourceKey>,
) -> BTreeMap<u32, LocalObject> {
    let mut nodes = node_keys.into_iter();
    let mut ports = port_keys.into_iter();
    let mut connectors = connector_keys.into_iter();
    let mut controls = control_keys.into_iter();
    specifications
        .iter()
        .map(|(ordinal, family)| {
            let object = match family {
                LocalFamily::Node => LocalObject::Node(
                    nodes
                        .next()
                        .expect("node reservation count matches local declarations"),
                ),
                LocalFamily::Port => LocalObject::Resource(
                    ports
                        .next()
                        .expect("Port reservation count matches local declarations"),
                ),
                LocalFamily::Connector => LocalObject::Resource(
                    connectors
                        .next()
                        .expect("Connector reservation count matches local declarations"),
                ),
                LocalFamily::Control => LocalObject::Resource(
                    controls
                        .next()
                        .expect("control reservation count matches local declarations"),
                ),
            };
            (*ordinal, object)
        })
        .collect()
}

fn insert_provisional(
    operation: &UiOperation,
    local_objects: &BTreeMap<u32, LocalObject>,
    tree: &mut TreeDraft<'_>,
    resources: &mut ResourceDraft<'_>,
) -> Result<(), CommitIssue> {
    match operation {
        UiOperation::CreateNode {
            local_ordinal,
            kind,
        } => {
            let key = local_node_object(local_objects, *local_ordinal)?;
            tree.insert_created(key, Occurrence::new(*kind, None))
                .map_err(tree_issue)
        }
        UiOperation::CreateRoot {
            local_ordinal,
            role,
            ..
        } => {
            if *role == RootRole::Body {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "the host body root already exists",
                ));
            }
            let key = local_node_object(local_objects, *local_ordinal)?;
            tree.insert_created(key, Occurrence::new(HostKind::Box, Some(*role)))
                .map_err(tree_issue)
        }
        UiOperation::CreatePort {
            local_ordinal,
            content_family,
            ownership,
            owner,
        } => {
            if *ownership == OwnershipMode::OccurrenceOwned && owner.is_none() {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "occurrence-owned Port requires an occurrence owner",
                ));
            }
            let key = local_resource_object(local_objects, *local_ordinal, HandleKind::Port)?;
            resources.insert_created(key, ResourceRecord::port(*ownership, None, *content_family))
        }
        UiOperation::CreateConnector {
            local_ordinal,
            source_index,
            port,
            ownership,
        } => {
            let key = local_resource_object(local_objects, *local_ordinal, HandleKind::Connector)?;
            let port = provisional_resource_key(port, local_objects, resources.document)?;
            if port.kind != HandleKind::Port {
                return Err(CommitIssue::new(
                    CommitDetail::WrongKind,
                    "Connector creation requires a Port",
                ));
            }
            resources.insert_created(
                key,
                ResourceRecord::connector(*ownership, port, *source_index),
            )
        }
        UiOperation::CreateControl {
            local_ordinal,
            kind,
            ownership,
            owner,
        } => {
            if *ownership == OwnershipMode::OccurrenceOwned && owner.is_none() {
                return Err(CommitIssue::new(
                    CommitDetail::InvalidTopology,
                    "occurrence-owned control requires an occurrence owner",
                ));
            }
            let key = local_resource_object(local_objects, *local_ordinal, HandleKind::Control)?;
            resources.insert_created(key, ResourceRecord::control(*ownership, None, *kind))
        }
        _ => Ok(()),
    }
}

struct ReservedDraft {
    local_objects: BTreeMap<u32, LocalObject>,
    tree: TreePlan,
    resources: ResourcePlan,
    roots_added: Vec<NodeKey>,
    roots_removed: Vec<NodeKey>,
    history_roots: Vec<HistoryRootSummary>,
    effects: EffectMask,
    physical_work: bool,
    changed: bool,
}

impl OccurrenceDocument {
    fn reserve_draft(&mut self, batch: &UiCommit) -> Result<CommitDraft<'_>, UiRejection> {
        let (local_specs, node_count, port_count, connector_count, control_count) =
            collect_local_creations(batch.operations())
                .map_err(|(index, issue)| self.rejection(Some(index), issue))?;
        validate_commit_configs(batch, &local_specs)
            .map_err(|issue| self.rejection(None, issue))?;
        let total_live = self
            .nodes
            .live_count()
            .saturating_add(self.ports.live_count())
            .saturating_add(self.connectors.live_count())
            .saturating_add(self.controls.live_count());
        let provisional = node_count
            .saturating_add(port_count)
            .saturating_add(connector_count)
            .saturating_add(control_count);
        if total_live.saturating_add(provisional) > self.arena_capacity {
            return Err(self.rejection(
                None,
                CommitIssue::new(
                    CommitDetail::Capacity,
                    "UI commit exceeds the host occurrence/resource capacity",
                ),
            ));
        }

        self.roots.try_reserve(local_specs.len()).map_err(|_| {
            self.rejection(
                None,
                CommitIssue::new(CommitDetail::Capacity, "root capacity"),
            )
        })?;
        self.port_owners
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "Port owner index capacity"),
                )
            })?;
        self.control_owners
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "control owner index capacity"),
                )
            })?;

        let node_keys = self.reserve_node_keys(node_count).map_err(|error| {
            self.rejection(
                None,
                CommitIssue::new(CommitDetail::Capacity, error.to_string()),
            )
        })?;
        let port_keys = self
            .reserve_resource_keys(ResourceFamily::Port, port_count)
            .map_err(|error| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, error.to_string()),
                )
            })?;
        let connector_keys = self
            .reserve_resource_keys(ResourceFamily::Connector, connector_count)
            .map_err(|error| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, error.to_string()),
                )
            })?;
        let control_keys = self
            .reserve_resource_keys(ResourceFamily::Control, control_count)
            .map_err(|error| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, error.to_string()),
                )
            })?;
        let local_objects = assign_local_objects(
            &local_specs,
            node_keys,
            port_keys,
            connector_keys,
            control_keys,
        );

        let mut tree = TreeDraft::new(&self.nodes, &self.portals_by_owner);
        tree.reserve(batch.operations().len()).map_err(|error| {
            self.rejection(
                None,
                CommitIssue::new(CommitDetail::Capacity, error.to_string()),
            )
        })?;
        let mut resources = ResourceDraft::new(self);
        resources
            .reserve(batch.operations().len())
            .map_err(|issue| self.rejection(None, issue))?;

        let mut roots_added = Vec::new();
        let mut roots_removed = Vec::new();
        roots_added.try_reserve(local_specs.len()).map_err(|_| {
            self.rejection(
                None,
                CommitIssue::new(CommitDetail::Capacity, "root capacity"),
            )
        })?;
        roots_removed
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "root retirement capacity"),
                )
            })?;
        for (index, operation) in batch.operations().iter().enumerate() {
            if let Err(issue) =
                insert_provisional(operation, &local_objects, &mut tree, &mut resources)
            {
                return Err(self.rejection(Some(index), issue));
            }
        }

        let mut draft = CommitDraft {
            document: self,
            local_objects,
            tree,
            resources,
            roots_added,
            roots_removed,
            effects: EffectMask::NONE,
            property_initials: HashMap::new(),
            style_state_initials: HashMap::new(),
            interaction_initials: HashMap::new(),
            physical_work: false,
        };
        draft
            .property_initials
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "property snapshot capacity"),
                )
            })?;
        draft
            .style_state_initials
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "style-state snapshot capacity"),
                )
            })?;
        draft
            .interaction_initials
            .try_reserve(batch.operations().len())
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "interaction snapshot capacity"),
                )
            })?;
        Ok(draft)
    }

    fn reserve_plans(&mut self, finalized: FinalizedDraft) -> Result<ReservedDraft, UiRejection> {
        let FinalizedDraft {
            local_objects,
            tree,
            resources,
            roots_added,
            roots_removed,
            history_roots,
            effects,
            physical_work,
            changed,
        } = finalized;
        let mut tree = tree;
        let mut resources = resources;

        let (portal_buckets, portal_touched) = reserve_reverse_index(
            &mut self.portals_by_owner,
            tree.edits.iter().filter_map(|(key, record)| {
                if tree.retired_set.contains(key) || record.root_role != Some(RootRole::Portal) {
                    return None;
                }
                record.root_owner.map(|owner| (owner, *key))
            }),
        )
        .map_err(|issue| self.rejection(None, issue))?;
        tree.portal_buckets = portal_buckets;
        tree.portal_touched = portal_touched;

        let (connector_buckets, connector_touched) = reserve_reverse_index(
            &mut self.connectors_by_port,
            resources.edits.iter().filter_map(|(key, record)| {
                (!resources.retired_set.contains(key) && key.kind == HandleKind::Connector)
                    .then_some(record.port)
                    .flatten()
                    .map(|port| (port, *key))
            }),
        )
        .map_err(|issue| self.rejection(None, issue))?;
        resources.connector_buckets = connector_buckets;
        resources.connector_touched = connector_touched;

        let (selected_buckets, selected_touched) = reserve_reverse_index(
            &mut self.ports_by_selected_connector,
            resources.edits.iter().filter_map(|(key, record)| {
                (!resources.retired_set.contains(key) && key.kind == HandleKind::Port)
                    .then_some(record.selected)
                    .flatten()
                    .map(|connector| (connector, *key))
            }),
        )
        .map_err(|issue| self.rejection(None, issue))?;
        resources.selected_buckets = selected_buckets;
        resources.selected_touched = selected_touched;

        self.nodes
            .ensure_free_capacity_for(
                tree.retired
                    .iter()
                    .filter(|key| key.generation != u32::MAX)
                    .count(),
            )
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "node retirement capacity"),
                )
            })?;
        self.ports
            .ensure_free_capacity_for(
                resources
                    .retired
                    .iter()
                    .filter(|key| key.kind == HandleKind::Port && key.generation != u32::MAX)
                    .count(),
            )
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "Port retirement capacity"),
                )
            })?;
        self.connectors
            .ensure_free_capacity_for(
                resources
                    .retired
                    .iter()
                    .filter(|key| key.kind == HandleKind::Connector && key.generation != u32::MAX)
                    .count(),
            )
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "Connector retirement capacity"),
                )
            })?;
        self.controls
            .ensure_free_capacity_for(
                resources
                    .retired
                    .iter()
                    .filter(|key| key.kind == HandleKind::Control && key.generation != u32::MAX)
                    .count(),
            )
            .map_err(|_| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Capacity, "control retirement capacity"),
                )
            })?;

        Ok(ReservedDraft {
            local_objects,
            tree,
            resources,
            roots_added,
            roots_removed,
            history_roots,
            effects,
            physical_work,
            changed,
        })
    }

    pub fn prepare_ui_commit(&mut self, batch: &UiCommit) -> Result<PreparedUiCommit, UiRejection> {
        if batch.expected_ui_revision != self.accepted_ui_revision {
            return Err(self.rejection(
                None,
                CommitIssue::new(
                    CommitDetail::StaleRevision,
                    format!(
                        "expected UI revision {}, current revision {}",
                        batch.expected_ui_revision, self.accepted_ui_revision
                    ),
                ),
            ));
        }

        let revision = self.accepted_ui_revision;
        let mut draft = self.reserve_draft(batch)?;
        let mut changed = false;
        for (index, operation) in batch.operations().iter().enumerate() {
            match draft.apply_operation(operation) {
                Ok(operation_changed) => changed |= operation_changed,
                Err(issue) => return Err(rejection_at_revision(revision, Some(index), issue)),
            }
        }
        let finalized = draft
            .finalize(changed)
            .map_err(|issue| rejection_at_revision(revision, None, issue))?;
        let reserved = self.reserve_plans(finalized)?;
        let next_ui_revision = if reserved.changed {
            self.accepted_ui_revision.checked_add(1).ok_or_else(|| {
                self.rejection(
                    None,
                    CommitIssue::new(CommitDetail::Invariant, "accepted UI revision exhausted"),
                )
            })?
        } else {
            self.accepted_ui_revision
        };
        let mut created_handles = Vec::with_capacity(reserved.local_objects.len());
        for object in reserved.local_objects.values() {
            let handle = match object {
                LocalObject::Node(key) => key.handle(self.namespace),
                LocalObject::Resource(key) => key.handle(self.namespace),
            };
            created_handles.push(handle);
        }
        let result = UiOperationResult::accepted(
            next_ui_revision,
            created_handles,
            if reserved.changed || !reserved.effects.is_empty() {
                WAKE_DRAIN
            } else {
                0
            },
        );
        Ok(PreparedUiCommit {
            expected_ui_revision: self.accepted_ui_revision,
            next_ui_revision,
            changed: reserved.changed,
            effects: reserved.effects,
            physical_work: reserved.physical_work,
            tree: reserved.tree,
            resources: reserved.resources,
            roots_added: reserved.roots_added,
            roots_removed: reserved.roots_removed,
            history_roots: reserved.history_roots,
            result,
        })
    }

    pub fn commit_ui(&mut self, batch: &UiCommit) -> Result<AppliedUiCommit, UiRejection> {
        let prepared = self.prepare_ui_commit(batch)?;
        Ok(self.apply_prepared_ui_commit(prepared))
    }

    pub fn apply_prepared_ui_commit(&mut self, prepared: PreparedUiCommit) -> AppliedUiCommit {
        assert_eq!(
            self.accepted_ui_revision, prepared.expected_ui_revision,
            "prepared UI commit belongs to another accepted revision"
        );
        let PreparedUiCommit {
            next_ui_revision,
            tree,
            resources,
            roots_added,
            roots_removed,
            result,
            ..
        } = prepared;
        apply_tree_plan(&mut self.nodes, &mut self.portals_by_owner, tree);
        apply_resource_plan(
            &mut self.ports,
            &mut self.connectors,
            &mut self.controls,
            &mut self.port_owners,
            &mut self.control_owners,
            &mut self.connectors_by_port,
            &mut self.ports_by_selected_connector,
            resources,
        );
        for root in roots_removed {
            self.roots.remove(&root);
        }
        for root in roots_added {
            self.roots.insert(root);
        }
        self.accepted_ui_revision = next_ui_revision;
        result
    }

    fn rejection(&self, index: Option<usize>, issue: CommitIssue) -> UiRejection {
        rejection_at_revision(self.accepted_ui_revision, index, issue)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ColorValue, HandleError, HostNamespace, PropertyValue, SizeMode};
    use super::*;

    fn document() -> OccurrenceDocument {
        OccurrenceDocument::new(HostNamespace::new(7).expect("nonzero namespace"))
    }

    fn node_ref(handle: UiHandle) -> NodeRef {
        NodeRef::Existing(handle)
    }

    fn mount_two_children(document: &mut OccurrenceDocument) -> (UiHandle, UiHandle) {
        let body = document.body_handle();
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        batch.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::Box,
        });
        batch.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(2),
            before: None,
        });
        let result = document.commit_ui(&batch).expect("initial mount");
        let handles = &result.acknowledgement.created;
        assert_eq!(handles.len(), 2);
        assert_eq!(result.words.len(), 16);
        (handles[0], handles[1])
    }

    #[test]
    fn initial_mount_uses_local_handles_and_preallocated_ack_layout() {
        let mut document = document();
        let (first, second) = mount_two_children(&mut document);
        assert_eq!(document.accepted_ui_revision(), 1);
        assert_eq!(document.live_node_count(), 3);
        let first_key = document.node_key(first).expect("first handle");
        let second_key = document.node_key(second).expect("second handle");
        let body = document
            .nodes
            .get(document.body_root().slot, document.body_root().generation)
            .expect("body");
        assert_eq!(body.links.first_child, Some(first_key));
        assert_eq!(body.links.last_child, Some(second_key));
        assert_eq!(body.links.child_count, 2);
    }

    #[test]
    fn preflight_keeps_authoritative_document_unchanged_until_reserved_apply() {
        let mut document = document();
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        batch.push(UiOperation::InsertBefore {
            parent: node_ref(document.body_handle()),
            child: NodeRef::Local(1),
            before: None,
        });
        let prepared = document.prepare_ui_commit(&batch).expect("preflight");
        assert_eq!(document.accepted_ui_revision(), 0);
        assert_eq!(document.live_node_count(), 1);
        let result = document.apply_prepared_ui_commit(prepared);
        assert_eq!(result.acknowledgement.accepted_ui_revision, 1);
        assert_eq!(document.live_node_count(), 2);
    }

    #[test]
    fn property_layers_preserve_masked_base_then_reveal_after_clear() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let mut base = UiCommit::new(document.accepted_ui_revision());
        base.push(UiOperation::SetDeclared {
            node: node_ref(node),
            property: PropertyId::Background,
            value: LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(1))),
        });
        document.commit_ui(&base).expect("base property");

        let mut override_commit = UiCommit::new(document.accepted_ui_revision());
        override_commit.push(UiOperation::SetOverride {
            node: node_ref(node),
            property: PropertyId::Background,
            value: LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(2))),
        });
        document
            .commit_ui(&override_commit)
            .expect("override property");

        let mut masked_base = UiCommit::new(document.accepted_ui_revision());
        masked_base.push(UiOperation::SetDeclared {
            node: node_ref(node),
            property: PropertyId::Background,
            value: LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(3))),
        });
        let result = document
            .commit_ui(&masked_base)
            .expect("masked base update");
        let key = document.node_key(node).expect("node");
        let record = document
            .nodes
            .get(key.slot, key.generation)
            .expect("record");
        assert_eq!(
            record.properties.effective(PropertyId::Background),
            LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(2)))
        );
        assert_eq!(result.acknowledgement.accepted_ui_revision, 4);

        let mut clear = UiCommit::new(document.accepted_ui_revision());
        clear.push(UiOperation::ClearOverride {
            node: node_ref(node),
            property: PropertyId::Background,
        });
        document.commit_ui(&clear).expect("clear override");
        let record = document
            .nodes
            .get(key.slot, key.generation)
            .expect("record");
        assert_eq!(
            record.properties.effective(PropertyId::Background),
            LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(3)))
        );
    }

    #[test]
    fn rejected_batch_does_not_publish_valid_prefix_or_property_write() {
        let mut document = document();
        let (parent, _child) = mount_two_children(&mut document);
        let body = document.body_handle();
        let mut descendant = UiCommit::new(document.accepted_ui_revision());
        descendant.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        descendant.push(UiOperation::InsertBefore {
            parent: node_ref(parent),
            child: NodeRef::Local(1),
            before: None,
        });
        let grandchild = document
            .commit_ui(&descendant)
            .expect("descendant mount")
            .acknowledgement
            .created[0];
        let mut invalid = UiCommit::new(document.accepted_ui_revision());
        invalid.push(UiOperation::SetDeclared {
            node: node_ref(parent),
            property: PropertyId::Width,
            value: LayerValue::Value(PropertyValue::SizeMode(SizeMode::Fit)),
        });
        invalid.push(UiOperation::InsertBefore {
            parent: node_ref(grandchild),
            child: node_ref(parent),
            before: None,
        });
        let rejection = document.commit_ui(&invalid).expect_err("cycle rejection");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(rejection.result.acknowledgement.failed_record, 1);
        assert_eq!(
            rejection.result.acknowledgement.accepted_ui_revision,
            document.accepted_ui_revision()
        );
        let parent_key = document.node_key(parent).expect("parent");
        let record = document
            .nodes
            .get(parent_key.slot, parent_key.generation)
            .expect("parent");
        assert_eq!(
            record.properties.declared(PropertyId::Width),
            &LayerValue::Unset
        );
        let body_record = document
            .nodes
            .get(document.body_root().slot, document.body_root().generation)
            .expect("body");
        assert_eq!(body_record.links.first_child, Some(parent_key));
        assert_eq!(body_record.links.child_count, 2);
        assert_eq!(body, document.body_handle());
    }

    #[test]
    fn self_anchor_is_a_noop_but_wrong_parent_anchor_is_rejected() {
        let mut document = document();
        let (first, second) = mount_two_children(&mut document);
        let body = document.body_handle();
        let mut noop = UiCommit::new(document.accepted_ui_revision());
        noop.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: node_ref(first),
            before: Some(node_ref(first)),
        });
        let result = document.commit_ui(&noop).expect("self anchor");
        assert_eq!(result.acknowledgement.accepted_ui_revision, 1);

        let mut nested = UiCommit::new(document.accepted_ui_revision());
        nested.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        nested.push(UiOperation::InsertBefore {
            parent: node_ref(first),
            child: NodeRef::Local(1),
            before: None,
        });
        let nested_handle = document
            .commit_ui(&nested)
            .expect("nested child")
            .acknowledgement
            .created[0];
        let mut invalid = UiCommit::new(document.accepted_ui_revision());
        invalid.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: node_ref(second),
            before: Some(node_ref(nested_handle)),
        });
        let rejection = document
            .commit_ui(&invalid)
            .expect_err("wrong anchor parent");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
    }

    #[test]
    fn local_creation_ordinals_must_be_dense() {
        let mut document = document();
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::Box,
        });
        let rejection = document
            .commit_ui(&batch)
            .expect_err("sparse local ordinal");
        assert_eq!(rejection.detail, CommitDetail::Malformed);
        assert_eq!(rejection.result.acknowledgement.failed_record, 0);
        assert_eq!(document.live_node_count(), 1);
        assert_eq!(document.accepted_ui_revision(), 0);
    }

    #[test]
    fn moving_a_child_marks_both_parent_frontiers() {
        let mut document = document();
        let (parent, _) = mount_two_children(&mut document);
        let body = document.body_handle();
        let mut nested = UiCommit::new(document.accepted_ui_revision());
        nested.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        nested.push(UiOperation::InsertBefore {
            parent: node_ref(parent),
            child: NodeRef::Local(1),
            before: None,
        });
        let child = document
            .commit_ui(&nested)
            .expect("nested child")
            .acknowledgement
            .created[0];
        let parent_key = document.node_key(parent).expect("parent");
        let body_key = document.node_key(body).expect("body");
        let before_parent = document
            .nodes
            .get(parent_key.slot, parent_key.generation)
            .expect("parent")
            .revisions
            .structure;
        let before_body = document
            .nodes
            .get(body_key.slot, body_key.generation)
            .expect("body")
            .revisions
            .structure;

        let mut move_commit = UiCommit::new(document.accepted_ui_revision());
        move_commit.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: node_ref(child),
            before: None,
        });
        document.commit_ui(&move_commit).expect("move child");

        assert_eq!(
            document
                .nodes
                .get(parent_key.slot, parent_key.generation)
                .expect("parent")
                .revisions
                .structure,
            before_parent + 1
        );
        assert_eq!(
            document
                .nodes
                .get(body_key.slot, body_key.generation)
                .expect("body")
                .revisions
                .structure,
            before_body + 1
        );
    }

    #[test]
    fn descendant_detached_before_retirement_survives() {
        let mut document = document();
        let (parent, child) = mount_two_children(&mut document);
        let body = document.body_handle();
        let mut rescue = UiCommit::new(document.accepted_ui_revision());
        rescue.push(UiOperation::InsertBefore {
            parent: node_ref(parent),
            child: node_ref(child),
            before: None,
        });
        // The first child is already a child of body in this helper; move it
        // beneath the first node, then rescue it before retiring that node.
        document.commit_ui(&rescue).expect("move under parent");
        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::Detach {
            parent: node_ref(parent),
            child: node_ref(child),
        });
        retire.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: node_ref(child),
            before: None,
        });
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(parent),
        });
        document.commit_ui(&retire).expect("rescue then retire");
        assert!(document.node_key(parent).is_err());
        let child_key = document.node_key(child).expect("rescued child");
        assert_eq!(
            document
                .nodes
                .get(child_key.slot, child_key.generation)
                .expect("child")
                .links
                .parent,
            Some(document.body_root())
        );
    }

    #[test]
    fn final_orphan_is_rejected_when_detach_is_not_rescued() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::Detach {
            parent: node_ref(document.body_handle()),
            child: node_ref(node),
        });
        let rejection = document.commit_ui(&batch).expect_err("orphan rejection");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        let key = document.node_key(node).expect("node remains live");
        assert_eq!(
            document
                .nodes
                .get(key.slot, key.generation)
                .expect("node")
                .links
                .parent,
            Some(document.body_root())
        );
    }

    #[test]
    fn slot_reuse_increments_generation_and_rejects_old_handle() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(node),
        });
        document.commit_ui(&retire).expect("retire node");
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(document.body_handle()),
            child: NodeRef::Local(1),
            before: None,
        });
        let replacement = document
            .commit_ui(&create)
            .expect("replacement")
            .acknowledgement
            .created[0];
        assert_eq!(replacement.slot, node.slot);
        assert_eq!(replacement.generation, node.generation + 1);
        assert_eq!(document.node_key(node), Err(HandleError::Stale));
        assert!(document.node_key(replacement).is_ok());
    }

    #[test]
    fn wrong_kind_and_wrong_host_are_rejected_before_apply() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let wrong_kind = UiHandle::new(
            document.namespace(),
            node.slot,
            node.generation,
            HandleKind::Port,
        )
        .expect("well-formed wrong-kind handle");
        let mut kind_batch = UiCommit::new(document.accepted_ui_revision());
        kind_batch.push(UiOperation::SetHidden {
            node: node_ref(wrong_kind),
            hidden: true,
        });
        let rejection = document.commit_ui(&kind_batch).expect_err("wrong kind");
        assert_eq!(rejection.detail, CommitDetail::WrongKind);

        let wrong_host = UiHandle::new(
            HostNamespace::new(8).expect("namespace"),
            node.slot,
            node.generation,
            HandleKind::Node,
        )
        .expect("well-formed wrong-host handle");
        let mut host_batch = UiCommit::new(document.accepted_ui_revision());
        host_batch.push(UiOperation::SetHidden {
            node: node_ref(wrong_host),
            hidden: true,
        });
        let rejection = document.commit_ui(&host_batch).expect_err("wrong host");
        assert_eq!(rejection.detail, CommitDetail::WrongHost);
    }

    #[test]
    fn resource_owner_index_is_atomic_and_occurrence_owned_resources_retire() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        let result = document.commit_ui(&create).expect("resource mount");
        let node = result.acknowledgement.created[0];
        let port = result.acknowledgement.created[1];
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 1);

        let mut duplicate = UiCommit::new(document.accepted_ui_revision());
        duplicate.push(UiOperation::AttachPort {
            node: node_ref(body),
            port: Some(ResourceRef::Existing(port)),
        });
        let rejection = document.commit_ui(&duplicate).expect_err("duplicate owner");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 1);

        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(node),
        });
        document
            .commit_ui(&retire)
            .expect("retire owner and resource");
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 0);
        assert!(document.resource_key(port, ResourceFamily::Port).is_err());

        let mut explicit_document =
            OccurrenceDocument::new(HostNamespace::new(9).expect("nonzero namespace"));
        let body = explicit_document.body_handle();
        let mut explicit = UiCommit::new(explicit_document.accepted_ui_revision());
        explicit.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        explicit.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        explicit.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: Some(NodeRef::Local(1)),
        });
        let explicit_result = explicit_document
            .commit_ui(&explicit)
            .expect("explicit resource mount");
        let explicit_node = explicit_result.acknowledgement.created[0];
        let explicit_port = explicit_result.acknowledgement.created[1];
        let mut remove = UiCommit::new(explicit_document.accepted_ui_revision());
        remove.push(UiOperation::RetireSubtree {
            root: node_ref(explicit_node),
        });
        explicit_document
            .commit_ui(&remove)
            .expect("unmount explicit resource");
        assert!(
            explicit_document
                .resource_key(explicit_port, ResourceFamily::Port)
                .is_ok()
        );
    }

    #[test]
    fn detaching_an_occurrence_owned_port_retires_all_bound_connectors() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 3,
            source_index: 0,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::OccurrenceOwned,
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 4,
            source_index: 1,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::OccurrenceOwned,
        });
        create.push(UiOperation::SelectConnector {
            port: ResourceRef::Local(2),
            connector: Some(ResourceRef::Local(3)),
        });
        let result = document.commit_ui(&create).expect("content resources");
        let node = result.acknowledgement.created[0];
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 1);
        assert_eq!(document.live_resource_count(ResourceFamily::Connector), 2);

        let mut detach = UiCommit::new(document.accepted_ui_revision());
        detach.push(UiOperation::AttachPort {
            node: node_ref(node),
            port: None,
        });
        document.commit_ui(&detach).expect("detach resources");
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 0);
        assert_eq!(document.live_resource_count(ResourceFamily::Connector), 0);
    }

    #[test]
    fn detached_occurrence_port_can_dispose_an_explicit_connector_later() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 3,
            source_index: 0,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::Explicit,
        });
        let result = document.commit_ui(&create).expect("content resources");
        let node = result.acknowledgement.created[0];
        let connector = result.acknowledgement.created[2];

        let mut invalid_detach = UiCommit::new(document.accepted_ui_revision());
        invalid_detach.push(UiOperation::AttachPort {
            node: node_ref(node),
            port: None,
        });
        let rejection = document
            .commit_ui(&invalid_detach)
            .expect_err("explicit Connector keeps Port live");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 1);
        assert_eq!(document.live_resource_count(ResourceFamily::Connector), 1);

        let mut detach = UiCommit::new(document.accepted_ui_revision());
        detach.push(UiOperation::AttachPort {
            node: node_ref(node),
            port: None,
        });
        detach.push(UiOperation::DisposeConnector {
            connector: ResourceRef::Existing(connector),
        });
        document
            .commit_ui(&detach)
            .expect("dispose explicit Connector after Port detach");
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 0);
        assert_eq!(document.live_resource_count(ResourceFamily::Connector), 0);
    }

    #[test]
    fn explicit_port_retains_selected_connector_after_occurrence_detach() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 3,
            source_index: 0,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::Explicit,
        });
        create.push(UiOperation::SelectConnector {
            port: ResourceRef::Local(2),
            connector: Some(ResourceRef::Local(3)),
        });
        let result = document.commit_ui(&create).expect("selected resources");
        let port = result.acknowledgement.created[1];
        let connector = result.acknowledgement.created[2];
        let port_key = document
            .resource_key(port, ResourceFamily::Port)
            .expect("Port handle");
        let connector_key = document
            .resource_key(connector, ResourceFamily::Connector)
            .expect("Connector handle");

        let mut detach = UiCommit::new(document.accepted_ui_revision());
        detach.push(UiOperation::AttachPort {
            node: node_ref(result.acknowledgement.created[0]),
            port: None,
        });
        document.commit_ui(&detach).expect("detach explicit Port");
        assert_eq!(
            document
                .ports
                .get(port_key.slot, port_key.generation)
                .expect("detached Port")
                .selected,
            Some(connector_key),
        );

        let mut dispose = UiCommit::new(document.accepted_ui_revision());
        dispose.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: None,
        });
        dispose.push(UiOperation::DisposeConnector {
            connector: ResourceRef::Existing(connector),
        });
        dispose.push(UiOperation::DisposePort {
            port: ResourceRef::Existing(port),
        });
        document
            .commit_ui(&dispose)
            .expect("deselect and dispose detached resources");
    }

    #[test]
    fn disposing_a_connector_then_its_port_is_atomic_within_one_batch() {
        let mut document = document();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreatePort {
            local_ordinal: 1,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 2,
            source_index: 0,
            port: ResourceRef::Local(1),
            ownership: OwnershipMode::Explicit,
        });
        let result = document.commit_ui(&create).expect("resources");
        let port = result.acknowledgement.created[0];
        let connector = result.acknowledgement.created[1];

        let mut dispose = UiCommit::new(document.accepted_ui_revision());
        dispose.push(UiOperation::DisposeConnector {
            connector: ResourceRef::Existing(connector),
        });
        dispose.push(UiOperation::DisposePort {
            port: ResourceRef::Existing(port),
        });
        document
            .commit_ui(&dispose)
            .expect("retire connector before its port");
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 0);
        assert_eq!(document.live_resource_count(ResourceFamily::Connector), 0);
    }

    #[test]
    fn repeated_history_action_is_a_noop_and_wrong_root_is_rejected() {
        let mut document = document();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: RootRole::LegacyHistoryUnit,
            owner: None,
        });
        let root = document
            .commit_ui(&create)
            .expect("history root")
            .acknowledgement
            .created[0];

        let mut freeze = UiCommit::new(document.accepted_ui_revision());
        freeze.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(root),
            action_id: 1,
        });
        document.commit_ui(&freeze).expect("first history action");
        let mut repeated = UiCommit::new(document.accepted_ui_revision());
        repeated.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(root),
            action_id: 1,
        });
        let result = document.commit_ui(&repeated).expect("repeated action");
        assert_eq!(result.acknowledgement.accepted_ui_revision, 2);

        let mut wrong_root = UiCommit::new(document.accepted_ui_revision());
        wrong_root.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(document.body_handle()),
            action_id: 1,
        });
        let rejection = document
            .commit_ui(&wrong_root)
            .expect_err("body is not a history root");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
    }

    #[test]
    fn discard_history_action_retires_only_the_live_tail_root() {
        let mut document = document();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: RootRole::LegacyHistoryUnit,
            owner: None,
        });
        create.push(UiOperation::CreateRoot {
            local_ordinal: 2,
            role: RootRole::LegacyHistoryUnit,
            owner: None,
        });
        let roots = document
            .commit_ui(&create)
            .expect("History roots")
            .acknowledgement
            .created;

        let mut non_tail = UiCommit::new(document.accepted_ui_revision());
        non_tail.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(roots[0]),
            action_id: 2,
        });
        let rejection = document
            .commit_ui(&non_tail)
            .expect_err("non-tail History discard");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);

        let mut discard = UiCommit::new(document.accepted_ui_revision());
        discard.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(roots[1]),
            action_id: 2,
        });
        document.commit_ui(&discard).expect("tail History discard");
        assert_eq!(document.history_roots().len(), 1);
        assert_eq!(
            document.history_roots()[0],
            roots[0].node_key().expect("node root")
        );
    }

    #[test]
    fn occurrence_owned_resources_require_an_occurrence_owner() {
        let mut document = document();
        let mut port = UiCommit::new(document.accepted_ui_revision());
        port.push(UiOperation::CreatePort {
            local_ordinal: 1,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: None,
        });
        let rejection = document
            .commit_ui(&port)
            .expect_err("ownerless occurrence-owned Port");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(document.live_resource_count(ResourceFamily::Port), 0);

        let mut control = UiCommit::new(document.accepted_ui_revision());
        control.push(UiOperation::CreateControl {
            local_ordinal: 1,
            kind: super::super::ControlKind::Editor,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: None,
        });
        let rejection = document
            .commit_ui(&control)
            .expect_err("ownerless occurrence-owned control");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(document.live_resource_count(ResourceFamily::Control), 0);
    }

    #[test]
    fn dropped_prepare_does_not_publish_reverse_index_buckets() {
        let mut document = document();
        let body = document.body_handle();
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        batch.push(UiOperation::CreateRoot {
            local_ordinal: 2,
            role: RootRole::Portal,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 3,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        batch.push(UiOperation::CreateConnector {
            local_ordinal: 4,
            source_index: 0,
            port: ResourceRef::Local(3),
            ownership: OwnershipMode::Explicit,
        });
        batch.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::SelectConnector {
            port: ResourceRef::Local(3),
            connector: Some(ResourceRef::Local(4)),
        });

        let prepared = document.prepare_ui_commit(&batch).expect("preflight");
        assert!(document.portals_by_owner.is_empty());
        assert!(document.connectors_by_port.is_empty());
        assert!(document.ports_by_selected_connector.is_empty());
        drop(prepared);
        assert!(document.portals_by_owner.is_empty());
        assert!(document.connectors_by_port.is_empty());
        assert!(document.ports_by_selected_connector.is_empty());

        document.accepted_ui_revision = u64::MAX;
        let failing = UiCommit::with_operations(u64::MAX, batch.operations().to_vec());
        assert!(document.prepare_ui_commit(&failing).is_err());
        assert!(document.portals_by_owner.is_empty());
        assert!(document.connectors_by_port.is_empty());
        assert!(document.ports_by_selected_connector.is_empty());
        document.accepted_ui_revision = 0;

        let prepared = document
            .prepare_ui_commit(&batch)
            .expect("second preflight");
        document.apply_prepared_ui_commit(prepared);
        assert_eq!(document.portals_by_owner.len(), 1);
        assert_eq!(document.connectors_by_port.len(), 1);
        assert_eq!(document.ports_by_selected_connector.len(), 1);
    }

    #[test]
    fn created_portal_dependency_ignores_unrelated_created_occurrences() {
        const ORDINARY: u32 = 64;
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        for ordinal in 2..=ORDINARY + 1 {
            create.push(UiOperation::CreateNode {
                local_ordinal: ordinal,
                kind: HostKind::Box,
            });
        }
        create.push(UiOperation::CreateRoot {
            local_ordinal: ORDINARY + 2,
            role: RootRole::Portal,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        for ordinal in 2..=ORDINARY + 1 {
            create.push(UiOperation::InsertBefore {
                parent: node_ref(body),
                child: NodeRef::Local(ordinal),
                before: None,
            });
        }
        let result = document.commit_ui(&create).expect("sparse portal mount");
        let owner = result.acknowledgement.created[0];
        let ordinary = result.acknowledgement.created[1];
        let portal = result.acknowledgement.created[(ORDINARY + 1) as usize];

        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(owner),
        });
        document.commit_ui(&retire).expect("owner retirement");
        assert!(document.node_key(ordinary).is_ok());
        assert!(document.node_key(portal).is_err());
        assert_eq!(document.live_node_count(), ORDINARY as usize + 1);
    }

    #[test]
    fn occurrence_owned_literal_replacement_is_atomic() {
        let mut document = document();
        let body = document.body_handle();
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(ResourceRef::Local(2)),
        });
        batch.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(2),
            content_format: 1,
            content: b"hello".to_vec(),
            annotations: Vec::new(),
        });
        document
            .commit_ui(&batch)
            .unwrap_or_else(|rejection| panic!("literal replacement rejected: {rejection}"));
    }

    #[test]
    fn subtree_retirement_reserves_all_recycled_slots_before_apply() {
        const COUNT: u32 = 32;
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        for ordinal in 1..=COUNT {
            create.push(UiOperation::CreateNode {
                local_ordinal: ordinal,
                kind: HostKind::Box,
            });
        }
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        for ordinal in 2..=COUNT {
            create.push(UiOperation::InsertBefore {
                parent: NodeRef::Local(ordinal - 1),
                child: NodeRef::Local(ordinal),
                before: None,
            });
        }
        let first = document
            .commit_ui(&create)
            .expect("deep subtree")
            .acknowledgement
            .created[0];
        assert_eq!(document.nodes.free_len(), 0);
        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(first),
        });
        let prepared = document.prepare_ui_commit(&retire).expect("preflight");
        assert!(document.nodes.free_capacity() - document.nodes.free_len() >= COUNT as usize);
        document.apply_prepared_ui_commit(prepared);
        assert_eq!(document.nodes.free_len(), COUNT as usize);
    }

    #[test]
    fn portal_owner_retirement_retires_the_portal_and_descendants() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        create.push(UiOperation::CreateRoot {
            local_ordinal: 2,
            role: RootRole::Portal,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Box,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(2),
            child: NodeRef::Local(3),
            before: None,
        });
        let result = document.commit_ui(&create).expect("portal mount");
        let owner = result.acknowledgement.created[0];
        let portal = result.acknowledgement.created[1];
        let child = result.acknowledgement.created[2];

        let mut retire = UiCommit::new(document.accepted_ui_revision());
        retire.push(UiOperation::RetireSubtree {
            root: node_ref(owner),
        });
        document.commit_ui(&retire).expect("owner retirement");
        assert_eq!(document.live_node_count(), 1);
        assert!(document.node_key(owner).is_err());
        assert!(document.node_key(portal).is_err());
        assert!(document.node_key(child).is_err());
        assert!(
            document
                .roots
                .iter()
                .all(|root| *root == document.body_root)
        );
    }

    #[test]
    fn moving_a_portal_owner_under_its_portal_child_rejects_the_owner_cycle() {
        let mut document = document();
        let body = document.body_handle();
        let mut create = UiCommit::new(document.accepted_ui_revision());
        create.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        create.push(UiOperation::CreateRoot {
            local_ordinal: 2,
            role: RootRole::Portal,
            owner: Some(NodeRef::Local(1)),
        });
        create.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Box,
        });
        create.push(UiOperation::InsertBefore {
            parent: node_ref(body),
            child: NodeRef::Local(1),
            before: None,
        });
        create.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(2),
            child: NodeRef::Local(3),
            before: None,
        });
        let result = document.commit_ui(&create).expect("portal mount");
        let owner = result.acknowledgement.created[0];
        let child = result.acknowledgement.created[2];
        let mut move_owner = UiCommit::new(document.accepted_ui_revision());
        move_owner.push(UiOperation::InsertBefore {
            parent: node_ref(child),
            child: node_ref(owner),
            before: None,
        });
        let rejection = document
            .commit_ui(&move_owner)
            .expect_err("portal ownership cycle");
        assert_eq!(rejection.detail, CommitDetail::InvalidTopology);
        assert_eq!(rejection.result.acknowledgement.failed_record, 0);
        assert_eq!(document.accepted_ui_revision(), 1);
        let owner_key = document.node_key(owner).expect("owner remains live");
        assert_eq!(
            document
                .nodes
                .get(owner_key.slot, owner_key.generation)
                .expect("owner")
                .links
                .parent,
            Some(document.body_root())
        );
    }

    #[test]
    fn a_true_noop_does_not_advance_the_accepted_revision() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let mut batch = UiCommit::new(document.accepted_ui_revision());
        batch.push(UiOperation::SetDeclared {
            node: node_ref(node),
            property: PropertyId::Width,
            value: LayerValue::Value(PropertyValue::SizeMode(SizeMode::Fit)),
        });
        let first = document.commit_ui(&batch).expect("first property");
        assert_eq!(first.acknowledgement.accepted_ui_revision, 2);
        let mut repeat = UiCommit::new(document.accepted_ui_revision());
        repeat.push(UiOperation::SetDeclared {
            node: node_ref(node),
            property: PropertyId::Width,
            value: LayerValue::Value(PropertyValue::SizeMode(SizeMode::Fit)),
        });
        let second = document.commit_ui(&repeat).expect("same property");
        assert_eq!(second.acknowledgement.accepted_ui_revision, 2);
        assert_eq!(document.accepted_ui_revision(), 2);

        let mut base = UiCommit::new(document.accepted_ui_revision());
        base.push(UiOperation::SetDeclared {
            node: node_ref(node),
            property: PropertyId::Background,
            value: LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(1))),
        });
        document.commit_ui(&base).expect("coalescing base");
        let mut coalesced = UiCommit::new(document.accepted_ui_revision());
        for color in [1, 2, 1] {
            coalesced.push(UiOperation::SetDeclared {
                node: node_ref(node),
                property: PropertyId::Background,
                value: LayerValue::Value(PropertyValue::Color(ColorValue::Ansi(color))),
            });
        }
        let third = document.commit_ui(&coalesced).expect("coalesced property");
        assert_eq!(third.acknowledgement.accepted_ui_revision, 3);

        let mut style = UiCommit::new(document.accepted_ui_revision());
        style.push(UiOperation::SetStyleState {
            node: node_ref(node),
            layer: 0,
            key: "selected".to_owned(),
            value: "on".to_owned(),
        });
        document.commit_ui(&style).expect("style state");
        let mut style_coalesced = UiCommit::new(document.accepted_ui_revision());
        for value in ["on", "off", "on"] {
            style_coalesced.push(UiOperation::SetStyleState {
                node: node_ref(node),
                layer: 0,
                key: "selected".to_owned(),
                value: value.to_owned(),
            });
        }
        let fourth = document
            .commit_ui(&style_coalesced)
            .expect("coalesced style state");
        assert_eq!(fourth.acknowledgement.accepted_ui_revision, 4);

        let mut hidden = UiCommit::new(document.accepted_ui_revision());
        hidden.push(UiOperation::SetHidden {
            node: node_ref(node),
            hidden: true,
        });
        document.commit_ui(&hidden).expect("hidden state");
        let mut hidden_coalesced = UiCommit::new(document.accepted_ui_revision());
        for value in [true, false, true] {
            hidden_coalesced.push(UiOperation::SetHidden {
                node: node_ref(node),
                hidden: value,
            });
        }
        let fifth = document
            .commit_ui(&hidden_coalesced)
            .expect("coalesced hidden state");
        assert_eq!(fifth.acknowledgement.accepted_ui_revision, 5);
    }

    #[test]
    fn style_state_rejects_empty_values_without_partial_apply() {
        let mut document = document();
        let (node, _) = mount_two_children(&mut document);
        let mut invalid = UiCommit::new(document.accepted_ui_revision());
        invalid.push(UiOperation::SetStyleState {
            node: node_ref(node),
            layer: 0,
            key: "selected".to_owned(),
            value: String::new(),
        });
        let rejection = document
            .commit_ui(&invalid)
            .expect_err("empty style state value");
        assert_eq!(rejection.detail, CommitDetail::Malformed);
        assert_eq!(document.accepted_ui_revision(), 1);
        let key = document.node_key(node).expect("node");
        assert!(
            document
                .nodes
                .get(key.slot, key.generation)
                .expect("record")
                .style_states
                .is_empty()
        );
    }
}
