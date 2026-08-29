# API-H3 — H3-E completion

**Status:** COMPLETE
**Baseline:** H3-D `81229fc` (`refactor: remove H3 migration compatibility paths`)
**Final implementation:** `fe05092` (`fix: harden H3 retained transport boundaries`), including `c807cd6` (`fix: preserve structural axis track encoding`)
**Final report commit:** the commit introducing this audit update
**Platform:** macOS arm64
**Bun:** `1.4.0` (`34cbb9a40`)
**Rust:** `1.97.1 (8bab26f4f 2026-07-14)`

## 1. Final architecture

The relevant final source tree is:

```text
packages/iyon-tui/src/
├── api/
│   ├── controls/
│   ├── presentation/
│   │   ├── semantic-style.ts
│   │   └── ...
│   └── view/
│       ├── geometry.ts
│       ├── scene.ts
│       ├── semantic-node.ts
│       └── view.ts
├── composition/
│   ├── child-owner.ts
│   ├── compose.ts
│   ├── define-view.ts
│   ├── execution-context.ts
│   ├── execution.ts
│   ├── persistent-seq.ts
│   ├── publication.ts
│   └── tracked-state.ts
├── runtime/
├── testing/
└── transport/
    ├── abi/structural/generated/
    ├── native/
    └── structural/
        ├── cold-lowering.ts
        ├── component-id.ts
        ├── encoding.ts
        ├── ir.ts
        ├── native-view-abi.ts
        ├── policy.ts
        ├── retained-dag.ts
        ├── retained-path.ts
        └── style-lowering.ts
```

The ownership proof is:

```text
api/view owns:       immutable SemanticViewNode data and semantic NodeId
composition owns:    semantic execution, scope identity, reuse, and publication protocol
runtime owns:        lifecycle and concrete target binding
transport owns:      ABI encoding, NativeRef hints, physical retention, and fallback lowering
```

Semantic styles are copied/frozen by `api/presentation/semantic-style.ts` and
semantic style records in `api/view/semantic-node.ts`. Semantic derivations and
wide sequence overrides are weak semantic sidecars. `PersistentSeq` is exposed
to transport only through the read-only `SemanticSequence` contract; transport
does not import the persistent-sequence implementation.

Component View nodes carry the JavaScript-local `HandleId`. The private
`transport/structural/component-id.ts` resolver maps that identity to native
`ComponentId` only while lowering.

Physical encoding is centralized in `transport/structural/encoding.ts`.
Bridge/ABI discriminants are never reused as semantic discriminants. In
particular, axis-create track words now use explicit physical codes
`contentMax=2`, `fixed=3`, `flex=4`, and `flexMax=5`, distinct from the
bridge layout-child codes. This corrected a H3-C defect where a retained flex
child was decoded as a fixed one-cell track.

The cold path is:

```text
SemanticViewNode -> cold-lowering.ts -> BridgeViewNode -> safe N-API decoder
```

The warm path is:

```text
SemanticViewNode -> retained-dag.ts -> encoding.ts -> generated N-API calls
```

The warm path does not allocate cold bridge objects. NativeRef hints,
transaction-local leases, stale recovery, path metadata, and ABI calls remain
private to structural transport.

## 2. Publication contract

Composition owns the protocol and exposes only semantic View values plus
prepare/commit/abort behavior:

```ts
export interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}

export interface StructuralPublicationTarget {
  preparePublication(output: View): PreparedStructuralPublication | undefined;
  needsPublication?(output: View): boolean;
}

export interface StructuralScopeProjection {
  readonly view: View;
  readonly target: StructuralPublicationTarget;
  dispose(): void;
}
```

Concrete private adapters are:

- the runtime root target, backed by `RetainedRootBoundary` and the History
  sideband;
- component-scope projections, backed by a Tui-created `ViewSlot`;
- `ViewSlot` builder roots, backed by `prepareSetView`;
- `ScrollPane` builder roots, backed by `prepareSetContent`.

The legacy non-transactional projection `install()` fallback is deleted.
Prepare refusal aborts the enclosing execution batch. Child-before-parent
staging, sideband publication, lease transfer, rollback, and deferred disposal
remain intact.

## 3. Deleted compatibility paths

H3 removed all migration-only compatibility machinery from framework
production code:

```text
packages/iyon-tui/src/transport/structural/view-bridge.ts
packages/iyon-tui/src/transport/structural/component-view.ts
```

Also removed or relocated:

- the bridge-to-semantic reverse association;
- bridge derivation and bridge sequence sidecars;
- bridge semantic construction helpers and hot-path bridge aliases;
- semantic construction from the former component transport helper;
- native path lineage and transaction metadata from `api/view/view.ts`;
- packed native path/transaction constructors from semantic API ownership;
- the execution-layer `ScopeProjection.install()` compatibility route;
- semantic records and semantic sidecars from structural `ir.ts`.

`component-view.ts` was replaced by the physical-only
`transport/structural/component-id.ts`. `ir.ts` is wire/bridge-only.
The direct-FFI benchmark has an independent benchmark-local bridge metadata
adapter; it does not restore production bridge sidecars.

## 4. Import and ownership scans

Final targeted source scans produced no output for all three forbidden
relationships:

```text
composition -> transport/structural: 0 matches
transport/structural -> composition: 0 matches
transport/structural -> runtime: 0 matches
api/view -> transport/structural or transport/abi: 0 matches
```

The final checker output included:

```text
PASS h3b-composition-transport-seam
PASS h3c-structural-composition-seam
PASS h3d-residual-cleanup
PASS h2-cut5-publication
PASS safe-napi-ts-boundary
PASS generated-napi-lowering
PASS ts-surface-snapshot — 40 value + 74 type exports
PASS rust-surface-snapshot — 1519 mapped Rust items
ALL OWNERSHIP CHECKS PASSED
```

The complete `bun run check:ownership` invocation passed every existing H1/H2
rule and all H3 seam rules.

## 5. ABI and public surface

```text
bun run check:tui-abi                 PASS; generated ABI unchanged
bun run typecheck                     PASS
bun run check:tui-declarations        PASS; 38 reachable declarations
```

The frozen public TypeScript surface is unchanged. No semantic internals,
NativeRef values, NodeIds, bridge records, ABI calls, or testing helpers were
added to `@iyon/tui` declarations. No structural ABI schema or generated
artifact changed.

## 6. Correctness and integration gates

```text
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
  PASS; 75 tests, 502 assertions

cargo test --workspace
  PASS; 735 Rust tests, 0 failures, 1 ignored
  plus public/integration/doc-test suites all passed

cargo test -p iyon-tui-native --features direct-ffi
  PASS; 37 library tests, 5 generated-ABI tests, 1 sync test, 1 ignored

cargo run -q -p tui-abi-gen -- check
  PASS

cargo test --workspace --features direct-ffi
  PASS; 735 Rust tests plus public/native/integration suites

git diff --check
  PASS
```

The default safe N-API Darwin arm64 addon was restored after direct-feature
checks. The direct-feature staging probe and default staging probe both passed.
No Rust source changed in H3, so Rust formatting and Clippy were not required
for this tranche.

External consumer workflow:

```text
bun run build:iyon -- api-h3-composition-transport-seam
  PASS; built against TUI @ c807cd658bbae650ebaf7ec325f4522ce8005b98

external worktree: bun run typecheck
  PASS

external worktree: bun test plugins/app/iyon/test
  PASS; 116 tests, 336 assertions
```

The external build used the documented public package entrypoints and did not
require a deep-import or application-specific framework compatibility change.

## 7. PERF-12 retained non-regression and parity

The authoritative T15 matrix was run once against source `c807cd6`, before
this audit's TypeScript-only hardening:

```text
311 cases per arm
622 result records
correctness failures: []
N-API/direct structural deltas: equal for every case
first rendered screen row: equal for every case
overall geometric mean (N-API median / direct median): 0.6546
```

This audit intentionally did **not** rerun the full PERF-12 matrix. The T15
runner is now stricter: each future candidate record carries the complete
rendered `screen_rows` snapshot and the measured semantic NodeId creation
count, and comparison rejects either mismatch in addition to structural
counter differences. A focused post-audit parity smoke covered 48 cases per
arm (96 result records) and reported zero correctness failures.

A value below `1.0` means the generated safe N-API arm was faster than the
feature-gated direct-FFI oracle for that run. Timings are smoke evidence; the
structural equality, full-screen equality, semantic-NodeId, and route/counter
invariants are the acceptance gate.

Representative N-API records from that matrix:

| Case | Median | Structural result |
|---|---:|---|
| exact root reuse, `plain_text_column/2000` | 751 ns | 10,000 hint hits; 0 inspected; 0 cold fallbacks |
| small text change, `plain_text_column/2000` | 218,041 ns | 2,000 nodes/children inspected; 1,000 host mutations |
| scalar/decorative patch, `decoration_heavy/200` | 37,625 ns | 10,000 derivation calls; stale recovery remains bounded and matched direct |
| wide axis set, `wide_axis/2048` | 2,401,375 ns | 1,000 derivation calls; 0 children visited |
| wide axis splice, `wide_axis/2048` | 2,472,667 ns | 1,000 derivation calls; 2,000 ref words; no flattening |
| wide grid cell, `wide_grid/2048` | 2,970,709 ns | 1,000 derivation calls; 0 children visited |
| cold materialization, `diff_heavy/rebuilt_equivalent/200` | 500,667 ns | 201,000 semantic nodes and 400,000 ref words; 1,000 expected fallback routes |
| stale-ref recovery, `decoration_heavy/decoration_patch/200` | 37,625 ns | 10,000 bounded derivation/recovery operations; direct matched exactly |

