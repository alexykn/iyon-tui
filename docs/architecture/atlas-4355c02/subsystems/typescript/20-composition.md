# 20 — TypeScript composition

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `packages/iyon-tui/src/composition/`
- Assignment goal: execution scopes, identity, child ownership, tracked state, normalization, and publication.
- Investigation mode: read-only static source inspection.
- No project files were modified.
- No dependencies were installed.
- No build, test, benchmark, or runtime suite was executed by this scout.

The report contract, atlas README, repository `AGENTS.md`, and the full `PRE-V5-ARCHITECTURE-REPORT.md` were inspected for scope and evidence requirements. The current source was treated as authoritative. Historical PERF/R-series comments are called out as historical or descriptive where they affect interpretation.

### Scope boundaries

The assigned production directory contains eight TypeScript files:

```text
packages/iyon-tui/src/composition/
├── child-owner.ts
├── compose.ts
├── define-view.ts
├── execution-context.ts
├── execution.ts
├── persistent-seq.ts
├── publication.ts
└── tracked-state.ts
```

The composition implementation cannot be understood in isolation from a few adjacent seams, which were also inspected selectively:

- `packages/iyon-tui/src/api/view/view.ts`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
- `packages/iyon-tui/src/api/presentation/semantic-style.ts`
- `packages/iyon-tui/src/api/controls/view-slot.ts`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/tui-consumer-fixture/src/consumer.ts`
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts`
- `packages/tui-consumer-fixture/tests/consumer.test.ts`
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`

No product/plugin or Iyon-agent meaning was attributed to the generic composition implementation. The external fixture uses application-shaped test values, but those are consumers of the generic APIs rather than framework concepts.

### Facts, inferences, and unknowns

- **Fact:** `defineView`, `state`, and `View.key` are public TypeScript authoring surfaces.
- **Fact:** `RetainedExecutionScope`, `RetainedExecutionRuntime`, `OwnedBuilderRoot`, publication contracts, and composition helpers are internal implementation machinery; most are not package-root exports.
- **Fact:** the runtime owns one retained execution runtime per `Tui` and shares its queue and transaction protocol among the root scene, component projections, `ViewSlot` builder roots, and `ScrollPane` builder roots (`src/runtime/runtime.ts:120-196`).
- **Fact:** composition retains per-scope semantic slots and per-owner child scopes, but it does not own native transport records directly.
- **Fact:** tracked state records subscriptions to retained execution scopes and invalidates those scopes on `Object.is`-visible writes.
- **Fact:** structural publication is abstracted behind `preparePublication` / `commit` / `abort`.
- **Inference:** this is a small synchronous retained component runtime, not a general dependency graph, effect system, or asynchronous scheduler.
- **Unknown:** no executed validation was available from this investigation, so behavior is reconstructed from source and test assertions rather than observed at runtime.
- **Unknown:** the exact set of native/transport resources held behind each publication target is implemented outside composition and depends on adjacent retained-DAG and attachment modules.

### Physical LOC methodology

Counts below are approximate physical line counts based on source line ranges, including comments and blank lines. They are not logical-code LOC.

| File | Approx. physical production LOC | Test LOC in same file | Generated LOC |
|---|---:|---:|---:|
| `composition/child-owner.ts` | 92 | 0 | 0 |
| `composition/compose.ts` | 878 | 0 | 0 |
| `composition/define-view.ts` | 65 | 0 | 0 |
| `composition/execution-context.ts` | 132 | 0 | 0 |
| `composition/execution.ts` | 1,254 | 0 | 0 |
| `composition/persistent-seq.ts` | 309 | 0 | 0 |
| `composition/publication.ts` | 39 | 0 | 0 |
| `composition/tracked-state.ts` | 136 | 0 | 0 |
| **Total** | **2,905** | **0** | **0** |

Related behavioral tests inspected but not counted as composition production files:

- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`: approximately 83 physical lines.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts`: approximately 193 physical lines.
- `packages/tui-consumer-fixture/tests/consumer.test.ts`: approximately 118 physical lines.
- `packages/tui-consumer-fixture/src/consumer.ts`: approximately 170 physical lines of external-consumer fixture source.

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Primary responsibility | Secondary responsibilities | Public surface | Hot path |
|---|---|---|---|---|
| `composition/child-owner.ts` | Child ownership bookkeeping | WIP/committed child streams, keyed identity namespaces, release semantics | Internal | Yes during component evaluation/commit |
| `composition/compose.ts` | Retained semantic `View` factory/modifier helpers | Slot-based memoization, normalized semantic comparison, wide-structure bailouts, validation parity | Internal | Yes during every retained component body |
| `composition/define-view.ts` | Public retained component wrapper | Public component type shape, synchronous render contract documentation | `defineView`, `ViewComponent` type | Component invocation path |
| `composition/execution-context.ts` | Active execution and active child-owner context | Key-group context switching, raw construction guard, protocol mutation flags | Internal | Yes during all retained evaluation |
| `composition/execution.ts` | Retained execution runtime | Scope identity, scheduling, invalidation, reconciliation, transactions, publication, teardown, builder roots, counters | Internal runtime classes/functions | Yes during evaluation, flush, commit |
| `composition/persistent-seq.ts` | Immutable wide sequence storage | O(log₃₂ N) set/insert/remove/split/concat, aggregate flags, structural counters | Internal | Conditional wide axis/grid mutation path |
| `composition/publication.ts` | Publication protocol contracts | Root/child target separation, prepared transaction shape | Internal interfaces | Yes at retained publication boundaries |
| `composition/tracked-state.ts` | Public observable `State<T>` | Scope dependency tracking, subscriber management, `Object.is` write discipline, diagnostics | `state`, `State<T>` | State reads and writes |

### 1.2 `ChildOwnerState`: ownership substrate

`ChildOwnerState` is a reusable ownership record used by two structurally similar but semantically different owners (`child-owner.ts:4-13`):

1. `RetainedExecutionScope` owns actual child component execution, dependencies, and scheduling.
2. `KeyGroup` owns only a keyed identity namespace.

The owner contains separate WIP and committed structures:

```ts
committedChildren: ChildRecord[]
pendingChildren: ChildRecord[]
cursor: number
committedKeyed?: Map<ViewKey, KeyGroup>
pendingKeyed?: Map<ViewKey, KeyGroup>
wipActive: boolean
```

The unkeyed child stream is strictly positional. The keyed map is lazily allocated. `beginChildPass()` resets pending structures and marks participation; `dropPending()` abandons WIP without modifying committed state; `release()` clears both (`child-owner.ts:27-69`).

The distinction represented by `wipActive` is important:

- An owner that **evaluated and produced zero keyed children** has `wipActive === true` and therefore removes all previously committed keyed groups during promotion.
- An owner that was **not evaluated** has `wipActive === false`, so its keyed groups remain untouched.

That distinction prevents an unrelated child-only invalidation from accidentally deleting keyed siblings merely because the parent itself did not run.

### 1.3 `RetainedExecutionScope`

`RetainedExecutionScope<P>` is the continuity boundary for one logical `defineView` instance (`execution.ts:103-219`). It is explicitly distinct from:

- semantic `View`/`SemanticNodeId`;
- native transport IDs and native resource leases;
- component handle IDs;
- physical native projection resources.

The scope stores:

```text
id
runtime
parent
depth
ordinal
key
type

currentProps / pendingProps / pendingPropsActive
currentOutput / pendingOutput

