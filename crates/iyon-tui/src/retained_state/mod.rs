//! Retained-state plane primitives.
//!
//! The host/application layer only coordinates lifecycle and frame ingress;
//! presentation override storage and effective-state derivation live here.

mod capabilities;
mod damage;
mod effects;
mod geometry;
mod occurrence;
mod presentation;
mod record;
mod registry;

pub(crate) use capabilities::{StateNodeKind, state_node_kind};
#[cfg(feature = "native-host")]
pub(crate) use capabilities::{presentation_state_capable, validate_geometry_for_kind};
pub(crate) use damage::DamageRegion;
pub(crate) use effects::StateEffects;
pub(crate) use geometry::EffectiveGeometry;
pub use geometry::GeometryAlignment;
#[cfg(feature = "native-host")]
pub use geometry::{ViewStateGeometryPatch, ViewStateGeometryProperty, ViewStateSizeMode};
pub(crate) use occurrence::OccurrenceBox;
pub(crate) use presentation::ViewStateSnapshot;
#[cfg(feature = "native-host")]
pub use presentation::{
    ViewStatePresentationPatch, ViewStatePresentationProperty, ViewStateTextAttributes,
};
#[cfg(feature = "native-host")]
pub(crate) use record::{ViewStateLifecycle, ViewStateRecord};
#[cfg(feature = "native-host")]
pub(crate) use registry::ViewStateRegistry;
