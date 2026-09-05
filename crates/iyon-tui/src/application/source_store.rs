//! Persistent Source storage: immutable pages, indexed chunk/line roots,
//! and an indexed annotation tree.
//!
//! The previous container copy-on-wrote whole chunk/line/annotation deques,
//! so every append cloned metadata proportional to the entire retained
//! Source while snapshots lived. This module replaces it with persistent
//! roots: snapshots retain root references and scalar stamps (O(1)),
//! appends copy only the right-edge path plus new pages, head truncation
//! splits by index, and old roots are shared, never mutated.
//!
//! Layout per §9.4: immutable UTF-8 pages with range views (a split shares
//! the page `Arc` and copies zero bytes), a persistent chunk sequence with
//! per-branch byte/line counts, and a persistent annotation index keyed by
//! `(start, seqno)`. Line entries are derived, never stored globally:
//! entry `0` is the range base and entry `i > 0` is the position after the
//! `(i-1)`-th newline, exactly matching the previous `line_starts` vector.

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::Result;

use crate::text::SemanticTag;
use crate::StyleRef;

pub(crate) const SOURCE_CHUNK_BYTES: usize = 16 * 1024;
pub(crate) const MAX_SOURCE_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_SOURCE_ANNOTATIONS: usize = 16 * 1024;
pub(crate) const MAX_ANNOTATION_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

/// Initial annotation kinds are deliberately closed and host-independent.
pub(crate) const CONTENT_ANNOTATION_KIND_TAG: u32 = 1;
pub(crate) const CONTENT_ANNOTATION_KIND_STYLE: u32 = 2;
pub(crate) const CONTENT_ANNOTATION_KIND_ATOMIC: u32 = 3;
pub(crate) const CONTENT_ANNOTATION_KIND_POINT: u32 = 4;

const LEAF_DESCS: usize = 16;
const BRANCH_CHILDREN: usize = 16;

/// One validated ingestion boundary (§9.3). UTF-8 is validated exactly once
/// and the newline count is computed in a single byte pass; chunking,
/// decoding, and retention preflight all share this view instead of
/// rescanning the input or the retained bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedInput<'a> {
    bytes: &'a [u8],
    text: &'a str,
    newlines: usize,
}

impl<'a> ValidatedInput<'a> {
    pub(crate) fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        let text =
            str::from_utf8(bytes).map_err(|_| anyhow::anyhow!("INVALID_UTF8: Source payload is not UTF-8"))?;
        Ok(Self {
            bytes,
            text,
            newlines: bytes.iter().filter(|byte| **byte == b'\n').count(),
        })
    }

    pub(crate) fn from_str(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            text,
            newlines: text.as_bytes().iter().filter(|byte| **byte == b'\n').count(),
        }
    }

    #[must_use]
    pub(crate) fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub(crate) fn text(&self) -> &'a str {
        self.text
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub(crate) fn newlines(&self) -> usize {
        self.newlines
    }
}

/// One range view over an immutable UTF-8 page. Splits share the page `Arc`
/// and copy zero bytes; `line_offsets` holds the relative `newline + 1`
/// positions strictly inside the view (never the view start itself).
#[derive(Clone, Debug)]
struct ChunkDesc {
    page: Arc<str>,
    page_start: u32,
    len: u32,
    abs_start: u64,
    line_offsets: Arc<[u32]>,
}

impl ChunkDesc {
    fn len_u64(&self) -> u64 {
        u64::from(self.len)
    }

    fn end(&self) -> u64 {
        self.abs_start.saturating_add(self.len_u64())
    }

    fn bytes(&self) -> &str {
        let start = self.page_start as usize;
        self.page
            .get(start..start + self.len as usize)
            .expect("chunk views always land on char boundaries")
    }

    fn newlines(&self) -> u64 {
        self.line_offsets.len() as u64
    }

    /// Splits at an absolute offset strictly inside the view. Both halves
    /// share the page; only the small offset arrays are rebuilt.
    fn split_at(&self, offset: u64) -> Option<(Self, Self)> {
        if offset <= self.abs_start || offset >= self.end() {
            return None;
        }
        let local = (offset - self.abs_start) as u32;
        let boundary = self.line_offsets.partition_point(|off| *off <= local);
        let (left_offsets, right_offsets) = self.line_offsets.split_at(boundary);
        let right_offsets = right_offsets
            .iter()
            .map(|off| off - local)
            .collect::<Vec<_>>();
        Some((
            Self {
                page: Arc::clone(&self.page),
                page_start: self.page_start,
                len: local,
                abs_start: self.abs_start,
                line_offsets: Arc::from(left_offsets),
            },
            Self {
                page: Arc::clone(&self.page),
                page_start: self.page_start + local,
                len: self.len - local,
                abs_start: offset,
                line_offsets: Arc::from(right_offsets.as_slice()),
            },
        ))
    }
}

#[derive(Clone, Debug)]
enum ChunkNode {
    Leaf {
        descs: Vec<ChunkDesc>,
        bytes: u64,
        newlines: u64,
    },
    Branch {
        children: Vec<Arc<ChunkNode>>,
        bytes: u64,
        newlines: u64,
        leaves: usize,
    },
}

impl ChunkNode {
    fn bytes(&self) -> u64 {
        match self {
            Self::Leaf { bytes, .. } => *bytes,
            Self::Branch { bytes, .. } => *bytes,
        }
    }

    fn newlines(&self) -> u64 {
        match self {
            Self::Leaf { newlines, .. } => *newlines,
            Self::Branch { newlines, .. } => *newlines,
        }
    }

    fn leaves(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Branch { leaves, .. } => *leaves,
        }
    }

    fn leaf_node(descs: Vec<ChunkDesc>) -> Arc<Self> {
        debug_assert!(!descs.is_empty() && descs.len() <= LEAF_DESCS);
        let mut bytes = 0u64;
        let mut newlines = 0u64;
        for desc in &descs {
            bytes = bytes.saturating_add(desc.len_u64());
            newlines = newlines.saturating_add(desc.newlines());
        }
        Arc::new(Self::Leaf {
            descs,
            bytes,
            newlines,
        })
    }

    fn branch_node(children: Vec<Arc<Self>>) -> Arc<Self> {
        debug_assert!(children.len() <= BRANCH_CHILDREN);
        let mut bytes = 0u64;
        let mut newlines = 0u64;
        let mut leaves = 0usize;
        for child in &children {
            bytes = bytes.saturating_add(child.bytes());
            newlines = newlines.saturating_add(child.newlines());
            leaves = leaves.saturating_add(child.leaves());
        }
        Arc::new(Self::Branch {
            children,
            bytes,
            newlines,
            leaves,
        })
    }
}

/// Persistent indexed chunk sequence. Leaves hold up to `LEAF_DESCS` range
/// views; branches hold byte/newline sums, so appends copy only the
/// right-edge path, splits divide by absolute offset, and line queries walk
/// the sums without a global line vector.
#[derive(Clone, Debug, Default)]
pub(crate) struct ChunkTree {
    root: Option<Arc<ChunkNode>>,
}

impl ChunkTree {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub(crate) fn total_bytes(&self) -> u64 {
        self.root.as_ref().map_or(0, |root| root.bytes())
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.total_bytes()
    }

    pub(crate) fn total_newlines(&self) -> u64 {
        self.root.as_ref().map_or(0, |root| root.newlines())
    }

    pub(crate) fn leaf_count(&self) -> usize {
        self.root.as_ref().map_or(0, |root| root.leaves())
    }

    /// Line entries in a range: entry `0` is the base, entry `i > 0` follows
    /// the `(i-1)`-th newline. Matches the previous `line_starts` vector.
    pub(crate) fn line_entries(&self) -> u64 {
        self.total_newlines().saturating_add(1)
    }

    pub(crate) fn appended(&self, text: &str, base: u64) -> Self {
        if text.is_empty() {
            return self.clone();
        }
        let mut tree = self.clone();
        let bytes = text.as_bytes();
        let mut cursor = 0;
        let mut absolute = base;
        while cursor < bytes.len() {
            let mut end = (cursor + SOURCE_CHUNK_BYTES).min(bytes.len());
            while end < bytes.len() && (bytes[end] & 0xc0) == 0x80 {
                end -= 1;
            }
            if end == cursor {
                end = (cursor + SOURCE_CHUNK_BYTES).min(bytes.len());
            }
            let part = &text[cursor..end];
            let page: Arc<str> = Arc::from(part);
            let mut line_offsets = Vec::new();
            for (idx, b) in part.bytes().enumerate() {
                if b == b'\n' {
                    line_offsets.push((idx + 1) as u32);
                }
            }
            let desc = ChunkDesc {
                page,
                page_start: 0,
                len: part.len() as u32,
                abs_start: absolute,
                line_offsets: Arc::from(line_offsets),
            };
            tree = tree.append_desc(desc);
            absolute = absolute.saturating_add(part.len() as u64);
            cursor = end;
        }
        tree
    }

