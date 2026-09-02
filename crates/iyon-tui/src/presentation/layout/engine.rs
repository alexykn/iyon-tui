//! Layout orchestration for the measure, prepare, and placement pipeline.

use crate::geometry::{AxisConstraint, LayoutConstraints, Point, Rect, Size};
use crate::presentation::ir::View;
use crate::scene::ResolutionOverlay;

use super::{
    cache::LayoutCache,
    measure::{WidthIntent, measure_node},
    place::emit_prepared,
    prepare::prepare_node,
};
use crate::presentation::{ContentProvider, EmptyContentProvider};

pub(crate) fn layout_view(view: &View, constraints: LayoutConstraints) -> super::tree::LayoutTree {
    layout_view_with_overlay(view, constraints, &ResolutionOverlay::default())
}

pub(crate) fn layout_view_with_overlay(
    view: &View,
    constraints: LayoutConstraints,
    overlay: &ResolutionOverlay,
) -> super::tree::LayoutTree {
    let mut cache = LayoutCache::default();
    layout_view_with_overlay_and_cache(view, constraints, overlay, &mut cache)
}

pub(crate) fn layout_view_with_overlay_and_cache(
    view: &View,
    constraints: LayoutConstraints,
    overlay: &ResolutionOverlay,
    cache: &mut LayoutCache,
) -> super::tree::LayoutTree {
    let mut content = EmptyContentProvider;
    layout_view_with_overlay_and_cache_and_content(
        view,
        constraints,
        overlay,
        None,
        cache,
        &mut content,
    )
}

pub(crate) fn layout_view_with_overlay_and_cache_and_content(
    view: &View,
    constraints: LayoutConstraints,
    overlay: &ResolutionOverlay,
    component_scope: Option<crate::component::ComponentId>,
    cache: &mut LayoutCache,
    content: &mut dyn ContentProvider,
) -> super::tree::LayoutTree {
    layout_view_with_overlay_and_cache_in_scope_and_content(
        view,
        constraints,
        overlay,
        component_scope,
        cache,
        content,
    )
}

/// Lays out one retained component root under the component scope that owns
/// it. This is the R6b local-layout path; it avoids rebuilding the scene tree
/// when the replacement keeps the existing geometry and shape.
pub(crate) fn layout_view_with_overlay_and_cache_in_scope(
    view: &View,
    constraints: LayoutConstraints,
    overlay: &ResolutionOverlay,
    component_scope: Option<crate::component::ComponentId>,
    cache: &mut LayoutCache,
) -> super::tree::LayoutTree {
    let mut content = EmptyContentProvider;
    layout_view_with_overlay_and_cache_in_scope_and_content(
        view,
        constraints,
        overlay,
        component_scope,
        cache,
        &mut content,
    )
}

pub(crate) fn layout_view_with_overlay_and_cache_in_scope_and_content(
    view: &View,
    constraints: LayoutConstraints,
    overlay: &ResolutionOverlay,
    component_scope: Option<crate::component::ComponentId>,
    cache: &mut LayoutCache,
    content: &mut dyn ContentProvider,
) -> super::tree::LayoutTree {
    let width = constraints.width.definite().unwrap_or_else(|| {
        measure_node(
            view,
            u16::MAX,
            WidthIntent::Semantic,
            overlay,
            component_scope,
            cache,
            content,
        )
        .size
        .width
    });
    let measured = measure_node(
        view,
        width,
        WidthIntent::Semantic,
        overlay,
        component_scope,
        cache,
        content,
    );
    let prepared = prepare_node(&measured, constraints.height.definite(), cache);
    let root_clip = Rect::new(
        0,
        0,
        prepared.size.width,
        constraints
            .height
            .definite()
            .unwrap_or(prepared.size.height),
    );
    let mut nodes = Vec::new();
    let root = emit_prepared(&prepared, Point::default(), root_clip, &mut nodes);
    let mut tree = super::tree::LayoutTree {
        root,
        nodes,
        size: prepared.size,
        physically_complete: prepared.complete,
        component_roots: Default::default(),
        parents: Vec::new(),
        state_roots: Default::default(),
    };
    tree.index_component_roots();
    if matches!(constraints.height, AxisConstraint::Unbounded) {
        tree.size.height = tree.node(root).rect.height;
    }
    debug_assert!(tree.validate(), "invalid layout tree: {tree:?}");
    tree
}

pub(crate) fn measure_view(view: &View, width: u16) -> Size {
    measure_view_with_overlay(view, width, &ResolutionOverlay::default())
}

pub(crate) fn measure_view_with_overlay(
    view: &View,
    width: u16,
    overlay: &ResolutionOverlay,
) -> Size {
    let mut cache = LayoutCache::default();
    measure_view_with_overlay_and_cache(view, width, overlay, &mut cache)
}

pub(crate) fn measure_view_with_overlay_and_cache(
    view: &View,
    width: u16,
    overlay: &ResolutionOverlay,
    cache: &mut LayoutCache,
) -> Size {
    let mut content = EmptyContentProvider;
    measure_view_with_overlay_and_cache_and_content(view, width, overlay, cache, &mut content)
}

pub(crate) fn measure_view_with_overlay_and_cache_and_content(
    view: &View,
    width: u16,
    overlay: &ResolutionOverlay,
    cache: &mut LayoutCache,
    content: &mut dyn ContentProvider,
) -> Size {
    measure_node(
        view,
        width,
        WidthIntent::Semantic,
        overlay,
        None,
        cache,
        content,
    )
    .size
}
