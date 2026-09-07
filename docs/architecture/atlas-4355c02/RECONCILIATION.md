# Reconciliation and evidence qualifications — 4355c02

Status: **complete for this mapping**. Parent fully read all46 reports and evaluated consequential recommendations. Canonical report copies remain unchanged; parent synthesis and this register carry corrections.

## Authority and acceptance

- Source determines actual architecture. Approved unsuperseded PERF13/API-H/L1/PRE-V5 handoffs determine intended contracts.
- An implementation mismatch is not automatically stale documentation, nor automatically a correctness bug.
- A later approved supersession can resolve a mismatch. A claimed improvement requires comparison of original purpose, correctness, ownership, complexity, performance and maintenance. Scout recommendation is not owner approval.
- Deprecated documents outside those families, including LAY-1, are historical navigation only.
- Reports contain static test inspection, not newly executed test/benchmark results.
- The integrated report and maintained guide must use corrected claims below, not majority vote between scouts.
- Issues requiring action/proof belong in [ISSUES.md](ISSUES.md), not this errata table alone.

## Parent source-confirmed report corrections

Source prefixes: R=`crates/iyon-tui/src/`, N=`crates/iyon-tui-native/src/`, T=`packages/iyon-tui/src/`. Detailed read ranges and original quotations are in the linked parent notes.

