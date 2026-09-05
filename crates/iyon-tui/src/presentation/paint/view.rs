use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{
    Theme,
    component::ComponentId,
    geometry::Rect,
    perf::{self, Counter},
    physical::{PhysicalRow, PhysicalStyle, Surface},
    presentation::{IntoView, TextSpan, View},
};

use crate::presentation::{
    ContentProvider, EmptyContentProvider,
    ir::{ViewId, ViewKind, WidthRule},
    layout::{LayoutContent, LayoutNode, LayoutNodeId, LayoutTree, ViewCompiler},
};

use super::text::TextGeometryCache;
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
    /// Text alignment/width intent are retained-state geometry inputs that do
    /// not necessarily change the immutable semantic `ViewId` or rectangle.
    text_layout: Option<(u8, u8)>,
    /// Content projection changes must invalidate the retained surface even
    /// when the `ContentHost` rectangle and style are unchanged.
    content_revision: Option<u64>,
    /// Keep the measured geometry domain explicit even when a parent happens
    /// to allocate the same rectangle for two content revisions.
    content_metric_revision: Option<u64>,
    /// Border/background glyph data can change without changing a rect or
    /// resolved text style. Keep it in the retained paint key as a compact
    /// fingerprint rather than reusing stale decoration output.
    box_fingerprint: u64,
}

fn text_layout_key(content: &LayoutContent) -> Option<(u8, u8)> {
    match content {
        LayoutContent::Text { text, width_rule } => Some((
            match text.align {
                crate::presentation::HorizontalAlign::Start => 0,
                crate::presentation::HorizontalAlign::Center => 1,
                crate::presentation::HorizontalAlign::End => 2,
            },
            match width_rule {
                WidthRule::Fit => 0,
                WidthRule::Fill => 1,
            },
        )),
        LayoutContent::Spacer { .. }
        | LayoutContent::ContentHost { .. }
        | LayoutContent::Children
        | LayoutContent::Clamp { .. }
        | LayoutContent::RowViewport { .. } => None,
    }
}

fn content_projection_revision(content: &LayoutContent) -> Option<u64> {
    match content {
        LayoutContent::ContentHost { paint_revision, .. } => Some(*paint_revision),
        _ => None,
    }
}

fn content_metric_revision(content: &LayoutContent) -> Option<u64> {
    match content {
        LayoutContent::ContentHost {
            metric_revision, ..
        } => Some(*metric_revision),
        _ => None,
    }
}

