//! Core owner for accepted direct-occurrence UI resource state and commit planning.
//!
//! This module owns resource lifecycle and semantic control preparation. N-API
//! only decodes and qualifies input before calling this owner.
use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

use crate::binding::{
    ContentAnnotationRecord, ContentFamily, ControlError, ControlState, FunnelSpec, HandleKind,
    HostContentSource, HostNamespace, NodeKey, ResourceKey, ResourceRef, RootConfig, RootRole,
    SourceInstallDisposition, TextFunnelKind, TuiEnvironment, UiAcknowledgement, UiCommit,
    UiOperation,
};

use super::content::TextSourceKind;
use crate::history::HistoryUnitId;

pub struct UiCommitOutput {
    pub result: crate::binding::UiOperationResult,
    pub wakes: crate::application::content::PreparedSourceWakes,
    pub(crate) changes: crate::occurrence::UiChangeSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HistoryUnitStatus {
    Live,
    Frozen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoryUnitOwner {
    pub(crate) id: HistoryUnitId,
    pub(crate) flow_boundary: u32,
    pub(crate) status: HistoryUnitStatus,
}

/// Fully prepared but not yet accepted UI/resource work. The host reserves
/// its projection frontier using this exact summary before this value is
/// installed; dropping it leaves the authoritative document, Sources, and
/// pending projection frontier unchanged.
pub(crate) struct PreparedUiResources {
    prepared: crate::occurrence::commit::PreparedUiCommit,
    plan: ResourceCommitPlan,
    source_mutations: Vec<crate::application::content::PreparedSourceMutation>,
    changes: crate::occurrence::UiChangeSet,
    history_updates: Vec<(NodeKey, Option<HistoryUnitOwner>)>,
}

impl PreparedUiResources {
    pub(crate) fn changes(&self) -> &crate::occurrence::UiChangeSet {
        &self.changes
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceIdentity {
    pub id: u64,
    pub generation: u32,
    pub environment_slot: u32,
    pub environment_generation: u32,
}

impl SourceIdentity {
    pub fn from_source(source: &HostContentSource) -> Self {
        Self {
            id: source.id(),
            generation: source.generation(),
            environment_slot: source.environment_slot(),
            environment_generation: source.environment_generation(),
        }
    }
}

pub struct UiResourceOwner {
    pub namespace: HostNamespace,
    pub document: Option<crate::occurrence::OccurrenceDocument>,
    pub environment: TuiEnvironment,
    pub ports: HashSet<ResourceKey>,
    pub connectors: HashMap<ResourceKey, (SourceIdentity, FunnelSpec)>,
    pub literal_sources: HashMap<ResourceKey, SourceIdentity>,
    pub private_sources: HashMap<ResourceKey, SourceIdentity>,
    pub controls: HashMap<ResourceKey, ControlState>,
    pub root_configs: HashMap<NodeKey, RootConfig>,
    pub(crate) history_units: HashMap<NodeKey, HistoryUnitOwner>,
    lifecycle: UiResourceLifecycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiResourceLifecycle {
    Open,
    Closing,
    Closed,
}

const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<UiResourceOwner>();
};

impl UiResourceOwner {
    pub fn new(namespace: HostNamespace, environment: TuiEnvironment) -> Self {
        Self {
            namespace,
            document: Some(crate::occurrence::OccurrenceDocument::new(namespace)),
            environment,
            ports: HashSet::new(),
            connectors: HashMap::new(),
            literal_sources: HashMap::new(),
            private_sources: HashMap::new(),
            controls: HashMap::new(),
            root_configs: HashMap::new(),
            history_units: HashMap::new(),
            lifecycle: UiResourceLifecycle::Open,
        }
    }

    fn document_ref(&self) -> &crate::occurrence::OccurrenceDocument {
        self.document
            .as_ref()
            .expect("open UI resource owner has an occurrence document")
    }

    fn document_mut(&mut self) -> &mut crate::occurrence::OccurrenceDocument {
        self.document
            .as_mut()
            .expect("open UI resource owner has an occurrence document")
    }

    pub fn body_handle(&self) -> std::result::Result<crate::binding::UiHandle, String> {
        if self.lifecycle != UiResourceLifecycle::Open {
            return Err("UI resource owner is closing or closed".to_owned());
        }
        Ok(self.document_ref().body_handle())
    }

    pub(crate) fn is_open(&self) -> bool {
        self.lifecycle == UiResourceLifecycle::Open && self.document.is_some()
    }

    pub(crate) fn document_snapshot(
        &self,
        key: NodeKey,
    ) -> std::result::Result<crate::occurrence::OccurrenceSnapshot, String> {
        self.document_ref()
            .snapshot(key)
            .map_err(|error| format!("occurrence snapshot failed for {key:?}: {error:?}"))
    }

    pub(crate) fn demanded_ports(&self) -> anyhow::Result<Vec<ResourceKey>> {
        let document = self.document_ref();
        let mut demanded = Vec::new();
        let mut stack = vec![document.body_root()];
        stack.extend(document.history_roots());
        for root in document.portal_roots() {
            if self.portal_is_demanded(root)? {
                stack.push(root);
            }
        }
        while let Some(node) = stack.pop() {
            let snapshot = self.document_snapshot(node).map_err(anyhow::Error::msg)?;
            if let Some(port) = snapshot.port {
                demanded.push(port);
            }
            if snapshot.kind == crate::occurrence::HostKind::Animation {
                let active = snapshot
                    .control
                    .and_then(|key| self.control_state(key))
                    .and_then(crate::occurrence::ControlState::animation_active_frame)
                    .unwrap_or(0) as usize;
                if let Some(child) = snapshot.children.get(active) {
                    stack.push(*child);
                }
            } else {
                stack.extend(snapshot.children.iter().rev().copied());
            }
        }
        Ok(demanded)
    }

    pub(crate) fn demanded_port(&self, port: ResourceKey) -> anyhow::Result<(bool, usize)> {
        let Some(node) = self.document_ref().port_owner(port) else {
            return Ok((false, 0));
        };
        let active_animation = |control| {
            self.control_state(control)
                .and_then(crate::occurrence::ControlState::animation_active_frame)
                .map(|frame| frame as usize)
        };
        self.document_ref()
            .demanded_node(node, active_animation)
            .map_err(|error| anyhow!("occurrence demand lookup failed: {error:?}"))
    }

    pub(crate) fn ports_under_nodes(
        &self,
        roots: &[NodeKey],
    ) -> anyhow::Result<(Vec<ResourceKey>, usize)> {
        self.document_ref()
            .ports_under_nodes(roots)
            .map_err(|error| anyhow!("occurrence content membership lookup failed: {error:?}"))
    }

    pub(crate) fn portal_is_demanded(&self, root: NodeKey) -> anyhow::Result<bool> {
        let active_animation = |control| {
            self.control_state(control)
                .and_then(crate::occurrence::ControlState::animation_active_frame)
                .map(|frame| frame as usize)
        };
        self.document_ref()
            .demanded_node(root, active_animation)
            .map(|(demanded, _)| demanded)
            .map_err(|error| anyhow!("occurrence Portal demand lookup failed: {error:?}"))
    }

    pub(crate) fn history_roots(&self) -> Vec<NodeKey> {
        self.document_ref().history_roots()
    }

    pub(crate) fn root_config(&self, root: NodeKey) -> Option<RootConfig> {
        self.root_configs.get(&root).copied()
    }

    pub(crate) fn history_unit(&self, root: NodeKey) -> Option<HistoryUnitOwner> {
        self.history_units.get(&root).copied()
    }

    fn prepare_history_updates(
        &self,
        prepared: &crate::occurrence::commit::PreparedUiCommit,
        plan: &ResourceCommitPlan,
        existing_history: &HashMap<HistoryUnitId, bool>,
    ) -> std::result::Result<Vec<(NodeKey, Option<HistoryUnitOwner>)>, crate::binding::UiRejection>
    {
        // A rejection describes the still-accepted document.  The prepared
        // acknowledgement carries the revision that would be accepted, which
        // must not leak out as though this transaction had committed.
        let revision = self.document_ref().accepted_ui_revision();
        let mut updates = Vec::new();
        updates
            .try_reserve(prepared.history_root_summaries().len())
            .map_err(|_| self.history_rejection(revision, "History owner update capacity"))?;
        let added = prepared.added_root_keys();
        let retired_nodes = prepared.retired_node_keys();
        let mut supplied_ids = HashSet::new();
        for summary in prepared.history_root_summaries() {
            let config = plan
                .root_config_for(summary.root)
                .or_else(|| self.root_configs.get(&summary.root).copied())
                .ok_or_else(|| {
                    self.history_rejection(revision, "History root config is missing")
                })?;
            let current = self.history_units.get(&summary.root).copied();
            if current.is_none() && !added.contains(&summary.root) {
                return Err(self.history_rejection(
                    revision,
                    "History root has no accepted native unit identity",
                ));
            }
            if current.is_none() && added.contains(&summary.root) {
                if plan.supplied_history_identity(summary.root) {
                    let id = HistoryUnitId::from_value(
                        config
                            .unit_identity
                            .expect("supplied History identity is present in its config"),
                    )
                    .ok_or_else(|| {
                        self.history_rejection(revision, "History unit identity is invalid")
                    })?;
                    if !supplied_ids.insert(id)
                        || self.history_units.values().any(|owner| owner.id == id)
                    {
                        return Err(self.history_rejection(
                            revision,
                            "History unit identity is already owned by this UI document",
                        ));
                    }
                    let expected_live = match summary.action {
                        Some(1) => false,
                        None => true,
                        Some(2) => {
                            return Err(self.history_rejection(
                                revision,
                                "History discard requires an accepted live tail root",
                            ));
                        }
                        Some(_) => unreachable!("History action was validated by occurrence owner"),
                    };
                    if existing_history.get(&id).copied() != Some(expected_live) {
                        return Err(self.history_rejection(
                            revision,
                            "History identity must adapt a same-host unit with matching status",
                        ));
                    }
                }
            }
            if let Some(owner) = current {
                if config.flow_boundary != owner.flow_boundary
                    || config
                        .unit_identity
                        .is_some_and(|id| id != owner.id.value())
                {
                    return Err(self.history_rejection(
                        revision,
                        "History root config does not match its accepted native unit",
                    ));
                }
            }
            let owner = match current {
                Some(owner) => match (owner.status, summary.action) {
                    (HistoryUnitStatus::Frozen, None) => {
                        return Err(
                            self.history_rejection(revision, "a frozen History unit cannot change")
                        );
                    }
                    (HistoryUnitStatus::Frozen, Some(1)) => {
                        return Err(
                            self.history_rejection(revision, "a frozen History unit cannot change")
                        );
                    }
                    (HistoryUnitStatus::Live, Some(1)) => {
                        if summary.has_live_control {
                            return Err(self.history_rejection(
                                revision,
                                "History freeze final content cannot contain a live control",
                            ));
                        }
                        HistoryUnitOwner {
                            status: HistoryUnitStatus::Frozen,
                            ..owner
                        }
                    }
                    (HistoryUnitStatus::Live, None) => owner,
                    (HistoryUnitStatus::Live, Some(2)) => {
                        if !retired_nodes.contains(&summary.root) {
                            return Err(self.history_rejection(
                                revision,
                                "History discard must retire its live tail root",
                            ));
                        }
                        // The retirement update below removes the owner.  Do
                        // not leave a transient accepted state that the
                        // renderer could observe as a surviving unit.
                        continue;
                    }
                    (HistoryUnitStatus::Frozen, Some(2)) => {
                        return Err(self.history_rejection(
                            revision,
                            "History discard requires a live tail root",
                        ));
                    }
                    (_, Some(action)) => {
                        return Err(self.history_rejection(
                            revision,
                            if action == 1 {
                                "History freeze action is invalid"
                            } else {
                                "History action is invalid"
                            },
                        ));
                    }
                },
                None => {
                    if summary.action == Some(2) {
                        return Err(self.history_rejection(
                            revision,
                            "History discard requires an accepted live tail root",
                        ));
                    }
                    HistoryUnitOwner {
                        id: HistoryUnitId::from_value(
                            config
                                .unit_identity
                                .expect("History root preflight allocated its unit identity"),
                        )
                        .expect("History root identity is non-zero"),
                        flow_boundary: config.flow_boundary,
                        status: if summary.action == Some(1) {
                            if summary.has_live_control {
                                return Err(self.history_rejection(
                                    revision,
                                    "History freeze final content cannot contain a live control",
                                ));
                            }
                            HistoryUnitStatus::Frozen
                        } else {
                            HistoryUnitStatus::Live
                        },
                    }
                }
            };
            updates.push((summary.root, Some(owner)));
        }
        let current_history_roots = self.document_ref().history_roots();
        let current_tail = current_history_roots.last().copied();
        for root in prepared.retired_node_keys() {
            let Some(owner) = self.history_units.get(&root).copied() else {
                continue;
            };
            if owner.status != HistoryUnitStatus::Live || current_tail != Some(root) {
                return Err(self.history_rejection(
                    revision,
                    "RetireRoot cannot bypass History live-tail restrictions",
                ));
            }
            updates.push((root, None));
        }
        Ok(updates)
    }

    fn history_rejection(
        &self,
        revision: u64,
        message: &'static str,
    ) -> crate::binding::UiRejection {
        crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::InvalidTopology,
            message,
        )
    }

    fn apply_history_updates(&mut self, updates: Vec<(NodeKey, Option<HistoryUnitOwner>)>) {
        for (root, owner) in updates {
            match owner {
                Some(owner) => {
                    self.history_units.insert(root, owner);
                }
                None => {
                    self.history_units.remove(&root);
                }
            }
        }
    }

    pub(crate) fn content_binding(
        &self,
        port: ResourceKey,
    ) -> anyhow::Result<Option<(ResourceKey, HostContentSource, FunnelSpec)>> {
        let connector = self
            .document_ref()
            .selected_connector(port)
            .or_else(|| self.literal_sources.contains_key(&port).then_some(port));
        let Some(connector) = connector else {
            return Ok(None);
        };
        let Some((identity, funnel)) = self.connectors.get(&connector).copied() else {
            return Err(anyhow!(
                "UI connector {connector:?} is selected but has no accepted Source"
            ));
        };
        let source = self
            .environment
            .lookup_content_source(identity.id, identity.generation)?;
        Ok(Some((connector, source, funnel)))
    }

    pub(crate) fn connector_requested(&self, connector: ResourceKey) -> bool {
        self.document_ref()
            .resource_port(connector)
            .and_then(|port| self.document_ref().selected_connector(port))
            == Some(connector)
    }

    pub(crate) fn resource_is_live(&self, key: ResourceKey) -> bool {
        self.document_ref().resource_is_live(key)
    }

    pub(crate) fn resource_port_key(&self, key: ResourceKey) -> Option<ResourceKey> {
        self.document_ref().resource_port(key)
    }

    pub(crate) fn control_state(
        &self,
        key: ResourceKey,
    ) -> Option<&crate::occurrence::ControlState> {
        self.controls.get(&key)
    }

    pub(crate) fn set_native_animation_frame(
        &mut self,
        key: ResourceKey,
        frame: usize,
    ) -> anyhow::Result<()> {
        let Some(state) = self.controls.get_mut(&key) else {
            return Err(anyhow!("native animation control is retired"));
        };
        let crate::occurrence::ControlState::Animation(animation) = state else {
            return Err(anyhow!(
                "native animation frame targeted a non-animation control"
            ));
        };
        animation
            .set_active_frame(frame)
            .map_err(|_| anyhow!("native animation frame exceeds the UI wire range"))
    }

    pub fn close(&mut self) -> std::result::Result<(), String> {
        if self.lifecycle == UiResourceLifecycle::Closed {
            return Ok(());
        }
        self.lifecycle = UiResourceLifecycle::Closing;
        let connector_keys = self.connectors.keys().copied().collect::<Vec<_>>();
        let mut errors = Vec::new();
        for key in connector_keys {
            let Some((identity, _)) = self.connectors.get(&key).copied() else {
                continue;
            };
            let source = self
                .environment
                .lookup_content_source(identity.id, identity.generation);
            match source.and_then(|source| source.release_connector()) {
                Ok(()) => {
                    self.connectors.remove(&key);
                }
                Err(error) => errors.push(format!(
                    "failed to release Source membership for Connector {key:?}: {error:#}"
                )),
            }
        }

        let private_ports = self.private_sources.keys().copied().collect::<Vec<_>>();
        for port in private_ports {
            if self.connectors.contains_key(&port) {
                continue;
            }
            let Some(identity) = self.private_sources.get(&port).copied() else {
                continue;
            };
            match self
                .environment
                .lookup_content_source(identity.id, identity.generation)
                .and_then(|source| source.dispose())
            {
                Ok(()) => {
                    self.private_sources.remove(&port);
                }
                Err(error) => errors.push(format!(
                    "failed to dispose private Source for Port {port:?}: {error:#}"
                )),
            }
        }

        if errors.is_empty() {
            self.connectors.clear();
            self.literal_sources.clear();
            self.ports.clear();
            self.controls.clear();
            self.root_configs.clear();
            self.history_units.clear();
            self.document = None;
            self.lifecycle = UiResourceLifecycle::Closed;
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    fn ensure_open(&self) -> std::result::Result<(), crate::binding::UiRejection> {
        if self.lifecycle == UiResourceLifecycle::Open && self.document.is_some() {
            return Ok(());
        }
        Err(crate::binding::UiRejection::internal(
            self.document
                .as_ref()
                .map_or(0, |document| document.accepted_ui_revision()),
            crate::binding::CommitDetail::Invariant,
            "UI resource owner is closed",
        ))
    }
}

impl Drop for UiResourceOwner {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("native UI resource cleanup failed during drop: {error}");
        }
    }
}

fn source_for(
    environment: &TuiEnvironment,
    sources: &[HostContentSource],
    identity: SourceIdentity,
) -> anyhow::Result<HostContentSource> {
    if identity.environment_slot != environment.environment_slot()
        || identity.environment_generation != environment.environment_generation()
    {
        return Err(anyhow!("Source identity belongs to another environment"));
    }
    if let Some(source) = sources
        .iter()
        .find(|source| SourceIdentity::from_source(source) == identity)
    {
        return Ok(source.clone());
    }
    environment.lookup_content_source(identity.id, identity.generation)
}

fn decode_annotation_records(
    bytes: &[u8],
) -> anyhow::Result<(Vec<ContentAnnotationRecord>, Vec<u8>)> {
    if bytes.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    if bytes.len() < 4 {
        return Err(anyhow!("annotation sidecar header is truncated"));
    }
    let count = usize::try_from(u32::from_le_bytes(
        bytes[0..4]
            .try_into()
            .expect("validated four-byte annotation count"),
    ))
    .map_err(|_| anyhow!("annotation count"))?;
    let record_bytes = count
        .checked_mul(32)
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| anyhow!("annotation sidecar size"))?;
    if record_bytes > bytes.len() {
        return Err(anyhow!("annotation records exceed sidecar"));
    }
    let payload = bytes[record_bytes..].to_vec();
    let mut records = Vec::new();
    records
        .try_reserve(count)
        .map_err(|_| anyhow!("annotation record capacity"))?;
    for index in 0..count {
        let start = 4 + index * 32;
        let lane = &bytes[start..start + 32];
        let read = |offset: usize| {
            u32::from_le_bytes(
                lane[offset..offset + 4]
                    .try_into()
                    .expect("validated four-byte annotation field"),
            )
        };
        let record = ContentAnnotationRecord {
            kind: read(0),
            flags: read(4),
            start_byte: read(8),
            end_byte: read(12),
            payload_offset: read(16),
            payload_length: read(20),
            aux0: read(24),
            aux1: read(28),
        };
        let payload_end = record
            .payload_offset
            .checked_add(record.payload_length)
            .ok_or_else(|| anyhow!("annotation payload range"))?;
        if usize::try_from(payload_end).map_or(true, |end| end > payload.len()) {
            return Err(anyhow!("annotation payload range"));
        }
        records.push(record);
    }
    Ok((records, payload))
}

fn acknowledged_resource(
    acknowledgement: &crate::binding::UiAcknowledgement,
    ordinal: u32,
    kind: crate::binding::HandleKind,
    revision: u64,
) -> std::result::Result<crate::binding::ResourceKey, crate::binding::UiRejection> {
    let handle = acknowledgement
        .created
        .get(usize::try_from(ordinal.saturating_sub(1)).unwrap_or(usize::MAX))
        .copied()
        .ok_or_else(|| {
            crate::binding::UiRejection::internal(
                revision,
                crate::binding::CommitDetail::Invariant,
                "created resource ordinal is not acknowledged",
            )
        })?;
    let key = handle.resource_key().ok_or_else(|| {
        crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::WrongKind,
            "created handle is not a resource",
        )
    })?;
    if key.kind != kind {
        return Err(crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::WrongKind,
            "created resource kind is invalid",
        ));
    }
    Ok(key)
}

