use std::collections::HashMap;

use crate::{
    component::{ComponentId, ComponentSnapshot, MountGraph},
    interaction::MountedCapabilities,
    presentation::View,
};

/// Component snapshots and mount-time topology needed to interpret semantic
/// component slots during layout. It never owns a reconstructed View tree.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResolutionOverlay {
    pub(crate) components: HashMap<ComponentId, ComponentSnapshot>,
}

impl ResolutionOverlay {
    pub(crate) fn component(&self, id: ComponentId) -> Option<&ComponentSnapshot> {
        self.components.get(&id)
    }
}

/// A semantic scene plus the derived component topology used by presentation.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedScene {
    pub(crate) view: View,
    pub(crate) mounts: MountGraph,
    pub(crate) capabilities: MountedCapabilities,
    pub(crate) overlay: ResolutionOverlay,
}

impl PartialEq for ResolvedScene {
    fn eq(&self, other: &Self) -> bool {
        self.view == other.view && self.mounts == other.mounts
    }
}
