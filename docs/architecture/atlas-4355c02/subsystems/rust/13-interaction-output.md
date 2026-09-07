# 13 — Interaction and typed output

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: `crates/iyon-tui/src/interaction/` and `crates/iyon-tui/src/output/`, recursively.
- Supporting seams inspected where necessary:
  - application input and action dispatch;
  - `SceneHost`;
  - component registry and tick scheduler;
  - `TextInput` output registration;
  - terminal input decoding;
  - native host key/paste dispatch;
  - crate binding exports and identity allocation.

The report follows the current-source-first rule. Historical and V5 documents were used only for context and evidence expectations; no V5 migration or disposition analysis is performed here.

### Current source conclusions

The assigned subsystem consists of two tightly connected but conceptually distinct mechanisms:

1. **Interaction capabilities and routing**
   - Component-declared focusability, modal scope, focus-transition callbacks, key-command mappings, paste handlers, ticks, and layout notifications.
   - Host-owned focus state and modal restoration.
   - Focus/ancestor routing for key and paste events.
   - Framework-level fallback for unmodified `Tab`.

2. **Typed semantic output**
   - Caller-created, opaque `Output<T>` channel identities.
   - Borrow-scoped `EventCx` emission.
   - A host-owned FIFO queue that erases payloads only internally.
   - Application-owned `OutputRouter<Action>` mappings.
   - Deferred conversion of component outputs into application actions.

The interaction module is generic framework machinery. It does not know about agent, assistant, transcript, provider, tool, or other product concepts. The output mechanism similarly transports caller-defined payload types and maps them into caller-defined application actions.

### Evidence status

This was a static read-only investigation. I did not modify repository files, install dependencies, run tests, run benchmarks, or execute a build. Therefore:

- All behavioral claims below are derived from source and existing tests.
- Existing tests are cited as behavioral evidence, but are not claimed to have passed in this run.
- No runtime counters or execution traces were observed.

### Important boundary observation

`crates/iyon-tui/src/lib.rs:1-11, 112-117` explicitly describes the Rust crate as an implementation crate for the TypeScript facade and in-tree native addon. The interaction and output modules are `pub(crate)` modules at `lib.rs:21-22`; their items are re-exported to the native crate through the deliberately narrow `binding` module.

The generic Rust APIs therefore have two relevant surfaces:

- **Internal framework surface:** `ComponentCx`, `EventCx`, `OutputRouter`, `InteractionResult`, `FocusState`, `MountedCapabilities`, and the private output queue.
- **Native binding surface:** selected key, modifier, and output types exposed through `crates/iyon-tui/src/binding/mod.rs:64-84`.

The Rust crate is not an intended external Rust UI-authoring API.

---

## 1. Responsibility and structure

### 1.1 File inventory and physical LOC

Physical line counts below are based on the final line numbers visible in the source files, including blank lines, comments, and test code.

| Area | File | Approx. physical LOC | Primary responsibility |
|---|---|---:|---|
| Interaction | `interaction/mod.rs` | 18 | Module organization and visibility |
| Interaction | `interaction/command.rs` | 223 | Component capability declarations, type-erased handlers, mounted capability storage |
| Interaction | `interaction/focus.rs` | 335 | Focus state, modal scope, restoration, geometry-aware eligibility |
| Interaction | `interaction/key.rs` | 144 | Backend-neutral key and modifier vocabulary |
| Interaction | `interaction/result.rs` | 6 | `Ignored`/`Consumed` interaction result |
| Interaction | `interaction/routing.rs` | 110 | Focus-chain key/paste routing |
| Interaction tests | `interaction/tests/mod.rs` | 927 | Focus, modal, routing, tick, erased-handler, and modifier contracts |
| **Interaction subtotal** | production | **836** |  |
| **Interaction subtotal** | tests | **927** |  |
| Output | `output/mod.rs` | 18 | Module organization and visibility |
| Output | `output/event.rs` | 68 | `EventCx`, erased queue entries, FIFO queue |
| Output | `output/handle.rs` | 68 | Opaque `Output<T>` and globally allocated `OutputId` |
| Output | `output/router.rs` | 101 | Route registration, conflict detection, dispatch |
| Output tests | `output/tests/mod.rs` | 3 | Test module declarations |
| Output tests | `output/tests/event.rs` | 103 | Queue ownership and emission timing |
| Output tests | `output/tests/handle.rs` | 64 | Identity and trait guarantees |
| Output tests | `output/tests/router.rs` | 135 | FIFO routing, conflicts, removal, and mismatch handling |
| **Output subtotal** | production | **255** |  |
| **Output subtotal** | tests | **305** |  |
| **Combined scope** | production | **1,091** |  |
| **Combined scope** | tests | **1,232** |  |
| **Combined scope** | all files | **2,323** |  |

There are no generated files in the assigned directories.

### 1.2 Interaction structure

`interaction/mod.rs:1-18` exposes:

- `ComponentCx` publicly within the crate/native binding boundary;
- `Key`, `KeyStroke`, `MediaKey`, `ModifierKey`, and `Modifiers`;
- `InteractionResult`.

The following remain crate-private:

- `ComponentCapabilities`;
- `MountedCapabilities`;
- `FocusState`;
- `route_key_local`;
- `route_paste`;
- `route_paste_interceptor`.

`interaction/command.rs` is the capability declaration layer. It defines:

- `KeyCommandCapability`;
- `TickCapability`;
- `ComponentCapabilities`;
- `ComponentCx<'a, C>`;
- `MountedCapabilities`.

`interaction/focus.rs` is intentionally host-owned. Components only declare capabilities and receive focus-change callbacks; components do not own global focus or modal state.

`interaction/routing.rs` is a small policy layer over:

- a `FocusState`;
- a committed `MountGraph`;
- committed `MountedCapabilities`;
- the sole `ComponentRegistry`;
- the host output queue.

### 1.3 Output structure

`output/mod.rs:1-18` identifies the output subsystem as a backend-neutral boundary between component event emission and application action mapping.

`output/event.rs` owns:

- `ErasedOutputEvent`;
- `OutputQueue`;
- `EventCx<'a>`.

`output/handle.rs` owns:

- `OutputId`;
- the typed but zero-storage `Output<T>` handle;
- global output ID allocation.

`output/router.rs` owns:

- `Route<A>`;
- `RouteConflict`;
- `OutputDispatchError`;
- `OutputRouter<A>`.

The queue is not public. `EventCx` is public within the crate/native binding boundary, while `Output<T>` and `OutputRouter<A>` are re-exported through `binding`.

### 1.4 Primary and secondary responsibilities

#### Interaction primary responsibilities

- Normalize backend key events into an application-independent value type.
- Let components declare local interaction participation.
- Retain mounted capability snapshots independently of component code execution.
- Maintain focus and modal scope.
- Route key and paste events from focused component toward ancestors.
- Support typed component-local commands.
- Integrate component ticks with the same output queue as input events.

#### Interaction secondary responsibilities

- Trigger focus callbacks.
- Retain the previous focus handler long enough to blur a component removed from the latest capability snapshot.
- Filter focus eligibility by committed geometry visibility.
- Support incremental reconciliation after topology-preserving updates.
- Support host-only layout/content extent notifications.

