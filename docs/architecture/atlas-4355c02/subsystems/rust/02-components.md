# 02 — Components: identity, registry, mount graph, capabilities, slots and ticks

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/component/`
- Assignment: `02`, Rust/components
- Assignment goal: identity, registry, mount graph, capabilities, slots and ticks.

The investigation followed:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`

The repository-level framework boundary is material here: component identity, retained component lifecycle, native interaction, scheduling, and generic slots are framework concerns. No product-specific Iyon/agent/application semantics were observed in the component subsystem.

### Scope boundaries

The primary production scope is the complete `component/` module:

```text
crates/iyon-tui/src/component/
├── capability.rs
├── graph.rs
├── id.rs
├── mod.rs
├── mount.rs
├── mount_tests.rs
├── registry.rs
├── revision.rs
├── slot.rs
├── tests.rs
└── tick.rs
├── tick_tests.rs
```

Supporting source was inspected where required to establish ownership and call paths:

- `crates/iyon-tui/src/interaction/command.rs`
- `crates/iyon-tui/src/interaction/routing.rs`
- relevant portions of `crates/iyon-tui/src/interaction/focus.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/scene/resolved.rs`
- relevant portions of `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/application/context.rs`
- relevant portions of `crates/iyon-tui/src/application/kernel.rs`
- relevant portions of `crates/iyon-tui/src/application/host.rs`
- relevant portions of `crates/iyon-tui/src/presentation/factory.rs`
- `crates/iyon-tui/src/presentation/ir.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/perf_bench.rs`
- relevant native-binding portions of `crates/iyon-tui-native/src/tui.rs`

### Evidence status

This is a static source investigation. No build, test suite, benchmark suite, runtime process, or service was executed in this session. Behavioral claims below are either:

1. directly observed from source, or
2. supported by existing tests that were read but not run.

I do not claim current test execution or runtime validation.

### Facts, inferences and unknowns

- **Fact:** `ComponentRegistry` owns the actual retained `Component` values.
- **Fact:** `ComponentHandle<C>` is a typed, copyable, non-owning identity.
- **Fact:** component IDs are process-global, monotonic, non-zero `u64` values.
- **Fact:** `MountGraph` stores component topology separately from the registry.
- **Fact:** component capabilities are collected into snapshots and keyed by `ComponentId`.
- **Fact:** component snapshots are invalidated by registry mutation or native invalidation.
- **Fact:** mount transitions are derived by comparing the previous and next graph membership, not by revision changes.
- **Fact:** `component/slot.rs` contains no implementation; semantic component slots are represented by `presentation::ir::ComponentSlotNode`.
- **Fact:** native `HostViewSlot` is implemented outside `component/` by wrapping a registered `MountedViewSlot` component.
- **Inference:** the component subsystem is the generic retained native runtime kernel behind both Rust-internal components and native/TypeScript-hosted controls.
- **Unknown:** no explicit parent-existence or cycle validation is performed by `MountGraph::new`; resolver-generated graphs are expected to satisfy those invariants.
- **Unknown:** no independent generation/lease field exists beyond globally monotonic IDs. Raw component IDs have no separate generation encoded in them.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate physical LOC | Production/test | Primary responsibility | Public surface |
|---|---:|---|---|---|
| `component/mod.rs` | 37 | Production | Module declarations, exports, `Component` trait | `Component` is declared `pub`; module itself is crate-private |
| `component/capability.rs` | 6 | Production | Public component capability facade | Re-exports `interaction::ComponentCx` |
| `component/id.rs` | 100 | Production | Component identity allocation and typed handles | `ComponentHandle<C>` is `pub`; `ComponentId` is crate-private |
| `component/revision.rs` | 13 | Production | Monotonic component revision | Entirely crate-private |
| `component/registry.rs` | 226 | Production | Type-erased retained component ownership, snapshot cache, mutation access | `ComponentRegistry` and snapshots are crate-private |
| `component/graph.rs` | 184 | Production | ID-keyed retained mount topology and subtree replacement | Entirely crate-private |
| `component/mount.rs` | 58 | Production | Previous/next graph reconciliation and mount transitions | Entirely crate-private |
| `component/tick.rs` | 305 | Production | Mounted capability tick registration, deadlines and callback dispatch | Entirely crate-private |
| `component/slot.rs` | 0 | Production | Empty module placeholder; no symbols | No implementation |
| `component/tests.rs` | 188 | Tests | Identity, registry, resolution, nested component and graph contracts | Test-only |
| `component/mount_tests.rs` | 73 | Tests | Parent-first mounting, child-first unmounting, reordering and revision behavior | Test-only |
| `component/tick_tests.rs` | 296 | Tests | Deadline, order, remount, dynamic interval and zero-interval contracts | Test-only |

The production estimate is approximately **929 physical source lines** across the component directory, calculated by summing the highest source line number for each production file. Component-local tests account for approximately **557 physical lines**. Blank lines and comments are included in these physical estimates; this is not a semantic statement of executable LOC.

### 1.2 `Component` contract

`component/mod.rs:28-37` defines:

```rust
pub trait Component: 'static {
    fn view(&self) -> View;

    fn capabilities(&self, _cx: &mut ComponentCx<'_, Self>)
    where
        Self: Sized,
    {
    }
}
```

The component contract has two orthogonal outputs:

1. `view(&self) -> View`
   - produces the component’s semantic presentation;
   - is immutable from the component API’s perspective;
   - may contain nested component slots.

2. `capabilities(&self, &mut ComponentCx<Self>)`
   - declares native interaction and scheduling capabilities;
   - defaults to no capabilities;
   - is collected into a separate capability snapshot.

The `'static` bound enables type erasure into `Box<dyn ErasedComponent>` and callback storage through `Any` downcasts. The trait does not require `Send` or `Sync`. The registry is therefore an application/runtime-thread-owned structure rather than a generally thread-safe component store.

### 1.3 `ComponentId` and `ComponentHandle`

`component/id.rs:5-28` defines a process-global atomic allocator and opaque `ComponentId`:

- `NEXT_COMPONENT_ID` starts at `1`.
- `ComponentId::allocate()` delegates to `crate::id::next_nonzero_id`.
- `ComponentId` wraps `NonZeroU64`.
- Allocation overflow panics with `"component id exhausted"`.
- `ComponentId::from_raw(0)` panics with `"component id must be non-zero"`.

`crate/id.rs:6-12` shows that allocation uses `AtomicU64::fetch_update` with checked increment and relaxed ordering. The current value is returned and the counter is advanced, so the first allocated ID is `1`.

`ComponentHandle<C>` (`component/id.rs:40-100`) contains:

```rust
struct ComponentHandle<C> {
    id: ComponentId,
    marker: PhantomData<fn() -> C>,
}
```

Properties:

- `Copy` and `Clone`;
- typed by `C` through `PhantomData`;
- equality and hashing use only the underlying `ComponentId`;
- no ownership or destructor behavior;
- `Debug` intentionally prints only `"ComponentHandle"` rather than exposing the ID;
- `raw_id()` is public and returns the opaque non-zero `u64`;
- conversion from raw IDs is crate-private.

The handle is therefore a typed reference token, not an owning smart pointer and not a lease. The registry is the owner of the actual object.

