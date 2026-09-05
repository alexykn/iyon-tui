use anyhow::Result;
use tokio::sync::oneshot;

use crate::{backend::NativeHistorySink, geometry::Size, scene::PreparedSceneFrame};

pub(crate) type PresentReceipt = oneshot::Receiver<Result<()>>;

/// Typed terminal-worker lifecycle failure. Host recovery must classify this
/// before formatting diagnostics; matching human-readable error strings in a
/// frame hot path is both brittle and observability-hostile.
#[derive(Debug)]
pub(crate) struct TerminalWorkerStopped;

impl std::fmt::Display for TerminalWorkerStopped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("terminal worker stopped")
    }
}

impl std::error::Error for TerminalWorkerStopped {}

pub(crate) fn terminal_worker_stopped() -> anyhow::Error {
    anyhow::Error::new(TerminalWorkerStopped)
}

pub(crate) fn is_terminal_worker_stopped(error: &anyhow::Error) -> bool {
    error.downcast_ref::<TerminalWorkerStopped>().is_some()
}

/// Semantic terminal input understood by the runtime driver.
#[derive(Debug)]
pub(crate) enum TerminalEvent {
    Key(crate::KeyStroke),
    Paste(String),
    Resize,
}

/// Private semantic terminal session boundary.
///
/// `next_event` is cancellation-safe: the runtime may stop awaiting it during
/// a select cycle and await the next event again without losing backend state.
pub(crate) trait TerminalBackend: NativeHistorySink<Error = anyhow::Error> {
    async fn next_event(&mut self) -> Result<TerminalEvent>;

    fn try_next_event(&mut self) -> Result<Option<TerminalEvent>>;

    fn viewport(&mut self) -> Result<Size>;

    fn begin_frame(&mut self, frame: &PreparedSceneFrame) -> Result<PresentReceipt>;

    fn position_after_final_frame(&mut self) -> Result<()>;

    fn restore(&mut self) -> Result<()>;
}
