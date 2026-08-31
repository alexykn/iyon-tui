# iyon-ui — Preliminary Architecture Design

**Status:** Preliminary architecture direction, revision 5  
**Date:** 2026-08-31  
**Supersedes:** `IYON-UI-PRELIMINARY-DESIGN-v4.md`  
**Depends on:** PERF-13 reaching a stable three-plane retained-runtime reference implementation  
**Revision focus:** Resolve the content execution model; make Source/Funnel/Connector/ContentPort ownership implementation-ready; integrate incremental semantic text, Markdown, diff, ANSI, and smoothing; replace the special mixed-content History abstraction with component-only Surfaces; define reversible cache residency rather than rendering freeze; specify the canonical React ergonomics and the dual-backend `iyon` reference application.

---

## 0. Executive decision

The intended destination architecture is:

> **One public programming model: React + TypeScript.**  
> **One plane-split TypeScript frontend/runtime.**  
> **One internal retained Rust runtime.**  
> **One Iyon-owned Flexbox/Grid semantic contract.**  
> **Taffy as the final general-layout engine for terminal and GPUI.**  
> **One retained content architecture built from Source, Funnel, Connector, and ContentPort.**  
> **One component-only Surface model; no special mixed text/component History renderer.**  
> **One first-class Rust Host Environment layer.**  
> **Two physical hosts: terminal and GPUI.**  
> **No public native Rust UI authoring API.**

The TypeScript ↔ Rust boundary exposes three semantic protocols:

1. **Structural plane** — retained occurrence identity, topology, order, and retained-resource attachment identity.
2. **Retained state plane** — geometry, presentation, and interaction values that mutate an existing occurrence.
3. **Content plane** — retained Sources, Funnel specifications, Connector execution, semantic content IR, and bulk payload transport.

The bridge rule remains stronger than “send diffs”:

> **A fact owned by one plane MUST NOT be represented using another plane's transport vocabulary.**

Examples:

```text
background changed
    → one presentation-state delta
    → no structural publication

flex gap changed
    → one geometry-state delta
    → no complete Taffy Style transport

text bytes appended
    → content data lane
    → no React reconciliation
    → no structural/state payload

new tool-call component inserted
    → structural delta
    → no text payload hidden inside structural props
```

The content model is now explicit:

```text
                         Funnel F
                    immutable specification
                              │
                              │ configures
                              ▼
Source S ───────────── Connector C ───────────── ContentPort P
retained content       live binding/execution      UI destination
```

Definitions:

```text
Source
    authoritative retained content and source revision

Funnel
    immutable, shareable description of transformation,
    delivery, and projection policy

Connector
    retained mutable execution of one exact
    (Source, Funnel, ContentPort) binding

ContentPort
    structurally mounted destination owned by one UI occurrence
```

The Connector is not “another stage after the Funnel.” The Funnel configures the Connector. The Connector owns incremental execution state that belongs to the relationship rather than either endpoint.

The canonical high-level React API hides Port and Connector:

```tsx
<Markdown source={assistantStream} smooth />
```

The generic React API exposes Source and Funnel:

```tsx
<Content
    source={assistantStream}
    funnel={markdown().smooth()}
/>
```

Advanced users MAY explicitly own Ports and Connectors, but convenience components MUST lower to the same primitives.

The content pipeline is settled conceptually:

```text
accepted Source bytes/content
        ↓
input decoding
        ↓
semantic transformation
    plain / Markdown / diff / ANSI / semantic IR
        ↓
backend-neutral semantic content IR
        ↓
delivery policy
    immediate / smooth
        ↓
visible semantic frontier
        ↓
width- and host-dependent projection
        ↓
Taffy measurement/layout
        ↓
terminal or GPUI paint
```

**Smoothing occurs after semantic parsing and before physical width/backend projection.** It reveals semantic content using a Rust-owned clock; it does not pace React updates and does not deliberately expose raw Markdown control delimiters.

The Surface model is also settled:

```text
ScrollSurface
    ordered retained component children
    layout
    clipping
    scroll/follow policy
    anchoring
    culling/residency
    cache policy
```

A chat history is an application composition of ordinary components:

```tsx
<HistorySurface followEnd>
    <UserMessage message={userMessage} />
    <AssistantText source={segment1} />
    <ToolCall call={call} />
    <ToolResult result={result} />
    <AssistantText source={segment2} />
</HistorySurface>
```

There is no runtime-level `LIVE → COMPLETED → FROZEN` rendering lifecycle.

> **Iyon does not use semantic completion as a rendering lifetime boundary.**

A resident occurrence remains mutable under theme, effort, Host Environment, selection, editing, resize, and state changes. Stable inputs merely make its derived caches reusable. Cache invalidation is automatic and reversible.

PERF-13 remains the reference implementation for the retained planes, identity, transaction discipline, and content resources. Revision 5 deliberately supersedes PERF-13's special frozen-History interpretation in the later React architecture while preserving its performance objective through dependency-keyed caches, culling, and residency.

---

## 1. Normative language and decision status

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative within this preliminary design.

This document distinguishes three kinds of statements.

### 1.1 Settled architectural decisions

These are destination constraints:

```text
React + TypeScript is canonical public composition.
There is no public Rust UI authoring API.
The TS→Rust boundary is plane-specific.
Taffy is the final general-layout engine for both hosts.
Host Environment is a first-class Rust layer.
Source/Funnel/Connector/ContentPort are distinct ownership concepts.
Surfaces contain component occurrences, not a special text/component history mixture.
Semantic completion never irreversibly freezes presentation.
The reference iyon app ships terminal and GPUI hosts over shared application/UI code.
```

### 1.2 Working API names

Names such as:

```text
ScrollSurface
ContentPort
Connector
Funnel
SemanticTextDocument
defineContentComponent
useContentPort
useContentConnector
```

are normative working names. A focused naming review MAY adjust spelling before public stabilization, but MUST preserve ownership, lifecycle, and transport semantics.

### 1.3 Deferred implementation choices

Representation details are deferred only where the public/cross-plane contract does not depend on them. Examples include:

```text
high-level TaffyTree vs low-level Taffy traits
exact packed state ABI layout
specific semantic-IR arena representation
specific smoothing rate-control algorithm
specific Rust crate boundaries
```

No deferred choice may silently weaken the three-plane bridge, create a second content architecture, or broaden a public API into untyped bags.

---

## 2. Why this architecture exists

The framework serves workloads with fundamentally different rhythms:

```text
structure
    components appear, disappear, move, or change retained identity

state
    presentation/geometry/interaction values mutate

content
    streams, documents, diffs, logs, and semantic text advance

host environment
    capabilities, palette, viewport, scale, focus, and services change

native frames
    layout/paint/input clocks advance without new application semantics
```

A generic native React bridge tends to make the host element mutation the unit of transport. That can remain concise, but it risks carrying complete style objects, strings, or generic props through one bridge vocabulary.

Iyon's stronger proposition is:

```text
React determines occurrence lifecycle.
TypeScript retains what Rust has accepted.
TypeScript classifies semantic deltas before the bridge.
Rust retains and executes each plane independently.
Bulk content bypasses React.
Host capabilities normally alter native realization without React.
```

The additional content complexity is intentional. It enables capabilities a plain `text` prop cannot provide efficiently:

```text
incremental UTF-8 streaming
incremental Markdown parsing
semantic diff rendering
safe ANSI interpretation
backend-neutral semantic style roles
post-hoc theme/effort recoloring
stream smoothing on a native clock
one Source displayed in several places
independent width-specific projection
cold offscreen connectors
terminal and GPUI realization from the same semantic content
```

The complexity is justified only when it remains locally owned.

```text
React does not understand Markdown parsing.
Taffy does not understand streaming.
Markdown does not understand terminal escape output.
Surface does not understand Funnel policy.
Host Environment does not own application content.
```

Revision 5 removes complexity that merely duplicated another subsystem:

```text
custom public composition         → React
custom general layout             → Taffy
special mixed History renderer    → ScrollSurface + components
irreversible frozen views         → cache validity + residency
per-token React updates           → Sources/content plane
backend checks in components      → Host Environment
```

---

## 3. Core terminology

### 3.1 Occurrence

> **Occurrence:** one retained native runtime entity corresponding to one mounted React host instance.

It survives React rerenders while Fiber preserves that host instance identity.

It is not:

```text
a React function invocation
a reusable semantic declaration
a transient GPUI Element
a Taffy layout result
a terminal cell
a history entry
```

### 3.2 Source

> **Source:** authoritative retained content independent of where it is displayed.

A Source owns data, logical coordinates, retention, and revision. It does not own port geometry, theme, scroll offset, or width-specific projection.

### 3.3 Funnel

> **Funnel:** immutable, fingerprintable, shareable specification of how Source content becomes semantic content, how it is delivered, and how it should be projected.

It contains configuration, not mutable execution state.

### 3.4 Connector

> **Connector:** one retained mutable execution instance of applying Funnel F from Source S to ContentPort P.

It owns the relationship-local parser/delivery/projection state.

### 3.5 ContentPort

> **ContentPort:** one structurally retained destination attached to a UI occurrence.

It identifies where content is presented and supplies destination geometry/clip context. It is not the Source and not the parser execution.

### 3.6 Surface

> **Surface:** a retained spatial container of component occurrences with layout, clipping, scrolling, anchoring, culling, and residency policy.

A Surface does not own a special text log and does not understand Markdown or smoothing.

### 3.7 Semantic content IR

> **Semantic content IR:** backend-neutral retained meaning produced by content transformation before terminal rows, glyph shaping, or pixels.

### 3.8 Residency

> **Residency:** how much derived native realization for a mounted occurrence is currently retained near a Surface viewport.

Residency is reversible and independent of application-level completion.

### 3.9 Cache validity

> **Cache validity:** whether derived projection, measurement, layout, style, or paint work is reusable for its declared dependency key.

A cache hit is an optimization, never a semantic freeze.

---

## 4. Non-negotiable invariants

### 4.1 React is canonical public composition

Application UI is authored through React + TypeScript.

```text
Application model
    ↓
React components
    ↓
Iyon host primitives
    ↓
three-plane frontend runtime
    ↓
retained Rust runtime
```

### 4.2 No public Rust UI authoring surface

Rust is an implementation/runtime boundary. Applications are not required to implement Rust Views, Rust widgets, or Rust Funnels.

### 4.3 Plane ownership is established before crossing

Wrong:

```text
setProps(id, object)
setStyle(id, completeStyle)
setText(id, entireAccumulatedString)
```

Target:

```text
STATE: occurrence + PropertyId + normalized value
CONTENT: SourceId + append bytes
STRUCTURE: occurrence/edge/attachment operations
```

### 4.4 Rust owns consequences

TypeScript communicates semantic intent. Rust validates and determines layout, projection, style, paint, damage, and host effects.

### 4.5 Unchanged information never crosses again

TypeScript retains transport knowledge. Rust retains execution state.

### 4.6 Bulk content never requires React

Once a content component exists:

```ts
source.append(chunk)
```

MUST perform zero React reconciliation and zero structural/state transport.

### 4.7 Funnel is specification; Connector is execution

A Funnel MUST NOT contain:

```text
current parser cursor
consumed Source revision
smoothing queue/frontier
destination width
active/cold status
port-specific wrap cache
```

A Connector that contains no mutable state beyond `{ source, funnel, port }` has not earned a separate runtime identity and SHOULD be collapsed into an internal binding representation.

### 4.8 ContentPort is a destination, not an execution object

A Port MUST NOT become the owner of Source storage or parser/smoothing execution merely to reduce object count.

### 4.9 Surface policy is spatial; content policy belongs to children

A Surface MUST NOT gain APIs such as:

```text
appendMarkdownChunk()
freezeTextUnit()
setGlobalSmoothingForHistory()
```

### 4.10 No irreversible rendering freeze

A mounted/resident occurrence MUST remain responsive to all relevant dependency changes.

```text
theme
effort
Host Environment
selection
editing
focus/hover
viewport width
content revision
```

### 4.11 Completion belongs to application/content semantics

A chat turn, tool call, or Source MAY be marked complete/sealed by its owning application/resource. That MUST NOT freeze the occurrence's presentation or layout.

### 4.12 Host Environment is orthogonal to the planes

Capabilities and environment facts are native inputs, not a fourth application plane.

### 4.13 Taffy is final general layout

Terminal and GPUI use Taffy for general Flexbox/Grid semantics. The legacy terminal allocator is migration-only.

### 4.14 Convenience lowers to explicit primitives

Every batteries-included component MUST use the same Source/Funnel/Connector/Port machinery exposed to advanced users.

### 4.15 Defaults are local and typed

Content defaults MUST resolve at the component instance, component definition, or nearest typed subtree scope. There is no assumption that all content in an application uses one Funnel or delivery policy.

---

## 5. Target architecture

