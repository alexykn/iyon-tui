# 36 — layout-resize

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `36 / traces / layout-resize`
- Scope: TypeScript layout authoring through Rust measurement, placement, retained geometry, resize, damage, painting, and resolved terminal cells.

The investigation follows the atlas report contract and the assignment manifest:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

The current source is treated as authoritative for executing behavior. Historical or architectural comments are reported as documentation evidence only; this report does not perform V5 disposition or migration analysis.

### Scope boundaries

Included:

1. TypeScript public layout authoring:
   - `View.horizontal`
   - `View.vertical`
   - `View.grid`
   - `ChildrenBuilder`
   - `GridBuilder`
   - `GridRowBuilder`
   - layout child track declarations
   - grid tracks, rows, cells, spans, and alignment
   - immutable semantic node construction
   - retained composition equality and wide-sequence sidecars

2. TypeScript-to-native structural materialization:
   - semantic axis/grid encoding
   - `ensureNative`
   - retained identity and derivation paths
   - native ABI construction of rows, columns, and grids

3. Rust layout:
   - constraints
   - measurement
   - intrinsic sizing
   - row/column/grid allocation
   - prepare-time bounded height allocation
   - placement
   - clipping
   - retained layout tree indexes
   - component geometry and dependency metadata

4. Retained geometry/state:
   - width/height mode changes
   - padding
   - bounds
   - gap
   - alignment
   - border edges
   - state-driven layout invalidation and local relayout

5. Runtime resize:
   - TypeScript runtime resize
   - native host resize
   - resize-triggered invalidation and full layout behavior
   - backend presenter resize behavior

6. Damage/output:
   - layout geometry damage
   - retained-state damage
   - incremental repaint
   - full repaint
   - row-based physical lowering
   - text/content-to-cell output
   - surface and terminal presenter boundaries

Excluded except where necessary to prove a seam:

- semantic text parsing and projection internals
- History model details unrelated to layout geometry
- general input routing
- V5 design conclusions
- unrelated application/product concepts

### Evidence status

This is a read-only static inspection report. No repository files were edited. No dependencies were installed. No test or benchmark command was executed in this investigation, so all test observations below are source-based behavioral evidence rather than run results.

Facts, inferences, and unknowns are distinguished:

- **Fact**: directly represented by an inspected type, function, call path, or test.
- **Inference**: behavior reconstructed from several source paths.
- **Unknown**: not provable from the inspected source or not exercised here.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Area | Path | Approximate physical LOC | Responsibility | Public surface |
|---|---|---:|---|---|
| TypeScript public View API | `packages/iyon-tui/src/api/view/view.ts` | ~1,149 | Immutable `View` authoring, row/column/grid builders, track declarations, scalar layout modifiers, semantic derivations, wide sequence sidecars | Yes, intentional package export through `index.ts` |
| TypeScript semantic model | `packages/iyon-tui/src/api/view/semantic-node.ts` | ~612 | Backend-neutral immutable semantic node vocabulary and sidecars | Mostly private/internal |
| TypeScript composition | `packages/iyon-tui/src/composition/compose.ts` | ~878 | Retained slot equality, semantic View reuse, builder execution, retained axis/grid construction | Internal composition machinery |
| TypeScript geometry values | `packages/iyon-tui/src/api/view/geometry.ts` | ~41 | Public `Insets` and normalization | Yes |
| TypeScript retained state API | `packages/iyon-tui/src/api/view/retained-state.ts` | ~300+ | Public mutable state handle, geometry and presentation patch entrypoints | Yes |
| TypeScript structural encoding | `packages/iyon-tui/src/transport/structural/encoding.ts` | ~350 | Semantic layout track/grid/span/alignment conversion into ABI words | Internal transport |
| TypeScript retained materialization | `packages/iyon-tui/src/transport/structural/retained-dag.ts` | ~1,400+ | Semantic node → native retained View construction, identity fast paths, axis/grid buffers, stale-ref recovery | Internal transport |
| Rust layout facade | `crates/iyon-tui/src/presentation/layout/mod.rs` | ~247 | Pipeline contract, compiler facade, final physical row block | Private crate API |
| Rust layout orchestration | `crates/iyon-tui/src/presentation/layout/engine.rs` | ~185 | Measure → prepare → place orchestration and root tree construction | Private |
| Rust measurement | `crates/iyon-tui/src/presentation/layout/measure.rs` | ~828 | Width-dependent semantic measurement and intrinsic sizing | Private |
| Rust preparation | `crates/iyon-tui/src/presentation/layout/prepare.rs` | ~402 | Bounded height allocation and prepared child positions | Private |
| Rust row/column allocator | `crates/iyon-tui/src/presentation/layout/tracks.rs` | ~108 | One-dimensional fixed/content/flex/flex-max allocation | Private |
| Rust grid allocator | `crates/iyon-tui/src/presentation/layout/grid.rs` | ~461 including unit tests | Span-aware two-dimensional track allocation | Private |
| Rust placement | `crates/iyon-tui/src/presentation/layout/place.rs` | ~225 | Prepared geometry → flat `LayoutTree` nodes | Private |
| Rust retained layout tree | `crates/iyon-tui/src/presentation/layout/tree.rs` | ~821 | Rectangles, clips, content records, parent/index maps, dependency metadata, local patches | Private |
| Rust scene layout bridge | `crates/iyon-tui/src/scene/layout.rs` | ~190 | Resolved scene → layout tree; component geometry and layout callbacks | Private |
| Rust scene host | `crates/iyon-tui/src/scene/host.rs` | ~3,500+ | Convergence, retained candidate handling, incremental relayout, damage, painting | Private |
| Rust painting | `crates/iyon-tui/src/presentation/paint/view.rs` | ~2,000+ | Layout tree → `Surface`/`PhysicalRow`, text/content lowering, paint cache, incremental subtree paint | Private |
| Rust state geometry | `crates/iyon-tui/src/retained_state/geometry.rs` | ~334 | Sparse geometry overrides and effect classification | Typed portions exported |
| Rust state records | `crates/iyon-tui/src/retained_state/record.rs` | ~180+ | Revisioned host-owned state and geometry/presentation snapshots | Private |
| Rust damage | `crates/iyon-tui/src/retained_state/damage.rs` | ~103 | Damage rectangle clipping, merging, and full-damage threshold | Private |
| Native resize wrapper | `crates/iyon-tui-native/src/tui.rs` | relevant range ~1008–1017 | N-API resize validation and forwarding | N-API |
| Rust application host resize | `crates/iyon-tui/src/application/host.rs` | relevant range ~1391–1403 | Host dimensions, frame invalidation, render attempt | Native host API |

LOC values are approximate source-line counts based on file extents and inspected manifests, not executable-code-only counts. Layout test LOC is reported separately below.

### 1.2 Pipeline ownership

The current implementation has a clean conceptual split:

```text
TypeScript caller
    │
    │ View.horizontal / vertical / grid
    ▼
Immutable TS semantic View node
    │
    │ retained composition may reuse exact node
    │ or materialization may encode it
    ▼
Native retained Rust View
    │
    │ resolved Scene overlay supplies component/state/content values
    ▼
Rust measure
    │  width-dependent MeasuredNode
    ▼
Rust prepare
    │  bounded-height PreparedNode
    ▼
Rust place
    │  flat LayoutTree with absolute rects/clips
    ▼
Rust paint
    │  text/content/decorations → Surface or PhysicalRow
    ▼
Damage-aware host/presenter
    │
    ▼
Terminal cells and terminal output
```

The TypeScript side authoritatively declares semantic structure. The Rust side authoritatively resolves geometry and produces physical cells. TypeScript does not receive resolved rectangles or cells during ordinary rendering.

### 1.3 Production versus tests

Relevant layout tests:

- `crates/iyon-tui/src/presentation/layout/tests/flow.rs` — ~520 lines
- `crates/iyon-tui/src/presentation/layout/tests/grid.rs` — ~584 lines
- `crates/iyon-tui/src/presentation/layout/tests/mod.rs` — ~1,185 lines
- `crates/iyon-tui/src/presentation/layout/tests/style.rs`
- `crates/iyon-tui/src/presentation/layout/tests/text.rs`

The test suite covers:

