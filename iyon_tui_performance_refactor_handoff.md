# Iyon TUI Performance Architecture Refactor — Implementation Agent Handoff

> **Audience:** implementation agent executing the refactor mechanically.
>
> **Purpose:** performance architecture refactor of `iyon-tui` and the TUI portion of `iyon-native` / `packages/iyon-runtime`. This is **not** feature work. Do not improvise product semantics, new framework concepts, or alternate transports unless a tranche explicitly asks for an experiment.
>
> **Primary rule:** preserve identity and stability across every layer. Only the genuinely changed path/suffix should lose identity and require work.

---

## 0. Read this before touching code

This handoff is intentionally prescriptive. The architecture has already been researched against the current Iyon repository, the two design/audit documents supplied with this task, Bun's current Node-API implementation, Flutter's retained UI architecture, React/React Native's retained/structurally-shared trees, and OpenTUI's native-core model.

### Starting-state rule

At research time:

- `alexykn/iyon-tui` `main` points at `223bf4840a03c90d713f12419bba25a501ae217f`.
- That latest commit is documentation-only. The code state inspected by the second supplied audit was `88c66f9073c425c049a1ac31c7fbb4b0e771a4d1`, and the code paths re-inspected for this handoff still match the architectural findings from that audit.

**Before the first edit, run:**

```bash
git status --short
git rev-parse HEAD
git branch --show-current
```

Record the exact starting SHA in your implementation notes and benchmark output.

**Do not:**

- reset the branch to any SHA named in this document;
- rewrite unrelated work;
- discard recovery work;
- force-push;
- change app behavior to make a benchmark pass;
- skip or delete tests because they are slow.

If your local source differs from the paths or shapes described below, inspect the diff first. Adapt the *mechanics*, but do not reinterpret the architectural invariants.

---

# 1. The architecture you are implementing

The refactor is based on one idea:

> **Identity and stability must survive every layer of the rendering/update pipeline.**

Today, identity is repeatedly destroyed:

```text
TypeScript immutable-ish View DAG
    ↓
materializeView() recursively encodes the whole tree
    ↓
serde_json::Value crosses N-API
    ↓
lower_view() recursively creates a fresh Rust View tree
    ↓
View clones duplicate recursively-owned descendants
    ↓
scene resolution recursively creates another semantic View tree
    ↓
measurement creates a fresh measured tree
    ↓
prepare creates another fresh tree
    ↓
History rebuilds projection plans and recomputes resident geometry
    ↓
stream row indexes are recreated from semantic suffix views
    ↓
paint recursively allocates/composites surfaces
```

The target is:

```text
TS immutable private View DAG with stable private NodeId
    ↓ direct retained decoder
Arc-backed native View with stable ViewId
    ↓ no semantic-tree reconstruction
resolution overlay + cached component snapshots
    ↓
bounded retained measurement/prepare cache
    ↓
retained History per-unit geometry index
    ↓
retained stream semantic prefix + suffix-only row reflow
    ↓
ordinary placement + paint
```

An update that changes one leaf should therefore look like:

```text
new TS root identity
├── old huge subtree A identity ─────────────┐
└── new path to changed leaf                 │
                                             │
N-API decoder:                               │
  root miss                                  │
  changed-path misses                        │
  subtree A node-id hit ── STOP traversal ◄──┘

Rust:
  new root/path ViewIds
  subtree A keeps same Arc/ViewIds

resolver:
  no ordinary semantic reconstruction

layout:
  cache misses only on changed path / affected geometry
  cache hits for subtree A

History/stream:
  stable prefix retained
  changed suffix only recomputed
```

That is the success criterion. Do not replace this with a different reconciliation architecture.

---

# 2. Non-negotiable architectural contract

These rules are acceptance requirements, not suggestions.

1. **`iyon-tui` remains a generic TUI framework.** This includes feature-gated host code. It must not own assistant, reasoning, thinking, tool, provider, agent-turn, approval, or Iyon-product semantics.
2. **The public Rust `View` API keeps value semantics.** Builder-style operations still return a new semantic value. No public mutable node handles.
3. **The TypeScript `View` API mirrors those semantics.** Any valid tree may be replaced with any other valid tree. Every child, style, text, track, placement, border, clamp, component placement, etc. may change between revisions.
4. **Retention is private/internal.** Mutable retained objects are components, component snapshots, History, streams, text buffers, scroll panes, view slots, layout caches, and renderer state—not public semantic `View` nodes.
5. **Steady-state `View::clone()` is O(1).** Cheap `Arc` clone operations are expected and should not be “optimized away.”
6. **A semantic mutation always creates a new `ViewId`.** This is true even when the underlying `Arc` is uniquely owned. Allocation uniqueness is not semantic identity.
7. **`ViewId` never changes semantic equality.** It is an identity/cache key, not part of `PartialEq` semantics.
8. **No JSON or `serde_json::Value` remains in the production TUI `View` hot bridge at completion.** Low-frequency DTOs may use typed N-API objects. Genuinely arbitrary JSON elsewhere may remain JSON.
9. **Do not replace JSON with another generic serializer.** No MessagePack/CBOR merely to call the bridge optimized.
10. **Do not make every `View` builder method an N-API call.** Tree construction stays in JS/Rust semantic layers; the boundary is crossed for state mutations/commit-like operations.
11. **Do not move an O(total-tree) walk from Rust to JS and call it a win.** A packed JS encoder is production-worthy only if end-to-end benchmarks beat the direct retained decoder.
12. **Stable History prefixes are not remeasured because a tail changed.**
13. **Stable stream semantic prefixes and visual row anchors are not rebuilt because a suffix was appended.**
14. **Semantic stability and visual-row stability are different concepts.** A new character can alter wrapping before the literal append offset.
15. **Every retained cache is bounded or naturally lifetime-bounded.** No forever-growing `HashMap<ViewId, ...>`.
16. **A normal N-API mutation marks retained native state dirty and returns.** It does not force terminal rendering synchronously unless the API is explicitly a synchronous test/finalization primitive.
17. **Synchronous native operations stay synchronous at the TS API after PERF-8.** Artificial `Promise.resolve(...)` wrappers are removed.
18. **Performance claims require counters/benchmarks.** “This should be faster” is not an acceptance argument.
19. **Do not add paint caching until profiling after PERF-9 proves paint is at least 15% of p95 dirty-frame CPU.**
20. **Do not begin `iyon-core` / non-TUI API bridge cleanup until the TUI performance gate has passed.**

---

# 3. Upstream design audit: what to borrow and what not to borrow

This section exists so you understand *why* the target architecture is shaped this way. Do not copy these frameworks wholesale.

## 3.1 Flutter — borrow the separation and invalidation boundaries

Inspected current Flutter source at commit:

```text
bb8568060d4f8882b4c069f764b74b2ff92993b0
```

Relevant sources inspected:

```text
packages/flutter/lib/src/widgets/framework.dart
packages/flutter/lib/src/rendering/object.dart
```

Flutter's current `Widget` documentation explicitly defines a widget as an **immutable description** that is inflated into retained `Element` objects managing the render tree. Its `Widget.canUpdate()` determines whether an existing element can be retained. The performance guidance recommends pushing changing work toward leaves, caching common subtrees, and allowing unchanged/const widgets to short-circuit rebuild work.

At the render layer, Flutter tracks dirty layout state and relayout boundaries. A child that is a relayout boundary can be dirtied without propagating layout invalidation unnecessarily to its parent.

### Iyon lesson

Use the same **separation of concerns**:

```text
immutable public semantic description
                ↓
retained internal state + dependency-aware invalidation
```

Do **not** create a public Element tree for Iyon. `View` already has the right value-builder API. Retention belongs underneath it.

## 3.2 React / React Native — borrow retained identity and structural sharing

Inspected current React source at commit:

```text
eb8feb71096eec5c885b2a4c7d8d030d3622f265
```

Relevant source:

```text
packages/react-reconciler/src/ReactFiber.js
```

`createWorkInProgress()` reuses an existing alternate Fiber when available and copies/retains current child, memoized props/state, lanes and other retained information rather than reconstructing an unrelated tree from scratch.

Inspected current React Native Fabric source at commit:

```text
4bf55754905dfdcd6460867dca9ad45bfb9fae45
```

Relevant source:

```text
packages/react-native/ReactCommon/react/renderer/core/ShadowNode.cpp
```

Fabric `ShadowNode` children are held in shared storage. Cloning a node keeps the existing shared children unless new children are supplied. `cloneChildrenIfShared()` copies the child vector only when a mutation actually needs private child storage.

### Iyon lesson

Use stable node identity as a **cutoff**:

```text
same identity + still alive
    → reuse retained native semantic subtree
    → stop walking it
```

Do not build a React-style general reconciler at the N-API boundary. Iyon's existing TS `View` representation already structurally shares objects. Preserve that identity instead of rediscovering equivalence.

## 3.3 Bun Node-API — use the actual runtime primitives conservatively

Inspected current Bun `main` at commit:

```text
079cb0a6a8f02229eb16d03b297b3a8984177c29
```

Relevant actual implementation sources:

```text
src/jsc/bindings/napi.cpp
src/runtime/napi/napi_body.rs
```

Also reviewed recent N-API compatibility/lifetime work and the corresponding tests under:

```text
test/napi/napi.test.ts
test/napi/napi-app/standalone_tests.cpp
```

Important implementation facts:

- Bun implements Node-API wrapping/references with weak/strong lifetime behavior rather than treating all wrapped JS objects as permanently strong.
- `napi_wrap`/reference lifetime is intentionally weak at refcount zero; recent Bun tests specifically exercise collection/lifetime behavior.
- Bun implements typed-array information access, including returning the backing data address, length, type and byte offset.
- Bun implements environment instance data/finalization primitives.
- Bun implements external ArrayBuffers, but this does **not** mean they should be the default transport.
- Bun's N-API implementation has had active compatibility/termination/lifetime hardening in August 2026. Therefore every less-common primitive used by Iyon gets an explicit Bun conformance test rather than assuming Node behavior automatically.

### Iyon lesson

Candidate A should be a synchronous direct decoder that reads private immutable JS data and constructs/fetches final native `View` values. Candidate B may use a borrowed `Uint32Array`, but borrowed JS backing memory is consumed entirely within the synchronous call. Never store the raw pointer beyond the call.

Do not use external ArrayBuffers for the primary JS→Rust path. A JS-owned typed array already provides the useful property: native code can synchronously inspect its backing bytes without asking JS to copy them again at the boundary.

## 3.4 OpenTUI — native code alone is not the architecture

Inspected current OpenTUI at commit:

```text
4067477dd89b554641753dcfbc5e506f61bdd52f
```

Relevant source/documentation:

```text
packages/core/README.md
packages/core/src/
```

OpenTUI is a native Zig core with TypeScript bindings and a C ABI, and it treats benchmarkability as a first-class concern. Bun's own July 2026 FFI work also used OpenTUI benchmark suites to measure call-dense layout/render paths.

### Iyon lesson

A native renderer does **not** guarantee bounded update cost. If each text append still scans/re-wraps/re-indexes all accumulated content, long sessions still become slower. Optimize the *algorithmic amount of work per update*, then optimize the boundary.

---

# 4. Current Iyon source audit — exact files and current problems

Every existing file named in the mandatory tranches below was inspected for this handoff.

## 4.1 Rust semantic `View`

### `crates/iyon-tui/src/presentation/ir.rs`

Current `View` is a recursively owned value with fields including component/layout/style data, `component_scope`, and a recursively-owned `ViewKind`.

Problems:

- derived/ordinary clone operations recursively clone child `Vec`s / boxed values / text payload structures;
- `component_scope` is not semantic app input but lives on semantic `View`;
- `contains_component_identity()` recursively scans descendants;
- recursive ownership prevents O(1) semantic subtree retention.

### `crates/iyon-tui/src/presentation/api/view.rs`

Public builder API already consumes/returns values in a way compatible with persistent internals. Preserve it.

## 4.2 Scene resolution and component registry

### `crates/iyon-tui/src/scene/resolve.rs`

Current resolver:

- recursively reconstructs ordinary `View` nodes;
- stamps derived `component_scope` into clones;
- resolves components;
- performs missing/duplicate/cycle validation;
- builds mount/capability information.

This reconstruction must disappear.

### `crates/iyon-tui/src/component/registry.rs`

Current `ComponentEntry` has component + revision but does not cache its rendered snapshot. `resolution()` calls `component.view()` and `component.capabilities()` again even when revision is unchanged.

## 4.3 Layout

### `crates/iyon-tui/src/presentation/layout/measure.rs`

Current `MeasuredNode<'a>` borrows semantic `View`, making measurement output throwaway. Text flow measurement is rerun on fresh layout.

### `crates/iyon-tui/src/presentation/layout/prepare.rs`

Prepare is a distinct height-sensitive stage and creates fresh state each time.

### `crates/iyon-tui/src/presentation/layout/place.rs`

Placement produces final positioned/clip geometry. Keep this uncached initially.

### `crates/iyon-tui/src/presentation/layout/engine.rs`

Current layout path drives fresh measure → prepare → placement.

### `crates/iyon-tui/src/presentation/layout/tree.rs`

