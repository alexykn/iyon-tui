# 32 — State mutation: TypeScript state APIs through Rust invalidation and output

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `32 — traces/state-mutation`
- Framework boundary: `iyon-tui` and its TypeScript facade are generic terminal mechanics. The state API investigated here controls caller-supplied geometry, presentation, style-state, layout, paint, and scheduling behavior; no Iyon-agent semantics were found or assumed.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`

The current source is treated as authoritative for executing behavior. Documentation/comments are identified as explanatory evidence, not as substitutes for source behavior.

### Scope

This report follows retained state from:

```text
TypeScript public state API
  → TypeScript normalization and generated envelope
  → N-API NativeViewState wrapper
  → Rust HostViewState / HostInner
  → ViewStateRegistry / ViewStateRecord
  → Rust effects and invalidation worklists
  → StateCandidateOverlay / StateFrameView
  → semantic state attachment and resolved layout
  → retained layout tree / paint
  → terminal candidate frame, backend receipt, visible commit
```

The primary concern is the state plane:

- declared/base semantic values;
- sparse retained overrides;
- effective values;
- geometry and presentation deltas;
- Rust effect classification;
- layout and paint invalidation;
- state snapshot transport;
- frame preparation, failure, rollback, retry, and output visibility.

Structural composition, content/source/projection internals, input routing, and general terminal implementation are discussed only where state mutation crosses those boundaries.

### Evidence status

This is a read-only static investigation. I did not edit project files, install dependencies, run agents, or execute tests/builds/benchmarks. All behavior claims are based on source inspection. No runtime output was observed during this assignment.

Where physical line references are given, they are based on the inspected files at the stated baseline. The most important references are:

- TypeScript API: `packages/iyon-tui/src/api/view/retained-state.ts:25-235`
- TypeScript normalization/packing: `packages/iyon-tui/src/transport/state/control.ts:36-359`
- Generated envelope: `packages/iyon-tui/src/transport/state/generated/state_envelope.ts:1-375`
- TypeScript runtime creation/lifecycle: `packages/iyon-tui/src/runtime/runtime.ts:577-599`, `:783-815`, `:913-917`
- TypeScript attachment validation: `packages/iyon-tui/src/runtime/attachments.ts:181-255`
- Structural lowering: `packages/iyon-tui/src/transport/structural/retained-dag.ts:1055-1087`
- Native N-API wrapper: `crates/iyon-tui-native/src/tui/view_state.rs:1-285+`
- Host wrapper: `crates/iyon-tui/src/application/view_state.rs:1-144`
- Rust state effects: `crates/iyon-tui/src/retained_state/effects.rs:1-66`
- Geometry overrides/effects: `crates/iyon-tui/src/retained_state/geometry.rs:1-334`
- Presentation overrides/effective derivation: `crates/iyon-tui/src/retained_state/presentation.rs:1-234`
- Mutable records/revisions: `crates/iyon-tui/src/retained_state/record.rs:1-194`
- Registry/versioning/binding: `crates/iyon-tui/src/retained_state/registry.rs:1-419`
- Candidate overlay: `crates/iyon-tui/src/retained_state/capture.rs:1-155`
- Host invalidation and frame integration: `crates/iyon-tui/src/application/host.rs:1702-1797`, `:1885-2355`
- Scene invalidation/local refresh: `crates/iyon-tui/src/scene/host.rs:341-350`, `:641-710`, `:1344-1698`
- Resolution/layout state use: `crates/iyon-tui/src/scene/resolve.rs:95-106`; `crates/iyon-tui/src/presentation/layout/measure.rs:200-225`, `:254-369`
- Layout-tree application: `crates/iyon-tui/src/presentation/layout/tree.rs:278-316`
- State counters: `crates/iyon-tui/src/perf.rs:41-52`

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Area | Path | Approximate physical size | Responsibility | Plane |
|---|---|---:|---|---|
| TS public state API | `packages/iyon-tui/src/api/view/retained-state.ts` | ~235 LOC | `ViewState`, typed geometry/presentation patches, clear operations, mutation wake handling | TypeScript state/public API |
| TS state normalization | `packages/iyon-tui/src/transport/state/control.ts` | ~359 LOC | Validation, semantic color/style lowering, clear-list validation, envelope construction | TypeScript state transport |
| Generated TS envelope | `packages/iyon-tui/src/transport/state/generated/state_envelope.ts` | ~375 LOC | Generated property IDs, masks, fixed word/string lanes, packing functions | Generated ABI |
| TS native contract | `packages/iyon-tui/src/transport/native/addon.ts` | ~247+ LOC | Private `NativeViewStateContract`, host state operations, wake primitive | TS/native boundary |
| TS semantic attachment | `packages/iyon-tui/src/api/view/view.ts` | state-related spans around ~409-440, ~578-594 | Attach a `ViewState` identity to a semantic occurrence | Structural/state bridge |
| TS attachment preparation | `packages/iyon-tui/src/runtime/attachments.ts` | state-related spans ~181-255 | Candidate traversal, uniqueness checks, resource lease preparation | Runtime lifecycle |
| TS structural lowering | `packages/iyon-tui/src/transport/structural/retained-dag.ts` | state-related spans ~1055-1087 | Resolve opaque TS handle to host-local native state identity | Structural transport |
| TS runtime owner | `packages/iyon-tui/src/runtime/runtime.ts` | state-related spans ~577-599, ~783-815, ~913-917 | Create/own/dispose state, wake broker integration, mutation guard | Runtime lifecycle |
| N-API state wrapper | `crates/iyon-tui-native/src/tui/view_state.rs` | ~700+ LOC including decoders | Decode mask/lane envelopes, validate, call canonical Rust state API, expose wake bit | Native addon |
| Rust host wrapper | `crates/iyon-tui/src/application/view_state.rs` | ~144 LOC | Weak host identity, host lock access, mutation/disposal adaptation | Host/native integration |
| Rust state effects | `crates/iyon-tui/src/retained_state/effects.rs` | 66 LOC | Consequence bitset and presentation effect classifier | Retained-state policy |
| Rust geometry state | `crates/iyon-tui/src/retained_state/geometry.rs` | 334 LOC | Sparse geometry overrides, effective geometry, property-specific invalidation | State/layout |
| Rust presentation state | `crates/iyon-tui/src/retained_state/presentation.rs` | 234 LOC | Sparse presentation overrides, effective decoration and style states | State/presentation |
| Rust mutable record | `crates/iyon-tui/src/retained_state/record.rs` | 194 LOC | Mutable source of truth, lifecycle, revisions, mutation acceptance | Retained state |
| Rust registry | `crates/iyon-tui/src/retained_state/registry.rs` | ~680+ LOC including tests | Identity allocation, ownership, desired/visible/in-flight bindings, versions, dirty worklist | Retained state/lifetime |
| Rust capture overlay | `crates/iyon-tui/src/retained_state/capture.rs` | 155 LOC | Immutable frame-time state view over committed versions | Frame state transport |
| Rust scene host | `crates/iyon-tui/src/scene/host.rs` | state-specific spans ~341-350, ~641-710, ~1344-1698 | Invalidation accumulation, cache invalidation, local geometry refresh, paint refresh | Layout/paint scheduling |
| Rust layout integration | `crates/iyon-tui/src/presentation/layout/{measure,tree,place}.rs` | state-specific spans | Revision-keyed measurement, effective geometry/style application, retained tree update | Layout/presentation |
| Rust application host | `crates/iyon-tui/src/application/host.rs` | state-specific spans ~1702-2355 | Candidate capture, preparation, backend handoff, receipt, state commit/rollback | Frame/output |

Approximate sizes are source-file physical LOC estimates from inspected ranges, not compiler-generated or semantic LOC. The state-specific portions of `application/host.rs`, `scene/host.rs`, `view.ts`, and `retained-dag.ts` are not counted as separate ownership; they remain part of their larger modules.

### 1.2 Primary versus secondary responsibility

The state design intentionally splits responsibilities:

1. **TypeScript owns caller-facing typing and ingress validation.**
   - Public callers choose values and invoke mutation methods.
   - TS validates shape, range, enum strings, colors, styles, and duplicate clear entries.
   - TS does not classify effects or decide layout/paint consequences.

2. **Generated ABI code owns representation, not policy.**
   - Property IDs, masks, fixed lanes, and packing are generated from `tools/tui-abi/view_abi.toml`.
   - Generated code validates lane-level representation again.
   - The wire does not carry effect classes.

3. **N-API owns decoding and wrapper lifecycle.**
   - `NativeViewState` checks wrapper liveness, decodes complete envelopes, then invokes `HostViewState`.
   - Decode failure occurs before canonical Rust mutation.

4. **Rust owns state authority and effect classification.**
   - `ViewStateRecord` stores mutable sparse overrides and revisions.
   - `StateEffects` is Rust-only and derived from actual before/after transitions.
   - Rust decides whether a property requires style resolution, paint, damage, geometry, measurement, placement, clipping, content projection, or ancestor propagation.

5. **Scene/layout owns effective realization.**
   - State snapshots are merged with immutable semantic base values at measure/layout time.
   - The retained layout tree stores effective decoration/style and can be updated locally.
   - State does not modify semantic topology.

6. **Application host owns frame visibility.**
   - Candidate frames are prepared separately from the last visible frame.
   - State binding/version changes become visible only after successful preparation and backend receipt/commit.

---

## 2. Types, APIs and contracts

### 2.1 Public TypeScript surface

`ViewState` is exported from the package root (`packages/iyon-tui/src/index.ts:88-96`). Its public operations are:

```ts
setGeometry(patch: ViewStateGeometryPatch): void
clearGeometry(...properties): void
setPresentation(patch: ViewStatePresentationPatch): void
clearPresentation(...properties): void
setStyleState(key, value): void
clearStyleState(key): void
```

The object is intentionally independent from `View` construction. The `View.state(state)` method only attaches the opaque state handle identity to a semantic occurrence; state storage belongs to the host/native runtime (`retained-state.ts:140-145`).

The supported geometry properties are:

- `width`: `"fit" | "fill"`
- `height`: `"fit" | "fill"`
- `padding`
- `minWidth`, `maxWidth`
- `minHeight`, `maxHeight`
- `gap`
- `alignment`
- `borderEdges`

The supported presentation properties are:

- `foreground`
- `background`
- `borderColor`
- `borderStyle`
- `borderGlyphs`
- `textAttributes`
- `style`

This is a sparse patch API. Omitted fields do not mutate existing values. For nullable fields:

- outer absence means “do not touch this override”;
- explicit `null` means “store an explicit nullable override”;
- clear removes the override itself and exposes the base semantic value again.

### 2.2 Declared/base/override/effective layers

The source has four distinguishable layers, although the public TypeScript API exposes only the state handle and patch methods:

| Layer | Owner | Representation | Meaning |
|---|---|---|---|
| Declared/base semantic value | TypeScript/Rust `View` | Immutable semantic width/height/decoration/style-state | Value authored into the semantic occurrence |
| Retained override | Rust `GeometryOverrides` / `PresentationOverrides` / `style_states` | Sparse host-owned mutable record | State mutation independent of rebuilding `View` |
| Effective value | Rust `EffectiveGeometry`, effective `Decoration`, `StyleStates` | Derived at frame/layout use | Base plus sparse override |
| Realized output | Rust layout tree, physical surface, terminal/backend | Physical geometry/cells/styles | Effective value after measurement, placement, painting, and output |

The semantic base remains unchanged when a state mutation is accepted. `GeometryOverrides::effective` combines override fields with base width/height/decoration/gap/alignment (`geometry.rs:124-167`). `ViewStateSnapshot::effective_decoration` applies presentation overrides after geometry decoration derivation (`presentation.rs:133-169`). `effective_style_states` overlays dynamic state values on base selector state (`presentation.rs:172-178`).

This separation is consequential: a state mutation can alter output without creating a new semantic `View` or changing structural topology.

### 2.3 Rust nullable and sparse semantics

`PresentationOverrides` documents the important nested option convention (`presentation.rs:12-20`):

- `None`: no retained override;
- `Some(Some(value))`: explicit override value;
- `Some(None)`: explicit nullable semantic value;
- clear restores the outer `None`.

Geometry uses the same outer/inner distinction for nullable bounds and border edges (`geometry.rs:35-49`).

Examples:

- `setPresentation({ foreground: null })` stores an explicit nullable foreground override, which can suppress the base foreground.
- `clearPresentation("foreground")` removes the override and restores the base foreground.
- `setGeometry({ minWidth: null })` explicitly removes the minimum bound from the effective geometry.
- `clearGeometry("minWidth")` exposes the base minimum bound again.

These are intentionally different operations.

### 2.4 Mutation authority and guards

TypeScript state mutations call `assertMutationAllowed` before entering native code (`retained-state.ts:159-203`). The runtime guard rejects mutations during a retained protocol pass or active retained execution scope (`runtime.ts:913-917`):

```text
protocolState.mutating && !protocolState.internalPublication
  OR activeExecutionScope() !== undefined
  → mutation forbidden
