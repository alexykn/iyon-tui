//! Private retained semantic View representation.
//!
//! Construction APIs lower immediately into these owned nodes. This module
//! contains no terminal/backend state.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    component::ComponentId,
    perf::{self, Counter},
};

use super::api::{
    style::{
        BorderSpec, ColorSpec, Insets, OverflowIndicator, StyleFacts, StyleRef, StyleStates,
        VerticalAlign,
    },
    text::{HorizontalAlign, TextSpan, WrapMode},
};

/// Process-local identity for one immutable semantic node.
///
/// Identity is deliberately separate from semantic equality: it is a cache
/// key and a retention cutoff, never part of the public value semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ViewId(u64);

static NEXT_VIEW_ID: AtomicU64 = AtomicU64::new(1);

fn next_view_id() -> ViewId {
    let current = NEXT_VIEW_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("semantic ViewId exhausted");
    ViewId(current)
}

/// Cached facts about a semantic view's recursive payload.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ViewFlags(u8);

impl ViewFlags {
    const CONTAINS_COMPONENT_SLOT: u8 = 1 << 0;
    const CONTAINS_CONTENT_ATTACHMENT: u8 = 1 << 2;

    pub(crate) const fn contains_component_slot(self) -> bool {
        self.0 & Self::CONTAINS_COMPONENT_SLOT != 0
    }

    pub(crate) const fn contains_content_attachment(self) -> bool {
        self.0 & Self::CONTAINS_CONTENT_ATTACHMENT != 0
    }

    const fn with_component_slot() -> Self {
        Self(Self::CONTAINS_COMPONENT_SLOT)
    }

    const fn with_content_attachment(self) -> Self {
        Self(self.0 | Self::CONTAINS_CONTENT_ATTACHMENT)
    }
}

/// Wide immutable sequence used by retained layout payloads. Updates copy
/// only the root-to-leaf path and retain unchanged Arc-backed chunks.
#[derive(Clone, Debug)]
pub(crate) struct PersistentSeq<T: SequenceAggregate + Clone> {
    root: Arc<SeqNode<T>>,
}

pub(crate) trait SequenceAggregate {
    fn sequence_flags(&self) -> u8;
}

#[derive(Clone, Debug)]
enum SeqNode<T: SequenceAggregate + Clone> {
    Leaf {
        items: Arc<[T]>,
        flags: u8,
    },
    Branch {
        children: Arc<[Arc<SeqNode<T>>]>,
        sizes: Arc<[usize]>,
        flags: u8,
    },
}

impl<T: SequenceAggregate + Clone> PersistentSeq<T> {
    const BRANCH: usize = 32;

    pub(crate) fn from_vec(values: Vec<T>) -> Self {
        // This constructor consumes its input.  Collecting `chunks()` with
        // `to_vec()` needlessly cloned every child/handle before it entered
        // the immutable leaf, which made a large freshly lowered axis scale
        // with an avoidable second ownership pass.
        let mut values = values.into_iter();
        let mut level = Vec::new();
        loop {
            let items = values.by_ref().take(Self::BRANCH).collect::<Vec<_>>();
            if items.is_empty() {
                break;
            }
            let flags = items
                .iter()
                .fold(0, |flags, item| flags | item.sequence_flags());
            level.push(Arc::new(SeqNode::Leaf {
                items: items.into(),
                flags,
            }));
        }
        if level.is_empty() {
            level.push(Arc::new(SeqNode::Leaf {
                items: Arc::new([]),
                flags: 0,
            }));
        }
        while level.len() > Self::BRANCH {
            level = level.chunks(Self::BRANCH).map(Self::make_branch).collect();
        }
        perf::add(Counter::PersistentSeqNodesAllocated, level.len() as u64);
        let root = if level.len() == 1 {
            level.pop().expect("sequence root")
        } else {
            Self::make_branch(&level)
        };
        Self { root }
    }

    fn make_branch(children: &[Arc<SeqNode<T>>]) -> Arc<SeqNode<T>> {
        perf::inc(Counter::PersistentSeqNodesAllocated);
        perf::inc(Counter::PersistentSeqBranchClones);
        let mut total = 0;
        let mut flags = 0;
        let mut sizes = Vec::with_capacity(children.len());
        for child in children {
            total += child.len();
            sizes.push(total);
            flags |= child.flags();
        }
        Arc::new(SeqNode::Branch {
            children: children.to_vec().into(),
            sizes: sizes.into(),
            flags,
        })
    }

