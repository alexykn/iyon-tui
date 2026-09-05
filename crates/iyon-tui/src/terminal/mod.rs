pub(crate) mod backend;
pub(crate) mod crossterm;
pub(crate) mod termwiz;

pub(crate) use backend::{
    PresentReceipt, TerminalBackend, TerminalEvent, is_terminal_worker_stopped,
};

pub(crate) fn enter_default() -> anyhow::Result<impl TerminalBackend> {
    termwiz::TermwizBackend::enter()
}
