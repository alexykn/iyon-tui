# POST-PERF13-ROT-CLEANUP.md

**Status:** cleanup complete, all blockers resolved (R0-B001, R0-B002 extended
the road; R0-B003 demolished it)
**Tranche:** PRE-V5-R0 (CLEAN2.md), executed against the finished post-PERF-13 `iyon-tui`
**Rule applied:** one semantic operation → one authoritative production path.
Failure means failure; no previous-generation implementation may make the
operation succeed.

Result: **the retained structural path is the single production structural
architecture with no expressiveness gaps and no oracle residue.** B001 adds
the variadic N-span text constructor, B002 adds the custom-glyph decorated
lane, and B003 physically deletes the previous-generation JSON-decode graph,
the cold-lowering oracle, obsolete benchmarks, and oracle-bound tests (all
rewritten against the authoritative path). 98/98 TypeScript tests pass,
`check:ownership` passes (extended with B003 absence assertions),
`check:tui-abi` passes, `tsc --noEmit` is repo-clean, `cargo` is
warning-free, and the native crate tests pass. The architecture census is
UNGATED.

---

## 0. Result

CLEAN2 exit criteria (§42): all 27 hold. R0-B001 and R0-B002 are resolved by
ABI extension (full parity: every View the API can construct materializes on
the retained lane); R0-B003 is executed (physical deletion tranche, §10).
The only refusals left in production are for inputs with no valid rendering
(empty span lists, incomplete glyph sets, malformed payloads) — not for
expressible operations.

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

NOT yet physically deleted at tranche close (zero production
importers/callers, recorded under R0-B003): `cold-lowering.ts` +
`BridgeViewNode`/`ir.ts` (test/bench oracle), `tuiViewAbiDecodeRef`
native export + Rust bridge-decode graph, `retained-path.ts`
path-recipe constructors (test/bench-only), `NativeViewRoute
"fallback"` member (never recorded now), `RetainedFastFallbackError`
name (kept: generated ABI code imports it; now means explicit retained
refusal), `bridge_*` counter names (measure the authoritative path).
All of these except the `retained-path.ts` constructors were removed in
the follow-up residue eradication recorded in §9 (dead types deleted,
generated duplicate materializer deleted at the source-of-truth level,
`fallback` metadata deleted, counters/helpers renamed, schema file
renamed, dead `build.rs` block deleted).

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
      children_visited == 0) — previously cold-routed
h3_c: 64 KiB+NUL text renders on the retained exact-byte lane with
      direct_materializer_calls > 0 — previously cold-routed
h3_c: 6-span styled text + trailing NUL renders on the retained buffer
      lane (R0-B001) — previously refused
h3_c: custom "*" border glyphs replace the named style on the retained
      decorated lane (R0-B002) — previously refused
perf13_b: 4-span styled text + presentation state on the retained path with
      direct_materializer_calls > 0 (renamed from "cold fallback")
perf13_a: desired/visible seam test without the cold bootstrap hook
fuzz: malformed buffer framings + unknown layout codes fail without host
      mutation; live NodeId resolves from the semantic cache without
      reading the payload
values: malformed buffer payloads fail at the retained boundary; worker
      lifecycle test drives the current N-API surface
h3_a: 15 sample Views compare against hand-written semantic literals
      (ids stripped); normalizers pinned to backend-neutral goldens