### 1.4 `ComponentRevision`

`component/revision.rs:1-13` defines a private monotonically increasing revision:

```rust
pub(crate) struct ComponentRevision(u64);
```

- starts at `0`;
- `increment()` uses checked addition;
- overflow panics with `"component revision exhausted"`;
- revision changes are independent of mount transitions;
- revision changes invalidate a registry snapshot but do not themselves imply unmount/remount.

### 1.5 `ComponentRegistry`

`component/registry.rs:65-69`:

```rust
pub(crate) struct ComponentRegistry {
    slots: HashMap<ComponentId, ComponentEntry>,
}
```

Each entry (`registry.rs:53-57`) contains:

```rust
struct ComponentEntry {
    component: Box<dyn ErasedComponent>,
    revision: ComponentRevision,
    snapshot: RefCell<Option<ComponentSnapshot>>,
}
```

The registry is explicitly documented as the “sole owner of retained component instances” (`registry.rs:65`).

`ErasedComponent` (`registry.rs:8-14`) provides:

- `view() -> View`;
- `capabilities() -> ComponentCapabilities`;
- immutable and mutable `Any` access;
- boxed conversion back to `Box<dyn Any>` for typed removal.

The blanket implementation for every `C: Component`:

- increments `Counter::ComponentViewCalls` before calling `Component::view`;
- increments `Counter::ComponentCapabilityCalls` before collecting capabilities;
- creates a fresh `ComponentCapabilities`;
- creates an ephemeral `ComponentCx`;
- invokes `Component::capabilities`.

### 1.6 Mount graph

`component/graph.rs:11-22` defines:

```rust
pub(crate) struct MountGraph {
    entries: HashMap<ComponentId, MountNode>,
    roots: Vec<ComponentId>,
    children: HashMap<ComponentId, Vec<ComponentId>>,
}

pub(crate) struct MountNode {
    pub(crate) id: ComponentId,
    pub(crate) parent: Option<ComponentId>,
    pub(crate) revision: ComponentRevision,
}
```

The graph is not an indexed vector. It is keyed by stable IDs and maintains:

- node lookup by `ComponentId`;
- ordered root IDs;
- ordered child vectors by parent ID.

The graph’s iterator performs depth-first traversal while preserving the input order of roots and children.

### 1.7 Mounted-components reconciler

`component/mount.rs:3-35` defines:

```rust
pub(crate) struct MountedComponents {
    current: MountGraph,
}
```

`reconcile(next)` compares membership between `current` and `next`:

- old IDs absent from `next` produce `Unmounted` transitions in reverse depth-first order;
- new IDs absent from `current` produce `Mounted` transitions in depth-first order;
- existing IDs produce no transition even if reordered, reparented, or revision-changed;
- `current` is replaced by `next` after transition collection.

### 1.8 Tick scheduler

`component/tick.rs:55-60` defines:

```rust
pub(crate) struct TickScheduler {
    registrations: HashMap<ComponentId, TickRegistration>,
    mounted: HashSet<ComponentId>,
    mount_order: Vec<ComponentId>,
}
```

A registration contains:

- `interval: Duration`;
- `next_due: Option<Instant>`;
- erased callback driver.

The scheduler is private and is driven by `SceneHost`, not directly by public caller code.

---

## 2. Types, APIs and contracts

### 2.1 Intentional public versus internal API

There is an important visibility distinction:

- `Component` is declared `pub` in `component/mod.rs`.
- `ComponentHandle<C>` is declared `pub`.
- `ComponentCx` is publicly exposed through `component/capability.rs`.
- However, `lib.rs:15` declares `mod component;`, not `pub mod component;`.
- `lib.rs:59` re-exports `Component`, `ComponentCx`, and `ComponentHandle` only with `pub(crate) use`.

Consequently, these are not supported external Rust crate authoring APIs despite their item-level `pub` declarations. `crates/iyon-tui/Cargo.toml:6-8` makes the intended package boundary explicit: the crate is runtime implementation for the in-tree native binding, while external UI code authors against the TypeScript surface.

This is consistent with the framework boundary but should not be mistaken for an externally consumable public Rust component package.

### 2.2 Component registration APIs

The effective registration surfaces are:

#### Application context

`application/context.rs:76-82`:

```rust
pub fn register<C>(&mut self, component: C) -> ComponentHandle<C>
where
    C: Component
```

This delegates to `ComponentRegistry::register`.

`AppCx::with_component` and `with_component_mut` (`context.rs:84-105`) provide typed closure-scoped access.

`AppCx::remove_component` (`context.rs:108-119`) removes a typed component immediately and also removes associated paste interceptors if removal succeeds.

#### Native host

`application/kernel.rs:75-80` has `RunningApp::host_register`, delegating to the same registry.

Native controls and slots use this path:

- `application/host.rs:1151-1157`: text input creation;
- `application/host.rs:1160-1166`: view slot creation;
- `application/host.rs:1169-1175`: scroll pane creation.

### 2.3 Registry invariants

Observed registry contracts:

- IDs are globally monotonic and never reused after removal.
- A typed handle resolves only if both its ID exists and the stored erased type is `C`.
- A handle from a different registry does not resolve because IDs are process-global and registry membership is local.
- Immutable access does not change revision.
- Successful mutable access changes revision and clears the cached snapshot.
- Failed type downcasts do not change revision.
- Removal by typed handle verifies the stored type before removal.
- Removal by raw ID exists for deferred native retirement.
- `resolution(id)` returns `None` for an absent ID.
- A cached snapshot is reusable only if its revision equals the entry revision.

`with_any_mut` (`registry.rs:128-137`) and `with_mut` (`registry.rs:178-191`) invalidate the entry after the callback returns successfully. The mutation API therefore treats mutable access itself as a state change, even if the callback does not logically alter fields.

### 2.4 Component snapshots

`ComponentSnapshot` (`registry.rs:46-50`) contains:

```rust
pub(crate) struct ComponentSnapshot {
    pub(crate) view: View,
    pub(crate) revision: ComponentRevision,
    pub(crate) capabilities: ComponentCapabilities,
}
```

The snapshot couples:

- semantic view output;
- the component revision at which that view was produced;
- the component capability set at the same revision.

This is the unit placed in `scene::ResolutionOverlay`.

`ComponentRegistry::resolution` (`registry.rs:157-175`):

1. looks up the entry;
2. checks whether a cached snapshot exists with the current revision;
3. returns a clone of that snapshot on a cache hit;
4. otherwise invokes `view()` and `capabilities()`;
5. stores and returns a new snapshot.

The `View` and capability data are therefore resolved together and invalidated together.

### 2.5 Capability declaration APIs

The declaration context is `interaction/command.rs:68-78`:

```rust
pub struct ComponentCx<'a, C> {
    pub(crate) capabilities: &'a mut ComponentCapabilities,
    marker: PhantomData<fn(&'a C)>,
}
```

`component/capability.rs:1-6` re-exports this type for the component facade.

Public capability declarations:

- `focusable()` (`command.rs:81-84`);
- `modal_scope()` (`command.rs:86-89`);
- `on_focus_changed(fn(&mut C, bool))` (`command.rs:91-102`);
- `on_paste(...)` (`command.rs:104-120`);
- `key_commands(map, handle)` (`command.rs:150-175`);
- `tick(interval, handler)` (`command.rs:177-198`).