#### Output primary responsibilities

- Give each output channel a stable opaque identity.
- Preserve payload type at the producer API.
- Permit non-`Clone`, non-`Send`, and non-`Sync` payloads as long as they are `'static`.
- Preserve event order across heterogeneous output types.
- Map output payloads to application actions exactly once per registered route.

#### Output secondary responsibilities

- Detect duplicate route registration.
- Detect internal payload type corruption.
- Discard events for currently unrouted channels.
- Provide deferred delivery semantics: emitting only queues; routing occurs later.

---

## 2. Types, APIs and contracts

### 2.1 Key and modifier vocabulary

`interaction/key.rs:3-34` defines the non-exhaustive `Key` enum. It covers:

- character keys;
- navigation and editing keys;
- function keys;
- lock and system keys;
- media keys;
- physical modifier keys.

`MediaKey` and `ModifierKey` are also non-exhaustive enums at `key.rs:35-70`.

`Modifiers` is a six-bit value type (`key.rs:72-112`):

- `SHIFT`;
- `CONTROL`;
- `ALT`;
- `SUPER`;
- `HYPER`;
- `META`.

It supports:

- `contains`;
- `union`;
- `BitOr`;
- `BitOrAssign`.

`KeyStroke` combines a `Key` and `Modifiers` (`key.rs:114-144`). It is `Copy`, `Eq`, `Hash`, and has no backend state. The constructor contract is explicit:

- `KeyStroke::new(key)` creates an unmodified stroke;
- `KeyStroke::with_modifiers(key, modifiers)` preserves the exact modifier value.

This is a normalized event identity, not a live keyboard-state object. The test `interaction/tests/mod.rs:921-926` explicitly protects the value-type behavior.

### 2.2 Interaction result

`interaction/result.rs:1-6` defines:

```rust
pub enum InteractionResult {
    Ignored,
    Consumed,
}
```

This is the only result used by local key and paste handlers. `Ignored` permits routing to an ancestor or later application-global route. `Consumed` stops the corresponding local routing pass.

A handler may emit output and still return `Ignored`. The interaction test at `interaction/tests/mod.rs:332-361` verifies that a child can emit before an ancestor consumes the same key.

### 2.3 Component capabilities

`interaction/command.rs:37-47` defines the capability snapshot:

- `focusable: bool`;
- `modal_scope: bool`;
- optional focus callback;
- optional paste callback;
- ordered vector of local key-command capabilities;
- optional tick capability;
- optional layout-size callback;
- optional content-extent callback.

The fields are not component-owned mutable runtime state. They are collected from a component and copied into a capability snapshot.

`ComponentCx<'a, C>` (`command.rs:67-79`) is an ephemeral declaration context borrowing a mutable `ComponentCapabilities`. It carries `PhantomData<fn(&'a C)>` so the declaration context is associated with the component type without storing a component reference.

The public declaration methods are:

- `focusable` (`command.rs:81-84`);
- `modal_scope` (`command.rs:86-89`);
- `on_focus_changed` (`command.rs:91-102`);
- `on_paste` (`command.rs:104-121`);
- `key_commands` (`command.rs:150-175`);
- `tick` (`command.rs:177-198`).

The layout and content extent callbacks are crate-private (`command.rs:123-148`), indicating that they are framework synchronization hooks rather than general application authoring APIs.

### 2.4 Type erasure and handler contracts

Handlers are erased into `dyn Any` closures:

```rust
type MapCommand = dyn Fn(&dyn Any, KeyStroke) -> Option<Box<dyn Any>>;
type HandleCommand =
    dyn for<'a> Fn(&mut dyn Any, Box<dyn Any>, &mut EventCx<'a>) -> InteractionResult;
type FocusChanged = dyn Fn(&mut dyn Any, bool);
type PasteHandler = dyn for<'paste, 'event> Fn(
    &mut dyn Any,
    &'paste str,
    &mut EventCx<'event>,
) -> InteractionResult;
type TickHandler = dyn for<'a> Fn(&mut dyn Any, Instant, &mut EventCx<'a>) -> bool;
```

The registration methods require `C: 'static`. The function pointers themselves are stored behind `Arc`, but the public methods accept function pointers rather than arbitrary captured closures.

Downcasts fail loudly with `expect` messages such as:

- `"component focus handler type mismatch"`;
- `"component paste handler type mismatch"`;
- `"component key mapping type mismatch"`;
- `"component key command type mismatch"`;
- `"component tick type mismatch"`.

This is an internal invariant boundary, not a user-facing recoverable error path. `interaction/tests/mod.rs:902-919` explicitly verifies loud failure for erased key and focus handler mismatches.

### 2.5 Ordered local command routing

`ComponentCx::key_commands` accepts:

```rust
map: fn(&C, KeyStroke) -> Option<Command>
handle: for<'event> fn(
    &mut C,
    Command,
    &mut EventCx<'event>,
) -> InteractionResult
```

The command type is generic but erased internally into `Box<dyn Any>`. Multiple registrations are appended to `ComponentCapabilities.key_commands` (`command.rs:158-174`), so registration order is semantically meaningful.

Routing (`routing.rs:60-75`) iterates:

1. components in the focus/ancestor chain;
2. each component’s key-command registrations in declaration order;
3. the mapper first;
4. the handler only if the mapper returns a command.

A component can therefore have multiple independent command domains. The first handler returning `Consumed` ends routing. A mapped command returning `Ignored` permits later commands and ancestors to participate.

### 2.6 Focus state contracts

`FocusState` (`focus.rs:9-16`) stores:

- focused component ID;
- the currently retained focus callback;
- active modal component ID;
- modal restoration frames;
- optional cloned component geometry.

The focused callback is stored independently of the currently queried capability map. This is consequential when a focused component disappears from a replacement scene: the old callback can still be used to deliver blur before the component is reclaimed.

The tests at `interaction/tests/mod.rs:423-467` and `469-514` cover:

- blur of a removed focused component;
- blur when a focused component loses focusability;
- use of the previous capability callback during that transition.

### 2.7 Mounted capabilities

`MountedCapabilities` (`command.rs:201-223`) is a `HashMap<ComponentId, ComponentCapabilities>`. It provides:

- insertion/replacement by component ID;
- lookup;
- filtering of mount order to modal IDs.

It is a committed host-side index. `SceneHost` imports and owns it (`scene/host.rs:179-187`), alongside the committed graph, mounted components, focus state, tick scheduler, and output queue.

### 2.8 Typed outputs

`Output<T>` (`output/handle.rs:16-62`) contains only:

- an opaque `OutputId`;
- `PhantomData<fn() -> T>`.

The `fn() -> T` phantom form avoids imposing ownership, variance, or trait requirements from the payload type. Consequently:

- `Output<T>` is `Copy`;
- `Output<T>` is `Clone`;
- `Output<T>` is `Eq`;
- `Output<T>` is `Hash`;
- `Output<T>` is `Debug`;
- none of these require `T: Clone`, `T: Send`, or `T: Sync`.

The output identity is independent of the payload type. Two `Output<usize>` values are distinct channels; copying one preserves its identity.

### 2.9 Event context and queue

`EventCx<'a>` (`output/event.rs:60-68`) holds a mutable borrow of the host-owned `OutputQueue`.

