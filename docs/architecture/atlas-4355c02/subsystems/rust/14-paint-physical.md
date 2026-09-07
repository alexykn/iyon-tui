# 14 — Paint / Physical Cells, Rows, Surfaces, Unicode Measurement, and Damage

## 0. Baseline, scope, and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope assignment: `14 / rust / paint-physical`
- Assignment scope: `crates/iyon-tui/src/presentation/paint/`, `crates/iyon-tui/src/physical/`, and `crates/iyon-tui/src/text.rs`
- Assignment goal: painting, cells, rows, surfaces, Unicode measurement, clipping, damage, and physical styles.

The report contract, atlas README, AGENTS.md, assignment manifest, and PRE-V5 architecture handoff were read for context. The PRE-V5 handoff was treated as an evidence and reporting requirement document only; this report inventories the current source and does not perform V5 migration/disposition analysis.

### Scope boundaries

Primary source inspection covered:

- `crates/iyon-tui/src/physical/mod.rs`
- `crates/iyon-tui/src/physical/glyph.rs`
- `crates/iyon-tui/src/physical/row.rs`
- `crates/iyon-tui/src/physical/style.rs`
- `crates/iyon-tui/src/physical/surface.rs`
- `crates/iyon-tui/src/physical/text_metrics.rs`
- `crates/iyon-tui/src/physical/tests.rs`
- `crates/iyon-tui/src/presentation/paint/mod.rs`
- `crates/iyon-tui/src/presentation/paint/decoration.rs`
- `crates/iyon-tui/src/presentation/paint/text.rs`
- `crates/iyon-tui/src/presentation/paint/theme.rs`
- `crates/iyon-tui/src/presentation/paint/view.rs`
- `crates/iyon-tui/src/text.rs`

Supporting seam inspection covered the relevant symbols in:

- `presentation/wrap.rs`
- `presentation/content.rs`
- `presentation/layout/mod.rs`
- `presentation/layout/measure.rs`
- `presentation/layout/place.rs`
- `presentation/layout/tree.rs`
- `scene/host.rs`
- `retained_state/damage.rs`
- `terminal/termwiz/lower.rs`
- `terminal/termwiz/presenter.rs`
- `terminal/termwiz/shadow.rs`
- `application/content.rs`
- `presentation/api/style.rs`
- `presentation/ir.rs`
- `perf.rs`
- `crates/iyon-tui/Cargo.toml`

### Evidence classifications

- **Current-source fact:** directly observed in the baseline source.
- **Static inference:** behavior inferred from source paths but not executed.
- **Unknown:** not established by the inspected source.
- **Observed execution:** none. No build, test, benchmark, or running service was executed in this investigation.

A number of static hazards are called out below. They are not claims that a production failure was observed at runtime; they identify paths that do not enforce the same whole-glyph invariant as the canonical physical compositing helpers.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate production LOC | Approximate test LOC | Primary responsibility | Secondary responsibilities |
|---|---:|---:|---|---|
| `physical/mod.rs` | 23 | — | Private physical-layer module boundary | Re-exports cell, row, style, surface, and metric helpers |
| `physical/glyph.rs` | 275 | — | Whole-glyph traversal, validation, copying, clearing, and physical-column lookup | Wide-cell clipping/composition safety; corruption diagnostics |
| `physical/row.rs` | 117 | — | Immutable physical row representation | Placement, clipping, occupied width, plain-text extraction, validation |
| `physical/style.rs` | 40 | — | Backend-neutral physical color/style vocabulary | ANSI color names and terminal attribute storage |
| `physical/surface.rs` | 258 | — | Mutable rectangular physical composition surface | Background fill, clipping, compositing, crop, glyph clearing, completeness tracking |
| `physical/text_metrics.rs` | 46 | 62 | Canonical EGC-to-terminal-cell measurement | Termwiz metric delegation and Unicode corpus tests |
| `physical/tests.rs` | — | 218 | Physical cell/row/surface behavior | Wide glyph property tests and corruption invariants |
| `presentation/paint/mod.rs` | 14 | — | Private semantic-paint module boundary | Re-exports painter, style resolver, decoration painter, and text geometry cache |
| `presentation/paint/decoration.rs` | 256 | 52 | Border and surface-decoration painting | Border labels, signed origins, physical clipping |
| `presentation/paint/text.rs` | 474 | — | Semantic text flow lowering into physical rows/cells | Text geometry retention, cursor marker painting, source metadata |
| `presentation/paint/theme.rs` | 223 | 344 | Semantic style cascade and theme resolution | Focus/state selectors, framework/application theme layering |
| `presentation/paint/view.rs` | 1,440 | 519 | Recursive full-tree and row-window physical painting | Paint cache, content-host windows, RowViewport, incremental subtree paint |
| `text.rs` | 8 | — | Root semantic-text facade | Re-exports `crate::content::text::*` |

The line counts above are source-line approximations based on the inspected file extents, with test modules counted separately where they are physically embedded in production files. They include comments and blank lines and are therefore maintenance-size indicators rather than compiler LOC.

Approximate totals for the assigned scope:

- Production/source lines: approximately **3,174**
- Embedded and dedicated test lines: approximately **1,195**
- The largest production mass is `presentation/paint/view.rs`, followed by text lowering and theme resolution.
- The physical layer is comparatively small, but it defines correctness invariants that every higher-level paint route depends on.

### 1.2 Responsibility split

The assigned scope is not a single “paint” layer. It contains four distinct responsibilities:

1. **Terminal-cell geometry**
   - `physical/text_metrics.rs`
   - `presentation/wrap.rs`
   - The source text is segmented into extended grapheme clusters and each cluster receives one canonical physical width.

2. **Physical storage and glyph-safe composition**
   - `physical/glyph.rs`
   - `physical/row.rs`
   - `physical/surface.rs`
   - These modules model physical cells, immutable rows, mutable surfaces, wide-glyph continuation cells, clipping, and composition.

3. **Semantic-to-physical lowering**
   - `presentation/paint/text.rs`
   - `presentation/paint/decoration.rs`
   - `presentation/paint/view.rs`
   - These consume layout geometry and semantic `View` data, resolve styles, lower text, and compose child/content surfaces.

4. **Style and theme resolution**
   - `presentation/paint/theme.rs`
   - `physical/style.rs`
   - Semantic `StyleRef`, `StyleSpec`, selectors, state facts, and themes are resolved into `PhysicalStyle`; the terminal backend later converts that into Termwiz attributes.

`src/text.rs` itself does not implement text measurement or painting. It is a one-line facade that publicly re-exports the semantic text module from `content/text`.

### 1.3 Backend-neutrality is layered, not absolute

The `physical` module documentation describes the vocabulary as backend-neutral and states that it has no semantic-view, stream, theme, or terminal-backend knowledge (`physical/mod.rs :: module documentation`). That is true for most data structures, but the physical geometry layer delegates directly to Termwiz:

- `physical/text_metrics.rs :: grapheme_cell_width`
- `physical/text_metrics.rs :: text_cell_width`

The canonical width function is `termwiz::cell::grapheme_column_width`, and `Cargo.toml` directly depends on `termwiz`. Thus:

- `PhysicalStyle`, `PhysicalCell`, `PhysicalRow`, and `Surface` are structurally backend-independent.
- The canonical width policy is currently Termwiz-defined.
- Termwiz is not only a final lowering dependency; it participates in the framework’s layout/paint geometry contract.

This is consequential for any future backend abstraction: changing the backend or width policy changes wrapping, measurement, cursor positions, row geometry, glyph validation, and terminal emission behavior together.

---

## 2. Types, APIs, and contracts

### 2.1 Physical cell contract

`physical/surface.rs :: PhysicalCell`:

