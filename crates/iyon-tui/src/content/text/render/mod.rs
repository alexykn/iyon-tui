//! Generic semantic text to [`View`] lowering.
//!
//! The renderer emits reserved text [`StyleRef`] identity plus typed
//! [`TextFacts`]. Semantic paint is resolved later by Theme.

mod block;
mod identity;
mod inline;
mod policy;

#[cfg(test)]
mod source_format;
#[cfg(test)]
mod structured;
#[cfg(test)]
mod tests;

pub use policy::{
    CodeBlockLabelPolicy, SoftBreakPolicy, TableColumnSizing, TaskListMarkerPolicy,
    TextRenderPolicy,
};

pub(crate) use block::BlockLoweringCache;

use std::sync::{Arc, Mutex};

use super::{Block, BlockKind, ListMarker, TextContent, text_style_ref};
use crate::content::Renderer;
use crate::{Insets, IntoView, View};
use identity::RenderContext;

/// The one generic renderer for the frozen text IR.
#[derive(Clone, Debug)]
pub struct TextRenderer {
    policy: TextRenderPolicy,
    cache: Arc<Mutex<BlockLoweringCache>>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self {
            policy: TextRenderPolicy::default(),
            cache: Arc::new(Mutex::new(BlockLoweringCache::new())),
        }
    }
}

impl TextRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_policy(policy: TextRenderPolicy) -> Self {
        Self {
            policy,
            cache: Arc::new(Mutex::new(BlockLoweringCache::new())),
        }
    }

    #[must_use]
    pub fn policy(&self) -> &TextRenderPolicy {
        &self.policy
    }

    #[must_use]
    pub fn render_block(&self, block: &Block) -> View {
        self.lower_block(block, &RenderContext::default())
    }

    pub(crate) fn lower_semantic_iter<'a, I>(&self, input: I) -> View
    where
        I: IntoIterator<Item = &'a TextContent>,
    {
        let mut children = Vec::new();
        let mut previous: Option<&'a TextContent> = None;
        let context = RenderContext::default();
        for content in input {
            let gap = previous
                .and_then(list_of)
                .zip(list_of(content))
                .map_or_else(
                    || {
                        if previous.is_some() {
                            self.policy.block_gap()
                        } else {
                            0
                        }
                    },
                    |(left, right)| {
                        if same_list_kind(left.marker(), right.marker())
                            && left.tight()
                            && right.tight()
                        {
                            0
                        } else {
                            self.policy.block_gap()
                        }
                    },
                );
            let child = match content {
                TextContent::Raw(raw) => {
                    View::text(raw.text()).style(text_style_ref()).into_view()
                }
                TextContent::Block(block) => self.lower_block(block, &context),
            };
            children.push(child.padding(Insets::new(gap, 0, 0, 0)));
            previous = Some(content);
        }
        View::column_from_views(children, 0)
    }
}

impl Renderer<TextContent> for TextRenderer {
    fn render(&self, input: &TextContent) -> View {
        match input {
            TextContent::Raw(raw) => View::text(raw.text()).style(text_style_ref()).into_view(),
            TextContent::Block(block) => self.lower_block(block, &RenderContext::default()),
        }
    }
}

impl Renderer<Block> for TextRenderer {
    fn render(&self, input: &Block) -> View {
        self.lower_block(input, &RenderContext::default())
    }
}

impl Renderer<[TextContent]> for TextRenderer {
    fn render(&self, input: &[TextContent]) -> View {
        self.lower_semantic_iter(input.iter())
    }
}

fn list_of(content: &TextContent) -> Option<&super::List> {
    match content {
        TextContent::Block(block) => match block.kind() {
            BlockKind::List(list) => Some(list),
            _ => None,
        },
        TextContent::Raw(_) => None,
    }
}

fn same_list_kind(left: ListMarker, right: ListMarker) -> bool {
    match (left, right) {
        (ListMarker::Bullet, ListMarker::Bullet) => true,
        (
            ListMarker::Ordered {
                style: left_style,
                delimiter: left_delimiter,
                ..
            },
            ListMarker::Ordered {
                style: right_style,
                delimiter: right_delimiter,
                ..
            },
        ) => left_style == right_style && left_delimiter == right_delimiter,
        _ => false,
    }
}
