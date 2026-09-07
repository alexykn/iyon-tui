# Parent reading notes — 44 historical/document drift

Fully read lines 1–475, 476–950, 951–1419. Static investigation, not current runtime qualification.

## Accepted reconciliation
- Authority is current source for execution, with PERF-13/API-H/L1/PRE-V5 as intended-contract and chronology records. Resolved PERF-13 overrides its integrated tentative baseline; F is interim, G/H migrate/delete old content/stream/fallback routes, L1 changes later costs and products. Older LAY/performance proposals are not normative.
- API-H1 pushStream/sealStream/nativeObject and optional setLayout statements do not describe current History. Current TS push/freeze use retained references, explicit refusal, native pushRef/freezeRef, plus layout/discardLive/setLayout.
- Detached History creation and one-way single-host transfer are deliberate; detached freeze/discard reject. Static versus live is component-identity classification, NOT whether provider content remains mutable.
- Current root-level History, finalized provider rows, exact partial physical remainders, retirement callbacks and synchronization-unknown recovery replace pre-L1 complete-surface/open-row/suffix descriptions. V5 future component-only/Taffy/residency ideas are not current architecture and not requirements of this mapping.
- Historical totals and performance acceptance belong to their revision/artifact/platform/workload. L1-13 checkpoint is not final acceptance; later macOS arm64 qualification has limits.
- Semantic freeze is distinct from irreversible physical transfer. ContentPort/Connector provider owns source-revision-keyed Arc products; History owns order/unit/physical frontier. Contract wording about History capturing immutable historical descriptor remains interpretation question, not proven violation.
- Useful HD01–15 chronology register should feed synthesis; README pending status will be resolved by parent, not treated as source drift.

## Parent corrections/limits
- Parent independently read lib.rs: history is pub(crate) mod history, not bare mod history as §2.1 says. §9 correctly has pub(crate). Public declarations under it do not make supported external Rust authoring API.
- §4.2 attach/publication diagram is schematic, not proof of rollback/strict temporal order. Prior parent source checks establish native desired root/control mutation can precede later flush failure; no all-plane rollback.
- A successful recovery candidate clears unknown marker, but does not reconstruct previously uncertain physical terminal history. Preserve prior source-backed notes37.
- L1 pure-tick claims concern delivery advancement work, not literally every complete host frame.
- H deletion wording refers to old stream-specific machinery; surviving generic HistoryUnit is not contradiction.
