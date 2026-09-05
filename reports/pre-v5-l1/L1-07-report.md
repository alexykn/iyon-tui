# L1-07 completion report: semantic style/theme normalization

Date: 2026-09-05. Base: `ed5f45d` (L1-06 committed).

## What changed

**One sparse-attribute vocabulary (steps 1, 8).** Deleted the duplicate
`ViewStateTextAttributes` record; the state plane now uses the canonical
`TextAttributeSpec` everywhere (`apply_to` moved onto it, `overlay`
widened to `pub(crate)`, new shared `attribute_value` reader). Native
`read_text_attributes` builds through the public `attribute()` builder.
Gate bookkeeping swapped deliberately: `check-binding.ts` blessed list,
one `mappings/iyon-tui.toml` record, one
`iyon-tui-rust-surface.txt` line, plus a new record each for
`attribute_value`.

**Canonical atom table (steps 2, 7).** New `theme/atoms.rs`:
`StyleAtomTable` (bounded FIFO, generation-checked eviction) plus
`intern_style_atom` exposed over the binding seam. `ThemeKey` now holds
`Arc<str>`, so sharing a key is a refcount bump. All three hot
theme-string paths intern through it: state-envelope `color_spec_str`,
structural `parse_color_atom`, and structural `StyleRef::themed` keys.
Live `Arc` values are immune to eviction by construction.

**Single-sort batch theme build (step 3).** New `theme/batch.rs`:
`ThemeBatch` accumulates entries with declaration orders and sorts each
entry exactly once at `finish`; duplicate selectors replace in place with
the newest order, matching the sequential setters bit-for-bit (differential
test, including mixed predicate counts and text styles). One new public
method, `Theme::assemble_batched`, is the single native entry point.

**Typed theme/border DTO (steps 2, 4, 8).** New native `tui/theme_dto.rs`
replaces the deleted `lower_theme`/`lower_style_spec`/`lower_selector`/
`lower_theme_color`/`lower_text_selector`/`lower_text_role`/
`lower_text_part`/`lower_border`/`color_spec` walks (~370 lines removed).
`serde` DTOs decode the exact accepted shapes — color strings,
`{type:"ansi",value}` objects, `{type:"default"}` theme colors, sparse
styles, all nine text-selector field families, TextInput borders — and
terminate directly in `assemble_batched`/`BorderSpec`. No second
theme-like model exists. A DTO-vs-sequential full-`Theme` equality test
pins duplicate handling, every selector field, and border shapes.

**Shared immutable theme (step 5).** `RunningApp.theme` and the content
registry theme are `Arc<Theme>`; `ContentProvider::set_theme` takes the
`Arc`, adopts it on change, and fast-paths on pointer equality before
value comparison (the per-frame full-map compare is gone). `AppCx` lends
through `Arc::make_mut` (copy-on-write; content re-syncs next prepare).

**Semantic/presentation revision split (step 6).** New
`SemanticProjectionKey` (full key minus theme, plus source range so
stable-prefix snapshots cannot collide) with a bounded per-connector
semantic cache. `project_text_snapshot` resolves full and prefix IR
through it; surfaces keep the theme-qualified key and repaint on recolor.
New `SemanticProjectionRebuilds` counter (name-read, additive-safe)
counts actual parser runs. `set_theme` keeps clearing surface products
only; hide/deactivate paths clear both caches.

## Stop-gate evidence

- Two-host parity: `identical_themes_resolve_identically_across_hosts`
  (same definition incl. duplicate variants → equal rows and per-cell
  styles on two hosts, themed `ansi:1` foreground resolved).
- No reparse on recolor:
  `theme_recolor_reuses_semantic_projection` (markdown funnel: first
  measure rebuilds, recolor leaves `SemanticProjectionRebuilds` flat while
  `projection_revision` still invalidates paint, source append rebuilds).
- No geometry change on presentation-only theme: existing
  `theme_changes_paint_without_changing_geometry` passes unchanged.
- Bounded interner: `unique_value_stream_stays_bounded_and_live_values_survive`
  (cap 8 over 100 unique keys, live `Arc` usable, re-intern equal).

## Gates (this session)

- `cargo test --workspace --all-features`: 23 suites ok, 749 passed,
  0 failed (lib 600 incl. 2 atoms + 2 batch + 1 recolor + 1 parity;
  native 53 incl. 3 DTO).
- `bun test packages/iyon-tui/tests/`: 103 pass, 0 fail (native rebuilt).
- `cargo fmt --check`: clean. `tools/lint/clippy-gate.sh`: 0 errors.
- `check:tui-abi` + `generate`: stable, no drift. `check:tui-binding`:
  pass. `check:tui-declarations`: pass. `lint:ts`, `typecheck`: clean.
- `check:ownership`: ALL PASS after two deliberate updates: the parity
  scan now covers `theme_dto.rs`, and `decode_border` was renamed
  `build_border_spec` (dead bridge-vocabulary collision).

## Deliberate decisions and notes

- Atom authority is a canonical process table, not `TuiEnvironment`-owned:
  threading env access through stateless N-API helpers and the structural
  runtime is surgery with no behavioral gain; one shared bounded authority
  satisfies §8.3. `serde` (workspace, derive) enabled for the native
  crate — new edge on an existing workspace dependency, not a new
  third-party crate.
- Malformed *containers* (non-object colors entry, non-array variants,
  wrong-typed selector fields) now fail closed instead of being silently
  ignored. No test or TS producer pins the old leniency; TS always emits
  well-typed definitions. Leaf accept/reject sets are unchanged.
- Pre-existing gap observed, not changed: `HostContentFunnel.hyperlinks`
  feeds the ANSI projector but is in neither projection key (its
  `fingerprint()` has no callers), so toggling it reuses stale IR. It
  predates this tranche and is orthogonal to the theme split; flagged for
  the projection-ticket work (L1-09/10).
- `SemanticProjectionKey` keeps width/wrap/delivery_revision (status-quo
  granularity, zero soundness risk); only theme moved domains.
- Overlay epoch from L1-06 still test-read only; L1-12 remains its
  consumer.
