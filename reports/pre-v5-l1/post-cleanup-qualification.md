# Post-cleanup qualification: accepted for macOS arm64

## Decision

The parent accepts the feature-preserving cleanup, Source append/wake fixes,
and bounded cache policy after personal diff review and independent checks.
Under the user's explicit delegation of acceptance judgment, the parent also
accepts the measured small-append cost as a **specific local tradeoff**, not a
blanket regression tolerance: persistent snapshot correctness, substantially
faster large appends/frame work, and bounded revision-history retention justify
the remaining small-operation overhead. The append phase is still slower in
some workloads and tails; those measurements are not averaged away.

**Accepted for the owner's macOS arm64 use.** After reviewing the remaining
platform gap, the owner explicitly stated that full platform qualification was
unnecessary for this single-user macOS project. Linux x64 and the other mapped
targets therefore remain unqualified but are **not acceptance blockers** for
this delivery. Existing package mappings and CI definitions were not removed.
The original implementation is not smaller overall, and the complete original
§19 workload/platform matrix is not claimed. Acceptance is scoped to the
reviewed implementation, measured local tradeoffs, and qualified macOS paths;
it is not a retroactive assertion that every original target or expectation
was met.

## Provenance

- Original implementation baseline: `90f19f5c9057ebb41fd1f8ffc73adbab05656cbc`.
- Performance baseline: `36efe16f90ac7ef3c758c65ee35a0502d6b95ea1`, isolated at
  `/private/tmp/iyon-l1-00-baseline`.
- Corrective commit: `313689e2976c020ea539e43d66e1ed88dcda680e`; subsequent
  cleanup/qualification remains uncommitted.
- Final tracked source diff SHA-256 (`git diff --binary HEAD -- crates
  packages/iyon-tui/src`):
  `82df37f64c1df2ce134e5c67d57af3aad3acc19617e8765b12b10cae7f662ec6`.
  This hash excludes the untracked 477-line application test driver, which is
  part of the accepted cleanup and must be included when committing.
- Host: macOS 26.6.2, arm64; Bun 1.4.0; Rust/Cargo 1.97.1.
- Final default addon SHA-256:
  `86506ed3623aa1b1236b8541068cb2052a4dd282e5739ea9adfbfe5374f38238`.
- Baseline addon SHA-256:
  `5a9cedeee661e8a59c0aa0cf9401783217cef0bf0107bd5133ba83460b86a6f9`.

The three-stage Luna workflow completed scouting and cleanup, then hit its
three-hour deadline during qualification. The parent captured the partial diff
and verified process/worktree state before same-protocol child recovery. No
CLI fallback or concurrent writer was used. The final child handoff is the
managed `qualification.md` output of workflow
`a8121576-161d-430a-ba79-6c4d6996492f`, completed through retained child
`4598b56e-82a9-4a08-9dce-729ac6723577`.

## Authoritative implementation reviewed

- Removed unused private authoring aliases, helpers, and standalone Rust run
  entry points. Existing driver assertions remain in test-only support.
- Unified persistent and unique-owner append paths around one right-edge
  `Arc::make_mut` insertion routine. Shared nodes retain immutable snapshot
  semantics. Only prevalidated unannotated, non-truncating input takes the
  unique-storage fast path; other mutations retain atomic fallible behavior.
- Cached immutable Connector port identity and skipped unnecessary immediate
  delivery deadline synchronization. Parent review identified an exceptional
  poisoned/retired Port path; the final visible fast path checks weak-owner
  upgrade and poison status without acquiring the Port mutex, preserving the
  environment-visible failure path.
- Limited block/semantic-sequence caches to four entries and content-derived
  caches to two. Strong Block/Raw ownership remains while pointer-key entries
  exist. Visible/in-flight products retain their own owners outside caches;
  evicted historical inputs can be recomputed.
- Closed denied Clippy errors by private alias/import cleanup and an equivalent
  RGB parser `if let`, without suppressions, dependency changes, or public API
  changes. Existing warning backlogs remain warnings.

## Independent final-image timing

The parent ran 20 alternating fresh-process pairs of the byte-identical
`perf13_h_content.ts` script with 1,000 appends and 64 KiB retention, using the
final and baseline addon hashes above. Times below are milliseconds; p50 is
the midpoint of the middle samples, p95/p99 use nearest-rank order statistics.

| Phase | Baseline p50 / p95 / p99 | Final p50 / p95 / p99 |
|---|---:|---:|
| Append acceptance | 2.7015 / 2.9726 / 3.1399 | 2.8833 / 3.8731 / 3.9500 |
| Frame flush | 11.0060 / 11.6411 / 13.0799 | 4.6101 / 4.9290 / 5.2536 |

Final append p50 is about **6.7% slower**, with higher tail latency (p95 about
30% slower in this run). Frame p50 is about **58% faster**. The original
35% median append defect was substantially reduced, not replaced by a claim
that every append percentile is faster. Twenty processes provide bounded
qualification evidence, not a universal statistical guarantee.

