//! Physical products emitted by the occurrence/Taffy rendering route.
//!
//! This tree deliberately contains no semantic `View` values. Occurrence
//! snapshots own the declared state and Taffy owns allocation; this module
//! only retains the bounded geometry and concrete control/content products
//! needed by the terminal presenter.

use std::collections::HashMap;

use crate::presentation::api::style::{BorderSpec, ColorSpec, StyleFacts, StyleRef, StyleStates};
use crate::{
    component::{ComponentId, ControlSnapshot},
    geometry::{Rect, Size},
    occurrence::{NodeKey, OccurrenceSnapshot},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DirectNodeId(pub(crate) usize);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DirectContent {
    Children,
    ContentHost {
        port_id: u64,
        connector_id: Option<u64>,
        projection_revision: u64,
        metric_revision: u64,
        paint_revision: u64,
        projection_identity: u64,
        physically_complete: bool,
        intrinsic_size: Size,
    },
    Control(ControlSnapshot),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectNode {
    pub(crate) key: NodeKey,
    pub(crate) snapshot: Option<OccurrenceSnapshot>,
    pub(crate) rect: Rect,
    pub(crate) content_rect: Rect,
    pub(crate) content_width: u16,
    pub(crate) clip_rect: Rect,
    pub(crate) paint_origin: (i32, i32),
    pub(crate) content_origin: (i32, i32),
    pub(crate) component: Option<ComponentId>,
    pub(crate) children: Vec<DirectNodeId>,
    pub(crate) style_states: StyleStates,
    pub(crate) style_facts: StyleFacts,
    pub(crate) decoration: DirectDecoration,
    pub(crate) content: DirectContent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirectDecoration {
    pub(crate) surface_background: Option<ColorSpec>,
    pub(crate) border: Option<BorderSpec>,
    pub(crate) text_style: StyleRef,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectTree {
    pub(crate) root: DirectNodeId,
    pub(crate) nodes: Vec<DirectNode>,
    pub(crate) size: Size,
    pub(crate) physically_complete: bool,
    pub(crate) content_roots: HashMap<u64, Vec<DirectNodeId>>,
    pub(crate) parents: Vec<Option<DirectNodeId>>,
}

impl DirectTree {
    pub(crate) fn node(&self, id: DirectNodeId) -> &DirectNode {
        &self.nodes[id.0]
    }

    pub(crate) fn content_node(&self, port_id: u64) -> Option<DirectNodeId> {
        self.content_roots
            .get(&port_id)
            .and_then(|nodes| nodes.first().copied())
    }

    pub(crate) fn component_geometry(&self) -> ComponentGeometryMap {
        let mut map = ComponentGeometryMap::default();
        for node in &self.nodes {
            let Some(component) = node.component else {
                continue;
            };
            map.entries.insert(
                component,
                ComponentGeometry {
                    outer: node.rect,
                    content: node.content_rect,
                    visible: node.clip_rect.intersection(node.rect),
                },
            );
        }
        for (index, node) in self.nodes.iter().enumerate() {
            let Some(component) = node.component else {
                continue;
            };
            if let Some(extent) = self.content_extent_in_subtree(DirectNodeId(index)) {
                map.content_extents.insert(component, extent);
            }
        }
        map
    }

    /// Returns the full physical extent owned by a control's content route.
    /// A Scroll control reports its first laid-out child rather than its
    /// clipped allocation; ContentHost reports its measured content box.
    /// This is geometry feedback for the mounted control, not a second
    /// viewport/layout owner.
    fn content_extent_in_subtree(&self, id: DirectNodeId) -> Option<Size> {
        let node = self.node(id);
        if node
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.kind == crate::occurrence::HostKind::Scroll)
        {
            return node
                .children
                .first()
                .map(|child| self.node(*child).rect.size())
                .or(Some(node.content_rect.size()));
        }
        match &node.content {
            DirectContent::ContentHost { .. } => Some(node.content_rect.size()),
            _ => node
                .children
                .iter()
                .find_map(|child| self.content_extent_in_subtree(*child)),
        }
    }

    pub(crate) fn index(&mut self) {
        self.parents = vec![None; self.nodes.len()];
        self.content_roots.clear();
        let mut pending = vec![(self.root, None)];
        while let Some((id, parent)) = pending.pop() {
            self.parents[id.0] = parent;
            if let DirectContent::ContentHost { port_id, .. } = self.node(id).content {
                self.content_roots.entry(port_id).or_default().push(id);
            }
            let children = self.node(id).children.clone();
            pending.extend(children.into_iter().rev().map(|child| (child, Some(id))));
        }
    }
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
    pub(crate) content_extents: HashMap<ComponentId, Size>,
}