```text
                                TYPESCRIPT

                       React application/components
                                  │
                                  ▼
                           React reconciler
                                  │
                     plane-aware normalization
                                  │
          ┌───────────────────────┼────────────────────────┐
          │                       │                        │
          ▼                       ▼                        ▼

    STRUCTURAL TS             STATE TS                CONTENT TS
    retention                 retention               resources
    occurrences               normalized values       Sources/Funnels
    topology                  pending patches         producer batching
    attachments               local defaults          content factories
          │                       │                        │
          ▼                       ▼                        ▼
    structural ABI           state ABI             content data ABI

══════════════════════════ TypeScript / Rust ═══════════════════════════
                                  │
                                  ▼
                    ┌──────────────────────────┐
                    │  RETAINED IYON RUNTIME   │
                    │                          │
                    │ occurrences/topology     │
                    │ retained state           │
                    │ Sources/Connectors/Ports │
                    │ semantic content IR      │
                    │ events/focus/control     │
                    │ transactions/epochs      │
                    └────────────┬─────────────┘
                                 │
          ┌──────────────────────┼────────────────────────┐
          │                      │                        │
          ▼                      ▼                        ▼

   HOST ENVIRONMENT       TAFFY LAYOUT             SURFACE RUNTIME
   capabilities           shared Flex/Grid         scroll/anchors
   environment            backend measure          culling/residency
   policy/services        cells or pixels          cache orchestration
          │                      │                        │
          └──────────────────────┼────────────────────────┘
                                 │
                      ┌──────────┴──────────┐
                      │                     │
                      ▼                     ▼

                 TERMINAL HOST           GPUI HOST
                 term measurement        GPUI measurement
                 cell paint              GPUI Scene/GPU
                 tty protocols           windows/IME/pointer
                 scrollback              AccessKit/services
```

The retained semantic graph belongs to Iyon Rust.

The React Fiber tree and TS HostInstances retain enough identity/publication knowledge to avoid resending unchanged semantics.

Taffy and backend frame objects are downstream physical representations, not the semantic graph.

---

## 6. React frontend and occurrence identity

### 6.1 React components and host primitives

Application components may be freely named:

```tsx
function AssistantText({ source }: { source: TextStreamSource }) {
    return <Markdown source={source} smooth />;
}
```

`AssistantText` is a React component. The native occurrence is created for the Iyon host primitive(s) it returns.

```text
React component tree
    AssistantText
        ↓ expands to
Iyon host tree
    ContentHost occurrence
```

### 6.2 HostInstance

Each React host occurrence retains a small TS publication record:

```ts
interface HostInstance {
    readonly occurrence: OccurrenceHandle;
    readonly kind: HostKind;

    structural: StructuralSnapshot;
    declaredState: DeclaredStateSnapshot;
    contentBinding: ContentBindingSnapshot;
    eventSubscriptions: EventSubscriptionSnapshot;

    materialized: boolean;
}
```

This is not another application VDOM. It records what Rust has accepted for one occurrence.

### 6.3 Fiber and Iyon have complementary roles

```text
Fiber:
    component scheduling
    keyed identity
    host insertion/removal/move
    speculative rendering

Iyon TS HostInstance:
    native occurrence correspondence
    last accepted semantic snapshots
    plane-specific pending deltas
```

### 6.4 Speculative render safety

Render-phase host creation remains JS-only.

```text
React speculative render
    → create JS HostInstance candidate
    → no native occurrence yet

accepted React commit
    → native desired-state transaction
```

Abandoned work MUST NOT leak native resources.

### 6.5 Stable keys in component Surfaces

Long-lived Surface children MUST use stable application identities, not array indices.

This is necessary for:

```text
occurrence reuse
scroll anchoring
selection identity
cache reuse
residency
transactional insertion around tool calls
```

---

## 7. Generated semantic schemas

Iyon SHOULD generate finite schemas for application properties, content families/stages, events, and normalized host capabilities.

### 7.1 Property schema

Conceptual examples:

```text
property                 plane           value type

kind                     structural      HostKind
contentPort              structural      ContentPortHandle

width / height           state/geometry  SizeRule
margin / padding         state/geometry  Insets
gap                      state/geometry  Length
flex direction/wrap      state/geometry  enum
flex grow/shrink/basis   state/geometry  typed values
grid templates           state/geometry  track spec
grid placement/span      state/geometry  placement spec

background               state/present.  ColorIntent
foreground               state/present.  ColorIntent
opacity                  state/present.  scalar

text payload             content         bytes/content IR

onClick presence         interaction     subscription bit
onPointerMove presence   interaction     subscription bit
```

Generate:

```text
JSX types
normalization/equality
stable PropertyIds
plane classification
wire encoders
Rust descriptors/validators
capability requirements
observability names
docs/tests
```

### 7.2 Content-family and Funnel-stage schema

The schema SHOULD define stable content families and compatible stage transitions:

```text
Utf8Bytes
PlainText
SemanticText
UnifiedDiffText
AnsiText
ImageData
future domain families
```

Invalid pipeline composition should be a TypeScript error where statically knowable and a deterministic native validation error otherwise.

### 7.3 Host capability schema

Capabilities remain typed and finite:

```text
render.color
render.alpha
render.images
input.pointer
input.pointer_precision
input.ime
accessibility.semantic_tree
services.clipboard
services.windows
```

### 7.4 No implementation types in public schemas

Do not expose:

```text
taffy::Style
GPUI Platform types
SGR/OSC/Kitty numeric protocol identifiers
Rust arena indices
```

as portable application semantics.

---

## 8. Structural plane

A fact is structural when it changes retained graph identity, topology, or resource ownership.

Structural examples:

```text
occurrence create/destroy
parent-child insertion/removal/move/reorder
host semantic kind with distinct retained behavior family
root/portal ownership
ContentPort attachment identity
native controller/resource attachment identity
```

### 8.1 Layout configuration is state

These are geometry state when the occurrence remains the same:

```text
display flex/grid
flex direction/wrap/grow/shrink/basis
gap/margin/padding
width/height/min/max
alignment/justification
grid tracks/placement/span
```

Large downstream layout consequences do not make a property structural.

### 8.2 Content attachment identity is structural

Attaching a different `ContentPortId` changes retained destination identity and is structural.

Changing which Connector is selected for an already attached Port is content control, not structure.

### 8.3 Structural validation

Before desired-state acceptance, validate:

```text
unique occurrence parentage
no attachment duplication
host affinity
valid Port attachment kind
valid retained resource generations
```

Hard-invalid structural facts MUST be rejected before they become authoritative desired state.

---

## 9. Retained state plane

The state plane owns mutable semantic properties that preserve occurrence identity.

```text
geometry/layout state
presentation state
interaction-derived effective state
application style-state keys
```

### 9.1 TS retains accepted normalized values

For every host occurrence, TypeScript compares normalized semantic values against the last accepted snapshot.

```text
previous gap = 1
next gap     = 2

transport:
    occurrence 42
    PropertyId::Gap
    value 2
```

It does not send the complete style object.

### 9.2 Commit-local coalescing

Within one frontend commit, last write wins per `(occurrence, property)`.

```text
42.background = red
42.background = blue
42.background = green

sent:
42.background = green
```

### 9.3 React-declared base and retained overrides

Effective state is conceptually:

```text
React-declared base
    + explicit retained override
    + native host/control facts
    = effective state
```

If React changes a base value while an override masks it, the base still updates. Clearing the override reveals the newest base.

### 9.4 Rust classifies semantic effects

The shared classifier SHOULD remain semantic rather than encoding one layout engine's exact traversal:

```text
PRESENTATION
CONTENT_PROJECTION
LAYOUT_INPUT
INTERACTION_RUNTIME
HOST_ENVIRONMENT_DEPENDENT
STRUCTURE_GUARD
```

Taffy, content projection, Surface runtime, and backend paint refine the physical consequences.

### 9.5 Selection and editing are state/content changes, not unfreezing

Selection may invalidate only interaction/presentation ranges.

Editing may:

```text
change application component state
patch/replace a DocumentSource
replace one component with an editor component
change layout/state properties
```

None of these operations requires a generic `unfreeze()` transition.

### 9.6 Host-dependent presentation

A declared semantic color may realize differently:

```text
TrueColor terminal → RGB SGR
ANSI256 terminal   → palette quantization
ANSI16 terminal    → nearest supported semantic color
GPUI               → native RGBA
```

The declaration remains unchanged. Host Environment subrevision changes invalidate only the caches that depend on those facts.

### 9.7 Preserve color intent

The retained presentation model SHOULD distinguish at least conceptually:

```rust
enum ColorIntent {
    Rgba(Rgba),
    Indexed(u8),
    HostDefaultForeground,
    HostDefaultBackground,
    ThemeToken(ThemeColorId),
}
```

Exact representation is implementation-local.

---

## 10. Content entity model

### 10.1 Normative graph

The correct conceptual graph is:

```text
                         Funnel F
                    immutable specification
                              │
                              │ configures
                              ▼
Source S ───────────── Connector C ───────────── ContentPort P
```

Do not document Funnel and Connector as two serial runtime processors.

### 10.2 Source

A Source owns authoritative content independent of display.

For a text Source it may own:

```text
accepted UTF-8 bytes or semantic records
logical coordinate range
source revision
retention policy
append/replace/patch capabilities
optional seal state
width-independent annotations/provenance
```

A Source does not own:

```text
port geometry
scroll offset
terminal rows
GPUI glyph runs
current theme colors
connector activation
smoothing cursor for one display
```

Sources are environment/content-registry owned and may outlive any mounted component or host.

### 10.3 Source families

Do not expose one universal `Source<any>` with every mutation method.

Initial typed families SHOULD include:

```text
TextStreamSource
    append-monotonic UTF-8
    optional seal

TextBlockSource
    replace whole block/document

TextDocumentSource
    replace/patch editable document content

SemanticTextSource
    accept backend-neutral semantic text IR

RollingTextSource
    monotonic logical coordinates with retention/truncation
    intended initially for restartable plain/log content
```

A Source method unsupported by its family is absent from the API, not a runtime mode flag.

### 10.4 Funnel

A Funnel is an immutable typed value.

It owns configuration for:

```text
input decoding
semantic transformation
semantic delivery policy
physical projection policy
host capability requirements
minimum geometry requirements
configuration fingerprint
```

A Funnel does not own live progress.

Funnels may be reused across Connectors and MAY be interned by fingerprint.

### 10.5 Connector

A Connector is one exact binding:

```text
(SourceId, FunnelSpec, ContentPortId)
```

The endpoints and Funnel of an existing Connector are immutable. Switching Source or Funnel creates a candidate Connector and transactionally selects it on the Port.

A Connector owns relationship-local mutable state such as:

```text
consumed Source revision
incremental decoder/parser state
semantic IR revision/checkpoints
stable semantic prefix
replaceable unstable semantic tail
smoothing/reveal frontier
smoothing clock and lag state
width/viewport-dependent projection cache
last committed projected metrics
host capability validation status
cold/preparing/ready/error execution status
```

This state belongs neither to Source nor Port.

### 10.6 Connector existence test

The implementation MUST apply this test:

> If Connector has no identity-bearing mutable execution/lifecycle state beyond `{source, funnel, port}`, collapse it into an internal binding value instead of preserving a ceremonial runtime object.

Revision 5 expects Connectors to earn their identity through incremental execution, delivery, activation, transactional switching, and diagnostics.

### 10.7 ContentPort

A ContentPort is a retained UI destination structurally attached to one content-host occurrence.

It owns:

```text
accepted semantic content family
mounted/unmounted destination state
current geometry/clip binding
connector membership
0..1 selected Connector
current committed visible projection reference
candidate switch during frame preparation
```

It does not own:

```text
Source data
Funnel configuration
incremental parser state
smoothing progress
Surface scroll offset
```

### 10.8 Port multiplicity

```text
ContentPort.connectors: 0..N
ContentPort.selected:   0..1
```

Connector membership and selected Connector are content-control state. They do not change the structural root.

### 10.9 Source sharing

One Source may feed several Connectors:

```text
                           Source S
                        /            \
                       /              \
        Connector A: Markdown+Smooth   Connector B: Markdown+Immediate
                  │                              │
                Port A                         Port B
```

Each Connector has independent execution, delivery, width, viewport, and status.

### 10.10 Optional semantic parse cache sharing

The runtime MAY deduplicate immutable semantic results keyed by:

```text
Source identity/revision
semantic-transform fingerprint
retained source range/checkpoint
```

It MUST NOT share mutable Connector state such as reveal frontier, selected status, width projection, or error lifecycle.

Cache sharing is an optimization, not a change to ownership semantics.

### 10.11 Connector selection and switching

Port switching is transactional:

```text
current Connector A visible
        ↓
request Connector B
        ↓
prepare B against Source/Funnel/Port/HostEnvironment
        ↓
if successful:
    atomically select B
    dispose/cold A according ownership

if unsuccessful:
    A remains committed/visible
    B exposes typed status/error
```

No blank intermediate frame is allowed.

### 10.12 Mounted and cold execution demand

Selection and execution demand are distinct.

A Connector may remain selected while its Port is outside the derived-resident Surface window.

```text
selected + resident demand
    → execute/project/reveal

selected + cold destination
    → retain binding/status
    → perform no parser/smoothing/projection work
```

On renewed demand it catches up according to its delivery policy.

### 10.13 Connector lifecycle/status

A useful conceptual status family is:

```text
cold
waiting-for-mount
preparing
catching-up
ready
blocked-geometry
unsupported-host
retention-incompatible
errored
disposed
```

