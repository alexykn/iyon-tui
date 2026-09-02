//! Backend-neutral semantic style and decoration resolution.

mod decoration;
mod text;
mod theme;
mod view;

pub(crate) use decoration::paint_border;
#[cfg(test)]
pub(crate) use text::row_from_string;
pub(crate) use theme::{StyleContext, ThemeResolver};
pub(crate) use view::{PaintCache, ViewPainter};
