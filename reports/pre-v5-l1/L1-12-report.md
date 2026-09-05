# L1-12 report

Tranche: L1-12 — targeted frame work, prepared commit, wake routing, and report/error cleanup

Starting SHA: `407b73fb80a58f373978f454c71d5b43adde574f` (`refactor(tui): complete L1-11 cached content lowering and row painting`)

Prior approved lineage: L1-11 started at `b03405ee3855f5b922ee8b173fea92799b378f48` and was committed by the parent as `407b73fb80a58f373978f454c71d5b43adde574f`. This report records the approved L1-12 result. Resolve its containing commit with `git log -1 --format=%H -- reports/pre-v5-l1/L1-12-report.md`.

## Scope and compatibility

The implementation keeps the pre-v5 custom terminal layout, native input/tick/History ownership, TypeScript facade, current History residency/receipt semantics, and one canonical retained semantic DAG. It adds no dependency and does not edit `AGENTS.md`. All added runtime concepts are generic content/frame/terminal mechanics; no Iyon application policy was added.

The implementation does not introduce React, Taffy, GPUI, component-only Surface residency, semantic-Smooth migration, or a second renderer. Pre-v5 compatibility remains authoritative as requested.

## Targeted dirty work

- Added typed `ContentDirty` records and `ContentDirtyReason` values for Source input, delivery visibility, width/measurement, presentation, viewport, and selection/lifecycle changes.
- Added retained `LayoutTree` ContentPort occurrence indexes. Source/control invalidation now targets the ContentHost dependency path and paint root instead of calling a host-wide content cache clear.
- Kept input dirtiness, actual metric changes, and paint propagation distinct. Content layout is evaluated when required, intrinsic height/physical completeness changes conservatively escalate to the normal dependency-aware pass, and same-geometry paint-only changes use retained subtree repaint.
- Stored ContentHost intrinsic size and physical-completeness metadata in the retained layout product so a bounded local patch cannot hide a changed metric or leave a stale global completeness result.
- Theme-only updates refresh prepared content paint identities and invalidate only affected ContentHost measurement entries when a concurrent structural change forces a full pass; semantic products remain reusable.
- Added focused locality counters for dirty records, metric evaluations/changes, paint roots, wake groups, due Connectors, and prepared commit records. `GlobalCacheClears` is not used by the new Source wake route.

## Prepared content commit

- Added candidate-owned `PreparedContentCommit` data containing next Port associations, old/next Connector records, cleanup records, Source ownership pins, prepared deadlines, delivery stamps, and requested/visibility state.
- Candidate commit now includes every touched Connector, including content-only Source/delivery candidates. This is required to promote their prepared projection, represented Source revision, and visible delivery frontier; cleanup no longer silently discards those products while leaving the old committed projection authoritative.
- Expected visibility is computed before receipt from next-visible/hidden/current-visible records. A failed or non-selected candidate remains non-visible and cannot promote speculative projection state.
- The host stores this plan while a backend receipt is outstanding. New desired selection, deactivation, disposal, Source, and delivery work cannot consume or overwrite the old plan. Candidate capacity is independent from newer pending operations.
- Visible commit consumes prevalidated Arc-backed records. It does not resolve handles, invoke callbacks, revalidate ordinary caller input, or scan the complete Port registry. Bounded prepared-record lookups remain local to the candidate plan.
- Candidate cleanup is staged in the plan; `end_candidate` releases the in-flight lease and constant-time candidate flags. Cleanup checks control and delivery revisions so newer requested selection or delivery progress survives an older receipt.
- Commit preparation checks Port, Connector, and Source lock usability before the first visible association mutation. Poisoned records return explicit errors before visible association swaps; no best-effort skip remains in the prepared receipt commit path.
- Source association cleanup is intentionally outside the infallible logical-promotion path. After a successful promotion, a Source lock that becomes poisoned leaves its subscription and membership retained, records `SOURCE_CLEANUP_PENDING` in the Connector status, and queues a candidate-owned deferred cleanup entry. The environment marks that Host retry-blocked rather than requeueing indefinitely; an explicit barrier, new Source wake, or independent Host mutation admits one retry, and removal releases membership exactly once.
- Deferred-cleanup vectors and ID membership are reserved during preparation, before the backend receipt. The large prepared-record fixture verifies captured plan capacities do not grow while newer work is accepted; this is a capacity proof for those candidate-owned tables, not a claim that every allocator in the process is instrumented.
- The Host two-Source cleanup fixture samples `pending`, `pending_set`, `queued`, and `retry_blocked` capacities before reservation and after completion, including the deferred-failure branch; reserved and final capacities are equal, so retry blocking does not allocate after logical promotion.
- Connector activation failure remains blocked across all width probes in one candidate. This prevents an unconstrained measurement followed by a committed-width measurement from consuming a synthetic failure once and accidentally selecting the failed Connector.
- Mounting an unactivated Connector now commits the destination mount as an idle/cold binding without repeatedly re-queuing an unresolved desired selection. Activation remains an explicit operation.
- Unmount/remount/dispose and previous-visible Connector transitions retain the old-visible fallback discipline. A disposing in-flight Connector stays pinned until its captured receipt commits or aborts; if Source cleanup is transiently unavailable after promotion, the Connector and Source membership remain pinned until the deferred cleanup candidate succeeds.

