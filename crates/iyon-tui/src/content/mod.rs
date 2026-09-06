//! Immutable semantic content models and their geometry-independent lowering
//! into [`crate::View`].

pub(crate) mod diff;
mod render;
pub(crate) mod text;

pub(crate) use render::Renderer;
