# T7 post-tranche deletion ledger

## Authority and method

Requested cleanup: audit obsolete files, code, paths, tests, benchmarks and
supporting infrastructure; parent independently verifies findings before Luna
deletions, then reviews functionality and commits. Baseline: `b2a8d1c` on
`agent/dom-occurrence-runtime`. This ledger is the deletion authority; scout
recommendations alone are not approval.

Four read-only Luna reports came from workflow
`37abf0d7-7298-4081-8985-9592ef6dda49`: `cleanup/scout-rust.md`,
`scout-typescript.md`, `scout-tests-bench.md`, and `scout-infra.md`, under that
workflow's durable output directory. Parent read each report and independently
inspected definitions, callers, module declarations, package exports, native
wrappers, feature boundaries and enforcement code. Repository-wide symbol
searches covered `crates`, `packages`, and `tools`, including inline tests.
No absence finding relies only on a default-feature build.

One writer/build owner at a time in the main checkout. No clones/worktrees,
Linux builds, new target directories, commits by children, or pushes. Parent
commits only after final review and functionality checks. Preserve user changes
and historical measurement provenance. This cleanup does not waive or expand
the previously recorded performance-comparison limitations.

## Approved: Rust/runtime phase

| ID | Parent-verified deletion and constraints | Implementation/evidence |
|---|---|---|
| R-PERF-01 | Remove the 24 orphan `Counter` variants listed below and matching names in `crates/iyon-tui/src/perf.rs`. Remove their three unowned selections from `packages/iyon-tui/bench/react_content.ts`. Native snapshots encode names, not a public packed ordinal ABI; no counter schema hash/codegen owner was found, so do not invent one. Preserve all live counters, opt-in exports and default-build timer exclusion. Do not rewrite old raw reports as new measurements. | Implemented: removed all 24 orphan Counter variants and matching NAMES entries, plus the three unowned benchmark selections. Evidence: /tmp/t7-cleanup-static-3.log (absence and live-lane checks); /tmp/t7-cleanup-cargo-check.log (all-feature core check passed); /tmp/t7-cleanup-typecheck.log (TypeScript check passed). |
| R-PERF-02 | Delete unused `perf::set`. Remove the blanket module `allow(dead_code)` if appropriate feature gating of measurement-only functions makes it unnecessary; do not delete used test counters or introduce replacement blanket allowances. | Implemented: removed unused perf::set and the module blanket dead_code allowance without adding a replacement blanket allowance. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-cargo-check.log. |
| R-UTIL-01 | Delete `_pulldown_options` and its now-unused `Options` import in `content/text/markdown.rs`. Actual parsing at `Parser::new_with_broken_link_callback` still calls `MarkdownOptions::pulldown`. | Implemented: removed _pulldown_options and the unused Options import; MarkdownOptions::pulldown remains in the parser path. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-cargo-check.log. |
| R-UTIL-02 | Delete `StyleStates::iter` in `presentation/api/style.rs`. Keep `StyleAssignments::iter`, which is called by `StyleFacts::iter`; remove its redundant dead-code allowance. Keep selector state lookup/overlay. | Implemented: removed StyleStates::iter while retaining StyleAssignments::iter for StyleFacts::iter and selector state lookup/overlay. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-rust-fmt-3.log; /tmp/t7-cleanup-cargo-check.log. |
| T-001 | Delete the one-newline `component/slot.rs` and `mod slot;`. It defines no items. | Implemented: removed the empty component/slot.rs module and declaration. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-cargo-check.log. |
| T-002 | Delete unused MountGraph `len`, `is_empty`, `to_nodes`, `same_topology`, `reparent_roots`, `update_revision`, `subtree_ids`, `replace_subtree`. Only the dead replacement helper calls the dead subtree helper. Update the obsolete incremental replacement comment. Keep live graph traversal, lookup, construction and ancestry behavior. | Implemented: removed only the uncalled MountGraph compatibility/replacement helpers and updated the stale replacement comment; live graph traversal, lookup, construction and ancestry remain. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-cargo-check.log. |
| T-006 | Delete old private N-API `NativeTextInput`/`NativeTuiOutput` classes and host `textInput`, `route`, `interceptPaste` wrappers, their TS addon contracts/constructors and direct unused imports/helpers. Supported package exports expose React Editor, not these raw-addon classes. Remove the obsolete wrapper-specific `native_text_input_owns_unicode_cursor_state` test and the positive `NativeTextInput` disposal regex assertion in `tools/ownership/check.ts`; replace the latter with a narrow absence guard for the removed classes/host methods in the existing native-surface prohibition owner. Keep current Editor/ref Unicode/input/retirement tests and actual core control behavior. | Implemented: removed NativeTextInput/NativeTuiOutput, direct textInput/route/interceptPaste N-API wrappers/contracts/constructors, the obsolete wrapper test, and replaced the ownership positive assertion with a narrow absence guard. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-ownership-2.log (all ownership checks passed); /tmp/t7-cleanup-typecheck.log; /tmp/t7-cleanup-cargo-check-native.log. |
| T-007 | Delete unused private N-API wrappers and TS declarations `uiContentVisible`, `disposeContentResources`, `nextWakeMs`, `pollTerminal`. Keep underlying Rust methods that serve real terminal scheduling, content/barriers or tests. Keep `waitForUiPresentation`, `waitForOutput`, harness dispatch/flush, `contentPort`, `bindKey`, and `interceptPasteUi`. | Implemented: removed only the unused uiContentVisible, disposeContentResources, nextWakeMs and pollTerminal N-API wrappers/declarations; core methods and current barriers remain. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-ownership.log; /tmp/t7-cleanup-typecheck.log; /tmp/t7-cleanup-cargo-check-native.log. |
| R-TEST-ORPHAN-HOST | Delete only the uncalled test hooks and their associated test-only state/branches enumerated below. Receipt submission and settlement must always use the actual backend; preserve production lifecycle/error transitions and live receipt/latch hooks. | Implemented: removed the enumerated uncalled host test hooks, test-only receipt/failure/control state and injection branches; actual backend receipt submission/settlement remains. Evidence: /tmp/t7-cleanup-cargo-test-host.log (3 focused host tests passed); /tmp/t7-cleanup-cargo-check.log. |
| R-TEST-ORPHAN-CONTENT | Delete only the uncalled test hooks and test-only mirror/poison state enumerated below. Preserve production cleanup, subscription membership, poison handling and retry. | Implemented: removed the enumerated uncalled content test hooks, mirrors, visibility helpers and poison-injection state/branch; production cleanup, membership and poison handling remain. Evidence: /tmp/t7-cleanup-cargo-test-content.log (6 focused content tests passed); /tmp/t7-cleanup-cargo-check.log. |
| R-TEST-ORPHAN-MISC | Delete unused `TickScheduler::next_timeout`, `TextSpan::source_page_ptr`, theme resolver `with_layers`, focus `modal_restore_is_empty`, and text input `move_up_in_rows_for_test`. These are test-only methods with no test/feature/native consumers. Preserve adjacent used helpers. | Implemented: removed the five uncalled cfg(test) helpers and the now-unused Range import, preserving adjacent helpers. Evidence: /tmp/t7-cleanup-static-3.log; /tmp/t7-cleanup-cargo-test-host.log; /tmp/t7-cleanup-cargo-test-content.log. |

