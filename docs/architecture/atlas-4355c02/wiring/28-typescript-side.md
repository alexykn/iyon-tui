# 28 — TypeScript-side

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `packages/`
- Assignment: actual TypeScript module/package graph, API/composition/runtime/transport relationships, consumer package boundaries, and retained states.
- Investigation mode: read-only static inspection. No source, configuration, generated artifact, or documentation changes were made.
- No tests, type checks, benchmarks, native builds, or runtime smoke commands were executed by this investigation.

The report contract and atlas README were inspected first. `PRE-V5-ARCHITECTURE-REPORT.md` was read for its evidence expectations and current-state inventory requirements. Its migration/disposition suggestions are treated as historical context only; this report describes the current TypeScript source and does not make V5 disposition decisions.

### Framework boundary

The TypeScript package is a generic terminal UI framework. It contains:

- generic semantic `View` construction and presentation;
- retained composition and state invalidation;
- native handles for generic controls;
- structural/state/content transport;
- terminal runtime lifecycle, scheduling, input, output, and inspection;
- generic History, text sources, funnels, connectors, annotations, and smoothing.

No Iyon agent/application concepts were found in the package source or the consumer fixture. There are no product/plugin packages under `packages/` in this baseline.

### Evidence categories

- **Current-source fact:** directly visible in the TypeScript source, package manifests, generated files, or fixture.
- **Static inference:** dependency/lifetime behavior inferred from imports, constructors, fields, and control flow.
- **Historical/documentary context:** comments or architecture documents describing intended behavior; source remains authoritative.
- **Unknown:** not provable without executing code, inspecting the native implementation, or inspecting packages outside this repository.

### Physical LOC methodology

The source was inspected through repository file indexing and targeted source searches. The following are rounded physical-line estimates from file spans and module sizes, not executed `wc -l` results:

| Area | Approximate physical LOC | Method/qualification |
|---|---:|---|
| `packages/iyon-tui/src/api/` | ~5,100 | Rounded file-span estimate; includes public API and private semantic helpers |
| `packages/iyon-tui/src/composition/` | ~2,800 | Includes the ~1,250-line retained execution substrate |
| `packages/iyon-tui/src/runtime/` | ~2,100 | Includes runtime, wake broker, attachments, environment, registries |
| `packages/iyon-tui/src/transport/` | ~5,500–6,000 | Includes structural DAG/ABI, native adapters, state/content transport, generated support |
| `packages/iyon-tui/src/testing/` | ~155 | `AppHarness` facade |
| `packages/iyon-tui` production scripts/bench support | ~1,500+ | Scripts and benchmark sources are separate from the runtime API |
| `packages/iyon-tui/tests/` | ~6,000–7,000 | Rounded estimate across the listed suites |
| `packages/tui-consumer-fixture/src/` and tests | ~600 | Consumer and scoped-invalidation fixture |
| Generated TypeScript/JSON | ~1,000–2,000 | ABI calls, conformance, state envelope, manifests/schema |

The largest uncertainty in these estimates is generated ABI and test-file accounting. No claim of exact LOC is made.

---

## 1. Responsibility and structure

## 1.1 Package topology

There are exactly two workspace packages under `packages/`:

| Package | Manifest | Public/private status | Dependency boundary |
|---|---|---|---|
| `@iyon/tui` | `packages/iyon-tui/package.json` | Framework package | No runtime dependencies; `bun-types` is a dev dependency |
| `@iyon/tui-consumer-fixture` | `packages/tui-consumer-fixture/package.json` | Private external-consumer-shaped fixture | Depends only on `@iyon/tui: workspace:*` |

The repository-root `package.json` also exposes the framework package directly:

```json
{
  "exports": {
    ".": "./packages/iyon-tui/src/index.ts",
    "./testing": "./packages/iyon-tui/src/testing/index.ts",
    "./native-stage": "./packages/iyon-tui/scripts/stage-native.ts"
  },
  "workspaces": [
    "packages/iyon-tui",
    "packages/tui-consumer-fixture"
  ]
}
```

The package-local manifest has the same three exports:

- `.` → `./src/index.ts`
- `./testing` → `./src/testing/index.ts`
- `./native-stage` → `./scripts/stage-native.ts`

The package-local binary is `iyon-tui-native-stage`.

Evidence:

- `/Users/alxknt/github/iyon-n/iyon-tui/package.json:7-17`
- `/Users/alxknt/github/iyon-n/iyon-tui/packages/iyon-tui/package.json:7-13`
- `/Users/alxknt/github/iyon-n/iyon-tui/packages/tui-consumer-fixture/package.json:7-9`

No second published TypeScript package, plugin package, application package, or compatibility package exists under `packages/`.

## 1.2 Production source inventory

### Public API layer: `packages/iyon-tui/src/api/`

#### Errors

- `src/api/errors.ts`
  - `TuiError`
  - `TuiErrorCategory`
  - `tuiError`
  - `asTuiError`
  - `isTuiError`
  - `isTuiCancelledError`
  - Normalizes native error codes into generic categories: invalid handle, disposed handle, validation, terminal, runtime, projection, stream, cancelled.
  - Imported throughout the runtime/transport/control layers.
  - Public error class and helpers are exported from the package root.

#### View and semantic representation

- `src/api/view/view.ts` — approximately 1,146 physical lines
  - Public immutable `View` class.
  - Primitive semantic forms: text, styled text, spacer, diff, row, column, hanging indent, grid, container, clamp, content maximum, component, content host, decorated nodes.
  - Public builders: `ChildrenBuilder`, `GridBuilder`, `GridRowBuilder`.
  - Layout types: `LayoutChild`, `GridTrack`, `GridSpec`, `GridCell`, `GridRow`, `ViewChildren`.
  - Modifier methods: width/height fit/fill, min/max dimensions, padding, colors, style, attributes, border, wrapping, alignment, state attachment, and style state.
  - Retained-composition-aware methods route to `composition/compose.ts` when an active execution scope exists; otherwise they construct ordinary immutable semantic nodes.
  - Internal transport helpers support persistent axis/grid edits and derivation records.

- `src/api/view/semantic-node.ts` — approximately 610 lines
  - Backend-neutral semantic node discriminated union.
  - `SEMANTIC_VIEW_KIND` numeric kind table.
  - Semantic style, color, border, decoration, text span, layout-child, grid, diff, component, content-host, and attachment fields.
  - Semantic node identity and attachment sidecars.
  - Derivation records for text layout, common scalar changes, axis set/splice, and grid cell edits.
  - Persistent sequence overrides for wide axes and grids.
  - `createSemanticViewNode`, `semanticNodeOf`, `installSemanticNode`, `freezeSemanticViewNode`, attachment-reference functions, derivation and sequence override functions.

- `src/api/view/geometry.ts`
  - `Insets` and `InsetsValue`.
  - Small immutable/validated geometry value helper.

- `src/api/view/scene.ts`
  - `SceneContract { body: View; history?: History }`.
  - `SceneProducer = SceneContract | (() => SceneContract)`.
  - Concrete `Scene` wrapper and `Scene.from`.
  - Scene is a root body plus optional History sideband, not a general application object.

- `src/api/view/retained-state.ts` — approximately 236 lines
  - Public `ViewState`.
  - Typed geometry and presentation patch contracts.
  - `setGeometry`, `clearGeometry`, `setPresentation`, `clearPresentation`, `setStyleState`, `clearStyleState`.
  - Maps JS patches into generated state envelopes and native state resource calls.
  - Accepted node kinds are explicitly constrained by `VIEW_STATE_NODE_KINDS`.
  - State is attached to a semantic View by opaque `HandleId`; it is not embedded into the immutable View node.

#### Presentation

- `src/api/presentation/style.ts`
  - `StyleSpec`, `StyleRef`, `StyleSelector`, `StyleStateKey`, `StyleStateValue`.
  - Border and text-attribute contracts.
  - `Style` convenience namespace.
  - Style values are immutable/persistent and validated.
- `src/api/presentation/theme.ts`
  - `Theme`, `themeColor`, color union types, `themeDefinitionFor`.
  - Supports base styles/colors and selector/text-style variants.
  - Theme is caller-supplied presentation data; it does not encode product policy.
- `src/api/presentation/theme-key.ts`
  - `ThemeKey`.
- `src/api/presentation/semantic-style.ts`
  - Internal normalization from public style/theme/text values to semantic style records.
  - `semanticColorFor`, `semanticStyleFor`, `semanticBorderFor`, `semanticTextSpanFor`, `semanticDecorationFor`, clone/merge helpers.
  - Important seam: public style APIs are converted to backend-neutral semantic values before structural transport.

#### Text/content authoring

- `src/api/content/text.ts`
  - `TextSelector`, `TextSpan`.
  - Text roles/parts, annotation selectors, origin/format/language selectors, focus/style-state selector predicates.
  - Text style references are shared with presentation.
- `src/api/content/text-content.ts`
  - `TextContent`, `RawText`.
  - Plain/Markdown/raw content wrapper with origin.
  - `TextContent.render()` currently returns `View.text`, so this small content value is coupled to View authoring.
- `src/api/content/annotations.ts`
  - Generic tags, semantic values, semantic text styles, `Annotations`.
  - `TEXT_SOURCE_ANNOTATION_SCHEMA`.
  - No product-specific annotation names are encoded here.
- `src/api/content/diff.ts`
  - `DiffRange`, `DiffLine`, `DiffHunk`, `DiffRenderer`.
  - Diff values are generic and rendered into a `View`.
- `src/api/content/projection.ts`
  - Minimal public `Projection`, `ProjectionBuilder`, `Smooth`, and `ProjectionSpan` contracts.
  - These are currently very small wrappers/contracts compared with the actual native content implementation.
- `src/api/content/retained.ts` — approximately 810 lines
  - Public retained text source, funnel, port, and connector system.
  - `TextStreamSource`, `TextBlockSource`, `TextFunnel`, `ContentPort`, `ContentConnector`.
  - Source snapshots/stats/mutation contracts, source annotations, retention policy, smoothing options, connector status/failure phases.
  - Deepest public-to-transport coupling in the API directory: imports runtime environment, native resource registry, content FFI, content control transport, structural semantic kinds, and framework handles.

#### Controls and handles

- `src/api/controls/framework-handle.ts`
  - `FrameworkHandle<K>`.
  - `HandleId` is a JS-local nominal identity and is explicitly not a native identifier.
  - Central public handle methods: `id`, `kind`, `disposed`, `dispose`.
  - `ComponentHandle` adds `view(): View`.
  - Delegates raw native resource storage and lifecycle to runtime/transport registries.
