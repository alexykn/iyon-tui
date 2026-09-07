# 05 — Layout Geometry

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assigned scope: `crates/iyon-tui/src/presentation/layout/` and `crates/iyon-tui/src/geometry/`
- Assignment goal: general allocator, measurement/placement, invalidation, and allocator assumptions outside the assigned directories.

The repository-level contract and atlas README were read before source inspection:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The pre-V5 report is treated as historical/instructional context only. This report maps current source behavior and does not make V5 disposition decisions.

### Scope boundary

Primary source ownership is limited to:

```text
crates/iyon-tui/src/geometry/
crates/iyon-tui/src/presentation/layout/
```

The following adjacent files were inspected where necessary to prove seams, ownership, invalidation, and consumers:

```text
crates/iyon-tui/src/presentation/content.rs
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/wrap.rs
crates/iyon-tui/src/presentation/paint/view.rs
crates/iyon-tui/src/retained_state/record.rs
crates/iyon-tui/src/scene/layout.rs
crates/iyon-tui/src/scene/host.rs
crates/iyon-tui/src/scene/root.rs
crates/iyon-tui/src/history/projection/mod.rs
crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/perf.rs
```

These adjacent files are not counted as owned production LOC for this assignment.

### Evidence classification

- **Current-source fact:** directly visible in the baseline source.
- **Static inference:** behavior inferred from call structure and type/data flow without executing the code.
- **Unknown:** not established because tests/builds were not run or because the relevant owner lies outside this assignment.
- **Executed validation:** none. No build, test, benchmark, dependency installation, or running-service operation was performed.

---

## 1. Responsibility and structure

### 1.1 Geometry module inventory

`crates/iyon-tui/src/geometry/` is a private crate-internal, backend-neutral value layer.

| Path | Approx. physical LOC | Public surface | Primary responsibility | Secondary responsibility |
|---|---:|---|---|---|
| `geometry/mod.rs` | 11 | `pub(crate)` re-exports only | Private module boundary | Keeps geometry types hidden from external consumers |
| `geometry/constraints.rs` | 38 | `pub(crate)` | Width/height bounded-versus-unbounded constraints | Converts definite axes to optional numeric bounds |
| `geometry/point.rs` | 5 | `pub(crate)` | Two-dimensional origin/offset | Placement coordinate carrier |
| `geometry/rect.rs` | 50 | `pub(crate)` | Rectangle geometry and clipping intersection | Saturating right/bottom calculations; containment-independent clipping |
| `geometry/size.rs` | 10 | `pub(crate)` | Width/height extent | Shared intrinsic and resolved-size carrier |

Approximate geometry production LOC: **114 physical lines**, including blank lines and comments, based on file line ranges.

There are no public external geometry APIs. All types are `pub(crate)` and re-exported only within the Rust crate.

### 1.2 Layout production inventory

| Path | Approx. physical LOC | Primary responsibility | Significant secondary responsibilities |
|---|---:|---|---|
| `presentation/layout/mod.rs` | 247 | Layout compiler facade and pipeline contract | `ViewCompiler`, layout blocks, test counters, paint handoff |
| `presentation/layout/engine.rs` | 185 | Orchestrates measure → prepare → placement | Root constraints, cache/provider wiring, tree indexing/validation |
| `presentation/layout/measure.rs` | 828 | Recursive semantic measurement | Decoration metrics, text/content measurement, component snapshots, rows/columns/grids/viewports |
| `presentation/layout/prepare.rs` | 402 | Height-bounded allocation over measured facts | Vertical/flex allocation, alignment, clamp/viewport preparation, completeness |
| `presentation/layout/place.rs` | 225 | Emits prepared geometry into `LayoutTree` | Absolute origins, clipping, style/content records, dependency metadata |
| `presentation/layout/tree.rs` | 821 | Retained resolved geometry/index structure | Component/state/content indexes, incremental patching, clipping, geometry maps, validation |
| `presentation/layout/tracks.rs` | 108 | One-dimensional track allocation | Fixed/content/flex/flex-max distribution and gap handling |
| `presentation/layout/grid.rs` | 461 | Span-aware two-dimensional track allocation | Content-span growth, intrinsic/fill modes, track offsets/extents |
| `presentation/layout/cache.rs` | 159 | Two-generation measurement/preparation cache | View/content invalidation, epoch rotation, cache-key construction |

Approximate layout production LOC: **3,436 physical lines**.

The production layout directory is therefore substantially larger than the geometry value layer. Most implementation complexity is in semantic measurement, tree retention/indexing, local patching, and specialized row/grid/viewport behavior rather than in the raw `Rect`/`Size` types.

### 1.3 Layout tests and observability

The assigned test directory contains:

```text
presentation/layout/tests/flow.rs
presentation/layout/tests/grid.rs
presentation/layout/tests/mod.rs
presentation/layout/tests/style.rs
presentation/layout/tests/text.rs
```

Approximate physical test LOC is **roughly 3,200–3,400 lines** based on the observed file extents:

- `flow.rs`: approximately 520 lines
- `grid.rs`: approximately 583 lines
- `mod.rs`: approximately 1,200 lines
- `style.rs`: approximately 835 lines
- `text.rs`: small, approximately 100–150 lines

The tests cover:

- row/column track sizing and gaps;
- fit/fill width and height;
- zero-width/zero-height geometry;
- decoration, border, and padding bounds;
- hanging prefixes;
- clamping and physical completeness;
- row viewport clipping;
- grid spans and alignment;
- style propagation through layout/paint;
- direct row-paint lowering parity;
- retained layout cache reuse and epoch rotation;
- optional performance counters and pruning behavior.

### 1.4 Module-level ownership

The layout directory has a clearly staged architecture:

```text
semantic View + ResolutionOverlay + retained state + ContentProvider
                              |
                              v
                         measure.rs
                    MeasuredNode / facts
                              |
                              v
                         prepare.rs
                   PreparedNode / allocation
                              |
                              v
                          place.rs
                 LayoutNode / absolute geometry
                              |
                              v
                          tree.rs
           LayoutTree indexes, patches, geometry queries
                              |
                              v
                     presentation/paint/view.rs
```

`geometry/` supplies only scalar geometric values and constraints. It does not own semantic Views, retained state records, content streams, paint cells, component lifetime, or frame scheduling.

