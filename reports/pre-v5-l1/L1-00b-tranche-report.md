# Pre-V5 L1-00b completion report (this tranche)

Date: 2026-09-04. Commit: [TBD — tranche commit]. Parent: 244b978 (`feat: add pre-v5 lowering handoff`).

## Scope (this tranche = steps 5–8 plus L1-00b follow-ups)

- Trace harness: `packages/iyon-tui/bench/pre-v5-l1-trace.ts` — per-frame JSONL events
  (frame/snapshot/poll/render/seal/error), analyzed summaries, baseline diff vs prior run.
- New Rust counters (6, in `crates/iyon-tui/src/perf.rs`, read via new `content_ffi.rs` exports):
  - `content_snapshot_validation_failures` — `StreamSnapshot::validate_or_record` invalid-snapshot count.
  - `content_handle_invalid` — N-API snapshot-handle resolution misses (expired/missing handle).
  - `content_handle_revoked` — explicit snapshot-handle revocations.
  - `content_snapshot_projector_runs` — rewrite-projector invocations inside `compile_stream_snapshot`.
  - `content_snapshot_projector_fallbacks` — projector panics/pollutions caught and replaced with unprojected text.
  - `stream_seal_completes` — stream-seal polls that resolved a pending seal (drives `sealed` edge).
- Fixture: `packages/iyon-tui/tests/tui_smooth_delivery.test.ts` — gradual smooth reveal
  (per-frame `setSnapshotRange`-like widening) and completion-at-seal behavior.

## Behavior change worth noting (step 8 characterization)

- `Smooth` seal polls complete pending seals asynchronously (`sealed` edge goes Config→Complete
  only on seal-poll completion), so "seal at complete range" does not force `sealed=true`
  synchronously. Fixture asserts observed behavior: `sealed=true` after seal-poll completion.
- Guarded codegen panics in the rewrite-projector path are now *counted and recovered*, not fatal:
  `content_snapshot_projector_fallbacks` + text passthrough. This is pre-existing behavior made
  observable, not a new catch.

## Verification (this session, working tree at commit)

- `cargo fmt --all -- --check` — clean.
- `tools/lint/clippy-gate.sh` — exit 0.
- `cargo test --workspace --all-features` — 23 binaries `ok`, zero FAILED/panics.
- Full bun suite (`packages/iyon-tui/tests` + `packages/tui-consumer-fixture/tests`) — 100 tests, 0 fail.
- `biome lint` on the 2 new files — clean (3 lint assists fixed: literal-key access, index loop, non-null assertions).
- `tsc --noEmit` — clean.
- `bun packages/iyon-tui/bench/pre-v5-l1-trace.ts` — exits 0, writes `reports/pre-v5-l1-trace/`;
  sampled harness verified (frames advance, counters non-null, Rust/TS deltas null).
- `check:ownership` — fails only on pre-existing `crates/iyon-tui/src/content.rs` violations
  (lines 90–100, present at 244b978); new files owned correctly per facade rules.

## Deferred to CI

- `check:tui-declarations` and anything needing `bunx`/network — sandbox default-deny this session.

## Files changed

- `crates/iyon-tui/src/perf.rs` — 6 new counters + snapshot accessors.
- `crates/iyon-tui/src/application/content.rs` — counter call sites.
- `crates/iyon-tui/src/scene/host.rs` — seal-completion counter call site.
- `crates/iyon-tui-native/src/content_ffi.rs` — N-API export + handle-miss/revoke counters.
- `packages/iyon-tui/bench/pre-v5-l1-trace.ts` — NEW trace harness.
- `packages/iyon-tui/tests/tui_smooth_delivery.test.ts` — NEW fixture.
- `reports/pre-v5-l1/L1-00-baseline.md` — NEW baseline report.

## Suggested acceptance question for the planner

Trace harness + 6 counters + smooth delivery fixture = steps 5–8 closure, or are further
characterization fixtures required before L1-01? Remaining L1-00 steps 9–16
(T15 captures, baseline doc, owner map, freeze readiness) are untouched.
