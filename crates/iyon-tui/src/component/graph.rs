use std::{collections::HashMap, ops::Range};

use super::{ComponentId, ComponentRevision};

/// Semantic component placements in depth-first mount order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MountGraph {
    pub(crate) nodes: Vec<MountNode>,
    index: HashMap<ComponentId, usize>,
    children: HashMap<ComponentId, Vec<ComponentId>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MountNode {
    pub(crate) id: ComponentId,
    pub(crate) parent: Option<ComponentId>,
    pub(crate) revision: ComponentRevision,
}

impl MountGraph {
    pub(crate) fn new(nodes: Vec<MountNode>) -> Self {
        let mut graph = Self {
            nodes,
            index: HashMap::new(),
            children: HashMap::new(),
        };
        graph.rebuild_indexes();
        graph
    }

    pub(crate) fn contains(&self, id: ComponentId) -> bool {
        self.index.contains_key(&id)
    }

    pub(crate) fn parent(&self, id: ComponentId) -> Option<ComponentId> {
        self.index
            .get(&id)
            .and_then(|index| self.nodes.get(*index))
            .and_then(|node| node.parent)
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.nodes.iter().map(|node| node.id)
    }

    /// Updates a mounted component revision without rediscovering the graph.
    pub(crate) fn update_revision(&mut self, id: ComponentId, revision: ComponentRevision) -> bool {
        let Some(index) = self.index.get(&id).copied() else {
            return false;
        };
        self.nodes[index].revision = revision;
        true
    }

    /// Returns the contiguous depth-first span owned by one mounted component.
    pub(crate) fn subtree_range(&self, id: ComponentId) -> Option<Range<usize>> {
        let start = *self.index.get(&id)?;
        let mut end = start + 1;
        while end < self.nodes.len() && self.is_descendant_or_self(self.nodes[end].id, id) {
            end += 1;
        }
        Some(start..end)
    }

    pub(crate) fn subtree_ids(&self, id: ComponentId) -> Vec<ComponentId> {
        let Some(range) = self.subtree_range(id) else {
            return Vec::new();
        };
        self.nodes[range].iter().map(|node| node.id).collect()
    }

    /// Replaces only one component's mounted descendant span. The index is
    /// rebuilt after a topology change; ordinary revision-only updates stay
    /// O(1) through `update_revision`.
    pub(crate) fn replace_subtree(&mut self, id: ComponentId, replacement: MountGraph) -> bool {
        let Some(range) = self.subtree_range(id) else {
            return false;
        };
        self.nodes.splice(range, replacement.nodes);
        self.rebuild_indexes();
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

    fn rebuild_indexes(&mut self) {
        self.index.clear();
        self.children.clear();
        for (index, node) in self.nodes.iter().enumerate() {
            self.index.insert(node.id, index);
            if let Some(parent) = node.parent {
                self.children.entry(parent).or_default().push(node.id);
            }
        }
    }
}