state: clean | evaluating | aborted
mounted
dirty
disposed

owner: ChildOwnerState
table: ScopeSemanticTable
dependencies / pendingDependencies

publicationTarget
projection
projectedOutput
stagedPublication
```

The constructor assigns a monotonically increasing module-local scope ID (`execution.ts:319`, `151-168`). Scope identity is preserved across evaluations when reconciliation finds the same component type in the same parent-local position, or the same component type under the same keyed group.

The scope owns:

- the component’s retained semantic slots;
- child scopes and keyed namespaces;
- committed state dependencies;
- pending output and props;
- publication target/projection metadata;
- disposal of the complete descendant tree.

`dispose()` first disposes its projection, unsubscribes all committed dependencies, clears outputs and props, releases semantic slots, recursively disposes all owned children and nested keyed groups, and increments the unmount counter (`execution.ts:192-217`).

### 1.4 `ScopeSemanticTable`

The private `ScopeSemanticTable` is a dense, call-order-addressed table of semantic slots (`execution.ts:52-101`):

```ts
interface SemanticSlot {
  current: View | undefined;
  pending: View | undefined;
}
```

A pass starts with `beginLength = slots.length` and `cursor = 0`. Each composition helper consumes the next slot. `commit()` promotes pending values and truncates the table to the number of slots consumed in the successful pass. `rollback()` clears pending values and truncates newly allocated slots back to the previous length.

This is not a global memoization cache. It is per-retained-scope positional retention. The current implementation has no SiteId table, no source transform, and no global composition registry.

### 1.5 `compose.ts`

`compose.ts` supplies the internal retained versions of public `View` factories and modifiers. Its contract is:

1. If no active scope exists, call ordinary `View` construction.
2. If an active scope exists, consume one dense semantic slot.
3. Compare the new raw arguments against the previous normalized semantic node.
4. Reuse the exact previous `View` object where the comparison proves equality.
5. Otherwise construct one new immutable semantic `View`.
6. Stage the result in the slot’s pending field.

The module intentionally uses integer modifier tags rather than strings or reflective dispatch (`compose.ts:85-101`). Its comparators operate on normalized semantic records rather than on transport objects.

The factory families include:

- text, styled text, spacer;
- component and content-host occurrences;
- state attachments;
- hanging layouts;
- vertical/horizontal axes;
- content max, container, clamp;
- grid;
- diff;
- width/height/padding/color/style/border modifiers;
- wrapping and text alignment patches.

The helper implementation uses `withoutRetainedComposition()` while invoking raw public `View` methods so the public validation and derivation behavior remains the same without recursively consuming additional retained slots (`compose.ts:298-350`, `execution-context.ts:47-55`).

### 1.6 `define-view.ts`

`defineView` returns a callable function object with a stable `.render` property (`define-view.ts:37-65`).

The public type is:

```ts
interface ViewComponentType<P = unknown> {
  readonly render: (props: P) => View;
}

interface ViewComponent<P = unknown> {
  readonly render: (props: P) => View;
  (props: P): View;
}
```

Calling the returned component does not directly run the render body. It calls `invokeComponent(component, props)`, which requires an active evaluating scope and reconciles the component as a retained child (`define-view.ts:42-46`, `59-64`).

The component token is the function object itself. Component identity is therefore reference identity of the `defineView` result, not a string name or render-function source identity.

### 1.7 `execution-context.ts`

This module deliberately remains lower-level than `execution.ts` and does not import `View`, avoiding a cycle with `View.key` (`execution-context.ts:5-7`).

It stores two independent active contexts:

```text
ACTIVE_EXECUTION_SCOPE
  Which scope owns semantic slots and dependency reads.

ACTIVE_CHILD_OWNER
  Which ChildOwnerState receives component invocations.