Its only public operation is:

```rust
pub fn emit<T: 'static>(&mut self, output: Output<T>, value: T)
```

Emission immediately allocates an internal `ErasedOutputEvent`, but does not invoke application route closures.

`ErasedOutputEvent` (`event.rs:8-12`) stores:

- `OutputId`;
- `TypeId` of the payload;
- `Box<dyn Any>` payload.

The queue is a `VecDeque`, so insertion is FIFO.

The `'static` payload requirement means a producer cannot emit a borrowed payload. This is necessary because the queue may outlive the callback invocation. Borrowed data can still be used transiently inside a projector to create an owned result; `TextInput` does exactly this for `TextChange<'a>`.

### 2.10 Output router

`OutputRouter<A>` (`output/router.rs:46-100`) maps `OutputId` to an erased `Route<A>` containing:

- expected payload `TypeId`;
- an owned `'static` mapping closure.

Public methods:

- `new`;
- `route<T>`;
- `remove<T>`.

`route` rejects a duplicate channel with `RouteConflict` (`router.rs:58-78`). The first route remains intact when a second route is rejected; this is protected by `output/tests/router.rs:41-58`.

`remove` is identity-based and does not require the output payload to be routed or present in the queue (`router.rs:80-82`).

Dispatch is crate-private (`router.rs:84-100`) and is invoked by `SceneHost::drain_outputs`.

### 2.11 Output dispatch failure contract

`OutputDispatchError` currently has one variant:

```rust
TypeMismatch
```

`OutputRouter::drain` compares the route’s expected `TypeId` with the queue entry’s payload `TypeId` (`router.rs:92-94`). If they differ, it returns `Err(TypeMismatch)`.

The public producer path makes this mismatch unrepresentable under normal use. The mismatch test uses the crate-private `push_mismatched_for_test` helper to forge corruption (`output/tests/router.rs:99-111`).

The mismatch is intentionally not silently ignored. This conforms to the repository’s internal-invariant policy.

---

## 3. Dependency and ownership map

### 3.1 High-level ownership graph

```text
terminal backend / native host
        │
        │ normalized KeyStroke or paste text
        ▼
TuiHost / application driver
        │
        ▼
RunningApp
        │ owns
        ├── ComponentRegistry
        ├── SceneHost
        │     ├── committed MountGraph
        │     ├── MountedCapabilities
        │     ├── FocusState
        │     ├── TickScheduler
        │     └── OutputQueue
        ├── OutputRouter<Action>
        ├── GlobalBindings<Action>
        └── PasteInterceptors<Action>
                 │
                 ▼
          application actions
```

### 3.2 Component lifecycle and capability flow

```text
ComponentRegistry::register(C)
        │ creates
        ▼
ComponentHandle<C> + ComponentId
        │
        ├── Component::view()
        └── Component::capabilities(ComponentCx)
                         │
                         ▼
                 ComponentCapabilities
                         │
                         ▼
                  ResolvedScene capabilities
                         │
                         ▼
                  SceneHost committed indexes
                         ├── MountGraph
                         ├── MountedCapabilities
                         └── TickScheduler
```

The `ComponentRegistry` is the sole owner of component instances (`component/registry.rs:65-69`). Capability snapshots contain type-erased function pointers and `Arc` wrappers, not references into component instances.

### 3.3 Output ownership graph

```text
component state
    │ stores copied Output<T> identity
    ▼
Component handler / tick / paste callback
    │ receives &mut EventCx
    ▼
SceneHost::outputs: OutputQueue
    │ drained by
    ▼
SceneHost::drain_outputs(&OutputRouter<Action>)
    │
    ▼
RunningApp::actions: VecDeque<Action>
    │ consumed by
    ▼
RunningApp::update(state, action, AppCx)
```

A component does not own the output queue. A component only owns channel handles and, for `TextInput`, its output projector registrations.

An output route is owned by `RunningApp.outputs`, not by the component that created the output channel. There is no generic reverse map from `OutputId` to component ID.

### 3.4 Focus ownership

```text
SceneHost
 ├── FocusState
 │    ├── focused ComponentId
 │    ├── retained focus callback
 │    ├── active modal ComponentId
 │    ├── modal restoration stack
 │    └── optional geometry clone
 ├── MountGraph
 ├── MountedCapabilities
 └── ComponentRegistry
```

`FocusState` has no independent component lifetime. IDs are valid only while the corresponding registry entry and committed mount graph make them valid. The retained focus callback is specifically used to bridge a replacement/removal transition.

### 3.5 Creation and destruction

#### Components

- Created by `ComponentRegistry::register` (`component/registry.rs:84-98`).
- Held in a `Box<dyn ErasedComponent>`.
- Unmounted when the committed scene graph changes.
- Physically removed either by direct `AppCx::remove_component` or by deferred retirement in the native host/kernel path.
- Deferred retirement is necessary because a failed candidate frame must not destroy components still referenced by the last committed graph (`application/kernel.rs:113-142`).

#### Capabilities

- Recomputed by `ComponentRegistry::resolution` when the component revision changes (`component/registry.rs:157-175`).
- Stored in scene resolution and then copied into `SceneHost.capabilities`.
- Tick registrations are rebuilt or updated from the committed capability set.
- Stale tick registrations are removed when a mounted component no longer declares a tick (`component/tick.rs:171-179`).

#### Output channels

- Created by `Output::new`, using the process-wide `NEXT_OUTPUT_ID` (`output/handle.rs:5-13, 22-28`).
- Never explicitly destroyed or recycled.
- Channel identity can remain copied in application state after a component is gone.
- Stale handles do not match freshly allocated channels because IDs are monotonically allocated.

#### Output routes

- Created by `AppCx::route` (`application/context.rs:121-127`) or host-specific wrappers.
- Owned by `RunningApp.outputs`.
- Removed explicitly by `AppCx::remove_route`.
- Not automatically removed when a component is unmounted or removed.
- The route closure is `'static`; it may retain caller-owned state for the lifetime of the route.

#### Paste interceptors

- Created in `AppCx::intercept_paste` (`application/context.rs:183-195`).
- Indexed by `ComponentId`.
- Removed by explicit API, `AppCx::remove_component`, or deferred raw-ID retirement (`application/input.rs:54-65`, `application/kernel.rs:133-141`).
- An interceptor can remain registered while its component is unmounted, but it is unreachable because routing walks only the committed focus/modal chain.

### 3.6 Framework dependencies

| Subsystem | Depends on | Major reverse dependencies |
|---|---|---|
| `interaction/key.rs` | standard library only | terminal key adapters, native binding, all command mappers |
| `interaction/command.rs` | `ComponentId`, `Size`, `EventCx`, `KeyStroke` | component registry, scene resolution, tick scheduler, routing |
| `interaction/focus.rs` | `ComponentId`, `MountGraph`, `ComponentRegistry`, geometry map | `SceneHost`, key routing |
| `interaction/routing.rs` | focus, graph, capabilities, registry, output queue | `SceneHost`, application kernel |
| `output/handle.rs` | ID allocator | components, application context, native host |
| `output/event.rs` | `Output<T>` | all component interaction callbacks and ticks |
| `output/router.rs` | queue and `OutputId` | `RunningApp`, `SceneHost`, native host wrappers |
| `SceneHost` | all interaction/output internals | application kernel and native host |
| terminal crossterm adapter | `KeyStroke`, terminal event types | `TermwizBackend`, native runtime driver |
| native host parser | binding key types | N-API `dispatchKey` and `bindKey` |

