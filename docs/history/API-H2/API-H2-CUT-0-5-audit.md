# API-H2 / STRUCT-1 — CUT 0–5 implementation audit

**Status:** COMPLETE — no unresolved H2 or T13.1 audit defects remain
**Audit commit:** `d39802af57d31a6dd6d5c99eca8495f37c517604`
**Branch:** `api-h2-cut-1`
**Reviewed against:**

- `API-H2-STRUCT-1-HANDOFF-v2.md` (read in full; H2 defines CUT 0–5 only)
- `docs/history/PERF-12/PERF-12-T13.1-retained-view-composition-handoff.md` (read in full)
- `docs/history/PERF-12/PERF-12-T13.1-AMENDMENT-C-optimal-retained-dag-execution.md` (read in full)

This was a corrective review, not a new H2 segment. No PERF-13 work was started.

## 1. Audit conclusion

The H2 source moves and CUT 5 enforcement match the handoff. The public framework surface remains unchanged at **40 value exports + 74 type exports**. Structural ABI records, generated output semantics, native lifecycle, retained execution identity, and transport selection remain unchanged.

The review did find several evidence/maintenance shortcuts and one real downstream retained-frontier cost. All were corrected in the audit commit and revalidated:

1. the current `ARCHITECTURE.md` still described the pre-H2 `src/native.ts` layout;
2. the external §31.1 fixture claimed `App=0` but did not count App executions;
3. the fixture had no committed 1,000-public-scope execution-frontier proof;
4. the stream isolation test did not verify retained revision or final host content;
5. the R6b same-geometry path recomputed the complete component-geometry map and cloned clean host indexes on every local update.

No correctness, API, ABI, ownership, or performance issue remains unaddressed within the reviewed scope.

## 2. H2 architecture review

### CUT 0 baseline

The frozen H1/S0 public surface, declaration closure, ABI, native staging, consumer fixture, retained tests, and Iyon integration were recorded before movement. The baseline explicitly contains no PERF-13 state/content implementation.

### CUT 1–2 ownership moves

The final tree has explicit owners:

```text
api/                         caller-facing semantic values/contracts
composition/                 retained semantic execution and identity
runtime/                     live Tui instances, orchestration, lifetime
transport/structural/        bridge IR, lowering, materialization, retained transport
transport/abi/structural/    structural schema and generated ABI
transport/native/            addon loading and raw native contracts
 testing/                    public harness facade
```

The former root `values/`, `traits/`, `generated/`, bridge, and error modules are gone. Structural generated ABI remains under `transport/abi/structural/`; no universal `transport/abi/generated/` bucket was introduced.

`api/view/view.ts` still uses the existing private structural backing required by the immutable PERF-12 `View -> BridgeViewNode` implementation. It does not export generated ABI records or expose transport paths in reachable public declarations. Moving this backing now would be a transport/semantic redesign, not a physical H2 move, and was intentionally not introduced.

### CUT 3–4 ownership moves

Controls, live runtime, runtime handle identity, raw native resources, and root cleanup are separated:

```text
api/controls/framework-handle.ts  semantic handle contract
runtime/handle-registry.ts        JS handle identity/lifetime
transport/native/resources.ts     raw resource association/disposal
transport/native/addon.ts         addon/native-only contracts
runtime/runtime.ts                live host and composition-runtime ownership
```

`types.ts`, `runtime/tui.ts`, `composition/internal-composition.ts`, all former root implementation peers, and temporary forwarding modules are absent. `testing/` no longer serves as a production dependency directory.

### CUT 5 enforcement

The existing ownership command now enforces:

- production cannot import `testing/`;
- composition cannot import live runtime/native/testing owners;
- semantic API modules cannot import generated ABI paths;
- root exports are explicit and cannot publish bridge/native/generated/testing modules;
- package exports are limited to `.`, `./testing`, and the existing tooling-only `./native-stage` entrypoint;
- `baseUrl`/`paths` aliases, self-package imports, absolute local imports, and duplicate local spellings are rejected;
- future `transport/state`, `transport/content`, `transport/abi/state`, and `transport/abi/content` paths cannot be created or published;
- the entire external fixture package is checked for undocumented TUI deep imports;
- reachable public declarations cannot import transport paths.

The current ownership run reports **28 passing checks**.

## 3. Corrective findings and fixes

### 3.1 Current architecture documentation

`ARCHITECTURE.md` referenced the removed `packages/iyon-tui/src/native.ts` seam and did not describe the final `api/` / `composition/` / `runtime/` / `transport/` / `testing/` tree. It now reflects the actual H2 ownership model and lists ABI, declaration, and ownership checks.

Historical reports, S0 inventories, and the explicitly protected `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md` were not rewritten.

### 3.2 §31.1 execution-frontier evidence

The fixture test name stated `App=0`, but `buildScopedConsumer()` did not expose an App execution counter. It now counts App body executions and asserts:

```text
initial render:       App=1, Header=1, Card A=1, Card B=1
Header-local State:   App=1, Header=2, Card A=1, Card B=1
```

The keyed parent-update test now also proves the necessary parent-driven behavior:

```text
reorder:              App +1, existing keyed card bodies +0
one keyed prop change: App +1, changed card +1, other card +0
```

The test uses only `@iyon/tui` and `@iyon/tui/testing`.

### 3.3 1,000-scope execution-frontier evidence

The external fixture now includes 1,000 sibling public `defineView` scopes, each reading its own `State<string>`. It asserts both variants:

```text
same-geometry update:     App +0, changed child +1, clean children +0
geometry-changing update: App +0, changed child +1, clean children +0
```

