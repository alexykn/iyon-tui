use super::super::*;
use super::TestSource;
use crate::history::projection::project;
use crate::{
    Component, ComponentCx, View,
    component::{ComponentHandle, ComponentId, ComponentRegistry},
    geometry::Size,
    presentation::{Insets, IntoView},
    scene::{LayoutSync, LayoutSynchronizer, ResolveError, layout_resolved_scene},
    stream::{build_index_call_count, reset_build_index_call_count},
};

struct LabelComponent {
    label: &'static str,
}

struct NestedFillUnit;

impl Component for NestedFillUnit {
    fn view(&self) -> View {
        View::vertical(|column| {
            column.child(
                View::text(
                    (1..=20)
                        .map(|row| format!("nested {row}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .fill_height(),
            );
        })
        .fill_width()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

struct FlexibleScrollUnit {
    pane: crate::component::ComponentHandle<crate::ScrollPane>,
}

impl Component for FlexibleScrollUnit {
    fn view(&self) -> View {
        View::vertical(|column| {
            column.fixed(1, View::text("header"));
            column.flex_max(16, View::component(self.pane));
        })
        .fill_width()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

struct FlexibleUnit;

impl Component for FlexibleUnit {
    fn view(&self) -> View {
        View::vertical(|column| {
            column.fixed(1, View::text("header"));
            column.flex_max(
                16,
                View::text(
                    (1..=20)
                        .map(|row| format!("body {row}\n"))
                        .collect::<String>(),
                ),
            );
        })
        .fill_width()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

impl Component for LabelComponent {
    fn view(&self) -> View {
        View::text(self.label).into_view()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

fn rows(history: &History, registry: &ComponentRegistry, size: Size) -> Vec<String> {
    let scene = project(history, registry, size).unwrap();
    crate::presentation::layout::compile_bounded_view_with_overlay(
        &scene.scene.view,
        size,
        &scene.scene.overlay,
    )
    .rows
    .into_iter()
    .map(|row| row.plain_text())
    .collect()
}

#[test]
fn bounded_live_unit_allocates_header_before_flexible_body() {
    let mut history = History::new();
    let mut registry = ComponentRegistry::new();
    let component = registry.register(FlexibleUnit);
    history.push(View::component(component)).unwrap();

    let rendered = rows(&history, &registry, Size::new(20, 8));
    assert_eq!(rendered[0], "header");
    assert!(rendered[1].contains("body 1"));

    let rendered = rows(&history, &registry, Size::new(20, 5));
    assert_eq!(rendered[0], "header");
    assert!(rendered[1].contains("body 1"));
}

#[test]
fn bounded_history_keeps_header_above_flex_max_scroll_pane() {
    let mut history = History::new();
    let mut registry = ComponentRegistry::new();
    let pane = registry.register(crate::ScrollPane::new(View::text(
        (1..=30)
            .map(|row| format!("pane {row}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )));
    let component = registry.register(FlexibleScrollUnit { pane });
    history.push(View::component(component)).unwrap();

    let rendered = rows(&history, &registry, Size::new(20, 8));
    assert_eq!(rendered[0], "header");
    assert!(rendered.iter().any(|row| row.contains("pane 1")));
}

#[test]
fn nested_fill_like_content_does_not_trigger_bounded_live_suffix() {
    let mut history = History::new();
    let mut registry = ComponentRegistry::new();
    let component = registry.register(NestedFillUnit);
    history.push(View::component(component)).unwrap();

    let rendered = rows(&history, &registry, Size::new(20, 5));
    assert!(rendered[0].contains("nested 16"));
    assert!(!rendered.iter().any(|row| row.trim() == "nested 1"));
}

#[test]
fn history_is_one_bottom_anchored_flow() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B").unwrap();
    history.push("C").unwrap();
    let registry = ComponentRegistry::new();

    assert_eq!(rows(&history, &registry, Size::new(8, 3)), ["A", "B", "C"]);
    history.push("D").unwrap();
    assert_eq!(rows(&history, &registry, Size::new(8, 3)), ["B", "C", "D"]);
}

#[test]
fn mixed_lifetime_append_preserves_continuous_flow() {
    let mut registry = ComponentRegistry::new();
    let live = registry.register(LabelComponent { label: "B" });
    let source = TestSource::new("D", 1, false);
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 1));
    history.push("A1\nA2").unwrap();
    history.push(View::component(live)).unwrap();
    history.push("C").unwrap();
    let stream = history.push_stream(source).unwrap();

    assert_eq!(
        rows(&history, &registry, Size::new(8, 7)),
        ["A2", "", "B", "", "C", "", "D"]
    );

    history
        .update_stream(stream, |source| {
            let snapshot = source.snapshot_with(1, 3, "D\nE");
            source.replace_snapshot(snapshot);
        })
        .unwrap();

    assert_eq!(
        rows(&history, &registry, Size::new(8, 7)),
        ["", "B", "", "C", "", "D", "E"]
    );
}

#[test]
fn boundaries_and_padding_are_parent_owned_flow_geometry() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::new(1, 1, 1, 1), 2));
    history.push("A").unwrap();
    history
        .push_with_boundary("B", FlowBoundary::AttachToPrevious)
        .unwrap();
    history.push("C").unwrap();
    let registry = ComponentRegistry::new();

    assert_eq!(
        rows(&history, &registry, Size::new(8, 8)),
        ["", "", " A", " B", "", "", " C", ""]
    );
}

#[test]
fn identical_freeze_is_visually_inert() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(LabelComponent { label: "B" });
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::new(1, 1, 1, 1), 1));
    history.push("A").unwrap();
    let live = history.push(View::component(handle)).unwrap();
    history.push("C").unwrap();
    history.push_stream(TestSource::new("D", 1, false)).unwrap();
    let before = rows(&history, &registry, Size::new(8, 9));

    history.freeze(live, "B").unwrap();

    assert_eq!(rows(&history, &registry, Size::new(8, 9)), before);
}

#[test]
fn open_stream_preserves_protected_live_band_before_following_suffix() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(LabelComponent { label: "A1\nA2" });
    let second = registry.register(LabelComponent { label: "B1\nB2" });
    let before = (1..=20)
        .map(|row| format!("S{row}"))
        .collect::<Vec<_>>()
        .join("\n");
    let after = (1..=21)
        .map(|row| format!("S{row}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut history = History::new();
    history.push(View::component(first)).unwrap();
    history.push(View::component(second)).unwrap();
    let stream = history
        .push_stream(TestSource::new(&before, before.len() as u64, false))
        .unwrap();

    assert_eq!(
        rows(&history, &registry, Size::new(10, 8)),
        ["A1", "A2", "B1", "B2", "S17", "S18", "S19", "S20"]
    );
    assert_eq!(
        project(&history, &registry, Size::new(10, 8))
            .unwrap()
            .scene
            .mounts
            .ids()
            .collect::<Vec<_>>(),
        [first.id(), second.id()]
    );

    history
        .update_stream(stream, |source| {
            source.replace_snapshot(source.snapshot_with(1, after.len() as u64, &after));
        })
        .unwrap();
    assert_eq!(
        rows(&history, &registry, Size::new(10, 8)),
        ["A1", "A2", "B1", "B2", "S18", "S19", "S20", "S21"]
    );
    assert_eq!(
        project(&history, &registry, Size::new(10, 8))
            .unwrap()
            .scene
            .mounts
            .ids()
            .collect::<Vec<_>>(),
        [first.id(), second.id()]
    );
}

#[test]
fn multiple_live_units_resolve_in_history_order_even_when_old() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(LabelComponent { label: "A" });
    let second = registry.register(LabelComponent { label: "C" });
    let mut history = History::new();
    history.push(View::component(first)).unwrap();
    history.push("B").unwrap();
    history.push(View::component(second)).unwrap();
    history.push("D").unwrap();

    let scene = project(&history, &registry, Size::new(8, 1)).unwrap();
    assert_eq!(
        scene.scene.mounts.ids().collect::<Vec<_>>(),
        [first.id(), second.id()]
    );
    assert_eq!(rows(&history, &registry, Size::new(8, 1)), ["D"]);
}

#[derive(Debug)]
struct AllocationAware {
    sizes: Vec<Size>,
}

impl Component for AllocationAware {
    fn view(&self) -> View {
        View::text("allocated").fill_width().into_view()
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.on_layout_changed(Self::layout_changed);
    }
}

impl AllocationAware {
    fn layout_changed(component: &mut Self, size: Size) {
        component.sizes.push(size);
    }
}

#[test]
fn offscreen_live_retains_width_dependent_allocation() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(AllocationAware { sizes: Vec::new() });
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    history.push("visible").unwrap();
    let mut synchronizer = LayoutSynchronizer::default();

    for width in [8, 13] {
        let size = Size::new(width, 1);
        let scene = project(&history, &registry, size).unwrap();
        let layout = layout_resolved_scene(&scene.scene, size);
        let geometry = layout.components.entries.get(&handle.id()).unwrap();
        assert_eq!(scene.scene.mounts.ids().collect::<Vec<_>>(), [handle.id()]);
        assert_eq!(geometry.visible, None);
        assert_eq!(geometry.content.size().width, width);
        assert_eq!(
            synchronizer.synchronize(
                &scene.scene.mounts,
                &scene.scene.capabilities,
                &layout.components,
                &mut registry,
            ),
            LayoutSync::Dirty
        );
    }

    assert_eq!(
        registry.with(handle, |component| {
            component
                .sizes
                .iter()
                .map(|size| size.width)
                .collect::<Vec<_>>()
        }),
        Some(vec![8, 13])
    );
}

#[test]
fn duplicate_component_across_history_units_is_rejected() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(LabelComponent { label: "same" });
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    history.push(View::component(handle)).unwrap();

    assert_eq!(
        project(&history, &registry, Size::new(8, 1)),
        Err(ResolveError::DuplicateComponent { id: handle.id() })
    );
}

#[test]
fn partial_live_viewport_translates_component_geometry_with_painted_rows() {
    let mut registry = ComponentRegistry::new();
    let child = registry.register(TallComponent);
    let parent = registry.register(NestedComponent { child: child.id() });
    let mut history = History::new();
    history.push(View::component(parent)).unwrap();

    let scene = project(&history, &registry, Size::new(12, 3)).unwrap();
    let layout = layout_resolved_scene(&scene.scene, Size::new(12, 3));
    let child_geometry = layout.components.entries.get(&child.id()).unwrap();
    assert_eq!(child_geometry.visible.map(|rect| rect.y), Some(0));
    assert_eq!(child_geometry.visible.map(|rect| rect.height), Some(1));
    assert_eq!(
        crate::presentation::layout::compile_bounded_view_with_overlay(
            &scene.scene.view,
            Size::new(12, 3),
            &scene.scene.overlay,
        )
        .rows
        .into_iter()
        .map(|row| row.plain_text())
        .collect::<Vec<_>>(),
        ["three", "child-tail", "bottom"]
    );
}

struct TallComponent;

impl Component for TallComponent {
    fn view(&self) -> View {
        View::vertical(|column| {
            column.children(["one", "two", "three"]);
        })
    }
}

struct NestedComponent {
    child: crate::component::ComponentId,
}

impl Component for NestedComponent {
    fn view(&self) -> View {
        View::vertical(|column| {
            column.child("top");
            column.child(View::vertical(|nested| {
                nested.child(View::component(crate::component::ComponentHandle::<
                    TallComponent,
                >::from_id(self.child)));
                nested.child("child-tail");
            }));
            column.child("bottom");
        })
    }
}

#[test]
fn padding_slack_and_top_cut_are_distinct_flow_cases() {
    let mut short = History::new();
    short.set_layout(HistoryLayout::from_parts(Insets::new(1, 1, 2, 1), 0));
    short.push("A").unwrap();
    let registry = ComponentRegistry::new();
    assert_eq!(
        rows(&short, &registry, Size::new(8, 5)),
        ["", "", " A", "", ""]
    );

    let mut long = History::new();
    long.set_layout(HistoryLayout::from_parts(Insets::new(1, 0, 0, 0), 0));
    long.push("A").unwrap();
    long.push("B").unwrap();
    long.push("C").unwrap();
    assert_eq!(rows(&long, &registry, Size::new(8, 2)), ["B", "C"]);
}

#[test]
fn a_top_cut_inside_a_gap_keeps_only_the_gap_suffix() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::default());
    history.push("A").unwrap();
    history.push("B").unwrap();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 3));
    let registry = ComponentRegistry::new();

    assert_eq!(rows(&history, &registry, Size::new(8, 3)), ["", "", "B"]);
}

#[test]
fn width_and_height_changes_recompute_the_bottom_suffix() {
    let mut history = History::new();
    history.push("abcdef").unwrap();
    history.push("B").unwrap();
    let registry = ComponentRegistry::new();

    assert_eq!(rows(&history, &registry, Size::new(6, 1)), ["B"]);
    assert_eq!(rows(&history, &registry, Size::new(3, 2)), ["def", "B"]);

    history.push("C").unwrap();
    assert_eq!(
        rows(&history, &registry, Size::new(8, 3)),
        ["abcdef", "B", "C"]
    );
}

#[test]
fn sealing_a_stream_with_identical_semantics_is_visually_inert() {
    let source = TestSource::new("A\nB", 3, false);
    let mut history = History::new();
    let handle = history.push_stream(source).unwrap();
    let registry = ComponentRegistry::new();
    let before = rows(&history, &registry, Size::new(8, 2));

    history.seal_stream(handle).unwrap();

    assert_eq!(rows(&history, &registry, Size::new(8, 2)), before);
}

#[test]
fn offscreen_streams_are_lazy_and_visible_streams_prepare_once() {
    let mut history = History::new();
    history
        .push_stream(TestSource::new("old", 3, true))
        .unwrap();
    history.push("B").unwrap();
    history.push("C").unwrap();
    history.push("D").unwrap();
    let registry = ComponentRegistry::new();

    reset_build_index_call_count();
    assert_eq!(rows(&history, &registry, Size::new(8, 1)), ["D"]);
    assert_eq!(build_index_call_count(), 0);

    reset_build_index_call_count();
    assert_eq!(
        rows(&history, &registry, Size::new(8, 4)),
        ["old", "B", "C", "D"]
    );
    assert_eq!(build_index_call_count(), 1);
}

#[test]
fn stream_projection_uses_shared_window_without_mutating_source() {
    let source = TestSource::new("A\nB\nC", 5, false);
    let mut history = History::new();
    history.push_stream(source.clone()).unwrap();
    let registry = ComponentRegistry::new();

    assert_eq!(rows(&history, &registry, Size::new(8, 2)), ["B", "C"]);
    assert_eq!(source.compact_calls(), 0);
}

#[test]
fn missing_and_cyclic_multi_root_resolution_keep_existing_errors() {
    let missing = ComponentHandle::<LabelComponent>::from_id(ComponentId::allocate());
    let mut missing_history = History::new();
    missing_history.push(View::component(missing)).unwrap();
    assert_eq!(
        project(&missing_history, &ComponentRegistry::new(), Size::new(8, 1)),
        Err(ResolveError::MissingComponent { id: missing.id() })
    );

    let mut registry = ComponentRegistry::new();
    let first = registry.register(CycleComponent {
        child: ComponentId::allocate(),
    });
    let second = registry.register(CycleComponent { child: first.id() });
    registry
        .with_mut(first, |component| component.child = second.id())
        .unwrap();
    let mut cyclic = History::new();
    cyclic.push(View::component(first)).unwrap();
    assert!(matches!(
        project(&cyclic, &registry, Size::new(8, 1)),
        Err(ResolveError::ComponentCycle { .. })
    ));
}

struct CycleComponent {
    child: ComponentId,
}

impl Component for CycleComponent {
    fn view(&self) -> View {
        View::component(ComponentHandle::<CycleComponent>::from_id(self.child))
    }
}

#[test]
fn live_component_mutation_is_reflected_by_the_next_projection() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(MutableLabel {
        label: "before".to_owned(),
    });
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    assert_eq!(rows(&history, &registry, Size::new(8, 1)), ["before"]);