    fn from_roots(roots: Vec<Arc<SeqNode<T>>>) -> Self {
        if roots.is_empty() {
            return Self::from_vec(Vec::new());
        }
        Self {
            root: Self::make_branch(&roots),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.root.len()
    }
    fn height(&self) -> usize {
        self.root.height()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn aggregate_flags(&self) -> u8 {
        self.root.flags()
    }
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.root.get(index)
    }
    pub(crate) fn set(&self, index: usize, value: T) -> Self {
        if index >= self.len() {
            panic!("persistent sequence index out of range");
        }
        perf::inc(Counter::PersistentSeqLeafClones);
        perf::add(Counter::PersistentSeqBranchClones, self.height() as u64);
        perf::add(
            Counter::PersistentSeqNodesAllocated,
            (self.height() + 1) as u64,
        );
        Self {
            root: self.root.set(index, value),
        }
    }
    pub(crate) fn insert(&self, index: usize, value: T) -> Self {
        if index > self.len() {
            panic!("persistent sequence insert index out of range");
        }
        let inserted = insert_node(&self.root, index, value);
        let root = if inserted.len() == 1 {
            inserted[0].clone()
        } else {
            Self::make_branch(&inserted)
        };
        Self { root }
    }
    pub(crate) fn remove(&self, index: usize) -> Self {
        if index >= self.len() {
            panic!("persistent sequence remove index out of range");
        }
        Self {
            root: normalize_root(remove_node(&self.root, index)),
        }
    }
    pub(crate) fn split(&self, index: usize) -> (Self, Self) {
        if index > self.len() {
            panic!("persistent sequence split index out of range");
        }
        let (left, right) = split_node(&self.root, index);
        (
            Self {
                root: normalize_root(left),
            },
            Self {
                root: normalize_root(right),
            },
        )
    }
    pub(crate) fn concat(&self, other: &Self) -> Self {
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }
        let roots = concat_nodes(&self.root, &other.root);
        Self {
            root: normalize_root(if roots.len() == 1 {
                roots.into_iter().next().expect("concatenated root")
            } else {
                Self::make_branch(&roots)
            }),
        }
    }
    pub(crate) fn splice(&self, index: usize, remove_count: usize, inserted: Vec<T>) -> Self {
        if index > self.len() || remove_count > self.len() - index {
            panic!("persistent sequence splice range out of bounds");
        }
        let (left, remainder) = self.split(index);
        let (_, right) = remainder.split(remove_count);
        left.concat(&Self::from_vec(inserted)).concat(&right)
    }
    pub(crate) fn iter(&self) -> PersistentSeqIter<'_, T> {
        PersistentSeqIter {
            stack: vec![(self.root.as_ref(), 0)],
        }
    }
    pub(crate) fn root_ptr(&self) -> *const () {
        Arc::as_ptr(&self.root) as *const ()
    }
}

impl<T: SequenceAggregate + Clone> From<Vec<T>> for PersistentSeq<T> {
    fn from(values: Vec<T>) -> Self {
        Self::from_vec(values)
    }
}

impl<T: SequenceAggregate + Clone + PartialEq> PartialEq for PersistentSeq<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root, &other.root) || self.iter().eq(other.iter())
    }
}

impl<T: SequenceAggregate + Clone + Eq> Eq for PersistentSeq<T> {}

impl<T: SequenceAggregate + Clone> std::ops::Index<usize> for PersistentSeq<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .expect("persistent sequence index out of range")
    }
}

fn insert_node<T: SequenceAggregate + Clone>(
    node: &Arc<SeqNode<T>>,
    index: usize,
    value: T,
) -> Vec<Arc<SeqNode<T>>> {
    match node.as_ref() {
        SeqNode::Leaf { items, .. } => {
            let mut values = items.to_vec();
            values.insert(index, value);
            if values.len() <= PersistentSeq::<T>::BRANCH {
                return vec![Arc::new(SeqNode::Leaf {
                    flags: values
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                    items: values.into(),
                })];
            }
            let right = values.split_off(PersistentSeq::<T>::BRANCH);
            vec![
                Arc::new(SeqNode::Leaf {
                    flags: values
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                    items: values.into(),
                }),
                Arc::new(SeqNode::Leaf {
                    flags: right
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                    items: right.into(),
                }),
            ]
        }
        SeqNode::Branch {
            children, sizes, ..
        } => {
            let child = sizes
                .partition_point(|size| *size <= index)
                .min(children.len().saturating_sub(1));
            let offset = if child == 0 {
                index
            } else {
                index - sizes[child - 1]
            };
            let inserted = insert_node(&children[child], offset, value);
            let mut next = Vec::with_capacity(children.len() + inserted.len() - 1);
            next.extend(children[..child].iter().cloned());
            next.extend(inserted);
            next.extend(children[child + 1..].iter().cloned());
            if next.len() <= PersistentSeq::<T>::BRANCH {
                return vec![PersistentSeq::make_branch(&next)];
            }
            vec![
                PersistentSeq::make_branch(&next[..PersistentSeq::<T>::BRANCH]),
                PersistentSeq::make_branch(&next[PersistentSeq::<T>::BRANCH..]),
            ]
        }
    }
}

fn remove_node<T: SequenceAggregate + Clone>(
    node: &Arc<SeqNode<T>>,
    index: usize,
) -> Arc<SeqNode<T>> {
    match node.as_ref() {
        SeqNode::Leaf { items, .. } => {
            let mut values = items.to_vec();
            values.remove(index);
            Arc::new(SeqNode::Leaf {
                flags: values
                    .iter()
                    .fold(0, |flags, item| flags | item.sequence_flags()),
                items: values.into(),
            })
        }
        SeqNode::Branch {
            children, sizes, ..
        } => {
            let child = sizes.partition_point(|size| *size <= index);
            let offset = if child == 0 {
                index
            } else {
                index - sizes[child - 1]
            };
            let replacement = remove_node(&children[child], offset);
            let mut next = children.to_vec();
            next[child] = replacement;
            if next[child].len() == 0 {
                next.remove(child);
            }
            if next.is_empty() {
                Arc::new(SeqNode::Leaf {
                    items: Arc::new([]),
                    flags: 0,
                })
            } else {
                PersistentSeq::make_branch(&next)
            }
        }
    }
}

fn empty_node<T: SequenceAggregate + Clone>() -> Arc<SeqNode<T>> {
    Arc::new(SeqNode::Leaf {
        items: Arc::new([]),
        flags: 0,
    })
}

fn branch_or_empty<T: SequenceAggregate + Clone>(children: &[Arc<SeqNode<T>>]) -> Arc<SeqNode<T>> {
    if children.is_empty() {
        empty_node()
    } else {
        PersistentSeq::make_branch(children)
    }
}