fn acknowledged_node(
    acknowledgement: &crate::binding::UiAcknowledgement,
    ordinal: u32,
    revision: u64,
) -> std::result::Result<crate::binding::NodeKey, crate::binding::UiRejection> {
    let handle = acknowledgement
        .created
        .get(usize::try_from(ordinal.saturating_sub(1)).unwrap_or(usize::MAX))
        .copied()
        .ok_or_else(|| {
            crate::binding::UiRejection::internal(
                revision,
                crate::binding::CommitDetail::Invariant,
                "created node ordinal is not acknowledged",
            )
        })?;
    handle.node_key().ok_or_else(|| {
        crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::WrongKind,
            "created handle is not a node",
        )
    })
}

enum UiResourceAction {
    CreatePort {
        key: crate::binding::ResourceKey,
    },
    CreateConnector {
        key: crate::binding::ResourceKey,
        source: HostContentSource,
        funnel: FunnelSpec,
    },
    ReplaceLiteral {
        port: crate::binding::ResourceKey,
        source: HostContentSource,
        funnel: FunnelSpec,
    },
    SetLiteralFunnel {
        port: crate::binding::ResourceKey,
        source: HostContentSource,
        funnel: FunnelSpec,
    },
    DisposeConnector {
        key: crate::binding::ResourceKey,
    },
    DisposePort {
        key: crate::binding::ResourceKey,
    },
    DisposeLiteral {
        port: crate::binding::ResourceKey,
    },
    InstallControl {
        key: crate::binding::ResourceKey,
        state: ControlState,
    },
    InstallRootConfig {
        key: crate::binding::NodeKey,
        config: RootConfig,
        identity_supplied: bool,
    },
    DisposeRootConfig {
        key: crate::binding::NodeKey,
    },
    DisposeControl {
        key: crate::binding::ResourceKey,
    },
}

