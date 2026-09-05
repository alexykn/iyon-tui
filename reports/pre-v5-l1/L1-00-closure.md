# L1-00 closure report: baseline frozen at the fixed SHA (this tranche)

Date: 2026-09-05. SHA: `36efe16f90ac7ef3c758c65ee35a0502d6b95ea1`
(bun 1.4.0, rustc 1.97.1, cargo 1.97.1, aarch64-apple-darwin).
Supersedes the 244b978 baseline for behavior; the 244b978 artifacts stay on
record as the pre-fix reference.

## §19 battery (repo's actual commands, this session, clean tree)

- `generate:tui-abi` → no drift; `check:tui-abi` exit 0; `cargo test -p tui-abi-gen` 7/7.
- `cargo fmt --all -- --check` clean; `clippy-gate` exit 0.
- `cargo test --workspace` all ok; `cargo test --workspace --all-features` 23/23 ok.
- `bun run typecheck` exit 0 (bunx works with `BUN_INSTALL=/tmp/bun-home HOME=/tmp/bun-home`).
- `bun run lint:ts` exit 0: 0 errors, 380 warnings + 4 infos — identical count to
  the 244b978 baseline at 4 more files, i.e. new files contribute zero warnings.
- `bun run check:tui-declarations` PASS (37/37 boundary + closure). The old
  sandbox deferral is lifted: it runs fine under plain bun.
- `check:ownership` ALL PASSED (44 value + 97 type TS exports frozen; 1369 Rust
  items match — the fixes added no public surface).
- `native:stage` + `native:smoke` ok; full bun suite 105 tests / 760 expects,
  0 fail (consumer fixture alone: 10 tests / 44 expects, 0 fail).
- Generated bindings unchanged: `view_calls.ts` still 60 exported functions.

## Step 3 — Rust export census (reaffirmed, not re-typed)

Ownership `rust-surface-snapshot` passing at this SHA is the census: 1369
mapped items with the module split recorded in `L1-00-baseline.md`, native
import categories unchanged (no signature touched by the fixes). Per-item
authoring/passive/host-integration classification remains L1-01 ledger work.

## Step 4 — TS declarations (closed)

`check:tui-declarations` 37/37 PASS this session + frozen 44+97 export surface
+ consumer fixture green. Nothing deferred.

## Step 6 — counter coverage map (observed in `trace-perf-counters.json`)

Doc item → counter → trace value: deep-copied bytes/metadata/cells →
`annotation_records_copied=10`, `paint_cells_allocated=10637`,
`surface_cells_composited=2902` (+ per-run `copiedBytes`/`acceptedBytes`);
node construction → `view_nodes_constructed_rust=38`,
`persistent_seq_nodes_allocated=21`; state visits →
`view_state_mutations_accepted=2`, `view_state_presentation_invalidations=2`;
Source acquisition → `source_snapshots_acquired=10`; semantic parse →
`semantic_preparations=4`; content lowering → `content_surface_clones`
(0 on this path; pinned =1 by the Rust `reveal_surface` unit test);
row layout → `layout_nodes_emitted=27`, `measure_node_calls=31`;
cache clears → `global_cache_clears=1`; registry scans →
`content_registry_port_scans=3`.

## Step 7 — trace artifacts (with provenance)

- `trace-default.json` (244b978, pre-fix reference, default addon).
- `trace-fixed.json` (36efe16, default addon; `rust_counters: null` by design —
  the default build omits the feature).
- `trace-perf-counters.json` (36efe16, `perf-counters` build via
  `ION_NATIVE_FEATURES=perf-counters`): 44 counters live, 23 nonzero (table above).
  Default addon restored + smoked afterwards; staged `.node` is gitignored.

## Stop-condition ledger

Reproduced AND fixed (fixtures pin the fixed behavior): exhaustion
partial-installs (`36efe16`), scanner 0x9B suppression (`e0cc922`), wake abort
(`36efe16`). Details in `L1-00d-fix-note.md`.
Still unconfirmed inspection risks (no forced test — out of L1-00 reach):
static ID-counter exhaustion panics by design (global state, untestable
in-process); wake fan-out grouping cost (S25, performance only, behavior
pinned); `cargo fix` unavailable in this environment (lock-server socket).
Reproducibility: trace harness exits 0 on both addon profiles at the pinned
SHA with artifact hashes embedded; expected outputs are the green suites above.
Nothing is deferred to CI anymore except genuinely networked publishing steps.

L1-00 is closed. Next: L1-01 (unsupported internal binding contract).
