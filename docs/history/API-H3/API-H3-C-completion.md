# API-H3 — H3-C completion

**Status:** COMPLETE
**Baseline:** H3-B `4b16bfe` (`feat: cut composition over to semantic views`)
**Scope:** Make structural transport consume semantic nodes directly on the retained hot path

## 1. Retained transport cutover

`packages/iyon-tui/src/transport/structural/retained-dag.ts` now keys all
transaction-local maps, cycle detection, NativeRef hints, derivation lookup,
root preparation, and exact-root rendering by `SemanticViewNode`.

The ordinary retained path is now:

```text
SemanticViewNode -> retained-dag materializer -> generated ABI calls
```

It does not call `nodeForBridge()`, construct a bridge tree, or read a cold
bridge representation. Exact-root hits still perform only the semantic brand
lookup, NativeRef hint lookup, and host render operation.

The materializers cover text, diff, spacer, row, column, grid, hanging,
container, clamp, contentMax, component, and decorated nodes. Wide axis/grid
sequence sidecars are read through their semantic read-only sequence interface;
wide initial roots may refuse at the existing cap, while axis/grid derivation
edits remain bounded and do not flatten the sequence.

## 2. Explicit physical encoding

Added:

```text
packages/iyon-tui/src/transport/structural/encoding.ts
```

This owns explicit mappings for semantic kinds, layout/grid tracks, axis edit
words, wrapping/alignment, diff metadata, overflow, scalar patch payloads,
size modes, and color atom strings. Semantic numeric values are never cast to
bridge/ABI values.

Styles are resolved directly from frozen semantic styles into the existing
generation-scoped native style table. Component materialization resolves the
semantic JavaScript `HandleId` to the live physical native `ComponentId` only
through the structural resolver.

## 3. Cold fallback

`cold-lowering.ts` remains the complete derived fallback:

```text
SemanticViewNode -> cold BridgeViewNode -> safe N-API decoder
```

It is weakly cached and now exposes only a transitional reverse association so
the generated legacy materializer remains type-correct during the stacked H3
migration. The reverse association is not used by ordinary retained
materialization. A cold allocation counter proves the hot path does not enter
this route.

## 4. Ownership enforcement

Added `h3c-structural-composition-seam` to `tools/ownership/check.ts`.
Structural transport has no imports resolving under `composition/**`; the
existing composition-to-transport gate remains active.

## 5. Focused coverage

```text
packages/iyon-tui/tests/tui_h3_c_transport.test.ts
```

Covers:

- retained semantic materialization with zero cold bridge allocations;
- wide semantic axis derivation without full sequence traversal;
- semantic component publication and `HandleId` resolution;
- complete cold fallback after retained refusal.

## 6. Counter comparison

Representative 30-iteration PERF-12 T15 cases were run at the H3-B baseline
and after H3-C. Structural counters were identical for every case:

| Workload / mode | Size | H3-B median | H3-C median | Counter result |
|---|---:|---:|---:|---|
| `plain_text_column` / `exact_identity` | 2,000 | 3,374 ns | 3,251 ns | identical; 0 inspected, 0 children, 30 hint hits |
| `plain_text_column` / `shared_path` | 2,000 | 250,958 ns | 263,166 ns | identical; 60 inspected, 60 children |
| `wide_axis` / `wide_axis_set` | 2,048 | 3,739,917 ns | 2,725,250 ns | identical; 30 derivation calls, 0 children |
| `wide_grid` / `wide_grid_cell` | 2,048 | 4,042,917 ns | 3,173,125 ns | identical; 30 derivation calls, 0 children |
| `decoration_heavy` / `decoration_patch` | 200 | 61,042 ns | 60,125 ns | identical; 30 derivation calls |
| `diff_heavy` / `rebuilt_equivalent` | 200 | 846,875 ns | 702,042 ns | identical; 6,030 inspected, 12,000 ref words |

Timing is smoke evidence only; work counters are the acceptance criterion.
The H3-C focused hot-path test reports
`cold_bridge_objects_allocated == 0`. The oversized embedded-NUL text case
refuses the retained lane and reports a nonzero cold allocation count.

## 7. Memory convergence

The 1,000-operation retained replacement memory probe matched the H3-B
baseline after explicit GC/native maintenance:

```text
semantic_cache_entries:       204
semantic_cache_live:           204
native_ref_slots:              204
leased_slots:                 1
unleased_live_slots:         203
node_ref_entries:             204
path_nodes:                    1
scavenge_queue:                0
semantic_cache_entries_removed: 1999
native_ref_expired_slots_removed: 1999
```

The cold bridge cache is weakly keyed. No permanent second retained JS DAG is
introduced by H3-C.

## 8. Native path disposition

Native path lineage and edit-transaction metadata remain under
`api/view/view.ts` for this tranche because the existing path transaction API
and tests still use them. They are transport-only metadata and are explicitly
scheduled for the H3-D native-path cleanup; no retained hot materializer uses a
bridge node or bridge path record.

## 9. Validation

| Gate | Result |
|---|---|
| `bun test packages/iyon-tui/tests/tui_h3_c_transport.test.ts` | PASS; 4 passed, 10 assertions |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | PASS; 75 passed, 502 assertions |
| `bun run typecheck` | PASS |
| `bun run check:ownership` | PASS, including H3-B and H3-C seam gates |
| `bun run check:tui-declarations` | PASS; 36 reachable declaration files |
| `bun run check:tui-abi` | PASS; generated ABI unchanged |
| `cargo test --workspace` | PASS; 735 Rust tests, plus public/integration suites |
| `git diff --check` | PASS |

No Rust source, public export, or structural ABI schema changed. The direct
FFI benchmark addon was not staged in this tranche; the default safe N-API
addon remained installed. Direct parity remains an H3-E final gate.

H3-D is the next stacked tranche: publication-contract cleanup, native-path
relocation, and deletion of migration-only bridge/component compatibility
modules.