```rust
pub(crate) struct PhysicalCell {
    pub(crate) grapheme: Option<String>,
    pub(crate) style: PhysicalStyle,
    pub(crate) painted: bool,
    pub(crate) continuation: bool,
}
```

The four fields have separate semantics:

- `grapheme: Some(text)` identifies the leader’s text.
- `grapheme: None` may represent either a blank painted cell or a continuation cell.
- `painted` distinguishes transparent composition space from an actual output cell.
- `continuation` marks a physical cell occupied by the preceding wide glyph.

Constructors:

- `PhysicalCell::transparent()` creates an unpainted, non-continuation cell with default style.
- Test-only `PhysicalCell::blank(style)` creates an intentionally painted blank cell.

The transparent/painted-blank distinction is behaviorally important. `Surface::apply_surface_background` and `apply_surface_background_at` turn transparent cells into painted blank cells with a background. The terminal lowerer emits unpainted cells as default-style spaces, while painted blank cells carry their resolved background/style.

### 2.2 Physical style contract

`physical/style.rs :: PhysicalStyle` is private to the crate:

```rust
pub(crate) struct PhysicalStyle {
    pub(crate) foreground: Option<PhysicalColor>,
    pub(crate) background: Option<PhysicalColor>,
    pub(crate) bold: bool,
    pub(crate) dim: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) reversed: bool,
    pub(crate) strikethrough: bool,
}
```

`PhysicalColor` supports:

- `Default`
- named ANSI colors
- indexed ANSI colors
- RGB colors

The physical style is resolved, not authoring intent. Sparse semantic style is represented separately by `StyleSpec`, `StyleRef`, and `TextAttributeSpec` in `presentation/api/style.rs`.

All physical style types and functions are `pub(crate)`, not public authoring APIs.

### 2.3 Physical row contract

`physical/row.rs :: PhysicalRow` contains a private `Vec<PhysicalCell>` and is intended to be immutable after construction.

Important methods:

- `from_cells`
- `place`
- `placed`
- `clipped_after`
- `empty`
- `width`
- `occupied_width`
- `cells`
- `cell`
- `glyphs`
- `validate_cell_geometry`
- `plain_text`

`PhysicalRow::from_cells` performs a debug-only geometry assertion. It does not return a runtime `Result`; invalid geometry is treated as a programming error in trusted internal code.

`PhysicalRow::place(width, left)` creates a destination row filled with transparent cells and copies complete glyph spans only. The returned boolean indicates whether all painted source glyphs were copied. A wide glyph that would cross the destination boundary is omitted as a unit and makes the result incomplete.

`PhysicalRow::clipped_after(end)` preserves the row’s original width but clears every glyph whose span intersects the clipping boundary. This avoids retaining a wide-glyph leader without its continuation cells.

`plain_text` removes continuation cells and emits transparent/blank cells as spaces up through the final painted leader. It is therefore a diagnostic/logical representation, not a byte-for-byte reconstruction of the physical buffer.

### 2.4 Glyph API and invariant

`physical/glyph.rs` is the core correctness module for wide terminal glyphs.

`PhysicalGlyph` records:

- `start`: leader’s physical column in the source slice
- `width`: leader plus contiguous continuation cells
- `leader`: reference to the leader cell

`glyphs(cells)` walks a cell slice as whole glyph spans. Orphan continuations are skipped after a debug assertion.

`validate_cells` checks:

- no orphan continuation
- no zero-width stored grapheme
- complete leader span exists
- actual continuation count matches canonical `text_cell_width`
- continuation cells contain no grapheme
- continuation styles match the leader’s style
- continuation cells are painted

Validation is deliberately strict for rows containing stored graphemes. It is the invariant used before Termwiz lowering and in debug assertions after composition.

Whole-glyph operations:

- `copy_glyph`
- `place_glyphs`
- `clear_glyph_covering`
- `write_glyph_span`
- test-only `cell_x_of`

`write_glyph_span` first clears every destination glyph that intersects the destination span, then copies the complete source glyph. This mirrors the documented Termwiz behavior: writing over any cell occupied by a wide glyph must neutralize the entire obscuring glyph first.

### 2.5 Surface contract

`physical/surface.rs :: Surface`:

```rust
pub(crate) struct Surface {
    pub(crate) size: Size,
    pub(crate) cells: Vec<PhysicalCell>,
    pub(crate) physically_complete: bool,
}
```

The surface is a row-major `width * height` cell buffer.

`physically_complete` means that all painted source glyphs represented by the surface fit completely within the physical destination. It does **not** mean every cell is painted. Transparent cells are legal and common before background application.

Important methods:

- `new(width, height)`
- `width`, `height`
- `row_cells`, `row_cells_mut`
- `get`, `get_mut`
- `apply_surface_background`
- `composite`
- `composite_clipped`
- `crop_to`
- `clear_rect`
- `clear_rect_with_background`
- `clear_glyph_at`

`Surface::composite` is unsigned and uses whole-glyph writes. It propagates child incompleteness and marks the parent incomplete if painted child glyphs fall outside the parent’s bounds.

`Surface::composite_clipped` supports signed x/y offsets and an explicit clip rectangle. It copies only glyphs whose entire span lies inside both the clip and target surface. It propagates an already-incomplete child, but intentionally skips glyphs crossing the clip rather than marking incompleteness solely because the clip excludes them.

### 2.6 Canonical Unicode measurement

`physical/text_metrics.rs` defines the single canonical width policy:

```rust
pub(crate) fn grapheme_cell_width(grapheme: &str) -> usize
pub(crate) fn text_cell_width(text: &str) -> usize
```

`grapheme_cell_width`:

1. Returns zero for an empty string.
2. Debug-asserts that the input contains exactly one extended grapheme cluster.
3. Calls `termwiz::cell::grapheme_column_width`.

`text_cell_width` segments the entire string with `unicode_segmentation::UnicodeSegmentation::graphemes(true)` and sums the per-cluster metric.

The source explicitly distinguishes:

- Unicode grapheme identity, determined by UAX #29 segmentation.
- Terminal physical occupancy, determined by Termwiz’s grapheme-column metric.

The test corpus includes:

- ASCII
- spaces
- CJK
- combining marks
- variation selectors
- keycaps
- flags
- skin tones
- ZWJ sequences
- family emoji
- regional indicators
- rainbow flag sequences

The explicit rationale is to avoid width disagreement for keycaps, VS-16, ZWJ sequences, flags, and other clusters that simpler `unicode-width` logic can under-count.

### 2.7 Semantic text facade

`crates/iyon-tui/src/text.rs` is:

```rust
pub use crate::content::text::*;
```

It is a facade over semantic text APIs. It does not own:

- terminal geometry
- physical rows
- terminal styles
- History
- stream lifecycle
- application state

The physical and paint layers consume semantic text through `presentation::api::text::TextSpan`, `presentation::ir::TextView`, and `presentation::wrap::StyledGrapheme`.

### 2.8 Paint APIs

`presentation/paint/text.rs` extends `ViewCompiler` with:

- `paint_text`
- `compile_text_with_metadata`
- `compile_text_geometry`
- `paint_text_row`
- `paint_compiled_text_row`
- `paint_text_geometry_row`

It also defines:

- `CompiledTextRow`
- `TextGeometryGrapheme`
- `TextGeometryRow`
- `TextGeometry`
- `TextGeometryCache`
- `row_from_string`
- `row_from_graphemes`

`TextGeometry` intentionally retains grapheme text, width, source range, and semantic span index without resolving physical styles. This enables a theme-only repaint to reuse width-dependent wrapping and only rebuild `PhysicalStyle` cells.

### 2.9 View painter APIs

`presentation/paint/view.rs :: ViewPainter` exposes crate-private methods for:

- full-tree painting
- row-window painting
- cached painting
- content-provider painting
- component subtree painting
- non-component subtree painting
- row-range painting
- incremental content-host painting

The key methods are:

- `paint_tree`
- `paint_tree_rows_with_content`
- `paint_tree_rows_with_content_and_text_cache`
- `paint_tree_row_range_with_text_cache`
- `paint_tree_with_cache`
- `paint_tree_with_content`
- `paint_component_into`
- `paint_component_into_with_content`
- `paint_subtree_into`
- `paint_subtree_into_with_content`
- `paint_tree_with_style`

These are internal framework mechanics. The intentional authoring surface remains semantic `View` and style APIs, not `Surface`, `PhysicalRow`, or `ViewPainter`.

### 2.10 Theme and style contracts

`presentation/paint/theme.rs :: StyleContext` carries:

- inherited style states
- local style facts
- focused status
- focus-within status

`enter_node` overlays node states/facts and updates focus scope.

`for_descendant` clears local facts while preserving inherited states and focus scope.

`ThemeResolver` owns separate framework and application themes. `resolve_text_style` applies, in order:

1. framework named style
2. application named style
3. local style patch

`resolve_color` handles direct ANSI/named/RGB values and theme-token lookup. Theme-token lookup prefers application theme colors, then framework colors.

The style resolution output is a concrete `PhysicalStyle`. Terminal-specific conversion occurs later in `terminal/termwiz/lower.rs`.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency diagram

```text
semantic TextView / View tree
        |
        v
presentation::wrap
  - Unicode EGC segmentation
  - canonical stored grapheme widths
  - wrapping and cursor geometry
        |
        v
presentation::layout
  - measure
  - prepare
  - place
  - LayoutTree / LayoutNode
        |
        v
presentation::paint::ViewPainter
  - style cascade
  - text lowering
  - decoration
  - child/content composition
        |
        +----------------------+
        |                      |
        v                      v
physical::PhysicalRow       physical::Surface
physical::PhysicalCell      mutable row-major buffer
        |                      |
        +----------+-----------+
                   |
                   v
terminal::termwiz::lower
  - PhysicalStyle -> CellAttributes
  - PhysicalRow/Surface -> Termwiz Change sequences
                   |
                   v
Termwiz terminal surface / presenter
```

### 3.2 Reverse ownership edges

| Artifact | Created by | Retained by | Destroyed/replaced by |
|---|---|---|---|
| `PhysicalCell` | text lowering, background/decoration code, content provider | `PhysicalRow`, `Surface` | vector/surface replacement or row/surface clone drop |
| `PhysicalRow` | `row_from_graphemes`, `PhysicalRow::from_cells`, row lowering | `LayoutBlock`, content projections, History adapters, native scrollback interfaces | projection replacement, History receipt, cache/product eviction |
| `Surface` | `Surface::new`, `ViewPainter`, content provider targets | `PreparedSceneFrame`, `SceneHost::last_surface`, `PaintCache` `Arc` values, Termwiz desired/presented surfaces | frame replacement, cache generation discard, presenter resize |
| `TextGeometry` | `ViewCompiler::compile_text_geometry` | local compile cache or `PreparedPaintProduct`/prefix proof cache | cache/product replacement |
| `PhysicalStyle` | `ThemeResolver` and direct style constructors | each cell and paint cache key | cell/surface replacement |
| `PaintCache` | `SceneHost` or test caller | scene host across paint epochs | `clear`, theme change, scene host reset |
| `DamageRegion` | retained-state damage builder and scene host | `PreparedSceneFrame` | frame consumption/replacement |

### 3.3 Content-provider seam

`presentation/content.rs :: ContentProvider` is the main external seam around physical painting.

The provider:

- supplies projection revision and layout-input revision;
- returns intrinsic `ContentMeasurement`;
- paints a `ContentWindow` directly into a `Surface`;
- receives a `PreparedProjectionTicket`;
- may supply History rows.

The paint contract explicitly says the provider writes directly into a target surface clipped to a rectangle. This means the generic content boundary is physically coupled to `Surface`, `PhysicalStyle`, and physical row geometry even though the provider itself owns projection/content lifecycle.

The production implementation is `application/content.rs :: ContentHostRegistry`, whose `paint_window` delegates to `paint_window_direct`.

The contract is trusted rather than type-enforced: `ContentProvider::paint_window` can write arbitrary `PhysicalCell` values into the target. The production provider performs glyph-safe writes and validates rows in debug builds, but the trait itself cannot statically guarantee valid glyph geometry.

### 3.4 Scene host ownership

`scene/host.rs` owns the higher-level frame lifecycle and decides whether to:

- perform a full `ViewPainter.paint_tree_with_content` pass;
- reuse `last_surface`;
- repaint a History subtree;
- repaint state roots;
- repaint component roots;
- repaint content roots.

The scene host computes damage separately, then stores the resulting `Surface` and `DamageRegion` in `PreparedSceneFrame`.

The terminal presenter does not consume `DamageRegion` directly according to the inspected Termwiz paths. It receives the prepared surface and constructs desired changes/diffs through Termwiz’s in-memory surface model. This makes damage currently metadata for scene scheduling and future/backend consumers rather than the direct write region used by the Termwiz presenter.

### 3.5 Physical-lifetime diagram

```text
SceneHost
  |
  +--> retained LayoutTree
  |
  +--> PaintCache
  |       +--> current: HashMap<PaintKey, Arc<Surface>>
  |       +--> previous: HashMap<PaintKey, Arc<Surface>>
  |
  +--> last_surface: Surface
  |
  +--> PreparedSceneFrame
          +--> surface: Surface
          +--> damage: DamageRegion
          +--> optional History overlay
```

A cache hit avoids recursive painting and allocation for a subtree, but the cached `Arc<Surface>` is still composited into the new parent. Parent geometry and z-order are therefore not bypassed by caching.

---

## 4. Execution paths and state transitions

### 4.1 Text source to physical cells

The normal semantic text path is:

```text
TextView
  -> TextSpan values
  -> styled_hard_lines
  -> Unicode grapheme segmentation
  -> StyledGrapheme { text, width, style, source }
  -> text_flow
  -> WrappedLine rows
  -> row_from_graphemes
  -> PhysicalRow
  -> PhysicalRow::place
  -> Surface cells
```

`presentation/wrap.rs` performs segmentation and computes `StyledGrapheme.width` once. Subsequent wrapping and text lowering are intended to trust this stored width.

`presentation/paint/text.rs :: row_from_graphemes` explicitly documents that recomputing width there would introduce a second metric between wrap and physical storage. It skips width-zero graphemes, creates a painted leader, and creates painted continuation cells with the leader’s style.

The text painter then:

1. computes the target width from `WidthRule`;
2. computes row offset from horizontal alignment;
3. places complete glyph spans into the allocated width;
4. copies painted cells into a surface;
5. optionally inserts/reverses a cursor marker;
6. propagates the row’s fit/completeness status.

### 4.2 Full-tree paint path

`ViewPainter.paint_tree_with_content` eventually calls `paint_tree_with_style_and_cache`, which calls `paint_node` recursively.

For each node, `paint_node`:

1. increments `PaintNodesVisited`;
2. enters the node’s style state/fact/focus context;
3. resolves node text style to `PhysicalStyle`;
4. computes a `PaintKey`;
5. checks the two-generation cache if permitted;
6. allocates a `Surface` sized to the node rectangle;
7. paints node content;
8. paints/composes children;
9. applies the node surface background;
10. paints border decoration;
11. wraps the result in `Arc<Surface>`;
12. inserts it into the current paint-cache generation if cacheable.

Content variants:

- `Text`: calls `ViewCompiler::paint_text`, then composites the resulting surface.
- `Spacer`: allocates a transparent content surface.
- `ContentHost`: calls `ContentProvider::paint_window` for the full content rectangle.
- `Children`: recursively paints children.
- `Clamp`: recursively paints children, then paints an overflow indicator if the child exceeds the clamp height.
- `RowViewport`: either asks a content host for a direct visible window or paints a child and copies a source-row range.