```

This prevents reentrant state changes while the runtime is normalizing/publishing a retained candidate.

The native host wrapper serializes all state access through the `HostInner` mutex (`application/view_state.rs:45-123`). No per-record mutex exists. The host registry owns the records, and wrappers carry a stable identity plus a weak host reference.

### 2.5 Node-kind capability contract

State identity may be attached only to an addressable presentation box. Rust `presentation_state_capable` marks concrete physical kinds as capable and rejects `ComponentSlot` (`retained_state/capabilities.rs:45-62`). The `View::native_with_state_attachment` path also checks this (`presentation/ir.rs:832-847`, `:880-887`).

Geometry has additional kind-specific validation:

- `gap` is only accepted on row, column, or grid;
- horizontal alignment is only accepted on text;
- vertical alignment is only accepted on row;
- component slots reject geometry;
- other geometry fields are accepted according to the exhaustive capability table (`capabilities.rs:65-95`).

Validation occurs both during desired binding and mutation validation against the desired/visible target kind.

### 2.6 Revisions

Each record stores:

```text
revision
geometry_revision
presentation_revision
```

(`record.rs:16-31`).

A successful geometry mutation increments `revision` and `geometry_revision`. A successful presentation or style-state mutation increments `revision` and `presentation_revision`. No-op writes do not advance revisions.

The layout measure key incorporates the applicable geometry and presentation revisions (`presentation/layout/measure.rs:200-219`). This is the cache invalidation contract for state-dependent measurement.

---

## 3. Dependency and ownership map

### 3.1 Ownership diagram

```text
TS Tui runtime
  owns ViewState wrapper
  owns semantic attachment reference
        |
        | opaque HandleId in SemanticViewNode
        v
