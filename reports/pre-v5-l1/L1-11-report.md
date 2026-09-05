# L1-11 report

Tranche: L1-11 — cached terminal content model, row-window paint, and History adapter

Baseline SHA: b03405ee3855f5b922ee8b173fea92799b378f48 (the L1-11 starting commit)

Result SHA: the commit containing this report records the approved L1-11 result; resolve with `git log -1 --format=%H -- reports/pre-v5-l1/L1-11-report.md`.

Native artifact hash / target / profile / features:
- 77004ff819d0dd129344ba7b9f7fd41265f4be23ed514f4d19ba15d98f15bd86 — target/debug/libiyon_tui_native.dylib, macOS arm64, debug test profile, built by cargo test -p iyon-tui-native --all-features
- Rust test feature profile: native-host,test-util,perf-counters
- A staged/published addon was not produced by this child; no artifact was copied or committed.

Bun and Rust versions:
- Bun 1.4.0
- rustc 1.97.1 (8bab26f4f 2026-07-14)
- cargo 1.97.1 (c980f4866 2026-06-30)

Changed authoritative path:
- crates/iyon-tui/src/application/content.rs: carries the immutable finalized-prefix row product through HostContentProjection; History now consumes that product rather than slicing open semantic rows; finalized products carry a row-granular Smooth delivery compatibility; semantic cache keys distinguish open and sealed parser products; width-dependent LayoutTree products are reused across theme-only repaints; added full Markdown prefix/History differential and layout-reuse fixtures.
- crates/iyon-tui/src/content/text/render/mod.rs: finalized-prefix cached sequence shortening now truncates persistent roots instead of returning a stale longer tail.
- crates/iyon-tui/src/presentation/layout/mod.rs: ViewCompiler::compile, compile_tree, overlay, and bounded helpers use the row product path instead of allocating a content-sized physical Surface.
- crates/iyon-tui/src/presentation/paint/view.rs: one-row physical compositor uses the existing Surface glyph-safe composition, border/background helpers, text compiler, and ContentProvider::paint_window contract. It rejects disjoint rows before allocation; sorted child ranges require monotonic starts and ends for dual binary pruning; overlapping/non-monotonic children retain original order through a filtered fallback; viewport clips are transformed into local coordinates.
- crates/iyon-tui/src/presentation/paint/text.rs: compiled TextGeometryCache rows retain width-dependent wrapping/row geometry and span indexes; theme repaint resolves styles without rebuilding wrap products.
- crates/iyon-tui/src/presentation/layout/tests/mod.rs: row/surface differential, nested/non-zero viewport, tall-first/short-following horizontal/grid (including non-monotonic interval ends), wide border-label clipping, many-span/empty-span/combining-seam, and pruning-counter fixtures.
- crates/iyon-tui/src/content/text/render/tests.rs: stale-tail and Source-page retention fixtures.
- crates/iyon-tui/src/presentation/paint/decoration.rs: clipped border-edge work counter and glyph-safe wide-label fixture.
- All paths below are part of this delegated L1-11 tranche; formatting was applied to the complete delegated tranche set.

Deleted path/helpers:
- No public API was deleted in this child.
- The production ViewCompiler content compile route no longer returns a content-sized offscreen Surface; lower_surface remains only as a cfg(test) differential helper.
- The former open-row History slice is no longer authoritative; the finalized-prefix product is the sole source for transferable open/sealed content rows.
- No alternate Markdown/parser implementation was added.

Surviving passive types and why they are direct storage:
- LayoutBlock remains a private row-product container because History and content paint need immutable PhysicalRow storage.
- PhysicalRow and Surface remain terminal-correct physical storage/composition primitives; the row compositor allocates only one row-sized Surface at a time.
- LayoutTree remains the retained geometry product and is now held by theme-independent prepared paint entries.
- FinalizedPrefixProduct remains an immutable row product over the established Source-range finalized prefix so open Markdown prefixes can differ from the open document without copying Source bytes.
- PreparedProjectionTicket remains an internal identity witness for exact Connector/product selection.

Any temporary scaffold and its deletion tranche:
- No temporary source scaffold. target-* directories are untracked build artifacts and must not be staged.
- The row path is the L1-11 implementation; later optimization can narrow demand to visible rows, but L1-11 now forbids full content-sized physical Surface allocation.
- The old cfg(test) reveal helper is retained only for its historical copy-counter regression; it is not a production route.

