# 30 — composition-root

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: public TypeScript authoring through native structural publication, native frame scheduling, Rust layout, physical paint, and teardown.
- Required context read:
  - `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - `docs/architecture/atlas-4355c02/README.md`
  - `PRE-V5-ARCHITECTURE-REPORT.md`
  - `AGENTS.md`
- Parent-added atlas documentation was treated as investigation metadata only. It was not treated as source-baseline implementation.

### Scope boundary

This report follows the actual current generic framework path:

```text
public TypeScript authoring
    → immutable semantic View / Scene values
    → retained execution root, if using a producer
    → TypeScript retained structural materialization
    → generated N-API View ABI
    → Rust NativeViewRuntime and NativeRef table
    → NativeTuiHost desired-root publication
    → Rust TuiHost frame preparation
    → SceneHost resolution
    → layout measurement / placement
    → physical paint
    → headless or real terminal presentation
    → visible-frame commit
    → retained-resource and host teardown
```

The framework ownership boundary from `AGENTS.md` is consequential here: the code inspected is generic terminal/UI infrastructure. No product/application-specific authoring path is present in this repository.

### Evidence status

- This is a static source investigation.
- No build, test suite, benchmark suite, terminal session, or live headless render was executed for this report.
- Therefore:
  - call chains below are source-proven;
  - identity and lifetime behavior are inferred from the implementation;
  - no claim is made that a particular frame was observed at runtime in this investigation;
  - existing counters, tracing hooks, and test harnesses are documented as available observability, not as executed evidence.

### Important current-state observation

The canonical TypeScript `Tui.render(builder)` route does **not** directly call the standalone `renderExactRoot`/`hostRenderRef` path. It constructs a deferred `RetainedRootBoundary` with `deferHostCommit: true`; root publication uses `setDesiredViewRef`, and the native environment later prepares and presents the frame.

The exact-root `hostRenderRef` fast path remains implemented for other retained boundaries and is reachable through `RetainedRootBoundary` in non-deferred mode, but a source search over `packages/iyon-tui/src` found no production caller of `renderExactRoot` other than its own boundary wrapper:

- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1391-1463`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:2021-2044`
- `packages/iyon-tui/src/runtime/runtime.ts:451-548`

This distinction matters when describing the real composition-root trace.

---

## 1. Responsibility and structure

### 1.1 Public TypeScript authoring surface

The public package exports the authoring nouns from `packages/iyon-tui/src/index.ts`:

| Symbol | Source | Responsibility |
|---|---|---|
| `Tui` | `runtime/runtime.ts` | Open/own a native host, render scenes, flush, resize, route input/output, create controls, close/exit |
| `TuiRuntime` | `runtime/runtime.ts` | Public runtime contract |
| `View` | `api/view/view.ts` | Immutable semantic view value |
| `Scene` | `api/view/scene.ts` | Semantic body plus optional `History` sideband |
| `SceneProducer` | `api/view/scene.ts` | `SceneContract` or synchronous producer closure |
| `defineView` | `composition/define-view.ts` | Defines a callable retained component |
| `state` / `State<T>` | `composition/tracked-state.ts` | Observable invalidation source |
| `ViewState` | `api/view/retained-state.ts` | Host-owned retained presentation state |
| `History` | `api/controls/history.ts` | Ordered scrollback/history sideband |
| `ContentPort` and content types | `api/content/retained.ts` | Host-bound content attachment |
| `ViewSlot` / `ScrollPane` | `api/controls/view-slot.ts`, `api/controls/scroll-pane.ts` | View-bearing native control boundaries |
| presentation types | `api/presentation/*` | Generic styles, themes, borders, colors, attributes |
| structural types | `api/view/view.ts` | Axis/grid tracks, wrapping, alignment, overflow |
| `AppHarness` | `testing/index.ts` | Headless deterministic wrapper around `Tui` for tests |

The index exports are intentional and narrow. Internal execution and transport machinery is not re-exported as ordinary public API.

### 1.2 Semantic authoring layer

`packages/iyon-tui/src/api/view/view.ts` owns:

- semantic `View` node identity;
- child relationships;
- semantic node kinds;
- immutable normalized text/presentation data;
- retained derivation hints;
- attachment references;
- layout child and grid representations.

The file explicitly describes the semantic tree as the declaration and says structural transport is absent from the `View` layer (`view.ts:1-7`).

The main semantic kinds materialized by the retained transport are:

- `spacer`
- `contentHost`
- `row`
- `column`
- `grid`
- `text`
- `diff`
- `hanging`
- `container`
- `clamp`
- `contentMax`
- `component`
- `decorated`

The semantic value is immutable/frozen. `View` methods produce a new semantic node identity for mutations:

- `View` constructor and identity installation: `view.ts:245-271`
- `withSemanticIdentity` / `withSemanticUpdate`: `view.ts:221-242`
- modifiers and constructors: `view.ts:274-530`

Node identity is allocated from a global symbol-backed counter:

- `view.ts:145-158`

The identity is a semantic `NodeId`; it is not the native `NativeRef`, the retained execution-scope identity, or a component handle identity.

### 1.3 Retained composition layer

The composition subsystem is in `packages/iyon-tui/src/composition/`:

| File | Responsibility |
|---|---|
| `define-view.ts` | Public callable component wrapper |
| `execution.ts` | Retained scopes, reconciliation, dependency tracking, scheduling, prepare/commit/abort |
| `execution-context.ts` | Active execution scope and keyed child-owner context |
| `child-owner.ts` | Positional and keyed child ownership |
| `compose.ts` | Semantic slot reuse and retained construction of `View` values |
| `tracked-state.ts` | State sources and invalidation |
| `publication.ts` | Structural publication contracts |
| `persistent-seq.ts` | Persistent axis/grid sequence implementation |

The retained execution root is a `RetainedExecutionScope`. Each scope separately owns:

- execution identity;
- current/pending props;
- current/pending semantic output;
- current/pending dependencies;
- semantic slots;
- child scopes;
- publication target;
- optional native projection.

`RetainedExecutionScope` fields and lifecycle are defined at `execution.ts:109-218`.

The distinction between identities is explicit in the source:

```text
ExecutionScope identity ≠ NodeId ≠ physical resource
```

(`execution.ts:105-108`).

### 1.4 TypeScript native transport layer

Relevant files:

| File | Responsibility |
|---|---|
| `transport/native/addon.ts` | Private TypeScript declaration of native host/resource contracts and canonical artifact loading |
| `transport/native/artifact.ts` | Native artifact resolution |
| `transport/native/resources.ts` | Host/environment/resource lookup |
| `transport/structural/native-view-abi.ts` | ABI session bootstrap, generated-call wrappers, retained transient helpers |
| `transport/structural/retained-dag.ts` | Semantic-node to NativeRef correspondence and retained materialization |
| `transport/abi/structural/generated/view_calls.ts` | Generated typed N-API calls |
| `transport/abi/structural/generated/view_abi.ts` | Generated ABI method contract |
| `runtime/wake-broker.ts` | Environment-level pending host queue and frame barrier |

`addon.ts` deliberately describes itself as a private native contract for the generic TUI package (`addon.ts:1-6`).

### 1.5 Native addon and Rust runtime

Relevant native-addon files:

| File | Responsibility |
|---|---|
| `crates/iyon-tui-native/src/tui.rs` | N-API classes such as `NativeTuiHost`, `NativeHistory`, `NativeViewSlot`, `NativeScrollPane`, and N-API forwarding |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Native View runtime, NativeRef table, semantic cache, generated ABI implementations |
| `crates/iyon-tui-native/src/generated/*` | Generated ABI tables/types/exports/conformance |
| `crates/iyon-tui-native/src/lib.rs` | Native addon module boundary |

Relevant Rust framework files:

| File | Responsibility |
|---|---|
| `crates/iyon-tui/src/application/host.rs` | `TuiHost`, `HostInner`, desired/visible epochs, candidate frame, backend handoff |
| `crates/iyon-tui/src/application/kernel.rs` | `RunningApp`, application view callback, frame preparation |
| `crates/iyon-tui/src/application/app.rs` | Generic Rust `App` definition |
| `crates/iyon-tui/src/scene/host.rs` | Scene resolution, incremental/full reconciliation, history projection and paint preparation |
| `crates/iyon-tui/src/scene/layout.rs` | Resolved-scene to layout-tree orchestration |
| `crates/iyon-tui/src/presentation/layout/engine.rs` | Semantic measurement, preparation and placement |
| `crates/iyon-tui/src/presentation/paint/view.rs` | Layout-tree to physical `Surface`/rows |
| `crates/iyon-tui/src/presentation/factory.rs` | Internal semantic `View` construction |
| `crates/iyon-tui/src/presentation/ir.rs` | Private Rust semantic IR |
| `crates/iyon-tui/src/lib.rs` | Visibility boundary and native binding exposure |

### 1.6 Approximate physical size

The following are approximate source spans, counted from the physical source line ranges and final symbols inspected. Counts include comments and blank lines; generated files are listed separately. These are not compiler-generated code-size measurements.

| Area | Files | Approximate physical production LOC | Approximate test/generated LOC | Method/notes |
|---|---|---:|---:|---|
| Public TS entry/runtime | `src/index.ts`, `runtime/runtime.ts`, `runtime/access.ts`, `runtime/environment.ts` | ~1,200 | small | Runtime implementation ends near line 928 |
| TS composition | `composition/define-view.ts`, `execution.ts`, `execution-context.ts`, `compose.ts`, `child-owner.ts`, `tracked-state.ts`, `publication.ts`, `persistent-seq.ts` | ~3,500 | small | `execution.ts` ends near line 1,254; `persistent-seq.ts` near line 309; `compose.ts` near line 876 |
| TS semantic API | `api/view/view.ts`, `scene.ts`, `semantic-node.ts`, geometry/presentation dependencies | ~2,000 | small | `view.ts` spans roughly 1,146 lines |
| TS retained transport | `retained-dag.ts`, `native-view-abi.ts`, structural encoding and resource helpers | ~3,000 | small | `retained-dag.ts` spans roughly 2,138 lines |
| TS generated ABI | `view_calls.ts`, `view_abi.ts`, conformance and schema artifacts | generated, several hundred | generated | Generated from `tools/tui-abi/view_abi.toml`; not manually authored |
| TS testing wrapper | `testing/index.ts` | ~154 | test helper | Headless wrapper, not production composition root |
| Rust host/kernel/app | `application/host.rs`, `kernel.rs`, `app.rs` | ~4,000 production | ~2,000 tests | `host.rs` contains substantial in-file tests after production code |
| Rust scene/layout/paint path | `scene/host.rs`, `scene/layout.rs`, `presentation/layout/engine.rs`, `presentation/layout/tree.rs`, `presentation/paint/view.rs` | ~6,000 | substantial tests | Includes incremental paint and history paths |
| Native addon | `crates/iyon-tui-native/src/tui.rs`, `tui/view_abi.rs` | ~6,000 | small/in-file | `view_abi.rs` spans approximately 4,550 lines |
| Generated Rust/native ABI | `crates/iyon-tui-native/src/generated/*` | generated | generated | Indexed, not treated as hand-authored semantic ownership |

---

## 2. Types, APIs and contracts

### 2.1 `SceneContract` and `SceneProducer`

`packages/iyon-tui/src/api/view/scene.ts:4-28` defines:

```ts
interface SceneContract {
  readonly history?: History;
  readonly body: View;
}

type SceneProducer = SceneContract | (() => SceneContract);
```

`Scene` is a concrete immutable holder for `body` and optional `history`. `Scene.from` normalizes a structural scene object into a concrete `Scene`.

The public runtime therefore accepts either:

1. a direct structural scene value; or
2. a synchronous retained producer.

The two routes intentionally have different ownership and lifecycle semantics.

### 2.2 `TuiRuntime`

`TuiRuntime` is declared in `runtime/runtime.ts:59-90`.

The central method is:

```ts
render(scene: SceneProducer, signal?: AbortSignal): void;
```

The interface documentation explicitly states:

- direct scene values take over the root immediately;
- producer functions own a retained root;
- producers remain subscribed to tracked state.

Other composition-root methods:

- `flush`
- `resize`
- `close`
- `exit`
- `createHistory`
- `viewState`
- `contentPort`
- `createTextInput`
- `createViewSlot`
- `createScrollPane`
- `setTheme`
- generic key/output/paste routing

### 2.3 `View`

`View` is the public semantic authoring value. Public constructors include:

- `View.text`
- `View.styledText`
- `View.spacer`
- `View.horizontal`
- `View.vertical`
- `View.hanging`
- `View.grid`
- `View.content`
- `View.contentMax`
- `View.diff`

Modifiers include:

- text attributes;
- `padding`;
- foreground/background;
- border;
- style/style state;
- retained `ViewState`;
- container/clamp;
- fit/fill/min/max dimensions;
- wrapping and text alignment.

The normal direct-construction path creates a new `View`/semantic node for each constructor or modifier. During retained execution, `isRetainedConstruction()` routes the same public methods into `compose.ts`, where semantic slots compare the prior value and reuse it where possible.

For example:

- retained construction decision: `view.ts:152-154`;
- direct versus retained text: `view.ts:299-307`;
- direct versus retained vertical/horizontal: `view.ts:326-347`;
- direct versus retained modifiers: `view.ts:365-475`;
- semantic slot reuse for text/decorations: `compose.ts:303-430`.

### 2.4 `defineView`

`defineView` is a public wrapper around `invokeComponent`:

- public contract and examples: `composition/define-view.ts:11-31`;
- type declarations: `define-view.ts:37-46`;
- implementation: `define-view.ts:48-65`.

The returned object is both:

- a component value carrying `.render`;
- a callable function that invokes the component in the active retained scope.

A call outside any active evaluating scope fails with `TUI_EXECUTION_NO_ACTIVE_SCOPE`.

`defineView` does not create a global registry, site ID, compiler transform, or globally shared component identity. The component object itself is the stable component type identity; the execution scope is reconciled by parent-local position and optional key.

### 2.5 `State<T>`

`state()` returns a wrapper exposing:

```ts
interface State<T> {
  readonly value: T;
  set(value: T): void;
  update(update: (previous: T) => T): void;
}
```

Implementation and contract are in `tracked-state.ts:1-136`.

Important invariants:

- reads during active evaluation link the current execution scope;
- reads outside evaluation are ordinary untracked reads;
- writes inside a component body throw;
- writes only publish if `Object.is` detects a change;
- a changed source invalidates each subscribed live scope;
- dependency subscription changes are committed only after a successful scope evaluation;
- aborted evaluations preserve the previous committed dependency set.

### 2.6 Structural publication contracts

`composition/publication.ts` defines the separation between:

- where a scope publishes output (`StructuralPublicationTarget`);
- how a scope is represented in a parent (`StructuralScopeProjection`);
- the prepared publication operation.

`RetainedExecutionScope` exposes:

```ts
publicationTarget
projection
projectedOutput
stagedPublication
```

(`execution.ts:144-149`).

This separation is important for the composition root:

- the root producer publishes to `RetainedRootBoundary`;
- a child component may be projected through a native `ViewSlot`;
- component execution identity is not the same thing as the slot/native root it projects into.

### 2.7 Native ABI session

`native-view-abi.ts:57-70` defines `NativeViewAbiSession`:

- opaque native runtime handle;
- generated symbol object;
- ABI metadata;
- semantic version;
- schema and generator hashes;
- generation;
- N-API transport identifier.

`nativeViewAbiSession()` bootstraps once and validates metadata against the generated manifest (`native-view-abi.ts:98-128`).

Generated calls use the opaque N-API runtime handle. The generated wrappers convert error-bit return values into `NativeAbiStatusError`:

- `view_calls.ts:7-27`;
- all generated constructor wrappers follow the same checked-reference pattern.

### 2.8 Retained NativeRef correspondence

`retained-dag.ts` owns the semantic-to-native correspondence. The main contracts are:

- `SemanticNativeHint`: generation-scoped weak hint;
- `MaterializeTx`: transaction-local refs and temporary leases;
- `RootPublication`: prepared root with `commit`/`abort`;
- `RetainedRootBoundary`: desired/visible root lease manager.

The source emphasizes that `SEMANTIC_NATIVE` is acceleration metadata, not ownership:

- `retained-dag.ts:52-59`;
- `retained-dag.ts:91-103`;
- `retained-dag.ts:208-229`.

The actual ownership unit is a native lease represented by the NativeRef table and boundary/transaction lifecycle.

---

## 3. Dependency and ownership map

### 3.1 Main dependency diagram

```text
Caller
  │
  ├─ View.text / View.vertical / View modifiers
  │       └─ immutable semantic nodes + NodeIds
  │
  ├─ Scene { body, history? }
  │
  └─ Tui.render(...)
          │
          ├─ direct SceneContract
          │      └─ renderDirect
          │
          └─ () => SceneContract
                 └─ renderCanonical
                        └─ OwnedBuilderRoot
                               └─ RetainedExecutionScope
                                      ├─ State subscriptions
                                      ├─ child scopes
                                      ├─ semantic slots
                                      └─ RetainedRootBoundary publication target
                                              │
                                              ├─ MaterializeTx
                                              │      └─ ensureSemanticNative
                                              │             ├─ semantic hints
                                              │             ├─ NodeId promotion
                                              │             ├─ derivation patches
                                              │             └─ direct ABI constructors
                                              │
                                              └─ NativeTuiHost.setDesiredViewRef
                                                     │
                                                     └─ Rust TuiHost::set_desired_view
                                                            │
                                                            └─ Environment pending-host queue
                                                                   │
                                                                   └─ HostInner::render
                                                                          │
                                                                          ├─ SceneHost resolution
                                                                          ├─ layout
                                                                          ├─ paint
                                                                          ├─ candidate frame
                                                                          ├─ backend presentation
                                                                          └─ visible commit
```

### 3.2 Ownership and identity layers

| Layer | Identity | Owner | Lifetime |
|---|---|---|---|
| Public component | `ViewComponent` function/object | caller/application | until caller drops it |
| Execution | `RetainedExecutionScope.id` | `RetainedExecutionRuntime` / parent scope | mount through scope disposal |
| Semantic | `View`/semantic node `NodeId` | immutable semantic value | value reachability |
| Native structural | `NativeRef` | native runtime lease table plus boundary/transaction | lease-bearing publication |
| Host | `host_id`/native host object | `Tui` and native environment | `Tui.open` through close/exit |
| Component control | native component ID | Rust component registry/host | mount through retirement |
| State | `State<T>` wrapper/source | caller; subscriptions owned by scopes | source lifetime; subscriptions scoped |
| ViewState | host state ID | `Tui`/native host | host lifetime unless explicitly disposed |
| ContentPort | host attachment ID | `Tui`/native host | host lifetime unless explicitly disposed |
| History | wrapper + native history value | caller or `Tui` depending on route | detached caller-owned, or attached to one host |
| Physical frame | `Surface` and backend receipt | `HostInner`/backend | candidate until commit, then visible frame |

### 3.3 Root ownership

`Tui` owns:

- one native host;
- one `RetainedExecutionRuntime`;
- one optional canonical root builder;
- one optional retained root boundary;
- root history binding state;
- all handles created through the `Tui`;
- runtime environment registration;
- host attachment binding state.

The constructor and ownership setup are in `runtime.ts:108-215`.

The runtime deliberately creates one retained execution runtime eagerly per `Tui` (`runtime.ts:120-125`). The canonical scene root, `ViewSlot` roots, and `ScrollPane` builder roots share the same dirty queue and batch protocol.

### 3.4 Child scope ownership

A component invocation proceeds through:

```text
defineView callable
    → invokeComponent
        → activeExecutionScope
            → RetainedExecutionRuntime.invokeChild
                → resolve keyed group, if keyed
                → invokeInto
                    → reconcileChild
                    → evaluateIntoPendings
```

Relevant source:

- `execution.ts:1166-1180`
- `execution.ts:941-1041`

Unkeyed children reconcile by positional ordinal and component type. Keyed children route through a keyed child-owner namespace. Key groups are not independently scheduled scopes; their child scopes remain execution scopes.

### 3.5 Projection ownership

When a child scope is created under a `Tui` runtime, the configured projection factory in `runtime.ts:169-194` creates a `ViewSlot` seeded with `View.spacer(0)`.

The projection factory:

1. creates a native `ViewSlot`;
2. creates a component semantic view for the slot handle;
3. retains an attachment reference to the slot;
4. returns:
   - the slot-backed semantic `View`;
   - a publication target whose `preparePublication` calls `slot.prepareSetView`;
   - a `dispose` function that disposes the slot.

This means:

- execution scope identity is held by `RetainedExecutionScope`;
- the embedded parent view is a semantic `component` node referring to a slot/component handle;
- the child output is published into the slot’s native root;
- the parent output embeds the slot/component view rather than directly embedding the child’s full semantic subtree.

### 3.6 Root boundary lease ownership

`RetainedRootBoundary` tracks separate:

- `previousRef` in direct mode;
- `desiredRef`;
- `visibleRef`;
- `desiredNode`;
- `visibleNode`;
- `supersededDesired`;
- desired and visible structural revisions.

Fields are declared at `retained-dag.ts:1572-1585`.

For the deferred canonical `Tui` root:

```text
prepared root ref
    → desired root lease
        → native host desired revision
            → environment frame
                → visible root lease
```

The old visible root is not released merely because a new desired root was accepted. It is released when the corresponding frame becomes visible through `commitVisible`.

### 3.7 Forward and reverse edges

Forward:

```text
Tui.render
  → OwnedBuilderRoot.start or renderDirect
  → RetainedExecutionRuntime.mountExistingRoot/update
  → RootPublication target
  → RetainedRootBoundary.prepareDesiredInstall
  → ensureSemanticNative
  → generated ABI calls
  → NativeViewRuntime publish
  → NativeTuiHost.setDesiredViewRef
  → TuiHost::set_desired_view
  → EnvironmentWakeBroker / native environment queue
  → HostInner::render
  → SceneHost
  → layout / paint / backend
```

Reverse/ownership:

```text
Tui.close
  → RuntimeHostRegistration.dispose
  → native content resource cascade
  → attachment binding disposal
  → ViewState binding cleanup
  → resource registry host invalidation
  → owned handle disposal
  → root builder disposal
  → retained execution runtime disposal
  → RetainedRootBoundary.close
  → NativeTuiHost.dispose
  → TuiHost.close
  → native retained view root reset and host unregister
```

---

## 4. Execution paths and state transitions

## 4.1 Open/create trace

A real `Tui` creation path is:

```text
await Tui.open(options)
  → validate AbortSignal
  → choose width/height defaults
  → validateSize
  → requireNativeClass(native.NativeTuiHost)
  → new NativeTuiHost(width, height, headless)
  → new Tui(host, width, height)
  → runtimeEnvironment.registerHost(...)
  → create AttachmentRuntimeContext
  → new RetainedExecutionRuntime(...)
  → registerRuntimeAccess(...)
  → optional tui.setTheme(theme)
  → return Tui
```

TypeScript source:

- `Tui.open`: `runtime.ts:319-341`
- constructor: `runtime.ts:143-215`
- dimensions and `NativeTuiHost` creation: `runtime.ts:319-330`

Native forwarding:

```text
NativeTuiHost::new
  → host_environment_for_env
  → TuiHost::open_in_environment
```

Native source:

- `crates/iyon-tui-native/src/tui.rs:603-635`

Rust host creation:

```text
TuiHost::open_in_environment
  → validate width/height
  → choose Headless or Real backend
  → TuiApp::new(host_init, host_update, host_view)
  → with_theme(Theme::new)
  → with_history(History::new)
  → app.start(now)
  → prepare_frame(...)
  → construct HostInner
  → environment.register_host
  → present_frame initial frame
  → return TuiHost
```

Rust source:

- `crates/iyon-tui/src/application/host.rs:961-1044`

The native host starts with an internal generic application/kernel and initial semantic body. This bootstrap frame exists before the TypeScript caller publishes its first scene.

### 4.2 Direct scene authoring and first render

Representative public authoring shape:

```ts
const body = View.vertical([
  View.text("hello").bold(),
  View.spacer(1),
]);

const scene = new Scene(body);
const tui = await Tui.open({ headless: true });
tui.render(scene);
tui.flush();
```

The direct route is:

```text
tui.render(scene)
  → Tui.render
      → typeof sceneOrBuilder !== "function"
      → renderDirect
```

`renderDirect` performs:

1. signal check;
2. closed check;
3. retained-runtime drain;
4. reentrancy check;
5. `Scene.from(scene)`;
6. semantic body validation via `semanticNodeOf`;
7. History ownership/native-resource validation;
8. effective history selection;
9. same-body/same-history identity no-op check;
10. `nativeViewAbiSession()`;
11. assign staged history;
12. `prepareRootPublication`;
13. `publication.commit`;
14. dispose any canonical builder root;
15. `flush`.

Source: `runtime.ts:499-548`.

For a first direct scene, `prepareRootPublication`:

```text
prepareRootPublication(session, output)
  → prepareSemanticAttachments
  → new Scene(output, effectiveHistory)
  → ensureBoundary(session)
  → boundary.prepareDesiredInstall(output)
```

Source: `runtime.ts:224-277`.

Because `ensureBoundary` creates the root boundary with `{ deferHostCommit: true }` (`runtime.ts:551-560`), `prepareDesiredInstall` uses the deferred path:

```text
RetainedRootBoundary.prepareDesiredInstall
  → prepareFrom
  → MaterializeTx
  → ensureSemanticNative(root)
  → create native semantic refs
  → return RootPublication
```

`prepareDesiredInstall` is in `retained-dag.ts:1705-1735`.

The publication commit then:

```text
publication.commit
  → publishDesiredPrepared
  → NativeTuiHost.setDesiredViewRef(rootRef)
  → NativeTuiHost::set_desired_view_ref
  → resolve_native_view
  → TuiHost::set_desired_view
  → validate state attachments
  → validate content attachments
  → set desired state/content bindings
  → update running scene body and host body
  → increment desired_structural_revision
  → mark_pending
  → commit attachment bindings
  → hostRegistration.markPending
  → currentScene = nextScene
```

TypeScript root-publication commit: `runtime.ts:254-269`.

Native N-API forwarding: `crates/iyon-tui-native/src/tui.rs:655-672`.

Rust desired-root acceptance: `crates/iyon-tui/src/application/host.rs:1091-1120`.

The desired root is accepted before it becomes visible. No layout/paint is performed by `setDesiredViewRef` itself.

### 4.3 Canonical retained producer first mount

Representative route:

```ts
const count = state(0);

tui.render(() => ({
  body: View.text(`count=${count.value}`),
}));
```

The canonical route is:

```text
tui.render(builder)
  → renderCanonical(builder)
  → construct producer closure
  → if first canonical render:
       construct rootTarget
       OwnedBuilderRoot.start(...)
           → new OwnedBuilderRoot(...)
           → construct root RetainedExecutionScope
           → runtime.mountExistingRoot(scope)
               → runWork(scope)
               → stagePublicationsRecursive(scope)
               → commitBatch([scope])
       save rootBuilder
       mark rootScopeCreated
       flush
```

Source: `runtime.ts:451-497`.

`OwnedBuilderRoot`:

- stores the current producer closure;
- creates a root component type whose `.render` invokes the current producer;
- creates an execution scope with no parent;
- assigns the root publication target.

Source: `execution.ts:1198-1221`.

The root producer closure itself performs:

```text
builder()
  → Scene.from(builder result)
  → stageHistoryBinding(scene.history)
  → return scene.body
```

Source: `runtime.ts:457-460`.

Initial retained execution:

```text
mountExistingRoot
  → roots.push(scope)
  → execution_scope_mounts++
  → runWork
      → execution_scope_body_calls++
      → evaluateIntoPendings
          → scope.state = evaluating
          → scope.table.begin
          → scope.owner.beginChildPass
          → pending dependency set reset
          → push active execution frame
          → call scope.type.render
          → reject Promise-like result
          → validate semantic View output
          → scope.pendingOutput = output
          → pop active frame
  → stagePublicationsRecursive
      → stage child publications first
      → inspect root publication target
      → prepare root publication if output changed or target needs publication
  → commitBatch
      → commit descendants first
      → publication.commit
      → promote output/props/dependencies/semantic slots
      → mounted = true
```

Relevant source:

- evaluation: `execution.ts:627-657`;
- publication staging: `execution.ts:659-683`;
- batch commit: `execution.ts:709-781`;
- root mount: `execution.ts:844-886`.

After `OwnedBuilderRoot.start` returns, `Tui.renderCanonical` calls `this.flush()` because the retained execution commit has accepted desired native structure but host visibility is still a separate frame barrier (`runtime.ts:480-485`).

### 4.4 Nested `defineView` component creation/mount

Representative authoring:

```ts
const Label = defineView<{ text: string }>(({ text }) =>
  View.text(text),
);

tui.render(() => ({
  body: View.vertical((children) => {
    children.child(Label({ text: "one" }));
  }),
}));
```

Call trace during root body evaluation:

```text
Label({ text: "one" })
  → callable component closure from defineView
  → invokeComponent(component, props)
  → activeExecutionScope()
  → active root runtime.invokeChild
  → reconcileChild(root.owner, rootScope, component, key=undefined)
```

On first invocation:

```text
reconcileChild
  → read owner.cursor as ordinal
  → no compatible committed child
  → new RetainedExecutionScope
  → execution_scope_mounts++
  → projectionFactory(scope)
      → create native ViewSlot seeded with View.spacer(0)
      → componentViewForHandle(slot.id)
      → retain slot attachment reference
      → return slot-backed projection target
  → insert child scope into pendingChildren
```

Then `invokeInto`:

```text
invokeInto
  → propsShallowEqual check only for reused scope
  → pendingProps = props
  → pendingPropsActive = true
  → dirty = false
  → execution_scope_body_calls++
  → evaluateIntoPendings(child)
      → push child active frame
      → call Label.render(props)
      → produce View.text(...)
      → child.pendingOutput = View
      → pop child frame
```

The parent receives an embeddable `View`:

```text
typed.projection.view
```

when a projection exists; otherwise it receives the child pending/current output.

Source:

- `invokeComponent`: `execution.ts:1166-1180`;
- child reconciliation: `execution.ts:941-974`;
- child invocation: `execution.ts:976-1011`;
- core child evaluation: `execution.ts:1013-1041`;
- `defineView` callable wrapper: `define-view.ts:55-64`.

### 4.5 Child publication order

The root’s semantic output contains a component/slot reference, while the child’s own output is published into its slot. During retained batch preparation:

```text
stagePublicationsRecursive(root)
  → stageOwnerPublications(root.owner)
      → stagePublicationsRecursive(child)
          → prepare child ViewSlot publication
      → prepare root publication
```

The source explicitly stages child publications before the parent publication (`execution.ts:659-683`).

During commit:

```text
commitScope(root)
  → commitOwnerChildren(root.owner)
      → commitScope(child)
          → child publication.commit
          → child output/props/dependencies/slots become current
  → root staged publication.commit
  → root output becomes current
```

The descendant-first rule is implemented at `execution.ts:723-781`.

This guarantees that a parent output embedding a child projection sees the child’s native slot publication accepted before the parent root is promoted.

### 4.6 State-driven update without calling `render` again

The public state path is:

```text
state.value read in builder/component body
  → StateSource.value
  → activeExecutionScope()
  → scope.linkDependency(this)
  → dependency set committed after successful mount

state.set(newValue)
  → Object.is comparison
  → source.currentValue update
  → publish subscribers
  → scope.runtime.invalidateFromState(scope)
  → scope.runtime.invalidate(scope)
  → queue scope once
  → schedule microtask
```

Source:

- state read and dependency linking: `tracked-state.ts:49-54`;
- publication and writes: `tracked-state.ts:65-92`;
- invalidation: `execution.ts:550-554`;
- queue/scheduling: `execution.ts:413-438`.

The automatically scheduled retained flush:

```text
microtask
  → RetainedExecutionRuntime.flush
  → acquire dirty queue
  → sort parent-before-child
  → evaluate dirty scopes
  → stage all publications
  → commit batch
  → host registration is marked pending by root publication
```

The state source is not itself a semantic value and does not directly send a View across N-API. It invalidates execution scopes; the scope re-evaluates the producer/body and only then creates or reuses semantic/native structure.

### 4.7 Explicit `Tui.flush`

`Tui.flush` is a two-plane barrier:

```text
Tui.flush
  → ensureOpen
  → retainedRuntime.flush
  → hostRegistration.flush
```

Source: `runtime.ts:407-420`.

The first step drains TypeScript retained execution. The second step calls the environment wake broker, which drains native pending hosts and ensures the requested host reaches the captured pending epoch.

Wake broker path:

```text
RuntimeHostRegistration.flush
  → EnvironmentWakeBroker.flush(registration)
  → read native epochs
  → cancel queued microtask
  → drain(forceRetry=true, preferred host)
  → native.flushPendingHosts(...)
  → consumeReport
      → commit report invokes onCommitted callback
      → errors enter RuntimeErrorChannel
  → compare committed epoch with captured epoch
  → retry up to MAX_EXPLICIT_DRAINS if needed
  → throw pending runtime error or barrier failure
```

Source:

- `wake-broker.ts:205-250`;
- native drain call: `wake-broker.ts:307-341`;
- report consumption/commit callback: `wake-broker.ts:344-375`.

For a canonical root, the registered `onCommitted` callback is:

```text
Tui constructor callback
  → owner.deref()?.commitVisibleAfterDrain(commit)
```

(`runtime.ts:147-152`).

This callback promotes the deferred root’s desired role to visible and synchronizes attachment lifecycles:

- `runtime.ts:770-781`;
- `retained-dag.ts:1743-1789`.

### 4.8 Native frame preparation

Once the native environment drains a pending host:

```text
NativeTuiHost.flushPendingHosts
  → TuiHost::flush_pending_hosts
  → environment.drain_pending_for(...)
  → HostInner::flush_for_environment(true)
      → flush_pending_frame
          → if pending candidate receipt completed: commit candidate
          → if pending epoch != committed epoch: render
```

Source:

- N-API forwarding: `crates/iyon-tui-native/src/tui.rs:683-740`;
- host entry: `crates/iyon-tui/src/application/host.rs:1122-1149`;
- environment flush: `host.rs:1671-1679`;
- render selection: `host.rs:2324-2355`.

`HostInner::render` performs:

1. refuse to start a second preparation while a backend presentation receipt is pending;
2. capture pending epoch and desired structural revision;
3. begin content candidate;
4. capture retained-state candidate overlay;
5. call `prepare_frame_with_content`;
6. prepare state commit;
7. prepare content commit;
8. install candidate frame metadata;
9. submit candidate to backend;
10. either wait for receipt or commit candidate immediately for headless mode.

Source: `application/host.rs:1870-1959`.

### 4.9 Scene resolution

`prepare_frame_with_content` delegates to `RunningApp::prepare_frame_with_states`:

```text
prepare_frame_with_content
  → RunningApp::prepare_frame_with_states
      → if body_dirty:
          call application view callback
          update Scene body
      → SceneHost::render_at_with_states
```

Source:

- `application/host.rs:2365-2426`;
- `application/kernel.rs:667-702`.

`SceneHost::render_at_with_states` loops through resolution and history pressure:

```text
SceneHost::render_at_with_states
  → obtain viewport size
  → resolve_stable_at_with_anchor
      → incremental or full scene resolution
      → layout
      → component synchronization
      → possible re-resolution if callbacks make layout dirty
  → if history overflow:
      → drain native history pressure
      → repeat with updated frontier or pinned anchor
  → paint_with_content
```

Source: `scene/host.rs:1036-1135`.

The stable-resolution path can:

- reuse incremental scene/layout state;
- begin a layout cache epoch for full resolution;
- resolve full structural topology;
- synchronize component geometry callbacks;
- repeat if layout callbacks alter revisions;
- discard candidate state on failure.

### 4.10 Native layout

The principal full layout chain is:

```text
layout_resolved_scene_with_cache_and_content
  → layout_view_with_overlay_and_cache_and_content
      → layout_view_with_overlay_and_cache_in_scope_and_content
          → determine width constraint
          → measure_node
          → prepare_node
          → emit_prepared
          → construct LayoutTree
          → index component roots/state roots/content roots
```

Source:

- `scene/layout.rs:39-54`;
- `presentation/layout/engine.rs:45-143`.

The layout tree contains:

- node rectangles;
- content rectangles;
- clip rectangles;
- component ownership;
- child dependencies;
- layout style;
- content classification;
- state/content attachment indexes;
- vertical child-order indexes for paint pruning.

`LayoutTree` fields are in `presentation/layout/tree.rs:100-136`.

Layout measurement is terminal-aware. Text wrapping and Unicode cell geometry are handled in the Rust presentation/layout and paint layers; TypeScript only supplies semantic text and style values.

### 4.11 Native paint

The full paint chain is:

```text
SceneHost::paint_with_content
  → install retained StableScene
  → construct ViewCompiler
  → begin PaintCache epoch
  → ViewPainter.paint_tree_with_content
      → paint_tree_with_style_and_cache
          → recursively paint LayoutTree nodes
          → resolve inherited style/theme
          → paint text, decoration and content
          → composite physical surfaces
  → retain Surface for later incremental repaint
  → PreparedSceneFrame
```

Source:

- `scene/host.rs:2058-2240`;
- `presentation/paint/view.rs:749-763`;
- full tree painter entry around `presentation/paint/view.rs:901` onward.

Incremental paint can repaint only:

- History subtree;
- ViewState-rooted subtree;
- selected component subtree;
- ContentPort-rooted repaint roots.

The source documents these paths and the corresponding methods:

- `paint_subtree_into_with_content`: `paint/view.rs:821-888`;
- `paint_component_into_with_content`: `paint/view.rs:781-801`;
- incremental use in `scene/host.rs:2087-2203`.

The full painter uses two generations of cached physical subtree surfaces:

- `PaintCache.current`;
- `PaintCache.previous`.

Cache behavior is in `paint/view.rs:162-222`.

### 4.12 Backend submission and visible frame commit

For headless hosts:

```text
present_frame
  → frame_pending = false
  → no asynchronous receipt
  → commit_frame
```

For real hosts:

```text
present_frame
  → TerminalBackend.begin_frame(candidate)
  → store asynchronous presentation receipt
  → frame_pending = false
  → later flush polls receipt
  → commit_frame after receipt succeeds
```

Source: `application/host.rs:1961-2023`.

`commit_frame` promotes candidate authority under the environment completion authority:

```text
commit_frame
  → environment.with_host_completion(...)
      → commit content candidate
      → take candidate frame
      → take state/content plans
      → commit prepared ViewState table
      → end content candidate
      → commit content candidate into running scene
      → clear candidate metadata
      → recover native History synchronization if needed
      → frame = candidate
      → visible structural revision = candidate revision
      → visible frame revision++
      → committed epoch = candidate epoch
      → clear content dirty if epochs match
      → return HostFlushOutcome::committed
```

Source: `application/host.rs:2026-2114`.

The native report returns visible structural revision and committed epoch. The TypeScript wake broker invokes `commitVisibleAfterDrain`, which promotes the deferred root lease to visible.

### 4.13 Replacement/update

Canonical producer replacement:

```text
tui.render(newBuilder)
  → renderCanonical
  → drain pending retained work
  → rootBuilder.replaceProducer(new producer)
      → save previous producer
      → assign new producer
      → runtime.update(rootScope)
          → invalidate
          → synchronous retained flush
      → on failure:
          restore previous producer
          cancel only retry obligation introduced by this attempt
          rethrow
  → Tui.flush
```

Source: `runtime.ts:487-497`, `execution.ts:1224-1248`.

The closure identity itself is not the retained identity. Every call replaces the producer on the same root execution scope. This is a significant current contract.

Direct scene replacement:

```text
tui.render(scene2)
  → normalize/validate scene2
  → prepareRootPublication(scene2.body)
  → commit desired root
  → disposeRootBuilder()
  → flush
```

The direct route takes over root publication and tears down any canonical producer root after the direct publication commits (`runtime.ts:535-548`).

### 4.14 Direct same-identity no-op

If the current direct `Scene` has the same `body` object and effective `History` object:

```text
renderDirect
  → validate semantic attachments
  → stagedHistory = effectiveHistory
  → disposeRootBuilder
  → flush
  → return
```

No new root structural publication is prepared (`runtime.ts:518-532`).

This is an identity cutoff above the transport. It is separate from semantic equality: two distinct `View` objects with equivalent fields are not the same object and can enter retained materialization.

### 4.15 Unmount/destruction

`Tui.close()`:

```text
tui.close
  → if already closed: return
  → assertNotMutating
  → closed = true
  → historyLifetime.closed = true
  → disposeRetainedExecution
      → hostRegistration.dispose
      → host.disposeContentResources
      → clear runtime error listener
      → attachmentBindings.dispose
      → host.clearViewStateBindings
      → runtimeEnvironment.resources.invalidateHost(host token)
      → disposeOwnedHandles
      → disposeRootBuilder
          → root scope detach
          → scope.dispose
          → projection dispose
          → unsubscribe dependencies
          → dispose child scopes
          → noteUnmount
      → retainedRuntime.dispose
      → boundary.close
          → release desired/visible/superseded root refs
  → currentScene = undefined
  → host.dispose
      → NativeTuiHost.dispose
          → abort native edit transactions
          → TuiHost.close
              → wait presentation receipt
              → reset body to spacer
              → clear retained native views
              → dispose ViewStates/content
              → mark closed
              → unregister host
```

TypeScript source:

- close entry: `runtime.ts:817-840`;
- retained teardown: `runtime.ts:784-814`;
- scope disposal: `execution.ts:192-218`;
- root detach: `execution.ts:888-900`;
- boundary close: `retained-dag.ts:2047-2070`.

Native/Rust source:

- `NativeTuiHost.dispose`: `crates/iyon-tui-native/src/tui.rs:742-750`;
- `TuiHost.close`: `crates/iyon-tui/src/application/host.rs:1558-1609`.

`Tui.exit()` differs in one important ordering:

```text
tui.exit
  → host.exit()
      → complete earlier presentation
      → request native application exit
      → advance/render final frame
      → wait final presentation
      → restore terminal / collect final headless rows
  → disposeRetainedExecution
  → mark closed
```

Source: `runtime.ts:843-877`, native host exit forwarding at `tui.rs:761-767`, Rust final-frame handling at `application/host.rs:1189-1239`.

The final exit frame is intentionally rendered while content bindings still exist. `close()` does not prepare another frame.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | TypeScript production path | Native/Rust path | Selection condition | Failure behavior |
|---|---|---|---|---|
| Open TUI | `Tui.open` | `NativeTuiHost::new` → `TuiHost::open_in_environment` | Always | Validates size/artifact; cleanup may aggregate errors |
| Render direct scene | `Tui.render` → `renderDirect` | `setDesiredViewRef` → `TuiHost::set_desired_view` | Argument is object | Invalid scene/attachment/history rejects before desired mutation |
| Render retained producer | `renderCanonical` → `OwnedBuilderRoot` | Same desired-root native publication after retained execution | Argument is function | Producer/evaluation/prepare failures roll back; no fallback transport |
| Construct text | `View.text` or retained `composeText` | `ensureSemanticNative` → cstring/UTF-8 generated call → `view_text_*_impl` | NUL-free/embedded-NUL and span count | Empty spans or oversized payload explicitly refuse |
| Construct axis | `View.vertical/horizontal` → retained compose or direct semantic node | fixed arity constructor for <=4; buffer constructor above 4 | Child count | Native status becomes retained refusal |
| Construct grid | `View.grid` | `viewGridCreateBuffer` → Rust parse/factory | Grid semantic node | Invalid word encoding/status rejects |
| Decorate | `View.padding/style/...` | derivation fast path or decorated constructor | Base hint availability and derivation compatibility | Derivation failure falls back to direct retained materialization, not another architecture |
| Structural axis replacement | transport helper/semantic derivation | `viewAxisSetChild` or `viewAxisSpliceBuffer` | Derivation base available | One stale retry, then explicit refusal |
| Root publication | `RetainedRootBoundary.prepareDesiredInstall` | `setDesiredViewRef` | Canonical Tui root deferred mode | Previous visible root stays authoritative until frame commit |
| Frame flush | `Tui.flush` → wake broker | `flushPendingHosts` → native environment | Explicit barrier | Structured runtime error or barrier failure thrown |
| Resize | `Tui.resize` | `NativeTuiHost.resize` → `TuiHost.resize` | Explicit caller call | Dimensions update only after native success |
| State update | `State.set` → scope invalidation | Later native frame uses ViewState candidate overlay if applicable | State source has subscribers | State writes in active body forbidden |
| Close | `Tui.close` | host/resource disposal | Explicit caller call | Cleanup attempts continue and aggregate errors |

### 5.2 Retained identity lookup order

The actual `ensureSemanticNative` order is:

```text
1. generation-valid semantic NativeRef hint
2. transaction-local ref
3. NodeId → NativeRef promotion, only if NodeId <= nativeLookupCeiling
4. cycle check
5. derivation fast path
6. per-kind semantic materializer
7. optional ViewState attachment
8. install hint, retain transaction lease
```

Source: `retained-dag.ts:1148-1220`.

The source explicitly avoids NodeId promotion for genuinely new nodes. The boundary captures `nativeLookupCeiling` after successful publication.

### 5.3 Materialization routes

Direct materializers include:

- spacer: `viewSpacerCreate`, `retained-dag.ts:475-479`;
- content host: `viewContentHostCreate`, `retained-dag.ts:482-501`;
- row/column:
  - fixed arities 0-4;
  - `viewAxisCreateBuffer` above four children;
  - `retained-dag.ts:515-566`;
- grid: `viewGridCreateBuffer`, `retained-dag.ts:578-620`;
- text:
  - cstring lane for NUL-free 1-4 span text;
  - UTF-8 lane for embedded-NUL 1-4 span text;
  - words+bytes buffer lane for >4 spans;
  - `retained-dag.ts:685-797`;
- diff: words+bytes lane;
- hanging/container/clamp/component/decorated:
  - `retained-dag.ts:808-1054` and materializer dispatch at `1106-1120`.

### 5.4 Derivation route

`tryDerivation` tries a retained native patch before inspecting the full semantic payload:

- text layout patch: `viewTextLayoutPatchRoot`;
- common scalar patch: `viewCommonPatchRoot`;
- axis child replacement: `viewAxisSetChild`;
- axis splice: `viewAxisSpliceBuffer`;
- grid cell replacement: `viewGridSetCell`.

Source: `retained-dag.ts:1252-1366`.

If the derivation base is unavailable, the implementation falls through to direct semantic materialization within the same retained transaction (`retained-dag.ts:1222-1230`). This is a same-architecture recovery/optimization, not a fallback to the former complete-object or path transport.

### 5.5 Stale-reference recovery

Expected native cache-miss status is decoded from `NativeAbiStatusError`. One targeted stale retry is allowed per transaction:

```text
native cache miss
  → identify stale child/base
  → delete stale semantic hint
  → delete transaction-local ref
  → re-run ensureSemanticNative
  → retry constructor/derivation once
```

Source:

- `recoverStaleNode`: `retained-dag.ts:415-431`;
- materializer recovery: `retained-dag.ts:434-467`;
- derivation recovery: `retained-dag.ts:1350-1365`;
- exact-root recovery: `retained-dag.ts:1380-1463`.

If the retry fails, the retained path refuses explicitly. The source repeatedly states there is no secondary recovery transport.

### 5.6 Root prepare/commit/abort

`RetainedRootBoundary.prepareInstall` and `prepareDesiredInstall` separate fallible preparation from publication:

```text
prepare:
  → retained walk
  → materialization
  → stale recovery
  → lease acquisition
  → no host desired/visible mutation

commit:
  → set desired root or direct host render
  → release temporary leases
  → transfer root lease
  → update desired/visible bookkeeping

abort:
  → release temporary leases
  → release newly acquired boundary lease if needed
  → previous root remains authoritative
```

Source:

- protocol comments: `retained-dag.ts:1488-1519`;
- publication contract: `retained-dag.ts:1521-1529`;
- deferred commit: `retained-dag.ts:1711-1735`;
- preparation: `retained-dag.ts:1801-1916`.

### 5.7 Failure masking and non-masking

Failures are generally explicit:

- retained refusal is not converted to an older transport;
- expected ABI status becomes `RetainedRefusalError`;
- a failed producer restores the prior producer;
- a failed retained execution batch restores dirty obligations but does not schedule an infinite automatic retry;
- native frame preparation failures remain associated with attempted epoch and desired revision;
- automatic wake drains record errors into `RuntimeErrorChannel` rather than throwing from a microtask;
- explicit `Tui.flush` surfaces pending runtime errors synchronously;
- unknown native error codes become `INTERNAL_INVARIANT`, not ordinary retryable frame errors.

Wake-broker error normalization:

- `wake-broker.ts:344-375`;
- error code conversion: `wake-broker.ts:494-539`.

### 5.8 Alternate path search scope

A repository search for:

```text
renderExactRoot
renderExact(
nativePathRefForLineage
tryRetainedEditTransactionRender
pathRoot
pathChild
```

within `packages/iyon-tui/src/**/*.ts` found:

- exact-root implementation and boundary wrapper;
- path/edit helper definitions in `native-view-abi.ts`;
- generated ABI calls.

No public `Tui` composition-root call to those path/edit helpers was found. The canonical route is retained semantic materialization through `RetainedRootBoundary`. The path/edit APIs remain internal/generated or specialized helper routes.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Semantic composition slots

`ScopeSemanticTable` stores one semantic slot per composition call ordinal:

- `begin` resets cursor;
- `next` reuses an existing slot;
- `commit` promotes pending values;
- `rollback` clears pending values and truncates newly created slots;
- `release` drops all slots.

Source: `execution.ts:57-100`.

`compose.ts` compares prior semantic values and calls `stageReuse` when compatible. This avoids allocating a fresh `View` object for stable retained construction.

The cache key is the enclosing execution scope plus semantic ordinal. Keyed component identity does not replace ordinary semantic-slot ordering for raw `View` construction; `View.key` only changes the child-owner namespace for component invocation (`view.ts:249-264`).

### 6.2 Native semantic hints

`SEMANTIC_NATIVE` is:

```ts
WeakMap<SemanticViewNode, { generation, nativeRef }>
```

It is weak and generation-scoped. It does not own the native lease.

Counters expose:

- hint hits/misses;
- NodeId promotion attempts/hits/misses;
- nodes inspected;
- children visited;
- direct materializers;
- derivation calls;
- stale retries;
- host mutations.

Source: `retained-dag.ts:111-155`.

### 6.3 Native NodeId cache and lookup ceiling

The native runtime keeps NodeId/native-reference correspondence. TypeScript only probes NodeId promotion when:

```text
node.id <= tx.nativeLookupCeiling
```

The boundary updates its ceiling after successful desired/visible root acceptance. This prevents an extra FFI lookup for every genuinely new semantic node.

Source:

- lookup order: `retained-dag.ts:1174-1191`;
- ceiling capture in adoption and boundary bookkeeping: `retained-dag.ts:1632`, `1801-1822`.

### 6.4 Transaction-local leases

`MaterializeTx` tracks:

- refs by semantic node;
- `inProgress` recursion guard;
- `temporaryLeases`;
- borrowed hints;
- stale retry count.

Created refs are temporary until root publication transfers ownership. `releaseAll` and `releaseAllExcept` call generated `viewReleaseMany`.

Source: `retained-dag.ts:213-340`.

### 6.5 Scratch buffers

The retained path uses environment-level reusable scratch:

- axis ref scratch per active recursion depth;
- grid/diff/text word scratch;
- UTF-8 byte scratch;
- style atom/style-ref caches.

Relevant source:

- axis scratch: `retained-dag.ts:241-260`;
- word scratch: `retained-dag.ts:263-289`;
- byte scratch: `retained-dag.ts:297-311`;
- style cache: `retained-dag.ts:638-682`.

The source states that native does not retain pointers into these buffers after the synchronous call.

### 6.6 Text transport specialization

Text transport selection:

```text
spans.length == 0
  → refusal

