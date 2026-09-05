use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    component::{ComponentId, ComponentSnapshot, MountGraph},
    interaction::MountedCapabilities,
    presentation::View,
    retained_state::ViewStateSnapshot,
};

/// Component snapshots and mount-time topology needed to interpret semantic
/// component slots during layout. It never owns a reconstructed View tree.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResolutionOverlay {
    pub(crate) components: HashMap<ComponentId, ComponentSnapshot>,
    /// Immutable shared versions of host-owned retained presentation state,
    /// keyed by the native attachment identity carried by a semantic View.
    /// Sharing the frame's `Arc` versions removes the repeated map and
    /// decoration copies of the old per-branch snapshot clones.
    pub(crate) states: HashMap<u64, Arc<ViewStateSnapshot>>,
}

impl ResolutionOverlay {
    pub(crate) fn component(&self, id: ComponentId) -> Option<&ComponentSnapshot> {
        self.components.get(&id)
    }

    pub(crate) fn state(&self, id: u64) -> Option<&ViewStateSnapshot> {
        self.states.get(&id).map(Arc::as_ref)
    }
}

/// A semantic scene plus the derived component topology used by presentation.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedScene {
    pub(crate) view: View,
    pub(crate) mounts: MountGraph,
    pub(crate) capabilities: MountedCapabilities,
    pub(crate) overlay: ResolutionOverlay,
    /// Retained semantic paths for ContentHost occurrences. Content updates
    /// use this occurrence index rather than recursively searching sibling
    /// Views on every dirty notification.
    pub(crate) content_paths: HashMap<u64, Vec<View>>,
    /// Retained semantic paths to component-slot occurrences. Structural
    /// component replacement uses these prefixes to update only affected
    /// content paths without another whole-scene DFS.
    pub(crate) component_paths: HashMap<ComponentId, Vec<View>>,
    /// Reverse index from component-slot identity to affected ContentPorts.
    pub(crate) content_path_components: HashMap<ComponentId, Vec<u64>>,
}

impl PartialEq for ResolvedScene {
    fn eq(&self, other: &Self) -> bool {
        self.view == other.view && self.mounts == other.mounts
    }
}
