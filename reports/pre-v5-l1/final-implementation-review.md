# L1 implementation review and L1-13 corrective closure

## Scope and authority

- Remote fetched before review: `origin/main` = `90f19f5c9057ebb41fd1f8ffc73adbab05656cbc`.
- Reviewed tip at start: `8aff2f8697cbd2106ad71ea3f8091a4b349e5e96` (`main`, 23 commits ahead).
- User requests a personal parent review of the entire implementation, not only L1-13, and authorizes corrective changes. Luna implements confirmed fixes; the parent reviews their diffs and evidence.
- Performance qualification is paused. Existing untracked benchmark fixtures, draft report, and target directories are preserved, not part of this committed review range.
- Decision: the parent approves the F1–F17 corrective implementation after personal code review and focused verification. Full L1-13 acceptance under §19–21 remains open; performance qualification has not been resumed or waived.

## Correction-pass decision

The original review range remains `90f19f5..8aff2f8`. The checkout subsequently
advanced through planning-documents commit `d199bf5` and instruction commit
`5ffab4f`; neither replaces the implementation review baseline. Corrective code remains uncommitted. The final child
timed out after saving its handoff; read-only same-protocol recovery confirmed
its implementation and checks were finished. No completed stage was rerun.

The following dispositions reflect **parent inspection of actual code**, not
blanket acceptance of child verdicts. Historical finding descriptions below
record the original defects; this table supersedes their old scheduling status.

| Findings | Correction / parent disposition |
|---|---|
| F1, F2, F4, F5 | Reviewed: UTF-8 retention stops at the descriptor boundary; ordered annotations materialize lazily; annotation identities preflight atomically; chunk statistics count retained descriptors. |
| F3 | Reviewed: accepted Source mutation remains successful; failed host wakes use the environment error channel without suppressing healthy fan-out. |
| F6 | Reviewed: obsolete Rust authoring root removed; binding exports use private owners; nine semantic test files moved byte-for-byte; root boundary gate passes. Hidden, feature-gated `perf_bench` remains an explicit package-binary tooling exception. |
| F7 | Reviewed: semantic sequence cache retains its Block/Raw owners, preventing pointer-reuse stale output. |
| F8 | Reviewed: native state masks and generator lane shapes reject malformed values before mutation; ignored/null lanes retain their contract. |
| F9 | Reviewed: parser reuse is bound to Source/content lineage; replacement/clear resets parser state, not Smooth policy or old candidate ownership. Parent mounted regression passes. |
| F10 | Correction diff reviewed: RawDomain retains page witnesses and assembles only a selected working region; Markdown trailing-line checks and plain newline traversal no longer force whole-domain assembly. The parent independently reran the deterministic working-region regression successfully. |
| F11 | Reviewed: candidate-free bootstrap receipt failure no longer expects a nonexistent state commit. |
| F12 | Reviewed: a retained candidate is retried before newer desired structure/state/Source work is prepared. |
| F13 | Reviewed: root publication invalidates affected retained-state dependency paths before clearing their worklist. |
| F14 | Reviewed: content incremental work does not erase the full-paint obligation needed to recolor ordinary siblings. |
| F15 | Reviewed: component replacement refreshes the exact demanded state map for body/History branches, without unbounded obsolete state retention; old candidates retain their own owners. |
| F16 | Reviewed: theme copy-on-write occurs at `theme_mut()`, not merely when creating an update context. |
| F17 | Reviewed: malformed six-byte non-ASCII RGB input is rejected before byte slicing instead of aborting across N-API. |

Parent spot-checks on the corrective tree include:
- wake/runtime tests: 11 passing, 55 expectations;
- parser-generation mounted regression: 1 passing, 8 expectations;
- retained-scene/cache tests and demanded-state ownership regression recorded in the review notes;
- all nine relocated semantic test files are byte-for-byte identical to their prior files;
- binding and ownership gates independently pass (124 binding exports; unchanged 44-value/97-type public TypeScript snapshot).

Child evidence additionally records focused Source/native/parser/generator
checks, generated drift validation, and a final all-feature Rust library run
with 719 passing tests and one ignored benchmark. Evidence is not a claim of
unperformed platform or performance qualification.

**Approval boundary:** the correction pass is approved; this is not full L1-13
acceptance. The literal §19–21 performance, memory and platform evidence is
incomplete and has not been waived. Historical reports below remain historical
records, not substitutes for final qualification. The implementation is also
not claimed to be smaller overall; the accounting below records that limit.

