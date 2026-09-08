use std::{
    fmt,
    sync::atomic::{AtomicU32, Ordering},
};

use super::generated::HandleKind;

const FIRST_GENERATION: u32 = 1;
static NEXT_HOST_NAMESPACE: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HostNamespace(pub u32);

impl HostNamespace {
    pub fn allocate() -> Option<Self> {
        let mut current = NEXT_HOST_NAMESPACE.load(Ordering::Relaxed);
        loop {
            if current == 0 || current == u32::MAX {
                return None;
            }
            let next = current + 1;
            match NEXT_HOST_NAMESPACE.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(Self(current)),
                Err(observed) => current = observed,
            }
        }
    }

    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeKey {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceKey {
    pub slot: u32,
    pub generation: u32,
    pub kind: HandleKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UiHandle {
    pub host_namespace: u32,
    pub slot: u32,
    pub generation: u32,
    pub kind: HandleKind,
}

impl UiHandle {
    pub const fn new(
        host_namespace: HostNamespace,
        slot: u32,
        generation: u32,
        kind: HandleKind,
    ) -> Option<Self> {
        if slot == 0 || generation == 0 {
            return None;
        }
        Some(Self {
            host_namespace: host_namespace.get(),
            slot,
            generation,
            kind,
        })
    }

    pub const fn node_key(self) -> Option<NodeKey> {
        if self.kind as u32 != HandleKind::Node as u32 {
            return None;
        }
        Some(NodeKey {
            slot: self.slot,
            generation: self.generation,
        })
    }

    pub const fn resource_key(self) -> Option<ResourceKey> {
        if self.kind as u32 == HandleKind::Node as u32 {
            return None;
        }
        Some(ResourceKey {
            slot: self.slot,
            generation: self.generation,
            kind: self.kind,
        })
    }
}

impl NodeKey {
    pub const fn handle(self, host_namespace: HostNamespace) -> UiHandle {
        // Keys are created only by the arena, which guarantees nonzero fields.
        UiHandle {
            host_namespace: host_namespace.get(),
            slot: self.slot,
            generation: self.generation,
            kind: HandleKind::Node,
        }
    }
}

impl ResourceKey {
    pub const fn handle(self, host_namespace: HostNamespace) -> UiHandle {
        UiHandle {
            host_namespace: host_namespace.get(),
            slot: self.slot,
            generation: self.generation,
            kind: self.kind,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArenaError {
    Capacity,
    InvalidKey,
    StaleKey,
}

impl fmt::Display for ArenaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => formatter.write_str("arena capacity exhausted"),
            Self::InvalidKey => formatter.write_str("arena key is invalid"),
            Self::StaleKey => formatter.write_str("arena key is stale"),
        }
    }
}

#[derive(Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
    burned: bool,
    free_index: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    pub(crate) fn ensure_capacity_for(&mut self, additional: usize) -> Result<(), ArenaError> {
        let tail = additional.saturating_sub(self.free.len());
        self.slots
            .try_reserve(tail)
            .map_err(|_| ArenaError::Capacity)
    }

    pub(crate) fn ensure_free_capacity_for(&mut self, additional: usize) -> Result<(), ArenaError> {
        self.free
            .try_reserve(additional)
            .map_err(|_| ArenaError::Capacity)
    }

    fn preview_raw(&self, count: usize) -> Result<Vec<(u32, u32)>, ArenaError> {
        let tail = count.saturating_sub(self.free.len());
        let end = self
            .slots
            .len()
            .checked_add(tail)
            .ok_or(ArenaError::Capacity)?;
        if end > u32::MAX as usize {
            return Err(ArenaError::Capacity);
        }
        let mut keys = Vec::with_capacity(count);
        for index in
            (0..count.min(self.free.len())).map(|offset| self.free[self.free.len() - 1 - offset])
        {
            let slot = self
                .slots
                .get(index.checked_sub(1).ok_or(ArenaError::InvalidKey)? as usize)
                .ok_or(ArenaError::InvalidKey)?;
            if slot.burned || slot.value.is_some() || slot.free_index.is_none() {
                return Err(ArenaError::InvalidKey);
            }
            keys.push((index, slot.generation));
        }
        for offset in 0..tail {
            let index = self.slots.len() + offset + 1;
            keys.push((
                u32::try_from(index).map_err(|_| ArenaError::Capacity)?,
                FIRST_GENERATION,
            ));
        }
        Ok(keys)
    }

    pub(crate) fn preview_node_keys(&mut self, count: usize) -> Result<Vec<NodeKey>, ArenaError> {
        self.ensure_capacity_for(count)?;
        self.preview_raw(count).map(|keys| {
            keys.into_iter()
                .map(|(slot, generation)| NodeKey { slot, generation })
                .collect()
        })
    }

    pub(crate) fn preview_resource_keys(
        &mut self,
        count: usize,
        kind: HandleKind,
    ) -> Result<Vec<ResourceKey>, ArenaError> {
        self.ensure_capacity_for(count)?;
        self.preview_raw(count).map(|keys| {
            keys.into_iter()
                .map(|(slot, generation)| ResourceKey {
                    slot,
                    generation,
                    kind,
                })
                .collect()
        })
    }

    pub(crate) fn insert_reserved_node(&mut self, key: NodeKey, value: T) {
        self.insert_reserved_raw(key.slot, key.generation, value);
    }

    pub(crate) fn insert_reserved_resource(&mut self, key: ResourceKey, value: T) {
        self.insert_reserved_raw(key.slot, key.generation, value);
    }

    fn insert_reserved_raw(&mut self, slot: u32, generation: u32, value: T) {
        assert!(slot > 0);
        let index = slot as usize - 1;
        if index == self.slots.len() {
            assert_eq!(generation, FIRST_GENERATION);
            self.slots.push(Slot {
                generation,
                value: Some(value),
                burned: false,
                free_index: None,
            });
        } else {
            let free_index = self.slots[index]
                .free_index
                .take()
                .expect("arena reservation points at a free slot");
            let removed = self.free.swap_remove(free_index);
            assert_eq!(removed, slot);
            if free_index < self.free.len() {
                let moved = self.free[free_index];
                self.slots[moved as usize - 1].free_index = Some(free_index);
            }
            let slot_ref = self
                .slots
                .get_mut(index)
                .expect("arena reservation points inside the slot table");
            assert!(!slot_ref.burned);
            assert_eq!(slot_ref.generation, generation);
            assert!(slot_ref.value.is_none());
            slot_ref.value = Some(value);
        }
        self.live = self.live.checked_add(1).expect("arena live count overflow");
    }

    pub(crate) fn get(&self, slot: u32, generation: u32) -> Result<&T, ArenaError> {
        let slot = self
            .slots
            .get(slot.checked_sub(1).ok_or(ArenaError::InvalidKey)? as usize)
            .ok_or(ArenaError::InvalidKey)?;
        if generation == 0 || slot.generation != generation || slot.value.is_none() {
            return Err(ArenaError::StaleKey);
        }
        slot.value.as_ref().ok_or(ArenaError::StaleKey)
    }

    pub(crate) fn get_mut(&mut self, slot: u32, generation: u32) -> Result<&mut T, ArenaError> {
        let slot = self
            .slots
            .get_mut(slot.checked_sub(1).ok_or(ArenaError::InvalidKey)? as usize)
            .ok_or(ArenaError::InvalidKey)?;
        if generation == 0 || slot.generation != generation || slot.value.is_none() {
            return Err(ArenaError::StaleKey);
        }
        slot.value.as_mut().ok_or(ArenaError::StaleKey)
    }

    pub(crate) fn remove(&mut self, slot: u32, generation: u32) -> Result<T, ArenaError> {
        let slot_ref = self
            .slots
            .get_mut(slot.checked_sub(1).ok_or(ArenaError::InvalidKey)? as usize)
            .ok_or(ArenaError::InvalidKey)?;
        if generation == 0 || slot_ref.generation != generation {
            return Err(ArenaError::StaleKey);
        }
        let value = slot_ref.value.take().ok_or(ArenaError::StaleKey)?;
        self.live = self
            .live
            .checked_sub(1)
            .expect("arena live count underflow");
        if slot_ref.generation == u32::MAX {
            slot_ref.burned = true;
            slot_ref.free_index = None;
        } else {
            slot_ref.generation += 1;
            let free_index = self.free.len();
            self.free.push(slot);
            slot_ref.free_index = Some(free_index);
        }
        Ok(value)
    }

    pub(crate) fn contains(&self, slot: u32, generation: u32) -> bool {
        self.get(slot, generation).is_ok()
    }

    pub(crate) fn live_count(&self) -> usize {
        self.live
    }

    #[cfg(test)]
    pub(crate) fn free_capacity(&self) -> usize {
        self.free.capacity()
    }

    #[cfg(test)]
    pub(crate) fn free_len(&self) -> usize {
        self.free.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, u32, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    u32::try_from(index + 1).expect("arena slot index fits u32"),
                    slot.generation,
                    value,
                )
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_changes_generation_and_exhausted_slots_are_burned() {
        let mut arena = Arena::<u8>::new();
        let first = arena.preview_node_keys(1).expect("first slot")[0];
        arena.insert_reserved_node(first, 1);
        assert_eq!(arena.remove(first.slot, first.generation), Ok(1));
        let second = arena.preview_node_keys(1).expect("reused slot")[0];
        assert_eq!(second.slot, first.slot);
        assert_eq!(second.generation, first.generation + 1);
        arena.insert_reserved_node(second, 2);
        assert_eq!(arena.remove(second.slot, second.generation), Ok(2));

        let slot = &mut arena.slots[0];
        slot.generation = u32::MAX;
        let exhausted = arena.preview_node_keys(1).expect("exhaustion candidate")[0];
        assert_eq!(exhausted.generation, u32::MAX);
        arena.insert_reserved_node(exhausted, 3);
        assert_eq!(arena.remove(exhausted.slot, exhausted.generation), Ok(3));
        assert!(arena.slots[0].burned);
        let replacement = arena.preview_node_keys(1).expect("new slot")[0];
        assert_eq!(replacement.slot, 2);
    }

    #[test]
    fn reserved_free_slots_consume_their_index_in_constant_time() {
        let mut arena = Arena::<u8>::new();
        let initial = arena.preview_node_keys(2).expect("initial slots");
        for (index, key) in initial.iter().copied().enumerate() {
            arena.insert_reserved_node(key, index as u8);
        }
        for key in initial {
            arena.remove(key.slot, key.generation).expect("retire slot");
        }

        let reused = arena.preview_node_keys(2).expect("reused slots");
        assert_eq!(reused[0].slot, 2);
        assert_eq!(reused[1].slot, 1);
        arena.insert_reserved_node(reused[0], 2);
        arena.insert_reserved_node(reused[1], 3);
        assert_eq!(arena.live_count(), 2);
    }
}
