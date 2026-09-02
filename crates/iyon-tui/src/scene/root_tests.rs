use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::{
    History, HistoryLayout, Insets, IntoView, View,
    backend::NativeHistorySink,
    component::{Component, ComponentCx, ComponentRegistry},
    geometry::Size,
    history::transfer_native_prefix,
    physical::PhysicalRow,
};

#[derive(Debug)]
struct RootComponent(&'static str);

struct CountingRootComponent(Arc<AtomicUsize>);

impl Component for CountingRootComponent {
    fn view(&self) -> View {
        self.0.fetch_add(1, Ordering::Relaxed);
        View::text("counted").into_view()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

impl Component for RootComponent {
    fn view(&self) -> View {
        View::text(self.0).into_view()
    }

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>) {}
}

#[derive(Default)]
struct RootSink {
    rows: Vec<PhysicalRow>,
    accepted: usize,
}

impl NativeHistorySink for RootSink {
    type Error = ();

    fn insert_history_rows(&mut self, rows: &[PhysicalRow]) -> Result<usize, Self::Error> {
        let accepted = self.accepted.min(rows.len());
        self.rows.extend(rows[..accepted].iter().cloned());
        Ok(accepted)
    }
}

#[test]
fn body_component_is_resolved_once_per_root_pass() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(CountingRootComponent(calls.clone()));
    let mut history = History::new();
    history.push("history").unwrap();

    resolve_root_scene(
        &Scene::with_history(history, View::component(handle)),
        &registry,
        Size::new(20, 6),
    )
    .unwrap();

    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn body_only_root_has_no_history_overlay() {
    let registry = ComponentRegistry::new();
    let resolved = resolve_root_scene(&Scene::new("body"), &registry, Size::new(10, 6)).unwrap();
    assert!(resolved.history_overlay.is_none());
    assert_eq!(resolved.history_height, 0);
    assert_eq!(resolved.body_height, 1);
    assert!(resolved.scene.mounts.is_empty());
}

#[test]
fn history_and_body_use_remaining_height_and_terminal_width() {
    let mut history = History::new();
    history.push("H1\nH2\nH3\nH4\nH5\nH6").unwrap();
    let body = View::vertical(|column| {
        column.child("B1");
        column.child("B2");
    });
    let resolved = resolve_root_scene(
        &Scene::with_history(history, body),
        &ComponentRegistry::new(),
        Size::new(10, 8),
    )
    .unwrap();
    let layout = layout_resolved_scene(&resolved.scene, Size::new(10, 8));
    let root = layout.tree.node(layout.tree.root);
    let history_node = layout.tree.node(root.children[0]);
    let body_node = layout.tree.node(root.children[1]);
    assert_eq!(resolved.history_height, 6);
    assert_eq!(resolved.body_height, 2);
    assert_eq!(history_node.rect, crate::geometry::Rect::new(0, 0, 10, 6));
    assert_eq!(body_node.rect, crate::geometry::Rect::new(0, 6, 10, 2));
}

#[test]
fn history_tracks_all_space_left_by_intrinsic_body() {
    let body = View::text("B1\nB2\nB3\nB4");
    for (height, expected) in [(24, 20), (40, 36), (10, 6)] {
        let mut history = History::new();
        history.push("history").unwrap();
        let resolved = resolve_root_scene(
            &Scene::with_history(history, body.clone().into_view()),
            &ComponentRegistry::new(),
            Size::new(20, height),
        )
        .unwrap();
        assert_eq!(resolved.history_height, expected);
    }
}

#[test]
fn history_follow_end_stays_above_body() {
    let mut history = History::new();
    history.push("H1\nH2\nH3\nH4\nH5\nH6").unwrap();
    let resolved = resolve_root_scene(
        &Scene::with_history(history, "B1\nB2"),
        &ComponentRegistry::new(),
        Size::new(10, 6),
    )
    .unwrap();
    let rows = crate::presentation::layout::compile_bounded_view_with_overlay(
        &resolved.scene.view,
        Size::new(10, 6),
        &resolved.scene.overlay,
    )
    .rows
    .into_iter()
    .map(|row| row.plain_text())
    .collect::<Vec<_>>();
    assert_eq!(rows, ["H3", "H4", "H5", "H6", "B1", "B2"]);
}

#[test]
fn narrow_body_does_not_narrow_history() {
    let mut history = History::new();
    history.push("H").unwrap();
    let resolved = resolve_root_scene(
        &Scene::with_history(history, View::text("B").fit_width()),
        &ComponentRegistry::new(),
        Size::new(20, 4),
    )
    .unwrap();
    let layout = layout_resolved_scene(&resolved.scene, Size::new(20, 4));
    let root = layout.tree.node(layout.tree.root);
    assert_eq!(layout.tree.node(root.children[0]).rect.width, 20);
    assert_eq!(layout.tree.node(root.children[1]).rect.width, 20);
}

#[test]
fn body_exhaustion_gives_history_zero_height_but_keeps_live_mounted() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(RootComponent("live"));
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    let scene = Scene::with_history(history, "B1\nB2\nB3");
    let resolved = resolve_root_scene(&scene, &registry, Size::new(10, 3)).unwrap();
    assert_eq!(resolved.history_height, 0);
    assert_eq!(
        resolved.scene.mounts.ids().collect::<Vec<_>>(),
        [handle.id()]
    );
    let layout = layout_resolved_scene(&resolved.scene, Size::new(10, 3));
    let root = layout.tree.node(layout.tree.root);
    assert_eq!(layout.tree.node(root.children[0]).rect.height, 0);
}

#[test]
fn duplicate_component_across_history_and_body_uses_one_session() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(RootComponent("duplicate"));
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    let scene = Scene::with_history(history, View::component(handle));
    assert_eq!(
        resolve_root_scene(&scene, &registry, Size::new(10, 4)),
        Err(ResolveError::DuplicateComponent { id: handle.id() })
    );
}