### 4.3 Row-window paint path

`paint_tree_rows_with_content_and_text_cache` and `paint_tree_row_range_with_text_cache` are optimized paths for retained content and History.

They avoid allocating full node-height surfaces:

```text
for each requested output row:
  paint_row_node(root, row)
    -> allocate width-by-one node output
    -> resolve styles
    -> lower only text/content for this row
    -> recurse only into children intersecting this row
    -> apply one-row background/border decoration
    -> return one-row Surface
  -> convert Surface to PhysicalRow
```

`paint_row_node` uses:

- `clip.intersection(node.clip_rect)` for ordinary nodes;
- a special vertical clip for RowViewport child-source-row translation;
- `child_y_sorted` and `partition_point` for binary pruning of ordinary sorted child ranges;
- an unsorted fallback for hanging/overlay-like children that require original z-order.

Text row geometry is cached by `LayoutNodeId` in `TextGeometryCache`. The complete wrapped-row index is still computed, but only the requested physical row is materialized.

### 4.4 RowViewport path

There are two distinct RowViewport implementations.

#### Full-surface RowViewport

`paint_node` handles `LayoutContent::RowViewport`:

- If the child is a `ContentHost`, `paint_viewport_content_host` directly requests a visible content window.
- Otherwise it paints the child surface and copies rows beginning at `skip_rows`.

The direct content path computes:

- child origin relative to viewport;
- scroll offset;
- content origin;
- signed content and child clip rectangles;
- `ContentWindow { first_row, row_count }`;
- a prepared ticket containing port, connector, offered width, projection revision, and projection identity.

#### Row-window RowViewport

`paint_row_node` handles the same semantic case without allocating the child’s full surface.

For a content host child it calls `paint_viewport_content_host_row`, which:

- resolves child style context;
- paints child background and border at translated origin;
- converts viewport row plus `skip_rows` into a source row;
- computes target content coordinates and clip;
- requests one source row through `ContentProvider::paint_window`.

For a non-content child it recursively asks for the translated source row with `honor_node_clip = false`, then composites the one-row result.

The two paths are tested for equivalence in `presentation/paint/view.rs :: row_viewport_rows_match_full_paint_at_a_nonzero_nested_origin`.

### 4.5 Incremental repaint path

`scene/host.rs :: paint_with_content` selects incremental repaint when:

- no `full_paint_pending` flag is set; and
- at least one History, component, state, or content paint target is queued; and
- `last_surface` is available.

It takes the previous surface and executes, in order:

1. History subtree repaint.
2. State-root subtree repaint.
3. Component-root repaint.
4. Content-root repaint.

Each operation uses `ViewPainter.paint_subtree_into_with_content` or `paint_component_into_with_content`.

`paint_subtree_into_with_content`:

1. walks the subtree path to reconstruct inherited style/background context;
2. obtains incremental geometry and clip from the retained `LayoutTree`;
3. paints the subtree into a temporary surface;
4. clears the old destination region with `clear_rect_with_background`;
5. composites the painted subtree into the previous frame surface.

If any target cannot be found or painted, the incremental pass falls back to a full-tree paint. The previous frame remains authoritative if candidate preparation or backend presentation fails, according to the scene-host lifecycle comments.

### 4.6 Damage lifecycle

`retained_state/damage.rs :: DamageRegion` is not owned by the physical surface. It is generated from semantic state/content changes and attached to the prepared scene frame.

The scene host:

- computes state damage from state roots and `incremental_paint_rect`;
- computes content damage from content repaint roots and `incremental_paint_rect`;
- uses pending geometry damage when a relayout has already compared old/new trees;
- otherwise merges state/content rectangles;
- falls back to full damage if no rectangles are available.

The physical surface is still repainted in the incremental path even though damage metadata is narrower. `DamageRegion` identifies affected output regions; it does not itself perform physical cell clipping.

### 4.7 Terminal lowering path

`terminal/termwiz/lower.rs` consumes `Surface` and `PhysicalRow`.

For each row:

1. choose effective cells, optionally overlaying native History rows;
2. identify the last painted cell;
3. skip continuation cells as independent text;
4. group adjacent same-style leaders into Termwiz `Change::Text`;
5. emit `Change::AllAttributes` before style changes;
6. optionally clear the tail of the line;
7. verify, in debug builds, that the physical leader width equals the Termwiz-emitted text width.

`physical_style` maps:

- missing colors to `ColorAttribute::Default`;
- named colors to Termwiz palette indices;
- indexed colors to palette indices;
- RGB values to true color with default fallback;
- bold over dim when both are set, because Termwiz has one intensity field.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation matrix

| Semantic operation | Primary production path | Alternate path | Selection condition | Fallback/recovery | Production reachable |
|---|---|---|---|---|---|
| Measure text width | `wrap::styled_hard_lines` + `text_flow_metrics` | `compile_text_geometry` during prepared paint | Layout or content measurement | No physical fallback; width-zero and oversized graphemes become incomplete | Yes |
| Lower text to physical cells | `ViewCompiler::paint_text` | `paint_text_geometry_row` | Full paint versus retained geometry row paint | Whole glyph omission if destination cannot fit | Yes |
| Compose child surface | `Surface::composite` | `Surface::composite_clipped` | Full subtree versus signed/clipped row/incremental path | Out-of-bounds painted glyph marks/inherits incompleteness in full path; clipped path skips crossing glyph | Yes |
| Compose content | `ContentProvider::paint_window` | `ContentHostRegistry::paint_window_direct` | All ContentHost paint | Missing ticket returns without writing; provider contract is trusted | Yes |
| Paint a RowViewport | Full `paint_viewport_content_host` or child copy | Row-specific `paint_viewport_content_host_row` | Full surface versus row-window route | Non-content child uses translated recursive route | Yes |
| Paint a changed component | `paint_component_into_with_content` | Full-tree repaint | Stable component root and old surface available | Missing root or failed target causes full repaint | Yes |
| Paint changed content | `paint_subtree_into_with_content` for content roots | Full-tree repaint | Content repaint roots exist | Missing roots cause full repaint | Yes |
| Paint borders | `paint_border` | `paint_border_at` | Ordinary full surface versus signed/clipped/viewport decoration | Off-screen points/ranges skipped | Yes |
| Resolve style | Framework theme → application theme → local style | Direct physical inherited style for missing semantic patch | Every node/span | Missing named style is a no-op; missing theme color becomes `PhysicalColor::Default` | Yes |
| Produce damage | `DamageRegion::from_rects` | `DamageRegion::full` | Incremental geometry/state/content path | More than 64 rectangles or at least half viewport area becomes full damage | Yes |
| Lower to terminal changes | `lower::direct_row_changes` | `lower::row_changes` | Full prepared surface versus History/native row route | Unpainted rows still receive cursor positioning; no last-paint text emitted | Yes |

### 5.2 Missing projection ticket is a silent blanking path

`application/content.rs :: paint_window_direct` first calls `projection_for_ticket`. If no matching prepared projection exists, it returns immediately (`application/content.rs :: 4364–4370`) before marking the target incomplete.

The ticket must match:

- connector identity;
- nonzero projection identity;
- offered width;
- projection revision.

This is intentionally designed to prevent painting from substituting a newer same-width projection. However, the failure behavior is asymmetric:

- a poisoned text-geometry lock marks the target physically incomplete;
- an incomplete projection marks the target physically incomplete;
- an unavailable/mismatched projection ticket returns without writing and without marking incompleteness.

That can produce transparent/blank physical output while the target’s completeness flag remains unchanged. The code comments describe failed-closed ticket behavior, but the current implementation’s observable failure signal is weaker than for other provider failures.

This is a current-source observation, not an executed failure test.

### 5.3 Whole-glyph-safe routes versus cell-copy routes

