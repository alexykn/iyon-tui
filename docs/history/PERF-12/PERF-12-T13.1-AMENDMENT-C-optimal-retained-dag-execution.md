# PERF-12 Tranche 13.1 Amendment C
## Incremental Retained Execution over Independently Retained Immutable View DAG Roots

**Status:** normative amendment to `PERF-12-T13.1-retained-view-composition-handoff.md`.

**Supersedes:** both `PERF-12-T13.1-AMENDMENT-runtime-retained-composition.md` (Amendment A) and `PERF-12-T13.1-AMENDMENT-B-retained-execution-scopes.md` (Amendment B) wherever they conflict. Amendment C is the authoritative correction.

**Starting implementation state assumed by this amendment:**

- T13.1 Step 1 has been implemented.
- T13.1 Step 2 has been implemented substantially according to the original handoff.
- T13.1 Step 3 has been implemented substantially according to the original handoff.
- Step 4 has only been partially explored/implemented up to the lexical SiteId / source-transform decision point.
- No Step 4 source-transform design is considered committed architecture by this amendment.

**Repository baseline for the surrounding PERF-12 work:** `f665c9ef913a7a8eda552a385b072f25f853b359` on `perf-refactor` unless the implementation branch has advanced locally.

---

# 0. Executive correction

The previous T13.1 handoff correctly identified the missing problem:

> repeated ordinary application state updates do not automatically preserve the immutable `View -> BridgeViewNode` DAG identity that PERF-12 needs in order to cut off unchanged native work.

Its first solution, however, optimized the wrong unit.

The original retained-composition design primarily said:

```text
run the render/build code again
        |
        v
look up previous semantic slots
        |
        +-- equal   -> return the previous View
        |
        +-- changed -> build a new View
```

That is useful, but it is not the desired final architecture.

It still permits this shape:

```text
root build executes
  header build executes
  composer build executes
  footer build executes

only afterward do we discover that 3/4 of them were unchanged
```

For PERF-12, the stronger target is:

```text
footer dependency changes
        |
        v
mark Footer execution scope dirty
        |
        v
execute Footer only
        |
        v
produce only Footer's new immutable semantic frontier
        |
        v
install only Footer's retained sub-DAG root

Header execution:   0
Composer execution: 0
App execution:      0   when the change is local to Footer
```

This amendment therefore changes the primary abstraction from:

```text
memoized semantic construction slots
```

to:

```text
persistent retained execution scopes
        +
independently retained immutable sub-DAG roots
        +
tracked invalidation
```

Semantic construction slots remain valuable *inside a scope that genuinely executes*, but they are no longer the system that discovers all changes by replaying the whole declarative program.

The new slogan is:

> **Do not re-execute clean scopes. Do not rebuild clean semantic nodes. Preserve immutable DAG identity all the way down.**

---

# 1. Why this amendment exists

PERF-12 proved that the retained DAG is extremely effective once identity already exists:

```text
unchanged semantic identity
    -> zero descendant semantic traversal
    -> zero payload re-transport
    -> zero native reconstruction
```

The unresolved application-layer question was:

```text
how does normal declarative code keep/generate that identity when application state evolves?
```

There are two superficially similar answers that are not equivalent.

## 1.1 Weak answer: replay everything cheaply

```text
3 semantic regions
next update changes region B

run A -> compare -> reuse
run B -> compare -> change
run C -> compare -> reuse
```

This avoids allocating A and C, but A and C were still executed as semantic construction work.

For a deep or broad application tree this can become:

```text
O(all executed UI sites)
```

per update even when the native retained frontier is tiny.

That is not the final architecture required here.

## 1.2 Strong answer: retain the execution graph too

```text
A scope clean        -> do not execute A
B scope invalidated  -> execute B
C scope clean        -> do not execute C
```

The resulting semantic output remains immutable.

Only B obtains a new output DAG root.

This is the model this amendment requires.

---

# 2. Research conclusions

This section records the architectural lessons that matter. It is not a mandate to reproduce another framework mechanically.

## 2.1 React Fiber: persistent execution identity is separate from React elements

React's persistent `Fiber` objects are not the same thing as the ephemeral element values returned by rendering.

A Fiber records persistent execution/lifecycle identity across updates. React also maintains current/work-in-progress state rather than treating every render value as a completely unrelated tree.

The important PERF-12 lesson is not the DOM or JSX.

It is:

```text
persistent execution node
    + pending-work metadata
    + child pending-work metadata
    -> clean subtrees can be skipped without invoking them
```

Current React source explicitly checks whether a Fiber's children contain work and returns immediately when they do not. If the Fiber itself has no work but a descendant does, React can continue into the child subtree without re-running the clean Fiber's component body.

That is the property Iyon wants.

### React identity

React associates ordinary identity with parent-local tree position/type. `key` replaces positional identity when the application has real movable/repeated identity.

Keys are local, not application-global identifiers.

Do not make Iyon users assign global IDs to every component or View.

### React Compiler

React Compiler is distinct from Fiber identity and scheduling.

Its current role is automatic memoization: reducing cascading re-renders and repeated calculations by generating memo-cache checks.

Important lesson:

```text
compiler memoization is useful
but it is not the substrate that makes a state update target a Fiber
```

Iyon should not depend on a source transform merely to obtain the persistent execution identity that a canonical component API can provide directly.

## 2.2 Jetpack Compose: restart scopes + tracked state reads

Compose provides the closest model for dependency-driven invalidation.

The relevant concepts are:

```text
restart group
state read recorded inside that group
state write invalidates that group
group can later be recomposed independently
unchanged/skippable groups can be skipped
```

Compose documents that snapshot-state reads are associated with restart scopes and that changing the state schedules the composables that read it.

The compiler inserts restart-group machinery because Kotlin composables otherwise look like ordinary function calls.

Compose also compares function inputs and skips eligible composables whose inputs did not change.

This yields two complementary invalidation channels:

```text
1. local observed-state change
   -> exact restart scope invalidated

2. parent re-executes with child inputs
   -> child can be skipped when its inputs are unchanged
```

Iyon should have both.

## 2.3 Flutter: dirty retained Elements, immutable Widgets

Flutter's official architecture description is especially relevant to Iyon's immutable semantic values.

Flutter keeps immutable Widget configurations but also keeps a persistent Element tree. Dirty Elements are scheduled directly; clean Elements are not rebuilt merely because another Element became dirty.

Flutter explicitly describes widget building as sublinear because the framework keeps a dirty-element list and jumps directly to dirty retained Elements.

This is almost exactly the separation Iyon needs:

```text
immutable description value    <-> View / BridgeViewNode DAG
persistent execution object    <-> RetainedExecutionScope
```

Flutter also notes that identical immutable widget instances allow immediate cutoffs. This matches PERF-12's identity semantics closely.

## 2.4 Keys across React / Compose / Flutter

All three converge on the same public design principle:

```text
ordinary identity:
    parent-local structural position / type

dynamic repeated or movable identity:
    explicit key
```

Iyon should therefore expose keys for real logical ambiguity, not make IDs the normal API.

## 2.5 Fine-grained reactive systems

Fine-grained signal systems demonstrate the lower bound:

```text
observable write
    -> exactly the computations that read it are invalidated
```

Iyon does not need to turn the semantic DAG into reactive mutable nodes.

But a small generic observable-value primitive is useful as an *invalidation source* for retained execution scopes.

The semantic result of every successful scope execution remains a new immutable View snapshot.

## 2.6 The lesson extends past component execution into layout and paint

A retained execution system is only half-finished if it jumps directly to one dirty component in JavaScript and then makes the renderer rediscover the entire mounted component/layout forest.

Flutter's current architecture documentation is particularly instructive here. It does not stop at a dirty Element list: layout is also retained, clean render objects can return immediately when constraints are unchanged, and only a limited region around dirty layout nodes is revisited. Compose similarly records state reads by phase; a read that only affects layout or draw need not necessarily force composition.

The Iyon analogue is:

```text
execution invalidation frontier
        -> semantic DAG frontier
        -> component/mount frontier
        -> layout dependency frontier
        -> paint frontier
```

These frontiers are related but not identical. A text/color change can require semantic + paint work without changing parent layout. A child height change can require layout work on an ancestor/sibling placement frontier even though clean component bodies still must not execute.

Therefore the acceptance target is not merely:

```text
one dirty JS body
```

but:

```text
no phase performs O(total application scopes/nodes) work unless the semantics of that phase genuinely require it.
```

This amendment explicitly rejects moving an O(N) rediscovery pass from the JS construction layer into component resolution or layout and then declaring success.

