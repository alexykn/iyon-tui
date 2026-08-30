# iyon-ui — Preliminary Architecture Design

**Status:** Preliminary design direction, revision 3  
**Date:** 2026-08-30  
**Revision focus:** Keep React/three-plane architecture settled; make full Flexbox/Grid parity the shared layout goal; treat Taffy-on-terminal as an evidence-gated implementation candidate; dogfood the real Iyon TUI against the current terminal layout engine before deleting it.  
**Depends on:** PERF-13 three-plane retained runtime reaching a stable reference implementation  
**Purpose:** Capture the intended post-PERF-13 evolution of `iyon-tui` into a unified `iyon-ui` runtime with React/TypeScript as the canonical public programming model and terminal + GPUI as native backends.

---

## 0. Executive decision

The intended destination architecture is:

> **One public programming model: React + TypeScript.**  
> **One plane-split TypeScript frontend/runtime.**  
> **One internal retained Rust runtime.**  
> **One Iyon-owned layout API with strong terminal/GPUI parity, targeting a feature-complete Flexbox/Grid contract.**  
> **GPUI uses GPUI's Taffy-backed layout path.**  
> **Terminal Taffy is the preferred consolidation candidate, but only if real Iyon dogfood benchmarks prove the trade acceptable.**  
> **The current custom terminal layout engine remains temporarily as an oracle/fallback during that decision.**  
> **Two physical backends: terminal and GPUI.**  
> **No public native Rust UI API.**

The bridge is not a generic "UI object" transport. It exposes three semantically distinct protocols:

1. **Structural plane** — occurrence identity, topology, order, retained resource/attachment ownership.
2. **Retained state plane** — geometry/layout inputs and presentation properties that can mutate the same occurrence.
3. **Content plane** — retained source data, streaming bytes/structured content, projection state, connector state.

The same split exists on both sides of the TypeScript ↔ Rust boundary.

The governing transport rule is stronger than "only send diffs":

> **A change in one plane must not even be represented using another plane's transport vocabulary.**

Examples:

- A background-color change cannot be represented as a structural update.
- A `flexGrow` or grid-track change cannot require retransmitting the host object or complete layout style.
- A streamed text append cannot be represented as a React host-tree mutation.
- A child insertion cannot be represented as content.
- A GPUI frame or terminal redraw that needs no new semantic input performs no TypeScript ↔ Rust traffic.

The layout engine sits **below** that bridge contract.

For the terminal backend, this means the same state-plane update:

```text
OccurrenceId
PropertyId::FlexGrow
NormalizedValue
```

may temporarily feed either:

```text
current custom terminal layout engine
```

or:

```text
Taffy terminal layout engine
```

without changing the React API or wire protocol.

PERF-13 remains the reference implementation and migration vehicle for the retained runtime, state/content semantics, identity model, transaction semantics, and transport split. It is not necessarily the final public API shape or final terminal layout implementation.

The terminal Taffy decision is deliberately **not** settled by architectural preference alone. The desired outcome is Taffy because full Flexbox/Grid parity and deleting bespoke general layout code would materially reduce long-term maintenance. The current engine is retained until real application data proves that this consolidation does not create unacceptable hot-path regressions.

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

terminal-specific state model
GPUI-specific state model

terminal-specific content model
GPUI-specific content model

multiple bridge protocols that all carry generic "props"

different public meanings for ordinary Flexbox/Grid properties
```

The maintenance target is **one semantic frontend and one retained runtime**.

That does not require every backend to share every internal implementation.

In particular:

```text
same public layout semantics
    !=
same physical layout implementation
```

The preferred long-term result is still:

```text
terminal → Taffy
GPUI     → GPUI/Taffy
```

because that gives the strongest Flexbox/Grid parity and deletes the most duplicated general-layout logic.

But the project should not pay a pathological terminal performance cost merely to make both arrows point at the same crate.

The current custom terminal engine has real strengths:

- it implements a deliberately narrower model;
- its Row/Column/Grid paths can exploit known primitive semantics;
- it has terminal-specific text/layout integration;
- PERF-13 is adding targeted retained invalidation around it;
- it already serves the real Iyon workload.

Those strengths make it a valuable benchmark oracle.

The correct question after PERF-13 is therefore not:

> Is Taffy theoretically cleaner?

It is:

> **On the real Iyon application, does Taffy buy enough parity and maintenance reduction while remaining comfortably inside the terminal performance budget?**

The design should make that question cheap to answer and cheap to reverse.

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

- native retained topology;
- state arenas;
- content storage/projection;
- layout-engine integration;
- backend execution;
- input/focus/control state where appropriate;
- validation and semantic effect classification;
- frame scheduling and commit visibility.

It does not expose a second public Rust component/view authoring API that must remain feature-compatible with React.

Internal Rust APIs can be structured however implementation requires.

### 2.3 Plane separation exists before the bridge

TypeScript must know which plane changed before data enters native code.

Rust must never receive a generic host update and then be responsible for discovering that only one presentation or layout field changed.

Wrong:

```text
React
    ↓
setProps(id, object)
    ↓
Rust compares object
    ↓
Rust discovers one property changed
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
    occurrence = 42
    property   = GAP
    value      = 2
```

Rust remains authoritative for validation and consequences, but irrelevant information never crosses the boundary.

### 2.4 TypeScript communicates intent; Rust classifies consequences

The TypeScript side knows:

- property identity;
- plane ownership;
- normalized semantic value;
- equality;
- transport encoding.

Rust knows:

- whether the value is valid;
- whether the selected backend supports it;
- whether the mutation is presentation/content/layout/interaction relevant;
- which backend-local caches/revisions are affected;
- which native work is scheduled.

TypeScript does **not** send instructions such as:

```text
repaint subtree
remeasure ancestors
mark Taffy dirty
invalidate terminal track cache
damage old rect
```

Those remain Rust/backend decisions.

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
- a Taffy layout result;
- a terminal layout cache entry.

Changing a host kind in a way that React represents as replacement destroys one occurrence and creates another. Updating state on the same host instance preserves the occurrence.

### 2.7 Backend choice and terminal layout-engine choice are different decisions

The application chooses its physical host when Iyon starts:

```ts
createIyonHost({ backend: "terminal" });
```

or:

```ts
createIyonHost({ backend: "gpui" });
```

That is a product-level choice: remain in the terminal or open/run a native GUI window.

During the terminal layout migration there is a second, internal choice:

```text
terminal backend
    ├── Taffy layout candidate
    └── legacy custom layout engine
```

That second choice exists for development, dogfooding, differential testing, and rollback. It is not intended to become a permanent application programming concept.

The terminal **renderer/paint path is not duplicated** merely because two layout engines temporarily exist.

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
           topology            property values     Funnels/specs
           kinds               pending patches     Connectors
           child order         coalescing          Ports
           attachments                             producer state
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
                  revisions / epochs / transaction state
                  backend capability validation

                                   │
                          Iyon layout contract
                      full Flexbox/Grid parity goal
                                   │
                 ┌─────────────────┴─────────────────┐
                 │                                   │
                 ▼                                   ▼

          TERMINAL BACKEND                      GPUI BACKEND
                 │                                   │
       layout strategy during                         │
          migration window                            │
         ┌───────┴────────┐                          │
         │                │                          │
         ▼                ▼                          ▼
       Taffy            legacy                  GPUI Elements
     candidate          custom                       │
         │             layout                       ▼
         └───────┬────────┘                     GPUI/Taffy
                 │
                 ▼
       terminal measurement/
       content projection
                 │
                 ▼
       same terminal paint/
       damage/escape output
```

