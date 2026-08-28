# PERF-13 — Three-Plane Retained Runtime

**Repository:** `alexykn/iyon-tui`  
**Baseline inspected:** `api-h1` at `dc928ba1dc3c209ebadc1c2aa25398275f726c1b`  
**Required sequence:** `PERF-12 → API-H1 → API-H2 / STRUCT-1 → API-H3 → PERF-13`  
**Document type:** normative architecture and implementation handoff  
**Audience:** implementation agent with competent Rust/TypeScript skills but no assumed prior knowledge of retained UI internals  
**Delivery model:** stacked, individually reviewable merge requests; do not merge any tranche to the protected integration branch until every PERF-13 tranche is complete and the final gates pass

---

## 0. Executive directive

PERF-13 has one architectural thesis:

> **The retained semantic View DAG stops being the universal carrier for structure, mutable runtime state, and content.**

The runtime must establish three distinct planes:

```text
                         TYPESCRIPT

                 structure / composition
                          │
                          ▼
                ┌────────────────────┐
                │ STRUCTURAL PLANE   │
                │ retained View DAG  │
                └─────────┬──────────┘
                          │ stable semantic identity
                          │
               ┌──────────┴───────────┐
               │                      │
               ▼                      ▼

      RETAINED STATE PLANE        CONTENT PLANE
      ────────────────────        ─────────────

      geometry                    ContentPort
      presentation                    │
      style-state / interaction       │ 0..N Connectors
               │                      │ 0..1 active
               │                   Funnels
               │                      │
               │                   Sources
               │                      │
               └──────────┬───────────┘
                          ▼

                 ┌──────────────────┐
                 │   RUST RUNTIME   │
                 │                  │
                 │ measurement      │
                 │ layout           │
                 │ projection       │
                 │ paint            │
                 │ damage           │
                 │ terminal backend │
                 └──────────────────┘
```

The governing rules are:

1. **If structural topology and retained attachment identity did not change, the structural View DAG should normally not change.**
2. **TypeScript communicates semantic intent; Rust classifies and executes the consequences.**
3. **A semantic `ViewId`/`NodeId` is not a mutable occurrence address.** A shared View may occur more than once. Dynamic state and content therefore require explicit opaque attachment identities.
4. **State and content mutation do not execute composition scopes.** They bypass `defineView`, semantic child ownership, and structural lowering.
5. **The committed frame remains authoritative until a complete replacement frame has been validated, prepared, and committed.**
6. **Bulk content bytes use one mandatory fast data plane.** N-API remains the lifecycle/control plane; it is not a permanent second payload architecture.
7. **PERF-13 implements cold connector standby only.** It does not implement buffered or hot inactive connectors, arbitration, priority, preemption, or automatic fallback scheduling.

The scope correction is precise:

```text
PERF-13 includes:
    structural plane
    retained mutable state plane
    retained content plane
    multiple connectors per port
    explicit manual activation
    cold inactive semantics

PERF-13 excludes:
    buffered inactive delivery
    hot inactive projection/layout
    automatic arbitration
    priority/preemption/yield
    Kitty/Sixel/video/live surfaces
    property-level reactive bindings
```

### 0.1 Required precondition: API-H3

PERF-13 must not start until API-H3 has established this invariant:

> **Composition owns semantic retention. Structural transport owns physical/native retention.**

After API-H3:

```text
composition/
    semantic View identity
    scope execution
    State dependency tracking
    child occurrence ownership
    semantic subtree reuse
          │
          │ narrow structural publication seam
          ▼
transport/structural/
    NativeRef mapping
    leases
    bridge records
    generated structural ABI
    materialization
    N-API/direct structural lowering
```

Composition must not import or name:

```text
NativeRef
bridge IR records
View ABI calls
materialization policies
transport generations
state-plane operations
content-plane operations
```

State and content APIs must route from their runtime/API owners directly to their respective transports. Composition must never become the dispatcher for all native work.

If API-H3 leaves composition importing structural transport implementation modules, stop. Do not use PERF-13 to hide that debt behind new abstractions.

### 0.2 What “done” means

PERF-13 is complete only when all of the following are true:

- A paint-only state change performs no semantic View construction, no new `NodeId`, no composition scope execution, and no structural publication.
- A geometry state change performs no semantic View reconstruction and propagates native work only through the required dependency frontier.
- High-frequency text append/replace performs no View construction and sends bytes through the content data ABI, not the View ABI.
- A port can retain several connectors and switch explicitly between them without changing the structural root or rerunning parent composition.
- Inactive cold connectors perform no projection, wrapping, layout, paint, or connector-local buffering.
- A failed state commit or connector activation leaves the previous committed frame and previous active connector visible.
- Existing PERF-12 structural identity, direct/N-API parity, leases, retry behavior, and performance gates do not regress.
- Old high-volume View/text payload paths are deleted after migration; there is one structural architecture and one content architecture.

---

## 1. Normative language and decision policy

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

This handoff deliberately resolves the architectural questions that were previously open. An implementation may change an internal representation only when all of these conditions hold:

1. the public and cross-plane semantics in this document remain unchanged;
2. the replacement is documented in the tranche completion report;
3. differential correctness tests pass;
4. the replacement does not broaden tranche scope;
5. the replacement is approved during review before later tranches depend on it.

Public type and method names in examples are **normative working names**. A naming-only API review may adjust spelling before the first public PERF-13 tranche is finalized, but it must not alter identity, ownership, lifecycle, scheduling, or failure semantics.

No `TODO`, unspecified fallback, or “implement whichever is easiest” decision is acceptable in the completed implementation.

---

## 2. Baseline reality at `api-h1`

The design must be implemented against the code that exists, not against an imagined clean architecture.

### 2.1 TypeScript View currently crosses too many layers

At the inspected baseline, [`values/view.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/values/view.ts) imports or participates in:

```text
semantic View construction
bridge IR records
NodeId/native path metadata
retained composition helpers
style lowering
PersistentSeq structural derivations
native fast-path decisions
```

That coupling is why API-H2 makes ownership visible and API-H3 creates the semantic publication seam. PERF-13 must not add state/content lowering to the same module.

### 2.2 Semantic execution is already transactional

[`execution.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/execution.ts) already provides important correctness patterns:

```text
prepare all publications
commit once
abort without publishing partial roots
restore retry obligations
microtask-coalesce State invalidations
keep execution-scope identity distinct from NodeId/NativeRef
```

PERF-13 should reuse the model—authoritative committed state plus staged work—but state/content mutation must not be implemented as fake composition scopes.

### 2.3 Semantic and physical retention are currently adjacent

[`retained_dag.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/retained_dag.ts) owns JS semantic-node-to-`NativeRef` correspondence, generation-scoped hints, leases, transaction-local materialization, and cold recovery. [`native_view_abi.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/native_view_abi.ts) owns generated structural calls and retained structural edit transactions.

Those are physical/native-retention responsibilities. API-H3 must isolate them before PERF-13 adds independent state and content transports.

### 2.4 A semantic View is not a unique mounted occurrence

The Rust presentation IR stores immutable semantic Views behind `Arc<ViewNode>`, with a semantic `ViewId`. Layout expands those semantic values into occurrence-specific `LayoutNode`s with:

```text
rect
content_rect
clip_rect
parent
children
component association
style context
```

The same semantic View can appear at more than one occurrence. Therefore:

> **Never target mutable state by `ViewId`, TypeScript `NodeId`, or `NativeRef`.**

`ViewId` remains useful as an immutable semantic/cache identity. It is not sufficient to identify “the box currently at row 7, column 12 under this parent.”

### 2.5 Current View properties are immutable semantic fields

The baseline Rust `ViewNode` contains immutable fields such as:

```text
width/height rule
decoration
style states/facts
ViewKind
```

`Decoration` currently includes padding, bounds, surface background, border, and text style. Fluent changes construct a new semantic View identity. PERF-13 migrates selected properties to retained native state without claiming every property is mutable in the first release.

### 2.6 The current border model is one terminal-cell edge, not arbitrary thickness

The current TypeScript [`BorderSpec`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/types.ts) has glyph/style/edge/color semantics and no border-width property. PERF-13 must not invent `borderWidth = 3` as a shipped API merely because it is a useful conceptual example.

For PERF-13:

```text
border absent       → no border inset
border edge present → exactly one terminal cell on that edge
```

Arbitrary terminal border thickness is future work.

### 2.7 The native stream subsystem is not greenfield

The existing Rust stream modules already provide:

- width-independent snapshots;
- monotonic source revisions and absolute UTF-8 byte coordinates;
- stable frontiers;
- source compaction validation;
- projected semantic text;
- width-specific compilation and row indexing;
- pure History transfer planning;
- follow-end/detached viewport anchors.

Relevant baseline modules include:

- [`stream/model.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/model.rs)
- [`stream/source.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/source.rs)
- [`stream/projected.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/projected.rs)
- [`stream/snapshot.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/snapshot.rs)
- [`stream/transfer.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/transfer.rs)
- [`stream/pane`](https://github.com/alexykn/iyon-tui/tree/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/pane)

PERF-13 must refactor and reuse this machinery. It must not replace it with an unindexed `String` plus repeated full wrapping.

### 2.8 The host already has retained local invalidation machinery

The current [`SceneHost`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/scene/host.rs) retains:

```text
stable resolved scene
layout cache
paint cache
last surface
mount graph
focus/capabilities
component-local invalidation sets
```

It already attempts same-shape component subtree patching, local paint into the retained surface, and full-layout fallback when topology or geometry changes. PERF-13 should generalize these ideas from “component changed” to “attached node state/content changed.” It must not create a parallel renderer.

### 2.9 Current stream mutation renders too eagerly

The baseline native host updates a `HostTextStream`, invalidates the frame, and calls `advance_and_render()` from each mutation. That is correct but defeats high-frequency append coalescing.

PERF-13 must change the scheduling contract so that:

```text
append/update/state mutation
    → mutate or enqueue cheap native semantic state
    → advance one pending-work epoch
    → schedule one host flush
    → project/layout/paint once for the latest state
```

### 2.10 Existing performance counters are the starting point

[`perf.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/perf.rs) already measures View construction/copying, N-API cache behavior, resolve, measure, prepare, layout emission, paint, compositing, History, stream reindex/reuse, and PersistentSeq work.