Exact enum spelling is deferred, but status MUST distinguish recoverable operating state from fatal invariant failure.

### 10.14 Ownership and disposal

Defaults:

```text
Source created by application/factory
    → application/resource owner disposes

implicit Port created by <Content>
    → host occurrence owns/disposes

implicit Connector created by <Content source funnel>
    → host occurrence owns/disposes/switches

hook-created Port/Connector
    → hook lifetime owns unless explicitly detached

externally created Port/Connector
    → caller owns; component only mounts/uses
```

A Source in use by live Connectors SHOULD reject disposal with a typed `SOURCE_IN_USE` error rather than silently cascading.

### 10.15 Automatic retry triggers

Recoverable Connector states retain their request and retry on relevant native changes:

```text
blocked-geometry
    → retry when Port geometry/projection requirements change

unsupported-host / capability unknown
    → retry when HostCapabilities refine

waiting-for-mount / cold-derived
    → retry when destination gains execution demand

source temporarily unavailable within accepted semantics
    → retry when Source revision/range becomes usable
```

The previous committed projection remains visible where one exists.

### 10.16 Shared Source wake routing

The environment-owned Source registry tracks weak subscriptions to affected Connectors/Hosts.

A Source mutation:

```text
advances Source revision
    ↓
marks only Hosts with selected or preparation-pending affected Connectors
    ↓
advances each Host pending epoch
    ↓
coalesces a wake through the environment HostScheduler/WakeBroker
```

Do not mirror the full Source→Connector→Host graph in JavaScript merely to schedule frames.

For JS-thread producers, a single environment-level wake hint/microtask MAY ask Rust to flush all pending live Hosts. Native pending epochs remain authoritative. GPUI/native producer paths require a genuine native event-loop redraw/wake mechanism.

### 10.17 Source mutation linearization

Each Source serializes mutation internally and assigns the next native Source revision. That native serialization order is the content-plane linearization order.

Do not add caller-generated sequence lanes without a concrete correctness requirement.

### 10.18 Bulk and control transport

```text
N-API/generated control ABI:
    Source/Port/Connector lifecycle
    Funnel descriptors
    status/diagnostics

direct same-image FFI data ABI:
    UTF-8 bytes
    semantic IR records/sidecars
```

The two paths are control and data lanes of one content architecture, not competing payload architectures.

---

## 11. Funnel composition and execution

### 11.1 Funnel is a normalized typed pipeline

The public API MAY look chainable:

```ts
markdown().smooth()
```

but the runtime SHOULD normalize it into a finite structured specification rather than an arbitrary callback pipeline.

Conceptually:

```rust
struct FunnelSpec<Input, Semantic> {
    input: InputAdapterSpec,
    semantic: SemanticTransformSpec,
    delivery: DeliveryPolicy,
    projection: ProjectionPolicy,
    requirements: FunnelRequirements,
    fingerprint: FunnelFingerprint,
}
```

This makes stage ownership and ordering explicit.

### 11.2 Normative stage order

```text
Source accepted data
    ↓
1. input decode/adaptation
    ↓
2. semantic transformation
    ↓
3. semantic delivery/reveal
    ↓
4. physical projection policy
    ↓
5. backend measurement/layout/paint
```

### 11.3 Input stage

Responsibilities may include:

```text
incremental UTF-8 validation/decoding
source family adaptation
source-coordinate bookkeeping
semantic checkpoint restoration
```

Malformed UTF-8 policy must be explicit per Funnel:

```text
replace invalid sequences
diagnostic + replacement
hard reject
```

The default text Funnel SHOULD use deterministic replacement/diagnostics, not undefined decoder behavior.

### 11.4 Semantic transform stage

Initial semantic transforms:

```text
PlainText
Markdown
UnifiedDiff
ANSI-to-semantic-text
IdentitySemanticText
```

This stage produces backend-neutral semantic IR.

### 11.5 Delivery stage

Initial delivery policies:

```text
Immediate
Smooth(config)
```

Delivery controls how much semantically parsed content is currently visible. It does not change Source acceptance.

### 11.6 Projection policy

Projection policy may configure:

```text
word/grapheme/no-wrap behavior
alignment
truncation/ellipsis policy
code/table presentation policy
semantic role mapping hooks
minimum useful geometry
selection enablement
```

Physical projection executes in Connector/backend context because width, viewport, text measurement, and Host Environment are destination-specific.

### 11.7 Funnel immutability

Funnel builders return immutable values. Changing a Funnel prop creates a new specification/fingerprint and therefore a candidate Connector switch.

Do not mutate a Funnel underneath several live Connectors.

### 11.8 Funnel type compatibility

Examples:

```text
Utf8 TextStreamSource
    + Markdown Funnel
    → SemanticText Port
    valid

SemanticTextSource
    + Markdown Funnel
    invalid: already semantic / wrong input family

ImageSource
    + Markdown Funnel
    invalid
```

Compatibility MUST exist in TypeScript types and Rust runtime validation.

### 11.9 No JS callback in the native hot pipeline

Iyon does not require application developers to implement Funnel execution in Rust.

It also MUST NOT call arbitrary JS transform callbacks for every content chunk on the native hot path.

Custom application formats have two supported directions:

1. Compose registered native/built-in Funnel stages.
2. Parse/transform in TypeScript and feed a `SemanticTextSource` through the content data ABI.

The second still bypasses React structure; it simply moves custom semantic parsing to the producer side.

### 11.10 Pipeline compilation

Creating or first using a Funnel SHOULD compile/normalize it into a compact native descriptor.

Pipeline validation errors are deterministic caller errors:

```text
incompatible stage families
multiple semantic transforms in an invalid order
Smooth applied to unsupported mutable source semantics
unsupported projection requirement
unknown generated stage ID
```

---

## 12. Semantic text IR

### 12.1 Purpose

Semantic text IR preserves meaning after parsing so style, selection, projection, and backend realization may change independently of original ingestion.

```text
Source bytes
    ↓ parse once/incrementally
SemanticTextDocument
    ↓ many resolutions
terminal rows / GPUI text / new theme / new effort / new width
```

### 12.2 Required properties

The IR MUST be:

```text
backend-neutral
host-independent
width-independent
revisioned
source-coordinate aware
incrementally replaceable
safe to retain
safe to restyle
```

It MUST NOT contain:

```text
terminal escape output
terminal cell positions
GPUI glyph atlas data
Taffy geometry
host-specific native Style IDs
resolved ANSI16/ANSI256 colors only
```

### 12.3 Conceptual structure

```rust
struct SemanticTextDocument {
    blocks: SemanticBlockSeq,
    source_range: LogicalRange,
    revision: SemanticRevision,
    stable_frontier: LogicalOffset,
    diagnostics: DiagnosticSeq,
}
```

Blocks/inlines may express:

```text
paragraph
heading level
list/list item
quote
code block + language
inline code
emphasis/strong/strike
link + target
line break
rule
table
plain text span
diff hunk/header/add/remove/context
semantic annotation/provenance
```

Exact representation is deferred.

### 12.4 Semantic style, not final color

Markdown heading or diff addition should retain semantic roles:

```text
MarkdownHeading(level=2)
DiffAddition
CodeKeyword
Link
MutedAnnotation
```

Theme, effort state, selection, and Host Environment resolve these roles later.

This is why already displayed Markdown/diff text can change color after the fact without reparsing or React reconstruction.

### 12.5 Source ranges and stable identities

Semantic nodes/spans SHOULD carry logical source ranges and stable incremental identities where possible.

These support:

```text
selection
copy
annotations
incremental parser reuse
diagnostics
smooth reveal frontier
cache reuse
```

### 12.6 Stable prefix and unstable tail

Streaming grammars may reinterpret the trailing region as new bytes arrive.

The semantic parser therefore exposes:

```text
stable prefix
replaceable unstable tail
semantic-ready frontier
```

The stable prefix is monotonic for append-only input.

The unstable tail MAY be replaced/restyled as delimiters or blocks complete.

This matches the useful incremental pattern seen in OpenTUI's Markdown parser: unchanged leading tokens are reused while a configurable trailing token region remains unstable and is reparsed.

### 12.7 Markdown semantics

Markdown Funnel behavior:

```text
incrementally parse accepted Source revision
reuse stable semantic prefix
reparse unstable tail
conceal syntax according policy
preserve source ranges
emit semantic roles
finalize tail when Source seals
```

Malformed or incomplete Markdown during streaming is not a frame-fatal error. It remains a valid unstable semantic tail according to deterministic parser fallback rules.

### 12.8 Diff semantics

Unified diff input SHOULD become semantic records such as:

```text
file header
hunk header
context line
addition
removal
no-newline marker
metadata
```

Diff colors are resolved later. The IR remains selectable and restylable.

### 12.9 ANSI semantics and safety

ANSI Funnel MUST parse supported display intent into semantic roles/ColorIntent rather than forwarding arbitrary control bytes to the terminal backend.

At minimum:

```text
SGR style/color → semantic style intent
OSC 8 link      → semantic link if allowed
unsafe cursor/window/control operations → stripped or diagnostic
```

Raw untrusted ANSI MUST never bypass the renderer and write directly to the host terminal.

### 12.10 SemanticTextSource

Advanced applications may push semantic blocks/spans directly through a compact content ABI.

This is the extension path for custom Markdown variants, log parsers, diagnostics, or domain formats when no native Funnel exists.

### 12.11 Annotation lifetime

Annotations use logical source coordinates/semantic IDs and MUST remain host-independent.

Retention/truncation policy defines whether a crossing annotation:

```text
clips
disappears
becomes diagnostic
```

per annotation kind. It may not silently reference evicted bytes.

---

## 13. Smoothing and semantic delivery

### 13.1 Definition

> **Smoothing:** a Connector-local delivery policy that advances the visible semantic frontier over time using a Rust-owned clock.

It is not:

```text
React animation
Source backpressure
parser input throttling
terminal frame delay
raw byte trickling by default
```

### 13.2 Why smoothing follows semantic parsing

Normative order:

```text
accepted UTF-8
    ↓
Markdown/diff/ANSI semantic parsing
    ↓
semantic IR
    ↓
Smooth delivery
```

This avoids deliberately revealing raw Markdown delimiters and allows smoothing to operate on graphemes/semantic text units.

Future input throttling, if needed, must be a separately named policy rather than overloading `Smooth`.

### 13.3 Source acceptance vs visibility

Track distinct frontiers:

```text
Source.accepted_revision/frontier
Connector.semantic_ready_frontier
Connector.visible_semantic_frontier
Connector.physical_projection_revision
```

Source append returns after content acceptance, not after smoothing displays the bytes.

### 13.4 Connector-local execution

Two Connectors consuming the same Source can differ:

```text
Connector A:
    Markdown + Immediate
    visible at semantic-ready frontier

Connector B:
    Markdown + Smooth(40 graphemes/s)
    visible behind semantic-ready frontier
```

The Source is stored once.

### 13.5 Semantic reveal units

Smoothing SHOULD reveal semantic grapheme/text units, respecting block/line boundaries and concealed syntax.

It MAY use adaptive batching for efficiency, but MUST preserve deterministic ordering and never split invalid UTF-8/grapheme boundaries.

### 13.6 Unstable Markdown tail

The parser may revise the current unstable tail while some of it is visible.

Allowed behavior:

```text
visible text remains selected by logical source frontier
semantic roles/layout of the unstable tail may revise
stable prefix remains reusable
```

For example, trailing text may become emphasized when a closing delimiter arrives. That dynamic restyling is a feature, not a reason to re-run React.

### 13.7 Catch-up policy

Default behavior SHOULD be:

```text
initial mount with pre-existing Source content:
    show existing backlog immediately
    smooth newly accepted content

cold → resident reactivation:
    catch up immediately to current semantic-ready frontier
    smooth subsequent content
```

This avoids replaying minutes of old data when a component is mounted or returns from cold residency.

An explicit `replay` policy MAY smooth retained backlog.

### 13.8 Bounded lag

Smooth configuration SHOULD support a bounded-lag/adaptive policy so producer bursts do not create unbounded UI delay.

Conceptual options:

```text
target rate
minimum/maximum batch
maximum lag
catch-up multiplier
newline/block preference
on-seal drain/flush policy
```

Exact algorithm is benchmark-tunable.

### 13.9 Source seal

For append-only Sources, `seal()` means no further appends are accepted and allows semantic parser finalization.

It does not freeze presentation.

Default Smooth behavior SHOULD drain with bounded catch-up and guarantee eventual convergence to the final semantic frontier. An explicit policy MAY flush immediately on seal.

### 13.10 Mutable document compatibility

Smooth delivery is naturally defined for append-monotonic Sources.

For replace/patch document mutations, v1 SHOULD either:

```text
reset visible frontier to immediate current document
```

or reject Smooth as incompatible.

Do not invent ambiguous animation semantics for arbitrary edits in the first implementation.

### 13.11 Native scheduling

Smooth ticks advance native pending epochs and schedule frames without TypeScript.

```text
Rust clock tick
    ↓
Connector visible frontier advances
    ↓
projection/metrics maybe change
    ↓
Taffy/paint as required

React = 0
TS→Rust = 0
```

### 13.12 Cold semantics