## Dirty metric propagation and locality

- Bounded local content refresh probes the current ContentMeasurement once against the retained width. Intrinsic height or physical completeness changes escalate to the regular layout dependency path, preserving following-sibling reflow and RowViewport extent correctness.
- Fit-width intrinsic width changes escalate when retained parent dependency metadata says the parent uses child width; Fill-width content can patch paint/layout identity locally when allocation remains fixed.
- Added `content_metric_growth_reflows_following_siblings`, which verifies changed content rows become visible and root follow-end geometry remains consistent.
- Added `content_dirty_work_is_local_to_one_of_many_ports` and `one_of_hundreds_of_ports_has_constant_targeted_prepare_work`. A Source update to one of many fixed-width ports has one dirty record, one wake group, no Port-registry scan, and changed-record preparation rather than a complete layout flush.

## Wake and deadline routing

- Source subscriber grouping uses a maintained host-keyed index plus per-group reusable token storage and a reusable outer wake batch. Source mutex ownership ends before any Host mutex is acquired; Source-to-Host lock nesting was not introduced.
- One host wake group is coalesced into one host pending epoch while retaining all affected Port/Connector IDs for targeted invalidation. Weak/generation checks remain authoritative when a mutation drains.
- Due Connector and active synchronization worklists are reused and contain only mounted/visible/requested Smooth Connector IDs. Immediate Connectors have no deadline membership; cold/unmounted Smooth Connectors do no delivery/parser work until their destination is mounted.
- A poisoned due Connector is removed from deadline/index membership and reported as a typed scheduler failure instead of spinning on every native tick.
- Source wake of an unmounted requested Connector does not schedule delivery work; remount or a new relevant signal re-admits it.
- Delivery-clock advancement returns typed affected records and reports scheduler errors through the existing typed host attempt envelope. Pure ticks still do not parse, lower a new semantic tree, or copy a full content surface.

## Physical and History failure handling