Crate-private capability declarations:

- `on_layout_changed` (`command.rs:123-134`);
- `on_content_extent_changed` (`command.rs:136-148`).

The capability structure (`command.rs:37-46`) stores:

- boolean `focusable`;
- boolean `modal_scope`;
- one optional focus callback;
- one optional paste callback;
- ordered vector of key-command capabilities;
- one optional tick capability;
- one optional layout callback;
- one optional content-extent callback.

The callback storage is erased through `Arc<dyn Fn...>` wrappers, with runtime downcasts back to the component’s concrete type. A type mismatch is treated as an invariant violation and panics through `expect(...)`.

### 2.6 Capability replacement semantics

Capabilities are rediscovered whenever the component snapshot is rebuilt. Within one declaration pass:

- repeated `on_focus_changed` calls replace the prior callback;
- repeated `on_paste` calls replace the prior callback;
- repeated layout/content callbacks replace the prior callback;
- repeated `tick` calls replace the prior tick;
- repeated `key_commands` calls append in declaration order.

The key-command vector is therefore intentionally ordered and can contain multiple mappings, while most other capability types are singleton declarations.

### 2.7 Tick capability contract

`ComponentCx::tick` rejects zero intervals:

```rust
assert!(
    !interval.is_zero(),
    "component tick interval must be nonzero"
);
```

This is an explicit programming-contract assertion, not a recoverable error.

The handler signature is:

```rust
for<'event> fn(
    &mut C,
    Instant,
    &mut EventCx<'event>,
) -> bool
```

The returned `bool` is interpreted by the scheduler as whether the component became dirty and needs retained-scene work. Output emission through `EventCx` is independent of the returned dirty flag.

### 2.8 Mounted capabilities

`MountedCapabilities` (`interaction/command.rs:201-222`) is an internal map:

```rust
pub(crate) struct MountedCapabilities {
    pub(crate) entries: HashMap<ComponentId, ComponentCapabilities>,
}
```

It provides:

- `insert(id, caps)`;
- `get(id)`;
- `modal_ids(order)` filtering mounted graph order by `modal_scope`.

The map is produced as part of scene resolution and consumed by:

- focus reconciliation;
- modal containment;
- key routing;
- paste routing;
- layout callbacks;
- tick synchronization.

It does not itself store component revisions. Revision association is carried by the scene-resolution process and host update logic.

---

## 3. Dependency and ownership map

### 3.1 Ownership diagram

```text
Application / native host
        │
        │ register(C)
        ▼
ComponentRegistry
        │ owns
        ▼
Box<dyn ErasedComponent>
        │
        ├── view() ───────────────┐
        └── capabilities() ──────┤
                                  ▼
                         ComponentSnapshot
                         (View + revision + caps)
                                  │
                                  ▼
                         ResolveSession / Resolver
                                  │
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
             ResolutionOverlay  MountGraph  MountedCapabilities
                    │             │             │
                    ▼             ▼             ▼
             layout/paint      mount diff    focus/routing/ticks
                                                  │
                                                  ▼
                                           TickScheduler
                                                  │
                                                  ▼
                                      ComponentRegistry::with_any_mut
```

### 3.2 Forward dependency direction

```text
component::id
    └── crate::id::next_nonzero_id

component::registry
    ├── component::id
    ├── component::revision
    ├── interaction::ComponentCapabilities / ComponentCx
    ├── presentation::View
    └── perf counters

component::graph
    ├── component::id
    └── component::revision

component::mount
    └── component::graph

component::tick
    ├── component::id
    ├── component::graph
    ├── component::mount
    ├── component::registry
    ├── interaction::MountedCapabilities
    └── output::EventCx / OutputQueue

scene::resolve
    ├── component registry
    ├── component revision
    ├── mount graph
    ├── mounted capabilities
    └── resolution overlay

scene::host
    ├── scene resolution
    ├── mounted graph reconciliation
    ├── focus/routing
    └── tick scheduler

application::kernel
    ├── registry ownership
    └── SceneHost ownership

application::host
    ├── native control/slot wrappers
    └── host registration and invalidation
```

### 3.3 Registry ownership and destruction

The registry creates and owns each `Box<dyn ErasedComponent>`:

- `ComponentRegistry::register` allocates the ID and inserts the boxed value.
- `ComponentRegistry::remove` consumes and downcasts the boxed value.
- `ComponentRegistry::remove_id` removes by erased ID.
- Registry destruction drops all remaining entries.

The handle never owns the value. Cloning a handle does not clone component state.

For native-hosted objects, destruction is intentionally split:

1. native object disposal requests retirement;
2. `RunningApp` records the raw ID in `pending_component_retirements`;
3. registry removal is delayed while the last committed mount graph still contains the ID;
4. after successful frame preparation/reconciliation, `reap_retired_components` removes unmounted entries.

This safety path is in `application/kernel.rs:113-143`, not in the registry itself.

### 3.4 Identity and lifetime edges

`ComponentId` identifies a registry entry and all scene/mount/capability records derived from that entry. It also acts as the identity carried by `ViewKind::ComponentSlot`.

There is no explicit generation field. Non-reuse is supplied by the process-global monotonic allocator. This eliminates ordinary ID ABA after removal, provided callers cannot manufacture conflicting raw IDs.

Internal raw conversion exists:

- `ComponentId::from_raw(u64)` (`component/id.rs:23-28`);
- `ComponentHandle::from_raw_id` (`component/id.rs:86-88`);
- `presentation::factory::native_component(raw_id)` (`presentation/factory.rs:353-366`).

The constructors reject zero but do not verify registry membership. Invalid non-zero raw IDs fail later during scene resolution as `MissingComponent`.

### 3.5 Scene-resolution ownership

`scene/resolve.rs:528-537` creates a resolver containing:

- immutable registry reference;
- a vector of mount nodes;
- mounted capabilities;
- resolution overlay;
- resolver-local cycle/duplicate tracking.

At `resolve_slot` (`resolve.rs:585-613`):

1. `registry.resolution(id)` obtains the snapshot;
2. missing IDs return `ResolveError::MissingComponent`;
3. active recursion detects cycles;
4. global `seen` detects duplicate component identity;
5. the mount node is appended with parent and snapshot revision;
6. capabilities and snapshot are added to resolver state;
7. the snapshot view is recursively scanned.

The resolver does not own the component. It owns only derived scene data.

---

## 4. Execution paths and state transitions

### 4.1 Registration and first use

#### Registration

Typical Rust-internal path:

```text
AppCx::register
    → ComponentRegistry::register
        → ComponentId::allocate
        → Box::new(component)
        → revision = 0
        → snapshot = None
        → typed ComponentHandle<C>
```

Evidence:

- `application/context.rs:76-82`
- `application/kernel.rs:75-80`
- `component/registry.rs:84-98`
- `component/id.rs:11-16`

Native control path:

```text
TuiHost::create_view_slot
    → HostViewSlot::new
    → attach_host
    → RunningApp::host_register(MountedViewSlot)
    → set_component_id(handle.raw_id())
```

Evidence:

- `application/host.rs:1160-1166`
- `application/host.rs:608-626`

#### First semantic use

The caller creates a semantic component slot using either:

- `presentation::factory::component(handle)` (`factory.rs:369-380`), or
- `presentation::factory::native_component(raw_id)` (`factory.rs:353-366`).

Both create a `ViewKind::ComponentSlot(ComponentSlotNode { id })`. Component metadata is not itself painted; it is an indirection into the registry/overlay.

### 4.2 Resolution path

The normal branch-resolution path is:

```text
SceneHost resolution
    → ResolveSession::new(registry)
    → ResolveSession::resolve_root(view)
    → Resolver::scan_view
    → Resolver::resolve_slot(id, parent)
    → ComponentRegistry::resolution(id)
        ├── snapshot hit: clone cached snapshot
        └── snapshot miss: call Component::view and Component::capabilities
    → append MountNode
    → add MountedCapabilities entry
    → add ResolutionOverlay component snapshot
    → recursively scan snapshot.view
    → ResolveSession::finish
        → MountGraph::new
        → ResolvedScene
```

Evidence:

- `scene/resolve.rs:40-47`
- `scene/resolve.rs:49-72`
- `scene/resolve.rs:543-582`
- `scene/resolve.rs:585-613`
- `scene/resolve.rs:113-133`

The resolver uses `View` flags to avoid descending into component-free branches:

```rust
if !view.contains_component_identity() {
    return Ok(());
}
```

This means component topology discovery is conditional on semantic component identity flags rather than an unconditional complete view traversal.

### 4.3 Resolution errors

The resolver has three explicit component errors (`scene/resolve.rs:20-25`):

- `MissingComponent { id }`
- `DuplicateComponent { id }`
- `ComponentCycle { path }`

Observed triggers:

- missing registry entry;
- same component ID encountered more than once in a resolved branch;
- recursive component reference encountered while the ID is active.

A component can therefore be referenced only once within one resolved graph. This is stronger than ordinary tree sharing: repeated references to the same retained component are rejected as duplicate ownership rather than represented as multiple occurrences.

### 4.4 Mount reconciliation

After a successful candidate scene has been resolved and prepared, `SceneHost` stores the candidate graph/capabilities and performs mount reconciliation (`scene/host.rs:1268-1295`):

```text
candidate MountGraph
        │
        ▼
MountedComponents::reconcile
        │
        ├── old absent IDs → Unmounted, reverse DFS
        └── new absent IDs → Mounted, DFS
        │
        ▼
TickScheduler::sync_capabilities
```

`MountedComponents::reconcile`:

- unmounts children before parents;
- mounts parents before children;
- does not emit transitions for revision-only changes;
- does not emit transitions for reordering or reparenting of existing IDs.

This ordering is explicitly tested in `component/mount_tests.rs:12-40`.

### 4.5 Revision-only update

When a component’s state changes but its mount topology remains the same:

```text
registry.with_mut / with_any_mut / invalidate
    → entry.revision += 1
    → cached snapshot cleared
    → later resolution recomputes View + capabilities
    → MountGraph revision updated
    → no mount transition
```

`MountGraph::same_topology` compares only `(id, parent)` pairs and ignores revisions (`graph.rs:97-104`).

This is used by host incremental paths. `SceneHost::update_incremental_host_state` (`scene/host.rs:874-889`) updates:

- graph revisions for affected IDs;
- capability entries for affected IDs;
- removes capability entries if no replacement capability exists.

It deliberately avoids cloning clean mounted entries.

### 4.6 Local component subtree replacement

The local replacement path is:

```text
component invalidated
    → registry snapshot invalidated
    → resolve component snapshot
    → resolve_component_subtree_with_states
    → reparent branch roots under owner
    → compare old/new direct descendants
    → reject duplicate IDs
    → MountGraph::replace_subtree(owner, replacement)
    → preserve owner node
    → update owner/descendant revisions
    → update capabilities
    → synchronize only affected ticker/focus state
```

Evidence:

- `scene/root.rs:274-298`
- `scene/host.rs:2357-2392`
- `scene/host.rs:2468-2480`
- `scene/host.rs:874-889`
- `scene/host.rs:1292-1296`

`resolve_component_subtree_with_states` resolves a branch and then calls `MountGraph::reparent_roots(parent)`. The owner component remains in the existing graph; only its descendants are replaced.

`MountGraph::replace_subtree` (`graph.rs:141-171`):

- requires the owner to exist;
- rejects a replacement that contains the owner ID;
- removes old descendants;
- replaces the owner’s child list;
- inserts replacement nodes and child lists;
- does not rebuild unrelated graph indexes.

The host performs duplicate checks against the rest of the graph before invoking this trusted graph operation (`scene/host.rs:2388-2391`).

### 4.7 Failed candidate and rollback semantics

The source comments establish a transactional rule:

- committed graph/capability state remains authoritative until the new candidate prepares successfully;
- failed frame preparation must not trigger retirement of components still present in the previous graph;
- `SceneHost::discard_candidate` invalidates the root and clears candidate caches;
- native component retirement is not reaped after a failed frame.

Evidence:

- `application/kernel.rs:123-128`
- `application/kernel.rs:696-701`
- `scene/host.rs:866-872`
- `scene/host.rs:251-255`

A component can therefore remain physically registered after its native handle was disposed if the last committed scene still references it or if a candidate replacement failed.

### 4.8 Interaction capability path

Capabilities are consumed by focus and routing using the current mount graph:

```text
SceneHost graph + MountedCapabilities
    → FocusState / routing_chain
    → registry.with_any / with_any_mut
    → erased callback
    → concrete component mutation
    → registry revision increment on mutable callback
```

Key routing (`interaction/routing.rs:45-85`):

- starts with the focused component or active modal;
- walks ancestors;
- checks each component’s ordered key-command list;
- maps the key immutably;
- invokes the handler mutably;
- stops at the first `Consumed`;
- if no component consumes Tab, invokes native focus traversal.

Paste routing (`routing.rs:22-43`) follows the same focused/ancestor chain and uses the first consuming paste capability.

Focus callbacks use the previous/current capability snapshots and invoke mutable registry access (`interaction/focus.rs:248-278`, `focus.rs:329-335`).

A callback returning `Ignored` does not prevent revision advancement if it was invoked through `with_any_mut`; the registry treats the mutable callback access as a revision boundary.

### 4.9 Tick path

The scheduler path is:

```text
application/kernel update loop
    → SceneHost::tick_due(now, registry)
    → TickScheduler::tick_due_with_events
    → due registrations in mount order
    → CapabilityTickDriver::tick
    → registry.with_any_mut(component_id, callback)
    → callback returns dirty bool
    → next_due = now + interval
    → TickOutcome { ran, dirty, changed_components }
```

Evidence:

- `application/kernel.rs:585-588`
- `scene/host.rs:963-975`
- `component/tick.rs:240-288`

The scheduler does not itself rebuild scenes. It reports dirty state to the application/host loop. `SceneHost::tick_due` records changed components as invalidations so subsequent retained reconciliation cannot reuse stale component slot frames.

### 4.10 Tick activation/deactivation

`sync_mounts` (`tick.rs:77-116`):

