use std::collections::{HashMap, HashSet};

use crate::{
    component::ComponentId,
    geometry::{Rect, Size},
    presentation::api::style::{StyleFacts, StyleStates},
    presentation::ir::{Decoration, TextView, ViewId},
    presentation::{OverflowIndicator, WidthRule},
    retained_state::{OccurrenceBox, ViewStateSnapshot},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LayoutNodeId(pub(crate) usize);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutStyle {
    pub(crate) component_scope: Option<ComponentId>,
    pub(crate) style_states: StyleStates,
    pub(crate) style_facts: StyleFacts,
    pub(crate) decoration: Decoration,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LayoutContent {
    Text {
        text: TextView,
        width_rule: WidthRule,
    },
    Spacer {
        rows: u16,
    },
    Children,
    Clamp {
        overflow: OverflowIndicator,
    },
    RowViewport {
        skip_rows: u16,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutNode {
    pub(crate) view_id: ViewId,
    pub(crate) paint_cacheable: bool,
    pub(crate) occurrence: OccurrenceBox,
    pub(crate) rect: Rect,
    pub(crate) content_rect: Rect,
    pub(crate) clip_rect: Rect,
    pub(crate) component: Option<ComponentId>,
    pub(crate) children: Vec<LayoutNodeId>,
    pub(crate) style: LayoutStyle,
    pub(crate) content: LayoutContent,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutTree {
    pub(crate) root: LayoutNodeId,
    pub(crate) nodes: Vec<LayoutNode>,
    pub(crate) size: Size,
    pub(crate) physically_complete: bool,
    pub(crate) component_roots: HashMap<ComponentId, LayoutNodeId>,
    pub(crate) parents: Vec<Option<LayoutNodeId>>,
    pub(crate) state_roots: HashMap<u64, LayoutNodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComponentGeometry {
    pub(crate) outer: Rect,
    pub(crate) content: Rect,
    pub(crate) visible: Option<Rect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ComponentGeometryMap {
    pub(crate) entries: HashMap<ComponentId, ComponentGeometry>,
    pub(crate) roots: HashMap<ComponentId, LayoutNodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SignedRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl SignedRect {
    fn from(rect: Rect) -> Self {
        Self {
            x: i32::from(rect.x),
            y: i32::from(rect.y),
            width: i32::from(rect.width),
            height: i32::from(rect.height),
        }
    }

    fn translate_y(self, offset: i32) -> Self {
        Self {
            y: self.y.saturating_add(offset),
            ..self
        }
    }

    fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }

    fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (left < right && top < bottom).then_some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    fn to_rect(self) -> Rect {
        Rect::new(
            self.x.clamp(0, i32::from(u16::MAX)) as u16,
            self.y.clamp(0, i32::from(u16::MAX)) as u16,
            self.width.clamp(0, i32::from(u16::MAX)) as u16,
            self.height.clamp(0, i32::from(u16::MAX)) as u16,
        )
    }

    fn to_rect_opt(self) -> Option<Rect> {
        (self.width > 0 && self.height > 0).then(|| self.to_rect())
    }
}

fn translate_rect(rect: Rect, dx: i32, dy: i32) -> Rect {
    Rect::new(
        (i32::from(rect.x) + dx).clamp(0, i32::from(u16::MAX)) as u16,
        (i32::from(rect.y) + dy).clamp(0, i32::from(u16::MAX)) as u16,
        rect.width,
        rect.height,
    )
}

fn contains(outer: Rect, inner: Rect) -> bool {
    if inner.is_empty() {
        return inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x <= outer.right()
            && inner.y <= outer.bottom();
    }
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

impl LayoutTree {
    pub(crate) fn node(&self, id: LayoutNodeId) -> &LayoutNode {
        &self.nodes[id.0]
    }

    pub(crate) fn index_component_roots(&mut self) {
        self.component_roots.clear();
        self.state_roots.clear();
        self.parents = vec![None; self.nodes.len()];
        self.collect_component_roots(self.root, None);
    }

    fn collect_component_roots(&mut self, id: LayoutNodeId, parent: Option<LayoutNodeId>) {
        self.parents[id.0] = parent;
        let node = &self.nodes[id.0];
        if let Some(component) = node.component {
            self.component_roots.insert(component, id);
        }
        if let Some(state) = node.occurrence.state_attachment {
            self.state_roots.insert(state, id);
        }
        let children = node.children.clone();
        for child in children {
            self.collect_component_roots(child, Some(id));
        }
    }

    pub(crate) fn state_bindings(&self) -> Vec<(u64, crate::retained_state::StateNodeKind)> {
        let mut bindings = self
            .state_roots
            .iter()
            .filter_map(|(id, node)| {
                self.nodes
                    .get(node.0)
                    .map(|node| (*id, node.occurrence.node_kind))
            })
            .collect::<Vec<_>>();
        bindings.sort_unstable_by_key(|(id, _)| *id);
        bindings
    }

    pub(crate) fn apply_state_snapshot(
        &mut self,
        state_id: u64,
        snapshot: &ViewStateSnapshot,
    ) -> bool {
        let Some(node_id) = self.state_roots.get(&state_id).copied() else {
            return false;
        };
        let node = &mut self.nodes[node_id.0];
        node.occurrence.apply_state(snapshot);
        node.style.decoration = node.occurrence.effective_decoration.clone();
        node.style.style_states = node.occurrence.effective_style_states.clone();
        true
    }

    pub(crate) fn path_to_root(&self, id: LayoutNodeId) -> Vec<LayoutNodeId> {
        let mut path = Vec::new();
        let mut current = Some(id);
        while let Some(node) = current {
            path.push(node);
            current = self.parents.get(node.0).copied().flatten();
        }
        path.reverse();
        path
    }

    /// Returns the vertical translation and effective ancestor clip needed to
    /// repaint one subtree directly into the committed screen surface. A
    /// RowViewport keeps child layout coordinates in its unscrolled space;
    /// incremental paint must apply the same offset as full-tree compositing.
    pub(crate) fn incremental_paint_geometry(&self, id: LayoutNodeId) -> (i32, Rect) {
        let path = self.path_to_root(id);
        let mut offset_y = 0;
        let mut inherited_clip =
            SignedRect::from(Rect::new(0, 0, self.size.width, self.size.height));
        for ancestor in path.iter().take(path.len().saturating_sub(1)) {
            let node = self.node(*ancestor);
            inherited_clip = SignedRect::from(node.clip_rect)
                .translate_y(offset_y)
                .intersection(inherited_clip)
                .unwrap_or(SignedRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                });
            if let LayoutContent::RowViewport { skip_rows } = node.content {
                offset_y = offset_y.saturating_sub(i32::from(skip_rows));
            }
        }
        let node = self.node(id);
        let clip = SignedRect::from(node.clip_rect)
            .translate_y(offset_y)
            .intersection(inherited_clip)
            .unwrap_or(SignedRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            });
        (offset_y, clip.to_rect())
    }

    pub(crate) fn incremental_paint_rect(&self, id: LayoutNodeId) -> Option<Rect> {
        let (offset_y, clip) = self.incremental_paint_geometry(id);
        SignedRect::from(self.node(id).rect)
            .translate_y(offset_y)
            .intersection(SignedRect::from(clip))
            .and_then(SignedRect::to_rect_opt)
    }

    /// Replaces one topology-preserving physical occurrence subtree with a
    /// freshly laid-out candidate. The caller has already proved that the
    /// target's outer allocation remains fixed; parent and sibling IDs stay
    /// stable while changed descendants receive their new box geometry.
    pub(crate) fn patch_subtree(&mut self, target: LayoutNodeId, replacement: &LayoutTree) -> bool {
        let old_ids = self.preorder_ids(target);
        let new_ids = replacement.preorder_ids(replacement.root);
        if old_ids.len() != new_ids.len() {
            return false;
        }
        let old_origin = self.nodes[target.0].rect;
        let old_clip = self.nodes[target.0].clip_rect;
        for (old_id, new_id) in old_ids.iter().zip(new_ids.iter()) {
            let old_node = &self.nodes[old_id.0];
            let new_node = &replacement.nodes[new_id.0];
            if old_node.children.len() != new_node.children.len()
                || old_node.view_id != new_node.view_id
                || old_node.component != new_node.component
                || old_node.style.component_scope != new_node.style.component_scope
            {
                return false;
            }
        }
        for (old_id, new_id) in old_ids.iter().zip(new_ids.iter()) {
            let old_node = &self.nodes[old_id.0];
            let new_node = &replacement.nodes[new_id.0];
            let mut patched = new_node.clone();
            patched.rect = translate_rect(patched.rect, old_origin.x.into(), old_origin.y.into());
            patched.content_rect = translate_rect(
                patched.content_rect,
                old_origin.x.into(),
                old_origin.y.into(),
            );
            patched.clip_rect =
                translate_rect(patched.clip_rect, old_origin.x.into(), old_origin.y.into())
                    .intersection(old_clip)
                    .unwrap_or(Rect::new(old_clip.x, old_clip.y, 0, 0));
            patched.children = old_node.children.clone();
            self.nodes[old_id.0] = patched;
        }
        self.physically_complete &= replacement.physically_complete;
        self.index_component_roots();
        debug_assert!(self.validate(), "invalid patched layout tree: {self:?}");
        true
    }

    pub(crate) fn patch_component_subtree(
        &mut self,
        component: ComponentId,
        replacement: &LayoutTree,
    ) -> bool {
        let Some(component_root) = self.component_roots.get(&component).copied() else {
            return false;
        };
        let Some(old_child) = self.nodes[component_root.0].children.first().copied() else {
            return false;
        };
        let old_ids = self.preorder_ids(old_child);
        let new_ids = replacement.preorder_ids(replacement.root);
        if old_ids.len() != new_ids.len() {
            return false;
        }
        let old_origin = self.nodes[old_child.0].rect;
        let old_clip = self.nodes[old_child.0].clip_rect;
        for (old_id, new_id) in old_ids.iter().zip(new_ids.iter()) {
            let old_node = &self.nodes[old_id.0];
            let new_node = &replacement.nodes[new_id.0];
            if old_node.children.len() != new_node.children.len()
                || old_node.component != new_node.component
            {
                return false;
            }
        }
        let replacement_root = &replacement.nodes[replacement.root.0];
        self.nodes[component_root.0].view_id = replacement_root.view_id;
        self.nodes[component_root.0].paint_cacheable = replacement_root.paint_cacheable;
        for (old_id, new_id) in old_ids.iter().zip(new_ids.iter()) {
            let old_node = &self.nodes[old_id.0];
            let new_node = &replacement.nodes[new_id.0];
            let dx = i32::from(old_origin.x);
            let dy = i32::from(old_origin.y);
            let mut patched = new_node.clone();
            patched.rect = translate_rect(patched.rect, dx, dy);
            patched.content_rect = translate_rect(patched.content_rect, dx, dy);
            patched.clip_rect = translate_rect(patched.clip_rect, dx, dy)
                .intersection(old_clip)
                .unwrap_or(Rect::new(old_clip.x, old_clip.y, 0, 0));
            patched.children = old_node.children.clone();
            self.nodes[old_id.0] = patched;
        }
        self.physically_complete &= replacement.physically_complete;
        // Same-shape component patches may move a retained state attachment
        // between occurrences. Refresh both occurrence indexes before a later
        // state-only paint can target the new owner.
        self.index_component_roots();
        debug_assert!(self.validate(), "invalid patched layout tree: {self:?}");
        true
    }

    fn preorder_ids(&self, root: LayoutNodeId) -> Vec<LayoutNodeId> {
        let mut ids = Vec::new();
        self.collect_preorder(root, &mut ids);
        ids
    }

    fn collect_preorder(&self, id: LayoutNodeId, ids: &mut Vec<LayoutNodeId>) {
        ids.push(id);
        for child in &self.nodes[id.0].children {
            self.collect_preorder(*child, ids);
        }
    }

    pub(crate) fn component_geometry(&self) -> ComponentGeometryMap {
        let mut entries = HashMap::new();
        let root = SignedRect::from(Rect::new(0, 0, self.size.width, self.size.height));
        self.collect_component_geometry(self.root, 0, root, &mut entries);
        ComponentGeometryMap {
            entries,
            roots: self.component_roots.clone(),
        }
    }

    /// Refreshes component geometry for one already-indexed subtree. The caller
    /// uses this only after a topology-preserving patch, so the component-root
    /// index remains valid and clean siblings are not traversed.
    pub(crate) fn patch_component_geometry(
        &self,
        component: ComponentId,
        geometry: &mut ComponentGeometryMap,
    ) -> bool {
        let Some(component_root) = self.component_roots.get(&component).copied() else {
            return false;
        };
        let path = self.path_to_root(component_root);
        let mut offset_y = 0;
        let mut inherited_clip =
            SignedRect::from(Rect::new(0, 0, self.size.width, self.size.height));
        for ancestor in path.iter().take(path.len().saturating_sub(1)) {
            let node = self.node(*ancestor);
            let clip = SignedRect::from(node.clip_rect)
                .translate_y(offset_y)
                .intersection(inherited_clip)
                .unwrap_or(SignedRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                });
            offset_y = match node.content {
                LayoutContent::RowViewport { skip_rows } => {
                    offset_y.saturating_sub(i32::from(skip_rows))
                }
                _ => offset_y,
            };
            inherited_clip = clip;
        }
        self.collect_component_geometry(
            component_root,
            offset_y,
            inherited_clip,
            &mut geometry.entries,
        );
        true
    }

    fn collect_component_geometry(
        &self,
        id: LayoutNodeId,
        offset_y: i32,
        inherited_clip: SignedRect,
        entries: &mut HashMap<ComponentId, ComponentGeometry>,
    ) {
        crate::perf::inc(crate::perf::Counter::ComponentGeometryNodesVisited);
        let node = self.node(id);
        let rect = SignedRect::from(node.rect).translate_y(offset_y);
        let content = SignedRect::from(node.content_rect).translate_y(offset_y);
        let clip = SignedRect::from(node.clip_rect)
            .translate_y(offset_y)
            .intersection(inherited_clip)
            .unwrap_or(SignedRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            });
        if let Some(component) = node.component {
            entries.insert(
                component,
                ComponentGeometry {
                    outer: rect.to_rect(),
                    content: content.to_rect(),
                    visible: rect.intersection(clip).and_then(|rect| rect.to_rect_opt()),
                },
            );
        }

        let child_offset = match node.content {
            LayoutContent::RowViewport { skip_rows } => {
                offset_y.saturating_sub(i32::from(skip_rows))
            }
            _ => offset_y,
        };
        for child in &node.children {
            self.collect_component_geometry(*child, child_offset, clip, entries);
        }
    }

    pub(crate) fn validate(&self) -> bool {
        if self.nodes.get(self.root.0).is_none() {
            return false;
        }
        let root_rect = Rect::new(0, 0, self.size.width, self.size.height);
        let mut parents = vec![None; self.nodes.len()];
        for (parent, node) in self.nodes.iter().enumerate() {
            for child in &node.children {
                if child.0 < parents.len() {
                    parents[child.0] = Some(LayoutNodeId(parent));
                }
            }
        }
        if !self.nodes.iter().enumerate().all(|(index, node)| {
            node.children.iter().all(|child| child.0 < self.nodes.len())
                && contains(node.rect, node.content_rect)
                && (contains(root_rect, node.clip_rect)
                    || self.has_viewport_ancestor(LayoutNodeId(index), &parents))
        }) {
            return false;
        }
        let mut component_ids = HashSet::new();
        if !self
            .nodes
            .iter()
            .filter_map(|node| node.component)
            .all(|id| component_ids.insert(id))
        {
            return false;
        }
        let mut states = vec![0u8; self.nodes.len()];
        if !self.visit(self.root, &mut states) {
            return false;
        }
        states.into_iter().all(|state| state == 2)
    }

    fn has_viewport_ancestor(&self, id: LayoutNodeId, parents: &[Option<LayoutNodeId>]) -> bool {
        let mut current = parents[id.0];
        while let Some(parent) = current {
            if matches!(self.node(parent).content, LayoutContent::RowViewport { .. }) {
                return true;
            }
            current = parents[parent.0];
        }
        false
    }

    fn visit(&self, id: LayoutNodeId, states: &mut [u8]) -> bool {
        if states[id.0] == 1 {
            return false;
        }
        if states[id.0] == 2 {
            return true;
        }
        states[id.0] = 1;
        for child in &self.nodes[id.0].children {
            if !self.visit(*child, states) {
                return false;
            }
        }
        states[id.0] = 2;
        true
    }
}