1..4 spans, all NUL-free
  → viewTextCreateCstring{1..4}

1..4 spans, one or more embedded NULs
  → exact-byte UTF-8 buffer lane

>4 spans
  → words + bytes variadic buffer lane
```

Payloads above `MAX_DIRECT_TEXT_BYTES` refuse explicitly.

Source: `retained-dag.ts:623-797`.

### 6.7 Layout cache

`LayoutCache` is used by:

- full layout epoch;
- incremental component-local layout;
- retained stable scene reuse.

Scene resolution begins a layout epoch only for a full/fallback resolution path. It can invalidate view IDs for state changes, discard candidate layout on failure, and reuse retained layout when topology/geometry contracts allow it.

Relevant sources:

- `scene/host.rs:1177-1260`;
- `scene/layout.rs:30-54`;
- `presentation/layout/cache.rs` indexed as a layout subsystem file.

### 6.8 Paint cache and damage

`PaintCache` keeps two generations of cached physical subtree surfaces. It invalidates by semantic `ViewId` and clears both generations on theme change.

Source: `presentation/paint/view.rs:162-222`.

Incremental paint computes damage from:

- state roots;
- content repaint roots;
- component roots;
- History root.

Source: `scene/host.rs:2064-2086`.

A full paint is selected when:

- `full_paint_pending`;
- no usable incremental plan;
- incremental painting fails;
- topology or geometry changed.

### 6.9 Scheduling

There are two scheduling layers:

#### Retained execution scheduler

- `State.set` queues dirty execution scopes;
- `RetainedExecutionRuntime.scheduleFlush` posts one microtask;
- explicit `flush` consumes the scheduled token;
- failed evaluation/prepare restores dirty obligations without automatically rearming the failed retry.

Source: `execution.ts:413-489`, `execution.ts:556-592`.

#### Native host environment scheduler

- native host pending epochs are marked through the wake broker;
- one environment-level microtask drives a fair host queue;
- automatic errors are stored;
- real-terminal presentation receipts are polled with a timer rather than a busy microtask loop;
- explicit barriers force retry and surface errors.

Source: `wake-broker.ts:121-383`.

### 6.10 Work granularity

Per initial root publication:

- semantic tree walk is proportional to newly materialized nodes, unless hints/promotions/derivations apply;
- child traversal is children-first;
- one root publication is installed;
- one environment frame is prepared and committed.

Per stable producer update:

- only dirty scopes re-evaluate;
- unchanged props skip component body execution;
- semantic slots can reuse prior values;
- derivation patches may touch only changed native children;
- native layout may remain incremental or be rebuilt depending on topology/geometry;
- paint may repaint only damaged subtrees.

Per frame:

- native layout and paint are Rust-owned;
- TypeScript does not drive per-tick rendering;
- retained `ViewSlot` animation machinery remains native-side for tick progression.

---

## 7. Tests, benchmarks and observability

### 7.1 Headless application harness

`packages/iyon-tui/src/testing/index.ts` defines `AppHarness`.

It:

- opens `Tui` with `headless: true`;
- forwards render/flush/control operations;
- calls `runtimeAccess(this.tui).advance(0)` after `render`;
- flushes and advances deterministic native work before inspection;
- exposes `screenRows`, `nativeHistoryRows`, `styleAt`, `cellXOfText`, and `exited`.

Source:

- harness creation/render: `testing/index.ts:36-58`;
- control forwarding: `testing/index.ts:63-114`;
- inspection barrier: `testing/index.ts:115-142`.

The harness is a behavioral test adapter, not a separate production root architecture.

### 7.2 Execution counters

`execution.ts:256-301` exposes counters for:

- scope mounts/unmounts;
- body calls;
- prop skips;
- state invalidations;
- dirty enqueues and duplicate invalidations;
- no-op/changed outputs;
- flush passes;
- commit batches/aborts;
- exact semantic reuses/new views.

These counters would verify the composition-root lifecycle, but were not queried during this investigation.

### 7.3 Retained structural counters

`retained-dag.ts:111-155` exposes counters for:

- semantic/native hint hits;
- NodeId promotions;
- inspected semantic nodes;
- child visits;
- direct materializer calls;
- derivation fast paths;
- ref words and byte payload;
- scratch reuse;
- stale retries;
- host mutations.

The exact-root hit contract is documented as:

```text
one hostRenderRef call
zero semantic payload reads
zero children visited
zero buffer words
zero node constructors
```

(`retained-dag.ts:1380-1389`).

This contract applies to `renderExactRoot` when a non-deferred retained boundary uses it. It is not the actual canonical deferred `Tui` root call path described above.

### 7.4 Wake-broker counters and trace

`wake-broker.ts:65-112` exposes:

- pending marks;
- latch wins/already-latched;
- microtasks queued;
- drains;
- hosts attempted;
- frames committed;
- automatic errors;
- rearm count;
- explicit barriers/failures;
- bounded `WakeTraceEvent` history.

Tracing is enabled only when `Bun.env.PERF_RUNTIME_TRACE === "1"` (`wake-broker.ts:115-119`).

Trace event kinds:

- pending;
- drain;
- commit;
- error;
- rearm.

### 7.5 Native epoch observability

`NativeTuiHost.epochs()` returns:

- `host_id`;
- desired structural revision;
- visible structural revision;
- visible frame revision;
- pending epoch;
- committed epoch.

Source:

- TypeScript contract: `addon.ts:125-132`, `134-188`;
- N-API implementation: `crates/iyon-tui-native/src/tui.rs:637-652`.

These epochs are the strongest available cross-boundary evidence for distinguishing:

- desired-root acceptance;
- frame preparation;
- presentation completion;
- visible-frame commit.

### 7.6 Native memory/ABI observability

The addon includes:

- `tuiViewEnvironmentCount`;
- ABI metadata/session bootstrap;
- optional `tuiViewAbiMaintain`;
- optional native runtime memory snapshot;
- optional performance counters.

The native View ABI snapshot includes:

- semantic cache entries;
- NativeRef slots/pages;
- leased/unleased live slots;
- NodeId entries;
- path nodes/keys;
- builders/edit transactions;
- style refs;
- scavenging counters;
- generation/alive state.

Relevant TypeScript contract: `addon.ts:191-228`.

### 7.7 Rust behavioral evidence available

The Rust source contains targeted tests for:

- host frame receipt handling;
- failed presentation;
- frame retry;
- history/content integration;
- layout/paint;
- retained component behavior;
- direct host rendering.

Examples visible in inspected source:

- `application/host.rs` tests around frame receipts and failures;
- `scene/host.rs` tests around retained/incremental paint and host resolution;
- `presentation/ir.rs` tests around identity and semantic mutation;
- `presentation/paint/view.rs` tests around cache and physical rendering.

No test was run for this report.

### 7.8 Missing route assertions

The source has counters and tests for many lower-level paths, but the public composition-root path would benefit from an end-to-end route assertion that verifies, in one scenario:

```text
Tui.open
  → canonical producer mount
  → child component mount
  → desired structural revision
  → visible structural revision
  → headless frame paint
  → State invalidation
  → retained update
  → child/root unmount
  → native resource cleanup
