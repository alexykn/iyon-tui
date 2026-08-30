# iyon-ui — Preliminary Architecture Design

**Status:** Preliminary design direction, revision 2  
**Date:** 2026-08-30  
**Revision focus:** Clarify Taffy ownership, narrow the post-PERF-13 structural boundary, and identify PERF-13 layout-invalidating machinery that is migration scaffolding rather than a permanent runtime contract.  
**Depends on:** PERF-13 three-plane retained runtime reaching a stable reference implementation  
**Purpose:** Capture the intended post-PERF-13 evolution of `iyon-tui` into a unified `iyon-ui` runtime with React/TypeScript as the canonical public programming model and terminal + GPUI as native backends.

---

## 0. Executive decision

The intended destination architecture is:

> **One public programming model: React + TypeScript.**  
> **One plane-split TypeScript frontend/runtime.**  
> **One internal retained Rust runtime.**  
> **One layout model: Taffy semantics.**  
> **Two physical backends: terminal and GPUI.**  
> **No public native Rust UI API.**

The bridge is not a generic "UI object" transport. It exposes three semantically distinct protocols:

1. **Structural plane** — identity, topology, order, structural boundaries, retained attachments.
2. **Retained state plane** — geometry and presentation properties that may change without changing topology.
3. **Content plane** — retained source data, streaming bytes/structured content, projection state, connector state.

The same split exists on both sides of the TypeScript ↔ Rust boundary.

The governing transport rule is stronger than "only send diffs":

> **A change in one plane must not even be represented using another plane's transport vocabulary.**

Examples:

- A background-color change cannot be represented as a structural update.
- A streamed text append cannot be represented as a React host-tree mutation.
- A child insertion cannot be represented as content.
- A GPUI frame or terminal redraw that needs no new semantic input performs no TypeScript ↔ Rust traffic.

PERF-13 remains the reference implementation and migration vehicle for the retained runtime, state/content semantics, identity model, transaction semantics, and transport split. It is not necessarily the final public API shape or the final terminal layout implementation.

---

## 1. Why this exists

The original `iyon-tui` architecture evolved toward three distinct classes of UI work because they have fundamentally different update frequencies and transport characteristics:

```text
structure
    rare, identity/topology oriented

state
    frequent, small semantic property mutations

content
    potentially very frequent and bulk-data oriented
```

PERF-13 formalizes that split inside the current TUI architecture.

The next step is not to build a separate React→GPUI stack beside it. The next step is to make the three-plane architecture the canonical `iyon-ui` frontend/runtime boundary and place both terminal and GPUI behind it.

The project should therefore avoid accumulating all of the following as permanent first-class surfaces:

```text
custom TypeScript View composition API
React API
public Rust UI API

custom terminal layout semantics
GPUI/Taffy layout semantics

terminal-specific state model
GPUI-specific state model

multiple bridge protocols that all carry generic "props"
```

The goal is one semantic frontend and one retained runtime with backend specialization only where the physical medium genuinely differs.

---

## 2. Non-negotiable architectural invariants

### 2.1 React is the canonical public UI model

Application UI is authored through React + TypeScript.

React is not merely an optional frontend layered beside a separate canonical `View` API. It becomes the normal composition, lifecycle, identity, and component model for both terminal and GPUI applications.

Long-term destination:

```text
Application
    ↓
React / JSX
    ↓
iyon React reconciler
    ↓
three-plane frontend runtime
    ↓
native retained runtime
```

The current TypeScript `View` composition system may remain during migration and PERF-13 implementation, but it is not assumed to survive as a separate permanent product surface.

### 2.2 No public Rust UI API

The Rust runtime is an implementation boundary, not a second application programming model.

Rust owns:

- native retained topology,
- state arenas,
- content storage/projection,
- Taffy integration,
- backend execution,
- input/focus/control state where appropriate,
- validation and effect classification,
- frame scheduling and commit visibility.

It does not expose a second public Rust component/view authoring API that must remain feature-compatible with React.

Internal Rust APIs can be structured however implementation requires.

### 2.3 Plane separation exists before the bridge

TypeScript must know which plane changed before data enters native code.

Rust must never receive a generic host update and then be responsible for discovering that only one presentation field changed.

Wrong:

```text
React
    ↓
setProps(id, object)
    ↓
Rust compares object
    ↓
Rust discovers one color changed
```

Wrong:

```text
React
    ↓
setStyle(id, completeStyle)
    ↓
Rust compares style
```

Target:

```text
React
    ↓
TS semantic normalization + retained comparison
    ↓
STATE:
    id=42
    property=BACKGROUND
    value=...
```

Rust remains authoritative for validation and consequences, but irrelevant information never crosses the boundary.

### 2.4 TypeScript communicates intent; Rust classifies consequences

The TypeScript side knows:

- property identity,
- plane ownership,
- normalized semantic value,
- equality,
- transport encoding.

Rust knows:

- whether the value is valid,
- whether the backend supports it,
- which native caches/revisions are affected,
- whether layout/measurement/placement/paint are invalidated,
- how dirty work propagates.

TypeScript does **not** send instructions such as:

```text
repaint subtree
remeasure ancestors
damage old rect
```

Those remain Rust decisions.

### 2.5 Unchanged information never crosses again

Both sides retain enough state to know what has already been accepted.

This is not symmetrical mirroring of the entire runtime.

The TypeScript side retains **transport knowledge**.

The Rust side retains **execution state**.

### 2.6 Terminology: occurrence

This document uses **occurrence** as the stable cross-layer term for one mounted host entity:

> **Occurrence:** one retained native runtime entity corresponding to one mounted React host instance. It survives React rerenders while React preserves that host instance's identity, and it owns the native identity to which state, content attachments, events, layout state, and backend state are associated.

An occurrence is not:

- a React component function invocation;
- a semantic declaration that may be reused in several places;
- a transient GPUI `Element`;
- a Taffy layout result.

Changing a host kind in a way that React represents as replacement destroys one occurrence and creates another. Updating state on the same host instance preserves the occurrence.

