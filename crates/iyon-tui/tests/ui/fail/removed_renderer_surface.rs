use iyon_tui::{DiffRenderer, Renderer, TextRenderer};
use iyon_tui::presentation::api::View;

fn main() {
    let _ = (DiffRenderer, TextRenderer);
    let _ = View;
    fn _renderer<R: Renderer<str>>() {}
}