TS NativeResourceRegistry
  resolves handle only during attachment preparation/lowering
        |
        | native stateId / host-local u64
        v
NativeViewState (N-API wrapper)
  owns wrapper liveness flag
  owns HostViewState clone
        |
        | weak host reference + immutable state id
        v
HostInner
  owns ViewStateRegistry
        |
        +--> records: HashMap<u64, Box<ViewStateRecord>>
        |      mutable source of truth
        |
        +--> committed: HashMap<u64, Arc<ViewStateSnapshot>>
        |      immutable demanded versions
        |
        +--> dirty: HashSet<u64>
        |      deduplicated capture worklist
        |
        +--> desired / visible / in_flight binding sets
        |
        v
StateCandidateOverlay
  frame-lifetime Arc versions
        |
        v
ResolutionOverlay.states
        |
        v
measure/layout
  base View + ViewStateSnapshot
  → effective geometry/decoration/style states
        |
        v
LayoutTree / StableScene
        |
        v
paint / Physical Surface
        |
        v
Prepared frame
        |
        v
backend receipt
        |
        v
visible frame commit
```

### 3.2 Create, retain, and destroy ownership

- `Tui.viewState()` calls the native host's `viewState()` constructor (`runtime.ts:577-590`; `crates/iyon-tui-native/src/tui.rs:839-846`).
- `TuiHost::create_view_state` allocates a host-local registry record and returns `HostViewState` (`application/host.rs:1054-1061`).
- The registry starts IDs at one and embeds host identity in the high bits:
  - host ID is bounded to `0x001f_ffff`;
  - local ID is bounded to `u32::MAX`;
  - identity is `(host_id << 32) | local_id` (`registry.rs:68-81`).
- TS owns a wrapper and resource-registry lease.
- Rust `HostInner` owns the mutable record even if the TS wrapper remains live or is garbage-collected.
- A state record may not be disposed while desired, visible, or in-flight (`registry.rs:327-345`).
- Host teardown explicitly clears bindings and disposes the host-owned registry (`runtime.ts:783-815`; `registry.rs:357-389`).

The state wrapper's lifetime is therefore not the same as an occurrence's lifetime. One state object can be detached, rebound later, and remain valid while unmounted, but committed immutable snapshots are retained only while demanded.

### 3.3 Binding sets

The registry tracks three overlapping states:

- `desired`: the latest accepted semantic root's state attachments;
- `visible`: state attachments in the last successfully committed frame;
- `in_flight`: state attachments pinned by a prepared candidate whose backend receipt/commit is outstanding.

A record's `demanded` status is the union of those bindings. The distinction prevents a newer mutation or structural root from accidentally entering an older in-flight candidate.

### 3.4 Forward and reverse edges

Forward:

```text
ViewState.setGeometry/setPresentation/setStyleState
  → NativeViewState.set*
  → HostViewState.set*
  → HostInner.mutate_view_state
  → ViewStateRegistry.mutate_record
  → ViewStateRecord.apply_*
  → StateEffects
  → HostInner.invalidate_state
  → SceneHost.invalidate_state
  → environment pending epoch
  → candidate capture and render
```

Reverse:

```text
semantic occurrence stateAttachment
  → state target collection
  → registry desired binding
  → committed snapshot demand
  → frame state overlay
  → layout tree state_roots[state_id]
  → local repaint/relayout or full retained-root layout
```

Output-side ownership is deliberately staged:

```text
StateEffects does not directly paint or write terminal bytes.
StateEffects marks SceneHost work.
SceneHost prepares a candidate.
HostInner only changes visible frame after backend success.
```

---

## 4. Execution paths and state transitions

### 4.1 Creation path

```text
TS tui.viewState()
  → Tui.prepareMutation("tui.viewState")
  → NativeTuiHost.viewState()
  → TuiHost.create_view_state()
  → ViewStateRegistry.create(host_id)
  → HostViewState::new(id, weak host)
  → NativeViewState::from_host
  → TS createViewState wrapper
