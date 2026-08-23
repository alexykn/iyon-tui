# PERF-12 Tranche 13.1 — Retained View Composition

## Automatic structural sharing for every supported `iyon-tui` consumer without exposing the internal DAG or compiler

**Status:** implementation handoff  
**Repository:** `alexykn/iyon`  
**Target branch:** `perf-refactor`  
**Source freeze:** `f665c9e3d2d69378caade3ee058a7ae1ed421d07` (`docs(tui): complete PERF-12 T13 production review`)  
**Parent tranche:** PERF-12 T13 production boundary integration/review  
**Architecture being extended:** PERF-12 Retained DAG Direct FFI  
**Transport dependency:** **none** — T13.1 is deliberately above the native transport and must survive a future PERF-12v2 N-API transport unchanged.  
**Framework ownership invariant:** retained composition and its source transform are mandatory, invisible `iyon-tui` infrastructure for every supported consumer path; no application installs or opts into them.

---

# 0. Executive decision

PERF-12 currently has a strong retained semantic DAG **after stable JavaScript View identity already exists**.

That is not yet enough for the public framework.

Idiomatic application code does this:

```ts
function createIyonView(state: State): View {
  return View.vertical(column => {
    column.push(View.text(state.status))
    column.push(View.component(composer).style(theme.composer))
    column.push(View.text(state.footer).style(theme.footer))
  }).fillWidth().fillHeight()
}
```

and then calls it again after state changes.

Without another framework layer, every call produces fresh `View` objects, fresh `BridgeViewNode`s, and fresh semantic `NodeId`s even when 95% of the UI is semantically unchanged. PERF-12 can only stop at an identity frontier if the caller happened to preserve those identities manually.

That is the wrong public contract.

**Users of `iyon-tui` must not need to know about the immutable DAG, NodeIds, NativeRefs, BridgeNativeHints, native leases, or manual View memoization to receive retained-rendering benefits.**

T13.1 adds a framework-owned **Retained View Composition** layer between declarative View construction and the existing immutable semantic DAG.

The final conceptual pipeline becomes:

```text
application state
      |
      v
ordinary declarative View-building code
      |
      v
+--------------------------------------------------+
| Retained View Composition                        |
|                                                  |
| compiler-known lexical site identity             |
| + occurrence identity                            |
| + explicit local keys only where needed          |
| + previous immutable View per logical slot       |
| + generated shallow semantic reuse checks        |
+--------------------------------------------------+
      |
      v
structurally-shared immutable BridgeViewNode DAG
      |
      v
PERF-12 retained_dag.ts
      |
      +-- exact root ----------------------> O(1)
      +-- stable subtree ------------------> O(1) cutoff
      +-- changed semantic frontier -------> derivation/materialization
      +-- wide retained mutation ----------> PersistentSeq O(log_32 N)
      |
      v
retained Rust View DAG
      |
      v
layout / paint / host
```

The core rule is:

> **Applications describe successive UI values. `iyon-tui` owns continuity between those values.**

T13.1 must make the following true for normal supported `iyon-tui` applications, including external library consumers:

```text
same logical construction site
+ same immediate semantic inputs
=
return the exact previously-committed View object
```

Therefore:

```text
same View object
-> same BridgeViewNode object
-> same NodeId
-> same BridgeNativeHint when available
-> same NativeRef
-> existing PERF-12 identity cutoff fires naturally
```

When semantics change:

```text
same logical composition identity
+ changed semantic inputs
=
new immutable View / BridgeViewNode / NodeId
+ known predecessor relationship
```

The existing PERF-12 derivation and retained edit machinery can then update only the true changed frontier.

This is **not** a VDOM reconciler.

This is **not** content hashing.

This is **not** a second semantic DAG.

This is **not** app-layer memoization.

This is a small retained composition index whose only job is to connect declarative construction sites across successful renders.

---

# 1. Why T13.1 exists

## 1.1 The gap in the original PERF-12 handoff

The PERF-12 handoff correctly requires:

```text
stable JS identity for unchanged semantic subtrees
semantic identity cutoff before payload/child inspection
semantic JS construction O(changed semantic nodes)
NativeRef cutoff O(changed frontier)
```

It also correctly defines:

```text
new semantic node -> new NodeId
unchanged shared semantic node -> same immutable BridgeViewNode -> same NodeId
```

What it did **not** fully specify is how ordinary application code that re-evaluates a render function from new state creates that structural sharing automatically.

The handoff therefore solved:

```text
shared immutable DAG
        ->
retained native update
```

but left this transition implicit:

```text
new application state
        ->
shared immutable DAG
```

T13.1 owns that missing transition.

## 1.2 The production trace proves the gap is real

At the source freeze, production `createIyonView()` rebuilds the small chrome tree each time from reduced state. The application then computes a `bodyKey` string and avoids `Tui.render()` entirely when that string has not changed.

This is effective as a temporary application optimization, but it is the wrong abstraction boundary:

```text
application code
  currently knows enough about rendering identity
  to build a cache key
```

The framework should own that.

The new retained root boundary in `Tui.render` is already correct **once it receives a shared View DAG**. Likewise `ViewSlot` and `ScrollPane` now own current-root leases. T13.1 must sit above those boundaries and feed them structurally shared Views.

## 1.3 Do not “fix” this by memoizing `createIyonView()` in the app

A T13 review implementation such as:

```ts
let previousBodyKey: string | undefined
let previousBody: View | undefined

function createIyonViewMemoized(state: State): View {
  const key = ...
  if (key === previousBodyKey) return previousBody!
  ...
}
```

is explicitly rejected as the architectural solution.

It may exist temporarily as a benchmark control/oracle, but not as the final production mechanism.

Reasons:

1. It leaks renderer identity discipline into every application.
2. It makes public performance depend on undocumented object-reuse behavior.
3. Every application would invent different cache keys and invalidation rules.
4. It scales poorly to nested changing subtrees.
5. It cannot naturally express structural sharing along only the changed path.
6. It makes framework optimization opportunities inaccessible to generic consumers.
7. It directly contradicts the goal of a declarative public API.

---

# 2. Source freeze — what already exists at `f665c9e`

T13.1 must build on the actual current code, not re-create a hypothetical architecture.

## 2.1 Eager immutable semantic DAG is already restored

Current:

`packages/iyon-runtime/src/tui/values/view.ts`

has the historical-style shape:

```text
View
  -> WeakMap<View, BridgeViewNode>
  -> BridgeViewNode eagerly created
  -> private NodeId assigned
  -> immutable/frozen View
```

`nodeForBridge(view)` is a lookup, not a serialization pass.

The file also already owns semantic derivation sidecars for retained optimizations including:

```text
text-layout
common-scalar
axis-set
axis-splice
grid-cell
```

and wide-child/grid override sidecars.

T13.1 must reuse this semantic representation exactly.

## 2.2 `retained_dag.ts` already solves the native frontier

Current:

`packages/iyon-runtime/src/tui/retained_dag.ts`

already contains:

```text
RetainedRootBoundary
ensureNativeRoot
BridgeNativeHint
NodeId -> NativeRef promotion
nativeLookupCeiling
identity-before-payload cutoff
transaction-local temporary leases
stale-ref recovery
cold/frontier caps
complete fallback
monomorphic materializers
PersistentSeq-aware derivation paths
```

T13.1 must **not** create another native transport, another native cache, another NativeRef table, or another semantic authority.

## 2.3 `Tui.render` already owns a retained root boundary

Current:

`packages/iyon-runtime/src/tui/runtime.ts`

has one `RetainedRootBoundary` for the scene body and already:

```text
checks exact object identity
tries retained root installation
routes cold graphs through the current cold builder
adopts the successful native root
falls back to complete N-API decode when necessary
```

T13.1 adds composition *before* this logic.

## 2.4 `ViewSlot` and `ScrollPane` already own retained root boundaries

Current:

```text
packages/iyon-runtime/src/tui/component.ts
packages/iyon-runtime/src/tui/scroll-pane.ts
```

already keep their current View and retained root boundary.

These are natural owners of independent composition roots.

## 2.5 The application still carries `bodyKey`

Current:

`plugins/app/iyon/src/app.ts`

still contains `lastRenderedBodyKey` / scene-body key logic because ordinary `createIyonView()` calls create fresh View values.

The `bodyKey` must remain during the initial T13.1 proof as a control. It is removed **only after** automatic composition proves semantic parity and performance.

## 2.6 Current build infrastructure already supports a source transform

Current CLI build:

```ts
Bun.build({
  ...,
  plugins: [iyonVirtualModulePlugin],
})
```

and `iyonVirtualModulePlugin` already uses Bun `onResolve` / `onLoad` hooks.

Therefore T13.1 does not need a new compiler toolchain.

Add one small Iyon source-transform plugin to the existing Bun build/plugin infrastructure.

---

# 3. Research result — what mature frameworks teach us

T13.1 should borrow ideas, not copy implementations wholesale.

## 3.1 React — logical continuity must not depend on element object identity

React associates persistent component state with the component's position/type in the render tree; explicit `key` changes or refines identity for dynamic children.

Important lesson:

```text
fresh declarative values are allowed
persistent logical identity lives elsewhere
```

React also makes keys local to the relevant parent/list rather than requiring globally unique application IDs.

React Compiler further demonstrates that manual memoization can be moved into build-time/runtime machinery instead of forcing users to write `useMemo` everywhere.

T13.1 adopts those usability lessons.

T13.1 does **not** adopt React's general runtime VDOM reconciliation pass.

## 3.2 Jetpack Compose — closest architectural precedent

Compose is the strongest precedent for T13.1:

```text
lexical call site identifies a composition instance
same call site repeated -> occurrence order by default
key(...) -> explicit identity for dynamic/reordered instances
stable unchanged inputs -> skip/reuse
retained composition table -> continuity across recomposition
```

This maps directly to Iyon's problem.

T13.1 therefore uses:

```text
compiler-assigned source site
+ occurrence index
+ optional explicit local key
```

as logical composition identity.

Unlike Compose, T13.1 initially does not implement a general reactive dependency graph or arbitrary function-level restart scopes. It only needs enough composition machinery to produce structurally shared immutable `View` snapshots.

## 3.3 SwiftUI — value is not identity

SwiftUI explicitly separates:

```text
ephemeral View value
persistent identity/lifetime
```

and uses structural identity by default plus explicit data identity where necessary.

That distinction is essential for Iyon:

```text
CompositionIdentity != NodeId != NativeRef
```

## 3.4 Flutter — immutable declarations over persistent retained state

Flutter demonstrates that immutable UI descriptions and persistent runtime identity are complementary, not contradictory.

Its dirty-element model also reinforces the principle that unchanged retained work should be skipped rather than re-derived from scratch.

T13.1 keeps Iyon's stronger immutable DAG cutoff instead of copying the Element tree.

## 3.5 Vue — compiler information can shrink runtime work

Vue's compiler-generated patch metadata demonstrates a useful principle:

```text
if source structure already tells the compiler where dynamic work can occur,
do not force the runtime to rediscover that structure by recursively diffing trees.
```

T13.1's source SiteIds follow that principle.

## 3.6 Solid — useful lesson, wrong semantic representation for this tranche