```

Full suite: 98 pass / 0 fail across 27 files. Breaking the retained path
(e.g. restoring a span/glyph refusal, removing a materializer) fails these
tests; no second implementation can make them pass — the fallback branches
they would have taken no longer exist anywhere in the tree.

Rewritten obsolete-oracle coverage: every differential test now compares
within the authoritative architecture (incremental edit ≡ retained full
publish via the renderRetained fixture) or against hand-written literals.
The cold-lowering module, its counters, the native decode export, and the
bench oracle copies are deleted; nothing references them.

---

## 7. Benchmarks

`perf12_t15_authoritative_case.ts` asserts the intended route: retained
publication or benchmark failure (CLEAN2 §25), with `perf12_t15_workload.ts`
as its scenario builder and `perf13_h_content.ts` as the live content bench.
All other PERF-12 benches are deleted (direct/realistic/memory/multi_edit
cases + runners, dispatcher, s6 ×2, t13 frontier) along with the entire
`bench/direct_ffi` oracle-copy tree. Historical `.jsonl` reports are
retained as records. The only benchmark with a standing mandate is the
authoritative route assertion. Follow-up: the differential
`perf12_t15_authoritative.ts` runner (napi-vs-`direct_ffi_oracle`
comparison staging `ION_NATIVE_FEATURES=direct-ffi`, referencing the
deleted `perf12_t15_case.ts`, zero referrers) is deleted and pinned in
the ownership `removedPaths`; the uncalled `makeT15Pair` helper and its
"Compatibility" comment go with it.

---

## 8. Temporary observability removed

No temporary counters were added (existing `direct_materializer_calls`,
`derivation_fast_path_calls`, cold-oracle allocation counters sufficed as
proof). Deleted after proving cleanup: `cold_fallbacks` counter field and
all ~15 increment sites, `COLD_ROOT_MATERIALIZER` hook. B003 additionally
deleted the entire `NativeViewRoute` counter apparatus (type, counters,
record/reset/snapshot, both runtime call sites) — its only consumer was the
deleted s6 bench — and `cold_bridge_objects_allocated` with the oracle
module itself.

---

## 9. Blockers — all resolved

```text
R0-B001  RESOLVED by extension (owner: full parity). New ABI function
         view_text_create_buffer (words [span_count, per span
         (style_ref, byte_length)] + concatenated UTF-8 bytes), mirroring
         view_diff_create_buffer: same 1_048_576/262_144 limits,
         semantic-cache-first, same text rules. Span counts 1..=4 keep
         their fixed-arity fast lanes; wider text (incl. NUL-bearing)
         rides the buffer lane. ABI regen + snapshot + function-count pin
         59→60 updated. Proven by h3_c wide-text test (6 styled spans +
         trailing NUL, retained counters, pixels) and the fuzz
         cache-skips-payload test.
R0-B002  RESOLVED by extension (owner: full parity). New mask bit
         DECORATION_GLYPHS (0x400) + glyph trailer between the state count
         and the style-state entries: count (always 8) + 8
         (offset,length) pairs in top/right/bottom/left/topLeft/topRight/
         bottomLeft/bottomRight order, glyph bytes after state bytes.
         Native replaces the named style with BorderSpec::custom exactly
         like the old decoder (style/edges codes still validated, then
         edges + color apply). No TOML signature change. Proven by h3_c
         custom-glyph test (plain style + "*" glyphs reach the pixels).
R0-B003  EXECUTED. Deleted: cold-lowering.ts; Rust decode graph
         (decode_view, tui_view_abi_decode_ref napi, ViewDecoder, all
         decode_*; test-only lower_* view family + its 2 tests;
         apply_decoration; view_bridge_cache/with_view_runtime trio;
         tui_perf_inc/add macros; tui_bridge_schema include) +
         publish_decoded_view/record_decoded_semantic_view; addon
         tuiViewAbiDecodeRef contract entry; bench/direct_ffi (7 files),
         12 obsolete bench files (direct/realistic/memory/multi_edit
         cases+runners, dispatcher, s6 ×2, t13 frontier); route-counter
         apparatus; "fallback" route member. Restored lower_style_spec
         (live theme pipeline, was sharing the deleted region) with a
         clarifying doc comment. Renamed RetainedFastFallbackError →
         RetainedRefusalError, FAST_FALLBACK → FAST_REFUSED, fallbackId →
         ancestorDefault. Rewrote all oracle tests to authoritative
         assertions (edit ≡ full publish via renderRetained; literals
         where crisp). Ownership gate extended with B003 absence
         assertions (deleted paths + 30 forbidden identifiers).