The layout tree already carries derived style/scope information. This is the correct layer for `component_scope` after it leaves semantic `View`.

### `crates/iyon-tui/src/scene/host.rs`

`SceneHost` owns retained scene execution and convergence but currently has no retained `LayoutCache`. It is the correct cache owner.

## 4.4 History

### `crates/iyon-tui/src/history/unit.rs`

History already has the correct semantic taxonomy:

```text
Static(View)
Live(View)
Stream(...)
```

Do not replace this with assistant/app concepts.

### `crates/iyon-tui/src/history/model.rs`

History owns the units/layout/native state but no retained per-unit presentation geometry index.

### `crates/iyon-tui/src/history/projection/mod.rs`

Current projection allocates/builds fresh collections such as unit plans, selected units/items, and flow items; it walks resident flow to determine total geometry and measures units as needed during those new plans.

This is the main PERF-4 target.

### `crates/iyon-tui/src/history/stream.rs`

`TypedHistoryStream` retains semantic model state, but presentation row indexing is recreated through `prepare_from()` / row-index construction rather than held across updates.

## 4.5 Generic stream system

### `crates/iyon-tui/src/stream/model.rs`

This is a good foundation. It already models:

- source base/end;
- revisions;
- `stable_through`;
- resident prefix;
- source compaction validation.

Preserve those concepts.

### `crates/iyon-tui/src/stream/resident.rs`

Resident-prefix storage/release machinery is already present. Build on it.

### `crates/iyon-tui/src/stream/node.rs`

Stream nodes carry source ranges/provenance and support range-sensitive operations.

### `crates/iyon-tui/src/stream/append.rs`

Append stability checkpoints already account for grapheme/display boundaries and newline behavior. Do not replace these semantics with byte-count shortcuts.

### `crates/iyon-tui/src/stream/text.rs`

Critical problem: generic `TextStream` keeps one growing `String`; snapshot construction clones it into one all-encompassing `exact_text` semantic node.

`StreamModel` only promotes whole semantic nodes ending at/before `stable_through`. Therefore an open text node that crosses the stable frontier blocks incremental stable-prefix capture.

### `crates/iyon-tui/src/stream/viewport/index.rs`

Current `build_index_from()` recompiles `model.semantic_view_from(start)` and builds a fresh anchor vector. This must become suffix-incremental.

### `crates/iyon-tui/src/stream/compile/mod.rs`
### `crates/iyon-tui/src/stream/compile/text.rs`
### `crates/iyon-tui/src/stream/compile/rows.rs`

Current wrapping/checkpoint behavior is the correctness baseline. Reuse its logic when rebuilding only the damaged visual suffix.

## 4.6 Feature-gated host stream genericity leak

### `crates/iyon-tui/src/application/host.rs`

Current generic TUI crate still contains application semantics such as:

- `Thinking` stream segment kind;
- `HostAssistantPipeline`;
- `ThinkingRewriter`;
- `("app", "thinking")` semantic tags;
- assistant-specific composition of smoothing + Markdown + thinking segmentation.

It also stores per-character pacing atoms with a separate allocated string per character and rebuilds semantic projection from historic atoms.

These concepts must not survive inside `iyon-tui`.

## 4.7 Paint

### `crates/iyon-tui/src/presentation/paint/view.rs`

Current painter recursively creates `Surface`s per node and composites child surfaces. Styling depends on:

- inherited physical style;
- node style states/facts;
- component scope / focus context;
- theme resolution;
- geometry and clip/viewport behavior.

This proves that a future paint cache cannot be keyed by `ViewId` alone. It also means we do **not** add a paint cache before profiling proves it is necessary.

## 4.8 Rust N-API TUI bridge

### `crates/iyon-native/src/tui.rs`

Current hot bridge still uses `serde_json::Value` / JSON-like recursive lowering for semantic View trees. `lower_view()` dispatches recursive string-tagged structures and constructs a new Rust `View`.

`NativeTuiView` is then used as an intermediate, and operations such as render/history/slot/pane insertion can clone the resulting native `View` again.

Stateful native classes already exist. Keep them native; do not serialize their state.

## 4.9 TypeScript TUI bridge

### `packages/iyon-runtime/src/tui/values/view.ts`

This is important: the TS `View` implementation already keeps private nodes in a `WeakMap` and naturally preserves child object references when wrapping/decorating/composing. It is already an identity-rich private DAG.

Do not invent a second semantic representation.

### `packages/iyon-runtime/src/tui/ir.ts`

Current private IR uses string discriminants and has no stable numeric node identity.

### `packages/iyon-runtime/src/tui/materialize.ts`

Current `materializeView()` recursively encodes the whole tree and then crosses N-API, destroying the existing TS structural sharing at the boundary.

### `packages/iyon-runtime/src/tui/handles.ts`

Current `HandleBase.call()` wraps synchronous operations in `Promise.resolve(...)`.

### `packages/iyon-runtime/src/tui/types.ts`

Current `TuiOperation<T>` is a Promise type, so PERF-8 is a real public TypeScript surface migration.

### `packages/iyon-runtime/src/tui/history.ts`
### `packages/iyon-runtime/src/tui/runtime.ts`
### `packages/iyon-runtime/src/tui/scroll-pane.ts`
### `packages/iyon-runtime/src/tui/text-input.ts`
### `packages/iyon-runtime/src/tui/component.ts`
### `packages/iyon-runtime/src/tui/stream.ts`

These are current consumers/wrappers around materialization and/or blanket async handle calls. `stream.ts` also exposes the app-semantic `"thinking"` segment kind and must become generic.

## 4.10 Deferred non-TUI native boundary

### `crates/iyon-native/src/core.rs`

Current session queue stores `mpsc::Sender<Value>` / `Receiver<Value>`, and `CoreEvent` is converted through `events::core_event()` before enqueueing.

### `crates/iyon-native/src/events.rs`

Current `CoreEvent` → `serde_json::Value` conversion builds string-tagged envelopes. Some nested fields are genuinely arbitrary JSON; the outer protocol is not.

### `crates/iyon-native/src/model_turn.rs`

Current `pushMany()` literally loops:

```text
for each Value
    await self.push(value)
```

so it is not a native batch. The native non-TUI batching cleanup is no longer
planned: `iyon-core` and `iyon-api` are moving to TypeScript, and provider
JSON/network work remains in the JS/TS layer.

---

# 5. Required execution order

Do not reorder this unless a concrete compile dependency forces a tiny local adjustment.

```text
PERF-0   instrumentation + benchmark oracle
PERF-1   persistent Arc-backed View + semantic identity
PERF-2   resolution overlay + cached component snapshots
PERF-3   bounded retained measurement/prepare cache
PERF-4A  retained History index for Static + Live units
PERF-5   generic TextStream fix + suffix row cache + host genericity cleanup
PERF-4B  integrate stream height deltas into retained History geometry
PERF-6   direct retained N-API View decoder
PERF-7   direct decoder vs packed TypedArray shootout
PERF-8   remove artificial Promise API for synchronous TUI operations
PERF-9   expose generic retained TextStream; add TextBuffer only if justified
PERF-10  conditional paint cache only if profiling gate is crossed
~~PERF-11  non-TUI iyon-native typed/batched boundary cleanup~~ (not needed; iyon-core and iyon-api are moving to TypeScript)
~~PERF-12  markdown pipeline spike detection and raw-text bypass~~ (deferred to `deferred.md`)
```

### Why PERF-4 is split

Do not teach History a temporary rule that “stream revision changed, therefore the whole stream unit is dirty.” First prove Static/Live unit incrementality in PERF-4A. Then make stream layout truly suffix-incremental in PERF-5. Only then connect stream height deltas into History in PERF-4B.

---

# 6. PERF-0 — instrumentation and the performance oracle

## Goal

Create the measurement system that proves later tranches are actually incremental. **No architecture changes in this tranche.**

## Existing files to touch

```text
crates/iyon-tui/Cargo.toml
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/scene/resolve.rs
crates/iyon-tui/src/component/registry.rs
crates/iyon-tui/src/presentation/layout/measure.rs
crates/iyon-tui/src/presentation/layout/prepare.rs
crates/iyon-tui/src/presentation/layout/place.rs
crates/iyon-tui/src/presentation/paint/view.rs
crates/iyon-tui/src/history/projection/mod.rs
crates/iyon-tui/src/history/stream.rs
crates/iyon-tui/src/stream/model.rs
crates/iyon-tui/src/stream/viewport/index.rs
crates/iyon-native/src/tui.rs
packages/iyon-runtime/... benchmark-only entry points as needed
```

Prefer adding one small internal perf-counter module rather than scattering global statics with duplicated reset/snapshot logic.

## Step-by-step implementation

### Step 0.1 — add an opt-in feature

In `crates/iyon-tui/Cargo.toml` add:

```toml
[features]
test-util = []
native-host = []
perf-counters = []
```

Counters must compile away from ordinary release builds.

### Step 0.2 — create one counter registry

Create an internal module, for example:

```text
crates/iyon-tui/src/perf.rs
```

Use atomics only when the instrumentation can be touched from multiple threads; otherwise keep implementation simple. Expose internal/test-only operations conceptually like:

```rust
reset();
snapshot() -> PerfSnapshot;
inc(Counter);
add(Counter, u64);
```

Do not expose this as ordinary public framework API.

### Step 0.3 — implement these required counters

```text
view_nodes_constructed_rust
view_nodes_deep_copied
view_clone_calls

napi_view_nodes_seen
napi_view_cache_hits
napi_view_cache_misses
napi_view_string_bytes_copied

resolver_nodes_visited
component_view_calls
component_capability_calls

measure_node_calls
text_flow_measure_calls
prepare_node_calls
layout_nodes_emitted
paint_nodes_visited
paint_cells_allocated
surface_cells_composited

history_units_examined
history_units_measured
history_cached_height_hits

stream_source_nodes_examined
stream_rows_reindexed
stream_stable_rows_reused
stream_semantic_restart_offset
stream_visual_restart_offset
```

The last counter is added here because semantic and visual restart points must be distinguished later.

### Step 0.4 — instrument only real work

Examples:

- `measure_node_calls`: increment at actual measurement entry, not cache lookup wrapper.
- `text_flow_measure_calls`: increment when text wrapping/flow is recomputed.
- `component_view_calls`: immediately around the actual `component.view()` call.
- `history_units_measured`: only when height is actually measured/recomputed.
- `stream_stable_rows_reused`: number of retained row anchors carried forward without recompilation.

Do not make counters lie by counting logical operations rather than work.

### Step 0.5 — create generic benchmark trees

Build deterministic constructors for:

```text
small_view            ~20 nodes
medium_view           ~200 nodes
large_view            ~2,000 nodes
huge_view             ~10,000 nodes

text_heavy
column_heavy
row_heavy
grid_heavy
styled_span_heavy
component_heavy
history_static_1000
history_live_tail
stream_1KiB
stream_10KiB
stream_50KiB
stream_100KiB
stream_500KiB
```

Do not use assistant messages/tool calls as fixtures. These are framework benchmarks.

### Step 0.6 — benchmark four View update patterns separately

```text
COLD
  completely fresh tree

IDENTICAL_IDENTITY
  exact same View object reused

SHARED_PATH
  one leaf changes while a large child/subtree object is reused

REBUILT_EQUIVALENT
  recreate equal semantics with new JS objects/identities
```

These patterns answer different questions. Never merge their numbers.

### Step 0.7 — canonical long-stream benchmark

Use fixed 256-byte chunks.

Graph/record:

```text
x = accumulated source bytes
y = time to process the NEXT 256-byte chunk
```

The target after PERF-5 is an approximately flat curve after warmup.

### Step 0.8 — JS/N-API benchmarking

Use Bun's high-resolution timing (`Bun.nanoseconds()`) and a benchmark harness such as `mitata` for repeated samples.

Measure **total operation latency**, not only Rust decoder time.

### Step 0.9 — emit machine-readable JSONL

Each record must contain at least:

```json
{
  "benchmark": "...",
  "implementation": "baseline",
  "node_count": 10000,
  "source_bytes": 0,
  "iterations": 100,
  "median_ns": 0,
  "p95_ns": 0,
  "p99_ns": 0,
  "counters": {},
  "git_sha": "..."
}
```

Do not commit generated result files.

## PERF-0 acceptance

- behavior unchanged;
- baseline captured for all required View/update/stream/History cases;
- counters are disabled from ordinary release builds;
- benchmark JSONL includes current git SHA;
- no production architecture changed.

## Suggested commit

```text
perf: add TUI performance counters and benchmark oracle
```

---

# 7. PERF-1 — make native `View` persistent and identity-safe

## Goal

Make semantic `View` cheap to clone and structurally share descendants.

## Exact files

Primary:

```text
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/api/view.rs
```

Tests likely in existing semantic/public test modules plus new internal unit tests.

## Required target shape

Conceptually:

```rust
#[derive(Clone)]
pub struct View {
    inner: Arc<ViewNode>,
}

struct ViewNode {
    id: ViewId,
    component: Option<ComponentId>,
    width: WidthRule,
    height: HeightRule,
    decoration: SharedDecoration,
    style_states: SharedStyleStates,
    style_facts: SharedStyleFacts,
    flags: ViewFlags,
    kind: ViewKind,
}
```