```

Normally both refer to the active scope and its `owner`. During `View.key`, only the active child owner changes; the active execution scope remains unchanged (`execution-context.ts:9-15`, `74-90`, `116-131`).

It also owns:

- `protocolState.mutating`;
- `protocolState.internalPublication`;
- `semanticConstruction.raw`;
- the active frame stack;
- keyed-group resolution and duplicate-key detection.

### 1.8 `tracked-state.ts`

`state(initial)` creates a public wrapper over a private `StateSource<T>` (`tracked-state.ts:41-125`). A source contains:

```ts
currentValue: T
subscribers: Set<RetainedExecutionScope>
```

Reads are legal anywhere. A read while a component body is evaluating adds the active execution scope to that scope’s pending dependency set. Reads outside evaluation are ordinary untracked reads (`tracked-state.ts:49-55`).

Writes:

- are rejected during any active component evaluation;
- use `Object.is` to suppress no-op updates;
- mutate the authoritative source value before publishing;
- invalidate each subscribed live scope exactly through the retained runtime.

There are no computed state values, effects, proxies, derived graphs, deep observation, or dependency propagation beyond direct scope invalidation.

### 1.9 `publication.ts`

The publication module only defines contracts:

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

Composition knows only a semantic `View` and this protocol. Native refs, transport records, resource leases, and host objects stay behind the publication target (`publication.ts:14-23`).

### 1.10 `persistent-seq.ts`

`PersistentSeq<T>` is an immutable branching sequence with branch factor 32 (`persistent-seq.ts:1-2`, `215-309`). It stores frozen leaves/branches, cumulative sizes, height, and an aggregate bitmask. It supports:

- random access;
- persistent `set`;
- append/insert/remove;
- splice;
- split;
- concat;
- iteration.

Mutation clones only the path to the changed leaf, giving the intended O(log₃₂ N) structural edit behavior. Counters distinguish cloned nodes, cloned branches, and leaf items iterated (`persistent-seq.ts:3-16`).

The sequence is not itself a component identity or execution-state mechanism. It is a side structure for wide axis/grid semantic mutations, referenced through weak semantic sidecars in `api/view/semantic-node.ts`.

## 2. Types, APIs and contracts

### 2.1 Intentional public TypeScript composition surfaces

Package-root exports in `packages/iyon-tui/src/index.ts` are:

- `defineView` (`index.ts:99`);
- `ViewComponent` type (`index.ts:61`);
- `state` (`index.ts:100`);
- `State<T>` type (`index.ts:101`);
- `View`, whose public static `key` method is implemented in `api/view/view.ts:249-265`.

The following are **not** package-root public composition APIs:

- `RetainedExecutionScope`;
- `RetainedExecutionRuntime`;
- `OwnedBuilderRoot`;
- `invokeComponent`;
- `executionCounters`;
- `executionContext`;
- `ChildOwnerState`;
- `KeyGroup`;
- `StructuralPublicationTarget`;
- `PersistentSeq`;
- all `compose*` helpers.

Tests can import some internals directly, but that is test/diagnostic access rather than the documented authoring surface.

### 2.2 `defineView` contract

`defineView(render)` requires a function and rejects non-functions with `TypeError` (`define-view.ts:55-58`).

The render body is expected to be:

- synchronous;
- pure with respect to tracked-state writes and builder-boundary mutations;
- a function from props to a semantic `View`.

Async or promise-like body returns are rejected at evaluation time with `ExecutionError` code `TUI_EXECUTION_ASYNC_BODY` (`execution.ts:235-252`, `627-650`).

The component’s render function receives props selected as:

```text
pendingProps when pendingPropsActive
otherwise currentProps
```

Props are committed only after the scope output commits.

### 2.3 Props equality

`propsShallowEqual` first uses `Object.is`. For non-null objects it compares `Object.keys()` length and each own enumerable value with `Object.is` (`execution.ts:303-317`).

Consequences:

- primitive or object reference identity can skip execution;
- freshly allocated nested objects do not compare equal;
- deep structural equality is not attempted;
- immutable props are the intended contract;
- only enumerable string-keyed own properties participate.

When an existing child has equal props, `invokeInto` increments `execution_scope_prop_skips` and skips the child body completely (`execution.ts:1027-1030`).

### 2.4 Component identity

Component reconciliation compares the component type object by reference (`execution.ts:949-957`).

At an unkeyed position, a child is reused only if:

```text
existing record exists
existing scope is not disposed
existing component type === invoked component type
existing scope key === undefined
```

A component type change at the same position creates a fresh scope and causes the old scope to be removed during owner promotion.

The public `defineView` return value is therefore the stable component identity token. Recreating a `defineView` wrapper on every parent evaluation defeats retention.

### 2.5 Key identity

`ViewKey` is `string | number` and is explicitly local to a child-owner namespace (`child-owner.ts:16-17`).

`View.key(key, build)` does not create a schedulable scope. It routes component invocations in `build` to a keyed `KeyGroup` owner while leaving the active execution scope unchanged (`view.ts:251-265`, `execution-context.ts:93-131`).

The current architecture intentionally separates:

```text
keyed identity namespace = KeyGroup
component execution       = RetainedExecutionScope
state invalidation        = State<T>
semantic View identity    = SemanticNodeId / View object
native identity           = transport/native handle or ref
```

A scope created under a keyed group still has `scope.key === undefined` in the current call path because `invokeChild` resolves the group first and calls `invokeInto(group.owner, ..., key = undefined)` (`execution.ts:994-1002`). The `ChildRecord.key` field exists in the type but current invocation routes use the group as the key identity owner. `keyGroupOf(scope)` performs a diagnostic search through pending and committed keyed namespaces (`execution.ts:1138-1163`).

### 2.6 State contract

The public `State<T>` interface is:

```ts
interface State<T> {
  readonly value: T;
  set(value: T): void;
  update(update: (previous: T) => T): void;
}
```

(`tracked-state.ts:104-108`).

The contract is deliberately minimal:

- reads subscribe the active scope;
- writes invalidate subscribed scopes;
- equal values do nothing;
- writes inside a component body throw before mutation;
- subscriptions are promoted only after a successful scope commit;
- aborted evaluations leave the previous dependency set intact.

`trackedStateSubscriberCount` and the `WeakMap` from public wrapper to subscriber set are diagnostics/test aids, not semantic API (`tracked-state.ts:127-136`).

### 2.7 Semantic `View` contract used by composition

Adjacent `api/view/view.ts` and `api/view/semantic-node.ts` establish that:

- a `View` is an immutable facade over one frozen semantic node;
- every newly created semantic node receives a fresh positive safe integer semantic ID;
- semantic IDs are not native ABI IDs;
- child relationships are semantic-node references;
- attachment fields contain local handle IDs and are resolved later;
- semantic records are frozen recursively;
- strong attachment references are retained through weak maps associated with semantic nodes.

The node vocabulary includes text, diff, spacer, row, column, grid, hanging, container, clamp, content-max, component, content-host, and decorated nodes (`semantic-node.ts:161-276`).

Composition comparators operate against this normalized vocabulary rather than against arbitrary public objects.

### 2.8 Publication contract

Preparation is the only ordinary fallible phase. Commit is expected to promote already-prepared state. Abort releases staged resources without changing the committed frame (`publication.ts:4-12`).

The optional `needsPublication(output)` hook handles target-owned sideband changes even when the semantic output object is unchanged. The root target uses this for staged history changes (`runtime.ts:463-467`).

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
public package root
  ├── defineView.ts
  ├── tracked-state.ts
  └── View from api/view/view.ts
          │
          ├── execution-context.ts
          ├── compose.ts
          └── persistent-seq.ts

define-view.ts
  └── execution.ts

tracked-state.ts
  └── execution.ts (active scope, invalidation, scope dependency API)

execution.ts
  ├── execution-context.ts
  ├── child-owner.ts
  ├── publication.ts
  ├── semantic-node.ts
  └── ViewComponentType / TrackedStateSource (type edges)

compose.ts
  ├── execution.ts / execution-context.ts
  ├── api/view/view.ts
  ├── api/view/semantic-node.ts
  ├── api/presentation/semantic-style.ts
  ├── content/style/handle types
  └── persistent wide-structure sidecars through View APIs

runtime/runtime.ts
  ├── owns RetainedExecutionRuntime
  ├── supplies root publication target
  └── supplies child StructuralScopeProjection factory

view-slot.ts / scroll-pane.ts
  ├── own OwnedBuilderRoot instances
  ├── supply publication targets
  └── expose component View occurrences via composeComponent

api/view/view.ts
  ├── calls compose helpers during retained construction
  └── calls withKeyedChildOwner for View.key
```

### 3.2 Ownership graph

```text
Tui
└── one RetainedExecutionRuntime
    ├── dirty queue
    ├── retained roots
    │
    ├── root OwnedBuilderRoot
    │   └── root RetainedExecutionScope
    │       ├── ScopeSemanticTable
    │       │   └── semantic slots: current/pending View
    │       ├── ChildOwnerState
    │       │   ├── unkeyed ChildRecord[] → child RetainedExecutionScope
    │       │   └── keyed Map<ViewKey, KeyGroup>
    │       │       └── KeyGroup.owner
    │       │           ├── unkeyed ChildRecord[] → child scopes
    │       │           └── nested keyed groups
    │       ├── State<T> dependencies
    │       └── publication target / child projection
    │
    ├── ViewSlot-owned OwnedBuilderRoot
    │   └── slot publication target → RetainedRootBoundary/native view slot
    │
    └── ScrollPane-owned OwnedBuilderRoot
        └── pane publication target → RetainedRootBoundary/native scroll pane
```

### 3.3 Component projection ownership

When `Tui` creates a child scope, its projection factory creates a `ViewSlot` seeded with `View.spacer(0)` and returns:

```text
StructuralScopeProjection
  ├── view: component semantic View with slot HandleId
  ├── target.preparePublication(output)
  │   └── slot.prepareSetView(output)
  └── dispose()
      └── slot.dispose()
```

(`runtime.ts:169-195`).

The component scope owns this projection object. The projection owns the native slot/control resources. The parent semantic tree embeds the projection’s component `View`, not the child’s raw rendered output. The child raw output is published into the projection target.

This distinction lets a child scope re-execute independently while its parent retains a stable component occurrence in the semantic tree.

### 3.4 State ownership

```text
StateSource<T>
  ├── currentValue
  └── subscribers: Set<RetainedExecutionScope>

RetainedExecutionScope
  ├── dependencies: committed StateSource set
  └── pendingDependencies: StateSource set from current evaluation
```

During evaluation, reads add to `pendingDependencies`. On commit:

1. dependencies absent from pending are unsubscribed;
2. pending dependencies not already present are subscribed;
3. committed dependency set is replaced by pending.

On abort, `pendingDependencies` is discarded and the old subscriptions remain.

### 3.5 Creation and destruction

#### Root scope

