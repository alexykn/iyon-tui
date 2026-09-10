use std::{
    any::{Any, TypeId},
    collections::HashMap,
    error::Error,
    fmt,
};

use super::{
    event::OutputQueue,
    handle::{Output, OutputId},
};

struct Route<A> {
    payload_type: TypeId,
    map: Box<dyn Fn(Box<dyn Any + Send>) -> A + Send>,
}

/// Failure to add a second route for an output channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteConflict;

impl fmt::Display for RouteConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("output already has an application route")
    }
}

impl Error for RouteConflict {}

/// An internal invariant failure while routing an erased output event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputDispatchError {
    TypeMismatch,
}

impl fmt::Display for OutputDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch => formatter.write_str("output payload type mismatch"),
        }
    }
}

impl Error for OutputDispatchError {}

pub struct OutputRouter<A> {
    routes: HashMap<OutputId, Route<A>>,
}

impl<A> OutputRouter<A> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn route<T: Send + 'static>(
        &mut self,
        output: Output<T>,
        map: impl Fn(T) -> A + Send + 'static,
    ) -> Result<(), RouteConflict> {
        if self.routes.contains_key(&output.id()) {
            return Err(RouteConflict);
        }

        let route = Route {
            payload_type: TypeId::of::<T>(),
            map: Box::new(move |payload| {
                let payload = payload
                    .downcast::<T>()
                    .expect("output route payload type was checked before dispatch");
                map(*payload)
            }),
        };
        self.routes.insert(output.id(), route);
        Ok(())
    }

    pub fn remove<T: 'static>(&mut self, output: Output<T>) -> bool {
        self.routes.remove(&output.id()).is_some()
    }

    pub(crate) fn drain(&self, queue: &mut OutputQueue) -> Result<Vec<A>, OutputDispatchError> {
        let mut actions = Vec::new();

        while let Some(event) = queue.pop_front() {
            let Some(route) = self.routes.get(&event.output) else {
                continue;
            };

            if route.payload_type != event.payload_type {
                return Err(OutputDispatchError::TypeMismatch);
            }

            actions.push((route.map)(event.payload));
        }

        Ok(actions)
    }
}
