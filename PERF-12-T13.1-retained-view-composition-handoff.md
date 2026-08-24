# PERF-12 Tranche 13.1 — Incremental Retained Execution over Independently Retained Immutable View DAG Roots

## Automatic dirty-scope-only rendering for every supported `iyon-tui` consumer without exposing the internal DAG, a compiler, or any internal machinery

**Status:** normative implementation handoff — **REWRITTEN to the Amendment C end state**
**Normative amendment:** `PERF-12-T13.1-AMENDMENT-C-optimal-retained-dag-execution.md` (supersedes Amendments A and B). Where this document and Amendment C conflict, **Amendment C wins**; this document incorporates its requirements and points to it as `AMENDMENT-C §N`.
**Superseded in place:** the original "Retained View Composition" design (lexical SiteId source transform, globally addressed composition slots, whole-root replay with exact reuse). See §42 for the authoritative list of retired sections.
**Repository:** `alexykn/iyon`
**Branch:** `perf-refactor`
**Baseline:** `f665c9ef913a7a8eda552a385b072f25f853b359` plus local T13.1 Steps 1–3 commits (`379e1cf`, `235a9da`, `dad92b5`).
**Parent tranche:** PERF-12 T13 production boundary integration/review
**Architecture being extended:** PERF-12 Retained DAG Direct FFI
**Transport dependency:** **none** — everything here sits above the physical JS/native transport and must survive PERF-12v2 N-API unchanged (AMENDMENT-C §25).
**Framework ownership invariant:** retained execution is mandatory, invisible `iyon-tui` infrastructure for every supported consumer path; no application installs, configures, compiles, or opts into anything (AMENDMENT-C §24).

---

# 0. Executive decision (as corrected by Amendment C)

The original T13.1 correctly identified the problem — repeated ordinary declarative construction does not preserve the immutable `View -> BridgeViewNode` identity PERF-12 cuts off on — but optimized the wrong unit. It made **full replay cheap** (per-site exact-reuse checks over the whole executed program). Amendment C requires the stronger property: **clean work must not execute at all**.

The primary abstraction is therefore changed from

```text
memoized semantic construction slots replayed on every render
```

to

```text
persistent retained execution scopes
+ independently retained immutable sub-DAG roots per scope
+ tracked invalidation (State<T> reads/writes, shallow props skipping)
+ incremental host-side component/mount/layout/paint frontiers
```

Semantic construction slots remain valuable **inside a scope that genuinely executes**, but they are no longer the mechanism that discovers all changes by replaying the entire declarative program.

The slogan:

> **Do not re-execute clean scopes. Do not rebuild clean semantic nodes. Preserve immutable DAG identity all the way down — through execution, resolution, layout, and paint.**

Canonical proof shape (AMENDMENT-C §8.1, §20.1):

```text
footer dependency changes
        |
        v
mark Footer execution scope dirty
        |
        v
execute Footer body only            App/Header/Composer bodies: NOT executed
        |
        v
Footer obtains a new immutable output root only if semantics changed
        |
        v
install only Footer's retained sub-DAG root
        |
        v
invalidate only the required layout/paint frontier
```

If any phase executes, reconstructs, resolves, or remeasures clean scopes merely to discover they are unchanged, T13.1 fails its objective (AMENDMENT-C §31).

---

# 1. Why T13.1 exists

## 1.1 The gap

PERF-12 proved that once stable identity exists, unchanged subtrees cost nothing:

```text
unchanged semantic identity
    -> zero descendant semantic traversal
    -> zero payload re-transport
    -> zero native reconstruction
```

The unresolved question is how normal application code *keeps/generates* that identity as state evolves. Two answers are not equivalent:

Weak (original design): replay everything cheaply — run A, compare, reuse; run B, compare, change; run C, compare, reuse. For deep/broad trees this is still O(all executed UI sites) per update.

Strong (required): retain the execution graph too — A clean ⇒ not executed; B invalidated ⇒ executed; C clean ⇒ not executed. Only B produces a new immutable output root (AMENDMENT-C §1).

## 1.2 The production trace proves the gap is real

Production `createIyonView()` rebuilds the chrome tree from reduced state on every render, guarded by an application-computed `bodyKey`. That is effective but is exactly the wrong boundary: application code currently knows enough about renderer identity to build cache keys. The framework must own continuity. `bodyKey` remains a benchmark control until the end state is proven, then is removed (§24.3, Step 14R).

## 1.3 Do not fix this by memoizing in the app

App-layer memo tables, body-derived keys as architecture, manual View retention — rejected (original §1.3 stands). Under Amendment C even *framework-side* full replay with cheap slot hits is rejected as the final hot path (AMENDMENT-C §27).

---

# 2. Starting substrate — what exists at the working baseline

T13.1 builds on actual code, plus the repository audit recorded in AMENDMENT-C §5.2 and §32.4.

## 2.1 Eager immutable semantic DAG

`packages/iyon-runtime/src/tui/values/view.ts`: `View -> WeakMap -> BridgeViewNode` eagerly created, private NodeId, frozen Views; semantic derivation sidecars (text-layout, common-scalar, axis-set, axis-splice, grid-cell, wide overrides). Reuse exactly; never duplicate.

## 2.2 Retained bridge and boundaries

`retained_dag.ts` (identity-before-payload cutoff, BridgeNativeHint, NativeRef promotion, cold/fallback discipline, PersistentSeq paths); `runtime.ts` root boundary; `component.ts` / `scroll-pane.ts` root leases. T13.1 adds no second transport, cache, or NativeRef table.

## 2.3 Component primitives already present (AMENDMENT-C §5.2, §32.4)

```text
packages/iyon-runtime/src/tui/component.ts
  ViewSlot: stable native component identity, owns RetainedRootBoundary,
  retains current root, installs replacements via setViewRef after success,
  preserves old root until materialization succeeds
crates/iyon-tui/src/component/slot.rs
  View::component = stable ComponentSlot(ComponentId) semantic node
crates/iyon-tui/src/component/registry.rs
  persistent entries carry revision + cached immutable snapshot;
  invalidation increments revision, drops only that snapshot
crates/iyon-tui/src/scene/host.rs
  SceneHost already retains MountGraph, LayoutCache, PaintCache
crates/iyon-tui/src/scene/resolve.rs
  resolver still scans component-bearing branches (gap — see §18)
crates/iyon-tui/src/presentation/layout/measure.rs
  MeasureKey::with_component includes resolved output identity (good);
  important component-containing parents intentionally uncached (gap — see §18)
```

These are the primitives Amendment C extends. Do not invent a parallel framework (AMENDMENT-C §32.4).

---

# 3. Research conclusions

Condensed from original §3 and AMENDMENT-C §2/§26; rationale lives there.

| Source | Take | Use | Reject |
|---|---|---|---|
| React Fiber | persistent execution identity ≠ ephemeral element values; clean subtrees bail out without executing | persistent scopes, dirty-work metadata, current/WIP separation, parent-local type/position/key identity | DOM/VDOM reconciliation, lane priority complexity |
| React Compiler | memoization layer is separate from the Fiber scheduling substrate | confirms a compiler is NOT needed for the core guarantee (AMENDMENT-C §11.1) | depending on a source transform for execution identity |
| Jetpack Compose | restart scopes + tracked state reads + input skipping | restartable scopes, read tracking, `Object.is` prop skipping, local keys | Kotlin compiler plumbing, SlotTable, stability annotations |
| Flutter | immutable Widgets over persistent Elements; dirty Elements scheduled directly; sublinear layout | immutable outputs over retained execution objects; direct dirty scheduling; layout dependency cutoffs | Element-tree copying |
| Fine-grained signals | minimal observable write → exact reader invalidation | small generic `State<T>` as invalidation source only | reactive mutable semantic nodes |

Decisive compiler conclusion (AMENDMENT-C §11): Compose needs compiler-generated restart groups because `@Composable` calls look like ordinary function calls. Iyon's `defineView` makes the restart boundary an ordinary explicit runtime abstraction, so **T13.1 requires no source transform, no SiteId compiler, and no Oxc/AST dependency** (AMENDMENT-C §11.3). Source code stays literal.

---

# 4. Non-negotiable invariants

Original §4 stands except where restated below; Amendment C strengthens it.

## 4.1 Public abstraction and framework ownership

```text
[ ] every supported iyon-tui build/run path activates retained execution automatically
[ ] no plugin install, feature flag, build config, or preload in consumer code
[ ] no compiler/source transform anywhere in the supported path
[ ] users never see RetainedExecutionScope, NodeId, NativeRef, BridgeViewNode,
    BridgeNativeHint, scope ids, subscriptions, or the dirty queue
[ ] the Iyon app is a normal consumer/reference fixture, never the owner
[ ] explicit keys required only for genuinely ambiguous repeated/dynamic instances
```

## 4.2 Semantic DAG (unchanged)

Views and BridgeViewNodes remain immutable; NodeId remains exact semantic object identity; changed node ⇒ new NodeId; unchanged composed node ⇒ exact old View/BridgeViewNode/NodeId; no mutable same-NodeId payloads; no second semantic graph.

## 4.3 PERF-12 retained bridge (unchanged)

Identity cutoff before payload/child work; hints/leases transport-only; `retained_dag.ts` authoritative; cold fallback complete; PersistentSeq logarithmic; streams outside the structural bridge.

## 4.4 Execution layer

```text
[ ] execution/lifecycle metadata is private framework data, never user-visible identity
[ ] no recursive tree equality anywhere
[ ] no content hashing
[ ] failed evaluation/materialization cannot publish partial committed state
[ ] a successful semantic no-op may leave every committed root untouched
[ ] scopes store execution state + pointers to immutable outputs, never payload copies
```

## 4.5 Transport independence

No FFI/NativeRef-table assumptions above `RetainedRootBoundary`; scopes own semantic View roots and retained boundaries, never physical refs (AMENDMENT-C §25).

---

# 5. Explicit non-goals

Original §5 stands, plus (AMENDMENT-C §0/§27):

```text
- whole-application replay-and-compare as the final hot path (even with cheap slot hits)
- a general VDOM reconciler
- a signals library / state management framework / hooks system
- compiler HIR/dataflow analysis (React-Compiler-class optimization of arbitrary JS)
- content hashing / interning
- lane-priority scheduling beyond a simple dirty bit/generation
- a source transform or AST dependency of any kind
```

T13.1 has one job:

> **Turn repeated ordinary declarative UI updates into dirty-scope-only execution over structurally shared immutable DAG snapshots automatically.**

---

# 6. Identity model

Three identities remain distinct (original §6 survives with scope identity replacing CompositionAddress):

## 6.1 Execution-scope identity — logical continuity

```text
ExecutionInstance =
    parent scope
    + component type (defineView value identity)
    + ordinary position | explicit local key
```

Parent-local; survives semantic changes; created/matched by child-scope reconciliation (§16). Not user-visible. Not a NodeId. Not a NativeRef.

## 6.2 NodeId — immutable semantic identity (unchanged)

Different payload ⇒ different immutable node ⇒ different NodeId. Same payload through the same logical instance ⇒ exact previous View object.

## 6.3 NativeRef — environment-local acceleration (unchanged)

Never made semantic by scopes; leases stay owned by existing retained boundaries.

## 6.4 Example over time

```text
render #1  Footer scope {parent App, type Footer, pos 3} output NodeId 8127, NativeRef 39, text "Working"
render #2  same scope instance                           output NodeId 8134, NativeRef 44, text "Done"
render #3  same scope instance                           output NodeId 8134, NativeRef 44  (semantic no-op)
```

---

# 7. Final architecture

(AMENDMENT-C §3.)

```text
+--------------------------------------------------------------+
| Application                                                   |
| ordinary state / props · optional tracked State<T> ·          |
| user-defined defineView components                            |
+------------------------------+-------------------------------+
                               v
+--------------------------------------------------------------+
| Retained execution graph                                      |
| RetainedExecutionScope: identity · inputs · dependencies ·    |
| dirty flag · child scopes · current immutable output View ·   |
| independently retained sub-DAG boundary                       |
+------------------------------+-------------------------------+
            only dirty scopes execute
                               v
+--------------------------------------------------------------+
| Scope-local semantic construction                             |
| T13.1 monomorphic helpers: exact old View reuse on immediate  |
| equality · new immutable node on change · derivation hints    |
+------------------------------+-------------------------------+
                               v
+--------------------------------------------------------------+
| Immutable View -> BridgeViewNode DAG                          |
| each live scope owns one current immutable root               |
+------------------------------+-------------------------------+
                               v
+--------------------------------------------------------------+
| Existing PERF-12 retained boundary                            |
| NativeRef / BridgeNativeHint / PersistentSeq / root leases    |
+------------------------------+-------------------------------+
                               v
+--------------------------------------------------------------+
| Retained host dependency graph                                |
| component revision/mount topology -> layout dependencies      |
| -> paint dependencies; local swaps invalidate only the        |
| required frontier                                             |
+------------------------------+-------------------------------+
                               v
                    Rust retained DAG / frame
```

Each component scope projects into its parent as a **stable ScopeRef** — a semantic `ComponentSlot(ComponentId)` whose content is a separate immutable DAG root (AMENDMENT-C §5):

```text
parent Column
├── ScopeRef(Header scope)     NodeId SAME across footer-local updates
├── ScopeRef(Composer scope)   NodeId SAME
└── ScopeRef(Footer scope)     NodeId SAME; content root NodeId NEW when footer changes
```

The outer DAG is not reconstructed for a Footer-local change. This is why existing component indirection is strategically load-bearing, and why the host side must consume such swaps incrementally (§18).

---

# 8. Public component API — `defineView`

(AMENDMENT-C §6.)

```ts
import { defineView, View } from "iyon:tui"

const Header = defineView<{ title: string }>(({ title }) =>
  View.text(title).bold(),
)

const Footer = defineView<{ status: string }>(({ status }) =>
  View.text(status),
)

tui.render(() =>
  new Scene(
    View.vertical(column => {
      column.child(Header({ title: state.title }))
      column.child(Footer({ status: state.status }))
    }),
    history,
  ),
)
```

Requirements:

- An invocation returns a normal `View` — the **stable scope projection**, not the raw latest body output.
- Typed props; parent-local positional identity; component-type mismatch ⇒ replacement/remount.
- Local `key` support for repeated/movable instances (§16). No global IDs ever.
- Users never see scopes, slots, ids, subscriptions, or the scheduler.

## 8.1 Props comparison (skip gate)

When a parent scope genuinely executes, each child invocation performs a cheap skip check first:

```text
same key/type + same own prop-key set
+ Object.is(oldPropValue, newPropValue) for every prop
    => skip child body entirely
```

Primitives by `Object.is`; objects/functions by identity. Never deep-compare props (hidden O(tree/data) work). Props are documented as immutable snapshots; independently mutating data belongs behind `State<T>` (AMENDMENT-C §6.2, §22.6).

---

# 9. Generic tracked `State<T>`

(AMENDMENT-C §7.)

```ts
export interface State<T> {
  readonly value: T
  set(value: T): void
  update(update: (previous: T) => T): void
}
export function state<T>(initial: T): State<T>
```

- While a retained execution scope runs, a `.value` read records `(state -> scope)`.
- A write that changes by `Object.is` marks subscribed live scopes dirty and enqueues each once.
- Dependency sets are execution-dependent: old subscriptions survive until successful commit; pending reads replace them only on commit; abort retains the committed set (§21).
- Component evaluation is **pure and synchronous**: reads allowed/tracked; `State.set/update` during evaluation rejected deterministically; Promise/async returns rejected; no external mutation through framework internals (AMENDMENT-C §7.2). Writes happen outside evaluation and enqueue work for the next transaction.
- No hidden-mutation guessing: no stack inspection, implicit Proxies, deep diffs, or hashing. External/opaque state uses explicit root/props updates. This is an information boundary, not a missing optimization (AMENDMENT-C §7.3).