- `Tui.render(builder)` creates an `OwnedBuilderRoot` on first canonical render (`runtime.ts:451-485`).
- `OwnedBuilderRoot.start()` constructs a root scope and calls `mountExistingRoot()` (`execution.ts:1213-1221`).
- Failed initial evaluation or preparation disposes the entire uncommitted tree and leaves the `Tui` retryable.
- `Tui.close()` disposes the root builder before disposing the retained runtime (`runtime.ts:730-734`, `806-810`).

#### Child scope

- Created in `RetainedExecutionRuntime.reconcileChild()` when no compatible committed child exists (`execution.ts:941-974`).
- Projection factory failures dispose the partially created scope immediately because it has not yet entered parent WIP ownership (`execution.ts:959-970`).
- Fresh child scopes become mounted only after their output and descendants commit.
- Removed children are collected during owner promotion and disposed only after the complete batch commits normally.

#### Keyed group

- Created lazily in `resolveKeyedGroup()` when no committed group exists for the local key (`execution-context.ts:105-113`).
- The group itself has no scheduler, dependency set, output, or native resource.
- Its descendant scopes are disposed when the key disappears from a successful evaluated pass or when the containing scope is disposed.

### 3.6 Identity/lifetime diagram

```text
defineView return object
        │ reference identity
        ▼
component invocation
        │ parent-local ordinal or keyed-group namespace
        ▼
RetainedExecutionScope
        │ continuity across evaluations
        ├── semantic slot table → immutable View objects / semantic nodes
        ├── State dependency subscriptions
        └── projection → component semantic occurrence + native target
                                  │
                                  └── native transport lease/resource

View.key key
        │ local identity only
        ▼
KeyGroup
        │ owns child namespace
        └── RetainedExecutionScope descendants
```

## 4. Execution paths and state transitions

### 4.1 Canonical root render

The canonical builder route begins at `Tui.render(sceneOrBuilder)`:

```text
Tui.render(builder)
  → renderCanonical(builder)
      → create producer()
          → Scene.from(builder())
          → stageHistoryBinding(scene.history)
          → return scene.body
      → first call:
          OwnedBuilderRoot.start(...)
            → mountExistingRoot(rootScope)
                → runWork(rootScope)
                → stagePublicationsRecursive(rootScope)
                → commitBatch([rootScope])
      → subsequent calls:
          rootBuilder.replaceProducer(producer)
            → runtime.update(rootScope)
                → invalidate(rootScope)
                → flush()
      → Tui.flush()
          → retainedRuntime.flush()
          → hostRegistration.flush()
```

(`runtime.ts:399-420`, `451-497`; `execution.ts:850-886`, `1213-1248`).

The producer closure’s object identity is not used as root identity. `OwnedBuilderRoot` retains the root scope; replacing the producer only changes the producer callback.

### 4.2 Component invocation

A component body invokes a child by calling the callable `defineView` result:

```text
Child(props)
  → component wrapper
      → invokeComponent(component, props)
          → activeExecutionScope()
          → runtime.invokeChild(component, props, key)
              → reconcileChild(...)
              → if existing + shallow-equal props:
                    prop skip; return existing scope
              → otherwise:
                    set pendingProps
                    clear queued dirty bit
                    evaluateIntoPendings(child)
                    return child scope/output
```

(`define-view.ts:59-61`; `execution.ts:976-1041`).

The returned `View` is:

- the projection `view` when the child has a projection;
- otherwise the child’s pending or current rendered output.

For a normal `Tui` runtime, component scopes receive a projection. A generic `RetainedExecutionRuntime` can be created without a projection factory, in which case the child output itself is embeddable.

### 4.3 Unkeyed reconciliation

Unkeyed reconciliation consumes the next owner cursor ordinal (`execution.ts:947-973`):

1. Increment `owner.cursor`.
2. Inspect `owner.committedChildren[ordinal]`.
3. Reuse only if the prior scope is live, the type object is identical, and the old scope is unkeyed.
4. Otherwise allocate a fresh scope with the current parent and ordinal.
5. Insert the record into `pendingChildren[ordinal]`.

Because the table is dense and positional, conditional branches or changed child ordering can shift later slots and child ordinals. There are no source-site IDs to repair positional changes.

### 4.4 Keyed reconciliation

`View.key(key, build)` calls `withKeyedChildOwner(activeChildOwnerOrThrow(), key, build)` (`view.ts:263-264`).

The keyed path is:

```text
View.key(key, build)
  → active child owner
  → resolveKeyedGroup(owner, key)
      → reject duplicate key in current WIP pass
      → reuse committed group or create KeyGroup
      → group.owner.beginChildPass()
  → temporarily switch ACTIVE_CHILD_OWNER to group.owner
  → execute build()
      → component invocations reconcile under group.owner
  → restore previous ACTIVE_CHILD_OWNER
```

The active execution scope is not changed during this process. Therefore:

- semantic factory slots in raw `View` construction inside the `build` thunk remain attached to the enclosing execution scope;
- state reads inside the thunk remain dependencies of the enclosing scope;
- only component child ownership is redirected into the keyed namespace.

A keyed group is not independently dirty or schedulable.

### 4.5 Evaluation state transition

`evaluateIntoPendings(scope)` performs the retained component pass (`execution.ts:627-650`):

```text
clean
  → evaluating
      table.begin()
      owner.beginChildPass()
      pendingDependencies = new Set()
      push active frame
      render(props)
      reject Promise-like output
      semanticNodeOf(output) validates View
      pendingOutput = output
      pop active frame
  → commit phase later → clean + mounted
```

The `finally` block always pops the active frame. A corrupted frame stack is converted into `ExecutionError("TUI_EXECUTION_CONTEXT", ...)` (`execution.ts:223-233`).

The source checks the returned value with `semanticNodeOf(output)` before placing it into pending output, so a JavaScript caller that violates the TypeScript return type cannot promote an invalid object as a mounted output (`execution.ts:642-647`).

### 4.6 Semantic composition state transition

Inside a component body, public `View` methods route to composition helpers because `isRetainedConstruction()` checks for an active scope and `semanticConstruction.raw === false` (`api/view/view.ts:152-154`).

Example:

```text
View.text(value)
  → composeText(value)
      → active scope.nextSemanticSlot()
      → inspect previous slot.current
      → compare normalized text node
      → exact previous View reuse
        OR withoutRetainedComposition(() => View.text(value))
           → fresh semantic View
      → slot.pending = result
```

When the body exits successfully, `ScopeSemanticTable.commit()` promotes pending values. If body evaluation or publication preparation fails, slot pending values are cleared and previous current values remain authoritative.

### 4.7 State invalidation

A state write follows:

```text
state.set(next)
  → StateSource.set(next)
      → reject if active execution scope
      → Object.is(currentValue, next)
           equal → return
           changed:
             currentValue = next
             publish()
                → for each subscribed scope:
                    scope.runtime.invalidateFromState(scope)
                        → counter increment
                        → runtime.invalidate(scope)
```

(`tracked-state.ts:65-92`; `execution.ts:550-554`).

`invalidate()` is level-triggered:

- if already dirty, it increments the duplicate-invalidation counter and does not enqueue a second copy;
- otherwise it marks dirty, pushes the scope to the queue, increments the dirty-enqueue counter, and schedules a flush if auto-flush is enabled (`execution.ts:413-437`).

A state write therefore targets the scopes that read the state, not necessarily the root or parent component.

### 4.8 Flush state machine

`RetainedExecutionRuntime.flush()` uses three phases (`execution.ts:440-548`).

#### Phase 1: evaluate

