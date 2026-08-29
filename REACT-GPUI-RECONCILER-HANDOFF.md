# React-GPUI Reconciler — Scratchpad

**Context:** derived from `PERF-13-THREE-PLANE-RUNTIME-HANDOFF.md` and conversation  
**Status:** pre-research notes, awaiting PERF-13 stable template  
**Intended velocity:** 1 focused day after PERF-13 template is proven (evidence: ~131–146 commits/day peak during iyon-tui API-H and PERF-12 tranches)

---

## 0. The Thesis

React in TypeScript for developer ergonomics. GPUI in Rust for GPU performance.
The bridge is the only critical path — and the three-plane architecture from
PERF-13 gives us the blueprint for making that bridge efficient.

The industry false dichotomy:

| Camp | Developer experience | Performance |
|---|---|---|
| DOM / Electron | React / TypeScript | CPU, memory-heavy |
| Native (GPUI/SwiftUI) | Rust / Swift / Kotlin | GPU, efficient |

The insight being explored: **does the PERF-13 plane separation make the
TS→Rust bridge cheap enough that you don't have to choose?**

Conventional wisdom: TS→Rust FFI is too expensive per frame. This is true if
serializing VDOM across the boundary every 16ms. PERF-13 splits UI work into
planes with different transport characteristics, so different kinds of work pay
different bridge costs.

---

## 1. Existing GPUI React Approaches

There are existing React→GPUI projects (gpui-shell, gpui-x). Their current
architecture (subject to verification) appears to conflate structure, state,
and content through a single bridge path:

```
React VDOM
    ↓ (every render, every setState)
Bridge (serialize tree, N-API or similar)
    ↓
GPUI element tree
    ↓
Paint
```

If this characterization is accurate, every `setState` serializes the full view
tree. A color change pays the same bridge cost as a structural reconfiguration.
This would explain bridge performance issues.

PERF-13's plane separation is a candidate solution to this problem. The
iyon-tui implementation already proves the individual mechanisms work under
terminal constraints (serial protocol, dual-language, streaming text). The
question is whether they compose to solve the GPUI bridge problem.

---

## 2. The Three-Plane Architecture (from PERF-13)

Structural, state, and content are orthogonal planes with different bridge
characteristics:

```
                         TYPESCRIPT (React)

                 ┌──────────────────────────────┐
                 │       STRUCTURAL PLANE       │
                 │                              │
                 │  React Fiber tree            │
                 │    ↓                         │
                 │  Retained element DAG        │
                 │                              │
                 │  Bridge: TBD transport        │
                 │  (N-API or generated ABI)    │
                 │  Frequency: rare             │
                 └──────────┬───────────────────┘
                            │ stable identity
                 ┌──────────┴───────────┐
                 │                      │
          STATE PLANE            CONTENT PLANE
          ────────────            ─────────────

          geometry                ContentPort
          presentation               │
          style-state              FFI byte lane? (TBD)
          animation                TextStream
                                   Markdown projection
          │                      │
          Bridge: TBD transport  Bridge: TBD transport
          Frequency: per-interaction  Frequency: per-frame
          └──────────┬───────────┘
                     ▼

              ┌──────────────────────┐
              │     RUST (GPUI)      │
              │                      │
              │  render cache        │
              │  measurement         │
              │  layout              │
              │  paint (GPU glyphs)  │
              │  damage              │
              │  actual pixel output │
              └──────────────────────┘
```

### 2.1 Plane ownership (from PERF-13)

| Plane | Owns | Changes when |
|---|---|---|
| **Structural** | element kind, parent-child topology, child order, key, component identity, ContentPort attachment | navigation, add/remove/reorder items, view type change |
| **State** | geometry (width/height rule, padding, bounds, gap), presentation (background, border color, text style), style-state overrides | scroll, hover, focus, animation, interaction |
| **Content** | UTF-8 bytes, source revisions, annotations, projection, wrapping, streaming | text input, streaming, file load |

