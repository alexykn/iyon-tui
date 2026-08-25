use super::{ComponentId, MountGraph};

#[derive(Debug, Default)]
pub(crate) struct MountedComponents {
    current: MountGraph,
}

impl MountedComponents {
    #[cfg(test)]
    pub(crate) fn current(&self) -> &MountGraph {
        &self.current
    }

    pub(crate) fn reconcile(&mut self, next: MountGraph) -> MountTransitions {
        let mut transitions = Vec::new();

        let current_ids = self.current.ids().collect::<Vec<_>>();
        for id in current_ids.into_iter().rev() {
            if !next.contains(id) {
                transitions.push(MountTransition::Unmounted { id });
            }
        }

        for node in next.iter() {
            if !self.current.contains(node.id) {
                transitions.push(MountTransition::Mounted {
                    id: node.id,
                    parent: node.parent,
                });
            }
        }

        self.current = next;
        MountTransitions { transitions }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MountTransition {
    Mounted {
        id: ComponentId,
        parent: Option<ComponentId>,
    },
    Unmounted {
        id: ComponentId,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MountTransitions {
    pub(crate) transitions: Vec<MountTransition>,
}

#[cfg(test)]
impl MountTransitions {
    pub(crate) fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }
}