Do not over-interpret the names `SharedDecoration` etc. Share variable/expensive values when it avoids deep copies; do not put every tiny scalar behind a separate `Arc` without measurement.

### Variable recursive payloads must be shallow-cloneable

Target patterns:

```rust
enum ViewKind {
    Text(Arc<TextView>),
    Column(Arc<ColumnView>),
    Row(Arc<RowView>),
    Grid(Arc<GridView>),
    Hanging(Arc<HangingView>),
    Container(View),
    Spacer { rows: u16 },
    ClampRows(Arc<ClampRowsView>),
    RowViewport(Arc<RowViewportView>),
    ComponentSlot(ComponentSlotNode),
}

struct ColumnView {
    children: Arc<[ColumnChild]>,
    gap: u16,
}

struct RowView {
    children: Arc<[RowChild]>,
    gap: u16,
    vertical_align: VerticalAlign,
}

struct GridView {
    columns: Arc<[TrackSize]>,
    rows: Arc<[TrackSize]>,
    cells: Arc<[GridCellView]>,
    column_gap: u16,
    row_gap: u16,
}
```

Text spans likewise become a shared slice/payload rather than a recursively cloned vector.

## The most important identity rule

A `ViewId` represents immutable semantic content.

Therefore:

```text
same ViewId for entire lifetime
    ⇒ same semantics
```

and:

```text
any semantic mutation
    ⇒ new ViewId
```

This is true even if `Arc::strong_count == 1`.

### Do not use naive `Arc::make_mut()` identity semantics

This is wrong:

```rust
let node = Arc::make_mut(&mut self.inner);
node.padding = ...;
// id accidentally unchanged when allocation happened to be unique
```

The semantic ID would lie.

### Recommended mutation helper

Implement one internal mutation path that always creates one new root node while reusing child/payload Arcs:

```rust
impl View {
    fn map_node(self, f: impl FnOnce(&mut ViewNode)) -> Self {
        let mut next = self.inner.shallow_clone_semantics_with_new_id();
        f(&mut next);
        next.flags = next.compute_flags_from_local_payloads();
        Self { inner: Arc::new(next) }
    }
}
```

The exact function names may differ. The behavior must not.

This costs O(1) for a root decoration and O(path length) when a semantic child path is rebuilt by public construction. It must not copy descendant arrays/text payloads.

## Step-by-step implementation

### Step 1.1 — define `ViewId`

Use a process-local monotonic integer:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ViewId(u64);
```

Generate with an atomic monotonic counter. Handle overflow as an impossible/fatal invariant rather than silently reusing IDs.

### Step 1.2 — define `ViewFlags`

At minimum:

```text
CONTAINS_COMPONENT_SLOT
```

The flag is derived from local kind + child flags at construction/mutation time.

### Step 1.3 — move recursive payloads behind shared storage

Convert children/spans/grid cell collections first. Compile frequently. Do not simultaneously rewrite layout logic.

### Step 1.4 — wrap node in `Arc`

Change `View` to one `Arc<ViewNode>`.

### Step 1.5 — rewrite accessors internally

Existing crate code that currently accesses `view.kind`, `view.width`, etc. should use internal getters or `Deref` only if doing so does not accidentally expose `ViewNode` publicly.

Prefer explicit internal accessors where privacy is useful.

### Step 1.6 — implement O(1) `Clone`

`View::clone()` clones the outer `Arc` only and keeps the same `ViewId`.

Increment `view_clone_calls`; `view_nodes_deep_copied` must remain zero for normal clone.

### Step 1.7 — implement semantic mutation helper

All builder methods that change semantics route through the helper that allocates a new node/new ID and shallowly reuses payloads.

### Step 1.8 — implement identity helpers

Private/internal:

```rust
View::id() -> ViewId
View::ptr_eq(&self, other: &View) -> bool
View::flags() -> ViewFlags
```

### Step 1.9 — fix `contains_component_identity()`

Make it a flag lookup rather than a recursive scan.

### Step 1.10 — semantic equality

Implement `PartialEq` so:

1. `Arc::ptr_eq` or same ID may return `true` immediately;
2. different IDs still perform ordinary semantic equality;
3. ID itself is **not** a semantic field.

This matters because `RunningApp::host_set_body` currently compares old/new semantic Views before dirtying.

### Step 1.11 — add opaque weak bridge support now, but do not wire N-API yet

Do **not** expose `ViewNode` to `iyon-native`.

Provide an opaque type in `iyon-tui`, gated/internal as appropriate:

```rust
pub struct WeakView(Weak<ViewNode>);

impl View {
    pub(crate or bridge-gated) fn downgrade(&self) -> WeakView;
}

impl WeakView {
    pub fn upgrade(&self) -> Option<View>;
}
```

The native bridge must cache `WeakView`, not `Weak<ViewNode>`.

## Mandatory tests

### Clone identity

```rust
let a = View::text("x").into_view();
let b = a.clone();
assert!(View::ptr_eq(&a, &b));
assert_eq!(a.id(), b.id());
```

### Mutation changes identity when shared

```rust
let a = View::text("x").into_view();
let b = a.clone();
let c = b.padding(1);
assert_ne!(a.id(), c.id());
assert!(!View::ptr_eq(&a, &c));
```

### Mutation changes identity when unique

This catches the dangerous COW bug:

```rust
let a = View::text("x").into_view();
let old = a.id();
let b = a.padding(1); // a was uniquely owned
assert_ne!(old, b.id());
```

### Equality ignores identity

Build semantically equal trees independently. IDs differ; `PartialEq` remains true.

### Structural sharing

Build a root with a large child. Decorate/change only the root. Assert child pointer/identity is unchanged.

### 10k clone benchmark

Compare clone of ~100-node vs ~10k-node tree. Time must be effectively constant order, not proportional to subtree size.

## PERF-1 stop condition

**Do not proceed** if `View::clone()` still copies child arrays/text spans or if a unique-owner mutation can retain the old ID.

## Suggested commits

```text
refactor(tui): make View Arc-backed and structurally shared
refactor(tui): add semantic View identity and flags
```

---

# 8. PERF-2 — remove derived component scope from semantic `View`

## Goal

Stop resolution from destroying the sharing PERF-1 just created.

## Exact files

```text
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/scene/resolve.rs
crates/iyon-tui/src/component/registry.rs
crates/iyon-tui/src/presentation/layout/tree.rs
crates/iyon-tui/src/presentation/layout/measure.rs
crates/iyon-tui/src/presentation/layout/prepare.rs
crates/iyon-tui/src/presentation/layout/place.rs
crates/iyon-tui/src/presentation/paint/view.rs
crates/iyon-tui/src/scene/host.rs
```

The layout/paint files are touched only as needed to thread derived scope context after removing it from semantic `View`.

## Why this is mandatory immediately after PERF-1

Today `resolve_view()` recursively reconstructs ordinary semantic nodes largely to stamp `component_scope` into them. If you stop after Arc-backing `View`, resolution immediately manufactures a new tree and discards most sharing benefits.

## Target separation

```text
semantic View
    immutable application description

ResolutionOverlay
    mounted component topology
    current component snapshots/revisions
    component capabilities

layout traversal context
    current component scope
```

`component_scope` is derived traversal context, not semantic application input.

## Target structures

Conceptually:

```rust
struct ResolutionOverlay {
    mounts: MountGraph,
    capabilities: MountedCapabilities,
    components: HashMap<ComponentId, ComponentSnapshot>,
}

struct ComponentSnapshot {
    revision: ComponentRevision,
    view: View,
    capabilities: ComponentCapabilities,
}
```

Do not create a `ResolvedView` that recursively clones every semantic node. That would merely rename the old problem.

## Step-by-step implementation

### Step 2.1 — characterize existing resolver errors first

Before edits, identify current tests for:

- missing component;
- duplicate component placement;
- cycle detection;
- focus/focus-within styling;
- mount graph/capability behavior.

Add focused tests if any behavior is currently unpinned.

### Step 2.2 — remove `component_scope` from semantic `ViewNode`

Do not delete scope support from layout/paint. Move it downward.

### Step 2.3 — introduce `ComponentSnapshot`

A snapshot contains exactly the component's current semantic View + revision + capabilities needed by scene resolution/layout.

### Step 2.4 — cache snapshot in `ComponentEntry`

Extend the current registry entry with cached snapshot state keyed by its existing revision.

Mutation path (`with_mut`, `with_any_mut`, or equivalent):

```text
mutate component
increment revision
invalidate cached view/capabilities snapshot
```

Resolution path:

```text
cached revision == current revision
    → clone cached View (O(1))
    → reuse capabilities
else
    → call component.view()
    → call component.capabilities()
    → cache snapshot
```

### Step 2.5 — make resolution produce an overlay/topology result

Resolution still performs correctness work:

```text
validate missing components
validate duplicate placements
detect component cycles
build mount graph
capture snapshots/revisions
capture capabilities
```

It does **not** recursively clone ordinary `View` nodes.

### Step 2.6 — use `ViewFlags` to skip component-free branches

If `!view.flags().contains(CONTAINS_COMPONENT_SLOT)`, topology scanning can stop for that semantic branch.

Count `resolver_nodes_visited` so this optimization is testable.

### Step 2.7 — thread current scope through layout traversal

Layout now receives conceptually:

```text
semantic View
+ ResolutionOverlay
+ current_scope: Option<ComponentId>
```

For ordinary nodes: pass `current_scope` downward.

For `ComponentSlot(id)`:

1. fetch component snapshot from overlay;
2. preserve today's wrapper/slot geometry semantics exactly;
3. descend into `snapshot.view` with `current_scope = Some(id)`.

### Step 2.8 — stamp derived scope into layout style context

`LayoutNode`/style data can continue carrying `component_scope`, because paint legitimately uses it for focus/focus-within and inherited style resolution.

### Step 2.9 — keep whole-scene duplicate/cycle correctness

Do not let caching hide duplicate placements or cycles. Cached component snapshots may avoid re-running `view()`, but their component topology still participates in current-scene validation.

If resolver topology scanning remains expensive after `ViewFlags`, a later measured optimization may cache a compact component-topology summary per `ViewId`. Do not add that now unless counters justify it.

## Required tests

- unchanged component `view()` called once, then not again on unrelated component ticks;
- unchanged component capabilities likewise cached;
- one component mutation invalidates only its snapshot;
- missing/duplicate/cycle tests unchanged;
- focused/focus-within rendering row-for-row identical;
- component-free large subtree produces an early resolver cutoff;
- scene convergence with no component revision change does not regenerate component Views.

## PERF-2 acceptance

```text
no recursive ordinary semantic View reconstruction in resolver
component_scope absent from semantic View
unchanged component view()/capabilities() not regenerated
component correctness behavior unchanged
```

## Suggested commits

```text
refactor(tui): cache component snapshots by revision
refactor(tui): resolve components through an overlay without cloning Views
```

---

# 9. PERF-3 — retained, bounded measurement/layout cache

## Goal

Reuse expensive geometry/text measurement for unchanged semantic identity at unchanged width.

## Exact files

```text
crates/iyon-tui/src/presentation/layout/measure.rs
crates/iyon-tui/src/presentation/layout/prepare.rs
crates/iyon-tui/src/presentation/layout/engine.rs
crates/iyon-tui/src/presentation/layout/tree.rs
crates/iyon-tui/src/scene/host.rs
```

## Key design decision

Measurement and prepare are different caches.

- **Measurement** depends primarily on semantic identity + width intent/width.
- **Prepare** additionally depends on height constraints/flex allocation.
- **Placement** depends heavily on parent coordinates/clip and remains uncached initially.

## Step-by-step implementation

### Step 3.1 — make measured output own cheap semantic handles

Replace lifetime-bound shapes such as:

```rust
MeasuredNode<'a> { view: &'a View, ... }
```

with:

```rust
MeasuredNode { view: View, ... }
```

`View` clone is O(1) now, so this does not deep-copy semantics.

Composite children may be `Arc<MeasuredNode>` if that avoids copying retained measured subtrees.

### Step 3.2 — define measurement key

Start with:

```rust
#[derive(Hash, Eq, PartialEq)]
struct MeasureKey {
    view: ViewId,
    width: u16,
    intent: WidthIntent,
}
```

Only cache nodes whose geometry dependencies are completely represented by the key/current component snapshot identity.

### Step 3.3 — component slots use snapshot View identity

Do not cache a component slot merely by slot identity while ignoring the component revision. Layout descends into the component snapshot's semantic `View`; that snapshot has a `ViewId`, which naturally changes when the component's rendered semantics change.

### Step 3.4 — create `LayoutCache` owned by `SceneHost`

`SceneHost` is retained across frame convergence and is the correct owner.

### Step 3.5 — make the cache bounded from day one

Do **not** use an unbounded `HashMap<MeasureKey, Arc<MeasuredNode>>`.

Use a simple two-generation cache unless existing project utilities provide a better bounded primitive:

```text
current_generation
previous_generation
```

Algorithm per frame/layout epoch:

1. lookup current;
2. if miss, lookup previous;
3. if previous hit, promote/copy cheap Arc into current;
4. if miss in both, measure and insert current;
5. at generation rotation, drop the old previous map and move current → previous.

This bounds retention to roughly two recent working sets and avoids adding a complicated LRU dependency.

If scene convergence invokes multiple layout passes inside one frame, do **not** rotate between convergence passes. Rotate at a meaningful frame/scene epoch boundary so same-frame passes get full reuse.

### Step 3.6 — cache text flow inside measured result

At same `ViewId`/width/intent, repeated layout must not rerun wrapping for unchanged `TextView`.

### Step 3.7 — add prepare cache only after measurement works

Use a key conceptually like:

```rust
struct PrepareKey {
    measured_identity: MeasuredId,
    height_constraint: HeightConstraint,
}
```

Assign a stable internal identity to retained measured objects or key directly with the exact dependencies needed by current prepare logic.

Do not conflate width measurement with height/flex preparation.

### Step 3.8 — keep placement fresh

Do not cache placement yet. Position/clip changes make a safe placement key substantially more complex, and placement should become relatively cheap once measure/prepare are retained.

### Step 3.9 — theme/focus do not invalidate geometry by default

Current theme resolution occurs at paint. Color/focus state changes should not flush measurement unless a future style actually affects geometry. If that changes later, add an explicit geometry-style revision to the key—never hidden global invalidation.

### Step 3.10 — convergence invalidation

If `on_layout_changed` mutates one component and increments its revision, the next convergence pass should create a changed component snapshot/ViewId and miss only where geometry depends on that new identity. Other subtrees remain hits.

## Required benchmark/test cases

### Warm render

Render a 10k-node component-free tree twice at same width.

Second render target:

```text
text_flow_measure_calls == 0
measure_node_calls dramatically lower than first render
```

### One-leaf change

Change one leaf while reusing surrounding shared identity. Only new leaf/affected ancestor measurements should miss; unchanged subtrees hit.

### Focus-only convergence

Trigger a focus/style-context change with unchanged geometry. Measurement should be almost entirely hits.

### Width change

A width-dependent broad remeasure is allowed.

### Cache lifetime

Create many transient ViewIds, advance generations, drop semantic owners, and prove old entries become collectible/dropped. No unbounded growth.

## PERF-3 acceptance

- persistent measured values exist;
- second same-width render does not rewrap unchanged text;
- cache bounded;
- one-leaf update reuses unaffected subtrees;
- placement remains correct;
- focus/theme changes do not unnecessarily invalidate geometry.

## Suggested commits

```text
perf(tui): retain measured layout by View identity
perf(tui): add bounded prepared-layout reuse
```

---

# 10. PERF-4A — retained History layout index for Static and Live units

## Goal

Stop History from rebuilding and remeasuring the entire resident sequence whenever the tail changes.

## Exact files

```text
crates/iyon-tui/src/history/unit.rs
crates/iyon-tui/src/history/model.rs
crates/iyon-tui/src/history/projection/mod.rs
crates/iyon-tui/src/scene/resolve.rs   # dependency extraction helper only if needed
crates/iyon-tui/src/component/registry.rs
```

Do **not** implement stream-specific incremental height logic here. PERF-5 comes next.

## Data model

Prefer co-locating presentation metadata with each History unit rather than maintaining a separate parallel `VecDeque` that can desynchronize.

Conceptually:

```rust
struct HistoryUnitLayout {
    width: Option<u16>,
    content_revision: HistoryContentRevision,
    height: usize,
    component_dependencies: Arc<[(ComponentId, ComponentRevision)]>,
}
```

For this tranche, keep at most the current relevant width per unit. A terminal resize invalidates/replaces that width cache. This is naturally bounded.

History also maintains retained aggregate geometry:

```rust
struct HistoryFlowIndex {
    total_content_height: usize,
    revision: u64,
    // exact additional fields needed for native preserved rows/gaps/padding
}
```

## Static invalidation algorithm

For `HistoryUnitContent::Static(View)`:

```text
cache dependency = ViewId + width
```

At a stable width:

```text
first projection → measure once
later tail append → reuse height
later unrelated component tick → reuse height
```

Static means no component identity is reachable by the unit according to current taxonomy. Do not add application exceptions.

## Live invalidation algorithm

For `HistoryUnitContent::Live(View)`:

```text
cache dependency = root semantic identity + width
                 + exact reachable component revision vector
