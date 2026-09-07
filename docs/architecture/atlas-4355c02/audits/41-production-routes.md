# 41 — Production routes

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: all production code, with emphasis on semantic-operation routing, authoritative paths, alternate/cold paths, recovery paths, compatibility residue, and exact failure branches.
- Framework boundary applied: `iyon-tui` and its TypeScript facade are generic terminal/UI mechanics. No Iyon application/plugin behavior was assumed or invented.

The assignment evidence identifies this scope as:

> Semantic-operation-to-path matrix; authoritative/alternate/cold/fallback routes and exact branch/failure conditions. No cleanup implementation.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- the tracked source manifest at `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

### Evidence status

This is a static source investigation. I did not run builds, tests, benchmarks, terminal sessions, or native code. Therefore:

- “Observed” below means observed in source control flow, not runtime execution.
- Existing test names and assertions are reported as behavioral evidence, but no test is claimed to have run during this assignment.
- Line references are source line numbers from the baseline files as returned by repository inspection.
- Parent-added atlas documentation is treated as outside the source baseline.

### Scope boundaries

The route analysis covers:

1. TypeScript public/runtime entrypoints.
2. TypeScript retained semantic construction and structural transport.
3. TypeScript content control and bulk-data transport.
4. Rust native N-API wrappers.
5. Rust retained view ABI materializers and NativeRef runtime.
6. Rust host/application frame, content, history, state, input, output, and terminal routes.
7. Generated ABI/schema surfaces where they select or constrain production paths.
8. Production-vs-test/debug/qualification route separation.

The source manifest is exhaustive for tracked source inventory. The principal route-relevant production groups are:

- `packages/iyon-tui/src/runtime/`
- `packages/iyon-tui/src/composition/`
- `packages/iyon-tui/src/api/view/`
- `packages/iyon-tui/src/api/controls/`
- `packages/iyon-tui/src/api/content/`
- `packages/iyon-tui/src/transport/`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`
- `crates/iyon-tui/src/application/`
- `crates/iyon-tui/src/presentation/`
- `crates/iyon-tui/src/history/`
- `crates/iyon-tui/src/retained_state/`
- `crates/iyon-tui/src/content/`
- `crates/iyon-tui/src/scene/`
- `crates/iyon-tui/src/terminal/`
- `crates/iyon-tui/src/interaction/`
- `crates/iyon-tui/src/component/`
- `crates/iyon-tui/src/backend/`

---

## 1. Responsibility and structure

### 1.1 Route-oriented module inventory

| Production area | Main files | Primary route responsibility | Alternate/cold/fallback behavior |
|---|---|---|---|
| Public TypeScript root/runtime | `packages/iyon-tui/src/index.ts`, `runtime/runtime.ts` | Public `Tui`, `render`, `flush`, input, resize, lifecycle, handle creation | Direct scene and retained producer are distinct ownership modes |
| Semantic View model | `packages/iyon-tui/src/api/view/view.ts`, `semantic-node.ts` | Immutable semantic View/node construction and identity | Retained construction context routes constructors through composition helpers |
| Retained composition | `packages/iyon-tui/src/composition/*.ts` | Retained scopes, child ownership, tracked state, producer evaluation | Positional reuse, keyed reuse, prop skip, aborted evaluation rollback |
| Structural retained transport | `packages/iyon-tui/src/transport/structural/retained-dag.ts` | NativeRef correspondence, semantic DAG materialization, derivation fast paths, root leases | Hint hit, transaction-local hit, NodeId promotion, direct materialization, one stale retry |
| Structural ABI facade | `packages/iyon-tui/src/transport/structural/native-view-abi.ts` | Generated N-API calls for materialization and structural updates | Small fixed-arity constructors vs variable-axis builders; direct host object vs install callback |
| Content control transport | `packages/iyon-tui/src/transport/content/control.ts` | N-API Source/Port/Connector control operations | No compatibility transport selected |
| Content bulk transport | `packages/iyon-tui/src/transport/content/ffi.ts` | Bun `dlopen` Source append/replace/clear/seal/truncate | Per-environment FFI session cache; no N-API payload fallback |
| Native addon wrapper | `crates/iyon-tui-native/src/tui.rs` | N-API classes and host-facing operations | Headless/real host is selected in Rust host |
| Native View ABI runtime | `crates/iyon-tui-native/src/tui/view_abi.rs` | NativeRef table, semantic cache, ABI operation implementation | Cache miss, stale references, explicit status failures, direct-FFI qualification exports |
| Rust host/frame | `crates/iyon-tui/src/application/host.rs` | Desired/visible frame epochs, candidate frame, backend receipt, frame commit | Headless vs real backend; async receipt and retry paths |
| Rust content provider | `crates/iyon-tui/src/application/content.rs` | Source storage, connector lifecycle, semantic projection, layout/paint products, history rows | Candidate vs committed projection; cached projection; visible rollback |
| Rust History | `crates/iyon-tui/src/history/*` | Ordered semantic units, live/static/frozen content, viewport and native frontier | Semantic resident route vs native physical rows route |
| Rust state plane | `crates/iyon-tui/src/retained_state/*`, native `view_state.rs` | Validated retained state envelopes, candidate/committed state, invalidation | Geometry/presentation wake classification and explicit validation failures |
| Rust old/core presentation | `crates/iyon-tui/src/presentation/*`, `scene/*` | Semantic View to layout/paint/cells | Still the rendering implementation consumed by native host |
| Rust input/output | `interaction/*`, `output/*`, `application/input.rs` | Native key/paste/focus/routing/output channels | Focused/modal chain, global fallback, ignored output |
| Terminal backend | `crates/iyon-tui/src/terminal/*`, `backend/*` | Termwiz/Crossterm setup, frame changes, flush, restore | Real terminal and headless sink are separate backend modes |

### 1.2 Approximate physical source footprint

The repository manifest includes both production and test/generated files. I did not run a line-count command in this read-only investigation environment, so the following are intentionally approximate bands derived from source-file line endpoints and the manifest rather than exact `wc -l` output:

- `crates/iyon-tui`:
  - roughly 100–120 handwritten production files;
  - approximately 28,000–34,000 production physical LOC;
  - approximately 18,000–24,000 Rust test LOC;
  - largest route-heavy files include `application/content.rs` (source extends beyond line 9,600), `application/host.rs` (beyond line 3,000), and `presentation/layout/*`.
- `crates/iyon-tui-native`:
  - approximately 8–10,000 handwritten production LOC;
  - `tui/view_abi.rs` extends beyond line 6,200, with a large test section beginning around line 4,603;
  - approximately 1,000–2,000 native test LOC;
  - generated bodies and ABI tables are separate maintenance artifacts.
- `packages/iyon-tui`:
  - approximately 60–65 TypeScript source files in the tracked manifest, excluding generated schema bodies and tests;
  - approximately 17,000–22,000 handwritten production LOC;
  - approximately 8,000–12,000 TypeScript test/fixture LOC;
  - route-heavy files include `transport/structural/retained-dag.ts` (beyond line 2,100), `composition/execution.ts` (beyond line 1,200), `runtime/runtime.ts` (around 900), and `api/content/retained.ts`.
- Generated ABI/schema files are present under:
  - `packages/iyon-tui/src/transport/abi/structural/generated/`
  - `packages/iyon-tui/src/transport/state/generated/`
  - `crates/iyon-tui-native/src/generated/`
  - `crates/iyon-tui-native/include/`
- The estimates above should not be used as acceptance metrics. Exact physical LOC should be recomputed by the parent if the integrated report needs numerical totals.

### 1.3 Architectural route summary

The source has one dominant current production architecture for structural rendering:

```text
public TS scene / producer
        │
        ├─ direct Scene value
        └─ retained producer evaluation
                │
                ▼
        immutable semantic View/node DAG
                │
                ▼
        TS retained NativeRef materialization
                │
                ▼
        generated structural N-API calls
                │
                ▼
        native View ABI NativeRef table
                │
                ▼
        NativeTuiHost.setDesiredViewRef(...)
                │
                ▼
        Rust TuiHost / HostRunning
                │
                ▼
        Scene resolve → layout → paint → physical surface
                │
                ▼
        headless commit or real terminal receipt
```

The important nuance is that the current native View ABI materializes Rust `View` values inside the native runtime. It does not serialize a complete generic object back through a secondary decoder, but the final host frame still executes the existing Rust `View`/`Scene`/layout/paint pipeline.