fn split_node<T: SequenceAggregate + Clone>(
    node: &Arc<SeqNode<T>>,
    index: usize,
) -> (Arc<SeqNode<T>>, Arc<SeqNode<T>>) {
    if index == 0 {
        return (empty_node(), Arc::clone(node));
    }
    if index == node.len() {
        return (Arc::clone(node), empty_node());
    }
    match node.as_ref() {
        SeqNode::Leaf { items, .. } => (
            Arc::new(SeqNode::Leaf {
                flags: items[..index]
                    .iter()
                    .fold(0, |flags, item| flags | item.sequence_flags()),
                items: items[..index].to_vec().into(),
            }),
            Arc::new(SeqNode::Leaf {
                flags: items[index..]
                    .iter()
                    .fold(0, |flags, item| flags | item.sequence_flags()),
                items: items[index..].to_vec().into(),
            }),
        ),
        SeqNode::Branch {
            children, sizes, ..
        } => {
            let child = sizes.partition_point(|size| *size <= index);
            let offset = if child == 0 {
                index
            } else {
                index - sizes[child - 1]
            };
            let child_height = children[child].height();
            let (left_child, right_child) = split_node(&children[child], offset);
            let left_child = wrap_to_height(&left_child, child_height);
            let right_child = wrap_to_height(&right_child, child_height);
            let mut left_children = children[..child].to_vec();
            if left_child.len() > 0 {
                left_children.push(left_child);
            }
            let mut right_children = Vec::with_capacity(children.len() - child);
            if right_child.len() > 0 {
                right_children.push(right_child);
            }
            right_children.extend(children[child + 1..].iter().cloned());
            (
                branch_or_empty(&left_children),
                branch_or_empty(&right_children),
            )
        }
    }
}

fn wrap_to_height<T: SequenceAggregate + Clone>(
    node: &Arc<SeqNode<T>>,
    height: usize,
) -> Arc<SeqNode<T>> {
    let mut current = Arc::clone(node);
    while current.height() < height {
        current = PersistentSeq::make_branch(&[current]);
    }
    current
}

fn concat_nodes<T: SequenceAggregate + Clone>(
    left: &Arc<SeqNode<T>>,
    right: &Arc<SeqNode<T>>,
) -> Vec<Arc<SeqNode<T>>> {
    match (left.as_ref(), right.as_ref()) {
        (
            SeqNode::Leaf {
                items: left_items, ..
            },
            SeqNode::Leaf {
                items: right_items, ..
            },
        ) => {
            let mut items = Vec::with_capacity(left_items.len() + right_items.len());
            items.extend(left_items.iter().cloned());
            items.extend(right_items.iter().cloned());
            if items.len() <= PersistentSeq::<T>::BRANCH {
                vec![Arc::new(SeqNode::Leaf {
                    flags: items
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                    items: items.into(),
                })]
            } else {
                let right_items = items.split_off(PersistentSeq::<T>::BRANCH);
                vec![
                    Arc::new(SeqNode::Leaf {
                        flags: items
                            .iter()
                            .fold(0, |flags, item| flags | item.sequence_flags()),
                        items: items.into(),
                    }),
                    Arc::new(SeqNode::Leaf {
                        flags: right_items
                            .iter()
                            .fold(0, |flags, item| flags | item.sequence_flags()),
                        items: right_items.into(),
                    }),
                ]
            }
        }
        (
            SeqNode::Branch {
                children: left_children,
                ..
            },
            SeqNode::Branch {
                children: right_children,
                ..
            },
        ) if left.height() == right.height() => {
            let boundary = concat_nodes(
                left_children.last().expect("left branch child"),
                right_children.first().expect("right branch child"),
            );
            let mut children = Vec::with_capacity(left_children.len() + right_children.len());
            children.extend(left_children[..left_children.len() - 1].iter().cloned());
            children.extend(boundary);
            children.extend(right_children[1..].iter().cloned());
            if children.len() <= PersistentSeq::<T>::BRANCH {
                vec![PersistentSeq::make_branch(&children)]
            } else {
                vec![
                    PersistentSeq::make_branch(&children[..PersistentSeq::<T>::BRANCH]),
                    PersistentSeq::make_branch(&children[PersistentSeq::<T>::BRANCH..]),
                ]
            }
        }
        _ => {
            let height = left.height().max(right.height());
            let left = wrap_to_height(left, height);
            let right = wrap_to_height(right, height);
            concat_nodes(&left, &right)
        }
    }
}

fn normalize_root<T: SequenceAggregate + Clone>(mut root: Arc<SeqNode<T>>) -> Arc<SeqNode<T>> {
    loop {
        match root.as_ref() {
            SeqNode::Branch { children, .. } if children.len() == 1 => root = children[0].clone(),
            _ => return root,
        }
    }
}

impl<T: SequenceAggregate + Clone> SeqNode<T> {
    fn len(&self) -> usize {
        match self {
            Self::Leaf { items, .. } => items.len(),
            Self::Branch { sizes, .. } => sizes.last().copied().unwrap_or(0),
        }
    }
    fn height(&self) -> usize {
        match self {
            Self::Leaf { .. } => 0,
            Self::Branch { children, .. } => children.first().map_or(0, |child| child.height() + 1),
        }
    }
    fn flags(&self) -> u8 {
        match self {
            Self::Leaf { flags, .. } | Self::Branch { flags, .. } => *flags,
        }
    }
    fn get(&self, mut index: usize) -> Option<&T> {
        match self {
            Self::Leaf { items, .. } => items.get(index),
            Self::Branch {
                children, sizes, ..
            } => {
                let child = sizes.partition_point(|size| *size <= index);
                if child > 0 {
                    index -= sizes[child - 1];
                }
                children.get(child)?.get(index)
            }
        }
    }
    fn set(&self, index: usize, value: T) -> Arc<Self> {
        match self {
            Self::Leaf { items, .. } => {
                let mut next = items.to_vec();
                next[index] = value;
                Arc::new(Self::Leaf {
                    flags: next
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                    items: next.into(),
                })
            }
            Self::Branch {
                children, sizes, ..
            } => {
                let child = sizes.partition_point(|size| *size <= index);
                let offset = if child == 0 {
                    index
                } else {
                    index - sizes[child - 1]
                };
                let mut next = children.to_vec();
                next[child] = next[child].set(offset, value);
                let flags = next.iter().fold(0, |flags, item| flags | item.flags());
                Arc::new(Self::Branch {
                    children: next.into(),
                    sizes: sizes.clone(),
                    flags,
                })
            }
        }
    }
}

