# 39 — slots-controls-animation

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `39`, `traces/slots-controls-animation`
- The atlas README identifies this assignment as: “TS slots/controls through native retained values and output,” with the goal of tracing “Creation vs first materialization vs update/replacement/tick/reset/destruction; identify multiple architectural routes.”
- Parent-added atlas documentation is outside the source baseline.

I read:

1. `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
2. `docs/architecture/atlas-4355c02/README.md`
3. `PRE-V5-ARCHITECTURE-REPORT.md`
4. `AGENTS.md`
5. `docs/architecture/atlas-4355c02/evidence/assignments.json`
6. Relevant entries in `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

The report follows the required broad headings and treats source as authoritative for current behavior. I do not make V5 disposition decisions.

### Scope

Primary inspected scope:

- TypeScript public/control facades:
  - `packages/iyon-tui/src/api/controls/view-slot.ts`
  - `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - `packages/iyon-tui/src/api/controls/text-input.ts`
  - `packages/iyon-tui/src/api/controls/history.ts`
  - `packages/iyon-tui/src/api/controls/output.ts`
  - `packages/iyon-tui/src/api/controls/framework-handle.ts`
- TypeScript runtime/composition and transport:
  - `packages/iyon-tui/src/runtime/runtime.ts`
  - `packages/iyon-tui/src/runtime/attachments.ts`
  - `packages/iyon-tui/src/runtime/handle-registry.ts`
  - `packages/iyon-tui/src/transport/native/resources.ts`
  - `packages/iyon-tui/src/transport/native/addon.ts`
  - `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `packages/iyon-tui/src/composition/compose.ts`
  - `packages/iyon-tui/src/composition/execution.ts`
  - `packages/iyon-tui/src/testing/index.ts`
- Rust retained controls and host/application path:
  - `crates/iyon-tui/src/application/host.rs`
  - `crates/iyon-tui/src/application/kernel.rs`
  - `crates/iyon-tui/src/scene/host.rs`
  - `crates/iyon-tui/src/component/tick.rs`
  - `crates/iyon-tui/src/controls/text_input/mod.rs`
  - `crates/iyon-tui/src/controls/text_input/output.rs`
  - `crates/iyon-tui/src/scroll.rs`
  - `crates/iyon-tui/src/output/{event,handle,router}.rs`
- N-API/native retained bridge:
  - `crates/iyon-tui-native/src/tui.rs`
  - `crates/iyon-tui-native/src/tui/view_abi.rs`
- Relevant tests:
  - `packages/iyon-tui/tests/tui_harness.test.ts`
  - `packages/iyon-tui/tests/tui_runtime.test.ts`
  - `packages/iyon-tui/tests/tui_realtime.test.ts`
  - `packages/iyon-tui/tests/tui_handles.test.ts`
  - `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
  - `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
  - `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
  - `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
  - selected Rust tests embedded in the inspected modules

### Evidence status

This is a static source inspection report. I did not run the test suite, build, benchmark, or native addon. Therefore:

- Behavior described as “observed” means observed in source control flow, not dynamically executed.
- Test references identify behavioral evidence present in the repository; they are not claims that those tests passed during this investigation.
- Line references are source line ranges from the baseline files as inspected.
- Approximate LOC counts below use physical source line ranges, including comments and blank lines unless otherwise stated; they are orientation figures, not compiler/token counts.

The framework boundary in `AGENTS.md` is respected: slots, controls, animation, output, native input, retained values, and terminal rendering are generic framework behavior. No application-specific meaning was found or inferred.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Language | Approx. physical LOC | Public API | Primary responsibility | Secondary responsibilities | Plane / hot path |
|---|---:|---:|---|---|---|---|
| `packages/iyon-tui/src/api/controls/view-slot.ts` | TypeScript | ~420 | Yes, `ViewSlot` | Host-owned component slot, direct/builder replacement, animation API, root lease lifecycle | Attachment validation, ownership transitions, native-ref staging, disposal | Host/control/structural; conditional hot path |
| `packages/iyon-tui/src/api/controls/scroll-pane.ts` | TypeScript | ~264 | Yes, `ScrollPane` | Host-owned retained scrolling component and content replacement | Builder ownership, root lease lifecycle, follow-end | Host/control/structural; conditional hot path |
| `packages/iyon-tui/src/api/controls/text-input.ts` | TypeScript | ~95 | Yes, `TextInput` | Host-bound text input facade and stable submitted output channel | Native-resource lookup, component occurrence creation | Native interaction/output; event hot path |
| `packages/iyon-tui/src/api/controls/history.ts` | TypeScript | ~132 | Yes, `History` | Detached/host-bound ordered history facade | Retained View import, freeze/discard, attach-once ownership | Content/history; conditional hot path |
| `packages/iyon-tui/src/api/controls/output.ts` | TypeScript | ~8 | Yes, opaque type | Type-level output-channel identity | Prevents caller-created invalid channels | Output identity; not computationally hot |
| `packages/iyon-tui/src/api/controls/framework-handle.ts` | TypeScript | ~66 | Partially | Opaque JS handle identity and common disposal/closed checks | Native resource delegation, error normalization | Lifetime/host boundary |
| `packages/iyon-tui/src/runtime/runtime.ts` | TypeScript | ~897 | Yes, `Tui` | Runtime/session owner, root scene publication, factory ownership, frame barriers | Environment registration, history sideband, shutdown, output/event APIs | Host/runtime; hot boundary |
| `packages/iyon-tui/src/runtime/attachments.ts` | TypeScript | ~350 | Internal | Desired/visible resource attachment lease ledger | Duplicate attachment detection, revision tracking, teardown | State/content/host synchronization |
| `packages/iyon-tui/src/runtime/handle-registry.ts` | TypeScript | ~80 | Internal | Framework handle IDs and disposal transition | Resource registry lifecycle | Lifetime boundary |
| `packages/iyon-tui/src/transport/native/resources.ts` | TypeScript | ~88 | Internal | Handle-local and environment-wide native resource lookup | Handle ID resolution and release | Native transport |
| `packages/iyon-tui/src/transport/native/addon.ts` | TypeScript | ~248 | Internal contract | TypeScript declaration of N-API host/control operations | Native version/bootstrap and diagnostics declarations | Native transport |
| `packages/iyon-tui/src/transport/structural/native-view-abi.ts` | TypeScript | >~700 | Internal | Native ABI session, retained ref acquisition, structural calls | Ref release, axis/buffer paths, exact-root support | Structural transport; hot path |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | TypeScript | ~2,140 | Internal | Semantic-node to NativeRef retained materialization and root lease protocol | Identity hints, stale recovery, style refs, prepared publication, desired/visible roots | Structural transport; very hot conditionally |
| `packages/iyon-tui/src/composition/compose.ts` | TypeScript | ~1,000+ | Internal composition | Dense semantic slot reuse and fresh immutable `View` construction | Component occurrence reuse, style/decorations, state attachments | Composition; hot path in builder mode |
| `packages/iyon-tui/src/composition/execution.ts` | TypeScript | ~1,253 | Internal runtime | Retained builder scopes, WIP evaluation, publication staging, commit/abort | State subscriptions, keyed ownership, builder-root ownership, scheduling | Composition/state; hot path in builder mode |
| `crates/iyon-tui/src/application/host.rs` | Rust | ~2,900 | Internal binding host | Host-bound controls, frame candidate/receipt lifecycle, output queue, native host wrappers | Animation state, control invalidation, N-API-facing host API | Host/backend/control; hot |
| `crates/iyon-tui/src/application/kernel.rs` | Rust | ~825 | Internal application kernel | Actions, component registry, scene host, ticks, rendering preparation | Output-to-action reduction, timers, input dispatch | Application/runtime; hot |
| `crates/iyon-tui/src/scene/host.rs` | Rust | >~1,300 | Internal | Mount graph, component invalidation, tick synchronization, retained scene resolution | Incremental component resolve, focus, output queue | Structural/layout/interaction; hot |
| `crates/iyon-tui/src/component/tick.rs` | Rust | ~305 | Internal | Mounted-component tick registration and scheduling | Registration activation/deactivation, changed-component reporting | Scheduling; hot for animated/ticking controls |
| `crates/iyon-tui/src/controls/text_input/mod.rs` | Rust | ~309 | Generic Rust API/internal | Unicode editor state and generic input component | Focus, paste, command mapping, layout synchronization | Native interaction/control; hot |
| `crates/iyon-tui/src/controls/text_input/output.rs` | Rust | ~74 | Internal support | Typed output projections for text changes | Output channel registration and emission | Output/event |
| `crates/iyon-tui/src/scroll.rs` | Rust | ~290 | Generic Rust control | Scroll mode, viewport anchoring, content extent, row viewport View | Focus/key command behavior, follow-end | Control/layout; conditional hot |
| `crates/iyon-tui/src/output/event.rs` | Rust | ~68 | Internal | Erased output event queue and `EventCx` | TypeId payload tracking | Output hot path |
| `crates/iyon-tui/src/output/handle.rs` | Rust | ~67 | Generic opaque API | Stable typed output channel IDs | Identity/equality/hash | Output identity |
| `crates/iyon-tui/src/output/router.rs` | Rust | ~100 | Generic API | Typed route registration and output-to-action conversion | Conflict/type mismatch handling | Output hot path |
| `crates/iyon-tui-native/src/tui.rs` | Rust/N-API | ~2,000 | Native binding surface | N-API wrappers for host, controls, output, frame and clock operations | Conversion and native lifetime checks | Native boundary |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Rust/native | >~5,000 | Generated-facing internal | Environment-local semantic cache, NativeRef table, ABI materializers and ref leases | Stale cache maintenance, style/path/builder/edit state | Structural native hot path |

### 1.2 Responsibility split

The control architecture is intentionally split into four layers:

1. **Semantic authoring and control facade**
   - TypeScript callers supply immutable `View` values and call control methods.
   - `ViewSlot.view()`/`ScrollPane.view()`/`TextInput.view()` produce semantic component occurrences.
   - TypeScript does not drive every animation tick.