fn box_paint_key(
    compiler: &ViewCompiler,
    decoration: &crate::presentation::ir::Decoration,
    inherited: &PhysicalStyle,
    context: &StyleContext,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    decoration
        .surface_background
        .as_ref()
        .map(|color| compiler.theme.resolve_color(color, context))
        .hash(&mut hasher);
    if let Some(border) = &decoration.border {
        match border.style {
            crate::presentation::BorderStyle::Plain => 0u8,
            crate::presentation::BorderStyle::Rounded => 1,
            crate::presentation::BorderStyle::Double => 2,
        }
        .hash(&mut hasher);
        border.edges.top.hash(&mut hasher);
        border.edges.right.hash(&mut hasher);
        border.edges.bottom.hash(&mut hasher);
        border.edges.left.hash(&mut hasher);
        for glyph in [
            &border.glyphs.top,
            &border.glyphs.right,
            &border.glyphs.bottom,
            &border.glyphs.left,
            &border.glyphs.top_left,
            &border.glyphs.top_right,
            &border.glyphs.bottom_left,
            &border.glyphs.bottom_right,
        ] {
            glyph.hash(&mut hasher);
        }
        border
            .color
            .as_ref()
            .map(|color| compiler.theme.resolve_color(color, context))
            .or(inherited.foreground)
            .hash(&mut hasher);
        border.top_label.hash(&mut hasher);
    } else {
        0u8.hash(&mut hasher);
    }
    hasher.finish()
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
    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }

    pub(crate) fn invalidate_view_ids(&mut self, view_ids: &std::collections::HashSet<ViewId>) {
        self.current
            .retain(|key, _| !view_ids.contains(&key.view_id));
        self.previous
            .retain(|key, _| !view_ids.contains(&key.view_id));
    }

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
        let content = EmptyContentProvider;
        self.paint_tree_with_style_and_cache(
            compiler,
            tree,
            PhysicalStyle::default(),
            &mut cache,
            &content,
        )
    }

    /// Paints a layout tree as independent physical rows.  Layout measurement
    /// and text wrapping still establish the complete row index, but no
    /// content-sized `Surface` is allocated: each iteration owns only one
    /// width-by-one row surface and immediately lowers it to an immutable
    /// `PhysicalRow`.
    pub(crate) fn paint_tree_rows_with_content(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        content: &dyn ContentProvider,
    ) -> (Vec<PhysicalRow>, bool) {
        let mut text_rows = TextGeometryCache::new();
        self.paint_tree_rows_with_content_and_text_cache(compiler, tree, content, &mut text_rows)
    }

    pub(crate) fn paint_tree_rows_with_content_and_text_cache(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        content: &dyn ContentProvider,
        text_rows: &mut TextGeometryCache,
    ) -> (Vec<PhysicalRow>, bool) {
        let mut rows = Vec::with_capacity(usize::from(tree.size.height));
        let mut physically_complete = tree.physically_complete;
        let clip = tree.node(tree.root).clip_rect;
        let child_y_sorted = tree
            .nodes
            .iter()
            .map(|node| {
                node.children.windows(2).all(|pair| {
                    let first = tree.node(pair[0]).rect;
                    let second = tree.node(pair[1]).rect;
                    first.y <= second.y && first.bottom() <= second.bottom()
                })
            })
            .collect::<Vec<_>>();
        for row in 0..tree.size.height {
            let Some(surface) = self.paint_row_node(
                compiler,
                tree,
                tree.root,
                row,
                PhysicalStyle::default(),
                compiler.style_context(tree.node(tree.root).style.component_scope),
                clip,
                true,
                content,
                text_rows,
                &child_y_sorted,
            ) else {
                rows.push(PhysicalRow::from_cells(
                    Surface::new(tree.size.width, 1).cells,
                ));
                continue;
            };
            physically_complete &= surface.physically_complete;
            let cells = surface.cells;
            let physical = PhysicalRow::from_cells(cells);
            debug_assert!(physical.validate_cell_geometry().is_ok());
            rows.push(physical);
        }
        (rows, physically_complete)
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_row_node(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        id: LayoutNodeId,
        global_row: u16,
        inherited: PhysicalStyle,
        inherited_context: StyleContext,
        clip: Rect,
        honor_node_clip: bool,
        content: &dyn ContentProvider,
        text_rows: &mut TextGeometryCache,
        child_y_sorted: &[bool],
    ) -> Option<Surface> {
        let node = tree.node(id);
        if global_row < node.rect.y || global_row >= node.rect.bottom() {
            return None;
        }
        let effective_clip = if honor_node_clip {
            clip.intersection(node.clip_rect)
        } else {
            // A RowViewport maps the requested output row to a different
            // source row in its child.  The ancestor clip still constrains
            // the destination, but its vertical coordinates must not reject
            // that translated source row.
            Some(Rect::new(clip.x, 0, clip.width, u16::MAX))
        };
        let Some(effective_clip) = effective_clip else {
            return None;
        };
        if global_row < effective_clip.y || global_row >= effective_clip.bottom() {
            return None;
        }
        // Reject rows before allocating even the one-row destination.  The
        // caller's children are emitted in nondecreasing y order, so the
        // parent can binary-prune all disjoint siblings below.
        perf::inc(Counter::PaintNodesVisited);
        perf::add(Counter::PaintCellsAllocated, u64::from(node.rect.width));
        let mut output = Surface::new(node.rect.width, 1);

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
        let local_row = usize::from(global_row.saturating_sub(node.rect.y));
        let local_clip = local_clip_for_row(node.rect, effective_clip);

        match &node.content {
            LayoutContent::Text { text, width_rule } => {
                if global_row >= node.content_rect.y && global_row < node.content_rect.bottom() {
                    let geometry = text_rows.entry(id).or_insert_with(|| {
                        compiler.compile_text_geometry(text, node.content_rect.width, *width_rule)
                    });
                    if let Some(row) = geometry
                        .rows
                        .get(usize::from(global_row.saturating_sub(node.content_rect.y)))
                    {
                        let x = node.content_rect.x.saturating_sub(node.rect.x);
                        let (row, complete) = compiler.paint_text_geometry_row(
                            text,
                            geometry,
                            row,
                            resolved,
                            &descendant_context,
                        );
                        let row_surface = surface_from_row(row, complete);
                        output.composite_clipped(&row_surface, i32::from(x), 0, local_clip);
                        if !complete {
                            output.physically_complete = false;
                        }
                    }
                }
            }
            LayoutContent::Spacer { .. } => {}
            LayoutContent::ContentHost {
                port_id,
                projection_revision,
                ..
            } => {
                if global_row >= node.content_rect.y && global_row < node.content_rect.bottom() {
                    let x = node.content_rect.x.saturating_sub(node.rect.x);
                    let ticket = crate::presentation::PreparedProjectionTicket {
                        port_id: *port_id,
                        connector_id: node_content_connector_id(&node.content),
                        offered_width: node.content_rect.width,
                        projection_revision: *projection_revision,
                        projection_identity: node_content_projection_identity(&node.content),
                    };
                    let window = crate::presentation::ContentWindow {
                        first_row: u64::from(global_row.saturating_sub(node.content_rect.y)),
                        row_count: 1,
                    };
                    let content_clip =
                        local_clip.intersection(Rect::new(x, 0, node.content_rect.width, 1));
                    if let Some(content_clip) = content_clip {
                        content.paint_window(
                            ticket,
                            window,
                            &mut output,
                            (x, 0),
                            content_clip,
                            resolved,
                        );
                    }
                }
            }
            LayoutContent::Children | LayoutContent::Clamp { .. } => {
                for_each_child_for_row(tree, node, global_row, child_y_sorted[id.0], |child_id| {
                    let child = tree.node(child_id);
                    let Some(child_surface) = self.paint_row_node(
                        compiler,
                        tree,
                        child_id,
                        global_row,
                        resolved,
                        descendant_context.clone(),
                        effective_clip,
                        true,
                        content,
                        text_rows,
                        child_y_sorted,
                    ) else {
                        return;
                    };
                    let child_x = i32::from(child.rect.x.saturating_sub(node.rect.x));
                    output.composite_clipped(&child_surface, child_x, 0, local_clip);
                    if !child_surface.physically_complete {
                        output.physically_complete = false;
                    }
                });
                if let LayoutContent::Clamp { overflow } = &node.content
                    && global_row == node.rect.bottom().saturating_sub(1)
                    && node
                        .children
                        .first()
                        .is_some_and(|child| tree.node(*child).rect.height > node.rect.height)
                {
                    self.paint_overflow_indicator_row(
                        compiler,
                        &mut output,
                        node,
                        overflow,
                        resolved,
                        &descendant_context,
                        local_clip,
                    );
                }
            }
            LayoutContent::RowViewport { skip_rows } => {
                let Some(child_id) = node.children.first().copied() else {
                    return Some(output);
                };
                let child = tree.node(child_id);
                let viewport_row = global_row.saturating_sub(node.rect.y);
                if let LayoutContent::ContentHost {
                    port_id,
                    connector_id,
                    projection_revision,
                    projection_identity,
                    ..
                } = &child.content
                {
                    self.paint_viewport_content_host_row(
                        compiler,
                        node,
                        child,
                        viewport_row,
                        *skip_rows,
                        *port_id,
                        *connector_id,
                        *projection_revision,
                        *projection_identity,
                        &mut output,
                        resolved,
                        &descendant_context,
                        effective_clip,
                        content,
                    );
                } else {
                    let source_row = child
                        .rect
                        .y
                        .saturating_add(u16::from(*skip_rows))
                        .saturating_add(viewport_row);
                    let Some(child_surface) = self.paint_row_node(
                        compiler,
                        tree,
                        child_id,
                        source_row,
                        resolved,
                        descendant_context.clone(),
                        effective_clip,
                        false,
                        content,
                        text_rows,
                        child_y_sorted,
                    ) else {
                        return Some(output);
                    };
                    let child_x = i32::from(child.rect.x.saturating_sub(node.rect.x));
                    output.composite_clipped(&child_surface, child_x, 0, local_clip);
                    if !child_surface.physically_complete {
                        output.physically_complete = false;
                    }
                }
            }
        }

        let origin_y = -i32::try_from(local_row).unwrap_or(i32::MAX);
        if let Some(color) = &node.style.decoration.surface_background {
            apply_surface_background_at(
                &mut output,
                (0, origin_y),
                node.rect.size(),
                local_clip,
                compiler.theme.resolve_color(color, &node_context),
            );
        }
        if let Some(border) = &node.style.decoration.border {
            crate::presentation::paint::paint_border_at(
                &mut output,
                border,
                &compiler.theme,
                resolved,
                &node_context,
                (0, origin_y),
                (node.rect.width, node.rect.height),
                local_clip,
            );
        }
        Some(output)
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_viewport_content_host_row(
        &self,
        compiler: &ViewCompiler,
        viewport: &LayoutNode,
        child: &LayoutNode,
        viewport_row: u16,
        skip_rows: u16,
        port_id: u64,
        connector_id: Option<u64>,
        projection_revision: u64,
        projection_identity: u64,
        output: &mut Surface,
        inherited: PhysicalStyle,
        inherited_context: &StyleContext,
        clip: Rect,
        content: &dyn ContentProvider,
    ) {
        let child_scope = compiler.style_context(child.style.component_scope);
        let child_context = inherited_context.enter_node(
            &child.style.style_states,
            &child.style.style_facts,
            child_scope,
        );
        let child_resolved = compiler.theme.resolve_text_style(
            inherited,
            &child.style.decoration.text_style,
            &child_context,
        );
        let child_x = i32::from(child.rect.x.saturating_sub(viewport.rect.x));
        let child_y = i32::from(child.rect.y.saturating_sub(viewport.rect.y));
        let local_child_row = i32::from(viewport_row)
            .saturating_add(i32::from(skip_rows))
            .saturating_sub(child_y);
        let child_origin_x = child_x;
        let viewport_clip = Rect::new(0, 0, output.width(), output.height());
        let viewport_local_clip = local_clip_for_row(viewport.rect, clip);
        let child_clip = viewport_clip.intersection(viewport_local_clip);
        let Some(child_clip) = child_clip else {
            return;
        };

        if let Some(color) = &child.style.decoration.surface_background {
            apply_surface_background_at(
                output,
                (child_origin_x, -local_child_row),
                child.rect.size(),
                child_clip,
                compiler.theme.resolve_color(color, &child_context),
            );
        }
        if let Some(border) = &child.style.decoration.border {
            crate::presentation::paint::paint_border_at(
                output,
                border,
                &compiler.theme,
                child_resolved,
                &child_context,
                (child_origin_x, -local_child_row),
                (child.rect.width, child.rect.height),
                child_clip,
            );
        }

        let content_x = i32::from(child.content_rect.x.saturating_sub(child.rect.x));
        let content_y = i32::from(child.content_rect.y.saturating_sub(child.rect.y));
        let source_row = local_child_row.saturating_sub(content_y);
        if source_row < 0
            || source_row >= i32::from(child.content_rect.height)
            || child.content_rect.width == 0
        {
            return;
        }
        let target_x = child_origin_x.saturating_add(content_x);
        let content_clip = child_clip.intersection(Rect::new(
            u16::try_from(target_x.max(0)).unwrap_or(u16::MAX),
            0,
            child.content_rect.width,
            1,
        ));
        let Some(content_clip) = content_clip else {
            return;
        };
        let ticket = crate::presentation::PreparedProjectionTicket {
            port_id,
            connector_id,
            offered_width: child.content_rect.width,
            projection_revision,
            projection_identity,
        };
        content.paint_window(
            ticket,
            crate::presentation::ContentWindow {
                first_row: u64::try_from(source_row).unwrap_or(u64::MAX),
                row_count: 1,
            },
            output,
            (u16::try_from(target_x.max(0)).unwrap_or(u16::MAX), 0),
            content_clip,
            child_resolved,
        );
    }

    fn paint_overflow_indicator_row(
        &self,
        compiler: &ViewCompiler,
        output: &mut Surface,
        node: &LayoutNode,
        overflow: &crate::presentation::OverflowIndicator,
        inherited: PhysicalStyle,
        context: &StyleContext,
        clip: Rect,
    ) {
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
        let ViewKind::Text(indicator_text) = indicator_view.kind() else {
            unreachable!("overflow indicator must be text")
        };
        let (_, Some(row), complete) = compiler.paint_text_row(
            indicator_text,
            node.rect.width,
            WidthRule::Fill,
            inherited,
            context,
            0,
        ) else {
            return;
        };
        let painted = surface_from_row(row, complete);
        output.composite_clipped(&painted, 0, 0, clip);
        if !complete {
            output.physically_complete = false;
        }
    }

    pub(crate) fn paint_tree_with_cache(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        cache: &mut PaintCache,
    ) -> Surface {
        let content = EmptyContentProvider;
        self.paint_tree_with_style_and_cache(
            compiler,
            tree,
            PhysicalStyle::default(),
            cache,
            &content,
        )
    }

    pub(crate) fn paint_tree_with_content(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        cache: &mut PaintCache,
        content: &dyn ContentProvider,
    ) -> Surface {
        self.paint_tree_with_style_and_cache(
            compiler,
            tree,
            PhysicalStyle::default(),
            cache,
            content,
        )
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
        let content = EmptyContentProvider;
        self.paint_component_into_with_content(compiler, tree, component, surface, cache, &content)
    }

    pub(crate) fn paint_component_into_with_content(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        component: ComponentId,
        surface: &mut Surface,
        cache: &mut PaintCache,
        content: &dyn ContentProvider,
    ) -> bool {
        let Some(component_root) = tree.component_roots.get(&component).copied() else {
            return false;
        };
        self.paint_subtree_into_with_content(
            compiler,
            tree,
            component_root,
            surface,
            cache,
            content,
        )
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
        let content = EmptyContentProvider;
        self.paint_subtree_into_with_content(compiler, tree, subtree_root, surface, cache, &content)
    }

    pub(crate) fn paint_subtree_into_with_content(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        subtree_root: LayoutNodeId,
        surface: &mut Surface,
        cache: &mut PaintCache,
        content: &dyn ContentProvider,
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
            content,
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
        let content = EmptyContentProvider;
        self.paint_tree_with_style_and_cache(compiler, tree, inherited, &mut cache, &content)
    }

    fn paint_tree_with_style_and_cache(
        &self,
        compiler: &ViewCompiler,
        tree: &LayoutTree,
        inherited: PhysicalStyle,
        cache: &mut PaintCache,
        content: &dyn ContentProvider,
    ) -> Surface {
        let surface = self.paint_node(
            compiler,
            tree,
            tree.root,
            inherited,
            compiler.style_context(tree.node(tree.root).style.component_scope),
            cache,
            false,
            content,
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
        content: &dyn ContentProvider,
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
            text_layout: text_layout_key(&node.content),
            content_revision: content_projection_revision(&node.content),
            content_metric_revision: content_metric_revision(&node.content),
            box_fingerprint: box_paint_key(
                compiler,
                &node.style.decoration,
                &inherited,
                &node_context,
            ),
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
            LayoutContent::ContentHost {
                port_id,
                projection_revision,
                ..
            } => {
                let x = node.content_rect.x.saturating_sub(node.rect.x);
                let y = node.content_rect.y.saturating_sub(node.rect.y);
                let clip = Rect::new(x, y, node.content_rect.width, node.content_rect.height);
                let ticket = crate::presentation::PreparedProjectionTicket {
                    port_id: *port_id,
                    connector_id: node_content_connector_id(&node.content),
                    offered_width: node.content_rect.width,
                    projection_revision: *projection_revision,
                    projection_identity: node_content_projection_identity(&node.content),
                };
                let window = crate::presentation::ContentWindow {
                    first_row: 0,
                    row_count: u32::from(node.content_rect.height),
                };
                content.paint_window(ticket, window, &mut output, (x, y), clip, resolved);
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
                    content,
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
                    let child_node = tree.node(child_id);
                    if let LayoutContent::ContentHost {
                        port_id,
                        connector_id,
                        projection_revision,
                        projection_identity,
                        ..
                    } = child_node.content
                    {
                        self.paint_viewport_content_host(
                            compiler,
                            node,
                            child_node,
                            *skip_rows,
                            port_id,
                            connector_id,
                            projection_revision,
                            projection_identity,
                            &mut output,
                            resolved,
                            &descendant_context,
                            content,
                        );
                    } else {
                        let painted = self.paint_node(
                            compiler,
                            tree,
                            child_id,
                            resolved,
                            descendant_context.clone(),
                            cache,
                            true,
                            content,
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

    /// Paints a ContentHost child in a RowViewport without constructing the
    /// child's full offscreen Surface.  The child still contributes its own
    /// decoration, inherited style, content origin, and clip; only its rows
    /// are windowed by the viewport.
    #[allow(clippy::too_many_arguments)]
    fn paint_viewport_content_host(
        &self,
        compiler: &ViewCompiler,
        viewport: &LayoutNode,
        child: &LayoutNode,
        skip_rows: u16,
        port_id: u64,
        connector_id: Option<u64>,
        projection_revision: u64,
        projection_identity: u64,
        output: &mut Surface,
        inherited: PhysicalStyle,
        inherited_context: &StyleContext,
        content: &dyn ContentProvider,
    ) {
        let child_scope = compiler.style_context(child.style.component_scope);
        let child_context = inherited_context.enter_node(
            &child.style.style_states,
            &child.style.style_facts,
            child_scope,
        );
        let child_resolved = compiler.theme.resolve_text_style(
            inherited,
            &child.style.decoration.text_style,
            &child_context,
        );

        let child_x = i32::from(child.rect.x.saturating_sub(viewport.rect.x));
        let child_y = i32::from(child.rect.y.saturating_sub(viewport.rect.y));
        let scroll = i32::from(skip_rows);
        let child_origin = (child_x, child_y.saturating_sub(scroll));
        let viewport_clip = Rect::new(0, 0, output.width(), output.height());

        if let Some(color) = &child.style.decoration.surface_background {
            apply_surface_background_at(
                output,
                child_origin,
                child.rect.size(),
                viewport_clip,
                compiler.theme.resolve_color(color, &child_context),
            );
        }
        if let Some(border) = &child.style.decoration.border {
            crate::presentation::paint::paint_border_at(
                output,
                border,
                &compiler.theme,
                child_resolved,
                &child_context,
                child_origin,
                (child.rect.width, child.rect.height),
                viewport_clip,
            );
        }

        let content_offset_x = i32::from(child.content_rect.x.saturating_sub(child.rect.x));
        let content_offset_y = i32::from(child.content_rect.y.saturating_sub(child.rect.y));
        let content_origin_y = child_origin.1.saturating_add(content_offset_y);
        let source_offset = scroll
            .saturating_sub(child_y)
            .saturating_sub(content_offset_y);
        let first_row = u64::try_from(source_offset.max(0)).unwrap_or(u64::MAX);
        let target_origin = (
            u16::try_from(child_origin.0.saturating_add(content_offset_x).max(0))
                .unwrap_or(u16::MAX),
            u16::try_from(content_origin_y.max(0)).unwrap_or(u16::MAX),
        );

        let content_rect = translated_rect(
            child.content_rect,
            -i32::from(viewport.rect.x),
            (-i32::from(viewport.rect.y)).saturating_sub(scroll),
        );
        let child_clip = translated_rect(
            child.clip_rect,
            -i32::from(viewport.rect.x),
            (-i32::from(viewport.rect.y)).saturating_sub(scroll),
        );
        let clip = intersect_signed_rects(viewport_clip, content_rect, child_clip);
        let Some(clip) = clip else {
            return;
        };
        let ticket = crate::presentation::PreparedProjectionTicket {
            port_id,
            connector_id,
            offered_width: child.content_rect.width,
            projection_revision,
            projection_identity,
        };
        let window = crate::presentation::ContentWindow {
            first_row,
            row_count: u32::from(output.height()),
        };
        content.paint_window(ticket, window, output, target_origin, clip, child_resolved);
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
        content: &dyn ContentProvider,
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
                content,
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

type SignedRect = (i32, i32, i32, i32);

fn translated_rect(rect: Rect, dx: i32, dy: i32) -> SignedRect {
    (
        i32::from(rect.x).saturating_add(dx),
        i32::from(rect.y).saturating_add(dy),
        i32::from(rect.width),
        i32::from(rect.height),
    )
}

fn intersect_signed_rects(first: Rect, second: SignedRect, third: SignedRect) -> Option<Rect> {
    let first = translated_rect(first, 0, 0);
    let left = first.0.max(second.0).max(third.0);
    let top = first.1.max(second.1).max(third.1);
    let right = (first.0.saturating_add(first.2))
        .min(second.0.saturating_add(second.2))
        .min(third.0.saturating_add(third.2));
    let bottom = (first.1.saturating_add(first.3))
        .min(second.1.saturating_add(second.3))
        .min(third.1.saturating_add(third.3));
    if left >= right || top >= bottom {
        return None;
    }
    Some(Rect::new(
        u16::try_from(left).ok()?,
        u16::try_from(top).ok()?,
        u16::try_from(right.saturating_sub(left)).ok()?,
        u16::try_from(bottom.saturating_sub(top)).ok()?,
    ))
}

fn apply_surface_background_at(
    surface: &mut Surface,
    origin: (i32, i32),
    size: crate::geometry::Size,
    clip: Rect,
    color: crate::physical::PhysicalColor,
) {
    let Some(region) = intersect_signed_rects(
        clip,
        (
            origin.0,
            origin.1,
            i32::from(size.width),
            i32::from(size.height),
        ),
        translated_rect(clip, 0, 0),
    ) else {
        return;
    };
    for y in region.y..region.bottom() {
        for x in region.x..region.right() {
            let cell = surface.get_mut(x, y);
            if !cell.painted {
                cell.style = PhysicalStyle {
                    background: Some(color),
                    ..PhysicalStyle::default()
                };
                cell.grapheme = None;
                cell.continuation = false;
                cell.painted = true;
            } else if cell.style.background.is_none() {
                cell.style.background = Some(color);
            }
        }
    }
}

fn local_clip_for_row(node: Rect, clip: Rect) -> Rect {
    let left = clip.x.saturating_sub(node.x).min(node.width);
    let right = clip.right().saturating_sub(node.x).min(node.width);
    Rect::new(left, 0, right.saturating_sub(left), 1)
}

fn for_each_child_for_row(
    tree: &LayoutTree,
    node: &LayoutNode,
    row: u16,
    sorted: bool,
    mut visit: impl FnMut(LayoutNodeId),
) {
    if sorted {
        let first = node
            .children
            .partition_point(|child| tree.node(*child).rect.bottom() <= row);
        let remaining = &node.children[first..];
        let count = remaining.partition_point(|child| tree.node(*child).rect.y <= row);
        for child in &remaining[..count] {
            visit(*child);
        }
        return;
    }

    // Hanging prefixes and a few overlay-like structures intentionally have
    // overlapping/non-monotonic y ranges.  Preserve their original z-order
    // while filtering before recursive row allocation; only ordinary sorted
    // columns use the logarithmic range lookup above.
    for child in &node.children {
        let rect = tree.node(*child).rect;
        if rect.y <= row && row < rect.bottom() {
            visit(*child);
        }
    }
}

fn surface_from_row(row: PhysicalRow, physically_complete: bool) -> Surface {
    let width = u16::try_from(row.width()).unwrap_or(u16::MAX);
    let mut surface = Surface::new(width, 1);
    surface.physically_complete = physically_complete;
    for (column, cell) in row.cells().iter().enumerate() {
        if cell.painted {
            *surface.get_mut(column as u16, 0) = cell.clone();
        }
    }
    surface
}

fn node_content_connector_id(content: &LayoutContent) -> Option<u64> {
    match content {
        LayoutContent::ContentHost { connector_id, .. } => *connector_id,
        _ => None,
    }
}

fn node_content_projection_identity(content: &LayoutContent) -> u64 {
    match content {
        LayoutContent::ContentHost {
            projection_identity,
            ..
        } => *projection_identity,
        _ => 0,
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

    #[test]
    fn row_viewport_paints_content_host_window_directly() {
        use crate::presentation::{ContentMeasurement, ContentWindow, PreparedProjectionTicket};

        struct TestProvider {
            calls: std::sync::Arc<std::sync::Mutex<Vec<(u64, u32)>>>,
        }
        impl ContentProvider for TestProvider {
            fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
                1
            }
            fn measure(
                &mut self,
                _port_id: u64,
                _offered_width: u16,
                _width_rule: crate::presentation::WidthRule,
            ) -> ContentMeasurement {
                ContentMeasurement {
                    intrinsic_size: Size::new(10, 100),
                    physically_complete: true,
                    projection_revision: 1,
                    metric_revision: 1,
                    paint_revision: 1,
                    connector_id: Some(42),
                    projection_identity: 0,
                }
            }
            fn paint_window(
                &self,
                _ticket: PreparedProjectionTicket,
                window: ContentWindow,
                target: &mut Surface,
                target_origin: (u16, u16),
                _clip: crate::geometry::Rect,
                _style: crate::physical::PhysicalStyle,
            ) {
                self.calls
                    .lock()
                    .unwrap()
                    .push((window.first_row, window.row_count));
                for r in 0..window.row_count {
                    let y = target_origin.1 + r as u16;
                    if y < target.height() {
                        let cell = target.get_mut(target_origin.0, y);
                        cell.grapheme = Some(format!("{}", window.first_row + r as u64));
                        cell.painted = true;
                    }
                }
            }
        }

        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut provider = TestProvider {
            calls: std::sync::Arc::clone(&calls),
        };
        let content = View::content_host(42);
        let viewport = View::row_viewport(content, 10);
        let theme = Theme::default();
        let compiler = ViewCompiler::new(&theme);
        let mut cache = PaintCache::default();
        let mut layout_cache = crate::presentation::layout::LayoutCache::default();
        let tree = crate::presentation::layout::layout_view_with_overlay_and_cache_and_content(
            &viewport,
            LayoutConstraints::bounded(Size::new(10, 3)),
            &crate::scene::ResolutionOverlay::default(),
            None,
            &mut layout_cache,
            &mut provider,
        );
        let surface = ViewPainter.paint_tree_with_content(&compiler, &tree, &mut cache, &provider);

        assert_eq!(*calls.lock().unwrap(), vec![(10, 3)]);
        assert_eq!(surface.get(0, 0).grapheme.as_deref(), Some("10"));
        assert_eq!(surface.get(0, 1).grapheme.as_deref(), Some("11"));
        assert_eq!(surface.get(0, 2).grapheme.as_deref(), Some("12"));
    }

    #[test]
    fn row_viewport_content_window_preserves_child_decoration_and_style() {
        use crate::presentation::{ContentMeasurement, ContentWindow, PreparedProjectionTicket};

        struct DecoratedProvider;
        impl ContentProvider for DecoratedProvider {
            fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
                1
            }

            fn measure(
                &mut self,
                _port_id: u64,
                _offered_width: u16,
                _width_rule: crate::presentation::WidthRule,
            ) -> ContentMeasurement {
                ContentMeasurement {
                    intrinsic_size: Size::new(4, 2),
                    physically_complete: true,
                    projection_revision: 1,
                    metric_revision: 1,
                    paint_revision: 1,
                    connector_id: Some(7),
                    projection_identity: 1,
                }
            }

            fn paint_window(
                &self,
                _ticket: PreparedProjectionTicket,
                window: ContentWindow,
                target: &mut Surface,
                target_origin: (u16, u16),
                clip: Rect,
                style: PhysicalStyle,
            ) {
                for row in 0..window.row_count {
                    let x = target_origin.0;
                    let y = target_origin.1.saturating_add(row as u16);
                    if x >= target.width()
                        || y >= target.height()
                        || x < clip.x
                        || x >= clip.right()
                        || y < clip.y
                        || y >= clip.bottom()
                    {
                        continue;
                    }
                    *target.get_mut(x, y) = crate::physical::PhysicalCell {
                        grapheme: Some("X".to_owned()),
                        style,
                        painted: true,
                        continuation: false,
                    };
                }
            }
        }

        let child = View::content_host(7)
            .padding(crate::Insets::all(1))
            .background(ColorSpec::ansi(2))
            .foreground(ColorSpec::ansi(3))
            .border(crate::presentation::BorderSpec::plain().color(ColorSpec::ansi(1)));
        let viewport = View::row_viewport(child, 0);
        let theme = Theme::default();
        let compiler = ViewCompiler::new(&theme);
        let mut provider = DecoratedProvider;
        let mut layout_cache = crate::presentation::layout::LayoutCache::default();
        let tree = crate::presentation::layout::layout_view_with_overlay_and_cache_and_content(
            &viewport,
            LayoutConstraints::bounded(Size::new(8, 4)),
            &crate::scene::ResolutionOverlay::default(),
            None,
            &mut layout_cache,
            &mut provider,
        );
        let mut paint_cache = PaintCache::default();
        let surface =
            ViewPainter.paint_tree_with_content(&compiler, &tree, &mut paint_cache, &provider);

        assert_eq!(surface.get(0, 0).grapheme.as_deref(), Some("┌"));
        assert_eq!(
            surface.get(1, 1).style.background,
            Some(crate::physical::PhysicalColor::Indexed(2))
        );
        assert_eq!(surface.get(2, 2).grapheme.as_deref(), Some("X"));
        assert_eq!(
            surface.get(2, 2).style.foreground,
            Some(crate::physical::PhysicalColor::Indexed(3))
        );
        assert!(crate::physical::validate_cells(surface.row_cells(2)).is_ok());
    }

    #[test]
    fn row_viewport_rows_match_full_paint_at_a_nonzero_nested_origin() {
        use crate::presentation::{ContentMeasurement, ContentWindow, PreparedProjectionTicket};

        struct RowsProvider;
        impl ContentProvider for RowsProvider {
            fn projection_revision(&self, _port_id: u64, _offered_width: u16) -> u64 {
                1
            }

            fn measure(
                &mut self,
                _port_id: u64,
                _offered_width: u16,
                _width_rule: crate::presentation::WidthRule,
            ) -> ContentMeasurement {
                ContentMeasurement {
                    intrinsic_size: Size::new(6, 8),
                    physically_complete: true,
                    projection_revision: 1,
                    metric_revision: 1,
                    paint_revision: 1,
                    connector_id: Some(19),
                    projection_identity: 1,
                }
            }

            fn paint_window(
                &self,
                _ticket: PreparedProjectionTicket,
                window: ContentWindow,
                target: &mut Surface,
                target_origin: (u16, u16),
                clip: Rect,
                style: PhysicalStyle,
            ) {
                for row in 0..window.row_count {
                    let x = target_origin.0;
                    let y = target_origin.1.saturating_add(row as u16);
                    if x >= target.width()
                        || y >= target.height()
                        || x < clip.x
                        || x >= clip.right()
                        || y < clip.y
                        || y >= clip.bottom()
                    {
                        continue;
                    }
                    *target.get_mut(x, y) = crate::physical::PhysicalCell {
                        grapheme: Some(format!("{}", window.first_row + row as u64)),
                        style,
                        painted: true,
                        continuation: false,
                    };
                }
            }
        }

        let child = View::content_host(19)
            .padding(crate::Insets::all(1))
            .background(ColorSpec::ansi(2))
            .foreground(ColorSpec::ansi(3))
            .border(crate::presentation::BorderSpec::plain().color(ColorSpec::ansi(1)));
        let view = View::vertical(|column| {
            column.fixed(1, View::text("header"));
            column.flex(View::row_viewport(child, 2).fill_width().fill_height());
        });
        let compiler = ViewCompiler::new(&Theme::default());
        let mut provider = RowsProvider;
        let mut layout_cache = crate::presentation::layout::LayoutCache::default();
        let tree = crate::presentation::layout::layout_view_with_overlay_and_cache_and_content(
            &view,
            LayoutConstraints::bounded(Size::new(10, 5)),
            &crate::scene::ResolutionOverlay::default(),
            None,
            &mut layout_cache,
            &mut provider,
        );
        let mut paint_cache = PaintCache::default();
        let full =
            ViewPainter.paint_tree_with_content(&compiler, &tree, &mut paint_cache, &provider);
        let (rows, complete) =
            ViewPainter.paint_tree_rows_with_content(&compiler, &tree, &provider);
        assert_eq!(complete, full.physically_complete);
        assert_eq!(rows.len(), usize::from(full.height()));
        for (index, row) in rows.iter().enumerate() {
            let expected = PhysicalRow::from_cells(full.row_cells(index as u16).to_vec());
            assert_eq!(row, &expected, "nested viewport row {index} diverged");
        }
    }
}
