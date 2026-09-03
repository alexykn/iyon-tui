# POST-PERF13-ROT-CLEANUP.md

**Status:** cleanup complete with 3 recorded owner blockers (R0-B001..R0-B003)
**Tranche:** PRE-V5-R0 (CLEAN2.md), executed against the finished post-PERF-13 `iyon-tui`
**Rule applied:** one semantic operation → one authoritative production path.
Failure means failure; no previous-generation implementation may make the
operation succeed.

Result: **all automatic architecture fallbacks are deleted from production.**
The retained structural path is the single production structural
architecture; state and content planes were already single-path and are
unchanged. 96/96 TypeScript tests pass, `check:ownership` passes,
`check:tui-abi` passes, `tsc --noEmit` is clean. The architecture census is
gated ONLY on the 3 blockers in §9 (two small ABI extensions + one physical
deletion tranche). No blocker is embedded as compatibility code.

---

## 0. Result

CLEAN2 exit criteria (§42): 24 of 27 hold. The 3 that do not are exactly the
recorded blockers — genuine retained-ABI expressiveness gaps (text spans
>4, custom border glyphs) and the physical deletion of now-unreachable
oracle/native-decode code. Every other criterion holds, including:

```text
STRUCTURE = one authoritative path (retained; refusal fails explicitly)
STATE     = one authoritative path (transport/state, untouched)
CONTENT   = one authoritative path (transport/content, untouched)
```

---

## 1. Rot discovered

| ID | Area | Conflicting paths | Result |
|---|---|---|---|
| ROT-001 | root publication (`runtime.ts`) | `prepareDesiredInstall` vs `?? prepareColdInstall` (complete bridge decode, no paint) | FIXED |
| ROT-002 | ViewSlot (5 sites: seed, direct set, transactional set, animation frames, stop-animation) | `tryRetainedMaterializeRef` vs `?? tryNativeMaterialize` (bridge lower + `tuiViewAbiDecodeRef`); `prepareInstall` vs `?? prepareColdInstall` | FIXED |
| ROT-003 | ScrollPane (3 sites: seed, transactional set, direct set) | same dual pair as ROT-002 | FIXED |
| ROT-004 | History push/freeze | `tryRetainedMaterializeRef` vs `?? tryNativeMaterialize` | FIXED |
| ROT-005 | retained recovery (`retained-dag.ts`, `renderExactRoot`) | retained stale-retry vs per-node `decodeRef(lowerSemanticView(node))` and exact-root decode fallback | FIXED |
| ROT-006 | heuristic refusal budgets | 512 new nodes / 256 depth / 1024 axis children / 64 KiB payloads → old transport (CLEAN2 §18 forbids performance-heuristic architecture switches) | FIXED |
| ROT-007 | byte-tier allocation | fixed 64 KiB upfront grab regardless of payload | FIXED (exact-size requests) |
| ROT-008 | text spans >4 | retained constructors cover 1..=4; wider text routed to cold decode | FIXED as explicit refusal; ABI extension → R0-B001 |
| ROT-009 | custom border glyphs | no retained decorated-word lane; routed to cold decode | FIXED as explicit refusal; ABI extension → R0-B002 |
| ROT-010 | ownership gate `h1j-contract-parity` | gate PINNED the deleted fallback (`tryNativeMaterialize` + decode required) | FIXED (gate now asserts the single path) |
| ROT-011 | authoritative benchmark (`perf12_t15_authoritative_case.ts`) | `prepareInstall ?? prepareColdInstall`, else `renderColdRef` + adopt | FIXED (retained-only; refusal throws) |
| ROT-012 | tests asserting cold behavior | h3_c refusal test, perf13_b 5-span test, perf13_a cold bootstrap | FIXED (rewritten, see §6) |

No entry ends as “accepted compatibility”.

---

## 2. Fallbacks deleted

Every automatic switch from the retained architecture into the
previous-generation complete-bridge decode:

```text
Tui.prepareRootPublication:            prepareDesiredInstall ?? prepareColdInstall  → explicit TUI_ROOT_PREPARATION_FAILED
ViewSlot/ScrollPane/History (10 sites): tryRetainedMaterializeRef ?? tryNativeMaterialize → retained-only, explicit failure
ViewSlot/ScrollPane (4 sites):          prepareInstall ?? prepareColdInstall → retained-only, explicit failure
retained-dag: recoverNodeWithDirectDecode, renderExactRoot decode fallback,
              recoverStaleNode decode tail → retained retry only, then explicit failure
retained-dag: prepareColdInstall, prepareDesiredColdInstall, releaseColdLease,
              COLD_ROOT_MATERIALIZER, setRootColdMaterializer → deleted (127 lines)
native-view-abi: tryNativeMaterialize, renderColdRef → deleted;
              internal axis/grid child materialization → tryRetainedMaterializeRef
runtime bootstrap: setRootColdMaterializer wiring → deleted
```

Version mismatch still fails explicitly (`native View N-API metadata is
incompatible`) — that was already the target behavior, unchanged.

---

## 3. Unmigrated consumers migrated

```text
runtime.ts            root publication is retained-only
view-slot.ts          seed / setView / prepareSetView / animation / stopAnimation retained-only
scroll-pane.ts        seed / prepareSetContent / setContent retained-only
history.ts            push / freeze retained-only
bench authoritative   retained-only, refusal throws (CLEAN2 §25 shape)
native axis/grid/edit helpers (bench/test-only, no production callers):
                      child materialization retained-only
```

State plane (`transport/state`) and content plane (`transport/content`)
needed no migration: grep confirms no structural reroutes and no
catch-to-older-architecture in either transport. Plane purity holds:

```text
structure change → structural plane (retained-dag)
state change     → state plane (ViewState deltas; no View rebuild)
content change   → content plane (Source/Port/Connector; no composition)
```

---

## 4. Architecture deleted

Production code removed (net −173 lines across 15 files; 251+/424−):

```text
retained-dag.ts     cold boundary methods, decode recovery, budgets, cap
                    accounting (−357 changed lines in file)
native-view-abi.ts  tryNativeMaterialize, renderColdRef, cold import (−63)
policy.ts           refusal budgets/caps → native-limit documentation
runtime/controls    15 fallback selection sites → single path
ownership gate      stale dual-architecture clause → single-path assertion
```

Approximate production LOC removed: ~300 (mostly retained-dag cold
machinery + fallback branches). Added: ~120 (explicit-failure comments,
blocker annotations, exact-size byte accounting).

NOT yet physically deleted (zero production importers/callers, recorded
under R0-B003): `cold-lowering.ts` + `BridgeViewNode`/`ir.ts` (test/bench
oracle), `tuiViewAbiDecodeRef` native export + Rust bridge-decode graph,
`retained-path.ts` path-recipe constructors (test/bench-only),
`NativeViewRoute "fallback"` member (never recorded now),
`RetainedFastFallbackError` name (kept: generated ABI code imports it;
now means explicit retained refusal), `bridge_*` counter names (measure
the authoritative path).

---

## 5. Authoritative route table

| Operation | Production paths | Authoritative PERF-13 path | Deleted | Consumers migrated |
|---|---|---|---|---|
| root publication / replacement | 1 | `prepareDesiredInstall` → `ensureSemanticNative` → direct ABI constructors | `prepareColdInstall`, decode fallback | runtime.ts |
| structural node materialization | 1 | `MATERIALIZERS` per-kind constructors | per-node cold decode | — |
| structural node reuse | 1 | hint → NodeId promotion → derivation → materialize | decode recovery | — |
| stale-reference recovery | 1 | invalidate hint → one retained retry → explicit failure | decode fallback | — |
| ViewSlot create/seed/update/animation/reset | 1 | boundary `prepareInstall` / `tryRetainedMaterializeRef` | cold materializer pairs | view-slot.ts |
| ScrollPane create/seed/update | 1 | same as ViewSlot | cold materializer pairs | scroll-pane.ts |
| History push/freeze | 1 | `tryRetainedMaterializeRef` → `pushRef`/`freezeRef` | `tryNativeMaterialize` pair | history.ts |
| state mutation (geometry/presentation) | 1 | `transport/state` deltas | none existed | — |
| content append/replacement | 1 | Source/Port/Connector | none existed | — |
| frame publication | 1 | wake-broker + `commitVisible` | none existed | — |

---

## 6. Tests

Route integrity is asserted, not just pixels (CLEAN2 §26):