---

## 3. Target architecture

```text
                              PUBLIC SURFACE

                             React + TypeScript

                                  JSX
                                   │
                                   ▼
                            React reconciler
                                   │
                       semantic normalization
                       + per-plane comparison
                                   │
                ┌──────────────────┼──────────────────┐
                │                  │                  │
                ▼                  ▼                  ▼

           STRUCTURAL             STATE             CONTENT
           TS retention        TS retention        TS retention

           occurrence IDs      declared state      Sources
           topology            snapshots           Funnels/specs
           kinds               property values     Connectors
           child order         pending patches     Ports
           attachments         coalescing          producer state
                │                  │                  │
                ▼                  ▼                  ▼

          structural ABI        state ABI        content data ABI

═══════════════════════════ TypeScript / Rust ═══════════════════════════

                │                  │                  │
                └──────────────────┼──────────────────┘
                                   ▼

                         INTERNAL IYON RUNTIME

                  retained occurrence/topology graph
                  retained mutable state
                  retained content sources/projections
                  event/focus/control state
                  semantic effect classification
                  non-layout invalidation / scheduling
                  revisions / epochs / transaction state

                                   │
                         shared semantic layout
                              vocabulary
                                   │
                       ┌───────────┴───────────┐
                       │                       │
                       ▼                       ▼

                TERMINAL BACKEND          GPUI BACKEND

                Taffy in cell-space       GPUI/Taffy lowering
                terminal measurement      pixel/text measurement
                cell geometry             GPUI geometry
                Surface / damage          GPUI Elements / Scene
                escape-code output        GPU paint
                terminal scrollback       native GPUI facilities
```

---

## 4. One frontend, not two

The final architecture should not maintain both:

```text
immutable custom View composition runtime
```

and:

```text
React Fiber composition runtime
```

as equally canonical public models.

React already provides:

- component composition,
- keyed identity,
- conditional structure,
- lifecycle,
- refs,
- scheduling,
- concurrent/speculative rendering,
- Suspense/Offscreen semantics,
- a mature ecosystem and developer model.

The part iyon must add is not another component system. It is the **native semantic runtime and narrow transport architecture** beneath React.

The project should therefore distinguish:

```text
React/Fiber retention
    = application/component reconciliation

iyon TS retention
    = knowledge of native publication state

Rust retention
    = authoritative native runtime state
```

These are related but not duplicates.

---

## 5. Host occurrence model

React Fiber supplies host occurrence lifecycle and reuse information, but each Iyon host occurrence should retain a small transport-side record.

Conceptually:

```ts
interface HostInstance {
    readonly occurrence: OccurrenceHandle;
    readonly kind: HostKind;

    structural: StructuralSnapshot;
    declaredState: DeclaredStateSnapshot;
    contentBinding: ContentBindingSnapshot;

    materialized: boolean;
}
```

This is **not another VDOM**.

It answers:

> What semantic facts for this host occurrence has Rust already accepted?

React/Fiber answers:

> Is this still the same host occurrence, or was something inserted, removed, moved, or replaced?

Together they allow incremental publication without reconstructing or retransmitting unchanged nodes.

---

## 6. Generated semantic property schema

The host API should be defined from one semantic schema shared by code generation.

Example conceptual schema:

```text
property              plane           value type          native consequence owner

kind                  structural      HostKind            structural runtime
contentPort           structural      ContentPortHandle   structural runtime
grid placement        structural      placement spec      structural runtime

width                 state/geometry  SizeRule            Rust effect classifier
height                state/geometry  SizeRule            Rust effect classifier
padding               state/geometry  Insets              Rust effect classifier
gap                   state/geometry  Length              Rust effect classifier
alignment             state/geometry  enum                Rust effect classifier

background            state/present.  Color               Rust effect classifier
foreground            state/present.  Color               Rust effect classifier
border color          state/present.  Color               Rust effect classifier
opacity               state/present.  scalar              Rust effect classifier

text payload          content         bytes/content IR    content runtime
```

The schema should generate as much of the following as practical:

- JSX/TypeScript types,
- normalization logic,
- equality/comparison logic,
- stable property IDs,
- plane classification,
- wire value encoding,
- Rust descriptor tables,
- validation glue,
- documentation/tests.

This makes the architecture difficult to accidentally collapse into a future generic `setProps()` API.

### 6.1 Public ergonomics may be richer than the wire model

A convenience API such as:

```tsx
<Box
    style={{
        padding: 1,
        background: theme.panel,
    }}
/>
```

may exist.

But `style` is only syntax.

It must normalize before transport into independent semantic property values.

The style object itself is never the wire format.

---

## 7. React props are declarative base state

Geometry and presentation props should normally map to the retained state plane, not structural publication.

Example:

```tsx
<Box background="blue" padding={2} />
```

establishes a React-declared base state:

```text
background = blue
padding    = 2
```

A later render:

```tsx
<Box background="red" padding={2} />
```

produces:

```text
STRUCTURAL:
    nothing

STATE:
    background blue → red
```

React may rerender. The invariant is not "React must never rerender."

The invariant is:

> **A React rerender must not become structural transport unless structural semantics actually changed.**

### 7.1 Base state and retained overrides

The runtime should preserve the PERF-13 distinction between base values and retained mutable overrides.

Conceptually:

```text
React-declared base
        +
explicit retained override
        +
native host/control state
        ↓
effective state
```

Example:

```text
React base background:      red
explicit retained override: green
effective background:       green
```

If React later changes its base to purple while the override remains green:

```text
React base background:      purple
explicit retained override: green
effective background:       green
```

Clearing the override reveals purple.

This prevents imperative state and React from racing to overwrite a single slot.

### 7.2 Imperative state is an optimization/escape hatch

Ordinary application code should use React props.

An imperative retained-state handle may exist for:

- high-frequency interaction,
- native-driven state,
- cases where rerunning React is undesirable,
- animation/control integrations.

It must terminate in the same Rust state machinery as declarative prop updates.