```

This report does not propose implementation changes; it records that the available observability is split across separate TypeScript, native and Rust counters.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Canonical root is deferred desired publication, not direct host render

`runtime.ts` creates:

```ts
new RetainedRootBoundary(session, () => this.host, undefined, {
  deferHostCommit: true,
});
```

(`runtime.ts:551-560`).

Therefore the real canonical root route is:

```text
ensure/materialize root
  → NativeTuiHost.setDesiredViewRef
  → native desired revision
  → later frame preparation/presentation
  → commitVisible
```

It is not:

```text
ensure/materialize root
  → hostRenderRef
  → immediate visible host mutation
```

The latter is implemented for non-deferred boundaries and exact-root helpers.

### 8.2 Runtime comments describe a warm-root fast path not directly selected by canonical root

`runtime.ts:373-389` describes a production scene router including:

```text
warm root hint → exact-root fast path
```

However, source search found no runtime call from `Tui.renderCanonical` to `renderExactRoot` or `boundary.renderExact`. The canonical path stages and publishes desired structure through `prepareDesiredInstall`.

This is a documentation/source-path discrepancy inside the current implementation. The exact-root mechanism exists, but the canonical root’s deferred host commit means the actual visible frame is still driven by the environment frame barrier.

### 8.3 Old path/edit machinery remains internally present

`native-view-abi.ts` still contains:

- path lineage reference construction;
- path root/child calls;
- edit transaction render helpers.

The generated ABI also contains corresponding methods. The runtime comments say the pre-T13 recipe cascade is gone (`runtime.ts:387-389`), and the current canonical route does not use those helpers. Their presence therefore represents internal specialized/generated machinery whose production composition-root reachability is not evident from the inspected TS caller graph.

Absence claim scope: `packages/iyon-tui/src/**/*.ts`, searching the helper names and generated path calls.

### 8.4 Rust authoring visibility is intentionally restricted

`crates/iyon-tui/src/lib.rs` states:

- Rust applications do not author Views, controls or renderers through this crate;
- semantic construction stays private;
- TypeScript callers use the facade.

Visibility confirms this:

- `application`, `presentation`, `scene`, and most semantic modules are `pub(crate)`;
- `presentation::binding` is public only as a hidden native bridge;
- the public crate root exposes `binding`, but explicitly describes it as unsupported and intended for `iyon-tui-native`.

Source:

- `lib.rs:1-12`;
- module visibility: `lib.rs:13-50`;
- binding boundary: `lib.rs:112-117`;
- `presentation/mod.rs:1-23`.

Thus the “Rust authoring alternate entry” exists in two different senses:

1. **crate-internal Rust authoring** through `presentation::factory`, `Scene`, `TuiHost`, and `App`;
2. **native bridge authoring** where TypeScript semantic payloads are decoded by Rust ABI implementations and lowered to private Rust `View` values.

External Rust consumers cannot directly use the generic crate’s semantic authoring layer under the current visibility boundary.

### 8.5 Rust direct host route differs from TypeScript canonical root route

Rust `TuiHost::render(body)` is a convenience method:

```text
TuiHost::render(body)
  → set_desired_view(body)
  → flush_pending()
