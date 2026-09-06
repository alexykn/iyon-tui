# L1-13 reviewed implementation checkpoint

**Implementation parent:** `e4a580509974eae5bb4e8058aa0daeb7a6851592` (L1-12).

**Performance comparison baseline:** `36efe16f90ac7ef3c758c65ee35a0502d6b95ea1`.

**Decision:** commit the reviewed implementation and passing correctness gates;
continue performance qualification in follow-up commits. This checkpoint is
**not final L1-13 acceptance** under the handoff's sections 19–21.

## Included implementation

- Retire the Rust fluent authoring facade, migrate native/internal callers to
  private retained factories, and enforce the narrow binding surface with
  compile-fail and binding checks. The TypeScript public surface remains stable.
- Retain demand-lazy immutable state snapshots and boxed mutable records, with
  exact desired/visible/in-flight lifetime handling and remount coverage.
- Defer immediate non-History content painting to prepared row windows while
  retaining the physical products required by Smooth and History.
- Capture themes in prepared projections, distinguish physical-row demand in
  cache keys, propagate physical completeness, and cache row traversal indexes.
- Bulk-build rebuilt persistent semantic suffixes. Coalesce repeated per-Port
  dirty records without losing updates after a prepared epoch or skipping
  layout invalidation when a paint-only reason escalates to measurement.
- Preserve captured receipt products and add focused coverage for old snapshots,
  stale theme tickets, immediate/Smooth cache sharing, clipped wide glyphs,
  backgrounds, row windows, delayed receipts, and metric/paint escalation.

## Parent verification at the checkpoint

On macOS arm64, the parent inspected the implementation changes and ran:

| Check | Result |
|---|---|
| `cargo test -p iyon-tui --lib --all-features` | 647 passed, 1 ignored |
| `cargo test -p iyon-tui --all-features --test root_compile_contract` | Passed |
| `bun run test` | 115 passed, 809 expectations |
| `bun run check:ownership` | Passed |
| `bun run check:tui-binding` | Passed; 124 binding exports |
| `bun run typecheck` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `bun run native:smoke` | Passed with the staged default addon |
| `git diff --check` | Passed |

No non-macOS validation is claimed by this checkpoint.

## Follow-up performance gates

The exploratory measurements show substantial content-frame improvements and
unmounted-state memory parity, but do not satisfy the complete acceptance
matrix. In particular, repeated 1K tiny content appends remain approximately
20–38% slower in measured medians (roughly 0.5–1.2 ms per burst). That regression
has not been waived. The separate 100K retention-heavy profile does not explain
the below-retention 1K workload.

Complete the literal section 19.3 workloads and section 19.4 evidence, including
native retained-snapshot memory/copy slopes, construction and persistent
structure coverage, difficult grammar restarts, timed two-host styling,
delivery, viewport/resize, lifecycle/fanout, and repeated failures/partial
receipts. Functional tests are not substitutes for these measurements.

Follow-up qualification must distinguish cold and steady-state work, Source
acceptance from visible-frame latency, and default timing from instrumented
counter profiles. Repeated comparable baseline/candidate runs must report
phase distributions, work/copy counts, and retained/peak memory with verified
artifact provenance. Preserve the existing platform matrix and explicitly
record unavailable platform checks.

The exploratory `l13_*` TypeScript benchmarks, unfinished qualification report,
and generated target directories are intentionally outside this checkpoint.
They remain available locally for the performance follow-up; no generated
binary or temporary benchmark result is included here.
