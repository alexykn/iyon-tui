# Architecture mapping runbook

## Purpose

Reproduce this investigation method against a future repository revision, including after V5. Reproduce the **method and evidence**, not an identical model response or the old subsystem split.

This document has two parts: a reusable procedure and the concrete execution record for the current run. The current run is **complete as an architecture mapping**. All46 reports were fully read and reconciled into the inventory and maintained guide. Technical risks remain open where evidence is incomplete; completion is not a runtime-safety or V5 design approval.

## 1. Outputs and readers

| Output | Purpose |
|---|---|
| Revision-specific subsystem reports | Detailed local source maps with evidence and coverage limits |
| Wiring reports | Actual cross-module, cross-package and cross-language dependencies and ownership |
| Behavioral traces | Prove complete operations through source call sites rather than infer them from folder names |
| Cross-cutting audits | Identify parallel routes, failure masking, test/benchmark ambiguity and historical drift |
| Reconciliation register | Resolve contradictory claims against source; preserve genuine unknowns |
| Comprehensive report | Dense, agent-readable full technical inventory; not a concatenation of reports |
| Living architecture overview | Human-readable detailed presentation with explanations and ASCII diagrams |
| Run evidence | Baseline, scope, prompts, workflow, provenance, reading ledger and validation |

Current-state mapping comes before design assessment. Do not let a proposed future architecture determine what the existing source supposedly contains.

## 2. Freeze and record the baseline

1. Record repository identity, remote, branch, full commit and worktree status.
2. Capture any pre-existing diff; do not silently include unrelated local changes.
3. Keep implementation fixed throughout collection. If source changes, identify affected reports and revalidate them before combining evidence.
4. Create a new snapshot directory, such as `atlas-<short-commit>/`; never overwrite an older atlas to describe a newer baseline.
5. Generate a tracked-file manifest. Distinguish production, tests, generated code, fixtures, documentation, tools, ignored artifacts and dependencies.
6. Read applicable project instructions. Record obsolete references explicitly rather than recreating retired documents.

## 3. Design the investigation from the current tree

Build four complementary groups:

```text
local subsystem maps ----+
whole-side wiring ------+--> evidence reconciliation --> parent synthesis
end-to-end traces -------+
cross-cutting audits ----+
```

- Local scouts own deep source coverage for bounded modules.
- Wiring scouts own breadth and forward/reverse edges across a language/package side.
- Trace scouts follow real creation, update, failure and destruction operations across those modules.
- Audit scouts test claims about authoritative paths, compatibility, observability, tests and historical records.

Give every significant source area a primary owner. Intentional overlap at behavioral seams is useful; accidental gaps and duplicated generic summaries are not. Large recursive folders may need splitting. Tiny related helpers may share an assignment.

After V5, regenerate this split from the new tree. Do not mechanically dispatch scouts for removed View, Scene, History or composition modules.

## 4. Give each scout a concrete report contract

Include:

- exact repository/cwd/ref;
- assignment ID, primary scope, distinct question and boundary with adjacent assignments;
- source and historical-document anchors;
- read-only source authority, designated output artifact and no child-agent spawning;
- required report headings and evidence standard;
- actual consumers, dependencies, lifetimes, state transitions and failure paths;
- inspected-file list, uninspected/sampled areas, LOC counting method and validation limits;
- explicit separation of source facts, historical assertions, inference and unknowns.

Use the current [report contract](atlas-4355c02/REPORT-CONTRACT.md) as a starting template, not immutable policy. Scouts return complete Markdown, not merely a completion message. Their designated artifacts must be saved outside shared source edits.

## 5. Launch one bounded supervised workflow

1. Read the installed orchestration skill and capability documentation.
2. Discover executable, enabled agents. For external runners, check runner availability and the applicable policy; do not silently switch execution mode.
3. Validate the workflow before launching.
4. Launch one asynchronous workflow with an explicit concurrency bound and spawn budget.
5. Await independent report results inside the workflow; pass their actual output references to the coverage/reconciliation assignment.
6. Preserve the output-reference/path mapping. A relative declared output name is not proof that a file exists in the repository.
7. Parent supervises failures and genuine blockers. Children do not spawn agents.

Use current approved models and tools. This run used Luna-high scouts; future runs must verify availability and project policy rather than assume that identifier still exists.

The parent is the sole repository writer. Source-read-only scouts can still produce designated managed report artifacts. Publish those reports with a bulk copy, retaining originals for dependent investigations; do not manually rewrite 45 handoffs.

