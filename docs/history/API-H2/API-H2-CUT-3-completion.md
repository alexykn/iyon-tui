# API-H2 / STRUCT-1 — CUT 3 completion

**Status:** COMPLETE
**TUI commit:** `758cef9c598e0cf075f68e685ab48669675443cd`
**Branch:** `api-h2-cut-1` (continued; no new branch)
**Purpose:** Establish the live-runtime, semantic-control, runtime-lifetime, and raw-native ownership seams without changing behavior, public API, or PERF-13.

## 1. Final `src/` tree

```text
packages/iyon-tui/src/
├── api/
│   ├── content/
│   ├── controls/
│   │   ├── framework-handle.ts
│   │   ├── history.ts
│   │   ├── scroll-pane.ts
│   │   ├── text-input.ts
│   │   ├── text-stream.ts
│   │   └── view-slot.ts
│   ├── errors.ts
│   ├── extensions/traits/
│   ├── presentation/
│   └── view/
├── composition/
│   └── [retained semantic composition modules]
├── runtime/
│   ├── handle-registry.ts
│   ├── runtime.ts
│   └── tui.ts
├── transport/
│   ├── abi/structural/
│   │   ├── schema/
│   │   └── generated/
│   ├── native/
│   │   ├── addon.ts
│   │   ├── factories.ts
│   │   └── resources.ts
│   └── structural/
│       └── [CUT 2 structural transport modules]
├── testing/
├── index.ts
└── types.ts
```

`types.ts` remains as the intentionally deferred mixed-contract residue for CUT 4; the public root remains curated by `index.ts`.

## 2. Removed root modules

These root paths no longer exist:

```text
component.ts
handle-registry.ts
handles.ts
history.ts
native-handles.ts
native.ts
runtime.ts
scroll-pane.ts
stream.ts
text-input.ts
tui.ts
```

Their responsibilities now have explicit owners:

| Former path | CUT 3 owner |
|---|---|
| `runtime.ts` | `runtime/runtime.ts` |
| `tui.ts` | `runtime/tui.ts` public re-export |
| `history.ts` | `api/controls/history.ts` |
| `text-input.ts` | `api/controls/text-input.ts` |
| `stream.ts` | `api/controls/text-stream.ts` |
| `scroll-pane.ts` | `api/controls/scroll-pane.ts` |
| `component.ts` | `api/controls/view-slot.ts` |
| `native.ts` | `transport/native/addon.ts` |
| `native-handles.ts` | `transport/native/factories.ts` |
| `handles.ts` | removed; component placement lookup lives in structural transport |
| `handle-registry.ts` | split between `runtime/handle-registry.ts` and `transport/native/resources.ts` |

## 3. Files split or extracted across ownership boundaries

- `FrameworkHandle` and `HandleId` were extracted from `types.ts` into `api/controls/framework-handle.ts`. The public class remains nominal and its declaration remains semantic.
- The former combined handle registry was split. Runtime owns local handle identity and lifecycle delegation; `transport/native/resources.ts` owns the raw handle-to-native-resource association and disposal primitive.
- The component-ID lookup formerly housed by `handles.ts` moved into `transport/structural/component-view.ts`, where component placement lowering already belongs.
- Addon loading, native contracts, and `requireNativeClass()` moved into `transport/native/addon.ts`; native constructor factories moved into `transport/native/factories.ts`.
- `runtime.ts` itself remained one cohesive live-host implementation after classification. It now imports composition, semantic controls, and transport seams instead of owning their source files.

No retained algorithm, lifecycle ordering, scheduling rule, ABI record, or public name was rewritten.

## 4. Final composition/runtime/control/native boundary

### `composition/`

Owns retained semantic execution, scope and state dependency tracking, child identity, builder-root ownership, and semantic reuse. It has no imports of live runtime, raw native, or semantic control implementation owners.

### `runtime/`

Owns `Tui`, live host orchestration, creation and teardown of retained execution instances, host-level lifecycle, and local framework-handle identity/lifecycle registration. Runtime uses composition and transport; composition does not own the host.

### `api/controls/`