```

The initial record contains:

```text
lifecycle = Live
desired_bound = false
visible_bound = false
in_flight_bound = false
geometry = default
presentation = default
style_states = {}
revision = 0
geometry_revision = 0
presentation_revision = 0
```

Creation does not publish a committed snapshot and does not dirty the host (`registry.rs:41-65`, creation test around `:455-460`).

### 4.2 Attachment path

```text
const state = tui.viewState()
const view = text("x").state(state)
```

`View.state` checks that the wrapper is live, then attaches the opaque handle identity (`view.ts:409-415`). It does not mutate the state record.

During candidate preparation:

1. `prepareSemanticAttachments` traverses the semantic graph.
2. It detects duplicate state identity use and rejects it with `DUPLICATE_VIEW_STATE_ATTACHMENT` (`attachments.ts:181-255`).
3. It prepares a native resource lease, including target node kind.
4. Structural lowering resolves the TS resource and calls `stateId()` (`retained-dag.ts:1055-1087`).
5. The native structural representation stores the host-local u64 state identity, not a TS object pointer.
6. Rust `View::native_state_attachment_targets` collects `(state_id, StateNodeKind)` pairs and rejects duplicate identities/cycles (`presentation/ir.rs:901-998`).
7. `TuiHost::set_desired_view` validates targets and calls `ViewStateRegistry::set_desired` (`application/host.rs:1091-1120`).

The state object is therefore attached to a semantic occurrence before it is bound to desired/visible frame state.

### 4.3 Geometry mutation path

```text
ViewState.setGeometry(patch)
  → assertMutationAllowed()
  → normalizeGeometryPatch()
  → geometryEnvelope()
  → NativeViewState.setGeometry(setMask, nullMask, clearMask, words, strings)
  → decode_geometry_envelope()
  → HostViewState.set_geometry()
  → HostViewState::mutate()
  → HostInner.mutate_view_state()
  → ViewStateRegistry.mutate_record()
  → ViewStateRecord.apply_geometry()
  → GeometryOverrides.apply_patch()
  → StateEffects
  → HostInner.invalidate_state()
  → SceneHost.invalidate_state()
  → HostInner.mark_pending()
  → environment drain / flush
```

The TS layer normalizes width/height modes, padding, bounds, gap, alignment, border edges, and ranges (`control.ts:36-174`). The generated encoder creates masks and fixed lanes (`state_envelope.ts:106-225`).

The native wrapper decodes the entire envelope before calling Rust state storage. Its comment explicitly promises atomic decode semantics: malformed input does not leave a partial override (`native/view_state.rs:68-86`).

At the Rust record:

- a patch is applied to a clone of the current geometry override;
- effects are computed against the current value;
- if unchanged, the mutation is a no-op;
- if changed, kind validation runs before replacing the record;
- revision counters advance only on acceptance (`record.rs:71-110`).

### 4.4 Presentation mutation path

`setPresentation` follows the same transport chain but calls `ViewStateRecord.apply_presentation`.

TypeScript normalization:

- colors become named/theme/ANSI/RGB transport representations;
- border styles and glyphs are normalized;
- text attributes are validated against the shared vocabulary;
- style refs/specs are lowered to a normalized semantic style object (`control.ts:176-302`).

Presentation effects are classified as:

```text
RESOLVE_STYLE
DAMAGE
PAINT_SELF
```

For a style-state mutation, Rust additionally uses:

```text
PAINT_SUBTREE
```

because selectors may observe inherited state (`effects.rs:52-66`).

The presentation patch is sparse. `TextAttributeSpec::overlay` merges only supplied attribute fields. `clearPresentation("textAttributes")` resets the complete sparse attribute spec.

### 4.5 Style-state mutation path

```text
ViewState.setStyleState(key, value)
  → normalize nonempty strings in TS
  → native setStyleState(key, value)
  → BTreeMap<String, String>
  → presentation_effects(true)
  → subtree paint invalidation
```

The dynamic map is intentionally separate from fixed ABI properties. The N-API wrapper comments state that dynamic style-state keys remain a typed operation and are not assigned generated property IDs (`native/view_state.rs:142-162`).

`clearStyleState` removes the dynamic key. If the key is absent, the operation is a no-op. If a base semantic style-state with the same key exists, removal of the retained override exposes that base value again through `effective_style_states`.

### 4.6 Clear path

`clearGeometry` and `clearPresentation` have two forms:

- omitted property list: clear the whole domain;
- explicit property list: clear only those sparse fields.

TypeScript validates that clear lists contain known properties and no duplicates (`control.ts:79-96`, `:214-230`). The generated mask is then passed with `clearAll` (`control.ts:338-358`).

Rust applies clear to a cloned override record, derives effects from before/after difference, validates geometry against the bound node kind, and only then commits the replacement (`geometry.rs:98-121`; `record.rs:92-110`).

### 4.7 Capture path

Each accepted mutation:

1. mutates the host-owned mutable record;
2. if the state is demanded, publishes a new `Arc<ViewStateSnapshot>` into `committed`;
3. inserts the ID into a deduplicated `dirty` set.

Unbound state records do not receive committed snapshots until a frame first demands them. This avoids one immutable duplicate for every detached state (`registry.rs:84-120`).

At capture:

1. `capture_epoch` increments.
2. The demanded set is `desired ∪ visible ∪ in_flight`.
3. Changed or newly demanded states are copied into a `StateCandidateOverlay`.
4. Clean demanded IDs remain served through the committed table.
5. Dirty marks for demanded states are drained; unrelated unmounted dirty marks remain queued (`registry.rs:123-164`).

`StateFrameView` reads the candidate overlay first and the committed table second (`capture.rs:74-114`).

### 4.8 Effective realization

During resolve, only demanded IDs are copied into the resolution overlay (`scene/resolve.rs:95-106`).

During measure:

- the state snapshot is looked up by `view.state_attachment_id()`;
- `geometry_revision` and `presentation_revision` enter `MeasureKey`;
- effective geometry combines state overrides with base semantic geometry;
- effective decoration and style-state values are retained in the measured node (`measure.rs:200-225`, `:254-369`).

During placement, `OccurrenceBox::from_effective` stores the effective geometry, decoration, and style state. The retained `LayoutTree` indexes the state identity in `state_roots` and can apply a fresh snapshot directly to the occurrence (`tree.rs:278-316`).

### 4.9 Invalidation and frame transition

`HostInner.invalidate_state` first checks whether the state is bound. An unbound state mutation has no current output to invalidate and returns a default wake (`application/host.rs:1787-1797`).

For a bound state, the host forwards the ID and effect set to `SceneHost.invalidate_state`, which:

- inserts the state ID in `invalidated_states`;
- unions multiple effects for that ID in `invalidated_state_effects`.

This means several mutations before the next frame coalesce by identity and effect bit.

The next candidate path is:

```text
capture state overlay
  → StateFrameView
  → prepare_frame_with_content
  → SceneHost refresh/resolve/layout/paint
  → candidate state bindings prepared
  → candidate frame retained separately
  → backend presentation
  → receipt
  → commit frame and state bindings