### Final same-artifact verification

The parent staged the complete corrective tree with `bun run native:stage`
(default N-API, darwin-arm64), then ran the focused native-facing regressions
against that artifact:

- `tui_native_input_validation`, `tui_retained_scene_regressions`,
  `tui_semantic_cache_ownership`, `tui_ansi_scanner`,
  `tui_history_prefix`, and `tui_smooth_delivery`:
  **14 tests passed, 2,586 expectations**.
- `bun run typecheck`, `bun run check:tui-abi`,
  `bun run check:tui-binding`, `bun run check:ownership`,
  `cargo fmt --all -- --check`, and `git diff --check`: **passed**.
- The generated manifest's 17 state-property records were independently
  compared with the canonical TOML schema: all match.
- Final addon SHA-256:
  `15188a508c2a0b2996b8e124b87117432218b33032b507b50bbe3070e575505a`.
- The final metadata follow-up removes four nonexistent/private associated
  View-method entries. There are **81 mapping/snapshot records** and **124
  binding exports**; these are different inventories, not competing counts.
  The retained NativeTextPage methods are public native-host binding methods.

Detailed parent command logs are in `/tmp/iyon-parent-final-*.log`.
No full timing campaign or cross-platform claim follows from these checks.

### Size and test accounting

The final F6/F16 slice removes obsolete authoring/test support rather than
replacing it with another public facade. Its nine relocated semantic test
files retain all 2,663 original lines byte-for-byte; those moves are not
deletions of behavior. The slice reports 337 net fewer test/support lines,
including removal of the unused Rust testing facade, while its production
visibility/COW plumbing is about 25 net additional Rust lines.

Do not confuse the large documentation/mapping deletion with a smaller native
implementation. Across `crates/`, comparing the original `90f19f5` baseline
with the corrective worktree and including new untracked/moved crate files,
the measured raw difference is **+17,370 lines**:
26,062 tracked additions − 11,355 tracked deletions + 2,663 new-file lines.
This count includes tests, generated files, documentation and manifests;
it is not production-only LOC. The handoff's hoped-for overall size reduction
has therefore **not been demonstrated**. No feature removal or future-v5
redesign was undertaken to manufacture a lower count.

### Remaining finalization work

1. Resume or explicitly revise the paused §19 performance/memory qualification,
   including the previously unwaived tiny-append regression and same-image
   platform/profile matrix.
2. Resolve the handoff's overall size/simplification expectation using honest
   production/test/generated accounting; the metadata deletion is not evidence
   that this expectation is satisfied.
3. Publish final L1-13 acceptance only after those outstanding requirements are
   satisfied or explicitly amended by the owner. The existing checkpoint and
   untracked performance draft must not be relabeled as a passing final report.

Corrective source changes have not been staged, committed, or pushed by this
review pass. The user/external documentation commits are preserved.

## Requirements and commit coverage

The parent re-read all of `PRE-V5-RUST-LOWERING-HANDOFF.md` (1–1971)
during this final pass. Review scope is explicitly **L1-01 through L1-13**.
The following are the primary tranche commits, not isolated review ranges:
later commits modify earlier stages, so each stage is checked at the final tip
and against the remote baseline. The complete 23-commit list also includes
L1-00 characterization/repairs, lint preparation, reports, and the user-owned
instruction update.

