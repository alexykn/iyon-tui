use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use super::{
    arena::{Arena, NodeKey, ResourceKey},
    generated::{EffectMask, HostKind, RootRole},
    properties::PropertyLayers,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Links {
    pub(crate) parent: Option<NodeKey>,
    pub(crate) first_child: Option<NodeKey>,
    pub(crate) last_child: Option<NodeKey>,
    pub(crate) previous_sibling: Option<NodeKey>,
    pub(crate) next_sibling: Option<NodeKey>,
    pub(crate) child_count: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Attachments {
    pub(crate) port: Option<ResourceKey>,
    pub(crate) control: Option<ResourceKey>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Revisions {
    pub(crate) structure: u64,
    pub(crate) geometry: u64,
    pub(crate) presentation: u64,
    pub(crate) interaction: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirtyState {
    pub(crate) effects: EffectMask,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Occurrence {
    pub(crate) kind: HostKind,
    pub(crate) root_role: Option<RootRole>,
    pub(crate) root_owner: Option<NodeKey>,
    pub(crate) links: Links,
    pub(crate) properties: PropertyLayers,
    pub(crate) attachments: Attachments,
    pub(crate) subscriptions: u64,
    pub(crate) renderer_hidden: bool,
    pub(crate) style_states: std::collections::BTreeMap<String, String>,
    pub(crate) revisions: Revisions,
    pub(crate) dirty: DirtyState,
}

impl Occurrence {
    pub(crate) fn new(kind: HostKind, root_role: Option<RootRole>) -> Self {
        Self {
            kind,
            root_role,
            root_owner: None,
            links: Links::default(),
            properties: PropertyLayers::default(),
            attachments: Attachments::default(),
            subscriptions: 0,
            renderer_hidden: false,
            style_states: std::collections::BTreeMap::new(),
            revisions: Revisions::default(),
            dirty: DirtyState::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TreeError {
    Missing(NodeKey),
    Retired(NodeKey),
    WrongParent {
        child: NodeKey,
        expected: NodeKey,
        actual: Option<NodeKey>,
    },
    InvalidAnchor(NodeKey),
    Cycle,
    ProtectedRoot(NodeKey),
    ParentCannotHaveChildren(HostKind),
    InvalidAnimationChild(HostKind),
    Orphan(NodeKey),
    Capacity,
    CountOverflow(NodeKey),
    CountUnderflow(NodeKey),
    BrokenLink(NodeKey),
    LinkLoop(NodeKey),
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(key) => write!(formatter, "unknown occurrence {key:?}"),
            Self::Retired(key) => write!(formatter, "retired occurrence {key:?}"),
            Self::WrongParent {
                child,
                expected,
                actual,
            } => write!(
                formatter,
                "occurrence {child:?} is not a child of {expected:?} (actual {actual:?})"
            ),
            Self::InvalidAnchor(key) => write!(formatter, "invalid insertion anchor {key:?}"),
            Self::Cycle => formatter.write_str("insertion would create an ownership cycle"),
            Self::ProtectedRoot(key) => write!(formatter, "protected root {key:?} cannot move"),
            Self::ParentCannotHaveChildren(kind) => {
                write!(formatter, "{kind:?} cannot have ordinary children")
            }
            Self::InvalidAnimationChild(kind) => {
                write!(formatter, "animation frame must be a Box, got {kind:?}")
            }
            Self::Orphan(key) => write!(formatter, "surviving occurrence {key:?} is detached"),
            Self::Capacity => formatter.write_str("occurrence transaction capacity exhausted"),
            Self::CountOverflow(key) => write!(formatter, "child count overflow at {key:?}"),
            Self::CountUnderflow(key) => write!(formatter, "child count underflow at {key:?}"),
            Self::BrokenLink(key) => write!(formatter, "broken sibling link at {key:?}"),
            Self::LinkLoop(key) => write!(formatter, "sibling link loop at {key:?}"),
        }
    }
}

pub(crate) struct TreeDraft<'a> {
    base: &'a Arena<Occurrence>,
    portals_by_owner: &'a HashMap<NodeKey, HashSet<NodeKey>>,
    pub(crate) edits: HashMap<NodeKey, Occurrence>,
    pub(crate) created: Vec<NodeKey>,
    created_set: HashSet<NodeKey>,
    pub(crate) retired: Vec<NodeKey>,
    retired_set: HashSet<NodeKey>,
    link_touched: HashSet<NodeKey>,
    created_portals_by_owner: HashMap<NodeKey, HashSet<NodeKey>>,
}

pub(crate) struct TreePlan {
    pub(crate) edits: HashMap<NodeKey, Occurrence>,
    pub(crate) created: Vec<NodeKey>,
    pub(crate) retired: Vec<NodeKey>,
    pub(crate) retired_set: HashSet<NodeKey>,
    pub(crate) portal_buckets: HashMap<NodeKey, HashSet<NodeKey>>,
    pub(crate) portal_touched: HashSet<NodeKey>,
}

impl<'a> TreeDraft<'a> {
    pub(crate) fn new(
        base: &'a Arena<Occurrence>,
        portals_by_owner: &'a HashMap<NodeKey, HashSet<NodeKey>>,
    ) -> Self {
        Self {
            base,
            portals_by_owner,
            edits: HashMap::new(),
            created: Vec::new(),
            created_set: HashSet::new(),
            retired: Vec::new(),
            retired_set: HashSet::new(),
            link_touched: HashSet::new(),
            created_portals_by_owner: HashMap::new(),
        }
    }

    pub(crate) fn reserve(&mut self, additional: usize) -> Result<(), TreeError> {
        self.edits
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.created
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.created_set
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.retired
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.retired_set
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.link_touched
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        self.created_portals_by_owner
            .try_reserve(additional)
            .map_err(|_| TreeError::Capacity)?;
        Ok(())
    }

    pub(crate) fn insert_created(
        &mut self,
        key: NodeKey,
        occurrence: Occurrence,
    ) -> Result<(), TreeError> {
        if self.base.contains(key.slot, key.generation) || self.edits.contains_key(&key) {
            return Err(TreeError::BrokenLink(key));
        }
        self.created.push(key);
        self.created_set.insert(key);
        if occurrence.root_role == Some(RootRole::Portal) {
            if let Some(owner) = occurrence.root_owner {
                self.created_portals_by_owner
                    .entry(owner)
                    .or_default()
                    .insert(key);
            }
        }
        self.edits.insert(key, occurrence);
        self.link_touched.insert(key);
        Ok(())
    }

    pub(crate) fn read(&self, key: NodeKey) -> Result<&Occurrence, TreeError> {
        if self.retired_set.contains(&key) {
            return Err(TreeError::Retired(key));
        }
        self.edits
            .get(&key)
            .or_else(|| self.base.get(key.slot, key.generation).ok())
            .ok_or(TreeError::Missing(key))
    }

    pub(crate) fn is_retired(&self, key: NodeKey) -> bool {
        self.retired_set.contains(&key)
    }

    pub(crate) fn retirement_keys(&self, root: NodeKey) -> Result<Vec<NodeKey>, TreeError> {
        let mut keys = Vec::new();
        let mut stack = vec![root];
        let mut visited = HashSet::new();
        while let Some(key) = stack.pop() {
            if !visited.insert(key) {
                return Err(TreeError::LinkLoop(key));
            }
            let record = self.read_any(key)?;
            keys.push(key);
            let mut child = record.links.first_child;
            let mut siblings = HashSet::new();
            while let Some(child_key) = child {
                if !siblings.insert(child_key) {
                    return Err(TreeError::LinkLoop(child_key));
                }
                stack.push(child_key);
                child = self.read_any(child_key)?.links.next_sibling;
            }
            for portal in self.portal_dependents(key) {
                stack.push(portal);
            }
        }
        Ok(keys)
    }

    pub(crate) fn portal_dependents(&self, owner: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
        self.portals_by_owner
            .get(&owner)
            .into_iter()
            .flat_map(|portals| portals.iter().copied())
            .chain(
                self.created_portals_by_owner
                    .get(&owner)
                    .into_iter()
                    .flat_map(|portals| portals.iter().copied()),
            )
            .filter(|key| !self.retired_set.contains(key))
    }

    pub(crate) fn read_any(&self, key: NodeKey) -> Result<&Occurrence, TreeError> {
        self.edits
            .get(&key)
            .or_else(|| self.base.get(key.slot, key.generation).ok())
            .ok_or(TreeError::Missing(key))
    }

    pub(crate) fn edit(&mut self, key: NodeKey) -> Result<&mut Occurrence, TreeError> {
        if self.retired_set.contains(&key) {
            return Err(TreeError::Retired(key));
        }
        if !self.edits.contains_key(&key) {
            let value = self
                .base
                .get(key.slot, key.generation)
                .map_err(|_| TreeError::Missing(key))?
                .clone();
            self.edits.insert(key, value);
        }
        self.edits.get_mut(&key).ok_or(TreeError::Missing(key))
    }

    pub(crate) fn set_root_owner(
        &mut self,
        root: NodeKey,
        owner: Option<NodeKey>,
    ) -> Result<(), TreeError> {
        let is_created_portal = self.read(root)?.root_role == Some(RootRole::Portal)
            && self.created_set.contains(&root);
        self.edit(root)?.root_owner = owner;
        if is_created_portal {
            if let Some(owner) = owner {
                self.created_portals_by_owner
                    .entry(owner)
                    .or_default()
                    .insert(root);
            }
        }
        Ok(())
    }

    pub(crate) fn insert_before(
        &mut self,
        parent: NodeKey,
        child: NodeKey,
        before: Option<NodeKey>,
    ) -> Result<bool, TreeError> {
        // Validate the anchor before the self-anchor no-op.  A foreign anchor
        // must not become valid merely because it equals the child.
        if let Some(anchor) = before {
            if self.read(anchor)?.links.parent != Some(parent) {
                return Err(TreeError::InvalidAnchor(anchor));
            }
        }
        let parent_kind = self.read(parent)?.kind;
        let child_record = self.read(child)?;
        if child_record.root_role.is_some() {
            return Err(TreeError::ProtectedRoot(child));
        }
        match parent_kind {
            HostKind::ContentHost | HostKind::Editor => {
                return Err(TreeError::ParentCannotHaveChildren(parent_kind));
            }
            HostKind::Animation if child_record.kind != HostKind::Box => {
                return Err(TreeError::InvalidAnimationChild(child_record.kind));
            }
            _ => {}
        }
        if before == Some(child) {
            return Ok(false);
        }
        if parent == child {
            return Err(TreeError::Cycle);
        }
        let child_parent = child_record.links.parent;
        if child_parent == Some(parent) && child_record.links.next_sibling == before {
            return Ok(false);
        }

        let mut cursor = Some(parent);
        let mut visited = HashSet::new();
        while let Some(key) = cursor {
            if !visited.insert(key) {
                return Err(TreeError::LinkLoop(key));
            }
            if key == child {
                return Err(TreeError::Cycle);
            }
            let record = self.read(key)?;
            cursor = record.root_owner.or(record.links.parent);
        }

        self.unlink_internal(child)?;
        let previous = if let Some(anchor) = before {
            self.read(anchor)?.links.previous_sibling
        } else {
            self.read(parent)?.links.last_child
        };
        self.touch_links([Some(parent), Some(child), previous, before]);

        if let Some(previous) = previous {
            self.edit(previous)?.links.next_sibling = Some(child);
        } else {
            self.edit(parent)?.links.first_child = Some(child);
        }
        if let Some(anchor) = before {
            self.edit(anchor)?.links.previous_sibling = Some(child);
        } else {
            self.edit(parent)?.links.last_child = Some(child);
        }
        {
            let child_record = self.edit(child)?;
            child_record.links.parent = Some(parent);
            child_record.links.previous_sibling = previous;
            child_record.links.next_sibling = before;
        }
        let parent_record = self.edit(parent)?;
        parent_record.links.child_count = parent_record
            .links
            .child_count
            .checked_add(1)
            .ok_or(TreeError::CountOverflow(parent))?;
        Ok(true)
    }

    pub(crate) fn detach(&mut self, parent: NodeKey, child: NodeKey) -> Result<bool, TreeError> {
        if self.read(child)?.links.parent != Some(parent) {
            return Err(TreeError::WrongParent {
                child,
                expected: parent,
                actual: self.read(child)?.links.parent,
            });
        }
        self.unlink_internal(child)?;
        Ok(true)
    }

    fn unlink_internal(&mut self, child: NodeKey) -> Result<bool, TreeError> {
        let links = self.read(child)?.links;
        let Some(parent) = links.parent else {
            return Ok(false);
        };
        self.touch_links([
            Some(parent),
            Some(child),
            links.previous_sibling,
            links.next_sibling,
        ]);
        if let Some(previous) = links.previous_sibling {
            if self.read(previous)?.links.next_sibling != Some(child) {
                return Err(TreeError::BrokenLink(previous));
            }
            self.edit(previous)?.links.next_sibling = links.next_sibling;
        } else {
            if self.read(parent)?.links.first_child != Some(child) {
                return Err(TreeError::BrokenLink(parent));
            }
            self.edit(parent)?.links.first_child = links.next_sibling;
        }
        if let Some(next) = links.next_sibling {
            if self.read(next)?.links.previous_sibling != Some(child) {
                return Err(TreeError::BrokenLink(next));
            }
            self.edit(next)?.links.previous_sibling = links.previous_sibling;
        } else {
            if self.read(parent)?.links.last_child != Some(child) {
                return Err(TreeError::BrokenLink(parent));
            }
            self.edit(parent)?.links.last_child = links.previous_sibling;
        }
        let parent_record = self.edit(parent)?;
        parent_record.links.child_count = parent_record
            .links
            .child_count
            .checked_sub(1)
            .ok_or(TreeError::CountUnderflow(parent))?;
        let child_record = self.edit(child)?;
        child_record.links.parent = None;
        child_record.links.previous_sibling = None;
        child_record.links.next_sibling = None;
        Ok(true)
    }

    pub(crate) fn retire_subtree_with_keys(
        &mut self,
        root: NodeKey,
        members: &[NodeKey],
    ) -> Result<(), TreeError> {
        self.retired
            .try_reserve(members.len())
            .map_err(|_| TreeError::Capacity)?;
        self.retired_set
            .try_reserve(members.len())
            .map_err(|_| TreeError::Capacity)?;
        self.unlink_internal(root)?;
        for key in members {
            if self.retired_set.insert(*key) {
                self.retired.push(*key);
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), TreeError> {
        for key in self.link_touched.iter().copied() {
            if self.retired_set.contains(&key) {
                continue;
            }
            let record = self.read(key)?;
            if record.root_role.is_none() && record.links.parent.is_none() {
                return Err(TreeError::Orphan(key));
            }
            if let Some(parent) = record.links.parent {
                if self.read(parent)?.links.first_child.is_none()
                    && self.read(parent)?.links.last_child.is_none()
                {
                    return Err(TreeError::BrokenLink(parent));
                }
            }
            if let Some(previous) = record.links.previous_sibling {
                if self.read(previous)?.links.next_sibling != Some(key) {
                    return Err(TreeError::BrokenLink(previous));
                }
            }
            if let Some(next) = record.links.next_sibling {
                if self.read(next)?.links.previous_sibling != Some(key) {
                    return Err(TreeError::BrokenLink(next));
                }
            }
            match (record.links.first_child, record.links.last_child) {
                (None, None) if record.links.child_count == 0 => {}
                (Some(first), Some(last)) if record.links.child_count > 0 => {
                    let first_record = self.read(first)?;
                    let last_record = self.read(last)?;
                    if first_record.links.parent != Some(key)
                        || first_record.links.previous_sibling.is_some()
                        || last_record.links.parent != Some(key)
                        || last_record.links.next_sibling.is_some()
                    {
                        return Err(TreeError::BrokenLink(key));
                    }
                }
                _ => return Err(TreeError::BrokenLink(key)),
            }
        }
        Ok(())
    }

    fn touch_links<I>(&mut self, keys: I)
    where
        I: IntoIterator<Item = Option<NodeKey>>,
    {
        for key in keys.into_iter().flatten() {
            self.link_touched.insert(key);
        }
    }

    pub(crate) fn into_plan(self) -> TreePlan {
        TreePlan {
            edits: self.edits,
            created: self.created,
            retired: self.retired,
            retired_set: self.retired_set,
            portal_buckets: HashMap::new(),
            portal_touched: HashSet::new(),
        }
    }
}
