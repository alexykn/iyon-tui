# PERF-13-C completion

**Status:** implemented on `perf-13`.

PERF-13-C adds retained geometry state without reconstructing semantic Views or
publishing a new structural root for state mutations.

## Delivered

- Typed `ViewState.setGeometry()` and `clearGeometry()` APIs with explicit
  nullable-bound/border-edge and clear semantics.
- Geometry support for width/height mode, padding, min/max bounds, axis/grid
  gap, physical alignment, and one-cell border-edge presence.
- Native prospective-patch validation keeps invalid or incompatible geometry
  mutations atomic and validates stored overrides at the H3 attachment seam.
- Rust-owned geometry overrides, independent geometry/presentation revisions,
  exhaustive node-kind capability validation, and bit-set effect
  classification for projection/measure/place/clip/paint/damage work.
- Effective geometry is applied during candidate measurement/preparation;
  dynamic padding, bounds, alignment, gaps, and border edges update the same
  physical occurrence, including initially undecorated nodes.
- Fixed-allocation occurrences use a local retained subtree layout/paint path;
  changes that can escape their allocation invalidate the target-to-root
  dependency frontier and use the retained resolved root as a conservative
  fallback. Clean sibling cache entries remain reusable.
- Old/new layout rectangles and effective occurrence boxes produce rectangle
  damage metadata, with full-frame escalation through the existing damage
  thresholds.
- Semantic `Decorated` compatibility input is classified as property-only and
  normalized by the native decoder into the canonical child/base occurrence;
  normalization is observable through retained/native counters.
- Paint-cache keys include retained text alignment and width intent so geometry
  changes cannot reuse stale text surfaces.
- Audit hardening validates state attachments in resolved component/History
  views, reconciles component state remounts before frame preparation, and
  preserves rejected detached-History attachments.
- Local dependency metadata, path-scoped layout/paint cache invalidation,
  deterministic state paint ordering, and candidate-cache discard prevent
  stale derived state from crossing a failed frame boundary.

## Verification

Passed constrained Rust checks/tests and clippy, TypeScript typechecking,
declaration closure, ABI checks, native staging, focused PERF-13-A/B and
harness tests, `git diff --check`, and a native PERF-13-C geometry smoke test.
The full PERF-12 suite was not rerun.