#[cfg(feature = "native-host")]
fn decode_native_track_word(value: u32) -> Result<TrackSize, String> {
    if value == 0 {
        return Ok(TrackSize::Content { max: None });
    }
    let kind = value & 0xff;
    let amount =
        u16::try_from(value >> 8).map_err(|_| "native axis track value exceeds u16".to_owned())?;
    match kind {
        1 => {
            if amount != 0 {
                return Err("content axis track cannot carry a value".to_owned());
            }
            Ok(TrackSize::Content { max: None })
        }
        2 => Ok(TrackSize::Content { max: Some(amount) }),
        3 => Ok(TrackSize::Fixed(amount)),
        4 => Ok(TrackSize::Flex { min: amount.max(1) }),
        5 => Ok(TrackSize::FlexMax {
            min: 1,
            max: amount,
        }),
        _ => Err("native axis track kind is invalid".to_owned()),
    }
}

pub(crate) struct PersistentSeqIter<'a, T: SequenceAggregate + Clone> {
    stack: Vec<(&'a SeqNode<T>, usize)>,
}

impl<'a, T: SequenceAggregate + Clone> Iterator for PersistentSeqIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (node, index) = self.stack.last_mut()?;
            match node {
                SeqNode::Leaf { items, .. } => {
                    if *index >= items.len() {
                        self.stack.pop();
                        continue;
                    }
                    let value = &items[*index];
                    *index += 1;
                    return Some(value);
                }
                SeqNode::Branch { children, .. } => {
                    if *index >= children.len() {
                        self.stack.pop();
                        continue;
                    }
                    let child = children[*index].as_ref();
                    *index += 1;
                    self.stack.push((child, 0));
                }
            }
        }
    }
}

/// An owned backend-neutral semantic view.
///
/// Views are persistent values. Cloning one only clones this outer `Arc`; a
/// semantic builder operation allocates a new root identity while retaining
/// every unchanged recursive payload.
#[derive(Debug, PartialEq)]
pub struct View {
    inner: Arc<ViewNode>,
}

impl std::panic::RefUnwindSafe for View {}
impl std::panic::UnwindSafe for View {}

#[derive(Debug)]
pub(crate) struct ViewNode {
    id: ViewId,
    flags: ViewFlags,
    /// Native retained content attachment. This is resolved only at the
    /// structural boundary and is never a semantic/native pointer identity.
    content_attachment: Option<u64>,
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) style_states: StyleStates,
    pub(crate) style_facts: StyleFacts,
    pub(crate) kind: ViewKind,
}

impl Clone for View {
    fn clone(&self) -> Self {
        perf::inc(Counter::ViewCloneCalls);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
impl View {
    #[cfg(test)]
    pub(crate) fn component<C>(handle: crate::component::ComponentHandle<C>) -> Self {
        crate::presentation::factory::component(handle)
    }
}

pub(crate) struct ViewNodeParts {
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) style_states: StyleStates,
    pub(crate) style_facts: StyleFacts,
    pub(crate) content_attachment: Option<u64>,
    pub(crate) kind: ViewKind,
}

/// The only semantic shape whose physical rows can be transferred to native
/// History without projecting away part of the unit.
///
/// History is an irreversible physical export.  A nested ContentHost is not
/// enough to make its containing View exportable: siblings, wrappers,
/// clipping, backgrounds, borders, bounds, and inherited style all have
/// physical effects that `ContentProvider::history_rows` does not encode.
/// Root padding is the one supported transform because the content adapter
/// explicitly emits its transparent rows and horizontal offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ContentHistoryTransfer {
    pub(crate) port_id: u64,
    pub(crate) padding: Insets,
}

fn add_history_padding(left: Insets, right: Insets) -> Option<Insets> {
    Some(Insets::new(
        left.top.checked_add(right.top)?,
        left.right.checked_add(right.right)?,
        left.bottom.checked_add(right.bottom)?,
        left.left.checked_add(right.left)?,
    ))
}

impl ViewNodeParts {
    /// Destructures a base view into candidate parts. Payloads stay shared
    /// behind their existing allocations; only the outer node identity is
    /// rebuilt by the final constructor. Attachments travel with the parts
    /// so a patch never drops them.
    pub(crate) fn from_view(view: &View) -> Self {
        let inner = &view.inner;
        Self {
            width: inner.width,
            height: inner.height,
            decoration: inner.decoration.clone(),
            style_states: inner.style_states.clone(),
            style_facts: inner.style_facts.clone(),
            content_attachment: inner.content_attachment,
            kind: inner.kind.clone(),
        }
    }
}

impl View {
    /// The one final-node construction function: canonical identity
    /// allocation with aggregate and attachment flags computed once from the
    /// finished parts. Callers assemble complete parts first; no modifier
    /// chain may follow a `from_node` call on a hot ingress path.
    pub(crate) fn from_node(parts: ViewNodeParts) -> Self {
        perf::inc(Counter::ViewNodesConstructedRust);
        Self {
            inner: Arc::new(ViewNode {
                id: next_view_id(),
                flags: ViewNode::flags_for(&parts.kind, parts.content_attachment),
                content_attachment: parts.content_attachment,
                width: parts.width,
                height: parts.height,
                decoration: parts.decoration,
                style_states: parts.style_states,
                style_facts: parts.style_facts,
                kind: parts.kind,
            }),
        }
    }