The intentional sharing boundary is:

```text
shared:
    React model
    plane classification
    transport
    retained runtime
    layout property semantics
    content model
    state model
    identity
    transaction semantics

backend-specific:
    physical measurement
    physical layout realization
    paint/presentation
    native controls/capabilities
```

The architecture therefore permits the terminal implementation to be swapped between the legacy engine and Taffy without changing React or the bridge.

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
property                 plane           value type          native consequence owner

kind                     structural      HostKind            structural runtime
contentPort              structural      ContentPortHandle   structural runtime
native controller attach structural      Handle              structural runtime

display                  state/geometry  Display              layout backend
position                 state/geometry  Position             layout backend
width / height           state/geometry  SizeRule             layout backend
min / max                state/geometry  SizeRule             layout backend
margin / padding         state/geometry  Insets               layout backend
gap                      state/geometry  Length               layout backend
flex direction/wrap      state/geometry  enum                 layout backend
flex grow/shrink/basis   state/geometry  typed values         layout backend
align / justify          state/geometry  enum                 layout backend
grid templates           state/geometry  track spec           layout backend
grid placement/span      state/geometry  placement spec       layout backend

background               state/present.  Color                Rust effect classifier
foreground               state/present.  Color                Rust effect classifier
border presentation      state/present.  BorderStyle          Rust effect classifier
opacity                  state/present.  scalar               Rust effect classifier

text payload             content         bytes/content IR     content runtime
```

The schema should generate as much of the following as practical:

- JSX/TypeScript types;
- normalization logic;
- equality/comparison logic;
- stable property IDs;
- plane classification;
- wire value encoding;
- Rust descriptor tables;
- backend capability metadata;
- validation glue;
- documentation/tests.

This makes the architecture difficult to accidentally collapse into a future generic `setProps()` API.

### 6.1 Public ergonomics may be richer than the wire model

A convenience API such as:

```tsx
<Box
    style={{
        display: "flex",
        flexDirection: "row",
        gap: 1,
        padding: 1,
        background: theme.panel,
    }}
/>
```

may exist.

But `style` is only syntax.

It must normalize before transport into independent semantic property values.

The style object itself is never the wire format.

### 6.2 Iyon owns the schema, not Taffy

The shared layout schema should align closely enough with Flexbox/Grid that both terminal Taffy and GPUI lowering are straightforward.

It must not expose Rust/Taffy implementation types as the public contract.

This is intentional:

```text
Iyon semantic property
    ↓
backend adapter
    ↓
Taffy/GPUI/custom terminal representation
```

If the terminal implementation changes later, the React API and transport IDs remain stable.

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

### 8.1 Layout configuration is state regardless of the selected layout engine

Layout configuration should be treated as geometry state whenever it can mutate the same retained occurrence.

Examples:

```text
display: flex / grid / block
positioning inputs
flex direction / wrap
grow / shrink / basis
gap
margin / padding
width / height / min / max
align / justify
grid track definitions
grid auto-placement inputs
grid item placement/span
```

Changing `display: flex` to `display: grid` can cause substantial native layout work, but it does not inherently require a new occurrence.

Likewise, changing grid tracks can reposition every child without changing the retained object graph.

Therefore:

```text
retained graph identity/lifetime = structural
mutable layout configuration      = geometry state
```

The selected terminal engine may internally have very different invalidation behavior, but that implementation difference never changes plane ownership.

### 8.2 Structural exceptions are explicit resource/lifetime changes

Some properties that look stylistic may still require structural work if changing them creates or destroys retained resources.

Examples may include:

- switching between an ordinary box and a specialized native text-editor host;
- adding/removing a retained scroll controller if scrolling is modeled as an attached resource;
- adding/removing a content host/port attachment;
- changing to a host family with different native lifecycle semantics.

The criterion is not:

> this affects layout a lot.

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

This lifecycle rule is independent of which backend or terminal layout engine eventually renders the occurrence.

---

## 9. State plane

The state plane owns mutable semantic properties that do not require retained-graph replacement.

At minimum:

```text
geometry/layout:
    Flexbox/Grid inputs
    width/height/min/max
    margin/padding
    gap
    alignment/justification
    positioning inputs
    overflow/layout policy where shared

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
    gap        = 1
    background = #202020
    foreground = #eeeeee

next:
    gap        = 2
    background = #202020
    foreground = #eeeeee
```

Transport:

```text
target   = occurrence 42
property = GAP
value    = 2
```

Not:

```text
complete style object
complete Taffy Style
complete host props
```

### 9.2 State patches coalesce inside a commit

Within one React commit, last write wins for the same `(occurrence, property)` pair.

Example staged sequence:

```text
42.gap        = 2
17.padding    = 2
42.gap        = 3
42.gap        = 4
```

Final state batch:

```text
42.gap        = 4
17.padding    = 2
```

This reduces both bridge work and Rust-side redundant mutation processing.

### 9.3 Rust classifies semantic effects, not layout-engine internals

Receiving:

```text
BACKGROUND → value
```

may classify to:

```text
presentation changed
```

Receiving:

```text
PADDING → value
```

or:

```text
GRID_TEMPLATE_COLUMNS → value
```

classifies to:

```text
layout input changed
```

TypeScript does not encode the downstream recomputation frontier.

The shared Rust classifier should stay comparatively coarse:

```text
PRESENTATION
CONTENT_PROJECTION
LAYOUT_INPUT
INTERACTION_RUNTIME
STRUCTURE_GUARD
```

Backend/layout-engine-specific machinery refines those effects.

### 9.4 Layout recomputation belongs to the selected layout engine

The common runtime says:

```text
occurrence 42
layout property GAP changed
```

The terminal implementation may temporarily route that fact to either:

```text
legacy engine:
    custom ChildDependency / measure / placement invalidation

Taffy engine:
    update retained layout input
    invalidate Taffy cache/tree as required
```

GPUI routes the same semantic fact into GPUI's normal style/Taffy path.

This is the desired ownership boundary:

```text
Iyon core:
    what semantic category changed?

layout implementation:
    what layout work must be recomputed?
```

There must not be a permanent universal Iyon layout-dependency graph **and** a second layout-engine dependency graph unless measurements prove both are necessary.

### 9.5 PERF-13 dirty machinery is engine-specific migration machinery

PERF-13 currently specifies machinery such as:

```text
ChildDependency bits
width_dirty / height_dirty
placement_dirty
MEASURE_SELF / MEASURE_ANCESTORS
PLACE_SELF / PLACE_DESCENDANTS
custom measure-cache keys
```

Those mechanisms remain valid and required for PERF-13 against the current terminal engine.

Revision 3 deliberately does **not** say they are automatically obsolete.

Their future depends on the terminal engine decision:

```text
if Taffy is accepted:
    delete/collapse redundant legacy layout propagation and caches