---

# 3. Final architecture

The final T13.1 architecture is:

```text
+--------------------------------------------------------------+
| Application                                                   |
|                                                              |
| ordinary state / props                                       |
| optional generic tracked State<T> values                     |
| user-defined View components                                 |
+------------------------------+-------------------------------+
                               |
                               v
+--------------------------------------------------------------+
| Retained execution graph                                      |
|                                                              |
| RetainedExecutionScope                                        |
| - component identity                                          |
| - current inputs                                              |
| - tracked State<T> dependencies                               |
| - dirty state                                                 |
| - child component scopes                                      |
| - current immutable output View                               |
| - independently retained sub-DAG boundary                     |
+------------------------------+-------------------------------+
                               |
                 only dirty scopes execute
                               |
                               v
+--------------------------------------------------------------+
| Scope-local semantic construction                             |
|                                                              |
| T13.1 T3 monomorphic semantic helpers                         |
| - exact old View reuse on immediate semantic equality         |
| - new immutable node only when semantics changed              |
| - derivation hint where a proven PERF-12 lane exists          |
+------------------------------+-------------------------------+
                               |
                               v
+--------------------------------------------------------------+
| Immutable View -> BridgeViewNode DAG                          |
|                                                              |
| each retained execution scope owns one current immutable root |
+------------------------------+-------------------------------+
                               |
                               v
+--------------------------------------------------------------+
| Existing PERF-12 retained boundary                            |
| NativeRef / BridgeNativeHint / PersistentSeq / root leases    |
+------------------------------+-------------------------------+
                               |
                               v
+--------------------------------------------------------------+
| Retained host dependency graph                                |
|                                                              |
| component revision/mount topology                             |
| -> retained layout dependencies / measured subtrees           |
| -> retained paint dependencies                                |
|                                                              |
| local scope root swaps invalidate only required host frontier |
+------------------------------+-------------------------------+
                               |
                               v
+--------------------------------------------------------------+
| Rust retained View DAG / terminal frame                       |
+--------------------------------------------------------------+
```

The crucial change is that the application is no longer forced to have one monolithic semantic root whose construction code must run for every state update.

User-defined View components become independently retained execution boundaries.

---

# 4. Do not build another semantic graph

`RetainedExecutionScope` is not another View IR.

It must not duplicate:

```text
text payloads
styles
layout child arrays
Grid cells
Diff hunks
BridgeViewNode payloads
native records
```

It stores execution/lifecycle information and pointers to the immutable semantic outputs.

Conceptually:

```ts
interface RetainedExecutionScope<P = unknown> {
  readonly type: ViewComponentType<P>
  readonly key: ViewKey | undefined

  parent: RetainedExecutionScope | null

  currentProps: P | undefined
  currentOutput: View | undefined

  children: ScopeChildSet
  dependencies: StateDependencySet

  dirty: boolean
  disposed: boolean

  // Existing retained-boundary ownership adapted for a scope root.
  boundary: RetainedRootBoundary

  // Stable semantic component/ref View presented to its parent, if the
  // scope is represented through an independently replaceable component root.
  projection: View

  // Scope-local semantic memo slots from the adapted T2/T3 work.
  semanticSlots: SemanticSlot[]
}
```

Exact layout is implementation-dependent.

The semantic source of truth remains:

```text
currentOutput: View
    -> nodeForBridge(currentOutput)
```

---

# 5. The central optimization: each component scope owns a retained sub-DAG root

This is the Iyon-specific advantage over copying React literally.

The project already has a semantic component node and `ViewSlot`-style independently replaceable content.

Use that mechanism, or a dedicated lower-overhead internal equivalent, as the execution-scope projection.

Conceptually:

```text
parent semantic DAG

Column
├── ScopeRef(Header scope)
├── ScopeRef(Composer scope)
└── ScopeRef(Footer scope)
```

Each `ScopeRef` is stable while that logical component instance remains mounted.

The scope's content is a separate immutable DAG root:

```text
Footer scope

Decorated
└── Text("Working")
```

When Footer changes:

```text
parent Column NodeId                 SAME
Header ScopeRef NodeId               SAME
Composer ScopeRef NodeId             SAME
Footer ScopeRef NodeId               SAME

Footer scope content root NodeId     NEW
```

The outer application DAG does not need to be reconstructed at all for a Footer-local state change.

This is stronger than immutable parent path-copying and is exactly why existing component indirection is strategically useful.

## 5.1 Scope forest, not one giant mutable tree

The whole UI becomes conceptually:

```text
immutable root DAG
    |
    +-- stable ScopeRef A -> immutable sub-DAG A
    |
    +-- stable ScopeRef B -> immutable sub-DAG B
    |
    +-- stable ScopeRef C -> immutable sub-DAG C
```

Every sub-DAG root remains immutable.

Only the stable component/ref handle is mutable in the already-existing sense that it can point the renderer at a newly installed retained root.

This is consistent with existing `View.component(handle)` semantics and existing high-frequency `ViewSlot` updates.

## 5.2 Source audit: the current repository already contains most of the right substrate

The `f665c9e` source audit makes this design substantially less speculative than a green-field Fiber clone.

The TypeScript `ViewSlot` already:

```text
- has stable native component identity
- owns a `RetainedRootBoundary`
- retains the current root across replacements
- installs a changed root independently via `setViewRef`
- preserves the old root until retained materialization succeeds
```

The Rust `View::component` representation is already a stable `ComponentSlot(ComponentId)` semantic node rather than an inline copy of component content.

The Rust `ComponentRegistry` already stores persistent component entries with:

```text
component identity
revision
cached immutable View snapshot
```

and mutation/invalidation increments the revision and drops only that component's cached snapshot.

The layout engine already recognizes component slots specially. Its `MeasureKey::with_component(...)` includes the resolved component output View identity, so a changed component output cannot legitimately reuse a measurement keyed to the old output.

These are exactly the primitives Amendment C needs. Do not replace them with a second generic virtual tree.

## 5.3 Source audit also exposes the remaining scalability gap

Current correctness behavior is not yet sufficient evidence for the final performance architecture.

The current Rust resolver still scans semantic branches that advertise component identity in order to rebuild the resolution overlay/mount information. A parent containing many component slots can therefore still incur work proportional to the number of mounted component references even when exactly one independently retained child changed.

Likewise, the current layout path deliberately marks component-containing semantic nodes as non-cacheable in important cases. That is safe, but it means a local component revision can cause an unchanged component-bearing parent to be remeasured and its child collection revisited.

That would move the O(N) problem downstream rather than solve it. T13.1 must not stop there.

## 5.4 Retain component mount topology across local component updates

`SceneHost` already retains a `MountGraph`; make it authoritative incremental state rather than reconstructing the whole mounted component forest after every local scope-root replacement.

For a local execution-scope commit:

```text
component scope S output root changes
        |
        +-- component topology inside S unchanged
        |       -> update S revision/snapshot only
        |       -> do not rescan parent/sibling component topology
        |
        +-- component topology inside S changed
                -> re-resolve only S's mounted descendant subtree
                -> patch the retained MountGraph transactionally
```

A parent's immutable semantic output did not change merely because a descendant component swapped its independent root. Therefore its child component topology cannot have changed. The framework already has the provenance necessary to avoid rediscovering that fact.

The mount graph must provide enough reverse ownership information to locate the affected retained component subtree in O(depth + changed mounts), not O(total mounts).

## 5.5 Retained layout dependency invalidation

A local component update may change:

```text
paint only
text wrapping
intrinsic width/height
row count
child component topology
```

The correct algorithm is not 'rerun layout for the whole Scene because components are dynamic'.

Retain layout results/dependencies and invalidate from the changed component outward. Conceptually each cached measured/layout node records enough dependency information to answer:

```text
which ComponentId/revision(s) did this result depend on?
what parent layout result depends on this node's geometry?
what constraints were supplied to this child last time?
```

Use a reverse dependency index or an equivalent retained dirty-propagation structure so a component revision marks only affected measurement/layout entries dirty. Do not compute a fresh hash/fingerprint by walking every descendant revision on each frame.

Required behavior:

```text
child scope content changes
        -> remeasure changed scope under previous constraints

if measured geometry/layout facts are unchanged:
        -> parent layout remains valid
        -> invalidate/repaint only affected paint subtree/region

if geometry changed:
        -> propagate layout dirty only to ancestors that consume that geometry
        -> reuse cached measurements for unchanged siblings
        -> reposition/repaint only the necessary layout frontier
```

