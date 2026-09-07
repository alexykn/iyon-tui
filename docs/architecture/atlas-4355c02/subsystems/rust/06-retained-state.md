# 06 — Retained State

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/retained_state/`
- Assignment goal: occurrences, storage, revisions, effective values, effects, capture, geometry, and damage dependencies.
- Parent-added atlas documentation is outside the source baseline and was treated as investigative guidance only.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- repository `AGENTS.md`
- retained-state implementation files
- the direct Rust consumers in application, scene, presentation layout, and terminal paths
- relevant TypeScript/native retained-state consumers
- PERF-13 historical completion and handoff material as historical context, not as authority where source differed.

### Scope boundaries

The retained-state directory owns the Rust state-plane primitives:

- state capability classification;
- state-attached physical occurrence representation;
- mutable host-owned records;
- immutable snapshots and revisions;
- geometry and presentation override values;
- native effect classification;
- candidate-frame capture overlays;
- rectangle damage metadata.

It does **not** own:

- semantic `View` construction;
- structural component ownership;
- content Source/Funnel/Connector storage;
- terminal decoding or terminal worker behavior;
- public TypeScript scheduling/runtime policy;
- the general layout allocator itself;
- focus or interaction state beyond the presentation effects consumed by the scene host.

The retained-state code is deliberately coupled to presentation types such as `ViewKind`, `Decoration`, `StyleStates`, `WidthRule`, `HeightRule`, `BorderSpec`, `Insets`, and `ColorSpec`. The state plane is separate from semantic structure and content, but its effective-value calculation targets the current presentation representation.

### Evidence status

This report is based on static source inspection. I did not run builds, tests, benchmarks, or services. Tests cited below are source-level behavioral evidence; they are not claims that this run executed them.

The exact source manifest identifies the retained-state files and adjacent state consumers. The primary inspected Rust files were:

```text
crates/iyon-tui/src/retained_state/capabilities.rs
crates/iyon-tui/src/retained_state/capture.rs
crates/iyon-tui/src/retained_state/damage.rs
crates/iyon-tui/src/retained_state/effects.rs
crates/iyon-tui/src/retained_state/geometry.rs
crates/iyon-tui/src/retained_state/mod.rs
crates/iyon-tui/src/retained_state/occurrence.rs
crates/iyon-tui/src/retained_state/presentation.rs
crates/iyon-tui/src/retained_state/record.rs
crates/iyon-tui/src/retained_state/registry.rs
```

Important adjacent consumers inspected:

```text
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/application/kernel.rs
crates/iyon-tui/src/application/view_state.rs
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/layout/measure.rs
crates/iyon-tui/src/presentation/layout/place.rs
crates/iyon-tui/src/presentation/layout/tree.rs
crates/iyon-tui/src/scene/resolve.rs
crates/iyon-tui/src/scene/resolved.rs
crates/iyon-tui/src/scene/host.rs
crates/iyon-tui/src/terminal/backend.rs
crates/iyon-tui/src/terminal/termwiz/lower.rs
crates/iyon-tui/src/perf.rs
crates/iyon-tui-native/src/tui.rs
crates/iyon-tui-native/src/tui/view_state.rs
packages/iyon-tui/src/api/view/retained-state.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/tests/tui_perf13_b.test.ts
packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
```

---

## 1. Responsibility and structure

### 1.1 Module inventory

The retained-state directory contains ten Rust files. All are production modules; most also contain inline unit tests.

| File | Approx. physical LOC | Production responsibility | Test responsibility | Public surface |
|---|---:|---|---|---|
| `retained_state/mod.rs` | 35 | Module declaration and re-export boundary | None | Selective public or `pub(crate)` re-exports |
| `capabilities.rs` | 121 | Exhaustive `ViewKind` → state capability classification and geometry validation | Capability and presentation-box assertions | Internal |
| `capture.rs` | 155 | Frame-lifetime immutable candidate overlay and lookup view | Overlay shadowing and empty-view behavior | Internal |
| `damage.rs` | 102 | Rectangle clipping, touching-rectangle merging, full-damage escalation | Merge/full-damage behavior | Internal |
| `effects.rs` | 66 | Bit-set consequences of state mutations | None | Internal |
| `geometry.rs` | 334 | Public geometry patch vocabulary, sparse overrides, effective geometry, effect mapping | None in this file | Public patch types under native-host |
| `occurrence.rs` | 76 | Physical state-attached occurrence box containing base/effective values | None | Internal |
| `presentation.rs` | 234 | Public presentation patch vocabulary, sparse overrides, immutable snapshots, effective decoration/style-state derivation | Null/clear and effective-style tests | Public patch types under native-host |
| `record.rs` | 193 | Mutable host-owned state record, mutation/revision lifecycle | No-op revision behavior | Internal |
| `registry.rs` | 685 | State identity allocation, records, committed snapshots, dirty worklist, desired/visible/in-flight bindings, prepare/commit/dispose | Identity, capture, lifecycle, remount, rollback, validation tests | Internal |

Approximate counting method:

- physical line ranges from the source files;
- inline `#[cfg(test)]` modules counted separately;
- no generated code included;
- approximately 2,001 total physical lines;
- approximately 1,600 production lines;
- approximately 400 inline test lines.

The directory is therefore small in file count but central in the frame transaction. Its code is not a passive value container: the registry and capture layers define state ownership, visibility, retry, and lifecycle semantics.

### 1.2 Primary and secondary responsibilities

#### Primary

1. Preserve host-owned mutable state independently of semantic `View` rebuilding.
2. Associate a state identity with exactly one candidate physical occurrence.
3. Derive effective geometry and presentation from immutable semantic base values plus mutable overrides.
4. Publish immutable state versions to frame preparation.
5. Keep desired, visible, and in-flight bindings distinct.
6. Classify mutation consequences for layout, placement, clipping, paint, content projection, and damage.
7. Produce bounded incremental paint damage.

#### Secondary

- Enforce node-kind capability rules.
- Preserve old immutable state versions across failed or concurrent frame attempts.
- Prevent state resources from being disposed while still attached or pinned by a candidate.
- Keep unmounted state values without eagerly allocating committed snapshots.
- Supply state-root indexes and state paths used by layout-cache invalidation.
- Expose counters for mutation, invalidation, relayout, paint, and damage activity.

### 1.3 Framework boundary

The retained-state implementation is generic framework machinery. It operates on caller-supplied presentation values and generic style state keys. It does not interpret application or Iyon meaning.

The `style_states: BTreeMap<String, String>` field is generic. The Rust code does not hard-code product states such as assistant activity, model status, tool state, or conversation policy.

---

## 2. Types, APIs and contracts

### 2.1 Module export boundary

`retained_state/mod.rs` exposes:

```rust
pub(crate) use capabilities::{StateNodeKind, state_node_kind};
#[cfg(feature = "native-host")]
pub(crate) use capabilities::{presentation_state_capable, validate_geometry_for_kind};

pub(crate) use capture::{StateCandidateOverlay, StateFrameView};
pub(crate) use damage::DamageRegion;
pub(crate) use effects::StateEffects;
pub(crate) use geometry::EffectiveGeometry;
pub use geometry::GeometryAlignment;

#[cfg(feature = "native-host")]
pub use geometry::{ViewStateGeometryPatch, ViewStateGeometryProperty};

pub(crate) use occurrence::OccurrenceBox;
pub(crate) use presentation::ViewStateSnapshot;

#[cfg(feature = "native-host")]
pub use presentation::{ViewStatePresentationPatch, ViewStatePresentationProperty};

#[cfg(all(test, not(feature = "native-host")))]
pub(crate) use presentation::{
    ViewStatePresentationPatch,
    ViewStatePresentationProperty,
};

#[cfg(feature = "native-host")]
pub(crate) use record::{ViewStateLifecycle, ViewStateRecord};

#[cfg(feature = "native-host")]
pub(crate) use registry::{PreparedStateCommit, ViewStateRegistry};
```

The intentional public authoring surface is:

- `GeometryAlignment`;
- `ViewStateGeometryPatch`;
- `ViewStateGeometryProperty`;
- `ViewStatePresentationPatch`;
- `ViewStatePresentationProperty`.

The registry, record, snapshot, effect, occurrence, capture, capability, and damage types are internal runtime machinery.

`HostViewState` is defined in `application/view_state.rs`, not in `retained_state/`. It is the host-bound wrapper exposed to the native binding layer.

### 2.2 `StateNodeKind`

`capabilities.rs:11-24` defines:

```rust
pub(crate) enum StateNodeKind {
    Text,
    Spacer,
    Row,
    Column,
    Grid,
    Hanging,
    Container,
    ClampRows,
    RowViewport,
    ContentHost,
    ComponentSlot,
}
```