    pub(crate) fn truncated_head(&self, base: u64, offset: u64) -> (Self, u64) {
        if offset <= base {
            return (self.clone(), 0);
        }
        let dropped = offset.saturating_sub(base);
        let (_left, right) = self.split_at(base, offset);
        (right, dropped)
    }

    pub(crate) fn from_descs(descs: Vec<ChunkDesc>) -> Self {
        if descs.is_empty() {
            return Self::default();
        }
        let mut level = descs
            .chunks(LEAF_DESCS)
            .map(|chunk| ChunkNode::leaf_node(chunk.to_vec()))
            .collect::<Vec<_>>();
        while level.len() > BRANCH_CHILDREN {
            level = level
                .chunks(BRANCH_CHILDREN)
                .map(|chunk| ChunkNode::branch_node(chunk.to_vec()))
                .collect();
        }
        let root = if level.len() == 1 {
            level.pop().expect("chunk level")
        } else {
            ChunkNode::branch_node(level)
        };
        Self { root: Some(root) }
    }

    /// Appends one descriptor on the right edge with B-tree splits, so
    /// repeated tiny appends keep logarithmic depth instead of chaining.
    pub(crate) fn append_desc(&self, desc: ChunkDesc) -> Self {
        let Some(root) = self.root.clone() else {
            return Self::from_descs(vec![desc]);
        };
        let (root, carry) = Self::insert_right(root, desc);
        let root = match carry {
            None => root,
            Some(sibling) => ChunkNode::branch_node(vec![root, sibling]),
        };
        Self { root: Some(root) }
    }

    /// Inserts on the right edge; returns the rebuilt node plus an optional
    /// split sibling to link above.
    fn insert_right(node: Arc<ChunkNode>, desc: ChunkDesc) -> (Arc<ChunkNode>, Option<Arc<ChunkNode>>) {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => {
                let mut descs = descs.clone();
                descs.push(desc);
                if descs.len() <= LEAF_DESCS {
                    (ChunkNode::leaf_node(descs), None)
                } else {
                    let right = descs.split_off(descs.len() / 2);
                    (
                        ChunkNode::leaf_node(descs),
                        Some(ChunkNode::leaf_node(right)),
                    )
                }
            }
            ChunkNode::Branch { children, .. } => {
                let mut children = children.clone();
                let Some(last) = children.pop() else {
                    return (ChunkNode::branch_node(vec![node]), None);
                };
                let (rebuilt, carry) = Self::insert_right(last, desc);
                children.push(rebuilt);
                if let Some(carry) = carry {
                    children.push(carry);
                }
                if children.len() <= BRANCH_CHILDREN {
                    (ChunkNode::branch_node(children), None)
                } else {
                    let right = children.split_off(children.len() / 2);
                    (
                        ChunkNode::branch_node(children),
                        Some(ChunkNode::branch_node(right)),
                    )
                }
            }
        }
    }

    /// Splits into `(bytes < offset, bytes >= offset)` by absolute offset.
    /// Page bytes are shared; only the boundary view is re-described.
    pub(crate) fn split_at(&self, base: u64, offset: u64) -> (Self, Self) {
        let Some(root) = self.root.clone() else {
            return (Self::default(), Self::default());
        };
        if offset <= base {
            return (Self::default(), self.clone());
        }
        // Splits compare absolute offsets: descriptors keep their original
        // absolute starts, so shared subtrees stay valid under any base.
        let (left, right) = Self::split_node(&root, base, offset);
        (
            Self { root: left },
            Self { root: right },
        )
    }

    fn split_node(
        node: &Arc<ChunkNode>,
        abs_start: u64,
        offset: u64,
    ) -> (Option<Arc<ChunkNode>>, Option<Arc<ChunkNode>>) {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => {
                let mut left = Vec::new();
                let mut right = VecDeque::new();
                let mut divided = false;
                for desc in descs {
                    if divided {
                        right.push_back(desc.clone());
                    } else if offset <= desc.abs_start {
                        divided = true;
                        right.push_back(desc.clone());
                    } else if offset >= desc.end() {
                        left.push(desc.clone());
                    } else {
                        divided = true;
                        let (before, after) =
                            desc.split_at(offset).expect("offset is strictly inside");
                        left.push(before);
                        right.push_back(after);
                    }
                }
                (
                    (!left.is_empty()).then(|| ChunkNode::leaf_node(left)),
                    (!right.is_empty())
                        .then(|| ChunkNode::leaf_node(right.into_iter().collect())),
                )
            }
            ChunkNode::Branch { children, .. } => {
                let mut left_children = Vec::new();
                let mut right_children = VecDeque::new();
                let mut cursor = abs_start;
                let mut divided = false;
                for child in children {
                    if divided {
                        right_children.push_back(Arc::clone(child));
                        continue;
                    }
                    let child_end = cursor.saturating_add(child.bytes());
                    if offset <= cursor {
                        divided = true;
                        right_children.push_back(Arc::clone(child));
                    } else if offset >= child_end {
                        left_children.push(Arc::clone(child));
                    } else {
                        divided = true;
                        let (left, right) = Self::split_node(child, cursor, offset);
                        if let Some(left) = left {
                            left_children.push(left);
                        }
                        if let Some(right) = right {
                            right_children.push_back(right);
                        }
                    }
                    cursor = child_end;
                }
                (
                    (!left_children.is_empty()).then(|| ChunkNode::branch_node(left_children)),
                    (!right_children.is_empty()).then(|| {
                        ChunkNode::branch_node(right_children.into_iter().collect())
                    }),
                )
            }
        }
    }

    /// Locates the descriptor containing an absolute offset plus the
    /// page-local byte index. Returns `None` outside the tree range.
    pub(crate) fn locate(&self, base: u64, offset: u64) -> Option<(&[u8], usize)> {
        let root = self.root.as_ref()?;
        if offset < base || offset >= base.saturating_add(root.bytes()) {
            return None;
        }
        Self::locate_node(root, base, offset).map(|desc| {
            let local = (offset - desc.abs_start) as usize;
            (desc.bytes().as_bytes(), local)
        })
    }

    pub(crate) fn locate_node<'a>(
        node: &'a Arc<ChunkNode>,
        abs_start: u64,
        offset: u64,
    ) -> Option<&'a ChunkDesc> {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => descs
                .iter()
                .find(|desc| offset >= desc.abs_start && offset < desc.end()),
            ChunkNode::Branch { children, .. } => {
                let mut cursor = abs_start;
                for child in children {
                    let end = cursor.saturating_add(child.bytes());
                    if offset >= cursor && offset < end {
                        return Self::locate_node(child, cursor, offset);
                    }
                    cursor = end;
                }
                None
            }
        }
    }

    /// Byte at an absolute offset, for boundary classification.
    pub(crate) fn byte_at(&self, base: u64, offset: u64) -> Option<u8> {
        self.locate(base, offset)
            .and_then(|(bytes, local)| bytes.get(local).copied())
    }

    /// Entry `0` is the range base; entry `i > 0` follows the `(i-1)`-th
    /// newline. Returns `None` past the last entry.
    pub(crate) fn line_entry(&self, base: u64, index: u64) -> Option<u64> {
        if index == 0 {
            return Some(base);
        }
        let root = self.root.as_ref()?;
        if index > root.newlines() {
            return None;
        }
        Self::nth_newline_end(root, index - 1)
    }

    /// Absolute offset just after the `n`-th newline (0-based) in the tree.
    fn nth_newline_end(node: &Arc<ChunkNode>, mut n: u64) -> Option<u64> {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => {
                for desc in descs {
                    if n < desc.newlines() {
                        let off = desc.line_offsets[n as usize];
                        return desc.abs_start.checked_add(u64::from(off));
                    }
                    n -= desc.newlines();
                }
                None
            }
            ChunkNode::Branch { children, .. } => {
                for child in children {
                    if n < child.newlines() {
                        return Self::nth_newline_end(child, n);
                    }
                    n -= child.newlines();
                }
                None
            }
        }
    }

    /// Smallest line entry at or after `target`, or `None` when the target
    /// is past the last line start (callers fall back to a UTF-8 boundary).
    /// A view start counts exactly when it opens a line: the range base, or
    /// a previous byte that is a newline.
    pub(crate) fn first_line_at_or_after(&self, base: u64, target: u64) -> Option<u64> {
        if target <= base {
            return Some(base);
        }
        let root = self.root.as_ref()?;
        self.first_line_from(root, base, base, target)
    }

    fn first_line_from(
        &self,
        node: &Arc<ChunkNode>,
        abs_start: u64,
        base: u64,
        target: u64,
    ) -> Option<u64> {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => {
                // Whether the running cursor opens a line. Only the range
                // base is known here; later starts consult the previous
                // byte on demand (at most once per leaf scan).
                let mut prev_opens_line = false;
                let mut cursor = abs_start;
                for desc in descs {
                    debug_assert_eq!(desc.abs_start, cursor);
                    if desc.abs_start >= target {
                        if desc.abs_start == base {
                            return Some(base);
                        }
                        if prev_opens_line {
                            return Some(desc.abs_start);
                        }
                        prev_opens_line = self.byte_at(base, desc.abs_start.saturating_sub(1))
                            == Some(b'\n');
                        if prev_opens_line {
                            return Some(desc.abs_start);
                        }
                    }
                    if target <= desc.end() {
                        let position = desc.line_offsets.partition_point(|off| {
                            desc.abs_start.saturating_add(u64::from(*off)) < target
                        });
                        if let Some(off) = desc.line_offsets.get(position) {
                            return desc.abs_start.checked_add(u64::from(*off));
                        }
                    }
                    prev_opens_line = desc.bytes().as_bytes().last() == Some(&b'\n');
                    cursor = desc.end();
                }
                None
            }
            ChunkNode::Branch { children, .. } => {
                let mut cursor = abs_start;
                for child in children {
                    let end = cursor.saturating_add(child.bytes());
                    if target <= end
                        && let Some(found) = self.first_line_from(child, cursor, base, target)
                    {
                        return Some(found);
                    }
                    cursor = end;
                }
                None
            }
        }
    }

    /// Absolute offset of the last line start in the range: the position
    /// after the final newline, or `base` when there is none.
    pub(crate) fn last_line_start(&self, base: u64) -> u64 {
        let Some(root) = self.root.as_ref() else {
            return base;
        };
        Self::last_line_node(root).unwrap_or(base)
    }

    fn last_line_node(node: &Arc<ChunkNode>) -> Option<u64> {
        match node.as_ref() {
            ChunkNode::Leaf { descs, .. } => {
                for desc in descs.iter().rev() {
                    if let Some(off) = desc.line_offsets.last() {
                        return desc.abs_start.checked_add(u64::from(*off));
                    }
                }
                None
            }
            ChunkNode::Branch { children, .. } => {
                for child in children.iter().rev() {
                    if let Some(found) = Self::last_line_node(child) {
                        return Some(found);
                    }
                }
                None
            }
        }
    }

    /// Ordered descriptor views, for text materialization and projection.
    /// Collects leaf slices first (borrowed, no data copies).
    pub(crate) fn iter_descs(&self) -> impl Iterator<Item = &ChunkDesc> {
        let mut runs = Vec::new();
        if let Some(root) = self.root.as_ref() {
            Self::collect_leaves(root, &mut runs);
        }
        runs.into_iter().flat_map(|descs| descs.iter())
    }

    fn collect_leaves<'a>(node: &'a ChunkNode, out: &mut Vec<&'a [ChunkDesc]>) {
        match node {
            ChunkNode::Leaf { descs, .. } => {
                if !descs.is_empty() {
                    out.push(descs);
                }
            }
            ChunkNode::Branch { children, .. } => {
                for child in children {
                    Self::collect_leaves(child, out);
                }
            }
        }
    }
}

