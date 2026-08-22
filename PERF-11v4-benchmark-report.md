# PERF-11v4 benchmark report

**Status:** complete; comparison only. No PERF-12 transport or architecture was designed.

## Frozen candidates

```text
Candidate A: direct_7v2
  historical eager immutable BridgeViewNode DAG reconstructed from:
  e5292d62c4011610850cbdc1ba4a35f296f78e4f
  current Bun 1.4 Direct N-API decoder and current Rust TUI/cache

Diagnostic control: direct_current
  current lazy/pending View construction + Direct decoder

Candidate B: native_11v3
  completed production PERF-11v3 Tui.render path, unmodified
```

The reconstruction is benchmark-only at
`packages/iyon-runtime/bench/perf7v2_direct/view.ts`. Its adaptations are
mechanical import/schema/class-name changes; it preserves eager construction,
one NodeId per immutable value, immediate frozen BridgeViewNode creation,
WeakMap identity, and `nodeForPerf7v2Bridge()` as a lookup-only operation.

## Environment and provenance

```text
source SHA:             7c670ccd99fb296b18719f62c1aa845a3e3605de
benchmark source paths: clean at run
working tree note:      markdown-only planning/documentation changes were present and excluded from provenance because they cannot affect measured code paths
Bun:                    1.4.0
Bun revision:           1.4.0+34cbb9a40
Rust:                   rustc 1.97.1 (8bab26f4f 2026-07-14)
target:                 aarch64-apple-darwin
macOS:                  26.5.2
CPU:                    MacBookPro18,3
native artifact SHA:    81a1682d90f3b0be14fb0bb5cd07007c6e1a6b2a9c09158ff2e45be8aff54a9e
Cargo profile:          release
Cargo timing feature:   perf-packed-timing
RUSTFLAGS:              unset
LTO/codegen overrides:  none (Cargo release defaults)
```

`bun run build:iyon` was executed with `ION_NATIVE_FEATURES` unset. The
staged production addon rendered a valid bridge node through
`NativeTuiHost.render(Object)`. Replacing its schema with `999` failed with
`unsupported TUI View bridge schema 999`, proving the normal build executes the
Direct decoder.

## Correctness gates

Passed:

- Full current-schema parity covering text, styled text, diff, spacer, row,
  column, all layout child variants, hanging, grid, container, clamp,
  content-max, component, decoration, styles, borders, padding, alignment,
  overflow, and Unicode text.
- 100 deterministic randomized differential seeds, including shared trees,
  Unicode, styles, grids, decorations, and retained structures.
- Cache identity test: repeated root identity produces a Direct cache hit.
- Cache expiry/recovery test: stale weak entries are pruned and the same bridge
  reconstructs correctly.
- Same current native artifact and isolated child Bun environment per case.

Test command:

```text
bun test packages/iyon-runtime/tests/perf11v4_direct.test.ts
7 pass, 0 fail
```

## Matrix and sampling

The raw artifact retains every sample in
`packages/iyon-runtime/bench/PERF-11v4-comparison-raw.jsonl`.

```text
normal retained:  200 matched cases
  9 workloads × 3 sizes (20, 200, 2,000) × 7 modes
  9 exact-identity cases at 10,000 nodes
  targeted plain-text 10,000-node SHARED_DEEP and
  LARGE_SHARED_SUBTREE_CUTOFF cases (depth 128 / stable cutoff)
special/text:       24 matched cases
wide scaling:       15 matched cases
  widths 32, 256, 2,048, 10,000, 100,000 × replace/insert/remove
realistic trace:     1 matched case, 1,000 operations
matched total:     240 cases
raw records:       705
```

Normal cases used 50 warmups and 1,000 measured samples; exact identity used
10,000 warmups and 10,000 measured samples. Wide scaling used 10 warmups and
1,000 measured samples, except 100,000-child cases used 50 because construction
is not duration-sensible at the normal sample count. Candidate order alternated
deterministically between isolated child processes. `direct_current` was
recorded for all normal/special cases. The run retained 705 raw child records
and froze one source SHA for every child, so concurrent markdown-only commits
cannot create mixed provenance or affect timing. Each record also includes CPU,
current RSS delta, peak RSS delta, and heap delta; these are secondary resource
observations and make no GC claim.

## End-to-end result

Ratios are `native_11v3 / direct_7v2`; below 1.0 means Native Shadow is
faster. Aggregation is the geometric mean of matched per-case median ratios.
The summary also retains each matched pair's percentage difference, p95/p99
ratios, bootstrap median-ratio CI, and construction/transport/native phases.

| Group | Cases | Ratio | Result |
|---|---:|---:|---|
| Normal retained | 200 | 0.780× | 22.0% faster for native in aggregate |
| Text/special | 24 | 1.400× | Candidate A 28.6% faster |
| Wide scaling | 15 | 0.631× | 36.9% faster for native |
| Realistic trace | 1 | 1.977× | Candidate A 49.4% faster |

Mode aggregates:

| Mode | Native/A ratio |
|---|---:|
| COLD | 1.389× |
| FIRST_USE | 1.242× |
| IDENTICAL_IDENTITY | 0.099× |
| SHARED_PATH | 1.049× |
| SHARED_DEEP | 1.082× |
| LARGE_SHARED_SUBTREE_CUTOFF | 1.146× |
| REBUILT_EQUIVALENT | 1.457× |
| TEXT_METADATA_PATCH | 0.936× |
| DECORATION_PATCH | 2.519× |
| WIDE_PARENT_ONE_EDIT | 0.460× |
| WIDE_PARENT_INSERT | 0.489× |
| WIDE_PARENT_REMOVE | 1.118× |