2. **Native control state**
   - Rust `HostViewSlot`, `HostScrollPane`, and `HostTextInput` contain the mutable control state.
   - The N-API wrappers hold these Rust values and expose operations to TypeScript.
   - Rust owns frame selection, scroll state, text editing, focus, tick intervals, and native invalidation.

3. **Retained structural bridge**
   - TypeScript semantic nodes become NativeRefs through `retained-dag.ts` and `native-view-abi.ts`.
   - Rust native retained runtime stores semantic Nodes, NodeId→NativeRef correspondence, leases, and weak references.
   - A component semantic node carries a component handle identity; it does not encode the current child View payload directly.

4. **Frame/output kernel**
   - `SceneHost` resolves mounted components and produces a physical frame.
   - The host performs terminal/backend presentation and exposes committed screen rows.
   - Typed Rust output channels are routed to generic host output records; TypeScript receives only a route ID plus optional string payload.

### 1.3 Important lifecycle distinction

Control “creation,” semantic `View` occurrence creation, retained materialization, component mounting, and first visible frame are separate events:

- A `ViewSlot` or `ScrollPane` native control is created by `Tui.createViewSlot`/`Tui.createScrollPane`.
- Its initial seed `View` is already retained-materialized before the N-API control object is returned.
- Calling `.view()` creates a separate semantic component occurrence containing a local `HandleId`.
- That component occurrence is materialized when the surrounding scene is rendered through the retained ABI.
- The underlying native component may be registered before it is mounted in the current SceneHost graph.
- A first successful frame is later required before the control is visible.

This distinction is central to understanding why some updates need a root boundary and others only invalidate an already-registered native component.

---

## 2. Types, APIs and contracts

### 2.1 TypeScript public controls

#### `ViewSlot`

`ViewSlot` is declared in `view-slot.ts:50-57`:

```ts
interface ViewSlot extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setView(view: View | (() => View)): void;
  setAnimation(frames: readonly View[], intervalMs: number): void;
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): void;
  stopAnimation(view: View): void;
  revision(): number;
}
```

The interface deliberately exposes two different content-authoring modes:

- `setView(View)` — direct caller-owned immutable value.
- `setView(() => View)` — retained builder whose output is re-evaluated when tracked state changes.

Animation is separate from the builder/direct replacement path:

- `setAnimation(frames, intervalMs)`
- `setAnimationAtCycleBoundary(frames, intervalMs)`
- `stopAnimation(view)`

The class-level contract states that:

- construction is only through `Tui.createViewSlot()`;
- the owning `Tui` disposes factory-created slots on close/exit;
- the slot owns its builder root, current View identity, animation state, and root lease;
- one handle can be mounted at only one retained graph location;
- duplicate component nodes are rejected;
- direct and builder ownership transitions are transactional (`view-slot.ts:86-100`).

The implementation uses:

- `currentView?: View` (`view-slot.ts:102-106`);
- a `RetainedRootBoundary` (`view-slot.ts:108-113`);
- optional shared `RetainedExecutionRuntime` (`view-slot.ts:115-117`);
- optional `OwnedBuilderRoot` (`view-slot.ts:117`);
- attachment context and `AttachmentBindingState` (`view-slot.ts:118-119`).

#### `ScrollPane`

`ScrollPane` has the analogous direct/builder content contract:

```ts
interface ScrollPane extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setContent(view: View | (() => View)): void;
  followEnd(): void;
}
```

(`scroll-pane.ts:24-29`).

Its implementation owns:

- current content View;
- a root boundary;
- optional builder root;
- attachment bindings;
- shared retained execution runtime;
- native follow-end/scroll state, which is explicitly not reset by content rebuilds (`scroll-pane.ts:61-71`, `119-123`).

#### `TextInput`

`TextInput` is a host-bound control:

```ts
interface TextInput extends ComponentHandle {
  readonly kind: "text-input";
  text(): string;
  cursorBytes(): number;
  setText(value: string): void;
  clear(): void;
  submitted(): Output<string>;
  setMultiline(enabled: boolean): void;
  isMultiline(): boolean;
  view(): View;
}
```

(`text-input.ts:15-24`).

There is no TypeScript builder mode for `TextInput`; the mutable editor state remains native. `submitted()` returns a stable JS facade for one native output channel. The same `Output<string>` object is returned repeatedly after first creation (`text-input.ts:56-65`).

#### `History`

`History` supports two construction modes:

- `new History()` creates detached caller-owned storage;
- `Tui.createHistory()` creates a Tui-owned host-bound history.

The documented attach-once contract is in `history.ts:28-43`. `push` and `freeze` use the retained View materializer and then pass a NativeRef to native History (`history.ts:62-91`).

#### `Output<T>`

TypeScript `Output<T>` is intentionally opaque:

- private constructor;
- private brand and variance marker;
- `kind = "output"`.

(`output.ts:1-8`).

Only native-backed output objects are created by `TextInput.submitted()`, using `Object.create(Output.prototype)` and registration into the native-resource map (`text-input.ts:87-94`). There is no supported caller-side `new Output()` path.

### 2.2 Native contracts

The TypeScript private N-API contract exposes:

- `NativeViewSlotContract`:
  - `dispose`
  - `revision`
  - `componentId`
  - `setViewRef`
  - scalar animation setters for 1–4 refs
  - cycle-boundary scalar setters
  - buffer animation setters
  - `stopAnimationRef`
- `NativeScrollPaneContract`:
  - `dispose`
  - `componentId`
  - `setContentRef`
  - `followEnd`
- `NativeTextInputContract`:
  - `dispose`
  - text/cursor getters
  - text/clear/multiline setters
  - `submitted`
  - `componentId`
- `NativeTuiHostContract`:
  - control factories
  - `setDesiredViewRef`
  - frame flush and epochs
  - input dispatch
  - output retrieval
  - deterministic `advanceTime`
  - screen/history readback

(`packages/iyon-tui/src/transport/native/addon.ts:48-189`).

The N-API wrapper uses `resolve_native_view(runtime, ref)` to turn a positive u32 NativeRef into a cloned Rust `View` via `view_abi::view_for_ref` (`crates/iyon-tui-native/src/tui.rs:286-296`). The wrapper does not transmit View object payloads for slot/pane updates.

### 2.3 Semantic component identity

The semantic component occurrence is created by `componentViewForHandle(handleId, reference?)`:

- allocates a fresh semantic NodeId;
- stores `kind: component`;
- stores the local framework `HandleId`;
- optionally retains a strong reference to the underlying handle (`view.ts:596-601`).

`composeComponent` reuses a previous semantic component View when the local `HandleId` is unchanged (`compose.ts:434-450`). Therefore:

- the JS framework handle identity is not the same as the semantic occurrence NodeId;
- the semantic occurrence NodeId is not the same as the native component registry ID;
- neither is the same as a NativeRef.

The retained materializer for a component reads the component handle ID, lowers it to a u64 component identity, and calls `viewComponentCreate` (`retained-dag.ts:916-923`). The current mutable child View is recovered later from the native component registry during SceneHost rendering.

### 2.4 Native Rust control state

`HostViewSlot` stores:

```rust
struct ViewSlotState {
    view: View,
    revision: u64,
    frames: Vec<View>,
    pending_frames: Option<Vec<View>>,
    frame_index: usize,
    interval: Duration,
    last_tick: Option<Instant>,
}
```

(`crates/iyon-tui/src/application/host.rs:211-219`).

`HostScrollPane` wraps `ScrollPane` (`host.rs:203-208`), whose state is:

```rust
pub struct ScrollPane {
    content: View,
    mode: ScrollMode,
    layout_size: Option<Size>,
    content_extent: Option<Size>,
}
```

(`crates/iyon-tui/src/scroll.rs:11-29`).

`TextInput` stores editing state, focus, output channels, layout size, scroll row, and optional border (`controls/text_input/mod.rs:38-47`).

---

## 3. Dependency and ownership map

### 3.1 Main TS-to-native graph

```text
caller
  │
  ├─ Tui.createViewSlot(initialView)
  │    ├─ Tui.prepareMutation()
  │    ├─ createViewSlot(...)
  │    ├─ tryRetainedMaterializeRef(initialView)
  │    │    └─ ensureSemanticNative(...)
  │    │         └─ native retained ABI / NativeRef table
  │    ├─ NativeTuiHost.createViewSlotRef(ref)
  │    │    └─ NativeTuiHost.create_view_slot_ref(...)
  │    │         └─ HostViewSlot::new(View)
  │    │         └─ RunningApp::host_register(MountedViewSlot)
  │    └─ RetainedRootBoundary.adopt(initialView)
  │
  ├─ slot.view()
  │    └─ composeComponent(slot)
  │         └─ semantic component occurrence with HandleId
  │
  ├─ Tui.render(scene containing slot.view())
  │    ├─ retained execution/root publication
  │    ├─ ensureSemanticNative(component node)
  │    │    └─ viewComponentCreate(component ID)
  │    └─ Native host desired/visible root
  │
  └─ slot.setView / setAnimation / stopAnimation
       ├─ materialize transient View(s) to NativeRef(s)
       ├─ N-API NativeViewSlot operation
       ├─ HostViewSlot mutation
       ├─ component/frame invalidation
       └─ SceneHost resolve → layout → paint → backend frame
```

### 3.2 Ownership layers

