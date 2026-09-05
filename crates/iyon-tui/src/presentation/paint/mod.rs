//! Backend-neutral semantic style and decoration resolution.

mod decoration;
mod text;
mod theme;
mod view;

pub(crate) use decoration::{paint_border, paint_border_at};
pub(crate) use text::TextGeometryCache;
#[cfg(test)]
pub(crate) use text::{reset_text_geometry_builds, row_from_string, text_geometry_builds};
pub(crate) use theme::{StyleContext, ThemeResolver};
pub(crate) use view::{PaintCache, ViewPainter};