```

Store the exact dependency set, for example sorted by ComponentId:

```text
[(component_a, rev_12), (component_b, rev_4), ...]
```

Do not hash this to a single number as the sole correctness check.

On projection/update:

```text
same width
and root ViewId unchanged
and every exact component revision unchanged
    → reuse cached height

otherwise
    → re-resolve/re-measure this unit
    → recapture exact dependency vector
    → adjust total content height by (new_height - old_height)
```

Nested component topology can change. When that happens, replace the dependency vector from the new topology result.

## Step-by-step implementation

### Step 4A.1 — add unit presentation metadata

Keep it private to History internals.

### Step 4A.2 — centralize invalidation helpers

Create methods such as conceptually:

```text
unit.invalidate_width()
unit.invalidate_content()
unit.live_dependencies_stale(registry)
```

Do not scatter ad hoc cache resets throughout mutation methods.

### Step 4A.3 — wire every History mutation

Audit and update exact operations in `history/model.rs`, including current equivalents of:

```text
push
push_with_boundary
freeze
discard_live
replace_live_with_stream
push_stream
set_layout
native prefix retirement
unit removal/compaction
```

For PERF-4A, stream units may still use old measurement path, but their presence must not invalidate cached Static/Live metadata before them.

### Step 4A.4 — maintain total flow geometry incrementally

When a cached unit height becomes known or changes:

```text
delta = new_height - old_height
total_content_height += delta
```

Also account exactly for current gap/padding/native preserved-row semantics. Do not change visual behavior.

### Step 4A.5 — stop recomputing total resident height in projection

Replace “walk every unit to discover total height” with retained aggregate geometry.

### Step 4A.6 — viewport-bounded selection

For `FollowEnd`:

1. know total flow height from retained index;
2. walk backward from tail;
3. stop once selected height covers viewport + required boundary context.

For `NativeFrontier`:

1. begin at frontier;
2. walk forward;
3. stop once viewport capacity is covered.

Do not iterate all 1,000 static units merely to calculate overflow.

## Required stress test

Setup:

```text
1000 static one-line units
+ one live tail
stable width
warm cache
```

Update live tail 100 times.

Required:

```text
static-prefix history_units_measured == 0 after warmup
history_units_examined should be viewport-bounded, not 1000*100
```

Resize terminal once:

```text
full width-dependent remeasure allowed once
```

Then another 100 tail updates must return to incremental behavior.

## PERF-4A acceptance

- static unit measured once per relevant width;
- live unit invalidated by exact component revisions;
- total resident flow height retained incrementally;
- viewport selection no longer scans the entire stable resident prefix for ordinary tail updates.

## Suggested commit

```text
perf(tui): retain History geometry for static and live units
```

---

# 11. PERF-5 — make generic streaming truly suffix-incremental

## Goal

This is the highest-value long-session tranche after persistent View/layout retention.

### Repository status: generic recovery already completed

The generic-framework recovery portion of PERF-5 was completed after PERF-0. Do not redo that work or reintroduce the old product-specific host pipeline. The repository already has:

- generic annotated `TextStream` appends;
- thinking normalization and annotations in `plugins/app/iyon`;
- working frames, steering labels, and spinner choreography in the Iyon plugin;
- no `Thinking`, `HostAssistantPipeline`, or product-specific working/assistant stream types in the generic Rust/native TUI boundary;
- generic routed output events and caller-configurable stream pacing.

The remaining PERF-5 work is the incremental generic stream engine: promotable stable prefixes, retained visual row anchors, separate semantic/visual restart coordinates, suffix-only reflow, and removal of any remaining full-history replay or per-character storage.

It has four subproblems that must be solved together:

1. generic `TextStream` must expose newly stable semantic content in promotable pieces;
2. visual row indexing must retain stable anchors and reflow only a safe suffix;
3. `HostTextStream` must stop storing per-character strings/replaying full historic projection;
4. assistant/thinking semantics must leave `iyon-tui`.

## Exact files

```text
crates/iyon-tui/src/stream/text.rs
crates/iyon-tui/src/stream/model.rs
crates/iyon-tui/src/stream/resident.rs
crates/iyon-tui/src/stream/node.rs
crates/iyon-tui/src/stream/append.rs
crates/iyon-tui/src/stream/viewport/index.rs
crates/iyon-tui/src/stream/compile/mod.rs
crates/iyon-tui/src/stream/compile/text.rs
crates/iyon-tui/src/stream/compile/rows.rs
crates/iyon-tui/src/history/stream.rs
crates/iyon-tui/src/application/host.rs
packages/iyon-runtime/src/tui/stream.ts
```

App/native adapter location for removed assistant semantics will likely be `iyon-native` or the application/runtime layer that already owns agent semantics. Do not put them back into another `iyon-tui` module.

## 11.1 Fix generic `TextStream` stable-prefix capture

### Current failure mode

Current source resembles:

```text
source String = "all text not compacted yet..."
snapshot() => one exact_text node covering source_base..source_end
```

Suppose:

```text
source_base    = 0
stable_through = 900
source_end     = 1000
```

The single node covers `[0,1000)`, so `StreamModel` cannot move it to `ResidentPrefix` even though `[0,900)` is semantically stable. The node ends after the frontier.

### Correct output invariant

An append-only source must expose stable semantics in units whose ranges can be promoted independently.

For plain text, the simplest safe shape is:

```text
stable node:   [source_base, stable_through)
unstable tail: [stable_through, source_end)
```

when both ranges are non-empty.

### Step-by-step algorithm

1. Preserve existing `append.rs` stable checkpoint calculation.
2. Compute `stable_rel = stable_through - source_base` safely.
3. Verify it is a valid UTF-8 boundary; existing grapheme/display checkpoint semantics should guarantee a safe boundary. Assert in debug.
4. Emit `exact_text(stable_slice)` with owned range ending exactly at `stable_through`.
5. Emit `exact_text(tail_slice)` for remaining unstable bytes.
6. `StreamModel::refresh()` can now promote the stable node to `ResidentPrefix`.
7. Once promoted and release/compaction rules permit, compact source prefix.
8. After compaction, future snapshots must not re-copy already-resident text because the source buffer no longer contains it.

### Important: do not solve this with ever-growing duplicate chunks

After content becomes resident, the mutable source should be allowed to forget it. ResidentPrefix owns the stable semantic representation.

### Required tests

- repeated ASCII chunks;
- multi-byte UTF-8 split across appends;
- combining marks/grapheme clusters;
- newline advances frontier to source end according to current rules;
- open final word remains unstable as current semantics require;
- seal makes final suffix stable;
- after N chunks, old compacted source bytes are no longer retained in `TextStream` mutable source.

## 11.2 Retain row anchors and separate semantic vs visual damage

### Three coordinates you must track

```text
semantic_stable_through
    semantic content strictly before here will not change

semantic_changed_from
    earliest source coordinate whose semantic projection changed this refresh

visual_restart_from
    earliest source coordinate whose VISUAL rows may change
```

They are not interchangeable.

### Why visual restart can be earlier than append offset

At width 10:

```text
"hello wor"
```

then append:

```text
"ld"
```

A word-aware wrapper may decide row breaks differently for the existing characters in `wor` once it knows the word continues. Reflow may need to restart at the beginning of the relevant hard line/word context, not byte 9 where new text began.

### Conservative safe restart algorithm for first implementation

Use **hard-line start** as the visual invalidation boundary.

On refresh:

1. determine earliest semantic changed source coordinate;
2. locate the hard-line start that contains that coordinate in the retained/indexed source/row metadata;
3. set `visual_restart_from` to that hard-line start;
4. keep every existing row anchor whose owned source range is strictly before `visual_restart_from`;
5. discard row anchors at/after restart;
6. compile semantic/visual rows from the restart point through current suffix using the existing wrapping compiler;
7. append new anchors to retained prefix anchors;
8. assert row source offsets are monotonic and cover the same logical output as a cold full compile.

This is conservative: it may reflow one entire hard line, but not the whole transcript.

Only optimize to a smaller word/grapheme restart boundary later if benchmarks show extremely long hard lines are a real problem and correctness tests can prove the tighter boundary.

### Width change

Width change invalidates wrapping globally for the stream presentation cache. A full row rebuild is allowed.

### Native release/compaction

When source/resident prefix is retired, drop corresponding row anchors and adjust base coordinates consistently. Never leave row anchors referring to retired source offsets.

## 11.3 Add retained stream presentation state

`TypedHistoryStream` should own presentation state conceptually like:

```rust
struct StreamLayoutCache {
    width: u16,
    revision: StreamRevision,

    semantic_base: StreamOffset,
    stable_through: StreamOffset,
    source_end: StreamOffset,

    indexed_from: StreamOffset,
    indexed_through: StreamOffset,