| Resource | Created by | Current owner | Release/destroy path |
|---|---|---|---|
| JS `ViewSlot` facade | `Tui.createViewSlot` | `Tui.ownedHandles` plus caller reference | `ViewSlot.dispose`; builder root, retained boundary, attachment bindings, then FrameworkHandle/native dispose |
| Native `NativeViewSlot` N-API wrapper | `NativeTuiHost.createViewSlotRef` | JS native resource map | `disposeNativeResource` → N-API `dispose` → `HostViewSlot::retire` |
| Rust `HostViewSlot` | `TuiHost::create_view_slot` | `Arc` state plus component registry wrapper | deferred component retirement after unmount; host teardown drops host/control state |
| Native component registry entry | `RunningApp::host_register` | component registry until retired and proven unmounted | `reap_retired_components` after successful reconciliation |
| semantic component occurrence | `componentViewForHandle`/`composeComponent` | surrounding semantic View and/or execution slot | ordinary JS GC/immutable scope rollback; attachment strong reference may keep handle alive |
| semantic NodeId cache entry | retained native runtime | weak `View` cache | weak expiry, bounded maintenance, full sweep, runtime teardown |
| NativeRef slot | native retained runtime | temporary lease, boundary root lease, or unleased weak slot | `viewReleaseMany`; zero-lease weak expiry and scavenging |
| `HostViewSlotState.frames` | native Rust animation setter | `HostViewSlot` state | replaced on animation install/cycle update; dropped with host state |
| `TextInput` output channel | Rust `TextInput::new` | TextInput state and output router registrations | native control disposal/host teardown |
| JS output facade | `TextInput.submitted()` | `TextInput.submittedOutput` and weak reverse map | JS/native resource cleanup through owning TextInput |
| builder root | `OwnedBuilderRoot.start` | slot/pane/Tui root owner | `disposeOwnedBuilder`, control disposal, or Tui retained-runtime disposal |

### 3.3 Identity map

```text
TS FrameworkHandle.id (local branded HandleId)
   │
   ├─ resource registry HandleId → native control wrapper
   │
   └─ semantic component node.handleId
          │
          └─ retained ABI viewComponentCreate(component ID)
                 │
                 └─ Rust ComponentId / registry entry
                        │
                        └─ MountedViewSlot / MountedScrollPane / MountedTextInput
                               │
                               └─ mutable native control state
```

Separately:

```text
TS semantic View → semantic NodeId
   │
   └─ NodeId → NativeRef in native retained runtime
          │
          ├─ temporary NativeRef lease
          ├─ RetainedRootBoundary root lease
          └─ weak semantic/native cache acceleration
```

The architecture therefore has at least three independent identities in play:

1. local TS `HandleId`;
2. semantic `View` NodeId;
3. Rust component registry ID / NativeRef.

Confusing these would lead to incorrect lifecycle assumptions.

### 3.4 Native component mounting

Host factories create and register controls immediately:

- `TuiHost::create_text_input` creates `HostTextInput`, attaches its host weak reference, registers `MountedTextInput`, then stores its component ID (`host.rs:1151-1157`);
- `TuiHost::create_view_slot` does the same for `MountedViewSlot` (`host.rs:1160-1166`);
- `TuiHost::create_scroll_pane` does the same for `MountedScrollPane` (`host.rs:1169-1175`).

Registration is not equivalent to SceneHost mounting. Actual mount status is determined by the current resolved semantic scene. Deferred retirement explicitly checks whether the component ID remains in the successfully reconciled mount graph (`kernel.rs:128-143`; `scene/host.rs:251-256`).

---

## 4. Execution paths and state transitions

## 4.1 ViewSlot creation and first materialization

### Step 1: Tui factory entry

`Tui.createViewSlot(initialView)`:

1. calls `prepareMutation("tui.createViewSlot")`, which:
   - checks Tui open state;
   - drains pending retained execution;
   - rejects reentrant mutation (`runtime.ts:643-646`, `439-449`);
2. calls internal `createViewSlot` with:
   - native host;
   - initial View;
   - the Tui’s shared retained execution runtime;
   - attachment context (`runtime.ts:643-652`).

### Step 2: Initial seed retained materialization

`buildSlotHandle`:

1. validates any semantic attachments with `prepareAttachmentsForView(...).abort()`;
2. substitutes `View.spacer(0)` only if no initial View was supplied;
3. calls `tryRetainedMaterializeRef(seed)`;
4. explicitly throws if the retained path refuses;
5. invokes `host.createViewSlotRef(retained)`;
6. releases the temporary NativeRef lease in `finally` (`view-slot.ts:24-45`).

This is the first native materialization of the seed View. The temporary lease is not intended to remain owned by JS after the native HostViewSlot has cloned the View.

The initial materializer path is identity-first:

```text
semantic NativeRef hint
  → transaction-local reference
  → NodeId→NativeRef promotion if NodeId ≤ nativeLookupCeiling
  → derivation fast path
  → direct semantic materializer
```

(`retained-dag.ts:1148-1219`).

For genuinely new nodes, the NodeId promotion probe is skipped when the NodeId exceeds the previous boundary high-water (`retained-dag.ts:1174-1178`). New materialization recursively visits children before constructing parents, with per-kind direct materializers (`retained-dag.ts:1097-1120`).

### Step 3: Native HostViewSlot construction

N-API `create_view_slot_ref`:

1. validates the host;
2. resolves the NativeRef into a cloned Rust `View`;
3. calls `host.create_view_slot(View)` (`crates/iyon-tui-native/src/tui.rs:888-895`).

`TuiHost::create_view_slot`:

1. constructs `HostViewSlot::new(view)`;
2. stores a weak host reference;
3. registers `MountedViewSlot(slot.clone())`;
4. records the generated native component ID (`host.rs:1160-1166`).

Initial Rust state is:

- `view = seed`;
- `revision = 0`;
- no animation frames;
- `frame_index = 0`;
- default interval 480 ms;
- no `last_tick` (`host.rs:223-236`).

### Step 4: Boundary adoption

After native control construction, the TypeScript `ViewSlot` constructor:

1. stores `currentView = initialView`;
2. creates its own `RetainedRootBoundary` with an `installRef` callback that calls native `setViewRef`;
3. adopts the seed using `boundary.adopt(seed)`;
4. if there is an initial View, commits its semantic attachment bindings (`view-slot.ts:121-145`).

`RetainedRootBoundary.adopt` performs a ceiling-free NodeId→NativeRef promotion and transfers the returned lease into the boundary (`retained-dag.ts:1602-1641`). Thus the slot has a second, boundary-owned retained root lease after the temporary creation lease is released.

### Step 5: Semantic component occurrence

`slot.view()` does not return the seed View. It returns `composeComponent(this)` (`view-slot.ts:150-153`).

Outside a retained execution scope, this creates a semantic component occurrence through `componentViewForHandle(handleId, handle)` (`compose.ts:434-450`; `view.ts:596-601`). Inside a retained execution scope, the component occurrence is stored in a semantic slot and reused when the same local `HandleId` is seen again.

The component occurrence’s native materialization is separate from seed materialization. When the enclosing scene is rendered, `materializeComponentNode` lowers the component handle identity using `viewComponentCreate` (`retained-dag.ts:916-923`). The actual mutable `HostViewSlot` state is then consulted by the Rust component registry during SceneHost resolution.

### 4.2 Direct `ViewSlot.setView(View)` replacement

The direct path is implemented by `setViewDirect` (`view-slot.ts:204-228`):

1. reject reentrant mutation;
2. prepare semantic attachments for the new View;
3. call `boundary.prepareInstall(view)`;
4. if retained preparation refuses, throw `TUI_VIEW_SLOT_UPDATE_FAILED`;
5. commit the prepared publication;
6. commit desired and visible attachment bindings;
7. assign `currentView = view`;
8. only after successful publication, dispose any builder root.

The root boundary performs all fallible retained work before installation:

```text
keep old root leased
  → retained materialize new root / resolve hints / recover stale refs
  → acquire boundary lease if the new root was borrowed
  → call installRef(newRef)
  → release temporary leases
  → transfer root lease
```

The `installRef` callback for this boundary is the native `NativeViewSlot.setViewRef` operation (`view-slot.ts:132-138`).

N-API:

- `set_view_ref` resolves the NativeRef into a Rust `View`;
- calls `set_view_value`;
- `set_view_value` calls `HostViewSlot::set_view` (`crates/iyon-tui-native/src/tui.rs:1653-1663`).

Rust `HostViewSlot::set_view`:

- replaces `state.view`;
- clears all animation frames;
- clears pending frames;
- resets `frame_index = 0`;
- resets `last_tick = None`;
- increments `revision`;
- invalidates the host (`host.rs:239-253`).

The host invalidation path marks the component and frame dirty, then immediately advances/rendering from the control method (`host.rs:437-461`):

```text
HostViewSlot::set_view
  → HostViewSlot::invalidate_host
      → RunningApp::host_invalidate_component(component_id)
          → ComponentRegistry::invalidate
          → SceneHost::invalidate_component
          → RunningApp::invalidate_frame
      → HostInner::advance_and_render
          → pending frame preparation
          → SceneHost render/layout/paint
```

If the native install fails, the root boundary leaves the old root installed and leased. Attachments are aborted, and the builder root remains authoritative if a builder owned the slot before the attempted direct transition.

### 4.3 Direct `ViewSlot.setView(() => View)` builder mode

Builder mode requires a Tui-created slot and the shared `RetainedExecutionRuntime` (`view-slot.ts:181-188`).

First call:

1. creates an `OwnedBuilderRoot`;
2. constructs a root execution scope with a producer that returns the caller’s View;
3. assigns the slot’s `prepareSetView` as the publication target;
4. calls `mountExistingRoot`, which evaluates, stages, and commits the initial publication (`execution.ts:1185-1221`).

Subsequent calls to `setView(() => View)` replace the producer through `OwnedBuilderRoot.replaceProducer`:

1. save the previous producer;
2. optimistically assign the new producer;
3. synchronously invalidate/update the root scope;
4. on success, retain the new producer;
5. on failure, restore the old producer;
6. cancel only the retry obligation introduced solely by the failed producer attempt (`execution.ts:1224-1248`).

The retained execution sequence is:

```text
State write / explicit producer replacement
  → RetainedExecutionRuntime.invalidate(scope)
  → queue + microtask or synchronous flush
  → evaluate body under active scope
  → semantic slot reuse / fresh View construction
  → stage child publications recursively
  → prepare root publication
  → commit descendants and publications
  → slot boundary installRef(new root)
  → native control invalidation/frame
```

`RetainedExecutionRuntime` keeps WIP separate from committed state:

- `currentOutput` vs `pendingOutput`;
- `currentProps` vs `pendingProps`;
- committed child owner vs pending WIP child owner;
- current dependency subscriptions vs pending dependency subscriptions;
- semantic slot `current` vs `pending`.

(`execution.ts:109-149`).

Evaluation is synchronous and rejects Promise-like component results (`execution.ts:627-650`). Publication staging refuses atomically if the target returns `undefined` (`execution.ts:659-683`). Commit runs descendants before parents and only promotes staged publications after preparation succeeds (`execution.ts:709-781`).

### 4.4 `ViewSlot.setAnimation(frames, intervalMs)`

The TypeScript animation path deliberately does not send one JS/native call per frame tick. It materializes the caller-supplied array once per animation installation:

1. reject reentrant mutation;
2. reject empty frame list;
3. validate frame attachments;
4. use scalar NativeRef parameters for 1–4 frames;
5. use a reusable per-slot `Uint32Array` scratch buffer for larger frame arrays;
6. call `tryRetainedMaterializeRef` for every frame;
7. call one native scalar or buffer animation setter;
8. release every acquired temporary NativeRef lease;
9. dispose an owned builder root only after native animation installation succeeds (`view-slot.ts:284-334`).

The scalar/buffer split is a transport optimization, not a semantic mode:

- `setAnimationRef1` through `setAnimationRef4`;
- `setAnimationRefs(refs, usedCount, intervalMs)`;
- matching cycle-boundary forms (`view-slot.ts:68-84`, `345-363`).

For frame count greater than four, the JS scratch array is reused through a `WeakMap` keyed by the native slot wrapper (`view-slot.ts:48`, `337-343`). Temporary leases are released one-by-one even when the same View appears more than once; this avoids under-release of duplicate references (`view-slot.ts:323-328`).

#### Native animation installation

N-API `set_animation_refs`:

1. validates `used_count`;
2. resolves each u32 NativeRef to a cloned Rust `View`;
3. calls `set_animation_with_mode(frames, interval, false)` (`crates/iyon-tui-native/src/tui.rs:1666-1688`).

N-API scalar setters follow the same route through `set_animation_ref_values` (`tui.rs:1715-1726`, `1729-1764`). Cycle-boundary setters pass `true` (`tui.rs:1766-1805`).

`HostViewSlot::replace_animation`:

- validates non-empty frames;
- reads host time before taking the slot lock to avoid host/slot lock inversion;
- preserves phase only when there was a prior animation and the interval is unchanged;
- otherwise starts at frame zero and anchors `last_tick` to host time;
- sets the current View to the selected frame;
- replaces the stored `frames` vector;
- clears pending frames;
- updates interval;
- increments revision;
- invalidates the host (`host.rs:338-372`).

The native `HostViewSlotState` therefore owns strong Rust `View` clones for all animation frames. The JS NativeRefs are temporary transport leases; after installation, Rust control state owns the frame values.

#### Animation ticking

`MountedViewSlot` declares a fixed 16 ms component tick:

```rust
fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
    cx.tick(Duration::from_millis(16), Self::tick);
}
```

(`host.rs:608-626`).

The animation interval is not itself registered as the scheduler interval. Instead:

- scheduler cadence is 16 ms;
- each tick checks elapsed time against `state.interval`;
- only a due animation advances.

`HostViewSlot::tick(now)`:

1. if fewer than two frames exist:
   - clears `last_tick`;
   - returns `false`;
2. if there is no `last_tick`:
   - sets `last_tick = now`;
   - returns `true`;
3. if elapsed duration is below the configured interval:
   - returns `false`;
4. otherwise:
   - sets `last_tick = now`;
   - advances `frame_index = (frame_index + 1) % frames.len()`;
   - if the index wrapped to zero and `pending_frames` exists, swaps in pending frames;
   - updates `state.view`;
   - increments revision;
   - returns `true`.

(`host.rs:386-418`).

SceneHost turns a changed tick result into a component invalidation:

- `SceneHost::tick_due` invokes `TickScheduler::tick_due_with_events`;
- for every changed component, it calls `invalidate_component`;
- the resulting frame sees the changed `MountedViewSlot.view()` (`scene/host.rs:963-979`).

The scheduler itself only activates tick registrations for components present in the mounted graph. Registration alone is insufficient; `sync_mounts` activates/deactivates based on mount transitions (`component/tick.rs:77-115`). Tick execution is due-time based and schedules the next deadline at `now + interval` (`component/tick.rs:240-304`).

#### First tick and deterministic clock behavior

`AppHarness.advance(ms)`:

1. flushes retained/native pending work;
2. calls runtime access `advance(ms)`;
3. increments its public deterministic clock only after the native advancement succeeds (`testing/index.ts:102-114`).

The host adds the duration to `HostInner.now`, then calls `advance_and_render` (`host.rs:1405-1409`). A single large time jump advances an animation by at most one frame because `HostViewSlot::tick` sets `last_tick = now` rather than repeatedly catching up. This is an intentional fixed-step behavior visible in source.

### 4.5 Cycle-boundary animation replacement

`setAnimationAtCycleBoundary` has two native behaviors (`host.rs:294-336`):

#### Immediate installation case

If:

- current frame list has fewer than two frames, or
- interval changes,

then the new animation is installed immediately. Phase is preserved only when the old frame list is non-empty and the interval is unchanged.

#### Deferred installation case

If there is an existing multi-frame animation with the same interval:

- the current `state.view` remains unchanged;
- the new frame list is stored in `pending_frames`;
- no revision increment occurs;
- no host invalidation occurs.

At a native tick that wraps from the last old frame to index zero:

1. `pending_frames.take()` replaces the current frame list;
2. the new frame zero becomes `state.view`;
3. revision increments;
4. the changed component is invalidated and rendered.

Thus cycle-boundary replacement is a native state transition synchronized to frame-index wrap, not a TypeScript execution transaction.

### 4.6 Animation reset and stop

#### `setView` reset

`HostViewSlot::set_view` resets all animation state:

- `frames.clear()`;
- `pending_frames = None`;
- `frame_index = 0`;
- `last_tick = None`;
- `revision += 1`.

(`host.rs:239-253`).

This is the canonical transition from animation back to static content.

#### `stopAnimation(view)`

TypeScript `stopAnimation(view)`:

1. validates attachments;
2. materializes the stop View into a temporary NativeRef;
3. calls native `stopAnimationRef`;
4. releases the temporary NativeRef;
5. disposes any owned builder root (`view-slot.ts:365-380`).

Native `stop_animation_ref` resolves the ref and calls `HostViewSlot::stop_animation`, which delegates to `set_view` (`crates/iyon-tui-native/src/tui.rs:1837-1846`; `host.rs:374-376`). Therefore stopping animation has the same reset semantics as direct `setView`.

### 4.7 ScrollPane creation and content replacement

The creation path is parallel to ViewSlot:

1. TypeScript `Tui.createScrollPane(initialView)` calls `createScrollPane` with shared runtime and attachment context (`runtime.ts:658-670`).
2. `buildPaneHandle` validates attachments, retained-materializes the seed, calls `host.scrollPaneRef(ref)`, and releases the temporary NativeRef (`scroll-pane.ts:42-58`).
3. Native N-API resolves the ref and calls `TuiHost::create_scroll_pane` (`tui.rs:898-905`).
4. `HostScrollPane::new` wraps `ScrollPane::new(content)` and registers `MountedScrollPane` (`host.rs:465-473`, `1169-1175`).
5. TypeScript creates/adopts a root boundary and commits initial attachment bindings (`scroll-pane.ts:84-107`).

Direct `setContent(View)` follows the boundary replacement protocol:

- prepare retained root;
- native `setContentRef`;
- `HostScrollPane::set_content`;
- preserve scroll mode;
- reset `content_extent`;
- repair detached top row;
- invalidate/render (`scroll-pane.ts:189-218`; `host.rs:475-481`; `scroll.rs:46-54`).

Builder content uses `OwnedBuilderRoot` and the shared retained execution runtime in the same manner as ViewSlot (`scroll-pane.ts:125-145`).

#### Scroll behavior that survives content replacement

`ScrollPane` has two modes:

- `FollowEnd`;
- `Detached { top_row }` (`scroll.rs:11-15`).

`set_content` does not change the mode; it clears content extent and clamps a detached position (`scroll.rs:46-54`, `131-140`). `follow_end()` explicitly switches back to `FollowEnd` (`scroll.rs:72-84`).

The rendered View:

- fills width/height before layout is known;
- preserves intrinsic content if layout dimensions are zero;
- otherwise computes top row and creates a `row_viewport` (`scroll.rs:191-213`).

The native mounted wrapper registers:

- focusable capability;
- layout changed callback;
- content extent changed callback;
- key commands (`host.rs:555-605`).

This makes ScrollPane content replacement structurally separate from viewport state. Replacing content does not implicitly reset follow-end/detached semantics.

### 4.8 TextInput creation, native state, and output

#### Creation

`Tui.createTextInput(options)`:

1. prepares the runtime mutation;
2. lowers the optional border;
3. calls `host.textInput(multiline, border)`;
4. wraps the native result with `createTextInput`;
5. adds the handle to Tui-owned handles (`runtime.ts:632-640`).

N-API `NativeTuiHost.text_input`:

1. validates/lowers the border;
2. calls Rust `TuiHost::create_text_input`;
3. returns a host-bound `NativeTextInput` (`crates/iyon-tui-native/src/tui.rs:865-885`).

`TuiHost::create_text_input` registers `MountedTextInput` immediately (`host.rs:1151-1157`). The native N-API wrapper’s `NativeTextInput` delegates all methods to its `HostTextInput` when host-bound (`tui.rs:457-600`).

#### Semantic mounting

`TextInput.view()` uses `composeComponent(this)` (`text-input.ts:69-72`). The semantic component occurrence is the only structure inserted into the caller’s View tree. The mutable text state remains inside the native component.

`MountedTextInput.view()` obtains a cloned semantic View from `HostTextInput.view()` (`host.rs:751-758`). Its capabilities register:

- focus;
- focus transition callback;
- key command mapping/handling;
- paste;
- layout changed callback (`host.rs:760-766`).

#### Local editing

Rust `TextInput` owns the Unicode buffer and cursor. `set_text` and `clear` mutate native state and call `render_host`, which invalidates the mounted component and advances rendering (`host.rs:647-655`, `717-741`).

Key handling is local to Rust:

```text
terminal key
  → TuiHost.dispatch_key
  → RunningApp.dispatch_key
  → SceneHost.dispatch_key_local
  → MountedTextInput command mapping
  → TextInput::handle_command
  → native buffer mutation
  → optional EventCx output emission
  → component/frame invalidation
```

`TextInput` itself explicitly does not know application submission policy, terminal backend, or geometry (`controls/text_input/mod.rs:1-5`).

#### Submission output

Rust `TextInput` owns one stable `Output<String>` channel (`mod.rs:38-43`, `125-128`). On submit, command handling emits the current text via `EventCx`.

The generic output machinery:

- stores erased output events in `OutputQueue`;
- records output identity and payload `TypeId`;
- routes through `OutputRouter`;
- refuses a payload type mismatch;
- ignores events for which no route exists (`output/event.rs:8-67`; `output/router.rs:46-100`).

The host binding route is:

```text
TextInput.submitted()
  → NativeTextInput.submitted()
  → NativeTuiOutput { output: Output<String> }
  → JS Output facade
  → Tui.route(output, routeId)
  → NativeTuiHost.route(...)
  → TuiHost.route_text_input_output(...)
  → RunningApp.host_route(Output<String>, map)
  → OutputRouter route identity
  → submit command emits Output<String>
  → OutputQueue
  → SceneHost.drain_outputs
  → OutputRouter.drain
  → HostOutput::Routed { route_id, payload }
  → HostState.outputs
  → NativeTuiHost.nextOutput()/waitForOutput()
  → Tui.nextEvent()
```

The binding code is in:

- TypeScript `Tui.route` (`runtime.ts:682-693`);
- N-API route wrapper (`crates/iyon-tui-native/src/tui.rs:921-927`);
- host route registration (`host.rs:1281-1297`);
- host output extraction (`host.rs:1411-1413`, `1513-1535`);
- TypeScript event conversion (`runtime.ts:347-360`).

The output route carries only a caller-defined route string and text payload. The framework does not interpret route IDs.

### 4.9 History control path

History is not the main animation target, but it is a relevant control path because it imports arbitrary `View` values into native retained state.

`History.push(view)`:

1. checks host/detached lifetime;
2. calls `tryRetainedMaterializeRef(view)`;
3. passes the NativeRef to native `pushRef`;
4. releases the temporary lease (`history.ts:68-77`).

Host-bound native History validates state/content attachment targets against the current scene before pushing and invalidates the frame afterward (`host.rs:848-874`). `freeze` follows the same retained ref route (`history.ts:80-91`).

The important distinction is that History stores a cloned native `View` in its native history model; it does not retain a caller-side NativeRef lease indefinitely.

### 4.10 Destruction

#### ViewSlot

`ViewSlot.dispose()` ordering:

1. if a builder root exists, dispose it first while the native slot target remains valid;
2. close the retained root boundary;
3. clear boundary/current View;
4. dispose attachment bindings;
5. invoke `FrameworkHandle.dispose()` (`view-slot.ts:382-402`).

The framework handle disposal path:

1. marks the native resource registry entry as beginning disposal;
2. invokes native `.dispose()`;
3. releases the framework resource mapping;
4. restores registry liveness if native disposal fails (`handle-registry.ts:66-80`).

Native `NativeViewSlot.dispose()` performs an idempotent deferred retirement request (`crates/iyon-tui-native/src/tui.rs:1629-1645`). The registry entry is not necessarily reclaimed immediately because a committed scene may still reference the component. `RunningApp::reap_retired_components` removes it only after successful scene reconciliation proves it is no longer mounted (`kernel.rs:128-143`).

#### ScrollPane

`NativeScrollPane.dispose()` follows the analogous deferred retirement path. TypeScript `NativeScrollPane.dispose()`:

1. disposes builder root;
2. closes root boundary;
3. clears content/current state;
4. disposes attachment bindings;
5. invokes base handle disposal (`scroll-pane.ts:228-246`).

#### TextInput

TextInput has no custom TypeScript `dispose`; it uses `FrameworkHandle.dispose()`. Native `NativeTextInput.dispose()`:

- atomically marks itself dead;
- requests host input retirement if host-bound (`crates/iyon-tui-native/src/tui.rs:467-474`).

The host component is therefore retired through the same deferred component registry mechanism.

#### Tui close/exit

`Tui.close()`:

1. rejects reentrant close;
2. marks history liveness closed;
3. disposes retained execution:
   - host registration;
   - content resources;
   - attachments;
   - host state bindings;
   - resource registry host entries;
   - owned controls;
   - root builder;
   - retained execution runtime;
   - root boundary;
4. disposes the native host (`runtime.ts:822-841`).