    registry
        .with_mut(handle, |component| component.label = "after".to_owned())
        .unwrap();

    assert_eq!(rows(&history, &registry, Size::new(8, 1)), ["after"]);
}

struct MutableLabel {
    label: String,
}

impl Component for MutableLabel {
    fn view(&self) -> View {
        View::text(self.label.clone()).into_view()
    }
}

#[cfg(feature = "perf-counters")]
#[test]
fn retained_history_tail_updates_do_not_remeasure_the_static_prefix() {
    let _lock = crate::perf::test_lock();
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(MutableLabel {
        label: "tail-0".to_owned(),
    });
    let mut history = History::new();
    for index in 0..1_000 {
        history.push(format!("static-{index}")).unwrap();
    }
    history.push(View::component(handle)).unwrap();
    let size = Size::new(32, 8);

    rows(&history, &registry, size);
    registry
        .with_mut(handle, |component| component.label = "tail-1".to_owned())
        .unwrap();
    rows(&history, &registry, size);
    crate::perf::reset();

    registry
        .with_mut(handle, |component| component.label = "tail-2".to_owned())
        .unwrap();
    rows(&history, &registry, size);
    let counters = crate::perf::snapshot();

    assert_eq!(
        counters.value(crate::perf::Counter::HistoryUnitsMeasured),
        1,
        "only the mutated live tail may be remeasured"
    );
    assert!(
        counters.value(crate::perf::Counter::HistoryCachedHeightHits) >= 999,
        "the static prefix must be served from the retained height cache"
    );
    assert!(
        counters.value(crate::perf::Counter::HistoryUnitsExamined) <= 10,
        "tail selection should inspect only a viewport-sized suffix"
    );
}

