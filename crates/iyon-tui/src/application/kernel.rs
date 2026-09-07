use std::{collections::VecDeque, sync::Arc, time::Instant};

use anyhow::Result;

use crate::presentation::factory as vf;
use crate::{
    ComponentHandle, InteractionResult, OutputRouter, Scene, View,
    backend::NativeHistorySink,
    component::ComponentRegistry,
    geometry::Size,
    output::OutputDispatchError,
    presentation::ContentProvider,
    retained_state::StateFrameView,
    scene::{PreparedSceneFrame, SceneHost, SceneHostError},
};

use super::{
    host::RoutedOutput,
    input::{GlobalBindings, PasteInterceptors},
};

const OUTPUT_BATCH_BUDGET: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadyStatus {
    pub(crate) dirty: bool,
    pub(crate) exiting: bool,
    pub(crate) more_ready: bool,
}

/// Native retained runtime owned by `HostInner`.
///
/// This is intentionally concrete: the native host has one caller-defined
/// routed-output value, and does not expose a generic Rust application loop,
/// callback set, external ingress, or application timer queue.
pub(crate) struct NativeRuntime {
    scene: Scene,
    theme: Arc<crate::Theme>,
    components: ComponentRegistry,
    outputs: OutputRouter<RoutedOutput>,
    scene_host: SceneHost,
    pending_outputs: VecDeque<RoutedOutput>,
    global_bindings: GlobalBindings,
    paste_interceptors: PasteInterceptors,
    /// PERF-12 T13.1 R8: component ids whose language handle was disposed and
    /// which may be physically reclaimed once the last SUCCESSFULLY reconciled
    /// mount graph no longer contains them (deferred retirement — never
    /// eager, because committed roots may still reference them until their
    /// replacement publishes).
    pending_component_retirements: Vec<u64>,
    deferred_pastes: VecDeque<String>,
    routed_outputs: VecDeque<RoutedOutput>,
    dirty: bool,
    exit_requested: bool,
}

impl NativeRuntime {
    pub(crate) fn host_register<C>(&mut self, component: C) -> ComponentHandle<C>
    where
        C: crate::Component,
    {
        self.components.register(component)
    }

    pub(crate) fn host_bind_key(
        &mut self,
        key: crate::KeyStroke,
        factory: impl Fn() -> RoutedOutput + 'static,
    ) {
        self.global_bindings.bind(key, factory);
    }