if legacy custom layout remains:
    keep/evolve the mechanisms that make that engine performant
```

The architectural contract is only that none of these engine-internal details leak back into:

```text
React props
TS plane ownership
wire records
public native API
```

Do not over-generalize PERF-13's dirty bit layout into a permanent cross-backend interface before the benchmark decision.

### 9.6 The terminal Taffy experiment must not weaken TS → Rust performance

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

Rust may internally update Taffy state, clear a cache entry, or rebuild a Rust-local layout structure. Those costs are independent native implementation questions.

The invariant is:

> **Changing terminal layout engines may change Rust work; it must not increase semantic bridge payload for the same React mutation.**

### 9.7 Engine adapter boundary

The exact Rust interface is deferred, but conceptually the retained runtime needs only a narrow layout integration boundary:

```text
apply layout-property delta
notify intrinsic content metrics changed
compute layout for current viewport
read resolved geometry
```

The legacy engine and Taffy may implement those operations very differently.

Do not force them into a common internal data structure beyond what the retained runtime genuinely needs.

### 9.8 Layout dirty state is not the whole UI dirty state

Regardless of layout engine, Iyon still owns invalidation/scheduling for:

- content projection;
- presentation/style resolution;
- terminal/GPUI paint;
- damage;
- focus/input/control state;
- frame scheduling and epochs;
- transaction visibility.

Taffy can potentially replace general-layout caching and propagation.

It cannot replace Iyon's whole retained runtime.

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
semantic effect classification
content Sources
content Ports/Connectors/projections
focus/control runtime state
backend capability validation
layout-property storage
layout-engine/backend integration
non-layout invalidation and damage state
epochs/frame transaction state
backend scheduling
```

The core runtime should not encode one terminal layout engine's detailed propagation model as a universal architectural concept.

Instead:

```text
shared runtime
    → semantic state changed
    → selected backend/layout implementation handles physical consequences
```

During terminal layout evaluation, the runtime may host two implementations behind one internal adapter:

```text
TerminalLayoutStrategy::Taffy
TerminalLayoutStrategy::Legacy
```

That is temporary migration infrastructure.

The exact crate boundaries are deferred until implementation proves useful ownership seams.

The design should not start with a large crate reorganization merely to match an architectural diagram.

---

## 14. Layout semantics and the terminal engine decision

Revision 3 separates two questions that earlier revisions conflated:

```text
1. What layout semantics should Iyon expose?
2. Which engine should implement those semantics on the terminal?
```

The first is an architecture/API decision.

The second is an implementation decision that should be resolved by real application evidence.

### 14.1 Shared target: one Iyon Flexbox/Grid contract

The long-term parity goal is a common, Iyon-owned layout API with a substantially complete Flexbox/Grid model for both terminal and GPUI.

Representative shared semantics include:

```text
display: flex / grid
positioning model where portable

width / height
min / max
margin / padding

flex direction
flex wrap
flex grow / shrink / basis
order where supported by the host model

align items / self / content
justify content / items / self
row / column gap

grid template rows / columns
grid auto rows / columns
grid auto flow
grid row / column placement
row / column span
track sizing supported by the Iyon contract
```

`Row` and `Column` remain ergonomic React primitives:

```tsx
<Row gap={1} align="center">
    ...
</Row>
```

but should lower to the same semantic layout property system rather than imply permanently bespoke layout algorithms.

"Full Flexbox/Grid" here means:

> **Feature-complete Flexbox/Grid semantics for the Iyon layout contract.**

It does **not** mean:

- CSS parsing;
- the cascade;
- arbitrary browser stylesheets;
- every CSS layout mode;
- browser DOM compatibility.

Iyon owns the typed contract and can deliberately define unsupported or nonsensical terminal behavior.

### 14.2 Same usage semantics, different physical units

The aim is that the same React layout expression has the same relationship semantics on both backends:

```tsx
<Box
    display="flex"
    flexDirection="row"
    flexWrap="wrap"
    gap={...}
>
```

Both backends should agree on:

- which items participate;
- ordering;
- grow/shrink relationships;
- wrapping semantics;
- alignment;
- grid placement;
- track relationships.

Physical realization differs:

```text
terminal:
    integer cells
    terminal grapheme metrics

GPUI:
    pixels
    shaped font metrics
```

Exact public unit syntax and cross-backend dimension scaling are deferred.

Parity means shared layout meaning, not pixel-identical or cell-identical geometry.

### 14.3 GPUI uses GPUI/Taffy

GPUI already lays out ordinary elements using web-based layout rules implemented by Taffy.

The GPUI backend should therefore lower Iyon layout state into GPUI's native style/Element path and let GPUI/Taffy perform layout.

Conceptually:

```text
Iyon normalized layout state
        ↓
GPUI backend lowering
        ↓
GPUI Element/style
        ↓
GPUI/Taffy
        ↓
pixel geometry
```

Iyon should not bypass GPUI's normal layout machinery merely to share an exact terminal implementation.

### 14.4 Terminal Taffy is the preferred consolidation candidate, not an axiom

The maintenance/parity argument for Taffy is strong:

```text
one mature Flexbox/Grid implementation
same broad layout semantics as GPUI
less bespoke general layout code
less need to grow the custom terminal engine toward CSS-like behavior
```

But the current Iyon terminal workload is also unusually sensitive to incremental text/layout behavior:

```text
large retained history
streaming content
wrapped text
frequent small local updates
resizing
scrolling
tool blocks
intrinsic sizing
```

Therefore terminal Taffy must be proven by dogfooding the real application.

The architecture should assume:

```text
Taffy = preferred candidate
```

not:

```text
Taffy = mandatory terminal destination regardless of measurements
```

### 14.5 The current terminal layout engine remains a temporary oracle/fallback

During evaluation:

```text
terminal backend
    │
    ├── Taffy layout candidate
    │
    └── current custom layout engine
             │
             └── existing PERF-13 dirty/cache machinery
```

Both feed the same terminal-specific downstream systems where practical:

```text
terminal text/content measurement
terminal projection/wrapping
terminal resolved geometry contract
terminal paint
damage/diff
escape output
```

The old engine is therefore a **layout engine**, not a separate renderer.

Do not duplicate the whole terminal renderer just to compare layout.

### 14.6 Development A/B selection and feature gating

During the benchmark phase, the preferred development arrangement is to compile both engines into the same development binary and select them at runtime:

```text
iyon --terminal-layout=taffy
iyon --terminal-layout=legacy
```

or an equivalent internal/debug configuration.

This allows:

```text
same source commit
same binary
same compiler mode
same input trace
same machine
```

for cleaner A/B results.

After Taffy passes initial correctness gates, dogfood builds should make Taffy the default while retaining the legacy engine behind an explicit compatibility/debug feature, for example:

```text
legacy-terminal-layout
```

Exact naming is non-normative.

Intended lifecycle:

```text
phase 1:
    both engines built
    runtime A/B switch

phase 2:
    Taffy default for dogfooding
    legacy available as rollback/oracle

phase 3:
    decision made
        ↓
    either delete legacy
        or document why it remains
```

The project should not maintain both engines indefinitely without an explicit reason.

### 14.7 Terminal-specific text remains outside general Flexbox/Grid

Taffy does not replace terminal text semantics.

The terminal backend still owns:

```text
grapheme width
Unicode cell width
unicode-linebreak policy
word/grapheme/no-wrap projection
terminal annotations
content projection
intrinsic text measurement
```

Taffy can ask the terminal backend to measure a leaf under constraints.

The benchmark must therefore count text-measure and wrap calls, because an engine that requests the same expensive measurement repeatedly can be slower even when the layout algorithm itself is fast.

Measurement caches may remain Iyon/content-owned where appropriate.

### 14.8 Terminal-specific controls do not require a bespoke general layout language

Controls such as `RowViewport` may remain first-class terminal behavior even if general terminal layout moves to Taffy.

A useful model is:

```text
outer layout engine
    allocates Content/Viewport rectangle
        ↓
RowViewport/native control
    applies row offset
    selects visible projected rows
    clips
    reports/maintains viewport state
```

This separates:

```text
general box/Flex/Grid layout
```

from:

```text
terminal-specific retained control behavior
```

Do not attempt to encode every terminal concept as a CSS display mode.

### 14.9 Terminal rounding is an early proof gate

A terminal ultimately requires integral cell rectangles.

Taffy's rounding machinery is designed around rounding cumulative absolute edges and deriving size from those rounded edges, avoiding the obvious gap problem of independently rounding every width.

The initial terminal Taffy prototype must test this immediately.

Minimum corpus:

```text
10 cells / 3 equal flex children
odd widths with 2, 3, 4, 5 children
nested fractional flex layouts
gaps + padding + borders
flex wrap
row-reverse / column-reverse
fractional grid tracks
min/max constraints
text whose measured width changes by one cell
resize sequences repeatedly crossing fractional boundaries
```

Acceptance criteria:

- no gaps caused solely by independent rounding;
- no overlaps caused solely by independent rounding;
- deterministic results for the same semantic tree and viewport;
- stable layout under repeated recomputation;
- terminal integer snapping remains a backend physical rule rather than a GPUI restriction.

### 14.10 Taffy caching exists, but scoped invalidation is a real benchmark risk

Taffy does have per-node layout caching and dirty invalidation.

Therefore claims that Taffy necessarily lays out the whole tree every time, or has no equivalent of subtree caching, are incorrect.

However, current Taffy behavior can still invalidate caches through ancestors for scoped changes. As of this design revision, Taffy issue #917 explicitly tracks the cost of small internal changes invalidating layout toward the root, especially for nested/grid-heavy UI.

That is exactly why the terminal decision must use real Iyon traces.

The question is not:

```text
does Taffy cache?
```

It does.

The question is:

```text
does its invalidation/recomputation behavior remain cheap
for Iyon's real retained, text-heavy, streaming workload?
```

### 14.11 The bridge is invariant across the experiment

Whichever terminal engine runs, changing:

```tsx
<Box gap={1} />
```

to:

```tsx
<Box gap={2} />
```

must cross as one state-plane semantic delta.

The Taffy experiment is invalid if it "wins" only by broadening the bridge into complete-style transport.

Likewise, the legacy engine is not allowed to demand a different React API.

### 14.12 Decision policy

The decision is based primarily on **absolute end-to-end cost and maintainability**, not relative microbenchmark ratios.

Example:

```text
legacy: 0.15 ms
Taffy:  0.40 ms
```

may be an easy Taffy win if:

- frame latency remains comfortably below budget;
- total application CPU barely moves;
- memory remains acceptable;
- implementation complexity drops materially;
- full Flexbox/Grid parity becomes straightforward.

Conversely:

```text
local streaming update:
legacy: 0.10 ms
Taffy:  4.0 ms
```

would be a meaningful problem if it appears repeatedly in real workloads.

The terminal decision gate is:

```text
A. Taffy effectively free / faster
    → adopt Taffy
    → remove legacy after soak

B. Taffy slower but operationally irrelevant
    → adopt Taffy for parity/maintenance
    → remove legacy after soak

C. Taffy causes material hot-path regressions
    → diagnose measurement/invalidation/tree integration
    → optimize/retest
    → if still unacceptable, retain custom engine
       and explicitly revisit how full layout parity is achieved
```

The preferred outcome is A or B.

C is an evidence-based escape hatch, not a reason to preemptively abandon Taffy.

---

## 15. Backend responsibilities

Backend-specific code should be kept as narrow as practical.

### 15.1 Terminal backend

Stable terminal responsibilities regardless of layout engine:

```text
terminal viewport dimensions
terminal text measurement
Unicode cell-width handling
content wrapping/projection under terminal constraints
terminal-specific controls such as RowViewport
integral cell geometry
terminal Surface/cell paint
damage/diff
escape-code output
terminal cursor
terminal scrollback/history integration
terminal capability handling
```

General layout is temporarily strategy-selectable:

```text
TerminalLayoutStrategy
    ├── Taffy
    └── Legacy
```

The strategy should not duplicate:

```text
content Sources
React integration
state transport
terminal paint
terminal event model
```

### 15.2 Taffy terminal strategy

Expected responsibilities:

```text
map Iyon retained layout state into Taffy-compatible inputs
maintain/integrate Taffy layout tree/cache
call terminal leaf/text measurement
compute cell-space layout
perform/consume rounding to integral cells
expose resolved rectangles to the terminal backend
```

Whether this uses:

```text
high-level TaffyTree
```

or:

```text
Taffy's low-level traits over Iyon-owned storage
```

is an implementation decision to benchmark.

### 15.3 Legacy terminal strategy

During migration it remains responsible for its existing:

```text
Row/Column/Grid allocation
custom intrinsic measurement flow
PERF-13 ChildDependency/dirty propagation
layout cache behavior
resolved terminal geometry
```

It is retained as:

```text
correctness oracle
performance oracle
rollback path
```

not as a second desired permanent layout language.

### 15.4 GPUI backend

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

### 15.5 Shared runtime responsibilities

Keep shared:

```text
React frontend
three-plane TypeScript retention
wire protocols
native identities/generations
transaction semantics
semantic PropertyId/schema
semantic state/effect classification
Source/Port/Connector lifecycle
content semantic storage where backend-independent
event identity
Iyon Flexbox/Grid property vocabulary
backend capability model
```

The layout implementation is below this boundary.

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
    Grid,
    Text,
    Content,
    createTextStreamSource,
    createIyonHost,
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
                <Box flexGrow={1} />
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

Backend choice happens when the framework host is created:

```ts
const host = await createIyonHost({
    backend: "terminal",
});

host.render(<App />);
```

or:

```ts
const host = await createIyonHost({
    backend: "gpui",
});

host.render(<App />);
```

