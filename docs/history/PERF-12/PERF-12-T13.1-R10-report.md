# PERF-12 T13.1 — R10 Authoritative Report

**Status:** decision record for Tranche 13.1 (Step 14R).
**Outcome:** **ADOPTED** as the supported `iyon-tui` rendering path — with the sanctioned R6b deferral carried as **blocked-by-deferral** (T13.1 remains formally PARTIAL per Review Addendum §33.7 until R6b runs against the finalized PERF-12v2 transport).

---

## 1. Provenance

| Artifact | Commit | Profile | Notes |
|---|---|---|---|
| Four-arm composition matrix (`PERF-12-T13.1-R10-composition-authoritative.jsonl`) | `dd6ae3d` (clean tree) | authoritative | §102 minimums: 1,000 measured ops/case → valid p99; cross-arm screen parity enforced |
| Cold fall-through gate (`PERF-12-T13.1-R0-cold-fallthrough.jsonl`) | measured at `4e32761` | smoke gate instrument | ≤3% gate |
| Projection overhead instrument (`PERF-12-T13.1-R3-projection-overhead.jsonl`) | measured at `4e32761` | smoke gate instrument | R6b go/no-go input |
| End-to-end overhead instrument (`PERF-12-T13.1-R6a-end-to-end-overhead.jsonl`) | measured at `4e32761` | smoke gate instrument | resolver-gap curve |
| Memory soak (`bench/perf12_t13_1_r10_memory_soak.ts`, raw console record: `bench/PERF-12-T13.1-R10-memory-soak.log`) | `5492290` (clean tree) | full scale | §22/§43 targets |

bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`); native addon unchanged since the T13-era staged artifact; no Rust changes after `b382d75`/`33bb5c7`.

## 2. Authoritative four-arm matrix (medians, ns)

Same deterministic §37 state sequence driven through four separate headless sessions; final screens identical across all arms per case (parity enforced by the harness — a mismatch throws).

| Case | current_body_key | rebuild_uncomposed | manual_stable_oracle | **retained_scopes** |
|---|---:|---:|---:|---:|
| exact_noop | 667 | 19,709 | 542 | **375** |
| footer_only | 67,084 | 60,500 | 50,833 | **59,083** |
| effort_style_state | 67,916 | 65,688 | 59,500 | **113,250** ⚠ |
| working_toggle | 59,042 | 56,875 | 39,896 | **45,833** |
| approval_toggle | 73,167 | 72,229 | 52,042 | **60,188** |
| steering_preview | 77,938 | 75,646 | 58,542 | **65,355** |
| tool_slot_update | 28,459 | 27,000 | 1,291 | **583** |
| pane_output_update | 958 | 28,375 | 1,292 | **542** |

Materializer calls per 1,000-op window (semantic construction density):

| Case | body_key | rebuild | oracle | retained |
|---|---:|---:|---:|---:|
| exact_noop | 0 | 10,000 | 0 | **0** |
| footer_only | 10,000 | 10,000 | 4,000 | **2,000** |
| working_toggle | 9,000 | 9,000 | 4,000 | **2,000** |
| approval_toggle | 10,500 | 10,500 | 3,500 | **1,500** |
| tool_slot_update | 17,988 | 18,000 | 6,000 | 6,000* |
| pane_output_update | 3,000 | 15,000 | 3,000 | 3,000* |

\* chrome untouched in these cases; counts come from the specialized card-slot/pane boundary work that all arms perform identically.

Reading:

- The candidate beats the application's hand-tuned bodyKey guard on **every** case except `effort_style_state` (see caveat), and beats or matches the manual oracle on no-op and boundary cases — with **zero application-side identity code**.
- `exact_noop` performs zero semantic construction and zero native mutations in the retained arm (333–375 ns is scheduler bookkeeping + drain only).
- ⚠ Honest regression: `effort_style_state` (+~47 µs vs rebuild). Root cause: an effort change legitimately re-executes Composer (style-state) AND Footer (label text), publishing through two scoped ViewSlot boundaries, versus one whole-scene install on the uncomposed arms. The delta is bounded and absolute (~0.11 ms median, p99 ~0.21 ms); every other case wins. Accepted.

## 3. Gate evidence (handoff §37 checklist)

**Execution**

- State write targets a scope directly; clean sibling/parent bodies don't execute — `perf12_t13_1_state.test.ts` §31.1 frontier gate END-TO-END: write B ⇒ App=0 A=0 B=1 C=0 by counters; production equivalents green (`perf12_t13_1_r9_production.test.ts`: no-op dispatch ⇒ zero bodies; configChanged ⇒ Footer=1 others 0).
- Props updates skip unchanged children — `perf12_t13_1_defineview.test.ts`; both skip/supersede directions pinned again in §32.3 tests.
- Mount/unmount/reorder identity correct; keys local — `perf12_t13_1_r8_boundaries.test.ts` (keyed reorder/insert/remove/nested/duplicate-key).
- Abort semantics — `perf12_t13_1_abort_retry.test.ts` (level-triggered obligations preserved across early/middle phase-1 and phase-2 failures; no auto-retry loop; producer rollback scoping).

**Semantic DAG**

- Changed ⇒ new NodeId; unchanged scope-local nodes reuse exact Views — `perf12_t13_1_execution/compose/projection.test.ts` (bridge-level identity assertions, scenario I zero-native-work).
- Clean projections keep exact identity — parent output object identical across child-local updates (R3/R6a/R9 suites).

**Native**

- Per-scope independently retained roots via existing ViewSlot/boundary machinery; local update never materializes parent/siblings — R6a suite (`cold_fallbacks 0`, `host_mutations 0` across repeated hint-driven updates).
- Old lease survives until replacement — R7 transactional publication suite (prepare failure publishes NOTHING anywhere).
- Multi-scope publication atomic — R7 atomicity test + prepare-all/commit-once flush protocol.
- Transport details private — scopes never touch FFI; projection factory owns all native handles (R8/R9 wiring).

**Performance**

- Cold raw construction: **+1.31% / +0.53% / +1.89% / +1.32%** across four clean-tree runs of the R0 instrument (gate ≤3%) — pass with margin despite compose routing now being live in EVERY `View.*` constructor.
- Projection overhead flat: leaf update 31.7 / 24.7 / 22.9 µs at 10/100/1,000 scopes (R3 instrument) — per-update cost does not scale with mounted-scope count pre-scene-render.
- Zero-allocation exact hits: `composition_exact_view_reuses` counters; noop outputs counter-verified.
- Production trace not slower: authoritative matrix above (footer-only −12% vs shipped body-key arm; tool-card boundary case −98%).
- Wide asymptotics: PERF-12 wide matrix unregressed (battery green incl. `perf12_t10_wide.test.ts`).

**Generic public API**

- External fixture demonstrates direct scoped invalidation with zero setup — `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts` (public imports only; §31.1 frontier gate; keyed reorder skips; ownership modes; 10k stream-append isolation).
- Iyon app used only public APIs — `plugins/app/iyon/src/view.ts` (defineView/state chrome), and as of Step 14R **bodyKey is removed**: no renderer-identity logic exists anywhere in the app (`grep bodyKey` → comment-only).

**Memory**

- Full-scale soak (`5492290`, clean tree): **100,000 keyed mount/unmount cycles** over a bounded 64-key sliding window with 6,250 interleaved abort-churn passes. RSS plateaued by cycle 20k (78–80 MB across runs; no growth across 80k further cycles); State subscriber count held exactly at the live-window size (64) throughout; post-dispose subscriber count 0. Targets met: bounded live set, subscribers follow live scopes, aborted pendings reclaimed, disposal immediate. Raw console record committed as `packages/iyon-runtime/bench/PERF-12-T13.1-R10-memory-soak.log`; independently reproduced twice.

**Cleanup (Step 14R)**

- Lexical SiteId architecture: gone since R0; no Oxc/AST dependency added (`package.json` deps unchanged).
- `bodyKey`: removed (commit `a17ff2b`); spinner advance side effect preserved and pinned by `perf12_t13_1_r9_production.test.ts`.
- Dead code: `disposeFreshPending` removed (commit `814132c`). Full runtime battery before/after identical: 261 pass / 1 documented pre-existing perf11v4 weak-cache interference failure (passes isolated); app plugin suite 113 pass / 1 documented pre-existing recovery3 viewport failure (predates all T13.1 work — R9 record Finding 5); iyon-plugins packages 30/30; fixture 10/10; typecheck clean.

## 4. §31.5 sibling independence — BLOCKED BY DEFERRAL (never waived)

The 1-of-1000 same-geometry and geometry-changing end-to-end gates are **not claimed**. Per the Staged-delivery contract they are reported as blocked by the sanctioned R6b deferral, alongside the measured rescan costs at current mounted-scope counts:

| N mounted scopes | leaf update pre-scene-render | initial scene render | leaf update post-scene-render |
|---:|---:|---:|---:|
| 10 | 34.7 µs | 0.91 ms | 65 µs |
| 100 | 25.5 µs | 0.95 ms | 222.8 µs |
| 1,000 | 23.0 µs | 8.16 ms | 2,292 µs |

Pre-scene-render scoped updates are FLAT (execution + JS frontier do not scale). Once a scene embeds the projections, the unmodified Rust resolver re-resolves component-bearing branches per frame: **≈ 2.2 µs per mounted scope per update** (the AMENDMENT-C SS5.3 gap, quantified — this is exactly the "moving O(N) downstream" shape R6b exists to remove).

**Recorded revisit trigger:** when real-trace mounted-scope counts approach **N ≈ 400 projected components under one scene** (≈ 0.9 ms ≈ 5% of a 60 Hz frame budget at the measured slope), OR the PERF-12v2 transport decision lands — whichever comes first. Below that threshold the deferral posture is measurably sound: production chrome mounts single-digit projections (Working/Footer/tool cards), and the authoritative matrix shows scoped updates at parity or better end-to-end.

## 5. Oracle-vs-runtime divergence resolution (§39 standing note)

The Step-1 `manual_stable_oracle` models hand-preserved identity surviving absences: it keys each part on application-known semantic inputs and returns the exact previous root object when children are unchanged. Runtime semantics differ deliberately (§21/§22): identity follows execution scopes and tracked invalidation, not application-managed keys. Resolution recorded for closure:

- On no-op and boundary cases the runtime candidate now MEETS OR BEATS the oracle (375 vs 542 ns noop; 583 vs 1,291 ns tool-slot) because scoped execution skips work the oracle still pays to discover.
- On structural/content cases the oracle retains a 5–20% edge where its app-side key knowledge has no public-API equivalent — by design (§1.3 rejects app memoization as architecture). The oracle stays in the harness as the reuse-density upper bound; the divergence is closed as intentional, documented, and measured.

## 6. Adoption decision

**Adopt.** Retained execution is already the default path on every supported consumer route (framework-owned, invisible, no activation surface — proven by the external fixture). Evidence summary: all §31 gates pass except the two §31.5 variants, which are blocked-by-deferral with committed numbers and triggers; the one honest regression (effort-style-state, ~+47 µs absolute) is bounded and dominated by wins everywhere else; memory soak green at full scale.

Follow-ups (outside T13.1): R6b incremental host frontier after PERF-12v2 transport finalization or upon trigger; PERF-12-wide §93/T15 authoritative comparison runs under the parent tranche's own registry.