R-PERF-01 variants (independent exact-symbol search found definitions only):
`ResolverNodesVisited`, `MeasureNodeCalls`, `PrepareNodeCalls`,
`LayoutNodesEmitted`, `PaintNodesVisited`, `PaintCellsAllocated`,
`HistoryUnitsExamined`, `HistoryUnitsMeasured`, `HistoryCachedHeightHits`,
`PersistentSeqNodesAllocated`, `PersistentSeqLeafClones`,
`PersistentSeqBranchClones`, `ComponentGeometryNodesVisited`,
`DecoratedNormalizedNodes`, `AnnotationRecordsCopied`, `SemanticPreparations`,
`GlobalCacheClears`, `ContentSurfaceClones`, `TextBytesCopied`,
`ContentDirtyRecordsMarked`, `ContentMetricEvaluations`, `ContentMetricChanges`,
`ContentPaintPropagations`, `ContentPathIndexNodesVisited`.

R-TEST-ORPHAN-HOST (`application/host.rs`):
- Methods: `fail_next_frame_for_test`, `mark_backend_stopped_for_test`,
  `install_test_final_receipt`, `install_test_history_receipt`, `ui_history_len`,
  `prepare_test_candidate`, `install_test_bootstrap_receipt`,
  `test_ui_control_keys_visited`.