Solid's fine-grained reactive graph shows how powerful direct dependency invalidation can be.

Do **not** embed reactive cells into Iyon's semantic View DAG in T13.1.

The immutable snapshot DAG is already a proven asset.

A future reactive scheduler may decide *which composition root/scope to reevaluate*, but each successful reevaluation must still produce an immutable semantic snapshot.

---

# 4. Non-negotiable invariants

T13.1 is accepted only if all of these remain true.

## 4.1 Public abstraction and framework ownership

The retained-composition compiler is an implementation detail of `iyon-tui`, in the same category as its native bridge or generated bindings. A supported consumer must never have to discover, import, install, register, configure, feature-flag, or version-match it manually.

```text
[ ] every supported iyon-tui build/run path activates retained composition automatically
[ ] no public composition/compiler plugin is required from consumers
[ ] no application-level enableRetainedComposition()/feature flag exists
[ ] no application-specific build configuration is required solely for retained composition
[ ] the Iyon app is only a normal consumer/reference fixture, never the owner of the mechanism
[ ] users do not see NodeId
[ ] users do not see NativeRef
[ ] users do not see BridgeViewNode
[ ] users do not manage BridgeNativeHint
[ ] users do not manually retain View objects for performance
[ ] users do not write application-specific semantic cache keys for ordinary rendering
[ ] explicit keys are required only for genuinely ambiguous repeated/dynamic identity
```

## 4.2 Semantic DAG

```text
[ ] View remains immutable
[ ] BridgeViewNode remains immutable
[ ] NodeId remains exact semantic object identity
[ ] a changed semantic node gets a new NodeId
[ ] an unchanged composed semantic node reuses the exact old View/BridgeViewNode/NodeId
[ ] no mutable "same NodeId, new payload" state exists
[ ] no second semantic graph is introduced
```

## 4.3 PERF-12 retained bridge

```text
[ ] identity cutoff still happens before native payload/child work
[ ] BridgeNativeHint remains transport acceleration only
[ ] NativeRef remains runtime-local acceleration only
[ ] NativeRef leases remain owned by existing retained boundaries
[ ] retained_dag.ts remains authoritative for native materialization
[ ] cold fallback remains complete
[ ] PersistentSeq wide paths stay logarithmic
[ ] streams remain outside the structural View bridge
```

## 4.4 Composition

```text
[ ] composition identity is private framework metadata
[ ] composition identity is not NodeId
[ ] composition identity is not NativeRef
[ ] no full-tree recursive equality pass
[ ] no content-addressed subtree hashing
[ ] no global user-visible key registry
[ ] no strong cache of all historical View values
[ ] failed host/native installation cannot commit composition state
[ ] a successful no-op composition may return the exact committed root View
```

## 4.5 Transport independence

T13.1 must not depend on:

```text
extern "C"
raw pointers
borrowed FFI buffers
Bun FFI status encoding
NativeRef physical table layout
```

It may consume the existing `RetainedRootBoundary` API, but the composition model must remain valid if PERF-12v2 later replaces the transport with safe N-API calls.

---

# 5. Explicit non-goals

Do not expand this tranche into:

```text
- general React-style virtual DOM reconciliation
- a signal/reactivity system
- a state management library
- a hooks system
- a component-local state framework
- a scheduler rewrite
- compiler HIR/dataflow analysis comparable to React Compiler
- content hashing / structural interning
- persistent wire/mirror records
- native changes unrelated to exposing existing retained operations
- automatic optimization of arbitrary JavaScript calculations
```

T13.1 has one job:

> **Turn repeated ordinary declarative View construction into structurally shared immutable View DAG snapshots automatically.**

---

# 6. Identity model

The implementation must make three identities explicit in design and naming.

## 6.1 Composition identity — logical continuity

Composition identity answers:

```text
"Which logical construction site from the previous successful render corresponds
 to this construction now?"
```

It is scoped to one composition root.

Conceptually:

```text
CompositionAddress =
    CompositionRoot
    + lexical SiteId
    + occurrence OR explicit Key
    + active keyed-group ancestry
```

It survives semantic changes.

Example:

```text
logical footer
render 1: text="Working"
render 2: text="Done"

same CompositionAddress
```

## 6.2 NodeId — immutable semantic identity

NodeId continues to mean:

```text
"this exact immutable BridgeViewNode object"
```

Example:

```text
footer "Working" -> NodeId 8127
footer "Done"    -> NodeId 8134
```

Different payload, therefore different immutable semantic identity.

## 6.3 NativeRef — environment-local native acceleration

NativeRef continues to mean:

```text
"fast handle that may resolve this NodeId's retained Rust View in this runtime generation"
```

Composition must not make NativeRef semantic.

## 6.4 Example over time

```text
render #1

CompositionAddress footer = {module 3, site 8, occurrence 0}
NodeId                    = 8127
NativeRef                 = 39
semantic text             = "Working"

render #2

CompositionAddress footer = {module 3, site 8, occurrence 0}   SAME
NodeId                    = 8134                               NEW
NativeRef                 = 44                                 NEW
semantic text             = "Done"

render #3

CompositionAddress footer = {module 3, site 8, occurrence 0}   SAME
NodeId                    = 8134                               SAME
NativeRef                 = 44                                 SAME
semantic text             = "Done"                            SAME
```

That is the model.

---

# 7. Architecture layers

T13.1 must retain clear ownership boundaries.

```text
+---------------------------------------------------------+
| Application                                             |
| State, actions, ordinary View-building functions        |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| Build-time composition transform                        |
| - assigns lexical SiteIds                               |
| - lowers recognized View factories/modifier chains     |
| - injects tiny private composition helpers              |
| - changes no user-visible semantics                     |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| ViewCompositionRoot / ViewCompositionPass               |
| - current slots                                         |
| - pending slots                                         |
| - occurrence cursors                                    |
| - keyed groups                                          |
| - commit/abort                                          |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| View semantic constructors                              |
| - compare immediate new semantic inputs against old     |
| - return previous View on exact semantic match          |
| - otherwise create new immutable View + derivation hint |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| Existing immutable BridgeViewNode DAG                   |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| Existing retained_dag.ts                                |
+---------------------------------------------------------+
                         |
                         v
+---------------------------------------------------------+
| Existing NativeViewRuntime / Rust View DAG              |
+---------------------------------------------------------+
```

No layer above `retained_dag.ts` may know how a NativeRef table is physically implemented.

---

# 8. Runtime composition data structures

Names may change to fit local conventions, but responsibilities may not.

Baseline conceptual types:

```ts
type CompositionModuleId = number
type CompositionSiteId = number
export type ViewKey = string | number

interface CompositionSlot {
  current: View | undefined
  pending: View | undefined
  seenEpoch: number
}

interface SiteBucket {
  epoch: number
  occurrenceCursor: number
  positional: CompositionSlot[]
  keyed: Map<ViewKey, CompositionSlot> | undefined
}

interface ModuleSlots {
  readonly siteCount: number
  readonly sites: Array<SiteBucket | undefined>
}

class ViewCompositionRoot {
  private modules: Array<ModuleSlots | undefined>
  private committedEpoch: number
  private activePass: ViewCompositionPass | undefined

  begin(): ViewCompositionPass
  commit(pass: ViewCompositionPass): void
  abort(pass: ViewCompositionPass): void
  dispose(): void
}

class ViewCompositionPass {
  readonly root: ViewCompositionRoot
  readonly baseEpoch: number
  readonly nextEpoch: number
  readonly touchedSlots: CompositionSlot[]
  readonly touchedBuckets: SiteBucket[]
  readonly groupStack: CompositionGroup[]
  state: "building" | "prepared" | "committed" | "aborted"
}
```

This is conceptual, not a mandate to allocate exactly these classes.

Hot-path implementation should minimize objects:

```text
prefer arrays and small structs
avoid generic Map lookups for ordinary unkeyed sites
only keyed repeated groups require a Map
avoid per-call descriptor allocation
avoid rest arrays
avoid closure allocation inside generated semantic helpers
```

---

# 9. Dense source SiteIds — no runtime hashing

## 9.1 Do not derive hot identity from source strings

Reject:

```ts
"plugins/app/iyon/src/view.ts:81:17"
```

as a hot Map key.

Reject runtime hashing of source location.

Reject stack inspection.

## 9.2 Register each transformed module once

The source transform injects a module registration once at module initialization:

```ts
import {
  __iyonRegisterCompositionModule,
  __iyonCompose,
} from "@iyon/runtime/tui/internal-composition"

const __iyonModule = __iyonRegisterCompositionModule(17)
```

`17` is the number of transformed View semantic sites in that module.

Registration returns a dense process-local integer:

```text
module 0
module 1
module 2
...
```

No stable cross-build module ID is required.

A composition root lives only inside one running program image.

## 9.3 Site IDs are dense per module

Within the transformed source module:

```text
site 0
site 1
site 2
...
```

are assigned deterministically by AST/source order.

Hot lookup becomes:

```ts
const moduleSlots = root.modules[moduleId]
const bucket = moduleSlots.sites[localSiteId]
```

rather than:

```ts
Map<string, Slot>.get(hash(sourcePath + line + column))
```

## 9.4 HMR/reload semantics

Site IDs need not survive recompilation.

A source/module reload creates a new composition module registration. Old composition metadata is either:

```text
- dropped with the old root/runtime,
- reset on explicit HMR generation change,
- or reclaimed by bounded stale-module maintenance.
```

Do not preserve logical composition identity across arbitrary code replacement in T13.1.

---

# 10. Site occurrence algorithm

A lexical site can execute more than once in one render:

```ts
for (const item of items) {
  View.text(item.label)
}
```

Without an explicit key, identity is:

```text
SiteId + occurrence index
```

matching the useful Compose default.

Conceptual:

```ts
function nextPositionalSlot(
  pass: ViewCompositionPass,
  moduleId: number,
  siteId: number,
): CompositionSlot {
  const bucket = getOrCreateBucket(pass.root, moduleId, siteId)

  if (bucket.epoch !== pass.nextEpoch) {
    bucket.epoch = pass.nextEpoch
    bucket.occurrenceCursor = 0
    pass.touchedBuckets.push(bucket)
  }

  const occurrence = bucket.occurrenceCursor++
  return bucket.positional[occurrence] ??= newSlot()
}
```

The exact implementation must ensure:

```text
one normal array lookup per common site
one cursor increment
no string key
no source hash
no tree search
```

---

# 11. Explicit key algorithm

## 11.1 Public API

Add one small public composition primitive:

```ts
export type ViewKey = string | number

export class View {
  static key(key: ViewKey, build: () => View): View
}
```

Example:

```ts
const rows = tools.map(tool =>
  View.key(tool.id, () => toolView(tool)),
)
```

or inside a builder:

```ts
View.vertical(column => {
  for (const tool of tools) {
    column.push(
      View.key(tool.id, () => toolView(tool)),
    )
  }
})
```

## 11.2 Key scope

A key is **not global**.

It is unique only within:

```text
current composition scope
+ lexical View.key call site
```

This matches React/Compose-style local identity and avoids application-global registries.

## 11.3 Key does not imply semantic equality

This is critical.

Wrong:

```text
same key -> return same View without checking new semantics
```

Correct:

```text
same key
-> locate logical predecessor slot
-> compare immediate semantic inputs at each View site
-> unchanged -> exact View reuse
-> changed -> new immutable semantic node
```

`View.key("footer", ...)` means:

```text
"this is the same logical footer instance"
```

not:

```text
"the footer's value can never change"
```

## 11.4 Duplicate keys

Two executions of the same keyed site with the same key in one composition pass are an error.

Do not silently alias them.

Use a deterministic framework error such as:

```text
TUI_COMPOSITION_DUPLICATE_KEY
```

Include enough debug metadata to locate the transformed module/site in non-minified development builds.

## 11.5 Key changes reset logical lifetime

If:

```ts
View.key("a", () => ...)
```

becomes:

```ts
View.key("b", () => ...)
```

that is a new logical composition instance.

Do not derive from the old keyed instance automatically.

## 11.6 A key owns a nested composition scope

Do not implement `View.key` as merely another discriminator on every leaf slot.

The keyed invocation itself owns a small child `CompositionScope`.

Conceptually:

```ts
interface KeyedGroup {
  readonly key: ViewKey
  readonly scope: CompositionScope
  seenEpoch: number
}

interface KeySiteBucket {
  groups: Map<ViewKey, KeyedGroup>
}
```

A transformed:

```ts
View.key(tool.id, () => toolView(tool))
```

behaves conceptually as:

```ts
function composeKey(
  moduleId: number,
  siteId: number,
  key: ViewKey,
  build: () => View,
): View {
  const parent = activeCompositionScope()
  const keySite = getOrCreateKeySite(parent, moduleId, siteId)

  if (keySite.seenThisPass(key)) {
    throw duplicateCompositionKey(...)
  }

  const group = keySite.groups.get(key) ?? createKeyedGroup(key)
  markVisited(group)

  return withCompositionScope(group.scope, build)
}
```

All transformed View sites inside `toolView(tool)` therefore resolve against that keyed child scope.

This is important for helper functions. The same lexical `View.text(...)` site inside `toolView()` can execute for 100 tools without flattening those 100 instances into one global occurrence sequence. Each keyed tool group owns its own child site table.

The resulting logical address is naturally hierarchical:

```text
Tui composition root
  -> View.key lexical site
     -> key = tool-17
        -> toolView() internal lexical sites
```

Reordering tools merely changes traversal order. It does not change the keyed group's child scope or its retained View identities.

## 11.7 Key-group commit/reclamation

During a pass, keyed groups are marked visited but the committed map is not destructively changed.

On successful commit:

```text
visited existing key -> keep child scope and promote pending slots
new key              -> commit new child scope
previous key absent  -> dispose/remove that child scope
```

On abort:

```text
retain the previous committed key map exactly
discard newly-created pending groups
do not dispose previously committed absent-looking groups
```

This is required because a failed render must not unmount logical keyed content that is still visible in the old committed root.

---

# 12. Source transform — scope and philosophy

## 12.1 This is deliberately much smaller than React Compiler

Do not build a general optimizer.

The transform needs to understand only Iyon's own View API.

Its job is:

```text
1. find the imported public iyon-tui View binding
2. assign dense lexical site IDs
3. lower recognized View factories/modifier chains to internal monomorphic compose helpers
4. lower View.key to a keyed composition group helper
5. inject one private internal import/module registration
```

No data-flow HIR is required.

No arbitrary dependency inference is required.

## 12.2 Recognized import sources

At minimum:

```text
"iyon:tui"
"@iyon/runtime/tui"
"@iyon/runtime" when View is imported/re-exported there
```

Track aliases:

```ts
import { View as TuiView } from "iyon:tui"
```

must still be recognized.

Do not transform an unrelated class named `View`.

## 12.3 Factory lowering

Conceptually:

```ts
View.text(text)
```

becomes:

```ts
__iyonCompose.text(__iyonModule, 7, text)
```

and:

```ts
View.spacer(0)
```

becomes:

```ts
__iyonCompose.spacer(__iyonModule, 8, 0)
```

The helper immediately falls through to ordinary construction if no composition pass is active.

## 12.4 Container builder lowering

Conceptually:

```ts
View.vertical(column => {
  column.push(a)
  column.push(b)
})
```

becomes:

```ts
__iyonCompose.vertical(__iyonModule, 10, column => {
  column.push(a)
  column.push(b)
})
```

The callback executes first to obtain child Views.

Then the composition helper compares:

```text
layout scalars
child count
child View/BridgeViewNode identities
track metadata
```

against the previous semantic node for that site.

If all immediate semantics match, return the previous parent View.

No recursive child equality occurs.

## 12.5 Fluent modifier lowering

Production depends heavily on fluent modifiers:

```ts
View.component(composer)
  .style(theme.composer)
  .styleState("iyon.agent.effort", effort)
```

and:

```ts
View.vertical(...)
  .fillWidth()
  .fillHeight()
```

The transform must cover the full public View modifier chain used by the project.

Conceptually:

```ts
base.style(spec)
```

becomes:

```ts
__iyonCompose.style(__iyonModule, 11, base, spec)
```

and:

```ts
base.fillWidth()
```

becomes:

```ts
__iyonCompose.fillWidth(__iyonModule, 12, base)
```

The implementation may use generated helper names or a generated method table, but the hot path must remain monomorphic.

Do not lower common calls through:

```ts
__compose("style", [base, spec])
```

with string dispatch/rest arrays.

## 12.6 Conservative transformation

The transform must never guess incorrectly about an unrelated method call.

Safe cases include:

```text
- direct chains rooted in a recognized View static factory
- chains rooted in an already-transformed View modifier
- local identifiers whose initializer is statically a recognized View expression
- values explicitly typed as View if the transformer has reliable type information
```

If a View modifier cannot be proven syntactically, leave it unchanged rather than rewriting arbitrary `.style()` calls.

Add a development-only transform diagnostic/counter for untransformed known-View patterns. The Iyon workspace is one coverage fixture, but the transform is framework infrastructure and must be validated against a separate external-consumer fixture as well.

## 12.7 Full current production coverage is mandatory

The transform/runtime helper set must cover the normal public `iyon-tui` View construction surface, not merely the calls exercised by the bundled Iyon application. As a minimum proof, it must cover every operation used by:

```text
plugins/app/iyon/src/view.ts
plugins/app/iyon/src/app.ts
plugins/tools/*/render.ts
packages/iyon-plugins/src/tools/support/render.ts
external-consumer integration fixture(s) using only the published public API
```

including the production semantic families listed in the boundary trace:

```text
text
styled/decorated text
vertical/hanging layout
component
contentMax / clamp-related wrappers
fill width/height
style
style state
DiffRenderer outputs where composition is involved
```

Do not claim T13.1 complete while a canonical public-API pattern falls through to uncomposed rebuilds silently. The production Iyon app is evidence, not the definition of coverage.

---

# 13. Build/runtime integration — mandatory invisible framework infrastructure

This section is a hard architectural requirement.

The source transform is **not** an application plugin and is **not** an optional optimization package. It is private `iyon-tui` infrastructure. A normal external consumer must never write or maintain anything equivalent to:

```ts
plugins: [iyonViewCompositionPlugin]
enableRetainedComposition()
new Tui({ retainedComposition: true })
```

or add special `bunfig.toml`, preload, feature-flag, or app-local compiler wiring solely to receive retained composition.

## 13.1 Ownership

Place the transform and its runtime helpers under framework-owned implementation/build-support modules. Exact repository paths may follow local package conventions, but ownership must read conceptually as:

```text
iyon-tui public API
    |
    +-- private composition runtime
    +-- private composition transform
    +-- framework build/run integration
```

not:

```text
plugins/app/iyon
    -> installs/configures composition
```

The Iyon CLI may host one installation hook because it already owns a supported execution path, but the compiler must not be *defined* as an Iyon-app optimization.

Do not export a consumer-facing `iyonViewCompositionPlugin` as the normal integration contract. If a Bun plugin object exists internally, keep it private or expose it only from an explicitly internal/build-support module used by framework tooling.

## 13.2 Every supported consumer path must activate it automatically

Inventory every execution/build path that the project considers supported for `iyon-tui`, including at minimum the paths used by:

```text
- the bundled Iyon application
- external applications built through the standard Iyon/iyon-tui build path
- runtime-loaded application/plugin modules if they are part of the supported model
- tests/examples that represent documented public usage
```

For each supported path, retained composition must be activated automatically before consumer View code is evaluated.

The current repo already has two useful integration points:

```text
packages/iyon-cli/build.ts
    Bun.build(... plugins: [iyonVirtualModulePlugin])

packages/iyon-runtime/src/virtual-modules.ts
    installIyonVirtualModules()
    Bun.plugin(...)
```

Reuse or consolidate those mechanisms as appropriate, but make the result a **framework-owned aggregate bootstrap**. Consumers should invoke the normal supported runtime/build entrypoint and receive virtual modules + composition lowering together.

If the current packaging cannot guarantee transparent transformation for some advertised consumer path, fix the supported framework entrypoint/tooling in this tranche rather than documenting “also install this plugin.” Public API/tooling changes are allowed for that purpose.

## 13.3 External-consumer proof is mandatory

Add a small fixture outside `plugins/app/iyon` that looks like a third-party library consumer. It must:

```text
- import only documented public iyon-tui APIs
- contain no internal imports
- contain no composition/compiler/plugin setup
- contain no feature flag
- contain no manual View memoization
- build/run through a supported public entrypoint
```

Drive at least:

```text
exact no-op
single text change
conditional branch toggle
dynamic keyed reorder
ViewSlot recurring update
ScrollPane recurring update
```

and assert composition counters / exact View reuse where appropriate.

This fixture is the acceptance proof for “generic TUI framework”, while `plugins/app/iyon` remains the realistic production trace.

## 13.4 No silent supported-path downgrade

Untransformed execution may exist internally for:

```text
- differential tests
- benchmark control arms
- low-level/debug escape hatches
```

but it is **not** an acceptable steady-state for a normal supported consumer path.

If framework bootstrap accidentally fails to install the transform, integration tests must fail. In development, expose a diagnostic such as:

```text
composition_expected = true
composition_transformed_sites = N
composition_untransformed_known_sites = 0
```

Do not silently tell ordinary users “it is still correct, just slower” when the framework itself failed to install mandatory infrastructure.

## 13.5 AST implementation

The workspace already depends on TypeScript for type checking. A TypeScript AST transform is acceptable for this tranche.

Do not add a large Babel/SWC dependency merely to inject SiteIds unless measured build-time or implementation evidence requires it.

The transform must preserve:

```text
source maps where supported
async semantics
evaluation order
short-circuit behavior
exceptions
this binding
optional chaining semantics
```

Never move semantic expressions across side-effect boundaries just for composition.

## 13.6 Bun 1.4 qualification

The repository is qualified on Bun 1.4.0. Verify the exact build-plugin/runtime-plugin behavior on that pinned version.

The acceptance question is not “can Bun expose a plugin?” It is:

> Can the supported `iyon-tui` execution path guarantee that consumer source is transformed before relevant View construction executes, with no consumer configuration?

