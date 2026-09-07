# 38 — Input callbacks

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `38`, `traces/input-callbacks`
- Primary scope: terminal decoding through native routing and TypeScript callbacks.
- Specific goal: real keyboard/paste/focus/scroll path, subscriptions, native control state, output transport, callback-induced update, and pointer-support evidence.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md` for architecture-report evidence expectations and the interaction/backend/ABI requirements.

The current source is treated as authoritative for executing behavior. Historical handoffs and design documents are not treated as proof that a route exists.

### Scope boundary

This report follows input and output behavior across:

```text
terminal backend
  → normalized TerminalEvent
  → native Host/Tui runtime
  → FocusState + MountGraph + MountedCapabilities
  → component key/paste handlers
  → typed OutputQueue / OutputRouter
  → application Action queue
  → application update callback
  → dirty/invalidation state
  → semantic view/frame preparation
  → terminal presentation

and, for the TypeScript facade:

real terminal or test injection
  → NativeTuiHost N-API methods
  → native routed output
  → Tui.nextEvent()
  → TypeScript TuiEvent
```

This report does not analyze general layout, History implementation, content projection, or structural ABI internals except where they are needed to explain callback-induced rendering or terminal output.

### Evidence status

This is a static source inspection. I did not run tests, compile the workspace, build the native addon, start a terminal service, or execute a benchmark. Test files are cited as behavioral evidence encoded in the repository, not as tests run during this investigation.

### Important headline finding

The production input route is deliberately native-first:

- crossterm reads terminal events on a dedicated blocking thread.
- Rust normalizes supported events to a private `TerminalEvent`.
- Rust owns focus, routing, TextInput editing, ScrollPane scrolling, and component callbacks.
- TypeScript receives only caller-defined routed outputs or termination events.
- TypeScript does not receive raw keyboard, paste, focus, scroll, or pointer events.

The source also contains an exported TypeScript `ComponentAdapter`/`AsyncComponentAdapter` with `onKey`, `onPaste`, and `onTick` methods. Within the inspected repository, those types are only defined, exported, and unit-tested as standalone adapters; no production composition or native mounting path consumes them. Therefore, they do not currently form a TypeScript component-event path.

Pointer and terminal focus support are absent from the production route: `Event::Mouse(_)`, `FocusGained`, and `FocusLost` are explicitly discarded, and terminal setup enables raw mode and bracketed paste but no mouse or focus reporting.

---

## 1. Responsibility and structure

### 1.1 Terminal decoding and terminal session

| File | Responsibilities | Approximate physical LOC |
|---|---|---:|
| `crates/iyon-tui/src/terminal/backend.rs` | Private `TerminalEvent`, `TerminalBackend`, presentation receipt and worker failure types | 49 |
| `crates/iyon-tui/src/terminal/crossterm/mod.rs` | Raw mode, bracketed-paste activation, blocking crossterm reader thread, event mapping | 119 |
| `crates/iyon-tui/src/terminal/crossterm/key.rs` | Crossterm `KeyEvent` → backend-neutral `KeyStroke` conversion and tests | 200 |
| `crates/iyon-tui/src/terminal/termwiz/backend.rs` | Termwiz backend, input channel polling, terminal worker command transport, resize tracking | 167 |
| `crates/iyon-tui/src/terminal/termwiz/worker.rs` | Terminal initialization, termwiz capability probing, presenter command worker, restore | 163 |
| `crates/iyon-tui/src/terminal/termwiz/presenter.rs` | Differential/full presentation, native scrollback insertion, flush and failure recovery; includes extensive tests | approximately 1,200 |
| `crates/iyon-tui/src/terminal/termwiz/lower.rs` | Physical rows/cells → termwiz changes | not independently line-counted |
| `crates/iyon-tui/src/terminal/termwiz/mod.rs` | Module wiring | approximately 10 |

The line counts above are source-line-span approximations based on the checked-in files and source line references; they are not a fresh `wc -l` measurement. `presenter.rs` and `application/host.rs` contain substantial test sections, so their approximate spans include both production and test code.

### 1.2 Native interaction and component capabilities

| File | Responsibilities | Approximate physical LOC |
|---|---|---:|
| `crates/iyon-tui/src/interaction/key.rs` | Backend-neutral `Key`, `Modifiers`, `KeyStroke` | 144 |
| `crates/iyon-tui/src/interaction/result.rs` | `InteractionResult::{Ignored, Consumed}` | 6 |
| `crates/iyon-tui/src/interaction/command.rs` | `ComponentCapabilities`, callback types, `ComponentCx`, mounted capability map | 223 |
| `crates/iyon-tui/src/interaction/focus.rs` | Host-owned focused component, modal focus, focus traversal, focus callbacks, geometry-aware eligibility | 334 |
| `crates/iyon-tui/src/interaction/routing.rs` | Focus-to-ancestor key/paste routing and paste interception | 110 |
| `crates/iyon-tui/src/interaction/mod.rs` | Private/public exports | approximately 16 |
| `crates/iyon-tui/src/scroll.rs` | Retained `ScrollPane`, FollowEnd/detached scrolling, scroll key commands | 291 |
| `crates/iyon-tui/src/scroll_command.rs` | Exact unmodified-key-to-scroll-command mapping | 26 |
| `crates/iyon-tui/src/controls/text_input/mod.rs` | Retained editor state, component implementation, callback bridge, presentation | approximately 310 |
| `crates/iyon-tui/src/controls/text_input/command.rs` | Text editing key map and command execution | 136 |
| `crates/iyon-tui/src/controls/text_input/output.rs` | Borrowed `TextChange`, typed output projection, change-output registration | 73 |
| `crates/iyon-tui/src/controls/text_input/edit.rs` | Paste/text canonicalization and word separator rules | 20 |
| `crates/iyon-tui/src/controls/text_input/cursor.rs` | Grapheme-aware cursor/display helpers | 89 |
| `crates/iyon-tui/src/controls/text_input/presentation.rs` | TextInput semantic view and cursor/scroll presentation | 85 |

### 1.3 Application kernel and native host

| File | Responsibilities | Approximate physical LOC |
|---|---|---:|
| `crates/iyon-tui/src/application/input.rs` | Global key bindings and component-scoped paste interceptors | 72 |
| `crates/iyon-tui/src/application/context.rs` | `AppCx`: registration, routing, binding, paste forwarding, timers, exit | 227 |
| `crates/iyon-tui/src/application/kernel.rs` | `RunningApp`, action queue, event dispatch, output draining, application update, frame invalidation | 821 |
| `crates/iyon-tui/src/application/host.rs` | Native `TuiHost`, `HostTextInput`, `HostScrollPane`, terminal polling, routed output state, host render/commit lifecycle; includes extensive tests | approximately 4,100 |
| `crates/iyon-tui/src/application/run.rs` | Present-receipt and deadline waits | 22 |
| `crates/iyon-tui/src/application/tests.rs` | Application/kernel/terminal-input behavioral tests | approximately 1,900 |
| `crates/iyon-tui/src/application/tests/driver.rs` | Fake-terminal runtime driver and input dispatch helper | approximately 450 |

`application/host.rs` is the central seam for this assignment. It combines the native public host wrapper, native controls, event polling, output extraction, and rendering transaction management.

### 1.4 Output transport

| File | Responsibilities | Approximate physical LOC |
|---|---|---:|
| `crates/iyon-tui/src/output/event.rs` | Type-erased deferred event queue and `EventCx::emit` | 68 |
| `crates/iyon-tui/src/output/handle.rs` | Opaque typed `Output<T>` and monotonically allocated output identity | 68 |
| `crates/iyon-tui/src/output/router.rs` | Typed route registration, conflict detection, erased-event draining and type checking | 101 |
| `crates/iyon-tui/src/output/mod.rs` | Public/internal exports and module contract | 18 |

### 1.5 Native addon and TypeScript facade

| File | Responsibilities | Approximate physical LOC |
|---|---|---:|
| `crates/iyon-tui-native/src/tui.rs` | N-API wrappers for `NativeTuiHost`, `NativeTextInput`, `NativeScrollPane`, `NativeTuiOutput`, key parsing and host operations; includes tests | approximately 2,000 |
| `packages/iyon-tui/src/transport/native/addon.ts` | Private TypeScript native contracts | 248 |
| `packages/iyon-tui/src/runtime/runtime.ts` | `TuiRuntime`, `Tui`, N-API event/output wrappers and lifecycle | approximately 900 |
| `packages/iyon-tui/src/runtime/events.ts` | `OutputEvent`, `TerminateEvent`, `TuiEvent` | 14 |
| `packages/iyon-tui/src/api/controls/text-input.ts` | TypeScript TextInput handle and stable submitted output facade | 95 |
| `packages/iyon-tui/src/api/controls/scroll-pane.ts` | TypeScript ScrollPane handle, retained content and native scrolling method | 264 |
| `packages/iyon-tui/src/api/controls/output.ts` | Opaque TypeScript `Output<T>` brand | 8 |
| `packages/iyon-tui/src/api/extensions/traits/component.ts` | Exported TypeScript component adapter types and standalone async adapter | 53 |
| `packages/iyon-tui/src/testing/index.ts` | Headless harness input injection and output access | approximately 150 |
| `packages/iyon-tui/tests/tui_runtime.test.ts` | TypeScript native-host input/output tests | 31 |
| `packages/iyon-tui/tests/tui_harness.test.ts` | Headless TextInput, key, output, history, and snapshot tests | 49 |
| `packages/iyon-tui/tests/tui_traits.test.ts` | Standalone `AsyncComponentAdapter` test | approximately 35 |

### Primary versus secondary responsibility

The primary input responsibility is split as follows:

```text
crossterm             decode physical terminal input
TerminalBackend       private semantic terminal boundary
TuiHost.poll_terminal feed events into RunningApp
RunningApp            local routing, global binding, action queue
SceneHost             focus graph, component capability dispatch, OutputQueue
TextInput/ScrollPane  native control state and command semantics
OutputRouter          typed output → application Action
host_update           Action → application state mutation
Tui.nextEvent         routed output → TypeScript event
```

Rendering is a consequence of input handling rather than a callback destination. A consumed key or paste marks the application dirty, and the host renders a new semantic/frame state after action reduction.

---

## 2. Types, APIs and contracts

### 2.1 Terminal event contract

`crates/iyon-tui/src/terminal/backend.rs::TerminalEvent` is private and has exactly three variants:

```rust
Key(crate::KeyStroke)
Paste(String)
Resize
```

The private `TerminalBackend` trait exposes:

```rust
try_next_event() -> Result<Option<TerminalEvent>>
viewport() -> Result<Size>
begin_frame(&PreparedSceneFrame) -> Result<PresentReceipt>
position_after_final_frame() -> Result<()>
restore() -> Result<()>
```

There is no public raw-event stream. There is no terminal-level `FocusGained`, `FocusLost`, `Mouse`, wheel, click, hover, selection, or pointer event variant.

### 2.2 Key identity

`crates/iyon-tui/src/interaction/key.rs` defines backend-neutral `Key`, `Modifiers`, and `KeyStroke`.

`Key` includes:

- printable/control characters through `Char(char)`;
- Enter, Escape, Backspace, Tab, Delete, Insert;
- Home, End, PageUp, PageDown;
- arrows;
- function keys;
- lock/media/modifier keys;
- Null and KeypadBegin.

`Modifiers` preserves Shift, Control, Alt, Super, Hyper, and Meta. `KeyStroke` is the normalized pair `(Key, Modifiers)` used as the command-routing key.

`crates/iyon-tui/src/terminal/crossterm/key.rs::key_stroke`:

- drops `KeyEventKind::Release`;
- accepts press and repeat;
- canonicalizes `BackTab` into `Key::Tab` while adding Shift;
- preserves all supported crossterm modifier bits;
- maps `KeyCode::Char(c)` directly to `Key::Char(c)`;
- does not invent Enter from newline/carriage-return character events.

The corresponding tests explicitly protect release dropping, repeat handling, modifier preservation, Enter/Shift-Enter distinction, character newline behavior, and ETX preservation (`key.rs:108-200`).

### 2.3 Component capability contract

`crates/iyon-tui/src/interaction/command.rs::ComponentCapabilities` stores:

```rust
focusable: bool
modal_scope: bool
focus_changed: Option<Arc<FocusChanged>>
paste: Option<Arc<PasteHandler>>
key_commands: Vec<KeyCommandCapability>
tick: Option<TickCapability>
layout_changed: Option<Arc<LayoutChanged>>
content_extent_changed: Option<Arc<LayoutChanged>>
```

The callback types are native Rust closures over erased `dyn Any` component storage:

```rust
type MapCommand =
    dyn Fn(&dyn Any, KeyStroke) -> Option<Box<dyn Any>>;