## 6. Supervise and collect evidence

- Use native completion/attention notifications. Inspect status when triaging a specific question, not in a polling loop.
- A progress message is not a report; a completed status is not proof of a complete artifact.
- Check the actual files, expected headings, start/end, size and output completeness.
- Record report ID, child run, source revision, original artifact, canonical path and content hash.
- Copy only completed artifacts. Preserve originals until all dependent work has finished.
- On infrastructure failure, capture exact error/run/status and repository state. Stop the affected path and recover through an explicit same-protocol retry; do not substitute a CLI/foreground agent without required owner approval.
- Do not count queued workflow entries as simultaneously executing agents merely because the status view labels them running.

## 7. Parent full-text reading and reconciliation

The parent must personally read **every completed report in full**, including the reconciliation report.

For large reports, read bounded sequential chunks and record ranges in a reading ledger. Recover truncated output by reading the missing ranges. Counting lines, extracting headings, reading summaries or delegating a summary does not meet this requirement.

For each report:

1. Verify its baseline and stated coverage.
2. Read the whole document.
3. Extract concrete contracts, dependency edges, ownership and route claims.
4. Compare overlapping reports and historical expectations.
5. Source-check consequential claims and apparent contradictions.
6. Record resolutions with evidence; retain unresolved questions without inventing certainty.

The reconciliation scout assists with coverage and contradiction discovery; it does not replace parent reading or acceptance.

## 8. Write the two final documents
### Handoff-oracle and deviation assessment
For this run, the user restricted relevant historical authority to PERF-13, API-H, L1 and PRE-V5 documents. Earlier records have no normative weight. Do not create findings solely from disagreement with deprecated documents. A future run should explicitly establish its own applicable authority set and supersession chain. Proposed future designs remain distinct from requirements on the current implementation.


Approved handoffs are the default oracle for intended architectural contracts; source proves actual implementation. First check whether a later approved handoff supersedes a requirement. Otherwise, record a mismatch as a deviation requiring explanation, not merely stale documentation. Maintain an issues register separating confirmed defects, contract deviations, candidate improvements and unresolved evidence.

Where current architecture may genuinely improve on an unsuperseded requirement, group related issues for focused Luna scout investigation. Evaluate the original requirement's purpose, retained/lost guarantees, correctness, ownership, simplicity, performance evidence and maintenance cost. Do not launch one scout per issue. The parent inspects the evidence and determines whether to recommend restored compliance, an explicitly justified architectural improvement or further investigation. Intent alone does not justify divergence. These follow-ups do not authorize implementation changes.


### Comprehensive technical inventory

Organize by responsibilities, cross-system connections and observable behavior. Favor exact symbols, compact tables, state machines, route matrices and evidence. Deduplicate repeated explanations while retaining unique facts and uncertainty. Explain both local implementation and how the parts compose.

### Living technical overview

Use clear section progression, explanatory prose and ASCII diagrams. Describe responsibilities, ownership, public boundaries, key runtime paths and invariants at a depth useful to engineers. Link to source and the inventory for exhaustive detail. Keep proposed architecture distinct from current implementation.

Do not produce the overview by merely truncating the inventory.

## 9. Validate and maintain

Before completion:

- check all expected reports and required sections;
- reconcile coverage against the tracked-source manifest;
- verify integrated artifacts against originals and record hashes/provenance;
- finish the parent full-reading ledger;
- check paths, relative Markdown links, diagrams and source-symbol references;
- verify no unauthorized implementation or instruction-file edits;
- run appropriate documentation checks and the project ownership check when the framework boundary/public-surface documentation is touched;
- report validation actually performed and remaining limitations.

Record exact validation commands/results in the snapshot. Do not imply production tests were run when this was static source investigation.

Update the living overview alongside future architectural changes. Keep snapshot evidence historical. Run a new full atlas only when accumulated change or unresolved architectural drift warrants it.

---

## 10. Concrete execution record: atlas-4355c02