---

# 10. RetainedExecutionRuntime and RetainedExecutionScope

(AMENDMENT-C §4/§12.)

Not another View IR. Stores execution/lifecycle state and pointers to immutable outputs only:

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
  boundary: RetainedRootBoundary          // existing primitive, scope-owned root lease
  projection: View                        // stable semantic component/ref View shown to parent
  semanticSlots: SemanticSlot[]           // scope-local T3 slots (§11)
}
```

Semantic source of truth remains `currentOutput -> nodeForBridge(currentOutput)`.

Runtime minimum:

```ts
class RetainedExecutionRuntime {
  invalidate(scope): void   // mark dirty; enqueue once (dirty bit/generation, not Set churn)
  flush(): void             // one batched work transaction (§17)
}
```

Active-scope context: synchronous save/restore stack; nested independent boundaries nest correctly (original §14 discipline carries over; no AsyncLocalStorage; async builders rejected).

---

# 11. Scope-local semantic construction — adapt T2/T3, do not throw them away

(AMENDMENT-C §10/§17.3 — normative for the already-implemented work.)

Inside a **dirty** scope, the monomorphic helpers operate on scope-local slots with a dense cursor:

```text
View.text(value)
    -> ACTIVE_EXECUTION_SCOPE -> next semantic slot
    -> immediate semantic equality vs previous slot View?
       yes -> return exact previous View (zero allocation)
       no  -> construct new immutable View (+ derivation hint where proven)
```

Helper signature change from the Step 3 form:

```text
composeText(moduleId, siteId, value)   -->   composeText(value)
```

addressing flows through the active scope instead of module/site tables. Outside an active execution scope, helpers fall through immediately to the raw eager constructor; ordinary uncomposed `View.*` construction keeps the ≤3% regression gate (original §45.5, retained).

Why this no longer threatens the DAG: replay cost is bounded by the invalidated scope, not the application. Control-flow shifts inside a dirty scope may reduce slot reuse locally but cannot select another component instance, corrupt retained execution identity, or produce stale semantics (AMENDMENT-C §10.1/§10.2).

Retired from Step 2/3 addressing: `CompositionModuleId`, `CompositionSiteId`, `ModuleSlots`, module registration, lexical site buckets as identity. Performant primitives worth salvaging (dense arrays, epoch counters, touched-slot accounting, dispose/cleanup infrastructure, counters) move **inside** each execution scope rather than being deleted gratuitously (AMENDMENT-C §17.2).

---

# 12. Semantic equality — generated, shallow, kind-specific

Original §18 stands verbatim and remains binding:

- Monomorphic comparisons per known kind; never `JSON.stringify`/`Object.keys`/recursive equality/content hashes/reflection walks.
- Scalars and strings by direct equality; children by node identity (`old === nodeForBridge(next)` equivalent), never recursion.
- Small-child arrays O(arity); styles/decorations compared in their normalized semantic representation, never resolved theme colors; component handles by stable native component ID; Diff treated as changed unless existing retained identity applies; unknown kinds ⇒ construct new (never guess equality).

---

# 13. Avoiding allocation on exact reuse

Original §19 stands verbatim: compare raw arguments against the previous committed node's immediate fields **before** constructing anything. Candidate-then-discard is permitted only while bringing up uncommon kinds; production-hot families are pre-allocation. This is what makes `semantic construction O(changed nodes)` true inside dirty scopes.

---

# 14. Predecessor / derivation integration

Original §20 stands, scoped: predecessor relations are optimization metadata (WeakMap/private sidecar or folded into existing derivation sidecars), never semantic. Feed only proven families — text-layout, common-scalar, axis-set/splice, grid-cell — and let unsupported changes take normal `ensureNative` materialization.

---

# 15. Wide structures and PersistentSeq

Original §22 stands verbatim and is strengthened by AMENDMENT-C §20.7:

- Never scan 100k descendants to prove a one-child edit; wide axes ride base identity + sequence sidecar + operation descriptors.
- Small containers (2–20 children) compare linearly; fine.
- Fresh arbitrary arrays have an information cost — no hiding it behind hashes. Large sequences with mutation provenance use PersistentSeq/History/streams/specialized APIs, which remain authoritative and are **not** wrapped in execution scopes.
- No O(width) composition scan may enter the retained wide-edit benchmarks; asymptotics stay `O(log_32 N + inserted)`.

---

# 16. Child-scope reconciliation and keys

(AMENDMENT-C §9; supersedes original §23's slot-level key design.)

Reconciliation covers **component execution instances owned by one parent scope**, not View trees:

- Unkeyed common path: same parent + ordinal position + same component type ⇒ same scope. Type mismatch ⇒ replacement/remount. Array/index based; no Map on the common path.
- Keyed path: `same parent + same key + same type ⇒ same scope regardless of movement`. Map only for keyed/reorder contexts.
- Duplicate live keys under the same parent/repeated context: deterministic error (`TUI_COMPOSITION_DUPLICATE_KEY` heritage preserved), thrown before commit. Never alias.
- Keys assert logical instance identity, **not** semantic equality — props/dependencies still decide execution.

Public surface: `View.key(key, () => …)` remains the single explicit identity primitive for repeated/movable instances (and/or a component-call convenience if more ergonomic). Static nodes never require keys.

---

# 17. Scheduler, batching, and one authoritative batch commit

(AMENDMENT-C §12/§13.)

## 17.1 Dirty queue

Simple dirty bit/generation; a scope already dirty is never enqueued twice. Parent-before-child determinism within a batch via epoch tracking: if a parent's structural execution removes a dirty child, the child's pending work is discarded; if the parent already executed a child with newer inputs, no double execution.

## 17.2 Batching

Multiple synchronous `State.set` calls coalesce into one flush/frame: first invalidation schedules one flush; later ones join the dirty set; one evaluate + one commit. No arbitrary latency; single-set latency must reach the next supported frame.

## 17.3 Work-in-progress protocol

Closer to React WIP/Compose prepare-apply than eager per-scope publication:

```text
PREPARE JS       evaluate dirty scopes into pending outputs; collect pending
                 props/deps; stage mounts/unmounts/reorders; committed state untouched
PREPARE NATIVE   ensureNative/materialize each changed pending scope root; retain new
                 NativeRefs; compute topology patches; validate ids/generations; old roots leased
COMMIT ONCE      atomically swap changed scope roots · apply MountGraph patches ·
                 install Scene root if changed · advance revisions · invalidate layout/paint deps ·
                 promote pending props/deps/children · dispose unmounts · release superseded
                 leases · request ONE authoritative host frame
ABORT            expose none of the pending state; keep old roots/MountGraph/subscriptions;
                 release only pending/new leases
```

All failure-prone work precedes publication. If final swaps cannot be proven infallible post-validation, implement rollback or a native batch transaction (`prepare_scope_batch / commit_scope_batch / abort_scope_batch`) reusing PERF-12's temporary-lease/status discipline — do not rely on "failures are unlikely" (AMENDMENT-C §13.1). One frame per turn even with many dirty scopes (§13.2).

---

# 18. Host-side retained frontier — mount, layout, paint

(AMENDMENT-C §2.6/§5.3–§5.6 — the part that prevents moving O(N) downstream.)

Execution retention is half the system; the renderer must not rediscover the forest afterwards.

## 18.1 Incremental MountGraph

`SceneHost`'s `MountGraph` becomes authoritative incremental state:

```text
scope S output root changes
  topology inside S unchanged -> update S revision/snapshot only; no parent/sibling rescans
  topology inside S changed   -> re-resolve only S's descendant subtree; patch transactionally
```

A parent's immutable output did not change because a descendant swapped its independent root, so its child topology cannot have changed — provenance already exists to skip rediscovery. Reverse ownership must locate affected mounted subtrees in O(depth + changed mounts), not O(total mounts).

## 18.2 Retained layout dependencies

Invalidate outward from the changed component; never fingerprint by walking descendants per frame:

```text
child scope content changes
  -> remeasure changed scope under previous constraints
  geometry/layout facts unchanged -> parent layout stays valid; repaint affected region only
  geometry changed -> propagate layout dirtiness only along real dependency frontiers;
                      reuse unchanged siblings' cached measurements under unchanged constraints
```

Layout validity ties to component revision/output and retained dependencies — **never** to projection-View equality, which intentionally stays stable while content changes (AMENDMENT-C §5.6). A height change early in a long stack may legitimately require O(suffix) placement/paint; that is real output work and must be reported separately from semantic execution (§31).

Both directions must be proven: same-geometry content change leaves parent layout caches valid and matches cold rebuild; geometry change invalidates exactly the required ancestors while reusing valid sibling measurements.

---

# 19. Relationship to ViewSlot, ScrollPane, History, streams

(AMENDMENT-C §14; supersedes original §28–§30 details.)

- **ViewSlot** is precedent and likely the projection substrate. Do not expose one public ViewSlot per component; if its animation/public machinery is too expensive per scope, build a slim private `ScopeSlot` sharing the native component-root primitive carrying only: stable ComponentId/target id, current root NativeRef, revision/generation, root lease, minimal invalidation metadata. Benchmark 10/100/1,000 scopes before choosing (gate §31.6). Architecture first; per-scope overhead is an implementation problem, not a reason to return to monolithic replay.
- **ScrollPane** stays a specialized independently retained boundary; viewport/follow-end semantics are distinct; content composition identity decoupled from scroll position (original §29 holds).
- **History/streams** unchanged; retained collection models stay authoritative; no scopes wrapped around units or tokens.

---

# 20. Root API and canonical boundaries

(Original §15 + AMENDMENT-C §15.)

```ts
tui.render(() => new Scene(view, history))   // canonical recurring form
slot.setView(() => …)                          // recurring local builders retained
pane.setContent(() => …)
```

The closures are ordinary lifecycle API — they give the runtime ownership of the root execution scope, transaction lifetime, active-scope context, initial child mounts, and explicit root-props updates. Not opt-ins. Direct `render(scene)` may remain compatibility/one-shot; it must not be presented as an equivalent recurring path. Initial-construction APIs stay lean (original §15.6). History keeps B2 semantics (original §15.7).

---

# 21. Transaction semantics

Original §16 survives intact, generalized to the execution runtime:

```text
1 begin pass/work transaction from last committed state
2 activate context; execute builders synchronously
3 deactivate; prepare
4 attempt retained/cold/complete install routes
5 success  -> commit (pointer swaps, epoch advances, pending promotion, bounded deletions)
6 failure  -> abort; old composition/roots/leases remain authoritative; rethrow builder errors
```

Builder throws: abort, no host mutation, old lease untouched, rethrow. Materialization failure with successful complete fallback: commit. Commit itself effectively infallible — no user callbacks, no semantic factory calls after host mutation (original §16.4).

State written *while* a scope executes records a later dirty generation; schedule one further pass; never re-enter recursively (AMENDMENT-C §22.3). A child removed while dirty loses to the parent's structural transaction; discard its pending work and dispose subscriptions/root lease after successful commit (§22.4).

---

# 22. Memory model

(Original §25 + AMENDMENT-C §23.)

A live scope strongly retains exactly its committed execution state: current props, current output View, stable projection View, live children, live State subscriptions, scope-local slots, root lease/boundary. Removed scopes reclaim after successful commit. Aborts drop pending state and never disturb committed maps/subscriptions. Root disposal (`Tui.close`, scope dispose) releases all strong references immediately. No FinalizationRegistry as a correctness clock. Soak targets: mount/unmount 100k keyed scopes against a bounded live set; subscriber counts follow live scopes; aborted pendings reclaimed.

---

# 23. The execution layer must not become another semantic cache

Original §26 carries over: scopes store `logical instance -> immutable View pointer`, never payload copies, serialized records, NodeId→content maps, wire bytes, or native graphs. Semantic authority remains `BridgeViewNode/NodeId + NativeViewRuntime`.

---

# 24. Iyon application — normal consumer migration

## 24.1 Componentize through public APIs only

Per AMENDMENT-C §16/§30, the chrome decomposes at its natural semantic boundaries:

```ts
const Working = defineView<WorkingProps>(…)
const Approval = defineView<ApprovalProps>(…)
const ComposerChrome = defineView<ComposerProps>(…)
const Footer = defineView<FooterProps>(…)