The child's earlier 20-pair source-only workload measured p50/p95/p99 ratios
of **1.093 / 1.101 / 1.138**. It used the pre-final-Port-guard image
`34d395cd2544ea224440ae44525641b2115df65cc6322032bcde5ac4057bca92`;
the later guard does not run in source-only operations. The child's six-process
10K-burst comparison measured append p50 158.952 → 17.495 ms and frame p50
130.704 → 79.690 ms on its earlier qualified image. These are distinct
workloads/images, not substitutes for the final paired 1K measurements.

## Memory findings

The original 1024-entry block cache strongly retained historical semantic
Blocks/Views. In the mounted append/flush workload, a 10K process reached
roughly 579 MB RSS and macOS inspection reported about 466 MB reachable malloc
allocations. Source-only retention and static flushes did not exhibit this
growth; reducing the sequence cache alone did not resolve it.

With the final cache policy, fixed-64-KiB-retention process RSS at 5K/10K/20K
appends settled at approximately **121 / 141 / 146 MB**. At the paused 10K
inspection, physical footprint was 68.9 MB, malloc-zone allocation 82.2 MB,
and `leaks` reported about 39 MB malloced with zero leaks. The baseline's
corresponding live-memory measures were lower in some categories (54.9 MB
physical footprint, 78.9 MB allocated, about 14.5 MB malloced), so this is
evidence of removing excessive historical retention and achieving a plateau,
not a claim of lower memory in every metric. Teardown left zero leased native
slots and a bounded expired weak entry awaiting maintenance.

These measurements used the pre-static-cleanup image identified above; cache
ownership/lifetimes did not change in the final aliases/Port-guard follow-up.
RSS, allocator high-water, reachable allocations, and proven leaks are distinct.
No complete Rust live-object census or cross-platform memory result is claimed.

## Verification and platform boundary

Parent-independent final checks passed:

- Exact repository Clippy gate (existing warning backlog retained).
- 21 all-feature Source storage tests.
- 22 native-facing regression tests across seven files, 2,591 expectations:
  cache ownership, retained scene, malformed RGB, wake/failure, lifetime,
  Smooth, and History prefix behavior.
- Ownership (81 Rust records), binding (124 exports), formatting, diff check,
  TypeScript lint (warnings/info only), and native smoke.

Child final-image evidence additionally passed the all-feature workspace suite
(719 core tests plus native/generator suites), 714 native-host library tests,
60 native tests, 123 Bun tests with 3,332 expectations, typecheck, ABI drift,
and public declaration checks. No repository tests were added. One existing
timing-sensitive receipt test failed once during the campaign; its targeted
rerun and subsequent complete suite passed.

| macOS arm64 profile | Final addon SHA-256 | Evidence |
|---|---|---|
| default | `86506ed3623aa1b1236b8541068cb2052a4dd282e5739ea9adfbfe5374f38238` | stage/load/smoke and final Bun tests |
| fast-view-abi | `9a7c5c750fb17fc1c5fd8c7ce11497b35118fdf58cb0639c6c83f891607f3e03` | stage and malformed-input containment |
| direct-ffi | `abbc18d0c83618113d1f419e0756c06f7ae2ab44afe4d13ea9b7a3bf261d93a0` | stage and direct structural smoke |
| perf-counters | `ebcbc07f8df49952c54a812c463bf41f40f1573bbde45195b0292f3b11d6b9c3` | stage/content smoke; full earlier-image counter lanes identified in child handoff |
| direct-ffi + perf-counters | `9c0831c6e0e6c45927e47f6ac47d9a98acac72f7aa69b42604cc5385ce071d8d` | stage and symbol/load checks |

Linux x64 (CI matrix), Linux arm64, Darwin x64, and Windows x64 have no local
build/runtime qualification. The owner explicitly excludes those missing
results as blockers for this macOS delivery. No Miri, sanitizer, TSan, or
foreign-target pass is inferred from macOS checks. No remote CI was dispatched
or code pushed.

## Final size accounting

After cleanup, qualification fixes, and static cleanup, versus `313689e`:

| Category | Added | Deleted | Net textual change |
|---|---:|---:|---:|
| Rust runtime | 274 | 733 | −459 |
| Rust test/support | 571 | 74 | +497 |
| TypeScript runtime | 0 | 32 | −32 |
| Generated | 0 | 0 | 0 |
| Total source, including new test driver | 845 | 839 | **+6** |

Runtime snapshot counts are 56,685 Rust and 16,487 TypeScript lines: 690 and
32 fewer than `313689e`, respectively. The Rust runtime decrease additionally
includes test-only reclassification; it is not all physical deletion. Runtime
remains **8,883 Rust lines and 11 TypeScript lines larger than `90f19f5`**.
The earlier cleanup-only −117 physical-line result is historical; the final
combined source change is +6. Reports/evidence are excluded from these counts.

## Evidence retention

`post-cleanup-qualification-evidence.json` preserves the parent's final paired
timing samples, the child's source-only samples, memory plateau data, and their
image identities. Detailed logs remain in `/tmp/iyon-parent-qualification-*`,
`/tmp/iyon-parent-final-h1k-*`, and the child's `/tmp/qual-*` files. The managed
child handoff contains the full workload matrix and command/log mapping.
The historical `L1-13-report.md` draft was preserved and was not relabeled as
passing. Final implementation review and acceptance authority remain here and
in `final-implementation-review.md`, not in a child verdict.
