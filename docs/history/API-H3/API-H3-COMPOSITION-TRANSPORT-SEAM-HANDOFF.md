# API-H3 - Composition / Structural Transport Seam

**Repository:** `alexykn/iyon-tui`  
**Baseline inspected:** `main` at `1539afd0b53f58c699f146630ca1e3ad84961c5b`  
**Sequence:** `PERF-12 -> API-H1 -> API-H2 / STRUCT-1 -> API-H3 -> PERF-13`  
**Document type:** normative implementation handoff  
**Audience:** implementation agent with solid TypeScript/Rust skills but no assumed knowledge of the retained architecture  
**Delivery model:** five stacked, individually reviewable merge requests on one feature branch; do not merge a partial H3 stack to `main` until H3-E passes the final gates

---

# 0. Executive directive

API-H2 made the physical ownership boundaries visible. API-H3 makes the most important of those boundaries real.

The H3 invariant is:

> **Composition owns semantic retention. Structural transport owns physical/native retention.**

After H3, the TypeScript architecture must read like this:

```text
                         api/view/
                    immutable semantic View
                           identity
                              |
              +---------------+---------------+
              |                               |
              v                               v
       composition/                     runtime / controls
 semantic execution/reuse                 orchestration
              |                               |
              | View publication              | binds the two sides
              | contract only                 |
              +---------------+---------------+
                              |
                              v
                    transport/structural/
                 physical/native retention

                    NativeRef / leases
                    ABI schema / records
                    generated calls
                    materialization policy
                    cold bridge lowering
                    N-API / direct oracle
```

The forbidden shape is:

```text
composition/
    -> BridgeViewNode
    -> BRIDGE_VIEW_KIND
    -> bridge schema numbers
    -> style-lowering.ts
    -> NativeRef policy
    -> generated ABI
```

The equally forbidden inverse is:

```text
transport/structural/
    -> composition/PersistentSeq implementation details
    -> execution scopes
    -> State dirtiness
    -> child-owner bookkeeping
```

H3 is complete when the dependency relationship is:

```text
composition/ ---------> api/view semantic model
                              ^
                              |
transport/structural/ --------+

runtime/ and selected control owners bind composition publication targets
onto structural transport implementations.
```

The structural transport may consume semantic Views. It must not define what a semantic View *is*.

The composition engine may decide whether a semantic View occurrence is reused and whether a scope executes. It must not know how that View becomes a `NativeRef`, bridge record, Rust `View`, or host mutation.

## 0.1 H3 is not a new renderer or reconciler

Do **not** introduce a second structural change language such as:

```text
SemanticGraphDelta
InsertChild
RemoveChild
MoveChild
ReplaceNode
```

for H3.

PERF-12 already established the useful semantic information:

```text
immutable View identity
stable NodeId
unchanged-subtree cutoff
semantic derivation hints
PersistentSeq structural edits
transactional publication
```

Structural transport already knows how to exploit that information. H3 must separate ownership of those facts from their physical encoding, not duplicate them.

If profiling later proves that an explicit semantic delta object is useful, that is a separate optimization task. It is not required to decouple composition from transport.

## 0.2 H3 is a TypeScript architecture task

H3 should not redesign Rust presentation semantics or the structural ABI.

The Rust retained graph remains the authoritative physical representation after transport crossing. The existing generated structural ABI remains the physical wire/native contract.

Rust changes are allowed only when required to preserve an existing structural parity test after TypeScript ownership changes. They are not the goal of H3.

## 0.3 Public behavior must not change

H3 must preserve:

```text
H1 public API surface
H2 physical directory ownership
View fluent semantics
NodeId allocation semantics
stable View identity
semantic equality behavior
PERF-12 subtree cutoff
keyed/unkeyed execution identity
State subscription ownership
dirty scheduling and microtask coalescing
PersistentSeq asymptotics
retained derivation fast paths
NativeRef leases and generations
cold fallback behavior
N-API/direct structural parity
History sideband publication semantics
ViewSlot and ScrollPane ownership modes
animation/tick behavior
headless screen output
Unicode/cell correctness
```

A user should not be able to tell that H3 happened except through repository structure, internal diagnostics, and future extensibility.

---

# 1. Baseline: what H2 actually left behind

The inspected `main` branch already has the correct coarse tree:

```text
packages/iyon-tui/src/
|-- api/
|-- composition/
|-- runtime/
|-- testing/
|-- transport/
`-- index.ts
```

Composition currently contains:

```text
composition/
|-- child-owner.ts
|-- compose.ts
|-- define-view.ts
|-- execution-context.ts
|-- execution.ts
|-- persistent-seq.ts
`-- tracked-state.ts
```

Structural transport currently contains:

```text
transport/structural/
|-- component-view.ts
|-- ir.ts
|-- native-view-abi.ts
|-- policy.ts
|-- retained-dag.ts
|-- style-lowering.ts
`-- view-bridge.ts
```

This is the H2 success condition: ownership is visible.

It is not yet the H3 success condition.

## 1.1 `composition/compose.ts` currently reasons in transport IR

At the baseline, `composition/compose.ts` imports and uses:

```text
BRIDGE_VIEW_KIND
BRIDGE_LAYOUT_CHILD_KIND
BRIDGE_GRID_TRACK_KIND
BRIDGE_OVERFLOW_KIND
BridgeViewNode
BridgeLayoutChild
BridgeGrid* records
StyleNode / ColorNode / DecorationNode
style-lowering.ts
component-view.ts
view-bridge.ts
peekBridgeSequenceOverride
peekBridgeGridSequenceOverride
```

Examples of semantic reuse decisions are expressed as bridge comparisons:

```text
previousNode.kind === BRIDGE_VIEW_KIND.text
previousNode.child === nodeForBridge(base)
previous grid track === BRIDGE_GRID_TRACK_KIND.fixed
```

That is the primary H3 seam problem.

Composition is correctly deciding:

> "Is the newly requested semantic value the same as the previously committed semantic value for this slot?"

But it is answering that question by inspecting the physical structural transport representation.

The decision belongs to composition. The representation does not.

## 1.2 `composition/execution.ts` uses structural lowering as a View validator

`execution.ts` is mostly transport-independent already. It owns the right concepts:

```text
scope identity
pending/committed outputs
prepare/commit/abort publication
State dependencies
child ownership
retry obligations
batch ordering
```

However the baseline still calls `nodeForBridge(output)` after a component body returns, effectively using the structural bridge as the proof that the returned value is a real framework `View`.

H3 must replace this with a semantic-layer assertion.

A component body's output must be validated by the semantic View owner, not by asking whether transport can lower it.

## 1.3 `api/view/view.ts` is the deeper coupling

Removing imports from `composition/` is not enough.

The current public `View` implementation directly imports and constructs structural bridge records. It currently owns or references concepts such as:

```text
BRIDGE_* numeric kind codes
VIEW_BRIDGE_SCHEMA_VERSION
BridgeViewNode
Bridge layout/grid records
bridge style/color records
setViewNode / nodeForBridge
bridge derivation sidecars
bridge sequence override sidecars
native retained path metadata
native transaction encodings
packed track words
```

A fresh `View` is therefore currently backed by a transport-shaped `BridgeViewNode`.

That means an import-only cleanup would be dishonest. Composition could import a helper called `semanticNodeOf()`, but if that helper still returned a structural bridge record carrying schema/version/packed-native concepts, the architecture would still be coupled.

H3 must therefore create a genuinely semantic private View representation.

## 1.4 `transport/structural/ir.ts` owns semantic facts today

The current structural IR module is mixed. It contains:

```text
wire/schema constants
BridgeViewNode records
semantic-ish style/decoration records
derivation metadata
PersistentSeq sequence overrides
clone/merge helpers used by semantic View construction
```

It also imports `composition/persistent-seq.ts`.

That creates the reverse dependency:

```text
transport/structural -> composition
```

H3 must remove that dependency too.

## 1.5 `retained-dag.ts` is correctly physical, but consumes the wrong node type

`retained-dag.ts` owns appropriate physical responsibilities:

```text
semantic-node -> NativeRef correspondence
runtime generations
borrowed NativeRef hints
lease ownership
scratch buffers
generated structural materializers
cold fallback routing
stale-ref recovery
root boundaries
transport performance counters
```

Those responsibilities stay there.

What changes is its input type.

Today it treats `BridgeViewNode` as both:

```text
semantic declaration
and
physical lowering record
```

After H3 it consumes a transport-independent `SemanticViewNode` and performs any ABI mapping itself.

## 1.6 Runtime is already the natural integration layer

`runtime/runtime.ts` already imports both:

```text
composition/execution
and
transport/structural
```

That is healthy.

The runtime creates the one retained execution runtime per Tui, creates component-scope projections, stages root publication, binds History sideband state, and invokes retained/cold structural publication.

H3 should make this explicit rather than moving more transport knowledge into composition.

## 1.7 The existing publication transaction is worth preserving

The current execution engine already has an important generic shape:

```text
evaluate semantic scopes
    -> stage every fallible publication
    -> if any prepare fails: abort all staged publications
    -> if all prepare succeeds: commit semantic state + publications
```

The current `PreparedPublication` / `PublicationTarget` concept is close to the desired H3 seam.

H3 should narrow and name it, not replace it.

---

# 2. Research: comparable architectures and the transferable lesson

H3 was compared against retained UI runtimes and compiler/lowering architectures. The purpose is not to copy another framework; it is to identify boundaries that have survived similar pressure.

## 2.1 Jetpack Compose: Composer vs Applier

Jetpack Compose explicitly separates composition from target-tree application.

The Compose `Applier` is responsible for applying tree operations emitted by composition. The Composer calculates/records changes first, and Applier calls happen only during the apply phase after composable execution. `onBeginChanges()` / `onEndChanges()` bracket target mutation.

Transferable lesson:

> Composition must not contain the target tree implementation.

Non-transferable detail:

> Iyon does not need Compose's insert/remove/move operation language in H3, because PERF-12 already retains immutable semantic Views and derives an efficient native frontier.

Use the boundary, not the exact protocol.

References:

- https://developer.android.com/reference/kotlin/androidx/compose/runtime/Applier
- https://android.googlesource.com/platform/frameworks/support/+/0ecddc8152eda57b806c09d55477d0c715d132fe/compose/runtime/runtime/src/commonMain/kotlin/androidx/compose/runtime/Applier.kt

## 2.2 React reconciler: HostConfig

React's reconciler is target-agnostic. A renderer supplies a HostConfig describing the physical host environment. React explicitly distinguishes mutation and persistence modes, and treats reconciler-internal handles as opaque to host code.

Transferable lesson:

> Target mechanics should be injected or adapted at the host boundary rather than imported into semantic reconciliation logic.

Iyon is closer to a persistence-oriented model than a DOM mutation renderer: semantic Views are immutable retained values, and replacement/materialization is already identity-driven.

Non-transferable detail:

> H3 should not grow a huge HostConfig with dozens of callbacks. The existing structural publication contract is much narrower and better matched to Iyon.

Reference:

- https://github.com/facebook/react/blob/main/packages/react-reconciler/README.md

## 2.3 Flutter: Widget / Element / RenderObject separation

Flutter keeps immutable configuration, retained occurrence identity, and physical render objects separate. A Widget may configure multiple locations; an Element represents a particular retained use; RenderObjects own layout and paint behavior.

Flutter's own architecture documentation explicitly calls out performance and clarity benefits from keeping Element and RenderObject trees separate.

Transferable lesson:

> Do not make the semantic immutable declaration literally be the physical retained renderer object or wire record.

That maps closely to H3:

```text
Iyon semantic View / composition identity
    !=
