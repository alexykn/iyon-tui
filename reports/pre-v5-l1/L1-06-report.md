# L1-06 completion report: changed state versions and host-owned access

Date: 2026-09-05. Base: `8e63141` (L1-05 committed).

## What changed

**Dirty-state worklist + immutable versions (step 1).**
`ViewStateRegistry` gains a deduplicated `dirty: HashSet<u64>` worklist and
a committed version table (`HashMap<u64, Arc<ViewStateSnapshot>>`).
`mutate_record` applies one validated mutation atomically, then publishes a
new immutable `Arc` version and queues the id — exactly once no matter how
many accepted mutations land before capture. No-op writes return before
publishing, so versions advance only when logical values change (the record
already distinguished noop via empty `StateEffects`; the registry now
honors it at the version level).

**One candidate overlay over the committed table (step 2).** New module
`retained_state/capture.rs`: `StateCandidateOverlay { epoch, demanded,
touched }` plus the borrowed reader `StateFrameView { committed, overlay }`
with `get()` reading overlay-then-committed. `capture_candidate` builds the
demanded set from desired ∪ visible ∪ in-flight, owns `Arc` versions for
changed-through-epoch and newly demanded ids only, and drains only the
served dirty marks. Every `states: &HashMap<u64, ViewStateSnapshot>`
consumer — `application/host.rs` prepare fns, `kernel.rs`, `scene/host.rs`
(8 sites), `scene/root.rs` (5 sites), `ResolveSession::set_state_snapshots`
— now takes `&StateFrameView<'_>`; `states.get(&id)` call sites compile
unchanged. The old whole-registry `snapshots()` is deleted (step 8); no
production caller remains.

**Newly attached/remounted capture (step 3).** `create` publishes the
initial committed version without dirtying, and capture puts
desired-but-not-yet-visible ids into `touched` even when clean, so
configure-while-unmounted → mount resolves current values with no extra
mutation.

**Immutable base references (step 4).** `ResolutionOverlay.states` is now
`HashMap<u64, Arc<ViewStateSnapshot>>`; branch overlays retain frame
versions by refcount bump instead of cloning maps/decorations per branch
(body, history, component subtree each cloned the full map before). Cheap
scalar revisions stay direct `u64` copies.

**Lock and validation reduction (step 5).** `HostViewState` carries the
immutable id — `state_id()` needs no lock, and dispose passes the id
instead of re-locking the record to read it. `validate_targets` is fused
from two lock-every-record passes into one lookup per target; the obsolete
`validate_ids` helper is deleted.

**Host-owned records (step 6).** Audit result: every record access already
ran under the `HostInner` mutex except `state_id()` (record-only read) and
`validate_node_kind` (record-only read). Both are unified — the id is
stored, validation routes through the host lock — so records now live
directly in the registry (`HashMap<u64, ViewStateRecord>`, no
`Arc<Mutex<>>`). Wrappers hold `{ id, Weak<HostInner> }`; `record→host`
lock inversion in dispose is gone. Cross-host misuse is still rejected via
the host namespace in the id high bits; unknown-id disposal stays a no-op.
Any record still needing independent access keeps none: teardown
(`dispose_all`), binding, capture, and diagnostics all run under the host.

**Binding preparation + in-flight pins (step 7).** `set_visible` is now
two-phase (`Result`: validate all bindings live, then apply), and
`commit_visible_state_bindings` propagates it — a failed commit can no
longer install a partial or ghost binding set (previously an infallible
`debug_assert`). In-flight pins are set at prepare, preserved across the
visible swap (visible commits before in-flight clears), released on
candidate discard, and block disposal (`STATE_MOUNTED`) while held.

## Stop-gate evidence

- One paint mutation visits only its attachment:
  `capture_ignores_unmounted_states` (demanded == touched == {bound});
  `dirty_marks_deduplicate_and_capture_drains_once` (3 writes → 1 mark,
  second capture empty, identical rewrite schedules nothing).
- Failed frame / delayed receipt keep old versions:
  `captured_overlay_pins_old_versions_across_later_mutations` (pinned `Arc`
  reads v1 while committed reads v2); host-level
  `failed_frame_retains_old_state_versions_until_retry` (injected failure →
  old style visible → retry commits new style); existing
  `failed_frame_keeps_old_visible_state…` and
  `environment_requeues_in_flight_presentation_receipts` pass.
- Clear after override: `clear_after_override_reveals_base` (registry) plus
  TS `tui_state_envelope` "reveals base on clear" end to end through the
  rebuilt binary.
- No revival / disposal race: `dispose_rejects_bound_records_and_stays_idempotent`
  (desired-bound, in-flight-bound, double dispose, unknown id, monotonic
  non-reuse, post-dispose capture clean); `visible_bindings_validate_before_commit`
  (partial set rejected atomically).

## Gates (this session)

- `cargo test --workspace --all-features`: 23 suites ok, 0 failures
  (lib 593 incl. 8 new registry + 2 capture + 1 host tests).
- `bun test packages/iyon-tui/tests/`: 103 pass, 0 fail (native rebuilt).
- `cargo fmt --all -- --check`: clean. `tools/lint/clippy-gate.sh`: 0 errors
  (5 new warn-mode dead-code notes on test-observability accessors
  `epoch/touched_len/contains_touched/overlay_epoch`, consistent with the
  file's warn backlog; L1-12 will consume the epoch).
- `bun run check:tui-abi` + `generate:tui-abi`: stable, no drift (no ABI or
  binding signature changed; TS untouched).
- `bun run check:ownership`: ALL PASS (1417-item Rust snapshot matches).

## Notes for L1-07 / L1-12

- `StateCandidateOverlay.epoch` is written but only read by tests; L1-12's
  prepared-commit work is its first real consumer.
- Commit-time `set_visible` failure is unreachable (in-flight + desired pins
  block disposal between prepare and commit) and now errors instead of
  debug-panicking; no wedge path added.