```

A presentation-only state change can usually update retained style/effective occurrence state and repaint the affected state subtrees. A geometry effect may use a local geometry refresh if parent allocation is fixed; otherwise it relayouts the retained resolved root along the affected dependency frontier.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production paths

| Semantic operation | Primary production path | Selection/conditions | Failure or masking behavior |
|---|---|---|---|
| Create state | `Tui.viewState` → native host → registry create | Host open and not closed | Host creation error becomes TS runtime error; cleanup attempts native dispose |
| Attach state | `View.state` → semantic handle → attachment preparation → native state ID | State wrapper live; one identity per candidate; node kind capable | Duplicate identity rejected before candidate commit |
| Geometry set | TS normalize/encode → N-API decode → `apply_geometry` | Any supported field; current target kind validates geometry | Invalid envelope is rejected before mutation; invalid kind rejects whole patch |
| Geometry clear | TS clear mask → Rust `GeometryOverrides.clear` | Explicit fields or whole geometry domain | Unknown/duplicate fields rejected in TS; state remains unchanged on failure |
| Presentation set | TS normalize/encode → Rust presentation patch | Supported fields; target is a presentation-capable node | Malformed representation rejected before mutation |
| Presentation clear | Clear mask → `PresentationOverrides.clear` | Explicit fields or all fields | No-op if values were already absent |
| Style-state set | Dynamic key/value path → BTreeMap | Nonempty strings | Style-state mutation is subtree paint-conservative |
| Style-state clear | Dynamic key removal | Existing key required for accepted mutation | Missing key is a no-op |
| Bound state mutation | Record mutation → state effects → SceneHost invalidation | State desired/visible/in-flight | Wake is suppressed for unbound state |
| Unbound state mutation | Record mutation and dirty mark only | No current binding | It is retained for future mount but produces no immediate frame |
| Candidate preparation | Capture overlay → resolve/layout/paint | Pending epoch or dirty state | Candidate is discarded on late preparation failure |
| Backend presentation | Candidate frame → terminal/backend receipt | Headless or real backend | Visible frame remains old; state in-flight pins are cleared on discard |
| Backend receipt pending | `presentation` retained | Async real-terminal worker | Flush reports `waiting_for_presentation`; no newer candidate supersedes the exact receipt |
| Backend receipt failure | `BACKEND_IO_FAILED` or `BACKEND_NOT_READY` | Sink/worker failure | Candidate discarded, physical sync marked unknown, pending work rearmed |
| State commit failure | Prepared candidate has state/content plans | Commit-time preflight failure | Exact candidate retained for explicit retry; no new preparation pass |
| Host teardown | Runtime disposal → clear bindings → registry disposal | TUI close/host owner death | Host-owned state records are removed; wrappers become unusable |

### 5.2 Validation failure versus recovery

There are multiple failure classes, and they are not silently conflated:

1. **TypeScript ingress validation**
   - Wrong shape, unknown property, invalid enum, duplicate clear field, out-of-range integer, invalid color/style.
   - Failure occurs before native call.

2. **Generated envelope validation**
   - Mask/lane mismatch or invalid packed value.
   - Failure occurs in the generated encoder or Rust decoder before record mutation.

3. **State semantic validation**
   - State disposed, unknown identity, host closed, unsupported node kind, unsupported geometry property.
   - Canonical Rust record remains unchanged on failure.

4. **Candidate preparation failure**
   - A late layout/content/backend preparation failure can occur after SceneHost has staged derived data.
   - `HostInner` keeps the last complete logical frame authoritative, aborts candidate content, discards SceneHost candidate state, and records failed attempt coordinates (`application/host.rs:1885-1924`).

5. **Backend presentation failure**
   - A candidate may have been prepared but not become physically visible.
   - Physical synchronization becomes unknown and the candidate is discarded; pending work is restored (`application/host.rs:1940-1949`, `:1961-2023`).

6. **Commit failure after receipt**
   - The exact candidate is retained and retried instead of building a newer candidate over an uncommitted receipt (`application/host.rs:2124-2157`).

### 5.3 Silent fallback search results

The state path contains deliberate compatibility/test handling:

- `retained-dag.ts:1067-1071` allows a validation-only state fixture lacking a native `stateId()` operation to proceed without lowering a state attachment.
- This is explicitly labeled as an internal fixture path, not a production state implementation.
- No old JSON state map, legacy effect-classification transport, or direct TS-to-terminal state route was found in the inspected state files.
- Dynamic style-state intentionally bypasses generated property IDs, but it is not a fallback; it is a separate declared operation with its own Rust storage and effect classification.

The main architectural hazard is not a hidden fallback in state transport but the number of deliberate candidate/visible/in-flight routes. These are required to preserve old visible output while new state mutations or structural updates are pending.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 State cache/version keys

The state registry has two levels of retention:

```text
mutable records: all live state objects
committed snapshots: only demanded states
candidate overlay: changed/newly demanded snapshots for one frame
```

The committed map is keyed by state identity. The candidate overlay is also keyed by state identity.

Layout cache keys include:

- geometry revision;
- presentation revision;
- view identity/measurement inputs;
- width;
- component identity where relevant;
- content layout revision for content hosts (`measure.rs:200-225`).

A presentation-only mutation can therefore invalidate style-dependent measurement keys without necessarily triggering geometry work. Geometry mutations change `geometry_revision` and carry geometry effects.

### 6.2 Dirty retention and coalescing

`ViewStateRegistry.dirty` is a `HashSet<u64>`, so repeated accepted writes to the same state before capture are coalesced. The registry tests explicitly assert:

- three accepted presentation writes produce one dirty entry;
- one capture drains it;
- a repeated identical write does not publish a new version or dirty the record (`registry.rs:463-484`).

Effects are separately unioned in `SceneHost.invalidated_state_effects`, so multiple changes to one identity accumulate consequences rather than causing multiple independent frame operations.

### 6.3 Effect classification

`StateEffects` is a 16-bit Rust-only bitset. It includes:

```text
RESOLVE_STYLE
PAINT_SELF
PAINT_SUBTREE
DAMAGE
GEOMETRY
MEASURE_SELF
MEASURE_ANCESTORS
PLACE_SELF
PLACE_DESCENDANTS
UPDATE_CLIP
DAMAGE_OLD
DAMAGE_NEW
PROJECT_CONTENT
INTRINSIC_WIDTH
INTRINSIC_HEIGHT
```

The public effects are intentionally not caller supplied.

#### Presentation

Ordinary presentation mutation:

```text
RESOLVE_STYLE
PAINT_SELF
DAMAGE
```

Style-state mutation:

```text
RESOLVE_STYLE
PAINT_SUBTREE
DAMAGE
```

#### Geometry

All geometry properties start with:

```text
GEOMETRY
PAINT_SUBTREE
DAMAGE_OLD
DAMAGE_NEW
```

Additional effects:

| Property | Additional consequences |
|---|---|
| Width | project content, measure self/ancestors, place self/descendants, update clip, intrinsic width + height |
| Padding | project content, measure self/ancestors, place self/descendants, update clip, intrinsic width + height |
| Min/max width | same width dependency set |
| Border edges | same width/content/measurement set and intrinsic width + height |
| Height | measure self/ancestors, place self/descendants, update clip, intrinsic height |
| Min/max height | same height dependency set |
| Gap | measure self/ancestors, place descendants, intrinsic width + height |
| Alignment | place self/descendants only; no intrinsic measurement |

The width classification conservatively includes intrinsic height because width can alter wrapping and therefore descendant height (`geometry.rs:280-334`).

### 6.4 Local versus root layout work

`SceneHost` first distinguishes state-only work from structural/component/body/history work.

For state-only work:

1. It clears or path-invalidates layout and paint caches.
2. It updates retained overlays with the new state snapshots.
3. Presentation-only state changes call `LayoutTree::apply_state_snapshot` and schedule incremental state painting.
4. Geometry changes determine whether local refresh is safe.

For a geometry change:

- `try_local_geometry_refresh` looks up `state_roots`.
- It checks whether the target's geometry can escape its fixed parent allocation.
- Parent dependency metadata and intrinsic-width/height effects determine whether it must climb to an ancestor frontier.
- If local refresh is safe, only the state subtree and affected component roots are patched.
- Otherwise, the retained resolved semantic root is relaid out after invalidating target-to-root cache paths (`scene/host.rs:641-710`, `:1637-1692`).

If physical geometry is unchanged after relayout, the retained surface is reused and only affected state subtrees repaint. If geometry changed, a full repaint is counted for the candidate.

### 6.5 Damage and old/new geometry

Geometry effects include both `DAMAGE_OLD` and `DAMAGE_NEW`. This is needed because changing padding/bounds/border/size can leave old cells that are no longer covered by the new box. The scene host computes layout geometry damage between retained and next trees (`scene/host.rs:1663-1675`).

Presentation-only damage marks the existing box, while style-state changes conservatively repaint descendants because inherited selectors may differ.

### 6.6 Scheduling

A successful bound mutation returns a primitive wake bit from native:

```text
STATE_WAKE_DRAIN = 1
```

TS `ViewState.mutate` requests the environment wake only if the returned bit includes `STATE_WAKE_DRAIN` (`retained-state.ts:211-214`). `HostInner.mark_pending` and the environment pending queue remain authoritative; the bit is an edge-trigger scheduling hint rather than a visibility guarantee.

Important scheduling behavior:

- unbound mutations do not wake;
- accepted bound mutations mark the host pending;
- multiple mutations coalesce into the same pending epoch;
- a candidate in-flight through an async backend receipt is not overwritten;
- newer work remains pending after older candidate commit;
- failures restore the retry obligation rather than clearing it.

### 6.7 Instrumentation

Relevant counters declared in `crates/iyon-tui/src/perf.rs:41-52` include:

- `ViewStateMutationsAccepted`
- `ViewStateMutationsNoop`
- `ViewStatePresentationInvalidations`
- `ViewStateStyleStateInvalidations`
- `ViewStateIncrementalPaints`
- `ViewStateDamageRects`
- `ViewStateFullDamageRepaints`
- `ViewStateGeometryInvalidations`
- `ViewStateGeometryRelayouts`
- `ViewStateGeometryLocalPatches`
- `ViewStateGeometryFullRepaints`
- `ViewStateDirtyPropagationNodes`

The counters provide visibility into whether a mutation was accepted/no-op, whether style or geometry invalidation occurred, whether geometry used local refresh, and whether cache propagation broadened to a full repaint.

---

## 7. Tests, benchmarks and observability

### 7.1 Behavioral tests found

The state implementation contains focused Rust tests for:

- explicit nullable presentation value versus clear (`presentation.rs:181-201`);
- effective presentation preserving base decoration while applying sparse overrides (`presentation.rs:203-234`);
- repeated identical presentation assignment as a no-op (`record.rs:179-194`);
- generationally unique state IDs (`registry.rs:445-452`);
- creation deferring initial snapshots (`registry.rs:454-460`);
- dirty-mark deduplication and capture draining (`registry.rs:463-484`);
- capture ignoring unmounted states;
- newly demanded states receiving snapshots;
- committed snapshot pruning after unbinding;
- in-flight snapshots surviving newer desired changes;
- clear operations producing new versions;
- visible binding validation before commit;
- empty state-frame view behavior and overlay shadowing (`capture.rs:129-155`);
- concrete node capability classification (`capabilities.rs:98-120`).

Semantic View tests also cover duplicate state attachment rejection (`presentation/ir.rs:2607-2621`).

### 7.2 Test contracts inferred

The tests establish several important contracts:

1. **No-op identity**
   - Repeating an equal mutation must not increment revisions, publish a version, dirty the state, or schedule a frame.

2. **Explicit null versus clear**
   - The state plane must preserve the distinction between explicit nullable value and absent override.

3. **Unmounted retention**
   - A detached state may remain mutable and retain its mutable source of truth without forcing snapshots or frame work.

4. **Demand-based capture**
   - Capture must not clone unrelated unmounted states.

5. **In-flight lifetime safety**
   - State snapshots and attachment records must stay alive until the backend candidate is either committed or discarded.

6. **Atomic validation**
   - A failed kind validation or malformed envelope must not partially mutate the record.

7. **Duplicate identity rejection**
   - One state identity may not appear more than once in one semantic candidate.

### 7.3 Observability limitations

The following are visible through counters or epoch APIs:

- accepted/no-op mutations;
- presentation/style-state invalidation count;
- geometry invalidation and relayout count;
- local versus full geometry patch count;
- dirty propagation node count;
- pending/committed/visible epochs;
- frame/backend error records.

The following are not directly exposed as a public state introspection API:

- current declared/base value versus effective value;
- current sparse override map;
- current state revision values;
- current `StateEffects` bitset;
- candidate overlay membership;
- whether a particular state is desired versus visible versus in-flight;
- exact cache keys or per-state cache retention.

This is consistent with keeping Rust effect authority internal, but it makes black-box diagnosis dependent on output comparison, host epochs, counters, and errors.

### 7.4 Validation execution status

No tests, benchmarks, or build commands were run for this report. The tests listed above are source evidence only.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Rust, not TypeScript, owns invalidation authority

The TypeScript API selects values but never submits an effect class. This is a strong and consequential ownership boundary:

```text
TS says what value changed.
Rust determines what that change means for layout, paint, damage, and scheduling.
```

This prevents callers from under-invalidating or over-invalidating the retained runtime and keeps layout policy out of the generic TS facade.

### 8.2 State mutation is independent of semantic topology

The retained state object stores only sparse overrides and dynamic style state. It does not own children, component identity, content source, or semantic topology. `View.state()` adds an attachment identity to an occurrence but does not clone or rebuild the semantic subtree.

This allows a state mutation to skip composition and structural publication in the common case. The SceneHost can patch a retained layout tree or repaint an affected subtree.

### 8.3 State crosses the structural boundary twice

There are two distinct representations:

1. TypeScript semantic graph stores an opaque branded `HandleId`.
2. Native structural transport stores a host-local state identity (`u64`) for frame overlays.

The state object itself is not sent across the wire. The identity is resolved only at the structural boundary, and the Rust host owns the corresponding record.

This keeps TS object lifetime and native state lifetime separate, but it creates a required invariant: the resource registry must resolve the same live state identity during candidate preparation as was used during semantic attachment preparation.

### 8.4 Attachment uniqueness is checked in multiple layers

Uniqueness is checked:

- TypeScript runtime attachment preparation (`attachments.ts:189-245`);
- Rust semantic `View` target traversal (`presentation/ir.rs:931-938`);
- host desired/visible registry target processing.

This is not simply duplicated validation: TS protects the TS/native resource lease boundary, while Rust protects the native semantic graph and host state registry. However, diagnostics differ by layer (`DUPLICATE_VIEW_STATE_ATTACHMENT` in TS versus generic duplicate state errors in Rust), so the same semantic error can have different textual forms depending on the route.

### 8.5 Geometry validation is target-kind dependent and can reject an otherwise valid stored patch

A state may be created and mutated while unbound. Geometry capability validation is deferred until the state is associated with a concrete node kind or bound target. Thus:

```text
valid while detached
  → later rejected when attached to an incompatible node kind