The canonical safe paths are:

- `Surface::composite`
- `Surface::composite_clipped`
- `PhysicalRow::place`
- `glyph::write_glyph_span`
- `glyph::clear_glyph_covering`
- `PhysicalRow::clipped_after`
- `application/content.rs :: paint_window_direct`, which iterates glyphs and uses `write_glyph_span`

There are also cell-by-cell routes:

1. `Surface::clear_rect_with_background` writes each cell directly.
2. `ViewPainter::paint_node`’s non-content RowViewport branch copies `painted.get(x, source_y)` cell-by-cell (`presentation/paint/view.rs :: 1092–1103`).
3. `paint_overflow_indicator` clears and copies a row cell-by-cell (`presentation/paint/view.rs :: 1296–1302`).
4. `decoration::set_cell_at` writes one cell directly (`presentation/paint/decoration.rs :: 199–226`).

The overflow-indicator path clears the entire destination row first, which substantially reduces stale-glyph risk. The other three routes can be problematic if they encounter a wide glyph boundary.

### 5.4 Static whole-glyph hazards

#### Incremental clear rectangles

`Surface::clear_rect_with_background` clears a rectangle by assigning independent cells (`physical/surface.rs :: 214–246`). It does not call `clear_glyph_covering`.

If an existing wide glyph’s leader lies outside the rectangle while its continuation lies inside, the operation can leave a leader with a missing continuation. The subsequent composite may repair the region if it writes a replacement glyph, but if the replacement is transparent or contains no glyph at that location, the malformed row can remain.

This bypasses the explicit `glyph.rs` invariant that every crop/composite operation should operate on complete glyphs.

#### Border writes

`presentation/paint/decoration.rs :: set_cell_at` assigns a single cell and does not first clear any glyph covering the destination coordinate. A border painted over a continuation cell can therefore leave the old leader/continuation relationship inconsistent unless the surrounding geometry guarantees that border coordinates cannot overlap wide child glyphs.

The ordinary layout model may make such overlap uncommon, but the physical helper itself does not enforce the invariant.

#### Full-surface RowViewport child copy

The non-content full-surface RowViewport branch copies each source cell individually. If a child’s physical row is wider than the viewport output, a wide leader can be copied without its continuation. The row-specific path uses `composite_clipped` and is safer, but the two production routes do not share the same physical-copy primitive.

#### Content-provider trust boundary

`ContentProvider::paint_window` has direct mutable access to `Surface`. The production `ContentHostRegistry` uses glyph-safe logic, but arbitrary implementations—including test providers—can write invalid continuation geometry. Validation is only debug-asserted after some production paths.

### 5.5 Failure masking and trusted invariants

The source generally treats malformed physical geometry as a programming bug:

- constructors use debug assertions;
- lowering uses debug assertions;
- row composition uses debug assertions;
- invalid cursor anchors use assertions;
- invalid projection tickets are treated as no-op in the content provider.

This is consistent with the AGENTS guidance that trusted internal invariant violations should not become best-effort fallback behavior. The inconsistency is that some physical corruption routes do not invoke the whole-glyph helper or post-write validation, while the canonical routes do.

### 5.6 Width and clipping semantics

- An EGC wider than the offered width is not split.
- Word wrapping falls back to grapheme-level hard breaks.
- A row containing an oversized grapheme is retained with `fits = false`.
- Physical placement omits the oversized glyph if it cannot fit the destination.
- `physically_complete` then propagates through layout/content/surface products.
- `NoWrap` deliberately retains an over-wide row as one row and marks it incomplete.
- Width-zero surfaces are supported, and zero-width/zero-height tests exist.

The source treats “fits” and “physically complete” as separate but related concepts:

- `WrappedLine::fits` concerns the logical row against its target width.
- `PhysicalRow::place`’s boolean concerns whether painted glyph spans were copied.
- `Surface::physically_complete` aggregates the physical result.
- `LayoutTree::physically_complete` and `ContentMeasurement::physically_complete` carry the upstream completeness fact through layout and content retention.

---

## 6. Caches, invalidation, scheduling, and performance

### 6.1 Paint cache

`presentation/paint/view.rs :: PaintCache` is a bounded two-generation cache:

```text
current:  HashMap<PaintKey, Arc<Surface>>
previous: HashMap<PaintKey, Arc<Surface>>
theme:    Option<Theme>
```

Epoch behavior:

- On the first theme or a changed theme, both generations are cleared and the new theme is retained.
- On an unchanged theme, `current` is moved into `previous`, and a new empty `current` map begins the epoch.
- Lookup checks `current`, then `previous`.
- A previous-generation hit is promoted into `current`.
- A miss increments `PaintCacheMisses`.
- A hit increments `PaintCacheHits`.

The retention bound is temporal rather than cardinality-based per generation. The test `cache_retention_is_bounded_to_two_generations` paints 32 cacheable children and confirms that after three epochs only 32 current plus 32 previous entries remain.

### 6.2 Paint key

`PaintKey` includes:

- `view_id`
- node `rect`
- `content_rect`
- `clip_rect`
- inherited physical style
- resolved physical style
- node style context
- descendant style context
- text alignment and width intent
- content projection revision
- content metric revision
- decoration/background/border fingerprint

This covers several otherwise-hidden invalidation inputs:

- style inheritance
- focus/focus-within state
- text alignment
- `WidthRule`
- content paint revision
- content metric revision
- border glyph changes
- theme-resolved decoration colors

`ViewId` is regenerated when semantic view modifiers or text payloads are changed through the immutable `View` mapping functions (`presentation/ir.rs :: map_node`, `map_text`). Content hosts instead include explicit projection revisions and identities in the layout/painter data.

### 6.3 Paint-cache invalidation

`PaintCache::invalidate_view_ids` removes entries from both generations whose `PaintKey.view_id` is in the provided set.

Scene-host invalidation invokes this when:

- retained state presentation changes;
- content dependency frontiers change;
- state paint roots change;
- theme/state invalidation requires repaint.

A theme change is handled by `PaintCache::begin_epoch` clearing both generations. The cache therefore does not attempt to treat theme changes as a style-key-only update.

### 6.4 Text geometry cache

`TextGeometryCache` is a `HashMap<LayoutNodeId, TextGeometry>`.

It intentionally stores geometry separately from physical styles:

- segmentation and width-dependent wrapping are computed once;
- semantic span indexes and source ranges are retained;
- each theme repaint reconstructs styles from the current semantic spans and context;
- physical rows are then rebuilt from the stable geometry.

The cache is owned by the caller:

- ordinary `ViewCompiler::compile` creates a new cache;
- content prepared products retain an `Arc<Mutex<TextGeometryCache>>`;
- prefix proofs retain a geometry cache;
- deferred row-window paint reuses the cache associated with its prepared layout product.

The key does not include width or tree identity. The source relies on the cache’s lifetime being scoped to a single prepared `LayoutTree` and offered-width product. That is safe only if callers do not reuse a `TextGeometryCache` across incompatible trees. The current construction paths appear to honor that lifetime, but the type itself does not encode it.

### 6.5 Layout/content revisions involved in paint

`ContentMeasurement` has separate revisions:

- `projection_revision`
- `metric_revision`
- `paint_revision`
- `projection_identity`
- connector identity

The layout cache uses layout-input revisions rather than paint-only revisions. The paint key uses content projection and metric revisions. This separates:

- width/intrinsic geometry changes;
- physical paint/content delivery changes;
- exact prepared product identity.

`LayoutContent::ContentHost` carries these fields into the retained `LayoutTree`, where `ViewPainter` builds `PreparedProjectionTicket` values.

### 6.6 Per-row and per-frame work

Full-tree paint:

- visits every retained node unless a cache hit occurs;
- allocates a surface per uncached node;
- composites all child surfaces;
- lowers full content rectangles.