| Reports | Incorrect/overbroad claim | Reconciled fact | Parent evidence |
|---|---|---|---|
| 03 | Scene body normalization changes every visited node | `layout_body` uses nonrecursive `map_node`; only the outer root is normalized | [03 notes](evidence/parent-notes/03-scene.md); R/scene/root.rs:1–100, R/presentation/ir.rs:1001–1040 |
| 04 | Aggregate LOC/test estimates are a consistent census | Component subtotals contradict summary; do not use approximate report totals as exact quantitative inventory | [04 notes](evidence/parent-notes/04-view-presentation.md) |
| 08 | Exact semantic TextRun is zero-copy source-page-backed | RawText/RawDomain retain pages; TextRun::exact constructs fresh Arc<str> text plus provenance range | [08 notes](evidence/parent-notes/08-semantic-content.md); source witness/run constructors |
| 08 | Restart context failure direction inverted | InsufficientRestartContext occurs when available source base is later than required restart context | Same notes; markdown.rs:141–161 |
| 09 | Projection fields pub(crate) are inaccessible outside their module | They are accessible throughout the crate; builder use is not sibling-module-enforced privacy | [09 notes](evidence/parent-notes/09-projection-smoothing.md); projection/value.rs:1–35 |
| 09 | Then diagram makes Smooth a structural mandatory stage | Then and Smooth separately implement Projector; active connector dispatch is not a generic Then pipeline | Same notes |
| 10, 27 | NativeTextSource has N-API append/replace/clear/seal payload mutations | Complete NativeTextSource implementation has constructor/control/identity/snapshot/stats/family only; production content data uses direct FFI | [10](evidence/parent-notes/10-stream-l1.md), [27](evidence/parent-notes/27-rust-wiring.md); N/tui.rs:1089–1238 |
| 10, 35, 41 | Sealing ends every possible Source mutation | Append/replace/clear/reseal reject as applicable; truncate_head has no sealed guard and can compact retained sealed content | [10](evidence/parent-notes/10-stream-l1.md), [35](evidence/parent-notes/35-stream-content.md); R/application/content.rs:2461–2526 |
| 11 | Theme::color is base-only | It resolves variants in default focus/empty-state context; unconditional variant can override base. Theme::style is base-only | [11 notes](evidence/parent-notes/11-theme.md); theme/mod.rs:206–240 |
| 13 | Output emitted before route registration is necessarily lost | Router looks up route at drain time; a queued event is delivered if registration precedes drain. Already drained unrouted events are lost | [13 notes](evidence/parent-notes/13-interaction-output.md); output/router.rs |
| 13 | PhantomData<fn()->T> avoids variance | It is covariant; its relevant ownership/auto-trait behavior differs from owning T | Same notes; output/handle.rs |
| 15 | First application frame necessarily starts with unknown presenter | Successful startup already paints an empty frame and establishes known state; later application frame can diff | [15 notes](evidence/parent-notes/15-terminal-backend.md) |
| 15 | Terminal module file count/production shadow extent consistent | Listed paths exceed stated count; shadow is test-only. Do not infer duplicate production backend | Same notes |
| 17 | Runtime pointer first checked against live environment registry | Ordinary runtime_mut dereferences first and checks header/thread; separate registry helper is not that guard path | [17 notes](evidence/parent-notes/17-native-structure-state.md); N/tui/view_abi.rs:1401–1478 |
| 17, 31, 39 | Failed host/control install leaves all old native state unchanged | host.render/set_view can mutate desired/control state before later flush/render error. Visible frame may remain old; TS bookkeeping/staged refs can also remain old | [17](evidence/parent-notes/17-native-structure-state.md), [31](evidence/parent-notes/31-structural-mutation.md), [39](evidence/parent-notes/39-slots-controls-animation.md) |
| 18, 27 | Every native wrapper sets alive=false on disposal | Connector keeps status callable after logical disposal and does not use that blanket rule | [18 notes](evidence/parent-notes/18-native-content-host.md); N/tui.rs:1344–1431 |
| 21, 34 | Flush performs up to64 silent forced retries of an error | It throws pending error after each drain;64 bounds error-free incomplete passes | [21 notes](evidence/parent-notes/21-runtime.md); runtime.ts:749–928, wake-broker.ts:161–295 |
| 22 | Duplicate clear properties are deduplicated | Duplicate clear entries throw validation errors | [22 notes](evidence/parent-notes/22-ts-structure-state.md); state/control.ts:80–97 |
| 22, 33 | StyleRef cache reset occurs before native setTheme | Reset is in finally after the native invocation; lowering failure before try does not reach reset | [22](evidence/parent-notes/22-ts-structure-state.md), [33](evidence/parent-notes/33-themes-styles.md) |
| 22, 45 | Diff semantic kind necessarily mismatches native state kind | Typed diff lowers to Column; semantic mapping to Column is correct for that representation | [22 notes](evidence/parent-notes/22-ts-structure-state.md); view_state.rs:127–139, content/diff/render.rs:1–30 |
| 24 | Runtime fingerprints prove equality of all generator inputs | Separate view-kind-codes.json is not included in schema_hash or generator_hash; output freshness has broader input coverage than runtime fingerprint comparison | [24 notes](evidence/parent-notes/24-codegen.md); render_manifest.rs, main.rs |
| 25 | renderRetained helper creates a fresh ABI session | It calls cached nativeViewAbiSession; fresh/full render is not fresh cache/runtime | [25 notes](evidence/parent-notes/25-tests-fixtures.md) |
| 25, 28 | All keyed fixture updates prove autonomous scheduling without renderApp | Keyed reorder/label cases explicitly call renderApp after state.set; narrower child/header cases exercise automatic invalidation | Same notes; consumer scoped-invalidation test |
| 26 | Staging only replaces addon after all validation succeeds | Copy precedes require/marker/content identity/nm qualification; post-copy failure can leave unqualified new artifact | [26 notes](evidence/parent-notes/26-bench-build-examples.md); stage-native.ts |
| 26, 42 | T15 direct-ffi label proves structural direct-FFI dispatch | Same N-API session/RetainedRootBoundary executes; env values label output but do not select transport | [26](evidence/parent-notes/26-bench-build-examples.md), [42](evidence/parent-notes/42-benchmark-integrity.md) |
| 26 | Rust fresh/rebuilt-equivalent benchmark modes exercise different routes | Their match arms perform the same fixture construction/rendering | [26 notes](evidence/parent-notes/26-bench-build-examples.md); perf_bench.rs:291–456 |
| 27 | NativeViewRuntime is the only native retained View owner | Host/history/component/candidate roots retain shared Views independently; runtime table is identity/lease cache | [27 notes](evidence/parent-notes/27-rust-wiring.md) |
| 29 | All wide-axis materialization uses builder | Production retained-dag uses one pair buffer; specialized helper uses builder. Name exact caller | [29 notes](evidence/parent-notes/29-three-planes.md); retained-dag.ts:515–564, native-view-abi.ts:194–303 |
| 29 | Three commit boundaries imply strict chronology and universal rollback | Desired acceptance can occur during TS commit; pathological commit failure may partially publish | Same notes |
| 29, 32 | Registry committed Arc table advances only with visible frame | Demanded mutations update latest logical Arc table immediately; candidate pins protect older frame state | [29](evidence/parent-notes/29-three-planes.md), [32](evidence/parent-notes/32-state-mutation.md); registry.rs:90–195 |
| 30 | AppHarness is a root export | Supported testing-subpath export only; public index does not export it | [30 notes](evidence/parent-notes/30-composition-root.md); T/index.ts |
| 30, 41 | Exact-root helper counter proves canonical Tui synchronous hostRenderRef route | Canonical Tui uses deferred desired-install boundary; nondeferred helper is a distinct route | Same notes |
| 32 | Every style-state patch adds both self and subtree paint | Effect is subtree paint instead of the self bit; actual repaint policy is conservative subtree | [32 notes](evidence/parent-notes/32-state-mutation.md) |
| 34, 41 | Native wait necessarily never drives pending host frames | wait_for_output -> poll_terminal -> advance_and_render -> flush_pending_frame drives local host; it is not the environment drain path | [34 notes](evidence/parent-notes/34-runtime-scheduling.md) |
| 35 | Missing ticket falls back to newest same-width projection | Exact ticket lookup uses candidate/committed/cache identities; miss/poison returns None, not newest-width substitution | [35 notes](evidence/parent-notes/35-stream-content.md) |
| 36 | Any y-sorted child layout can be pruned by top alone | Optimization requires both top and bottom monotonicity; otherwise linear z-order-preserving scan | [36 notes](evidence/parent-notes/36-layout-resize.md) |
| 37 | No native History synchronization recovery exists | Successful frame commit clears logical unknown marker; cannot reconstruct uncertain irreversible external scrollback | [37 notes](evidence/parent-notes/37-history-scrollback.md) |
| 37 | Semantic freeze is ordinary reversible state | Live->static finalization has no general inverse; distinct from physical frozen prefix retention | Same notes |
| 38 | Unchanged modal always focuses first eligible control | Eligible current focus is preserved; first eligible applies only when reconciliation requires replacement | [38 notes](evidence/parent-notes/38-input-callbacks.md) |
| 39 | Same-root slot publication skips required install | publishPrepared calls installRef even for identical retained root; no demonstrated same-identity no-op defect | [39 notes](evidence/parent-notes/39-slots-controls-animation.md) |
| 40 | Weak candidate maintenance guarantees eventual idle collection | Still-live candidates are discarded from maintenance queue; later weak expiry may need lookup/insertion/full scan | [40 notes](evidence/parent-notes/40-lifetime-caches.md) |
| 43, 45 | Rust native ABI tests absent or only generated stubs | view_abi.rs has extensive real NativeViewRuntime/generated-export unit tests in addition to separate stubs | [43](evidence/parent-notes/43-test-contracts.md), [45](evidence/parent-notes/45-coverage-reconciliation.md); N/tui/view_abi.rs:4591–6274 |
| 43, 45 | Missing addon can silently pass all guarded tests; nativeViewAbiSession returns undefined | Eager addon require/identity errors throw before test bodies; session accessor errors throw. Loaded partial addon Host guard/ref-miss guards require separate treatment | [43 notes](evidence/parent-notes/43-test-contracts.md); addon.ts:236–248, session accessor:98–128 |
| 44 | history module is bare private mod | lib.rs declares pub(crate) history; neither form makes it external authoring API | [44 notes](evidence/parent-notes/44-history-doc-drift.md) |
| 45 | Source manifest count1031 | Original collected manifest has1082 paths; final Git baseline comparison has1088, with six nonproduction/config omissions fully read and recorded in coverage addendum; report appendix is not full manifest | [45 notes](evidence/parent-notes/45-coverage-reconciliation.md) |