- width and height parity
- zero-width and zero-height behavior
- row/column gaps
- fixed/content/flex/flex-max tracks
- grid spans and alignment
- nested grids and rows
- padding and borders
- text wrapping and wide grapheme handling
- row-based lowering parity with full-surface painting
- retained layout cache behavior
- component/state-local relayout
- layout counters and pruning

Relevant TypeScript tests include:

- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `packages/iyon-tui/tests/tui_harness.test.ts`

---

## 2. Types, APIs and contracts

### 2.1 TypeScript row and column authoring

The public type declarations are in `packages/iyon-tui/src/api/view/view.ts:106–144`.

```ts
type LayoutChild =
  | { kind: "normal"; child: View }
  | { kind: "fixed"; size: number; child: View }
  | { kind: "flex"; child: View }
  | { kind: "flexMax"; maxRows: number; child: View }
  | { kind: "contentMax"; maxRows: number; child: View };
```

The same track vocabulary is used for both horizontal and vertical axes. The name `maxRows` is retained even when a `LayoutChild` is used inside a horizontal row; the Rust representation later treats the value as a generic track cap.

Public row/column construction:

- `View.horizontal(children)` → semantic kind `row`
- `View.vertical(children)` → semantic kind `column`
- `ChildrenBuilder.child(view)` → normal/content track
- `ChildrenBuilder.fixed(size, view)`
- `ChildrenBuilder.flex(view)`
- `ChildrenBuilder.flexMax(maxRows, view)`
- `ChildrenBuilder.contentMax(maxRows, view)`
- `ChildrenBuilder.gap(value)`

Relevant implementation:

- `view.ts:166–218` — builders
- `view.ts:326–347` — `View.horizontal` and `View.vertical`
- `view.ts:1120–1131` — conversion to semantic layout children
- `compose.ts:528–588` — retained composition path and equality

Validation is eager at the TypeScript authoring boundary:

- `validateU16` accepts integers in `0..=65535`.
- `validatePositiveU16` accepts integers in `1..=65535`.
- `fixed`, `flexMax`, and `contentMax` validate their values.
- Gap values are validated.
- The semantic node is frozen after normalization.

A normal child is encoded as a content track. An explicit `fixed` child preserves the fixed size. `flex` has an implicit minimum of one in the ABI encoding. `flexMax` and `contentMax` preserve their cap.

### 2.2 TypeScript grid authoring

Public grid types are in `view.ts:117–142`:

```ts
type GridTrack =
  | { kind: "content" }
  | { kind: "contentMax"; max: number }
  | { kind: "fixed"; size: number }
  | { kind: "flex" }
  | { kind: "flexMax"; max: number };

interface GridCell {
  view: View;
  columnSpan?: number;
  rowSpan?: number;
  horizontalAlign?: HorizontalAlign;
  verticalAlign?: VerticalAlign;
}

interface GridRow {
  track?: GridTrack;
  cells: readonly GridCell[];
}

interface GridSpec {
  columns?: readonly GridTrack[];
  rows: readonly GridRow[];
  columnGap?: number;
  rowGap?: number;
}
```

Construction supports three forms:

1. `View.grid(readonly View[])`
2. `View.grid(GridSpec)`
3. `View.grid((builder: GridBuilder) => void)`

`GridBuilder` and `GridRowBuilder` are at `view.ts:166–193`.

Important normalization behavior:

- Array shorthand creates one row.
- Each array item becomes a cell.
- Every shorthand column receives a `content` track.
- Default row track is `content`.
- Default `columnSpan` and `rowSpan` are `1`.
- Default cell alignment is horizontal `start`, vertical `top`.
- Grid gaps default to zero.
- Explicit spans use `validatePositiveU16`; zero spans are rejected.
- Grid tracks and gap values are validated as u16 values.

`gridViewFromBuilder` at `view.ts:638–658` creates the immutable semantic grid node and seeds a wide-grid sidecar when necessary.

### 2.3 Grid auto-placement

There is no public explicit `(row, column)` coordinate on `GridCell`. Cells are auto-placed in source row order by `gridPlacement` at `view.ts:834–871`.

The algorithm:

1. Maintains `occupiedUntil[column]`.
2. Starts each cell at column zero.
3. Extends the occupied array to fit the requested span.
4. Searches for the first contiguous range not occupied by prior row-spanning cells.
5. Records the cell's starting column.
6. Marks all columns in the span as occupied through `rowIndex + rowSpan`.
7. Records row offsets and source sequence indexes.

This placement map serves retained cell replacement and wide-grid sequence indexing. The Rust layout allocator receives the flattened semantic cells and their spans, then performs the actual geometry allocation.

### 2.4 Semantic node vocabulary

`packages/iyon-tui/src/api/view/semantic-node.ts:96–132` defines the backend-neutral layout records:

- `SemanticLayoutChild`
- `SemanticAxisTrack`
- `SemanticGridTrack`
- `SemanticGridCell`
- `SemanticGridRow`

The semantic node kinds are private numeric discriminants at `semantic-node.ts:164–178`:

- text
- diff
- spacer
- row
- column
- grid
- hanging
- container
- clamp
- contentMax
- component
- decorated
- contentHost

The semantic node does not contain resolved rectangles, native refs, palette values, or cells. It retains:

- semantic identity
- semantic child relationships
- layout declarations
- decoration and style values
- state/content attachment IDs
- derivation hints
- optional lazy sequence sidecars

`createSemanticViewNode` at `semantic-node.ts:286–293` assigns a fresh semantic ID and recursively freezes semantic payloads. The TypeScript `View` wrapper is separately frozen by `view.ts:560–570` and `view.ts:997–1006`.

### 2.5 Retained composition contracts

Inside a retained execution scope, public layout operations route through `compose.ts`.

The contract documented at `compose.ts:1–22` is:

1. No active scope → ordinary construction.
2. Active scope → consume the next semantic slot.
3. Compare normalized raw arguments against the previous committed View.
4. Return the exact previous View on equality.
5. Construct a new immutable View on difference.
6. Compare children by semantic-node identity.
7. Avoid generic tree scans and reflection on the hot path.

For row/column:

- The builder executes first so child View calls consume their own slots.
- The parent then compares kind, gap, child count, child identity, track kind, and track scalar.
- Wide sequence-backed axes intentionally do not flatten to prove equality; `axisMatches` returns false if a sequence sidecar exists (`compose.ts:561–588`).

For grids:

- `composeGrid` first normalizes the public specification into a `GridBuilder`.
- It compares gaps, column count, row count, track values, cell counts, child identities, spans, and alignments.
- Wide grids intentionally bypass equality scanning if a grid sidecar exists (`compose.ts:666–714`).

This means retained composition avoids allocating a replacement semantic View for ordinary unchanged narrow row/column/grid declarations, but wide structures use a different path to avoid flattening.

### 2.6 Immutable layout modifiers

The `View` fluent methods at `view.ts:461–475` provide:

- `fitWidth`
- `fillWidth`
- `fitHeight`
- `fillHeight`
- `minWidth`
- `maxWidth`
- `minHeight`
- `maxHeight`
- `wrap`
- `textAlign`

Geometry decorations are accumulated into a `SemanticDecoration` record. `commonScalarDerivation` at `view.ts:786–817` recognizes only scalar layout changes:

- padding
- width mode
- height mode
- min/max width
- min/max height

It deliberately refuses to produce a scalar derivation when the decoration also contains colors, borders, non-empty style, or style-state data. This avoids treating mixed presentation/layout changes as a narrower retained patch than the semantics justify.

Text layout patches use a `textLayout` derivation (`view.ts:511–556`), retaining the original text spans while changing wrapping or alignment.

### 2.7 State-owned retained geometry

The public TypeScript `ViewState` API is in `packages/iyon-tui/src/api/view/retained-state.ts`.

Geometry fields (`retained-state.ts:39–85`):

- width: `"fit" | "fill"`
- height: `"fit" | "fill"`
- padding
- minWidth / maxWidth
- minHeight / maxHeight
- gap
- alignment
- borderEdges

`View.state(state)` only attaches the opaque state handle ID to the semantic occurrence (`view.ts:408–414`). State mutation does not rebuild the semantic View topology.

The native state control API packs normalized patches into a mask envelope:

- `geometryEnvelope`
- `geometryClearEnvelope`
- `setGeometry`
- `clearGeometry`

Validation occurs in `transport/state/control.ts:39–174`:

- unknown properties rejected
- width and height restricted to fit/fill
- insets normalized and range-checked
- bounds allow explicit null to clear the effective bound
- gap range-checked
- alignment must specify at least one axis
- border edges normalized

Rust receives a typed `ViewStateGeometryPatch` at `retained_state/geometry.rs:12–25`.

### 2.8 Rust semantic layout IR

Rust’s retained semantic IR is in `crates/iyon-tui/src/presentation/ir.rs`.

Relevant types:

- `ViewKind` (`ir.rs:1959–1977`)
- `WidthRule` (`ir.rs:1985–1991`)
- `HeightRule` (`ir.rs:1993–1999`)
- `ColumnView` and `ColumnChild` (`ir.rs:2026–2067`)
- `RowView` and `RowChild` (`ir.rs:2069–2119`)
- `GridView` (`ir.rs:2121–2129`)
- `TrackSize` (`ir.rs:2150–2157`)
- `ViewBounds` (`ir.rs:2194–2199`)
- `RowViewportView` (`ir.rs:2219–2226`)

Rust `TrackSize` is:

```rust
enum TrackSize {
    Content { max: Option<u16> },
    Fixed(u16),
    Flex { min: u16 },
    FlexMax { min: u16, max: u16 },
}
```

Rust `ViewNode` stores:

- `ViewId`
- aggregate attachment flags
- state/content attachment IDs
- width/height rules
- decoration
- style states/facts
- `ViewKind`

Rust semantic nodes are immutable `Arc`-backed values. A changed parent can retain unchanged child `View` identity, as tested in `ir.rs` around the semantic retention tests.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
TS public View API
  ├── semantic-node.ts
  ├── geometry.ts
  ├── semantic-style.ts
  └── composition/compose.ts when an execution scope is active
          │
          ▼
TS immutable semantic View node
          │
          ├── transport/structural/encoding.ts
          ├── transport/structural/retained-dag.ts
          └── native ABI generated calls
                    │
                    ▼
          Rust retained View / ViewNode
                    │
                    ▼
          Scene resolution overlay
          ├── Component snapshots
          ├── ViewState snapshots
          └── Content measurement/projection
                    │
                    ▼
          presentation/layout/engine.rs
          ├── measure.rs
          ├── prepare.rs
          └── place.rs
                    │
                    ▼
          presentation/layout/tree.rs
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
  scene/layout.rs       presentation/paint/view.rs
  component geometry    Surface / PhysicalRow
  callbacks             cells and styles
          │                   │
          └─────────┬─────────┘
                    ▼
             scene/host.rs
             damage / incremental paint
                    │
                    ▼
             terminal presenter/backend
```

### 3.2 Object ownership

| Object | Created by | Retained by | Destroyed/released by |
|---|---|---|---|
| TypeScript `View` | `View` API or retained compose helper | caller, composition slot, semantic child references | JavaScript reachability/GC |
| TS semantic node | `createSemanticViewNode` or lazy sidecar constructor | `View` and semantic child references | weak sidecars disappear with node |
| TS wide sequence | `PersistentSeq.from` in `seedWideAxisSequence` / `seedWideGridSequence` | semantic sidecar weakly associated with node | node/sidecar reachability |
| Native retained View ref | `ensureNative` / generated ABI constructor | native retained root and child graph | retained root boundary release |
| Rust `View` | native ABI decode/materializer | `Arc` values in Rust semantic IR | `Arc` ownership and root replacement |
| `MeasuredNode` | Rust `measure_node` | two-generation `LayoutCache` | cache rotation/clear/invalidation |
| `PreparedNode` | Rust `prepare_node` | two-generation `LayoutCache` | cache rotation/clear/invalidation |
| `LayoutTree` | `layout_resolved_scene_with_cache_and_content` | current retained `StableScene` | replacement/discard |
| `Surface` | `ViewPainter` | current host surface, paint cache, prepared frame | next frame/cache rotation |
| `PhysicalRow` | row-based painting | prepared frame or content/history consumers | frame replacement |
| ViewState geometry | native state record | host registry and state overlays | state disposal/clear |

### 3.3 Identity and lifetime

The TS semantic `NodeId` and Rust `ViewId` are distinct identity systems.

TypeScript:

- `view.ts:145–158` uses a process-wide monotonic safe-integer counter.
- `viewNodeId` and `viewNodeIdHighWater` are exposed internally for retained transport.
- Node identity is not semantic equality and is not itself a native resource lease.

Native transport:

- `retained-dag.ts:52–59` stores generation-scoped semantic-to-native hints in a `WeakMap`.
- `MaterializeTx` (`retained-dag.ts:213–229`) tracks transaction-local refs and temporary leases.
- `ensureNative` first checks a generation-matching hint, then a transaction-local ref, then attempts NodeId promotion only for IDs below the captured native lookup ceiling.
- A newly created semantic NodeId skips the native promotion probe and is directly materialized.

Rust:

- Rust `ViewId` is a process-local cache/retention key.
- `LayoutNodeId` is a per-layout-tree vector index, not semantic identity.
- `LayoutTree` indexes state attachments, content ports, component roots, and parent links separately from semantic identity.

### 3.4 Layout tree ownership

`LayoutTree` is explicitly ephemeral and rebuilt for placement:

- `layout/cache.rs:1–6` states that measurement/preparation facts are retained, but the layout tree is rebuilt because placement depends on parent origin and clip.
- `LayoutTree` does not recursively clone semantic View subtrees.
- `LayoutNode` holds the semantic `ViewId`, resolved rectangles, style facts, content metadata, and child `LayoutNodeId`s.

This is a consequential boundary: geometry is retained at the host level through `StableScene.layout`, but the actual `LayoutTree` is a derived candidate product rather than the authoritative semantic structure.

### 3.5 Dependency metadata

`LayoutNode.child_dependencies` (`tree.rs:100–115`) is parallel to `children`. Each `ChildDependency` records whether:

- parent uses child width
- parent uses child height
- child width depends on parent
- child height depends on parent

`place.rs:114–150` assigns dependency metadata by measured kind:

- containers, clamps, hanging nodes, and grids conservatively use `ChildDependency::all`
- columns infer width/height dependency from track type
- rows conservatively use all dependencies because row measurement probes child intrinsic widths even for eventual fill tracks
- row viewports distinguish intrinsic content height from fixed visible height
- leaves have no child dependencies

The metadata enables retained state/content invalidation to stop at a proven allocation boundary rather than re-inferring layout rules from node kinds.

---

## 4. Execution paths and state transitions

### 4.1 TypeScript authoring to native retained structure

#### Ordinary construction path

```text
View.horizontal(...)
    ├── buildChildren(...)
    ├── semanticLayoutChildren(...)
    ├── createSemanticViewNode(kind=row)
    └── optional wide-axis sidecar
```

`View.vertical` is identical except for the semantic kind.

```text
View.grid(...)
    ├── gridBuilderFromSpecification(...)
    ├── normalize rows/tracks/cells/spans/alignment
    ├── createSemanticViewNode(kind=grid)
    └── optional wide-grid sidecar
```

No native call occurs in these API functions. The result is a frozen semantic TypeScript `View`.

#### Retained composition path

Inside `defineView`/retained execution:

```text
View.horizontal(builder)
    ↓
composeHorizontal
    ↓
composeAxisImpl
    ↓
execute child builder first
    ↓
consume parent semantic slot
    ↓
axisMatches(previous, entries, gap)
    ├── equal → stageReuse(previous)
    └── different → composedAxis(...)
```

The exact same pattern applies to `View.vertical` and `View.grid`, with `gridBuilderMatches` for grids.

A retained no-op returns the previous `View` object exactly, preserving its semantic identity and allowing downstream native identity reuse.

#### Structural materialization path

For a row or column:

```text
ensureNative(semanticAxisNode, MaterializeTx)
    ↓
ensureSemanticNative
    ├── generation hint
    ├── transaction-local ref
    ├── NodeId → NativeRef promotion
    └── direct semantic materialization
          ├── child semantic refs
          ├── track words
          └── row/column ABI constructor
```

`retained-dag.ts:515–565` handles axis materialization.

- Up to four children use fixed-arity ABI constructors.
- More than four children use a reusable `Uint32Array` scratch buffer.
- Each child contributes a track word and native child ref.
- Native retains no pointer to the scratch buffer after the synchronous call.

For grids, `retained-dag.ts:578–620` packs:

```text
[column_count, column_track_words...,
 row_count,
 per row: row_track_word, cell_count,
 per cell: child_ref, span_word, alignment_word]