| Tranche | Primary commit | Requirements to verify at final tip | Status |
|---|---|---|---|
| L1-01 | `37c275d` | §5 narrow binding/public deletion, ownership and TS compatibility | reviewed; F6 root/binding closure and TS surface gates pass |
| L1-02 | `3e28006` | §6.1/6.3/6.7 final roots, decoration precedence, attachments and cache-first leases | reviewed; direct final-root factories, precedence and leases inspected |
| L1-03 | `e0348bb` | §6.2 text ownership, 1–4/N spans, NUL/UTF-8 and matching generated adapters | reviewed; page ownership/UTF-8 ingress and generated adapters checked |
| L1-04 | `8e1b0cc` | §6.4–6.6 persistent edits/grid placement, all kinds and static Diff metadata | reviewed; persistent edit/grid placement implementation and migration inspected |
| L1-05 | `8e63141` | §7.1–7.2 generated state masks, full validation and atomic null/clear semantics | reviewed; F8 exact typed-value validation and generator shape correction |
| L1-06 | `ed5f45d` | §7.3–7.7 changed-version overlay, effective base, lifecycle and native effects | reviewed; F13/F15 dependency invalidation and captured state ownership corrected |
| L1-07 | `4fa8333` | §8 semantic styles/theme selectors, ordering, lifetime and revision separation | reviewed; typed theme/style parity, F16 lazy COW and F17 safe RGB decoding |
| L1-08 | `d1541c1` | §9 persistent Source, annotations, truthful acceptance, UTF-8/FFI and retention | reviewed; F1/F2/F3/F4/F5 storage, accounting and truthful acceptance corrected |
| L1-09 | `4706fa7` | §10–11 semantic reuse, restart/provenance and fresh-parser equivalence | reviewed; F9 lineage and F10 working-region fixes verified |
| L1-10 | `86afe80` | §12 current Smooth policy, tick-only work, independent clocks and committed frontiers | reviewed; existing Smooth policy preserved; focused final-addon regressions pass |
| L1-11 | `407b73f` | §13 cached lowering, captured tickets, viewport clipping and History receipts | reviewed; F7 cache-owner lifetime and captured content painting inspected |
| L1-12 | `e4a5805` | §14 targeted invalidation, prepared promotion, pending epochs, wakes and failures | reviewed; F11–F15 receipt ordering, failure cleanup and invalidation corrected |
| L1-13 | `8aff2f8` | §15/17 deletion census, controls, test migration and §21 line-by-line closure | corrective implementation approved; full §19–21 qualification remains open |

Cross-cutting checks: §18 behavior/failure matrix, actual generated/native
artifacts and package profiles in §19.1/19.5, and honest evidence under §20.
Timing/memory qualification under §19.2–19.4 remains paused. Deterministic
algorithm/ownership requirements remain part of the implementation review;
pausing benchmarks does not waive them.

## Review plan

1. Source storage, annotations, snapshots, UTF-8 and FFI lifetimes.
2. State schema/transport, typed mutation, captured state and lifecycle.
3. Content parsing, semantic reuse, smoothing and History frontiers.
4. Candidate preparation, external receipts, wake routing and teardown.
5. Persistent structures, construction, layout, style, row painting and clipping.
6. Native binding/generated ABI, panic containment and feature profiles.
7. TypeScript compatibility, tests removed or weakened, checks/CI and documentation.
8. Integrate confirmed fixes; personally inspect code and run focused/full appropriate checks.

## Findings

### F1 — L1-08 byte retention can discard the entire valid suffix

**Confirmed, awaiting Luna fix and parent verification.**

`StoredSource::next_boundary` (`application/source_store.rs:1452–1476`)
returns the whole Source end when the requested offset is inside the final
multibyte scalar of a nonfinal chunk. The correct next boundary is that
chunk's end. `offset_for_max_bytes` uses this result when no later newline
exists, so ordinary drop-oldest retention silently drops valid later chunks.

Parent reproduced through the actual default TypeScript/FFI route at
`8aff2f8`: create stream with `{maxBytes: 5, overflow: "drop-oldest"}`,
append `"abcé"`, then `"tail"`. Actual snapshot is empty with base/end `9/9`
and dropped bytes `9`; expected retained text is `"tail"`, base/end `5/9`,
dropped bytes `5`, partial head true. This violates §9.4–9.7 and §18.3.
Required coverage includes separate append chunks and 16 KiB page seams,
multibyte widths, retained native old snapshots, and unchanged newline-first
retention behavior.

### F2 — L1-08 eager annotation materialization defeats persistence

**Confirmed implementation requirement gap; fix design pending caller audit.**

`StoredSource::apply_append` unconditionally executes
`Arc::from(next.annotations.in_application_order())`, including when no new
annotations exist. This traverses/clones/sorts every surviving annotation on
each text append. Truncation likewise materializes every surviving annotation.
The persistent index therefore coexists with an eagerly rebuilt whole-array
representation, contrary to §9.4 and the L1-08 stop gate. Snapshot creation
itself remains cheap. Review callers before choosing lazy diagnostic
materialization versus persistent application-order storage; preserve overlap
precedence and frozen-snapshot semantics. This is deterministic work visible
in code, not a new timing qualification claim.

### F3 — accepted Source wake failures are erased at the FFI boundary

**Confirmed by end-to-end code-path inspection; failure-injection coverage required.**

