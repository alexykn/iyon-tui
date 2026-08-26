mod host;
mod layout;
mod resolve;
mod resolved;
mod root;

pub(crate) use host::{PreparedSceneFrame, SceneHost, SceneHostError};
#[cfg(any(test, feature = "perf-counters"))]
pub(crate) use layout::layout_resolved_scene;
pub(crate) use layout::{
    LayoutSync, LayoutSynchronizer, ResolvedSceneLayout, layout_resolved_scene_with_cache,
};
#[cfg(test)]
pub(crate) use resolve::resolve_scene;
pub(crate) use resolve::{ResolveError, ResolveSession};
pub(crate) use resolved::{ResolutionOverlay, ResolvedScene};
pub use root::Scene;
#[cfg(test)]
pub(crate) use root::resolve_root_scene;
pub(crate) use root::{
    ResolvedRootScene, resolve_component_subtree, resolve_root_scene_with_anchor_and_cache,
};

#[cfg(test)]
mod root_tests;
#[cfg(test)]
mod tests;