struct CandidateSources(Vec<HostContentSource>);

impl Drop for CandidateSources {
    fn drop(&mut self) {
        for source in self.0.drain(..) {
            if let Err(error) = source.dispose() {
                eprintln!("unreleased provisional Source during rollback: {error:#}");
            }
        }
    }
}

struct SourcePlan {
    source: HostContentSource,
    delta: i64,
    replacement: Option<crate::application::content::PreparedSourceReplacement>,
    disposition: SourceInstallDisposition,
}

fn default_funnel() -> FunnelSpec {
    FunnelSpec {
        kind: 0,
        wrap: 0,
        hyperlinks: true,
        smooth: false,
    }
}

fn prepare_root_config(role: RootRole, mut config: RootConfig) -> RootConfig {
    if role == RootRole::LegacyHistoryUnit && config.unit_identity.is_none() {
        config.unit_identity = Some(HistoryUnitId::allocate().value());
    }
    config
}

fn source_for_identity(
    environment: &crate::binding::TuiEnvironment,
    sources: &[HostContentSource],
    candidates: &CandidateSources,
    identity: SourceIdentity,
) -> anyhow::Result<HostContentSource> {
    if let Some(source) = candidates
        .0
        .iter()
        .find(|candidate| SourceIdentity::from_source(candidate) == identity)
    {
        return Ok(source.clone());
    }
    source_for(environment, sources, identity)
}