`HostContentSource::finish_mutation` returns an `Err` containing
`SOURCE_WAKE_FAILED` after installing the revision and attempting other hosts.
`content_ffi::status_for_diagnostic` has no mapping for that code and reports
`INTERNAL_INVARIANT`; mutation output stays zero. TypeScript `finishMutation`
throws before decoding the result or forwarding the drain hint. The caller
therefore loses the accepted revision and healthy-host wake disposition.
The existing poisoned-subscriber test checks only Rust error prose and does
not prove healthy-host presentation or FFI semantics. The earlier `36efe16`
repair continued fan-out but did not close this boundary defect.

This violates §9.6/14.6/14.7: retain successful acceptance, report the failed
host through the environment error channel, preserve healthy-host scheduling,
and do not introduce a new public ABI just to carry an ordinary rejection of
already accepted bytes. Verify actual FFI results plus fair, no-spin draining.

### F4 — Source annotation acceptance-order counter is not preflighted

**Confirmed code defect; forced-boundary reproduction required.**

Storage mutation uses `next_seqno.saturating_add(1)` for accepted annotations.
Multiple annotations per revision can exhaust this counter before the Source
revision, creating duplicate `(start, seqno)` keys and losing strict overlay
ordering/index invariants. §9.5/18.3 requires all fallible identity/counter
arithmetic to be rejected before installation. Validate the complete batch's
sequence range before constructing/installing it, and assert bytes, annotation
order, revision, accounting, membership and wakes remain unchanged on rejection.

### F5 — public Source chunk statistics now count tree leaves

**Confirmed against baseline and reproduced on the staged addon.**

`StoredSource::chunk_count` returns `ChunkTree::leaf_count`, not the count of
retained chunk descriptors/pages. Two one-byte appends report `chunkCount: 1`
despite retaining two pages; the remote baseline reports `storage.chunks.len()`.
Maintain descriptor counts in the index and preserve snapshot/stats semantics
through append and partial truncation. This is a public diagnostic behavior
regression, not a request to change chunk coalescing policy.

### F6 — L1-01/L1-13 still expose the old Rust extension vocabulary

**Confirmed contract gap; private-test migration and boundary enforcement required.**

The crate root says its only public bridge is `binding`, but still publicly
exports `Projector`, `ProjectorExt`, `Scene`, `History`, controls, host operations,
and broad `text`, `projection`, `stream`, and feature-gated `testing` namespaces.
`text.rs` blanket-reexports semantic authoring/traversal/projector APIs. External
Markdown/projection tests still depend on those paths. Removing View's fluent
methods did not complete the single unsupported binding seam required by §5.1
and L1-13. `check-binding.ts` checks native imports and three banned names, not
the remaining external root/module closure, so it misses this residue.

Keep the algorithms and necessary native passive types/operations. Migrate
private algorithm tests without dropping their assertions; narrow external
reachability to the required unsupported seam (with any benchmark-only surface
explicitly justified), add negative export/closure checks, and update misleading
docs. Do not remove TypeScript functionality or its generic extension contracts.

### F7 — L1-11 sequence cache reuses a freed Block address for different content

**Confirmed by ownership inspection and reproduced through TypeScript rendering.**

`SemanticSequenceEntry.items` stores `SemanticItemKey::Block { block_ptr }`
without retaining that Block owner. The separate `BlockLoweringCache` correctly
retains owners, but clears after 1024 entries while sequence entries remain.
Sequence hits bypass `lower_block` entirely, so an allocator-reused Block address
can select an older View even though the Source revision is new.

Parent reproduction at `8aff2f8`: one mounted Markdown Connector over a block
Source; repeatedly replace with unique `item-NNNNN` strings and flush, asserting
the nonblank screen equals the current input. At iteration 1025, expected
`item-01025`, actual `item-01007`, while Connector status claims represented
Source revision 1026. Further stale results occurred around the next eviction.
The first probe incorrectly assumed content occupied row zero; the corrected
probe compares all nonblank rows and demonstrates actual stale output.

Each pointer-keyed sequence must retain the complete immutable identity owner
or use a collision-safe non-address identity. Audit sibling caches for the same
assumption. Preserve bounded residency and persistent append reuse. Require a
deterministic lifetime/eviction regression and repeated-replacement physical
coverage; matching cache counters alone is insufficient.

### F8 — L1-05 native value-lane masks silently accept unknown bits

