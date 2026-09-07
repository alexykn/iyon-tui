# 22 — TypeScript structure-state transport

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Investigation mode: read-only static source inspection.
- No repository files, source, configuration, generated artifacts, or tests were modified.
- No test suite or benchmark was executed during this investigation. Test files were read as behavioral evidence only.

The required architecture documents were read first:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The historical PRE-V5 document was used only for context and evidence expectations. This report describes the current implementation and does not make V5 disposition decisions.

### Primary scope

The primary scope is:

```text
packages/iyon-tui/src/transport/structural/
packages/iyon-tui/src/transport/state/
packages/iyon-tui/src/transport/abi/
```

Generated bodies under `transport/abi/**/generated` were not treated as primary implementation scope. Generated schemas, manifests, and wrapper signatures were indexed where necessary to describe the wire contract.

### Closely-followed seams

The transport code cannot be understood independently of the following source seams, which were inspected as supporting evidence:

```text
packages/iyon-tui/src/api/view/semantic-node.ts
packages/iyon-tui/src/api/view/view.ts
packages/iyon-tui/src/api/view/retained-state.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/runtime/attachments.ts
packages/iyon-tui/src/runtime/native-resource-registry.ts
packages/iyon-tui/src/transport/native/addon.ts
packages/iyon-tui/src/transport/native/resource-registry.ts
```

The native state and retained-runtime implementation was also inspected to prove cross-boundary behavior:

```text
crates/iyon-tui/src/retained_state/
crates/iyon-tui/src/application/view_state.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui-native/src/tui/view_state.rs
crates/iyon-tui-native/src/tui/view_abi.rs
```

### Facts, inferences and unknowns

- **Current-source fact:** TypeScript transport materializes immutable semantic View nodes directly into native retained View references. There is no production complete-object decode route in the inspected TypeScript path.
- **Current-source fact:** NativeRef hints are weak JavaScript acceleration metadata, not ownership records. Native leases are explicitly managed by root boundaries and temporary materialization transactions.
- **Current-source fact:** ViewState mutations are accepted by the native retained-state record independently of structural View reconstruction. They invalidate native state/layout/paint work and schedule an environment drain when the state is bound.
- **Current-source fact:** A structural candidate is prepared before publication. In deferred host mode, structural desired state is published before a later frame barrier promotes it to visible state.
- **Inference:** The implementation has three distinct notions of retention: semantic View identity in TypeScript, accepted native View structure in the native ABI runtime, and native host-owned state records. These are deliberately related by NodeId/HandleId but are not the same cache or lifetime.
- **Uncertainty requiring parent/source-owner attention:** `crates/iyon-tui/src/application/view_state.rs::semantic_state_node_kind` maps numeric kind `1` to `StateNodeKind::Column`, while TypeScript `SEMANTIC_VIEW_KIND.diff` is also numeric `1`. The native Rust `ViewKind` enum has no visible `Diff` variant. This may be intentional because diff is lowered to an existing native presentation kind, but the TypeScript-to-native kind contract is not self-evident and deserves explicit reconciliation, especially for geometry capability validation on state-attached diff nodes.

---

## 1. Responsibility and structure

### 1.1 Module inventory

Approximate physical LOC are based on the highest source line inspected in each file, including comments, imports, blank lines, and declarations. Generated bodies are excluded from the primary total.

| Path | Language | Approx. production LOC | Approx. test LOC | Public API? | Primary responsibility | Plane |
|---|---:|---:|---:|---|---|---|
| `transport/structural/component-id.ts` | TypeScript | 27 | 0 | Internal export | Resolve a framework `HandleId` to a native component identity | Structural |
| `transport/structural/encoding.ts` | TypeScript | 343 | 0 | Internal exports | Convert semantic values to compact ABI numeric lanes and packed words | Structural/ABI |
| `transport/structural/ir.ts` | TypeScript | 130 | 0 | Internal exports | Import generated kind codes and define transport-side semantic/structural records | Structural |
| `transport/structural/native-view-abi.ts` | TypeScript | 629 | 0 | Internal exports | Session bootstrap, direct retained ABI calls, native-ref installation, path references, release | Structural/ABI |
| `transport/structural/policy.ts` | TypeScript | 38 | 0 | Internal constants | Native arity/payload thresholds and retained-path policy | Structural |
| `transport/structural/retained-dag.ts` | TypeScript | 2,140 | 0 | Internal exports | Semantic DAG materialization, NativeRef hints, leases, derivation fast paths, root transaction boundary | Structural/state seam |
| `transport/structural/retained-path.ts` | TypeScript | 303 | 0 | Internal exports | Immutable path lineage and multi-text-edit metadata | Structural |
| `transport/structural/style-lowering.ts` | TypeScript | 222 | 0 | Internal exports | Lower public style/theme values into semantic retained style records | Structural/presentation |
| `transport/state/control.ts` | TypeScript | 359 | 0 | Internal exports consumed by `ViewState` | Validate ViewState patches, normalize colors/styles/geometry, call generated state-envelope encoders | State/ABI |
| `transport/abi/structural/generated/view_calls.ts` | Generated TypeScript wrapper | Indexed, excluded | 0 | Internal generated API | Checked wrappers around native retained View functions | ABI |
| `transport/abi/structural/generated/view_abi.ts` | Generated TypeScript types | Indexed, excluded | 0 | Internal generated API | Opaque native runtime/session handle types | ABI |
| `transport/abi/structural/generated/view_abi_conformance.ts` | Generated TypeScript | Indexed, excluded | 0 | Internal generated API | Generated conformance metadata | ABI |
| `transport/abi/structural/generated/view_abi_manifest.json` | Generated JSON | Indexed, excluded | 0 | Schema metadata | ABI identity, function list, hashes, result encoding | ABI |
| `transport/abi/structural/schema/view-kind-codes.json` | Generated/schema JSON | 42 | 0 | Schema data | Numeric view/layout/alignment/diff discriminants | ABI |
| `transport/state/generated/state_envelope.ts` | Generated TypeScript | Indexed, excluded | 0 | Internal generated API | State masks, fixed word lanes, string lanes, envelope encoders | State/ABI |

The non-generated primary TypeScript transport implementation is approximately **4,191 physical lines** by the stated method.

### 1.2 Structural transport responsibilities

The structural transport owns:

1. ABI session acquisition and metadata verification.
2. Semantic kind-to-native-kind conversion.
3. Compact encoding of:
   - View kinds
   - Axis tracks
   - Grid tracks
   - Wrapping
   - Alignment
   - Diff line metadata
   - Overflow indicators
   - Styles and attributes
   - Decorations
   - u64 NodeId/state/component coordinates
4. Direct materialization of every semantic View kind.
5. Native retained reference hinting and promotion.
6. Temporary NativeRef lease accounting.
7. Root replacement transactions.
8. Structural derivation fast paths:
   - text layout patches
   - scalar geometry patches
   - axis child replacement
   - axis splice
   - grid cell replacement
9. Wide-path support through native persistent sequences.
10. Native path and edit transaction metadata.
11. Stale-reference recovery and explicit retained refusal behavior.

### 1.3 State transport responsibilities

`transport/state/control.ts` is intentionally a control-plane adapter rather than a state authority.

It owns:

- Runtime validation of public TypeScript ViewState patch objects.
- Conversion of public `ColorSpec`, `StyleSpec`, `StyleRef`, border, inset, alignment, and attribute forms into normalized transport records.
- Distinguishing:
  - omitted property
  - explicit `null`
  - explicit set value
  - clear operation
- Delegating fixed mask/word/string packing to generated state-envelope code.
- Exporting the wake bit used by `ViewState.mutate`.

It does **not** own:

- Mutable state records.
- State revisions.
- Effective geometry calculation.
- Effective presentation merging.
- Dirty worklists.
- Visible/in-flight binding.
- Effect classification.

Those responsibilities belong to the native Rust retained-state plane.

### 1.4 ABI responsibilities

The generated ABI wrappers expose a stable N-API-shaped object contract. The handwritten TypeScript transport owns:

- ABI metadata verification in `nativeViewAbiSession()`.
- Which generated operation is selected.
- How semantic objects become fixed arguments or typed buffers.
- How NativeRef ownership is acquired/released around each call.
- Which native statuses are recoverable cache misses versus retained refusals.
- Whether a host mutation is direct, deferred, or transactional.

---

## 2. Types, APIs and contracts

### 2.1 Semantic source model

`api/view/semantic-node.ts` explicitly defines the semantic model as backend-neutral:

- `SemanticViewNode`
- `SemanticNodeId`
- `SemanticColor`
- `SemanticStyle`
- `SemanticDecoration`
- `SemanticLayoutChild`
- `SemanticGridTrack`
- `SemanticGridCell`
- `SemanticDiffHunk`
- `SemanticDerivation`
- `SemanticSequence`
- `SemanticAxisSequenceOverride`
- `SemanticGridSequenceOverride`

The module states that it has no knowledge of structural transport, native handles, ABI schema, or generated calls (`semantic-node.ts:1-10`). This is an important ownership boundary: semantic declarations are produced before transport lowering.

Each semantic node has:

```ts
readonly id: SemanticNodeId;
readonly kind: SemanticViewKind;
readonly stateAttachment?: HandleId;
readonly contentAttachment?: HandleId;
```