Behavior fixtures exercised:
- cargo test -p iyon-tui --features native-host,test-util,perf-counters --lib: 648 passed, 0 failed, 1 ignored (independently rerun by parent after the final decoration fix).
- Full finalized-prefix differential: tilde/backtick fence, ordered list, setext heading, reference definition, GFM table, and open paragraph; every valid UTF-8 append prefix compares complete PhysicalRow cells/styles against a sealed baseline. Log: /tmp/iyon-lowering-verification-20260905/finalized-prefix-differential.log.
- Partial and zero History receipts compare sink rows with the finalized baseline and assert the committed row frontier does not move on a zero receipt.
- Smooth visible/committed frontier traces and the multirow sealed/open finalized-baseline + partial-receipt differential pass in /tmp/iyon-lowering-verification-20260905/smooth-history-differential.log and /tmp/iyon-lowering-verification-20260905/content-tests-final.log.
- Shortened semantic sequence drops stale cached tails and preserves unchanged child View identity. Log: /tmp/iyon-lowering-verification-20260905/semantic-shorten.log.
- Row/surface differential covers text, columns, horizontal rows, row viewport, decorated shells, tall-first/short-following horizontal and grid cases; nested non-zero-origin viewport content-host differential also passes. Logs: row-painter-tall-grid.log, nested-viewport-equivalence.log.
- Wide glyph, border/background/padding, wrapping, text-input, scroll, History and scene regression tests pass in the complete lib run.

Failure/rollback fixtures exercised:
- Exact prepared ticket identity cannot select a newer same-width projection.
- Candidate Source snapshot is shared within an attempt.
- Activation failure/rollback, source wake poisoning, lifecycle disposal, partial/zero sink receipts, and frozen remainder/resize fixtures pass.
- Sealed/open semantic cache aliasing is prevented by the sealed key component.

Ownership/lifetime evidence:
- bun run check:ownership: all ownership checks passed; log /tmp/iyon-lowering-verification-20260905/check-ownership-final.log.
- Finalized prefix rows retain immutable Arc<FinalizedPrefixProduct>; Source-backed semantic text retains page ownership rather than a bare pointer.
- Theme-only repaint retains the same Arc<LayoutTree> and TextGeometryCache while replacing theme-resolved PhysicalRows; the text-geometry build counter stays unchanged across recolor; log /tmp/iyon-lowering-verification-20260905/theme-layout-reuse.log.
- Native crate all-feature tests pass (50 unit + 5 generated ABI + 1 sync integration); log /tmp/iyon-lowering-verification-20260905/native-all-features.log.
- cargo test --workspace --all-features passes, including UI compile-fail fixtures and doctests; log /tmp/iyon-lowering-verification-20260905/cargo-workspace-all-features.log.

Route/copy/work counters:
- Row-pruning fixture builds a 4,096-child Column and paints a one-row bounded viewport: PaintNodesVisited=2, PaintCellsAllocated=13; this proves disjoint children are rejected before row-surface allocation. Log /tmp/iyon-lowering-verification-20260905/row-paint-prune-counter.log.
- Tall bordered-row fixture reports only the visible edge intersection (2 border candidates for a 20,001-row allocation); wide-label differential uses whole-glyph clipping. Log /tmp/iyon-lowering-verification-20260905/border-row-work.log and /tmp/iyon-lowering-verification-20260905/decoration-window-tests.log.
- Native Smooth tick fixture asserts no semantic parser rebuild and no whole-surface copy after the initial preparation.
- Theme repaint fixture asserts parser rebuild count stays unchanged, metric revision stays unchanged, paint revision changes, and the width-dependent LayoutTree Arc is shared.
- The current sequence-candidate implementation still scans a bounded per-Connector sequence cache and uses mutex-protected persistent-root lookups. These costs are disclosed rather than represented as zero work; future tuning should improve candidate indexing/lock granularity without changing semantics.
- No end-to-end timing/memory benchmark was claimed from unit tests. The 256-style-span + empty-span + combining-mark differential exercises geometry/style lookup and canonical row equivalence.

Focused timing and memory results, with raw report paths:
- Rust unit test wall time: approximately 0.54s for the final iyon-tui lib feature profile; raw log /tmp/iyon-lowering-verification-20260905/iyon-tui-lib-final.log.
- Workspace all-features test run completed successfully; raw log /tmp/iyon-lowering-verification-20260905/cargo-workspace-all-features.log.
- Native all-features test run completed successfully; raw log /tmp/iyon-lowering-verification-20260905/native-all-features.log.
- No benchmark p50/p95/p99 or peak-memory claim is made; the required longer benchmark/staging qualification remains a parent/CI responsibility.

Generation/public-declaration/ownership gates:
- cargo fmt --all -- --check: passed after formatting the full delegated tranche changed set.
- cargo test --workspace --all-features: passed.
- sh tools/lint/clippy-gate.sh: passed (warnings only); log /tmp/iyon-lowering-verification-20260905/clippy-final.log.
- bun run check:tui-abi: passed; log /tmp/iyon-lowering-verification-20260905/check-tui-abi.log.
- bun run check:ownership: passed.
- bun run typecheck: passed; log /tmp/iyon-lowering-verification-20260905/bun-typecheck.log.
- git diff --check: passed.
- No staging, commit, push, or generated-addon publication was performed.