- `NativeFrontier` records an irreversible `synchronization_unknown` marker when a native sink returns an error or invalid acknowledgement after a write may have started. A subsequent ordinary transfer is blocked until the host's successful recovery frame clears the marker; History rows/frontiers are never rolled back.
- Native History unit retirements are recorded directly in a reusable frontier list and drained even when a later sink operation fails. The old before/after whole-History `HashSet` scan was removed, and History adapter unit-to-Port lookup is maintained rather than scanning every Port on retirement.
- A zero-row retirement is surfaced as transfer progress even when recursive transfer reaches a blocked/live successor. SceneHost re-resolves before painting so a candidate cannot retain a retired unit.
- Native History display revision is advanced once per successful transfer operation (including zero-row retirement), avoiding the prior double bump when a transfer both accepted rows and retired a unit.
- Backend receipt failure marks Host physical synchronization unknown. The next successful candidate is a recovery/full-paint frame; old logical frame authority remains unchanged while retry is pending. The host does not claim the old physical screen is intact after a failed presentation.
- Terminal worker stop classification uses a typed error marker instead of matching the worker's human-readable diagnostic string.
- A poisoned Host no longer aborts the entire fair environment drain or drops later pending Hosts. It produces a `HOST_LOCK_POISONED` report, retry-blocks only that Host, and continues draining unrelated candidates.

## Changed files

Exact tracked source/report paths changed from the starting SHA:

- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/environment.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/history/model.rs`
- `crates/iyon-tui/src/history/native/frontier.rs`
- `crates/iyon-tui/src/history/native/mod.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/presentation/content.rs`
- `crates/iyon-tui/src/presentation/layout/engine.rs`
- `crates/iyon-tui/src/presentation/layout/place.rs`
- `crates/iyon-tui/src/presentation/layout/tree.rs`
- `crates/iyon-tui/src/presentation/mod.rs`
- `crates/iyon-tui/src/retained_state/mod.rs`
- `crates/iyon-tui/src/retained_state/registry.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/scene/resolved.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/terminal/backend.rs`
- `crates/iyon-tui/src/terminal/mod.rs`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `reports/pre-v5-l1/L1-12-report.md`

Current tracked source/test diff from Starting SHA: **5,495 additions / 829 deletions across 24 tracked source/test files**. The report is untracked until the parent includes it in the separate tranche commit. The existing untracked `target-direct-ffi-fast-view-abi/`, `target-fast-view-abi/`, and `target-perf-counters/` directories are build artifacts and must not be staged.

No generated ABI/schema output, TypeScript public declaration, dependency manifest, or `AGENTS.md` was changed by this tranche. The native/TS status regression intentionally keeps the existing JSON/TypeScript status shape and carries deferred cleanup through the existing `error` field.

## Tests and failure fixtures

Added or extended focused fixtures in the existing Rust unit-test modules:

- `source_wake_repaints_only_the_affected_content_port`: Source wake reaches the visible screen through the targeted content path and asserts no global cache clear or stale committed Source revision.
- `content_metric_growth_reflows_following_siblings`: intrinsic content growth is not hidden by a bounded local patch.
- `content_dirty_work_is_local_to_one_of_many_ports`: independent Source mutation keeps sibling content work local.
- `one_of_hundreds_of_ports_has_constant_targeted_prepare_work`: 512-port locality workload checks wake, metric, layout, paint, and prepared-record counters.
- `theme_recolor_refreshes_content_paint_without_rebuilding_layout`: theme-only content paint refresh updates resolved styles while retaining geometry products.
- `prepared_content_commit_preserves_newer_requested_selection`: a newer requested Connector accepted while an older plan is held remains desired and pending after the old plan commits.
- `prepared_content_commit_preserves_a_newer_delivery_tick`: a newer delivery clock advance is not promoted by an older prepared receipt.
- `prepared_content_commit_rejects_a_poisoned_record_before_swapping`: an independently poisoned Connector lock returns before visible association mutation.
- `mounting_unactivated_connector_does_not_requeue_forever`: mount and activation remain distinct and do not cause an automatic retry loop.
- `delayed_content_receipt_preserves_newer_source_work_for_next_candidate`: a Source mutation accepted while an older content receipt is in flight is committed by the next candidate, not lost or folded into the old plan.
- `source_cleanup_failure_after_first_receipt_is_exactly_once`: a poisoned Source fails commit preflight before any old-visible Source cleanup, preserving both old bindings and memberships for retry.
- `post_promotion_source_cleanup_failure_retains_membership_for_retry`: a cleanup failure after logical promotion retains the Source membership, keeps the Connector-owned pending cleanup, and releases it exactly once on the next candidate.
- `host_source_cleanup_failure_preserves_membership_until_retry`: the host-level two-Source fixture verifies committed retirement, explicit `cleanup_pending`/`SOURCE_CLEANUP_PENDING` status, no environment rearm spin, and exact-once recovery.
- `content_connector_status_maps_cleanup_through_the_existing_error_lane`: native status JSON maps deferred cleanup into the existing `error` field without adding cleanup-specific JSON keys, while preserving normal operating errors.
- `PERF-13-D content identities maps deferred cleanup through the supported error status lane`: TypeScript status mapping exposes the cleanup diagnostic and normal operating diagnostic through the existing `error` property without inventing public fields.
- `native_sink_failure_marks_synchronization_unknown_without_rewinding_history`: partial/error sink behavior marks synchronization unknown and requires explicit recovery.
- `failed_presentation_marks_physical_sync_unknown_until_recovery_frame`: delayed backend receipt failure leaves the old frame authoritative and clears the physical marker only after a successful recovery frame.
- `poisoned_host_does_not_drop_unrelated_pending_hosts_from_fair_drain`: a poisoned Host is reported/blocked while an unrelated pending Host still commits.

The existing activation rollback, Source wake poison, partial/zero History receipt, Smooth, lifecycle, native tick, History differential, and terminal presenter fixtures remain enabled.

## Commands and evidence

Environment:

- Bun `1.4.0`
- `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- `cargo 1.97.1 (c980f4866 2026-06-30)`
- macOS arm64, debug/test profile
- No native addon was staged or published by this child.

