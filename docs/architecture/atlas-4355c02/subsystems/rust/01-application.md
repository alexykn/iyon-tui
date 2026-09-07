# 01 — Application

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `01`, `rust/application`
- Primary scope: `crates/iyon-tui/src/application/`
- Goal: lifecycle, updates, scheduling, errors, and process/runtime ownership.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui/src/lib.rs`
- the application production modules and their in-module tests
- the principal native and TypeScript runtime seams that consume the application host:
  - `crates/iyon-tui-native/src/tui.rs`
  - `packages/iyon-tui/src/runtime/runtime.ts`
  - `packages/iyon-tui/src/runtime/wake-broker.ts`

The report describes current source reality only. It does not make V5 migration or deletion decisions.

### Scope boundary

The application directory contains two related but distinct layers:

1. **Generic Rust application kernel**
   - `App`
   - `AppCx`
   - `AppHandle`
   - `RunningApp`
   - action routing
   - timer scheduling
   - generic component/input integration
   - frame preparation hooks

2. **Native-host runtime integration**
   - `TuiHost`
   - `HostInner`
   - `TuiEnvironment`
   - native content Sources, Ports, and Connectors
   - retained ViewState wrappers
   - asynchronous terminal presentation and transactional frame commits

The generic Rust application kernel is not exposed as a supported public Rust authoring API. `crates/iyon-tui/src/lib.rs:13` declares `application` as `pub(crate)`, and `App`/`AppCx` are re-exported only inside the crate at `lib.rs:52-57`. The native host is exposed through the in-tree N-API crate rather than directly through the Rust crate’s public API.

### Facts, inferences, and unknowns

- **Fact:** `App` has no terminal or executor dependency (`application/app.rs:7-10`).
- **Fact:** The application directory itself contains no production Rust event-loop function. `run.rs` only contains blocking presentation-receipt and deadline helpers (`application/run.rs:5-21`).
- **Fact:** The reusable event loop is test-only in this crate (`application/tests/driver.rs:1-4`).
- **Fact:** `TuiHost` owns the native runtime and provides its own synchronous/asynchronous host APIs (`application/host.rs:949-1403`, `1510-1536`).
- **Fact:** TypeScript normally drives shared-environment wake delivery through `EnvironmentWakeBroker`, which calls native `flushPendingHosts` (`packages/iyon-tui/src/runtime/wake-broker.ts:121-175`, `307-338`).
- **Observation/inference:** `TuiHost::wait_for_output` itself does not call `TuiEnvironment::drain_pending_for`; it polls only the host’s terminal backend and local runtime (`application/host.rs:1513-1535`). Therefore source/content wakes depend on the external environment wake broker when accessed through the normal TypeScript runtime. The native host’s standalone `wait_for_output` path does not independently prove that shared-environment pending hosts are drained.
- **Observation/inference:** `HostHistory` retains an `Arc<Mutex<HostInner>>` (`application/host.rs:818-822`), while `TuiHost::Drop` closes only when the host Arc has one strong owner (`application/host.rs:1662-1667`). If a `HostHistory` outlives the final `TuiHost`, `HostInner::Drop` disposes content and unregisters the environment host but does not explicitly invoke terminal restoration (`application/host.rs:175-185`). Whether the `TermwizBackend` destructor independently restores the terminal is outside this assignment and remains an integration question.
- **Not executed:** No build, test, benchmark, or runtime suite was run during this investigation. Test code was inspected as behavioral evidence only.

### Approximate LOC methodology

Counts below are physical line spans, including comments and blank lines, and are therefore approximate rather than code-only LOC.

| Area | Approximate physical lines | Basis |
|---|---:|---|
| Generic/native application production modules | ~13,500 | Sum of production portions before each `#[cfg(test)]` module or file end |
| `application/tests.rs` | ~1,840 | File span from source inspection |
| `application/tests/driver.rs` | ~477 | File span from source inspection |
| `host.rs` in-module tests | ~1,600 | Begins around line 2,429 and continues to approximately line 4,011 |
| `content.rs` in-module tests | ~5,000+ | Test module begins at line 7,011 and continues beyond line 10,000 |
| `source_store.rs` in-module tests | ~650+ | Test module begins at line 1,672 and continues to approximately line 2,330 |

The application directory is therefore approximately 25,000 physical lines including substantial content/source-store and native-host behavioral tests. The largest production files are `content.rs`, `host.rs`, and `source_store.rs`; the generic lifecycle kernel itself is comparatively small.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| File | Production responsibility | Test responsibility | Feature/visibility |
|---|---|---|---|
| `application/mod.rs` | Module declaration and narrow re-exports | Enables application tests and test driver | `App`, `AppCx` are crate-visible; host/content/environment/view-state are `native-host` |
| `application/app.rs` | Declarative application definition and startup handoff | Covered through `tests.rs` | `App` is public inside the module but not public from the crate |
| `application/context.rs` | Borrow-scoped capabilities available to `init` and `update` | Behavioral use throughout tests | `AppCx` is public inside the crate, not public crate API |
| `application/kernel.rs` | Running state machine, actions, updates, timers, component dispatch, frame preparation | Core lifecycle tests use `RunningApp` directly | Internal |
| `application/handle.rs` | Cloneable bounded action ingress and send errors | Channel capacity, backpressure, closure tests | `AppHandle` re-exported only under tests from crate root |
| `application/timer.rs` | One-shot timer identity, queue, cancellation, deadline lookup | Ordering/cancellation/overflow-adjacent semantics | `TimerHandle` is public in module; queue internal |
| `application/input.rs` | Global key bindings and component-scoped paste interceptors | Routing precedence/lifetime tests | Internal |
| `application/run.rs` | Blocking receipt wait and async deadline wait helpers | Used by test driver and host shutdown | Internal |
| `application/tests/driver.rs` | Reusable fake-backend event loop | Only compiled as test module | Test-only |
| `application/tests.rs` | Generic application/kernel and production-driver behavioral suite | Main application test suite | Test-only |
| `application/environment.rs` | Shared native environment identity, pending-host queue, wake latch, fair drain, retry/error bookkeeping | Native environment fairness and retry behavior is also exercised from host/content tests | `native-host`; `TuiEnvironment` public in module |
| `application/host.rs` | Native host ownership, backend/session lifecycle, frame candidates, receipts, commit/rollback, native controls, history bridge | Extensive native-host lifecycle/failure tests | `native-host`; module itself is crate-private |
| `application/view_state.rs` | Host-owned retained state wrappers and host-lock serialization | State wrapper behavior covered by host/native tests | `native-host` |
| `application/content.rs` | Environment Sources, host Ports/Connectors, content scheduling, projections, caches, source wake lifecycle | Extensive Source/Connector/cache/rollback tests | `native-host` |
| `application/source_store.rs` | Persistent chunk/annotation storage, append/seal/truncate semantics | Persistent storage and annotation-index tests | `native-host` |

### 1.2 Primary responsibilities

The application directory owns the following framework responsibilities:

- application state and callback lifecycle;
- action ingress and bounded queueing;
- deterministic one-shot timers;
- component output-to-action routing;
- global key and paste routing;
- generic update batching;
- frame dirtiness and deferred body derivation;
- native host lifetime and terminal backend ownership;
- shared native environment wake scheduling;
- transactional desired/visible frame promotion;
- host-owned retained-state and content-resource lifecycle;
- native-host error reporting and retry state.

It does **not** own product/application meaning. The callbacks receive caller-defined `State`, `Action`, and `View` values. `HostOutput` and `RoutedOutput` are generic routing wrappers, not Iyon-specific application concepts (`application/host.rs:40-45`, `59-89`).

### 1.3 Secondary responsibilities that have accumulated in the host

`host.rs` also contains native wrappers for:

- `HostTextInput`;
- `HostViewSlot`;
- `HostScrollPane`;
- `HostHistory`;
- content port/connector integration;
- ViewState integration;
- test-only failure injection;
- readback and terminal inspection.

These wrappers all ultimately route mutations through `HostInner` and the same retained runtime. For example:

- `HostViewSlot::set_view` updates slot state, increments its revision, and calls `invalidate_host` (`host.rs:239-253`);
- `HostTextInput::set_text`, `clear`, `set_border`, and `set_multiline` mutate the control under its own mutex and synchronously request host rendering (`host.rs:647-669`, `717-741`);
- `HostScrollPane::set_content` and `follow_end` mutate the control and synchronously request host rendering (`host.rs:475-489`);
- `HostHistory::push`, `freeze`, and `discard_live` update history and then invoke host rendering (`host.rs:848-935`).

This makes `HostInner` the practical process/runtime ownership boundary even though several semantic subsystems remain in their own modules.

---

## 2. Types, APIs and contracts

### 2.1 `App<State, Action, Error, Init, Update, ViewFn>`

`App` is a generic standalone application definition (`application/app.rs:7-20`).

Stored fields:

```text
init: Init
update: Update
view: ViewFn
history: Option<History>
theme: Theme
handle: AppHandle<Action>
ingress: Option<Receiver<Action>>
marker: PhantomData<fn(State, Action) -> Error>
```

The callback contracts are:

```rust
Init: FnOnce(&mut AppCx<'_, Action>) -> Result<State, Error>
Update: FnMut(&mut State, Action, &mut AppCx<'_, Action>) -> Result<(), Error>
ViewFn: Fn(&State) -> View
```

