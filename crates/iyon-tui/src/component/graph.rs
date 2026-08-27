use std::collections::HashMap;

use super::{ComponentId, ComponentRevision};

/// Semantic component placements in depth-first mount order.
///
/// The graph stores topology by stable component IDs instead of indexing a
/// contiguous node vector. Local subtree replacement therefore updates only
/// the affected entries; iterating the complete graph is reserved for callers
/// that explicitly need global order (focus, mount reconciliation, or ticks).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MountGraph {
    entries: HashMap<ComponentId, MountNode>,
    roots: Vec<ComponentId>,
    children: HashMap<ComponentId, Vec<ComponentId>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MountNode {
    pub(crate) id: ComponentId,
    pub(crate) parent: Option<ComponentId>,
    pub(crate) revision: ComponentRevision,
}

pub(crate) struct MountGraphIter<'a> {
    graph: &'a MountGraph,
    pending: Vec<ComponentId>,
}

impl<'a> Iterator for MountGraphIter<'a> {
    type Item = &'a MountNode;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.pending.pop()?;
        if let Some(children) = self.graph.children.get(&id) {
            self.pending.extend(children.iter().rev().copied());
        }
        self.graph.entries.get(&id)
    }
}

impl MountGraph {
    pub(crate) fn new(nodes: Vec<MountNode>) -> Self {
        let mut graph = Self::default();
        for node in nodes {
            let id = node.id;
            assert!(
                graph.entries.insert(id, node).is_none(),
                "duplicate component id in mount graph: {id:?}"
            );
            let parent = graph.entries.get(&id).and_then(|node| node.parent);
            if let Some(parent) = parent {
                graph.children.entry(parent).or_default().push(id);
            } else {
                graph.roots.push(id);
            }
        }
        graph
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn contains(&self, id: ComponentId) -> bool {
        self.entries.contains_key(&id)
    }

    pub(crate) fn node(&self, id: ComponentId) -> Option<&MountNode> {
        self.entries.get(&id)
    }

    pub(crate) fn parent(&self, id: ComponentId) -> Option<ComponentId> {
        self.entries.get(&id).and_then(|node| node.parent)
    }

    /// Iterates mounted components in depth-first semantic order.
    pub(crate) fn iter(&self) -> MountGraphIter<'_> {
        MountGraphIter {
            graph: self,
            pending: self.roots.iter().rev().copied().collect(),
        }
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.iter().map(|node| node.id)
    }

    pub(crate) fn to_nodes(&self) -> Vec<MountNode> {
        self.iter().cloned().collect()
    }

    /// Compares only mount ownership/order, not component revisions. A
    /// revision change updates an existing component snapshot and must stay on
    /// the incremental host path; it is not a topology change.
    pub(crate) fn same_topology(&self, other: &Self) -> bool {
        self.iter()
            .map(|node| (node.id, node.parent))
            .eq(other.iter().map(|node| (node.id, node.parent)))
    }

    /// Reparents the roots of a standalone component subtree under its owner.
    pub(crate) fn reparent_roots(&mut self, parent: ComponentId) {
        for id in &self.roots {
            if let Some(node) = self.entries.get_mut(id) {
                node.parent = Some(parent);
            }
        }
    }

    /// Updates a mounted component revision without rediscovering the graph.
    pub(crate) fn update_revision(&mut self, id: ComponentId, revision: ComponentRevision) -> bool {
        let Some(node) = self.entries.get_mut(&id) else {
            return false;
        };
        node.revision = revision;
        true
    }

    /// Returns the contiguous semantic subtree owned by one mounted component.
    /// The returned IDs are depth-first and include `id` itself.
    pub(crate) fn subtree_ids(&self, id: ComponentId) -> Vec<ComponentId> {
        if !self.contains(id) {
            return Vec::new();
        }
        let mut ids = Vec::new();
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            ids.push(current);
            if let Some(children) = self.children.get(&current) {
                pending.extend(children.iter().rev().copied());
            }
        }
        ids
    }

    /// Replaces only one component's mounted descendants. The owner entry is
    /// retained, and the replacement graph contains descendants only. No
    /// global index rebuild is required: entries and child lists are keyed by
    /// stable ComponentId values.
    pub(crate) fn replace_subtree(&mut self, id: ComponentId, replacement: MountGraph) -> bool {
        if !self.contains(id) || replacement.contains(id) {
            return false;
        }
        let old_ids = self.subtree_ids(id);
        for old_id in old_ids.into_iter().skip(1) {
            self.entries.remove(&old_id);
            self.children.remove(&old_id);
        }

        let MountGraph {
            entries: replacement_entries,
            roots: replacement_roots,
            children: replacement_children,
        } = replacement;

        self.children.remove(&id);
        if !replacement_roots.is_empty() {
            self.children.insert(id, replacement_roots);
        }
        for (child_id, child) in replacement_entries {
            self.entries.insert(child_id, child);
        }
        for (parent, children) in replacement_children {
            self.children.insert(parent, children);
        }
        true
    }

    pub(crate) fn is_descendant_or_self(&self, id: ComponentId, ancestor: ComponentId) -> bool {
        let mut current = Some(id);
        while let Some(candidate) = current {
            if candidate == ancestor {
                return true;
            }
            current = self.parent(candidate);
        }
        false
    }
}