### 2.2 The cardinal rule (from PERF-13)

> A change in one plane must not cause work in another plane.

- Changing color → state plane → paint-only → no structural bridge traffic
- Streaming text → content plane → bytes → no React re-render
- Moving a list item → structural plane → reconciliation → structural bridge traffic

This is the hypothesis to validate.

---

## 3. Bridge Transport — Open Questions

Each plane may use a different transport. The PERF-13 document specifies a
specific approach (N-API for structural, typed patches for state, raw FFI for
content). The appropriate transport for the GPUI target is a research question.

### 3.1 Structural plane

iyon-tui's approach: generated N-API call-per-node, generation-scoped handle
validation, NativeRef leases, three-tier materialization (cache hint → fast
path → cold fallback).

The React reconciler changes the structural pattern: instead of building an
immutable View DAG and publishing it through a seam, React calls host config
methods (createInstance, appendChild, commitUpdate, removeChild). These could
map to N-API calls that create/mutate GPUI elements directly.

Research questions:
- Does the fiber tree's identity tracking replace or complement NativeRef caching?
  (In iyon-tui, PersistentSeq Arc identity proved unchanged subtrees; in React,
  fiber reconciliation tells us directly which instances are new vs reused.)
- Does the three-tier materialization model apply? (hint: known NativeRef still
  valid → skip; small fast-path: create simple GPUI element directly; cold:
  full bridge serialization for complex cases)
- What is the correct granularity for handle lifetimes?

### 3.2 State plane

iyon-tui's approach: `ViewState` handle with `setGeometry(patch)` /
`setPresentation(patch)` → typed N-API call → Rust property descriptor →
effect classification → dirty propagation → localized repaint.

This is the key architectural claim: **state patches bypass React's VDOM
entirely.** They go from the API surface to native state arena. No fiber
re-render, no structural bridge traffic.