The native R6b differential test remains responsible for layout/paint propagation and cold-frame parity.

### 3.4 Stream isolation evidence

The 10,000-append test now verifies all relevant retained-content facts:

- no App/Header/Card execution caused by stream appends;
- stream revision advances;
- the concatenated retained History rows plus visible tail equal every expected line exactly;
- the stream remains unsealed until the explicit seal;
- the final stream snapshot is sealed after `History.sealStream()`.

The test no longer uses `as never` casts for the public History/TextStream contract.

### 3.5 R6b hidden full-tree work

The original local same-geometry path performed full-tree work that was not visible in the resolver/measure/paint counters:

```text
ResolvedSceneLayout::patch_component_with_cache
    -> LayoutTree::component_geometry()          # full layout-tree traversal

SceneHost::resolve_stable_at_with_anchor
    -> MountGraph::clone()                      # all mounted entries
    -> MountedCapabilities::clone()             # all capability entries

FocusState::reconcile_incremental
    -> ComponentGeometryMap::clone()            # all geometry entries
```

That contradicted T13.1 Amendment C §§5.4–5.6 and §20.5: a same-geometry local update must not rediscover clean siblings downstream.

The correction is narrow and preserves the existing algorithms:

- `LayoutTree::patch_component_geometry()` derives ancestor clip/viewport context in O(depth) and refreshes only the changed component subtree;
- `SceneHost` updates only affected graph revisions and capability entries for topology-preserving candidates, retaining full replacement only for root/topology-changing candidates;
- `FocusState::update_geometry_incremental()` updates only affected geometry entries and retains full cloning for full reconciliation;
- `ComponentGeometryNodesVisited` instrumentation and the R6b assertion prevent regression.

No semantic View, mount topology, ABI, public contract, or transport behavior was changed.

## 4. H2 and T13.1 validation

### Static and ownership gates

```text
bun install --frozen-lockfile       PASS
bun run check:tui-abi               PASS
bun run typecheck                   PASS
bun run check:tui-declarations      PASS; 36 reachable declarations
bun run check:ownership              PASS; 28 checks
cargo fmt --all -- --check          PASS
cargo clippy ... -D warnings        PASS
cargo test -p tui-abi-gen           PASS; 27 passed
```

### Retained/native tests

```text
Rust workspace, all features       PASS; 903 passed, 0 failed, 3 ignored
Rust workspace, direct-FFI config   PASS; 897 passed, 0 failed, 3 ignored
Default TUI + fixture Bun tests    PASS; 60 passed, 384 assertions
Direct functional Bun tests        PASS; 56 passed, 365 assertions
```

The direct functional count excludes the intentionally N-API-only generated-session test file; that file deliberately fails when direct qualification symbols are present. The direct Rust suite and remaining direct functional suite pass.

### R6b same-geometry and geometry-change smoke

Current default N-API artifact, 1,000 scopes, 50 warmups, 200 measured updates:

```text
same_geometry:   median 27,625 ns; p95 34,333 ns; p99 45,709 ns
geometry_change: median 29,416 ns; p95 54,042 ns; p99 142,500 ns
```

With the opt-in perf-counter artifact, the same-geometry run reports:

```text
resolver_nodes_visited:          200
measure_node_calls:              200
component_geometry_nodes_visited: 400   # 2 per update, not 1,000
prepare_node_calls:              200
paint_nodes_visited:              400
surface_cells_composited:       1,200
```

The corrected path therefore has bounded component-geometry work and does not perform a full clean-sibling geometry-map traversal.

### Structural N-API/direct parity

The representative T15 smoke matched screen output and structural deltas:

```text
default median: 79,749 ns
 direct median: 72,958 ns
structural output: equal
screen output:     equal
```

The current authoritative reduced matrix also passed:

```text
27 cases per arm, 54 total results, no correctness failures
overall N-API/direct geometric mean: 1.0503
```

All temporary benchmark artifacts were removed and default N-API staging was restored.

### Public module identity

The package workspace link and direct source import resolve to the same `View` constructor (`sameView: true`). Removing the root TypeScript path alias did not break typecheck, fixture tests, or Iyon integration.

## 5. Iyon integration

The established ephemeral worktree workflow consumed the corrected branch directly:

```text
bun run build:iyon -- api-h2-cut-1   PASS at d39802a...
Iyon typecheck                       PASS
Iyon standalone build                PASS
Iyon full suite                      PASS; 281 tests, 733 assertions
```

The checked-in Iyon TUI pin was not changed. No application concepts or product behavior were added to `iyon-tui`.

## 6. Explicitly deferred work

The audit did not start a new architecture segment and did not implement PERF-13. The following remain deliberately deferred:

- retained geometry/presentation/interaction state planes;
- `Source`, `Funnel`, `Connector`, `ContentPort`;
- `transport/state/` or `transport/content/` implementations;
- state/content FFI or a content scheduler;
- buffered/hot connector semantics and capability fallback;
- Kitty/Sixel/video/live surfaces;
- property-level reactive bindings;
- `pushStream`/`sealStream` redesign under `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`;
- H1 semantic naming follow-up;
- historical documentation/inventory rewrites outside current architecture docs.

**Audit decision: GO.** CUT 0–5 conform to the API-H2 source-ownership specification, and the corrected implementation now has direct public 1,000-scope execution evidence plus a counter-proven R6b same-geometry downstream frontier. The branch is clean, synchronized with `origin`, and remains ready for the separately authorized next phase without hidden H2 residue.