`App::new` creates the bounded action ingress channel immediately (`app.rs:22-42`). The default action ingress capacity is 1,024 (`handle.rs:3`, `24-26`).

`App` configuration methods:

- `handle()` clones the action producer (`app.rs:44-47`);
- `with_theme(theme)` replaces the initial theme (`app.rs:49-54`);
- `with_history(history)` installs one persistent root History (`app.rs:56-61`);
- `start(now)` consumes the definition and creates `RunningApp` (`app.rs:63-73`).

`App` itself does not create a terminal backend, spawn a task, or run an event loop.

### 2.2 `AppCx<'a, Action>`

`AppCx` is the borrow-scoped capability object passed to `init` and every `update` (`context.rs:18-45`).

It lends mutable access to:

- `Scene`;
- `ComponentRegistry`;
- `OutputRouter<Action>`;
- `TimerQueue<Action>`;
- shared `Theme`;
- global key bindings;
- paste interceptors;
- deferred paste queue;
- exit flag;
- `AppHandle<Action>`.

The important public-in-module methods are:

#### Component registration/access

- `register(component) -> ComponentHandle<C>` (`context.rs:76-82`);
- `with_component(handle, closure) -> Option<R>` (`context.rs:84-94`);
- `with_component_mut(handle, closure) -> Option<R>` (`context.rs:96-106`);
- `remove_component(handle) -> Option<C>` (`context.rs:108-119`).

Component removal also removes the associated paste interceptor if the component was present (`context.rs:114-117`).

#### Output routing

- `route(Output<T>, map) -> Result<(), RouteConflict>` (`context.rs:121-128`);
- `remove_route(Output<T>) -> bool` (`context.rs:130-133`).

Output routing is typed at registration time and is later converted into application actions by `RunningApp::drain_outputs_to_actions`.

#### History/theme

- `history() -> Option<&History>` (`context.rs:135-140`);
- `history_mut() -> Option<&mut History>` (`context.rs:142-145`);
- `theme() -> &Theme` (`context.rs:147-151`);
- `theme_mut() -> &mut Theme` (`context.rs:153-156`).

The theme is stored behind `Arc<Theme>` and uses copy-on-write. A read-only update does not clone the theme; mutation through `theme_mut` clones only when shared. The test `app_theme_cow_is_deferred_until_theme_mut` verifies this (`application/tests.rs:114-152`).

#### Input and timers

- `bind_key(key, factory)` (`context.rs:170-176`);
- `unbind_key(key) -> bool` (`context.rs:178-181`);
- `intercept_paste(component, map)` (`context.rs:183-195`);
- `remove_paste_interceptor(component) -> bool` (`context.rs:197-203`);
- `forward_paste(text)` (`context.rs:205-209`);
- `schedule_after(delay, action) -> TimerHandle` (`context.rs:211-214`);
- `cancel_timer(handle) -> bool` (`context.rs:216-220`).

`forward_paste` deliberately bypasses paste interceptors on the forwarded pass. It places text on a deferred queue so the current callback returns before ordinary focused-component paste routing occurs.

#### Lifecycle

- `exit()` sets the exit flag and returns; shutdown happens after the current update (`context.rs:222-225`).

### 2.3 `AppHandle<Action>`

`AppHandle` is a cloneable producer for application actions (`handle.rs:5-13`).

Contracts:

- `send(action)` is synchronous and nonblocking (`handle.rs:29-35`);
- `send_async(action)` waits asynchronously for capacity (`handle.rs:38-47`);
- the queue is bounded to 1,024 actions;
- a full queue returns the original action as `AppSendError::Full`;
- a closed queue returns the original action as `AppSendError::Closed`;
- `AppSendError::action()` borrows the undelivered action;
- `AppSendError::into_inner()` recovers it (`handle.rs:50-74`);
- `AppClosed` provides equivalent recovery for the async path (`handle.rs:102-134`).

The type is conditionally thread-capable through its underlying Tokio sender. The test asserts `AppHandle<String>: Send + Sync`, while the application itself can still use non-`Send` State and Action on a local runtime (`application/tests.rs:1590-1618`).

### 2.4 `RunningApp`

`RunningApp` is the internal live application kernel (`kernel.rs:42-68`).

Owned state includes:

```text
state: State
scene: Scene
theme: Arc<Theme>
components: ComponentRegistry
outputs: OutputRouter<Action>
scene_host: SceneHost
actions: VecDeque<Action>
timers: TimerQueue<Action>
global_bindings: GlobalBindings<Action>
paste_interceptors: PasteInterceptors<Action>
pending_component_retirements: Vec<u64>
deferred_pastes: VecDeque<String>
ingress: Option<Receiver<Action>>
handle: AppHandle<Action>
update: Update
view: ViewFn
dirty: bool
body_dirty: bool
exit_requested: bool
```

The kernel’s most important invariant is that the semantic `view` callback is not run for every event. Updates mark `body_dirty`, and the body callback runs once at the next successful frame preparation (`kernel.rs:679-685`). Component ticks can dirty the frame without causing body derivation (`kernel.rs:585-588`, `application/tests.rs:823-845`).

### 2.5 `ReadyStatus`

`ReadyStatus` reports the result of one `advance_ready` pass (`kernel.rs:35-40`):

- `dirty`: the current logical frame needs preparation/presentation;
- `exiting`: application exit has been requested;
- `more_ready`: action work remains after the bounded update batch.

`more_ready` is suppressed after exit (`kernel.rs:787-792`).

### 2.6 `KernelError<Error>`

The generic kernel distinguishes:

```rust
Application(Error)
Output(OutputDispatchError)
```

(`kernel.rs:29-33`).

The test driver maps these into:

```text
RunError::Application(application error)
RunError::Runtime(runtime error)
```

(`application/tests/driver.rs:20-50`, `472-476`).

There is no generic kernel variant for terminal or frame errors because the generic kernel does not own a backend. Those errors are introduced by the native host layer as `HostAttemptError`/`HostFrameError`.

### 2.7 `TimerHandle` and `TimerQueue`

`TimerHandle` contains a process-global monotonically allocated ID (`timer.rs:6-12`). Each `TimerEntry` stores:

```text
handle
deadline: Instant
sequence: u64
action
```

(`timer.rs:14-19`).

`TimerQueue` is a vector-backed queue (`timer.rs:21-24`):

- `schedule(now, delay, action)` computes a checked deadline and appends an entry (`timer.rs:35-63`);
- `cancel(handle)` linearly searches and uses `swap_remove` (`timer.rs:65-71`);
- `pop_due(now)` scans all entries and selects the minimum `(deadline, sequence)` among due timers (`timer.rs:73-82`);
- `next_deadline()` scans for the minimum deadline (`timer.rs:84-86`);
- `clear()` drops all pending timers (`timer.rs:88-91`).

Timer IDs, sequence IDs, and deadline arithmetic are checked. Exhaustion panics rather than returning a recoverable error (`timer.rs:42-55`). This is treated as an impossible identity/resource-exhaustion invariant, not as ordinary application failure.

### 2.8 `GlobalBindings` and `PasteInterceptors`

`GlobalBindings<Action>` is a `HashMap<KeyStroke, Box<dyn Fn() -> Action>>` (`input.rs:5-7`).

- Binding the same key replaces the prior factory.
- `unbind` returns whether a binding existed.
- `action` invokes the stored factory on demand (`input.rs:17-29`).

`PasteInterceptors<Action>` is keyed by raw `ComponentId` (`input.rs:31-33`).

- Generic registration uses a typed `ComponentHandle`.
- Removal has both typed and raw-ID variants.
- Raw-ID removal exists for deferred component retirement (`input.rs:54-65`).
- Lookup maps the text into an action (`input.rs:67-71`).

### 2.9 Native host epoch types

`HostEpochs` tracks the host’s desired/visible and pending/committed state (`environment.rs:53-62`):

```text
host_id
desired_structural_revision
visible_structural_revision
visible_frame_revision
pending_epoch
committed_epoch
```

`HostFrameError` carries:

```text
host_id
attempted_epoch
desired_revision
phase
code
retryable
diagnostic
```

(`environment.rs:64-73`).

`HostDrainReport` contains:

```text
rearm
waiting_for_presentation
attempted
commits
errors
wake_epoch
```

(`environment.rs:82-90`).

These types are the structured bridge from native Rust frame processing into TypeScript runtime error channels.

### 2.10 `TuiHost` and host-owned native controls

`TuiHost` is a cloneable wrapper around `Arc<Mutex<HostInner>>` (`host.rs:949-959`). `HostInner` owns:

- the `RunningApp`;
- the selected backend;
- the authoritative last committed frame;
- an optional candidate frame;
- asynchronous presentation receipt;
- candidate state/content commit plans;
- desired/visible epoch state;
- content and ViewState registries;
- the shared `TuiEnvironment`;
- the host identity;
- failure and recovery markers (`host.rs:122-173`).

The host exposes:

- opening and environment sharing (`host.rs:961-1045`);
- desired root installation (`host.rs:1091-1120`);
- explicit host/environment barriers (`host.rs:1122-1149`);
- native controls (`host.rs:1151-1175`);
- generic routing (`host.rs:1178-1318`);
- rendering/theme/history/input/resize (`host.rs:1320-1403`);
- runtime polling/output/readback/close (`host.rs:1411-1667`).