type HandleCommand =
    dyn for<'a> Fn(&mut dyn Any, Box<dyn Any>, &mut EventCx<'a>)
        -> InteractionResult;

type FocusChanged =
    dyn Fn(&mut dyn Any, bool);

type PasteHandler =
    dyn for<'paste, 'event> Fn(
        &mut dyn Any,
        &'paste str,
        &mut EventCx<'event>,
    ) -> InteractionResult;
```

`ComponentCx` exposes:

- `focusable()`;
- `modal_scope()`;
- `on_focus_changed`;
- `on_paste`;
- `key_commands`;
- `tick`.

The typed wrappers downcast component values and use `expect` on an internal type mismatch. These are native callbacks, not N-API callbacks.

### 2.4 Interaction result

`crates/iyon-tui/src/interaction/result.rs::InteractionResult` has only:

```rust
Ignored
Consumed
```

Routing continues through ancestors after `Ignored` and stops on `Consumed`.

### 2.5 Focus state contract

`crates/iyon-tui/src/interaction/focus.rs::FocusState` stores:

```rust
focused: Option<ComponentId>
focused_handler: Option<Arc<dyn Fn(&mut dyn Any, bool)>>
active_modal: Option<ComponentId>
modal_restore: Vec<(Option<ComponentId>, Option<ComponentId>)>
geometry: Option<ComponentGeometryMap>
```

Focus is host-owned semantic component state, not terminal focus state. Eligibility requires:

- component is in the mount graph;
- capability is `focusable`;
- component lies inside the active modal scope, if one exists;
- if geometry is available, the component has visible geometry.

`set_focus` notifies the old focused component with `false` and the new focused component with `true` (`focus.rs:248-280`). Notification failures caused by missing registry entries are discarded by `notify_focus_handler` (`focus.rs:328-335`).

### 2.6 Output contract

`crates/iyon-tui/src/output/handle.rs::Output<T>` is an opaque typed identity. It contains a private `OutputId` and is `Copy`, `Clone`, `Eq`, and `Hash`. It does not contain a callback.

`OutputQueue` stores erased events:

```rust
ErasedOutputEvent {
    output: OutputId,
    payload_type: TypeId,
    payload: Box<dyn Any>,
}
```

`EventCx::emit` pushes one typed payload into the queue (`output/event.rs:38-66`).

`OutputRouter::route` associates one output identity with one application-action mapping. A second route is rejected with `RouteConflict` (`output/router.rs:58-78`). On drain:

- no matching route: event is removed and silently ignored (`router.rs:87-90`);
- mismatched payload `TypeId`: explicit `OutputDispatchError::TypeMismatch` (`router.rs:92-94`);
- matching route: erased payload is downcast and mapped (`router.rs:96`).

### 2.7 TextInput contract

`crates/iyon-tui/src/controls/text_input/mod.rs::TextInput` stores:

```rust
buffer: TextBuffer
multiline: bool
focused: bool
submitted: Output<String>
change_outputs: ChangeOutputs
layout_size: Option<Size>
scroll_row: usize
border: Option<BorderSpec>
```

Important public methods:

- `new`;
- `multiline`;
- `border`;
- `set_multiline`;
- `text`;
- `is_empty`;
- `cursor_bytes`;
- `set_text`;
- `clear`;
- `submitted`;
- `output_on_change`.

`output_on_change` is a Rust API. It registers a typed projection over a borrowed `TextChange` and returns an `Output<R>` (`mod.rs:125-137`, `output.rs:5-73`).

User edits emit change outputs only when the editing command changes the buffer. Programmatic `set_text`, `clear`, and `set_multiline` do not emit user change events.

### 2.8 ScrollPane contract

`crates/iyon-tui/src/scroll.rs::ScrollPane` stores:

```rust
content: View
mode: ScrollMode
layout_size: Option<Size>
content_extent: Option<Size>
```

`ScrollMode` is:

```rust
FollowEnd
Detached { top_row: usize }
```

The pane is focusable and maps only unmodified:

```text
Up       → LineUp
Down     → LineDown
PageUp   → PageUp
PageDown → PageDown
Home     → Start
End      → End
```

This exact mapping is in `scroll_command.rs:13-24`.

The scroll component has no output channel. Its key handler always returns `Consumed`, even when the requested movement reaches a boundary (`scroll.rs:166-188`). This means a ScrollPane can prevent global fallback for those keys even if its visual position does not change.

### 2.9 TypeScript public event contract

`packages/iyon-tui/src/runtime/events.ts` exposes only:

```ts
OutputEvent {
  type: "output";
  routeId: string;
  payload?: string;
}