```

The buffer includes all columns, rows, and flattened cells. `gridCellSpanWord` packs column and row spans into two u16 halves. `gridCellAlignmentWord` packs horizontal and vertical alignment similarly (`encoding.ts:288–298`).

### 4.2 Rust root layout path

`scene/layout.rs:39–54` is the scene-level layout seam:

```text
ResolvedScene
    ↓
layout_view_with_overlay_and_cache_and_content(
    scene.view,
    LayoutConstraints::bounded(size),
    scene.overlay,
    cache,
    content
)
    ↓
LayoutTree
    ↓
tree.component_geometry()
    ↓
ResolvedSceneLayout { tree, components }
```

`presentation/layout/engine.rs:84–142` orchestrates the three stages:

1. Determine width:
   - use definite constraint width if present
   - otherwise measure at `u16::MAX`

2. Measure the root with semantic width intent.

3. Prepare with optional definite height.

4. Build root clip.

5. Emit `PreparedNode`s into a flat vector through `emit_prepared`.

6. Index component roots, state roots, content roots, parents, and child-Y ordering.

7. For unbounded height, set tree height to the emitted root rectangle height.

The module-level contract at `layout/mod.rs:6–15` explicitly separates:

- measurement into width-dependent `MeasuredNode`
- bounded allocation into `PreparedNode`
- placement into `LayoutTree`

Placement must not remeasure, and `LayoutTree` must not retain recursive semantic subtree clones.

### 4.3 Measurement state

`MeasuredNode` (`measure.rs:92–112`) retains:

- source `View`
- `MeasureKey`
- cacheability
- component identity/scope
- width capacity
- width/height rules
- base/effective gap
- base/effective alignment
- decoration metrics and effective decoration
- effective style states
- total and core size
- measured kind

`MeasuredKind` contains the per-layout-kind measured facts:

- `Text` with `TextFlowMetrics`
- `Spacer`
- `ContentHost` with `ContentMeasurement`
- `Container`
- `Column`
- `Row` with `TrackAllocation`
- `Hanging`
- `ClampRows`
- `RowViewport`
- `Grid` with column allocation, row track declarations, intrinsic row allocation, and measured cells

### 4.4 Effective geometry calculation

At `measure.rs:264–370`, Rust combines:

1. immutable View base fields
2. state overlay snapshot, if attached
3. effective decoration
4. effective style-state values

For a state-attached node:

```text
state.effective_geometry(
    base width,
    base height,
    base decoration,
    base gap,
    base alignment
)
```

The result determines:

- effective width mode
- effective height mode
- effective bounds
- effective padding
- effective border edges
- effective gap
- effective alignment

Decoration metrics then calculate:

- border widths/heights
- left/right/top/bottom padding
- total horizontal/vertical decoration
- inner width

The available width is clamped by the effective maximum bound. The node kind is measured using the resulting inner width.

For width:

- `ForceFit` or `WidthRule::Fit` uses intrinsic core width.
- `WidthRule::Fill` uses the available inner width.
- Outer width is then expanded by decoration and clamped to min/max capacity.

For height:

- intrinsic core height is measured first.
- height is later bounded during preparation.
- outer height includes vertical decoration and effective height bounds.

### 4.5 Row measurement

`measure.rs:559–612`:

1. Collects each child’s declared track.
2. Calls `allocate_tracks(width, gap, tracks, ...)`.
3. Allocation probes each child with `WidthIntent::ForceFit` and the remaining width.
4. Re-measures every child using its final allocated track width and semantic width intent.
5. Stores the resulting `TrackAllocation` and measured children.

This is intentionally a two-step process. The first pass determines parent-owned track widths; the second pass computes child geometry under those widths.

### 4.6 Column measurement

`measure.rs:531–556` measures every child under the parent width. Column track allocation is primarily a height-time concern, so each child is measured for width and intrinsic height before preparation.

The intrinsic column size:

- width = maximum child width
- height = sum of track-specific intrinsic heights plus inter-child gaps

`track_intrinsic_height` (`measure.rs:806–812`) applies:

- fixed track → fixed height
- content track → child height, optionally capped
- flex track → child height for intrinsic sizing
- flex-max → child height capped by max

### 4.7 Grid measurement

`measure.rs:615–697`:

1. For every cell, measures the cell View with `ForceFit`.
2. Builds column `SpanRequirement`s from preferred cell widths.
3. Allocates columns with `FlexMode::Fill`.
4. Computes each cell’s allocated column span extent.
5. Re-measures each cell under that span width.
6. Builds row span requirements from measured cell heights.
7. Allocates intrinsic rows with `FlexMode::Intrinsic`.
8. Stores the column allocation, row declarations, intrinsic row allocation, and measured cells.

Column and row gaps are selected as follows:

- geometry state gap override, if present
- otherwise grid’s declared column/row gap

The current implementation uses one effective gap override for both grid axes when a state gap override exists (`measure.rs:643–644`).

### 4.8 Preparation and placement

`prepare.rs:49–121` creates or retrieves a `PreparedNode` keyed by:

```text
MeasureKey + optional height bound
```

Preparation computes:

- height capacity
- minimum core height
- requested core height
- prepared child kind
- final node size
- content offsets
- completeness

#### Column preparation

`prepare.rs:213–248`:

- allocates column tracks against requested core height
- prepares each child against its allocated track height
- accumulates child `y` positions
- marks incomplete if an unclamped child exceeds its allocated track
- retains inter-child gap

#### Row preparation

`prepare.rs:250–288`:

- computes row height
- places children left-to-right using measured track widths
- computes vertical alignment offset:
  - top = zero
  - center = extra height / 2
  - bottom = all extra height
- prepares each child with the row height
- retains x positions and gap

#### Grid preparation

`prepare.rs:341–400`:

- allocates rows with `FlexMode::Fill`
- computes each cell’s area x/y, width, and height
- prepares each cell against its area height
- marks incomplete when an unclamped cell exceeds its area
- applies horizontal and vertical cell alignment from remaining area space
- returns used grid width/height

#### Placement

`place.rs:14–111` converts the prepared tree into a flat `LayoutTree`.

Each `LayoutNode` receives:

- semantic/paint View identity
- `OccurrenceBox`
- absolute `rect`
- `content_rect`
- `clip_rect`
- component identity
- child IDs
- dependency metadata
- style facts and decoration
- content record

The node clip is the inherited clip intersected with the node rectangle. Content origin includes border and padding offsets. A `RowViewport` uses a special child clip with unrestricted vertical coordinates so source rows can be translated into the viewport.

### 4.9 Layout tree indexes

`LayoutTree::index_component_roots` (`tree.rs:253–269`) builds:

- component root map
- state attachment root map
- content port root map
- parent links
- child-Y-sorted flags

The tree also supports:

- state snapshot application (`tree.rs:304–317`)
- path-to-root lookup (`tree.rs:319–328`)
- content dependency frontier (`tree.rs:330–345`)
- content repaint roots (`tree.rs:348–371`)
- content measurement refresh (`tree.rs:374–410`)
- incremental paint geometry through viewport translation (`tree.rs:413–455`)
- topology-preserving subtree patching (`tree.rs:458–572`)
- component geometry calculation (`tree.rs:603–617`)
- local component geometry refresh (`tree.rs:639–711`)
- structural validation (`tree.rs:756–821`)

### 4.10 Scene-host convergence

`scene/host.rs:1167–1326` performs up to `MAX_LAYOUT_PASSES = 8`.

Each pass:

1. Captures the current content dirty epoch.
2. Attempts an incremental path unless forced full.
3. Otherwise begins a layout cache epoch and performs full scene resolution/layout.
4. Determines whether host synchronization can be incremental.
5. Delivers component layout callbacks.
6. If callbacks mark layout dirty, discards the candidate and forces a full next pass.
7. If content mutates synchronously during preparation, discards the candidate and forces a fresh pass.
8. Updates host graph/capabilities/focus indexes.
9. Clears invalidation records only after a stable candidate.
10. Returns the stable candidate or `DidNotConverge`.

This makes layout callback mutation and synchronous content mutation explicit convergence events rather than silently accepting stale geometry.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production path matrix

| Semantic operation | Primary production path | Alternate path | Selection condition | Failure behavior |
|---|---|---|---|---|
| `View.horizontal` outside retained execution | `View.horizontal` → semantic row node | None | No execution scope | Type/range errors from builder values |
| `View.horizontal` inside retained execution | `composeHorizontal` → slot compare → reuse or `composedAxis` | Wide-axis reconstruction | Active scope; wide sidecar causes equality bailout | Composition errors propagate |
| `View.vertical` | Same as horizontal, semantic column | Wide-axis reconstruction | Same | Same |
| `View.grid` outside retained execution | `rawGrid` → `gridBuilderFromSpecification` → semantic grid node | None | No execution scope | Invalid spans/tracks/gaps reject |
| `View.grid` inside retained execution | `composeGrid` → normalized builder comparison → reuse or rebuild | Wide-grid reconstruction | Active scope; wide sidecar causes equality bailout | Invalid input rejects before materialization |
| Axis native materialization | `ensureNative` → `materializeAxisNode` → fixed-arity or buffer ABI constructor | NodeId promotion / generation hint | Existing semantic/native identity available | Retained refusal; no secondary transport |
| Grid native materialization | `ensureNative` → `materializeGridNode` → reusable words buffer | Existing native hint/promotion | Same | Explicit retained refusal or ABI status failure |
| Row/column measurement | `measure_node` → `measure_row`/`measure_column` → `allocate_tracks` | Layout cache hit | Cacheable View and matching key | Internal invariant/panic or explicit content error |
| Grid measurement | `measure_node` → `measure_grid` → `allocate_grid_tracks` | Layout cache hit | Cacheable View and matching key | Internal invariant/panic or explicit content error |
| Height allocation | `prepare_node` → `prepare_kind` | Prepare cache hit | Matching `MeasureKey + height_bound` | Candidate rejected or incomplete |
| Placement | `emit_prepared` | None | Always after preparation | Debug validation detects invalid tree |
| Full paint | `paint_tree_with_content` → `paint_node` | Row-based output for row-oriented callers | Host requires full surface | Physical incomplete flag retained |
| Row paint | `paint_tree_rows_with_content` → `paint_row_node` | Full-surface paint | Host/content asks for row output | Missing/empty row yields blank row |
| State geometry mutation | TS envelope → native state record → state overlay → local/root relayout | Full cache clear when state path unavailable | State is attached and valid for target kind | Native validation rejects unsupported geometry |
| Content update | content dirty frontier → local content refresh | Full root layout | Stable allocation/path versus metric escape | Candidate retained only after successful preparation |
| Resize | TS `Tui.resize` → N-API host resize → Rust `TuiHost::resize` → invalidate frame → render | Terminal resize event path | Explicit API or backend event | Invalid size or backend/render error |
| Layout geometry damage | `layout_geometry_damage` | Full damage | Same tree shape and node count versus changed shape | Full viewport damage |
| State/content incremental repaint | `DamageRegion::from_rects` plus targeted subtree paint | Full tree paint | Retained surface and safe repaint roots available | Falls back to full paint if any target cannot be painted |

### 5.2 Retained materialization refusal

`retained-dag.ts:185–198` defines `RetainedRefusalError`.

The source explicitly says a retained refusal is not a route selector:

- it is not permission to choose a previous transport
- no compatibility materializer is selected
- callers receive an explicit preparation failure

Native constructor status errors are converted to retained refusal errors after one targeted stale-child recovery attempt (`retained-dag.ts:415–467`).

This is important for the layout boundary: a TypeScript semantic row/grid that cannot be materialized does not silently bypass the retained architecture.

### 5.3 Layout failure

`SceneHost` maps layout convergence failure to an explicit preparation error:

- `SceneHostError::DidNotConverge`
- native frame error code `LAYOUT_DID_NOT_CONVERGE`
- marked non-retryable by the host adapter (`application/host.rs` around frame preparation error mapping)

A candidate layout does not become authoritative until it has been resolved, synchronized, painted, and accepted by the surrounding frame protocol.

### 5.4 Shape-changing local updates

`ResolvedSceneLayout::patch_component_with_cache` (`scene/layout.rs:57–97`) only patches a component subtree if:

1. the component root exists
2. its content child exists
3. replacement layout has the same outer size
4. `LayoutTree::patch_component_subtree` accepts the topology/shape
5. component geometry indexes can be refreshed

If the replacement size changes, the function returns false and the caller performs an authoritative broader layout pass.

This prevents a local component update from corrupting parent allocation or sibling positions.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Layout cache

`LayoutCache` (`presentation/layout/cache.rs:37–158`) has four maps:

- current measurement
- previous measurement
- current preparation
- previous preparation

The cache is two-generation and rotates once per host frame, not between convergence passes (`cache.rs:37–41`).

Keys:

```text
MeasureKey:
  view
  component_view
  geometry_revision
  presentation_revision
  content_revision
  width
  intent

