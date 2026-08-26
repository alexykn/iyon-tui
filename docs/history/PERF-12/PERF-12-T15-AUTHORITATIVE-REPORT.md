# PERF-12 T15 authoritative transport report

**Status:** technical comparison complete; transport adoption remains an
explicit owner decision
**Final harness revision:** `019a048b7c6fa8e6cc6c1e4f0e6635dc78c1b0b7`
**Bun:** 1.4.0 / `34cbb9a40b4bd1bd767d134a7065e66c2432a676`
**Rust:** 1.97.1 (`8bab26f4f`, target `aarch64-apple-darwin`)
**Candidates:** `napi_default`, `direct_ffi_oracle`

## Artifacts

- Common matrix JSONL:
  `packages/iyon-tui/bench/PERF-12-T15-authoritative-019a048b7c6f.jsonl`
- Common matrix summary:
  `packages/iyon-tui/bench/PERF-12-T15-authoritative-019a048b7c6f.json`
- Memory gate:
  `packages/iyon-tui/bench/PERF-12-T15-memory-3a76f5069246.jsonl`
- Multi-edit gate:
  `packages/iyon-tui/bench/PERF-12-T15-multi-edit-7acdc10375e9.jsonl`
- Generic trace gate:
  `packages/iyon-tui/bench/PERF-12-T15-realistic-80707ce7d9af.jsonl`

All timing cases ran in fresh Bun processes. The native artifact was staged
separately for each arm, and the default N-API artifact was restored after the
runs. The direct arm remained behind `direct-ffi` throughout.

## Common matrix

The authoritative orchestrator completed **311 cases per arm / 622 total
records** with:

- 9 workload families;
- 10 primary retained modes;
- base sizes 20, 200, and 2,000;
- selected 10,000-node cutoff/rebuild cases;
- wide axis sizes through 100,000 and wide grid/path supplemental cases;
- warmup and sample counts from §102, including 10,000 samples for tiny cases.

Results:

- correctness/structural mismatches: **0**;
- phase-array length mismatches: **0**;
- all records contain semantic, transport-preparation,
  native-materialization, host-commit, and total timing samples;
- geometric mean `N-API / direct-FFI`: **1.0421**;
- therefore N-API was approximately **4.2% slower** than the direct oracle on
  this heterogeneous matrix.

This ratio is evidence, not an automatic adoption decision.

## Supplemental gates

### Multi-edit

The typed transaction runner completed 2, 8, 32, and 64-edit cases for both
arms with 1,000 measured samples per case. All eight cases passed final screen
parity. N-API/direct median ratios were approximately:

```text
2 edits:  1.049
8 edits:  1.055
32 edits: 1.050
64 edits: 1.035
```

### Generic terminal trace

The process-isolated trace exercised stable history-like content,
progressive stream-like updates, periodic status and layout changes, and
structural insertions. Both arms produced matching first-screen output and
1,000 phase-visible samples. Median timings were:

```text
N-API:       18,791 ns
Direct FFI:  15,750 ns
ratio:       1.193
```

This is a generic framework trace, not product-specific application policy.

### Memory gate

Both arms completed the one-million-operation churn run, retaining 100 sampled
views. After each maintenance checkpoint, both converged to:

```text
semantic cache entries: 204
native ref slots:       204
leased slots:           1
unleased live slots:    203
```

The explicit live-state counters showed no historical-operation slope. RSS
remained allocator/JIT dependent and was not used as the sole decision metric.

## Decision status

The T15 technical evidence is complete and reproducible from committed raw
artifacts. It does **not** select a transport. Both generated safe N-API and
feature-gated direct FFI remain buildable, tested, and retained until the
repository owner makes an explicit decision. T16 cleanup is not started.
