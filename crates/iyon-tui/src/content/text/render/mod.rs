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

use super::{Block, BlockKind, ListMarker, RawText, TextContent, text_style_ref};
#[cfg(test)]
use crate::content::Renderer;
use crate::presentation::factory as vf;
use crate::presentation::ir::{ColumnChild, PersistentSeq, ViewId};
use crate::{HorizontalAlign, Insets, StyleRef, TextSpan, View, WrapMode};
pub(crate) use identity::RenderContext;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RawCacheKey {
    page_ptr: usize,
    start: u32,
    len: u32,
}

#[derive(Clone, Debug)]
struct RawCacheEntry {
    /// Keep the page alive for as long as the address-based key is resident.
    owner: Arc<str>,
    view: View,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EdgeCacheKey {
    child: ViewId,
    gap: u16,
    predecessor: Option<(ListMarker, bool)>,
}

#[derive(Clone, Debug)]
enum SemanticItemKey {
    Raw {
        page_ptr: usize,
        start: u32,
        len: u32,
        /// Keep the source page alive while this persistent sequence entry is
        /// retained. The pointer is only an efficient lookup key; without the
        /// owner, eviction of `raw` could let the allocator reuse the address
        /// for different source bytes while a sequence hit still returns the
        /// old View.
        owner: Arc<str>,
    },
    Block {
        block_ptr: usize,
        /// A sequence entry can outlive the bounded block-lowering cache. Keep
        /// the immutable identity owner with the address key so a later Block
        /// allocation cannot alias a retained sequence hit.
        owner: Block,
    },
}

impl PartialEq for SemanticItemKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Raw {
                    page_ptr: left_page,
                    start: left_start,
                    len: left_len,
                    ..
                },
                Self::Raw {
                    page_ptr: right_page,
                    start: right_start,
                    len: right_len,
                    ..
                },
            ) => (left_page, left_start, left_len) == (right_page, right_start, right_len),
            (
                Self::Block {
                    block_ptr: left, ..
                },
                Self::Block {
                    block_ptr: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }
}

impl Eq for SemanticItemKey {}

impl std::hash::Hash for SemanticItemKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Raw {
                page_ptr,
                start,
                len,
                ..
            } => {
                0u8.hash(state);
                page_ptr.hash(state);
                start.hash(state);
                len.hash(state);
            }
            Self::Block { block_ptr, .. } => {
                1u8.hash(state);
                block_ptr.hash(state);
            }
        }
    }
}

impl crate::presentation::ir::SequenceAggregate for SemanticItemKey {
    fn sequence_flags(&self) -> u8 {
        0
    }
}

impl crate::presentation::ir::SequenceAggregate for EdgeCacheKey {
    fn sequence_flags(&self) -> u8 {
        0
    }
}

#[derive(Clone, Debug)]
struct SemanticSequenceEntry {
    items: PersistentSeq<SemanticItemKey>,
    edges: PersistentSeq<EdgeCacheKey>,
    sequence: PersistentSeq<ColumnChild>,
    view: View,
}

// Keep a small latest/full and finalized-prefix working set, plus prior entries
// for an in-flight candidate retry. The newest matching append prefix is
// preferred; older or otherwise-valid inputs may be recomputed after eviction.
// Replacement/truncation changes the first source key, while width/theme
// changes are handled by the outer prepared-product keys. Retaining more full
// persistent roots keeps historical View trees alive for limited reuse benefit.
const SEMANTIC_SEQUENCE_CACHE_CAPACITY: usize = 4;

#[derive(Clone, Debug, Default)]
struct SemanticLoweringCache {
    raw: std::collections::HashMap<RawCacheKey, RawCacheEntry>,
    edges: std::collections::HashMap<EdgeCacheKey, View>,
    sequences: Vec<SemanticSequenceEntry>,
}

