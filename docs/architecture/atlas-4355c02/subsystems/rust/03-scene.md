# 03 — Scene — Root ownership, scene resolution, and host integration

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/scene/`
- Assignment goal: root ownership, scene resolution, host integration.
- Parent-added atlas documentation was treated as investigation guidance, not as source baseline.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- repository `AGENTS.md`
- every source and test file under `crates/iyon-tui/src/scene/`
- directly connected application/runtime seams in:
  - `crates/iyon-tui/src/application/kernel.rs`
  - `crates/iyon-tui/src/application/host.rs`
  - `crates/iyon-tui/src/component/graph.rs`
  - `crates/iyon-tui/src/component/mount.rs`
  - `crates/iyon-tui/src/lib.rs`
  - `crates/iyon-tui/src/perf.rs`

The report describes current source behavior only. It does not make V5 disposition decisions.

### Scope boundaries

This report concentrates on:

- `Scene` root ownership;
- root-level History/body composition;
- `ResolveSession`, component-slot expansion, topology discovery, and resolution errors;
- `ResolvedScene` and resolution overlays;
- `LayoutSynchronizer`;
- `SceneHost` retention, incremental resolution, history refresh, invalidation, painting, and native History pressure;
- application-kernel and native-host integration needed to establish ownership and lifecycle.

Other assignments own the detailed internals of:

- `component/` registry/component implementation;
- `presentation/` views, layout, and paint;
- `history/` storage/projection/scrollback;
- `retained_state/`;
- native/TypeScript transport and ABI.

Those modules are referenced here only at their integration seams.

### Evidence status

This was a static source investigation. I did not run tests, builds, benchmarks, or application processes. Test names and assertions are reported as source evidence, not as executed validation.

Facts, inferences, and unknowns are labelled implicitly as follows:

- **Observed** means directly present in source.
- **Derived** means an ownership or call-chain conclusion from source.
- **Unknown** means not established by the inspected source.

---

## 1. Responsibility and structure

## 1.1 Module inventory

| Path | Approximate production physical LOC | Approximate test physical LOC | Public API status | Primary responsibility |
|---|---:|---:|---|---|
| `crates/iyon-tui/src/scene/mod.rs` | 34 | 0 | `Scene` is re-exported only as `pub(crate)` from crate root | Module wiring and visibility |
| `crates/iyon-tui/src/scene/root.rs` | 440 | 0 | `Scene` itself is `pub`, but the module/re-export are crate-private | Semantic root ownership, root History/body composition, root resolution and merge |
| `crates/iyon-tui/src/scene/resolve.rs` | 614 | 0 | Internal (`pub(crate)`) | Component-slot resolution, mount graph construction, capabilities and overlays, attachment validation |
| `crates/iyon-tui/src/scene/resolved.rs` | 56 | 0 | Internal (`pub(crate)`) | Resolved scene and resolution overlay data model |
| `crates/iyon-tui/src/scene/layout.rs` | 190 | 0 | Internal (`pub(crate)`) | Scene-level layout wrapper and component layout synchronization |
| `crates/iyon-tui/src/scene/host.rs` | approximately 2,600 production lines before its `#[cfg(test)]` module | approximately 1,600+ test lines | Internal (`pub(crate)`) | Retained runtime host, invalidation, incremental resolution, native pressure, focus/ticks/outputs, painting |
| `crates/iyon-tui/src/scene/root_tests.rs` | 0 | approximately 302 | Internal tests | Root sizing, History/body composition and root mount semantics |
| `crates/iyon-tui/src/scene/tests.rs` | 0 | approximately 929 | Internal tests | Resolver, component topology, mount/focus/layout behavior |

Approximate counts are physical source-line ranges derived from the line-numbered source extraction, not nonblank/token LOC. The production/test boundary for `host.rs` is the `SceneHostError` definition followed by its `#[cfg(test)] mod tests` around line 2600. No generated files occur under this directory.

### Module responsibilities

#### `mod.rs`

`scene/mod.rs` exposes:

- `PreparedSceneFrame`
- `SceneHost`
- `SceneHostError`
- scene layout helpers
- `ResolveError`
- `ResolveSession`
- `ResolutionOverlay`
- `ResolvedScene`
- `Scene`
- root-resolution helpers

Most exports are `pub(crate)`. `Scene` is declared `pub` in `root.rs`, but the module is crate-private and `lib.rs` re-exports it as `pub(crate)`, so this is not an external Rust authoring surface.

#### `root.rs`

`root.rs` owns the semantic root abstraction:

```rust
pub struct Scene {
    history: Option<History>,
    body: View,
    layout_body: View,
    layout_root: View,
}
```

The root has exactly two conceptual branches:

1. an optional root-level `History`;
2. one ordinary body `View`.

Roots are explicitly not ordinary nested presentation values. `Scene` constructors create a root-level wrapper and maintain a separate layout-normalized body.

The module also owns:

- `ResolvedRootScene`;
- body/history branch resolution;
- History projection into a shared `ResolveSession`;
- root branch merge;
- duplicate component detection across History and body;
- root-level height allocation;
- local component-subtree resolution for retained updates.

#### `resolve.rs`

`resolve.rs` owns semantic-to-component topology resolution:

- `ResolveError`;
- `ResolveSession`;
- `Resolver`;
- `MountNode` creation;
- component snapshot lookup;
- component capability capture;
- component cycle and duplicate detection;
- state/content attachment target validation;
- content and component path indexes.

The resolver does not reconstruct a replacement `View` tree for ordinary component slots. It retains the original semantic `View` and stores component snapshots in a `ResolutionOverlay`.

#### `resolved.rs`

`resolved.rs` defines:

```rust
pub(crate) struct ResolutionOverlay {
    components: HashMap<ComponentId, ComponentSnapshot>,
    states: HashMap<u64, Arc<ViewStateSnapshot>>,
}
```

and:

```rust
pub(crate) struct ResolvedScene {
    view: View,
    mounts: MountGraph,
    capabilities: MountedCapabilities,
    overlay: ResolutionOverlay,
    content_paths: HashMap<u64, Vec<View>>,
    component_paths: HashMap<ComponentId, Vec<View>>,
    content_path_components: HashMap<ComponentId, Vec<u64>>,
}
```

It is a retained semantic resolution product, not a physical layout tree and not a second reconstructed semantic tree.

#### `layout.rs`

`layout.rs` wraps presentation layout for a resolved scene:

```rust
pub(crate) struct ResolvedSceneLayout {
    tree: LayoutTree,
    components: ComponentGeometryMap,
}
```

It owns:

- full layout of a `ResolvedScene`;
- component-subtree layout patching;
- `LayoutSynchronizer`;
- delivery of size and content-extent changes to mounted components.

#### `host.rs`

`SceneHost` is the retained native runtime host for one `Scene`. Its stated responsibility is broader than layout:

> “This module owns frame geometry, component synchronization, focus, ticks, and native History pressure. Application code supplies only semantic Scene state and consumes routed outputs.”

Observed host responsibilities include:

- retained candidate/committed scene state;
- layout and paint cache ownership;
- component mount graph/capability retention;
- component, state, content, root, and theme invalidation;
- local component subtree replacement;
- local state geometry refresh;
- local content refresh;
- History-only projection refresh;
- layout callback convergence;
- focus and interaction routing;
- tick scheduling;
- output queue routing;
- native History transfer pressure;
- full and incremental painting;
- damage-region production;
- recovery after failed frame preparation or native presentation.

