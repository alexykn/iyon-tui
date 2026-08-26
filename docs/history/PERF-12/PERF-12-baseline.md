# PERF-12 baseline record

**Status:** Tranche 12.0 (T1) evidence freeze complete. No PERF-12 architecture code was written; this record, the attribution JSONL, and the FFI floor JSONL are the tranche deliverables (`PERF-12` handoff `§82`, `§57`–`§58`, `§60`, `§61`, `§83`, `§109`).

## 1. Source freeze (§82)

```text
final PERF-11v4 SHA:      7c670ccd99fb296b18719f62c1aa845a3e3605de
historical 7v2 SHA:       e5292d62c4011610850cbdc1ba4a35f296f78e4f
PERF-12 starting SHA:     3d156a78e1577b4b2d491cf393b08956cf2aa7f5 (perf-refactor, clean tree)
Bun version:              1.4.0
Bun revision:             1.4.0+34cbb9a40
Rust:                     rustc 1.97.1 (8bab26f4f 2026-07-14)
target:                   aarch64-apple-darwin
macOS:                    26.5.2
CPU:                      Apple M1 Pro
native artifact SHA-256:  81a1682d90f3b0be14fb0bb5cd07007c6e1a6b2a9c09158ff2e45be8aff54a9e
```

The staged native artifact is byte-identical to the one used for the authoritative PERF-11v4 run; no Rust source changed between `ae6dd1f` (last code commit) and the T1 starting SHA, so every measurement below runs the frozen 11v4 binary.

### Bun qualification (§60)

`bun --version` = `1.4.0`, `bun --revision` = `1.4.0+34cbb9a40`. `tools/tui-abi/view_abi.toml` declares `minimum_bun = "1.4.0"` and `qualified_bun = "1.4.0"`; `packageManager` pins `bun@1.4.0`. The same runtime revision produced the PERF-11v4 results and all T1 probes.

### Same-image audit (§61)

Confirmed by source inspection of `crates/iyon-native/src/tui/view_abi.rs` (`tuiViewAbiBootstrap`, `runtime_handle_for_env`) and `packages/iyon-runtime/src/tui/native_view_abi.ts`:

```text
Node-API loads iyon-native.node            yes (require("../native/iyon-native.node"))
Node-API owns/returns NativeViewRuntime    yes (per-N-API-env Arc<NativeViewRuntime>, env cleanup hook)
bootstrap returns function pointers        yes (raw fn addresses + runtime ptr + manifest BLAKE3 handshake)
Bun linkSymbols binds same-image pointers  yes (generated linkViewAbi over bootstrap pointers)
second dlopen anywhere in runtime TS       no (grep "dlopen" over packages/iyon-runtime/src: no matches)
```

All transports in one Bun environment share one `NativeViewRuntime`; the semantic cache, ref slots, styles, hosts, and fallback state are singletons per environment as required.

## 2. PERF-11v4 result category

**Category D — Candidate A wins.** Candidate A (`direct_7v2`, faithful 7v2 reconstruction) beat `native_11v3` on the realistic trace by 49.4% (ratio 1.977×) while Native Shadow won exact identity (0.099×), wide persistent edits (~0.46–0.49× replace/insert), and normal retained aggregate (0.780×). Source: `../perf/PERF-11v4-benchmark-report.md`.

Consequences for PERF-12 (`§105`): the stop condition does **not** apply — 11v3 is not a decisive realistic-trace winner. PERF-12 proceeds with the tranches.

## 3. Current production ViewBacking shape (re-audited at 3d156a7)

`packages/iyon-runtime/src/tui/values/view.ts` describes a stable-shape private backing with three states (`view.ts`, `ViewBacking`):

```text
state 0 = materialized semantic node   (node?: BridgeViewNode)
state 1 = pending create               (createKind: text | spacer | axis + compact recipe fields)
state 2 = pending patch                (patchKind: textLayout | common, base View + mask/scalars)
```