/// One retained annotation with its acceptance sequence number. The `seqno`
/// preserves the original semantic application order across indexed
/// storage: overlaps apply in acceptance order no matter how the index
/// orders them for lookup.
#[derive(Clone, Debug)]
pub(crate) struct SourceAnnotation {
    pub(crate) kind: u32,
    pub(crate) flags: u32,
    pub(crate) start_byte: u64,
    pub(crate) end_byte: u64,
    pub(crate) payload: Arc<[u8]>,
    pub(crate) aux0: u32,
    pub(crate) aux1: u32,
    pub(crate) tag: Option<SemanticTag>,
    pub(crate) style: Option<StyleRef>,
    pub(crate) seqno: u64,
}

/// Deterministic treap priority from the index key (no RNG needed).
fn annotation_priority(start: u64, seqno: u64) -> u64 {
    let mut key = start.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(seqno);
    key ^= key >> 30;
    key = key.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    key ^= key >> 27;
    key = key.wrapping_mul(0x94D0_49BB_1331_11EB);
    key ^ (key >> 31)
}

#[derive(Clone, Debug)]
struct AnnotationNode {
    key: (u64, u64),
    value: SourceAnnotation,
    priority: u64,
    max_end: u64,
    size: usize,
    left: Option<Arc<AnnotationNode>>,
    right: Option<Arc<AnnotationNode>>,
}

impl AnnotationNode {
    fn new(start: u64, seqno: u64, value: SourceAnnotation) -> Arc<Self> {
        let max_end = value.end_byte;
        Arc::new(Self {
            key: (start, seqno),
            value,
            priority: annotation_priority(start, seqno),
            max_end,
            size: 1,
            left: None,
            right: None,
        })
    }

    fn rebuilt(
        key: (u64, u64),
        value: SourceAnnotation,
        priority: u64,
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Arc<Self> {
        let mut max_end = value.end_byte;
        let mut size: usize = 1;
        if let Some(left) = &left {
            max_end = max_end.max(left.max_end);
            size = size.saturating_add(left.size);
        }
        if let Some(right) = &right {
            max_end = max_end.max(right.max_end);
            size = size.saturating_add(right.size);
        }
        Arc::new(Self {
            key,
            value,
            priority,
            max_end,
            size,
            left,
            right,
        })
    }
}

/// Persistent annotation index keyed by `(start, seqno)`. Inserts, splits,
/// and merges copy only the affected path; overlap queries prune by
/// `max_end` and stop at `end`, so a text run never scans every annotation.
#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationTree {
    root: Option<Arc<AnnotationNode>>,
    count: usize,
}

