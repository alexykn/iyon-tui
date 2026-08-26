use std::time::{Duration, Instant};

use super::*;
use crate::interaction::MountedCapabilities;
use crate::output::OutputQueue;
use crate::presentation::{IntoView, View};
use crate::{ComponentCx, EventCx};

#[derive(Debug)]
struct Blinker {
    frame: u8,
    interval: Duration,
}

impl Component for Blinker {
    fn view(&self) -> View {
        View::text(self.frame.to_string()).into_view()
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.tick(self.interval, Self::tick);
    }
}

impl Blinker {
    fn tick(&mut self, _now: Instant, _cx: &mut EventCx<'_>) -> bool {
        self.frame += 1;
        true
    }
}

fn make_graph(ids: &[(ComponentId, Option<ComponentId>)]) -> MountGraph {
    MountGraph::new(
        ids.iter()
            .map(|(id, parent)| MountNode {
                id: *id,
                parent: *parent,
                revision: ComponentRevision::default(),
            })
            .collect(),
    )
}

fn make_capabilities(registry: &ComponentRegistry, graph: &MountGraph) -> MountedCapabilities {
    let mut capabilities = MountedCapabilities::default();
    for node in graph.iter() {
        capabilities.insert(
            node.id,
            registry
                .capabilities(node.id)
                .expect("registered component capabilities"),
        );
    }
    capabilities
}

#[test]
fn one_mounted_component_ticks_only_after_its_deadline() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let mut scheduler = TickScheduler::new();

    let start = Instant::now();
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(make_graph(&[(handle.id(), None)]));
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, start);
    assert_eq!(
        scheduler.next_timeout(start, Duration::from_secs(1)),
        Duration::from_millis(80)
    );
    assert_eq!(
        scheduler
            .tick_due_with_events(
                start + Duration::from_millis(79),
                &mut registry,
                &mut OutputQueue::new()
            )
            .ran,
        false
    );
    assert_eq!(registry.with(handle, |blinker| blinker.frame), Some(0));
    assert!(
        scheduler
            .tick_due_with_events(
                start + Duration::from_millis(80),
                &mut registry,
                &mut OutputQueue::new()
            )
            .dirty
    );
    assert_eq!(registry.with(handle, |blinker| blinker.frame), Some(1));
    assert_eq!(registry.revision(handle).unwrap().value(), 1);
}

#[test]
fn multiple_due_components_tick_in_mount_order() {
    let mut registry = ComponentRegistry::new();
    let first = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let second = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let mut scheduler = TickScheduler::new();
    let now = Instant::now();
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(make_graph(&[(first.id(), None), (second.id(), None)]));
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, now);

    assert!(
        scheduler
            .tick_due_with_events(
                now + Duration::from_millis(80),
                &mut registry,
                &mut OutputQueue::new()
            )
            .dirty
    );
    assert_eq!(registry.with(first, |blinker| blinker.frame), Some(1));
    assert_eq!(registry.with(second, |blinker| blinker.frame), Some(1));
    assert_eq!(registry.revision(first).unwrap().value(), 1);
    assert_eq!(registry.revision(second).unwrap().value(), 1);
}

#[test]
fn local_capability_sync_updates_a_mounted_tick_without_graph_rescan() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let mut scheduler = TickScheduler::new();
    let now = Instant::now();
    let graph = make_graph(&[(handle.id(), None)]);
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(graph.clone());
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, now);

    registry
        .with_mut(handle, |blinker| {
            blinker.interval = Duration::from_millis(10)
        })
        .unwrap();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_component_capability(handle.id(), &capabilities, now);
    assert_eq!(
        scheduler.next_timeout(now, Duration::from_secs(1)),
        Duration::from_millis(10)
    );
    assert!(
        scheduler
            .tick_due_with_events(
                now + Duration::from_millis(10),
                &mut registry,
                &mut OutputQueue::new(),
            )
            .dirty
    );
    assert_eq!(registry.with(handle, |blinker| blinker.frame), Some(1));
}

#[test]
fn unmount_deactivates_and_remount_resets_the_deadline() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let mut scheduler = TickScheduler::new();
    let initial = Instant::now();
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(make_graph(&[(handle.id(), None)]));
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, initial);

    let transitions = mounted.reconcile(MountGraph::default());
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(
        &graph,
        &capabilities,
        &transitions,
        initial + Duration::from_millis(100),
    );
    assert!(
        !scheduler
            .tick_due_with_events(
                initial + Duration::from_secs(1),
                &mut registry,
                &mut OutputQueue::new()
            )
            .ran
    );
    assert_eq!(registry.with(handle, |blinker| blinker.frame), Some(0));

    let remount = initial + Duration::from_secs(2);
    let transitions = mounted.reconcile(make_graph(&[(handle.id(), None)]));
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, remount);
    assert!(
        !scheduler
            .tick_due_with_events(remount, &mut registry, &mut OutputQueue::new())
            .ran
    );
    assert!(
        scheduler
            .tick_due_with_events(
                remount + Duration::from_millis(80),
                &mut registry,
                &mut OutputQueue::new(),
            )
            .dirty
    );
    assert_eq!(registry.with(handle, |blinker| blinker.frame), Some(1));
}

#[test]
fn different_intervals_select_the_earliest_deadline_independently() {
    let mut registry = ComponentRegistry::new();
    let fast = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(80),
    });
    let slow = registry.register(Blinker {
        frame: 0,
        interval: Duration::from_millis(200),
    });
    let mut scheduler = TickScheduler::new();
    let start = Instant::now();
    let mut mounted = MountedComponents::default();
    let transitions = mounted.reconcile(make_graph(&[(fast.id(), None), (slow.id(), None)]));
    let graph = mounted.current().clone();
    let capabilities = make_capabilities(&registry, &graph);
    scheduler.sync_capabilities(&graph, &capabilities, &transitions, start);
    assert_eq!(
        scheduler.next_timeout(start, Duration::from_secs(1)),
        Duration::from_millis(80)
    );
    scheduler.tick_due_with_events(
        start + Duration::from_millis(80),
        &mut registry,
        &mut OutputQueue::new(),
    );
    assert_eq!(registry.with(fast, |blinker| blinker.frame), Some(1));
    assert_eq!(registry.with(slow, |blinker| blinker.frame), Some(0));
    assert_eq!(
        scheduler.next_timeout(start + Duration::from_millis(80), Duration::from_secs(1)),
        Duration::from_millis(80)
    );
    scheduler.tick_due_with_events(
        start + Duration::from_millis(200),
        &mut registry,
        &mut OutputQueue::new(),
    );
    assert_eq!(registry.with(slow, |blinker| blinker.frame), Some(1));
}

#[derive(Debug)]
struct ZeroTick;

impl Component for ZeroTick {
    fn view(&self) -> View {
        View::spacer(0)
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.tick(Duration::ZERO, Self::tick);
    }
}

impl ZeroTick {
    fn tick(&mut self, _now: Instant, _cx: &mut EventCx<'_>) -> bool {
        false
    }
}

#[test]
fn zero_intervals_are_rejected_by_the_component_capability_contract() {
    let mut registry = ComponentRegistry::new();
    let handle = registry.register(ZeroTick);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registry.capabilities(handle.id());
    }));
    assert!(result.is_err());
}
