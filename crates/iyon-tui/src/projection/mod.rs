//! Generic root-coordinate projection algebra.
//!
//! This layer owns projection envelopes, stability/transition/relation
//! validation, projector composition, and generic temporal publication. It
//! intentionally does not know Markdown, text IR, History, Views, or terminal
//! geometry.

mod compose;
mod projector;
mod smooth;
mod validate;
mod value;

#[cfg(test)]
mod migrated_tests;
#[cfg(test)]
mod tests;

pub use compose::{Then, ThenError};
pub use projector::{Projector, ProjectorExt};
pub use smooth::{Smooth, SmoothConfig, SmoothConfigError};
pub(crate) use validate::validate_projection;
pub use validate::{
    ProjectionRelationError, ProjectionTransitionError, ProjectionValidationError,
    validate_projection_relation, validate_projection_transition,
};
pub use value::{Projection, ProjectionBuilder, ProjectionSpan};
