# 40 — Lifetime, Leases, Generations and Caches

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Branch: `main`
- Scope: all three runtime planes:
  - TypeScript authoring/runtime/transport
  - handwritten native N-API/ABI layer
  - Rust retained runtime and state/snapshot ownership
- Parent-added atlas documentation was treated as investigation guidance, not as source-baseline code.
- `ARCHITECTURE.md` was not used; the contract states it was deliberately deleted.

I read the assignment contract and atlas README, including the required headings and evidence rules. I also reviewed the complete PRE-V5 handoff structure and its sections relevant to structural identity, generation/lease semantics, snapshots, residency, caches, native ABI, and failure behavior. This report describes the current implementation only. It does not make V5 disposition decisions.

### Scope boundary

The primary question for this assignment is how objects and derived data live, overlap, become stale, and are reclaimed:

- framework handles and native resources;
- semantic `NodeId` and native `NativeRef` identity;
- generation-scoped hints;
- temporary, desired, visible, root, and in-flight leases;
- structural attachment ownership;
- native weak caches and paged ref storage;
- Rust state snapshots and frame overlays;
- content/source snapshots and revisions;
- stale-reference recovery;
- cleanup on replacement, disposal, host teardown, and environment teardown;
- allocation behavior versus retained metadata and process RSS.

### Evidence status

This is a static source inspection report. I did not run tests, benchmarks, builds, or a live N-API process. Runtime behavior below is reconstructed from the source and existing tests. Existing tests are cited as behavioral evidence but are not claimed to have been executed for this report.

Facts are stated as implementation observations. Where a conclusion is inferred from ownership or allocator behavior, it is labeled as an inference. Where the source has no measurement or no explicit bound, that absence is stated.

---

## 1. Responsibility and structure

### 1.1 Three-plane lifetime map

| Plane | Primary lifetime owner | Main identity | Strong ownership | Weak/derived state |
|---|---|---|---|---|
| TypeScript framework/resource plane | `NativeResourceRegistry`, framework wrapper, host/boundary | `HandleId` plus resource generation | wrapper/resource while registered; prepared/desired/visible attachment leases | `WeakMap` handle/resource lookups, `FinalizationRegistry` cleanup |
| TypeScript structural transport plane | `RetainedRootBoundary`, `MaterializeTx`, semantic `View` graph | semantic `NodeId`, generation-scoped `NativeRef` hint | semantic `View`, root lease, temporary transaction leases | `SEMANTIC_NATIVE` hint, style sidecars, scratch buffers |
| Native ABI plane | environment-owned `NativeViewRuntime` | `NodeId`, monotonic `NativeRef`, path/builder/edit refs | `View` in leased slots and parent-owned retained graphs | `HashMap<NodeId, WeakView>`, weak slots, scavenge queue |
| Rust retained state plane | host-owned `ViewStateRegistry` | host-namespaced `u64` state identity | mutable `ViewStateRecord`, desired/visible/in-flight pins, `Arc<ViewStateSnapshot>` | committed snapshot table, candidate overlay |
| Rust content/source plane | source/resource owner and stream/connector state | source ID, source generation, content generation, revision | source chunks, annotation data, connector state | snapshots and projected/revision metadata |

### 1.2 Main files and approximate physical size

The following is an approximate production/test inventory for the lifetime-related implementation. Counts are based on inspected source extents and are intentionally rounded; generated bodies are separated from handwritten logic.

| Path | Language | Approx. production LOC | Approx. test LOC | Responsibility |
|---|---:|---:|---:|---|
| `packages/iyon-tui/src/runtime/handle-registry.ts` | TS | 80 | 0 | framework handle IDs, disposal bridge |
| `packages/iyon-tui/src/transport/native/resource-registry.ts` | TS | 480 | in nearby test files | environment-level resource ownership, generations, attachment leases |
| `packages/iyon-tui/src/transport/native/resources.ts` | TS | 90 | 0 | handle-local raw resource map and registry delegation |
| `packages/iyon-tui/src/api/controls/framework-handle.ts` | TS | 65 | 0 | nominal public wrapper and disposal API |
| `packages/iyon-tui/src/runtime/attachments.ts` | TS | 313 | 0 | semantic attachment discovery, prepare/desired/visible binding ledger |
| `packages/iyon-tui/src/api/view/semantic-node.ts` | TS | 365 | 0 | immutable semantic node identity, cached attachment presence, strong attachment references |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | TS | approximately 2,140 | embedded/nearby tests | retained identity hints, transaction-local leases, materialization, stale recovery, root boundary |
| `packages/iyon-tui/src/transport/structural/native-view-abi.ts` | TS | approximately 700 | 0 | N-API session, retained operation wrappers, ref release and host install |
| `packages/iyon-tui/src/api/content/retained.ts` | TS | approximately 800 | nearby tests | source/content handles, source snapshots, generations, connector lifecycle |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Rust | approximately 5,200 | approximately 700 embedded tests | native runtime, semantic cache, paged ref table, leases, ABI implementations, diagnostics |
| `crates/iyon-tui-native/src/tui.rs` | Rust | approximately 1,050 | 0 | N-API host/session wrappers, environment integration and teardown |
| `crates/iyon-tui/src/retained_state/registry.rs` | Rust | approximately 390 | approximately 100 embedded tests | state records, committed snapshots, candidate overlays, desired/visible/in-flight pins |
| `crates/iyon-tui/src/retained_state/capture.rs` | Rust | approximately 160 | approximately 40 embedded tests | frame snapshot overlay |
| `crates/iyon-tui/src/retained_state/presentation.rs` | Rust | approximately 300 | approximately 120 embedded tests | immutable `ViewStateSnapshot` and effective presentation state |
| `crates/iyon-tui/src/retained_state/record.rs` | Rust | approximately 180 | 0 | mutable source-of-truth state record and snapshot creation |
| `crates/iyon-tui/src/retained_state/occurrence.rs` | Rust | approximately 110 | 0 | occurrence-local geometry derived from snapshots |
| `crates/iyon-tui-native/src/generated/view_abi_*.rs` | generated Rust | generated | generated | schema-driven ABI wrappers and conformance |
| `packages/iyon-tui/src/transport/abi/structural/generated/*` | generated TS | generated | generated | schema-driven TS ABI wrappers and manifest |

The largest lifetime concentration is `view_abi.rs` plus `retained-dag.ts`. The TypeScript resource registry and attachment ledger are smaller but control cross-plane ownership correctness.

### 1.3 Primary and secondary responsibilities

#### `resource-registry.ts`

Primary:

- assigns and validates framework resource identity;
- associates wrappers and native resources;
- tracks lifecycle (`live`, `disposing`, `disposed`);
- tracks prepared, desired, and visible attachment leases;
- rejects cross-environment, wrong-host, wrong-kind, duplicate, and mounted disposal operations.

Secondary:

- weakly references wrapper and native resource;
- provides finalizer-based cleanup when wrappers become unreachable;
- exposes diagnostics through `stats()`.

#### `attachments.ts`

Primary:

- walks a semantic candidate and resolves all state/content attachments during H3 prepare;
- rejects duplicate attachment use;
- stages attachment leases;
- keeps desired and visible sets separate;
- handles superseded revisions while frames are in flight.

Secondary:

- cycle detection over semantic nodes;
- attachment-presence fast path;
- release ordering on replacement and disposal.

#### `retained-dag.ts`

Primary:

- maps semantic immutable nodes to native retained objects;
- maintains weak generation-scoped hints;
- acquires temporary leases for transaction-created or promoted refs;
- performs prepare/commit/abort root replacement;
- recovers from stale refs once per transaction.

Secondary:

- reuses transport scratch buffers;
- caches native style refs and style atoms;
- counts structural route work;
- records phase instrumentation.

