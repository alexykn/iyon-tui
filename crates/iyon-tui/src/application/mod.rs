//! Generic standalone application kernel.
//!
//! [`App`] owns application state and framework runtime state while [`AppCx`]
//! exposes the narrow capabilities available to `init` and `update`.
//! Components continue to handle local interaction; their typed outputs are
//! routed into the application's action queue.

mod app;
mod context;
#[cfg(feature = "native-host")]
mod environment;
mod error;
mod handle;
#[cfg(feature = "native-host")]
mod host;
mod input;
mod kernel;
mod run;
mod timer;
#[cfg(feature = "native-host")]
mod view_state;

#[cfg(test)]
mod tests;

pub use app::App;
pub use context::AppCx;
#[cfg(feature = "native-host")]
pub use environment::{
    HostCommit, HostDrainReport, HostEpochs, HostFrameError, TuiEnvironment, WakeDisposition,
};
pub use error::{RunError, RuntimeError};
pub use handle::{AppClosed, AppHandle, AppSendError};
#[cfg(feature = "native-host")]
pub use host::{
    HostCellStyle, HostHistory, HostScrollPane, HostTextInput, HostTextStream, HostViewSlot,
    RoutedOutput, TextStreamAnnotation, TextStreamPresentation, TuiHost,
};
#[cfg(feature = "test-util")]
pub(crate) use kernel::{KernelError, RunningApp};
pub use timer::TimerHandle;
#[cfg(feature = "native-host")]
pub use view_state::HostViewState;