`state_node_kind()` maps every current `presentation::ir::ViewKind` variant to one concrete state class (`capabilities.rs:27-42`).

This is an exhaustive mapping. A new semantic node kind must be added to this match before it can participate in state validation.

`presentation_state_capable()` reports whether the semantic kind owns an addressable presentation box (`capabilities.rs:45-62`):

- all physical layout/content kinds are state-capable;
- `ComponentSlot` is not state-capable.

The reason for the latter is explicit in the source: a component slot is a structural indirection, not its own physical presentation box. The concrete component `View` owns the retained presentation state.

### 2.3 `OccurrenceBox`

`occurrence.rs:12-28` defines the canonical physical occurrence record:

```rust
pub(crate) struct OccurrenceBox {
    pub(crate) state_attachment: Option<u64>,
    pub(crate) node_kind: StateNodeKind,

    pub(crate) base_width: WidthRule,
    pub(crate) base_height: HeightRule,
    pub(crate) effective_width: WidthRule,
    pub(crate) effective_height: HeightRule,

    pub(crate) base_gap: Option<u16>,
    pub(crate) effective_gap: Option<u16>,

    pub(crate) base_alignment: GeometryAlignment,
    pub(crate) effective_alignment: GeometryAlignment,

    pub(crate) base_decoration: Decoration,
    pub(crate) effective_decoration: Decoration,

    pub(crate) base_style_states: StyleStates,
    pub(crate) effective_style_states: StyleStates,
}
```

Important contracts:

- Every physical state-capable layout occurrence has one `OccurrenceBox`, including an initially undecorated node.
- `state_attachment` is optional; an occurrence can exist without retained mutable state.
- Mutable state does not create a structural wrapper.
- Base and effective values are retained simultaneously.
- `apply_state()` recalculates all effective geometry and style values from the immutable base values plus a supplied `ViewStateSnapshot` (`occurrence.rs:62-76`).

The physical occurrence is embedded directly in `presentation::layout::LayoutNode` (`layout/tree.rs:100-116`). The layout node also stores physical rectangles, clipping, child IDs, dependency metadata, style, and content.

This means retained state is not a separate parallel tree. It is a state overlay applied to the canonical physical occurrence owned by the layout tree.

### 2.4 Geometry patch vocabulary

`geometry.rs:12-25` defines `ViewStateGeometryPatch`:

```rust
pub struct ViewStateGeometryPatch {
    pub width: Option<ViewStateSizeMode>,
    pub height: Option<ViewStateSizeMode>,
    pub padding: Option<Insets>,
    pub min_width: Option<Option<u16>>,
    pub max_width: Option<Option<u16>>,
    pub min_height: Option<Option<u16>>,
    pub max_height: Option<Option<u16>>,
    pub gap: Option<u16>,
    pub alignment: Option<GeometryAlignment>,
    pub border_edges: Option<Option<BorderEdges>>,
}
```

The outer `Option` means “this field is present in this patch.” Nullable geometry values use a nested option:

- `None`: no override;
- `Some(Some(value))`: explicit override;
- `Some(None)`: explicit nullable semantic null.

For bounds and border edges, this distinction is meaningful. Clear operations restore the outer `None` state.

`GeometryAlignment` (`geometry.rs:27-32`) has independent horizontal and vertical axes:

```rust
pub struct GeometryAlignment {
    pub horizontal: Option<HorizontalAlign>,
    pub vertical: Option<VerticalAlign>,
}
```

`ViewStateSizeMode` (`geometry.rs:244-263`) supports only:

```rust
Fit
Fill
```

They map directly to `WidthRule::{Fit, Fill}` and `HeightRule::{Fit, Fill}`.

`ViewStateGeometryProperty` (`geometry.rs:219-230`) enumerates individually clearable geometry domains:

```text
Width
Height
Padding
MinWidth
MaxWidth
MinHeight
MaxHeight
Gap
Alignment
BorderEdges
```

### 2.5 Presentation patch vocabulary

`presentation.rs:21-30` defines `ViewStatePresentationPatch`:

```rust
pub struct ViewStatePresentationPatch {
    pub foreground: Option<Option<ColorSpec>>,
    pub background: Option<Option<ColorSpec>>,
    pub border_color: Option<Option<ColorSpec>>,
    pub border_style: Option<Option<BorderStyle>>,
    pub border_glyphs: Option<Option<BorderGlyphs>>,
    pub text_attributes: TextAttributeSpec,
    pub style: Option<Option<StyleRef>>,
}
```

The clearable domains are enumerated by `ViewStatePresentationProperty` (`presentation.rs:32-41`):

```text
Foreground
Background
BorderColor
BorderStyle
BorderGlyphs
TextAttributes
Style
```

`TextAttributeSpec` is reused from the existing presentation/style vocabulary rather than creating a state-specific duplicate (`presentation.rs:18-20`).

`PresentationOverrides` (`presentation.rs:44-53`) is internal mutable sparse storage. Its `apply_patch()` operation:

1. clones the old override set;
2. applies only fields present in the patch;
3. overlays text attributes;
4. returns whether the logical values changed (`presentation.rs:55-78`).

Its `clear()` operation restores selected domains to absent or resets all fields (`presentation.rs:80-100`).

### 2.6 `ViewStateSnapshot`

`presentation.rs:103-115` defines the immutable frame-time copy:

```rust
pub(crate) struct ViewStateSnapshot {
    pub(crate) id: u64,
    pub(crate) geometry: GeometryOverrides,
    pub(crate) presentation: PresentationOverrides,
    pub(crate) style_states: BTreeMap<String, String>,
    pub(crate) revision: u64,
    pub(crate) geometry_revision: u64,
    pub(crate) presentation_revision: u64,
}
```

Three revision levels exist:

- `revision`: any logical state mutation;
- `geometry_revision`: geometry override mutations only;
- `presentation_revision`: presentation or style-state mutations only.

These revisions are independent and are included in measurement/cache keys by `presentation/layout/measure.rs:216-219`.

The snapshot is immutable after publication and is normally stored behind `Arc<ViewStateSnapshot>`.

### 2.7 Effective geometry

`GeometryOverrides::effective()` (`geometry.rs:124-168`) derives an `EffectiveGeometry` from:

- base width rule;
- base height rule;
- base decoration;
- base gap;
- base alignment;
- retained geometry overrides.

The derivation order is:

1. clone the base decoration;
2. apply retained padding;
3. apply min/max width and height bounds;
4. apply border-edge presence/removal;
5. merge retained alignment per axis;
6. override width and height mode;
7. override gap;
8. return `EffectiveGeometry`.

`EffectiveGeometry` (`geometry.rs:233-242`) contains:

```rust
pub(crate) struct EffectiveGeometry {
    pub(crate) width: WidthRule,
    pub(crate) height: HeightRule,
    pub(crate) decoration: Decoration,
    pub(crate) gap: Option<u16>,
    pub(crate) alignment: GeometryAlignment,
}
```

Bounds are applied with nullable semantics (`geometry.rs:171-182`):

- explicit minimum null maps to `0`;
- explicit maximum null maps to `u16::MAX`.

Border-edge behavior is significant:

- `Some(Some(edges))` creates a plain `BorderSpec` if necessary and applies edge presence;
- `Some(None)` removes the decoration border;
- the border override is therefore capable of adding a border to an initially undecorated occurrence.

### 2.8 Effective presentation

`ViewStateSnapshot::effective_decoration()` (`presentation.rs:133-170`) overlays presentation values on a supplied decoration:

- `StyleRef` can replace the text style entirely for themed style refs;
- direct style refs are merged into the existing text style;
- explicit style null produces an empty direct style;
- foreground overrides text foreground;
- background overrides surface background;
- border color/style/glyphs apply only when a border exists;
- text attributes are applied last.

`effective_style_states()` (`presentation.rs:172-178`) clones base style states and overwrites keys from the snapshot’s `BTreeMap`.

### 2.9 Mutable `ViewStateRecord`

`record.rs:13-31` defines the mutable host-owned record:

```rust
pub(crate) struct ViewStateRecord {
    pub(crate) id: u64,
    pub(crate) lifecycle: ViewStateLifecycle,

    pub(crate) desired_bound: bool,
    pub(crate) desired_kind: Option<StateNodeKind>,

    pub(crate) visible_bound: bool,
    pub(crate) visible_kind: Option<StateNodeKind>,

    pub(crate) in_flight_bound: bool,

    pub(crate) geometry: GeometryOverrides,
    pub(crate) presentation: PresentationOverrides,
    pub(crate) style_states: BTreeMap<String, String>,

    pub(crate) revision: u64,
    pub(crate) geometry_revision: u64,
    pub(crate) presentation_revision: u64,
}
```