It is not a second styling system.

---

## 8. Structural plane

The post-PERF-13 target deliberately narrows the structural plane.

A fact is structural when changing it changes the **retained runtime object graph or ownership/lifetime relationships**, not merely because it changes layout.

Structural examples:

- host occurrence creation/destruction;
- parent-child insertion, removal, move, or reorder;
- host semantic kind when the kind selects a different retained native behavior family;
- root/portal ownership;
- retained attachment identity;
- `ContentPort` attachment;
- native controller/resource attachment whose existence has independent retained identity or lifecycle.

The target rule is:

> **If the retained occurrence/attachment graph is unchanged, structural transport is silent.**

This is intentionally stricter than PERF-13's initial structural boundary.

### 8.1 Layout configuration is normally state, not structure

Once Taffy is the layout engine, layout configuration should be treated as geometry state whenever it can mutate the same retained occurrence.

Examples that should normally be state-plane values:

```text
display: flex / grid / block
flex direction
grow / shrink / basis
gap
padding
width / height / min / max
alignment
grid track definitions
grid item placement/span
```

Changing `display: flex` to `display: grid` can cause large native layout consequences, but it does not inherently require a new occurrence. It therefore belongs to the state plane unless implementation evidence shows that the change requires a different retained host object/resource.

Likewise, grid track definitions are not structural merely because they alter layout relationships. They are layout input.

This is an important simplification over PERF-13:

```text
PERF-13:
    layout algorithm family / some edge semantics may be structural

post-PERF-13 iyon-ui target:
    retained graph identity/lifetime = structural
    mutable layout configuration      = geometry state
```

### 8.2 Structural exceptions are explicit resource/lifetime changes

Some properties that look stylistic may still require structural work if changing them creates or destroys retained resources.

Examples may include:

- switching between an ordinary box and a specialized native text editor host;
- adding/removing a retained scroll controller if scrolling is modeled as an attached resource rather than ordinary overflow state;
- adding/removing a content host/port attachment;
- changing to a host family with different native lifecycle semantics.

The criterion is not "this affects layout a lot."

The criterion is:

> **Does this mutation change which retained runtime entities exist or who owns them?**

### 8.3 React placement and speculative render safety

`createInstance()` and `createTextInstance()` occur during React render and may belong to work React later abandons.

Therefore render-phase creation must remain TypeScript-local.

Conceptual flow:

```text
React render
    create HostInstance
    appendInitialChild
    build speculative JS-side description
        ↓

React accepts placement during commit
        ↓
structural transaction stages native materialization
```

Abandoned speculative renders never create native retained nodes.

This lifecycle rule is independent of which backend eventually renders the occurrence.

---

## 9. State plane

The state plane owns mutable semantic properties that do not require topology replacement.

At minimum:

```text
geometry:
    width/height rules
    min/max
    padding
    gap
    local alignment
    other Taffy-style scalar layout properties

presentation:
    background
    foreground/text style
    border presentation
    opacity
    style-state-derived appearance

interaction-derived effective state:
    focus
    hover/active where native
    selection/control state where appropriate
```

### 9.1 TypeScript retains normalized last-published state

For every mounted occurrence, TypeScript retains the normalized values Rust has accepted.

Example:

```text
previous:
    padding    = 2
    background = #202020
    foreground = #eeeeee

next:
    padding    = 2
    background = #242424
    foreground = #eeeeee
```

Transport:

```text
target   = occurrence 42
property = BACKGROUND
value    = #242424
```

Not:

```text
complete style object
```

### 9.2 State patches coalesce inside a commit

Within one React commit, last write wins for the same `(occurrence, property)` pair.

Example staged sequence:

```text
42.background = red
17.padding    = 2
42.background = blue
42.background = green
```

Final state batch:

```text
42.background = green
17.padding    = 2
```

This reduces both bridge work and Rust-side redundant mutation processing.

### 9.3 Rust remains authoritative for effects

Receiving:

```text
BACKGROUND → value
```

may classify to presentation/paint work only.

Receiving:

```text
PADDING → value
```

classifies as a layout-input change.

TypeScript does not encode the downstream recomputation frontier.

### 9.4 Dirty ownership: Iyon classifies, Taffy propagates layout work

Taffy already provides per-node layout caches and invalidation mechanics. Its high-level tree exposes `mark_dirty()` / `dirty()`, and its low-level API exposes `CacheTree` plus `compute_cached_layout()`.

Therefore the target architecture must **not** preserve two competing layout dependency systems merely because PERF-13 needed one before Taffy.

The intended responsibility split is:

```text
semantic mutation arrives
        ↓
Iyon property/effect classifier
        │
        ├── presentation only
        │      → no Taffy invalidation
        │      → backend paint/damage scheduling
        │
        ├── content/projection
        │      → update/project content
        │      → if intrinsic metrics changed:
        │             invalidate Taffy layout input
        │
        ├── layout input
        │      → update retained layout value
        │      → invalidate/clear the relevant Taffy cache entry
        │
        └── interaction/runtime
               → update native state
               → invalidate layout only if the resulting semantics require it

Taffy
    → determines which cached layout computations can be reused
    → propagates the required layout computation through its layout algorithm
```

Iyon still needs invalidation outside Taffy for:

- content projection;
- presentation/style resolution;
- backend paint;
- damage;
- focus/input/control state;
- frame scheduling and epochs;
- transaction visibility.

Taffy is therefore the **layout recomputation engine**, not the universal UI dirty system.

### 9.5 PERF-13 layout dirtiness is migration scaffolding

PERF-13 currently specifies machinery such as:

```text
ChildDependency bits
width_dirty / height_dirty
placement_dirty
MEASURE_SELF / MEASURE_ANCESTORS
PLACE_SELF / PLACE_DESCENDANTS
custom measure-cache keys
```

Those mechanisms remain valid for implementing PERF-13 against the current bespoke terminal layout engine.

They are **not** assumed to be permanent iyon-ui contracts.