A cold Connector owns no active smoothing timer, reveal queue, semantic projection, or width cache.

The Source continues advancing independently.

---

## 14. Source retention and restartability

### 14.1 Retention is Source policy

A Source may retain:

```text
entire document/segment
bounded byte range
time window
bounded semantic records
```

Retention does not belong to the Surface.

### 14.2 Semantic Funnel restartability

An inactive Connector may need to construct parser state from retained Source data.

Funnels therefore declare restartability characteristics:

```text
restartable from arbitrary retained head
restartable only from checkpoint/document start
checkpointable
requires complete retained document
```

### 14.3 Markdown v1 retention rule

For streamed assistant Markdown segments, v1 SHOULD retain the full segment until the containing conversation item is released/archived.

Do not combine drop-oldest raw retention with incremental Markdown correctness unless semantic checkpoints/block-boundary truncation are explicitly implemented.

### 14.4 Rolling streams

`RollingTextSource` with drop-oldest retention is initially appropriate for restartable plain/log funnels.

Connecting a retention-incompatible Funnel returns deterministic `RETENTION_INCOMPATIBLE` status/error.

### 14.5 Semantic checkpoints

Future Funnel implementations MAY emit checkpoints that allow restart from a retained offset.

Checkpoint format is Funnel-specific execution data and MUST NOT leak into Source's generic public API beyond an opaque compatibility contract.

---

## 15. React content API: progressive control

The public API uses progressive disclosure.

### 15.1 Level 1: batteries included

```tsx
<Markdown source={stream} smooth />
<Diff source={diffSource} />
<PlainText source={logSource} />
```

The component resolves:

```text
content family
default Funnel
default delivery policy
implicit ContentPort
implicit Connector
activation/lifecycle
backend projection
```

The user initially needs only this model:

```text
React component
    = where/how content appears

Source
    = content that may advance independently of React
```

### 15.2 Level 2: generic Funnel control

```tsx
<Content
    source={stream}
    funnel={markdown({
        projection: { wrap: "word" },
    }).smooth({
        targetGraphemesPerSecond: 40,
        maximumLagMs: 700,
    })}
/>
```

### 15.3 Level 3: explicit resource control

Advanced users MAY explicitly manage Port and Connector:

```tsx
function AdvancedContentPane({ source }: Props) {
    const port = useContentPort(SemanticTextContent);

    const connector = useContentConnector({
        port,
        source,
        funnel: markdown().smooth(),
        selected: true,
    });

    useConnectorDiagnostics(connector, reportStatus);

    return <Content port={port} />;
}
```

Exact hook names are working names.

### 15.4 Implicit mode

In:

```tsx
<Content source={source} funnel={funnel} />
```

one host occurrence owns an implicit Port and implicit Connector.

When props change:

```text
same Source + same Funnel fingerprint
    → keep Connector

new Source or new Funnel fingerprint
    → create candidate Connector
    → transactionally switch Port
    → old projection remains until ready
```

### 15.5 Explicit mode

In:

```tsx
<Content port={port} />
```

the caller controls Connector creation/selection.

The prop API SHOULD use a discriminated union so implicit and explicit ownership cannot be ambiguously combined.

Wrong:

```tsx
<Content
    port={port}
    source={source}
    connector={connector}
    funnel={funnel}
/>
```

unless a single clear ownership mode is defined.

### 15.6 Source sharing is ordinary React usage

```tsx
<>
    <Markdown source={output} smooth />

    <PreviewPane>
        <Markdown source={output} smooth={false} />
    </PreviewPane>
</>
```

This creates two Ports and two Connectors over one Source.

### 15.7 Source prop change

```tsx
<Markdown source={sourceA} />
```

becoming:

```tsx
<Markdown source={sourceB} />
```

keeps the React occurrence/Port and transactionally switches to a new Connector binding.

### 15.8 Funnel prop change

Changing:

```tsx
<Markdown source={source} smooth={false} />
```

to:

```tsx
<Markdown source={source} smooth />
```

creates/selects a Connector with the new immutable Funnel specification. It does not mutate shared Funnel state.

### 15.9 Refs and diagnostics

Convenience components MAY expose a ref for:

```text
current Connector status
Source/Port identities for diagnostics
flush/visibility barrier
selection/copy operations
explicit resume-follow or scroll-to-occurrence through Surface refs
```

A ref MUST NOT be required for normal streaming.

---

## 16. Component-local defaults and factory pattern

The framework follows:

> **Convention-first, injectable underneath, scoped locally.**

### 16.1 Defaults are not one app-global content policy

One Surface may contain:

```tsx
<AssistantText source={liveText} />        // Markdown + Smooth
<ToolResult source={finishedOutput} />      // block/immediate
<DiffResult source={patch} />               // diff/immediate
<LogTail source={logs} />                   // plain stream/no smoothing
```

Each component resolves its own defaults.

### 16.2 Component factory

Iyon SHOULD provide a typed component factory or equivalent helper:

```tsx
const AssistantText = defineContentComponent({
    displayName: "AssistantText",
    sourceFamily: Utf8Text,
    defaultFunnel: markdown({
        projection: { wrap: "word" },
    }).smooth({
        targetGraphemesPerSecond: 40,
    }),
});

const ToolOutput = defineContentComponent({
    displayName: "ToolOutput",
    sourceFamily: Utf8Text,
    defaultFunnel: plainText({
        delivery: "immediate",
        projection: { wrap: "word" },
    }),
});
```

Usage stays simple:

```tsx
<AssistantText source={segment.source} />
<ToolOutput source={tool.output} />
```

This factory bakes in local defaults while still lowering to ordinary `<Content>` primitives.

### 16.3 Application item factory

This is distinct from the component factory.

The application item factory creates semantic model identity plus resources:

```ts
function createAssistantTextSegment(id: ConversationItemId) {
    const source = createTextStreamSource();

    return {
        source,
        item: {
            kind: "assistant-text" as const,
            id,
            source,
        },
    };
}
```

The component factory defines **how a kind of item renders by default**.

The item factory creates **one actual model item and its Source**.

### 16.4 Default precedence

Normative resolution order:

```text
1. explicit component-instance prop
2. nearest compatible typed subtree override
3. defaults baked into the component definition/factory
4. framework built-in default
```

A component MAY choose whether it inherits a subtree default for each policy family.

### 16.5 Scoped defaults

A typed React Context MAY provide subtree policy:

```tsx
<MarkdownDefaults
    value={{
        delivery: smooth({ targetGraphemesPerSecond: 60 }),
    }}
>
    <AssistantArea />
</MarkdownDefaults>
```

This is intentionally scoped. Placing it at the application root is possible but not the canonical answer for heterogeneous content.

React Context changes rerender consumers, so defaults SHOULD be stable configuration rather than per-frame state.

### 16.6 No arbitrary global Connector factory

Do not expose an unrestricted app-global hook that can replace Connector construction for every content component and violate lifecycle invariants.

Injectable policies/stage registries remain typed and scoped. Explicit advanced resource APIs provide full control when needed.

### 16.7 Same implementation underneath

```tsx
<Markdown source={stream} smooth />
```

MUST lower to the same conceptual operations as:

```text
resolve FunnelSpec
create/retain ContentPort
create Connector(Source, Funnel, Port)
select Connector
manage occurrence-owned lifecycle
```

There is no special fast-path renderer with different semantics.

---

## 17. Surface model

### 17.1 Framework primitive

The generic primitive is `ScrollSurface` or `FlowSurface`, not a chat-specific `History` object.

A Surface contains ordered component occurrences.

```tsx
<ScrollSurface direction="column" followEnd>
    {children}
</ScrollSurface>
```

### 17.2 Application-level HistorySurface

The reference app may define:

```tsx
function HistorySurface({ children }: PropsWithChildren) {
    return (
        <ScrollSurface
            direction="column"
            followEnd
            anchor="visible-start"
            overscan={2}
        >
            {children}
        </ScrollSurface>
    );
}
```

`HistorySurface` is a named React component, not a distinct native mixed-content storage engine.

### 17.3 Children are components

Example conversation:

```tsx
<HistorySurface>
    <UserMessage message={userMessage} />
    <AssistantText source={segment1} />
    <ToolCall call={call} />
    <ToolResult result={result} />
    <AssistantText source={segment2} />
</HistorySurface>
```

Tool calls/results are ordinary components.

Every resumed assistant text segment is another content-backed component.

### 17.4 Raw React text normalization

React raw text children MAY be supported as syntax sugar, but the renderer normalizes them into retained text occurrences before the Surface/runtime boundary.

The Surface itself never stores a special raw text stream.

### 17.5 Surface responsibilities

A Surface owns:

```text
ordered child participation
Taffy container/layout relationship
viewport and clip
scroll offset
follow policy
scroll anchoring
focus/selection integration where spatial
paint culling
overscan/residency policy
extent/cache bookkeeping
```

### 17.6 Surface non-responsibilities

A Surface does not own:

```text
Source bytes
Markdown parsing
Diff parsing
smoothing policy
Connector parser state
child theme semantics
tool-call lifecycle
chat completion state
```

### 17.7 Content growth

An `AssistantText` component uses content-sized height.

```text
Source append
    ↓
semantic IR/reveal changes
    ↓
projected intrinsic height changes
    ↓
Taffy recomputes affected layout
    ↓
following siblings move
    ↓
Surface applies follow/anchor policy
```

No Surface-specific append API is involved.

### 17.8 Component heterogeneity

Surfaces support arbitrary child composition:

```text
text
Markdown
diff
tool cards
forms
images where supported
editable messages
approval controls
custom app components
```

This is one reason the special mixed text/component History representation must disappear.

### 17.9 Surface policy is independent of content policy

Normative statement:

> **Surface policy is spatial and lifecycle policy. Content policy belongs to each child attachment.**

A Surface-level `smoothAllContent` option is architecturally invalid.

---

## 18. Cacheability, residency, and virtualization

### 18.1 No generic live/completed/frozen renderer state

The runtime does not model:

```text
LIVE
COMPLETED
FROZEN
```

as presentation lifetime states.

Applications may model tool/message completion. Sources may seal. Neither event makes current layout/presentation immutable.

### 18.2 Cache validity replaces freeze

Derived work is reusable when its declared inputs still match.

Conceptual dependency keys:

```text
semantic structure generation
content semantic/projection revision
layout property revision
layout constraints
text-measurement environment revision
presentation/theme/state revision
Host Environment presentation subrevision
interaction revision
geometry/clip
```

Do not put one universal HostEnvironment revision into every cache key. Use relevant subrevisions so a pointer capability probe does not invalidate text layout.

### 18.3 Reversible invalidation

Examples:

```text
effort changes
    → presentation cache invalid
    → layout cache remains if geometry unaffected

selection changes
    → affected paint/selection ranges invalid
    → content parse/layout remain reusable

viewport width changes
    → projection/text measure/Taffy layout invalid

edit patches document
    → Source/semantic/projection revisions advance

Host palette changes
    → resolved presentation invalid
    → semantic IR remains reusable
```

### 18.4 Resident occurrence guarantee

> **Any occurrence resident on an Iyon-controlled Surface remains an active participant in relevant state, content, layout, presentation, Host Environment, accessibility, and interaction changes.**

This directly forbids the current bug class where old visible user-message borders stop responding while newer UI still updates.

### 18.5 Residency levels

Conceptually:

```text
VISIBLE
    in viewport
    full projection/layout/paint participation

NEAR
    in overscan
    prepared for imminent visibility

COLD_DERIVED
    compact retained occurrence/binding remains
    expensive projection/paint artifacts may be dropped
    last known extent/metadata may remain
```

The application/React identity remains mounted unless a separate virtual-data API explicitly unmounts it.

### 18.6 Derived eviction, not semantic destruction

Cold residency MAY drop:

```text
paint cache
terminal row projection
GPUI shaped runs/display list
Connector smoothing timer
width-specific content projection
heavy child control caches
```

It retains enough compact state to preserve:

```text
occurrence identity
Source/Funnel/Port binding identity
selection/application identity
last known extent where valid
scroll anchor continuity
```

### 18.7 Connector demand from Surface residency

Surface residency contributes execution demand:

```text
visible/near child
    → selected Connector demanded

cold child
    → selected Connector binding retained
    → Connector execution cold
```

Re-entry follows Connector catch-up policy.

### 18.8 First implementation staging

The first implementation MAY stage optimization:

1. Retain all semantic occurrences and Taffy layout.
2. Cull offscreen paint.
3. Add near/cold projection and paint residency.
4. Add deeper extent-based virtualization only when needed.

Do not block the architecture on immediately virtualizing every React child.

### 18.9 React virtualization is a separate future API

For extremely large datasets, a future `VirtualSurface` MAY use:

```text
data source
stable item keys
item factory/render function
extent estimates
windowed React mounting
```

This is separate from the baseline Surface's native derived-residency system.

React Native's VirtualizedList demonstrates the memory value and state-lifetime tradeoff of unmounting outside a finite render window. Iyon SHOULD keep application/resource state external before introducing that optimization.

### 18.10 Extent caches

Cold children may retain last-known extent keyed by the constraints under which it was measured.

A width/measurement-environment change invalidates that extent.

When a stale extent is corrected, Surface anchoring prevents a visible jump where possible.