TerminateEvent {
  type: "terminate";
  reason?: string;
}

TuiEvent = OutputEvent | TerminateEvent;
```

`TuiRuntime.nextEvent()` therefore cannot deliver raw key, paste, focus, scroll, mouse, or resize events.

The TypeScript native contract (`transport/native/addon.ts:134-188`) exposes:

- `bindKey`;
- `route`;
- `interceptPaste`;
- `dispatchKey`;
- `dispatchPaste`;
- `forwardPaste`;
- `pollTerminal`;
- `nextWakeMs`;
- `nextOutput`;
- `waitForOutput`;
- `resize`;
- control factories.

There is no N-API event-subscription method and no TypeScript callback registration method for mounted components.

### 2.10 TypeScript `ComponentAdapter` discrepancy

`packages/iyon-tui/src/api/extensions/traits/component.ts` exports:

```ts
ComponentAdapter {
  view(...)
  capabilities?(...)
  onKey?(...)
  onPaste?(...)
  onTick?(...)
}
```

`AsyncComponentAdapter` wraps those methods in `Promise.resolve().then(...)`.

The repository search found no production caller that:

- accepts a `ComponentAdapter`;
- creates a native component from it;
- lowers its capabilities into `ComponentCapabilities`;
- stores its callbacks in the native mount graph;
- dispatches a native event into `AsyncComponentAdapter.key()` or `.paste()`.

The only references are the trait definition/export and the standalone TypeScript trait test. This is a public-looking but currently disconnected API surface. It should not be described as the active TS callback path.

---

## 3. Dependency and ownership map

### 3.1 Main ownership graph

```text
crossterm blocking reader thread
        │
        │ UnboundedSender<Result<crossterm::Event>>
        ▼
TermwizBackend.events
        │ try_next_event()
        ▼
private TerminalEvent
        │
        ▼
TuiHost.poll_terminal()
        │
        ▼
RunningApp::dispatch_key / dispatch_paste
        │
        ├── SceneHost::dispatch_key_local / dispatch_paste
        │       │
        │       ├── FocusState
        │       ├── MountGraph
        │       ├── MountedCapabilities
        │       ├── ComponentRegistry
        │       └── OutputQueue
        │
        ├── GlobalBindings
        ├── PasteInterceptors
        └── OutputRouter → Action queue
                              │
                              ▼
                       application update
                              │
                              ├── state/body mutation
                              ├── dirty flag
                              └── deferred paste/timer work
                                      │
                                      ▼
                            prepare/render/present frame
                                      │
                                      ▼
                            Termwiz worker / terminal bytes

OutputQueue / OutputRouter
        │
        ▼
HostState.outputs: VecDeque<RoutedOutput>
        │
        ▼
NativeTuiHost.nextOutput()/waitForOutput()
        │
        ▼
Tui.nextEvent()
        │
        ▼
TypeScript OutputEvent
```

### 3.2 Terminal backend ownership and lifetime

`TermwizBackend::enter`:

1. starts the termwiz terminal worker;
2. waits for terminal startup;
3. starts the crossterm `EventReader`;
4. retains the event receiver, event reader, terminal worker handle, and current size.

`EventReader` owns:

```rust
shutdown: Arc<AtomicBool>
worker: Option<JoinHandle<()>>
```

It polls crossterm every 50 ms and sends either an event or a read/poll error. `Drop` sets shutdown and joins the input thread (`terminal/crossterm/mod.rs:58-118`).

`TermwizBackend::restore` stops the input reader before sending `TerminalCommand::Restore` and joining the terminal worker (`terminal/termwiz/backend.rs:144-166`).

The input reader and terminal presentation worker are separate threads. Input does not borrow termwiz terminal state and terminal presentation does not run on the input thread.

### 3.3 Native control ownership

`HostTextInput` and `HostScrollPane` are cloneable handles over shared native state:

```rust
HostTextInput {
    state: Arc<Mutex<TextInput>>,
    component_id: Arc<Mutex<Option<u64>>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
}

HostScrollPane {
    state: Arc<Mutex<ScrollPane>>,
    component_id: Arc<Mutex<Option<u64>>>,
    host: Arc<Mutex<Option<Weak<Mutex<HostInner>>>>>,
}
```

They are constructed by `TuiHost::create_text_input` and `TuiHost::create_scroll_pane`, registered in `RunningApp` as `MountedTextInput`/`MountedScrollPane`, and assigned a native component ID (`application/host.rs:1151-1175`).

The public handle retains the control state. The mounted wrapper provides the `Component` implementation and native callbacks. The weak host reference allows programmatic mutation to invalidate and render the owning host without creating a strong host/control cycle.

Component disposal requests deferred retirement. `RunningApp::reap_retired_components` physically removes the registry entry only after a successfully reconciled mount graph proves the component is no longer mounted (`application/kernel.rs:113-142`).

### 3.4 Focus ownership

`FocusState` owns the current focus and modal restoration stack. `MountGraph` owns topology. `MountedCapabilities` owns the capability declarations and callback closures. `ComponentRegistry` owns the actual type-erased component values.

No terminal backend owns focus. No TypeScript object owns the native focused ID. The TextInput `focused` Boolean is updated only through the native focus callback.

### 3.5 Output ownership

The component owns the stable `Output<T>` identity. `OutputQueue` owns transient emitted payloads. `OutputRouter<Action>` owns the mapping from output identities to application actions. `RunningApp.actions` owns queued actions until the application update consumes them. `HostState.outputs` owns final caller-visible routed outputs for the native host wrapper.

The TypeScript `Output<T>` object is a facade around a native output resource. It does not own or execute a callback itself. TypeScript registers the route ID with the host; native Rust emits and maps the output.

---

## 4. Execution paths and state transitions

### 4.1 Real keyboard path

#### Step 1: terminal read

`terminal/crossterm/mod.rs::read_events` runs on the `iyon-terminal-input` thread:

```text
while !shutdown:
  crossterm::event::poll(50 ms)
  crossterm::event::read()
  send Result<Event> over unbounded channel
```

Read and poll errors are sent through the same channel and terminate the reader thread (`mod.rs:95-119`).

#### Step 2: backend normalization

`TermwizBackend::try_next_event` receives events with `try_recv`:

- empty channel → `Ok(None)`;
- disconnected channel → `Err("terminal input closed")`;
- event → `map_event`.

Resize updates `TermwizBackend.size` and produces `TerminalEvent::Resize`. Other events are delegated to `crossterm::map_event` (`termwiz/backend.rs:85-123`).

`crossterm::map_event` currently maps:

```text
Event::Key    → TerminalEvent::Key(KeyStroke), unless release
Event::Paste  → TerminalEvent::Paste(String)
Event::Resize → TerminalEvent::Resize
Event::FocusGained / FocusLost / Mouse → None
```

Unsupported events are silently discarded and the backend loops to inspect the next event.

#### Step 3: host pump

`TuiHost::poll_terminal`:

1. locks `HostInner`;
2. updates real-time clock;
3. consumes up to `INPUT_PUMP_BUDGET = 32` events;
4. stops early if `RunningApp` already has pending actions;
5. dispatches each event;
6. stops after dispatch if a routed action now exists;
7. calls `inner.advance_and_render()` once after the pump (`application/host.rs:99`, `1471-1507`).

The explicit stop-after-action behavior is consequential: later terminal keystrokes are not consumed before the caller has reduced the previously generated application action.

#### Step 4: local component routing

`RunningApp::dispatch_key`:

1. returns `Ignored` immediately if the app is exiting;
2. records previous focus;
3. invokes `SceneHost::dispatch_key_local`;
4. records next focus;
5. drains component outputs to application actions;
6. if local routing was ignored, checks the exact global binding;
7. if consumed, invalidates affected focus components and marks the app dirty.

The local route is:

```text
focused component
  → parent
  → grandparent
  → ...