Plus native-route metadata types that live beside the semantic layer today:
`NativeScalarPatch`, `NativeStructuralEdit` (axisSet / axisSplice / gridCell), `NativePathLineage`/`NativePathStep`, and packed-v3 meta registration (`packed_v3_meta.ts`). `BridgeViewNode` materialization is lazy via `nodeForBridge` for the cold/Direct compatibility path. This is exactly the pending-recipe architecture PERF-12 must not preserve into the new candidate (`§4`, `§84`).

## 4. Historical 7v2 shape (e5292d62)

Verified from `git show e5292d62:packages/iyon-runtime/src/tui/values/view.ts`:

```ts
export class View {
  private constructor(node: BridgeViewNode | BridgeViewNodeDraft) {
    nodes.set(this, withPrivateIdentity(node));
    Object.freeze(this);
  }
}
const nodes = new WeakMap<View, BridgeViewNode>();
export function nodeForBridge(view: View): BridgeViewNode { ... }  // lookup-only
```

Eager frozen `BridgeViewNode` construction at View creation, one NodeId per immutable value, direct child references, WeakMap-only identity. `bench/perf7v2_direct/view.ts` is the already-adapted benchmark-only reconstruction used by 11v4 and inherited by PERF-12 comparisons.

## 5. Current NativeViewRuntime maps and lease model (view_abi.rs)

```rust
pub(super) struct NativeViewRuntime {
    nodes: HashMap<u64, iyon_tui::WeakView>,   // semantic cache, environment-owned
    slots: HashMap<u32, NativeViewSlot>,       // NativeRef -> {node_id, weak, leased: Option<View>, js_lease_count}
    node_refs: HashMap<u64, u32>,              // NodeId -> NativeRef
    path_nodes / path_keys / builders / edit_txns / style_atoms / styles: HashMap<...>,
    next_native_ref, generation, stale_removals, release_batches, released_refs, ...
}
```

Lease model observed:

```text
publish(node_id, view):
    node_id == 0                                  -> FAST_INVALID
    node_refs hit with equal existing View        -> re-acquire lease, return same ref (semantic-cache-first)
    node_refs hit with different View             -> FAST_INVALID (identity conflict)
    weak cache hit with different live View       -> FAST_INVALID
    otherwise allocate ref, insert weak + slot(leased=Some(view), js_lease_count=1)
resolve_ref(ref): leased clone, else weak upgrade; expired -> remove slot+node_refs, FAST_CACHE_MISS
release_many(refs, n): decrement js_lease_count; zero-lease expired slots removed inline
prune_expired(): removes zero-lease weak-expired slots and stale node_refs (full sweep)
```

Expired-entry removal currently happens only on resolve/release paths plus explicit `tuiViewAbiBootstrap(prune=true)`; there is no periodic bounded maintenance hook yet (that is Tranche 3, `§55`). NativeRef values are monotonic (`next_native_ref`); valid refs are `< 0x8000_0000`, high bit reserved for statuses (`FAST_INVALID=0x8000_0001`, `FAST_CACHE_MISS=0x8000_0004`, `FAST_FALLBACK=0x8000_0005`).

## 6. Current Direct decoder behavior (tui.rs)

The Direct N-API `ViewDecoder` reads the bridge object's `id` first (`required_u64(&value, "id")`), rejects `id == 0`, consults the environment `NodeId -> WeakView` cache before decoding any payload, removes expired entries when directly observed, guards recursion with an `active` set, and publishes through `cache.nodes.insert(node_id, view.downgrade())`. This preserves the 7v2 identity-first ordering inside the Direct oracle path (`§72`).

Note: the pure-Direct path publishes into `nodes` without creating NativeRef slots, so `native_ref_slots` stays 0 under Direct-only workloads — visible in the attribution records below.

## 7. Generator/runtime bootstrap

