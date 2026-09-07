# 29 — Three-plane runtime, transport, native addon, and retained-runtime wiring

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `29 — wiring/three-planes`
- Required output artifact:  
  `/Users/alxknt/.pi/agent/sessions/--Users-alxknt-github-iyon-n-iyon-tui--/subagent-artifacts/outputs/b363950e-0ff9-4622-a8f0-491385679af2/wiring/29-three-planes.md`

The source tree was inspected read-only. No source, configuration, generated file, or documentation was edited. No dependencies were installed, no services were started, and no test or build command was run.

### Scope

Primary scope:

1. TypeScript runtime:
   - `packages/iyon-tui/src/runtime/`
   - retained execution/composition protocol where it determines runtime ownership and transaction behavior.
2. TypeScript transport:
   - structural retained-DAG transport and ABI wrappers;
   - state envelope transport;
   - content control and direct data transport;
   - native resource/attachment ownership.
3. Native addon:
   - `crates/iyon-tui-native/src/`;
   - N-API wrappers and direct content FFI;
   - environment/runtime registries.
4. Rust retained runtime:
   - Rust application host/environment frame barriers;
   - retained-state registry, snapshots, candidate overlays, and state-binding lifecycle;
   - the native structural View runtime and its NodeId/NativeRef/cache/lease tables.

This report is about current source wiring only. It does not perform a V5 census or make V5 disposition decisions.

### Three distinct forms of retention

The source clearly separates three notions that must not be conflated:

| Retention form | Current owner | What it retains | What it does not own |
|---|---|---|---|
| Fiber-like composition retention | TypeScript `RetainedExecutionRuntime`, `RetainedExecutionScope`, `ChildOwnerState`, `OwnedBuilderRoot` | Component scope identity, child ownership, keyed namespaces, props, semantic output, tracked-state subscriptions, pending/committed WIP | Native View leases, Rust host frame state, native content/source state |
| TypeScript accepted-native knowledge | TypeScript `SEMANTIC_NATIVE` hints, `RetainedRootBoundary`, `AttachmentBindingState`, resource registry, ABI session/path caches | Knowledge that a semantic NodeId or attachment was accepted by a particular native runtime generation; weak acceleration metadata and desired/visible JS leases | Authoritative native View object graph or Rust frame state |
| Rust retained runtime state | Native `NativeViewRuntime`, Rust `TuiHost`/`HostInner`, `ViewStateRegistry`, content registries, scene/frame/backend state | Strongly leased native Views, weak semantic cache, NodeId→NativeRef associations, host desired/visible/candidate epochs, state versions, content projections, backend frame state | TypeScript component scopes and JS producer identity |

The most important architectural observation is that these are coupled at transaction boundaries but are not one shared retention table.

### Evidence status

Evidence is static source inspection. Exact source paths and symbols are listed in §10. The report includes line references based on the inspected source output. No runtime behavior is claimed as observed by execution. Existing tests are cited as behavioral evidence encoded in source, not as tests run during this investigation.

---

## 1. Responsibility and structure

### 1.1 TypeScript runtime inventory

| Path | Approximate physical extent | Primary responsibility | Secondary responsibility | Plane |
|---|---:|---|---|---|
| `packages/iyon-tui/src/runtime/runtime.ts` | ~928 lines | `Tui` lifecycle, root rendering, frame flush, host factories, resize/close/exit | History ownership, state/content/slot/pane factory wiring, theme, event routing | Mixed runtime/host |
| `packages/iyon-tui/src/runtime/wake-broker.ts` | ~551 lines | One environment-level edge-triggered host drain broker | Fairness, microtask scheduling, error delivery, retry-block handling, counters/traces | Scheduling/host |
| `packages/iyon-tui/src/runtime/attachments.ts` | ~313 lines | Semantic attachment traversal and desired/visible attachment leases | Duplicate detection, host/environment/kind validation, WIP attachment commit | State/content/structural |
| `packages/iyon-tui/src/runtime/native-resource-registry.ts` | small re-export | Runtime-level export of resource registry | None | Runtime seam |
| `packages/iyon-tui/src/runtime/environment.ts` | ~46 lines | Realm-wide runtime environment singleton | Resource registry and wake broker construction | Runtime |
| `packages/iyon-tui/src/runtime/error-channel.ts` | indexed in manifest; not fully read | Runtime frame error channel | Error reporting/consumption | Runtime |
| `packages/iyon-tui/src/runtime/access.ts` | indexed in manifest; not fully read | Internal runtime access registration | Testing/runtime access | Runtime |
| `packages/iyon-tui/src/runtime/events.ts` | indexed in manifest; not fully read | Event contract | None | Runtime |
| `packages/iyon-tui/src/runtime/handle-registry.ts` | indexed in manifest; not fully read | Handle tracking | None | Runtime |

`runtime.ts` constructs a single `RetainedExecutionRuntime` eagerly per `Tui` instance (lines 120–196), registers the native host with the realm-wide wake broker (lines 147–157), and installs an attachment context containing the realm environment token, host token, and resource registry (lines 153–157).

### 1.2 TypeScript composition/runtime retention

| Path | Approximate physical extent | Responsibility |
|---|---:|---|
| `packages/iyon-tui/src/composition/execution.ts` | ~1,254 lines | Execution scopes, WIP evaluation, child reconciliation, tracked-state invalidation, prepare/commit/abort, builder roots |
| `packages/iyon-tui/src/composition/child-owner.ts` | ~92 lines | Positional unkeyed children and keyed identity namespaces |
| `packages/iyon-tui/src/composition/execution-context.ts` | indexed; not fully read | Active execution scope and protocol context |
| `packages/iyon-tui/src/composition/publication.ts` | ~38 lines | Structural prepare/commit/abort interfaces |
| `packages/iyon-tui/src/composition/tracked-state.ts` | ~136 lines | `State<T>` source and scope subscriptions |
| `packages/iyon-tui/src/composition/define-view.ts`, `compose.ts`, `execution-context.ts`, `persistent-seq.ts` | indexed; not fully read | Public/component composition helpers and persistent sequence support |

The composition layer’s central object is `RetainedExecutionScope` (`execution.ts:109`). It stores:

- stable scope ID;
- parent/depth/ordinal/key/type;
- current and pending props;
- current and pending semantic `View` output;
- current/pending children;
- semantic slots;
- committed and pending tracked-state dependencies;
- publication target/projection;
- dirty/mounted/disposed state.

`ChildOwnerState` explicitly distinguishes:

- a positional unkeyed stream;
- lazily allocated keyed groups;
- current WIP participation;
- committed children/groups.

`KeyGroup` (`child-owner.ts:85`) is deliberately only an identity namespace. It has no dependency set, dirty flag, scheduler, or output. The executable component scope is mounted inside the group.

### 1.3 TypeScript transport inventory

#### Structural transport

| Path | Approximate physical extent | Responsibility |
|---|---:|---|
| `transport/structural/native-view-abi.ts` | ~629 lines | Native ABI session, NativeRef acquisition/release, small/wide axis transport, path/edit transaction wrappers |
| `transport/structural/retained-dag.ts` | ~2,065+ lines | Semantic DAG→NativeRef materialization, identity hints, direct constructors, leases, root boundaries, stale recovery |
| `transport/structural/encoding.ts` | indexed; not fully read | Numeric wire packing |
| `transport/structural/ir.ts` | ~130 lines | Shared kind and payload type declarations |
| `transport/structural/policy.ts` | indexed; not fully read | Transport limits |
| `transport/structural/retained-path.ts` | indexed; not fully read | Immutable path-lineage representation |
| `transport/structural/component-id.ts` | indexed; not fully read | Component identity packing |
| `transport/structural/style-lowering.ts` | indexed; not fully read | Theme/style lowering |

`retained-dag.ts` explicitly describes the semantic node DAG as the declaration and the NativeRef correspondence as the physical retained representation. It also states that retained materialization is the single production structural architecture and that refusal is explicit rather than a selector for a legacy transport (`retained-dag.ts:1–22`).

#### Generated structural ABI

| Path | Approximate physical extent | Responsibility |
|---|---:|---|
| `transport/abi/structural/generated/view_abi.ts` | ~80 lines | Opaque N-API `NativeViewAbiHandle` contract |
| `transport/abi/structural/generated/view_calls.ts` | ~327 lines | Checked TypeScript wrappers over NativeRef, constructors, patch, path, edit transaction, style, and release calls |
| `transport/abi/structural/generated/view_abi_manifest.json` | generated metadata | ABI function/schema manifest |
| `transport/abi/structural/generated/view_abi_conformance.ts` | generated conformance code | ABI conformance |
| `transport/abi/structural/schema/view-kind-codes.json` | schema | Stable kind codes |

