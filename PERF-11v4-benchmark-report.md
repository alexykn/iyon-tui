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
source SHA:             7d72f208aee1da8163b240ef5e34c5222253f802
working tree at run:    clean
Bun:                    1.4.0
Bun revision:           1.4.0+34cbb9a40
Rust:                   rustc 1.97.1 (8bab26f4f 2026-07-14)
target:                 aarch64-apple-darwin
macOS:                  26.5.2
CPU:                    MacBookPro18,3
native artifact SHA:    81a1682d90f3b0be14fb0bb5cd07007c6e1a6b2a9c09158ff2e45be8aff54a9e
Cargo timing feature:   perf-packed-timing
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
5 pass, 0 fail
```

## Matrix and sampling

The raw artifact retains every sample in
`packages/iyon-runtime/bench/PERF-11v4-comparison-raw.jsonl`.

```text
normal retained:  189 matched cases
  9 workloads × 3 sizes (20, 200, 2,000) × 7 modes
special/text:      24 matched cases
wide scaling:      15 matched cases
  widths 32, 256, 2,048, 10,000, 100,000 × replace/insert/remove
realistic trace:    1 matched case, 1,000 operations
matched total:    229 cases
raw records:      672
```

Normal cases used 50 warmups and 1,000 measured samples; exact identity used
10,000 measured samples. Wide scaling used 10 warmups and 50 samples because
100,000-child construction is not duration-sensible at the normal sample
count. Candidate order alternated deterministically between isolated child
processes. `direct_current` was recorded for all normal/special cases.

## End-to-end result

Ratios are `native_11v3 / direct_7v2`; below 1.0 means Native Shadow is
faster. Aggregation is the geometric mean of matched per-case median ratios.

| Group | Cases | Ratio | Result |
|---|---:|---:|---|
| Normal retained | 189 | 0.843× | 15.7% faster for native in aggregate |
| Text/special | 24 | 1.235× | Candidate A 23.5% faster |
| Wide scaling | 15 | 0.796× | 20.4% faster for native |
| Realistic trace | 1 | 2.561× | Candidate A 61.0% faster |

Mode aggregates:

| Mode | Native/A ratio |
|---|---:|
| COLD | 1.435× |
| FIRST_USE | 1.512× |
| IDENTICAL_IDENTITY | 0.072× |
| SHARED_PATH | 1.051× |
| SHARED_DEEP | 1.031× |
| LARGE_SHARED_SUBTREE_CUTOFF | 1.141× |
| REBUILT_EQUIVALENT | 1.492× |
| TEXT_METADATA_PATCH | 0.908× |
| DECORATION_PATCH | 1.690× |
| WIDE_PARENT_ONE_EDIT | 0.858× |
| WIDE_PARENT_INSERT | 0.870× |
| WIDE_PARENT_REMOVE | 0.676× |

Representative phase medians from the raw records:

| Case | Candidate | Construction | Transport prepare | Native/host commit | Total |
|---|---|---:|---:|---:|---:|
| small exact identity | direct_7v2 | 42 ns | 42 ns | 708 ns | 791 ns |
| small exact identity | native_11v3 | 42 ns | 0 ns | 42 ns | 84 ns |
| small shared path | direct_7v2 | 1,416 ns | 42 ns | 45,208 ns | 46,791 ns |
| small shared path | native_11v3 | 1,875 ns | 0 ns | 47,083 ns | 49,624 ns |
| small cold | direct_7v2 | 11,083 ns | 42 ns | 137,500 ns | 149,208 ns |
| small cold | native_11v3 | 12,042 ns | 0 ns | 142,000 ns | 154,542 ns |
| realistic trace/op | direct_7v2 | 544,584 ns | 125 ns | 76,250 ns | 625,917 ns |
| realistic trace/op | native_11v3 | 1,501,333 ns | 0 ns | 88,584 ns | 1,603,125 ns |

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

## Route and diagnostic evidence

Timing runs deliberately disabled route counters so per-route JS counter work
did not distort Candidate A versus Native Shadow timing. The separate
non-timing diagnostic artifact is
`packages/iyon-runtime/bench/PERF-11v4-route-diagnostics.json`.

Its 14-case counter run recorded:

```text
no_op:          12
native_builder: 82
all other named routes: 0
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