function App(props: AppProps): View {
  return View.vertical(column => {
    column.child(Working(props.working))
    column.child(Approval(props.approval))
    column.contentMax(MAX_COMPOSER_ROWS, ComposerChrome(props.composer))
    column.child(Footer(props.footer))
  }).fillWidth().fillHeight()
}
```

Footer-tracked status change ⇒ Footer executes; App/Working/Approval/Composer do not. Structural condition change ⇒ owning scope executes; unchanged children skip. Tool cards/streaming stay on ViewSlot/ScrollPane specialized paths — stream deltas never enter the app execution graph.

## 24.2 No special treatment

No internal imports, manual DAG caches, compiler wiring, or app-specific composition primitives — ever. `plugins/app/iyon/src/view.ts` and tool renderers remain ordinary declarative code.

## 24.3 bodyKey lifecycle

Keep `lastRenderedBodyKey` as benchmark control until the Amendment C behavior is proven (footer-local update executes one body; parity with current visuals; animations advance — preserve today's `advance?.(0)` side effect in a semantically correct framework location, with a regression test that repeated exact-root renders still advance spinners/streams/headless time). Remove only at Step 14R. Never relocate identity logic into the app to paper over a framework gap.

---

# 25. Public API and consumer-experience contract

Original §32 carries over with Amendment C substitutions:

- Retained execution is part of the framework contract on every supported path; the consumer experience contains no `installX()`, no plugin arrays, no flags, no manual memoization, **and no compiler** (AMENDMENT-C §24).
- Public API changes defining lifecycle semantics are legitimate (`defineView`, `state`, `Tui.render(() => …)`, builder forms); none are marketed as "optimized variants".
- Transformation-visible-semantics rule (original §32.4) becomes runtime-visible-semantics: identical visual/layout/style/component/History/stream/event semantics; differences limited to object reuse, NodeIds, allocations, and bridge work; permitted exception: composition-specific duplicate-key errors.
- View reference equality is framework-private; audit docs/tests promising fresh allocations per call (original §32.5).
- No normal supported untransformed/unscoped mode: a supported consumer silently missing execution activation is a framework bug, caught by the external fixture (original §32.6).
- No public escape hatch by default (original §32.7).

External-consumer fixture requirement (original §13.3, extended per AMENDMENT-C §24): the fixture in `packages/tui-consumer-fixture` imports only public APIs, has zero setup, and must ultimately demonstrate **direct scoped invalidation** — e.g. `count.set(1)` updates `Counter` without executing `Static` or the parent render body.

---

# 26. Internal API stability

Private contracts stay narrow: scope registry/reconciliation, begin/end active scope context, monomorphic compose helpers (active-scope form), keyed reconciliation helper, State dependency bookkeeping, counters/debug metadata. Exported only from `@internal` modules; never from `iyon:tui`'s public TypeScript surface (original §33).

---

# 27. Failure modes

(AMENDMENT-C §22 + original §41.)

```text
component body throws            -> abort pending scope/frame tx; committed roots/deps stand
native materialization fails     -> no pending projection commits; complete-fallback success commits
write during scope execution     -> later dirty generation; one further pass; no re-entry
child removed while dirty        -> parent structural tx wins; discard pending; dispose
duplicate keys                   -> deterministic error before commit
mutable prop mutated in place    -> undetectable by design; document immutability contract
```

Failure-injection tests (original §41) apply at: NativeRef resolution, new-node materialization, retained patch, `hostRenderRef`/`setViewRef`/`setContentRef`, cold fallback, complete fallback, and now scope-batch commit/abort. For every failure: old slots current, old root Views current, old leases valid, pending refs dropped, retry correct, then success commits once.

---

# 28. Counters and instrumentation

(AMENDMENT-C §21; replaces original §35's module/site counters.)

Execution layer minimum:

```text
execution_scope_mounts / unmounts / body_calls / prop_skips
execution_scope_state_invalidations / dirty_enqueues / duplicate_invalidations
execution_scope_noop_outputs / changed_outputs / commit_aborts
execution_scope_dependency_reads / subscriptions / unsubscriptions
keyed_scope_hits / keyed_scope_mounts / keyed_scope_moves
mount_subtrees_resolved / mount_nodes_visited
layout_dependency_invalidations / layout_nodes_remeasured / layout_measure_cache_hits
paint_subtrees_invalidated
```

Existing semantic/native counters remain. Benchmarks report together: scopes called · semantic sites visited · semantic nodes allocated · native nodes inspected · constructors/derivations · host layout/paint. Never hide execution cost inside "construction".

---

# 29. Evidence probes and benchmark scenarios

## 29.1 Existing baseline (Step 1 artifact — KEEP)

`PERF-12-T13.1-composition-baseline.jsonl` + `perf12_t13_1_composition_probe.ts`: three arms (`current_body_key`, `rebuild_uncomposed`, `manual_stable_oracle`) × eight §37 production transitions through the real router, cross-arm screen parity, full provenance. Headline controls: exact no-op rebuild 19,125 ns + 10 materializers vs oracle 417 ns zero-bridge; tool slot 28 µs A/B vs 1,083 ns zero-host-mutation oracle; footer-only 10→4 constructors. These arms remain; add execution-scope arms rather than deleting evidence (AMENDMENT-C §17.1). The future retained arm plugs into the same harness.

## 29.2 Required scenarios (AMENDMENT-C §29)

```text
A  exact no-op root call                     -> exact root/scope reuse
B  one local State change in 3 siblings      -> canonical execution-frontier proof (§31.1)
C  one local State change in 1,000 siblings  -> same-geometry variant (§31.5A)
D  child prop change via parent/root update  -> parent executes; unchanged children skip
E  structural parent toggle                  -> unaffected mounted children skip; mount/unmount right
F  keyed prepend/reorder                     -> unchanged keyed bodies unexecuted
G  child output size change                  -> revision/layout propagation; sibling measurements reused
H  multi-scope batch                         -> several writes, one frame transaction
I  semantic no-op invalidation               -> dirty scope emits exact previous View; zero native work
J  B3/B4/History interaction                 -> specialized boundaries unregressed
K  wide axis operations                      -> PERF-12 wide matrix unchanged
```

All process-isolated per §44 methodology (median/p95/p99, raw JSONL, phase separation: scope evaluation · scope-local semantic construction · native frontier · mount/layout/paint · total).

---

# 30. Tests

Carried forward from original §§38–43, re-anchored:

- **Keyed/conditional (was §38–39)**: keyed reorder/insert/remove/abort semantics now at scope level (§16); conditional structure safety moves from "lexical sites don't shift" to "structural parent changes mount/unmount the right scopes and leave others' identities untouched".
- **Semantic-kind differentials (§40)**: for every supported family, untransformed construction vs composed construction inside a dirty scope — unchanged inputs give exact previous View/NodeId; changed inputs give new NodeId with output equal to the uncomposed reference.
- **Failure atomicity (§41)**: per §27.
- **Multi-root isolation (§42)**: independent runtimes/scopes never share logical state; semantic sharing only via explicitly passed identical Views.
- **Memory (§43)**: per §22 soak targets, including abort churn and root disposal.
- **New execution tests (AMENDMENT-C §20/§22)**: 1-of-3 and 1-of-1000 body-execution proofs; parent-props skip proofs; duplicate-key errors; write-during-evaluation rejection; child-removed-while-dirty; same-geometry vs geometry-change layout propagation parity against cold rebuilds.

---

# 31. Performance gates

Original gates stand unless strengthened; Amendment C adds the decisive ones.

## 31.1 Execution-frontier gate (mandatory, AMENDMENT-C §20.1)

```text
App/A/B/C each with own State dependency; change only B's State
=> App body executions 0 · A 0 · B 1 · C 0
```

Executing clean bodies "to discover they're unchanged" fails the tranche.

## 31.2 Parent-props skip gate (§20.2)

Explicit root update changing only B's props ⇒ App 1, A 0, B 1, C 0.

## 31.3 Semantic DAG gate (§20.3)

Local B update ⇒ A/C outputs and all ScopeRef NodeIds unchanged; only B's changed semantic path gets new NodeIds.

## 31.4 Native gate (§20.4)

Local B update ⇒ no root cold decode; no A/C/parent materialization; B's boundary visits its changed frontier only.

## 31.5 1-of-1000 sibling independence (§20.5 — the end-to-end proof)

Variant A (same measured geometry): dirty body executions 1; clean bodies 0; parent 0; clean semantic allocations 0; clean sibling materializations 0; mount-forest rescans 0; clean sibling remeasurements 0. Variant B (geometry changes): body/native work still exactly the child frontier; layout propagates only as dependencies require; unchanged sibling measurements reused; placement/paint suffix reported separately.

## 31.6 Projection overhead (§20.6)

Cold mount / exact no-op / local leaf / full structural update / layout / memory at 10/100/1,000 scopes. If public ViewSlot-per-component is too expensive, keep the architecture and build the slim private handle — never fall back to global re-execution.

## 31.7 Carried gates

Exact no-op JS cost ≤ bodyKey guard +10% (original §45.3); composed updates ≥10% faster than `rebuild_uncomposed` on substantially-stable traces (§45.4); cold uncomposed construction ≤3% regression (§45.5); keyed lists avoid rebuilding unchanged item semantics (§45.6); wide asymptotics unchanged (§45.7 / §20.7).

---

# 32. Implementation order from the current partial state

Supersedes original §48 Steps 4–12 entirely. Steps 1–3 dispositions per AMENDMENT-C §17; new steps are the AMENDMENT-C §18 sequence (4R–14R). Each step names its Amendment anchor.

## Step 1 — KEEP (done, commit `379e1cf`) — AMENDMENT-C §17.1

Evidence/probe work and fixture stand as-is; extend with execution-scope arms/counters; the key new benchmark is three independent scopes with one scope-local State change proving zero execution elsewhere (feeds §31.1/§31.5).

## Step 2 — ADAPT (implemented `235a9da`; adaptation owed) — AMENDMENT-C §17.2

Keep: transaction generation/epochs, current-vs-pending discipline, begin/commit/abort, active-context save/restore, touched-slot accounting where useful, dispose/cleanup infrastructure, counters. Remove/retire: module/site registration and lexical addressing as identity (§11 retirement list). Add: runtime/scope/parent-links/reconciliation/scope-local slots/dependency sets/dirty queue/batch epoch/pending mount state. Salvage performant primitives inward.

## Step 3 — KEEP AND REWIRE (implemented `dad92b5`; rewiring owed) — AMENDMENT-C §17.3

Keep: raw constructors, monomorphic comparators, exact-reuse-before-allocation, derivation selection, validation, counters. Change: addressing to active-scope form (`composeText(value)` via `ACTIVE_EXECUTION_SCOPE`), immediate fall-through outside scopes, ≤3% cold gate re-verified after rewiring.

## Step 4 — STOP AND SALVAGE (partial work halted) — AMENDMENT-C §17.4

Do not continue the TS-AST/Oxc/handwritten-scanner SiteId transform, module registration injection, Bun onLoad View-call rewriting, or transform-specific source-map machinery. Remove partial code that exists solely for lexical lowering. Salvage generic tests/fixtures (public-API coverage, fluent chains, alias/import compat, source-semantics regressions) converted to exercise the public runtime API. Historical note: the TS 7.0.2 unstable-API and Bun.Transpiler capability findings that shaped the abandoned transform design are recorded in the prior progress record (§43) and are moot for architecture.

## Step 4R — execution-scope substrate — AMENDMENT-C §18

`RetainedExecutionRuntime`, `RetainedExecutionScope`, parent/current/pending state, scope-local slot ownership, active-scope nesting, disposal, counters. Prove scope identity + props skipping synthetically. No native projection yet if that simplifies tests.

## Step 5R — generic `defineView` API — AMENDMENT-C §6/§18

Typed props, parent-local positional identity, type checks, local keys, shallow `Object.is` skip, no global IDs, no app-specific types.

## Step 6R — retained scope projection — AMENDMENT-C §5/§14/§18

Independent sub-DAG projection per live scope (existing ViewSlot primitive or slim private equivalent). Prove: child scope content change ⇒ parent semantic View identity exact.

## Step 7R — tracked `State<T>` — AMENDMENT-C §7/§18

Observable primitive + dependency collection; write ⇒ exact subscriber dirty, unrelated scopes untouched.

## Step 8R — dirty scheduler and batching — AMENDMENT-C §12/§18

One-turn batching/coalescing; duplicate invalidations join; parent-before-child epoch determinism.

## Step 9R — retained native mount/layout frontier — AMENDMENT-C §5.4–5.6/§9R/§18

MountGraph subtree patching by ComponentId; revision-driven resolver invalidation; retained layout dependency invalidation; unchanged-constraint measurement cutoffs; unchanged-sibling measurement reuse; paint invalidation no broader than required. Gate: same-size local leaf among 1,000 mounted siblings causes no rescan/remeasure of the other 999 (§31.5).

## Step 10R — multi-scope transactional commit — AMENDMENT-C §13/§18

Pending materialization + atomic component-root batch swaps + MountGraph patches + layout/paint invalidation + dependency promotion + mounts/unmounts + rollback/atomicity proof.

## Step 11R — canonical boundaries — AMENDMENT-C §15/§18

Wire `tui.render(() => …)` and recurring ViewSlot/ScrollPane builders to the execution runtime.

## Step 12R — keyed reorder and structural parent changes — AMENDMENT-C §18

Insert/remove/prepend/middle-insert/reorder/conditional mount-unmount identity proofs.

## Step 13R — production reference conversion — AMENDMENT-C §16/§30/§18

Convert Iyon via public APIs only (§24); bodyKey stays as control during evidence collection.

## Step 14R — authoritative benchmark and cleanup — AMENDMENT-C §18/§14R

After all gates pass: remove `bodyKey`; remove abandoned transform code and any unused transform dependencies; freeze the four-arm-plus-scopes benchmark record with full provenance.

Hard sequencing rule: Step 9R is mandatory before completeness claims — passing the JS execution frontier while the host rescans the forest is failure (AMENDMENT-C §2.6, §31).

## 32.1 Tranche decomposition — registry

The remaining work (Step 2/3 adaptation through Step 14R) splits into merge-request-sized tranches following the conventions of the parent experiment (`PERF-12-retained-dag-direct-ffi-handoff.md`, "Exact implementation tranches"). Do not collapse tranches into one burst. Do not begin a tranche whose predecessors' gates have not been demonstrated with committed evidence.

**Testing doctrine (binding for every tranche):**

1. **Every architectural assumption gets a test that fails if the assumption is false**, in the same tranche that introduces the assumption. Examples: "a descendant swapping its independent sub-DAG root cannot change the parent's mount topology" is proven by bounding `mount_nodes_visited` under a descendant-only swap (R6); "props skipping never confuses logical instances" is proven by the reconciliation tests (R2/R8); "abort leaves committed state authoritative" is proven by failure injection (R1/R7).
2. **Non-execution is proven by counters, never by output parity alone.** Screen equality cannot distinguish "did not execute" from "executed and got lucky". Body-call counts, semantic-allocation deltas, materializer calls, and mount/layout visit bounds are mandatory gate evidence. Scenario I (semantic no-op invalidation, §29.2) exists specifically to catch implementations that execute everything and emit identical output.
3. **Every cache or incremental shortcut requires differential parity against a cold rebuild** in the same tranche (layout measurement reuse, MountGraph patching, scope-local slots). Incremental output must equal cold-rebuild output exactly, both directions of §18.2.
4. **Failure injection is part of correctness, not hardening garnish**: the T12/T13 injection suite is extended to scope-batch transactions (R7); no exception path may silently swallow state.
5. **Memory claims get soak evidence** scaled to smoke profile per tranche and repeated at full scale in R10 (§22 targets).
6. **Benchmarks**: smoke profile only during R0–R9 (`§102.1` discipline); the authoritative full-matrix run happens exactly once, in R10, process-isolated, raw JSONL committed with provenance.
7. **Records are mandatory**: each completed tranche appends an implementation record to this document under a new `## Tranche implementation records` heading, following the parent handoff's record convention (scope, commits, review findings, summary, provenance, per-gate measured evidence, status line). A tranche without a conforming record is not complete regardless of passing code. Missing evidence for any gate row forces status PARTIAL.
8. **Gate failures stop the sequence.** Never lower a gate because later tranches depend on it. R6 failure blocks R7–R10 entirely (AMENDMENT-C §2.6: moving O(N) downstream is failure, not partial credit).

Risk-domain mapping per Review Addendum §33.7: **T13.1A** = R0–R6a (retained execution + projection root replacement), **T13.1B** = R6b+R7 (retained host frontier), integration/acceptance = R8–R10. T13.1 is not complete until both domains are individually proven — R6b deferral leaves the tranche formally PARTIAL (see Staged delivery), never redefines done.

### Registry