Row-window paint:

- allocates only width-by-one temporary node surfaces;
- still computes/retains full text row geometry;
- prunes ordinary sorted children with binary search;
- retains original traversal order for unsorted/hanging structures;
- asks content providers for only the requested row window.

Incremental paint:

- reuses `SceneHost::last_surface`;
- clears and repaints affected subtree regions;
- avoids painting clean siblings;
- can still fall back to full paint if roots or retained geometry are unavailable.

### 6.7 Performance counters

`perf.rs` defines counters directly relevant to this assignment:

- `TextFlowMeasureCalls`
- `PaintNodesVisited`
- `PaintCellsAllocated`
- `PaintCacheHits`
- `PaintCacheMisses`
- `SurfaceCellsComposited`
- `ViewStateIncrementalPaints`
- `ViewStateDamageRects`
- `ViewStateFullDamageRepaints`
- `ViewStateGeometryInvalidations`
- `ViewStateGeometryRelayouts`
- `ViewStateGeometryLocalPatches`
- `ViewStateGeometryFullRepaints`
- `ViewStateDirtyPropagationNodes`
- `ContentSurfaceClones`
- `ContentPaintPropagations`
- `ContentMetricEvaluations`
- `ContentMetricChanges`

The physical layer increments:

- `SurfaceCellsComposited` for whole-glyph writes;
- `PaintCellsAllocated` from paint allocation sites.

The counters are compiled as no-ops without the `perf-counters` feature. No benchmark or counter run was performed for this report.

### 6.8 Damage compaction policy

`retained_state/damage.rs :: DamageRegion::from_rects`:

1. clips every rectangle to the viewport;
2. merges overlapping or touching rectangles;
3. records the resulting count;
4. promotes to full damage when:
   - there are more than 64 merged rectangles, or
   - the merged area is at least half the viewport;
5. increments `ViewStateFullDamageRepaints` on full promotion.

The threshold is a policy over rectangles/area, not physical cell complexity. A small number of rectangles containing many wide glyphs is still represented as ordinary rectangle damage; glyph-safe repainting is handled by paint/composition.

---

## 7. Tests, benchmarks, and observability

### 7.1 Physical geometry tests

`physical/tests.rs` verifies:

- transparent and painted blank cells remain distinct;
- zero-width and zero-height surfaces are legal;
- `Surface::composite` propagates incompleteness;
- wide continuation cells count toward physical width but not logical text;
- wide glyphs that do not fit are omitted atomically;
- `PhysicalRow::clipped_after` clears intersecting wide glyphs;
- compositing over a continuation clears the old entire glyph;
- `Surface::crop_to` does not retain a truncated leader;
- physical column lookup does not confuse UTF-8 byte offsets with terminal columns;
- geometry validation rejects truncated wide leaders;
- property-based row placement and crop operations preserve wide-cell validity.

The test corpus includes CJK, emoji, flags, keycaps, ZWJ sequences, combining marks, and variation selectors.

### 7.2 Width and wrapping tests

`presentation/wrap.rs` tests verify:

- termwiz width is used rather than `unicode-width`;
- keycaps, VS-16, ZWJ sequences, flags, and emoji remain atomic;
- combining sequences are not split;
- hard newlines remain logical row boundaries;
- oversized graphemes mark rows as not fitting;
- `NoWrap` retains over-wide rows as one row;
- cursor movement does not rewrap a no-wrap line;
- cursor columns use stored grapheme widths.

`presentation/layout/tests/text.rs`, `flow.rs`, and `mod.rs` additionally assert:

- full and row-based lowering agree;
- physical completeness is propagated;
- row widths remain allocated widths;
- oversized text remains geometrically valid even when incomplete.

### 7.3 Style/theme tests

`presentation/paint/theme.rs` tests verify:

- selectors normalize duplicate/state entries;
- more-specific variants win within a layer;
- application theme overrides framework theme;
- application palette resolves theme colors referenced from framework styles;
- `StyleSpec::plain()` explicitly clears attributes while leaving colors unspecified;
- local sparse styles override named-style fields;
- theme changes recolor physical output without changing geometry;
- focus movement changes focus-dependent physical style;
- inherited-style changes do not reuse a child’s old cached surface.

These tests establish that style resolution is separate from text geometry and that the paint cache must include inherited and focus-related context.

### 7.4 Paint-cache and viewport tests

`presentation/paint/view.rs` tests cover:

- theme-switch invalidation;
- inherited-style cache safety;
- focus-dependent cache invalidation;
- bounded two-generation retention;
- RowViewport scroll and geometry cache safety;
- direct content-host row-window requests;
- prepared row ranges at nonzero rows;
- clipped wide background geometry;
- child decoration and style preservation in RowViewport;
- equality of full and row-window RowViewport output at nested, nonzero origins.

The tests provide behavioral evidence for the intended equivalence between full-surface and row-window painting, but the non-content RowViewport cell-copy path is not independently tested with wide glyphs in the inspected test cases.

### 7.5 Border tests

`presentation/paint/decoration.rs` tests cover:

- large signed-origin decorations visit only the visible edge intersection;
- clipped wide top labels do not leave orphan continuation cells;
- border work counters measure only the visible intersection.

The clipped label test uses the safe text-surface composition path. It does not directly test a border cell overwriting a continuation from an existing child glyph.

### 7.6 Terminal lowering tests

`terminal/termwiz/lower.rs` tests verify:

- physical styles map to Termwiz colors and attributes;
- bold takes precedence over dim;
- wide continuations are not emitted as separate text;
- representative wide graphemes do not cause Termwiz wrapping onto a canary row;
- Iyon leader widths equal Termwiz-emitted text widths;
- `PhysicalRow` geometry is valid before lowering.

`terminal/termwiz/presenter.rs` tests cover desired/presented surface behavior and presenter transactions, but the inspected presenter path does not consume `DamageRegion`.

### 7.7 Observability gaps

Current observability is strong for aggregate work:

- nodes visited;
- cells allocated;
- cache hits/misses;
- cells composited;
- damage rectangle count;
- full-damage promotion;
- incremental paint counts.

It is weaker for physical correctness failures:

- there is no production counter for malformed row geometry;
- `ContentProvider::paint_window` has no returned success/failure value;
- projection-ticket misses return without an incomplete signal;
- no counter distinguishes whole-glyph composition from unsafe direct cell assignment;
- no counter identifies whether row-window or full-surface RowViewport paths are active;
- `DamageRegion` is not visibly consumed by the Termwiz presenter.

No tests or benchmarks were run in this investigation.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Physical layer is nominally backend-neutral but metrically Termwiz-bound

`physical/mod.rs` documents a backend-neutral physical vocabulary, but `physical/text_metrics.rs` directly calls Termwiz. This is an intentional consistency choice: wrapping, painting, clipping, cursor placement, and Termwiz lowering must agree. It nevertheless means the physical metric is currently backend-policy-bearing rather than fully backend-neutral.

This is a cross-boundary coupling between:

```text
Unicode segmentation
  -> Termwiz width policy
  -> layout/wrap
  -> PhysicalRow validation
  -> terminal lowering
```

### 8.2 Semantic presentation is lowered to physical style before backend output

Semantic style remains sparse and theme-aware through `StyleSpec`, `StyleRef`, selectors, and `ThemeResolver`. It becomes a concrete `PhysicalStyle` in `presentation/paint/theme.rs` and `presentation/paint/text.rs`.

Only after that does `terminal/termwiz/lower.rs` map it into Termwiz `CellAttributes`.

This is a healthy separation of semantic style intent from backend attributes, but the resulting `PhysicalStyle` still contains terminal-oriented concepts:

- ANSI named/indexed colors;
- truecolor;
- bold/dim intensity;
- underline;
- reverse;
- strikethrough.

### 8.3 Content providers are coupled to physical surfaces

