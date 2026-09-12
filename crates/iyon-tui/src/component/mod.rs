mod capability;
mod graph;
mod id;
mod mount;
mod registry;
mod revision;
mod slot;
mod tick;

use std::sync::Arc;

pub use capability::ComponentCx;
pub(crate) use graph::{MountGraph, MountNode};
pub use id::ComponentHandle;
pub(crate) use id::ComponentId;
pub(crate) use mount::{MountTransition, MountTransitions, MountedComponents};
pub(crate) use registry::ComponentRegistry;
pub(crate) use revision::ComponentRevision;
pub(crate) use tick::{TickOutcome, TickScheduler};

/// Immutable frame facts exposed by a mounted native control. This is the
/// only component data consumed by the direct occurrence renderer; arbitrary
/// semantic content and layout recipes do not cross this boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ControlSnapshot {
    Editor(Box<EditorSnapshot>),
    Scroll(ScrollSnapshot),
    Animation(AnimationSnapshot),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditorSnapshot {
    pub(crate) text: Arc<str>,
    pub(crate) cursor_bytes: usize,
    pub(crate) focused: bool,
    pub(crate) multiline: bool,
    pub(crate) scroll_row: usize,
    pub(crate) border: Option<crate::BorderSpec>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScrollSnapshot {
    pub(crate) viewport_rows: u32,
    pub(crate) extent_rows: u32,
    pub(crate) top_row: u32,
    pub(crate) following_end: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnimationSnapshot {
    pub(crate) active_frame: u32,
    pub(crate) frame_count: u32,
    pub(crate) running: bool,
}

/// Native component capability and control-fact declaration contract.
pub trait Component: Send + 'static {
    /// Returns immutable facts for a concrete native control, when the
    /// component owns one. Ordinary components return `None`.
    fn control_snapshot(&self) -> Option<ControlSnapshot> {
        None
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>)
    where
        Self: Sized,
    {
    }
}