- applies explicit mount/unmount transitions;
- adds graph IDs to `mounted`;
- activates registrations on mount;
- deactivates registrations on unmount;
- repairs registrations whose `next_due` is unexpectedly `None`;
- removes IDs not in the graph;
- sets `mount_order = graph.ids().collect()`.

`sync_capabilities` (`tick.rs:120-180`):

- first synchronizes mounts;
- builds desired registrations from graph IDs and mounted capabilities;
- skips zero intervals;
- replaces callback drivers;
- resets deadlines when an interval changes;
- creates registrations for new tick capabilities;
- removes registrations no longer desired.

`sync_component_capability` (`tick.rs:182-223`) is the local update path used when topology is unchanged.

### 4.11 Tick deadline semantics

New or activated registrations receive:

```text
next_due = now + interval
```

A callback is due when `deadline <= now`.

After running, the scheduler sets:

```text
next_due = now + interval
```

It does not preserve the old deadline phase and does not catch up multiple missed intervals. If the host wakes late, one callback runs and the next interval is measured from the current `now`.

`next_deadline` scans current `mount_order` registrations and returns the minimum due time (`tick.rs:225-231`).

### 4.12 Native `HostViewSlot` path

Although `component/slot.rs` is empty, native view slots are implemented by wrapping a regular retained component.

`application/host.rs:197-218` defines `HostViewSlot` and its internal `ViewSlotState`:

```text
HostViewSlot
├── Arc<Mutex<ViewSlotState>>
├── Arc<Mutex<Option<u64>>> component_id
└── Arc<Mutex<Option<Weak<Mutex<HostInner>>>>> host

ViewSlotState
├── view
├── revision
├── frames
├── pending_frames
├── frame_index
├── interval
└── last_tick
```

`MountedViewSlot` implements `Component`:

- `view()` returns the current state view, or a spacer on poisoned lock;
- `capabilities()` declares a fixed `16ms` component tick;
- the tick delegates to `HostViewSlot::tick`.

Evidence:

- `application/host.rs:608-626`

The native host therefore uses the generic component registry and scheduler rather than a separate slot scheduler.

`HostViewSlot::set_view`:

- replaces the current semantic view;
- clears animation frames and pending frames;
- resets frame index and clock;
- increments slot-local revision;
- invalidates the host.

Evidence: `application/host.rs:239-253`.

Animation state is caller-supplied but tick-driven by Rust:

- `set_animation` rejects empty frames;
- animation interval is held in Rust;
- `set_animation_at_cycle_boundary` can defer frame replacement;
- frame replacement at a cycle boundary preserves current animation phase;
- `HostViewSlot::tick` advances frames and increments the slot revision when the current view changes.

Evidence:

- `application/host.rs:283-336`
- `application/host.rs:338-372`
- `application/host.rs:386-417`

A noteworthy distinction exists between generic component tick declarations and view-slot animation intervals:

- `ComponentCx::tick` rejects `Duration::ZERO`;
- `HostViewSlot::set_animation` and `set_animation_at_cycle_boundary` validate non-empty frames but, in the inspected code, do not reject a zero animation interval;
- the wrapper’s scheduler capability is still the fixed non-zero `16ms`;
- a zero internal animation interval would make the internal `duration_since(last) >= interval` condition immediately true on every wrapper tick.

This is an observed contract difference, not a proposed change.

### 4.13 Native disposal and deferred retirement

Native wrappers expose `dispose()` through N-API. The durable path is:

```text
NativeViewSlot::dispose
    → HostViewSlot::retire
    → RunningApp::host_retire_component(raw_id)
    → pending_component_retirements
    → reap after successful scene reconciliation
    → ComponentRegistry::remove_id
```

Evidence:

- `crates/iyon-tui-native/src/tui.rs:1629-1638`
- `application/host.rs:260-276`
- `application/kernel.rs:113-143`

This differs from `AppCx::remove_component`, which removes immediately after application code has released semantic references (`application/context.rs:108-119`). The two paths serve different ownership assumptions:

- Rust application-context removal is caller-controlled and documented as safe after references are released.
- Native disposal must account for committed views and candidate publication failures, so it is deferred.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Primary route | Selection condition | Failure/recovery behavior |
|---|---|---|---|
| Allocate retained component | `ComponentRegistry::register` | Any `C: Component` | Panics only on ID exhaustion |
| Create semantic component reference | `factory::component(handle)` | Rust-internal typed handle | Stores ID in `ComponentSlotNode` |
| Create raw/native component reference | `factory::native_component(raw_id)` | Native/bridge path | Rejects zero; unknown non-zero ID fails during resolution |
| Resolve component | `ComponentRegistry::resolution` | Slot encountered by resolver | `None` becomes `MissingComponent` |
| Reuse component snapshot | `resolution` cache | Cached revision equals entry revision | Returns cloned snapshot |
| Recompute view/capabilities | `ErasedComponent::view/capabilities` | Cache miss or invalidated revision | Callback/type invariant failures panic |
| Mutate component | `with_mut` / `with_any_mut` | Typed or erased mutable access | Failed lookup/downcast returns `None`; successful callback invalidates snapshot |
| Native external invalidation | `ComponentRegistry::invalidate` | `native-host` feature | Missing ID returns `false`; valid ID increments revision |
| Build graph | `ResolveSession::finish` | Successful resolution | Graph built from resolver mount vector |
| Reconcile mounts | `MountedComponents::reconcile` | Candidate accepted | Unmount reverse DFS, mount DFS |
| Replace local subtree | `MountGraph::replace_subtree` | Topology-preserving local candidate | Owner retained; old descendants removed |
| Focus routing | `route_key_local` / `route_paste` | Focus/modal chain exists | Missing capabilities or registry entries are skipped/treated as ignored |
| Register tick | `TickScheduler::sync_capabilities` | Mounted capability has tick | Zero interval is expected to be impossible after declaration assertion |
| Run tick | `tick_due_with_events` | `next_due <= now` | Missing registry entry results in `false`; deadline still advances |
| Native slot update | `HostViewSlot::set_view` | Native wrapper still attached | Lock/host errors return `anyhow::Result` |
| Native component disposal | deferred retirement | N-API handle disposed | Registry entry retained while committed graph still references it |
| Rust AppCx removal | `AppCx::remove_component` | Caller explicitly releases component | Immediate removal; stale semantic references later resolve as missing |

### 5.2 Missing versus stale versus rejected

The subsystem distinguishes several kinds of failure:

- **Missing registry entry:** explicit `ResolveError::MissingComponent`.
- **Duplicate identity in one graph:** explicit `ResolveError::DuplicateComponent`.
- **Recursive identity cycle:** explicit `ResolveError::ComponentCycle`.
- **Stale cached snapshot:** not an error; revision mismatch triggers recomputation.
- **Stale native disposal request:** not an error; retirement remains pending while mounted.
- **Failed type-typed access:** returns `None`, with no revision change.
- **Callback type mismatch:** panics through `expect`, treated as an internal invariant violation.
- **ID/revision exhaustion:** panics explicitly.
- **Zero generic tick interval:** panics at declaration.
- **Poisoned native wrapper lock:** wrapper `view()` methods commonly fall back to `spacer(0)`; mutating methods generally return an error.

