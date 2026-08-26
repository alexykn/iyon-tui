use super::*;

fn node(id: ComponentId, parent: Option<ComponentId>) -> MountNode {
    MountNode {
        id,
        parent,
        revision: ComponentRevision::default(),
    }
}

#[test]
fn mounts_are_parent_first_and_unmounts_are_child_first() {
    let a = ComponentId::allocate();
    let b = ComponentId::allocate();
    let mut mounted = MountedComponents::default();
    let next = MountGraph::new(vec![node(a, None), node(b, Some(a))]);

    let mounted_transitions = mounted.reconcile(next);
    assert_eq!(
        mounted_transitions.transitions,
        vec![
            MountTransition::Mounted {
                id: a,
                parent: None
            },
            MountTransition::Mounted {
                id: b,
                parent: Some(a),
            },
        ]
    );

    let removed = mounted.reconcile(MountGraph::default());
    assert_eq!(
        removed.transitions,
        vec![
            MountTransition::Unmounted { id: b },
            MountTransition::Unmounted { id: a },
        ]
    );
}

#[test]
fn reordering_and_reparenting_do_not_remount_existing_ids() {
    let a = ComponentId::allocate();
    let b = ComponentId::allocate();
    let c = ComponentId::allocate();
    let mut mounted = MountedComponents::default();
    mounted.reconcile(MountGraph::new(vec![
        node(a, None),
        node(c, Some(a)),
        node(b, None),
    ]));

    let reordered = mounted.reconcile(MountGraph::new(vec![
        node(b, None),
        node(c, Some(b)),
        node(a, None),
    ]));
    assert!(reordered.is_empty());
    assert_eq!(mounted.current().iter().nth(1).unwrap().parent, Some(b));
}

#[test]
fn revisions_do_not_create_mount_transitions() {
    let a = ComponentId::allocate();
    let mut mounted = MountedComponents::default();
    mounted.reconcile(MountGraph::new(vec![node(a, None)]));

    let mut changed = node(a, None);
    changed.revision = ComponentRevision::default().increment();
    assert!(mounted.reconcile(MountGraph::new(vec![changed])).is_empty());
}