**Confirmed by decoder inspection and actual default N-API calls.**

The generated envelope header checks property masks, but the handwritten
`read_alignment`, `read_border_edges`, and `read_text_attributes` readers
discard unknown value bits. At `8aff2f8`, direct native `setGeometry` accepts
alignment word `0x80000001` and edge-object bits `0x80000001`;
`setPresentation` accepts text-attribute presence/value `0x80000001`.
All return success instead of rejecting the complete malformed patch.
Attribute value bits outside their presence mask likewise go unchecked.
Public TS normalization normally prevents these inputs, but §7.2 explicitly
requires native scalar/mask validation before installation; the native seam
must not rely on the JS caller for this.

Validate meaningful value lanes (including nested Style attributes) against
their declared domains, while preserving absent/null lane don't-care behavior.
Add mixed valid-plus-invalid patch regressions proving no revision/effect/
override changes. Check generator/schema constants rather than duplicating
another unrelated mask authority.

### F9 — L1-09 parser state survives Source replacement/clear generations

**Confirmed through actual mounted Markdown, ANSI, and Diff TypeScript routes.**

`replace_utf8`/`clear` advance `content_generation`, and outer projection keys
include it, but `ConnectorExecution` does not track/reset parser lineage.
Markdown dropped its old byte-prefix equality guard; ANSI/Diff resume solely
from base/end offsets. A new logical document at the same coordinates therefore
inherits completed content and parser context from the old one.

Parent reproduction at `8aff2f8`, immediate Markdown block Source:
replace `first\n\nlast`, flush, then replace `other\n\nlast`, flush.
The screen still says `first / last`, while Connector status claims revision 2.
Replacing with `third\n\ntail` produces `first / tail`. ANSI and Diff likewise
render `first / tail` after `first\nlast` → `other\ntail`.
This is independent of F7's allocator/cache eviction defect.

Track the complete logical Source lineage at the execution boundary and reset
stateful parsers before consuming a replacement generation, including clear
followed by append (even if the empty intermediate Source was never flushed).
Preserve genuine append reuse and Source-rooted coordinates; audit handling of
non-append inputs and multiple Raw domains inside the projector machinery.
Check Smooth's established replacement policy before changing its clock/frontier
semantics merely as a side effect of a broad execution reset. Add physical
end-to-end replacement cases and fresh-parser equality coverage for all three
stateful formats, including retained old candidate products and error recovery.

### F10 — L1-09 still concatenates the complete Source before suffix parsing

**Confirmed deterministic implementation gap; performance timing remains paused.**

`source_projection` builds page-backed spans, but every stateful projector
immediately calls `RawDomain::from_spans` on the entire consecutive Raw domain.
For more than one span that function copies every retained byte into a fresh
`String` and then an `Arc<str>`. Only afterward do Markdown/ANSI/Diff choose their
resume suffix. Thus an ordinary multi-chunk append recopies the complete Source
even when grammar work only needs its tail. This is the exact full rehydration
that §11.2 prohibits, not a question about benchmark variance.

Give parser execution a source-witness-preserving working region and select/
reuse the required region before assembling contiguous bytes. Retain legitimate
full-domain grammar restarts (e.g. Markdown reference context); count assembled
bytes as well as parser bytes. Preserve fresh-parser equivalence for every
prefix, Source generations, chunk/page seams, retention constraints, and sealed
inputs. Do not repair this by losing provenance or by resurrecting a second IR.

## File coverage

`pending` means this full-range final pass has not yet reviewed the file, even
when parts were reviewed during prior implementation work. Generated files
must be checked against their generator/schema rather than merely accepted.