Iyon NativeRef / Rust View / ABI record
```

References:

- https://docs.flutter.dev/resources/inside-flutter
- https://api.flutter.dev/flutter/widgets/Element-class.html
- https://api.flutter.dev/flutter/rendering/RenderObject-class.html

## 2.4 MLIR: source dialect vs target lowering

MLIR dialect conversion separates source IR from target legality and conversion rules. The LLVM target flow goes further: non-trivial transformation happens in a target-adjacent dialect before final translation, keeping the final bridge simple and reducing dependency/churn.

Transferable lesson:

> Semantic tags and values should not be defined by the ABI schema merely because the initial target happens to use similar concepts.

H3 should therefore have an explicit mapping:

```text
SemanticViewKind.text
    -> BRIDGE_VIEW_KIND.text
```

rather than:

```text
SemanticViewKind === BRIDGE_VIEW_KIND
```

Even if both happen to use small integers internally, the code must not rely on equality of the numeric values.

References:

- https://mlir.llvm.org/docs/DialectConversion/
- https://mlir.llvm.org/docs/TargetLLVMIR/

## 2.5 Resulting H3 design rule

The research converges on a simple rule:

```text
semantic representation
    -> explicit adapter/lowering
        -> physical target representation
```

But Iyon should preserve its existing retained-value strengths:

```text
NO generic DOM-style operation list
NO second reconciliation engine
NO new compiler pass over every unchanged node
NO duplicate permanent semantic/bridge DAGs
```

---

# 3. Normative target architecture

The final dependency picture is:

```text
api/view/
    View
    SemanticViewNode
    semantic NodeId
    semantic normalized geometry/presentation
    semantic derivation hints
    semantic lazy sequence metadata

composition/
    defineView
    execution scopes
    State tracking
    child identity
    semantic slot reuse
    structural publication contract

runtime/
    owns live composition runtime
    owns host lifecycle
    supplies concrete publication targets

api/controls/
    may continue owning control-specific runtime/transport adapters
    but MUST NOT force composition to import transport

transport/structural/
    semantic -> structural ABI mapping
    NativeRef cache/hints
    leases
    generated ABI calls
    retained materialization
    cold object bridge lowering
    stale-ref recovery

transport/abi/structural/
    schema
    generated records/calls