The native module forwards these through N-API, including:

- `flushPendingHosts` (`crates/iyon-tui-native/src/tui.rs:683-729`);
- `pollTerminal` (`tui.rs:965-970`);
- `waitForOutput` (`tui.rs:981-993`).

---

## 3. Dependency and ownership map

### 3.1 Generic application dependency direction

```text
App::new(...)
    │
    ├── AppHandle<Action> + bounded Tokio ingress
    ├── Init callback
    ├── Update callback
    └── View callback
          │
          ▼
RunningApp::new
    │
    ├── Scene / History
    ├── ComponentRegistry
    ├── OutputRouter
    ├── TimerQueue
    ├── GlobalBindings
    ├── PasteInterceptors
    └── initial View derivation
          │
          ▼
external AppHandle / terminal / component tick
          │
          ▼
action queue
          │
          ▼
RunningApp::advance_ready
    │
    ├── due timers
    ├── SceneHost component ticks
    ├── component outputs → OutputRouter → actions
    ├── up to 128 Update callbacks
    ├── deferred paste routing
    └── dirty/body_dirty
          │
          ▼
RunningApp::prepare_frame
          │
          ├── ViewFn if body_dirty
          ├── SceneHost render/reconcile/layout/paint
          └── PreparedSceneFrame
```

### 3.2 Native host ownership graph

```text
TuiHost
  └── Arc<Mutex<HostInner>>
        ├── RunningApp<HostState, HostOutput, ...>
        │     ├── Scene
        │     ├── ComponentRegistry
        │     ├── OutputRouter
        │     ├── TimerQueue
        │     └── HostState.outputs
        ├── HostBackend
        │     ├── Headless(HeadlessSink)
        │     └── Real(TermwizBackend)
        ├── authoritative frame
        ├── optional candidate frame
        ├── optional PresentReceipt
        ├── ViewStateRegistry
        ├── ContentHostRegistry
        └── TuiEnvironment
              ├── host weak registry
              ├── pending host queue
              ├── retry-blocked set
              ├── waiting-for-presentation set
              └── environment content Sources
```

### 3.3 Host-handle lifetime

| Object | Owns/retains | Does not own |
|---|---|---|
| `TuiHost` | Strong Arc to `HostInner` | No separate runtime task |
| `HostHistory` | Strong Arc to `HostInner` (`host.rs:818-822`) | No independent History storage |
| `HostTextInput` | Its own `Arc<Mutex<TextInput>>`; weak host reference | No strong host ownership |
| `HostViewSlot` | Its own slot state; weak host reference | No strong host ownership |
| `HostScrollPane` | Its own pane state; weak host reference | No strong host ownership |
| `HostViewState` | Immutable state ID; weak host reference (`view_state.rs:23-35`) | No state registry ownership |
| `HostContentPort` | Port identity/state and weak host association | No independent host lifetime |
| `HostContentConnector` | Connector identity/state and weak host association | No independent host lifetime |
| `HostInner` | All runtime registries, backend, frame, and environment membership | Does not outlive all strong handles |

`HostInner::Drop` performs content disposal and environment unregistering (`host.rs:175-185`). It does not itself set `closed`, flush receipts, or explicitly restore a real terminal backend.

### 3.4 Component ownership and deferred retirement

`RunningApp` owns the `ComponentRegistry`. Host-created controls are registered as mounted wrapper components:

- `MountedTextInput`;
- `MountedViewSlot`;
- `MountedScrollPane`.

The wrapper handle’s raw ID is stored back into the public host control (`host.rs:1151-1175`).

Retirement is deferred:

1. Public control calls `retire`.
2. `RunningApp::host_retire_component` appends the raw ID to `pending_component_retirements` (`kernel.rs:113-121`).
3. `reap_retired_components` checks the last successfully reconciled `SceneHost` mount graph.
4. The registry entry and paste interceptor are removed only after the component is no longer mounted (`kernel.rs:123-143`).

This prevents a failed candidate frame from destroying a component still referenced by the last authoritative scene.

### 3.5 Environment ownership

One `TuiEnvironment` is shared by all hosts opened through `open_in_environment` (`host.rs:966-973`). It owns:

- process-local environment identity;
- source registry;
- host ID → weak host mapping;
- pending host queue;
- pending/queued/retry-blocked/waiting sets;
- edge-trigger wake latch;
- wake epoch.

Hosts own their own frame/runtime state; the environment owns only cross-host pending scheduling and source registry state. `TuiEnvironment` is marked `Send + Sync` with unsafe implementations, justified by mutex serialization at this boundary (`environment.rs:121-135`). This is an explicit safety assumption: callbacks and non-`Send` component state remain behind the host lock.

---

## 4. Execution paths and state transitions

## 4.1 Generic application startup

`App::start(now)` delegates to `RunningApp::new` (`app.rs:63-73`).

`RunningApp::new`:

1. Destructures the `App`.
2. Creates a `Scene` with either:
   - caller-supplied History plus a spacer body, or
   - no History plus a spacer body.
3. Creates:
   - `ComponentRegistry`;
   - `OutputRouter`;
   - `TimerQueue`;
   - key bindings;
   - paste interceptors;
   - deferred paste queue;
   - exit flag;
   - shared `Arc<Theme>`.
4. Constructs `AppCx`.
5. Runs `init` exactly once.
6. Stores the returned State and all runtime machinery.
7. Derives the initial body by invoking `ViewFn` once.
8. Installs that body in the Scene.
9. Closes ingress immediately if `init` requested exit (`kernel.rs:443-520`).

Initialization errors become `KernelError::Application` and prevent a `RunningApp` from being created (`application/tests.rs:1006-1041`).

The initial `dirty` flag is true, while `body_dirty` starts false because the initial body has already been derived (`kernel.rs:493-520`).

## 4.2 Action ingress path

There are two ingress stages:

```text
AppHandle::send/send_async
    ▼
bounded Tokio Receiver<Action>
    ▼
RunningApp::collect_external / collect_external_pending
    ▼
VecDeque<Action>
    ▼
RunningApp::advance_ready
    ▼
Update(State, Action, AppCx)
```

The generic kernel does not continuously read the receiver itself. Its driver must call:

- `collect_external_pending()` to drain up to 128 actions;
- `collect_external(first)` to enqueue one received action and up to 127 more (`kernel.rs:729-756`).

The production TypeScript runtime does not use `AppHandle`; it drives the native `TuiHost` API and native wake broker instead.

## 4.3 Update scheduling

`RunningApp::advance_ready(now)` (`kernel.rs:574-627`) proceeds as follows:

1. If exiting:
   - close ingress;
   - clear queued actions;
   - clear timers;
   - return status.
2. Move all currently due timers to the **front** of the action queue.
3. Run due component ticks through `SceneHost::tick_due`.
4. OR tick dirtiness into `self.dirty`.
5. Drain component outputs into actions.
6. Process at most `ACTION_BATCH_BUDGET = 128` actions.
7. For each action:
   - create a fresh `AppCx`;
   - call the user `update`;
   - map update failure to `KernelError::Application`;
   - set `dirty = true`;
   - set `body_dirty = true`;
   - drain outputs;
   - drain deferred pastes;
   - append newly due timers to the **back** of the queue;
   - if exit was requested, close ingress and clear remaining actions/timers.
8. Return `ReadyStatus`.

This produces several deliberate scheduling rules:

- due timers get a fair turn before an already buffered external action backlog (`tests.rs:652-683`);
- a zero-duration timer scheduled from inside an update is not recursively executed before that update returns (`tests.rs:614-650`);
- self-rescheduling zero-duration work is bounded to 128 updates per pass (`tests.rs:685-723`);
- the view callback is not re-derived for every action; one next frame consumes the coalesced `body_dirty` state (`tests.rs:570-612`);
- exit discards later queued actions (`tests.rs:948-978`).

## 4.4 Component tick path

Component capabilities can register periodic ticks through `ComponentCx`. `SceneHost::tick_due` invokes them before application actions (`kernel.rs:585-588`).

A tick can:

- emit a typed component output;
- request a redraw by returning `true`;
- do neither and return `false`.

The output path is:

```text
SceneHost::tick_due
    ▼
EventCx::emit(Output<T>, value)
    ▼
OutputRouter
    ▼
RunningApp::drain_outputs_to_actions
    ▼
application action queue
```

A redraw-only tick marks the frame dirty but does not mark `body_dirty`, so `ViewFn` is not called again (`tests.rs:805-845`).

## 4.5 Input path

### Key dispatch

`RunningApp::dispatch_key` (`kernel.rs:523-547`):

1. Ignore the key if exiting.
2. Capture previous focus.
3. Let `SceneHost` route the key through focused/ancestor/component handlers.
4. Capture next focus.
5. Drain component outputs.
6. If the result is `Ignored`, consult the exact-key global binding.
7. If a global binding exists, enqueue its action and return `Consumed`.
8. If a component consumed the key:
   - invalidate previous and newly focused components as needed;
   - mark the frame dirty.
9. Return `InteractionResult`.

Global bindings therefore run only after local component routing and framework focus traversal. This is tested for replacement, unbinding, and traversal behavior (`tests.rs:1711-1808`).

### Paste dispatch

`RunningApp::dispatch_paste` (`kernel.rs:549-572`):