- `src/api/controls/history.ts`
  - Public `History` and `HistoryLayout`.
  - Detached caller-created History and Tui-created host-bound History both exist.
  - `push`, `freeze`, `discardLive`, `setLayout`.
  - Native root materialization is performed through `tryRetainedMaterializeRef`.
  - History attachment is one-way and single-host.
- `src/api/controls/output.ts`
  - Opaque generic `Output<T>` channel class.
  - Private constructor prevents fabricated routing identities.
- `src/api/controls/text-input.ts`
  - Public host-bound `TextInput`.
  - Text/cursor/editing API, multiline mode, submitted `Output<string>`, `view()`.
  - Native output channel is cached by input identity.
- `src/api/controls/view-slot.ts` — approximately 420 lines
  - Public retained component slot.
  - Direct View, builder, animation, cycle-boundary animation, and stop-animation modes.
  - Owns a retained root boundary and optionally an `OwnedBuilderRoot`.
  - Requires a Tui-created shared retained execution runtime for builder mode.
- `src/api/controls/scroll-pane.ts`
  - Public retained scrolling component.
  - Direct/builder `setContent`, `followEnd`.
  - Owns a retained root boundary and optional builder root.
  - Scroll state is native-owned and intentionally independent from content rebuilds.

#### Extension traits

- `src/api/extensions/traits/component.ts`
  - Generic `ComponentAdapter`, `ComponentContext`, `ComponentCapabilities`, key/paste events, interaction results, `AsyncComponentAdapter`.
- `src/api/extensions/traits/renderer.ts`
  - `Renderer`, `RenderContext`, `RendererAdapter`.
- `src/api/extensions/traits/projector.ts`
  - `Projector`, `ProjectorAdapter`.
- `src/api/extensions/traits/text-rewriter.ts`
  - `TextRewriter`, `TextRewriterAdapter`.
- `src/api/extensions/traits/text-visitor.ts`
  - `TextVisitor`, `TextVisitorAdapter`.

These trait adapters are public/type-level extension surfaces but are not wired into the retained execution runtime. Search found their direct runtime usage only in trait tests; the package runtime does not automatically turn a `ComponentAdapter` into a native component.

### Composition layer: `packages/iyon-tui/src/composition/`

- `define-view.ts`
  - Public `defineView`.
  - `ViewComponentType` and callable `ViewComponent`.
  - The wrapper invokes `invokeComponent` and returns a normal `View`.
  - Component identity is function-object identity; execution identity is parent-local scope position plus optional key.
- `tracked-state.ts`
  - Public `state<T>` and `State<T>`.
  - Internal `StateSource<T>` tracks active scope reads and publishes invalidations.
  - `Object.is` change discipline.
  - Writes from inside component evaluation reject with `ExecutionError`.
  - Dependency subscriptions are committed only after evaluation commit; aborted evaluations retain the prior dependency set.
- `execution-context.ts`
  - Active execution scope and active child-owner cells.
  - `protocolState.mutating` and `protocolState.internalPublication`.
  - `semanticConstruction.raw`.
  - Keyed child-owner swapping.
  - Deliberately does not import `View`, reducing composition/View cycles.
- `child-owner.ts`
  - `ChildOwnerState` for positional child records and keyed namespaces.
  - `KeyGroup` owns identity only, not execution, scheduling, or output.
  - WIP/committed split for child reconciliation.
- `publication.ts`
  - `PreparedStructuralPublication`.
  - `StructuralPublicationTarget`.
  - `StructuralScopeProjection`.
  - Composition only knows semantic `View` plus prepare/commit/abort; native refs and host objects remain in target implementations.
- `compose.ts` — approximately 778 lines
  - Internal monomorphic retained composition helpers.
  - Dense per-scope semantic slots.
  - Modifier tags are small integer constants.
  - Performs pre-allocation comparisons against normalized semantic records.
  - Exact previous View identity is reused when operation inputs match.
- `persistent-seq.ts` — approximately 309 lines
  - Persistent branching sequence with branch factor 32.
  - `set`, `insert`, `remove`, `splice`, `split`, `concat`, iterators, aggregate flags.
  - Retains structural sharing for wide axis/grid mutations.
  - Counters expose nodes/cloned branches/items iterated.
- `execution.ts` — approximately 1,250 lines
  - `RetainedExecutionScope`.
  - `RetainedExecutionRuntime`.
  - `OwnedBuilderRoot`.
  - Scope semantic table, child reconciliation, keyed groups, dependency promotion, dirty queue, scheduling, three-phase flush, rollback, publication staging, commit, and teardown.
  - This is the primary retained composition/runtime machinery.

### Runtime layer: `packages/iyon-tui/src/runtime/`

- `runtime.ts` — approximately 928 lines
  - Public `TuiRuntime` and `Tui`.
  - Owns native host, dimensions, retained runtime, environment, host registration, root boundary, scene sideband, attachment ledger, runtime error channel, and all Tui-factory handles.
  - Canonical render and direct render are separate routes.
  - Runtime startup, events, flush, resize, key routing, paste routing, theme, History, ViewState, ContentPort, TextInput, ViewSlot, and ScrollPane creation.
  - Close/exit teardown ordering.
- `environment.ts`
  - One runtime environment per JS realm.
  - Shared `NativeResourceRegistry` and `EnvironmentWakeBroker`.
- `wake-broker.ts`
  - One edge-triggered broker per environment.
  - Host registration, pending set, fairness cursor, microtask scheduling, explicit barriers, asynchronous presentation receipt polling, structured error routing.
- `attachments.ts`
  - Semantic tree attachment validation.
  - `AttachmentBindingState`.
  - Desired/visible/superseded attachment lease sets.
  - Duplicate attachment detection and host/environment/node-kind validation.
- `handle-registry.ts`
  - JS handle identity map.
  - Delegates native registration, disposal, release, and disposal-state rollback.
- `native-resource-registry.ts`
  - Re-export seam for native resource registry types/functions used by runtime.
- `access.ts`
  - Private runtime inspection/control access table.
- `events.ts`
  - `OutputEvent`, `TerminateEvent`, `TuiEvent`.

### Transport layer: `packages/iyon-tui/src/transport/`

#### Native artifact and resource transport

- `native/addon.ts`
  - Private native contract.
  - `NativeTuiAddon`, `NativeTuiHostContract`, `NativeHistoryContract`, `NativeViewSlotContract`, `NativeScrollPaneContract`, `NativeTextInputContract`, source/port/connector/state contracts, host epochs and drain reports.
  - Loads a single staged addon using `resolveNativeArtifact`.
  - Checks `nativeVersion()` against `iyon-tui-native/s6`.
- `native/artifact.ts`
  - Platform/architecture artifact resolution.
  - Build identity: `iyon-tui-native/s6`.
  - Both N-API and direct content FFI resolve the same staged `.node` artifact.
- `native/resources.ts`
  - Handle-local weak native map plus authoritative registry lookup.
  - Supports handles with no `HandleId`, notably `Output`.
- `native/resource-registry.ts` — approximately 480 lines
  - Environment-level resource registry.
  - Monotonic HandleId records, WeakRefs, lifecycle (`live`, `disposing`, `disposed`), environment/host ownership, accepted node kinds, generation, prepared/desired/visible lease counts.
  - `PreparedResourceLease` transitions prepared → desired → visible or abort/release.
  - Enforces wrong-environment, wrong-host, node-kind, duplicate-preparation, mounted-disposal, and stale-handle rules.
- `native/factories.ts`
  - Native creation wrappers for History/content source/native objects.

#### Structural transport

- `structural/ir.ts`
  - Generated-schema-backed numeric maps for View kinds, layout child kinds, tracks, wrap/alignment, diff kinds and terminations.
  - Private numeric transport IR, not package-root API.
- `structural/encoding.ts`
  - Converts semantic records to ABI words, masks, style/color atoms, dimensions, tracks, alignments, diff metadata, decoration fields.
- `structural/style-lowering.ts`
  - Public style/theme/text values → private transport style/border/text nodes.
- `structural/policy.ts`
  - Native child/axis/text/diff limits and direct-transport policy constants.
- `structural/component-id.ts`
  - Framework HandleId → native component identity.
- `structural/retained-path.ts`
  - Native path lineage and text-layout transaction types/helpers retained for specialized transport calls.
- `structural/native-view-abi.ts`
  - ABI session bootstrap and metadata validation.
  - Retained materialization wrapper, root/ref acquisition, native axis creation/edit paths, native path/edit transactions, and ref release.
  - Generated ABI calls are imported from `abi/structural/generated/view_calls.ts`.
- `structural/retained-dag.ts` — approximately 2,138 lines
  - Main retained semantic DAG → NativeRef implementation.
  - Native hint sidecar, materialization transaction, per-depth axis/grid scratch, byte scratch, style cache sidecar, structural counters, stale-ref retry, direct materializers for every semantic node kind, exact-root fast path, and `RetainedRootBoundary`.
  - Production structural route; no complete-object fallback path is selected on retained refusal.
  - Owns root desired/visible lease transitions and host publication preparation.

#### State transport

- `state/control.ts`
  - Normalizes ViewState patches and clear operations.
  - Converts public geometry/presentation values into generated envelopes.
- `state/generated/state_envelope.ts`
  - Generated fixed-mask/fixed-word/fixed-string state ABI.
  - Geometry: 10 properties, 14 words.
  - Presentation: 7 properties, 5 words and 14 strings.
  - Generated source header says it is derived from `tools/tui-abi/view_abi.toml`.

#### Content transport

- `content/control.ts`
  - Small native control calls for text Source, ContentPort, and Connector.
  - Does not serialize bulk source payloads.
- `content/ffi.ts`
  - Bun direct FFI data plane for append/replace/clear/seal/head-truncate.
  - Validates content ABI metadata and fixed annotation payloads.
  - Encodes generic tags, styles, atomic and point annotations.
  - Source identities are `(environmentSlot, environmentGeneration, sourceSlot, sourceGeneration)`.
  - Returns source revision, environment wake epoch, and schedule-drain flag.
- `content/abi.ts`
  - Content ABI constants/status mappings.

#### Generated structural ABI

- `abi/structural/generated/view_abi.ts`
- `abi/structural/generated/view_abi_conformance.ts`
- `abi/structural/generated/view_calls.ts`
- `abi/structural/generated/view_abi_manifest.json`
- `abi/structural/schema/view-kind-codes.json`