The content boundary is explicitly presentation-facing and promises not to own source/viewport state. However, its paint method receives:

- `&mut Surface`
- physical coordinates
- physical clip rectangle
- `PhysicalStyle`

Thus content lifecycle and projection ownership are separated from physical rendering, but the content provider still depends on the physical cell representation at its output boundary.

The production provider handles projection identity and width correctly:

- it never substitutes a newer same-width product for a stale prepared ticket;
- it uses connector identity and projection identity;
- it writes complete glyph spans;
- it merges inherited physical style into projected cells.

### 8.4 Decoration depends back on text/layout compiler

`presentation/paint/decoration.rs :: paint_border_at` paints a border label by constructing a `TextView` and invoking `ViewCompiler::with_resolver(theme).paint_text`.

This means the decoration layer depends on the text compiler and physical text lowering rather than being an independent glyph-only decoration primitive. The dependency is reasonable for labels and width handling, but it makes decoration part of the semantic paint compiler rather than a leaf physical operation.

### 8.5 ViewPainter contains both full and incremental architectures

`ViewPainter` is simultaneously:

- a full semantic-tree renderer;
- a row-window physical renderer;
- a content-host direct-window renderer;
- an incremental subtree renderer;
- a cache facade.

These routes share physical primitives in some cases but not all. The physical surface abstraction therefore sits underneath several distinct route implementations with different clipping/copy guarantees.

### 8.6 Layout tree and painter share physical completeness

`LayoutTree`, `LayoutBlock`, `ContentMeasurement`, `HostContentProjection`, `Surface`, and `PhysicalRow` all carry a `physically_complete` concept.

The concept means roughly the same thing—whether wide glyphs fit—but is propagated through several stages:

```text
ContentMeasurement
  -> measured/prepared layout node
  -> LayoutTree
  -> ViewPainter surface/row
  -> Content projection product
  -> PreparedSceneFrame surface
  -> terminal lowering
```

This is important because physical completeness is not just a paint-local optimization. It influences layout fit, retained content metrics, History transfer eligibility, and frame completeness.

### 8.7 Damage does not own or directly drive physical clipping

Damage is generated in retained-state/scene code and attached to the frame. `ViewPainter` still repaints explicit subtree regions and `Surface` still performs physical clipping. The damage rectangle is not passed into the Termwiz presenter in the inspected route.

Therefore there are two distinct concepts:

- **semantic/incremental damage:** which retained roots need repaint;
- **physical clipping:** which cells/glyphs may be written during composition.

They are related but not the same mechanism.

### 8.8 Potential contradiction in physical safety documentation

`physical/glyph.rs` states that every crop/composite path must operate on whole glyphs. The following paths do not visibly enforce that rule:

- `Surface::clear_rect_with_background`;
- border `set_cell_at`;
- full-surface non-content RowViewport cell copy;
- some direct overflow/copy helpers.

The canonical routes are safe, and existing geometry may prevent many of these paths from encountering wide-glyph boundaries. Nonetheless, the documentation describes a stronger invariant than all current helpers enforce.

### 8.9 Generic framework boundary

No Iyon-specific product semantics were observed in the assigned paint/physical modules. The code uses generic concepts:

- `View`
- `ContentProvider`
- `ContentWindow`
- `Projection`
- `History`
- `StyleContext`
- `Theme`
- `Surface`
- `PhysicalRow`

The physical and paint layers are consistent with the framework boundary in AGENTS.md. The root `text.rs` facade is also generic and does not encode application meaning.

---

## 9. Open questions and coverage gaps

1. **Physical metric ownership**
   - Is Termwiz intended to remain the canonical width policy for every future backend, or should width become a framework policy/trait independent of Termwiz?
   - Current source deliberately centralizes the metric, but it does not abstract it.

2. **Projection ticket failure signaling**
   - Should a missing/mismatched `PreparedProjectionTicket` mark the destination incomplete or return an explicit failure?
   - Current `paint_window_direct` returns silently before setting `physically_complete = false`.

3. **Whole-glyph invariant coverage**
   - Are layout guarantees sufficient to prove that `clear_rect_with_background`, border cell writes, and full-surface RowViewport copies can never intersect a wide glyph partially?
   - The physical helpers themselves do not establish that proof.

4. **Incremental clear correctness**
   - Does every incremental repaint necessarily repaint every glyph intersecting the cleared rectangle?
   - If not, cell-wise clear can leave malformed wide-glyph state in the retained surface.

5. **Generic content-provider contract**
   - Is `ContentProvider::paint_window` intended to be trusted internal code only, or should the interface provide a validated row/glyph output abstraction rather than direct mutable `Surface` access?

6. **Row-window cache lifetime**
   - `TextGeometryCache` is keyed by `LayoutNodeId` only. What explicit invariant guarantees that a cache cannot outlive or be reused across a different tree/width domain?
   - Current callers appear to scope it to a prepared layout product, but the type does not encode that.

7. **Damage consumption**
   - `PreparedSceneFrame` carries `DamageRegion`, but the inspected Termwiz presenter path appears to derive its own desired changes from the complete surface.
   - Is the damage metadata intentionally reserved for other backends/future optimization, or is a physical damage route incomplete?

8. **Physical style semantics**
   - `PhysicalStyle` permits both `bold` and `dim`, while Termwiz has one intensity field and chooses bold.
   - Is that normalization policy correct for all semantic combinations, or should sparse style conflict resolution prevent both from becoming true?

9. **Border glyph width**
   - `BorderSpec` stores glyphs as strings, while `set_cell_at` writes one physical cell and does not use `grapheme_cell_width`.
   - What invariant guarantees that custom border glyphs are always one cell wide? The public API validation in `presentation/api/style.rs` appears to validate border glyphs, but the physical setter does not independently enforce it.

10. **RowViewport route equivalence**
    - Full and row-window routes have equivalence tests for ordinary content providers and nested origins, but wide-glyph tests primarily exercise the row-safe path.
    - The non-content full-surface RowViewport copy route needs source-level proof or targeted behavior coverage for wide children.

11. **Surface crop completeness**
    - `Surface::crop_to` marks incomplete output when a wide glyph is clipped horizontally, but its height truncation copies only the overlapping rows and does not visibly mark incompleteness merely because painted source rows were dropped vertically.
    - The function is currently annotated `allow(dead_code)`, so its production reachability is unclear.

12. **Public physical API**
    - All physical types are `pub(crate)`. It is unknown whether future host/backend boundaries are expected to retain this exact internal representation or expose a different backend projection.

### Coverage limitations

The assigned production files were inspected comprehensively. Supporting modules were inspected at the relevant symbols and call sites rather than exhaustively line-by-line. No source modifications, dependency installation, builds, tests, benchmarks, or runtime traces were performed.

---

## 10. Evidence appendix

### 10.1 Primary assigned files and symbols

#### Physical storage and geometry

- `crates/iyon-tui/src/physical/mod.rs`
  - module documentation
  - private module declarations
  - crate-private re-exports

- `crates/iyon-tui/src/physical/glyph.rs`
  - `PhysicalGlyph`
  - `glyphs`
  - `PhysicalGlyphIter`
  - `CellGeometryError`
  - `validate_cells`
  - `copy_glyph`
  - `place_glyphs`
  - `clear_glyph_covering`
  - `write_glyph_span`
  - `cell_x_of`

- `crates/iyon-tui/src/physical/row.rs`
  - `PhysicalRow`
  - `from_cells`
  - `place`
  - `placed`
  - `clipped_after`
  - `occupied_width`
  - `plain_text`
  - `validate_cell_geometry`

- `crates/iyon-tui/src/physical/style.rs`
  - `AnsiColor`
  - `PhysicalColor`
  - `PhysicalStyle`

