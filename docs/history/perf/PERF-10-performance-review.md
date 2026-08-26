# PERF-10 performance review

## Audit qualification

The checked-in PERF-10 JSONL is now the **counter-free timing run**. It was built with `perf-packed-timing`, which enables the FastShared/V3 transport without native atomic counters; the benchmark was run with `PERF_COUNTERS=0`. The JSONL records `instrumentation: "timing"`. A separate counter build remains available for structural diagnostics.

The benchmark now attributes bridge conversion consistently to the encoding phase for direct, V3, V4, and FastShared. The total timer includes view construction plus the complete bridge/encoding/commit path. The old PERF-7v2 comparison remains cross-revision and uses a different sample count, so it is directional rather than a same-build regression test. Its historical phase fields were also recorded before the current uniform hook accounting, so only the total comparison should be used as the cross-version timing result.

## Requested comparison: PERF-10 versus PERF-7v2 direct

This compares **FastShared directly against the old pre-packed `direct` transport**, using 90 exactly matched cases (`workload + size + mode + node_count`). Ratios are medians of per-case ratios; below 1.0 is faster.

| Phase or result | FastShared / PERF-7v2 direct | Interpretation |
|---|---:|---|
| Total | **1.101** | **10.1% slower** |
| Commit | **0.831** | **16.9% faster** |
| Native | **0.505** | **49.5% faster** |
| JS construction | **1.975** | **97.5% slower** |
| Encoding | 0 vs **about 9.1 µs** on the matched subset | New FastShared page/command encoding cost (about 7.5 µs across all 156 local cases) |

By mode, FastShared was 5.9% slower on cold, 10.0% slower on first-use, 34.7% faster on exact identity, 17.2% slower on shared-path updates, and 9.4% faster on rebuilt-equivalent cases. These are cross-revision comparisons, so they show direction rather than a clean single-release benchmark.

The important result is: **FastShared is slower than the old direct baseline on the matched normal matrix.** Its native/commit savings are real, but they are more than consumed by JS construction and page encoding. As a sensitivity estimate, subtracting the measured FastShared encoding time from each case would change the matched-matrix ratio from 1.101 to about 0.965—roughly **3.5% faster** than PERF-7v2 direct. Against the current local PERF-10 direct rerun, the same estimate is about **8.6% faster**. That is an elimination estimate, not a reclassification: moving the same work into native would not produce this gain. The retained-trace result below is specific to the new authoritative trace and is not a replacement for the normal matrix.

## PERF-10 against its local direct baseline

| Measure | Direct baseline | PERF-10 FastShared | Change |
|---|---:|---:|---:|
| Normal workloads | 100.0% | 103.7% | **3.7% slower** |
| Exact identity | 1,042 ns | 749 ns | **28.1% faster** |
| 100-operation retained trace | 787 ms | 152 ms | **80.7% faster** |

These are counter-free PERF-10 direct-versus-FastShared rerun measurements. The trace result is specific to the retained-update mix; it is not a general 80.7% improvement.

## Bottom line: PERF-10 versus PERF-7v2 direct

| Measure | PERF-7v2 direct baseline | PERF-10 FastShared |
|---|---:|---:|
| Normal workload | 100.0% | **110.1%** (**10.1% slower**) |
| Commit phase | 100.0% | **83.1%** (**16.9% faster**) |
| Native phase | 100.0% | **50.5%** (**49.5% faster**) |
| JS construction | 100.0% | **197.5%** (**97.5% slower**) |
| Exact identity | Not present in the PERF-7v2 file | **749 ns** |
| 100-operation trace | Not comparable/present in the same form | **152 ms** |

This is the direct comparison to use for PERF-10. The historical PERF-7v2 packed candidate is not used as the baseline. FastShared reduces commit/native work but loses overall because JS construction and shared-page encoding cost more. Its large advantage is specific to the retained-update trace; cold, first-use, rebuilt, and bulk work still fall back to V3.

## Scope

This review compares the checked-in benchmark JSONL files:

- `packages/iyon-runtime/bench/PERF-7v2-results.jsonl`
- `packages/iyon-runtime/bench/PERF-9-results.jsonl`

The historical PERF-9 comparison uses:

- **PERF-7v2 historical transport:** `packed` (context only; not the PERF-10 baseline)
- **PERF-9 V3:** `packed_v3`
- **PERF-9 Proto V4:** `packed_v4`

For the PERF-10 decision, the baseline is explicitly **PERF-7v2 `direct`**, matched against the PERF-10 `fast_shared` candidate.

Cases are matched by `workload`, `size`, `mode`, and `node_count`. Reported ratios are the median of per-case `median_ns` ratios, rather than a ratio of pooled samples.

The PERF-7v2 file contains two appended runs. The main comparison uses the complete original run (`git_sha=e5292d6`, 137 paired cases, 200 measured samples) and excludes the later partial run to avoid mixing revisions.

## Dataset summary

| Dataset | Records used | Paired cases | Normal samples | String lanes |
|---|---:|---:|---:|---|
| PERF-7v2 | 274 | 137 | 200 | n/a |
| PERF-9 | 1,124 | 156 per lane | 20 | S1 UTF-8 arena, S2 move-once strings |

PERF-9 also contains 10-sample tiny cases, a 1,000-sample exact-identity case, and 100-operation synthetic traces.