#[cfg(feature = "perf-counters")]
#[test]
fn stream_height_cache_reuses_history_geometry_and_rebuilds_only_stream_suffix() {
    let _lock = crate::perf::test_lock();
    let registry = ComponentRegistry::new();
    let mut history = History::new();
    for index in 0..1_000 {
        history.push(format!("static-{index}")).unwrap();
    }
    let stream = history.push_stream(crate::TextStream::new()).unwrap();
    let size = Size::new(32, 8);

    history
        .update_stream(stream, |source| source.push("first line\npending"))
        .unwrap();
    rows(&history, &registry, size);
    crate::perf::reset();

    rows(&history, &registry, size);
    let warm = crate::perf::snapshot();
    assert_eq!(
        warm.value(crate::perf::Counter::HistoryUnitsMeasured),
        0,
        "a stable stream revision must reuse its retained History height"
    );
    assert!(
        warm.value(crate::perf::Counter::HistoryCachedHeightHits) >= 1_001,
        "static units and the stream must all hit retained height metadata"
    );

    history
        .update_stream(stream, |source| source.push(" second line\n"))
        .unwrap();
    rows(&history, &registry, size);
    let updated = crate::perf::snapshot();
    assert!(
        updated.value(crate::perf::Counter::HistoryUnitsMeasured) <= 10,
        "stream updates may prepare only a bounded tail, not the static prefix"
    );
    assert!(
        updated.value(crate::perf::Counter::HistoryCachedHeightHits) >= 1_000,
        "the static prefix must remain cached when the stream advances"
    );
    assert!(
        updated.value(crate::perf::Counter::StreamStableRowsReused) > 0,
        "stream row work must reuse the stable prefix"
    );
}

