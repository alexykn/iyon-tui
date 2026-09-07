# 04 — View / presentation authoring, IR, construction and semantic attachments

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `04`, Rust `view-presentation`
- Required scope: `crates/iyon-tui/src/presentation/`, excluding `presentation/layout/` and `presentation/paint/`
- Assignment goal: authoring API, retained semantic IR, construction, content attachments and semantic representation.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- repository `AGENTS.md`
- the assignment evidence manifest at `docs/architecture/atlas-4355c02/evidence/assignments.json`
- the tracked source manifest at `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

The required source manifest identifies the assigned production files as:

```text
crates/iyon-tui/src/presentation/api/grid.rs
crates/iyon-tui/src/presentation/api/mod.rs
crates/iyon-tui/src/presentation/api/style.rs
crates/iyon-tui/src/presentation/api/text.rs
crates/iyon-tui/src/presentation/api/view.rs
crates/iyon-tui/src/presentation/content.rs
crates/iyon-tui/src/presentation/factory.rs
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/mod.rs
crates/iyon-tui/src/presentation/wrap.rs
```

`presentation/layout/**` and `presentation/paint/**` were indexed as neighboring consumers but were not treated as owned source for this report.

### Scope interpretation

This report describes the current source architecture only. It does not perform the V5 census/disposition analysis requested in the historical handoff, and it does not decide what should be deleted, preserved or rewritten.

The relevant framework boundary is explicit in `AGENTS.md`: `iyon-tui` is generic terminal/UI framework code. The presentation layer handles generic semantic views, text, style, layout declarations, content receiving regions and native retained identities. It does not own application/product meaning.

### Evidence status

- Evidence type: comprehensive static source inspection of all assigned production files.
- Additional seam inspection: `src/lib.rs`, `src/binding/mod.rs`, `src/application/content.rs`, `src/content/text/render/mod.rs`, selected `scene`, `history`, `retained_state`, `theme`, `interaction` and control references.
- Tests: read as source evidence, including tests embedded in `api/grid.rs`, `api/style.rs`, `api/text.rs`, `ir.rs` and `wrap.rs`.
- Execution: no commands, builds, tests, benchmarks or runtime services were executed in this investigation.
- All line references below refer to the inspected baseline source paths and are approximate only where a larger function is discussed.

The central current-state finding is:

> The presentation layer has a private persistent semantic `View` IR, a deliberately narrow hidden/native construction facade, and a canonical final-root construction path. Semantic modifiers rebuild an outer node identity while sharing unchanged payloads. Native state and content identities are retained as validated attachment values rather than wrapper nodes or public pointer identities.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approx. physical LOC | Approx. production/test split | Responsibility | Visibility |
|---|---:|---:|---|---|
| `presentation/mod.rs` | 318 | ~318 / 0 | Module boundaries, hidden native binding wrappers, crate-local reexports | Module is `pub(crate)` from `lib.rs`; selected child modules are hidden/public for binding |
| `presentation/api/mod.rs` | 23 | ~23 / 0 | Curated semantic construction facade and reexports | Hidden public module; most items are reexported through `binding` |
| `presentation/api/view.rs` | 116 | ~116 / 0 | Internal `View` constructors and native common-property patch type | `NativeCommonPatch` is hidden/public under `native-host`; constructor methods are crate-private |
| `presentation/api/style.rs` | 1,181 | ~1,086 / ~95 | Backend-neutral style, border, color, selector and semantic state vocabulary | Public declarations inside hidden presentation API; fields mostly private |
| `presentation/api/text.rs` | 327 | ~254 / ~74 | Text spans, storage, native text pages, wrapping/alignment enums | Public semantic types through hidden binding facade |
| `presentation/api/grid.rs` | 513 | ~250 / ~263 | Grid track/cell authoring records and auto-placement lowering | Public semantic records; lowering/factory methods are crate-private |
| `presentation/content.rs` | 252 | ~252 / 0 | Generic presentation/content-provider seam, measurement, dirty reasons, projection tickets and history row products | Entirely `pub(crate)` |
| `presentation/factory.rs` | 628 | ~628 / 0 | Canonical in-crate factories and semantic modifiers | Entirely `pub(crate)` |
| `presentation/ir.rs` | ~2,630 | ~2,236 / ~394 | Private retained semantic IR, persistent sequences, identity, flags, attachment traversal and native structural edits | `View` is public in the private module; IR internals are crate-private |
| `presentation/wrap.rs` | 839 | ~552 / ~287 | Grapheme tokenization, hard-line splitting, UAX word wrapping, input wrapping, cursor placement and source ranges | Entirely `pub(crate)` |

Approximate total physical size for the assigned files is therefore **about 5,825 lines**, with approximately **5,250 production/comment/implementation lines** and **575 embedded-test lines**, depending on whether documentation and cfg-only sections are counted as production. The counts above were derived from the source line-number ranges visible during inspection, not from an executed `wc`/LOC tool; they are intended as architecture sizing rather than exact code metrics.

No generated source is inside the assigned presentation subtree.

### 1.2 `presentation/mod.rs`

`presentation/mod.rs` establishes the intended layer split:

- `api`: curated semantic construction facade.
- `ir`: private retained semantic tree.
- `layout`: semantic-to-physical compilation.
- `paint`: style/decorative resolution and physical painting.
- `wrap`: Unicode/text wrapping.
- generic stream provenance remains in sibling `stream` code.

Relevant declarations are at `presentation/mod.rs:1-16`.

The module additionally defines a hidden `presentation::binding` module under `feature = "native-host"` (`mod.rs:18-301`). This is the native ingress surface for the TypeScript/native bridge. It wraps factories and private `View` methods rather than exposing the retained IR directly.

The module reexports selected API types and private IR/content types for in-crate users at `mod.rs:303-318`.

### 1.3 `presentation/api/*`

The API directory is not a fluent Rust builder framework. Its documentation explicitly says that consumers construct the canonical owned `View` IR through semantic records, and that the layout representation remains private (`api/mod.rs:1-21`).

The main API categories are:

1. **Style and decoration**
   - `StyleSpec`
   - `StyleRef`
   - `StyleSelector`
   - `StyleStateKey`, `StyleStateValue`
   - `ColorSpec`, `ThemeColor`, `ThemeKey`
   - `BorderSpec`, `BorderGlyphs`, `BorderEdges`, `BorderStyle`
   - `Insets`
   - `OverflowIndicator`
   - `TextAttribute`, `TextAttributeSpec`
   - `VerticalAlign`

2. **Text**
   - `TextSpan`
   - `NativeTextPage` under `native-host`
   - `WrapMode`
   - `HorizontalAlign`

3. **Grid**
   - `GridTrack`
   - `GridCellSpec`

4. **Native patching**
   - `NativeCommonPatch`, containing optional common properties intended to be applied in one final retained root.

These are semantic input/value types. They do not expose terminal cells, geometry, layout caches, physical styles or paint surfaces.

### 1.4 `presentation/factory.rs`

`factory.rs` is the production construction vocabulary. Its module documentation says these functions are the only in-crate construction vocabulary used by production lowering, assemble final retained records directly, and are not a public builder facade (`factory.rs:1-5`).

The factories divide into:

#### Text construction

- `text`
- `styled_text`
- `text_from_spans`
- `text_from_spans_with_style`
- `text_with_style`
- `text_with_style_fill`
- `text_with_cursor`
- `styled_text_fill`

These eventually call `text_with_rules_cursor` (`factory.rs:20-161`), which allocates exactly one `ViewNode` containing `ViewKind::Text(Arc<TextView>)`.

#### Structural construction

- `row`
- `row_specs`
- `column`
- `column_specs`
- `column_persistent`
- `grid`
- `hanging`
- `spacer`
- `container`
- `clamp_rows`
- `row_viewport`
- `row_viewport_default`
- `bounded_row_viewport`

These all construct a complete `ViewNodeParts` value and then call `View::from_node`.

#### Content/component construction

- `content_host`
- `native_component`
- generic `component`

`content_host` validates a positive port identity and stores the content identity in the node’s attachment field (`factory.rs:337-351`).

`native_component` asserts that its raw component identity is nonzero and creates a deferred `ViewKind::ComponentSlot` (`factory.rs:353-367`).

#### Semantic modifiers

- `native_patched`
- `style`
- `wrap`
- `cursor_at`
- `fill_width`
- `fill_height`
- `fit_width`
- `fit_height`
- `padding`
- `background`
- `foreground`
- `border`
- `text_attribute`
- `style_states`
- `with_style_facts`
- `style_fact`
- `style_state`
- `min_width`
- `max_width`
- `min_height`
- `max_height`

Most modifiers call `View::map_node` or `View::map_text`, producing a new root `ViewId` while preserving unchanged child/payload allocations.

### 1.5 `presentation/ir.rs`

`ir.rs` contains no terminal/backend state according to its module documentation (`ir.rs:1-4`). It owns:

- process-local semantic identity (`ViewId`);
- recursive aggregate flags (`ViewFlags`);
- wide persistent child sequences (`PersistentSeq`);
- the outer `View` handle and `ViewNode`;
- native attachment fields;
- semantic `ViewKind` variants;
- text, axis, grid, clamp, viewport and decoration records;
- retained path traversal and replacement;
- native axis/grid structural edits;
- semantic equality separate from identity equality.

The IR is “retained” in the sense that immutable roots and unchanged recursive payloads remain alive through `Arc`, not in the sense that it stores resolved physical cells.

### 1.6 `presentation/content.rs`

`content.rs` is a generic seam between presentation and the application/content registry. It intentionally prevents layout/paint from reaching into source, port, connector or scheduling lifecycle state (`content.rs:1-5`).

Its responsibilities include:

- classifying content invalidation reasons;
- expressing content work by affected port/connector identity;
- describing width/viewport row windows;
- carrying prepared projection identity/revision;
- returning intrinsic measurement and revision fingerprints;
- direct row-window painting into a physical `Surface`;
- optionally transferring physical rows to native History;
- recording accepted history rows;
- supplying an occurrence-specific resident history view;
- retiring history units.

The concrete production implementation is outside this assignment in `application/content.rs`, where `ContentHostRegistry` implements `ContentProvider` (`application/content.rs:6572` onward).

### 1.7 `presentation/wrap.rs`

`wrap.rs` is a semantic-to-physical text-flow helper, but it remains outside `layout/` and `paint/` because its ownership is Unicode/grapheme/tokenization and wrapping policy rather than general layout or terminal painting.

It owns:

- `StyledGrapheme`;
- `WrappedLine`;
- `styled_hard_lines`;
- grapheme-aware tokenization across style-span boundaries;
- `text_flow` and `text_flow_metrics`;
- `wrap_styled_lines`;
- UAX #14 word wrapping with grapheme fallback;
- composer/input wrapping;
- input source ranges;
- cursor placement.

The output contains `PhysicalStyle`, terminal cell widths and source byte ranges. Therefore, the IR upstream is backend-neutral, but the final wrap product is intentionally terminal/physical.

---

## 2. Types, APIs and contracts

### 2.1 Public authoring surface versus internal machinery

The Rust crate itself intentionally does not export the presentation module from the crate root:

- `lib.rs:36` declares `pub(crate) mod presentation`.
- `lib.rs:88-93` reexports the style/text/grid types and `View` only as `pub(crate)`.
- `binding/mod.rs:23-52` is the native-facing reexport surface.
- `binding/mod.rs:52` reexports `View` for native binding use.

Thus, the semantic API declarations use `pub` so the native binding can expose them, but they are not an ordinary external Rust application API. The documented architecture expects application code to author through the TypeScript facade.

The binding-facing presentation surface includes:

- `GridTrack`, `GridCellSpec`;
- style/color/border/inset/selector types;
- `TextSpan`, `WrapMode`, `HorizontalAlign`;
- `NativeTextPage`;
- `NativeCommonPatch`;
- `View`;
- hidden `view_*` operations for construction, native edits, attachments and weak handles.

The distinction is important:

| Surface | Consumers | Role |
|---|---|---|
| `presentation::api::*` | native binding and in-crate semantic renderers | Typed semantic values |
| `presentation::factory::*` | Rust components, scene/history/application code, semantic renderers and tests | Internal canonical lowering vocabulary |
| `presentation::ir::*` | layout, paint, scene, retained state, native binding and in-crate tests | Retained semantic runtime representation |
| `presentation::binding::*` | native bridge | Hidden ABI-oriented function wrappers |
| `presentation::content::*` | `SceneHost`, layout/paint, application content registry and History transfer | Generic content delivery seam |
| `presentation::wrap::*` | layout/text-input/paint paths | Terminal-aware text flow |

### 2.2 View identity and semantic equality

`View` is:

```rust
pub struct View {
    inner: Arc<ViewNode>,
}
```

at `ir.rs:687-695`.

The identity model is intentionally split:

- `ViewId(u64)` is generated from a process-local atomic counter (`ir.rs:30-45`).
- `ViewId` is used for cache/retention and cycle/path bookkeeping.
- `ViewId` is not included in semantic equality.
- Cloning a `View` increments the outer `Arc` only (`ir.rs:718-724`).
- `View::ptr_eq` compares the outer `Arc` (`ir.rs:1008-1010`).
- `ViewNode::semantic_eq` compares semantic fields and payload, excluding `id` and cached `flags` (`ir.rs:1933-1942`).

Tests establish the intended behavior:

- `clone_retains_identity_and_only_clones_the_outer_arc` (`ir.rs:2241-2249`).
- `semantic_mutation_gets_a_new_identity_even_when_unique` (`ir.rs:2251-2257`).
- `semantic_mutation_gets_a_new_identity_when_shared` (`ir.rs:2259-2268`).
- `semantic_equality_ignores_view_identity` (`ir.rs:2271-2280`).
- `changing_a_parent_retains_an_unchanged_child_identity` (`ir.rs:2282-2306`).

This gives the runtime two independent notions:

```text
same semantic value  !=  same retained occurrence/root identity
```

### 2.3 `ViewNode` fields

`ViewNode` (`ir.rs:700-715`) stores:

- `id: ViewId`
- aggregate `flags: ViewFlags`
- `state_attachment: Option<u64>`
- `content_attachment: Option<u64>`
- `width: WidthRule`
- `height: HeightRule`
- `decoration: Decoration`
- `style_states: StyleStates`
- `style_facts: StyleFacts`
- `kind: ViewKind`

The attachment comments explicitly distinguish these values from public semantic/native pointers:

- state attachment: a native retained state preparation value, not a public pointer identity;
- content attachment: resolved at the structural boundary, not a semantic/native pointer identity.

### 2.4 View kind vocabulary

`ViewKind` is declared at `ir.rs:1959-1977`:

- `Text(Arc<TextView>)`
- `Column(Arc<ColumnView>)`
- `Row(Arc<RowView>)`
- `Grid(Arc<GridView>)`
- `Hanging(Arc<HangingView>)`
- `Container(Arc<ContainerNode>)`
- `Spacer { rows: u16 }`
- `ClampRows(Arc<ClampRowsView>)`
- `RowViewport(Arc<RowViewportView>)`
- `ComponentSlot(ComponentSlotNode)`
- `ContentHost`

The variants distinguish semantic constructs that layout/paint interpret later. `ContentHost` is not a materialized text/content tree; it is a structural receiving region.

### 2.5 Text IR

`TextView` (`ir.rs:2000-2025`) contains:

- `spans: Arc<[TextSpan]>`
- `wrap: WrapMode`
- `align: HorizontalAlign`
- optional `TextCursorAnchor`

`TextCursorAnchor` stores a UTF-8 source byte offset, not a terminal coordinate (`ir.rs:2009-2014`).

`TextSpan` (`api/text.rs:110-116`) contains:

- `TextStorage`
- `StyleRef`
- `StyleFacts`

The storage representation is optimized for retained cloning and source-backed lowering:

```text
Inline      <= 12 bytes, copied into [u8; 12]
PageSlice   Arc<NativeUtf8Page> + byte range
SourcePage  Arc<str> + byte range
Owned       String
```

See `api/text.rs:9-34` and `api/text.rs:73-107`.

Important properties:

- `TextStorage` equality compares resulting string content (`api/text.rs:67-70`).
- cloned `PageSlice` and `SourcePage` values share the same `Arc` allocation (`api/text.rs:36-55`).
- mutable access to a borrowed/page-backed span materializes an `Owned(String)` copy (`api/text.rs:166-173`).
- `NativeTextPage::span` validates bounds and UTF-8 boundaries before constructing a `PageSlice` (`api/text.rs:118-157`).
- `TextSpan::from_source_page` retains the source page in the final `View`, so source ranges remain valid after source-side cache eviction (`api/text.rs:209-218`).

The source-backed text renderer uses this directly. `content/text/render/mod.rs:497-499` calls `factory::text_from_spans_with_style` with `TextSpan::from_source_page`, so semantic text rendering does not need to copy each projected run into a separate owned `String`.

### 2.6 Style representation

`StyleSpec` is a sparse backend-neutral patch:

- optional foreground;
- optional background;
- sparse optional boolean attributes.

See `api/style.rs:19-27`.

`StyleSpec::plain()` is intentionally different from `StyleSpec::new()`:

- `new()` leaves all fields unspecified/inheritable.
- `plain()` explicitly sets every supported text attribute to `false` while leaving colors unspecified (`style.rs:34-48`).

`StyleSpec::overlay` only applies explicitly specified incoming values and therefore preserves unspecified existing fields (`style.rs:128-138`).

`StyleRef` combines:

- optional semantic theme key;
- local sparse override (`style.rs:614-619`).

It can be:

- direct local style;
- theme-only;
- themed with local overrides;
- extended through sparse overrides.

`StyleRef::Deref<Target = StyleSpec>` provides convenience access to the local style, but its theme identity remains separate (`style.rs:621-687`).

Two private style-state bags are semantically distinct:

- `StyleStates`: inheritable semantic context propagated through the presentation tree.
- `StyleFacts`: self-only facts for the current node/span/run and cleared before descending.

See `style.rs:348-396`.

`StyleSelector` matches:

- positive framework predicates `focused` and `focus_within`;
- positive application-owned key/value predicates.

A selector first checks `StyleFacts`, then inherited `StyleStates` (`style.rs:440-521`). This gives node/span-specific facts precedence over inherited context.

Assignments are maintained in sorted vectors with binary lookup (`style.rs:307-345`), keeping small style bags deterministic and cloneable.

### 2.7 Borders and overflow

`BorderGlyphs::new` validates all eight glyphs:

- exactly one grapheme;
- exactly one terminal cell wide.

It uses `unicode_segmentation` and `crate::physical::text_cell_width` (`style.rs:842-925`).

`BorderSpec` stores:

- border family;
- optional color;
- enabled edges;
- glyph set;
- optional semantic top label.

The border dimensions are one cell per enabled edge (`style.rs:943-1068`).

`OverflowIndicator` supports:

- `None`;
- `Ellipsis { style }`;
- `Footer { prefix, style }`.

It remains semantic input; the actual truncation/paint behavior is handled by neighboring layout/paint code.

### 2.8 Grid records and lowering

`GridTrack` (`api/grid.rs:19-59`) is a semantic wrapper around private `TrackSize`:

- content;
- content with maximum;
- fixed;
- flex;
- flex with maximum.

`GridCellSpec` (`api/grid.rs:62-118`) stores:

- nonzero column span;
- nonzero row span;
- horizontal alignment;
- vertical alignment.

A zero span causes an explicit panic through `NonZeroU16::new(...).expect(...)`, treating it as an invalid authoring invariant rather than a recoverable runtime error.

`lower_grid_parts` (`api/grid.rs:121-176`) performs source-order auto-placement:

- tracks are consumed from input;
- each cell is placed at the first available column;
- row/column spans mark occupancy;
- implicit columns become content tracks;
- implicit rows are added when row spans extend beyond explicitly declared rows;
- source cell order is preserved;
- a coordinate-to-cell-index `HashMap` is built for retained updates.

The resulting `GridView` stores persistent sequences for columns, rows and cells plus the coordinate index.

The grid tests at `api/grid.rs:268-511` cover:

- basic row-major placement;
- column spans;
- row spans;
- combined spans;
- implicit columns;
- implicit rows;
- component identity propagation;
- source-order preservation.

A consequential detail is that `cell_indices` indexes explicit cell starting coordinates, not every coordinate covered by a spanning cell. `native_grid_set_cell` therefore targets the cell’s recorded origin coordinate, not an arbitrary coordinate inside its span (`ir.rs:1204-1230`).

### 2.9 Native common patch

`NativeCommonPatch` (`api/view.rs:12-33`) is a hidden/native ingress record with optional fields:

- padding;
- background;
- foreground;
- border;
- style;
- style states;
- fill/fit width and height;
- min/max width and height.

The contract is that `None` preserves the base value. It is applied by `factory::native_patched` in one final root (`factory.rs:396-450`).

Notable style behavior:

- a themed incoming `StyleRef` replaces the node text style;
- a direct/local style overlays existing local style;
- foreground is overlaid sparsely;
- all existing attachments are preserved because patching starts from `ViewNodeParts::from_view`.

The IR tests compare this final patch path with a sequential modifier chain and assert one final root construction (`ir.rs:2401-2443`).

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
TypeScript semantic facade
        │
        ▼
native binding / generated ABI wrappers
        │
        ▼
presentation::binding
        │
        ├── presentation::api::{text, style, grid, view}
        │
        ├── presentation::factory
        │       └── presentation::ir::View::from_node / map_node / map_text
        │
        └── presentation::ir native edit/attachment methods

Rust Component / Renderer / Scene / History / application content
        │
        ▼
presentation::factory + presentation::api values
        │
        ▼
private semantic View IR
        ├── scene component resolution
        ├── retained state attachment discovery
        ├── presentation/layout (out of assignment)
        ├── presentation/paint (out of assignment)
        ├── presentation/content provider seam
        └── presentation/wrap
```

### 3.2 Reverse ownership map

| Value/object | Created by | Retained by | Destroyed when |
|---|---|---|---|
| `View` outer handle | `View::from_node` or `map_node` | caller/scene/history/component/native handle via `Arc<ViewNode>` | last strong `Arc` drops |
| `ViewNode` | canonical final constructor | `View` strong references and optional `WeakView` | last strong reference drops; weak upgrade then fails |
| `ViewId` | `next_view_id` at each final root | inside `ViewNode` | with node |
| child `PersistentSeq` tree | `PersistentSeq::from_vec`, set/insert/remove/split/concat/splice | `RowView`, `ColumnView`, `GridView` | last owning semantic payload drops |
| text page | `NativeTextPage::new` or source renderer `Arc<str>` | `TextStorage::PageSlice`/`SourcePage` spans | last span/page `Arc` drops |
| component identity | `ComponentSlotNode` from `component`/`native_component` | `ViewKind::ComponentSlot` and aggregate flags | with semantic tree |
| state attachment | `native_with_state_attachment` | `ViewNode.state_attachment` | with semantic node |
| content attachment | `factory::content_host` or `native_with_content_attachment` | `ViewNode.content_attachment` | with semantic node |
| content projection | application `ContentHostRegistry` | content registry and prepared ticket consumers | registry/projection lifecycle |
| weak view | `View::downgrade` | `WeakView` | weak object drop; cannot extend strong lifetime |

### 3.3 One-root construction path

The canonical construction sequence is:

```text
caller/native binding supplies semantic records
        │
        ▼
factory/API validates and normalizes input
        │
        ▼
ViewNodeParts assembled completely
        │
        ▼
View::from_node(parts)
        │
        ├── allocates one Arc<ViewNode>
        ├── assigns fresh ViewId
        ├── computes recursive ViewFlags once
        └── stores semantic fields and attachments
```

`View::from_node` is at `ir.rs:766-790`. Its documentation explicitly prohibits following a final `from_node` call with a modifier chain on hot ingress paths. The native patch and final grid tests validate this construction strategy.

### 3.4 Modifier update path

```text
existing View
   │
   ▼
ViewNodeParts::from_view
   │   (clone scalar/decorative state, retain child payload Arc)
   ▼
factory/API applies one semantic update
   │
   ▼
View::map_node or View::map_text
   │
   ├── shallow-clone node
   ├── update selected field
   ├── recompute aggregate flags
   ├── assign fresh ViewId
   └── allocate one new outer Arc<ViewNode>
```

`map_node` and `map_text` are at `ir.rs:1012-1032`.

### 3.5 Persistent sequence ownership

`PersistentSeq<T>` uses a 32-way tree (`ir.rs:82-107`):

- leaf items are `Arc<[T]>`;
- branches hold child `Arc`s and cumulative sizes;
- aggregate flags are maintained at every node;
- indexing uses cumulative sizes and `partition_point`;
- `set` copies the root-to-leaf path;
- insertion/removal/split/concat/splice retain unchanged subtrees where possible.

This is particularly relevant to native structural updates. Axis edits do not flatten wide children into a new `Vec`:

- `native_axis_set_child` uses `PersistentSeq::set` (`ir.rs:1090-1141`);
- `native_axis_splice` uses `PersistentSeq::splice` (`ir.rs:1147-1198`);
- `native_grid_set_cell` uses `PersistentSeq::set` for the cell sequence (`ir.rs:1204-1230`);
- batched retained replacement chains sequence `set` calls and allocates one new parent root (`ir.rs:1470-1482`).

### 3.6 Content ownership boundary

```text
ContentHost View
   │
   └── content_attachment: port_id
            │
            ▼
Scene/layout identifies occurrence and offered width
            │
            ▼
ContentProvider::projection_revision / layout_input_revision
            │
            ▼
ContentProvider::measure
            │
            ├── ContentMeasurement
            └── PreparedProjectionTicket
            │
            ▼
ContentProvider::paint_window
            │
            ▼
physical Surface
```

The presentation layer deliberately does not own source bytes, connector selection or projection scheduling. Those are owned by the application/content registry. The `ContentProvider` trait only receives typed identities, widths, tickets, row windows and physical destinations.

---

## 4. Execution paths and state transitions

### 4.1 Plain text construction

Representative path:

```text
Component::view / renderer / binding
    → factory::text("...")
    → TextSpan::plain
    → factory::text_from_spans
    → factory::text_from_spans_with_style
    → factory::text_with_rules_cursor
    → View::from_node(ViewNodeParts { kind: ViewKind::Text(...) })
```

Relevant source:

- `factory.rs:20-58`
- `factory.rs:72-97`
- `api/text.rs:160-199`
- `ir.rs:766-790`

For semantic text rendering, the source-backed route is:

```text
semantic text renderer
    → TextSpan::from_source_page(...)
    → factory::text_from_spans_with_style(...)
    → TextView.spans: Arc<[TextSpan]>
```

This keeps source-backed byte ranges alive without copying each source run.

### 4.2 Styled text and text-local updates

A text view contains multiple styled spans and a node-level text style. Text-local modifiers use `View::map_text`:

```text
View
  → map_text
  → Arc::make_mut(TextView)
  → modify wrap/align/cursor
  → map_node
  → new outer semantic root
```

`factory::wrap` and `factory::cursor_at` use this route (`factory.rs:524-537`).

The `Arc::make_mut` behavior means the `TextView` payload is cloned only if it is shared. Regardless of uniqueness, the semantic root receives a fresh `ViewId`.

The checked native text patch path first verifies `ViewKind::Text` and returns `"text layout patch base is not text"` instead of panicking (`api/view.rs:103-115`).

### 4.3 Row/column construction

Rows and columns own child order, tracks and gaps:

- `RowView` owns `PersistentSeq<RowChild>`, gap and vertical alignment.
- `ColumnView` owns `PersistentSeq<ColumnChild>` and gap.
- each child owns a `TrackSize` and a `View`.

Construction uses `PersistentSeq::from_vec`, so the initial child vector is moved into the final sequence. `ir.rs:109-144` specifically documents avoiding an unnecessary second ownership pass.

The semantic parent owns sibling gaps. Child views do not carry neighboring spacing.

### 4.4 Grid construction

The grid path is:

```text
native or in-crate grid records
    → factory::grid
    → View::grid_from_parts
    → api::grid::lower_grid_parts
    → auto-placement / implicit track insertion
    → GridView with persistent columns, rows and cells
    → View::new_kind
    → View::from_node
```

`factory.rs:272-297`, `api/grid.rs:121-196`.

Grid placement is eager at semantic construction time, not deferred until layout. The retained IR therefore contains explicit cell row/column/span positions before physical layout.

### 4.5 Component-slot construction and resolution

`factory::component` and `factory::native_component` create `ViewKind::ComponentSlot` values. The node does not contain the concrete component view; it stores `ComponentId`.

Aggregate component presence is computed in `ViewFlags`, allowing scene/component resolution to stop early for subtrees without component slots. `ViewFlags::CONTAINS_COMPONENT_SLOT` and `ViewNode::compute_flags` are at `ir.rs:48-80` and `ir.rs:1899-1915`.

The scene tests exercise this route extensively, including:

- static trees without mounts;
- nested component slots;
- component cycles;
- repeated component use;
- component replacement;
- hanging prefixes and component identity restrictions.

The component slot itself is therefore a semantic indirection, not a concrete visual node.

### 4.6 Content-host construction

The content-host path is:

```text
port_id from native/content caller
    → factory::content_host(port_id)
    → reject port_id == 0
    → ViewNodeParts {
          kind: ViewKind::ContentHost,
          content_attachment: Some(port_id),
      }
    → View::from_node
```

`factory.rs:337-351`.

The native attachment method (`ir.rs:850-865`) independently verifies:

- nonzero port identity;
- node kind is `ViewKind::ContentHost`.

The attachment is retained on the structural occurrence. It is not converted into an owned content object and does not itself carry projection rows.

`application/content.rs:6658-6660` reads the attached port identity when deriving a History resident view. Layout/paint use the same occurrence identity to request measurement and paint from `ContentProvider`.

### 4.7 Retained state attachment

Native state attachment is deliberately applied after ordinary semantic construction:

```text
ordinary semantic View
    → native_with_state_attachment(state_id)
    → validate positive ID
    → validate presentation_state_capable(kind)
    → map_node(state_attachment = Some(state_id))
    → new retained root
```

`ir.rs:832-847`.

`native_state_capable` delegates capability classification to `retained_state::presentation_state_capable` (`ir.rs:880-887`). The presentation layer therefore does not duplicate the full state-kind policy.

For a candidate root, `native_state_attachment_targets` recursively traverses the semantic tree:

- uses `ViewFlags` to skip trees without attachments;
- detects duplicate `ViewState` identities;
- detects cyclic semantic graphs;
- returns `(state_id, StateNodeKind)` pairs for retained-state validation.

See `ir.rs:890-998`.

The traversal descends through:

- columns;
- rows;
- grids;
- hanging prefixes/body;
- containers;
- clamps;
- row viewports.

It intentionally does not recurse into `Text`, `Spacer`, `ComponentSlot` or `ContentHost`, because those variants do not contain nested semantic child views in this IR representation.

### 4.8 Native axis structural edits

The native axis construction/editing route uses compact track words:

```text
track_word
  low byte: track kind
  high 16 bits: value
```

`decode_native_track_word` supports:

- zero or kind 1: unbounded content;
- kind 2: content max;
- kind 3: fixed;
- kind 4: flex with minimum;
- kind 5: flex max.

Invalid combinations return `Err(String)` (`ir.rs:628-652`).

`native_axis_from_children` validates/decode-converts each track before allocating the final root, and builds `RowChild` or `ColumnChild` directly (`ir.rs:1042-1082`).

`native_axis_set_child`:

- checks row/column kind;
- checks child index;
- preserves the existing track when `track_word == 0`;
- otherwise decodes a replacement track;
- calls persistent `set`;
- rebuilds one outer semantic root.

`native_axis_splice`:

- decodes inserted tracks;
- validates splice bounds;
- calls persistent `splice`;
- rebuilds one outer root.

The implementation comments preserve validation ordering: inserted track words are decoded before splice range validation.

### 4.9 Native retained path edits

`RetainedPathStep` carries only semantic selectors and expected parent-kind tags; it never retains a `View`. Native ABI code interns these steps externally.

The path operations support:

- container child;
- clamp child;
- row/column child;
- grid cell;
- row-viewport child;
- hanging prefix;
- hanging continuation;
- hanging body.

`try_retained_child` validates expected view kind and selector shape/range before descending (`ir.rs:1381-1455`).

`try_replace_retained_child` and `try_replace_retained_children` rebuild immutable ancestors while sharing unchanged siblings (`ir.rs:1458-1482`).

The batched operation validates all steps first, then applies multiple replacements to one parent. For sequence parents it uses persistent `set` rather than flattening. The test at `ir.rs:2547-2604` confirms:

- semantic equality with sequential replacements;
- a changed root identity;
- error parity for out-of-range later replacements;
- empty replacement list is a pointer-preserving no-op.

Path depth is bounded at 128 in text-layout patch entrypoints (`ir.rs:1349-1374`).

### 4.10 Text wrapping and cursor transition

The wrapping route is:

```text
TextView.spans
    → styled_hard_lines
    → StyledGrapheme tokens
        - text/Cow
        - stored terminal cell width
        - physical style
        - optional source byte range
    → text_flow / wrap_styled_lines
    → WrappedLine rows
    → cursor_position (when requested)
```

`wrap.rs:61-173`, `wrap.rs:182-229`.

Hard lines split on `\n`, but grapheme clusters can cross span boundaries. The implementation concatenates fragments for tokenization when necessary and assigns the style of the grapheme’s first contributing span (`wrap.rs:121-169`).

The stored `StyledGrapheme.width` is authoritative for all later wrapping and cursor calculations. Tests specifically check termwiz cell widths for emoji, keycaps, variation selectors, ZWJ clusters and CJK.

Cursor anchors are validated as:

- not past source length;
- valid UTF-8 boundaries.

Invalid anchors assert loudly (`wrap.rs:212-223`). When the cursor falls inside an extended grapheme, it snaps to the grapheme’s leading cell instead of bisecting it.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Primary path | Alternate/compatibility route | Failure behavior |
|---|---|---|---|
| Plain text | `factory::text` → `text_from_spans` → `ViewKind::Text` | `binding::view_text_plain` | No ordinary failure; caller-supplied string becomes retained text |
| Styled text spans | `factory::styled_text` / `text_from_spans_with_style` | `binding::view_styled_text`, semantic text renderer source-page route | Span storage is retained; invalid native page ranges return `None` |
| Text wrapping/alignment | `factory::wrap` or native checked patch | `View::map_text` / path patch | Non-text checked patch returns `Err`; invalid cursor asserts during flow |
| Row/column | `factory::row`, `column`, specs | native axis constructor from compact words | Invalid track word, wrong kind or index/range returns `Err` |
| Grid | `factory::grid` → `lower_grid_parts` | native final grid binding | Zero spans panic during semantic input construction; invalid native cell coordinates return `Err` |
| Component slot | `factory::component` / `native_component` | `View::component` test helper | Native raw ID zero asserts |
| Content host | `factory::content_host` | `binding::view_native_content_host`; `View::content_host` internal | Port ID zero returns `Err` |
| State attachment | `native_with_state_attachment` | hidden binding wrapper | Zero ID or unsupported node kind returns `Err`; duplicate IDs rejected during target collection |
| Common style/layout patch | `factory::native_patched` | sequential Rust modifiers | Patch itself preserves attachments; unknown semantic values are not represented |
| Structural path replacement | `native_replace_at_path`, batched child replacement | single-child replacement helpers | Wrong expected kind, selector/range or unknown path step returns `Err` |
| Content measurement | `ContentProvider::measure` | `EmptyContentProvider` default | Empty provider returns zero/default metrics |
| Content paint | `ContentProvider::paint_window` | no-op empty provider | Provider controls ticket validation/rejection outside this layer |
| History transfer | `history_rows`, `history_rows_committed`, `history_view` | default `None`/clone/no-op methods | Optional behavior is absent unless provider implements it |
| Unicode wrapping | `wrap_styled_lines` | `wrap_input_styled_lines` for input/caret reservation | Oversized graphemes are emitted as non-fitting rows; clusters are never split |

### 5.2 Invalid input and explicit errors

Observed explicit failure contracts include:

- `content_host(0)` → `Err("ContentPort identity must be positive")` (`factory.rs:337-340`).
- `native_with_content_attachment(0)` → same positive-ID error (`ir.rs:854-857`).
- content attachment on a non-`ContentHost` → `Err("ContentPort is unsupported on this node kind")` (`ir.rs:858-860`).
- `native_with_state_attachment(0)` → `Err("ViewState identity must be positive")` (`ir.rs:837-840`).
- state attachment on unsupported kind → `Err("ViewState is unsupported on this node kind")` (`ir.rs:841-843`).
- invalid native track kind → `Err("native axis track kind is invalid")` (`ir.rs:629-652`).
- content track carrying a nonzero amount → explicit `Err`.
- axis edits on non-axis nodes → `Err`.
- axis child index out of range → `Err`.
- axis splice range out of bounds → `Err`.
- grid edits on non-grid nodes → `Err`.
- unknown grid start coordinate → `Err`.
- path expected-kind mismatch → `Err`.
- malformed path selector → `Err`.
- path depth above 128 → `Err("retained path exceeds maximum depth")`.
- text layout patch on non-text node → `Err("text layout patch base is not text")`.
- duplicate retained state identity → `Err("duplicate ViewState attachment")`.
- cyclic semantic graph during state target collection → `Err("cyclic semantic View graph")`.
- `NativeTextPage::span` out-of-bounds or UTF-8-splitting range → `None`.
- invalid border glyph shape/width → `BorderGlyphError`.
- zero grid span → panic via `NonZeroU16::new(...).expect(...)`.
- zero native component identity → assertion.
- hanging continuation prefix containing component identity → assertion (`factory.rs:303-307`).
- invalid cursor source offset/boundary → assertion in `wrap.rs:214-220`.

The pattern is consistent with the repository instructions: untrusted/native boundary inputs return typed/string errors; established internal invariant violations assert or panic rather than silently default.

### 5.3 Legitimate fallback/default routes

The content trait has intentional default methods:

- `set_theme` is no-op by default;
- `layout_input_revision` defaults to `projection_revision`;
- history row transfer defaults to `None`;
- history commit/retirement methods are no-op;
- `history_view` defaults to cloning the original view;
- `history_transfer_blocked` defaults to `false`.

`EmptyContentProvider` is an explicit compatibility provider for generic layout/paint callers and tests that do not mount a retained `ContentPort` (`content.rs:223-252`). Its behavior is:

- revision `0`;
- zero/default measurement;
- no painting.

This is a deliberate unmounted-content route, not evidence that content should be silently rendered as empty in a mounted production path. However, the presentation seam itself cannot distinguish an intentionally empty provider from a provider that was accidentally omitted; that distinction belongs to its caller.

### 5.4 Search coverage and absence claims

Within the assigned presentation files, no alternate public Rust builder hierarchy, closure-scoped View builder, or second retained semantic node representation was found. The only construction mechanisms are:

- direct factory functions;
- `View::from_node`;
- `View::new_kind` for selected internal paths;
- `View::map_node`/`map_text`;
- hidden native retained-edit methods.

This absence statement is limited to the assigned source scope and supporting references inspected here; it does not claim that no alternate route exists elsewhere in the repository.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Identity and structural sharing

The presentation IR has several optimization mechanisms:

1. **Outer `Arc<ViewNode>`**
   - cheap `View::clone`;
   - weak handles do not keep nodes alive;
   - pointer identity available through `View::ptr_eq`.

2. **Persistent child sequences**
   - 32-way tree;
   - path-copy updates;
   - unchanged sibling chunks retained;
   - aggregate flags avoid recursive scans for common presence checks.

3. **Cached recursive flags**
   - component-slot presence;
   - state-attachment presence;
   - content-attachment presence.

4. **Text storage sharing**
   - inline short strings;
   - shared native text pages;
   - source-backed `Arc<str>` slices.

5. **Theme key sharing**
   - `ThemeKey` stores `Arc<str>` (`style.rs:574-610`).

6. **One-final-root native lowering**
   - avoids constructing an intermediate chain of modifier roots on common native ingress paths;
   - counters in `ir.rs` measure `ViewNodesConstructedRust`, clones and persistent sequence allocations.

### 6.2 Invalidation keys at the content seam

The presentation/content contract is revision-keyed:

- `PreparedProjectionTicket` contains:
  - `port_id`;
  - optional `connector_id`;
  - offered width;
  - projection revision;
  - projection identity.

The documentation explicitly says connector identity is required because one port may have several connectors with equal widths (`content.rs:88-102`).

`ContentMeasurement` contains:

- intrinsic `Size`;
- `physically_complete`;
- projection revision;
- metric revision;
- paint revision;
- connector identity;
- projection identity (`content.rs:104-120`).

This separates:

- projection/source changes;
- intrinsic metric changes;
- paint/theme/delivery-frontier changes.

The layout cache therefore has enough information to avoid treating width alone as a safe content selector. The provider validates the prepared projection identity at paint time outside this module.

### 6.3 Dirty categories

`ContentDirtyReason` (`content.rs:21-44`) distinguishes:

- `SourceInput`;
- `DeliveryVisibility`;
- `WidthOrMeasurement`;
- `Presentation`;
- `Viewport`;
- `SelectionLifecycle`.

Measurement-required reasons:

- source input;
- delivery visibility;
- width/measurement;
- selection lifecycle.

Paint-only reasons:

- presentation;
- viewport.

This is a framework invalidation taxonomy. It does not encode application concepts such as message status or completion.

### 6.4 Per-operation/per-frame work

Within the assigned source:

- `TextSpan::text()` is O(1) for stored ranges.
- `TextSpan::text_mut()` materializes a copy for borrowed/page-backed data.
- style assignment lookup is binary search over sorted vectors.
- selector state requirements are sorted at construction and checked linearly over the selector’s small predicate bag.
- `PersistentSeq::get` is logarithmic in tree height.
- `PersistentSeq::set` copies the root-to-leaf path.
- `PersistentSeq::splice` composes split/concat operations.
- grid auto-placement is construction-time work over input rows/cells and occupancy state.
- wrapping tokenizes and wraps per invocation; no persistent wrapping cache exists in `wrap.rs`.
- `text_flow_metrics` increments `Counter::TextFlowMeasureCalls` (`wrap.rs:239-263`).
- View cloning increments `Counter::ViewCloneCalls` (`ir.rs:718-724`).
- final retained node construction increments `Counter::ViewNodesConstructedRust` (`ir.rs:771-789`).
- persistent sequence allocation/cloning counters are updated in `PersistentSeq` methods (`ir.rs:138-149`, `ir.rs:189-198`).

No scheduling loop is owned by this scope. Scheduling and frame requests are handled by application/scene/host layers, while presentation receives candidate inputs and provider calls.

### 6.5 Potential documentation/source uncertainty

`ThemeKey` documentation says ingress paths intern repeated keys through a shared style atom table (`style.rs:574-579`). Within the assigned source, `From<&str>`, `From<String>` and `From<Arc<str>>` directly construct an `Arc<str>` (`style.rs:596-610`); no style-atom interning table is defined in these files.

This may be implemented in an uninspected ingress layer, or the comment may describe an intended optimization not visible in this scope. The source evidence here proves `Arc` sharing for cloned keys, but does not prove global/intern-table deduplication for independently constructed equal keys.

---

## 7. Tests, benchmarks and observability

### 7.1 Tests in assigned files

#### `api/style.rs`

Tests at `style.rs:1087-1180` cover:

- explicit versus unspecified text attributes;
- sparse style overlay preserving unspecified fields;
- false-valued attribute overrides;
- primitive constructors;
- border constructors and replacement colors.

These protect semantic style contracts rather than terminal rendering.

#### `api/text.rs`

Tests at `text.rs:254-327` cover:

- short text stays inline;
- page spans share storage;
- dropping the page owner does not invalidate retained views;
- out-of-bounds ranges return `None`;
- UTF-8-splitting ranges return `None`;
- newline-containing ranges remain valid.

#### `api/grid.rs`

Tests at `grid.rs:250-513` cover:

- row-major placement;
- row/column spans;
- implicit tracks;
- no overlap;
- source-order retention;
- component identity propagation.

#### `ir.rs`

Tests at `ir.rs:2236-2621` cover:

- outer-Arc clone identity;
- fresh identity after mutation;
- semantic equality independent of identity;
- child identity retention after parent changes;
- persistent sequence structural sharing;
- aggregate component flags;
- weak-handle lifetime;
- one-root native patch construction;
- attachment preservation;
- final grid construction;
- batched versus sequential retained replacements;
- duplicate state attachment rejection.

#### `wrap.rs`

Tests at `wrap.rs:552-839` cover:

- input whitespace attachment;
- hard-newline behavior;
- termwiz widths;
- no-wrap cursor movement;
- word wrapping;
- long-word grapheme breaks;
- emoji/keycap/ZWJ/flag clusters;
- combining marks across style spans;
- oversized graphemes and `fits`;
- no-wrap overflow behavior.

### 7.2 Observability counters

The assigned source references these counters:

- `Counter::ViewCloneCalls`
- `Counter::ViewNodesConstructedRust`
- `Counter::PersistentSeqNodesAllocated`
- `Counter::PersistentSeqBranchClones`
- `Counter::PersistentSeqLeafClones`
- `Counter::TextFlowMeasureCalls`

The counter usage demonstrates that performance concerns are not merely comments:

- root construction count is observable;
- cloning count is observable;
- persistent sequence structural allocation is observable;
- text measurement calls are observable.

The final-grid test explicitly checks that one final grid root is constructed rather than one root per cell (`ir.rs:2535-2541`).

No benchmark was run. Benchmark definitions and broader counter interpretation are outside this assignment.

### 7.3 Behavioral contracts not directly executable here

The source establishes, but this investigation did not execute:

- native binding operation behavior;
- ABI serialization and native handle lifetime;
- provider ticket validation;
- scene integration with actual mounted component/content registries;
- layout/paint cache invalidation behavior;
- History acceptance and source-frontier progression.

Those are covered by neighboring source and tests, but not runtime-validated in this report.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Generic presentation versus application content

`presentation/content.rs` is deliberately generic, while `application/content.rs` owns source/port/connector/projection lifecycle. The seam is clean at the type level:

```text
presentation:
    port/connector IDs
    offered width
    projection ticket
    measurements
    row windows
    physical destination

application/content:
    source bytes
    connector selection
    projection preparation
    delivery frontier
    source-rooted revisions
    history row production
```

The presentation layer therefore knows enough to select/cache/paint a prepared product without owning application content semantics.

### 8.2 Semantic IR versus physical output

The `ir.rs` module is backend-neutral by construction. It stores:

- semantic text and style;
- layout rules;
- child topology;
- content/component/state identities;
- semantic decorations.

However, the adjacent `wrap.rs` product is no longer fully backend-neutral:

- it stores `PhysicalStyle`;
- it calls `grapheme_cell_width`;
- it uses termwiz-compatible cell width semantics;
- it computes physical row widths.

This is an intentional seam: semantic `TextView` remains independent of physical terminal cells, while wrapping is the point at which terminal grapheme width becomes necessary.

### 8.3 Style API and state-plane coupling

The presentation IR stores `StyleStates` and `StyleFacts`; paint resolves them against focus/theme state. The state bags are generic and caller-owned, but the retained-state attachment path independently uses `retained_state::presentation_state_capable`.

This produces two distinct state concepts that must not be conflated:

1. `StyleStates` / `StyleFacts`: semantic style-resolution context.
2. `state_attachment: Option<u64>`: native retained state identity used for state-plane binding.

Both occur on `ViewNode`, but they have different ownership and lifecycle.

### 8.4 Content attachment is not a content tree

A `ContentHost` contains only a port identity and semantic node metadata. It does not embed:

- source text;
- projection output;
- connector;
- content rows;
- history unit;
- scheduling state.

The provider and host resolve those at measurement/paint/history-transfer time. This is consequential for retained identity: replacing or decorating a content-host `View` changes the semantic root, but the content source itself remains externally owned.

### 8.5 Component slots are not child occurrences

A `ComponentSlotNode` contains only `ComponentId`. The concrete component `View` is resolved by scene/component infrastructure. A component slot has no independently addressable retained presentation state according to the `native_state_capable` documentation (`ir.rs:880-887`); the concrete resolved component owns presentation state instead.

This differs from ordinary structural children, which are recursively present in the semantic IR and whose identities/attachments can be traversed directly.

### 8.6 One-root construction and modifier construction coexist

The source has two valid construction styles:

- direct final construction for native/bulk ingress;
- immutable modifier construction for ordinary semantic updates.

The code and tests distinguish them rather than pretending all construction is identical:

- `factory::native_patched` assembles one `ViewNodeParts` and allocates one root;
- regular `padding`, `style`, `fill_width`, etc. each create a new semantic root;
- `ViewNodeParts::from_view` preserves attachments for either route.

This is a consequential current invariant for native performance and identity publication.

### 8.7 Historical-document contradiction/limitation

The historical `PRE-V5-ARCHITECTURE-REPORT.md` treats the current View/presentation architecture as a subject to decompose, while current source documentation already describes the post-L1 one-final-root construction seam and private IR. The current source is authoritative for this report.

No claim is made here that every historical abstraction has been removed repository-wide; only the assigned current files were evaluated.

### 8.8 Framework boundary observations

No product-specific agent/application terms were found in the assigned presentation implementation. The content dirty reasons, style state keys, theme keys, stream/content tickets and history methods are generic or caller-supplied.

The generic presentation layer does expose mechanisms that could carry application-specific values—opaque port IDs, style keys, component IDs—but it does not interpret them as product concepts in this scope.

---

## 9. Open questions and coverage gaps

1. **Theme-key interning**
   - `ThemeKey` comments describe shared interning, but the assigned source only proves `Arc<str>` ownership and cloning.
   - The actual ingress path/table, if any, needs source verification outside `presentation/api/style.rs`.

2. **Native ABI ownership**
   - The binding wrappers are visible, but generated ABI schema, N-API handle retention and TypeScript-side native handle ownership were not part of this assignment.
   - The source here shows the Rust operation boundaries, not the full wire-level allocation/copy profile.

3. **Provider ticket failure behavior**
   - `PreparedProjectionTicket` documents identity validation and fail-closed behavior, but concrete rejection/recovery semantics live in `application/content.rs`.
   - The presentation trait alone does not establish whether a stale ticket causes repaint, remeasure, skip or frame retry.

4. **Full route reachability**
   - The assigned factories are heavily referenced by components, scene, content rendering, History and tests.
   - A complete production reachability audit for every factory function belongs to the repository-wide route assignment.

5. **Layout/paint interpretation**
   - The IR is designed for layout/paint consumers, but detailed handling of `ClampRows`, `RowViewport`, borders, background and style cascading belongs to excluded directories.
   - This report names the semantic records and call seams without duplicating their implementation.

6. **Content-host placement identity**
   - The content attachment is a numeric port ID on the semantic occurrence, while `PreparedProjectionTicket` additionally carries optional connector and projection identities.
   - The exact relationship between repeated identical `ContentHost` occurrences and host-side occurrence indexing requires scene/paint source tracing beyond this report.

7. **Persistent sequence balancing**
   - The implementation preserves sharing and uses a fixed branch factor, but no benchmark was run here to characterize tree height, splice costs or behavior under highly skewed edits.

8. **Rust authoring visibility**
   - Rust `View` and API declarations are publicly declared within hidden/private modules for binding use, while crate-root visibility is `pub(crate)`.
   - The exact external Rust documentation/API surface should be confirmed against the crate’s generated docs/build configuration.

9. **Unexecuted tests**
   - All behavioral claims are source/test evidence only. No tests or benchmarks were run.

---

## 10. Evidence appendix

### 10.1 Assigned production files read

```text
crates/iyon-tui/src/presentation/api/grid.rs
crates/iyon-tui/src/presentation/api/mod.rs
crates/iyon-tui/src/presentation/api/style.rs
crates/iyon-tui/src/presentation/api/text.rs
crates/iyon-tui/src/presentation/api/view.rs
crates/iyon-tui/src/presentation/content.rs
crates/iyon-tui/src/presentation/factory.rs
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/mod.rs
crates/iyon-tui/src/presentation/wrap.rs
```

### 10.2 Supporting files inspected for seams/consumers

```text
AGENTS.md
PRE-V5-ARCHITECTURE-REPORT.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/evidence/assignments.json
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt
crates/iyon-tui/src/lib.rs
crates/iyon-tui/src/binding/mod.rs
crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/content/text/render/mod.rs
crates/iyon-tui/src/scene/layout.rs
crates/iyon-tui/src/scene/resolved.rs
crates/iyon-tui/src/scene/tests.rs
crates/iyon-tui/src/scene/root_tests.rs
crates/iyon-tui/src/scene/host.rs
crates/iyon-tui/src/history/model.rs
crates/iyon-tui/src/history/native/mod.rs
crates/iyon-tui/src/retained_state/capabilities.rs
crates/iyon-tui/src/retained_state/presentation.rs
crates/iyon-tui/src/theme/mod.rs
crates/iyon-tui/src/theme/batch.rs
crates/iyon-tui/src/controls/text_input/tests/presentation.rs
crates/iyon-tui/src/component/tests.rs
crates/iyon-tui/src/component/tick_tests.rs
crates/iyon-tui/src/interaction/tests/mod.rs
```

`presentation/layout/**` and `presentation/paint/**` were indexed and referenced only to establish downstream seams and were excluded from detailed ownership/LOC accounting.

### 10.3 Exact symbol groups

#### Construction and API

```text
presentation::api::view::{NativeCommonPatch, View::new_kind, View::text_from_spans,
                          View::with_text_layout_patch,
                          View::try_with_text_layout_patch}
presentation::api::text::{TextStorage, TextSpan, NativeTextPage, WrapMode,
                          HorizontalAlign}
presentation::api::style::{StyleSpec, StyleRef, StyleSelector, StyleStates,
                           StyleFacts, ColorSpec, ThemeKey, BorderSpec,
                           BorderGlyphs, Insets, OverflowIndicator}
presentation::api::grid::{GridTrack, GridCellSpec, lower_grid_parts}
presentation::factory::{text, styled_text, text_from_spans,
                        text_from_spans_with_style, row, column, grid,
                        hanging, spacer, content_host, native_component,
                        component, native_patched, clamp_rows,
                        row_viewport, style, wrap, cursor_at, padding,
                        background, foreground, border, style_state,
                        style_states, fill_width, fill_height}
```

#### IR and retention

```text
presentation::ir::{ViewId, ViewFlags, PersistentSeq, SequenceAggregate,
                   View, ViewNode, ViewNodeParts, ViewKind, TextView,
                   ColumnView, RowView, GridView, HangingView,
                   ComponentSlotNode, ContainerNode, ContentHost,
                   WidthRule, HeightRule, TrackSize, Decoration,
                   ClampRowsView, RowViewportView, WeakView}
View::{from_node, map_node, map_text, ptr_eq,
       native_with_state_attachment, native_with_content_attachment,
       native_state_attachment_targets, native_axis_from_children,
       native_axis_set_child, native_axis_splice, native_grid_set_cell,
       native_replace_at_path, try_retained_child,
       try_replace_retained_child, try_replace_retained_children}
```

#### Content seam

```text
presentation::content::{ContentDirtyReason, ContentDirty,
                        ContentWindow, PreparedProjectionTicket,
                        ContentMeasurement, HistoryContentRows,
                        ContentProvider, EmptyContentProvider}
```

#### Wrapping

```text
presentation::wrap::{StyledGrapheme, WrappedLine, StyledTextFlow,
                     TextFlowMetrics, styled_hard_lines, text_flow,
                     text_flow_metrics, wrap_styled_lines,
                     wrap_input_styled_lines, input_wrap_ranges}
```

### 10.4 Key source line ranges

```text
presentation/mod.rs:
  1-16       module boundaries
  18-301     hidden native binding wrappers
  303-318    crate-local reexports

presentation/api/view.rs:
  12-33      NativeCommonPatch
  35-75      final/internal View constructors
  77-115     text layout patching

presentation/api/text.rs:
  9-107      TextStorage
  110-156    TextSpan and NativeTextPage
  160-233    TextSpan API/source-page lowering
  236-252    WrapMode/HorizontalAlign
  254-327    tests

presentation/api/style.rs:
  19-138     StyleSpec
  140-219    Insets
  220-271    ANSI/style key/value primitives
  307-396    StyleAssignments/StyleStates/StyleFacts
  440-533    StyleSelector
  535-687    colors/theme/style references
  703-798    attributes
  800-1068   border semantics
  1071-1085  OverflowIndicator
  1087-1180  tests

presentation/api/grid.rs:
  19-118     tracks/cell specs
  121-176    grid lowering
  179-247    retained grid construction/placement
  250-513    tests

presentation/factory.rs:
  20-161     text constructors
  164-297    row/column/grid constructors
  303-393    hanging/spacer/content/component/container
  396-510    native patch/clamp/viewport
  513-628    modifiers

presentation/content.rs:
  21-69      dirty reasons/items
  72-120     window/ticket/measurement
  137-220    history/content provider contract
  223-252    empty provider

presentation/ir.rs:
  30-80      ViewId/ViewFlags
  82-685     PersistentSeq
  687-790    View/ViewNode construction
  817-998    state/content attachments and target traversal
  1012-1039  immutable node/text mapping
  1042-1277  native axis/grid/path edits
  1279-1683  retained path types and traversal
  1686-1872  text path patch and view-kind tags
  1880-1957  flags/equality/WeakView
  1959-2234  semantic IR type declarations
  2236-2621  tests

presentation/wrap.rs:
  14-50      grapheme/line products
  61-173     hard-line tokenization
  175-263    text flow and metrics
  266-400    wrapping kernels
  403-488    input wrapping/source ranges
  490-550    cursor placement
  552-839    tests
```

### 10.5 LOC methodology

- Counts are approximate physical source-line counts from inspected line-number ranges.
- Documentation/comments, cfg-only native sections and embedded tests are included in physical ranges.
- Embedded tests were separated by `#[cfg(test)]` module start where practical.
- No generated files are included.
- No test or benchmark was executed, so the report makes no pass/fail claim.