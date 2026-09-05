# Pre-V5 L1-00c completion report: step-8 characterization fixtures (this tranche)

Date: 2026-09-05. Parent: 0b1e29d.

## Scope

L1-00 step 8 remaining risks (Smooth was done in L1-00b). Five fixtures, all
characterization — no production code changed, per the L1-00 freeze rule.

## Fixtures added (11 Rust tests + 7 TS tests, all passing)

1. **State/structural patch distinction** (`application/host.rs`)
   - `state_patch_leaves_desired_structural_revision_untouched`: a retained-state
     `set_presentation` consumes no structural revision; a structural
     publication consumes exactly one.

2. **Stable History-prefix output** (`tests/tui_history_prefix.test.ts`)
   - Appends extend the settled row sequence verbatim (body stays pinned last);
     re-render is idempotent; freezing a live tail unit swaps only the tail.
   - Placement fact: units render on-screen above the body; `nativeHistoryRows`
     stays empty until units scroll off-screen (transfer only on scroll-off).

3. **Source post-acceptance wake failures** (`application/host.rs`, two hosts/one
   environment, mutex-poison injection)
   - `..._keeps_revision_authoritative`: error surfaces, installed revision stays
     readable, earlier-woken hosts stay pending.
   - `..._aborts_later_subscriber_wakes`: documents the order-dependent gap —
     `finish_mutation` returns on the first failing host, so later wakes are
     lost. Pinned for the §9.6 / L1-12 "no lost wake" target; any change must be
     explicit.

4. **Counter-exhaustion atomicity** (`application/content.rs` x6,
   `application/host.rs` x1, `stream/coord.rs` x1)
   - Revision-MAX matrix per mutation: `replace`/`clear` preflight before install
     (storage intact); `append` installs bytes before the preflight (storage
     grows, revision/accounting frozen — stale-revision snapshots observe
     never-accepted bytes); `seal` sets the flag before the preflight;
     `truncate_head` advances the head before the preflight; `clear` at
     generation-MAX empties storage before the generation check.
   - Structural lane preflights cleanly: rejected publication disturbs nothing.
   - `StreamOffset::checked_add` boundary contract pinned.
   - These are reproduced §9.6/§18.6 gaps, not fixed here — the storage tranche
     must change these fixtures explicitly (preflight fallible arithmetic before
     installing candidate storage).
   - Out of scope noted: static ID counters (`HistoryUnitId`, history/host
     identities) panic on exhaustion by design and cannot be forced in-process
     without poisoning global state shared with other tests.

5. **UTF-8/ANSI scanner boundaries** (`tests/tui_ansi_scanner.test.ts`)
   - Every append split through SGR/OSC/text/newline byte positions renders
     identically; SGR bold reaches styles; `\x1b[2J` consumed with text joined;
     unfinished escape is harmless; OSC 8 identical under BEL and ST, on/off.
   - Demonstrated scanner defect: U+00DB "Û" (bytes C3 9B) hits the C1-CSI arm
     with no escape anywhere nearby. Failure is safe (no panic/corruption,
     holds last committed frame, clean revision recovers) but lossy: every
     newer revision is suppressed while the byte remains in the domain. Control
     "é" (no 0x9B byte) renders fine. Any scanner repair must change this
     fixture explicitly.

## Verification (this session)

- `cargo fmt --all -- --check` — clean (one fmt reflow applied to new code).
- `tools/lint/clippy-gate.sh` — exit 0; no new clippy/rustc warnings from new code.
- `cargo test --workspace --all-features` — 23 binaries `ok`, zero failures.
- Full bun suite — 105 tests (100 + 5 new), 0 fail. `tsc --noEmit` — clean.
- `biome lint` on the 2 new TS files — clean.
- `bun run check:ownership` — ALL CHECKS PASSED.

## Deferred to CI

- `check:tui-declarations` and anything needing `bunx`/network.

## Step-8 status after this tranche

All six named risks now have fixtures: Smooth (L1-00b), state/structural,
History-prefix, Source wake, counter-exhaustion, scanner boundaries (this
tranche). Remaining L1-00 work sits in steps 3–4 (export/declaration surface
recording, partly CI-deferred), step 7 completion review, and the Stop
condition (reproducibility + defect-vs-unconfirmed-risk ledger).
