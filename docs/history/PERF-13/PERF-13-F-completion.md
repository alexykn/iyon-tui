# PERF-13-F — Connector projection, unified convergence, and viewport integration

**Status:** implemented

## Delivered

- Added a generic presentation-facing `ContentProvider` boundary. Layout and
  paint consume content measurements and derived surfaces without importing the
  Source/Port/Connector registry or any TypeScript transport.
- Added a plain-text Connector projection backed by immutable retained Source
  chunks. Projection uses the existing UTF-8-aware `ProjectedText` and
  `ViewCompiler` wrapping path for `word`, `grapheme`, and `noWrap` Funnels;
  it does not construct semantic Views for changing content.
- Added Connector-local bounded width-dependent projection caching keyed by
  Source identity/generation, content generation, Source revision, offered
  width, and Funnel wrap configuration. Committed and candidate projections are
  separate; candidate projections are promoted only after the frame commits.
  Non-visible/cold Connectors release derived projections and caches.
- Replaced zero-sized ContentHost measurement with candidate projection
  intrinsic metrics. ContentHost layout records retain the Port identity and
  projection revision, and paint cache keys invalidate when content-derived
  output changes.
- Integrated Connector projection into the existing measure/place/paint frame
  transaction. Projection occurs while measuring the candidate, before final
  placement/paint, and requested Connector switches use the old visible
  Connector as fallback until the new projection is ready.
- Preserved transactional operational failure behavior: failed candidate
  projections leave the old Connector/frame visible; no-active-Connector cases
  remain empty; retry-blocked failures do not spin; Source revision changes,
  remounts, and relevant width changes provide retry boundaries.
- Added ContentHost clipping/compositing through the retained `Surface`, with
  ContentHost inherited presentation styles applied without putting native
  style IDs into Source data. Unicode/newline/wide-glyph behavior continues to
  use the existing physical compiler and Surface rules.
- Added content-aware layout cache invalidation and a native content-dirty
  boundary so a Source mutation coalesced with retained-state work cannot
  commit a stale content projection through the state-only fast path.
- Integrated existing `ScrollPane`/`RowViewport` behavior. Layout reports the
  full extent behind a component-owned viewport through a crate-private generic
  extent notification; `ScrollPane` retains offset/follow state, clamps after
  content growth or Connector switches, and preserves detached scrolling.
  ContentPort/Connector do not own scroll state.
- Exposed committed `projectedSourceRevision` through native Connector status
  and the TypeScript `ContentConnectorStatus` API. Candidate revisions are not
  reported as visible before commit.

## Architecture and v5 alignment

PERF-13-F deliberately implements the plain immediate content path only. The
v5 direction in `IYON-UI-PRELIMINARY-DESIGN-v5.md` is recorded in the companion
implementation notes:

- Funnel is the immutable normalized specification;
- Connector owns relationship-local execution state;
- a future `Smooth(config)` is a Funnel delivery-policy value, while mutable
  reveal frontiers and the Rust clock belong to Connector;
- Markdown, diff, ANSI, complete semantic annotation interpretation, and
  native smoothing remain later content transformations/migrations;
- the semantic text IR remains backend-neutral and width-independent; physical
  rows are Connector/host-derived;
- future component-only ScrollSurface/residency and Taffy/React work are not
  pulled into PERF-13-F.

The existing host-owned `HostTextStream`/History pipeline remains available as
the pre-G compatibility path. PERF-13-F does not double-dispatch Source
mutations into it or migrate Markdown/History consumers prematurely.

## Verification

The full PERF-12 benchmark suite was not run. Relevant tests and focused
runtime checks passed:

- `cargo test -p iyon-tui --features native-host --lib` — 754 passed, 1 ignored;
- `cargo test -p iyon-tui-native --tests` — native/unit and generated ABI tests
  passed, with the existing representation benchmark ignored;
- focused PERF-13-D, harness, and generated-ABI Bun tests — 12 passed;
- retained Source/Connector probes for append, replace, cold activation,
  wrapping modes, shared multi-host Source fan-out, state/content coalescing,
  unmount/remount, candidate fallback, and Connector status;
- ScrollPane probes for follow-end, detached scrolling, append growth,
  Connector switching, and resize/reflow;
- direct-FFI content probe against the staged `.node` artifact;
- Rust formatting, clippy, TypeScript typecheck, declaration closure, ownership,
  ABI generation, no-default-feature checks, package bundling, C-header layout
  assertions, and default/direct-FFI native staging/symbol checks.

No full benchmark suite was run because the PERF-13 handoff benchmark policy
limits PERF-13 tranches to relevant tests and explicitly scoped measurements;
the full PERF-12 benchmark run takes roughly four hours.

## Post-implementation audit hardening

A follow-up audit against the resolved handoff and v5 ownership model found and
corrected several edge cases:

- CRLF was being treated as one grapheme by the projected-text compiler, so a
  retained plain Source could collapse `"a\\r\\nb"` into one physical line.
  The projected compiler now preserves the existing LF hard-line semantics.
- A candidate activation failure could be retried by a second measurement pass
  in the same frame, especially when the fallback used a different Source.
  Failure keys now include the attempted width/Source input, and final binding
  selection never performs a second arbitrary-width activation attempt.
- Deactivating or disposing a failed requested Connector did not schedule the
  removal of the old visible fallback Connector. The Port now advances a frame
  for that transition.
- Layout measurement keys now include the selected Connector, exact offered
  width, projection readiness, and mount/error state. This prevents stale
  measured content from being committed when a Connector cache entry was
  evicted or a cold candidate had no derived projection.
- Content-only frames without the legacy History sideband reuse the retained
  resolved semantic scene and rebuild only derived layout/paint products.
- Content projections now retain only their widest painted row rather than an
  offered-width cell buffer for every row, reject impossible terminal-row or
  logical-line materialization before compilation, and report an explicit
  `LIMIT_EXCEEDED` Connector diagnostic instead of risking allocator-fatal
  derived-row growth.
- ContentHost physical completeness now records vertical clipping when content
  is placed in a bounded non-viewport allocation.