`ViewStateLifecycle` has only:

```text
Live
Disposed
```

There is no `completed`, `frozen`, `resident`, or `cold` lifecycle in this module. Those terms belong to other state machines such as History/content. In retained state, binding and frame-pin status are represented separately by booleans and sets.

`ViewStateRecord::snapshot()` (`record.rs:59-68`) clones the logical values and revisions into an immutable snapshot.

### 2.10 Public host wrapper

`application/view_state.rs:23-28` defines `HostViewState`:

```rust
pub struct HostViewState {
    id: u64,
    host: Weak<Mutex<HostInner>>,
}
```

The wrapper stores only:

- immutable state identity;
- weak host ownership.

It does not own the mutable record. The host’s registry owns the record directly.

Public wrapper methods:

```text
state_id()
validate_node_kind()
set_geometry()
clear_geometry()
set_presentation()
clear_presentation()
set_style_state()
clear_style_state()
dispose()
```

Every mutation upgrades the weak host, locks `HostInner`, checks host liveness, invokes the registry/record mutation, and then routes non-empty effects through host invalidation (`application/view_state.rs:106-124`).

There is no per-record lock. All mutation and validation are serialized through the host lock, matching the ownership comment in `view_state.rs:6-9`.

---

## 3. Dependency and ownership map

### 3.1 High-level graph

```text
TypeScript ViewState facade
    │
    ▼
N-API NativeViewState
    │  decode typed masks/words/string lanes
    ▼
HostViewState { id, Weak<HostInner> }
    │
    ▼
HostInner mutex
    │
    ├── ViewStateRegistry
    │      ├── records: HashMap<u64, Box<ViewStateRecord>>
    │      ├── committed: HashMap<u64, Arc<ViewStateSnapshot>>
    │      ├── dirty: HashSet<u64>
    │      ├── desired / visible / in_flight binding sets
    │      └── capture_epoch
    │
    ├── SceneHost invalidation
    │      ├── invalidated_states
    │      ├── invalidated_state_effects
    │      ├── layout-cache invalidation
    │      └── incremental paint state IDs
    │
    └── frame candidate metadata
           ├── StateCandidateOverlay
           ├── StateFrameView
           ├── PreparedStateCommit
           └── candidate PreparedSceneFrame

StateCandidateOverlay
    │
    ▼
StateFrameView
    │  overlay lookup, then committed fallback
    ▼
ResolveSession / ResolutionOverlay
    │
    ▼
layout::measure_node()
    │  effective geometry/style + revision-keyed measurement
    ▼
layout::place::emit_prepared()
    │
    ▼
LayoutTree::LayoutNode
    └── OccurrenceBox
           ├── base values
           └── effective values

LayoutTree indexes
    ├── state_roots: state ID → physical node
    ├── parents
    └── child_dependencies

SceneHost
    ├── state path cache invalidation
    ├── local geometry refresh or retained-root relayout
    ├── incremental subtree paint
    └── DamageRegion

PreparedSceneFrame
    └── TerminalBackend::begin_frame()
```

### 3.2 Identity and lifetime

A state identity is allocated by `ViewStateRegistry::create(host_id)` (`registry.rs:68-81`).

The identity is:

```text
(host_id << 32) | local_id
```

Constraints:

- `host_id` must be nonzero and ≤ `0x001f_ffff`;
- local IDs start at `1`;
- local IDs must fit in `u32`;
- local IDs advance monotonically;
- overflow returns `ViewState identity exhausted`.

The first test confirms:

```text
host 1, local 1 → 0x1_0000_0001
host 1, local 2 → 0x1_0000_0002
```

The identity is not reused after disposal. Disposal removes the record and committed snapshot, but `next_id` continues forward.

Ownership:

- `HostInner` owns `ViewStateRegistry`;
- registry owns `Box<ViewStateRecord>`;
- `HostViewState` holds a weak reference to `HostInner`;
- an in-flight candidate owns `Arc<ViewStateSnapshot>` pins;
- the visible frame owns the committed physical occurrence and surface;
- host teardown disposes all state records after clearing bindings.

### 3.3 Structural identity versus occurrence identity

The semantic `View` carries an optional state attachment scalar in `ViewNode.state_attachment` (`presentation/ir.rs:700-709`). The attachment is not itself a wrapper node.

`View::native_with_state_attachment()` (`presentation/ir.rs:832-847`) validates:

- state ID must be positive;
- target `View` kind must be presentation-state-capable;
- repeated application of the same state ID to the same `View` is idempotent;
- attaching a different state ID creates a shallow semantic node copy with a new `ViewId`.

The state ID identifies the mutable state resource. The physical occurrence is identified by the corresponding `LayoutNodeId` in the current candidate layout tree.

The source intentionally prevents the public API from exposing a path or using semantic `ViewId` as an occurrence address. The layout tree creates the state ID → physical node index (`layout/tree.rs:253-301`).

### 3.4 Creation and destruction

Creation path:

```text
TuiHost::create_view_state()
    → HostInner.view_states.create(host_id)
    → ViewStateRecord::new(id)
    → HostViewState::new(id, Weak<HostInner>)
```

No committed snapshot and no dirty mark are created at state creation (`registry.rs:455-460` test).

Individual disposal:

```text
HostViewState::dispose()
    → HostInner::dispose_view_state()
    → ViewStateRegistry::dispose()
    → ViewStateRecord::dispose()
    → ViewStateRegistry::remove()
```

Disposal rejects records that are:

- desired-bound;
- visible-bound;
- in-flight-bound.

Unknown and repeated disposal are idempotent (`registry.rs:327-345`, tests at `registry.rs:648-670`).

Host teardown:

```text
Host close / failed-host retirement
    → clear_view_state_bindings()
    → dispose_view_states()
    → ViewStateRegistry::dispose_all()
```

The host clears bindings before disposal so mounted resources do not fail disposal merely because teardown is in progress (`application/host.rs:1128-1134`, `1816-1818`).

---

## 4. Execution paths and state transitions

### 4.1 TypeScript/native mutation path

The TypeScript facade defines `ViewState` methods in `packages/iyon-tui/src/api/view/retained-state.ts`:

```text
ViewState.setGeometry()
ViewState.clearGeometry()
ViewState.setPresentation()
ViewState.clearPresentation()
ViewState.setStyleState()
ViewState.clearStyleState()
```

The facade:

1. checks mutation timing through `assertMutationAllowed`;
2. converts the ergonomic object into a typed envelope;
3. invokes N-API with masks, word lanes, and string lanes;
4. requests an environment wake when the returned wake disposition contains the drain bit.

`iyon-tui-native/src/tui/view_state.rs` decodes the envelope before calling the Rust host wrapper. The native code explicitly documents that malformed envelopes must not leave partial state behind.

The Rust path is:

```text
NativeViewState::set_geometry()
    → decode_geometry_envelope()
    → HostViewState::set_geometry()
    → HostViewState::mutate()
    → HostInner::mutate_view_state()
    → ViewStateRegistry::mutate_record()
    → ViewStateRecord::apply_geometry()
    → HostInner::invalidate_state()
    → SceneHost::invalidate_state()
```

Presentation and style-state mutations follow the same route.

### 4.2 Geometry mutation transition

`ViewStateRecord::apply_geometry()` (`record.rs:71-90`):

1. clones current `GeometryOverrides`;
2. applies the patch to the clone;
3. returns a no-op if no logical field changes;
4. validates the prospective clone against `desired_kind` or `visible_kind`;
5. replaces the record’s geometry only after validation succeeds;
6. increments accepted-mutation and geometry-invalidation counters;
7. increments `revision` and `geometry_revision`;
8. returns classified effects.

This gives geometry mutations atomic prospective-patch semantics. A failed validation leaves the prior geometry untouched.

`clear_geometry()` uses the same pattern (`record.rs:92-110`).

### 4.3 Presentation mutation transition

`ViewStateRecord::apply_presentation()` (`record.rs:113-126`):

1. mutates the sparse presentation overlay;
2. returns `StateEffects::NONE` for a logical no-op;
3. increments `revision` and `presentation_revision`;
4. classifies the mutation through `presentation_effects(false)`.

Presentation mutations do not validate against a node kind because the current presentation patch vocabulary is not node-kind restricted in this module.

### 4.4 Generic style-state transition

`set_style_state()` and `clear_style_state()` (`record.rs:143-164`) use a `BTreeMap<String, String>`.

Behavior:

- setting the same key/value is a no-op;
- setting a different value inserts/replaces the key;
- clearing a missing key is a no-op;
- logical changes increment `revision` and `presentation_revision`;
- style-state changes classify as subtree paint effects because descendant selectors may observe inherited state.

The public host wrapper rejects empty keys and values (`application/view_state.rs:78-92`).

### 4.5 Registry mutation and publication

`ViewStateRegistry::mutate_record()` (`registry.rs:92-120`) is the sole registry-level mutation path.

Behavior:

1. look up the record;
2. reject unknown or disposed identities;
3. invoke the record mutation closure;
4. return immediately for empty effects;
5. determine whether the record is currently demanded:
   - desired-bound, or
   - visible-bound, or
   - in-flight-bound;
6. if demanded, snapshot the current mutable record and replace the committed `Arc`;
7. insert the state ID into the deduplicated dirty set.

An accepted mutation therefore has two separate consequences:

- logical state becomes current in the host registry;
- a frame invalidation work item is queued.

The dirty set is deduplicated, so repeated writes before capture result in one state ID in the worklist.

An important visibility distinction:

- the registry’s `committed` table is the current immutable state-version table;
- `HostInner.frame` remains the last successfully presented physical frame;
- changing the registry’s committed `Arc` does not itself make the new style or geometry visible.

### 4.6 Desired binding transition

`TuiHost::set_desired_view()` (`application/host.rs:1091-1120`) performs pre-publication validation:

1. collects state targets from the body plus current History views;
2. rejects duplicate state IDs;
3. validates each state ID and stored geometry against its concrete node kind;
4. validates content targets;
5. increments the desired structural revision;
6. updates desired state bindings;
7. updates desired content bindings;
8. publishes the desired semantic body;
9. marks the host pending.

`ViewStateRegistry::set_desired()` (`registry.rs:191-202`) validates all targets before changing any binding. It removes desired bindings no longer in the target set and prunes snapshots only when no desired, visible, or in-flight binding remains.

The desired set is not the visible set. Desired structural publication can advance while the old physical frame remains visible.

### 4.7 Candidate capture transition

At frame preparation, `HostInner::capture_state_candidate()` delegates to `ViewStateRegistry::capture_candidate()` (`application/host.rs:1702-1706`).

The registry:

1. increments `capture_epoch`;
2. computes the union of desired, visible, and in-flight IDs;
3. sorts demanded IDs;
4. visits only demanded IDs;
5. includes an ID in `touched` if:
   - it is dirty;
   - it is newly demanded but not visible; or
   - no committed snapshot exists;
6. lazily snapshots a demanded record if the committed table is missing;
7. removes dirty marks for the demanded set;
8. leaves dirty marks for unmounted records queued.

The resulting `StateCandidateOverlay` contains:

```rust
epoch: u64
demanded: Vec<u64>
touched: HashMap<u64, Arc<ViewStateSnapshot>>
```

This deliberately avoids cloning or visiting unrelated unmounted state records.

### 4.8 Frame lookup transition

`StateFrameView` (`capture.rs:78-127`) borrows:

- the registry’s committed map;
- one candidate overlay.

Its lookup order is:

```text
overlay.touched[id]
    or committed[id]
```

This means:

- changed/newly demanded states read through the candidate overlay;
- clean states read through the committed table;
- state versions remain shared through `Arc`;
- no branch-wide map copy is needed.

`ResolveSession::set_state_snapshots()` (`scene/resolve.rs:95-106`) copies only demanded Arc references into the branch `ResolutionOverlay`.

### 4.9 Candidate layout path

The state-bearing candidate enters:

```text
StateFrameView
    → ResolveSession::set_state_snapshots()
    → ResolutionOverlay.states
    → layout::measure_node()
    → layout::place::emit_prepared()
    → LayoutNode::OccurrenceBox
    → LayoutTree indexes
```

During measurement (`presentation/layout/measure.rs:191-246`):

- the state snapshot is looked up by the semantic attachment ID;
- `geometry_revision` and `presentation_revision` are added to `MeasureKey`;
- effective geometry is computed before measuring the kind;
- effective style states are computed;
- text alignment can be changed by the retained state;
- content measurement sees the effective width rule;
- the measured result is cached under the state-sensitive key.

During placement (`presentation/layout/place.rs:14-111`):

- `OccurrenceBox::from_effective()` stores the base/effective geometry and style values;
- the state attachment ID and concrete `StateNodeKind` are retained;
- the effective decoration/style state also populates the `LayoutStyle`.

### 4.10 Visible and in-flight transitions

`PreparedSceneFrame` reports the state bindings encountered in its fully prepared candidate (`scene/host.rs:129-139`).

Before backend presentation, `HostInner::candidate_state_commit()` invokes `ViewStateRegistry::prepare_candidate()` (`application/host.rs:1753-1761`):

1. validates the complete candidate target set;
2. prepares visible additions/removals;
3. records all candidate state IDs as in-flight;
4. updates record-level in-flight flags.

The candidate is then held in `HostInner.candidate_frame` and paired with:

- candidate host epoch;
- candidate structural revision;
- candidate content commit;
- candidate state commit;
- backend receipt.

On successful receipt:

```text
candidate state commit
    → commit_visible_prepared()
    → visible bindings replaced
    → in-flight bindings cleared
    → committed snapshots pruned where no binding remains
    → candidate frame becomes HostInner.frame
```

Relevant code is `application/host.rs:2070-2099` and `registry.rs:257-287`.

On failed preparation or failed backend presentation:

```text
candidate state commit
    → clear_in_flight_prepared()
    → candidate frame discarded
    → visible binding remains unchanged
    → old HostInner.frame remains authoritative
```

This is the key transaction invariant. State candidate preparation may be discarded without partially promoting visibility.

### 4.11 State-only scene transition

`SceneHost::try_incremental_stable()` handles state-only work (`scene/host.rs:1542-1697`).

When state invalidation is combined with structural/body/History change, incremental handling returns `None`, forcing the retained-root/full path.

When state invalidation is the only relevant work:

1. state IDs are sorted;
2. `geometry_refresh` is true if any effect has `GEOMETRY`;
3. state dependency paths are invalidated in layout and paint caches;
4. snapshots are inserted into the retained root/body/History overlays;
5. pure presentation changes apply snapshots directly to existing `OccurrenceBox` values;
6. geometry changes use either:
   - local geometry refresh; or
   - retained-root layout recomputation;
7. state IDs become incremental paint targets when physical geometry is stable.

### 4.12 Local geometry refresh transition

`SceneHost::try_local_geometry_refresh()` (`scene/host.rs:641-790`) attempts topology-preserving local remeasurement.

For every changed state:

1. locate the state root in `LayoutTree.state_roots`;
2. derive physical path to root;
3. derive semantic path to root using `state_view_path()`;
4. ensure physical and semantic paths have equal length;
5. invalidate the path’s layout-cache entries;
6. walk from target toward root;
7. use child dependency metadata and effect intrinsic flags to determine whether a changed intrinsic size can escape the current allocation;
8. remeasure the smallest safe subtree;
9. patch it if the replacement fits and preserves topology;
10. update affected component geometry.

If no safe local patch exists, the caller rebuilds the retained resolved-root layout while preserving semantic structure and cache entries outside the affected dependency frontier.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production path matrix

| Semantic operation | Authoritative production path | Selection condition | Failure/no-op behavior |
|---|---|---|---|
| Create state | `TuiHost::create_view_state()` → `ViewStateRegistry::create()` | Host open and host lock available | Closed host or identity exhaustion returns error |
| Attach state to View | `View::native_with_state_attachment()` | Native structural construction | Zero ID or unsupported kind returns error; same ID on same View is idempotent |
| Publish desired body | `TuiHost::set_desired_view()` | User/public structural publication | Duplicate state, disposed state, wrong host, unsupported geometry, or content error rejects before desired body mutation |
| Set geometry | `HostViewState::set_geometry()` → `ViewStateRecord::apply_geometry()` | External mutation allowed and host live | Prospective validation failure leaves prior values unchanged |
| Clear geometry | `clear_geometry()` → `ViewStateRecord::clear_geometry()` | External mutation allowed and host live | Missing values are no-op; invalid prospective result leaves prior values unchanged |
| Set presentation | `set_presentation()` → `apply_presentation()` | External mutation allowed and host live | Repeated identical patch is no-op |
| Clear presentation | `clear_presentation()` → `clear_presentation()` | External mutation allowed and host live | Clearing absent values is no-op |
| Set style state | `set_style_state()` → record map | Non-empty key/value | Empty key/value rejected; same key/value is no-op |
| Clear style state | `clear_style_state()` → record map | Non-empty key | Missing key is no-op |
| Capture state | `capture_candidate()` | Candidate frame attempt | Unmounted records are not visited; dirty marks remain queued |
| Read state for frame | `StateFrameView::get()` | Layout/resolution path | Overlay first, committed fallback |
| Apply pure presentation state | `LayoutTree::apply_state_snapshot()` | State effects contain no geometry and retained tree path exists | Missing state root causes incremental path to abort and use normal re-resolution |
| Apply geometry state | Local refresh or retained-root layout | Effect contains geometry | Local patch only when allocation/topology/dependency conditions prove safety |
| Build damage | `DamageRegion::from_rects()` | State/content/geometry paint metadata | Off-viewport rectangles are discarded; large/fragmented damage escalates to full |
| Dispose state | `ViewStateRegistry::dispose()` | Resource not desired/visible/in-flight | Bound resource returns `STATE_MOUNTED`; unknown/repeated disposal is no-op |
| Backend failure | Host candidate discard | Preparation or receipt error | Old visible frame/bindings remain; in-flight state pins are cleared |