`stateAttachment` and `contentAttachment` are opaque framework HandleIds in the semantic model. They are resolved only at the native boundary.

### 2.2 Semantic identity versus native identity

The identities are intentionally separate:

| Identity | Owner | Purpose | Lifetime |
|---|---|---|---|
| `SemanticNodeId` | TypeScript `View`/semantic factory | Immutable semantic node identity, derivation key, NodeId transport key | Until semantic node becomes unreachable |
| `HandleId` | TypeScript framework handle/resource registry | Identity of ViewState, ContentPort, component, and other resources | Monotonic/retired through resource registry |
| Native `NodeId` | Native retained View runtime | Native semantic cache key; encoded as low/high u32 | Native runtime generation |
| Native `NativeRef`/ViewRef | Native ABI runtime | Lease-bearing reference to native retained View | Explicit lease ownership |
| Native state ID | Native retained-state registry | Host-owned mutable state record identity | Host lifetime plus resource leases |
| Native component ID | Native component registry | Native component indirection identity | Component resource lifetime |

TypeScript never treats a NativeRef as a semantic identity. A NativeRef hint is only an acceleration result (`retained-dag.ts:52-59`, `1122-1128`).

### 2.3 Structural transport exports

The significant internal APIs are:

#### `native-view-abi.ts`

- `nativeViewAbiSession()`
- `nativeViewRefForNodeId(view)`
- `tryRetainedMaterializeRef(next)`
- `tryRetainedAxisCreate(...)`
- `tryRetainedAxisCreateRender(...)`
- `tryRetainedAxisSetChildRender(...)`
- `tryRetainedAxisSpliceRender(...)`
- `tryRetainedGridSetCellRender(...)`
- `tryRetainedEditTransactionRender(...)`
- `nativePathRefForLineage(...)`
- `releaseNativeViewRef(...)`

These APIs are transport helpers rather than public authoring surfaces.

#### `retained-dag.ts`

- `MaterializeTx`
- `ensureNative(...)`
- `ensureSemanticNative(...)`
- `renderExactRoot(...)`
- `acquireKnownRoot(...)`
- `RetainedRootBoundary`
- `RootPublication`
- `retainedIdentityCounterSnapshot()`
- `resetRetainedIdentityCounters()`
- `setRetainedPhaseInstrumentation(...)`
- `resetStyleRefCacheForThemeChange()`
- `RetainedRefusalError`
- `RetainedCycleError`

#### `retained-path.ts`

- `NativePathStep`
- `NativePathLineage`
- `NativeTextLayoutTransactionEdit`
- `textLayoutAtNativePathForTransport(...)`
- `textLayoutTransactionForTransport(...)`
- `nativePathLineage(...)`
- `nativePathChildLineage(...)`
- `attachNativePathLineage(...)`
- `nativeTextLayoutTransaction(...)`

#### `encoding.ts`

The module is the only handwritten TypeScript owner of compact numeric structural encodings:

- `nativeViewKind`
- `axisKind`
- `axisKindForHorizontal`
- `layoutTrackWord`
- `axisTrackWord`
- `gridTrackWord`
- `wrapModeCode`
- `horizontalAlignCode`
- `verticalAlignCode`
- `diffLineKindCode`
- `diffTerminationCode`
- `diffLineMetadata`
- `overflowKindCode`
- `sizeModeCode`
- `u64Words`
- `styleAttributeEncoding`
- `decorationWordEncoding`
- `gridCellSpanWord`
- `gridCellAlignmentWord`
- `commonScalarEncoding`
- `colorAtomValue`

### 2.4 State transport exports

`transport/state/control.ts` exports:

- `normalizeGeometryPatch`
- `normalizeClearGeometryProperties`
- `normalizePresentationPatch`
- `normalizeClearProperties`
- `geometryEnvelope`
- `geometryClearEnvelope`
- `presentationEnvelope`
- `presentationClearEnvelope`
- `StateClearEnvelope`
- generated `StateEnvelope`
- generated `STATE_WAKE_DRAIN`

The public `ViewState` API is in `api/view/retained-state.ts`, not in transport/state. `ViewState` calls the transport normalizers and then invokes the native resource methods:

```text
ViewState.setGeometry
  → geometryEnvelope
  → NativeViewStateResource.setGeometry

ViewState.setPresentation
  → presentationEnvelope
  → NativeViewStateResource.setPresentation

ViewState.clearGeometry
  → geometryClearEnvelope
  → NativeViewStateResource.clearGeometry

ViewState.clearPresentation
  → presentationClearEnvelope
  → NativeViewStateResource.clearPresentation
```

`ViewState.setStyleState` and `clearStyleState` validate non-empty text directly and call native typed operations.

### 2.5 Structural materialization contract

`ensureSemanticNative` defines the central ordering (`retained-dag.ts:1148-1219`):

```text
1. same-generation semantic NativeRef hint
2. transaction-local reference
3. NodeId → NativeRef promotion, but only when NodeId <= nativeLookupCeiling
4. derivation fast path
5. direct semantic payload inspection
6. child traversal/materialization
7. optional state attachment
8. hint installation and temporary lease registration
```

This ordering is consequential:

- A warm root does not inspect semantic payload.
- A previously native-published node without a JS hint can be recovered by NodeId promotion.
- Newly created NodeIds skip the extra NodeId probe.
- Derivations are attempted before full payload materialization.
- Child references are materialized before parent constructors.
- A state attachment is applied after the base native View exists.

### 2.6 Native kind mapping

`ir.ts` imports generated schema codes and exposes private native codes:

```text
semantic text          → native view code 1
semantic diff          → native view code 2
semantic spacer        → native view code 3
semantic row           → native view code 4
semantic column        → native view code 5
semantic hanging       → native view code 6
semantic grid          → native view code 7
semantic container     → native view code 8
semantic clamp         → native view code 9
semantic contentMax    → native view code 10
semantic component     → native view code 11
semantic decorated     → native view code 12
semantic contentHost   → native view code 13
```

`encoding.ts:nativeViewKind` owns this conversion (`encoding.ts:39-55`). The semantic numeric discriminants deliberately differ from native numeric codes.

### 2.7 State patch contract

Geometry properties:

```text
width
height
padding
minWidth
maxWidth
minHeight
maxHeight
gap
alignment
borderEdges
```

Presentation properties:

```text
foreground
background
borderColor
borderStyle
borderGlyphs
textAttributes
style
```

The generated state envelope uses:

- Geometry: 14 u32 words, no strings.
- Presentation: 5 u32 words, 14 strings.
- Geometry property IDs 0–9.
- Presentation property IDs 0–6.
- `setMask` identifies properties included in a set patch.
- `nullMask` distinguishes explicit nullable null values from ordinary values.
- `clearMask` identifies fields to clear.
- Omitted clear-property lists become `clearAll: true`.
- Explicit clear lists are deduplicated and validated before encoding.

The TypeScript normalizer rejects unknown fields before generated encoding. The Rust N-API decoder then checks masks, lane counts, and values again at the new trust boundary.

### 2.8 State attachment contract

`View.state(state)` adds `stateAttachment: state.id` to a fresh semantic node without changing topology (`api/view/view.ts:408-415`, `578-594`).

The semantic node also retains a strong sidecar reference to the ViewState wrapper:

```text
semantic node
  → strong attachment reference
  → TypeScript ViewState handle
  → native ViewState resource
```

This ensures that an attached ViewState wrapper/resource remains alive while its semantic View remains reachable, independent of the caller retaining the original `ViewState` variable.

`prepareSemanticAttachments` separately acquires a resource-registry lease before structural publication. This lease is not the same as the native ViewState ID stored in the native View.

### 2.9 Effective state contract

The TypeScript side only transports patches. The native Rust retained-state implementation owns effective values.

The effective model is:

```text
immutable semantic View base
  +
mutable ViewState geometry overrides
  +
mutable ViewState presentation overrides
  +
mutable style-state key/value overrides
  =
frame-time effective geometry/presentation
```

Important distinctions:

- Outer `None`: no retained override exists; base value is visible.
- `Some(None)`: explicit nullable override to null/default.
- `clear`: remove the retained override and reveal the immutable base.
- Sparse text attribute patches overlay only named attributes.
- Clearing `textAttributes` resets the retained attribute overlay.
- A style override may replace a themed style identity or merge into the base local style depending on the style form.
- Dynamic style-state entries are merged into the base selector state at frame time.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency graph

```text
public View / semantic constructors
    │
    ├── semantic-node.ts
    │       ├── immutable SemanticViewNode
    │       ├── stateAttachment / contentAttachment HandleIds
    │       ├── SemanticDerivation sidecars
    │       └── wide sequence override sidecars
    │
    ├── retained-path.ts
    │       └── NodeId/path lineage metadata
    │
    └── runtime/runtime.ts
            │
            ├── runtime/attachments.ts
            │       └── NativeResourceRegistry.prepareResolve
            │
            └── structural/retained-dag.ts
                    ├── structural/encoding.ts
                    ├── structural/ir.ts
                    ├── structural/component-id.ts
                    ├── structural/policy.ts
                    ├── native-view-abi.ts
                    │       ├── generated/view_calls.ts
                    │       └── native/addon.ts
                    └── native retained View ABI
```