PrepareKey:
  measured MeasureKey
  height_bound
```

Consequences:

- Width and width intent distinguish semantic measurement from forced fit probes.
- State geometry and presentation revisions participate in measurement keys.
- Content layout-input revision participates only for `ContentHost`.
- A candidate’s repeated convergence passes share the same working cache epoch.
- Placement itself is not cached.

Invalidation:

- `invalidate_view_ids` drops only affected semantic View IDs and component replacement products.
- `invalidate_content_entries` drops content-dependent entries while preserving ordinary siblings.
- `clear` drops all generations.
- Current/previous entries are promoted on use.

### 6.2 Paint cache

`PaintCache` (`presentation/paint/view.rs:162–221`) also has two generations and is theme-aware.

`PaintKey` includes:

- semantic View ID
- node rect
- content rect
- clip rect
- inherited and resolved physical style
- node and descendant style context
- text alignment/width rule
- content paint revision
- content metric revision
- border/background/glyph fingerprint

This prevents stale physical surfaces when:

- text alignment changes without a semantic View ID change
- content projection changes while its rectangle remains constant
- content measurement revision changes while allocation happens to match
- border glyphs/background change without a resolved text-style change
- inherited style context changes

Theme changes clear both cache generations (`view.rs:189–194`).

### 6.3 Measurement and preparation work

The expected hot-path cost is:

- TypeScript retained composition: compare only immediate normalized parent fields and child identities.
- Native materialization: identity hint or NodeId promotion where eligible; otherwise inspect semantic node and traverse children.
- Rust measurement: recursive over cache misses and affected dependency frontiers.
- Rust preparation: recursive over cache misses and changed height bounds.
- Placement: rebuilds the derived tree each layout pass.
- Painting: either full recursive tree paint, row-oriented paint, or targeted component/content/state subtree paint.

Counters observed in source:

- `MeasureNodeCalls`
- `PrepareNodeCalls`
- `LayoutNodesEmitted`
- `PaintNodesVisited`
- `PaintCellsAllocated`
- `ComponentGeometryNodesVisited`
- `ViewStateGeometryRelayouts`
- `ViewStateDirtyPropagationNodes`
- `ViewStateIncrementalPaints`
- `ViewStateDamageRects`
- `ViewStateFullDamageRepaints`

`layout/mod.rs:61–95` additionally provides test-only semantic stage counters:

- measured nodes
- prepared nodes
- emitted nodes

### 6.4 Wide structure handling

TypeScript thresholds:

- `WIDE_AXIS_SEQUENCE_THRESHOLD = 1_024`
- `WIDE_GRID_SEQUENCE_THRESHOLD = 1_024`

For wide axes:

- `PersistentSeq` stores the authoritative child sequence.
- A lazy `children` getter materializes a flat array only when directly requested.
- Retained composition intentionally refuses to flatten wide children for equality.
- Axis replacement and splice derivations can update only the changed sequence segment.
- Native materialization traverses the persistent sequence directly.

For wide grids:

- cells are flattened into one persistent sequence
- row offsets, row tracks, and cell-index maps are retained as sidecar metadata
- the lazy `rows` getter reconstructs ordinary row records only when accessed
- grid cell replacement updates one persistent sequence item instead of copying all row arrays

The sidecar is an optimization representation; the semantic row/cell vocabulary remains unchanged.

### 6.5 State invalidation

`retained_state/geometry.rs:280–334` classifies geometry properties.

All geometry changes include:

- geometry invalidation
- subtree repaint
- old and new damage

Additional effects:

| Property group | Additional effects |
|---|---|
| width, padding, min/max width, border edges | content projection, self measurement, ancestor measurement, self placement, descendant placement, clip update, intrinsic width and height |
| height, min/max height | self measurement, ancestor measurement, self placement, descendant placement, clip update, intrinsic height |
| gap | self measurement, ancestor measurement, descendant placement, intrinsic width and height |
| alignment | self placement and descendant placement; no intrinsic measurement effect |

Width changes conservatively invalidate intrinsic height because width can alter text wrapping.

The local state refresh path in `scene/host.rs:1542–1692`:

1. invalidates state-dependent layout and paint cache entries
2. applies candidate state snapshots
3. attempts fixed-allocation local geometry refresh
4. if geometry may escape its allocation, propagates dirty View IDs to root
5. relayouts the resolved scene with targeted caches
6. computes whether physical geometry changed
7. uses targeted repaint if rectangles are unchanged
8. otherwise uses full repaint

### 6.6 Content invalidation

Content is tracked separately from semantic structure.

`SceneHost` retains:

- content dirty records
- dirty epoch
- prepared content epoch
- affected content ports
- retained content dependency paths
- content repaint roots

`try_local_content_refresh` (`scene/host.rs:487–619`) tries to relayout only the affected ContentHost path. If the content metric revision changes but its parent allocation remains stable, the host can patch the leaf and repaint it. If the metric change escapes the fixed allocation boundary, the refresh climbs to the relevant parent dependency or falls back to a broader layout candidate.

### 6.7 Scheduling

TypeScript `Tui.resize` calls `prepareMutation`, which:

1. ensures the runtime is open
2. drains pending retained execution
3. rejects mutation during a retained protocol pass

Then it calls the native host resize synchronously.

Tracked state writes use the retained runtime wake broker and are drained on a microtask. Explicit render additionally drains pending work before returning, so caller-visible output is coherent.

Rust host layout convergence is bounded at eight passes. A layout callback that changes component state causes a fresh authoritative pass rather than a second local patch over a half-synchronized candidate.

---

## 7. Tests, benchmarks and observability

### 7.1 Rust layout behavioral contracts

#### Flow tests

`presentation/layout/tests/flow.rs` validates:

- row continuation indentation and track width
- no overflow at widths `0..=4`
- container physical identity
- zero-width/height preservation
- fit versus fill width
- fit versus fill child allocation in fixed tracks
- intrinsic spacer width zero with retained height
- empty flow gap behavior
- one-child gap omission
- gaps counted between all semantic children
- background/padding/border geometry
- clamping and overflow indicator behavior

Representative evidence:

- `flow.rs:4–8` — row continuation behavior
- `flow.rs:11–28` — narrow-width no-overflow invariant
- `flow.rs:62–109` — fit/fill allocation
- `flow.rs:230–259` — zero-width spacer vertical extent
- `flow.rs:263–322` — empty flows and one-child gaps
- `flow.rs:456–483` — fixed track preserves child sizing intent
- `flow.rs:509–519` — clamp overflow indicator

#### Grid tests

`presentation/layout/tests/grid.rs` validates:

- shared columns align across rows
- fixed/content/flex consume width
- wrapped cells contribute row height
- fit cells retain intrinsic width
- fill cells use their allocated area
- horizontal and vertical cell alignment
- span areas include internal gaps
- spanning cells grow content columns/rows
- nested grid measurement
- grid-inside-row and row-inside-grid composition
- bounds and decoration use inner width
- style-state inheritance into cells
- capped row-span allocation and remainder redistribution

Representative evidence:

- `grid.rs:36–77` — shared-column alignment
- `grid.rs:80–118` — fixed/content/flex allocation
- `grid.rs:120–143` — wrapping affects row height
- `grid.rs:146–173` — fit versus fill cell width
- `grid.rs:176–245` — horizontal/vertical alignment
- `grid.rs:248–294` — span area gap inclusion
- `grid.rs:297–335` — spanning content growth
- `grid.rs:339–449` — nested grid/row combinations
- `grid.rs:484–531` — bounds and style-state behavior
- `grid.rs:563–582` — row-span remainder allocation

#### Pipeline and cache tests

`presentation/layout/tests/mod.rs` validates:

- measurement result equals width-only layout result
- measurement/preparation/emission stage counters
- row-paint lowering matches full-surface painting
- row pruning avoids allocating disjoint children
- warm measurement/preparation cache reuse
- unaffected shared-path reuse after a change
- two-generation cache retention and rotation
- flex-max intrinsic height caps
- flex redistribution
- bounded vertical and horizontal allocation
- alignment with bounded height
- clamping and wide grapheme safety
- width/height bound behavior

Important evidence:

- `mod.rs:112–174` — measurement/layout parity and stage counters
- `mod.rs:178–364` — row lowering parity
- `mod.rs:374–399` — row child pruning
- `mod.rs:407–550` — layout cache retention and invalidation
- `mod.rs:800–905` — bounded track allocation and alignment
- `mod.rs:940–1,025` — grapheme clipping and bounds

### 7.2 TypeScript authoring and transport tests

`packages/iyon-tui/tests/tui_h3_a_semantic.test.ts` verifies:

- every current View family is represented in semantic nodes
- layout fields survive semantic conversion
- row/column child track declarations are preserved
- grid tracks, rows, cells, spans, and alignment survive conversion
- semantic node freezing
- semantic derivation variants
- wide axis/grid sidecars
- axis-set, axis-splice, and grid-cell derivation metadata

`packages/iyon-tui/tests/tui_h3_b_composition.test.ts` verifies retained slot reuse and composition behavior.

`packages/iyon-tui/tests/tui_native_builder.test.ts` verifies:

- native compact axis construction
- fixed and flex builder entries
- semantic text retained on the same generated route
- parity between native builder output and retained rendering

`packages/iyon-tui/tests/tui_native_scalar.test.ts` verifies:

- scalar View changes through generated FFI
- text layout patches through retained paths
- nested row/column path edits
- generated retained route parity

### 7.3 Resize and output observability

TypeScript testing helpers expose:

- `resize(width, height)`
- `screenRows()`
- `nativeHistoryRows()`
- `styleAt(row, column)`
- `cellXOfText(row, text)`

`packages/iyon-tui/src/testing/index.ts:27–31, 76–77, 115–126` defines these observability surfaces.

`Tui.resize` source path:

- `packages/iyon-tui/src/runtime/runtime.ts:716–728`
- `crates/iyon-tui-native/src/tui.rs:1008–1017`
- `crates/iyon-tui/src/application/host.rs:1391–1403`

Output is observable both as logical rows and as terminal-cell properties. The harness can verify Unicode cell addressing and style resolution, not only plain strings.

### 7.4 Benchmark/instrumentation status

No benchmark was executed here.

The source exposes counters for:

- layout stage calls
- paint traversal/allocation
- component geometry visits
- state geometry relayout
- damage rectangle count
- incremental paint
- full damage repaint
- content metric evaluations
- content paint propagation

TypeScript retained transport additionally exposes counters in `retained-dag.ts:111–155` for:

- semantic/native hint hits/misses
- NodeId promotion
- semantic node inspection
- child visits
- direct materializer calls
- derivation fast paths
- ref words written
- payload bytes
- scratch reuse
- stale-ref recovery

These counters are observability evidence, not proof that every route is exercised in production.

---

## 8. Cross-boundary findings and contradictions

### 8.1 TypeScript declares; Rust resolves

The most important seam is that TypeScript layout authoring stops at semantic structure:

```text
TS View.horizontal / vertical / grid
    → semantic node with tracks, gaps, spans, alignment
    → native retained View
    → Rust Scene resolution
    → Rust measure/prepare/place
    → Rust physical cells