---

## 1.2 Primary and secondary responsibility table

| Responsibility | Primary owner in current source | Secondary integration |
|---|---|---|
| Root semantic ownership | `Scene` in `scene/root.rs` | `RunningApp` owns the `Scene` instance |
| Body authoring value | `Scene.body` / `Scene.layout_body` | Application `ViewFn` in `RunningApp` |
| Root History ownership | `Scene.history` | `History` owns semantic units and native frontier |
| Component registry ownership | `RunningApp.components` | `SceneHost` borrows registry during resolution and callbacks |
| Mounted component identity/topology | `ResolvedScene.mounts`, then `SceneHost.graph` | `MountGraph`, `MountedComponents` |
| Component snapshots | `ResolutionOverlay.components` | `ComponentRegistry::resolution` |
| State snapshot overlay | `ResolutionOverlay.states` | `StateFrameView`, retained-state host |
| Resolved semantic scene | `ResolvedScene` | root merge and layout |
| Root branch split/merge | `resolve_root_scene_with_anchor...`, `merge_root_scene` | History projection and body resolution |
| Layout product | `ResolvedSceneLayout` | presentation layout engine |
| Component size callbacks | `LayoutSynchronizer` | `ComponentRegistry`, mounted capabilities |
| Focus and key/paste routing | `SceneHost.focus`, `dispatch_key_local`, `dispatch_paste` | `interaction/`, `RunningApp` |
| Tick scheduling | `SceneHost.ticker` | `component::TickScheduler`, application kernel |
| Frame retention | `SceneHost.retained`, `last_surface` | application host `frame`/`candidate_frame` |
| Terminal viewport | caller-supplied closure to `SceneHost::render_at_with_states` | headless sink or terminal backend |
| Physical History output | `NativeHistorySink` and `drain_native_pressure` | History native transfer and backend |
| Visible-frame transaction | application `HostInner` | `SceneHost` candidate preparation and discard |

---

## 2. Types, APIs and contracts

## 2.1 `Scene` API and invariants

`Scene` exposes these methods:

- `Scene::new(body: View) -> Scene`
- `Scene::with_history(history: History, body: View) -> Scene`
- `history(&self) -> Option<&History>`
- `history_mut(&mut self) -> Option<&mut History>`
- `body(&self) -> &View`
- `set_body(&mut self, body: View)`
- crate-private `set_history(&mut self, history: History)`

Important invariants:

1. A `Scene` has at most one root-level `History`.
2. A `Scene` always has one body `View`.
3. The root itself cannot be nested in ordinary presentation composition.
4. The `body` value is preserved as the caller-supplied semantic body.
5. `layout_body` is a clone of the body with every visited node’s width and height rewritten to `Fill`.
6. `layout_root` is a synthetic column:
   - optional History branch: `TrackSize::Flex { min: 0 }`;
   - body branch: `TrackSize::Content { max: None }`;
   - root width and height: `Fill`.

The distinction between `body` and `layout_body` is consequential. `RunningApp::prepare_frame_with_states` compares the freshly generated body against `scene.body()` and calls `set_body` when changed. Incremental host resolution compares retained `body_view` against `scene.layout_body()` by `View::ptr_eq`, so the layout-normalized clone is the structural identity used to determine whether the body branch changed.

### Root sizing contract

The body is measured at terminal width, then clamped to terminal height:

```text
body_height = min(measure(body, terminal_width).height, terminal_height)
history_height =
    terminal_height - body_height if History exists
    0 otherwise
```

The History branch therefore receives all remaining height. A body whose intrinsic height exhausts the viewport receives the full terminal height and leaves History at zero height, but History components remain semantically mounted and resolved.

The root layout intentionally prevents a narrow body from narrowing History: `layout_body` is rewritten to fill width/height before it enters the synthetic root column.

## 2.2 `ResolvedRootScene`

`ResolvedRootScene` is internal and contains:

```rust
pub(crate) struct ResolvedRootScene {
    scene: ResolvedScene,
    body_scene: ResolvedScene,
    history_scene: Option<ResolvedScene>,
    history_components: HashSet<ComponentId>,
    body_view: View,
    history_overlay: Option<HistoryPhysicalOverlay>,
    history_overflow_rows: usize,
    history_height: u16,
    body_height: u16,
}
```

The separate branch products are intentional:

- `body_scene` can be retained for body-local component updates;
- `history_scene` can be replaced for History-only projection updates;
- merged `scene` is the layout/paint representation;
- `history_components` distinguishes invalidations in History from body invalidations;
- `history_overlay` carries frozen physical rows that are painted over the semantic surface;
- `history_overflow_rows` controls native scrollback pressure;
- `body_height` and `history_height` encode root allocation.

This is not merely a convenience wrapper. The host’s incremental routes depend on retaining these branch boundaries.

## 2.3 `ResolveError`

`ResolveError` has three variants:

```rust
pub(crate) enum ResolveError {
    MissingComponent { id: ComponentId },
    DuplicateComponent { id: ComponentId },
    ComponentCycle { path: Vec<ComponentId> },
}
```

Their contracts are distinct:

- `MissingComponent`: a semantic `ComponentSlot` references an ID absent from the registry.
- `DuplicateComponent`: the same component identity appears more than once in one resolution domain or across merged History/body roots.
- `ComponentCycle`: recursive component snapshots form a cycle; the returned path includes the cycle.

`ResolveError` implements `Display` and `Error`.

## 2.4 `ResolveSession`

`ResolveSession<'a>` borrows a `ComponentRegistry` and owns one resolution transaction:

- `new(registry)`
- `resolve_root(view)`
- `resolve_root_with_dependencies(view)`
- `overlay()`
- `set_state_snapshots(states)`
- test-only `nodes_visited()`
- `finish(view)`

`resolve_root` scans component-bearing semantic nodes and returns a clone of the original `View`. It does not substitute component views directly into the semantic root. `finish` packages:

- the original semantic view;
- `MountGraph`;
- mounted capabilities;
- component/state overlays;
- content/component path indexes.

`resolve_root_with_dependencies` returns sorted `(ComponentId, ComponentRevision)` pairs for exact reachable component dependencies. The source comments say this is intended to support equality/cache keys where semantic view equality alone is insufficient.

## 2.5 `ResolutionOverlay`

The overlay is the indirection seam allowing layout, state validation, and path indexing to enter `ComponentSlot` contents without rebuilding the main `View`.

- `components` maps `ComponentId` to cloned `ComponentSnapshot`.
- `states` maps native attachment identity (`u64`) to shared `Arc<ViewStateSnapshot>` values.

`set_state_snapshots` copies only the demanded IDs from `StateFrameView`, retaining shared `Arc`s. The source explicitly says this avoids repeated map and decoration copies of older per-branch snapshot cloning.

## 2.6 `ResolvedScene`

`ResolvedScene` combines semantic and retained topology products.

Its indexes are:

- `content_paths`: ContentPort ID → semantic path of `View` Arc clones;
- `component_paths`: Component ID → semantic path to a component slot;
- `content_path_components`: component ID → affected ContentPort IDs.

`PartialEq` intentionally compares only `view` and `mounts`; it excludes capabilities, overlays, and path indexes. This means scene semantic/topology equality is not a complete equality of all retained derived state.

## 2.7 Attachment target APIs