### 3.2 State dependency graph

```text
public ViewState methods
    │
    └── api/view/retained-state.ts
            │
            └── transport/state/control.ts
                    ├── public-value validation
                    ├── semantic style/color lowering
                    └── generated/state_envelope.ts
                            │
                            └── NativeViewStateResource N-API methods
                                    │
                                    └── crates/iyon-tui-native/src/tui/view_state.rs
                                            │
                                            └── crates/iyon-tui/src/application/view_state.rs
                                                    │
                                                    └── HostInner
                                                            ├── retained_state::ViewStateRegistry
                                                            ├── retained_state::ViewStateRecord
                                                            ├── StateEffects
                                                            ├── dirty state worklist
                                                            └── frame candidate overlay
```

### 3.3 Root ownership and NativeRef ownership

There are several ownership layers:

| Object | Created by | Destroyed/released by |
|---|---|---|
| Semantic View/node | `View` API and semantic factories | JavaScript reachability/GC |
| NativeRef returned from constructor | Native ABI constructor | Caller via `viewReleaseMany` |
| Temporary materialization lease | `MaterializeTx` | `releaseAll` / `releaseAllExcept` |
| Boundary root lease | `RetainedRootBoundary` | Root replacement or `close()` |
| JS NativeRef hint | `SEMANTIC_NATIVE` WeakMap | Semantic node GC, explicit clear, generation mismatch |
| ViewState JS handle | `Tui.viewState()` | Framework handle disposal / owner teardown |
| ViewState native record | Native host registry | Explicit disposal after no prepared/desired/visible leases, or host teardown |
| Attachment prepared lease | `prepareSemanticAttachments` | `commitDesired`, `commitVisible`, replacement, abort, or finalizer |
| Native path reference | native `PathStore`, indexed by TypeScript shape caches | Native runtime maintenance/teardown; no per-use TypeScript release operation observed |

### 3.4 Attachment ownership

`prepareSemanticAttachments` traverses the semantic candidate and handles every state/content attachment exactly once:

- It uses a `Map<HandleId, path>` to reject duplicate state or content attachment identities in one candidate.
- It allows ordinary semantic DAG reuse only when the attachment identity itself is not duplicated.
- It tracks recursion with an `active` node set to reject cycles.
- It traverses lazy axis/grid sequence overrides rather than assuming the eager arrays are authoritative.
- It resolves each attachment through `NativeResourceRegistry.prepareResolve`.
- It validates environment, host, expected kind, accepted node kind, and optional native `validateNodeKind`.
- It creates a prepared lease before structural root publication.

This creates a two-phase attachment lifecycle parallel to structural desired/visible root publication:

```text
prepare candidate
  → prepared attachment leases

structural desired commit
  → desired attachment leases

successful host frame
  → visible attachment leases

replacement / close
  → release visible and desired leases
```

### 3.5 Useful end-to-end diagram

```text
View.text("x").state(state)
    │
    ├── new immutable SemanticTextNode
    │       ├── fresh NodeId
    │       └── stateAttachment = state.id
    │
    ├── runtime.prepareSemanticAttachments
    │       └── resource registry prepared lease
    │
    ├── RetainedRootBoundary.prepareDesiredInstall
    │       └── MaterializeTx
    │               ├── ensureSemanticNative(text)
    │               │       └── viewTextCreate...
    │               └── attachStateIfPresent
    │                       └── viewStateAttach(baseRef, nodeId, stateId)
    │
    ├── RootPublication.commit
    │       ├── host.setDesiredViewRef(rootRef)
    │       ├── attachmentBindings.commitDesired
    │       └── host pending epoch
    │
    ├── environment frame drain
    │       ├── native state registry candidate capture
    │       ├── layout/paint using effective state snapshot
    │       └── visible desired root
    │
    └── state.setPresentation(...)
            ├── native ViewState record mutation
            ├── state revision + dirty mark
            ├── StateEffects derived by Rust
            ├── environment wake
            └── next frame reads the same attached state identity
```

---

## 4. Execution paths and state transitions

### 4.1 Structural View lifecycle

#### Creation

A public `View` constructor creates an immutable semantic node. `View` construction assigns a fresh semantic NodeId and stores the node in a private `WeakMap<View, SemanticViewNode>` (`semantic-node.ts:286-310`, `view.ts:245-270`).

Semantic modifiers create fresh immutable nodes:

```text
old View/node
  → semantic update
  → fresh NodeId
  → copy attachment references
  → optional derivation sidecar
  → new View wrapper
```

This is why a text layout update cannot reuse the old NodeId: NodeId promotion must not return the old native layout.

#### First native use

The first retained boundary call creates a `MaterializeTx` containing:

- ABI symbols/runtime
- ABI generation
- `nativeLookupCeiling`
- local `refs`
- active recursion set
- temporary leases
- borrowed hint metadata
- stale retry count

The direct materializer visits children before the parent constructor.

Examples:

- `materializeContainerNode` resolves the child, then calls `viewContainerCreate`.
- `materializeHangingNode` resolves prefix, continuation, and body, then calls `viewHangingCreate`.
- `materializeAxisNode` resolves all child refs and encodes track words.
- `materializeGridNode` resolves every cell and writes a packed word buffer.
- `materializeDecoratedNode` resolves the child first, then writes decoration metadata.
- `materializeComponentNode` resolves a `HandleId` into a native component identity.
- `materializeContentHostNode` resolves a ContentPort identity.

#### State attachment

After either a derivation or direct constructor returns a base NativeRef, `ensureSemanticNative` calls `attachStateIfPresent` (`retained-dag.ts:1203-1215`).

`attachStateIfPresent`:

1. Resolves the semantic `stateAttachment` HandleId from the TypeScript resource registry.
2. Reads the native `stateId`.
3. Splits it into u32 words.
4. Calls native `viewStateAttach`.
5. Releases the ordinary base constructor lease if the attachment replacement succeeds.
6. Returns the stateful replacement NativeRef.
7. Converts invalid state identities or native status failures to retained refusal.

The semantic node retains only the opaque HandleId. The native View stores the host-local state identity needed by frame-time rendering.

#### Root installation

`RetainedRootBoundary.prepareFrom` performs all fallible preparation before host publication:

- semantic node extraction
- host/install-ref availability
- `MaterializeTx` creation
- retained semantic materialization
- stale current-root validation/recovery
- boundary lease acquisition if a borrowed hint was used
- phase instrumentation

Then one of these publication modes is used:

- **Direct boundary:** host `hostRenderRef`, or `installRef` for a ViewSlot/ScrollPane.
- **Deferred H3 host boundary:** `setDesiredViewRef`; actual paint occurs during a later host frame drain.

The installed/desired root remains leased until replacement or close.

### 4.2 Exact-root path

`renderExactRoot` is the warm-root fast path (`retained-dag.ts:1372-1463`):

```text
same-generation semantic NativeRef hint
    → hostRenderRef
    → no semantic payload reads
    → no child traversal
    → no buffer writes
    → no constructor calls
```

On host cache miss:

1. Remove the stale hint.
2. Promote by NodeId.
3. Install the refreshed hint.
4. Retry `hostRenderRef` once.
5. Keep the promotion lease only if the retry succeeds.
6. Return `no_root_ref` if promotion or retry fails.

A second cache miss is not routed to a different transport.

### 4.3 Derivation paths

The semantic node can carry one derivation sidecar:

```text
textLayout
commonScalar
axisSet
axisSplice
gridCell
```

`tryDerivation` runs after identity resolution and before full payload materialization.

#### Text layout derivation

`viewTextLayoutPatchRoot` changes wrap/alignment on an existing native root.

#### Common scalar derivation

`commonScalarEncoding` packs sparse padding/width/height/min/max changes and calls `viewCommonPatchRoot`.

The mask uses:

```text
padding  = 4
width    = 8
height   = 16
minWidth = 32
maxWidth = 64
minHeight= 128
maxHeight= 256
```

Zero mask means no scalar change. `sizeModeCode` uses `fit=1`, `fill=2`, omitted=0.

#### Axis child derivation

`axisSet` resolves only the replacement child and calls `viewAxisSetChild`. The old native sequence remains retained.

#### Axis splice derivation

`axisSplice` encodes only inserted `(track_word, child_ref)` pairs into reusable scratch. The old native persistent sequence is not flattened or resent.

#### Grid cell derivation

`gridCell` resolves only the replacement child and calls `viewGridSetCell`.

#### Derivation fallback

If the base ref is unavailable, or a native derivation primitive returns an expected status, TypeScript falls back to direct semantic materialization of the same semantic node. This is a same-architecture fallback, not a legacy/secondary transport route.

### 4.4 Axis and grid materialization

#### Axis

For row/column child counts 0–4, fixed-arity generated functions are selected:

```text
viewRowCreate0 ... viewRowCreate4
viewColumnCreate0 ... viewColumnCreate4
```

For wider sequences:

```text
[track_word, child_ref] × count
    → reusable Uint32Array scratch
    → viewAxisCreateBuffer
```

`NATIVE_SMALL_AXIS_ARITY_MAX = 4`.

The policy declares:

```text
NATIVE_BUILDER_MAX_CHILDREN = 524_288
MAX_DIRECT_AXIS_REFS = 524_288
```

TypeScript does not impose a separate retained refusal budget for tree size/depth. The native constructor is the final authority for true child-count limits.

Axis track encoding has two distinct lanes:

- Construction-time `layoutTrackWord`:
  - normal = 0
  - contentMax = 2 | maxRows << 8
  - fixed = 3 | size << 8
  - flex = 4 | 1 << 8
  - flexMax = 5 | maxRows << 8
- Edit-time `axisTrackWord`:
  - `undefined` = 0, preserve existing track
  - normal = 1
  - contentMax = 2 | maxRows << 8
  - fixed = 3 | size << 8
  - flex = 4
  - flexMax = 5 | maxRows << 8

The difference is intentional and must not be collapsed.

#### Grid

Grid construction writes one flat word buffer:

```text
[column_count]
[column track words...]
[row_count]
for each row:
    [row track word]
    [cell_count]
    for each cell:
        [child_ref]
        [column_span | row_span << 16]
        [horizontal_align | vertical_align << 16]
```

The TypeScript implementation calculates:

```text
wordCount = 2
          + column_count
          + row_count * 2
          + cell_count * 3
```

Grid sequence overrides supply row offsets, row tracks, and cell records without forcing the semantic owner to flatten the old sequence.

### 4.5 Text and style materialization

Text uses three transport families.

#### NUL-free spans, one to four spans

When all spans contain no NUL:

```text
viewTextCreateCstring
viewTextCreateCstring2
viewTextCreateCstring3
viewTextCreateCstring4
```

No JavaScript UTF-8 byte buffer is allocated; the native wrapper receives strings.

#### NUL-bearing spans, one to four spans

When any span contains a NUL:

1. Calculate total UTF-8 byte length.
2. Reuse/grow the environment byte scratch.
3. Encode once using `TextEncoder.encodeInto`.
4. Store per-span byte lengths.
5. Call the corresponding UTF-8 constructor.

#### More than four spans

For more than four spans, TypeScript uses a length-delimited words+bytes buffer:

```text
words:
  [span_count]
  [style_ref, span_byte_length] × span_count

bytes:
  concatenated UTF-8 span payloads
```

This avoids an arbitrary refusal at four spans and supports embedded NUL values.

The native text payload limit is `16 MiB` (`policy.ts:31-38`). Above that limit, `byteScratch` throws `RetainedRefusalError`. No other transport is selected.

#### Style refs

`styleRefFor` maps stable semantic style objects to native `StyleRef`s through a generation-scoped sidecar:

- `WeakMap<object, number>` for style objects.
- `Map<string, number>` for color/theme atom strings.
- Native style table remains authoritative.
- Cache is reset when runtime/generation changes.
- `resetStyleRefCacheForThemeChange()` explicitly drops the sidecar before a new theme is installed.

Text attributes are encoded into presence/truth bit lanes:

```text
bold           = 1
dim            = 2
italic         = 4
underline      = 8
reversed       = 16
strikethrough  = 32
```

Unknown attributes and non-boolean values cause retained refusal during structural materialization.

### 4.6 Diff materialization

`materializeDiffNode` creates a packed words+bytes payload.

For each hunk:

```text
oldRange.start      u64 lo/hi
oldRange.count      u64 lo/hi
newRange.start      u64 lo/hi
newRange.count      u64 lo/hi
line_count
```

For each line:

```text
metadata
oldLine u64 lo/hi
newLine u64 lo/hi
text byte length
```

The metadata word is:

```text
diff line kind
  OR
termination code << 16
```

The TypeScript materializer validates all coordinates as safe non-negative integers and encodes text once into a reusable byte scratch. The native decoder validates:

- word framing
- byte framing
- UTF-8
- hunk/range consistency
- line-number semantics
- line termination
- exact word/byte consumption

A mismatch results in an explicit retained refusal, not a partially published diff.

### 4.7 Decorated materialization

Decoration is normalized into the child’s canonical physical box rather than creating a separate physical layout occurrence (`retained-dag.ts:959-1053`).

The fixed decoration header has:

```text
mask
padding top/right
padding bottom/left
width/height modes
min/max width
min/max height
foreground atom
background atom
border style/edges
border color atom
style-state count
```

A custom border glyph trailer contains:

```text
glyph_count
(offset, length) × 8 glyphs
```

Then style-state key/value records contain:

```text
key offset
key length
value offset
value length
```

The TypeScript side:

- Requires all eight custom glyph fields when a glyph object is present.
- Treats an empty glyph object as absent.
- Encodes glyphs and style-state strings in one byte payload.
- Uses absolute payload offsets.
- Counts words/bytes through retained counters.
- Resolves the outer decoration style after child and decoration payload preparation.

### 4.8 State mutation lifecycle

A public mutation has this path:

```text
ViewState.setPresentation(patch)
    │
    ├── assertMutationAllowed()
    ├── presentationEnvelope(patch)
    │       ├── normalizePresentationPatch
    │       └── encodePresentationEnvelope
    ├── FrameworkHandle.call(...)
    ├── NativeViewStateResource.setPresentation(...)
    │       └── native envelope decode
    ├── HostViewState::set_presentation
    │       └── ViewStateRegistry::mutate_record
    ├── ViewStateRecord::apply_presentation
    │       ├── sparse override update
    │       ├── no-op detection
    │       ├── revision increment
    │       └── presentation effect classification
    ├── registry dirty mark
    ├── host.invalidate_state
    └── wake bit → TypeScript registration.markPending()
```

Geometry follows the same path but has native kind validation and geometry-specific effect classification.

A bound mutation does not reconstruct the semantic View. It changes the native mutable state record associated with the attached state identity.

### 4.9 State effective-value transition

For geometry, the native effective calculation is:

```text
base width/height rules
  overridden by retained width/height if present

base decoration
  padding overridden if retained
  min/max bounds overridden if retained
  border edges added/removed if retained
  gap overridden if retained
  alignment axes merged with retained axes
```

For presentation:

```text
base decoration
  style override applied
  foreground override applied
  background override applied
  border color/style/glyph overrides applied
  sparse text attributes overlaid
  dynamic style-state map merged into base style states
```

The semantic View remains immutable. The state snapshot is frame-time mutable/derived data.

### 4.10 Frame capture and state retention

The native registry maintains:

```text
records:     HashMap<state_id, Box<ViewStateRecord>>
committed:   HashMap<state_id, Arc<ViewStateSnapshot>>
dirty:       HashSet<state_id>
desired:     HashSet<state_id>
visible:     HashSet<state_id>
in_flight:   HashSet<state_id>
```

A frame candidate captures the union:

```text
desired ∪ visible ∪ in_flight
```

Only demanded and touched records enter the frame overlay. Unmounted records remain in mutable source-of-truth storage but are not cloned into frame snapshots.

A captured overlay can continue reading an old immutable `Arc<ViewStateSnapshot>` even when a newer state revision is accepted later. This is how a failed/in-flight frame remains internally consistent without preventing newer desired state from being accepted.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Production path | Selection condition | Failure behavior |
|---|---|---|---|
| Render unchanged root | `renderExactRoot` → `hostRenderRef` | Same-generation root hint exists | One stale hint recovery; otherwise `no_root_ref` or explicit error |
| Render changed semantic root | `RetainedRootBoundary.prepareFrom` → `ensureSemanticNative` → root publish | No usable exact-root hit | Refusal leaves old root authoritative; runtime raises preparation failure |
| Reuse prior native node | `ensureSemanticNative` → `SEMANTIC_NATIVE` hint | Same generation | Borrowed hint has no lease; boundary acquires its own lease if needed |
| Promote prior NodeId | `viewRefForNodeId` | NodeId ≤ `nativeLookupCeiling` | Expected cache miss falls through to derivation/direct materialization |
| New text View | `materializeTextNode` → cstring/UTF-8 constructor | One to four spans | Invalid style, empty spans, oversized bytes, native status → retained refusal |
| Wide styled text | `materializeWideTextNode` → words+bytes constructor | More than four spans | Exact framing/UTF-8 checks; refusal on invalid payload |
| New diff View | `materializeDiffNode` → `viewDiffCreateBuffer` | Diff semantic kind | Coordinate/framing/native failure → retained refusal |
| Axis replacement | `tryDerivation(axisSet)` → `viewAxisSetChild` | Valid axis derivation/base ref | Native status may trigger one stale-child/base recovery; then direct fallback |
| Axis splice | `tryDerivation(axisSplice)` → `viewAxisSpliceBuffer` | Valid axis derivation/base ref | Same retained transaction; inserted refs released in cleanup |
| Grid cell replacement | `tryDerivation(gridCell)` → `viewGridSetCell` | Valid grid derivation/base ref | Expected native failure falls back to direct materialization |
| Text path patch | `retained-path.ts` construction → `tryRetainedEditTransactionRender` or path metadata | Path depth ≤4, valid lineage | Constructor throws for invalid paths; native status returns undefined/refusal |
| Multiple text edits | `editTxnBegin` → repeated `editTxnAddTextLayout` → `editTxnCommitRender` | 1–256 ABI edits; semantic constructor requires 2–256 | Any invalid edit aborts transaction; commit failure aborts and throws/returns undefined |
| Attach ViewState | `attachStateIfPresent` → `viewStateAttach` | Semantic node has `stateAttachment` | Invalid/disposed/unsupported state or native attach status releases base lease |
| Mutate ViewState | `ViewState` → envelope → native state wrapper → registry | State wrapper live and mutation allowed | Validation errors occur before native call; disposed/attached lifecycle errors are explicit |
| Clear ViewState override | Clear mask or `clearAll` | Explicit property list or no arguments | Unknown/duplicate fields rejected; no-op clear is accepted without new revision |
| Dynamic style state | `setStyleState`/`clearStyleState` | Non-empty key/value | No-op writes do not revise or schedule work; accepted writes repaint selector subtree |
| Replace root | `RetainedRootBoundary.install` | Direct root boundary | Old root remains on preparation/publish failure |
| Deferred desired root | `prepareDesiredInstall` → `setDesiredViewRef` | H3 host boundary | Desired structure may remain retryable after frame failure |
| Promote desired to visible | `commitVisible(revision)` | Successful host frame receipt | Older/equal revisions ignored; superseded desired refs retained until safe release |