Once terminal layout moves to Taffy, the default direction is:

```text
retire parallel ChildDependency propagation
retire duplicate width/height/placement dirty tracking
retire a duplicate general layout measurement cache
let Taffy own layout cache/dependency propagation
retain Iyon's semantic effect classification and non-layout invalidation
```

If profiling later proves that Taffy's invalidation is too broad for a critical workload, add targeted metadata only from measured evidence. Do not preserve a parallel dependency graph preemptively.

### 9.6 This collapse must not change TS → Rust transport

Taffy lives entirely below the language boundary.

A React geometry update remains:

```text
OccurrenceId
PropertyId
NormalizedValue
```

It must never become:

```text
complete Taffy Style
complete layout node
complete host props object
```

crossing the bridge.

Rust may internally update a Taffy style value, clear a cache entry, or rebuild a Rust-local layout representation. Those costs are separate native implementation questions.

The plane-split bridge invariant remains unchanged:

> **Using Taffy's cache instead of Iyon's bespoke layout cache is allowed; retransmitting more TypeScript state to make Taffy work is not.**

### 9.7 Taffy integration should prefer embedding over a permanent mirror

Taffy's low-level API is explicitly intended for UI frameworks that already own a retained node/widget tree, and it permits the embedding tree to provide its own cache storage.

The preferred terminal direction is therefore:

```text
Iyon retained occurrence graph
    + per-layout-occurrence Taffy cache/layout storage
        ↓
Taffy low-level layout traits
```

rather than automatically maintaining:

```text
Iyon retained graph
    +
second permanently mirrored TaffyTree
```

A high-level `TaffyTree` is acceptable for an early prototype if it materially reduces implementation risk. A second long-lived mirrored tree should only survive if benchmarks and code simplicity justify it.

One current limitation matters for backend damage: Taffy caches layout computations, but it does not provide a general public "these nodes changed layout this run" set. Iyon may therefore still need to compare committed/candidate rectangles or otherwise track changed backend geometry for damage. That is not a reason to duplicate layout dependency propagation.

---

## 10. Content plane

Content is not React host-tree structure.

This distinction is fundamental for streaming and large retained text.

The PERF-13 entity model remains the conceptual basis:

```text
Source
    │ authoritative width-independent semantic data
    ▼
Funnel / projection specification
    ▼
Connector
    │ retained attachment-local projection state
    ▼
ContentPort
    │ structurally mounted receiving region
    ▼
backend layout/paint
```

The public React API may hide some of these objects where that improves ergonomics, but the internal ownership distinctions remain valuable.

### 10.1 Sources remain independent of React

Canonical streaming must look like:

```ts
source.append(chunk)
```

not:

```tsx
<Markdown text={entireAccumulatedString} />
```

A content append must be able to execute:

```text
Source append
    ↓
content data ABI
    ↓
Rust retained Source
    ↓
projection/layout invalidation
    ↓
backend frame
```

with:

```text
React work      = 0
structural ops  = 0
state ops       = 0
```

### 10.2 React owns attachment, not byte rhythm

React may mount:

```tsx
<Content source={assistantSource} />
```

Internally this can establish/stabilize the required port/connector/projection objects.

After mount, the Source advances independently.

React owns the receiving region's lifecycle.

The Source owns the data lifecycle.

### 10.3 Source sharing remains first-class

The same Source may feed several display regions:

```text
                     Source
                   /                          /                   Connector A       Connector B
             │                 │
           Port A            Port B
```

Projection state that depends on width/viewport remains connector-local.

Width-independent content remains stored once.

### 10.4 React text should use content semantics

A React text host occurrence is structural.

Its textual payload is content.

Example:

```tsx
<Text>Hello</Text>
```

Initial mount:

```text
STRUCTURAL:
    create TextHost

CONTENT:
    text = "Hello"
```

Later:

```tsx
<Text>Goodbye</Text>
```

should become:

```text
STRUCTURAL:
    nothing

STATE:
    nothing

CONTENT:
    replace text payload
```

Small React-owned text does not need to expose the full public Source/Connector model. It may use an internal lightweight retained content slot implemented by the same content subsystem.

### 10.5 Bulk payload path

Bulk content bytes should preserve the PERF-13 data-plane split:

```text
control/lifecycle:
    N-API or generated control ABI

bulk bytes:
    direct same-image FFI data lane
```

Shared memory is not assumed necessary.

The first optimization is always to avoid sending irrelevant data.

---

## 11. React commit mapping and transaction envelope

The React reconciler host config should **stage** semantic operations. It should not itself be the native transport.

Conceptually:

```ts
commitUpdate(instance, type, oldProps, newProps) {
    const normalized = schema[type].normalize(newProps);

    structuralBatch.diff(instance.structural, normalized.structural);
    stateBatch.diff(instance.declaredState, normalized.state);
    contentBatch.diff(instance.contentBinding, normalized.content);
    eventBatch.diff(...);
}
```

The accepted React commit boundary then finalizes publication.

```text
prepareForCommit / React commit starts
        ↓

host callbacks stage:
    structural operations
    state property deltas
    React-owned content replacements
    event subscription changes

        ↓

resetAfterCommit / frontend commit finalize
        ↓

prepare native semantic transaction
        ↓
validate all sections
        ↓
commit desired native runtime state atomically
        ↓
mark backend pending once
```

### 11.1 One transaction envelope, separate plane sections

Atomicity and plane separation are compatible.

A frontend commit may have one envelope containing distinct sections:

```text
FRONTEND COMMIT N

STRUCTURAL
    create DebugPanel
    insert DebugPanel under root

STATE
    root.gap = 2
    panel.background = active

CONTENT
    textSlot.replace(...)

EVENT
    occurrence 17 click-enabled = true
```

The envelope provides all-or-nothing acceptance.

Each section keeps its own semantic vocabulary and transport representation.

### 11.2 Failure semantics

Ordinary fallible work occurs before authoritative publication.

A failed transaction must not leave TS and Rust believing different committed state.