```

If there is no focused component, the active modal is used as the starting point. The route stops at the modal boundary (`interaction/routing.rs:88-110`).

For every component in the chain:

1. locate mounted capabilities;
2. iterate ordered `key_commands`;
3. call the capability mapper through `ComponentRegistry::with_any`;
4. if mapping returns a command, call the handler through `with_any_mut`;
5. stop on `InteractionResult::Consumed`.

If the key is unmodified Tab and no local handler consumes it, `FocusState::focus_next` runs and may consume the key (`routing.rs:45-85`).

#### Step 5: TextInput editing

`MountedTextInput` declares:

```rust
cx.focusable();
cx.on_focus_changed(mounted_focus_changed);
cx.key_commands(mounted_command_for_key, mounted_handle_command);
cx.on_paste(mounted_paste);
cx.on_layout_changed(mounted_layout_changed);
```

`mounted_command_for_key` locks the shared `TextInput` and calls `TextInput::command_for_key`.

`controls/text_input/command.rs::command_for_key` recognizes:

- character insertion;
- Enter/submit;
- multiline newline variants;
- arrows and word movement;
- Home/End and control equivalents;
- Backspace/Delete and word deletion;
- kill-to-line-start;
- yank;
- Up/Down and control equivalents.

The command is returned only when `TextInput::can_execute` permits it.

`handle_command` then mutates the buffer. For a changed edit it:

1. calls `input.emit_change(cx)`;
2. returns `Consumed`.

For submit it:

1. emits `input.submitted` with a copied `String`;
2. returns `Consumed`;
3. does not clear the buffer or otherwise apply application submission policy.

The `TextInput` component owns editing semantics. The application decides what a `submitted` action means.

### 4.2 Real paste path

#### Bracketed paste activation

`crossterm::setup` enables raw mode and then executes `EnableBracketedPaste` (`terminal/crossterm/mod.rs:30-40`). On failure it attempts to disable raw mode and returns an error. Restore disables bracketed paste and raw mode, preserving the first error (`mod.rs:43-55`).

There is no corresponding enablement for:

- mouse capture;
- focus reporting;
- pointer motion;
- click reporting.

#### Paste interception order

`RunningApp::dispatch_paste` first calls:

```rust
scene_host.intercept_paste(text, |component, text| {
    paste_interceptors.action(component, text)
})
```

`SceneHost::intercept_paste` uses the same focused/modal ancestor routing chain as ordinary events.

The first registered matching interceptor wins. If one matches:

- its mapped Action is pushed;
- `Consumed` is returned;
- the TextInput paste handler is not called.

This permits a caller to intercept paste before editing, for example to turn pasted text into a product-defined action.

#### Ordinary paste route

If no interceptor matches, `SceneHost::dispatch_paste` invokes component paste handlers along the focused-to-ancestor route. `TextInput::paste_callback` calls `handle_paste`, which:

1. canonicalizes CRLF/CR to LF;
2. expands tabs to four spaces;
3. preserves newlines only in multiline mode;
4. inserts the resulting text at the current cursor;
5. repairs the viewport scroll row;
6. emits one or more registered change outputs;
7. returns `Consumed` if the buffer changed.

The existing test `mounted_text_input_paste_is_one_consumed_change_event` confirms that `"a\r\nb"` becomes `"a\nb"` and emits one routed change value (`controls/text_input/tests/mod.rs:92-116`).

If the focused component ignores paste, ancestors may consume it. A modal scope prevents routing to background components (`interaction/tests/mod.rs`, `controls/text_input/tests/mod.rs`, and `application/tests.rs:1636-1685`).

#### Deferred forwarding

`AppCx::forward_paste` pushes text into `deferred_pastes` and documents that it bypasses interceptors and is ordinary focused-component paste routing after the current update (`application/context.rs:205-209`).

After each application update, `RunningApp::advance_ready` drains:

1. output-generated actions;
2. deferred pastes;
3. newly due timers.

`drain_deferred_pastes` invokes ordinary component paste routing directly, deliberately bypassing the interceptor phase (`application/kernel.rs:758-770`).

The native host-level `forward_paste` method performs this through `host_forward_paste`, then advances/render the host (`application/host.rs:1382-1388`).

### 4.3 Focus path

Focus is established/reconciled while resolving a scene:

```text
resolved mount graph + capabilities + geometry
  → FocusState::reconcile_with_geometry
  → eligible focus order
  → active modal calculation
  → selected focused ComponentId
  → old focus callback(false)
  → new focus callback(true)
```

The initial preferred focus is:

- restored modal focus if valid;
- otherwise first eligible component;
- otherwise retain the currently focused eligible component;
- otherwise first eligible component.

Tab traversal calls `focus_next` only after local key handlers and only for an unmodified Tab. Shift-Tab is not automatically handled by the fallback focus traversal; it must be consumed by a component/global binding if desired.

When TextInput receives focus callback `true`, `TextInput.focused` becomes true. Its semantic view then uses `vf::text_with_cursor`; when false, it uses styled text without a cursor (`controls/text_input/presentation.rs:20-37`).

Focus callbacks do not directly emit application outputs. A focus transition causes the dispatch path to mark old/new focused components dirty when the key was consumed (`application/kernel.rs:530-545`).

There is no terminal focus event in this path. Terminal focus and component focus are separate concepts, and only the latter currently exists.

### 4.4 Scroll path

A ScrollPane is mounted as `MountedScrollPane`, which declares:

```rust
cx.focusable();
cx.on_layout_changed(Self::on_layout_changed);
cx.on_content_extent_changed(Self::on_content_extent_changed);
cx.key_commands(Self::map_command, Self::handle_command);
```

The pane’s state receives layout size and content extent from native layout callbacks. It tracks either end-following or a detached visual row.

A keyboard scroll operation is:

```text
TerminalEvent::Key(KeyStroke)
  → FocusState route
  → MountedScrollPane::map_command
  → ScrollPane::handle_command
  → mode/top_row mutation
  → Consumed
  → dirty host frame
  → new RowViewport view
  → rendering
```

The ScrollPane never receives mouse wheel or pointer events because those events are discarded before native routing.

Programmatic `HostScrollPane::follow_end` and `set_content` mutate shared state, invalidate the registered component, and immediately invoke `advance_and_render` (`application/host.rs:465-552`). The TypeScript `.followEnd()` method delegates to this native operation (`api/controls/scroll-pane.ts:221-226`).

### 4.5 Callback-induced update and rendering

A callback-induced update has two distinct forms.

#### Component callback emits an output

```text
component key/paste handler
  → EventCx::emit
  → SceneHost.outputs: OutputQueue
  → RunningApp::drain_outputs_to_actions
  → RunningApp.actions
  → RunningApp::advance_ready
  → application update callback
  → state/body mutation
  → dirty/body_dirty
  → frame preparation
  → terminal presentation
