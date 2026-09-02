use std::{
    future::pending,
    time::{Duration, Instant},
};

use crate::terminal::{PresentReceipt, TerminalBackend, TerminalEvent};

use super::{
    app::App,
    error::{RunError, RuntimeError},
    kernel::{KernelError, RunningApp},
};

pub(crate) async fn run<State, Action, Error, Init, Update, ViewFn>(
    app: App<State, Action, Error, Init, Update, ViewFn>,
) -> Result<(), RunError<Error>>
where
    Init: FnOnce(&mut super::context::AppCx<'_, Action>) -> Result<State, Error>,
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
{
    run_with_factory(app, crate::terminal::enter_default).await
}

async fn run_with_factory<State, Action, Error, Init, Update, ViewFn, Backend, Factory>(
    app: App<State, Action, Error, Init, Update, ViewFn>,
    factory: Factory,
) -> Result<(), RunError<Error>>
where
    Init: FnOnce(&mut super::context::AppCx<'_, Action>) -> Result<State, Error>,
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
    Factory: FnOnce() -> anyhow::Result<Backend>,
{
    let now = Instant::now();
    let mut app = app.start(now).map_err(map_kernel_error)?;
    if app.is_exiting() {
        app.close_ingress();
        return Ok(());
    }
    let backend = factory().map_err(runtime_error)?;
    run_running(app, backend).await
}

#[cfg(test)]
pub(crate) async fn run_with_backend<State, Action, Error, Init, Update, ViewFn, Backend>(
    app: App<State, Action, Error, Init, Update, ViewFn>,
    backend: Backend,
) -> Result<(), RunError<Error>>
where
    Init: FnOnce(&mut super::context::AppCx<'_, Action>) -> Result<State, Error>,
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    run_with_factory(app, || Ok(backend)).await
}

#[cfg(test)]
pub(crate) async fn run_with_backend_factory<
    State,
    Action,
    Error,
    Init,
    Update,
    ViewFn,
    Backend,
    Factory,
>(
    app: App<State, Action, Error, Init, Update, ViewFn>,
    factory: Factory,
) -> Result<(), RunError<Error>>
where
    Init: FnOnce(&mut super::context::AppCx<'_, Action>) -> Result<State, Error>,
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
    Factory: FnOnce() -> anyhow::Result<Backend>,
{
    run_with_factory(app, factory).await
}

async fn run_running<State, Action, Error, Update, ViewFn, Backend>(
    mut app: RunningApp<State, Action, Error, Update, ViewFn>,
    backend: Backend,
) -> Result<(), RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    let mut session = TerminalSession::new(backend);
    let result = drive(&mut app, &mut session).await;
    match result {
        Ok(()) => {
            session
                .position_after_final_frame()
                .map_err(|error| RunError::Runtime(runtime_error(error)))?;
            session
                .restore()
                .map_err(|error| RunError::Runtime(runtime_error(error)))
        }
        Err(error) => {
            let _ = session.restore();
            Err(error)
        }
    }
}

const INPUT_PUMP_BUDGET: usize = 32;
const MIN_PRESENT_INTERVAL: Duration = Duration::from_millis(8);

struct PresentationScheduler {
    last_present: Instant,
    deadline: Option<Instant>,
}

impl PresentationScheduler {
    fn new(now: Instant) -> Self {
        Self {
            last_present: now,
            deadline: None,
        }
    }

    fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn should_present(&mut self, dirty: bool, now: Instant) -> bool {
        if !dirty {
            self.deadline = None;
            return false;
        }
        let next = self.last_present + MIN_PRESENT_INTERVAL;
        if now >= next {
            self.deadline = None;
            return true;
        }
        self.deadline.get_or_insert(next);
        false
    }

    fn presented(&mut self, now: Instant) {
        self.last_present = now;
        self.deadline = None;
    }
}

async fn drive<State, Action, Error, Update, ViewFn, Backend>(
    app: &mut RunningApp<State, Action, Error, Update, ViewFn>,
    session: &mut TerminalSession<Backend>,
) -> Result<(), RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    prepare_and_draw(app, session, Instant::now()).await?;
    let mut in_flight = None;
    let mut presentation = PresentationScheduler::new(Instant::now());
    presentation.presented(Instant::now());
    app.drain_deferred_pastes().map_err(map_kernel_error)?;
    app.collect_external_pending();

    loop {
        pump_terminal_input(app, session)?;
        let now = Instant::now();
        let status = app.advance_ready(now).map_err(map_kernel_error)?;
        if in_flight.is_none() && presentation.should_present(status.dirty, now) {
            in_flight = Some(prepare_and_begin(app, session, now)?);
        }
        if status.exiting {
            if let Some(receipt) = in_flight.take() {
                receipt
                    .await
                    .map_err(|error| RunError::Runtime(runtime_error(error)))?
                    .map_err(|error| RunError::Runtime(runtime_error(error)))?;
            }
            let final_status = app
                .advance_ready(Instant::now())
                .map_err(map_kernel_error)?;
            if final_status.dirty {
                prepare_and_draw(app, session, Instant::now()).await?;
            }
            break;
        }
        if status.more_ready {
            tokio::task::yield_now().await;
            continue;
        }

        let deadline = [app.next_deadline(), presentation.deadline()]
            .into_iter()
            .flatten()
            .min();
        tokio::select! {
            result = wait_for_present(&mut in_flight) => {
                result.map_err(|error| RunError::Runtime(runtime_error(error)))?;
                in_flight = None;
                presentation.presented(Instant::now());
            }
            _ = wait_for_deadline(deadline) => {}
            event = session.next_event() => {
                dispatch_terminal_event(app, event
                    .map_err(|error| RunError::Runtime(runtime_error(error)))?)?;
            }
            action = app.recv_external(), if app.ingress_is_open() => {
                match action {
                    Some(action) => app.collect_external(action),
                    None => app.close_ingress(),
                }
            }
        }
    }

    Ok(())
}

fn dispatch_terminal_event<State, Action, Error, Update, ViewFn>(
    app: &mut RunningApp<State, Action, Error, Update, ViewFn>,
    event: TerminalEvent,
) -> Result<(), RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
{
    match event {
        TerminalEvent::Key(key) => {
            app.dispatch_key(key).map_err(map_kernel_error)?;
        }
        TerminalEvent::Paste(text) => {
            app.dispatch_paste(&text).map_err(map_kernel_error)?;
        }
        TerminalEvent::Resize => app.invalidate_frame(),
    }
    Ok(())
}

fn pump_terminal_input<State, Action, Error, Update, ViewFn, Backend>(
    app: &mut RunningApp<State, Action, Error, Update, ViewFn>,
    session: &mut TerminalSession<Backend>,
) -> Result<(), RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    for _ in 0..INPUT_PUMP_BUDGET {
        let Some(event) = session
            .try_next_event()
            .map_err(|error| RunError::Runtime(runtime_error(error)))?
        else {
            break;
        };
        dispatch_terminal_event(app, event)?;
    }
    Ok(())
}

