# J-Space Workspace Ledger

## Goal
Reduce the repository’s routine full-test checkpoint below 600 seconds without hiding ordinary regressions; disable only explicitly non-routine case/stress tests after profiling.

## Core
- Routine checkpoint — preserve ordinary unit/integration coverage; only opt out tests proven to be characterization, stress, or long-running case tests.
- PERF-4 scope — retain History geometry for static/live units and incrementally integrate stream geometry without visual regressions.
- PERF gate — processing the next fixed 256-byte append after warmup must remain approximately flat as accumulated source grows.
- Source coordinates — every retained stream update must use source-rooted offsets at or after the resident prefix.
- PERF-5 scope — implement exactly the handoff requirements while preserving repository boundaries and existing behavior.

## Verified
- ✓01 Read the handoff in full through line 3473. PERF-5 requires: promotable stable TextStream nodes with compaction; retained row anchors; separate semantic and visual restart offsets with safe hard-line restart; suffix-only reflow; generic host chunk/offset storage without full projection replay or per-character strings; no assistant/thinking semantics in generic TUI; generic TypeScript stream API; full checkpoint and performance/counter proof. — verified by: read tool output covering offsets 1–3473; coverage: complete handoff, PERF-5 section, checklist, final mechanical gate, and operating rules
- ✓02 Reproduced the markdown-triggered compaction regression with character-by-character `# heading`/paragraph/list/code input; removing `pipeline.reset()` during source compaction preserves Smooth's published suffix and the regression test now passes. Full iyon-tui native-host lib suite: 718 passed, 1 ignored. Native crate: 8 passed. TypeScript check, release build, and 10 focused Bun TUI tests pass. — verified by: cargo test -p iyon-tui --features native-host --lib; cargo test -p iyon-native --lib; bun run typecheck; bun run build:iyon; focused Bun test command. Coverage: source compaction, Markdown incremental path, native snapshot, runtime handles/realtime/harness. — closes: ?01
- ✓03 Completed post-fix release benchmark. Layout median for next 256B: 1024B 757,541 ns; 10,240B 527,791 ns; 51,200B 521,875 ns; 102,400B 521,792 ns; 512,000B 559,209 ns. At 512KiB, 5 samples p95=591,833 ns; counters show 35 rows reindexed and 60,210 stable rows reused, semantic/visual restart=514,304. Raw append median at 512KiB=1,750 ns. Replaced the linear retained-row `take_while` with `partition_point`, removed legacy `reindex_from`, and reran production reindex tests. — verified by: `/tmp/iyon-tui-perf5-postfix-curve.jsonl` and `/tmp/iyon-tui-perf5-512-postfix.jsonl`; coverage: 1KiB–512KiB release curve, 5 samples per point, fixed 256B append, incremental render/index counters
- ✓04 Completed post-fix release benchmark. Layout median for next 256B: 1024B 757,541 ns; 10,240B 527,791 ns; 51,200B 521,875 ns; 102,400B 521,792 ns; 512,000B 559,209 ns. At 512KiB, 5 samples p95=591,833 ns; counters show 35 rows reindexed and 60,210 stable rows reused, semantic/visual restart=514,304. Raw append median at 512KiB=1,750 ns. Replaced the linear retained-row `take_while` with `partition_point`, removed legacy `reindex_from`, and reran production reindex tests. — verified by: `/tmp/iyon-tui-perf5-postfix-curve.jsonl` and `/tmp/iyon-tui-perf5-512-postfix.jsonl`; coverage: 1KiB–512KiB release curve, 5 samples per point, fixed 256B append, incremental render/index counters

## Open
- ?02 Is the final post-fix benchmark flat across all required target sizes? — settled by: Completed release curve JSONL with 1KiB, 10KiB, 50KiB, 100KiB, and 512KiB points plus 512KiB p95 evidence.

## Next
Profile the previously blocking api-surface cfg test and rerun the warmed full workspace test with package-level timing.