---

## 2. Types, APIs and contracts

### 2.1 Geometry contracts

#### `AxisConstraint`

`geometry/constraints.rs:4-16`

```rust
pub(crate) enum AxisConstraint {
    Definite(u16),
    Unbounded,
}
```

`definite()` returns `Some(value)` only for a bounded axis. There is no minimum/maximum range type at this layer; ranges are represented later through presentation IR bounds and allocation logic.

#### `LayoutConstraints`

`geometry/constraints.rs:18-38`

```rust
pub(crate) struct LayoutConstraints {
    pub(crate) width: AxisConstraint,
    pub(crate) height: AxisConstraint,
}
```

Constructors:

- `width_only(width)` — definite width, unbounded height.
- `bounded(size)` — definite width and height.

This is a very small constraint contract. It does not model percentages, intrinsic/available combinations, margins, aspect ratios, rounding policies, or overflow policies.

#### `Point`

`geometry/point.rs:1-5`

A `u16` origin/offset:

```rust
pub(crate) struct Point {
    pub(crate) x: u16,
    pub(crate) y: u16,
}
```

#### `Size`

`geometry/size.rs:1-10`

A `u16` width/height extent:

```rust
pub(crate) struct Size {
    pub(crate) width: u16,
    pub(crate) height: u16,
}
```

The entire layout system is therefore bounded by terminal-coordinate `u16` dimensions.

#### `Rect`

`geometry/rect.rs:3-50`

Fields:

```rust
x: u16,
y: u16,
width: u16,
height: u16,
```

Important behavior:

- `right()` and `bottom()` use `saturating_add`.
- `size()` converts to `Size`.
- `is_empty()` returns true if either width or height is zero.
- `intersection()` computes a half-open overlap and returns `None` for empty intersections.

The type has no explicit negative coordinate representation. Negative viewport translations are handled elsewhere in `tree.rs` through `SignedRect`, then clamped back to `Rect`.

### 2.2 Semantic layout pipeline

`presentation/layout/mod.rs:1-15` states the core contract:

1. Measure semantic Views into `MeasuredNodes`.
2. Resolve bounded allocation into `PreparedNodes` from measured facts.
3. Place `PreparedNodes` into a `LayoutTree` without re-measuring or allocating.

The most consequential invariant is:

> Placement must not repeat semantic measurement, and `LayoutTree` must not retain recursive clones of semantic View subtrees.

The first part is enforced structurally: `place::emit_prepared()` consumes `PreparedNode`. The second is true of `LayoutTree` itself, but not entirely true of retained measured cache state; see §8.

### 2.3 `MeasuredNode`

`presentation/layout/measure.rs:92-166`

`MeasuredNode` stores:

- the semantic `View` clone;
- `MeasureKey`;
- cacheability;
- component identity and component scope;
- width capacity;
- width and height rules;
- base/effective gap;
- base/effective alignment;
- decoration metrics and effective decoration;
- effective style states;
- resolved outer/core sizes;
- a `MeasuredKind`.

`MeasuredKind` variants:

- `Text`
- `Spacer`
- `ContentHost`
- `Container`
- `Column`
- `Row`
- `Hanging`
- `ClampRows`
- `RowViewport`
- `Grid`

The measured representation includes all semantic data required by preparation and placement. It is not just a numeric size cache.

### 2.4 `PreparedNode`

`presentation/layout/prepare.rs:18-47`

`PreparedNode` stores:

- shared `Arc<MeasuredNode>`;
- resolved outer and core sizes;
- content offsets;
- physical completeness;
- one of:
  - `Leaf`
  - `Children(Vec<PreparedChild>)`
  - `Clamp`
  - `RowViewport`

`PreparedChild` carries a local `(x, y)` offset and an `Arc<PreparedNode>`.

Preparation is explicitly bounded by an optional height bound. It allocates vertical tracks and computes child alignment, but it does not recompute text metrics or semantic widths.

### 2.5 `LayoutNode` and `LayoutTree`

`presentation/layout/tree.rs:100-136`

A `LayoutNode` is the retained resolved occurrence record:

- `view_id`
- paint-cacheability
- retained-state `OccurrenceBox`
- outer `rect`
- `content_rect`
- `clip_rect`
- optional `ComponentId`
- child IDs
- per-child `ChildDependency` metadata
- layout style
- layout content discriminator/data

`LayoutContent` retains only the content needed by painting and incremental content updates:

- text and width rule;
- spacer rows;
- content-host identity/revisions/intrinsic size/completeness;
- children;
- clamp overflow indicator;
- row viewport skip count.

`LayoutTree` owns:

- flat `Vec<LayoutNode>`;
- root ID;
- root size;
- aggregate physical completeness;
- component-root index;
- parent index;
- state-root index;
- content-root index;
- child-Y-order flags.

A tree is created for each layout pass. The retained `LayoutTree` is held by `ResolvedSceneLayout` inside the retained `StableScene` owned by `SceneHost`.

### 2.6 `ChildDependency`

`presentation/layout/tree.rs:51-98`

The four dependency bits are:

```text
PARENT_USES_CHILD_WIDTH
PARENT_USES_CHILD_HEIGHT
CHILD_WIDTH_DEPENDS_ON_PARENT
CHILD_HEIGHT_DEPENDS_ON_PARENT
```

This metadata is used outside the layout directory by `SceneHost` to decide whether a local state/content change can remain inside a fixed allocation or must climb to an ancestor dependency frontier.

The metadata is conservative in several cases:

- containers, clamps, hanging views, and grids use `ChildDependency::all()`;
- row children all use `ChildDependency::all()` because row width measurement probes intrinsic widths even when eventual child tracks are fill;
- columns derive dependency bits from fixed versus non-fixed tracks;
- row viewports use intrinsic-height metadata.

### 2.7 Allocator contracts

#### One-dimensional tracks

`presentation/layout/tracks.rs:5-108`

`TrackAllocation` contains allocated track widths/heights and a normalized gap.

Supported track types from `presentation/ir.rs`:

```text
Content { max: Option<u16> }
Fixed(u16)
Flex { min: u16 }
FlexMax { min: u16, max: u16 }
```

`allocate_tracks()`:

1. clamps aggregate gaps to available space;
2. allocates fixed tracks in source order;
3. measures content tracks using the remaining capacity;
4. allocates flex minimums in source order;
5. distributes remaining capacity in equal rounds;
6. removes saturated `FlexMax` tracks from the active set and redistributes their unused share.

There are no flex weights. All active flex tracks receive equal-share distribution, with integer remainder biased toward earlier active tracks.

#### Grid tracks

`presentation/layout/grid.rs:74-162`

`allocate_grid_tracks()` supports:

- fixed tracks;
- minimum flex tracks;
- content tracks;
- capped flex tracks;
- span requirements;
- intrinsic versus fill mode.

Grid span requirements are sorted by `(span length, start index, original index)` before growth. Content growth is attempted before flex growth. `span_extent()` includes internal gaps.

### 2.8 Intentional internal surfaces versus external APIs

No layout or geometry type in this scope is an external public authoring surface. All are `pub(crate)` or private.

The external-facing authoring types are in presentation IR/factory/API modules outside this assignment. They produce semantic `View` structures consumed by the layout compiler. The layout layer is therefore internal runtime machinery, not an application-facing layout API.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency graph

```text
presentation::View / presentation::ir
        |
        +--> ResolutionOverlay
        |
        +--> retained_state::ViewStateSnapshot / EffectiveGeometry
        |
        +--> presentation::ContentProvider
        |
        v
presentation/layout/measure.rs
        |
        +--> tracks.rs
        +--> grid.rs
        +--> presentation::wrap::text_flow_metrics
        |
        v
MeasuredNode (Arc graph)
        |
        v
presentation/layout/prepare.rs
        |
        +--> tracks.rs
        +--> grid.rs
        |
        v
PreparedNode (Arc graph)
        |
        v
presentation/layout/place.rs
        |
        +--> geometry::{Point, Rect}
        +--> retained_state::OccurrenceBox
        |
        v
LayoutTree / LayoutNode
        |
        +--> scene/layout.rs component geometry synchronization
        +--> scene/host.rs incremental patch/invalidation
        +--> presentation/paint/view.rs physical lowering
        +--> interaction/output consumers through scene geometry
```

### 3.2 Reverse consumers

Major consumers of layout output:

- `scene/layout.rs`
  - creates `ResolvedSceneLayout`;
  - extracts `ComponentGeometryMap`;
  - patches component subtrees;
  - synchronizes component size and content extents.
- `scene/host.rs`
  - owns the long-lived `LayoutCache`;
  - invalidates cache entries;
  - performs local state/content patches;
  - commits or discards retained layout candidates.
- `presentation/paint/view.rs`
  - paints `LayoutTree`;
  - uses node rectangles, clips, content records, styles, and viewport translations.
- `history/projection/mod.rs`
  - invokes standalone layout measurement for History unit heights.
- `application/content.rs`
  - implements `ContentProvider` and supplies `ContentMeasurement`.
- `content/text/render/structured.rs`
  - uses layout trees to derive structural child rectangles for rendered content.
- `perf_bench.rs`
  - invokes layout and cache paths for benchmark scenarios.

### 3.3 Ownership and lifetime

```text
SceneHost
  ├── layout_cache: LayoutCache
  ├── retained: Option<StableScene>
  │     └── ResolvedSceneLayout
  │           ├── LayoutTree
  │           └── ComponentGeometryMap
  └── frame candidate / commit-discard lifecycle

one layout pass
  ├── temporary MeasuredNode Arc graph
  ├── temporary PreparedNode Arc graph
  └── emitted LayoutTree

LayoutCache
  ├── current_measure / previous_measure
  └── current_prepare / previous_prepare
```

`SceneHost` is the runtime owner of cache epochs and retained layout. The layout compiler itself does not own a persistent cache unless the caller supplies one. Convenience functions create temporary caches.

`LayoutTree` owns its flat nodes and indexes. `LayoutNodeId` is an index into the tree’s node vector and is not globally stable across full rebuilds. Local topology-preserving patches deliberately preserve existing node IDs.

### 3.4 Identity

There are several identity domains:

- semantic `ViewId`;
- optional component snapshot `ViewId` in `MeasureKey.component_view`;
- `LayoutNodeId`, local to one `LayoutTree`;
- `ComponentId`;
- state attachment ID;
- content port ID;
- content connector/projection identity.

The layout cache is keyed by semantic View identity plus revisions and width intent. The retained tree is keyed by local node indexes. Component and content indexes bridge semantic/runtime identity to retained geometry.

---

## 4. Execution paths and state transitions

### 4.1 Fresh standalone width-only layout

Typical chain:

```text
ViewCompiler::layout_tree
  -> layout_view
      -> layout_view_with_overlay
          -> new LayoutCache
              -> layout_view_with_overlay_and_cache
                  -> layout_view_with_overlay_and_cache_and_content
                      -> layout_view_with_overlay_and_cache_in_scope_and_content
                          -> measure_node
                          -> prepare_node
                          -> emit_prepared
                          -> LayoutTree::index_component_roots
                          -> LayoutTree::validate
```

`engine.rs:92-142` performs the root sequence:

1. If width is unbounded, measure at `u16::MAX` to derive width.
2. Measure again at the selected width.
3. Prepare using an optional definite height.
4. Emit the prepared tree at origin `(0, 0)`.
5. Build indexes.
6. For unbounded height, derive tree height from the emitted root rectangle.
7. Debug-validate the tree.

For normal convenience calls, the cache is discarded after the call. Warm reuse requires a caller-owned `LayoutCache`.

### 4.2 Measurement

`measure_node()` (`measure.rs:191-247`) first builds a `MeasureKey`.

Key inputs:

```text
ViewId
optional component snapshot ViewId
geometry revision
presentation revision
content revision
offered width
WidthIntent::{Semantic, ForceFit}
```

Cacheability is disabled when the View or its component snapshot contains component identity.

State overlay behavior:

- If the View has a state attachment and the overlay contains a snapshot, geometry and presentation revisions enter the key.
- `EffectiveGeometry` is computed from base View properties plus retained state overrides.
- Effective decoration, gap, alignment, and style states are stored in the measured node.

Kind-specific measurement:

- text uses width-dependent `text_flow_metrics`;
- content host delegates to `ContentProvider::measure`;
- containers recurse;
- hanging views measure a force-fit prefix, body, and continuation prefix;
- columns measure all children at the offered width;
- rows allocate horizontal tracks, first probing force-fit widths and then measuring children at allocated widths;
- grids measure cell preferred widths, allocate columns, remeasure cells at span widths, then derive intrinsic row tracks;
- row viewports measure the child and carry skip/visible/layout-height metadata.

### 4.3 Preparation

`prepare_node()` (`prepare.rs:49-68`) uses:

```text
PrepareKey {
    measured: MeasureKey,
    height_bound: Option<u16>,
}
```

Preparation computes:

- bounded core height;
- requested core height according to `HeightRule`;
- child allocation;
- child positions;
- completeness;
- content offsets.

Examples:

- columns allocate vertical tracks against the requested core height;
- rows use measured horizontal track widths and align children vertically;
- grids allocate row tracks against requested height and align cells within span areas;
- clamps prepare the child without a height bound but expose a clipped outer height;
- row viewports prepare the child using the layout-height bound and expose a skipped visible range;
- hanging views materialize one prefix per output row plus the body at `prefix_width`.

### 4.4 Placement

`place::emit_prepared()` (`place.rs:14-112`) recursively emits a flat tree.

For every node it computes:

- outer `Rect`;
- intersection with inherited clip;
- content origin and content rectangle;
- child clip;
- state occurrence record;
- style record;
- content record;
- child dependency metadata.

The special row-viewport route preserves the child’s unscrolled coordinates and uses a wide vertical child clip:

```rust
Rect::new(inherited_clip.x, 0, inherited_clip.width, u16::MAX)
```

The later geometry traversal applies the viewport’s negative vertical translation.

Placement increments `Counter::LayoutNodesEmitted` and test-only emitted-node counters.

### 4.5 Scene-level full layout

`scene/layout.rs:39-54`:

```text
layout_resolved_scene_with_cache_and_content
  -> layout_view_with_overlay_and_cache_and_content
  -> LayoutTree::component_geometry
  -> ResolvedSceneLayout { tree, components }
```

`scene/root.rs` first measures the body at the terminal width to determine body height and therefore History height. It then resolves/project History and merges root-level History/body views. The merged semantic root is laid out with bounded terminal size.

This means a root frame can involve:

1. standalone body measurement;
2. History projection and unit measurement;
3. merged semantic root construction;
4. final layout-tree measurement/preparation/placement.

### 4.6 Retained cache warm path

`SceneHost` owns one `LayoutCache` (`scene/host.rs:179-228`).

At a frame/layout epoch:

```text
SceneHost
  -> layout_cache.begin_epoch()
  -> layout root / local replacement
  -> cache.measured() or measure_node_uncached()
  -> cache.prepared() or prepare_node_uncached()
  -> emitted tree
```

`LayoutCache::begin_epoch()` swaps current and previous maps, clears the new current maps, and thereby retains at most two working generations. Entries moved from previous to current on use are removed from previous.

The cache intentionally retains semantic measured/prepared products while the emitted `LayoutTree` is rebuilt.

### 4.7 Component replacement/local patch

`scene/layout.rs:57-97` handles a topology-preserving component update:

1. Find the component root.
2. Find its existing child occurrence.
3. Record old child size.
4. Layout replacement under `width_only(old_width)`.
5. Reject if replacement size changes.
6. Call `LayoutTree::patch_component_subtree`.
7. Refresh component geometry indexes.

If any step fails, `SceneHost` falls back to an authoritative full/retained-root layout route.

### 4.8 State geometry refresh

`SceneHost::try_local_geometry_refresh` (`scene/host.rs:641-791`) handles state mutations:

1. Find state attachment root in the committed tree.
2. Derive layout path and semantic path.
3. Invalidate cache entries for the full path.
4. Walk from target upward.
5. If a parent dependency may be affected, perform an unconstrained natural-size probe.
6. If the natural result still fits, build a bounded replacement.
7. Patch the subtree if topology and shape remain compatible.
8. Refresh component geometry.
9. Otherwise climb further or fall back to a full retained-root pass.

This is the main consumer of `ChildDependency`.

### 4.9 Content refresh

`SceneHost::try_local_content_refresh` (`scene/host.rs:487-612`) handles content changes:

1. Find the ContentHost node and semantic path.
2. Measure current content once at the committed width.
3. Detect intrinsic height changes, physical completeness changes, or width changes that affect a parent.
4. If no allocation escape is detected, attempt fixed-allocation patches from the ContentHost upward.
5. Refresh geometry.
6. Repaint only the ContentHost or the nearest viewport ancestor when safe.
7. Escalate to normal dependency-aware root layout if metrics changed.

Content measurement and paint changes are deliberately separated.

### 4.10 Reset/destruction

`SceneHost::clear_retained_views` clears:

- layout cache;
- retained scene;
- last surface;
- invalidation queues;
- content dependency/index state;
- pending damage and candidate state.

`SceneHost::discard_candidate` invokes `invalidate_root`, clears both layout and paint caches, and aborts the content candidate. This protects the last committed frame from failed candidate data.

`LayoutCache::clear()` drops all four internal maps. There is no explicit per-node destructor; ordinary `Arc`/`HashMap` ownership releases measured/prepared products when entries and temporary graphs are dropped.

---

## 5. Alternate routes and failure semantics