### 5.2 Deterministic validation

State target validation occurs before desired binding changes:

- `ViewStateRegistry::validate_targets()` validates identity liveness and all stored geometry against the target kind (`registry.rs:175-188`);
- `ViewStateRegistry::set_desired()` calls this before changing desired membership (`registry.rs:191-202`);
- `application/kernel.rs:336-367` resolves body and History views through `ResolveSession`, collects state attachments, rejects duplicates, then returns concrete `(id, StateNodeKind)` targets.

The source contains both:

- a direct `View` helper for native state attachment traversal (`presentation/ir.rs:893-998`);
- a resolved candidate traversal that expands component indirections (`scene/resolve.rs:228-374`);
- host kernel wrappers that run the resolved traversal for body and History (`application/kernel.rs:200-250`, `336-367`).

The resolved path is necessary because a direct semantic root cannot inspect the concrete View inside a `ComponentSlot`.

### 5.3 Unsupported capability route

`validate_geometry_for_kind()` (`capabilities.rs:68-92`) rejects:

- all geometry for `ComponentSlot`;
- `gap` except on Row, Column, and Grid;
- horizontal alignment except on Text;
- vertical alignment except on Row.

The errors use the explicit diagnostic prefix:

```text
UNSUPPORTED_STATE_PROPERTY
```

The target kind is selected from `desired_kind.or(visible_kind)` during mutation validation. For an unbound state record, there is no target kind yet, so geometry can be configured while unmounted; the complete stored override is validated when the state becomes desired-bound.

This is intentional deferred validation and is tested by the remount/unmounted override scenarios.

### 5.4 Candidate miss versus recovery

The state plane distinguishes several non-success cases:

1. **No-op mutation**
   - No revision increment.
   - No dirty insertion.
   - No wake requirement from the Rust host wrapper.

2. **Invalid mutation**
   - Prospective geometry clone fails capability validation.
   - Original record remains unchanged.
   - No accepted-mutation counter or revision update.

3. **Unmounted mutation**
   - Mutable record changes.
   - Dirty ID is queued.
   - No committed snapshot is retained until capture demands the state.

4. **Unmounted capture**
   - No state snapshot is created for an unmounted record.
   - Dirty mark remains in the queue.

5. **New demand/remount**
   - Capture notices desired-but-not-visible or missing committed entry.
   - Current mutable record is snapshotted into the candidate/committed table.

6. **Missing retained physical occurrence**
   - State-only incremental path cannot find the state root or semantic path.
   - Scene host returns `None`, causing a normal candidate rebuild.

7. **Backend receipt failure**
   - Candidate state pins are released.
   - Visible state remains old.
   - The current state registry values remain available for retry.

### 5.5 Failure masking and silent fallback review

No previous-generation state mutation path was found in the inspected current Rust source. The state mutations all terminate in `ViewStateRecord`/`ViewStateRegistry`.

However, effect classification has a partial consumption characteristic:

- `SceneHost` directly checks `StateEffects::geometry()`, `intrinsic_width()`, and `intrinsic_height()`;
- the other effect bits are not individually queried anywhere in the Rust source search;
- pure presentation behavior is selected by the absence of `GEOMETRY`, not by inspecting `PAINT_SELF`, `PAINT_SUBTREE`, `DAMAGE`, or `RESOLVE_STYLE`.

This is not a second route, but it means the effect bit-set currently serves partly as declarative metadata and partly as a geometry/intrinsic control signal. A future mutation class that requires a different response cannot currently rely on an existing per-bit dispatcher without adding one.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Registry storage

`ViewStateRegistry` (`registry.rs:20-42`) contains:

```rust
records: HashMap<u64, Box<ViewStateRecord>>,
committed: HashMap<u64, Arc<ViewStateSnapshot>>,
dirty: HashSet<u64>,
capture_epoch: u64,
next_id: u64,
desired: HashSet<u64>,
visible: HashSet<u64>,
in_flight: HashSet<u64>,
```

The storage design separates:

- mutable source of truth (`records`);
- immutable frame-readable versions (`committed`);
- pending work (`dirty`);
- lifecycle/binding sets.

Records are boxed so rehashing the identity map moves one pointer rather than widening every bucket with the full mutable record (`registry.rs:22-25`).

There is one host lock, not one lock per state record.

### 6.2 Snapshot retention

Committed snapshots are retained only when a record is demanded:

- desired;
- visible;
- in-flight.

When all bindings leave, `prune_committed_if_unbound()` removes the committed snapshot (`registry.rs:410-418`).

The mutable `ViewStateRecord` remains while the state resource itself is live, allowing:

- unmounted configuration to survive;
- remounts to reconstruct the current snapshot;
- detached state not to retain duplicate immutable decorations.

A candidate overlay may retain an old `Arc` after the registry’s committed table has been pruned. This is what preserves a failed/in-flight candidate’s read view.

### 6.3 Dirty worklist and capture complexity

The dirty set is deduplicated. Multiple accepted mutations of one state before capture produce one dirty entry.

Capture work is proportional to the demanded set, not all records:

```text
demanded = desired ∪ visible ∪ in_flight
```

Only demanded IDs are sorted and inspected. Unmounted dirty records remain queued.

The current implementation allocates a temporary `HashSet` for the demanded union and a sorted `Vec<u64>` on every capture (`registry.rs:129-164`). This is deliberate bounded work over the currently demanded attachment set, but the capture path is not allocation-free.

### 6.4 Measurement cache keys

`presentation/layout/measure.rs:216-219` adds state revisions to `MeasureKey`:

```rust
key.geometry_revision = state.geometry_revision;
key.presentation_revision = state.presentation_revision;
```

Consequences:

- a state geometry mutation cannot reuse the old geometry measurement;
- a state presentation mutation also changes the measurement key on a full candidate path, even when physical metrics are expected to remain unchanged;
- the state-only fast path avoids remeasuring for pure presentation changes by directly applying the snapshot to the retained physical occurrence.

Content layout revision is independently added for ContentHost measurements (`measure.rs:220-227`).

### 6.5 State path cache invalidation

For a pure state-only update, `SceneHost` finds the state physical root and invalidates all state-to-root view IDs in both layout and paint caches (`scene/host.rs:1562-1574`).

For a structural publication arriving in the same pending period as a state mutation, `invalidate_root()` similarly invalidates the old state-to-root path before discarding the retained layout tree (`scene/host.rs:801-825`). This prevents a stable ancestor cache entry from resurrecting old descendant state values in the newly resolved root.

If a state root cannot be found, the scene host clears both caches conservatively rather than guessing a path (`scene/host.rs:1568-1574`).

### 6.6 Local geometry dependency propagation

`LayoutTree` stores per-edge `ChildDependency` metadata (`presentation/layout/tree.rs:51-98`, `place.rs:114-150`).

For state geometry refresh:

- the physical path to the state root is collected;
- each path node’s `ViewId` is invalidated;
- parent dependency metadata determines whether intrinsic width/height changes can escape the child allocation;
- local patching stops at the smallest safe ancestor;
- otherwise the retained semantic root is re-laid out.

The path and dependency mechanism avoids automatically clearing unrelated siblings. It is a key coupling between retained state and the custom layout dependency representation.

### 6.7 Paint cache invalidation

Pure state presentation invalidation:

1. invalidates state path entries in layout and paint caches;
2. applies the new snapshot to the retained `OccurrenceBox`;
3. schedules the state root for incremental paint.

Geometry invalidation:

- invalidates the affected state paths;
- performs local subtree geometry replacement when possible;
- if physical geometry differs, stores old/new damage and uses full or broader painting as needed.