That is the natural point at which the application decides whether it stays in the terminal or runs in a GPUI window.

Packaging may later use backend-specific entrypoints if native bundling requires it:

```text
iyon-ui/terminal
iyon-ui/gpui
```

but these must not become different React/component/state/content frameworks.

### 17.1 Full common layout first; backend extensions second

The long-term goal is that ordinary Flexbox/Grid layout is shared.

Do not prematurely classify standard Flex/Grid controls as GPUI-only merely because the legacy terminal engine does not implement them today.

Backend-specific APIs remain appropriate for genuinely physical capabilities:

```text
terminal:
    RowViewport
    terminal history/scrollback integration
    terminal hyperlinks/cell capabilities

GPUI:
    graphical surfaces
    images
    richer pointer/window/native facilities
```

The backend is known at host creation, so capability validation can be early and explicit.

That fact should not be used as an excuse to fragment the core layout contract if Taffy can provide parity affordably.

### 17.2 Internal explicitness does not imply public explicitness

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

### 17.3 Terminal layout-engine selection is not a stable public API

During development there may be an option equivalent to:

```ts
createIyonHost({
    backend: "terminal",
    terminalLayoutEngine: "taffy", // debug/migration only
});
```

or an environment/CLI switch.

That is migration instrumentation.

Applications should not be authored around:

```text
if legacy layout engine ...
if Taffy layout engine ...
```

Only the physical backend is a real application-level choice.

---

## 18. Backend parity

Backend parity is an architectural goal, with **core Flexbox/Grid parity intentionally stronger in revision 3** than in revision 2.

The same component:

```tsx
<Box
    display="flex"
    flexDirection="row"
    flexWrap="wrap"
    justifyContent="space-between"
>
    ...
</Box>
```

should ultimately preserve the same layout relationship semantics on terminal and GPUI.

Likewise for Grid:

```tsx
<Grid
    templateColumns={...}
    autoFlow="row"
    gap={...}
>
    ...
</Grid>
```

The goal is:

```text
same host topology
same Flexbox/Grid meaning
same state model
same Source ownership
same event lifecycle
same content attachment semantics
```

Physical results differ:

```text
terminal:
    integral cells
    terminal grapheme metrics
    terminal viewport/capabilities

GPUI:
    pixels
    shaped fonts
    graphical/native capabilities
```

### 18.1 Backend choice is known early

Because applications choose terminal vs GPUI when creating the host, the runtime can validate capability requirements against a concrete backend.

This supports explicit backend-only controls without infecting portable code.

### 18.2 Backend-specific extensions remain valid

Parity does not require pretending a terminal and desktop GPU window are physically identical.

Examples of legitimate divergence:

```text
terminal-only:
    terminal scrollback/history primitives
    RowViewport-style controls
    terminal cell/hyperlink capabilities

GPUI-only:
    native window features
    arbitrary graphical surfaces
    image/GPU capabilities
```

These should be explicit capabilities.

### 18.3 A smaller terminal layout subset is the fallback, not the preferred starting assumption

If Taffy proves operationally acceptable on the terminal, the preferred outcome is full shared Flex/Grid semantics.

If Taffy proves materially unsuitable and the legacy engine remains, the project must make a deliberate second decision:

```text
extend custom engine toward full parity
        OR
document an explicit terminal layout capability subset
```

Do not silently let the legacy engine's current feature set define the permanent public API before the Taffy experiment.

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

A future common effect classifier should distinguish semantic categories such as:

```text
presentation changed
content projection changed
layout input changed
interaction/runtime changed
```

It does **not** need to preserve PERF-13's exact `EffectMask` bit layout across all backends.

### 19.3 PERF-13 layout mechanisms remain valid until the Taffy decision

PERF-13's current terminal engine needs machinery such as:

```text
ChildDependency layout-edge bitsets
custom width_dirty / height_dirty propagation
custom placement_dirty propagation
MEASURE_SELF / MEASURE_ANCESTORS
PLACE_SELF / PLACE_DESCENDANTS
custom measure/layout cache keys
bespoke Row/Column/Grid allocation
```

Revision 3 changes the policy from:

```text
Taffy will replace these
```

to:

```text
Taffy may make these redundant
```

Decision:

```text
Taffy accepted:
    remove the redundant legacy layout machinery
    after correctness/performance soak

Taffy rejected:
    keep the legacy machinery that proves its value
    and continue treating it as terminal-backend implementation detail
```

Therefore:

> **Implement PERF-13 correctly, but do not spend extra effort turning its exact layout dirtiness representation into a permanent cross-backend abstraction before the terminal Taffy benchmark.**

### 19.4 PERF-13 remains the performance oracle

The existing terminal engine after PERF-13 is uniquely valuable because it gives the Taffy experiment:

```text
a known-correct retained runtime
a known terminal renderer
targeted dirty propagation
real Iyon application behavior
existing performance gates
```

The Taffy experiment should change as little else as possible.

That makes differences attributable to the layout engine rather than React, content, or paint.

### 19.5 Public surfaces not assumed permanent

The following remain migration surfaces rather than destination requirements:

```text
current public TypeScript View builder/composition API
any public Rust UI authoring API
TUI-specific naming that exists only because terminal is currently the sole backend
legacy terminal general-layout API concepts that duplicate the future Iyon Flex/Grid schema
```

PERF-13 is therefore best viewed as:

> **the reference implementation that proves the retained-runtime invariants and supplies the terminal performance oracle before the frontend/layout/backend surfaces are consolidated.**

---

## 20. Intended migration/evolution path

This work should not interrupt the current PERF-13 implementation.

The ordering isolates the major risks:

```text
bridge semantics
layout semantics
terminal layout-engine performance
public API migration
GPUI integration
```

### Phase 1 — Finish PERF-13

```text
Finish PERF-13
    ↓
stable reference implementation of:
    structural plane
    state plane
    content plane
    frame transaction semantics
    handle/lifetime rules
    terminal dirty/cache behavior
    performance gates
```

Do not weaken or shortcut PERF-13 because a later layout engine may replace some of its internals.

### Phase 2 — Minimal React frontend on the current terminal engine

Build a deliberately small React frontend against the existing terminal implementation.

Prove:

```text
Fiber lifecycle integration
TS-side three-plane retention
generated PropertyId/schema
typed state deltas
React text through content semantics
Source streaming with zero React work
atomic frontend commit envelope
```

Keep the component/property surface narrow enough that bridge behavior is easy to attribute.

The purpose is to establish:

```text
React/bridge correctness
```

before changing layout.

### Phase 3 — Establish the shared Flexbox/Grid semantic schema

Expand the generated layout schema toward the intended common Flexbox/Grid contract.

The legacy terminal engine only needs adapters for the subset required by the current application/test corpus at this stage.

Do not first reimplement all new Flex/Grid behavior in the legacy engine merely to create a benchmark oracle.

### Phase 4 — Add Taffy as a second terminal layout strategy

Keep:

```text
same React frontend
same TS→Rust transport
same retained runtime
same content system
same terminal measurement where possible
same terminal paint/output
```