/// The one generic renderer for the frozen text IR.
#[derive(Clone, Debug)]
pub(crate) struct TextRenderer {
    policy: TextRenderPolicy,
    cache: Arc<Mutex<BlockLoweringCache>>,
    /// Connector-local semantic lowering owns this cache through the
    /// TextRenderer value.  It is deliberately not global: inherited style
    /// context and renderer policy are part of the keys/owner.
    lowering_cache: Arc<Mutex<SemanticLoweringCache>>,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self {
            policy: TextRenderPolicy::default(),
            cache: Arc::new(Mutex::new(BlockLoweringCache::new())),
            lowering_cache: Arc::new(Mutex::new(SemanticLoweringCache::default())),
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
            lowering_cache: Arc::new(Mutex::new(SemanticLoweringCache::default())),
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
        let mut previous: Option<&'a TextContent> = None;
        let context = RenderContext::default();
        let mut candidate = None;
        let mut sequence = None;
        let mut rebuilding = false;
        let mut item_keys = None;
        let mut edge_keys = None;
        // A first render (or a mismatch before any retained prefix) has no
        // persistent structure to preserve. Accumulate that rebuilt suffix
        // in owned vectors and bulk-build its sequence once instead of
        // inserting every item through the root-to-leaf path.
        let mut pending_rebuild: Option<(
            Vec<ColumnChild>,
            Vec<SemanticItemKey>,
            Vec<EdgeCacheKey>,
        )> = None;
        let mut index = 0usize;
        for content in input {
            let predecessor = previous
                .and_then(list_of)
                .map(|list| (list.marker(), list.tight()));
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
            let item_key = semantic_item_key(content);
            if candidate.is_none() && index == 0 {
                candidate = self.find_sequence_candidate(&item_key);
            }
            if !rebuilding
                && let Some(candidate_index) = candidate
                && let Some(cached) =
                    self.cached_sequence_item(candidate_index, index, &item_key, gap, predecessor)
            {
                if sequence.is_none() {
                    sequence = Some(cached.0);
                }
                if item_keys.is_none() {
                    item_keys = Some(cached.1);
                }
                if edge_keys.is_none() {
                    edge_keys = Some(cached.2);
                }
                index += 1;
                previous = Some(content);
                continue;
            }
            if !rebuilding {
                let (prefix_sequence, prefix_items, prefix_edges) = candidate
                    .and_then(|candidate_index| self.sequence_prefix(candidate_index, index))
                    .unwrap_or_else(|| {
                        (
                            PersistentSeq::from_vec(Vec::new()),
                            PersistentSeq::from_vec(Vec::new()),
                            PersistentSeq::from_vec(Vec::new()),
                        )
                    });
                sequence = Some(prefix_sequence);
                item_keys = Some(prefix_items);
                edge_keys = Some(prefix_edges);
                pending_rebuild = Some((Vec::new(), Vec::new(), Vec::new()));
                rebuilding = true;
            }
            let child = match content {
                TextContent::Raw(raw) => self.lower_raw(raw),
                TextContent::Block(block) => self.lower_block(block, &context),
            };
            let child = self.lower_edge(child, gap, predecessor);
            let edge_key = EdgeCacheKey {
                child: child.id(),
                gap,
                predecessor,
            };
            if let Some((children, items, edges)) = pending_rebuild.as_mut() {
                children.push(ColumnChild::content(child));
                items.push(item_key);
                edges.push(edge_key);
            } else {
                sequence = Some(
                    sequence
                        .take()
                        .expect("semantic sequence initialized")
                        .insert(index, ColumnChild::content(child)),
                );
                item_keys = Some(
                    item_keys
                        .take()
                        .expect("semantic item sequence initialized")
                        .insert(index, item_key),
                );
                edge_keys = Some(
                    edge_keys
                        .take()
                        .expect("semantic edge sequence initialized")
                        .insert(index, edge_key),
                );
            }
            previous = Some(content);
            index += 1;
        }
        if !rebuilding
            && let Some(candidate_index) = candidate
            && let Some(view) = self.cached_sequence_view(candidate_index, index)
        {
            return view;
        }
        if !rebuilding
            && let Some(candidate_index) = candidate
            && index < self.cached_sequence_len(candidate_index)
        {
            // Every input item matched the beginning of a longer cached
            // sequence.  The cached sequence itself is not a valid result:
            // finalized-prefix updates may legitimately shorten the stream,
            // and returning its root would leak the old tail into the new
            // view.  Keep the persistent prefix roots and materialize/cache
            // exactly the requested length.
            if let Some((prefix_sequence, prefix_items, prefix_edges)) =
                self.sequence_prefix(candidate_index, index)
            {
                sequence = Some(prefix_sequence);
                item_keys = Some(prefix_items);
                edge_keys = Some(prefix_edges);
            }
        }
        if let Some((children, items, edges)) = pending_rebuild {
            let suffix = PersistentSeq::from_vec(children);
            let suffix_items = PersistentSeq::from_vec(items);
            let suffix_edges = PersistentSeq::from_vec(edges);
            sequence = Some(
                sequence
                    .take()
                    .expect("semantic sequence initialized")
                    .concat(&suffix),
            );
            item_keys = Some(
                item_keys
                    .take()
                    .expect("semantic item sequence initialized")
                    .concat(&suffix_items),
            );
            edge_keys = Some(
                edge_keys
                    .take()
                    .expect("semantic edge sequence initialized")
                    .concat(&suffix_edges),
            );
        }
        let sequence = sequence.unwrap_or_else(|| PersistentSeq::from_vec(Vec::new()));
        let item_keys = item_keys.unwrap_or_else(|| PersistentSeq::from_vec(Vec::new()));
        let edge_keys = edge_keys.unwrap_or_else(|| PersistentSeq::from_vec(Vec::new()));
        let view = vf::column_persistent(sequence.clone(), 0);
        if let Ok(mut cache) = self.lowering_cache.lock() {
            if cache.sequences.len() >= SEMANTIC_SEQUENCE_CACHE_CAPACITY {
                cache.sequences.remove(0);
            }
            cache.sequences.push(SemanticSequenceEntry {
                items: item_keys,
                edges: edge_keys,
                sequence,
                view: view.clone(),
            });
        }
        view
    }