Under the `native-host` feature:

- `state_attachment_targets(view, overlay) -> Result<Vec<(u64, StateNodeKind)>, String>`
- `content_attachment_targets(view, overlay) -> Result<Vec<u64>, String>`

Both recursively expand component slots through the overlay. Both track active `ViewId`s to detect cyclic semantic graphs. Both reject duplicate attachment identities.

Content validation additionally rejects a content identity attached to anything other than `ViewKind::ContentHost`:

```text
UNSUPPORTED_CONTENT_PORT_ATTACHMENT
DUPLICATE_CONTENT_PORT_ATTACHMENT
```

State validation rejects duplicate state identity attachment with:

```text
DUPLICATE_VIEW_STATE_ATTACHMENT
```

The application kernel invokes these helpers against prospective body/History views before publishing native-host desired state. This is an important boundary: ordinary root resolution itself can produce overlays, while native-host publication validates complete attachment identity across the candidate.

## 2.8 `LayoutSynchronizer`

`LayoutSynchronizer` retains two per-component delivery maps:

```rust
delivered: HashMap<ComponentId, Size>
delivered_content_extents: HashMap<ComponentId, Size>
```

It reports `LayoutSync::Stable` or `LayoutSync::Dirty`.

For each mounted component with geometry:

- invokes `layout_changed` only when allocated content size changes;
- invokes `content_extent_changed` only when full content extent changes;
- tracks the two callback domains independently;
- removes delivery records when the callback is absent or the component disappears from the graph.

A callback may mutate the registry. If any callback causes a dirty result, `SceneHost` abandons the current retained candidate and performs another authoritative full pass. This is the convergence mechanism for callback-driven layout changes.

## 2.9 `SceneHost` API

`SceneHost` is crate-private. Principal methods include:

- `Default::default`
- `clear_retained_views`
- `invalidate_component`
- `has_invalidated_components`
- `invalidate_state`
- `invalidate_content`
- `invalidate_theme`
- `commit_content_candidate`
- `abort_content_candidate`
- `discard_candidate`
- `next_tick_deadline`
- `focused_component`
- `dispatch_key_local`
- `intercept_paste`
- `dispatch_paste`
- `drain_outputs`
- `tick_due`
- `render` (test-only)
- `render_at`
- `render_at_with_states`

The normal production frame entrypoint is:

```rust
SceneHost::render_at_with_states(
    now,
    &mut Scene,
    &mut ComponentRegistry,
    &Theme,
    sink,
    viewport,
    states,
    content,
)
```

Caller-supplied values include:

- semantic `Scene`;
- component registry;
- theme;
- native History sink;
- viewport callback;
- state frame;
- content provider.

The host owns generic terminal mechanics and does not interpret product/application semantics. This matches the framework boundary in `AGENTS.md`.

## 2.10 `PreparedSceneFrame`

`PreparedSceneFrame` contains:

```rust
pub(crate) struct PreparedSceneFrame {
    surface: Surface,
    history_overlay: Option<HistoryPhysicalOverlay>,
    damage: DamageRegion,
    state_bindings: Vec<(u64, StateNodeKind)>,
}
```

`state_bindings` represents all state attachment identities encountered in the fully prepared candidate. The application host uses these bindings to prepare state lifecycle promotion and retain in-flight pins until backend presentation succeeds.

`screen_lines()` materializes visible surface text and overlays frozen History rows at their physical row positions.

---

## 3. Dependency and ownership map

## 3.1 Ownership graph

```text
RunningApp<State, Action, ...>
│
├── owns Scene
│   ├── optional History
│   ├── body View
│   ├── layout_body View
│   └── layout_root View
│
├── owns ComponentRegistry
│   └── actual Component values and revisions
│
├── owns SceneHost
│   ├── committed MountGraph/capabilities
│   ├── retained StableScene candidate
│   ├── LayoutCache/PaintCache
│   ├── focus/ticker/output queue
│   └── last painted Surface
│
└── owns application-level state/update/view function

SceneHost
│
├── borrows Scene and ComponentRegistry during frame preparation
├── creates ResolveSession
│   ├── reads ComponentRegistry::resolution
│   ├── captures ComponentSnapshot clones
│   ├── creates MountGraph
│   └── captures MountedCapabilities
│
├── creates ResolvedRootScene
│   ├── body ResolvedScene
│   ├── optional History ResolvedScene
│   └── merged ResolvedScene
│
├── asks presentation layout to create ResolvedSceneLayout
├── synchronizes layout callback delivery
├── updates committed interaction/tick graph after preparation succeeds
└── asks presentation paint to create Surface
```

## 3.2 Create/own/destroy table

| Object | Created by | Owner during normal lifetime | Destroy/reclaim condition |
|---|---|---|---|
| `Scene` | `RunningApp::start` | `RunningApp` | application/runtime teardown |
| `History` inside Scene | application/kernel or host initialization | `Scene` | replacing/dropping Scene |
| Body `View` | application `ViewFn` | `Scene` after `set_body` | body replacement or Scene drop |
| `Component` value | `ComponentRegistry::register` | `RunningApp.components` | deferred retirement after successful unmount proof |
| `ComponentSnapshot` | registry resolution | `ResolutionOverlay` for candidate | candidate/branch drop |
| `MountGraph` resolution | `ResolveSession::finish` | `ResolvedScene`, then `StableScene` | replacement/drop of retained candidate |
| Committed mount graph | successful host reconciliation | `SceneHost.graph` | successful replacement or host clear |
| `StableScene` | full/incremental resolution | `SceneHost.retained` | replacement, paint commit, discard |
| `ResolvedSceneLayout` | scene layout wrapper | `StableScene.layout` | candidate replacement |
| `Surface` | `ViewPainter` | `SceneHost.last_surface` and prepared frame | repaint or host clear |
| Physical History overlay | History projection | `ResolvedRootScene` / prepared frame | next History projection |
| State frame snapshots | state host | `ResolutionOverlay.states` via shared `Arc` | candidate replacement or state revision |
| Content dependency index | resolved scene/tree and `SceneHost.retained_content_dependencies` | `SceneHost` | root replacement/clear |

### Critical lifetime distinction

The component registry owns actual component values. `SceneHost` owns only:

- stable IDs;
- revisions;
- snapshots;
- capabilities;
- mount topology;
- retained derived products.

This permits deferred component reclamation. `RunningApp::host_retire_component` queues raw IDs; `reap_retired_components` consults `SceneHost::is_mounted`, which checks the last successfully reconciled graph. A failed frame cannot cause premature component destruction.

## 3.3 Forward dependency edges

```text
scene/root.rs
  → History projection and native History types
  → ComponentRegistry / ComponentId
  → ResolveSession / ResolvedScene
  → presentation IR and layout measurement
  → retained state frame

scene/resolve.rs
  → ComponentRegistry / ComponentSnapshot / ComponentRevision
  → MountGraph / MountNode
  → MountedCapabilities
  → presentation View / ViewKind / ViewId
  → retained state node classification
  → perf counters

scene/layout.rs
  → presentation layout engine
  → ComponentGeometryMap / LayoutCache / LayoutTree
  → ComponentRegistry for callbacks
  → MountGraph / MountedCapabilities

scene/host.rs
  → root resolution/merge
  → resolve session and component subtree resolution
  → presentation layout/compiler/painter
  → History projection and native transfer
  → component mount/reconciliation/ticks
  → interaction focus/key/paste
  → output queue/router
  → retained state
  → physical Surface
  → Theme
```