#[test]
fn zero_size_projection_still_resolves_live_units() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(LabelComponent { label: "live" });
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();

    let scene = project(&history, &registry, Size::new(0, 0)).unwrap();
    assert_eq!(scene.scene.mounts.ids().collect::<Vec<_>>(), [handle.id()]);
    assert!(rows(&history, &registry, Size::new(0, 0)).is_empty());
}

#[derive(Default)]
struct TestSink {
    rows: Vec<crate::physical::PhysicalRow>,
}

impl crate::backend::NativeHistorySink for TestSink {
    type Error = ();

    fn insert_history_rows(
        &mut self,
        rows: &[crate::physical::PhysicalRow],
    ) -> Result<usize, Self::Error> {
        self.rows.extend(rows.iter().cloned());
        Ok(rows.len())
    }
}

fn assert_resident_frontier_contiguous(
    history: &History,
    registry: &ComponentRegistry,
    size: Size,
    expected_first_row: &str,
) {
    assert!(
        history.native.has_physical_rows(),
        "assertion applies only when native physical rows exist"
    );
    let rendered = rows(history, registry, size);
    assert!(!rendered.is_empty(), "rendered viewport must not be empty");
    assert_eq!(
        rendered[0].trim(),
        expected_first_row,
        "first visible resident row must begin at the native boundary (no blank gap above it)"
    );
}

