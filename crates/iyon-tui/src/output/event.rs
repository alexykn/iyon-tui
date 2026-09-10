use std::{
    any::{Any, TypeId},
    collections::VecDeque,
};

use super::handle::{Output, OutputId};

pub(super) struct ErasedOutputEvent {
    pub(super) output: OutputId,
    pub(super) payload_type: TypeId,
    pub(super) payload: Box<dyn Any + Send>,
}

pub(crate) struct OutputQueue {
    events: VecDeque<ErasedOutputEvent>,
}

impl OutputQueue {
    pub(crate) fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(crate) fn event_cx(&mut self) -> EventCx<'_> {
        EventCx { queue: self }
    }

    pub(super) fn pop_front(&mut self) -> Option<ErasedOutputEvent> {
        self.events.pop_front()
    }

    fn push<T: Send + 'static>(&mut self, output: Output<T>, payload: T) {
        self.events.push_back(ErasedOutputEvent {
            output: output.id(),
            payload_type: TypeId::of::<T>(),
            payload: Box::new(payload),
        });
    }

    #[cfg(test)]
    pub(super) fn push_mismatched_for_test<T: 'static, U: Send + 'static>(
        &mut self,
        output: Output<T>,
        payload: U,
    ) {
        self.events.push_back(ErasedOutputEvent {
            output: output.id(),
            payload_type: TypeId::of::<U>(),
            payload: Box::new(payload),
        });
    }
}

pub struct EventCx<'a> {
    queue: &'a mut OutputQueue,
}

impl EventCx<'_> {
    pub fn emit<T: Send + 'static>(&mut self, output: Output<T>, value: T) {
        self.queue.push(output, value);
    }
}