---

## 4. Execution paths and state transitions

### 4.1 Native terminal key path

The production terminal path is:

```text
crossterm reader thread
    ↓ raw crossterm::event::Event
unbounded event channel
    ↓
TermwizBackend::try_next_event
    ↓
crossterm::map_event
    ├── key::key_stroke
    ├── TerminalEvent::Paste
    └── TerminalEvent::Resize
    ↓
TuiHost::poll_terminal
    ↓
RunningApp::dispatch_key / dispatch_paste
    ↓
SceneHost::dispatch_key_local / dispatch_paste
    ↓
interaction routing
    ↓
OutputQueue
    ↓
OutputRouter<Action>
    ↓
application action queue
```

Evidence:

- `terminal/crossterm/mod.rs:21-27` maps crossterm events.
- `terminal/crossterm/key.rs:7-45` normalizes key events.
- `terminal/termwiz/backend.rs:109-124` polls and converts queued events.
- `application/host.rs:1482-1507` dispatches terminal events and stops input pumping once an action becomes pending.
- `application/kernel.rs:523-546` performs key dispatch, drains outputs, and applies global bindings.
- `application/kernel.rs:549-571` performs paste dispatch and drains outputs.

### 4.2 Native-addon key path

The native host path is separate from crossterm:

```text
TypeScript/native caller
    ↓ key string + modifier strings
iyon-tui-native::parse_key
    ↓ KeyStroke
TuiHost::dispatch_key
    ↓
RunningApp::dispatch_key
    ↓
same SceneHost interaction and output path
```

Evidence:

- `crates/iyon-tui-native/src/tui.rs:941-946` exposes `dispatchKey`.
- `tui.rs:1043-1086` validates known names, single-character keys, and modifier strings.
- `application/host.rs:1364-1371` dispatches the normalized stroke.

The native parser accepts fewer variants than the Rust `Key` vocabulary:

- named navigation/editing keys;
- one-character fallback;
- `shift`, `control`/`ctrl`, `alt`/`option`, `super`/`meta`.

It does not expose all `MediaKey`, physical `ModifierKey`, `HYPER`, or `META` forms accepted by the Rust core key vocabulary through this string parser. This appears to be an intentional native façade limitation, but it is a cross-boundary asymmetry worth preserving in any API inventory.

### 4.3 Key routing state transition

`RunningApp::dispatch_key` (`application/kernel.rs:523-546`) performs:

1. Ignore the key if application exit has been requested.
2. Record previous focused component.
3. Call `SceneHost::dispatch_key_local`.
4. Record next focused component.
5. Drain component output events into application actions.
6. If local routing returned `Ignored`, consult the exact `GlobalBindings` map.
7. If a global binding exists, enqueue its action and return `Consumed`.
8. If local routing returned `Consumed`, invalidate old/new interaction components and mark the application dirty.
9. Return the local result.

`SceneHost::dispatch_key_local` (`scene/host.rs:918-930`) delegates to `route_key_local`.

`route_key_local` (`interaction/routing.rs:45-85`) performs:

1. Build the focused-to-ancestor routing chain.
2. For every component in the chain:
   - look up mounted capabilities;
   - iterate ordered key-command registrations;
   - run the mapper against immutable component state;
   - if a command is mapped, run its mutable handler;
   - stop on `Consumed`.
3. If still ignored and the key is unmodified `Tab`, call `FocusState::focus_next`.
4. Return `Consumed` only if a handler or focus transition consumed the event.

Important consequences:

- Ancestors can handle an ignored child key.
- Siblings are never considered.
- Background components outside the focus/modal chain are never considered.
- The framework handles forward `Tab` only when modifiers are exactly `Modifiers::NONE`.
- Shift-Tab is not a built-in reverse-focus route in `route_key_local`; it can be handled by a component command or application-global binding.

### 4.4 Paste routing state transition

There are two paste paths.

#### Interceptor path

`RunningApp::dispatch_paste` first invokes `SceneHost::intercept_paste` (`application/kernel.rs:553-561`).

`route_paste_interceptor` (`interaction/routing.rs:8-20`) walks the same focus/modal ancestor chain and calls the caller-supplied lookup closure for each component. The first `Some(Action)` wins.

`PasteInterceptors::action` converts the borrowed input text to an owned `String` before invoking the registered map (`application/input.rs:67-71`).

Interceptors therefore:

- precede component `on_paste` handlers;
- are scoped by `ComponentId`;
- are selected by focus/modal routing;
- produce an application action directly;
- do not enter the component output queue.

#### Ordinary component path

If no interceptor matches, `SceneHost::dispatch_paste` calls `route_paste` (`scene/host.rs:941-953`).

`route_paste`:

1. creates one `EventCx` borrowing the output queue;
2. walks the focus/modal chain;
3. skips components without a paste callback;
4. calls each callback with the borrowed text;
5. stops on `Consumed`;
6. returns `Ignored` if no callback consumes.

A paste callback can emit typed output while returning either result. The output is not routed until after `dispatch_paste` returns and the kernel calls `drain_outputs_to_actions`.

### 4.5 Focus reconciliation

`FocusState::reconcile_with_geometry` (`interaction/focus.rs:44-90`) performs:

1. Clone or clear the current geometry map.
2. Determine the latest modal scope from mount order.
3. Detect modal transition.
4. Restore a saved focus frame if returning to a previously seen modal scope.
5. Otherwise save the previous focus when entering a nested modal.
6. Clear stale restore frames for non-nested replacement transitions.
7. Compute eligible focus order from graph, capabilities, active modal, and visibility.
8. Choose:
   - a restored focus if valid;
   - the current focus if still eligible;
   - otherwise the first eligible component.
9. Call `set_focus`.

Focus eligibility (`focus.rs:299-317`) requires:

- component present in the mount graph;
- `focusable == true`;
- if geometry is supplied, a geometry entry with `visible.is_some()`.

Without geometry, all mounted focusable components are eligible. This makes focus reconciliation usable before layout has produced geometry.

### 4.6 Focus transitions and callbacks

`set_focus` (`focus.rs:248-280`) has two cases:

- If the target is unchanged, it refreshes the retained handler but does not call callbacks.
- If changed:
  1. take the previous callback;
  2. update `self.focused`;
  3. invoke the previous callback with `false`;
  4. select and retain the new callback;
  5. invoke the new callback with `true`.

`notify_focus_handler` uses `ComponentRegistry::with_any_mut` (`focus.rs:328-335`). The registry increments the component revision and invalidates its cached snapshot after every mutable access (`component/registry.rs:128-137`), including focus callbacks.

The test `interaction/tests/mod.rs:394-420` establishes that focus transitions advance component revisions. This is important because focus state is semantically rendered state for controls such as `TextInput`.

### 4.7 Incremental focus reconciliation

`reconcile_incremental` (`focus.rs:161-198`) updates geometry only for changed IDs, then avoids rebuilding complete focus order when:

- the current focus still exists;
- no changed capability affects focusability or modal ownership;
- the focused component remains visible;
- the focused component remains focusable.

When these conditions hold, it refreshes the focused callback and returns `false`. Otherwise, it falls back to full `reconcile_with_geometry`.

This is a performance specialization for topology-preserving updates. It does not change the source of truth: graph, capabilities, geometry, and registry remain authoritative.

### 4.8 Tick path

Capability ticks are declared through `ComponentCx::tick` (`interaction/command.rs:177-198`) and materialized by `TickScheduler`.

`TickScheduler::sync_capabilities` (`component/tick.rs:120-179`) builds registrations from the committed graph/capability set, updating intervals and replacing callback drivers. Stale registrations are removed.

`TickScheduler::tick_due_with_events` (`component/tick.rs:240-288`) creates one `EventCx` for the queue, invokes all due callbacks in mount order, and records changed components.

`SceneHost::tick_due` (`scene/host.rs:963-980`) invalidates components whose tick callback returned `true`.

The output queue is the same queue used for key and paste events. `RunningApp::advance_ready` ticks, drains outputs, and then processes resulting application actions (`application/kernel.rs:574-626`).

### 4.9 TextInput output path

`TextInput` declares all relevant interaction capabilities (`controls/text_input/mod.rs:297-309`):

- focusable;
- focus-change callback;
- typed key commands;
- paste callback;
- layout-change callback.

For output subscriptions, `TextInput` owns:

- one stable `submitted: Output<String>`;
- a `ChangeOutputs` projector vector.

`TextInput::output_on_change` (`controls/text_input/mod.rs:125-137`) creates and returns a new `Output<R>` channel. `ChangeOutputs::register` (`controls/text_input/output.rs:52-66`) stores a boxed projector and returns the corresponding output identity.

On a mutation:

```text
TextInput handler
    ↓
buffer mutation
    ↓
ChangeOutputs::emit
    ↓
each projector receives borrowed TextChange<'_>
    ↓
projector returns owned R: 'static
    ↓
EventCx::emit(Output<R>, R)
    ↓
SceneHost output queue
```

`TextChange<'a>` is borrowed (`controls/text_input/output.rs:5-25`), but it never escapes the synchronous projector call. The emitted result must be `'static`, so queue retention is safe.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation to production-path table

| Semantic operation | Primary production path | Alternate/secondary path | Selection condition | Failure or masking behavior |
|---|---|---|---|---|
| Key normalization from terminal | crossterm `Event` → `key_stroke` → `KeyStroke` | native `parse_key` from strings | backend mode | release keys dropped; unsupported native names rejected |
| Key dispatch | `RunningApp::dispatch_key` → `SceneHost::dispatch_key_local` → `route_key_local` | application-global `GlobalBindings` | local result is `Ignored` | local consumed suppresses global binding |
| Forward focus traversal | `route_key_local` → `FocusState::focus_next` | component command or global binding | exactly unmodified `Tab`, and focus transition succeeds | ignored when no eligible focus or singleton cycle has no effective change |
| Reverse focus traversal | no built-in route in assigned interaction code | component command/global binding | caller-defined | no native default Shift-Tab behavior |
| Paste dispatch | `RunningApp::dispatch_paste` | deferred `forward_paste` queue | direct terminal paste vs application-requested deferred paste | interceptor is bypassed for deferred forward paste |
| Paste interception | `SceneHost::intercept_paste` → `route_paste_interceptor` | none | first matching focus/modal chain interceptor | unmounted/background registrations are unreachable |
| Component paste | `SceneHost::dispatch_paste` → `route_paste` | none | no interceptor matched | ignored handlers allow ancestor fallback |
| Component output emission | `EventCx::emit` → `OutputQueue` | same for tick and paste | callback has `EventCx` | no immediate action dispatch |
| Output action mapping | `SceneHost::drain_outputs` → `OutputRouter::drain` | no route | route exists at drain time | unrouted events silently dropped |
| Output route replacement | `remove` then `route` | none | caller explicitly changes route | duplicate registration otherwise returns `RouteConflict` |
| Output payload validation | `TypeId` comparison in `drain` | forged test queue entry | internal corruption only | returns `TypeMismatch`; queue is partially consumed |
| Tick callback | `TickScheduler` → `EventCx` | no alternate callback path | component mounted and tick due | stale registrations removed on capability sync |
| TextInput change subscription | `ChangeOutputs` projector vector | stable `submitted` channel | caller calls `output_on_change` vs `submitted` | projector remains until component destruction; no per-projector unsubscribe |

### 5.2 Unrouted outputs are intentionally dropped

`OutputRouter::drain` removes every queue entry with `pop_front` before checking whether a route exists (`output/router.rs:87-90`). If no route is registered, that event is discarded and processing continues.

This is different from a dispatch failure. The source treats “no current consumer” as a valid condition. The tests at `output/tests/router.rs:11-39` explicitly include an unrouted output and expect it not to appear in the returned action vector.

This means route registration timing matters: an event emitted before a route is installed is not replayed.

### 5.3 Type mismatch is explicit but not transactional

