use std::{collections::HashMap, sync::Arc};

use crate::{
    Theme,
    component::ComponentId,
    geometry::Rect,
    perf::{self, Counter},
    physical::{PhysicalStyle, Surface},
    presentation::{IntoView, TextSpan, View},
};

use crate::presentation::{
    ir::{ViewId, WidthRule},
    layout::{LayoutContent, LayoutNode, LayoutNodeId, LayoutTree, ViewCompiler},
};

use super::theme::StyleContext;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StyleContextKey {
    inherited_states: Vec<(String, String)>,
    local_facts: Vec<(String, String)>,
    focused: bool,
    focus_within: bool,
}

impl From<&StyleContext> for StyleContextKey {
    fn from(context: &StyleContext) -> Self {
        Self {
            inherited_states: context
                .inherited_states
                .iter()
                .map(|(key, value)| (key.as_str().to_owned(), value.as_str().to_owned()))
                .collect(),
            local_facts: context
                .local_facts
                .iter()
                .map(|(key, value)| (key.as_str().to_owned(), value.as_str().to_owned()))
                .collect(),
            focused: context.focused,
            focus_within: context.focus_within,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PaintKey {
    view_id: ViewId,
    rect: Rect,
    content_rect: Rect,
    clip_rect: Rect,
    inherited_style: PhysicalStyle,
    resolved_style: PhysicalStyle,
    node_context: StyleContextKey,
    descendant_context: StyleContextKey,
}

/// A bounded two-generation cache for retained physical subtree surfaces.
///
/// Entries are native-owned `Arc<Surface>` values. A cache hit therefore
/// skips recursive paint, surface allocation, and child compositing; the
/// unchanged surface is composited by its new parent as usual. Theme changes
/// discard both generations because theme revisions are not semantic View
/// dependencies.
#[derive(Debug, Default)]
pub(crate) struct PaintCache {
    current: HashMap<PaintKey, Arc<Surface>>,
    previous: HashMap<PaintKey, Arc<Surface>>,
    theme: Option<Theme>,
}

impl PaintCache {
    pub(crate) fn begin_epoch(&mut self, theme: &Theme) {
        if self.theme.as_ref() != Some(theme) {
            self.current.clear();
            self.previous.clear();
            self.theme = Some(theme.clone());
            return;
        }
        self.previous = std::mem::take(&mut self.current);
    }

    fn surface(&mut self, key: &PaintKey) -> Option<Arc<Surface>> {
        if let Some(surface) = self.current.get(key) {
            perf::inc(Counter::PaintCacheHits);
            return Some(Arc::clone(surface));
        }
        if let Some(surface) = self.previous.get(key) {
            let surface = Arc::clone(surface);
            self.current.insert(key.clone(), Arc::clone(&surface));
            perf::inc(Counter::PaintCacheHits);
            return Some(surface);
        }
        perf::inc(Counter::PaintCacheMisses);
        None
    }

    fn insert(&mut self, key: PaintKey, surface: Arc<Surface>) {
        self.current.insert(key, surface);
    }

    #[cfg(test)]
    fn retained_entries(&self) -> usize {
        self.current.len() + self.previous.len()
    }
}

/// Physical lowering facade. The compiler supplies root bounds; bounded
/// callers compute retained geometry before requesting lowering.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ViewPainter;

impl ViewPainter {
    pub(crate) fn paint_tree(&self, compiler: &ViewCompiler, tree: &LayoutTree) -> Surface {
        let mut cache = PaintCache::default();
        self.paint_tree_with_style_and_cache(compiler, tree, PhysicalStyle::default(), &mut cache)
    }

    pub(crate) fn paint_tree_with_cache(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        cache: &mut PaintCache,
    ) -> Surface {
        self.paint_tree_with_style_and_cache(compiler, tree, PhysicalStyle::default(), cache)
    }

    /// Repaints one component root into an existing frame surface. The
    /// retained layout tree supplies the ancestor style context and stable
    /// coordinates, so clean sibling surfaces are neither painted nor
    /// composited again.
    pub(crate) fn paint_component_into(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        component: ComponentId,
        surface: &mut Surface,
        cache: &mut PaintCache,
    ) -> bool {
        let Some(component_root) = tree.component_roots.get(&component).copied() else {
            return false;
        };
        self.paint_subtree_into(compiler, tree, component_root, surface, cache)
    }

    /// Repaints one non-component subtree into an existing frame surface.
    ///
    /// This is used for the root-level History branch: a History revision can
    /// change its projected rows without changing the body layout or component
    /// forest. Painting this root keeps the retained surface contract while
    /// avoiding a walk of the clean sibling branch.
    pub(crate) fn paint_subtree_into(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        subtree_root: LayoutNodeId,
        surface: &mut Surface,
        cache: &mut PaintCache,
    ) -> bool {
        let path = tree.path_to_root(subtree_root);
        if path.is_empty() {
            return false;
        }
        let mut inherited = PhysicalStyle::default();
        let mut inherited_background = None;
        let mut context = compiler.style_context(tree.node(tree.root).style.component_scope);
        for ancestor in path.iter().copied().take(path.len().saturating_sub(1)) {
            let node = tree.node(ancestor);
            let node_context = context.enter_node(
                &node.style.style_states,
                &node.style.style_facts,
                compiler.style_context(node.style.component_scope),
            );
            inherited = compiler.theme.resolve_text_style(
                inherited,
                &node.style.decoration.text_style,
                &node_context,
            );
            if let Some(color) = &node.style.decoration.surface_background {
                inherited_background = Some(compiler.theme.resolve_color(color, &node_context));
            }
            context = node_context.for_descendant();
        }
        if let Some(background) = inherited_background {
            inherited.background = Some(background);
        }
        let node = tree.node(subtree_root);
        let (offset_y, clip) = tree.incremental_paint_geometry(subtree_root);
        let painted = self.paint_node(
            compiler,
            tree,
            subtree_root,
            inherited,
            context,
            cache,
            false,
        );
        let effective_y = i32::from(node.rect.y).saturating_add(offset_y);
        let effective_bottom = effective_y.saturating_add(i32::from(node.rect.height));
        let visible_top = effective_y.max(i32::from(clip.y));
        let visible_bottom = effective_bottom.min(i32::from(clip.bottom()));
        if visible_top < visible_bottom {
            surface.clear_rect_with_background(
                Rect::new(
                    node.rect.x.max(clip.x),
                    visible_top as u16,
                    node.rect
                        .width
                        .min(clip.right().saturating_sub(node.rect.x.max(clip.x))),
                    (visible_bottom - visible_top) as u16,
                ),
                inherited_background,
            );
        }
        surface.composite_clipped(&painted, i32::from(node.rect.x), effective_y, clip);
        true
    }

    pub(crate) fn paint_tree_with_style(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        inherited: PhysicalStyle,
    ) -> Surface {
        let mut cache = PaintCache::default();
        self.paint_tree_with_style_and_cache(compiler, tree, inherited, &mut cache)
    }

    fn paint_tree_with_style_and_cache(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        inherited: PhysicalStyle,
        cache: &mut PaintCache,
    ) -> Surface {
        let surface = self.paint_node(
            compiler,
            tree,
            tree.root,
            inherited,
            compiler.style_context(tree.node(tree.root).style.component_scope),
            cache,
            false,
        );
        let mut surface = Arc::try_unwrap(surface).unwrap_or_else(|surface| (*surface).clone());
        surface.physically_complete = tree.physically_complete;
        surface
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_node(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        id: LayoutNodeId,
        inherited: PhysicalStyle,
        inherited_context: crate::presentation::paint::StyleContext,
        cache: &mut PaintCache,
        use_cache: bool,
    ) -> Arc<Surface> {
        perf::inc(Counter::PaintNodesVisited);
        let node = tree.node(id);
        let node_context = inherited_context.enter_node(
            &node.style.style_states,
            &node.style.style_facts,
            compiler.style_context(node.style.component_scope),
        );
        let resolved = compiler.theme.resolve_text_style(
            inherited,
            &node.style.decoration.text_style,
            &node_context,
        );
        let descendant_context = node_context.for_descendant();
        let can_cache = use_cache && node.paint_cacheable;
        let key = PaintKey {
            view_id: node.view_id,
            rect: node.rect,
            content_rect: node.content_rect,
            clip_rect: node.clip_rect,
            inherited_style: inherited,
            resolved_style: resolved,
            node_context: StyleContextKey::from(&node_context),
            descendant_context: StyleContextKey::from(&descendant_context),
        };
        if can_cache && let Some(surface) = cache.surface(&key) {
            return surface;
        }
        perf::add(
            Counter::PaintCellsAllocated,
            u64::from(node.rect.width) * u64::from(node.rect.height),
        );
        let mut output = Surface::new(node.rect.width, node.rect.height);

        match &node.content {
            LayoutContent::Text { text, width_rule } => {
                let painted = compiler.paint_text(
                    text,
                    node.content_rect.width,
                    *width_rule,
                    resolved,
                    &descendant_context,
                );
                let x = node.content_rect.x.saturating_sub(node.rect.x);
                let y = node.content_rect.y.saturating_sub(node.rect.y);
                output.composite(&painted, x, y);
                output.physically_complete = painted.physically_complete;
            }
            LayoutContent::Spacer { rows } => {
                let height = (*rows).min(node.content_rect.height);
                perf::add(
                    Counter::PaintCellsAllocated,
                    u64::from(node.content_rect.width) * u64::from(height),
                );
                let painted = Surface::new(node.content_rect.width, height);
                let x = node.content_rect.x.saturating_sub(node.rect.x);
                let y = node.content_rect.y.saturating_sub(node.rect.y);
                output.composite(&painted, x, y);
            }
            LayoutContent::Children | LayoutContent::Clamp { .. } => {
                self.paint_children(
                    compiler,
                    tree,
                    node,
                    &mut output,
                    resolved,
                    &descendant_context,
                    cache,
                );
                if let LayoutContent::Clamp { overflow } = &node.content
                    && node
                        .children
                        .first()
                        .is_some_and(|child| tree.node(*child).rect.height > node.rect.height)
                {
                    self.paint_overflow_indicator(
                        compiler,
                        &mut output,
                        node,
                        overflow,
                        resolved,
                        &descendant_context,
                    );
                }
            }
            LayoutContent::RowViewport { skip_rows } => {
                if output.width() != 0 && output.height() != 0 {
                    let child_id = node
                        .children
                        .first()
                        .copied()
                        .expect("row viewport must have one child");
                    let painted = self.paint_node(
                        compiler,
                        tree,
                        child_id,
                        resolved,
                        descendant_context.clone(),
                        cache,
                        true,
                    );
                    for y in 0..output.height() {
                        let source_y = usize::from(*skip_rows).saturating_add(usize::from(y));
                        if source_y >= usize::from(painted.height()) {
                            continue;
                        }
                        for x in 0..output.width().min(painted.width()) {
                            *output.get_mut(x, y) = painted.get(x, source_y as u16).clone();
                            perf::inc(Counter::SurfaceCellsComposited);
                        }
                    }
                    output.physically_complete = painted.physically_complete;
                }
            }
        }

        if let Some(color) = &node.style.decoration.surface_background {
            output.apply_surface_background(compiler.theme.resolve_color(color, &node_context));
        }
        if let Some(border) = &node.style.decoration.border {
            crate::presentation::paint::paint_border(
                &mut output,
                border,
                &compiler.theme,
                resolved,
                &node_context,
            );
        }
        let output = Arc::new(output);
        if can_cache {
            cache.insert(key, Arc::clone(&output));
        }
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_children(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        node: &LayoutNode,
        output: &mut Surface,
        resolved: PhysicalStyle,
        context: &crate::presentation::paint::StyleContext,
        cache: &mut PaintCache,
    ) {
        for child in &node.children {
            let child_node = tree.node(*child);
            let painted = self.paint_node(
                compiler,
                tree,
                *child,
                resolved,
                context.clone(),
                cache,
                true,
            );
            let x = child_node.rect.x.saturating_sub(node.rect.x);
            let y = child_node.rect.y.saturating_sub(node.rect.y);
            output.composite(&painted, x, y);
        }
    }

    fn paint_overflow_indicator(
        &self,
        compiler: &ViewCompiler,
        output: &mut Surface,
        node: &LayoutNode,
        overflow: &crate::presentation::OverflowIndicator,
        inherited: PhysicalStyle,
        context: &crate::presentation::paint::StyleContext,
    ) {
        if output.height() == 0 {
            return;
        }
        let Some((text, style)) = (match overflow {
            crate::presentation::OverflowIndicator::None => None,
            crate::presentation::OverflowIndicator::Ellipsis { style } => {
                Some(("…".to_owned(), style.clone()))
            }
            crate::presentation::OverflowIndicator::Footer { prefix, style } => {
                Some((prefix.clone(), style.clone()))
            }
        }) else {
            return;
        };
        let indicator_view = View::styled_text(vec![TextSpan::styled(text, style)])
            .fill_width()
            .no_wrap()
            .into_view();
        let crate::presentation::ir::ViewKind::Text(indicator_text) = indicator_view.kind() else {
            unreachable!("overflow indicator must be text")
        };
        let indicator = compiler.paint_text(
            indicator_text,
            node.rect.width,
            WidthRule::Fill,
            inherited,
            context,
        );
        let row = output.height() - 1;
        for x in 0..output.width() {
            *output.get_mut(x, row) = crate::physical::PhysicalCell::transparent();
            if x < indicator.width() {
                *output.get_mut(x, row) = indicator.get(x, 0).clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ColorSpec, IntoView, StyleRef, StyleSelector, StyleSpec,
        component::{ComponentId, ComponentRevision, MountGraph, MountNode},
        geometry::{LayoutConstraints, Size},
        presentation::layout::layout_view,
        theme::Theme,
    };

    fn paint(
        view: &View,
        size: Size,
        theme: &Theme,
        compiler: &ViewCompiler,
        cache: &mut PaintCache,
    ) -> Surface {
        cache.begin_epoch(theme);
        let tree = layout_view(view, LayoutConstraints::bounded(size));
        ViewPainter.paint_tree_with_cache(compiler, &tree, cache)
    }

    #[test]
    fn theme_switch_invalidates_cached_surfaces() {
        let view = View::vertical(|column| {
            column.child(View::text("x"));
        })
        .foreground(ColorSpec::theme("accent"))
        .into_view();
        let red = Theme::new().with_color("accent", crate::ThemeColor::Indexed(1));
        let blue = Theme::new().with_color("accent", crate::ThemeColor::Indexed(4));
        let mut cache = PaintCache::default();
        let red_compiler = ViewCompiler::new(&red);
        let blue_compiler = ViewCompiler::new(&blue);

        let red_surface = paint(&view, Size::new(4, 1), &red, &red_compiler, &mut cache);
        let blue_surface = paint(&view, Size::new(4, 1), &blue, &blue_compiler, &mut cache);

        assert_ne!(
            red_surface.get(0, 0).style.foreground,
            blue_surface.get(0, 0).style.foreground
        );
    }

    #[test]
    fn inherited_style_change_does_not_reuse_child_surface() {
        let child = View::text("x").into_view();
        let red = View::vertical(|column| {
            column.child(child.clone());
        })
        .foreground(ColorSpec::ansi(1))
        .into_view();
        let blue = View::vertical(|column| {
            column.child(child);
        })
        .foreground(ColorSpec::ansi(4))
        .into_view();
        let theme = Theme::default();
        let compiler = ViewCompiler::new(&theme);
        let mut cache = PaintCache::default();

        let red_surface = paint(&red, Size::new(4, 1), &theme, &compiler, &mut cache);
        let blue_surface = paint(&blue, Size::new(4, 1), &theme, &compiler, &mut cache);

        assert_ne!(
            red_surface.get(0, 0).style.foreground,
            blue_surface.get(0, 0).style.foreground
        );
    }

    #[test]
    fn focus_move_invalidates_focus_dependent_surface() {
        let component = ComponentId::allocate();
        let graph = MountGraph::new(vec![MountNode {
            id: component,
            parent: None,
            revision: ComponentRevision::default(),
        }]);
        let view = View::vertical(|column| {
            column.child(View::text("x").style(StyleRef::theme("focus")));
        });
        let theme = Theme::new().with_style_variant(
            "focus",
            StyleSelector::focused(),
            StyleSpec::new().bold(),
        );
        let unfocused = ViewCompiler::with_interaction(&theme, None, &graph);
        let focused = ViewCompiler::with_interaction(&theme, Some(component), &graph);
        let mut cache = PaintCache::default();

        cache.begin_epoch(&theme);
        let mut unfocused_tree = layout_view(&view, LayoutConstraints::bounded(Size::new(4, 1)));
        unfocused_tree.nodes[1].style.component_scope = Some(component);
        let unfocused_surface =
            ViewPainter.paint_tree_with_cache(&unfocused, &unfocused_tree, &mut cache);

        cache.begin_epoch(&theme);
        let mut focused_tree = layout_view(&view, LayoutConstraints::bounded(Size::new(4, 1)));
        focused_tree.nodes[1].style.component_scope = Some(component);
        let focused_surface =
            ViewPainter.paint_tree_with_cache(&focused, &focused_tree, &mut cache);

        assert!(!unfocused_surface.get(0, 0).style.bold);
        assert!(focused_surface.get(0, 0).style.bold);
    }

    #[test]
    fn cache_retention_is_bounded_to_two_generations() {
        let theme = Theme::default();
        let compiler = ViewCompiler::new(&theme);
        let mut cache = PaintCache::default();

        for epoch in 0..3 {
            let view = View::vertical(|column| {
                for child in 0..32 {
                    column.child(View::text(format!("{epoch}-{child}")));
                }
            });
            let tree = layout_view(&view, LayoutConstraints::bounded(Size::new(16, 40)));
            cache.begin_epoch(&theme);
            ViewPainter.paint_tree_with_cache(&compiler, &tree, &mut cache);
            assert!(cache.retained_entries() <= 64);
        }

        assert_eq!(cache.current.len(), 32);
        assert_eq!(cache.previous.len(), 32);
    }

    #[test]
    fn viewport_scroll_and_geometry_are_cache_safe() {
        let content = View::vertical(|column| {
            column.children(["one", "two", "three"]);
        });
        let first = View::row_viewport(content.clone(), 0);
        let second = View::row_viewport(content, 1);
        let theme = Theme::default();
        let compiler = ViewCompiler::new(&theme);
        let mut cache = PaintCache::default();

        let first_surface = paint(&first, Size::new(8, 1), &theme, &compiler, &mut cache);
        let second_surface = paint(&second, Size::new(8, 1), &theme, &compiler, &mut cache);
        assert_eq!(first_surface.get(0, 0).grapheme.as_deref(), Some("o"));
        assert_eq!(second_surface.get(0, 0).grapheme.as_deref(), Some("t"));

        let wide = View::vertical(|column| {
            column.child(View::text("abcdef").fill_width());
        });
        let narrow_surface = paint(&wide, Size::new(2, 1), &theme, &compiler, &mut cache);
        let wide_surface = paint(&wide, Size::new(6, 1), &theme, &compiler, &mut cache);
        assert_eq!(narrow_surface.width(), 2);
        assert_eq!(wide_surface.width(), 6);
    }
}
