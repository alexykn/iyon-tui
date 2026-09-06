use iyon_tui::binding::{HorizontalAlign, View, WrapMode};

fn main() {
    let _ = View::spacer(1);
    let _ = View::native_text_final(Vec::new(), WrapMode::default(), HorizontalAlign::Start);
}