- Associated test-only fields, initializers and injection/extraction branches:
  `fail_next_frame`, `final_backend_receipt`, `test_history_receipt`, and the
  `ui_control_keys_visited` mirror. The production `Counter::UiControlKeysVisited`
  remains live. Preserve `install_test_content_latch`, `install_test_layout_latch`,
  `install_test_in_flight`, `install_before_wait_hook`, normal bootstrap and
  in-flight presentation state.

R-TEST-ORPHAN-CONTENT (`application/content.rs`):
- Methods: `subscriber_count`, `test_ui_demand_nodes_visited`,
  `test_ui_owner_nodes_visited`, `test_ui_adapter_count`,
  `test_ui_confirmed_connector`, `test_port_count`, `test_connector_count`,
  `set_connector_visible_record`, `set_connector_visible`,
  `poison_connector_for_test`, `clear_connector_poison_for_test`,
  `poison_source_after_first_cleanup_for_test`, `clear_source_poison_for_test`,
  `pending_source_cleanup_count`.
- Associated test-only state and branches:
  `test_poison_source_after_first_cleanup`, `source_cleanup_completed`,
  `ui_demand_nodes_visited`, `ui_owner_nodes_visited` mirrors. Keep real perf
  counters, authoritative maps, poison checks, cleanup queues and used worker
  latches. Both visibility helpers form an isolated unused test-only path.

## Approved: subsequent supporting phase

This phase requires its own explicit parent approval after the Rust writer
finishes; it must not race with Rust/counter/native-contract changes.