---

## 2. Types, APIs and contracts

### 2.1 Public TypeScript runtime contract

`packages/iyon-tui/src/runtime/runtime.ts` defines:

- `TuiRuntime.render(scene, signal?)`
- `flush()`
- `nextEvent()`
- `resize()`
- `close()`
- `exit()`
- `createHistory()`
- `viewState()`
- `contentPort()`
- `createTextInput()`
- `createViewSlot()`
- `createScrollPane()`

The runtime comments explicitly distinguish:

- direct structural scene values;
- retained scene producers.

`runtime.ts` lines 64–68 describe the distinction, and lines 373–389 document the intended structural route:

```text
same body object → no-op
warm root hint → exact-root fast path
otherwise → boundary install / ensureNative
refused → explicit preparation failure
```

However, the actual call graph needs a qualification:

- `renderCanonical()` uses `OwnedBuilderRoot` and the shared `RetainedExecutionRuntime`.
- `renderDirect()` invokes `prepareRootPublication()`.
- `prepareRootPublication()` invokes `RetainedRootBoundary.prepareDesiredInstall()`.
- `prepareDesiredInstall()` invokes `prepareFrom()`, which invokes `ensureSemanticNative()`.
- `renderExactRoot()` exists, but repository-wide search found no external production caller of `RetainedRootBoundary.renderExact()`. The method itself calls `renderExactRoot()`, but the public `Tui` runtime path currently routes through `prepareDesiredInstall()` rather than visibly selecting `renderExact()` as an early branch.

This is a material route/documentation discrepancy: the documented “warm root hint → exact-root fast path” is implemented as a callable route, but it is not demonstrably the authoritative route for `Tui.render()` from the inspected call graph.

### 2.2 Semantic View contract

`packages/iyon-tui/src/api/view/view.ts` and `semantic-node.ts` own:

- immutable semantic node shape;
- NodeId allocation;
- child relationships;
- normalized semantic styles and decorations;
- semantic attachments;
- derivation metadata;
- construction-time identity.

`semantic-node.ts` explicitly says that semantic nodes have no knowledge of structural transport, native handles, ABI schema, or generated calls. `view.ts` similarly states that structural transport is absent from View construction.

Constructors route as follows:

- `View.text()`:
  - validates string;
  - if retained construction is active, calls `composeText()`;
  - otherwise creates an immutable semantic text node.
- `View.spacer()`:
  - validates `u16` row count;
  - retained construction calls `composeSpacer()`;
  - otherwise creates a semantic spacer.
- `View.grid()`:
  - retained construction calls `composeGrid()`;
  - otherwise calls `rawGrid()`.
- Modifiers create fresh semantic identities.
- Text layout changes attach `"textLayout"` derivation metadata.
- Scalar-only decoration changes attach `"commonScalar"` derivation metadata.
- Axis child updates attach `"axisSet"` or `"axisSplice"` derivations.
- Grid cell updates attach `"gridCell"` derivations.

`view.ts` lines 240–246, 299–323, 360–362, 493–508, and 511–554 provide the relevant construction branches.

Semantic identity is therefore the declaration; NativeRef hints and native storage are derived physical state.

### 2.3 Native structural ABI contract

`packages/iyon-tui/src/transport/structural/native-view-abi.ts` exposes internal route helpers:

- `nativeViewAbiSession()`
- `nativeViewRefForNodeId()`
- `tryRetainedMaterializeRef()`
- `tryRetainedAxisCreate()`
- `tryRetainedAxisCreateRender()`
- `tryRetainedAxisSetChildRender()`
- `tryRetainedAxisSpliceRender()`
- `tryRetainedGridSetCellRender()`
- `tryRetainedEditTransactionRender()`
- `nativePathRefForLineage()`
- `releaseNativeViewRef()`

Session acquisition is cached once per JS environment at lines 98–128. It validates:

- ABI name;
- ABI version;
- semantic version;
- schema fingerprint;
- generator fingerprint;
- transport kind `"napi"`;
- positive generation;
- function count.

Any mismatch throws `"native View N-API metadata is incompatible"`. A failed `runtimeNoop()` bootstrap probe also throws.

This means ABI incompatibility is a hard runtime failure, not a selection of a prior ABI or alternate decoder.

### 2.4 Retained NativeRef contract

`retained-dag.ts` owns:

- weak semantic-to-NativeRef hints;
- environment scratch arrays;
- generation-scoped style sidecars;
- temporary lease tracking;
- transaction-local NativeRefs;
- stale-ref recovery;
- direct semantic materializers;
- root boundary lease ownership.

The principal identity order is documented at lines 1149–1152:

```text
semantic NativeRef hint
→ transaction-local ref
→ ceiling-gated NodeId → NativeRef promotion
→ semantic payload inspection
→ child traversal/materialization
```

This order is actual production logic in `ensureSemanticNative()`:

- same-generation hint hit returns a borrowed NativeRef;
- transaction-local hit returns the existing ref;
- NodeId promotion is attempted only when `node.id <= tx.nativeLookupCeiling`;
- promotion miss falls through to direct semantic materialization;
- new nodes above the ceiling skip the extra NodeId probe;
- direct materialization traverses children as needed.

The `nativeLookupCeiling` is captured after successful root publication and is intended to distinguish potentially pre-existing nodes from definitely new nodes.

### 2.5 Native host and generated ABI contract

`crates/iyon-tui-native/src/tui/view_abi.rs` provides the native implementation behind generated calls. The key boundary functions include:

- `view_ref_for_node_id_impl()`
- `view_render_ref_impl()`
- `host_render_ref_impl()`
- `view_state_attach_impl()`
- `path_root_impl()`
- `path_child_impl()`
- `view_text_layout_patch_*_impl()`
- `edit_txn_*_impl()`
- `view_content_host_create_impl()`
- `view_spacer_create_impl()`
- `view_axis_create_buffer_impl()`
- fixed-arity row/column constructors;
- axis builder functions;
- axis child/splice functions;
- grid functions;
- diff/decorated/container/clamp/component constructors;
- text and style constructors;
- `view_release_many_impl()`.

`host_render_ref_impl()` lines 1753–1776:

1. validates the NativeView runtime;
2. validates the host pointer;
3. validates host liveness;
4. resolves the NativeRef;
5. returns `HOST_STATUS_CACHE_MISS` if the reference cannot resolve;
6. calls `host.host.render(view)`;
7. maps success to `HOST_STATUS_OK`;
8. maps host render failure to `HOST_STATUS_INTERNAL`.

The native implementation retains Rust `View` values in its NativeRef table. `view_for_ref()` lines 1680–1682 resolves a NativeRef to a Rust `View`.

---

## 3. Dependency and ownership map

### 3.1 Main ownership graph

```text
Tui (TS)
 ├─ NativeTuiHost handle
 ├─ RetainedExecutionRuntime
 ├─ optional root OwnedBuilderRoot
 ├─ optional RetainedRootBoundary
 ├─ AttachmentBindingState
 ├─ owned ViewState/History/ContentPort/Slot/Pane handles
 └─ RuntimeHostRegistration
       │
       ▼
NativeTuiHost (N-API)
 ├─ Rust TuiHost
 ├─ native View runtime pointer/handle
 └─ Native environment registration
       │
       ├─ HostRunning / Scene / History
       ├─ ContentHostRegistry
       ├─ ViewStateRegistry
       └─ terminal backend
```

Structural ownership is split:

```text
semantic View object
    └─ weak semantic NativeRef hint (TS sidecar)
        └─ native NativeRef table entry
            └─ Rust View held by native ABI runtime
```

A hint is not a lease. Every transient boundary explicitly acquires and releases a lease as needed.

### 3.2 Root boundary ownership

`RetainedRootBoundary` has two modes:

1. `deferHostCommit = false`
   - used by `ViewSlot` and `ScrollPane`;
   - direct install callback calls `setViewRef()` or `setContentRef()`;
   - `previousRef` is the current boundary root.
2. `deferHostCommit = true`
   - used by `Tui`;
   - `desiredRef` and `visibleRef` are separate;
   - `setDesiredViewRef()` records desired structure;
   - the environment frame drain eventually makes it visible;
   - `commitVisible()` promotes the desired revision only after the frame transaction succeeds.