fn add_source_plan_delta(
    source_plans: &mut HashMap<SourceIdentity, SourcePlan>,
    identity: SourceIdentity,
    source: HostContentSource,
    delta: i64,
    revision: u64,
) -> std::result::Result<(), crate::binding::UiRejection> {
    let plan = source_plans.entry(identity).or_insert(SourcePlan {
        source,
        delta: 0,
        replacement: None,
        disposition: SourceInstallDisposition::Keep,
    });
    plan.delta = plan.delta.checked_add(delta).ok_or_else(|| {
        crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::Capacity,
            "Source membership delta overflow",
        )
    })?;
    Ok(())
}

fn control_rejection(revision: u64, error: ControlError) -> crate::binding::UiRejection {
    let detail = match error {
        ControlError::WrongKind => crate::binding::CommitDetail::WrongKind,
        ControlError::UnknownCommand => crate::binding::CommitDetail::Unsupported,
        ControlError::InvalidOperands | ControlError::InvalidValue => {
            crate::binding::CommitDetail::Malformed
        }
        ControlError::StaleEditRevision => crate::binding::CommitDetail::StaleRevision,
        ControlError::RevisionExhausted => crate::binding::CommitDetail::Capacity,
    };
    crate::binding::UiRejection::internal(
        revision,
        detail,
        format!("control transition: {error:?}"),
    )
}

#[derive(Clone)]
struct LiteralActionPlan {
    source: HostContentSource,
    funnel: FunnelSpec,
    replacement: Option<crate::application::content::PreparedSourceReplacement>,
}

/// Sparse final binding state for only the resource keys touched by this
/// commit.  Membership changes are derived once from the accepted state to
/// this final state, rather than from each command and retirement path.
struct BindingPlan {
    entries: HashMap<crate::binding::ResourceKey, Option<SourceIdentity>>,
}

impl BindingPlan {
    fn new(
        capacity: usize,
        revision: u64,
    ) -> std::result::Result<Self, crate::binding::UiRejection> {
        let mut entries = HashMap::new();
        entries.try_reserve(capacity).map_err(|_| {
            crate::binding::UiRejection::internal(
                revision,
                crate::binding::CommitDetail::Capacity,
                "Source binding plan capacity",
            )
        })?;
        Ok(Self { entries })
    }

    fn set(&mut self, key: crate::binding::ResourceKey, source: Option<SourceIdentity>) {
        self.entries.insert(key, source);
    }
}

fn resolve_native_resource(
    document: &crate::binding::OccurrenceDocument,
    reference: &crate::binding::ResourceRef,
    acknowledgement: &crate::binding::UiAcknowledgement,
    kind: crate::binding::HandleKind,
    planned: &HashSet<crate::binding::ResourceKey>,
) -> std::result::Result<crate::binding::ResourceKey, crate::binding::UiRejection> {
    let key = match reference {
        crate::binding::ResourceRef::Existing(handle) => {
            handle.resource_key().ok_or_else(|| {
                crate::binding::UiRejection::internal(
                    document.accepted_ui_revision(),
                    crate::binding::CommitDetail::WrongKind,
                    "resource handle is invalid",
                )
            })?
        }
        crate::binding::ResourceRef::Local(ordinal) => acknowledged_resource(
            acknowledgement,
            *ordinal,
            kind,
            document.accepted_ui_revision(),
        )?,
    };
    if key.kind != kind || (!planned.contains(&key) && !document.resource_is_live(key)) {
        return Err(crate::binding::UiRejection::internal(
            document.accepted_ui_revision(),
            crate::binding::CommitDetail::StaleHandle,
            "resource is not live",
        ));
    }
    Ok(key)
}

fn validate_funnel_spec(
    spec: FunnelSpec,
    revision: u64,
) -> std::result::Result<(), crate::binding::UiRejection> {
    if spec.kind > 3 || spec.wrap > 2 {
        return Err(crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::InvalidProperty,
            "Funnel value is invalid",
        ));
    }
    Ok(())
}

fn validate_native_source(
    source: &HostContentSource,
    funnel: FunnelSpec,
    revision: u64,
) -> std::result::Result<(), crate::binding::UiRejection> {
    validate_funnel_spec(funnel, revision)?;
    if !source.is_live() || source.family() != ContentFamily::Text {
        return Err(crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::StaleHandle,
            "Source is not live",
        ));
    }
    let kind = match funnel.kind {
        0 => TextFunnelKind::Plain,
        1 => TextFunnelKind::Markdown,
        2 => TextFunnelKind::Diff,
        3 => TextFunnelKind::Ansi,
        _ => unreachable!(),
    };
    if !source.retention_compatible(kind).map_err(|error| {
        crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::InvalidTopology,
            error.to_string(),
        )
    })? {
        return Err(crate::binding::UiRejection::internal(
            revision,
            crate::binding::CommitDetail::InvalidTopology,
            "Source retention is incompatible with Funnel",
        ));
    }
    Ok(())
}

