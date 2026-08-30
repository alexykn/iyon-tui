# PERF-13-B completion

**Status:** implemented on `perf-13`.

PERF-13-B adds retained presentation state without changing semantic View
structure for state mutations.

## Delivered

- Public `Tui.viewState()` and `View.state(state)` APIs with typed presentation
  patches, explicit clear operations, nullable-field semantics, and style-state
  overrides.
- Host-owned native ViewState records with monotonic host-scoped identities,
  desired/visible binding tracking, lifecycle validation, and owner teardown.
- Plane-neutral resource registration with host affinity, attachment leases,
  duplicate-use checks, and stale/disposed-handle rejection.
- Structural-only state identity lowering: semantic Views retain framework
  `HandleId` values; native state identities are resolved at the structural
  boundary.
- Exhaustive native node-kind presentation capability classification.
- Native `OccurrenceBox` base/effective records, state revisions, effect
  classification, local retained repaint, rectangle damage, and state-aware
  layout/paint cache keys.
- PERF-13-A environment wake/epoch scheduling moved into
  `crates/iyon-tui/src/application/environment.rs`; ViewState N-API boundary is
  isolated in `crates/iyon-tui-native/src/tui/view_state.rs`.

Geometry mutation is intentionally not exposed; it remains PERF-13-C scope.
Legacy `Decorated` lowering remains the compatibility path and is counted.

## Verification

Focused PERF-13-B, H3, handle, harness, and runtime tests pass, along with
TypeScript typechecking, declaration closure, ABI generation/checks, ownership,
Rust formatting, constrained native checks, clippy, native staging, and native
ViewState/layout tests.