    fn find_sequence_candidate(&self, first: &SemanticItemKey) -> Option<usize> {
        self.lowering_cache
            .lock()
            .ok()?
            .sequences
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.items.get(0) == Some(first))
            .max_by_key(|(_, entry)| entry.items.len())
            .map(|(index, _)| index)
    }

    fn cached_sequence_item(
        &self,
        candidate: usize,
        index: usize,
        item: &SemanticItemKey,
        gap: u16,
        predecessor: Option<(ListMarker, bool)>,
    ) -> Option<(
        PersistentSeq<ColumnChild>,
        PersistentSeq<SemanticItemKey>,
        PersistentSeq<EdgeCacheKey>,
    )> {
        let cache = self.lowering_cache.lock().ok()?;
        let entry = cache.sequences.get(candidate)?;
        if entry.items.get(index)? != item {
            return None;
        }
        let child = entry.sequence.get(index)?.view.clone();
        let expected = EdgeCacheKey {
            child: child.id(),
            gap,
            predecessor,
        };
        if entry.edges.get(index)? != &expected {
            return None;
        }
        Some((
            entry.sequence.clone(),
            entry.items.clone(),
            entry.edges.clone(),
        ))
    }

    fn sequence_prefix(
        &self,
        candidate: usize,
        index: usize,
    ) -> Option<(
        PersistentSeq<ColumnChild>,
        PersistentSeq<SemanticItemKey>,
        PersistentSeq<EdgeCacheKey>,
    )> {
        let cache = self.lowering_cache.lock().ok()?;
        let entry = cache.sequences.get(candidate)?;
        Some((
            entry.sequence.split(index).0,
            entry.items.split(index).0,
            entry.edges.split(index).0,
        ))
    }

    fn cached_sequence_view(&self, candidate: usize, len: usize) -> Option<View> {
        let cache = self.lowering_cache.lock().ok()?;
        let entry = cache.sequences.get(candidate)?;
        (entry.items.len() == len).then(|| entry.view.clone())
    }

    fn cached_sequence_len(&self, candidate: usize) -> usize {
        self.lowering_cache
            .lock()
            .ok()
            .and_then(|cache| {
                cache
                    .sequences
                    .get(candidate)
                    .map(|entry| entry.items.len())
            })
            .unwrap_or(0)
    }

    fn lower_raw(&self, raw: &RawText) -> View {
        let key = RawCacheKey {
            page_ptr: Arc::as_ptr(raw.page()) as *const () as usize,
            start: raw.page_start(),
            len: raw.len() as u32,
        };
        if let Ok(cache) = self.lowering_cache.lock()
            && let Some(entry) = cache.raw.get(&key)
        {
            debug_assert_eq!(
                Arc::as_ptr(&entry.owner) as *const () as usize,
                key.page_ptr
            );
            return entry.view.clone();
        }
        let view = vf::text_from_spans_with_style(
            vec![TextSpan::from_source_page(
                Arc::clone(raw.page()),
                raw.page_start(),
                raw.len() as u32,
                StyleRef::default(),
            )],
            WrapMode::WordThenGrapheme,
            HorizontalAlign::Start,
            text_style_ref(),
        );
        if let Ok(mut cache) = self.lowering_cache.lock() {
            if cache.raw.len() >= 1024 {
                cache.raw.clear();
            }
            cache.raw.insert(
                key,
                RawCacheEntry {
                    owner: Arc::clone(raw.page()),
                    view: view.clone(),
                },
            );
        }
        view
    }

    fn lower_edge(&self, child: View, gap: u16, predecessor: Option<(ListMarker, bool)>) -> View {
        if gap == 0 {
            return child;
        }
        let key = EdgeCacheKey {
            child: child.id(),
            gap,
            predecessor,
        };
        if let Ok(cache) = self.lowering_cache.lock()
            && let Some(view) = cache.edges.get(&key)
        {
            return view.clone();
        }
        let view = vf::padding(child, Insets::new(gap, 0, 0, 0));
        if let Ok(mut cache) = self.lowering_cache.lock() {
            if cache.edges.len() >= 2048 {
                cache.edges.clear();
            }
            cache.edges.insert(key, view.clone());
        }
        view
    }
}

fn semantic_item_key(content: &TextContent) -> SemanticItemKey {
    match content {
        TextContent::Raw(raw) => SemanticItemKey::Raw {
            page_ptr: Arc::as_ptr(raw.page()) as *const () as usize,
            start: raw.page_start(),
            len: raw.len() as u32,
            owner: Arc::clone(raw.page()),
        },
        TextContent::Block(block) => SemanticItemKey::Block {
            block_ptr: block.identity_ptr(),
            owner: block.clone(),
        },
    }
}

#[cfg(test)]
impl Renderer<TextContent> for TextRenderer {
    fn render(&self, input: &TextContent) -> View {
        match input {
            TextContent::Raw(raw) => self.lower_raw(raw),
            TextContent::Block(block) => self.lower_block(block, &RenderContext::default()),
        }
    }
}

#[cfg(test)]
impl Renderer<Block> for TextRenderer {
    fn render(&self, input: &Block) -> View {
        self.lower_block(input, &RenderContext::default())
    }
}

#[cfg(test)]
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
