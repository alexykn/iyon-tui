use iyon_tui::{
    Component, ComponentCx, ComponentHandle, FlowBoundary, History, HistoryError, HistoryLayout,
    HistoryUnitId, Insets, IntoView, View,
};

#[derive(Debug)]
struct PublicComponent;

impl Component for PublicComponent {
    fn view(&self) -> View {
        View::text("component").into_view()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

fn live_signature<C: Component>(handle: ComponentHandle<C>) -> Result<HistoryUnitId, HistoryError> {
    let mut history = History::new();
    let unit = history.push(View::component(handle))?;
    history.freeze(unit, "finished")?;
    Ok(unit)
}

#[test]
fn public_history_static_live_layout_and_boundary_api() {
    let mut history = History::default();
    let layout = HistoryLayout::from_parts(Insets::all(1), 2);
    history.set_layout(layout);
    assert_eq!(history.layout(), layout);
    assert_eq!(history.layout().padding(), Insets::all(1));
    assert_eq!(history.layout().gap(), 2);

    let first = history.push("A").unwrap();
    let attached = history
        .push_with_boundary("B", FlowBoundary::AttachToPrevious)
        .unwrap();
    assert_ne!(first, attached);
    assert_eq!(first, first);
    let copied = first;
    assert_eq!(copied, first);
    assert!(format!("{first:?}").contains("HistoryUnitId"));
}

#[test]
fn public_error_has_standard_error_contract() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<HistoryError>();
    let _ = live_signature::<PublicComponent>;
}
