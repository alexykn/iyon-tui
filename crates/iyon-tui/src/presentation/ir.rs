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

#[cfg(feature = "native-host")]
use std::collections::HashSet;

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
    const CONTAINS_STATE_ATTACHMENT: u8 = 1 << 1;

    pub(crate) const fn contains_component_slot(self) -> bool {
        self.0 & Self::CONTAINS_COMPONENT_SLOT != 0
    }

    const fn contains_state_attachment(self) -> bool {
        self.0 & Self::CONTAINS_STATE_ATTACHMENT != 0
    }

    const fn with_component_slot() -> Self {
        Self(Self::CONTAINS_COMPONENT_SLOT)
    }

    const fn with_state_attachment(self) -> Self {
        Self(self.0 | Self::CONTAINS_STATE_ATTACHMENT)
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
        let mut level: Vec<Arc<SeqNode<T>>> = values
            .chunks(Self::BRANCH)
            .map(|items| {
                Arc::new(SeqNode::Leaf {
                    items: items.to_vec().into(),
                    flags: items
                        .iter()
                        .fold(0, |flags, item| flags | item.sequence_flags()),
                })
            })
            .collect();
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
            Self::Branch { children, .. } => children
                .first()
                .map(|child| child.height() + 1)
                .unwrap_or(0),
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
    /// Native retained state attachment. This is a physical preparation value,
    /// never a public semantic/native pointer identity.
    state_attachment: Option<u64>,
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

pub(crate) struct ViewNodeParts {
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) style_states: StyleStates,
    pub(crate) style_facts: StyleFacts,
    pub(crate) kind: ViewKind,
}