#### `view_abi.rs`

Primary:

- owns the environment-level native semantic cache;
- maps `NodeId` to weak `View`;
- maps `NodeId` to `NativeRef`;
- owns native leases and paged ref storage;
- constructs retained Rust `View` values from ABI inputs;
- implements stale lookup, ref release, weak-cache maintenance, and runtime diagnostics.

Secondary:

- owns path definitions, axis builders, edit transactions, style atoms, and style refs;
- validates ABI limits and status details;
- owns environment cleanup and runtime liveness.

#### `retained_state/registry.rs`

Primary:

- owns mutable Rust `ViewStateRecord` instances;
- owns committed immutable snapshots;
- tracks desired, visible, and in-flight bindings;
- constructs candidate overlays;
- prevents disposal while a state is attached or a frame is in flight.

Secondary:

- deduplicates dirty IDs;
- preserves unmounted mutable state without publishing duplicate snapshots;
- removes committed versions once a state becomes unbound.

---

## 2. Types, APIs and contracts

### 2.1 TypeScript framework handles

`FrameworkHandle` exposes:

- `readonly id: HandleId`
- `readonly kind`
- `disposed`
- `dispose()`

The public ID is a JavaScript-local semantic identity, not a native pointer or `NativeRef` (`packages/iyon-tui/src/api/controls/framework-handle.ts:9-22`).

Construction calls `registerFrameworkHandle`, which:

1. allocates a global monotonic `HandleId`;
2. normalizes component-like kinds;
3. registers the wrapper/resource pair in the environment registry;
4. records the local wrapper-to-ID relation in a `WeakMap`.

The counter is global-realm state stored under `Symbol.for("iyon:tui:private-handle-counter")` (`packages/iyon-tui/src/runtime/handle-registry.ts:19-45`). IDs are not reused. `NativeResourceRegistry.retiredThrough` rejects registration at or below the highest retired ID (`resource-registry.ts:169-175`, `183-201`).

This is an anti-ABA property for framework identities: an old wrapper cannot be retired and then replaced by a new object with the same framework ID.

### 2.2 Native resource records and generations

Each `ResourceRecord` contains:

```text
handleId
kind
WeakRef(handle)
WeakRef(resource)
owner/environment/host
acceptedNodeKinds
generation
lifecycle
preparedLeases
desiredLeases
visibleLeases
```

(`packages/iyon-tui/src/transport/native/resource-registry.ts:34-47`).

The resource registry has a separate monotonic `nextGeneration`, incremented on every registration (`resource-registry.ts:179`, `209-228`). This generation is exposed by `PreparedResourceLease.generation` but is not itself embedded in a `HandleId` or attachment wire value.

Consequences:

- `HandleId` uniqueness and resource-generation uniqueness are distinct mechanisms.
- `HandleId` protects wrapper identity and retirement.
- `generation` identifies the registration incarnation of a resource record.
- Native structural `NativeRef` values do not carry this TypeScript resource generation.
- Attachment validation relies on live registry lookup, environment/host checks, kind checks, and lease state rather than on a generation word in the semantic node.

### 2.3 Resource lifecycle

A record transitions:

```text
live
  ├─ beginDisposal() ──> disposing
  │                         └─ no leases ──> disposed/retired
  ├─ wrapper finalizer with no leases ──> disposed/retired
  └─ prepareResolve ──> lease counts
```

`beginDisposal()` rejects mounted resources if any prepared, desired, or visible lease exists (`resource-registry.ts:340-359`). `release()` likewise rejects active leases (`361-368`).

If native disposal fails after tentative disposal, `cancelDisposal()` restores `live` only when all lease counts are zero (`376-384`). This is important: the registry does not permanently poison a resource merely because the native owner rejected a disposal attempt.

If the wrapper is garbage-collected while leases exist, `finalizeUnowned()` moves the record to `disposing` and preserves native use until outstanding leases drain (`450-460`). This prevents an unreachable wrapper from making a frame-bound native resource invalid mid-frame.

### 2.4 Prepared attachment lease

`PreparedResourceLease` has phases:

```text
prepared -> desired -> released
```

and an independent `visible` flag.

- `prepareResolve()` increments `preparedLeases`.
- `commitDesired()` changes one prepared lease to desired.
- `commitVisible()` promotes desired to visible.
- `releaseDesired()` removes desired ownership while preserving visible ownership if present.
- `releaseVisible()` drops visibility and then releases the lease if it is no longer desired.
- `abort()` drops a prepare lease without changing desired/visible state.
- `FinalizationRegistry` cleanup decrements counts if a lease object itself is abandoned.

Source: `packages/iyon-tui/src/transport/native/resource-registry.ts:49-162`.

The strong `keepAlive` pair in each prepared lease holds both the JS handle and raw native resource while prepared, desired, or visible (`84-100`). This is separate from count bookkeeping and is necessary because registry records themselves only contain weak references.

### 2.5 Semantic attachment identity

Semantic nodes contain opaque attachment IDs:

- `stateAttachment?: HandleId`
- `contentAttachment?: HandleId`

(`packages/iyon-tui/src/api/view/semantic-node.ts:186-190`).

The semantic node does not hold a registry record directly. To prevent the caller’s wrapper/resource from disappearing while the immutable semantic node remains reachable, `semantic-node.ts` keeps strong references in a `WeakMap<SemanticViewNode, object[]>`:

- `retainSemanticAttachmentReference()`
- `copySemanticAttachmentReferences()`

(`semantic-node.ts:297-336`).

The reference list is keyed weakly by semantic node. Thus:

- a reachable node can keep an attached wrapper/resource alive;
- when the node itself becomes unreachable, the sidecar does not independently keep it alive;
- immutable derivations explicitly copy the strong references;
- ordinary semantic child ownership remains through the semantic graph itself.

`semanticAttachmentPresence` is a separate weak summary used to avoid walking wide sequences simply to determine whether attachment validation is needed (`semantic-node.ts:299-320`).

### 2.6 Semantic `NodeId` and native `NativeRef`

The TypeScript semantic node has a stable positive safe-integer `id`. Native runtime identity is split:

```text
NodeId -> WeakView
NodeId -> NativeRef
NativeRef -> NativeViewSlot
```

`NativeViewSlot` contains:

```text
node_id
weak: WeakView
leased: Option<View>
js_lease_count
kind
```

(`crates/iyon-tui-native/src/tui/view_abi.rs:125-131`).

`NativeRef` values are monotonic and never recycled within one native runtime generation (`view_abi.rs:133-142`). Because refs are not reused, the implementation does not require an ABA generation field in each slot. A stale ref is a cache miss, not a possible alias to a newly allocated object.

This is a strong design distinction:

- `NodeId` is semantic identity;
- `NativeRef` is a runtime-local handle;
- the JS hint is merely an acceleration mapping;
- lease count determines whether the slot has a strong `View`;
- weak/native parent ownership can keep a zero-lease view alive even after JS temporary ownership ends.

### 2.7 Rust state snapshots

The Rust state registry maintains:

```text
records: HashMap<u64, Box<ViewStateRecord>>
committed: HashMap<u64, Arc<ViewStateSnapshot>>
dirty: HashSet<u64>
capture_epoch: u64
next_id: u64
desired: HashSet<u64>
visible: HashSet<u64>
in_flight: HashSet<u64>
```

(`crates/iyon-tui/src/retained_state/registry.rs:20-42`).

A state ID combines a host namespace and monotonic local slot:

```text
id = (host_id << 32) | local_id
```

The host ID is bounded to 21 bits and the local ID to `u32` (`registry.rs:68-81`). State IDs are therefore host-scoped and monotonic within the host registry.

