use std::sync::mpsc::{Receiver, Sender, SyncSender};

use anyhow::{Context, Result, anyhow};
use termwiz::{
    caps::{Capabilities, ProbeHints},
    surface::{Change, CursorVisibility, Surface},
    terminal::{Terminal, new_terminal},
};
use tokio::sync::oneshot;

type Reply<T> = Sender<Result<T>>;
type AsyncReply<T> = oneshot::Sender<Result<T>>;

pub(crate) enum TerminalCommand {
    Present {
        desired: Surface,
        reply: AsyncReply<()>,
    },
    InsertHistory {
        rows: Vec<crate::physical::PhysicalRow>,
        reply: AsyncReply<usize>,
    },
    PositionAfterFinalFrame {
        reply: Reply<()>,
    },
    Restore {
        reply: Reply<()>,
    },
}

const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<TerminalCommand>();
};

pub(crate) struct Startup {
    pub(crate) size: crate::geometry::Size,
}

pub(crate) fn run(commands: Receiver<TerminalCommand>, startup: SyncSender<Result<Startup>>) {
    let setup = setup_terminal();
    let (mut terminal, mut presenter, size) = match setup {
        Ok(value) => value,
        Err(error) => {
            let _ = startup.send(Err(error));
            return;
        }
    };

    if startup.send(Ok(Startup { size })).is_err() {
        presenter.finish_sync_output_best_effort(&mut *terminal);
        let _ = restore_terminal(&mut *terminal);
        return;
    }

    while let Ok(command) = commands.recv() {
        if handle_command(command, &mut *terminal, &mut presenter) {
            break;
        }
    }

    presenter.finish_sync_output_best_effort(&mut *terminal);
    let _ = restore_terminal(&mut *terminal);
}

fn setup_terminal() -> Result<(
    Box<dyn Terminal + Send>,
    super::presenter::TermwizPresenter,
    crate::geometry::Size,
)> {
    let hints = ProbeHints::new_from_env().mouse_reporting(Some(false));
    let capabilities =
        Capabilities::new_with_hints(hints).context("construct terminal capabilities")?;
    let terminal = new_terminal(capabilities).context("open system terminal")?;
    let mut terminal: Box<dyn Terminal + Send> = Box::new(terminal);
    if let Err(error) = crate::terminal::crossterm::setup().context("set terminal input mode") {
        return setup_error(&mut *terminal, error);
    }

    let size = match terminal.get_screen_size().context("get terminal size") {
        Ok(size) => size,
        Err(error) => return setup_error(&mut *terminal, error),
    };
    let hidden = Change::CursorVisibility(CursorVisibility::Hidden);
    let line_breaks = Change::Text("\r\n".repeat(size.rows));
    if let Err(error) = terminal
        .render(&[hidden, line_breaks])
        .and_then(|()| terminal.flush())
        .context("establish main-screen inline viewport")
    {
        return setup_error(&mut *terminal, error);
    }

    let mut presenter = super::presenter::TermwizPresenter::new(size.cols, size.rows);
    if let Err(error) = presenter
        .present(&mut *terminal, Surface::new(size.cols, size.rows))
        .context("paint initial terminal viewport")
    {
        return setup_error(&mut *terminal, error);
    }
    Ok((
        terminal,
        presenter,
        crate::geometry::Size::new(
            u16::try_from(size.cols).context("terminal width exceeds framework range")?,
            u16::try_from(size.rows).context("terminal height exceeds framework range")?,
        ),
    ))
}

fn setup_error<T>(terminal: &mut dyn Terminal, error: anyhow::Error) -> Result<T> {
    let _ = restore_terminal(terminal);
    Err(error)
}

fn handle_command(
    command: TerminalCommand,
    terminal: &mut dyn Terminal,
    presenter: &mut super::presenter::TermwizPresenter,
) -> bool {
    match command {
        TerminalCommand::Present { desired, reply } => {
            let result = presenter.present(terminal, desired);
            let _ = reply.send(result);
            false
        }
        TerminalCommand::InsertHistory { rows, reply } => {
            let result = presenter.insert_history(terminal, &rows);
            let _ = reply.send(result);
            false
        }
        TerminalCommand::PositionAfterFinalFrame { reply } => {
            let result = presenter.position_after_final_frame(terminal);
            let _ = reply.send(result);
            false
        }
        TerminalCommand::Restore { reply } => {
            presenter.finish_sync_output_best_effort(terminal);
            let result = restore_terminal(terminal);
            let _ = reply.send(result);
            true
        }
    }
}

fn restore_terminal(terminal: &mut dyn Terminal) -> Result<()> {
    let mut first_error = None;
    if let Err(error) = terminal
        .render(&[
            Change::AllAttributes(Default::default()),
            Change::CursorVisibility(CursorVisibility::Visible),
        ])
        .and_then(|()| terminal.flush())
    {
        first_error = Some(anyhow!(error));
    }
    if let Err(error) = crate::terminal::crossterm::restore()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = terminal.flush()
        && first_error.is_none()
    {
        first_error = Some(anyhow!(error));
    }
    first_error.map_or(Ok(()), Err)
}
