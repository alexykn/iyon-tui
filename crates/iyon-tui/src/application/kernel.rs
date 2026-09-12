//! Native runtime owner for accepted occurrences and native control state.
//!
//! The runtime keeps interaction, scheduling, content ownership, and the
//! physical History frontier. General UI layout is always delegated to the
//! occurrence/Taffy route; no second semantic UI shell is retained.

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};

use anyhow::Result;

use crate::{
    ComponentHandle, HistoryUnitId, InteractionResult, OutputRouter,
    component::ComponentRegistry,
    history::{FlowBoundary, History},
    output::OutputDispatchError,
    presentation::ContentProvider,
    scene::{PreparedSceneFrame, SceneHost},
};

use super::{
    host::RoutedOutput,
    input::{GlobalBindings, PasteInterceptors},
    ui_resources::HistoryUnitStatus as UiHistoryUnitStatus,
};

const OUTPUT_BATCH_BUDGET: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadyStatus {
    pub(crate) dirty: bool,
    pub(crate) exiting: bool,
    pub(crate) more_ready: bool,
    pub(crate) changed_components: Vec<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PasteDispatchOutcome {
    Disabled,
    Intercepted(RoutedOutput),
    Local,
}

pub(crate) struct NativeRuntime {
    history: History,
    theme: Arc<crate::Theme>,
    components: ComponentRegistry,
    outputs: OutputRouter<RoutedOutput>,
    scene_host: SceneHost,
    pending_outputs: VecDeque<RoutedOutput>,
    global_bindings: GlobalBindings,
    paste_interceptors: PasteInterceptors,
    ui_history_units: HashMap<crate::occurrence::NodeKey, UiHistoryBinding>,
    pending_component_retirements: Vec<u64>,
    deferred_pastes: VecDeque<String>,
    routed_outputs: VecDeque<RoutedOutput>,
    dirty: bool,
    exit_requested: bool,
}

#[derive(Clone, Copy)]
struct UiHistoryBinding {
    id: HistoryUnitId,
    status: UiHistoryUnitStatus,
    native_transfer_allowed: bool,
}

pub(crate) struct HistoryUnitRecipe {
    pub(crate) root: crate::occurrence::NodeKey,
    pub(crate) port_id: u64,
    pub(crate) padding: crate::Insets,
    pub(crate) native_transfer_allowed: bool,
    pub(crate) unit_identity: HistoryUnitId,
    pub(crate) status: UiHistoryUnitStatus,
    pub(crate) flow_boundary: FlowBoundary,
}

impl NativeRuntime {
    pub(crate) fn host_register<C: crate::Component>(
        &mut self,
        component: C,
    ) -> ComponentHandle<C> {
        self.components.register(component)
    }
    pub(crate) fn host_bind_key(
        &mut self,
        key: crate::KeyStroke,
        factory: impl Fn() -> RoutedOutput + Send + 'static,
    ) {
        self.global_bindings.bind(key, factory);
    }
    pub(crate) fn host_route<T: Send + 'static>(
        &mut self,
        output: crate::Output<T>,
        map: impl Fn(T) -> RoutedOutput + Send + 'static,
    ) -> Result<(), crate::RouteConflict> {
        self.outputs.route(output, map)
    }
    pub(crate) fn host_intercept_paste<C: crate::Component>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> RoutedOutput + Send + 'static,
    ) {
        self.paste_interceptors.intercept(component, map);
    }
    pub(crate) fn host_forward_paste(&mut self, text: String) -> Result<(), OutputDispatchError> {
        self.deferred_pastes.push_back(text);
        self.drain_deferred_pastes()
    }

    pub(crate) fn host_retire_component(&mut self, raw_id: u64) {
        self.pending_component_retirements.push(raw_id);
        self.reap_retired_components();
    }
    pub(crate) fn reap_retired_components(&mut self) {
        if self.pending_component_retirements.is_empty() {
            return;
        }
        let mut pending = Vec::new();
        for raw_id in self.pending_component_retirements.drain(..) {
            let id = crate::component::ComponentId::from_raw(raw_id);
            if self.scene_host.is_mounted(id) {
                pending.push(raw_id);
            } else {
                self.components.remove_id(id);
                self.paste_interceptors.remove_id(id);
            }
        }
        self.pending_component_retirements = pending;
    }

    pub(crate) fn host_invalidate_component(&mut self, id: u64) -> anyhow::Result<()> {
        let id = crate::component::ComponentId::from_raw(id);
        self.scene_host.invalidate_direct_control_measurement(id)?;
        self.components.invalidate(id);
        self.scene_host.invalidate_component(id);
        self.invalidate_frame();
        Ok(())
    }
    pub(crate) fn host_invalidate_content(
        &mut self,
        dirty: crate::presentation::ContentDirty,
    ) -> anyhow::Result<()> {
        self.scene_host
            .invalidate_direct_content_measurement(dirty.port_id)?;
        self.invalidate_frame();
        Ok(())
    }
    pub(crate) fn host_set_direct_control_component(
        &mut self,
        key: crate::occurrence::ResourceKey,
        component: u64,
    ) {
        self.scene_host
            .set_direct_control_component(key, crate::component::ComponentId::from_raw(component));
    }
    pub(crate) fn host_set_direct_driver_id(&mut self, driver_id: u64) -> anyhow::Result<()> {
        self.scene_host.set_direct_driver_id(driver_id)
    }
    pub(crate) fn host_clear_direct_driver(&mut self) -> anyhow::Result<()> {
        self.scene_host.clear_direct_driver()
    }

    pub(crate) fn host_take_direct_driver(
        &mut self,
    ) -> Option<crate::presentation::direct::DirectDriverHandle> {
        self.scene_host.take_direct_driver()
    }

    pub(crate) fn set_async_wake(&mut self, wake: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.scene_host.set_async_wake(wake);
    }
    pub(crate) fn host_remove_direct_control_component(
        &mut self,
        key: crate::occurrence::ResourceKey,
    ) {
        self.scene_host.remove_direct_control_component(key);
    }
    pub(crate) fn host_sync_direct_occurrences(
        &mut self,
        sync_revision: u64,
        snapshots: Vec<crate::occurrence::OccurrenceSnapshot>,
        changes: Option<&crate::occurrence::UiChangeSet>,
        participation: &[crate::presentation::taffy::NodeParticipation],
        port_ids: HashMap<crate::occurrence::ResourceKey, u64>,
        roots: Vec<crate::occurrence::NodeKey>,
        body_root: crate::occurrence::NodeKey,
        portal_owners: HashMap<crate::occurrence::NodeKey, crate::occurrence::NodeKey>,
    ) -> anyhow::Result<()> {
        self.scene_host.sync_direct_occurrences(
            sync_revision,
            snapshots,
            changes,
            participation,
            port_ids,
            roots,
            body_root,
            portal_owners,
        )
    }

    pub(crate) fn host_direct_control_for_component(
        &self,
        component: u64,
    ) -> Option<crate::occurrence::ResourceKey> {
        self.scene_host
            .direct_control_for_component(crate::component::ComponentId::from_raw(component))
    }
    pub(crate) fn host_direct_component_for_control(
        &self,
        control: crate::occurrence::ResourceKey,
    ) -> Option<crate::component::ComponentId> {
        self.scene_host.direct_component_for_control(control)
    }
    pub(crate) fn host_direct_body_root(&self) -> Option<crate::occurrence::NodeKey> {
        self.scene_host.direct_body_root()
    }
    pub(crate) fn host_has_direct_occurrences(&self) -> bool {
        self.scene_host.has_direct_occurrences()
    }
    pub(crate) fn host_direct_history_overflow_rows(&self) -> usize {
        self.scene_host.direct_history_overflow_rows()
    }
    pub(crate) fn host_direct_port_ids(&self) -> HashMap<crate::occurrence::ResourceKey, u64> {
        self.scene_host.direct_port_ids().clone()
    }
    pub(crate) fn host_native_history_anchored(&self) -> bool {
        self.history.native_has_physical_rows()
    }
    pub(crate) fn host_native_history_blocked(&self) -> bool {
        self.history.native_transfer_semantically_blocked_front()
    }
    pub(crate) fn host_native_history_front_content_port(&self) -> Option<u64> {
        self.history.front_content_attachment_id()
    }
    pub(crate) fn host_ui_history_exported(&self, root: crate::occurrence::NodeKey) -> bool {
        self.ui_history_units
            .get(&root)
            .is_some_and(|binding| !self.history.contains_unit(binding.id))
    }
    pub(crate) fn host_native_history_synchronization_unknown(&self) -> bool {
        self.history.native_synchronization_unknown()
    }
    pub(crate) fn host_mark_native_history_synchronization_unknown(&mut self) {
        self.history.mark_native_synchronization_unknown();
    }
    pub(crate) fn host_has_invalidated_components(&self) -> bool {
        self.scene_host.has_invalidated_components()
    }
    pub(crate) fn host_focus_component(
        &mut self,
        id: u64,
        geometry: &crate::presentation::direct_tree::ComponentGeometryMap,
    ) -> bool {
        let focused = self.scene_host.focus_component(
            crate::component::ComponentId::from_raw(id),
            geometry,
            &mut self.components,
        );
        if focused {
            self.invalidate_frame();
        }
        focused
    }
    pub(crate) fn host_focused_component(&self) -> Option<u64> {
        self.scene_host.focused_component().map(|id| id.value())
    }

    pub(crate) fn host_content_candidate_epoch(&self) -> u64 {
        self.scene_host.content_candidate_epoch().unwrap_or(0)
    }
    pub(crate) fn host_commit_content_candidate(&mut self, epoch: u64) {
        self.scene_host.commit_content_candidate(epoch);
    }
    pub(crate) fn host_abort_content_candidate(&mut self) {
        self.scene_host.abort_content_candidate();
    }
    pub(crate) fn host_discard_candidate(&mut self) {
        self.invalidate_frame();
    }

    pub(crate) fn host_sync_ui_history(
        &mut self,
        units: Vec<HistoryUnitRecipe>,
        changes: Option<&crate::occurrence::UiChangeSet>,
        content: &mut crate::application::content::ContentHostRegistry,
    ) -> anyhow::Result<()> {
        for unit in units {
            let binding = UiHistoryBinding {
                id: unit.unit_identity,
                status: unit.status,
                native_transfer_allowed: unit.native_transfer_allowed,
            };
            let previous = self.ui_history_units.get(&unit.root).copied();
            let present = self.history.unit_is_live(binding.id);
            match (previous, present) {
                (None, None) => self.history.push_content_with_identity(
                    unit.unit_identity,
                    unit.port_id,
                    unit.padding,
                    unit.flow_boundary,
                    binding.status == UiHistoryUnitStatus::Live,
                    !binding.native_transfer_allowed,
                )?,
                (Some(old), Some(_))
                    if old.status == UiHistoryUnitStatus::Live
                        && binding.status == UiHistoryUnitStatus::Live =>
                {
                    self.history.replace_content(
                        binding.id,
                        unit.port_id,
                        unit.padding,
                        !binding.native_transfer_allowed,
                    )?
                }
                (Some(old), Some(_))
                    if old.status == UiHistoryUnitStatus::Live
                        && binding.status == UiHistoryUnitStatus::Frozen =>
                {
                    self.history.freeze_content(
                        binding.id,
                        unit.port_id,
                        unit.padding,
                        !binding.native_transfer_allowed,
                    )?
                }
                (Some(_), Some(_)) => self
                    .history
                    .set_native_transfer_blocked(binding.id, !binding.native_transfer_allowed),
                (Some(old), None) if old.status == UiHistoryUnitStatus::Frozen => {}
                (Some(_), None) => {
                    return Err(anyhow::anyhow!("accepted live History unit disappeared"));
                }
                (None, Some(_)) => self
                    .history
                    .set_native_transfer_blocked(binding.id, !binding.native_transfer_allowed),
            }
            if binding.status == UiHistoryUnitStatus::Frozen && unit.port_id != 0 {
                content.set_history_unit(unit.port_id, binding.id.value(), unit.padding)?;
            }
            self.history
                .set_native_transfer_blocked(binding.id, !binding.native_transfer_allowed);
            self.ui_history_units.insert(unit.root, binding);
        }
        if let Some(changes) = changes {
            for root in &changes.retired_nodes {
                if let Some(binding) = self.ui_history_units.remove(root)
                    && self.history.contains_unit(binding.id)
                {
                    content.clear_history_unit(binding.id.value());
                    self.history.retire_unit(binding.id)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn host_exited(&self) -> bool {
        self.exit_requested
    }

    pub(crate) fn input_disabled(&self) -> bool {
        self.exit_requested
    }

    pub(crate) fn host_set_theme(&mut self, theme: crate::Theme) {
        self.theme = Arc::new(theme);
        self.invalidate_frame();
    }
    pub(crate) fn host_exit(&mut self) {
        self.exit_requested = true;
        self.pending_outputs.clear();
        self.deferred_pastes.clear();
        self.dirty = true;
    }
    pub(crate) fn scene_history(&self) -> Option<&History> {
        Some(&self.history)
    }
    pub(crate) fn scene_history_mut(&mut self) -> Option<&mut History> {
        Some(&mut self.history)
    }
    pub(crate) fn new() -> Self {
        Self {
            history: History::new(),
            theme: Arc::new(crate::Theme::new()),
            components: ComponentRegistry::new(),
            outputs: OutputRouter::new(),
            scene_host: SceneHost::default(),
            pending_outputs: VecDeque::new(),
            global_bindings: GlobalBindings::default(),
            paste_interceptors: PasteInterceptors::default(),
            ui_history_units: HashMap::new(),
            pending_component_retirements: Vec::new(),
            deferred_pastes: VecDeque::new(),
            routed_outputs: VecDeque::new(),
            dirty: true,
            exit_requested: false,
        }
    }

    pub(crate) fn dispatch_key(
        &mut self,
        key: crate::KeyStroke,
    ) -> Result<InteractionResult, OutputDispatchError> {
        if self.exit_requested {
            return Ok(InteractionResult::Ignored);
        }
        if let Some(output) = self.global_bindings.output(key) {
            self.pending_outputs.push_back(output);
            return Ok(InteractionResult::Consumed);
        }
        let previous = self.scene_host.focused_component();
        let result = self
            .scene_host
            .dispatch_key_local(key, &mut self.components);
        let next = self.scene_host.focused_component();
        self.drain_outputs_to_pending()?;
        if result == InteractionResult::Consumed {
            self.invalidate_interaction_components(previous, next);
            self.dirty = true;
        }
        Ok(result)
    }
    pub(crate) fn prepare_paste_route(&mut self, text: &str) -> PasteDispatchOutcome {
        if self.exit_requested {
            return PasteDispatchOutcome::Disabled;
        }
        self.intercept_paste(text).map_or(
            PasteDispatchOutcome::Local,
            PasteDispatchOutcome::Intercepted,
        )
    }
    pub(crate) fn intercept_paste(&mut self, text: &str) -> Option<RoutedOutput> {
        self.scene_host.intercept_paste(text, |component, _| {
            self.paste_interceptors.output(component, text)
        })
    }
    pub(crate) fn queue_intercepted_paste(&mut self, output: RoutedOutput) {
        self.pending_outputs.push_back(output);
    }
    pub(crate) fn dispatch_paste_local(
        &mut self,
        text: &str,
    ) -> Result<InteractionResult, OutputDispatchError> {
        if self.exit_requested {
            return Ok(InteractionResult::Ignored);
        }
        let previous = self.scene_host.focused_component();
        let result = self.scene_host.dispatch_paste(text, &mut self.components);
        let next = self.scene_host.focused_component();
        self.drain_outputs_to_pending()?;
        if result == InteractionResult::Consumed {
            self.invalidate_interaction_components(previous, next);
            self.dirty = true;
        }
        Ok(result)
    }
    pub(crate) fn advance_ready(
        &mut self,
        now: Instant,
    ) -> Result<ReadyStatus, OutputDispatchError> {
        if self.exit_requested {
            self.pending_outputs.clear();
            self.deferred_pastes.clear();
            return Ok(self.status(false, Vec::new()));
        }
        let tick = self.scene_host.tick_due(now, &mut self.components);
        self.dirty |= tick.dirty;
        self.drain_outputs_to_pending()?;
        for _ in 0..OUTPUT_BATCH_BUDGET {
            let Some(output) = self.pending_outputs.pop_front() else {
                break;
            };
            self.routed_outputs.push_back(output);
            self.dirty = true;
        }
        Ok(self.status(
            !self.pending_outputs.is_empty(),
            tick.changed_components
                .into_iter()
                .map(|id| id.value())
                .collect(),
        ))
    }
    pub(crate) fn has_pending_outputs(&self) -> bool {
        !self.pending_outputs.is_empty()
    }
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.scene_host.next_tick_deadline()
    }
    pub(crate) fn theme_shared(&self) -> &Arc<crate::Theme> {
        &self.theme
    }

    #[cfg(test)]
    pub(crate) fn scene_host(&self) -> &crate::scene::SceneHost {
        &self.scene_host
    }

    pub(crate) fn prepare_frame_for_history(
        &mut self,
        now: Instant,
        size: crate::geometry::Size,
        content: &mut dyn ContentProvider,
        direct_root: Option<crate::occurrence::NodeKey>,
        _direct_port_ids: &HashMap<crate::occurrence::ResourceKey, u64>,
        anchor: crate::presentation::direct::DirectHistoryAnchor,
        async_wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> anyhow::Result<(
        PreparedSceneFrame,
        Option<crate::history::NativeTransferPlan>,
    )> {
        content.set_theme(&self.theme);
        self.scene_host.set_async_wake(async_wake);
        let root = direct_root.ok_or_else(|| anyhow::anyhow!("direct Body root is unavailable"))?;
        let frame = self.scene_host.prepare_direct_at_with_content(
            now,
            root,
            size,
            anchor,
            &mut self.components,
            &self.theme,
            content,
            &HashMap::new(),
        )?;
        // This successful candidate captured the current kernel state under
        // the host lock. Later input dirties it again; failed preparation
        // returns above without consuming the obligation.
        self.dirty = false;
        let plan = (self.scene_host.direct_history_overflow_rows() > 0)
            .then(|| {
                crate::history::prepare_native_transfer_with_theme_and_content(
                    &self.history,
                    size.width,
                    self.scene_host.direct_history_overflow_rows(),
                    &self.theme,
                    content,
                )
            })
            .flatten();
        Ok((frame, plan))
    }
    pub(crate) fn invalidate_frame(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub(crate) fn next_output(&mut self) -> Option<RoutedOutput> {
        self.routed_outputs.pop_front()
    }
    pub(crate) fn drain_deferred_pastes(&mut self) -> Result<(), OutputDispatchError> {
        while let Some(text) = self.deferred_pastes.pop_front() {
            let previous = self.scene_host.focused_component();
            let result = self.scene_host.dispatch_paste(&text, &mut self.components);
            let next = self.scene_host.focused_component();
            self.drain_outputs_to_pending()?;
            if result == InteractionResult::Consumed {
                self.invalidate_interaction_components(previous, next);
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn invalidate_interaction_components(
        &mut self,
        previous: Option<crate::component::ComponentId>,
        next: Option<crate::component::ComponentId>,
    ) {
        if let Some(id) = previous {
            self.scene_host.invalidate_component(id);
        }
        if next != previous
            && let Some(id) = next
        {
            self.scene_host.invalidate_component(id);
        }
    }
    fn status(&self, more_ready: bool, changed_components: Vec<u64>) -> ReadyStatus {
        ReadyStatus {
            dirty: self.dirty,
            exiting: self.exit_requested,
            more_ready: more_ready && !self.exit_requested,
            changed_components,
        }
    }
    fn drain_outputs_to_pending(&mut self) -> Result<(), OutputDispatchError> {
        self.pending_outputs.extend(
            self.scene_host
                .drain_outputs(&self.outputs)
                .map_err(|_| OutputDispatchError::TypeMismatch)?,
        );
        Ok(())
    }
}