This is the same principle Flutter uses for sublinear subsequent layout: unchanged children under unchanged constraints cut off immediately, and layout dirtiness propagates only where geometry dependencies require it.

A vertical list whose changed child height shifts every later child may legitimately require O(suffix) placement/paint work. That is real semantic geometry work. But it must not require re-executing those siblings' component bodies, rebuilding their semantic DAGs, or remeasuring their content when their constraints and measurements are still valid.

## 5.6 Component-content changes must invalidate layout correctly

The optimization above may never make layout stale. T13.1 must prove both directions:

```text
child scope changes content but keeps same measured geometry
    -> parent layout cache remains valid
    -> output matches cold rebuild

child scope grows/shrinks or changes layout-relevant facts
    -> required ancestor layout is invalidated
    -> unchanged sibling measurements are reused where valid
    -> output matches cold rebuild
```

Do not infer layout stability from semantic equality of the component projection View: the projection intentionally remains stable while its independently retained content changes. Layout validity must be tied to the component content revision/output and retained layout dependencies.

---

# 6. Public component API

The exact naming should fit the existing package, but the public model should be this simple.

Example:

```ts
import { defineView, View } from "iyon:tui"

const Header = defineView<{ title: string }>(({ title }) =>
  View.text(title).bold()
)

const Footer = defineView<{ status: string }>(({ status }) =>
  View.text(status)
)
```

Usage:

```ts
tui.render(() =>
  new Scene(
    View.vertical(column => {
      column.child(Header({ title: state.title }))
      column.child(Footer({ status: state.status }))
    }),
    history,
  )
)
```

A component invocation returns a normal `View` to its parent.

That View is the stable scope projection, not the component body's latest raw output root.

Users do not see:

```text
RetainedExecutionScope
ViewSlot
NativeRef
NodeId
BridgeViewNode
scope ids
subscriptions
```

## 6.1 No required IDs for ordinary components

Identity is parent-local:

```text
parent scope
+ component type
+ ordinary position
```

Repeated/reordered instances use `key`.

Example:

```ts
View.key(tool.id, () => ToolCard({ tool }))
```

or a component-specific keyed convenience if that is more ergonomic.

The key is local to the parent/repeated site.

No global registry.

## 6.2 Props comparison

When a parent scope genuinely executes, each existing child component invocation should perform a cheap skip check before executing the child's body.

Default component props comparison:

```text
same key/type
same own prop-key set
Object.is(oldPropValue, newPropValue) for every prop
    -> skip child body
```

This is deliberately similar to React's ordinary memo semantics and Compose's parameter-skipping model.

Rules:

```text
primitive props        compare by Object.is
object/function props  compare by identity
```

Do not recursively deep-compare arbitrary props.

That would reintroduce hidden O(tree/data) work.

Application values passed as props should therefore be immutable/stable in the normal JavaScript sense.

A custom comparator may exist as an advanced API only if it is clearly semantic and cannot compromise correctness, but it is not required for T13.1.

---

# 7. Generic tracked state for exact dirty-scope invalidation

Props skipping alone solves cascading parent-driven updates, but it does not let the framework directly target a deeply nested scope when an external value changes.

To obtain Compose/Flutter-style direct invalidation, add a very small generic observable primitive.

This is not a reducer/store/state-management framework.

It is an invalidation source.

Conceptually:

```ts
export interface State<T> {
  readonly value: T
  set(value: T): void
  update(update: (previous: T) => T): void
}

export function state<T>(initial: T): State<T>
```

While a retained execution scope is running:

```ts
const value = someState.value
```

records:

```text
someState -> current RetainedExecutionScope
```

When `someState` changes by `Object.is`:

```text
for each subscribed live scope:
    mark scope dirty
    enqueue scope once
```

On the next flush:

```text
only the dirty scopes execute
```

## 7.1 Dependency refresh

Dependencies are execution-dependent.

Therefore when a scope re-executes:

```text
old subscriptions remain valid until successful commit
new reads are collected into pendingDependencies

commit:
    unsubscribe dependencies no longer read
    subscribe newly read dependencies

abort:
    retain old dependency set
```

Do not destructively clear the committed dependency set before the scope update succeeds.

## 7.2 Component evaluation is pure and synchronous

The tracked-state mechanism should stay simple by making component execution a pure calculation.

During a retained component body:

```text
State.value read     -> allowed and tracked
State.set/update      -> reject deterministically
Promise/async return  -> reject deterministically
external host mutation through composition internals -> forbidden
```

This avoids needing Compose's much larger mutable-snapshot conflict system merely to make UI construction transactional. State writes occur before/after component evaluation, enqueue dirty scopes, and are batched into the next work transaction.

The rule is generic framework semantics, not an Iyon convention. It also makes aborted component evaluation straightforward: committed state subscriptions/output remain authoritative until the new work commits.

## 7.3 No hidden mutable-state guessing

The framework cannot correctly observe arbitrary mutation to plain JavaScript objects.

Do not attempt:

```text
stack inspection
runtime source parsing
Proxy every application object implicitly
deep object diff
content hashing
```

If application state is external/opaque, the supported update path is explicit root/component props updates.

This is not a missing optimization. It is an information boundary.

Other mature frameworks likewise require update provenance:

```text
React    -> setState/hook update is attached to a Fiber
Compose  -> observable State read/write is tracked by a restart scope
Flutter  -> setState / inherited dependency marks an Element dirty
Solid    -> signal write notifies subscribers
```

Iyon should do the same instead of pretending it can infer writes to arbitrary JavaScript.

---

# 8. Update algorithms

## 8.1 Local tracked-state update: ideal hot path

Initial tree:

```text
App scope
├── Header scope
├── Composer scope
└── Footer scope
```

`Footer` read `statusState.value` during its last successful execution.

Now:

```ts
statusState.set("Running tool")
```

Required work:

```text
1. State marks Footer scope dirty.
2. Footer enters dirty queue.
3. Header is untouched.
4. Composer is untouched.
5. App is untouched.
6. Footer body executes once.
7. Scope-local T3 semantic helpers reuse unchanged Footer nodes.
8. Footer obtains a new immutable output root only if semantic output changed.
9. Footer retained root boundary materializes only its changed frontier.
10. Footer ScopeRef/component content swaps to the new root.
11. Native component revision invalidates required layout/paint.
12. One frame/host commit is produced.
```

Structural proof requirement:

```text
app_scope_executions       = 0
header_scope_executions    = 0
composer_scope_executions  = 0
footer_scope_executions    = 1
```

This is the canonical proof that T13.1 did not merely make full replay cheaper.

## 8.2 Parent props update

Suppose an external store causes the app to call the root render boundary again.

The root scope executes because that is the update provenance supplied by the application.

Inside it:

```text
Header(old props == new props)   -> return stable Header ScopeRef, body not executed
Composer(same props)             -> body not executed
Footer(changed props)            -> mark/recompute Footer only
```

The root's own semantic construction helpers then see the same stable child ScopeRef Views.

If root structure/scalars are otherwise unchanged, the exact previous root View can be returned.

Therefore a child prop change can still leave the parent's semantic DAG root unchanged when the child is represented by an independent ScopeRef.

## 8.3 Parent structural dependency update

If `App` itself reads state that controls structure:

```ts
if (showFooter.value) {
    column.child(Footer(...))
}
```

then `App` is correctly subscribed to `showFooter`.

Changing it invalidates App.

App reruns because its own structure genuinely changed.

During that rerun:

```text
existing child component props unchanged -> bodies skip
new child component                      -> mount body once
removed child component                  -> pending unmount
```

Only App's semantic structure plus newly mounted/removed scopes changes.

That is necessary work.

## 8.4 Semantic no-op after invalidation

A State value may invalidate a scope yet the resulting semantic View may be identical.

Example:

```text
raw counter changes
formatted displayed bucket remains "10+"
```

The dirty scope executes, but T3 semantic equality returns the exact previous output View.

Then:

```text
no new NodeId
no native materialization
no component root swap
```

The scope becomes clean again.

---

# 9. Scope identity and child reconciliation

T13.1 needs a small retained child-scope reconciliation algorithm.

This is not View-tree reconciliation.

It reconciles only **component execution instances** owned by one parent scope.

## 9.1 Common unkeyed path

For ordinary children:

```text
same parent
same ordinal component position
same component type
no key
    -> same execution scope
```

A component function identity mismatch means replacement/remount.

## 9.2 Keyed path

For keyed repeated/movable children:

```text
same parent
same key
same component type
    -> same execution scope regardless of sibling movement
```

Use a Map only for keyed/reorder reconciliation.