| Tranche | Parent steps | Exact scope | Required result before proceeding |
|---|---|---|---|
| **R0** | Steps 2-adapt, 3-rewire, 4-stop (AMENDMENT-C §17.2–§17.4) | §11 retirement list executed: module/site registration and lexical addressing removed; compose helpers rewired to active-scope form with `ACTIVE_EXECUTION_SCOPE` permanently inactive for now (pure fall-through); `composition_registry.ts` retired; any transform remnants removed; salvaged generic tests converted to public-runtime form | All T13.1 suites green with helpers inactive; **measured** cold uncomposed construction ≤3% vs pre-change baseline (record numbers); no `CompositionModuleId`/`CompositionSiteId`/site-bucket symbols remain; known perf11v4 interference failure unchanged |
| **R1** | Step 4R | §10 `RetainedExecutionRuntime`, `RetainedExecutionScope`, parent/current/pending state, scope-local slot ownership, active-scope nesting, disposal, §28 execution counters | Synthetic-driver proofs: same type+position ⇒ same instance across updates; type mismatch ⇒ remount; dirty scope executes once, clean scopes zero `body_calls` (counter-proven); abort keeps committed slots/subscriptions; disposal releases all strong refs (soak evidence) |
| **R2** | Step 5R | §8 `defineView`: typed props, parent-local positional identity, component-type checks, local-key plumbing, shallow `Object.is` prop skipping | Public-API tests: invocation returns stable projection View; props skip proven by body-call counters (unchanged props ⇒ 0 executions); fresh-object-literal props correctly NOT skipped (documents Review Addendum §33.6 contract); no way to express global IDs |
| **R3** | Step 6R | §19 scope projection: independently replaceable retained sub-DAG root per live scope (existing primitive first; slim private slot only if §31.6 measurements demand) | Child scope content change ⇒ parent semantic View identity **exact** (Bridge-level assertion, not string compare); §31.3 semantic-DAG gate green at 3-scope scale; projection-overhead baseline recorded at 10/100/1,000 scopes — **these numbers are also the go/no-go instrument that schedules R6b (see Staged delivery below)** |
| **R4** | Step 7R | §9 tracked `State<T>`: read tracking during evaluation, write⇒dirty+enqueue-once, dependency refresh on commit/abort (§21) | Purity enforcement tests (write-during-evaluation rejected, async builder rejected deterministically); subscription lifecycle (deps dropped when no longer read; abort retains old set); **§31.1 execution-frontier gate passes end-to-end**: App/A/B/C each with own State, write B ⇒ body executions App=0 A=0 B=1 C=0 by counters |
| **R5** | Step 8R + §17 | Dirty queue with dedup, one-turn batching, parent-before-child epoch determinism, JS-side WIP prepare/stage discipline | 10 synchronous writes ⇒ exactly 1 flush/commit (counter); duplicate invalidations coalesce (`duplicate_invalidations` counter); child-removed-while-dirty discarded after parent commit; scenario H smoke green; no partial committed state observable under staged-failure injection |
| **R6a** | Step 9R part 1 (**always executed** — completes R3) | Scope→projection root replacement through EXISTING host machinery only: each live scope's output swaps its retained sub-DAG root via the `ViewSlot`/`ComponentSlot`/registry revision path (stable ComponentId, revision bump, snapshot invalidation, root lease); pending multi-root materialization rides existing PERF-12 lease/prepare discipline. **No new host dependency graphs** | A local scope update renders end-to-end through the current resolver: changed scope's content root swapped, parent semantic View identity exact, old lease survives until successful replacement. Overhead at current mounted-scope counts measured and recorded as the R6b decision input |
| **R6b** | Step 9R remainder (**T13.1B**, highest risk, isolated commit, **gated: measurement trigger AND PERF-12 transport finalization** — see Staged delivery below) | §18 incremental host frontier: incremental `MountGraph` subtree patching by ComponentId, revision-driven resolver invalidation, retained layout dependency invalidation, unchanged-constraint measurement cutoffs, sibling measurement reuse, paint invalidation scoping | **§31.5 both variants at 1,000 siblings**: same-geometry ⇒ 1 body, 0 clean bodies, 0 clean materializations, 0 mount-forest rescans (`mount_nodes_visited` bounded), 0 clean sibling remeasurements; geometry-change ⇒ layout propagates only along real dependencies, sibling measurements reused; **differential parity: scoped-update output === cold-rebuild output in both variants**; layout cache-hit counters reported |
| **R7** | Step 10R (**proceeds regardless of the R6b decision**) | §17.3/§27 multi-scope transactional commit: pending materialization via existing lease paths, atomic component-root batch swaps (or proven-infallible publication), dependency promotion, mounts/unmounts, rollback. Applies `MountGraph` patches only when R6b has run | Extended failure-injection suite (scope-batch commit/abort at every stage): every injected failure leaves old roots/leases/subscriptions authoritative and pending refs released; retry succeeds; success publishes exactly one frame for N dirty scopes |
| **R8** | Steps 11R+12R | §20 canonical boundaries (`tui.render(() => …)`, slot/pane builders) wired to the runtime; §16 keyed reorder + structural parent changes (insert/remove/prepend/middle-insert/reorder/conditional mount-unmount) | Reconciliation identity tests green (keyed items survive movement with unexecuted bodies — counter-proven); scenarios E/F/J (§29.2) pass; B3/B4/History interactions unregressed; duplicate keys throw before commit |
| **R9** | Step 13R + fixture extension (AMENDMENT-C §24) | §25 external-consumer fixture extended to **direct scoped invalidation**; §24 production conversion via public APIs only, bodyKey kept armed as control | Fixture: `count.set(1)` updates `Counter` executing neither `Static` nor the parent body (counter-proven), zero setup in consumer source/config; production chrome §38 cases green with visual parity harness; animation/time side effect preserved (regression test per §24.3) |
| **R10** | Step 14R | §29 full authoritative benchmark matrix (four arms + scopes), process-isolated, full statistics/provenance; adoption decision; **only after gates pass**: bodyKey removal, dead-code/dependency cleanup | Every §37 checklist line evidenced with raw numbers; carried gates re-verified; oracle-vs-runtime divergence (§39) resolved explicitly in the analysis; report published regardless of outcome; cleanup commit shows zero behavioral delta. **If R6b is deferred:** the §31.5 gates are reported as **blocked-by-deferral — never waived or silently omitted** — alongside measured rescan costs at current mounted-scope counts and the recorded revisit trigger |

### Registry rules

- **Single source of truth:** this registry (R0–R10, with R6 split into R6a/R6b) is the only tranche numbering in force. Do not maintain parallel roadmaps; external proposals map into these rows or they do not happen.
- **Order is mandatory:** R0 precedes all runtime work (it establishes the clean fall-through floor the ≤3% gate needs); R1–R5 sequential (each builds on the prior's proven identity); **R6a executes unconditionally after R5** — scoped updates cannot reach the screen without root replacement, and it rides existing machinery only; R7 requires R3+R5+R6a but **does not block on the R6b decision** (its native work uses existing lease/prepare paths; it applies `MountGraph` patches only if R6b has run); R8–R10 sequential and likewise unblocked by R6b.
- **Once started, R6b is the architecture's point of no return:** if its gates fail, fix inside R6b or stop — proceeding on a rescan-based host would ship exactly the moved-O(N) failure Amendment C forbids.
- **R6b additionally waits for the PERF-12 transport decision:** its incremental host machinery binds to the commit/materialization boundary (leases, error surfaces, swap mechanics), which a Direct-FFI → safe N-API switch reshapes (AMENDMENT-C §25). Building R6b twice — once per transport — is prohibited waste. Order: T13.1-minus-R6b ships → its many-small-frontier traces inform the PERF-12v2 decision → R6b is designed against the FINALIZED transport. The transport-agnostic algorithmic wins (identity cutoffs, PersistentSeq wide edits, derivation hints, payload families, root leases) are protected architecture (T16 rescoping, handoff §35) and are unaffected by either decision.
- **No gate may be deferred to "the next tranche":** each row's Required result is that tranche's definition of done, evidenced in its implementation record. The single sanctioned deferral is R6b *as a whole*, per the decision gate below.
- Related tranches may share an implementation session only when every individual gate still runs and commits separately.
- **Flush/frame integration rule:** the dirty-scope flush must hook the runtime's EXISTING clock/tick/frame loop (§24.1 animation discipline; AMENDMENT-C §12.1 "smallest mechanism compatible with the existing runtime loop"). Never introduce a competing scheduler.

### Staged delivery — supported intermediate state and the R6b decision gate

Amendment C remains the final T13.1 end state (Review Addendum §33.7: not complete until both risk domains are proven). Within that arc, two delivery postures are explicitly legitimate:

**Supported intermediate end state: everything through R9 except R6b.** Because R6a rides existing host machinery, the system at this posture already delivers, with no new Rust dependency graphs:

```text
tui.render(() => …) root scope          -> whole-root replay replaced by one scope pass;
                                           exact-reuse semantics inside the root reproduce the old
                                           composition design's no-op behavior WITHOUT any transform
defineView componentization where it pays -> clean child scopes never execute (§31.1 proven)
State<T> tracked invalidation            -> update provenance is exact; batching coalesces writes
scope root swaps render end-to-end       -> existing resolver/registry paths (R6a); overhead at
                                           current mounted-scope counts measured and known
```

This posture is correct while mounted structural scope counts stay small and hot content remains behind History/streams/ViewSlot/ScrollPane — which is what the production trace shows today. It is an honest release state, not a failure: record it in the implementation record as `INTERMEDIATE (R0–R9 minus R6b)` with the measured rationale. The one property it cannot claim is §31.5 sibling independence at scale: those gates are **blocked-by-deferral and must be reported as such in every evidence table**, never waived or silently omitted.

**The R6b decision gate — two preconditions, both required:**

*Precondition 1 — measurement trigger.* Decided by R3/R6a measurements at 10/100/1,000 scopes plus real-trace mounted-scope counts, not by preference:

- If measured resolve/layout behavior at realistic and projected scope counts keeps §31.4-class costs bounded without incremental host patching (i.e., the rescan cost over few mounted scopes is noise against total frame time), **defer R6b**: ship the intermediate state, revisit when mounted-scope counts or benchmark curves cross the recorded threshold. The deferral decision, its numbers, and its numeric revisit trigger are committed; T13.1 stays formally PARTIAL until R6b runs.
- If measurements show per-update host work scaling with mounted-scope count in any real or projected trace, **run R6b now** — deferral would be shipping the moved-O(N) failure.
- The trigger cuts both ways: "layout got slower" alone does not justify R6b, and "we'd rather not" alone does not justify deferral. Only the numbers decide, and they are written down either way.

*Precondition 2 — PERF-12 transport finalization.* Even when the measurement trigger fires, R6b does not start until the Direct-FFI vs safe N-API question (PERF-12v2) is settled: the incremental host frontier binds to lease/error/swap semantics at the physical boundary, and rebuilding it across a transport switch is exactly the duplicated native work this tranche structure exists to prevent. This is not a deadlock but a deliberate staging: the shipped intermediate state produces the many-small-frontier workload profiles that make the v2 decision evidence-based, and R6b then targets one finalized boundary. If v2 lands first, R6b proceeds against it; if the trigger fires first, R6b waits — the intermediate state remains correct and fast enough by definition of the trigger threshold.
- A compiler-assisted alternative (source-level boundary detection) is explicitly rejected as a middle path: `defineView` provides the execution boundary at runtime with zero build machinery; reintroducing a transform for boundaries re-adds bootstrap fragility and dual identity sources to solve a solved problem (AMENDMENT-C §11).

Either way the decision is made once, from committed benchmark evidence, and documented in the tranche records — never re-litigated informally mid-sequence.

## Tranche implementation records

### R0 implementation record

**1. Scope statement.** Tranche R0 (Steps 2-adapt / 3-rewire / 4-stop; AMENDMENT-C §17.2–§17.4): lexical composition machinery retirement, active-scope helper re-shaping, fall-through parity proof, cold-cost gate.

**2. Commits.** `acc947f` — refactor(tui): retire T13.1 lexical composition machinery (R0). Benchmark JSONL captured at working tree `4f4e576` (pre-commit), committed in `acc947f`.

**3. Review findings.**
- Finding 1: AMENDMENT-C §17.2 says "adapt" the Step 2 runtime, but once helpers stop consulting module/site slots every structure in it (module registry, site buckets, occurrence cursors, keyed groups, pass context) has zero callers. Per the dead-code directive, retired outright rather than kept dormant; R1 rebuilds against scope semantics that do not map onto these shapes. Dual-epoch transaction discipline survives via git history (`235a9da`) and handoff §21 prose.
- Finding 2: Step 3 comparators were likewise unreachable without slots. Retired to git history (`dad92b5`); their semantic contracts (decorate() merge deltas, wide-axis bail-out, layout-patch three-shape rules) remain documented in handoff §12 for R1's scoped reimplementation.
- Finding 3: `View.__composedAxis` duplicated exactly what `View.vertical/horizontal` already do from builder callbacks — removed instead of kept for a caller that no longer exists.

**4. Implementation summary.** `composition.ts` and `composition_registry.ts` deleted; `compose.ts` reduced to 27 value-shaped fall-through helpers (`composeText(content)` etc.) whose signatures are final for the R1 scoped arm; `internal-composition.ts` reduced to the helper surface only; `__composedAxis` removed from `values/view.ts`; module/site test suite deleted; parity suite rewritten (11 tests); interleaved-arm cold-cost benchmark added with JSONL evidence.

**5. Provenance block.** Source revision at capture: `4f4e57604f284b19981cc24f03dc842b39538478` (commit `acc947f`). bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). No native addon/schema/generator involvement (pure TypeScript tranche).

**6. Gate evidence.**
- *All T13.1 suites green with helpers inactive:* `perf12_t13_1_compose.test.ts` 11 pass / 0 fail (39 assertions). The former `perf12_t13_1_composition.test.ts` no longer exists (machinery deleted).
- *Measured cold construction ≤3%:* three independent process runs of `perf12_t13_1_r0_cold_fallthrough.ts` (30 rounds × 2,000 ops/arm, interleaved A/B): composed-vs-direct median overhead **+0.46% / −0.14% / −0.29%** — inside noise; final run recorded in `PERF-12-T13.1-R0-cold-fallthrough.jsonl` (direct median 5,867 ns vs composed 5,850 ns, −0.29%). Gate pass on every run.
- *No retired symbols remain:* typecheck clean (`bun run typecheck`, 0 errors; plugins 0 errors) — no `CompositionModuleId`/`CompositionSiteId`/site-bucket/module-registration references compile anywhere.
- *Known interference failure unchanged:* full runtime battery 143 pass / 1 fail; the single failure is `perf11v4_direct.test.ts` weak-cache expiry, which passes in isolation — identical to the pre-R0 documented state (§39).

**7. Status line.** **Tranche R0 status: COMPLETE.** The lexical-composition architecture is fully retired, helpers carry their final scoped call shape with proven ≤noise fall-through cost, and the tree is clean for R1 (RetainedExecutionScope substrate).

### R1 implementation record

**1. Scope statement.** Tranche R1 (Step 4R; AMENDMENT-C §18): retained execution substrate — `RetainedExecutionRuntime`/`RetainedExecutionScope`, parent/current/pending state, scope-local semantic slot ownership wired into every compose helper, active-scope nesting, disposal, §28 execution counters, synthetic identity/props-skipping proofs. No native projection (R3), no keyed dynamics (R8), no State<T> tracking (R4).

**2. Commits.** `2ee5c88` — feat(tui): add T13.1 retained execution scope substrate (R1). Bench JSONL re-captured at `9f1cf91` (pre-commit working tree of this tranche) and committed in `2ee5c88`.

**3. Review findings.**
- Finding 1 (gate regression caught by measurement, fixed): inserting the scoped arm added one cross-module function call (`activeExecutionScope()`) per helper — cold construction regressed to **+5.9%…+7.0%**, FAILING the ≤3% gate. Root cause: per-call cost of a module-boundary function returning the context-stack top (~30 ns × ~13 helpers/op under JSC without bundle inlining). Fix: hot path now reads a stable shared cell (`executionContext.top`, property load on an imported constant object); push/pop sync it. Post-fix: **+1.48% / +0.90% / +0.47%** across three runs — gate pass with real margin. Lesson recorded: any future per-helper probe must be a property load, never a call.
- Finding 2 (real commit-coverage bug found by tests before ever shipping): children evaluated INLINE during a parent's render were not in the flush batch's processed list, so their prepared outputs/slots were never promoted — a skipped child later re-presented an uncommitted (`undefined`) output. Fix: `commitScope`/`abortScope` recurse depth-first through `pendingChildren` carrying prepared work; fresh never-committed subtrees are disposed on abort after rollback. This is exactly the class of bug §13's prepare/commit separation exists to surface.
- Finding 3: axis fall-through initially called `View.vertical(build)` for construction after already running the builder callback for comparison — double-executing user builder callbacks (side effects). `View.__composedAxis` (removed in R0 as caller-less) was reinstituted with updated docs so the builder runs EXACTLY once.

**4. Implementation summary.** `execution.ts` NEW (~600 lines): scopes (type/parent/ordinal/key/current+pending output/state/mounted/dirty), dense semantic slot tables (begin/next/commit/rollback/release), positional child reconciliation, `invokeComponent` primitive (the future defineView wrapper guts, including shallow props skip), runtime batch protocol with depth-ordered passes, generation-safe duplicate-invalidation coalescing, recursive disposal, execution counters + semantic reuse counters. `compose.ts`: scoped arm inserted into all 27 helpers (slot → immediate-equality comparator → exact previous View | fresh build + stage); comparators ported from Step 3 (`dad92b5`) unchanged in contract. `view.ts`: `__composedAxis` reinstated (@internal).

**5. Provenance block.** Source revision at capture: `9f1cf91fdb60d8b19fe8de00b440624da3b83c35` (commit `2ee5c88`). bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Pure TypeScript tranche; no native addon/schema/generator involvement.