These are generated support and should not be treated as public authoring modules.

### Testing facade

- `src/testing/index.ts` — approximately 155 lines
  - Public subpath `@iyon/tui/testing`.
  - `AppHarness` wraps a real `Tui` opened in headless mode.
  - Adds deterministic `pressKey`, `paste`, `advance`, screen rows, native History rows, style/cell inspection, `exited`, and deterministic clock.
  - Uses private `runtimeAccess` to drive native/test-only operations.
  - `render()` calls the underlying Tui render and then deterministic zero-time advancement.

### Package scripts and benchmark/support sources

- `scripts/stage-native.ts`
  - Builds/stages `iyon-tui-native`.
  - Uses `ION_NATIVE_FEATURES`.
  - Validates addon build identity, removed native surface, content symbols, direct-FFI qualification symbols, and content ABI artifact path.
- `scripts/smoke-native.ts`
  - Imports public root `TextStreamSource`, `TextFunnel`, `View`, and the testing facade.
  - Creates source/port/connector, activates it, renders `View.content(port)`, appends source text, flushes, and checks screen output.
  - This is a useful package-level integration route, but it was not executed here.
- `bench/`
  - Contains PERF-12/PERF-13 workloads, generated ABI cases, retained trace scripts, and content benchmarks.
  - These are benchmark consumers of private/runtime instrumentation, not package consumers.

---

## 1.3 Public package surface

`packages/iyon-tui/src/index.ts` is the intentional package-root API. It exports:

### Public values

- Errors: `TuiError`, `asTuiError`, `isTuiCancelledError`, `isTuiError`, `tuiError`
- View: `View`, `ChildrenBuilder`, `GridBuilder`, `GridRowBuilder`, `ViewState`
- Composition: `defineView`, `state`
- Geometry: `Insets`
- Presentation: `Style`, `StyleRef`, `StyleSelector`, `StyleSpec`, `StyleStateKey`, `StyleStateValue`, `Theme`, `ThemeKey`, `themeColor`
- Text/content: `TextSelector`, `TextSpan`, `TextContent`, `RawText`, `Annotations`, `Projection`, `ProjectionBuilder`, `Smooth`, `DiffRange`, `DiffLine`, `DiffHunk`, `DiffRenderer`
- Controls: `History`, `TextInput`, `ContentConnector`, `ContentPort`, `TextBlockSource`, `TextFunnel`, `TextStreamSource`, `Scene`
- Runtime: `Tui`

### Public types

- Theme/style/color/border contracts.
- Component traits and `ComponentHandle`.
- Layout/view types.
- History/output/event types.
- Renderer/projector/text visitor/rewriter contracts.
- `TerminalMetadata`, `TuiOpenOptions`, `TuiRuntime`.
- Content source/funnel/connector/snapshot/stats/annotation contracts.
- ViewState patch contracts.
- `ViewComponent`, `State`, `ViewSlot`, `ScrollPane`.

Notably, `FrameworkHandle` appears in a root `export type` list, not as a runtime value export. Internal execution classes, native ABI sessions, retained DAG classes, attachment ledgers, wake broker, and resource registry are not exported from the package root.

The only documented secondary package export is `@iyon/tui/testing`, whose runtime value is `AppHarness`.

---

## 2. Types, APIs and contracts

## 2.1 Identity types

There are several intentionally distinct identity domains:

| Identity | Definition/owner | Meaning | Lifetime |
|---|---|---|---|
| JS `HandleId` | `runtime/handle-registry.ts` / `FrameworkHandle` | Monotonic framework-handle identity | Until handle registration is retired |
| Semantic NodeId | `api/view/view.ts` / `semantic-node.ts` | Immutable semantic node identity | Node object lifetime; monotonically allocated |
| Native View `NativeRef` | Native ABI / retained DAG | Native retained physical value/reference | Lease-controlled |
| Execution scope id | `composition/execution.ts` | Logical component instance identity | Scope mount through disposal |
| ComponentId | Native component mapping | Native input/interaction routing identity | Native component lifetime |
| Source identity | Native source + environment generation | Content source identity and stale-source protection | Native source lifetime |
| Host identity/epochs | Native host + wake broker | Desired/visible/frame transaction coordinates | Host lifetime |
| History unit number | Native History | Ordered scrollback unit identity | History lifetime |

The code repeatedly prevents identity conflation. For example:

- `HandleId` is documented as not being a native identifier (`framework-handle.ts:9-16`).
- Execution scope identity is documented as distinct from NodeId and physical resource (`execution.ts:105-108`).
- Keys are local composition identity, not NodeIds or global IDs (`child-owner.ts:16-17`).
- Semantic native hints are weak and generation-scoped, not lease ownership (`retained-dag.ts:52-59`).

## 2.2 Public semantic `View` contract

`View` objects are immutable and frozen after semantic node installation. Public construction supports:

- text/styled text;
- spacer;
- row/column with dense children and track metadata;
- hanging indent;
- grid;
- diff;
- container/clamp/content maximum;
- component handles;
- content-port host nodes;
- decorated nodes.

The public fluent modifiers preserve immutable semantics. In a retained execution scope, the same methods are intercepted by composition helpers, which compare normalized semantic arguments against the current per-scope slot before allocating.

The semantic node union in `semantic-node.ts` is the contract consumed by:

- composition equality;
- attachment traversal;
- structural retained materialization;
- state attachment;
- style lowering/encoding;
- native ABI constructors.

## 2.3 `defineView` and retained composition contract

`defineView` is the only public retained component constructor. Its documented behavior in `composition/define-view.ts:48-64` is:

1. A callable component wrapper is created.
2. Calling it inside an active execution scope invokes `invokeComponent`.
3. Invocation reconciles by parent-local ordinal and component token.
4. Optional `View.key` changes the child identity namespace.
5. Same own-key set plus `Object.is` equality for each prop causes a body skip.
6. Immutable nested props are compared by identity, not deep value.
7. Component bodies must return synchronous `View` values.

No source transform, compiler plugin, global site IDs, or feature flag is involved.

## 2.4 Tracked `State<T>` contract

`State<T>` is intentionally minimal:

```ts
interface State<T> {
  readonly value: T;
  set(value: T): void;
  update(update: (previous: T) => T): void;
}
```

Contractual behavior:

- Reading `.value` while a scope is evaluating links the scope to the state source.
- Reading outside evaluation is ordinary untracked access.
- `set`/`update` use `Object.is`; unchanged values do not publish.
- Writes from component bodies reject deterministically.
- State writes invalidate only subscribed live scopes.
- Pending dependency sets are promoted only after a successful scope commit.
- Aborted evaluation leaves the prior committed subscription set in place.
- Public `trackedStateSubscriberCount` exists as diagnostics/test support but is not package-root exported.

Evidence: `composition/tracked-state.ts:5-18`, `:41-82`, `:95-136`.

## 2.5 Retained execution scope contract

`RetainedExecutionScope` fields include:

- `id`, `runtime`, `parent`, `depth`, `key`, `type`, `ordinal`;
- current/pending props;
- current/pending output;
- `state`, `mounted`, `dirty`, `disposed`;
- `ChildOwnerState`;
- semantic slot table;
- committed/pending dependency sets;
- publication target/projection/projected output/staged publication.

The distinction between `publicationTarget` and `projection` is consequential:

- A root builder or control-owned builder publishes to its own boundary.
- A child component may be projected into a parent through a `StructuralScopeProjection`.
- The component execution identity does not itself own native refs; its projection target does.

## 2.6 Structural publication contract

`composition/publication.ts` defines the central seam:

```ts
interface StructuralPublicationTarget {
  preparePublication(output: View): PreparedStructuralPublication | undefined;
  needsPublication?(output: View): boolean;
}

interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}
```

Composition owns evaluation and transaction ordering. Targets own native structural details. Current target implementations include:

- Tui root `RetainedRootBoundary`;
- ViewSlot boundary;
- ScrollPane boundary;
- scope projections created by the Tui retained runtime.

## 2.7 ViewState contract

`ViewState` is a separate retained native state plane from composition `State<T>`:

- Composition `State<T>` controls which JS component scopes are dirty.
- `ViewState` controls native retained geometry/presentation overrides on one attached View occurrence.
- Semantic View nodes only carry an opaque state `HandleId`.
- Native capability checks happen at attachment preparation and state mutation.
- `ViewState` patches are encoded into generated fixed envelopes.

Geometry fields include width, height, padding, min/max dimensions, gap, alignment, and border edges. Presentation fields include foreground/background, border color/style/glyphs, text attributes, and style.

## 2.8 Content source/funnel/port/connector contracts

`api/content/retained.ts` exposes a generic content pipeline:

```text
TextStreamSource / TextBlockSource
        +
     TextFunnel
        ↓
    ContentPort.connect
        ↓
 ContentConnector
        ↓
 View.content(ContentPort)
        ↓
 Native content host / projection / paint
```

Source contracts include:

- Source ID and generation;
- environment slot/generation;
- content generation;
- source revision and source coordinates;
- text and annotation snapshots;
- retention statistics;
- stream append/replace/clear/seal/truncate operations.

Funnel contracts are immutable and support:

- plain/Markdown/diff/ANSI mode;
- word/grapheme/no-wrap;
- hyperlink enablement;
- immediate or smooth delivery;
- tick interval, spring, and min/max unit rates.

A `ContentPort` owns Connector membership and is attachable only to `contentHost` semantic nodes. A `ContentConnector` links exactly one Source, Funnel, and Port. Connector status retains intermediate phases such as waiting-for-mount, activation-pending, active, failed, blocked-geometry, unsupported-backend, disposing, and disposed.

---

## 3. Dependency and ownership map

## 3.1 Forward dependency graph

The principal production graph is:

```text
Public consumer
    │
    ├── @iyon/tui/src/index.ts
    │       │
    │       ├── api/view/View, Scene, ViewState
    │       ├── api/presentation/style/theme
    │       ├── api/content/text/retained/diff
    │       ├── api/controls/History/TextInput/ViewSlot/ScrollPane
    │       ├── composition/define-view + state
    │       └── runtime/Tui
    │
    └── @iyon/tui/testing
            └── testing/AppHarness
                    └── runtime/Tui
```

Within the framework:

```text
api/View
  ├── composition/compose
  │     ├── composition/execution-context
  │     ├── composition/execution
  │     ├── semantic-style
  │     └── semantic-node
  │
  ├── semantic-node
  │
  └── transport structural types
        [mostly type-only imports]

runtime/Tui
  ├── native/addon
  ├── native/resource registry/resources
  ├── runtime/environment
  ├── runtime/wake-broker
  ├── runtime/attachments
  ├── composition/execution
  ├── api View/Scene/controls/content/state/theme
  └── transport structural/content/state

transport/structural/retained-dag
  ├── semantic-node
  ├── api/View identity helpers
  ├── structural encoding/IR/policy
  ├── native resources
  └── generated structural ABI calls

api/content/retained
  ├── FrameworkHandle + handle registry
  ├── content/control
  ├── content/ffi
  ├── native/resource registry
  ├── runtime/environment
  └── semantic node kind validation

api/ViewState
  └── transport/state/control
        └── generated state envelope
```

## 3.2 Reverse dependency highlights

### `runtime/Tui` reverse dependencies

`Tui` is the composition root and is consumed by:

- package root `index.ts`;
- `testing/AppHarness`;
- package scripts through the public/testing APIs;
- all public Tui factories and canonical render operations.

It is not imported by semantic API modules as a whole. This prevents the API layer from requiring a live terminal to construct ordinary values.

### `api/view/view.ts` reverse dependencies

`View` is consumed by nearly every layer:

- public package root;
- composition helpers and execution;
- Scene;
- controls;
- content text/diff;
- semantic node;
- attachment validation;
- structural retained DAG;
- native View ABI;
- testing types.

This is the central semantic value type and consequently the broadest dependency hub.

### `semantic-node.ts` reverse dependencies

Semantic nodes are consumed by:

- composition equality and slot updates;
- View construction;
- attachment traversal;
- state attachment metadata;
- structural encoding and retained materialization;
- native ABI ref paths;
- style lowering.

The semantic node is the main internal IR between authoring/composition and structural transport.

### `native/resource-registry.ts` reverse dependencies

The registry is shared by:

- `FrameworkHandle` through `handle-registry`;
- all native-backed controls;
- `ViewState`;
- content Ports and Connectors;
- structural attachment preparation;
- Tui teardown;
- diagnostics.

It deliberately does not understand content/state/component semantics beyond generic resource kind and accepted node-kind metadata.

### `composition/execution.ts` reverse dependencies

The retained execution runtime is consumed by:

- `defineView`;
- public `state` indirectly through active scope linkage;
- Tui root canonical render;
- ViewSlot builder mode;
- ScrollPane builder mode;
- component projection creation.

No consumer package imports it directly. The fixture only calls public `defineView`, `state`, `View.key`, and `TuiRuntime.render`.

## 3.3 Ownership/lifetime map

| Object/resource | Created by | Retained by | Destroyed/released by |
|---|---|---|---|
| `Tui` | `Tui.open` | caller | caller `close`/`exit` |
| Native host | `Tui.open` | `Tui`/environment broker | `Tui.close` or failed `exit` cleanup |
| Root retained runtime | Tui constructor | Tui | `disposeRetainedExecution` |
| Canonical root builder | first `render(() => ...)` | Tui | direct render replacement or Tui teardown |
| `RetainedExecutionScope` | retained runtime | parent owner or runtime root list | owner reconciliation/disposal/runtime teardown |
| Semantic View | caller/composition | semantic node references, component slots, boundaries | GC after references drop |
| Native ViewRef | retained DAG/native ABI | boundary leases/native parent/control/history | root replacement, frame visibility, boundary close |
| ViewSlot | `Tui.createViewSlot` | Tui owned-handle set and caller references | explicit dispose/Tui close |
| ScrollPane | `Tui.createScrollPane` | Tui owned-handle set and caller references | explicit dispose/Tui close |
| TextInput | `Tui.createTextInput` | Tui owned-handle set | explicit dispose/Tui close |
| ContentPort | `Tui.contentPort` | Tui owned-handle set and semantic attachment leases | unmount then dispose/Tui teardown |
| ContentConnector | Port.connect | Port membership + native connector | explicit disposal, deferred native removal, host teardown |
| Text Source | `TextStreamSource.create`/`TextBlockSource.create` | caller/environment registry | caller dispose; source may outlive hosts |
| History | `new History` or Tui factory | caller/Tui owner and host sideband | caller/Tui teardown, with attach-once rules |
| ViewState | `Tui.viewState` | Tui owned-handle set and attachment leases | unmount then dispose/Tui teardown |
| Output | TextInput.submitted | weak input map and caller | native resource release with input/native teardown |
| Theme cache refs | retained DAG style sidecar | generation/runtime sidecar | theme change reset or generation replacement |

## 3.4 Structural attachment ownership

`runtime/attachments.ts` maintains three concepts:

1. **Prepared leases:** candidate tree has been validated and resolved.
2. **Desired leases:** candidate structural publication has committed, but host frame may not yet be visible.
3. **Visible leases:** host has acknowledged the frame.

`AttachmentBindingState` keeps superseded desired revisions while asynchronous backend presentation is in flight. This avoids dropping resources that a submitted but not-yet-visible frame still references.

The same semantic handle cannot be attached twice in one candidate tree. Ordinary semantic DAG reuse is allowed, but attachment identity reuse is rejected. This is an important distinction between immutable semantic sharing and native attachment ownership.

## 3.5 Root publication ownership

`RetainedRootBoundary` is the shared root-lease protocol for:

- Tui root;
- ViewSlot root;
- ScrollPane root.

The old root stays leased while the new root is prepared. On successful commit:

1. new root is installed;
2. desired lease is promoted;
3. old desired/visible ownership is released at the appropriate transition;
4. host visibility promotion happens separately when the wake broker reports a frame commit.

---

## 4. Execution paths and state transitions

## 4.1 Canonical retained render path

The principal production route is:

```text
consumer:
  tui.render(() => new Scene(App({})))
        │
        ▼
Tui.render
  └── renderCanonical
        │
        ├── producer = () => Scene.from(builder())
        ├── stageHistoryBinding(scene.history)
        └── rootBuilder.start/replaceProducer
              │
              ▼
OwnedBuilderRoot
  └── RetainedExecutionRuntime.mountExistingRoot/update
        │
        ▼
RetainedExecutionScope evaluation
  ├── begin semantic slot table
  ├── begin child-owner WIP
  ├── push active scope/child owner
  ├── invoke component body
  ├── track State reads
  ├── reconcile defineView children and View.key groups
  ├── validate synchronous View result
  └── pop active context
        │
        ▼
RetainedExecutionRuntime phase 2
  ├── recursively stage child publications
  ├── prepare structural publication target
  ├── prepare semantic attachments
  └── abort entire batch if any prepare fails
        │
        ▼
RetainedExecutionRuntime phase 3
  ├── commit descendants before parents
  ├── promote pending props/output/children/dependencies
  ├── commit publication callbacks
  └── release removed scopes after promotion
        │
        ▼
RetainedRootBoundary / native-view-abi
  ├── ensureSemanticNative
  ├── hint/transaction-local cache lookup
  ├── NodeId promotion when eligible
  ├── direct generated ABI materialization for new nodes
  └── host desired structural install
        │
        ▼
EnvironmentWakeBroker
  ├── pending host
  ├── microtask or explicit drain
  ├── native flushPendingHosts
  ├── asynchronous presentation receipt if needed
  └── Tui.commitVisibleAfterDrain
        │
        ▼
Visible host frame
```

Evidence:

- `runtime/runtime.ts:391-496`
- `composition/execution.ts:627-780`
- `composition/execution.ts:494-542`
- `transport/structural/retained-dag.ts:1-22`, `:1153-1200`, `:1572+`

## 4.2 First canonical render

On the first function-form render:

1. `Tui.render` selects `renderCanonical`.
2. Tui drains existing retained work and rejects reentrant mutation.
3. A producer converts the consumer’s `SceneContract` to `Scene`.
4. History is staged as sideband state.
5. `OwnedBuilderRoot.start` creates a root execution scope.
6. The root evaluates and recursively creates child scopes.
7. Publication preparation resolves semantic nodes and attachments.
8. The desired root is committed.
9. Tui performs a host visibility flush.

A failed initial evaluation/preparation restores staged History, disposes fresh scopes, removes the root from the runtime, and leaves the Tui retryable. This behavior is explicit in `runtime/runtime.ts:463-485` and `execution.ts:353-410`.

## 4.3 Subsequent canonical render

Subsequent function-form render calls do not create a new root scope. `OwnedBuilderRoot.replaceProducer` replaces the producer on the existing root and re-drives it through the same retained protocol.

The closure identity is irrelevant. The root execution scope remains stable; body evaluation and child reconciliation determine what changes.

## 4.4 Direct render path

`Tui.render(SceneContract)` uses `renderDirect`:

- normalize `SceneContract`;
- validate semantic body;
- validate History ownership and native liveness;
- no-op if body and effective History are identical;
- otherwise prepare/commit a root publication;
- dispose the canonical builder root if present;
- flush host visibility.

Direct and builder render are intentionally distinct ownership modes:

- direct render takes over the root immediately;
- builder render retains producer and state subscriptions.

Direct render does not silently fall back to a prior route when retained materialization refuses.

Evidence: `runtime/runtime.ts:499-548`.

## 4.5 Composition evaluation

For each scope:

1. `scope.state = "evaluating"`.
2. Semantic slot cursor resets.
3. Child-owner WIP starts.
4. Pending dependency set resets.
5. Active execution scope and child owner are pushed.
6. Component body executes synchronously.
7. State reads link dependencies.
8. Component calls reconcile children.
9. Promise-like result rejects with `TUI_EXECUTION_ASYNC_BODY`.
10. `semanticNodeOf(output)` validates the returned View.
11. Active frame is popped.
12. Output remains pending until commit.

The child owner’s WIP has separate positional and keyed state. KeyGroup has no scheduler or dirty state; descendants inside it carry execution state.

## 4.6 State invalidation path

```text
StateSource.value read
   └── activeExecutionScope()
         └── scope.linkDependency(source)
               └── dependency promoted on commit

StateSource.set/update
   ├── reject during active evaluation
   ├── Object.is no-op if unchanged
   └── publish()
         └── scope.runtime.invalidateFromState(scope)
               └── dirty queue + one scheduled microtask
                     └── RetainedExecutionRuntime.flush
```

A state write invalidates exactly subscribed live scopes. It does not automatically re-evaluate ancestors or unrelated siblings.

The external consumer fixture proves this behavior:

- `status.set("running")` reruns `Header`, but not `App` or item cards.
- Updating one of 1,000 child state sources reruns only that child.
- Body outputs are visible without another explicit render call.

Evidence: `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:40-114`.

## 4.7 Dirty queue and flush transitions

`RetainedExecutionRuntime.flush`:

1. Consumes any scheduled microtask token.
2. Sets `protocolState.mutating`.
3. Acquires a queue batch.
4. Snapshots dirty retry obligations.
5. Sorts parent-before-child.
6. Evaluates all queued scopes.
7. On evaluation failure:
   - aborts WIP;
   - restores all dirty obligations;
   - does not automatically retry.
8. Prepares all publications.
9. On prepare failure:
   - unwinds staged publications;
   - aborts WIP;
   - restores dirty obligations;
   - does not automatically retry.
10. Commits all prepared work.
11. Defers removed-scope disposal until promotion completes.
12. Clears protocol state.

A commit-phase throw is treated as pathological: it is recorded in `pathologicalCommitFailures` and is not treated as an ordinary rollback.

## 4.8 ViewSlot state transitions

ViewSlot has two ownership modes.

### Builder mode

```text
slot.setView(() => View.text(label.value))
   └── OwnedBuilderRoot.start
         └── shared Tui RetainedExecutionRuntime
               └── State read subscribes slot root
                     └── subsequent label.set updates slot automatically
```

### Direct mode

```text
slot.setView(View.text("fixed"))
   ├── prepare attachment set
   ├── boundary.prepareInstall
   ├── commit native replacement
   ├── promote attachment/current View
   └── dispose old builder root last
```

If direct publication fails, the builder remains authoritative. A stale builder cannot ghost-overwrite a successful direct value because the builder root is disposed only after direct commit.

Evidence: `view-slot.ts:157-228`, `:238-281`, `:382-419`; fixture acceptance test `scoped-invalidation.test.ts:155-191`.

## 4.9 ScrollPane state transitions

ScrollPane mirrors ViewSlot for content ownership:

- builder mode uses shared retained runtime;
- direct mode installs through its own retained boundary;
- content replacement does not reset native scroll/follow-end state;
- builder root is disposed only after direct publication succeeds;
- `followEnd` is a separate native mutation and cannot run during retained protocol evaluation.

Evidence: `scroll-pane.ts:61-71`, `:119-225`.

## 4.10 History path

```text
History.push(view)
   └── tryRetainedMaterializeRef(view)
         ├── retained semantic hint/materialization
         ├── one temporary root lease
         └── release temporary refs after pushRef
```

History has special attach-once semantics:

- `new History()` creates detached caller-owned storage.
- `Tui.createHistory()` creates an already host-attached Tui-owned History.
- Detached History can transfer to one Tui during render.
- Once attached, it cannot move to another Tui.
- `freeze` and `discardLive` require attached History.
- Tui close marks caller-owned attached History unusable through a shared liveness token.

Evidence: `history.ts:28-42`, `:68-100`; `runtime/runtime.ts:280-316`.

## 4.11 Content source mutation path

```text
TextStreamSource.append/replace/clear/seal/truncate
   └── validate JS inputs
         └── transport/content/ffi.ts
               ├── load/cached direct FFI session
               ├── validate ABI metadata
               ├── encode UTF-8 and annotation records
               ├── invoke native source symbol
               ├── decode revision/wake result
               └── mark environment pending if requested
```

The native content control path is separate:

```text
ContentPort.connect
   └── transport/content/control.connectContent
         └── NativeContentPortContract.connect
               └── ContentConnector wrapper
```

Source payload mutation is direct FFI. Connector/Port activation/deactivation/disposal are small N-API control operations.

## 4.12 State attachment path

```text
ViewState.setGeometry/setPresentation
   └── normalize public patch
         └── generated state envelope
               └── native ViewState resource
                     └── wake flag
                           └── Tui/environment host drain
```

Semantic attachment occurs later:

```text
View.state(viewState)
   └── immutable semantic node stores state HandleId
         └── prepareSemanticAttachments traverses candidate tree
               ├── environment/host/node-kind validation
               ├── registry.prepareResolve
               └── desired/visible binding ledger
```

---

## 5. Alternate routes and failure semantics

The current package has multiple semantic operation routes, but they are generally optimization or ownership distinctions inside the same retained architecture rather than independent complete implementations.

| Semantic operation | Production path | Selection condition | Failure/recovery semantics |
|---|---|---|---|
| Ordinary `View.text`/row/column/etc. construction | `View` static/helper → semantic node | No active retained scope | Constructs immutable semantic node |
| Retained `View` construction | `View` modifier/helper → `composition/compose.ts` | Active execution scope | Exact slot reuse or one new immutable node |
| Canonical scene render | `Tui.render` → `renderCanonical` → `OwnedBuilderRoot` | Function argument | Retained execution; failed eval/prepare rolls back |
| Direct scene render | `Tui.render` → `renderDirect` | Scene object | Immediate root takeover; failed publication leaves prior root |
| Exact root reuse | `RetainedRootBoundary`/`renderExactRoot` → `hostRenderRef` | Same native root hint/generation | Zero semantic reads and zero payload encoding |
| Retained node lookup | `ensureSemanticNative` | Generation-valid semantic hint | Borrowed hint; native lease promotion where necessary |
| NodeId promotion | `ensureSemanticNative` → `viewRefForNodeId` | NodeId ≤ captured native lookup ceiling | Cache hit promotes; miss proceeds to direct materialization |
| Direct structural materialization | `retained-dag` direct materializer → generated ABI constructor | New node or derivation route | Native expected statuses may trigger one stale retry; otherwise explicit retained refusal |
| Stale child recovery | `recoverStaleNode` | Native fast-cache miss identifies child/base | One targeted retry only; no alternate transport |
| Wide axis update | `View.axisSetChildForTransport`/splice → persistent sequence/derivation → native ABI | Wide axis or structural edit | Native persistent sequence path; preparation refusal aborts batch |
| Grid cell update | grid transport helper → sequence/derivation → native ABI | Grid cell edit | Same retained transaction semantics |
| History push/freeze | `History` → `tryRetainedMaterializeRef` → `pushRef`/`freezeRef` | History operation | Retained refusal is explicit failure |
| ViewSlot direct update | ViewSlot boundary → retained materialization → native slot ref | `setView(View)` | Previous root remains leased until commit |
| ViewSlot builder update | `OwnedBuilderRoot` → shared retained runtime → slot target | `setView(() => View)` | State-driven; stale builder disposed after direct transition |
| ScrollPane direct/builder update | Same split as ViewSlot | `setContent(View)` or function | Native scroll state remains separate |
| ViewState mutation | `ViewState` → generated state envelope → native resource | State patch call | Validation before native call; wake flag drives host drain |
| Source append/replace | Content FFI | Source data mutation | Native status mapped to `TuiError`; no silent fallback |
| Connector activation | Content control N-API | Connector API | Native wake status; connector phase remains observable |
| TextInput submitted output | Native input output channel → `Output<T>` wrapper | `submitted()` | One cached channel per input |
| Key/paste dispatch | Tui → native host dispatch methods | `pressKey`, `bindKey`, paste methods | Native routing remains native; TypeScript receives generic output events |
| Automatic host drain | Wake broker microtask | Pending environment/host wake | Automatic errors are placed in error channel and do not throw from microtask |
| Explicit host flush | Wake broker explicit barrier | `Tui.flush`/testing access | Retries pending work, throws pending host error or explicit barrier failure |

### Silent fallback assessment

The source comments and control flow explicitly prohibit a previous-generation complete-object fallback in structural production:

- `RetainedRefusalError` is a retained materialization refusal, not a route selector.
- Boundaries convert refusal into explicit preparation failure.
- `native-view-abi.ts:143-157` states that retained refusal returns `undefined` for transient materialization helpers and callers fail explicitly.
- `retained-dag.ts:185-191` says no caller may catch refusal to select a previous-generation transport.

The content FFI and N-API content control routes are not duplicate complete View transports. They carry different semantic operations: bulk Source payload versus Port/Connector control.

### Failure masking

- Evaluation and preparation failures preserve the prior committed View/output and restore dirty obligations.
- Automatic wake broker failures are routed to `RuntimeErrorChannel`; the microtask does not throw.
- Explicit barriers surface pending errors synchronously.
- Native expected status failures in retained materialization may be normalized as retained refusal after one targeted stale retry.
- Commit-phase failures are intentionally not rolled back as ordinary failures; they are recorded as pathological.
- Tui close aggregates cleanup errors.
- Owned-handle cleanup retries disposal after dependent handles have had a chance to release leases.
- ViewSlot/ScrollPane direct replacement leaves builder ownership intact if installation fails.
- Content Connector disposal is deferred until native removal commits; wrapper status remains observable in intermediate phases.

---

## 6. Caches, invalidation, scheduling and performance

## 6.1 Composition caches and retention

### Per-scope semantic slots

`ScopeSemanticTable` stores current and pending `View` outputs by dense ordinal:

- start pass resets cursor;
- each composition operation consumes one slot;
- equal normalized operation stages the prior View identity;
- changed operation stages a fresh immutable View;
- commit truncates the slot table to the current cursor;
- rollback clears pending values and restores prior slot length.

The table is scope-local and does not survive scope disposal.

### Child reconciliation

`ChildOwnerState` stores:

- committed unkeyed children;
- pending unkeyed children;
- committed keyed `Map<ViewKey, KeyGroup>`;
- pending keyed map;
- cursor and WIP participation flag.

A keyed owner that evaluates with zero keyed children unmounts old keyed groups. An owner that never evaluates preserves committed groups. Abort drops only WIP.

### Persistent sequences

`PersistentSeq<T>` uses a branch factor of 32. `set`, insert, remove, split, concat and splice clone only a bounded path plus touched leaves/branches. Counters:

- `nodes_cloned`;
- `branches_cloned`;
- `items_iterated`.

Wide axis/grid transport uses persistent sequences and derivation overrides rather than rebuilding complete JS arrays on each edit.

## 6.2 Structural retained caches

`retained-dag.ts` contains:

- `SEMANTIC_NATIVE`: `WeakMap<SemanticViewNode, { generation, nativeRef }>`;
- transaction-local `refs`;
- transaction-local `inProgress`;
- temporary lease list;
- borrowed hint list;
- environment/runtime-specific axis scratch arrays by recursion depth;
- environment/runtime-specific grid word scratch arrays by recursion depth;
- one byte scratch array for text/diff payloads;
- generation-scoped style `WeakMap` plus color atom map.