#[test]
fn fresh_history_still_bottom_follows() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B").unwrap();
    let registry = ComponentRegistry::new();
    assert_eq!(
        rows(&history, &registry, Size::new(8, 4)),
        ["", "", "A", "B"]
    );
}

#[test]
fn native_prefix_underflow_places_slack_after_resident_suffix() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B").unwrap();
    history.push("C").unwrap();
    history.push("D").unwrap();
    history.push("E").unwrap();
    let mut sink = TestSink::default();
    let outcome1 = transfer_native_prefix(&mut history, &mut sink, 8, 2).unwrap();
    assert_eq!(outcome1.inserted, 1);
    let outcome2 = transfer_native_prefix(&mut history, &mut sink, 8, 1).unwrap();
    assert_eq!(outcome2.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 2);
    assert!(history.native.has_physical_rows());

    let registry = ComponentRegistry::new();
    assert_eq!(
        rows(&history, &registry, Size::new(8, 5)),
        ["C", "D", "E", "", ""]
    );
    assert_resident_frontier_contiguous(&history, &registry, Size::new(8, 5), "C");
}

#[test]
fn body_shrink_does_not_create_gap_above_resident_history() {
    let mut history = History::new();
    history.push("A").unwrap();
    history.push("B").unwrap();
    history.push("C").unwrap();
    history.push("D").unwrap();
    history.push("E").unwrap();
    let mut sink = TestSink::default();
    let outcome1 = transfer_native_prefix(&mut history, &mut sink, 8, 2).unwrap();
    assert_eq!(outcome1.inserted, 1);
    let outcome2 = transfer_native_prefix(&mut history, &mut sink, 8, 1).unwrap();
    assert_eq!(outcome2.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 2);

    let registry = ComponentRegistry::new();
    assert_eq!(rows(&history, &registry, Size::new(8, 3)), ["C", "D", "E"]);

    // Body shrinks / History viewport expands from 3 to 6:
    // First resident row C remains at y=0, not shifted down to y=3
    assert_eq!(
        rows(&history, &registry, Size::new(8, 6)),
        ["C", "D", "E", "", "", ""]
    );
    assert_resident_frontier_contiguous(&history, &registry, Size::new(8, 6), "C");
}