`ViewStateSnapshot` is immutable and separates geometry and presentation revisions (`retained_state/presentation.rs:103-117`). A frame reads immutable `Arc<ViewStateSnapshot>` values through `StateFrameView`, optionally shadowed by a candidate overlay (`capture.rs:78-109`).

### 2.8 Content/source snapshots

TypeScript content APIs expose snapshots containing:

- `sourceId`
- `sourceGeneration`
- `contentGeneration`
- `revision`
- source range
- sealed/head-partial flags
- text
- annotation snapshots

(`packages/iyon-tui/src/api/content/retained.ts:69-102`).

Source generations are validated as non-negative 32-bit safe integers (`retained.ts:325-333`). Content generation and revision are widened to `bigint` in the TypeScript façade (`retained.ts:276-301`).

This is a content-plane generation model, distinct from:

- framework resource-registration generation;
- native ABI runtime generation;
- semantic `NodeId`;
- native `NativeRef`.

No single cross-plane generation token covers all of these identities.

---

## 3. Dependency and ownership map

### 3.1 Ownership diagram

```text
TS caller
  │ creates
  ▼
FrameworkHandle wrapper ────────┐
  │ HandleId                     │ strong while wrapper reachable
  ▼                              │
NativeResourceRegistry           │
  │ ResourceRecord               │
  │ WeakRef(handle/resource)     │
  │ prepared/desired/visible     │
  ▼                              │
semantic View node               │
  │ opaque state/content ID      │
  │ WeakMap strong attachment ref│
  ▼                              │
AttachmentBindingState           │
  │ PreparedResourceLease        │
  │ desired/visible/superseded   │
  ▼                              │
RetainedRootBoundary             │ root lease
  │ MaterializeTx                │ temporary leases
  ▼                              │
NativeViewAbiSession             │ environment-owned Arc
  ▼                              │
NativeViewRuntime                │
  ├─ NodeId -> WeakView
  ├─ NodeId -> NativeRef
  └─ NativeRef -> NativeViewSlot
       ├─ leased: Option<View>
       ├─ weak: WeakView
       └─ js_lease_count
             │
             ▼
        Rust View DAG
        parent retained Views keep children alive
```

For state attachments, the semantic `stateAttachment` ID is consumed by the native `view_state_attach` route and by the Rust state registry. The state registry is not owned by the semantic node; it is owned by the host/application runtime.

### 3.2 Owner/create/destroy table

| Object | Created by | Strong owner | Destroy/release trigger |
|---|---|---|---|
| `FrameworkHandle` | TS wrapper constructor | caller/wrapper references | explicit `dispose()`, finalizer path |
| raw native resource | native factory or control constructor | wrapper, lease `keepAlive`, native owner | native dispose followed by registry retirement |
| `ResourceRecord` | `NativeResourceRegistry.register()` | registry map | `retireRecord()` after no leases |
| semantic node | `createSemanticViewNode()` / `View` constructors | immutable `View` and child graph | JS GC when no `View`/derived graph remains |
| semantic attachment reference list | attachment constructor/derivation | node sidecar keyed by weak node | disappears with node |
| `NativeRef` slot | native publication | runtime slot table; strong `leased` view only while lease count > 0 | release to zero plus weak expiry, or stale cleanup |
| `NodeId -> WeakView` entry | native publication | runtime map weak value | explicit replacement/miss cleanup/full sweep |
| root lease | `RetainedRootBoundary` | previous/desired/visible role | root replacement, visibility promotion, close |
| temporary ref lease | `MaterializeTx` | transaction list | commit transfer, `releaseAllExcept`, abort |
| Rust state record | `ViewStateRegistry.create()` | registry `records` map | explicit state disposal after all bindings clear |
| `Arc<ViewStateSnapshot>` | registry capture/mutation | committed map and candidate overlay | unbound pruning, candidate/frame completion |
| source snapshot | native source `snapshot()` call | returned TS object | caller object lifetime; native source remains owner |
| style sidecar entry | TS retained transport | `WeakMap` style object or `Map` atom | generation/runtime reset or GC for weak key; atom map reset |
| scratch buffer | TS module-level sidecar | process/module | runtime switch resets; otherwise capacity grows and remains |

### 3.3 Reverse dependencies

- `FrameworkHandle` depends on the runtime handle registry and transport/native resource lookup.
- The native resource registry is used by:
  - framework handle construction/disposal;
  - semantic attachment preparation;
  - retained-DAG materialization for content/state resources.
- `AttachmentBindingState` is used by `ViewSlot` and `ScrollPane`.
- `RetainedRootBoundary` is used by those controls and host rendering paths.
- `retained-dag.ts` calls generated TS ABI wrappers, which invoke generated native N-API methods, which call handwritten `view_abi.rs`.
- `view_abi.rs` calls binding-level Rust `View` constructors and retained-child replacement APIs.
- Rust state snapshots are consumed by retained-state frame preparation, occurrence state application, layout/presentation, and paint-related paths.

### 3.4 Key ownership invariant

A semantic node may be reachable without being natively materialized. A native ref may remain in the native cache without a JS-owned lease. A native `View` may remain live without a JS lease because:

1. the native slot’s weak reference can still upgrade;
2. a parent `View` can own a child;
3. a native root or another boundary may own an independent lease.

Thus none of the following are equivalent:

```text
semantic node reachable
native ref slot present
native View live
JS lease count > 0
visible in current frame
resource registry attachment visible
```

The source intentionally tracks these separately.

---

## 4. Execution paths and state transitions

### 4.1 Trace A — first structural materialization

Assume a caller creates a semantic column containing text.

#### TypeScript creation

1. `View` constructors allocate semantic `NodeId`s and freeze semantic node objects.
2. The text/column `View` contains immutable semantic data and no native ref.
3. If an attachment exists, the constructor or derivation records:
   - opaque attachment ID in the semantic node;
   - a strong wrapper/resource reference in the node sidecar.

Relevant files:

- `packages/iyon-tui/src/api/view/semantic-node.ts`
- `packages/iyon-tui/src/api/view/view.ts`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`

#### Boundary prepare

`RetainedRootBoundary.prepareInstall()` creates a `MaterializeTx` containing:

- ABI symbols/runtime;
- current ABI generation;
- `nativeLookupCeiling`;
- transaction-local `refs`;
- `temporaryLeases`;
- borrowed hint tracking;
- one stale-ref retry allowance.

`ensureSemanticNative()` resolves each node in this order:

1. generation-matching `SEMANTIC_NATIVE` hint;
2. transaction-local ref;
3. if `node.id <= nativeLookupCeiling`, `NodeId -> NativeRef` promotion through native ABI;
4. direct semantic payload inspection/materialization;
5. child traversal from the materializer.

Source: `retained-dag.ts:1148-1190`.

For a genuinely new node above the ceiling, no extra `NodeId` probe is made. The materializer recursively creates children first, then calls an ABI constructor.

#### Native publication

The native constructor:

1. validates the node ID and payload;
2. checks the semantic cache;
3. constructs a Rust `View`;
4. calls `runtime.publish(node_id, view)`;
5. allocates a monotonic native ref if necessary;
6. inserts:
   - weak semantic cache entry;
   - node-ref association;
   - `NativeViewSlot`;
7. returns a ref with one lease for ordinary `PublicationLease::Leased`.

`install_semantic_view()` is the common installation path (`view_abi.rs:1064-1081`). `publish_semantic_view()` applies identity conflict checks and lease mode (`1083-1134`).

#### Transaction ownership

Every newly published ref is placed in `tx.temporaryLeases`. The root is eventually transferred to the boundary; non-root refs are released in one `viewReleaseMany` batch.

The native `View` itself may survive after a non-root temporary lease drains if the newly constructed root’s parent graph owns it.

### 4.2 Trace B — exact-root reuse

If the boundary already holds the current root and the same semantic node is rendered again:

1. `SEMANTIC_NATIVE` can yield a generation-matching hint.
2. `hostRenderRef` can render the exact root without semantic field reads or payload buffers.
3. The hint does not itself acquire a lease.
4. The boundary must already own its root lease, or must promote through `NodeId` to acquire an independent lease before treating the ref as its root.

The source explicitly distinguishes borrowed hint use from root lease ownership (`retained-dag.ts:1491-1507`, `1644-1654`).

### 4.3 Trace C — root replacement, successful commit

For a direct boundary replacement:

```text
old root leased
  ↓
