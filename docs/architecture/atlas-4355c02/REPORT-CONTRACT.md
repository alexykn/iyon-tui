# Current-state architecture atlas: investigation contract

## Objective and baseline
Map what actually exists at commit 4355c02d6853549adf32a1e038b14665ce5c6bf8 on main, including completed L1 changes. This is the factual prerequisite to the later architecture census, not the census/V5 redesign itself. Read PRE-V5-ARCHITECTURE-REPORT.md in full for evidence expectations, but do not execute its migration/disposition analysis. Current source is authoritative; label historical claims and disagreement explicitly.

## Authority and boundaries
Historical authority scope (explicit user clarification): only PERF-13, API-H, L1 and PRE-V5 documents are relevant handoff authorities for this investigation. Older documents are deprecated and have no normative weight. They may explain incidental history, but disagreement with them alone is not a current issue. Within the relevant families, distinguish approved current requirements, superseding decisions and proposed future designs.

Applicable approved handoffs are the default oracle for intended architectural contracts. Source is authoritative for what currently executes, not automatically for what is correct. Check for superseding approved handoffs before labeling a requirement obsolete. An unsuperseded mismatch is a deviation requiring explanation. If the current implementation may genuinely improve on the handoff, identify the evidence and tradeoffs for a focused follow-up investigation; do not declare it justified solely because it exists or was intentional. The parent will group related candidate deviations for further scout investigation and final judgment.

Read-only investigation. Do not edit project files, commit, push, install dependencies, run other agents, or start expensive build/benchmark suites. Return the complete detailed Markdown report as your final response; the workflow saves it to an isolated managed output artifact. The parent alone integrates reports into the repository. Do not shorten the deliverable to a handoff summary or claim a report exists elsewhere.
Only the parent may spawn agents. Escalate genuine infrastructure or access blockers; continue useful in-scope inspection. No external-agent fallback.
Mandatory framework boundary: iyon-tui and its TS facade are generic terminal mechanics, semantic presentation, native input, retained structure/state/content, layout/paint and scheduling. Iyon agent/application meaning belongs in TS product plugins, not generic Rust/facade code. Distinguish caller-supplied values from framework policy; report observed violations without modifying code. Do not assume product plugins or external consumers are present in this repository.
ARCHITECTURE.md was deliberately deleted as deprecated; do not resurrect its historical assertions. Read AGENTS.md and applicable scoped instructions.

## Required report format
Use exactly these broad headings (add detailed subsections/tables/diagrams as useful):
# <ID> — <assignment title>
## 0. Baseline, scope and evidence status
Commit, paths, scope boundaries, facts/inferences/unknowns, static inspection versus executed validation.
## 1. Responsibility and structure
Detailed module/file inventory and approximate production/test/generated physical LOC with counting method; primary and secondary responsibilities.
## 2. Types, APIs and contracts
Exact symbols, intentional public surfaces vs internal machinery, actual consumers, invariants.
## 3. Dependency and ownership map
Forward/reverse edges, owner/create/destroy, identity and lifetime, at least one useful diagram.
## 4. Execution paths and state transitions
Real call chains, create/first use/update/replacement/reset/destruction, appropriate plane interactions, mutable and immutable data.
## 5. Alternate routes and failure semantics
Semantic-operation -> production paths table; selection conditions, cache misses vs recovery vs compatibility, failure masking; absence claims require search scope.
## 6. Caches, invalidation, scheduling and performance
Keys/bounds/retention, invalidation, per-append/tick/frame/width work, observed counters; N/A with reason if not relevant.
## 7. Tests, benchmarks and observability
Behavioral contracts, route assertions, missing visibility, precise files/tests/counters. Do not claim tests ran unless they did.
## 8. Cross-boundary findings and contradictions
Evidence-backed couplings, doc/source disagreement, overlapping ownership, references to other assignment IDs where useful. No V5 disposition decisions.
## 9. Open questions and coverage gaps
Explicit unanswered questions and limits; no unsupported complete/unused/migrated claims.
## 10. Evidence appendix
Paths :: exact symbols (line ranges optional), commands, inspected-file manifest (grouped exhaustive list), files merely indexed/not read, LOC methodology and counts.

## Depth and source discipline
A detailed technical report, not an executive summary. Explain enough that the parent can reconstruct behavior without rereading your whole scope. Read assigned production files comprehensively; if output truncates, read remaining chunks. Follow dependencies to prove seams; avoid generic prose. For huge fixture/generated sets, index and explain generation/contracts rather than repeating all records. State which were sampled. Tests are behavioral evidence.
Each report must distinguish current source, historical documents, static inference and observed execution. Report missing features/consumers honestly. Do not invent assistant/app paths where only generic framework examples exist. Cross-cutting assignments use the same structure adapted to their concern, not artificial LOC ownership.