```

Source: `application/host.rs:1320-1323`.

That is a direct Rust host route. TypeScript canonical `Tui` root publication instead calls the native `setDesiredViewRef` after a TypeScript retained materialization transaction. Both converge on the same native desired-root/frame pipeline, but their authoring and ownership layers differ.

### 8.6 Native semantic construction is Rust-owned after ABI crossing

For a TS text/axis/grid node:

```text
TS semantic node
  → generated N-API call
  → Rust view_abi implementation
  → private Rust factory function
  → NativeViewRuntime.publish
  → NativeRef
```

Examples:

- spacer: `view_spacer_create_impl` calls `view_spacer` and `runtime.publish` (`tui/view_abi.rs:2321-2349`);
- axis: `view_axis_create_buffer_impl` resolves child refs and builds the private semantic view (`tui/view_abi.rs:2609-2642`, continuing through constructor);
- grid: parser explicitly moves rows into the shared placement factory (`tui/view_abi.rs:3047-3055`);
- text cstring: `view_text_create_cstring_impl` decodes payload, resolves style and publishes private Rust view (`tui/view_abi.rs:4045-4075`).

No complete semantic object is serialized from TypeScript into Rust. TypeScript sends generated ABI scalars, buffers, strings and existing child refs.

### 8.7 Desired and visible roots are separate authority planes

The TS root boundary and Rust host both maintain desired versus visible state:

- TS boundary: `desiredRef`, `visibleRef`, revisions;
- Rust host: desired structural revision, visible structural revision, pending/committed epochs;
- host frame candidate: candidate frame, candidate epoch, candidate structural revision;
- backend receipt: asynchronous physical presentation acknowledgement.

This is why a successful `setDesiredViewRef` does not necessarily mean pixels are visible, and why a frame preparation failure must preserve the previous visible frame while retaining a retryable desired epoch.

### 8.8 Application/kernel and TUI composition are distinct retained layers

The Rust `TuiApp`/`RunningApp` kernel has its own:

- application state;
- action queue;
- `view` callback;
- scene;
- component registry;
- dirty/body-dirty state.

The TypeScript `Tui` canonical producer has a separate retained execution runtime. The native host bootstrap uses the Rust kernel, but TypeScript canonical render publication updates the native scene body through the desired-root host seam. They converge at `HostRunning::prepare_frame_with_states` and `SceneHost`.

This is a real layering distinction, not evidence that the two retained execution systems are interchangeable.

---

## 9. Open questions and coverage gaps

1. **Canonical exact-root usage**
   - The exact-root helper and comments exist, but canonical deferred `Tui.render` does not call it in the inspected source.
   - It remains unclear whether a different package-level integration invokes `RetainedRootBoundary.renderExact` indirectly outside `packages/iyon-tui/src`.

2. **External Rust authoring**
   - The crate root deliberately makes application/presentation/scene modules crate-private.
   - The repository contains internal Rust factory authoring and hidden native binding authoring, but no external Rust consumer package was found in the inspected repository.
   - A complete external-consumer claim would require searching outside this repository.

3. **Native ABI generated bodies**
   - Generated ABI files were indexed but not exhaustively read line-by-line.
   - The handwritten Rust ABI implementation and generated TypeScript call wrappers were inspected for the composition-root functions.

4. **Real-terminal receipt behavior**
   - The source clearly distinguishes headless immediate completion from real asynchronous presentation receipts.
   - No live terminal or backend receipt was executed in this investigation.

5. **Cross-plane identity instrumentation**
   - Separate counters exist for execution scopes, semantic/native materialization and wake/frame commits.
   - There is no single built-in trace record that correlates one public `Tui.render` call with execution scope ID, NodeId, NativeRef, desired revision, visible revision and physical frame revision.

6. **History/content-heavy composition roots**
   - The root path supports optional History and ContentPort attachment validation, but content projection, stream, projection and History internals were outside the primary scope.
   - The frame path was followed far enough to establish where those systems enter (`SceneHost`, content provider, History projection and paint), not to census their complete semantics.

7. **Failure after desired commit**
   - Source comments state that frame failure leaves the newly accepted desired revision retryable and does not roll desired sideband state back.
   - Exact behavior under all combinations of a failed real backend receipt, newer desired roots, and deferred root supersession is implemented across `HostInner`, wake broker and root boundary, but was not dynamically exercised here.

8. **Parent-added documentation versus source**
   - The atlas documentation is not source baseline. Any integrated report should continue reconciling its historical descriptions against the current files above rather than treating earlier architecture claims as implementation evidence.

---

## 10. Evidence appendix

### 10.1 Primary TypeScript files inspected

#### Public/API and semantic authoring

- `packages/iyon-tui/src/index.ts`
  - public exports: `View`, `Scene`, `Tui`, `defineView`, `state`, controls and presentation types
- `packages/iyon-tui/src/api/view/view.ts`
  - `View`
  - `ChildrenBuilder`
  - `GridBuilder`
  - semantic constructors/modifiers
  - `isRetainedConstruction`
  - semantic NodeId allocation
- `packages/iyon-tui/src/api/view/scene.ts`
  - `SceneContract`
  - `SceneProducer`
  - `Scene`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - semantic node representation and attachment/derivation helpers

#### Composition

- `packages/iyon-tui/src/composition/define-view.ts`
  - `ViewComponentType`
  - `ViewComponent`
  - `defineView`
- `packages/iyon-tui/src/composition/execution.ts`
  - `RetainedExecutionScope`
  - `RetainedExecutionRuntime`
  - `OwnedBuilderRoot`
  - `invokeComponent`
  - `mountExistingRoot`
  - `invokeChild`
  - `evaluateIntoPendings`
  - `stagePublicationsRecursive`
  - `commitScope`
  - `abortBatch`
  - `detachRoot`
- `packages/iyon-tui/src/composition/execution-context.ts`
  - active execution scope/child-owner context
- `packages/iyon-tui/src/composition/child-owner.ts`
  - `ChildOwnerState`
  - `KeyGroup`
- `packages/iyon-tui/src/composition/compose.ts`
  - retained semantic slot reuse and composition helpers
- `packages/iyon-tui/src/composition/tracked-state.ts`
  - `StateSource`
  - `State`
  - `state`
- `packages/iyon-tui/src/composition/publication.ts`
  - structural publication interfaces
- `packages/iyon-tui/src/composition/persistent-seq.ts`
  - persistent sequence implementation

#### Runtime and scheduling

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `Tui`
  - `TuiRuntime`
  - `Tui.open`
  - `renderCanonical`
  - `renderDirect`
  - `prepareRootPublication`
  - `flush`
  - close/exit
- `packages/iyon-tui/src/runtime/wake-broker.ts`
  - `EnvironmentWakeBroker`
  - `RuntimeHostRegistration`
  - native pending-host drain
  - commit/error trace
- `packages/iyon-tui/src/runtime/access.ts`
  - runtime access bridge
- `packages/iyon-tui/src/runtime/environment.ts`
  - environment ownership
- `packages/iyon-tui/src/runtime/attachments.ts`
  - semantic attachment prepare/commit/visible/dispose
- `packages/iyon-tui/src/runtime/native-resource-registry.ts`
  - native resource ownership and host invalidation
- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness`