    pub(crate) fn id(&self) -> ViewId {
        self.inner.id
    }

    /// Returns a renderer-local identity for an occurrence-backed layout
    /// node.  Occurrence identity remains the canonical key; this value only
    /// lets the existing paint cache and layout metadata represent a direct
    /// Taffy tree without materializing a semantic View for every occurrence.
    pub(crate) fn direct_id(key: crate::occurrence::NodeKey) -> ViewId {
        let mut value = u64::from(key.slot) << 32 | u64::from(key.generation);
        value |= 1 << 63;
        ViewId(value)
    }

    pub(crate) fn direct_root_id(driver_id: u64) -> ViewId {
        ViewId((1 << 63) | (driver_id & u64::from(u32::MAX)))
    }

    pub(crate) fn width(&self) -> WidthRule {
        self.inner.width
    }

    pub(crate) fn height(&self) -> HeightRule {
        self.inner.height
    }

    pub(crate) fn decoration(&self) -> &Decoration {
        &self.inner.decoration
    }

    pub(crate) fn view_style_states(&self) -> &StyleStates {
        &self.inner.style_states
    }

    pub(crate) fn view_style_facts(&self) -> &StyleFacts {
        &self.inner.style_facts
    }

    pub(crate) fn content_attachment_id(&self) -> Option<u64> {
        self.inner.content_attachment
    }

    pub(crate) fn content_host(port_id: u64) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            content_attachment: Some(port_id),
            kind: ViewKind::ContentHost,
        })
    }

    pub(crate) fn kind(&self) -> &ViewKind {
        &self.inner.kind
    }

    pub(crate) fn flags(&self) -> ViewFlags {
        self.inner.flags
    }

    pub(crate) fn ptr_eq(left: &Self, right: &Self) -> bool {
        Arc::ptr_eq(&left.inner, &right.inner)
    }

    /// Creates a new semantic root while retaining all unchanged payloads.
    pub(crate) fn map_node(self, update: impl FnOnce(&mut ViewNode)) -> Self {
        let mut next = self.inner.shallow_clone();
        update(&mut next);
        next.flags = ViewNode::flags_for(&next.kind, next.content_attachment);
        next.id = next_view_id();
        Self {
            inner: Arc::new(next),
        }
    }

    /// Applies a text-local semantic update without copying its span storage.
    pub(crate) fn map_text(self, update: impl FnOnce(&mut TextView)) -> Self {
        self.map_node(|node| {
            let ViewKind::Text(text) = &mut node.kind else {
                unreachable!("text wrapper must always contain ViewKind::Text")
            };
            update(Arc::make_mut(text));
        })
    }

    pub(crate) fn contains_component_identity(&self) -> bool {
        self.flags().contains_component_slot()
    }

    pub(crate) fn contains_content_identity(&self) -> bool {
        self.flags().contains_content_attachment()
    }

    /// Returns the complete physical transfer contract for this View, if it
    /// is represented by exactly one ContentHost product and only transforms
    /// implemented by the native History adapter.
    ///
    /// Recursive attachment traversal remains available through
    /// `content_attachment_ids` for metric/cache dependencies.  This method
    /// intentionally does not use that traversal for export eligibility.
    pub(crate) fn content_history_transfer(&self) -> Option<ContentHistoryTransfer> {
        let mut padding = Insets::ZERO;
        let port_id = self.collect_content_history_transfer(&mut padding, true)?;
        Some(ContentHistoryTransfer { port_id, padding })
    }

    fn collect_content_history_transfer(
        &self,
        padding: &mut Insets,
        allow_content_padding: bool,
    ) -> Option<u64> {
        let decoration = self.decoration();
        if (!allow_content_padding && decoration.padding != Insets::ZERO)
            || *self.view_style_states() != StyleStates::default()
            || *self.view_style_facts() != StyleFacts::default()
            || self.height() != HeightRule::Fit
            || decoration.bounds != ViewBounds::default()
            || decoration.surface_background.is_some()
            || decoration.border.is_some()
            || decoration.text_style != StyleRef::default()
        {
            return None;
        }
        *padding = add_history_padding(*padding, decoration.padding)?;
        match self.kind() {
            ViewKind::ContentHost
                if self.width() == WidthRule::Fit || self.width() == WidthRule::Fill =>
            {
                self.content_attachment_id()
            }
            // The M1 History adapter can preserve these one-child shells
            // exactly. Any sibling, gap, alignment, viewport, or clamp would
            // add physical geometry that a content-row product cannot carry.
            ViewKind::Container(container) if self.width() == WidthRule::Fit => container
                .child
                .collect_content_history_transfer(padding, false),
            ViewKind::Column(column)
                if self.width() == WidthRule::Fit
                    && column.gap == 0
                    && column.children.len() == 1 =>
            {
                column
                    .children
                    .get(0)
                    .expect("one-child Column has a child")
                    .view
                    .collect_content_history_transfer(padding, false)
            }
            ViewKind::Row(row)
                if self.width() == WidthRule::Fit
                    && row.gap == 0
                    && row.vertical_align == VerticalAlign::Top
                    && row.children.len() == 1 =>
            {
                row.children
                    .get(0)
                    .expect("one-child Row has a child")
                    .view
                    .collect_content_history_transfer(padding, false)
            }
            _ => None,
        }
    }

    pub(crate) fn content_attachment_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.collect_content_attachment_ids(&mut ids);
        ids
    }

    fn collect_content_attachment_ids(&self, ids: &mut Vec<u64>) {
        if let Some(id) = self.content_attachment_id() {
            ids.push(id);
        }
        match &self.inner.kind {
            ViewKind::Container(container) => container.child.collect_content_attachment_ids(ids),
            ViewKind::Hanging(hanging) => {
                hanging.prefix.collect_content_attachment_ids(ids);
                hanging
                    .continuation_prefix
                    .collect_content_attachment_ids(ids);
                hanging.body.collect_content_attachment_ids(ids);
            }
            ViewKind::ClampRows(clamp) => clamp.child.collect_content_attachment_ids(ids),
            ViewKind::RowViewport(viewport) => viewport.child.collect_content_attachment_ids(ids),
            ViewKind::Column(column) => {
                for child in column.children.iter() {
                    child.view.collect_content_attachment_ids(ids);
                }
            }
            ViewKind::Row(row) => {
                for child in row.children.iter() {
                    child.view.collect_content_attachment_ids(ids);
                }
            }
            ViewKind::Grid(grid) => {
                for cell in grid.cells.iter() {
                    cell.view.collect_content_attachment_ids(ids);
                }
            }
            ViewKind::Text(_)
            | ViewKind::Spacer { .. }
            | ViewKind::ComponentSlot(_)
            | ViewKind::ContentHost => {}
        }
    }

    pub(crate) fn single_content_attachment_id(&self) -> Option<u64> {
        let ids = self.content_attachment_ids();
        if ids.len() == 1 {
            return ids.into_iter().next();
        }
        None
    }
}