prepare new root
  ↓
materialize/reuse candidate
  ↓
host install/render candidate
  ↓ success
release old root
transfer candidate temporary lease to boundary root
release all other temporary leases
capture NodeId high-water
```

`RetainedRootBoundary` documents this sequence at `retained-dag.ts:1488-1503`.

For an H3 deferred host boundary:

```text
old visible root remains visible and leased
  ↓
prepare candidate
  ↓
commit desired root
  ↓
candidate remains desired and leased
  ↓
backend frame succeeds
  ↓
commitVisible(revision)
  ↓
candidate becomes visible
  ↓
old visible lease is released
  ↓
superseded desired revisions are released when ordered visibility catches up
```

`desiredRef` and `visibleRef` are deliberately separate (`retained-dag.ts:1572-1585`). Revision ordering uses decimal-string normalization and `BigInt` comparison.

### 4.4 Trace D — root replacement abort/failure

If prepare fails:

- old root remains installed;
- old root lease remains untouched;
- `MaterializeTx.releaseAll()` releases every temporary lease;
- no desired/visible attachment ledger is committed;
- no host mutation occurs.

If native host installation fails after materialization, the candidate is released and the old root remains. `prepareInstall()` makes the prepare/commit boundary explicit: `commit()` should contain only the already-validated publication and bookkeeping, while `abort()` unwinds all acquired resources (`retained-dag.ts:1510-1529`, `1679-1702`).

This is stronger than merely restoring a pointer: it preserves old native liveness and avoids exposing a partially installed semantic/native graph.

### 4.5 Trace E — stale JS hint or native ref

A stale path can occur when:

1. a semantic node retains an old `SEMANTIC_NATIVE` hint;
2. the native ref has been released and the weak `View` has expired;
3. a later boundary attempts to use the hint.

`ensureSemanticNative()` first accepts a generation-matching hint without probing. The native constructor then returns `FAST_CACHE_MISS`. `materializeWithRecovery()` can:

1. identify a stale child ordinal or stale base;
2. delete the stale semantic hint;
3. remove the transaction-local ref;
4. retry that semantic node once through the same retained materializer;
5. turn a second native failure into `RetainedRefusalError`.

The retry limit is one per root transaction (`retained-dag.ts:415-467`). There is no secondary compatibility/legacy transport route. An unrecoverable refusal returns an explicit retained failure.

Native stale behavior is also self-cleaning:

- `resolve_ref()` finds a slot;
- if `leased` is absent and `WeakView` cannot upgrade:
  - removes `node_refs[node_id]`;
  - removes the native ref slot;
  - increments expiration counters;
  - returns `FAST_CACHE_MISS`.

Source: `view_abi.rs:1006-1024`.

### 4.6 Trace F — attachment prepare and replacement

`prepareAttachmentsForView()` traverses the semantic node only when the cached attachment-presence summary says attachments may exist.

During traversal:

1. cycles are rejected;
2. state and content attachment IDs are looked up;
3. duplicate attachment IDs of the same category are rejected;
4. `NativeResourceRegistry.prepareResolve()` validates:
   - live record;
   - not disposing;
   - expected kind;
   - target environment;
   - target host;
   - accepted semantic node kind;
5. a `PreparedResourceLease` is accumulated.

If any attachment fails, all already-prepared leases call `abort()`.

On successful replacement:

1. root structural preparation completes;
2. publication commits;
3. attachment prepared leases become desired;
4. attachment desired leases become visible;
5. previous visible/desired attachment lease sets are released according to revision/ordering rules.

`AttachmentBindingState` holds:

```text
desired
visible
superseded[] = { revision, leases[] }
desiredRevision
visibleRevision
```

(`attachments.ts:55-152`).

A no-revision replacement releases superseded and old desired state immediately. Revision-aware replacement retains superseded desired leases until the corresponding visible revision is committed. This is necessary for backend frames that can be in flight, but it means the superseded list is bounded by frame/revision acknowledgment, not by a fixed compile-time count.

### 4.7 Trace G — state mutation and snapshot capture

Rust state mutation:

1. finds mutable `ViewStateRecord`;
2. rejects missing or disposed records;
3. applies validated mutation;
4. returns immediately for no-op effects;
5. if the record is desired, visible, or in flight:
   - creates a new immutable snapshot;
   - inserts it into `committed`;
6. marks the state ID dirty.

Source: `registry.rs:84-120`.

Frame capture:

1. increments `capture_epoch`;
2. unions desired, visible, and in-flight state IDs;
3. sorts demanded IDs;
4. for changed/newly demanded/missing committed entries, obtains an `Arc<ViewStateSnapshot>`;
5. stores those in a `StateCandidateOverlay`;
6. removes served IDs from the dirty set;
7. leaves dirty marks for undemanded states queued.

Source: `registry.rs:123-165`.

A clean demanded state is read directly from the committed table. An unmounted state has no duplicate immutable snapshot merely because it exists; it retains only its mutable source of truth until demanded.

### 4.8 Trace H — state attachment replacement

The native state attachment route is `view_state_attach_impl()`:

1. resolve `base_ref`;
2. verify state capability;
3. if the requested state ID is already attached, return `base_ref`;
4. require `node_refs[node_id] == base_ref`;
5. create a new `View` with state attachment;
6. remove old weak/node-ref mappings temporarily;
7. publish the attached `View`;
8. on success, release the ordinary constructor lease on `base_ref`;
9. on failure, restore old mappings.

Source: `crates/iyon-tui-native/src/tui/view_abi.rs:1780-1832`.

This replacement is identity-sensitive: the supplied base ref must still be the current ref for the semantic node. A stale or mismatched base is invalid rather than silently patched.

### 4.9 Trace I — disposal and teardown

#### Resource disposal

`FrameworkHandle.dispose()` calls the registry/native disposal path. Mounted resources are rejected until attachment leases drain. Once unmounted:

- native resource disposal runs;
- registry record retires;
- wrapper/resource weak maps are cleared;
- retired wrapper/resource weak sets prevent re-registration.

#### View boundary close

`RetainedRootBoundary.close()`:

- releases desired and visible roots, avoiding duplicate release when the refs are equal;
- releases superseded desired roots;
- clears desired/visible nodes and revisions;
- is idempotent.

#### Host disposal

Native host disposal calls:

- `abort_all_edit_txns()` to discard runtime-scoped builders/edit transactions that could retain strong staged `View`s;
- host close;
- host `alive` state transition.

The edit/builder tables are runtime-scoped rather than host-scoped, so host disposal conservatively clears all uncommitted transaction state for that environment.

#### Environment teardown

`NativeViewRuntime` is stored in an `Arc` map keyed by N-API environment address. The environment cleanup hook:

- sets `alive = 0`;
- removes the runtime from the environment map.

Subsequent calls fail `valid_on_owner_thread()` with the closing/runtime-invalid status. The runtime’s `generation` is initialized to `1`, and the source shown does not increment it during cleanup; liveness is instead invalidated by `alive` and removal from the environment registry.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Primary production path | Miss/recovery behavior | Failure masking |
|---|---|---|---|
| Materialize new semantic node | `RetainedRootBoundary.prepareFrom` → `MaterializeTx` → `ensureSemanticNative` → generated ABI → `view_abi.rs` constructor → `publish` | direct semantic payload path for IDs above lookup ceiling | retained refusal becomes explicit failure; no secondary transport |
| Reuse known semantic node | generation-matching `SEMANTIC_NATIVE` hint | native status miss triggers one targeted stale retry | stale retry limited to one per transaction |
| Promote old semantic node | `viewRefForNodeId` | native `FAST_CACHE_MISS` falls through to direct materialization where allowed | miss is not treated as proof of semantic invalidity |
| Root install | prepare candidate, host/ref install, transfer root lease | abort leaves old root installed | host failure does not release old root |
| Deferred frame visibility | desired root commit, then `commitVisible(revision)` | out-of-order/older revisions ignored | stale visibility callbacks are ignored, not applied |
| Attachment resolve | `prepareSemanticAttachments` → registry `prepareResolve` | candidate abort releases all prepared leases | disposed/wrong-host/wrong-kind errors are explicit |
| State mutation | Rust record mutation → committed snapshot if demanded → dirty queue | undemanded records remain mutable-only | disposed state rejects mutation |
| State frame capture | demanded set union → immutable `Arc` snapshots and overlay | clean committed values reused | undemanded dirty marks stay queued |
| Release native refs | `viewReleaseMany` batch | zero-lease/live refs enter scavenge queue; expired weak refs removed | repeated/bad release behavior is mostly status/count based |
| Weak cache cleanup | bounded queue, threshold full sweep, explicit maintenance | stale lookup also removes individual entries | cleanup delay is bounded by maintenance policy, except non-weak maps |
| Source snapshot | native `snapshot()` → TS normalization to bigint | invalid numeric shape throws | native source remains owner; snapshot is a copy |

### 5.2 Native status classes

The native ABI uses:

- `FAST_INVALID`
- `FAST_CACHE_MISS`
- `FAST_REFUSED`
- `FAST_INTERNAL`

with status detail bits identifying stale child/base cases (`view_abi.rs:49-62`).

TypeScript recognizes expected native status-shaped errors in retained transport. It retries stale child/base misses but turns ordinary native constructor failure into explicit `RetainedRefusalError`.

The source explicitly disallows choosing a previous-generation or complete-object fallback path after retained refusal. This is a route invariant rather than an incidental error-handler choice.

### 5.3 Release underflow observation

`release_one_lease()` rejects a zero-count release (`view_abi.rs:1275-1287`).

`release_many()` behaves differently: it uses `saturating_sub(1)` on each slot and does not return an invalid status for a zero-count slot (`view_abi.rs:1289-1323`). It removes the slot only if, after decrement, the lease count is zero and the weak view has expired. Therefore:

- ordinary code is expected to release one entry per acquired lease;
- repeated batch release is not necessarily surfaced as an error;
- a repeated release can silently leave the slot at zero;
- correctness depends on higher-level “one release per acquired lease” discipline.

The TS transaction code explicitly preserves duplicate ref entries in release arrays to avoid under-releasing or over-coalescing repeated child occurrences (`native-view-abi.ts:369-399`; `retained-dag.ts:327-339`).

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 TypeScript cache inventory

#### Semantic native hint

`SEMANTIC_NATIVE` is:

```text
WeakMap<SemanticViewNode, { generation, nativeRef }>
```

Properties:

- weak key, so it does not keep semantic nodes alive;
- generation-scoped;
- acceleration only;
- no lease ownership;
- invalidated explicitly after confirmed stale ref;
- recreated by `refreshNativeHint()` after a lease-bearing NodeId promotion.

Source: `retained-dag.ts:52-60`, `1122-1142`.

The weak-key property bounds retention by reachable semantic nodes, but the native ref in the value is only metadata. It cannot independently keep the native view live.

#### Style sidecars

`STYLE_REF_CACHE` contains:

```text
runtime
generation
WeakMap<style object, native style ref>
Map<string, native atom ref>
```

It is reset when runtime or ABI generation changes (`retained-dag.ts:91-103`, `638-645`). The style-object map is weak-keyed. The atom map is strong and remains until generation/runtime reset.

`resetStyleRefCacheForThemeChange()` exists because theme-key resolution can change after host theme replacement. The native style table is intended to remain authoritative, while TS sidecars are acceleration metadata. The exact invalidation function is at `retained-dag.ts:2127-2134`.

#### Transport scratch buffers

The module retains:

- one reusable axis-ref array per active materialization depth;
- one reusable grid-word array per active materialization depth;
- one byte scratch array;
- style sidecar state.

The arrays grow to the largest request at that recursion depth and are not shrunk. This is an allocation/RSS distinction: logical work may return to zero, but process memory can remain at the high-water capacity.

`MaterializeTx.byteScratch()` refuses payloads above `MAX_DIRECT_TEXT_BYTES`, but it otherwise grows to the request and keeps the buffer (`retained-dag.ts:292-311`).

### 6.2 Native semantic cache

`NativeViewRuntime` owns:

```text
nodes: HashMap<u64, WeakView>
node_refs: HashMap<u64, u32>
slots: NativeRefTable
```

A semantic publication increments `nodes_inserted_since_full_sweep`. Weak entries are removed:

- opportunistically on `consult_semantic_identity`;
- when `resolve_ref` discovers an expired weak slot;
- by bounded scavenge;
- by full prune.

### 6.3 Native ref table and pages

`NativeRefTable` uses 4096-slot pages (`NATIVE_REF_PAGE_BITS = 12`).

Properties:

- ref lookup is page/offset vector access rather than hash lookup;
- refs are monotonic and never recycled within a runtime generation;
- an empty page’s boxed slot allocation is dropped;
- the outer `pages: Vec<Option<Box<NativeRefPage>>>` keeps its high-water length;
- stale refs into a dropped page return cache miss.

Source: `view_abi.rs:133-180`, `191-280`.

This gives physical page reclamation but not necessarily complete allocator/RSS reclamation:

- the page box and its slot array drop when page live count reaches zero;
- the outer directory capacity/high-water vector remains;
- allocator arenas may retain freed memory;
- `HashMap` bucket capacity may remain after entries are removed.

### 6.4 Native maintenance policy

Constants:

```text
SCAVENGE_BATCH_BUDGET = 256
FULL_SWEEP_METADATA_GROWTH_THRESHOLD = 4096
```

(`view_abi.rs:64-68`).

On release, zero-lease refs are placed in `scavenge_queue`. The next bounded maintenance pass checks at most 256 queued candidates. A candidate is removed only if:

```text
js_lease_count == 0
and WeakView cannot upgrade
```

When semantic weak metadata growth since the last full sweep reaches 4096 inserted nodes, `prune_expired()` runs.

Full prune:

1. scans `nodes` for expired weak entries;
2. removes corresponding `node_refs`;
3. removes unleased expired slots;
4. scans all slots for unleased expired weak views;
5. updates counters.

Source: `view_abi.rs:1183-1273`.

The intended logical bound is:

```text
live semantic state + bounded maintenance slack
```

However, this is a logical-entry bound, not a process-RSS bound.

### 6.5 Native auxiliary maps with weaker retention bounds

The native runtime also owns:

```text
path_nodes
path_keys
builders
edit_txns
style_atoms
styles
```

Path refs and path keys are monotonic and not reclaimed during ordinary maintenance. They are bounded by reference ranges and path depth/selector limits, but the source does not provide an LRU or deletion policy.

Builders and edit transactions have explicit count/size limits:

- maximum path depth: 128;
- maximum edit count: 256;
- maximum staged objects: 4,096;
- maximum new text bytes: 16 MiB;
- builder child count bounded by ABI policy.

Uncommitted builders/edit transactions are explicitly cleared at host disposal. There is no evidence in the inspected source of periodic timeout cleanup for a builder or edit transaction abandoned without host disposal.

Style atoms and style refs use monotonic allocation ranges and equality lookup. The maps do not have ordinary eviction. Their logical values may be reused for equal style values, but distinct values accumulate until environment teardown.

### 6.6 Attachment cache/ledger bounds

`AttachmentBindingState.superseded` retains revisions whose desired root has been replaced but whose visible frame may still be in flight.

This is semantically necessary for out-of-order frame visibility. The source has no fixed maximum number of superseded revisions. The practical bound is the number of desired-but-not-yet-acknowledged frames. If the backend or host stops acknowledging visibility, old desired leases can remain retained until boundary disposal.

This is a consequential distinction from the native weak cache:

- native weak cache retention is maintenance-bounded;
- attachment supersession is acknowledgment-bounded;
- a backend stall can retain desired attachment leases and prevent disposal.

### 6.7 Rust state cache and snapshot bounds

The Rust state registry intentionally separates:

```text
mutable records
committed immutable snapshots
candidate overlays
desired / visible / in_flight membership
dirty IDs
```

Snapshot production is demand-aware:

- unmounted records do not receive duplicate committed snapshots on mutation;
- demanded mutation publishes a new `Arc<ViewStateSnapshot>`;
- clean values reuse existing committed snapshots;
- committed snapshots are pruned when the state is no longer desired, visible, or in flight;
- in-flight pins prevent premature disposal.

The candidate overlay can retain `Arc` snapshots for a frame. Its memory is proportional to demanded/changed IDs in that frame, not all records.

Dirty marks for undemanded states intentionally persist. Therefore a large set of detached-but-mutated states can retain dirty-set entries, though not immutable snapshots for each mutation.

### 6.8 Scheduling interactions

The lifetime structures are coupled to scheduling at two points:

1. H3 desired/visible publication:
   - desired structural and attachment state is committed before the frame is visible;
   - visible ownership changes only after backend receipt/frame success.
2. Rust in-flight state pins:
   - candidate preparation marks IDs in flight;
   - completion clears only IDs belonging to that candidate;
   - newer desired operations are kept separate.

This prevents a later update from releasing data still needed by an earlier frame.

---

## 7. Tests, benchmarks and observability

### 7.1 Existing native lifecycle tests

`crates/iyon-tui-native/src/tui/view_abi.rs` contains focused tests including:

- generated spacer publish/lookup/release;
- bulk publication sharing the environment ref table;
- constructor lease count starts at one;
- child temporary lease remains live through parent ownership;
- batch release of temporary leases;
- root lease transfer on replacement;
- failed host install retaining old root;
- failed transaction releasing every temporary lease;
- stale unleased weak slot returns cache miss;
- slot metadata scavenged after weak expiry;
- repeated `NodeId` lookup acquires independent leases;
- stale path base returns cache miss then recovers.

Representative locations:

- `view_abi.rs:4757-4796`
- `view_abi.rs:4928-5109`
- `view_abi.rs:5162-5191`

The tests make the intended lease model explicit:

- an ordinary constructor returns one lease;
- repeated `ref_for_node_id` calls increment lease count;
- releasing one of two leases leaves one;
- releasing the last lease may leave an unleased-live slot if a parent still owns the `View`;
- if no owner remains and weak upgrade fails, the slot is removed.

### 7.2 Rust state tests

`retained_state/registry.rs` has embedded tests for:

- generationally unique monotonic state IDs;
- snapshot publication only for demanded state;
- overlay shadowing committed versions;
- binding/disposal restrictions;
- in-flight lifecycle pin behavior.

`retained_state/capture.rs` tests that overlays shadow committed entries without copying clean entries.

### 7.3 Instrumentation

#### TypeScript retained identity counters

`retained-dag.ts` exports counters for:

- hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast paths;
- ref words written;
- byte payload bytes;
- scratch reuse;
- stale-ref retries;
- decorated-node normalization;
- host mutations.

These counters distinguish logical route work from wall-clock timing.

#### Native diagnostics

`view_abi.rs` exposes:

- semantic cache entries;
- live semantic cache entries when requested;
- native ref slots;
- leased slots;
- unleased live slots;
- node-ref entries;
- path nodes/keys;
- builders/edit transactions;
- style atoms/style refs;
- scavenge queue length;
- processed candidates;
- full sweep count;
- expired/removed counters;
- pages and pages freed.

`view_abi.rs:1489-1513` and `1611-1663`.

#### Explicit maintenance

N-API exposes `tuiViewAbiMaintain(full)`:

- `full = false`: bounded scavenge;
- `full = true`: full expired-weak sweep.

This is intended for tests/benchmarks and is not itself evidence that production scheduling invokes maintenance at a particular cadence.

#### Memory snapshot limitations

`runtime_memory_snapshot` explicitly reports:

```json
"string_bytes": null
```

because retained text/style payload byte accounting is not tracked by the runtime (`view_abi.rs:1657-1659`).

The snapshot reports counts, not bytes, and `count_live` controls whether potentially expensive live weak upgrades are performed. No RSS or allocator statistics are exposed by the inspected code.

### 7.4 What the observability can and cannot prove

It can prove:

- logical entries and lease counts;
- weak cache entry counts;
- page count and page frees;
- maintenance work;
- retained-transport route work;
- text byte payloads sent/encoded;
- state snapshot/binding counts if corresponding Rust diagnostics are used.

It cannot directly prove:

- native heap bytes;
- JS heap bytes;
- allocator fragmentation;
- process RSS;
- bytes retained by each `View`;
- `HashMap` capacity after removal;
- capacity of TS scratch arrays;
- memory retained by N-API/runtime object wrappers;
- actual GC timing for weak maps/finalizers.

Therefore a zero logical-cache count does not imply RSS has returned to baseline.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Multiple independent generation systems

The implementation has at least four generation/revision families:

1. TypeScript resource registration generation;
2. native ABI runtime generation;
3. source/content generation and revision;
4. Rust state capture epoch and state revisions.

They serve different purposes and are not interchangeable.

The native ABI runtime generation is initialized to `1` in `NativeViewRuntime::new()` (`view_abi.rs:399-441`). The inspected cleanup path invalidates `alive` and removes the environment map entry but does not visibly increment this generation. JS hints are therefore invalidated across runtime object changes primarily because:

- the runtime object identity changes;
- cached session/runtime references become invalid;
- `valid_on_owner_thread()` rejects dead runtimes.

The generation number is still useful for same-runtime sidecars and ABI metadata, but it is not a universal teardown epoch.

### 8.2 Native ref monotonicity avoids ABA but creates high-water metadata

Native refs are never recycled. This prevents stale JS/native refs from accidentally referring to a new object. The cost is monotonic high-water allocation and an outer page directory whose length can track the largest ref ever allocated.

Page payloads are freed when empty, but the directory and allocator behavior can keep address space or RSS elevated. This is not a correctness problem but is relevant to memory investigations.

### 8.3 Weak cache versus strong retained graph

The native semantic cache is weak, but retained parent `View`s are strong. A child may be absent from JS leases yet remain live because it is owned by a parent retained graph. This is intentional and tested.

Conversely, a weak cache entry may outlive the underlying `View` temporarily. It is logically stale metadata and is removed on lookup or maintenance. Thus “cache entry exists” does not imply a live native object.

### 8.4 Attachment ownership is separate from structural ownership

A semantic `View` can own a strong wrapper/resource reference through its sidecar while the resource registry separately tracks attachment leases. Structural root replacement does not automatically mean attachment disposal. Attachment disposal requires:

1. the desired/visible binding ledger to release its leases;
2. no other semantic candidate/boundary to retain the resource;
3. explicit resource disposal or finalization.

This separation is necessary when the same resource is used by a structural candidate, visible frame, or another boundary, but it also creates more states to inspect during leak diagnosis.

### 8.5 Delayed cleanup is intentional but asymmetric

The implementation has several cleanup delays:

- native weak entries await lookup, bounded maintenance, or full sweep;
- superseded attachment/root revisions await visible revision acknowledgment;
- Rust snapshots await unbinding and frame completion;
- source chunks remain until source policy/disposal;
- TS scratch buffers remain at high-water capacity;
- native style/path metadata can remain until environment teardown.

These are not all “leaks.” They represent different retention semantics:

```text
semantic lifetime
physical residency
frame-in-flight lifetime
derived-cache lifetime
allocator capacity
```

A diagnostic must identify which category grew.

### 8.6 Failure semantics are generally transactional

The strongest common contract across planes is:

```text
prepare can allocate/acquire and fail;
commit should only promote already-valid state;
abort releases staged state and preserves old visible state.
```

Evidence:

- `PreparedAttachmentSetImpl.abort()`;
- `RootPublication.abort()`;
- `MaterializeTx.releaseAll()`;
- Rust `prepare_visible`/`commit_visible_prepared`;
- native staged publication prepare/commit;
- state in-flight pinning.

This is a consequential coupling: resource disposal and state disposal are intentionally forbidden while the corresponding prepare/desired/visible/in-flight ownership exists.

### 8.7 Potentially surprising native release behavior

The native one-ref release path is strict, while batch release saturates. This asymmetry is not documented as a user-facing contract. It means diagnostics may show an apparently successful batch release even when a caller supplied a ref whose JS lease count was already zero.

The higher-level TypeScript code takes care to preserve multiplicity in temporary lease arrays, so the intended route is correct. Nevertheless, repeated-release misuse may be silently tolerated rather than reported.

### 8.8 Content snapshot copies are explicit allocations

TypeScript source snapshots copy native text and annotations into JS structures. UTF-8 attachment payloads are copied into `Uint8Array`s. A content snapshot is therefore a value snapshot, not a borrowed native view.

The native source reports both logical retained bytes and accepted/copied/dropped byte counters, but these are source-domain counters, not process memory measurements.

---

## 9. Open questions and coverage gaps

1. **Exact production maintenance cadence**
   - The source exposes native bounded/full maintenance hooks, but this inspection did not establish every production caller and cadence.
   - A route audit should verify whether maintenance runs on each release, frame, wake, or explicit diagnostic call.

2. **Native `HashMap`/`Vec` capacity after pruning**
   - Logical entry counts are observable.
   - Capacity and allocator retention are not exposed.
   - This is central to RSS diagnosis.

3. **Runtime generation rollover**
   - `NativeViewRuntime.generation` is initialized and exposed, but the inspected cleanup path invalidates `alive` rather than incrementing generation.
   - It remains unclear whether any alternate runtime recreation path increments or replaces the generation in production.

4. **Path/style metadata cleanup**
   - `path_nodes`, `path_keys`, `style_atoms`, and `styles` are range-bounded but do not show ordinary eviction.
   - It is unclear whether environment teardown is the only cleanup boundary intended for these maps.

5. **Superseded desired revision bound**
   - The TS attachment/root ledgers retain revision entries until visible acknowledgment.
   - No fixed cap is visible.
   - The practical memory bound depends on host/backend progress.

6. **Finalization timing**
   - TypeScript `FinalizationRegistry` cleanup is best-effort and GC-scheduled.
   - No tests or runtime instrumentation establish latency between wrapper reachability loss and registry retirement.

7. **RSS accounting**
   - There is no direct RSS measurement in the inspected implementation.
   - `string_bytes` is explicitly `null`.
   - No JS heap snapshot, Rust allocator accounting, page allocator accounting, or N-API object-size accounting is integrated.

8. **Native `View` graph memory**
   - The slot table counts and weak/strong status are visible.
   - The byte size of retained Rust `View` graphs, strings, spans, grids, and child sequences is not.

9. **Content source lifetime coupling**
   - Content ports/connectors have their own disposal/status protocol.
   - A full source-to-connector-to-attachment teardown trace should be checked alongside the structural boundary trace to establish whether all connector finalization paths are host-frame synchronized.

10. **Cross-realm/global symbol behavior**
    - Framework counters and registry instances are held via `Symbol.for` globals.
    - The intended behavior across multiple JS realms or addon reloads was not verified.

11. **Uncommitted builder/edit transaction timeout**
    - Host disposal clears them.
    - No ordinary timeout/age-based cleanup was found in the inspected native runtime.

12. **No live execution validation**
    - Counts, stale recovery, page frees, finalizer timing, and actual RSS behavior remain source-inferred.

---

## 10. Evidence appendix

### 10.1 Primary inspected source paths and symbols

#### TypeScript framework/resource ownership

- `packages/iyon-tui/src/api/controls/framework-handle.ts`
  - `HandleId`
  - `FrameworkHandle`
  - `dispose()`
  - `nativeAs()`
- `packages/iyon-tui/src/runtime/handle-registry.ts`
  - `registerFrameworkHandle`
  - global handle counter
  - `releaseFrameworkHandle`
  - `disposeFrameworkResource`
- `packages/iyon-tui/src/transport/native/resource-registry.ts`
  - `ResourceRecord`
  - `PreparedResourceLease`
  - `NativeResourceRegistry`
  - `register`
  - `prepareResolve`
  - `beginDisposal`
  - `release`
  - `cancelDisposal`
  - `invalidateHost`
  - `maybeFinalize`
  - `retireRecord`
  - `finalizeUnowned`
- `packages/iyon-tui/src/transport/native/resources.ts`
  - `nativeResources`
  - `registerNativeResource`
  - `nativeResourceForHandleId`
  - `nativeResourceOf`
  - `releaseNativeResource`
  - `disposeNativeResource`

#### TypeScript attachments and semantic liveness

- `packages/iyon-tui/src/runtime/attachments.ts`
  - `AttachmentBindingState`
  - `PreparedAttachmentSet`
  - `prepareAttachmentsForView`
  - `prepareSemanticAttachments`
  - `validateSemanticAttachments`
  - `PreparedAttachmentSetImpl`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - `createSemanticViewNode`
  - `semanticAttachmentPresence`
  - `semanticAttachmentReferences`
  - `semanticNodeHasAttachments`
  - `retainSemanticAttachmentReference`
  - `copySemanticAttachmentReferences`
  - `setSemanticAttachmentPresence`
- `packages/iyon-tui/src/api/view/view.ts`
  - attachment-bearing view constructors/derivations
  - `attachStateDirect`
  - `attachStateForComposition`
  - semantic derivation attachment propagation

#### TypeScript retained structural transport

- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `SemanticNativeHint`
  - `SEMANTIC_NATIVE`
  - `STYLE_REF_CACHE`
  - scratch buffers
  - `MaterializeTx`
  - `RetainedRefusalError`
  - `RetainedCycleError`
  - `recoverStaleNode`
  - `materializeWithRecovery`
  - `ensureNative`
  - `ensureSemanticNative`
  - `acquireKnownRoot`
  - `RetainedRootBoundary`
  - `RootPublication`
  - `prepareInstall`
  - `prepareDesiredInstall`
  - `commitVisible`
  - `close`
  - `resetStyleRefCacheForThemeChange`
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `NativeViewAbiSession`
  - `nativeViewAbiSession`
  - `nativeViewRefForNodeId`
  - `tryRetainedMaterializeRef`
  - `tryRetainedAxisCreate`
  - `tryRetainedAxisSetChildRender`
  - `tryRetainedAxisSpliceRender`
  - `tryRetainedGridSetCellRender`
  - `tryRetainedEditTransactionRender`
  - `releaseNativeViewRef`
- `packages/iyon-tui/src/api/controls/view-slot.ts`
  - root boundary ownership
  - attachment binding replacement/disposal
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - root boundary ownership
  - attachment binding replacement/disposal

#### TypeScript content snapshots and source generations

- `packages/iyon-tui/src/api/content/retained.ts`
  - `TextSourceSnapshot`
  - `TextSourceStats`
  - `NativeSourceSnapshot`
  - `sourceSnapshot`
  - `sourceStats`
  - source generation/content generation/revision façade
  - `TextStreamSource`
  - `TextBlockSource`
  - `ContentPort`
  - `ContentConnector`
  - connector disposal and status synchronization

#### Native N-API retained runtime

- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - constants and status values
  - `NativeViewSlot`
  - `NativeRefPage`
  - `NativeRefTable`
  - `NativeViewRuntime`
  - `NativeViewRuntime::new`
  - `diagnostic_counts`
  - `acquire_lease`
  - `ensure_lease`
  - `resolve_ref`
  - `consult_semantic_identity`
  - `install_semantic_view`
  - `publish_semantic_view`
  - `publish`
  - `publish_bulk`
  - `ref_for_node_id`
  - `maintain_bounded`
  - `maintain`
  - `prune_expired`
  - `release_one_lease`
  - `release_many`
  - environment runtime registry
  - `runtime_handle_for_env`
  - `NativeViewAbiSession`
  - `runtime_from_handle`
  - `abort_all_edit_txns`
  - `tui_view_abi_maintain`
  - `tui_view_runtime_memory_snapshot`
  - `view_state_attach_impl`
  - `view_content_host_create_impl`
  - `view_release_many_impl`
  - embedded lifecycle/stale-ref tests

- `crates/iyon-tui-native/src/tui.rs`
  - `NativeTuiHost`
  - host disposal
  - environment runtime association
  - `abort_all_edit_txns`

#### Rust retained state and snapshots

- `crates/iyon-tui/src/retained_state/registry.rs`
  - `ViewStateRegistry`
  - `PreparedStateCommit`
  - state identity allocation
  - `mutate_record`
  - `capture_candidate`
  - `set_desired`
  - `prepare_visible`
  - `prepare_candidate`
  - `commit_visible_prepared`
  - `commit_prepared`
  - in-flight transitions
  - `dispose`
  - `remove`
  - `clear_bindings`
  - `dispose_all`
- `crates/iyon-tui/src/retained_state/capture.rs`
  - `StateCandidateOverlay`
  - `StateFrameView`
- `crates/iyon-tui/src/retained_state/presentation.rs`
  - `ViewStateSnapshot`
  - geometry/presentation revision separation
- `crates/iyon-tui/src/retained_state/record.rs`
  - mutable record snapshot creation
- `crates/iyon-tui/src/retained_state/occurrence.rs`
  - occurrence-local state application

### 10.2 Native line-reference groups

The most consequential source ranges are:

- `crates/iyon-tui-native/src/tui/view_abi.rs:125-180`
  - slot and paged ref-table representation
- `view_abi.rs:358-441`
  - runtime maps, counters, generation initialization
- `view_abi.rs:973-1024`
  - lease acquisition and weak/stale resolution
- `view_abi.rs:1027-1164`
  - semantic identity consultation and publication
- `view_abi.rs:1171-1273`
  - bounded maintenance and full prune
- `view_abi.rs:1275-1324`
  - lease release and batch release
- `view_abi.rs:1327-1453`
  - environment runtime ownership/liveness
- `view_abi.rs:1459-1468`
  - host transaction cleanup
- `view_abi.rs:1470-1663`
  - maintenance/bootstrap/memory diagnostics
- `view_abi.rs:1780-1832`
  - state attachment replacement
- `view_abi.rs:4757-5191`
  - lifecycle, stale-ref, lease, and cleanup tests

### 10.3 TypeScript line-reference groups

- `packages/iyon-tui/src/transport/native/resource-registry.ts:34-68`
  - resource record and finalizer state
- `resource-registry.ts:74-162`
  - prepared/desired/visible lease object
- `resource-registry.ts:168-233`
  - registry identity and registration
- `resource-registry.ts:251-338`
  - resource resolution and attachment prepare
- `resource-registry.ts:340-460`
  - disposal, finalization, retirement
- `packages/iyon-tui/src/runtime/attachments.ts:55-152`
  - desired/visible/superseded attachment ledger
- `attachments.ts:159-255`
  - semantic attachment traversal and duplicate/cycle checks
- `packages/iyon-tui/src/api/view/semantic-node.ts:286-336`
  - semantic identity and attachment strong-reference sidecars
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:52-149`
  - hints, scratch caches, counters