- `crates/iyon-tui/src/physical/surface.rs`
  - `PhysicalCell`
  - `Surface`
  - `transparent`
  - `blank`
  - `new`
  - `row_cells`
  - `row_cells_mut`
  - `get`
  - `get_mut`
  - `apply_surface_background`
  - `composite`
  - `composite_clipped`
  - `crop_to`
  - `clear_rect`
  - `clear_rect_with_background`
  - `clear_glyph_at`

- `crates/iyon-tui/src/physical/text_metrics.rs`
  - `grapheme_cell_width`
  - `text_cell_width`
  - Unicode/Termwiz corpus tests

- `crates/iyon-tui/src/physical/tests.rs`
  - physical cell distinction tests
  - surface bounds/composition tests
  - wide-glyph clipping tests
  - UTF-8 versus physical-column tests
  - property-based geometry tests

#### Paint and style

- `crates/iyon-tui/src/presentation/paint/mod.rs`
  - `paint_border`
  - `paint_border_at`
  - `TextGeometryCache`
  - `StyleContext`
  - `ThemeResolver`
  - `PaintCache`
  - `ViewPainter`

- `crates/iyon-tui/src/presentation/paint/decoration.rs`
  - `paint_border`
  - `paint_border_at`
  - `border_style`
  - `set_cell_at`
  - `visible_range`
  - `visible_point`
  - border work counters/tests

- `crates/iyon-tui/src/presentation/paint/text.rs`
  - `CompiledTextRow`
  - `TextGeometryGrapheme`
  - `TextGeometryRow`
  - `TextGeometry`
  - `TextGeometryCache`
  - `paint_text`
  - `compile_text_with_metadata`
  - `compile_text_geometry`
  - `paint_text_row`
  - `paint_compiled_text_row`
  - `paint_text_geometry_row`
  - `text_span_ranges`
  - `semantic_span_index_for_source`
  - `validate_cursor_anchor`
  - `row_from_string`
  - `row_from_graphemes`

- `crates/iyon-tui/src/presentation/paint/theme.rs`
  - `StyleContext`
  - `enter_node`
  - `for_descendant`
  - `with_local_facts`
  - `for_scope`
  - `ThemeResolver`
  - `resolve_text_style`
  - `apply_style`
  - `resolve_style`
  - `resolve_theme_color`
  - `resolve_color`
  - `to_physical_color`
  - `to_physical_ansi`
  - style/theme behavior tests

- `crates/iyon-tui/src/presentation/paint/view.rs`
  - `StyleContextKey`
  - `PaintKey`
  - `text_layout_key`
  - `content_projection_revision`
  - `content_metric_revision`
  - `box_paint_key`
  - `PaintCache`
  - `ViewPainter`
  - `paint_tree_rows_with_content_and_text_cache`
  - `paint_tree_row_range_with_text_cache`
  - `paint_row_node`
  - `paint_viewport_content_host_row`
  - `paint_overflow_indicator_row`
  - `paint_tree_with_cache`
  - `paint_tree_with_content`
  - `paint_component_into_with_content`
  - `paint_subtree_into_with_content`
  - `paint_tree_with_style_and_cache`
  - `paint_node`
  - `paint_viewport_content_host`
  - `paint_children`
  - `paint_overflow_indicator`
  - `translated_rect`
  - `intersect_signed_rects`
  - `apply_surface_background_at`
  - `local_clip_for_row`
  - `for_each_child_for_row`
  - `surface_from_row`
  - `node_content_connector_id`
  - `node_content_projection_identity`
  - paint-cache, content-window, clipping, and RowViewport tests

- `crates/iyon-tui/src/text.rs`
  - root `crate::content::text::*` re-export only

### 10.2 Supporting files and symbols

- `crates/iyon-tui/src/presentation/wrap.rs`
  - `StyledGrapheme`
  - `WrappedLine`
  - `styled_hard_lines`
  - `text_flow`
  - `text_flow_metrics`
  - `wrap_styled_lines`
  - `wrap_graphemes_exact`
  - `wrap_line_word_then_grapheme`
  - `cursor_position`
  - Unicode width and wrapping tests

- `crates/iyon-tui/src/presentation/content.rs`
  - `ContentDirtyReason`
  - `ContentWindow`
  - `PreparedProjectionTicket`
  - `ContentMeasurement`
  - `HistoryContentRows`
  - `ContentProvider`
  - `EmptyContentProvider`

- `crates/iyon-tui/src/presentation/layout/mod.rs`
  - `LayoutBlock`
  - `ViewCompiler`
  - `compile_tree`
  - `compile_tree_with_text_cache`
  - `layout_tree`
  - test compilation helpers

- `crates/iyon-tui/src/presentation/layout/measure.rs`
  - `measure_node`
  - `measure_node_uncached`
  - `measure_kind`
  - text/content measurement integration

- `crates/iyon-tui/src/presentation/layout/place.rs`
  - `LayoutNode` construction
  - `LayoutContent` construction
  - `physically_complete` propagation into placed nodes

- `crates/iyon-tui/src/presentation/layout/tree.rs`
  - `LayoutContent`
  - `LayoutNode`
  - `LayoutTree`
  - content/state root indexes
  - incremental paint geometry
  - physical-completeness patch behavior

- `crates/iyon-tui/src/scene/host.rs`
  - `paint_with_content`
  - `pending_damage`
  - `incremental_paint_*`
  - `last_surface`
  - `state_damage`
  - `content_damage`
  - full versus incremental painter selection

- `crates/iyon-tui/src/retained_state/damage.rs`
  - `DamageRegion`
  - `full`
  - `from_rects`
  - `touches`
  - `union`

- `crates/iyon-tui/src/application/content.rs`
  - `ContentHostRegistry::paint_window`
  - `paint_window_direct`
  - `projection_for_ticket`
  - prepared product and text geometry retention
  - glyph-safe content-row composition

- `crates/iyon-tui/src/terminal/termwiz/lower.rs`
  - `desired_surface`
  - `surface_changes_with_overlay`
  - `direct_row_changes`
  - `effective_cell`
  - `row_changes`
  - `iyon_leader_cell_width`
  - `termwiz_emitted_cell_width`
  - `physical_style`
  - `color`
  - Termwiz width/style tests

- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
  - presented/desired surface flow
  - full surface and row transaction behavior

- `crates/iyon-tui/src/terminal/termwiz/shadow.rs`
  - test/shadow terminal physical width and style helpers

- `crates/iyon-tui/src/presentation/api/style.rs`
  - `StyleSpec`
  - `StyleRef`
  - `TextAttributeSpec`
  - `ColorSpec`
  - `BorderSpec`
  - border glyph validation

- `crates/iyon-tui/src/presentation/ir.rs`
  - `ViewId`
  - immutable `View` identity
  - `View::map_node`
  - `View::map_text`
  - `TextView`
  - `Decoration`
  - semantic `ViewKind`

- `crates/iyon-tui/src/perf.rs`
  - `Counter`
  - paint, surface, damage, content, and cache counters
  - feature-gated counter implementation

- `crates/iyon-tui/crates/Cargo.toml`
  - Termwiz, Unicode segmentation, and Unicode line-break dependencies

### 10.3 Read-only investigation methods

Read-only repository search/index operations were used to:

- enumerate assigned and supporting files;
- inspect source contents and exact symbols;
- locate reverse references to `Surface`, `PhysicalRow`, `PhysicalStyle`, `physically_complete`, `DamageRegion`, and paint counters;
- inspect test names and call sites.

No source files or configuration were edited. No dependencies were installed. No tests, builds, benchmarks, or runtime services were run.

### 10.4 Files indexed but not read comprehensively

The remainder of the repository was indexed for reverse references but not read in full. In particular, unrelated Rust subsystems, native addon implementation details, TypeScript packages, generated artifacts, fixtures, examples, and unrelated documentation were outside this assignment’s evidence scope.