impl PartialEq for ViewNode {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_eq(other)
    }
}

impl ViewNode {
    /// Aggregate child flags plus attachment presence, computed once per
    /// final root. Shared by the final constructor and the retained
    /// single-field patch path so both agree on every flag bit.
    fn flags_for(kind: &ViewKind, content_attachment: Option<u64>) -> ViewFlags {
        let mut flags = Self::compute_flags(kind);
        if content_attachment.is_some() {
            flags = flags.with_content_attachment();
        }
        flags
    }

    fn compute_flags(kind: &ViewKind) -> ViewFlags {
        match kind {
            ViewKind::ComponentSlot(_) => ViewFlags::with_component_slot(),
            ViewKind::ContentHost => ViewFlags::default(),
            ViewKind::Text(_) | ViewKind::Spacer { .. } => ViewFlags::default(),
            ViewKind::Container(container) => container.child.flags(),
            ViewKind::Hanging(hanging) => ViewFlags(
                hanging.prefix.flags().0
                    | hanging.continuation_prefix.flags().0
                    | hanging.body.flags().0,
            ),
            ViewKind::ClampRows(clamp) => clamp.child.flags(),
            ViewKind::RowViewport(viewport) => viewport.child.flags(),
            ViewKind::Column(column) => ViewFlags(column.children.aggregate_flags()),
            ViewKind::Row(row) => ViewFlags(row.children.aggregate_flags()),
            ViewKind::Grid(grid) => ViewFlags(grid.cells.aggregate_flags()),
        }
    }

    fn shallow_clone(&self) -> Self {
        Self {
            id: self.id,
            flags: self.flags,
            content_attachment: self.content_attachment,
            width: self.width,
            height: self.height,
            decoration: self.decoration.clone(),
            style_states: self.style_states.clone(),
            style_facts: self.style_facts.clone(),
            kind: self.kind.clone(),
        }
    }

    fn semantic_eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.decoration == other.decoration
            && self.style_states == other.style_states
            && self.style_facts == other.style_facts
            && self.content_attachment == other.content_attachment
            && self.kind == other.kind
    }
}

/// RETAINED SEMANTIC IR. Generic view node kinds understood by the compiler.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ViewKind {
    Text(Arc<TextView>),
    Column(Arc<ColumnView>),
    Row(Arc<RowView>),
    Grid(Arc<GridView>),
    Hanging(Arc<HangingView>),
    Container(Arc<ContainerNode>),
    Spacer {
        rows: u16,
    },
    ClampRows(Arc<ClampRowsView>),
    RowViewport(Arc<RowViewportView>),
    ComponentSlot(ComponentSlotNode),
    /// A structural receiving region for a retained `ContentPort`. Derived
    /// content rows are supplied by the host frame's content provider.
    ContentHost,
}

/// RETAINED SEMANTIC IR. Deferred placement of a retained component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComponentSlotNode {
    pub(crate) id: ComponentId,
}

/// RETAINED SEMANTIC IR. Width allocation requested from a parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WidthRule {
    #[default]
    Fit,
    Fill,
}

/// RETAINED SEMANTIC IR. Height allocation requested from a parent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum HeightRule {
    #[default]
    Fit,
    Fill,
}
/// RETAINED SEMANTIC IR. Styled text, represented without terminal types.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextView {
    pub(crate) spans: Arc<[TextSpan]>,
    pub(crate) wrap: WrapMode,
    pub(crate) align: HorizontalAlign,
    pub(crate) cursor: Option<TextCursorAnchor>,
}

/// Private semantic caret metadata. It describes a UTF-8 source boundary;
/// layout resolves it to a physical cell without exposing geometry upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextCursorAnchor {
    pub(crate) byte_offset: usize,
}

impl TextView {
    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            spans: vec![TextSpan::plain(text)].into(),
            wrap: WrapMode::WordThenGrapheme,
            align: HorizontalAlign::Start,
            cursor: None,
        }
    }
}
/// RETAINED SEMANTIC IR. Vertical composition. The parent owns sibling gaps.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColumnView {
    pub(crate) children: PersistentSeq<ColumnChild>,
    pub(crate) gap: u16,
}

/// RETAINED SEMANTIC IR. One column child and its height track.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColumnChild {
    pub(crate) track: TrackSize,
    pub(crate) view: View,
}