The previous committed native runtime/frame remains authoritative until the complete candidate frontend commit is accepted.

This follows the PERF-13/API-H3 prepare→commit discipline.

---

## 12. Event and callback model

Events are a bridge concern but not a fourth rendering plane.

JavaScript owns application callbacks.

Rust owns native event detection and only needs subscription knowledge.

Conceptual registration:

```text
JS callback registry:
    (OccurrenceHandle, EventKind) → callback

Rust:
    occurrence → event subscription bitmask
```

Changing:

```tsx
onClick={foo}
```

to:

```tsx
onClick={bar}
```

does not require a native change if the occurrence already subscribes to click.

Changing:

```text
no click handler
    →
click handler
```

updates the native subscription mask.

Native event flow:

```text
backend hit-test/input
        ↓
native event { occurrence, kind, payload }
        ↓
JS dispatcher
        ↓
registered React callback
```

Controls whose hottest interaction is better kept native may update state/content first and notify React/application logic as a side channel.

---

## 13. Internal Rust runtime

The Rust runtime is the authoritative execution environment after semantic publication.

It owns approximately:

```text
retained occurrence graph
retained structural attachments
retained declared/effective state
effect classification
dirty propagation
content Sources
content Ports/Connectors/projections
focus/control runtime state
backend capability validation
layout input construction
epochs/frame transaction state
backend scheduling
```

The exact crate boundaries are deferred until implementation proves useful ownership seams.

The design should not start with a large crate reorganization merely to match an architectural diagram.

---

## 14. Layout: Taffy semantics for both backends

The project should stop maintaining a separate general-purpose terminal layout language and dependency engine when the unified runtime can use Taffy semantics.

The target is one shared semantic layout vocabulary modeled closely on Taffy.

Examples:

```text
display
flex direction
grow/shrink
basis
gap
padding
width/height
min/max
alignment
grid tracks/placement
overflow/clipping semantics where portable
```

Public React components may provide ergonomic wrappers:

```tsx
<Row gap={1} align="center">
    ...
</Row>
```

but normalize into the same underlying layout properties.

`Row`/`Column` should be treated as API ergonomics where possible, not as evidence that the native runtime needs bespoke row/column layout algorithms.

### 14.1 Terminal layout uses Taffy in cell-space

Conceptually:

```text
1 Taffy layout unit = 1 terminal cell
```

Terminal text/content measurement supplies cell-space intrinsic sizes:

```text
"hello"
    → width 5
    → height 1
```

Terminal-specific Unicode width, wrapping, content projection, and terminal capability behavior remain Iyon/backend responsibilities.

Taffy supplies the general box/flex/grid layout computation and cache.

### 14.2 Terminal rounding policy is an early proof gate, not a late detail

Taffy's `round_layout` is designed to avoid cumulative gaps: it rounds cumulative absolute edges and derives width/height from rounded edges rather than independently rounding each width.

That behavior maps naturally to a terminal cell grid:

```text
Taffy float layout
        ↓
cumulative edge rounding
        ↓
integral terminal-cell rectangles
```

The initial terminal Taffy prototype must test this immediately.

Minimum rounding corpus:

```text
10 cells / 3 equal flex children
odd widths with 2, 3, 4, 5 children
nested fractional flex layouts
gaps + padding + borders
row-reverse / column-reverse
fractional grid tracks
min/max constraints
text whose measured width changes by one cell
resize sequences that repeatedly cross fractional boundaries
```

Acceptance criteria:

- no gaps caused solely by independent rounding;
- no overlaps caused solely by independent rounding;
- deterministic results for the same semantic tree and viewport;
- stable layout under repeated recomputation;
- terminal-specific integer snapping is documented as a physical backend rule, not forced on GPUI.

Backend parity means equivalent layout semantics, not identical discrete allocation after one backend snaps to cells and the other renders in pixels.

### 14.3 GPUI layout

The GPUI backend should not bypass or fight GPUI's own Taffy integration.

Instead:

```text
shared normalized layout semantics
        ↓
GPUI backend lowering
        ↓
GPUI style / Element construction
        ↓
GPUI/Taffy layout
```

The terminal and GPUI backends therefore share:

- semantic layout vocabulary;
- normalization;
- state property identity;
- intended layout relationships.

They do **not** necessarily share:

- one physical `TaffyTree`;
- resolved rectangles;
- text measurement;
- rounding;
- paint data.

### 14.4 Resolved geometry remains backend-specific

The project should not force the current terminal `ResolvedScene/LayoutTree` to become a universal pixel+cell IR.

Share semantic layout inputs.

Allow backend-specific resolved geometry.

This prevents a bloated lowest-common-denominator representation.

### 14.5 Taffy is below the bridge

Taffy integration must remain an internal native implementation choice.

The TS state plane publishes semantic layout-property deltas. It does not know whether Rust applies those deltas to:

```text
TaffyTree::set_style
a low-level Taffy trait implementation
a compact retained style representation
```

That freedom is intentional and protects bridge performance from layout-engine refactors.

---

## 15. Backend responsibilities

Backend-specific code should be kept as narrow as practical.

### 15.1 Terminal backend

Expected responsibilities:

```text
Taffy tree/integration in cell-space
terminal text measurement
Unicode cell-width handling
content wrapping in terminal constraints
integer cell placement
terminal Surface/cell paint
damage/diff
escape-code output
terminal cursor
terminal scrollback/history integration
terminal capability handling
```

### 15.2 GPUI backend

Expected responsibilities:

```text
lower retained semantic nodes into ephemeral GPUI Elements
lower shared layout properties into GPUI style semantics
pixel/font measurement through GPUI
GPUI/Taffy execution
prepaint/paint integration
GPU/native clipping
images and richer graphical capabilities
GPUI input/focus/accessibility integration
```

### 15.3 Shared runtime responsibilities

Try to keep shared:

```text
React frontend
three-plane TypeScript retention
wire protocols
native identities/generations
transaction semantics
state descriptor/effect system
Source/Port/Connector lifecycle
content semantic storage where backend-independent
event identity
layout property vocabulary
backend capability model
```