1. Ignore if exiting.
2. Ask `SceneHost::intercept_paste` whether the active route has a registered interceptor.
3. If so, enqueue the resulting Action and return `Consumed`.
4. Otherwise dispatch paste through local component handlers.
5. Drain outputs.
6. If consumed, invalidate focus-related components and dirty the frame.

`forward_paste` bypasses interception and uses ordinary focused-component routing (`kernel.rs:758-769`). The production test confirms that an intercepted raw paste can enqueue a routed action, whose update then forwards a marker that is delivered without re-interception (`tests.rs:1431-1478`).

## 4.6 Frame preparation

`RunningApp::prepare_frame_with_states` (`kernel.rs:667-701`) is the semantic-to-scene preparation bridge.

If `body_dirty`:

1. Invoke `ViewFn(&State)`.
2. Replace the Scene body only if the returned View differs.
3. Clear `body_dirty`.

Then call:

```text
SceneHost::render_at_with_states(
    now,
    Scene,
    ComponentRegistry,
    Theme,
    NativeHistorySink,
    viewport,
    state frame view,
    content provider
)
```

On success:

- retired components are reaped;
- `dirty` is cleared;
- a `PreparedSceneFrame` is returned.

On failure:

- `dirty` is not cleared by the generic kernel’s caller;
- the native host wraps the failure with phase/code/retryability and keeps the last committed frame authoritative.

## 4.7 Test-only generic event loop

`application/tests/driver.rs` is not production code. It provides a fake terminal event loop for behavioral tests.

Important constants:

```text
INPUT_PUMP_BUDGET = 32
MIN_PRESENT_INTERVAL = 8ms
```

(`tests/driver.rs:188-190`).

The loop (`tests/driver.rs:228-300`) does:

1. Prepare and draw the initial frame.
2. Initialize presentation scheduling.
3. Drain deferred pastes and external actions.
4. Repeatedly:
   - pump up to 32 buffered terminal events;
   - call `advance_ready`;
   - if dirty and presentation interval permits, prepare and begin a frame;
   - if exiting, await any in-flight receipt, advance once more, and draw a final dirty frame if needed;
   - if more actions remain, yield to Tokio;
   - otherwise select among:
     - in-flight receipt completion;
     - application/component deadline;
     - terminal event;
     - external action ingress.

This test driver demonstrates the intended production scheduling contract, but it is not the production owner of a generic Rust application.

## 4.8 Native host startup

`TuiHost::open_in_environment` (`host.rs:968-1045`) performs:

1. Validate positive width/height.
2. Create either:
   - `HeadlessSink { width, height }`, or
   - `TermwizBackend::enter()`.
3. Construct an internal `TuiApp` with:
   - `host_init`;
   - `host_update`;
   - `host_view`;
   - a fresh `Theme`;
   - a fresh `History`.
4. Start its `RunningApp`.
5. Prepare a bootstrap frame.
6. Create `HostInner` with:
   - authoritative bootstrap frame;
   - `frame_pending = true`;
   - no candidate metadata;
   - empty epochs;
   - ViewState and content registries.
7. Register the host’s weak Arc with the environment and assign its host ID.
8. Present the bootstrap frame.
9. Return `TuiHost`.

`host_init` installs a spacer body and empty routed-output queue. `host_update` only appends `HostOutput::Routed` values to the queue. Product/application interpretation remains outside Rust host code (`host.rs:69-89`).

## 4.9 Desired structural publication

`TuiHost::set_desired_view` (`host.rs:1091-1120`) accepts the desired structural root but deliberately does not immediately prepare/present it.

The operation:

1. Locks `HostInner`.
2. Rejects a closed host.
3. Resolves all ViewState attachments in the candidate body plus current History.
4. Rejects duplicate ViewState IDs.
5. Resolves all ContentPort attachments in the candidate body plus current History.
6. Validates content targets.
7. Computes the next checked structural revision.
8. Updates desired state bindings and content bindings.
9. Stores the new body in the internal host state and Scene.
10. Writes the desired structural revision.
11. Increments `pending_epoch` and marks the host pending in the shared environment.

The returned `WakeDisposition` contains only an edge-trigger scheduling hint. The host epoch and environment queue remain authoritative.

`TuiHost::render` composes this with `flush_pending` (`host.rs:1320-1323`).

## 4.10 Candidate preparation and presentation

`HostInner::flush_pending_frame` is the principal native state machine (`host.rs:2242-2351`).

The high-level sequence is:

```text
closed check
    ▼
deferred Source cleanup admission
    ▼
reconcile completed older candidate, if any
    ▼
advance content delivery
    ▼
advance generic RunningApp
    ▼
refresh desired bindings if invalidated components changed
    ▼
ensure pending epoch when dirty
    ▼
poll bootstrap/in-flight presentation
    ▼
prepare a new candidate if needed
    ▼
submit candidate to backend
    ▼
wait/poll PresentReceipt
    ▼
commit candidate
```

`HostInner::render` prepares a candidate (`host.rs:1870-1959`):

1. If another presentation receipt is still in flight, report `waiting_for_presentation`.
2. Capture the target pending epoch and desired structural revision.
3. Begin content projection candidate capture.
4. Capture a retained-state candidate overlay.
5. Prepare a scene frame using committed state plus candidate state overlay.
6. Prepare state and content commit plans.
7. Store:
   - candidate frame;
   - candidate epoch;
   - candidate structural revision;
   - candidate content epoch;
   - candidate content commit;
   - candidate state commit.
8. Begin in-flight content leases.
9. Submit the candidate to the backend.
10. If submission fails:
    - retain failure coordinates;
    - discard candidate state/content/frame;
    - restore the generic kernel’s dirty bit;
    - leave the desired epoch pending for retry.
11. If a receipt is pending, return waiting status.
12. If headless presentation completed synchronously, commit immediately.

## 4.11 Receipt and visibility transitions

`present_frame` (`host.rs:1961-2023`) handles both prior receipts and new frame submission.

For an existing receipt:

- `Ok(())` means presentation completed;
- `TryRecvError::Empty` leaves it in flight;
- `TryRecvError::Closed` becomes `BACKEND_NOT_READY`;
- receipt failure becomes `BACKEND_IO_FAILED`;
- receipt failures mark `physical_sync_unknown`.

For a new frame:

- Real backend `begin_frame` returns and stores a `PresentReceipt`;
- stopped terminal worker marks the host closed and returns non-retryable `BACKEND_NOT_READY`;
- other backend errors return retryable `BACKEND_IO_FAILED`;
- headless backend clears `frame_pending` synchronously.

The last complete logical frame remains in `HostInner::frame`. A candidate is not visible to readback until commit. This explicitly prevents readback from observing a frame whose backend receipt or retained-state/content promotion has not completed.

## 4.12 Commit transition

`commit_frame` (`host.rs:2026-2114`) is the visible-authority boundary.

Before entering environment completion authority, it validates:

- visible frame revision can increment;
- candidate frame exists;
- candidate epoch exists;
- candidate structural revision exists;
- candidate content epoch exists;
- candidate state commit exists.

It then calls `TuiEnvironment::with_host_completion` while holding environment authority across content/state/frame promotion (`environment.rs:339-414`, `host.rs:2051-2113`).

Inside the commit closure:

1. Commit prepared content.
2. If content commit fails:
   - retain candidate coordinates in `failed_attempt`;
   - return an error without promoting visible state.
3. Take the candidate frame and commit plans.
4. Promote retained-state prepared values.
5. End content candidate leases.
6. Commit the generic SceneHost content candidate.
7. Clear candidate metadata.
8. If physical synchronization is unknown:
   - recover native History synchronization;
   - clear the marker.
9. Promote candidate frame to authoritative `frame`.
10. Update:
    - `frame_pending`;
    - visible structural revision;
    - visible frame revision;
    - committed epoch.
11. Clear `content_dirty` only when no newer pending epoch exists.
12. Return a `HostFlushOutcome::committed`.

If a newer mutation arrived while the candidate receipt was in flight, `pending_epoch != candidate_epoch`, so the committed older frame is visible but the newer epoch remains pending and is requeued by environment completion bookkeeping.

## 4.13 Native host close and exit

### `TuiHost::exit`

`exit` (`host.rs:1189-1240`) is a final-frame shutdown path:

1. If already closed, unregister the host and return.
2. Block for any earlier receipt.
3. Request generic application exit via `RunningApp::host_exit`.
4. Advance/render the final state.
5. Block for the final presentation receipt.
6. Extract final non-empty physical rows.
7. For headless mode, append them to the native History sink.
8. For Real mode:
   - call `position_after_final_frame`;
   - restore the terminal.
9. On success:
   - dispose ViewStates;
   - dispose content;
   - mark closed;
   - unregister from the environment.

If the final frame or positioning/restoration fails, the host is not marked closed in the success-only cleanup branch.

### `TuiHost::close`

`close` (`host.rs:1558-1609`) is the explicit resource shutdown path.

- A lost presentation receipt causes:
  - ViewState/content disposal;
  - host closure;
  - Real-backend restoration attempt;
  - environment unregistering;
  - combined error if both receipt and restore fail.
- Normal close:
  - replaces the host state body with a spacer;
  - clears the SceneHost retained views;
  - disposes ViewStates/content;
  - marks closed;
  - restores Real terminal;
  - unregisters environment membership.