`view_calls.ts` contains the public-in-module transport functions for:

- `hostRenderRef`;
- `viewRefForNodeId`;
- `viewReleaseMany`;
- fixed-arity row/column constructors;
- buffer axis/grid/text/diff constructors;
- axis child replacement/splice;
- grid cell replacement;
- path interning and path-localized text/structural patches;
- edit transaction begin/add/commit/abort;
- style atom/style creation.

#### State transport

| Path | Approximate physical extent | Responsibility |
|---|---:|---|
| `transport/state/control.ts` | ~359 lines | Typed validation/normalization and envelope creation |
| `transport/state/generated/state_envelope.ts` | ~375 lines | Generated mask/lane encoding |
| `api/view/retained-state.ts` | ~236 lines | Public `ViewState` API and native wrapper mutation calls |

#### Content transport

| Path | Approximate physical extent | Responsibility |
|---|---:|---|
| `transport/content/control.ts` | ~98 lines | N-API Source/Port/Connector control calls |
| `transport/content/ffi.ts` | ~785 lines | High-volume Source append/replace/clear/seal/truncate direct FFI, metadata handshake, annotation encoding |
| `transport/content/abi.ts` | indexed; not fully read | Content ABI constants/status names |
| `transport/native/addon.ts` | ~248 lines | Private native class and operation contracts |
| `transport/native/resource-registry.ts` | ~480 lines | Realm-level wrapper/resource identity and lease registry |
| `transport/native/resources.ts` | indexed; not fully read | Resource lookup helpers |
| `transport/native/factories.ts` | indexed; not fully read | Native source/control factories |
| `transport/native/artifact.ts` | indexed; not fully read | Native artifact identity/path resolution |

### 1.4 Native addon inventory

| Path | Approximate physical extent | Responsibility | Plane |
|---|---:|---|---|
| `crates/iyon-tui-native/src/tui.rs` | ~1,450 lines | N-API classes for host, history, text input, source, port, connector, slot/pane wrappers | Mixed |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | ~6,000+ lines | Native structural runtime, NativeRef slots, semantic cache, ABI implementation, path/build/edit transaction storage | Structural |
| `crates/iyon-tui-native/src/tui/view_state.rs` | ~950 lines | State envelope decoding and `NativeViewState` wrapper | State |
| `crates/iyon-tui-native/src/content_ffi.rs` | ~584 lines | Direct Source data ABI | Content |
| `crates/iyon-tui-native/src/generated/view_abi_exports.rs` | generated | Structural ABI dispatch/export layer | Structural |
| `crates/iyon-tui-native/src/generated/view_abi_napi.rs` | generated | N-API generated bindings | Structural |
| `crates/iyon-tui-native/src/generated/view_abi_table.rs` | generated | ABI table | Structural |
| `crates/iyon-tui-native/src/generated/view_abi_types.rs` | generated | ABI type definitions | Structural |
| `crates/iyon-tui-native/src/generated/view_state_schema.rs` | generated | State property IDs, offsets, masks, constants | State |
| `crates/iyon-tui-native/src/generated/view_abi_conformance.rs` | generated | ABI conformance functions | Structural |
| `crates/iyon-tui-native/src/lib.rs` | ~16 lines | Module/export root | Native |
| `crates/iyon-tui-native/src/sync.rs` | indexed; not fully read | Native package versioning | Native |
| `crates/iyon-tui-native/src/error.rs` | indexed; not fully read | Native error conversion | Native |

### 1.5 Rust retained runtime inventory

| Path | Approximate physical extent | Responsibility | Plane |
|---|---:|---|---|
| `crates/iyon-tui/src/application/host.rs` | ~2,900+ lines | Host desired/candidate/visible frame state, presentation receipt, state/content candidate commit, frame epochs | Host/backend/state/content |
| `crates/iyon-tui/src/application/environment.rs` | ~700+ lines | Environment host registry, fair pending queue, drain outcomes, retry blocking, wake epochs | Scheduling/host |
| `crates/iyon-tui/src/application/kernel.rs` | ~700+ lines | Scene/History/state/content attachment extraction and frame preparation ingress | Structural/state/content |
| `crates/iyon-tui/src/retained_state/registry.rs` | ~685 lines including tests | Host-owned state identity, mutable records, committed immutable snapshots, dirty set, desired/visible/in-flight bindings | State |
| `crates/iyon-tui/src/retained_state/record.rs` | ~194 lines | One mutable state record and revision/effect mutation semantics | State |
| `crates/iyon-tui/src/retained_state/capture.rs` | ~155 lines | Candidate overlay and frame-time state reads | State |
| `crates/iyon-tui/src/retained_state/geometry.rs` | indexed; not fully read | Geometry override representation and effects | State |
| `crates/iyon-tui/src/retained_state/presentation.rs` | indexed; not fully read | Presentation override representation and snapshots | State |
| `crates/iyon-tui/src/retained_state/capabilities.rs` | indexed; not fully read | Node-kind capability checks | State |
| `crates/iyon-tui/src/retained_state/effects.rs` | indexed; not fully read | Effect classification | State |
| `crates/iyon-tui/src/retained_state/damage.rs` | indexed; not fully read | State damage representation | State |
| `crates/iyon-tui/src/retained_state/occurrence.rs` | indexed; not fully read | Physical occurrence state attachment | State |

LOC methodology: counts above are approximate source-line extents from the inspected files and source line references, with tests and generated sections called out separately where visible. They are not claims from an executed `wc` command. Large files such as `view_abi.rs`, `host.rs`, and `ffi.ts` are intentionally represented by responsibility rather than every helper.

---

## 2. Types, APIs and contracts

### 2.1 `TuiRuntime` and `Tui`

`TuiRuntime` (`runtime.ts:59–91`) exposes generic framework operations:

- size and event waiting;
- direct scene or producer rendering;
- explicit flush;
- runtime error subscription;
- resize/close/exit;
- History, ViewState, ContentPort, TextInput, ViewSlot, and ScrollPane factories;
- generic key/output/paste routing;
- theme changes.

The public contract distinguishes direct scenes from retained producers:

- direct scene values “take over the root immediately”;
- producers own the retained root and remain subscribed to tracked state.

`Tui` owns:

- the native host object;
- current width/height;
- current scene;
- `RetainedRootBoundary`;
- one `RetainedExecutionRuntime`;
- one `RuntimeEnvironment` registration;
- one `AttachmentRuntimeContext`;
- History sideband state;
- owned handles;
- History liveness token.

This makes `Tui` the TypeScript owner of the runtime session, but not the owner of the Rust structural object graph itself. Native View objects remain in the environment-owned NativeViewRuntime and are held through NativeRef leases.

### 2.2 Composition publication contracts

`composition/publication.ts` defines the critical seam:

```ts
interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}

interface StructuralPublicationTarget {
  preparePublication(output: View): PreparedStructuralPublication | undefined;
  needsPublication?(output: View): boolean;
}

interface StructuralScopeProjection {
  readonly view: View;
  readonly target: StructuralPublicationTarget;
  dispose(): void;
}
```

Composition knows only that a semantic `View` can be prepared, committed, or aborted. It does not know NativeRefs, native host objects, ABI buffers, or resource leases.

The key consequence is that the composition transaction and native structural transaction are nested:

1. TypeScript composition evaluates semantic bodies and stages child scopes.
2. The publication target performs native structural preparation.
3. Only after every publication prepares successfully does composition commit.
4. Publication commit is called from the composition commit phase under `protocolState.internalPublication`.

### 2.3 `RetainedExecutionScope`

`RetainedExecutionScope` (`execution.ts:109–218`) has separate current and pending fields for:

- props;
- output;
- dependencies;
- children/keys;
- publication.

Its identity contract is explicit: scope identity is not NodeId identity and is not physical resource identity (`execution.ts:105–108`).

The `dispose()` path:

- marks the scope disposed;
- disposes its projection;
- clears publication/projection fields;
- unsubscribes all committed dependencies;
- releases semantic slots;
- recursively disposes unkeyed and nested keyed children;
- records an unmount.

### 2.4 `OwnedBuilderRoot`

`OwnedBuilderRoot` (`execution.ts:1185–1253`) is the producer-owned root used by `Tui.render(builder)` and control boundaries.

Its producer is part of the transaction:

- `replaceProducer()` optimistically installs the new producer;
- calls `runtime.update(scope)`;
- restores the old producer on any failure;
- cancels only a retry obligation introduced solely by the failed producer attempt;
- preserves any pre-existing state invalidation obligation.

This is a significant distinction from Fiber retention. The producer function itself is current TypeScript boundary state, but its accepted semantic output is published through the same structural prepare/commit protocol.

### 2.5 Structural ABI session

`native-view-abi.ts:57–70` defines an opaque `NativeViewAbiSession`:

- opaque native runtime handle;
- generated symbol surface;
- ABI name/version;
- semantic version;
- schema and generator hashes;
- NativeViewRuntime generation;
- transport kind;
- function count.

`nativeViewAbiSession()` (`native-view-abi.ts:107–128`) obtains the session once, validates metadata against the checked-in manifest, and runs a native no-op bootstrap probe. A mismatch throws immediately; it is not converted to a fallback route.

The opaque session object prevents a raw Rust runtime pointer from crossing TypeScript. The generated calls pass the opaque N-API class handle back to native.

### 2.6 Structural semantic and physical identity

Semantic nodes carry NodeIds in TypeScript. Native structural calls receive NodeId as two `u32` words:

- low 32 bits;
- high 32 bits.

The native runtime validates NodeId high bits and rejects zero (`view_abi.rs:1685–1690`).

NativeRefs are positive `u32` handles. The generated TypeScript wrappers check returned references through `checkedRef`. Ref ranges are partitioned in Rust:

- ordinary ViewRefs below `PATH_ROOT_REF`;
- path refs in a disjoint range;
- builder refs and edit transaction refs in high ranges;
- style atom and style refs in separate ranges.

The disjoint ranges prevent a path/builder/edit handle from being interpreted as a ViewRef.

### 2.7 State API and generated envelope contract

The public `ViewState` API (`api/view/retained-state.ts:145–214`) exposes:

- geometry set/clear;
- presentation set/clear;
- dynamic style state set/clear.

It does not embed state values in a semantic `View`. Instead, `.state()` places an opaque state handle identity into the semantic attachment record (`retained-state.ts:140–143`).

Geometry properties:

- `width`, `height`;
- `padding`;
- min/max width/height;
- `gap`;
- alignment;
- border edges.

Presentation properties:

- foreground/background/borderColor;
- border style/glyphs;
- text attributes;
- style.

The generated geometry envelope uses:

- 10 property bits;
- 14 numeric words;
- no strings;
- explicit `setMask`, `nullMask`, and `clearMask`.

The generated presentation envelope uses:

- 7 property bits;
- 5 numeric words;
- 14 string lanes;
- explicit set/null/clear masks.

`control.ts` validates public values before encoding. The generated encoder then checks lane-level representation. Rust `view_state.rs` checks the envelope header, lane lengths, masks, enum values, unknown bits, and nullable semantics before constructing canonical Rust patch types.

### 2.8 Content Source data ABI

The direct content ABI uses fixed scalar identity lanes:

```text
environment_slot
environment_generation
source_slot
source_generation
```

Payload mutation calls carry:

- UTF-8 bytes and byte length;
- fixed-size annotation records;
- annotation payload bytes and length;
- fixed output mutation-result buffer.

The output record has six `u32` lanes:

```text
source_revision_lo
source_revision_hi
environment_wake_epoch_lo
environment_wake_epoch_hi
flags
reserved0
```

The direct FFI is not a structural fallback path. It is specifically the high-volume content data lane (`content_ffi.rs:1–6`).

### 2.9 Native class contracts

`transport/native/addon.ts` intentionally exposes private framework contracts rather than public application/session classes.

Important contracts:

- `NativeTuiHostContract`: host lifecycle, desired ViewRef, epochs, pending-host drain, state/content factories, input/output, readback.
- `NativeViewStateContract`: state identity, kind validation, envelope mutations.
- `NativeTextSourceContract`: Source identity/generation, snapshot/stats, content generation.
- `NativeContentPortContract`: content identity, connect/deactivate/mounted.
- `NativeContentConnectorContract`: activate/deactivate/dispose/status.
- `NativeHistoryContract`: detached/attached state, push/freeze/discard.
- `NativeViewSlotContract` and `NativeScrollPaneContract`: direct NativeRef installation and animation/content control.

The native addon loader checks the artifact’s build identity against `nativeArtifact.packageBuildId` (`addon.ts:236–242`).

---

## 3. Dependency and ownership map

### 3.1 High-level dependency diagram

```text
TS public semantic View / Scene
          │
          │ semanticNodeOf(View)
          ▼
TS composition/runtime
  RetainedExecutionScope
  OwnedBuilderRoot
  ChildOwnerState
  State<T> subscriptions
          │
          │ StructuralPublicationTarget
          ▼
TS structural transport
  RetainedRootBoundary
  MaterializeTx
  ensureSemanticNative / ensureNative
  AttachmentBindingState
  NativeResourceRegistry
          │
          │ generated N-API ABI calls
          ▼
Native addon
  NativeViewAbiSession
  NativeViewRuntime
  NativeTuiHost
  NativeViewState / ContentPort / Source wrappers
          │
          ├── iyon_tui binding::View / WeakView
          ├── ViewStateRegistry / StateFrameView
          ├── ContentHostRegistry / Source registry
          └── TuiEnvironment / HostInner
                    │
                    ▼
              scene/layout/paint/backend
```

### 3.2 Ownership direction

#### TypeScript composition

```text
Tui
 ├── RetainedExecutionRuntime
 │    ├── root OwnedBuilderRoot
 │    └── control/builder child scopes
 ├── RetainedRootBoundary
 ├── AttachmentBindingState
 ├── RuntimeHostRegistration
 └── owned handle set
```

A component scope owns its child scopes. A keyed group owns only a child identity namespace. The scope runtime owns scheduling and dependency subscriptions.

#### TypeScript native-resource registry

```text
NativeResourceRegistry
 ├── ResourceRecord(handleId)
 │    ├── weak handle reference
 │    ├── weak native resource reference
 │    ├── owner environment/host
 │    ├── accepted node-kind set
 │    └── prepared/desired/visible lease counts
 └── PreparedResourceLease
      └── strong handle/resource keep-alive while bound
```

`NativeResourceRegistry` is realm-wide and plane-neutral. It does not understand state/content semantics; it validates kind, environment, host, and accepted node kinds.

#### Native structural runtime

```text
NativeViewRuntime
 ├── nodes: NodeId → WeakView
 ├── node_refs: NodeId → NativeRef
 ├── slots: paged NativeRef → NativeViewSlot
 ├── path_nodes/path_keys
 ├── builders
 ├── edit_txns
 ├── style_atoms/styles
 └── generation
```

`NativeViewSlot` contains:

- NodeId;
- `WeakView`;
- optional strong leased `View`;
- JS lease count;
- kind tag.

A NativeRef table slot can therefore outlive a strong JS/native View lease while retaining weak semantic identity metadata.

#### Rust host

```text
TuiHost / HostInner
 ├── HostRunning / SceneHost
 ├── backend
 ├── visible frame
 ├── candidate frame
 ├── presentation receipt
 ├── ViewStateRegistry
 ├── ContentHostRegistry
 ├── desired/visible/candidate epochs
 └── environment registration
```

The host owns the authoritative visible frame. A candidate frame is not visible until backend presentation succeeds.

### 3.3 Creation/destruction paths

#### Tui and host

1. `Tui.open()` validates size and loads `NativeTuiHost`.
2. Native `NativeTuiHost::new()` obtains or creates a `TuiEnvironment`, registers the content environment, creates `TuiHost`, obtains the environment-local `NativeViewRuntime`, and registers the host.
3. Native host prepares and presents a bootstrap frame.
4. TypeScript `Tui` registers the host with the JavaScript `EnvironmentWakeBroker`.
5. `Tui.close()`:
   - unregisters host scheduling;
   - cascades native content-resource disposal;
   - disposes attachment bindings;
   - clears native ViewState bindings;
   - invalidates TypeScript host-owned resources;
   - disposes owned handles and retained execution;
   - closes the structural root boundary;
   - disposes the native host.
6. Native `HostInner::drop()` disposes content memberships and unregisters the native host from the environment.

#### Structural View

1. A semantic TypeScript View is validated through `semanticNodeOf`.
2. `RetainedRootBoundary.prepare...` creates a `MaterializeTx`.
3. `ensureSemanticNative()` resolves a same-generation hint, transaction-local ref, pre-existing NodeId ref, derivation, or direct materialization.
4. Native constructors publish semantic View and return a leased NativeRef.
5. The boundary keeps the candidate root lease and releases all temporary non-root leases.
6. On abort, all acquired temporary refs release and the old root remains leased.
7. On close, desired/visible root leases are released exactly once.

#### Attachments

1. Semantic attachment traversal discovers state/content handle IDs.
2. The resource registry prepares one lease per attachment.
3. Duplicate attachment use in one semantic candidate is rejected.
4. Desired binding promotion changes prepared leases to desired leases.
5. Visibility promotion changes desired leases to visible leases.
6. On replacement, old desired/visible leases are released only at the appropriate revision boundary.

---