Hints are weak acceleration metadata. Native ownership remains in explicit leases and native retained structures.

The exact-root fast path avoids semantic tree reads and payload writes when a valid root identity is already available.

## 6.3 Attachment cache/ledger

`AttachmentBindingState` retains:

- current desired leases;
- current visible leases;
- superseded desired revision leases;
- desired revision;
- visible revision.

When visible frame acknowledgements arrive out of order or lag desired publication, revision comparison prevents older visible revisions from replacing newer visible state. Superseded desired bindings survive until no longer needed by backend flight.

## 6.4 Theme cache invalidation

Style refs and atoms are cached by runtime/generation. `Tui.setTheme` always calls `resetStyleRefCacheForThemeChange()` in a `finally` block, including when native theme application reports an error. This ensures later retained materialization does not reuse style refs from the prior theme epoch.

Evidence: `runtime/runtime.ts:880-894`; `retained-dag.ts:92-103`, `:2134-2138`.

## 6.5 State invalidation and scheduling

Composition scheduling:

- State source publish walks only subscribed scopes.
- Each scope is enqueued once while dirty.
- Duplicate invalidations increment a counter and re-arm scheduling if necessary.
- `queueMicrotask` auto-flushes by default.
- Explicit `flush` consumes the pending microtask generation token.
- Failed batches restore dirty obligations but do not automatically retry, preventing infinite microtask loops for permanently throwing bodies.

Native/environment scheduling:

- Native mutation returns a wake disposition.
- Source FFI returns an environment wake epoch and schedule flag.
- ViewState/Content control calls may call `requestWake`.
- `EnvironmentWakeBroker` edge-latches pending hosts and drains fairly with a default budget.
- Asynchronous presentation receipts are polled with a timer rather than a microtask spin.

## 6.6 Instrumentation

Composition counters:

- scope mounts/unmounts/body calls;
- prop skips;
- state invalidations;
- dirty enqueues and duplicate invalidations;
- no-op/changed outputs;
- flush passes/commit batches/aborts;
- exact View reuse/new View counts.

Structural counters:

- retained hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast paths;
- ref words written;
- byte payload bytes;
- scratch reuse;
- stale-ref retries;
- decorated normalization;
- host mutations.

Persistent sequence counters and wake broker counters are separate. Runtime phase instrumentation can record prepare/native materialize/host commit nanoseconds.

No counters were observed at the public package root. They are private/internal diagnostics used by tests and benchmarks.

## 6.7 Hot paths

| Path | Hotness | Reason |
|---|---|---|
| `StateSource.value` | High when rendering many scopes | Active-scope dependency link |
| `compose.ts` equality/reuse | High | Every retained semantic operation |
| execution dirty queue/flush | High under state updates | Selective rerender frontier |
| retained hint lookup | High on recurrent materialization | Avoids payload reads and native reconstruction |
| semantic attachment traversal | Conditional | Required when candidate contains attachments |
| persistent wide-axis edit | Conditional | Large child sequences only |
| native source append FFI encoding | High for streaming text | Bulk payload path |
| wake broker | Conditional but cross-host | One environment-level scheduler |
| ViewState envelope packing | Conditional | Only state mutations |
| animation frame materialization | Conditional | Each animation update/cycle setup |

---

## 7. Tests, benchmarks and observability

## 7.1 Test package inventory

The framework test suite under `packages/iyon-tui/tests/` contains:

- `tui_ansi_scanner.test.ts`
- `tui_demo.test.ts`
- `tui_generated_view_abi.test.ts`
- `tui_h3_a_semantic.test.ts`
- `tui_h3_b_composition.test.ts`
- `tui_h3_c_transport.test.ts`
- `tui_handles.test.ts`
- `tui_harness.test.ts`
- `tui_history_prefix.test.ts`
- `tui_native_builder.test.ts`
- `tui_native_input_validation.test.ts`
- `tui_native_persistent_seq.test.ts`
- `tui_native_scalar.test.ts`
- `tui_native_strings.test.ts`
- `tui_native_transaction.test.ts`
- `tui_perf13_a.test.ts`
- `tui_perf13_b.test.ts`
- `tui_perf13_d.test.ts`
- `tui_perf13_h.test.ts`
- `tui_realtime.test.ts`
- `tui_retained_scene_regressions.test.ts`
- `tui_semantic_cache_ownership.test.ts`
- `tui_semantic_pipeline.test.ts`
- `tui_smooth_delivery.test.ts`
- `tui_state_envelope.test.ts`
- `tui_surface_contract.test.ts`
- `tui_t14_differential.test.ts`
- `tui_t14_fuzz_property.test.ts`
- `tui_text_lanes.test.ts`
- `tui_traits.test.ts`
- `tui_values.test.ts`
- `tui_runtime.test.ts`
- `generated/view_abi_layout.test.ts`

The repository package script runs both framework and fixture tests:

```json
"test": "bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests"
```

Evidence: root `package.json:31-36`, package manifest `packages/iyon-tui/package.json:15-20`.

## 7.2 Behavior covered by tests

The test names provide direct architecture evidence:

- semantic View foundations and generated ABI conformance;
- retained composition and state invalidation;
- native builder/scalar/string/transaction routes;
- attachment lease validation and duplicate attachment rejection;
- desired versus visible structural publication;
- state envelopes and clear semantics;
- content source direct FFI and ownership cycles;
- History prefix/tail behavior;
- persistent sequence axis/grid edits;
- smooth delivery and deterministic clock;
- semantic cache ownership;
- runtime error and input behavior;
- malformed boundary fuzz/differential properties;
- public surface contract and trait adapters.

Particularly relevant tests:

- `tui_perf13_a.test.ts`
  - semantic attachment prepare/lease;
  - duplicate use;
  - failed host channel behavior;
  - asynchronous presentation receipt polling;
  - desired/visible publication separation.
- `tui_perf13_b.test.ts`
  - presentation overrides without republishing structure;
  - state through retained structural path;
  - attachment identity through immutable modifiers;
  - mounted/wrong-host/duplicate state behavior.
- `tui_perf13_d.test.ts`
  - Source ownership independent from host teardown;
  - cross-host/duplicate Port attachments;
  - visible Connector preservation across candidate failure;
  - deferred cleanup/lifecycle error codes.
- `tui_perf13_h.test.ts`
  - direct FFI revision/wake result;
  - shared Source subscriptions across host teardown;
  - repeated host/Connector ownership cycles.
- `tui_retained_scene_regressions.test.ts`
  - state dependency frontier;
  - theme obligations;
  - captured state values through ViewSlot replacement.
- `tui_state_envelope.test.ts`
  - geometry/presentation packing and validation.
- `tui_semantic_cache_ownership.test.ts`
  - repeated Markdown replacements and content-generation reset.

No tests were run for this report.

## 7.3 External-consumer fixture evidence

`packages/tui-consumer-fixture/src/consumer.ts` explicitly prohibits:

- internal runtime imports;
- composition/compiler/plugin setup;
- feature flags;
- manual View memoization;
- identity discipline.

Its imports are restricted to:

```ts
import { Insets, Scene, Style, View } from "@iyon/tui";
import { AppHarness } from "@iyon/tui/testing";
import type { History, ScrollPane, TextInput, TuiRuntime, View as ViewValue, ViewSlot } from "@iyon/tui";
```

The componentized section also imports only public root `defineView` and `state`.

The fixture proves:

- plain public API session setup;
- Tui-owned TextInput, ViewSlot, ScrollPane, History;
- public scene rendering;
- public `defineView`, `state`, `View.key`;
- automatic shared retained runtime activation;
- scoped invalidation and keyed identity preservation;
- direct/builder ownership transition behavior.

This is the only in-repository package deliberately shaped as an external consumer. No independent third-party or product package is present.

## 7.4 Observability gaps

- Runtime counters are internal and not exposed through `@iyon/tui`.
- Wake traces are gated by `Bun.env.PERF_RUNTIME_TRACE === "1"`.
- Native retained memory snapshots are optional addon exports.
- The package root does not expose native ABI metadata or registry statistics.
- There is no static package graph artifact in `packages/`; graph reconstruction requires import/manifests/source inspection.
- No executed validation was available in this report, so native artifact staging and current test pass status remain unknown.

---

## 8. Cross-boundary findings and contradictions

## 8.1 The package boundary is intentionally thin but runtime coupling is deep

The package-root surface is compact and generic, but each public handle often reaches directly into transport and runtime internals:

- `ViewState` directly imports generated state envelope machinery.
- `ContentPort`/`ContentConnector` directly import content control/FFI and resource registries.
- ViewSlot/ScrollPane directly own `RetainedRootBoundary`, retained execution roots, attachment ledgers, and native ABI refs.
- `History` directly uses retained materialization and native ref release.
- `Tui` directly imports nearly every public and private subsystem.

This creates a small external API over a tightly coupled framework runtime.

## 8.2 API ↔ composition cycle is deliberate

`View` imports composition helpers for retained construction. `composition/compose.ts` imports `View` constructors and semantic-node functions.

The cycle is controlled by:

- `execution-context.ts` not importing `View`;
- type-only imports where possible;
- `withoutRetainedComposition` for framework/internal raw seed construction;
- semantic construction flag rather than monkey patching.

This is not a package boundary violation, but it is a consequential module-level coupling.

## 8.3 Content API ↔ runtime environment cycle is also deliberate

`api/content/retained.ts` reaches into:

- `runtime/environment.ts`;
- `runtime/handle-registry.ts`;
- `transport/native/resource-registry.ts`;
- content FFI/control.

`runtime/runtime.ts` imports content API and content control to construct host-owned ContentPorts. The cycle is avoided operationally because content API imports `runtime/environment`, not the `Tui` class itself.

## 8.4 Generic controls rely on shared Tui runtime

ViewSlot and ScrollPane appear as generic `ComponentHandle` values, but builder mode is only supported for handles created through `Tui.createViewSlot`/`createScrollPane`, because they require the Tui’s shared `RetainedExecutionRuntime`.

Raw/private construction can still create internal slot/pane values for component projections, but those values intentionally do not support caller builder mode. This distinction is documented in:

- `view-slot.ts:86-99`, `:115-129`, `:181-188`;
- `scroll-pane.ts:61-80`, `:84-93`, `:125-142`.

## 8.5 Native component adapters are not the same as TypeScript retained components

The public `ComponentAdapter` trait is an asynchronous/native interaction-oriented contract. `defineView` is a synchronous retained semantic composition contract. They are not connected automatically.