### 18.11 Interaction promotes residency

Focus, selection, find-result priority, pointer capture, editing, or accessibility activation MUST promote the relevant occurrence enough to provide correct behavior.

### 18.12 Cacheability is not a user-facing mode

Do not require application code to call:

```text
freeze()
unfreeze()
markStable()
```

The runtime derives reuse from revisions/dependencies.

---

## 19. Scroll, follow-end, and anchoring semantics

### 19.1 Scroll ownership

Scroll offset and follow state belong to Surface/viewport runtime, not ContentPort or Connector.

This preserves scroll state across content-source switches.

### 19.2 Follow-end state machine

`followEnd` enables a policy with states:

```text
Disabled
Following
SuspendedByUser
```

Behavior:

```text
Following:
    content growth pins viewport end

user scrolls away from end:
    → SuspendedByUser

user returns within end threshold:
    → Following

explicit resumeFollowEnd():
    → Following + scroll to end
```

Do not continuously fight the user's manual scroll.

### 19.3 End threshold

The at-end threshold is backend physical policy:

```text
terminal:
    small cell threshold

GPUI:
    pixel threshold
```

The semantic state remains `Following`/`SuspendedByUser`.

### 19.4 Scroll anchoring

When not following end, Surface SHOULD preserve a stable visible anchor across layout changes.

Anchor representation:

```text
stable child occurrence identity
+ local offset within that occurrence
```

Priority candidates SHOULD include:

```text
focused occurrence
active selection/find result
otherwise first suitable visible occurrence near viewport start
```

This follows the useful principle of CSS scroll anchoring: preserve the user's visible document position when content outside the viewport changes.

### 19.5 Anchor adjustment

If the anchor's block-start position changes from `y0` to `y1`, adjust scroll offset by the delta so the anchor remains at the same viewport location.

### 19.6 Following wins over start anchoring

When `Following`, the logical end is the anchor. Growth pins the end instead of preserving the first visible item.

### 19.7 Insertions/removals above viewport

Stable occurrence keys let Surface account for:

```text
components inserted above anchor
components removed above anchor
components expanding/collapsing above anchor
cold extent corrections above anchor
```

### 19.8 Anchor disappearance

If the anchor occurrence is removed:

1. prefer the nearest surviving visible successor;
2. otherwise predecessor;
3. otherwise clamp scroll range;
4. if following, pin end.

### 19.9 Tool/text segment growth

A tool card expanding or streamed text growing is ordinary child extent change. Surface applies the same anchoring/follow rules; it does not need message-type-specific code.

### 19.10 Surface refs

Advanced refs MAY provide:

```text
scrollToOccurrence(id)
scrollToEnd()
resumeFollowEnd()
suspendFollowEnd()
getVisibleOccurrences()
```

---

## 20. React commit and native transaction seam

### 20.1 React render phase remains pure with respect to native authority

React may create speculative HostInstances and update payloads. Native authoritative state is not mutated during render.

### 20.2 Validate hard errors before desired commit

TypeScript normalization and generated schemas MUST reject ordinary hard-invalid inputs before the accepted React commit where possible:

```text
invalid property value
invalid host kind/property combination
incompatible Source/Funnel family
ambiguous implicit/explicit Content ownership
invalid retained handle generation already known in TS
```

### 20.3 React commit stages plane-specific sections

```ts
commitUpdate(instance, type, oldProps, newProps) {
    const normalized = schema[type].normalize(newProps);

    structuralBatch.diff(instance.structural, normalized.structural);
    stateBatch.diff(instance.declaredState, normalized.state);
    contentBatch.diff(instance.contentBinding, normalized.content);
    eventBatch.diff(instance.eventSubscriptions, normalized.events);
}
```

### 20.4 Frontend commit envelope

```text
FRONTEND COMMIT N

STRUCTURAL
STATE
REACT-OWNED CONTENT REPLACEMENTS
CONTENT BINDING CONTROL
EVENT SUBSCRIPTIONS
```

The sections remain semantically separate inside one atomic desired-state acceptance.

### 20.5 Desired state vs visible frame

React commit establishes the frontend's desired semantic state.

Native transaction acceptance MUST establish a corresponding desired native revision before `resetAfterCommit`/renderer commit completion returns normally.

The desired revision may become visible on a later native frame.

```text
React desired revision N accepted
    ↓
old visible frame remains until complete frame N prepares
    ↓
frame N commits atomically
```

### 20.6 Ordinary native desired-state acceptance

All permanently failing ordinary work SHOULD occur before or during a prepare phase.

After successful prepare, authoritative desired-state commit SHOULD be infallible.

Transient frame preparation failure does not roll back React desired state; it retains pending work and leaves the previous visible frame on screen.

### 20.7 Hard invariant failures

A native invariant failure after the frontend commit indicates a renderer/runtime bug. It follows the fatal runtime error path rather than silently diverging TS and Rust or rebuilding indiscriminately.

### 20.8 Automatic flush errors

Automatic frame errors are stored on the Host runtime and MAY notify an error observer.

The next explicit barrier reports them:

```ts
await host.flush();
await host.whenVisible(desiredRevision);
```

Exact API spelling is deferred.

### 20.9 React effects and visibility

After a successful React commit, effects may assume the native desired semantic revision exists.

They MUST NOT assume pixels/cells have already been presented unless awaiting a visibility barrier.

### 20.10 Host Environment snapshot

Each candidate frame captures one consistent Host Environment snapshot/subrevisions.

An environment update arriving mid-frame schedules the next pending epoch.

### 20.11 Source append is outside React commit

Source mutations are linearized by the Source/content runtime and participate in the next native frame transaction without creating a fake React commit.

---

## 21. Event and callback model

Events are a bridge concern, not a fourth rendering plane.

### 21.1 Callback ownership

JavaScript owns application closures.

```text
JS callback registry:
    (OccurrenceHandle, EventKind) → callback

Rust:
    occurrence → subscription bitmask
```

Changing callback identity while subscription presence remains true requires no native update.

### 21.2 Native event flow

```text
backend input
    ↓
protocol/platform decoder
    ↓
semantic input event
    ↓
hit testing / focus / capture / selection
    ↓
event { occurrence, kind, payload }
    ↓
Rust→JS event lane
    ↓
JS registry
    ↓
React/application callback
```

### 21.3 Pointer is not GPUI-only

Terminals may support:

```text
button events
movement
hover-like enter/leave
wheel
selection drag
sometimes pixel-coordinate reports
```

Pointer precision is a HostCapability.

### 21.4 Event subscription drives feature demand

```text
first onPointerMove listener
    ↓
subscription bit turns on
    ↓
FeatureDemand::PointerMotion +1
    ↓
Host Feature Broker
    ↓
terminal protocol activated if supported/policy permits
```

When the last consumer disappears, on-demand mode may be disabled.

### 21.5 Focus/action model

Keyboard-first action dispatch and focus traversal SHOULD remain portable across terminal and GPUI.

Pointer input adds a modality; it does not replace the action/focus model.

### 21.6 Native hot controls

A native text editor, selection controller, scroll interaction, or smooth Connector MAY update retained native state and schedule a frame before sending a higher-level application event to JS.

### 21.7 Event ordering

Events emitted from one host input sequence MUST preserve native semantic order.

Event delivery is separate from Source mutation ordering, but callbacks that mutate Sources/state create ordinary subsequent pending work.

### 21.8 Backpressure/coalescing

High-frequency pointer-move/scroll events MAY coalesce when semantics permit.

Discrete events such as click, key press, paste, activation, and service result MUST NOT be silently dropped.

---

## 22. Host Environment layer

Host Environment is a first-class Rust subsystem between semantic runtime state and physical realization.

### 22.1 Ownership concepts

```text
HostProfile
HostCapabilities
EnvironmentState
HostPolicy
FeatureDemand / FeatureBroker
ActiveHostFeatures
HostServices
```

These concepts have distinct lifecycles.

### 22.2 HostProfile

Comparatively stable host identity:

```rust
enum BackendKind {
    Terminal,
    Gpui,
}

enum CoordinateSpace {
    Cell,
    Pixel,
}
```

Terminal diagnostics may include terminal identity, multiplexer, and remote-session classification without making them portable app semantics.

### 22.3 HostCapabilities

Capabilities describe what Iyon currently believes the host can support.

```rust
struct HostCapabilities {
    revision: u64,
    render: RenderCapabilities,
    input: InputCapabilities,
    accessibility: AccessibilityCapabilities,
    services: ServiceCapabilities,
}
```

### 22.4 Unknown is first-class

```rust
enum Availability<T> {
    Unknown,
    Supported(T),
    Unsupported,
}
```

Terminal startup commonly refines heuristic state after probe replies.

### 22.5 Raw facts vs normalized capabilities

Raw terminal facts:

```text
TERM/COLORTERM
terminfo
Kitty keyboard/graphics
Sixel
SGR mouse/pixel mouse
OSC 8 / OSC 52
focus reporting
bracketed paste
synchronized output
Unicode width modes
multiplexer state
```

Normalized Iyon concepts:

```text
render.color = TrueColor
input.pointer = Supported(CellPointer { hover, buttons, drag })
render.images = Supported(...)
services.clipboard = Supported(...)
```

### 22.6 EnvironmentState

Current observations, not support declarations:

```text
viewport
pixel resolution if known
scale factor
light/dark appearance
terminal palette/default colors
host focus
last input modality
```

### 22.7 HostPolicy

Examples:

```text
pointer tracking: auto / disabled / forced
pointer movement: on-demand / always / disabled
images: auto / kitty / sixel / blocks / disabled
clipboard: local-only / terminal-only / best-available / disabled
notifications: enabled / disabled
remote host services: allowed / denied
```

### 22.8 Feature Broker

Combines:

```text
capability
+ policy
+ current demand leases
= active host modes
```

Modal terminal features include pointer tracking, focus reports, bracketed paste, enhanced keyboard, and synchronized output.

### 22.9 HostServices

Services are attempted operations:

```text
clipboard read/write
notifications
open URL
window creation/management
system prompts/dialogs
attention request
```

Capability helps plan UI. Operation result is authoritative.

```ts
const result = await host.clipboard.writeText(text);
```

### 22.10 Accessibility

React declares semantic accessibility independently of backend realization.

GPUI maps it to AccessKit/native trees.

Terminal retains labels/roles/actions for focus, help, tests, and future integrations without claiming a desktop accessibility tree.

### 22.11 Revisioned snapshot

```rust
struct HostEnvironmentSnapshot {
    revision: u64,
    profile: HostProfile,
    capabilities: HostCapabilities,
    environment: EnvironmentState,
    policy_revision: u64,
    active_features: ActiveHostFeatures,
}
```

Prefer relevant subrevisions in cache dependencies.

### 22.12 Rust→TS observation

Most Host Environment changes remain native.

Explicit React observation uses one external store:

```text
Rust snapshot/delta
    ↓
HostEnvironmentStore in JS
    ↓
selector subscription
    ↓
useSyncExternalStoreWithSelector or equivalent
```

Working hooks:

```ts
useBackend()
useHostCapability(selector)
useHostEnvironment(selector)
```

React core's `useSyncExternalStore` provides external-store subscription; selector behavior must be implemented with a selector wrapper/equivalent rather than assumed to exist in the base hook.

### 22.13 Observation is opt-in

Prefer semantic declaration and native realization:

```tsx
<Box background="#e65353" />
```

Use capability hooks only when component composition itself changes.

### 22.14 Provider boundary

Backend adapters publish observations through a typed sink and apply active feature sets through a typed provider interface.

### 22.15 Terminal provider

Inputs may include Termwiz capability information, environment hints, terminfo, active queries, palette/theme/resolution replies, terminal identity, multiplexer detection, and remote policy.

### 22.16 Trust boundary

Capability parsing accepts only bounded recognized reply shapes associated with expected probe lifecycle.

Untrusted application content must not become capability authority.

### 22.17 GPUI provider

GPUI provider publishes window bounds, scale, appearance, pointer/input state, accessibility adapter state, and platform services as normalized Iyon observations/capabilities.

### 22.18 Synthetic testing

Tests inject deterministic Host Environment snapshots and transitions without depending on the CI terminal emulator.

---

## 23. Internal Rust runtime ownership

The Rust runtime owns:

```text
retained occurrences/topology
retained state and effective-state resolution
Sources/Funnels/Connectors/Ports
semantic content IR and projection state
Surface viewport/anchor/residency state
event subscriptions/focus/selection/control
Host Environment and Feature Broker
Taffy integration
presentation resolution/damage
frame transactions/epochs
backend scheduling
```

Conceptual dependency direction:

```text
semantic retained runtime
    ├── content runtime
    │      ├── Sources
    │      ├── Funnel execution
    │      ├── semantic IR
    │      ├── Connectors
    │      └── Ports
    ├── Surface runtime
    ├── Host Environment
    ├── Taffy integration
    └── backend presenter/input adapter
```

Neither GPUI Elements nor terminal rows are authoritative semantic state.

---

## 24. Layout architecture: Taffy on both hosts

### 24.1 Shared typed contract

Iyon exposes an intentionally scoped but substantial Flexbox/Grid contract:

```text
display flex/grid
positioning where portable
width/height/min/max
margin/padding
flex direction/wrap/grow/shrink/basis
alignment/justification
gap
grid templates/auto tracks/flow
placement/spans
```

It does not expose a CSS parser/cascade/DOM compatibility promise.

### 24.2 Terminal path

```text
retained Iyon occurrences
    ↓
Taffy integration
    ↓
terminal leaf/content measurement
    ↓
cell-space layout
    ↓
integral rectangles
    ↓
terminal paint
```

### 24.3 GPUI path

```text
retained Iyon state
    ↓
ephemeral GPUI Element/style lowering
    ↓
GPUI/Taffy
    ↓
pixel geometry / Scene
```

### 24.4 Terminal-specific content measurement

Taffy does not replace:

```text
Unicode/grapheme cell width
word/grapheme wrapping
semantic text projection
row viewport/history controls
```

### 24.5 Rounding

Terminal rounding must be deterministic and cumulative-edge based to avoid gaps/overlaps.

### 24.6 Cache/invalidation

Taffy owns general layout caching/recomputation after migration.

Iyon retains distinct content, presentation, Surface, and damage invalidation.

### 24.7 Legacy engine lifecycle

```text
PERF-13:
    current engine required

migration:
    oracle/fallback only

Taffy stable:
    delete custom allocator
    delete redundant general-layout dependency machinery
    delete selector/feature
```

### 24.8 Bridge invariance

Taffy internal representation must never broaden TS transport.

---

## 25. Backend responsibilities

### 25.1 Terminal host

```text
HostEnvironment detection/provider
terminal protocol activation/restoration
Taffy cell-space integration
terminal text/content measurement
Surface/cell paint and damage
input decoding and semantic normalization
history/scrollback integration
terminal services and protocol extensions
```

### 25.2 GPUI host

```text
HostEnvironment provider
GPUI Element lowering
GPUI/Taffy and font measurement
prepaint/paint/GPU surfaces
pointer/keyboard/IME/window integration
AccessKit
platform services
```

### 25.3 Shared runtime

```text
React frontend
three-plane transport
identity/transactions
state/content schemas
Source/Funnel/Connector/Port
semantic text IR
Surface semantics
Host Environment model
focus/action/event identities
```

### 25.4 Raw backend details stay local

Portable application code does not directly depend on SGR/OSC/Kitty numbers, GPUI Platform enums, or AccessKit request types except through explicit backend diagnostics/extensions.

---

## 26. Native frame rule

Once native semantic state is current, ordinary frames do not require TypeScript.

```text
cursor blink / native animation
    TS→Rust = 0

native hover/focus/selection
    TS→Rust = 0

Smooth reveal tick
    TS→Rust = 0
    React = 0

Source append
    content bytes only
    React = 0

Host color capability refinement
    native presentation invalidation
    React = 0 unless explicitly observing

Surface scroll/anchor adjustment
    native state/layout/paint
    React = 0
```

> **Most native frames, content-delivery ticks, and physical-host changes require no TypeScript→Rust traffic.**

---

## 27. Public API direction

### 27.1 Core package surface

Possible eventual imports:

```ts
import {
    Box,
    Row,
    Column,
    Grid,
    Text,
    Content,
    Markdown,
    Diff,
    ScrollSurface,
    defineContentComponent,
    createTextStreamSource,
    createTextBlockSource,
    createIyonHost,
    useBackend,
    useHostCapability,
    useHostEnvironment,
} from "iyon-ui";
```

### 27.2 Backend choice

```ts
const host = await createIyonHost({
    backend: "terminal",
    policy: {
        pointer: "auto",
        images: "auto",
        clipboard: "best-available",
    },
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

Backend choice is an application-level host decision.

### 27.3 Simple content

```tsx
<Markdown source={assistantStream} smooth />
```

The component manages its implicit destination/binding.

### 27.4 Static React text

```tsx
<Text>Hello</Text>
```

Changing children to `Goodbye` becomes content replacement, not occurrence replacement.

### 27.5 Content component definition

```tsx
export const AssistantText = defineContentComponent({
    displayName: "AssistantText",
    defaultFunnel: markdown({
        projection: { wrap: "word" },
    }).smooth({
        targetGraphemesPerSecond: 40,
        maximumLagMs: 700,
    }),
});
```

### 27.6 Generic content

```tsx
<Content source={source} funnel={customFunnel} />
```

### 27.7 Advanced explicit binding

```tsx
const port = useContentPort(SemanticTextContent);
const connector = useContentConnector({
    port,
    source,
    funnel,
    selected: true,
});

return <Content port={port} />;
```

### 27.8 Surface composition

```tsx
<ScrollSurface followEnd>
    {items.map(renderConversationItem)}
</ScrollSurface>
```

### 27.9 Host capability observation

```ts
const imageCapability = useHostCapability(
    capabilities => capabilities.render.images,
);
```

Most components should not need this.

### 27.10 Services

```ts
const result = await host.clipboard.writeText(text);
```

### 27.11 No public layout-engine selection

There is no permanent public `terminalLayoutEngine` option after migration.

---

## 28. The `iyon` reference application

`iyon` is not merely a demo. It is the canonical reference for building a serious dual-host application with Iyon.

### 28.1 Two hosts, one application

The app ships:

```text
iyon terminal
    polished primary experience

iyon GPUI
    initially simpler native-window experience
```

Both share:

```text
conversation model
Sources
item factories
React components
content Funnels
tool-call/result presentation
state/event logic
most Flex/Grid layout semantics
HostEnvironment-aware semantic styles
```

Host entrypoints are shallow.

### 28.2 Suggested source layout

```text
plugins/app/iyon-ui/
    model/
        conversation.ts
        conversation-store.ts

    content/
        item-factory.ts
        assistant-stream.ts
        funnels.ts

    components/
        ConversationSurface.tsx
        UserMessage.tsx
        AssistantText.tsx
        ToolCall.tsx
        ToolResult.tsx
        Approval.tsx
        Composer.tsx

    hosts/
        terminal.tsx
        gpui.tsx

    reference/
        content-plane.md
        surface-and-residency.md
```

Exact repository placement is deferred.

### 28.3 Conversation model

```ts
type ConversationItem =
    | {
          kind: "user";
          id: ConversationItemId;
          text: string;
      }
    | {
          kind: "assistant-text";
          id: ConversationItemId;
          source: TextStreamSource;
      }
    | {
          kind: "tool-call";
          id: ConversationItemId;
          call: ToolCallModel;
      }
    | {
          kind: "tool-result";
          id: ConversationItemId;
          result: ToolResultModel;
      };
```

### 28.4 Shared conversation Surface

```tsx
function ConversationSurface({ items }: { items: readonly ConversationItem[] }) {
    return (
        <HistorySurface followEnd>
            {items.map(renderConversationItem)}
        </HistorySurface>
    );
}
```

### 28.5 Item renderer

```tsx
function renderConversationItem(item: ConversationItem) {
    switch (item.kind) {
        case "user":
            return (
                <UserMessage
                    key={item.id}
                    text={item.text}
                />
            );

        case "assistant-text":
            return (
                <AssistantText
                    key={item.id}
                    source={item.source}
                />
            );

        case "tool-call":
            return (
                <ToolCall
                    key={item.id}
                    call={item.call}
                />
            );

        case "tool-result":
            return (
                <ToolResult
                    key={item.id}
                    result={item.result}
                />
            );
    }
}
```

### 28.6 Assistant content component

```tsx
const AssistantText = defineContentComponent({
    displayName: "AssistantText",
    defaultFunnel: markdown().smooth({
        targetGraphemesPerSecond: 40,
        maximumLagMs: 700,
    }),
});
```

The terminal and GPUI apps use the same component definition.

### 28.7 Segment factory

```ts
function createAssistantTextSegment(id: ConversationItemId) {
    const source = createTextStreamSource();

    return {
        source,
        item: {
            kind: "assistant-text" as const,
            id,
            source,
        },
    };
}
```

### 28.8 Producer flow

```ts
const segment = createAssistantTextSegment(nextId());

// One application-model/React structural change:
conversation.append(segment.item);

for await (const chunk of agentStream) {
    // Important: do not accumulate this text in React state.
    // React owns that the AssistantText component exists.
    // Source owns the changing payload.
    // This enters Iyon's content plane directly.
    segment.source.append(chunk);
}

segment.source.seal();
```

### 28.9 Tool boundary segmentation

```text
assistant text starts
    → append AssistantText item/Source once
    → stream bytes through Source

tool call starts
    → append ToolCall component

tool result arrives
    → update/append ToolResult component

assistant text resumes
    → create a new AssistantText item/Source
    → stream bytes through the new Source
```

This produces the natural component sequence:

```text
AssistantText segment A
ToolCall
ToolResult
AssistantText segment B
```

### 28.10 Why a new Source per text segment

The segmentation gives each visual text interval stable identity around non-text components.

It also avoids forcing one Markdown parser/document to contain tool-card holes.

### 28.11 Reference “wrong way”

The reference docs SHOULD show:

```tsx
// Do not use this for token streaming.
const [text, setText] = useState("");

for await (const token of stream) {
    setText(previous => previous + token);
}

return <Markdown text={text} />;
```

Why it is wrong for this workload:

```text
every token
    → React state update
    → component rerender
    → prop comparison
    → content replacement
```

Correct:

```text
Source append
    → content data plane
    → retained semantic pipeline
```

### 28.12 User messages and dynamic styling

A user message is an ordinary resident component.

If effort/theme changes while visible:

```text
presentation dependency invalidates
    → border/style resolves again
    → no message reconstruction