    pub(crate) fn host_route<T: 'static>(
        &mut self,
        output: crate::Output<T>,
        map: impl Fn(T) -> RoutedOutput + 'static,
    ) -> Result<(), crate::RouteConflict> {
        self.outputs.route(output, map)
    }

    pub(crate) fn host_intercept_paste<C>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> RoutedOutput + 'static,
    ) where
        C: crate::Component,
    {
        self.paste_interceptors.intercept(component, map);
    }

    pub(crate) fn host_forward_paste(&mut self, text: String) -> Result<(), OutputDispatchError> {
        self.deferred_pastes.push_back(text);
        self.drain_deferred_pastes()
    }

    /// PERF-12 T13.1 R8: request deferred retirement of a host-registered
    /// component by raw id. The registry entry survives until a successful
    /// reconciliation proves the component unmounted — an eager remove here
    /// would leave committed roots referencing a destroyed component when a
    /// later publication fails.
    pub(crate) fn host_retire_component(&mut self, raw_id: u64) {
        self.pending_component_retirements.push(raw_id);
        self.reap_retired_components();
    }

    /// Physically reclaim retired components that the last successfully
    /// reconciled mount graph no longer contains. Called immediately on
    /// retirement (covers components that never mounted) and after every
    /// successful `prepare_frame`. Deliberately NOT called after a failed
    /// frame — the previous authoritative graph still matters then.
    pub(crate) fn reap_retired_components(&mut self) {
        if self.pending_component_retirements.is_empty() {
            return;
        }
        let mut still_pending = Vec::new();
        for raw_id in self.pending_component_retirements.drain(..) {
            let id = crate::component::ComponentId::from_raw(raw_id);
            if self.scene_host.is_mounted(id) {
                still_pending.push(raw_id);
                continue;
            }
            self.components.remove_id(id);
            self.paste_interceptors.remove_id(id);
        }
        self.pending_component_retirements = still_pending;
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_invalidate_component(&mut self, id: u64) {
        let id = crate::component::ComponentId::from_raw(id);
        self.components.invalidate(id);
        self.scene_host.invalidate_component(id);
        self.invalidate_frame();
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_invalidate_state(
        &mut self,
        id: u64,
        effects: crate::retained_state::StateEffects,
    ) {
        self.scene_host.invalidate_state(id, effects);
        self.invalidate_frame();
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_invalidate_content(&mut self, dirty: crate::presentation::ContentDirty) {
        self.scene_host.invalidate_content(dirty);
        self.invalidate_frame();
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_content_candidate_epoch(&self) -> u64 {
        self.scene_host.content_candidate_epoch()
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_commit_content_candidate(&mut self, epoch: u64) {
        self.scene_host.commit_content_candidate(epoch);
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_abort_content_candidate(&mut self) {
        self.scene_host.abort_content_candidate();
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_recover_native_history_synchronization(&mut self) {
        if let Some(history) = self.scene.history_mut() {
            history.recover_native_synchronization();
        }
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_has_invalidated_components(&self) -> bool {
        self.scene_host.has_invalidated_components()
    }

    /// Materializes component snapshots without running layout so H3 can
    /// validate retained-state attachments in the complete desired scene, not
    /// only on the root View's direct semantic nodes.
    #[cfg(feature = "native-host")]
    pub(crate) fn host_state_attachment_targets(
        &self,
        body: &View,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        let history_views = self
            .scene
            .history()
            .map_or_else(Vec::new, crate::History::state_views);
        self.host_state_attachment_targets_from_history_views(body, history_views)
    }

    /// Collects state attachments from a prospective History view in addition
    /// to the currently retained body/History. This is used before History
    /// mutation so unsupported state geometry fails before the unit changes.
    #[cfg(feature = "native-host")]
    pub(crate) fn host_state_attachment_targets_with_history_view(
        &self,
        body: &View,
        history_view: &View,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        let mut history_views = self
            .scene
            .history()
            .map_or_else(Vec::new, crate::History::state_views);
        history_views.push(history_view.clone());
        self.host_state_attachment_targets_from_history_views(body, history_views)
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_state_attachment_targets_for_history(
        &self,
        body: &View,
        history: &crate::History,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        self.host_state_attachment_targets_from_history_views(body, history.state_views())
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_state_attachment_targets_for_history_views(
        &self,
        body: &View,
        history_views: Vec<View>,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        self.host_state_attachment_targets_from_history_views(body, history_views)
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_current_state_attachment_targets(
        &self,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        self.host_state_attachment_targets(self.scene.body())
    }

    /// Collects `ContentPort` attachments from the prospective body and current
    /// static/live History views. Source-backed History occurrences use the
    /// same retained `ContentPort` projection provider as body content.
    #[cfg(feature = "native-host")]
    pub(crate) fn host_content_attachment_targets(&self, body: &View) -> anyhow::Result<Vec<u64>> {
        let history_views = self
            .scene
            .history()
            .map_or_else(Vec::new, crate::History::content_views);
        self.host_content_attachment_targets_from_history_views(body, history_views)
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_current_content_attachment_targets(&self) -> anyhow::Result<Vec<u64>> {
        self.host_content_attachment_targets(self.scene.body())
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_content_attachment_targets_for_history(
        &self,
        body: &View,
        history: &crate::History,
    ) -> anyhow::Result<Vec<u64>> {
        self.host_content_attachment_targets_from_history_views(body, history.content_views())
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_content_attachment_targets_with_history_view(
        &self,
        body: &View,
        history_view: &View,
    ) -> anyhow::Result<Vec<u64>> {
        let mut history_views = self
            .scene
            .history()
            .map_or_else(Vec::new, crate::History::content_views);
        history_views.push(history_view.clone());
        self.host_content_attachment_targets_from_history_views(body, history_views)
    }

    #[cfg(feature = "native-host")]
    pub(crate) fn host_content_attachment_targets_for_history_views(
        &self,
        body: &View,
        history_views: Vec<View>,
    ) -> anyhow::Result<Vec<u64>> {
        self.host_content_attachment_targets_from_history_views(body, history_views)
    }

    #[cfg(feature = "native-host")]
    fn host_content_attachment_targets_from_history_views(
        &self,
        body: &View,
        history_views: Vec<View>,
    ) -> anyhow::Result<Vec<u64>> {
        let mut session = crate::scene::ResolveSession::new(&self.components);
        let mut targets = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut append_view = |view: &View| -> anyhow::Result<()> {
            let resolved = session
                .resolve_root(view)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let overlay = session.overlay().clone();
            for id in crate::scene::content_attachment_targets(&resolved, &overlay)
                .map_err(anyhow::Error::msg)?
            {
                if !seen.insert(id) {
                    return Err(anyhow::anyhow!(
                        "DUPLICATE_CONTENT_PORT_ATTACHMENT: ContentPort {id} occurs more than once in the candidate"
                    ));
                }
                targets.push(id);
            }
            Ok(())
        };
        append_view(body)?;
        for view in history_views {
            append_view(&view)?;
        }
        Ok(targets)
    }

    #[cfg(feature = "native-host")]
    fn host_state_attachment_targets_from_history_views(
        &self,
        body: &View,
        history_views: Vec<View>,
    ) -> anyhow::Result<Vec<(u64, crate::retained_state::StateNodeKind)>> {
        let mut session = crate::scene::ResolveSession::new(&self.components);
        let mut targets = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let mut append_view = |view: &View| -> anyhow::Result<()> {
            let resolved = session
                .resolve_root(view)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let overlay = session.overlay().clone();
            for (id, kind) in crate::scene::state_attachment_targets(&resolved, &overlay)
                .map_err(anyhow::Error::msg)?
            {
                if !seen.insert(id) {
                    return Err(anyhow::anyhow!(
                        "DUPLICATE_VIEW_STATE_ATTACHMENT: state {id} occurs more than once in the candidate"
                    ));
                }
                targets.push((id, kind));
            }
            Ok(())
        };

        append_view(body)?;
        for view in history_views {
            append_view(&view)?;
        }
        Ok(targets)
    }

    /// Discards an unpresented Scene candidate after a backend failure. The
    /// logical `HostInner` frame remains authoritative, so the next retry must
    /// rebuild the derived scene instead of treating the rejected candidate as
    /// committed.
    pub(crate) fn host_discard_candidate(&mut self) {
        self.scene_host.discard_candidate();
        self.invalidate_frame();
    }

    pub(crate) fn host_clear_retained_views(&mut self) {
        self.scene_host.clear_retained_views();
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

    pub(crate) fn host_set_body(&mut self, body: View) {
        if self.scene.body() == &body {
            return;
        }
        self.scene.set_body(body);
        self.scene_host.invalidate_root();
        self.dirty = true;
    }

    pub(crate) fn host_set_theme(&mut self, theme: crate::Theme) {
        self.theme = Arc::new(theme);
        self.scene_host.invalidate_theme();
        self.invalidate_frame();
    }

    pub(crate) fn host_set_history(&mut self, history: crate::History) {
        self.scene.set_history(history);
        self.scene_host.invalidate_root();
        self.invalidate_frame();
    }

    pub(crate) fn host_exited(&self) -> bool {
        self.exit_requested
    }

    pub(crate) fn host_exit(&mut self) {
        self.exit_requested = true;
        self.pending_outputs.clear();
        self.deferred_pastes.clear();
        self.dirty = true;
    }

    pub(crate) fn scene_body(&self) -> &View {
        self.scene.body()
    }

    pub(crate) fn scene_history(&self) -> Option<&crate::History> {
        self.scene.history()
    }

    pub(crate) fn scene_history_mut(&mut self) -> Option<&mut crate::History> {
        self.scene.history_mut()
    }

    pub(crate) fn new() -> Self {
        Self {
            scene: Scene::with_history(crate::History::new(), vf::spacer(0)),
            theme: Arc::new(crate::Theme::new()),
            components: ComponentRegistry::new(),
            outputs: OutputRouter::new(),
            scene_host: SceneHost::default(),
            pending_component_retirements: Vec::new(),
            pending_outputs: VecDeque::new(),
            global_bindings: GlobalBindings::default(),
            paste_interceptors: PasteInterceptors::default(),
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
        let previous_focus = self.scene_host.focused_component();
        let result = self
            .scene_host
            .dispatch_key_local(key, &mut self.components);
        let next_focus = self.scene_host.focused_component();
        self.drain_outputs_to_pending()?;
        if result == InteractionResult::Ignored
            && let Some(output) = self.global_bindings.output(key)
        {
            self.pending_outputs.push_back(output);
            return Ok(InteractionResult::Consumed);
        }
        if result == InteractionResult::Consumed {
            self.invalidate_interaction_components(previous_focus, next_focus);
            self.dirty = true;
        }
        Ok(result)
    }

    pub(crate) fn dispatch_paste(
        &mut self,
        text: &str,
    ) -> Result<InteractionResult, OutputDispatchError> {
        if self.exit_requested {
            return Ok(InteractionResult::Ignored);
        }
        if let Some(output) = self.scene_host.intercept_paste(text, |component, _text| {
            self.paste_interceptors.output(component, text)
        }) {
            self.pending_outputs.push_back(output);
            return Ok(InteractionResult::Consumed);
        }

        let previous_focus = self.scene_host.focused_component();
        let result = self.scene_host.dispatch_paste(text, &mut self.components);
        let next_focus = self.scene_host.focused_component();
        self.drain_outputs_to_pending()?;
        if result == InteractionResult::Consumed {
            self.invalidate_interaction_components(previous_focus, next_focus);
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
            return Ok(self.status(false));
        }

        let tick = self.scene_host.tick_due(now, &mut self.components);
        self.dirty |= tick.dirty;
        self.drain_outputs_to_pending()?;

        for _ in 0..OUTPUT_BATCH_BUDGET {
            let Some(output) = self.pending_outputs.pop_front() else {
                break;
            };
            self.routed_outputs.push_back(output);
            // Keep the old host-visible scheduling contract: reducing a
            // routed interaction produces one native dirty step even though
            // there is no application view callback to rerun.
            self.dirty = true;
        }

        Ok(self.status(!self.pending_outputs.is_empty()))
    }

    pub(crate) fn has_pending_outputs(&self) -> bool {
        !self.pending_outputs.is_empty()
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.scene_host.next_tick_deadline()
    }

    /// Shared ownership of the active theme for frame/content contexts.
    /// Readers share one immutable table instead of cloning its maps.
    pub(crate) fn theme_shared(&self) -> &Arc<crate::Theme> {
        &self.theme
    }

    pub(crate) fn prepare_frame_with_states<S, F>(
        &mut self,
        now: Instant,
        sink: &mut S,
        mut viewport: F,
        states: &StateFrameView<'_>,
        content: &mut dyn ContentProvider,
    ) -> Result<PreparedSceneFrame, SceneHostError<S::Error>>
    where
        S: NativeHistorySink,
        F: FnMut(&mut S) -> Result<Size>,
    {
        let frame = self.scene_host.render_at_with_states(
            now,
            &mut self.scene,
            &mut self.components,
            &self.theme,
            sink,
            &mut viewport,
            states,
            content,
        )?;
        // Retirement is deferred until this successful reconciliation has
        // replaced the committed mount graph. A retired component that was
        // still mounted during the prior frame is now safe to reclaim.
        self.reap_retired_components();
        self.dirty = false;
        Ok(frame)
    }

    pub(crate) fn invalidate_frame(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn next_output(&mut self) -> Option<RoutedOutput> {
        self.routed_outputs.pop_front()
    }

    pub(crate) fn drain_deferred_pastes(&mut self) -> Result<(), OutputDispatchError> {
        while let Some(text) = self.deferred_pastes.pop_front() {
            let previous_focus = self.scene_host.focused_component();
            let result = self.scene_host.dispatch_paste(&text, &mut self.components);
            let next_focus = self.scene_host.focused_component();
            self.drain_outputs_to_pending()?;
            if result == InteractionResult::Consumed {
                self.invalidate_interaction_components(previous_focus, next_focus);
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn status(&self, more_ready: bool) -> ReadyStatus {
        ReadyStatus {
            dirty: self.dirty,
            exiting: self.exit_requested,
            more_ready: more_ready && !self.exit_requested,
        }
    }

    fn drain_outputs_to_pending(&mut self) -> Result<(), OutputDispatchError> {
        let outputs = self.scene_host.drain_outputs(&self.outputs)?;
        self.pending_outputs.extend(outputs);
        Ok(())
    }
}