`Tui.exit()` differs in that it first asks the native host to produce the final exit frame while content bindings still exist, then tears down retained resources (`runtime.ts:843-878`). Native `TuiHost::exit()` completes earlier presentation, marks the application exiting, renders the final frame, waits for presentation, transfers final rows to history/backend, disposes view states/content, and unregisters the host (`host.rs:1189-1239`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path matrix

| Semantic operation | TypeScript route | Native/Rust route | Selection condition | Failure behavior |
|---|---|---|---|---|
| Create ViewSlot | `Tui.createViewSlot` → `createViewSlot` → `buildSlotHandle` | `NativeTuiHost.createViewSlotRef` → `TuiHost::create_view_slot` | Tui-created host slot only | Retained seed refusal or native creation error fails creation explicitly |
| First `.view()` | `composeComponent(slot)` | Semantic component node later lowered by `viewComponentCreate` | Outside scope: fresh occurrence; inside scope: exact HandleId reuse | Disposed slot rejected; duplicate mounted component identity rejected downstream |
| Direct slot replacement | `slot.setView(View)` → boundary `prepareInstall` → `setViewRef` | `HostViewSlot::set_view` | Non-function argument | Retained refusal leaves old boundary root; native install failures surface |
| Builder slot replacement | `slot.setView(() => View)` → `OwnedBuilderRoot` | shared retained execution + slot boundary | Function argument and Tui-created shared runtime | Producer/evaluation/preparation failure restores old producer and old committed output |
| Animation install | `setAnimationWithRefs` → per-frame retained refs → scalar/buffer native setter | resolve refs to cloned Views → `HostViewSlot::replace_animation` | Frame count 1–4 uses scalar calls; >4 uses buffer | Empty list/type/range/native ref failure; no secondary transport |
| Cycle-boundary animation install | same TypeScript ref path | native `pending_frames` until frame-index wrap | Existing multi-frame animation with same interval | Deferred no-op until native wrap; invalid input fails immediately |
| Animation tick | no TypeScript call per tick | 16 ms mounted component tick checks configured interval | Component must be mounted; tick declaration active | Lock poisoning returns false; not due returns false; due invalidates component |
| Stop animation | materialize one View → `stopAnimationRef` | `HostViewSlot::stop_animation` delegates to `set_view` | Explicit caller stop | Resets frames/pending/index/clock; native failures surface |
| Create ScrollPane | same seed retained path as slot | `HostScrollPane::new` + `MountedScrollPane` | Tui factory | Empty/component-containing content can fail via Rust assertions or retained preparation |
| ScrollPane content replacement | boundary → `setContentRef` | `ScrollPane::set_content` | Direct View or builder output | Old content remains on prepare failure; scroll mode preserved |
| Scroll follow-end | direct N-API call | `ScrollPane::follow_end` and host invalidation | Explicit call | Host/control errors surface |
| Create TextInput | `Tui.createTextInput` | Host input state + mounted component registration | Host-bound only in TS | Invalid border/native creation fails before returned handle |
| TextInput edit | facade calls native method | native `TextInput` buffer + host invalidation | `setText`, `clear`, key, paste | Native closed/lock/input errors surface |
| TextInput submission | stable `Output<string>` facade + `Tui.route` | Rust `Output<String>` queue/router → host output queue | Route must be registered once | Route conflict/type mismatch/ownership error; unregistered output is dropped by router drain |
| History push | retained View ref → `pushRef` | native History clone + host sideband validation | Detached or attached History | Retained refusal, duplicate attachment, wrong host, or history state error |
| Tui root direct render | `render(Scene)` → `prepareRootPublication` | desired root and host frame barrier | Non-function scene | Previous visible frame remains authoritative on preparation failure |
| Tui root builder render | `render(() => Scene)` → root `OwnedBuilderRoot` | shared retained execution and desired root boundary | Function scene producer | Producer rollback preserves old output and sideband; frame failures leave desired retry state |

### 5.2 Legitimate alternate modes

The repository contains several genuine alternate routes, not one single slot mutation implementation:

1. **Direct value versus retained builder**
   - Direct View replacement is synchronous and caller-controlled.
   - Builder replacement creates retained execution state and automatic State subscriptions.

2. **Small animation scalar ABI versus variadic buffer ABI**
   - Both represent the same animation semantics.
   - Scalar functions avoid buffer setup for 1–4 frames.
   - Buffer path avoids a large JS number-array copy for larger animations.

3. **Immediate animation replacement versus cycle-boundary replacement**
   - Immediate replacement resets/selects the new animation now.
   - Cycle-boundary replacement stores pending frames and waits for native frame-index wrap.

4. **Direct boundary versus deferred H3 host boundary**
   - Slot/pane boundaries use an `installRef` callback and update the native control directly.
   - Tui root boundary uses `setDesiredViewRef`, then the environment/frame broker promotes desired to visible after a successful frame.

5. **Detached versus host-bound History**
   - Detached History supports local layout/push storage but cannot freeze/discard.
   - Host-bound History participates in scene validation, native scrollback, and frame invalidation.

6. **Native local editor state versus output event transport**
   - Text editing stays native.
   - Submission crosses as a typed output event and is reduced into a generic routed host output.

### 5.3 Explicit absence of old fallback routes

The retained materializer documentation states that the retained path is the single production structural architecture and that retained refusal fails explicitly rather than selecting a complete-object decoding transport (`retained-dag.ts:19-22`).

The Tui render documentation similarly says the pre-T13 recipe cascade (`render_ref`/scalar/path/structural/edit transaction) is gone; identity hints and derivation patches are now inside the retained materializer (`runtime.ts:373-389`).

Within the inspected TypeScript control paths:

- `ViewSlot`, `ScrollPane`, `History`, and animation all use retained NativeRef materialization;
- none catches retained refusal to select an older or parallel complete-object transport;
- expected NativeRef/status failures either become explicit refusal (`undefined`) or throw.

### 5.4 Failure semantics by phase

#### Preparation failure

Examples:

- unsupported semantic kind;
- stale NativeRef that cannot be recovered;
- duplicate state/content attachment;
- invalid embedded NUL/style/payload;
- missing native resource;
- cyclic semantic graph.

Preparation drains temporary leases and leaves the old root installed. Root publications expose `commit()` and `abort()`; abort releases all preparation-acquired state without mutating the installed root (`retained-dag.ts:1510-1529`, `1679-1702`).

#### Native install failure

Direct control boundaries invoke `installRef` during publication commit. A false return means failure; the root boundary unwinds newly acquired leases and retains the old boundary root (`retained-dag.ts:1945-2005`).

The comments explicitly distinguish this from an ordinary retained refusal: after successful preparation, a commit refusal indicates native runtime teardown or a pathological state and is surfaced loudly (`retained-dag.ts:1687-1695`).

#### Builder producer failure

`OwnedBuilderRoot.replaceProducer` restores the old producer and conditionally cancels the newly introduced retry obligation (`execution.ts:1229-1248`). Pre-existing State invalidation obligations are retained, so newer state writes are not lost.

#### Frame/presentation failure

Tui root desired structure is committed before visible frame promotion. The environment callback later invokes `commitVisibleAfterDrain`, which promotes the native boundary’s desired root and attachment bindings only after a successful host frame (`runtime.ts:770-782`).

The host stores:

- committed frame;
- candidate frame;
- presentation receipt;
- pending/candidate epochs;
- physical synchronization uncertainty.

(`application/host.rs:122-164`).

Thus an accepted desired control/root update can remain non-visible until the frame receipt succeeds.

#### Runtime teardown failure

Native closed handles fail through explicit liveness checks (`ION_DISPOSED_HANDLE` in N-API `tui.rs:275-283`). Tui close/exit aggregate cleanup errors rather than silently discarding them.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Retained NativeRef identity cache

TypeScript `retained-dag.ts` maintains:

- `SEMANTIC_NATIVE`: generation-scoped WeakMap semantic node → NativeRef hint;
- per-runtime reusable axis scratch arrays;
- grid word scratch;
- byte scratch;
- generation-scoped style-ref sidecar (`retained-dag.ts:52-104`).

Hints are acceleration metadata and never leases (`retained-dag.ts:7-11`, `52-59`). A borrowed hint is promoted to a lease before transient callers release it (`native-view-abi.ts:159-182`).

The native runtime has:

- weak semantic NodeId→View cache;
- NodeId→NativeRef map;
- paged NativeRef table;
- builders/edit transactions;
- styles/style atoms;
- bounded scavenging queue (`crates/iyon-tui-native/src/tui/view_abi.rs:357-397`).

NativeRefs are monotonic and not recycled inside one runtime generation. Empty pages can be freed while directory high-water remains (`view_abi.rs:133-142`).

### 6.2 Root boundary leases

A direct boundary tracks:

- `previousRef`;
- current desired/visible state in deferred mode;
- semantic nodes;
- superseded desired roots;
- structural revisions;
- `nativeLookupCeiling`.

(`retained-dag.ts:1572-1585`).

Direct slot/pane replacement:

- keeps old root leased while preparing;
- installs new root;
- releases old root only after successful installation;
- transfers exactly one root lease to the new root.

Deferred Tui root publication has separate desired and visible leases. Superseded desired revisions remain leased until visibility ordering proves they are no longer needed (`retained-dag.ts:1743-1789`, `2074-2118`).

### 6.3 Attachment binding cache

`AttachmentBindingState` tracks:

- desired resource leases;
- visible resource leases;
- superseded desired revisions;
- desired and visible revision keys (`attachments.ts:55-61`).

Desired attachment leases are committed alongside structural desired publication. Visible leases are promoted only after frame visibility. This parallels the desired/visible root distinction and prevents a newer control attachment from being associated with an older in-flight frame (`attachments.ts:63-134`).

ViewSlot and ScrollPane use this binding state for state/content attachments, even though animation frame arrays themselves use transient retained View materialization rather than a persistent attachment binding set.

### 6.4 Invalidation routes

#### Control mutation invalidation

`HostViewSlot`, `HostScrollPane`, and `HostTextInput` call `invalidate_host()`/`render_host()` after native state mutation:

```text
control mutation
  → component ID lookup
  → RunningApp::host_invalidate_component
  → ComponentRegistry::invalidate
  → SceneHost::invalidate_component
  → RunningApp::invalidate_frame
  → HostInner::advance_and_render
```

This invalidates one component and asks for a frame. SceneHost can perform an incremental component resolve when topology and geometry permit.

#### Tick invalidation

The mounted slot always declares a 16 ms tick. The scheduler only runs it while mounted. A due tick returns `true`, and SceneHost invalidates the changed component (`component/tick.rs:240-288`; `scene/host.rs:963-979`).

#### Builder State invalidation

TypeScript tracked State invalidates execution scopes:

- duplicate invalidations do not enqueue a scope twice;
- the queue is level-triggered;
- a microtask is scheduled when auto-flush is enabled;
- explicit flush consumes an existing scheduling token;
- failed batches restore retry obligations without auto-retrying persistent failures (`execution.ts:413-448`, `550-587`).

### 6.5 Animation scheduling cost

Animation work is divided by phase:

| Phase | Frequency | Work |
|---|---|---|
| Frame array installation | Per `setAnimation` call | Retained materialization of every caller View; one scalar/buffer N-API install |
| Native frame selection | Mounted tick cadence, nominally every 16 ms | Lock state, compare elapsed time, possibly advance one frame |
| Scene invalidation | Only when a frame is actually due | Mark one component invalid |
| Layout/paint | Per invalidating frame | SceneHost resolve/layout/paint, potentially incremental |
| TypeScript execution | Not per animation tick | No JS callback or per-tick ref transfer |
| Readback | On explicit test harness inspection | Flush and zero-time native advance before reading `frame` |

The fixed scheduler cadence may be more frequent than the animation interval, but the expensive structural frame transition occurs only when elapsed time reaches the animation interval.

### 6.6 NativeRef lease and memory behavior

Transient animation installation:

- acquires one temporary lease per frame;
- stores cloned Rust Views in `HostViewSlotState.frames`;
- releases all temporary NativeRefs afterward.

This is semantically safe because the native slot stores the View values. It also means a long animation retains a strong Rust `View` per frame even after its NativeRef lease is released. The native semantic cache may retain only weak metadata, but the control’s frame vector is a separate strong retention path.

Native release maintenance is bounded:

- `release_many` increments release-batch/released-ref counters;
- processes bounded scavenge candidates;
- queues zero-lease refs whose weak View remains live;
- runs full sweeps only after metadata-growth thresholds (`view_abi.rs:1183-1231`, `1275-1324`).

### 6.7 Observability counters

TypeScript retained counters include:

- retained hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast-path calls;
- ref words written;
- payload bytes;
- scratch reuse;
- stale ref retries;
- host mutations (`retained-dag.ts:106-148`).

Execution counters include:

- scope mounts/unmounts;
- body calls;
- prop skips;
- State invalidations;
- dirty enqueues/duplicates;
- no-op/changed outputs;
- flush passes;
- commit batches/aborts;
- exact View reuses/new Views (`execution.ts:254-301`).

Native runtime diagnostics expose:

- semantic cache entries/live count;
- NativeRef slots/pages;
- leased/unleased live slots;
- NodeId map entries;
- builders/edit transactions;
- style refs;
- scavenge counters;
- generation/alive state (`crates/iyon-tui-native/src/tui/view_abi.rs:1489-1515`, `1634-1669`).

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript behavioral tests

Relevant TypeScript tests found:

- `tui_harness.test.ts:29-38`
  - creates a ViewSlot;
  - installs two animation frames;
  - renders `slot.view()`;
  - advances deterministic time by 80 ms;
  - checks frame-one/frame-two screen output;
  - disposes slot and closes harness.
- `tui_realtime.test.ts:15-24`
  - checks that a real runtime drives slot ticks without manual clock advancement.
- `tui_runtime.test.ts:6-21`
  - creates a TextInput;
  - renders its component View;
  - injects text;
  - obtains stable submitted Output;
  - routes it;
  - submits Enter;
  - expects `{type: "output", routeId, payload}`.
- `tui_handles.test.ts:17-22`
  - verifies TextInput native state and Unicode cursor byte count.
- `tui_handles.test.ts:43-49`
  - creates a ViewSlot and checks repeated `slot.view()` values are distinct outside retained composition.
- `tui_h3_b_composition.test.ts:66-71`
  - checks component semantic identity uses local HandleId.
- `tui_h3_c_transport.test.ts:57-62`
  - checks retained component lowering resolves a HandleId to live native component state.
- `tui_retained_scene_regressions.test.ts:70-90`
  - applies a ViewState presentation update;
  - renders a slot;
  - replaces slot content with a state-attached View;
  - verifies replacement text and captured style survive.
- `tui_h3_a_semantic.test.ts:520-523`
  - samples slot semantic kinds as part of retained semantic transport coverage.

No test was run in this investigation.

### 7.2 Rust control tests

Relevant source-owned tests:

- `crates/iyon-tui/src/scroll.rs:246-290`
  - follow-end and detached scrolling;
  - content replacement while detached;
  - follow-end restoration;
  - resize window repair.
- `crates/iyon-tui/src/controls/text_input/tests/*`
  - buffer editing;
  - commands;
  - output;
  - presentation.
- `crates/iyon-tui/src/application/host.rs:2562+`
  - component slot replacement carrying captured state versions.
- `crates/iyon-tui/src/application/host.rs:2777+`
  - incremental resolution and paint behavior for one changed component among many.
- `crates/iyon-tui/src/component/tick_tests.rs`
  - tick activation/deactivation, due scheduling, and changed-component behavior.
- `crates/iyon-tui-native/src/tui.rs:1963+`
  - native TextInput Unicode state and disposal.
- `crates/iyon-tui-native/src/tui/view_abi.rs:4554+`
  - NativeRef table pages, removal, representation benchmark, and retained identity behavior.

### 7.3 Benchmarks and instrumentation

Relevant benchmark/instrumentation source:

- `retained-dag.ts:106-182`
  - structural counters and phase instrumentation hooks.
- `view_abi.rs:4675-4719+`
  - ignored NativeRef table representation benchmark.
- `crates/iyon-tui/src/perf.rs`
  - native/layout/paint counters used by SceneHost tests.
- `application/host.rs:2777+`
  - incremental resolve/paint counter assertions.

The instrumentation distinguishes:

- retained identity/caching work;
- NativeRef lease work;
- host mutations;
- component tick changes;
- incremental versus full scene resolution;
- frame/paint paths.

There is no direct per-animation-frame JS callback counter because animation ticks are intentionally native-owned.

### 7.4 Missing visibility

The current diagnostics do not directly expose all control-specific state:

- `ViewSlotState.frame_index`;
- animation frame vector length;
- pending cycle-boundary frame presence;
- `HostScrollPane` current scroll mode/top row;
- TextInput focus/scroll row;
- native output route registry contents.

Control `revision()` exposes a monotonic slot revision but not the reason for a revision change. Distinguishing direct replacement, immediate animation install, cycle-boundary promotion, and tick advancement requires screen output or source-level inference.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Native animation ownership is correctly below TypeScript

The framework boundary explicitly requires Rust to own stable slot identity, frame selection, scheduling, invalidation, and rendering; TypeScript should supply semantic frames only when application state changes. The source follows this contract:

- TypeScript materializes and installs the frame list once (`view-slot.ts:290-334`);
- Rust stores `Vec<View>`, selects frames, and ticks natively (`host.rs:211-219`, `386-418`);
- no TypeScript method is invoked for each tick.

This is a consequential architectural fact: moving animation scheduling into TypeScript would duplicate existing native scheduling and would violate the current ownership split.

### 8.2 Native control state and retained structural roots are parallel but coordinated

A slot has both:

1. a root boundary that retains the last statically installed semantic root;
2. native `HostViewSlotState` that may currently display an animation frame.

Direct `setView` goes through the boundary and native slot together. Animation setters update only native `HostViewSlotState` and do not call `RetainedRootBoundary.prepareInstall`.

Evidence:

- `setAnimationWithRefs` calls native animation setters and then disposes any builder root, but does not modify `currentView` or call `boundary.prepareInstall` (`view-slot.ts:284-334`);
- `stopAnimation` calls native `stopAnimationRef`, but does not update `currentView` or boundary state (`view-slot.ts:365-380`);
- native `HostViewSlot::set_animation` changes `state.view`, `frames`, and revision (`host.rs:338-372`).

This may be intentional: the outer semantic component occurrence remains stable, and SceneHost resolves the component ID to the current native slot state. Nevertheless, the boundary’s `previousRef` is not necessarily the View currently shown by an animation. A later direct replacement still uses the boundary’s last static root as its old-root lease, while native slot state may hold other frame Views. The source provides no explicit synchronization from animation state back into `currentView`/boundary. This is an important lifetime/ownership seam for future changes.

### 8.3 `currentView` is not a complete representation of ViewSlot state

`ViewSlot.currentView` is assigned:

- at construction (`view-slot.ts:127`);
- direct replacement commit (`view-slot.ts:220`);
- transactional publication commit (`view-slot.ts:268`).

It is not assigned by:

- `setAnimation`;
- `setAnimationAtCycleBoundary`;
- native animation ticks;
- `stopAnimation`.

Therefore `currentView` is better understood as the last direct/builder-authoritative root than as “the currently displayed frame.” The actual currently displayed frame belongs to Rust `HostViewSlotState.view`.

### 8.4 The TypeScript native contract contains a likely stale stop-animation member

`NativeViewSlotContract` declares both:

```ts
stopAnimation(view: object): void;
stopAnimationRef(viewRef: number): void;
```

(`packages/iyon-tui/src/transport/native/addon.ts:99-116`).

The actual native N-API implementation inspected in `crates/iyon-tui-native/src/tui.rs` exposes `stopAnimationRef` and a private Rust helper `stop_animation_view`, but no exported object-valued `stopAnimation(view: object)` method (`tui.rs:1837-1846`). TypeScript `ViewSlot` uses only `stopAnimationRef` (`view-slot.ts:365-377`).

Search scope for this finding:

- all relevant `NativeViewSlotContract` declarations;
- `packages/iyon-tui/src/api/controls/view-slot.ts`;
- `crates/iyon-tui-native/src/tui.rs` NativeViewSlot implementation.

This appears to be contract residue or an unimplemented private-contract member. It is not currently an active production route in the TypeScript facade.

### 8.5 Registration, mounting, and retirement are intentionally deferred

Control factory methods register native component instances before those components are necessarily present in the current semantic scene. Disposal therefore cannot always remove a component immediately:

- native control disposal requests retirement;
- `RunningApp` keeps the registry entry if the last successfully reconciled mount graph still contains it;
- physical removal occurs after a successful reconciliation proves unmounting.

This prevents a committed semantic root from dereferencing a prematurely deleted component.

### 8.6 Immediate native control invalidation differs from retained execution batching

A direct native control mutation often calls `advance_and_render()` synchronously from Rust (`host.rs:437-461`, `527-552`, `717-741`).

By contrast, a TypeScript retained builder State write:

- marks an execution scope dirty;
- queues a microtask;
- evaluates and stages publication;
- then the Tui/root/native host barrier decides when the frame becomes visible.

Both ultimately enter the same host frame pipeline, but they have distinct temporal semantics:

- native control methods can perform immediate host-side rendering;
- retained execution can batch multiple State changes before one publication;
- Tui `flush()` explicitly drains both retained execution and host visibility (`runtime.ts:417-421`).

### 8.7 Output is native-routed rather than a TypeScript callback stream

TextInput submission does not invoke a TypeScript callback directly from native input handling. The route is:

- Rust typed `Output<String>`;
- erased local output queue;
- output router;
- host `HostOutput::Routed`;
- N-API polling/wait;
- TypeScript `Tui.nextEvent`.

This preserves generic route IDs and keeps text editing/input handling native.

### 8.8 Generic semantic View values remain the native retained currency

Animation, slot replacement, pane content replacement, History push/freeze, and root scene publication all use the same retained semantic View→NativeRef path. The source repeatedly documents that retained refusal is explicit and that no secondary complete-object transport is selected.

This is the strongest cross-boundary coupling in this scope: controls are not independent payload systems; they are native holders/consumers of the generic retained View currency.

---

## 9. Open questions and coverage gaps

1. **Animated root/boundary alignment**
   - The source does not expose a direct query for the boundary’s current root versus the native slot’s current animated frame.
   - It is unclear whether this divergence is purely intentional because the component occurrence is stable, or whether future direct replacement/lease behavior must explicitly account for frames held only by `HostViewSlotState`.

2. **Animation frame reclamation timing**
   - Native animation replacement swaps the Rust `frames` vector, but the source does not expose frame-level retention diagnostics.
   - It is not possible from the public diagnostics to distinguish Views retained by:
     - a root boundary;
     - an animation frame vector;
     - the semantic weak cache;
     - another control or History entry.

3. **Cycle-boundary replacement with changed frame count**
   - Same-interval multi-frame replacement is deferred.
   - The source handles index modulo behavior on immediate replacement, but there is no separate public observable for whether a pending frame list exists before the next wrap.

4. **Tick scheduling after control creation but before mount**
   - Controls register a native component at factory creation, but tick activation is graph/mount dependent.
   - The source clearly indicates mount-based activation, but no public control API reports whether the slot is registered, mounted, or actively ticking.

5. **Error classification at N-API wrapper boundaries**
   - Some native errors are mapped to invalid-input errors and some to internal errors.
   - The exact mapping between all retained ABI refusal/status classes and public `TuiError` categories would require a broader generated ABI/error audit.

6. **Stale `stopAnimation(view)` contract member**
   - The TypeScript private native contract declares an object-valued member absent from the inspected exported N-API class.
   - It is unknown whether an external generated declaration or old loader depends on it; no such consumer was found in the inspected source.

7. **TextInput output route cleanup**
   - The output router has route registration and removal at the Rust generic layer, but the host-bound TypeScript Tui API exposes route registration without an explicit route-unregister operation.
   - Host teardown naturally removes the owning kernel, but finer-grained route lifetime behavior is outside this assignment’s complete scope.

8. **Detached History control identity**
   - Detached History can push retained Views before host attachment, while `freeze`/`discardLive` require attachment.
   - The exact state transition when a detached History is first attached through a Scene was inspected in TypeScript runtime code but not exhaustively traced through every native History ownership branch.

9. **Dynamic `ComponentCapabilities` re-resolution**
   - `MountedViewSlot` declares its fixed tick capability, while ScrollPane/TextInput declare interaction/layout capabilities.
   - Full capability re-resolution behavior after component replacement and retirement belongs partly to the broader input/runtime assignments.

10. **Executed validation**
    - No tests/builds/benchmarks were executed here, so no runtime claims about observed frame timing, memory counts, or route output beyond static source and test assertions are made.

---

## 10. Evidence appendix

### 10.1 Exact source paths and symbols inspected

#### TypeScript controls and public surfaces

- `packages/iyon-tui/src/api/controls/view-slot.ts`
  - `buildSlotHandle`
  - `ViewSlot`
  - `NativeViewSlotHandle`
  - `ViewSlot` constructor
  - `setView`
  - `setViewBuilder`
  - `setViewDirect`
  - `prepareSetView`
  - `setAnimation`
  - `setAnimationAtCycleBoundary`
  - `setAnimationWithRefs`
  - `animationScratch`
  - `setFixedAnimationRefs`
  - `stopAnimation`
  - `dispose`
  - `createViewSlot`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - `buildPaneHandle`
  - `ScrollPane`
  - `NativeScrollPaneHandle`
  - `NativeScrollPane` constructor
  - `setContent`
  - `prepareSetContent`
  - `setContentDirect`
  - `followEnd`
  - `dispose`
  - `createScrollPane`
- `packages/iyon-tui/src/api/controls/text-input.ts`
  - `TextInputOptions`
  - `TextInput`
  - `submitted`
  - `createNativeOutput`
  - `textInputForOutput`
  - `createTextInput`
- `packages/iyon-tui/src/api/controls/history.ts`
  - `History`
  - `HistoryLayout`
  - `push`
  - `freeze`
  - `discardLive`
  - `bindHistoryLifetime`
- `packages/iyon-tui/src/api/controls/output.ts`
  - `Output<T>`
- `packages/iyon-tui/src/api/controls/framework-handle.ts`
  - `FrameworkHandle`
  - `ComponentHandle`
  - `dispose`
  - `ensureOpen`
  - `call`

#### TypeScript runtime/composition/transport

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`
  - `Tui`
  - Tui constructor
  - `prepareRootPublication`
  - `render`
  - `renderCanonical`
  - `renderDirect`
  - `flush`
  - `prepareMutation`
  - `createHistory`
  - `createTextInput`
  - `createViewSlot`
  - `createScrollPane`
  - `route`
  - `interceptPaste`
  - `resize`
  - `commitVisibleAfterDrain`
  - `disposeRetainedExecution`
  - `close`
  - `exit`
- `packages/iyon-tui/src/runtime/attachments.ts`
  - `AttachmentBindingState`
  - `commitDesired`
  - `commitVisible`
  - `dispose`
  - `prepareAttachmentsForView`
  - `prepareSemanticAttachments`
- `packages/iyon-tui/src/runtime/handle-registry.ts`
  - `registerFrameworkHandle`
  - `disposeFrameworkResource`
  - `releaseFrameworkHandle`
- `packages/iyon-tui/src/transport/native/resources.ts`
  - `registerNativeResource`
  - `nativeResourceForHandleId`
  - `nativeResourceOf`
  - `releaseNativeResource`
  - `disposeNativeResource`
- `packages/iyon-tui/src/transport/native/addon.ts`
  - Native control/host contracts
  - `NativeViewSlotContract`
  - `NativeScrollPaneContract`
  - `NativeTextInputContract`
  - `NativeTuiHostContract`
  - diagnostics contracts
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `nativeViewAbiSession`
  - `tryRetainedMaterializeRef`
  - `tryRetainedAxisCreate`
  - retained update helpers
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `SemanticNativeHint`
  - `MaterializeTx`
  - `RetainedIdentityCounters`
  - `ensureSemanticNative`
  - `materializeComponentNode`
  - `RetainedRootBoundary`
  - `RootPublication`
  - `prepareInstall`
  - `prepareDesiredInstall`
  - `commitVisible`
  - `prepareFrom`
  - `publishPrepared`
  - `publishDesiredPrepared`
  - `unwindPrepared`
  - `renderExact`
  - `close`
  - `transferRoot`
- `packages/iyon-tui/src/composition/compose.ts`
  - `composeComponent`
  - `composeContent`
  - `composeState`
  - slot reuse/fresh staging
- `packages/iyon-tui/src/composition/execution.ts`
  - `RetainedExecutionScope`
  - `RetainedExecutionRuntime`
  - `mountRoot`
  - `invalidate`
  - `flush`
  - `evaluateIntoPendings`
  - `stagePublicationsRecursive`
  - `commitBatch`
  - `commitScope`
  - `OwnedBuilderRoot`
  - `replaceProducer`
  - `dispose`
- `packages/iyon-tui/src/api/view/view.ts`
  - `componentViewForHandle`
  - `attachStateForComposition`
- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - semantic node identity and attachment references
- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness.open`
  - `render`
  - `pressKey`
  - `paste`
  - `advance`
  - `inspect`

#### Rust retained controls and host/application

- `crates/iyon-tui/src/application/host.rs`
  - `HostViewSlot`
  - `HostScrollPane`
  - `HostTextInput`
  - `ViewSlotState`
  - `HostViewSlot::new`
  - `set_view`
  - `set_animation`
  - `set_animation_at_cycle_boundary`
  - `replace_animation`
  - `stop_animation`
  - `tick`
  - `invalidate_host`
  - `MountedViewSlot`
  - `MountedScrollPane`
  - `MountedTextInput`
  - `TuiHost::create_text_input`
  - `TuiHost::create_view_slot`
  - `TuiHost::create_scroll_pane`
  - `route_text_input`
  - `route_text_input_output`
  - `next_output`
  - `wait_for_output`
  - `advance_time`
  - `exit`
- `crates/iyon-tui/src/application/kernel.rs`
  - `RunningApp::host_register`
  - `host_route`
  - `host_intercept_paste`
  - `host_invalidate_component`
  - `host_exit`
  - `advance_ready`
  - `prepare_frame_with_states`
  - `reap_retired_components`
  - `drain_outputs_to_actions`
- `crates/iyon-tui/src/scene/host.rs`
  - `SceneHost::invalidate_component`
  - `SceneHost::tick_due`
  - `SceneHost::drain_outputs`
  - `SceneHost::render_at_with_states`
  - mount graph and capability synchronization
- `crates/iyon-tui/src/component/tick.rs`
  - `TickScheduler`
  - `sync_mounts`
  - `sync_capabilities`
  - `sync_component_capability`
  - `tick_due_with_events`
- `crates/iyon-tui/src/controls/text_input/mod.rs`
  - `TextInput`
  - `new`
  - `set_text`
  - `clear`
  - `submitted`
  - `handle_command`
  - `handle_paste`
  - `Component` implementation
- `crates/iyon-tui/src/controls/text_input/output.rs`
  - `TextChange`
  - `ChangeOutputs`
  - `TypedProjector`
- `crates/iyon-tui/src/scroll.rs`
  - `ScrollPane`
  - `ScrollMode`
  - `set_content`
  - `follow_end`
  - `view`
  - `handle_command`
- `crates/iyon-tui/src/output/event.rs`
  - `OutputQueue`
  - `EventCx`
- `crates/iyon-tui/src/output/handle.rs`
  - `Output<T>`
  - `OutputId`
- `crates/iyon-tui/src/output/router.rs`
  - `OutputRouter`
  - `route`
  - `drain`
  - `RouteConflict`
  - `OutputDispatchError`

#### Native N-API and retained runtime

- `crates/iyon-tui-native/src/tui.rs`
  - `NativeTuiOutput`
  - `resolve_native_view`
  - `NativeTextInput`
  - `NativeTuiHost`
  - `NativeViewSlot`
  - `NativeScrollPane`
  - `set_view_ref`
  - animation ref methods
  - `stop_animation_ref`
  - `next_output`
  - `wait_for_output`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - `NativeViewSlot`
  - `NativeRefTable`
  - `NativeViewRuntime`
  - `resolve_ref`
  - `publish_semantic_view`
  - `ref_for_node_id`
  - `release_many`
  - `maintain_bounded`
  - `prune_expired`
  - `view_for_ref`
  - `view_render_ref_impl`
  - `host_render_ref_impl`
  - diagnostics and runtime handles

### 10.2 Relevant tests indexed

- `packages/iyon-tui/tests/tui_harness.test.ts`
  - native TextInput submission route
  - generic ViewSlot animation
- `packages/iyon-tui/tests/tui_runtime.test.ts`
  - native local editing and submission output
- `packages/iyon-tui/tests/tui_realtime.test.ts`
  - real-time slot tick
- `packages/iyon-tui/tests/tui_handles.test.ts`
  - TextInput native state
  - ViewSlot occurrence identity
- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
  - retained semantic kind coverage
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
  - component HandleId identity
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
  - retained component lowering
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
  - ViewSlot replacement with ViewState styling
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
  - generated scalar retained route
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
  - retained transaction behavior
- `packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts`
  - semantic/native cache ownership
- `crates/iyon-tui/src/scroll.rs` tests
  - content replacement, detached/follow-end, resize
- `crates/iyon-tui/src/component/tick_tests.rs`
  - scheduler lifecycle
- `crates/iyon-tui/src/controls/text_input/tests/*`
  - editor, command, output, presentation contracts
- `crates/iyon-tui/src/application/host.rs` tests
  - slot replacement captured state
  - incremental changed-component rendering
- `crates/iyon-tui-native/src/tui.rs` tests
  - native TextInput lifecycle
- `crates/iyon-tui-native/src/tui/view_abi.rs` tests
  - NativeRef table and retained runtime behavior

### 10.3 Files indexed but not comprehensively read

The repository-wide tracked manifest includes many additional Rust, TypeScript, generated, test, fixture, benchmark, and documentation files. For this assignment, the following were indexed or searched for references but not read comprehensively:

- unrelated Rust content, projection, theme, layout, terminal, and History modules;
- generated ABI bodies other than the declarations/paths needed to establish the NativeRef calls;
- unrelated TypeScript content and presentation APIs;
- the complete TypeScript test suite;
- all benchmark suites outside retained identity, host/component incremental work, and NativeRef table representation.

### 10.4 Read-only investigation method

The investigation used repository file listing and content search only. No source/configuration files were edited. No dependencies were installed. No services, agents, builds, tests, or benchmarks were started.