Final commands run successfully:

1. `cargo fmt --all -- --check` — `/tmp/l12-fmt-final-status-map.log`; exit 0
2. `git diff --check`
3. `cargo check -p iyon-tui --features native-host,test-util,perf-counters` — `/tmp/l12-check-deferred-nospin.log`; exit 0
4. `cargo check -p iyon-tui` — `/tmp/l12-check-default-deferred-nospin.log`; exit 0
5. `cargo test -p iyon-tui --features native-host,test-util,perf-counters --lib -- --test-threads=1` — `/tmp/l12-lib-final-status-map.log`; **671 passed, 0 failed, 1 ignored**
6. `cargo test --workspace --all-features` — `/tmp/l12-workspace-final-status-map.log`; workspace tests, generated ABI tests, doctests, and compile-fail fixtures passed
7. `sh tools/lint/clippy-gate.sh` — `/tmp/l12-clippy-final-status-map.log`; exit 0, repository/generated warnings remain
8. `bun run check:tui-abi` — `/tmp/l12-tui-abi-deferred-nospin.log`; exit 0
9. `bun run check:tui-binding` — `/tmp/l12-binding-deferred-nospin.log`; exit 0
10. `bun run check:tui-declarations` — `/tmp/l12-declarations-deferred-nospin.log`; exit 0
11. `bun run typecheck` — `/tmp/l12-typecheck-final-status-map.log`; exit 0
12. `bun run check:ownership` — `/tmp/l12-ownership-final-status-map.log`; all ownership, framework-purity, public-surface, and banned-name checks passed

Focused evidence logs include:

- `/tmp/l12-source-wake-status.log`
- `/tmp/l12-growth-after-height.log`
- `/tmp/l12-many-after-height.log`
- `/tmp/l12-delayed-content.log`
- `/tmp/l12-unactivated2.log`
- `/tmp/l12-host-poison-fair.log`
- `/tmp/l12-plan-after-clean.log`
- `/tmp/l12-deadline-after-clean.log`
- `/tmp/l12-source-deferred-each.log`, `/tmp/l12-host-deferred-fair.log`, and `/tmp/l12-lib-final-source-cleanup.log`
- `/tmp/l12-native-status-mapping.log` and `/tmp/l12-ts-status-mapping2.log`
- `/tmp/l12-host-capacity-status.log`

