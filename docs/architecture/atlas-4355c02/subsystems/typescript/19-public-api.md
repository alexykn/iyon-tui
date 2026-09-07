# 19 — TypeScript public API

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary package: `packages/iyon-tui`
- Package name: `@iyon/tui`
- Package entry points: `packages/iyon-tui/package.json:7-10`
  - `"." -> "./src/index.ts"`
  - `"./testing" -> "./src/testing/index.ts"`
  - `"./native-stage" -> "./scripts/stage-native.ts"`

The initial worktree/source baseline was treated as read-only. No project files, dependencies, services, or tests were modified or run.

### Scope

Primary scope:

- `packages/iyon-tui/src/api/**`
- `packages/iyon-tui/src/index.ts`

Supporting seams inspected to establish real consumers and ownership:

- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/iyon-tui/src/testing/index.ts`
- `packages/iyon-tui/src/composition/define-view.ts`
- `packages/iyon-tui/src/composition/tracked-state.ts`
- `packages/iyon-tui/src/composition/compose.ts` import/use sites
- `packages/iyon-tui/src/transport/structural/retained-path.ts`
- `packages/tui-consumer-fixture/src/consumer.ts`
- `packages/iyon-tui/package.json`
- root `AGENTS.md`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The pre-V5 document was used as historical/contextual guidance only. This report maps current source and does not make V5 disposition decisions.

### Facts, inferences, and unknowns

- **Current-source fact:** the TypeScript facade exposes immutable semantic `View` values, styles/themes, content/source/funnel/connector values, controls, state handles, scene roots, and runtime contracts.
- **Current-source fact:** public package consumers normally reach the API through the root `@iyon/tui` export or `@iyon/tui/testing`; most files under `src/api` are not independently exposed by package exports.
- **Current-source fact:** public handle wrappers are tightly coupled to runtime registries, native transport, retained structural ownership, and host lifecycle.
- **Current-source fact:** `View` construction has two routes:
  1. direct semantic construction outside a retained execution scope;
  2. composition-aware construction inside a retained execution scope.
- **Static inference:** the semantic `View` model, styles, content data types, and handle contracts form the authoring-facing type boundary, while transport and native resource details are intentionally hidden from package-root consumers.
- **Unknown:** no external product/plugin package is present in this scope that exercises every exported content, projection, annotation, theme, or extension type. The consumer fixture demonstrates a substantial public path but is not proof that every public symbol is used externally.
- **Validation status:** static source inspection only. No test, typecheck, build, benchmark, or runtime execution was performed by this scout.

### Physical LOC methodology

Counts below are approximate physical source lines, including comments and blank lines, based on the source line numbers returned during inspection. They are not logical statement counts and exclude tests/generated files unless explicitly noted.

| Area | Files | Approx. physical production LOC |
|---|---:|---:|
| `api/content` | 6 | 1,189 |
| `api/controls` | 6 | 985 |
| `api/errors.ts` | 1 | 106 |
| `api/extensions/traits` | 5 | 102 |
| `api/presentation` | 4 | 816 |
| `api/view` | 5 | 2,066 |
| `src/index.ts` | 1 | 132 |
| **Total inspected production source** | **28** | **5,396** |

No generated source is in the primary scope. Test LOC was not counted because tests belong to another assignment; the test tree was indexed and selected tests were inspected for behavioral evidence.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approx. LOC | Public package surface | Primary responsibility | Significant secondary responsibilities |
|---|---:|---|---|---|
| `src/api/errors.ts` | 106 | Root value/type exports | Normalize framework/native errors into `TuiError` | Native error-code classification; cancellation and lifecycle categories |
| `src/api/content/annotations.ts` | 29 | `Annotations`, annotation types, schema constant | Generic semantic tags/properties and Source annotation schema | Immutable-style builder methods; host-independent semantic text style |
| `src/api/content/diff.ts` | 102 | Diff value classes and renderer | Validate and represent diff ranges, lines, hunks | Lowers diff values into `View` through `DiffRenderer` |
| `src/api/content/projection.ts` | 34 | `Projection`, `ProjectionBuilder`, `Smooth`, span type | Represent source-to-output text projections and smoothing offset | Projection span validation and concatenation |
| `src/api/content/retained.ts` | 810 | Source, Funnel, Port, Connector values/types | Public retained text source/funnel/content control boundary | Native FFI, source snapshots/stats, retention, connector lifecycle, host wake scheduling |
| `src/api/content/text-content.ts` | 30 | `TextContent`, `RawText`, origin/format types | Small immutable text value/origin facade | Rendering to `View`, visiting, rewriting |
| `src/api/content/text.ts` | 184 | `TextSelector`, `TextSpan`, text semantic types | Text roles, parts, annotations, selector values, styled spans | Selector validation and conversion of `StyleSpec` to `StyleRef` |
| `src/api/controls/framework-handle.ts` | 66 | Root type exports for `FrameworkHandle`, `ComponentHandle`, `HandleId` | Opaque JS-local handle identity and common native resource lifecycle | Runtime registry access; disposal/error guard |
| `src/api/controls/history.ts` | 132 | `History`, `HistoryLayout` | Ordered scrollback handle and detached/host-bound lifecycle | Retained View materialization for push/freeze; one-way host binding |
| `src/api/controls/output.ts` | 8 | `Output<T>` type only | Opaque typed routed-output identity | Private construction prevents fabricated native channels |
| `src/api/controls/scroll-pane.ts` | 264 | `ScrollPane` type; implementation is internal | Host-owned retained scrolling component | Root leases, builder/direct content ownership, retained replacement, follow-end |
| `src/api/controls/text-input.ts` | 95 | `TextInput`, options | Host-bound text input component | Stable submitted `Output<T>` facade; native output ownership association |
| `src/api/controls/view-slot.ts` | 420 | `ViewSlot` type; implementation is internal | Host-owned retained component slot | Direct/builder ownership, animation frame retention, root leases, attachment bindings |
| `src/api/extensions/traits/component.ts` | 53 | Component contracts/types | Generic component view/interaction/capability interface | Async adapter, key/paste/tick interaction normalization |
| `src/api/extensions/traits/projector.ts` | 10 | `Projector` type | Generic text transformation extension contract | Async adapter |
| `src/api/extensions/traits/renderer.ts` | 19 | `Renderer`, `RenderContext` types | Generic View transformation/rendering contract | Async queue adapter |
| `src/api/extensions/traits/text-rewriter.ts` | 10 | `TextRewriter` type | Generic text rewrite contract | Async adapter |
| `src/api/extensions/traits/text-visitor.ts` | 10 | `TextVisitor` type | Generic text traversal/observation contract | Async adapter |
| `src/api/presentation/semantic-style.ts` | 346 | Not root-exported; internal semantic bridge | Copy/validate public style values into backend-neutral semantic records | Color, border, decoration, attributes, overflow normalization; defensive freezing |
| `src/api/presentation/style.ts` | 220 | Root style values/types | Sparse style specs, named style references, style selectors, attributes, borders | Functional style composition; selector serialization |
| `src/api/presentation/theme-key.ts` | 8 | `ThemeKey` | Opaque semantic theme key | Non-empty key validation |
| `src/api/presentation/theme.ts` | 242 | Root theme values/types | Immutable theme definitions for styles, colors, text styles, variants | Theme lowering; base/unconditional lookup |
| `src/api/view/geometry.ts` | 41 | `Insets`, `InsetsValue` | Insets normalization and validation | Shared semantic decoration/state input |
| `src/api/view/retained-state.ts` | 236 | `ViewState`, patch/property types | Host-owned geometry/presentation override handle | Native state envelopes, wake scheduling, attachment-kind validation |
| `src/api/view/scene.ts` | 28 | `Scene`, `SceneProducer`/contract | Structural root containing body and optional History sideband | Plain object normalization; runtime root input |
| `src/api/view/semantic-node.ts` | 612 | Internal semantic model, not root-exported | Backend-neutral immutable View node vocabulary and identity | Attachment sidecars, derivation hints, persistent sequence sidecars, freezing |
| `src/api/view/view.ts` | 1,149 | Root `View` and public builders/types | Immutable semantic View authoring surface | Retained-composition dispatch, node identity, layout builders, semantic derivations, wide-sequence persistence |
| `src/index.ts` | 132 | Package root | Deliberate public export allowlist | Separates runtime values, type-only contracts, and internal implementation |

### 1.2 Area responsibilities

#### Content

The content API is split between:

1. **Pure semantic/value types**
   - `TextContent`
   - `RawText`
   - `TextSpan`
   - `TextSelector`
   - `Annotations`
   - `Projection`
   - diff values

2. **Retained native-backed controls**
   - `TextStreamSource`
   - `TextBlockSource`
   - `TextFunnel`
   - `ContentPort`
   - `ContentConnector`

The latter is not merely a DTO layer. `retained.ts` owns native resource lookup, source ID/generation conversion, snapshot decoding, mutation wake behavior, connector status, deferred disposal, port membership, and host mutation guards (`retained.ts:1-32`, `336-810`).

#### Controls and handles

The common `FrameworkHandle` is the JS-local identity/lifecycle base. Concrete controls layer additional ownership rules over it:

- `History`: detached or host-attached scrollback.
- `TextInput`: host-bound input and submitted output.
- `ViewSlot`: retained replacement/animation component.
- `ScrollPane`: retained scrolling component.
- `ContentPort`/`ContentConnector`: content-plane host resources.

The public contract frequently differs from the implementation class exported from the module. For example:

- `ViewSlot` has a public interface and an internal class.
- `ScrollPane` has a public interface while the implementation is `NativeScrollPane`.
- `TextInput` and `History` merge interface/class declarations.
- `ContentPort` and `ContentConnector` expose implementation classes at the root, but their constructors are private and normal creation is controlled by `Tui`.

#### Presentation

Presentation has three layers:

1. Public style/theme authoring (`style.ts`, `theme.ts`, `theme-key.ts`).
2. Backend-neutral semantic style records (`semantic-style.ts`).
3. Native-boundary lowering in transport code, outside this primary scope.

The semantic layer deliberately copies and freezes public values before structural transport consumes them (`semantic-style.ts:1-10`).

#### View

`view.ts` is the largest public-authoring module. It owns:

- semantic View node construction;
- a process/global-symbol monotonic semantic NodeId allocator;
- immutable fluent modifiers;
- rows, columns, grids, hanging indents, containers, clamps, content-max;
- text wrapping/alignment;
- state and content attachments;
- retained-composition dispatch;
- persistent sequence sidecars for wide axes/grids;
- derivation hints used by retained transport.

`semantic-node.ts` is the private semantic representation used by both View authoring and retained transport. It explicitly claims no knowledge of native ABI or structural transport (`semantic-node.ts:1-10`).

---

## 2. Types, APIs and contracts

## 2.1 Package-root exports

`src/index.ts` is the authoritative package-root export surface.

### Runtime value exports

`src/index.ts` exports these runtime values:

- Errors:
  - `TuiError`
  - `asTuiError`
  - `isTuiCancelledError`
  - `isTuiError`
  - `tuiError`
- View/state/builders:
  - `ViewState`
  - `View`
  - `ChildrenBuilder`
  - `GridBuilder`
  - `GridRowBuilder`
  - `defineView`
  - `state`
  - `Insets`
- Styles/themes:
  - `Style`
  - `StyleRef`
  - `StyleSelector`
  - `StyleSpec`
  - `StyleStateKey`
  - `StyleStateValue`
  - `Theme`
  - `ThemeKey`
  - `themeColor`
- Text/content:
  - `TextSelector`
  - `TextSpan`
  - `History`
  - `TextInput`
  - `TextContent`
  - `RawText`
  - `ContentConnector`
  - `ContentPort`
  - `TextBlockSource`
  - `TextFunnel`
  - `TextStreamSource`
  - `Annotations`
  - `Projection`
  - `ProjectionBuilder`
  - `Smooth`
  - `DiffRange`
  - `DiffLine`
  - `DiffHunk`
  - `DiffRenderer`
- Root/runtime:
  - `Scene`
  - `Tui`

Evidence: `src/index.ts:1-132`.

### Type-only exports

The root also exports:

- colors/theme types;
- border/style types;
- component contracts and events;
- `ComponentHandle`, `FrameworkHandle`, `HandleId`;
- layout types;
- `HistoryLayout`;
- `Output`;
- runtime events and runtime contracts;
- `Projector`, `Renderer`, `RenderContext`;
- `SceneProducer`;
- text annotation/span/role/selector types;
- `TextInputOptions`;
- `TextRewriter`, `TextVisitor`;
- content Source/Funnel/Connector types and status contracts;
- ViewState patch/property types;
- `ViewComponent`;
- `ViewSlot`, `ScrollPane`;
- diff and projection types;
- semantic annotation types;
- `TextFormat`, `TextOrigin`;
- `OverflowIndicator`.

A notable distinction is that several implementation classes exist in their modules but are deliberately not root-exported as runtime values:

- `NativeScrollPane` is not root-exported.
- `ViewSlot` is exported from the root with `export type`, not as a runtime constructor.
- `ScrollPane` is type-only.
- `Output` is type-only.
- `FrameworkHandle` and `ComponentHandle` are type-only at the root.
- The semantic-node and semantic-style internal functions/types are not root-exported.
- Trait adapter implementation classes are not root-exported.

### Effective external visibility

Because `package.json:7-10` exposes only `"."`, `"./testing"`, and `"./native-stage"`, direct consumers cannot rely on arbitrary `src/api/**` subpaths through normal package resolution. The root index therefore materially defines the public API, while module-level `export` declarations also serve internal/package-local consumers and tests.

This explains why `packages/iyon-tui/tests/tui_traits.test.ts` imports `AsyncComponentAdapter`, `RendererAdapter`, and `TextRewriterAdapter` directly from `src/api/extensions/traits/**` (`tui_traits.test.ts:3-6`), even though those adapter classes are not available from `@iyon/tui` root.

---

## 2.2 View contracts and builders

### Public View identity

`View` is an immutable semantic facade:

- `kind = "view"` (`view.ts:267`);
- the actual semantic node is held in a `WeakMap<View, SemanticViewNode>` sidecar (`semantic-node.ts:296-306`);
- the public object is frozen after construction (`view.ts:269-272`, `560-570`);
- semantic nodes have positive safe-integer IDs allocated by a monotonic counter (`view.ts:146-158`);
- `viewNodeId`, `nodeIdPair`, and `viewNodeIdHighWater` are internal/transport-facing helpers (`view.ts:1009-1028`).

The NodeId is a semantic JS-side identity, not a native ABI identifier. `semantic-node.ts:161-189` distinguishes semantic node identity, native-independent state attachment, and native-independent content attachment.

### Static View constructors

`View` provides:

- `View.text(value)` — plain text;
- `View.styledText(spans)` — styled text spans;
- `View.spacer(rows)` — vertical spacer;
- `View.horizontal(children)` — row;
- `View.vertical(children)` — column;
- `View.hanging(prefix, continuation, body)` — hanging layout;
- `View.grid(specification)` — grid;
- `View.content(port)` — retained content-host occurrence;
- `View.diff(hunks)` — semantic diff;
- `View.contentMax(maxRows, child)` — content-limited wrapper;
- `View.key(key, build)` — keyed child-owner group.

Evidence: `view.ts:263-363`.

`View.key` is not a generic key-value annotation. It requires an active retained child-owner (`view.ts:263-264`) and exists to preserve execution identity for keyed component invocations. The comments explicitly state that keys protect child execution identity, do not create independent schedulable scopes, and do not consume unkeyed ordinals (`view.ts:250-261`).

### Fluent View modifiers

The public fluent modifiers include:

- attributes:
  - `bold`
  - `dim`
  - `italic`
  - `underline`
  - `reversed`
  - `strikethrough`
  - `textAttribute`
- presentation:
  - `padding`
  - `background`
  - `foreground`
  - `border`
  - `style`
  - `styleState`
- retained state:
  - `state`
- layout:
  - `container`
  - `clampRows`
  - `fitWidth`
  - `fillWidth`
  - `fitHeight`
  - `fillHeight`
  - `minWidth`
  - `maxWidth`
  - `minHeight`
  - `maxHeight`
- text layout:
  - `wrap`
  - `noWrap`
  - `textAlign`

Evidence: `view.ts:365-475`.

All direct fluent operations return a new semantic value rather than mutating the original. `view.ts:482-508` consolidates decorations, copies nested semantic records, preserves attachment references, and adds a scalar-only derivation hint when possible.

### Retained construction versus direct semantic construction

`isRetainedConstruction()` is true only when an execution scope is active and `semanticConstruction.raw` is false (`view.ts:152-154`).

Therefore:

- Outside retained evaluation, constructors create semantic nodes directly.
- Inside retained evaluation, the same public APIs dispatch into `composition/compose.ts`.
- The public API does not expose a separate opt-in switch.

Examples:

- `View.text`: direct semantic node versus `composeText` (`view.ts:299-307`).
- `View.vertical`: direct child-builder normalization versus `composeVertical` (`view.ts:338-347`).
- `View.style`: direct semantic style merge versus `composeStyle` (`view.ts:393-405`).
- `View.state`: direct attachment versus `composeState` (`view.ts:408-415`).

This is a critical contract: callers author the same API in either mode, while runtime/composition controls whether identity and state subscriptions are retained.

### ChildrenBuilder

`ChildrenBuilder` supports:

- `child(view)`
- `childrenOf(views)`
- `gap(value)`
- `fixed(size, view)`
- `flex(view)`
- `flexMax(maxRows, view)`
- `contentMax(maxRows, view)`
- `gapValue()`

Evidence: `view.ts:196-219`.

The internal entries are ordinary mutable arrays during construction, then normalized into frozen semantic records by `semanticLayoutChildren` and `createSemanticView`.

### GridBuilder and GridRowBuilder

`GridRowBuilder` supports:

- `cell(view)`
- `cellWith(spec, view)`

`GridBuilder` supports:

- `columns(columns)`
- `columnGap(value)`
- `rowGap(value)`
- `row(buildOrRow)`
- `rowWith(track, build)`

Evidence: `view.ts:166-194`.

`GridCell` accepts optional column/row spans and horizontal/vertical alignment. `gridViewFromBuilder` normalizes defaults:

- span defaults to `1`;
- horizontal alignment defaults to `"start"`;
- vertical alignment defaults to `"top"`;
- row track defaults to `{ kind: "content" }`.

Evidence: `view.ts:124-142`, `638-658`.

### Wide sequence sidecars

For axes with more than 1,024 children, the semantic node receives a `PersistentSeq` sidecar rather than relying only on the eager child array (`view.ts:825-832`).

For grids with more than 1,024 total cells, the semantic node receives:

- persistent cell sequence;
- row offsets;
- row tracks;
- per-row column-to-sequence-index maps.

Evidence: `view.ts:874-888`.

Axis replacements and splices use persistent sequence edits and derivation metadata rather than copying all children:

- `axisSetChildForTransport` (`view.ts:662-683`);
- `axisSpliceForTransport` (`view.ts:685-715`);
- `gridSetCellForTransport` (`view.ts:717-776`);
- `buildWideAxisNode` (`view.ts:967-994`);
- `buildWideGridNode` (`view.ts:891-933`).

These are internal transport helpers, not root authoring APIs.

---

## 2.3 Scene and runtime contracts

### Scene

`SceneContract` is:

```ts
interface SceneContract {
  readonly history?: History;
  readonly body: View;
}
```

`SceneProducer` is either a `SceneContract` or a zero-argument producer returning one (`scene.ts:4-11`).

`Scene` is a concrete value with:

- `body`;
- optional `history`;
- `Scene.from(value)` normalization.

Evidence: `scene.ts:13-28`.

`TuiRuntime.render` accepts either a direct scene value or a producer (`runtime.ts:59-90`, `399-405`).

### Runtime root behavior

The runtime owns:

- native host;
- terminal dimensions;
- retained execution runtime;
- root retained builder;
- retained root boundary;
- attached/staged History;
- host-created handles;
- attachment binding state;
- runtime error channel.

Evidence: `runtime.ts:108-141`.

Direct `render(scene)` and producer `render(() => scene)` are distinct ownership modes:

- direct values are normalized, validated, prepared, committed, and replace the root;
- producers are installed into one persistent root execution scope;
- tracked state reads inside a producer subscribe the root scope;
- later state changes re-drive the producer without another explicit `render`.

Evidence: `runtime.ts:390-405`, `451-497`, and `composition/tracked-state.ts:5-18`.

### History sideband

History is not structurally embedded in the body View. It is a Scene sideband:

- `Scene.history` carries the handle;
- runtime stages and commits History separately from body structure;
- attachment is one-way and single-host;
- replacing a bound History with a different instance is rejected.

Evidence: `scene.ts:4-8`, `runtime.ts:224-317`, `history.ts:28-42`.

---

## 2.4 State contracts

There are two distinct public state mechanisms.

### `State<T>` from `state()`

`State<T>` is a JS-side tracked invalidation source:

```ts
interface State<T> {
  readonly value: T;
  set(value: T): void;
  update(update: (previous: T) => T): void;
}
```

Evidence: `composition/tracked-state.ts:104-124`.

Semantics:

- reading `.value` during retained evaluation subscribes the current execution scope;
- reading outside evaluation is untracked;
- writes compare with `Object.is`;
- unchanged writes do nothing;
- changed writes invalidate subscribed scopes and enqueue them once;
- writes from inside a component body throw `TUI_EXECUTION_STATE_WRITE_DURING_EVALUATION`;
- dependency sets become committed only after successful evaluation.

Evidence: `tracked-state.ts:5-18`, `41-92`.

### `ViewState`

`ViewState` is a host/native-backed retained override handle, independent of View construction except for its attachment identity (`retained-state.ts:140-145`).

It supports:

- `setGeometry`
- `clearGeometry`
- `setPresentation`
- `clearPresentation`
- `setStyleState`
- `clearStyleState`

Evidence: `retained-state.ts:159-204`.

Geometry properties:

- `width`
- `height`
- `padding`
- `minWidth`
- `maxWidth`
- `minHeight`
- `maxHeight`
- `gap`
- `alignment`
- `borderEdges`

Presentation properties:

- `foreground`
- `background`
- `borderColor`
- `borderStyle`
- `borderGlyphs`
- `textAttributes`
- `style`

Evidence: `retained-state.ts:39-115`.

`View.state(state)` attaches the JS-local `HandleId` to the semantic node and retains a strong reference to the handle (`view.ts:408-415`, `578-594`). Native attachment validation is performed later against accepted node kinds.

These two state concepts must not be conflated:

- `State<T>` controls retained execution invalidation.
- `ViewState` is native retained per-occurrence geometry/presentation override state.

---

## 2.5 Handle contracts and properties

### FrameworkHandle

`FrameworkHandle` provides:

- opaque branded `HandleId`;
- `kind`;
- `disposed`;
- `dispose()`;
- protected `nativeAs`;
- protected `ensureOpen`;
- protected `call`.

Evidence: `framework-handle.ts:9-60`.

`dispose()` is idempotent from the wrapper perspective. `call()` checks wrapper liveness and converts thrown values through `asTuiError`.

The handle’s JS ID is intentionally distinct from native identifiers. Native resources are stored in the runtime/native registry and retrieved through `nativeResourceOf`.

### History

`History` supports:

- detached `new History()`;
- host-created `Tui.createHistory()`;
- `layout()`;
- `push(view)`;
- `freeze(unit, view)`;
- `discardLive(unit)`;
- `setLayout(layout)`.

Evidence: `history.ts:9-16`, `44-104`.

Important lifecycle contract:

- detached histories can be used for layout and pushes before host attachment;
- a detached history transfers to one Tui only;
- host-created histories are already attached;
- caller-owned attached histories become unavailable after Tui close;
- `freeze` and `discardLive` require attachment;
- push/freeze use the retained materialization path and release only a temporary native View reference afterward.

Evidence: `history.ts:28-42`, `68-90`.

### TextInput

`TextInput` supports:

- `text()`;
- `cursorBytes()`;
- `setText(value)`;
- `clear()`;
- `submitted()`;
- `setMultiline(enabled)`;
- `isMultiline()`;
- `view()`.

Evidence: `text-input.ts:10-24`, `44-72`.

It is host-bound only. Its constructor is private-token guarded (`text-input.ts:47-50`). `submitted()` lazily creates and then caches one JS `Output<string>` facade per TextInput (`text-input.ts:56-65`). A weak association maps output back to its owning input for `Tui.route` validation (`text-input.ts:42`, `82-84`).

### Output

`Output<T>` has:

- an opaque private brand;
- `kind = "output"`;
- a private constructor;
- a type-only variance marker.

Evidence: `output.ts:1-8`.

Consumers cannot manufacture a routed channel that lacks native routing identity.

### ViewSlot

The public ViewSlot contract supports:

- `view()`;
- `capabilities()`;
- `setView(View | (() => View))`;
- `setAnimation(frames, intervalMs)`;
- `setAnimationAtCycleBoundary(frames, intervalMs)`;
- `stopAnimation(view)`;
- `revision()`.

Evidence: `view-slot.ts:50-58`.

Ownership modes:

- `setView(() => View)` installs a slot-owned retained execution root;
- `setView(View)` installs a direct View and releases any builder root only after successful publication;
- animation installation similarly relinquishes builder ownership only after successful native installation.

Evidence: `view-slot.ts:156-228`, `331-334`.

Animation behavior:

- empty frame arrays fail;
- up to four acquired refs use scalar native methods;
- larger animations use a reusable `Uint32Array` scratch buffer;
- all acquired temporary refs are released in `finally`;
- frame Views are retained by identity rather than rebuilt every cycle.

Evidence: `view-slot.ts:284-379`.

### ScrollPane

The public ScrollPane contract supports:

- `view()`;
- `capabilities()`;
- `setContent(View | (() => View))`;
- `followEnd()`.

Evidence: `scroll-pane.ts:24-29`.

Its direct/builder ownership semantics mirror ViewSlot. Scroll state is explicitly preserved across content rebuilds (`scroll-pane.ts:119-123`).

### ContentPort and ContentConnector

`ContentPort<TContent>`:

- is host-owned;
- owns Connector membership, not Source data;
- accepts one Source plus Funnel per `connect`;
- tracks connector wrappers in a JS `Set`;
- supports `deactivate`, `mounted`, `isMounted`, `connectorCount`;
- can be disposed and then synchronizes connector lifecycle.

Evidence: `retained.ts:560-658`.

`ContentConnector<TContent>`:

- links exactly one Source, Funnel, and Port;
- supports `activate`, `deactivate`, `status`, `dispose`;
- has explicit `"disposing"` and `"disposed"` phases;
- final wrapper disposal waits for native disposal status and removes itself from the Port set.

Evidence: `retained.ts:661-796`.

Content connector phases are:

```text
idle
waiting-for-mount
activation-pending
active
failed
disposing
disposed
blocked-geometry
unsupported-backend
```

Evidence: `retained.ts:143-164`.

### Sources

`TextStreamSource` supports:

- source ID/generation/environment identity;
- content generation;
- snapshots and stats;
- `append`/`appendUtf8`;
- `replace`/`replaceUtf8`;
- `clear`;
- `seal`;
- `truncateHead`.

Evidence: `retained.ts:336-407`.

`TextBlockSource` supports replacement-style operations:

- identity/snapshot/stat APIs;
- `replace`/`replaceUtf8`;
- `clear`;
- `truncateHead`.

Evidence: `retained.ts:409-468`.

Both are environment-owned, native-backed handles created by static `.create(options)` methods. Their native resources are registered with the framework handle registry, and mutations enter the content FFI with a wake callback.

### Funnels

`TextFunnel` is immutable and source-neutral:

- modes: `plain`, `markdown`, `diff`, `ansi`;
- wrapping: `word`, `grapheme`, `noWrap`;
- delivery: immediate or smooth;
- hyperlinks boolean.

Factories:

- `TextFunnel.plain`
- `TextFunnel.markdown`
- `TextFunnel.diff`
- `TextFunnel.ansi`

Modifiers:

- `.smooth(options)`
- `.immediate()`

Evidence: `retained.ts:111-138`, `470-558`.

A Funnel contains no Source data; the generic `Funnel<TContent>` interface uses a phantom `__content` marker (`retained.ts:133-138`).

---

## 2.6 Content, annotations, projections, and diff contracts

### TextContent and RawText

`TextContent` supports:

- `plain(value)`;
- `markdown(value)`;
- `raw(value, origin?)`;
- `withOrigin(origin)`;
- `text()`;
- `render()`;
- `walk(visitor)`;
- `rewrite(rewriter)`.

Evidence: `text-content.ts:3-24`.

`RawText` stores an unparsed value/origin and converts it to `TextContent` via `.content()` (`text-content.ts:26-30`).

`TextContent.render()` is a direct coupling to `View.text`, not a renderer pipeline (`text-content.ts:20-22`).

### Text selectors and spans

Text roles include paragraphs, headings, quotes, lists, code blocks, tables, inline semantic marks, links, images, and raw forms (`text.ts:3-26`).

Text parts include list markers, task markers, quote markers, code labels, table/thematic rules, and image fallbacks (`text.ts:28-35`).

`TextSelector` supports:

- role;
- part;
- annotation namespace/name;
- heading/inlineCode/codeBlock shorthands;
- language;
- origin;
- format;
- focused/focusWithin;
- arbitrary state key/value.

Evidence: `text.ts:59-110`.

`TextSpan` supports plain or styled text. `TextSpan.styled` normalizes a `StyleSpec` or `StyleRef` into a `StyleRef` (`text.ts:168-183`).

### Annotations

`Annotations` contains:

- readonly semantic tags;
- readonly scalar properties;
- `withTag`;
- `withProperty`;
- `containsTag`.

Evidence: `annotations.ts:4-29`.

`TEXT_SOURCE_ANNOTATION_SCHEMA` is a frozen closed schema containing:

- `tag`: clip truncation;
- `style`: clip truncation;
- `atomic`: drop truncation;
- `point`: point retention.

Evidence: `annotations.ts:15-20`.

### Projection

`Projection` associates a `TextContent` source with ordered spans and provides:

- `text()`;
- `sourceRange()`.

`ProjectionBuilder` adds spans and finishes a Projection. `Smooth` stores a non-negative integer offset.

Evidence: `projection.ts:3-25`.

#### Consequential validation observation

`Projection` documents spans as “contiguous and ordered,” but `validateSpans` rejects only when `sourceStart < expected` and permits `sourceStart > expected` (`projection.ts:28-34`). Thus:

- overlapping/backtracking spans are rejected;
- gaps between spans are accepted.

This is a source-level contract discrepancy between the comment/error wording and the actual predicate. No change was made.

### Diff

`DiffRange` validates non-negative safe integer start/count and provides `isEmpty`/`end` (`diff.ts:6-17`).

`DiffLine` validates:

- line kind;
- line termination;
- string text;
- positive safe integer old/new coordinates.

Factory helpers create context/addition/deletion lines (`diff.ts:19-47`).

`DiffHunk.validate()` walks lines, checks coordinate progression, and verifies old/new consumed counts equal the declared ranges (`diff.ts:49-78`).

`DiffHunk.render()` and `DiffRenderer.render()` lower the semantic diff into `View.diff` (`diff.ts:80-94`). This is deliberate content-to-presentation coupling at the public semantic layer.

---

## 2.7 Style, theme, and property contracts

### StyleSpec

`StyleSpec` is a sparse direct style record with:

- optional foreground/background;
- text attributes map.

Methods return new specs:

- `foreground`;
- `background`;
- `attribute`;
- `bold`;
- `dim`;
- `italic`;
- `underline`;
- `reversed`;
- `strikethrough`;
- `plain`.

Evidence: `style.ts:34-80`.

`Style.plain()` and `Style.new()` are convenience factories (`style.ts:217-220`).

### StyleRef

`StyleRef` represents:

- a direct style (`StyleRef.direct`);
- a named theme style plus optional sparse local override (`StyleRef.theme`);
- normalization from `StyleSpec | StyleRef` (`StyleRef.from`);
- merged local overrides (`overrides`).

Evidence: `style.ts:83-107`.

`View.style` distinguishes named `StyleRef` replacement from sparse direct style overlay (`view.ts:393-405`).

### Style selectors

`StyleSelector` supports positive conjunctions of:

- focused;
- focusWithin;
- arbitrary application-owned state key/value pairs.

Evidence: `style.ts:110-163`.

No negative predicates or priority model exist in this API. Selector equality in themes compares focus bits and exact state key/value sets (`theme.ts:231-238`).

### Theme

`Theme` is functionally immutable:

- `Theme.new()`;
- `withStyle`;
- `withStyleVariant`;
- `withColor`;
- `withColorVariant`;
- `withTextStyle`;
- `style`;
- `color`.

Evidence: `theme.ts:82-185`.

Theme definitions contain private maps for:

- named styles and variants;
- named colors and variants;
- text selector styles.

The native-boundary representation is produced by `themeDefinitionFor` (`theme.ts:188-213`).

`Theme.color` only applies an unconditional variant, whereas `themeDefinitionFor` preserves all variants for later native lowering. This is a deliberate difference between convenience lookup and complete boundary serialization.

### Semantic style normalization

`semantic-style.ts` converts public records into frozen backend-neutral values:

- `semanticColorFor`;
- `semanticStyleFor`;
- `semanticBorderFor`;
- `semanticTextSpanFor`;
- `semanticOverflowFor`;
- `semanticDecorationFor`;
- cloning/merging helpers.

Evidence: `semantic-style.ts:39-216`.

The normalization layer validates:

- ANSI color vocabulary;
- RGB/indexed byte ranges;
- text attributes;
- border style/edges/glyphs;
- inset ranges;
- width/height modes;
- style state names/values.

It intentionally keeps transport/native encodings out of the semantic representation (`semantic-style.ts:1-10`).

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
Public consumer
   │
   ▼
@iyon/tui package root (src/index.ts)
   │
   ├── View / builders
   │      ├── semantic-style
   │      ├── semantic-node
   │      └── composition when retained execution is active
   │
   ├── styles/themes
   │      ├── StyleSpec / StyleRef / StyleSelector
   │      ├── Theme / ThemeKey
   │      └── semantic-style and native style lowering
   │
   ├── content values
   │      ├── TextContent / TextSpan / TextSelector
   │      ├── Diff values → View.diff
   │      ├── Projection / Smooth
   │      └── Annotations
   │
   ├── retained content handles
   │      ├── FrameworkHandle
   │      ├── native content control / FFI
   │      ├── resource registry
   │      └── runtime environment wake broker
   │
   ├── retained View controls
   │      ├── FrameworkHandle
   │      ├── retained DAG / native View ABI
   │      ├── composition execution runtime
   │      └── attachment bindings
   │
   └── Tui / Scene / runtime
          ├── native host
          ├── root retained boundary
          ├── state/content attachment preparation
          ├── runtime scheduler
          └── terminal backend
```

### 3.2 Reverse consumers

| Public/API symbol | Actual major consumers |
|---|---|
| `View`, builders | `runtime/Tui`; `composition/compose.ts`; retained structural transport; tests; `tui-consumer-fixture` |
| `ViewState` | `runtime/Tui.viewState`; `View.state`; state transport; attachment registry |
| `State<T>` | `defineView`/execution runtime; consumer fixture scoped invalidation tests |
| `defineView` | retained execution runtime; external consumer fixture |
| `Scene` | `Tui.render`; consumer fixture |
| `History` | `Tui` root sideband; tests/fixtures; consumer fixture |
| `ViewSlot` | `Tui.createViewSlot`; composition scope projection; external fixture |
| `ScrollPane` | `Tui.createScrollPane`; external fixture |
| `TextInput` | `Tui.createTextInput`; native input routing; external fixture |
| `Output<T>` | `TextInput.submitted`; `Tui.route`; component `emit` contracts |
| `ContentPort` | `Tui.contentPort`; `View.content`; content attachment/runtime |
| `ContentConnector` | `ContentPort.connect`; runtime/native content control |
| `TextStreamSource`/`TextBlockSource` | Source caller code; `ContentPort.connect`; content FFI |
| `TextFunnel` | `ContentPort.connect`; native text funnel control |
| `Theme` | `Tui.open`/`setTheme`; native style lowering |
| `StyleSpec`/`StyleRef` | View modifiers, TextSpan, themes, state patches, native lowering |
| trait interfaces | TypeScript component/projector/renderer extension callers; selected package tests |
| adapter classes | Package-local tests/direct internal imports; not package-root consumers |
| `DiffRenderer` | `DiffHunk.render`; direct caller code; View semantic construction |
| `Projection`/`Smooth` | Public content/projection callers; no substantial in-repository production consumer found in inspected paths |

### 3.3 Create/destroy/lifetime ownership

```text
Tui.open()
  └── NativeTuiHost
       ├── retained execution runtime
       ├── root RetainedRootBoundary
       ├── host registration / wake broker
       ├── owned History handles
       ├── ViewState handles
       ├── ContentPort / ContentConnector resources
       ├── TextInput handles
       ├── ViewSlot handles
       └── ScrollPane handles

View / semantic node
  ├── immutable semantic node identity
  ├── weak sidecars for derivation/sequences
  └── strong attachment references while reachable

ContentPort
  └── Connector membership set
       └── one Source + one Funnel + one native Connector

ViewSlot / ScrollPane
  ├── native component resource
  ├── current content root lease
  ├── optional owned builder root
  └── attachment bindings

TextInput
  └── stable submitted Output facade
       └── weak association back to TextInput
```

Ownership consequences:

- `Tui` owns factory-created host-bound controls and disposes them during close/exit.
- Detached `History` remains caller-owned unless attached.
- Sources are environment-owned rather than Tui-host-owned.
- View attachment strong references prevent a caller from losing an attached handle merely because the original JS variable is dropped.
- Retained root boundaries keep current content leases through replacement transactions.
- Temporary native View references acquired for `History`, slots, panes, and animations are released after native ownership has been established.

### 3.4 Boundary directions

Important architectural edges:

- `View` → `composition` when called from a retained execution scope (`view.ts:72-104`, `152-154`).
- `View` → semantic-node sidecar (`view.ts:20-64`).
- `semantic-node` → only types from controls/style/theme; no transport import (`semantic-node.ts:1-16`).
- `ContentPort`/`Connector` → native content control/FFI and runtime registries (`retained.ts:1-32`).
- `ViewState` → native state control/envelopes (`retained-state.ts:17-23`).
- `History` → native View ABI and native History contract (`history.ts:1-7`).
- `ViewSlot`/`ScrollPane` → retained DAG, native View ABI, composition execution, attachment runtime (`view-slot.ts:1-22`, `scroll-pane.ts:1-22`).
- `Theme`/styles → native style lowering through runtime, not directly from public consumers (`runtime.ts:1-3`, `880-894`).

---

## 4. Execution paths and state transitions

## 4.1 Direct View construction

Representative path: `View.text("hello")`.

```text
consumer
  → View.text(value)
  → validate string
  → isRetainedConstruction() == false
  → create SemanticTextNode draft
  → createSemanticViewNode(nextNodeId(), draft)
  → install semantic node in WeakMap<View, SemanticViewNode>
  → freeze View wrapper
  → returned immutable View
```

Evidence: `view.ts:299-307`, `semantic-node.ts:286-293`, `view.ts:269-272`.

A direct modifier such as `.padding(1)`:

```text
base View
  → View.padding
  → normalize Insets
  → merge/decorate semantic node
  → allocate a fresh semantic identity
  → copy attachment references
  → optionally set commonScalar derivation
  → return a new frozen View
```

Evidence: `view.ts:377`, `482-508`, `793-817`.

### Identity behavior

Every semantic replacement receives a fresh NodeId. This includes text layout patches; `view.ts:532-553` explicitly avoids reusing the base text node ID because doing so would permit stale layout promotion.

## 4.2 Retained composition construction

Representative path: `Tui.render(() => ({ body: View.text(state.value) }))`.

```text
Tui.render(builder)
  → ensure signal/open state
  → drain pending execution
  → assert no retained protocol mutation
  → create root producer
  → producer evaluates Scene.from(builder())
  → View APIs see active executionContext
  → View.text dispatches to composeText
  → State.value read links current execution scope
  → producer output published through OwnedBuilderRoot
  → prepareRootPublication
  → retained root boundary prepares semantic/native install
  → commit desired root and attachments
  → host visibility barrier/frame drain
```

Evidence: `runtime.ts:451-497`, `view.ts:299-307`, `tracked-state.ts:49-54`.

On subsequent state mutation:

```text
State.set(next)
  → Object.is comparison
  → subscribed execution scope invalidated
  → retained runtime queues scope once
  → scope evaluates producer
  → new View semantic output prepared
  → old dependency set remains until successful commit
  → commit updates subscriptions and root publication
```

Evidence: `tracked-state.ts:65-92`; execution runtime is imported and used by `defineView`/Tui.

Failed evaluation retains the old dependency set; this avoids losing subscriptions when an evaluation aborts (`tracked-state.ts:16-18`).

## 4.3 Scene replacement and History sideband

Direct scene path:

```text
Tui.render(scene)
  → Scene.from(scene)
  → semanticNodeOf(scene.body)
  → validate History owner/resource if present
  → fast no-op if body and effective History identities unchanged
  → prepare root publication
       ├── prepare semantic attachments
       ├── prepare retained structural root
       └── stage History binding
  → commit History binding
  → commit structural root
  → commit desired attachment revision
  → mark host pending
  → dispose prior root builder if switching from producer
  → flush visible host
```

Evidence: `runtime.ts:499-548`.

A direct render preparation failure restores staged History and leaves the prior committed state authoritative (`runtime.ts:535-548`). A later host visibility failure leaves the newly accepted desired revision retryable, as described in the runtime comments (`runtime.ts:482-496`).

## 4.4 Content source mutation

Representative path: `source.append(text, annotations)`.

```text
TextStreamSource.append
  → FrameworkHandle.call
  → validate text string and annotation array
  → nativeResourceOf<NativeTextSourceContract>
  → appendTextSource(native, text, annotations, sourceWake)
  → native content mutation
  → return revision/wake result
  → sourceWake marks environment pending
```

Evidence: `retained.ts:380-384`, `214-217`.

Source snapshots/stats:

```text
source.snapshot()/stats()
  → native source contract
  → native numeric/string u64 fields
  → sourceBigInt validation and normalization
  → payload array → Uint8Array
  → kind-2 style payload decoding
  → public snapshot/stat object
```

Evidence: `retained.ts:276-319`.

The API accepts native u64 values represented as `bigint`, safe integer number, or decimal string, then enforces `[0, 2^64-1]` (`retained.ts:219-238`).

## 4.5 Content connection and visibility

```text
Tui.contentPort()
  → host content-port native resource
  → createContentPort wrapper
  → FrameworkHandle registration

port.connect(source, funnel)
  → mutation guard
  → live Source validation
  → TextFunnel/family validation
  → source and port native resource lookup
  → connectContent(port, source, native funnel control)
  → ContentConnector wrapper registration
  → Port connector membership set

View.content(port)
  → port liveness validation
  → semantic ContentHost node with contentAttachment = port.id
  → strong semantic attachment reference
  → retained/root attachment preparation
  → host/native content binding
```

Evidence: `runtime.ts:601-629`, `retained.ts:592-614`, `view.ts:285-296`.

Connector activation/deactivation is control-plane mutation and returns native wake information. Connector disposal is staged:

```text
connector.dispose()
  → begin JS registry disposal
  → native dispose request
  → phase becomes disposing
  → host/frame lifecycle progresses
  → status reaches disposed
  → wrapper finalizes
  → Port forgets connector
  → framework handle released
```

Evidence: `retained.ts:756-796`.

## 4.6 ViewState mutation

```text
Tui.viewState()
  → native host.viewState()
  → ViewState wrapper with owner/environment/host tokens
  → ownHandle()

view.state(state)
  → state liveness validation
  → retained composition attachment or direct stateAttachment id
  → strong reference retained by semantic node

state.setGeometry/setPresentation
  → mutation guard
  → geometry/presentation envelope packing
  → FrameworkHandle.call
  → native state operation
  → wake bit inspection
  → host registration markPending if drain required
```

Evidence: `runtime.ts:576-598`, `view.ts:408-415`, `retained-state.ts:159-214`.

The state handle is independent of View construction, but its attachment is occurrence-specific because the semantic node carries the attached handle ID.

## 4.7 ViewSlot direct and builder content

Initial construction:

```text
Tui.createViewSlot(initial)
  → Tui mutation preparation
  → retained materialize initial View
  → host.createViewSlotRef(native ref)
  → ViewSlot wrapper construction
  → RetainedRootBoundary.adopt(initial)
  → attachment bindings commit
```

Evidence: `runtime.ts:643-655`, `view-slot.ts:30-45`, `121-145`.

Direct replacement:

```text
slot.setView(nextView)
  → reject retained-protocol reentrant mutation
  → prepare attachments
  → boundary.prepareInstall(nextView)
  → if refused: abort attachments, leave old content
  → publication.commit()
  → commit attachments
  → update current View
  → dispose owned builder root last
```

Evidence: `view-slot.ts:204-236`.

Builder replacement:

```text
slot.setView(() => View)
  → ensure builder-capable Tui-owned runtime
  → OwnedBuilderRoot.start or replaceProducer
  → retained evaluation subscribes State reads
  → target.preparePublication delegates to slot.prepareSetView
```

Evidence: `view-slot.ts:167-201`.

Disposal order is explicit:

1. dispose owned builder root;
2. close retained boundary/root lease;
3. clear current View;
4. dispose attachment bindings;
5. dispose native framework resource.

Evidence: `view-slot.ts:382-402`.

## 4.8 ScrollPane

ScrollPane follows the same direct/builder retained content lifecycle as ViewSlot, with an additional native `followEnd` operation.

Direct content replacement preserves the prior root until successful publication and disposes the old builder root afterward (`scroll-pane.ts:189-218`). `followEnd` is a host/native control mutation and is rejected during retained protocol or execution scope (`scroll-pane.ts:221-226`).

## 4.9 TextInput output routing

```text
Tui.createTextInput(options)
  → borderSpec lowered at host boundary
  → host.textInput(multiline, border)
  → createTextInput native wrapper
  → Tui owns handle

input.submitted()
  → first call creates opaque Output facade
  → native output resource registered
  → WeakMap output → input association
  → repeated calls return same JS Output identity

Tui.route(output, routeId)
  → verify output maps to live Tui-owned TextInput
  → native output resource lookup
  → host.route(nativeOutput, routeId)
```

Evidence: `runtime.ts:632-640`, `682-693`, `text-input.ts:56-65`, `82-94`.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation to production-path matrix

| Semantic operation | Primary production path | Alternate route | Selection condition | Failure behavior |
|---|---|---|---|---|
| `View.text`, `View.vertical`, modifiers | Semantic node creation or `compose*` | Direct semantic construction | Active retained execution scope and `semanticConstruction.raw` | Type/range validation; retained composition errors propagate |
| `View.key` | Keyed child-owner composition | None | Requires active child owner | Deterministic `TUI_EXECUTION_NO_ACTIVE_SCOPE` outside retained evaluation |
| `Tui.render(scene)` | Root retained boundary publication | Producer-root path | Argument is object vs function | Invalid semantic body/History/attachment rejects before partial commit |
| `Tui.render(builder)` | `OwnedBuilderRoot` root scope | Direct render | Argument is function | Evaluation failure restores staged sideband; old committed root remains |
| `View.state` | Direct attachment or `composeState` | None | Retained scope active or not | Dead/non-state handle rejected |
| `View.content` | ContentHost semantic attachment | None | Live ContentPort required | Dead/wrong-kind Port rejected |
| `ContentPort.connect` | Native content Connector control | None | Source and TextFunnel checks | Explicit type/validation/native errors |
| Source append/replace | Content FFI source mutation | `appendUtf8`/`replaceUtf8` aliases | Method spelling only; both use string path | Native error converted; wake marked only as requested |
| Connector activation | Native control request | Port-level deactivate | Connector or Port operation | Disposing/disposed/invalid lifecycle surfaced |
| ViewSlot `setView` | Retained boundary prepare/commit | Builder-owned root | View versus function argument | Refusal/failure leaves old content and builder authority intact |
| ViewSlot animation | Native retained refs | Scalar methods for 1–4 frames vs scratch buffer for >4 | Frame count | Empty frame list/refusal/native failure explicit |
| ScrollPane `setContent` | Same retained boundary model | Direct/builder | View versus function | Explicit preparation/update failure |
| History push/freeze | Retained View materialization ref | None | `tryRetainedMaterializeRef` result | Refusal throws; temporary ref always released |
| Text style application | Semantic style merge | Named StyleRef replacement | `StyleRef` with theme key replaces; direct style overlays | Invalid attributes/colors/borders rejected |
| Theme lookup | Convenience base/unconditional lookup | Full theme serialization | `Theme.style/color` versus `themeDefinitionFor` | Missing key returns undefined; malformed Theme object rejected at lowering |
| Diff render | `DiffRenderer` → `View.diff` | `DiffHunk.render` convenience | Same implementation | Range/coordinate validation throws early |
| Projection spans | `ProjectionBuilder` → `Projection` | Direct constructor | Caller choice | Overlap/backtracking rejected; gaps currently accepted |
| State invalidation | `State<T>` subscriber queue | Native `ViewState` wake path | JS tracked state versus attached native state | Writes during evaluation rejected; native state errors propagated |
| Routed output | `Tui.route` native route | No public fabricated Output | Output must be associated with owned TextInput | Foreign/dead Output rejected as invalid handle |

### 5.2 Legitimate alternate modes

The code contains several intentional alternate routes rather than silent fallback implementations:

1. **Direct versus retained View construction**
   - same public authoring surface;
   - composition adds execution identity/subscriptions;
   - raw semantic path is explicitly selected by execution context.

2. **Direct scene versus retained scene producer**
   - direct values replace root immediately;
   - producer owns a retained root and remains subscribed.

3. **Narrow versus wide structural sequences**
   - eager arrays for ordinary sizes;
   - `PersistentSeq` sidecars above 1,024 children/cells.

4. **Small versus large animation frame arrays**
   - scalar native ABI methods for up to four frames;
   - reusable typed-array path for larger arrays.

5. **Immediate versus smooth Funnel delivery**
   - native content connector receives delivery policy;
   - no second JS-per-tick transport path is introduced.

### 5.3 Explicit refusal and no masking

Several callers explicitly reject an unavailable retained path:

- `History.push` and `freeze` throw when `tryRetainedMaterializeRef` returns undefined (`history.ts:68-76`, `80-90`);
- initial slot/pane construction throws on retained materialization refusal (`view-slot.ts:36-44`, `scroll-pane.ts:49-57`);
- slot/pane replacement throws on undefined prepared publication (`view-slot.ts:213-216`, `scroll-pane.ts:199-202`);
- animation frame materialization refusal throws (`view-slot.ts:308-311`);
- connector status preserves failure phases instead of converting them to invisible absence (`retained.ts:732-753`).

The source comments repeatedly state that there is no parallel fallback transport path for the retained structural route.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Cache/sidecar inventory

| Mechanism | Owner | Key | Retention | Invalidation/cleanup |
|---|---|---|---|---|
| Semantic View node map | `semantic-node.ts` | `View` object identity | Weak | Dies with View wrapper |
| Semantic attachment presence | `semantic-node.ts` | Semantic node identity | Weak | Derived node gets explicit summary |
| Strong attachment references | `semantic-node.ts` | Semantic node identity | Weak key, strong values | Copied across derivations; dies when node unreachable |
| Semantic derivation sidecar | `semantic-node.ts` | Semantic node identity | Weak | Replaced by new derived node |
| Axis persistent sequence | `semantic-node.ts`/`view.ts` | Derived semantic node | Weak sidecar | New sequence/node on edit |
| Grid persistent sequence | `semantic-node.ts`/`view.ts` | Derived semantic node | Weak sidecar | New sequence/node on edit |
| Lazy wide axis child array | `view.ts` | Derived node closure | Until node is collected | Materialized only when `.children` is accessed |
| Lazy wide grid rows | `view.ts` | Derived node closure | Until node is collected | Materialized only when `.rows` is accessed |
| Node ID allocator | `view.ts` | Global `Symbol.for` slot | Process/global runtime | Monotonic; no reuse |
| TextInput submitted Output | `text-input.ts` | TextInput instance | Strong field on input | Cleared only with input collection/disposal path |
| Output → input reverse map | `text-input.ts` | Output object | Weak key and WeakRef value | Does not keep input alive |
| ViewSlot animation scratch | `view-slot.ts` | Native slot resource object | Weak key, typed-array value | Dies with native slot object |
| Theme/style lowering cache | Transport/runtime, outside primary API | StyleRef/theme epoch | Runtime-owned | Reset by `Tui.setTheme` through `resetStyleRefCacheForThemeChange` |
| State subscribers | `tracked-state.ts` | Public State wrapper/source | Strong source set, scope references | Unsubscribe on scope lifecycle/dependency replacement |
| Content Port connector set | `retained.ts` | Port wrapper | Strong connector references | Connector finalization calls `forgetConnector` |

### 6.2 Per-operation work

- `View` direct scalar modifiers allocate a new semantic node and copy relevant semantic records.
- Ordinary axis/grid construction eagerly normalizes all child/cell entries.
- Axis/grid edits above threshold use persistent sequences, avoiding full sequence copying.
- Wide axis/grid `.children`/`.rows` are lazy; inspection can force flattening.
- `View.styledText` maps spans and semantic-normalizes each style.
- Diff conversion walks every hunk line once.
- Source snapshots convert every annotation payload and decode style annotation payloads.
- Source stats normalize every u64 field.
- Source append/replace enters native FFI per mutation.
- State writes enqueue subscribed scopes once; no deep/proxy observation exists.
- Smooth delivery is represented as connector policy; no JS-side per-tick text reconstruction occurs in this API.
- ViewSlot animation acquires one retained reference per frame and releases all temporaries in `finally`.
- Host-visible work is driven by runtime/environment wake and frame barriers rather than public API polling.

### 6.3 Invalidation

- `State<T>` invalidates retained execution scopes.
- `ViewState` native wake bits cause host registration pending work.
- Content Source mutation calls `sourceWake`, marking the environment pending.
- Connector activation/deactivation returns native wake information and invokes the request wake callback if the native disposition requires a drain.
- Theme changes clear theme-dependent style reference caches in runtime (`runtime.ts:880-894`).
- Structural replacements use retained root boundaries and attachment binding revisions.

No public API exposes internal counters for append/tick/frame work. The source contains benchmark/test infrastructure elsewhere, but this assignment did not execute it.

### 6.4 Performance-sensitive contracts

The most consequential public-facing performance contracts are:

1. **Retained composition**
   - callers do not manually memoize Views;
   - unchanged component props can skip body execution through `defineView`;
   - state reads subscribe scopes automatically.

2. **Identity-first retained structural transport**
   - immutable View objects and semantic NodeIds permit retained reuse;
   - slot/pane/history use retained materialization rather than a second legacy transport path.

3. **Persistent sequences**
   - wide child/cell edits avoid O(previous-tree) array copying;
   - lazy flattening preserves the optimization until a consumer requests the public array-shaped view.

4. **Native smoothing**
   - smooth delivery is configured once on the Funnel/Connector;
   - JS does not drive every smoothing tick over the native boundary.

5. **Typed-array animation staging**
   - large frame lists avoid a second JS number-array copy.

---

## 7. Tests, benchmarks and observability

### 7.1 Relevant behavioral tests

The test tree was indexed through `packages/iyon-tui/tests/**/*.ts`. Selected contract evidence:

- `tui_values.test.ts`
  - fluent operations return new semantic values;
  - nested composition crosses native boundary once;
  - semantic diff rendering;
  - malformed payload validation;
  - cache-hit behavior;
  - Unicode/native rendering;
  - NodeId behavior across module re-evaluation.
- `tui_traits.test.ts`
  - direct internal adapter tests;
  - renderer/rewriter callbacks run as promises;
  - component callbacks do not receive borrowed native values.
- `tui_handles.test.ts`
  - handle lifecycle and ownership.
- `tui_harness.test.ts`
  - History, ViewSlot animation, text/input/render behavior.
- `tui_perf13_a.test.ts`
  - duplicate attachment rejection and retained structural behavior.
- `tui_perf13_b.test.ts`
  - ViewState, style/theme, state attachment, duplicate state attachment, retained slot behavior.
- `tui_perf13_h.test.ts`
  - content Port/Source route.
- `tui_h3_b_composition.test.ts`
  - public composition behavior.
- `tui_native_persistent_seq.test.ts`
  - wide axis/grid persistent sequence edits.
- `tui_state_envelope.test.ts`
  - native state envelope masks and wake behavior.
- `tui_smooth_delivery.test.ts`
  - smooth Funnel/Connector delivery.
- `tui_retained_scene_regressions.test.ts`
  - root replacement, History and slot replacement behavior.
- `tui_text_lanes.test.ts`
  - text and styled text semantics.
- `tui_native_input_validation.test.ts`
  - invalid public input handling.

No test was run for this report.

### 7.2 External-consumer fixture

`packages/tui-consumer-fixture/src/consumer.ts` is the clearest actual public consumer.

It imports only package-root APIs:

```ts
import { Insets, Scene, Style, View } from "@iyon/tui";
import { AppHarness } from "@iyon/tui/testing";
import type {
  History,
  ScrollPane,
  TextInput,
  TuiRuntime,
  View as ViewValue,
  ViewSlot,
} from "@iyon/tui";
```

Evidence: `consumer.ts:15-17`.

The fixture uses:

- `Style.new().foreground(...)` (`consumer.ts:26-27`);
- `View.vertical`, `View.text`, `View.content`, `ViewSlot.view`, `TextInput.view`, `ScrollPane.view`;
- `Insets.of`;
- fluent `style`, `fillWidth`, `fillHeight`, `padding`;
- `Scene`;
- `AppHarness.open`;
- `createTextInput`, `createViewSlot`, `createScrollPane`, `createHistory`;
- direct `tui.render`.

Evidence: `consumer.ts:34-100`.

The componentized fixture uses the public retained composition surface:

- `defineView`;
- `state`;
- `View.key`;
- `TuiRuntime.render(() => new Scene(...))`.

Evidence: `consumer.ts:103-169`.

This demonstrates that the intended consumer path requires no feature flag, compiler setup, internal runtime import, or manual View memoization, consistent with the public contract documented in `define-view.ts:1-31`.

### 7.3 Observability

Public observability includes:

- `TuiRuntime.onRuntimeError`;
- `ContentConnector.status`;
- Source `snapshot()` and `stats()`;
- `ViewSlot.revision()`;
- Tui size;
- testing-only `AppHarness` screen/history/style/cell inspection;
- testing-only deterministic clock and event injection.

Public runtime error categories include:

- invalid/disposed handle;
- validation;
- terminal;
- runtime;
- projection;
- stream;
- cancellation.

Evidence: `errors.ts:1-105`.

The error mapper recognizes native codes including source lifecycle, connector lifecycle, projection, host disposal, runtime poison/panic, cancellation, and terminal closure. Unknown errors default to `"runtime"`.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Public API wrappers are runtime owners, not passive facades

Several modules under `src/api` directly own native/resource lifecycle:

- `retained.ts` imports content FFI/control, runtime environment, resource registry, and handle release (`retained.ts:1-32`);
- `history.ts` imports native View ABI and native History;
- `view-slot.ts`/`scroll-pane.ts` import retained DAG, execution runtime, attachment bindings, and native host contracts;
- `retained-state.ts` imports state envelope packing and native state control;
- `framework-handle.ts` imports runtime handle registry and native resource lookup.

Therefore, removing or relocating a public class would require preserving its ownership and transaction semantics somewhere else; the useful responsibility is not limited to type declarations.

### 8.2 Semantic-node is cleanly separated from transport, but View is not

`semantic-node.ts` explicitly states that it has no structural transport/native knowledge (`semantic-node.ts:1-10`). This is a clean semantic model seam.

`view.ts`, however, imports:

- composition execution context;
- composition builders;
- semantic style normalization;
- attachment handle types;
- persistent sequences.

This is intentional integration: the public View authoring API is also the entry point for retained composition and transport-facing derivation hints.

### 8.3 Content-to-presentation coupling is explicit

Two public content values directly depend on View:

- `TextContent.render()` returns `View.text(...)` (`text-content.ts:20-22`);
- `DiffRenderer` returns `View.diff(...)` (`diff.ts:80-94`).

These are generic framework semantics, not product-specific policy, but they couple content values to the current semantic View authoring surface.

### 8.4 Public root versus module exports is intentionally asymmetric

The root exports `ContentPort`, `ContentConnector`, `TextStreamSource`, `TextBlockSource`, and `TextFunnel` as runtime values, but their constructors are private or factory-mediated.

By contrast:

- `History` is directly constructible in detached mode;
- `ViewState` is root-exported as a value but its constructor is private;
- `ViewSlot` and `ScrollPane` are root-exported as contracts only;
- `Output` is type-only and cannot be fabricated;
- adapter classes are module-local exports but not package-root exports.

This asymmetry encodes ownership and prevents callers from constructing resources without an environment/host.

### 8.5 Handle identity layers are deliberately distinct

There are at least three identity concepts:

1. JS-local framework `HandleId` (`framework-handle.ts:9-11`);
2. semantic View NodeId (`semantic-node.ts:161-189`, `view.ts:156-158`);
3. native component/source/history/resource identity held behind registry/native contracts.

The code comments explicitly distinguish component local handle identity from native ComponentId (`semantic-node.ts:248-251`) and semantic NodeId from native ABI IDs (`semantic-node.ts:161-162`).

### 8.6 State is split across composition and native planes

`State<T>` and `ViewState` have different owners, lifetimes, and effects:

- `State<T>` is a JS source whose subscribers are retained execution scopes.
- `ViewState` is a native host-owned override record attached to a semantic occurrence.

The public names are similar enough that a consumer or future maintainer could incorrectly treat them as one abstraction. The source itself keeps them in separate modules and paths.

### 8.7 Projection validation contradiction

As noted above, the `Projection` comment says spans must be contiguous, but `validateSpans` allows gaps. This is a concrete source-level contract inconsistency (`projection.ts:9-13`, `28-34`).

### 8.8 Public immutability is partly by convention, partly by normalization

Many public values are described or intended as immutable, but construction does not uniformly deep-copy/freeze all caller-owned input:

- `StyleSpec` stores the supplied `StyleSpecValue` directly (`style.ts:48-51`);
- `TextSpan` stores the supplied `TextSpanValue` directly (`text.ts:168-171`);
- `Annotations` stores the supplied tags/properties directly (`annotations.ts:23-27`);
- `TextContent.raw` stores the supplied `TextOrigin` object (`text-content.ts:15-19`);
- `Theme` stores `StyleSpec.value`, color objects, and selector values in private maps (`theme.ts:103-166`);
- `GridRowBuilder`, `GridBuilder`, and `ChildrenBuilder` expose mutable construction arrays (`view.ts:166-219`).

The semantic View path subsequently copies/freezes values during normalization (`semantic-style.ts:58-216`, `semantic-node.ts:376-456`), reducing downstream mutation risk. However, mutable references remain observable before normalization or through direct value properties. This is an API-contract observation, not a proposed change.

### 8.9 Text source naming

`appendUtf8` and `replaceUtf8` accept `string` and simply delegate to `append`/`replace` (`retained.ts:386-396`). The actual encoding occurs inside the native/FFI path. The public names communicate a UTF-8 wire semantic but do not expose a raw byte input route.

### 8.10 Generic boundary compliance

Search of `packages/iyon-tui/src/api/**` found no product-specific agent/application concepts such as assistant, conversation, provider, tool, prompt, or product status. Generic names such as `History`, `Output`, `Stream`, `route`, and `smooth` are used with caller-supplied semantics.

The only potentially product-looking identifier found in this scope was the global symbol namespace `"iyon:tui:private-view-node-counter"` (`view.ts:146`), which is an implementation namespace and does not encode application meaning.

---

## 9. Open questions and coverage gaps

1. **Projection gap semantics**
   - Is a source gap between Projection spans intentional, or should “contiguous” mean `sourceStart === expected`?
   - Current source permits gaps.

2. **Public immutability guarantees**
   - Are callers expected to treat `StyleSpecValue`, `TextSpanValue`, `TextOrigin`, annotation arrays, and theme inputs as immutable by convention?
   - Semantic normalization protects View transport, but direct value objects can retain caller references.

3. **Adapter visibility**
   - `AsyncComponentAdapter`, `ProjectorAdapter`, `RendererAdapter`, `TextRewriterAdapter`, and `TextVisitorAdapter` are exported from their source modules but not from the package root.
   - Is this an intentional package-root omission, or are they only package-internal test utilities?

4. **Unused public content/projection symbols**
   - `Projection`, `ProjectionBuilder`, `Smooth`, `Projector`, and several text selector/annotation types have limited or no obvious in-repository production consumers in this inspected scope.
   - Absence of local consumers is not evidence that external consumers do not use them.

5. **History public constructor**
   - `new History()` is intentionally detached and caller-owned, while other native-backed controls cannot be directly constructed.
   - The long-term consistency of this asymmetry is not determined here.

6. **Public `View` introspection**
   - The root does not expose semantic node inspection or NodeId helpers.
   - Internal transport/tests use `viewNodeId` and related helpers.
   - It is unknown whether future diagnostics require a supported public identity/introspection contract.

7. **Theme resolution semantics**
   - `Theme.color()` only evaluates unconditional variants, while complete theme lowering preserves conditional variants.
   - It is not established whether callers expect `Theme.color()` to resolve selector-dependent values or only serve as a convenience base lookup.

8. **Content lifecycle after Tui close**
   - Source ownership is environment-scoped, while Port/Connector ownership is host-scoped.
   - The exact externally documented behavior for environment teardown versus host close should be verified against broader runtime/content documentation.

9. **Test and benchmark coverage**
   - This report did not execute tests or benchmarks.
   - Full behavioral coverage, route counters, and benchmark integrity require the support/test and benchmark assignments.

10. **Native ABI parity**
    - This report maps the TypeScript contract and native call names visible at the TS boundary.
    - Complete wire representation, generated schema ownership, and Rust-side parity require the native/ABI assignments.

---

## 10. Evidence appendix

### 10.1 Primary inspected-file manifest

#### Package root

- `packages/iyon-tui/src/index.ts`
- `packages/iyon-tui/package.json`

#### API content

- `packages/iyon-tui/src/api/content/annotations.ts`
- `packages/iyon-tui/src/api/content/diff.ts`
- `packages/iyon-tui/src/api/content/projection.ts`
- `packages/iyon-tui/src/api/content/retained.ts`
- `packages/iyon-tui/src/api/content/text-content.ts`
- `packages/iyon-tui/src/api/content/text.ts`

#### API controls

- `packages/iyon-tui/src/api/controls/framework-handle.ts`
- `packages/iyon-tui/src/api/controls/history.ts`
- `packages/iyon-tui/src/api/controls/output.ts`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
- `packages/iyon-tui/src/api/controls/text-input.ts`
- `packages/iyon-tui/src/api/controls/view-slot.ts`

#### API errors

- `packages/iyon-tui/src/api/errors.ts`

#### API extension traits

- `packages/iyon-tui/src/api/extensions/traits/component.ts`
- `packages/iyon-tui/src/api/extensions/traits/projector.ts`
- `packages/iyon-tui/src/api/extensions/traits/renderer.ts`
- `packages/iyon-tui/src/api/extensions/traits/text-rewriter.ts`
- `packages/iyon-tui/src/api/extensions/traits/text-visitor.ts`

#### API presentation

- `packages/iyon-tui/src/api/presentation/semantic-style.ts`
- `packages/iyon-tui/src/api/presentation/style.ts`
- `packages/iyon-tui/src/api/presentation/theme-key.ts`
- `packages/iyon-tui/src/api/presentation/theme.ts`

#### API view

- `packages/iyon-tui/src/api/view/geometry.ts`
- `packages/iyon-tui/src/api/view/retained-state.ts`
- `packages/iyon-tui/src/api/view/scene.ts`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
- `packages/iyon-tui/src/api/view/view.ts`

### 10.2 Supporting seam files inspected

- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/iyon-tui/src/testing/index.ts`
- `packages/iyon-tui/src/composition/define-view.ts`
- `packages/iyon-tui/src/composition/tracked-state.ts`
- `packages/iyon-tui/src/transport/structural/retained-path.ts`
- `packages/tui-consumer-fixture/src/consumer.ts`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

### 10.3 Important exact symbols and line ranges

| Concern | Evidence |
|---|---|
| Root export allowlist | `packages/iyon-tui/src/index.ts:1-132` |
| Package export map | `packages/iyon-tui/package.json:7-10` |
| Framework handle identity/lifecycle | `src/api/controls/framework-handle.ts:9-60` |
| Public View constructors/modifiers | `src/api/view/view.ts:249-475` |
| View semantic identity and sidecars | `src/api/view/semantic-node.ts:161-336` |
| Semantic node vocabulary | `src/api/view/semantic-node.ts:183-279` |
| Semantic freezing | `src/api/view/semantic-node.ts:376-456` |
| Wide sequence sidecars | `src/api/view/view.ts:825-933` |
| ViewState properties and mutations | `src/api/view/retained-state.ts:39-214` |
| Scene contract | `src/api/view/scene.ts:4-28` |
| History lifecycle and operations | `src/api/controls/history.ts:28-132` |
| ViewSlot public contract/lifecycle | `src/api/controls/view-slot.ts:50-145`, `156-402` |
| ScrollPane public contract/lifecycle | `src/api/controls/scroll-pane.ts:24-71`, `119-246` |
| TextInput/output contract | `src/api/controls/text-input.ts:10-95` |
| Source/Funnel/Connector contracts | `src/api/content/retained.ts:37-164`, `336-810` |
| Source snapshot/stat normalization | `src/api/content/retained.ts:219-319` |
| Style contracts | `src/api/presentation/style.ts:4-220` |
| Theme contracts/lowering | `src/api/presentation/theme.ts:82-242` |
| Semantic style normalization | `src/api/presentation/semantic-style.ts:39-216` |
| Text selectors/spans | `src/api/content/text.ts:3-183` |
| Diff validation/rendering | `src/api/content/diff.ts:6-102` |
| Projection validation | `src/api/content/projection.ts:3-34` |
| Error categories/native mapping | `src/api/errors.ts:1-106` |
| JS tracked State contract | `src/composition/tracked-state.ts:5-136` |
| Public defineView contract | `src/composition/define-view.ts:1-65` |
| Tui factories/root render | `src/runtime/runtime.ts:59-90`, `319-405`, `451-720` |
| Tui disposal/lifecycle | `src/runtime/runtime.ts:730-894` |
| External public consumer | `packages/tui-consumer-fixture/src/consumer.ts:15-169` |

### 10.4 Files indexed but not comprehensively read

- Most files under `packages/iyon-tui/tests/**/*.ts`
- Most files under `packages/iyon-tui/bench/**/*.ts`
- Remaining repository source outside the supporting seams above

Selected tests and consumers were searched for public API references and behavioral contract names, but they were not counted as primary source ownership for this assignment.

### 10.5 Static inspection commands/searches used

- File inventory of `packages/iyon-tui/src/api/**`
- File inventory of `packages/iyon-tui/tests/**`
- File inventory of atlas evidence files
- Content search for:
  - public exports;
  - API imports;
  - `View.*`;
  - Source/Funnel/Connector symbols;
  - style/theme/state/handle symbols;
  - prohibited product/application terms;
  - consumer and test call sites.
- Full line-oriented inspection of all 27 API files and `src/index.ts`, with supporting seam inspection as listed above.

### 10.6 Final evidence status

- Source revision was identified from the assignment/atlas documents.
- All primary API files and `src/index.ts` were inspected.
- No source or configuration changes were made.
- No tests, benchmarks, builds, typechecks, dependency installs, or running services were invoked.
- Findings distinguish current source facts from static inferences and unresolved questions.