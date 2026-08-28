# API-H2 / STRUCT-1 — CUT 4 completion

**Status:** COMPLETE
**TUI implementation commit:** `f5ce6c3392056746cfd025f41b1298dbc7bb1d2d`
**Branch:** `api-h2-cut-1` (continued; no per-cut branch)
**Purpose:** Remove the remaining root contract/forwarding residue and give the previously mixed public types explicit domain owners without changing behavior, public API, retained identity, or PERF-13.

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
│   ├── events.ts
│   ├── handle-registry.ts
│   └── runtime.ts
├── testing/
│   ├── access.ts
│   └── index.ts
├── transport/
│   ├── abi/structural/{schema,generated}/
│   ├── native/
│   │   ├── addon.ts
│   │   ├── factories.ts
│   │   └── resources.ts
│   └── structural/
├── index.ts
└── [no other root implementation files]
```

No `transport/state/` or `transport/content/` placeholder was created; both remain PERF-13 ownership slots only.

## 2. Deleted root and forwarding modules

CUT 4 deleted the final root/mixed residue:

```text
packages/iyon-tui/src/types.ts
packages/iyon-tui/src/runtime/tui.ts
packages/iyon-tui/src/composition/internal-composition.ts
```

The broader legacy root set from CUT 1–3 remains absent:

```text
component.ts
component-facade.ts
handle-registry.ts
handles.ts
history.ts
native-handles.ts
native.ts
runtime.ts
scroll-pane.ts
stream.ts
style-internals.ts
text-input.ts
tui.ts
view-internals.ts
```

`src/index.ts` now directly exports `Tui` from `runtime/runtime.ts`; there is no forwarding `runtime/tui.ts`. The former `internal-composition.ts` was only a composition re-export facade, so its callers use the owning composition module directly. `projectors.ts` is now under `api/content/` and is not a root peer.

## 3. Contract ownership moves

The former mixed `types.ts` declarations were moved to their domain owners:

| Contract family | Owner |
|---|---|
| `InsetsValue`, layout/grid vocabulary, `ViewChildren` | `api/view/geometry.ts`, `api/view/view.ts` |
| `SceneProducer` and scene shape | `api/view/scene.ts` |
| colors and theme color references | `api/presentation/theme.ts` |
| borders, text attributes, style values/selectors | `api/presentation/style.ts` |
| text roles/parts/selectors/spans | `api/content/text.ts` |
| stream annotations and snapshots | `api/content/stream-snapshot.ts` |
| text content format/origin | `api/content/text-content.ts` |
| projector/renderer/visitor/rewriter/source traits | `api/extensions/traits/` |
| `ComponentId`, component events, capabilities, adapter context | `api/extensions/traits/component.ts` |
| nominal framework/component handles | `api/controls/framework-handle.ts` |
| typed `Output<T>` channel identity | `api/controls/output.ts` |
| History, TextInput, TextStream, ViewSlot, ScrollPane contracts/options | their owning `api/controls/*.ts` modules |
| runtime events and `TuiRuntime` options/contract | `runtime/events.ts`, `runtime/runtime.ts` |
| `ViewComponent` | `composition/define-view.ts` |
| testing harness contract | `testing/index.ts` |

Control implementation classes keep their existing runtime names and construction/lifecycle behavior while their semantic contracts are declared beside the owning control. The root public type/value surface remains unchanged.

## 4. Final composition/runtime boundary

### `composition/`

Owns retained semantic evaluation: `defineView`, execution scopes, child ownership, dependency tracking, semantic slots, keyed identity, and retained reuse. It no longer contains the forwarding facade or live-host modules. The only control-related dependency is the nominal semantic `ComponentHandle` contract needed by component composition; it does not import control implementations, lifecycle registries, or native transport.

### `runtime/`

Owns live `Tui` host orchestration, runtime instance creation/teardown, event delivery, host-level rendering, and runtime contracts. `runtime/events.ts` owns the routed output/termination union. `runtime/runtime.ts` owns the `TuiRuntime`/open/size contracts together with their live implementation because those contracts describe the live runtime boundary rather than a generic shared type bag.

### `api/`

Owns caller-facing semantic values, traits, controls, and framework errors. Public controls delegate through private runtime/native seams but expose only semantic contracts.

## 5. Final public-handle / lifetime / raw-native split

- `api/controls/framework-handle.ts` owns the nominal `FrameworkHandle` and `HandleId` contract plus semantic component-handle shape.
- `runtime/handle-registry.ts` owns framework-handle identity and runtime disposal delegation.
- `transport/native/resources.ts` owns the private raw handle-to-native-resource association and native-resource disposal.
- `transport/native/addon.ts` owns addon loading and raw native contracts.
- `transport/native/factories.ts` owns native-backed constructor factories.
- Individual controls in `api/controls/` own public semantics and use only narrow private seams.

No single class is simultaneously the public semantic handle, runtime registry, and raw addon wrapper.

## 6. Structural schema/generated ABI confirmation

The existing structural schema and generated View ABI remain at:

```text
transport/abi/structural/schema/bridge-schema.json
transport/abi/structural/generated/
```

No structural ABI record, function, version, POD/layout signature, materialization rule, or generated output semantics changed. The generator continues to pass its canonical check; only TypeScript source imports and declaration ownership changed.

## 7. Enforcement updates

- Added `h2-cut4-root-cleanup` to `tools/ownership/check.ts`. It requires `src/index.ts` to be the only root implementation file, rejects all eliminated residue, rejects legacy `types.ts` imports, rejects escape-hatch directories, verifies explicit owners for moved contract families, and rejects root forwarding to eliminated modules.
- Updated CUT 2/CUT 3 ownership checks for the removed forwarding modules and the semantic handle-contract exception used by composition.
- Repointed H1 semantic/parity checks from `types.ts` to the owning API, control, runtime, and trait modules.
- Added eliminated module names to declaration-closure private-module enforcement.
- Refreshed the frozen root source hash in `docs/repository-separation/s0/api-surface.json` deliberately; the public surface remains 40 value exports and 74 type exports.

## 8. Validation evidence

| Gate | Result |
|---|---|
| TypeScript typecheck | PASS |
| Declaration closure | PASS; 36 reachable declaration files |
| ABI generator check | PASS |
| ABI generator tests | PASS; 27 passed |
| Ownership checks | PASS; all 23 checks, including `h2-cut4-root-cleanup` |
| Rust format | PASS |
| Rust Clippy | PASS |
| Rust workspace tests, all features | PASS; 903 passed, 0 failed, 3 ignored |
| Rust workspace tests, direct-FFI feature | PASS; 897 passed, 0 failed, 3 ignored |
| Default TUI + consumer Bun tests | PASS; 59 passed, 0 failed, 368 assertions |
| Direct-FFI functional Bun tests | PASS; 55 passed, 0 failed, 349 assertions |
| PERF-12 R6b smoke | PASS; 1,000 scopes, 50 warmups, 200 measured; median 110,166 ns |
| Representative T15 default/direct matrix | PASS; 27 cases per arm, 54 results, no correctness failures |

The direct functional count intentionally excludes the N-API-only generated-session test file; the direct Rust suite and all remaining direct functional tests passed.

The representative T15 structural parity case matched output and deltas on both transports:

```text
bridge hint hits:                  20
semantic nodes inspected:          40
children visited:                  40
direct materializer calls:          40
host mutations:                    20
cold fallbacks:                     0
```

## 9. Iyon integration

The existing branch workflow consumed the CUT 4 branch commit directly:

```text
bun run build:iyon -- api-h2-cut-1   PASS against f5ce6c3...
Iyon typecheck                       PASS
Iyon standalone build                PASS
Iyon full tests                      PASS; 281 passed, 0 failed, 733 assertions
```

The checked-in Iyon TUI pin was not changed. Integration used the existing ephemeral worktree only.

## 10. Deferred work

CUT 4 intentionally does not:

- implement retained geometry, presentation, or interaction state;
- implement `Source`, `Funnel`, `Connector`, or `ContentPort`;
- implement state/content FFI, content scheduling, or capability fallback;
- change `pushStream`/`sealStream` lifecycle semantics;
- change public names, lifecycle, scheduling, retained identity, or structural ABI semantics;
- reorganize the tree for PERF-13 beyond establishing the current structural ownership boundary;
- modify `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`.

CUT 5 remains for final enforcement/integration consolidation. PERF-13 remains the owner of future state/content transport design.

**CUT 4 decision: GO.** `src/` has only the curated root barrel; all mixed contracts have explicit domain owners; forwarding and ambiguous internal modules are gone; the public surface, declarations, retained behavior, native parity, performance smoke, and Iyon integration remain valid.