---

## 16. Native frame rule

Once the native runtime is current, a backend frame must not inherently require TypeScript.

Examples:

### GPUI animation/cursor blink

```text
TS → Rust traffic = 0
```

### Native hover/focus appearance

```text
TS → Rust traffic = 0
```

### Terminal redraw caused by native runtime state

```text
TS → Rust traffic = 0
```

### Stream append

```text
structural = 0
state      = 0
content    = appended payload only
React      = 0
```

### React color update

```text
structural = 0
state      = one semantic property delta
content    = 0
```

### Structural insertion

```text
structural = create occurrence + edge mutation
state      = initial state only as needed
content    = initial content only as needed
```

The desired performance model is therefore not "FFI is cheap enough every frame."

It is:

> **Most native frames require no language-boundary traffic at all.**

---

## 17. Public API direction

The public API should remain deliberately small.

Possible eventual shape:

```ts
import {
    Box,
    Row,
    Column,
    Text,
    Content,
    createTextStreamSource,
    render,
} from "iyon-ui";
```

Example:

```tsx
function App() {
    const stream = useMemo(
        () => createTextStreamSource(),
        [],
    );

    return (
        <Column>
            <Row gap={1}>
                <Text>Status</Text>
                <Box grow />
                <Text>Ready</Text>
            </Row>

            <Content
                source={stream}
                follow="end"
            />
        </Column>
    );
}
```

Backend selection might be explicit:

```ts
render(<App />, {
    backend: "terminal",
});
```

or:

```ts
render(<App />, {
    backend: "gpui",
});
```

Packaging may later use backend-specific entrypoints if native bundling requires it:

```text
iyon-ui/terminal
iyon-ui/gpui
```

but these must not become different component/state/content APIs.

### 17.1 Internal explicitness does not imply public explicitness

Internally, content may use:

```text
Source
Funnel
Connector
ContentPort
```

That internal model should be designed and stabilized **before** convenience React APIs erase its distinctions.

The simple React API is a facade over those ownership rules, not a replacement model invented independently.

For the common case:

```tsx
<Content
    source={stream}
    projection="plain"
    follow="end"
/>
```

may let React/runtime code own a stable Port/Connector automatically.

For advanced cases, the lower-level TypeScript **resource API** must remain capable of expressing the real internal semantics:

```text
one Source
    → Connector A / projection A / Port A
    → Connector B / projection B / Port B
```

The lower-level resource API is not a second UI composition API. It exposes retained data/attachment resources that React components consume.

Design rule:

> **Freeze the internal Source/Funnel/Connector/Port lifecycle first; derive ergonomic React sugar from it second.**

Do not create public convenience semantics that the retained model cannot represent cleanly, and do not expose every internal implementation detail merely because it exists.

---

## 18. Backend parity

Backend parity is an architectural requirement.

The same component:

```tsx
<Row>
    <Box width={24}>...</Box>
    <Box grow>...</Box>
</Row>
```

should preserve the same semantic relationship on terminal and GPUI:

```text
same host topology
same flex intent
same state model
same Source ownership
same event lifecycle
same content attachment semantics
```

Physical results differ:

```text
terminal:
    cells, terminal glyph metrics, terminal capabilities

GPUI:
    pixels, shaped fonts, graphical capabilities
```

Backend-specific features must be modeled as explicit capabilities rather than silently changing shared semantics.

The initial public surface should favor portable semantics.

Backend-specific escape hatches should be added only when real use cases justify the maintenance cost.

---

## 19. Relationship to PERF-13

PERF-13 remains essential.

Its architectural work is not discarded, but not every implementation mechanism should become a permanent compatibility obligation.

### 19.1 Expected to carry forward

These are architectural contracts:

```text
three-plane ownership
state/content bypass structural composition
opaque generation-scoped retained identities
React/TS communicates semantic intent
Rust validates and classifies consequences
retained Source ownership
Source/Funnel/Connector/ContentPort lifecycle distinctions
source sharing
cold inactive connector semantics as the first scheduler model
prepare/commit visibility discipline
previous committed state/frame remains authoritative on failure
bulk content data plane
backend capability checks
```

### 19.2 Expected to carry forward conceptually but may change representation

These are useful concepts, not frozen data structures:

```text
PropertyDescriptor / stable PropertyId
effect classification
dirty/non-dirty semantic consequences
candidate vs committed state
local revisions/fingerprints
damage tracking
frame epochs
content projection revisions
```

For example, a future effect classifier still needs to distinguish:

```text
presentation-only
content projection
layout-input changed
interaction/runtime changed
```

but it does **not** need to preserve PERF-13's exact `EffectMask` bit layout.

### 19.3 PERF-13 mechanisms specifically not assumed permanent

Once Taffy owns terminal general layout, the following PERF-13 mechanisms are expected to be candidates for deletion or collapse:

```text
ChildDependency layout-edge bitsets
custom width_dirty / height_dirty propagation
custom placement_dirty propagation
MEASURE_SELF / MEASURE_ANCESTORS as a permanent public/internal contract
PLACE_SELF / PLACE_DESCENDANTS as a permanent public/internal contract
a duplicate general-purpose measure/layout cache beside Taffy's cache
bespoke Row/Column/Grid layout algorithms
current terminal resolved-layout representation as a universal backend IR
```

They should still be implemented as PERF-13 requires while PERF-13 is being completed against the current layout engine.

The future rule is:

> **Do not over-invest in preserving these exact mechanisms beyond PERF-13 merely because they exist.**

Taffy must first prove equivalent correctness and acceptable performance. Only then should the redundant mechanisms be removed.

### 19.4 Public surfaces not assumed permanent

The following are migration surfaces rather than destination requirements:

```text
current public TypeScript View builder/composition API
any public Rust UI authoring API
TUI-specific naming that exists only because terminal is currently the sole backend
duplicate layout concepts that simply translate into Taffy concepts
```

PERF-13 is therefore best viewed as:

> **the reference implementation that proves the retained-runtime invariants before the frontend and backend surfaces are consolidated.**

---

## 20. Intended migration/evolution path

This work should not interrupt the current PERF-13 implementation.

The ordering deliberately isolates the two largest risks: **bridge semantics first, layout-engine replacement second**.

```text
1. Finish PERF-13
       ↓
   obtain stable reference implementation of:
       structural plane
       state plane
       content plane
       frame transaction semantics
       handle/lifetime rules
       performance gates

2. Build a deliberately small React frontend against the CURRENT terminal layout
       ↓
   prove only the frontend/bridge hypotheses:
       Fiber lifecycle integration
       TS-side three-plane retention
       generated property schema
       typed state deltas
       React text through content semantics
       Source streaming with zero React work
       atomic frontend commit envelope

   Keep the React component/property surface intentionally narrow.
   Do not perform broad application migration yet.

3. Replace terminal general layout with Taffy UNDER the already-working React path
       ↓
   preserve every bridge counter/invariant from step 2
   map shared layout vocabulary
   prove Taffy cache/invalidation ownership
   prove cell-space rounding
   differential-test against current terminal layout where semantics overlap

4. Make React the canonical public frontend
       ↓
   only after both:
       React transport is proven
       Taffy terminal layout is proven

   migrate examples/components
   expand ergonomic API
   reduce reliance on old View composition surface

5. Add GPUI backend
       ↓
   lower the same retained runtime semantics into GPUI Elements
   preserve zero-TS native frame property

6. Reach backend parity for core primitives/content/state
       ↓

7. Retire obsolete duplicate public surfaces
       ↓
   no public Rust UI API
   no second canonical TS UI composition system
   no duplicate general layout language
```

### 20.1 Why React is not made canonical before the Taffy swap

A possible alternative is step `2 → canonicalize React → replace layout`.

This document does **not** choose that ordering.

Making React canonical would trigger broad application/component migration onto layout semantics that are about to change. That creates more churn and makes compatibility obligations harder to distinguish.

Instead:

- step 2 isolates the bridge using the known current layout engine;
- step 3 changes only native layout under a small, instrumented React surface;
- step 4 expands/canonicalizes React only after both axes are independently proven.

This gives cleaner fault isolation without building a large React binding to layout APIs intended for deletion.

Crate/package extraction should follow proven ownership rather than precede it.

---

## 21. Initial implementation slice after PERF-13

The first React/Taffy prototype should be intentionally small.

Core host primitives:

```text
Box
Row
Column
Text
Content
```

Core state properties:

```text
width / height
min / max
padding
gap
grow/shrink
alignment
background
foreground
border basics
visibility where semantics are settled
```

Core content:

```text
small React-owned text
TextStreamSource
plain text projection
existing Markdown path or a minimal structured-text path
```

Core events:

```text
click
key
focus
scroll
```

Do not begin with:

```text
full CSS compatibility
arbitrary native custom elements
animation system
rich images/video
hot/buffered connector arbitration
large backend-specific API surfaces
universal structured-content protocol
```

---

## 22. Benchmark and observability requirements

Transport counters should exist from the first React prototype.

At minimum measure:

```text
React render time
React commit time

structural ops per commit
structural bytes per commit

state properties per commit
state bytes per commit

content bytes
content appends/replacements

native transaction prepare/commit time
native decode/apply time

frames with JS activity
frames with zero JS activity

terminal layout/paint time
GPUI layout/prepaint/paint time

native retained memory per occurrence
Source/content memory
```

Mandatory scenarios:

### 22.1 Cold mount

Large tree, e.g. 10k+ host occurrences.

Measure:

- React work,
- structural materialization,
- initial state publication,
- memory.

### 22.2 No-op rerender

Same semantic result.

Expected:

```text
structural ops = 0
state ops      = 0
content bytes  = 0
```

apart from unavoidable bookkeeping that does not cross the native boundary.

### 22.3 One presentation change

Example:

```text
background A → B
```

Expected:

```text
structural ops = 0
state ops      = 1
content bytes  = 0
```

### 22.4 One geometry change

Example:

```text
padding 1 → 2
```

Expected:

```text
structural ops = 0
state ops      = 1
```

Rust determines the required layout frontier.

### 22.5 Structural insertion/reorder

Expected transport proportional to the changed structural frontier, not the tree size.

### 22.6 React text replacement

Expected:

```text
structural ops = 0
content replacement only
```

### 22.7 Streaming text

Sustained append workload.

Expected:

```text
React commits = 0
structural     = 0
state          = 0
content        = payload traffic only
```

### 22.8 Native-only frames

Cursor blink, hover, scroll, animation/native interaction where supported.

Expected:

```text
TS → Rust traffic = 0
```

### 22.9 Taffy invalidation and rounding gates

Before terminal Taffy replaces the bespoke layout path, measure:

```text
presentation-only property update
    → Taffy cache invalidations = 0

single geometry property update
    → bridge state ops = 1
    → structural ops = 0
    → Taffy recomputation/cache misses measured

content append with unchanged intrinsic metrics
    → no layout invalidation

content append with changed intrinsic metrics
    → content transport only
    → Rust invalidates Taffy layout internally as required
```

Also run the rounding corpus from §14.2 and record:

```text
cache gets / hits / stores / clears
measure-function calls
nodes whose final rectangles changed
layout time
paint/damage time
```

A Taffy integration is not accepted merely because screenshots look correct. It must prove that removing PERF-13's bespoke dependency machinery does not cause pathological full-tree recomputation.

### 22.10 Backend comparison

For GPUI:

- compare against equivalent raw GPUI workload,
- compare against representative React→GPUI implementations where useful.

For terminal:

- preserve or improve existing PERF-13/PERF-12 performance gates.

---

## 23. Success criteria

The preliminary architecture is considered validated when the following are true:

1. React is capable of driving the terminal backend through the same three planes without structural over-transport.
2. A no-op React rerender does not produce native semantic mutations.
3. A presentation-only React update produces only presentation-state transport.
4. A geometry-only React update produces only geometry-state transport.
5. Streaming retained content performs no React reconciliation.
6. React-owned text changes use content semantics, not structural replacement.
7. State/content changes do not execute structural composition/publication.
8. Rust remains authoritative for mutation consequences and dirty propagation.
9. A layout-property mutation remains one narrow state-plane update across TS → Rust; adopting Taffy never requires complete style/props transport.
10. Taffy owns terminal general-layout caching/recomputation without a parallel permanent `ChildDependency`/width-height-placement dirty graph.
11. Presentation-only mutations do not invalidate Taffy layout.
12. Terminal layout uses Taffy semantics without losing required terminal cell behavior or introducing rounding gaps/overlaps.
13. GPUI can consume the same native retained semantics without requiring TypeScript on ordinary frames.
14. Core React components have equivalent semantics across terminal and GPUI.
15. The final public UI surface does not require maintaining a second Rust UI API.
16. The final public TypeScript surface does not require maintaining a second canonical non-React component/composition model.
17. Backend-specific features remain explicit capability extensions rather than silently fragmenting shared semantics.

---

## 24. Explicitly deferred questions

The following are intentionally **not** settled by this preliminary document.

### React / reconciler

- Exact `react-reconciler` version and HostConfig compatibility strategy.
- Precise Suspense/Offscreen hide/unhide mapping.
- Ref/public-instance API.
- Error propagation behavior if native prepare rejects a React commit.
- Whether some React priorities should map into native scheduling policy.

### State protocol

- Exact binary layout of structural/state batches.
- N-API TypedArray vs other control transport details.
- Whether property batching needs additional interning beyond stable numeric IDs.
- Exact imperative retained-state public API, if any.

### Content

- Final ergonomic React API for Source/Funnel/Connector/Port.
- Whether Funnels remain public objects or mostly declarative projection specs.
- Final structured-text/content IR.
- Markdown/diff/ANSI ownership boundaries.
- Future buffered/hot connector scheduling.
- Shared-memory transport, unless profiling later proves it necessary.

### Layout

- Exact shared Taffy-style property subset.
- Exact list of retained host/resource changes that remain structural rather than geometry state.
- Whether the terminal implementation uses Taffy's low-level traits directly over Iyon storage from day one or begins with a temporary high-level `TaffyTree`.
- Whether any measured workload justifies Iyon-specific supplemental layout dependency metadata after Taffy's cache is in place.
- Backend damage strategy given that Taffy does not currently expose a general changed-node set.
- Exact cross-backend parity tests.

### GPUI

- Exact retained-runtime → GPUI Element lowering shape.
- Stable GPUI `ElementId` mapping from Iyon occurrence identity.
- Which interaction/focus state is delegated directly to GPUI.
- Accessibility bridge.
- Whether some native widgets bypass generic Iyon lowering.

### Packaging

- Final crate/package names.
- `iyon-ui` vs scoped package naming.
- Backend-specific native artifact distribution.
- Compatibility facade duration for `iyon-tui`.

These should be resolved by prototype evidence rather than abstract completeness.

---

## 25. Architectural anti-goals

The implementation should actively resist drifting toward any of the following:

```text
generic setProps(id, object)
generic setStyle(id, completeStyle)
generic setText as a structural host mutation
full View/VDOM serialization per commit
TypeScript participation in every native frame
React participation in streaming content updates
Rust discovering plane ownership after receiving generic props
a public Rust UI API duplicating React
two independent layout languages
a permanent Iyon ChildDependency/measure-placement propagation graph running in parallel with Taffy's general layout cache
sending complete Taffy Style/host props across TS → Rust for a single geometry-property change
preserving PERF-13 dirty bit layouts solely for compatibility after Taffy makes them redundant
forcing terminal cell geometry and GPUI pixel geometry into one resolved IR
backend-specific forks of the public component model
```

If a proposed feature requires one of these, the architecture should be re-examined before accepting the shortcut.

---

## 26. Current Taffy research note

This revision is based on the current Taffy API shape at design time:

- Taffy has a per-node layout cache abstraction (`CacheTree`) used by `compute_cached_layout`.
- The high-level `TaffyTree` exposes `mark_dirty()` and `dirty()`.
- Taffy's own documentation recommends the low-level API when embedding Taffy into a UI framework that already owns a node/widget tree.
- `round_layout` rounds cumulative absolute edges and derives final sizes from rounded edges to avoid introducing gaps through independent rounding.
- Taffy does not currently expose a general public set of nodes whose layout changed in the last compute, so render damage remains an embedding concern.

These are implementation facts, not public iyon-ui contracts. The design should continue to treat Taffy as replaceable internal machinery below the semantic TS → Rust boundary.

Official reference points:

```text
docs.rs/taffy/latest/taffy/
docs.rs/taffy/latest/taffy/trait.CacheTree.html
docs.rs/taffy/latest/taffy/tree/struct.TaffyTree.html
docs.rs/taffy/latest/taffy/compute/fn.round_layout.html
github.com/DioxusLabs/taffy
```

---

## 27. Final north star

The intended system can be summarized as:

```text
                           React
                             │
                             ▼
                  plane-aware TS runtime

            structure       state        content
                │             │             │
                └─────────────┼─────────────┘
                              ▼
                    retained Rust runtime
                              │
                       Taffy semantics
                         /          \
                        /            \
                 terminal            GPUI
```

The critical design principle is:

> **React is the public composition model, but React is not the wire format.**

And:

> **The Rust runtime is retained, but Rust is not asked to rediscover semantic deltas that TypeScript already knows.**

And:

> **Taffy supplies the shared layout semantics, while each backend retains its own physical measurement and rendering model.**

The performance target is therefore not merely "a fast bridge."

It is a system in which:

```text
unchanged structure crosses nothing
unchanged state crosses nothing
streaming content bypasses React
native frames usually cross nothing
```

That is the post-PERF-13 direction for `iyon-ui`.