### `Drop`

`TuiHost::Drop` only calls `close` if the wrapper’s Arc strong count is one (`host.rs:1662-1667`). `HostInner::Drop` independently unregisters its environment host and disposes content (`host.rs:175-185`).

This is safe for ordinary cloned `TuiHost` wrappers, but `HostHistory` is a distinct strong owner and can delay the explicit close path.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation → production path matrix

| Semantic operation | Production path | Selection/conditions | Failure/recovery behavior |
|---|---|---|---|
| Define/start generic app | `App::start` → `RunningApp::new` | Generic Rust/in-crate use | `init` error becomes `KernelError::Application`; no runtime created |
| Send application Action | `AppHandle::send` | Nonblocking producer | `Full` or `Closed`; original Action recoverable |
| Async send Action | `AppHandle::send_async` | Asynchronous producer | Waits for capacity; only closure is reported |
| Receive external Action | driver `recv_external`/`collect_external` | Driver-controlled | Disconnected ingress closes; queued actions already collected remain processable |
| Schedule timer | `AppCx::schedule_after` → `TimerQueue` | One-shot action | Checked identity/deadline/sequence; exhaustion panics |
| Process timer | `advance_ready` | Deadline reached | Due timers inserted before current action backlog |
| Component output | `SceneHost` → `OutputRouter` → action queue | Typed route exists | Missing/type mismatch becomes `KernelError::Output` |
| Key dispatch | `RunningApp::dispatch_key` | Component route first, global exact-key fallback second | Component result returned; global factory runs only after local `Ignored` |
| Paste dispatch | interceptor → local component route | Active routing chain first | Interceptor produces Action; forwarded paste bypasses interceptors |
| Update callback | `advance_ready` → `Update` | Up to 128 actions/pass | `KernelError::Application`; current batch stops |
| Body derivation | `prepare_frame_with_states` → `ViewFn` | `body_dirty` only | View callback error is not a `Result`; panic propagates if callback panics |
| Native desired root | `TuiHost::set_desired_view` | Host open and target validation passes | Closed/duplicate/stale/invalid target errors; desired revision is assigned only after preflight |
| Native content wake | Source mutation → environment pending queue | Active subscribers | Source acceptance succeeds independently; wake failures reported later |
| Native automatic drain | `TuiEnvironment::drain_pending(false)` | Environment wake broker | Errors recorded, retry-blocked; no continuous retry spin |
| Native explicit barrier | `flush_pending_hosts(..., true)` | Explicit flush | Forces blocked retry and returns structured errors; TypeScript error channel can throw |
| Frame preparation | `HostInner::render` → `prepare_frame_with_content` | Pending epoch and no competing receipt | Candidate rollback; previous frame remains authoritative |
| Backend submit | `present_frame` → backend `begin_frame` | `frame_pending` | Worker stopped closes host; I/O failure retryable; candidate discarded |
| Receipt completion | `poll_presentation`/`finish_presentation_blocking` | Receipt available | Receipt failure marks physical sync unknown and leaves desired work pending |
| Content/state commit | `commit_frame` | Candidate receipt complete | Content/state/frame promotion is coordinated; commit failure retains candidate or reports blocked retry |
| Host close | `close` | Explicit close/drop | Lost receipt and restore errors are combined; normal close restores Real terminal |
| Host exit | `exit` | Final-frame shutdown | Final rows are transferred/positioned before restoration |

### 5.2 Application versus runtime errors

The generic application intentionally distinguishes semantic callback errors from runtime errors:

```text
KernelError::Application(Error)
KernelError::Output(OutputDispatchError)
```

The test driver then wraps backend/terminal failures as `RunError::Runtime`, preserving the application error type as `RunError::Application` (`tests/driver.rs:20-50`, `472-476`).

The runtime error source chain is tested. `RuntimeError::source()` exposes the wrapped top-level cause, including `OutputDispatchError` (`application/tests.rs:59-74`).

### 5.3 Native structured error phases/codes

`HostAttemptError` carries internal phase/code/retryability metadata (`environment.rs:23-50`).

Observed native codes include:

- `BACKEND_IO_FAILED`;
- `BACKEND_NOT_READY`;
- `LAYOUT_DID_NOT_CONVERGE`;
- `HISTORY_TRANSFER_FAILED`;
- `FRAME_PREPARATION_FAILED`;
- `CONTENT_SCHEDULER_FAILED`;
- `SOURCE_WAKE_FAILED`;
- `HOST_LOCK_POISONED`;
- `INTERNAL_INVARIANT`.

`prepare_frame_with_content` maps `SceneHostError` variants into phase/code/retryability (`host.rs:2375-2426`):

- non-convergence → `LAYOUT_DID_NOT_CONVERGE`, non-retryable;
- History transfer failure → `HISTORY_TRANSFER_FAILED`, retryable;
- other scene preparation failures → `FRAME_PREPARATION_FAILED`, retryable.

### 5.4 Candidate rollback versus failure masking

The intended native recovery path is explicit:

```text
failed preparation/submission/receipt
    ▼
record failed coordinates
    ▼
discard candidate state/content/frame
    ▼
restore generic kernel dirty state
    ▼
leave pending epoch active
    ▼
retry explicitly or on a later valid wake
```

The code does not replace a failed candidate with a default visible frame. `HostInner::frame` remains authoritative, and `SceneHost` candidate state is discarded (`host.rs:1895-1903`, `2170-2184`, `2190-2197`).

There are, however, narrower fallback paths inside mounted host controls:

- `MountedScrollPane::view` returns a spacer if its state mutex is poisoned (`host.rs:557-563`);
- `MountedViewSlot::view` returns a spacer on poisoned state (`host.rs:610-616`);
- `MountedTextInput::view` returns a spacer on poisoned input state (`host.rs:751-758`);
- command/paste callbacks map poisoned locks to `InteractionResult::Ignored` (`host.rs:780-804`).

These are observable failure-masking fallbacks. They are distinct from `HostInner` lock failures, which generally return explicit errors. They may be intended to preserve frame execution, but they can hide a poisoned control and render an apparently valid spacer instead of surfacing a runtime error.

### 5.5 Retry-blocking semantics

Automatic environment drains do not continuously retry a host whose same epoch failed:

- `complete_host_locked` inserts it into `retry_blocked` when `block_for_retry` applies (`environment.rs:423-464`);
- `block_host` removes it from the runnable queue while retaining pending membership (`environment.rs:467-483`);
- only a new wake, explicit force retry, or another defined requeue path removes the block (`environment.rs:486-502`, `569-588`).

This prevents permanent failures from spinning the microtask/event loop.

### 5.6 Shared-environment failure isolation

The environment drain handles hosts independently:

- a poisoned host lock becomes a `HOST_LOCK_POISONED` error;
- the host is blocked;
- unrelated candidate host IDs continue draining (`environment.rs:619-640`).

The in-tree native tests explicitly cover:

- fair draining across hosts;
- poisoned host not dropping unrelated pending hosts;
- Source wake errors reported while healthy subscribers still present frames;
- stale Source wake errors dropped after membership ends (`host.rs` and `content.rs` test modules).

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Generic application queues and bounds

| Mechanism | Key/bound | Retention/invalidation |
|---|---|---|
| External ingress | Tokio bounded channel, capacity 1,024 | Closed when app exits or receiver disconnects |
| Internal action queue | `VecDeque<Action>` | Actions removed as updates execute; cleared on exit |
| Update batch | 128 actions/pass | `more_ready` yields control to driver |
| Terminal input pump | 32 events/test-driver pass | Stops after a routed action or budget |
| Timer queue | Vector, no fixed count cap | Deadline/sequence checked; cancel uses swap-remove |
| Global bindings | `HashMap<KeyStroke, factory>` | Exact-key replacement/unbind |
| Paste interceptors | `HashMap<ComponentId, mapper>` | Removed with component registry retirement |
| Deferred pastes | `VecDeque<String>` | Drained after each update and then removed |
| Environment pending hosts | FIFO `VecDeque<host_id>` plus sets | Host ID queued once; retry-blocked IDs excluded |
| Presentation scheduler | Minimum 8ms in test driver | Dirty presentation deadline cleared when clean |

### 6.2 Timer scheduling cost

`TimerQueue` is a linear vector:

- insertion is amortized vector append;
- cancellation is O(n);
- due selection is O(n);
- next-deadline lookup is O(n).

The queue provides deterministic same-deadline ordering using `sequence`. Tests verify:

- earlier deadline wins;
- same deadline preserves scheduling order;
- cancellation is idempotent;
- timer handles are queue-specific;
- fired handles cannot cancel replacement timers (`tests.rs:725-773`).

The current implementation favors small/simple deterministic queues over a heap.

### 6.3 Content scheduling and caches

Although content implementation is in the native-host branch of the application directory, it is part of the host scheduling path.

`ContentHostRegistry` owns active Connector execution and deadlines:

- `advance(now)` updates active smooth delivery without reparsing Source storage (`content.rs:3586-3695`);
- `next_wakeup()` returns the minimum active connector deadline (`content.rs:3698-3700`);
- `sync_connector_deadline` updates active connector membership and deadlines (`content.rs:3702-3774`).

The host’s `next_wake_ms` combines generic component/timer deadlines with content wakeups (`host.rs:1243-1259`).