## 3.4 Reverse dependencies

- `application/kernel.rs` owns and invokes `SceneHost`.
- `application/host.rs` converts backend/headless viewport and sink behavior into `SceneHost` calls.
- component lifecycle/reclamation depends on `SceneHost::is_mounted`.
- native-host state/content APIs invalidate through `RunningApp`, then `SceneHost`.
- terminal and headless backends consume `PreparedSceneFrame`.
- presentation layout consumes `ResolvedScene`.
- presentation paint consumes `ResolvedSceneLayout` and host interaction state.

## 3.5 Root ownership versus host ownership

The root semantic model is split cleanly:

```text
Scene owns caller semantic intent:
    History + body View

ResolveSession owns one candidate's topology:
    snapshots + mounts + capabilities + attachment indexes

SceneHost owns retained runtime products:
    candidate/committed resolution, layout, paint, interaction, scheduling

RunningApp owns process/application lifetime:
    registry, update/view function, actions, SceneHost, Scene

application::HostInner owns visible-frame authority:
    last committed PreparedSceneFrame
    in-flight candidate frame
    backend receipt and commit metadata
```

The application host’s `HostInner` is the final visible authority. `SceneHost` can prepare a candidate, but it does not by itself make that candidate visible to the terminal.

---

## 4. Execution paths and state transitions

## 4.1 Initial frame path

The primary frame path is:

```text
RunningApp::start
  → create Scene::new(...) or Scene::with_history(...)
  → SceneHost::default
  → application::host prepare_frame
  → RunningApp::prepare_frame_with_states
  → if body_dirty:
       call application ViewFn
       Scene::set_body
  → SceneHost::render_at_with_states
  → obtain viewport from headless sink or TerminalBackend::viewport
  → try incremental path; no retained frame initially
  → resolve_full_stable
  → resolve_root_scene_with_anchor_and_cache_and_states_and_content
  → resolve body branch
  → measure body branch
  → project History branch if present
  → merge_root_scene
  → layout_resolved_scene_with_cache_and_content
  → LayoutSynchronizer
  → mount/focus/tick reconciliation
  → ViewPainter
  → PreparedSceneFrame
  → application HostInner candidate frame
  → backend presentation
  → visible-frame commit
```

`RunningApp::prepare_frame_with_states` clears `body_dirty` before calling the host. If frame preparation fails, the application host retains the old visible frame and later calls `host_discard_candidate`; the desired state remains dirty/pending at the application host layer.

## 4.2 Full root resolution

`resolve_root_scene_with_anchor_and_cache_and_states_and_content` performs:

1. Resolve `root.layout_body()` independently.
2. Measure resolved body at terminal width.
3. Clamp body height to terminal height.
4. Compute remaining History height.
5. If History exists:
   - create one `ResolveSession`;
   - install state snapshots;
   - call `project_into_session_for_host_with_content`;
   - finish the History `ResolvedScene`;
   - collect History component IDs.
6. Merge History and body in visual order.
7. Return branch and merged metadata.

Body resolution and History resolution intentionally use separate branch products but can share the root-level component registry. The History path uses a fresh session in `root.rs`, then `merge_root_scene` rejects component identity overlap with the body.

## 4.3 Component resolution path

The resolver uses cached `ViewFlags` to avoid walking component-free branches:

```text
ResolveSession::resolve_root
  → Resolver::scan_view
  → if !view.contains_component_identity():
       stop at this branch
  → ViewKind::ComponentSlot
  → Resolver::resolve_slot
  → ComponentRegistry::resolution(id)
  → MissingComponent on absent ID
  → active-stack test for ComponentCycle
  → seen-set test for DuplicateComponent
  → append MountNode { id, parent, revision }
  → copy capabilities
  → copy ComponentSnapshot into overlay
  → recurse into snapshot.view
```

The `parent` of a top-level component slot is `None`. A component slot nested inside another component snapshot is mounted with the enclosing component as parent.

The resolver does not replace the slot in the main `View`. Layout and auxiliary traversals enter the snapshot through `ResolutionOverlay`.

## 4.4 Root merge path

`merge_root_scene` has two routes.

### Body-only route

When History is absent:

- clones `layout_root`;
- prefixes all body content/component paths with the synthetic root view;
- retains body mounts/capabilities/overlay;
- returns a root-level `ResolvedScene`.

### History-plus-body route

When History is present:

1. `ensure_disjoint_mounts` compares History and body component IDs.
2. History and body mount nodes are concatenated in History-then-body order.
3. Capabilities and component overlays are combined.
4. A new synthetic root column is created with:
   - History flex track first;
   - body content track second.
5. Content and component paths from each branch are prefixed with the new root.
6. Reverse component/content path indexes are combined and deduplicated.

The mount order is observable and tested: History components precede body components.

## 4.5 Layout and synchronization path

`layout_resolved_scene_with_cache_and_content` calls presentation layout against the merged resolved semantic root and returns:

- `LayoutTree`;
- `ComponentGeometryMap`.

Then `SceneHost::resolve_stable_at_with_anchor` invokes `LayoutSynchronizer`.

For a full candidate:

```text
ResolvedRootScene
  → ResolvedSceneLayout
  → LayoutSynchronizer::synchronize
  → per-component layout_changed/content_extent_changed callbacks
```

For a topology-preserving incremental candidate, only affected components are synchronized through `synchronize_component`.

A callback-induced `LayoutSync::Dirty` causes:

- `force_full = true`;
- retained candidate discard;
- incremental plan reset;
- another pass through full resolution.

The loop is bounded by `MAX_LAYOUT_PASSES = 8`. Exceeding the bound returns `SceneHostError::DidNotConverge`.

## 4.6 Retained component replacement path

For component invalidations, `try_incremental_stable`:

1. Reads invalidated IDs.
2. Filters out IDs not present in the retained scene.
3. Excludes History IDs when selecting body-local invalidations.
4. Removes invalidated descendants whose ancestor is already invalidated.
5. Calls `prepare_component_subtree_update` for each root.
6. Re-resolves changed component snapshots.
7. Applies updates to the retained overlay/mount graph/path index.
8. Attempts same-shape component layout patching.
9. If shape/topology changes, performs a full retained-root layout.
10. Marks affected component IDs for incremental paint when safe.

If the retained component subtree cannot be patched safely, the host falls back to full layout while still retaining cache entries for unaffected siblings where possible.

A replacement is staged in `StableScene`; the committed host graph is updated only after resolution, synchronization, and convergence are successful.

## 4.7 History-only refresh path

When only History semantic/native revisions change and body identity/topology remains stable:

```text
SceneHost::try_incremental_stable
  → detect history_revision/native_history_revision change
  → take retained StableScene
  → refresh_history_projection
  → reuse retained body_scene
  → recompute History height/projection
  → merge new History with old resolved body
  → recompute merged layout
  → mark history-only paint or full paint
```

`refresh_history_projection` may also synchronize History components whose geometry changed even if their component revisions did not. This is important for controls whose viewport allocation changes because History height changed.

If History topology and geometry remain stable, the host can paint only the History subtree. If body geometry moves or History topology changes, the host forces a full paint and may require full host synchronization.

## 4.8 State invalidation path

`invalidate_state(id, effects)` records:

- state ID in `invalidated_states`;
- unioned `StateEffects` in `invalidated_state_effects`.