### 5.2 Cache misses versus recovery

The implementation distinguishes several failure classes:

1. **Expected native status**
   - Parsed from `NativeAbiStatusError` or the generated wrapper error string.
   - May represent invalid input, cache miss, refused operation, or stale base/child.
2. **Stale child cache miss**
   - Native status `FAST_CACHE_MISS` with child detail.
   - TypeScript identifies the child ordinal, drops its hint, and retries once.
3. **Stale base cache miss**
   - Native status detail identifies the base.
   - TypeScript drops/reacquires the base and retries once.
4. **Unrecoverable retained refusal**
   - Converted to `RetainedRefusalError`.
   - No second transport route is selected.
5. **Unexpected exceptions**
   - Re-thrown after lease cleanup.
6. **Host publication failure**
   - Direct root publication returns false/undefined or throws.
   - Old root remains authoritative where the protocol guarantees that.
7. **Runtime teardown**
   - A publish refusal after successful preparation is treated as an invariant/lifecycle failure, not as a retryable semantic fallback.

### 5.3 Failure masking and fallback behavior

The retained implementation does not silently mask invalid semantic data with defaults.

Examples:

- A missing axis child throws retained refusal.
- Empty text span lists are refused.
- Unknown text attributes are refused.
- Non-boolean text attributes are refused.
- Partial border glyph sets are refused.
- Invalid u64 coordinates are refused.
- Oversized payloads are refused.
- Invalid state attachment IDs are refused.
- Cycles throw `RetainedCycleError`.
- Wrong-host or duplicate resource attachment validation throws before structural publish.
- State mutation of a disposed or still-attached handle throws.

The main fallback that can obscure the original operation is derivation fallback: a failed derivation primitive may cause direct semantic materialization. This is intentional and remains within the retained architecture.

### 5.4 Root transaction behavior

The boundary protocol is explicitly two-phase:

```text
prepare:
  retain previous root
  materialize candidate
  acquire/recover candidate leases
  validate host/install target
  do not alter installed root

commit:
  publish/install candidate root
  release temporary non-root leases
  transfer candidate to boundary desired/previous role
  capture NodeId high-water
  perform bookkeeping only

abort:
  release all temporary leases
  release any newly acquired boundary lease
  keep previous root installed
```

For deferred host mode:

```text
prepareDesiredInstall
  → materialize candidate
  → commit desired native root
  → host frame drain
  → commitVisible(revision)
```

The desired and visible roots may differ. This allows an accepted desired structural revision to remain retryable after a frame/presentation failure.

### 5.5 State transaction behavior

State writes are individually atomic at the native retained-state record level:

- TypeScript normalizes before native invocation.
- Native N-API decoding validates the complete envelope before applying it.
- Rust `ViewStateRecord` clones its sparse override domain, applies the patch, validates effective geometry for the bound kind, and only then replaces the record field.
- A malformed envelope cannot leave a partial state override.
- A logical no-op does not advance revisions or produce invalidation.

Frame-state transition is separately transactional:

```text
prepare candidate:
  validate all demanded targets
  reserve visible-set capacity
  pin in-flight records
  capture immutable versions

backend receipt success:
  commit visible targets
  clear candidate in-flight pins

backend receipt failure:
  clear in-flight pins
  preserve newer desired state
  retain captured old Arc versions until receipt/candidate ownership ends
```

### 5.6 Disposal behavior

A ViewState cannot be disposed while it has prepared, desired, visible, or in-flight leases.

The TypeScript resource registry rejects disposal with:

```text
STATE_MOUNTED: ViewState is still attached
```

Native Rust registry disposal has the same concept and removes:

- record
- committed snapshot
- dirty mark
- desired/visible/in-flight membership

Identity is not reused.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 JavaScript-side caches

#### Semantic NativeRef hints

```ts
const SEMANTIC_NATIVE = new WeakMap<SemanticViewNode, SemanticNativeHint>();
```

Properties:

- Keyed by immutable semantic node object.
- Value includes native ABI generation and NativeRef.
- Weak key means semantic GC removes the hint.
- Hint does not own a NativeRef lease.
- Generation mismatch makes the hint unusable.
- Explicit stale recovery deletes the hint.

#### Style sidecar

```text
STYLE_REF_CACHE.runtime
STYLE_REF_CACHE.generation
STYLE_REF_CACHE.refs: WeakMap<object, number>
STYLE_REF_CACHE.atoms: Map<string, number>
```

The native style table remains authoritative. The TypeScript cache prevents duplicate style/atom publication within one runtime generation.

#### Path sidecars

```text
PATH_REFS:
  WeakMap<NativeViewAbiSession, WeakMap<object, number>>

PATH_SHAPE_REFS:
  WeakMap<NativeViewAbiSession, Map<string, number>>
```

Path lineages are weakly associated with TypeScript metadata objects. Shape-keyed references allow structurally identical lineages to reuse a native path reference.

No TypeScript per-use path release operation is present in the inspected code. Native path lifetime/maintenance is therefore a native runtime responsibility.

### 6.2 Reusable transport scratch

`MaterializeTx` uses environment-level reusable scratch:

- Axis reference scratch: one `Uint32Array` per active semantic recursion depth.
- Grid/diff word scratch: one `Uint32Array` per active recursion depth.
- Text/diff/decorated byte scratch: one environment-level `Uint8Array`, resized to request.
- Native retains no pointer after a synchronous ABI call returns.
- Scratch is keyed/reset by native runtime identity.

The code explicitly states that scratch is single-owner/synchronous and relies on native calls not retaining pointers beyond return.

### 6.3 Native retained caches

The native runtime exposes diagnostics for:

```text
semantic_cache_entries
semantic_cache_live
native_ref_slots
native_ref_pages
leased_slots
unleased_live_slots
node_ref_entries
path_nodes
path_keys
builders
edit_txns
style_refs
scavenge_queue
scavenge_processed
semantic_cache_expired_seen
semantic_cache_full_sweeps
semantic_cache_entries_removed
native_ref_expired_slots_removed
nodes_inserted_since_full_sweep
generation
alive
```

Native semantic entries are weakly associated with retained Views, while NativeRef slots are lease-tracked. Unleased native objects can be scavenged after weak references expire.

### 6.4 Invalidation and scheduling

#### State invalidation

Rust effect classification is authoritative. TypeScript never supplies effects.

Presentation state:

```text
RESOLVE_STYLE
+ PAINT_SELF or PAINT_SUBTREE
+ DAMAGE
```

Style-state changes conservatively repaint the descendant subtree because inherited selectors may observe the dynamic state.

Geometry state classifies effects by property:

- Width/min/max width:
  - geometry
  - measurement
  - ancestor measurement
  - placement
  - descendant placement
  - clip update
  - content projection
  - intrinsic width/height
  - old/new damage
- Height/min/max height:
  - geometry
  - measurement
  - ancestor measurement
  - placement
  - clip update
  - intrinsic height
  - old/new damage
- Padding/border edges:
  - broad geometry/content/measure/place/clip effects
- Gap:
  - measure and descendant placement
- Alignment:
  - placement only, without intrinsic measurement

A bound state mutation marks the host pending and schedules environment drain if its effect is non-empty. An unbound state mutation remains stored but does not wake a host because there is no demanded frame binding.

#### Structural invalidation

Structural updates alter desired structural revision and trigger a host pending epoch. In deferred mode, host frame drain performs actual visible presentation and state candidate capture.

### 6.5 Work per operation

| Operation | TypeScript work |
|---|---|
| Warm exact root | One semantic-node lookup and one host render call |
| New leaf text, one NUL-free span | Style lookup, one native string constructor |
| New leaf text, NUL-bearing | Byte-length calculation, one UTF-8 encode, one native constructor |
| Wide text | One style lookup per span, one byte encode pass, one word-buffer pass, one native constructor |
| New axis ≤4 children | Child refs plus one fixed-arity native call |
| New wide axis | Visit every child, write 2 words per child, one native buffer call |
| Axis `set` derivation | Resolve replacement child only, one native persistent-sequence call |
| Axis splice derivation | Resolve inserted children only, write 2 words per inserted child, one native call |
| New grid | Visit every cell, write 3 words per cell, one native buffer call |
| Grid cell derivation | Resolve replacement cell only, one native persistent-sequence call |
| State presentation mutation | Validate/pack fixed envelope, native record mutation, effect classification, dirty mark |
| State geometry mutation | Same plus native capability validation/effective geometry validation |
| Frame capture | Demanded state union, touched-state snapshot selection, dirty mark drain for demanded IDs |
| Root replacement | Changed semantic frontier only when hints/derivations permit; otherwise full candidate walk |

