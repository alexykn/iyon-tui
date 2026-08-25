use super::*;
use crate::presentation::{IntoView, View, layout::compile_view_with_overlay};

#[derive(Debug)]
struct Counter {
    value: u32,
}

impl Component for Counter {
    fn view(&self) -> View {
        View::text(self.value.to_string()).into_view()
    }
}

#[derive(Debug)]
struct SameVisual;

impl Component for SameVisual {
    fn view(&self) -> View {
        View::text("same").into_view()
    }
}

#[derive(Debug)]
struct Nested {
    child: View,
}

impl Component for Nested {
    fn view(&self) -> View {
        self.child.clone()
    }
}

#[test]
fn ids_are_unique_monotonic_and_never_reused_after_remove() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(Counter { value: 1 });
    let first_id = first.id().value();
    assert!(registry.remove(first).is_some());

    let second = registry.register(Counter { value: 2 });
    assert!(second.id().value() > first_id);
    assert_ne!(first.id(), second.id());
}

#[test]
fn typed_handles_resolve_only_their_own_state() {
    let mut registry = ComponentRegistry::new();
    let counter = registry.register(Counter { value: 7 });
    let wrong_type = ComponentHandle::<SameVisual>::from_id(counter.id());

    assert!(registry.contains(counter));
    assert!(!registry.contains(wrong_type));
    assert_eq!(registry.with(wrong_type, |_| ()), None);
    assert_eq!(registry.with_mut(wrong_type, |_| ()), None);
    assert!(registry.remove(wrong_type).is_none());
    assert_eq!(registry.with(counter, |value| value.value), Some(7));
}

#[test]
fn handles_from_another_registry_never_alias() {
    let mut first = ComponentRegistry::new();
    let handle = first.register(Counter { value: 1 });
    let second = ComponentRegistry::new();

    assert!(!second.contains(handle));
    assert_eq!(second.with(handle, |_| ()), None);
}

#[test]
fn revisions_only_change_after_successful_mutable_access() {
    let mut registry = ComponentRegistry::new();
    let counter = registry.register(Counter { value: 1 });
    let wrong_type = ComponentHandle::<SameVisual>::from_id(counter.id());

    assert_eq!(registry.revision(counter).unwrap().value(), 0);
    registry.with(counter, |_| ());
    assert_eq!(registry.revision(counter).unwrap().value(), 0);
    assert_eq!(registry.revision(counter).unwrap().value(), 0);
    registry.with_mut(wrong_type, |_| ());
    assert_eq!(registry.revision(counter).unwrap().value(), 0);
    registry.with_mut(counter, |value| value.value += 1);
    assert_eq!(registry.revision(counter).unwrap().value(), 1);
    registry.with_mut(counter, |_| {});
    assert_eq!(registry.revision(counter).unwrap().value(), 2);
}

#[test]
fn with_mut_changes_the_next_resolution_snapshot() {
    let mut registry = ComponentRegistry::new();
    let counter = registry.register(Counter { value: 1 });
    registry.with_mut(counter, |value| value.value = 9);

    assert_eq!(registry.with(counter, |value| value.value), Some(9));
    let resolved = crate::scene::resolve_scene(&View::component(counter), &registry).unwrap();
    assert_eq!(
        compile_view_with_overlay(&resolved.view, 20, &resolved.overlay).rows[0].plain_text(),
        "9"
    );
}

#[test]
fn resolved_components_have_stable_identity_and_distinct_visual_ownership() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(SameVisual);
    let second = registry.register(SameVisual);
    let first = crate::scene::resolve_scene(&View::component(first), &registry).unwrap();
    let second = crate::scene::resolve_scene(&View::component(second), &registry).unwrap();

    assert_ne!(first.view, second.view);
    assert_eq!(
        compile_view_with_overlay(&first.view, 20, &first.overlay),
        compile_view_with_overlay(&second.view, 20, &second.overlay)
    );
}

#[test]
fn stale_handles_do_not_resolve_but_existing_views_remain_compilable() {
    let mut registry = ComponentRegistry::new();
    let counter = registry.register(Counter { value: 4 });
    let resolved = crate::scene::resolve_scene(&View::component(counter), &registry).unwrap();
    assert!(registry.remove(counter).is_some());
    assert!(!registry.contains(counter));
    assert!(crate::scene::resolve_scene(&View::component(counter), &registry).is_err());
    assert!(
        !compile_view_with_overlay(&resolved.view, 20, &resolved.overlay)
            .rows
            .is_empty()
    );
}

#[test]
fn nested_component_attachment_wraps_without_overwriting_the_child() {
    let mut registry = ComponentRegistry::new();
    let child_handle = registry.register(SameVisual);
    let child_view = View::component(child_handle);
    let parent = registry.register(Nested { child: child_view });

    let resolved = crate::scene::resolve_scene(&View::component(parent), &registry).unwrap();
    assert_eq!(resolved.view, View::component(parent));
    let nodes = resolved.mounts.iter().collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].id, parent.id());
    assert_eq!(nodes[1].id, child_handle.id());
    assert_eq!(
        resolved.overlay.component(child_handle.id()).unwrap().view,
        View::text("same").into_view()
    );
}

#[test]
fn replacing_component_descendants_preserves_the_owner_and_depth_first_order() {
    let owner = ComponentId::allocate();
    let old_child = ComponentId::allocate();
    let new_child = ComponentId::allocate();
    let node = |id, parent| MountNode {
        id,
        parent,
        revision: ComponentRevision::default(),
    };
    let mut graph = MountGraph::new(vec![node(owner, None), node(old_child, Some(owner))]);
    let replacement = MountGraph::new(vec![node(new_child, Some(owner))]);

    assert!(graph.replace_subtree(owner, replacement));
    assert_eq!(graph.ids().collect::<Vec<_>>(), vec![owner, new_child]);
    assert_eq!(graph.parent(owner), None);
    assert_eq!(graph.parent(new_child), Some(owner));
    assert_eq!(graph.subtree_ids(owner), vec![owner, new_child]);
    assert!(!graph.contains(old_child));
}

#[test]
fn component_metadata_is_physically_invisible() {
    let mut registry = ComponentRegistry::new();
    let owned = registry.register(SameVisual);
    let with_identity = crate::scene::resolve_scene(&View::component(owned), &registry).unwrap();
    let without_identity = View::text("same").into_view();

    assert_eq!(
        compile_view_with_overlay(&with_identity.view, 20, &with_identity.overlay),
        compile_view_with_overlay(
            &without_identity,
            20,
            &crate::scene::ResolutionOverlay::default(),
        )
    );
}