```

There is no TypeScript resolved rectangle or cell model in the inspected production API. TypeScript tests inspect semantic nodes and native output, but geometry ownership remains Rust-side.

### 8.2 Two View models are intentional but easy to confuse

There are:

1. TypeScript `View` and `SemanticViewNode`
2. Rust `presentation::ir::View` and `ViewNode`

They are not the same type and do not share object identity. The transport layer translates between them through generated ABI calls and native retained objects.

This is not a duplicate layout algorithm. The TypeScript model authorizes structure; Rust owns measurement and physical layout. It is nevertheless a migration/maintenance hazard because similar names represent different lifetime and identity domains.

### 8.3 Public TS layout versus internal Rust factory layout

Rust still contains factory helpers such as:

- `presentation/factory.rs:183–297`
- `row_specs`
- `column_specs`
- `grid`
- `fill_width`
- `fit_width`
- `padding`
- related constructors

These are `pub(crate)` framework internals, not public native Rust authoring APIs. The public authoring path is TypeScript. Rust factories remain heavily used by Rust tests and internal framework code.

The source therefore contains two authoring-oriented construction surfaces at different visibility levels:

- public TypeScript semantic authoring
- private Rust semantic factory construction

The actual runtime layout engine is shared by both.

### 8.4 Track naming differs across boundaries

TypeScript axis children use:

- `normal`
- `fixed`
- `flex`
- `flexMax`
- `contentMax`

Rust uses generic `TrackSize`:

- `Content`
- `Fixed`
- `Flex`
- `FlexMax`

The TypeScript transport encoding deliberately uses separate code lanes:

- axis-create track words (`encoding.ts:65–82`)
- grid track words (`encoding.ts:100–108`)
- axis edit track words where zero means preserve existing track (`encoding.ts:85–98`)

This is a meaningful ABI distinction. Reusing one code table across axis construction and axis editing would change the meaning of zero.

### 8.5 Grid cell replacement uses logical start-column identity

`gridSetCellForTransport` accepts `(row, column)` and resolves the cell through `cellIndices`. For spanning cells, the map records the cell at its auto-placed starting column. The API does not expose a coordinate for every covered column.

That is consistent with the source’s auto-placement model, but callers must address the cell’s starting column rather than any interior covered column.

### 8.6 Layout cache and paint cache intentionally differ

The layout cache is keyed by semantic layout inputs:

- View identity
- width
- intent
- state revisions
- content metric revision
- height bound for preparation

The paint cache additionally includes:

- physical rectangles and clips
- inherited/resolved styles
- style contexts
- text layout key
- content paint/metric revisions
- box fingerprints

This distinction is necessary because a physical surface can be stale even when layout geometry is unchanged.

### 8.7 Placement is not retained as a reusable recursive semantic product

The source explicitly states that:

- measurement and preparation facts are retained
- the layout tree is rebuilt each pass
- placement depends on origin and clip
- the layout tree must not clone semantic subtrees

This means geometry is retained in the currently committed `StableScene`, but placement remains a derived candidate operation. Local patching exists for topology-preserving updates, not as a universal replacement for placement.

### 8.8 Row painting and full-surface painting are alternate lowering modes, not separate layout modes

`ViewPainter` provides:

- full tree → full `Surface`
- full tree → independent `PhysicalRow`s
- source row range → `PhysicalRow`s
- targeted subtree → existing `Surface`

All consume the same prepared `LayoutTree`. The row path avoids allocating a content-sized surface and prunes children by vertical ranges. The source tree and geometry remain authoritative.

### 8.9 Resize enters through multiple seams

There are at least three resize routes:

1. TypeScript explicit API:
   - `Tui.resize`
   - native `NativeTuiHost.resize`
   - Rust `TuiHost::resize`

2. Native terminal backend event:
   - backend updates terminal size
   - emits a resize event
   - application invalidates frame

3. Testing harness:
   - helper delegates to runtime resize

The explicit `Tui.resize` path updates TypeScript’s stored dimensions only after the native host call succeeds. Rust updates headless sink dimensions, invalidates the running frame, synchronizes real time for real terminals, and renders.

### 8.10 Geometry state changes do not mutate semantic topology

State geometry is a sparse host-owned overlay. It changes effective layout inputs without changing:

- semantic child count
- semantic row/column/grid topology
- semantic node identity
- native structural topology

This allows local layout refresh and cache invalidation, but it also means layout cache keys must include state geometry revisions. The source explicitly clears broader caches when parent entries cannot prove that descendant state revisions are represented.

### 8.11 Current implementation is generic

The inspected layout APIs contain generic terminal concepts only:

- row
- column
- grid
- track
- content
- text
- state
- component
- content port
- viewport

No application-specific or Iyon-agent meanings were found in the layout implementation. Caller-supplied values remain caller-owned semantic inputs; the framework owns generic measurement, placement, painting, and scheduling policy.

---

## 9. Open questions and coverage gaps

1. **Exact generated ABI/native Rust grid decoder path**  
   The TypeScript encoder and Rust semantic track types were inspected, but the complete generated/native decode implementation for every grid-track code was not traced line by line. The observable encoding contract is clear from `encoding.ts`, but the complete generated call chain was not independently executed.

2. **Runtime resize behavior with a real terminal backend**  
   Source shows the real backend updates viewport size and the presenter resets its known-surface state, but no live terminal resize was executed here. The real backend’s asynchronous presentation receipt interaction during resize remains unobserved.

3. **Exact post-resize damage supplied to the terminal presenter**  
   Source proves that a size/tree-shape difference causes full `DamageRegion`, and the presenter marks its prior surface unknown on resize. The exact sequence of damage consumption by every real backend mode was not executed.

4. **Content provider metric behavior during resize**  
   The layout pipeline passes width into `ContentProvider::measure`, and content revisions participate in layout keys. A concrete non-test content provider under repeated width changes was not exercised in this investigation.

5. **Grid state gap override semantics**  
   The source uses `geometry.gap` as an effective gap for both grid column and row allocation (`measure.rs:643–644`). It is unclear from the public API documentation whether this shared override is intentional for all grid state consumers or merely the current generic state contract.

6. **Large-track allocation limits beyond u16**  
   Track values, rectangles, gaps, and dimensions are u16-bounded. The allocators use saturating arithmetic and capacity clamps. No executed test was performed for combinations approaching `u16::MAX` across many tracks and gaps.

7. **Potential mismatch between `LayoutTree::child_y_sorted` and overlapping grid cells**  
   The optimization marks children as Y-sorted when adjacent rectangles meet a monotonic condition. Grid cells may overlap vertically through row spans. The source has validation and grid tests, but this particular optimization’s behavior on all overlapping span arrangements was not independently executed.

8. **Direct use of private Rust factories by production callers**  
   Rust factory helpers are clearly used by tests and internal framework code. A complete production call-site census outside the inspected layout/paint paths was not performed.

9. **Full generated code and include manifests**  
   Generated ABI files were indexed and the structural retained DAG was inspected. Generated bodies and native include files were not all read because they are supporting/generated material rather than the primary layout implementation.

10. **No tests or benchmarks were run**  
    All test results in this report are source-level evidence from assertions and test names. Runtime counts, allocations, cache hit rates, and real terminal behavior remain unobserved for this report.

---

## 10. Evidence appendix

### 10.1 Required documents

Read/indexed:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
- `AGENTS.md`

The pre-V5 document was used for context and evidence expectations only. No V5 disposition analysis was performed.

### 10.2 TypeScript files inspected

Primary:

- `packages/iyon-tui/src/api/view/view.ts`
  - `View.horizontal`
  - `View.vertical`
  - `View.grid`
  - `ChildrenBuilder`
  - `GridBuilder`
  - `GridRowBuilder`
  - `gridBuilderFromSpecification`
  - `gridViewFromBuilder`
  - `axisSetChildForTransport`
  - `axisSpliceForTransport`
  - `gridSetCellForTransport`
  - wide sequence helpers
  - layout modifiers
  - semantic identity helpers

- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - semantic layout types
  - semantic node kinds
  - freezing
  - attachment sidecars
  - derivation types
  - sequence sidecars

- `packages/iyon-tui/src/api/view/geometry.ts`
  - `Insets`
  - `InsetsValue`
  - `insets`

- `packages/iyon-tui/src/api/view/retained-state.ts`
  - geometry patch types
  - `ViewState`
  - geometry mutation entrypoints

- `packages/iyon-tui/src/composition/compose.ts`
  - retained axis composition
  - retained grid composition
  - immediate equality checks
  - scalar/layout patch behavior

- `packages/iyon-tui/src/transport/state/control.ts`
  - geometry patch normalization
  - envelope encoding
  - validation

- `packages/iyon-tui/src/transport/structural/encoding.ts`
  - axis track words
  - grid track words
  - span/alignment words

- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - retained identity
  - materialization transaction
  - row/column materialization
  - grid materialization
  - derivation fast paths
  - failure/recovery semantics

- `packages/iyon-tui/src/runtime/runtime.ts`
  - resize
  - render boundary
  - retained runtime setup

- `packages/iyon-tui/src/testing/index.ts`
  - resize and screen/cell inspection helpers

Supporting indexed or partially inspected:

- `packages/iyon-tui/src/index.ts`
- `packages/iyon-tui/src/transport/structural/ir.ts`
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
- `packages/iyon-tui/src/transport/structural/policy.ts`
- `packages/iyon-tui/src/transport/structural/retained-path.ts`

### 10.3 Rust production files inspected

Primary layout:

- `crates/iyon-tui/src/presentation/layout/mod.rs`
- `crates/iyon-tui/src/presentation/layout/cache.rs`
- `crates/iyon-tui/src/presentation/layout/engine.rs`
- `crates/iyon-tui/src/presentation/layout/measure.rs`
- `crates/iyon-tui/src/presentation/layout/prepare.rs`
- `crates/iyon-tui/src/presentation/layout/place.rs`
- `crates/iyon-tui/src/presentation/layout/tracks.rs`
- `crates/iyon-tui/src/presentation/layout/grid.rs`
- `crates/iyon-tui/src/presentation/layout/tree.rs`

Scene and host:

- `crates/iyon-tui/src/scene/layout.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/root.rs` symbol references
- `crates/iyon-tui/src/application/host.rs` resize path

Semantic IR and constraints:

- `crates/iyon-tui/src/presentation/ir.rs`
- `crates/iyon-tui/src/geometry/mod.rs`
- `crates/iyon-tui/src/geometry/constraints.rs`
- `crates/iyon-tui/src/presentation/factory.rs` relevant row/column/grid constructors

Painting and damage:

- `crates/iyon-tui/src/presentation/paint/view.rs`
- `crates/iyon-tui/src/retained_state/damage.rs`
- `crates/iyon-tui/src/retained_state/geometry.rs`
- `crates/iyon-tui/src/retained_state/effects.rs`
- `crates/iyon-tui/src/retained_state/record.rs`
- `crates/iyon-tui/src/retained_state/presentation.rs`
- `crates/iyon-tui/src/retained_state/capabilities.rs`

Native/backend resize:

- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
- `crates/iyon-tui/src/terminal/termwiz/shadow.rs`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
- `crates/iyon-tui/src/terminal/backend.rs`

### 10.4 Rust test files inspected

- `crates/iyon-tui/src/presentation/layout/tests/flow.rs`
- `crates/iyon-tui/src/presentation/layout/tests/grid.rs`
- `crates/iyon-tui/src/presentation/layout/tests/mod.rs`
- `crates/iyon-tui/src/presentation/layout/tests/style.rs`
- `crates/iyon-tui/src/presentation/layout/tests/text.rs`
- relevant tests in `crates/iyon-tui/src/presentation/paint/view.rs`
- relevant tests in `crates/iyon-tui/src/scene/host.rs`
- relevant tests in `crates/iyon-tui/src/scene/tests.rs`
- relevant resize tests in `crates/iyon-tui/src/application/content.rs`
- relevant presenter resize tests in `crates/iyon-tui/src/terminal/termwiz/presenter.rs`

### 10.5 TypeScript test files inspected

- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `packages/iyon-tui/tests/tui_harness.test.ts`
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `packages/iyon-tui/tests/tui_perf13_h.test.ts`
- `packages/iyon-tui/tests/tui_state_envelope.test.ts`
- `packages/iyon-tui/tests/tui_surface_contract.test.ts`

### 10.6 High-value symbol references

- TypeScript public authoring:
  - `View.horizontal` — `packages/iyon-tui/src/api/view/view.ts:326–336`
  - `View.vertical` — `view.ts:338–347`
  - `View.grid` — `view.ts:360–362`
  - `ChildrenBuilder` — `view.ts:196–219`
  - `GridBuilder` — `view.ts:172–193`
  - `gridViewFromBuilder` — `view.ts:638–658`
  - `gridPlacement` — `view.ts:834–871`

- TypeScript retained composition:
  - `composeAxisImpl` — `packages/iyon-tui/src/composition/compose.ts:528–550`
  - `axisMatches` — `compose.ts:561–588`
  - `composeGrid` — `compose.ts:666–684`
  - `gridBuilderMatches` — `compose.ts:687–714`

- TypeScript semantic model:
  - `SemanticLayoutChild` — `semantic-node.ts:100–105`
  - `SemanticGridTrack` — `semantic-node.ts:115–120`
  - `SemanticGridCell` — `semantic-node.ts:122–128`
  - `SemanticGridNode` — `semantic-node.ts:222–228`
  - `SemanticDerivation` — `semantic-node.ts:463–516`
  - sequence overrides — `semantic-node.ts:556–611`

- Structural materialization:
  - `materializeAxisNode` — `retained-dag.ts:515–565`
  - `materializeGridNode` — `retained-dag.ts:578–620`
  - `ensureNative` — `retained-dag.ts:1153–1159`
  - `ensureSemanticNative` — `retained-dag.ts:1163–1344`
  - stale recovery — `retained-dag.ts:415–467`

- Rust layout:
  - `layout_view_with_overlay_and_cache_in_scope_and_content` — `presentation/layout/engine.rs:84–142`
  - `measure_node` — `presentation/layout/measure.rs:191–247`
  - `measure_node_uncached` — `measure.rs:249–371`
  - `measure_row` — `measure.rs:559–612`
  - `measure_grid` — `measure.rs:615–697`
  - `prepare_node` — `presentation/layout/prepare.rs:49–121`
  - `prepare_kind` — `prepare.rs:124–401`
  - `emit_prepared` — `presentation/layout/place.rs:14–111`
  - `LayoutTree::index_component_roots` — `presentation/layout/tree.rs:253–269`
  - `LayoutTree::patch_component_subtree` — `tree.rs:458–513`

- Scene/resize/damage:
  - `layout_resolved_scene_with_cache_and_content` — `scene/layout.rs:39–54`
  - `ResolvedSceneLayout::patch_component_with_cache` — `scene/layout.rs:57–97`
  - `SceneHost::resolve_stable_at_with_anchor` — `scene/host.rs:1167–1326`
  - `SceneHost::resolve_full_stable` — `scene/host.rs:1329–1388`
  - `SceneHost::try_incremental_stable` — `scene/host.rs:1391–2049`
  - `SceneHost::paint_with_content` — `scene/host.rs:2058–2240`
  - `layout_geometry_unchanged` — `scene/host.rs:2310–2322`
  - `layout_geometry_damage` — `scene/host.rs:2324–2344`
  - `DamageRegion::from_rects` — `retained_state/damage.rs:23–59`
  - `Tui.resize` — `packages/iyon-tui/src/runtime/runtime.ts:716–728`
  - native resize wrapper — `crates/iyon-tui-native/src/tui.rs:1008–1017`
  - Rust host resize — `crates/iyon-tui/src/application/host.rs:1391–1403`

- Rust painting/output:
  - `PaintKey` — `presentation/paint/view.rs:52–75`
  - `PaintCache` — `view.rs:162–221`
  - row painting — `view.rs:247–293`
  - row node lowering — `view.rs:344–557`
  - full tree paint — `view.rs:901–1125`
  - incremental component paint — `view.rs:769–800`
  - incremental subtree paint — `view.rs:809–887`
  - row child pruning — `view.rs:1381–1400`

### 10.7 LOC methodology

Approximate counts use physical source-file extents from the inspected manifest and source paths:

- production and test files are separated where paths make that distinction explicit
- generated code is excluded from the primary layout LOC figures
- blank/comment lines are not separately removed
- very large host/paint files are reported as approximate because only layout-relevant sections were required for the cross-boundary trace
- no claim is made that these figures represent compiler-weighted or executable-code LOC

### 10.8 Files indexed but not comprehensively read

The following supporting/generated files were indexed or searched for references but were not comprehensively read because their role was secondary to the layout path:

- generated ABI bodies under `packages/iyon-tui/src/transport/abi/**`
- generated native include files
- complete `crates/iyon-tui/src/presentation/api/**`
- complete content/projection/history implementation outside layout seams
- complete terminal backend implementation outside resize/presenter seams
- all unrelated repository tests and fixtures

No production behavior conclusion in this report depends solely on an indexed-only file where a primary source implementation was available.