The focused multi-edit arm also passed semantic/rendered parity:

| Transaction | N-API median | Direct median | Result |
|---|---:|---:|---|
| 2 edits | 24,709 ns | 24,000 ns | equal rendered result |
| 8 edits | 53,583 ns | 51,333 ns | equal rendered result |
| 32 edits | 189,834 ns | 184,458 ns | equal rendered result |
| 64 edits | 360,500 ns | 349,584 ns | equal rendered result |

The route counters preserve the H3-C work profile: exact roots stop at the
semantic hint, small mutations stop at the changed frontier, wide edits use
semantic sequence derivations, and the warm retained path reports zero cold
bridge allocations. The H3-C representative counters remain equal after the
H3-D cleanup and the axis-encoding correction.

### Post-sign-off audit corrections

The handoff audit found and corrected four real shortcuts without changing the
public surface or ABI:

- `Tui.render(scene)` now validates the semantic root without eagerly building
  a cold bridge before the retained exact-root/frontier route. Cold lowering is
  deferred to the actual fallback; the no-session compatibility path still
  preflights before a History transfer.
- `tryNativeMaterialize` now attempts NodeId promotion before constructing a
  cold bridge, avoiding unnecessary whole-tree lowering for an already-retained
  physical root.
- Remaining retained decoration, style, diff, grid-cell, coordinate, and axis
  numeric packing is centralized in `transport/structural/encoding.ts` rather
  than being duplicated in the retained-DAG walker.
- The ownership checker now rejects future `transport/structural -> runtime`
  imports, in addition to the composition and semantic-layer boundaries.

These changes were checked by the focused 48-case-per-arm T15 smoke and the
framework suites above. No full PERF-12 benchmark was rerun.

## 8. Memory convergence

The one-million-replacement memory gate was run for both transports. The final
native snapshots were identical:

```text
alive:                         true
builders:                      0
edit_txns:                     0
generation:                    1
leased_slots:                  1
native_ref_slots:              204
unleased_live_slots:           203
node_ref_entries:              204
path_keys:                     0
path_nodes:                    1
scavenge_queue:                0
semantic_cache_entries:        204
semantic_cache_live:           204
semantic_cache_entries_removed:1999999
native_ref_expired_slots_removed: 1999999
native_ref_pages:              2
native_ref_pages_freed:        487
semantic_cache_full_sweeps:   490
```

There is no permanent duplicate semantic/bridge retained DAG. Native leases and
weak semantic/native metadata converge for both the safe N-API and direct-FFI
arms.

## 9. Finished-tool spacing diagnosis

A focused external fixture trace with six finished tool cards reproduced a
three-row gap before the composer at small viewport heights. The same rows
were reproduced against TUI `main` at `1539afd`; this is not an H3 regression
and is not caused by the H3 semantic transport path.

The gap is the existing generic History native-frontier underflow policy:
when physical History rows have already entered terminal scrollback,
`crates/iyon-tui/src/history/projection/mod.rs` deliberately places remaining
capacity after the resident live suffix. The behavior was introduced by the
native-frontier work in `062d0dc` and is covered by existing
`stream_growth_consumes_bottom_slack*` tests. It keeps later History growth in
bottom slack without moving the native frontier.

H3 leaves this behavior unchanged to preserve generic native scrollback
semantics. Changing the finished-suffix alignment is a separate History UX
policy decision, not an H3-E fix; no full PERF-12 rerun is required for that
follow-up.

## 10. Remaining debt

### PERF-13 state plane

Generic state transport/publication remains deferred. H3's structural
publication contract is intentionally not a state transport contract.

### PERF-13 content plane

Content/source transport, stream handoff, and `pushStream`/`sealStream`
redesign remain deferred to `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`.

### Future structural optimization

Only profiling-driven structural changes remain. H3 does not add a graph-delta
language, mutable retained geometry, or per-property binding compiler.

### Unrelated public API debt / follow-up

The native-frontier bottom-slack visual policy is unchanged and requires a
separate generic History or application-layout decision. It is not a temporary
H3 bridge alias or publication TODO.