Owns caller-facing `History`, `TextInput`, `TextStream`, `ViewSlot`, and `ScrollPane` semantics plus the nominal `FrameworkHandle` base. These controls retain their existing native delegation and composition behavior without exposing native contracts in public declarations.

### `transport/native/`

Owns addon loading, raw native contracts, constructor factories, and the private native-resource association. No runtime ownership is imported by this subsystem.

### `transport/structural/`

Continues to own structural component placement and the CUT 2 bridge/materialization machinery.

## 5. Structural ABI confirmation

The structural schema and generated View ABI remain under:

```text
transport/abi/structural/schema/
transport/abi/structural/generated/
```

Generated bindings now import the raw host contract from `transport/native/addon.ts`. Generator templates and all generated outputs were regenerated; no generated artifact was hand-authored. ABI version, semantic version, function count, and POD/layout signatures remain unchanged.

## 6. Enforcement updates

- Added `h2-cut3-ownership` to `tools/ownership/check.ts`. It verifies required owners, rejects all legacy root peers, prevents composition from importing runtime/native/control owners, prevents native transport from importing runtime ownership, verifies the split resource registry, checks addon ownership, and rejects private paths from the root barrel.
- Updated all H1/CUT 2 ownership checks to resolve the new paths.
- Extended declaration-closure private-module enforcement for `addon`, `factories`, and `resources`.
- Updated staging, tests, benchmarks, direct-FFI oracles, generator templates, and regenerated ABI outputs.
- Refreshed the frozen root source hash. The public surface remains 40 value exports and 74 type exports.

## 7. Validation evidence

| Gate | Result |
|---|---|
| TypeScript typecheck | PASS |
| Declaration closure | PASS; 27 reachable declaration files |
| ABI generator check | PASS |
| ABI generator tests | PASS; 27 passed |
| Ownership checks | PASS; 23 checks |
| Rust format | PASS |
| Rust Clippy | PASS |
| Rust workspace tests, all features | PASS; 903 passed, 0 failed, 3 ignored |
| Rust workspace tests, direct-FFI feature | PASS; 897 passed, 0 failed, 3 ignored |
| Default TUI/consumer Bun tests | PASS; 59 passed, 0 failed, 368 assertions |
| Direct-FFI functional Bun tests | PASS; 55 passed, 0 failed, 349 assertions |
| PERF-12 R6b smoke | PASS; 1,000 scopes, 50 warmups, 200 measured; median 105,583 ns |
| Representative T15 default/direct matrix | PASS; 27 cases per arm, 54 results, no correctness failures |

The direct-feature full TUI suite intentionally excludes the N-API-only ABI test file from the functional count: that file asserts that direct-FFI qualification symbols are absent, so it is not valid under the direct feature. The direct-FFI Rust suite and all direct functional behavior passed.

The representative T15 matrix matched screen output and structural deltas for every case. Its N-API/direct geometric-mean ratio was `1.0543`; timings remain smoke evidence rather than an adoption decision.

## 8. Iyon integration

The existing branch workflow consumed CUT 3 directly:

```text
bun run build:iyon -- api-h2-cut-1   PASS against 758cef9...
Iyon typecheck                       PASS
Iyon standalone build                PASS
Iyon full tests                      PASS; 281 passed, 0 failed, 733 assertions
```

The checked-in Iyon TUI pin was not changed. Integration used the existing ephemeral worktree only.

## 9. Deferred work

CUT 3 intentionally does not:

- eliminate the remaining mixed `types.ts` contract module;
- perform CUT 4 root cleanup or remove all remaining implementation residue;
- implement retained state or content transport planes;
- change public API names, lifecycle, scheduling, retained identity, or structural ABI semantics;
- add `transport/state/` or `transport/content/` implementations;
- modify `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`.

CUT 4 owns mixed-module elimination and root cleanup. CUT 5 owns final enforcement and integration consolidation. PERF-13 continues to own retained state/content transport design and implementation.

**CUT 3 decision: GO.** Live runtime, semantic controls, runtime lifetime, and raw native access now have explicit physical owners; composition no longer owns host/native/control implementations; all retained/native/parity gates pass; and the work remains on `api-h2-cut-1`.