`retained-dag.ts` lines 1545–1599 define this split. The constructor comments at lines 1591–1596 explicitly identify the direct-install callback as the alternate boundary commit route for:

- `ViewSlot.setViewRef`;
- `ScrollPane.setContentRef`.

### 3.3 Content ownership

Content has three distinct ownership domains:

```text
Source
 └─ environment-owned immutable byte/chunk store
     └─ source identity/generation/revision
         └─ Connector membership/subscription groups

ContentPort
 └─ host-owned destination attachment
     └─ selected desired/visible Connector

Connector
 └─ one Source + one Funnel + one Port
     ├─ parser/projection execution
     ├─ semantic projection cache
     ├─ prepared paint cache
     ├─ projection cache
     ├─ candidate projection
     └─ committed projection
```

`crates/iyon-tui/src/application/content.rs` lines 3215–3217 state that Source storage is deliberately separate from host-owned Port/Connector registries.

### 3.4 Frame ownership

`HostInner` stores:

- `frame`: last complete logical frame;
- `candidate_frame`: frame under preparation/presentation;
- `presentation`: terminal receipt;
- `frame_pending`;
- candidate epoch and structural/content/state revisions.

The last complete frame is authoritative for readback while a candidate is in flight. This is source-backed at `application/host.rs` lines 122–137.

### 3.5 Reverse dependency direction

Important reverse edges:

- TS `Tui.render()` → native structural ABI → N-API `NativeTuiHost.setDesiredViewRef()` → Rust `TuiHost.set_desired_view(View)`.
- TS `View` semantic model → TS retained materializer → native Rust View ABI constructors.
- Rust `HostRunning` → `Scene` → presentation/layout/paint.
- Rust `Scene` → `History` projection and `ContentProvider`.
- Rust content `ContentHostRegistry` → Source/Connector/Projection/TextRenderer/ViewCompiler.
- Rust `ViewStateRegistry` → retained state candidate overlay → Scene resolution.
- Rust terminal backend → physical surface changes and receipt.
- TS wake broker → native `flushPendingHosts()` → Rust environment fair pending-host queue.

The current implementation does not have a production route from TS semantic View to a separate non-View renderer. Native ABI materialization eventually produces the Rust `View` object consumed by the existing Rust scene pipeline.

---

## 4. Execution paths and state transitions

### 4.1 Direct scene render

The direct `Tui.render(scene)` route is selected when the argument is not a function.

Source: `packages/iyon-tui/src/runtime/runtime.ts` lines 399–405 and 499–549.

Call chain:

```text
Tui.render(Scene)
  → renderDirect()
      → ensureSignal()
      → ensureOpen()
      → drainExecution()
      → assertNotMutating()
      → Scene.from(scene)
      → semanticNodeOf(normalized.body)
      → validate History ownership if present
      → validate semantic attachments on same-body fast path
      → nativeViewAbiSession()
      → prepareRootPublication()
          → prepareSemanticAttachments()
          → new Scene(...)
          → RetainedRootBoundary.prepareDesiredInstall()
              → prepareFrom()
                  → ensureSemanticNative()
                  → materialize/derive NativeRef
              → publishDesiredPrepared()
                  → host.setDesiredViewRef(rootRef)
                  → capture desired host revision
                  → transfer desired root lease
      → commit publication
      → dispose any old producer root
      → flush()
          → retained execution flush
          → host registration flush
```

The direct scene route has these branches:

1. **Same scene body and same effective History**
   - validates semantic attachments;
   - stages effective History;
   - disposes any root builder;
   - flushes;
   - returns without a new structural publication.
2. **Different body or History**
   - validates and prepares attachments;
   - materializes the retained root;
   - publishes desired NativeRef;
   - commits History sideband;
   - marks the host pending.
3. **History instance differs from an already bound History**
   - throws `TUI_HISTORY_ALREADY_BOUND`.
4. **Retained preparation returns `undefined`**
   - `prepareRootPublication()` returns a publication type but its internal boundary preparation can refuse;
   - callers convert this to explicit update failure or throw the retained preparation error.
5. **Any evaluation/preparation failure**
   - restores staged History to the prior value;
   - attachments are aborted;
   - old visible root remains authoritative.
6. **Frame failure after desired structural commit**
   - desired structural revision remains pending/retryable;
   - visible frame remains old.

### 4.2 Retained producer render

The producer route is selected when `Tui.render()` receives a function.

Source: `runtime.ts` lines 391–497.

Call chain:

```text
Tui.render(() => Scene)
  → renderCanonical()
      → ensureSignal()
      → ensureOpen()
      → drainExecution()
      → assertNotMutating()
      → producer wrapper
          → Scene.from(builder())
          → stageHistoryBinding(scene.history)
          → return scene.body
      → first call:
          OwnedBuilderRoot.start()
            → RetainedExecutionRuntime mount/evaluate
            → preparePublication(root View)
            → RetainedRootBoundary.prepareDesiredInstall()
            → commit desired structural root
      → later calls:
          OwnedBuilderRoot.replaceProducer()
            → retained scope reevaluation
            → prop/identity/child reconciliation
            → prepare/commit through same root publication protocol
      → flush()
```

Producer-specific semantics:

- Closure identity is irrelevant after the root is established.
- Tracked state reads subscribe the root scope.
- A state write invalidates the root and schedules a microtask flush.
- Same props may skip a component body via `propsShallowEqual()`.
- Keyed children are reconciled through keyed groups.
- Failed evaluation restores prior subscriptions and WIP ownership.
- A producer failure restores staged History; a later frame failure leaves desired structure pending.

The first-root failure branch at lines 463–481 is important: the root is not marked live until initial evaluation/materialization succeeds. This leaves the `Tui` retryable after a failed first render.

### 4.3 Structural materialization

`ensureSemanticNative()` route:

```text
ensureSemanticNative(node, tx)
  ├─ same-generation semantic hint → borrowed ref
  ├─ transaction-local ref → local reuse
  ├─ node.id <= nativeLookupCeiling?
  │    ├─ viewRefForNodeId hit → install hint, temporary lease
  │    └─ cache miss → direct route
  ├─ cycle guard
  ├─ derivation hint?
  │    ├─ base hint/promotion available → derivation fast path
  │    └─ base unavailable → direct semantic route
  └─ materializer selected by semantic kind
```

Source: `retained-dag.ts` lines 1149–1219 and 1222–1364.

Materializers selected by semantic kind:

| Semantic kind | Materializer | Native construction route |
|---|---|---|
| `spacer` | `materializeSpacerNode()` | `viewSpacerCreate` |
| `contentHost` | `materializeContentHostNode()` | `viewContentHostCreate` |
| `row` | `materializeRowNode()` | fixed row constructor for ≤4 children, buffer constructor otherwise |
| `column` | `materializeColumnNode()` | fixed column constructor for ≤4 children, buffer constructor otherwise |
| `grid` | `materializeGridNode()` | one packed word buffer |
| `text` | `materializeTextNode()` | cstring family for NUL-free ≤4 spans; UTF-8 family for NUL-containing ≤4 spans; buffer family above 4 spans |
| `diff` | `materializeDiffNode()` | packed words plus bytes |
| `hanging` | `materializeHangingNode()` | three child refs |
| `container` | `materializeContainerNode()` | one child ref |
| `clamp`/`contentMax` | `materializeClampNode()` | child ref plus overflow metadata |
| `component` | `materializeComponentNode()` | component identity |
| `decorated` | `materializeDecoratedNode()` | child ref plus packed decoration/state payload |

All are part of the same retained architecture. There is no complete-object fallback route.

### 4.4 Derivation fast paths

A semantic node may retain derivation metadata:

- `textLayout`
- `commonScalar`
- `axisSet`
- `axisSplice`
- `gridCell`

`tryDerivation()` runs after identity resolution but before payload-inspecting materialization.

Examples:

- `textLayout` → `viewTextLayoutPatchRoot(baseRef, ...)`
- `commonScalar` → `viewCommonPatchRoot(baseRef, ...)`
- `axisSet` → resolves only replacement child and calls `viewAxisSetChild`
- `axisSplice` → resolves only inserted children and calls `viewAxisSpliceBuffer`
- `gridCell` → resolves only replacement child and calls `viewGridSetCell`

The old base sequence is not re-encoded for axis splice. This is a genuine hot-path specialization inside the retained architecture.