```

The reference app MUST include a regression test for older visible user-message borders.

### 28.13 Tool component mutation

A ToolCall may change:

```text
running → complete
collapsed → expanded
approval pending → resolved
```

through retained state/content and ordinary React model changes. It is not frozen because it is older.

### 28.14 Terminal-specific shell

Terminal host may add:

```text
terminal keybindings
scrollback integration
cell-aware composer
terminal capability diagnostics
```

without replacing shared conversation components.

### 28.15 GPUI-specific shell

Initial GPUI host may remain simple:

```text
one native window
same conversation Surface
same components/Sources
native pointer/scrollbar
basic AccessKit mapping
simple native composer
```

It does not need visual feature parity on day one to validate the architecture.

### 28.16 Reference quality gate

When a feature requires large duplicated terminal/GPUI application logic, treat that as an architecture smell and inspect whether Host Environment, backend extension, or shared semantic component boundaries are misplaced.

---

## 29. Backend parity and specialization

### 29.1 Shared semantics

Both hosts share:

```text
component identities
three-plane ownership
Source/Funnel/Connector/Port
semantic text IR
Flex/Grid relationships
state model
event identities
focus/action concepts
Surface follow/anchor semantics
```

### 29.2 Physical differences

Terminal:

```text
integral cells
capability-varying color
optional pointer
terminal keyboard protocols
scrollback/history concerns
terminal services/protocols
```

GPUI:

```text
pixels
GPU composition
native pointer/IME
window management
native accessibility tree
platform services
```

### 29.3 Capability-first decisions

Do not infer capability solely from backend name.

A terminal may support pointer and truecolor.

A GPUI service may still fail or be denied.

### 29.4 Presentation fallback

Semantic features may be:

```text
portable with automatic realization
capability-conditioned with documented fallback/rejection
explicit backend extension
```

Do not silently approximate when approximation violates the semantic contract.

### 29.5 History/scrolling

Shared Surface logic owns ordering, anchor, follow, and logical viewport state.

Terminal and GPUI specialize physical scrolling, scrollbars, scrollback integration, and input.

---

## 30. Errors, statuses, and lifetime semantics

### 30.1 Caller validation errors

Synchronous/prepare-time typed errors:

```text
invalid property value
invalid Source mutation
incompatible Source/Funnel family
invalid Funnel composition
stale handle
ambiguous Content ownership mode
Source disposed/in use
```

### 30.2 Connector operating errors

Connector exposes status/error for:

```text
unsupported host capability
blocked geometry
retention incompatibility
parser/transform failure
projection failure
source range unavailable
```

A failed candidate Connector does not replace the committed Connector.

### 30.3 Semantic parser diagnostics

Malformed Markdown/diff/ANSI input SHOULD generally produce semantic diagnostics/fallback text rather than fail the whole frame.

Security-sensitive or impossible stage errors may mark Connector errored.

### 30.4 Automatic frame errors

Stored on Host runtime and reported at explicit barriers/error observers. Previous frame remains visible.

### 30.5 Source lifecycle

```text
created
mutable according to family
optional sealed state for append Sources
disposed
```

`seal()` finalizes ingestion semantics. It does not freeze presentation.

### 30.6 Port lifecycle

```text
created unmounted
mounted to one occurrence
unmounted/remountable if explicitly owned
disposed only when unmounted and unused
```

Implicit occurrence-owned Ports dispose on unmount.

### 30.7 Connector lifecycle

Connector endpoints/spec are immutable.

```text
created cold
selected/waiting
preparing
ready/cold by demand
error/status transitions
disposed
```

### 30.8 Surface child removal

Removing a child occurrence disposes occurrence-owned Port/Connector resources, releases event/feature-demand leases, updates anchors, and preserves externally owned Sources.

### 30.9 Editing lifetime

Editing an old item does not resurrect a frozen renderer. It mutates/replaces semantic resources and invalidates dependencies normally.

---

## 31. Relationship to PERF-13

PERF-13 remains the implementation prerequisite and reference.

### 31.1 Carried forward

```text
three-plane ownership
state/content bypass structural composition
opaque retained identities
generation validation
Rust-side consequence classification
Source/Funnel/Connector/ContentPort concepts
source sharing
cold inactive execution
prepare/commit visibility discipline
bulk content data plane
```

### 31.2 Clarified by v5

Revision 5 sharpens:

```text
Funnel specification vs Connector execution
exact Connector identity justification
semantic pipeline stage order
Markdown/diff/ANSI semantic IR
Smooth delivery semantics
React convenience vs advanced resource ownership
Surface-only component history model
cache validity/residency instead of freeze
reference app architecture
```

### 31.3 Superseded future assumptions

Post-PERF-13 React architecture does not preserve:

```text
special History that mixes text and components
irreversible frozen View snapshots while resident
custom terminal general layout
current public View composition as canonical API
```

### 31.4 PERF-13 layout machinery

Implement correctly for current engine, then delete redundant general-layout machinery after Taffy migration.

### 31.5 PERF-13 content compatibility

If current PERF-13 API names differ, migrate them toward v5 ownership without creating a parallel content path.

### 31.6 No premature rewrite

Do not destabilize PERF-13 implementation by attempting all v5 work simultaneously. Use the migration tranches below.

---

## 32. Migration and implementation tranches

The final destination is settled, but migration must isolate risk.

### V5-A — Finish PERF-13

Deliver:

```text
stable structural/state/content planes
resolved frame transaction semantics
Source/Port/Connector foundations
bulk content data path
current terminal performance gates
```

No v5 shortcut may weaken this tranche.

### V5-B — React frontend seam

Implement a minimal React renderer over the current terminal backend.

Scope:

```text
HostInstance identity
render-phase purity
plane-aware normalization
frontend transaction envelope
Box/Row/Column/Text/Content
JS callback registry
transport counters
```

Gates:

```text
no-op rerender → zero native semantic ops
presentation update → one state delta
text replacement → content only
Source append → zero React
```

### V5-C — Content ownership clarification

Refactor/solidify:

```text
FunnelSpec immutable values
Connector exact binding/execution
Port destination ownership
Source typed families
Connector status/lifecycle
transactional connector switching
```

Delete any duplicate interpretation where Funnel owns live state or Port owns parser execution.

### V5-D — Semantic text IR

Implement:

```text
plain semantic text
source ranges
semantic roles
dynamic style resolution
selection/copy basics
compact content ABI
```

Then add Markdown, diff, and safe ANSI transforms.

### V5-E — Incremental Markdown and streaming tail

Implement:

```text
stable prefix
replaceable unstable tail
parser diagnostics
Source seal finalization
incremental counters
```

Differential-test against full reparse.

### V5-F — Smooth delivery

Implement Connector-local native smoothing:

```text
accepted/semantic/visible frontiers
Rust clock
bounded lag
catch-up policy
seal drain/flush
cold suspension/reactivation
```

Prove zero React and zero TS→Rust traffic on smooth ticks.

### V5-G — Component-local content API

Implement:

```text
<Markdown source smooth>
<Diff source>
<Content source funnel>
defineContentComponent
scoped typed defaults
advanced Port/Connector hooks
implicit ownership cleanup
```

### V5-H — Component-only ScrollSurface

Replace special mixed History semantics with:

```text
ordered component children
follow-end state machine
scroll anchoring
paint culling
resident cache invalidation
regression fix for old visible styling
```

Keep current History compatibility adapter temporarily if consumer migration requires it.

### V5-I — Derived residency

Add:

```text
visible/near/cold-derived demand
Connector cold suspension
extent caching
interaction promotion
overscan instrumentation
```

Do not introduce React unmount virtualization in the first version.

### V5-J — Host Environment skeleton

Implement typed profile/capability/environment/policy/service ownership and Rust→JS external-store observation.

### V5-K — Terminal Taffy migration

Run legacy and Taffy side-by-side in development/benchmark builds, dogfood real Iyon, optimize, then delete legacy general layout.

### V5-L — Reference Iyon terminal migration

Convert Iyon conversation history to component items/Sources/factories.

Remove:

```text
special append-text-to-history paths
freeze-as-presentation-lifetime semantics
mixed text/component history storage
```

### V5-M — GPUI host

Implement the simple reference GUI:

```text
native window
shared React application tree
shared Sources/Funnels/components
GPUI/Taffy
native pointer/scroll
basic accessibility/services
```

### V5-N — Cleanup

Delete:

```text
legacy TS View composition as canonical public path
legacy terminal layout
legacy History renderer
second text-stream mutation architecture
migration selectors/adapters
obsolete dirty/cache mechanisms
```

Each tranche must have explicit counters and stop gates before later tranches depend on it.

---

## 33. Benchmark and observability requirements

### 33.1 Always-on plane counters

```text
React render/commit count/time
structural ops/bytes
state ops/bytes/no-ops
content control ops
content payload bytes/appends/replaces/patches
frontend transaction prepare/commit
frames with/without JS activity
```

### 33.2 Source counters

```text
accepted revisions
accepted bytes/records
retained bytes/range
retention drops
seal operations
active Connector subscription count
```

### 33.3 Funnel/Connector counters

```text
Connector creates/disposes
cold/active transitions
candidate switch prepare/commit/failure
Source revisions consumed
semantic parse bytes/tokens/blocks
stable prefix reused
unstable tail reparsed
semantic cache hits/misses
Smooth ticks/batches/graphemes
visible lag/max lag/catch-up events
projection cache hits/misses
blocked/unsupported/error statuses
```

### 33.4 Semantic text benchmark scenarios

```text
plain append stream
Markdown paragraph stream
headings/emphasis delimiters arriving late
fenced code block stream
Markdown table stream
diff stream
ANSI style stream
full reparse vs incremental parse comparison
post-hoc theme/effort recolor
```

### 33.5 Smooth benchmark scenarios

```text
steady token rate
large burst
producer faster than target
producer slower than target
newline-heavy output
long code block
seal with backlog
cold reactivation with backlog
two Connectors: immediate + smooth
```

Expected:

```text
React commits during ticks = 0
TS→Rust bytes during ticks = 0
Source bytes stored once
```

### 33.6 Surface counters

```text
child count
visible/near/cold-derived counts
paint-cull count
projection eviction/materialization
extent cache hit/miss/stale correction
anchor selection/change/adjustment
follow state transitions
scroll corrections
interaction residency promotions
```

### 33.7 Surface benchmark scenarios

```text
10k stable components
streaming tail while following end
streaming tail while user scrolled away
expanding tool card above viewport
old visible theme/effort change
selection across several text components
editing old message
terminal resize/mass rewrap
rapid scroll through cold items
```

### 33.8 User-message-border regression counter/test

A presentation revision change must mark every resident matching occurrence, regardless of age/order.

No special live-batch check may be required.

### 33.9 Taffy counters

```text
layout calls/time p50/p95/p99/max
cache hits/misses/clears
leaf measurement calls
text wrap calls
nodes recomputed where observable
resolved rect changes
rounding corrections
```

### 33.10 Host Environment counters

```text
capability/environment revisions
probe lifecycle
Unknown→Supported/Unsupported transitions
feature demand leases
active protocol transitions
capability-triggered invalidations
Rust→JS notifications
selector rerenders
service attempts/results
```

### 33.11 Real Iyon trace

Record/replay:

```text
startup with realistic conversation
user messages
assistant Markdown streaming
multiple tool calls/results
assistant text resumption segments
expand/collapse
selection/edit entry
follow-end/manual scroll
terminal resize
Host Environment theme/color refinement
idle/smooth/native frames
```

Run terminal and GPUI where applicable.

### 33.12 Absolute-cost decision policy

Do not reject Taffy/semantic content architecture solely because an isolated operation is a larger relative multiple when absolute cost remains negligible.

Do not accept repeated multi-millisecond hot-path regressions merely for conceptual purity.

---

## 34. Correctness and testing matrix

### 34.1 Plane isolation tests

| Mutation | Structural ops | State ops | Content ops | React required |
|---|---:|---:|---:|---:|
| Background change | 0 | 1 | 0 | ordinary prop path may rerender |
| Gap change | 0 | 1 | 0 | ordinary prop path may rerender |
| Child insertion | changed frontier | initial only | initial only | yes |
| React text replacement | 0 | 0 | replace | yes |
| Source append | 0 | 0 | append bytes | no |
| Smooth tick | 0 | 0 | native frontier only | no |
| Host color refinement | 0 | 0 | 0 | no by default |

### 34.2 Funnel/Connector ownership tests

- Reusing one Funnel creates independent Connector execution state.
- Same Source + immediate/smooth Connectors reveal independently.
- Connector source/funnel/port endpoints cannot mutate in place.
- Transactional switch preserves old visible projection on failure.
- Port contains no parser/smoothing execution state.
- Funnel contains no mutable progress.

### 34.3 Markdown incremental tests

- Incremental result equals full parse for every prefix in corpus.
- Stable prefix identity is retained where valid.
- Trailing delimiters revise only unstable region.
- Seal finalization equals full final parse.
- Theme/effort recolor does not reparse.

### 34.4 Diff/ANSI tests

- Unified diff roles remain semantic and restylable.
- ANSI SGR converts to semantic style intent.
- Unsafe cursor/window controls never reach backend output.
- OSC links obey policy/capability.

### 34.5 Smoothing tests

- Grapheme-safe frontier.
- Monotonic visible frontier for append Sources.
- Bounded lag behavior.
- Cold connector has no active timer/work.
- React and TS transport counters remain zero during ticks.
- Backlog default is immediate; explicit replay is deterministic.

### 34.6 Source retention tests

- Retention-compatible Funnel activates correctly.
- Markdown rejects incompatible arbitrary-head truncation in v1.
- Rolling plain stream restarts from retained head.
- Annotations clip/disappear according to kind.

### 34.7 React ownership tests

- Implicit Port/Connector creation/disposal follows occurrence lifetime.
- External Source is not disposed on unmount.
- Explicit mode cannot ambiguously combine implicit props.
- Scoped defaults affect only matching subtree/components.
- Explicit instance props override component defaults.

### 34.8 Surface tests

- Surface children may be heterogeneous components.
- Content policy remains child-local.
- Theme/effort updates old resident children.
- Cold re-entry uses current theme/HostEnvironment.
- Selection does not force semantic reparse/layout when unnecessary.
- Editing invalidates correct dependency frontiers.

### 34.9 Follow/anchor tests

- Following pins end during streamed growth.
- Manual scroll suspends follow.
- Returning to end resumes follow.
- Insertion/resize above viewport preserves anchor.
- Removing anchor chooses deterministic replacement.
- Focus/selection receives anchor priority.

### 34.10 Residency tests

- Paint culling does not alter semantics.
- Cold projection eviction retains compact binding identity.
- Re-entry catches Connector up according to policy.
- Interaction promotes residency.
- Extent corrections preserve anchor.

### 34.11 React commit tests

- Abandoned speculative render creates no native resource.
- Desired native revision accepted before successful commit returns.
- Previous visible frame remains on transient failure.
- Hard-invalid props fail before authoritative desired state.
- Visibility barrier reports pending/error state.

### 34.12 Host Environment tests

Use synthetic fixtures for:

```text
Mono/ANSI16/ANSI256/TrueColor
pointer absent/cell/pixel
focus reports
bracketed paste
remote multiplexer
light/dark palette changes
GPUI desktop services
```

### 34.13 Dual-host reference tests

The same conversation model/Source trace should produce semantically equivalent component order, content, and interaction events on terminal and GPUI.

---

## 35. Success criteria

The architecture is validated when all of the following are true.

### 35.1 Frontend and transport

1. React is the canonical public composition model.
2. No public Rust UI authoring API is required.
3. No-op rerenders cross no semantic mutations.
4. State changes cross only typed state deltas.
5. Structural transport is proportional to the changed frontier.
6. Source appends require no React work.
7. Native Smooth ticks require no TypeScript traffic.

### 35.2 Content ownership

8. Source, Funnel, Connector, and ContentPort have non-overlapping normative ownership.
9. Funnel is immutable specification.
10. Connector is the retained execution identity of one exact binding.
11. Port is a destination and owns no parser/smoothing progress.
12. Connector switching is transactional.
13. One Source supports independent immediate/smoothed/multi-width Connectors.
14. Cold Connectors perform no semantic/projection/delivery work.

### 35.3 Semantic content

15. Markdown, diff, and ANSI produce backend-neutral semantic IR.
16. Semantic text can restyle after theme/effort/Host Environment changes without reparsing.
17. Incremental Markdown uses stable prefix/unstable tail semantics.
18. ANSI control input cannot escape the safe renderer.
19. Smooth delivery operates after semantic parsing.
20. Smoothing is bounded, grapheme-safe, native, and eventually convergent.

### 35.4 React ergonomics

21. `<Markdown source={stream} smooth />` is the canonical simple path.
22. Generic `<Content source funnel>` uses the same machinery.
23. Advanced users can explicitly own Port/Connector lifecycle.
24. Defaults are component-local or typed subtree-local, not assumed app-global.
25. Component factories and application item factories have distinct documented roles.

### 35.5 Surface model

26. Framework Surface stores/orders component occurrences only.
27. Chat History is an application component over ScrollSurface.
28. Surface knows no Markdown/smoothing policy.
29. Semantic completion does not freeze presentation.
30. Old resident components respond to current state/theme/Host Environment.
31. Cache validity and residency are reversible.
32. Follow-end and scroll anchoring are deterministic.
33. Tool/text segment growth requires no History-type-specific layout code.

### 35.6 Layout and backends

34. Terminal general layout uses Taffy.
35. GPUI general layout uses GPUI/Taffy.
36. Legacy terminal general layout is deleted.
37. Terminal-specific controls coexist with Taffy without a second general layout language.
38. Shared application components/Sources run on terminal and GPUI.

### 35.7 Host Environment and events

39. Host Environment remains orthogonal to the three planes.
40. Unknown capability state is explicit.
41. Capability changes are native-first.
42. JS owns callbacks; Rust owns subscriptions/input/event emission.
43. Feature Broker controls modal terminal protocols from capability + policy + demand.
44. Host service results are authoritative.

### 35.8 Reference application

45. Iyon ships a polished terminal host and a functional simple GPUI host.
46. The shared reference code visibly teaches structural React updates vs Source content updates.
47. Tool boundaries create components; token boundaries do not create React updates.
48. The historical user-border regression is impossible under the resident Surface model.

---

## 36. Explicitly deferred questions

The following remain implementation-specific or require prototype evidence.

### 36.1 React renderer

- exact `react-reconciler` version/HostConfig compatibility strategy;
- mutation vs persistence mode details;
- Suspense/Offscreen semantics;
- refs/public instances;
- priority mapping;
- exact effect/visibility-barrier API.

### 36.2 Binary transport

- exact structural/state batch layout;
- exact semantic IR sidecar encoding;
- N-API TypedArray vs direct control ABI split;
- value/string interning;
- ABI versioning/symbol spelling.

### 36.3 Semantic IR representation

- arena/tree/rope representation;
- exact node IDs/source-range encoding;
- table and code-block structures;
- syntax-highlight token integration;
- annotation ABI details;
- semantic cache sharing granularity.

### 36.4 Markdown parser

- parser library/implementation;
- precise stable-tail algorithm;
- table/fence streaming behavior;
- CommonMark/GFM feature boundary;
- custom extension registration.

### 36.5 Smoothing algorithm

- exact adaptive rate function;
- default rate/lag values;
- block/newline scheduling heuristics;
- seal drain vs flush default tuning;
- accessibility/reduced-motion integration;
- replay API naming.

### 36.6 Source families and retention

- exact patch protocol for TextDocumentSource;
- semantic checkpoint encoding;
- RollingSource coordinate API;
- archive/persistence integration;
- Source cloning/snapshot semantics.

### 36.7 React content defaults

- exact `defineContentComponent` API;
- exact scoped-provider names;
- whether defaults merge or replace by policy field;
- connector diagnostic ref shape;
- explicit hook ownership escape hatch.

### 36.8 Surface implementation

- exact overscan/residency thresholds;
- extent-estimation model;
- whether first release includes cold-derived eviction;
- future React-level VirtualSurface API;
- selection spanning cold/unmounted items;
- native scrollbar abstraction;
- terminal scrollback export/integration details.

### 36.9 Taffy

- high-level tree vs low-level trait integration;
- terminal text-measurement cache ownership;
- containment/isolation optimization;
- damage extraction after layout;
- final legacy deletion milestone.

### 36.10 Host Environment

- complete capability inventory;
- source/confidence metadata;
- exact terminal probe lifecycle;
- multiplexer passthrough;
- palette/theme query support;
- trust policy for remote/custom streams;
- service policy/cancellation/timeouts;
- accessibility schema details.

### 36.11 GPUI

- exact Element lowering;
- stable `ElementId` mapping;
- native text editor strategy;
- window lifecycle;
- AccessKit update schedule;
- graphical content/source families.

### 36.12 Packaging

- final package/crate names;
- native artifact distribution;
- `iyon-tui` compatibility duration;
- separate terminal/GPUI entrypoints vs one package export.

Deferred questions MUST be resolved without violating settled ownership/invariants.

---

## 37. Architectural anti-goals

The implementation must resist:

```text
generic setProps(id, object)
generic setStyle(id, completeStyle)
generic setText as structural mutation
full VDOM serialization per commit