| ID | Parent-verified deletion/correction and constraints | Implementation/evidence |
|---|---|---|
| T-003 | Delete `runtime/environment.ts`; replace the sole actual consumer in `transport/content/ffi.ts` with `runtimeResourceRegistry().environment`. Remove the already-unused import in `api/content/retained.ts`. Delete unused `runtimeResourceEnvironment()` getter, but KEEP the realm-global environment object/key and registry singleton identity in the authoritative native registry. | Implemented: deleted the forwarding runtime environment module, routed Content FFI sessions through the authoritative registry environment, removed the unused retained-content import and deleted only the unused getter while preserving the global environment key and registry singleton. Evidence: `/tmp/t7-cleanup-support-static.log`; `/tmp/t7-cleanup-support-typecheck.log`; `/tmp/t7-cleanup-support-test-content.log`. |
| T-004 | Delete the pure re-export `runtime/native-resource-registry.ts` after T-003. `runtime/handle-registry.ts` already imports the authoritative native registry directly; no invented change there is needed. | Implemented: deleted the runtime registry facade and moved `handle-registry.ts` type imports to the authoritative transport registry; runtime disposal and registration still use that singleton. Evidence: `/tmp/t7-cleanup-support-static.log`; `/tmp/t7-cleanup-support-typecheck.log`; `/tmp/t7-cleanup-support-ownership.log`. |
| T-005 | Inline the sole `nativeTui.textSource` constructor use into `transport/content/control.ts::createTextSource`, preserving `requireNativeClass` validation and types, then delete `transport/native/factories.ts`. | Implemented: inlined the validated `NativeTextSource` construction into `createTextSource`, preserving the native-class guard and contract type, and deleted the one-call factory facade. Evidence: `/tmp/t7-cleanup-support-static.log`; `/tmp/t7-cleanup-support-typecheck.log`; `/tmp/t7-cleanup-support-test-content.log`. |
| I-001 | Correct package-local `perf:content` to `bench/react_content.ts`; old `bench/perf13_h_content.ts` is absent. Keep the current benchmark and root command. | Implemented: corrected the package-local `perf:content` target to `bench/react_content.ts` and retained the current benchmark/root command. Evidence: `/tmp/t7-cleanup-support-static.log`; package benchmark execution is deferred to the parent because the currently staged addon predates this phase. |
| I-002 | Update living `docs/architecture/CURRENT-ARCHITECTURE.md`: remove claims/diagram paths that LegacySceneAdapter/View layout remain; describe current occurrence/Taffy/content/worker/receipt owners and intentional History/control behavior. | Implemented: replaced the stale adapter/View-layout diagram and status with the direct occurrence/Taffy/content/worker/receipt route, while documenting live SceneHost, controls and physical History owners. Evidence: `/tmp/t7-cleanup-support-diff-check.log`; `/tmp/t7-cleanup-support-static.log`. |
| I-003 | Clarify the DOM implementation ledger's current versus historical status only. Full historical performance acceptance remains unverified although local implementation is complete. Preserve baseline commands/results and hashes verbatim; label historical sections rather than substituting current benchmark paths into past evidence. | Implemented: clarified current T7/M2 implementation status, labeled historical tranche/design sections and retained historical commands, results and hashes verbatim; no past benchmark path was rewritten. Evidence: `/tmp/t7-cleanup-support-diff-check.log`; `/tmp/t7-cleanup-support-static.log`. |
| I-004 | Update `iyon-tui.md`: remove obsolete `perf_bench`/`tui_perf` exception and retained-view constructor claims; describe actual binding-only/native occurrence/content boundary. Prefer no brittle export count. | Implemented: removed the obsolete benchmark-module exception and retained-view constructor language, documented the direct occurrence/content/native binding seam and removed the brittle export count. Evidence: `/tmp/t7-cleanup-support-binding.log`; `/tmp/t7-cleanup-support-diff-check.log`. |
| I-005 | Remove the ENTIRE unreachable `name !== "binding"` tooling-exception branch and stale comments in `tools/api-surface/check-binding.ts`. Parent verified `ROOT_ALLOWED_PUBLIC_MODULES` is exactly `binding`, with rejection/continue before this branch. Preserve strict public-module rejection, forbidden authoring names and root re-export checks. | Implemented: removed the unreachable tooling-exception branch and stale benchmark-module comments; strict unsupported-module rejection, forbidden authoring names and root re-export checks remain. Evidence: `/tmp/t7-cleanup-support-binding.log`; `/tmp/t7-cleanup-support-static.log`. |

## Explicitly retained / rejected deletion proposals

- Current SceneHost/direct/direct_tree/Taffy, component registry, control/input,
  semantic content, Source FFI, actual resource registry/finalizers, physical
  History and surface helpers are live independent owners, not legacy names
  to delete. `Surface::crop_to` has meaningful physical Unicode tests.
- `RawDomain::prefix` is called by production Markdown despite an old allowance;
  do not delete it. Generated allowances are generator-owned and not this
  cleanup's target.
- All other current React/occurrence/native ingress/consumer tests and semantic
  Unicode/projection/History regressions remain. Native `test-util` hooks used
  across crates remain; orphan core `cfg(test)` helpers are not those exports.
- Generated UI schema/tooling, source ABI symbols, negative old-route guards,
  useful CI coverage (including Linux), `perf-counters`, `target` and
  `target-perf-counters` remain. Four obsolete alternate target directories
  were already deleted separately at the user's request.
- Historical docs/atlas/reports, current raw T7 evidence, and Bun provenance
  remain. No finding establishes that these are valueless executable residue.
- Keep the developer-facing `build:tui-native` alias: no repository caller
  alone does not prove a supported command is obsolete.

## Verification and closure