| File | Review status |
|---|---|
| `.github/workflows/api-surface.yml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `.github/workflows/ci.yml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `.github/workflows/t1-bun.yml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `.github/workflows/tui-typescript.yml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `AGENTS.md` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `ARCHITECTURE.md` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `Cargo.lock` | dependency diff inspected; only obsolete trybuild and its unused transitive dependencies removed in corrective closure |
| `PRE-V5-RUST-LOWERING-HANDOFF.md` | full re-read; requirements matrix above; implementation closure pending |
| `biome.jsonc` | baseline diff read; generated state transport joins the generated-file exclusion |
| `crates/iyon-tui-native/Cargo.toml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/include/iyon_view_abi.h` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/content_ffi.rs` | full implementation/baseline diff and F3 direct-FFI test read; successful acceptance preserves revision/drain hint; native evidence recorded |
| `crates/iyon-tui-native/src/generated/view_abi_conformance.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/generated/view_abi_exports.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/generated/view_abi_napi.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/generated/view_abi_table.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/generated/view_abi_types.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/generated/view_state_schema.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/sync.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/tui.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui-native/src/tui/theme_dto.rs` | full decoder/tests and removed decoder baseline comparison read; selector ordering preserved; F17 RGB fix reviewed |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | full baseline diff, native ingress/lease/cache code and F17 correction read; generated drift check reported passing |
| `crates/iyon-tui-native/src/tui/view_state.rs` | full decoder/tests and F8 correction read; unknown bits and invalid nested values rejected before install; absent/null lanes ignored |
| `crates/iyon-tui-native/tests/generated_view_abi.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/Cargo.toml` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/app.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/content.rs` | Source, parser, projection, receipt and lifecycle paths personally inspected; F1–F5/F7/F9 correction diffs reviewed; focused validation recorded |
| `crates/iyon-tui/src/application/context.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/environment.rs` | full implementation and F3 correction read; accepted mutations retain per-host error reporting; focused parent wake tests pass |
| `crates/iyon-tui/src/application/host.rs` | frame/receipt/state/lifecycle implementation and all correction diffs read; F11/F12 retry and bootstrap fixes reviewed; retained-scene regression additions read |
| `crates/iyon-tui/src/application/kernel.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/run.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/source_store.rs` | full implementation/tests and baseline comparison read; F1/F2/F4/F5 fixes personally reviewed; focused native evidence recorded |
| `crates/iyon-tui/src/application/tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/application/view_state.rs` | full implementation read; host serialization, captured-version lifecycle and F13/F15 integration reviewed |
| `crates/iyon-tui/src/binding/mod.rs` | final private-owner re-exports personally inspected; unchanged 124-export gate independently passes |
| `crates/iyon-tui/src/component/id.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/component/slot.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/component/tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/component/tick_tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/diff/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/diff/model.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/diff/render.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/render.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/annotations.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/ansi.rs` | full implementation/tests and baseline changes read; F9/F10 callers reviewed; final-addon ANSI regressions pass |
| `crates/iyon-tui/src/content/text/block.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/content.rs` | full baseline diff read; page-backed RawText ownership; bounded source slices |
| `crates/iyon-tui/src/content/text/diff.rs` | full implementation/tests read; F9/F10 Source pipeline fixes reviewed; replacement and incremental equivalence evidence recorded |
| `crates/iyon-tui/src/content/text/inline.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/markdown.rs` | full changed implementation and retained parse/cache/reference paths read; F9/F10 corrections reviewed; unchanged grammar covered by migrated tests |
| `crates/iyon-tui/src/content/text/markdown_options.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/origin.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/plain.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/provenance.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/block.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/identity.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/inline.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/mod.rs` | full implementation, sibling cache audit and F7 owner-pinning fix reviewed; parent mounted replacement regression passes |
| `crates/iyon-tui/src/content/text/render/policy.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/structured.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/render/tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/content/text/source.rs` | full original implementation and F10 correction diff read; lazy piece-backed assembly follows suffix selection; focused test evidence recorded |
| `crates/iyon-tui/src/content/text/style.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/controls/text_input/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/controls/text_input/output.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/controls/text_input/presentation.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/controls/text_input/tests/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/controls/text_input/tests/presentation.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/id.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/layout.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/model.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/native/frontier.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/native/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/history/projection/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/interaction/key.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/interaction/tests/mod.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/lib.rs` | final private root inspected; F6 closed with explicit hidden feature-gated benchmark exception |
| `crates/iyon-tui/src/output/handle.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/output/router.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/perf.rs` | counter enum/count/name additions read; no timing qualification inferred |
| `crates/iyon-tui/src/perf_bench.rs` | baseline factory-migration diff read; hidden feature-gated tooling exception documented; workloads not rerun |
| `crates/iyon-tui/src/physical/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/physical/row.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/physical/surface.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/physical/tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/api/composition.rs` | removed closure-builder implementation and authoring tests inspected; canonical private factory remains |
| `crates/iyon-tui/src/presentation/api/grid.rs` | full baseline diff and placement tests read; direct factory preserves placement semantics |
| `crates/iyon-tui/src/presentation/api/mod.rs` | baseline export/visibility diff read; actual public boundary narrowed in F6 |
| `crates/iyon-tui/src/presentation/api/style.rs` | full baseline diff read; sparse attribute vocabulary and shared semantic key ownership inspected |
| `crates/iyon-tui/src/presentation/api/text.rs` | full file/tests read; page ownership/range validation consistent with native bounded ingress |
| `crates/iyon-tui/src/presentation/api/view.rs` | full file read; attachment-preserving parts and checked text patch |
| `crates/iyon-tui/src/presentation/content.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/factory.rs` | full implementation read; final construction/precedence and migrated producers inspected |
| `crates/iyon-tui/src/presentation/ir.rs` | full implementation/tests read across chunks; final parts/flags, persistent sequences and batched edits; final-grid test currently compares same factory twice rather than independent placement oracle |
| `crates/iyon-tui/src/presentation/layout/cache.rs` | full implementation read; retained identity keys and F13 dependency invalidation consumers inspected |
| `crates/iyon-tui/src/presentation/layout/engine.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/layout/measure.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/layout/mod.rs` | full baseline diff read; direct row compilation and completeness propagation checked against retained painter tests |
| `crates/iyon-tui/src/presentation/layout/place.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/layout/tests/flow.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/presentation/layout/tests/grid.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/presentation/layout/tests/mod.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/presentation/layout/tests/style.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/presentation/layout/tests/text.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/presentation/layout/tree.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/mod.rs` | full binding-wrapper/export diff read; constructors delegate to canonical owners |
| `crates/iyon-tui/src/presentation/paint/decoration.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/paint/mod.rs` | full baseline diff read; paint/text-cache exports only |
| `crates/iyon-tui/src/presentation/paint/text.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/paint/theme.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/paint/view.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/presentation/wrap.rs` | full baseline diff read; private factory migration retains caret assertions |
| `crates/iyon-tui/src/projection/smooth.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/projection/value.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/retained_state/capabilities.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/retained_state/capture.rs` | full implementation/tests read; host-serialized capture and retained scene/frame ownership inspected |
| `crates/iyon-tui/src/retained_state/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/retained_state/presentation.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/retained_state/registry.rs` | full implementation/tests read; version table naming is not proof of visible-frame mutation; captured frame owners and F13/F15 consumers inspected |
| `crates/iyon-tui/src/scene/host.rs` | main retained-scene implementation and correction diffs inspected; F13–F15 geometry/theme/state capture regressions reviewed |
| `crates/iyon-tui/src/scene/resolve.rs` | full baseline diff read; demanded Arc versions and retained occurrence/reverse-path indexes inspected |
| `crates/iyon-tui/src/scene/resolved.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/scene/root.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/scene/root_tests.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/scene/tests.rs` | changed assertions/function inventory inspected; private-factory migration retains behavior; new row-paint cases personally inspected |
| `crates/iyon-tui/src/scroll.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/stream/coord.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/backend.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/mod.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/termwiz/backend.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/termwiz/presenter.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/termwiz/shadow.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/terminal/termwiz/worker.rs` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `crates/iyon-tui/src/testing.rs` | obsolete Rust-only authoring/test facade removed; callers migrated, native hooks retained, public TS testing surface unchanged |
| `crates/iyon-tui/src/theme/atoms.rs` | full implementation/tests read; optional bounded interning cache retains owners; poison fallback does not change semantic values |
| `crates/iyon-tui/src/theme/batch.rs` | full file/tests read; one final sort, duplicate-selector newest-order parity |
| `crates/iyon-tui/src/theme/mod.rs` | full baseline diff and resolver surrounding code read; batched variant order matches sequential setter semantics |
| `crates/iyon-tui/tests/diff_public.rs` | removed obsolete external authoring contract inspected; retained runtime behavior remains in internal/native tests |
| `crates/iyon-tui/tests/history_public.rs` | removed obsolete external authoring contract inspected; retained runtime behavior remains in internal/native tests |
| `crates/iyon-tui/tests/markdown_composition.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/markdown_hardening.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/markdown_incremental.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/markdown_smoke.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/p3c_ergonomics.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/public_semantic_api.rs` | removed obsolete external authoring contract inspected; retained runtime behavior remains in internal/native tests |
| `crates/iyon-tui/tests/root_compile_contract.rs` | removed obsolete external authoring contract inspected; retained runtime behavior remains in internal/native tests |
| `crates/iyon-tui/tests/scene_public.rs` | removed obsolete external authoring contract inspected; retained runtime behavior remains in internal/native tests |
| `crates/iyon-tui/tests/text_origin.rs` | migrated to private owning module; parent verified byte-identical corrective move and reviewed original assertion/constructor changes |
| `crates/iyon-tui/tests/ui/fail/history_not_column_child.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/history_not_into_view.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/nested_scene.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/removed_authoring_surface.rs` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/removed_authoring_surface.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/removed_renderer_surface.rs` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/removed_renderer_surface.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/root_block_kind.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/root_text_visitor.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/scene_not_into_view.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/fail/scene_requires_body.stderr` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/pass/history.rs` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/pass/p3c_api.rs` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `crates/iyon-tui/tests/ui/pass/scene.rs` | obsolete Rust authoring compile-contract fixture removed; root source gate now enforces the private boundary |
| `docs/history/perf/PERF-11-generated-abi-reference.md` | generated reference; canonical generator drift check passes, not independent performance evidence |
| `package.json` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/bench/generated/view_abi_cases.ts` | benchmark/generated fixture retained; performance execution and qualification remain paused |
| `packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts` | benchmark/generated fixture retained; performance execution and qualification remain paused |
| `packages/iyon-tui/bench/pre-v5-l1-trace.ts` | benchmark/generated fixture retained; performance execution and qualification remain paused |
| `packages/iyon-tui/src/api/view/retained-state.ts` | full baseline diff read; normalization retained, clear-all/empty distinction and scalar wake preserved |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts` | generated output verified through reviewed schema/generator and passing final drift check |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json` | generated output verified through reviewed schema/generator and passing final drift check |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/src/transport/native/addon.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/src/transport/state/control.ts` | full file read; public validation retained before generated packers |
| `packages/iyon-tui/src/transport/state/generated/state_envelope.ts` | generated output verified through reviewed schema/generator and passing final drift check |
| `packages/iyon-tui/tests/generated/view_abi_layout.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_ansi_scanner.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_history_prefix.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_perf13_d.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_smooth_delivery.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_state_envelope.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `packages/iyon-tui/tests/tui_text_lanes.test.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `reports/pre-v5-l1/L1-00-baseline.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-00-closure.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-00b-tranche-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-00c-tranche-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-00d-fix-note.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-01-ledger.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-01-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-02-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-03-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-04-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-05-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-06-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-07-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-08-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-09-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-10-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-11-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-12-report.md` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/L1-13-checkpoint.md` | full checkpoint re-read; historical evidence only, explicitly not final §19–21 acceptance |
| `reports/pre-v5-l1/perf13-h-content.json` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/t15-route-smoke.json` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/trace-default.json` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/trace-fixed.json` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `reports/pre-v5-l1/trace-perf-counters.json` | historical tranche/measurement record preserved; not rerun or treated as final acceptance evidence |
| `tools/api-surface/check-binding.ts` | full original check and final root-closure correction read; parent independently reran 124-export gate |
| `tools/api-surface/mappings/iyon-tui.toml` | binding-only metadata reviewed; four stale associated-method entries removed; final 81-record ownership check passes |
| `tools/ownership/check.ts` | baseline changes and relevant surrounding code personally reviewed; correction-specific evidence is summarized above |
| `tools/ownership/snapshots/iyon-tui-rust-surface.txt` | full final inventory read; 81 owner records match mapping after parent-requested metadata correction |
| `tools/tui-abi-gen/src/main.rs` | full baseline and F8 validator-test changes read; generator tests/drift check reported passing |
| `tools/tui-abi-gen/src/model.rs` | full baseline diff read; bounded typed state-property schema |
| `tools/tui-abi-gen/src/render_manifest.rs` | full baseline diff read; state schema included in metadata and generator fingerprint |
| `tools/tui-abi-gen/src/render_state.rs` | full generator read; generated masks and exact lane shapes checked; drift check reported passing |
| `tools/tui-abi-gen/src/snapshots/tui_abi_gen__tests__canonical_schema_renders_all_tranche_one_outputs.snap` | generated output verified through reviewed schema/generator and passing final drift check |
| `tools/tui-abi-gen/src/validate.rs` | full baseline and F8 exact-value-kind lane-shape correction read; generator tests reported passing |
| `tools/tui-abi/view_abi.toml` | full baseline diff read; all 17 state property lane shapes match current readers/packers |