When a routed queue entry has the wrong payload `TypeId`, `drain` returns `Err(OutputDispatchError::TypeMismatch)` (`router.rs:92-94).

The queue is not transactional:

- all earlier events have already been popped;
- the mismatched event itself has also been popped from the `VecDeque` before returning;
- later events remain in the queue.

Therefore, after a mismatch, retrying the same queue cannot recover the discarded prefix or mismatched event. This is acceptable as an internal-invariant failure, but it is consequential for error recovery and should not be treated as a recoverable route miss.

### 5.4 Type mismatch does not arise from normal public routing

`Output<T>` carries one compile-time payload type, and `OutputRouter::route` records the same `TypeId` from `T`. Under ordinary source use, an event emitted through an `Output<T>` reaches the same channel identity with payload `T`.

The only in-tree mismatch path is the crate-private test helper `OutputQueue::push_mismatched_for_test` (`output/event.rs:46-57`). The loud error is still valuable because it protects against future internal misuse, unsafe integration, or accidental changes to erasure logic.

### 5.5 Handler mismatch is loud rather than masked

Interaction capability handlers downcast component and command values with `expect`. A wrong component type or wrong erased command is a programming error and panics. The tests at `interaction/tests/mod.rs:902-919` verify this policy.

No fallback handler is attempted after a downcast failure.

### 5.6 Global key fallback is intentionally late

`AppCx::bind_key` stores exact `KeyStroke` factories in a `HashMap` (`application/input.rs:5-28`). `RunningApp::dispatch_key` only consults the global map after:

1. the focused component;
2. all ancestors through the modal boundary;
3. built-in unmodified-Tab focus traversal;
4. output draining from the local route.

The application context documentation at `application/context.rs:170-180` records this ordering explicitly. A global binding therefore cannot preempt a component-local command.

### 5.7 Paste interceptor is before ordinary component paste

The application-level interceptor path runs before `on_paste` component capability routing. This lets an application action consume a paste without mutating the focused input.

`AppCx::forward_paste` is explicitly different: it queues text for ordinary focused-component routing and bypasses interceptors (`application/context.rs:205-209`). This prevents a paste interceptor from recursively intercepting its own forwarded paste.

### 5.8 Input polling stops after an action is available

`application/host.rs:1500-1505` stops consuming terminal input once `RunningApp::has_pending_actions()` is true. The source comment explains that the caller must reduce that action before later keystrokes can alter focus or clear a composer.

This is a correctness boundary, not merely a throughput optimization: action reduction must happen before later input changes the interaction state.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Focus work

Full focus reconciliation:

- clones the supplied geometry map;
- computes modal transition;
- walks mount order to build eligible focus order;
- may invoke blur/focus callbacks.

Incremental reconciliation:

- updates only changed geometry entries;
- avoids rebuilding focus order if changed capabilities cannot affect eligibility/modal ownership;
- falls back to full reconciliation when focus visibility or focusability may have changed.

The focus geometry map is retained as a clone, so later incremental updates can patch only changed component IDs. When no geometry is available, the retained geometry state is cleared.

### 6.2 Routing work

Key and paste routing allocate a `Vec<ComponentId>` for the routing chain (`routing.rs:88-110`). This chain is bounded by the focused component’s ancestor depth and the active modal boundary.

Key routing has two nested loops:

- routing-chain depth;
- number of registered commands for each component.

Each mapped command performs:

- immutable registry access for mapping;
- mutable registry access for handling.

The queue `EventCx` remains borrowed for the whole routing operation, preventing output draining or recursive action reduction during callback execution.

### 6.3 Capability snapshot caching

`ComponentRegistry::resolution` caches a `ComponentSnapshot` keyed by the component revision (`component/registry.rs:157-175`). The snapshot contains:

- view;
- revision;
- capability snapshot.

Any mutable component access through `with_any_mut` or `with_mut` increments the revision and clears the snapshot (`registry.rs:128-137, 178-191`). Focus callbacks, key handlers, paste handlers, and tick callbacks therefore invalidate the component’s cached semantic view/capabilities even when they only mutate interaction state.

This is the mechanism that makes focus changes visible to subsequent scene preparation.

### 6.4 Tick scheduling

Tick registration state includes:

- interval;
- next deadline;
- erased callback driver.

`TickScheduler` additionally retains:

- mounted ID set;
- mount order vector.

On capability synchronization, unchanged intervals preserve the registration schedule unless the prior `next_due` is absent. Interval changes reset activation. Tick deadlines are computed by taking the minimum `next_due` in mount order (`component/tick.rs:225-231`).

A due tick does not itself guarantee rendering. The callback’s boolean return controls `TickOutcome.dirty`; changed components are separately invalidated in `SceneHost::tick_due`.

### 6.5 Output queue work

Output emission is O(1) amortized `VecDeque::push_back`.

Output drain is O(number of queued events), plus hash lookups and route closure execution. Payloads are boxed per emission, and heterogeneous events remain in strict global FIFO order.

There is no queue capacity bound in `OutputQueue`. The source does not expose queue length or overflow diagnostics in production. Boundedness is indirectly controlled by:

- terminal input pump budget;
- application action batch budget;
- tick scheduling;
- caller behavior.

The test-only `is_empty` method is the only queue observability helper (`output/event.rs:25-28`).

### 6.6 Route cache and retention

`OutputRouter` uses a `HashMap<OutputId, Route<A>>`; route entries remain until explicitly removed. The route map does not automatically track component mount state or component lifetime.

Consequences:

- a route can remain valid after its originating component is removed;
- a route can map events from any producer that possesses the same copied channel handle;
- a removed route causes future matching events to be silently dropped;
- a newly registered route can receive events emitted after registration, but not events already drained or discarded.

### 6.7 TextInput output projectors

Each `TextInput` change subscription stores one boxed `ChangeProjector` (`controls/text_input/output.rs:52-73`). Every mutation iterates all projectors synchronously.

Work per mutation is therefore:

```text
O(number of registered change projectors)
+
projector computation
+
one queue allocation per emitted result
```

There is no output-projector removal API. Projectors are retained for the lifetime of the `TextInput`. The stable `submitted` channel is separate and does not consume one projector slot.

### 6.8 Scheduling and output ordering

The same queue is used by:

- key handlers;
- paste handlers;
- tick handlers.

The application kernel drains outputs:

- immediately after direct key dispatch;
- immediately after direct paste dispatch;
- after due ticks;
- after each action update;
- during deferred paste processing.

This gives a common FIFO boundary for component-produced events, but action ordering still depends on when each production route is drained. The source does not globally merge all future terminal events, timer actions, and output events into one queue before dispatch; it uses explicit scheduling phases.

---

## 7. Tests, benchmarks and observability

### 7.1 Interaction tests

`interaction/tests/mod.rs` contains focused behavioral contracts for:

- cyclic focus traversal (`251-281`);
- singleton focus traversal preserving focus (`283-298`);
- typed command mutation and deferred output (`300-330`);
- output emission before ancestor consumption (`332-362`);
- ignored child bubbling to ancestor but not sibling (`364-392`);
- focus callback revision changes (`394-420`);
- blur on removed focused component (`423-467`);
- blur on lost focusability (`469-514`);
- tick capability emission and disablement (`516-559`);
- nested modal focus containment/restoration (`561-669`);
- modal hierarchy removal (`670-797`);
- modal replacement without stale restore frames (`798-900`);
- loud type-erasure failure (`902-919`);
- backend-neutral modifier value semantics (`921-926`).

These tests demonstrate that interaction is not merely a collection of callbacks. It has explicit focus, modal, lifecycle, and queue-order contracts.

### 7.2 Output tests

`output/tests/event.rs` verifies:

- non-`Clone`, non-`Send` payloads can be queued and routed;
- output payload observes post-mutation state;
- emission waits until queue draining;
- a live `EventCx` does not synchronously invoke route closures.

`output/tests/handle.rs` verifies:

- copied handles preserve identity;
- distinct `Output::new()` calls are distinct;
- payload traits are not required by handle traits;
- output handles do not require payload `Send` or `Clone`.

`output/tests/router.rs` verifies:

- heterogeneous FIFO order;
- duplicate route rejection preserving the first route;
- removal and re-registration;
- stale output identities not matching fresh channels;
- forged type mismatch failure;
- sequential drain boundaries.

### 7.3 Cross-subsystem tests

Supporting tests provide additional evidence:

- `controls/text_input/tests/mod.rs:93-223` tests paste routing, ancestor fallback, deferred typed output, and modal containment.
- `controls/text_input/tests/output.rs:15-111` tests stable submission channels and ordered change output projection.
- `application/tests.rs:367-466` exercises integrated input/output/timer/history behavior.
- `application/tests.rs:501-567` verifies that paste interceptors are selected through current mount routing and stop being reachable after component removal.
- `application/tests.rs:848-876` verifies that mounted tick output enters the same action queue.
- `application/tests.rs:981-1015` verifies route conflict, removal, and re-registration.
- `application/tests.rs:1719-1805` verifies local-vs-global key precedence and exact binding behavior.
- `terminal/crossterm/key.rs:116-200` tests key normalization, release suppression, BackTab canonicalization, modifier preservation, and preservation of character control keys.

### 7.4 Observability limitations

There are no production counters specific to:

- focus-chain length;
- number of attempted local command mappings;
- number of ignored versus consumed key/paste callbacks;
- output queue depth;
- dropped unrouted output count;
- route dispatch latency;
- number of active output routes;
- number of `TextInput` change projectors;
- output drain partial-consumption state.

Existing test-only visibility includes queue emptiness and component revisions, but no production diagnostics distinguish:

- a callback that was never registered;
- a callback that returned `Ignored`;
- a channel that was unrouted;
- a channel whose route was removed;
- an event dropped because application exit was already requested.

### 7.5 Validation status

No tests were executed in this investigation. The tests listed above are source evidence only.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Generic Rust typed outputs versus native string routes

The core output layer is strongly typed:

```text
Output<T> → OutputRouter<Action> → Action
```

The native host façade converts selected typed outputs into:

```text
RoutedOutput {
    route_id: String,
    payload: Option<String>,
}
```

Evidence:

- `application/host.rs:40-45` defines `RoutedOutput`;
- `application/host.rs:1178-1186` maps key bindings to route IDs;
- `application/host.rs:1262-1296` maps `Output<String>` channels to route IDs;
- `application/host.rs:1412-1414` exposes queued routed outputs.

This is not a contradiction in the current architecture: native callers need a serializable route envelope, while the internal framework preserves generic typed outputs. It is, however, an important boundary where type information is intentionally reduced to `Option<String>`.

### 8.2 Rust key vocabulary versus native parser vocabulary

The Rust key model includes media and physical modifier variants (`interaction/key.rs:3-70`), but native string parsing accepts only a subset (`iyon-tui-native/src/tui.rs:1043-1086`).

The crossterm adapter supports the full crossterm mapping, including media and physical modifiers (`terminal/crossterm/key.rs:71-106`). The native `dispatchKey` path does not expose equivalent string names.

This produces two legitimate input modes with different representational coverage:

- terminal backend: richer backend-derived key vocabulary;
- native host: validated, simpler string vocabulary.

The source does not document whether this difference is intentional API narrowing or an incomplete native mapping.

### 8.3 Native input remains Rust-owned

`AGENTS.md:83-103, 128` makes native keyboard routing a framework responsibility. The source complies:

- terminal decoding happens in Rust;
- `KeyStroke` normalization happens in Rust;
- focus, modal scope, key command, paste, and tick routing happen in Rust;
- TypeScript/native callers submit normalized key names or receive routed output envelopes.

No TypeScript callback is required for every keystroke.

### 8.4 Output routes are independent of component lifetime

A `ComponentHandle` and an `Output<T>` are separate identity systems:

- component IDs are allocated and owned by `ComponentRegistry`;
- output IDs are process-wide atomically allocated in `output/handle.rs`.

Removing a component removes its component registry entry and paste interceptor, but does not automatically remove output routes. This is source-backed by:

- `AppCx::remove_component` removing the component and paste interceptor only (`application/context.rs:108-118`);
- `OutputRouter::remove` being a separate explicit API (`output/router.rs:80-82`).

This independence is useful for application-level channels but creates a lifetime hazard if callers expect routes to disappear automatically with producers.

### 8.5 Output queue lifecycle is broader than a single callback

The output queue is owned by `SceneHost`, not by an individual dispatch call. `SceneHost::clear_retained_views` resets retained scene/layout/interaction derivations (`scene/host.rs:305-327`) but does not clear `self.outputs`.

Similarly, candidate discard paths reset retained presentation state but do not show an output queue reset. Pending component output events therefore belong to the host/action lifecycle, not the retained-frame lifecycle.

This is probably necessary to avoid losing events across render retries, but it means queue lifetime must be reasoned about separately from scene candidate lifetime.

### 8.6 Deferred component retirement is coupled to interaction routing

The kernel intentionally does not immediately reclaim a removed component. It waits until the committed mount graph no longer contains its ID (`application/kernel.rs:113-142`).

This matters to interaction:

- the old focused callback may still need to blur the component;
- a failed candidate may still leave the old graph authoritative;
- paste interceptors must remain available in registration storage until retirement is safe;
- component IDs cannot be reused while stale graph references remain.

### 8.7 Focus callback retention is a deliberate cross-snapshot bridge

`FocusState` retains the prior `Arc<dyn Fn(&mut dyn Any, bool)>` independently from `MountedCapabilities`. When a replacement scene omits the old component capability, `set_focus` can still call the old callback with `false`.

This is validated by `interaction/tests/mod.rs:423-467` and prevents stale focused visual state in the component instance before it is retired.

### 8.8 `TextInput` subscriptions are output channels, not callback subscriptions to the application

`TextInput::output_on_change` registers a local projector and returns an `Output<R>`. It does not register an application route. The caller must separately call `AppCx::route` or a host wrapper.

This separation is visible in:

- `controls/text_input/output.rs:52-66`;
- `controls/text_input/mod.rs:131-137`;
- `application/context.rs:121-127`.

Thus there are two lifecycle layers:

```text
TextInput lifetime
    owns projector and output identity

RunningApp lifetime
    owns route from output identity to Action
```

Neither layer automatically tears down the other.

### 8.9 Documentation/source alignment

The current source and `AGENTS.md` agree that:

- native input is framework-owned;
- generic output channels are caller-defined;
- component capabilities, focus, paste, and ticks are generic;
- product/application semantics belong outside this Rust framework.

No assignment-scope contradiction was found between the source and the mandatory framework boundary.

---

## 9. Open questions and coverage gaps

1. **Native parser coverage**
   - Is the reduced `parse_key` vocabulary intentional?
   - Should native callers eventually be able to submit media keys and physical modifier keys?
   - The current source does not provide a complete native key-name schema.

2. **Output queue recovery**
   - On `OutputDispatchError::TypeMismatch`, should the queue remain partially consumed by design?
   - The current source treats this as an invariant failure, but there is no transaction marker or queue reset policy.

3. **Unrouted output observability**
   - Unrouted events are silently dropped.
   - It is unclear whether this is always desired or whether debug-only accounting would be useful.

4. **Route lifetime policy**
   - Output routes are not tied to component lifetime.
   - The source does not define whether application code is expected to remove routes when a producer component is retired.

5. **TextInput projector removal**
   - `output_on_change` has no unsubscribe API.
   - The source does not clarify whether a projector’s lifetime is intentionally equal to the `TextInput` lifetime in all use cases.

6. **Focus geometry semantics**
   - Focus eligibility uses `entry.visible.is_some()`, but the exact construction and meaning of `ComponentGeometryMap` visibility is outside the assigned scope.
   - The source does not expose whether zero-size but present components can ever be focusable.

7. **Focus order definition**
   - `eligible_focus_order_with_geometry` uses `MountGraph::iter()`.
   - The exact traversal/order guarantees of `MountGraph::iter()` are defined outside the assigned directories and were not fully reconstructed here.

8. **Modal restoration semantics under arbitrary graph replacement**
   - Tests cover nested entry, removal, and replacement.
   - The source does not document all possible behavior when modal IDs are retained but parent topology changes simultaneously.

9. **Mouse and focus terminal events**
   - crossterm drops `FocusGained`, `FocusLost`, and `Mouse` events (`terminal/crossterm/mod.rs:21-27`).
   - No pointer routing or terminal focus routing exists in the inspected path.
   - It is unknown whether another backend or native façade supplies those semantics.

10. **Exit behavior with pending output**
    - `advance_ready` clears actions and timers when exiting (`application/kernel.rs:578-583`), but the source does not explicitly drain or clear the `SceneHost` output queue at exit.
    - The queue is eventually dropped with the host, but the exact intended treatment of pending component events is undocumented.

11. **Runtime validation**
    - No tests or builds were run in this investigation.
    - Dynamic ordering, error propagation, and native host behavior remain source-inferred rather than execution-observed.

---

## 10. Evidence appendix

### 10.1 Inspected production files

#### Assigned interaction scope

- `crates/iyon-tui/src/interaction/mod.rs`
- `crates/iyon-tui/src/interaction/command.rs`
- `crates/iyon-tui/src/interaction/focus.rs`
- `crates/iyon-tui/src/interaction/key.rs`
- `crates/iyon-tui/src/interaction/result.rs`
- `crates/iyon-tui/src/interaction/routing.rs`

#### Assigned output scope

- `crates/iyon-tui/src/output/mod.rs`
- `crates/iyon-tui/src/output/event.rs`
- `crates/iyon-tui/src/output/handle.rs`
- `crates/iyon-tui/src/output/router.rs`

#### Assigned tests

- `crates/iyon-tui/src/interaction/tests/mod.rs`
- `crates/iyon-tui/src/output/tests/mod.rs`
- `crates/iyon-tui/src/output/tests/event.rs`
- `crates/iyon-tui/src/output/tests/handle.rs`
- `crates/iyon-tui/src/output/tests/router.rs`

### 10.2 Supporting production files inspected

- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/id.rs`
- `crates/iyon-tui/src/component/mod.rs`
- `crates/iyon-tui/src/component/capability.rs`
- `crates/iyon-tui/src/component/registry.rs`
- `crates/iyon-tui/src/component/tick.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/application/app.rs`
- `crates/iyon-tui/src/application/context.rs`
- `crates/iyon-tui/src/application/input.rs`
- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/controls/text_input/mod.rs`
- `crates/iyon-tui/src/controls/text_input/output.rs`
- `crates/iyon-tui/src/controls/text_input/tests/mod.rs`
- `crates/iyon-tui/src/controls/text_input/tests/output.rs`
- `crates/iyon-tui/src/scroll_command.rs`
- `crates/iyon-tui/src/terminal/backend.rs`
- `crates/iyon-tui/src/terminal/crossterm/mod.rs`
- `crates/iyon-tui/src/terminal/crossterm/key.rs`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
- `crates/iyon-tui/src/binding/mod.rs`
- `crates/iyon-tui-native/src/tui.rs`

### 10.3 Relevant exact symbols and line references

#### Interaction

- `ComponentCx`, `ComponentCapabilities`, `MountedCapabilities`: `interaction/command.rs:25-223`
- `FocusState`: `interaction/focus.rs:9-335`
- `eligible_focus_order_with_geometry`: `interaction/focus.rs:299-317`
- `routing_chain`: `interaction/routing.rs:88-110`
- `route_key_local`: `interaction/routing.rs:45-85`
- `route_paste`: `interaction/routing.rs:22-42`
- `route_paste_interceptor`: `interaction/routing.rs:8-20`
- `Key`, `MediaKey`, `ModifierKey`, `Modifiers`, `KeyStroke`: `interaction/key.rs:3-144`
- `InteractionResult`: `interaction/result.rs:1-6`

#### Output

- `ErasedOutputEvent`, `OutputQueue`, `EventCx`: `output/event.rs:8-68`
- `OutputId`, `Output<T>`: `output/handle.rs:5-68`
- `OutputRouter`, `RouteConflict`, `OutputDispatchError`: `output/router.rs:18-100`

#### Host/application seams

- `AppCx::route`, `AppCx::remove_route`: `application/context.rs:121-133`
- `AppCx::bind_key`, `AppCx::unbind_key`: `application/context.rs:170-180`
- `AppCx::intercept_paste`, `AppCx::remove_paste_interceptor`, `AppCx::forward_paste`: `application/context.rs:183-209`
- `GlobalBindings`, `PasteInterceptors`: `application/input.rs:5-71`
- `RunningApp::dispatch_key`: `application/kernel.rs:523-546`
- `RunningApp::dispatch_paste`: `application/kernel.rs:549-571`
- `RunningApp::advance_ready`: `application/kernel.rs:574-626`
- `RunningApp::drain_outputs_to_actions`: `application/kernel.rs:811-817`
- `SceneHost::dispatch_key_local`: `scene/host.rs:918-930`
- `SceneHost::intercept_paste`: `scene/host.rs:933-939`
- `SceneHost::dispatch_paste`: `scene/host.rs:941-953`
- `SceneHost::drain_outputs`: `scene/host.rs:956-961`
- `SceneHost::tick_due`: `scene/host.rs:963-980`

#### Lifetime and scheduling seams

- Component sole ownership: `component/registry.rs:65-98`
- Component revision/snapshot invalidation: `component/registry.rs:123-137, 157-191`
- Deferred component retirement: `application/kernel.rs:113-142`
- Tick synchronization: `component/tick.rs:120-179`
- Tick output emission: `component/tick.rs:240-288`
- Host tick invalidation: `scene/host.rs:963-980`

#### TextInput output/subscription seams

- `TextInput` output fields and non-`Clone` lifetime statement: `controls/text_input/mod.rs:33-47`
- stable submission channel: `controls/text_input/mod.rs:125-129`
- change subscription registration: `controls/text_input/mod.rs:131-137`
- borrowed `TextChange`: `controls/text_input/output.rs:5-25`
- projector registration/emission: `controls/text_input/output.rs:29-73`
- TextInput capabilities: `controls/text_input/mod.rs:297-309`

#### Native input seams

- terminal event abstraction: `terminal/backend.rs:30-40`
- crossterm event mapping: `terminal/crossterm/mod.rs:21-27`
- crossterm key normalization: `terminal/crossterm/key.rs:7-106`
- termwiz event polling: `terminal/termwiz/backend.rs:85-124`
- native key dispatch API: `iyon-tui-native/src/tui.rs:941-946`
- native key parser: `iyon-tui-native/src/tui.rs:1043-1086`
- native routed output envelope: `application/host.rs:40-45`
- native route registration: `application/host.rs:1178-1317`
- native output retrieval: `application/host.rs:1412-1414`

### 10.4 Contract and context files read

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `AGENTS.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The pre-V5 report was consulted for the required inventory/dependency/evidence discipline and interaction/output census expectations. No V5 disposition claims are made here.

### 10.5 Files indexed but not read comprehensively

Within the repository, broad source searches indexed many other Rust, TypeScript, generated, and documentation files. They were not treated as assigned production evidence unless listed above. In particular:

- unrelated presentation, history, stream, projection, backend, and native source files;
- TypeScript package implementation files;
- generated ABI/schema bodies;
- benchmark sources;
- repository-wide application tests outside the cited interaction/output seams.

### 10.6 LOC methodology

- Counts are approximate physical source lines based on file line numbering.
- Production and test files are separated.
- Blank lines, comments, module declarations, and test scaffolding are included.
- Generated files are absent from the assigned directories.
- No executable validation was performed.