impl AnnotationTree {
    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.count
    }

    pub(crate) fn insert(&self, value: SourceAnnotation) -> Self {
        let key = (value.start_byte, value.seqno);
        let (left, right) = Self::split_key(&self.root, key);
        let node = AnnotationNode::new(key.0, key.1, value);
        Self {
            root: Self::merge(
                Self::merge(left, Some(node)),
                right,
            ),
            count: self.count.saturating_add(1),
        }
    }

    /// Builds an index from acceptance-ordered records: sorts by
    /// `(start, seqno)` and folds through the tested insert path, so the
    /// result always satisfies both key and heap order. Append batches are
    /// tiny (usually empty); even a full batch stays cheap.
    pub(crate) fn from_sorted_batch(mut values: Vec<SourceAnnotation>) -> Self {
        values.sort_by(|left, right| {
            (left.start_byte, left.seqno).cmp(&(right.start_byte, right.seqno))
        });
        let mut tree = Self::default();
        for value in values {
            tree = tree.insert(value);
        }
        tree
    }

    /// Merges two indexes where every key in `left` is below every key in
    /// `right` (append batches satisfy this: new starts are at or after the
    /// retained end, and sequence numbers only grow).
    pub(crate) fn merge_append(&self, batch: &Self) -> Self {
        Self {
            root: Self::merge(self.root.clone(), batch.root.clone()),
            count: self.count.saturating_add(batch.count),
        }
    }

    fn split_key(
        root: &Option<Arc<AnnotationNode>>,
        key: (u64, u64),
    ) -> (Option<Arc<AnnotationNode>>, Option<Arc<AnnotationNode>>) {
        let Some(node) = root.clone() else {
            return (None, None);
        };
        if node.key < key {
            let (middle, right) = Self::split_key(&node.right, key);
            (
                Some(AnnotationNode::rebuilt(
                    node.key,
                    node.value.clone(),
                    node.priority,
                    node.left.clone(),
                    middle,
                )),
                right,
            )
        } else {
            let (left, middle) = Self::split_key(&node.left, key);
            (
                left,
                Some(AnnotationNode::rebuilt(
                    node.key,
                    node.value.clone(),
                    node.priority,
                    middle,
                    node.right.clone(),
                )),
            )
        }
    }

    fn merge(
        left: Option<Arc<AnnotationNode>>,
        right: Option<Arc<AnnotationNode>>,
    ) -> Option<Arc<AnnotationNode>> {
        match (left, right) {
            (None, right) => right,
            (left, None) => left,
            (Some(left), Some(right)) => {
                if left.priority < right.priority {
                    let merged = Self::merge(left.right.clone(), Some(right));
                    Some(AnnotationNode::rebuilt(
                        left.key,
                        left.value.clone(),
                        left.priority,
                        left.left.clone(),
                        merged,
                    ))
                } else {
                    let merged = Self::merge(Some(left), right.left.clone());
                    Some(AnnotationNode::rebuilt(
                        right.key,
                        right.value.clone(),
                        right.priority,
                        merged,
                        right.right.clone(),
                    ))
                }
            }
        }
    }

    /// Head truncation: splits at `offset` and applies the stored-record
    /// truncation policy. Clip records straddling the offset restart at it
    /// (keeping their sequence numbers, so application order is preserved);
    /// Drop records need a live span past the offset; Point records need a
    /// start at or after it. Boundary work touches only survivors plus the
    /// split path; dropped subtrees are simply unreferenced.
    pub(crate) fn truncate_head(&self, offset: u64) -> Self {
        let (left, right) = Self::split_key(&self.root, (offset, 0));
        let mut survivors = Vec::new();
        Self::collect_clipped(&left, offset, &mut survivors);
        survivors.sort_by_key(|value| value.seqno);
        let mut count = Self::subtree_count(&right);
        let mut kept = right;
        for survivor in survivors {
            let key = (survivor.start_byte, survivor.seqno);
            let (before, after) = Self::split_key(&kept, key);
            kept = Self::merge(
                Self::merge(before, Some(AnnotationNode::new(key.0, key.1, survivor))),
                after,
            );
            count = count.saturating_add(1);
        }
        Self { root: kept, count }
    }

    fn subtree_count(root: &Option<Arc<AnnotationNode>>) -> usize {
        root.as_ref().map_or(0, |node| node.size)
    }

    /// Collects left-side Clip records that still cover `offset`, restarted
    /// at it. Prunes subtrees that cannot overlap.
    fn collect_clipped(
        root: &Option<Arc<AnnotationNode>>,
        offset: u64,
        out: &mut Vec<SourceAnnotation>,
    ) {
        let Some(node) = root else {
            return;
        };
        if node.max_end <= offset {
            return;
        }
        Self::collect_clipped(&node.left, offset, out);
        if (node.value.kind == CONTENT_ANNOTATION_KIND_TAG
            || node.value.kind == CONTENT_ANNOTATION_KIND_STYLE)
            && node.value.end_byte > offset
        {
            let mut clipped = node.value.clone();
            clipped.start_byte = offset;
            out.push(clipped);
        }
        Self::collect_clipped(&node.right, offset, out);
    }

    /// Indexed overlap lookup (§9.7): collects intersecting annotations in
    /// index order, pruning subtrees that end before `start` and stopping
    /// past `end`. Callers sort the (small) active set by `seqno` to recover
    /// the original semantic application order.
    pub(crate) fn overlapping<'a>(
        &'a self,
        start: u64,
        end: u64,
        out: &mut Vec<(u64, &'a SourceAnnotation)>,
    ) {
        Self::overlap_node(&self.root, start, end, out);
    }

    fn overlap_node<'a>(
        root: &'a Option<Arc<AnnotationNode>>,
        start: u64,
        end: u64,
        out: &mut Vec<(u64, &'a SourceAnnotation)>,
    ) {
        let Some(node) = root else {
            return;
        };
        if node.max_end <= start {
            return;
        }
        if node.key.0 < end {
            Self::overlap_node(&node.left, start, end, out);
            if node.value.start_byte < end && node.value.end_byte > start {
                out.push((node.value.seqno, &node.value));
            }
            Self::overlap_node(&node.right, start, end, out);
        } else {
            // Every key at or past `end` starts too late; only the left
            // subtree (earlier starts, possibly long spans) can overlap.
            Self::overlap_node(&node.left, start, end, out);
        }
    }

    /// Insertion-ordered records for diagnostics and snapshots.
    pub(crate) fn in_application_order(&self) -> Vec<SourceAnnotation> {
        let mut ordered = Vec::with_capacity(self.count);
        Self::collect_all(&self.root, &mut ordered);
        ordered.sort_by_key(|value| value.seqno);
        ordered
    }

    fn collect_all(root: &Option<Arc<AnnotationNode>>, out: &mut Vec<SourceAnnotation>) {
        let Some(node) = root else {
            return;
        };
        Self::collect_all(&node.left, out);
        out.push(node.value.clone());
        Self::collect_all(&node.right, out);
    }
}

/// One validated annotation ready for storage. Validation (range policy,
/// payload caps, generation checks) happens before this value exists, so
/// every `apply_*` entry point below is total over its inputs except for
/// the errors it documents.
#[derive(Clone, Debug)]
pub(crate) struct ValidatedAnnotation {
    pub(crate) kind: u32,
    pub(crate) flags: u32,
    pub(crate) start_byte: u64,
    pub(crate) end_byte: u64,
    pub(crate) payload: Vec<u8>,
    pub(crate) aux0: u32,
    pub(crate) aux1: u32,
    pub(crate) tag: Option<SemanticTag>,
    pub(crate) style: Option<StyleRef>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ContentError {
    Sealed,
    AlreadySealed,
    InvalidByteRange { start: u64, end: u64, len: u64 },
    LengthOverflow,
    RetentionOverflow,
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sealed => write!(f, "SOURCE_SEALED: Source is sealed"),
            Self::AlreadySealed => write!(f, "SOURCE_ALREADY_SEALED: Source is already sealed"),
            Self::InvalidByteRange { .. } => {
                write!(f, "INVALID_RANGE: Source head is outside the retained range")
            }
            Self::LengthOverflow => write!(f, "INVALID_RANGE: Source coordinate exhausted"),
            Self::RetentionOverflow => {
                write!(f, "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded")
            }
        }
    }
}

impl std::error::Error for ContentError {}

/// Persistent source storage: a chunk tree for bytes plus an annotation
/// index, with the accepted revision, end offset, seal state, and next
/// annotation sequence number. Every mutating entry point takes `&self`
/// and returns the next storage value, so a failed validation can never
/// leave partial bytes behind (§9.6): callers only swap the stored value
/// on success.
#[derive(Clone, Debug, Default)]
pub(crate) struct StoredSource {
    pub(crate) chunks: ChunkTree,
    pub(crate) annotations: AnnotationTree,
    pub(crate) ordered_annotations: Arc<[SourceAnnotation]>,
    pub(crate) base: u64,
    pub(crate) end: u64,
    pub(crate) head_partial: bool,
    pub(crate) revision: u64,
    pub(crate) next_seqno: u64,
    pub(crate) sealed: bool,
    pub(crate) sealed_at: Option<u64>,
}