Observed source facts:

- `ComponentAdapter` can provide async `view`, capability, key, paste, and tick methods.
- `defineView` requires a synchronous `View` body.
- Tests exercise adapters directly.
- No runtime import path was found that turns `ComponentAdapter` into `defineView` or a native component registration.

Treating these as one abstraction would be inaccurate.

## 8.6 Semantic attachments are backend-neutral until prepare

Semantic nodes store only opaque attachment IDs. The actual resource/environment/host/node-kind checks happen in `prepareSemanticAttachments`.

Consequences:

- A semantic View can be constructed without a live host.
- The same View can fail at publication time if an attachment belongs to another environment or host.
- Attachment errors preserve the previous committed frame.
- Semantic DAG sharing is allowed, but repeated attachment identity is rejected by occurrence path.

## 8.7 Native artifact identity is shared by N-API and direct FFI

`native/artifact.ts` deliberately centralizes artifact selection. Both:

- `transport/native/addon.ts` (`require`);
- `transport/content/ffi.ts` (`dlopen`)

resolve the same staged `iyon-tui-native.node`.

`stage-native.ts` verifies:

- native build identity;
- content ABI symbols;
- direct-FFI qualification surface depending on `ION_NATIVE_FEATURES`;
- no removed native classes/methods;
- content FFI resolves the same realpath as the staged addon.

This is a strong transport coupling and a useful invariant.

## 8.8 Source ownership versus host ownership is split

Text Sources are environment-owned and may outlive a Tui/host. Content Ports, Connectors, and host-bound handles are host-owned.

Tui teardown therefore:

1. unregisters host wake handling;
2. disposes host content resources;
3. disposes attachment bindings and ViewState bindings;
4. invalidates host-owned resources in the shared registry;
5. disposes owned handles;
6. disposes retained execution and structural boundary;
7. leaves detached caller-owned Sources/History available where contract permits.

This distinction is explicitly documented in `runtime/runtime.ts:784-820` and `retained.ts:336-355`, `:409-428`.

## 8.9 Naming drift in testing facade

`TuiRuntime` calls routing parameters `routeId`; `AppHarness.bindKey`/`route` name the same parameters `actionId`. The fixture uses generic route IDs. This appears to be nomenclature drift, not an application-specific action model, because the underlying native contract and root API use generic `routeId`.

## 8.10 Root workspace and package-local exports duplicate the same package boundary

The repository root presents itself as `iyon-tui-workspace` but exposes the same framework files as the `@iyon/tui` package. Consumers in the fixture resolve the workspace package alias, while scripts and internal tests often import source-relative paths.

This creates three source-consumption forms:

1. public package alias (`@iyon/tui`);
2. public testing alias (`@iyon/tui/testing`);
3. internal relative imports (`../src/...`).

The fixture demonstrates that only the first two are required for a consumer.

---

## 9. Open questions and coverage gaps

1. **Native implementation details**
   - The TypeScript contracts identify native methods and lifecycle semantics, but native ownership and Rust-side rendering behavior are outside this assignment’s primary scope.
   - Exact native retention/History/connector state transitions require corresponding Rust/native inspection.

2. **Actual runtime validation**
   - Native artifact staging status was not validated.
   - Tests/type checks/benchmarks were not executed.
   - The source tree contains staged/build target directories, but this report does not infer whether the baseline addon is currently loadable.

3. **External consumer coverage**
   - Only `packages/tui-consumer-fixture` is present.
   - No plugin/application package is available to prove usage of public renderer/projector traits, `Projection`, `History` variants, or all content APIs.

4. **Trait adapter reachability**
   - Direct source search found trait adapters in tests but no integration path from `ComponentAdapter`, `Renderer`, `Projector`, `TextVisitor`, or `TextRewriter` into Tui runtime composition.
   - It is unknown whether downstream consumers instantiate these manually outside the repository.

5. **Generated ABI completeness**
   - Generated ABI files and manifest were indexed and key signatures inspected.
   - Exact generated function count and every generated body’s implementation details were not independently reconstructed here.

6. **Cross-package type resolution**
   - Manifest/package graph was statically inspected.
   - No TypeScript module-resolution command was run to prove every package alias resolves at this commit.

7. **History attachment edge cases**
   - Source establishes attach-once ownership and liveness guards.
   - Exact native behavior after orphaning a previously bound History requires native-side confirmation.

8. **Direct FFI feature terminology**
   - Content data FFI is always represented in the TypeScript source through `content/ffi.ts`.
   - The `direct-ffi` feature checks in `stage-native.ts` concern qualification/export surfaces in the addon. The exact Cargo feature relationship is outside this package-only investigation.

9. **Public API versus intentionally reachable internals**
   - The package root excludes most runtime/transport symbols, but source-relative imports remain available to repository tests/scripts.
   - Whether published package consumers can technically reach source internals depends on packaging/publishing behavior not proven solely by the manifests.

---

## 10. Evidence appendix

## 10.1 Primary paths and symbols

### Package manifests and boundary

- `package.json`
  - Root exports and workspaces.
  - Scripts `typecheck`, `test`, `native:stage`, `check:tui-abi`, `check:tui-binding`, `check:ownership`.
- `packages/iyon-tui/package.json`
  - Package exports, package-local scripts, no runtime dependencies.
- `packages/tui-consumer-fixture/package.json`
  - Private external-consumer fixture and sole `@iyon/tui` workspace dependency.

### Public package root

- `packages/iyon-tui/src/index.ts:1-132`
  - Complete package-root export list.

### Runtime composition root

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`
  - `Tui`
  - `Tui.open`
  - `render`
  - `renderCanonical`
  - `renderDirect`
  - `prepareRootPublication`
  - `stageHistoryBinding`
  - `createHistory`
  - `viewState`
  - `contentPort`
  - `createTextInput`
  - `createViewSlot`
  - `createScrollPane`
  - `flush`
  - `close`
  - `exit`
  - `setTheme`

### Composition

- `packages/iyon-tui/src/composition/define-view.ts:34-65`
  - `ViewComponentType`
  - `ViewComponent`
  - `defineView`
- `packages/iyon-tui/src/composition/tracked-state.ts:27-136`
  - `State<T>`
  - `state`
  - `StateSource`
  - dependency subscription and invalidation rules.
- `packages/iyon-tui/src/composition/execution.ts:109-218`
  - `RetainedExecutionScope`
- `packages/iyon-tui/src/composition/execution.ts:328-554`
  - `RetainedExecutionRuntime`
  - queue/scheduling/flush.
- `packages/iyon-tui/src/composition/execution.ts:627-780`
  - evaluation, publication staging, commit.
- `packages/iyon-tui/src/composition/execution.ts:783-884`
  - abort/mount rollback.
- `packages/iyon-tui/src/composition/execution.ts:1198-1250`
  - `OwnedBuilderRoot`.
- `packages/iyon-tui/src/composition/child-owner.ts:27-88`
  - `ChildOwnerState`
  - `KeyGroup`.
- `packages/iyon-tui/src/composition/execution-context.ts:33-132`
  - active scope/owner, protocol flags, keyed owner swapping.
- `packages/iyon-tui/src/composition/publication.ts:9-38`
  - publication/projection interfaces.
- `packages/iyon-tui/src/composition/persistent-seq.ts:215-309`
  - `PersistentSeq`.

### Semantic View/API

- `packages/iyon-tui/src/api/view/view.ts:106-218`
  - layout types and builders.
- `packages/iyon-tui/src/api/view/view.ts:241-477`
  - `View` construction and modifiers.
- `packages/iyon-tui/src/api/view/view.ts:574-718`
  - transport-facing immutable node/edit helpers.
- `packages/iyon-tui/src/api/view/view.ts:779-1026`
  - composed axis, identity, sequence/grid utilities.
- `packages/iyon-tui/src/api/view/semantic-node.ts:22-279`
  - semantic node types and kind union.
- `packages/iyon-tui/src/api/view/semantic-node.ts:286-377`
  - node creation/installation/attachment identity.
- `packages/iyon-tui/src/api/view/semantic-node.ts:463-610`
  - derivations and persistent sequence overrides.
- `packages/iyon-tui/src/api/view/retained-state.ts:39-229`
  - ViewState fields, patches, native mutations.
- `packages/iyon-tui/src/api/view/scene.ts:4-27`
  - Scene contract and root wrapper.

### Handle/resource ownership

- `packages/iyon-tui/src/api/controls/framework-handle.ts:9-65`
  - `HandleId`, `FrameworkHandle`, `ComponentHandle`.
- `packages/iyon-tui/src/runtime/handle-registry.ts:14-80`
  - handle registration and disposal.
- `packages/iyon-tui/src/transport/native/resources.ts:22-88`
  - local/native resource map and registry delegation.
- `packages/iyon-tui/src/transport/native/resource-registry.ts:8-32`
  - registry contracts.
- `packages/iyon-tui/src/transport/native/resource-registry.ts:75-162`
  - `PreparedResourceLease`.
- `packages/iyon-tui/src/transport/native/resource-registry.ts:168-460`
  - `NativeResourceRegistry`.
- `packages/iyon-tui/src/transport/native/resource-registry.ts:463-480`
  - realm-wide environment/registry globals.

### Attachment ownership

- `packages/iyon-tui/src/runtime/attachments.ts:17-32`
  - attachment runtime contracts.
- `packages/iyon-tui/src/runtime/attachments.ts:55-152`
  - desired/visible/superseded binding ledger.
- `packages/iyon-tui/src/runtime/attachments.ts:159-254`
  - semantic attachment traversal and duplicate checks.
- `packages/iyon-tui/src/runtime/attachments.ts:275-306`
  - attachment node traversal.

### Native and structural transport

- `packages/iyon-tui/src/transport/native/addon.ts:14-248`
  - all private native TypeScript contracts and addon loading.
- `packages/iyon-tui/src/transport/native/artifact.ts:5-71`
  - canonical artifact location/build identity.
- `packages/iyon-tui/src/transport/structural/ir.ts:1-130`
  - schema-backed numeric IR.
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts:57-187`
  - ABI session, retained materialization and ref lease wrapper.
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts:194-450`
  - axis/grid/edit retained calls.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1-22`
  - retained structural architecture and no-secondary-route rule.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:52-229`
  - native hints, counters, materialization transaction.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:241-339`
  - scratch buffers and temporary lease handling.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:415-467`
  - stale-ref recovery and retained refusal.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:475-1050`
  - direct semantic-node materializers.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1153-1237`
  - semantic native resolution.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1376-1529`
  - exact root and root acquisition.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1545-2138`
  - `RetainedRootBoundary`, publication, visible commit, close, theme cache reset.

### State/content transport

- `packages/iyon-tui/src/transport/state/control.ts:37-358`
  - state patch normalization/envelope conversion.
- `packages/iyon-tui/src/transport/state/generated/state_envelope.ts:1-375`
  - generated state schema/envelopes.
- `packages/iyon-tui/src/transport/content/control.ts:1-98`
  - native Source/Port/Connector control calls.
- `packages/iyon-tui/src/transport/content/ffi.ts:1-350+`
  - content ABI, source identity, annotation encoding, direct FFI data path.
- `packages/iyon-tui/src/api/content/retained.ts:37-165`
  - public content contracts.
- `packages/iyon-tui/src/api/content/retained.ts:337-468`
  - Source wrappers.
- `packages/iyon-tui/src/api/content/retained.ts:470-557`
  - Funnel modes and delivery.
- `packages/iyon-tui/src/api/content/retained.ts:560-810`
  - Port/Connector lifecycle and ownership.

### Scheduling and testing

- `packages/iyon-tui/src/runtime/environment.ts:14-45`
  - one realm environment.
- `packages/iyon-tui/src/runtime/wake-broker.ts:3-370+`
  - host epochs, pending wake broker, explicit/automatic drains.
- `packages/iyon-tui/src/testing/index.ts:18-154`
  - `AppHarness`.
- `packages/iyon-tui/scripts/stage-native.ts:1-124`
  - native staging and surface validation.
- `packages/iyon-tui/scripts/smoke-native.ts:1-21`
  - public package smoke route.

### Consumer evidence

- `packages/tui-consumer-fixture/src/consumer.ts:1-170`
  - public-only consumer, ordinary and componentized forms.
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:1-193`
  - selective invalidation, keyed identity, builder/direct ownership.
