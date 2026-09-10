pub(crate) mod backend;
pub(crate) mod crossterm;
pub(crate) mod termwiz;

pub(crate) use backend::{
    HistoryReceipt, PresentReceipt, TerminalBackend, TerminalEvent, is_terminal_worker_stopped,
};