### 5.3 Mount graph alternate routes

`MountGraph::same_topology` is the key selection condition between full and incremental host work:

- same ID/parent sequence permits a topology-preserving local path;
- any membership/order/parent difference requires broader reconciliation;
- revision differences alone do not force mount transitions.

`scene/host.rs:1953` uses this comparison for a history-related incremental decision.

The graph itself is intentionally low-level and assumes its caller has validated replacement IDs. `replace_subtree` only rejects:

- absent owner;
- replacement containing the owner itself.

It does not independently reject all collisions with IDs outside the replaced subtree. The host’s duplicate check is therefore consequential and must remain part of the trusted replacement path.

### 5.4 Graph structural assumptions

`MountGraph::new` asserts duplicate IDs but does not visibly validate:

- that every non-root parent exists;
- that parents precede children in the input;
- that the topology is acyclic;
- that all roots are semantically reachable from a valid resolver output.

Resolver-generated graphs satisfy stronger invariants through recursive active/seen tracking. Direct graph tests construct valid trees. Direct external construction is not available because the graph is crate-private, but internal callers could still violate these assumptions.

`is_descendant_or_self` walks parent links without a cycle guard (`graph.rs:174-183`). A malformed cyclic graph could loop indefinitely; the resolver’s cycle check is the upstream protection.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Registry snapshot cache

**Key:** `ComponentId` entry plus `ComponentRevision`.

**Stored value:** `ComponentSnapshot` containing `View`, revision, and capabilities.

**Retention:** one optional snapshot per registry entry.

**Invalidation:**

- successful `with_mut`;
- successful `with_any_mut`;
- native `invalidate`;
- explicit component removal drops the entry.

**Reuse:** only when `snapshot.revision == entry.revision`.

The registry’s cache is entry-local, not scene-local. Multiple scene resolutions can clone the same current snapshot.

### 6.2 Scene overlay

`ResolutionOverlay` (`scene/resolved.rs:11-30`) stores:

- `HashMap<ComponentId, ComponentSnapshot>`;
- state snapshots for retained-state attachments.

The overlay is resolution-branch-local and is used by layout/paint and attachment traversal. It avoids reconstructing component views during later layout operations.

`ResolvedScene` (`scene/resolved.rs:33-50`) additionally stores:

- `MountGraph`;
- `MountedCapabilities`;
- content paths;
- component paths;
- reverse component-to-content indexes.

The component path indexes are derived data used to localize invalidation/replacement work.

### 6.3 Mount graph storage costs

`MountGraph` retains:

- one hash-map entry per mounted component;
- one child vector per parent having children;
- a roots vector;
- one `MountNode` revision/parent record per component.

Traversal allocates a pending stack in `MountGraphIter`. `ids()` and `to_nodes()` create iterator/collection outputs as requested. The graph avoids a global contiguous node index rebuild during local subtree replacement.

### 6.4 Mounted capability storage

`MountedCapabilities` contains one capability structure for each resolved mounted component. Capability callbacks are `Arc`-backed erased closures, so snapshot cloning copies `Arc`s rather than cloning concrete component state.

Capability snapshots are replaced when resolution or incremental synchronization publishes a component update. There is no independent capability cache outside the component snapshot/scene structures.

### 6.5 Scheduler work

Per mount synchronization:

- iterates transition list;
- updates mounted `HashSet`;
- builds a graph ID `HashSet`;
- repairs/removes registrations;
- materializes `mount_order` from graph DFS.

Per capability synchronization:

- walks all graph IDs;
- constructs a desired registration map;
- creates or replaces drivers;
- scans registrations for stale IDs.

Per deadline query:

- scans `mount_order`;
- filters to registrations;
- finds minimum `next_due`.

Per tick pass:

- scans all `mount_order` IDs for due registrations;
- runs only due callbacks;
- reschedules each due callback from current `now`.

The scheduler is therefore O(number of mounted components) for deadline selection and due discovery, plus callback cost.

### 6.6 Resolver performance counters

`perf.rs:15-29` defines component-relevant counters:

- `ResolverNodesVisited`;
- `ComponentViewCalls`;
- `ComponentCapabilityCalls`.

The corresponding machine names are:

- `resolver_nodes_visited`;
- `component_view_calls`;
- `component_capability_calls`.

`ResolverNodesVisited` increments before the component-identity flag check (`scene/resolve.rs:543-550`), so even a component-free root scan contributes one visited node, while flag pruning avoids recursive descent.

### 6.7 Benchmark coverage

`perf_bench.rs` contains a `ComponentHeavy` workload:

- registers many `PerfComponent` instances;
- creates a column of component slots;
- resolves and renders the component-heavy view.

Evidence:

- `perf_bench.rs:96-111`
- `perf_bench.rs:151-180`

It also contains a live History component workload:

- registers one component;
- appends a component slot to History;
- mutates the registry with `with_any_mut`;
- renders repeatedly.

Evidence:

- `perf_bench.rs:501-523`

These benchmarks exercise registry resolution and component identity retention. They were indexed but not executed.

### 6.8 Native view-slot scheduling overhead

`MountedViewSlot` declares a fixed 16ms scheduler capability even when:

- no animation is active;
- there is only one frame;
- the component tick returns false after internal state inspection.

`HostViewSlot::tick` returns false for fewer than two frames, but the registration remains present while mounted. This provides a uniform native tick path but means registered view slots can participate in scheduler scans even when visually static.

The generic scheduler’s dirty result and the slot’s internal revision are separate:

- wrapper tick may return true on initial clock setup;
- actual frame advancement increments `ViewSlotState.revision`;
- host invalidation causes component snapshot invalidation through the component ID.

---

## 7. Tests, benchmarks and observability

### 7.1 Component-local tests

`component/tests.rs` covers:

- monotonic non-reused IDs (`tests.rs:35-45`);
- typed-handle isolation (`tests.rs:47-59`);
- cross-registry non-aliasing (`tests.rs:61-69`);
- revision changes only after successful mutable access (`tests.rs:71-87`);
- mutation changing the next resolution snapshot (`tests.rs:89-100`);
- stable identity with distinct visual ownership (`tests.rs:103-115`);
- stale handles failing resolution while existing resolved views remain compilable (`tests.rs:118-130`);
- nested component attachment and DFS mount order (`tests.rs:134-149`);
- local descendant replacement preserving owner and order (`tests.rs:153-171`);
- component metadata being physically invisible (`tests.rs:174-188`).

These tests establish that component identity is semantic/runtime metadata, not visual content.

### 7.2 Mount tests

`component/mount_tests.rs` covers:

- parent-first mount transitions and child-first unmount transitions (`mount_tests.rs:12-40`);
- reordering/reparenting existing IDs without remount (`mount_tests.rs:44-62`);
- revision-only graph changes producing no mount transitions (`mount_tests.rs:64-73`).

The reparenting test is particularly consequential: lifecycle transitions are membership-based, not parent-relation-change-based.

### 7.3 Tick tests

`component/tick_tests.rs` covers:

- no callback before deadline;
- callback exactly at deadline;
- revision advancement from tick mutation;
- multiple due components running in mount order;
- local capability synchronization after interval change;
- unmount deactivation and remount deadline reset;
- independent intervals and earliest-deadline selection;
- zero-interval declaration panic.