```text
h3_c: 2,000-child axis renders retained (derivation_fast_path_calls == 1,
      children_visited == 0, cold objects == 0) — previously cold-routed
h3_c: 64 KiB+NUL text renders on the retained exact-byte lane with
      direct_materializer_calls > 0 — previously cold-routed
h3_c: 5-span text throws TUI_ROOT_PREPARATION_FAILED with zero bridge
      allocation — previously silently cold-routed (R0-B001)
perf13_b: 4-span styled text + presentation state on the retained path with
      direct_materializer_calls > 0 (renamed from "cold fallback")
perf13_a: desired/visible seam test without the cold bootstrap hook
```

Full suite: 96 pass / 0 fail across 27 files. Breaking the retained path
(e.g. restoring a span/glyph refusal, removing a materializer) fails these
tests; no second implementation can make them pass — the fallback branches
they would have taken no longer exist in production.

Deleted/rewritten obsolete-implementation coverage: the old “refusal uses
cold lowering” assertion, the 5-span cold-render assertion, the cold
bootstrap line. Oracle-style tests that never executed production fallback
(h3_a bridge comparisons, h3_b derivation checks, values, differential,
fuzz) are untouched and still pass.

---

## 7. Benchmarks

`perf12_t15_authoritative_case.ts` now asserts the intended route: retained
publication or benchmark failure (CLEAN2 §25). Historical `.jsonl` reports
are retained as records; executable old architecture they reference is not
 production-reachable. `bench/direct_ffi` oracle copies are untouched
(explicit qualification tooling, no production imports). Obsolete-benchmark
deletion beyond the authoritative case is deferred to the census.

---

## 8. Temporary observability removed

No temporary counters were added (existing `direct_materializer_calls`,
`derivation_fast_path_calls`, cold-oracle allocation counters sufficed as
proof). Deleted after proving cleanup: `cold_fallbacks` counter field and
all ~15 increment sites, `COLD_ROOT_MATERIALIZER` hook, `NativeViewRoute`
`"fallback"` recording (member retained for bench-type compat, never
recorded). `cold_bridge_objects_allocated` remains in the oracle module,
asserted `== 0` by route tests.

---

## 9. Blockers

```text
R0-B001  N-span retained text constructor. The retained ABI exposes 1..=4
         span constructors; wider styled text fails explicitly. Needed: a
         variadic text-buffer constructor (words+bytes lane, mirroring
         viewDiffCreateBuffer) + ABI regen + snapshot updates. Only known
         production-shape impact: >4-span View.styledText.
R0-B002  Custom-glyph decorated lane. The retained decorated-word encoding
         has no glyph lane; custom border glyphs fail explicitly. Needed:
         glyph-capable decorated constructor or explicit product decision
         that structural borders stay glyph-free (content plane owns rich
         borders). No current test renders custom glyphs productively.
R0-B003  Physical deletion tranche. Zero-reachability code awaiting removal:
         cold-lowering.ts, ir.ts BridgeViewNode, bridge-schema.json,
         retained-path.ts test-only constructors, native tuiViewAbiDecodeRef
         + Rust bridge-decode graph, "fallback" route member, bridge_*
         counter renames. Requires rewriting the oracle-consumer tests
         (h3_a, h3_b, values, bridge_worker, fixtures, fuzz) + ABI regen.
```

Any blocker prevents the architecture census per §42. None is embedded as
compatibility code; all surface as explicit failures.

---

## 10. Final verification

```text
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
  → 96 pass, 0 fail, 27 files
bun run check:ownership       → ALL OWNERSHIP CHECKS PASSED
cargo run -q -p tui-abi-gen -- check → clean (exit 0)
tsc --noEmit                  → clean apart from 2 pre-existing lib.dom
                               TextDecoder/TextEncoder variances, identical
                               on the unmodified baseline (environmental:
                               TS 5.8.3 lib vs bun-types; unrelated to this
                               change)
bun run check:tui-declarations → fails identically on baseline and branch:
                               sandbox denies bunx/tsc temp writes
                               (environmental). Compensating evidence:
                               ts-surface-snapshot passes — 44 value + 97
                               type public exports byte-identical to S0.
```

V5 handoff note: the foundation now matches the v5 plane contract —
structure/state/content each mutate through exactly one vocabulary, bulk
content never touches React/composition, and refusals are explicit instead
of hidden second architectures. The R0-B001/B002 ABI extensions slot
directly into the retained materializer table without touching the
boundary/lease/epoch machinery.
