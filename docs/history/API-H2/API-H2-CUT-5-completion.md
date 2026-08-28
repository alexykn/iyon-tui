# API-H2 / STRUCT-1 — CUT 5 completion

**Status:** COMPLETE
**Implementation commit:** `eec10c3a6a16e1fc4399094d2c316f793f1ac20e`
**Branch:** `api-h2-cut-1` (continued; no per-cut branch)
**Purpose:** Finalize H2 enforcement and integration consolidation without changing behavior, public names, lifecycle, scheduling, retained identity, structural ABI semantics, or PERF-13 scope.

## 1. Final `src/` tree

```text
packages/iyon-tui/src/
├── api/
│   ├── content/
│   │   ├── annotations.ts
│   │   ├── diff.ts
│   │   ├── projection.ts
│   │   ├── projectors.ts
│   │   ├── stream-snapshot.ts
│   │   ├── text-content.ts
│   │   └── text.ts
│   ├── controls/
│   │   ├── framework-handle.ts
│   │   ├── history.ts
│   │   ├── output.ts
│   │   ├── scroll-pane.ts
│   │   ├── text-input.ts
│   │   ├── text-stream.ts
│   │   └── view-slot.ts
│   ├── errors.ts
│   ├── extensions/traits/
│   │   ├── component.ts
│   │   ├── projector.ts
│   │   ├── renderer.ts
│   │   ├── streaming-source.ts
│   │   ├── text-rewriter.ts
│   │   └── text-visitor.ts
│   ├── presentation/
│   │   ├── style.ts
│   │   ├── theme-key.ts
│   │   └── theme.ts
│   └── view/
│       ├── geometry.ts
│       ├── scene.ts
│       └── view.ts
├── composition/
│   ├── child-owner.ts
│   ├── compose.ts
│   ├── define-view.ts
│   ├── execution-context.ts
│   ├── execution.ts
│   ├── persistent-seq.ts
│   └── tracked-state.ts
├── runtime/
│   ├── access.ts
│   ├── events.ts
│   ├── handle-registry.ts
│   └── runtime.ts
├── testing/
│   └── index.ts
├── transport/
│   ├── abi/structural/{schema,generated}/
│   ├── native/
│   │   ├── addon.ts
│   │   ├── factories.ts
│   │   └── resources.ts
│   └── structural/
└── index.ts
```

`transport/state/`, `transport/content/`, `transport/abi/state/`, and `transport/abi/content/` do not exist. They remain future PERF-13 ownership slots only.

## 2. Deleted/moved residue

CUT 5 removed the final production-to-testing import by moving the private access registry:

```text
packages/iyon-tui/src/testing/access.ts
    → packages/iyon-tui/src/runtime/access.ts
```

The registry is now a private runtime seam (`registerRuntimeAccess` / `runtimeAccess`). `testing/index.ts` owns the public harness and consumes that seam; `runtime/runtime.ts` no longer imports any module under `testing/`.

The root and forwarding modules deleted in CUT 1–4 remain absent, including:

```text
component.ts
component-facade.ts
handle-registry.ts
handles.ts
history.ts
internal-composition.ts
native-handles.ts
native.ts
projectors.ts
retained_dag.ts
runtime.ts
scroll-pane.ts
stream.ts
style-internals.ts
testing.ts
testing-access.ts
text-input.ts
types.ts
tui.ts
view-internals.ts
```

`src/index.ts` remains the only root implementation file.

## 3. Files split or consolidated at the CUT 5 boundary

No semantic or native algorithm was split in CUT 5. The one runtime/testing seam was relocated and renamed:

- `testing/access.ts` → `runtime/access.ts`: runtime owns the private registration/lifetime seam; testing owns the harness API and inspection operations.
- `tools/ownership/check.ts`: consolidated the final H2 import, root publication, module identity, package publication, and future-plane enforcement into the existing ownership command.
- `tools/api-surface/check-declaration-closure.ts`: added an explicit public-declaration transport-path gate and generic transport-module rejection.
- `tsconfig.json`: removed the `@iyon/tui` TypeScript path alias; workspace package resolution remains canonical through the package export/workspace link.

No generated ABI artifact was hand-authored or regenerated because CUT 5 does not change the schema or generated output.

## 4. Final composition/runtime boundary

### Composition

`composition/` owns retained semantic evaluation, execution scopes, state dependency tracking, child occurrence ownership, keyed identity, builder roots, and semantic reuse. CUT 5 enforces that it cannot import live `runtime/`, `transport/native/`, or `testing/` modules. Its existing structural transport imports remain the PERF-12 lowering path, not host-lifecycle ownership.

The semantic `api/controls/framework-handle.ts` type seam remains the only intentional control contract visible to composition; composition does not import control implementations or runtime registries.

### Runtime

`runtime/` owns live `Tui` instances, host orchestration, runtime lifetime, event delivery, control creation/disposal, and the private runtime access seam used by the testing harness. It creates and drives composition instances; composition does not create or dispose live hosts.

### Testing

`testing/` contains only the public `AppHarness` entrypoint. Input injection, deterministic clock advancement, and screen/cell/style inspection remain harness capabilities, with their private runtime registration seam outside the production-to-testing dependency direction.

## 5. Final public-handle / runtime-lifetime / raw-native split