**6. Gate evidence.**
- *Same type+position ⇒ same instance:* proven across parent re-renders (captured scope refs identical, not disposed).
- *Type mismatch ⇒ remount:* replacement scope created; predecessor disposed after commit; A.calls()==1, B.calls()==1.
- *Dirty-only execution, counter-proven:* three sibling roots, invalidate middle ⇒ body_calls delta exactly 1, siblings zero; nested variant: child-only invalidation executes exactly the child body (parent/header bodies untouched).
- *Exact reuse / no-op counters:* identical re-render returns the SAME View object; `composition_exact_view_reuses` +1, `composition_new_views` +0, `execution_scope_noop_outputs` +1. Changed payload ⇒ new NodeId + changed-output event.
- *Control-flow shift:* toggling a conditional middle child keeps output bridge-identical to cold reference construction in all three states (reuse may degrade locally — semantics never).
- *Slot lifetime:* tail shrinks on commit (4→6→abort@9→rewind to 6→grow to 5 verified via committedSlotCount); abort leaves currentOutput defined and authoritative.
- *Batch atomicity:* stable+bomber dirty in one batch, bomber throws after staging ⇒ whole batch aborted (abort counter +1), stable still presents OLD value, retry succeeds after failure clears.
- *Aborted fresh child disposal:* child created then parent body threw ⇒ child.disposed === true.
- *Props skipping (synthetic, AMENDMENT-C Step 4R requirement):* shallow-equal props skip the body (calls unchanged, `prop_skips` +1 semantics); changed props execute exactly once more.
- *Duplicate invalidation:* 3 invalidations ⇒ 1 enqueue + 2 duplicates + 1 execution.
- *Async rejection:* `TUI_EXECUTION_ASYNC_BODY` deterministic error; nothing committed.
- *Multi-runtime isolation:* same component type mounted under two runtimes ⇒ distinct scope instances; independent updates don't cross.
- *Disposal soak:* 1,000 mount/dispose cycles ⇒ mounts==1,000, unmounts ≥1,000, disposed roots have undefined output and empty slot tables.
- *Cold fall-through ≤3% (re-verified post-scoped-arm):* three runs +1.48% / +0.90% / +0.47%; final JSONL committed (direct median 5,805 ns vs composed 5,832 ns, +0.47%).
- *Battery:* typecheck clean (runtime + plugins); full suite 158 pass / 1 fail — the documented pre-existing perf11v4_direct weak-cache interference failure (passes isolated), unchanged.

**7. Status line.** **Tranche R1 status: COMPLETE.** Clean scopes provably never execute; only invalidated scopes run; exact-reuse is allocation-free inside dirty scopes; the transactional substrate holds under injected failure. Ready for R2 (defineView public API) on top of `invokeChild`/`invokeComponent`. Known limitation documented: cross-scope composite splicing after a child-only update awaits the R3 projection — body isolation does not depend on it.

### R2 implementation record

**1. Scope statement.** Tranche R2 (Step 5R; AMENDMENT-C §6/§18): public `defineView` component API — typed props, parent-local positional identity, component-type checks, local-key plumbing, shallow `Object.is` prop skipping, public export surface.

**2. Commits.** `e46b005` — feat(tui): add T13.1 public defineView component API (R2).

**3. Review findings.**
- Finding 1: the first defineView wrapper returned `invokeComponent`'s `{view, scope}` result while its type promised a bare `View`; caught immediately by embed-parity tests (`nodeForBridge` on a non-View). The public callable unwraps to `.view`; the raw primitive remains available internally for diagnostics.
- Finding 2: `ViewComponentType.render` tightened from method syntax to property-arrow syntax — method parameters are bivariant in TS, so `ViewComponent<A>` was silently assignable to `ViewComponent<B>`. Property syntax enforces contravariance.
- Finding 3: the production-chrome smoke initially placed the conditional element FIRST; toggling it shifted every later sibling's ordinal and correctly remounted them under positional identity. Test rewritten to the §8.3 trailing-conditional pattern; recorded as migration guidance for Step 13R (leading conditionals need keys once R8 lands).
- Finding 4: root-level props have no update channel yet (mount snapshots `currentProps`). This is R8/R11 territory (canonical boundary wiring); the interim test drives scalar changes via in-place field mutation, which the per-key `Object.is` comparison legitimately detects. Documented so nobody mistakes it for the final root API.

**4. Implementation summary.** `define-view.ts` NEW (public `defineView`, ~55 lines); `execution.ts`: `ViewComponent<P>` interface added (callable + render entry), invokeChild shape validation with `TUI_EXECUTION_NOT_A_COMPONENT`; `index.ts` exports `defineView`/`ViewComponent` from the public tui surface. 10-test proof suite added covering every §32.1 R2 gate row.

**5. Provenance block.** Source revision at capture: commit `e46b005` parent state; bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Pure TypeScript tranche.

**6. Gate evidence.**
- *Invocation returns stable View:* embed parity vs direct construction proven at bridge level (id-stripped deep equality) through a real mount.
- *Props skip counter-proven:* unchanged primitive props in FRESH literals ⇒ body calls unchanged across parent re-renders; changed primitive ⇒ exactly one more execution.
- *Fresh literals NOT skipped when identity-valued:* nested-object field re-executes the body; stable reference resumes skipping (both directions asserted — the §33.6 contract is now executable documentation).
- *Positional identity:* same component at two ordinals ⇒ two instances; each survives re-renders independently.
- *Local-key plumbing:* keys recorded on child scopes via `invokeComponent(..., key)`; public key API intentionally deferred to R8 with keyed reconciliation so users never see half-semantics.
- *No global IDs:* nothing in the API accepts or requires one; type-level impossible to express.
- *Deterministic errors:* outside-evaluation invocation, non-component values, async bodies — all typed framework errors.
- *Battery:* typecheck clean; 168 pass / 1 fail (documented perf11v4 interference); cold fall-through gate re-run +0.94% (≤3%).

**7. Status line.** **Tranche R2 status: COMPLETE.** The public component abstraction matches AMENDMENT-C §6 exactly: invocation-shaped, positionally identified, shallow-skipping, zero-setup. Ready for R3 (retained scope projection: stable ScopeRef views + independent sub-DAG roots).

### R3 implementation record

**1. Scope statement.** Tranche R3 (Step 6R; AMENDMENT-C §5/§14/§18): retained scope projections — independently retained sub-DAG roots per live scope behind stable component/ref views, backed by the existing ViewSlot/RetainedRootBoundary primitives; §31.3 semantic-DAG gate at 3-scope scale; projection-overhead baseline at 10/100/1,000 scopes as the R6b decision instrument.

**2. Commits.** `77351e2` — feat(tui): add T13.1 retained scope projections (R3). Bench JSONL captured pre-commit at the record state and committed with it.

**3. Review findings.**
- Finding 1 (benchmark confound caught before reporting): the first overhead run scaled HOST HEIGHT with N (`min(400,n)` rows) and showed linear leaf-update growth (~22→147 µs). Re-run with CONSTANT host geometry: leaf_update is FLAT (24.3/22.8/20.0 µs at 10/100/1,000). The original scaling was terminal layout size — a bench artifact, not scope-machinery cost. Recorded because reporting the first run would have wrongly fired the R6b trigger.
- Finding 2: installs are deduped against `projectedOutput`, so semantic-noop invalidations (scenario I) perform ZERO host mutations — verified via install counters rather than trusting setView idempotence.
- Finding 3: install ordering is deliberately BEFORE output promotion so a failed `setView` leaves old content authoritative on both JS and native sides without unwinding (ViewSlot's boundary already preserves the previous root on failure). Multi-scope install atomicity across ONE batch remains R7's deliverable (per-scope installs are individually atomic; cross-scope publication is not yet batched).
- Finding 4: detached mode (no factory) is a permanent supported configuration for tests/benchmarks — raw-output embedding per R1 semantics; production wiring always supplies a factory (R6a/R11).

**4. Implementation summary.** `execution.ts`: `ScopeProjection` interface, injectable `createScopeProjection` factory on runtime options, projection/projectedOutput fields on scopes, commit-time install-before-promote with no-op dedupe, invokeChild returns the stable projection view, dispose releases projections idempotently. Tests: 6 native-guarded proofs over headless hosts. Bench: 3-record JSONL instrument.

**5. Provenance block.** Source revision at capture: commit `77351e2` parent working tree (`19dbbc1` docs HEAD); bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Native addon exercised (ViewSlot/component registry) but not rebuilt — addon SHA unchanged from the T13-era staged artifact.

**6. Gate evidence.**
- *Child content change ⇒ parent semantic identity EXACT:* after footer-only update, parent committed output is the SAME OBJECT and the embedded ScopeRef bridge node is the SAME object (component kind) across updates.
- *§31.3 at 3 scopes:* local B update ⇒ A/C outputs identical objects, all three embedded ScopeRefs identical objects, App output identical object; B alone advances to a new content-root NodeId.
- *Scenario I:* dirty scope re-executes (body_calls +1), emits exact previous View (noop_outputs +1), installs stay at their previous count — zero native work.
- *Failure atomicity (single scope):* injected install failure ⇒ old text still authoritative on both sides, abort counter +1, recovery succeeds after the failure clears.
- *Detached compatibility:* factory-less runtime reproduces R1 raw-output embedding (embedded child remains a text node).
- *Projection-overhead instrument (R6b go/no-go):* leaf-update median FLAT across 10/100/1,000 sibling scopes (24,334 / 22,792 / 20,042 ns); cold mount amortizes (243 → 25 → 25 µs/scope); noop-all ~22–32 µs/scope (absolute value owned by R5 scheduler scrutiny). Per-update host work does NOT scale with mounted-scope count ⇒ R6b deferral posture VALIDATED by measurement; numbers committed in `PERF-12-T13.1-R3-projection-overhead.jsonl` with revisit triggers per Staged delivery.
- *Battery:* typecheck clean; 174 pass / 1 fail (documented perf11v4 interference, unchanged).

**7. Status line.** **Tranche R3 status: COMPLETE.** Scopes project as stable component refs over independent sub-DAG roots; the semantic-DAG gate passes at scale; the R6b instrument shows flat per-update cost. Ready for R4 (tracked State<T> invalidation).

### R4 implementation record

**1. Scope statement.** Tranche R4 (Step 7R; AMENDMENT-C §7/§18): generic tracked `State<T>` — read tracking during evaluation, write ⇒ dirty + enqueue-once, dependency refresh on commit/abort, purity enforcement, public export.

**2. Commits.** `3eae810` — feat(tui): add T13.1 tracked State invalidation (R4).

**3. Review findings.**
- Finding 1: §7.2 (reject writes inside bodies) vs §22.3 (writes during a running transaction schedule a later pass) reconcile cleanly by LOCATION: writes from inside any component body are rejected deterministically; writes outside bodies but inside a running flush join the standard drain loop as a later pass. Both behaviors verified.
- Finding 2 (test-design lesson, recorded for Step 13R): the abort-lifecycle test initially drove re-evaluation through the parent holder and expected the child body to run — but the props-skip gate correctly bypassed it. The dependency lifecycle must be tested via direct scope invalidation; skip-gate precedence over dirty-state is itself worth pinning (a skipped body never consumes its pending reads).
- Finding 3: the abort test also surfaced that a still-failing body re-throws when its COMMITTED subscription drives re-execution — correct behavior (invalidation ≠ success), and recovery semantics were asserted explicitly (failure cleared ⇒ next execution adopts both reads).

**4. Implementation summary.** `tracked-state.ts` NEW (~130 lines: StateSource with subscriber set, purity gate, Object.is publish discipline, `trackedStateSubscriberCount` diagnostic). `execution.ts`: committed/pending dependency sets on scopes, `linkDependency`, commit-time diff promotion (unsubscribe dropped / subscribe new), abort discards pendings only, dispose unsubscribes, `invalidateFromState` entry with dedicated counter. `index.ts`: `state`/`State` public exports. 8-test proof suite added.

**5. Provenance block.** Source revision at capture: commit `3eae810` parent working tree; bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Pure TypeScript tranche; native addon exercised headlessly in tests, not rebuilt.

**6. Gate evidence.**
- *Purity enforcement:* `counter.set(99)` inside a body throws `TUI_EXECUTION_STATE_WRITE_DURING_EVALUATION`; value unmutated; nothing committed.
- *Subscription lifecycle:* conditional read stops ⇒ writes to the dropped state leave the scope clean (0 executions) while live-dependency writes still invalidate (+1).
- *Abort retains committed set:* aborted read of `second` does NOT subscribe (write ⇒ 0 executions); committed `first` still drives re-execution (throws again while failure armed — invalidation ≠ success); after recovery both reads drive invalidation (+1 each).
- *Object.is discipline:* same-primitive and same-reference sets produce ZERO state invalidations/enqueues; `update()` transition applies end-to-end.
- *Batching:* two states read by one scope, both written synchronously ⇒ exactly one flush pass, one body execution; shared-state fan-out executes both subscribers in one pass.
- *Dispose safety:* writes after runtime dispose are silent no-ops; subscriber count follows live scopes (`trackedStateSubscriberCount` → 0).
- *§31.1 EXECUTION-FRONTIER GATE END-TO-END (through native projections):* App/A/B/C each with own State; `stateB.set("written")` alone schedules work ⇒ body executions App=0 A=0 B=1 C=0 by counters; `state_invalidations` delta exactly 1; only B's content root advances ("B=written" visible through its projection).
- *Battery:* typecheck clean; full suite 199 pass / 1 fail (documented perf11v4 interference, passes isolated); cold fall-through gate re-run −0.57% (≤3%).

**7. Status line.** **Tranche R4 status: COMPLETE.** Exact update provenance is live: one tracked write reaches exactly the scopes that read it, nothing else executes, and the frontier gate passes end-to-end through real projections. Ready for R5 (dirty scheduler hardening: batching/dedup formalization per §17).

### R5 implementation record

**1. Scope statement.** Tranche R5 (Step 8R; AMENDMENT-C §12/§17/§18): dirty-scheduler hardening — auto-scheduling with burst coalescing, commit-batch observability, parent-before-child determinism fixes, WIP prepare/commit discipline pinned under staged failure.

**2. Commits.** `21d8383` — feat(tui): harden T13.1 dirty scheduler & batching (R5).

**3. Review findings.**
- Finding 1 (real bug, found by test): a scope structurally REMOVED by an evaluating ancestor still executed from its stale queue entry, then threw during its post-dispose commit (`committing scope N without prepared output`). Fix: `isDroppedDuringPreparation()` walks the ancestor chain against reconciled `pendingChildren`; dropped scopes have their queued work discarded before running (dirty cleared, never executed). This is the SS22.4 discard semantics made executable.
- Finding 2 (real bug, found by test): when parent AND child were dirty in the same batch and the parent supplied newer child props inline, the child executed TWICE (inline + queued). Fix: inline evaluation supersedes queued work (dirty cleared at supersede), per SS12.2 "do not double-execute".
- Finding 3: middle-child removal under positional identity legitimately remounts later siblings (ordinal shift ⇒ replacement) — pinned as documented pre-R8-keys semantics in a dedicated test; keys land in R8.
- Finding 4: autoFlush defaults to TRUE (SS12.1); explicit flush pre-empts without double execution; production hosts may wire their frame loop instead via `autoFlush:false` (flush-integration rule, handoff §32.1 registry rules) — actual frame-loop hookup is R8/R11.

**4. Implementation summary.** `execution.ts`: microtask coalescing scheduler (`scheduleFlush` + `flushScheduled` guard), `autoFlush` runtime option, `execution_commit_batches` counter, double-execution supersede fix, ancestor-drop check in the flush loop. 9-test proof suite added. No public API changes.

**5. Provenance block.** Source revision at capture: commit `21d8383` working tree; bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Pure TypeScript tranche.

**6. Gate evidence.**
- *10 synchronous writes ⇒ 1 pass / 1 batch:* flush_passes delta 1, commit_batches delta 1, every body executed exactly once.
- *Duplicate invalidations coalesce:* 3 invalidations ⇒ 1 enqueue + 2 duplicates + 1 execution.
- *Scenario H:* three independent State writes ⇒ 1 pass, 1 commit batch, three bodies.
- *Removed-while-dirty discarded:* B's body never ran after structural removal; disposed=true; stable siblings untouched.
- *Middle-child removal semantics pinned:* tail remounts exactly once (mounts delta 1, calls 2 total).
- *No partial committed state under staged failure:* stable+bomber batch abort leaves old values authoritative; retry succeeds.
- *Battery:* typecheck clean; full suite 208 pass / 1 fail (documented perf11v4 interference, passes isolated); cold fall-through gate re-run +0.31% (≤3%).

**7. Status line.** **Tranche R5 status: COMPLETE.** The scheduler now guarantees single-pass batching for synchronous bursts, auto-schedules per §12.1, never double-executes superseded children, and discards doomed work structurally. Ready for R6a (production projection wiring over existing host machinery).

### R6a implementation record

**1. Scope statement.** Tranche R6a (Step 9R part 1; AMENDMENT-C §14/§18): production projection wiring through existing host machinery — `bindExecutionRuntime(tui)`, end-to-end rendering proofs over a real headless Tui, and the mounted-scope-count overhead curve as the R6b decision input. No new host dependency graphs.

**2. Commits.** `0cbf3ad` — feat(tui): wire T13.1 scope projections through the production host (R6a). Bench JSONL captured at the same working tree.

**3. Review findings.**
- Finding 1 (empirical anchor): `slot.setView` ALONE repaints the headless screen — native damage propagation is self-contained per component revision swap. This makes the R6a contract trivially strong: local updates are live with zero parent rebuild and zero scene re-render.
- Finding 2 (the §5.3 gap, now measured): once a scene embeds N projections, per-update cost grows ~2.3µs × N in the native resolve/damage path (67µs @ 10 → 214µs @ 100 → 2.36ms @ 1,000), while pre-scene-render updates stay flat (~20–40µs). Initial scene renders are themselves O(N) (0.85–9.6ms). This curve IS the R6b trigger evidence the Staged-delivery gate demanded; exact phase attribution inside Rust (resolve vs layout vs paint) is R6b planning work.
- Finding 3 (bench hygiene): two confounds were caught and removed before recording — host height scaling with N (fixed geometry), and an off-screen visibility probe replaced by a deterministic on-screen leaf update.
- Finding 4: root-level structural changes propagate through an EXPLICIT scene render at this tranche; wiring that propagation into the canonical render boundary is R8/R11.

**4. Implementation summary.** `tui-execution.ts` NEW (`bindExecutionRuntime`, private framework glue); 4-test end-to-end suite over real headless hosts; three-regime overhead instrument + JSONL. No changes to execution core semantics.

**5. Provenance block.** Source revision at capture: commit `0cbf3ad` working tree (`b92be10` docs HEAD). bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). Native addon exercised (ViewSlot/registry/host damage) but not rebuilt.