| Semantic operation | Production route | Selection condition | Cache/recovery behavior | Failure semantics |
|---|---|---|---|---|
| Width-only layout | `layout_view(..., LayoutConstraints::width_only(w))` | Standalone/compiler or unbounded-height layout | Fresh cache unless caller supplies one | Returns a tree; no `Result` |
| Bounded layout | `layout_view(..., LayoutConstraints::bounded(size))` | Scene frame/tests | Root and child heights are prepared against bounds | Truncation/incompleteness is represented in geometry/completeness |
| Standalone measure | `measure_view_with_overlay_and_cache_and_content` | History unit height and tests | Caller may provide cache; History currently creates a fresh cache per unit-height call | Returns `Size` only |
| ContentHost measurement | `ContentProvider::measure` | `ViewKind::ContentHost` | `content_revision` enters measurement key | Provider failure is generally represented by default/empty measurement at the provider boundary |
| Component slot | `ResolutionOverlay::component(slot.id)` | Component View kind | Component snapshot View ID enters key | Missing overlay snapshot panics in `measure_kind` (`measure.rs:467-480`) |
| Cached measurement | `LayoutCache::measured` | Cacheable View and exact key hit | Current hit or promotion from previous generation | Miss falls through to uncached measurement |
| Uncacheable measurement | `measure_node_uncached` | View/component contains component identity | No retained entry | Repeated recursively |
| Cached preparation | `LayoutCache::prepared` | Cacheable measured node and exact height bound | Current hit or previous-generation promotion | Miss falls through to preparation |
| Local component patch | `patch_component_subtree` | Existing root, same shape and compatible IDs | Preserves old `LayoutNodeId`s | Returns `false`, caller falls back |
| Generic subtree patch | `patch_subtree` | Same preorder shape and compatible occurrence identity | Translates replacement into old origin/clip | Returns `false`, caller escalates |
| Local state refresh | `try_local_geometry_refresh` | Dependency-safe fixed allocation | Invalidates path, tries target then ancestors | Returns `None` to trigger retained-root/full layout |
| Local content refresh | `try_local_content_refresh` | Same allocation and completeness | Reuses geometry and updates content identity | Returns `None` on metric/allocation change |
| Theme invalidation | `SceneHost::invalidate_theme` | Palette/theme revision only | Clears paint cache; invalidates ContentHost entries but preserves ordinary layout products | Forces repaint; does not generally force full semantic layout |
| Candidate failure | `discard_candidate` | Backend preparation/receipt failure | Drops candidate and caches | Last committed frame remains authoritative |

### 5.1 Failure masking and conservative fallback

The layout API has no ordinary error return. It uses:

- panic for missing required component overlay snapshots;
- `expect` for internally generated valid track indexes;
- `debug_assert!` for tree validity;
- `physically_complete: bool` for incomplete physical realization;
- `false`/`None` from patch methods to signal escalation;
- default `ContentMeasurement` for absent or unavailable content provider products.

The absence of `Result` at the layout layer means malformed semantic IR or a broken overlay can fail by panic, whereas dynamic metric incompatibility is handled by fallback/full layout.

### 5.2 Cache miss versus recovery

A cache miss is not a failure. It causes recomputation and insertion.

A local patch rejection is a recovery route: the caller performs a larger layout pass.

A physically incomplete text/content result is not necessarily a layout failure. It can produce a valid geometry tree with incomplete paint/content products. `LayoutBlock.physically_complete` combines tree and paint completeness.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Layout cache retention

`presentation/layout/cache.rs:37-48` contains four maps:

```text
current_measure
previous_measure
current_prepare
previous_prepare
```

The cache is two-generation, not unbounded by frame history. It has no capacity limit other than semantic working-set size.

`begin_epoch()` (`cache.rs:106-111`):

1. swaps current and previous measurement maps;
2. clears the new current measurement map;
3. swaps current and previous preparation maps;
4. clears the new current preparation map.

`measured()`/`prepared()` promote a previous-generation entry into current by removing it from previous and inserting an `Arc` clone into current.

### 6.2 Measurement cache keys

`MeasureKey` (`cache.rs:14-23`) includes:

```text
view: ViewId
component_view: Option<ViewId>
geometry_revision: u64
presentation_revision: u64
content_revision: u64
width: u16
intent: WidthIntent
```

The key does not directly include:

- height bound;
- parent identity;
- component scope;
- theme revision;
- terminal height;
- parent track position.

Height belongs in `PrepareKey`, not `MeasureKey`, because measurement is intended to be width-dependent while preparation is height/allocation-dependent.

### 6.3 Invalidation

`invalidate_view_ids()` removes entries when either:

- `key.view` is in the invalidated set; or
- `key.component_view` is in the invalidated set.

The corresponding prepare key is invalidated through `key.measured.view` and `key.measured.component_view`.

This design relies on callers deriving a complete dependency frontier. `SceneHost` generally supplies state/content path IDs from the committed tree rather than invalidating only the directly changed leaf.

`invalidate_content_entries()` removes entries with nonzero content revisions while retaining ordinary entries. This supports theme invalidation and content-provider product replacement without clearing unrelated semantic layout products.

### 6.4 State invalidation relationship

`retained_state/record.rs:71-163` maintains:

- general revision;
- geometry revision;
- presentation revision.

Geometry mutation increments geometry and general revisions. Presentation/style mutation increments presentation and general revisions.

`measure.rs:216-219` injects geometry and presentation revisions into the measurement key. Thus state presentation changes can cause measured-node cache misses even when numeric geometry is unchanged. This is conservative and ensures effective style/decoration state in the measured record is refreshed.

The host separately decides whether to:

- repaint only;
- locally relayout;
- escalate to parent/root layout.

### 6.5 Content invalidation relationship

`presentation/content.rs:13-45` classifies content dirtiness:

- source input;
- delivery visibility;
- width/measurement;
- presentation;
- viewport;
- selection lifecycle.

Measurement-required reasons invalidate layout cache paths. Presentation/viewport changes are paint-only unless the provider reports metric changes.

`ContentMeasurement` (`content.rs:104-121`) separates:

```text
intrinsic_size
physically_complete
projection_revision
metric_revision
paint_revision
connector_id
projection_identity
```

`LayoutContent::ContentHost` retains these values so painting and incremental updates can distinguish metric changes from paint-only changes.

### 6.6 Per-frame/per-append/per-width work

Static call-flow evidence indicates:

- **Per layout pass:** recursive measurement for cache misses, preparation for cache misses, and placement/emission of the full emitted tree.
- **Per warm frame:** cache epoch rotation plus placement/tree rebuild, even when measured/prepared facts are reused.
- **Per width change:** new width-keyed measurement products; content provider projection/measurement may also be width-keyed.
- **Per content append:** content invalidation path; targeted ContentHost metric probe; local patch if fixed allocation remains valid; otherwise dependency-aware relayout.
- **Per state geometry mutation:** path cache invalidation, local natural-size probe when parent dependencies may be affected, bounded replacement/patch or escalation.
- **Per theme change:** paint cache clear, ContentHost ticket invalidation, and repaint; ordinary layout entries are intended to survive.
- **Per paint row request:** tree traversal/row pruning in paint; layout emission itself is not row-pruned.

### 6.7 Observed counters