Representative phase medians from the raw records:

| Case | Candidate | Construction | Transport prepare | Native/host commit | Total |
|---|---|---:|---:|---:|---:|
| small exact identity | direct_7v2 | 42 ns | 42 ns | 667 ns | 750 ns |
| small exact identity | native_11v3 | 42 ns | 42 ns | 42 ns | 124 ns |
| small shared path | direct_7v2 | 1,625 ns | 83 ns | 45,750 ns | 48,417 ns |
| small shared path | native_11v3 | 1,833 ns | 83 ns | 47,667 ns | 50,168 ns |
| small cold | direct_7v2 | 11,792 ns | 83 ns | 140,875 ns | 153,250 ns |
| small cold | native_11v3 | 12,709 ns | 83 ns | 143,084 ns | 156,292 ns |
| realistic trace/op | direct_7v2 | 643,000 ns | 122,958 ns | 372,750 ns | 1,182,125 ns |
| realistic trace/op | native_11v3 | 1,758,042 ns | 129,250 ns | 383,917 ns | 2,337,000 ns |

Scaling checks from the plain-text cases:

| Check | Parameter | direct_7v2 median | native_11v3 median | Native/A |
|---|---:|---:|---:|---:|
| Exact identity | 20 nodes | 750 ns | 124 ns | 0.165× |
| Exact identity | 200 nodes | 750 ns | 124 ns | 0.165× |
| Exact identity | 2,000 nodes | 750 ns | 124 ns | 0.165× |
| Exact identity | 10,000 nodes | 751 ns | 124 ns | 0.165× |
| Shared deep | depth 4 | 112,875 ns | 116,792 ns | 1.035× |
| Shared deep | depth 16 | 210,999 ns | 259,292 ns | 1.229× |
| Shared deep | depth 64 | 6,736,251 ns | 8,767,792 ns | 1.302× |
| Shared deep | depth 128 | 101,561,750 ns | 133,037,291 ns | 1.310× |
| Stable cutoff | 10,000 nodes | 1,461,667 ns | 1,948,792 ns | 1.333× |

Exact identity is independent of descendant count for both candidates in this
run; shared-deep cost follows the changed ancestor frontier, and the 10,000-
node stable subtree remains a cutoff rather than a full descendant walk.

The trace distribution was deterministic and retained in the raw record:

```text
stream append:       550 / 1,000
no View change:      150 / 1,000
View replacement:    100 / 1,000
layout change:        80 / 1,000
component update:     50 / 1,000
History update:       40 / 1,000
larger structural:    30 / 1,000
```

The trace is decisive: changing the transport does not compensate for the
Candidate A construction/operation shape on this representative application
sequence.

## Streaming-path note (independent of the architecture verdict)

The PERF-8 through PERF-11v3 generations also rebuilt the assistant streaming
pipeline: native `TextStream` append/seal, incremental stream compilation,
source-rooted offsets/revisions, and native History stream units. The 11v4
realistic trace was dominated by 550/1,000 stream appends and their visibly
smoother rendering is a direct product of that work. This result is
**transport-independent**: the specialized native stream path does not route
text through structural View construction in any candidate (`§42` of PERF-12),
and it must be preserved regardless of which View bridge architecture wins.
Specifically:

```text
keep: native TextStream append/seal/snapshot, incremental stream row
      compilation, source-rooted StreamOffset/StreamRange/StreamRevision,
      frozen/live History units, tail promotion, viewport anchoring
keep: the streaming Markdown projector pipeline that consumes it
not under test: no 11v4 candidate measured the stream bytes path itself;
      Candidate A's realistic-trace win comes from semantic construction and
      tool/component update shape above the stream boundary
```

PERF-12 inherits this unchanged. If a future tranche touches the stream path,
it must benchmark it separately; do not fold streaming regressions or gains
into the View-bridge decision.

## Route and diagnostic evidence

Timing runs deliberately disabled route counters so per-route JS counter work
did not distort Candidate A versus Native Shadow timing. The separate
non-timing diagnostic artifact is
`packages/iyon-runtime/bench/PERF-11v4-route-diagnostics.json`.

Its 14-case counter run (source SHA
`7c670ccd99fb296b18719f62c1aa845a3e3605de`) recorded:

```text
no_op:             12
native_builder:    82
render_ref:         0
scalar:             0
shallow_depth:      0
path_ref:           0
structural:         0
edit_transaction:   0
fallback:           0
recovery:           0
```

These counts are diagnostic only, not timing results.

## Final classification

**D — Candidate A wins.** Candidate A wins the realistic trace by a large
margin, despite Native Shadow wins on exact identity and wide persistent
structures. The evidence justifies a separate PERF-12 investigation into
whether advantageous Candidate A semantic construction can be combined with a
lower-overhead native boundary. PERF-12 must be researched and designed
independently; it is outside PERF-11v4.

The current HEAD direct path, production build proof, faithful historical
builder, parity tests, isolated comparison harness, raw samples, comparison
summary, and report are committed separately from any future architecture.