If one supported path cannot satisfy that with the current bootstrap sequence, change that path's framework-owned launcher/build integration rather than leaking the private compiler into application setup.

---

# 14. Composition context

Generated compose helpers need to know which boundary is currently evaluating.

Use a private synchronous composition context.

Conceptually:

```ts
let ACTIVE_PASS: ViewCompositionPass | undefined
```

but implement nesting safely.

Required semantics:

```text
enter pass A
  helper calls use A
  nested independent composition boundary B
    helper calls use B
  leave B
  helper calls resume A
leave A
```

A small stack or save/restore token is sufficient.

Do not use `AsyncLocalStorage` for this hot synchronous construction path.

View construction must not retain the active pass beyond the synchronous builder invocation.

If an async builder accidentally returns a Promise, reject it.

Composition builders are synchronous by contract.

---

# 15. Boundary API — canonical retained evaluation, not an optimization opt-in

Construction must happen inside a composition pass. A `View` fully constructed before the framework enters a retained boundary cannot retroactively avoid those JS allocations.

Public API changes are therefore allowed when they establish the correct retained lifecycle. The crucial rule is that the API shape is **semantic/canonical**, not “the fast variant”. Users must never choose between a normal API and an optimized composition API.

## 15.1 Canonical builder types

Conceptually:

```ts
export type ViewBuilder = () => View
export type SceneBuilder = () => Scene
```

Names may follow existing conventions.

The implementation agent must choose one canonical repeated-boundary form and document it as ordinary `iyon-tui` usage.

## 15.2 `Tui.render`

Preferred canonical recurring form:

```ts
tui.render(() =>
  new Scene(
    createView(state),
    history,
  )
)
```

The closure is **not** a memoization hook and is **not** an optimization switch. Its semantic purpose is to give `Tui` ownership of the evaluation transaction so framework-owned composition can establish the correct root before View factories run.

Existing:

```ts
render(scene: Scene): void
```

may remain source-compatible for prebuilt/one-shot values. If retained, document reference identity/performance as framework-private and do not present the direct form as an equivalent recurring “slow path” users must reason about.

If the compiler can transparently lower ordinary direct render expressions into the same internal transaction without unsafe guessing, that is even better and may preserve the existing source shape. Do not require such lowering if it materially complicates the transform; the canonical builder form is acceptable because public API changes are permitted.

## 15.3 `ViewSlot.setView`

Canonical recurring form:

```ts
slot.setView(() => userBatchView(messages))
```

The slot owns its composition root and evaluates the builder within that root.

Existing `setView(view: View)` may remain for already-built/one-shot values and animation-frame cases where the View identity is already intentionally stable.

## 15.4 `ScrollPane.setContent`

Canonical recurring form:

```ts
pane.setContent(() => toolOutputView(update))
```

The pane owns its composition root and evaluates the builder within that root.

Existing direct-View form may remain for compatibility/one-shot use.

## 15.5 Public API promise

For every documented canonical recurring boundary:

```text
normal public API use
+ supported framework build/runtime path
=
retained composition automatically active
```

There is no separate `compose`, `memo`, `optimized`, `retained`, or compiler-registration step in application code.

## 15.6 Initial construction APIs

Do not add lazy/builder forms to every one-shot constructor merely for symmetry. Add them where a boundary actually owns repeated semantic evolution or where consistency materially improves the API.

## 15.7 History

Do not force History's immutable one-shot units through a new composition root.

History already owns retained native state and stream specialization. `push(view)` and a unit's terminal `freeze(unit, view)` do not form a general repeatedly-rendered root in the same sense as B1/B3/B4.

Leave B2 semantics unchanged unless a concrete repeated-unit workflow demonstrates a missing optimization.

---

# 16. Composition execution transaction

The composition state and native/host root state must advance atomically from the application's perspective.

## 16.1 Required sequence

For a composed boundary update:

```text
1. begin composition pass from last committed composition
2. activate pass
3. execute user builder synchronously
4. deactivate pass
5. finalize/prepare composition work state
6. attempt existing retained/cold/complete boundary install
7. if install succeeds:
       commit composition state
       boundary's existing root lease is now the new committed root
   else:
       abort composition state
       old composition remains authoritative
       old native root lease remains authoritative
8. return / rethrow
```

## 16.2 Builder failure

If the user builder throws:

```text
abort composition
make no host mutation
leave old root lease untouched
rethrow original exception
```

## 16.3 Native materialization failure

If retained materialization fails and complete fallback succeeds:

```text
commit composition
```

Transport route choice is irrelevant to composition correctness.

If all install routes fail:

```text
abort composition
```

## 16.4 Commit must be effectively infallible

Do not perform risky semantic construction after the host has accepted the new root.

Prepare any arrays/cleanup bookkeeping before host mutation where practical.

Post-host composition commit should be limited to operations such as:

```text
pointer/reference swaps
epoch advancement
pending -> current slot promotion
bounded map deletions
```

No user callbacks.

No semantic View factory calls.

---

# 17. Exact reuse algorithm

This is the heart of T13.1.

For every transformed semantic View operation:

```text
1. obtain current composition slot by module/site/(occurrence|key)
2. read slot.current
3. inspect only that previous node's immediate semantic fields
4. compare against the new operation's immediate semantic arguments
5. if exact semantic match:
       stage/revisit same View
       return exact previous View object
6. otherwise:
       construct one new immutable View
       record valid predecessor/derivation metadata
       stage new View
       return new View
```

No descendants are recursively compared.

## 17.1 Why immediate comparison is sufficient

Children are themselves built first.

If a child is unchanged, composition already returned the exact previous child View.

Therefore a parent can determine unchanged child semantics by identity:

```ts
oldChild === newChild
```

or equivalently:

```ts
nodeForBridge(oldChild) === nodeForBridge(newChild)
```

So parent comparison remains shallow.

## 17.2 Example

Old committed tree:

```text
root A
├── working A
├── approval A
├── composer A
└── footer A
```

Only footer text changes.

Composition produces:

```text
root B
├── SAME working A
├── SAME approval A
├── SAME composer A
└── footer B
```

Only the semantic wrappers from the changed footer back to the root become new nodes.

PERF-12 then sees exactly the changed frontier it was designed for.

---

# 18. Semantic equality — generated, shallow, kind-specific

Do not use:

```text
JSON.stringify
Object.keys
Object.entries
recursive equality
content hash
reflection over arbitrary properties
```

Use monomorphic comparisons for known View semantic kinds.

The exact generated/helper structure should follow the current `BridgeViewNode` schema.

## 18.1 Primitive scalars

Compare directly:

```ts
old.gap === gap
old.width === width
old.height === height
old.wrap === wrap
old.align === align
old.componentId === componentId
```

## 18.2 Strings

Use JavaScript string equality for correctness:

```ts
old.text === text
```

Do not pre-hash text.

Production long assistant text is already streamed outside the structural DAG, so common structural strings are small.

Benchmark long static strings separately to ensure no hidden pathological cost.

## 18.3 Child Views

Compare exact semantic child identity:

```ts
old.child === nodeForBridge(newChild)
```

or equivalent cached internal references.

Never recursively compare the child.

## 18.4 Arrays of ordinary small children

For common small Row/Column-style nodes, compare:

```text
length
per-entry layout/track scalar
per-entry child BridgeViewNode identity
```

This is O(arity), not O(descendant tree size).

For normal application chrome with small arity this is appropriate.

Wide structures are handled separately in §22.

## 18.5 Styles/decorations

Reuse existing normalized semantic representations rather than comparing user objects by arbitrary deep equality.

Where the semantic DAG already stores normalized immutable style data or a stable style reference/key, compare that representation.

Do not make composition depend on resolved theme colors; theme resolution remains late/native.

## 18.6 Component handles

Compare stable component identity already represented in the semantic node.

Do not compare wrapper object incidental identity if the existing semantic representation has a canonical native component ID.

## 18.7 Diff

Diff structural values may be large.

Do not introduce an O(diff payload) composition comparison on every re-render if the Diff path already has retained payload identity/renderer specialization.

Use existing retained Diff identity where available; otherwise treat a new Diff payload as changed and let the specialized retained/native lane handle it.

## 18.8 Unknown/rare kinds

Correct fallback is:

```text
cannot cheaply prove equal -> construct a new immutable View
```

Never guess equality.

Performance optimization requires proof of sameness; semantic correctness does not require reuse.

---

# 19. Avoiding allocation on exact reuse

The preferred hot path must compare the new operation's raw semantic arguments **before** allocating a new `View`, assigning a NodeId, or constructing transport metadata.

Preferred:

```ts
function composeSpacer(moduleId: number, siteId: number, size: number): View {
  const slot = currentSlot(moduleId, siteId)
  const previous = slot.current

  if (previous !== undefined) {
    const node = nodeForBridge(previous)
    if (node.kind === "spacer" && node.size === size) {
      stage(slot, previous)
      return previous
    }
  }

  const next = View.__uncomposedSpacer(size)
  stage(slot, next)
  return next
}
```

Avoid as the final common path:

```text
construct fresh View
assign NodeId
construct BridgeViewNode
then compare candidate vs previous
then discard candidate
```

That fallback may be useful while bringing up uncommon semantic kinds, but the production-hot families must be pre-allocation reuse checks.

This is necessary to make the handoff's original promise:

```text
semantic JS construction O(changed semantic nodes)
```

true in ordinary application code.

---

# 20. Predecessor / derivation integration

Composition gives PERF-12 information it did not reliably have before:

```text
"new semantic node X is the next value of the same logical construction site as old node Y"
```

Use that information only where it maps to an existing valid retained derivation.

## 20.1 Do not make predecessor identity semantic

A predecessor relation is optimization metadata.

Store it in a WeakMap/private sidecar, not on the BridgeViewNode object.

Conceptually:

```ts
const COMPOSITION_PREDECESSOR = new WeakMap<BridgeViewNode, BridgeViewNode>()
```

or fold it into the existing derivation sidecar when a specific derivation can be proven.

## 20.2 Prefer specific derivation variants

If composition observes:

```text
same logical Text
same payload
new wrap/alignment
```

install/retain the existing text-layout derivation.

If it observes:

```text
same logical common node
one supported scalar changed
```

use existing common-scalar derivation.

If a container child group exposes one set/splice:

```text
axis-set / axis-splice
```

If one Grid cell changes:

```text
grid-cell
```

Do not add a generic native "diff these two arbitrary Views" operation.

## 20.3 Unsupported changed semantics

If no specific retained derivation applies:

```text
new immutable semantic node
-> normal ensureNative materialization
```

Correctness first.

---

# 21. Conditional structure

Lexical SiteIds are specifically chosen so an earlier conditional does not renumber unrelated later sites.

Example:

```ts
if (state.working) {
  View.component(workingHandle)
}

View.component(composer)
View.text(footer)
```

The transform assigns distinct lexical sites:

```text
working  -> site 4
composer -> site 5
footer   -> site 6
```

When `working` disappears:

```text
site 4 is absent
site 5 is still site 5
site 6 is still site 6
```

This is a major advantage over a single global "construction ordinal" counter.

Do not implement composition identity as merely:

```text
first View created this render
second View created this render
third View created this render
```