    visual_restart_from: StreamOffset,
    rows: StreamRowIndex,
}
```

Do not blindly rebuild `StreamRowIndex` on every revision.

### Core invariant

```text
old visually stable row anchors
+
recompiled damaged/new suffix
=
new row index
```

Instrument:

```text
stream_semantic_restart_offset
stream_visual_restart_offset
stream_rows_reindexed
stream_stable_rows_reused
```

## 11.4 Fix `HostTextStream` storage/projection

### Current anti-pattern

Feature-gated host code creates one `String` per character in pacing atoms and later reconstructs projection from historic atoms.

This is allocation-heavy and O(total-content) per refresh.

### Target storage

Use retained chunks/source spans:

```rust
struct HostSourceChunk {
    range: StreamRange,
    // generic semantic metadata only
    text: String,
}
```

If generic segment metadata is needed, make it generic semantic metadata—not `Thinking`.

Pacing state stores offsets/grapheme boundaries into retained source, not a newly allocated String per character.

### Append algorithm

One incoming append:

```text
1. append one owned source chunk/string
2. update source_end
3. advance pacing/stability frontier using existing generic rules
4. project only semantic suffix affected by that frontier/restart
5. refresh row cache only from visual_restart_from
6. increment stream revision
7. invalidate host/scene
8. return
```

No full `ProjectionBuilder` replay over all historic atoms.

## 11.5 Remove assistant/reasoning semantics from `iyon-tui`

Delete/move concepts from `crates/iyon-tui/src/application/host.rs` such as:

```text
HostAssistantSegmentKind::Thinking
HostAssistantPipeline
ThinkingRewriter
app/thinking semantic tag
assistant-specific pacing policy naming
```

Markdown is allowed in `iyon-tui`: Markdown is a generic source format.

Thinking/reasoning semantics belong in the application/native adapter layer. That adapter may map application events into generic TUI semantic tags/styles or generic stream operations.

### TypeScript correction

Current `packages/iyon-runtime/src/tui/stream.ts` API exposing:

```ts
appendSegment(kind: "text" | "thinking", ...)
```

is not generic.

Prefer a core API such as:

```ts
stream.append(text)
```

plus, only if existing generic semantic styling requires it, something like:

```ts
stream.appendSemantic({ namespace, name, text, ... })
```

The exact public generic semantic type must mirror Rust capabilities. Do not rename `thinking` to another hard-coded app word.

## PERF-5 performance gate

Canonical fixed 256-byte append chunks.

After warmup:

```text
cost(next chunk at 100 KiB)
```

must remain in the same order of magnitude as:

```text
cost(next chunk at 10 KiB)
```

It must not scale linearly with accumulated transcript bytes.

Also assert mechanically that restart offsets advance with the stream rather than repeatedly returning to source base for stable content.

## Full-suite checkpoint

This is one of the designated full checkpoints. After narrow tests and crate tests pass, run the full workspace/API-surface/Bun suite required by the repository's existing validation policy.

## Suggested commits

```text
fix(tui): expose stable plain-text stream prefixes for retention
perf(tui): retain stream row anchors and reflow only damaged suffixes
refactor(tui): make host streaming generic and remove assistant semantics
```

---

# 12. PERF-4B — connect incremental stream heights into History

## Goal

Complete History retention without treating a stream revision as a full-unit invalidation.

## Exact files

```text
crates/iyon-tui/src/history/stream.rs
crates/iyon-tui/src/history/unit.rs
crates/iyon-tui/src/history/model.rs
crates/iyon-tui/src/history/projection/mod.rs
```

## Algorithm

The stream layout cache from PERF-5 can report current width/revision/index/height.

When a stream changes:

```text
old_height = cached stream presentation height
refresh suffix row index
new_height = new row index height
delta = new_height - old_height
History.total_content_height += delta
```

Do not remeasure static/live prefix units.

If stream native release removes rows from front, apply the corresponding negative delta and update base/frontier bookkeeping atomically with stream cache retirement.

## Step-by-step

1. Add a stream layout query/update method that returns whether height changed and by how much.
2. Replace History's old “prepare whole stream again because revision changed” behavior with `stream.refresh_layout(width)`.
3. Update per-unit cached height from stream cache result.
4. Apply aggregate History flow-height delta.
5. Ensure `FollowEnd` viewport stays pinned correctly when only tail rows are added.
6. Ensure native-frontier mode remains correct when prefix rows are retired.
7. Ensure terminal width change still performs one full stream row rebuild and updates delta once.
8. Add counters proving no static/live remeasure occurs because stream revision advanced.

## Stress test

```text
1000 static units
1 stream tail
warm width
append 256 bytes 1000 times
```

Required:

- static-prefix measurements remain zero after warmup;
- per-append stream row work is suffix-bounded;
- History total height remains exact;
- viewport output equals cold-reference projection.

## Suggested commit

```text
perf(tui): integrate incremental stream geometry into History
```

---

# 13. PERF-6 — direct retained N-API semantic View decoder

## Goal

Preserve TS subtree identity across the JS→Rust boundary and remove recursive JSON lowering from the hot path.

## Exact files

Rust:

```text
crates/iyon-native/src/tui.rs
crates/iyon-native/Cargo.toml      # only if N-API feature/import changes are necessary
crates/iyon-tui/src/presentation/ir.rs  # opaque WeakView bridge hooks already prepared
```

TypeScript:

```text
packages/iyon-runtime/src/tui/values/view.ts
packages/iyon-runtime/src/tui/ir.ts
packages/iyon-runtime/src/tui/materialize.ts
packages/iyon-runtime/src/tui/runtime.ts
packages/iyon-runtime/src/tui/history.ts
packages/iyon-runtime/src/tui/component.ts
packages/iyon-runtime/src/tui/scroll-pane.ts
```

Also update affected TUI binding/runtime tests.

## 13.1 Do not start with an opcode buffer

Candidate A is the production implementation first:

```text
existing TS private View DAG
+
private stable NodeId
+
numeric fixed-shape node discriminants
    ↓ one synchronous N-API operation
native decoder
    ↓
WeakView cache cutoff
    ↓
final Arc-backed iyon_tui::View
```

No second JS traversal/encoding pass beyond reading the semantic node structure itself on cache misses.

## 13.2 Add private exact TS NodeIds to the existing DAG

### Identity requirements

- every private semantic node gets one monotonically increasing safe integer ID;
- immutable node object keeps that ID forever;
- changed semantics create a new private node and therefore new ID;
- reused child object reuses ID;
- ID is not public API.

### Survive module reloads

Do not use a simple module-local `let nextId = 1` if old `View` objects can survive module reload/re-evaluation.

Use a realm-global private symbol counter conceptually:

```ts
const COUNTER = Symbol.for("iyon:tui:private-view-node-counter")

type CounterBox = { next: number }
const root = globalThis as typeof globalThis & { [COUNTER]?: CounterBox }
const box = root[COUNTER] ??= { next: 1 }

function nextViewNodeId(): number {
  if (box.next > Number.MAX_SAFE_INTEGER) {
    throw new Error("TUI View node identity exhausted")
  }
  return box.next++
}
```

Keep the actual symbol/helper private to the package.

## 13.3 Replace hot string discriminants with numeric codes

Current IR strings such as view kind/wrap/flex/etc. should not repeatedly cross N-API merely for dispatch.

Use canonical numeric discriminants for the hot recursive View representation.

**Important:** do not hand-maintain mismatched magic numbers in Rust and TS.

Preferred options, in order:

1. generate both sides from a tiny canonical schema already compatible with repository tooling;
2. if codegen is excessive, define a single checked mapping with exhaustive tests that compare exported/test-only schema versions.

Add a private bridge schema version if needed so native can fail clearly on mismatched package/addon versions rather than misdecode.

## 13.4 Arrays vs objects is not an architectural decision yet

The private node may be a fixed-shape object or fixed array. Candidate A should choose the simpler representation that preserves direct access. PERF-7 measures whether alternate shapes/packed transport matter.

Do not delay the direct decoder waiting for the “perfect” packed format.

## 13.5 Create a per-environment native weak cache

Conceptually:

```rust
struct ViewBridgeCache {
    nodes: HashMap<ViewNodeId, WeakView>,
}
```

`WeakView` is the opaque `iyon-tui` type. `iyon-native` never depends on `ViewNode` internals.

### Environment-lifetime requirement

The cache is per N-API environment/realm/addon environment, not a process-global forever map.

For the version actually used by Iyon (`napi` 3.12.1), napi-rs already exposes `Env::add_env_cleanup_hook` (available because its default `napi4` feature includes `napi3`) and explicitly documents that an `Env` must not be cached/reused across Worker environments and becomes invalid when the addon environment unloads. **Use that cleanup-hook API; do not take ownership of raw `napi_set_instance_data` for this cache.**

Recommended implementation:

```rust
static VIEW_BRIDGE_CACHES: OnceLock<Mutex<HashMap<EnvKey, Arc<Mutex<ViewBridgeCache>>>>> = ...;
```

where `EnvKey` is the raw `napi_env` pointer value obtained inside the decoder entry point. On first use of a new environment:

1. create exactly one `ViewBridgeCache`;
2. insert it into the side table under that environment key;
3. construct a temporary `napi::Env` from the same raw env only for the current call;
4. register `Env::add_env_cleanup_hook(env_key, |key| remove_cache(key))`;
5. record that the cleanup hook is installed so repeated decoder calls do not register duplicates.

Never store the `napi::Env` object itself in the side table. Store only the inert pointer-sized key and native Rust cache state. The cleanup hook is the lifetime authority.

The cache map itself may be process-global because every entry is environment-keyed and deterministically removed on environment teardown; the semantic View entries inside each cache remain `WeakView`s.

Add a Bun worker teardown test that repeatedly creates/destroys workers/addon environments and asserts the side-table environment count returns to baseline. This is mandatory because Bun's N-API implementation is actively maintained and environment-lifetime behavior must be proven on the actual runtime.

## 13.6 Direct decoder algorithm

This is the central algorithm. Implement it exactly in this order.

```text
decode_view(env, js_node, cache):

1. read nodeId FIRST
2. increment napi_view_nodes_seen

3. cache lookup(nodeId)

4. if WeakView exists and upgrades:
       increment napi_view_cache_hits
       return upgraded View
       DO NOT read kind
       DO NOT read children
       DO NOT read strings/styles

5. if entry exists but WeakView expired:
       remove entry

6. increment napi_view_cache_misses

7. read numeric kind + local scalar fields
8. recursively decode only child nodes required by this cache miss
9. construct the FINAL Arc-backed iyon_tui::View directly
10. cache view.downgrade() under nodeId
11. return View
```

The hit cutoff is the reason this is fast.

Example:

```text
new root #900
├── old huge child #41
└── new child #899
```

Native work:

```text
#900 miss → inspect
#41 hit  → Arc clone and STOP
#899 miss → inspect/decode
```

The 10k-node subtree under `#41` is not crossed again.

## 13.7 Decoder safety rules based on Bun's actual N-API implementation

- Decoder is synchronous.
- Private node structures must not rely on user getters/proxies executing arbitrary JS during decoding.
- Do not store borrowed JS string/typed-array pointers after the call.
- Convert text strings into native-owned text exactly once on a cache miss.
- Do not call back into JS during recursive decode.
- Cache only weak native semantic handles so the bridge does not retain every historical View forever.
- Prune expired entries opportunistically after a reasonable map-size threshold or during periodic cache maintenance.

## 13.8 Remove mandatory `NativeTuiView` two-step materialization

Current conceptual path:

```text
materializeView(view)
→ NativeTuiView
→ history.push(nativeView)
```

Target public use:

```ts
history.push(view)
history.freeze(unit, view)
slot.setView(view)
pane.setContent(view)
tui.render(view)
```

Each native operation invokes the same decoder internally.

If root already exists in cache:

```text
one N-API call
read nodeId
WeakView upgrade
Arc clone
state mutation
return
```

Keep a private native materialized object only if a real internal use case still needs it; it must not be mandatory.

## 13.9 Stateful objects remain N-API classes

Keep native handles/classes for:

```text
TuiHost
History
TextInput
ViewSlot
ScrollPane
Working
streams
```

Do not serialize their state into tree transactions.

## 13.10 Low-frequency values

Use normal typed napi-rs objects/enums for configuration/snapshot DTOs such as layout options/theme setup/action results where recursive hot-tree cost is irrelevant.

Do not use a custom raw decoder everywhere merely for consistency.

## 13.11 Strings

Do not force ordinary JS strings through `TextEncoder` to claim zero-copy.

For an owned native text node on cache miss:

```text
JS String
→ N-API UTF-8 conversion
→ Rust-owned string/text
```

is appropriate.

## Required conformance tests

Run under **Bun**, and where practical compare with Node:

1. same node object decoded twice → second root cache hit, no child reads;
2. shared-path tree → only changed path + root misses;
3. JS references dropped + GC → weak native cache does not keep tree alive forever;
4. cache entry with expired `WeakView` is removed/redecoded safely;
5. worker/environment teardown frees environment cache;
6. repeated worker creation cannot see another environment's NodeId cache entries;
7. module reload counter does not reuse live NodeIds;
8. malformed private node fails cleanly with no UB;
9. numeric enum mismatch/schema mismatch produces explicit error;
10. text strings containing Unicode round-trip exactly.

## PERF-6 acceptance

