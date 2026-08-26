//! Immutable semantic resident-prefix ownership.

use std::collections::VecDeque;

use super::{StreamNode, StreamOffset, StreamView};

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ResidentPrefix {
    base: StreamOffset,
    end: StreamOffset,
    nodes: VecDeque<StreamNode>,
}

impl ResidentPrefix {
    pub(crate) const fn new(base: StreamOffset) -> Self {
        Self {
            base,
            end: base,
            nodes: VecDeque::new(),
        }
    }

    pub(crate) const fn base(&self) -> StreamOffset {
        self.base
    }

    pub(crate) const fn end(&self) -> StreamOffset {
        self.end
    }

    pub(crate) fn nodes(&self) -> impl Iterator<Item = &StreamNode> {
        self.nodes.iter()
    }

    /// Returns the first resident node that may overlap `offset`.
    ///
    /// Resident nodes are contiguous and ordered, so a binary search avoids
    /// walking the retained prefix merely to find the damaged suffix.
    pub(crate) fn nodes_from(&self, offset: StreamOffset) -> impl Iterator<Item = &StreamNode> {
        let mut low = 0;
        let mut high = self.nodes.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if self.nodes[middle].owned_range().end <= offset {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        self.nodes.range(low..)
    }

    pub(crate) fn view(&self) -> StreamView {
        StreamView::new(self.nodes.iter().cloned().collect())
    }

    pub(crate) fn push(&mut self, node: StreamNode) {
        let range = node.owned_range();
        assert_eq!(
            range.start, self.end,
            "resident semantic nodes must be contiguous"
        );
        if self.nodes.is_empty() {
            self.base = range.start;
        }
        self.end = range.end;
        self.nodes.push_back(node);
    }

    pub(crate) fn release_through(&mut self, offset: StreamOffset) -> StreamOffset {
        while self
            .nodes
            .front()
            .is_some_and(|node| node.owned_range().end <= offset)
        {
            self.nodes.pop_front();
        }
        self.base = self
            .nodes
            .front()
            .map_or(self.end, |node| node.owned_range().start);
        self.base
    }
}