## 4. Execution paths and state transitions

### 4.1 Canonical builder render path

The canonical recurring path is `Tui.render(builder)` (`runtime.ts:451–497`).

#### Initial render

```text
Tui.render(builder)
  → renderCanonical
  → producer closure:
       Scene.from(builder())
       stageHistoryBinding(scene.history)
       return scene.body
  → OwnedBuilderRoot.start
  → RetainedExecutionRuntime.mountExistingRoot
  → runWork(root)
  → evaluateIntoPendings(root)
  → semanticNodeOf(output)
  → stagePublicationsRecursive(root)
  → rootTarget.preparePublication(output)
  → prepareRootPublication
  → RetainedRootBoundary.prepareDesiredInstall
  → MaterializeTx / ensureSemanticNative
  → RootPublication.commit
  → desired structural root accepted
  → hostRegistration.markPending
  → Tui.flush
  → RetainedExecutionRuntime.flush
  → hostRegistration.flush
  → EnvironmentWakeBroker.flush
  → NativeTuiHost.flushPendingHosts
  → HostInner frame candidate/receipt/commit
  → broker onCommitted
  → RetainedRootBoundary.commitVisible
  → AttachmentBindingState.commitVisible
```

`Tui` deliberately does not make the root boundary live until initial materialization and publication succeed (`runtime.ts:469–481`). If initial evaluation or preparation fails, the failed root is disposed and the next render remains retryable.

#### Recurring builder replacement

A later `render(builder)` replaces only the producer:

- `OwnedBuilderRoot.replaceProducer()` stores the previous producer;
- calls `runtime.update(scope)`;
- the runtime invalidates and synchronously flushes the root;
- if evaluation or preparation fails, the old producer is restored;
- if frame presentation fails after desired structural acceptance, the desired revision remains retryable and is not rolled back by the TypeScript producer path.

This creates an intentional separation between:

1. TypeScript producer/evaluation success;
2. native desired structural acceptance;
3. host visible-frame success.

### 4.2 Direct scene path

`Tui.render(scene)` (`runtime.ts:499–549`) bypasses retained builder execution.

The direct path:

1. normalizes `Scene.from(scene)`;
2. validates the semantic body;
3. validates History ownership/resource liveness;
4. detects same-body/same-History identity no-op;
5. otherwise stages the effective History sideband;
6. prepares a root publication through the same `RetainedRootBoundary`;
7. commits it;
8. disposes any previous builder root;
9. flushes the host barrier.

The direct path and builder path converge at the retained structural root boundary, but the builder path additionally retains component scope identity and tracked-state subscriptions.

### 4.3 Composition evaluation and commit

`RetainedExecutionRuntime.flush()` (`execution.ts:440–547`) is a three-phase protocol.

#### Phase 1: evaluate

- queued dirty scopes are acquired;
- dirty obligations are captured for retry;
- scopes are sorted parent-before-child;
- each still-live dirty scope runs synchronously;
- promise-like bodies throw `TUI_EXECUTION_ASYNC_BODY`;
- outputs are validated through `semanticNodeOf`;
- pending child/dependency/output state is populated.

A parent can evaluate child scopes inline. A queued child can also appear in the same batch; the commit set prevents duplicate commit.

#### Phase 2: prepare

- every processed scope recursively stages child publications;
- every publication target can refuse;
- any refusal unwinds every staged publication;
- all WIP child ownership, semantic slots, dependencies, and sideband state are rolled back;
- the original dirty obligations are restored.

#### Phase 3: commit

- descendants commit before parents;
- prepared publication `commit()` callbacks execute under `protocolState.internalPublication`;
- output/props/dependencies/semantic slots become current;
- removed child scopes are deferred until the entire batch promotion completes;
- a commit-phase throw is recorded in `pathologicalCommitFailures` and is treated as a pathological invariant/teardown failure rather than an ordinary recoverable prepare failure.

### 4.4 State<T> invalidation path

`State<T>` (`tracked-state.ts:41–92`) is intentionally minimal:

- reading `.value` while a scope is evaluating links the source to the scope;
- reading outside evaluation is untracked;
- a changed write uses `Object.is` and invalidates subscribed scopes;
- writes during component evaluation throw `TUI_EXECUTION_STATE_WRITE_DURING_EVALUATION`;
- dependency subscription changes become authoritative only after the next successful scope commit;
- an aborted evaluation preserves the old dependency set.

The path is:

```text
State<T>.set/update
  → StateSource.currentValue changes
  → publish subscribers
  → RetainedExecutionRuntime.invalidateFromState(scope)
  → scope.dirty = true
  → runtime queue
  → microtask auto-flush or explicit Tui.flush
  → composition evaluate/prepare/commit
  → semantic output publication
  → host frame barrier
```

This does not directly mutate Rust retained state. It invalidates TypeScript producer scopes, which may create a new semantic View and then cross the structural transport.

### 4.5 Structural materialization path

`ensureSemanticNative()` (`retained-dag.ts:1148–1220`) has hard ordering:

1. same-generation `SEMANTIC_NATIVE` hint;
2. transaction-local `tx.refs`;
3. NodeId→NativeRef promotion if `node.id <= nativeLookupCeiling`;
4. cycle detection;
5. derivation fast path;
6. direct semantic payload inspection/materialization;
7. transaction temporary lease recording.

A NodeId above `nativeLookupCeiling` is treated as genuinely new and skips an extra native lookup. A prior NodeId below the ceiling may be probed because JS-side hint metadata can be absent while native cache state still exists.

The direct materializer is children-first for structural nodes:

- row/column child refs are resolved before constructor invocation;
- grid cells are resolved before grid construction;
- hanging/container/clamp/decorated children are resolved first;
- text styles are resolved before text constructor calls;
- state/content attachment identities are validated before native publication.

### 4.6 Native structural publication

Native `NativeViewRuntime` (`view_abi.rs:358–441`) owns the structural caches and tables.

A fresh constructor routes through `publish_semantic_view()` (`view_abi.rs:1089–1128`), which centralizes:

- NodeId validation;
- identity conflict detection;
- `NodeId → WeakView`;
- `NodeId → NativeRef`;
- NativeRef slot allocation;
- lease mode;
- semantic cache diagnostics.

`PublicationLease::Leased` is used for normal constructors. `PublicationLease::Weak` is used for bulk/path intermediate publications where a strong JS backing is not required.

The native runtime is owner-thread constrained:

- magic/ABI/semantic version must match;
- alive must be set;
- current thread must equal the runtime owner thread (`view_abi.rs:444–450`);
- invalid runtime calls return failure status.

### 4.7 Structural exact-root path

`renderExactRoot()` (`retained-dag.ts:1381–1463`) is the TypeScript accepted-native knowledge fast path:

- requires a warm generation-valid semantic hint;
- calls exactly one `hostRenderRef`;
- does not inspect semantic payload or descendants;
- on `HOST_STATUS_CACHE_MISS`, deletes the hint, promotes by NodeId once, and retries;
- successful recovery transfers the recovered lease to the owning root boundary;
- a second cache miss returns `no_root_ref`;
- other statuses throw explicitly.

This path does not mean TypeScript owns the native tree. It means TypeScript has a generation-valid hint that native can verify by attempting to render a NativeRef.

### 4.8 State mutation path across all planes

```text
public ViewState.setGeometry/setPresentation/setStyleState
  → TS public validation
  → generated state envelope
  → NativeViewState N-API method
  → native envelope decoder
  → HostViewState / ViewStateRegistry::mutate_record
  → mutable record update
  → revision increment and StateEffects
  → HostInner::invalidate_state
  → HostRunning invalidation
  → HostInner::mark_pending
  → TuiEnvironment pending host queue
  → candidate state capture:
       ViewStateRegistry::capture_candidate
       StateCandidateOverlay
       StateFrameView
  → scene/layout/paint preparation reads immutable candidate snapshot
  → ViewStateRegistry::prepare_candidate
       validate visible target IDs/kinds
       prepare visible transitions
       pin in-flight IDs
  → backend frame presentation
  → receipt success:
       commit_prepared
       visible/in-flight transitions
       candidate overlay becomes frame authority
  → receipt failure:
       clear in-flight candidate bindings
       old visible state/frame stays authoritative
```

There are two state-related ledgers:

1. TypeScript `AttachmentBindingState`, which keeps JS/native wrappers alive across desired/visible revisions.
2. Rust `ViewStateRegistry`, which stores state values, revisions, desired/visible/in-flight membership, and immutable frame snapshots.

They are synchronized by semantic attachment identity and host frame revisions, but they are not one shared data structure.

### 4.9 Content Source append path

The high-volume path is:

```text
TS ContentPort/Source API
  → transport/content/ffi.ts
  → encode UTF-8 bytes
  → encode annotation records/payload
  → validate ABI metadata/session
  → direct symbol call:
       iyon_tui_source_append_utf8_v1
       or replace/clear/seal/head_truncate
  → native content_ffi.rs
  → identity lookup:
       environment slot/generation
       source slot/generation
  → HostContentSource append/replace/clear/seal/truncate
  → ContentMutationResult
       source revision
       environment wake epoch
       drain flag
  → TS finishMutation
  → requestWake when drain flag is set
  → EnvironmentWakeBroker
  → affected host drain
  → content projection candidate
  → scene/layout/paint
  → backend presentation
```

The direct FFI retains no input pointer after return (`content_ffi.rs:167–180`). JS TypedArray buffers are synchronously borrowed for the call; Rust copies/owns resulting content according to the Source implementation.

### 4.10 Host frame state transition

`HostInner` stores separate:

- `frame`: last complete logical frame;
- `candidate_frame`: candidate not yet visible;
- `presentation`: optional asynchronous backend receipt;
- `frame_pending`;
- candidate epoch, structural revision, content dirty epoch;
- candidate state/content commit plans;
- failed attempt metadata;
- physical synchronization marker.

The transition is:

```text
desired acceptance
  → pending_epoch increments
  → render()
      if presentation outstanding:
          return waiting_for_presentation
      capture state/content candidate
      prepare scene/frame
      prepare state commit
      prepare content commit
      store candidate + plans
      submit backend frame
          synchronous headless:
              commit_frame
          real terminal:
              retain receipt, report waiting
      receipt poll
          success:
              commit_frame
          failure:
              discard candidate and plans
              preserve visible frame
              mark physical sync unknown where required
```

`commit_frame()` (`host.rs:2026–2114`) promotes:

- state visible/in-flight bindings;
- content candidate;
- scene frame;
- visible structural revision;
- visible frame revision;
- committed epoch.

If newer desired work was accepted while the candidate was in flight, the committed epoch is only the candidate epoch; the newer pending epoch remains outstanding.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Production path | Selection condition | Failure behavior |
|---|---|---|---|
| Initial builder render | `OwnedBuilderRoot.start` → retained root boundary | Callable producer | Evaluation, attachment, structural preparation, or native failure rejects initial mount; root remains absent/retryable |
| Re-render builder | `OwnedBuilderRoot.replaceProducer` | New producer identity | Producer restored on evaluation/prepare failure; previous producer remains authoritative |
| Direct scene render | `Tui.render(scene)` → root boundary | Non-function scene | Native structural refusal is explicit; no previous transport selection |
| Reuse an already accepted root | `renderExactRoot`/NativeRef hint | Same-generation semantic hint | One cache-miss recovery via NodeId promotion; otherwise explicit no-root/failure |
| New semantic node | `ensureSemanticNative` direct materializer | No valid hint/local ref/promotion | Direct constructor status converted to retained refusal; transaction releases temporary leases |
| Wide axis creation | fixed arity for 0–4, builder for larger counts | Child count | Builder invalid/status failure aborts builder and releases child refs |
| Axis child replacement | `viewAxisSetChild` | Native retained base and valid child index | Native status causes explicit retained failure; child/next refs released |
| Axis splice | `viewAxisSpliceBuffer` | Valid base/index/remove count | Status failure releases next and every acquired child lease |
| Grid cell replacement | `viewGridSetCell` | Valid native base and row/column | Status failure releases child/next refs |
| Text layout localized patch | path ABI or edit transaction | Explicit specialized caller path | Invalid path/depth/status returns undefined to caller; no generic fallback selected |
| State mutation | State envelope → native `HostViewState` | Valid public patch | TS and Rust validation precede record mutation; malformed input leaves state unchanged |
| Source append/replace | direct content FFI | Content Source data operation | Status returned; accepted Source mutation can still produce a later host wake failure |
| Content control connect/activate | N-API control wrapper | Valid Source/Port/Funnel | Native content errors surfaced as content errors; no structural route involved |
| Host desired ViewRef | `NativeTuiHost.setDesiredViewRef` | A valid NativeRef resolves in the structural runtime | Host/content/state target validation occurs before desired root acceptance |
| Host frame drain | `EnvironmentWakeBroker` → `flushPendingHosts` | Pending host and fair-driver selection | Automatic drain stores errors; explicit barrier retries and throws pending errors |

### 5.2 No structural fallback route

The source repeatedly states that retained refusal is not a route selector:

- `RetainedRefusalError` is an explicit retained materialization refusal (`retained-dag.ts:185–197`);
- stale recovery allows one targeted retry (`retained-dag.ts:415–467`);
- `tryRetainedMaterializeRef()` returns undefined for retained refusal, and callers turn that into operation-specific failure (`native-view-abi.ts:151–187`);
- control code comments state there is no parallel native-view mutation route after retained preparation (`view-slot.ts` and `scroll-pane.ts` call sites);
- the retained root boundary’s `route` is always `"retained"`.

There are still structural ABI functions for path patches and edit transactions, but these are specialized retained operations, not a second complete-object decoding architecture.

### 5.3 Composition abort behavior

Evaluation/prepare abort:

- current output, child topology, dependency subscriptions, and publication target remain authoritative;
- pending child scopes are recursively rolled back;
- fresh never-committed scopes are disposed;
- dirty obligations are restored;
- no automatic retry microtask is armed after a persistent failure.

This last property is deliberate: a throwing component cannot create an infinite microtask loop. Recovery requires an explicit `flush`, a later state write, or another ordinary scheduling trigger (`execution.ts:556–561`).

### 5.4 Native structural failure behavior

Native status values are converted into `RetainedRefusalError` in normal expected-status cases. The status detail channel identifies:

- stale child cache miss;
- stale base cache miss;
- child ordinal for targeted recovery.

A stale child ref can be dropped and re-materialized once. If the retry fails, the retained transaction fails; it does not decode a complete View object or fall back to a prior transport.

Unexpected non-status errors propagate. Cycle detection is explicit through `RetainedCycleError`.

### 5.5 Root boundary failure behavior

For direct boundaries, the documented sequence is:

1. retain previous root lease;
2. materialize next root;
3. host-render next root;
4. on success, release previous root and transfer next root lease;
5. release all other temporary leases;
6. on failure, keep previous root and release all temporary leases.

For the H3 host boundary (`deferHostCommit: true`):

1. prepare and materialize next root;
2. commit desired native root;
3. do not make it visible yet;
4. the host frame barrier presents it;
5. `commitVisible()` promotes the desired root only after the host reports a successful frame.

This is why `Tui.prepareRootPublication()` commits the structural desired root before the later host drain, while `commitVisibleAfterDrain()` promotes the visible root.

### 5.6 Wake broker failure behavior

`EnvironmentWakeBroker` (`wake-broker.ts:121–551`) has different automatic and explicit semantics.

Automatic path:

- one microtask per pending edge;
- fair host driver selection;
- native `flushPendingHosts(budget, false)`;
- errors are accepted into the host error channel;
- the microtask does not throw;
- retry-blocked hosts are not continuously requeued;
- asynchronous backend presentation is polled through a timer rather than a busy microtask loop.

Explicit path:

- captures the host pending epoch;
- repeatedly drains up to `MAX_EXPLICIT_DRAINS = 64`;
- calls `throwPending()` after each report;
- forces retry semantics;
- returns after committed epoch reaches the captured epoch;
- otherwise creates a retryable frame-preparation failure and throws.

### 5.7 Host frame failure behavior

`HostInner::render()` (`host.rs:1870–1959`) separates:

- scene preparation failures;
- state candidate preparation failures;
- content candidate preparation failures;
- backend submission failures;
- asynchronous presentation failures;
- successful frame commit.

A preparation failure:

- records failed attempt epoch/revision;
- aborts content candidate;
- discards SceneHost candidate state;
- leaves `frame` authoritative.

A backend failure sets `physical_sync_unknown` where terminal state may no longer match logical state. The next successful candidate invokes `host_recover_native_history_synchronization()` before clearing the marker (`host.rs:2091–2094`).

### 5.8 Content accepted-but-wake-failed semantics

The direct Source FFI mutation can succeed even if one subscribed host cannot wake or drain:

- the Source revision and bytes are accepted;
- the result returns a nonzero drain hint;
- a later host drain reports `SOURCE_WAKE_FAILED`;
- healthy subscribers still receive the accepted Source data;
- the failed host is blocked rather than continuously spun.

This is explicitly tested in `content_ffi.rs:501–584` and in the Rust application content tests around lines 7339–7363.

### 5.9 History ownership failure behavior

History is attach-once:

- `Tui.createHistory()` creates a host-attached History owned by that `Tui`;
- detached History can transfer to a host exactly once;
- a different History after one has already been bound is rejected with `TUI_HISTORY_ALREADY_BOUND`;
- Rust `NativeHistory::take_for_host()` rejects an already attached History;
- History sideband staging is rolled back on producer/evaluation/preparation failure;
- committed History binding is swapped before body publication, matching the Rust `set_history`/install ordering.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 TypeScript structural caches

#### Semantic NativeRef hints

`SEMANTIC_NATIVE` is a `WeakMap<SemanticViewNode, SemanticNativeHint>`:

```text
semantic node object
  → { generation, nativeRef }
```

It is:

- generation-scoped;
- weak;
- acceleration only;
- not a lease;
- not authoritative native ownership.

A hint hit avoids semantic payload inspection. A stale or generation-mismatched hint is not trusted.

#### Transaction-local refs

`MaterializeTx.refs` deduplicates nodes within one materialization transaction. `inProgress` detects cycles. Temporary leases are held until root transfer or abort.

#### Path caches

`native-view-abi.ts` maintains per-session:

- `PATH_REFS`: lineage object→path ref;
- `PATH_SHAPE_REFS`: immutable path shape→path ref.

Native also maintains `path_nodes` and `path_keys`. Path refs are runtime-scoped handles and are not ViewRefs.

#### Style sidecar

`STYLE_REF_CACHE` is generation/runtime scoped:

- weak style object→StyleRef;
- string atom→style atom ref.

The Rust NativeViewRuntime’s style tables remain authoritative. TypeScript style caching is acceleration metadata. `Tui.setTheme()` resets the style sidecar (`runtime.ts:880–894`) because style materializations must resolve against the new host theme.

### 6.2 Native structural retention and maintenance

`NativeViewRuntime` uses:

- weak NodeId→View cache;
- NodeId→NativeRef map;
- paged dense NativeRef slots;
- strong View only while JS lease count is nonzero;
- monotonic, non-recycled NativeRefs within one runtime generation.

The page table (`view_abi.rs:133–280`) drops empty pages while preserving directory high-water metadata. A stale ref into a dropped page is a cache miss.

Maintenance tracks:

- expired semantic cache entries;
- expired NativeRef slots;
- bounded scavenge queue;
- full-sweep count;
- nodes inserted since full sweep;
- pages freed;
- release batches and released refs.

The runtime exposes maintenance and memory snapshots through the native addon for tests/benchmarks.

### 6.3 State cache and invalidation

Rust `ViewStateRegistry` has five central structures:

```text
records: id → mutable ViewStateRecord
committed: id → Arc<ViewStateSnapshot>
dirty: set of state IDs
desired: set of desired bindings
visible: set of visible bindings
in_flight: set pinned by submitted candidate
```

A state mutation:

- applies to a cloned override domain;
- validates against the currently desired/visible kind;
- commits the mutable record only after validation;
- increments general and domain-specific revisions;
- publishes an immutable `Arc<ViewStateSnapshot>` if demanded;
- inserts the ID into a deduplicated dirty set.

No-op mutations do not advance revisions or enqueue dirty work.

### 6.4 Candidate state overlay

`StateCandidateOverlay` (`retained_state/capture.rs:21–72`) contains:

- capture epoch;
- sorted demanded IDs;
- touched immutable versions.

`StateFrameView` reads:

1. touched candidate version;
2. committed version table.

Unchanged clean records are not copied into the overlay. Unmounted records are not visited. If a newer mutation arrives while an old candidate is in flight, the old `Arc` remains readable by the old candidate while the committed table points to the newer version.

### 6.5 Attachment lease invalidation

TypeScript `AttachmentBindingState` keeps:

- desired attachment leases;
- visible attachment leases;
- superseded desired revisions.

For a revisioned host:

- replacing desired bindings moves the old desired lease set into `superseded`;
- visibility promotes the exact desired or superseded revision matching the committed host revision;
- older superseded revisions are released after visibility advances;
- desired and visible leases are released separately.

This is the TypeScript analogue of Rust candidate/in-flight state pinning, but it protects JS/native wrapper/resource liveness rather than state values themselves.

### 6.6 Scheduling granularity

Composition scheduling:

- State writes enqueue subscribed scopes once;
- duplicate invalidations increment a diagnostic counter but do not enqueue duplicates;
- auto-flush is one microtask per runtime;
- explicit `flush()` consumes a pending scheduled token and prevents stale automatic retry after failure.

Host scheduling:

- resource/state/content writes request a host pending mark;
- the native environment stores the affected host set;
- the JS broker does not mirror native subscriptions;
- a fair broker drain invokes native’s own environment queue;
- report `rearm` controls whether runnable work remains;
- `waiting_for_presentation` controls timer-based polling.

Content scheduling:

- multiple Source mutations can return the same environment wake epoch;
- native content fanout coalesces affected hosts;
- host-side `content_dirty_scratch` is reused to reduce per-mutation fanout allocation.

### 6.7 Hot-path operations

Hot path work is intentionally layered:

- exact root hit: one `hostRenderRef`, zero semantic field reads and zero buffer writes;
- semantic hint hit: zero payload reconstruction for the hinted node;
- first-time structural construction: direct NodeId/kind/payload ABI calls;
- small axes 0–4: fixed-arity generated calls;
- wide axes: one borrowed `(track_word, child_ref)` buffer and one builder/buffer call;
- grid: one borrowed flat word buffer;
- text: C-string lane when all spans are NUL-free; exact-byte lane for embedded NUL/non-C-string payloads; variadic word+byte lane for more than four spans;
- state: fixed masks and word/string lanes;
- content: direct FFI buffers and fixed output record.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript execution observability

`execution.ts:254–301` exposes counters for:

- scope mounts/unmounts;
- body calls;
- prop skips;
- state invalidations;
- dirty enqueues and duplicate invalidations;
- no-op/changed outputs;
- flush passes;
- commit batches/aborts;
- exact View reuses;
- new Views.

`pathologicalCommitFailures` records commit-phase failures rather than hiding them.

### 7.2 Wake broker observability

`wake-broker.ts:65–113` exposes:

- pending marks;
- latch wins/already-latched events;
- microtasks queued;
- drains;
- hosts attempted;
- frames committed;
- automatic errors;
- rearms;
- explicit barriers/failures.

When `Bun.env.PERF_RUNTIME_TRACE === "1"`, a bounded 256-event trace records pending, drain, commit, error, and rearm events.

### 7.3 Structural observability

`retained-dag.ts:111–155` exposes counters for:

- hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast-path calls;
- ref words written;
- byte payload bytes;
- scratch reuse;
- stale-ref retries;
- normalized decorated nodes;
- host mutations.

`RetainedPhaseInstrumentation` records:

- transport preparation time;
- native materialization time;
- host commit time.

Native structural maintenance and memory snapshots expose:

- semantic cache entries/live entries;
- NativeRef slots/pages;
- leased/unleased slots;
- NodeId entries;
- path nodes/keys;
- builders/edit transactions;
- style refs/string bytes;
- scavenge queue/processed counts;
- generation/alive state.

### 7.4 State tests as architecture evidence

The inspected Rust state tests encode the following contracts:

- repeated equal presentation assignment is a no-op (`record.rs:184–193`);
- state IDs include host namespace and monotonic local ID (`registry.rs:445–452`);
- initial state creation does not create a committed version or dirty entry (`registry.rs:454–460`);
- multiple accepted writes deduplicate dirty state and capture one current version (`registry.rs:463–485`);
- unmounted state mutations remain dirty but are not captured until demanded (`registry.rs:489–510`);
- mount/remount captures the current mutable value even when no new mutation occurs at mount (`registry.rs:513–558`);
- candidate overlays pin old versions across later mutations and unbinding (`registry.rs:561–617`);
- clearing an override reveals base state (`registry.rs:620–645`);
- disposal rejects bound states, is idempotent for unknown/repeated IDs, and never revives identity (`registry.rs:648–670`);
- visible binding preparation validates all targets before changing visible membership (`registry.rs:672–684`).

### 7.5 Structural native tests as architecture evidence

The inspected `view_abi.rs` tests encode:

- stale child status detail preceding parent publication;
- text cstring/UTF-8 preservation, including embedded NULs;
- grid buffer construction;
- semantic cache consultation before payload parsing;
- localized text layout patch semantics;
- decorated buffer validation;
- wide-axis buffer validation.

These tests demonstrate that the native runtime treats semantic NodeId identity and NativeRef retention as first-class contracts rather than incidental implementation details.

### 7.6 Content FFI tests as architecture evidence

`content_ffi.rs:501–584` verifies:

- accepted direct FFI mutation returns a Source revision;
- environment wake epoch is returned;
- drain flag remains set even when one host’s wake path is failed;
- healthy hosts receive the accepted content;
- reported wake failure does not create an automatic spin.