struct ResourceCommitPlan {
    revision: u64,
    actions: Vec<UiResourceAction>,
    planned_ports: HashSet<crate::binding::ResourceKey>,
    planned_connectors: HashSet<crate::binding::ResourceKey>,
    planned_controls: HashSet<crate::binding::ResourceKey>,
    control_plans: HashMap<crate::binding::ResourceKey, ControlState>,
    literal_actions: HashMap<crate::binding::ResourceKey, LiteralActionPlan>,
    candidates: CandidateSources,
    binding_plan: BindingPlan,
}

impl ResourceCommitPlan {
    fn root_config_for(&self, root: NodeKey) -> Option<RootConfig> {
        self.actions.iter().find_map(|action| match action {
            UiResourceAction::InstallRootConfig { key, config, .. } if *key == root => {
                Some(*config)
            }
            _ => None,
        })
    }

    fn supplied_history_identity(&self, root: NodeKey) -> bool {
        self.actions.iter().any(|action| {
            matches!(
                action,
                UiResourceAction::InstallRootConfig {
                    key,
                    identity_supplied: true,
                    ..
                } if *key == root
            )
        })
    }

    fn touched_resource_keys(&self) -> Vec<crate::binding::ResourceKey> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                UiResourceAction::CreatePort { key }
                | UiResourceAction::DisposePort { key }
                | UiResourceAction::CreateConnector { key, .. }
                | UiResourceAction::DisposeConnector { key }
                | UiResourceAction::InstallControl { key, .. }
                | UiResourceAction::DisposeControl { key } => Some(*key),
                UiResourceAction::ReplaceLiteral { port, .. }
                | UiResourceAction::SetLiteralFunnel { port, .. }
                | UiResourceAction::DisposeLiteral { port } => Some(*port),
                UiResourceAction::InstallRootConfig { .. }
                | UiResourceAction::DisposeRootConfig { .. } => None,
            })
            .filter(|key| key.kind != crate::binding::HandleKind::Node)
            .collect()
    }

    fn new(
        owner: &mut UiResourceOwner,
        operation_count: usize,
        retired_resource_count: usize,
        retired_node_count: usize,
        revision: u64,
    ) -> std::result::Result<Self, crate::binding::UiRejection> {
        let action_capacity = operation_count
            .saturating_add(retired_resource_count)
            .saturating_add(retired_node_count);
        let mut actions = Vec::new();
        actions.try_reserve(action_capacity).map_err(|_| {
            crate::binding::UiRejection::internal(
                revision,
                crate::binding::CommitDetail::Capacity,
                "UI resource plan capacity",
            )
        })?;
        owner
            .ports
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "UI Port capacity"))?;
        owner
            .connectors
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "UI Connector capacity"))?;
        owner
            .literal_sources
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "literal Source capacity"))?;
        owner
            .controls
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "UI control capacity"))?;
        owner
            .private_sources
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "private Source capacity"))?;
        owner
            .root_configs
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "root config capacity"))?;

        let mut planned_ports = HashSet::new();
        planned_ports
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "planned Port capacity"))?;
        let mut planned_connectors = HashSet::new();
        planned_connectors
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "planned Connector capacity"))?;
        let mut planned_controls = HashSet::new();
        planned_controls
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "planned control capacity"))?;
        let mut control_plans = HashMap::new();
        control_plans
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "control plan capacity"))?;
        let mut literal_actions = HashMap::new();
        literal_actions
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "literal action capacity"))?;
        let mut candidates = CandidateSources(Vec::new());
        candidates
            .0
            .try_reserve(operation_count)
            .map_err(|_| capacity_rejection(revision, "candidate Source capacity"))?;
        let binding_plan = BindingPlan::new(
            operation_count.saturating_add(retired_resource_count),
            revision,
        )?;
        Ok(Self {
            revision,
            actions,
            planned_ports,
            planned_connectors,
            planned_controls,
            control_plans,
            literal_actions,
            candidates,
            binding_plan,
        })
    }
}

fn capacity_rejection(revision: u64, message: &'static str) -> crate::binding::UiRejection {
    crate::binding::UiRejection::internal(revision, crate::binding::CommitDetail::Capacity, message)
}

impl ResourceCommitPlan {
    fn interpret_operations(
        &mut self,
        owner: &UiResourceOwner,
        batch: &UiCommit,
        acknowledgement: &UiAcknowledgement,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        for operation in batch.operations() {
            self.interpret_operation(
                owner,
                batch,
                acknowledgement,
                environment,
                sources,
                operation,
            )?;
        }
        Ok(())
    }

    fn interpret_operation(
        &mut self,
        owner: &UiResourceOwner,
        batch: &UiCommit,
        acknowledgement: &UiAcknowledgement,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
        operation: &UiOperation,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        match operation {
            UiOperation::CreateRoot {
                local_ordinal,
                role,
                ..
            } => self.interpret_create_root(batch, acknowledgement, *local_ordinal, *role),
            UiOperation::CreatePort { local_ordinal, .. } => {
                self.interpret_create_port(acknowledgement, *local_ordinal)
            }
            UiOperation::CreateConnector {
                local_ordinal,
                source_index,
                port,
                ..
            } => self.interpret_create_connector(
                owner,
                batch,
                acknowledgement,
                sources,
                *local_ordinal,
                *source_index,
                port,
            ),
            UiOperation::ReplaceLiteral {
                port,
                content_format,
                content,
                annotations,
            } => self.interpret_replace_literal(
                owner,
                acknowledgement,
                environment,
                sources,
                port,
                *content_format,
                content,
                annotations,
            ),
            UiOperation::SetLiteralFunnel {
                port,
                kind,
                wrap,
                hyperlinks,
                smooth,
            } => self.interpret_set_literal_funnel(
                owner,
                acknowledgement,
                environment,
                sources,
                port,
                FunnelSpec {
                    kind: *kind,
                    wrap: *wrap,
                    hyperlinks: *hyperlinks,
                    smooth: *smooth,
                },
            ),
            UiOperation::SelectConnector { port, connector } => {
                self.interpret_select_connector(owner, acknowledgement, port, connector)
            }
            UiOperation::DisposeConnector { connector } => {
                self.interpret_dispose_connector(owner, acknowledgement, connector)
            }
            UiOperation::DisposePort { port } => {
                self.interpret_dispose_port(owner, acknowledgement, port)
            }
            UiOperation::CreateControl {
                local_ordinal,
                kind,
                ..
            } => self.interpret_create_control(batch, acknowledgement, *local_ordinal, *kind),
            UiOperation::ControlCommand {
                control,
                command_id,
                operands,
            } => self.interpret_control_command(
                owner,
                acknowledgement,
                control,
                *command_id,
                operands,
            ),
            UiOperation::ReplaceEditorContent {
                control,
                content,
                expected_edit_revision,
            } => self.interpret_editor_replacement(
                owner,
                acknowledgement,
                control,
                content,
                *expected_edit_revision,
            ),
            UiOperation::DisposeControl { control } => {
                self.interpret_dispose_control(owner, acknowledgement, control)
            }
            _ => Ok(()),
        }
    }

    fn interpret_create_root(
        &mut self,
        batch: &UiCommit,
        acknowledgement: &UiAcknowledgement,
        local_ordinal: u32,
        role: RootRole,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = acknowledged_node(acknowledgement, local_ordinal, self.revision)?;
        let supplied_identity = role == RootRole::LegacyHistoryUnit
            && batch.root_config(local_ordinal).unit_identity.is_some();
        let config = prepare_root_config(role, batch.root_config(local_ordinal));
        self.actions.push(UiResourceAction::InstallRootConfig {
            key,
            config,
            identity_supplied: supplied_identity,
        });
        Ok(())
    }