### 6.6 Counters and phase instrumentation

`RetainedIdentityCounters` exposes:

```text
retained_hint_hits
retained_hint_misses
node_id_ref_promotion_attempts
node_id_ref_promotion_hits
node_id_ref_promotion_misses
retained_semantic_nodes_inspected
retained_children_visited
direct_materializer_calls
derivation_fast_path_calls
ref_words_written
byte_payload_bytes
transport_scratch_reuses
stale_ref_retries
decorated_normalized_nodes
host_mutations
```

The counters are plain field increments and are intended to prove asymptotic route shape rather than timing.

Optional phase instrumentation records:

```text
transport_prepare_ns
native_materialize_ns
host_commit_ns
```

Native memory diagnostics explicitly report `"string_bytes": null` because the current runtime does not track retained text/style payload bytes.

### 6.7 Width/tick/frame work

This assignment’s transport layer has no per-tick semantic traversal requirement:

- Smoothing/ticking belongs to content/native runtime paths outside this scope.
- State updates schedule a host/environment drain.
- Structural TypeScript transport is synchronous and event-driven.
- Warm roots avoid semantic field reads entirely.
- State presentation changes can update paint without changing structural revision.
- Width-dependent text layout changes are represented as immutable semantic derivations or native path edits, not as a per-frame TypeScript walk.

---

## 7. Tests, benchmarks and observability

### 7.1 Tests inspected

The following TypeScript tests were inspected as behavioral evidence:

```text
packages/iyon-tui/tests/tui_perf13_a.test.ts
packages/iyon-tui/tests/tui_perf13_b.test.ts
packages/iyon-tui/tests/tui_h3_c_transport.test.ts
packages/iyon-tui/tests/tui_native_persistent_seq.test.ts
packages/iyon-tui/tests/tui_native_transaction.test.ts
packages/iyon-tui/tests/tui_state_envelope.test.ts
packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
packages/iyon-tui/tests/tui_native_input_validation.test.ts
packages/iyon-tui/tests/tui_generated_view_abi.test.ts
packages/iyon-tui/tests/generated/view_abi_layout.test.ts
```

No test was run in this investigation.

### 7.2 Behavioral contracts protected by tests

#### Structural materialization

`tui_h3_c_transport.test.ts` protects:

- Semantic nodes materialize through retained transport.
- No second complete-object architecture is required.
- Wide axes remain lazy on derivation paths.
- Component handles resolve to live native component identities.
- NUL-bearing text uses the retained byte lane.
- More-than-four styled spans use the retained variadic buffer lane.
- Custom border glyphs use the retained decorated trailer.

#### Persistent sequence edits

`tui_native_persistent_seq.test.ts` protects:

- Axis replacement parity.
- Wide axis insertion/removal parity.
- Grid cell replacement parity.
- Native retained sequence operations preserve host output compared with a full reference render.

#### Multi-edit transaction

`tui_native_transaction.test.ts` protects:

- Two typed text layout edits share one changed-root transaction.
- A single native edit transaction can update multiple paths.
- The result matches a full reference render.

#### ViewState and accepted-state retention

`tui_perf13_b.test.ts` protects:

- Presentation overrides update style without changing structural revision.
- Dynamic presentation state flows through retained structural attachment.
- Attachment identity survives later immutable View modifiers.
- Border presentation updates do not change box dimensions.
- Unmounted state overrides survive until later mount.
- Explicit null differs from clear.
- Same-host remount preserves state override.
- Clearing reveals the new immutable base.
- Dynamic style-state selectors update native style resolution.
- Disposed state cannot be mutated.
- Component indirection and unsupported state attachments are rejected.
- Invalid patches are atomic and do not alter host epochs.

#### Retained root and attachment transaction

`tui_perf13_a.test.ts` protects:

- Attachment preparation leases one resource per attachment.
- Wrong-host attachments are rejected before visible mutation.
- Unsupported node kinds are rejected.
- Duplicate attachment identities are rejected.
- Failed candidate preparation leaves the visible frame unchanged.
- Wakes are coalesced and explicit barriers control retries.
- Desired structural publication is separated from visible frame commit.

#### Regression behavior

`tui_retained_scene_regressions.test.ts` protects:

- State geometry invalidation reaches dependent layout paths.
- Content refresh preserves full-paint/theme obligations.
- Captured state values carry through ViewSlot replacement.

### 7.3 Native tests inspected

`crates/iyon-tui-native/src/tui/view_abi.rs` contains native tests for:

- NodeId semantic-cache-first behavior.
- Text constructor cache-first behavior.
- Diff buffer framing and validation.
- Axis/grid edit cache-first behavior.
- Text layout patch behavior.
- Multi-edit transaction begin/add/abort/commit.
- Rejection of malformed and overlong payloads.
- Native retained structure cache lookup.

`crates/iyon-tui/src/retained_state/registry.rs` and `record.rs` contain tests for:

- Monotonic state IDs.
- Deferred initial snapshots.
- Dirty deduplication.
- Demand-only capture.
- Newly demanded state capture.
- Old snapshot pinning across later mutations.
- Clear revealing base.
- Disposal rejection while bound/in-flight.
- Atomic visible binding validation.
- Repeated no-op mutation behavior.

### 7.4 Observability gaps

- No TypeScript transport counter records state patch counts directly. State mutation counters are native Rust performance counters.
- Native memory diagnostics expose `string_bytes: null`; retained payload memory cannot currently be directly observed through the current memory snapshot.
- The TypeScript phase instrumentation is optional and benchmark-oriented, not a normal runtime telemetry channel.
- `RetainedRootBoundary` does not expose a public status object for desired/visible NativeRef identities; inspection is possible only through internal tests or host epochs.
- There is no observed public TypeScript API for inspecting whether a semantic node’s NativeRef hint is warm.
- Path references have no visible TypeScript release API; path lifetime is opaque/native.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Semantic model is cleanly separated from transport

`semantic-node.ts` explicitly avoids ABI and native knowledge. The structural transport imports semantic nodes and lowers them directly. This is a strong direction:

```text
semantic declarations
  → transport encoding/materialization
  → native retained View
```

The semantic model does, however, retain opaque `HandleId`s for state/content attachments. This is necessary to defer environment/host validation, but it means semantic-node lifetime has a direct liveness relationship to resource handles through `retainSemanticAttachmentReference`.

### 8.2 Accepted native structure is distinct from TypeScript semantic retention

The implementation correctly separates:

```text
TypeScript semantic node reachability
≠ TypeScript NativeRef hint
≠ native NodeId cache entry
≠ native NativeRef lease
≠ host visible root
```

A semantic node may:

- Have no native materialization yet.
- Have a weak JS hint to a native ref.
- Have a native NodeId cache entry without a JS hint.
- Be represented by a NativeRef held by another boundary.
- Be visible through one root while being used as a child in another root.

The boundary therefore promotes a borrowed hint to its own lease before taking ownership of a root.

### 8.3 Structural root and state attachment use parallel but separate ledgers

Structural roots are tracked by `RetainedRootBoundary`:

```text
desiredRef
visibleRef
previousRef
supersededDesired
desiredRevision
visibleRevision
```

State resources are tracked by:

```text
preparedLeases
desiredLeases
visibleLeases
```

The two ledgers are coordinated by `runtime/attachments.ts` and `runtime/runtime.ts`, but they are not merged.

This is consequential for replacement:

1. Candidate attachments are prepared.
2. Candidate native structure is prepared.
3. Desired root is committed.
4. Attachment leases become desired.
5. A later successful host frame promotes desired attachments to visible.

A structural publication can therefore be accepted while its state attachment is still only desired, not visible.

### 8.4 Dynamic state changes intentionally avoid structural publication

Presentation state updates preserve:

```text
desired_structural_revision
```

while changing:

```text
pending_epoch
visible_frame_revision
```

This is directly tested in `tui_perf13_b.test.ts:23-51`. It proves that accepted native state is retained independently of structural semantic reconstruction.

### 8.5 Native state records are source of truth for dynamic overrides

The native registry retains mutable state records even when unmounted. It prunes immutable committed snapshots when no desired/visible/in-flight binding remains, but keeps the mutable record so a future remount can capture its current values.

This gives the desired behavior:

```text
mutate while unmounted
  → mutable state source changes
  → no demanded frame snapshot
  → dirty mark remains queued

later mount
  → desired binding demands state
  → capture creates committed snapshot from mutable source
  → visible frame sees the prior override
```

### 8.6 Content and state attachments share a generic resource resolver

`NativeResourceRegistry` is deliberately unaware of the semantics of state or content. It validates:

- Handle identity/liveness.
- Resource kind.
- Environment identity.
- Host identity.
- Accepted node-kind set.
- Optional native node-kind validator.