The code defines counters in `perf.rs:15-70`, including:

```text
MeasureNodeCalls
TextFlowMeasureCalls
PrepareNodeCalls
LayoutNodesEmitted
PaintNodesVisited
PaintCellsAllocated
PaintCacheHits
PaintCacheMisses
ComponentGeometryNodesVisited
ViewStateGeometryRelayouts
ViewStateGeometryLocalPatches
ViewStateGeometryFullRepaints
ViewStateDirtyPropagationNodes
ContentMetricEvaluations
ContentMetricChanges
ContentPaintPropagations
```

Layout-specific increments:

- `measure.rs:261` — `MeasureNodeCalls`;
- `prepare.rs:76` — `PrepareNodeCalls`;
- `place.rs:20` — `LayoutNodesEmitted`;
- `tree.rs:721` — component geometry nodes visited.

Test-only stage counters are declared in `layout/mod.rs:60-95`.

### 6.8 Performance tests

`presentation/layout/tests/mod.rs` includes feature-gated tests for:

- row paint pruning with 4,096 column children;
- warm measurement/preparation cache reuse;
- unaffected shared-path reuse;
- two-generation old View ID rotation.

The tests assert counter relationships, such as:

```text
prepare count == emitted count
warm TextFlowMeasureCalls == 0
warm PrepareNodeCalls == 0
offscreen paint visits remain small
old cache generations rotate out
```

No performance tests were executed during this investigation.

---

## 7. Tests, benchmarks and observability

### 7.1 Behavioral test contracts

Representative tests read from the assigned directory establish:

- standalone measurement equals width-only layout (`tests/mod.rs:554-663`);
- stage counters correspond to semantic/prepared/emitted nodes (`tests/mod.rs:121-175`);
- row-paint lowering matches full-surface paint (`tests/mod.rs:177-370`);
- flex/flex-max track behavior (`tests/mod.rs:665-711`);
- bounded vertical allocation (`tests/mod.rs:799-849`);
- unbounded flex tracks remain intrinsic (`tests/mod.rs:851-881`);
- fit-row allocation preserves fixed and content semantics (`tests/mod.rs:883-905`);
- alignment uses extra bounded height (`tests/mod.rs:907-923`);
- narrow widths do not exceed available capacity (`tests/flow.rs:11-28`);
- zero-width spacer/container geometry preserves vertical extent (`tests/flow.rs:197-227`, `tests/flow.rs:229-260`);
- grid spans include internal gaps (`tests/grid.rs:247-294`);
- grid alignment uses cell area surplus (`tests/grid.rs:175-245`);
- content/fixed/flex columns consume expected widths (`tests/grid.rs:79-117`);
- style facts remain scoped and do not leak into cells/spans (`tests/grid.rs:504-560`, `tests/style.rs`);
- text wrapping and physical wide-grapheme behavior are preserved (`tests/text.rs` and `tests/mod.rs:939-950`).

### 7.2 Route assertions

The test suite does not merely assert final strings. It also checks route-level behavior:

- warm cache path has no text-flow remeasurement;
- preparation is not repeated on warm cache;
- stage counters demonstrate measure/prepare/emit separation;
- row paint prunes disjoint children before allocating cells;
- cache generations retire old View IDs;
- full-tree paint and direct row lowering produce equivalent physical rows.

### 7.3 Benchmarks

`crates/iyon-tui/src/perf_bench.rs` uses `LayoutCache` and layout routes for timing/counter scenarios. It includes:

- repeated warm layout;
- shared subtree reuse;
- paint-cache interaction;
- layout and paint timing helpers.

The assigned tests also contain an ignored local performance characterization probe (`tests/mod.rs:1032-1186`). It is explicitly ignored and was not run.

### 7.4 Missing observability

There is no production counter for:

- cache map sizes by generation;
- cache hit/miss counts specific to layout measurement/preparation;
- number of local patch attempts versus escalation attempts;
- number of tree nodes emitted for a local patch versus full layout;
- number of measured semantic View clones retained by cache;
- cache memory/`Arc` retention size;
- `ChildDependency`-based early-stop decisions;
- exact allocator rounds or track overflow/slack.

These are consequential if layout memory or invalidation behavior needs diagnosis.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Layout tree is clone-free, but retained measured cache is not

`layout/mod.rs:13-15` says `LayoutTree` must not retain recursive clones of semantic View subtrees. `LayoutTree` itself satisfies this: `LayoutNode` stores `ViewId`, text payloads, content metadata, and styles, but not arbitrary recursive `View` trees.

However, `MeasuredNode` contains `pub(super) view: View` (`measure.rs:92-112`), and each cache entry retains an `Arc<MeasuredNode>`. Measured children also retain `Arc<MeasuredNode>` recursively. Therefore:

```text
LayoutTree: no recursive semantic View clone
LayoutCache: may retain recursive semantic View graphs for current + previous generations
```

This is not necessarily incorrect—the cache requires semantic facts and lifetime ownership—but it is an important distinction in memory and deletion analysis.

### 8.2 Standalone measurement bypasses persistent frame cache ownership

`history/projection/mod.rs:1087-1098` creates a fresh `LayoutCache` inside `view_height()` for each History unit measurement. This means History height measurement does not share the long-lived `SceneHost` layout cache, even though it uses the same `measure_view_with_overlay_and_cache_and_content` route.

The resulting behavior is:

- generic scene layout can reuse retained measured/prepared entries;
- History unit-height measurement may repeatedly allocate/discard local cache state;
- History has a separate unit-layout cache keyed by its own `HistoryUnitLayoutKey`, but that cache is outside this assignment.

This is a current ownership split, not an inferred future design.

### 8.3 Theme invalidation and presentation revisions use different scopes

`SceneHost::invalidate_theme` intentionally keeps ordinary layout entries but invalidates ContentHost-related entries. In contrast, state presentation mutations increment `presentation_revision`, which participates directly in `MeasureKey`.

Consequently:

- global theme changes are treated as paint-only for ordinary layout;
- retained state presentation changes can invalidate measured semantic products for the affected path;
- ContentHost products are additionally invalidated because provider paint products may carry theme-dependent revisions.

The source deliberately distinguishes theme policy from state-attached effective presentation.

### 8.4 Content layout keys rely on provider-supplied revision discipline

