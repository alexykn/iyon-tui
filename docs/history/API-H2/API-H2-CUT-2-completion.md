# API-H2 / STRUCT-1 — CUT 2 completion

**Status:** COMPLETE
**TUI commit:** `17540862d3eda0f099ccec04544c34d5dcfb5b08`
**Branch:** `api-h2-cut-1` (continued; no new branch)
**Purpose:** Establish the retained semantic composition and structural transport ownership boundaries without changing behavior or PERF-13.

## 1. Final CUT 2 source shape

```text
packages/iyon-tui/src/
├── api/
│   ├── content/
│   ├── errors.ts
│   ├── extensions/traits/
│   ├── presentation/
│   └── view/
├── composition/
│   ├── child-owner.ts
│   ├── compose.ts
│   ├── define-view.ts
│   ├── execution-context.ts
│   ├── execution.ts
│   ├── internal-composition.ts
│   ├── persistent-seq.ts
│   └── tracked-state.ts
├── testing/
├── transport/
│   ├── abi/structural/
│   │   ├── generated/
│   │   └── schema/
│   └── structural/
│       ├── component-view.ts
│       ├── ir.ts
│       ├── native-view-abi.ts
│       ├── policy.ts
│       ├── retained-dag.ts
│       ├── style-lowering.ts
│       └── view-bridge.ts
└── [remaining live runtime/control/native modules and index.ts]
```

The following former root peers were removed:

```text
child-owner.ts
compose.ts
define-view.ts
execution-context.ts
execution.ts
internal-composition.ts
persistent_seq.ts
tracked-state.ts
component-facade.ts
ir.ts
native_view_abi.ts
native_view_policy.ts
retained_dag.ts
style-internals.ts
view-internals.ts
```

## 2. Boundary decisions

- `composition/` owns retained semantic evaluation, execution scopes, state dependency tracking, child identity, semantic slots, builder-root ownership, and `PersistentSeq`.
- `transport/structural/` owns bridge IR, retained DAG/native-ref correspondence, structural ABI session calls, materialization policy, style lowering, View-to-bridge association, and component placement lowering.
- `componentIdForPlacement()` keeps native component identity lookup inside structural placement lowering rather than making composition depend directly on the handle registry.
- `runtime.ts`, controls, raw addon loading, and runtime/native handle lifetime remain at the root for the coordinated CUT 3 move. No live host lifecycle was moved into composition.
- `api/view/view.ts` continues to use private structural backing internally, but no generated ABI module is imported by `api/**` and no transport type is part of the public declaration surface.
- Algorithms, retained identity, scheduling, leases, and structural ABI records were not rewritten.

## 3. Tooling/seam updates

- Updated all source, test, benchmark, direct-FFI oracle, generated-materializer, and generator imports.
- The ABI generator now emits materializer imports from `transport/structural/`; outputs were regenerated rather than hand-edited.
- Updated the generator snapshot and generated fingerprints. Schema version, ABI version, semantic version, function count, and ABI layouts remain unchanged.
- Updated declaration-closure private-module detection for the new structural names.
- Added `h2-cut2-ownership` to `tools/ownership/check.ts`. It verifies target owners exist, legacy root peers are absent, composition does not import live runtime/native owners, and `api/**` does not import generated ABI directly.
- Refreshed the frozen public-root source hash in `docs/repository-separation/s0/api-surface.json`; the public export set remains 40 values and 74 types.

## 4. Validation evidence

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test -p tui-abi-gen` | PASS; 27 passed, 0 failed |
| `bun run check:tui-abi` | PASS |
| `bun run typecheck` | PASS |
| `bun run check:tui-declarations` | PASS; 25 declaration files reachable |
| `bun run check:ownership` | PASS; 22 checks |
| Default TUI/consumer Bun tests | PASS; 59 passed, 0 failed, 368 assertions |
| `cargo test --workspace --all-features` | PASS; 903 passed, 0 failed, 3 ignored |
| `cargo test --workspace --features direct-ffi` | PASS; 897 passed, 0 failed, 3 ignored |
| Direct-FFI functional Bun suite | PASS; 55 passed, 0 failed, 349 assertions |
| PERF-12 R6b frontier smoke | PASS; 1,000 scopes, 50 warmups, 200 measured samples |
| T15 default/direct structural smoke | PASS; screen and structural deltas matched |

The default/direct 20-sample structural smoke reported identical retained deltas: 20 bridge-hint hits, 40 semantic nodes inspected, 40 children visited, 40 materializer calls, 20 host mutations, and zero cold fallbacks per arm. Timing was retained as smoke evidence only, not as an adoption decision.

## 5. Iyon integration

The existing branch workflow consumed the committed CUT 2 revision directly:

```text
bun run build:iyon -- api-h2-cut-1       PASS at 1754086...
bun run typecheck in branch worktree    PASS
bun run build:standalone                PASS
bun test in branch worktree             PASS; 281 passed, 0 failed, 733 assertions
```

The checked-in Iyon TUI pin was not changed. The external build used its ephemeral integration worktree only.

## 6. Deferred work

CUT 2 intentionally does not:

- move or split the live runtime, controls, raw native loader, or native lifetime registry;
- eliminate `types.ts` or `style-internals.ts`-era mixed concepts beyond the structural lowering move;
- implement retained state/content transport planes;
- change public names, lifecycle, scheduling, retained identity, or ABI semantics;
- modify `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`.

CUT 3 owns the runtime/native/control seam. CUT 4 owns remaining root cleanup and mixed-module elimination. CUT 5 owns final enforcement and integration consolidation.

**CUT 2 decision: GO.** Composition and structural transport now have explicit physical owners, the legacy root peers are gone, all retained/native/parity gates pass, and the work remains on the existing `api-h2-cut-1` branch.