- Take the current queue as one batch.
- Snapshot dirty scopes as retry obligations.
- Sort parent-before-child by depth, ordinal, and ID.
- Clear each scope’s dirty bit before evaluation.
- Skip scopes dropped from a parent’s current WIP tree.
- Evaluate every remaining scope.
- If evaluation throws, abort WIP and restore all retry obligations without automatically retrying.

#### Phase 2: prepare publications

- Recursively stage pending child publications and then the scope’s own publication.
- A target may refuse preparation by returning `undefined`.
- Every staged publication is unwound with `abort()` on preparation failure.
- JS scope state and invalidation obligations are restored.
- No prior committed output is replaced.

#### Phase 3: commit and promote

- Commit descendants before parents.
- Commit each scope at most once per batch using a `Set`.
- Execute prepared publication commits.
- Promote output, props, dependencies, semantic slots, and child ownership.
- Defer disposal of removed scopes until normal promotion completes.

A commit-phase throw is treated as pathological rather than as an ordinary rollback. It is recorded in `pathologicalCommitFailures` and deliberately does not enter the normal abort path (`execution.ts:522-542`).

### 4.9 Child commit and removal

`commitScope` commits descendants before promoting the current scope (`execution.ts:723-781`). This ensures that a parent’s projected child publication is authoritative before the parent output is published.

`commitOwnerChildren`:

1. commits pending unkeyed child scopes with pending outputs;
2. recursively commits keyed groups;
3. promotes owner child structures.

`promoteOwnedChildren`:

- computes the set of pending child scopes;
- collects committed scopes absent from that set into a deferred-removal sink;
- replaces committed unkeyed children with pending children;
- marks promoted child scopes mounted;
- resets pending children and cursor;
- if the owner participated in the pass, merges pending keyed groups and collects absent keyed group subtrees for deferred disposal (`execution.ts:1060-1097`).

### 4.10 Abort and retry state

An evaluation or preparation abort preserves the previous committed frame:

```text
pending output        discarded
pending props         discarded
pending dependencies  discarded
semantic slot pending discarded
pending child WIP     discarded
pending keyed WIP     discarded
committed children    retained
committed keyed map   retained
committed deps        retained
current output        retained
dirty obligations     restored to queue
```

(`execution.ts:783-842`, `565-571`).

The restored retry obligation is intentionally not automatically re-scheduled after the failed flush. A later explicit flush, later state write, or other scheduling trigger must re-drive it. This avoids infinite microtask loops for permanently throwing components.

### 4.11 Owned builder roots and producer replacement

`OwnedBuilderRoot` wraps a producer callback in a root `RetainedExecutionScope` and supplies a publication target (`execution.ts:1185-1221`).

`replaceProducer(producer)`:

1. ignores an identical producer function;
2. remembers the previous producer;
3. installs the new producer optimistically;
4. synchronously calls `runtime.update(scope)`;
5. restores the previous producer if evaluation/preparation fails;
6. cancels only the retry obligation introduced solely by the failed producer replacement.

A pre-existing dirty obligation is retained because the restored producer may still need to render newer state values (`execution.ts:1229-1246`).

### 4.12 Teardown

`OwnedBuilderRoot.dispose()` delegates to `runtime.detachRoot()`, which disposes the scope tree, removes the root, and filters disposed scopes from the dirty queue (`execution.ts:888-897`, `1250-1252`).

`RetainedExecutionRuntime.dispose()` invalidates pending scheduled microtasks, disposes all roots, and clears the queue (`execution.ts:594-604`).

The `Tui` runtime disposes:

1. host registration;
2. content resources;
3. attachment bindings;
4. owned handles;
5. root builder;
6. retained execution runtime;
7. root boundary.

This ordering is outside composition but is consequential because publication targets must remain valid while root execution state is released (`runtime.ts:786-814`).

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | No active scope | Active retained scope | Equality behavior | Failure/recovery behavior |
|---|---|---|---|---|
| `View.text(value)` | Ordinary immutable `View` construction | `composeText` consumes one slot | Exact normalized text payload, default wrap/alignment | Public validation still applies |
| `View.styledText(spans)` | Ordinary construction | `composeStyledText` | Span text and normalized styles compared | Fresh immutable node on mismatch |
| `View.spacer(rows)` | Ordinary construction | `composeSpacer` | Row count | Validation through public API |
| `View.vertical/horizontal` | Build children, normalize eager semantic axis | Build children first, then compare parent slot | Child semantic identity, track metadata, gap | Wide sidecar intentionally rejects cheap reuse |
| `View.grid(spec)` | `rawGrid` normalization | `gridBuilderFromSpecification`, compare, then materialize | Tracks, rows, cells, spans, alignments, gaps | Wide grid sidecar intentionally rebuilds |
| `View.diff(hunks)` | Ordinary construction | Always fresh and consumes one slot | No cheap immediate comparator | Fresh immutable output each evaluation |
| `.padding/.foreground/.border/.style/...` | Ordinary public decoration path | Integer-tagged `applyDecoration` | Flattened normalized decoration delta comparison | Unknown tag is explicit `RangeError` |
| `.wrap/.textAlign` | Ordinary layout patch | Slot-based layout patch comparison | Text spans/decoration and layout scalars | Invalid modes explicitly rejected |
| `defineView` invocation | Throws outside an active scope | Reconcile child under active child owner | Type identity + props shallow equality | Invalid component, no active scope, async body, invalid output reject |
| `View.key(key, build)` | Throws outside active execution | Resolve local keyed group and swap only child owner | Duplicate key rejected in current pass | WIP group dropped on abort; absent groups removed on commit |
| `state.value` read | Untracked ordinary read | Adds current scope to pending dependencies | Set deduplicates repeated reads | Dependency set promoted only on commit |
| `state.set/update` | Object.is write and subscriber invalidation | Throws while any component body evaluates | Equal write is no-op | Source value remains changed even if later flush aborts |
| `Tui.render(builder)` | N/A; runtime route | Root `OwnedBuilderRoot` producer | Root scope retained independent of closure identity | Producer restored on failed replacement |
| `Tui.render(scene)` | Direct scene publication | Drains retained work, then direct install | Direct identity cutoff by body/history identity | Direct install failure restores staged sideband |
| `ViewSlot.setView(builder)` | Unsupported for raw internal slot | Slot-owned `OwnedBuilderRoot` | Root scope and semantic slots retained | Failed replacement restores old builder |
| `ViewSlot.setView(view)` | Direct boundary install | Direct boundary install | Root builder disposed after success | Old builder remains if install fails |
| `ScrollPane.setContent(builder)` | Unsupported for raw internal pane | Pane-owned `OwnedBuilderRoot` | Same retained runtime protocol | Ownership transition is transactional |
| Child projection publication | No target if runtime configured without factory | `StructuralScopeProjection.target` | Optional `needsPublication` can force publication | Prepare refusal aborts complete batch |

### 5.2 Explicit failures

The composition layer rejects rather than masks these conditions:

- `defineView` receives a non-function (`define-view.ts:55-58`);
- component invocation outside active evaluation (`execution.ts:987-990`, `1176-1180`);
- invalid component object without a `render` function (`execution.ts:981-985`);
- duplicate keyed use in one WIP pass (`execution-context.ts:105-113`);
- `View.key` outside an evaluating scope (`execution-context.ts:65-71`);
- async/promise-like component body (`execution.ts:635-641`);
- component body returns a non-semantic object (`execution.ts:642-647`);
- tracked-state write during body evaluation (`tracked-state.ts:72-90`);
- reentrant builder-boundary mutation during protocol execution (`execution.ts:606-614`);
- publication target refuses preparation (`execution.ts:673-679`);
- corrupted active execution stack (`execution.ts:227-233`);
- operation against disposed retained scope (`execution.ts:653-656`);
- invalid public semantic values through the corresponding `View` validation paths.