If the base NativeRef is unavailable, `derivationBaseRef()` returns `undefined`, and `tryDerivation()` falls back to direct semantic materialization inside the same transaction. This is a legitimate internal fallback, not a previous-generation transport.

### 4.5 Root publication modes

#### Direct boundary mode

Used by slots and panes.

```text
prepareInstall(view)
  → prepareFrom(view)
  → materialize/derive root
  → installRef(rootRef)
      → NativeViewSlot.setViewRef()
      or
      → NativeScrollPane.setContentRef()
  → transfer root lease
  → release old root only after success
```

Failure:

- unsupported semantic/materialization route → `undefined`, caller throws explicit update failure;
- native install exception → propagated;
- callback returns `false` → old root remains current;
- temporary leases are released.

#### Deferred host mode

Used by `Tui`.

```text
prepareDesiredInstall(view)
  → prepareFrom(view)
  → publishDesiredPrepared()
      → NativeTuiHost.setDesiredViewRef(rootRef)
      → record desired structural revision
      → retain desired root
  → environment flush
      → Rust host frame preparation
      → host/backend receipt
      → commitVisible(revision)
```

The native host is not painted by `setDesiredViewRef()` itself. `crates/iyon-tui-native/src/tui.rs` lines 655–672 describe the call as accepting desired structure without presenting it. `RetainedRootBoundary.commitVisible()` lines 1738–1788 promotes the desired root only after the host frame succeeds.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation → production-path matrix

| Semantic operation | Authoritative production path | Cold/alternate route | Recovery/fallback | Exact failure behavior |
|---|---|---|---|---|
| `Tui.render(Scene)` | `renderDirect()` → `prepareRootPublication()` → deferred retained root publication | Same-body identity no-op | None; new body uses retained materializer | Invalid scene/body/History ownership throws; retained refusal leaves old visible root |
| `Tui.render(() => Scene)` | `renderCanonical()` → retained execution root scope → same deferred publication target | First mount vs later producer replacement; keyed/positional child reuse | Scope WIP rollback and staged-History restoration | Async body rejected; evaluation errors abort batch; frame failure leaves desired revision retryable |
| View NativeRef lookup | `nativeViewRefForNodeId()` / `viewRefForNodeId` | No hint or no prior NativeRef | Direct materialization only after promotion miss | Native status is interpreted as cache miss where expected; unexpected errors propagate |
| Generic View materialization | `tryRetainedMaterializeRef()` → `ensureSemanticNative()` | Hint hit, tx-local hit, NodeId promotion | One stale-hint retry; direct materialization | `RetainedRefusalError`/cycle becomes `undefined` at helper boundary; unexpected errors throw |
| Warm exact root host render | `renderExactRoot()` → `hostRenderRef()` | Root cache miss → NodeId promotion and one retry | One targeted stale root retry | Unavailable recovery returns `no_root_ref`; second miss returns `no_root_ref`; other host statuses throw |
| Actual `Tui` root route | `RetainedRootBoundary.prepareDesiredInstall()` | Deferred desired vs visible root roles | Host frame barrier/`commitVisible()` | Desired publication can succeed while visible frame remains old; no old-generation structural transport |
| Slot `setView(View)` | `ViewSlot.setViewDirect()` → slot boundary `prepareInstall()` → `setViewRef()` | Existing builder root is disposed only after direct publication success | None beyond boundary materialization/retry | Refusal becomes `TUI_VIEW_SLOT_UPDATE_FAILED`; old builder/root remains authoritative |
| Slot `setView(() => View)` | Owned slot builder root → shared retained execution runtime | Existing scope reevaluation and child reconciliation | Batch rollback | Producer/evaluation error propagates; direct mode does not silently take over |
| Slot animation frame setup | `tryRetainedMaterializeRef()` for every frame → native `setAnimationRef*` or `setAnimationRefs` | Scalar fixed-arity setters vs variadic typed-array setter | None; failed frame materialization is explicit | `TUI_VIEW_SLOT_ANIMATION_UPDATE_FAILED`; builder remains authoritative if native install fails |
| Slot animation tick | Native Rust slot scheduler selects frame | JS only supplies frame refs and interval | Rust-side stable scheduling | Tick/render failures are native lifecycle/runtime failures; TS does not drive per-tick N-API |
| Pane `setContent(View)` | Pane boundary → `setContentRef()` | Builder mode via owned retained root | None beyond retained preparation | `TUI_SCROLL_PANE_UPDATE_FAILED`; old content remains installed |
| History `push(View)` | `tryRetainedMaterializeRef()` → `NativeHistory.pushRef()` → host frame | Existing semantic NativeRef hint | One materialization stale retry | `HISTORY_PUSH_FAILED` if materialization refuses; NativeHistory errors propagate; temporary ref always released |
| History `freeze(unit, View)` | Retained materialization → `NativeHistory.freezeRef()` → host frame | Existing hint and native cache | Same retained retry | `HISTORY_FREEZE_FAILED`; invalid/non-live unit fails in Rust History |
| Source append/replace | TS validation → Bun direct FFI `iyon_tui_source_*_utf8_v1` | Per-environment cached `dlopen` session | No N-API fallback | ABI mismatch, size, annotation, sealed source, or native status becomes typed `TuiError` |
| Source clear/seal/truncate | TS `invokeNoPayload()` → direct FFI symbol | Cached FFI session | No alternate payload route | `SOURCE_SEALED`, invalid offset, ABI or runtime errors throw |
| Port connect | TypeScript type/ownership validation → N-API `port.connect()` → Rust ContentHostRegistry | No alternate family; text only | None | Invalid source/funnel/family throws before native connect |
| Connector activate | N-API connector control → Rust candidate activation | Requested-but-unmounted connector remains cold | Visible connector can remain selected on failed switch | Activation/projection failure is recorded; old visible connector can remain visible |
| Connector deactivate | N-API control → Rust desired state | Port-level deactivation removes selected destination | Existing visible state remains until frame commit | Errors are surfaced through status/error lane |
| Connector projection | Rust `prepare_connector_projection()` | Candidate projection cache hit; committed/cache lookup | Cached committed projection used for visible rollback; retry block by same failed key | Same failed key returns `PROJECTION_RETRY_BLOCKED`; projection errors are recorded |
| Content measurement | `measure_content()` → selected connector → `prepare_connector_projection()` | Candidate, committed, or projection-cache measurement | `projection_measurement()` and visible connector rollback; fit-width rekey | Missing/poisoned registry paths often return default measurement; projection failures may retain old measurement |
| Content paint | Prepared ticket → `projection_for_ticket()` → `paint_window_direct()` | Candidate projection, committed projection, then projection cache | No newest-projection substitution; missing ticket returns without painting | Missing projection ticket is silently a no-op in `paint_window_direct()` |
| History content rows | `history_rows()` → selected connector projection → finalized-prefix rows | Native frozen rows / finalized-prefix proof | Native physical rows and semantic resident rows coexist | Missing connector/source/projection returns `None` |
| ViewState geometry mutation | TS normalization/envelope → N-API `setGeometry` → Rust state record | Clear-specific envelope vs set envelope | Wake bit determines environment drain | Malformed envelope rejected before state mutation; disposed state rejected |
| ViewState presentation mutation | TS normalization/envelope → N-API `setPresentation` → Rust state record | Clear-specific envelope vs set envelope | Theme/presentation invalidation | Same transactional validation semantics |
| Native key dispatch | `Tui.nextEvent()`/runtime access → N-API `dispatchKey()` → Rust interaction routing | Headless has no terminal poll; explicit dispatch still available | Focus/modal/global routing | Parse/interaction errors propagate; ignored key produces no output |
| Native paste dispatch | N-API `dispatchPaste()` → interceptor/focused routing | `forwardPaste()` deliberately bypasses interceptors | Global/default handling | Routing errors propagate; forwarded paste does not re-enter interceptor chain |
| Output wait, no signal | `NativeTuiHost.waitForOutput()` | Native async wait | Native Rust driver handles terminal/ticks/wakes | Closed host returns terminate/null |
| Output wait, with signal | JS `pollOutput()` → `pollTerminal()` → `nextOutput()` → bounded delay | This is a genuine API-level wait-mode alternate | Poll interval bounded by `nextWakeMs()` and 1–16 ms | Abort throws cancellation; closed runtime returns null |
| Frame preparation | Rust `prepare_frame_with_content()` | Headless sink vs real Termwiz backend | Candidate/committed frame barrier | Preparation errors discard candidate and restore pending obligation |
| Frame presentation | Real backend `begin_frame()` and receipt | Headless has no asynchronous receipt | `finish_presentation_blocking()` or polling receipt | Terminal render/flush error marks physical sync unknown and retains failed candidate for retry |
| Runtime close | TS ordered handle/content/root/boundary cleanup → host disposal | Explicit `exit()` path also restores terminal | Cleanup aggregates errors | Closed runtime rejects future operations; close attempts all cleanup domains |