```text
0 serde_json::Value on recursive production TUI View path
0 generic JSON View lowering
0 mandatory materializeView → NativeTuiView → operation sequence
cache hit stops before kind/children
WeakView cache does not retain dead trees
arbitrary semantic tree replacement remains correct
```

## Suggested commits

```text
refactor(tui): add stable private TS View node identities
perf(native): decode retained TUI Views directly over N-API
refactor(runtime): pass semantic Views directly to native TUI mutations
```

---

# 14. PERF-7 — direct structured decoder vs packed TypedArray transaction

## Goal

Empirically decide whether an opcode/packed transport is worth its complexity.

**Do not merge a packed production path before this experiment.**

## Candidate A — direct retained structured decoder

Already implemented in PERF-6:

```text
private TS node DAG
→ one N-API operation
→ NodeId lookup
→ WeakView cutoff
→ decode only misses
```

## Candidate B — packed transaction

Use conceptually:

```text
Uint32Array structural words
+
string[]
```

The transaction must support references to already-known NodeIds so unchanged subtrees are not serialized again.

Do **not** build a full-tree opcode buffer every update.

A Candidate-B-only JS cache may track which node IDs native is expected to know. Define reset/environment semantics carefully so benchmark conditions are fair.

## TypedArray lifetime rule

Bun's Node-API typed-array info exposes backing memory synchronously. Native may borrow the `Uint32Array` input for the duration of the N-API call.

**Never retain that pointer/slice after return.**

If a later stage needs the transaction after returning to JS, copy the required data into native-owned memory before return.

## Benchmark matrix

Tree sizes:

```text
20
200
2,000
10,000 nodes
```

Update modes:

```text
COLD
IDENTICAL_IDENTITY
SHARED_PATH
REBUILT_EQUIVALENT
```

At least 20 recorded post-warmup samples per workload; preferably enough for stable p95.

## Measure the entire public operation

For Candidate A and B include:

```text
public View construction
+
any JS reconciliation/encoding/cache work
+
N-API call
+
Rust decode/cache work
+
final View ready for insertion
```

Do not publish only “native boundary decode ns.”

## Cache fairness

For each pattern define whether cache is warm/cold and reset both candidates equivalently. Do not let Candidate B start with a warm known-node table while Candidate A starts cold.

## Benchmark-based production decision

The decision must be based on repeatable end-to-end benchmark results, not on
native-boundary measurements, estimates, or implementation preference. Measure
at least the median and p95 total latency, JS/native CPU, and memory/allocation
behavior under equal cache conditions.

Use the measured total-latency improvement over Candidate A as the decision
threshold:

```text
< 5% improvement:
  Do not keep Candidate B in production.

>= 5% and < 15% improvement:
  Keep Candidate B only when the added complexity is manageable.

>= 15% improvement:
  Consider keeping Candidate B even with a somewhat notable complexity tax.
```

These thresholds apply only when the benchmark suite demonstrates the result
reliably across representative workloads, including the 2k/10k-node cases and
small-workload regressions. Memory/allocation regressions, encoder-dominated
JS CPU, or correctness failures must be reported alongside the latency result
and can reject the packed transport regardless of its speedup.

If the benchmark evidence does not justify Candidate B:

- delete the production packed implementation;
- keep benchmark notes/tests as appropriate;
- Candidate A remains the only production transport.

Do not keep two production transports “in case.”

## PERF-7 completion note

PERF-7 is complete. The packed transport was benchmarked end to end and then
removed because it was not a sufficiently general or consistently better
production path. The default direct structured decoder remains the only
transport.

The authoritative run used 20 post-warmup samples for every 20/200/2,000/
10,000-node workload and measured public construction or packing, N-API,
Rust decode/cache work, final rendering, CPU, heap, and perf counters. Packed
improvement over direct total-operation latency was:

```text
nodes   mode                   median       p95
2,000   COLD                   +48.4%       +49.8%
2,000   IDENTICAL_IDENTITY      +4.1%       +39.0%
2,000   SHARED_PATH             -0.5%       +38.5%
2,000   REBUILT_EQUIVALENT     +54.1%       +35.7%
10,000  COLD                   +63.6%       -55.6%
10,000  IDENTICAL_IDENTITY      -0.4%       +55.0%
10,000  SHARED_PATH             -0.7%       +47.8%
10,000  REBUILT_EQUIVALENT     +72.3%       +42.2%
```

The packed path did not consistently meet the 5% median threshold for warm
identity/shared-path workloads, had unstable 10,000-node tail behavior, and
only covered the text/column benchmark shape rather than the complete generic
View schema. Those correctness-scope and consistency issues outweighed the
cold/rebuild speedups, even where they exceeded the 15% consideration
threshold. No packed production transport or string-arena follow-up remains.

## Optional string arena only if B wins

Then compare:

```text
Uint32Array + string[]
```

versus:

```text
Uint32Array + UTF-8 Uint8Array arena
```

Include **JS encoding time**. Use UTF-8 arena only if total latency improves.

## External ArrayBuffers

Do not use them for primary JS→Rust input. They may be a future native→JS snapshot experiment with explicit Bun conformance tests, but they are outside this tranche unless a benchmark requires them.

## Suggested commit

If A wins, likely only benchmark infrastructure/result rationale remains:

```text
bench(native): compare retained View decoder with packed transport
```

If B wins, separate measured production commit after the benchmark evidence.

---

# 15. PERF-8 — remove artificial Promise allocation from synchronous TUI APIs

## Goal

Make synchronous native state mutations/reads synchronous in TypeScript.

## This was a public API migration

At tranche start, the contract was:

```ts
export type TuiOperation<T> = Promise<T>
```

and `HandleBase.call()` wrapped synchronous native operations in
`Promise.resolve(...)`. PERF-8 changed the completed contract to:

```ts
export type TuiOperation<T> = T
```

`HandleBase.call()` now returns direct values. Callers doing `.then(...)` on
synchronous operations must migrate; callers using `await` remain compatible
because `await` accepts ordinary values. Real waits remain Promise-based.

## Exact inspected TS files

```text
packages/iyon-runtime/src/tui/types.ts
packages/iyon-runtime/src/tui/handles.ts
packages/iyon-runtime/src/tui/history.ts
packages/iyon-runtime/src/tui/runtime.ts
packages/iyon-runtime/src/tui/scroll-pane.ts
packages/iyon-runtime/src/tui/text-input.ts
packages/iyon-runtime/src/tui/stream.ts
packages/iyon-runtime/src/tui/component.ts
```

Rust implementation source:

```text
crates/iyon-native/src/tui.rs
```

At tranche start, grep every `HandleBase.call(` call site and classify it by whether the native implementation truly waits.

## Classification rule

### Synchronous

Operations that only mutate/query retained native state and return immediately, including current equivalents of:

```text
History.push
History.freeze
History.setLayout

ViewSlot.setView
ViewSlot.setAnimation
ViewSlot.stopAnimation

ScrollPane.setContent
ScrollPane.followEnd
scroll/query operations whose native implementations are immediate

TextStream append/mutation
Working setActive/setPending

Tui setTheme/render/style invalidation
synchronous dispose/revision/getter operations
```

### Asynchronous

Keep promises only for real waits:

```text
nextAction()
terminal/input wait
operations that await native asynchronous work
shutdown only if the native implementation genuinely waits
```

Do not retain a Promise solely for API symmetry.

## Step-by-step

1. Inventory all `HandleBase.call()` users.
2. For each, inspect matching `#[napi]` Rust method.
3. Mark `sync` or `async` in a temporary implementation checklist.
4. Change `HandleBase` so sync calls return direct values; create a separate helper only for actual async calls if useful.
5. Replace blanket `TuiOperation<T>` with accurate return types.
6. Update public runtime/history/scroll/input/stream method signatures.
7. Remove `Promise.resolve(operation())` around sync native calls.
8. Update tests that expect Promise identity/.then behavior.
9. Add migration note in public API docs/changelog if repository policy requires.
10. Run API-surface scanner immediately because this tranche intentionally changes public TS contracts.

## N-API rule

Do not introduce ThreadsafeFunction or worker dispatch machinery for same-thread synchronous mutations.

If a sync method consumes a borrowed TypedArray in Candidate B, the borrow ends before the method returns.

## PERF-8 completion note

PERF-8 is complete. Synchronous TUI mutations and reads now return directly
through the TS facade and declarations: HandleBase operations, History,
TextInput, TextStream, ViewSlot/Component, ScrollPane, and Tui render/resize,
close, exit, theme, and retained-state operations. `nextEvent`/`nextAction`
and actual terminal waits remain asynchronous.

Evidence: implementation commit `8fca589`, direct-return contract test commit
`9e0b5d4`, `bun run t5:check`, `cargo test -p iyon-native`, and the focused
TUI handle/runtime tests. No synchronous TUI method remains typed as a
Promise; the remaining Promise adapters are for caller-supplied async
renderer/projector/component/stream hooks rather than native TUI operations.

## Full-suite checkpoint

Run full workspace/API-surface/Bun suite after this tranche.

## Suggested commit

```text
perf(runtime): make synchronous TUI bindings actually synchronous
```

---

# 16. PERF-9 — generic retained text API for high-frequency content

## Goal

Expose the incremental machinery from PERF-5 through the public Rust/TS TUI API without inventing app-specific streams.

## First decide which semantic model is actually needed

### Append-only `TextStream`

Use for:

```text
logs
chat text
compiler output
file tails
command output
incremental Markdown source
other monotonic streams
```

Contract:

```text
content before stable frontier becomes immutable
```

This should already be largely solved by PERF-5.

### Editable `TextBuffer`

Only add if there is a concrete high-volume use case requiring arbitrary range replacement of old content.

Contract is fundamentally different:

```text
old ranges may become dirty again
→ use damage ranges/revisions, not monotonic stable frontier
```

Do not force editable semantics into `TextStream`.

## Exact likely files

```text
crates/iyon-tui/src/stream/text.rs
crates/iyon-tui/src/stream/mod.rs
crates/iyon-tui/src/application/host.rs      # generic host exposure only
crates/iyon-native/src/tui.rs
packages/iyon-runtime/src/tui/stream.ts
packages/iyon-runtime/src/tui/index.ts
packages/iyon-runtime/src/tui/types.ts
```

Only add new TextBuffer files if the use case is real and accepted.

## Public append-only path

High-frequency append should be:

```text
one JS string crossing
→ append into native retained generic TextStream
→ advance revision/stability
→ suffix-only semantic/row update
→ mark host dirty
→ return
```

It must **not** be:

```text
read old full string
concatenate in JS
rebuild full View::text(full_text)
resend entire tree/document
```

## Rust/TS API symmetry

Introduce/update both languages together. Do not expose a capability in TS that Rust does not semantically model or vice versa.

## Ordinary text replacement remains ordinary View

Small text changing `"Ready" → "Running"` inside a component does not need stream machinery. Component revision + persistent View/layout caches already make that cheap.

Large intentional document replacement is inherently proportional to the replacement and may use a new semantic View identity.

## Acceptance

- public append-only stream uses PERF-5 incremental source/model/index;
- no app-specific names;
- long append benchmark remains flat-ish;
- no second streaming engine exists;
- optional TextBuffer, if added, has explicit damage-range semantics and independent tests.

## PERF-9 completion note

PERF-9 is complete. The public native `TextStream` path now uses the generic
PERF-5 `iyon-tui::TextStream` source for ordinary unprojected streams, so each
append crosses N-API once and reaches the retained source/model/index without
rebuilding the accumulated text in JavaScript. Annotated and Markdown streams
continue through the generic host adapter and retain their existing semantic
projection behavior.

The stream-to-View lowering now coalesces adjacent continuous text nodes before
constructing the static semantic View. This preserves one physical row when a
stable resident prefix and an unstable append suffix share a hard line; without
this, the retained engine's row index and semantic View could disagree and drop
the final suffix under native History anchoring.

No editable `TextBuffer` was added: the repository's high-volume stream use is
append-only, while the existing text-input buffer remains a separate control
implementation. The SDK declarations now mirror the runtime stream options,
History stream operations, and annotated snapshot shape.

Verification included the compiled native/Bun path, a fixed 256-byte newline
append probe (median per-append latency remained approximately flat from the
20th through the 200th append), focused native/runtime stream tests, TypeScript
typecheck, API-surface validation, and Rust formatting/diff checks. The
implementation was committed as `6b47453`.

## Suggested commit

```text
feat(tui): expose generic retained text streaming to TypeScript
```

If TextBuffer is genuinely needed, give it a separate commit and benchmark.

---

# 17. PERF-10 — conditional paint cache, only if profiling demands it

## Gate

After PERF-1 through PERF-9, rerun PERF-0 profiles.

If paint is **< 15% of p95 dirty-frame CPU**, stop. Mark PERF-10 unnecessary.

If paint is **>= 15%**, proceed.

## Why the key is complicated

Inspected `crates/iyon-tui/src/presentation/paint/view.rs` shows paint depends on more than semantic `View`:

```text
View identity
allocated geometry
inherited PhysicalStyle
Theme resolution/revision
StyleContext from style states/facts
component scope
focus/focus-within state
clip/viewport behavior
```

Therefore this is invalid:

```text
PaintCache[ViewId]
```