```

The registry deliberately validates at desired binding and mutation against desired/visible kind. This is necessary because the same state identity could otherwise carry geometry whose legality depends on the occurrence kind.

### 8.6 Component slots are intentionally not independently state-capable

`ComponentSlot` is a structural indirection, not an independently addressable physical box. `presentation_state_capable` rejects it, and its concrete component `View` owns the presentation state.

This is an important structural/presentation seam. A caller applying state to a component reference must attach state to the concrete occurrence that owns a box, not to the indirection node itself.

### 8.7 Presentation null and clear semantics are semantically significant

A potential source of confusion is that `Some(None)` can be an explicit effective value, while `None` means no override. This is correct but not obvious from a basic patch API.

The distinction matters for:

- foreground/background;
- border color/style/glyphs;
- style references;
- nullable bounds;
- border edges.

Any future transport or API simplification must preserve this three-state behavior, or clearing a retained override will no longer restore the declared base correctly.

### 8.8 Style-state invalidation is intentionally conservative

A dynamic style state can be observed by selectors in descendants, so Rust repaints the subtree rather than only the attached occurrence. This is a generic framework policy, not product-specific behavior. The tradeoff is correctness over minimal paint scope.

### 8.9 Candidate state snapshots are independent from visible state

The registry publishes new immutable snapshots when demanded, but `HostInner` does not expose them as visible until candidate frame commit. A mutation accepted while an older candidate is in flight updates desired mutable state and committed demand state, yet the old candidate keeps its own `Arc` snapshot.

This avoids “new state appears through an old backend receipt” races.

### 8.10 Failure semantics preserve old output

On preparation or presentation failure:

- the previously committed frame remains the authoritative visible frame;
- the candidate is discarded;
- in-flight state pins are cleared;
- pending work is restored;
- physical sync is marked unknown for backend failures;
- the next attempt is a recovery candidate.

This is stronger than merely returning an error: state/output visibility is explicitly transactional.

### 8.11 Potentially surprising interaction: committed snapshot publication precedes frame visibility

For a demanded state, `ViewStateRegistry::mutate_record` immediately replaces the committed immutable snapshot before the next candidate is prepared (`registry.rs:108-120`). “Committed” in that map means committed as the state-plane source/version table, not committed to terminal output.

The visible frame remains protected by `HostInner.frame` and candidate separation. Naming could be confusing to readers because `committed` has both state-version and visible-frame meanings in adjacent layers.

### 8.12 No product/application coupling found

The inspected state API and effects use generic concepts:

- geometry;
- presentation;
- style state;
- selector observation;
- content projection;
- layout/paint/damage.

No agent, assistant, tool, transcript, provider, or product-specific state semantics were found in these paths.

---

## 9. Open questions and coverage gaps

1. **Runtime route coverage**
   - I did not execute the native addon or TypeScript tests, so actual addon/build artifact parity was not observed.
   - The source includes generated ABI hashes, but this report does not independently regenerate or verify them.

2. **Generated Rust schema details**
   - `crates/iyon-tui-native/src/generated/view_state_schema.rs` was indexed through uses from `view_state.rs` but not reproduced exhaustively in this report.
   - The generated schema is evidently the source of property IDs, offsets, masks, counts, and wake constants; exact generator-side validation should be checked by the codegen/ABI assignments.

3. **Full presentation decoder**
   - The native wrapper was inspected through its public operations and decoder entry points. The fixed-lane decoder body is large; exact per-field Rust decoding details should be source-checked if a field-specific ABI discrepancy is suspected.

4. **Theme revision coupling**
   - State presentation revisions cover retained state mutations, but theme changes are a separate invalidation route. The exact interaction between a state snapshot's `StyleRef`, theme revision, and paint cache key belongs partly to the theme/presentation assignments.

5. **Output backend details**
   - This report traces candidate receipt and frame commit but does not census terminal escape generation or surface diff internals. It establishes the state-to-candidate boundary; backend byte-level behavior belongs to terminal/backend and paint assignments.

6. **Cross-host state reuse**
   - IDs embed host identity, and TS attachment/resource ownership appears host-bound. The exact error path if a state wrapper is attached to a different `Tui` host should be checked in the broader resource-registry tests.

7. **State mutation during internal publication**
   - The runtime guard permits `protocolState.internalPublication`. The inspected source shows the guard but not every caller that sets this flag. Whether state mutation during all internal publication phases is intentionally allowed should be confirmed against runtime/composition traces.

8. **State attachment in History**
   - `application/host.rs` explicitly collects state targets for history push/freeze/replacement. The state behavior is integrated, but a dedicated end-to-end history-state report should verify whether all history replacement and freeze routes preserve state snapshot semantics under failure.

9. **Counter exposure**
   - Counters exist in Rust, but this report does not establish which public benchmark/debug APIs expose every state counter or whether all counters are incremented on every alternate route.

10. **No claims about unused or obsolete paths**
    - This assignment did not perform a repository-wide production reachability census. The report identifies the inspected state paths and a fixture compatibility branch but does not claim that all other state-like code is unused.

---

## 10. Evidence appendix

### 10.1 Inspected source manifest

#### Documentation and instructions

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`