### 5.2 Structural cold and recovery routes

#### Hint miss versus true cold node

A semantic NativeRef hint miss is not itself a failure:

- same-generation hint hit is the cheapest route;
- a stale-generation hint is ignored;
- if the node existed before the last boundary commit, NodeId promotion may recover it;
- if promotion misses, the node is treated as cold and direct materialization proceeds;
- if the node is newer than the boundary ceiling, the NodeId probe is intentionally skipped and direct materialization begins immediately.

This is the central distinction between an optimization miss and a semantic failure.

#### Stale reference recovery

`MaterializeTx.staleRefRetries` permits one targeted stale recovery per root transaction. `recoverStaleNode()`:

1. refuses a second recovery;
2. increments `stale_ref_retries`;
3. deletes the stale semantic hint;
4. removes the transaction-local ref;
5. invokes `ensureSemanticNative()` again.

`materializeWithRecovery()` then retries the same materializer once if the native status contains a stale child ordinal. A second native failure becomes `RetainedRefusalError("native constructor retry reported a failure status")`.

No stale reference selects a previous-generation transport.

#### Derivation-base fallback

For derivation nodes, a base hint or pre-existing NodeId promotion is required for the derivation primitive. If unavailable:

```text
derivation fast path unavailable
        ↓
direct semantic materialization of the derived node
```

This direct route may inspect/rebuild the semantic subtree. It remains within the retained architecture.

#### Root exact fast path

`renderExactRoot()` has a separate host cache-miss recovery:

1. If a generation-valid root hint exists:
   - call `hostRenderRef()`;
   - status `HOST_STATUS_OK` returns success with no payload walk.
2. If status is `HOST_STATUS_CACHE_MISS`:
   - delete the hint;
   - promote by NodeId;
   - install refreshed hint;
   - retry `hostRenderRef()` exactly once.
3. If promotion fails or retry misses:
   - return `{ status: "no_root_ref" }`.
4. Any other status throws.

The helper is well specified, but as noted above, the repository-wide production call graph does not demonstrate that `Tui.render()` currently invokes this route before `prepareDesiredInstall()`.

### 5.3 Native operation families

The structural ABI has multiple constructor families, but they are performance specializations of one route rather than architecture alternatives:

- fixed scalar row/column constructors for 0–4 children;
- axis builder for larger child counts;
- packed axis buffer for direct materialization;
- packed grid words;
- fixed-arity text constructors for ≤4 spans;
- cstring and exact-byte UTF-8 text lanes;
- variadic text buffer for >4 spans;
- packed diff words plus bytes;
- typed edit transactions;
- path-based edit helpers.

The thresholds are explicit:

- `NATIVE_SMALL_AXIS_ARITY_MAX = 4`;
- `NATIVE_BUILDER_MAX_CHILDREN = 524_288`;
- direct text limit `MAX_DIRECT_TEXT_BYTES = 16 MiB`;
- edit transaction cap in the TypeScript structural helper: 256 edits.

The source comments state that exceeding true native limits fails explicitly rather than selecting another architecture.

### 5.4 Content projection fallbacks

The Rust content provider has several legitimate fallback levels:

1. `prepared_paint_cache` hit.
2. Reusable paint product with same semantic key/width/requirements.
3. `semantic_cache` hit.
4. Semantic projection rebuild.
5. Full layout and paint compilation.
6. For non-History immediate content, physical rows may be deferred.
7. For smoothing or History, physical rows are retained.
8. For finalized History prefixes, a separate prefix proof/product is used.

Projection cache keys distinguish:

- Source identity/generation/revision;
- content generation;
- width;
- wrapping;
- Funnel kind;
- delivery revision;
- theme revision;
- finalized-prefix requirement;
- physical-row requirement.

Source: `application/content.rs` lines 210–324 and 900–1168.

### 5.5 Content selection and visible rollback

`selected_connector_id()` lines 3894–3920 chooses:

1. candidate selection if a candidate is active;
2. otherwise desired connector when the port is mounted;
3. but skips desired connector if it has an error and is not visible;
4. otherwise visible connector;
5. no connector if the port is not mounted.

This is not a transport fallback. It is a transactional visible-association policy.

During measurement, if a desired connector activation or projection fails:

- the failure is recorded;
- the previously visible connector is used for rollback where available;
- rollback first attempts a fresh preparation;
- if that fails, `projection_measurement()` is consulted;
- if no measurement exists, `ContentMeasurement::default()` is returned.

The final default is a potentially masking branch: missing/failed content can become zero/default measurement while the error remains recorded in Connector status.

### 5.6 Potentially silent branches

Several production branches return a default/empty result rather than an error. These require careful distinction between intentional absence and masked failure:

- `measure_content()` returns `ContentMeasurement::default()` when the port is missing or its lock cannot be acquired.
- `projection_for_ticket()` returns `None` if connector, ticket identity, or projection cannot be found.
- `paint_window_direct()` returns without painting when `projection_for_ticket()` returns `None`.
- `native_history_rows()` returns an empty vector if the host lock is unavailable and returns no rows for real backend mode.
- Some `ContentHostRegistry` query paths use `.ok()?` or default values around lock/registry absence.
- `selected_connector_id()` returns `None` for an unmounted or absent port.
- `tryRetained*` helpers return `undefined` for expected validation/native status failures, and callers generally convert that to explicit operation errors; these are not silent at public control boundaries.

The most consequential possible masking branch is the ticket-paint route: a missing prepared projection causes no paint call, with no direct error from `paint_window_direct()`. This may be intentional for a stale ticket, but source does not expose a route counter or diagnostic at that point.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Structural caches and scratch

`retained-dag.ts` owns:

- `SEMANTIC_NATIVE`: weak semantic-node → generation/ref hint;
- `AXIS_REF_SCRATCH`: one reusable axis scratch per active materialization depth;
- `GRID_WORD_SCRATCH`: reusable grid words per depth;
- `BYTE_SCRATCH`: reusable text/diff byte array;
- `STYLE_REF_CACHE`: generation-scoped style object and atom sidecars;
- `PATH_REFS`: WeakMap lineage refs;
- `PATH_SHAPE_REFS`: shape-level path refs.

The sidecars are explicitly non-authoritative acceleration state. Native style and semantic tables remain authoritative.

Structural counters include:

- `retained_hint_hits`;
- `retained_hint_misses`;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast-path calls;
- ref words written;
- byte payload bytes;
- scratch reuse;
- stale-ref retries;
- decorated normalized nodes;
- host mutations.

These are exposed by internal counter snapshot helpers and are relevant to benchmark route integrity.

### 6.2 Content caches

`application/content.rs` uses bounded per-Connector caches:

- `CONTENT_CACHE_CAPACITY = 2`;
- `CONTENT_PREFIX_CACHE_CAPACITY = 2`.

Per Connector:

- `projection_cache`;
- `prepared_paint_cache`;
- `semantic_cache`;
- `prefix_proof_cache`;
- `candidate_projection`;
- `committed_projection`.

Inactive connectors clear parser/delivery/projection state. Source remains authoritative.

Delivery ticks:

- advance smoother state;
- update delivery frontier/revision;
- clear `candidate_projection`;
- retain semantic/projection/paint caches;
- do not reparse on pure ticks.

The source comments at lines 358–410 and 3679–3680 explicitly identify this distinction.

### 6.3 Native content cache and invalidation

Native content Source storage uses:

- Source ID;
- Source generation;
- source revision;
- immutable chunk/page storage;
- annotation records;
- retention policy;
- subscriber groups.

Source replacement creates a fresh content generation while existing snapshots retain old immutable storage.

Source mutation wakes subscribed hosts. The TypeScript FFI decodes the mutation result and calls `requestWake()` only when the native result flag contains `CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN`.

### 6.4 Frame scheduling

TypeScript scheduling:

- retained state writes schedule a microtask;
- `Tui.flush()` drains retained execution first and host registration second;
- the environment wake broker batches pending hosts;
- automatic flushes may leave retry-blocked hosts blocked;
- explicit barriers can force one retry and surface errors.

Rust scheduling:

- source/control wakes enter the environment pending queue;
- smooth Connectors are indexed by active deadlines;
- inactive/unmounted/non-smooth Connectors are removed from deadline indexes;
- `next_wake_ms()` exposes the next native wake interval.

A Connector with smooth delivery may fall back to:

1. smoother-provided next wake;
2. authoritative host clock;
3. wall-clock `Instant::now()` if no host clock exists.

This is source-backed at `application/content.rs` lines 3715–3766. The wall-clock fallback is relevant to deterministic behavior: it is only used when neither explicit nor previously authoritative host time is available.

### 6.5 Performance specialization versus route duplication

The following are legitimate specializations:

- fixed-arity versus variadic structural constructors;
- derivation patch versus direct semantic rebuild;
- semantic projection cache versus rebuild;
- physical-row retention versus deferred immediate-mode rows;
- headless sink versus real terminal backend;
- NativeFrontier physical rows versus semantic resident rows;
- direct FFI bulk Source mutation versus N-API control calls;
- native async output wait versus signal-aware JS polling.

The source does not show a previous-generation complete-object structural route selected on refusal.

---

## 7. Tests, benchmarks and observability

### 7.1 Structural route tests

Relevant existing test evidence includes:

- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - stale child status precedence;
  - semantic cache reuse;
  - lease ownership;
  - failed host install retaining old root;
  - failed transaction releasing temporary leases;
  - stale weak slot cache miss;
  - repeated NodeId lookups;
  - path refs and depth specialization;
  - stale path base recovery;
  - edit transactions;
  - malformed buffers;
  - text cstring/UTF-8 lanes;
  - diff/decorated validation.
- `packages/iyon-tui/tests/tui_generated_view_abi.test.ts`
  - N-API session metadata;
  - opaque handle stability;
  - host render of existing NativeRef;
  - spacer construction/publication/release;
  - text layout patch parity.
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
  - fixed-arity and builder axis routes;
  - parity against reference host.
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
  - shared changed root across multiple text edits.
- `packages/iyon-tui/tests/tui_t14_fuzz_property.test.ts`
  - malformed boundary inputs;
  - no partial host mutation;
  - cache hit avoiding payload reads.
- `packages/iyon-tui/tests/tui_native_strings.test.ts`
  - Unicode;
  - embedded NUL;
  - styled spans;
  - native style atoms.
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
  - desired structural publication versus visible frame commit.

No tests were run during this assignment.

### 7.2 Runtime/frame tests

`crates/iyon-tui/src/application/host.rs` includes behavioral tests for:

- desired revision waiting for a successful frame barrier;
- failed frame retaining old visible state;
- explicit retry recovery;
- presentation-only repaint without measurement/semantic republication;
- structural publication invalidating retained state dependency paths.

The source test at lines 2465–2480 demonstrates the intended failure contract:

- old rows remain visible after failed preparation;
- desired revision advances;
- visible frame revision does not;
- pending and committed epochs diverge;
- later explicit flush can recover.

### 7.3 Content observability

Connector status contains:

- `phase`;
- `requested`;
- `visible`;
- `projected_source_revision`;
- `error`;
- `cleanup_pending`;
- `cleanup_error`.

The source distinguishes:

- projection failure;
- activation/operating failure;
- post-promotion Source cleanup failure.

This is valuable because a visible logical frame can succeed while cleanup remains pending.

Content traces exist in `history/trace.rs` and are controlled by `IYON_HISTORY_TRACE=1`. Trace functions include:

- projection;
- native transfer;
- pressure resolve.

The route audit did not execute trace logging.

### 7.4 Missing route visibility

There are counters for structural identity/materialization routes, but the following routes have weaker visibility:

- `paint_window_direct()` projection ticket miss;
- default measurement fallback from missing/poisoned content state;
- exact-root route reachability from public `Tui.render()`;
- direct FFI versus N-API bulk Source route at runtime;
- whether all production callers exercise `renderExactRoot()`.

The parent should preserve these as open observability gaps rather than infer that they are unused or unreachable solely from tests.

---

## 8. Cross-boundary findings and contradictions

### 8.1 The retained structural path is authoritative, but the final renderer remains the Rust View pipeline

The TypeScript structural transport is explicitly documented as the single retained production architecture:

- `retained-dag.ts` lines 19–22;
- `native-view-abi.ts` lines 151–157;
- `runtime.ts` lines 217–222.

However, native ABI materializers in `crates/iyon-tui-native/src/tui/view_abi.rs` build Rust `View` values through functions such as:

- `text_view_from_spans()`;
- `text_view_from_owned()`;
- `view_native_patched()`;
- `view_native_axis_from_children()`;
- `view_native_grid_final()`;
- `view_native_component()`.

The resulting Rust `View` is then passed through `TuiHost`/`HostRunning`/`Scene`. Therefore:

- there is no complete-object secondary transport fallback;
- but the current runtime still depends on the old/core Rust View, Scene, layout, and paint implementation as the final realization route.

This is not necessarily incorrect; it is the actual current seam and should not be described as a wholly independent renderer.

### 8.2 Exact-root route implementation versus public reachability

`runtime.ts` documents:

```text
warm root hint → exact-root fast path
```

`retained-dag.ts` implements `renderExactRoot()` and `RetainedRootBoundary.renderExact()` with one stale retry.

But repository-wide source search found no production caller outside the boundary method itself. The public `Tui.render()` path uses:

```text
prepareRootPublication()
  → RetainedRootBoundary.prepareDesiredInstall()
  → prepareFrom()
  → ensureSemanticNative()
  → publishDesiredPrepared()
```

This means the exact-root route is either:

- reserved for a caller not present in the tracked production source;
- retained for future/qualification use;
- or a route-documentation mismatch.

This is a consequential production-route finding. It should not be silently reconciled by assuming the intended path is active.

### 8.3 Direct FFI is a legitimate lane alternate, not a structural fallback

There are two native transport styles:

- generated N-API calls for structural View/state/control operations;
- Bun direct FFI for bulk Source mutation operations.

`packages/iyon-tui/src/transport/content/ffi.ts` opens the native artifact using `dlopen()` and binds:

- `iyon_tui_source_append_utf8_v1`;
- `iyon_tui_source_replace_utf8_v1`;
- `iyon_tui_source_clear_v1`;
- `iyon_tui_source_seal_v1`;
- `iyon_tui_source_head_truncate_v1`.

The direct FFI session is cached per runtime environment. There is no code path that catches direct FFI failure and retries through N-API.

Conversely, `direct-ffi` feature exports in Rust are qualification-only. `packages/iyon-tui/scripts/stage-native.ts` validates:

- direct symbols exist only when the feature is requested;
- default artifacts expose no direct qualification symbols;
- the default artifact still exposes `tuiViewAbiSession`.

Thus direct FFI is lane-specific transport specialization, not a hidden structural fallback.

### 8.4 Hidden native-host Rust binding API remains compiled

`crates/iyon-tui/src/binding/mod.rs` contains a hidden native-host binding module exposing functions such as:

- `view_native_component`;
- `view_native_content_host`;
- `view_native_text_final`;
- `view_native_grid_final`;
- `view_native_axis_from_children`;
- `view_native_axis_set_child`;
- `view_native_axis_splice`;
- `view_native_grid_set_cell`;
- `view_native_replace_at_path`;
- `view_native_patched`;
- state attachment helpers;
- weak View helpers;
- retained path helpers.

These APIs are `#[doc(hidden)]` and are not the public TypeScript authoring surface. They are still used by native ABI implementations to construct Rust Views. They should be classified as hidden bridge/runtime compatibility machinery, not as an externally consumed Rust UI authoring API.

### 8.5 Content fallback can preserve visible state while reporting failure

Content switching intentionally separates desired and visible Connector state. A failed desired activation/projection can:

- record an error;
- keep the old Connector visible;
- return old visible measurement/rows;
- leave the desired Connector requested for a later retry.

This is legitimate recovery behavior. It is not equivalent to silently accepting the new Connector.

However, in the absence of a visible old projection, measurement fallback can return default measurement. That branch is less explicit and should be distinguished from the visible rollback route.

