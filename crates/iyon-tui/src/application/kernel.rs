use std::{collections::VecDeque, marker::PhantomData, time::Instant};

use tokio::sync::mpsc::{Receiver, error::TryRecvError};

use anyhow::Result;

use crate::{
    ComponentHandle, InteractionResult, OutputRouter, Scene, View,
    backend::NativeHistorySink,
    component::ComponentRegistry,
    geometry::Size,
    output::OutputDispatchError,
    scene::{PreparedSceneFrame, SceneHost, SceneHostError},
};

use super::{
    app::App,
    context::{AppCx, AppCxParts},
    handle::AppHandle,
    input::{GlobalBindings, PasteInterceptors},
    timer::TimerQueue,
};

const ACTION_BATCH_BUDGET: usize = 128;

#[derive(Debug)]
pub(crate) enum KernelError<Error> {
    Application(Error),
    Output(OutputDispatchError),
    Stream(crate::HistoryError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadyStatus {
    pub(crate) dirty: bool,
    pub(crate) exiting: bool,
    pub(crate) more_ready: bool,
}

pub(crate) struct RunningApp<State, Action, Error, Update, ViewFn> {
    pub(crate) state: State,
    scene: Scene,
    theme: crate::Theme,
    components: ComponentRegistry,
    outputs: OutputRouter<Action>,
    scene_host: SceneHost,
    actions: VecDeque<Action>,
    timers: TimerQueue<Action>,
    global_bindings: GlobalBindings<Action>,
    paste_interceptors: PasteInterceptors<Action>,
    /// PERF-12 T13.1 R8: component ids whose language handle was disposed and
    /// which may be physically reclaimed once the last SUCCESSFULLY reconciled
    /// mount graph no longer contains them (deferred retirement — never
    /// eager, because committed roots may still reference them until their
    /// replacement publishes).
    pending_component_retirements: Vec<u64>,
    deferred_pastes: VecDeque<String>,
    ingress: Option<Receiver<Action>>,
    handle: AppHandle<Action>,
    update: Update,
    view: ViewFn,
    dirty: bool,
    body_dirty: bool,
    exit_requested: bool,
    marker: PhantomData<fn() -> Error>,
}

impl<State, Action, Error, Update, ViewFn> RunningApp<State, Action, Error, Update, ViewFn>
where
    Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> View,
{
    pub(crate) fn host_register<C>(&mut self, component: C) -> ComponentHandle<C>
    where
        C: crate::Component,
    {
        self.components.register(component)
    }

    pub(crate) fn host_bind_key(
        &mut self,
        key: crate::KeyStroke,
        action: impl Fn() -> Action + 'static,
    ) {
        self.global_bindings.bind(key, action);
    }

    pub(crate) fn host_route<T: 'static>(
        &mut self,
        output: crate::Output<T>,
        map: impl Fn(T) -> Action + 'static,
    ) -> Result<(), crate::RouteConflict> {
        self.outputs.route(output, map)
    }

    pub(crate) fn host_intercept_paste<C>(
        &mut self,
        component: ComponentHandle<C>,
        map: impl Fn(String) -> Action + 'static,
    ) where
        C: crate::Component,
    {
        self.paste_interceptors.intercept(component, map);
    }

    pub(crate) fn host_forward_paste(&mut self, text: String) -> Result<(), KernelError<Error>> {
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
        self.components
            .invalidate(crate::component::ComponentId::from_raw(id));
        self.invalidate_frame();
    }

    pub(crate) fn host_clear_retained_views(&mut self) {
        self.scene_host.clear_retained_views();
    }

    pub(crate) fn host_set_body(&mut self, body: View) {
        if self.scene.body() == &body {
            self.body_dirty = false;
            return;
        }
        self.scene.set_body(body);
        self.body_dirty = false;
        self.dirty = true;
    }

    pub(crate) fn host_set_theme(&mut self, theme: crate::Theme) {
        self.theme = theme;
        self.invalidate_frame();
    }

    pub(crate) fn host_set_history(&mut self, history: crate::History) {
        self.scene.set_history(history);
        self.invalidate_frame();
    }

    pub(crate) fn host_exited(&self) -> bool {
        self.exit_requested
    }

    pub(crate) fn host_exit(&mut self) {
        self.exit_requested = true;
        self.close_ingress();
        self.dirty = true;
    }

    pub(crate) fn scene_history(&self) -> Option<&crate::History> {
        self.scene.history()
    }

    pub(crate) fn scene_history_mut(&mut self) -> Option<&mut crate::History> {
        self.scene.history_mut()
    }

    pub(crate) fn new<Init>(
        app: App<State, Action, Error, Init, Update, ViewFn>,
        now: Instant,
    ) -> Result<Self, KernelError<Error>>
    where
        Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>,
    {
        let App {
            init,
            update,
            view,
            history,
            mut theme,
            handle,
            ingress,
            marker: _,
        } = app;
        let mut scene = history.map_or_else(
            || Scene::new(View::spacer(0)),
            |history| Scene::with_history(history, View::spacer(0)),
        );
        let mut components = ComponentRegistry::new();
        let mut outputs = OutputRouter::new();
        let mut timers = TimerQueue::default();
        let mut global_bindings = GlobalBindings::default();
        let mut paste_interceptors = PasteInterceptors::default();
        let mut deferred_pastes = VecDeque::new();
        let mut exit_requested = false;
        let state = {
            let mut cx = AppCx::new(
                AppCxParts {
                    scene: &mut scene,
                    components: &mut components,
                    outputs: &mut outputs,
                    timers: &mut timers,
                    theme: &mut theme,
                    global_bindings: &mut global_bindings,
                    paste_interceptors: &mut paste_interceptors,
                    deferred_pastes: &mut deferred_pastes,
                    exit_requested: &mut exit_requested,
                    handle: &handle,
                },
                now,
            );
            init(&mut cx).map_err(KernelError::Application)?
        };
        let mut running = Self {
            state,
            scene,
            theme,
            components,
            outputs,
            scene_host: SceneHost::default(),
            pending_component_retirements: Vec::new(),
            actions: VecDeque::new(),
            timers,
            global_bindings,
            paste_interceptors,
            deferred_pastes,
            ingress,
            handle,
            update,
            view,
            dirty: true,
            body_dirty: false,
            exit_requested,
            marker: PhantomData,
        };
        let body = (running.view)(&running.state);
        running.scene.set_body(body);
        if running.exit_requested {
            running.close_ingress();
        }
        Ok(running)
    }

    pub(crate) fn dispatch_key(
        &mut self,
        key: crate::KeyStroke,
    ) -> Result<InteractionResult, KernelError<Error>> {
        if self.exit_requested {
            return Ok(InteractionResult::Ignored);
        }
        let result = self
            .scene_host
            .dispatch_key_local(key, &mut self.components);
        self.drain_outputs_to_actions()?;
        if result == InteractionResult::Ignored
            && let Some(action) = self.global_bindings.action(key)
        {
            self.actions.push_back(action);
            return Ok(InteractionResult::Consumed);
        }
        if result == InteractionResult::Consumed {
            self.dirty = true;
        }
        Ok(result)
    }

    pub(crate) fn dispatch_paste(
        &mut self,
        text: &str,
    ) -> Result<InteractionResult, KernelError<Error>> {
        if self.exit_requested {
            return Ok(InteractionResult::Ignored);
        }
        if let Some(action) = self.scene_host.intercept_paste(text, |component, _text| {
            self.paste_interceptors.action(component, text)
        }) {
            self.actions.push_back(action);
            return Ok(InteractionResult::Consumed);
        }

        let result = self.scene_host.dispatch_paste(text, &mut self.components);
        self.drain_outputs_to_actions()?;
        if result == InteractionResult::Consumed {
            self.dirty = true;
        }
        Ok(result)
    }

    pub(crate) fn advance_ready(
        &mut self,
        now: Instant,
    ) -> Result<ReadyStatus, KernelError<Error>> {
        if self.exit_requested {
            self.close_ingress();
            self.actions.clear();
            self.timers.clear();
            return Ok(self.status(false));
        }

        self.collect_due_timers_front(now);
        let tick = self.scene_host.tick_due(now, &mut self.components);
        self.dirty |= tick.dirty;
        self.drain_outputs_to_actions()?;

        for _ in 0..ACTION_BATCH_BUDGET {
            let Some(action) = self.actions.pop_front() else {
                break;
            };
            let update_result = {
                let mut cx = AppCx::new(
                    AppCxParts {
                        scene: &mut self.scene,
                        components: &mut self.components,
                        outputs: &mut self.outputs,
                        timers: &mut self.timers,
                        theme: &mut self.theme,
                        global_bindings: &mut self.global_bindings,
                        paste_interceptors: &mut self.paste_interceptors,
                        deferred_pastes: &mut self.deferred_pastes,
                        exit_requested: &mut self.exit_requested,
                        handle: &self.handle,
                    },
                    now,
                );
                (self.update)(&mut self.state, action, &mut cx)
            };
            update_result.map_err(KernelError::Application)?;
            self.dirty = true;
            self.body_dirty = true;
            self.drain_outputs_to_actions()?;
            self.drain_deferred_pastes()?;
            self.collect_due_timers(now);
            if self.exit_requested {
                self.close_ingress();
                self.actions.clear();
                self.timers.clear();
                break;
            }
        }

        if !self.exit_requested {
            let stream_changed = self
                .scene
                .advance_streams(now)
                .map_err(|error| KernelError::Stream(error))?;
            self.dirty |= stream_changed;
        }

        Ok(self.status(!self.actions.is_empty()))
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        [
            self.timers.next_deadline(),
            self.scene_host.next_tick_deadline(),
            self.scene.next_stream_wakeup(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn prepare_frame<S, F>(
        &mut self,
        now: Instant,
        sink: &mut S,
        mut viewport: F,
    ) -> Result<PreparedSceneFrame, SceneHostError<S::Error>>
    where
        S: NativeHistorySink,
        F: FnMut(&mut S) -> Result<Size>,
    {
        if self.body_dirty {
            let body = (self.view)(&self.state);
            if self.scene.body() != &body {
                self.scene.set_body(body);
            }
            self.body_dirty = false;
        }
        let frame = self.scene_host.render_at(
            now,
            &mut self.scene,
            &mut self.components,
            &self.theme,
            sink,
            &mut viewport,
        )?;
        self.dirty = false;
        Ok(frame)
    }

    pub(crate) fn invalidate_frame(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn is_exiting(&self) -> bool {
        self.exit_requested
    }

    #[cfg(feature = "test-util")]
    pub(crate) fn handle(&self) -> AppHandle<Action> {
        self.handle.clone()
    }

    pub(crate) fn close_ingress(&mut self) {
        self.ingress = None;
    }

    pub(crate) fn ingress_is_open(&self) -> bool {
        self.ingress.is_some()
    }

    pub(crate) async fn recv_external(&mut self) -> Option<Action> {
        self.ingress.as_mut()?.recv().await
    }

    fn drain_ingress(&mut self, budget: usize) {
        for _ in 0..budget {
            let Some(ingress) = self.ingress.as_mut() else {
                break;
            };
            match ingress.try_recv() {
                Ok(action) => self.actions.push_back(action),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.close_ingress();
                    break;
                }
            }
        }
    }

    pub(crate) fn collect_external_pending(&mut self) {
        self.drain_ingress(ACTION_BATCH_BUDGET);
    }

    pub(crate) fn collect_external(&mut self, first: Action) {
        self.actions.push_back(first);
        self.drain_ingress(ACTION_BATCH_BUDGET.saturating_sub(1));
    }

    pub(crate) fn drain_deferred_pastes(&mut self) -> Result<(), KernelError<Error>> {
        while let Some(text) = self.deferred_pastes.pop_front() {
            let result = self.scene_host.dispatch_paste(&text, &mut self.components);
            self.drain_outputs_to_actions()?;
            if result == InteractionResult::Consumed {
                self.dirty = true;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn focused_for_test(&self) -> bool {
        self.scene_host.focused().is_some()
    }

    #[cfg(test)]
    pub(crate) fn mount_count_for_test(&self) -> usize {
        self.scene_host.mount_count_for_test()
    }

    #[cfg(test)]
    pub(crate) fn focusable_count_for_test(&self) -> usize {
        self.scene_host.focusable_count_for_test()
    }

    fn status(&self, more_ready: bool) -> ReadyStatus {
        ReadyStatus {
            dirty: self.dirty,
            exiting: self.exit_requested,
            more_ready: more_ready && !self.exit_requested,
        }
    }

    fn collect_due_timers_front(&mut self, now: Instant) {
        let mut due = Vec::new();
        while let Some(action) = self.timers.pop_due(now) {
            due.push(action);
        }
        for action in due.into_iter().rev() {
            self.actions.push_front(action);
        }
    }

    fn collect_due_timers(&mut self, now: Instant) {
        while let Some(action) = self.timers.pop_due(now) {
            self.actions.push_back(action);
        }
    }

    fn drain_outputs_to_actions(&mut self) -> Result<(), KernelError<Error>> {
        let actions = self
            .scene_host
            .drain_outputs(&self.outputs)
            .map_err(KernelError::Output)?;
        self.actions.extend(actions);
        Ok(())
    }
}

impl<Error> From<OutputDispatchError> for KernelError<Error> {
    fn from(error: OutputDispatchError) -> Self {
        Self::Output(error)
    }
}