### 7.7 Missing visibility

The following visibility limitations remain:

- no test execution was performed;
- native frame/terminal behavior is inferred from Rust implementation and test source;
- no allocation profiler or benchmark counters were run;
- no actual N-API metadata handshake was executed;
- no native artifact was loaded;
- no external consumer/plugin package was assumed present.

---

## 8. Cross-boundary findings and contradictions

### 8.1 The boundary is generic and caller-value driven

`AGENTS.md:77–130` defines `iyon-tui` and its TypeScript facade as generic terminal infrastructure. The inspected runtime/transport/native code follows that rule:

- routes contain opaque caller-defined route IDs;
- content annotations carry generic namespaced values;
- Source/Port/Connector names describe generic data flow;
- no product-specific agent/application state is stored in the runtime;
- the Rust host owns terminal mechanics, native input, retained state/content, scene/layout/paint, and scheduling.

No external product/plugin paths were used as evidence.

### 8.2 TypeScript attachment validation and Rust attachment validation overlap intentionally

State/content attachment validity is checked in both planes:

TypeScript:

- semantic traversal;
- duplicate attachment detection;
- resource kind/environment/host checks;
- accepted-node-kind checks;
- wrapper/resource lease preparation.

Rust:

- semantic scene resolution;
- complete candidate state/content target extraction;
- duplicate state attachment detection;
- ViewStateRegistry identity/kind/geometry validation;
- ContentHostRegistry identity validation.

This overlap is not redundant in the same trust domain. The checks occur at distinct boundaries:

- TypeScript validates JS handle/resource ownership;
- Rust validates the complete native semantic candidate against host-owned registries.

A TypeScript-valid attachment can still fail Rust candidate validation if the intervening semantic candidate or native host state is inconsistent.

### 8.3 State has parallel TypeScript and Rust retention ledgers

There is no single shared state-retention object:

```text
TS:
  semantic node → state HandleId
  NativeResourceRegistry → wrapper/resource + desired/visible leases
  AttachmentBindingState → revisioned binding lease sets

Rust:
  ViewStateRegistry → mutable state record
  committed Arc snapshots
  desired/visible/in-flight ID sets
  StateCandidateOverlay
```

The coupling key is the opaque state ID carried in the semantic native View attachment. Visibility is coordinated by host structural revisions and frame commits.

This arrangement protects against two different failures:

- JS wrapper/resource collection while a native frame still uses the attachment;
- mutable Rust state changing while an older candidate frame still needs its previous immutable snapshot.

### 8.4 Native structural runtime and Rust host are distinct native retained layers

`NativeViewRuntime` is the environment-scoped structural identity/lease/cache runtime. `HostInner` is the host-scoped scene/frame/backend runtime.

The structural runtime provides:

- NativeRef identity;
- semantic NodeId cache;
- View construction and localized derivations;
- root render call;
- NativeRef release.

The host runtime provides:

- desired/visible structural revision;
- candidate scene frame;
- backend presentation and receipt;
- state/content candidate commits;
- terminal synchronization and visible frame authority.

`NativeTuiHost.setDesiredViewRef()` is the seam: it resolves a NativeRef in `NativeViewRuntime`, obtains a strong `View`, and passes that View into `HostInner::set_desired_view()`.

### 8.5 Structural accepted-native knowledge is weaker than native ownership

A generation-valid `SEMANTIC_NATIVE` hint is not a lease. It can accelerate resolution but cannot safely be released by a transient caller. `tryRetainedMaterializeRef()` explicitly promotes a hinted ref through `viewRefForNodeId()` to acquire a caller-owned lease (`native-view-abi.ts:151–182`).

This prevents a temporary History/animation/control operation from accidentally releasing another boundary’s root lease.

### 8.6 Root publication has a three-stage commit, not one commit

The current path has three independently meaningful acceptance points:

1. TypeScript composition commit:
   - scope output/children/dependencies become current.
2. Native desired structural commit:
   - NativeRef becomes desired in `RetainedRootBoundary`;
   - HostInner stores desired root and increments structural revision.
3. Host visible frame commit:
   - backend receipt succeeds;
   - HostInner visible frame and Rust state/content visible bindings promote;
   - TypeScript boundary and attachment visible leases promote.

A failure at stage 1/2 preserves the prior authoritative semantic/native desired state. A failure at stage 3 can leave desired native state newer than visible state, with retry obligation retained.

### 8.7 Asynchronous presentation explains desired/visible duplication

`HostInner` retains `candidate_frame` and `presentation` because real terminal submission is asynchronous. New desired state/content/structure may arrive while a previous candidate is in flight.

The source explicitly prevents newer desired work from being accidentally promoted into the older frame:

- candidate epoch and structural revision are captured;
- candidate content/state plans are retained with the candidate;
- in-flight state IDs remain pinned;
- receipt commit promotes only the captured candidate;
- newer `pending_epoch` remains pending.

The TypeScript root and attachment boundaries mirror this with superseded desired revisions.

### 8.8 Specialized ABI routes remain but do not constitute a second complete structural architecture

The generated ABI still exposes:

- path lineage;
- localized text layout patches;
- axis child/path operations;
- grid cell/path operations;
- edit transactions.

These are specialized retained operations that operate on NativeRefs, persistent native sequences, and NodeId lanes. They do not decode an entire JS View object. The complete retained materializer remains the only production structural architecture.

### 8.9 Content data and content controls use different transport lanes

Content controls use N-API object methods:

- create Source/Port;
- connect;
- activate/deactivate/dispose;
- status/snapshot.

High-volume Source data uses direct FFI:

- append/replace;
- clear/seal;
- head truncate;
- annotation records/payloads.

Both lanes address the same environment/source identity tuple and the same Rust Source registry. This is a deliberate control-vs-bulk split, not two competing content stores.

### 8.10 Native source acceptance is not equivalent to frame visibility

A Source append can return `OK` and increment Source revision before any subscribed host presents it. The environment wake epoch and schedule flag signal that hosts must drain, but the later frame can fail, wait on a receipt, or be blocked by a failed host.

Consequently:

```text
Source accepted
  ≠ projection prepared
  ≠ frame submitted
  ≠ frame visible
```

The content status API retains an operating error and cleanup error distinction internally, while the N-API status projection uses one stable `error` field with cleanup taking precedence (`tui.rs:53–72`).

### 8.11 Rust history and generic host state are coupled through candidate extraction

Although History is a generic control, state/content target extraction includes:

- current body;
- current/static/live History Views;
- prospective History replacement Views;
- History content-port attachments;
- History state attachments.

This is why `HostInner::set_history`, `push`, `replace`, and `discard_live` validate complete body-plus-History attachment sets before changing History. History operations can therefore affect state/content desired bindings and frame invalidation even when the root body is unchanged.

---

## 9. Open questions and coverage gaps

1. The exact implementation details of `error-channel.ts`, `access.ts`, `events.ts`, and `handle-registry.ts` were indexed but not fully read. Their public interaction with the inspected runtime is visible at call sites, but their complete contracts are not reconstructed here.
2. `transport/native/factories.ts`, `resources.ts`, and several public handle wrappers were indexed but not fully read. The report infers their role from `runtime.ts`, `addon.ts`, and direct call sites.
3. Several Rust retained-state modules (`geometry.rs`, `presentation.rs`, `capabilities.rs`, `effects.rs`, `damage.rs`, `occurrence.rs`) were indexed but not comprehensively read. Registry/capture/record behavior is covered; detailed effective-state merge rules remain a coverage gap.
4. The detailed `HostInner::commit_frame()` closure body was reconstructed from targeted source excerpts, but not every surrounding helper in `host.rs` was read line-by-line.
5. The full `TuiEnvironment` implementation in `application/environment.rs` was only inspected around host registration, queue drain, completion, and retry-blocking paths. Additional environment-level APIs may exist outside the reported path.
6. The full Rust content registry and projection implementation was not comprehensively read. Content identity and host frame integration are covered through native addon, direct FFI, host call sites, and tests, but projection internals remain outside this assignment’s primary scope.
7. No source execution was performed. Claims about exact runtime counters, actual ABI metadata values in a loaded artifact, terminal receipt timing, and native memory behavior are static/inferred.
8. The repository contains generated ABI code and native generated wrappers. The report identifies their role and key contracts but does not duplicate every generated function signature.
9. The native `NativeViewRuntime` is documented as environment-owned, while `Tui`/`HostInner` are host-owned. The source appears to use one NativeViewRuntime per JS environment, but the exact lifetime of the global `RUNTIME_HANDLES` entry after all hosts disappear was not fully traced.
10. The direct FFI path’s relationship to Bun-only loading and package artifact generation was inspected in `ffi.ts`, but native artifact build selection and CI packaging were not part of this report.
11. No external application/plugin consumer was assumed. The source baseline itself does not establish product-specific consumers.