When no structural/body/History changes are present:

- state snapshots are copied into retained overlays;
- state-specific layout and paint cache paths are invalidated;
- presentation-only state updates use local retained-tree state application and subtree paint;
- geometry-affecting updates attempt local geometry refresh;
- if local geometry cannot be proven safe, the retained semantic root is reused but layout is recomputed;
- full paint is selected if geometry changed.

A geometry patch can be local only when the target remains within its committed allocation. If a changed intrinsic width/height can escape parent dependency constraints, the host climbs to the parent frontier or recomputes the root layout.

## 4.9 Content invalidation path

`invalidate_content(ContentDirty)` records one port ID and an epoch. Measurement and paint dirtiness are tracked independently:

- measurement dirtiness invalidates layout dependency paths;
- paint dirtiness invalidates paint paths;
- presentation-only content changes avoid unnecessary ancestor measurement;
- source/delivery changes can trigger metric evaluation and dependency-frontier relayout.

The host retains:

- `content_dirty` records;
- `content_dirty_epoch`;
- `content_prepared_epoch`;
- semantic path indexes in `ResolvedScene`;
- physical dependency roots in the retained `LayoutTree`;
- fallback `retained_content_dependencies` across structural invalidation.

For body-only content changes with stable History:

- `try_local_content_refresh` probes affected ContentHost metrics;
- a stable allocation is patched in place;
- a height/width/completeness change escalates to normal layout;
- successful local patch marks affected content ports for incremental paint.

History content changes conservatively use the normal root/History projection path because History transfer and receipt ownership are separate.

## 4.10 Theme invalidation path

`invalidate_theme`:

- clears `PaintCache`;
- invalidates content-related layout entries as needed;
- marks the theme dirty;
- queues a full paint obligation;
- does not rebuild semantic structure;
- intentionally excludes theme revisions from content layout-input keys because current theme changes are treated as metric-neutral.

The host can therefore reuse semantic/layout products while repainting with a new palette.

## 4.11 Interaction and tick path

Key and paste dispatch use the committed host graph/capabilities:

```text
SceneHost::dispatch_key_local
  → route_key_local
  → FocusState
  → MountGraph
  → MountedCapabilities
  → ComponentRegistry mutation
  → OutputQueue
```

Paste has a separate interception route before normal dispatch.

`tick_due` invokes `TickScheduler`. Any changed component IDs returned from tick callbacks are also inserted into `invalidated_components`; the source explicitly notes that registry mutation alone is insufficient to drive retained scene reconciliation.

Application-level action processing in `RunningApp::advance_ready` separately marks the body dirty so the application view function can regenerate the semantic body.

## 4.12 Native History pressure path

`SceneHost::render_at_with_states` repeatedly:

1. obtains viewport size;
2. resolves a candidate;
3. checks History overflow rows;
4. drains native pressure if needed;
5. retains the candidate when native transfer makes progress;
6. repeats resolution, reusing the retained body branch;
7. if the sink is blocked, re-resolves with `HistoryViewportAnchor::NativeFrontier`;
8. paints the pinned candidate.

The drain loop is bounded by the candidate’s overflow-row budget and can consume multiple History units before re-resolving.

Special cases:

- no overflow: paint immediately;
- no History: paint immediately;
- front ContentPort transfer blocked: paint without spinning;
- native synchronization unknown after a prior failed transfer: do not emit duplicate rows; paint a recovery frame;
- semantic blocker after physical progress: force a re-resolution before painting, avoiding stale pre-transfer geometry.

---

## 5. Alternate routes and failure semantics

## 5.1 Semantic operation to production path

| Semantic operation | Production path | Selection condition | Failure/recovery behavior |
|---|---|---|---|
| Create body-only root | `Scene::new` → root resolution | no History | body-only root, no History overlay |
| Create History root | `Scene::with_history` → root resolution | caller supplies History | History gets remaining root height |
| Replace body | `Scene::set_body` → host body identity comparison | application view changed | full/root path unless a safe retained route applies |
| Resolve component slot | `ResolveSession` → `Resolver::resolve_slot` | `ViewFlags` says component identity present | missing/duplicate/cycle error |
| Resolve component-free branch | `Resolver::scan_view` stops | `contains_component_identity == false` | no recursive topology scan |
| Validate ContentPort | `content_attachment_targets` | native-host candidate validation | string error for unsupported/duplicate attachment |
| Validate state attachment | `state_attachment_targets` | native-host candidate validation | string error for cyclic/duplicate/missing overlay |
| Initial frame | full resolve → layout → sync → paint | no retained frame | `SceneHostError` on failure |
| Component update | local subtree update | invalidated mounted body component | fallback to full layout if patch unsafe |
| History update | History-only projection refresh | body stable; History revision/native revision changed | body branch reused; full paint if geometry/topology changes |
| State presentation update | retained state apply + subtree paint | no structural/geometry escalation | fallback full/root if path unavailable |
| State geometry update | local geometry patch | fixed allocation remains safe | parent/root relayout when geometry can escape |
| Content paint-only update | retained content subtree paint | metrics unchanged | full/root if patch cannot prove safety |
| Content metric update | local content probe/frontier | body-only and path retained | full/root layout if metrics change |
| Theme change | paint invalidation/repaint | theme only changed | semantic root retained |
| Native History pressure | `drain_native_pressure` | `history_overflow_rows > 0` | progress re-resolve; blocked NativeFrontier anchor |
| Key input | local interaction router | committed graph/focus | callback mutation becomes invalidation |
| Paste | interceptor then normal route | registered interceptor/focused component | output/action route; callback mutation invalidates |
| Tick | `TickScheduler::tick_due_with_events` | timer deadline reached | changed IDs invalidate components |
| Frame backend failure | application host discard path | preparation/presentation/receipt failure | old visible frame remains authoritative |

## 5.2 Resolution errors

The resolver fails explicitly rather than silently falling back:

- missing registry entry returns `MissingComponent`;
- duplicate identity returns `DuplicateComponent`;
- recursive active-stack identity returns `ComponentCycle`.

`root.rs` adds a cross-branch duplicate check after independently resolving History and body. This matters because each branch’s `ResolveSession` can otherwise see a component only once locally.

The source tests include:

- missing slots;
- duplicate slots;
- self and multi-component cycles;
- duplicate History/body component identities;
- failed resolution preserving previous mount state.

## 5.3 Candidate versus committed state

A central transaction boundary exists between:

```text
candidate SceneHost state
    and
last successfully prepared/painted/presented state
```

`SceneHost` has:

- `retained: Option<StableScene>`;
- `last_surface`;
- committed `graph` and `capabilities`;
- pending invalidation state.

The application host has:

- `frame`: last complete logical frame;
- `candidate_frame`: prepared but not yet visible candidate;
- backend `PresentReceipt`;
- candidate state/content commit metadata.

If preparation or presentation fails:

1. application host captures failure metadata;
2. candidate frame is discarded;
3. `SceneHost::discard_candidate` invalidates retained candidate and clears caches;
4. content/state candidate commits are aborted;
5. old visible frame remains authoritative;
6. retry remains pending.

This is an observed two-level transaction, not merely an error return.

## 5.4 Native transfer failure

Native History transfer is irreversible at the physical boundary. If a sink fails after partial physical application:

- History records native synchronization as unknown;
- the host avoids retrying the same rows immediately;
- the candidate is painted as a recovery frame;
- application host marks physical synchronization failure;
- later recovery is explicit.

This protects against duplicate terminal scrollback emission but introduces an important semantic mode: the semantic History and physical terminal frontier may temporarily be out of sync.

## 5.5 Convergence failure

Layout, focus, or content callbacks can mutate upstream state while preparing a candidate. The host reruns up to eight passes. If no stable candidate emerges:

```rust
SceneHostError::DidNotConverge
```

The native host maps this to `LAYOUT_DID_NOT_CONVERGE` and marks it non-retryable in the inspected mapping.

## 5.6 Silent fallback search result

Within `scene/` the major alternate routes are explicit:

- local patch returns `None` or `false`;
- caller falls back to full retained-root layout or full root resolution;
- failed resolution returns an error;
- failed merge drops the candidate;
- failed History transfer produces explicit pressure/error handling.

I did not find a route in the inspected Scene code that silently substitutes an empty component, silently ignores a missing slot, or silently paints a stale pre-transfer candidate after known physical progress. The source contains conservative fallback-to-full-work behavior rather than compatibility substitution.

One caveat is that `merge_root_scene` combines `overlay.states` with map insertion and does not itself reject cross-branch duplicate state IDs. Complete state-target validation is performed by native-host/application preparation paths. Direct internal callers that bypass that validation would not receive the same duplicate-state guarantee.

---

## 6. Caches, invalidation, scheduling and performance

## 6.1 Resolution indexes

`ResolveSession::finish` builds indexes once per resolved branch:

- `content_paths`: keyed by ContentPort identity;
- `component_paths`: keyed by ComponentId;
- reverse ContentPort-to-component mapping.

The path values are cheap `View` Arc clones, not a second semantic tree. The source explicitly uses the path index so content dirtiness does not recursively scan unrelated siblings on every notification.

Counters:

- `ResolverNodesVisited`
- `ContentPathIndexNodesVisited`

Resolver traversal short-circuits at branches whose cached `ViewFlags` show no component identity.

## 6.2 Layout cache

`SceneHost` owns `LayoutCache`.

Observed invalidation operations include:

- begin a new layout epoch;
- invalidate individual `ViewId`s;
- invalidate content entries;
- clear all layout entries;
- preserve clean sibling entries across root invalidation where safe.

The host intentionally avoids blanket cache clearing for narrow content/theme changes. It clears both layout and paint caches when state invalidation is combined with structural/component changes because parent cache entries do not encode all descendant state revisions.

## 6.3 Paint cache and surface retention

`SceneHost` owns `PaintCache` and `last_surface`.

Incremental painting attempts to reuse `last_surface` when:

- History subtree is independently paintable;
- affected component subtrees can be painted in place;
- state subtree paths are available;
- content repaint roots are known.

If an incremental paint target is missing or unsafe, the host falls back to full-tree paint.

`full_paint_pending` is retained when geometry changes may have moved siblings. This prevents stale cells from the previous surface remaining outside an overly narrow repaint rectangle.

## 6.4 Content dirty epochs

Content invalidation uses monotonically increasing epochs:

- `content_dirty_epoch`;
- per-port `ContentDirtyRecord { epoch, measurement, paint }`;
- `content_prepared_epoch`.

Newer invalidations arriving while a candidate is in flight survive that candidate and are not accidentally committed with an older frame. `commit_content_candidate(epoch)` removes only records at or below the committed candidate epoch.

This is a host-level scheduling and visible-frame contract, not just a cache detail.

## 6.5 State and geometry scheduling

State invalidations accumulate by ID and union their effects. Geometry states are sorted by retained tree order before paint to produce deterministic ancestor/descendant paint behavior.

The host tracks:

- local state geometry relayouts;
- local geometry patches;
- full geometry repaints;
- dirty propagation nodes;
- state damage rectangles.

## 6.6 Native pressure scheduling

The native pressure drain loops within `overflow_rows`, allowing multiple one-line History units to transfer before re-resolving. This avoids a full Scene resolve for each physical row.

On transfer progress:

- the current candidate is retained;
- the next pass can reuse the body branch;
- only History projection/frontier work is repeated.

On a blocked frontier:

- no rows are discarded;
- a NativeFrontier anchor is used;
- the host paints the pinned state rather than looping indefinitely.

## 6.7 Per-operation work summary

| Work | Frequency |
|---|---|
| Component topology scan | full resolution or component subtree replacement; short-circuits component-free branches |
| Content path index | once per resolved branch |
| Body measurement | full root resolve; History-only refresh only if affected body geometry requires it |
| History projection | initial root resolve or History-only refresh |
| Full layout | initial/root/unsafe incremental fallback |
| Local component layout | component invalidation when same-shape patch is possible |
| Local state layout | state geometry invalidation when fixed allocation is safe |
| Local content measurement | dirty ContentPort body refresh |
| Full paint | initial frame, theme/full-paint obligation, unsafe incremental target, geometry movement |
| Incremental paint | History subtree, component subtree, state subtree, or ContentPort repaint root |
| Focus/tick reconciliation | full candidate or targeted component update |
| Native transfer | only when resolved History projection reports overflow |

No direct runtime counter values were collected because no tests or benchmark suites were executed.

---

## 7. Tests, benchmarks and observability

## 7.1 Root tests

`crates/iyon-tui/src/scene/root_tests.rs` covers:

- body component resolved once per root pass;
- body-only root has no History overlay;
- History/body remaining-height allocation;
- History consuming all space left by intrinsic body;
- FollowEnd History placement above body;
- body width not narrowing History;
- body exhaustion producing zero-height History while retaining mounted components;
- duplicate component identity across History/body;
- root mount order History then body;
- zero dimensions preserving semantic resolution;
- frozen History overlay staying within History track.

These tests establish root contracts rather than only visual snapshots.

## 7.2 Resolver/component tests

`crates/iyon-tui/src/scene/tests.rs` covers:

- snapshot caching by revision;
- component-free resolution short-circuit;
- static scene equality with no mounts;
- component ownership and nested component re-read;
- hanging-prefix/body component handling;
- duplicate slot rejection;
- failed resolution preserving previous mount state;
- missing slots;
- component cycle path reporting;
- typed-handle identity isolation;
- snapshot metadata not creating live mounts;
- layout callback initial/change-only delivery;
- geometry visibility versus mounting;
- focus exclusion for fully clipped components;
- clipped components remaining semantically mounted.

These tests show that semantic mounting is independent from visibility and physical clipping.

## 7.3 Host tests

The `host.rs` test module covers, among other routes:

- incremental update of one component among many;
- topology replacement preserving owner and mount state;
- failed incremental preparation preserving committed frame;
- ancestor surface background preservation during component paint;
- History-only refresh without rebuilding body;
- state overlay replacement and old-candidate pinning;
- content path index update;
- content dirty coalescing/escalation;
- retained ContentHost metric refresh;
- theme repaint obligations surviving content refresh;
- geometry changes with History forcing full paint;
- History transfer and scroll-pane detachment;
- state crossing component boundaries;
- focus callback convergence;
- framework focus variants and focus-within;
- parent focus not leaking to nested child;
- layout generated from stable convergence pass;
- semantic blockers and native transfer pressure;
- physical progress followed by blocker forcing re-resolution.

The source contains assertions against performance counters, including:

- resolver node limits;
- component view call limits;
- measure/layout traversal limits;
- `ContentPathIndexNodesVisited == 0` during indexed local refresh.

No test was run for this report.

## 7.4 Observability

Scene-specific counters are exposed through `perf.rs`, including:

- `ResolverNodesVisited`
- `ComponentViewCalls`
- `ComponentCapabilityCalls`
- `ViewStateGeometryInvalidations`
- `ViewStateGeometryRelayouts`
- `ViewStateGeometryLocalPatches`
- `ViewStateGeometryFullRepaints`
- `ViewStateDirtyPropagationNodes`
- `ContentPathIndexNodesVisited`

History pressure has trace hooks:

- `trace_transfer`
- `trace_resolve_pressure`

`SceneHost` test-only fields include:

- `resolve_count`
- `full_resolves`
- `full_paints`
- `incremental_resolves`

These provide useful route-level observability, but production users do not receive a public SceneHost metrics API from the inspected scope.

---

## 8. Cross-boundary findings and contradictions

## 8.1 Scene is generic framework machinery

The Scene code is generic:

- History is caller-supplied;
- body `View` is caller-supplied;
- components are generic registry values;
- content ports are identity-based generic attachments;
- output and interaction routing remain caller-defined;
- theme is supplied from outside;
- no application/product meaning is interpreted by SceneHost.

This conforms to the framework boundary in `AGENTS.md`. The inspected scene code does not encode agent, assistant, model, tool, conversation, queue, or product-status semantics.

## 8.2 Root semantic ownership is distinct from application process ownership

`Scene` owns semantic root data, but `RunningApp` owns:

- the Scene instance;
- component registry;
- update/view callbacks;
- action/timer queues;
- SceneHost;
- application lifecycle.

The root abstraction is therefore not the process/application kernel. `SceneHost` also does not own actual component values. It owns retained runtime products and consults the application-owned registry.

## 8.3 SceneHost bridges all runtime planes

`SceneHost` couples:

- structural resolution;
- retained state overlays;
- content invalidation;
- layout/measurement;
- physical painting;
- interaction focus;
- tick scheduling;
- outputs;
- native History transfer.

This coupling is deliberate host integration, not necessarily an ownership violation. The host is the current convergence point because each plane must agree before a frame becomes visible.

The consequential seam is `StableScene`: it carries semantic root, branch products, layout, History metadata, and revision identity together so local routes can be selected without reconstructing all planes.

## 8.4 Application host adds a second transaction layer

`SceneHost` prepares a frame candidate, but `application::HostInner` governs visible authority and backend receipt completion. This creates two distinct transactional boundaries:

```text
SceneHost candidate
    → PreparedSceneFrame
    → HostInner candidate_frame
    → backend receipt
    → visible frame/state/content commit
```

A SceneHost-local success does not necessarily mean terminal visibility. Conversely, a backend failure does not invalidate the previously visible logical frame.

## 8.5 Deferred component retirement is coupled to SceneHost

`application/kernel.rs` uses `SceneHost::is_mounted` before removing retired registry entries. This means component lifetime cannot be reasoned about solely from application handles or semantic desired roots. It depends on the last successfully reconciled SceneHost graph.

A failed candidate must therefore preserve the previous graph even if a newer desired root no longer references the component.

## 8.6 History/native frontier is a second identity/revision domain

`StableScene` stores:

- History semantic identity;
- History semantic revision;
- native History revision.

`History` native promotion can change the display frontier without changing semantic History ordering. SceneHost uses this distinction to refresh History while retaining the body branch.

This is a consequential ownership split:

```text
semantic History revision
    !=
native display frontier revision
```

The native frontier can also become physically unknown after a partial sink failure, which requires recovery behavior outside ordinary semantic resolution.

## 8.7 Potential cross-branch attachment caveat

`merge_root_scene` combines state overlays by map insertion but does not itself reject duplicate state attachment IDs across History and body. The native-host/application preparation path separately calls complete attachment-target validation, including History views, before publication.

Therefore:

- native-host frame publication has a stronger duplicate-state contract;
- direct internal root-resolution callers rely on upstream validation or do not receive that protection.

This is not proven to be a current bug because the inspected application path performs the validation, but it is an important seam and assumption.

## 8.8 Root-level History remains a special semantic branch

History is not just another ordinary child View:

- it owns a separate semantic model;
- projection can produce frozen physical overlays;
- it has a native transfer frontier;
- it can be refreshed without rebuilding body;
- its height affects body geometry;
- History-only changes can require body paint even if body semantic identity is unchanged.

Any later architecture work that treats History as an ordinary presentation child would need to preserve these host/runtime responsibilities explicitly. This report does not assign a future disposition.

## 8.9 Historical document versus current source

The pre-V5 report explicitly warns not to assume:

```text
current View == future occurrence
current Projector == future Funnel
current StreamPane == future Connector
current History == future ScrollSurface
current component registry == future React HostConfig
```

The current Scene source confirms that `Scene` is a root composition object with History/body ownership, not a generic occurrence abstraction. It also confirms that `SceneHost` combines many retained-runtime responsibilities beyond simple root composition.

No historical claim from the pre-V5 report was treated as authoritative over current source.

---

## 9. Open questions and coverage gaps

1. **Direct external Rust consumers**
   - `Scene` is declared `pub` but exposed through a crate-private module/re-export.
   - The inspected source establishes in-crate consumers, especially `RunningApp`, but does not establish whether downstream Rust crates can access Scene through another path.

2. **Native/TypeScript desired-root validation completeness**
   - The application kernel exposes state/content attachment validation helpers, and native-host code invokes them.
   - This report did not inspect every native or TypeScript call route that constructs prospective History/body views.

3. **Exact commit-time ordering across all state/content registries**
   - `application/host.rs` clearly keeps candidate state/content commits alongside candidate frames and backend receipts.
   - The detailed state/content registry implementation is outside this assignment.

4. **History projection internals**
   - This report records the Scene seam and projection outputs.
   - Full semantic History lifecycle, frozen units, native transfer, and projection behavior belong to the History assignment.

5. **Presentation cache-key completeness**
   - SceneHost comments document intentional exclusions such as theme revisions from current layout keys.
   - Exact key structure and invalidation implementation belong to presentation/layout/paint assignments.

6. **Performance under real workloads**
   - Source tests assert counter ceilings in selected scenarios.
   - No benchmark or runtime counter collection was performed here.

7. **Potential duplicate state IDs in non-native direct callers**
   - Root merge does not locally reject cross-branch state overlay collisions.
   - Native-host validation appears to cover the production binding path, but the complete set of internal callers was not exhaustively executed.

8. **Failure semantics after callbacks mutate the registry**
   - The source explicitly reruns after dirty layout/focus/content callbacks.
   - The exact interaction between callback-side application mutations and all external action queues is distributed across component/application assignments.

9. **SceneHost ownership after `clear_retained_views`**
   - The method clears layout/retained/surface/invalidation state but does not itself clear every application-level pending desired revision or registry entry.
   - Application-layer callers determine the intended reset boundary.

10. **No executed validation**
    - All test descriptions and counter claims in this report are source-derived.
    - Whether the baseline currently compiles or all tests pass remains unverified in this run.

---

## 10. Evidence appendix

## 10.1 Primary paths and exact symbols

### Scene module

- `crates/iyon-tui/src/scene/mod.rs`
  - module declarations;
  - internal re-exports;
  - test module inclusion.