because a conditional would shift every later identity.

---

# 22. Wide structures and PersistentSeq

T13.1 must not accidentally undo PERF-12's largest asymptotic wins.

## 22.1 Never scan 100k descendants to prove a retained one-child edit

Current PERF-12 wide paths already know how to express:

```text
set(index, child)
remove(index)
splice(index, removeCount, inserted)
```

through retained PersistentSeq semantics.

Composition must preserve those operation descriptors where they are already produced by the View API.

## 22.2 Small ordinary container

For common 2–20 child chrome:

```text
compare immediate child identities linearly
```

is acceptable and simpler.

## 22.3 Wide container

For a wide sidecar-backed sequence:

```text
base View identity
+ sequence derivation sidecar
+ operation descriptor
```

should be the reuse/delta source.

Do not flatten it merely so composition can compare arrays.

## 22.4 Fresh arbitrary arrays have an information cost

If the public caller gives Iyon a brand-new arbitrary 100,000-element array every render and provides neither stable Views nor retained edit information, some O(N) work is unavoidable to discover what changed.

T13.1 must not hide that fact behind a hash table or expensive content digest.

The preferred scalable APIs remain:

```text
History
ViewSlot
ScrollPane
streams
PersistentSeq-backed retained edits
keyed repeated groups where appropriate
```

Production already places large conversation content behind these retained subsystems.

---

# 23. Keyed dynamic collections

For repeated calls from one lexical site, occurrence order is the default.

Example:

```ts
for (const item of items) {
  column.push(itemView(item))
}
```

If insertion/reorder continuity matters, use:

```ts
for (const item of items) {
  column.push(
    View.key(item.id, () => itemView(item)),
  )
}
```

This should be the only identity API most users ever see.

Do not require IDs on:

```text
text nodes
rows
columns
spacers
styles
ordinary static components
ordinary modifiers
```

## 23.1 Optional future convenience

A future collection builder such as:

```ts
column.each(items, item => item.id, item => itemView(item))
```

may sugar `View.key`, but do not expand T13.1 unless production code clearly benefits.

---

# 24. Helper functions and repeated call sites

T13.1 does not need a full compiler-level call graph.

A View factory site inside a helper may execute multiple times. The baseline identity is:

```text
lexical site inside helper + occurrence order
```

This remains semantically safe because Views are immutable semantic values, not stateful React component instances.

If repeated helper instances can reorder and retaining logical continuity matters, callers use `View.key` around the helper's produced View.

Example:

```ts
function toolRow(tool: Tool): View {
  return View.text(tool.label).style(toolStyle(tool))
}

for (const tool of tools) {
  column.push(
    View.key(tool.id, () => toolRow(tool)),
  )
}
```

This avoids building a general function-call composition compiler in T13.1.

---

# 25. Composition slot lifetime and reclamation

## 25.1 Strongly retain the last committed View for live sites

A composition slot must hold its last committed View strongly while the logical site is live.

That is intentional: the entire purpose is to make exact identity available to the next composition.

## 25.2 Do not retain unlimited historical values

Each ordinary positional slot retains at most:

```text
one committed View
one temporary pending View during a pass
```

A successful commit replaces the old slot value.

An abort drops the pending value.

## 25.3 Repeated positional sites

If a site executed N times last render and M < N times this render, successful commit must release the tail:

```text
slots[M..N]
```

No indefinite high-water retention after the site remains active with a smaller cardinality.

## 25.4 Keyed sites

On successful commit, remove keyed entries that were present in the previous committed pass but not visited in the new pass.

Do not run this cleanup on abort.

## 25.5 Entire skipped lexical sites

A site absent from one successful composition pass is logically unmounted.

When it next appears, its previous semantic value must not be treated as continuously mounted.

Implementation choices:

```text
A. eagerly mark/clear previous-pass active sites at commit
B. mark last-seen committed epoch and reset lazily on next appearance
```

Choose based on benchmark evidence, with these hard requirements:

```text
- semantic lifetime is correct
- memory remains bounded
- exact stable chrome does not gain an O(all historical sites) cleanup path
```

A small previous-active-site list is acceptable for normal 20–200 node roots if measured cost is negligible.

## 25.6 Root disposal

`ViewCompositionRoot.dispose()` must immediately release all strong View references and keyed maps.

Wire it to:

```text
Tui.close/dispose lifecycle
ViewSlot.dispose
ScrollPane.dispose
```

as appropriate.

---

# 26. Composition must not become another semantic cache

The composition table stores:

```text
logical slot -> actual immutable View object
```

It does **not** store:

```text
copied semantic payload
serialized node record
child record graph
NativeRef ownership graph
NodeId -> semantic content map
wire bytes
```

Semantic authority remains:

```text
BridgeViewNode / NodeId
        +
NativeViewRuntime NodeId -> WeakView
```

Composition is only continuity metadata.

This distinction is why T13.1 does not recreate the rejected Shared Mirror architecture on the JS side.

---

# 27. B1 — `Tui.render` integration

Add one `ViewCompositionRoot` beside the existing `RetainedRootBoundary`.

Conceptually:

```ts
class Tui {
  readonly #rootBoundary = new RetainedRootBoundary()
  readonly #compositionRoot = new ViewCompositionRoot()

  render(sceneOrBuilder: Scene | (() => Scene)): void {
    if (typeof sceneOrBuilder !== "function") {
      this.#renderPreparedScene(sceneOrBuilder)
      return
    }

    const pass = this.#compositionRoot.begin()
    let scene: Scene

    try {
      scene = withActiveComposition(pass, sceneOrBuilder)
      pass.prepare()
      this.#renderPreparedScene(scene)
      this.#compositionRoot.commit(pass)
    } catch (error) {
      this.#compositionRoot.abort(pass)
      throw error
    }
  }
}
```

Real code must respect current error/fallback behavior.

## 27.1 No-op path

If composition returns the exact previous root View:

```text
currentScene.body === nextScene.body
```

must still trigger the existing no-op route.

Expected structural result:

```text
new NodeIds:                 0
semantic nodes inspected:   0
children visited:           0
NativeRef materializations: 0
host render native calls:   0 for the existing JS no-op route
```

Composition evaluation itself is measured separately.

## 27.2 History object changes

`Scene` also carries History.

Do not treat exact body identity as permission to ignore a materially different History binding if current `Scene` semantics allow changing it.

Preserve current normalization/render semantics exactly.

---

# 28. B3 — `ViewSlot` integration

Each `ViewSlot` gets one `ViewCompositionRoot` in addition to its existing `RetainedRootBoundary`.

Builder update:

```ts
slot.setView(() => userBatchView(messages))
```

executes inside that slot's composition root.

If only one line changes:

```text
unchanged lines -> same Views
changed line    -> new View
container spine -> changed semantic path only
```

then the existing retained boundary installs the result.

## 28.1 Animation APIs

Do not force stable prebuilt animation frame arrays through new composition every tick.

Current animation architecture intentionally reuses stable frame View identities and native animation machinery.

Composition may be used when the animation *definition* is rebuilt, but the tick path must remain native/retained.

---

# 29. B4 — `ScrollPane` integration

Same model as ViewSlot:

```ts
pane.setContent(() => toolOutputView(update))
```

uses a pane-local composition root.

`followEnd()` remains native and unrelated.

Do not couple content composition identity to scroll position.

---

# 30. B2/B5/B6 behavior

## 30.1 History B2

History stays retained native state.

Do not add a composition root to every frozen History unit by default.

Streams continue to bypass structural View composition entirely.

## 30.2 Component references B5

`View.component(handle)` continues to lower the stable component handle/native ID into the semantic node.

Composition merely recognizes unchanged component semantics automatically.

No new component lease model.

## 30.3 Theme B6

Theme installation remains independent.

T13.1 must not cache resolved colors into composition metadata.

If T13 proper adds theme epochs for style references, composition compares semantic style/theme keys under those existing rules.

---

# 31. Iyon application — normal consumer migration and black-box validation

`plugins/app/iyon` is **not** an architectural layer of retained composition. It is one application built on the generic public `iyon-tui` framework and serves two purposes in T13.1:

```text
1. realistic production-trace benchmark
2. black-box proof that normal consumers receive the framework behavior
```

No app-local compiler setup, composition plugin registration, internal SiteId import, DAG access, NativeRef access, or manual stable-View cache is allowed.

## 31.1 Migrate only to canonical public API where needed

If §15 establishes builder-based repeated boundaries as the canonical public contract, migrate the Iyon app exactly as any external consumer would:

```ts
tui.render(() =>
  new Scene(
    createIyonView(options),
    this.historyHandle,
  )
)
```

and similarly for recurring `ViewSlot` / `ScrollPane` updates.

That source change is acceptable because it is a normal public API evolution. It must not contain any knowledge of retained DAG internals or compiler implementation.

If the compiler safely preserves the existing direct-expression source shape instead, no app migration is necessary. Choose the simpler public contract, not a special app path.

## 31.2 `createIyonView()` remains ordinary declarative code

Do not add:

```text
memo tables
body-derived semantic caches
NodeIds
NativeRefs
compiler SiteIds
internal composition helpers
manual stable View retention
```

to `plugins/app/iyon/src/view.ts`.

The same rule applies to tool renderers and other application/plugin code.

## 31.3 `bodyKey` is temporary evidence only

Keep `lastRenderedBodyKey` during initial parity/benchmarking as a control arm.

Add a benchmark mode that bypasses it while framework composition is active.

Remove the production `bodyKey` only after:

```text
automatic composition no-op produces exact root reuse
real trace performance is not worse
animations still advance correctly
all bodyKey-covered state transitions produce identical visuals
external-consumer fixture passes without special setup
```

### Preserve the current animation/time side effect

Today a `bodyKey` hit also calls `advance?.(0)`. Removing the guard must not accidentally remove this runtime advancement opportunity.

Place that behavior in the smallest semantically correct framework/application scheduling location after tracing current ownership. View composition itself must not become a clock or scheduler.

Add a regression test in which repeated exact-root composed renders still permit the existing spinner/stream/headless advancement behavior.

The final Iyon app should not need `bodyKey` for renderer identity.

## 31.4 ViewSlot and ScrollPane

Migrate recurring reconstructed content only to the canonical public boundary forms selected in §15. Do not use any internal composition API.

Already-stable prebuilt animation frames do not need artificial reconstruction solely to exercise composition.

---

# 32. Public API and consumer-experience contract

This section is mandatory for T13.1 review.

## 32.1 Retained composition is part of the framework contract

For a documented supported `iyon-tui` consumer path, retained composition is not optional.

The user experience must **not** contain:

```ts
installCompositionCompiler()
use(iyonViewCompositionPlugin)
enableRetainedViews()
new Tui({ composition: true })
```

and must not require app-local Bun plugin arrays, preload configuration, feature flags, or manual memoization.

## 32.2 Public API changes are allowed when they define lifecycle semantics

T13.1 may add or canonicalize APIs such as:

```ts
Tui.render(() => Scene)
ViewSlot.setView(() => View)
ScrollPane.setContent(() => View)
View.key(key, () => View)
```

because these express real lifecycle/identity semantics.

They must not be marketed or designed as “optimized variants”. They are ordinary framework APIs.

