//! Private semantic layout/compiler boundary.
//!
//! This module owns backend-neutral constraints, retained layout geometry, and
//! the compiler facade. Physical lowering lives in painting.
//!
//! Layout is a three-stage pipeline:
//!
//! 1. Measure semantic Views into width-dependent `MeasuredNodes`.
//! 2. Resolve bounded allocation into `PreparedNodes` using only measured facts.
//! 3. Place `PreparedNodes` into a `LayoutTree` without performing measurement or
//!    layout allocation.
//!
//! A semantic View subtree must never be re-measured merely because placement
//! needs geometry, and `LayoutTree` must not retain recursive clones of semantic
//! View subtrees.

mod cache;
mod damage;
mod engine;
mod geometry;
mod grid;
mod measure;
mod place;
mod prepare;
mod tracks;
mod tree;

pub(crate) use damage::DamageRegion;
#[cfg(test)]
mod tests;

use crate::{
    Theme,
    component::{ComponentId, MountGraph},
    geometry::LayoutConstraints,
    physical::PhysicalRow,
    presentation::View,
};

pub(crate) use cache::LayoutCache;
#[cfg(test)]
pub(crate) use engine::layout_view_with_overlay;
#[allow(unused_imports)]
pub(crate) use engine::{
    layout_view, layout_view_with_overlay_and_cache,
    layout_view_with_overlay_and_cache_and_content, layout_view_with_overlay_and_cache_in_scope,
    layout_view_with_overlay_and_cache_in_scope_and_content, measure_view,
    measure_view_with_overlay, measure_view_with_overlay_and_cache,
    measure_view_with_overlay_and_cache_and_content,
};
pub(crate) use tree::{
    ComponentGeometry, ComponentGeometryMap, LayoutContent, LayoutNode, LayoutNodeId, LayoutTree,
};

#[cfg(test)]
use crate::geometry::Size;
#[cfg(test)]
use crate::physical::Surface;

use super::paint::{StyleContext, TextGeometryCache, ThemeResolver, ViewPainter};

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static MEASURE_NODES: Cell<usize> = const { Cell::new(0) };
    static PREPARE_NODES: Cell<usize> = const { Cell::new(0) };
    static EMITTED_NODES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(super) fn record_measure_node() {
    MEASURE_NODES.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn record_prepare_node() {
    PREPARE_NODES.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn record_emitted_node() {
    EMITTED_NODES.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(crate) fn reset_layout_counters() {
    MEASURE_NODES.with(|count| count.set(0));
    PREPARE_NODES.with(|count| count.set(0));
    EMITTED_NODES.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn layout_counters() -> (usize, usize, usize) {
    (
        MEASURE_NODES.with(Cell::get),
        PREPARE_NODES.with(Cell::get),
        EMITTED_NODES.with(Cell::get),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LayoutBlock {
    pub(crate) width: u16,
    pub(crate) rows: Vec<PhysicalRow>,
    pub(crate) physically_complete: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ViewCompiler<'a> {
    pub(crate) theme: ThemeResolver,
    pub(crate) focused: Option<ComponentId>,
    pub(crate) graph: Option<&'a MountGraph>,
}

impl<'a> ViewCompiler<'a> {
    pub(crate) fn new(theme: &Theme) -> Self {
        Self {
            theme: ThemeResolver::new(theme),
            focused: None,
            graph: None,
        }
    }

    pub(crate) fn with_interaction(
        theme: &Theme,
        focused: Option<ComponentId>,
        graph: &'a MountGraph,
    ) -> Self {
        Self {
            theme: ThemeResolver::new(theme),
            focused,
            graph: Some(graph),
        }
    }

    pub(crate) fn with_resolver(theme: &ThemeResolver) -> Self {
        Self {
            theme: theme.clone(),
            focused: None,
            graph: None,
        }
    }

    pub(crate) fn style_context(&self, scope: Option<ComponentId>) -> StyleContext {
        StyleContext::for_scope(scope, self.focused, self.graph)
    }

    pub(crate) fn compile(&self, view: &View, max_width: u16) -> LayoutBlock {
        let tree = self.layout_tree(view, LayoutConstraints::width_only(max_width));
        self.compile_tree(&tree)
    }

    /// Paints an already measured/prepared layout tree under this compiler's
    /// theme.  The tree contains no resolved palette values, so callers can
    /// retain it across theme-only repaint revisions and avoid rebuilding the
    /// layout universe.
    pub(crate) fn compile_tree(&self, tree: &LayoutTree) -> LayoutBlock {
        let mut text_geometry = TextGeometryCache::new();
        self.compile_tree_with_text_cache(tree, &mut text_geometry)
    }

    pub(crate) fn compile_tree_with_text_cache(
        &self,
        tree: &LayoutTree,
        text_geometry: &mut TextGeometryCache,
    ) -> LayoutBlock {
        let content = crate::presentation::EmptyContentProvider;
        let (rows, physically_complete) = ViewPainter.paint_tree_rows_with_content_and_text_cache(
            self,
            tree,
            &content,
            text_geometry,
        );
        LayoutBlock {
            width: tree.size.width,
            rows,
            physically_complete: tree.physically_complete && physically_complete,
        }
    }

    pub(crate) fn layout_tree(&self, view: &View, constraints: LayoutConstraints) -> LayoutTree {
        layout_view(view, constraints)
    }
}

#[cfg(test)]
pub(crate) fn compile_view(view: &View, width: u16) -> LayoutBlock {
    compile_view_with_theme(view, width, &Theme::default())
}

#[cfg(test)]
pub(crate) fn compile_view_with_overlay(
    view: &View,
    width: u16,
    overlay: &crate::scene::ResolutionOverlay,
) -> LayoutBlock {
    let compiler = ViewCompiler::default();
    let tree = layout_view_with_overlay(view, LayoutConstraints::width_only(width), overlay);
    let content = crate::presentation::EmptyContentProvider;
    let (rows, physically_complete) =
        ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &content);
    LayoutBlock {
        width: tree.size.width,
        rows,
        physically_complete: tree.physically_complete && physically_complete,
    }
}

pub(crate) fn compile_view_with_theme(view: &View, width: u16, theme: &Theme) -> LayoutBlock {
    ViewCompiler::new(theme).compile(view, width)
}

#[cfg(test)]
pub(crate) fn compile_bounded_view(view: &View, size: Size) -> LayoutBlock {
    compile_bounded_view_with_overlay(view, size, &crate::scene::ResolutionOverlay::default())
}

#[cfg(test)]
pub(crate) fn compile_bounded_view_with_overlay(
    view: &View,
    size: Size,
    overlay: &crate::scene::ResolutionOverlay,
) -> LayoutBlock {
    let compiler = ViewCompiler::default();
    let tree = layout_view_with_overlay(view, LayoutConstraints::bounded(size), overlay);
    let content = crate::presentation::EmptyContentProvider;
    let (mut rows, mut physically_complete) =
        ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &content);
    rows.truncate(usize::from(size.height));
    physically_complete &= tree.physically_complete;
    LayoutBlock {
        width: tree.size.width.min(size.width),
        rows,
        physically_complete,
    }
}

#[cfg(test)]
fn lower_surface(surface: Surface) -> Vec<PhysicalRow> {
    let width = usize::from(surface.width());
    let height = usize::from(surface.height());
    let mut cells = surface.cells.into_iter();
    (0..height)
        .map(|_| {
            let row = PhysicalRow::from_cells(cells.by_ref().take(width).collect());
            debug_assert!(row.validate_cell_geometry().is_ok());
            row
        })
        .collect()
}