Workers run focused checks for their deletions and record exact results above.
Do not add a test for every removed helper. Check actual surviving Editor
Unicode/paste/retirement behavior and native surface absence after T-006.
Parent then independently reviews full diffs and affected owners; runs final
workspace/all-feature Rust tests, project Clippy, formatting, TypeScript,
relevant Biome, generator/binding/ownership/declaration checks, fresh default
addon staging/smoke and full Bun tests. Exercise package-local benchmark command
and instrumented counter names if changed; restore default addon afterward.
Before each Cargo/stage build: at least 100 GiB free, aggregate target directories
at most 20 GiB; jobs=2, incremental=0, dev/test debug=0. No extra build profiles.

### Final parent acceptance

All 19 approved ledger items are implemented and independently reviewed.
Parent inspected the complete Rust and supporting diffs, verified the reported
caller evidence and focused logs, and corrected two final integration details:
the architecture page's surviving references to nonexistent HostViewSlot and
HostScrollPane types, and the staging script's compiled-addon absence checks
for the two deleted classes and seven deleted host methods. Type-only registry
imports in handle-registry were correctly redirected along with the removed
facade; the realm-global environment/registry identity is unchanged.

Final checks ran on the completed cleanup working tree before its commit:

| Check | Result | Evidence |
|---|---|---|
| Workspace Rust tests, all features | 356 passed; one ignored doctest. Core 328, native 18, sync 1, generator 9. The one removed test belonged only to the deleted NativeTextInput wrapper. | `/tmp/t7-cleanup-final-workspace.log` |
| Project Clippy gate | Passed; configured warning backlog remains, no new blanket allowance. | `/tmp/t7-cleanup-final-clippy.log` |
| Rust formatting and TypeScript | Passed. | Parent command results; supporting TypeScript logs above |
| Focused Biome lint | Passed with 85 configured warnings across the ten checked TS/tool files; no lint errors. | `/tmp/t7-cleanup-final-biome.log` |
| Generator, binding, ownership, declarations | Passed. | `/tmp/t7-cleanup-final-{generated,binding,ownership,declarations}.log` |
| Fresh default native staging and smoke | Passed, including actual addon class/method absence checks. | `/tmp/t7-cleanup-final-stage.log`, `/tmp/t7-cleanup-final-smoke.log` |
| Full Bun package/consumer tests | 87 passed, 399 expectations. Includes Editor, Source, receipt, History and native animation behavior. | `/tmp/t7-cleanup-final-bun.log` |
| Repaired package-local benchmark | All 15 workloads and eight traffic witnesses completed, default 7 samples/1 warmup/16 appends. | `/tmp/t7-cleanup-package-perf.log`, `/tmp/t7-cleanup-package-perf.json` |
| Instrumented root benchmark | All 15 workloads and eight witnesses completed; projection and paint timers recorded real work. Snapshot exposes exactly the 21 remaining live counter names. | `/tmp/t7-cleanup-instrumented-perf.log`, `/tmp/t7-cleanup-instrumented-perf.json` |
| Default addon restoration and smoke | Passed after instrumented validation. | `/tmp/t7-cleanup-default-restored.log`, `/tmp/t7-cleanup-restored-smoke.log` |

Default addon SHA-256:
`fa57911ee24f710d77db2e05e03a1b56b709811ab51399a48d930a4f13e2736b`.
Instrumented addon SHA-256:
`b51a03bac548fdc35dd290318b9e496388c82025fd139de2c68bee927ac75589`.
The cleanup benchmark runs validate functionality/traffic and counter ownership;
their pre-commit HEAD field is not a claim that the dirty cleanup source was
already committed. Existing source-qualified raw T7 reports remain unchanged.

Parent accepts this cleanup for commit after passing these checks. Four tracked
files are deleted outright; obsolete code within live files is removed rather
than replaced with compatibility scaffolding. Retained items and evidence
limitations above are deliberate, not undisclosed pending deletion tasks.
No Linux build, new target directory, clone, worktree or push was performed.