Keep existing direct-View APIs where source compatibility is cheap and semantics remain clear. Deprecation is preferable to maintaining two permanently equivalent-looking APIs with materially different lifecycle guarantees.

## 32.3 Explicit `View.key` is semantic, not opt-in performance plumbing

`View.key` is allowed only where logical identity is ambiguous, such as repeated/reordered siblings originating from the same lexical site.

Static ordinary nodes must not require user IDs.

The key is local composition identity metadata, not a NodeId, NativeRef, cache key, or declaration that content is unchanged.

## 32.4 Transformation must not change visible semantics

The same canonical application logic must have identical visible semantics in internal transformed-vs-reference differential tests.

Transformation may change:

```text
object reuse
NodeId reuse
allocation count
amount of bridge work
```

It must not change:

```text
visual output
layout semantics
style semantics
component identity
History semantics
stream semantics
event routing
evaluation order
exception behavior except composition-specific duplicate-key errors
```

## 32.5 View reference equality is framework-private

Audit documentation/tests for any promise that every View factory call returns a newly allocated distinct object.

The public contract should define `View` as an immutable semantic value whose reference identity may be reused by the framework.

Application logic must not rely on `a === b` meaning semantic equality or inequality.

## 32.6 No normal supported untransformed mode

An untransformed/reference mode may exist for tests and benchmarks, but it is not a normal consumer feature.

A supported external consumer that follows documented setup and still runs canonical View code untransformed is a framework integration bug.

The external-consumer fixture from §13.3 must guard this invariant.

## 32.7 No public compiler escape hatch by default

Do not add a public transform plugin or opt-out switch merely for convenience.

If a genuine tooling incompatibility later requires an escape hatch, isolate it in framework tooling/debug support and preserve semantics. It must not become the documented normal installation path.

---

# 33. Internal API stability

The following private contracts should remain narrow:

```text
register composition module
begin/end active composition pass
monomorphic composition factory helpers
monomorphic modifier helpers
keyed group helper
composition counters/debug metadata
```

Do not expose these from `iyon:tui`'s normal public TypeScript surface.

The transform may import them from a private/internal package export that is clearly marked `@internal`.

---

# 34. Compiler/framework failure behavior

## 34.1 Unsupported syntax

If the transform cannot safely lower an unusual expression, semantic correctness wins: leave the expression unchanged rather than emit incorrect code.

However, distinguish two cases:

```text
A. unusual/noncanonical dynamic JS pattern
   -> correct fallback is acceptable

B. documented canonical iyon-tui usage pattern
   -> missed lowering is a T13.1 defect
```

The supported external-consumer fixture and public API coverage tests must keep case B at zero.

## 34.2 Bootstrap/integration failure

If the framework expects composition transformation for a supported path but the private transform was not installed/executed, do not silently normalize that as “optional optimization disabled”.

Development/test builds must make this diagnosable and acceptance tests must fail.

## 34.3 Build diagnostics

Development builds should be able to report privately:

```text
module path
site count
transformed View factory count
transformed modifier count
untransformed suspicious known-View call count
framework bootstrap/transform-active status
```

No diagnostic work belongs in the hot runtime path.

## 34.4 Source maps

Preserve usable source locations for exceptions and debugging.

A mandatory invisible transform that makes consumer stack traces unusable is not acceptable.

---

# 35. Composition counters

Add counters separate from existing retained DAG counters.

At minimum:

```text
composition_passes
composition_commits
composition_aborts
composition_modules_touched
composition_sites_touched
composition_positional_slot_hits
composition_positional_slot_misses
composition_keyed_slot_hits
composition_keyed_slot_misses
composition_exact_view_reuses
composition_new_views
composition_predecessor_hints
composition_duplicate_key_errors
composition_removed_positional_slots
composition_removed_keyed_slots
composition_untransformed_fallbacks   // dev/bench only if useful
```

For benchmark correlation also retain the existing PERF-12 counters:

```text
semantic_nodes_inspected
children_visited
direct_materializer_calls
derivation_fast_path_calls
byte_payload_bytes
cold_fallbacks
NativeRef promotion counters
host mutation counters
```

The important proof is the cross-layer relationship:

```text
composition exact reuse
        ->
zero new NodeId
        ->
zero retained frontier visit for that subtree
```

---

# 36. Required evidence probe before deleting `bodyKey`

Create one focused T13.1 benchmark/probe through the **real production `Tui.render` router**.

Model the actual `createIyonView` chrome shape from production.

Required arms:

```text
A. current_body_key
   current application guard + ordinary construction

B. rebuild_uncomposed
   bodyKey disabled; fresh ordinary View construction each op

C. manual_stable_oracle
   hand-preserved View identities purely as an experimental upper-bound/control
   NOT production code

D. composed_auto
   bodyKey disabled; ordinary application source evaluated through T13.1 composition
```

The manual stable arm answers:

```text
"What structural counters should perfect automatic composition reproduce?"
```

The composed arm must converge to that structural shape without app memoization.

---

# 37. Production-state benchmark cases

At minimum model these state transitions from `plugins/app/iyon`.

## 37.1 Exact semantic no-op

Reduced state changes elsewhere but chrome inputs are unchanged.

Expected:

```text
exact root View reused
0 new semantic nodes
0 retained semantic nodes inspected
0 native materializers
existing Tui no-op route
```

## 37.2 Footer-only change

Change:

```text
provider/model/effort/status footer text
```

Expected:

```text
working subtree reused
approval subtree reused
composer subtree reused
footer changed
only footer wrapper path + ancestors become new
```

## 37.3 Effort style-state change

Change only:

```text
"iyon.agent.effort"
```

Expected:

```text
component handle reused
unrelated chrome reused
derivation/common-scalar path used where valid
```

## 37.4 Working spinner visibility toggle

Toggle conditional:

```text
working row <-> spacer
```

Expected:

```text
composer lexical identity preserved
footer lexical identity preserved
approval lexical identity preserved
no positional-site shift caused by the earlier conditional
```

## 37.5 Approval appear/disappear/change

Same principle as working.

## 37.6 Steering queue preview change

Only affected working-row branch and ancestors should change.

## 37.7 Tool status update behind retained slot

Scene chrome may be logically unchanged while a tool ViewSlot changes.

Expected:

```text
scene composition -> exact root reuse/no-op
slot composition  -> changed frontier only
```

## 37.8 Tool pane output update

Expected:

```text
scene unchanged
pane-local composition only
followEnd native behavior unchanged
```

---

# 38. Key correctness tests

Add deterministic tests for:

```text
[ ] same site + same key + same semantics -> exact View reuse
[ ] same site + same key + changed semantics -> new NodeId, known predecessor
[ ] same site + changed key -> new logical lifetime
[ ] same key at different lexical sites -> independent
[ ] same key in different composition roots -> independent
[ ] duplicate key in same site/pass -> deterministic error
[ ] reorder keyed items -> individual item View identities follow keys
[ ] insert keyed item at front -> existing items retain identity
[ ] remove keyed item -> removed slot released after successful commit
[ ] aborted pass with removals -> old keyed map remains committed
```

---

# 39. Conditional/occurrence tests

Required:

```text
[ ] later lexical sites retain identity when earlier conditional disappears
[ ] same lexical site repeated without keys uses occurrence order
[ ] shrinking occurrence count releases tail after successful commit
[ ] aborted shrink does not release committed tail
[ ] reappearing previously-unmounted site receives correct lifetime behavior
[ ] nested View.key groups scope child identities independently
```

---

# 40. Semantic-kind differential tests

For every public View semantic family supported by composition:

```text
untransformed construction
vs
transformed composed construction
```

must produce equivalent Bridge semantics.

Test both:

```text
exact unchanged inputs
one-field mutation
multiple-field mutation
child identity mutation
```

For unchanged inputs assert:

```text
composed_next === composed_previous
nodeForBridge(composed_next) === nodeForBridge(composed_previous)
NodeId unchanged
```

For changed inputs assert:

```text
composed_next !== composed_previous
NodeId changed
semantic output equals untransformed reference
```

---

# 41. Failure atomicity tests

Reuse T12/T13 failure injection where possible.

Inject failure after composition evaluation at:

```text
NativeRef resolution
new-node materialization
retained patch
hostRenderRef
ViewSlot setViewRef
ScrollPane setContentRef
cold fallback
complete fallback
```

For every failure:

```text
[ ] old composition slots still current
[ ] old root View still current
[ ] old native root lease still valid
[ ] pending composition refs dropped
[ ] retry from same application state produces correct result
```

Then verify success commits once.

---

# 42. Multi-root isolation tests

Composition identity is root-local.

Test:

```text
Tui A renders module/site 7
Tui B renders same module/site 7
```

They must have independent slots and independent committed Views.

Likewise:

```text
ViewSlot A
ViewSlot B
ScrollPane A
ScrollPane B
```

must never share logical composition state merely because transformed source sites are identical.

Semantic DAG sharing that occurs through actual identical immutable View objects is still allowed where explicitly passed/shared by the caller.

---

# 43. Memory tests

T13.1 adds strong JS references by design, so memory behavior must be proven.

## 43.1 Static chrome churn

Run at least 1,000,000 composition updates over a ~200-site synthetic UI while changing a small subset each op.

After GC/maintenance:

```text
live composition slots = O(current logical sites)
not O(total updates)
```

## 43.2 Repeated positional churn

Grow a repeated site to a large count, shrink to a small count, commit, GC.

Tail Views must become reclaimable.

## 43.3 Key churn

Cycle through many keys while retaining only a small live set.

After each successful removal pass and GC:

```text
keyed map size follows live keys
```

not historical keys.

## 43.4 Abort churn

Repeatedly build large pending compositions and inject failure.

Pending Views must not remain strongly retained after abort.

## 43.5 Root dispose

After dispose + GC:

```text
composition root retains no Views
```

---

# 44. Performance benchmark methodology

Use process isolation and the same discipline as PERF-11v4/PERF-12.

Report:

```text
median
p95
p99
mean only as supplementary
raw JSONL samples
independent process repetitions
```

Separate phases:

```text
application render-function evaluation
composition lookup/equality
semantic View construction
retained native frontier
host layout/paint
total operation
```

Do not hide composition time inside a renamed "construction" bucket.

---

# 45. Performance gates

T13.1 is primarily an architecture-correctness tranche, but it must not silently buy identity with expensive JS reconciliation.

## 45.1 Structural gate — mandatory

For the real production chrome cases:

```text
composed_auto structural counters
=
manual_stable_oracle structural counters
```

where the semantics are equivalent.

In particular:

```text
exact no-op:
  0 new semantic nodes
  0 native semantic nodes inspected
  0 children visited

footer-only:
  only footer semantic path changes

conditional toggle:
  unrelated later sites retain exact identities
```

## 45.2 No full-tree native work

No ordinary state update may regress to:

```text
fresh whole-tree materialization
full Direct decode
cold graph build
```

merely because application code re-evaluated its View builder.

## 45.3 Exact no-op JS cost

Compare:

```text
current bodyKey guard
vs
composed builder returning exact old root
```

Preferred:

```text
composed no-op <= current bodyKey total guard cost + 10%
```