```

`RunningApp::dispatch_key` and `dispatch_paste` drain output events immediately after local component dispatch (`application/kernel.rs:523-571`). The resulting Actions are reduced in `advance_ready`.

Every successfully processed Action sets `dirty = true` and `body_dirty = true`, then drains any outputs and deferred pastes generated during that update (`kernel.rs:590-624`).

#### Component mutates native state directly

TextInput command handlers mutate `TextInput` state directly and mark the app dirty through the consumed interaction result. Programmatic TextInput operations call `render_host`, which:

1. resolves the weak host;
2. resolves the component ID;
3. calls `host_invalidate_component`;
4. calls `advance_and_render` (`application/host.rs:717-741`).

The native control does not call TypeScript or invoke a JavaScript callback during the edit. TypeScript observes the resulting state only through handle reads or receives an explicitly routed submitted/change output where such a route exists.

### 4.6 Native output to TypeScript

The native host defines:

```rust
HostOutput::Routed(RoutedOutput)
RoutedOutput {
    route_id: String,
    payload: Option<String>,
}
```

Host-level registrations create these values:

- `bind_key` maps a global key to `HostOutput::Routed` with no payload (`application/host.rs:1178-1186`);
- `route_text_input_output` maps a typed `Output<String>` to `HostOutput::Routed` with `payload: Some(text)` (`host.rs:1281-1297`);
- `intercept_paste` maps the interceptor Action to `HostOutput::Routed` with pasted text (`host.rs:1299-1318`).

The host application update stores routed outputs in `HostState.outputs: VecDeque<RoutedOutput>` (`host.rs:60-88`).

`NativeTuiHost.next_output` converts one native output to JSON:

```json
{
  "route_id": "...",
  "payload": "..."
}
```

`wait_for_output` runs the native interaction driver until output or exit, while terminal input, ticks, stream wakeups, and rendering remain native (`crates/iyon-tui-native/src/tui.rs:973-993`, `application/host.rs:1510-1536`).

`Tui.nextEvent()` maps that JSON to:

```ts
{
  type: "output",
  routeId: output.route_id,
  payload?: output.payload
}
```

If the host returns `null`, TypeScript produces `{ type: "terminate", reason: "closed" }` (`runtime/runtime.ts:347-359`).

---

## 5. Alternate routes and failure semantics

| Semantic operation | Production path | Alternate/test path | Selection/failure behavior |
|---|---|---|---|
| Physical key | crossterm reader → `TermwizBackend::try_next_event` → `TuiHost.poll_terminal` | `NativeTuiHost.dispatchKey`, TypeScript harness `pressKey`, Rust fake backend | Real route normalizes crossterm; direct dispatch bypasses physical decode |
| Physical paste | bracketed crossterm `Event::Paste` → `TerminalEvent::Paste` → native paste routing | `dispatchPaste`, harness `.paste()` | Unsupported/non-bracketed terminal input is not guaranteed to become a paste event |
| Resize | crossterm resize → backend size update → `TerminalEvent::Resize` → invalidate frame | `NativeTuiHost.resize`, harness resize event | Resize updates viewport; no application callback is emitted |
| Component key | focused-to-ancestor `route_key_local` | direct Rust `RunningApp::dispatch_key` | first consumed handler stops route |
| Global key | global binding after local route and Tab fallback | `bindKey`, `AppCx::bind_key` | exact `(KeyStroke)` match only; local component consumption wins |
| Paste interception | interceptor chain before component paste handlers | `interceptPaste`, `AppCx::intercept_paste` | first focused/modal-chain interceptor wins; background interceptors excluded |
| Ordinary paste | focused-to-ancestor paste handlers | `dispatchPaste`, `forwardPaste` | bubbles on `Ignored`; stops on `Consumed` |
| Deferred paste | `AppCx::forward_paste` queue → after current update → ordinary paste | host `forwardPaste` | bypasses interception deliberately |
| Text submit | TextInput Enter → `Output<String>` → route → Action → application update | `Tui.route(input.submitted(), id)` | no route means the emitted output is discarded when the queue drains |
| Text change | Rust `TextInput::output_on_change` → `Output<R>` → route | no equivalent TypeScript TextInput API | Rust-only in current public facade |
| Scroll | focused ScrollPane key commands | `.followEnd()`, `.setContent()` | no pointer/wheel path |
| Native output | `HostState.outputs` → `nextOutput`/`waitForOutput` | headless host queue | TypeScript sees only routed output/terminate |
| Pointer/mouse | no production route | no meaningful fake route found | crossterm mouse events explicitly dropped |

### Failure masking and explicit failures

#### Explicit failures

- crossterm raw mode/bracketed paste setup errors return errors and attempt cleanup (`crossterm/mod.rs:30-55`);
- input channel disconnection returns `terminal input closed` (`termwiz/backend.rs:118-121`);
- terminal worker command send failure returns typed `TerminalWorkerStopped` (`terminal/backend.rs:8-27`);
- output route conflicts return `RouteConflict`;
- output payload type mismatch returns `OutputDispatchError::TypeMismatch`;
- native key parsing rejects empty keys, multi-character character keys, and unknown modifiers (`iyon-tui-native/src/tui.rs:1043-1086`);
- invalid/dead native control handles return native errors through the N-API wrappers.

#### Deliberate ignored behavior

- unsupported crossterm events (`Mouse`, `FocusGained`, `FocusLost`) are silently discarded;
- key release events are dropped;
- a missing component capability skips that component;
- a missing registry component maps to `Ignored`;
- a missing route silently consumes/removes the queued output event;
- a no-op scroll command still returns `Consumed`;
- focus notification ignores a missing registry target;
- mounted TextInput/ScrollPane poisoned-lock paths often degrade to `Ignored` or spacer views;
- `TuiHost.next_output` returns `None` if the host lock cannot be acquired.

These choices are not all equivalent. Ignoring unsupported terminal feature events is a compatibility/feature absence behavior. Ignoring a missing mounted component or poisoned lock may mask an internal lifecycle defect. Missing output routes silently discard caller-significant events, which is particularly important because TypeScript has no queue inspection or output-route presence query.

### Terminal presentation failure

A callback-induced dirty frame eventually travels through:

```text
RunningApp frame preparation
  → PreparedSceneFrame
  → TermwizBackend::begin_frame
  → TerminalCommand::Present
  → TermwizPresenter::present
  → termwiz Terminal::render
  → terminal.flush
  → oneshot PresentReceipt