Change:

```text
general layout engine
```

Development builds should initially contain both:

```text
Taffy
Legacy
```

and allow runtime A/B selection.

### Phase 5 — Differential correctness and synthetic gates

Before dogfooding Taffy as default:

- run overlapping-layout differential tests;
- run terminal rounding tests;
- verify no bridge counter changes;
- verify presentation-only changes cause no layout work;
- verify content metrics trigger layout only when needed;
- add representative Flexbox/Grid cases that only Taffy supports.

The legacy engine is authoritative only for semantics it already supports.

### Phase 6 — Dogfood the real Iyon TUI

The real Iyon TUI becomes the primary performance fixture.

Record/replay realistic sessions that include:

```text
cold startup
large conversation history
streaming assistant output
tool-call appearance and completion
expanding/collapsing live units
wrapped text
fixed-height/follow-end content
scrolling through history
terminal resize
focus/state changes
idle/native-only frames
```

Run the same traces through both terminal layout strategies.

### Phase 7 — Make Taffy the dogfood default; feature-gate legacy

Once correctness is credible:

```text
normal development/dogfood:
    Taffy default

compatibility/debug:
    legacy-terminal-layout feature or equivalent
```

Keep runtime A/B ability in benchmark builds while useful.

This phase should last long enough to observe workloads synthetic tests miss.

### Phase 8 — Terminal layout decision gate

Classify the outcome:

```text
A. Taffy effectively free / faster
    → adopt
    → remove legacy after soak

B. Taffy measurably slower but operationally irrelevant
    → adopt for parity/maintenance
    → remove legacy after soak

C. Taffy materially harms real hot paths
    → diagnose and optimize
    → rerun
    → if still unacceptable, retain legacy
       and document the parity plan
```

Do not make the decision from one isolated microbenchmark.

### Phase 9 — Make React canonical

Only after the React bridge and terminal layout direction are both understood:

```text
migrate examples/components
expand ergonomic React API
reduce reliance on old View composition surface
```

This avoids broad application migration while both the frontend and general layout model are moving simultaneously.

### Phase 10 — Add/complete GPUI backend

Lower the same retained semantic state into GPUI Elements and GPUI/Taffy.

Prove:

```text
same three-plane bridge semantics
same common Flexbox/Grid contract
zero-TS ordinary native frames
content Source reuse
backend capability validation
```

GPUI work may begin earlier in parallel once the semantic schema is stable, but it should not redefine the shared contract independently.

### Phase 11 — Retire duplicate surfaces

Final cleanup target:

```text
no public Rust UI API
no second canonical TS UI composition system
no duplicate public layout language
no permanent terminal dual-layout-engine switch unless justified by evidence
```

Crate/package extraction should follow proven ownership rather than precede it.

---

## 21. Initial implementation slice after PERF-13

The first React + terminal-layout experiment should be intentionally constrained while still exercising the architecture.

### 21.1 Core host primitives

```text
Box
Row
Column
Grid
Text
Content
```

### 21.2 First geometry/layout tranche

Enough Flexbox/Grid semantics to stress the common model:

```text
display flex / grid
flex direction
flex wrap
grow / shrink / basis
gap
padding
width / height
min / max
alignment / justification
basic grid templates
grid placement/span
```

The destination is broader than this tranche.

Do not require every Flexbox/Grid feature before meaningful dogfooding begins.

### 21.3 Core presentation

```text
background
foreground
border basics
visibility where semantics are settled
```

### 21.4 Core content

```text
small React-owned text
TextStreamSource
plain text projection
existing Markdown path or a minimal structured-text path
```

### 21.5 Core events

```text
click
key
focus
scroll
```

### 21.6 Terminal layout experiment plumbing

Required from the beginning:

```text
TerminalLayoutStrategy::Legacy
TerminalLayoutStrategy::Taffy

runtime debug/A-B selector
shared layout instrumentation interface
same terminal paint path
differential geometry test harness
record/replay dogfood trace harness
```

### 21.7 Do not begin with

```text
full CSS parser/cascade compatibility
arbitrary native custom elements
general animation system
rich video/live surfaces
hot/buffered connector arbitration
large backend-specific React forks
universal structured-content protocol
permanent support for two terminal layout engines
```

The goal is enough surface to make the real Iyon app a meaningful benchmark, not a complete desktop web platform before the architecture is validated.

---

## 22. Benchmark, dogfood, and observability requirements

Transport counters must exist from the first React prototype.

Layout-engine counters must exist from the first dual-engine terminal build.

The terminal decision is made from **end-to-end application evidence**, with synthetic tests as diagnostics.

### 22.1 Always-on bridge counters

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
```

These counters must remain semantically identical between terminal layout engines.

A Taffy implementation that causes broader TS→Rust payload is architecturally invalid even if Rust-side layout is fast.

### 22.2 Layout/paint counters

For both terminal strategies measure where applicable:

```text
layout calls
layout CPU time
p50 / p95 / p99 layout latency
max layout latency

nodes visited/recomputed
cache gets / hits / misses / stores / clears
dirty/invalidation propagation counts

text measurement calls
text wrapping/projection calls
intrinsic-size queries

resolved rectangles changed
paint/damage CPU
damage region size

allocations where practical
retained layout memory
peak process memory
```

Taffy-specific counters and legacy-specific counters need not be identical internally. They should support equivalent questions.

### 22.3 Mandatory bridge scenarios

#### No-op rerender

Expected:

```text
structural ops = 0
state ops      = 0
content bytes  = 0
```

#### One presentation change

Expected:

```text
structural ops = 0
state ops      = 1
layout work    = 0
content bytes  = 0
```

#### One geometry/layout change

Example:

```text
gap 1 → 2
```

Expected:

```text
structural ops = 0
state ops      = 1
```

The selected layout engine determines the native recomputation frontier.

#### Structural insertion/reorder

Expected transport proportional to the structural delta, not tree size.

#### React text replacement

Expected:

```text
structural ops = 0
state ops      = 0
content replacement only
```

#### Streaming text

Expected:

```text
React commits = 0
structural     = 0
state          = 0
content        = payload traffic only
```

Any required layout invalidation is generated inside Rust after content metrics change.

#### Native-only frames

Expected:

```text
TS → Rust traffic = 0
```

### 22.4 Real Iyon TUI is the primary terminal benchmark

The primary benchmark is not an artificial tree.

Use the actual Iyon TUI application with deterministic or recorded workload replay.

Required scenarios should include:

```text
startup with realistic history
large retained conversation
assistant response streaming
multiple simultaneous/live tool units
tool completion/freeze transitions
expand/collapse interactions
focus transitions
scroll through deep history
follow-end behavior
terminal width resize causing mass rewrap
terminal height resize
idle frames
native cursor/spinner/animation activity where present
```

Where possible record:

```text
same user/event sequence
same Source data/chunks
same timings or deterministic logical ticks
same terminal dimensions
```

and replay against both engines.

### 22.5 Compare total session cost, not only isolated frames

For every recorded workload report:

```text
total wall time
total CPU time
total layout CPU
total text measure/wrap CPU
total paint CPU