`LayoutTree::patch_subtree()` preserves state attachment identity as one topology invariant (`layout/tree.rs:458-513`). A state attachment identity mismatch causes the local patch to fail.

### 6.8 Incremental paint scheduling

`SceneHost` stores:

```rust
incremental_paint_states: Vec<u64>
state_only_refresh: bool
```

State IDs are sorted by physical `LayoutNodeId` before paint (`scene/host.rs:2243-2249`). This ensures paint order follows tree order rather than state-ID allocation order.

During paint (`scene/host.rs:2058-2203`):

1. state damage rectangles are derived from state roots;
2. the previous surface is reused if incremental painting is eligible;
3. each state root is painted with `ViewPainter.paint_subtree_into_with_content()`;
4. any missing state root or failed subtree paint aborts the incremental attempt;
5. the painter falls back to full painting;
6. `ViewStateIncrementalPaints` increments only after successful incremental painting.

### 6.9 Damage algorithm

`DamageRegion` (`damage.rs:5-9`) contains:

```rust
pub(crate) struct DamageRegion {
    pub(crate) rects: Vec<Rect>,
    pub(crate) full: bool,
}
```

`DamageRegion::from_rects()` (`damage.rs:23-60`):

1. clips every rectangle to the viewport;
2. merges rectangles that touch or overlap;
3. counts merged rectangles;
4. computes total merged area;
5. escalates to full damage when:
   - merged rectangle count exceeds 64; or
   - total area is at least half the viewport area;
6. returns the entire viewport when escalated.

`DamageRegion::full()` returns:

- `full: true`;
- no rectangle for a zero-sized viewport;
- one full viewport rectangle otherwise (`damage.rs:11-20`).

The merge operation uses `swap_remove`, so merged-vector order is not stable. No ordering contract is exposed.

### 6.10 Damage dependencies

State damage is derived from:

```text
incremental_paint_states
    → LayoutTree.state_roots
    → LayoutTree.incremental_paint_rect()
    → DamageRegion::from_rects()
```

For geometry relayout, `layout_geometry_damage()` (`scene/host.rs:2324-2345`) compares old/new layout nodes and adds:

- old outer rect;
- new outer rect;

when any of these change:

- `rect`;
- `content_rect`;
- `clip_rect`;
- `OccurrenceBox`.

Thus state effective-value changes can generate damage even if the semantic `View` identity is unchanged.

### 6.11 Damage/backend boundary

`PreparedSceneFrame` carries `DamageRegion` (`scene/host.rs:129-139`), but the current terminal lowering path does not inspect it.

`terminal/termwiz/lower.rs:15-43` converts the entire `frame.surface` into terminal changes by scanning all rows. Its `desired_surface()` uses:

- `frame.surface`;
- `frame.history_overlay`;

but not `frame.damage`.

Therefore:

- damage metadata is produced and carried through the frame;
- current Termwiz lowering still scans the full physical surface;
- actual terminal diffing happens later through terminal-worker surface state, not through `DamageRegion` selection.

This is an important current-state distinction: retained-state damage is an architectural contract and future backend optimization input, but it is not currently a row-level terminal-output filter.

---

## 7. Tests, benchmarks and observability

### 7.1 Inline retained-state tests

#### Capability tests

`retained_state/capabilities.rs:98-121`

- verifies representative concrete layout kinds have presentation boxes.

#### Capture tests

`retained_state/capture.rs:129-155`

- empty view resolves nothing;
- overlay values shadow committed values;
- clean committed entries remain available without being copied into the overlay.

#### Damage tests

`retained_state/damage.rs:87-102`

- touching/overlapping rectangles merge;
- large damage escalates to full viewport damage.

#### Presentation tests

`retained_state/presentation.rs:181-234`

- explicit null differs from clear;
- sparse retained presentation preserves base background;
- foreground and text attributes overlay the base decoration.

#### Record tests

`retained_state/record.rs:179-193`

- repeated presentation assignment is a no-op;
- no revision increment occurs for an identical value.

#### Registry tests

`retained_state/registry.rs:421-685` cover:

- monotonic host-scoped ID allocation;
- initial version deferral;
- dirty-mark deduplication;
- capture draining;
- unmounted dirty-state retention;
- newly demanded state capture;
- remount capture from mutable source of truth;
- candidate overlay pinning across later mutations;
- committed-table pruning;
- clear revealing base values;
- disposal rejection while desired/in-flight;
- idempotent disposal;
- no stale version/dirty state after disposal;
- validation-before-commit for visible bindings.

These tests provide the clearest evidence of the intended lifecycle contracts.

### 7.2 Rust scene/application tests

Relevant state behavior is exercised in `application/host.rs` and `scene/host.rs`, including:

- presentation state repaint without measurement or semantic republication;
- structural publication invalidating retained-state dependency paths;
- component-slot replacement preserving captured state values;
- failed frame retaining old state versions until retry;
- in-flight presentation receipt handling;
- failed presentation recovery;
- state geometry updates through retained layout paths.

The source names and assertions were inspected, but no test command was run.

### 7.3 TypeScript/native tests

`packages/iyon-tui/tests/tui_perf13_b.test.ts` contains retained-state scenarios:

- presentation override without structural republication;
- state carried through the retained structural path;
- attachment identity surviving immutable modifiers;
- border update without box geometry change;
- unmounted override preservation;
- null versus clear;
- wrong-host and duplicate attachment rejection;
- same-host remount preservation;
- style-state selector cascade;
- disposed state rejection;
- component indirection and mounted disposal rejection;
- invalid patch atomicity.

`packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts` contains:

- state dependency-path invalidation before structural publication;
- state preservation through ViewSlot replacement.

`tui_native_input_validation.test.ts` covers malformed presentation envelopes and verifies that malformed native inputs are rejected.

### 7.4 Counters

`perf.rs:41-52` defines state-related counters:

```text
ViewStateMutationsAccepted
ViewStateMutationsNoop
ViewStatePresentationInvalidations
ViewStateStyleStateInvalidations
ViewStateIncrementalPaints
ViewStateDamageRects
ViewStateFullDamageRepaints
ViewStateGeometryInvalidations
ViewStateGeometryRelayouts
ViewStateGeometryLocalPatches
ViewStateGeometryFullRepaints
ViewStateDirtyPropagationNodes
```

Additional adjacent counters measure:

- layout nodes visited/emitted;
- component geometry nodes visited;
- content path traversal;
- content metric changes;
- content paint propagation.

No runtime counter values were observed in this investigation.

### 7.5 Observability limitations

Current visibility gaps:

1. `StateEffects` has no formatter or public bit inspection API.
2. Only geometry and intrinsic flags are queried by SceneHost.
3. There is no counter for:
   - demanded-state count per capture;
   - touched-state count per capture;
   - committed snapshot table size;
   - overlay hit versus committed fallback;
   - state candidate discard count;
   - state in-flight pin duration.
4. `DamageRegion` is carried through `PreparedSceneFrame` but not consumed by current Termwiz lowering.
5. There is no direct production diagnostic of why a local geometry patch was rejected and escalated to retained-root relayout.

---

## 8. Cross-boundary findings and contradictions

### 8.1 State is structurally attached but not structurally represented

The semantic `View` stores one optional state attachment scalar, but state mutations do not create a new semantic `View` or wrapper (`presentation/ir.rs:700-709`, `832-847`).

The physical representation is instead:

```text
semantic View attachment ID
    → LayoutTree state_roots
    → LayoutNode OccurrenceBox
```

This is the strongest retained-state seam in the current code. State semantics are separate from structural publication, but effective values still target the existing layout/presentation machinery.

### 8.2 State attachments inside components and History require resolved traversal

The ordinary `View` helper cannot traverse through `ComponentSlot` indirections because it has no component overlay. The host kernel therefore:

1. creates a `ResolveSession`;
2. resolves the candidate view;
3. obtains its component overlay;
4. collects state attachments through the resolved graph.

This path is used for:

- body publication;
- current History views;
- prospective History insertion;
- History replacement;
- History validation.

The resolved traversal also detects:

- duplicate state IDs;
- cyclic semantic View graphs;
- missing component overlays.

### 8.3 Public wrapper ownership versus registry ownership

The native/TypeScript `ViewState` object looks resource-like, but its mutable state is not owned by the wrapper.

Actual ownership:

```text
HostInner → ViewStateRegistry → ViewStateRecord
HostViewState → Weak<HostInner>
```

Consequences:

- wrapper cloning does not duplicate mutable state;
- dropping a wrapper does not dispose state automatically;
- disposal is explicit;
- wrapper GC cannot invalidate a mounted occurrence;
- host teardown is the owner-death cascade.