    fn interpret_create_port(
        &mut self,
        acknowledgement: &UiAcknowledgement,
        local_ordinal: u32,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = acknowledged_resource(
            acknowledgement,
            local_ordinal,
            HandleKind::Port,
            self.revision,
        )?;
        self.planned_ports.insert(key);
        self.actions.push(UiResourceAction::CreatePort { key });
        Ok(())
    }

    fn interpret_create_connector(
        &mut self,
        owner: &UiResourceOwner,
        batch: &UiCommit,
        acknowledgement: &UiAcknowledgement,
        sources: &[HostContentSource],
        local_ordinal: u32,
        source_index: u32,
        port: &ResourceRef,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = acknowledged_resource(
            acknowledgement,
            local_ordinal,
            HandleKind::Connector,
            self.revision,
        )?;
        resolve_native_resource(
            owner.document_ref(),
            port,
            acknowledgement,
            HandleKind::Port,
            &self.planned_ports,
        )?;
        let source = sources
            .get(usize::try_from(source_index).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                crate::binding::UiRejection::internal(
                    self.revision,
                    crate::binding::CommitDetail::Malformed,
                    "Connector Source index is out of range",
                )
            })?
            .clone();
        let funnel = batch.funnel_for_connector(local_ordinal);
        validate_native_source(&source, funnel, self.revision)?;
        self.binding_plan
            .set(key, Some(SourceIdentity::from_source(&source)));
        self.planned_connectors.insert(key);
        self.actions.push(UiResourceAction::CreateConnector {
            key,
            source,
            funnel,
        });
        Ok(())
    }

    fn interpret_select_connector(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        port: &ResourceRef,
        connector: &Option<ResourceRef>,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        if connector.is_none() {
            return Ok(());
        }
        let port_key = resolve_native_resource(
            owner.document_ref(),
            port,
            acknowledgement,
            HandleKind::Port,
            &self.planned_ports,
        )?;
        if owner.literal_sources.contains_key(&port_key)
            && !self.binding_plan.entries.contains_key(&port_key)
        {
            self.binding_plan.set(port_key, None);
            self.actions
                .push(UiResourceAction::DisposeLiteral { port: port_key });
        }
        Ok(())
    }

    fn interpret_replace_literal(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
        port: &ResourceRef,
        content_format: u32,
        content: &[u8],
        annotations: &[u8],
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        if content_format != 1 {
            return Err(crate::binding::UiRejection::internal(
                self.revision,
                crate::binding::CommitDetail::Malformed,
                "literal replacement requires format 1",
            ));
        }
        let port = resolve_native_resource(
            owner.document_ref(),
            port,
            acknowledgement,
            HandleKind::Port,
            &self.planned_ports,
        )?;
        let (source, funnel) = self.literal_source_for(owner, environment, sources, port)?;
        let (records, payload) = decode_annotation_records(annotations).map_err(|error| {
            crate::binding::UiRejection::internal(
                self.revision,
                crate::binding::CommitDetail::Malformed,
                error.to_string(),
            )
        })?;
        let replacement =
            HostContentSource::validate_replacement(content.to_vec(), &records, &payload).map_err(
                |error| {
                    crate::binding::UiRejection::internal(
                        self.revision,
                        crate::binding::CommitDetail::Malformed,
                        error.to_string(),
                    )
                },
            )?;
        validate_native_source(&source, funnel, self.revision)?;
        self.binding_plan
            .set(port, Some(SourceIdentity::from_source(&source)));
        self.literal_actions.insert(
            port,
            LiteralActionPlan {
                source,
                funnel,
                replacement: Some(replacement),
            },
        );
        Ok(())
    }

    fn literal_source_for(
        &mut self,
        owner: &UiResourceOwner,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
        port: ResourceKey,
    ) -> std::result::Result<(HostContentSource, FunnelSpec), crate::binding::UiRejection> {
        if let Some(literal_plan) = self.literal_actions.get(&port) {
            return Ok((literal_plan.source.clone(), literal_plan.funnel));
        }
        let source = if let Some(identity) = owner.literal_sources.get(&port) {
            source_for_identity(environment, sources, &self.candidates, *identity).map_err(
                |error| {
                    crate::binding::UiRejection::internal(
                        self.revision,
                        crate::binding::CommitDetail::StaleHandle,
                        error.to_string(),
                    )
                },
            )?
        } else {
            let source = environment
                .create_content_source(TextSourceKind::Stream)
                .map_err(|error| {
                    crate::binding::UiRejection::internal(
                        self.revision,
                        crate::binding::CommitDetail::Capacity,
                        error.to_string(),
                    )
                })?;
            self.candidates.0.push(source.clone());
            source
        };
        let funnel = owner
            .connectors
            .get(&port)
            .map_or_else(default_funnel, |(_, funnel)| *funnel);
        Ok((source, funnel))
    }

    fn interpret_set_literal_funnel(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
        port: &ResourceRef,
        funnel: FunnelSpec,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let port = resolve_native_resource(
            owner.document_ref(),
            port,
            acknowledgement,
            HandleKind::Port,
            &self.planned_ports,
        )?;
        validate_funnel_spec(funnel, self.revision)?;
        if let Some(literal_plan) = self.literal_actions.get_mut(&port) {
            literal_plan.funnel = funnel;
            return Ok(());
        }
        let (source, _) = self.literal_source_for(owner, environment, sources, port)?;
        self.binding_plan
            .set(port, Some(SourceIdentity::from_source(&source)));
        self.literal_actions.insert(
            port,
            LiteralActionPlan {
                source,
                funnel,
                replacement: None,
            },
        );
        Ok(())
    }

    fn interpret_dispose_connector(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        connector: &ResourceRef,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = resolve_native_resource(
            owner.document_ref(),
            connector,
            acknowledgement,
            HandleKind::Connector,
            &self.planned_connectors,
        )?;
        if !self.planned_connectors.contains(&key) && !owner.connectors.contains_key(&key) {
            return Err(crate::binding::UiRejection::internal(
                self.revision,
                crate::binding::CommitDetail::StaleHandle,
                "Connector is not installed",
            ));
        }
        self.binding_plan.set(key, None);
        Ok(())
    }

    fn interpret_dispose_port(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        port: &ResourceRef,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = resolve_native_resource(
            owner.document_ref(),
            port,
            acknowledgement,
            HandleKind::Port,
            &self.planned_ports,
        )?;
        self.binding_plan.set(key, None);
        Ok(())
    }

    fn interpret_create_control(
        &mut self,
        batch: &UiCommit,
        acknowledgement: &UiAcknowledgement,
        local_ordinal: u32,
        kind: crate::binding::ControlKind,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = acknowledged_resource(
            acknowledgement,
            local_ordinal,
            HandleKind::Control,
            self.revision,
        )?;
        let config = batch.control_config(local_ordinal);
        self.planned_controls.insert(key);
        self.control_plans
            .insert(key, ControlState::new_with_config(kind, config));
        Ok(())
    }

    fn control_plan_mut(
        &mut self,
        owner: &UiResourceOwner,
        key: ResourceKey,
    ) -> std::result::Result<&mut ControlState, crate::binding::UiRejection> {
        if !self.control_plans.contains_key(&key) {
            let state = owner.controls.get(&key).cloned().ok_or_else(|| {
                crate::binding::UiRejection::internal(
                    self.revision,
                    crate::binding::CommitDetail::Invariant,
                    "live Control has no concrete native state",
                )
            })?;
            self.control_plans.insert(key, state);
        }
        self.control_plans.get_mut(&key).ok_or_else(|| {
            crate::binding::UiRejection::internal(
                self.revision,
                crate::binding::CommitDetail::Invariant,
                "control plan was not retained",
            )
        })
    }

    fn interpret_control_command(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        control: &ResourceRef,
        command_id: u32,
        operands: &[u32],
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = resolve_native_resource(
            owner.document_ref(),
            control,
            acknowledgement,
            HandleKind::Control,
            &self.planned_controls,
        )?;
        let state = self.control_plan_mut(owner, key)?;
        state
            .apply_command(command_id, operands)
            .map_err(|error| control_rejection(self.revision, error))
    }

    fn interpret_editor_replacement(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        control: &ResourceRef,
        content: &[u8],
        expected_edit_revision: u64,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let key = resolve_native_resource(
            owner.document_ref(),
            control,
            acknowledgement,
            HandleKind::Control,
            &self.planned_controls,
        )?;
        let state = self.control_plan_mut(owner, key)?;
        state
            .replace_editor(content, expected_edit_revision)
            .map_err(|error| control_rejection(self.revision, error))
    }

    fn interpret_dispose_control(
        &mut self,
        owner: &UiResourceOwner,
        acknowledgement: &UiAcknowledgement,
        control: &ResourceRef,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        resolve_native_resource(
            owner.document_ref(),
            control,
            acknowledgement,
            HandleKind::Control,
            &self.planned_controls,
        )?;
        Ok(())
    }
}