impl SequenceAggregate for ColumnChild {
    fn sequence_flags(&self) -> u8 {
        self.view.flags().0
    }
}

impl ColumnChild {
    pub(crate) fn content(view: View) -> Self {
        Self {
            track: TrackSize::Content { max: None },
            view,
        }
    }

    pub(crate) fn fixed(height: u16, view: View) -> Self {
        Self {
            track: TrackSize::Fixed(height),
            view,
        }
    }

    pub(crate) fn flex(view: View) -> Self {
        Self {
            track: TrackSize::Flex { min: 1 },
            view,
        }
    }
}

/// RETAINED SEMANTIC IR. Horizontal composition. The parent owns sibling gaps.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowView {
    pub(crate) children: PersistentSeq<RowChild>,
    pub(crate) gap: u16,
    pub(crate) vertical_align: VerticalAlign,
}

/// Semantic first-line prefix plus repeated continuation prefix.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HangingView {
    pub(crate) prefix: View,
    pub(crate) continuation_prefix: View,
    pub(crate) body: View,
}

/// RETAINED SEMANTIC IR. One row child and its width track.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowChild {
    pub(crate) track: TrackSize,
    pub(crate) view: View,
}

impl SequenceAggregate for RowChild {
    fn sequence_flags(&self) -> u8 {
        self.view.flags().0
    }
}

impl RowChild {
    pub(crate) fn content(view: View) -> Self {
        Self {
            track: TrackSize::Content { max: None },
            view,
        }
    }

    pub(crate) fn fixed(width: u16, view: View) -> Self {
        Self {
            track: TrackSize::Fixed(width),
            view,
        }
    }

    pub(crate) fn flex(view: View) -> Self {
        Self {
            track: TrackSize::Flex { min: 1 },
            view,
        }
    }
}

/// RETAINED SEMANTIC IR. Shared two-dimensional track layout.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GridView {
    pub(crate) columns: PersistentSeq<TrackSize>,
    pub(crate) rows: PersistentSeq<TrackSize>,
    pub(crate) column_gap: u16,
    pub(crate) row_gap: u16,
    pub(crate) cells: PersistentSeq<GridCellView>,
    pub(crate) cell_indices: Arc<HashMap<(usize, usize), usize>>,
}

/// RETAINED SEMANTIC IR. One grid cell and its explicit track placement.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GridCellView {
    pub(crate) row: usize,
    pub(crate) column: usize,
    pub(crate) row_span: u16,
    pub(crate) column_span: u16,
    pub(crate) horizontal_align: HorizontalAlign,
    pub(crate) vertical_align: VerticalAlign,
    pub(crate) view: View,
}

impl SequenceAggregate for GridCellView {
    fn sequence_flags(&self) -> u8 {
        self.view.flags().0
    }
}

/// RETAINED SEMANTIC IR. Width allocation for a row child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrackSize {
    Content { max: Option<u16> },
    Fixed(u16),
    Flex { min: u16 },
    FlexMax { min: u16, max: u16 },
}

impl SequenceAggregate for TrackSize {
    fn sequence_flags(&self) -> u8 {
        0
    }
}

/// RETAINED SEMANTIC IR. Structural container holding one semantic child.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContainerNode {
    pub(crate) child: View,
}

/// RETAINED SEMANTIC IR. Common semantic decoration applied by the compiler
/// around a View node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AxisBounds {
    pub(crate) min: u16,
    pub(crate) max: u16,
}

impl Default for AxisBounds {
    fn default() -> Self {
        Self {
            min: 0,
            max: u16::MAX,
        }
    }
}

impl AxisBounds {
    pub(crate) fn normalized_max(self) -> u16 {
        self.max.max(self.min)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ViewBounds {
    pub(crate) width: AxisBounds,
    pub(crate) height: AxisBounds,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Decoration {
    pub(crate) padding: Insets,
    pub(crate) bounds: ViewBounds,
    /// Paints the allocated physical surface, including transparent geometry.
    pub(crate) surface_background: Option<ColorSpec>,
    pub(crate) border: Option<BorderSpec>,
    /// Sparse text intent inherited by descendants and text spans.
    pub(crate) text_style: StyleRef,
}

/// RETAINED SEMANTIC IR. Truncation behavior after physical layout.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClampRowsView {
    pub(crate) child: View,
    pub(crate) max_rows: u16,
    pub(crate) overflow: OverflowIndicator,
}

/// Private physical row crop used by semantic local scroll panes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowViewportView {
    pub(crate) child: View,
    pub(crate) skip_rows: u16,
    /// When set, the viewport contributes this intrinsic height instead of
    /// the child's remaining height. `None` lets the parent provide it.
    pub(crate) visible_height: Option<u16>,
    /// Internal bounded allocation for flexible child layout. This differs
    /// from `visible_height`: the child itself receives this height.
    pub(crate) layout_height: Option<u16>,
    /// Lets a local scroll pane advertise its full content height during
    /// width-only measurement while retaining its allocated viewport height
    /// during bounded layout.
    pub(crate) intrinsic_content_height: bool,
}

#[cfg(test)]
mod tests {
    use super::{PersistentSeq, SeqNode, SequenceAggregate, View, ViewKind};
    use std::sync::Arc;

    #[test]
    fn clone_retains_identity_and_only_clones_the_outer_arc() {
        let original = crate::presentation::factory::text("x");
        let cloned = original.clone();

        assert!(View::ptr_eq(&original, &cloned));
        assert_eq!(original.id(), cloned.id());
        assert_eq!(std::sync::Arc::strong_count(&original.inner), 2);
    }

