# L1-00 baseline — behavior, route, and cost pinning (record, no code changes)

Tranche: L1-00 (steps 1–4; steps 5–8 pending)
Baseline SHA: 244b978b80ff479408bbe6c83af223a69f310b6b
Result SHA: 244b978b80ff479408bbe6c83af223a69f310b6b (no production changes)
Native artifact hash / target / profile / features:
  sha256 3fbde2f58c405b171a78f08b774d8eee16c851b6b2ee897bd6ba8d12278a14dc
  packages/iyon-tui/native/iyon-tui-native.node
  darwin-arm64, default N-API structural addon, debug profile
Bun and Rust versions: bun 1.4.0, rustc 1.97.1 (8bab26f4f), cargo 1.97.1,
  host aarch64-apple-darwin

## Gates (all green, 2026-09-04)

- cargo fmt --all -- --check: clean
- sh tools/lint/clippy-gate.sh: exit 0 (0 errors; 2425 warnings)
- cargo test --workspace --all-features: 23/23 binaries ok, 0 failures
- cargo test -p tui-abi-gen: 7/7 ok
- tsc --noEmit (current tsconfig): clean
- biome lint packages tools: 0 errors, 380 warnings, 4 infos (106 files)
- bun run check:ownership: ALL OWNERSHIP CHECKS PASSED
- bun test (iyon-tui tests + consumer fixture): 98 pass, 0 fail
- bun run native:stage + native:smoke: staged ok, smoke ok
  (content ok, packaged TUI smoke rows render)

## Environment notes

- `bun run check:tui-declarations`: PASS (37 reachable public declarations;
  root and testing public types nameable). Requires
  `BUN_INSTALL=/tmp/bun-home HOME=/tmp/bun-home` in this environment
  because `~/.bun/install` is not writable here; without the redirect,
  `bunx` fails with tempdir PermissionDenied (reproduced on clean tree).
- Typecheck evidence uses the cached typescript@5.9.3 compiler directly;
  Biome evidence uses @biomejs/biome@2.5.12 (same pinned version as
  package.json scripts).
- `cargo fix` remains unavailable here (cargo lock-server socket blocked);
  machine fixes go through the JSON-suggestion applier instead.

## Rust export census (step 3)

- Ownership snapshot `tools/ownership/snapshots/iyon-tui-rust-surface.txt`:
  1369 items. By top module: content 665, presentation 285, projection 123,
  interaction 90, application 63, history 39, controls 21, output 18,
  theme 15, stream 15, scroll 14, component 11, scene 8, text 1, prelude 1.
- Native crate (`iyon-tui-native`) imports ~65 root items from `iyon_tui`:
  authoring/construction types (View, IntoView, Renderer, DiffRenderer,
  TextSpan, StyleSpec, GridCellSpec, GridTrack, TextInput, Border*/ColorSpec,
  Insets, Diff*), host controls (TuiHost, TuiEnvironment, Host*,
  HostViewState, NativeRef-adjacent), state patches
  (ViewStateGeometryPatch/Property, ViewStatePresentationPatch/Property,
  ViewStateSizeMode, ViewStateTextAttributes), content primitives
  (ContentAnnotationRecord, ContentMutationResult, ContentDelivery,
  ContentFamily, HostContentSource/Funnel/Port/Connector, SmoothConfig),
  interaction (Key, KeyStroke, Modifiers, Output), text module.
- Test-only hooks: `crates/iyon-tui/src/testing.rs` (+ `testing/` re-export
  surface used by the TS testing entrypoint); benches under `src/bin/`,
  `examples/`, `benches/`.
- Per-item authoring vs passive vs host-integration classification is
  L1-01 ledger work (handoff §5.1, step L1-01.6), not repeated here.

## TS surface (step 4)

- Public contract frozen by `check:tui-declarations` (CI) + ownership checks
  h2-cut4-root-cleanup, h2-cut5-package-publication,
  standalone-consumer-public-entrypoint (all PASS above).
- Consumer fixture green inside the 98-test bun run.
- Generated TS bindings: `view_calls.ts` 60 exported functions;
  conformance + structural ABI generated files excluded from lint by config.

## Still open (steps 5–8)

- Route-preserving trace harness (structure/state/content/native time).
- Copy/route counters (payload bytes, metadata items, node constructions,
  retains, cells) on the existing perf reporting path.
- Baseline T15 route-smoke + perf:content captures into reports/pre-v5-l1/.
- Characterization fixtures (§18 risks: patch distinction, Smooth
  prefixes/timing/seal, History-prefix output, post-acceptance wake failure,
  counter-exhaustion atomicity, UTF-8/ANSI boundaries).

Decision: accepted (record-only tranche, no behavior change)
Reason for any block: none; steps 5–8 proceed as L1-00b.