async fn prepare_and_draw<State, Action, Error, Update, ViewFn, Backend>(
    app: &mut RunningApp<State, Action, Error, Update, ViewFn>,
    session: &mut TerminalSession<Backend>,
    now: Instant,
) -> Result<(), RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    prepare_and_begin(app, session, now)?
        .await
        .map_err(|error| RunError::Runtime(runtime_error(error)))?
        .map_err(|error| RunError::Runtime(runtime_error(error)))
}

fn prepare_and_begin<State, Action, Error, Update, ViewFn, Backend>(
    app: &mut RunningApp<State, Action, Error, Update, ViewFn>,
    session: &mut TerminalSession<Backend>,
    now: Instant,
) -> Result<PresentReceipt, RunError<Error>>
where
    Update: FnMut(&mut State, Action, &mut super::context::AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> crate::View,
    Backend: TerminalBackend,
{
    let frame = app
        .prepare_frame(now, session.backend_mut(), |backend| backend.viewport())
        .map_err(|error| RunError::Runtime(runtime_error(error)))?;
    session
        .begin_frame(&frame)
        .map_err(|error| RunError::Runtime(runtime_error(error)))
}

async fn wait_for_present(pending: &mut Option<PresentReceipt>) -> anyhow::Result<()> {
    let Some(receipt) = pending.as_mut() else {
        return std::future::pending().await;
    };
    receipt
        .await
        .map_err(|error| anyhow::anyhow!("terminal presentation reply lost: {error}"))?
}

pub(crate) fn wait_for_present_blocking(
    pending: &mut Option<PresentReceipt>,
) -> anyhow::Result<()> {
    let Some(receipt) = pending.take() else {
        return Ok(());
    };
    receipt
        .blocking_recv()
        .map_err(|error| anyhow::anyhow!("terminal presentation reply lost: {error}"))?
}

pub(crate) async fn wait_for_deadline(deadline: Option<Instant>) {
    let Some(deadline) = deadline else {
        pending::<()>().await;
        return;
    };
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
}

struct TerminalSession<Backend: TerminalBackend> {
    backend: Backend,
    restored: bool,
}

impl<Backend> TerminalSession<Backend>
where
    Backend: TerminalBackend,
{
    fn new(backend: Backend) -> Self {
        Self {
            backend,
            restored: false,
        }
    }

    fn backend_mut(&mut self) -> &mut Backend {
        &mut self.backend
    }

    fn next_event(
        &mut self,
    ) -> impl std::future::Future<Output = anyhow::Result<TerminalEvent>> + '_ {
        self.backend.next_event()
    }

    fn try_next_event(&mut self) -> anyhow::Result<Option<TerminalEvent>> {
        self.backend.try_next_event()
    }

    fn begin_frame(
        &mut self,
        frame: &crate::scene::PreparedSceneFrame,
    ) -> anyhow::Result<PresentReceipt> {
        self.backend.begin_frame(frame)
    }

    fn position_after_final_frame(&mut self) -> anyhow::Result<()> {
        self.backend.position_after_final_frame()
    }

    fn restore(&mut self) -> anyhow::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        self.backend.restore()
    }
}

impl<Backend> Drop for TerminalSession<Backend>
where
    Backend: TerminalBackend,
{
    fn drop(&mut self) {
        if !self.restored {
            self.restored = true;
            let _ = self.backend.restore();
        }
    }
}

fn runtime_error(error: impl Into<anyhow::Error>) -> RuntimeError {
    RuntimeError::new(error)
}

fn map_kernel_error<Error>(error: KernelError<Error>) -> RunError<Error> {
    match error {
        KernelError::Application(error) => RunError::Application(error),
        KernelError::Output(error) => RunError::Runtime(runtime_error(error)),
    }
}
