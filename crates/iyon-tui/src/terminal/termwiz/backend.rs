use std::{
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result, anyhow};
use crossterm::event::Event;
use tokio::sync::{mpsc::error::TryRecvError, oneshot};

use crate::{
    backend::NativeHistorySink,
    geometry::Size,
    physical::PhysicalRow,
    scene::PreparedSceneFrame,
    terminal::{PresentReceipt, TerminalBackend, TerminalEvent},
};

use super::worker::{Startup, TerminalCommand};

pub(crate) struct TermwizBackend {
    commands: Sender<TerminalCommand>,
    events: tokio::sync::mpsc::UnboundedReceiver<Result<Event>>,
    input: Option<crate::terminal::crossterm::EventReader>,
    size: Size,
    worker: Option<JoinHandle<()>>,
    restored: bool,
}

impl TermwizBackend {
    pub(crate) fn enter() -> Result<Self> {
        let (commands, command_receiver) = mpsc::channel();
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("iyon-terminal".to_string())
            .spawn(move || super::worker::run(command_receiver, startup_sender))
            .context("spawn terminal worker")?;

        let startup = match startup_receiver.recv() {
            Ok(startup) => startup,
            Err(error) => {
                let _ = worker.join();
                return Err(anyhow!(error)).context("terminal worker startup failed");
            }
        }?;

        let (events_sender, events) = tokio::sync::mpsc::unbounded_channel();
        let input = match crate::terminal::crossterm::EventReader::start(events_sender) {
            Ok(input) => input,
            Err(error) => {
                let (reply, receiver) = mpsc::channel();
                let _ = commands.send(TerminalCommand::Restore { reply });
                let _ = receiver.recv();
                let _ = worker.join();
                return Err(error);
            }
        };

        Ok(Self::from_startup(commands, events, input, worker, startup))
    }

    fn from_startup(
        commands: Sender<TerminalCommand>,
        events: tokio::sync::mpsc::UnboundedReceiver<Result<Event>>,
        input: crate::terminal::crossterm::EventReader,
        worker: JoinHandle<()>,
        startup: Startup,
    ) -> Self {
        Self {
            commands,
            events,
            input: Some(input),
            size: startup.size,
            worker: Some(worker),
            restored: false,
        }
    }

    fn send<T>(&self, command: TerminalCommand, receiver: mpsc::Receiver<Result<T>>) -> Result<T> {
        self.commands
            .send(command)
            .map_err(|_| crate::terminal::backend::terminal_worker_stopped())?;
        receiver.recv().context("terminal worker reply lost")?
    }

    fn map_event(&mut self, event: Event) -> Option<TerminalEvent> {
        if let Event::Resize(width, height) = event {
            self.size = Size::new(width, height);
            return Some(TerminalEvent::Resize);
        }
        crate::terminal::crossterm::map_event(event)
    }
}

impl NativeHistorySink for TermwizBackend {
    type Error = anyhow::Error;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        let (reply, receiver) = mpsc::channel();
        self.send(
            TerminalCommand::InsertHistory {
                rows: rows.to_vec(),
                reply,
            },
            receiver,
        )
    }
}

impl TerminalBackend for TermwizBackend {
    fn try_next_event(&mut self) -> Result<Option<TerminalEvent>> {
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    if let Some(event) = self.map_event(event?) {
                        return Ok(Some(event));
                    }
                }
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => {
                    return Err(anyhow!("terminal input closed"));
                }
            }
        }
    }

    fn viewport(&mut self) -> Result<Size> {
        Ok(self.size)
    }

    fn begin_frame(&mut self, frame: &PreparedSceneFrame) -> Result<PresentReceipt> {
        let (reply, receiver) = oneshot::channel();
        let desired = super::lower::desired_surface(frame);
        self.commands
            .send(TerminalCommand::Present { desired, reply })
            .map_err(|_| crate::terminal::backend::terminal_worker_stopped())?;
        Ok(receiver)
    }

    fn position_after_final_frame(&mut self) -> Result<()> {
        let (reply, receiver) = mpsc::channel();
        self.send(TerminalCommand::PositionAfterFinalFrame { reply }, receiver)
    }

    fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        if let Some(mut input) = self.input.take() {
            input.stop();
        }
        let (reply, receiver) = mpsc::channel();
        let result = self.send(TerminalCommand::Restore { reply }, receiver);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        result
    }
}

impl Drop for TermwizBackend {
    fn drop(&mut self) {
        if !self.restored {
            let _ = self.restore();
        }
    }
}