It can return physically wrong colors/focus/inherited styling/viewport pixels.

## Minimum safe key dependencies

Conceptually:

```text
ViewId
exact allocated geometry
ThemeRevision
resolved inherited style context identity/value
focus/focus-within state
clip/viewport state
```

A cached physical subtree should be bounded similarly to layout cache.

## Implementation strategy if gate is crossed

1. Profile whether surface allocation or compositing dominates.
2. Choose the narrowest cache target—e.g. expensive text surface—before giant subtree surfaces.
3. Define exact dependency key.
4. Add correctness tests for theme switch, focus move, inherited style change, viewport scroll, and same View at different geometry.
5. Add bounded retention policy.
6. Re-benchmark memory as well as CPU.

Do not execute this tranche merely because caching is fashionable.

## PERF-10 completion note

PERF-10 proceeded because the PERF-0 post-PERF-9 gate measured paint at roughly
29–46% of dirty-frame p95 across the 2,000/10,000-node text, column, and styled
span workloads; the pre-cache paint share was approximately 99%. The retained
subtree cache is owned by `SceneHost`, uses two generations, and caches only
component-free descendants. Its key includes semantic identity, all allocated
geometry/clip rectangles, inherited and resolved physical styles, and both
node/descendant style-context state. Theme changes clear both generations.

Correctness coverage includes theme changes, inherited styles, focus movement,
viewport scrolling, geometry changes, and arbitrary component updates. The
retention test holds at most two generations; a release 10,000-node probe
reported 222,625,792 bytes maximum resident set size with five iterations.

The final 20-sample release gate at commit `ed6facf` recorded cache hits on
every warmed shared subtree and only 60 paint-node visits over 20 dirty frames.
Representative cached p95 dirty/paint times were 3.17/1.47 ms (text 2,000),
22.83/8.86 ms (text 10,000), 7.00/2.23 ms (column 10,000), and 16.83/6.63 ms
(styled span 10,000). `cargo test -p iyon-tui`, `cargo test --workspace`,
`cargo test -p iyon-native`, `bun run t5:check`, the focused Iyon app tests,
and formatting/checks passed. The native addon was restaged before the Bun
verification so retained component updates and animation use the current Rust
implementation.

---

# 18. ~~PERF-11 — deferred non-TUI `iyon-native` boundary cleanup~~ — **not needed**

> Decision: do not implement this tranche. `iyon-core` and `iyon-api` will be
> TypeScript, while provider/network JSON stays in JS/TS where it is not the
> bottleneck. The detailed design below is retained only as historical context.

## Start condition

Do **not** start until final TUI performance gate passes.

## Exact inspected files

```text
crates/iyon-native/src/core.rs
crates/iyon-native/src/events.rs
crates/iyon-native/src/model_turn.rs
crates/iyon-native/src/api.rs            # conversion helpers implicated by existing callers
crates/iyon-native/src/value.rs          # generic Value helper boundary
```

## Rule for each field/type

```text
stable protocol envelope
    → typed N-API structure/enum

retained native object
    → N-API class

bulk numeric/binary data
    → borrowed TypedArray where measured beneficial

genuinely arbitrary user/tool data field
    → serde_json::Value allowed for that exact field
```

Do not wrap a typed envelope in `Value` because one nested field happens to be arbitrary.

## 18.1 Queue Rust events, not preconverted JSON

Current:

```rust
mpsc::Sender<Value>
mpsc::Receiver<Value>
```

and `emit(CoreEvent)` converts via `events::core_event(&event)` before queueing.

Target:

```rust
mpsc::Sender<CoreEvent>
mpsc::Receiver<CoreEvent>
```

Convert to JS/N-API output only when `nextEvent`/`nextEvents` delivers across the boundary.

Benefits:

- queue stores domain type;
- no premature JS/JSON representation allocation;
- batching/coalescing can operate on structured events;
- arbitrary nested JSON remains only inside exact CoreEvent fields that are arbitrary.

## 18.2 Make `pushMany()` a real batch

Current `model_turn.rs`:

```text
for value in values
    self.push(value).await
```

Target algorithm:

```text
1. validate batch length once
2. convert all stable envelopes to Rust stream events
   (or fail before mutating turn if atomic validation is desired by current semantics)
3. lock/take mutable NativeModelTurn once
4. push all events in order
5. collect resulting CoreEvents
6. unlock
7. enqueue/deliver collected events efficiently
```

Preserve current cancellation/backpressure semantics. Do not accidentally turn a partially-applied batch into all-or-nothing unless that is deliberately specified and tested.

## 18.3 Add `nextEvents(max)`

For burst delivery:

```text
wait for at least one event using current async semantics
then drain up to max immediately available events
convert/deliver as one JS batch
```

Avoid one JS wakeup/crossing per event under high-rate streams.

Bound `max` to a reasonable value.

## 18.4 Coalescing

Only coalesce adjacent compatible text deltas when semantic ordering is unchanged. Never coalesce across tool/event boundaries or reorder lifecycle events.

## ~~PERF-11 acceptance~~ — not applicable

- event queue holds `CoreEvent`;
- stable envelopes typed at N-API boundary;
- arbitrary fields remain arbitrary only where semantically required;
- `pushMany` uses one turn mutation critical section rather than looping public `push`;
- burst `nextEvents(max)` available and tested;
- no event ordering regressions.

## ~~Final full-suite checkpoint~~ — covered by the final TUI/Perf-10 validation

---

---

# 19. Cache algorithms and invariants — reference section

This section is deliberately redundant. Use it when implementing caches so you do not accidentally weaken correctness.

## 19.1 View identity

```text
View::clone()
    same Arc
    same ViewId

semantic mutation
    new root Arc
    new ViewId
    unchanged children keep their Arc/ViewId

separately rebuilt equivalent value
    different ViewId
    PartialEq may still be true
```

Do not content-hash/intern rebuilt-equivalent trees in this project unless a future benchmark specifically justifies it.

## 19.2 Component snapshot identity

```text
ComponentRevision unchanged
    component.view() NOT called
    cached View clone O(1)

ComponentRevision changed
    regenerate component View/capabilities once
    resulting semantic View has its own new identities as constructed
```

## 19.3 Measurement cache dependency

For ordinary semantic nodes:

```text
(ViewId, width, WidthIntent)
```

must contain every geometry dependency. If a future feature adds a hidden geometry-affecting global, make that dependency explicit in the key.

## 19.4 History unit dependency

```text
Static:
  ViewId + width

Live:
  ViewId + width + exact reachable (ComponentId, Revision) set

Stream:
  width + stream semantic/presentation revisions/frontiers
  but revision does NOT imply full row recomputation
```

## 19.5 Stream stability

```text
semantic stable frontier
    source semantics before this point cannot change

visual restart frontier
    rows before this point cannot change at current width
```

The second may be earlier than a new append because line wrapping has context.

## 19.6 Native View cache

```text
NodeId → WeakView
```

- weak only;
- environment-local;
- hit stops recursive decode immediately;
- dead entry removed;
- map pruned/bounded by weak cleanup policy;
- no reuse of NodeId for changed semantics.

---

# 20. Required validation policy

The repository/test universe is large. Use narrow tests per commit, wider tests at tranche boundaries, and full suites only at designated checkpoints.

## Per commit

At minimum:

```bash
cargo check -p iyon-tui -p iyon-native
```

plus exact affected tests.

Examples using existing test/module names where applicable:

```bash
cargo test -p iyon-tui history
cargo test -p iyon-tui stream
cargo test -p iyon-tui scene
cargo test -p iyon-tui markdown_incremental
cargo test -p iyon-tui public_semantic_api
```

Use the narrowest filter that actually covers the change.

For N-API changes, run affected TUI Bun/runtime binding tests and a direct addon smoke/conformance test under Bun.

## At every tranche completion

```text
cargo test -p iyon-tui
cargo test -p iyon-native
affected TUI Bun tests
that tranche's performance benchmark
```

## Full checkpoints

Run complete workspace/API-surface/Bun suite:

```text
after PERF-5
after PERF-8
after PERF-10/final
before merge
```

Run the API-surface scanner immediately when actual public Rust/TS surface changes, especially PERF-8 and PERF-9.

Do not run the entire test universe after every tiny representation commit unless repository policy requires it; that encourages agents to avoid testing. Narrow tests should be frequent, full tests deliberate.

---

# 21. Tranche-specific implementation checklists

These are intended to be copied into the implementation agent's scratchpad and checked one by one.

## PERF-0 checklist

- [ ] Record starting git SHA.
- [ ] Add `perf-counters` feature.
- [ ] Add one internal counter registry.
- [ ] Add all required counters.
- [ ] Verify release build without feature has no active counters.
- [ ] Add 20/200/2k/10k generic View builders.
- [ ] Add text/row/column/grid/span/component workloads.
- [ ] Add History 1000 static + live tail fixture.
- [ ] Add stream 1K/10K/50K/100K/500K fixtures.
- [ ] Add four identity/rebuild update modes.
- [ ] Add 256-byte next-chunk stream benchmark.
- [ ] Emit JSONL with SHA and p50/p95/p99/counters.
- [ ] Capture baseline, do not commit result files.

## PERF-1 checklist

- [ ] Introduce `ViewId`.
- [ ] Introduce `ViewFlags`.
- [ ] Wrap semantic node in `Arc`.
- [ ] Share recursive child/span payloads.
- [ ] Route builders through semantic-mutation helper.
- [ ] New ID on mutation even if unique owner.
- [ ] O(1) Clone keeps same ID.
- [ ] `PartialEq` excludes ID.
- [ ] `contains_component_identity()` becomes flag lookup.
- [ ] Add opaque `WeakView`.
- [ ] Test unique-owner mutation identity.
- [ ] Test child sharing.
- [ ] Benchmark 100 vs 10k clone.
- [ ] Stop if clone still scales with subtree.

## PERF-2 checklist

- [ ] Pin missing/duplicate/cycle behavior with tests.
- [ ] Remove semantic `component_scope`.
- [ ] Add cached `ComponentSnapshot` by revision.
- [ ] Invalidate snapshot on every component mutation revision bump.
- [ ] Resolution returns overlay/topology, not rebuilt View.
- [ ] Use ViewFlags to skip component-free branches.
- [ ] Thread current component scope through layout traversal.
- [ ] Put scope on derived layout style data.
- [ ] Preserve focus/focus-within rendering.
- [ ] Verify unrelated component tick does not call cached component `view()`.

## PERF-3 checklist

- [ ] Measured nodes own cheap `View`, no semantic lifetime borrow.
- [ ] Add `MeasureKey(ViewId,width,intent)`.
- [ ] Add SceneHost-owned LayoutCache.
- [ ] Make cache bounded before enabling broadly.
- [ ] Cache text flow metrics.
- [ ] Prove second same-width render does not wrap text.
- [ ] Add prepared-state cache separately.
- [ ] Keep placement uncached.
- [ ] Test one-leaf invalidation.
- [ ] Test focus-only convergence hits.
- [ ] Test cache generations release transient trees.

## PERF-4A checklist

- [ ] Add per-unit presentation metadata.
- [ ] Static cache = ViewId + width.
- [ ] Live cache = ViewId + width + exact component revisions.
- [ ] Capture nested component dependency vector.
- [ ] Replace vector if topology changes.
- [ ] Maintain total flow height by delta.
- [ ] Wire every History mutation.
- [ ] FollowEnd walks backwards viewport-bounded.
- [ ] NativeFrontier walks forwards viewport-bounded.
- [ ] 1000-static stress test shows zero prefix remeasure after warmup.

## PERF-5 checklist

- [ ] Split generic TextStream snapshot into promotable stable node + unstable tail.
- [ ] Confirm UTF-8/grapheme boundary correctness.
- [ ] Confirm source compaction removes resident bytes.
- [ ] Track semantic changed/stable coordinates.
- [ ] Track separate visual restart coordinate.
- [ ] First implementation restarts wrapping at safe hard-line boundary.
- [ ] Retain row anchors before restart.
- [ ] Width change performs full rebuild only once.
- [ ] Native release drops corresponding anchors.
- [ ] Replace HostTextStream per-character Strings with retained chunks/offsets.
- [ ] Stop replaying full ProjectionBuilder history.
- [ ] Remove Thinking/Assistant semantics from `iyon-tui`.
- [ ] Replace TS `"text" | "thinking"` core API with generic stream semantics.
- [ ] 256-byte next-chunk curve no longer grows linearly with transcript.
- [ ] Run full checkpoint.

## PERF-4B checklist

- [ ] Stream cache reports new height/delta.
- [ ] History applies stream delta only.
- [ ] Static/live prefix remains untouched.
- [ ] Prefix retirement subtracts geometry correctly.
- [ ] FollowEnd remains pinned.
- [ ] 1000-static + stream-tail stress test passes.

## PERF-6 checklist

