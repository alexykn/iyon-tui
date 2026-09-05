# L1-04 completion report: persistent axis/grid and remaining node kinds

Date: 2026-09-05. Base: `e0348bb` (L1-03 committed).

## What changed

**One shared grid placement algorithm (step 3).** `lower_grid` in
`presentation/api/grid.rs` is split: the placement/normalization body moves
verbatim into `lower_grid_parts(columns, gaps, rows)` taking moved values,
and the public closure builder delegates to it. New `View::grid_from_parts`
(crate-internal) and `View::native_grid_final` (native-host seam) construct
exactly one root from parsed rows. The public `View::grid`/`Grid`/`GridRow`
API is unchanged and still used by tests and external authors.

**No more clone replay on the grid lane (steps 1, 9).**
`parse_and_build_grid` in `view_abi.rs` keeps its two-phase
validate-then-build shape and every `FAST_*` code, but the tail
`View::grid(|..| row.cell_with(*spec, view.clone()))` replay is replaced by
`View::native_grid_final(moved columns, gaps, rows)`: no second collection,
no per-cell clone. This was the only closure-based builder replay left in
native materializers (verified by grep: all remaining `View::vertical` /
`horizontal` / `grid` uses are test-only). Axis lanes already moved
(`resolve_axis_children`, `native_axis_from_children`/`splice` from L1-02);
staged `AxisBuilder` limits/cleanup and axis set/splice/grid set-cell
`PersistentSeq` sharing are untouched (step 4).

**One root per changed trie parent (step 5).** New
`View::try_replace_retained_children(&[(step, child)])` in `ir.rs`:
validates every step in order with byte-identical errors to sequential
single replaces, chains `PersistentSeq::set` for sequence parents (path
copies, never flat copies; one `map_node` at the end), applies all hanging
slots / last-wins single-child writes into one root. The existing
`try_replace_retained_child` delegates to it (single-element batch), so the
path-publish recursion and all other callers keep exact behavior.
`stage_edit_trie` stages every changed child first, then applies one batch
per parent; staged-entry order and transaction semantics are unchanged.

**Producers onto the direct factory (steps 6–8).** New crate-internal
`View::row_from_views` / `View::column_from_views` (content tracks, matching
`Horizontal`/`Vertical` defaults). Migrated, each a mechanical
closure-pushes-to-vec-moves change with identical gaps/tracks/policies:
`content/diff/render.rs` (hunk + multi-hunk columns; numeric
hunk/line/termination metadata untouched), `content/text/render/mod.rs`
(list-aware gap policy untouched), `block.rs` (`render_blocks_with_gap`,
list items, task+list row, code-label column, table grid via
`grid_from_parts` with content source-row tracks, captioned-table column),
`controls/text_input/presentation.rs` (body + empty columns),
`history/projection` (root column). Hanging/Container/ClampRows/RowViewport/
Spacer/ComponentSlot/ContentHost constructions were already single-root on
every native and producer path (`View::hanging`, `wrap_structural`,
`row_viewport*`, `View::spacer`, `View::native_component`,
`View::native_content_host`); verified, not rewritten.

**Tests.** Core: `final_grid_assembly_equals_closure_builder_with_one_root`
(`semantic_eq` vs the closure builder + exactly 1 `ViewNodesConstructedRust`
under the perf lock) and `batched_retained_replace_equals_sequential_singles`
(batch `semantic_eq` sequential, new-root allocated, out-of-range error
parity both ways, empty batch returns the same root). Native:
`grid_malformed_tail_publishes_nothing_and_leaves_no_lease` (truncated tail
→ `FAST_INVALID`, NodeId stays unpublished, later valid buffer publishes and
resolves). TS: existing wide-host suites (axis replace/insert/remove,
64×2 grid set-cell, transaction paths) pass unchanged on the restaged addon;
the grid set-cell case exercises `native_grid_final` end to end, and
incremental edits render identical rows to fresh publication.

**Seam accounting.** No new public API and no new blessed binding type
(methods only). Mapping +2 `InternalBinding` records (1417 total) with
snapshot regen; `check:tui-binding`, ownership green.

## Kind/constructor coverage table (step 8)

| Kind | Direct factory | Native ingress (single root) | Built-in producers |
|---|---|---|---|
| Text | `native_text_final` (L1-03); `Text` fluent pre-root | utf8/cstring/buffer lanes | diff leaves, text render, text_input (unchanged) |
| Row | `native_axis_from_children` (L1-02); `row_from_views` (new) | axis builder/set/splice | task+list markers (migrated) |
| Column | `native_axis_from_children`; `column_from_views` (new) | axis builder/set/splice | diff, text render, text_input, history (migrated) |
| Grid | `native_grid_final` (new seam); `grid_from_parts` (new) | `grid_create_buffer` (moved, no clone) | `render_table` (migrated) |
| Hanging | `View::hanging` | `hanging_create` | diff lines, quote/list items (already direct) |
| Container | `.container()` via `from_node` | `container_create` | table cells, code blocks (already direct) |
| ClampRows | `.clamp_rows()` via `from_node` | `clamp_create` | none (native only) |
| RowViewport | `row_viewport*` via `from_node` | none | text_input, history (already direct) |
| Spacer | `View::spacer` | `spacer_create` | history, list items (already direct) |
| ComponentSlot | `View::native_component` | `component_create` | none (native only) |
| ContentHost | `View::native_content_host` | `content_host_create` | none (native only) |
| Diff | `DiffRenderer` (now on `column_from_views`) | `diff_create_buffer` | — (the renderer is the producer) |

## Stop condition

TS grids/axes/diffs/histories render through the retained path (full bun
suite 98 pass); incremental axis/grid edits equal fresh publication
(persistent-seq + transaction suites); wide-host edits stay structural
(`PersistentSeq` set/splice, no flattening); malformed grid tails publish
nothing and leave the NodeId reusable (new native test).

## Verification (this session)

- Baselines before coding: core 580, native 43, TS builder+persistent-seq 4.
- `cargo test --workspace --all-features`: exit 0, 23 binaries ok
  (core 582 incl. 2 new; native 44 incl. 1 new).
- `bun test packages/iyon-tui/tests/` on the restaged addon: 98 pass,
  0 fail (incl. lane-equivalence + wide parity + transaction suites).
- `cargo fmt --check` clean, `clippy-gate` exit 0, binding checks pass,
  `check:ownership` pass.
- One self-caught incident, no product impact: the first version of the new
  grid test built its leaves inside the perf-counter window (3 vs 1) and its
  failure poisoned the shared perf `test_lock`, failing 5 layout-cache tests
  as collateral. Fixed by building fixtures before reset; suite stable at
  582 ×3 runs. `map_node` deliberately bypasses `ViewNodesConstructedRust`,
  so the batch test pins equality/error-parity deterministically instead of
  counter values (process-global `ViewId` deltas would be racy).
