use tokio::sync::mpsc::Receiver;

use crate::{History, Theme, View};

use super::{context::AppCx, error::RunError, handle::AppHandle, kernel::RunningApp};

/// A generic standalone application definition.
///
/// `App` stores application State, Action, and the three semantic callbacks.
/// It has no terminal or executor dependency.
pub struct App<State, Action, Error, Init, Update, ViewFn> {
    pub(crate) init: Init,
    pub(crate) update: Update,
    pub(crate) view: ViewFn,
    pub(crate) history: Option<History>,
    pub(crate) theme: Theme,
    pub(crate) handle: AppHandle<Action>,
    pub(crate) ingress: Option<Receiver<Action>>,
    pub(crate) marker: std::marker::PhantomData<fn(State, Action) -> Error>,
}

impl<State, Action, Error, Init, Update, ViewFn> App<State, Action, Error, Init, Update, ViewFn> {
    /// Defines an application from initialization, action update, and body
    /// derivation callbacks.
    pub fn new(init: Init, update: Update, view: ViewFn) -> Self
    where
        Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>,
        Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
        ViewFn: Fn(&State) -> View,
    {
        let (handle, ingress) = AppHandle::channel();
        Self {
            init,
            update,
            view,
            history: None,
            theme: Theme::default(),
            handle,
            ingress: Some(ingress),
            marker: std::marker::PhantomData,
        }
    }

    /// Returns a cloneable Action-only producer for this application.
    pub fn handle(&self) -> AppHandle<Action> {
        self.handle.clone()
    }

    /// Configures the application-owned semantic paint theme.
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Runs the application with the default terminal adapter.
    ///
    /// The future may remain single-threaded when State or Action is not
    /// `Send`. Await it on a Tokio-compatible runtime.
    pub async fn run(self) -> Result<(), RunError<Error>>
    where
        Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>,
        Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
        ViewFn: Fn(&State) -> View,
    {
        super::run::run(self).await
    }

    /// Configures the one persistent root History owned by this application.
    #[must_use]
    pub fn with_history(mut self, history: History) -> Self {
        self.history = Some(history);
        self
    }

    pub(crate) fn start(
        self,
        now: std::time::Instant,
    ) -> Result<RunningApp<State, Action, Error, Update, ViewFn>, super::kernel::KernelError<Error>>
    where
        Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>,
        Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
        ViewFn: Fn(&State) -> View,
    {
        RunningApp::new(self, now)
    }
}