The common non-keyed path should be array/index based.

## 9.3 Duplicate keys

Duplicate live keys under the same relevant parent/repeated context are deterministic errors.

Do not alias scopes.

## 9.4 Keys do not assert semantic equality

A key means:

```text
same logical component instance
```

not:

```text
same props
same View
```

Props/dependencies still decide whether the scope must execute.

---

# 10. Scope-local semantic construction: adapt T2/T3, do not throw it away

T1-T3 were useful work.

The correction is where that machinery lives.

Instead of being the global mechanism that every render replays, semantic slots belong to the execution scope that is already known to be dirty.

## 10.1 Adapted hot path

Inside a dirty scope:

```text
View.text(...)
    -> next scope-local semantic slot
    -> compare immediate semantics with previous slot View
    -> exact match: return exact old View
    -> mismatch: construct new immutable View
```

Same for modifiers/containers.

The scope-local slot table is allowed to use a simple dense cursor because it is only an allocation/reuse optimization within a scope that already needed execution.

A control-flow shift may reduce semantic-slot reuse inside that dirty scope, but:

```text
it cannot select another component instance
it cannot corrupt retained execution identity
it cannot produce stale semantics
```

Immediate semantic equality still authorizes reuse.

## 10.2 Why this no longer threatens the DAG

The previous concern was:

```text
one leaf changed
-> execute every semantic site in the entire application
```

After this amendment:

```text
one Footer-local state changed
-> only Footer scope executes
-> only Footer's semantic slots are visited
```

Therefore slot replay cost is bounded by the genuinely invalidated scope, not by the whole application tree.

## 10.3 Large/wide semantic content

Do not use scope-local sequential slots as a generic reconciliation engine for huge collections.

Existing specializations remain authoritative:

```text
History
streams
ViewSlot/ScopeRef
PersistentSeq wide axes
wide grid sidecars
specialized text paths
```

A fresh arbitrary 100,000-entry JavaScript array still contains 100,000 pieces of information. T13.1 does not claim to discover its mutation without reading it.

Use retained collection APIs when mutation provenance exists.

---

# 11. Compiler decision: explicit retained component scopes make a compiler unnecessary for the core guarantee

This amendment removes the lexical `SiteId` source-transform requirement from T13.1 after comparing what React Fiber, React Compiler, and Compose actually use their compiler/runtime layers for.

This is not a simplification that gives up dirty-scope execution. It is the opposite: the public component abstraction supplies a real persistent runtime execution boundary directly, and tracked `State<T>` supplies exact update provenance.

React demonstrates that persistent component identity and state-targeted scheduling do not require React Compiler. React Compiler improves automatic memoization of parent-driven component/value creation, but Fiber/state scheduling is already the retained execution substrate.

Compose needs compiler-generated restart/skipping groups because an `@Composable` call remains syntactically an ordinary Kotlin function call. Iyon does not need to hide its restart boundary: `defineView` can make it an ordinary, generic, explicit framework abstraction.

Iyon can therefore keep source code literal and still get direct dirty-scope scheduling:

```ts
const Footer = defineView(...)
```

The returned component wrapper is a real runtime type and can own persistent scope identity.

Therefore T13.1 does not need to rewrite:

```ts
View.text(...)
```

into hidden SiteId calls merely to know what an execution scope is.

## 11.1 React supports this decision

React Fiber identity and local state scheduling work without React Compiler.

React Compiler later improves automatic memoization of ordinary component calculations and JSX creation.

The separation is useful here too:

```text
retained execution correctness / scope scheduling
    !=
whole-language optimization compiler
```

## 11.2 Could a compiler help later?

Possibly, but this is not a missing part of the T13.1 execution architecture.

A future compiler could safely optimize arbitrary pure calculations *inside an already dirty scope*.

That would be an application-JavaScript optimization analogous to React Compiler.

The T13.1 guarantee is already:

```text
clean scopes do not execute
clean semantic DAGs do not rebuild
```

No future compiler is required to obtain that property.

## 11.3 Oxc decision

Do not add Oxc for T13.1 after this amendment unless another concrete required source transform appears.

If a future Iyon compiler is built, prefer a real production parser/semantic engine such as Oxc over a handwritten lexer/patcher.

But T13.1 should not carry an AST dependency for an architecture that no longer needs source rewriting.

---

# 12. Scheduler and dirty queue

The runtime needs a real dirty-scope scheduler.

Minimum shape:

```ts
class RetainedExecutionRuntime {
  private dirty: RetainedExecutionScope[]
  private dirtySet: Set<RetainedExecutionScope> // or generation bit to avoid Set on hot path
  private flushing = false

  invalidate(scope: RetainedExecutionScope): void
  flush(): void
}
```

Optimize representation after measurement.

A scope already dirty must not be enqueued repeatedly.

## 12.1 Batching

Multiple State writes in one JavaScript turn should coalesce into one flush/frame.

Do not perform a complete host repaint for every individual setter if several setters occur synchronously.

Required semantics:

```text
first invalidation in turn -> schedule one flush
later invalidations        -> join same dirty set
flush                      -> evaluate all required scopes
                           -> commit once
```

Use the smallest mechanism compatible with the existing runtime loop.

Do not introduce arbitrary animation latency.

Benchmark:

```text
single State.set -> visible next supported frame/flush
10 synchronous State.set calls -> one semantic/native commit batch
```

## 12.2 Parent-before-child invalidation

If both a parent and one of its descendants are dirty in the same batch because the parent structurally controls whether the descendant exists, evaluate the parent first.

After parent evaluation:

```text
if descendant remains mounted and still dirty -> execute it
if descendant was removed                    -> discard its dirty work
if parent execution already caused child execution with newer inputs -> do not double-execute
```

Track batch epoch/generation to make this deterministic.

---

# 13. Work-in-progress execution and one authoritative batch commit

The correct model is closer to React's work-in-progress/commit separation and Compose's prepare/apply discipline than to calling `ViewSlot.setView` eagerly as each dirty scope finishes.

A frame may contain several dirty component scopes, mount topology changes, and possibly a root Scene change. No committed host state may expose a partial mixture.

Required protocol:

```text
PREPARE JS WORK
  evaluate dirty scopes into pending outputs
  collect pending props and State dependencies
  stage child scope mounts/unmounts/reorders
  keep every committed scope/root untouched

PREPARE NATIVE WORK
  ensureNative/materialize each changed pending scope root
  retain every new NativeRef required by commit
  calculate component topology patches for scopes whose output topology changed
  validate target component/scope ids and generations
  old roots remain leased

COMMIT ONCE
  atomically swap all changed retained scope roots
  apply MountGraph/subtree topology patches
  install changed Scene root if any
  advance component revisions/generations
  invalidate retained layout/paint dependencies from changed components
  promote pending props/dependencies/child sets
  dispose committed unmounts
  release superseded root leases
  request one authoritative host frame

ABORT
  expose none of the pending roots/topology/dependencies
  keep old component roots, Scene root, MountGraph and subscriptions
  release only pending/new leases
```

## 13.1 Add a native/internal scope-root batch transaction if necessary

If the current host API can only mutate component targets one-by-one with externally observable failure between calls, that API is not sufficient for the final architecture. Add a small generic internal batch primitive rather than weakening transaction semantics.

Conceptually:

```text
prepare_scope_batch(changes...) -> validated transaction
commit_scope_batch(tx)          -> infallible-or-atomic publication
abort_scope_batch(tx)
```

The exact ABI may differ and should reuse PERF-12's existing temporary-lease/status discipline. The important property is that all failure-prone materialization/validation happens before committed roots are swapped.

If the implementation can prove that the final swaps are infallible after validation and all operations happen before paint/observation, a separate rollback log is unnecessary. Otherwise implement rollback or a true native batch. Do not rely on 'failures are unlikely'.

## 13.2 One scope update does not imply one terminal frame

Several dirty scopes in one turn are evaluated/materialized as one work transaction and committed once. A local single-scope update is the degenerate one-change transaction.

This keeps incremental execution from turning into excess host calls.

---

# 14. Relationship to existing ViewSlot and ScrollPane

## 14.1 ViewSlot

`ViewSlot` is strong precedent and likely the implementation substrate for retained execution-scope projections.

Do not expose one user-visible ViewSlot per component.

The framework may own an internal equivalent.

Requirements:

```text
- stable native component identity
- independently replaceable retained root
- root lease
- component revision/layout invalidation
- cheap O(1) scope projection View
```

If ordinary ViewSlot has extra public/animation machinery that makes one-per-component expensive, introduce a slimmer private `RetainedSubtreeSlot` / `ScopeSlot` sharing the same native component-root primitive.