impl View {
    pub(crate) fn from_node(parts: ViewNodeParts) -> Self {
        perf::inc(Counter::ViewNodesConstructedRust);
        Self {
            inner: Arc::new(ViewNode {
                id: next_view_id(),
                flags: ViewNode::compute_flags(&parts.kind),
                state_attachment: None,
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

    pub(crate) fn state_attachment_id(&self) -> Option<u64> {
        self.inner.state_attachment
    }

    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_state_attachment_id(&self) -> Option<u64> {
        self.state_attachment_id()
    }

    /// Attaches a retained ViewState identity without introducing a wrapper
    /// occurrence. Native structural transport calls this after constructing
    /// the ordinary semantic value.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_with_state_attachment(self, state_id: u64) -> Result<Self, String> {
        if state_id == 0 {
            return Err("ViewState identity must be positive".to_owned());
        }
        if !self.native_state_capable() {
            return Err("ViewState is unsupported on this node kind".to_owned());
        }
        if self.state_attachment_id() == Some(state_id) {
            return Ok(self);
        }
        Ok(self.map_node(|node| node.state_attachment = Some(state_id)))
    }

    /// Exhaustive native capability classification for retained presentation
    /// state. Component indirections have no independently addressable box;
    /// their concrete component View owns presentation state instead.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_state_capable(&self) -> bool {
        crate::retained_state::presentation_state_capable(self.kind())
    }

    /// Returns every retained state identity in this semantic value. The
    /// native host uses this to establish desired binding before a frame.
    #[cfg(feature = "native-host")]
    pub fn native_state_attachment_ids(&self) -> Result<Vec<u64>, String> {
        Ok(self
            .native_state_attachment_targets()?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    /// Returns each state attachment with the concrete physical kind that owns
    /// it. H3 uses this target table to validate stored geometry overrides
    /// before installing a new desired root.
    #[cfg(feature = "native-host")]
    pub(crate) fn native_state_attachment_targets(
        &self,
    ) -> Result<Vec<(u64, crate::retained_state::StateNodeKind)>, String> {
        if !self.inner.flags.contains_state_attachment() {
            return Ok(Vec::new());
        }
        let mut targets = Vec::new();
        let mut active = HashSet::new();
        self.collect_state_attachment_targets(&mut targets, &mut active)?;
        Ok(targets)
    }

    #[cfg(feature = "native-host")]
    fn collect_state_attachment_targets(
        &self,
        targets: &mut Vec<(u64, crate::retained_state::StateNodeKind)>,
        active: &mut HashSet<ViewId>,
    ) -> Result<(), String> {
        if !self.inner.flags.contains_state_attachment() {
            return Ok(());
        }
        if !active.insert(self.id()) {
            return Err("cyclic semantic View graph".to_owned());
        }
        if let Some(state_id) = self.state_attachment_id() {
            if targets.iter().any(|(id, _)| *id == state_id) {
                return Err("duplicate ViewState attachment".to_owned());
            }
            targets.push((
                state_id,
                crate::retained_state::state_node_kind(self.kind()),
            ));
        }
        match self.kind() {
            ViewKind::Text(_) | ViewKind::Spacer { .. } | ViewKind::ComponentSlot(_) => {}
            ViewKind::Column(column) => {
                for child in column.children.iter() {
                    child
                        .view
                        .collect_state_attachment_targets(targets, active)?;
                }
            }
            ViewKind::Row(row) => {
                for child in row.children.iter() {
                    child
                        .view
                        .collect_state_attachment_targets(targets, active)?;
                }
            }
            ViewKind::Grid(grid) => {
                for cell in grid.cells.iter() {
                    cell.view
                        .collect_state_attachment_targets(targets, active)?;
                }
            }
            ViewKind::Hanging(hanging) => {
                hanging
                    .prefix
                    .collect_state_attachment_targets(targets, active)?;
                hanging
                    .continuation_prefix
                    .collect_state_attachment_targets(targets, active)?;
                hanging
                    .body
                    .collect_state_attachment_targets(targets, active)?;
            }
            ViewKind::Container(container) => {
                container
                    .child
                    .collect_state_attachment_targets(targets, active)?;
            }
            ViewKind::ClampRows(clamp) => {
                clamp
                    .child
                    .collect_state_attachment_targets(targets, active)?;
            }
            ViewKind::RowViewport(viewport) => {
                viewport
                    .child
                    .collect_state_attachment_targets(targets, active)?;
            }
        }
        active.remove(&self.id());
        Ok(())
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
        next.flags = ViewNode::compute_flags(&next.kind);
        if next.state_attachment.is_some() {
            next.flags = next.flags.with_state_attachment();
        }
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

    /// Constructs an axis from scalar track words and already-materialized
    /// child Views. The sequence is built once and remains persistent for
    /// subsequent structural edits.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_axis_from_children(
        horizontal: bool,
        gap: u16,
        children: Vec<(u32, View)>,
    ) -> Result<Self, String> {
        let tracks = children
            .into_iter()
            .map(|(track_word, view)| {
                let track = decode_native_track_word(track_word)?;
                Ok((track, view))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let kind = if horizontal {
            ViewKind::Row(Arc::new(RowView {
                children: PersistentSeq::from_vec(
                    tracks
                        .into_iter()
                        .map(|(track, view)| RowChild { track, view })
                        .collect(),
                ),
                gap,
                vertical_align: VerticalAlign::Top,
            }))
        } else {
            ViewKind::Column(Arc::new(ColumnView {
                children: PersistentSeq::from_vec(
                    tracks
                        .into_iter()
                        .map(|(track, view)| ColumnChild { track, view })
                        .collect(),
                ),
                gap,
            }))
        };
        Ok(Self::new_kind(kind))
    }

    /// Replaces one axis child with a persistent sequence `set`. A zero track
    /// word means preserve the existing track; nonzero words use the compact
    /// `(track kind in low byte, value in high 16 bits)` representation.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_axis_set_child(
        self,
        index: usize,
        track_word: u32,
        child: View,
    ) -> Result<Self, String> {
        match self.kind() {
            ViewKind::Row(row) => {
                let current = row
                    .children
                    .get(index)
                    .ok_or_else(|| "row child index is out of range".to_owned())?;
                let replacement = RowChild {
                    track: if track_word == 0 {
                        current.track
                    } else {
                        decode_native_track_word(track_word)?
                    },
                    view: child,
                };
                let children = row.children.set(index, replacement);
                Ok(self.map_node(|node| {
                    let ViewKind::Row(row) = &mut node.kind else {
                        unreachable!("validated row axis")
                    };
                    Arc::make_mut(row).children = children;
                }))
            }
            ViewKind::Column(column) => {
                let current = column
                    .children
                    .get(index)
                    .ok_or_else(|| "column child index is out of range".to_owned())?;
                let replacement = ColumnChild {
                    track: if track_word == 0 {
                        current.track
                    } else {
                        decode_native_track_word(track_word)?
                    },
                    view: child,
                };
                let children = column.children.set(index, replacement);
                Ok(self.map_node(|node| {
                    let ViewKind::Column(column) = &mut node.kind else {
                        unreachable!("validated column axis")
                    };
                    Arc::make_mut(column).children = children;
                }))
            }
            _ => Err("axis edit base is not a row or column".to_owned()),
        }
    }

    /// Splices axis children through the persistent sequence split/concat
    /// operations. No flat child vector is constructed by this method.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_axis_splice(
        self,
        index: usize,
        remove_count: usize,
        inserted: Vec<(u32, View)>,
    ) -> Result<Self, String> {
        let tracks = inserted
            .into_iter()
            .map(|(track_word, view)| Ok((decode_native_track_word(track_word)?, view)))
            .collect::<Result<Vec<_>, String>>()?;
        match self.kind() {
            ViewKind::Row(row) => {
                if index > row.children.len() || remove_count > row.children.len() - index {
                    return Err("row axis splice range is out of bounds".to_owned());
                }
                let values = tracks
                    .into_iter()
                    .map(|(track, view)| RowChild { track, view })
                    .collect();
                let children = row.children.splice(index, remove_count, values);
                Ok(self.map_node(|node| {
                    let ViewKind::Row(row) = &mut node.kind else {
                        unreachable!("validated row axis")
                    };
                    Arc::make_mut(row).children = children;
                }))
            }
            ViewKind::Column(column) => {
                if index > column.children.len() || remove_count > column.children.len() - index {
                    return Err("column axis splice range is out of bounds".to_owned());
                }
                let values = tracks
                    .into_iter()
                    .map(|(track, view)| ColumnChild { track, view })
                    .collect();
                let children = column.children.splice(index, remove_count, values);
                Ok(self.map_node(|node| {
                    let ViewKind::Column(column) = &mut node.kind else {
                        unreachable!("validated column axis")
                    };
                    Arc::make_mut(column).children = children;
                }))
            }
            _ => Err("axis splice base is not a row or column".to_owned()),
        }
    }

    /// Replaces the semantic View of one grid cell while retaining its
    /// placement metadata and sharing every unchanged sequence node.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_grid_set_cell(
        self,
        row: usize,
        column: usize,
        child: View,
    ) -> Result<Self, String> {
        let ViewKind::Grid(grid) = self.kind() else {
            return Err("grid edit base is not a grid".to_owned());
        };
        let index = *grid
            .cell_indices
            .get(&(row, column))
            .ok_or_else(|| "grid cell coordinates are out of range".to_owned())?;
        let current = grid
            .cells
            .get(index)
            .ok_or_else(|| "grid cell index is out of range".to_owned())?;
        let mut replacement = current.clone();
        replacement.view = child;
        let cells = grid.cells.set(index, replacement);
        Ok(self.map_node(|node| {
            let ViewKind::Grid(grid) = &mut node.kind else {
                unreachable!("validated grid")
            };
            Arc::make_mut(grid).cells = cells;
        }))
    }

    /// Applies a retained axis or grid child update at a path target.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_replace_at_path(
        self,
        steps: &[RetainedPathStep],
        axis_index: Option<usize>,
        track_word: u32,
        grid_row: Option<usize>,
        grid_column: Option<usize>,
        child: View,
    ) -> Result<(Self, Vec<Self>), String> {
        let Some(step) = steps.first().copied() else {
            let patched = if let Some(index) = axis_index {
                self.native_axis_set_child(index, track_word, child)?
            } else {
                self.native_grid_set_cell(
                    grid_row.ok_or_else(|| "grid row is missing".to_owned())?,
                    grid_column.ok_or_else(|| "grid column is missing".to_owned())?,
                    child,
                )?
            };
            return Ok((patched.clone(), vec![patched]));
        };
        let nested = self.try_retained_child(step)?;
        let (changed, mut views) = nested.native_replace_at_path(
            &steps[1..],
            axis_index,
            track_word,
            grid_row,
            grid_column,
            child,
        )?;
        let rebuilt = self.try_replace_retained_child(step, changed)?;
        views.push(rebuilt.clone());
        Ok((rebuilt, views))
    }

    #[cfg(feature = "native-host")]
    pub fn downgrade(&self) -> WeakView {
        WeakView {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

/// A compact, caller-owned path step for retained native edits.
///
/// Path nodes contain only semantic selectors and expected parent kinds; they
/// never retain a `View`. The native ABI interns these steps as `PathRef`s.
#[cfg(feature = "native-host")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedPathStep {
    pub kind: u32,
    pub expected_view_kind: u32,
    pub selector: u32,
}

#[cfg(feature = "native-host")]
#[doc(hidden)]
impl RetainedPathStep {
    pub const fn new(kind: u32, expected_view_kind: u32, selector: u32) -> Self {
        Self {
            kind,
            expected_view_kind,
            selector,
        }
    }
}

#[cfg(feature = "native-host")]
const PATH_VIEW_TEXT: u32 = 1;
#[cfg(feature = "native-host")]
const PATH_VIEW_ROW: u32 = 2;
#[cfg(feature = "native-host")]
const PATH_VIEW_COLUMN: u32 = 3;
#[cfg(feature = "native-host")]
const PATH_VIEW_GRID: u32 = 4;
#[cfg(feature = "native-host")]
const PATH_VIEW_HANGING: u32 = 5;
#[cfg(feature = "native-host")]
const PATH_VIEW_CONTAINER: u32 = 6;
#[cfg(feature = "native-host")]
const PATH_VIEW_CLAMP_ROWS: u32 = 7;
#[cfg(feature = "native-host")]
const PATH_VIEW_ROW_VIEWPORT: u32 = 8;

#[cfg(feature = "native-host")]
const PATH_STEP_CONTAINER_CHILD: u32 = 1;
#[cfg(feature = "native-host")]
const PATH_STEP_CLAMP_CHILD: u32 = 2;
#[cfg(feature = "native-host")]
const PATH_STEP_ROW_VIEWPORT_CHILD: u32 = 3;
#[cfg(feature = "native-host")]
const PATH_STEP_COLUMN_CHILD: u32 = 4;
#[cfg(feature = "native-host")]
const PATH_STEP_ROW_CHILD: u32 = 5;
#[cfg(feature = "native-host")]
const PATH_STEP_GRID_CELL: u32 = 6;
#[cfg(feature = "native-host")]
const PATH_STEP_HANGING_PREFIX: u32 = 7;
#[cfg(feature = "native-host")]
const PATH_STEP_HANGING_CONTINUATION: u32 = 8;
#[cfg(feature = "native-host")]
const PATH_STEP_HANGING_BODY: u32 = 9;

#[cfg(feature = "native-host")]
impl View {
    /// Applies a text-layout edit through a validated retained path.
    ///
    /// The recursion follows only the supplied path and rebuilds each changed
    /// ancestor once. Axis and grid children use `PersistentSeq::set`, so a
    /// wide parent never becomes a flat copy during a path edit.
    #[doc(hidden)]
    pub fn try_with_text_layout_patch_path(
        self,
        steps: &[RetainedPathStep],
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> Result<Self, String> {
        if steps.len() > 128 {
            return Err("retained path exceeds maximum depth".to_owned());
        }
        patch_text_layout_path_with_nodes(self, steps, wrap, align).map(|(view, _)| view)
    }

    /// Applies a retained path edit and returns the changed leaf followed by
    /// each rebuilt ancestor. Native uses these values to publish every JS
    /// semantic NodeId without sending a NodeId array over FFI.
    #[doc(hidden)]
    pub fn try_with_text_layout_patch_path_with_nodes(
        self,
        steps: &[RetainedPathStep],
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> Result<(Self, Vec<Self>), String> {
        if steps.len() > 128 {
            return Err("retained path exceeds maximum depth".to_owned());
        }
        patch_text_layout_path_with_nodes(self, steps, wrap, align)
    }

    /// Returns the child selected by one validated retained-path step.
    /// Transaction staging uses this to descend a native path trie without
    /// rebuilding or flattening any unchanged sibling sequence.
    #[doc(hidden)]
    pub fn try_retained_child(&self, step: RetainedPathStep) -> Result<Self, String> {
        if view_kind_tag(self.kind()) != step.expected_view_kind {
            return Err("retained path expected view kind does not match base".to_owned());
        }
        match step.kind {
            PATH_STEP_CONTAINER_CHILD => {
                if step.selector != 0 {
                    return Err("container path selector must be zero".to_owned());
                }
                let ViewKind::Container(container) = self.kind() else {
                    return Err("container path step requires a container".to_owned());
                };
                Ok(container.child.clone())
            }
            PATH_STEP_CLAMP_CHILD => {
                if step.selector != 0 {
                    return Err("clamp path selector must be zero".to_owned());
                }
                let ViewKind::ClampRows(clamp) = self.kind() else {
                    return Err("clamp path step requires a clamp".to_owned());
                };
                Ok(clamp.child.clone())
            }
            PATH_STEP_ROW_VIEWPORT_CHILD => {
                if step.selector != 0 {
                    return Err("row viewport path selector must be zero".to_owned());
                }
                let ViewKind::RowViewport(viewport) = self.kind() else {
                    return Err("row viewport path step requires a viewport".to_owned());
                };
                Ok(viewport.child.clone())
            }
            PATH_STEP_COLUMN_CHILD => {
                let ViewKind::Column(column) = self.kind() else {
                    return Err("column path step requires a column".to_owned());
                };
                column
                    .children
                    .get(step.selector as usize)
                    .map(|child| child.view.clone())
                    .ok_or_else(|| "column path selector is out of range".to_owned())
            }
            PATH_STEP_ROW_CHILD => {
                let ViewKind::Row(row) = self.kind() else {
                    return Err("row path step requires a row".to_owned());
                };
                row.children
                    .get(step.selector as usize)
                    .map(|child| child.view.clone())
                    .ok_or_else(|| "row path selector is out of range".to_owned())
            }
            PATH_STEP_GRID_CELL => {
                let ViewKind::Grid(grid) = self.kind() else {
                    return Err("grid path step requires a grid".to_owned());
                };
                grid.cells
                    .get(step.selector as usize)
                    .map(|cell| cell.view.clone())
                    .ok_or_else(|| "grid path selector is out of range".to_owned())
            }
            PATH_STEP_HANGING_PREFIX | PATH_STEP_HANGING_CONTINUATION | PATH_STEP_HANGING_BODY => {
                let ViewKind::Hanging(hanging) = self.kind() else {
                    return Err("hanging path step requires a hanging view".to_owned());
                };
                if step.selector != 0 {
                    return Err("hanging path selector must be zero".to_owned());
                }
                Ok(match step.kind {
                    PATH_STEP_HANGING_PREFIX => hanging.prefix.clone(),
                    PATH_STEP_HANGING_CONTINUATION => hanging.continuation_prefix.clone(),
                    _ => hanging.body.clone(),
                })
            }
            _ => Err("unknown retained path step".to_owned()),
        }
    }

    /// Replaces one retained-path child with a persistent structural update.
    /// The receiver is immutable; all unaffected subtrees and sequence nodes
    /// remain shared with it.
    #[doc(hidden)]
    pub fn try_replace_retained_child(
        self,
        step: RetainedPathStep,
        child: Self,
    ) -> Result<Self, String> {
        if view_kind_tag(self.kind()) != step.expected_view_kind {
            return Err("retained path expected view kind does not match base".to_owned());
        }
        match step.kind {
            PATH_STEP_CONTAINER_CHILD => {
                if step.selector != 0 {
                    return Err("container path selector must be zero".to_owned());
                }
                let ViewKind::Container(_) = self.kind() else {
                    return Err("container path step requires a container".to_owned());
                };
                Ok(self.map_node(|node| {
                    let ViewKind::Container(container) = &mut node.kind else {
                        unreachable!("validated container path")
                    };
                    Arc::make_mut(container).child = child;
                }))
            }
            PATH_STEP_CLAMP_CHILD => {
                if step.selector != 0 {
                    return Err("clamp path selector must be zero".to_owned());
                }
                let ViewKind::ClampRows(_) = self.kind() else {
                    return Err("clamp path step requires a clamp".to_owned());
                };
                Ok(self.map_node(|node| {
                    let ViewKind::ClampRows(clamp) = &mut node.kind else {
                        unreachable!("validated clamp path")
                    };
                    Arc::make_mut(clamp).child = child;
                }))
            }
            PATH_STEP_ROW_VIEWPORT_CHILD => {
                if step.selector != 0 {
                    return Err("row viewport path selector must be zero".to_owned());
                }
                let ViewKind::RowViewport(_) = self.kind() else {
                    return Err("row viewport path step requires a viewport".to_owned());
                };
                Ok(self.map_node(|node| {
                    let ViewKind::RowViewport(viewport) = &mut node.kind else {
                        unreachable!("validated viewport path")
                    };
                    Arc::make_mut(viewport).child = child;
                }))
            }
            PATH_STEP_COLUMN_CHILD => {
                let ViewKind::Column(column) = self.kind() else {
                    return Err("column path step requires a column".to_owned());
                };
                let current = column
                    .children
                    .get(step.selector as usize)
                    .ok_or_else(|| "column path selector is out of range".to_owned())?;
                let replacement = ColumnChild {
                    track: current.track,
                    view: child,
                };
                let children = column.children.set(step.selector as usize, replacement);
                Ok(self.map_node(|node| {
                    let ViewKind::Column(column) = &mut node.kind else {
                        unreachable!("validated column path")
                    };
                    Arc::make_mut(column).children = children;
                }))
            }
            PATH_STEP_ROW_CHILD => {
                let ViewKind::Row(row) = self.kind() else {
                    return Err("row path step requires a row".to_owned());
                };
                let current = row
                    .children
                    .get(step.selector as usize)
                    .ok_or_else(|| "row path selector is out of range".to_owned())?;
                let replacement = RowChild {
                    track: current.track,
                    view: child,
                };
                let children = row.children.set(step.selector as usize, replacement);
                Ok(self.map_node(|node| {
                    let ViewKind::Row(row) = &mut node.kind else {
                        unreachable!("validated row path")
                    };
                    Arc::make_mut(row).children = children;
                }))
            }
            PATH_STEP_GRID_CELL => {
                let ViewKind::Grid(grid) = self.kind() else {
                    return Err("grid path step requires a grid".to_owned());
                };
                let current = grid
                    .cells
                    .get(step.selector as usize)
                    .ok_or_else(|| "grid path selector is out of range".to_owned())?;
                let mut replacement = current.clone();
                replacement.view = child;
                let cells = grid.cells.set(step.selector as usize, replacement);
                Ok(self.map_node(|node| {
                    let ViewKind::Grid(grid) = &mut node.kind else {
                        unreachable!("validated grid path")
                    };
                    Arc::make_mut(grid).cells = cells;
                }))
            }
            PATH_STEP_HANGING_PREFIX | PATH_STEP_HANGING_CONTINUATION | PATH_STEP_HANGING_BODY => {
                let ViewKind::Hanging(_) = self.kind() else {
                    return Err("hanging path step requires a hanging view".to_owned());
                };
                if step.selector != 0 {
                    return Err("hanging path selector must be zero".to_owned());
                }
                Ok(self.map_node(|node| {
                    let ViewKind::Hanging(hanging) = &mut node.kind else {
                        unreachable!("validated hanging path")
                    };
                    let hanging = Arc::make_mut(hanging);
                    match step.kind {
                        PATH_STEP_HANGING_PREFIX => hanging.prefix = child,
                        PATH_STEP_HANGING_CONTINUATION => hanging.continuation_prefix = child,
                        _ => hanging.body = child,
                    }
                }))
            }
            _ => Err("unknown retained path step".to_owned()),
        }
    }
}

#[cfg(feature = "native-host")]
fn patch_text_layout_path_with_nodes(
    view: View,
    steps: &[RetainedPathStep],
    wrap: WrapMode,
    align: HorizontalAlign,
) -> Result<(View, Vec<View>), String> {
    let Some(step) = steps.first().copied() else {
        let patched = view
            .try_with_text_layout_patch(Some(wrap), Some(align))
            .map_err(str::to_owned)?;
        return Ok((patched.clone(), vec![patched]));
    };
    if view_kind_tag(view.kind()) != step.expected_view_kind {
        return Err("retained path expected view kind does not match base".to_owned());
    }
    let tail = &steps[1..];
    match step.kind {
        PATH_STEP_CONTAINER_CHILD => {
            if step.selector != 0 {
                return Err("container path selector must be zero".to_owned());
            }
            let ViewKind::Container(container) = view.kind() else {
                return Err("container path step requires a container".to_owned());
            };
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(container.child.clone(), tail, wrap, align)?;
            let patched = view.map_node(|node| {
                let ViewKind::Container(container) = &mut node.kind else {
                    unreachable!("validated container path")
                };
                Arc::make_mut(container).child = child;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_CLAMP_CHILD => {
            if step.selector != 0 {
                return Err("clamp path selector must be zero".to_owned());
            }
            let ViewKind::ClampRows(clamp) = view.kind() else {
                return Err("clamp path step requires a clamp".to_owned());
            };
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(clamp.child.clone(), tail, wrap, align)?;
            let patched = view.map_node(|node| {
                let ViewKind::ClampRows(clamp) = &mut node.kind else {
                    unreachable!("validated clamp path")
                };
                Arc::make_mut(clamp).child = child;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_ROW_VIEWPORT_CHILD => {
            if step.selector != 0 {
                return Err("row viewport path selector must be zero".to_owned());
            }
            let ViewKind::RowViewport(viewport) = view.kind() else {
                return Err("row viewport path step requires a viewport".to_owned());
            };
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(viewport.child.clone(), tail, wrap, align)?;
            let patched = view.map_node(|node| {
                let ViewKind::RowViewport(viewport) = &mut node.kind else {
                    unreachable!("validated row viewport path")
                };
                Arc::make_mut(viewport).child = child;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_COLUMN_CHILD => {
            let ViewKind::Column(column) = view.kind() else {
                return Err("column path step requires a column".to_owned());
            };
            let current = column
                .children
                .get(step.selector as usize)
                .ok_or_else(|| "column path selector is out of range".to_owned())?;
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(current.view.clone(), tail, wrap, align)?;
            let replacement = ColumnChild {
                track: current.track,
                view: child,
            };
            let children = column.children.set(step.selector as usize, replacement);
            let patched = view.map_node(|node| {
                let ViewKind::Column(column) = &mut node.kind else {
                    unreachable!("validated column path")
                };
                Arc::make_mut(column).children = children;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_ROW_CHILD => {
            let ViewKind::Row(row) = view.kind() else {
                return Err("row path step requires a row".to_owned());
            };
            let current = row
                .children
                .get(step.selector as usize)
                .ok_or_else(|| "row path selector is out of range".to_owned())?;
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(current.view.clone(), tail, wrap, align)?;
            let replacement = RowChild {
                track: current.track,
                view: child,
            };
            let children = row.children.set(step.selector as usize, replacement);
            let patched = view.map_node(|node| {
                let ViewKind::Row(row) = &mut node.kind else {
                    unreachable!("validated row path")
                };
                Arc::make_mut(row).children = children;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_GRID_CELL => {
            let ViewKind::Grid(grid) = view.kind() else {
                return Err("grid path step requires a grid".to_owned());
            };
            let current = grid
                .cells
                .get(step.selector as usize)
                .ok_or_else(|| "grid path selector is out of range".to_owned())?;
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(current.view.clone(), tail, wrap, align)?;
            let mut replacement = current.clone();
            replacement.view = child;
            let cells = grid.cells.set(step.selector as usize, replacement);
            let patched = view.map_node(|node| {
                let ViewKind::Grid(grid) = &mut node.kind else {
                    unreachable!("validated grid path")
                };
                Arc::make_mut(grid).cells = cells;
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        PATH_STEP_HANGING_PREFIX | PATH_STEP_HANGING_CONTINUATION | PATH_STEP_HANGING_BODY => {
            let ViewKind::Hanging(hanging) = view.kind() else {
                return Err("hanging path step requires a hanging view".to_owned());
            };
            if step.selector != 0 {
                return Err("hanging path selector must be zero".to_owned());
            }
            let child = match step.kind {
                PATH_STEP_HANGING_PREFIX => &hanging.prefix,
                PATH_STEP_HANGING_CONTINUATION => &hanging.continuation_prefix,
                _ => &hanging.body,
            };
            let (child, mut nodes) =
                patch_text_layout_path_with_nodes(child.clone(), tail, wrap, align)?;
            let patched = view.map_node(|node| {
                let ViewKind::Hanging(hanging) = &mut node.kind else {
                    unreachable!("validated hanging path")
                };
                let hanging = Arc::make_mut(hanging);
                match step.kind {
                    PATH_STEP_HANGING_PREFIX => hanging.prefix = child,
                    PATH_STEP_HANGING_CONTINUATION => hanging.continuation_prefix = child,
                    _ => hanging.body = child,
                }
            });
            nodes.push(patched.clone());
            Ok((patched, nodes))
        }
        _ => Err("unknown retained path step".to_owned()),
    }
}

#[cfg(feature = "native-host")]
fn view_kind_tag(kind: &ViewKind) -> u32 {
    match kind {
        ViewKind::Text(_) => PATH_VIEW_TEXT,
        ViewKind::Row(_) => PATH_VIEW_ROW,
        ViewKind::Column(_) => PATH_VIEW_COLUMN,
        ViewKind::Grid(_) => PATH_VIEW_GRID,
        ViewKind::Hanging(_) => PATH_VIEW_HANGING,
        ViewKind::Container(_) => PATH_VIEW_CONTAINER,
        ViewKind::ClampRows(_) => PATH_VIEW_CLAMP_ROWS,
        ViewKind::RowViewport(_) => PATH_VIEW_ROW_VIEWPORT,
        ViewKind::Spacer { .. } | ViewKind::ComponentSlot(_) => 0,
    }
}

impl PartialEq for ViewNode {
    fn eq(&self, other: &Self) -> bool {
        self.semantic_eq(other)
    }
}

impl ViewNode {
    fn compute_flags(kind: &ViewKind) -> ViewFlags {
        match kind {
            ViewKind::ComponentSlot(_) => ViewFlags::with_component_slot(),
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
            state_attachment: self.state_attachment,
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
            && self.state_attachment == other.state_attachment
            && self.kind == other.kind
    }
}

#[cfg(feature = "native-host")]
#[derive(Clone)]
pub struct WeakView {
    inner: std::sync::Weak<ViewNode>,
}

#[cfg(feature = "native-host")]
impl WeakView {
    pub fn upgrade(&self) -> Option<View> {
        self.inner.upgrade().map(|inner| View { inner })
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
    Spacer { rows: u16 },
    ClampRows(Arc<ClampRowsView>),
    RowViewport(Arc<RowViewportView>),
    ComponentSlot(ComponentSlotNode),
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
    use crate::presentation::IntoView;
    use std::sync::Arc;

    #[test]
    fn clone_retains_identity_and_only_clones_the_outer_arc() {
        let original = View::text("x").into_view();
        let cloned = original.clone();

        assert!(View::ptr_eq(&original, &cloned));
        assert_eq!(original.id(), cloned.id());
        assert_eq!(std::sync::Arc::strong_count(&original.inner), 2);
    }

    #[test]
    fn semantic_mutation_gets_a_new_identity_even_when_unique() {
        let original = View::text("x").into_view();
        let original_id = original.id();
        let changed = original.padding(1);
        assert_ne!(original_id, changed.id());
    }

    #[test]
    fn semantic_mutation_gets_a_new_identity_when_shared() {
        let original = View::text("x").into_view();
        let shared = original.clone();
        let shared_id = shared.id();
        let changed = shared.padding(1);

        assert_ne!(original.id(), changed.id());
        assert_eq!(original.id(), shared_id);
        assert!(!View::ptr_eq(&original, &changed));
    }

    #[test]
    fn semantic_equality_ignores_view_identity() {
        let first = View::text("same").padding(1).into_view();
        let second = View::text("same").padding(1).into_view();

        assert_ne!(first.id(), second.id());
        assert_eq!(first, second);
    }

    #[test]
    fn changing_a_parent_retains_an_unchanged_child_identity() {
        let child = View::text("stable").into_view();
        let root = View::vertical(|column| {
            column.child(child.clone());
        });
        let changed = root.clone().padding(1);

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
        let ordinary = View::text("ordinary").into_view();
        assert!(!ordinary.contains_component_identity());

        let mounted = View::vertical(|column| {
            column.child(View::native_component(1));
        });
        assert!(mounted.contains_component_identity());
        assert!(mounted.padding(1).contains_component_identity());
    }

    #[cfg(feature = "native-host")]
    #[test]
    fn weak_bridge_handles_do_not_keep_views_alive() {
        let view = View::text("weak").into_view();
        let weak = view.downgrade();
        let upgraded = weak.upgrade().expect("live view must upgrade");
        assert_eq!(upgraded.id(), view.id());
        drop(upgraded);
        drop(view);
        assert!(weak.upgrade().is_none());
    }
}