- `crates/iyon-tui/src/scene/root.rs`
  - `Scene`
  - `Scene::new`
  - `Scene::with_history`
  - `Scene::history`
  - `Scene::history_mut`
  - `Scene::body`
  - `Scene::set_body`
  - `Scene::set_history`
  - `ResolvedRootScene`
  - `resolve_root_scene`
  - `resolve_root_scene_with_anchor`
  - `resolve_root_scene_with_anchor_and_cache`
  - `resolve_root_scene_with_anchor_and_cache_and_states`
  - `resolve_root_scene_with_anchor_and_cache_and_states_and_content`
  - `resolve_branch`
  - `resolve_component_subtree`
  - `resolve_component_subtree_with_states`
  - `merge_root_scene`
  - `ensure_disjoint_mounts`
  - `root_view`

- `crates/iyon-tui/src/scene/resolve.rs`
  - `ResolveError`
  - `ResolveSession`
  - `ResolveSession::new`
  - `ResolveSession::resolve_root`
  - `ResolveSession::resolve_root_with_dependencies`
  - `ResolveSession::set_state_snapshots`
  - `ResolveSession::finish`
  - `index_content_paths`
  - `reverse_content_path_components`
  - `state_attachment_targets`
  - `content_attachment_targets`
  - `Resolver`
  - `Resolver::scan_view`
  - `Resolver::resolve_slot`

- `crates/iyon-tui/src/scene/resolved.rs`
  - `ResolutionOverlay`
  - `ResolutionOverlay::component`
  - `ResolutionOverlay::state`
  - `ResolvedScene`
  - `ResolvedScene` equality implementation

- `crates/iyon-tui/src/scene/layout.rs`
  - `ResolvedSceneLayout`
  - `layout_resolved_scene`
  - `layout_resolved_scene_with_cache`
  - `layout_resolved_scene_with_cache_and_content`
  - `ResolvedSceneLayout::patch_component_with_cache`
  - `LayoutSync`
  - `LayoutSynchronizer`
  - `LayoutSynchronizer::synchronize`
  - `LayoutSynchronizer::synchronize_component`

- `crates/iyon-tui/src/scene/host.rs`
  - `NativePressure`
  - `drain_native_pressure`
  - `PreparedSceneFrame`
  - `PreparedSceneFrame::screen_lines`
  - `StableScene`
  - `ContentDirtyRecord`
  - `SceneHost`
  - `SceneHost::clear_retained_views`
  - `SceneHost::invalidate_component`
  - `SceneHost::invalidate_state`
  - `SceneHost::invalidate_content`
  - `SceneHost::invalidate_theme`
  - `SceneHost::try_local_content_refresh`
  - `SceneHost::try_local_geometry_refresh`
  - `SceneHost::invalidate_root`
  - `SceneHost::discard_candidate`
  - `SceneHost::dispatch_key_local`
  - `SceneHost::dispatch_paste`
  - `SceneHost::tick_due`
  - `SceneHost::render_at`
  - `SceneHost::render_at_with_states`
  - `SceneHost::resolve_stable_at_with_anchor`
  - `SceneHost::resolve_full_stable`
  - `SceneHost::try_incremental_stable`
  - `SceneHost::refresh_history_projection`
  - `SceneHost::paint_with_content`
  - `SceneHost::is_mounted`
  - `SceneHostError`

### Scene tests

- `crates/iyon-tui/src/scene/root_tests.rs`
  - `body_component_is_resolved_once_per_root_pass`
  - `body_only_root_has_no_history_overlay`
  - `history_and_body_use_remaining_height_and_terminal_width`
  - `history_tracks_all_space_left_by_intrinsic_body`
  - `history_follow_end_stays_above_body`
  - `narrow_body_does_not_narrow_history`
  - `body_exhaustion_gives_history_zero_height_but_keeps_live_mounted`
  - `duplicate_component_across_history_and_body_uses_one_session`
  - `root_mount_order_is_history_then_body`
  - `zero_dimensions_preserve_semantic_resolution_without_fake_rows`
  - `frozen_history_overlay_stays_inside_history_track_above_body`

- `crates/iyon-tui/src/scene/tests.rs`
  - component snapshot, resolver short-circuit, duplicate/cycle/missing-slot, component ownership, layout delivery, mount/visibility, and focus tests.

- `crates/iyon-tui/src/scene/host.rs` test module
  - incremental update, candidate rollback, History refresh, state/content/theme invalidation, focus/tick convergence, native pressure and performance-counter tests.

### Application integration

- `crates/iyon-tui/src/application/kernel.rs`
  - `RunningApp`
  - `RunningApp::host_retire_component`
  - `RunningApp::reap_retired_components`
  - native-host invalidation forwarding methods
  - `RunningApp::prepare_frame`
  - `RunningApp::prepare_frame_with_states`
  - `RunningApp::advance_ready`
  - `RunningApp::next_deadline`

- `crates/iyon-tui/src/application/host.rs`
  - `HostInner`
  - `frame`
  - `candidate_frame`
  - candidate state/content commit metadata
  - candidate discard/retry paths
  - `prepare_frame_with_content`
  - headless/real backend SceneHost calls
  - visible frame commit behavior

### Supporting ownership seams

- `crates/iyon-tui/src/component/graph.rs`
  - `MountGraph`
  - `MountNode`
  - `MountGraph::new`
  - `MountGraph::same_topology`
  - `MountGraph::reparent_roots`
  - `MountGraph::update_revision`
  - subtree APIs

- `crates/iyon-tui/src/component/mount.rs`
  - `MountedComponents`
  - `MountedComponents::reconcile`
  - mount/unmount transition tracking

- `crates/iyon-tui/src/lib.rs`
  - crate-private `scene` module;
  - crate-private `Scene` re-export.

- `crates/iyon-tui/src/perf.rs`
  - resolver/path-index/state/content counters.

## 10.2 Inspected-file manifest

### Read comprehensively in primary scope

- `crates/iyon-tui/src/scene/mod.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/scene/resolved.rs`
- `crates/iyon-tui/src/scene/layout.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/root_tests.rs`
- `crates/iyon-tui/src/scene/tests.rs`

### Read for direct integration evidence

- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/component/graph.rs`
- `crates/iyon-tui/src/component/mount.rs`
- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/perf.rs`

### Read for investigation contract/context

- `AGENTS.md`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `PRE-V5-ARCHITECTURE-REPORT.md`

### Indexed but not read as part of this assignment’s full source scope

The following were referenced by Scene integration but their detailed internals belong to other assignments and were not treated as fully read:

- remaining `crates/iyon-tui/src/component/` implementation files;
- `crates/iyon-tui/src/history/` implementation;
- `crates/iyon-tui/src/presentation/` implementation outside Scene-facing symbols;
- `crates/iyon-tui/src/retained_state/` implementation;
- `crates/iyon-tui/src/interaction/` implementation;
- `crates/iyon-tui/src/output/` implementation;
- terminal/backend implementation;
- native addon and TypeScript packages.

## 10.3 LOC methodology

- Counts are approximate physical line ranges from line-numbered source extraction.
- Production counts exclude `#[cfg(test)]` modules where the boundary was identifiable.
- Test counts include test helper types and fixtures inside the test module.
- No generated code occurs under `crates/iyon-tui/src/scene/`.
- No test/build/benchmark command was run; all validation statements are static source evidence.