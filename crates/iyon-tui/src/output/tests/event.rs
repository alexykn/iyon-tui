use std::{cell::Cell, rc::Rc};

use crate::output::{EventCx, Output, OutputQueue, OutputRouter};

#[derive(Debug, PartialEq, Eq)]
struct NonClone {
    text: String,
}

enum PayloadAction {
    NonClone(String),
    Local(Rc<String>),
}

#[test]
fn event_context_owns_non_clone_and_non_send_payloads() {
    let non_clone = Output::<NonClone>::new();
    let local = Output::<Rc<String>>::new();
    let mut queue = OutputQueue::new();
    {
        let mut cx = queue.event_cx();
        cx.emit(
            non_clone,
            NonClone {
                text: String::from("owned"),
            },
        );
        cx.emit(local, Rc::new(String::from("same thread")));
    }

    let mut router = OutputRouter::<PayloadAction>::new();
    router
        .route(non_clone, |value| PayloadAction::NonClone(value.text))
        .unwrap();
    router.route(local, PayloadAction::Local).unwrap();

    let actions = router.drain(&mut queue).unwrap();
    assert!(queue.is_empty());
    assert!(matches!(&actions[0], PayloadAction::NonClone(value) if value == "owned"));
    assert!(matches!(&actions[1], PayloadAction::Local(value) if value.as_str() == "same thread"));
}

#[derive(Debug)]
struct Counter {
    value: usize,
    changed: Output<usize>,
}

impl Counter {
    fn new() -> Self {
        Self {
            value: 0,
            changed: Output::new(),
        }
    }

    fn increment(&mut self, cx: &mut EventCx<'_>) {
        self.value += 1;
        cx.emit(self.changed, self.value);
    }
}

#[test]
fn event_payload_contains_post_mutation_state() {
    let mut counter = Counter::new();
    let output = counter.changed;
    let mut queue = OutputQueue::new();

    {
        let mut cx = queue.event_cx();
        counter.increment(&mut cx);
    }

    let mut router = OutputRouter::<usize>::new();
    router.route(output, |value| value).unwrap();

    assert_eq!(counter.value, 1);
    assert_eq!(router.drain(&mut queue).unwrap(), vec![1]);
}

#[test]
fn event_context_only_queues_until_its_borrow_ends() {
    let output = Output::<()>::new();
    let called = Rc::new(Cell::new(false));
    let mut queue = OutputQueue::new();
    let mut router = OutputRouter::<()>::new();
    let route_called = Rc::clone(&called);
    router
        .route(output, move |()| {
            route_called.set(true);
        })
        .unwrap();

    {
        let mut cx = queue.event_cx();
        cx.emit(output, ());
        assert!(!called.get());
    }
    assert!(!called.get());

    router.drain(&mut queue).unwrap();
    assert!(called.get());
}