---

## 10. Evidence appendix

### 10.1 Contract and context read in full

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`

Important contract constraints applied:

- source baseline is authoritative;
- read-only investigation;
- no V5 mapping/disposition;
- generic framework boundary;
- distinguish source facts, inference, and unknowns;
- report exact state/structure/content paths, ownership, failure, and transaction behavior.

### 10.2 TypeScript production files inspected

#### Runtime

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`
  - `Tui`
  - `prepareRootPublication`
  - `commitHistoryBinding`
  - `stageHistoryBinding`
  - `renderCanonical`
  - `renderDirect`
  - `flush`
  - `close`
  - `exit`
  - `setTheme`
- `packages/iyon-tui/src/runtime/wake-broker.ts`
  - `EnvironmentWakeBroker`
  - `RuntimeHostRegistrationImpl`
  - `NativeHostDrainReport`
  - `NativeHostCommit`
  - wake counters/trace
- `packages/iyon-tui/src/runtime/environment.ts`
  - `RuntimeEnvironment`
  - `runtimeEnvironment`
- `packages/iyon-tui/src/runtime/attachments.ts`
  - `AttachmentBindingState`
  - `prepareSemanticAttachments`
  - `validateSemanticAttachments`
- `packages/iyon-tui/src/runtime/native-resource-registry.ts`
  - runtime re-exports

#### Composition

- `packages/iyon-tui/src/composition/execution.ts`
  - `RetainedExecutionScope`
  - `RetainedExecutionRuntime`
  - `ExecutionError`
  - `OwnedBuilderRoot`
  - `invokeComponent`
  - `mountExistingRoot`
  - `replaceProducer`
  - prepare/commit/abort walkers
- `packages/iyon-tui/src/composition/child-owner.ts`
  - `ChildOwnerState`
  - `KeyGroup`
  - `ChildRecord`
- `packages/iyon-tui/src/composition/publication.ts`
  - publication interfaces
- `packages/iyon-tui/src/composition/tracked-state.ts`
  - `State<T>`
  - `StateSource`
  - tracked invalidation

#### Structural transport

- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `MaterializeTx`
  - `RetainedRefusalError`
  - `RetainedCycleError`
  - `ensureNative`
  - `ensureSemanticNative`
  - `renderExactRoot`
  - `acquireKnownRoot`
  - `RetainedRootBoundary`
  - `RootPublication`
  - style and payload materializers
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `NativeViewAbiSession`
  - `nativeViewAbiSession`
  - `tryRetainedMaterializeRef`
  - axis/grid/edit transaction wrappers
  - `nativePathRefForLineage`
  - `releaseNativeViewRef`
- `packages/iyon-tui/src/transport/structural/ir.ts`
  - kind codes and structural payload types
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - generated NativeRef constructors, patches, path/edit operations, releases
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
  - `NativeViewAbiHandle`
- `packages/iyon-tui/src/transport/structural/policy.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/structural/encoding.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/structural/retained-path.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/structural/style-lowering.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/structural/component-id.ts`
  - indexed, not fully read

#### State transport

- `packages/iyon-tui/src/api/view/retained-state.ts`
  - `ViewState`
  - public geometry/presentation APIs
- `packages/iyon-tui/src/transport/state/control.ts`
  - normalization and envelopes
- `packages/iyon-tui/src/transport/state/generated/state_envelope.ts`
  - generated mask/lane encoders

#### Content/native transport

- `packages/iyon-tui/src/transport/content/control.ts`
- `packages/iyon-tui/src/transport/content/ffi.ts`
  - metadata validation
  - identity tuple
  - annotation encoding
  - mutation result handling
- `packages/iyon-tui/src/transport/native/addon.ts`
  - native contracts
- `packages/iyon-tui/src/transport/native/resource-registry.ts`
  - `NativeResourceRegistry`
  - `PreparedResourceLease`
  - resource identity/lifecycle
- `packages/iyon-tui/src/transport/native/resources.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/native/factories.ts`
  - indexed, not fully read
- `packages/iyon-tui/src/transport/native/artifact.ts`
  - indexed, not fully read

### 10.3 Native addon files inspected

- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/tui.rs`
  - `NativeTuiHost`
  - `NativeHistory`
  - `NativeTextInput`
  - `NativeTextSource`
  - `NativeContentPort`
  - `NativeContentConnector`
  - host epochs, desired ViewRef, flush, lifecycle
- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - `NativeViewRuntime`
  - `NativeViewSlot`
  - `NativeRefTable`
  - `PathNode`, `PathKey`
  - `AxisBuilder`, `EditTxn`
  - semantic publication/lease/cache functions
  - generated ABI implementation
- `crates/iyon-tui-native/src/tui/view_state.rs`
  - `NativeViewState`
  - state envelope decoders
- `crates/iyon-tui-native/src/content_ffi.rs`
  - direct Source ABI metadata/result records
  - Source mutation entrypoints
  - panic guard/status conversion
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
  - indexed through inclusion/call sites
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
  - indexed
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
  - indexed
- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
  - indexed
- `crates/iyon-tui-native/src/generated/view_state_schema.rs`
  - included and consumed by `view_state.rs`
- `crates/iyon-tui-native/src/generated/view_abi_conformance.rs`
  - included and referenced by probes
- `crates/iyon-tui-native/src/error.rs`
  - indexed, not fully read
- `crates/iyon-tui-native/src/sync.rs`
  - indexed, not fully read
- `crates/iyon-tui-native/src/tui/theme_dto.rs`
  - indexed, not fully read

### 10.4 Rust retained-runtime files inspected

- `crates/iyon-tui/src/application/host.rs`
  - `HostInner`
  - desired/visible/candidate state
  - `set_desired_view`
  - `flush_for_environment`
  - `environment_pending_epoch`
  - `invalidate_state`
  - `render`
  - `present_frame`
  - `commit_frame`
  - candidate discard/retry
- `crates/iyon-tui/src/application/environment.rs`
  - `HostEpochs`
  - `HostDrainReport`
  - host completion/requeue/retry-blocking
  - pending host drain
- `crates/iyon-tui/src/application/kernel.rs`
  - state/content attachment target extraction
  - frame preparation ingress
- `crates/iyon-tui/src/retained_state/mod.rs`
- `crates/iyon-tui/src/retained_state/record.rs`
  - `ViewStateRecord`
  - `ViewStateLifecycle`
  - revision/effect mutation
- `crates/iyon-tui/src/retained_state/registry.rs`
  - `ViewStateRegistry`
  - `PreparedStateCommit`
  - desired/visible/in-flight transitions
  - candidate capture
- `crates/iyon-tui/src/retained_state/capture.rs`
  - `StateCandidateOverlay`
  - `StateFrameView`
- `crates/iyon-tui/src/retained_state/capabilities.rs`
  - indexed
- `crates/iyon-tui/src/retained_state/damage.rs`
  - indexed
- `crates/iyon-tui/src/retained_state/effects.rs`
  - indexed
- `crates/iyon-tui/src/retained_state/geometry.rs`
  - indexed
- `crates/iyon-tui/src/retained_state/occurrence.rs`
  - indexed
- `crates/iyon-tui/src/retained_state/presentation.rs`
  - indexed

### 10.5 Files merely indexed/not fully read

The evidence manifest was consulted. The following relevant files were indexed or sampled but not fully read line-by-line:

- remaining TypeScript runtime helper files:
  - `runtime/access.ts`
  - `runtime/error-channel.ts`
  - `runtime/events.ts`
  - `runtime/handle-registry.ts`
- remaining composition helper files:
  - `composition/compose.ts`
  - `composition/define-view.ts`
  - `composition/execution-context.ts`
  - `composition/persistent-seq.ts`
- remaining structural transport helpers:
  - `encoding.ts`
  - `policy.ts`
  - `retained-path.ts`
  - `style-lowering.ts`
  - `component-id.ts`
- remaining native helper files:
  - `transport/native/factories.ts`
  - `transport/native/resources.ts`
  - `transport/native/artifact.ts`
- detailed Rust state modules:
  - `retained_state/capabilities.rs`
  - `retained_state/damage.rs`
  - `retained_state/effects.rs`
  - `retained_state/geometry.rs`
  - `retained_state/occurrence.rs`
  - `retained_state/presentation.rs`
- portions of Rust content/projection/backend code outside host ingress and cited tests
- generated source bodies not needed to establish the ABI ownership/shape contract

### 10.6 No executed validation

No command was run to load the native artifact, run TypeScript, exercise a host, execute tests, or collect counters. All runtime behavior in this report is derived from source control flow, type contracts, comments, and existing test code.