`measure.rs:220-227` obtains `content.layout_input_revision(port_id, width)` and places it into `MeasureKey.content_revision`.

The layout layer does not independently hash content bytes or projection products. Correct invalidation depends on `ContentProvider` honoring its revision contract. If a provider changes intrinsic metrics without changing its layout-input revision, stale measured/prepared products could be reused. The provider implementation is therefore an architectural trust boundary.

### 8.5 Component scope is not a direct cache-key field

`MeasuredNode` stores `component_scope`, and the recursive measurement path passes component scope through component slots. `MeasureKey` includes the immediate component snapshot View ID but not `component_scope` itself.

The current source appears to rely on View/component identities being sufficient to distinguish cacheable products. If the same semantic View identity can be measured under different component scopes with different effective presentation or paint ownership, the key may be too weak. The source was not executed with deliberately aliased View IDs, so this remains an assumption/coverage gap rather than a proven defect.

### 8.6 Patch compatibility is intentionally topology-based but has asymmetries

`LayoutTree::patch_subtree` (`tree.rs:458-513`) verifies:

- same preorder count;
- same child counts;
- same dependency-vector lengths;
- same View IDs;
- same component IDs;
- same state attachment IDs;
- same component scope.

It does not compare the actual `ChildDependency` values, only vector lengths. This relies on same semantic shape producing equivalent dependency metadata.

`patch_component_subtree` (`tree.rs:516-572`) performs a less restrictive comparison and does not check the same state/style-scope identity fields as generic subtree patching. It then rebuilds indexes after patching.

The two patch routes therefore have intentionally different compatibility contracts. A caller must select the appropriate route and know which retained identities remain trustworthy.

### 8.7 Geometry indexes assume topology-preserving local patches

`patch_subtree` preserves old child IDs and explicitly avoids rebuilding parents/indexes (`tree.rs:508-511`). This is safe only because the patch contract promises stable topology and existing parent relationships.

Likewise, `child_y_sorted` is computed at `index_component_roots()` time. Local patches do not recompute it. The current source assumes that a topology-preserving fixed-allocation patch does not invalidate row-child ordering assumptions used by painting. That assumption is plausible for many replacements but is not independently checked by the patch operation.

### 8.8 Viewport negative coordinates are deliberately outside `Rect`

`Rect` uses unsigned coordinates, while row viewport painting requires negative translation for skipped rows. `tree.rs` solves this with `SignedRect` and applies:

```text
offset_y -= skip_rows
```

before intersecting and clamping back into `Rect`.

This is a cross-boundary contract between:

- layout placement, which stores unscrolled child geometry;
- `LayoutTree` geometry traversal, which applies viewport translation;
- paint, which uses incremental/full painting with the same translation.

A replacement layout engine or geometry type cannot treat `Rect` coordinates as the complete physical coordinate model without preserving this signed intermediate behavior.

### 8.9 Root layout is normalized outside the allocator

`scene/root.rs:31-60` maps the body root View to `WidthRule::Fill` and `HeightRule::Fill` before final root composition. This is caller-supplied scene policy around the generic layout engine.

The allocator itself does not know that the terminal body should fill the root. It simply honors the normalized View rules. Removing or changing root normalization would change final geometry without changing allocator code.

### 8.10 Framework boundary remains generic

`ContentProvider` and `ContentMeasurement` (`presentation/content.rs`) are generic framework seams. They carry source-independent projection/measurement/paint identities and do not encode product/application semantics.

The layout subsystem consumes:

- offered width;
- width rule;
- intrinsic size;
- physical completeness;
- revisions and identities.

It does not infer assistant/application meaning. Any application-specific interpretation remains outside this generic Rust presentation boundary.

---

## 9. Open questions and coverage gaps

1. **No executed validation.** Tests, benchmarks, and compilation were not run. All behavior claims are static source claims or test-contract observations.
2. **Cache memory profile is not measured.** `MeasuredNode` retains recursive `View` clones, and two cache generations are retained. The exact memory cost for large trees is not instrumented.
3. **Cache-key aliasing is not exercised.** In particular, the interaction between `component_scope`, repeated semantic View IDs, and `component_view` was not tested with adversarial aliasing.
4. **Patch ordering assumptions are not directly asserted.** `child_y_sorted` is not visibly recomputed after generic subtree patches. Existing tests cover geometry/paint parity but not every possible topology-preserving reorder or track-allocation change.
5. **Provider revision correctness is external.** The layout cache trusts `ContentProvider::layout_input_revision`; no generic assertion verifies that a metric-changing provider update changes that revision.
6. **Track allocation policy is terminal-specific.** The allocator uses `u16`, equal-share flex distribution, source-order remainder assignment, and saturating arithmetic. Whether these are product requirements or implementation constraints is not established by this scope.
7. **Malformed IR behavior is not uniformly specified.** Some invalid inputs are clamped, some become incomplete, some return zero geometry, and some panic through `expect`/missing overlays.
8. **No explicit margin abstraction exists in this scope.** Decoration handles border/padding; row/column/grid handle gaps. There is no observed margin layout concept.
9. **No general rounding model exists.** All coordinates and dimensions are integer terminal cells; there is no fractional layout or explicit rounding phase.
10. **No independent overflow model exists for ordinary layout.** Overflow is represented through clamping, clipping, row viewport translation, and physical completeness rather than a general overflow object.
11. **Deep-tree recursion limits are not characterized.** Measurement, preparation, placement, index construction, component geometry, and validation all recurse. No depth stress test was observed in the assigned files.
12. **History unit measurement has a separate cache strategy.** The interaction between History’s unit cache and the scene/layout cache is outside this assignment and was only traced at the call boundary.
13. **The parent allocator boundary is not independently abstracted.** `tracks.rs` and `grid.rs` are generic over track inputs but remain tightly coupled to `TrackSize` and presentation IR types.

---

## 10. Evidence appendix

### 10.1 Baseline contract and assignment evidence

Read:

```text
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/evidence/assignments.json
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt
PRE-V5-ARCHITECTURE-REPORT.md
```

Relevant assignment record:

```text
ID: 05
slug: layout-geometry
scope: presentation/layout/ and geometry/
goal: General allocator, measurement/placement, invalidation and allocator assumptions outside this scope.
```

### 10.2 Geometry source manifest