The private hot representation should carry only what a retained execution scope needs:

```text
stable ComponentId / scope target id
current retained root NativeRef
component revision/generation
root lease
minimal layout/paint invalidation metadata
```

No animation arrays, public handle wrappers, or unrelated capability state should be paid per ordinary `defineView` component unless actually used.

Benchmark before deciding between direct reuse and the slim primitive, but keep the architecture: avoiding per-scope overhead is an implementation problem, not a reason to return to monolithic root replay.

## 14.2 ScrollPane

ScrollPane remains a specialized independently retained boundary.

Do not replace it with generic component scopes.

Its viewport/follow-end behavior is semantically distinct.

## 14.3 History and streams

Unchanged.

History already owns a retained collection model.

Streams already bypass structural reconstruction.

Do not wrap each History unit or stream token in execution scopes.

---

# 15. Root API

Keep the canonical lifecycle boundary introduced by T13.1:

```ts
tui.render(() => new Scene(view, history))
```

This is important even without a compiler.

It gives the runtime ownership of:

```text
root execution scope
transaction lifetime
active scope context
initial child-scope mount
explicit root-props updates
```

The closure is ordinary framework lifecycle API, not a performance opt-in.

Existing direct `render(scene)` may remain as a compatibility/one-shot path if desired, but the canonical recurring API must establish the retained execution root before View-component invocations occur.

Likewise recurring `ViewSlot`/ScrollPane builder forms remain sensible for their own local semantic construction where applicable.

---

# 16. Current Iyon application shape under this API

Iyon remains merely a reference consumer.

Conceptually, current chrome can become:

```ts
const Working = defineView<WorkingProps>(...)
const Approval = defineView<ApprovalProps>(...)
const ComposerChrome = defineView<ComposerProps>(...)
const Footer = defineView<FooterProps>(...)

function App(props: AppProps): View {
  return View.vertical(column => {
    column.child(Working(props.working))
    column.child(Approval(props.approval))
    column.contentMax(MAX_COMPOSER_ROWS, ComposerChrome(props.composer))
    column.child(Footer(props.footer))
  }).fillWidth().fillHeight()
}
```

This is not app-specific infrastructure.

It is ordinary componentized use of the generic framework.

If Footer's tracked status changes locally:

```text
Footer executes
App does not
Working does not
Approval does not
ComposerChrome does not
```

If the App's own structural condition changes:

```text
App executes
unchanged child component bodies skip
```

The app's existing `bodyKey` remains a benchmark control until this behavior is proven, then should become unnecessary.

---

# 17. Exact adaptation of already-implemented T1-T3

This section is normative for the implementation agent.

Do not restart the tranche from scratch.

## 17.1 Step 1 — KEEP

Keep the evidence/probe work.

The existing arms remain useful:

```text
current_body_key
rebuild_uncomposed
manual_stable_oracle
composed/retained candidate
```

Add new execution-scope arms/counters rather than deleting baseline evidence.

The most important new benchmark is:

```text
three independent component scopes
change exactly one scope-local State
```

and prove the other two component bodies plus the parent body do not execute.

## 17.2 Step 2 — ADAPT

Keep:

```text
- transaction generation/epoch machinery
- current vs pending state
- begin / commit / abort discipline
- active synchronous composition context
- touched-slot accounting where still useful
- dispose/memory cleanup infrastructure
- counters
```

Replace compiler-specific/global identity structures:

```text
REMOVE / RETIRE
  CompositionModuleId
  CompositionSiteId
  ModuleSlots
  module registration
  lexical SiteBucket ownership
  source-site occurrence addressing as component identity
```

with:

```text
ADD / REPLACE
  RetainedExecutionRuntime
  RetainedExecutionScope
  parent-local child component reconciliation
  optional keyed-child map
  scope-local semanticSlots
  tracked State dependency sets
  dirty queue / batch epoch
  pending scope mount/unmount state
```

If the T2 implementation already has useful dense arrays/epoch counters, reuse them inside each execution scope rather than deleting performant primitives gratuitously.

## 17.3 Step 3 — KEEP AND REWIRE

Keep:

```text
- raw eager constructors
- monomorphic kind-specific semantic comparators
- exact previous-View return before allocation
- zero-allocation exact-hit fast paths
- safe derivation selection
- validation behavior
- semantic counters
```

Change helper addressing from:

```ts
composeText(moduleId, siteId, value)
```

to a private active-scope form conceptually like:

```ts
composeText(value)
```

where the helper uses:

```text
ACTIVE_EXECUTION_SCOPE
    -> next semantic slot
```

Outside an active execution scope:

```text
fall through immediately to the raw eager constructor
```

Cold ordinary `View.*` construction must preserve the existing <=3% regression gate.

## 17.4 Step 4 partial work — STOP AND SALVAGE ONLY GENERIC TESTS

Do not continue implementing:

```text
- TypeScript AST SiteId transform
- module/site registration injection
- Bun onLoad View-call rewriting
- Oxc dependency solely for this transform
- handwritten scanner/patcher
- transform-specific source-map machinery
```

Remove partial code that exists only for lexical SiteId lowering.

Salvage tests/fixtures that remain valuable as:

```text
- public API coverage fixtures
- fluent-chain semantic tests
- alias/import compatibility tests
- source semantics regression cases
```

but convert them to exercise the public runtime API rather than generated SiteIds.

This partial T4 work was useful because it forced the architecture to distinguish:

```text
execution identity
from
semantic identity
```

before the source transform became entrenched.

---

# 18. New implementation order from the current partial state

Continue T13.1 from the current local branch in this order.

## Step 4R — execution-scope substrate

Implement:

```text
RetainedExecutionRuntime
RetainedExecutionScope
parent/current/pending state
scope-local semantic slot ownership
active-scope nesting
scope disposal
scope counters
```

No native projection yet if separating that makes tests easier.

Prove component-scope identity and props skipping synthetically.

## Step 5R — generic `defineView` component API

Add the canonical public component abstraction.

Required:

```text
- typed props
- parent-local positional identity
- component type check
- local key support
- shallow Object.is props skip
- no global IDs
- no app-specific types
```

## Step 6R — retained scope projection

Give each live component scope an independently replaceable retained sub-DAG projection using:

```text
existing ViewSlot/component primitive
or
private lower-overhead equivalent
```

Prove:

```text
child scope content changes
parent semantic View identity remains exact
```

for a local child update.

## Step 7R — tracked `State<T>` invalidation

Implement the small generic observable primitive and dependency collection.

Prove:

```text
State write
-> exact subscriber scope dirty
-> unrelated scopes not executed
```

## Step 8R — dirty scheduler and batching

Implement one-turn batching / dirty-scope queue.

Prove duplicate invalidations coalesce.

## Step 9R — retained native mount/layout dependency frontier

Before claiming the execution system complete, make the host consume local scope-root changes incrementally.

Implement/adapt:

```text
retained MountGraph subtree patching by changed ComponentId
component-revision driven resolver invalidation
retained layout dependency invalidation
unchanged-constraint measurement cutoffs
reuse of unchanged sibling measurements
paint invalidation no broader than required by layout/visual change
```

Prove that a same-size local leaf update among 1,000 mounted sibling scopes does not rescan/re-resolve/remeasure all 1,000 merely because one component revision changed.

## Step 10R — multi-scope transactional commit

Integrate pending output materialization, atomic component-root batch swaps, retained MountGraph patches, layout/paint invalidation, dependency promotion, mounts/unmounts, and rollback/atomicity.

## Step 11R — root / ViewSlot / ScrollPane canonical boundaries

Wire `tui.render(() => ...)` and recurring local builders to the execution runtime.

## Step 12R — keyed reorder and structural parent changes

Prove keyed child scope identity under:

```text
insert
remove
prepend
middle insert
reorder
conditional mount/unmount
```

## Step 13R — production reference conversion

Convert Iyon only through public generic APIs.

No internal scope imports.

No manual DAG cache.

No compiler wiring.

Keep bodyKey as control switch during evidence collection.

## Step 14R — authoritative benchmark / cleanup

Only after all gates pass:

```text
remove obsolete bodyKey optimization
remove abandoned lexical transform code
remove unused transform dependencies
```

---

# 19. Complexity targets

## 19.1 Scope-local observed-state update

Let:

```text
D = number of execution scopes directly invalidated
S = semantic sites executed inside those dirty scopes
F = new/changed semantic frontier inside those scopes
```

Let additionally:

```text
M = changed component-mount topology inside dirty scopes
L = layout dependency frontier made dirty by changed geometry
P = paint/terminal frontier that must be redrawn or moved
```

Target:

```text
execution work          O(D + S_dirty)
semantic allocation     O(F)
native semantic work    O(F_native)
mount resolution work   O(M + affected mount depth/frontier)
layout work             O(L)
paint/terminal work     O(P)
```

Critically, a local tracked-state update must not acquire unconditional terms:

```text
+ O(total application scopes)
+ O(total application semantic nodes)
+ O(total mounted components)
+ O(total layout nodes)
```

A particular layout may legitimately make `L` or `P` large (for example, changing the height of the first child in a long vertical stack can move a large suffix). That is genuine output work. It is different from re-executing or re-resolving every sibling to discover that only one child changed.

## 19.2 Parent/root props update

If the external application explicitly asks to re-evaluate a parent/root with new opaque props, some work at that parent is necessary.

Target:

```text
parent body executes
immediate child component props checked
unchanged child bodies skipped
changed child bodies execute
```

No descendant tree walk through clean child scopes.

## 19.3 Keyed dynamic list

When the parent list itself changes, enumerating a fresh arbitrary list is proportional to that list's size; this is unavoidable without mutation provenance.

However:

```text
existing keyed child component bodies must not execute merely because they moved
```

Large mutable sequences with known edit operations must continue using PersistentSeq/specialized retained APIs.

---

# 20. Performance gates

The old T13.1 gates remain unless explicitly strengthened here.

## 20.1 Mandatory execution-frontier gate

New canonical test:

```text
App
├── A
├── B
└── C
```

Each child has its own State dependency.

After changing only B's State:

```text
App body executions during update = 0
A body executions                 = 0
B body executions                 = 1
C body executions                 = 0
```

If App/A/C execute to discover they are unchanged, the tranche fails this gate.

## 20.2 Parent-props skip gate

Explicit root/parent update where only B props change:

```text
App body executions = 1
A body executions   = 0
B body executions   = 1
C body executions   = 0
```

## 20.3 Semantic DAG gate

For local B update:

```text
A current output View/NodeIds       unchanged
C current output View/NodeIds       unchanged
parent ScopeRef semantic NodeIds    unchanged
only B scope's changed semantic path obtains new NodeIds
```

## 20.4 Native gate

For local B update:

```text
no root cold decode
no A materialization
no C materialization
no parent semantic materialization
B retained boundary visits changed frontier only
```

## 20.5 1-of-1000 sibling independence — end-to-end frontier proof

Create 1,000 sibling component scopes under a retained parent. Change a tracked State read by exactly one child. Run two variants.

### Variant A — same geometry

The changed child produces different visible content but identical measured geometry/constraints. Require:

```text
dirty child body executions             = 1
clean child body executions             = 0
parent body executions                  = 0
clean semantic nodes allocated          = 0
clean sibling native materializations   = 0
full mount-forest rescans                = 0
clean sibling content remeasurements     = 0
```

The host may perform bounded scheduler/commit bookkeeping, but semantic resolve/layout work must not scale linearly with 1,000 clean sibling scopes.

### Variant B — geometry changes

The changed child's height changes. Require:

```text
component body executions remain exactly 1
semantic/native DAG work remains child frontier only
layout propagates only as geometry dependencies require
unchanged sibling measurements are reused when constraints are unchanged
```

Placement/paint work may scale with the suffix that actually moves. Report that separately; do not conflate it with semantic execution or component resolution.

This is the benchmark that proves the architecture behaves like the retained DAG across phases rather than merely moving a full-tree scan downstream.

## 20.6 Scope projection overhead

Component indirection itself has a cost.

Benchmark trees containing:

```text
10 scopes
100 scopes
1,000 scopes
```

for:

```text
cold mount
exact no-op
local leaf update
full parent structural update
layout/paint
memory
```

If reusing full public `ViewSlot` for every scope is measurably too expensive, keep the execution architecture and implement a slimmer private retained-subtree handle. Do not fall back to global re-execution to avoid fixing the primitive.

## 20.7 Existing PERF-12 wide gates

Still mandatory:

```text
axis set/splice O(log_32 N + inserted)
no O(width) composition scan added to retained wide-edit path
```

---

# 21. Instrumentation

Add counters at the execution layer.

Minimum:

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
execution_scope_commit_aborts
execution_scope_dependency_reads
execution_scope_dependency_subscriptions
execution_scope_dependency_unsubscriptions
keyed_scope_hits
keyed_scope_mounts
keyed_scope_moves
mount_subtrees_resolved
mount_nodes_visited
layout_dependency_invalidations
layout_nodes_remeasured
layout_measure_cache_hits
paint_subtrees_invalidated
```

Existing semantic/native counters remain.

For benchmark output, report together:

```text
execution scopes called
semantic sites visited
semantic nodes allocated
native semantic nodes inspected
native constructors/derivations
host layout/paint
```

Do not hide execution cost inside "construction".

---

# 22. Failure modes and required behavior

## 22.1 Component body throws

Abort pending scope/frame transaction.

Keep committed component roots and dependencies.

## 22.2 Native materialization fails

No pending component projection may become committed.

## 22.3 State changes while scope is executing

Record a later dirty generation and schedule one further pass after the current transaction.

Do not recursively re-enter the same scope.

## 22.4 Child removed while dirty

Parent structural transaction wins.

If child is absent after successful parent commit, discard pending dirty execution and dispose subscriptions/root lease.

## 22.5 Duplicate keys

Throw deterministic framework error before commit.

## 22.6 Mutable prop object changed in place

Default props comparison cannot detect this safely.

Document the semantic contract:

```text
props are immutable snapshots
or
mutable data that should independently trigger updates uses tracked State<T>
```

Do not deep-diff arbitrary mutable objects.

---

# 23. Memory model

A scope strongly retains only its live committed execution state:

```text
current props
current output View
stable projection View
live child scopes
live State dependencies
scope-local semantic slots
root lease/native boundary
```

Removed scopes must be reclaimed after successful commit.

Tests:

```text
mount/unmount 100k keyed scopes over time with bounded live set
subscriber counts follow live scopes
pending aborted scopes reclaimed
State source does not retain disposed subscribers
root close releases all scope roots
```

No FinalizationRegistry as correctness clock.

---

# 24. Public API stability and generic-framework requirement

This remains non-negotiable.

The feature belongs to generic `iyon-tui`.

The Iyon app is not special.

A third-party-style fixture must be able to write only public APIs such as:

```ts
import { defineView, state, View, Tui } from "iyon:tui"

const count = state(0)

const Counter = defineView(() =>
  View.text(String(count.value))
)

const Static = defineView(() =>
  View.text("static")
)

await tui.render(() =>
  new Scene(
    View.horizontal(row => {
      row.child(Counter({}))
      row.child(Static({}))
    })
  )
)
```

Then:

```ts
count.set(1)
```

must update `Counter` without executing `Static` or the parent render body.

No:

```text
compiler plugin setup
NodeId
NativeRef
manual memo cache
application bodyKey
unique ID on every node
DAG manipulation
```

is allowed.

`key` is the only explicit identity primitive needed for repeated/moving logical instances.

---

# 25. Transport independence / PERF-12v2

Everything in this amendment sits above the physical JS/native transport.

A retained execution scope owns a semantic View root and a retained boundary.

Today that boundary may use PERF-12 direct FFI.

PERF-12v2 may later use safe N-API.

The execution architecture must not change.

Preserve:

```text
scope identity
State dependencies
props skipping
dirty scheduler
component projections
immutable View roots
scope-local semantic reuse
transaction semantics
```

Only the physical `ensureNative`/NativeRef transport changes.

---

# 26. Relationship to React Fiber / Compose: what to copy and what not to copy

## Copy from React Fiber

```text
- persistent execution identity separate from ephemeral values
- local position/type/key identity
- dirty work attached to retained execution nodes
- ability to skip clean subtrees/scopes
- current vs pending/transactional state discipline
```

Do not copy:

```text
- DOM-oriented host mutation assumptions
- broad VDOM child reconciliation for semantic View nodes
- lane priority complexity unless Iyon later needs priority scheduling
```

A simple dirty bit/generation is sufficient for T13.1 unless real priority requirements exist.

## Copy from Compose

```text
- restartable execution scopes
- state-read dependency recording
- state writes invalidating the scopes that read them
- skippable component calls when inputs are unchanged
- local key semantics for repeated/moving identity
```

Do not copy:

```text
- Kotlin compiler plumbing
- giant generic SlotTable semantics when simpler JS structures fit
- stability annotation ecosystem
```

JavaScript's safe default is:

```text
Object.is for prop fields
tracked State<T> writes for mutable/reactive values
```

## Copy from Flutter

```text
- immutable output/configuration values
- persistent dirty execution objects
- direct dirty-object scheduling
- identity cutoff when immutable values are reused
```

Iyon's independently retained component sub-DAG is the equivalent host-level primitive that lets a dirty scope update without rebuilding ancestors.

---

# 27. Why this is not a shortcut

A shortcut would be:

```text
rerun whole app
compare every View site
reuse most outputs
```

This amendment explicitly rejects that as the final hot path.

The required local update is:

```text
known state source
    -> known execution scope
    -> one dirty body
    -> one immutable sub-DAG frontier
    -> one retained native sub-root update