## Evidence distinctions that survive reconciliation

1. Capability existence is not production-route reachability.
2. Public source import is not a separately packaged consumer qualification.
3. Test assertions read are not tests run.
4. Headless screen correctness is not actual terminal-byte/scrollback correctness.
5. Retained-versus-fresh comparison can share one implementation oracle.
6. Generated wrapper/manifest agreement is not independent native semantic validation; real native unit coverage must be counted separately.
7. Counter ceilings prove only the instrumented dimension; zero parser work on a pure pacing tick does not prove zero Source snapshot work across the entire host frame.
8. Historical benchmark artifacts prove their recorded run, not baseline4355 performance.
9. Semantic immutability, native strong ownership, TS resource leases and explicit disposal permission are separate contracts.
10. A local invariant gap or algorithmic counterexample is not automatically a reachable public defect.
11. Old logical visible state after failure does not undo accepted desired state, callback side effects, output consumption or irreversible native writes.
12. Exact counts require a nonoverlapping classified census; report LOC estimates and duplicated cross-cutting paths are navigation, not that census.

## Coverage still requiring explicit closure

- Report46 full reading and source inspection: complete; [parent qualifications](evidence/parent-notes/46-grouped-deviation-followup.md) reject overstrong A5/B4/C4 closure or impact claims.
- Declaration-level proof for merged ViewSlot public type exposure.
- Native pointer/thread ownership safety argument beyond affinity comments.
- Reachable lifetime chain for shrinking cleanup-loop bound after production pre-cleanup.
- Nested content/state History invalidation and physical wide-cell counterexamples through actual producers.
- Benchmark/ABI fingerprint provenance limits and handoff deviation dispositions.
- Final documentation checks: recorded separately in [validation evidence](evidence/final-validation.md). Open technical proofs above are not documentation-completion blockers.
