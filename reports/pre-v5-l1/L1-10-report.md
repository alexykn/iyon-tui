# L1-10 completion report: Delivery state without per-tick reprocessing

Date: 2026-09-05. Base: `4706fa7` (L1-09 committed).

## What changed

**1. Deterministic delivery characterization and indexing (`ConnectorDelivery`) (§12.1, §12.2).**
- Implemented `ConnectorDelivery` in `crates/iyon-tui/src/application/content.rs` to separate input acceptance (`accept_input`) from clock advancement (`advance`).
- Input indexing is executed strictly when source generation, revision, or sealed state changes (`indexed_generation`, `indexed_revision`, `indexed_sealed`).
- Grapheme segmentation projection is cached in `ConnectorDelivery.units` and binary-searched on ticks (`partition_point`) via `reveal_units()` without resegmenting source UTF-8 bytes.
- Preserved existing `SmoothConfig` rate calculation, spring dynamics, minimum/maximum characters per second, first-publication behavior, and seal completion.

**2. Prepared paint product and glyph-safe window reveal (`VisibilityIndex`) (§12.3).**
- Introduced `VisibilityIndex` to precalculate row-glyph boundary indices (`row_glyphs: Vec<Vec<u16>>`) and total painted glyph counts from the unmasked semantic surface.
- Implemented `VisibilityIndex::apply_reveal`:
  - Directly allocates only the target revealed height instead of cloning the entire surface.
  - Copies only revealed cells up to the exact cut column without full-surface duplication.
  - Completely eliminates the previous `reveal_surface` full `Surface.clone()`, cutting tick-time surface clones (`ContentSurfaceClones`) to zero.
- Cached unmasked rendered surfaces and their visibility indices in `PreparedPaintCache` keyed by `(SemanticProjectionKey, theme_revision, width)`. Pure delivery ticks reuse the unmasked rendered surface and only run `apply_reveal`.

**3. Due-Connector scheduling without inactive scanning (`active_deadlines`) (§12.4).**
- Added `active_deadlines: HashMap<u64, Instant>` to `ContentHostRegistry`.
- `advance(&mut self, now: Instant)` now examines and iterates *only* due connectors whose deadlines satisfy `deadline <= now`, rather than scanning inactive connectors across the entire host registry.
- `next_wakeup(&self)` returns the minimum deadline directly from `active_deadlines` in O(D) where D is active due connectors, returning `None` immediately when idle.
- Added `sync_connector_deadline` to update or remove connector deadlines on mount, unmount, visibility change, text input arrival, or disposal.
- Connectors with immediate delivery (`ContentDelivery::Immediate`) never schedule timers or enter `active_deadlines`.

**4. Candidate delivery frontier separation and rollback (§12.3).**
- Separated `candidate_delivery_frontier` from `committed_delivery_frontier` on `ConnectorRecord`.
- Delivery clock advancement (`advance`) updates only `candidate_delivery_frontier` and increments `delivery_revision`.
- Readback via `connector_delivery_frontier()` / `HostContentConnector::visible_delivery_frontier()` exposes only `committed_delivery_frontier`, guaranteeing that readback never claims a frontier that was not successfully presented to the terminal.
- `promote_candidate_projection` commits candidate progress (`committed_delivery_frontier = candidate_delivery_frontier`).
- `clear_candidate_projections` rolls back candidate progress (`candidate_delivery_frontier = committed_delivery_frontier`) when a frame is aborted or cancelled.

**5. Deleted tick-triggered semantic cache clears and grapheme rebuilds (§12.2, §12.3).**
- Removed `state.projection_cache.clear()` from `advance`. Pure ticks preserve compiled semantic projections and prepared paint products.
- Ticks perform zero JavaScript/TypeScript transport, zero parser/diff/markdown/ansi work, zero fresh content View lowering, and zero whole-surface copying.

## Stop-gate evidence

- **Trace parity and monotonicity:**
  - `application::content::tests::delivery_trace_parity_and_monotonicity`:
    - Deterministic clock stepping (16ms increments) produces strictly monotonic visible frontier progress.
    - Backlog units drain smoothly at the configured rates.
    - Sealed streams drain completely and cancel active deadlines (`next_wakeup() == None`).
- **Two independent Connectors on one Source:**
  - `application::content::tests::two_connectors_independent_delivery_on_same_source`:
    - A smooth connector and an immediate connector bind to the same shared Source.
    - Immediate connector projects full content instantly and schedules zero timer deadlines.
    - Smooth connector schedules deadlines and reveals units incrementally per tick without altering or being altered by the immediate connector.
- **Zero parser work and zero surface clones on ticks:**
  - `application::content::tests::native_ticks_perform_zero_parser_and_zero_surface_clones`:
    - Verified `SemanticProjectionRebuilds` remains identical across delivery ticks (0 parser work).
    - Verified `ContentSurfaceClones` remains identical across delivery ticks (0 full-surface copies via `VisibilityIndex`).
- **Visible-frontier commit separated from candidate progress:**
  - `application::content::tests::visible_frontier_commit_separated_from_execution_progress`:
    - Advancing clock advances `candidate_delivery_frontier` while `committed_delivery_frontier` remains pinned to the presented frame.
    - Candidate abortion (`clear_candidate_projections`) restores candidate to committed frontier.
    - Promotion (`promote_candidate_projection`) safely installs candidate as the new committed frontier.
- **Cold and disposed connectors clean up deadlines:**
  - `application::content::tests::cold_and_disposed_connectors_clean_up_deadlines`:
    - Setting connector invisible removes it from `active_deadlines` (`next_wakeup() == None`).
    - Remounting restores the pending deadline.
    - Disposing the connector permanently purges its deadline.

## Verification results

- `cargo test --workspace`: all workspace tests pass (50 unit tests, 6 doc tests, 6 compile-fail tests, 10 generator tests, 5 view ABI tests, 5 origin tests, 1 sync test).
- `cargo test --features native-host,perf-counters --lib -p iyon-tui`: 626 passed, 0 failed, 1 ignored.
- `bun run check:ownership`: ALL OWNERSHIP CHECKS PASSED.
- `bun test`: 113 passed, 0 failed across 32 files.
- Clippy & cargo check clean.
