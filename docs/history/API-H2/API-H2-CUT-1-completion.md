# API-H2 / STRUCT-1 — CUT 1 completion

**Status:** COMPLETE
**TUI commit:** `e7253757edf2b21a73cbb3078f668316b8ce4bd8`
**Remote branch:** `origin/api-h2-cut-1`
**Purpose:** Apply the low-ambiguity ownership moves from H2 CUT 1 without changing public semantics or runtime behavior.

## 1. Final CUT 1 tree

```text
packages/iyon-tui/src/
├── api/
│   ├── errors.ts
│   ├── content/
│   │   ├── annotations.ts
│   │   ├── diff.ts
│   │   ├── projection.ts
│   │   ├── projectors.ts
│   │   ├── stream-snapshot.ts
│   │   ├── text-content.ts
│   │   └── text.ts
│   ├── extensions/
│   │   └── traits/
│   │       ├── component.ts
│   │       ├── projector.ts
│   │       ├── renderer.ts
│   │       ├── streaming-source.ts
│   │       ├── text-rewriter.ts
│   │       └── text-visitor.ts
│   ├── presentation/
│   │   ├── style.ts
│   │   ├── theme-key.ts
│   │   └── theme.ts
│   └── view/
│       ├── geometry.ts
│       ├── scene.ts
│       └── view.ts
├── testing/
│   ├── access.ts
│   └── index.ts
└── transport/
    └── abi/
        └── structural/
            ├── generated/
            │   ├── view_abi.ts
            │   ├── view_abi_conformance.ts
            │   ├── view_abi_manifest.json
            │   ├── view_calls.ts
            │   └── view_materialize.ts
            └── schema/
                └── bridge-schema.json
```

The following old source directories no longer exist:

```text
packages/iyon-tui/src/values/
packages/iyon-tui/src/traits/
packages/iyon-tui/src/generated/
```

The remaining root modules are intentionally deferred to later atomic cuts:

```text
composition-related: compose.ts, define-view.ts, execution.ts,
                     execution-context.ts, child-owner.ts, persistent_seq.ts,
                     tracked-state.ts, internal-composition.ts, view-internals.ts
runtime/control-related: runtime.ts, tui.ts, handles.ts, handle-registry.ts,
                         component.ts, component-facade.ts, history.ts,
                         text-input.ts, stream.ts, scroll-pane.ts, native-handles.ts
structural transport: ir.ts, retained_dag.ts, native_view_abi.ts,
                      native_view_policy.ts, native.ts
temporary mixed contract: types.ts, style-internals.ts
```

## 2. Ownership moves

| Previous path | CUT 1 owner |
|---|---|
| `src/errors.ts` | `src/api/errors.ts` |
| `src/values/view.ts`, `src/values/geometry.ts`, `src/scene.ts` | `src/api/view/` |
| `src/values/style.ts`, `src/values/theme.ts`, `src/values/theme-key.ts` | `src/api/presentation/` |
| `src/values/annotations.ts`, `diff.ts`, `projection.ts`, `stream-snapshot.ts`, `text-content.ts`, `text.ts` | `src/api/content/` |
| `src/projectors.ts` | `src/api/content/projectors.ts` |
| `src/traits/*` | `src/api/extensions/traits/` |
| `src/testing.ts` | `src/testing/index.ts` |
| `src/testing-access.ts` | `src/testing/access.ts` |
| `src/bridge-schema.json` | `src/transport/abi/structural/schema/bridge-schema.json` |
| `src/generated/*` | `src/transport/abi/structural/generated/` |

The public entrypoints remain `@iyon/tui` and `@iyon/tui/testing`; only their private filesystem targets changed.

## 3. Tooling and seam updates

- `tools/tui-abi/view_abi.toml` now names the structural schema location.
- `tools/tui-abi-gen` writes and validates TypeScript ABI outputs under `transport/abi/structural/generated/`.
- Generated TypeScript imports use the new depth-relative paths to private root transport modules.
- Generated manifest, ABI reference, layout test, and benchmark registry were regenerated.
- `crates/iyon-tui-native/build.rs` reads the relocated schema.
- Declaration closure emits and traverses `testing/index.d.ts`.
- Ownership checks inspect the relocated semantic/testing/generated files.
- CI generated-output checks cover the relocated TypeScript generated directory.
- The H1 source hash in `docs/repository-separation/s0/api-surface.json` was refreshed because `index.ts` import paths changed; the 40 value and 74 type export sets are unchanged.

No generated file was hand-authored to compensate for the move; generator inputs/templates were updated and outputs were regenerated.

## 4. Validation evidence

All validation was rerun after the final generated output and before completion:

| Gate | Result |
|---|---|
| `bun run check:tui-abi` | PASS |
| `cargo test -p tui-abi-gen` | PASS; 27 passed, 0 failed |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `bun run typecheck` | PASS |
| `bun run check:tui-declarations` | PASS; 25 declaration files reachable |
| `bun run check:ownership` | PASS; 21 gates |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | PASS; 59 passed, 0 failed, 368 assertions |
| `cargo test --workspace --all-features` | PASS; 903 passed, 0 failed, 3 ignored |
| PERF-12 focused Rust gate | PASS; 832 passed, 0 failed, 2 ignored |
| PERF-12 R6b frontier smoke | PASS; 200 measured samples |
| `cargo test --workspace --features direct-ffi` | PASS; 897 passed, 0 failed, 3 ignored |
| Direct functional Bun suite | PASS; 55 passed, 0 failed |
| T15 default/direct structural comparison | PASS; equal screen and structural counters |

The direct functional suite omits only the default-only private-surface assertions in `tui_generated_view_abi.test.ts`; the direct Rust and workload parity gates pass.

For `plain_text/shared_path/size=20`, both transports reported 20 bridge hint hits, 0 misses, 40 semantic nodes inspected, 40 children visited, 40 materializer calls, 0 cold fallbacks, and 20 host mutations. Smoke medians were 80,084 ns for default N-API and 79,125 ns for direct FFI.

## 5. Iyon integration

The branch workflow consumed the pushed CUT 1 branch directly:

```text
bun run build:iyon -- api-h2-cut-1       PASS
resolved TUI revision: e7253757edf2...
bun run typecheck in branch worktree    PASS
bun run build:standalone                PASS
bun test in branch worktree             PASS; 281 passed, 0 failed, 733 assertions
```

The checked-in Iyon TUI pin was not changed. The build used the workflow's ephemeral branch worktree and rewrote only its local manifests/lockfile.

## 6. Non-goals and deferred work

CUT 1 does not:

- implement `composition/`, `runtime/`, or control ownership splits;
- split `types.ts`, `style-internals.ts`, `view-internals.ts`, or mixed retained/transport modules;
- implement state/content transport planes;
- change public names, lifecycle, scheduling, retained identity, or ABI semantics;
- modify `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md`.

Those decisions remain for CUT 2–5 under the H2 handoff.

**CUT 1 decision: GO.** The low-ambiguity filesystem ownership move is complete, generated/staging paths are authoritative, the H1 public surface is unchanged, and the retained/native/Iyon gates pass.