This matches the documented framework lifetime model.

### 8.4 State revisions are not host frame revisions

There are three distinct revision families:

1. state-record revisions:
   - `revision`;
   - `geometry_revision`;
   - `presentation_revision`;

2. structural host revisions:
   - desired structural revision;
   - visible structural revision;

3. host frame epochs:
   - pending epoch;
   - committed epoch;
   - candidate epoch.

A state revision can be accepted and placed in the registry while the prior physical frame remains visible. Backend success is required before the candidate state becomes visible.

### 8.5 Historical handoff versus current source

The PERF-13 resolved handoff describes the intended state-plane invariants:

- mutable geometry/presentation state;
- immutable state snapshots;
- Rust-owned effect classification;
- one physical occurrence;
- no semantic View rebuild for state mutation;
- desired/visible/in-flight distinction.

The current source implements those invariants, but with concrete current representations:

- native state attachments are stored as `Option<u64>` in `ViewNode`;
- state registry IDs are host-scoped u64 values;
- `ViewStateRecord` uses explicit fields rather than a generic property descriptor table;
- capability validation is an exhaustive `StateNodeKind` match;
- state effects are a u16 bit-set;
- snapshots share `Arc<ViewStateSnapshot>` values.

No `PropertyId` or `PropertyDescriptor` abstraction was found in the retained-state source or its directly inspected consumers. The current implementation is field-explicit rather than descriptor-driven.

### 8.6 Rich effects versus narrow consumer use

`StateEffects` defines fifteen effect bits (`effects.rs:3-24`):

```text
RESOLVE_STYLE
PAINT_SELF
PAINT_SUBTREE
DAMAGE
GEOMETRY
MEASURE_SELF
MEASURE_ANCESTORS
PLACE_SELF
PLACE_DESCENDANTS
UPDATE_CLIP
DAMAGE_OLD
DAMAGE_NEW
PROJECT_CONTENT
INTRINSIC_WIDTH
INTRINSIC_HEIGHT
```

Geometry classification sets different combinations according to the property. Presentation classification sets style resolution, paint, and generic damage.

Current SceneHost consumption is narrower:

- `geometry()` determines whether a geometry refresh is needed;
- `intrinsic_width()` and `intrinsic_height()` determine parent dependency escape;
- pure presentation follows when no geometry bit is set.

The source search found no direct consumers of `RESOLVE_STYLE`, `PAINT_SELF`, `PAINT_SUBTREE`, `DAMAGE`, `MEASURE_SELF`, `MEASURE_ANCESTORS`, `PLACE_SELF`, `PLACE_DESCENDANTS`, `UPDATE_CLIP`, `DAMAGE_OLD`, `DAMAGE_NEW`, or `PROJECT_CONTENT`.

This is not a contradictory route, but it is a consequential implementation fact: the bit-set is more expressive than the current dispatcher. Some bits document intended consequences or support future consumers rather than independently changing current behavior.

### 8.7 Presentation revision in measurement keys

The state-only presentation path intentionally avoids remeasurement. Nevertheless, full measurement keys include both geometry and presentation revisions (`measure.rs:216-219`).

Therefore:

- pure presentation state can invalidate full-path measured-cache entries;
- the optimized retained path avoids the cost where physical geometry is known unchanged;
- a future change that broadens presentation effects must account for both cache-key behavior and state-only patch behavior.

### 8.8 Damage metadata is not current terminal clipping

The retained state and scene layers produce rectangle damage, and the terminal frame carries it. The current Termwiz lowerer ignores that field and scans all surface rows.

This means there are two distinct optimizations:

```text
retained-state incremental paint:
    paints only affected subtrees into a reused surface

terminal output lowering:
    currently lowers the whole resulting surface
```

The first is active. The second is not currently damage-region driven.

### 8.9 Geometry and custom layout coupling

State geometry effects depend on:

- `LayoutTree.path_to_root`;
- `ChildDependency`;
- layout cache invalidation by semantic `ViewId`;
- topology-preserving subtree patching;
- physical allocation and clipping;
- semantic path reconstruction through resolved component overlays.

This is a consequential migration seam. The state values themselves are conceptually independent of a particular allocator, but current incremental geometry behavior depends heavily on custom layout-tree dependency metadata.

### 8.10 Generic boundary compliance

No Iyon/application-specific concepts were found in retained-state code. The fields and APIs are generic:

- style state is caller-defined;
- geometry and presentation values are generic terminal presentation concepts;
- no application state is hidden behind the state registry;
- no product-specific styling policy is encoded.

---

## 9. Open questions and coverage gaps

1. **Effect-bit completeness**
   - The effect table defines fifteen consequence classes, but current consumers inspect only geometry and intrinsic dimensions directly.
   - It remains unclear whether the unused bits are intentionally reserved metadata or whether some current effects are relying on broad fallback behavior.

2. **State revision rollover**
   - Revisions use saturating increment (`record.rs:87-89`, `108-109`, `123-124`, `138-139`, `150-151`, `161-162`).
   - At `u64::MAX`, further logical changes do not produce a distinct numeric revision.
   - No explicit wrap/saturation diagnostic was found.

3. **Concurrent accepted mutations**
   - The registry lock serializes individual mutations.
   - A state mutation can occur after capture but before backend receipt, producing a newer committed `Arc` while the old candidate overlay remains pinned.
   - The source tests cover this behavior, but the complete interaction with all combined structural/content mutations is not exhaustively visible from the retained-state module alone.

4. **Multiple physical occurrences**
   - The current candidate rejects repeated use of one state ID, so `state_roots` is a single `u64 → LayoutNodeId` map.
   - The source does not support one state record intentionally controlling multiple simultaneous physical occurrences.
   - Whether that is a permanent semantic rule or a current PERF-13 constraint is not represented as a type-level distinction.

5. **Presentation effects on inherited descendants**
   - Style-state changes conservatively classify as subtree paint.
   - Plain presentation fields classify as self paint.
   - The source does not expose a field-by-field selector inheritance model explaining why all non-style-state fields are safe for self-only paint.

6. **Border presentation on borderless bases**
   - Geometry `border_edges` can create a border on an undecorated occurrence.
   - Presentation `border_color`, `border_style`, and `border_glyphs` only apply when a border exists after geometry derivation.
   - The public API contract does not state whether a presentation border field should implicitly create a border.

7. **Damage consumption**
   - `DamageRegion` is carried to the terminal backend but ignored by the current Termwiz lowerer.
   - It is unclear whether other backends or future adapters consume it; no additional current consumer was found in the inspected Rust source.

8. **Capture allocation costs**
   - Capture avoids whole-registry snapshotting but still allocates a demanded-set `HashSet`, sorted demand vector, and touched map.
   - No benchmark/counter exposes these allocations directly.

9. **State path reconstruction cost**
   - Local geometry refresh reconstructs a semantic path recursively through the retained resolved scene (`scene/host.rs:2265-2308`).
   - No dedicated path index exists for state IDs comparable to the layout tree’s `state_roots` map.

10. **History-specific state residency**
    - The root scene can contain body and History branches with shared state snapshots.
    - The retained-state module itself is History-agnostic; lifecycle and History replacement semantics are coordinated by application and scene hosts.
    - A complete History state-residency analysis belongs with the History assignment.

11. **Executed validation**
    - No checks were run in this investigation.
    - Historical PERF-13 completion documents report passing focused Rust/native/TypeScript checks, but those are historical claims and not execution evidence for this run.

---

## 10. Evidence appendix

### 10.1 Primary retained-state paths and symbols

#### `retained_state/mod.rs`

- Module declarations: lines 6-14.
- Re-export boundary: lines 16-35.
- Relevant symbols:
  - `StateNodeKind`
  - `StateCandidateOverlay`
  - `StateFrameView`
  - `DamageRegion`
  - `StateEffects`
  - `EffectiveGeometry`
  - `GeometryAlignment`
  - `ViewStateGeometryPatch`
  - `ViewStateGeometryProperty`
  - `OccurrenceBox`
  - `ViewStateSnapshot`
  - `ViewStatePresentationPatch`
  - `ViewStatePresentationProperty`
  - `ViewStateLifecycle`
  - `ViewStateRecord`
  - `PreparedStateCommit`
  - `ViewStateRegistry`

#### `retained_state/capabilities.rs`

- `StateNodeKind`: lines 11-24.
- `state_node_kind()`: lines 27-42.
- `presentation_state_capable()`: lines 45-62.
- `validate_geometry_for_kind()`: lines 65-92.
- Capability test: lines 98-121.