```

That is stronger than the lexical-SiteId design that triggered this amendment.

The SiteId design gave the runtime a cheap way to recognize repeated work.

This design gives the runtime a way not to perform clean work in the first place.

---

# 28. Acceptance checklist

T13.1 is not complete until all are true.

## Execution

```text
[ ] tracked State write can target a retained execution scope directly
[ ] clean sibling component bodies do not execute
[ ] clean parent component body does not execute for a child-local State update
[ ] parent props updates skip children with unchanged props
[ ] component mount/unmount/reorder identity is correct
[ ] keys are local and required only for dynamic ambiguity
```

## Semantic DAG

```text
[ ] each scope output remains immutable
[ ] changed semantic node gets new NodeId
[ ] unchanged scope-local semantic nodes reuse exact old Views/NodeIds
[ ] clean component projection Views remain exact identity
[ ] no mutable same-NodeId payloads
[ ] no second semantic payload graph
```

## Native

```text
[ ] each execution scope has an independently retained sub-DAG root
[ ] local scope update does not materialize parent/sibling DAGs
[ ] local component update patches/reuses retained MountGraph state instead of globally rediscovering unchanged mounts
[ ] same-geometry local update does not remeasure clean sibling content
[ ] geometry-changing local update invalidates exactly the required layout dependency frontier
[ ] component revision invalidates layout correctly
[ ] old root lease survives until successful replacement
[ ] multi-scope/root publication is atomic or infallible after complete validation
[ ] NativeRef/transport details remain private
```

## Performance

```text
[ ] 1-of-3 local update executes exactly one child body and zero parent/sibling bodies
[ ] 1-of-1000 local update executes exactly one child body and zero parent/sibling bodies
[ ] 1-of-1000 same-geometry local update has no O(total mounted scopes) resolve/remeasure pass
[ ] geometry-changing update reports semantic/native work separately from unavoidable layout/placement work
[ ] scope-local exact semantic hits allocate zero new View/BridgeViewNode
[ ] cold raw View construction <= 3% credible regression
[ ] component projection overhead measured at 10/100/1000 scopes
[ ] wide PersistentSeq asymptotics unchanged
[ ] real production trace no slower end-to-end in representative updates
```

## Generic public API

```text
[ ] no Iyon-app-specific composition primitive
[ ] no user compiler/plugin configuration
[ ] no global IDs required
[ ] no manual DAG retention required
[ ] external consumer fixture demonstrates direct scoped invalidation
[ ] Iyon app uses only the same public APIs
```

## T4 cleanup

```text
[ ] lexical SiteId transform no longer required by architecture
[ ] partial transform code removed or isolated only if independently useful
[ ] no Oxc/AST dependency added solely for abandoned SiteId lowering
```

---

# 29. Authoritative benchmark scenarios

Run every scenario in isolated benchmark processes where practical.

## A. exact no-op root call

Explicitly call root render with identical inputs.

Expect exact root/scope reuse.

## B. one local State change in 3 siblings

Canonical execution-frontier proof.

## C. one local State change in 1,000 siblings, same measured geometry

Proves no hidden sibling execution, mount-forest rediscovery, or clean-sibling content remeasurement in a scoped update.

## D. one child prop change through explicit parent/root update

Parent executes; unchanged child bodies skip.

## E. structural parent toggle

Parent executes; unaffected mounted child scopes skip; mount/unmount correct.

## F. keyed prepend/reorder

Existing keyed child bodies remain unexecuted when their props/dependencies are unchanged.

## G. child output size change

Proves component revision/layout propagation while retaining unchanged sibling measurements and avoiding semantic sibling execution.

## H. multi-scope batch

Several independent State writes -> one frame transaction.

## I. semantic no-op invalidation

Scope executes but emits exact previous View -> zero native work.

## J. B3/B4/History interaction

Ensure generic execution scopes do not regress specialized retained boundaries.

## K. wide axis retained operations

Existing PERF-12 wide matrix unchanged.

---

# 30. Concrete production proof for current Iyon chrome

Current production chrome is approximately:

```text
vertical
├── working
├── approval
├── composer
└── footer
```

Create retained execution scopes at those natural semantic boundaries.

Required test cases:

### Footer-only status update

```text
Footer body                 1
App body                    0 when update originates in Footer-tracked State
Working body                0
Approval body               0
Composer body               0
outer vertical NodeId       SAME
Footer ScopeRef NodeId      SAME
Footer content root NodeId  changed only as required
parent/sibling mount topology  reused
clean sibling semantic bodies  not executed
clean sibling native DAGs      not materialized
layout remeasurement           only if Footer geometry/layout facts require it
```

### Reasoning-effort/composer style update

Only the scope that owns the relevant semantic style state executes.

### Working visibility structural update

The scope that owns the conditional structure executes.

Stable sibling component bodies remain skipped.

### Tool card streaming

Keep existing ViewSlot/ScrollPane specialized update path.

Do not funnel stream deltas through the app execution graph.

---

# 31. Final instruction to the implementation agent

Implement T13.1 from the current partial local state as an **incremental retained execution system over independently retained immutable View DAG roots, with retained host-side component/layout dependency frontiers**.

The success criterion is not merely that repeated full rendering allocates fewer Views, and it is not merely that only one JavaScript component body executes. The complete pipeline must possess enough retained identity and update provenance that unchanged work is not rediscovered at the next layer.

Use a generic public `defineView`-style component abstraction to create persistent component instances. A mounted component scope has parent-local type/position/key identity and a stable framework-owned projection into its parent. Its actual output is an independent immutable semantic DAG root with its own retained root lease. A local child update therefore replaces only that scope root while the parent's semantic DAG and clean sibling projection Views remain exact identity.

Add a minimal generic tracked `State<T>` primitive. Reads during a pure synchronous component evaluation subscribe the current scope; writes outside evaluation invalidate exactly the live scopes that read the value and enqueue them once. Parent-driven props remain the second update channel: when a parent genuinely executes, shallow `Object.is` field comparison skips unchanged child component bodies. Use local keys only for repeated/movable component identity. Do not require application-global IDs.

Adapt the already-implemented T2 runtime/transaction machinery into the retained execution runtime. Keep T3 monomorphic semantic comparators/raw constructors, but make their memo slots scope-local so semantic-site replay occurs only inside a scope that was already invalidated. Stop partial T4 lexical-SiteId compiler work. React Fiber and Compose research does not justify an Oxc/source-transform dependency for the core guarantee: React can target retained Fibers without its compiler, while Compose needs compiler restart groups because its public composable boundary is otherwise an ordinary Kotlin call. Iyon's explicit generic component API can provide the restart boundary directly.

Do not stop after the JS execution frontier. Current repository code already retains component snapshots/revisions and a `MountGraph`, but current resolution/layout paths can still revisit broad component-bearing structure. Make local scope-root commits update retained component mount topology and layout dependency state incrementally. If a child output keeps the same measured geometry, clean parents/siblings must not be re-resolved or remeasured merely because one component revision changed. If geometry changes, propagate layout dirtiness only along the real dependency frontier, reuse unchanged sibling measurements under unchanged constraints, and allow only genuinely necessary placement/paint work to scale with the affected region.

Prepare all dirty-scope outputs, NativeRefs, mount patches and dependency changes without mutating committed host state. Publish them in one authoritative batch commit (or prove the final publication operations are infallible after complete validation). A failure may not leave a mixture of old/new component roots, MountGraph state, subscriptions or Scene root.

Preserve the immutable `View -> BridgeViewNode` DAG, NodeId semantics, BridgeNativeHint/NativeRef separation, RetainedRootBoundary, PersistentSeq, History, streams, ScrollPane and every existing PERF-12 retained transport invariant. Do not introduce a second semantic payload graph. Do not make semantic nodes mutable. Do not deep-diff props, View trees, or arbitrary application state.

The decisive evidence is two 1-of-1000 sibling tests. In the same-geometry variant, changing state read by one child must execute exactly that child scope, allocate/materialize only its changed semantic frontier, avoid full mount-forest rescans, and avoid clean sibling content remeasurement. In the geometry-changing variant, component execution/native DAG work must remain just as narrow while layout/paint expand only as required by the geometry dependency. If the implementation executes, reconstructs, resolves, or remeasures all clean scopes to discover the one changed branch, T13.1 has failed its architectural objective.

The resulting system is generic `iyon-tui` infrastructure. The Iyon app is only a reference consumer and may be migrated to the same canonical public component/state APIs any external application uses. It must never own or configure the mechanism.

---

# 32. Research and repository evidence used for this amendment

The architecture above was informed by current primary/official sources and the actual `f665c9e` repository implementation.

## 32.1 React

- React documentation, **Render and Commit**: a state update targets the component that owns that state; child rendering proceeds recursively from there rather than requiring the application root to execute first.
- React `memo` and React Compiler documentation: unchanged child component work can be skipped; React Compiler adds automatic memoization but is separate from the basic retained Fiber/state scheduling model.
- React reconciler architecture/source: persistent Fibers carry pending/child work and support bailouts; child identity is parent-local type/position/key, with keyed reconciliation for dynamic movement.

Use from React: persistent execution nodes, direct update provenance, parent-local identity, WIP/commit separation. Do not import DOM-oriented VDOM diffing or lane complexity without a demonstrated scheduling need.

## 32.2 Jetpack Compose

- Android Developers, **Lifecycle of composables**: eligible composables may be skipped when inputs are unchanged.
- Android Developers, **Jetpack Compose phases**: snapshot-state reads are tracked in the phase/scope that reads them; writes schedule the readers that depend on the value.
- `SnapshotMutableState` / `SnapshotStateObserver` API documentation: reads subscribe the current recomposition scope and changed writes invalidate associated scopes.

Use from Compose: restartable retained scopes, read tracking, exact invalidation provenance, skippable unchanged component inputs. Do not copy compiler plumbing merely because Compose needs it to make ordinary Kotlin function syntax restartable.

## 32.3 Flutter

- Flutter documentation, **Inside Flutter**: immutable Widgets are retained by a persistent Element tree; dirty Elements are scheduled directly and clean Elements are skipped.
- The same document's **Sublinear layout** section: clean render objects under unchanged constraints cut off layout work and only a limited region around dirty nodes is revisited.
- Its reconciliation discussion confirms ordinary local type/key identity and keyed matching for dynamic children.

Use from Flutter: separate immutable description values from persistent execution/layout state, plus phase-local dirty propagation rather than whole-tree rediscovery.

## 32.4 Iyon source audit at `f665c9ef913a7a8eda552a385b072f25f853b359`

The current repository already provides critical pieces:

```text
packages/iyon-runtime/src/tui/component.ts
  ViewSlot owns a RetainedRootBoundary and stable native component identity
  independent root replacement uses setViewRef after retained materialization