per-token React state updates for streaming
React participation in Smooth ticks
JS callbacks inside the native Funnel hot path

Funnel containing mutable parser/progress state
Connector existing only as a ceremonial tuple
Port owning Source/parser/smoothing state
Source owning width/viewport-specific projection

arbitrary untyped Funnel stage arrays
one universal Source with every mutation method
raw ANSI passthrough to the terminal
semantic IR containing native host Style IDs

<History> mixing raw text buffers and components
Surface appendMarkdownChunk APIs
Surface-wide smoothing policy
irreversible freeze/unfreeze rendering lifecycle
using semantic completion as cache lifetime

old resident UI ignoring theme/effort/HostEnvironment changes
reconstructing historical components merely to restyle them

app-global content policy as the only defaults mechanism
unrestricted global Connector factory injection
separate implementation for convenience and advanced Content APIs

permanent custom terminal general-layout engine
making taffy::Style the wire format
forcing terminal and GPUI into one physical geometry IR

HostCapabilities as HashMap<String, any>
backend-name checks scattered through components
boolean capabilities where Unknown differs from Unsupported
React manually enabling terminal protocols

JS closures stored as native event-handler ownership
assuming terminal means no mouse/truecolor
assuming GPUI service support means operation success

silent semantic approximation without a documented fallback
forking the entire React application for terminal vs GPUI
```

---

## 38. Research basis and transferable lessons

Revision 5 is grounded in repository/source research, not only architectural inference.

### 38.1 OpenTUI Markdown

OpenTUI's current Markdown implementation provides useful evidence for incremental streaming semantics:

- streaming mode keeps trailing Markdown unstable;
- incremental parsing reuses unchanged leading tokens;
- a trailing token region is deliberately reparsed;
- Markdown rendering, styled text, tables, code, and links remain structured rather than flattening immediately to terminal bytes.

Transferable lesson:

> **Incremental semantic parsing should expose reusable stable prefix plus replaceable unstable tail.**

Iyon does not copy OpenTUI's Renderable object model; it moves the semantic result into the retained content plane.

### 38.2 OpenTUI ScrollBox

OpenTUI's ScrollBox keeps scrolling/sticky behavior and viewport culling separate from Markdown/renderable content.

Transferable lesson:

> **Scrolling/culling belongs to a spatial container; content parsing belongs to children.**

### 38.3 OpenTUI NativeSpanFeed

OpenTUI's native span-feed path demonstrates the value of feeding structured styled text separately from ordinary React tree mutations.

Transferable lesson:

> **A compact retained semantic text lane is a legitimate native primitive, not merely an optimization hack.**

### 38.4 GPUiX

GPUiX demonstrates a small coherent React→GPUI architecture and a useful event ownership pattern: JS retains callback closures while native retains listener presence and emits event payloads.

Transferable lesson:

> **Keep application closures in JS; keep physical subscriptions and event normalization in native runtime.**

Iyon deliberately diverges by using plane-specific state/content protocols rather than one generic host mutation vocabulary.

### 38.5 React Context

React Context resolves the nearest provider above a component and is suitable for scoped, relatively stable component defaults.

Transferable lesson:

> **Subtree defaults can be local and compositional without an application-global DI container.**

Dynamic hot content does not belong in Context.

### 38.6 React external stores

React's `useSyncExternalStore` is the supported integration point for state that changes outside React.

Transferable lesson:

> **HostEnvironment observation belongs in a revisioned external store, with a selector wrapper to avoid unrelated rerenders.**

### 38.7 React Native virtualization

React Native's VirtualizedList maintains a finite active render window and warns that unmounted item-local state must be externalized.

Transferable lesson:

> **Windowing can reduce memory, but resource/application state must survive outside evicted React instances.**

Iyon initially prefers native derived-residency while retaining React identity, then may add a data-driven VirtualSurface later.

### 38.8 CSS Scroll Anchoring

The CSS Scroll Anchoring specification preserves a visible anchor node when content outside the viewport changes.

Transferable lesson:

> **Surface anchoring should use stable child identity plus local offset rather than preserving an absolute row number.**

### 38.9 Taffy

Taffy provides Flexbox/Grid, cached computation, custom leaf measurement, low-level embedding, and rounding.

Transferable lesson:

> **Use one general-layout engine while keeping terminal/GPUI measurement and physical realization specialized.**

### 38.10 Termwiz

Termwiz provides terminal abstraction, Surface rendering, input decoding, and capability support.

Transferable lesson:

> **Reuse low-level terminal portability rather than rebuilding all tty/platform behavior.**

### 38.11 GPUI

GPUI supplies frame-level Elements, Taffy-backed layout, platform/window services, native input, and AccessKit integration.

Transferable lesson:

> **The GPUI backend should lower retained Iyon semantics into ephemeral native frame objects rather than making GPUI Elements the semantic graph.**

### 38.12 Reference URLs

```text
OpenTUI:
https://github.com/anomalyco/opentui
https://github.com/anomalyco/opentui/blob/main/packages/core/src/renderables/Markdown.ts
https://github.com/anomalyco/opentui/blob/main/packages/core/src/renderables/markdown-parser.ts
https://github.com/anomalyco/opentui/blob/main/packages/core/src/renderables/ScrollBox.ts
https://github.com/anomalyco/opentui/blob/main/packages/core/src/NativeSpanFeed.ts
https://opentui.com/docs/reference/terminal-capabilities/

GPUiX:
https://github.com/remorses/gpuix

React:
https://react.dev/reference/react/createContext
https://react.dev/reference/react/useContext
https://react.dev/reference/react/useSyncExternalStore

Virtualization:
https://reactnative.dev/docs/next/virtualizedlist
https://reactnative.dev/docs/virtualview

Scroll anchoring:
https://www.w3.org/TR/css-scroll-anchoring-1/

Taffy:
https://docs.rs/taffy/latest/taffy/
https://docs.rs/taffy/latest/taffy/compute/fn.compute_cached_layout.html

Termwiz:
https://docs.rs/termwiz/latest/termwiz/

GPUI:
https://github.com/zed-industries/zed/blob/main/crates/gpui/src/element.rs
https://github.com/zed-industries/zed/blob/main/crates/gpui/src/platform.rs
```

These systems support particular separation/implementation lessons. They do not override Iyon's repository-specific invariants.

---

## 39. Final north star

```text
                                  REACT
                                    │
                                    ▼
                         plane-aware TS runtime

                    structure      state      content
                        │             │           │
                        └─────────────┼───────────┘
                                      ▼
                           retained Rust runtime
                                      │
           ┌──────────────────────────┼──────────────────────────┐
           │                          │                          │
           ▼                          ▼                          ▼

   CONTENT RUNTIME             HOST ENVIRONMENT             SURFACE RUNTIME
   Sources                     capabilities                 components only
   Funnel specs                environment                  scroll/anchor
   Connectors                  policy/services              residency/cache
   Ports                       feature broker
   semantic IR
   Smooth delivery
           │                          │                          │
           └──────────────────────────┼──────────────────────────┘
                                      ▼
                                  TAFFY
                              shared Flex/Grid
                                  /       \
                                 /         \
                         terminal cells   GPUI pixels
                               │               │
                               ▼               ▼
                         TERMINAL HOST      GPUI HOST
```

The architecture can be summarized by ten rules:

1. **React says which UI things exist and how they compose.**
2. **The wire format is three semantic protocols, not React props.**
3. **Sources advance the content of existing things without React.**
4. **Funnels describe immutable content policy.**
5. **Connectors own one binding's live execution state.**
6. **Ports identify where projected content appears.**
7. **Semantic text is retained before width, terminal cells, or GPUI pixels.**
8. **Smoothing reveals semantic content on a native clock.**
9. **Surfaces contain components and manage space, scrolling, anchoring, and residency—not content parsing.**
10. **Stable inputs create cache hits; nothing becomes semantically frozen merely because it is old or complete.**

The canonical application pattern is:

```text
new semantic item appears
    → update application model once
    → React inserts named component once

existing content advances
    → Source.append()
    → content plane
    → semantic Funnel execution
    → Smooth/projection/layout/paint
    → React does nothing
```

The canonical conversation structure is:

```text
UserMessage
AssistantText(Source A)
ToolCall
ToolResult
AssistantText(Source B)
```

The terminal and GPUI versions of `iyon` share this model, these Sources, these component factories, and these React components.

Host Environment realizes the same semantics according to the actual physical host.

Taffy supplies one general Flexbox/Grid implementation.

PERF-13 supplies the retained-plane foundation.

Revision 5 completes the missing architectural story: the content plane is no longer merely a list of nouns, and history is no longer a privileged frozen renderer. The system has one explicit content execution model and one ordinary component Surface model.

That is the post-PERF-13 direction for `iyon-ui`.