- `retained-dag.ts:208-340`
  - transaction-local refs and release behavior
- `retained-dag.ts:415-467`
  - stale-ref retry
- `retained-dag.ts:1148-1247`
  - identity-first resolution and NodeId promotion
- `retained-dag.ts:1488-1529`
  - root-lease protocol
- `retained-dag.ts:1572-1813`
  - root boundary state, desired/visible revisions, prepare APIs
- `retained-dag.ts:1801-2124`
  - prepare, commit, visibility, close, release
- `packages/iyon-tui/src/api/content/retained.ts:69-102`
  - content snapshot contract
- `retained.ts:241-315`
  - native-to-TS snapshot/stat normalization
- `retained.ts:336-395`
  - source lifecycle and mutations
- `retained.ts:617-764`
  - content port/connector disposal/status

### 10.4 Inspected-file manifest

#### Contract and context

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`

#### TypeScript

- `packages/iyon-tui/src/api/controls/framework-handle.ts`
- `packages/iyon-tui/src/runtime/handle-registry.ts`
- `packages/iyon-tui/src/transport/native/resource-registry.ts`
- `packages/iyon-tui/src/transport/native/resources.ts`
- `packages/iyon-tui/src/runtime/attachments.ts`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
- `packages/iyon-tui/src/api/view/view.ts`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
- `packages/iyon-tui/src/api/controls/view-slot.ts`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
- `packages/iyon-tui/src/api/content/retained.ts`

#### Native/Rust

- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
- `crates/iyon-tui/src/retained_state/registry.rs`
- `crates/iyon-tui/src/retained_state/capture.rs`
- `crates/iyon-tui/src/retained_state/presentation.rs`
- `crates/iyon-tui/src/retained_state/record.rs`
- `crates/iyon-tui/src/retained_state/occurrence.rs`

### 10.5 Files indexed but not comprehensively read

The repository-wide source manifest and directory trees were indexed to identify cross-plane consumers. The following were not read line-by-line because they were outside the lifetime-focused source paths:

- most Rust presentation/layout/paint modules;
- most Rust history, stream, interaction, backend, and terminal modules;
- most TypeScript public API and composition modules;
- generated ABI bodies beyond the portions needed to establish routing;
- bulk test fixtures, replay traces, and generated schema tables;
- unrelated native handwritten files.

Claims in this report about those areas are limited to reverse references and the explicitly inspected lifetime seams.

### 10.6 LOC methodology

- Generated source is separated from handwritten implementation.
- Approximate counts are based on source extents and inspected line-number ranges, rounded to avoid presenting inferred counts as exact.
- Embedded tests are counted separately where recognizable.
- No build-generated expansion, macro expansion, or dependency LOC is included.
- These are physical source-size estimates, not executable-code-size or binary-size measurements.

### 10.7 Validation status

No tests, benchmarks, builds, or runtime probes were run in this investigation. Existing test names and assertions were used as source evidence only.