```

`TermwizPresenter::present` maintains a differential `presented` surface and a `known` flag. A failed render/flush clears the known state, attempts to terminate synchronized output, and returns the error (`presenter.rs:161-170`). The host retains its previous authoritative frame and discards the failed candidate rather than promoting it.

The terminal presentation path is separate from the routed output path. A routed output is reduced through the application update before the new frame is committed; it is not sent as terminal bytes.

---

## 6. Caches, invalidation, scheduling and performance

### Input pumping

`TuiHost::poll_terminal` uses a fixed `INPUT_PUMP_BUDGET` of 32 events (`application/host.rs:99`, `1474`). It stops as soon as an application action is pending. This prevents a buffered key burst from changing focus or mutating the composer repeatedly before the caller reduces the first action.

The test `buffered_terminal_input_is_serviced_between_action_batches` documents this behavior: input is serviced before a large action backlog fully drains, but not after every queued key without yielding to application updates (`application/tests.rs:1198-1250`).

### Event wait scheduling

Native `wait_for_output`:

1. checks exit;
2. refreshes the headless clock when needed;
3. calls `poll_terminal`;
4. checks `next_output`;
5. sleeps until the next deadline, capped at 16 ms.

TypeScript’s abortable `pollOutput` mirrors this:

```text
host.pollTerminal()
host.nextOutput()
wait(min(max(host.nextWakeMs(), 1), 16))
```

There is no separate input subscription or callback wake channel exposed to TypeScript. The wait is a polling/deadline driver around the native host.

### TextInput work

A user edit performs:

- command mapping;
- buffer mutation;
- optional Unicode/grapheme cursor work;
- change-output projection;
- native component invalidation;
- a later frame resolution/paint.

`TextInput` viewport scroll is maintained as a row index (`scroll_row`) and repaired after edits, cursor movement, multiline changes, and layout callbacks. Cursor display uses stored grapheme widths to avoid re-segmenting text inconsistently with paint (`controls/text_input/cursor.rs:1-88`).

### ScrollPane work

Scroll movement computes content height from either:

- cached native content extent; or
- `measure_view(content, width)`.

The pane retains only a visual row position, not semantic content snapshots. Follow-end mode computes `total - viewport`; detached mode clamps an explicit row.

A scroll key always causes a consumed interaction and host dirtying, even if movement is clamped to the same row. There is no output emission and no asynchronous scroll event.

### Output queue growth

`OutputQueue` is an unbounded `VecDeque`. `HostState.outputs` is also an unbounded `VecDeque<RoutedOutput>`. The normal event driver extracts one output as soon as available, but no hard queue capacity or backpressure contract is visible in the inspected input/output path.

### Frame coalescing

`RunningApp` coalesces state/action effects into a frame:

- component dispatch marks dirty;
- action updates mark dirty/body-dirty;
- multiple actions may be reduced before one frame;
- `TuiHost.poll_terminal` advances/renders after its bounded input pump;
- in-flight presentation protects the previous authoritative frame until receipt success.

The test `input_updates_state_while_presentation_is_in_flight` demonstrates that keys can continue to update application state while an older presentation is delayed; the updates are eventually observed in order and a later frame is presented (`application/tests.rs:1253-1322`).

### TypeScript/native transport volume

Raw keyboard and paste events do not cross N-API. Only:

- key registrations;
- paste registrations;
- direct key/paste injection methods;
- routed output records;
- state/control reads/writes

cross the native boundary. This is consistent with the framework boundary rule that TypeScript should not reimplement terminal key interpretation or native component routing.

---

## 7. Tests, benchmarks and observability

### 7.1 Decoder tests

`crates/iyon-tui/src/terminal/crossterm/key.rs:108-200` protects:

- BackTab canonicalization;
- release dropping;
- repeat acceptance;
- all supported modifiers;
- Enter versus Shift-Enter;
- newline/carriage-return character preservation;
- control-character preservation.

No test evidence was found for physical mouse, terminal focus gained/lost, click, wheel, hover, or pointer motion because those are not represented by `TerminalEvent`.

### 7.2 Native routing tests

`crates/iyon-tui/src/interaction/tests/mod.rs` includes:

- TextInput paste normalization and one change event;
- paste bubbling to ancestors;
- focused input preventing ancestor paste handling;
- key command output emission;
- modal paste containment;
- routing behavior through focus and mount graphs.

`crates/iyon-tui/src/controls/text_input/tests/mod.rs` covers the same component-level contracts, including routed change outputs and modal behavior.

### 7.3 Application-level tests

Important tests in `crates/iyon-tui/src/application/tests.rs`:

- `neutral_app_composes_input_output_action_timer_and_persistent_history` (`367-469`): TextInput registration, submitted output route, Enter dispatch, Action reduction, History mutation, and subsequent rendering.
- `headless_paste_dispatch_stays_on_the_local_component_path` (`472-498`): paste mutates native TextInput state and appears in the prepared frame.
- `paste_interceptor_follows_registration_not_mount_lifetime` (`500-568`): interceptor registration can outlive temporary mounting, but only mounted/focused routing receives ordinary paste.
- `buffered_terminal_input_is_serviced_between_action_batches` (`1198-1250`): bounded input pumping and action-batch fairness.
- `input_updates_state_while_presentation_is_in_flight` (`1253-1322`): input/action reduction proceeds while presentation receipt is delayed.
- `production_runtime_uses_backend_viewport_after_resize_event` (`1356-1392`): resize event invalidates and uses the backend-reported viewport.
- `init_forward_paste_routes_after_initial_mount_without_terminal_input` (`1394-1429`): deferred paste runs after initial mount and reaches a routed change action.
- `production_paste_interceptor_forwards_without_reinterception` (`1431-1478`): an intercepted paste can be forwarded from application update without recursively hitting the interceptor.
- `application_paste_interceptors_respect_active_modal_routing` (`1636-1685`): modal routing excludes background interceptors.
- `application_global_keys_preserve_local_traversal_and_binding_lifetime` (`1710-1808`): local component consumption wins over global binding; Tab traversal and binding lifetime are distinct.

### 7.4 TypeScript tests

`packages/iyon-tui/tests/tui_runtime.test.ts:6-31` verifies:

- TextInput creation and native local editing;
- `submitted()` output shape;
- route registration;
- Enter → native output → `Tui.nextEvent()` with `routeId` and payload;
- aborting a pending event wait;
- idempotent close.

`packages/iyon-tui/tests/tui_harness.test.ts:7-27` verifies the headless facade’s equivalent path, including `pressKey`, `route`, `nextEvent`, and deterministic time.

`packages/iyon-tui/tests/tui_traits.test.ts` verifies `AsyncComponentAdapter` in isolation. It does not prove that the adapter is mounted or called by the native runtime.

### 7.5 Observability gaps

No route counter or runtime diagnostic was found for:

- dropped mouse events;
- dropped terminal focus events;
- dropped key releases;
- missing output routes;
- output queue depth;
- count of native key/paste callbacks;
- focus transitions;
- component route-chain length;
- no-op ScrollPane commands;
- TypeScript `ComponentAdapter` invocation.

The source has extensive rendering/content/performance counters elsewhere, but the input path itself has limited direct observability. A visually updated screen or successful `nextEvent()` does not expose whether a raw event was ignored, routed, or dropped before routing.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Native-first input boundary is implemented

The source and current TypeScript contracts agree on the main boundary:

```text
terminal bytes/events and native key interpretation stay in Rust
TypeScript receives routed semantic outputs
```

Evidence:

- `TerminalEvent` is private (`terminal/backend.rs:30-40`);
- `NativeTuiHostContract` exposes only semantic dispatch and output methods (`transport/native/addon.ts:134-188`);
- `TuiRuntime.nextEvent` returns only output/terminate (`runtime/runtime.ts:347-359`);
- `application/host.rs` explicitly documents that terminal input, routing, ticks, wakeups, and rendering stay on the Rust side (`host.rs:1510-1513`).

### 8.2 TypeScript callback API is disconnected from native runtime

The exported `ComponentAdapter` describes an alternate architecture in which TypeScript callbacks handle `onKey`, `onPaste`, and `onTick`. That architecture is not wired into current production composition.

Current production callbacks are:

- Rust function pointers/closures in `ComponentCapabilities`;
- component values in a Rust `ComponentRegistry`;
- native `EventCx` output emission;
- native `InteractionResult`.

No production path was found from `ComponentAdapter` to `ComponentCapabilities`, and no production path was found from native routing back to `AsyncComponentAdapter.key()`/`.paste()`.

This is the most consequential TS callback contradiction in the scope: the public TypeScript type surface suggests callbacks can be authored in TypeScript, but the real runtime path only supports native Rust callbacks and TypeScript output consumption.

### 8.3 TypeScript TextInput API is narrower than Rust TextInput API

Rust TextInput exposes `output_on_change`, returning typed change outputs. The TypeScript `TextInput` contract exposes only:

```ts
submitted(): Output<string>
```

There is no TypeScript `outputOnChange`/`onChange` method. Consequently:

- native local editing occurs without a per-edit TypeScript callback;
- TypeScript can observe text by polling `input.text()`;
- TypeScript can receive submit output if `submitted()` is routed;
- TypeScript cannot currently register the Rust change-output projection through the normal facade.

### 8.4 Focus is virtual component focus, not terminal focus

The source uses focus for:

- selecting the component route start;
- enabling TextInput cursor presentation;
- style/focus state;
- Tab traversal;
- modal containment.

It does not map terminal focus gained/lost events. The same word “focus” therefore names two different potential concepts, but only component focus exists in the implementation.

### 8.5 Pointer support is not merely unexposed; it is dropped

Evidence is stronger than “no public pointer API”:

```rust
Event::FocusGained | Event::FocusLost | Event::Mouse(_) => None
```

in `terminal/crossterm/mod.rs:21-27`.

Additionally:

```rust
ProbeHints::new_from_env().mouse_reporting(Some(false))
```

in `terminal/termwiz/worker.rs:65-72`.

No `EnableMouseCapture`, `DisableMouseCapture`, `MouseEvent`, click, motion, wheel, hit-testing, hover, selection, or pointer-routing symbols were found in the inspected production crates. Pointer support is therefore absent from the current route.

### 8.6 Scroll is keyboard-only and component-owned

Scroll state is native and retained in `ScrollPane`. The route does not expose scroll events to TypeScript. TypeScript can call `followEnd` and replace content, but cannot subscribe to “scrolled” notifications or receive pointer wheel events.

### 8.7 Output routing is semantic but string route IDs are opaque

The native output route stores caller-supplied `route_id: String` and does not interpret it. This preserves genericity. The application receives a routed output record and decides its meaning.

The route identity is typed inside Rust until it crosses the native host boundary. At the TypeScript boundary, output payloads are currently represented as optional strings, so the N-API host-level path is narrower than the generic Rust `Output<T>` system.

### 8.8 Callback-induced frame updates are native and transactional

A consumed native interaction can modify:

- component state;
- focus state;
- ScrollPane mode;
- TextInput buffer;
- application state through routed Action;
- deferred paste queues.

The host then prepares a candidate frame and only promotes it after the terminal presentation receipt succeeds. This is stronger than a direct callback-to-JavaScript repaint loop: event callbacks do not themselves write terminal bytes or invoke TypeScript rendering.

---

## 9. Open questions and coverage gaps

1. Is `packages/iyon-tui/src/api/extensions/traits/component.ts` intentionally reserved for a future TS-component route, or is it stale public API that should be removed or narrowed?
2. If TypeScript component callbacks are intended, how are asynchronous callbacks supposed to reconcile with the current synchronous native `ComponentCapabilities` and `EventCx` borrowing model?
3. Should the TypeScript facade expose Rust `TextInput.output_on_change` semantics, or is submit-only output the intentional product boundary?
4. Should missing `OutputRouter` routes remain silent, or should the host expose diagnostics for dropped routed output events?
5. Should output queues have a documented capacity/backpressure policy?
6. Is terminal focus reporting intentionally unsupported, or is it expected to become a capability-driven feature later?
7. Is mouse/pointer support out of scope permanently, or merely deferred? The current code does not preserve discarded events for a later layer.
8. Should ScrollPane no-op commands return `Ignored` instead of `Consumed` so global key bindings can run at boundaries?
9. Focus callback failures currently can be ignored when registry access fails. Is this acceptable for lifecycle races, or should an internal inconsistency be surfaced?
10. The native `NativeTextInput::new` constructor still supports detached state, while the TypeScript facade intentionally requires Tui-owned host-bound creation. Is the detached N-API constructor retained for compatibility or test-only use?
11. TypeScript `Tui.nextEvent()` has no resize event even though native resize is decoded and rendered. Is resize intentionally represented only through `Tui.size`, or should it be observable as an event?
12. There is no raw terminal event subscription. If future pointer/focus support is added, should it remain native-to-output-only or introduce typed TypeScript events?
13. The direct TypeScript test harness injects key/paste through `dispatchKey`/`dispatchPaste`, bypassing crossterm decoding. A separate integration fixture would be needed to prove actual terminal protocol event bytes and crossterm mapping end-to-end.
14. No test currently proves that `FocusGained`, `FocusLost`, or `Mouse` are intentionally discarded; this behavior is established by source search rather than a route assertion.
15. No test currently proves there are no TypeScript callback consumers beyond the repository search. This is a source-reachability conclusion, not runtime evidence.

---

## 10. Evidence appendix

### 10.1 Contract and assignment evidence

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md` :: required headings, evidence discipline, source authority, framework boundary.
- `docs/architecture/atlas-4355c02/README.md` :: assignment 38 scope and repository baseline.
- `docs/architecture/atlas-4355c02/evidence/assignments.json:262-267` :: assignment 38 goal.
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt` :: tracked source manifest for relevant files.

### 10.2 Terminal decoding evidence

- `crates/iyon-tui/src/terminal/backend.rs:30-49` :: `TerminalEvent`, `TerminalBackend`, worker failure type.
- `crates/iyon-tui/src/terminal/crossterm/mod.rs:21-27` :: event mapping; explicit Mouse/focus dropping.
- `crates/iyon-tui/src/terminal/crossterm/mod.rs:30-55` :: raw mode and bracketed paste setup/restore.
- `crates/iyon-tui/src/terminal/crossterm/mod.rs:58-119` :: input thread lifetime and event channel.
- `crates/iyon-tui/src/terminal/crossterm/key.rs:7-45` :: key normalization.
- `crates/iyon-tui/src/terminal/crossterm/key.rs:48-105` :: modifier/media/modifier-key mapping.
- `crates/iyon-tui/src/terminal/crossterm/key.rs:108-200` :: decoder tests.
- `crates/iyon-tui/src/terminal/termwiz/backend.rs:20-91` :: backend ownership and resize mapping.
- `crates/iyon-tui/src/terminal/termwiz/backend.rs:109-167` :: event polling, presentation and restore.
- `crates/iyon-tui/src/terminal/termwiz/worker.rs:65-72` :: `mouse_reporting(Some(false))`.
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs:60-170` :: presentation, flush and failure behavior.
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs:184-212` :: terminal change generation.
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs:222-242` :: model-only scroll-region update versus real terminal output.

