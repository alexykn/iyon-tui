//! High-level semantic test driving for applications.
//!
//! This feature exposes no terminal backend or physical rendering types. It
//! drives the same kernel ordering as the production runtime and returns
//! textual snapshots suitable for integration tests.

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::{
    App, AppCx, AppHandle, KeyStroke, RunError, RuntimeError, Theme, View,
    application::{KernelError, RunningApp},
    backend::NativeHistorySink,
    geometry::Size,
    physical::{PhysicalColor, PhysicalRow, Surface},
    scene::PreparedSceneFrame,
};

#[derive(Default)]
struct HeadlessSink {
    width: u16,
    height: u16,
    history: Vec<PhysicalRow>,
}

impl NativeHistorySink for HeadlessSink {
    type Error = anyhow::Error;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        self.history.extend(rows.iter().cloned());
        Ok(rows.len())
    }
}

/// A deterministic semantic application driver for integration tests.
pub struct AppHarness<State, Action, Error, Update, ViewFn> {
    app: RunningApp<State, Action, Error, Update, ViewFn>,
    sink: HeadlessSink,
    now: Instant,
    frame: PreparedSceneFrame,
}

/// Starts an application using the same initialization and initial-frame
/// ordering as the production runtime.
pub fn start<State, Action, Error, Init, Update, ViewFn>(
    app: App<State, Action, Error, Init, Update, ViewFn>,
    width: u16,
    height: u16,
) -> Result<AppHarness<State, Action, Error, Update, ViewFn>, RunError<Error>>
where
    Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>,
    Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> View,
{
    let now = Instant::now();
    let mut app = app.start(now).map_err(map_kernel_error)?;
    let mut sink = HeadlessSink {
        width,
        height,
        ..HeadlessSink::default()
    };
    let frame = app
        .prepare_frame(now, &mut sink, |sink| {
            Ok(Size::new(sink.width, sink.height))
        })
        .map_err(|error| RunError::Runtime(RuntimeError::new(error)))?;
    app.drain_deferred_pastes().map_err(map_kernel_error)?;
    app.collect_external_pending();
    Ok(AppHarness {
        app,
        sink,
        now,
        frame,
    })
}

impl<State, Action, Error, Update, ViewFn> AppHarness<State, Action, Error, Update, ViewFn>
where
    Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>,
    ViewFn: Fn(&State) -> View,
{
    pub fn handle(&self) -> AppHandle<Action> {
        self.app.handle()
    }

    pub fn key(&mut self, key: KeyStroke) -> Result<(), RunError<Error>> {
        self.app.dispatch_key(key).map_err(map_kernel_error)?;
        self.step().map(|_| ())
    }

    pub fn paste(&mut self, text: &str) -> Result<(), RunError<Error>> {
        self.app.dispatch_paste(text).map_err(map_kernel_error)?;
        self.step().map(|_| ())
    }

    /// Advances one finite ready batch. Call again when the returned value is
    /// `true`; self-rescheduling applications are intentionally not drained
    /// forever.
    pub fn step(&mut self) -> Result<bool, RunError<Error>> {
        self.app.collect_external_pending();
        let status = self.app.advance_ready(self.now).map_err(map_kernel_error)?;
        if status.dirty {
            self.redraw()?;
        }
        Ok(status.more_ready)
    }

    pub fn advance_time(&mut self, duration: Duration) -> Result<bool, RunError<Error>> {
        self.now += duration;
        self.step()
    }

    pub fn resize(&mut self, width: u16, height: u16) -> Result<(), RunError<Error>> {
        self.sink.width = width;
        self.sink.height = height;
        self.app.invalidate_frame();
        self.step().map(|_| ())
    }

    pub fn screen_lines(&self) -> Vec<String> {
        let mut lines = surface_lines(&self.frame.surface);
        if let Some(overlay) = &self.frame.history_overlay {
            for (index, row) in overlay.rows.iter().enumerate() {
                let position = usize::from(overlay.row).saturating_add(index);
                if position < lines.len() {
                    lines[position] = row.plain_text();
                }
            }
        }
        lines
    }

    pub fn native_history_lines(&self) -> Vec<String> {
        self.sink
            .history
            .iter()
            .map(PhysicalRow::plain_text)
            .collect()
    }

    pub fn is_exiting(&self) -> bool {
        self.app.is_exiting()
    }

    fn redraw(&mut self) -> Result<(), RunError<Error>> {
        self.frame = self
            .app
            .prepare_frame(self.now, &mut self.sink, |sink| {
                Ok(Size::new(sink.width, sink.height))
            })
            .map_err(|error| RunError::Runtime(RuntimeError::new(error)))?;
        Ok(())
    }
}

#[doc(hidden)]
pub fn compile_view_lines(view: &View, width: u16) -> Vec<String> {
    crate::presentation::layout::compile_view_with_theme(view, width, &crate::Theme::default())
        .rows
        .iter()
        .map(PhysicalRow::plain_text)
        .collect()
}

/// Painted attributes at the first cell of `needle` after compiling `view`.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintedFlags {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
    pub strikethrough: bool,
    pub fg_rgb: Option<(u8, u8, u8)>,
}

/// Physical cell column of the first leader whose visible text contains `needle`.
///
/// Unlike `str::find`, this is a terminal column, not a UTF-8 byte offset.
#[doc(hidden)]
pub fn cell_x_of_text(view: &View, width: u16, needle: &str) -> usize {
    cell_x_of_text_matching(view, width, needle, |_| true)
}

/// Like [`cell_x_of_text`], but only searches rows whose plain text matches `pred`.
#[doc(hidden)]
pub fn cell_x_of_text_matching(
    view: &View,
    width: u16,
    needle: &str,
    pred: impl Fn(&str) -> bool,
) -> usize {
    let block =
        crate::presentation::layout::compile_view_with_theme(view, width, &crate::Theme::default());
    for row in &block.rows {
        let text = row.plain_text();
        if !pred(&text) {
            continue;
        }
        if let Some(x) = row.cell_x_of(needle) {
            return x;
        }
    }
    panic!(
        "did not find {needle:?} in {:?}",
        block
            .rows
            .iter()
            .map(PhysicalRow::plain_text)
            .collect::<Vec<_>>()
    );
}

#[doc(hidden)]
pub fn style_at_text(view: &View, width: u16, theme: &Theme, needle: &str) -> PaintedFlags {
    let block = crate::presentation::layout::compile_view_with_theme(view, width, theme);
    for row in &block.rows {
        if let Some(index) = row.cell_x_of(needle) {
            let style = row.style_at(index).expect("painted cell");
            let fg_rgb = match style.foreground {
                Some(PhysicalColor::Rgb { r, g, b }) => Some((r, g, b)),
                _ => None,
            };
            return PaintedFlags {
                bold: style.bold,
                italic: style.italic,
                underline: style.underline,
                dim: style.dim,
                strikethrough: style.strikethrough,
                fg_rgb,
            };
        }
    }
    panic!("did not find {needle:?}");
}

fn surface_lines(surface: &Surface) -> Vec<String> {
    (0..surface.height())
        .map(|y| {
            (0..surface.width())
                .map(|x| surface.get(x, y).grapheme.as_deref().unwrap_or(" "))
                .collect()
        })
        .collect()
}

fn map_kernel_error<Error>(error: KernelError<Error>) -> RunError<Error> {
    match error {
        KernelError::Application(error) => RunError::Application(error),
        KernelError::Output(error) => RunError::Runtime(RuntimeError::new(error)),
    }
}