- [ ] Add private monotonic TS NodeId.
- [ ] Counter survives module reload/re-evaluation.
- [ ] Fail before MAX_SAFE_INTEGER reuse.
- [ ] Add numeric hot discriminants.
- [ ] Add schema mismatch guard/test.
- [ ] Add environment-local `NodeId → WeakView` cache.
- [ ] Do not expose `ViewNode` to native.
- [ ] Decoder reads NodeId first.
- [ ] Cache hit returns before kind/children.
- [ ] Cache miss builds final View directly.
- [ ] Expired weak entries removed.
- [ ] Cache cleanup/environment teardown tested under Bun.
- [ ] Remove recursive JsonValue View lowering.
- [ ] Remove mandatory NativeTuiView intermediate.
- [ ] History/slot/pane/render accept semantic View directly.
- [ ] Unicode strings tested.
- [ ] Shared-path 10k benchmark shows cutoff.

## PERF-7 checklist

- [x] Candidate A fixed baseline.
- [x] Candidate B references known NodeIds, not full-tree every time.
- [x] TypedArray borrowed only synchronously.
- [x] Equal cache conditions.
- [x] Measure public construction + JS encode + NAPI + Rust.
- [x] >=20 recorded samples/workload.
- [x] Apply the benchmark-based 5%/15% complexity decision rule.
- [x] Profile JS encoder on SHARED_PATH.
- [x] Compare memory/allocation.
- [x] Delete losing production transport.
- [x] String arena experiment only if packed B wins (not run; Candidate B was rejected).

## PERF-8 checklist

- [x] Inventory every HandleBase.call site.
- [x] Inspect matching native method.
- [x] Mark sync/async explicitly.
- [x] Remove blanket Promise wrapper from sync path.
- [x] Update TuiOperation/public return types.
- [x] Keep real waits async.
- [x] Audit dispose/getters/revision too, not only mutations.
- [x] Update `.then()`-expecting tests/callers.
- [x] Run API-surface scan.
- [x] Run full checkpoint.

## PERF-9 checklist

- [x] Public generic TextStream uses PERF-5 engine.
- [x] One append = one string boundary crossing + suffix update.
- [x] No full-text JS concatenation/rebuild.
- [x] Rust and TS API mirror each other.
- [x] Ordinary small text remains ordinary View.
- [x] TextBuffer was not added because no concrete editable-range need exists.
- [x] No TextBuffer damage-range implementation is needed.

## PERF-10 checklist

- [x] Profile after PERF-9.
- [x] Gate decision: paint was >=15% p95 dirty CPU, so PERF-10 proceeded.
- [x] Identify allocation/compositing work as the cache target.
- [x] Define complete paint dependency key.
- [x] Add theme/focus/inherited-style/viewport correctness tests.
- [x] Bound cache.
- [x] Measure memory and p95 CPU.

## ~~PERF-11 checklist~~ — not applicable

- [ ] Begin only after TUI gate.
- [ ] Queue `CoreEvent`, not `Value`.
- [ ] Convert stable envelope only at JS delivery boundary.
- [ ] Keep arbitrary nested fields arbitrary where genuinely needed.
- [ ] Make pushMany a single native batch critical section.
- [ ] Preserve cancellation/backpressure semantics.
- [ ] Add nextEvents(max).
- [ ] Coalesce only adjacent semantically compatible deltas.
- [ ] Run final full suite/perf gate.

## ~~PERF-12 checklist~~ — moved to `deferred.md`

---

# 22. Common implementation mistakes — reject these in review

## Mistake 1 — “We used `Arc`, therefore clone is cheap”

If `ViewKind` still directly owns a `Vec<View>` and mutation clones that vector/descendants, the architecture is incomplete. Inspect allocation/counter evidence.

## Mistake 2 — keeping the same ID after a unique-owner mutation

This breaks every cache. Semantic identity cannot depend on current reference count.

## Mistake 3 — putting `ViewNode` in `iyon-native`

That leaks semantic-tree internals across crate boundaries. Cache opaque `WeakView`.

## Mistake 4 — resolver returns `ResolvedView` clone tree

You have only renamed the reconstruction. Use an overlay/context.

## Mistake 5 — layout cache is unbounded

A cache that keeps every transient `ViewId` alive is a memory leak disguised as a performance optimization.

## Mistake 6 — History caches only total height but still walks every unit

The viewport selection algorithm itself must become bounded from the relevant end/frontier.

## Mistake 7 — stream restart = append byte offset

Word/grapheme wrapping can alter an earlier part of the same visual hard line. Use a safe visual restart boundary.

## Mistake 8 — stable frontier = visual-row frontier

These are different dependency domains. Track both.

## Mistake 9 — TextStream still snapshots one giant node

Then ResidentPrefix cannot promote a stable prefix when the node crosses into unstable content.

## Mistake 10 — optimize `HostAssistantPipeline` in place

That cements an app-semantic architecture violation. Move assistant semantics out of `iyon-tui` while fixing the generic machinery.

## Mistake 11 — “zero-copy” string encoding in JS

Turning a JS string into a UTF-8 TypedArray costs CPU and allocation too. Count end-to-end time.

## Mistake 12 — packed transaction always serializes all nodes

Then it destroys the very identity benefit we want. Candidate B must reference already-known NodeIds.

## Mistake 13 — native weak cache is actually strong

A strong cache retains every View ever decoded. Use `WeakView` and teardown/pruning tests.

## Mistake 14 — per-process cache without environment cleanup

Workers/realms can come and go. Cache is environment-scoped.

## Mistake 15 — use raw N-API borrowed pointer after return

Invalid lifetime. Consume borrowed TypedArray bytes synchronously.

## Mistake 16 — leave sync methods typed Promise for convenience

PERF-8 exists specifically to remove artificial allocation. Real waits only.

## Mistake 17 — cache paint by ViewId

Paint depends on geometry, inherited style, theme, focus/component context and viewport state.

## Mistake 18 — optimize non-TUI `serde_json` before TUI gate

That broadens scope and makes performance attribution harder. The former PERF-11
native boundary work is explicitly out of scope because the core/API boundary is
moving to TypeScript.

---

# 23. Final TUI mechanical acceptance gate

The TUI refactor is not complete until all of the following are demonstrated with tests/counters/benchmarks.

## Public architecture

```text
Rust iyon-tui is generic, including feature-gated host code.
TypeScript TUI surface mirrors generic Rust semantics.
No assistant/reasoning/tool/provider/application concept in iyon-tui.
Arbitrary semantic View subtrees may change at runtime.
No public mutable semantic View node handles.
```

## View

```text
View::clone() O(1).
Same ViewId means same semantics for identity lifetime.
Every semantic mutation creates a new ViewId.
ViewId excluded from semantic equality.
Changing one shared node does not deep-copy unchanged descendants.
contains-component query is cached/flag-based.
```

## Components/resolution

```text
No recursive ordinary semantic View reconstruction.
component_scope is derived below semantic View.
Unchanged component revision does not rerun view()/capabilities().
Missing/duplicate/cycle behavior unchanged.
Focus/focus-within rendering unchanged.
```

## Layout

```text
Unchanged same-width text is not rewrapped.
One-leaf shared-path update reuses unaffected measurements.
Cache has bounded retention.
Focus/theme-only changes do not cause geometry-wide misses.
```

## History

```text
Static unit measured once per relevant width.
Live unit invalidates by exact component revision dependencies.
Tail updates do not remeasure stable static prefix.
Total resident geometry retained incrementally.
Viewport selection bounded from end/frontier.
```

## Streams

```text
Open generic TextStream can promote/compact stable source prefix.
Stable resident semantic prefix is not reconstructed on append.
Stable visual row anchors are retained.
Visual restart frontier is safe for wrapping context.
Fixed-size next-chunk cost does not grow linearly with transcript size.
No per-character String atom history.
No assistant-specific stream machinery in iyon-tui.
Markdown stream viewport does not freeze during long mid-paragraph/mid-table content.
Spike detection and raw-text bypass activate only when Markdown stable frontier stalls, not during normal newline-terminated streaming.
```

## N-API TUI boundary

```text
No serde_json::Value recursive View transport.
No JSON.stringify/View generic serializer.
Direct decoder builds final Arc-backed View.
Private TS node identity survives as native weak-cache cutoff.
Cache hit does not inspect kind/children.
Weak cache is environment-scoped and collectible.
No mandatory NativeTuiView intermediate.
One semantic N-API mutation call per state operation.
```

## TypeScript synchronization

```text
Synchronous native TUI mutations are synchronous TS methods.
Actual waits remain async.
Public migration documented/tested.
```

## Performance

```text
10k View clone effectively constant order vs 100-node clone.
Second 10k same-width render has near-zero text wrapping work.
1000 static History units are not remeasured for tail updates.
256-byte stream append cost remains roughly flat after warmup.
SHARED_PATH N-API decode stops at reused subtree identity.
Packed transport, if production, met the benchmark-based PERF-7 decision rule.
Paint cache exists only if paint >=15% p95 after prior optimizations.
```

## Memory/lifetime

```text
No retained cache grows without a bounded/lifetime policy.
Dropping semantic owners eventually allows native Views to die.
Worker/environment teardown releases bridge cache.
Stream source compaction actually releases old mutable source bytes.
```

---

# 24. Final implementation-agent operating rules

1. **Work one tranche at a time.** Do not mix unrelated performance tranches in one huge patch.
2. **Before editing a file, reread the current version.** The paths in this handoff were inspected, but your branch may have moved.
3. **Keep behavior changes out of performance tranches** except the deliberate PERF-8 public sync API migration and the genericity cleanup explicitly required by PERF-5.
4. **Add a test before changing a subtle invariant** when current behavior is not already pinned.
5. **Use counters to prove work disappeared.** Timing alone can be noisy.
6. **Do not optimize equality of rebuilt-equivalent trees** with hashing/interning in this project. `REBUILT_EQUIVALENT` may remain proportional; correctness is required, magic sublinear equivalence detection is not.
7. **Do optimize identity-preserving updates.** `IDENTICAL_IDENTITY` and `SHARED_PATH` are where retention must win decisively.
8. **Keep generic framework semantics clean.** If you are about to type `assistant`, `thinking`, `tool`, `provider`, or `agent` inside `crates/iyon-tui`, stop and move that concern upward.
9. **Do not hide memory retention behind `Arc`.** Every long-lived map needs a lifecycle story.
10. **Do not claim completion until the final mechanical gate is recorded.**

---

# 25. Source audit record used to prepare this handoff

## Supplied design documents

Both supplied Markdown documents were read in full. The second document's corrections override conflicting details in the first, especially:

- new `ViewId` on every semantic mutation regardless of Arc uniqueness;
- `ViewId` excluded from `PartialEq`;
- opaque `WeakView` instead of exposing `ViewNode` to native;
- bounded layout caches;
- generic `TextStream` stable-node fix;
- separate semantic and visual stability;
- PERF-8 treated as public API migration;
- execution order split into PERF-4A → PERF-5 → PERF-4B.

## Iyon current repository

Detailed inspection included the semantic View, scene resolution, component registry, layout measurement/prepare/place/tree/host, History model/unit/projection/stream, stream model/resident/node/append/compile/viewport, feature-gated host stream, paint path, native TUI bridge, TS View IR/materialization/handles/runtime/history/stream/scroll/text input/component paths, and deferred core/events/model-turn native boundary.

## Bun

Current source reviewed at:

```text
079cb0a6a8f02229eb16d03b297b3a8984177c29
src/jsc/bindings/napi.cpp
src/runtime/napi/napi_body.rs
```

Relevant current tests/recent lifetime/compatibility work were checked rather than relying only on Bun's documentation.

## napi-rs used by Iyon

Iyon pins `napi = 3.12.1` through the workspace. The matching napi-rs 3.12.1 source was checked, especially:

```text
crates/napi/Cargo.toml
crates/napi/src/env.rs
crates/napi/src/cleanup_env.rs
```

`Env::add_env_cleanup_hook` is available with the default feature chain and is the prescribed lifetime hook for the per-environment View decoder side table. Do not make the implementation agent invent a competing raw instance-data ownership scheme.

## Flutter

```text
bb8568060d4f8882b4c069f764b74b2ff92993b0
packages/flutter/lib/src/widgets/framework.dart
packages/flutter/lib/src/rendering/object.dart
```

## React

```text
eb8feb71096eec5c885b2a4c7d8d030d3622f265
packages/react-reconciler/src/ReactFiber.js
```

## React Native Fabric

```text
4bf55754905dfdcd6460867dca9ad45bfb9fae45
packages/react-native/ReactCommon/react/renderer/core/ShadowNode.cpp
```

## OpenTUI

```text
4067477dd89b554641753dcfbc5e506f61bdd52f
packages/core/README.md
packages/core/src/
```

---

# 26. One-page mental model

If you remember nothing else, remember this chain:

```text
TS View object reused
    ↓ keep private NodeId
native decoder sees NodeId
    ↓ WeakView hit
Rust semantic subtree reused
    ↓ keep ViewId
resolver does not rebuild semantic node
    ↓ overlay only
layout sees same ViewId + width
    ↓ measurement hit
History sees same unit/dependencies
    ↓ height hit
stream sees stable prefix + safe visual restart
    ↓ old anchors reused
paint receives correct derived style/geometry
    ↓ render only genuinely dirty frame work
```

And the genericity rule:

```text
The retention machinery must not know WHY content changed.
```

A compiler log, file tail, Markdown stream, chat transcript, dashboard, or ordinary component update should all use the same generic primitives.