**6. Gate evidence.**
- *Local scope update renders end-to-end:* tracked write → auto-flush → projection install → screen shows new content with NO parent rebuild, NO scene re-render, NO cold fallbacks.
- *Parent semantic View identity EXACT:* root output object identical across child-local updates.
- *Old lease survives:* 8 consecutive hint-driven updates ⇒ cold_fallbacks 0, host_mutations 0 (slots are not scene hosts).
- *Root structural change:* explicit render propagates cleanly, zero fallbacks.
- *Independent regions:* left/right state writes repaint only their own regions.
- *Overhead recorded (R6b input):* see Finding 2 + committed JSONL (three regimes × N=10/100/1,000, constant host geometry, visibility asserted deterministically).
- *Battery:* typecheck clean; full suite 212 pass / 1 fail (documented perf11v4 interference, unchanged).

**7. Status line.** **Tranche R6a status: COMPLETE.** Scoped updates render live through the unmodified production resolver/registry/damage machinery; parent identity exact; leases hold; the R6b trigger curve is quantified and committed. Next per registry: R7 (multi-scope transactional commit atomicity across projections), then R8+ integration arc.

### R7 implementation record

**1. Scope statement.** Tranche R7 (Step 10R; AMENDMENT-C §13/§18): transactional retained-root publication — boundary-level prepare/commit/abort, runtime prepare-all-then-commit-all flush protocol, legacy fallback pinning. MountGraph/layout/paint frontier work explicitly deferred (R6b/R9).

**2. Commits.** `6a47b68` — feat(tui): add T13.1 transactional retained-root publication (R7).

**3. Review findings.**
- Finding 1: publish-refusal handling was redesigned mid-tranche per the planning review. Original plan treated a post-prepare publish refusal as recoverable with full unwind; final design makes it PATHOLOGICAL: after preparation holds a validated lease in the current generation, the only remaining failure input is runtime teardown, so refusal surfaces loudly instead of silently going stale. Cross-scope native atomicity against process death requires the §13.1 native batch primitive — explicitly deferred with R6b.
- Finding 2: a commit-phase throw intentionally does NOT trigger abortBatch — already-promoted scopes of the same batch would be corrupted by a rollback that runs after their promotion. Post-commit-throw state is unspecified by protocol; tests assert only that the error surfaces loudly.
- Finding 3: consumed notifications do not replay after an aborted batch — an application re-drive (any subsequent state write or explicit update) is required for recovery. Pinned by test with explanatory comment; matches handoff §41's retry semantics.
  **[SUPERSEDED by the post-R9 correctness review — see §32.3.]** The original implementation consumed dirty flags on abort, which silently discarded invalidations whose State values remain current (State.set mutates before publishing). Final invariant: an evaluation/PREPARE abort RESTORES the original batch's still-live dirty obligations to the retry queue without arming a scheduler retry; a later application re-drive (any flush trigger) drains those already-current inputs. "Re-drive" means *cause the pending transaction to be retried* — never *reproduce/rewrite every State value whose invalidation was consumed*. Commit-phase pathology remains unspecified.
- Finding 4: R3's legacy-path failure test updated — legacy projections fail at COMMIT phase (no prepare), which under the R7 protocol is pathological rather than a batch abort; counter expectation corrected from 1 to 0 with protocol documentation.

**4. Implementation summary.** `retained_dag.ts`: RootPublication type, prepareInstall / prepareFrom / publishPrepared / unwindPrepared (install recomposed, behavior identical); `component.ts`: ViewSlot.prepareSetView wrapper; `execution.ts`: ScopeProjection.preparePublication optional hook, scope.stagedPublication cell, three-phase flush (evaluate → stage → commit+promote) with staging-failure atomic unwind; `tui-execution.ts`: production factory publishes via ViewSlot.prepareSetView. 4-test proof suite added.

**5. Provenance block.** Source revision at capture: commit `6a47b68` working tree (`f6d4273` docs HEAD). bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`). No ABI changes — pure JS restructuring over existing FFI calls.

**6. Gate evidence.**
- *Atomic multi-scope publication:* A/B/C all changed, C's preparation armed to fail ⇒ zero publishes across ALL probes (install counts unchanged), every committed output shows OLD values, `execution_commit_aborts` +1; recovery (failure cleared + re-drive) publishes all three atomically.
- *Ownership:* publications delegate to ViewSlot.prepareSetView → RetainedRootBoundary.prepareInstall; slot state and lease tables never diverge (no split-brain path exists).
- *Prepare/commit split:* prepare performs no visible mutation (asserted via committed-output stability through staged failures); commit performs publish + bookkeeping only.
- *Commit-phase pathological:* injected commit failure surfaces loudly; documented as unspecified-state by protocol.
- *Legacy fallback:* projections without preparePublication still drive content swaps (installs counted).
- *ViewSlot parity:* prepare→commit ≡ setView (revision +1, content visible); prepare→abort leaves revision and content untouched; double-commit guarded.
- *Battery:* typecheck clean; full suite 216 pass / 1 fail (documented perf11v4 interference, passes isolated).

**7. Status line.** **Tranche R7 status: COMPLETE.** Multi-scope updates now have a correctness model: prepare-everything → commit-once, with prepare failures fully atomic and commit failures structurally confined to pathological teardown. Ready for R8 (canonical boundaries + keyed reconciliation).

### R8 implementation record

**1. Scope statement.** Tranche R8 (Steps 11R+12R; AMENDMENT-C §18/§32.2 addendum): canonical retained boundaries — `tui.render(() => Scene)` root execution scope, ViewSlot/ScrollPane builder overloads with transactional ownership modes, keyed child-owner groups + `View.key`, PublicationTarget split, Rust deferred component retirement, isolation gates.

**2. Commits.** WIP checkpoint (core substrate) + `33bb5c7` — feat(tui): add T13.1 canonical boundaries, keyed groups & isolation gates (R8).

**3. Review findings.**
- Finding 1 (Rust ownership audit): `take_for_host()` rejects already-attached Histories and scene replacement drops the old value without returning it — History binding is attach-once, NOT a reversible pointer. Codified as attach-once invariant with deterministic `TUI_HISTORY_ALREADY_BOUND` error on different-handle rebinding.
- Finding 2: cold fallback must NEVER paint during PREPARE (§32.2.3). Root publication uses `prepareColdInstall` via an injectable cold materializer (`setRootColdMaterializer` → tryNativeMaterialize) that decodes WITHOUT painting; commit paints once via hostRenderRef and transfers the lease.
- Finding 3: native component retirement was missing (dispose only marked alive=false; registry entries survived forever). Added deferred retirement: dispose REQUESTS retirement by ComponentId; RunningApp reaps after successful frame reconciliation proves unmount (never after failed frames).
- Finding 4: pendingKeyed WIP maps mean evaluation never mutates committed keyed state; wipActive distinguishes 'evaluated with zero keys' from 'not evaluated'.
- Finding 5: keyed invoke routes through resolveKeyedGroup (one mechanism); View.key swaps only ACTIVE_CHILD_OWNER (execution scope unchanged); key lives on the group, not child records (keyGroupOf diagnostic added).

**4. Implementation summary.** New modules: execution-context.ts (active frames, no View import), child-owner.ts (ChildOwnerState/KeyGroup); execution.ts: PublicationTarget split, OwnedBuilderRoot (transactional producer), mountExistingRoot, keyed reconcile/promote/abort walks; runtime.ts: eager runtime, render(() => Scene) canonical overload, drainExecution, history sideband, cold materializer bootstrap; component.ts/scroll-pane.ts: builder overloads + ownership modes + reentrancy guards; view.ts: View.key public API. Rust: ComponentRegistry::remove_id, PasteInterceptors::remove_id, SceneHost::is_mounted, RunningApp::host_retire_component/reap_retired_components (reaps after successful frames only), NativeViewSlot/NativeScrollPane dispose requests retirement.

**5. Provenance block.** bun 1.4.0 (34cbb9a40b4bd1bd767d134a7065e66c2432a676). Rust compiled clean but addon not rebuilt this tranche (retirement exercised in R9+ integration).

**6. Gate evidence.**
- *Keyed reorder:* identity follows keys (same scopes across passes), bodies skip on shallow-equal props (calls delta 0).
- *Content change on one key:* exactly one body executes (+1).
- *Removal:* exactly that instance's body stops; removed scopes disposed post-commit.
- *Nested/duplicate/abort:* duplicate keys reject deterministically; abort preserves committed groups; nested namespaces independent.
- *Ownership modes:* builder→direct→builder never ghosts (stale builder cannot overwrite direct values); animation takeover transactional.
- *Isolation gate (§32.2.6):* 10k TextStream appends ⇒ zero scope executions, zero new views, appends accepted at the TS boundary.
- *Lifecycle:* state.set + close + microtask ⇒ no use-after-dispose.
- *Battery:* typecheck clean; full suite 220 pass / 1 fail (documented perf11v4 interference); cold gate +0.31% (≤3%); cargo check clean for iyon-native + iyon-tui.

**7. Status line.** **Tranche R8 status: COMPLETE.** Canonical boundaries are live (root/slot/pane builders through one transaction model), keyed identity works across reorder/insert/remove/nesting, ownership modes never ghost, and retained-content engines (History/TextStream) are proven isolated from the execution graph. Ready for R9 (fixture scoped-invalidation acceptance + production conversion).

## 32.2 R8 design addendum — CORRECTED (builder boundaries, key groups, retained-content isolation)

Supersedes the earlier draft addendum after the Rust ownership audit and design review. Normative for R8.

### 32.2.1 Grounded facts — the three retained mechanisms are distinct

```text
1. Structural execution (defineView scopes)
   immutable View DAG roots; R7 prepare/publish; ScopeProjection for children.

2. History — a HYBRID:
     push(view)/freeze(unit, view)  => identity-first immutable View-DAG unit
                                       import (pushRef/freezeRef when possible);
     pushStream(TextStream)/seal    => attach a native stream handle ONCE;
     TextStream.append/update/seal  => STRINGS ONLY into the native buffer thereafter;
     StreamLayoutCache (Rust)       => width-keyed incremental reindex from the
                                       semantic-changed frontier (reindex_in_place).

3. ScrollPane — like ViewSlot: stable ComponentId, View.component(...),
   RetainedRootBoundary, setContent/setContentRef. Belongs with ViewSlot for
   R8 builder ownership.
```

CORRECTION: StreamPane/TextStream is NOT a View component — no ComponentId, no view(), no DAG root. It is a native retained-content handle attached to History by native reference. ScrollPane IS a component boundary. Do not conflate them.

CORRECTION: History is a HYBRID — do not say "History is not a semantic View DAG path": unit import IS a DAG path (once per unit); stream tokens are NOT.

### 32.2.2 Retained-content isolation gate (corrected invariants)

```text
10,000 TextStream.append() operations with mounted History+StreamPane+chrome:
  TS execution-scope reruns                    = 0
  structural JS View creations from content    = 0
  View-DAG re-imports caused by stream content = 0
  root builder executions                      = 0
  native stream mutations accepted             = yes (assert final content exact;
                                                 revisionAfter > revisionBefore —
                                                 internal coalescing is allowed)
Terminal resize:
  content layout invalidation; still zero chrome executions
MEASURED AND REPORTED, not asserted zero:
  native scene resolves, generic layout traversals, paint work,
  stream reindex work — nonzero/O(N) findings are R6b evidence, not failures
```

### 32.2.3 HARD BLOCKER — cold fallback must not paint during PREPARE

Forbidden: retained prepare refuses ⇒ `host.render(body)` during prepare ⇒ visible mutation before the transaction decides. Transport-route fallback and transaction visibility are separate concerns.

Root/slot/pane publication prepare therefore uses materialize-only paths:

```text
boundary.prepareInstall(view)            // existing retained path (R7)
  ?? boundary.prepareColdInstall(view)   // materialize-only cold path:
                                         // tryNativeMaterialize-class primitive
                                         // producing a leased ref WITHOUT publish
```

Both return the same PreparedPublication {commit, abort}. If the existing cold decoder cannot produce a retained ref without publishing for any case, ADD the small native materialize-only primitive required. Never weaken atomicity because fallback is rare.

### 32.2.4 History binding — attach-once ownership (Rust-audited)

Rust facts (`crates/iyon-native/src/tui.rs`): `take_for_host()` REJECTS an already-attached History; `set_history` moves state into the host and drops the previous scene-side History value without returning it. History models `detached → attached`, not a reversible pointer.

Codified invariant:

```text
Tui has zero or one History attachment.
First Scene with History        -> attach (validated at prepare: isDetached)
Later Scenes, same History      -> fine (no-op sideband)
Later Scenes, no History        -> keep current binding
Later Scenes, DIFFERENT History -> deterministic API error
                                  (TUI_HISTORY_ALREADY_BOUND)