The attachment walker supplies expected kind and target node kind. This allows generic resource lifetime handling while keeping state/content interpretation in their respective planes.

### 8.7 Native state-kind numeric contract needs explicit reconciliation

The TypeScript semantic kind table and native state validator do not appear to use exactly the same vocabulary:

TypeScript semantic values:

```text
0 text
1 diff
2 spacer
3 row
4 column
5 grid
6 hanging
7 container
8 clamp
9 contentMax
10 component
11 decorated
12 contentHost
```

Native `semantic_state_node_kind` currently maps:

```text
0 → Text
1 → Column
2 → Spacer
3 → Row
4 → Column
5 → Grid
6 → Hanging
7 → Container
8/9 → ClampRows
10 → ComponentSlot
12 → ContentHost
```

There is no visible native mapping for `11` (`decorated`), and `1` maps to `Column` rather than an apparent diff class. This may be intentional because native `ViewKind` has no separate diff/decorated variants and TypeScript strips decorated wrappers when validating attachments, but the diff mapping is still potentially consequential:

- A diff state attachment can be validated as a column for geometry capability purposes.
- `gap` could therefore be accepted or rejected according to the wrong native class.
- Vertical/horizontal alignment capability could be classified incorrectly.

The current tests cover state on text, ordinary containers, component indirection, and unsupported cases, but no explicit `View.diff(...).state(...)` capability case was observed.

### 8.8 Decoration normalization is a cross-layer coupling

`materializeDecoratedNode` states that decoration applies directly to the child’s canonical physical box and does not own a separate physical occurrence. The native decoder and retained-state effective decoration machinery both rely on this convention.

This creates a migration hazard if a future structure representation treats decoration as a standalone occurrence: state attachments currently placed on decorated semantic nodes, decoration payload masks, and native state attachment replacement all assume the current normalized shape.

### 8.9 Generated schema and handwritten encoding have parallel authority

The generated schema owns:

- ABI function signatures.
- State property IDs and masks.
- Generated native kind codes.
- ABI hashes and metadata.

The handwritten TypeScript encoding owns:

- Semantic-to-native mapping.
- Different track-word conventions for construction versus edit.
- Decoration-specific masks.
- Diff metadata packing.
- Style attribute bit packing.
- Clamp-specific overflow code mapping.

This is a necessary split but a potential drift point. `nativeViewAbiSession` verifies schema/generator hashes and function count, but semantic encoding correctness is protected mainly by tests and source-level agreement.

### 8.10 No secondary production transport remains in the inspected path

Multiple comments and route implementations state that retained refusal is explicit and does not select a previous-generation complete-object path. This includes:

- `retained-dag.ts:19-22`
- `policy.ts:5-12`
- `native-view-abi.ts:143-157`
- `runtime/runtime.ts:217-222`, `373-389`

Fixtures may use helper calls for tests, but the production route is retained semantic materialization.

---

## 9. Open questions and coverage gaps

1. **Diff state-kind mapping:** Is native numeric kind `1` intentionally interpreted as `Column` for diff ViewState capability checks, or is this a semantic/native kind mismatch?
2. **Decorated state validation:** TypeScript attachment traversal strips decorated wrappers before registry validation, but native `viewStateAttach` operates on the materialized decorated/native patched View. Is `view_native_state_capable` intentionally true for the decorated wrapper, or is the child box the actual native state owner?
3. **State attachment replacement and state resource leases:** `AttachmentBindingState` tracks wrapper/resource leases, while native `viewStateAttach` stores only the numeric state identity in the native View. The exact native frame-time lookup from state ID to state registry snapshot was not fully traced through all host paint paths.
4. **Path reference lifetime:** TypeScript interns path references but does not expose a corresponding release function. The native `PathStore` maintenance policy should be verified against long-lived workloads with many distinct path shapes.
5. **StyleRef cache invalidation:** Theme changes call `resetStyleRefCacheForThemeChange`, but the native style table and existing retained NativeRefs may still contain old style refs. Current tests demonstrate theme refresh behavior, but the exact relationship between existing native styles and newly resolved styles is not fully visible from this scope.
6. **Boundary failure after native desired publication:** `publishDesiredPrepared` calls `setDesiredViewRef` and then performs revision/bookkeeping work. The code treats post-prepare failures as runtime teardown. The native host’s behavior if a bookkeeping call throws after desired publication should be verified.
7. **Concurrent/reentrant use:** Reusable scratch is intentionally single-owner and synchronous. The TypeScript runtime appears event-loop serialized, but the scope does not prove behavior under worker/reentrant calls against one ABI session.
8. **Resource finalizer races:** `PreparedResourceLease` uses `FinalizationRegistry` to repair lease counts. The exact interaction among explicit release, finalizers, host teardown, and in-flight candidate receipts is not fully observable from tests.
9. **Native payload memory:** `string_bytes` remains unavailable in native diagnostics. Large text/diff/decorated payload retention therefore lacks direct memory accounting.
10. **State patch empty object semantics:** The TypeScript normalizers allow empty geometry/presentation patch objects. Native records classify these as no-ops, but no explicit public test for empty patch no-op behavior was observed.
11. **State clear ordering:** Presentation `textAttributes` is a sparse overlay, while `clearPresentation("textAttributes")` resets the whole sparse attribute domain. This is documented by implementation but deserves explicit contract coverage if callers expect per-attribute clears.
12. **Native component state behavior:** Component indirection is rejected for attached ViewState by tests, but the exact reason is split between resource accepted-node-kind validation and native `view_native_state_capable`; the source contract could be made more explicit.
13. **Generated-schema drift:** Hash verification proves the loaded ABI matches the generated manifest, but it does not prove handwritten semantic encoders remain synchronized with all generated schema meanings.
14. **No executed validation in this report:** All test claims above are based on source inspection and test assertions, not execution in this run.

---

## 10. Evidence appendix

### 10.1 Primary source paths and exact symbols

#### Structural transport

```text
packages/iyon-tui/src/transport/structural/component-id.ts
  componentIdForHandleId
  nativeComponentIdOf

packages/iyon-tui/src/transport/structural/encoding.ts
  nativeViewKind
  axisKind
  layoutTrackWord
  axisTrackWord
  gridTrackWord
  wrapModeCode
  horizontalAlignCode
  verticalAlignCode
  diffLineMetadata
  u64Words
  styleAttributeEncoding
  decorationWordEncoding
  commonScalarEncoding

packages/iyon-tui/src/transport/structural/ir.ts
  NATIVE_VIEW_KIND
  NATIVE_LAYOUT_CHILD_KIND
  NATIVE_GRID_TRACK_KIND
  NATIVE_WRAP_MODE
  NATIVE_HORIZONTAL_ALIGN
  NATIVE_VERTICAL_ALIGN
  NATIVE_DIFF_LINE_KIND
  NATIVE_DIFF_LINE_TERMINATION
  StyleNode
  TextSpanNode
  BorderNode

packages/iyon-tui/src/transport/structural/native-view-abi.ts
  NativeViewAbiSession
  NativeViewRenderHost
  nativeViewAbiSession
  nativeViewRefForNodeId
  tryRetainedMaterializeRef
  tryRetainedAxisCreate
  tryRetainedAxisCreateRender
  tryRetainedAxisSetChildRender
  tryRetainedAxisSpliceRender
  tryRetainedGridSetCellRender
  tryRetainedEditTransactionRender
  nativePathRefForLineage
  releaseNativeViewRef

packages/iyon-tui/src/transport/structural/policy.ts
  NATIVE_SMALL_AXIS_ARITY_MAX
  NATIVE_BUILDER_MAX_CHILDREN
  MAX_RETAINED_NEW_NODES
  MAX_RETAINED_DEPTH
  MAX_DIRECT_AXIS_REFS
  MAX_DIRECT_TEXT_BYTES
  MAX_DIRECT_DIFF_BYTES

packages/iyon-tui/src/transport/structural/retained-dag.ts
  SemanticNativeHint
  SEMANTIC_NATIVE
  MaterializeTx
  RetainedRefusalError
  RetainedCycleError
  retainedIdentityCounterSnapshot
  resetRetainedIdentityCounters
  setRetainedPhaseInstrumentation
  ensureNative
  ensureSemanticNative
  tryDerivation
  materializeWithRecovery
  materializeTextNode
  materializeWideTextNode
  materializeDiffNode
  materializeDecoratedNode
  attachStateIfPresent
  renderExactRoot
  acquireKnownRoot
  RootPublication
  RetainedRootBoundary
  resetStyleRefCacheForThemeChange

packages/iyon-tui/src/transport/structural/retained-path.ts
  NativePathStep
  NativePathLineage
  NativeTextLayoutTransactionEdit
  textLayoutAtNativePathForTransport
  textLayoutTransactionForTransport
  nativePathLineage
  nativePathChildLineage
  attachNativePathLineage
  nativeTextLayoutTransaction

packages/iyon-tui/src/transport/structural/style-lowering.ts
  colorNodeFor
  styleNodeFor
  borderNodeFor
  textSpanNodeFor
  materializeStyle
  materializeTheme
```

#### State transport

