# Parent acceptance — grouped follow-up46

Fully read lines1–180 (prior window),181–550,551–940,941–1477. Static source investigation; no tests/benchmarks executed by scout. Canonical report hash is recorded in report-provenance.json. Recommendations are evaluated below, not automatically accepted.

## Accepted conclusions
- No approved supersession found for the typed ContentDataTransport seam, unified descriptor guarantees, or generated content-schema requirement. Public direct imports/handwritten packing are actual differences; absence of a named descriptor struct alone is not a behavioral defect. Keep the latter as equivalence proof/design decision.
- T15 environment labels do not choose dispatch; actual current structural session is N-API. This is an evidence-integrity finding.
- Arc-to-static-mut and unsafe host/environment Send/Sync require independent soundness evidence; mutex serialization and owner-thread comments do not supply that evidence.
- Native slot mutation occurs before invalidate/render can fail. Old visible frame preservation does not imply old control/root rollback.
- Animation/stop skip the ordinary TS attachment/currentView boundary; native strong references alone do not prove all resource lifecycle invariants.
- Environment-wide staged transaction abort is explicit source policy, with possible cross-host consequences.
- Root-only History content cache key, full RowViewport/clear wide-cell behavior and missing-ticket early return remain bounded concerns. No runnable production counterexample was produced.
- TS style-name regex defect and TextInput cursor-change contract mismatch are source-confirmed; intended cursor contract remains undecided.
- Source truncation after seal is permitted by source; Markdown post-connect restart policy must not be invented.

## Parent independent checks and qualifications
Read packages/iyon-tui/src/api/controls/view-slot.ts:45–165, application/kernel.rs:665–707, application/content.rs:4348–4378 again.
- A5 is NOT closed by type-only root export. ViewSlot interface/class merge includes public implementation members; source visibly declares tuiViewAbiInstallRef and prepareSetView outside the small interface. Private construction prevents construction, not member exposure. Declaration/public-import proof remains open; do not repeat scout “merged declaration does not expose internals by itself” as a proven closure.
- B4: successful scene preparation has already replaced the reconciled mount graph when retirement is reaped, per actual kernel comment. Scout hypothetical “logical/committed mount graph may remain old” conflates graph and receipt. Old physical frame can remain old, but that alone does not prove it needs a live callback-bearing component. Keep retirement-versus-receipt ownership question, not demonstrated use-after-free.
- C4: early return definitely does not mark incomplete. A valid pending ticket becoming unavailable is NOT established merely by comments about newer candidates: exact candidate/committed/cache pinning is mitigation. Register failure-path observability plus reachable-miss proof, not demonstrated blank successful frame.
- C5: broad conservative invalidation resolves simple state cache omission claims; do not infer all possible generic subtree dependency/Y-sort metadata patches are thereby proven correct.
- Public TextContent/Projection/Smooth are lightweight conveniences; no full native parity claim in integrated docs.
- Original-report errata labels in follow-up are occasionally broader/wrong IDs (theme19, Diff02/09); canonical RECONCILIATION uses parent-verified report IDs.
- The scout's report-preservation claim applies to canonical repository copies only. Resume overwrote its managed output45 path; canonical45 remains hash-identical and canonical46 separately records new bytes.

## Disposition
No source repairs authorized or performed. ISSUES.md records comparisons, recommendations and outstanding proofs. Safety, transaction and lifetime candidates are not marked runtime-proven. Full parent reading is complete for all46 reports; acceptance is corrected evidence acceptance, not approval of every scout recommendation.