```

No TypeScript rollback via `host.setHistory(old)`. Root staging tracks `pendingScene {body, history}` vs `committedScene {body, history}`; publication fires when EITHER changed (body equality alone is insufficient — the metadata-only change must republish).

### 32.2.5 PublicationTarget separate from ScopeProjection

```ts
interface PublicationTarget {
  preparePublication(output: View): PreparedPublication | undefined;
}
interface ScopeProjection extends PublicationTarget {
  readonly view: View;   // stable representation in the parent
  dispose(): void;
}
```

Scope fields: `publicationTarget?: PublicationTarget` (where output installs) and `projection?: ScopeProjection` (how represented in parent). defineView child: projection set, target = projection. Builder roots (Tui body / slot / pane): target set, projection undefined. No dummy spacer views; concepts never conflated.

### 32.2.6 OwnedBuilderRoot with committed/pending producer

Producer is part of the transaction:

```text
replace producer -> pendingProducer = X; invalidate scope
evaluate/prepare succeed -> commit: currentProducer = X
fail -> abort: pendingProducer cleared; currentProducer unchanged
```

Ownership transitions are themselves transactional (prepare-new BEFORE releasing-old):

```text
builder -> direct : prepare direct B (builder still authoritative; failure keeps builder owner + A visible) -> commit B -> THEN dispose builder scope/subscriptions
direct  -> builder: new builder authoritative only after initial evaluation+publication succeed
builder -> animation: materialize/validate/install frames first, then relinquish builder ownership (existing frame-ref materialization provides the boundary); stopAnimation obeys the same rule
```

### 32.2.7 One eager runtime per Tui; reentrancy guard

`Tui.open()` creates the RetainedExecutionRuntime eagerly; createViewSlot/createScrollPane pass it through. Raw `new ViewSlot(host)` (internal/test construction) supports direct mode only — production architecture does not bend around test constructors. Reentrancy guard: builder-boundary mutations (`setView`/`setContent`/render-builder calls) from INSIDE any evaluating/preparing/committing scope reject deterministically (internal R7 callbacks bypass the user-facing guard).

### 32.2.8 Keys: ChildOwnerState + WIP maps (no seenSerial, no monkey patch)

```ts
class ChildOwnerState {
  committedChildren: ChildRecord[] = [];
  pendingChildren: ChildRecord[] = [];
  committedKeyed?: Map<ViewKey, KeyGroup>;   // untouched during evaluation
  pendingKeyed?: Map<ViewKey, KeyGroup>;
}
class KeyGroup { readonly children = new ChildOwnerState(); }
```

Two contexts: `ACTIVE_EXECUTION_SCOPE` (unchanged by keys) and `ACTIVE_CHILD_OWNER` (= active scope's child-owner normally; swapped to group.children around a key thunk). Unkeyed invocations use strict positional ordinals and NEVER consume keyed slots; keyed groups live in their own namespace. Duplicate key in pending ⇒ immediate error. Commit promotes pendings and disposes absent committed groups; abort discards pendings/new groups, leaving committed maps literally untouched.

Module layout (kills import cycles, no monkey-patching):

```text
execution-context.ts  ActiveExecutionFrame, activeExecutionScope(),
                      activeChildOwner(), withChildOwner(), withKeyedChildOwner()
                      — imports NO View.