```text
source of truth:   tools/tui-abi/view_abi.toml (469 declared items; 49 generated ABI functions)
generator:         tools/tui-abi-gen (cargo run -p tui-abi-gen -- generate/check)
outputs:           packages/iyon-runtime/src/tui/generated/{view_abi.ts,view_calls.ts,view_abi_conformance.ts,view_abi_manifest.json}
                   crates/iyon-native/src/generated/*
schema BLAKE3:     f7b30e32493e2e95f86541401308e5db64103bd8a7e694cbecbfe851040025d3
generator BLAKE3:  20435cb0e211e543dd671e6c86669cf3f205c8e77c5070f47f4d181a4a9d3c71
```

`tuiViewAbiBootstrap` validates the manifest hashes and exposes raw pointers for all 49 functions plus the runtime pointer; diagnostics (`semantic_cache_entries`, `native_ref_slots`, `leased_slots`, `path_nodes`, `builders`, `edit_transactions`, `style_atoms`, `styles`, `stale_removals`, `release_batches`, `released_refs`, `live_weak_upgrades`, `generation`, `alive`) are returned from the same call, with `prune_expired=true` performing the full weak sweep first. `path_keys` and `node_refs` entry counts are **not** exposed yet (recorded as null in T1 artifacts; diagnostic ABI extension belongs to Tranche 3, `§89`).

## 8. Memory attribution results (§57–§58)

Protocol: reproduce each ≥2.7 GiB-class PERF-11v4 block in a fresh child process against the frozen artifact; snapshot RSS/heap/native counters pre/fixtures/post-workload; forced cleanup checkpoint = release/close all roots → `Bun.gc(true)` → native full weak sweep → `Bun.gc(true)` → snapshot again. Raw numbers: `packages/iyon-runtime/bench/PERF-12-memory-attribution.jsonl`.

| Block | ops | peak RSS | post-cleanup RSS | post-cleanup JS heap | cache entries → after sweep | slots after sweep |
|---|---:|---:|---:|---:|---:|---:|
| control mixed_realistic/20 IDENTICAL_IDENTITY | 500 | 38 MiB | 39.5 MiB | 1.8 MiB | 22 → 0 | 0 |
| plain_text_column/2000 FIRST_USE | 1,000 | 1,279 MiB | 1,278.9 MiB | 3.5 MiB | 4,000 → 0 | 0 |
| row_heavy/2000 FIRST_USE | 1,000 | 1,346 MiB | 1,345.9 MiB | 3.5 MiB | 4,002 → 0 | 0 |
| plain_text_column/10000 SHARED_DEEP | 1,000 | 6,613 MiB | 4,314.6 MiB | 15.3 MiB | 4,106 → 0 | 0 |
| wide/100000 WIDE_PARENT_ONE_EDIT | 1,000 | 5,090 MiB | 2,451.3 MiB | 239.5 MiB | 100,069 → 0 | 0 |
| realistic_trace/1000 | 1,000 | 113 MiB | 113.0 MiB | 3.2 MiB | 4,297 → 1 (live 1) | 0 |

Reproduction confirmed: the SHARED_DEEP 10k block peaked at 6.6 GiB here versus 6.2–6.5 GiB in the frozen 11v4 raw records for the same case/candidates.

### §58 bucket classification

```text
A. benchmark result/sample retention      NOT the driver. Retained sample arrays are ~8–16 KiB/block.
B. fixtures intentionally live            small in control blocks; the wide block retains 239.5 MiB JS heap
                                          even after gc(true) (large child arrays/JSC non-compaction residual).
C. JS semantic objects / pending backings converge to ≤ ~15 MiB after cleanup in five of six blocks.
D. JSC/JIT/allocator high-water           DOMINANT. 1.3–4.3 GiB RSS remains after full cleanup while JS
                                          heap is 3–15 MiB and every native counter is 0.
E. native View strong leases              none: leased_slots = 0 in every block after cleanup.
F. expired NodeId WeakView metadata       transient accumulation is real (up to 100,069 entries in the
                                          wide block) but the existing prune/sweep converges it to O(live)=0.
G. expired NativeRef/node_ref metadata    ≈ none on the Direct path (no slots created); trace's 32 slots
                                          were fully released.
H. PersistentSeq structural high-water    not separately instrumented at T1 (limitation, noted below).
I. retained string/style payload          styles/style_atoms = 0 in all six blocks.
J. other native allocation                remainder of post-cleanup RSS unaccounted by live state;
                                          same allocator-retained address-space class as D.
```