If composition is measurably slower in the tiny chrome no-op case, profile and remove avoidable per-site allocations/Map lookups before deleting bodyKey.

The final decision should use absolute nanoseconds/microseconds as well as percentages.

## 45.4 Changed production update

`composed_auto` should be no slower than `rebuild_uncomposed` end-to-end and should approach the `manual_stable_oracle` retained/native counters.

Preferred:

```text
>= 10% faster than rebuild_uncomposed
```

on state changes where substantial semantic subtrees remain stable.

Do not reject the architecture solely because host paint dominates total time if composition structurally removes the redundant bridge/native work; but do reject a composition mechanism whose JS overhead erases the benefit.

## 45.5 Cold one-shot View construction

Uncomposed `View.*` construction outside an active composition pass must remain within noise of the pre-T13.1 eager DAG path.

The transform's internal helpers must immediately fall through when no pass is active without imposing a meaningful universal tax.

Target:

```text
<= 3% credible regression on ordinary uncomposed construction
```

## 45.6 Keyed list

For reorder/insert of a keyed list:

```text
unchanged keyed item Views retain identity
```

and native work is proportional to changed container/frontier semantics rather than every reordered item's payload.

## 45.7 Wide retained edits

Existing PERF-12 wide benchmark asymptotics must remain unchanged:

```text
axis set/splice retained semantic work stays O(log_32 N + inserted)
```

T13.1 may not introduce an O(width) composition scan into those benchmark paths.

---

# 46. Benchmark matrix

At minimum:

```text
small chrome ~20 nodes
production-like chrome
200-node synthetic declarative tree
2,000-node structural test
wide 32
wide 256
wide 2,000
wide 10,000
wide 100,000 where current PERF-12 benchmark applies
```

Operations:

```text
exact no-op
one leaf text change
one metadata/style-state change
one conditional branch toggle
three changed nodes
multiple independent changed branches
keyed insert
keyed remove
keyed reorder
axis set
axis remove
axis splice4
ViewSlot update
ScrollPane update
```

Arms:

```text
current_body_key
rebuild_uncomposed
manual_stable_oracle
composed_auto
```

Do not compare only composition microbenchmarks.

---

# 47. Instrumentation proof examples

A successful exact no-op should resemble:

```text
composition_passes               1
composition_exact_view_reuses    ~= all executed View semantic sites
composition_new_views            0
semantic_nodes_inspected         0
children_visited                 0
direct_materializer_calls        0
derivation_fast_path_calls       0
cold_fallbacks                   0
```

A footer-only update should resemble:

```text
composition_exact_view_reuses    many
composition_new_views            small changed path only
semantic_nodes_inspected         changed frontier only
derivation/materializers         changed frontier only
```

A keyed list reorder should show:

```text
keyed_slot_hits                  existing item count
new item semantic construction  only genuinely new/changed items
```

---

# 48. Implementation order inside this single tranche

This remains one ambitious MR/tranche. Implement in this order so framework ownership is proven before app cleanup.

## Step 1 — freeze and probe current behavior

Add the four-arm production-chrome evidence benchmark before changing runtime behavior:

```text
bodyKey control
uncomposed/reference rebuild
manual stable oracle
current structural counters
```

Also create the skeleton external-consumer fixture with no composition setup.

## Step 2 — composition runtime

Implement:

```text
ViewCompositionRoot
ViewCompositionPass
module/site tables
occurrence slots
commit/abort
counters
```

Unit-test with synthetic internal helper calls.

## Step 3 — internal monomorphic semantic compose helpers

Cover production-hot families first, then the documented public semantic View surface required by T13.1.

## Step 4 — lexical SiteId transform

Implement the private framework-owned source transform and module registration.

Prove:

```text
conditional site stability
alias imports
fluent chains
source maps/evaluation order
```

Do not expose an application-installable plugin as the normal contract.

## Step 5 — automatic framework bootstrap

Wire the private transform into every supported build/runtime entrypoint identified in §13.

At this point the external-consumer fixture must receive transformed composition with **zero** plugin/configuration code in the fixture.

This step is a hard gate before app-specific migration.

## Step 6 — `View.key`

Add keyed group semantics, duplicate detection, cleanup, reorder tests.

## Step 7 — canonical retained boundary APIs

Wire composition roots into:

```text
Tui.render
ViewSlot.setView
ScrollPane.setContent
```

Use the canonical public shape chosen in §15. Keep compatibility overloads only where they remain conceptually clear.

## Step 8 — derivation integration

Use same-logical-site predecessor information to feed only proven retained derivation families.

## Step 9 — generic external-consumer acceptance

Run the fixture from §13.3 and require:

```text
no compiler/plugin setup in consumer source/config
transform active automatically
zero canonical known-View fallback sites
exact no-op View/root reuse
changed-frontier structural counters
keyed reorder correctness
```

T13.1 cannot pass solely because the bundled Iyon app works.

## Step 10 — Iyon reference consumer migration

If the canonical public boundary API changed, migrate `plugins/app/iyon` exactly as an external consumer would. No internal imports or compiler wiring are permitted.

Keep `bodyKey` as a benchmark/control switch initially.

## Step 11 — full production trace test

Exercise B1/B3/B4 plus History/stream interactions and the animation/time side effect.

## Step 12 — authoritative benchmark and bodyKey decision

If all structural, performance, memory, external-consumer, and production gates pass:

```text
remove app-layer bodyKey
```

If they do not, fix framework composition cost/integration. Do not move the optimization into app code as a workaround.

---

# 49. Likely files to add/change

Exact organization may differ, but ownership should be approximately:

```text
packages/iyon-runtime/src/tui/composition.ts                 NEW
packages/iyon-runtime/src/tui/internal-composition.ts        NEW/private
packages/iyon-runtime/src/tui/values/view.ts                 MODIFY
packages/iyon-runtime/src/tui/runtime.ts                      MODIFY
packages/iyon-runtime/src/tui/component.ts                    MODIFY
packages/iyon-runtime/src/tui/scroll-pane.ts                  MODIFY
packages/iyon-runtime/src/tui/index.ts                        MODIFY only for canonical public lifecycle/key API

framework-owned build-support/composition-transform module    NEW/private
packages/iyon-runtime/src/virtual-modules.ts                  MODIFY framework bootstrap as needed
packages/iyon-cli/build.ts                                    MODIFY supported build integration as needed

external consumer fixture/package                             NEW
  - imports only public iyon-tui surface
  - contains zero transform/plugin setup

packages/iyon-runtime/src/tui/__tests__/composition*.test.ts  NEW
packages/iyon-runtime/src/tui/__tests__/runtime*.test.ts      EXTEND
packages/iyon-runtime/bench/perf12_t13_1_*.ts                 NEW
packages/iyon-runtime/bench/PERF-12-T13.1-*.jsonl             generated evidence

plugins/app/iyon/src/app.ts                                   MODIFY only for canonical public API migration/bodyKey cleanup
plugins/app/iyon/src/view.ts                                  no composition internals or memoization logic
```

If repository architecture suggests a cleaner package boundary for generic `iyon-tui` build support, use it.

Hard ownership test:

```text
deleting plugins/app/iyon must not remove or disable retained composition support
```

The framework mechanism must stand on its own.

---

# 50. Code-generation policy

The semantic equality/helper coverage must stay synchronized with View schema evolution.

Preferred hierarchy:

```text
1. reuse an existing canonical semantic schema if it cleanly describes the needed fields
2. generate repetitive compose/equality helpers from that schema
3. keep handwritten specializations for complex semantic kinds
```

Do not force composition metadata into `view_abi.toml` if that would couple a JS-only concern to the physical native ABI unnecessarily.

If a separate small generator/spec is cleaner, use it.

CI must fail when a new public semantic View kind is added without a composition policy:

```text
reuse comparator
specialized retained policy
or explicit "always changed" fallback
```

Silent omission is not acceptable.

---

# 51. Interaction with future PERF-12v2 safe N-API transport

T13.1 must be written so this entire upper half survives:

```text
application
composition transform
ViewCompositionRoot
immutable shared View DAG
NodeId semantics
derivation metadata
PersistentSeq semantics
```

while only this lower part changes later:

```text
PERF-12 FFI materialization
        ->
PERF-12v2 safe N-API materialization/ref operations
```

This is a major design constraint.

Do not let the composition API call FFI functions directly.

Do not let composition slots store NativeRefs.

Do not let keys encode transport state.

Composition returns Views. `RetainedRootBoundary` owns native transport.

---

# 52. Anti-patterns — explicitly reject

## 52.1 App memoization

```ts
const bodyKey = JSON.stringify(...)
if (bodyKey === lastBodyKey) return lastBody
```

Not final architecture.

## 52.2 Content interning

```text
hash every BridgeViewNode payload
Map<Hash, WeakRef<View>>
```

Rejected.

Reasons:

```text
hashing cost
string/payload cost
collision machinery
cleanup machinery
turns semantic equality into global cache policy
```

## 52.3 Full-tree old/new reconciliation

```text
render fresh tree
then recursively compare against previous tree
```

Rejected.

It pays O(tree) object construction plus O(tree) comparison and undermines the point of construction-time identity.

## 52.4 User ID on every node

```ts
View.text("footer", text)
View.row("toolbar", ...)
View.spacer("gap-7", ...)
```

Rejected as normal API.

Only dynamic/reordered repeated identity needs an explicit `View.key`.

## 52.5 Key means immutable value

Never use a key as permission to skip semantic comparison.

## 52.6 Mutable semantic node

Never mutate old BridgeViewNode payload in place because a composition address stayed stable.

## 52.7 Composition-owned NativeRef

Never turn composition into a second native lifetime owner.

## 52.8 Generic reflection hot path

No:

```text
Object.keys
property-name loops
string kind dispatch
rest-array argument packs
JSON serialization
```

for common composed semantic operations.

## 52.9 Async composition builders

No Promises from `render(() => ...)`, `setView(() => ...)`, or `setContent(() => ...)` builders.

The composition context is synchronous and transactional.

---

# 53. Correctness acceptance checklist

```text
[ ] current public View semantics preserved
[ ] canonical public APIs receive retained composition on every supported consumer path
[ ] external consumer fixture needs zero compiler/plugin/feature-flag setup
[ ] Iyon app contains no special composition/compiler installation
[ ] private transform is automatically active before canonical consumer View evaluation
[ ] reference/untransformed mode exists only for tests/bench/debug, not normal consumption
[ ] composition transform changes no visual semantics
[ ] View remains immutable
[ ] BridgeViewNode remains immutable
[ ] NodeId remains semantic object identity
[ ] CompositionIdentity remains separate from NodeId
[ ] NativeRef remains separate from both
[ ] exact semantic repeat returns exact previous View
[ ] changed semantic value gets new NodeId
[ ] no recursive tree equality
[ ] no content hashing
[ ] no second semantic DAG/cache
[ ] key local to lexical site/scope
[ ] duplicate keys detected
[ ] conditional lexical sites do not shift unrelated identities
[ ] repeated unkeyed sites have deterministic occurrence identity
[ ] keyed reorder preserves item identity
[ ] failed builder aborts cleanly
[ ] failed native/host install aborts composition
[ ] successful fallback commits composition
[ ] multi-root isolation proven
[ ] root dispose releases composition refs
[ ] ViewSlot composition isolated
[ ] ScrollPane composition isolated
[ ] History/stream semantics unchanged
[ ] animation tick path unchanged
[ ] theme semantics unchanged
```