## Normal workload results

Ratio versus the same dataset's `direct` candidate. Values below 1.0 are faster.

| Candidate | Lane | Cases | Median total ratio | Result |
|---|---|---:|---:|---|
| PERF-7v2 `packed` | n/a | 137 | 0.990 | 1.0% faster |
| PERF-9 `packed_v3` | S1 UTF-8 | 156 | 1.060 | 6.0% slower |
| PERF-9 `packed_v4` | S1 UTF-8 | 156 | 1.061 | 6.1% slower |
| PERF-9 `packed_v3` | S2 strings | 156 | 1.054 | 5.4% slower |
| PERF-9 `packed_v4` | S2 configured lane | 156 | 1.064 | 6.4% slower |

V4 is compared primarily in S1 because V4 always transports UTF-8; the S2 V4 row is a diagnostic comparison, not a like-for-like string-lane implementation.

## V4 versus V3

### S1 UTF-8 lane

| Metric | V4 / V3 | Change |
|---|---:|---:|
| Total | 0.998 | 0.2% faster |
| Commit | 0.999 | 0.1% faster |
| Encoding | 0.969 | 3.1% faster |
| Native | 1.001 | 0.1% slower |

The end-to-end result is effectively tied. V4's lower encoding cost is mostly consumed by the rest of the bridge and native work.

### S2 configured lane

| Metric | V4 / V3 | Change |
|---|---:|---:|
| Total | 1.009 | 0.9% slower |
| Commit | 1.015 | 1.5% slower |
| Encoding | 1.236 | 23.6% slower |
| Native | 0.991 | 0.9% faster |

This is expected to be unfavorable: V3 uses move-once strings in S2, while V4 continues to encode UTF-8.

## Common PERF-7v2/PERF-9 cases

There are 90 exact common normal cases. Normalizing each transport against its local direct baseline gives:

| Transport | Median packed/direct ratio |
|---|---:|
| PERF-7v2 packed | 0.969 |
| PERF-9 V3 | 1.010 |
| PERF-9 Proto V4 | 1.000 |

On this common subset, V4 is approximately at direct parity and improves about 0.2 percentage points over V3. This does not establish a broad production win: PERF-9 has fewer samples and a different revision/matrix.

## Special cases

| Case | PERF-7v2 packed | PERF-9 V3 | PERF-9 V4 |
|---|---:|---:|---:|
| Exact identity vs local direct | +30.9% | +19.1% | +14.4% |
| Tiny S1 vs local direct | n/a | +29.4% | +28.9% |

The exact-identity V4 result is better than V3, but still slower than the direct candidate in this single case.

## Synthetic traces

The traces are not cross-version comparable: PERF-7v2 uses a 70% `SHARED_PATH` mix and 1,000 samples, while PERF-9 uses a 70% `LARGE_SHARED_SUBTREE_CUTOFF` mix and 100 operations.

Within PERF-9:

| Lane | V3 vs direct | V4 vs direct | V4 vs V3 |
|---|---:|---:|---:|
| S1 UTF-8 | 44.7% faster | 8.9% faster | 64.6% slower |
| S2 configured strings | 35.7% faster | 34.9% faster | 1.2% slower |

## Decision

PERF-7v2 remains the strongest broad normal-workload baseline. Proto V4 closes the V3 gap on the common subset and improves UTF-8 encoding modestly, but it is not a meaningful end-to-end improvement over V3.

The evidence supports keeping PERF-10 FastShared as an experimental, opt-in path for applications dominated by exact identity reuse or the authoritative retained trace. It does **not** justify making FastShared the default: against the old PERF-7v2 direct baseline it is 10.1% slower on the matched normal matrix, and against the current local direct rerun it is 3.7% slower. JS construction is 97.5% slower and shared-path updates are 17.2% slower against the old direct baseline. Retain V3 for cold, rebuilt, and bulk work. V4 should remain experimental rather than becoming the default transport.

## PERF-10 implementation validation

A reduced PERF-10 run was completed with the same iteration policy as the PERF-9 V4 evidence:

- 10 warmups
- 20 normal samples
- 10 tiny samples
- 1,000 exact-identity samples for `plain_text_column/small_view`
- 100 synthetic-trace operations
- small and medium view sizes

The run is recorded in `packages/iyon-runtime/bench/PERF-10-results.jsonl`.

On its 156 common normal S1 cases, the newly implemented FastShared candidate measured:

| Candidate | Median total versus local direct | Median versus V3 |
|---|---:|---:|
| PERF-10 V3 rerun | 2.8% slower | baseline |
| PERF-10 V4 rerun | 1.4% slower | 1.4% faster |
| FastShared | **3.7% slower** | **0.9% slower** |

The exact-identity case measured 1,042 ns direct, 1,125 ns V3, 1,083 ns V4, and **749 ns FastShared**. The 100-operation PERF-10 trace measured 787 ms direct and **152 ms FastShared**. These are counter-free PERF-10 rerun measurements, not direct replacements for the older PERF-9 JSONL results.

The benchmark router sends COLD, FIRST_USE, REBUILT_EQUIVALENT, unsupported batches, and FastShared cache misses through V3, while supported retained text, decoration, axis, grid, and PersistentSeq path-copy updates use the fixed-op native shared-page path.