### 10.3 Interaction and focus evidence

- `crates/iyon-tui/src/interaction/key.rs:3-144` :: `Key`, `Modifiers`, `KeyStroke`.
- `crates/iyon-tui/src/interaction/result.rs:1-6` :: `InteractionResult`.
- `crates/iyon-tui/src/interaction/command.rs:13-23` :: callback type aliases.
- `crates/iyon-tui/src/interaction/command.rs:25-46` :: capability storage.
- `crates/iyon-tui/src/interaction/command.rs:67-198` :: `ComponentCx` registrations.
- `crates/iyon-tui/src/interaction/focus.rs:9-27` :: `FocusState` ownership.
- `crates/iyon-tui/src/interaction/focus.rs:44-90` :: focus reconciliation.
- `crates/iyon-tui/src/interaction/focus.rs:161-198` :: incremental reconciliation.
- `crates/iyon-tui/src/interaction/focus.rs:201-280` :: Tab traversal and focus callbacks.
- `crates/iyon-tui/src/interaction/focus.rs:299-335` :: eligible focus order and notification.
- `crates/iyon-tui/src/interaction/routing.rs:8-19` :: paste interceptor chain.
- `crates/iyon-tui/src/interaction/routing.rs:22-43` :: ordinary paste route.
- `crates/iyon-tui/src/interaction/routing.rs:45-85` :: key route, Tab fallback.
- `crates/iyon-tui/src/interaction/routing.rs:88-110` :: focused/modal ancestor routing chain.

### 10.4 Native controls and scroll evidence

- `crates/iyon-tui/src/controls/text_input/mod.rs:33-148` :: TextInput state and public operations.
- `crates/iyon-tui/src/controls/text_input/mod.rs:151-235` :: movement, command bridge and focus/paste callback bridge.
- `crates/iyon-tui/src/controls/text_input/mod.rs:250-307` :: viewport repair and component capabilities.
- `crates/iyon-tui/src/controls/text_input/command.rs:26-80` :: key-to-edit command mapping.
- `crates/iyon-tui/src/controls/text_input/command.rs:82-135` :: command mutation and output emission.
- `crates/iyon-tui/src/controls/text_input/output.rs:5-73` :: typed change outputs.
- `crates/iyon-tui/src/controls/text_input/edit.rs:5-20` :: text/paste canonicalization.
- `crates/iyon-tui/src/controls/text_input/presentation.rs:20-85` :: focused cursor and scroll-row presentation.
- `crates/iyon-tui/src/scroll.rs:11-43` :: ScrollPane state.
- `crates/iyon-tui/src/scroll.rs:56-128` :: FollowEnd/detached movement.
- `crates/iyon-tui/src/scroll.rs:143-220` :: layout/content callbacks and component key capabilities.
- `crates/iyon-tui/src/scroll_command.rs:3-26` :: exact scroll command mapping.
- `crates/iyon-tui/src/application/host.rs:188-209` :: HostTextInput/HostScrollPane shared ownership.
- `crates/iyon-tui/src/application/host.rs:465-552` :: HostScrollPane mutations and invalidation.
- `crates/iyon-tui/src/application/host.rs:555-606` :: MountedScrollPane capabilities and callbacks.
- `crates/iyon-tui/src/application/host.rs:629-749` :: HostTextInput operations, invalidation and retirement.
- `crates/iyon-tui/src/application/host.rs:751-816` :: MountedTextInput capabilities and callback bridge.
- `crates/iyon-tui/src/application/host.rs:1151-1175` :: native control creation and registration.

### 10.5 Application routing and update evidence