Platform qualification:
- macOS arm64 Rust debug/all-feature tests passed.
- Same-image packaged addon staging, Node integration/smoke, and release-profile native artifact qualification were not run by this child.

Baseline failures reproduced, if any:
- None in the final Rust/workspace/ownership/ABI/typecheck runs.
- Earlier final-diff failures were fixed: stale semantic sequence tail, finalized rows using open rows, full Surface path, row-pruning assumptions, and non-zero viewport clip transform.
- cargo fmt --check initially failed because the L1-11 tranche set was unformatted; cargo fmt --all was then run and the check is now green.

New failures or deviations:
- No known behavioral failures after the final 648-test lib run and workspace all-feature run.
- The specialized row compositor is a second traversal shape but not a second semantic/terminal renderer: it delegates text flow to ViewCompiler, uses the same Surface glyph-safe composition and border/background functions, uses the same ContentProvider::paint_window ticket/window contract, and is different only in its one-row allocation target. The differential tests compare it against the existing full compositor.
- The row product still computes full width-dependent row indexing and stores all rows; theme-independent TextGeometryCache retains wrapping/row geometry while paint resolves current styles. Semantic span selection builds one cumulative range index per text leaf and advances a monotonic cursor, avoiding an all-spans scan per grapheme; geometry stores span indexes rather than copied StyleRef/StyleFacts. The theme fixture asserts the text-geometry build counter is unchanged across recolor, not only LayoutTree identity. L1-11 allows initial row indexing, while a later demand-driven residency tranche can reduce row-product materialization further.

Production LOC added/deleted:
- Current aggregate worktree diff for this delegated L1-11 tranche (implementation plus formatting): 3,907 added / 743 deleted lines across 33 files (the latest geometry-cache, Smooth, row-pruning, and grid fixtures are included in this count). Parent made no source edits during this delegated run; the aggregate is the exact diff from baseline SHA b03405e.
- Generated LOC added/deleted: no generated ABI schema/output change was made by this child.

Decision: accepted for the L1-11 tranche commit after parent actual-diff review and independent checks.

Parent verification and remaining qualification:
- Independently passed formatting, 648 library tests (one ignored), the Clippy gate (warnings remain), ownership, and `git diff --check` after reviewing the final source changes. Logs: `/tmp/iyon-lowering-takeover-20260905/parent-l11-lib-final.log`, `parent-l11-clippy.log`, and `parent-l11-ownership.log` in the same directory.
- Reviewed the final border intersection and glyph-safe label composition fixes; the delegate's decoration regressions are recorded in `/tmp/iyon-lowering-verification-20260905/decoration-window-tests.log`.
- End-to-end benchmark, packaged-addon, and full remaining handoff qualification are still required in L1-13. This tranche approval does not claim that L1-12, L1-13, or the complete handoff acceptance matrix has passed.

Changed files (exact current git diff --name-only set):
- crates/iyon-tui-native/src/tui.rs
- crates/iyon-tui-native/src/tui/view_abi.rs
- crates/iyon-tui/src/application/content.rs
- crates/iyon-tui/src/application/host.rs
- crates/iyon-tui/src/application/mod.rs
- crates/iyon-tui/src/application/source_store.rs
- crates/iyon-tui/src/binding/mod.rs
- crates/iyon-tui/src/content/diff/mod.rs
- crates/iyon-tui/src/content/diff/render.rs
- crates/iyon-tui/src/content/text/ansi.rs
- crates/iyon-tui/src/content/text/diff.rs
- crates/iyon-tui/src/content/text/markdown.rs
- crates/iyon-tui/src/content/text/render/block.rs
- crates/iyon-tui/src/content/text/render/identity.rs
- crates/iyon-tui/src/content/text/render/mod.rs
- crates/iyon-tui/src/content/text/render/tests.rs
- crates/iyon-tui/src/physical/row.rs
- crates/iyon-tui/src/physical/tests.rs
- crates/iyon-tui/src/presentation/api/text.rs
- crates/iyon-tui/src/presentation/api/view.rs
- crates/iyon-tui/src/presentation/content.rs
- crates/iyon-tui/src/presentation/ir.rs
- crates/iyon-tui/src/presentation/layout/measure.rs
- crates/iyon-tui/src/presentation/layout/mod.rs
- crates/iyon-tui/src/presentation/layout/place.rs
- crates/iyon-tui/src/presentation/layout/tests/mod.rs
- crates/iyon-tui/src/presentation/layout/tree.rs
- crates/iyon-tui/src/presentation/paint/decoration.rs
- crates/iyon-tui/src/presentation/paint/mod.rs
- crates/iyon-tui/src/presentation/paint/text.rs
- crates/iyon-tui/src/presentation/paint/view.rs
- crates/iyon-tui/src/scene/host.rs
- tools/api-surface/check-binding.ts