The final library run includes every focused fixture listed above, including the two-Source cleanup atomicity/deferred-retry fixtures and environment capacity sample. The final workspace run was executed after the retained layout metadata, candidate-commit, wake, History, poison, cleanup status, no-spin, native status mapping, and TypeScript status mapping fixes.

## Residual risks and limitations

- The implementation retains pre-v5 custom terminal layout and current History residency by request. Taffy, React, component-only Surface residency, and future semantic-Smooth behavior are intentionally not introduced.
- `ContentHostRegistry` still uses per-record `Mutex` values for compatibility with existing detached handles. Port/Connector/Source locks are preflighted before logical promotion. A Source that becomes poisoned in the separate post-promotion cleanup phase is not treated as a failed frame: its membership/subscription and Connector owning pin stay retained, `SOURCE_CLEANUP_PENDING` is observable through status, and the Host is retry-blocked until explicit/new readiness. A future host-owned record migration could remove that remaining lock boundary.
- Native JSON and the TypeScript facade intentionally preserve the established Connector status shape: callers diagnose `status().error.code === "SOURCE_CLEANUP_PENDING"` and issue an explicit Host flush/retry or wait for a new Source/Host readiness signal; they must not re-append/reapply the already-accepted Source operation.
- Some diagnostic/readback and abort-only compatibility paths still use Option-style lock handling or bounded fallback behavior outside the prepared receipt commit hot path. They are not used to commit a visible frame and are not claimed as zero-cost.
- The `ContentProvider::measure` seam remains a non-`Result` compatibility interface. Rare fit-width preparation failures are recorded as typed Connector operating failures and preserve the existing method shape; the prepared commit then reports an unaccounted lock/invariant failure if the record cannot be made usable.
- A native sink error does not expose an accepted-row count. The synchronization-unknown marker therefore blocks replay until the caller/host runs the existing recovery frame; it intentionally does not guess which physical rows may have been accepted.
- Source direct-FFI v1 status layout was not expanded. Post-acceptance Source wake errors remain explicit in the Rust Source mutation result/error path; a future versioned FFI envelope may carry an accepted revision and wake failure as separate lanes without changing v1.
- No release-profile benchmark, p50/p95/p99 timing, peak-memory measurement, packaged-addon staging, Node integration smoke, or real-terminal platform qualification was claimed. Those remain parent/CI qualification work.
- Existing Clippy warnings are from generated ABI/tests and pre-existing private/dead-code surfaces; the gate exits successfully.

## Parent acceptance

Accepted for the separate L1-12 tranche commit after iterative actual-source review and independent verification. The parent reviewed transaction authority, delayed desired work, Source membership/subscription lifetime, deferred cleanup and no-spin recovery, retained path-index maintenance, and native/TypeScript error mapping. The parent changed only this report; implementation edits were made by the delegate.

Final independent checks passed: formatting; 671 library tests (one ignored); the native cleanup-status mapping regression; all five tests in `packages/iyon-tui/tests/tui_perf13_d.test.ts`; the Clippy gate (warnings remain); ownership; and `git diff --check`. Evidence is under `/tmp/iyon-lowering-takeover-20260905/`: `parent-l12-final-lib.log`, `parent-l12-native-status.log`, `parent-l12-ts-status.log`, `parent-l12-final-clippy.log`, and `parent-l12-final-ownership.log`.

This approval does not claim completion of L1-13 or the complete handoff acceptance matrix. Final retirement must completely delete obsolete implementations, tests specific to retired paths, and fallbacks, while preserving canonical behavior coverage and performance. Packaged-artifact qualification, full workload timing/memory comparisons, and the final definition-of-done ledger remain required in L1-13.