### 8.6 Frame failure and structural publication failure are different failure planes

Structural desired publication can commit before the host frame is presented. The frame then may fail in:

- scene preparation;
- content projection;
- state resolution;
- terminal backend render/flush.

The source deliberately preserves:

- desired structural revision;
- old visible frame;
- candidate content/state/frame records.

This creates a two-stage failure model:

```text
structural publication failure
    → desired root not accepted; old root remains

frame preparation/presentation failure
    → desired root accepted; old visible frame remains; retry pending
```

This distinction is central to interpreting route counters and test outcomes.

---

## 9. Open questions and coverage gaps

1. **Is `RetainedRootBoundary.renderExact()` intended to be production-reachable?**
   - The helper is implemented and tested conceptually, but no external production caller was found.
   - The public `Tui.render()` route currently appears to use deferred `prepareDesiredInstall()` directly.

2. **Does `paint_window_direct()` silently hide stale-ticket errors by design?**
   - A missing projection ticket returns without painting.
   - No explicit counter or runtime error is emitted at that call site.

3. **Are default content measurements on missing/poisoned records acceptable?**
   - `measure_content()` may return `ContentMeasurement::default()`.
   - Connector errors may be recorded separately, but a caller consuming only layout output can see a zero/default measurement.

4. **What is the intended status of hidden Rust `binding` View APIs?**
   - They are not public authoring APIs, but native ABI implementations depend on them.
   - They are therefore not unused merely because TypeScript does not import them.

5. **Is the existing Rust View/Scene pipeline an intentional final realization layer or migration residue?**
   - Current source proves it remains operational.
   - This report does not make a V5 disposition decision.

6. **Are all NativeRef cache misses observable in production?**
   - Structural counters exist, but no integrated route report is emitted for every refusal/fallback.
   - Exact-root route reachability remains uncertain.

7. **How often does direct FFI bulk content transport fail in production relative to N-API control transport?**
   - Source clearly shows separate lanes but no production runtime fallback or unified route counter.

8. **Do all default native artifacts expose exactly the N-API surface expected by the TS ABI manifest?**
   - Startup metadata validation enforces this when loaded.
   - No artifact-loading execution was performed here.

9. **Are content parser/projection failures always retained in status until a Source revision changes?**
   - `failed_source_revision` and `projection_failure_key` indicate this policy.
   - The exact clearing behavior across every Connector lifecycle transition deserves a dedicated source-check if needed.

10. **Can stale content tickets arise through production frame concurrency rather than only tests?**
    - The source explicitly supports newer Source revisions, delivery ticks, Connector switches, and theme changes while an older ticket is in flight.
    - `projection_for_ticket()` intentionally pins the prepared product, but missing-ticket behavior remains weakly observable.

---

## 10. Evidence appendix

### 10.1 Required documents

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - mandatory headings and source-discipline requirements;
  - current source authoritative for execution;
  - no unsupported V5 disposition decisions.
- `docs/architecture/atlas-4355c02/README.md`
  - assignment 41 scope;
  - atlas baseline and ownership.
- `PRE-V5-ARCHITECTURE-REPORT.md`
  - semantic operation/path matrix requirements;
  - alternate/fallback/residue audit additions;
  - no migration analysis performed here.
- `AGENTS.md`
  - generic TUI framework boundary;
  - no silent exception swallowing;
  - no speculative fallback;
  - Rust owns generic terminal/runtime mechanics.

### 10.2 TypeScript route evidence

#### Runtime

- `packages/iyon-tui/src/runtime/runtime.ts`
  - lines 59–85: public runtime API.
  - lines 108–141: Tui ownership fields.
  - lines 159–196: shared retained runtime and component projection.
  - lines 197–214: runtime access/input/readback wiring.
  - lines 217–277: retained root publication target.
  - lines 280–317: History attach-once semantics.
  - lines 319–342: native host construction and cleanup.
  - lines 347–370: no-signal native wait vs signal-aware JS polling.
  - lines 373–389: documented scene route.
  - lines 391–405: direct versus canonical producer selection.
  - lines 407–421: explicit flush barrier.
  - lines 451–497: canonical producer lifecycle.
  - lines 499–549: direct scene lifecycle.
  - lines 551–560: deferred `RetainedRootBoundary` construction.
  - lines 717–728: transactional resize.
  - lines 770–778: visible commit callback.
  - lines 798–815: cleanup ordering.

#### Semantic View

- `packages/iyon-tui/src/api/view/view.ts`
  - lines 226–246: fresh semantic identity/update construction.
  - lines 249 onward: View class.
  - lines 299–323: text/spacer constructor routing.
  - lines 360–362: grid constructor routing.
  - lines 493–508: decoration derivation.
  - lines 511–554: text layout derivation.
  - lines 561–612: semantic rewrapping and attachments.
  - lines 615 onward: raw grid and structural update helpers.
  - lines 1021–1027: NodeId high-water.

- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - lines 1–11: semantic model deliberately independent of transport.
  - lines 164–186: semantic kinds and node identity.
  - lines 371–390: branding/freezing.

#### Retained structural transport

- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - lines 1–23: single retained production structural architecture.
  - lines 52–103: hints, scratch, style sidecars.
  - lines 111–145: structural counters.
  - lines 185–229: refusal/cycle/transaction state.
  - lines 235–311: reusable scratch and explicit size refusal.
  - lines 319–339: lease draining.
  - lines 344–374: native status/cache-miss decoding.
  - lines 415–467: one stale-node recovery and materializer retry.
  - lines 475–620: spacer/content/axis/grid materializers.
  - lines 623–797: text/style/cstring/UTF-8/variadic text lanes.
  - lines 808 onward: diff materialization.
  - lines 871–976: hanging/container/clamp/component/decorated materializers.
  - lines 1109–1120: semantic-kind materializer dispatch table.
  - lines 1149–1219: identity-first `ensureSemanticNative`.
  - lines 1222–1364: derivation-base lookup and derivation fast paths.
  - lines 1372–1463: exact-root host route and one stale retry.
  - lines 1489–1507: root lease protocol.
  - lines 1510–1548: prepared publication contract.
  - lines 1572–1600: deferred/direct root-boundary options.
  - lines 1602–1641: root adoption.
  - lines 1644–1703: direct prepared install.
  - lines 1705–1735: deferred desired install.
  - lines 1738–1788: visible promotion.
  - lines 1796–1916: preparation and NodeId recovery.
  - lines 1919–1942: desired root publication.
  - lines 1945–2019: direct host/install publication and unwind.
  - lines 2021–2045: `renderExact()` wrapper.
  - lines 2048–2110: close, transfer, and superseded desired root release.

- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - lines 57–128: N-API session and metadata validation.
  - lines 131–187: NativeRef lookup and transient materialization.
  - lines 190–310: small-axis and builder routes.
  - lines 312–400: axis set/splice routes.
  - lines 402–435: grid set route.
  - lines 438–510: edit transaction route.
  - lines 512–550: path ref interning.
  - lines 588–629: host resolution/install and NativeRef validation.

- `packages/iyon-tui/src/transport/structural/retained-path.ts`
  - lines 69–108: semantic path patch construction.
  - lines 109–169: lineage metadata.
  - lines 185–236: path validation/semantic traversal.
  - lines 245–286: path child lookup and failure branches.

#### Slot and pane boundaries

- `packages/iyon-tui/src/api/controls/view-slot.ts`
  - lines 30–45: initial retained materialization and lease release.
  - lines 86–98: ownership modes.
  - lines 124–145: boundary creation/adoption.
  - lines 157–172: direct versus builder mode.
  - lines 204–236: direct publication and builder disposal ordering.
  - lines 238–275: prepared direct publication.
  - lines 300–334: animation ref routes and failure preservation.
  - lines 365–379: stop-animation materialization.
  - lines 382–402: disposal order.

- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - lines 47–52: initial materialization failure.
  - lines 90–111: direct boundary callback.
  - lines 147–201: prepared/update publication and explicit failure.

#### Content TypeScript routes

- `packages/iyon-tui/src/api/content/retained.ts`
  - lines 37–165: Source/Funnel/Connector public contracts.
  - lines 176–204: Source option validation.
  - lines 276–334: snapshot/stats identity decoding.
  - lines 337–407: Stream Source mutation APIs.
  - lines 410–468: Block Source mutation APIs.
  - lines 471–559: Funnel normalization.
  - lines 561–650: ContentPort connect/deactivate/lifecycle.
  - lines 661–785: Connector activate/deactivate/status/disposal.
  - lines 800–810: internal ContentPort construction.