impl StoredSource {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.base == 0 && self.end == 0 && self.revision == 0 && self.annotations.is_empty()
    }

    pub(crate) fn base(&self) -> u64 {
        self.base
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    pub(crate) fn head_partial(&self) -> bool {
        self.head_partial
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn byte_len(&self, base: u64) -> u64 {
        self.end.saturating_sub(base)
    }

    pub(crate) fn retained_bytes(&self) -> u64 {
        self.end.saturating_sub(self.base)
    }

    pub(crate) fn sealed(&self) -> bool {
        self.sealed
    }

    pub(crate) fn sealed_at(&self) -> Option<u64> {
        self.sealed_at
    }

    pub(crate) fn annotation_count(&self) -> usize {
        self.annotations.len()
    }

    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.leaf_count()
    }

    pub(crate) fn line_count(&self) -> usize {
        self.chunks.line_entries() as usize
    }

    /// Appends validated text plus its validated annotations. Rejects sealed
    /// sources exactly like the previous `String` storage did.
    pub(crate) fn apply_append(
        &self,
        text: &str,
        revision: u64,
        annos: Vec<ValidatedAnnotation>,
    ) -> Result<Self, ContentError> {
        if self.sealed {
            return Err(ContentError::Sealed);
        }
        let mut next = self.clone();
        next.revision = revision;
        if !text.is_empty() {
            next.chunks = next.chunks.appended(text, next.end);
            next.end = next
                .end
                .checked_add(text.len() as u64)
                .ok_or(ContentError::LengthOverflow)?;
        }
        if !annos.is_empty() {
            let mut records = Vec::with_capacity(annos.len());
            for anno in annos {
                let seqno = next.next_seqno;
                next.next_seqno = next.next_seqno.saturating_add(1);
                records.push(SourceAnnotation {
                    kind: anno.kind,
                    flags: anno.flags,
                    start_byte: anno.start_byte,
                    end_byte: anno.end_byte,
                    payload: Arc::from(anno.payload.into_boxed_slice()),
                    aux0: anno.aux0,
                    aux1: anno.aux1,
                    tag: anno.tag,
                    style: anno.style,
                    seqno,
                });
            }
            let batch = AnnotationTree::from_sorted_batch(records);
            next.annotations = next.annotations.merge_append(&batch);
        }
        next.ordered_annotations = Arc::from(next.annotations.in_application_order());
        Ok(next)
    }

    /// Seals the source at `at`. Range errors precede the sealed check, as
    /// before; the atomic-marker annotation (when present) is stored last
    /// so a concurrent snapshot never sees a marker without its seal.
    pub(crate) fn apply_seal(
        &self,
        base: u64,
        at: u64,
        revision: u64,
        marker: Option<ValidatedAnnotation>,
    ) -> Result<Self, ContentError> {
        if at < base || at > self.end {
            return Err(ContentError::InvalidByteRange {
                start: at,
                end: at,
                len: self.byte_len(base),
            });
        }
        if self.sealed {
            return Err(ContentError::AlreadySealed);
        }
        let mut next = self.clone();
        next.revision = revision;
        next.sealed = true;
        next.sealed_at = Some(at);
        if let Some(anno) = marker {
            let seqno = next.next_seqno;
            next.next_seqno = next.next_seqno.saturating_add(1);
            next.annotations = next.annotations.insert(SourceAnnotation {
                kind: anno.kind,
                flags: anno.flags,
                start_byte: anno.start_byte,
                end_byte: anno.end_byte,
                payload: Arc::from(anno.payload.into_boxed_slice()),
                aux0: anno.aux0,
                aux1: anno.aux1,
                tag: anno.tag,
                style: anno.style,
                seqno,
            });
        }
        next.ordered_annotations = Arc::from(next.annotations.in_application_order());
        Ok(next)
    }

    /// Records one validated annotation, then enforces the retention caps
    /// with the exact predicate the previous `Vec` storage used: a record
    /// survives when it overlaps the retained byte window, or when it is
    /// zero-length at or after the window floor; over-count survivors drop
    /// oldest (lowest sequence number) first.
    pub(crate) fn apply_annotation(
        &self,
        anno: ValidatedAnnotation,
        revision: u64,
        max_annotations: usize,
        max_retained_bytes: u64,
    ) -> Result<Self, ContentError> {
        if self.sealed {
            return Err(ContentError::Sealed);
        }
        let mut next = self.clone();
        next.revision = revision;
        let seqno = next.next_seqno;
        next.next_seqno = next.next_seqno.saturating_add(1);
        next.annotations = next.annotations.insert(SourceAnnotation {
            kind: anno.kind,
            flags: anno.flags,
            start_byte: anno.start_byte,
            end_byte: anno.end_byte,
            payload: Arc::from(anno.payload.into_boxed_slice()),
            aux0: anno.aux0,
            aux1: anno.aux1,
            tag: anno.tag,
            style: anno.style,
            seqno,
        });
        next = next.enforce_retention(max_annotations, max_retained_bytes);
        next.ordered_annotations = Arc::from(next.annotations.in_application_order());
        Ok(next)
    }

    fn enforce_retention(&self, max_annotations: usize, max_retained_bytes: u64) -> Self {
        if self.annotations.len() <= max_annotations {
            return self.clone();
        }
        let floor = self.end.saturating_sub(max_retained_bytes);
        let mut ordered = self.annotations.in_application_order();
        ordered.retain(|record| {
            record.end_byte > floor
                || (record.start_byte == record.end_byte && record.start_byte >= floor)
        });
        if ordered.len() > max_annotations {
            let drop = ordered.len() - max_annotations;
            ordered.drain(..drop);
        }
        let ordered_annotations = Arc::from(ordered.as_slice());
        Self {
            chunks: self.chunks.clone(),
            annotations: AnnotationTree::from_sorted_batch(ordered),
            ordered_annotations,
            base: self.base,
            end: self.end,
            head_partial: self.head_partial,
            revision: self.revision,
            next_seqno: self.next_seqno,
            sealed: self.sealed,
            sealed_at: self.sealed_at,
        }
    }

    /// Truncates the head at `offset`. Offsets past the end are rejected
    /// before anything moves; offsets at or before the base are a no-op
    /// reporting zero dropped bytes, as before.
    pub(crate) fn apply_truncate(
        &self,
        base: u64,
        offset: u64,
        revision: u64,
    ) -> Result<(Self, u64), ContentError> {
        if offset > self.end {
            return Err(ContentError::InvalidByteRange {
                start: offset,
                end: offset,
                len: self.byte_len(base),
            });
        }
        if offset <= base {
            return Ok((self.clone(), 0));
        }
        let (chunks, dropped) = self.chunks.truncated_head(base, offset);
        let annotations = self.annotations.truncate_head(offset);
        let partial = offset < self.end
            && (offset != 0
                && self.chunks.byte_at(base, offset.saturating_sub(1)) != Some(b'\n'));
        let ordered_annotations = Arc::from(annotations.in_application_order());
        Ok((
            Self {
                chunks,
                annotations,
                ordered_annotations,
                base: offset,
                end: self.end,
                head_partial: partial,
                revision,
                next_seqno: self.next_seqno,
                sealed: self.sealed,
                sealed_at: self.sealed_at,
            },
            dropped,
        ))
    }

    pub(crate) fn locate(&self, base: u64, offset: u64) -> Option<(&[u8], usize)> {
        self.chunks.locate(base, offset)
    }

    pub(crate) fn byte_at(&self, base: u64, offset: u64) -> Option<u8> {
        self.chunks.byte_at(base, offset)
    }

    pub(crate) fn line_entry(&self, base: u64, index: u64) -> Option<u64> {
        self.chunks.line_entry(base, index)
    }

    pub(crate) fn first_line_at_or_after(&self, base: u64, target: u64) -> Option<u64> {
        self.chunks.first_line_at_or_after(base, target)
    }

    pub(crate) fn last_line_start(&self, base: u64) -> u64 {
        self.chunks.last_line_start(base)
    }

    /// Collects absolute line-start entries in order, without copying text:
    /// walks descriptors and shifts each view's precomputed newline offsets
    /// by its absolute start.
    pub(crate) fn collect_line_entries(&self, base: u64, out: &mut Vec<u64>) {
        out.clear();
        out.push(base);
        for desc in self.chunks.iter_descs() {
            for off in desc.line_offsets.iter() {
                if let Some(entry) = desc.abs_start.checked_add(u64::from(*off)) {
                    out.push(entry);
                }
            }
        }
    }

    /// Materializes the full text (snapshots and diagnostics only).
    pub(crate) fn collect_text(&self, out: &mut Vec<u8>) {
        out.clear();
        for desc in self.chunks.iter_descs() {
            out.extend_from_slice(desc.bytes().as_bytes());
        }
    }

    /// Iterator over chunk bytes and their absolute start offsets.
    pub(crate) fn iter_chunks(&self) -> impl Iterator<Item = (&[u8], u64)> {
        self.chunks
            .iter_descs()
            .map(|desc| (desc.bytes().as_bytes(), desc.abs_start))
    }

    /// Materializes text as a String.
    pub(crate) fn text(&self) -> String {
        let mut out = String::with_capacity(usize::try_from(self.retained_bytes()).unwrap_or(0));
        for desc in self.chunks.iter_descs() {
            out.push_str(desc.bytes());
        }
        out
    }

    /// Copies `[start, end)` into `out` without materializing the whole
    /// text: walks only the descriptors overlapping the range.
    pub(crate) fn text_in(&self, base: u64, start: u64, end: u64, out: &mut Vec<u8>) {
        out.clear();
        let from = start.max(base).min(self.end);
        let upto = end.max(from).min(self.end);
        if upto <= from {
            return;
        }
        for desc in self.chunks.iter_descs() {
            let desc_end = desc.end();
            if desc_end <= from || desc.abs_start >= upto {
                continue;
            }
            let bytes = desc.bytes().as_bytes();
            let skip = from.saturating_sub(desc.abs_start) as usize;
            let take = (upto.saturating_sub(desc.abs_start) as usize).min(bytes.len());
            if take > skip {
                out.extend_from_slice(&bytes[skip..take]);
            }
        }
    }

    /// Previous UTF-8 character boundary at or before `offset`.
    pub(crate) fn floor_char_boundary(&self, base: u64, offset: u64) -> u64 {
        let mut at = offset.min(self.end);
        for _ in 0..4 {
            if at <= base {
                return base;
            }
            if at >= self.end {
                return self.end;
            }
            match self.byte_at(base, at) {
                // A character starts here unless this byte continues one.
                Some(byte) if byte >> 6 != 0b10 => return at,
                _ => at = at.saturating_sub(1),
            }
        }
        at.max(base)
    }

    /// Next UTF-8 character boundary at or after `offset`.
    pub(crate) fn ceil_char_boundary(&self, base: u64, offset: u64) -> u64 {
        let mut at = offset.max(base).min(self.end);
        for _ in 0..4 {
            if at >= self.end {
                return self.end;
            }
            match self.byte_at(base, at) {
                Some(byte) if byte >> 6 != 0b10 => return at,
                _ => at = at.saturating_add(1),
            }
        }
        at.min(self.end)
    }

    pub(crate) fn is_boundary(&self, offset: u64) -> bool {
        if offset == self.base || offset == self.end {
            return true;
        }
        if offset < self.base || offset > self.end {
            return false;
        }
        let Some(root) = self.chunks.root.as_ref() else {
            return false;
        };
        ChunkTree::locate_node(root, self.base, offset).is_some_and(|desc| {
            let local = (offset - desc.abs_start) as usize;
            desc.bytes().is_char_boundary(local)
        })
    }

    pub(crate) fn next_boundary(&self, offset: u64) -> u64 {
        if offset <= self.base {
            return self.base;
        }
        if offset >= self.end {
            return self.end;
        }
        if self.is_boundary(offset) {
            return offset;
        }
        let Some(root) = self.chunks.root.as_ref() else {
            return self.end;
        };
        if let Some(desc) = ChunkTree::locate_node(root, self.base, offset) {
            let local = (offset - desc.abs_start) as usize;
            let text = desc.bytes();
            if let Some(next) = text
                .char_indices()
                .map(|(idx, _)| idx)
                .find(|idx| *idx > local)
            {
                return desc.abs_start.saturating_add(next as u64);
            }
        }
        self.end
    }

    pub(crate) fn offset_for_max_bytes(&self, max_bytes: u64) -> u64 {
        let target = self.end.saturating_sub(max_bytes);
        self.first_line_at_or_after(self.base, target)
            .unwrap_or_else(|| self.next_boundary(target))
    }

    pub(crate) fn stable_prefix(&self) -> Option<Self> {
        let end = self.last_line_start(self.base);
        if end <= self.base || end > self.end {
            return None;
        }
        let (chunks, _) = self.chunks.split_at(self.base, end);
        let mut kept_annos = Vec::new();
        for anno in self.ordered_annotations.iter() {
            if anno.start_byte >= end {
                continue;
            }
            let mut anno = anno.clone();
            if anno.end_byte > end {
                anno.end_byte = end;
            }
            if anno.kind == CONTENT_ANNOTATION_KIND_POINT || anno.start_byte < anno.end_byte {
                kept_annos.push(anno);
            }
        }
        let annotations = AnnotationTree::from_sorted_batch(kept_annos.clone());
        Some(Self {
            chunks,
            annotations,
            ordered_annotations: Arc::from(kept_annos),
            base: self.base,
            end,
            head_partial: false,
            revision: self.revision,
            next_seqno: self.next_seqno,
            sealed: true,
            sealed_at: Some(end),
        })
    }

    /// Indexed overlap lookup (§9.7): intersecting annotations ordered by
    /// acceptance sequence, so overlaps apply in the original semantic
    /// order no matter how the index lays them out.
    pub(crate) fn overlapping(&self, start: u64, end: u64) -> Vec<StoredOverlap<'_>> {
        let mut found = Vec::new();
        self.annotations.overlapping(start, end, &mut found);
        found.sort_by_key(|(seqno, _)| *seqno);
        found
            .into_iter()
            .map(|(_, record)| StoredOverlap { record })
            .collect()
    }

    /// All records in acceptance order, for diagnostics and snapshots.
    pub(crate) fn annotations_in_order(&self) -> &[SourceAnnotation] {
        &self.ordered_annotations
    }

    /// Ordered chunk views with their backing pages, for the zero-copy FFI
    /// reader: descriptors sharing a page keep the page alive by clone.
    pub(crate) fn chunk_views(&self) -> Vec<ChunkView> {
        self.chunks
            .iter_descs()
            .map(|desc| ChunkView {
                page: desc.page.clone(),
                page_start: desc.page_start,
                len: desc.len,
                abs_start: desc.abs_start,
            })
            .collect()
    }
}

