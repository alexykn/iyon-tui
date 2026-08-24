# Repository separation S0 baseline

This directory freezes the pre-separation state required by `IYON-TUI-REPOSITORY-SEPARATION-HANDOFF.md` §6.

- Source revision: `bd503b0382e34d74a38c562b9662d08c8c96f58a`
- Source branch: `perf-refactor`
- Source remote: `git@github.com:alexykn/iyon.git`
- Source worktree: clean before capture
- Canonical post-record ref: annotated tag `pre-iyon-tui-repository-separation`
- Behavior changes: none

## Frozen artifacts

| File | Purpose |
|---|---|
| `environment.json` | Git and toolchain provenance. |
| `hosting.json` | Repository identity, branches, workflows, issues, pull requests, releases, tags, and cutover disposition. |
| `dependencies.json` | Direct Cargo and TypeScript package dependency/manifests graph. |
| `ownership.tsv` | Destination and required migration action for every one of the 1,509 tracked source-revision paths. |
| `api-surface.json` | Public TypeScript TUI values/types, declared and actual native addon surfaces, Rust inventory/mapping hashes and counts. |
| `artifacts.json` | Native artifact, locks, schema, generator input, ABI metadata, versions, sizes, and hashes. |
| `test-benchmark-inventory.tsv` | Hash-addressed inventory of tracked test and benchmark files. |
| `perf-artifacts.tsv` | Hash-addressed PERF documents/raw artifacts and their embedded full Git SHAs. |
| `perf-history.tsv` | Original commit identities touching the PERF-12/T13.1 evidence chain. |
| `checks.md` | Mandatory S0 checks with exact passing counts and known failures. |

## Inventory summary

- Cargo workspace packages: 7.
- TypeScript package manifests, including committed loader fixtures: 31.
- Tracked test source files: 226.
- Tracked runtime benchmark directory files: 65, including source and raw evidence.
- PERF raw artifacts (`.json`, `.jsonl`, `.log`): 36.
- PERF/TUI performance documents in the frozen artifact registry: 19 architecture/performance documents plus related reports captured by `perf-artifacts.tsv`.
- Public `@iyon/runtime/tui` runtime values: 55; exported types: 34.
- Actual staged addon exports: 34; generated View ABI functions: 57.
- Rust public-surface mapping records: 2,331 total, including 1,532 for `iyon-tui`.
- GitHub at capture: 3 branches, 4 workflows, 0 tags, 0 issues, 0 pull requests, and 0 releases.

## Ownership resolution

Every tracked path has one non-empty destination and one migration action in `ownership.tsv`:

| Destination | Paths | Meaning |
|---|---:|---|
| `iyon-tui` | 1,064 | Remains in the renamed current repository or moves into its dedicated TUI package/native crate. |
| `iyon` | 380 | Included in the filtered application repository and removed from the TUI repository HEAD in its assigned tranche. |
| `both-derived` | 64 | A known mixed/root seam. It must be independently split, rewritten, or minimized according to the recorded action; it is **not** authorization to copy a shared facade verbatim. |
| `retire` | 1 | Non-product root artifact removed after cutover and not treated as architecture. |

There are zero unclassified or unresolved paths. `both-derived` means the owning work is explicit (for example split native contracts, workspace manifests, workflow sets, inverse `AGENTS.md` rules, or minimal per-repository API-surface tooling). It does not mean shared runtime ownership.

## Hosting disposition

The current repository remains canonical for all original Git and PERF identities and will be renamed `alexykn/iyon-tui` without filtering or rewriting history. The application repository will be filtered from application-owned paths and will carry `SOURCE-SHA-MAP.jsonl`. No active issue, pull request, release, or existing tag requires migration at capture.

No remote mutation or push is part of S0. The annotated pre-separation tag is local until an explicit later push/cutover action.

## Verification posture

S0 is an evidence freeze, not a cleanup tranche. Passing checks and all discovered baseline failures are recorded in `checks.md`. In particular, formatting, Clippy, API-surface drift, one order-dependent runtime test, one reproducible app viewport test, and the missing native verification script are recorded without being repaired or summarized as green.
