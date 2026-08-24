# S0 baseline checks

**Source revision tested:** `bd503b0382e34d74a38c562b9662d08c8c96f58a`

**Worktree before checks:** clean

**Host/toolchains:** see `environment.json`

S0 records failures rather than repairing them. No product or framework behavior was changed.

## Results

| Check | Result | Evidence |
|---|---|---|
| `bun install --frozen-lockfile` | PASS | 25 installs checked across 21 packages; no changes. |
| `bun run native:stage` | PASS | Built/staged the darwin-arm64 release addon. Post-stage hash is in `artifacts.json`. |
| `bun run check:tui-abi` | PASS | Generated TUI ABI is current. |
| `cargo fmt --all -- --check` | **FAIL (known baseline)** | Four formatting diffs, all in `crates/iyon-native/src/events.rs` (import order and wrapping near lines 188, 200, and 209). No formatting was applied. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **FAIL (known baseline)** | 105 emitted `error:` diagnostics. Compile summaries report `api-surface` 6, `iyon-core` 8/11 by target, `tui-abi-gen` 9 by target, and `iyon-tui` 71. These include pre-existing dead code plus Rust 1.97 Clippy diagnostics. No lint fixes were applied. |
| `cargo test --workspace` | **FAIL (known baseline)** | Stops in `api-surface --test current_surface`: 1 pass / 3 fail. Failures: `current_surface_has_zero_mapping_drift`, `every_reachable_method_has_a_generated_ts_declaration`, and `checked_in_artifacts_are_fresh`. Exact causes are recorded below. |
| `cargo test --workspace --exclude api-surface` | PASS | 1,079 pass / 0 fail / 3 ignored across all remaining Rust test and doctest binaries. The `iyon-tui` library binary itself reports 724 pass / 0 fail / 1 ignored. |
| `cargo run -p api-surface -- check --config tools/api-surface/surface.toml --require-implemented` | **FAIL (known baseline)** | Generated manifest content hash drift: expected `ad78a6d6d08881cc`, computed `2b7dc68215a44084`. |
| `bun run typecheck` | PASS | TypeScript compiler exits 0. |
| Runtime TUI battery (command below) | **FAIL (known order-dependent baseline)** | 213 pass / 1 fail across 38 files. `perf11v4_direct.test.ts` weak-cache expiry assertion expected `live_weak_upgrades=0`, received `2196`. The same file passes 6/6 in a fresh isolated process. |
| `bun test packages/tui-consumer-fixture/tests` | PASS | 10 pass / 0 fail across 2 files. |
| `bun test plugins/app/iyon/test` | **FAIL (known baseline)** | 113 pass / 1 fail across 12 files. `production_successful_ls_is_green_finished` cannot find the expected row, passes `-1` to `cellXOfText`, and native validation reports `ION_INVALID_INPUT: row must fit in u16` at `recovery3_production.test.ts:220`. The failure reproduces in isolation. |
| `bun test packages/iyon-plugins/tests packages/iyon-plugins/test` | PASS | 30 pass / 0 fail across 18 files. |
| T13.1 execution-frontier battery | PASS | 18 pass / 0 fail across runtime, external-consumer, and production-app frontier files. |
| PERF-12 retained identity/wide/payload battery | PASS | 31 pass / 0 fail across T6 identity, T10 wide edits, and T11 payload families. |
| T13.1 memory soak | PASS | 100,000 keyed cycles; 6,250 interleaved aborts; RSS held at 79 MiB from 20k through 100k; 64 steady subscribers; 0 after disposal. |
| `bun run native:verify` | **FAIL (known repository defect)** | Root script points to missing `packages/iyon-runtime/scripts/verify-native.ts`; Bun reports `Module not found`. Native staging and direct load/version/ABI probes pass. |

## Exact API-surface test failures

`cargo test --workspace` reports:

1. `current_surface_has_zero_mapping_drift`: expected generated manifest hash `ad78a6d6d08881cc`, computed `2b7dc68215a44084`.
2. `every_reachable_method_has_a_generated_ts_declaration`: missing declaration for `iyon-tui::presentation::ir::View::native_axis_from_children`.
3. `checked_in_artifacts_are_fresh`: generation refuses mapping drift with `missing=10, stale=0`.

The source API, checked-in mappings, TypeScript facade, and runtime addon exports are independently frozen in `api-surface.json`; S0 does not disguise the stale generated parity baseline as green.

## Focused commands

Runtime TUI battery:

```sh
bun test \
  packages/iyon-runtime/tests/tui_* \
  packages/iyon-runtime/tests/view_materialize.test.ts \
  packages/iyon-runtime/tests/perf11v4_direct.test.ts \
  packages/iyon-runtime/tests/perf12_* \
  packages/iyon-runtime/test/tui_realtime.test.ts \
  packages/iyon-runtime/tests/tui_demo.test.ts \
  packages/iyon-runtime/tests/generated
```

Isolated weak-cache control:

```sh
bun test packages/iyon-runtime/tests/perf11v4_direct.test.ts
```

Execution-frontier battery:

```sh
bun test \
  packages/iyon-runtime/tests/perf12_t13_1_state.test.ts \
  packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts \
  plugins/app/iyon/test/perf12_t13_1_r9_production.test.ts
```

Retained PERF battery:

```sh
bun test \
  packages/iyon-runtime/tests/perf12_t6_identity.test.ts \
  packages/iyon-runtime/tests/perf12_t10_wide.test.ts \
  packages/iyon-runtime/tests/perf12_t11_payload.test.ts
```

Memory soak:

```sh
bun run packages/iyon-runtime/bench/perf12_t13_1_r10_memory_soak.ts
```