#[test]
fn stream_growth_consumes_bottom_slack_before_native_transfer() {
    let mut history = History::new();
    history.push("S1").unwrap();
    history.push("S2").unwrap();
    history.push("S3").unwrap();
    history.push("S4").unwrap();
    history.push("S5").unwrap();
    let mut sink = TestSink::default();

    let outcome1 = transfer_native_prefix(&mut history, &mut sink, 10, 2).unwrap();
    assert_eq!(outcome1.inserted, 1);
    let outcome2 = transfer_native_prefix(&mut history, &mut sink, 10, 1).unwrap();
    assert_eq!(outcome2.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 2);

    let registry = ComponentRegistry::new();
    // 3 resident rows (S3, S4, S5) in viewport height 5 => 2 bottom slack rows
    assert_eq!(
        rows(&history, &registry, Size::new(10, 5)),
        ["S3", "S4", "S5", "", ""]
    );

    // Row growth consumes 1 bottom slack row, physical_rows_inserted remains 2
    history.push("S6").unwrap();
    assert_eq!(history.native.physical_rows_inserted, 2);
    assert_eq!(
        rows(&history, &registry, Size::new(10, 5)),
        ["S3", "S4", "S5", "S6", ""]
    );

    // Row growth consumes final bottom slack row, physical_rows_inserted remains 2
    history.push("S7").unwrap();
    assert_eq!(history.native.physical_rows_inserted, 2);
    assert_eq!(
        rows(&history, &registry, Size::new(10, 5)),
        ["S3", "S4", "S5", "S6", "S7"]
    );

    // Row growth overflows height 5 by 1 row
    history.push("S8").unwrap();
    let mut session = crate::scene::ResolveSession::new(&registry);
    let parts = project_into_session_for_host(
        &history,
        Size::new(10, 5),
        &mut session,
        HistoryViewportAnchor::FollowEnd,
    )
    .unwrap();
    assert_eq!(parts.overflow_rows, 1);

    // Now transferring the 1 overflow row advances native frontier to 3
    let outcome = transfer_native_prefix(&mut history, &mut sink, 10, parts.overflow_rows).unwrap();
    assert_eq!(outcome.inserted, 1);
    assert_eq!(history.native.physical_rows_inserted, 3);
    assert_eq!(
        rows(&history, &registry, Size::new(10, 5)),
        ["S4", "S5", "S6", "S7", "S8"]
    );
}