PERF-13 must extend this seam rather than invent an unrelated benchmark reporting format.

---

## 3. Research synthesis: patterns to adopt and patterns not to copy

PERF-13 borrows proven separation principles from other domains. It does not attempt to clone those systems wholesale.

### 3.1 Retained UI engines: dirty effects belong to the renderer

Flutter’s [`markNeedsLayout`](https://api.flutter.dev/flutter/rendering/RenderObject/markNeedsLayout.html) marks layout dirty, propagates to a relayout boundary or parent according to dependency, schedules a visual update, and coalesces sequential writes. [`markNeedsPaint`](https://api.flutter.dev/flutter/rendering/RenderObject/markNeedsPaint.html) separately schedules paint and can stop at repaint boundaries. Flutter also records when intrinsic/baseline queries create dependencies that cross ordinary boundaries.

Qt Quick’s [`QSGNode::DirtyStateBit`](https://doc.qt.io/qt-6/qsgnode.html) distinguishes matrix, node-add/remove, geometry, material, opacity, and other dirty categories on a retained render graph.

Adopt:

```text
semantic property mutation
    → native effect classification
    → typed dirty flags
    → dependency-aware propagation
    → batched frame processing
```

Do not copy:

```text
Flutter's object hierarchy
Qt's rendering-thread API surface
GPU-specific layer machinery
```

### 3.2 Compositors/display stacks: pending state must commit atomically

Wayland `wl_surface` requests modify pending state. [`wl_surface.commit`](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_surface) makes buffer, damage, input/opaque region, and related state current together. Pending damage is accumulated as a union and only interpreted at commit.

Linux DRM/KMS performs an atomic check before commit and requires failure-prone preparation before the visible hardware flip. See the current [DRM KMS atomic documentation](https://docs.kernel.org/gpu/drm-kms.html).

Adopt:

```text
current committed runtime state
pending mutation batch
validate/prepare off to the side
atomic swap on success
old frame remains authoritative on failure
old connector remains active until candidate is ready
```

Do not copy:

```text
buffer-release protocol details
GPU fences/page-flip semantics
subsurface synchronization rules
```

### 3.3 Media graphs: contract, link, policy, and bytes are different things

GStreamer separates media-format negotiation from allocation negotiation. Its [caps negotiation](https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/negotiation.html) uses explicit capabilities, acceptance checks, fixed/transform/dynamic cases, and reconfiguration. Its [buffer-pool design](https://gstreamer.freedesktop.org/documentation/additional/design/bufferpool.html) negotiates allocation only after format compatibility.

PipeWire exposes nodes, ports, and links as separate graph objects, while its session manager owns higher-level routing policy. Its [overview](https://docs.pipewire.org/page_overview.html) also distinguishes passive links that do not make a graph active. Port implementation states move through creation, configuration, readiness, and paused phases.

Adopt:

```text
Source            authoritative semantic data
Funnel            typed transformation/delivery contract
Connector         one retained link with attachment-local state
ContentPort       one structural receiving region
manual activation policy outside the data object
semantic/backend/geometry checks at distinct phases
```

Do not copy:

```text
a generic multimedia pipeline scheduler
arbitrary graph cycles
buffer-pool negotiation for text in PERF-13
background inactive processing
```

### 3.4 SIP/SDP/RTP: control, description, and payload may share a feature but not a transport

SIP establishes/modifies sessions; SDP describes and negotiates media parameters; RTP transports media. [RFC 3261](https://www.rfc-editor.org/rfc/rfc3261), [RFC 3264](https://www.rfc-editor.org/rfc/rfc3264), and [RFC 3550](https://www.rfc-editor.org/rfc/rfc3550) make those roles explicit.

The transferable lesson is not SIP message syntax. It is this separation:

```text
N-API control/lifecycle
    ≠
Funnel/Port compatibility description
    ≠
fast content payload transport
```

### 3.5 ECS/game engines: stale identities need generations; writes should not imply real change

Bevy entities use an index plus generation so a freed slot does not make an old identifier valid for a new entity. Its [entity lifecycle documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/entity/index.html) explicitly bumps generations when slots are freed. Bevy change detection also demonstrates both the usefulness and the danger of mutation ticks: a mutable dereference may mark changed even when the value is equal, while helpers such as `set_if_neq` avoid redundant work.

Adopt:

```text
typed generational handles
fallible stale-handle validation
equality check before advancing property revisions
monotonic dirty/revision generations
```

Do not copy:

```text
a complete ECS
archetype storage
system scheduling
implicit “any mutable borrow means changed” semantics
```

### 3.6 Native transports: stable control, isolated experimental fast lane

Node-API is documented as ABI-stable and provides environment-scoped instance data and teardown. See [Node-API](https://nodejs.org/api/n-api.html). Bun FFI can pass TypedArray-backed pointers, but Bun warns that raw pointers are unsafe and its FFI surface must be treated as a platform-specific capability. See [Bun FFI](https://bun.sh/docs/runtime/ffi).

Adopt:

```text
Node-API for opaque objects, lifecycle, configuration, errors, and queries
one isolated direct-FFI adapter for bulk bytes
synchronous copy into Rust-owned memory before FFI return
startup ABI/version probes
no raw pointer retained across the call
```

The FFI lane is mandatory for production content payloads, but it must be isolated behind one module and gated on the exact runtime/platform matrix already supported by the project.

---

## 4. Architectural ownership after API-H2 and API-H3

### 4.1 TypeScript ownership

The expected ownership is:

```text
src/
├── api/
│   ├── view/
│   │   ├── ... existing structural semantics
│   │   └── retained-state.ts       # public ViewState semantics
│   ├── presentation/
│   ├── content/
│   │   └── retained-content.ts     # Sources, Funnels, Ports, Connectors
│   └── controls/
│
├── composition/
│   └── ... semantic retained execution only
│
├── runtime/
│   ├── ... Tui host/lifetime
│   ├── retained-handles.ts         # host binding and disposal orchestration
│   └── flush-scheduler.ts          # one host wake/flush scheduler
│
├── transport/
│   ├── structural/
│   ├── state/
│   │   └── control.ts              # N-API state mutation/lifecycle
│   ├── content/
│   │   ├── control.ts              # N-API content lifecycle/activation
│   │   ├── data.ts                 # semantic payload facade
│   │   └── ffi.ts                  # the only Bun FFI implementation
│   ├── native/
│   └── abi/
│       ├── structural/
│       ├── state/                  # only if generated schema adds value
│       └── content/                # control/data definitions may differ
│
└── testing/
```

This is responsibility guidance, not a demand for one file per symbol. Keep cohesive families together.

Import direction:

```text
api/state/content
      │
      ▼
runtime handle/lifetime facade
      │
      ▼
transport/state or transport/content
      │
      ▼
Rust

composition
      │
      ▼
structural publication interface
      │
      ▼
transport/structural
```

Forbidden:

```text
composition → transport/state
composition → transport/content
api/content → transport/structural
state/content APIs → defineView execution
public declarations → generated ABI records
```

### 4.2 Rust ownership

A reasonable coarse target is:

```text
crates/iyon-tui/src/
├── retained_state/
│   ├── mod.rs
│   ├── arena.rs
│   ├── property.rs
│   ├── effect.rs
│   ├── dirty.rs
│   ├── damage.rs
│   └── transaction.rs
│
├── content/
│   ├── mod.rs
│   ├── registry.rs
│   ├── capability.rs
│   ├── port.rs
│   ├── connector.rs
│   ├── funnel.rs
│   └── text/
│       ├── source.rs
│       ├── storage.rs
│       ├── projection.rs
│       └── retention.rs
│
├── stream/              # retained during migration; folded/adapted deliberately
├── presentation/
├── scene/
├── application/
└── physical/
```

Do not move every existing stream file merely to match the diagram. Move or rename code only when ownership genuinely changes.

### 4.3 Current-to-target mapping

| Current subsystem | PERF-13 role |
|---|---|
| `presentation::ir::ViewNode/ViewKind` | structural specification plus immutable initial state values |
| `presentation::layout::LayoutTree` | committed occurrence geometry and parent/dependency metadata |
| `scene::SceneHost` | host integration, retained frame, dirty processing, commit point |
| `presentation::paint` | paint resolution, local repaint, state/content-aware cache keys |
| `physical::Surface` | retained physical frame and rectangle clearing/compositing |
| `stream::StreamingSource/StreamSnapshot/StreamModel` | basis of authoritative text Source and semantic frontier rules |
| `stream::projected/compile/pane` | Funnel projection and Connector-local width/viewport work |
| `history::stream/transfer` | History adapter; not the generic connector scheduler |
| native `TuiHost`/`HostInner` | runtime generation, pending-work epochs, flush/commit ownership |
| generated View ABI | structural plane only |

---

## 5. Identity model

### 5.1 Identity vocabulary

The implementation must use these terms consistently:

| Identity | Meaning | Mutable target? | Public? |
|---|---|---:|---:|
| `SemanticNodeId` / current TS `NodeId` | immutable semantic View identity | no | no |
| Rust `ViewId` | immutable semantic/cache identity | no | no |
| `NativeRef` | leased native structural object reference | no | no |
| `LayoutNodeId` | occurrence in one committed layout tree | internally, only while that tree is current | no |
| `ViewStateId` | opaque retained mutation attachment | yes | wrapped by `ViewState` |
| `ContentPortId` | opaque structural content-host attachment | yes | wrapped by `ContentPort<T>` |
| `SourceId` | authoritative content producer record | yes | wrapped by source class |
| `ConnectorId` | one Source/Funnel/Port link | yes | wrapped by connector class |
| runtime generation | identifies one live native environment/host generation | validation only | no |

### 5.2 Why `ViewId` cannot address state

This is invalid:

```text
state.set_background(view_id = 42)
```

because semantic View 42 may be mounted in two places with different rectangles, parents, clips, inherited styles, and content widths.

The stateful form is:

```text
ViewStateId 7
    structurally attached to exactly one committed occurrence
    resolves to the current LayoutNodeId for that occurrence
```

### 5.3 Handle representation

Do not pass native pointers or JS-safe-integer-packed universal IDs.

Across the content FFI boundary, use explicit lanes:

```c
runtime_slot:       uint32_t
runtime_generation: uint32_t
object_slot:        uint32_t
object_generation:  uint32_t
```

The function name supplies the object kind. A stale generation returns a stable error code. If a generation counter would wrap, retire the slot permanently rather than making an ancient handle valid again.

N-API may wrap the same lanes in opaque class instances. Public TypeScript must never expose the lanes.

### 5.4 Mount uniqueness

Normative invariants:

- One `ViewState` may be attached to at most one committed occurrence at a time.
- One `ContentPort` may be attached to at most one committed `ContentHost` occurrence at a time.
- A structural node may have at most one `ViewState` and at most one `ContentPort` attachment.
- A `ViewState` or `ContentPort` may be unmounted and later remounted.
- Moving one attachment from an old occurrence to one new occurrence in the same structural transaction is legal.
- Duplicating an attachment in the candidate structural graph is a pre-commit error. The previous graph/frame remains committed.

The duplicate error must report the attachment kind and both candidate occurrence paths. Do not silently choose one.

---

## 6. Structural plane: normative boundary

### 6.1 Structural means identity/topology/algorithm selection

A field is structural in PERF-13 when changing it changes one of:

```text
node kind / layout algorithm family
parent-child relationship
child order or membership
edge participation identity
structural boundary existence
retained attachment identity
component identity
```

Structural changes continue through the PERF-12 path:

```text
defineView / retained semantic execution
        ↓
changed semantic frontier
        ↓
API-H3 structural publication seam
        ↓
transport/structural
        ↓
Rust retained structural graph
```

### 6.2 Structural in PERF-13 v1

The following remain structural:

| Concept | Reason |
|---|---|
| `Row`, `Column`, `Grid`, `Container`, `Hanging`, `ClampRows`, `RowViewport`, `ComponentSlot`, `ContentHost` kind | selects a different layout/interaction algorithm |
| parent/child insertion, removal, replacement, reordering | topology |
| axis child `TrackSize` participation (`content`, fixed, flex, content-max) | parent-child edge contract; not a scalar property of only one node |
| grid track count/order/definitions | defines the grid coordinate system and edge relationships |
| grid cell row/column/span/placement | structural relationship to tracks |
| component attachment | retained component identity and host graph |
| `ContentPort` attachment | retained content identity belongs to one structural region |
| container/viewport/clamp/scroll boundary existence | changes clipping/layout/interaction topology |
| clipping-boundary existence | changes ancestor/descendant paint relationship |
| overlay/stack relationship when introduced | topology |

### 6.3 Mutable geometry state in PERF-13

The first geometry-state tranche includes only scalar/local values whose current semantics can be preserved:

```text
width rule: fit / fill
height rule: fit / fill
padding
min/max width
min/max height
row/column/grid scalar gap
simple alignment owned by the current node
border edge presence under the current one-cell model
```

Caveat: some current builder APIs encode alignment or participation on the parent-child edge. Those remain structural until there is a dedicated retained edge-state design. Do not force an edge property into node state.

### 6.4 Mutable presentation state in PERF-13

```text
surface background
border color
glyph/style choice for an already-present border
text foreground/background
bold/dim/italic/underline/reverse/strikethrough
StyleRef / sparse StyleSpec
semantic style-state key/value overrides
```

Changing border edge presence is geometry-affecting because it changes the one-cell inset. Changing glyphs or color while the same edges remain is paint-only.

### 6.5 Interaction/runtime state in PERF-13

Interaction state has distinct owners:

| State | Owner |
|---|---|
| focus/focus-within | Rust host focus manager |
| selected/disabled/active/error/running and application-defined style keys | `ViewState` style-state overrides or control-specific native state |
| scroll offset/follow-end | `ScrollPane`/viewport runtime state |
| connector active/inactive/status | content plane |
| future animation phase | future native clock/state producer; architecture only |

Do not create one universal “interaction map” that lets callers mutate focus, scroll, connector status, and application style keys interchangeably.

### 6.6 Content policy is not geometry state

For new dynamic content:

```text
wrap mode
text projector/renderer
smoothing/pacing policy
annotation interpretation
```

belong to a Funnel/Connector contract, not to generic node geometry.

Existing static `View.text(...).wrap(...)` semantics may remain in the structural View representation during PERF-13. Do not force migration of all static text merely to claim purity.

### 6.7 Classification table for current concepts

| Current concept | PERF-13 owner | First release behavior |
|---|---|---|
| View kind | structural | unchanged |
| child list/order | structural | unchanged |
| axis child track | structural edge | unchanged |
| grid tracks/cell spans | structural | unchanged |
| `container()` existence | structural | unchanged |
| clamp/row viewport existence | structural | unchanged |
| padding | geometry state | mutable when a `ViewState` is attached; static fluent value remains initial |
| bounds | geometry state | mutable with native consequence classification |
| fit/fill | geometry state | mutable where represented on the node; edge participation remains structural |
| gap | geometry state | migrate scalar parent gap |
| border edges | geometry + paint | one-cell inset; no arbitrary width |
| border glyph/style/color | presentation | paint-only when edges unchanged |
| surface background | presentation | paint-only |
| text style / StyleRef | presentation | paint-only |
| style-state keys | presentation/interaction | native style resolution |
| focus/focus-within | host interaction | native style resolution |
| `ContentPort` attachment | structural | new `ContentHost` kind |
| connector membership | content retained state | never View topology |
| active connector | content retained state | explicit transactional switch |
| source bytes/revision | content source | never View topology |
| scroll offset | control runtime state | preserved across content switches |

---

## 7. Explicit terminal box model

PERF-13 cannot safely migrate geometry properties without a box model. The following model is normative.

### 7.1 Every layout occurrence has a box; not every node is automatically a general-purpose painter

Every emitted layout occurrence has:

```text
border_rect   = LayoutNode.rect
content_rect  = rect after border insets and padding
clip_rect     = effective intersection with structural clip boundaries
```

This does **not** mean every semantic View kind gains every decoration method. Public paint capability remains explicit and compatible with current APIs.

### 7.2 Box order

For each decorated box:

```text
allocated border box
    └── one-cell border edges, if present
          └── padding
                └── content box
```

Intrinsic size is:

```text
content intrinsic size
+ padding insets
+ present border-edge insets
then clamped by min/max bounds
```

### 7.3 Border semantics

- A present top/bottom/left/right edge consumes exactly one terminal cell on that side.
- `topBottom` consumes only top and bottom rows under the current `BorderSpec` semantics.
- Border color/glyph/style changes are paint-only if edge presence is unchanged.
- Edge presence changes require measurement/placement and old-plus-new damage.
- PERF-13 does not add multi-cell border thickness.
- Existing custom-glyph validation and Unicode rendering semantics must be preserved. Do not silently truncate a wide grapheme to force one-cell behavior.

### 7.4 Padding

Padding lies inside the border and outside content.

Changing padding:

```text
changes child constraints
changes intrinsic size
may change ancestor size
may move descendants
must damage old and new occupied rectangles
```

### 7.5 Fill/fit

`fillWidth()` and `fillHeight()` refer to the allocated **border box**.

The content box is the remaining space after border and padding. This makes fill semantics independent of decoration.

### 7.6 Background

A node surface background fills the box interior, including padding and otherwise-unpainted content cells. Border glyph cells are painted by the border renderer; their foreground/background comes from resolved border/text style semantics.

A child with transparent cells reveals the nearest retained ancestor background, matching the current incremental `clear_rect_with_background` behavior.

### 7.7 Gap

An axis/grid gap separates adjacent child border boxes. It is not padding on either child.

Changing gap is a mutation of the parent’s geometry state and usually requires:

```text
parent intrinsic remeasure
child placement
old + new child region damage
```

### 7.8 Clipping

The **existence** of a clipping/viewport boundary is structural. The derived clip rectangle is geometry.

Changing padding, bounds, placement, or viewport size may update the clip rectangle without changing topology.

### 7.9 Container remains structural

`container()` remains a real structural boundary in PERF-13. It may own box decoration and constraints, but it is not erased simply because some decorations become state fields.

A future simplification may prove that a particular wrapper is semantically redundant. That proof must be differential, not aesthetic.

### 7.10 `Decorated` migration rule

Do not begin PERF-13 by deleting `Decorated`.

For each current decoration wrapper:

1. characterize its layout, inheritance, clipping, identity, and paint semantics;
2. add the equivalent initial-state representation;
3. render old and new paths differentially;
4. collapse only wrappers with one-to-one semantics;
5. retain a structural box when removing it would merge allocation, inheritance, clipping, or background boundaries.

The goal is:

> **Decoration does not automatically require another structural node.**

The goal is not:

> **No decoration may ever be structural.**

### 7.11 ContentHost box semantics

`View.content(port)` creates a structural `ContentHost` leaf.

- With no active connector, its content intrinsic size is `0 × 0`; border/padding/bounds still apply.
- With an active connector, intrinsic size comes from that connector’s current projection at the offered width/constraints.
- During a candidate activation, the old active connector’s projection and size remain committed until the candidate is ready.
- A `Container` may wrap a `ContentHost`; a spacer cannot host content.

---

## 8. Public retained-state model

### 8.1 Decision: one opaque `ViewState`, not raw NodeIds or shareable geometry objects

The public working API is:

```ts
const paneState = tui.viewState();

const pane = View.container(child)
  .state(paneState)
  .padding(1)
  .background(theme.surface);

paneState.setPresentation({
  background: ColorSpec.named("red"),
});

paneState.setGeometry({
  padding: Insets.all(2),
});

paneState.setStyleState("status", "error");
```

`ViewState` is:

- an opaque, Tui-bound, generational handle;
- structurally attached to one occurrence through `.state(...)`;
- logically the mutable state of that occurrence;
- physically allowed to use a host arena record for lifetime, pending values, and mount binding;
- not shareable across two mounted occurrences;
- not a public `NodeId` wrapper;
- not a property-level reactive cell system.

A physical `ViewStateRecord` does not make geometry/presentation a second semantic graph. It is an addressing/lifetime record for one node-owned state bundle.

### 8.2 Initial values and dynamic overrides

Fluent View modifiers remain ergonomic immutable **initial/base values**:

```ts
View.container(child)
  .padding(1)
  .background(theme.surface)
```

When a `ViewState` is attached, effective state is resolved as:

```text
immutable View base values
    + sparse ViewState overrides
    + host interaction facts (focus/focus-within)
    + Theme resolution
```

Rules:

- A state mutation before mount records an override and is applied on first successful mount.
- A state mutation after mount updates the same retained occurrence.
- Re-publishing a structurally new View with the same `ViewState` may change base values; existing explicit overrides remain authoritative.
- Clearing an override returns that property to the current View base value.
- Inline modifiers are not magically reactive. Reconstructing Views to vary them may still use the structural path. Performance-sensitive dynamic values must use `ViewState`.

Working methods:

```ts
state.setGeometry(patch)
state.clearGeometry(...keys)
state.setPresentation(patch)
state.clearPresentation(...keys)
state.setStyleState(key, value)
state.clearStyleState(key)
state.dispose()
```

Patch calls are typed and atomic at the call level. Do not expose `setProperty(name: string, value: unknown)`.

### 8.3 Supported geometry patch in PERF-13

```ts
interface GeometryPatch {
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly padding?: InsetsValue;
  readonly minWidth?: number | null;
  readonly maxWidth?: number | null;
  readonly minHeight?: number | null;
  readonly maxHeight?: number | null;
  readonly gap?: number;
  readonly alignment?: AlignmentValue;
  readonly borderEdges?: BorderEdgesValue | null;
}
```

Only expose a field when the current node kind supports it. Invalid node/property combinations are typed or attach-time errors; they are not ignored.

### 8.4 Supported presentation patch in PERF-13

```ts
interface PresentationPatch {
  readonly foreground?: ColorSpec | null;
  readonly background?: ColorSpec | null;
  readonly borderColor?: ColorSpec | null;
  readonly borderStyle?: BorderStyle | null;
  readonly borderGlyphs?: BorderGlyphs | null;
  readonly textAttributes?: TextAttributesPatch;
  readonly style?: StyleRef | StyleSpec | null;
}
```

### 8.5 Style-state semantics

Application semantic states remain key/value pairs compatible with API-H1 selectors:

```ts
state.setStyleState("status", "running");
state.setStyleState("severity", "error");
state.clearStyleState("status");
```

The effective node state is:

```text
base immutable StyleStates from View
    overridden/extended by ViewState states
    combined with host facts such as focused/focus-within
```

Theme selectors in PERF-13 are presentation-only. A theme rule must not change padding, bounds, tracks, or other geometry. Geometry-affecting theme rules are rejected/deferred.

### 8.6 Mount and remount behavior

Lifecycle:

```text
created → unmounted → mounted → unmounted → ... → disposed
```

- Unmounted state retains overrides.
- Remount may target a different compatible View kind.
- Incompatible stored overrides cause candidate-mount validation failure before commit.
- A move from one occurrence to another in one structural transaction retains overrides.
- A duplicate candidate mount fails the structural transaction.

### 8.7 Disposal

- `dispose()` is idempotent when already disposed.
- Disposing a mounted `ViewState` is an error.
- Disposing an unmounted state invalidates its generation and clears pending overrides.
- Tui shutdown invalidates all Tui-bound `ViewState` handles.
- Finalizers are best-effort cleanup only; correctness must not depend on GC timing.

### 8.8 No public property bindings in PERF-13

Do not implement:

```ts
View.box().background(bind(state, ...))
```

Property-level dependency compilation is a separate architecture. Callers can subscribe to their own application state and invoke typed `ViewState` mutations.

---

## 9. Rust retained-state representation

### 9.1 Logical ownership versus physical storage

The state belongs logically to the mounted structural occurrence. A practical physical representation is a host arena:

```rust
struct ViewStateRecord {
    generation: u32,
    lifecycle: StateLifecycle,
    mount: Option<MountedStateBinding>,

    geometry_overrides: GeometryOverrides,
    presentation_overrides: PresentationOverrides,
    style_state_overrides: StyleStateOverrides,

    state_revision: u64,
    geometry_revision: u64,
    presentation_revision: u64,
}

struct MountedStateBinding {
    layout_node: LayoutNodeId,
    structural_generation: u64,
    compatible_kind: StatefulNodeKind,
}
```

The exact fields may differ, but these invariants do not:

- the record is Tui-host-owned;
- one record binds to zero or one committed occurrence;
- the binding is refreshed after successful structural/layout reconciliation;
- an unmounted record may retain overrides;
- cache validation uses relevant revisions, not pointer identity;
- the record cannot be shared as a style object among several nodes.

### 9.2 Base and effective state

The committed layout/presentation node must expose both:

```rust
struct NodeBaseState {
    geometry: GeometryState,
    presentation: PresentationState,
    style_states: StyleStates,
}

struct NodeEffectiveState {
    geometry: GeometryState,
    presentation: PresentationState,
    style_states: StyleStates,
}
```

`NodeBaseState` is derived from the structural View specification. `NodeEffectiveState` applies retained overrides and host interaction facts.

Do not mutate the immutable structural View object in place.

### 9.3 Equality before revision

For every patch:

1. validate the value;
2. normalize it to canonical native form;
3. compare it with the currently stored override/effective value;
4. if equal, record a no-op counter and do not advance revisions or dirty flags;
5. otherwise update the pending record, advance the relevant revision, and classify effects.

This avoids the “mutable access implies changed” false-positive problem.

### 9.4 Property descriptors

Use a finite native property table, not dynamic string dispatch:

```rust
struct PropertyDescriptor {
    id: PropertyId,
    supported_kinds: NodeKindMask,
    baseline_effects: EffectMask,
    validator: fn(&PropertyValue) -> Result<NormalizedPropertyValue, StateError>,
}
```

The TypeScript API lowers typed patches to these stable property IDs through N-API.

### 9.5 Effect model: bit flags, not an exclusive enum

A mutation can require more than one consequence. Use bit flags:

```rust
bitflags! {
    struct EffectMask: u32 {
        const RESOLVE_STYLE            = 1 << 0;
        const PROJECT_CONTENT          = 1 << 1;
        const MEASURE_SELF             = 1 << 2;
        const MEASURE_ANCESTORS        = 1 << 3;
        const PLACE_SELF               = 1 << 4;
        const PLACE_DESCENDANTS        = 1 << 5;
        const UPDATE_CLIP              = 1 << 6;
        const UPDATE_INTERACTION_GRAPH = 1 << 7;
        const PAINT_SELF               = 1 << 8;
        const PAINT_SUBTREE            = 1 << 9;
        const DAMAGE_OLD_RECT          = 1 << 10;
        const DAMAGE_NEW_RECT          = 1 << 11;
        const STRUCTURE                = 1 << 12;
    }
}
```

`STRUCTURE` is an internal guard. A state-plane public operation must never produce it. If a proposed patch would change topology, reject it and require the structural API.

### 9.6 Baseline effect table

| Mutation | Baseline effects |
|---|---|
| foreground/background/text attribute | `RESOLVE_STYLE`, `PAINT_SELF`, `DAMAGE_NEW_RECT` |
| border color/glyph/style, same edges | `RESOLVE_STYLE`, `PAINT_SELF`, `DAMAGE_NEW_RECT` |
| style-state key/value | `RESOLVE_STYLE`, `PAINT_SUBTREE` conservatively, `DAMAGE_NEW_RECT` |
| focus/focus-within | `RESOLVE_STYLE`, paint affected component/style subtree |
| padding | `MEASURE_SELF`, `MEASURE_ANCESTORS`, `PLACE_DESCENDANTS`, `UPDATE_CLIP`, old+new damage |
| bounds | `MEASURE_SELF`, `MEASURE_ANCESTORS`, `PLACE_SELF`, `PLACE_DESCENDANTS`, old+new damage |
| width/height fit/fill | measure parent dependency frontier, placement, old+new damage |
| scalar gap | measure the owning parent, place children, old+new damage |
| alignment | placement of affected child/subtree, old+new damage; measure only when current algorithm couples size |
| border edge presence | measurement, placement, clip, paint, old+new damage |
| content revision | `PROJECT_CONTENT`; runtime refines to paint-only or measure/placement after comparing projected metrics |

### 9.7 Runtime refinement

The baseline mask is conservative. Rust may remove unnecessary work after inspecting committed constraints and old/new metrics.

Example:

```text
text append
    active ContentHost has fixed 80×10 border box
    projection height changes from 100 to 101 rows
    viewport remains 10 rows and follows end

result:
    projection/viewport update
    no ancestor measurement
    paint ContentHost rectangle
```

Contrast:

```text
text append
    ContentHost height = fit
    projected height changes 3 → 4

result:
    measure ContentHost
    propagate height dependency to ancestors
    place affected descendants
    damage old + new regions
```

### 9.8 Dependency metadata

Targeted layout requires explicit dependency information. Add dependency bits to committed parent-child layout relationships:

```rust
bitflags! {
    struct ChildDependency: u16 {
        const PARENT_USES_CHILD_WIDTH      = 1 << 0;
        const PARENT_USES_CHILD_HEIGHT     = 1 << 1;
        const PARENT_USES_CHILD_BASELINE   = 1 << 2;
        const PARENT_USES_CHILD_INTRINSICS = 1 << 3;
        const CHILD_WIDTH_DEPENDS_ON_PARENT  = 1 << 4;
        const CHILD_HEIGHT_DEPENDS_ON_PARENT = 1 << 5;
    }
}
```

The layout algorithm that consumes the child must record these bits. Do not infer them later from node kinds in a separate duplicated rules table.

If metadata is absent or an algorithm cannot prove a boundary, propagate conservatively to the root. Incorrect extra work is acceptable during staged implementation; incorrect cutoff is not.

### 9.9 Dirty representation

Use local flags plus generations:

```rust
struct NodeDirty {
    effects: EffectMask,
    width_dirty: bool,
    height_dirty: bool,
    placement_dirty: bool,
    paint_dirty: bool,
    dirty_generation: u64,
}
```

Do not create four unrelated global “structural/geometry/presentation/content revisions” and compare them everywhere. Use:

- host `pending_epoch` / `committed_epoch` for frame scheduling;
- local record revisions for cache validation;
- source revision for content data;
- connector projection revision for attachment-specific derived work;
- structural generation for attachment rebinding.

### 9.10 Dirty propagation algorithm

Given one changed mounted occurrence:

```text
1. Seed local effect flags from the property descriptor.
2. If intrinsic width/height may change, mark those dimensions dirty.
3. Walk committed parent links upward.
4. For each edge, propagate only dimensions the parent recorded as dependent on.
5. Stop a dimension at a proven dependency boundary.
6. Continue through intrinsic/baseline dependencies even across ordinary boundaries.
7. Mark the smallest placement roots whose descendants may move.
8. Save old rectangles before any candidate layout writes.
9. If metadata is missing or inconsistent, fall back to full-root layout and increment a fallback counter.
```

Pseudo-code:

```rust
fn propagate_measure_dirty(tree: &LayoutTree, start: LayoutNodeId, mut dims: DirtyDims) {
    let mut child = start;
    while !dims.is_empty() {
        mark_measure(child, dims);
        let Some(parent) = tree.parent(child) else { break };
        let dependency = tree.child_dependency(parent, child);
        dims = dependency.propagate(dims);
        child = parent;
    }
}
```

### 9.11 Measure and placement order

For one candidate frame:

```text
bottom-up:
    project changed content when width/input requires it
    measure dirty leaves/subtrees
    recompute parent intrinsic results along dirty frontier
    stop when output metrics are unchanged and no other effect requires propagation

top-down:
    place the highest dirty placement roots
    update descendant rect/content_rect/clip_rect
    rebind attachment → LayoutNodeId mappings in candidate state
```

If one measure result equals the committed result, upward propagation may stop for that dimension after all local side effects are accounted for.

### 9.12 Cache keys

Existing measurement/paint caches key heavily on immutable `ViewId`. PERF-13 must add the relevant retained revisions:

```text
measure cache:
    semantic ViewId
    effective geometry revision/fingerprint
    active content connector projection metric revision
    constraints
    component revision as applicable

paint cache:
    semantic ViewId
    effective presentation revision/fingerprint
    active content projection paint revision
    rect/content_rect/clip_rect
    inherited/resolved style context
    theme revision
```

Do not put a universal host revision in every key; that would invalidate the whole cache on any mutation.

### 9.13 Candidate/committed state

Host state is double-buffered conceptually:

```rust
struct RetainedRuntime {
    committed: CommittedRuntimeState,
    pending: PendingMutationState,
}
```

A frame flush creates a candidate using copy-on-write or scratch structures for only changed records. It must not mutate committed geometry/active connector selection before validation succeeds.

### 9.14 Frame transaction algorithm

Normative flow:

```text
A. Capture target pending epoch.
B. Drain the latest structural publication(s).
C. Build/reconcile candidate structure and attachment bindings.
D. Validate duplicate/incompatible ViewState and ContentPort attachments.
E. Drain state and connector-control mutations through the captured epoch.
F. Snapshot active/candidate Sources at concrete source revisions.
G. Resolve style/content effects and dirty propagation.
H. Measure and place candidate geometry to convergence.
I. Prepare candidate connector activations and projections.
J. Compute damage from committed old rects and candidate new rects.
K. Paint candidate regions into a candidate surface.
L. Validate frame invariants.
M. Atomically install candidate runtime state, active connectors, surface, and committed epoch.
N. Present/diff through the terminal backend.
```

If steps B–L fail:

- committed structure/state/active connector/surface remain unchanged;
- hard-invalid operations are removed and reported;
- transient work remains pending for retry where meaningful;
- source data already accepted by a Source remains authoritative and will be observed on a later successful frame;
- no blank intermediate connector frame is emitted.

### 9.15 Error classes during flush

| Error class | Examples | Policy |
|---|---|---|
| caller validation | negative padding, wrong node kind, stale handle | reject operation; report typed error; do not retry |
| structural candidate validation | duplicate port/state attachment | reject candidate root; old root remains |
| capability unsupported | backend lacks required token | connector status becomes unsupported; old active remains |
| geometry blocked | port allocation below minimum | non-fatal blocked status; auto-retry on geometry change |
| transient preparation | temporary allocation/adapter failure | old frame remains; retain retry obligation when safe |
| invariant violation | generation mismatch inside committed graph, impossible parent link | fail loudly; do not silently full-rebuild unless explicitly classified as a recoverable cache miss |

### 9.16 Convergence

Retain the existing bounded multi-pass convergence model for layout-aware components. State/content dirty processing participates in the same candidate-frame convergence loop.

Do not run an independent content layout loop that can commit geometry separately from the Scene frame.

---

## 10. Damage model

### 10.1 PERF-13 baseline: rectangle damage

Implement rectangle-level damage first. Cell-range or glyph-range damage is deferred.

```rust
struct DamageRegion {
    rects: Vec<Rect>,
    full: bool,
}
```

### 10.2 Damage rules

| Change | Damage |
|---|---|
| paint-only property | current border rectangle |
| text/content paint with same geometry | ContentHost viewport/border rectangle |
| move | union of old and new rectangles |
| resize | union of old and new rectangles |
| clip change | old clipped region + new clipped region |
| removed occurrence | old rectangle |
| newly mounted occurrence | new rectangle |
| connector switch, same geometry | port rectangle |
| theme swap | affected styled rectangles; full frame is acceptable for first theme tranche |

### 10.3 Coalescing

Initial internal constants:

```text
MAX_DAMAGE_RECTS = 64
FULL_DAMAGE_AREA_RATIO = 0.50
```

Algorithm:

1. clamp damage to viewport;
2. discard empty rectangles;
3. merge overlapping or directly touching rectangles;
4. if rectangle count exceeds the cap, use full-frame damage;
5. if union area exceeds 50% of viewport, use full-frame damage;
6. record counters for rectangle count, merged count, damaged cells, and full fallback.

These constants may be tuned by benchmarks without changing semantics.

### 10.4 Physical surface integration

Reuse the retained `Surface` and its whole-glyph-safe clearing/compositing operations.

For each damage root:

- restore nearest retained ancestor background before compositing transparent child output;
- clear whole wide glyph spans when a changed cell intersects a continuation;
- repaint the smallest safe subtree/region;
- fall back to full paint if style inheritance or overlap prevents a safe local repaint.

### 10.5 Backend output

PERF-13’s required optimization is reduced Rust layout/paint work. The terminal backend may initially continue diffing a complete final surface.

`PreparedSceneFrame` should carry damage metadata for tests, counters, and future backend optimization. Do not block PERF-13 on partial terminal escape emission.

---

## 11. Theme, focus, and semantic style state

### 11.1 Theme

Theme replacement becomes native presentation invalidation:

```text
theme revision advances
    → invalidate resolved-style and paint cache entries
    → repaint affected nodes/full frame
    → no semantic View reconstruction
    → no layout unless a future theme system explicitly supports geometry
```

PERF-13 themes remain presentation-only.

### 11.2 Focus

Focus/focus-within already live in the Rust host/mount graph. PERF-13 must stop requiring a semantic View replacement merely to reflect focus-dependent appearance.

Focus transition:

```text
old focused path + new focused path
    → resolve style context
    → paint affected component/style subtrees
    → no layout
```

### 11.3 Application style states

`ViewState` key/value states are merged with immutable base states and host facts. A state change must invalidate descendants only when selectors/inheritance can observe it.

For the first implementation, conservative subtree paint is acceptable. Add selector-dependency indexing only if benchmarks show it is necessary.

### 11.4 Animation

PERF-13 does not implement a general animation API.

It must, however, leave a native mutation seam that a future Rust-owned clock can call without TypeScript per-frame execution. Existing `ViewSlot` animation remains supported and is not rewritten unless required for integration.

---

## 12. Content-plane entity model

### 12.1 Normative graph

```text
Source
    │ authoritative width-independent semantic data
    ▼
Funnel
    │ immutable typed transformation/delivery contract
    ▼
Connector
    │ one retained attachment and its derived state
    ▼
ContentPort
    │ one structurally mounted receiving region
    ▼
ContentHost layout occurrence
```

### 12.2 Source

A `Source` owns authoritative semantic content independent of where it is displayed.

For text it owns:

```text
UTF-8 content/storage
absolute logical byte coordinate range
revision
retention policy/state
seal state where applicable
width-independent annotations/provenance
```

A Source:

- does not know View topology;
- does not own port width;
- does not own scroll state;
- may be shared by several Connectors;
- may outlive a Tui host;
- advances while all Connectors are inactive/cold;
- is retained in Rust, not reconstructed from a JS String on every frame.

### 12.3 Funnel

A Funnel is an immutable typed value/configuration, not a separately mutable runtime handle in PERF-13.

It owns:

```text
input Source family
output content family
semantic projector/renderer selection
wrap/align/presentation policy appropriate to that family
backend requirements
minimum useful geometry requirements
configuration fingerprint
```

Examples:

```text
PlainUtf8BlockFunnel
PlainUtf8StreamFunnel
existing Markdown stream projector adapter, if needed for Iyon migration
```

A Funnel does not own:

```text
active/inactive state
port geometry
scroll position
width-specific row cache
source bytes
scheduler priority
```

The implementation may intern identical Funnel specs, but that is an optimization, not semantic identity.

### 12.4 Connector

A Connector is a Tui-bound retained identity linking exactly:

```text
one Source
one Funnel spec
one ContentPort
```

It owns attachment-local state:

```text
connector lifecycle/status
activation request/status
width-specific projection and wrap cache when active
last committed source revision
projection revision
measurement summary
backend-placement identity in future
```

In PERF-13, an inactive cold Connector owns no projected rows, no inactive queue, and no layout-ready shadow state.

### 12.5 ContentPort

A ContentPort is a Tui-bound retained identity structurally attached to one `ContentHost` occurrence.

It owns:

```text
semantic accepted content family
current mounted geometry/viewport
connector membership
0..1 active connector
current committed active projection
activation candidate during a frame transaction
```

A port does not own Source data and does not perform policy arbitration.

### 12.6 Source sharing

The same Source may feed several Connectors:

```text
                      TextStreamSource
                      /              \
                     /                \
          Connector A @ width 80   Connector B @ width 32
                   │                     │
                 Port X                Port Y
```

Each Connector has an independent width-specific projection cache. The Source stores width-independent semantic content once.

### 12.7 Port multiplicity

Invariant:

```text
ContentPort.connectors: 0..N
ContentPort.active:     0..1
```

Connector membership and active selection are retained content-plane state. They do not alter the structural View DAG.

### 12.8 Public working API

```ts
const source = TextStreamSource.create({
  retention: { maxBytes: 8 * 1024 * 1024, overflow: "drop-oldest" },
});

const port = tui.contentPort(TextContent);

const connector = port.connect(
  source.funnel({
    renderer: "plain",
    wrap: "word",
  }),
);

const pane = View.content(port)
  .fillWidth()
  .fillHeight();

connector.activate();
source.append("hello\n");
```

Working public concepts:

```ts
ContentPort<TContent>
ContentConnector<TContent>
TextBlockSource
TextStreamSource
Funnel<TContent>
TextContent
```

Do not expose a universal `content(anything)` API or untyped option bag.

### 12.9 Why `View.content(port)` is explicit

Only the structural `ContentHost` kind may host retained content in PERF-13.

This gives deterministic errors and prevents accidental APIs such as:

```ts
View.spacer(2).content(video)
```

A caller may wrap a ContentHost in Container/Grid/Row/Column structure as needed.

---

## 13. Capability model

Capability checks happen at three distinct levels.

### 13.1 Semantic family compatibility

Examples:

```text
Text funnel → TextContent port       valid
Graphics funnel → TextContent port   invalid
```

Represent this twice:

- TypeScript generic compatibility for ordinary callers;
- a compact Rust `ContentFamilyId` runtime check for ABI safety and dynamic paths.

Mismatch is a synchronous `CONTENT_FAMILY_MISMATCH` error at `connect()` time.

### 13.2 Backend capability

A Funnel may require backend tokens:

```text
plain terminal text  → no special token
future Kitty image   → kitty.graphics
future Sixel         → sixel
```

Requirements are a small sorted token set or typed enum family, not a mutable capability god-object.

Backend checks occur when:

- the port is mounted into a host with a known backend;
- a connector is activated;
- backend capabilities change, if that becomes possible.

An unsupported Connector remains connected but has status `unsupported-backend`. Activation fails without replacing the current active Connector.

### 13.3 Geometry readiness

A Funnel may declare minimum useful geometry:

```rust
struct GeometryRequirement {
    min_columns: u16,
    min_rows: u16,
}
```

Geometry is known only after layout.

If a candidate Connector cannot operate in the current allocation:

- status becomes `blocked-geometry`;
- the previous active Connector remains visible;
- the activation request remains pending;
- the runtime retries automatically when port geometry or Funnel/Source requirements change.

Geometry blocking is a status, not a thrown frame-fatal error.

### 13.4 Allocation negotiation is future-specific

Text content copies bytes into Rust-owned storage and does not require GStreamer-like buffer-pool negotiation.

The architecture leaves room for a future graphics Funnel to negotiate surfaces/placement allocation separately from semantic compatibility. Do not implement a generic allocation protocol in PERF-13.

### 13.5 Errors

Examples:

```text
Cannot connect Graphics funnel:
port accepts TextContent.

Cannot activate connector 17:
backend does not advertise kitty.graphics.

Connector 23 is blocked:
requires at least 2 rows; port currently allocates 1.
```

Every error/status includes:

```text
stable code
connector/port identity for diagnostics
required capability
actual capability/allocation
operation that failed
```

---

## 14. Connector lifecycle and cold semantics

### 14.1 Connector state machine

```text
created/inactive
      │ activate()
      ▼
preparing
  ├── compatible + prepared ──► active
  ├── insufficient geometry ──► blocked-geometry
  ├── backend missing ────────► unsupported-backend
  └── hard projection error ──► failed

active
  ├── another connector commits ─► inactive
  ├── port.deactivate() ─────────► inactive
  ├── disconnect/dispose ────────► disposed
  └── source changes ────────────► active + dirty
```

`blocked-geometry` and `unsupported-backend` are inactive states. They retain the activation request where automatic retry is meaningful.

### 14.2 Exact cold definition

An inactive cold Connector:

- performs no projection;
- performs no wrapping/line compilation;
- performs no measurement or layout;
- paints nothing;
- retains no inactive delivery queue;
- retains no inactive projected rows/surface;
- does not receive per-update callbacks for every Source revision beyond cheap host/source dirty bookkeeping;
- retains only identity, immutable Funnel config, Source/Port leases, lifecycle/status, and minimal diagnostics.

This is distinct from Source retention.

### 14.3 Source advancement while cold

Sources always accept semantic updates while Connectors are cold.

Example:

```text
Source revision 1: "hello"
Connector inactive
Source revision 2: "hello world"
activate Connector
→ candidate synchronizes from revision 2
```

No connector buffering occurred.

If a Source intentionally retains only a tail and old data was truncated, activation sees the retained tail. Cold does not recreate discarded Source history.

### 14.4 No automatic first activation

Creating the first Connector does not auto-activate it.

```ts
const connector = port.connect(funnel);
// port is still empty
connector.activate();
```

A future/convenience factory may create+connect+activate in one explicit helper, but the core state machine never relies on “first wins.”

### 14.5 Ports may be empty

A ContentPort may have no active Connector. It renders an empty content box according to §7.11.

Working API:

```ts
port.deactivate();
```

### 14.6 Transactional activation

Activation must use prepare/commit semantics:

```text
1. Record activation request and sequence.
2. Keep current active Connector committed.
3. Validate semantic family and backend requirements.
4. Snapshot candidate Source at a concrete revision.
5. Project/compile for current port geometry into candidate storage.
6. Validate geometry and projection invariants.
7. Compute candidate measurement/layout and damage.
8. Commit active Connector + projection + frame together.
9. Only then release old active projection state.
```

There is no blank or partially projected intermediate frame.

### 14.7 Source change during preparation

A Source snapshot is immutable for the duration of candidate preparation.

After preparation:

- if the Source revision is unchanged, commit normally;
- if it advanced, the runtime may commit the captured internally consistent snapshot and immediately leave the active Connector dirty for the next frame;
- do not restart indefinitely under a continuously writing Source.

For the current single-host-lock path, concurrent advancement may be rare, but the invariant must still be correct for future worker/native producers.

### 14.8 Activation failure

If activation fails:

```text
old active Connector remains active and visible
candidate status records the failure
port active identity is unchanged
structural DAG is unchanged
no partial candidate projection is retained for cold mode
```

If there was no old active Connector, the port remains empty.

### 14.9 Switching order

Multiple activation requests in one pending batch are last-request-wins for the same port, provided every superseded request has not already committed.

```text
A.activate()
B.activate()
C.activate()
flush
→ prepare/commit C only
```

Record coalescing counters for superseded requests.

A call to `tui.flush()` is a barrier. Requests after that barrier belong to a later frame.

### 14.10 Disconnect and dispose

- Disposing an inactive Connector detaches it at the next commit and releases Source/Port leases.
- Disposing the active Connector makes the port empty at commit unless another activation in the same batch succeeds.
- A replacement activation and active-Connector disposal in the same batch commit atomically with no empty intermediate frame.
- `dispose()` is idempotent after completion.

### 14.11 No scheduler in PERF-13

Do not implement:

```text
priority
preemption
blocking ownership
yield-on-seal
activate-on-ready
automatic fallback
policy-based connector selection
```

The application calls `activate()`.

---

## 15. Text content families

Text is the proving family. PERF-13 ships two Source semantics that both produce the same `TextContent` output family through typed Funnels.

### 15.1 UTF-8 Block Source

Snapshot-like content.

Operations:

```text
replace(bytes/text, optional metadata)
clear()
snapshot/query revision
```

No append operation is exposed. A Block replacement is one atomic Source-state swap.

### 15.2 UTF-8 Stream Source

Ordered evolving content.

Operations:

```text
append(text, optional annotations)
replace(text, optional annotations)
clear()
seal()
snapshot/query revision
```

`append` and `replace` are semantically distinct and must never be guessed from payload shape.

### 15.3 Exact append semantics

`append(x)`:

- validates UTF-8 at the native boundary;
- appends bytes after the current logical end;
- assigns absolute byte coordinates;
- appends annotations relative to the appended payload after validation/lowering;
- advances Source revision once for the logical call;
- applies retention atomically;
- preserves ordering among append calls.

### 15.4 Exact replace semantics

`replace(x)`:

- constructs a fresh candidate text store and metadata off to the side;
- resets the retained logical range according to the Source’s coordinate policy;
- swaps it atomically on success;
- invalidates all semantic projection from the new source base;
- advances revision once;
- is not implemented as `clear(); append(x)` because that would expose an intermediate empty revision and incorrect coalescing semantics.

The coordinate policy for PERF-13 text replacement is:

```text
source_base = 0
source_end  = encoded byte length
```

This matches a snapshot replacement. Existing rolling-stream adapters that require monotonic external coordinates must use the existing `StreamSnapshot` adapter path until they are deliberately migrated.

### 15.5 Clear

`clear()` is equivalent to an atomic replace with empty text for content semantics, but has its own ABI/control opcode so no empty byte pointer is required.

### 15.6 Seal

`seal()` is a one-way semantic transition:

```text
open → sealed
```

After seal:

- append/replace/clear fail with `SOURCE_SEALED`;
- Source content remains readable/projectable;
- Connector activation remains legal;
- History may use the sealed frontier for freezing/promotion;
- a second `seal()` returns `SOURCE_ALREADY_SEALED` to preserve strict bug detection and current behavior.

Seal does not automatically activate a Connector or freeze History.

### 15.7 Existing `TextStream.update`

The existing public `TextStream.update(text)` becomes a compatibility adapter to new Stream `replace(text)` semantics. Do not retain a separate native update implementation.

### 15.8 Plain and Markdown Funnels

Mandatory first Funnels:

```text
PlainUtf8BlockFunnel
PlainUtf8StreamFunnel
```

The existing Markdown projector may be adapted into a Stream Funnel if Iyon migration requires it. Do not rewrite Markdown as a new document engine in PERF-13.

### 15.9 Rich text and annotations

PERF-13 must preserve current stream annotation capability needed by Iyon, but it need not design a universal rich-document model.

Internal rule:

```text
text bytes use absolute UTF-8 byte coordinates
annotation ranges are validated against UTF-8 boundaries
width-independent annotations live with Source/Funnel semantic projection
width-specific painted runs live in Connector projection
```

Interaction annotations and arbitrary editable rich-text operations are future work.

---

## 16. Text Source retention

### 16.1 Retention is Source policy, not Connector standby policy

```text
Source retention:
    which semantic bytes remain authoritative?

Connector cold policy:
    does an inactive attachment perform/retain derived delivery work?
```

They are independent.

### 16.2 Default

Default retention is unbounded.

Rationale: silent truncation is a semantic data-loss policy and must never be the framework default.

### 16.3 Configurable policy

```ts
interface TextRetentionPolicy {
  readonly maxBytes?: number;
  readonly maxLines?: number;
  readonly overflow: "drop-oldest" | "error";
}
```

At least one limit must be present when a policy is supplied. Limits must be positive safe integers and are converted to bounded native sizes.

If both limits are present, the Source must satisfy both after each mutation.

### 16.4 `overflow: "error"`

The entire append/replace operation is rejected atomically with `SOURCE_RETENTION_OVERFLOW`. Source bytes, revision, annotations, and connector-visible content remain unchanged.

### 16.5 `overflow: "drop-oldest"`

The Source advances `source_base` and removes the oldest retained content.

Rules:

- `maxLines` removes complete oldest logical lines.
- `maxBytes` first removes complete oldest lines where possible.
- If a single remaining logical line exceeds `maxBytes`, retain a UTF-8-safe suffix and mark the retained head as partial.
- Never split a UTF-8 code point.
- Annotation ranges before the new base are discarded; crossing ranges are clipped only when their annotation type permits clipping, otherwise discarded according to the current annotation contract.
- Source revision advances once for the caller operation, not once per removed chunk.

### 16.6 Retention and coordinates

Use absolute logical byte offsets internally. Advancing `source_base` must not renumber retained bytes.

This preserves existing `StreamSnapshot`/frontier reasoning and lets Connectors recognize that their previous projection prefix is no longer available.

### 16.7 Retention observability

Expose read-only source statistics through testing/diagnostics, not the ordinary hot path:

```text
revision
source_base
source_end
retained_bytes
retained_lines
chunk_count
sealed
head_partial
```

---

## 17. Native text storage and projection

### 17.1 Required complexity

The first implementation must provide:

```text
append                  O(new bytes + new line breaks), amortized
replace                 O(replacement bytes)
head truncation         O(chunks/lines removed), not O(total retained bytes)
line lookup             indexed
snapshot                no full UTF-8 copy merely to inspect revision/range
width-specific compile  Connector-local and incremental where existing model permits
```

Repeatedly rebuilding one giant Rust `String` is not acceptable.

### 17.2 Storage shape

Recommended initial representation:

```rust
struct TextStore {
    source_base: u64,
    source_end: u64,
    chunks: VecDeque<TextChunk>,
    line_starts: VecDeque<u64>,
    annotations: AnnotationStore,
    revision: u64,
    sealed: bool,
    head_partial: bool,
}

struct TextChunk {
    start: u64,
    bytes: Arc<[u8]>,
}
```

Requirements:

- chunks are immutable after insertion;
- append may coalesce very small adjacent writes into a bounded tail builder before sealing a chunk;
- chunk thresholds are internal benchmark-tunable constants;
- snapshots share immutable chunks instead of copying all bytes;
- line starts are absolute logical byte offsets;
- every committed chunk is valid UTF-8.

### 17.3 Append algorithm

```text
1. Validate handle/runtime/source lifecycle.
2. Validate byte slice as UTF-8.
3. Validate/convert annotation sidecar against the encoded byte length.
4. Build a candidate tail chunk or merge into the bounded tail builder.
5. Scan only new bytes for line boundaries.
6. Compute candidate source_end and retention removals.
7. If overflow=error and a limit would be exceeded, reject with no mutation.
8. Apply candidate chunk/index/annotation changes under the Source lock.
9. Advance revision once.
10. Mark subscribed active Connector/host pending epochs.
11. Return before any projection/layout/paint.
```

### 17.4 Replace algorithm

```text
1. Validate bytes/annotations and retention in a fresh TextStore builder.
2. Build chunks and line index off to the side.
3. Set base=0 and end=byte length.
4. Swap store atomically.
5. Advance revision once relative to the Source record’s prior revision.
6. Mark active subscribers dirty.
```

Do not mutate the old store incrementally and leave it partially replaced on allocation failure.

### 17.5 Snapshot representation

A Source snapshot must be immutable and cheap to clone:

```rust
struct TextSourceSnapshot {
    revision: u64,
    source_base: u64,
    source_end: u64,
    stable_through: u64,
    chunks: Arc<ChunkSequence>,
    line_index: Arc<LineIndex>,
    annotations: Arc<AnnotationSnapshot>,
    sealed: bool,
    head_partial: bool,
}
```

Reuse/reshape the existing `StreamSnapshot` and `StreamModel` concepts instead of maintaining two unrelated correctness models.

### 17.6 Source/Funnel/Connector split for text

```text
Source
    raw authoritative UTF-8 + source annotations + revision

Funnel
    width-independent semantic projection policy
    e.g. plain or existing Markdown projector

Connector
    projection instance at one port width
    row index / wrap cache / viewport-facing compiled result
```

Current `ProjectedText`-style width-independent semantics may remain in the Funnel stage. Width-specific row compilation belongs to the Connector.

### 17.7 Connector projection cache

Key:

```rust
struct TextProjectionKey {
    source_revision: u64,
    source_base: u64,
    width: u16,
    funnel_fingerprint: u64,
    wrap_mode: WrapMode,
    alignment: TextAlign,
}
```

The value stores:

```text
compiled rows or row index
intrinsic width/height summary
stable semantic/visual frontier
paint runs/styles
projection revision
```

An inactive cold Connector must drop this value.

### 17.8 Incremental projection

Reuse current semantic/visual restart-frontier logic:

- append-only changes may restart from the prior unstable frontier;
- replacement restarts from the new Source base;
- head truncation invalidates data before the new base and repairs viewport anchors;
- width change invalidates width-specific rows but not Source data;
- a shared Source at two widths compiles independently.

### 17.9 Unicode invariants

- FFI accepts bytes and validates UTF-8 in Rust even when bytes came from `TextEncoder`.
- Logical coordinates are UTF-8 byte offsets.
- Exact range operations require code-point boundary validation.
- Grapheme-safe painting remains the physical layer’s responsibility.
- Retention never leaves an invalid UTF-8 prefix/suffix.
- Wide-glyph continuation rules in `Surface` remain authoritative.

### 17.10 Line semantics

Preserve current newline/wrapping semantics. Do not silently normalize CRLF, strip carriage returns, or reinterpret Unicode line separators during the storage refactor unless the current semantic layer already does so.

Add differential tests for:

```text
LF
CRLF
bare CR
combining marks
emoji ZWJ sequences
wide CJK glyphs
invalid UTF-8 ABI input
head truncation adjacent to multibyte code points
```

---

## 18. Content measurement contract

### 18.1 Measurement inputs

A Text Connector receives:

```text
Source snapshot
Funnel config
available content width / constraints
port viewport state
```

It produces:

```text
intrinsic content size
compiled rows / projection
paint data
projection revision
```

### 18.2 Ownership

```text
Source:
    width-independent data/index/revision

Funnel:
    immutable transform policy

Connector:
    width-specific projection/cache/measurement

Port/ContentHost:
    allocated geometry and clip
```

Width-specific caches must not live solely on Source because one Source can be attached at different widths.

### 18.3 Fixed versus fit allocation

For a fixed/fill ContentHost:

- Source revision may change projection and paint while allocated size stays fixed.
- Ancestor measurement is skipped when candidate intrinsic changes cannot escape the fixed constraints.

For a fit ContentHost:

- changed projection metrics propagate through recorded parent dependencies.

Rust decides this after projection; TypeScript does not submit `invalidateLayout` hints.

### 18.4 Empty/no-active measurement

A port with no active Connector reports zero intrinsic content size. Bounds, padding, border, and parent fill rules still apply.

### 18.5 Candidate activation measurement

Activation preparation measures the candidate off to the side. The committed layout continues using old active metrics until candidate commit.

If candidate metrics change ancestor geometry, the connector switch and the resulting layout commit atomically.

---

## 19. Transport architecture

### 19.1 Four distinct contracts

```text
STRUCTURAL CONTROL/DATA
    existing retained View ABI and structural publication

STATE CONTROL
    N-API lifecycle + typed property patches

CONTENT CONTROL
    N-API Source/Port/Connector lifecycle, config, status, activation

CONTENT DATA
    mandatory direct FFI UTF-8 payload submission
```

Do not extend the structural View schema with large content payload records.

### 19.2 N-API responsibilities

N-API owns:

```text
create/dispose ViewState
set geometry/presentation/style-state patches
create/dispose Source records and return internal handle lanes
create/dispose ContentPort
create/dispose Connector
connect/detach
activate/deactivate
query connector status
query backend capabilities
configure retention/funnel specs
flush host
map native errors to TuiError
```

### 19.3 Content FFI responsibilities

All UTF-8 payload bytes use direct FFI, including tiny updates:

```text
text block replace
text block clear
text stream append
text stream replace
text stream clear
text stream seal (no bytes but belongs to the same data sequencing API)
annotation sidecar submission when migrated
```

This avoids a permanent “small N-API / large FFI” semantic split.

### 19.4 Example C ABI

Illustrative v1 signatures:

```c
int32_t iyon_content_text_stream_append_utf8_v1(
    uint32_t runtime_slot,
    uint32_t runtime_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    size_t len,
    uint32_t sequence_low,
    uint32_t sequence_high
);

int32_t iyon_content_text_stream_replace_utf8_v1(
    uint32_t runtime_slot,
    uint32_t runtime_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    size_t len,
    uint32_t sequence_low,
    uint32_t sequence_high
);

int32_t iyon_content_text_stream_clear_v1(...);
int32_t iyon_content_text_stream_seal_v1(...);
int32_t iyon_content_text_block_replace_utf8_v1(...);
int32_t iyon_content_text_block_clear_v1(...);
```

The exact generated names may differ, but the ABI must include:

```text
ABI version
runtime generation
object slot + generation
ordered sequence or equivalent linearization token
stable integer status
pointer + byte length for payload calls
```

### 19.5 ABI metadata probe

At startup, `transport/content/ffi.ts` must verify:

```text
abi_name = iyon_tui_content
abi_version = 1
semantic_version = 1
schema/header hash
native runtime generation compatibility
required symbol count
pointer/usize assumptions for platform
```

A mismatch is a startup error with no silent payload fallback.

### 19.6 One adapter

Only `transport/content/ffi.ts` may import `bun:ffi` or construct raw pointers.

Public/API/runtime modules call a typed `ContentDataTransport` interface. Tests may inject an oracle implementation, but production has one implementation after the final tranche.

### 19.7 String encoding

Use one `TextEncoder` per module/runtime.

Recommended policy:

- maintain a reusable scratch `Uint8Array` for small/medium strings;
- grow it to a safe upper bound for one `encodeInto` call, up to an internal cap;
- for large strings, use one exact `TextEncoder.encode` allocation;
- perform one FFI payload call per public logical append/replace;
- pass the TypedArray pointer only for the duration of the synchronous call.

Do not split one logical append into several visible Source revisions merely to fit scratch storage.

### 19.8 Memory ownership

The FFI function must synchronously:

1. validate the pointer/length under the supported runtime contract;
2. validate/copy bytes into Rust-owned candidate storage;
3. complete or reject the Source mutation;
4. return before JS may reuse/free the TypedArray.

Rust must never retain the JS pointer.

### 19.9 Annotation sidecar

When current Iyon annotations migrate, use a compact typed sidecar ABI, for example parallel `Uint32Array` lanes plus a style/config table referenced through N-API-created IDs. Do not send arbitrary JS objects through the byte FFI path.

### 19.10 FFI support gate

Bun FFI is a platform/runtime risk and must be treated explicitly:

- support every staged-native platform already claimed by the project;
- add startup probes and CI smoke tests for each;
- add long-running append/replace/GC/teardown soak tests;
- never retain raw pointers;
- keep an N-API mirror only as a development/differential oracle behind a non-production flag;
- delete or make that oracle test-only in the final tranche;
- fail clearly when production FFI is unavailable rather than silently changing architecture.

PERF-13 does not add new platform targets; it preserves the existing support matrix.

### 19.11 Error status codes

The data ABI returns stable integer statuses such as:

```text
0  OK
1  INVALID_RUNTIME
2  STALE_HANDLE
3  WRONG_HANDLE_KIND
4  DISPOSED
5  SOURCE_SEALED
6  INVALID_UTF8
7  RETENTION_OVERFLOW
8  INVALID_SEQUENCE
9  OUT_OF_MEMORY
10 INTERNAL_INVARIANT
```

Map these to typed TypeScript `TuiError` codes outside the FFI hot function. The FFI function must not allocate error strings on success paths.

---

## 20. Scheduling, coalescing, and ordering

### 20.1 No render per mutation

State/content methods enqueue/update native semantic state and return. They do not project/layout/paint synchronously unless the caller explicitly reaches a flush/read barrier.

### 20.2 Native pending epoch is authoritative

Each Tui host owns:

```rust
pending_epoch: u64
committed_epoch: u64
```

Every host-affecting mutation atomically advances `pending_epoch` or associates work with the current pending epoch.

The TypeScript microtask scheduler is only a wake hint. Correctness must not depend on a JS boolean saying “scheduled.” If a wake is lost, native `pending_epoch != committed_epoch` still proves work exists.

### 20.3 Source subscriptions

A Source record tracks weak subscriptions from Connectors to their Tui hosts. A successful Source mutation marks only hosts with affected active or activation-pending Connectors dirty.

Inactive cold Connectors do not project, but an activation-pending Connector’s host must wake so it can prepare the latest revision.

### 20.4 TypeScript host scheduler

One scheduler per Tui wrapper:

```text
markWakeHint()
    if no microtask currently queued:
        queue one microtask

microtask:
    call native flush-through-current-epoch once
    if native still reports pending work:
        queue another microtask
```

Do not spin synchronously under continuous producers. Process one captured epoch, then yield and reschedule.

### 20.5 Read-your-writes barriers

The following force a flush before returning observable frame state:

```text
tui.flush()
screenRows()
nativeHistoryRows()
styleAt()
cellXOfText()
headless/testing frame snapshot
```

Input dispatch, resize, backend poll, and event-loop sleep must also process pending work before observing or waiting.

### 20.6 Structural publication ordering

For one host flush:

```text
1. latest structural publication through the captured epoch
2. candidate attachment reconciliation
3. retained state patches
4. connector activation/deactivation/disposal control
5. latest Source snapshots/revisions
6. one projection/layout/paint/commit
```

This permits:

```ts
const state = tui.viewState();
state.setPresentation({ background: red }); // unmounted override
await publishRoot(View.container(...).state(state));
tui.flush();
```

The first committed mount already uses the override.

### 20.7 State coalescing

Within one uncommitted epoch range:

- last write wins per `(ViewStateId, PropertyId)`;
- setting a value and then clearing the override resolves to the final base value;
- equal final value is a no-op;
- style-state writes coalesce per key;
- different properties remain one atomic candidate state.

### 20.8 Content ordering

Source operations are linearized in call/sequence order.

Examples:

```text
append A; append B
    → AB

append A; replace X; append B
    → XB

replace X; clear; append B
    → B
```

Adjacent appends may be physically coalesced into one chunk, but Source revision/ordering semantics must remain testable.

### 20.9 Activation is not a source snapshot barrier

`connector.activate()` requests that the Connector synchronize to the latest Source revision at the next commit.

```text
activate(); append("x"); flush();
```

The activated Connector may show `x` in that first committed frame.

To force a boundary:

```text
activate(); flush(); append("x");
```

### 20.10 Public transaction API

PERF-13 does not require a general public `tui.transaction()` or property-binding compiler.

Automatic epoch coalescing plus explicit `tui.flush()` provides the necessary semantics. A small `tui.batch()` convenience may be proposed later, but no tranche may depend on it.

### 20.11 Failure and retry

- Invalid caller operations are removed and surfaced once.
- A failed candidate frame does not advance `committed_epoch` through the failed work.
- Transient retryable work remains discoverable from pending records/epochs.
- There is no separate “dirty but not scheduled” state whose boolean can become stale.

---
