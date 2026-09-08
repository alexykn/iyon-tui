# Current architecture documentation

This directory contains a source-based mapping of revision `4355c02`, after L1 completion.

## Deliverables
- [Architecture mapping runbook](ARCHITECTURE-MAPPING-RUNBOOK.md): reproducible procedure and concrete execution record, including assignments, prompts, workflow, artifact handling and parent acceptance requirements.
- [Revision-specific atlas](atlas-4355c02/README.md): investigation plan, detailed evidence reports, comprehensive parent synthesis and reconciliation register.
- [Maintained architecture reference](CURRENT-ARCHITECTURE.md): detailed human-readable responsibilities, dependencies, ownership, runtime paths, invariants, diagrams and change-maintenance checklist.
- [DOM-like runtime implementation ledger](DOM-RUNTIME-IMPLEMENTATION.md): scoped T0/T1 evidence and the T2–T7 migration checklist.

The atlas is a historical source snapshot; the living reference is updated as implementation changes. Neither is a V5 implementation plan. Current source determines description; approved unsuperseded PERF13/API-H/L1/PRE-V5 handoffs remain intended-contract oracles. An unexplained mismatch is not casually dismissed as stale documentation.

## Two distinct reading experiences

The atlas `COMPREHENSIVE-REPORT.md` is an information-dense, agent-readable technical inventory. Prefer precise tables, exact symbols and paths, explicit contracts, execution details, and evidence over decorative prose. Completeness and auditability matter more than presentation.

`CURRENT-ARCHITECTURE.md` is a human-readable, detailed technical overview: deliberate section order, explanatory prose, useful ASCII diagrams, and clear links to deeper evidence. It is not merely an abridged copy of the inventory. Its structure should remain useful through V5 and later architectural changes, while its description tracks the actual implementation.

## Maintenance policy
Architectural changes should update the living reference in the same change: ownership, public surfaces, dependency edges, transport routes, lifetimes, invalidation, failure behavior and relevant observability. Link to tests/source; identify intentional alternate paths and removal conditions for temporary paths. Record the last verified revision. Do not rewrite the snapshot to pretend it describes newer source.

Status: complete current-state mapping. All45 primary reports plus grouped follow-up46 fully read by the parent; integrated inventory, reconciliation, findings, maintained guide and runbook delivered. Open technical questions remain explicit; no V5 implementation or production source changes were made. See [final validation](atlas-4355c02/evidence/final-validation.md).