Evidence:

- `tick_tests.rs:57-96`
- `tick_tests.rs:99-130`
- `tick_tests.rs:134-169`
- `tick_tests.rs:172-225`
- `tick_tests.rs:229-267`
- `tick_tests.rs:269-296`

### 7.4 Interaction tests as component evidence

`interaction/tests/mod.rs` establishes:

- cyclic focus traversal over graph order;
- singleton focus traversal is ignored;
- typed key commands mutate components and emit outputs;
- ignored focused commands bubble to ancestors but not siblings;
- focus callbacks advance component revisions;
- removed focused components receive blur;
- losing focusability blurs using the prior capability;
- tick capability output is delivered through the output queue;
- disabling a dynamic tick capability removes future ticks;
- modal focus is contained and restored in nested order.

Relevant evidence includes:

- `interaction/tests/mod.rs:252-298`
- `interaction/tests/mod.rs:301-421`
- `interaction/tests/mod.rs:424-514`
- `interaction/tests/mod.rs:517-559`
- `interaction/tests/mod.rs:562-634`

These tests show that capability declarations are not passive metadata; callbacks mutate retained components and therefore participate in revision invalidation.

### 7.5 Scene tests as component evidence

Scene tests cover:

- nested component rereading from the registry on each resolution;
- component slot ownership shells;
- duplicate slot rejection;
- missing slots;
- cycles;
- state/content attachment traversal through component overlays.

The component-local tests are not the only behavior contract. Scene and interaction tests are required to understand actual component lifecycle.

### 7.6 Observability gaps

Observed limitations:

- no runtime counter specifically records mount transitions;
- no runtime counter separately records tick callback invocations;
- no runtime counter records capability registration churn;
- no direct production logging for unresolved/missing component IDs;
- deferred retirement state is internal and not exposed as a diagnostic snapshot;
- scheduler registration count is not externally observable;
- `ComponentHandle` debug output intentionally omits ID, which protects opacity but reduces test/log diagnostics.

Perf counters expose view/capability resolution and resolver visitation, but not full identity/mount/tick lifecycle behavior.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Item-level public versus crate-level private contradiction

`Component`, `ComponentCx`, and `ComponentHandle` look public at the item level:

- `component/mod.rs:29`
- `component/id.rs:41`
- `interaction/command.rs:68`

But:

- `lib.rs:15` declares `mod component`;
- `lib.rs:59` re-exports these symbols with `pub(crate)`;
- Cargo documentation says the crate is not a supported external Rust authoring package (`Cargo.toml:6-8`).

The actual supported boundary is therefore internal Rust runtime plus native/TypeScript facade, not public Rust authoring.

### 8.2 Component capability layer crosses interaction ownership

`component/capability.rs` is only a re-export facade. The actual capability structures and declaration methods live in `interaction/command.rs`.

This means component capabilities are structurally owned by the interaction subsystem even though component authors reach them through `component::ComponentCx`. The coupling is intentional and generic:

```text
Component::capabilities
    → ComponentCx
    → interaction::ComponentCapabilities
    → focus/routing/layout/tick consumers
```

The component module does not own the capability data model.

### 8.3 Component slots are not implemented in `component/slot.rs`

The assignment scope includes slots, but `component/slot.rs` is empty. Actual semantic slot representation is:

- `presentation::ir::ViewKind::ComponentSlot`;
- `presentation::ir::ComponentSlotNode`;
- `presentation::factory::component`;
- `presentation::factory::native_component`.

Actual native slot behavior is:

- `application::host::HostViewSlot`;
- `application::host::MountedViewSlot`;
- generic component registry and tick scheduler.

This is an important decomposition: “component slot” is a presentation-level indirection, while “view slot” is a native host wrapper implemented as a regular retained component.

### 8.4 Registry and scene host have overlapping lifetime responsibilities

The registry owns component memory, while `SceneHost` owns the last successfully reconciled mount graph. Native retirement requires both:

```text
Registry owns object lifetime
+
SceneHost owns committed semantic reachability
```

`RunningApp::reap_retired_components` joins these concerns by consulting `SceneHost::is_mounted` before calling `ComponentRegistry::remove_id`.

This coupling is necessary for safe deferred disposal but means component destruction cannot be reasoned about from the registry alone.

### 8.5 Mutable callback access is broader than semantic change

`with_mut` and `with_any_mut` increment revisions after every successful mutable callback invocation. This includes:

- key handlers returning `Ignored`;
- focus callbacks;
- paste callbacks;
- tick callbacks returning `false`;
- layout callbacks that may discover no actual size change.

This is conservative and simple, but revision changes represent “mutable callback access completed,” not necessarily a visible or semantic component change.

### 8.6 Generic tick and native slot tick differ

Generic component tick declarations reject zero intervals. Native `HostViewSlot` stores a caller-supplied animation interval separately and does not visibly perform the same non-zero assertion. Its outer component scheduler remains fixed at 16ms.

The two timing layers are therefore not equivalent:

```text
ComponentCx::tick:
    interval is scheduler registration interval
    zero rejected

HostViewSlot animation:
    interval is internal frame-advance interval
    outer component registration fixed at 16ms
```

This distinction matters when reasoning about caller-controlled animation pacing.

### 8.7 Mount topology and component identity are intentionally separate

A component revision update does not create mount transitions. A parent change for an existing ID also does not create mount transitions. The graph can therefore change topology while preserving component object identity and avoiding mount/unmount callbacks.

This is visible in:

- `MountGraph::same_topology`;
- `MountedComponents::reconcile`;
- `mount_tests.rs:44-73`.

The current mount lifecycle is membership-oriented rather than full attachment-lifecycle-oriented.

### 8.8 Historical architecture guidance versus current implementation

The pre-V5 report warns against assuming that the current component registry is equivalent to a future host configuration abstraction. Current source supports that caution:

- the registry owns concrete Rust component objects;
- component snapshots produce `View` trees;
- the mount graph is a derived semantic occurrence graph;
- native slots are wrappers around generic components;
- capabilities live in interaction machinery;
- scheduling lives in `TickScheduler`.

These responsibilities are distributed rather than represented by one universal host object. No V5 disposition decision is made here.

---

## 9. Open questions and coverage gaps

1. **Graph validation boundary:** `MountGraph::new` asserts duplicate IDs but does not independently validate missing parents or cycles. Resolver paths validate component cycles, but direct internal graph callers rely on trusted construction. It is unclear whether all future graph-producing paths preserve this assumption.

2. **Raw ID trust boundary:** non-zero raw IDs can be converted into semantic component slots without registry membership validation. Invalid IDs fail only during resolution. The native binding’s exact policy for accepting arbitrary `componentId` values was not fully traced.

3. **Component ID generation:** IDs are globally monotonic but have no separate generation/lease representation. The source does not show a reclaim-and-reuse policy; IDs are intentionally never reused until exhaustion.

4. **Cross-thread guarantees:** `Component` is `'static` but not `Send`/`Sync`, and `ComponentEntry` contains `RefCell`. The runtime owner/thread model is implied by `RunningApp` but not encoded in the component API itself.