impl ResourceCommitPlan {
    fn prepare_source_mutations(
        &mut self,
        owner: &UiResourceOwner,
        prepared: &crate::occurrence::commit::PreparedUiCommit,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
    ) -> std::result::Result<
        Vec<crate::application::content::PreparedSourceMutation>,
        crate::binding::UiRejection,
    > {
        self.plan_retired_resources(owner, prepared)?;
        self.plan_control_installations();
        let mut source_plans = self.build_source_plans(owner, environment, sources)?;
        self.plan_literal_mutations(&mut source_plans);
        self.dispose_unaccepted_candidates()?;
        self.materialize_source_mutations(source_plans)
    }

    fn plan_retired_resources(
        &mut self,
        owner: &UiResourceOwner,
        prepared: &crate::occurrence::commit::PreparedUiCommit,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        for key in prepared.retired_resource_keys() {
            match key.kind {
                HandleKind::Connector => {
                    if !owner.connectors.contains_key(&key) {
                        return Err(crate::binding::UiRejection::internal(
                            self.revision,
                            crate::binding::CommitDetail::StaleHandle,
                            "Connector is not installed",
                        ));
                    }
                    self.binding_plan.set(key, None);
                    self.actions
                        .push(UiResourceAction::DisposeConnector { key });
                }
                HandleKind::Port => {
                    self.binding_plan.set(key, None);
                    self.actions.push(UiResourceAction::DisposePort { key });
                }
                HandleKind::Control => {
                    self.control_plans.remove(&key);
                    self.actions.push(UiResourceAction::DisposeControl { key });
                }
                HandleKind::Node => {}
            }
        }
        for key in prepared.retired_node_keys() {
            self.actions
                .push(UiResourceAction::DisposeRootConfig { key });
        }
        Ok(())
    }

    fn plan_control_installations(&mut self) {
        let mut control_plans = std::mem::take(&mut self.control_plans)
            .into_iter()
            .collect::<Vec<_>>();
        control_plans.sort_unstable_by_key(|(key, _)| (key.slot, key.generation));
        for (key, state) in control_plans {
            self.actions
                .push(UiResourceAction::InstallControl { key, state });
        }
    }

    fn build_source_plans(
        &self,
        owner: &UiResourceOwner,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
    ) -> std::result::Result<HashMap<SourceIdentity, SourcePlan>, crate::binding::UiRejection> {
        let mut source_plans = HashMap::new();
        source_plans
            .try_reserve(self.binding_plan.entries.len())
            .map_err(|_| capacity_rejection(self.revision, "Source transaction capacity"))?;

        for (key, after) in &self.binding_plan.entries {
            self.plan_private_source_disposal(
                owner,
                environment,
                sources,
                *key,
                *after,
                &mut source_plans,
            )?;
            let before = owner.connectors.get(key).map(|(source, _)| *source);
            if before == *after {
                continue;
            }
            if let Some(identity) = before {
                let source = source_for_identity(environment, sources, &self.candidates, identity)
                    .map_err(|error| self.source_rejection(error))?;
                add_source_plan_delta(&mut source_plans, identity, source, -1, self.revision)?;
            }
            if let Some(identity) = after {
                let source = source_for_identity(environment, sources, &self.candidates, *identity)
                    .map_err(|error| self.source_rejection(error))?;
                add_source_plan_delta(&mut source_plans, *identity, source, 1, self.revision)?;
            }
        }
        Ok(source_plans)
    }

    fn plan_private_source_disposal(
        &self,
        owner: &UiResourceOwner,
        environment: &TuiEnvironment,
        sources: &[HostContentSource],
        key: ResourceKey,
        after: Option<SourceIdentity>,
        source_plans: &mut HashMap<SourceIdentity, SourcePlan>,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        if key.kind != HandleKind::Port || after.is_some() {
            return Ok(());
        }
        let Some(identity) = owner.private_sources.get(&key).copied() else {
            return Ok(());
        };
        let source = source_for_identity(environment, sources, &self.candidates, identity)
            .map_err(|error| self.source_rejection(error))?;
        let plan = source_plans.entry(identity).or_insert(SourcePlan {
            source,
            delta: 0,
            replacement: None,
            disposition: SourceInstallDisposition::Keep,
        });
        plan.disposition = SourceInstallDisposition::DisposeWhenEmpty;
        Ok(())
    }

    fn plan_literal_mutations(&mut self, source_plans: &mut HashMap<SourceIdentity, SourcePlan>) {
        // Commands were validated sequentially, but only the final replacement
        // for each literal Source needs to reach the guarded Source install.
        let mut literal_actions = std::mem::take(&mut self.literal_actions)
            .into_iter()
            .collect::<Vec<_>>();
        literal_actions.sort_unstable_by_key(|(key, _)| (key.slot, key.generation));

        for (port, literal_plan) in literal_actions {
            if self
                .binding_plan
                .entries
                .get(&port)
                .is_some_and(Option::is_none)
            {
                continue;
            }
            let source_identity = SourceIdentity::from_source(&literal_plan.source);
            if let Some(replacement) = literal_plan.replacement {
                let source_plan = source_plans.entry(source_identity).or_insert(SourcePlan {
                    source: literal_plan.source.clone(),
                    delta: 0,
                    replacement: None,
                    disposition: SourceInstallDisposition::Keep,
                });
                source_plan.replacement = Some(replacement);
                self.actions.push(UiResourceAction::ReplaceLiteral {
                    port,
                    source: literal_plan.source,
                    funnel: literal_plan.funnel,
                });
            } else {
                self.actions.push(UiResourceAction::SetLiteralFunnel {
                    port,
                    source: literal_plan.source,
                    funnel: literal_plan.funnel,
                });
            }
        }
    }

