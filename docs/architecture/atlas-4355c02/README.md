# Current-state atlas — 4355c02

## Baseline and purpose
Repository: iyon-tui; branch: main; source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`.
Initial worktree was clean. Parent-added documentation is outside the source baseline.
This atlas precedes the architecture census and V5 disposition analysis.

## Layout
- `subsystems/rust/`, `subsystems/native/`, `subsystems/typescript/`, `subsystems/support/`: local maps.
- `wiring/`: Rust-wide, TS-wide and three-plane maps.
- `traces/`: end-to-end behavior.
- `audits/`: routes, benchmarks, test contracts, document drift and coverage.
- `evidence/`: source manifests, assignment/provenance data, parent reading ledger.
- [COMPREHENSIVE-REPORT.md](COMPREHENSIVE-REPORT.md): parent-authored integrated report, not concatenated scout reports.
- [RECONCILIATION.md](RECONCILIATION.md): resolved contradictions, retained uncertainties and coverage gaps.
- [ISSUES.md](ISSUES.md): all discovered defect/deviation/risk/evidence findings and bounded follow-ups.
- [46 grouped follow-up](audits/46-grouped-deviation-followup.md): continued read-only Luna evaluation against handoff oracles; fully read and evaluated with parent qualifications.
- [Report contract](REPORT-CONTRACT.md): mandatory evidence and Markdown structure.

## Supervision and acceptance
45 read-only Luna-high scouts, up to 8 concurrent. Scouts return isolated managed artifacts; the parent is the sole repository writer. Assignments 1–44 investigate independently; 45 reads their artifacts and source-checks coverage and contradictions. Parent reads every completed report in full, records that reading, resolves conflicts against source and writes both final documents. No production changes, new tests, V5 implementation or AGENTS edits.

## Assignments
| ID | Report | Scope |
|---|---|---|
| 01 | [rust/application](subsystems/rust/01-application.md) | application/ |
| 02 | [rust/components](subsystems/rust/02-components.md) | component/ |
| 03 | [rust/scene](subsystems/rust/03-scene.md) | scene/ |
| 04 | [rust/view-presentation](subsystems/rust/04-view-presentation.md) | presentation/ excluding layout/ and paint/ |
| 05 | [rust/layout-geometry](subsystems/rust/05-layout-geometry.md) | presentation/layout/ and geometry/ |
| 06 | [rust/retained-state](subsystems/rust/06-retained-state.md) | retained_state/ |
| 07 | [rust/history](subsystems/rust/07-history.md) | history/ recursively |
| 08 | [rust/semantic-content](subsystems/rust/08-semantic-content.md) | content/ recursively |
| 09 | [rust/projection-smoothing](subsystems/rust/09-projection-smoothing.md) | projection/ recursively |
| 10 | [rust/stream-l1](subsystems/rust/10-stream-l1.md) | stream/ recursively |
| 11 | [rust/theme](subsystems/rust/11-theme.md) | theme/ |
| 12 | [rust/controls-scroll](subsystems/rust/12-controls-scroll.md) | controls/ recursively, scroll.rs, scroll_command.rs |
| 13 | [rust/interaction-output](subsystems/rust/13-interaction-output.md) | interaction/ and output/ recursively |
| 14 | [rust/paint-physical](subsystems/rust/14-paint-physical.md) | presentation/paint/, physical/, text.rs |
| 15 | [rust/terminal-backend](subsystems/rust/15-terminal-backend.md) | backend/ and terminal/ recursively |
| 16 | [rust/public-binding](subsystems/rust/16-public-binding.md) | lib.rs, binding/, id.rs, crate manifest and all otherwise-unassigned root helpers |
| 17 | [native/structure-state](subsystems/native/17-structure-state.md) | crates/iyon-tui-native/src/ handwritten structural/state entrypoints |
| 18 | [native/content-host-events](subsystems/native/18-content-host-events.md) | crates/iyon-tui-native/src/ remaining handwritten files |
| 19 | [typescript/public-api](subsystems/typescript/19-public-api.md) | packages/iyon-tui/src/api/ recursively and src/index.ts |
| 20 | [typescript/composition](subsystems/typescript/20-composition.md) | packages/iyon-tui/src/composition/ |
| 21 | [typescript/runtime](subsystems/typescript/21-runtime.md) | packages/iyon-tui/src/runtime/ |
| 22 | [typescript/structure-state-transport](subsystems/typescript/22-structure-state-transport.md) | packages/iyon-tui/src/transport/{structural,state,abi}/ excluding generated bodies |
| 23 | [typescript/content-native-transport](subsystems/typescript/23-content-native-transport.md) | packages/iyon-tui/src/transport/{content,native}/ and other transport files |
| 24 | [support/codegen](subsystems/support/24-codegen.md) | tools/tui-abi/, tools/tui-abi-gen/, generated Rust/TS, native include/ |
| 25 | [support/tests-fixtures](subsystems/support/25-tests-fixtures.md) | Rust/TS tests, packages/iyon-tui/src/testing/, packages/tui-consumer-fixture/ |
| 26 | [support/bench-build-examples](subsystems/support/26-bench-build-examples.md) | benches/counters, scripts, manifests, CI, tools excluding ABI generator, examples/replays, package native artifacts |
| 27 | [wiring/rust-side](wiring/27-rust-side.md) | both crates and Rust tool crate |
| 28 | [wiring/typescript-side](wiring/28-typescript-side.md) | packages/ |
| 29 | [wiring/three-planes](wiring/29-three-planes.md) | TS runtime/transport, native addon and Rust retained runtime |
| 30 | [traces/composition-root](traces/30-composition-root.md) | public TS authoring through native layout/paint |
| 31 | [traces/structural-mutation](traces/31-structural-mutation.md) | TS and Rust structural update routes |
| 32 | [traces/state-mutation](traces/32-state-mutation.md) | TS state APIs through Rust invalidation/output |
| 33 | [traces/themes-styles](traces/33-themes-styles.md) | TS theme/style APIs through rendered cells |
| 34 | [traces/runtime-scheduling](traces/34-runtime-scheduling.md) | TS startup through native session/backend shutdown |
| 35 | [traces/stream-content](traces/35-stream-content.md) | TS source APIs through Rust source/connector/projection/paint |
| 36 | [traces/layout-resize](traces/36-layout-resize.md) | TS layout authoring through resolved cells |
| 37 | [traces/history-scrollback](traces/37-history-scrollback.md) | TS History consumers through Rust and terminal output |
| 38 | [traces/input-callbacks](traces/38-input-callbacks.md) | terminal decoding through native routing and TS callbacks |
| 39 | [traces/slots-controls-animation](traces/39-slots-controls-animation.md) | TS slots/controls through native retained values and output |
| 40 | [traces/lifetime-caches](traces/40-lifetime-caches.md) | all three planes TS/native/Rust |
| 41 | [audits/production-routes](audits/41-production-routes.md) | all production code |
| 42 | [audits/benchmark-integrity](audits/42-benchmark-integrity.md) | all benchmark suites and route counters |
| 43 | [audits/test-contracts](audits/43-test-contracts.md) | cross-repository tests and source reachability |
| 44 | [audits/history-doc-drift](audits/44-history-doc-drift.md) | docs/history/, reports/pre-v5-l1/, current docs and referenced source |
| 45 | [audits/coverage-reconciliation](audits/45-coverage-reconciliation.md) | repository manifest plus all preceding reports |


## Completed reading and evidence
All45 primary reports and grouped follow-up46 were fully read by the parent. Source-confirmed corrections override original report claims through RECONCILIATION.md; no canonical report was rewritten.
- [Human-readable maintained architecture guide](../CURRENT-ARCHITECTURE.md)
- [Reproducible mapping runbook](../ARCHITECTURE-MAPPING-RUNBOOK.md)
- [Full-text reading ledger](evidence/parent-reading-ledger.md)
- [Report hashes and run provenance](evidence/report-provenance.json)
- [Parent source-check notes](evidence/parent-notes/)
- [Grouped follow-up parent assessment](evidence/parent-notes/46-grouped-deviation-followup.md)
- [Final validation](evidence/final-validation.md)
- [Orchestration recovery](evidence/orchestration-recovery.md)
- [Managed-output reuse qualification](evidence/grouped-followup-execution.md)

The initial collected manifest has1082 paths; the complete Git baseline has1088, with six omissions fully read in the [coverage addendum](evidence/coverage-addendum.md). This is a source snapshot, not a classified symbol disposition census. Generated/fixture bulk and test sampling limits remain explicit in the reports. Original report test discussions are static; the ownership check and documentation validation are separately executed evidence.
