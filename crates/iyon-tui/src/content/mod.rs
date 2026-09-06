//! Immutable semantic content models and their geometry-independent lowering
//! into [`crate::View`].

pub(crate) mod diff;
#[cfg(test)]
mod render;
pub(crate) mod text;

#[cfg(test)]
pub(crate) use render::Renderer;