### Answers to the §110 questions

```text
How much of the ~2.7 GiB was semantic/native live state?   effectively zero (counters all zero after sweep).
How much was stale weak-cache metadata?                    real but transient; converges to 0 under the existing
                                                           sweep; no linear post-sweep slope demonstrated.
How much was the benchmark harness?                        sample storage negligible; fixture scale itself sets the
                                                           churn volume (bucket B/C are MiB-scale).
How much was JSC/allocator high-water?                     the dominant share — GiB-scale RSS with MiB-scale live state.
```

Conclusion: the large RSS figures are allocator/JIT address-space high-water from extreme synthetic fixtures, **not** live semantic state and not transport-graph retention. The user-facing corroboration (Iyon app sessions sit near ~80 MB) is consistent with this classification. However, bucket F shows the §6 mechanism exists: weak-cache metadata grows with historical churn between sweeps. The shared-runtime scavenging work stays a mandatory prerequisite fix (Tranche 3), because automatic bounded maintenance — not manual sweeps — must provide the `O(live + slack)` invariant.

T1 limitations recorded honestly: PersistentSeq high-water (H) has no dedicated counter; `path_keys`/`node_refs` counts are absent from the diagnostic ABI; the wide block's 239.5 MiB post-gc JS heap was not chased with a JSC heap snapshot because no gate depends on it. These feed Tranche 3's diagnostic ABI design rather than blocking T1.

## 9. FFI floor probe decision (§83)

Raw numbers: `packages/iyon-runtime/bench/PERF-12-ffi-floor.jsonl`. Discipline: 30 warmup + 50 measured blocks × 1,000 ops per timed block (tiny-case rule, §102).

| Shape | median/op |
|---|---:|
| noop chain ×1 | 8 ns |
| noop chain ×64 | 107 ns (~1.7 ns/call) |
| generated runtimeNoop (incl. status recording) | 10 ns |
| scalar constructor (`view_spacer_create`) | 246 ns |
| fixed-arity constructor (`view_row_create_2`) | 478 ns |
| ref-buffer constructor (`view_axis_create_buffer`, 4 children) | 693 ns |
| retained patch (`view_text_layout_patch_root`) | 436 ns |
| retained patch (`view_common_patch_root`) | 429 ns |

Projected retained-operation cost at the worst common shape versus frozen PERF-11v4 `direct_7v2` total medians:

```text
small SHARED_PATH, frontier 8        projected 5.5 µs   = 11.5% of 48.4 µs budget    OK
SHARED_DEEP d16, frontier 32         projected 22.2 µs  = 10.5% of 211.0 µs budget   OK
SHARED_DEEP d128, frontier 128       projected 88.7 µs  =  0.09% of 101.6 ms budget  OK
realistic trace upper bound, F=200   projected 139 µs   = 11.7% of 1.18 ms budget    OK
```

**Decision: GO.** The direct-call floor does not consume the expected retained-operation budget (`§83` threshold met with ≥8× headroom). The measured per-constructor cost is dominated by real native semantic work (publish/allocation), not dispatch — dispatch itself is ~2–10 ns.

## 10. Deliverables

```text
PERF-12-baseline.md                              this record
packages/iyon-runtime/bench/PERF-12-memory-attribution.jsonl
packages/iyon-runtime/bench/PERF-12-ffi-floor.jsonl
packages/iyon-runtime/bench/perf12_memory_attribution.ts   (protocol parent)
packages/iyon-runtime/bench/perf12_memory_child.ts         (isolated block runner)
packages/iyon-runtime/bench/perf12_ffi_floor.ts            (floor probe)
```

No production architecture code changed. The archaeology confirms the handoff's fundamental assumptions (pending-recipe current backing, environment-owned semantic cache, identity-first Direct decoder, qualified Bun 1.4 direct-FFI floor); nothing contradicts proceeding to Tranche 2.
