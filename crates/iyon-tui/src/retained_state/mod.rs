//! Retained-state plane primitives.
//!
//! The host/application layer only coordinates lifecycle and frame ingress;
//! presentation override storage and effective-state derivation live here.

mod capabilities;
mod damage;
mod effects;
mod occurrence;
mod presentation;
mod record;
mod registry;

#[cfg(feature = "native-host")]
pub(crate) use capabilities::presentation_state_capable;
pub(crate) use damage::DamageRegion;
#[cfg(feature = "native-host")]
pub(crate) use effects::StateEffects;
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