#### Structural/native transport

- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeTuiHostContract`
  - `NativeViewStateContract`
  - `NativeHistoryContract`
  - `NativeViewSlotContract`
  - `NativeScrollPaneContract`
  - native artifact loading
- `packages/iyon-tui/src/transport/native/resources.ts`
  - native handle/resource resolution
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `NativeViewAbiSession`
  - `nativeViewAbiSession`
  - retained transient materialization helpers
  - path/edit helper definitions
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `MaterializeTx`
  - `ensureNative`
  - `ensureSemanticNative`
  - materializer dispatch
  - derivation fast paths
  - stale recovery
  - `renderExactRoot`
  - `RetainedRootBoundary`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - generated N-API ABI wrappers
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
  - generated native ABI handle contract

### 10.2 Primary native/Rust files inspected

#### Native addon

- `crates/iyon-tui-native/src/lib.rs`
  - addon module boundary and public native exports
- `crates/iyon-tui-native/src/tui.rs`
  - `NativeTuiHost`
  - `NativeHistory`
  - N-API forwarding
  - `set_desired_view_ref`
  - `flush_pending_hosts`
  - `dispose`
  - `exit`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - `NativeViewRuntime`
  - `NativeViewAbiSession`
  - `view_render_ref_impl`
  - `host_render_ref_impl`
  - `view_ref_for_node_id_impl`
  - View constructors
  - `view_release_many_impl`
  - semantic-cache-first behavior
- `crates/iyon-tui-native/src/generated/*`
  - indexed generated ABI/schema/conformance files; generated bodies were not treated as hand-authored ownership

#### Rust composition/host

- `crates/iyon-tui/src/lib.rs`
  - crate/module visibility and Rust authoring boundary
- `crates/iyon-tui/src/application/app.rs`
  - `App`
  - `App::new`
  - `with_theme`
  - `with_history`
- `crates/iyon-tui/src/application/kernel.rs`
  - `RunningApp::prepare_frame_with_states`
  - application view callback update
- `crates/iyon-tui/src/application/host.rs`
  - `TuiHost`
  - `TuiHost::open_in_environment`
  - `set_desired_view`
  - `flush_pending`
  - `render`
  - `flush_for_environment`
  - `render`
  - `prepare_frame_with_content`
  - `present_frame`
  - `commit_frame`
  - `close`
  - `exit`
- `crates/iyon-tui/src/presentation/mod.rs`
  - private presentation modules
  - hidden `presentation::binding`
- `crates/iyon-tui/src/presentation/factory.rs`
  - internal Rust semantic authoring helpers
- `crates/iyon-tui/src/presentation/ir.rs`
  - private Rust semantic IR
- `crates/iyon-tui/src/scene/host.rs`
  - SceneHost resolution, incremental paths, layout/paint integration
- `crates/iyon-tui/src/scene/layout.rs`
  - resolved scene layout orchestration
- `crates/iyon-tui/src/presentation/layout/engine.rs`
  - measurement/preparation/placement
- `crates/iyon-tui/src/presentation/layout/tree.rs`
  - `LayoutTree` and layout node ownership/indexes
- `crates/iyon-tui/src/presentation/paint/view.rs`
  - `ViewPainter`
  - `PaintCache`
  - full/incremental paint

### 10.3 Key exact symbol ranges

| Path | Symbols/ranges |
|---|---|
| `packages/iyon-tui/src/runtime/runtime.ts` | `TuiRuntime` 59-90; constructor 143-215; root publication 224-277; open 319-341; render 399-548; close/exit 817-877 |
| `packages/iyon-tui/src/composition/define-view.ts` | `defineView` 55-65 |
| `packages/iyon-tui/src/composition/execution.ts` | `RetainedExecutionScope` 109-218; runtime mount/invalidation/flush 328-604; evaluation/publication/commit 627-781; child invocation 941-1041; `OwnedBuilderRoot` 1198-1253 |
| `packages/iyon-tui/src/composition/tracked-state.ts` | State source/read/write 41-92; public state 104-136 |
| `packages/iyon-tui/src/api/view/view.ts` | Node identity 145-158; constructors 249-362; modifiers 365-530 |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | `MaterializeTx` 213-340; materializer/recovery 415-1120; `ensureNative` 1153-1220; derivations 1232-1366; exact root 1391-1463; `RetainedRootBoundary` 1572-2057 |
| `packages/iyon-tui/src/runtime/wake-broker.ts` | broker registration/mark/flush/drain/commit 127-375; registration implementation 427-470 |
| `crates/iyon-tui-native/src/tui.rs` | `NativeTuiHost` 603-1028; desired root 655-672; host drain 683-740 |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | host/view render 1738-1777; constructors 2287 onward, 2609 onward, 3059 onward, 3616 onward, 3702 onward, text 4045 onward |
| `crates/iyon-tui/src/application/host.rs` | host creation 961-1044; desired root 1094-1120; direct render 1320-1323; flush/render 1671-2241; frame preparation 2365-2426 |
| `crates/iyon-tui/src/application/kernel.rs` | `prepare_frame_with_states` 667-702 |
| `crates/iyon-tui/src/scene/host.rs` | `render_at_with_states` 1036-1135; stable resolution 1167 onward; paint 2053-2240 |
| `crates/iyon-tui/src/scene/layout.rs` | layout orchestration 30-54 |
| `crates/iyon-tui/src/presentation/layout/engine.rs` | layout entry and measure/prepare/place 45-143 |
| `crates/iyon-tui/src/presentation/paint/view.rs` | `PaintCache` 162-222; tree/component/subtree painting 749-888 |

### 10.4 Generated/manifest evidence

- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - generated from `tools/tui-abi/view_abi.toml`;
  - schema/generator hashes in header;
  - generated wrappers for `hostRenderRef`, `viewSpacerCreate`, row/column constructors, buffer constructors, text constructors, release and NodeId promotion.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
  - opaque `NativeViewAbiHandle`.
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
  - assignment 30 scope and goal.
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
  - repository source manifest, indexed for coverage.

### 10.5 Files indexed but not comprehensively read for this trace

The following were relevant but outside the primary composition-root route and were indexed or sampled rather than read end-to-end:

- most content/stream/projection implementation files;
- most History implementation files;
- most control-specific files beyond the projection factory seam;
- generated ABI bodies and generated native export tables;
- benchmark-only files;
- broad test fixture trees;
- unrelated terminal/input modules.

Their role is represented only where the composition-root frame path directly crosses them.