#### `retained_state/capture.rs`

- `StateCandidateOverlay`: lines 21-33.
- Constructor and accessors: lines 35-71.
- `StateFrameView`: lines 74-127.
- Overlay lookup behavior: lines 104-113.
- Tests:
  - empty frame view: lines 129-138;
  - overlay shadows committed: lines 140-155.

#### `retained_state/damage.rs`

- `DamageRegion`: lines 5-9.
- `full()`: lines 11-21.
- `from_rects()`: lines 23-60.
- `touches()`: lines 63-68.
- `union()`: lines 70-85.
- Merge/full test: lines 87-102.

#### `retained_state/effects.rs`

- `StateEffects`: lines 3-8.
- Effect constants: lines 9-24.
- `is_empty()`, `contains()`, `union()`: lines 27-37.
- Geometry/intrinsic accessors: lines 39-49.
- `presentation_effects()`: lines 52-66.

#### `retained_state/geometry.rs`

- `ViewStateGeometryPatch`: lines 12-25.
- `GeometryAlignment`: lines 27-32.
- `GeometryOverrides`: lines 35-50.
- Patch application: lines 52-96.
- Clear operation: lines 98-122.
- Effective derivation: lines 124-168.
- Bound application: lines 171-182.
- Difference classification: lines 184-216.
- `ViewStateGeometryProperty`: lines 219-230.
- `EffectiveGeometry`: lines 233-242.
- `ViewStateSizeMode`: lines 244-264.
- Property effect mapping: lines 266-334.

#### `retained_state/occurrence.rs`

- `OccurrenceBox`: lines 12-28.
- Construction from measured/effective values: lines 30-60.
- Snapshot application: lines 62-76.

#### `retained_state/presentation.rs`

- `ViewStatePresentationPatch`: lines 12-30.
- `ViewStatePresentationProperty`: lines 32-41.
- `PresentationOverrides`: lines 44-53.
- Patch and clear: lines 55-100.
- `ViewStateSnapshot`: lines 103-115.
- Effective geometry: lines 117-131.
- Effective decoration: lines 133-170.
- Effective style states: lines 172-178.
- Tests:
  - null versus clear: lines 181-201;
  - sparse presentation overlay: lines 203-234.

#### `retained_state/record.rs`

- `ViewStateRecord`: lines 13-31.
- `ViewStateLifecycle`: lines 34-38.
- Constructor: lines 40-56.
- Snapshot: lines 59-68.
- Geometry mutation: lines 71-110.
- Presentation mutation: lines 113-140.
- Style-state mutation: lines 143-164.
- Disposal: lines 166-176.
- No-op revision test: lines 179-193.

#### `retained_state/registry.rs`

- Registry fields: lines 20-42.
- `PreparedStateCommit`: lines 44-52.
- Constructor: lines 54-65.
- Identity creation: lines 68-81.
- Mutation/publication: lines 84-120.
- Capture: lines 123-164.
- Committed lookup and record lookup: lines 167-173.
- Target validation: lines 175-188.
- Desired binding: lines 191-202.
- Visible preparation/commit: lines 205-270.
- Full candidate prepare/commit: lines 245-287.
- In-flight management: lines 290-315.
- Bound lookup: lines 317-325.
- Disposal/removal: lines 327-355.
- Binding clear and host disposal: lines 357-390.
- Internal binding/pruning helpers: lines 392-418.
- Tests: lines 421-685.

### 10.2 Adjacent state consumers

#### `application/view_state.rs`

- `HostViewState`: lines 23-35.
- Immutable `state_id()`: lines 38-43.
- Node-kind validation: lines 45-54.
- Geometry APIs: lines 56-65.
- Presentation APIs: lines 67-76.
- Style-state APIs: lines 78-92.
- Disposal: lines 94-104.
- Mutation bridge: lines 106-124.
- Numeric native node-kind mapping: lines 127-144.

#### `application/host.rs`

- Host-owned registry field: lines 123-173.
- State creation: lines 1054-1062.
- Desired View state target validation and binding: lines 1091-1120.
- State binding clear/teardown: lines 1128-1134, 1816-1818.
- Candidate state capture: lines 1702-1706.
- Registry mutation bridge: lines 1709-1717.
- Kind validation bridge: lines 1720-1728.
- Desired target refresh: lines 1730-1743.
- Candidate state commit: lines 1753-1761.
- Failed-candidate in-flight clear: lines 1763-1785.
- State invalidation bridge: lines 1787-1797.
- State disposal bridge: lines 1803-1818.
- Candidate capture and frame preparation: lines 1877-1939.
- Commit: lines 2070-2099.
- Candidate discard: lines 2170-2184.

#### `application/kernel.rs`

- Host state target collection for body/History: lines 196-250.
- Resolved state traversal and duplicate detection: lines 336-367.

#### `presentation/ir.rs`

- `ViewNode.state_attachment`: lines 700-709.
- `ViewNodeParts.state_attachment`: lines 735-743.
- State attachment accessor: lines 817-819.
- Native state attachment: lines 832-847.
- State attachment collection: lines 890-998.
- Attachment flags: lines 1880-1896.
- Semantic clone/identity preservation: lines 1918-1942.

#### `presentation/layout/measure.rs`

- `MeasuredNode`: lines 92-112.
- State revisions in `MeasureKey`: lines 216-219.
- State lookup and effective geometry: lines 264-319.
- Effective geometry used for intrinsic measurement: lines 320-370.

#### `presentation/layout/place.rs`

- Physical layout-node emission: lines 14-111.
- `OccurrenceBox` construction: lines 62-79.
- Child dependency metadata: lines 114-150.

#### `presentation/layout/tree.rs`

- `LayoutNode`: lines 100-116.
- `LayoutTree` indexes: lines 118-136.
- State/root indexing: lines 253-301.
- `apply_state_snapshot()`: lines 304-317.
- State path traversal: lines 319-328.
- Topology-preserving patch invariants: lines 458-513.
- Component patch state-index refresh: lines 516-572.
- Incremental paint geometry and clipping: lines 413-456.

#### `scene/resolve.rs`

- `ResolutionOverlay.states`: `scene/resolved.rs:13-30`.
- `ResolveSession::set_state_snapshots()`: `scene/resolve.rs:95-106`.
- State attachment target collection: lines 228-374.
- Component-overlay expansion during state collection: lines 266-280.

#### `scene/host.rs`

- State invalidation storage: lines 179-237.
- `invalidate_state()`: lines 341-350.
- Local geometry refresh: lines 641-790.
- Structural invalidation preserving state/content cache frontiers: lines 793-862.
- State-only incremental handling: lines 1533-1697.
- State path lookup: lines 2251-2308.
- Geometry equality/damage: lines 2310-2345.
- Incremental paint and state damage: lines 2053-2203.

#### `terminal/termwiz/lower.rs`

- `desired_surface()`: lines 15-33.
- Full-surface row traversal: lines 35-43.

### 10.3 Native/TypeScript boundary paths

- Native host `viewState()` creation: `crates/iyon-tui-native/src/tui.rs:839-847`.
- N-API `NativeViewState`: `crates/iyon-tui-native/src/tui/view_state.rs:25-37`.
- Geometry decode and call: lines 68-103.
- Presentation decode and call: lines 106-123.
- Native envelope decoders: lines 183-435.
- TypeScript `NativeViewStateResource`: `packages/iyon-tui/src/api/view/retained-state.ts:28-37`.
- TypeScript patch/property types: lines 39-115.
- TypeScript mutation APIs: lines 159-214.
- TypeScript state creation: lines 217-228.
- Runtime state resource ownership: `packages/iyon-tui/src/runtime/runtime.ts:576-590`.
- Runtime teardown ordering: lines 743-805.
- Mutation prohibition during retained protocol passes: lines 913-917.

### 10.4 Historical context

Historical PERF-13 material inspected:

```text
docs/history/PERF-13/PERF-13-C-completion.md
docs/history/PERF-13/PERF-13-D-completion.md
docs/history/PERF-13/PERF-13-F-completion.md
docs/history/PERF-13/PERF-13-F-implementation-notes.md
docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md
docs/history/PRE-V5/POST-PERF13-ROT-CLEANUP.md
```

Relevant historical claims corroborated by current source:

- state mutation does not republish a semantic root;
- geometry and presentation revisions are separate;
- a physical state-capable occurrence owns its own box;
- capability validation is Rust-owned and exhaustive;
- desired state and visible state are distinct;
- candidate state versions are captured and committed only with the frame;
- old/new geometry contributes damage metadata;
- failed candidates must preserve the old visible frame.

Historical documents remain context, not current-source authority.