/// One annotation intersecting a lookup range.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StoredOverlap<'a> {
    record: &'a SourceAnnotation,
}

impl<'a> StoredOverlap<'a> {
    pub(crate) fn record(&self) -> &'a SourceAnnotation {
        self.record
    }

    pub(crate) fn kind(&self) -> u32 {
        self.record.kind
    }

    pub(crate) fn flags(&self) -> u32 {
        self.record.flags
    }

    pub(crate) fn start(&self) -> u64 {
        self.record.start_byte
    }

    pub(crate) fn end(&self) -> u64 {
        self.record.end_byte
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.record.payload
    }

    pub(crate) fn aux0(&self) -> u32 {
        self.record.aux0
    }

    pub(crate) fn aux1(&self) -> u32 {
        self.record.aux1
    }

    pub(crate) fn tag(&self) -> Option<SemanticTag> {
        self.record.tag.clone()
    }

    pub(crate) fn style(&self) -> Option<StyleRef> {
        self.record.style.clone()
    }
}

/// One chunk view: absolute span plus its slice of a shared page.
#[derive(Clone, Debug)]
pub(crate) struct ChunkView {
    pub(crate) page: Arc<str>,
    pub(crate) page_start: u32,
    pub(crate) len: u32,
    pub(crate) abs_start: u64,
}