- `crates/iyon-tui/src/application/input.rs:5-28` :: global key bindings.
- `crates/iyon-tui/src/application/input.rs:31-71` :: paste interceptor ownership and lookup.
- `crates/iyon-tui/src/application/context.rs:121-133` :: typed route registration/removal.
- `crates/iyon-tui/src/application/context.rs:170-209` :: global bindings, paste interceptors, deferred paste.
- `crates/iyon-tui/src/application/kernel.rs:42-67` :: `RunningApp` ownership.
- `crates/iyon-tui/src/application/kernel.rs:523-571` :: key/paste dispatch, output draining, dirtying.
- `crates/iyon-tui/src/application/kernel.rs:574-627` :: action reduction, callback-induced update, timers and deferred paste.
- `crates/iyon-tui/src/application/kernel.rs:629-706` :: pending actions, deadlines, frame invalidation.
- `crates/iyon-tui/src/application/kernel.rs:758-818` :: deferred paste and output-to-action draining.
- `crates/iyon-tui/src/application/host.rs:40-88` :: `RoutedOutput`, `HostOutput`, `HostState`, application update.
- `crates/iyon-tui/src/application/host.rs:99-100` :: input pump budget.
- `crates/iyon-tui/src/application/host.rs:1178-1186` :: global key output construction.
- `crates/iyon-tui/src/application/host.rs:1262-1318` :: TextInput output route and paste interception.
- `crates/iyon-tui/src/application/host.rs:1364-1388` :: direct host key/paste/forward-paste methods.
- `crates/iyon-tui/src/application/host.rs:1411-1414` :: native routed-output extraction.
- `crates/iyon-tui/src/application/host.rs:1471-1507` :: real terminal polling and event dispatch.
- `crates/iyon-tui/src/application/host.rs:1510-1536` :: native output wait driver.

### 10.6 Output transport evidence

- `crates/iyon-tui/src/output/event.rs:8-68` :: erased output queue and `EventCx`.
- `crates/iyon-tui/src/output/handle.rs:5-68` :: typed opaque `Output<T>`.
- `crates/iyon-tui/src/output/router.rs:13-100` :: route registration, conflict, type check, drain.
- `crates/iyon-tui/src/output/mod.rs:1-18` :: boundary documentation and exports.

### 10.7 N-API and TypeScript evidence

- `crates/iyon-tui-native/src/tui.rs:450-600` :: NativeTextInput wrapper and stable submitted output.
- `crates/iyon-tui-native/src/tui.rs:603-1028` :: NativeTuiHost constructor and N-API methods.
- `crates/iyon-tui-native/src/tui.rs:908-993` :: bind/route/intercept/dispatch/output methods.
- `crates/iyon-tui-native/src/tui.rs:1043-1086` :: native key parser.
- `crates/iyon-tui-native/src/tui.rs:1574-1620` :: NativeScrollPane wrapper.
- `packages/iyon-tui/src/transport/native/addon.ts:14-16` :: native output contract.
- `packages/iyon-tui/src/transport/native/addon.ts:48-58` :: NativeTextInput contract.
- `packages/iyon-tui/src/transport/native/addon.ts:118-123` :: NativeScrollPane contract.
- `packages/iyon-tui/src/transport/native/addon.ts:134-188` :: NativeTuiHost contract.
- `packages/iyon-tui/src/runtime/events.ts:1-14` :: TypeScript event union.
- `packages/iyon-tui/src/runtime/runtime.ts:59-90` :: TuiRuntime public event/control surface.
- `packages/iyon-tui/src/runtime/runtime.ts:197-214` :: test/runtime event injection bridge.
- `packages/iyon-tui/src/runtime/runtime.ts:319-370` :: open, `nextEvent`, abortable output polling.
- `packages/iyon-tui/src/runtime/runtime.ts:633-714` :: TextInput, key, route and paste facade methods.
- `packages/iyon-tui/src/api/controls/text-input.ts:15-95` :: TypeScript TextInput and stable submitted output facade.
- `packages/iyon-tui/src/api/controls/scroll-pane.ts:24-40` :: TypeScript ScrollPane contract.
- `packages/iyon-tui/src/api/controls/scroll-pane.ts:73-118` :: native pane construction and capabilities metadata.
- `packages/iyon-tui/src/api/controls/scroll-pane.ts:221-246` :: follow-end and disposal.
- `packages/iyon-tui/src/api/extensions/traits/component.ts:8-53` :: disconnected TypeScript callback adapter API.
- `packages/iyon-tui/src/testing/index.ts:20-28` :: harness event/control interface.
- `packages/iyon-tui/src/testing/index.ts:88-100` :: test key/paste injection.

### 10.8 Behavioral test evidence

- `crates/iyon-tui/src/controls/text_input/tests/mod.rs:92-172` :: paste normalization and key-emitted outputs.
- `crates/iyon-tui/src/controls/text_input/tests/mod.rs:175-223` :: modal paste containment.
- `crates/iyon-tui/src/application/tests.rs:367-469` :: TextInput submit output → application Action → History/frame update.
- `crates/iyon-tui/src/application/tests.rs:472-498` :: local headless paste.
- `crates/iyon-tui/src/application/tests.rs:500-568` :: interceptor mount lifetime.
- `crates/iyon-tui/src/application/tests.rs:1198-1250` :: input pump/action backlog fairness.
- `crates/iyon-tui/src/application/tests.rs:1253-1322` :: input updates while presentation is in flight.
- `crates/iyon-tui/src/application/tests.rs:1356-1392` :: resize event and backend viewport.
- `crates/iyon-tui/src/application/tests.rs:1394-1478` :: forward-paste and interceptor bypass.
- `crates/iyon-tui/src/application/tests.rs:1636-1808` :: modal paste and local/global key precedence.
- `packages/iyon-tui/tests/tui_runtime.test.ts:6-31` :: TypeScript native editing, route, output event and cancellation.
- `packages/iyon-tui/tests/tui_harness.test.ts:7-27` :: headless native input/output.
- `packages/iyon-tui/tests/tui_traits.test.ts` :: standalone `AsyncComponentAdapter`; does not prove production mounting.

### 10.9 Inspected-file manifest

#### Rust terminal/input/output

```text
crates/iyon-tui/src/terminal/backend.rs
crates/iyon-tui/src/terminal/crossterm/key.rs
crates/iyon-tui/src/terminal/crossterm/mod.rs
crates/iyon-tui/src/terminal/mod.rs
crates/iyon-tui/src/terminal/termwiz/backend.rs
crates/iyon-tui/src/terminal/termwiz/lower.rs
crates/iyon-tui/src/terminal/termwiz/mod.rs
crates/iyon-tui/src/terminal/termwiz/presenter.rs
crates/iyon-tui/src/terminal/termwiz/worker.rs
crates/iyon-tui/src/interaction/command.rs
crates/iyon-tui/src/interaction/focus.rs
crates/iyon-tui/src/interaction/key.rs
crates/iyon-tui/src/interaction/mod.rs
crates/iyon-tui/src/interaction/result.rs
crates/iyon-tui/src/interaction/routing.rs
crates/iyon-tui/src/scroll.rs
crates/iyon-tui/src/scroll_command.rs
crates/iyon-tui/src/output/event.rs
crates/iyon-tui/src/output/handle.rs
crates/iyon-tui/src/output/mod.rs
crates/iyon-tui/src/output/router.rs
```

#### Rust controls/application/native

```text
crates/iyon-tui/src/controls/text_input/mod.rs
crates/iyon-tui/src/controls/text_input/command.rs
crates/iyon-tui/src/controls/text_input/cursor.rs
crates/iyon-tui/src/controls/text_input/edit.rs
crates/iyon-tui/src/controls/text_input/output.rs
crates/iyon-tui/src/controls/text_input/presentation.rs
crates/iyon-tui/src/application/context.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/application/input.rs
crates/iyon-tui/src/application/kernel.rs
crates/iyon-tui/src/application/run.rs
crates/iyon-tui-native/src/tui.rs
```

#### TypeScript facade/contracts/tests

```text
packages/iyon-tui/src/api/controls/output.ts
packages/iyon-tui/src/api/controls/scroll-pane.ts
packages/iyon-tui/src/api/controls/text-input.ts
packages/iyon-tui/src/api/extensions/traits/component.ts
packages/iyon-tui/src/runtime/events.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/testing/index.ts
packages/iyon-tui/src/transport/native/addon.ts
packages/iyon-tui/tests/tui_harness.test.ts
packages/iyon-tui/tests/tui_runtime.test.ts
packages/iyon-tui/tests/tui_traits.test.ts
```

#### Contract/context files

```text
AGENTS.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/evidence/assignments.json
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt
PRE-V5-ARCHITECTURE-REPORT.md
```

### 10.10 Files indexed but not relied on as primary evidence

The broader repository manifest, historical handoffs, V5 design documents, and parent-added architectural notes were searched for terminology and assignment context. They were not used to override current source behavior. No external source or web source was required.