    #[test]
    fn content_history_transfer_accepts_only_the_complete_adapter_shape() {
        let content = crate::presentation::factory::content_host(1).expect("positive port");
        assert_eq!(
            content.content_history_transfer(),
            Some(super::ContentHistoryTransfer {
                port_id: 1,
                padding: crate::Insets::ZERO,
            })
        );

        let transparent_shell = crate::presentation::factory::column(vec![content.clone()], 0);
        assert!(transparent_shell.content_history_transfer().is_some());
        let padded = crate::presentation::factory::padding(transparent_shell, 2);
        assert_eq!(
            padded
                .content_history_transfer()
                .expect("root padding around a transparent shell is transferable")
                .padding,
            crate::Insets::all(2)
        );
        let padded_content = crate::presentation::factory::padding(content.clone(), 2);
        assert_eq!(
            padded_content
                .content_history_transfer()
                .expect("root ContentHost padding is an adapter-supported transform")
                .padding,
            crate::Insets::all(2)
        );

        let composite = crate::presentation::factory::column(
            vec![
                crate::presentation::factory::text("heading"),
                content.clone(),
            ],
            0,
        );
        assert!(composite.content_history_transfer().is_none());
        let decorated = crate::presentation::factory::background(
            crate::presentation::factory::column(vec![content], 0),
            crate::ColorSpec::ansi(1),
        );
        assert!(decorated.content_history_transfer().is_none());
    }

    #[test]
    fn semantic_mutation_gets_a_new_identity_even_when_unique() {
        let original = crate::presentation::factory::text("x");
        let original_id = original.id();
        let changed = crate::presentation::factory::padding(original, 1);
        assert_ne!(original_id, changed.id());
    }

    #[test]
    fn semantic_mutation_gets_a_new_identity_when_shared() {
        let original = crate::presentation::factory::text("x");
        let shared = original.clone();
        let shared_id = shared.id();
        let changed = crate::presentation::factory::padding(shared, 1);

        assert_ne!(original.id(), changed.id());
        assert_eq!(original.id(), shared_id);
        assert!(!View::ptr_eq(&original, &changed));
    }

    #[test]
    fn semantic_equality_ignores_view_identity() {
        let first =
            crate::presentation::factory::padding(crate::presentation::factory::text("same"), 1);
        let second =
            crate::presentation::factory::padding(crate::presentation::factory::text("same"), 1);

        assert_ne!(first.id(), second.id());
        assert_eq!(first, second);
    }

    #[test]
    fn changing_a_parent_retains_an_unchanged_child_identity() {
        let child = crate::presentation::factory::text("stable");
        let root = crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                child.clone(),
            )],
            0,
        );
        let changed = crate::presentation::factory::padding(root.clone(), 1);

        let ViewKind::Column(original_column) = root.kind() else {
            panic!("expected column root");
        };
        let ViewKind::Column(changed_column) = changed.kind() else {
            panic!("expected column root");
        };
        assert_eq!(original_column.children[0].view.id(), child.id());
        assert_eq!(changed_column.children[0].view.id(), child.id());
        assert!(View::ptr_eq(
            &original_column.children[0].view,
            &changed_column.children[0].view
        ));
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct SequenceValue(usize);

    impl SequenceAggregate for SequenceValue {
        fn sequence_flags(&self) -> u8 {
            0
        }
    }

    fn shared_node_count<T: SequenceAggregate + Clone>(
        left: &Arc<SeqNode<T>>,
        right: &Arc<SeqNode<T>>,
    ) -> usize {
        if Arc::ptr_eq(left, right) {
            return 1;
        }
        match (left.as_ref(), right.as_ref()) {
            (
                SeqNode::Branch {
                    children: left_children,
                    ..
                },
                SeqNode::Branch {
                    children: right_children,
                    ..
                },
            ) => left_children
                .iter()
                .zip(right_children.iter())
                .map(|(left, right)| shared_node_count(left, right))
                .sum(),
            _ => 0,
        }
    }

    #[test]
    fn persistent_sequences_copy_only_structural_paths() {
        for size in [0, 1, 31, 32, 33, 1_024, 10_000, 100_000] {
            let values: Vec<_> = (0..size).map(SequenceValue).collect();
            let original = PersistentSeq::from_vec(values.clone());
            assert_eq!(original.iter().cloned().collect::<Vec<_>>(), values);
            if size == 0 {
                continue;
            }
            let middle = size / 2;
            let changed = original.set(middle, SequenceValue(usize::MAX));
            assert_eq!(original[middle], SequenceValue(middle));
            assert_eq!(changed[middle], SequenceValue(usize::MAX));
            if size > PersistentSeq::<SequenceValue>::BRANCH {
                assert!(shared_node_count(&original.root, &changed.root) > 0);
            }

            let inserted = original.insert(middle, SequenceValue(usize::MAX - 1));
            let removed = inserted.remove(middle);
            assert_eq!(removed, original);
            let (left, right) = inserted.split(middle);
            assert_eq!(left.concat(&right), inserted);
            let spliced = original.splice(middle, 1, vec![SequenceValue(7), SequenceValue(8)]);
            let mut expected = values.clone();
            expected.splice(middle..middle + 1, [SequenceValue(7), SequenceValue(8)]);
            assert_eq!(spliced.iter().cloned().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn component_presence_is_cached_in_flags() {
        let ordinary = crate::presentation::factory::text("ordinary");
        assert!(!ordinary.contains_component_identity());

        let mounted = crate::presentation::factory::column_specs(
            vec![(
                crate::presentation::ir::TrackSize::Content { max: None },
                crate::presentation::factory::native_component(1),
            )],
            0,
        );
        assert!(mounted.contains_component_identity());
        assert!(crate::presentation::factory::padding(mounted, 1).contains_component_identity());
    }
}