#[test]
fn frozen_overlay_underflow_is_not_shifted_by_bottom_slack() {
    let mut history = History::new();
    history.push("A1\nA2\nA3\nA4").unwrap();
    history.push("B").unwrap();
    let mut sink = TestSink::default();
    let outcome = transfer_native_prefix(&mut history, &mut sink, 10, 2).unwrap();
    assert_eq!(outcome.inserted, 2);
    assert!(history.native.has_physical_rows());
    assert!(history.native.frozen_static.is_some());

    let registry = ComponentRegistry::new();
    // Project into height 6 (resident: A3, A4, B = 3 rows, slack = 3 rows)
    let projection = project(&history, &registry, Size::new(10, 6)).unwrap();
    let overlay = projection.frozen_overlay.expect("frozen overlay exists");
    // Row must be 0 (the on-screen location of the resident row), NOT 3 (slack)!
    assert_eq!(overlay.row, 0);
    assert_eq!(overlay.rows.len(), 2);
    assert_eq!(overlay.rows[0].plain_text(), "A3");
    assert_eq!(overlay.rows[1].plain_text(), "A4");

    let rendered = crate::presentation::layout::compile_bounded_view_with_overlay(
        &projection.scene.view,
        Size::new(10, 6),
        &projection.scene.overlay,
    )
    .rows
    .into_iter()
    .map(|row| row.plain_text())
    .collect::<Vec<_>>();
    assert_eq!(rendered, ["", "", "B", "", "", ""]);
}

#[test]
fn native_frontier_anchor_pins_front_items() {
    let mut history = History::new();
    history.push("A1\nA2\nA3\nA4").unwrap();
    history.push("B1\nB2\nB3").unwrap();
    let registry = ComponentRegistry::new();

    // In FollowEnd mode, height 3 shows suffix ["B1", "B2", "B3"]
    assert_eq!(
        rows(&history, &registry, Size::new(10, 3)),
        ["B1", "B2", "B3"]
    );

    // In NativeFrontier mode, height 3 shows prefix ["A1", "A2", "A3"]
    let pinned = project_with_anchor(
        &history,
        &registry,
        Size::new(10, 3),
        HistoryViewportAnchor::NativeFrontier,
    )
    .unwrap();
    let rendered = crate::presentation::layout::compile_bounded_view_with_overlay(
        &pinned.scene.view,
        Size::new(10, 3),
        &pinned.scene.overlay,
    )
    .rows
    .into_iter()
    .map(|row| row.plain_text())
    .collect::<Vec<_>>();
    assert_eq!(rendered, ["A1", "A2", "A3"]);
}