---

# 54. Structural acceptance checklist

```text
[ ] production exact no-op: 0 new semantic nodes
[ ] production exact no-op: 0 retained semantic nodes inspected
[ ] footer-only update changes only footer path + ancestors
[ ] effort-only update preserves unrelated subtrees
[ ] working visibility toggle preserves later lexical-site identities
[ ] approval visibility toggle preserves later lexical-site identities
[ ] slot-only update leaves scene root exact-reused
[ ] pane-only update leaves scene root exact-reused
[ ] no common T13.1 path full-materializes a freshly rebuilt app tree
[ ] wide set/remove/splice retains current PERF-12 asymptotics
[ ] no 100k-child composition flatten introduced
```

---

# 55. Performance acceptance checklist

```text
[ ] raw process-isolated benchmark samples retained
[ ] application render-function time measured
[ ] composition lookup/equality time measured
[ ] semantic construction time measured
[ ] native retained frontier time measured
[ ] host time measured
[ ] total op time measured
[ ] composed_auto matches manual_stable_oracle structural counters
[ ] exact no-op composition competitive with bodyKey guard
[ ] uncomposed cold construction <=3% credible regression
[ ] no common production update slower than rebuild_uncomposed
[ ] wide benchmarks unchanged asymptotically
[ ] keyed reorder avoids rebuilding unchanged item semantics
```

---

# 56. Memory acceptance checklist

```text
[ ] one committed View per live ordinary composition slot
[ ] at most one pending View per touched slot during active pass
[ ] removed positional tails released
[ ] removed keyed entries released
[ ] aborted pending state released
[ ] root dispose releases all composition refs
[ ] million-update soak does not retain historical View generations
[ ] key churn memory follows live set, not historical key count
```

---

# 57. Required tests in the actual production trace

Use the production trace as the parity inventory.

Exercise:

```text
B1 Scene root
B2 History push/freeze around B1/B3 updates
B3 working/user/tool ViewSlots
B4 tool ScrollPanes
B5 View.component(handle)
B6 existing theme setup
assistant TextStream append/seal
spinner/slot animations
Diff result rendering
```

The point is not to make B2/B5/B6 composition roots.

The point is to prove that introducing composition at B1/B3/B4 does not break their interactions with the rest of the production surface.

---

# 58. Source references — Iyon

Source freeze:

- Commit: `f665c9e3d2d69378caade3ee058a7ae1ed421d07`
- `packages/iyon-runtime/src/tui/values/view.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/tui/values/view.ts>
- `packages/iyon-runtime/src/tui/retained_dag.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/tui/retained_dag.ts>
- `packages/iyon-runtime/src/tui/runtime.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/tui/runtime.ts>
- `packages/iyon-runtime/src/tui/component.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/tui/component.ts>
- `packages/iyon-runtime/src/tui/scroll-pane.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/tui/scroll-pane.ts>
- `plugins/app/iyon/src/view.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/plugins/app/iyon/src/view.ts>
- `plugins/app/iyon/src/app.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/plugins/app/iyon/src/app.ts>
- `packages/iyon-runtime/src/virtual-modules.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-runtime/src/virtual-modules.ts>
- `packages/iyon-cli/build.ts`
  - <https://github.com/alexykn/iyon/blob/f665c9e3d2d69378caade3ee058a7ae1ed421d07/packages/iyon-cli/build.ts>
- Production boundary trace:
  - `packages/iyon-runtime/bench/PERF-12-production-boundary-trace.md`

Companion architecture document:

- `PERF-12-retained-dag-direct-ffi-handoff.md`

---

# 59. External research references

## React

- Preserving/resetting state; position/type/key identity:
  - <https://react.dev/learn/preserving-and-resetting-state>
- React Compiler automatic memoization:
  - <https://react.dev/learn/react-compiler/introduction>
- React Compiler 1.0 architecture/background:
  - <https://react.dev/blog/2025/10/07/react-compiler-1>

Key lesson used here:

```text
users should not manually preserve declarative object identity;
framework/runtime/compiler owns continuity and memoization.
```

## Jetpack Compose

- Lifecycle of composables:
  - <https://developer.android.com/develop/ui/compose/lifecycle>
- Stability / skipping:
  - <https://developer.android.com/develop/ui/compose/performance/stability>
- Strong skipping:
  - <https://developer.android.com/develop/ui/compose/performance/stability/strongskipping>

Key lesson used here:

```text
call-site identity
+ occurrence default
+ local key for repeated dynamic instances
+ retained composition
```

This is the closest conceptual precedent for T13.1.

## SwiftUI

- Demystify SwiftUI — Identity, Lifetime, Dependencies:
  - <https://developer.apple.com/videos/play/wwdc2021/10022/>

Key lesson:

```text
View value != persistent view identity;
structural identity is automatic, explicit identity is data-driven where needed.
```

## Flutter

- Inside Flutter:
  - <https://docs.flutter.dev/resources/inside-flutter>
- Flutter UI/key matching overview:
  - <https://docs.flutter.dev/ui>

Key lesson:

```text
immutable descriptions can sit above persistent retained runtime state;
unchanged identity should cut off work.
```

## Vue

- Rendering mechanism / compiler-informed VDOM:
  - <https://vuejs.org/guide/extras/rendering-mechanism>

Key lesson:

```text
source/compiler knowledge should prevent unnecessary runtime rediscovery.
```

## Bun

- Bun plugin API:
  - <https://bun.com/docs/runtime/plugins>
- `PluginBuilder.onLoad`:
  - <https://bun.com/reference/bun/PluginBuilder/onLoad>

Key implementation fact:

```text
Bun build plugins can intercept source with onLoad and return transformed contents,
which fits the current Iyon Bun.build pipeline.
```

The project remains pinned/qualified on Bun 1.4.0; verify the exact transform path in the pinned runtime as part of T13.1 rather than assuming newer Bun behavior.

---

# 60. Final architecture after T13.1

The intended result is:

```text
               ANY IYON-TUI CONSUMER
                          |
                          | ordinary public declarative API
                          v
                +-------------------+
                | View Composition  |
                |-------------------|
                | lexical SiteId    |
                | occurrence/key    |
                | previous View     |
                | exact reuse       |
                +-------------------+
                          |
                          | structurally shared immutable snapshots
                          v
                +-------------------+
                | BridgeViewNode DAG|
                | NodeId semantic   |
                | identity          |
                +-------------------+
                          |
                          | existing PERF-12 cutoff
                          v
                +-------------------+
                | RetainedRoot      |
                | Boundary          |
                +-------------------+
                          |
                +---------+---------+
                |                   |
          NativeRef hint       NodeId recovery
                |                   |
                +---------+---------+
                          |
                          v
                +-------------------+
                | Rust View DAG     |
                | PersistentSeq     |
                | retained text     |
                +-------------------+
                          |
                          v
                     layout/paint
```

And a public consumer should look boring. If builder boundaries are selected as the canonical API:

```ts
tui.render(() =>
  new Scene(
    createIyonView({
      state,
      composer,
      working,
      theme,
    }),
    history,
  )
)
```

with ordinary View code:

```ts
export function createIyonView(options: Options): View {
  return View.vertical(column => {
    column.push(workingView(options))
    column.push(approvalView(options))
    column.push(
      View.component(options.composer)
        .style(options.theme.composer)
        .styleState("iyon.agent.effort", options.effort),
    )
    column.push(
      View.text(footerText(options))
        .style(options.theme.footer),
    )
  })
  .fillWidth()
  .fillHeight()
}
```

No DAG code.

No memo key.

No NativeRef.

No NodeId.

No manual retained identity discipline.
No compiler/plugin installation.
No retained-composition feature flag.
No app-specific build wiring.

The bundled Iyon app must use exactly this generic contract. That is the success condition.

---

# 61. Final instruction to the implementation agent

**Implement PERF-12 T13.1 as mandatory, framework-owned retained composition for the generic `iyon-tui` public API. Starting from `f665c9e3d2d69378caade3ee058a7ae1ed421d07`, preserve the eager immutable `View -> BridgeViewNode` DAG, NodeId semantics, RetainedRootBoundary, NativeRef/BridgeNativeHint lifecycle, PersistentSeq wide edits, streams, History, and all T13 boundary behavior. Add the Compose-like retained composition frontend: dense compiler-assigned lexical sites, occurrence identity by default, local `View.key()` only for genuinely ambiguous repeated/reordered identity, previous committed immutable View per logical slot, immediate kind-specific semantic comparison, exact old-View return before allocation on hits, and predecessor-backed derivation only for proven existing retained families on changes. Never recursively reconcile complete old/new trees, never content-hash subtrees, never equate keys with NodeIds, never mutate semantic nodes, and never introduce another semantic/native mirror. Composition commits transactionally with the existing root install and aborts without disturbing the previous committed composition/root.**

**The source transform is private `iyon-tui` infrastructure, not an application plugin. Every supported external consumer path must activate it automatically before canonical View construction executes. A normal consumer must never install/register a composition compiler, edit a Bun plugin list or preload solely for this feature, enable a feature flag, import internal helpers, or manually memoize Views. Public API changes are allowed where they establish the correct retained lifecycle; if builder-based `Tui.render(() => Scene)`, `ViewSlot.setView(() => View)`, or `ScrollPane.setContent(() => View)` are chosen, they are canonical semantic APIs, not “optimized variants”. `View.key()` is public only because dynamic/reordered logical identity is real application information, not as an optimization switch. Untransformed execution may remain as an internal differential/benchmark/debug mode, but a documented supported consumer path that silently misses the transform is a framework defect.**

**Prove this with two independent integration targets: (1) a new external-consumer fixture that imports only public `iyon-tui` APIs and contains zero compiler/plugin/configuration setup, and (2) the real Iyon production trace, where Iyon is treated only as an ordinary framework consumer. The external fixture must demonstrate automatic exact reuse, changed-frontier behavior, conditional identity, keyed reorder behavior, ViewSlot and ScrollPane retained updates, and zero canonical known-View transform fallbacks. The production benchmark must retain the four evidence arms—current bodyKey, uncomposed/reference rebuild, manual stable-identity oracle, and automatic composition—and require the automatic arm to reproduce the oracle's structural behavior where applicable. Only after external-consumer automatic activation, exact-root reuse, conditional-site stability, keyed identity, failure atomicity, bounded memory, full production parity, and performance gates pass may the Iyon app's `bodyKey` workaround be removed. Do not fix a framework integration failure by moving identity logic into `plugins/app/iyon`.**

**Keep the entire composition frontend transport-independent so it carries unchanged into PERF-12v2 when the physical FFI transport is replaced by safe N-API. The end state is: an external developer uses the normal documented `iyon-tui` API and automatically receives structurally shared immutable DAG snapshots and PERF-12 retained-update benefits without ever knowing that a compiler, SiteIds, NodeIds, NativeRefs, derivation hints, or a retained DAG exist.**