### 5.3 Failure preservation

Evaluation and preparation failures preserve the last committed frame and restore retry obligations. This is stronger than merely “not installing the new output”: pending child scopes, keyed groups, semantic slots, props, dependencies, publications, and boundary-owned sideband state all have rollback handling.

The source differentiates:

- **evaluation failure:** body throws or output is invalid;
- **publication preparation failure:** target cannot prepare;
- **publication cleanup failure:** abort of staged publication throws, surfaced as an `AggregateError`;
- **commit-phase failure:** considered pathological/unspecified and deliberately not rolled back as an ordinary transaction.

### 5.4 Silent fallback search result

Within the inspected composition/runtime paths, no legacy full-object composition fallback is used after retained publication refusal. The retained path either:

- reuses the exact semantic `View`;
- constructs a new immutable semantic `View`;
- refuses explicitly;
- or hands off to the retained structural boundary.

`compose.ts` does have legitimate special routes:

- no active scope falls through to ordinary construction;
- wide axis/grid sequences intentionally bail out of cheap immediate comparison;
- `composeDiff` always rebuilds because it has no cheap equality proof.

These are explicit compatibility/performance decisions, not silent recovery to a second complete architecture.

## 6. Caches, invalidation, scheduling and performance

### 6.1 Semantic slot retention

Bounds and retention:

- one dense slot per retained semantic operation per execution scope;
- slot count equals the number of compose operations reached in the last committed pass;
- stale trailing slots are truncated on commit;
- pending values are discarded on rollback;
- no global cache and no cross-scope sharing of slot state.

The slot table is invalidated implicitly by a new evaluation of the owning scope. An exact equality hit still consumes the slot and stages the prior object.

### 6.2 Child scope retention

Unkeyed children are retained by `(owner, ordinal, component type)`. Keyed children are retained by `(owner, ViewKey, component type within the group’s local child stream)`.

The parent owner’s WIP pass determines which child scopes survive. State invalidation does not inherently re-run parents, so a child scope may be independently dirty while its parent remains clean.

### 6.3 State dependency invalidation

State source subscribers are a Set, so repeated reads in one evaluation do not duplicate subscriptions. A source write loops over subscribers and calls runtime invalidation. Duplicate invalidation is counted and does not enqueue a second copy.

The source itself retains the mutable current value. Scope dependencies are derived runtime metadata and are committed transactionally.

### 6.4 Scheduler behavior

`RetainedExecutionRuntime` has:

```text
queue: RetainedExecutionScope[]
flushing: boolean
autoFlush: boolean
scheduledGeneration: number
flushScheduled: boolean
mutating: boolean
```

When `autoFlush` is enabled, the first dirty enqueue schedules one microtask. The generation counter invalidates stale scheduled callbacks when explicit `flush()` or runtime disposal consumes/cancels a pending schedule (`execution.ts:335-345`, `428-448`, `594-604`).

The queue is level-triggered. A failed pass restores dirty obligations but does not arm an automatic retry. A later state write to an already-dirty scope re-arms scheduling.

### 6.5 Wide sequence performance

`PersistentSeq` uses branch factor 32 and path copying. Mutation counters are:

```text
nodes_cloned
branches_cloned
items_iterated
```

(`persistent-seq.ts:12-22`).

The semantic `View` layer uses wide sidecars above thresholds:

- axis sequence threshold: 1,024 children;
- grid sequence threshold: 1,024 total cells.

(`api/view/view.ts:149-150`, `825-832`, `874-888`).

Composition deliberately refuses to flatten wide sidecars merely to prove an identity hit (`compose.ts:565-567`, `690-693`). This avoids turning a retained no-op comparison into an O(N) lazy flatten. The tradeoff is that a wide axis/grid construction is rebuilt through the persistent sidecar path rather than reused through the ordinary eager comparator.

### 6.6 Counters and observed work categories

Composition/execution counters include:

```text
execution_scope_mounts
execution_scope_unmounts
execution_scope_body_calls
execution_scope_prop_skips
execution_scope_state_invalidations
execution_scope_dirty_enqueues
execution_scope_duplicate_invalidations
execution_scope_noop_outputs
execution_scope_changed_outputs
execution_flush_passes
execution_commit_batches
execution_commit_aborts
composition_exact_view_reuses
composition_new_views
```

(`execution.ts:254-300`).

These counters are diagnostics, not scheduling inputs. No counter values were observed during this static investigation.

### 6.7 Work granularity

| Work | Frequency |
|---|---|
| State read dependency link | Once per read during a component evaluation |
| State write source publication | Once per changed write |
| Dirty enqueue | Once per clean scope invalidation |
| Component body | Initial mount, changed props, or direct state invalidation |
| Parent body | Only when parent itself is dirty or explicitly re-driven |
| Child body | Can run inline while parent evaluates; can also be independently queued |
| Semantic slot comparison | Once per reached composition operation in each executed body |
| Publication preparation | For changed projected output or target-requested sideband publication |
| Commit | Once per retained batch |
| Native visible frame flush | Runtime host barrier after desired publication |
| Wide sequence structural mutation | O(log₃₂ N) path cloning, with counters |
| Diff composition | Every evaluation reaching the diff operation |

## 7. Tests, benchmarks and observability

### 7.1 Direct composition test

`packages/iyon-tui/tests/tui_h3_b_composition.test.ts` provides direct evidence for:

1. Semantic construction:
   - `View.vertical` produces a semantic column node.
   - Semantic node and children are frozen.
   - No old transport-shaped `schema` or `handle` fields are present.
   - `semanticNodeOf(view)` returns the same node association.
2. Retained exact reuse:
   - Initial `defineView` body runs once.
   - Equal `state.set("ready")` produces no body call and no new view.
   - Changed text produces a new `View` and semantic node.
3. Component semantic identity:
   - `ViewSlot.view()` produces a component semantic node.
   - Its identity is the slot’s local `HandleId`.
   - Disposed slot access throws.

(`tui_h3_b_composition.test.ts:15-83`).

### 7.2 External consumer scoped-invalidation tests

`packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts` is important because it uses public APIs only:

```ts
import { defineView, Scene, state, View } from "@iyon/tui";
```

The tests assert:

- a state write re-executes only the scope that read that state (`:40-70`);
- 1,000 sibling scopes can be created and only the changed sibling re-executes (`:72-114`);
- keyed reorder preserves per-key component execution counts (`:116-153`);
- a keyed item content change re-executes only the changed key;
- builder → direct ownership transitions do not allow a stale builder to ghost-write after direct takeover (`:155-191`).

The fixture source constructs:

```text
App
├── Header
├── View.key(entry.id, () => ItemCard(entry)) × N
└── Footer
```

(`packages/tui-consumer-fixture/src/consumer.ts:137-158`).

This demonstrates the intended separation:

- `items.value` read subscribes `App`;
- each `ItemCard` has its own props and execution scope;
- keyed identity follows `entry.id`;
- `status.value` read subscribes only `Header`.

### 7.3 External consumer skeleton tests

`packages/tui-consumer-fixture/tests/consumer.test.ts` exercises the direct public scene and control paths:

- ordinary rendering;
- direct semantic no-op re-rendering;
- text change;
- conditional branch toggle;
- recurring `ViewSlot` updates;
- recurring `ScrollPane` updates.

Its header comments still describe the file as a pre-composition “skeleton” while the adjacent `scoped-invalidation.test.ts` explicitly exercises retained composition. This is documentation drift, not evidence that the current implementation lacks the retained path.

### 7.4 Observability

Available diagnostics include:

- execution counters;
- composition exact-reuse/new-view counters;
- persistent-sequence structural counters;
- `pathologicalCommitFailures`;
- `trackedStateSubscriberCount`;
- `keyGroupOf(scope)`;
- scope fields such as `currentOutput`, `pendingOutput`, `currentProps`, `dirty`, `mounted`, and `disposed`.

Visibility gaps:

- no public package-root API exposes execution counters;
- no public API exposes scope trees or keyed-group maps;
- publication target internals are hidden behind runtime/control boundaries;
- no executed test results were available from this report;
- tests assert behavior but do not provide a general production route-trace facility.

### 7.5 Benchmark considerations

The composition source contains counters intended to prove:

- exact semantic reuse;
- changed versus no-op output commits;
- scoped invalidation;
- wide persistent sequence asymptotics.

No benchmark source was analyzed as part of this assignment, and no benchmark was run. The available counters make it possible for adjacent benchmark assignments to distinguish execution-frontier work from native/transport work.

## 8. Cross-boundary findings and contradictions

### 8.1 Composition versus semantic `View`

`View` construction and composition are deliberately interdependent:

```text
api/view/view.ts
  → if retained construction:
      composition/compose.ts
  → otherwise:
      create immutable semantic node directly
```

`execution-context.ts` is kept free of `View` imports so that `View.key` can switch child-owner context without creating an import cycle (`execution-context.ts:5-7`).

The composition layer therefore does not own semantic node normalization itself. It consumes normalized semantic nodes and delegates actual immutable node creation to the `View`/semantic modules.

### 8.2 Composition versus transport

Composition does not import native transport or generated ABI code. It only refers to publication contracts and public/API types. The runtime supplies transport-aware publication implementations.

The native boundary is therefore:

```text
component scope / semantic output
  → StructuralPublicationTarget
      → ViewSlot / RetainedRootBoundary
          → retained structural transport
              → native host
```

This preserves a generic framework boundary: composition decides retained execution and semantic output continuity; runtime/control modules decide where the semantic output is installed.

### 8.3 Composition versus root runtime

`Tui` owns one shared `RetainedExecutionRuntime` (`runtime.ts:120-125`). The same runtime is used for:

- the canonical root producer;
- projected component scopes;
- `ViewSlot` builder roots;
- `ScrollPane` builder roots.

This means one shared dirty queue can contain scopes from multiple structural boundaries, while each scope carries its own publication target/projection.

### 8.4 Child identity versus execution identity

The source explicitly prevents `View.key` from replacing the active execution scope. The keyed owner only affects where child component invocations reconcile.

This avoids two common ownership errors:

1. treating `View.key` as an independently schedulable component;
2. associating state reads inside the keyed thunk with the keyed group instead of the actual enclosing execution scope.

The source comments repeatedly state the intended split:

```text
identity = View.key
execution = defineView
invalidation = State<T>
```

(`view.ts:258-261`).

### 8.5 Scope identity versus semantic node identity

A scope may retain a stable execution identity while producing a new semantic `View` object, and a semantic `View` may be reused while the scope body itself does not execute.

The implementation tracks these independently:

- scope IDs in `RetainedExecutionScope`;
- semantic node IDs in `View`/`semantic-node.ts`;
- local handle IDs for component/content/state attachments;
- native refs and leases behind publication/transport.

This is consequential for debugging: a changed semantic node does not automatically mean a new component scope, and a stable component projection node does not mean the child body ran.

### 8.6 Normalization and attachment retention

Public values are copied and frozen into semantic records before composition compares them. Attachment references are separately retained through semantic-node weak-map sidecars and copied across immutable derivations.

This means composition’s equality checks generally compare normalized values and local attachment IDs, not the identity of arbitrary public style/state/content wrapper objects.

For example:

- `composeComponent` compares component semantic `handleId`;
- `composeContent` compares content attachment ID;
- `composeState` compares state attachment ID plus all other semantic fields;
- style and decoration comparisons compare normalized semantic colors, attributes, borders, and scalar fields.

### 8.7 Historical/documentation drift

Observed contradictions or stale wording:

1. `define-view.ts:22-24` says keyed dynamics “land in R8,” but the current source contains the public `View.key` path and keyed-owner implementation.
2. `execution-context.ts:93-104` contains two adjacent documentation blocks describing keyed-group resolution; this is redundant commentary but does not change behavior.
3. `packages/tui-consumer-fixture/tests/consumer.test.ts:4-12` calls itself a pre-composition skeleton, while `scoped-invalidation.test.ts` in the same package validates the current retained runtime.
4. Several source comments refer to historical PERF/R-series stages, while current source has completed later revisions. Those comments are useful implementation history but should not be treated as separate runtime paths.

### 8.8 No generic framework boundary violation found in scope

The composition modules do not hard-code agent, assistant, model, tool, conversation, provider, or other application semantics. Their generic concepts—component, state, key, view, content, publication, execution, and scheduling—are caller-defined and usable by arbitrary terminal applications.

## 9. Open questions and coverage gaps

1. **Runtime validation was not executed.** The report relies on static source and test assertions. It does not claim that the suites pass at the baseline.
2. **Native publication internals are outside primary scope.** The exact retained-DAG behavior behind `StructuralPublicationTarget`, including native lease transitions and materialization route details, belongs to adjacent transport/runtime assignments.
3. **No direct public introspection of scope ownership exists.** Production callers cannot inspect the retained scope tree, pending queue, owner maps, or dependency sets.
4. **The `ChildRecord.key` field is not populated with the public key in current keyed invocation routes.** Key identity resides in the `KeyGroup` map. This is intentional according to comments but may confuse diagnostics or future consumers of `ChildRecord`.
5. **`propsShallowEqual` only compares enumerable string-keyed own properties.** Symbols, non-enumerables, and deep object content are outside the equality contract.
6. **Component wrapper mutability is not externally guarded at runtime.** The public type marks `.render` readonly, but the implementation assigns it as a function property. Static TypeScript consumers are expected to honor the type contract.
7. **Commit-phase failure semantics are intentionally not ordinary rollback.** The runtime records pathological failures but leaves state unspecified after a throw in phase 3. The surrounding publication implementations are expected to make commit effectively infallible.
8. **No benchmark measurements were captured.** Counter fields exist, but no numeric values can be reported from static inspection.
9. **No complete production route census beyond inspected composition/runtime edges was performed.** In particular, the full set of all package consumers and alternate native materialization paths belongs to broader wiring/audit assignments.
10. **Conditional composition remains positional.** There is no source-site identity for raw semantic factory calls. A caller changing conditional structure can shift slot positions and cause fresh semantic values or child remounts; the current contract does not provide branch-preserving reconciliation for arbitrary positional changes.
11. **A keyed group has no independent state dependency set.** If a caller expects state read inside `View.key` to invalidate only that keyed instance, current source contradicts that expectation: the read belongs to the enclosing execution scope unless the read occurs inside a nested `defineView` body.
12. **The relationship between public direct `Tui.render(scene)` and retained `Tui.render(builder)` is intentionally separate but broader route behavior belongs to runtime integration.** This report only traced the relevant root ownership transition.