Content caches have explicit small bounds:

- `CONTENT_CACHE_CAPACITY = 2`;
- `CONTENT_PREFIX_CACHE_CAPACITY = 2` (`content.rs:261-266`).

Semantic projection cache keys include:

```text
source identity/generation
content generation
source revision/range
sealed state
funnel kind
hyperlink mode
```

(`content.rs:269-298`).

Theme, width, delivery revision, wrap, finalized-prefix policy, and physical-row requirements are intentionally separated into later paint/projection keys. Theme-only changes therefore reuse semantic parsing (`content.rs:302-324`).

### 6.4 Persistent Source storage

`source_store.rs` uses immutable pages and persistent roots:

- Source chunks are 16 KiB pages (`source_store.rs:26`);
- chunk tree leaves hold up to 16 descriptors;
- branches hold up to 16 children;
- append copies only the right-edge path when old snapshots share nodes;
- truncation shares page Arcs and re-describes only boundary views;
- annotation index is a persistent treap keyed by `(start, seqno)`.

`StoredSource` stores:

```text
ChunkTree
AnnotationTree
ordered annotation cache
base/end offsets
head_partial
revision
next annotation sequence
sealed/sealed_at
```

(`source_store.rs:1077-1100`).

Validated mutation operations are transactional at the storage value level:

- `apply_append` validates sealed state, lengths, and annotation sequence before returning a new value (`source_store.rs:1152-1203`);
- `apply_seal` validates range/sealed state and atomically adds the seal marker (`source_store.rs:1221-1264`);
- `apply_truncate` validates offset before changing the retained range (`source_store.rs:1334-1355`).

This supports immutable snapshots for projection and diagnostic readers.

### 6.5 Frame invalidation classes

The generic kernel has two related flags:

- `dirty`: a frame needs preparation/presentation;
- `body_dirty`: the application `ViewFn` must be re-run before preparation.

Examples:

| Mutation | `dirty` | `body_dirty` | Typical path |
|---|---:|---:|---|
| Application update | yes | yes | `advance_ready` |
| Component redraw tick | yes | no | `SceneHost::tick_due` |
| Component output consumed | yes | no initially; update later sets both | output → action |
| Global key action | update later sets both | update later sets both | `dispatch_key` |
| Theme mutation | yes | no | `host_set_theme` |
| Desired root publication | yes/pending epoch | no after body already installed | `set_desired_view` |
| Source/control content mutation | host pending/content dirty | no generic body rederive | `mark_content_pending` |
| Resize | yes | no | `resize` |
| ViewState mutation | yes if bound | no | `invalidate_state` |

Content mutations deliberately dirty the content/presentation path without treating the generic application body as semantically changed (`host.rs:1831-1847`, `kernel.rs:163-167`).

### 6.6 Native candidate retention and invalidation

The host retains independent products for:

- authoritative visible frame;
- in-flight candidate frame;
- candidate state commit;
- candidate content commit;
- candidate source/content dirty epoch;
- backend receipt.

The candidate metadata is required to be all-present or all-absent. `clear_in_flight_state_bindings` explicitly checks this invariant and panics if partial metadata exists (`host.rs:1763-1785`).

This prevents a receipt-time commit from accidentally combining:

- an old frame;
- newer content;
- newer state;
- or newer structural bindings.

### 6.7 Environment queue cost and fairness

`TuiEnvironment` coalesces pending hosts by ID:

- `pending_set` prevents duplicate logical membership;
- `queued` prevents duplicate queue entries;
- `retry_blocked` removes failed hosts from automatic runnable work;
- `waiting_for_presentation` keeps asynchronous receipt completion discoverable without immediate spin.

`drain_pending_for` takes a bounded host budget, defaulting to at least one (`environment.rs:504-522`). It resolves weak hosts and drains candidates independently. The queue is FIFO except explicit barriers can prioritize the requested host (`environment.rs:586-588`).

### 6.8 Observed counters and instrumentation

The application module itself does not have counters for:

- action queue length;
- number of `Update` callbacks;
- timer insert/cancel/pop;
- body derivation count;
- generic event-loop wakeups.

The Rust performance counter subsystem has counters relevant to content/frame work, including:

- `ComponentViewCalls`;
- `ComponentCapabilityCalls`;
- `HistoryUnitsExamined`;
- `SourceSnapshotsAcquired`;
- `SemanticProjectionRebuilds`;
- `ContentDirtyRecordsMarked`;
- `ContentMetricEvaluations`;
- `ContentMetricChanges`;
- `ContentPaintPropagations`;
- `ContentWakeGroups`;
- `ContentDueConnectors`;
- `ContentCandidateRecordsPrepared`;
- `ContentPathIndexNodesVisited`.

(`crates/iyon-tui/src/perf.rs:15-70`, names at `80-135`).

The TypeScript wake broker has much more direct runtime scheduling observability:

```text
pending_marks
wake_latch_wins
wake_already_latched
microtasks_queued
drains
hosts_attempted
frames_committed
automatic_errors
rearm_count
explicit_barriers
explicit_barrier_failures
```

(`packages/iyon-tui/src/runtime/wake-broker.ts:65-77`).

It also maintains a bounded 256-entry wake trace when `PERF_RUNTIME_TRACE=1` (`wake-broker.ts:79-119`).

---

## 7. Tests, benchmarks and observability

### 7.1 Generic application behavior protected by tests

`application/tests.rs` covers:

- error source preservation (`59-74`);
- theme copy-on-write (`114-152`);
- combined input/output/timer/History behavior (`366-469`);
- local paste dispatch (`471-498`);
- paste interceptor registration versus mount lifetime (`500-568`);
- FIFO actions and one View derivation per frame (`570-612`);
- zero-duration timer deferral (`614-650`);
- timer fairness ahead of action backlog (`652-683`);
- bounded self-rescheduling timer work (`685-723`);
- timer ordering/cancellation/queue identity (`725-773`);
- tick redraw without body re-derivation (`823-845`);
- tick output routing (`847-878`);
- component mutation/removal (`880-918`);
- body-only apps, no History, non-`Send` state (`920-946`);
- exit clearing later work (`948-978`);
- output route conflict/remove/re-add (`980-1004`);
- initialization/update application error behavior (`1006-1041`);
- production driver processing of pre-run actions (`1043-1075`);
- init-exit skipping backend construction (`1077-1098`);
- AppHandle wake without terminal polling (`1100-1129`);
- component tick wake (`1131-1165`);
- yielding between finite action batches (`1167-1196`);
- servicing terminal input between action batches (`1198-1250`);
- input updates while presentation is in flight (`1252-1322`);
- application timer wake (`1324-1354`);
- resize viewport usage (`1356-1392`);
- init-time forwarded paste (`1394-1429`);
- production paste interception and forwarding (`1431-1478`);
- restore failure preservation (`1480-1502`);
- backend/application error mapping (`1504-1572`);
- restoration on dropped run future (`1574-1588`);
- local non-`Send` runtime support (`1590-1599`);
- closed action recovery and conditional thread traits (`1601-1619`);
- modal paste routing (`1636-1685`);
- global key traversal and binding lifetime (`1711-1808`);
- native History transfer through the backend (`1810` onward).

These tests are strong evidence for update ordering and event-loop fairness, but they do not provide production coverage for a generic Rust run loop because no such production run loop exists in this crate.

### 7.2 Native host behavioral tests

`application/host.rs` has tests beginning around line 2,442. Named coverage includes:

- desired revision waits for successful frame barrier;
- failed frame preserves old visible state and explicit retry recovers;
- presentation state repaint without measurement/semantic republication;
- structural publication invalidates retained-state dependency paths;
- component-slot replacement carries captured state versions;
- failed frame retains old state versions until retry;
- identical themes resolve identically across hosts;
- environment requeues in-flight presentation receipts;
- failed presentation marks physical sync unknown until recovery;
- failed bootstrap receipt reports backend error without candidate state;
- poisoned content commit preserves state/content/frame authority;
- delayed content receipt preserves newer Source work;
- Source cleanup failure preserves membership until retry;
- environment drain fairness across hosts;
- poisoned host does not drop unrelated pending hosts;
- post-acceptance wake failure retains revision authority;
- later subscribers still wake after one subscriber failure;
- exhausted desired revision rejects publication without mutation;
- state patch does not alter desired structural revision;
- Source wake repaints only affected content port;
- content metric growth reflows following siblings;
- content dirty work stays local to one of many ports;
- bounded targeted prepare work with hundreds of ports;
- path index boundedness;
- theme recolor refreshes content paint without rebuilding layout;
- structural/theme/Source order preserves fresh content tickets.

These tests directly protect the candidate/receipt/commit protocol rather than merely checking final pixels.

### 7.3 Test-only fake backend

The fake backend (`application/tests.rs:181-329`) can inject:

- terminal event failure;
- viewport failure;
- draw failure;
- restore failure;
- delayed presentation receipts.

It records:

- draw count;
- in-flight presentation senders;
- frame text;
- viewport calls/sizes;
- transferred native rows;
- final-position calls;
- restore calls.

The test driver uses these fields to verify lifecycle ordering, receipt waiting, and restoration.

### 7.4 Native environment/content tests outside `host.rs`

The content test module includes extensive environment/host lifecycle tests, including:

- host-scoped Source subscriptions;
- poisoned subscriber wake reporting;
- stale Source wake error dropping;
- cross-host ContentPort identity protection;
- activation failure rollback;
- failed Connector retry on remount;
- disposed Connector status/frontier retention;
- delayed smoothing candidate retention;
- finalized-prefix and History-row behavior.

These are application-directory tests even though their subject crosses into content and presentation modules.

### 7.5 Observability gaps

The following runtime state is not directly exposed by the generic Rust kernel:

- current internal action queue length;
- action batch count;
- timer queue length;
- number of body derivations outside test instrumentation;
- generic `dirty`/`body_dirty` transition trace;
- receipt latency;
- candidate age;
- host lock contention;
- number of retries before successful commit.

Native host epochs and drain reports expose desired/visible progress, errors, and commit outcomes. The TypeScript broker exposes scheduling counters and bounded wake traces. The Rust generic App path has much less production observability.

### 7.6 Test execution status

No tests were run for this report. All statements about tests are based on source inspection of test definitions and assertions.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Generic Rust App versus native host runtime

The generic `App` is intentionally backend-free (`app.rs:7-10`), but `TuiHost` embeds one as its internal implementation (`host.rs:986-996`). This produces two lifecycle layers:

```text
generic App/RunningApp:
    callbacks, actions, timers, component routing, semantic frame preparation

native TuiHost/HostInner:
    backend, receipts, epochs, candidate rollback, environment queue, close/restore
```

The separation is real in code, but `HostInner` has to reach deeply into `RunningApp` through host-only methods such as:

- `host_set_body`;
- `host_set_theme`;
- `host_set_history`;
- `host_exit`;
- `host_invalidate_component`;
- `host_invalidate_state`;
- `host_invalidate_content`;
- `host_commit_content_candidate`;
- `host_recover_native_history_synchronization`.

(`kernel.rs:145-189`, `398-429`).

### 8.2 There is no production Rust application driver

The only reusable application event loop is `application/tests/driver.rs`. Production native operation is driven through `TuiHost` APIs and the TypeScript runtime:

- TypeScript `Tui.nextEvent` calls native `waitForOutput` (`runtime.ts:347-370`);
- TypeScript `Tui.flush` calls retained execution flush followed by the wake broker’s explicit host barrier (`runtime.ts:407-421`);
- automatic state/content drains use `EnvironmentWakeBroker` (`wake-broker.ts:121-175`, `275-338`).

This means the generic Rust application callbacks are primarily an internal implementation mechanism and testable kernel, not an independently runnable public Rust application runtime.

### 8.3 Shared wake scheduling is split across Rust and TypeScript

Rust owns:

- source subscriptions;
- host pending set;
- wake latch;
- wake epoch;
- fair queue;
- retry blocking;
- host candidate processing.

TypeScript owns:

- environment-level broker registration;
- microtask scheduling;
- pending driver selection;
- structured runtime-error channels;
- presentation receipt polling timer;
- automatic versus explicit barrier policy.

The TypeScript broker explicitly states that native environment state owns the complete affected-host set and JavaScript should not mirror subscriptions (`wake-broker.ts:158-175`). This is consistent with `TuiEnvironment` being the source of truth.

### 8.4 `wait_for_output` does not directly drain the shared environment

`TuiHost::wait_for_output`:

1. checks `exited`;
2. refreshes headless time;
3. calls `poll_terminal`;
4. checks `next_output`;
5. sleeps until a local deadline.

(`host.rs:1513-1535`).

It does not call `flush_pending_hosts` or `environment.drain_pending_for`. The TypeScript runtime’s ordinary no-signal `nextEvent` calls this method directly (`runtime.ts:347-352`), while normal retained-state/content writes use the separate wake broker. This is either a deliberate separation—`waitForOutput` is only for terminal/routed-output events—or a seam requiring integration validation for Source-only wake scenarios. The source does not establish that `wait_for_output` alone is sufficient to present a Source append.

### 8.5 Desired structural revision and visible frame revision are intentionally independent

`set_desired_view` increments `desired_structural_revision` and `pending_epoch` before visibility (`host.rs:1099-1119`). `visible_structural_revision` and `visible_frame_revision` change only after receipt and commit (`host.rs:2095-2102`).

The tests confirm:

- desired publication can be pending while the old frame remains visible;
- a successful frame barrier makes desired and committed epochs converge;
- failed frame preparation does not roll back the accepted desired revision.

This is the core read-your-writes/visible-commit split.

### 8.6 History has multiple ownership paths

History may be:

1. created inside the native host through `TuiApp::with_history(History::new())` (`host.rs:986-992`);
2. mutated through `HostHistory`;
3. supplied from TypeScript and bound once through the TypeScript runtime sideband (`runtime.ts:280-317`);
4. retained by `Scene`.

The TypeScript runtime documents attach-once History semantics and rejects swapping a different already-bound History (`runtime.ts:296-317`). Rust `HostHistory::set_history` itself accepts a new `History` and updates the Scene (`host.rs:1347-1361`), but does not immediately call `advance_and_render`. Its desired work becomes visible only when a later host barrier/drain processes the invalidated generic runtime. This differs from `HostHistory::push`, `freeze`, and `discard_live`, which render immediately.

### 8.7 HostHistory strong lifetime can outlive TuiHost

`HostHistory` contains a strong `Arc<Mutex<HostInner>>` (`host.rs:818-822`). The public `TuiHost` wrapper closes only if its Arc count is one (`host.rs:1662-1667`). Therefore:

```text
last TuiHost wrapper dropped
    + HostHistory still alive
    → TuiHost::Drop does not call close
    → HostInner remains alive
```

When the final HostHistory is eventually dropped, `HostInner::Drop` disposes content and unregisters the host but does not explicitly restore the terminal backend. The behavior of `TermwizBackend::Drop` is not in the inspected application files, so the terminal restoration result is unresolved.

### 8.8 Host control mutation is synchronously rendering, while root publication is deferred

There are two distinct mutation styles:

- Root publication:
  - `set_desired_view` accepts and queues;
  - `flush_pending` is a separate visibility barrier.
- Host control mutations:
  - `HostTextInput`, `HostViewSlot`, and `HostScrollPane` mutate their state and call `advance_and_render` directly.
- Host History mutations:
  - most operations mutate and render immediately;
  - `set_history` only accepts/invalidates and does not render immediately.

This is a consequential API inconsistency for callers reasoning about when a mutation becomes visible.

### 8.9 Native host unsafe thread markers

Both `TuiEnvironment` and `TuiHost` are marked `Send`/`Sync` manually:

- `TuiEnvironment`: `environment.rs:129-135`;
- `TuiHost`: `host.rs:955-959`.

The stated invariant is that component registries and callbacks never cross the async boundary and all access is serialized through mutexes. The correctness of this depends on all callback invocation remaining under the intended host lock discipline. This is a substantial process/runtime ownership assumption rather than a type-system-enforced guarantee.

### 8.10 Generic framework boundary is respected in naming/meaning

The Rust host uses generic terms:

- `RoutedOutput`;
- `route_id`;
- `HostOutput`;
- `History`;
- `ContentSource`;
- `Connector`;
- `TextInput`;
- `ViewSlot`.

The internal host update only stores caller-defined routed outputs (`host.rs:76-89`). No agent, assistant, provider, model, transcript, or product action semantics were found in the application module. This matches the framework boundary in `AGENTS.md` and the report contract.

### 8.11 Application content code crosses into presentation

`application/content.rs` owns Source/Connector lifecycle but directly depends on:

- `presentation::ContentProvider`;
- presentation layout trees;
- presentation paint text geometry;
- View compilation;
- physical rows;
- semantic text projectors/renderers.

Examples include:

- `HostContentProjection.layout` and `text_geometry` (`content.rs:327-352`);
- `compile_semantic_content` (`content.rs:746-782`);
- `ContentHostRegistry::advance` and candidate commit paths (`content.rs:3589-3774`, `4673-5507`).

This is an observed cross-boundary coupling. It may be required by current content rendering architecture, but the application assignment does not determine its future ownership.

---

## 9. Open questions and coverage gaps

1. **Standalone native `wait_for_output` and Source wake behavior**
   - Does a Source append become visible if the caller only awaits `TuiHost::wait_for_output` and never invokes `flush_pending_hosts`?
   - The inspected method does not drain the shared environment queue.

2. **Final terminal restoration when `HostHistory` outlives `TuiHost`**
   - Does `TermwizBackend::Drop` restore terminal state?
   - If not, should HostInner’s destructor own a best-effort restore path?
   - This requires inspection of `terminal/termwiz` destructor behavior and native resource lifetime.

3. **Production generic Rust driver ownership**
   - Is the absence of a production `App` event loop intentional because all production usage is through `TuiHost`/TypeScript?
   - Or is a generic Rust driver expected to exist outside this crate?

4. **`HostHistory::set_history` visibility contract**
   - It invalidates the internal runtime but does not call `advance_and_render`.
   - Is the deferred visibility intentional and documented to all callers?

5. **Poisoned mounted-control fallback policy**
   - Spacer/ignored fallbacks preserve frame execution but hide poisoned state.
   - Is this expected resilience or should these paths report structured runtime errors?

6. **Unsafe `Send`/`Sync` guarantees**
   - The source comments state that callbacks remain under host ownership, but there is no compile-time proof that all future additions preserve this.
   - Are there cross-thread N-API invocation patterns that can call host methods concurrently with callback execution?