Read comprehensively:

```text
crates/iyon-tui/src/geometry/mod.rs
crates/iyon-tui/src/geometry/constraints.rs
crates/iyon-tui/src/geometry/point.rs
crates/iyon-tui/src/geometry/rect.rs
crates/iyon-tui/src/geometry/size.rs
```

Key symbols:

```text
AxisConstraint
LayoutConstraints
Point
Rect
Rect::right
Rect::bottom
Rect::intersection
Size
```

### 10.3 Layout production source manifest

Read comprehensively:

```text
crates/iyon-tui/src/presentation/layout/mod.rs
crates/iyon-tui/src/presentation/layout/cache.rs
crates/iyon-tui/src/presentation/layout/engine.rs
crates/iyon-tui/src/presentation/layout/grid.rs
crates/iyon-tui/src/presentation/layout/measure.rs
crates/iyon-tui/src/presentation/layout/place.rs
crates/iyon-tui/src/presentation/layout/prepare.rs
crates/iyon-tui/src/presentation/layout/tracks.rs
crates/iyon-tui/src/presentation/layout/tree.rs
```

Key symbols:

```text
ViewCompiler
LayoutBlock
LayoutCache
MeasureKey
PrepareKey
WidthIntent
MeasuredNode
MeasuredKind
PreparedNode
PreparedKind
PreparedChild
TrackAllocation
allocate_tracks
SpanRequirement
FlexMode
allocate_grid_tracks
track_offset
span_extent
emit_prepared
LayoutNodeId
LayoutStyle
LayoutContent
ChildDependency
LayoutNode
LayoutTree
ComponentGeometry
ComponentGeometryMap
LayoutTree::index_component_roots
LayoutTree::content_dependency_view_ids
LayoutTree::content_repaint_roots
LayoutTree::update_content_measurement
LayoutTree::incremental_paint_geometry
LayoutTree::incremental_paint_rect
LayoutTree::patch_subtree
LayoutTree::patch_component_subtree
LayoutTree::component_geometry
LayoutTree::patch_component_geometry_subtree
LayoutTree::validate
```

### 10.4 Layout test source manifest

Indexed and behaviorally sampled/read:

```text
crates/iyon-tui/src/presentation/layout/tests/flow.rs
crates/iyon-tui/src/presentation/layout/tests/grid.rs
crates/iyon-tui/src/presentation/layout/tests/mod.rs
crates/iyon-tui/src/presentation/layout/tests/style.rs
crates/iyon-tui/src/presentation/layout/tests/text.rs
```

Representative test symbols:

```text
layout_stage_counters_match_semantic_nodes
row_paint_lowering_matches_surface_paint_for_common_layouts
retained_layout_cache_reuses_warm_measurement_and_prepare
retained_layout_cache_reuses_unaffected_shared_path
retained_layout_cache_rotates_out_old_view_id_working_sets
standalone_measurement_matches_width_only_layout
bounded_vertical_tracks_allocate_multiple_flex_children
unbounded_column_treats_flex_as_intrinsic_after_fixed_tracks
fit_row_respects_fixed_track_and_fill_width_content
bounded_row_vertical_alignment_uses_extra_height
clamp_does_not_mask_impossible_wide_grapheme
row_uses_track_width_for_continuations
narrow_rows_never_overflow_the_surface
zero_width_spacers_contribute_vertical_extent_and_horizontal_height
shared_columns_align_across_rows
fixed_content_flex_consume_width
horizontal_alignment_places_the_child_view
vertical_alignment_places_the_child_view
column_span_area_includes_internal_gap
row_span_area_includes_internal_gap
spanning_cell_grows_content_columns
spanning_cell_grows_content_rows
```

### 10.5 Adjacent source inspected for ownership/invalidation seams

```text
crates/iyon-tui/src/presentation/content.rs
  ContentDirtyReason
  ContentDirty
  PreparedProjectionTicket
  ContentMeasurement
  ContentProvider
  EmptyContentProvider

crates/iyon-tui/src/presentation/ir.rs
  WidthRule
  HeightRule
  ColumnView
  RowView
  HangingView
  GridView
  TrackSize
  RowViewportView

crates/iyon-tui/src/retained_state/record.rs
  ViewStateRecord
  ViewStateLifecycle
  ViewStateRecord::apply_geometry
  ViewStateRecord::apply_presentation
  geometry_revision
  presentation_revision

crates/iyon-tui/src/scene/layout.rs
  ResolvedSceneLayout
  layout_resolved_scene_with_cache_and_content
  ResolvedSceneLayout::patch_component_with_cache
  LayoutSynchronizer
  LayoutSynchronizer::synchronize_component

crates/iyon-tui/src/scene/host.rs
  SceneHost
  layout_cache
  retained
  invalidate_content
  invalidate_theme
  try_local_content_refresh
  try_local_geometry_refresh
  invalidate_root
  discard_candidate
  commit_content_candidate
  abort_content_candidate

crates/iyon-tui/src/scene/root.rs
  Scene::new
  Scene::with_history
  Scene::set_body
  resolve_root_scene_with_anchor_and_cache_and_states_and_content
  merge_root_scene

crates/iyon-tui/src/history/projection/mod.rs
  project_into_session...
  view_height

crates/iyon-tui/src/application/content.rs
  ContentRegistry measurement/provider implementations
  ContentMeasurement construction
  content metric/paint revisions

crates/iyon-tui/src/presentation/paint/view.rs
  PaintCache
  ViewPainter
  row painting and LayoutTree consumption

crates/iyon-tui/src/perf.rs
  Counter
  layout/paint/state/content instrumentation
```

### 10.6 Files indexed but not treated as owned scope

The repository manifest was used to identify broader Rust, native, TypeScript, generated, benchmark, fixture, and documentation areas. Those areas were not comprehensively read because they are assigned to other scouts or outside this report’s scope. No claim is made that they are unused or irrelevant.

### 10.7 LOC methodology

LOC figures are approximate physical source-line counts from the observed file line ranges, including blank lines and comments, not executable-statement counts.

- Geometry production: approximately 114 physical lines.
- Layout production: approximately 3,436 physical lines.
- Layout tests: approximately 3,200–3,400 physical lines.
- Generated code, external packages, and unrelated tests were excluded from the assignment totals.

No repository edits were made.