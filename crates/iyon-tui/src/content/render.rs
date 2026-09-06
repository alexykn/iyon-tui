use crate::View;

/// Converts a semantic value into the generic presentation [`View`].
///
/// Renderers do not receive terminal geometry, parser state, clocks, or
/// stream lifecycle. Width-dependent layout remains in the View pipeline.
pub(crate) trait Renderer<Input: ?Sized> {
    fn render(&self, input: &Input) -> View;
}