- `packages/iyon-tui/src/transport/content/control.ts`
  - lines 1–6: control/data-plane separation.
  - lines 36–40: Source constructor.
  - lines 43–55: Port constructor/deactivation/mount.
  - lines 58–91: connect/activate/deactivate/dispose.
  - lines 94–98: status.

- `packages/iyon-tui/src/transport/content/ffi.ts`
  - lines 28–32: payload limits.
  - lines 74–124: direct FFI symbols.
  - lines 127–149: session/error types.
  - lines 190–218: mutation status decoding and wake scheduling.
  - lines 220–278: metadata validation and `dlopen`.
  - lines 281–309: per-environment FFI session/source identity caching.
  - lines 578–650: annotation and UTF-8 encoding.
  - lines 657–733: append/replace/no-payload invocation.
  - lines 736–778: public bulk mutation helpers.

#### State transport

- `packages/iyon-tui/src/transport/state/control.ts`
  - lines 36–76: geometry patch normalization.
  - lines 79–167: geometry clear/alignment/border validation.
  - lines 176–211: presentation patch normalization.
  - lines 214–275: presentation clear and nested value validation.
  - lines 333–359: envelope/clear-mask construction.

- `crates/iyon-tui-native/src/tui/view_state.rs`
  - lines 1–8: state wrapper responsibility boundary.
  - lines 25–66: lifecycle, identity, node-kind validation.
  - lines 68–104: geometry envelope set/clear.
  - lines 106–162: presentation and dynamic style-state operations.
  - lines 173–180: wake bit encoding.
  - lines 182 onward: envelope decoding and validation.

#### Native addon surface

- `packages/iyon-tui/src/transport/native/addon.ts`
  - lines 18–26: History contract.
  - lines 28–46: state contract.
  - lines 48–97: input/content contracts.
  - lines 99–123: slot/pane contracts.
  - lines 125–189: host contract.
  - lines 191–234: addon and optional constructor surface.
  - lines 236–242: canonical artifact loading/build identity.
  - lines 244–248: `requireNativeClass()` hard failure.

### 10.3 Rust route evidence

#### Native View ABI

- `crates/iyon-tui-native/src/tui.rs`
  - lines 42–146: host/content environment registries and cleanup.
  - lines 149–158: smoke probe.
  - lines 265–296: NativeRef environment/count and View resolution.
  - lines 603–607: `NativeTuiHost`.
  - lines 655–672: `setDesiredViewRef`.
  - lines 683–710: `flushPendingHosts`.
  - lines 776–805: theme and attach-once History.
  - lines 840–862: ViewState and ContentPort.
  - lines 1030–1040: direct-FFI qualification host pointer.

- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - lines 358–397: NativeViewRuntime.
  - lines 399 onward: NativeViewRuntime state.
  - lines 1327–1457: environment-scoped runtime handles/session.
  - lines 1459–1468: edit transaction cleanup.
  - lines 1470–1669: qualification/maintenance/memory diagnostics.
  - lines 1680–1776: NativeRef resolution, exact host render, status recording.
  - lines 1780–1851: state attachment and NodeId lookup.
  - lines 1855–2284: paths, text patches, edit transactions.
  - lines 2287–2390: content host/spacer/text/common patch.
  - lines 2507–2653: axis child resolution and axis creation.
  - lines 2768–2911: axis builder/set/splice/grid routes.
  - lines 3059–3260: grid/diff buffer routes.
  - lines 3263–3776: path/decorated/hanging/container/clamp/component/release routes.
  - lines 3946 onward: text decoding and publication.
  - tests beginning around lines 4603 onward provide NativeRef/cache/lease/path/transaction/validation evidence.

#### Rust host/frame

- `crates/iyon-tui/src/application/host.rs`
  - lines 102–120: headless and real backend variants.
  - lines 122–137: authoritative frame/candidate frame/receipt state.
  - lines 973–998: headless vs real host construction.
  - lines 1091–1102: desired View acceptance.
  - lines 1199–1228: final render/restore route.
  - lines 1320–1408: render/theme/input/paste/resize/time operations.
  - lines 1474–1507: input pump.
  - lines 1671–1679: environment flush.
  - lines 1870–1959: candidate render/preparation path.
  - lines 1961–2023: presentation receipt.
  - lines 2026–2184: frame commit/retry candidate.
  - lines 2186–2240: polling/blocking receipt recovery.
  - lines 2242–2355: pending-frame routing.
  - lines 2365–2410: headless/real `prepare_frame_with_content`.
  - tests around lines 2441 onward: frame barrier/failure/retry behavior.

#### Rust content

- `crates/iyon-tui/src/application/content.rs`
  - lines 208–266: projection keys and cache capacities.
  - lines 269–324: semantic cache.
  - lines 328–410: projection/delivery products.
  - lines 611–730: Source and semantic projections.
  - lines 746–797: full compile versus geometry-only layout.
  - lines 900–1168: paint products and finalized-prefix routes.
  - lines 1472–1521: Funnel representation.
  - lines 2951–3016: Port/Connector records, errors, and cache state.
  - lines 3215–3276: ContentHostRegistry ownership/indexing.
  - lines 3513 onward: desired Port state.
  - lines 3586–3688: smooth delivery tick route.
  - lines 3715–3773: deadline/cold connector handling.
  - lines 3894–3920: desired/visible Connector selection.
  - lines 3934–4088: projection preparation/cache hit/rebuild.
  - lines 4091–4170: projection measurement/failure recording.
  - lines 4173–4243: width-fit rekey/recompute.
  - lines 4261–4353: content measurement and rollback.
  - lines 4355–4543: ticket-pinned paint and lookup.
  - lines 4545–4572: candidate/cache/committed projection selection.
  - lines 4967–5181: activation candidate/abort/promotion/source cleanup.
  - lines 5537–6019: Connector activation/deactivation/disposal/subscription.
  - lines 6242–6265: History content rows.
  - lines 6572–6645: ContentProvider interface implementation.

#### Rust History and native frontier

- `crates/iyon-tui/src/history/model.rs`
  - lines 24–40: semantic History/native frontier ownership.
  - lines 72–128: push/discard/freeze lifecycle.
  - lines 145–242: state/content attachment view extraction.
  - lines 300–395: layout caches/invalidation.
- `crates/iyon-tui/src/history/projection/mod.rs`
  - lines 135–155: host content-aware projection.
  - lines 157–226: static/live unit planning and cache keys.
  - lines 409–524: native frontier versus semantic resident projection.
  - lines 595–710 and 717–875: cached/non-cached selection routes.
  - lines 990–1097: height and unit-view fallback logic.
- `crates/iyon-tui/src/history/native/frontier.rs`
  - lines 55–93: NativeFrontier and synchronization-unknown state.
- `crates/iyon-tui/src/history/native/mod.rs`
  - lines 88–113: synchronization failure handling.
  - lines 147 onward: spacing transfer and physical row transfer.

### 10.4 Generated/qualification route evidence

- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json`
- `packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
- `crates/iyon-tui-native/src/generated/view_state_schema.rs`
- `tools/tui-abi/view_abi.toml`
- `tools/tui-abi-gen/*`
- `packages/iyon-tui/scripts/stage-native.ts`
  - lines 91–124: direct-FFI symbol gating and default N-API surface checks.

### 10.5 Source manifest evidence

- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
  - lines 21–34: native addon source/generated inventory.
  - lines 40–225: Rust source inventory.
  - lines 922–990: TypeScript source, generated ABI, content/native/state/structural transport inventory.

### 10.6 Files indexed but not comprehensively route-read

The full source manifest was indexed. The following areas were not independently reconstructed file-by-file because their route behavior was either covered by the route-owning modules above or is outside the central semantic-operation matrix:

- every individual Rust text/document fixture;
- every presentation layout leaf implementation;
- all generated ABI bodies beyond the symbols required to establish the route;
- all benchmark JSONL/replay data;
- all terminal escape-generation implementation details beyond host/backend selection and receipt behavior;
- all test-only fixtures except where their names/assertions establish route behavior.

No absence claim in this report means “the subsystem is unused everywhere”; it means no additional production route was found within the searched manifest and route call graph.