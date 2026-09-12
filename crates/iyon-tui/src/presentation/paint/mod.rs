//! Backend-neutral semantic style and decoration resolution.

mod decoration;
mod theme;

pub(crate) use decoration::{paint_border, paint_border_at};
pub(crate) use theme::{StyleContext, ThemeResolver};