```

## 3.1 Required dependency rules

At H3 completion:

```text
composition/**
    MAY import api/**
    MAY import composition/**
    MUST NOT import transport/**
    MUST NOT import runtime/**
    MUST NOT import testing/**

transport/structural/**
    MAY import api/view private semantic internals
    MAY import api/presentation semantic types
    MAY import transport/native/**
    MAY import transport/abi/structural/**
    MUST NOT import composition/**
    MUST NOT import runtime/**

api/view/**
    MAY import composition helpers where the existing retained-construction API requires them
    MUST NOT import transport/structural/**
    MUST NOT import transport/abi/**
    MUST NOT name NativeRef or ABI schema values

runtime/**
    MAY import composition/**
    MAY import transport/**
    is an intentional integration/orchestration layer
```

Do not expand H3 into a universal rule that every `api/controls/**` implementation is transport-free. Controls currently own private native resources and retained structural boundaries by design. That broader cleanup is not necessary to make composition independent.

## 3.2 Semantic retention vs physical retention

Semantic retention includes:

```text
View identity
NodeId identity
View kind and semantic fields
child semantic identities
style/geometry semantic values
derivation relationship: "next came from base by operation X"
PersistentSeq-backed semantic child storage
execution scope identity
State subscriptions
keyed/unkeyed occurrence ownership
```

Physical/native retention includes:

```text
NativeRef
runtime generation
lease counts
NodeId -> NativeRef promotion
bridge schema version
bridge numeric discriminants
packed track words
packed scalar masks
style atom refs
ABI scratch buffers
N-API/direct calls
cold decode objects
native path refs
```

A useful review question is:

> Could this concept still exist if the structural target were an in-memory JS renderer instead of Rust/N-API?

If yes, it is probably semantic.

If it exists because of the current Rust ABI, it belongs to transport.

---
# 4. Semantic View representation

H3 must make the semantic View node a first-class private API-owned concept.

Recommended ownership:

```text
api/view/semantic-node.ts
```

The exact filename may be `semantic.ts` if that better fits the post-H2 tree. Do not expose it from `src/index.ts`.

## 4.1 Required semantic-node properties

The semantic node remains:

```text
immutable
process-local
identity-bearing
backend-neutral
safe to inspect from composition
safe to consume from structural transport
```

Conceptually:

```ts
interface SemanticNodeBase {
  readonly id: SemanticNodeId;
  readonly kind: SemanticViewKind;
}

type SemanticViewNode =
  | SemanticTextNode
  | SemanticDiffNode
  | SemanticSpacerNode
  | SemanticRowNode
  | SemanticColumnNode
  | SemanticGridNode
  | SemanticHangingNode
  | SemanticContainerNode
  | SemanticClampNode
  | SemanticContentMaxNode
  | SemanticComponentNode
  | SemanticDecoratedNode;
```

This is an internal shape, not public API.

## 4.2 Semantic discriminants must not come from bridge schema

Do not import `bridge-schema.json` or `BRIDGE_VIEW_KIND` into `api/view`.

Use a private semantic vocabulary.

A small integer discriminant is acceptable for performance:

```ts
export const SEMANTIC_VIEW_KIND = {
  text: 0,
  diff: 1,
  spacer: 2,
  row: 3,
  column: 4,
  grid: 5,
  hanging: 6,
  container: 7,
  clamp: 8,
  contentMax: 9,
  component: 10,
  decorated: 11,
} as const;
```

The exact values are private and may differ.

Structural transport MUST map them explicitly:

```ts
function bridgeViewKind(kind: SemanticViewKind): BridgeViewKind {
  switch (kind) {
    case SEMANTIC_VIEW_KIND.text: return BRIDGE_VIEW_KIND.text;
    // ... exhaustive mapping ...
  }
}
```

Do not use:

```ts
const bridgeKind = semanticKind as BridgeViewKind;
```

Do not add an assertion that the values remain numerically equal. Numeric equality would recreate the coupling H3 is removing.

## 4.3 NodeId stays semantic

The existing TypeScript NodeId remains a semantic identity and cache key.

H3 does not change:

```text
monotonic allocation
safe-integer range
clone/reuse behavior
NodeId halves used at ABI crossing
high-water semantics used by native cache lookup policy
```

Move the helpers that are semantically about identity next to the semantic node owner:

```ts
semanticNodeOf(view)
viewNodeId(view)
nodeIdPair(view)
viewNodeIdHighWater()
```

`nodeIdPair()` is allowed to exist in the semantic layer because splitting a safe integer into two u32 words is a pure representation helper. If review prefers stricter ownership, transport may own the split operation while semantic owns only the full ID. Do not let this minor placement question block H3.

The hard rule is that NodeId allocation and meaning are not defined by the generated ABI.

## 4.4 Semantic node association replaces `view-bridge.ts`

Today `transport/structural/view-bridge.ts` owns a WeakMap from `View` to `BridgeViewNode`.

Final H3 shape:

```ts
const semanticNodes = new WeakMap<View, SemanticViewNode>();

export function installSemanticNode(view: View, node: SemanticViewNode): void;
export function semanticNodeOf(view: View): SemanticViewNode;
```

This WeakMap belongs under `api/view` because it defines what the immutable `View` value contains.

The public `View` wrapper remains opaque.

A fake object with `kind: "view"` must not become valid simply because its shape resembles a View. `semanticNodeOf()` is the authoritative private brand check.

`composition/execution.ts` uses that semantic assertion instead of `nodeForBridge()`.

## 4.5 No permanent double representation

A final View must not permanently own both:

```text
SemanticViewNode
and
BridgeViewNode
```

That would double retained JS graph metadata and make both representations authoritative.

Temporary dual representation is allowed only inside H3-A/H3-B as a migration shim on the H3 feature branch.

By H3-D:

```text
View -> SemanticViewNode
```

is authoritative.

A bridge object may be created lazily for a cold N-API fallback, but it is a derived transport artifact, never the semantic source of truth.

## 4.6 Semantic children remain semantic node references

For hot retained traversal, child edges should remain direct semantic-node references rather than repeatedly unwrapping `View` wrappers.

For example:

```ts
interface SemanticLayoutChild {
  readonly track: SemanticTrack;
  readonly child: SemanticViewNode;
}
```

and:

```ts
interface SemanticContainerNode {
  readonly kind: typeof SEMANTIC_VIEW_KIND.container;
  readonly child: SemanticViewNode;
}
```

This preserves the current cheap child identity comparisons.

Composition can answer:

```text
previous.child === semanticNodeOf(nextChild)
```

without touching transport.

Transport can recurse the same semantic graph.

## 4.7 Semantic View construction remains frozen

Retain the current immutability discipline:

```text
new semantic node receives fresh NodeId
semantic record is frozen
View wrapper is frozen
unchanged child node identities are retained
```

Wide lazy nodes may use a lazy getter/cache internally as today, but that lazy representation must be expressed in semantic terms and must not expose bridge records.

---

# 5. Semantic normalized presentation values

`composition/compose.ts` currently calls structural `style-lowering.ts` merely to compare semantic style/color/border values.

That is wrong ownership.

H3 must split:

```text
semantic normalization
from
transport lowering
```

## 5.1 New private semantic normalization owner

Recommended owner:

```text
api/presentation/semantic-style.ts
```

or a similarly cohesive private module.

It should define frozen/copy-safe semantic records such as:

```ts
type SemanticColor =
  | { readonly kind: "theme"; readonly key: string }
  | { readonly kind: "named"; readonly value: AnsiColor }
  | { readonly kind: "indexed"; readonly value: number }
  | { readonly kind: "rgb"; readonly r: number; readonly g: number; readonly b: number };

interface SemanticStyle {
  readonly theme?: string;
  readonly foreground?: SemanticColor;
  readonly background?: SemanticColor;
  readonly attributes: Readonly<Record<TextAttribute, boolean>>;
}
```

The exact representation may be optimized, but it must not contain:

```text
bridge schema tags
native style refs
packed ABI values
```

## 5.2 Preserve snapshot semantics

Current style lowering copies public semantic values into plain records at View construction time.

H3 must preserve that behavior.

Do not store a mutable caller-owned `StyleSpecValue` object by reference and then observe later external mutation.

Normalize/copy once at semantic View construction.

The same applies to:

```text
BorderSpec
ColorSpec
StyleRef
style-state maps
overflow indicator style
text-span style
```

## 5.3 Composition compares semantic normalized values

All H3 composition equality helpers move from transport records to semantic records:

```text
style equality
color equality
border equality
decoration delta equality
text span equality
overflow equality
grid track equality
```

The result must be exactly equivalent to the current reuse decisions.

A retained no-op before H3 must remain a retained no-op after H3.

## 5.4 Transport owns the semantic-to-bridge style map

`transport/structural/style-lowering.ts` remains, but its purpose becomes narrow:

```text
SemanticColor -> BridgeColorNode/native style atom
SemanticStyle -> BridgeStyleNode/native style atom
SemanticBorder -> BridgeBorderNode
```

It must no longer be imported from composition.

If all hot retained structural materializers can encode semantic style directly, cold object lowering may be the only place that constructs bridge style records.

---

# 6. Semantic derivation hints

PERF-12 derivation hints are important and must survive H3.

They are semantic facts about how one immutable View was derived from another, even though the current records contain physical ABI packing.

H3 splits the fact from the encoding.

## 6.1 What is semantic

These relationships are semantic:

```text
text layout changed from base
common scalar/decorative value changed from base
axis child at index changed
axis splice occurred
one grid cell changed
```

They belong with the semantic View representation.

Conceptually:

```ts
type SemanticDerivation =
  | {
      readonly kind: "textLayout";
      readonly base: SemanticViewNode;
      readonly wrap: SemanticWrapMode;
      readonly align: SemanticHorizontalAlign;
    }
  | {
      readonly kind: "commonScalar";
      readonly base: SemanticViewNode;
      readonly changes: SemanticCommonScalarChanges;
    }
  | {
      readonly kind: "axisSet";
      readonly base: SemanticViewNode;
      readonly childIndex: number;
      readonly track: SemanticTrack | undefined;
      readonly child: SemanticViewNode;
    }
  | {
      readonly kind: "axisSplice";
      readonly base: SemanticViewNode;
      readonly index: number;
      readonly removeCount: number;
      readonly inserted: readonly SemanticLayoutChild[];
    }
  | {
      readonly kind: "gridCell";
      readonly base: SemanticViewNode;
      readonly row: number;
      readonly column: number;
      readonly child: SemanticViewNode;
    };
```

Exact internal fields can stay optimized.

## 6.2 What is physical

These do **not** belong in the semantic derivation:

```text
native patch bit masks
trackWord
packed u16 lanes
ABI enum values
pathRef
NativeRef
```

Transport derives those on demand.

Example:

```text
semantic:
    { kind: "fixed", size: 4 }

transport:
    3 | (4 << 8)
```

The transport packing function is the only owner of that encoding.

## 6.3 Do not replace derivation hints with tree diffing

H3 must not delete derivation hints and then compensate by recursively comparing previous/new semantic trees in transport.

That would turn a precise construction-time O(1)/O(log N) fact into repeated inference work.

Preserve construction-time hints.

## 6.4 Derivation sidecars remain weak

Retain weak ownership.

A derivation hint must not keep a View graph alive beyond the lifetime already implied by the derived node itself.

Use WeakMap sidecars keyed by semantic node or View, matching the current lifecycle discipline.

---

# 7. PersistentSeq and wide semantic structures

`PersistentSeq` remains a semantic-retention optimization established by PERF-12.

H3 does not replace it.

The key requirement is:

> Structural transport must not import the `composition/persistent-seq.ts` implementation.

## 7.1 Keep H2 ownership unless a move is clearly justified

H2 deliberately placed `PersistentSeq` under `composition/`.

H3 should not move it solely for aesthetic purity.

Instead introduce a narrow structural view of the sequence where transport needs it.

For example:

```ts
export interface SemanticSequence<T> {
  readonly length: number;
  get(index: number): T | undefined;
  values(): IterableIterator<T>;
}
```

The exact interface should match operations transport actually needs. Do not expose mutation methods unless required.

`PersistentSeq` can satisfy this interface without transport importing its class.

## 7.2 Semantic sequence overrides move out of transport IR

Current `BridgeSequenceOverride` / `BridgeGridSequenceOverride` sidecars belong to semantic View construction.

Replace them with transport-neutral equivalents:

```text
SemanticAxisSequenceOverride
SemanticGridSequenceOverride
```

They may contain:

```text
base semantic node
read-only semantic sequence
semantic edit descriptor
row offsets
semantic grid tracks
cell index maps
```

They must not contain:

```text
BridgeViewNode
trackWord
ABI schema values
```

## 7.3 Preserve lazy flattening behavior

Wide axes and grids currently avoid flattening until a consumer actually requests the flat representation.

H3 must preserve:

```text
100k-wide single child edit
    -> O(log_32 N) PersistentSeq work
    -> no eager 100k child flat copy
```

A semantic/transport boundary is not permission to call `.toArray()` on every publication.

## 7.4 Transport consumes the sequence through the semantic interface

Transport may iterate the exact changed edit or inspect sequence elements needed by a generated retained operation.

It must not know:

```text
PersistentSeq branch factor
leaf implementation
internal aggregate representation
clone counters
```

Those remain semantic retention implementation details.

---

# 8. Component placement seam

`composeComponent()` is a special case because the semantic View references a live framework handle whose physical host identity currently comes from transport.

H3 must prevent composition from calling `componentIdForPlacement()` in structural transport.

## 8.1 Semantic component identity is the framework HandleId

The existing `FrameworkHandle.id` is explicitly JavaScript-local framework identity and is not a native identifier.

Use that semantic identity in the semantic View node:

```ts
interface SemanticComponentNode {
  readonly id: SemanticNodeId;
  readonly kind: typeof SEMANTIC_VIEW_KIND.component;
  readonly handleId: HandleId;
}
```

Composition can compare:

```ts
previous.handleId === handle.id
```

without touching native resources.

## 8.2 Transport resolves HandleId to the current native component resource

The native component ID remains physical.

Extend the private handle/native resource association so transport can resolve a live resource from semantic HandleId without importing runtime ownership.

Recommended shape:

```text
runtime/handle-registry
    owns HandleId allocation/lifetime
        |
        | passes HandleId at registration/release
        v
transport/native/resources
    owns raw resource association
    may maintain HandleId -> weak raw resource lookup
```

Requirements:

```text
lookup must not keep a disposed handle alive
release removes the lookup
lowering a disposed/missing component fails deterministically
public ComponentId callback semantics do not change
```

A `Map<HandleId, WeakRef<object>>` plus explicit release is acceptable if Bun support is already guaranteed. A different weak association is acceptable if it preserves the same lifetime semantics.

Do not make composition import `nativeResourceOf()`.

## 8.3 `component-view.ts` should disappear or become purely transport-side resolution

Final semantic View construction should live under `api/view`.

If a tiny transport helper remains, it should be named for what it physically does, for example:

```text
transport/structural/component-id.ts
```

and should resolve:

```text
HandleId -> native ComponentId
```

It must not construct semantic Views.

---

# 9. Cold object bridge lowering

The safe N-API cold decoder still needs a bridge-shaped object for complete fallback.

That does not mean `View` must be bridge-shaped.

## 9.1 Add an explicit cold lowering module

Recommended:

```text
transport/structural/cold-lowering.ts
```

Responsibility:

```text
SemanticViewNode
    -> complete BridgeViewNode object
```

including:

```text
schema field
bridge numeric kind tags
bridge layout child tags
bridge grid tags
bridge overflow tags
bridge style/color records
native component ID resolution
```

This is the exact place where schema knowledge belongs.

## 9.2 Cold lowering must be complete

Cold fallback is the correctness path.

It must support every semantic structural View kind accepted by the public View API.

Do not produce a partial lowerer that silently refuses obscure border/diff/grid cases and depends on the retained fast path being available.

## 9.3 Cold lowering may cache weakly, but cannot become a second authoritative DAG

If repeated cold operations need to reuse object lowering, a WeakMap cache is acceptable:

```text
SemanticViewNode -> BridgeViewNode
```

provided:

```text
cache entries are weakly keyed
bridge objects do not outlive semantic nodes because of the cache
hot retained publication does not require the bridge object
no semantic mutation happens in the bridge object
```

## 9.4 Retained hot path must not construct bridge objects

By H3-C the ordinary retained materializer should consume `SemanticViewNode` directly.

This is a hard performance invariant.

Correct final shape:

```text
warm/retained:
    SemanticViewNode -> generated ABI calls

cold fallback:
    SemanticViewNode -> BridgeViewNode -> N-API decoder
```

Incorrect final shape:

```text
all publications:
    SemanticViewNode -> allocate BridgeViewNode tree -> retained materializer
```

---
# 10. Structural publication contract

The existing execution transaction is the correct conceptual seam. H3 makes it explicit and structural.

Recommended owner:

```text
composition/publication.ts
```

## 10.1 Working contract

Use a narrow contract equivalent to:

```ts
export interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}

export interface StructuralPublicationTarget {
  prepare(output: View): PreparedStructuralPublication | undefined;
  needsPublication?(output: View): boolean;
}
```

The existing names may be retained if renaming creates noise, but the comments and ownership must make clear that this is the structural publication seam.

The contract contains no:

```text
NativeRef
bridge node
ABI session
host object
path ref
state-plane mutation
content-plane mutation
```

## 10.2 `undefined` means prepare refusal

Preserve current semantics:

```text
prepare(output) -> PreparedStructuralPublication
    candidate can commit

prepare(output) -> undefined
    candidate could not be prepared
    entire retained execution batch aborts
```

Preparation refusal is not a partial commit.

## 10.3 Commit is infallible by contract

The current retained protocol treats a commit throw as pathological teardown/failure.

Keep that rule.

All ordinary fallible work must occur before commit:

```text
native materialization
lease acquisition
validation
cold fallback preparation
History sideband validation
component target validation
```

Commit should perform only the already-prepared authoritative swap/host install.

## 10.4 Abort is idempotent cleanup

Abort releases staged physical resources and leaves the previously committed frame authoritative.

Abort must not:

```text
publish a new root
mutate committed View ownership
advance projectedOutput
bind a staged History
leak temporary NativeRefs
```

Where practical, make abort idempotent. At minimum, calling it exactly once through `unwindStaged()` must be safe.

## 10.5 Preserve child-before-parent publication staging

The current recursive staging order prepares pending descendant scope projections before the parent scope publication.

Do not change this casually.

It ensures child projection targets exist/prep correctly before a parent publishes a semantic View that references those projections.

H3 changes types and ownership, not publication ordering.

## 10.6 Remove the non-transactional projection fallback

The current `ScopeProjection` still supports a legacy `install(output)` path when `preparePublication` is absent.

H3 should eliminate this fallback by the end of H3-D.

Every live retained projection target must support prepare/commit/abort.

Why:

```text
composition already promises batch atomicity
PERF-13 will rely on clear structural vs state/content transactions
an unprepared install path is a hidden second publication architecture
```

Migrate tests/fakes to use trivial prepared publications rather than preserving `install` forever.

A trivial in-memory test target can return:

```ts
{
  commit: () => { current = output; },
  abort: () => {},
}
```

## 10.7 `needsPublication` remains semantic-sideband escape hatch

The root currently needs publication when History sideband identity changes even if the body View object is identical.

Keep this capability.

Do not bake History into composition.

The target implementation owns the sideband closure and simply tells composition:

```text
same View identity, but target-specific committed sideband differs
```

This remains an acceptable structural publication concern.

---

# 11. Composition after H3

The composition directory should become straightforward to explain.

## 11.1 `compose.ts`

Owns:

```text
semantic slot allocation
same-semantic-value reuse checks
builder child reuse
modifier reuse
axis/grid immediate semantic comparison
component handle semantic identity comparison
```

Imports only semantic concepts.

Examples after H3:

```ts
const node = semanticNodeOf(previous);
if (
  node.kind === SEMANTIC_VIEW_KIND.text
  && semanticTextMatches(node, content)
) {
  stageReuse(slot, previous);
}
```

and:

```ts
if (
  node.kind === SEMANTIC_VIEW_KIND.container
  && node.child === semanticNodeOf(base)
) {
  stageReuse(slot, previous);
}
```

No bridge constants.

## 11.2 `execution.ts`

Owns:

```text
scope lifecycle
pending/committed props
State dependency commit/rollback
dirty queue
batch execution
publication staging
commit/abort
retry obligations
```

It validates View outputs with:

```text
semantic View brand/accessor
```

not structural lowering.

It does not know whether publication uses:

```text
N-API
direct FFI
in-memory tests
future transport implementation
```

## 11.3 `child-owner.ts`, `define-view.ts`, `execution-context.ts`, `tracked-state.ts`

These are already mostly on the correct side.

Do not opportunistically redesign them.

H3 should only update imports/types required by the semantic node/publication split.

## 11.4 `persistent-seq.ts`

Keep its algorithm intact.

Do not change branch factor, balancing, copy behavior, or performance counters as part of the seam split.

Only add a read-only semantic interface adapter if needed by transport consumers.

---

# 12. Structural transport after H3

Structural transport owns how semantic structural intent becomes native physical retention.

## 12.1 `ir.ts` becomes wire/bridge-only

Final `transport/structural/ir.ts` should contain only concepts that truly describe the bridge representation, for example:

```text
bridge schema constants
BridgeViewNode
BridgeStyleNode
BridgeColorNode
BridgeLayoutChild
BridgeGrid records
BridgeOverflow records
```

It must not own:

```text
semantic View identity
semantic NodeId allocator
semantic clone/merge helpers
semantic derivation WeakMaps
PersistentSeq implementation
composition reuse comparators
```

If the remaining file is tiny, merge it into `cold-lowering.ts` rather than keeping an abstract `ir.ts` for aesthetics.

## 12.2 Add explicit semantic encoding helpers

Recommended cohesive module:

```text
transport/structural/encoding.ts
```

It owns mappings such as:

```text
SemanticViewKind -> bridge/native kind
SemanticWrapMode -> native wrap code
SemanticHorizontalAlign -> native align code
SemanticVerticalAlign -> native align code
SemanticTrack -> trackWord
Semantic scalar change set -> native scalar patch mask
SemanticStyle -> native/bridge style representation
```

All ABI-specific packing belongs here or in generated helpers.

## 12.3 `retained-dag.ts` consumes semantic nodes directly

Change transaction types from:

```ts
Map<BridgeViewNode, number>
Set<BridgeViewNode>
```

conceptually to:

```ts
Map<SemanticViewNode, number>
Set<SemanticViewNode>
```

Likewise NativeRef hint WeakMaps are keyed by semantic identity.

The retained materializers switch on `SEMANTIC_VIEW_KIND`, then call explicit transport encoders before invoking generated ABI calls.

## 12.4 Rename bridge counters only if worth the churn

Current performance counters use names such as:

```text
bridge_semantic_nodes_inspected
bridge_children_visited
```

They can remain for H3 if benchmark tooling depends on the names.

If renamed to `semantic_nodes_inspected`, update all benchmark parsers and baselines in the same tranche.

Do not create noisy metric churn unless it improves clarity materially.

## 12.5 NativeRef hint ownership stays transport-side

The equivalent of current `BRIDGE_NATIVE` remains under structural transport.

The name should become semantic rather than bridge-specific, for example:

```text
NATIVE_HINTS
SEMANTIC_NATIVE_HINTS
```

but semantics stay:

```text
semantic node identity
    -> generation-scoped borrowed NativeRef hint
```

No NativeRef is stored in the semantic node.

## 12.6 Cold decoder remains the complete fallback

The retained direct path can refuse based on caps, unsupported fast family, stale recovery exhaustion, etc.

The fallback still:

```text
lowers complete semantic View to bridge object
calls safe N-API decoder
receives a leased native ref
```

Do not weaken the PERF-12 correctness fallback.

---

# 13. Native retained path / transaction metadata

`api/view/view.ts` currently contains `NativePathLineage`, native path kind tags, path step tags, native transaction edits, and helpers that patch `BridgeViewNode` paths.

That is transport knowledge in the semantic View owner.

H3 must remove it.

## 13.1 First inventory actual production use

Before moving anything, run:

```sh
rg -n "NativePath|nativePath|nativeTextLayoutTransaction|pathRoot|pathChild|editTxn" \
  packages/iyon-tui/src packages/iyon-tui/tests
```

Classify every result:

```text
production retained path
benchmark/oracle only
legacy test helper
dead
```

Do not preserve unused path machinery just because PERF-12 once used it.

## 13.2 If path machinery is still production-relevant

Move native-only types/builders to:

```text
transport/structural/retained-path.ts
```

The semantic layer may retain a transport-neutral derivation path if that path is genuinely semantic, for example:

```text
container child
row child index 4
grid semantic cell index 9
```

Transport converts that path to generated native path kind/selector words.

## 13.3 If path machinery is only oracle/test residue

Delete semantic View-side storage and keep only the transport/oracle helper necessary for parity tests.

Do not modify generated ABI symbols solely to delete TypeScript-side dead metadata unless that ABI cleanup is separately justified. Unused generated functions can remain until a later ABI version cleanup.

## 13.4 `patchBridgeTextPath()` must not survive in `api/view`

If the multi-edit semantic API still needs path-based text layout edits, implement the semantic edit against `SemanticViewNode` and emit a semantic derivation.

The bridge/native transaction encoder belongs in transport.

---

# 14. Runtime and control integration

H3 does not force every concrete structural publisher into `runtime/`.

The correct rule is:

> Composition defines the structural publication contract. Runtime/control owners implement that contract by adapting their concrete structural transport boundary.

## 14.1 Root publication

Current root publication already has the right orchestration order:

```text
producer evaluates
History sideband staged
structural root prepared
on failure: staged History restored / physical publication aborted
on commit: History binding + root publication
```

Keep this behavior.

The root target should be typed as `StructuralPublicationTarget` and call the retained structural boundary internally.

Composition never sees:

```text
NativeViewAbiSession
RetainedRootBoundary
rootRef
hostRenderRef
```

## 14.2 Component-scope projection

Current Tui runtime creates a ViewSlot for component scope projection.

This remains a good implementation.

The projection presented to composition should expose only:

```ts
interface StructuralScopeProjection {
  readonly view: View;
  readonly target: StructuralPublicationTarget;
  dispose(): void;
}
```

or the equivalent fields on the existing interface.

Do not expose ViewSlot native methods to composition.

## 14.3 ViewSlot and ScrollPane builder roots

These controls currently create `OwnedBuilderRoot` targets internally and use retained structural boundaries directly.

That is acceptable because the dependency is from the control owner to both systems; composition still depends only on the abstract target.

H3 may extract a small shared helper if both controls contain identical target adapters, but do not turn H3 into a complete control refactor.

The final rule is:

```text
composition -> no structural transport
control adapter -> composition contract + structural transport is allowed
```

## 14.4 Direct control setters remain direct

`slot.setView(view)` and `pane.setContent(view)` are ownership-changing direct operations, not composition publications unless builder mode is active.

Do not route all direct setters through `RetainedExecutionRuntime` merely to make the architecture look uniform.

---

# 15. Import and ownership enforcement

H3 needs machine gates. Prose is not enough.

Extend `tools/ownership/check.ts`.

## 15.1 Hard gate: composition cannot import transport

For every TypeScript source under:

```text
packages/iyon-tui/src/composition/
```

reject imports resolving under:

```text
packages/iyon-tui/src/transport/
```

No allowlist.

If a composition file needs a type currently owned by transport, that is evidence the type has the wrong owner.

## 15.2 Hard gate: structural transport cannot import composition

For every source under:

```text
packages/iyon-tui/src/transport/structural/
```

reject imports resolving under:

```text
packages/iyon-tui/src/composition/
```

No allowlist.

The read-only semantic sequence interface exists specifically to avoid an exception for PersistentSeq.

## 15.3 Hard gate: semantic View cannot import structural transport

Reject imports from:

```text
api/view/** -> transport/structural/**
api/view/** -> transport/abi/**
```

This is the gate that prevents future re-collapse of semantic View and structural wire representation.

## 15.4 Semantic naming guard

Add a conservative source-text check over `api/view/semantic*` and `composition/**` for transport-only names:

```text
NativeRef
BridgeViewNode
BRIDGE_VIEW_KIND
VIEW_BRIDGE_SCHEMA_VERSION
trackWord
pathRef
viewRefForNodeId
```

Do not make the regex so broad that public prose mentioning "native" fails. Keep it focused on structural implementation symbols.

## 15.5 Structural IR dependency guard

Reject `transport/structural/ir.ts` importing `composition/**`.

If `ir.ts` is deleted, update the gate to the remaining bridge/wire module.

## 15.6 Keep existing H2 gates

H3 extends the existing ownership checker. It must not weaken:

```text
framework import direction
runtime/native separation
public declaration closure
root export discipline
consumer deep-import prohibition
Rust framework purity
public surface snapshots
```

---

# 16. File-level disposition

The implementation agent should use this as the default mapping.

| Current file | H3 disposition |
| --- | --- |
| `composition/compose.ts` | Keep; replace bridge inspection/lowering with semantic-node inspection. |
| `composition/execution.ts` | Keep; move publication interfaces out if useful; replace `nodeForBridge` validation. |
| `composition/child-owner.ts` | Keep; semantic-only, minimal changes. |
| `composition/define-view.ts` | Keep; semantic-only, minimal changes. |
| `composition/execution-context.ts` | Keep; no transport knowledge. |
| `composition/tracked-state.ts` | Keep; no transport knowledge. |
| `composition/persistent-seq.ts` | Keep algorithm/owner; expose only a read-only semantic sequence interface outside composition. |
| `api/view/view.ts` | Major H3 cut: construct semantic nodes, not bridge nodes; remove ABI schema/path packing. |
| `api/view/semantic-node.ts` | New private owner for SemanticViewNode, semantic identity, sidecars, semantic sequence interfaces. |
| `api/presentation/semantic-style.ts` | New private semantic normalization owner if not cleanly colocated with `view.ts`. |
| `transport/structural/view-bridge.ts` | Transitional only; delete by H3-D. |
| `transport/structural/ir.ts` | Shrink to physical bridge/wire records or merge into cold-lowering. |
| `transport/structural/style-lowering.ts` | Keep only semantic -> physical lowering; no composition callers. |
| `transport/structural/retained-dag.ts` | Retain; change input/key types to semantic nodes; own ABI encoding. |
| `transport/structural/native-view-abi.ts` | Retain; consume semantic identity helpers; remove API-owned native path metadata. |
| `transport/structural/component-view.ts` | Remove semantic View construction; delete or reduce to native component-ID resolution. |
| `transport/structural/cold-lowering.ts` | New complete SemanticViewNode -> BridgeViewNode fallback mapper. |
| `transport/structural/encoding.ts` | New if it keeps ABI tag/packing logic cohesive. |
| `runtime/runtime.ts` | Keep as root integration owner; type targets against composition publication contract. |
| `api/controls/view-slot.ts` | Keep behavior; migrate to semantic/cold lowerer names and structural publication target type. |
| `api/controls/scroll-pane.ts` | Same as ViewSlot. |
| `tools/ownership/check.ts` | Add H3 import/naming gates. |
| `ARCHITECTURE.md` | Update with H3 dependency direction and semantic vs physical retention. |

Do not create every proposed file if the resulting module would be tiny. The ownership boundaries are normative; exact granularity is not.

---
# 17. Delivery strategy: five stacked tranches

H3 is deliberately split into five merge-request-sized tranches.

The tranches are stacked on one feature branch lineage:

```text
main
  |
  +-- H3-A semantic foundation
        |
        +-- H3-B composition cutover
              |
              +-- H3-C transport cutover
                    |
                    +-- H3-D publication/residual cleanup
                          |
                          +-- H3-E enforcement + final gates
```

Review each tranche independently. Do not merge an incomplete prefix to `main` if it leaves a transitional dual representation or compatibility alias that later tranches are required to delete.

Every tranche must compile and pass its listed correctness checks. H3-C and H3-E additionally carry strict performance gates because H3-C is where the final hot structural path exists.

---

# 18. H3-A - Semantic View foundation and equivalence oracle

## 18.1 Goal

Create a backend-neutral semantic node vocabulary and prove that it can represent every current View exactly, without yet deleting the current bridge-backed path.

This tranche is about definitions, adapters, and differential tests.

It should not change production rendering routes.

## 18.2 Required work

### A1. Add private semantic node types

Create the semantic View union under `api/view/`.

Cover every current kind:

```text
text
diff
spacer
row
column
grid
hanging
container
clamp
contentMax
component
decorated
```

Do not omit low-frequency kinds.

### A2. Add semantic normalized presentation records

Create semantic copies/normalizers for:

```text
color
style
border
decoration
style state/facts
text-span style
overflow style
```

Preserve all current validation.

### A3. Add semantic track/grid/overflow records

Define transport-independent values for:

```text
axis child participation
grid tracks
grid cell placement/alignment
overflow indicator
wrap/alignment
```

No bridge enum imports.

### A4. Define semantic derivation types

Model every current retained derivation without ABI packing.

Add tests that semantic derivations carry enough information to reproduce the current retained ABI operation.

### A5. Define semantic sequence override interfaces

Introduce read-only sequence interfaces and semantic wide-axis/grid override records.

Do not alter `PersistentSeq` implementation.

### A6. Build a temporary bridge <-> semantic equivalence oracle

During migration only, add a test/helper that translates the current authoritative BridgeViewNode into the new SemanticViewNode representation, or compares independently constructed semantic/bridge records.

The purpose is to prove field coverage before production cutover.

Do not make this translator the permanent production architecture.

## 18.3 Required tests

For each View family, construct representative values and verify semantic equivalence to the current bridge representation:

```text
plain/styled text
all wrap/alignment modes
diff with terminated/unterminated lines
spacer
row/column all track families
grid tracks/spans/alignment/gaps
hanging
container
clamp all overflow variants
contentMax
component
nested decoration
bounds/padding/fill/fit
foreground/background/border/style state
```

Also test:

```text
fresh View gets one semantic NodeId
clone/reuse keeps existing NodeId
semantic child references preserve identity
public object mutation after construction cannot mutate stored semantic snapshot
```

## 18.4 H3-A stop gate

Do not start H3-B until:

```text
semantic vocabulary covers every current View kind
no semantic type imports bridge-schema/generated ABI
semantic style normalization matches existing construction semantics
differential oracle passes
current production tests are unchanged and green
```

## 18.5 H3-A non-goals

Do not yet:

```text
switch compose.ts
switch retained-dag.ts
delete view-bridge.ts
change NativeRef hint keys
change publication interfaces
change control ownership
```

## 18.6 H3-A completion report

Record:

```text
new semantic modules
covered View kinds
normalization equivalence test matrix
known transitional compatibility helpers
confirmation that production rendering route is unchanged
```

---

# 19. H3-B - Make semantic View authoritative; cut composition off transport

## 19.1 Goal

Switch the semantic side of the framework to the new representation.

At the end of H3-B:

> `composition/**` imports no `transport/**` module.

A temporary structural compatibility lowerer is allowed so structural transport can continue working while H3-C is prepared.

## 19.2 Required work

### B1. Switch `View` construction to SemanticViewNode

`api/view/view.ts` must install semantic nodes as the authoritative node for new Views.

The following construction-time properties must remain identical in meaning:

```text
NodeId
kind
children
layout tracks
styles/decorations
bounds
text/diff data
derivation relationships
wide sequence metadata
```

### B2. Replace bridge helper names in semantic code

Replace:

```text
nodeForBridge
setViewNode
BRIDGE_VIEW_KIND
BridgeViewNode
BridgeLayoutChild
BridgeGrid*
BridgeOverflow*
```

with semantic equivalents inside `api/view` and `composition`.

### B3. Convert `compose.ts` comparators

Every existing reuse decision must operate on semantic records.

Important paths:

```text
text exact reuse
styled text reuse
decoration delta reuse
style/color/border reuse
component reuse
row/column immediate child reuse
grid immediate semantic reuse
hanging/container/clamp/contentMax reuse
diff reuse
wide sequence bailout behavior
```

Do not simplify comparisons unless a separate test proves behavior is equivalent.

### B4. Replace execution View validation

Use the semantic accessor/brand in `execution.ts`.

A malformed object must fail before publication preparation.

### B5. Resolve component semantic identity

Change semantic component nodes to use `FrameworkHandle.id` / `HandleId`, not native ComponentId.

Composition no longer calls transport to obtain component identity.

Add the temporary transport-side HandleId lookup required for existing bridge lowering.

### B6. Preserve a temporary complete transport compatibility route

Until H3-C, transport may obtain a complete BridgeViewNode through the new cold/compatibility lowerer.

This route must be:

```text
complete
correct
clearly marked transitional for hot retained paths
```

If the hot path temporarily uses compatibility lowering in this tranche, do not merge H3-B to `main` independently. The whole H3 stack is the merge unit.

## 19.3 H3-B ownership gate

Add the first new hard gate now:

```text
composition/** -X-> transport/**
```

No allowlist.

H3-B cannot be declared complete while `compose.ts` or `execution.ts` imports structural transport.

## 19.4 H3-B correctness tests

In addition to the full TypeScript suite, add focused retained-execution tests:

```text
same State value -> no semantic View replacement
same text -> same View identity
same decoration -> same View identity
one changed modifier -> one new semantic identity on expected frontier
keyed child identity unchanged
unkeyed occurrence semantics unchanged
failed component body -> committed output unchanged
failed publication prepare -> semantic rollback unchanged
component handle reuse -> same semantic component View
component disposed -> deterministic failure at same observable boundary
```

## 19.5 H3-B work counters

Capture before/after counters for representative no-op composition cases.

Hard requirement:

```text
no increase in semantic View constructions on no-op reuse paths
no increase in execution scope body calls
no new NodeId on a proven semantic no-op
```

Do not accept "the screen is identical" if H3 made no-op composition allocate new semantic nodes.

## 19.6 H3-B completion report

Record:

```text
all composition -> transport imports removed
View semantic node is authoritative
component semantic identity strategy
which compatibility lowering remains for H3-C
no-op composition counter comparison
```

---

# 20. H3-C - Structural transport consumes semantic nodes directly

## 20.1 Goal

Remove the inverse coupling and restore/confirm the final retained hot path.

At the end of H3-C:

> `transport/structural/**` imports no `composition/**` implementation.

and:

> ordinary retained materialization does not allocate a complete BridgeViewNode tree.

## 20.2 Required work

### C1. Convert `MaterializeTx` to semantic node identity

Change transport-local collections and hint sidecars to use `SemanticViewNode`.

Preserve:

```text
transaction-local cycle detection
borrowed hint semantics
temporary lease lists
native lookup ceiling
one stale-ref retry
scratch reuse
root lease transfer
```

### C2. Convert every retained materializer

Update materializers for every semantic kind.

Use explicit encoding functions for:

```text
view kind
wrap/alignment
axis track
grid track
grid cell span/alignment
style/color/border
overflow
common scalar patch
```

No direct assumption that semantic numeric tags equal ABI numeric tags.

### C3. Convert derivation fast paths

Map semantic derivations to current generated retained calls.

For each existing derivation family, preserve the same fallback and stale recovery behavior.

Required families:

```text
textLayout
commonScalar
axisSet
axisSplice
gridCell
```

### C4. Convert wide sequence reads

Use only the read-only semantic sequence interface.

Remove structural transport imports of `composition/persistent-seq.ts`.

Preserve lazy behavior and caps.

### C5. Keep cold lowering separate

Retained hot path:

```text
SemanticViewNode -> generated ABI
```

Cold path:

```text
SemanticViewNode -> cold BridgeViewNode -> N-API decode
```

### C6. Shrink structural IR

Move semantic helper functions/sidecars out of `transport/structural/ir.ts`.

Delete compatibility aliases that make SemanticViewNode and BridgeViewNode the same TypeScript type.

At the end of C, the compiler should make accidental cross-use inconvenient.

## 20.3 H3-C ownership gate

Add:

```text
transport/structural/** -X-> composition/**
```

No allowlist.

## 20.4 H3-C hard performance gates

This is the first tranche with the final hot path. Run the existing PERF-12 structural benchmarks/counters.

Required work invariants:

### Warm exact root

A generation-valid exact-root NativeRef hint should still require effectively constant work:

```text
no semantic DAG traversal
no bridge object tree construction
one root/native lookup/install class of work as before
```

### Small changed frontier

One leaf/text/decorative change in a large retained tree must inspect/materialize only the changed semantic frontier plus required ancestors.

### Wide axis/grid

Single edit remains logarithmic in PersistentSeq metadata and does not flatten the full sequence.

### NativeRef/lease behavior

No additional permanent lease.
No temporary lease survives abort.
No hint is treated as an owned lease without promotion.

### Timing

Work counters are the hard gate.

Timing is secondary because local benchmark noise is real. Investigate a repeatable >5% median regression on representative retained cases; do not waive a counter regression because wall-clock timing happened to look flat.

## 20.5 H3-C memory gate

After H3-C there must not be two retained JS DAGs per View.

Verify:

```text
semantic nodes converge with View lifetime
NativeRef hints remain weak/generation-scoped
cold bridge cache, if present, is weak
repeated publications do not monotonically grow bridge-object retention
```

## 20.6 H3-C completion report

Record:

```text
retained-dag input type before/after
all semantic -> ABI encoders added
transport -> composition imports removed
cold lowering path
PERF-12 counter comparison
memory convergence result
```

---

# 21. H3-D - Publication seam, native-path cleanup, and deletion of compatibility architecture

## 21.1 Goal

Finish the architectural separation and remove migration scaffolding.

H3-D should leave one semantic View representation, one hot retained structural path, and one cold fallback path.

## 21.2 Required work

### D1. Extract/name the structural publication contract

Move the generic prepared publication interfaces to a focused composition-owned module if they still live inside `execution.ts`.

Use names that make the structural scope clear.

Update:

```text
root target
component-scope projection
ViewSlot builder target
ScrollPane builder target
test fakes
```

### D2. Remove legacy non-transactional projection install

Require prepared structural publication for retained scope projections.

Delete the `install(output)` fallback from composition execution after all users migrate.

### D3. Move/delete native path metadata

Complete the inventory from section 13.

No `NativePath*`, bridge path kind, bridge path patcher, packed transaction code, or generated path concepts remain under `api/view`.

### D4. Delete `view-bridge.ts`

No semantic View association remains under transport.

Update every caller to use either:

```text
semanticNodeOf(view)
```

for semantic/hot retained work, or:

```text
lowerColdView(view)
```

for complete object fallback.

### D5. Delete/repurpose `component-view.ts`

Semantic component View creation belongs to API/view.

Keep only a clearly physical component ID resolver if needed.

### D6. Remove migration-only aliases and dual sidecars

Search and delete:

```text
legacyBridgeNode
semanticFromBridge
bridgeFromSemantic compatibility aliases used by hot paths
dual View-node WeakMaps
old Bridge derivation sidecars
old Bridge sequence override sidecars
```

The cold lowerer is not a migration shim; it is the final fallback architecture.

### D7. Update architecture documentation

Update `ARCHITECTURE.md` so it states:

```text
api/view owns immutable semantic View data
composition owns semantic execution/reuse
runtime/control owners bind structural publication targets
transport/structural owns physical retention/lowering
```

Clarify that PersistentSeq semantics are transport-independent even if its implementation remains under `composition/`.

## 21.3 H3-D source scan gate

Run targeted scans and explain every remaining hit:

```sh
rg -n "nodeForBridge|setViewNode|BridgeViewNode|BRIDGE_VIEW_KIND|VIEW_BRIDGE_SCHEMA_VERSION" \
  packages/iyon-tui/src/api packages/iyon-tui/src/composition

rg -n "composition/" packages/iyon-tui/src/transport/structural

rg -n "NativeRef|viewRefForNodeId|pathRef|trackWord" \
  packages/iyon-tui/src/composition packages/iyon-tui/src/api/view
```

Expected final result:

```text
composition: zero structural transport concepts
api/view: zero bridge/ABI/native-retention concepts
transport/structural: zero composition implementation imports
```

If a hit is only documentation explaining a boundary, the checker may ignore comments. Production symbol references are not allowed.

## 21.4 H3-D completion report

Record:

```text
publication contract final shape
legacy install fallback deletion
native path disposition
view-bridge deletion
component-view disposition
all migration aliases deleted
final source-scan output
```

---

# 22. H3-E - Enforcement, parity, performance, and integration

## 22.1 Goal

Make the architecture difficult to regress and prove H3 is behavior/performance neutral.

No new architecture work should be invented here unless a gate finds a real defect.

## 22.2 Ownership checker

Add final rules described in section 15.

The checker error should name:

```text
source file
target file
violated H3 rule
```

Example:

```text
FAIL h3-composition-transport-seam - composition/compose.ts imports transport/structural/ir.ts
```

## 22.3 Declaration/public surface gate

H3 adds only private internals.

The H1 public surface snapshot should be unchanged.

Run:

```sh
bun run typecheck
bun run check:tui-declarations
bun run check:ownership
```

Any public declaration change requires explicit review. Do not regenerate public surface snapshots just to make H3 pass unless a genuine, separately approved public change occurred.

## 22.4 Structural ABI gate

H3 should not require structural schema changes.

Run:

```sh
bun run check:tui-abi
```

Generated structural artifacts should be unchanged unless a discovered correctness bug truly requires an ABI change.

If generated files change solely because H3 moved TypeScript semantic ownership, stop and investigate.

## 22.5 Full framework tests

Run:

```sh
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
cargo test --workspace
```

Also run clippy/format if any Rust source changed.

## 22.6 PERF-12 non-regression suite

Use the existing retained structural benchmark/counter tooling and authoritative benchmark artifacts as the baseline.

The final report must compare at least:

```text
exact root reuse
small text change
small scalar/decorative change
wide axis set/splice
wide grid cell change
multi-edit transaction
cold materialization
stale-ref recovery
memory convergence
```

Required invariants:

```text
same or fewer semantic View constructions for equivalent workload
same scope executions
same NodeId creation count
same unchanged-subtree cutoff
same retained vs cold route decisions for equivalent input
same NativeRef lease convergence
no bridge-object construction on warm retained path
no new full-tree flattening
```

## 22.7 N-API/direct parity

Keep existing structural parity/oracle coverage.

H3 moves TypeScript ownership; it must not change the semantic meaning seen by Rust.

Compare equivalent semantic Views through:

```text
safe N-API cold decode
retained generated N-API structural path
direct/qualification oracle where the feature is available
```

Rendered result and retained Rust semantic structure must remain equivalent.

## 22.8 Iyon external integration

Use the established external consumer workflow after all framework gates pass.

Do not add Iyon-specific behavior or tests to this repository to make integration pass.

The integration check is looking for accidental public/deep-import breakage caused by H3.

## 22.9 Final H3-E report

The completion report must contain:

```text
baseline and final commits
final relevant src tree
composition import scan
transport import scan
semantic View import scan
public surface result
ABI generation/check result
unit/integration test results
PERF-12 counter comparison
memory convergence result
N-API/direct parity result
external consumer result
all deleted compatibility modules
remaining debt explicitly deferred to PERF-13 or another task
```

"Tests pass" is not a sufficient architecture completion report.

---
# 23. Detailed correctness matrix

The following is the minimum H3 correctness matrix. Existing tests that already cover a row should be reused and strengthened instead of duplicated.

## 23.1 Semantic construction

| Case | Required assertion |
| --- | --- |
| `View.text("x")` | semantic kind/text/wrap/align match existing behavior; one NodeId. |
| styled spans | stored style is a construction-time semantic snapshot. |
| diff | ranges, line kinds, termination, line numbers preserved. |
| spacer | row count validation/semantics unchanged. |
| row/column | child order, track type, scalar values, gap preserved. |
| grid | tracks, cells, spans, alignments, gaps, placement indices preserved. |
| hanging | prefix/continuation/body semantic child identity preserved. |
| container | exactly one semantic child; no implicit transport wrapper. |
| clamp | max rows and overflow semantics preserved. |
| contentMax | max rows semantics preserved. |
| component | semantic HandleId preserved; native ID not present. |
| decorated | flatten/merge behavior equivalent to current construction. |

## 23.2 Composition reuse

| Case | Required assertion |
| --- | --- |
| same text twice in same retained slot | second evaluation returns previous View object. |
| changed text | new View only for changed semantic occurrence/frontier. |
| same StyleRef/StyleSpec value | reuse decision matches pre-H3. |
| same style state map | reuse. |
| one style-state value changed | only expected semantic occurrence changes. |
| same axis children and gap | parent reused. |
| one child identity changed | parent replaced; unchanged siblings retained. |
| wide axis | no equality scan that flattens sequence. |
| same grid | reuse decision matches pre-H3. |
| wide grid | no full flatten merely to prove reuse. |
| component same handle | reuse. |
| component different handle with same native shape | no reuse. |

## 23.3 Transactional publication

Test these with an in-memory structural publication target; do not require native transport for all cases.

```text
prepare succeeds, commit succeeds
prepare refuses
first child prepare succeeds, parent prepare refuses
several children prepare, later sibling refuses
abort cleanup throws
commit throws path is surfaced as pathological failure
same semantic View but needsPublication() true
same semantic View and needsPublication() false
```

Assertions:

```text
no prepared target commits before all preparation succeeds
aborted batch does not advance projectedOutput
pending semantic State/dependencies roll back
fresh uncommitted scopes are disposed after rollback
previous committed output remains authoritative
```

## 23.4 Structural hot/cold equivalence

For every View family:

```text
semantic View
   |-- retained structural materialization
   `-- cold BridgeViewNode + N-API decode
```

must create semantically equivalent Rust Views.

Use rendered screen/cell output plus existing native semantic/ABI parity hooks where available.

## 23.5 Derivation parity

For each derivation, compare:

```text
A. construct final View from scratch, cold decode
B. base View + retained semantic derivation fast path
```

Required equality:

```text
Rust semantic output
screen rows
cell styles
NodeId publication mapping
```

Required work difference:

```text
B does not inspect/rebuild unrelated semantic subtree
```

## 23.6 Handle/disposal behavior

Tests:

```text
same component HandleId lowers to same live native component ID
handle disposed before later cold lowering -> disposed-handle failure
handle disposed after already committed native View -> existing native lifecycle remains as today
HandleId lookup does not retain raw resource after release
creating/destroying many component handles converges
```

Do not accidentally convert explicit disposal into GC-only lifetime.

## 23.7 Cold fallback completeness

Force retained fast-path refusal for representative reasons:

```text
text over retained byte cap
axis over direct-ref cap
unsupported custom border/style shape
forced no-session/direct availability in test seam
stale-ref recovery exhausted
```

Cold fallback must still render correctly.

## 23.8 History/root sideband

H3 must preserve root publication rules introduced by the recent retained-frame fixes.

Test:

```text
body View unchanged, History binding changes
History component invalidation that changes body allocation
failed root prepare while staged History differs
retry after failed prepare
```

The last successfully committed frame and History binding remain authoritative on failure.

## 23.9 ViewSlot / ScrollPane

Test both ownership modes:

```text
builder -> builder update
builder -> direct takeover
direct -> builder takeover
failed direct prepare retains old builder/content
failed builder publication retains old direct content
ScrollPane viewport/follow-end state unchanged across content rebuild
```

H3 must not alter their existing lifecycle/ownership semantics.

---

# 24. Performance methodology

H3 is an architecture refactor on top of PERF-12. A small-looking abstraction can destroy the very work reduction PERF-12 established.

Use work ownership first, wall-clock timing second.

## 24.1 Capture a baseline before H3-A

On the H3 branch point, record:

```text
commit SHA
Bun version
platform/architecture
benchmark command
every relevant perf counter
median timing where stable
memory snapshot where available
```

Do not compare final H3 to an old PERF-12 historical commit if current `main` has subsequent retained fixes. Compare to H3's exact branch point.

## 24.2 Hard work counters

Representative counters include the existing families around:

```text
View constructions/clones
N-API nodes seen/cache hits/cache misses
semantic/bridge nodes inspected
children visited
resolver visits
component view/capability calls
measure/prepare/layout nodes
paint nodes/cells/cache
PersistentSeq nodes/leaf/branch clones
PersistentSeq items iterated during patch
transport scratch reuse/ref words/bytes where exposed
```

The implementation may rename counters only with a mapping in the report.

## 24.3 H3-specific new counters

Add counters only if needed to prove the seam. Useful temporary/permanent candidates:

```text
cold_bridge_nodes_lowered
cold_bridge_objects_allocated
semantic_to_abi_kind_maps
semantic_derivation_encodes
```

The important proof is:

```text
warm retained publication -> cold_bridge_objects_allocated == 0
```

Do not create instrumentation so invasive that it changes the hot path when disabled.

## 24.4 Workload set

Use at least:

```text
10k-node stable tree, no-op State update
10k-node tree, one text leaf changes
10k-node tree, one decorative scalar changes
2k/10k/wide axis one child replacement
wide axis splice
large grid one cell replacement
multi-edit text transaction
cold large text
repeated root replacement with shared subtrees
1000 repeated exact-root publications
component handle projection reuse
```

## 24.5 Acceptance policy

Hard:

```text
no extra scope execution
no extra semantic View construction on no-op paths
no full semantic tree scan on warm exact-root hit
no bridge DAG allocation on retained hot path
no new O(N) wide-axis/grid flattening
no NativeRef/lease growth
```

Timing:

```text
<=5% repeated median regression: normally acceptable noise if work counters are identical
>5% repeated regression: investigate
>10% repeated regression: stop unless a measured correctness tradeoff is explicitly approved
```

Do not game timing by removing correctness checks or instrumentation that existed at baseline.

---

# 25. Failure semantics

H3 must keep failures boring and transactional.

## 25.1 Invalid semantic View

If user/component code returns an object that is not a framework View:

```text
semanticNodeOf() fails
before structural prepare
```

Do not let transport discover this later through an opaque bridge error.

## 25.2 Unsupported semantic-to-ABI fast encoding

If retained encoding cannot handle a semantic shape:

```text
retained path refuses
cold lowering is attempted
```

This is not an architecture error if cold lowering is complete.

## 25.3 Cold lowering bug

If a valid semantic View cannot be completely cold-lowered, that is a correctness bug.

Do not silently render a spacer/empty fallback.

## 25.4 Missing/disposed component resource

Resolve by HandleId at transport lowering.

Failure must be a normal framework disposed/missing resource error, not a null/zero component ID sent to Rust.

## 25.5 Publication prepare failure

Abort every already prepared publication in the batch.

Do not commit semantic `projectedOutput`.

Do not bind staged sideband state.

## 25.6 Abort cleanup failure

Aggregate cleanup errors as today. Preserve the original prepare failure when possible and surface cleanup failures rather than swallowing them.

## 25.7 Commit failure

Commit is expected to be infallible after prepare. A throw is pathological and visible.

Do not add a broad catch that converts commit exceptions into normal retained fallback after semantic commit has started.

## 25.8 Runtime generation change

Generation-scoped hints remain invalid outside their generation.

Semantic nodes are generation-neutral.

On generation change:

```text
semantic View remains valid
old NativeRef hint invalid
transport rematerializes/re-promotes as existing policy requires
```

This distinction is one of the reasons H3 exists.

---

# 26. Rejected alternatives

The implementation agent should not reopen these without review.

## 26.1 Keep BridgeViewNode but rename it SemanticViewNode

Rejected.

If it still contains bridge schema version, ABI numeric tags, packed track words, and native path data, it remains physical transport IR under a semantic name.

## 26.2 Composition emits `SemanticGraphDelta`

Rejected for H3.

It duplicates retained identity/derivation data and creates a new protocol with no demonstrated need.

## 26.3 Composition calls a giant HostConfig/Applier interface for every child operation

Rejected.

The existing immutable View + retained publication model is more appropriate. Research systems justify the boundary, not the exact callback vocabulary.

## 26.4 Transport imports PersistentSeq because it is "only a data structure"

Rejected.

That recreates the reverse dependency. Use a read-only semantic sequence interface.

## 26.5 Permanent dual semantic + bridge DAGs

Rejected.

Cold bridge objects are derived fallback data, not a second retained source of truth.

## 26.6 Move all structural materialization into composition

Rejected.

That is the opposite of H3 and would make PERF-13 state/content separation worse.

## 26.7 Route future state/content through `StructuralPublicationTarget`

Rejected.

The contract is structural only. PERF-13 state/content operations bypass semantic composition and use their own transport owners.

## 26.8 Make every `api/controls` implementation transport-free in H3

Rejected as unnecessary scope expansion.

Control implementations may own private native resources and structural boundaries. The required H3 seam is that composition does not know those mechanics and the semantic View core is not transport IR.

## 26.9 Change Rust View ABI while doing H3

Rejected unless a real correctness blocker is discovered.

H3 is specifically valuable because the same physical ABI can be fed by a cleaner semantic layer.

---

# 27. Review pitfalls

## 27.1 Hidden type alias coupling

Bad:

```ts
type SemanticViewNode = BridgeViewNode;
```

or:

```ts
export { BRIDGE_VIEW_KIND as SEMANTIC_VIEW_KIND };
```

A transitional alias may exist on the feature branch, but not at H3-D completion.

## 27.2 Numeric casts instead of mapping

Bad:

```ts
viewCreate(kind as number, ...)
```

when `kind` is semantic.

Require explicit exhaustive mapping at the boundary.

## 27.3 Accidental full-tree cold lowering on hot path

A clean import graph can still be a performance disaster.

Inspect flame/counters and source to verify retained materialization consumes semantic nodes directly.

## 27.4 Semantic equality by deep JSON

Do not introduce:

```ts
JSON.stringify(node)
```

or generic recursive deep equality.

Keep the current targeted immediate semantic comparisons and identity cutoffs.

## 27.5 Eager PersistentSeq flattening

Watch for convenience helpers that turn sequence interfaces into arrays before every ABI call.

Wide edits must remain bounded.

## 27.6 Native component ID smuggled back through a semantic field

Do not rename `handle` to `componentKey` while continuing to store `native.componentId()`.

The semantic component node uses JS-local HandleId.

## 27.7 Bridge schema included "for validation"

The semantic node does not carry bridge schema version.

Schema compatibility is checked when the native ABI session/cold bridge is used.

## 27.8 Public export leakage

New semantic internals must not appear in `@iyon/tui` declarations.

They are framework implementation details, not extension contracts.

---

# 28. Explicit non-goals

API-H3 must not:

- redesign public View methods;
- change H1 naming decisions;
- implement retained mutable geometry/presentation state;
- implement ContentPort/Source/Funnel/Connector;
- implement cold/buffered/hot content semantics;
- add state/content ABIs;
- rewrite Rust layout/paint;
- change the box model;
- delete decoration wrappers for semantic reasons;
- change History architecture;
- redesign ScrollPane semantics;
- redesign animation scheduling;
- change State<T> scheduling;
- change defineView keyed/unkeyed identity;
- add a property-level binding compiler;
- add a generic graph-delta language;
- add third-party dependencies;
- expose NodeId/NativeRef publicly;
- change structural ABI schema merely to make the TypeScript split easier.

These belong to PERF-13 or separate work.

---

# 29. Final target tree

The exact file count is not normative, but a healthy final shape is approximately:

```text
packages/iyon-tui/src/
|-- api/
|   |-- view/
|   |   |-- view.ts
|   |   |-- semantic-node.ts        # private
|   |   |-- geometry.ts
|   |   `-- scene.ts
|   |-- presentation/
|   |   |-- style.ts
|   |   |-- semantic-style.ts       # private if needed
|   |   `-- ...
|   `-- controls/
|       `-- ...
|
|-- composition/
|   |-- publication.ts              # optional focused extraction
|   |-- compose.ts
|   |-- define-view.ts
|   |-- execution.ts
|   |-- execution-context.ts
|   |-- child-owner.ts
|   |-- persistent-seq.ts
|   `-- tracked-state.ts
|
|-- runtime/
|   `-- ...
|
`-- transport/
    |-- structural/
    |   |-- retained-dag.ts
    |   |-- native-view-abi.ts
    |   |-- encoding.ts             # if warranted
    |   |-- cold-lowering.ts
    |   |-- retained-path.ts        # only if still needed
    |   |-- policy.ts
    |   `-- ir.ts                   # wire-only, or merged away
    |-- native/
    `-- abi/structural/
```

Expected deletion/repurpose candidates:

```text
transport/structural/view-bridge.ts
transport/structural/component-view.ts
transport-owned semantic derivation sidecars
transport-owned semantic sequence sidecars
API-owned native path/packed transaction metadata
legacy non-transactional projection install path
```

---

# 30. Acceptance checklist

H3 is complete only when all are true.

## Architecture

- [ ] `View` is backed by a backend-neutral private semantic node.
- [ ] Semantic node carries no bridge schema version.
- [ ] Semantic kind tags are not aliases of generated bridge tags.
- [ ] Composition imports no transport module.
- [ ] Structural transport imports no composition module.
- [ ] `api/view` imports no structural transport/ABI module.
- [ ] NativeRef/generation/lease data remains transport-owned.
- [ ] PersistentSeq remains a semantic-retention optimization hidden behind a read-only transport-facing interface.
- [ ] Component semantic nodes use HandleId rather than native ComponentId.
- [ ] Structural publication contract names only semantic View + prepare/commit/abort.
- [ ] State/content are not routed through that contract.

## Deletion

- [ ] `view-bridge.ts` deleted.
- [ ] semantic View construction removed from `component-view.ts` and file deleted/repurposed.
- [ ] bridge semantic derivation sidecars removed.
- [ ] bridge semantic sequence sidecars removed.
- [ ] native path metadata removed from `api/view`.
- [ ] legacy projection `install()` fallback removed.
- [ ] no permanent semantic+bridge dual DAG remains.

## Correctness

- [ ] all View families lower identically through retained and cold paths.
- [ ] no-op composition reuse decisions match baseline.
- [ ] transactional prepare/abort/commit behavior unchanged.
- [ ] History sideband failure/retry behavior unchanged.
- [ ] ViewSlot/ScrollPane ownership behavior unchanged.
- [ ] component disposal behavior unchanged.
- [ ] Unicode/rendered cell correctness unchanged.

## Performance

- [ ] no extra View constructions on no-op workloads.
- [ ] no extra scope executions.
- [ ] warm exact-root path performs no cold bridge construction.
- [ ] small mutation remains changed-frontier work.
- [ ] wide axis/grid edits preserve PersistentSeq asymptotics.
- [ ] NativeRef/lease memory converges.
- [ ] no persistent duplicate bridge graph.

## Tooling

- [ ] `bun run check:tui-abi`
- [ ] `bun run typecheck`
- [ ] `bun run check:tui-declarations`
- [ ] `bun run check:ownership`
- [ ] `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests`
- [ ] `cargo test --workspace`
- [ ] Rust format/clippy if Rust touched
- [ ] external Iyon consumer integration

---

# 31. Required implementation-agent completion report

The final implementation response must include all of the following.

## 31.1 Baseline

```text
H3 branch-point SHA
final SHA
Bun version
platform used for PERF gates
```

## 31.2 Final dependency proof

Paste the final import-scan/checker results proving:

```text
composition -X-> transport
transport/structural -X-> composition
api/view -X-> structural transport/ABI
```

## 31.3 Semantic representation

Describe:

```text
SemanticViewNode owner
NodeId owner
semantic style owner
semantic derivation owner
wide sequence interface
component HandleId representation
```

## 31.4 Physical representation

Describe:

```text
semantic -> ABI encoding owner
NativeRef hint owner
cold bridge lowering owner
component HandleId -> native ComponentId resolution
native path disposition
```

## 31.5 Publication seam

Show the final TypeScript contract for:

```text
prepared structural publication
structural publication target
scope projection
```

and list its concrete root/component/control adapters.

## 31.6 Deleted paths

List every migration-only or superseded file/helper removed.

## 31.7 Gates

Report exact commands and results for:

```text
ABI check
typecheck
declaration closure
ownership
TS tests
Rust tests
PERF-12 retained benchmarks
memory convergence
N-API/direct parity
external consumer integration
```

## 31.8 Remaining debt

Remaining work must be categorized as:

```text
PERF-13 state plane
PERF-13 content plane
future structural optimization
unrelated public API debt
```

Do not leave "temporary" H3 bridge aliases or TODO publication paths for PERF-13 to clean up.

---

# 32. Decision ledger

This section is normative and exists so the implementation agent does not repeatedly reopen settled architecture questions.

| Question | H3 decision |
| --- | --- |
| Who owns semantic View identity? | `api/view` private semantic model. |
| Who owns scope identity/reuse? | `composition`. |
| Who owns NativeRef? | `transport/structural`. |
| Who owns bridge schema/numeric tags? | `transport/structural` / generated ABI. |
| Does composition inspect BridgeViewNode? | No. |
| Does transport import PersistentSeq implementation? | No. |
| Does H3 add SemanticGraphDelta? | No. |
| Does H3 add insert/remove/move host operations? | No. |
| Does View retain bridge schema version? | No. |
| Are semantic kind codes ABI kind codes? | No; explicit mapping. |
| Does NodeId remain semantic? | Yes. |
| Is NodeId public? | No. |
| Are semantic child edges direct? | Yes, preferably SemanticViewNode references for hot identity comparisons. |
| Does semantic style copy caller values? | Yes, preserving current snapshot behavior. |
| Where does style ABI encoding occur? | Structural transport. |
| Are derivation hints retained? | Yes. |
| Are derivation ABI masks retained in semantic data? | No. |
| Are wide sequence overrides retained? | Yes, in semantic form. |
| Does wide sequence flatten at publication? | No. |
| What identifies semantic component attachment? | Framework HandleId. |
| What identifies native component placement? | Native ComponentId resolved in transport. |
| Does semantic component View keep native ID? | No. |
| Who resolves HandleId -> native resource? | Transport native/structural private seam fed by runtime handle registration. |
| Is cold object bridge still supported? | Yes, as complete derived fallback. |
| Is cold bridge object authoritative? | No. |
| Does retained hot path build cold bridge objects? | No. |
| Who owns prepared publication interface? | Composition, because it defines semantic execution commit protocol. |
| What values appear in publication interface? | View plus prepare/commit/abort only. |
| Is publication contract generic for state/content later? | No, structural only. |
| Is commit fallible? | Not in ordinary operation; fallibility belongs to prepare. |
| Is abort required? | Yes. |
| Keep `needsPublication`? | Yes, for target sidebands such as History. |
| Keep legacy `install()` projection fallback? | No; delete by H3-D. |
| Must all api/controls be transport-free? | No, not H3 scope. |
| May runtime import both composition and transport? | Yes, intentionally. |
| May a control adapter implement a composition target using transport? | Yes. |
| Do H1 public exports change? | No. |
| Does structural ABI schema change? | Normally no. |
| Does Rust layout/paint change? | No, absent discovered blocker. |
| Does H3 implement PERF-13 state/content? | No. |

---

# 33. Source research appendix

## Repository baseline

Inspected against:

```text
alexykn/iyon-tui
main
1539afd0b53f58c699f146630ca1e3ad84961c5b
```

Primary source files inspected:

```text
API-H2-STRUCT-1-HANDOFF-v2.md
PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md
ARCHITECTURE.md
AGENTS.md
packages/iyon-tui/src/api/view/view.ts
packages/iyon-tui/src/api/presentation/style.ts
packages/iyon-tui/src/api/controls/framework-handle.ts
packages/iyon-tui/src/api/controls/view-slot.ts
packages/iyon-tui/src/api/controls/scroll-pane.ts
packages/iyon-tui/src/composition/compose.ts
packages/iyon-tui/src/composition/execution.ts
packages/iyon-tui/src/composition/child-owner.ts
packages/iyon-tui/src/composition/define-view.ts
packages/iyon-tui/src/composition/execution-context.ts
packages/iyon-tui/src/composition/persistent-seq.ts
packages/iyon-tui/src/composition/tracked-state.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/runtime/handle-registry.ts
packages/iyon-tui/src/transport/native/resources.ts
packages/iyon-tui/src/transport/structural/ir.ts
packages/iyon-tui/src/transport/structural/view-bridge.ts
packages/iyon-tui/src/transport/structural/component-view.ts
packages/iyon-tui/src/transport/structural/retained-dag.ts
packages/iyon-tui/src/transport/structural/native-view-abi.ts
tools/ownership/check.ts
```

## External architecture references

Jetpack Compose:

```text
https://developer.android.com/reference/kotlin/androidx/compose/runtime/Applier
https://android.googlesource.com/platform/frameworks/support/+/0ecddc8152eda57b806c09d55477d0c715d132fe/compose/runtime/runtime/src/commonMain/kotlin/androidx/compose/runtime/Applier.kt
```

React reconciler:

```text
https://github.com/facebook/react/blob/main/packages/react-reconciler/README.md
```

Flutter:

```text
https://docs.flutter.dev/resources/inside-flutter
https://api.flutter.dev/flutter/widgets/Element-class.html
https://api.flutter.dev/flutter/rendering/RenderObject-class.html
```

MLIR:

```text
https://mlir.llvm.org/docs/DialectConversion/
https://mlir.llvm.org/docs/TargetLLVMIR/
```

The cited systems support the separation principle. They do not override the repository-specific invariants established by PERF-12/H1/H2.

---

# 34. Final directive

Implement H3 as a separation of ownership, not as a new rendering architecture.

The desired end state is simple:

```text
composition says:
    "this is the semantic View that should now be structurally published"

transport says:
    "I know how that semantic View maps to retained native structure"
```

Composition does not know:

```text
NativeRef
bridge schema
ABI calls
lease mechanics
cold decoding
native component IDs
```

Transport does not know:

```text
execution scopes
State subscription graphs
keyed child ownership
semantic slot reuse algorithm
PersistentSeq implementation
```

The semantic View is the common language between them.

Keep PERF-12's identity and derivation advantages. Keep the current transaction semantics. Keep the structural ABI stable. Delete the accidental representation sharing that H2 made visible.

When H3 is complete, PERF-13 should be able to add:

```text
transport/state/
transport/content/
```

without composition learning that either transport exists.