#[test]
fn root_mount_order_is_history_then_body() {
    let mut registry = ComponentRegistry::new();
    let a = registry.register(RootComponent("A"));
    let b = registry.register(RootComponent("B"));
    let c = registry.register(RootComponent("C"));
    let mut history = History::new();
    history.push(View::component(a)).unwrap();
    history.push(View::component(b)).unwrap();
    let resolved = resolve_root_scene(
        &Scene::with_history(history, View::component(c)),
        &registry,
        Size::new(10, 3),
    )
    .unwrap();
    assert_eq!(
        resolved.scene.mounts.ids().collect::<Vec<_>>(),
        [a.id(), b.id(), c.id()]
    );
}

#[test]
fn zero_dimensions_preserve_semantic_resolution_without_fake_rows() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(RootComponent("live"));
    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    let scene = Scene::with_history(history, View::component(handle));
    assert!(matches!(
        resolve_root_scene(&scene, &registry, Size::new(0, 0)),
        Err(ResolveError::DuplicateComponent { .. })
    ));

    let mut history = History::new();
    history.push(View::component(handle)).unwrap();
    let resolved = resolve_root_scene(
        &Scene::with_history(history, "body"),
        &registry,
        Size::new(0, 0),
    )
    .unwrap();
    assert_eq!(resolved.history_height, 0);
    assert_eq!(
        resolved.scene.mounts.ids().collect::<Vec<_>>(),
        [handle.id()]
    );
}

#[test]
fn frozen_history_overlay_stays_inside_history_track_above_body() {
    let mut history = History::new();
    history.set_layout(HistoryLayout::from_parts(Insets::ZERO, 0));
    history.push("A\nB\nC").unwrap();
    let expected_view = View::text("A\nB\nC").into_view();
    let expected =
        crate::presentation::layout::compile_bounded_view(&expected_view, Size::new(10, 3))
            .rows
            .into_iter()
            .map(|row| row.placed(10, 0))
            .collect::<Vec<_>>();
    let mut sink = RootSink {
        accepted: 1,
        ..RootSink::default()
    };
    transfer_native_prefix(&mut history, &mut sink, 10, 1).unwrap();

    let resolved = resolve_root_scene(
        &Scene::with_history(history, "body"),
        &ComponentRegistry::new(),
        Size::new(10, 5),
    )
    .unwrap();
    let overlay = resolved.history_overlay.as_ref().unwrap();
    assert_eq!(overlay.rows, expected[1..].to_vec());
    assert!(usize::from(overlay.row) + overlay.rows.len() <= usize::from(resolved.history_height));
    assert_eq!(resolved.body_height, 1);
    let layout = layout_resolved_scene(&resolved.scene, Size::new(10, 5));
    let root = layout.tree.node(layout.tree.root);
    assert_eq!(
        layout.tree.node(root.children[1]).rect.y,
        resolved.history_height
    );
}