Research questions:
- Can GPUI's existing element property system absorb typed patches, or does it
  need a separate state arena (iyon-tui's `ViewStateRecord`)?
- How do patches interact with GPUI's style resolution?
- What is the effect classification model? (iyon-tui uses `EffectMask` bitflags:
  RESOLVE_STYLE, PROJECT_CONTENT, MEASURE_SELF, MEASURE_ANCESTORS, PLACE_SELF,
  PAINT_SELF, PAINT_SUBTREE, DAMAGE_OLD_RECT, DAMAGE_NEW_RECT...)
- How does dirty propagation interact with GPUI's layout tree?
  (iyon-tui uses per-dimension `ChildDependency` bits: PARENT_USES_CHILD_WIDTH,
  PARENT_USES_CHILD_HEIGHT, etc. — propagate only dimensions the parent depends on.)

### 3.3 Content plane

iyon-tui's approach: Source/Funnel/Connector/Port model with raw FFI byte
transport for UTF-8 payloads. Startup ABI probe verifies compatibility.
Synchronous copy into Rust-owned memory. Generation-scoped handle validation.

Research questions:
- Can GPUI's existing `TextBuffer` absorb the content plane directly, or does
  it need a separate TextStore with chunked storage + line index (iyon-tui
  approach)?
- Does the Funnel/Connector/Port model (Source is authoritative data, Funnel is
  transform policy, Connector is width-specific projection instance, Port is
  structural receiving region) apply to GPUI, or is there a simpler mapping?
- Cold connector semantics (PERF-13 §14.2): inactive connector performs no
  projection, wrapping, measurement, layout, or paint. This matters for many
  invisible data sources (e.g., 100 chat channels, 1 visible). Transfer 1:1 or
  adapt?
- Is raw FFI the right transport, or would a shared memory region + atomic
  revision counter be better for high-frequency streaming?

---

## 4. The React Reconciler Host Config

The reconciler itself is a well-documented pattern using `react-reconciler`.
Dozens of prior implementations exist (react-dom, react-native,
react-three-fiber, react-pdf, react-ink). The host config interface has ~15-20
methods.

### 4.1 Host config sketch

```typescript
// Structural plane entry points
createInstance(type, props, rootContainer)    → StructuralHandle
appendInitialChild(parent, child)              → void
appendChild(parent, child)                     → void
removeChild(parent, child)                     → void
insertBefore(parent, child, before)            → void

commitUpdate(instance, updatePayload, ...)     → void
  // Classification: structural prop change vs state patch vs content change
  // Dispatch accordingly to respective plane
```

### 4.2 Candidate hook API (for discussion)

```typescript
// State plane — direct mutation, no React re-render
function useStyle(): ViewStateHandle
  // Returns handle to native state arena record
  // setPresentation/setGeometry mutate through typed bridge call
  // Component does NOT re-render when called

function useContentPort<T>(family: ContentFamily): ContentPortHandle
  // Returns handle to a content port
  // Connected to a native Source
  // Component re-renders only if port lifecycle changes (mount/unmount)
```

---

## 5. The Render Cache

The state plane only works if `setGeometry({ padding: 2 })` actually results in
only the affected nodes being measured and repainted. Without a render cache,
every state patch degrades to full layout → full paint.

iyon-tui's cache architecture is the proven template:

| iyon-tui cache | Purpose |
|---|---|
| `LayoutCache` keyed on `(ViewId, geometry_fingerprint, constraints)` | Avoid re-measuring unchanged subtrees |
| `PaintCache` keyed on `(ViewId, presentation_fingerprint, style_context, theme_revision)` | Avoid re-resolving styles for unchanged subtrees |
| `ChildDependency` bits per parent-child edge | Propagate only dependent dimensions when a child changes |
| `NodeDirty { effects, width_dirty, height_dirty, placement_dirty, paint_dirty }` | Localized invalidation |
| `EffectMask` bitflags for property mutations | Classify consequences of each state patch |
| `DamageRegion { rects, full }` with coalescing | Don't repaint what didn't change |

Without this, the state plane cannot deliver on its promise. Integration into
GPUI's frame loop is a research question.

---

## 6. Input Handling

Keyboard and mouse events are small (bytes at human timescales). Bridge overhead
is negligible. The risk is not input delivery cost but **what happens after
dispatch**.

Expensive pattern to avoid:
```
GPUI key event → bridge to TS → React dispatches → setState
    → React re-renders entire subtree → VDOM diff
    → bridge structural changes back → GPUI rebuilds elements → paint
```

Candidate pattern:
```
GPUI key event → native handler (TextInput buffer, cursor, composition)
    → source.append(char) → content plane → GPUI repaints content rect
    → optional lightweight bridge callback to TS if app needs it
```

The critical path is native. The React path is a side channel.

Research questions:
- Does this require GPUI's key dispatch to be interceptable before React sees
  the event?
- Can text input state (cursor, composition, selection) live in the state plane
  while content (bytes) lives in the content plane?

---

## 7. Architecture Elements to Carry Forward (Not Prematurely Dismissed)

The following are candidates for transfer based on the iyon-tui implementation
experience. Each must be evaluated during a dedicated research session.

### Identity model (PERF-13 §5)
- `StructuralHandle` — opaque retained mutation attachment (maps to ViewStateId)
- `ContentPortId` — opaque structural content-host attachment
- `SourceId` — authoritative content producer record
- `ConnectorId` — one Source/Funnel/Port link
- Runtime generation — identifies one live native environment
- Generation-scoped caches and handle validation

### Structural plane (PERF-13 §6)
- Node kind selects layout algorithm family
- Parent/child topology, child order, boundary existence
- TrackSize (content, fixed, flex, content-max) for axis children
- Grid track count/order/definitions
- ContentPort attachment as a structural kind
- Container/viewport/clamp/scroll boundary existence
- The migration rule: do not delete Decorated until proof that semantics are preserved

### State plane (PERF-13 §7–8)
- Mutable geometry state: width/height rule, padding, bounds, gap, alignment, border edge presence
- Mutable presentation state: surface background, border color/glyph/style, text style, StyleRef, style-state keys
- Effect classification bitflags per property mutation
- Base + effective state distinction (immutable View base + mutable overrides + host facts)
- Equality check before revision advance (avoid false-positive dirty)
- Dependency metadata on parent-child layout edges

### Content plane (PERF-13 §12–18)
- Source: authoritative semantic content, does not know View topology
- Funnel: immutable typed transformation/delivery contract
- Connector: one retained link with attachment-local projection state
- ContentPort: one structurally mounted receiving region
- Cold inactive semantics (zero projection/layout/paint when inactive)
- Source sharing (same Source at multiple widths, independent Connector caches)
- Transactional activation (prepare → validate → commit; old active remains until new commits)
- FFI byte transport for text payloads
- TextStore: chunked storage, line index, annotation store, retention policy
- Absolute UTF-8 byte coordinates
- Source revisions and epoch-based scheduling

### Caches (PERF-13 §5, §9, §10)
- LayoutCache with multi-key validation
- PaintCache with style context + presentation revision
- Per-dimension child dependency metadata
- EffectMask + NodeDirty + generation-based dirty flags
- Damage region coalescing (rectangle-level with cap/ratio fallback)
- Cache invalidation: never use a universal host revision, always per-component keys
- Old rect preservation for damage computation

### Scheduling / transactions (PERF-13 §20)
- pending_epoch / committed_epoch model
- Last-write-wins per (attachment, property) within an epoch
- Prepare all, commit once, abort without visible change
- Source mutations mark pending epoch, don't synchronously render
- Microtask coalesced flush on TS side
- Read-your-writes barriers (tui.flush() etc.)
- Structural → state → content ordering within a flush

### Text IR (iyon-tui content/text/)
- Block/Inline model with provenance
- Markdown projection
- PlainText projection
- Annotation ranges with absolute UTF-8 byte coordinates
- TextRenderer with policy options
- CodeBlockLabelPolicy, TaskListMarkerPolicy, SoftBreakPolicy, TableColumnSizing
- Line semantics (preserve existing newline/wrapping behavior)

---

## 8. Implementation Plan Sketch

**Prerequisite:** PERF-13 stable template exists as reference implementation.

Building from iyon-tui experience (131-146 commits/day peak velocity suggests
what's possible when the architecture is clearly specified):

1. React reconciler host config with three-plane dispatch classification
2. Structural bridge: host config → GPUI element tree (transport TBD)
3. State bridge: typed patches → native state arena → effect classification
4. Content bridge: Source/Funnel/Connector/Port → FFI byte transport (if chosen)
5. Render cache integration: LayoutCache + PaintCache + damage into GPUI frame loop
6. Benchmarking against gpui-shell/gpui-x and raw GPUI

---

## 9. Open Research Questions

- What is the correct bridge transport for each plane?
- Does GPUI's existing element system accommodate typed state patches, or does
  it need a parallel state arena?
- How does the fiber tree's identity tracking interact with NativeRef caching?
- Can GPUI's TextBuffer absorb the content plane, or is a separate TextStore needed?
- How do cold connector semantics compose with GPUI's element visibility model?
- What is the correct ABI generation scope? (Smaller than iyon-tui's full View
  schema, but still needed.)
- Does the three-plane architecture actually solve the performance problems
  that gpui-shell/gpui-x encountered, or are there other bottlenecks?

---

## 10. Success Criteria (Tentative)

- A geometry state change (padding, bounds) produces no React re-render beyond
  the mutation call site — state plane handles it
- A presentation state change (color, style) produces no structural bridge traffic
- A content append (text streaming) produces no React work — content plane handles it
- A structural change follows normal React reconciliation path — this is the
  expected slow path, but also the rare path
- Frame times at or near raw GPUI for equivalent workloads
- Cache hit rate > 90% for state-only mutations in production-like workloads
- No regressions against iyon-tui's existing performance gates where applicable