    fn dispose_unaccepted_candidates(
        &mut self,
    ) -> std::result::Result<(), crate::binding::UiRejection> {
        let mut accepted = HashSet::new();
        accepted
            .try_reserve(self.binding_plan.entries.len())
            .map_err(|_| capacity_rejection(self.revision, "candidate Source identity capacity"))?;
        accepted.extend(self.binding_plan.entries.values().flatten().copied());

        for candidate in &self.candidates.0 {
            if !accepted.contains(&SourceIdentity::from_source(candidate)) {
                candidate.dispose().map_err(|error| {
                    crate::binding::UiRejection::internal(
                        self.revision,
                        crate::binding::CommitDetail::InvalidTopology,
                        error.to_string(),
                    )
                })?;
            }
        }
        self.candidates
            .0
            .retain(|candidate| accepted.contains(&SourceIdentity::from_source(candidate)));
        Ok(())
    }

    fn materialize_source_mutations(
        &self,
        source_plans: HashMap<SourceIdentity, SourcePlan>,
    ) -> std::result::Result<
        Vec<crate::application::content::PreparedSourceMutation>,
        crate::binding::UiRejection,
    > {
        let mut mutations = Vec::new();
        mutations
            .try_reserve(source_plans.len())
            .map_err(|_| capacity_rejection(self.revision, "Source mutation capacity"))?;
        for source_plan in source_plans.into_values() {
            mutations.push(self.prepare_source_mutation(source_plan)?);
        }
        Ok(mutations)
    }

    fn prepare_source_mutation(
        &self,
        source_plan: SourcePlan,
    ) -> std::result::Result<
        crate::application::content::PreparedSourceMutation,
        crate::binding::UiRejection,
    > {
        let mutation = if let Some(replacement) = source_plan.replacement {
            source_plan
                .source
                .prepare_validated_replacement_with_membership(
                    replacement,
                    source_plan.delta,
                    source_plan.disposition,
                )
        } else {
            source_plan
                .source
                .prepare_membership_delta(source_plan.delta, source_plan.disposition)
        };
        mutation.map_err(|error| self.source_rejection(error))
    }

    fn source_rejection(&self, error: anyhow::Error) -> crate::binding::UiRejection {
        crate::binding::UiRejection::internal(
            self.revision,
            crate::binding::CommitDetail::InvalidTopology,
            error.to_string(),
        )
    }
}

impl UiResourceOwner {
    pub(crate) fn prepare_commit(
        &mut self,
        batch: UiCommit,
        sources: &[HostContentSource],
        existing_history: &HashMap<HistoryUnitId, bool>,
    ) -> std::result::Result<PreparedUiResources, crate::binding::UiRejection> {
        self.ensure_open()?;
        // The three phases are intentionally visible: the document validates
        // topology, this owner prepares concrete resource actions and Source
        // mutations, then installation performs the only fallible write step.
        let prepared = self.document_mut().prepare_ui_commit(&batch)?;
        let acknowledgement = prepared.acknowledgement().clone();
        let mut changes = prepared.change_set();
        let revision = self.document_ref().accepted_ui_revision();
        let mut plan = ResourceCommitPlan::new(
            self,
            batch.operations().len(),
            prepared.retired_resource_keys().len(),
            prepared.retired_node_keys().len(),
            revision,
        )?;
        let environment = self.environment.clone();
        plan.interpret_operations(self, &batch, &acknowledgement, &environment, sources)?;
        let history_updates = self.prepare_history_updates(&prepared, &plan, existing_history)?;
        let source_mutations =
            plan.prepare_source_mutations(self, &prepared, &environment, sources)?;
        changes
            .changed_resources
            .extend(plan.touched_resource_keys());
        changes
            .changed_resources
            .sort_unstable_by_key(|key| (key.kind as u32, key.slot, key.generation));
        changes.changed_resources.dedup();
        changes.physical_work |= !changes.changed_resources.is_empty();
        Ok(PreparedUiResources {
            prepared,
            plan,
            source_mutations,
            changes,
            history_updates,
        })
    }

    pub(crate) fn apply_prepared_commit(
        &mut self,
        prepared: PreparedUiResources,
    ) -> std::result::Result<UiCommitOutput, crate::binding::UiRejection> {
        let PreparedUiResources {
            prepared,
            mut plan,
            source_mutations,
            changes,
            history_updates,
        } = prepared;
        let wakes = HostContentSource::install_prepared_mutations(source_mutations)
            .map_err(|error| plan.source_rejection(error))?;

        // Both owners are now applied without fallible operations. Wake
        // scheduling is deliberately last, after the UI document is accepted.
        let result = self
            .document
            .as_mut()
            .expect("open UI owner retains document")
            .apply_prepared_ui_commit(prepared);
        self.apply_actions(plan.actions);
        self.apply_history_updates(history_updates);
        // Accepted candidate ownership has moved into the private Source
        // map through the final literal action. The local guard can release
        // its clone without scanning all bindings.
        plan.candidates.0.clear();
        Ok(UiCommitOutput {
            result,
            wakes,
            changes,
        })
    }

    pub fn commit(
        &mut self,
        batch: UiCommit,
        sources: &[HostContentSource],
        existing_history: &HashMap<HistoryUnitId, bool>,
    ) -> std::result::Result<UiCommitOutput, crate::binding::UiRejection> {
        let prepared = self.prepare_commit(batch, sources, existing_history)?;
        self.apply_prepared_commit(prepared)
    }

    fn apply_actions(&mut self, actions: Vec<UiResourceAction>) {
        for action in actions {
            match action {
                UiResourceAction::CreatePort { key } => {
                    self.ports.insert(key);
                }
                UiResourceAction::CreateConnector {
                    key,
                    source,
                    funnel,
                } => {
                    let identity = SourceIdentity::from_source(&source);
                    self.connectors.insert(key, (identity, funnel));
                }
                UiResourceAction::ReplaceLiteral {
                    port,
                    source,
                    funnel,
                } => {
                    let identity = SourceIdentity::from_source(&source);
                    self.literal_sources.insert(port, identity);
                    self.private_sources.insert(port, identity);
                    self.connectors.insert(port, (identity, funnel));
                }
                UiResourceAction::SetLiteralFunnel {
                    port,
                    source,
                    funnel,
                } => {
                    let identity = SourceIdentity::from_source(&source);
                    self.literal_sources.insert(port, identity);
                    self.private_sources.insert(port, identity);
                    self.connectors.insert(port, (identity, funnel));
                }
                UiResourceAction::DisposeConnector { key } => {
                    self.connectors.remove(&key);
                }
                UiResourceAction::DisposePort { key } => {
                    self.connectors.remove(&key);
                    self.literal_sources.remove(&key);
                    self.private_sources.remove(&key);
                    self.ports.remove(&key);
                }
                UiResourceAction::DisposeLiteral { port } => {
                    self.connectors.remove(&port);
                    self.literal_sources.remove(&port);
                    self.private_sources.remove(&port);
                }
                UiResourceAction::InstallControl { key, state } => {
                    self.controls.insert(key, state);
                }
                UiResourceAction::InstallRootConfig { key, config, .. } => {
                    self.root_configs.insert(key, config);
                }
                UiResourceAction::DisposeRootConfig { key } => {
                    self.root_configs.remove(&key);
                }
                UiResourceAction::DisposeControl { key } => {
                    self.controls.remove(&key);
                }
            }
        }
    }
}