7. **Action queue observability**
   - No production counter exposes internal action backlog size or update-batch latency.
   - This makes diagnosing application-level starvation separate from TypeScript wake scheduling difficult.

8. **Receipt latency and cancellation**
   - `PresentReceipt` can remain in flight while newer desired work is accepted.
   - There is no explicit receipt cancellation path in the application module.
   - Shutdown blocks for receipts except when the receipt channel is lost.

9. **History mutation rollback**
   - `HostHistory::push`, `freeze`, and `discard_live` mutate History before calling `advance_and_render`.
   - If later frame preparation fails, the semantic History mutation remains accepted while the old visible frame remains authoritative.
   - This appears consistent with retryable desired state, but the exact caller contract is not stated in `host.rs`.

10. **Content cleanup retry visibility**
    - Deferred Source cleanup can leave an epoch logically committed but retry-blocked for cleanup.
    - The caller-visible distinction between frame commit success and cleanup completion is represented internally but should be verified at the TypeScript API layer.

11. **Exact physical LOC**
    - This report uses source line spans, not a code-only counter. A reproducible `wc`/`cloc` measurement was not executed.

12. **No executed validation**
    - The report does not claim that Rust tests, native tests, TypeScript tests, benchmarks, or terminal integration tests passed.

---

## 10. Evidence appendix

### 10.1 Primary inspected application files

Exhaustive assignment-scope manifest from `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`:

```text
crates/iyon-tui/src/application/app.rs
crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/application/context.rs
crates/iyon-tui/src/application/environment.rs
crates/iyon-tui/src/application/handle.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/application/input.rs
crates/iyon-tui/src/application/kernel.rs
crates/iyon-tui/src/application/mod.rs
crates/iyon-tui/src/application/run.rs
crates/iyon-tui/src/application/source_store.rs
crates/iyon-tui/src/application/tests.rs
crates/iyon-tui/src/application/tests/driver.rs
crates/iyon-tui/src/application/timer.rs
crates/iyon-tui/src/application/view_state.rs
```

### 10.2 Exact symbols and evidence locations

#### Generic application

- `App` — `application/app.rs:7-73`
- `App::new` — `application/app.rs:22-42`
- `App::handle` — `application/app.rs:44-47`
- `App::with_theme` — `application/app.rs:49-54`
- `App::with_history` — `application/app.rs:56-61`
- `App::start` — `application/app.rs:63-73`
- `AppCxParts` — `application/context.rs:18-31`
- `AppCx` — `application/context.rs:33-45`
- component APIs — `application/context.rs:76-119`
- output APIs — `application/context.rs:121-133`
- History/theme APIs — `application/context.rs:135-156`
- time/handle APIs — `application/context.rs:158-168`
- key/paste APIs — `application/context.rs:170-209`
- timer APIs — `application/context.rs:211-220`
- exit — `application/context.rs:222-225`
- `AppHandle` — `application/handle.rs:5-48`
- `AppSendError` — `application/handle.rs:50-100`
- `AppClosed` — `application/handle.rs:102-134`
- `GlobalBindings` — `application/input.rs:5-29`
- `PasteInterceptors` — `application/input.rs:31-71`
- `TimerHandle`/`TimerQueue` — `application/timer.rs:6-91`
- `KernelError`/`ReadyStatus` — `application/kernel.rs:27-40`
- `RunningApp` — `application/kernel.rs:42-68`
- component retirement — `application/kernel.rs:113-143`
- host invalidation bridge — `application/kernel.rs:145-189`
- host target collection — `application/kernel.rs:196-368`
- body/theme/history/exit host methods — `application/kernel.rs:398-429`
- startup — `application/kernel.rs:443-520`
- key dispatch — `application/kernel.rs:523-547`
- paste dispatch — `application/kernel.rs:549-572`
- update scheduling — `application/kernel.rs:574-627`
- frame preparation — `application/kernel.rs:653-706`
- ingress and deferred paste handling — `application/kernel.rs:721-769`
- timer collection/output drain — `application/kernel.rs:795-817`

#### Native host/environment

- `HostEpochs`, `HostFrameError`, `HostDrainReport` — `application/environment.rs:53-95`
- `EnvironmentIdentity` — `application/environment.rs:97-119`
- `TuiEnvironment`/`EnvironmentInner` — `application/environment.rs:121-157`
- environment registration — `application/environment.rs:229-256`
- pending queue/latch — `application/environment.rs:258-310`
- completion and retry bookkeeping — `application/environment.rs:312-502`
- fair environment drain — `application/environment.rs:504-722`
- `HostInner` ownership fields — `application/host.rs:122-173`
- `HostInner::Drop` — `application/host.rs:175-185`
- `HostViewSlot` — `application/host.rs:188-463`
- `HostScrollPane` — `application/host.rs:465-606`
- `HostTextInput` — `application/host.rs:629-816`
- `HostHistory` — `application/host.rs:818-947`
- `TuiHost` open/configuration — `application/host.rs:949-1175`
- host routing/rendering/resize — `application/host.rs:1178-1403`
- output wait/readback/close — `application/host.rs:1411-1667`
- candidate preparation — `application/host.rs:1670-1959`
- backend receipt handling — `application/host.rs:1961-2024`
- frame commit — `application/host.rs:2026-2114`
- candidate rollback/retry — `application/host.rs:2116-2240`
- pending-frame scheduler — `application/host.rs:2242-2363`
- backend-specific frame preparation — `application/host.rs:2365-2427`

#### Content/source lifecycle

- Source/content types and keys — `application/content.rs:45-68`, `207-298`
- semantic projection cache — `application/content.rs:300-324`
- Connector delivery/execution — `application/content.rs:354-473`
- Source snapshots/stats/mutation results — `application/content.rs:492-515`
- persistent Source registry — `application/content.rs:1672-1764`
- Source mutation APIs — `application/content.rs:2197-2677`
- Source lifecycle/disposal — `application/content.rs:2630-2793`
- host content registry — `application/content.rs:3219-3774`
- content candidate/commit lifecycle — `application/content.rs:4673-5507`
- port/connector APIs — `application/content.rs:6715-6994`
- persistent chunk/annotation storage — `application/source_store.rs:223-703`, `789-1011`
- `StoredSource` and mutation operations — `application/source_store.rs:1077-1355`

#### Tests and drivers

- test runtime error mapping — `application/tests/driver.rs:20-50`, `472-476`
- test runtime startup — `application/tests/driver.rs:88-154`
- presentation scheduler — `application/tests/driver.rs:188-226`
- test event loop — `application/tests/driver.rs:228-300`
- terminal session restoration — `application/tests/driver.rs:406-465`
- generic behavior tests — `application/tests.rs:59-1840`
- host lifecycle/failure tests — `application/host.rs:2441-4011`
- content/environment tests — `application/content.rs:7010` onward
- persistent Source tests — `application/source_store.rs:1672` onward

### 10.3 Supporting boundary files inspected

- `crates/iyon-tui/Cargo.toml:1-42`
  - crate is unpublished;
  - `native-host`, `test-util`, and `perf-counters` features;
  - Tokio, Termwiz, and related dependencies.
- `crates/iyon-tui/src/lib.rs:1-84`
  - application module is crate-private;
  - `App`/`AppCx` are internal re-exports;
  - host/native runtime remains internal.
- `crates/iyon-tui-native/src/tui.rs:624-729`, `965-993`
  - native host construction;
  - `flushPendingHosts`;
  - `pollTerminal`;
  - `waitForOutput`.
- `packages/iyon-tui/src/runtime/runtime.ts:108-215`
  - TypeScript `Tui` ownership and native host registration.
- `packages/iyon-tui/src/runtime/runtime.ts:319-370`
  - opening, `nextEvent`, `pollOutput`.
- `packages/iyon-tui/src/runtime/runtime.ts:407-421`
  - explicit flush path.
- `packages/iyon-tui/src/runtime/wake-broker.ts:37-119`
  - native report contracts and counters.
- `packages/iyon-tui/src/runtime/wake-broker.ts:121-187`
  - broker registration and edge-trigger pending.
- `packages/iyon-tui/src/runtime/wake-broker.ts:205-250`
  - explicit barrier/retry behavior.
- `packages/iyon-tui/src/runtime/wake-broker.ts:275-410`
  - automatic drain, receipt polling, report/error handling, pending refresh.
- `crates/iyon-tui/src/perf.rs:15-135`
  - Rust performance counter inventory.
- `AGENTS.md:77-128`
  - generic TUI ownership boundary and application-policy prohibition.

### 10.4 Files indexed but not treated as primary assignment ownership

The following were consulted only to prove cross-boundary consumers and were not assigned for detailed subsystem ownership:

- `crates/iyon-tui-native/src/tui.rs`
- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/iyon-tui/src/runtime/wake-broker.ts`
- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui/src/perf.rs`

### 10.5 Commands/evidence method

Read-only repository inspection used:

- directory/file manifest searches;
- source-content searches for module declarations, public symbols, call paths, feature gates, test names, and exact line references;
- focused source reads around lifecycle, scheduler, candidate, commit, error, and shutdown functions.

No repository files were edited, no dependencies were installed, no services were changed, and no test/build/benchmark suite was executed.