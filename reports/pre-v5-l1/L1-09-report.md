# L1-09 completion report: Semantic execution and parser reuse

Date: 2026-09-05. Base: `d1541c1` (L1-08 committed).

## What changed

**1. Page-backed raw inputs (`RawText::from_page_slice`) (§10.1).**
- Updated `RawText` in `crates/iyon-tui/src/content/text/content.rs` to store `page: Arc<str>, start: u32, len: u32`.
- `RawText::text(&self)` slices the backing page without memory allocation.
- Replaced the `snapshot.chunks()` iteration in `source_projection` (which previously executed `TextContent::raw(text.to_owned())` on every frame) with `snapshot.chunk_views()` and `RawText::from_page_slice(view.page, view.page_start, view.len)`.

**2. Zero-copy semantic projection domains (`RawDomain`) (§10.2).**
- Upgraded `RawDomain` in `crates/iyon-tui/src/content/text/source.rs` to store `text: Arc<str>, sub_range: Range<usize>, pieces: Vec<RawPiece>`.
- Single-span inputs adopt the `Arc<str>` page directly without heap allocation.
- `RawDomain::suffix` and `prefix` now adjust `sub_range` on the shared `Arc<str>` page with zero byte copying.

**3. Retained Markdown working regions without String cloning (§10.3).**
- Refactored `CachedDomain` in `crates/iyon-tui/src/content/text/markdown.rs` to store immutable `Arc<[ProjectionSpan<TextContent>]>`, removing the redundant `prefix: String` and deep `Vec<CachedSpan>` cloning.
- `update_cache`, `cached_projection`, `prepend_cached`, and `stable_prefix_end` reuse existing spans directly.

**4. Incremental Diff projector (`DiffProjector`) (§10.4, §11).**
- Enhanced `DiffProjector` in `crates/iyon-tui/src/content/text/diff.rs` to maintain running parse state (`in_hunk`, `completed_inlines`, `completed_end`, `last_base`, `last_end`) across appends.
- On continuation, diff lines already completed are preserved; only the uncompleted trailing line and newly arrived bytes are parsed.

**5. Incremental safe ANSI projector (`AnsiProjector`) (§10.4, §11.5).**
- Enhanced `AnsiProjector` in `crates/iyon-tui/src/content/text/ansi.rs` to retain `AnsiState`, `completed_inlines`, and `completed_end` across stream appends.
- Incomplete escape sequences at chunk boundaries (e.g. partial CSI/OSC) are held in working state and completed when trailing bytes arrive, without leaking escape bytes into text runs.
- When the stream seals, incomplete escape sequences are safely dropped.
- Unsafe control sequences (cursor movement, screen clear, private modes) are consumed and never escape to the backend.

**6. Indexed annotation rewriting (`SourceAnnotationRewriter`) (§10.5).**
- Updated `SourceAnnotationRewriter` in `crates/iyon-tui/src/application/content.rs` to query `StoredSource::overlapping(start, end)` using the persistent interval index (Treap) in O(log N + K).
- Unaffected runs bypass splitting entirely; runs whose overlapping annotations span the entire run range are decorated directly without extra run partitions.

**7. Semantic cache key decoupling (`SemanticProjectionKey`) (§10.6).**
- Decoupled `SemanticProjectionKey` in `crates/iyon-tui/src/application/content.rs` from `width`, `wrap`, and `delivery_revision`.
- Viewport resizes, window reflows, and smooth animation timer ticks hit the semantic projection cache with 0 parser rebuilds.
- `ConnectorExecution` retains `diff: Option<DiffProjector>` and `ansi: Option<AnsiProjector>` alongside `markdown` and `smoother`.

## Stop-gate evidence

- **Zero parser work on theme/resize/tick:**
  - `application::content::tests::theme_recolor_repaints_without_reparsing_semantic_content`:
    - Verified `SemanticProjectionRebuilds` is incremented on initial parse.
    - Verified palette-only theme recolor leaves `SemanticProjectionRebuilds` unchanged.
    - Verified viewport width resize (20 -> 40 columns) leaves `SemanticProjectionRebuilds` unchanged.
    - Verified new source bytes trigger exactly one incremental semantic rebuild.
- **Incremental Diff equivalence & partial-line resumption:**
  - `content::text::diff::tests::incremental_diff_matches_one_shot`: Line-by-line streaming through varied hunks produces identical semantic IR to one-shot execution.
  - `content::text::diff::tests::unsealed_partial_line_resumes_correctly`: Partial trailing lines in unsealed chunks resume and merge cleanly with subsequent appends.
- **Incremental ANSI equivalence & safe boundary handling:**
  - `content::text::ansi::tests::incremental_ansi_matches_one_shot`: Character-by-character streaming through styles, colors, and OSC 8 hyperlinks matches one-shot parsing.
  - `content::text::ansi::tests::incomplete_escape_at_boundary_held_and_completed`: Incomplete CSI sequence split across appends is held in working state without leaking, then completes correctly.
  - `content::text::ansi::tests::incomplete_escape_at_stream_seal_is_dropped_safely`: Trailing incomplete escape sequence at stream seal is discarded safely.
  - `content::text::ansi::tests::unsafe_control_sequences_never_leak`: Cursor positioning, screen clearing, and private modes are stripped and never leak to the backend.

## Verification results

- `cargo test --workspace`: 23 test suites pass, 770 passed, 0 failed, 2 ignored.
- `cargo test --features native-host,perf-counters --lib -p iyon-tui`: 622 passed, 0 failed.
- `bun run check:ownership`: ALL OWNERSHIP CHECKS PASSED.
- `bun test`: 113 passed, 0 failed across 32 files.
- `cargo clippy --workspace --all-targets`: 0 errors.