Unchanged from CUT 4 and now additionally enforced against root publication and declaration leakage:

- `api/controls/framework-handle.ts`: nominal semantic `FrameworkHandle`, `HandleId`, and component handle contract.
- `runtime/handle-registry.ts`: framework identity, registration, and lifecycle delegation.
- `transport/native/resources.ts`: raw native-resource association and disposal.
- `transport/native/addon.ts`: addon loading and native-only contracts.
- `transport/native/factories.ts`: native-backed constructor factories.
- `api/controls/*.ts`: public control semantics and implementation classes.

No public declaration or root export exposes the raw native/bridge/generated layers.

## 6. Structural schema/generated ABI confirmation

The structural schema and generated View ABI remain owned by:

```text
packages/iyon-tui/src/transport/abi/structural/schema/bridge-schema.json
packages/iyon-tui/src/transport/abi/structural/generated/
```

Generator input paths, staging paths, generated records, function signatures, POD layouts, transport semantics, and direct-FFI qualification behavior are unchanged. `check:tui-abi` and the ABI generator tests pass.

## 7. Enforcement added or strengthened

`tools/ownership/check.ts` now includes four CUT 5 gates:

1. **`h2-cut5-import-boundaries`**
   - production cannot import `testing/`;
   - composition cannot import live runtime/native/testing owners;
   - API modules cannot import generated ABI paths.
2. **`h2-cut5-root-publication`**
   - root exports are explicit;
   - wildcard exports are rejected;
   - only semantic API modules and the approved runtime/composition contracts are root-exportable;
   - `Bridge*`, `Native*`, transport, generated, and testing modules cannot be published.
3. **`h2-cut5-module-identity`**
   - rejects `baseUrl`/`paths` aliases in the framework configs;
   - rejects package self-imports and absolute local imports;
   - rejects alternate local specifier spellings for one target module.
4. **`h2-cut5-package-publication`**
   - exact package/workspace export maps are enforced;
   - external deep exports are rejected;
   - future state/content planes cannot be created or published.

The consumer gate now scans the complete fixture package for undocumented `@iyon/tui/*` deep imports. The declaration checker emits **`h2-cut5-public-declaration-boundary`** and rejects reachable public declaration imports into any `transport/` path, in addition to its existing private module/type checks.

## 8. Public surface and declaration results

- Frozen public surface: **40 value exports + 74 type exports**, unchanged.
- Declaration closure: **PASS**, 36 reachable declaration files.
- Public declaration transport boundary: **PASS**.
- Consumer fixture: **PASS**, root and `@iyon/tui/testing` only.
- Package maps: only `.`, `./testing`, and the existing tooling-only `./native-stage` entrypoint are published.

## 9. PERF-12 non-regression and parity

| Gate | Result |
|---|---|
| ABI generator tests | PASS; 27 passed |
| Rust workspace tests, all features | PASS; 903 passed, 0 failed, 3 ignored |
| Rust workspace tests, direct-FFI configuration | PASS; 897 passed, 0 failed, 3 ignored |
| Default TUI + consumer Bun tests | PASS; 59 passed, 0 failed, 368 assertions |
| Direct-FFI functional tests | PASS; 55 passed, 0 failed, 349 assertions |
| PERF-12 R6b smoke | PASS; 1,000 scopes, 50 warmups, 200 measured; median 105,958 ns, p95 121,667 ns, p99 167,958 ns |
| T15 authoritative matrix | PASS; 27 cases per arm, 54 results, no correctness failures |

The T15 matrix produced an overall N-API/direct geometric mean of **1.0692**. The representative structural parity case matched screens and deltas on both transports:

```text
bridge hint hits:                  20
bridge hint misses:                 0
semantic nodes inspected:          40
children visited:                  40
direct materializer calls:          40
host mutations:                    20
cold fallbacks:                     0
```

Direct-FFI staging was followed by a successful restore to the default N-API addon.

## 10. Iyon integration

Using the established ephemeral integration worktree:

```text
bun run build:iyon -- api-h2-cut-1   PASS against eec10c3...
Iyon typecheck                       PASS
Iyon standalone build                PASS
Iyon full tests                      PASS; 281 passed, 0 failed, 733 assertions
```

The checked-in Iyon TUI pin was not changed.

## 11. Deferred debt

CUT 5 intentionally does not:

- implement retained geometry, presentation, or interaction state;
- implement state/content FFI or any content scheduler;
- add `Source`, `Funnel`, `Connector`, or `ContentPort`;
- add `transport/state/`, `transport/content/`, or their ABI implementations;
- change `pushStream`/`sealStream` semantics;
- redesign View identity, composition, lifecycle, scheduling, or structural ABI;
- add capability fallback, buffered/hot content, Kitty/Sixel/video, or property-level reactive bindings;
- rename accepted H1 public APIs;
- change the checked-in Iyon dependency pin.

These remain PERF-13 or a separate API tranche. The existing synthetic shutdown-ordering and mock-auth observations are outside CUT 5 and were not changed.

**CUT 5 decision: GO.** H2 enforcement now covers semantic/API imports, public declarations, testing direction, composition/runtime direction, root publication, external deep imports, module identity, package exports, and future-plane publication. The branch is ready for the PERF-13 structural/state/content work without another TypeScript tree reorganization.