child-owner.ts        ChildOwnerState, KeyGroup, ChildRecord.
execution.ts          scheduler, reconciliation, transactions.
values/view.ts        View.key(k, build) -> withKeyedChildOwner(...) — plain import.
```

Documented limit: `View.key(id, () => View.text(x))` protects CHILD EXECUTION IDENTITY (component scopes inside the thunk), not raw-DAG-node identity; wrapper Views built directly in the thunk follow ordinary enclosing-scope slot behavior (correctness unaffected; local reuse may fall).

### 32.2.9 Additional mandatory gates

- **Atomic frame with root included**: root + Footer slot + ToolSlot dirty in one batch ⇒ exactly the final coherent scene observable (reuse R7 protocol; if individual publishes become externally visible mid-commit, fix with a small host batch boundary — never weaken the test).
- **Lifecycle ordering**: `Tui.close()` = block invalidations/scheduled flushes → dispose runtime roots (builder scopes, subscriptions, pending work) → close scene-root boundary → dispose native host. Slot/pane dispose = owned builder root → pending publication release → boundary close → native handle. Queued auto-flush after teardown must be impossible (test: `state.set(); tui.close(); await microtask` ⇒ no use-after-dispose).
- **History/stream regression suite**: hybrid paths preserved (unit import identity-first; stream strings direct); isolation measurements recorded per §32.2.2.

---

# 33. Files

Updated from original §49:

```text
packages/iyon-runtime/src/tui/composition.ts        ADAPT -> retained execution runtime/scopes
packages/iyon-runtime/src/tui/compose.ts            ADAPT -> active-scope addressing
packages/iyon-runtime/src/tui/composition_registry.ts  RETIRE (module/site registry)
packages/iyon-runtime/src/tui/internal-composition.ts   ADAPT (@internal facade)
packages/iyon-runtime/src/tui/values/view.ts         MODIFY only for @internal hooks as needed
packages/iyon-runtime/src/tui/runtime.ts             MODIFY (root scope wiring)
packages/iyon-runtime/src/tui/component.ts           MODIFY (scope projection / slim slot)
packages/iyon-runtime/src/tui/scroll-pane.ts         MODIFY (builder boundary only)
packages/iyon-runtime/src/tui/index.ts               MODIFY for defineView/state/View.key public API
crates/iyon-tui (resolve.rs, host.rs, measure.rs, registry.rs)  MODIFY for §18 frontier work
framework-owned transform/build-support modules      DO NOT CREATE (removed architecture)
packages/iyon-cli/build.ts                           NO transform wiring required
packages/tui-consumer-fixture/*                      EXTEND for scoped-invalidation acceptance
tests/perf12_t13_1_*.test.ts                         ADAPT/EXTEND per §30
bench/perf12_t13_1_*.ts + JSONL                      EXTEND per §29
plugins/app/iyon/src/{app,view}.ts                   MODIFY only per §24 (public APIs; no internals)
```

Ownership test unchanged: deleting `plugins/app/iyon` must not disable retained execution.

---

# 34. Code generation policy

Original §50 stands: reuse the canonical semantic schema; generate repetitive compose/equality helpers from it; handwritten specializations for complex kinds; no forcing composition metadata into `view_abi.toml`; CI must fail when a new semantic View kind ships without a composition policy (reuse comparator / specialized policy / explicit always-changed).

---

# 35. Transport independence / PERF-12v2

Original §51 + AMENDMENT-C §25, verbatim in force: everything above the transport survives N-API replacement; scopes never touch FFI functions directly, never store NativeRefs, never encode transport state in keys or scope identity; composition returns Views; `RetainedRootBoundary` owns transport.

---

# 36. Anti-patterns — explicitly reject

Original §52 items all stand (app memoization as architecture, content interning, full-tree reconciliation, per-node user IDs, key-means-immutable, mutable semantic nodes, scope-owned NativeRefs, reflection hot paths, async builders). Amendment C adds:

```text
52.10  Whole-app replay-and-compare as the final hot path
       ("cheap slot hits make it fine") — rejected; clean scopes must not execute
52.11  Moving the O(N) rediscovery downstream — jumping to one dirty JS body
       then rescanning mounts/remeasuring siblings in resolve/layout/paint
52.12  Reintroducing a source transform/compiler for identity the component
       API provides directly
52.13  Deep-diffing props or arbitrary application state
52.14  One host frame per dirty scope instead of batched transactions
```

---

# 37. Acceptance checklist

Merged from original §§53–56 and AMENDMENT-C §28; the tranche is complete only when every line holds.

**Execution**: State write targets a scope directly; clean sibling/parent bodies don't execute; props updates skip unchanged children; mount/unmount/reorder identity correct; keys local and rare.

**Semantic DAG**: per-scope outputs immutable; changed ⇒ new NodeId; unchanged scope-local nodes reuse exact Views/NodeIds; clean projections keep exact identity; no mutable payloads; no second graph.

**Native**: per-scope independently retained sub-DAG roots; local update never materializes parent/siblings; MountGraph patched incrementally, never globally rediscovered; same-geometry update remeasures nothing clean; geometry-changing update invalidates exactly the dependency frontier; revision-driven layout correctness; old lease survives until replacement; multi-scope publication atomic or infallible-after-validation; transport details private.

**Performance**: 1-of-3 and 1-of-1000 body counts exact; no O(total scopes) resolve/remeasure pass in same-geometry variant; layout/paint reported separately from semantic work; zero-allocation exact hits; cold construction ≤3%; projection overhead measured at 10/100/1000; wide asymptotics unchanged; production trace not slower in representative updates.

**Generic public API**: no app-specific primitives; no compiler/plugin configuration; no global IDs; no manual DAG retention; external fixture demonstrates direct scoped invalidation; Iyon uses only the same public APIs.

**Cleanup**: lexical SiteId architecture gone; partial transform code removed or isolated only if independently useful; no Oxc/AST dependency added for the abandoned transform; `bodyKey` removed only after gates pass.

**Memory**: per §22 soak targets all green.

---

# 38. Production trace requirements

Original §57 + AMENDMENT-C §30: exercise B1/B3/B4 plus History/stream interactions, spinner/slot animations, Diff rendering — proving composition introduction doesn't break them — plus the concrete chrome proofs: footer-only status update (one body; outer vertical and sibling ScopeRef NodeIds unchanged; topology reused; layout remeasurement only as Footer geometry requires), effort/style-state update confined to the owning scope, working visibility structural toggle skipping stable siblings, tool-card streaming on the specialized path.

---

# 39. Progress record — updated for Amendment C

*(Append-only history; prior record preserved below in condensed form with revised dispositions.)*

- **Steps 1–3 implemented pre-amendment** (`379e1cf`, `235a9da`, `dad92b5`): baseline probe + fixture (KEEP); composition runtime with dual-epoch discipline, keyed slot groups, module/site registry (machinery KEPT, module/site identity RETIRING per §11); monomorphic compose helpers incl. the two fixed bugs (composeTextAlign double-mapping; harness multi-root correction) (KEPT, rewiring owed). Step 4 explored only to the toolchain-decision point (TS 7.0.2 exposes no stable parse/print JS API; Bun 1.4 Transpiler has no AST rewriting); **no transform code was committed and none of that exploration is committed architecture** (AMENDMENT-C preamble).
- **Oracle divergence note (still standing)**: the Step 1 oracle models hand-preserved identity surviving absences; runtime semantics differ deliberately per §21/§22; surface explicitly in Step 14R analysis.
- **Known pre-existing failure**: `perf11v4_direct.test.ts` cross-file weak-cache interference (passes isolated) — out of tranche.
- **Post-amendment obligations**: §11 retirement/rewiring, §17 scheduler/WIP commit, §18 host frontier work, fixture extension to scoped-invalidation proofs, then Steps 4R–14R in order.

---

# 40. Superseded sections index

Authoritative map from the original handoff to this rewrite (for reviewers diffing histories):

```text
§0–§8 exec/why/freeze/research/invariants/non-goals/identity/architecture
      -> kept, reframed to execution scopes (here §0–§10)
§9   Dense source SiteIds ......................... REMOVED (no compiler; AMENDMENT-C §11)
§10  Site occurrence algorithm .................... REPLACED by scope-local slot cursor (§11)
§11  View.key slot-group algorithm ................ REPLACED by scope reconciliation keys (§16);
                                                     public View.key API intent survives
§12  Source transform scope/philosophy ............ REMOVED entirely (AMENDMENT-C §11/§17.4)
§13  Build/runtime integration for the transform .. REDUCED to §4.1 automatic-activation
                                                     invariant; no plugin/bootstrap machinery
§13.3 external fixture ............................. KEPT, extended (§25)
§13.5/§13.6 AST/Bun qualification .................. MOOT (historical findings archived in §39)
§14  Composition context .......................... KEPT as active-scope context (§10)
§15  Boundary API ................................. KEPT (§20)
§16  Transaction .................................. KEPT, generalized (§21)
§17  Exact reuse .................................. KEPT, scope-local (§11–§13)
§18  Equality / §19 allocation / §20 predecessor .. KEPT (§12–§14)
§21  Conditional structure (site stability) ....... SUPERSEDED by structural scope semantics (§16, §30)
§22  Wide/PersistentSeq ........................... KEPT (§15)
§23  Keyed collections ............................ SUPERSEDED (§16)
§24  Helper functions ............................. SUPERSEDED by defineView components (§8)
§25  Slot lifetime ................................ KEPT as scope memory model (§22)
§26  Not-another-cache ............................ KEPT (§23)
§27–§30 B1–B6 boundaries .......................... GENERALIZED (§19–§20, §38)
§31–§33 app/API/internals .......................... UPDATED (§24–§26)
§34  Compiler/framework failure behavior .......... TRANSFORM halves removed; atomicity kept (§27)
§35  Counters ..................................... REPLACED by AMENDMENT-C §21 set (§28)
§36–§46 probes/tests/benchmarks/gates .............. EXTENDED (§29–§31)
§47  Instrumentation examples ..................... FOLDED into §28/§29
§48  Steps 1–12 ................................... REPLACED by dispositions + 4R–14R (§32)
§49–§51 files/codegen/transport .................... UPDATED (§33–§35)
§52  Anti-patterns ................................ EXTENDED (§36)
§53–§56 checklists ................................. MERGED (§37)
§57  Production trace ............................. MERGED (§38)
§58–§59 sources .................................... SEE AMENDMENT-C §32 (primary-source list)
§60  Final architecture ........................... REPLACED (§7)
§61  Final instruction ............................ REPLACED by §42 below
§62  Progress record (first addendum) ............. REVISED into §39
```

---

# 41. Source references

Repository anchors at the working baseline: `values/view.ts`, `retained_dag.ts`, `runtime.ts`, `component.ts`, `scroll-pane.ts`, `composition.ts`/`compose.ts`/`composition_registry.ts` (adaptation targets), `virtual-modules.ts` (unchanged), fixture package; Rust: `component/slot.rs`, `component/registry.rs`, `scene/{resolve.rs, host.rs}`, `presentation/layout/measure.rs`. Primary research sources: AMENDMENT-C §32 (React Render-and-Commit/memo/Fiber reconciler, Compose lifecycle/phases/SnapshotStateObserver, Flutter Inside-Flutter/sublinear-layout). Companion documents: `PERF-12-retained-dag-direct-ffi-handoff.md`, `PERF-12-production-boundary-trace.md`, `PERF-12-T13.1-AMENDMENT-C-optimal-retained-dag-execution.md`.

---

# 42. Final instruction to the implementation agent

Implement T13.1 from the current partial local state as an **incremental retained execution system over independently retained immutable View DAG roots, with retained host-side component/layout dependency frontiers** — per AMENDMENT-C §31, which this section incorporates by reference and summarizes:

Adapt the implemented T2 runtime/transaction machinery into the retained execution runtime; keep T3's monomorphic comparators and raw constructors but make their memo slots scope-local; stop and remove the partial T4 lexical-SiteId compiler work — React targets retained Fibers without its compiler, and Iyon's explicit `defineView` boundary supplies the restart scope directly, so no source transform or AST dependency is justified. Give each mounted component scope a stable parent-local projection and an independent retained sub-DAG root so a local update replaces only that scope root. Add the minimal tracked `State<T>` for exact invalidation and shallow `Object.is` props-skipping as the second channel; local keys only for repeated/movable identity. Prepare all dirty-scope outputs, NativeRefs, mount patches, and dependency changes without mutating committed state; publish in one authoritative batch commit or prove final publication infallible after complete validation. Then finish the job host-side: incremental MountGraph patching, revision-driven resolver invalidation, retained layout dependency invalidation with unchanged-constraint cutoffs and sibling measurement reuse, and paint invalidation no broader than required.

Preserve the immutable `View -> BridgeViewNode` DAG, NodeId semantics, hint/lease separation, `RetainedRootBoundary`, PersistentSeq, History, streams, ScrollPane, and every PERF-12 transport invariant. No second semantic graph, no mutable semantic nodes, no deep diffs.

The decisive evidence is the two 1-of-1000 sibling tests: same-geometry and geometry-changing. If the implementation executes, reconstructs, resolves, or remeasures clean scopes to discover the one changed branch — at any layer — T13.1 has failed. The end state is an external developer writing boring `defineView`/`state`/`View` code on the normal documented `iyon-tui` API and receiving dirty-scope-only retained updates without ever knowing that scopes, State tracking, NodeIds, NativeRefs, layout dependency graphs, or a retained DAG exist.

### R9 implementation record

**1. Scope statement.** Tranche R9 (Step 13R; handoff §24/§32.1, AMENDMENT-C §16/§30): external-consumer fixture extension to direct scoped-invalidation acceptance gates, plus production conversion of the app plugin (and its bundled tools' live rendering) to public retained APIs only. bodyKey stays armed as benchmark control until R10.

**2. Commits.** `808423f` — feat(tui): land T13.1 R9 — external fixture scoped-invalidation gates + production defineView conversion.

**3. Review findings.**
- Finding 1 (projection factory gap): the Tui's `createScopeProjection` factory constructed `new ViewSlot(hostRef)` without a seed view, so scope projections never owned boundaries and every child prepare refused (`TUI_EXECUTION_PREPARE_REFUSED`). Fixed: factory seeds `View.spacer(0)`. This is exactly the class of silent-activation gap §32.6 declares a framework bug — caught by the external fixture, as designed.
- Finding 2 (canonical history binding vs host-fabricated histories): `Tui.createHistory()` returns the host's own History (born attached); the R7 canonical commit closure called `host.setHistory` unconditionally → `ION_INVALID_INPUT: history is already attached to a native host`. Fixed with the same `isDetached()` guard the direct path has always had; attach-once semantics preserved.
- Finding 3 (state granularity): mirroring `info` as one tracked State made Composer re-execute on footer-only status edits (Object.is on a fresh slice object notifies all readers). Split into `footerInfo` + `effort` states so effort style-state edits skip nothing else and status edits skip Composer.
- Finding 4 (test premises): `turnStarted` alone does not flip `activityVisible`; `turnFinished` with live drafts keeps activity visible (`hasPreparedTool`). Structural-toggle gate uses `toolCallPreparing`/`turnCancelled`.
- Finding 5 (pre-existing failure, out of scope): `recovery3_production.test.ts > production_successful_ls_is_green_finished` fails because long tool-result output pushes the finished card above the fold (screen findIndex −1). Bisected: fails at clean HEAD `2838475`, at `83afefc`, and earlier — predates all T13.1 work; unrelated to this tranche. Recorded here per §37 reporting duty.

**4. Implementation summary.** Fixture: `buildScopedConsumer(tui)` (public APIs only: defineView/state/View.key/Tui.render builder) + 4 acceptance tests (§31.1 frontier gate with closure counters; keyed reorder skips bodies; ownership modes never ghost incl. tracked-write-driven repaint without setView; 10k stream-append isolation gate). Production: view.ts rewritten as defineView chrome components (Working/Approval/ComposerChrome/Footer/IyonRootView) over ChromeState tracked slices; app.ts start() publishes once canonically (`tui.render(() => new Scene(App(...), history))`), applyChrome replaces whole-scene renders (spinner choreography verbatim + syncChromeStates + bodyKey bookkeeping + advance tick), live tool cards become builder-owned slots reading per-card `State<LiveTool>` via ToolCallCard component; panes/freeze/history stay on specialized channels. Framework surface: TuiRuntime.render accepts SceneProducer; ViewSlot contract gains builder setView overload; virtual module iyon:tui exports defineView/state (+d.ts). Bundled tool renderers remain pure functions invoked inside card scopes (no SDK signature change).

**5. Provenance block.** bun 1.4.0 (34cbb9a40b4bd1bd767d134a7065e66c2432a676). No Rust changes this tranche; addon unchanged.

**6. Gate evidence.**
- *Fixture §31.1 frontier gate:* status write ⇒ Header executions +1, both keyed cards +0, new header text visible after microtask drain with zero explicit renders.
- *Fixture keyed reorder:* swap with shallow-equal props ⇒ card bodies +0/+0; content change on one key ⇒ exactly that card +1.
- *Fixture ownership:* builder paint → tracked write drives repaint without setView → direct takeover paints → stale builder writes are inert.
- *Fixture isolation (§32.2.6):* 10k stream appends ⇒ zero chrome/card body executions.
- *Production scope confinement (counters):* no-op dispatch ⇒ zero bodies; configChanged ⇒ Footer=1, Composer/Working/Approval/root=0; effort cycle ⇒ Composer=1+Footer=1, others 0; preparing/cancelled structural flips ⇒ Working=1 each, siblings 0.
- *Production tool-card isolation:* progressive events execute only that card's scope (card B mount+stream leaves card A count fixed); bullet lifecycle text correct ("ls . — running").
- *§24.3 side effect:* spinner animates across no-op dispatches; native animation channel undisturbed (frame deltas observed via harness.advance).
- *Parity:* full plugins/app/iyon suite green except the documented pre-existing recovery3 viewport failure; visual contracts (steer echo, queue tail, red/green bullets, composer border) locked by existing suites pass unchanged.
- *Battery:* typecheck clean; runtime+fixture+app+plugins 357 pass / 2 fail (both pre-existing & documented: perf11v4 weak-cache interference passes isolated; recovery3 viewport failure predates T13.1).

**7. Status line.** **Tranche R9 status: COMPLETE.** The external fixture proves zero-setup scoped invalidation through the public API, and the shipped app plugin now runs its chrome and live tool cards through retained execution scopes with counter-proven sibling skipping. Remaining for R10: authoritative four-arm-plus-scopes benchmark matrix, adoption decision, bodyKey removal + lexical remnant cleanup after gates pass.

### R10 implementation record

**1. Scope statement.** Tranche R10 (Step 14R; handoff §29/§32.1/§37, AMENDMENT-C §18): authoritative four-arm-plus-scopes benchmark matrix at §102 minimums, adoption decision, oracle-divergence resolution (§39 standing note), full-scale memory soak, and — only after gates pass — bodyKey removal and dead-code cleanup. Full evidence lives in `PERF-12-T13.1-R10-report.md`; this record summarizes and pins the gate rows.

**2. Commits.** `becd990` (gate-evidence refresh at post-R9 fix revision `4e32761`, clean tree) → `513e0cf` (four-arm probe extension: retained_scopes candidate arm, 1,000 measured ops/case, cross-arm screen parity enforcement) → `dd6ae3d` (single-encode provenance line; authoritative JSONL captured clean-tree at this revision) → `a17ff2b` (bodyKey removal) → `814132c` (`disposeFreshPending` dead-code removal; battery identical before/after) → `5492290` (R10 report + memory soak).

**3. Review findings.**
- Finding 1: the retained candidate regressed one matrix case honestly (`effort_style_state`, ~+47 µs median vs `rebuild_uncomposed`): an effort change legitimately re-executes Composer AND Footer through two scoped ViewSlot boundaries versus one whole-scene install on the uncomposed arms. Bounded (~0.11 ms median, p99 ~0.21 ms); dominated by wins on every other case. Accepted and recorded rather than tuned away.
- Finding 2: post-R10 record review found the cleanup section claimed "plugins 114/114" while the app plugin suite is actually 113 pass / 1 fail — the documented pre-existing recovery3 viewport failure predating all T13.1 work (R9 Finding 5). Corrected in the report per §37's honest-failure reporting duty; reproduced at HEAD during review.
- Finding 3: post-R10 record review found the memory soak cited "console record below" without a committed raw artifact. Committed as `bench/PERF-12-T13.1-R10-memory-soak.log` from a clean tree at `5492290` and independently reproduced twice (RSS plateau 78–80 MB, subscribers exactly 64, post-dispose 0 both runs).
- Finding 4: independent review reproduction of the authoritative matrix at HEAD matched all committed medians within noise (retained arm: 375 / 58,959 / 113,333 / 45,500 ns for exact_noop/footer_only/effort_style_state/working_toggle vs committed 375 / 59,083 / 113,250 / 45,833) with parity enforced and zero cold fallbacks/host mutations on scoped cases.

**4. Implementation summary.** Probe extended with the `retained_scopes` arm: production-chrome shape decomposed into defineView components over tracked State slices, own headless session, canonical initial publish before measurement, publish-only state mirroring in each op; sampling raised to 1,000 measured ops (valid p99); records marked profile:"authoritative"; artifact written to `PERF-12-T13.1-R10-composition-authoritative.jsonl`. App: bodyKey/bodyKey bookkeeping removed — syncChromeStates' Object.is-deduped tracked-state writes are the sole update provenance; spinner advance tick preserved verbatim. Runtime: unused `disposeFreshPending` removed.

**5. Provenance block.** bun 1.4.0 (`34cbb9a40b4bd1bd767d134a7065e66c2432a676`); rustc 1.97.1; target aarch64-apple-darwin; addon SHA `c960d943…` (unchanged since T13-era staging). Matrix provenance line records git SHA `dd6ae3d…`; soak log records `5492290…`; both clean trees.

**6. Gate evidence.** Full table in `PERF-12-T13.1-R10-report.md` §§2–4. Headlines:
- *Four-arm matrix:* retained beats the shipped body-key guard on every case except effort_style_state (footer_only −12%, tool_slot −98%, pane −43%); exact_noop 375 ns with ZERO materializer calls and zero host mutations; materializer density 2,000/1,500 per window on footer/approval cases vs 10,000–10,500 uncomposed.
- *Cross-arm screen parity:* enforced per case across all four arms; mismatch throws (verified by rerun).
- *Carried cold gate ≤3%:* +1.31% at `4e32761` clean tree (`PERF-12-T13.1-R0-cold-fallthrough.jsonl`).
- *Projection overhead flat:* 31.7 / 24.7 / 22.9 µs leaf update at 10/100/1,000 scopes (R3 instrument).
- *Memory soak:* PASS — see Finding 3.
- *Battery after cleanup:* runtime 261 pass / 1 documented perf11v4 interference failure (passes isolated); app plugin 113 pass / 1 documented pre-existing recovery3 viewport failure; fixture 10/10; typecheck clean. Zero behavioral delta across `814132c`/`a17ff2b`.
- *§31.5 sibling independence:* **blocked-by-deferral** (sanctioned R6b deferral), never waived — resolver-gap curve ≈2.2 µs/mounted-scope/update committed with the N≈400 revisit trigger (report §4).

**7. Status line.** **Tranche R10 status: COMPLETE.** The authoritative matrix adopts retained execution as the supported rendering path; Step 14R cleanup (bodyKey, dead code) landed with zero behavioral delta; the sanctioned R6b deferral keeps T13.1 formally PARTIAL until R6b runs against the finalized PERF-12v2 transport or the recorded N≈400 trigger fires.

## 32.3 Post-R9 correctness review invariants

Normative. Established by the adversarial review of R0–R9 (`perf12_t13_1_abort_retry.test.ts`, `perf12_t13_1_ownership.test.ts`); every rule below has a failing-before/passing-after test. Where any earlier prose conflicts with this section, this section wins.

### Dirty is level-triggered, not edge-triggered

```text
dirty === true  <=>  "the committed output may not reflect the scope's current authoritative inputs"
```

It is NOT a consumable event notification. `State.set()` mutates the authoritative value FIRST and publishes second; render aborts never roll State values back. Therefore aborting rendering cannot consume the invalidation obligation that made the work necessary:

```text
failure
   ↓
committed frame stays old · State values stay new
dirty obligations survive · queue retains retry work
```

### Evaluation/PREPARE abort preserves the whole original dirty batch

At batch acquisition, snapshot `retryObligations = batch.filter(s => !s.disposed && s.dirty)`. After a phase-1 (evaluation) OR phase-2 (PREPARE) failure: roll back all WIP/publications, then restore every still-live scope from the snapshot — `dirty = true`, queued exactly once. This includes processed scopes, unreached scopes, superseded-inline scopes, and structurally-dropped-in-WIP scopes: no commit happened, so the previous committed world remains authoritative while every input stays current.

### Abort never arms a scheduler retry; explicit drains consume pending tokens

- The abort path itself must NOT call `scheduleFlush()` — otherwise a persistently throwing component becomes an infinite microtask loop.
- An already-scheduled microtask MUST be invalidated when a synchronous `flush()` starts (`flushScheduled = false` + generation bump). Without this, `invalidate(); flush() /* throws */` would be silently auto-retried by the stale microtask.
- The schedule token is a GENERATION counter, not a bare boolean, so an obsolete callback can be recognized and dropped.

The resulting distinction:

```text
retry obligation preserved      YES
automatic retry after failure   NO
```

Recovery is any later re-drive: an explicit `flush()`, `runtime.update(scope)`, a later State write (which schedules normally), or any other scheduling trigger. A duplicate invalidation of an already-dirty scope counts itself as duplicate, enqueues nothing, but DOES ensure a future flush remains armed.

### Commit-phase failure stays pathological

R7's contract stands: a phase-3 throw is teardown-class, leaves unspecified state, surfaces loudly via `pathologicalCommitFailures`, and does NOT use retry restoration (the previous frame is no longer provably authoritative).

### Producer rollback cancels only its own obligation

`OwnedBuilderRoot.replaceProducer` restores the previously authoritative producer on failure and cancels the retry obligation introduced SOLELY by that attempt (`runtime.cancelRetry(scope)` — a narrow internal op, not public dirty manipulation). A pre-existing dirty obligation survives: restored producer + still-newer State values is exactly what the retry should render.

### Props skip does not consume independent dirtiness

Parent re-invocation with UNCHANGED props skips the child inline but must leave a queued independent (State-driven) obligation intact: the child still executes exactly once from its queue entry later in the same pass. Changed props instead supersede the queued entry and execute exactly once with new props + latest State. Both directions are pinned by test.

### Double-reach commits once

A child can be reached twice in one pass: recursively under its committing parent AND as its own queue entry (independently dirty, skipped inline). The first commit wins; the second is a no-op (`commitBatch` dedupes with a per-batch committed set) — never a "without prepared output" protocol failure.

### Ownership changes are semantic even when pixels do not change

`render(scene)` taking over from `render(() => scene)` must dispose the canonical builder root EVEN WHEN body+History identity makes native rendering a pixel no-op (`renderDirect`'s early branch calls `disposeRootBuilder()` before returning). Leaving it subscribed lets the old builder ghost-update the screen later — precisely the ownership-mode ghost R8 eliminates.

Direct takeover of a builder-produced scene FREEZES projected components rather than vanishing them: JS scopes/subscriptions die immediately while native ComponentIds referenced by the still-mounted direct snapshot stay registered until a later successful reconciliation proves them unmounted (deferred retirement). This is why the takeover is safe even when the body contains child projections.

### History sideband participates independently of body identity

Same `View` body plus first History attachment must still publish (`needsPublication()` observes `stagedHistory !== boundHistory`). Different History after binding remains the deterministic attach-once error (`TUI_HISTORY_ALREADY_BOUND`). A third same-body/same-History render is a true no-op.

### Benchmark provenance is clean-tree only

Authoritative JSONL evidence records `git rev-parse HEAD`; benchmarks execute whatever is in the working tree. Evidence is therefore valid ONLY from a clean worktree at the commit it names. Sequence: correctness fixes → full tests → commit → clean worktree → run authoritative benches → commit evidence separately. Ad-hoc dirty-tree runs are exploratory and never committed as gate evidence.