| Field | Value |
|---|---|
| Repository cwd | `/Users/alxknt/github/iyon-n/iyon-tui` |
| Branch | `main` |
| Source revision | `4355c02d6853549adf32a1e038b14665ce5c6bf8` |
| Initial worktree | Clean before parent-created documentation |
| Context | Completed L1; post-PERF13 brief contains historical assumptions requiring source verification |
| Profile/model | `scout`, `openai-codex/gpt-5.6-luna:high` |
| Assignments | 45 total: 26 local/support, 3 wiring, 11 traces, 5 audits |
| Concurrency bound | 8 |
| Spawn budget | 45 |
| Workflow timeout | 14,400,000 ms (4 hours) |
| Execution | Async; fresh child context; source-read-only; managed per-report outputs |
| Ordering | 1–44 independent via `runs.all`; 45 consumes their output references |
| Workflow run | `b363950e-0ff9-4622-a8f0-491385679af2` |
| Mission | `7f501258-f18d-4e92-a4f9-2af6333e0938` |
| Validation at launch | Agent capabilities checked; workflow validation passed |
| Current acceptance | Full parent reading and corrected synthesis complete; validation linked below; unresolved technical dispositions retained |

### Reproduction inputs

- [Assignment index](atlas-4355c02/README.md)
- [Report contract](atlas-4355c02/REPORT-CONTRACT.md)
- [Machine-readable assignments](atlas-4355c02/evidence/assignments.json)
- [Exact executed workflow and prompts](atlas-4355c02/evidence/workflow.js)
- [Original collected1082-path manifest](atlas-4355c02/evidence/tracked-source-manifest.txt), [complete1088-path Git baseline](atlas-4355c02/evidence/baseline-tracked-manifest.txt), and [six-path closure](atlas-4355c02/evidence/coverage-addendum.md)

The recorded workflow contains machine-local cwd and baseline references. Review and update these, the scope split, source anchors and model policy before rerunning; do not execute it blindly after V5.

### Artifact routing in this run

Declared outputs follow `subsystems/<language>/<ID>-<name>.md`, `wiring/`, `traces/` and `audits/`. The tool resolves them under a managed workflow-specific output root outside the repository. Original artifacts remain available to assignment 45. The parent copies completed documents into the matching atlas subdirectories.

The first six reports were verified to exist and copied in one operation after the user noticed no report files in the repository. This was artifact-location confusion, not a failed scout or inability to write Markdown. No scouts were restarted.

### Final completion record

- [46 report provenance records and hashes](atlas-4355c02/evidence/report-provenance.json).
- [Parent full-reading ledger](atlas-4355c02/evidence/parent-reading-ledger.md) and [source-check notes](atlas-4355c02/evidence/parent-notes/).
- [Comprehensive technical inventory](atlas-4355c02/COMPREHENSIVE-REPORT.md).
- [Maintained human-readable guide](CURRENT-ARCHITECTURE.md).
- [Reconciliation](atlas-4355c02/RECONCILIATION.md) and [findings/disposition register](atlas-4355c02/ISSUES.md).
- [Exact final validation](atlas-4355c02/evidence/final-validation.md).

### Recovery and follow-up

The original workflow failed at its JSON metadata projection **after all44 independent scouts completed**, with `emit.reports[0].outputReference must be a JSON value; received undefined`. Assignment45 had not launched. The parent captured repo/ref/diff state, preserved completed artifacts and retried only the pending reconciliation through the same native workflow protocol. No completed scout reran and no external/foreground fallback occurred. Exact IDs, error and script are in [orchestration recovery](atlas-4355c02/evidence/orchestration-recovery.md); [recovery workflow](atlas-4355c02/evidence/recovery-workflow.js) records the corrected JSON handling.

After all45 reports were fully read, related deviations were grouped into one focused read-only continuation of retained scout45. See its [task](atlas-4355c02/evidence/grouped-followup-task.md), [execution](atlas-4355c02/evidence/grouped-followup-execution.md), [report46](atlas-4355c02/audits/46-grouped-deviation-followup.md) and [parent assessment](atlas-4355c02/evidence/parent-notes/46-grouped-deviation-followup.md).

Management resume reused the stored managed output path for45. Canonical repository45 had already been preserved and remained hash-identical; new managed bytes became canonical46. Future reproductions must preserve original bytes before resume and must not assume a requested new filename changes retained output routing. Provenance records this exception explicitly.

### Acceptance limits

This run added documentation/evidence only. No production source, dependencies, tests or AGENTS files were changed; no broad suite or benchmark was run. The required ownership gate was executed separately. Approximate scout LOC totals are not a disjoint census. Generated/fixture sampling and open safety/lifetime/cache/physical proofs remain stated limitations.

Recommendations from grouped follow-up were not accepted wholesale. Parent corrections reject type-only export as sufficient proof of hidden merged members, distinguish mount-graph preparation from physical receipt, and retain reachable ticket-miss uncertainty. The delivered map records these questions rather than claiming they were fixed.