## 10. Evidence appendix

### 10.1 Primary inspected production files

Exhaustive assigned-directory manifest:

```text
packages/iyon-tui/src/composition/child-owner.ts
packages/iyon-tui/src/composition/compose.ts
packages/iyon-tui/src/composition/define-view.ts
packages/iyon-tui/src/composition/execution-context.ts
packages/iyon-tui/src/composition/execution.ts
packages/iyon-tui/src/composition/persistent-seq.ts
packages/iyon-tui/src/composition/publication.ts
packages/iyon-tui/src/composition/tracked-state.ts
```

### 10.2 Supporting files inspected

```text
AGENTS.md
PRE-V5-ARCHITECTURE-REPORT.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/evidence/assignments.json
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt

packages/iyon-tui/src/index.ts
packages/iyon-tui/src/api/view/view.ts
packages/iyon-tui/src/api/view/semantic-node.ts
packages/iyon-tui/src/api/presentation/semantic-style.ts
packages/iyon-tui/src/api/controls/view-slot.ts
packages/iyon-tui/src/api/controls/scroll-pane.ts
packages/iyon-tui/src/runtime/runtime.ts

packages/iyon-tui/tests/tui_h3_b_composition.test.ts
packages/tui-consumer-fixture/src/consumer.ts
packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts
packages/tui-consumer-fixture/tests/consumer.test.ts
```

Files merely indexed but not comprehensively read are not treated as evidence for composition behavior. Broader repository files, generated bodies, native Rust sources, transport implementation details, and unrelated tests were not claimed as exhaustively inspected.

### 10.3 Exact symbols and key line ranges

#### Composition ownership and identity

- `ChildOwnerState`: `composition/child-owner.ts:27-69`
- `ChildRecord`: `composition/child-owner.ts:72-77`
- `KeyGroup`: `composition/child-owner.ts:79-89`
- `RetainedExecutionScope`: `composition/execution.ts:109-219`
- `ScopeSemanticTable`: `composition/execution.ts:52-101`
- `RetainedExecutionRuntime`: `composition/execution.ts:328-604`
- `reconcileChild`: `composition/execution.ts:941-974`
- `invokeChild`: `composition/execution.ts:976-1011`
- `invokeInto`: `composition/execution.ts:1013-1041`
- `promoteOwnedChildren`: `composition/execution.ts:1070-1097`
- `disposeOwnedChildren`: `composition/execution.ts:1108-1126`
- `keyGroupOf`: `composition/execution.ts:1142-1164`
- `invokeComponent`: `composition/execution.ts:1171-1180`

#### Active context and keyed routing

- `executionContext`: `composition/execution-context.ts:25-45`
- `withoutRetainedComposition`: `composition/execution-context.ts:47-55`
- active frame stack: `composition/execution-context.ts:74-91`
- `resolveKeyedGroup`: `composition/execution-context.ts:93-114`
- `withKeyedChildOwner`: `composition/execution-context.ts:116-131`
- `View.key`: `packages/iyon-tui/src/api/view/view.ts:249-265`

#### Public component/state APIs

- `ViewComponentType`, `ViewComponent`: `composition/define-view.ts:37-46`
- `defineView`: `composition/define-view.ts:48-65`
- `TrackedStateSource`: `composition/tracked-state.ts:27-39`
- `StateSource`: `composition/tracked-state.ts:41-93`
- `State<T>`: `composition/tracked-state.ts:95-108`
- `state`: `composition/tracked-state.ts:110-125`
- subscriber diagnostics: `composition/tracked-state.ts:127-136`

#### Composition and normalization

- slot staging: `composition/compose.ts:73-83`
- modifier tags: `composition/compose.ts:85-101`
- normalized style/decoration comparators: `composition/compose.ts:103-296`
- retained decoration path: `composition/compose.ts:298-350`
- text/styled text/spacer/component/content/state helpers: `composition/compose.ts:356-503`
- axis helpers: `composition/compose.ts:528-589`
- grid helpers: `composition/compose.ts:666-735`
- diff behavior: `composition/compose.ts:737-750`
- layout patch helpers: `composition/compose.ts:770-842`
- validation helpers: `composition/compose.ts:844-878`

Adjacent normalization evidence:

- semantic node vocabulary and IDs: `api/view/semantic-node.ts:161-294`
- semantic node association/freeze: `api/view/semantic-node.ts:296-426`
- semantic derivation sidecars: `api/view/semantic-node.ts:463-549`
- wide sequence sidecars: `api/view/semantic-node.ts:552-611`
- semantic color/style normalization: `api/presentation/semantic-style.ts:39-68`
- decoration normalization: `api/presentation/semantic-style.ts:104-157`
- style merging/cloning: `api/presentation/semantic-style.ts:159-215`

#### Publication and transactions

- publication interfaces: `composition/publication.ts:4-39`
- evaluation: `composition/execution.ts:627-657`
- recursive publication preparation: `composition/execution.ts:659-707`
- commit batch/scope: `composition/execution.ts:709-781`
- abort batch/level: `composition/execution.ts:783-842`
- owned builder root mounting: `composition/execution.ts:844-886`
- root detachment: `composition/execution.ts:888-900`
- `OwnedBuilderRoot`: `composition/execution.ts:1185-1253`

Adjacent publication ownership:

- one retained runtime and projection factory: `runtime/runtime.ts:120-196`
- root publication preparation: `runtime/runtime.ts:217-277`
- canonical render route: `runtime/runtime.ts:391-497`
- direct render route: `runtime/runtime.ts:499-549`
- root builder disposal: `runtime/runtime.ts:730-735`
- runtime teardown ordering: `runtime/runtime.ts:786-814`
- `ViewSlot` builder/direct ownership: `api/controls/view-slot.ts:150-282`
- `ScrollPane` builder/direct ownership: `api/controls/scroll-pane.ts:112-219`

#### Persistent wide structures

- counters: `composition/persistent-seq.ts:3-22`
- persistent node vocabulary: `composition/persistent-seq.ts:43-60`
- path-copying mutation helpers: `composition/persistent-seq.ts:127-167`
- split/concat helpers: `composition/persistent-seq.ts:175-213`
- public `PersistentSeq` operations: `composition/persistent-seq.ts:215-309`

### 10.4 Behavioral evidence files and assertions

- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts:15-27`
  - semantic View construction is authoritative and frozen.
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts:29-64`
  - exact retained reuse and changed semantic output.
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts:66-83`
  - component semantic identity uses local `HandleId`.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:39-70`
  - state invalidates only the reading scope.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:72-114`
  - 1,000 sibling scopes and narrow invalidation frontier.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:116-153`
  - keyed reorder and per-key body skip.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:155-191`
  - builder/direct ownership transition and stale-builder suppression.
- `packages/tui-consumer-fixture/src/consumer.ts:126-169`
  - public-only consumer composition using `defineView`, `state`, `View.key`, and `Scene`.
- `packages/tui-consumer-fixture/tests/consumer.test.ts:33-118`
  - direct scene, ViewSlot, ScrollPane, conditional, and recurring update paths.

### 10.5 Validation status

No test or benchmark was run for this report. The report therefore distinguishes:

- source facts reconstructed from the baseline;
- expected behavior asserted by inspected tests;
- static inferences about ownership and lifetime;
- unverified runtime outcomes.