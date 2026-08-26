//! Backend-neutral semantic style and decoration resolution.

mod decoration;
mod text;
mod theme;
mod view;

pub(crate) use decoration::paint_border;
pub(crate) use text::{CompiledTextRow, row_from_graphemes, row_from_string};
pub(crate) use theme::{StyleContext, ThemeResolver};
pub(crate) use view::{PaintCache, ViewPainter};