impl ChunkView {
    pub(crate) fn abs_end(&self) -> u64 {
        self.abs_start.saturating_add(u64::from(self.len))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        let start = self.page_start as usize;
        self.page.as_bytes().get(start..start + self.len as usize).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anno(start: u64, end: u64) -> ValidatedAnnotation {
        ValidatedAnnotation {
            kind: CONTENT_ANNOTATION_KIND_TAG,
            flags: 0,
            start_byte: start,
            end_byte: end,
            payload: Vec::new(),
            aux0: 0,
            aux1: 0,
            tag: None,
            style: None,
        }
    }

    fn point(start: u64) -> ValidatedAnnotation {
        ValidatedAnnotation {
            kind: CONTENT_ANNOTATION_KIND_POINT,
            start_byte: start,
            end_byte: start,
            ..anno(start, start)
        }
    }

    fn style_anno(start: u64, end: u64) -> ValidatedAnnotation {
        ValidatedAnnotation {
            kind: CONTENT_ANNOTATION_KIND_STYLE,
            ..anno(start, end)
        }
    }

    fn stored_text(store: &StoredSource) -> Vec<u8> {
        let mut out = Vec::new();
        store.collect_text(&mut out);
        out
    }

    fn stored_entries(store: &StoredSource, base: u64) -> Vec<u64> {
        let mut entries = Vec::new();
        store.collect_line_entries(base, &mut entries);
        entries
    }

    #[test]
    fn empty_tree_reports_base_line() {
        let tree = ChunkTree::empty();
        assert_eq!(tree.bytes(), 0);
        assert_eq!(tree.line_entry(100, 0), Some(100));
        assert_eq!(tree.line_entry(100, 1), None);
        assert_eq!(tree.iter_descs().count(), 0);
        assert_eq!(tree.first_line_at_or_after(100, 500), None);
        assert_eq!(tree.last_line_start(100), 100);
    }

    #[test]
    fn single_chunk_roundtrip() {
        let tree = ChunkTree::empty().appended("hello\nworld", 0);
        assert_eq!(tree.bytes(), 11);
        let mut out = Vec::new();
        for desc in tree.iter_descs() {
            out.extend_from_slice(desc.bytes().as_bytes());
        }
        assert_eq!(out, b"hello\nworld");
        assert_eq!(tree.line_entry(0, 0), Some(0));
        assert_eq!(tree.line_entry(0, 1), Some(6));
        assert_eq!(tree.line_entry(0, 2), None);
        assert_eq!(tree.locate(0, 6), Some(("hello\nworld".as_bytes(), 6)));
        assert_eq!(tree.byte_at(0, 5), Some(b'\n'));
        assert_eq!(tree.byte_at(0, 11), None);
    }

    #[test]
    fn large_append_spans_pages_and_branches() {
        let mut text = String::new();
        for i in 0..5000 {
            text.push_str(&format!("line {i:05}\n"));
        }
        let tree = ChunkTree::empty().appended(&text, 1000);
        assert_eq!(tree.bytes(), text.len() as u64);
        let mut out = Vec::new();
        for desc in tree.iter_descs() {
            out.extend_from_slice(desc.bytes().as_bytes());
        }
        assert_eq!(out, text.as_bytes());
        // Spot-check absolute starts across page seams.
        let mut cursor = 1000u64;
        for desc in tree.iter_descs() {
            assert_eq!(desc.abs_start, cursor);
            cursor += u64::from(desc.len);
        }
        assert_eq!(cursor, 1000 + text.len() as u64);
        // Every line entry resolves through the tree.
        let newlines = text.bytes().filter(|b| *b == b'\n').count() as u64;
        assert_eq!(tree.line_entry(1000, newlines), Some(1000 + text.len() as u64));
        assert_eq!(tree.line_entry(1000, newlines + 1), None);
        // Seams land inside lines without duplicating entries.
        let mut entries = Vec::new();
        let store = StoredSource {
            chunks: tree,
            annotations: AnnotationTree::default(),
            ordered_annotations: Arc::from([]),
            base: 1000,
            end: 1000 + text.len() as u64,
            head_partial: false,
            revision: 1,
            next_seqno: 0,
            sealed: false,
            sealed_at: None,
        };
        store.collect_line_entries(1000, &mut entries);
        assert_eq!(entries.len() as u64, newlines + 1);
        assert!(entries.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn truncate_then_append_keeps_absolute_coordinates() {
        let base = 0u64;
        let store = StoredSource::empty()
            .apply_append("hello world", 1, Vec::new())
            .unwrap();
        let (next, dropped) = store.apply_truncate(base, 6, 2).unwrap();
        assert_eq!(dropped, 6);
        assert_eq!(stored_text(&next), b"world");
        assert_eq!(stored_entries(&next, 6), vec![6]);
        let next = next.apply_append("WORLD!", 3, Vec::new()).unwrap();
        assert_eq!(stored_text(&next), b"worldWORLD!");
        assert_eq!(next.end(), 17);
        // Absolute locate matches the concatenated bytes.
        for (index, byte) in b"worldWORLD!".iter().enumerate() {
            assert_eq!(next.byte_at(6, 6 + index as u64), Some(*byte));
        }
    }

    #[test]
    fn stacked_truncates_and_appends_stay_contiguous() {
        let mut store = StoredSource::empty();
        let mut base = 0u64;
        let mut model = Vec::new();
        let pieces = ["alpha\n", "beta\nbeta2\n", "gamma", "\ndelta\nepsilon\n", "zeta"];
        let mut rev = 1u64;
        for piece in pieces {
            store = store.apply_append(piece, rev, Vec::new()).unwrap();
            rev += 1;
            model.extend_from_slice(piece.as_bytes());
        }
        for cut in [3u64, 9, 14, 20] {
            let (next, dropped) = store.apply_truncate(base, cut, rev).unwrap();
            rev += 1;
            assert_eq!(dropped, cut - base);
            model.drain(..(cut - base) as usize);
            base = cut;
            store = next;
            assert_eq!(stored_text(&store), model);
            assert_eq!(store.byte_len(base), model.len() as u64);
            // Line entries restart at the new base and stay ordered.
            let entries = stored_entries(&store, base);
            assert_eq!(entries.first(), Some(&base));
            assert!(entries.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(store.line_count(), entries.len());
        }
    }

    #[test]
    fn newline_at_chunk_edge_starts_next_line() {
        // Fill the first page so it ends exactly on a newline, then append
        // the next line: the view start must count as a line entry.
        let filler = "x".repeat(SOURCE_CHUNK_BYTES - 1);
        let mut first = filler.clone();
        first.push('\n');
        assert_eq!(first.len(), SOURCE_CHUNK_BYTES);
        let store = StoredSource::empty()
            .apply_append(&first, 1, Vec::new())
            .unwrap();
        let store = store.apply_append("next", 2, Vec::new()).unwrap();
        let entries = stored_entries(&store, 0);
        assert_eq!(entries, vec![0, SOURCE_CHUNK_BYTES as u64]);
        assert_eq!(
            store.first_line_at_or_after(0, SOURCE_CHUNK_BYTES as u64),
            Some(SOURCE_CHUNK_BYTES as u64)
        );
        assert_eq!(
            store.last_line_start(0),
            SOURCE_CHUNK_BYTES as u64
        );
    }

    #[test]
    fn multibyte_char_split_across_chunks() {
        // 'é' is two bytes; size the filler so it straddles a page seam.
        let filler = "a".repeat(SOURCE_CHUNK_BYTES - 1);
        let text = format!("{filler}é\ntail");
        let store = StoredSource::empty()
            .apply_append(&text, 1, Vec::new())
            .unwrap();
        assert_eq!(stored_text(&store), text.as_bytes());
        let e_start = SOURCE_CHUNK_BYTES as u64 - 1;
        assert_eq!(store.byte_at(0, e_start), Some(0xC3));
        assert_eq!(store.byte_at(0, e_start + 1), Some(0xA9));
        // Boundary helpers step over the split character from either side.
        assert_eq!(store.floor_char_boundary(0, e_start + 1), e_start);
        assert_eq!(store.ceil_char_boundary(0, e_start + 1), e_start + 2);
        assert_eq!(store.floor_char_boundary(0, e_start), e_start);
        assert_eq!(store.ceil_char_boundary(0, e_start), e_start);
        // Partial reads never split the character.
        let mut out = Vec::new();
        store.text_in(0, e_start + 1, e_start + 2, &mut out);
        assert_eq!(out, vec![0xA9]);
    }

    #[test]
    fn annotation_overlap_applies_in_acceptance_order() {
        let store = StoredSource::empty()
            .apply_append("0123456789", 1, Vec::new())
            .unwrap();
        // Insert out of start order; overlaps must still apply by seqno.
        let store = store
            .apply_annotation(anno(6, 9), 2, 1024, u64::MAX)
            .unwrap();
        let store = store
            .apply_annotation(anno(0, 4), 3, 1024, u64::MAX)
            .unwrap();
        let store = store
            .apply_annotation(anno(2, 7), 4, 1024, u64::MAX)
            .unwrap();
        let hits = store.overlapping(0, 10);
        let spans: Vec<(u64, u64)> = hits.iter().map(|hit| (hit.start(), hit.end())).collect();
        assert_eq!(spans, vec![(6, 9), (0, 4), (2, 7)]);
        let partial = store.overlapping(5, 6);
        assert_eq!(partial.len(), 1);
        assert_eq!((partial[0].start(), partial[0].end()), (2, 7));
        assert!(store.overlapping(9, 10).is_empty());
    }

    #[test]
    fn truncate_applies_stored_record_policy() {
        let store = StoredSource::empty()
            .apply_append("0123456789abcdef", 1, Vec::new())
            .unwrap();
        let store = store.apply_annotation(anno(2, 12), 2, 1024, u64::MAX).unwrap();
        let store = store.apply_annotation(style_anno(3, 14), 3, 1024, u64::MAX).unwrap();
        let store = store.apply_annotation(point(4), 4, 1024, u64::MAX).unwrap();
        let store = store.apply_annotation(point(8), 5, 1024, u64::MAX).unwrap();
        let (next, _) = store.apply_truncate(0, 8, 6).unwrap();
        let ordered = next.annotations_in_order();
        // Clip straddler restarts at the offset with its seqno; Drop and
        // Points before the offset die; the Point at the offset survives.
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].kind, CONTENT_ANNOTATION_KIND_TAG);
        assert_eq!((ordered[0].start_byte, ordered[0].end_byte), (8, 12));
        assert_eq!(ordered[0].seqno, 0);
        assert_eq!(ordered[1].kind, CONTENT_ANNOTATION_KIND_STYLE);
        assert_eq!((ordered[1].start_byte, ordered[1].end_byte), (8, 14));
        assert_eq!(ordered[1].seqno, 1);
        assert_eq!(ordered[2].kind, CONTENT_ANNOTATION_KIND_POINT);
        assert_eq!((ordered[2].start_byte, ordered[2].end_byte), (8, 8));
        assert_eq!(ordered[2].seqno, 3);
        // Both clipped spans still overlap lookups past the offset.
        let hits = next.overlapping(8, 12);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].start(), 8);
        assert_eq!(hits[1].start(), 8);
    }

    #[test]
    fn retention_keeps_window_then_newest() {
        let mut store = StoredSource::empty()
            .apply_append("0123456789", 1, Vec::new())
            .unwrap();
        let mut rev = 2u64;
        for i in 0..6 {
            store = store
                .apply_annotation(anno(i, i + 1), rev, 4, 100)
                .unwrap();
            rev += 1;
        }
        // Six records against a cap of four: the two oldest drain first.
        let ordered = store.annotations_in_order();
        assert_eq!(ordered.len(), 4);
        let starts: Vec<u64> = ordered.iter().map(|record| record.start_byte).collect();
        assert_eq!(starts, vec![2, 3, 4, 5]);
    }