p50 / p95 / p99 frame time
p50 / p95 / p99 layout time
worst frame/layout

allocation/memory summary
bridge bytes/ops
```

This prevents misleading conclusions such as:

```text
Taffy is 3x slower
```

when the actual numbers are:

```text
0.10 ms → 0.30 ms
```

and layout is a negligible fraction of application cost.

It also catches the opposite problem: a seemingly small average that hides repeated multi-millisecond hot-path spikes.

### 22.6 Same-binary A/B where practical

During the comparison phase prefer:

```text
same development binary
same optimization level
same code except selected layout strategy
```

with runtime selection.

This reduces noise from:

```text
compiler differences
feature-dependent codegen
different application commits
different replay timing
```

Cargo features remain useful for eventual distribution/rollback, but should not be the only way to benchmark.

### 22.7 Differential correctness

For semantics supported by both engines:

```text
same retained semantic tree
same viewport
same measured content
        │
        ├── legacy layout
        └── Taffy layout
```

Compare after terminal rounding.

Expected equivalence should be defined per property.

Differences are acceptable only when:

- the old engine had intentionally different semantics;
- the new common Flexbox/Grid contract deliberately changes behavior;
- terminal rounding admits multiple equivalent distributions and one is specified as canonical.

The legacy engine is an oracle only for its existing semantic subset.

For new Flexbox/Grid features unsupported by legacy:

```text
Taffy + specification/tests
```

become the authority.

### 22.8 Rounding corpus

Run at minimum:

```text
10 cells / 3 equal flex children
odd widths across multiple children
nested flex
nested grid
flex wrap
reverse directions
gaps + padding + borders
fractional tracks
min/max
intrinsic text
repeated resize across fractional boundaries
```

Assert:

```text
no accidental gaps
no accidental overlaps
determinism
stable recomputation
```

### 22.9 Taffy invalidation stress

Specifically test workloads likely to expose broad ancestor invalidation:

```text
deeply nested flex hierarchy
deeply nested grid hierarchy
large grid with one local child mutation
fixed-size isolation boundary with internal changes
streaming content inside deeply nested container
one child intrinsic-size change
one presentation-only change
```

Measure:

```text
cache invalidations
nodes recomputed
layout time
text measurement calls
```

This exists because current Taffy scoped-recalculation performance is a known open concern.

### 22.10 Legacy engine stress

Do not benchmark only Taffy's weaknesses.

Also exercise cases where the custom engine may become expensive or difficult as the shared layout contract expands:

```text
flex wrap
complex min/max
nested flexible tracks
full alignment/justification
grid auto-placement
new Grid track semantics
```

Measure both performance **and implementation complexity**.

A fast engine that requires continually rebuilding CSS-like semantics is not free.

### 22.11 Maintenance evidence

The decision report should include more than CPU numbers.

Record:

```text
Rust LOC deleted/added
number of layout-specific caches/dirty structures
number of custom algorithms
number of duplicated semantics vs GPUI
test corpus size
known correctness bugs/edge cases
dependency surface
```

The maintenance win is part of the decision.

### 22.12 Decision gate

Do not use a universal relative slowdown threshold.

Adopt Taffy when:

- real Iyon workloads remain comfortably inside latency/frame budgets;
- hot streaming/interaction paths do not develop pathological spikes;
- total CPU/memory cost is operationally acceptable;
- text measurement behavior is controlled;
- full Flexbox/Grid parity becomes materially simpler;
- significant bespoke layout machinery can be deleted.

Retain/continue the legacy engine when:

- repeatable real workloads show material regressions;
- those regressions remain after reasonable Taffy integration/measurement optimizations;
- the cost would be user-visible or materially increase application CPU;
- retaining/extending the custom engine is demonstrably the better engineering trade.

### 22.13 GPUI comparison

For GPUI:

- compare representative workloads against equivalent raw GPUI where useful;
- compare bridge/React overhead separately from GPUI layout/paint;
- ensure ordinary GPUI frames require no TS traffic.

The terminal engine decision and GPUI backend benchmark share the same bridge counters but are otherwise separate performance questions.

---

## 23. Success criteria

The preliminary architecture is considered validated when all architecture-level criteria and one explicit terminal-layout outcome are satisfied.

### 23.1 Bridge/runtime criteria

1. React drives the terminal backend through the three planes without structural over-transport.
2. A no-op React rerender produces no native semantic mutation.
3. A presentation-only React update produces only presentation-state transport and no layout invalidation.
4. A geometry/Flex/Grid update produces only the relevant state-property delta across TS → Rust.
5. Streaming retained content performs no React reconciliation.
6. React-owned text changes use content semantics, not structural replacement.
7. State/content changes do not execute structural composition/publication.
8. Rust remains authoritative for mutation consequences and backend scheduling.
9. A terminal layout-engine swap requires no React API change and no wire-protocol broadening.
10. GPUI consumes the same retained semantic state without requiring TypeScript on ordinary frames.

### 23.2 Layout semantic criteria

11. Iyon has one typed shared layout contract rather than separate public terminal/GPUI layout languages.
12. The intended shared contract is broad enough to provide real Flexbox/Grid parity, not only the current terminal fixed/content/flex subset.
13. Terminal-specific controls remain expressible without contaminating the general layout contract.
14. Terminal rounding is deterministic and gap/overlap safe.
15. Backend-specific physical capabilities remain explicit rather than silently changing shared semantics.

### 23.3 Terminal Taffy decision criteria

One of the following must be documented:

#### Preferred outcome

```text
Taffy accepted
```

because real Iyon dogfood shows acceptable absolute cost.

Then:

16. Taffy becomes the canonical terminal general-layout engine.
17. Redundant legacy general-layout dirty/cache/algorithm machinery is removed after a soak window.
18. The legacy layout feature/switch is deleted unless a separately justified compatibility need remains.

#### Evidence-based fallback

```text
Taffy rejected or deferred
```

because repeatable real workloads show material unresolved regressions.

Then:

16. The benchmark report identifies the actual pathological workloads and measured costs.
17. The custom engine remains an internal terminal implementation, not a public semantic fork.
18. A deliberate plan exists for how the shared Flexbox/Grid contract is achieved or where terminal capability divergence is explicitly declared.

### 23.4 Public-surface criteria

19. The final public UI surface does not require maintaining a second Rust UI API.
20. The final TypeScript surface does not require maintaining a second canonical non-React component/composition model.
21. Applications choose terminal vs GPUI as a backend, not between different Iyon frameworks.
22. The terminal Taffy-vs-legacy benchmark switch does not become a permanent application-level concept without explicit justification.

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

### Shared layout contract

- Exact feature list defining "full Iyon Flexbox/Grid".
- Exact unit model for numbers across terminal cells and GPUI pixels.
- Percentage/viewport/logical-unit policy.
- Exact overflow and absolute-positioning subset.
- Whether margins and every Grid track sizing primitive are needed in the first stable release.
- Which CSS-spec corner cases Iyon intentionally does not promise.
- How backend-specific controls compose with shared overflow/scroll semantics.

### Terminal Taffy experiment

- High-level `TaffyTree` vs low-level Taffy traits over Iyon-owned storage.
- Exact cache ownership for terminal text measurement.
- Whether containment/isolation hints are needed to avoid broad invalidation.
- Damage tracking when layout geometry changes.
- Exact duration of the Taffy-default/legacy-fallback dogfood window.
- Exact runtime switch and Cargo feature names.
- What measured threshold counts as a material regression for each workload.
- Whether Taffy's scoped-invalidation behavior improves upstream before implementation begins.

### Legacy terminal engine if retained

- How far it should be extended toward the shared Flexbox/Grid contract.
- Whether some complex Grid/Flex behavior should delegate to a library while simple paths remain custom.
- Whether its PERF-13 dependency metadata remains optimal once React layout semantics expand.
- Whether a smaller explicit terminal capability profile is ever preferable to implementing full parity.

### GPUI

- Exact retained-runtime → GPUI Element lowering shape.
- Stable GPUI `ElementId` mapping from Iyon occurrence identity.
- Which interaction/focus state is delegated directly to GPUI.
- Accessibility bridge.
- Whether some native widgets bypass generic Iyon lowering.
- Exact mapping for Iyon layout units/properties into GPUI style.

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
a second canonical non-React TypeScript composition model

different public meanings for ordinary Flex/Grid properties without an explicit capability reason
shrinking the permanent common layout API merely to match the legacy terminal engine before Taffy is benchmarked

making Taffy types or complete Taffy Style the TS→Rust wire format
broadening bridge payload because one layout engine finds it convenient

assuming Taffy terminal performance is acceptable without real Iyon dogfood
rejecting Taffy only because a tiny microbenchmark is 2x slower while absolute cost is irrelevant
accepting Taffy despite repeatable multi-millisecond hot-path regressions just for conceptual purity

maintaining Taffy and the legacy terminal general-layout engine indefinitely without an explicit reason
duplicating terminal paint/content/event systems for each layout strategy
calling the legacy layout engine a separate renderer and accidentally forking more than layout

turning PERF-13 ChildDependency/measure-placement internals into a permanent cross-backend ABI
deleting proven PERF-13 layout machinery before the Taffy decision is made

forcing terminal cell geometry and GPUI pixel geometry into one resolved physical IR
backend-specific forks of the React component model
```