```text
packages/iyon-tui/src/transport/state/control.ts
  normalizeGeometryPatch
  normalizeClearGeometryProperties
  normalizePresentationPatch
  normalizeClearProperties
  geometryEnvelope
  geometryClearEnvelope
  presentationEnvelope
  presentationClearEnvelope
  StateClearEnvelope

packages/iyon-tui/src/api/view/retained-state.ts
  ViewStateGeometryProperty
  ViewStatePresentationProperty
  ViewStateGeometryPatch
  ViewStatePresentationPatch
  VIEW_STATE_NODE_KINDS
  ViewState
  ViewState.setGeometry
  ViewState.clearGeometry
  ViewState.setPresentation
  ViewState.clearPresentation
  ViewState.setStyleState
  ViewState.clearStyleState
  createViewState
```

#### Semantic/attachment seams

```text
packages/iyon-tui/src/api/view/semantic-node.ts
  SemanticViewNode
  SemanticNodeBase
  createSemanticViewNode
  semanticNodeOf
  semanticNodeHasAttachments
  retainSemanticAttachmentReference
  copySemanticAttachmentReferences
  SemanticDerivation
  setSemanticDerivation
  peekSemanticDerivation
  SemanticSequence
  SemanticAxisSequenceOverride
  SemanticGridSequenceOverride

packages/iyon-tui/src/api/view/view.ts
  View.state
  attachStateDirect
  attachStateForComposition
  attachSemanticResourceForTesting
  withSemanticUpdate
  textLayoutPatch

packages/iyon-tui/src/runtime/attachments.ts
  AttachmentBindingState
  prepareSemanticAttachments
  validateSemanticAttachments
  PreparedAttachmentSet
  PreparedAttachmentSetImpl

packages/iyon-tui/src/transport/native/resource-registry.ts
  NativeResourceRegistry
  PreparedResourceLease
  prepareResolve
  beginDisposal
  cancelDisposal
  invalidateHost
```

#### Runtime/native contracts

```text
packages/iyon-tui/src/runtime/runtime.ts
  Tui.prepareRootPublication
  Tui.render
  Tui.renderCanonical
  Tui.renderDirect
  Tui.prepareMutation
  Tui.flush
  Tui.viewState
  Tui.ensureBoundary

packages/iyon-tui/src/transport/native/addon.ts
  NativeStructuralAttachmentContract
  NativeViewStateContract
  NativeTuiHostContract
  NativeHostEpochs
  NativeTuiAddon
```

#### Native retained state evidence

```text
crates/iyon-tui/src/retained_state/record.rs
  ViewStateRecord
  ViewStateLifecycle
  apply_geometry
  clear_geometry
  apply_presentation
  clear_presentation
  set_style_state
  clear_style_state
  dispose

crates/iyon-tui/src/retained_state/geometry.rs
  GeometryOverrides
  EffectiveGeometry
  geometry_effects
  effects_for_difference

crates/iyon-tui/src/retained_state/presentation.rs
  PresentationOverrides
  ViewStateSnapshot
  effective_geometry
  effective_decoration
  effective_style_states

crates/iyon-tui/src/retained_state/effects.rs
  StateEffects
  presentation_effects

crates/iyon-tui/src/retained_state/registry.rs
  ViewStateRegistry
  PreparedStateCommit
  create
  mutate_record
  capture_candidate
  set_desired
  prepare_visible
  prepare_candidate
  commit_visible_prepared
  commit_prepared
  clear_in_flight_prepared
  dispose
  clear_bindings

crates/iyon-tui/src/retained_state/capture.rs
  StateCandidateOverlay
  StateFrameView

crates/iyon-tui/src/application/view_state.rs
  HostViewState
  state_id
  validate_node_kind
  set_geometry
  clear_geometry
  set_presentation
  clear_presentation
  set_style_state
  clear_style_state
  mutate
  semantic_state_node_kind

crates/iyon-tui/src/application/host.rs
  validate_view_state_kind
  mutate_view_state
  invalidate_state
  candidate_state_commit
  clear_in_flight_state_bindings

crates/iyon-tui-native/src/tui/view_state.rs
  NativeViewState
  view_state_attach wrapper-facing state methods
  decode_geometry_envelope
  decode_presentation_envelope
  decode_geometry_clear
  decode_presentation_clear

crates/iyon-tui-native/src/tui/view_abi.rs
  host_render_ref_impl
  view_state_attach_impl
  view_ref_for_node_id_impl
  view_diff_create_buffer_impl
  edit_txn_begin_impl
  edit_txn_add_text_layout_impl
  edit_txn_commit_render_impl
  edit_txn_abort_impl
```

### 10.2 Generated/schema evidence

```text
packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json
  schemaVersion
  viewText ... viewContentHost
  layoutNormal ... layoutContentMax
  trackContent ... trackFlexMax
  overflowNone ... overflowFooter
  wrapWordThenGrapheme ... wrapNoWrap
  horizontalStart ... horizontalEnd
  verticalTop ... verticalBottom
  diffContext ... diffDeletion
  terminationTerminated ... terminationUnterminated

packages/iyon-tui/src/transport/state/generated/state_envelope.ts
  STATE_WAKE_DRAIN
  GEOMETRY_PROPERTY_IDS
  GEOMETRY_CAPABILITIES
  PRESENTATION_PROPERTY_IDS
  PRESENTATION_CAPABILITIES
  encodeGeometryEnvelope
  encodeGeometryClearMask
  encodePresentationEnvelope
  encodePresentationClearMask

tools/tui-abi/view_abi.toml
  runtime/ViewRef/PathRef/StyleRef schema types
  view_state_attach
  view_content_host_create
  view_spacer_create
  view_text_layout_patch_root
  view_common_patch_root
  view_axis_create_buffer
  fixed row/column constructors
  axis builder operations
  axis set/splice operations
  grid set/create operations
  diff buffer operation
  hanging/container/clamp/component/decorated constructors
  path operations
  edit transaction operations
  style atom/style creation operations
  text cstring/UTF-8 operations
```

Generated structural wrapper operations indexed from `view_calls.ts` include:

```text
runtimeNoop
viewStatusDetail
viewRenderRef
hostRenderRef
viewStateAttach
viewContentHostCreate
viewSpacerCreate
viewTextLayoutPatchRoot
viewCommonPatchRoot
viewAxisCreateBuffer
viewRowCreate0 ... viewRowCreate4
viewColumnCreate0 ... viewColumnCreate4
axisBuilderBegin
axisBuilderPush
axisBuilderFinish
axisBuilderAbort
viewAxisSetChild
viewAxisSpliceBuffer
viewGridSetCell
viewAxisSetChildPath
viewGridCreateBuffer
viewDiffCreateBuffer
viewHangingCreate
viewContainerCreate
viewClampCreate
viewComponentCreate
viewDecoratedCreateBuffer
viewGridSetCellPath
viewReleaseMany
viewRefForNodeId
pathRoot
pathChild
viewTextLayoutPatchPath
viewTextLayoutPatchPathD1 ... viewTextLayoutPatchPathD4
editTxnBegin
editTxnAddTextLayout
editTxnCommitRender
editTxnAbort
styleAtomCreateCstring
styleCreateBits
viewTextCreateCstring
viewTextCreateUtf8
viewTextCreateUtf82 ... viewTextCreateUtf84
viewTextCreateCstring2 ... viewTextCreateCstring4
viewTextCreateBuffer
```

### 10.3 Test evidence manifest

```text
packages/iyon-tui/tests/tui_perf13_a.test.ts
  attachment preparation, duplicate/wrong-host rejection,
  root desired/visible transaction and wake behavior

packages/iyon-tui/tests/tui_perf13_b.test.ts
  retained presentation state, state retention/remount/clear,
  style-state selectors, disposal and invalid-patch behavior

packages/iyon-tui/tests/tui_h3_c_transport.test.ts
  direct semantic materialization, wide sequence derivation,
  component lowering, NUL text, wide styled text, custom glyphs

packages/iyon-tui/tests/tui_native_persistent_seq.test.ts
  axis persistent replacement/splice, grid cell replacement

packages/iyon-tui/tests/tui_native_transaction.test.ts
  multi-path text layout edit transaction

packages/iyon-tui/tests/tui_state_envelope.test.ts
  geometry/presentation normalization and mask envelope behavior

packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
  state geometry invalidation, content refresh, ViewSlot state retention

packages/iyon-tui/tests/tui_native_input_validation.test.ts
  malformed native state/structural input behavior

packages/iyon-tui/tests/tui_generated_view_abi.test.ts
  generated ABI wrapper behavior

packages/iyon-tui/tests/generated/view_abi_layout.test.ts
  generated ABI function/layout manifest assertions
```

### 10.4 Files indexed but not treated as primary handwritten bodies

```text
packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json
packages/iyon-tui/src/transport/state/generated/state_envelope.ts
```

Generated bodies were used only to identify signatures, masks, schema identity, and operation names.

### 10.5 LOC methodology

- Source LOC are approximate physical line counts derived from source line ranges.
- Comments, imports, blank lines, interfaces, and declarations are included.
- Generated bodies are excluded from the primary handwritten transport total.
- Test LOC are reported qualitatively by inspected file and suite behavior rather than claimed as executed coverage.
- No command output or runtime result is presented as executed validation in this report.