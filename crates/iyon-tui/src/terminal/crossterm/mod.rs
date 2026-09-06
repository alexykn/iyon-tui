use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::terminal::TerminalEvent;

pub(crate) mod key;

pub(crate) fn map_event(event: Event) -> Option<TerminalEvent> {
    match event {
        Event::Key(event) => key::key_stroke(event).map(TerminalEvent::Key),
        Event::Paste(text) => Some(TerminalEvent::Paste(text)),
        Event::Resize(_, _) => Some(TerminalEvent::Resize),
        Event::FocusGained | Event::FocusLost | Event::Mouse(_) => None,
    }
}

pub(crate) fn setup() -> Result<()> {
    enable_raw_mode().context("enable terminal raw mode")?;
    if let Err(error) = execute!(std::io::stdout(), EnableBracketedPaste) {
        return match disable_raw_mode() {
            Ok(()) => Err(anyhow!(error).context("enable bracketed paste")),
            Err(disable_error) => Err(anyhow!(error)
                .context("enable bracketed paste")
                .context(format!("also failed to disable raw mode: {disable_error}"))),
        };
    }
    Ok(())
}

pub(crate) fn restore() -> Result<()> {
    let mut first_error = None;
    if let Err(error) =
        execute!(std::io::stdout(), DisableBracketedPaste).context("disable bracketed paste")
    {
        first_error = Some(error);
    }
    if let Err(error) = disable_raw_mode().context("disable terminal raw mode")
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    first_error.map_or(Ok(()), Err)
}

/// Blocking TTY reader on its own thread.
///
/// The worker owns crossterm's blocking reader; a channel lets the host poll
/// events without borrowing terminal-reader state across frame boundaries.
pub(crate) struct EventReader {
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl EventReader {
    pub(crate) fn start(events: tokio::sync::mpsc::UnboundedSender<Result<Event>>) -> Result<Self> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let worker = thread::Builder::new()
            .name("iyon-terminal-input".to_string())
            .spawn(move || read_events(flag, events))
            .context("spawn terminal input thread")?;
        Ok(Self {
            shutdown,
            worker: Some(worker),
        })
    }

    pub(crate) fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for EventReader {
    fn drop(&mut self) {
        self.stop();
    }
}

fn read_events(
    shutdown: Arc<AtomicBool>,
    events: tokio::sync::mpsc::UnboundedSender<Result<Event>>,
) {
    while !shutdown.load(Ordering::SeqCst) {
        match crossterm::event::poll(Duration::from_millis(50)) {
            Ok(true) => match crossterm::event::read() {
                Ok(event) => {
                    if events.send(Ok(event)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = events.send(Err(anyhow!(error).context("read terminal event")));
                    break;
                }
            },
            Ok(false) => {}
            Err(error) => {
                let _ = events.send(Err(anyhow!(error).context("poll terminal events")));
                break;
            }
        }
    }
}