If a proposed feature requires one of these, the architecture should be re-examined before accepting the shortcut.

---

## 26. Current Taffy/GPUI research note

This revision separates verified implementation facts from benchmark assumptions.

### 26.1 Verified Taffy facts relevant to the decision

At design time:

- Taffy provides a per-node layout cache abstraction through `CacheTree`.
- `compute_cached_layout()` attempts to reuse a cached result for a node and layout input before computing and storing a new result.
- Taffy's high-level tree exposes dirty marking.
- Taffy's low-level API permits embedding into a framework-owned tree rather than requiring the public application model to become a Taffy tree.
- Leaf/text measurement is delegated to the embedding system.
- Taffy's `round_layout` exists specifically to round float-valued layout to integral output without independently rounding each size.

Therefore several blanket objections are not valid:

```text
"Taffy has no retained cache"
"Taffy cannot skip unchanged layout work"
"Taffy cannot integrate custom text measurement"
"Taffy requires Iyon to send complete styles across the bridge"
```

None of those follows from Taffy's API.

### 26.2 Verified performance risk

Taffy's caching does **not** prove that its invalidation frontier is ideal for Iyon.

As of 2026-08-30, DioxusLabs/taffy issue #917 remains open and describes small scoped internal changes invalidating layout caches through ancestors toward the root, with especially visible cost in nested/grid-heavy UI.

This is directly relevant to Iyon because the application combines:

```text
deep retained trees
large text history
streaming local changes
intrinsic content sizing
nested layout
```

The open issue is not proof that Taffy will be too slow.

It is proof that:

> **"Taffy caches, therefore the performance difference must be negligible" is not an acceptable assumption.**

### 26.3 Verified GPUI fact

GPUI's ordinary Element tree is laid out according to web-based layout rules implemented by Taffy.

GPUI Elements are frame-level objects; GPUI also permits custom Elements to take manual control of layout/painting for specialized cases such as editors.

Therefore the GPUI path naturally remains:

```text
Iyon retained runtime
    ↓
ephemeral GPUI Element lowering
    ↓
GPUI/Taffy
```

without requiring Iyon's terminal physical layout representation to become GPUI's representation.

### 26.4 What remains empirical

The following cannot be settled from Taffy documentation alone:

```text
real Iyon layout CPU
streaming-text invalidation cost
text measurement call amplification
terminal memory overhead
resize behavior on large histories
effect of Grid-heavy Iyon layouts
maintenance reduction after deleting legacy algorithms
```

Those are precisely the measurements required in §22.

### 26.5 Reference points

```text
https://docs.rs/taffy/latest/taffy/
https://docs.rs/taffy/latest/taffy/trait.CacheTree.html
https://docs.rs/taffy/latest/taffy/compute/fn.compute_cached_layout.html
https://docs.rs/taffy/latest/taffy/compute/fn.round_layout.html
https://github.com/DioxusLabs/taffy/issues/917
https://github.com/zed-industries/zed/blob/main/crates/gpui/src/element.rs
```

These are implementation references, not public iyon-ui contracts.

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
                       Iyon layout contract
                    full Flexbox/Grid parity goal
                         /               \
                        /                 \
               terminal backend          GPUI backend
                      │                       │
          ┌───────────┴──────────┐            │
          │                      │            │
        Taffy                 legacy          │
      preferred               oracle          │
      candidate              /fallback        │
          │                      │            │
          └───────────┬──────────┘            │
                      │                       │
              terminal cell paint        GPUI/Taffy
```

The critical design principle remains:

> **React is the public composition model, but React is not the wire format.**

And:

> **The Rust runtime is retained, but Rust is not asked to rediscover semantic deltas that TypeScript already knows.**

And revision 3 adds:

> **Shared layout semantics are an architectural goal; the terminal layout engine is an evidence-gated implementation choice.**

The preferred long-term destination is:

```text
React
    ↓
one Iyon Flexbox/Grid contract
    ↓
retained runtime
      /       \
terminal     GPUI
 Taffy       Taffy
```

because that maximizes parity and minimizes bespoke general-layout maintenance.

But the route to that destination is explicitly measured against the real Iyon terminal application.

The performance target is not merely "a fast bridge" and not merely "Taffy is fast enough in synthetic trees."

It is a system in which:

```text
unchanged structure crosses nothing
unchanged state crosses nothing
streaming content bypasses React
native frames usually cross nothing

and, ideally:

full Flexbox/Grid semantics are shared
without making terminal hot paths materially worse
```

The current custom terminal layout engine survives long enough to answer that last question honestly.

If Taffy passes, delete the duplication.

If Taffy materially fails after reasonable integration work, keep the specialization and document why.

That is the post-PERF-13 direction for `iyon-ui`.
