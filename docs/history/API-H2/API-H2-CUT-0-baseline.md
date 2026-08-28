# API-H2 / STRUCT-1 — CUT 0 baseline

**Status:** COMPLETE
**Captured:** 2026-08-28T10:58:51Z
**Purpose:** Freeze the H1 public surface and prove the pre-movement safety gates. CUT 0 changes no source architecture and does not implement PERF-13.

## 1. Source and environment

| Item | Value |
|---|---|
| TUI baseline revision | `c32b43fed939650d349cd53a6b0eb967cc53acfd` |
| TUI source-equivalent H1 revision | `477dced2f6d9fde2f6d1e7875a55f0cbdd88ab56` |
| TUI remote `main` | `477dced2f6d9fde2f6d1e7875a55f0cbdd88ab56` |
| Iyon revision / `main` | `3aced4f27709747679533e476f4615cd6e83a233` |
| Bun | `1.4.0` (`1.4.0+34cbb9a40`) |
| Rust | `rustc 1.97.1`, `cargo 1.97.1` |
| Target | `aarch64-apple-darwin` |
| macOS | `26.6.2` (`25G83`) |

The TUI baseline revision is a documentation-only commit on top of the merged H1 source. No TUI source file differs from `477dced`. The TUI and Iyon worktrees were clean before this record was written. `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md` was not modified.

## 2. H1 public-surface snapshot

The frozen H1/S0 snapshot remains:

```text
docs/repository-separation/s0/api-surface.json
```

Evidence:

```text
root entrypoint:                 @iyon/tui
root source:                     packages/iyon-tui/src/index.ts
root source SHA-256:             43b07cac381a033a03260c32e6d77c48f77112739a90b31b90a575cc5d9567d9
H1 snapshot SHA-256:             6173dae53ba06f49969730361b54ca83752415fe06dccfd081ff6b861da8303f
root value exports:              40
root type exports:               74
Rust mapped surface records:     1519
reachable public declarations:  25
```

`bun run check:ownership` confirmed the root surface matches the frozen snapshot, contains no application concepts, and keeps testing at `@iyon/tui/testing`.

## 3. Safety and compatibility gates

All commands were run from the clean TUI baseline before this document was added.

| Gate | Result |
|---|---|
| `bun install --frozen-lockfile` | PASS; no changes |
| `bun run check:tui-abi` | PASS |
| `cargo test -p tui-abi-gen` | PASS; 27 passed, 0 failed |
| Generated ABI diff check | PASS; clean |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `bun run typecheck` | PASS |
| `bun run check:tui-declarations` | PASS; 25 declaration files reachable |
| Default native staging | PASS; darwin-arm64 N-API addon |
| `bun run check:ownership` | PASS; 21 ownership/public-surface gates |
| TUI + external-consumer Bun tests | PASS; 59 passed, 0 failed, 368 assertions |
| `cargo test --workspace --all-features` | PASS; 903 passed, 0 failed, 3 ignored |
| PERF-12 focused Rust gate (`iyon-tui`, `native-host,perf-counters`) | PASS; 832 passed, 0 failed, 2 ignored |
| PERF-12 R6b frontier smoke | PASS; 200 measured samples |

The retained-composition tests included scoped invalidation, retained differential/fuzz coverage, native builder/scalar/transaction paths, History/stream refresh isolation, topology replacement, rollback, and independent animation/stream targets.

## 4. Structural N-API/direct-FFI parity

Both native variants were staged and exercised.

| Gate | Result |
|---|---|
| Direct native staging (`ION_NATIVE_FEATURES=direct-ffi`) | PASS |
| `cargo test --workspace --features direct-ffi` | PASS; 897 passed, 0 failed, 3 ignored |
| Direct functional Bun suite | PASS; 55 passed, 0 failed |
| PERF-12 T15 default N-API smoke | PASS; 20 measured samples |
| PERF-12 T15 direct-FFI smoke | PASS; 20 measured samples |
| Stable structural-result comparison | PASS; equal screen output and structural counters |

The full direct Bun suite reports two expected failures in `tui_generated_view_abi.test.ts`: those assertions deliberately require the default N-API addon to hide feature-gated direct pointer helpers. The direct functional suite omits that default-only private-surface test file; the direct Rust suite and direct T15 workload pass.

For the equal `plain_text/shared_path/size=20` workload, both transports reported:

```text
bridge_hint_hits:             20
bridge_hint_misses:            0
bridge_semantic_nodes:         40
bridge_children_visited:      40
direct_materializer_calls:    40
cold_fallbacks:                 0
host_mutations:                20
```

The default N-API smoke median was 90,167 ns and the direct-FFI smoke median was 80,584 ns. These are smoke measurements, not a performance decision; structural parity is the gate.

The default N-API addon was restaged after direct testing and is the final local native artifact.

## 5. Iyon integration

The branch build workflow was exercised without changing Iyon's checked-in TUI pin:

```text
bun run build:iyon -- main                         PASS
resolved iyon-tui/main: 477dced2f6d1...
dist/iyon --help                                  PASS
bun run build:standalone                           PASS
standalone distribution test                       PASS
bun run typecheck (branch worktree)                PASS
bun test (branch worktree)                         PASS; 281 passed, 0 failed, 733 assertions
```

The branch build used the workflow's ephemeral cached worktree and substituted the remote TUI `main` revision only there. The checked-in Iyon checkout remains clean and its dependency pin was not edited.

An ancillary pre-existing CI probe, `IYON_PROVIDER=mock dist/iyon auth status`, still reports `provider is not registered: mock` because the mock provider has no auth contribution. This is outside the H2 CUT 0 TUI/build gate and was not changed as part of this architecture-only baseline.

## 6. CUT 0 decision

**GO.** H1 public API, declarations, ownership direction, generated ABI, retained composition, native staging, N-API/direct structural behavior, and Iyon's branch build workflow are recorded and passing. CUT 1 may begin source movement. No PERF-13 state/content implementation is present.