- `packages/tui-consumer-fixture/tests/consumer.test.ts`
  - public consumer rendering tests.
- `packages/iyon-tui/tests/tui_surface_contract.test.ts:1-20`
  - public `Scene`, `History`, `TextInput`, `View`, and `ComponentAdapter` surface checks.

## 10.2 Inspected file manifest

### Framework production API

```text
packages/iyon-tui/src/api/errors.ts
packages/iyon-tui/src/api/content/annotations.ts
packages/iyon-tui/src/api/content/diff.ts
packages/iyon-tui/src/api/content/projection.ts
packages/iyon-tui/src/api/content/retained.ts
packages/iyon-tui/src/api/content/text-content.ts
packages/iyon-tui/src/api/content/text.ts
packages/iyon-tui/src/api/controls/framework-handle.ts
packages/iyon-tui/src/api/controls/history.ts
packages/iyon-tui/src/api/controls/output.ts
packages/iyon-tui/src/api/controls/scroll-pane.ts
packages/iyon-tui/src/api/controls/text-input.ts
packages/iyon-tui/src/api/controls/view-slot.ts
packages/iyon-tui/src/api/extensions/traits/component.ts
packages/iyon-tui/src/api/extensions/traits/projector.ts
packages/iyon-tui/src/api/extensions/traits/renderer.ts
packages/iyon-tui/src/api/extensions/traits/text-rewriter.ts
packages/iyon-tui/src/api/extensions/traits/text-visitor.ts
packages/iyon-tui/src/api/presentation/semantic-style.ts
packages/iyon-tui/src/api/presentation/style.ts
packages/iyon-tui/src/api/presentation/theme-key.ts
packages/iyon-tui/src/api/presentation/theme.ts
packages/iyon-tui/src/api/view/geometry.ts
packages/iyon-tui/src/api/view/retained-state.ts
packages/iyon-tui/src/api/view/scene.ts
packages/iyon-tui/src/api/view/semantic-node.ts
packages/iyon-tui/src/api/view/view.ts
```

### Composition

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

### Runtime

```text
packages/iyon-tui/src/runtime/access.ts
packages/iyon-tui/src/runtime/attachments.ts
packages/iyon-tui/src/runtime/environment.ts
packages/iyon-tui/src/runtime/error-channel.ts
packages/iyon-tui/src/runtime/events.ts
packages/iyon-tui/src/runtime/handle-registry.ts
packages/iyon-tui/src/runtime/native-resource-registry.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/runtime/wake-broker.ts
packages/iyon-tui/src/testing/index.ts
packages/iyon-tui/src/index.ts
```

### Transport

```text
packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json
packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts
packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json
packages/iyon-tui/src/transport/content/abi.ts
packages/iyon-tui/src/transport/content/control.ts
packages/iyon-tui/src/transport/content/ffi.ts
packages/iyon-tui/src/transport/native/addon.ts
packages/iyon-tui/src/transport/native/artifact.ts
packages/iyon-tui/src/transport/native/factories.ts
packages/iyon-tui/src/transport/native/resource-registry.ts
packages/iyon-tui/src/transport/native/resources.ts
packages/iyon-tui/src/transport/state/control.ts
packages/iyon-tui/src/transport/state/generated/state_envelope.ts
packages/iyon-tui/src/transport/structural/component-id.ts
packages/iyon-tui/src/transport/structural/encoding.ts
packages/iyon-tui/src/transport/structural/ir.ts
packages/iyon-tui/src/transport/structural/native-view-abi.ts
packages/iyon-tui/src/transport/structural/policy.ts
packages/iyon-tui/src/transport/structural/retained-dag.ts
packages/iyon-tui/src/transport/structural/retained-path.ts
packages/iyon-tui/src/transport/structural/style-lowering.ts
```

### Package scripts/benchmarks

```text
packages/iyon-tui/scripts/smoke-native.ts
packages/iyon-tui/scripts/stage-native.ts
packages/iyon-tui/bench/PERF-12-T13.1-R6b-frontier.jsonl
packages/iyon-tui/bench/PERF-12-T15-authoritative-019a048b7c6f.json
packages/iyon-tui/bench/PERF-12-T15-authoritative-019a048b7c6f.jsonl
packages/iyon-tui/bench/PERF-12-T15-authoritative-e1fdd93a1a20.json
packages/iyon-tui/bench/PERF-12-T15-authoritative-e1fdd93a1a20.jsonl
packages/iyon-tui/bench/PERF-12-T15-memory-3a76f5069246.jsonl
packages/iyon-tui/bench/PERF-12-T15-memory-3d32b5163962.jsonl
packages/iyon-tui/bench/PERF-12-T15-multi-edit-701b68055782.jsonl
packages/iyon-tui/bench/PERF-12-T15-multi-edit-7acdc10375e9.jsonl
packages/iyon-tui/bench/PERF-12-T15-realistic-6efb3d9216e7.jsonl
packages/iyon-tui/bench/PERF-12-T15-realistic-80707ce7d9af.jsonl
packages/iyon-tui/bench/PERF-12-s6-napi-dispatch.jsonl
packages/iyon-tui/bench/PERF-12-s6-napi-transport.jsonl
packages/iyon-tui/bench/generated/view_abi_cases.ts
packages/iyon-tui/bench/perf12_t15_authoritative_case.ts
packages/iyon-tui/bench/perf12_t15_workload.ts
packages/iyon-tui/bench/perf13_h_content.ts
packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts
packages/iyon-tui/bench/pre-v5-l1-trace.ts
```

### Tests

```text
packages/iyon-tui/tests/fixtures/native-host.ts
packages/iyon-tui/tests/fixtures/tui_demo.ts
packages/iyon-tui/tests/generated/view_abi_layout.test.ts
packages/iyon-tui/tests/tui_ansi_scanner.test.ts
packages/iyon-tui/tests/tui_demo.test.ts
packages/iyon-tui/tests/tui_generated_view_abi.test.ts
packages/iyon-tui/tests/tui_h3_a_semantic.test.ts
packages/iyon-tui/tests/tui_h3_b_composition.test.ts
packages/iyon-tui/tests/tui_h3_c_transport.test.ts
packages/iyon-tui/tests/tui_handles.test.ts
packages/iyon-tui/tests/tui_harness.test.ts
packages/iyon-tui/tests/tui_history_prefix.test.ts
packages/iyon-tui/tests/tui_native_builder.test.ts
packages/iyon-tui/tests/tui_native_input_validation.test.ts
packages/iyon-tui/tests/tui_native_persistent_seq.test.ts
packages/iyon-tui/tests/tui_native_scalar.test.ts
packages/iyon-tui/tests/tui_native_strings.test.ts
packages/iyon-tui/tests/tui_native_transaction.test.ts
packages/iyon-tui/tests/tui_perf13_a.test.ts
packages/iyon-tui/tests/tui_perf13_b.test.ts
packages/iyon-tui/tests/tui_perf13_d.test.ts
packages/iyon-tui/tests/tui_perf13_h.test.ts
packages/iyon-tui/tests/tui_realtime.test.ts
packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
packages/iyon-tui/tests/tui_runtime.test.ts
packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts
packages/iyon-tui/tests/tui_semantic_pipeline.test.ts
packages/iyon-tui/tests/tui_smooth_delivery.test.ts
packages/iyon-tui/tests/tui_state_envelope.test.ts
packages/iyon-tui/tests/tui_surface_contract.test.ts
packages/iyon-tui/tests/tui_t14_differential.test.ts
packages/iyon-tui/tests/tui_t14_fuzz_property.test.ts
packages/iyon-tui/tests/tui_text_lanes.test.ts
packages/iyon-tui/tests/tui_traits.test.ts
packages/iyon-tui/tests/tui_values.test.ts
```

### Consumer fixture

```text
packages/tui-consumer-fixture/src/consumer.ts
packages/tui-consumer-fixture/tests/consumer.test.ts
packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts
```

## 10.3 Files indexed but not comprehensively read line-by-line

The following were indexed for graph completeness and symbol/reference searches, but generated bodies, benchmark data, and some large test bodies were not read in full:

- all JSONL/JSON benchmark result files under `packages/iyon-tui/bench/`;
- complete generated structural ABI bodies under `src/transport/abi/structural/generated/`;
- complete generated ABI manifest/schema contents beyond key mappings;
- complete contents of every framework test file;
- native artifact binary and Rust/native implementation;
- package lockfile dependency internals.

This does not affect the main TypeScript package graph, public API, retained runtime, or ownership conclusions above, but it limits claims about exact generated function bodies and executed behavior.