#### TypeScript public/state/runtime paths

- `packages/iyon-tui/src/index.ts`
- `packages/iyon-tui/src/api/view/retained-state.ts`
- `packages/iyon-tui/src/api/view/view.ts`
- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/iyon-tui/src/runtime/attachments.ts`
- `packages/iyon-tui/src/transport/native/addon.ts`
- `packages/iyon-tui/src/transport/state/control.ts`
- `packages/iyon-tui/src/transport/state/generated/state_envelope.ts`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`

#### Native addon paths

- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`
- `crates/iyon-tui-native/src/generated/view_state_schema.rs` (indexed through native decoder/schema references)
- `crates/iyon-tui-native/src/generated/view_abi_types.rs` and related generated ABI references were indexed but not analyzed as independent state logic

#### Rust retained state

- `crates/iyon-tui/src/retained_state/mod.rs`
- `crates/iyon-tui/src/retained_state/capabilities.rs`
- `crates/iyon-tui/src/retained_state/capture.rs`
- `crates/iyon-tui/src/retained_state/effects.rs`
- `crates/iyon-tui/src/retained_state/geometry.rs`
- `crates/iyon-tui/src/retained_state/occurrence.rs`
- `crates/iyon-tui/src/retained_state/presentation.rs`
- `crates/iyon-tui/src/retained_state/record.rs`
- `crates/iyon-tui/src/retained_state/registry.rs`

#### Rust host/scene/layout/output integration

- `crates/iyon-tui/src/application/view_state.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/presentation/ir.rs`
- `crates/iyon-tui/src/presentation/layout/measure.rs`
- `crates/iyon-tui/src/presentation/layout/tree.rs`
- `crates/iyon-tui/src/presentation/layout/place.rs`
- `crates/iyon-tui/src/perf.rs`

### 10.2 Exact symbols and important ranges

| Concern | Symbols |
|---|---|
| Public state wrapper | `ViewState`, `createViewState` |
| TS geometry/presentation APIs | `ViewState.setGeometry`, `clearGeometry`, `setPresentation`, `clearPresentation`, `setStyleState`, `clearStyleState` |
| TS packing | `normalizeGeometryPatch`, `geometryEnvelope`, `normalizePresentationPatch`, `presentationEnvelope` |
| Generated wire contract | `StateEnvelope`, `STATE_WAKE_DRAIN`, `encodeGeometryEnvelope`, `encodePresentationEnvelope` |
| Native contract | `NativeViewStateContract`, `NativeViewState` |
| Rust wrapper | `HostViewState::set_geometry`, `set_presentation`, `set_style_state`, `mutate` |
| Mutable state | `ViewStateRecord`, `ViewStateLifecycle` |
| Geometry state | `GeometryOverrides`, `EffectiveGeometry`, `geometry_effects` |
| Presentation state | `PresentationOverrides`, `ViewStateSnapshot::effective_decoration`, `effective_style_states` |
| Effects | `StateEffects`, `presentation_effects` |
| Registry | `ViewStateRegistry::create`, `mutate_record`, `capture_candidate`, `prepare_candidate`, `commit_prepared`, `dispose` |
| Frame state transport | `StateCandidateOverlay`, `StateFrameView` |
| Host invalidation | `HostInner::invalidate_state`, `SceneHost::invalidate_state` |
| Local geometry route | `SceneHost::try_local_geometry_refresh` |
| State-only frame route | `SceneHost::resolve_incremental` state invalidation branch around `scene/host.rs:1533-1698` |
| Effective layout | `measure_node`, `ViewStateSnapshot::effective_geometry` |
| Layout retained patch | `LayoutTree::apply_state_snapshot` |
| Frame visibility | `HostInner::flush_pending_frame`, `present_frame`, `commit_frame`, `discard_candidate_frame` |

### 10.3 Static investigation method

The investigation used repository file discovery and source-content search over the baseline worktree. I traced definitions to direct consumers, then searched reverse references for:

```text
ViewState
setGeometry
setPresentation
setStyleState
StateEnvelope
StateEffects
effective_geometry
effective_decoration
StateFrameView
capture_candidate
mutate_record
invalidate_state
state_roots
apply_state_snapshot
ViewStateGeometry
ViewStatePresentation
```

No source files were modified. No build, test, benchmark, external web source, or runtime execution was used.

### 10.4 Files indexed but not read comprehensively

The following were encountered through imports/reverse references but were not read as part of the focused state trace:

- most unrelated TypeScript API modules;
- most content, history, interaction, backend, terminal, and theme modules;
- most generated ABI bodies unrelated to state envelopes;
- full test fixtures outside the state-related Rust tests;
- code-generator implementation outside the state schema references;
- full terminal surface/escape/diff implementation.

These limits do not affect the core state mutation-to-invalidation trace above, but they limit claims about repository-wide route completeness and backend byte-level behavior.