    #[test]
    fn retention_zero_length_at_floor_survives() {
        let mut store = StoredSource::empty()
            .apply_append("0123456789", 1, Vec::new())
            .unwrap();
        // Window floor sits at 10 - 4 = 6: the zero-length record exactly
        // at the floor survives while older spans drain.
        let mut rev = 2u64;
        for i in 0..2 {
            store = store
                .apply_annotation(anno(i, i + 1), rev, 3, 4)
                .unwrap();
            rev += 1;
        }
        store = store.apply_annotation(anno(6, 7), rev, 3, 4).unwrap();
        rev += 1;
        store = store.apply_annotation(anno(7, 8), rev, 3, 4).unwrap();
        rev += 1;
        store = store.apply_annotation(point(6), rev, 3, 4).unwrap();
        let ordered = store.annotations_in_order();
        assert_eq!(ordered.len(), 3);
        assert!(ordered.iter().any(|record| {
            record.kind == CONTENT_ANNOTATION_KIND_POINT && record.start_byte == 6
        }));
    }

    #[test]
    fn seal_rejects_bad_range_before_sealed_state() {
        let store = StoredSource::empty()
            .apply_append("hello", 1, Vec::new())
            .unwrap();
        let sealed = store.apply_seal(0, 5, 2, None).unwrap();
        assert!(sealed.sealed());
        assert_eq!(sealed.sealed_at(), Some(5));
        // Out-of-range still reports InvalidByteRange on a sealed source.
        assert!(matches!(
            sealed.apply_seal(0, 6, 3, None),
            Err(ContentError::InvalidByteRange { .. })
        ));
        // In-range on a sealed source reports AlreadySealed.
        assert!(matches!(
            sealed.apply_seal(0, 5, 3, None),
            Err(ContentError::AlreadySealed)
        ));
        assert!(matches!(
            sealed.apply_append("!", 3, Vec::new()),
            Err(ContentError::Sealed)
        ));
    }

    #[test]
    fn failed_append_leaves_storage_untouched() {
        let store = StoredSource {
            chunks: ChunkTree::empty(),
            annotations: AnnotationTree::default(),
            ordered_annotations: Arc::from([]),
            base: 0,
            revision: 7,
            end: u64::MAX - 1,
            head_partial: false,
            next_seqno: 0,
            sealed: false,
            sealed_at: None,
        };
        let before = store.clone();
        assert!(matches!(
            store.apply_append("ab", 8, Vec::new()),
            Err(ContentError::LengthOverflow)
        ));
        assert_eq!(before.revision(), 7);
        assert_eq!(before.end(), u64::MAX - 1);
    }

    #[test]
    fn treap_holds_invariants_under_random_ops() {
        // Deterministic xorshift: same seed, same operation stream.
        let mut rng = 0x1234_5678_9ABC_DEF1u64;
        let mut next_rand = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut store = StoredSource::empty();
        let mut base = 0u64;
        let mut rev = 1u64;
        let mut model_text: Vec<u8> = Vec::new();
        // Model records in acceptance order: (kind, start, end, seqno).
        let mut model_annos: Vec<(u32, u64, u64, u64)> = Vec::new();
        let mut seqno = 0u64;
        for _ in 0..1500 {
            match next_rand() % 4 {
                0 => {
                    // Append 0-40 bytes with newlines sprinkled in.
                    let len = (next_rand() % 41) as usize;
                    let mut text = String::new();
                    for _ in 0..len {
                        let pick = next_rand() % 8;
                        if pick == 0 {
                            text.push('\n');
                        } else {
                            text.push((b'a' + (next_rand() % 26) as u8) as char);
                        }
                    }
                    let end = store.end();
                    store = store.apply_append(&text, rev, Vec::new()).unwrap();
                    rev += 1;
                    model_text.extend_from_slice(text.as_bytes());
                    assert_eq!(stored_text(&store), model_text);
                    assert_eq!(store.end(), end + text.len() as u64);
                }
                1 => {
                    // Annotate a random span (tags, styles, points).
                    if model_text.is_empty() {
                        continue;
                    }
                    let len = model_text.len() as u64;
                    let kind = match next_rand() % 3 {
                        0 => CONTENT_ANNOTATION_KIND_TAG,
                        1 => CONTENT_ANNOTATION_KIND_STYLE,
                        _ => CONTENT_ANNOTATION_KIND_POINT,
                    };
                    let a = base + next_rand() % (len + 1);
                    let b = if kind == CONTENT_ANNOTATION_KIND_POINT {
                        a
                    } else {
                        a + next_rand() % (len + 1 - (a - base)).max(1)
                    };
                    let record = ValidatedAnnotation {
                        kind,
                        ..anno(a, b)
                    };
                    store = store
                        .apply_annotation(record, rev, 4096, u64::MAX)
                        .unwrap();
                    rev += 1;
                    model_annos.push((kind, a, b, seqno));
                    seqno += 1;
                }
                2 => {
                    // Truncate to a random valid offset.
                    if model_text.is_empty() {
                        continue;
                    }
                    let cut = base + next_rand() % (model_text.len() as u64 + 1);
                    let (next, dropped) = store.apply_truncate(base, cut, rev).unwrap();
                    rev += 1;
                    assert_eq!(dropped, cut - base);
                    model_text.drain(..(cut - base) as usize);
                    // Stored-record truncation policy, mirrored.
                    model_annos.retain(|(kind, a, b, _)| {
                        if *a >= cut {
                            return true;
                        }
                        (*kind == CONTENT_ANNOTATION_KIND_TAG || *kind == CONTENT_ANNOTATION_KIND_STYLE) && *b > cut
                    });
                    for record in model_annos.iter_mut() {
                        if record.1 < cut {
                            record.1 = cut;
                        }
                    }
                    base = cut;
                    store = next;
                    assert_eq!(stored_text(&store), model_text);
                    let entries = stored_entries(&store, base);
                    assert_eq!(entries.first(), Some(&base));
                    assert!(entries.windows(2).all(|pair| pair[0] < pair[1]));
                }
                _ => {
                    // Overlap probe: indexed lookup must match the model.
                    if model_text.is_empty() {
                        continue;
                    }
                    let len = model_text.len() as u64;
                    let a = base + next_rand() % (len + 1);
                    let b = a + next_rand() % (len + 1 - (a - base)).max(1);
                    let mut expected: Vec<(u32, u64, u64, u64)> = model_annos
                        .iter()
                        .copied()
                        .filter(|(_, s, e, _)| *s < b && *e > a)
                        .collect();
                    expected.sort_by_key(|(_, _, _, q)| *q);
                    let hits = store.overlapping(a, b);
                    assert_eq!(hits.len(), expected.len(), "overlap {a}..{b}");
                    for (hit, (_, s, e, _)) in hits.iter().zip(expected.iter()) {
                        assert_eq!((hit.start(), hit.end()), (*s, *e));
                    }
                }
            }
            check_treap(&store.annotations);
        }
        // Final agreement: text, line entries, and full annotation order.
        assert_eq!(stored_text(&store), model_text);
        let mut ordered = model_annos.clone();
        ordered.sort_by_key(|(_, _, _, q)| *q);
        let stored = store.annotations_in_order();
        assert_eq!(stored.len(), ordered.len());
        for (record, (kind, s, e, q)) in stored.iter().zip(ordered.iter()) {
            assert_eq!(record.kind, *kind);
            assert_eq!((record.start_byte, record.end_byte), (*s, *e));
            assert_eq!(record.seqno, *q);
        }
    }

    fn check_treap(tree: &AnnotationTree) {
        // Returns (min key, max key, subtree max_end, node count).
        fn visit(node: &Option<Arc<AnnotationNode>>) -> (Option<(u64, u64)>, Option<(u64, u64)>, u64, usize) {
            let Some(node) = node else {
                return (None, None, 0, 0);
            };
            let (left_min, left_max, left_max_end, left_count) = visit(&node.left);
            let (right_min, right_max, right_max_end, right_count) = visit(&node.right);
            if let Some(left_max) = left_max {
                assert!(left_max < node.key, "binary-search order");
            }
            if let Some(right_min) = right_min {
                assert!(node.key < right_min, "binary-search order");
            }
            if let Some(left) = &node.left {
                assert!(node.priority <= left.priority, "heap order");
            }
            if let Some(right) = &node.right {
                assert!(node.priority <= right.priority, "heap order");
            }
            let max_end = node.value.end_byte.max(left_max_end).max(right_max_end);
            assert_eq!(node.max_end, max_end, "max_end aggregation");
            let min = left_min.or(Some(node.key));
            let max = right_max.or(Some(node.key));
            (min, max, max_end, left_count + 1 + right_count)
        }
        let (_, _, _, count) = visit(&tree.root);
        assert_eq!(count, tree.len());
    }
}