```

Deliberately KEPT (investigated, not overlooked):
- `retained-path.ts` — verified clean: production imports types only;
  runtime helpers are current-arch path-patch test scaffolding, with no
  cold/decode/bridge references (one historical comment word).
- `decode_wrap/decode_align` Rust scalar parsers — normal
  value-parsing terminology, no architectural meaning.
- `hotness` labels — per-function ABI performance metadata orthogonal
  to fallback routing; retained.
- `BRIDGE_*` numeric code tables (`BRIDGE_VIEW_KIND`,
  `BRIDGE_LAYOUT_CHILD_KIND`, grid/wrap/align/diff codes in `ir.ts`)
  — load-bearing discriminant tables consumed by the live
  retained materializers (`encoding.ts`) and the ABI generator's enum
  validation. Only the names are legacy.

Follow-up residue eradication (post-tranche; everything below was listed
as kept-or-deferred above and has since been removed):
- `bridge-schema.json` RENAMED to `view-kind-codes.json` (content
  byte-identical); TOML enum sources, tui-abi-gen paths/names, and the
  `ir.ts` import/binding renamed with it.
- `ir.ts` dead full-view tree types DELETED (`BridgeViewNode`,
  `BridgeViewNodeDraft`, `BridgeLayoutChild`, grid/diff/overflow node
  types, `DecorationNode`, `DiffRangeNode`, `InsetsNode`,
  `VIEW_BRIDGE_SCHEMA_VERSION`, `BRIDGE_OVERFLOW_KIND`). The live theme
  atoms (`ColorNode`, `StyleNode`, `TextSpanNode`, `BorderNode`) and
  the numeric code tables stay.
- Generator `[[materializer]]` specs + `view_materialize.ts` DELETED:
  TOML blocks, model structs/roles, validation section, manifest and
  TypeScript rendering, generator tests, insta snapshot (regenerated),
  and all regenerated outputs. The duplicate materializer is gone at
  the source-of-truth level, not just at the file level.
- `fallback` ABI metadata DELETED: model field, all 63 TOML lines,
  manifest JSON, Rust `FunctionDescriptor` field + table literals,
  human-reference column, and `explain` output. Nothing read it at
  runtime (verified: zero readers outside generated literals).
- `bridge_*` TS counters RENAMED to `retained_*`
  (`retained_hint_hits/misses`, `retained_semantic_nodes_inspected`,
  `retained_children_visited`); authoritative bench/tests updated.
- `tryNative*` structural helpers RENAMED to `tryRetained*`
  (axis/grid/edit retained-only helpers + their test call sites).
- `build.rs` dead schema-constants block DELETED (it generated
  `tui_bridge_schema.rs` into OUT_DIR which nothing includes; numeric
  codes ship through tui-abi-gen instead) along with its now-unused
  `serde_json` build-dependency. `fn main` keeps `napi_build::setup()`.
- Ownership gate extended with absence assertions for all of the above
  (deleted file pinned in `removedPaths`; ~30 identifiers added to the
  `forbidden` production-source regex).
- Stale execution vocabulary removed without behavior change:
  `ViewStateFullDamageFallbacks`/`ViewStateGeometryFullFallbacks`
  counters → `…Repaints` (same-architecture damage escalation, not a
  second path), `cold_host` test fixtures → `fresh_host`, one
  "structural/component fallback" comment → "change", one
  "cold-frame parity" comment → "first-frame parity". The differential
  bench runner deletion above is pinned in `removedPaths` too.
- Second vocabulary sweep (no behavior change): `BRIDGE_*` kind-code
  tables → `NATIVE_*` (+ `bridgeViewKind` → `nativeViewKind`);
  `hotness = "cold"` → `"rare"` with ABI regen (schema hash rollover,
  native restaged, snapshots accepted); `perf_bench` `COLD` pattern →
  `FRESH`; content-plane `cold` membership → `inactive`, `fallback`
  rollback slots → `rollback`; `bridgeColor` → `transportColor`;
  `tuiViewBridgeEnvironmentCount` → `tuiViewEnvironmentCount` (native
  restaged); `ComponentAdapterBridge` → `AsyncComponentAdapter`;
  stale "Direct decoder"/"bridge schema"/"test-oracle lowering" comments
  reworded to the live architecture; `legacy_*` test names reworded to
  what they assert. Deliberately kept: absence-guard regexes (they must
  name the deleted things to forbid them), `imageFallback` (public
  content-model domain term), `*-compatible`/`incompatible-*`
  validation taxonomy, `retention_compatible`, vendored termwiz color
  fallback, `ION_TUI_NATIVE_ARTIFACT`, and the mandated `direct-ffi`
  feature surface.

---

## 10. Final verification

```text
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
  → 98 pass, 0 fail, 27 files (+2: wide text, custom glyphs)
bun run check:ownership       → ALL OWNERSHIP CHECKS PASSED
cargo run -q -p tui-abi-gen -- check → clean (exit 0)
cargo test -p tui-abi-gen     → 7 pass (27 at tranche close; materializer
                               generator tests deleted with the duplicate
                               materializer in the follow-up)
cargo test -p iyon-tui-native → 36 pass + 5 pass + 1 pass (lib + integration)
cargo check/fmt               → 0 errors, 0 warnings, fmt clean
tsc --noEmit                  → repo-clean (2 pre-existing lib.dom
                               TextDecoder/TextEncoder variances, identical
                               on the unmodified baseline)
bun run check:tui-declarations → probe compiles with zero non-lib errors;
                               gate exit still blocked by the same 2
                               environmental lib.dom variances above
```

V5 handoff note: the foundation now matches the v5 plane contract —
structure/state/content each mutate through exactly one vocabulary, bulk
content never touches React/composition, and refusals are explicit instead
of hidden second architectures. The census is ungated: every View the API
can construct materializes on the retained lane, and no second runtime
road remains to confuse the inventory.