5. **Mount/reparent callback semantics:** reparenting existing IDs emits no mount transitions. There are no component-level mount/unmount callback methods in the current trait, so the practical impact is limited to scheduler/focus/order state. Whether future consumers require explicit attachment notifications is not answered by current code.

6. **Zero native animation interval:** generic `ComponentCx::tick` rejects zero intervals, while `HostViewSlot` animation setters do not visibly reject zero. The intended contract for zero caller-supplied animation intervals is unclear.

7. **Scheduler missed-deadline behavior:** the scheduler reschedules from `now`, effectively dropping missed interval occurrences. This appears intentional, but there is no explicit policy type or observability counter describing the drop.

8. **Tick callback after physical registry removal:** a stale scheduler registration can call `with_any_mut`, receive `None`, return `false`, and advance its deadline. Deferred retirement is designed to prevent this for committed mounted components, but the exact behavior under unusual ordering/race conditions is not independently asserted.

9. **Capability snapshot revision coupling:** `MountedCapabilities` itself has no revision field. Host code updates graph revisions and capability entries together, but there is no standalone assertion proving that every capability map update corresponds to the same component snapshot revision.

10. **Public API classification:** item-level `pub` declarations could mislead consumers inspecting individual files, even though crate-level visibility prevents external Rust use. The source is internally consistent with Cargo/package documentation, but the distinction should remain explicit in integrated architecture documentation.

11. **Empty `component/slot.rs`:** the file is present but has no symbols or implementation. It is unclear whether this is an intentional reserved module, migration residue, or a placeholder left after slot logic moved to application host/presentation modules.

12. **Test execution:** all test evidence in this report comes from source inspection only. No tests or benchmarks were run during this investigation.

---

## 10. Evidence appendix

### 10.1 Primary component files and symbols

| Path | Symbols/evidence |
|---|---|
| `crates/iyon-tui/src/component/mod.rs` | `Component`, module declarations, crate-private exports |
| `crates/iyon-tui/src/component/capability.rs` | `ComponentCx` facade re-export |
| `crates/iyon-tui/src/component/id.rs` | `ComponentId`, `ComponentHandle`, ID allocation/raw conversion |
| `crates/iyon-tui/src/component/revision.rs` | `ComponentRevision`, `increment` |
| `crates/iyon-tui/src/component/registry.rs` | `ErasedComponent`, `ComponentSnapshot`, `ComponentEntry`, `ComponentRegistry`, registration/access/removal/resolution |
| `crates/iyon-tui/src/component/graph.rs` | `MountGraph`, `MountNode`, iterator, topology comparison, subtree replacement |
| `crates/iyon-tui/src/component/mount.rs` | `MountedComponents`, `MountTransition`, `MountTransitions` |
| `crates/iyon-tui/src/component/tick.rs` | `TickDriver`, `CapabilityTickDriver`, `TickRegistration`, `TickOutcome`, `TickScheduler` |
| `crates/iyon-tui/src/component/slot.rs` | Empty file; no symbols |
| `crates/iyon-tui/src/component/tests.rs` | identity, registry, resolution, nesting, replacement, visual invisibility tests |
| `crates/iyon-tui/src/component/mount_tests.rs` | mount/unmount ordering, reparenting, revision transition tests |
| `crates/iyon-tui/src/component/tick_tests.rs` | deadlines, order, remount, interval update, zero interval tests |

### 10.2 Supporting Rust source

| Path | Symbols/evidence |
|---|---|
| `crates/iyon-tui/src/id.rs` | `next_nonzero_id` checked atomic allocator |
| `crates/iyon-tui/src/interaction/command.rs` | `ComponentCapabilities`, `ComponentCx`, `TickCapability`, `MountedCapabilities` |
| `crates/iyon-tui/src/interaction/routing.rs` | `route_key_local`, `route_paste`, `route_paste_interceptor`, routing chain |
| `crates/iyon-tui/src/interaction/focus.rs` | capability-based focus eligibility, callback invocation, incremental focus reconciliation |
| `crates/iyon-tui/src/scene/resolve.rs` | `ResolveSession`, `Resolver`, `resolve_slot`, component errors, overlays |
| `crates/iyon-tui/src/scene/root.rs` | branch resolution, component subtree resolution, root merge, disjoint mount checks |
| `crates/iyon-tui/src/scene/resolved.rs` | `ResolutionOverlay`, `ResolvedScene` |
| `crates/iyon-tui/src/scene/host.rs` | committed graph, incremental component sync, mount reconciliation, ticker integration, retirement visibility |
| `crates/iyon-tui/src/application/context.rs` | `AppCx::register`, typed component access/removal |
| `crates/iyon-tui/src/application/kernel.rs` | registry ownership, native registration, deferred retirement, update loop, frame publication |
| `crates/iyon-tui/src/application/host.rs` | `HostViewSlot`, `ViewSlotState`, `MountedViewSlot`, native component wrappers, slot tick |
| `crates/iyon-tui/src/presentation/factory.rs` | `component`, `native_component`, `ComponentSlotNode` creation |
| `crates/iyon-tui/src/presentation/ir.rs` | `ViewKind::ComponentSlot`, component identity flags |
| `crates/iyon-tui/src/scroll.rs` | generic `ScrollPane` component capabilities and layout callbacks |
| `crates/iyon-tui/src/perf.rs` | resolver/component view/capability counters |
| `crates/iyon-tui/src/perf_bench.rs` | component-heavy and live component benchmark fixtures |

### 10.3 Supporting native source

| Path | Symbols/evidence |
|---|---|
| `crates/iyon-tui-native/src/tui.rs` | `NativeViewSlot`, native disposal, component ID access, view/animation reference methods, host creation route |

### 10.4 Documentation and manifest evidence

| Path | Evidence |
|---|---|
| `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md` | baseline, framework boundary, mandatory report headings, source discipline |
| `docs/architecture/atlas-4355c02/README.md` | assignment 02 scope and atlas structure |
| `docs/architecture/atlas-4355c02/evidence/assignments.json` | assignment ID, scope and goal |
| `PRE-V5-ARCHITECTURE-REPORT.md` | historical census expectations and component/scene/application-kernel questions |
| `AGENTS.md` | generic framework boundary, retained components, capabilities, slots, scheduling and no product semantics |
| `crates/iyon-tui/Cargo.toml` | crate-private/runtime implementation boundary and feature flags |
| `crates/iyon-tui/src/lib.rs` | private component module and crate-private re-exports |

### 10.5 Existing tests read, not run

- `crates/iyon-tui/src/component/tests.rs`
- `crates/iyon-tui/src/component/mount_tests.rs`
- `crates/iyon-tui/src/component/tick_tests.rs`
- `crates/iyon-tui/src/interaction/tests/mod.rs`
- relevant scene tests in `crates/iyon-tui/src/scene/tests.rs`
- relevant host tests in `crates/iyon-tui/src/scene/host.rs`
- relevant scroll tests in `crates/iyon-tui/src/scroll.rs`

### 10.6 Files indexed but not comprehensively read

The remainder of the repository, including unrelated Rust subsystems, TypeScript packages, native generated code, fixtures, examples, and documentation outside the required context, was not read comprehensively. They were outside this assignment’s component-focused scope.