crates/iyon-tui/src/component/slot.rs
  View::component is a stable ComponentSlot(ComponentId) semantic node

crates/iyon-tui/src/component/registry.rs
  persistent component entries carry revision + cached immutable View snapshot
  mutation/invalidation increments revision and invalidates only that snapshot

crates/iyon-tui/src/scene/resolve.rs
  component snapshots and MountGraph data already exist
  current resolver still scans component-bearing semantic branches

crates/iyon-tui/src/presentation/layout/measure.rs
  ComponentSlot measurement keys incorporate resolved component output View identity
  current cache policy intentionally avoids caching important component-containing parents

crates/iyon-tui/src/scene/host.rs
  SceneHost already retains MountGraph, LayoutCache and PaintCache across frames
```

This source audit is why Amendment C requires extending the existing retained component/mount/layout machinery instead of inventing a parallel framework. It also identifies the concrete downstream O(N) risk that the acceptance tests must eliminate.

---

# 33. Review Addendum — implementation-hardening invariants (appended; normative wherever implementation ambiguity exists)

This addendum amends Amendment C without changing the core architecture. It makes several invariants explicit to prevent accidental simplification during implementation.

The authoritative architecture remains:

> persistent retained execution scopes + independently retained immutable View DAG roots + tracked invalidation + incremental host frontiers.

Amendment C remains the architectural source of truth. This addendum only prevents accidental weakening during implementation.

## 33.1 Retained execution scopes are not a second reconciler

The execution graph MUST NOT become a second semantic reconciliation system.

Ownership boundaries:

```text
RetainedExecutionScope
    answers: "what code needs to execute?"

Immutable View DAG
    answers: "what semantic UI value exists?"

RetainedRootBoundary / Native retained graph
    answers: "what native representation exists?"

Layout dependency graph
    answers: "what geometry is still valid?"

Paint cache
    answers: "what pixels need repainting?"
```

No layer may rediscover another layer by scanning the whole structure.

A local state update must not become:

```text
dirty scope
    -> execute one scope
    -> scan all scopes
    -> scan all semantic nodes
    -> rediscover unchanged layout
```

That would merely move the O(N) work.

## 33.2 Execution scope, semantic identity, and native identity remain separate

Implementation MUST preserve:

```text
ExecutionScope identity != View/BridgeViewNode NodeId != NativeRef
```

A component instance can survive while its immutable semantic output changes:

```text
Footer scope, output NodeId 100, text "Working"
Footer scope, output NodeId 101, text "Done"     # same scope instance
```

The scope is the continuity boundary. The NodeId is the immutable semantic value. Do not encode execution scopes into semantic NodeIds.

## 33.3 `State<T>` must remain minimal

The `State<T>` primitive exists only as an invalidation source.

Required:

```text
State<T>
    value getter
    set()
    update()
    subscriber tracking
```

Not required:

```text
computed values
effects
derived signal graphs
global stores
automatic mutation tracking
implicit proxies
deep observation
```

T13.1 must not become a general reactive framework. The framework only needs enough information to answer: "which retained execution scopes must run?"

## 33.4 Host-side incremental behavior is part of T13.1 correctness

Passing JavaScript execution counters alone is insufficient. The following are separate acceptance requirements:

Semantic frontier — one changed scope:

```text
changed scope semantic nodes: allowed to change
clean scopes:                 zero semantic work
```

Native frontier — one changed scope:

```text
changed scope: materialize changed native frontier
clean scopes:  zero materialization
```

Mount frontier — one changed scope:

```text
changed component subtree: patch only affected mount topology
clean mounts:              no global mount graph scan
```

Layout frontier:

```text
content update with unchanged geometry:
    changed content -> repaint/update affected region
    unchanged sibling measurements reused

geometry-changing update:
    changed geometry -> propagate only through real layout dependencies
```

The existence of necessary geometry propagation does not justify rebuilding unrelated semantic scopes.

## 33.5 ViewSlot/component primitive guidance

Existing ViewSlot/component infrastructure is a foundation, not a requirement to allocate a public ViewSlot for every component.

The implementation must benchmark 10 / 100 / 1,000 scopes.

If public ViewSlot machinery is too expensive — allowed: private `RetainedSubtreeSlot` sharing the same retained-root principles. Not allowed: removing retained scopes and returning to global replay.

## 33.6 Props and user ergonomics

Default props skipping (`Object.is(old, new)`) is correct. Do not add deep comparison.

Documentation must make immutable props the normal contract. Examples should avoid fresh object literals when users expect skipping:

```ts
Component({ style: { color: "red" } })   // avoid: identity changes every call
```

Prefer stable values:

```ts
const footerStyle = ...
Footer({ style: footerStyle })
```

or `State<T>` for independently changing data.

## 33.7 Tranche decomposition clarification

The architecture remains one T13.1 end state, but implementation should internally respect two risk domains.

**T13.1A** — Retained execution: `defineView`, execution scopes, `State<T>`, dirty scheduler, scope-local semantic construction, immutable DAG root ownership.

**T13.1B** — Retained host frontier: mount graph incremental updates, layout dependency propagation, paint invalidation, component measurement reuse.

T13.1 is not complete until both are proven.

## 33.8 Final invariant

The final system must satisfy:

```text
one local state write
    -> one known retained execution scope
    -> one scope body execution
    -> one immutable semantic frontier
    -> one retained native/layout/paint frontier
```

The implementation fails if it instead performs:

```text
one local state write
    -> execute everything
    or
    scan everything to discover nothing changed
```
