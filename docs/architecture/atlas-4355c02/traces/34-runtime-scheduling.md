# 34 — Runtime scheduling

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: TypeScript runtime startup, retained execution wake/update scheduling, frame/tick scheduling, runtime error delivery, terminal ownership, native session/backend startup, and close/exit teardown.
- Required context reviewed:
  - `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - `docs/architecture/atlas-4355c02/README.md`
  - `PRE-V5-ARCHITECTURE-REPORT.md`
  - `AGENTS.md`
- The framework boundary is respected throughout this report. The inspected runtime is generic terminal/UI infrastructure. No Iyon agent/application semantics are present in the traced production paths.

### Scope boundaries

Included:

1. TypeScript `Tui.open()` through native `NativeTuiHost`.
2. TS runtime environment, host registration, wake broker, error channel, retained execution runtime, and root publication.
3. Native N-API host wrappers in `crates/iyon-tui-native/src/tui.rs`.
4. Rust `TuiHost`, `HostInner`, `TuiEnvironment`, the generic application kernel, timers, component ticks, frame candidates, presentation receipts, and terminal backend.
5. Termwiz worker startup, presentation, input polling, final-frame positioning, restoration, and worker shutdown.
6. Relevant tests and counters that establish scheduling and lifetime contracts.

Excluded or only referenced at seams:

- Detailed semantic `View` construction and ABI field encoding, except where needed to identify root publication or terminal frame preparation.
- Detailed layout, paint, content parser, history, and retained-state internals, except where their invalidation or commit behavior affects scheduling.
- V5 design/disposition analysis. This is a current-state report only.
- Executed test/build validation. No tests, benchmarks, builds, or services were run in this investigation.

### Evidence status

This is a static source inspection. Evidence comes from exact source paths and symbols listed in §10. The report distinguishes:

- **Current source fact** — directly observed implementation.
- **Static inference** — behavior reconstructed from connected call paths.
- **Uncertainty/open question** — a consequential behavior that requires execution, integration, or design confirmation.

No runtime counter values are claimed because no executable validation was performed.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate production LOC | Approximate test LOC | Primary responsibility | Scheduling/lifetime role |
|---|---:|---:|---|---|
| `packages/iyon-tui/src/runtime/runtime.ts` | ~928 | 0 | Public `Tui` lifecycle, rendering, mutation barriers, host factories, close/exit | Main TS session owner; translates API calls into retained/native work |
| `packages/iyon-tui/src/runtime/wake-broker.ts` | ~551 | 0 | One edge-triggered TS wake broker per JS realm | Coalesces pending hosts, schedules microtasks, drains native environments, routes errors/commits |
| `packages/iyon-tui/src/runtime/error-channel.ts` | ~128 | 0 | Host-scoped deferred runtime errors | Stores latest automatic-drain failure until explicit barrier or successful commit |
| `packages/iyon-tui/src/runtime/environment.ts` | ~46 | 0 | One JS-realm runtime environment | Owns TS resource registry and wake broker |
| `packages/iyon-tui/src/runtime/attachments.ts` | ~313 | 0 | Desired/visible attachment lease ledger | Keeps state/content resource leases aligned with desired and visible host epochs |
| `packages/iyon-tui/src/runtime/access.ts` | ~26 | 0 | Testing/harness access registration | Exposes controlled flush, input, clock, and readback operations |
| `packages/iyon-tui/src/runtime/events.ts` | ~14 | 0 | `TuiEvent` type definitions | Public output/termination event shape |
| `packages/iyon-tui/src/runtime/handle-registry.ts` | ~80 | 0 | Generic framework-handle identity and disposal | Ensures native resources have runtime-local ownership and disposal state |
| `packages/iyon-tui/src/runtime/native-resource-registry.ts` | ~18 | 0 | Re-export of resource registry | Keeps runtime ownership import-neutral |
| `packages/iyon-tui/src/composition/execution.ts` | ~1,000+ | 0 | Retained scope execution, dirty queues, microtask scheduling, transactional publication | Drives canonical scene producer re-evaluation |
| `packages/iyon-tui/src/composition/tracked-state.ts` | ~136 | 0 | Observable `State<T>` source | Reads subscribe scopes; writes invalidate them |
| `packages/iyon-tui/src/testing/index.ts` | ~151 | 0 | Deterministic/headless harness | Adds synthetic clock and native snapshot barriers |
| `packages/iyon-tui/src/transport/native/addon.ts` | ~248 | 0 | Native addon contract and artifact loading | Defines `NativeTuiHostContract`, checks native build identity |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | >2,000 | 0 | Retained native root materialization/publication | Supplies root refs to the native host without presenting directly |
| `packages/iyon-tui/src/api/view/retained-state.ts` | ~236 | 0 | TS wrapper for native retained state | Converts state patches into native wake-producing calls |
| `packages/iyon-tui/src/api/content/retained.ts` | ~800 | 0 | TS Source/Funnel/ContentPort/Connector APIs | Source mutation and connector control wake the native environment |

Rust-side session and scheduling modules:

| Path | Approximate production LOC | Approximate test LOC | Primary responsibility |
|---|---:|---:|---|
| `crates/iyon-tui/src/application/host.rs` | ~2,363 through `HostInner` implementation, with several thousand additional test lines | ~2,000+ | Native binding host, candidate frames, environment interaction, input, close/exit |
| `crates/iyon-tui/src/application/environment.rs` | ~730 | small in-file/test-adjacent contribution | Native shared environment, pending-host queue, wake epoch, fair draining |
| `crates/iyon-tui/src/application/kernel.rs` | ~825 | 0 | Generic application state/action/view kernel, action batches, timers, ticks |
| `crates/iyon-tui/src/application/app.rs` | ~73 | 0 | Generic application definition/startup |
| `crates/iyon-tui/src/application/run.rs` | ~22 | 0 | Blocking presentation receipt and async deadline waits |
| `crates/iyon-tui/src/application/timer.rs` | ~91 | 0 | One-shot application timer queue |
| `crates/iyon-tui/src/component/tick.rs` | ~305 | separate `tick_tests.rs` | Mounted-component periodic tick scheduler |
| `crates/iyon-tui/src/terminal/backend.rs` | ~30 | 0 | Private terminal backend trait and lifecycle errors |
| `crates/iyon-tui/src/terminal/termwiz/backend.rs` | ~167 | 0 | Termwiz worker command/event bridge |
| `crates/iyon-tui/src/terminal/termwiz/worker.rs` | ~163 | 0 | Terminal setup, command loop, restoration |
| `crates/iyon-tui/src/terminal/termwiz/presenter.rs` | ~281 production plus extensive tests | ~700+ | Surface diffing, synchronized scrollback insertion, terminal-state cleanup |
| `crates/iyon-tui-native/src/tui.rs` | ~1,600+ including all native wrappers/tests | substantial | N-API host/resource wrappers |
| `crates/iyon-tui-native/src/lib.rs` | ~15 | 0 | Native module registration |

### 1.2 Counting method and limitations

The approximate figures are physical source-line estimates based on the line-numbered source listings and module boundaries. Production and test portions were separated where `#[cfg(test)]` regions or test files were identifiable. The repository did not provide a shell execution tool in this investigation context, so these are not `wc -l` results.

Generated ABI files were not counted as production implementation. They were indexed from the tracked-source manifest and referenced only at the native-view session seam.

### 1.3 Responsibility split

The runtime is divided into three scheduler layers:

```text
TypeScript retained execution
    ├─ State<T> subscriptions
    ├─ scope dirty queue
    ├─ queueMicrotask() auto-flush
    └─ structural publication preparation/commit
             │
             ▼
TypeScript environment wake broker
    ├─ one pending set per JS realm
    ├─ one edge-triggered microtask latch
    ├─ fair native environment drain calls
    └─ automatic error/commit routing
             │
             ▼
Native TuiEnvironment / HostInner
    ├─ pending host IDs and native wake epoch
    ├─ component ticks and application timers
    ├─ content advancement
    ├─ candidate frame preparation
    ├─ terminal presentation receipt
    └─ visible frame commit
             │
             ▼
Termwiz worker / headless sink
    ├─ terminal bytes and cursor/state changes
    ├─ native scrollback insertion
    └─ terminal restoration
```

The layers are intentionally not interchangeable:

- TS retained execution decides **which producer scopes need evaluation**.
- The TS wake broker decides **when to ask native for a bounded environment drain**.
- Native `TuiEnvironment` decides **which pending native hosts are runnable and in what order**.
- Each `HostInner` decides **whether content, ticks, actions, structural work, or a delayed presentation receipt requires a frame**.
- The terminal worker owns **terminal mode, output bytes, presentation diffing, and restoration**.

---

## 2. Types, APIs and contracts

### 2.1 Public TypeScript runtime contract

`packages/iyon-tui/src/runtime/runtime.ts:45-90` defines:

```ts
TerminalMetadata
TuiOpenOptions
TuiRuntime
```

The intentional public runtime operations are:

- `Tui.open(options?)`
- `size`
- `nextEvent(signal?)`
- `render(sceneOrBuilder, signal?)`
- `flush()`
- `onRuntimeError(listener)`
- `resize(width, height)`
- `close()`
- `exit()`
- handle factories (`createHistory`, `viewState`, `contentPort`, `createTextInput`, `createViewSlot`, `createScrollPane`)
- generic input/output routing (`bindKey`, `route`, `interceptPaste`, `forwardPaste`)
- `setTheme`.

The public contract explicitly distinguishes:

- direct structural scene values;
- retained scene producers;
- explicit flush/read-your-writes barriers;
- output events;
- terminal termination events.

The `signal` is checked synchronously at operation entry. `Tui.open()` has no asynchronous work of its own after the synchronous native construction path begins, so an abort arriving after startup starts is not independently observed.

### 2.2 Native host contract

`packages/iyon-tui/src/transport/native/addon.ts:125-188` defines `NativeTuiHostContract`.

Relevant scheduling methods:

- `epochs()`
- `setDesiredViewRef(viewRef)`
- `clearViewStateBindings()`
- `flushPendingHosts(budget?, forceRetry?)`
- `nextWakeMs()`
- `advanceTime(milliseconds)`
- `pollTerminal()`
- `nextOutput()`
- `waitForOutput()`
- `resize()`
- `setTheme()`
- `exit()`
- `dispose()`.

The native host contract intentionally exposes no Rust `App`, `Component`, or callback object to TypeScript. The TS side exchanges opaque native View refs, generic host operations, and generic routed output.

### 2.3 Native frame epoch types

The TS wake broker normalizes native values to `bigint`:

```ts
NativeHostEpochs:
  host_id
  desired_structural_revision
  visible_structural_revision
  visible_frame_revision
  pending_epoch
  committed_epoch
```

`wake-broker.ts:456-480` rejects malformed epoch values:

- numbers must be non-negative safe integers;
- strings must contain only decimal digits.

This is a trust-boundary validation. Native Rust serializes all epoch values as decimal strings in `tui.rs:637-652`, so ordinary N-API calls preserve exact 64-bit values.

### 2.4 Runtime error contract

`runtime/error-channel.ts:3-26` defines:

```ts
FramePhase =
  "structural" | "content" | "frame" | "backend"

RuntimeFrameErrorCode =
  "FRAME_PREPARATION_FAILED"
  | "BACKEND_NOT_READY"
  | "BACKEND_IO_FAILED"
  | "SURFACE_DESYNCHRONIZED"
  | "LAYOUT_DID_NOT_CONVERGE"
  | "INTERNAL_INVARIANT"
  | "RUNTIME_POISONED"
  | "SOURCE_WAKE_FAILED"
```

Each error includes:

- host ID;
- attempted epoch;
- desired structural revision;
- phase;
- stable code;
- retryability;
- diagnostic text.

Automatic drains do not throw into the microtask. The error channel stores the latest record and sends it to:

1. the configured runtime error listener;
2. `globalThis.reportError`, if present;
3. `console.error` as the fallback.

`onRuntimeError()` permits one active listener. Registering a second listener replaces the first. A listener returning `true` consumes the error and suppresses the default reporter.

An explicit barrier converts the stored record into `TuiError` with stringified epoch metadata (`error-channel.ts:72-89`).

### 2.5 Native Rust host state contract

`crates/iyon-tui/src/application/host.rs:122-172` shows the central `HostInner` state:

- `running: HostRunning`
- `backend: HostBackend`
- committed `frame`
- optional `candidate_frame`
- optional asynchronous `presentation` receipt
- `frame_pending`
- candidate epoch and structural revision
- candidate content/state commit plans
- `failed_attempt`
- native clock `now`
- `headless`
- `closed`
- `environment`
- host/structural/frame/pending/committed epochs
- content dirty flag
- physical synchronization uncertainty
- retained-state registry
- content registry.

This is the authoritative native lifecycle record. The committed `frame` is the visible logical authority; candidate data remains provisional until the backend receipt and native completion protocol succeed.

### 2.6 Generic application kernel contract

`crates/iyon-tui/src/application/kernel.rs:42-67` defines `RunningApp`.

Important fields:

- application state;
- `Scene`;
- theme;
- component registry;
- output router;
- `SceneHost`;
- action queue;
- timer queue;
- global bindings;
- paste interceptors;
- deferred pastes;
- external ingress receiver;
- `dirty`;
- `body_dirty`;
- `exit_requested`.

`RunningApp::advance_ready()` is the native scheduler’s main application-level update step:

1. if exiting, closes ingress and clears actions/timers;
2. moves due timers to the front of the action queue;
3. executes due component ticks;
4. drains component outputs to actions;
5. processes up to `ACTION_BATCH_BUDGET = 128` actions;
6. marks application/view state dirty after each update;
7. drains outputs, deferred pastes, and newly due timers;
8. stops action processing if exit was requested.

### 2.7 Terminal backend contract

`crates/iyon-tui/src/terminal/backend.rs:6-48` defines:

```rust
type PresentReceipt = oneshot::Receiver<Result<()>>;

trait TerminalBackend {
    fn try_next_event(&mut self) -> Result<Option<TerminalEvent>>;
    fn viewport(&mut self) -> Result<Size>;
    fn begin_frame(&mut self, frame: &PreparedSceneFrame)
        -> Result<PresentReceipt>;
    fn position_after_final_frame(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
}
```

The backend is intentionally private to the native host. The only cross-language surface is the generic native host wrapper.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
packages/iyon-tui/src/index.ts
    └─ exports Tui
       └─ runtime/runtime.ts
          ├─ transport/native/addon.ts
          ├─ runtime/environment.ts
          │  ├─ runtime/native-resource-registry.ts
          │  └─ runtime/wake-broker.ts
          ├─ composition/execution.ts
          │  └─ composition/tracked-state.ts
          ├─ transport/structural/retained-dag.ts
          ├─ runtime/attachments.ts
          └─ generic control wrappers
             ├─ ViewState
             ├─ ContentPort
             ├─ TextInput
             ├─ ViewSlot
             └─ ScrollPane
```

```text
NativeTuiHost N-API wrapper
    └─ crates/iyon-tui-native/src/tui.rs
       └─ Box<TuiHost>
          └─ crates/iyon-tui/src/application/host.rs
             ├─ RunningApp / application/kernel.rs
             ├─ TuiEnvironment / application/environment.rs
             ├─ SceneHost
             ├─ retained-state registry
             ├─ content registry
             └─ TerminalBackend
                └─ TermwizBackend
                   ├─ terminal worker thread
                   ├─ crossterm EventReader
                   └─ TermwizPresenter
```

### 3.2 Create/destroy ownership

| Resource | Created by | Strong owner | Destruction trigger |
|---|---|---|---|
| JS `Tui` | `Tui.open()` | Caller/application | `close`, `exit`, eventual GC |
| Native `NativeTuiHost` | N-API constructor | JS `Tui` wrapper/native object | `dispose`, native object drop |
| Rust `TuiHost` | `NativeTuiHost::new` | `Box<TuiHost>` | native wrapper drop |
| `HostInner` | `TuiHost::open_in_environment` | `Arc<Mutex<HostInner>>` | final `Arc` owner drop |
| TS runtime environment | `runtimeEnvironment()` | global symbol in JS realm | realm/module lifetime |
| Native environment | `host_environment_for_env()` | N-API environment registry | N-API environment cleanup hook |
| TS retained root | first canonical `render(builder)` | `Tui.rootBuilder` + `RetainedExecutionRuntime` | replacement/dispose/close |
| Native retained root | root publication transaction | native ABI runtime/host retained scene | replacement, host close, ABI release |
| Host-owned controls | `Tui` factory methods | `ownedHandles` set | `disposeOwnedHandles` |
| Caller-owned detached History | caller | caller until attached | caller disposal |
| Source | environment/source wrapper | environment source registry | explicit source disposal/environment cleanup |
| terminal worker | `TermwizBackend::enter` | backend `JoinHandle` | `restore()` and worker join |

### 3.3 Weak references and lifetime barriers

The TS runtime registers the native host using a weak reference:

```ts
const owner = new WeakRef(this);
(commit) => owner.deref()?.commitVisibleAfterDrain(commit)
```

This prevents the broker’s commit callback from strongly retaining `Tui`.

`RuntimeHostRegistrationImpl` holds a `WeakRef<NativeFrameHost>` and a `FinalizationRegistry` registration (`wake-broker.ts:427-449`). Native host finalization unregisters its broker entry.

Rust `HostInner::Drop` performs two important final operations:

1. `self.content.dispose_all()`;
2. `self.environment.unregister_host(self.host_id)`.

This is necessary because host-bound handles such as `HostHistory` can keep the inner `Arc` alive after the public `TuiHost` wrapper is dropped (`application/host.rs:175-185`).

### 3.4 Frame and lease ownership

TS attachment state has separate desired and visible lease sets:

```text
desired leases
visible leases
superseded desired revisions
```

A root publication:

1. prepares new leases;
2. commits them to desired state when the native desired root is accepted;
3. keeps prior visible leases until a successful frame commit;
4. promotes the target revision only after native reports a visible commit;
5. releases superseded leases once the corresponding visible revision is known.

This mirrors native candidate/visible frame ownership and prevents a failed presentation from releasing resources still needed by the last visible frame.

### 3.5 Ownership diagram for a canonical frame

```text
TS builder / State write
        │
        ▼
RetainedExecutionScope dirty queue
        │
        ├─ evaluate producer
        ├─ prepare semantic publications
        └─ commit native desired root
                │
                ▼
NativeTuiHost.setDesiredViewRef
        │
        ├─ HostInner desired root
        ├─ desired structural revision++
        ├─ pending epoch++
        └─ native environment pending queue
                │
                ▼
TS EnvironmentWakeBroker microtask
        │
        ▼
Native TuiEnvironment::drain_pending_for
        │
        ▼
HostInner::flush_for_environment
        │
        ├─ content advance
        ├─ app timers/actions/ticks
        ├─ SceneHost frame preparation
        ├─ candidate state/content commits
        └─ backend begin_frame
                │
                ▼
Termwiz Present receipt
        │
        ▼
HostInner::commit_frame
        │
        ├─ committed frame becomes visible authority
        ├─ visible epochs advance
        ├─ History/state/content candidate promotion
        └─ TS commitVisibleAfterDrain callback
```

---

## 4. Execution paths and state transitions

## 4.1 Startup path

### TypeScript entry

`Tui.open()` (`runtime/runtime.ts:319-341`) performs:

1. Initial cancellation check:
   ```ts
   if (options.signal?.aborted) throw ...
   ```
2. Defaults dimensions to `80 × 24`.
3. Validates dimensions as integers in `1..65535`.
4. Resolves `NativeTuiHost` through `requireNativeClass`.
5. Constructs the native host:
   ```ts
   host = new Host(width, height, options.headless ?? false);
   ```
6. Constructs `new Tui(host, width, height)`.
7. Applies the optional theme.
8. Returns the runtime.

If construction or initial theme application throws:

- the primary error is normalized with `asTuiError`;
- if the `Tui` object exists, `tui.close()` is attempted;
- otherwise `host.dispose()` is attempted;
- cleanup failure is aggregated with the primary error.

The cancellation signal is not polled during native construction or theme setup. Because native construction is synchronous from the TS perspective, there is no internal await point at which a later abort could interrupt startup.

### TS runtime constructor

`Tui` construction (`runtime.ts:143-214`) performs:

1. Stores native host and dimensions.
2. Registers the host in the realm-wide `RuntimeEnvironment`.
3. Creates the attachment context from:
   - runtime resource registry;
   - environment token;
   - host registration token.
4. Eagerly creates one `RetainedExecutionRuntime`.
5. Registers testing/runtime access callbacks:
   - `flush`;
   - key/paste/resize enqueue;
   - screen rows/history rows;
   - cell style/cell position;
   - deterministic time advancement;
   - exit status.

The retained execution runtime is created eagerly even before the first canonical root render. Its `onBatchAbort` callback restores staged History sideband to the currently bound History.

The `createScopeProjection` callback creates native View slots for nested retained component scopes. These slots are framework plumbing, not user-level semantic slots.

### Native N-API constructor

`crates/iyon-tui-native/src/tui.rs:602-634`:

1. Applies default dimensions.
2. Converts dimensions to `u16`.
3. Gets an environment associated with the N-API `Env`.
4. Calls:
   ```rust
   TuiHost::open_in_environment(width, height, headless, environment)
   ```
5. Gets the native View ABI runtime pointer for the N-API environment.
6. Stores:
   - `Box<TuiHost>`;
   - `alive = true`;
   - ABI runtime pointer.

The environment registry is keyed by the raw N-API environment pointer (`tui.rs:122-146`). An N-API cleanup hook removes both the host environment and its content environment entry.

### Rust host startup

`TuiHost::open_in_environment()` (`application/host.rs:968-1044`) performs:

1. Rejects zero dimensions.
2. Selects backend:
   - `HostBackend::Headless(HeadlessSink)` for headless mode;
   - `HostBackend::Real(TermwizBackend::enter()?)` otherwise.
3. Constructs a generic `App`:
   - `host_init`;
   - `host_update`;
   - `host_view`;
   - default theme;
   - default History.
4. Calls `app.start(now)`.
5. Prepares the initial spacer frame.
6. Allocates `HostInner` with:
   - committed initial frame;
   - `frame_pending = true`;
   - all epochs initially zero;
   - empty candidate/receipt state.
7. Registers the host in `TuiEnvironment`, assigning `host_id`.
8. Calls `host.present_frame()` before returning.

Headless startup marks the initial frame as presented synchronously. Real startup calls `TermwizBackend::begin_frame()` and receives an asynchronous receipt; the native host can therefore return while the initial terminal presentation is still in flight.

If initial native presentation fails, the host is unregistered before returning the error.

### Real terminal worker startup

`TermwizBackend::enter()` (`terminal/termwiz/backend.rs:29-58`) creates:

- an `std::sync::mpsc` command channel;
- a bounded startup channel;
- a named worker thread `iyon-terminal`.

The worker (`terminal/termwiz/worker.rs:35-59`) performs:

1. `setup_terminal()`;
2. sends startup size back to the caller;
3. enters a blocking command loop;
4. best-effort finishes synchronized output;
5. restores the terminal after the command channel closes.

`setup_terminal()` (`worker.rs:61-103`) performs:

1. Termwiz capability probing.
2. System terminal opening.
3. Crossterm input-mode setup.
4. Terminal size query.
5. Cursor hiding.
6. Emission of `\r\n` line breaks for the current screen height to establish the inline main-screen viewport.
7. Initial full viewport painting through `TermwizPresenter`.

Only after the worker reports successful startup does `TermwizBackend::enter()` start the crossterm `EventReader`.

Startup cleanup handles both failure points:

- if the worker does not report startup, the worker is joined;
- if the input reader cannot start, a `Restore` command is sent, the response is awaited, and the worker is joined.

### Consequential startup observation

The native host has two asynchronous startup boundaries:

1. terminal worker startup is synchronously awaited;
2. the first logical frame’s terminal presentation is asynchronous.

Therefore, successful `Tui.open()` does not necessarily mean that the real terminal has completed the first `Present` receipt. `Tui.flush()` or the event/input driver is needed to complete the visible-frame barrier.

---

## 4.2 Canonical retained render path

### Entry and preconditions

`Tui.render()` (`runtime.ts:399-405`) dispatches by value type:

- function → `renderCanonical`;
- structural Scene value → `renderDirect`.

`renderCanonical()` (`runtime.ts:451-497`) performs:

1. signal check;
2. closed check;
3. retained execution drain;
4. reentrancy check;
5. wraps the caller builder in a producer:
   - calls `Scene.from(builder())`;
   - stages History binding;
   - returns `scene.body`.

On the first canonical render:

1. constructs a root publication target;
2. calls `OwnedBuilderRoot.start`;
3. the root producer evaluates synchronously;
4. retained execution prepares all nested publications;
5. the root target calls `prepareRootPublication`;
6. native desired structure is accepted during commit;
7. desired attachments are recorded;
8. host registration is marked pending;
9. `Tui.flush()` performs the explicit visibility barrier.

On later canonical renders:

1. the existing `OwnedBuilderRoot` replaces its producer;
2. the same retained execution root scope is re-driven;
3. successful publication follows the same desired-root/frame barrier;
4. a producer/evaluation failure restores the staged History sideband;
5. a later frame failure does not roll back the newly accepted desired revision.

### Root publication

`prepareRootPublication()` (`runtime.ts:224-277`) prepares:

- semantic resource leases;
- next `Scene`;
- retained root boundary publication.

The returned publication’s commit closure:

1. commits History binding before root publication;
2. commits the prepared structural root;
3. reads the native desired structural revision;
4. commits desired attachment leases at that revision;
5. marks the host pending;
6. updates `currentScene`.

The commit closure deliberately does not present the frame. Presentation is performed by the later environment/native frame drain.

### Direct render path

`renderDirect()` (`runtime.ts:499-548`) performs:

1. signal and closed checks;
2. retained execution drain;
3. mutation guard;
4. `Scene.from(scene)`;
5. semantic body validation;
6. History ownership/handle validation;
7. identity no-op check.

For the same body and effective History object:

- attachments are revalidated;
- staged History is updated;
- any retained builder root is disposed;
- `flush()` is called;
- no new structural publication is made.

For a changed direct Scene:

1. effective History is staged;
2. `prepareRootPublication()` is called;
3. desired root is committed;
4. root builder is disposed;
5. `flush()` makes the frame visible.

### Retained execution scheduling

`RetainedExecutionRuntime` (`composition/execution.ts:328-614`) has its own independent scheduler:

- `invalidate(scope)` adds one dirty scope to a queue;
- duplicate invalidation does not enqueue twice;
- a `queueMicrotask` is armed when `autoFlush` is enabled;
- an explicit `flush()` consumes/cancels the scheduled token;
- batches evaluate, prepare, and commit transactionally.

Evaluation and preparation failures:

- abort WIP child scopes/publications;
- preserve the previous committed output;
- restore dirty obligations;
- do not automatically retry.

This no-auto-retry rule is explicit in `execution.ts:556-561`: persistent producer failures must not create an infinite microtask loop.

The root `Tui` runtime adds a second barrier around this scheduler:

```text
Tui.render / Tui.flush / factory mutation
    └─ retainedRuntime.flush()
       └─ hostRegistration.flush()
```

Thus a caller-facing explicit barrier first makes TS retained execution coherent, then makes the native host frame visible.

---

## 4.3 Tracked-state write path

`StateSource.set()` (`composition/tracked-state.ts:72-92`) performs:

1. rejects writes from inside a component body;
2. applies `Object.is` change discipline;
3. updates the authoritative value;
4. invalidates every subscribed live scope.

A state read during retained evaluation (`tracked-state.ts:49-54`) links the active scope as a pending dependency. Subscriptions are promoted only after successful commit (`execution.ts:767-775`). Aborted evaluations retain the prior committed dependency set.

The complete path is:

```text
state.set(value)
    ↓
StateSource.publish()
    ↓
scope.runtime.invalidateFromState(scope)
    ↓
RetainedExecutionRuntime.invalidate(scope)
    ↓
queueMicrotask(auto flush)
    ↓
evaluate scope
    ↓
prepare structural publication if output changed
    ↓
root/slot publication commit
    ↓
host registration markPending
    ↓
EnvironmentWakeBroker native drain
    ↓
HostInner frame candidate / presentation / commit
```

A State write alone does not directly present a terminal frame. It first causes a retained scope output to be regenerated; only a successful structural publication or another native invalidation makes the native host pending.

### Failure behavior

If the component producer throws:

- the previous output and frame remain authoritative;
- the dirty obligation remains queued;
- no automatic retry is armed;
- a later explicit `Tui.flush()`, later State write, or other scheduling trigger can re-drive the scope.

`Tui.render()` explicitly drains pending retained work before accepting another render, so callers do not observe an old retained output when they invoke a new render.

---

## 4.4 Content mutation and wake/update path

Source mutations are accepted by native content objects and then wake subscribed native hosts.

TS `ContentSource.append/replace/clear/seal/truncateHead()` (`api/content/retained.ts:380-405`) calls native functions with `sourceWake`.

`sourceWake()` (`api/content/retained.ts:210-212`) calls:

```ts
runtimeEnvironment().wakeBroker.markEnvironmentPending();
```

The Source mutation path is intentionally split:

```text
Source mutation accepted
    ├─ Source revision/storage is updated natively
    ├─ subscriber hosts are marked pending natively
    └─ TS environment broker is nudged to schedule a drain
```

The accepted mutation is not rejected merely because a post-acceptance host wake fails. Rust documents this explicitly in `application/content.rs:506-515` and `2526-2582`.

Native `ContentSourceRegistry::finish_mutation()`:

1. captures host-grouped subscriptions;
2. validates source/connector generations;
3. batches dirty content entries per host;
4. calls `HostInner::mark_content_pending_batch`;
5. collects `schedule_environment_drain`;
6. records wake failures in the environment-owned `SourceWakeFailureChannel`;
7. reuses the per-source wake scratch storage.

The Source wake failure is delivered later as `SOURCE_WAKE_FAILED`, rather than making the already-accepted append look rejected.

### Content control mutation

`ContentPort.deactivate()` and `ContentConnector.activate()` use the native wake result (`api/content/retained.ts:206-208`). If the native result indicates that an environment drain should be scheduled, the wrapper calls the host’s `requestWake`, which is `hostRegistration.markPending()`.

### Native state control mutation

`ViewState` methods use the same pattern (`api/view/retained-state.ts:211-214`):

1. invoke native `setGeometry`, `setPresentation`, `setStyleState`, or clear operation;
2. inspect the native wake bit;
3. call `requestWake()` if the native state changed the host’s pending work.

---

## 4.5 Native environment wake path

`EnvironmentWakeBroker` (`runtime/wake-broker.ts:127-424`) is one broker per JavaScript realm.

### Marking pending

`markPending()`:

1. ignores unregistered hosts;
2. increments counters;
3. adds the host ID to the pending set;
4. increments `pendingGeneration`;
5. distinguishes latch winner from already-latched wake;
6. schedules exactly one microtask.

The broker’s microtask is edge-triggered:

```text
pending host mutation
    ├─ if no drain/microtask active: queue one microtask
    └─ otherwise: retain pending ID and generation
```

### Automatic drain

`drainAutomatically()`:

1. snapshots `pendingGeneration`;
2. calls `drain(false)`;
3. re-schedules if:
   - native reports `rearm`, or
   - a new edge arrived while native was running;
4. if native reports `waiting_for_presentation`, schedules a timer-based presentation poll instead of a microtask spin;
5. catches unexpected failures and puts a structured failure into the host error channel.

Automatic drains deliberately do not throw.

### Explicit barrier

`flush(registration)`:

1. adds the host to the pending set;
2. captures the host’s pending epoch;
3. cancels a queued automatic microtask;
4. performs up to `MAX_EXPLICIT_DRAINS = 64` native drains with `forceRetry = true`;
5. throws any stored host error after each drain;
6. succeeds once native `committed_epoch >= capturedEpoch`;
7. otherwise stores and throws `FRAME_PREPARATION_FAILED`.

The explicit barrier is therefore stronger than automatic scheduling:

- it forces retry-blocked work once;
- it waits for the requested epoch;
- it surfaces deferred errors synchronously.

### Fairness

The broker chooses a pending driver using `fairCursor` (`wake-broker.ts:412-423`). Native `TuiEnvironment` itself also maintains a queued/pending set and fair drain cursor conceptually through its pending deque.

The TypeScript broker does not mirror native subscriptions. Native environment state remains authoritative for the complete affected-host set.

---

## 4.6 Native frame preparation and visible commit

### Pending epoch establishment

`HostInner::environment_pending_epoch()` (`application/host.rs:1682-1688`) observes a dirty native kernel even if no explicit native pending epoch has been created:

```text
pending_epoch == committed_epoch
AND (running.is_dirty() OR pending source cleanup)
    → ensure_pending()
```

This matters for mutations such as theme changes, direct native control invalidation, and the initial bootstrap frame.

### `flush_pending_frame()`

`HostInner::flush_pending_frame()` (`application/host.rs:2242-2351`) proceeds in this order:

1. Reject closed hosts.
2. Admit a retry epoch for blocked Source cleanup.
3. Reconcile an existing completed candidate before starting a newer preparation pass.
4. Clear previous failed-attempt metadata.
5. Apply test-only injected frame failure if configured.
6. Advance content delivery/smoothing.
7. Mark content dirty items.
8. Run `RunningApp::advance_ready(now)`.
9. Refresh desired state bindings when invalidated components changed.
10. Ensure a pending epoch if application state is dirty.
11. Poll an in-flight presentation receipt if appropriate.
12. Render a new candidate if the application/native epoch is pending.
13. Present bootstrap frame if still pending.
14. Commit any candidate whose presentation completed.

### Candidate frame

`HostInner::render()` (`application/host.rs:1870-1959`) creates:

- content projection candidate;
- retained-state candidate overlay;
- complete `PreparedSceneFrame`;
- candidate state/content commit plans;
- candidate epoch/revision metadata.

It then calls `present_frame()`.

The candidate is not visible until:

1. the backend accepts the frame;
2. the asynchronous receipt succeeds;
3. `commit_frame()` succeeds under environment completion authority.

### Presentation receipt

`present_frame()` (`application/host.rs:1961-2023`) first checks an existing receipt:

- `Ok(Ok(()))`: previous presentation completed;
- `Empty`: receipt remains in flight;
- `Closed`: backend reply was lost, classified `BACKEND_NOT_READY`.

For a new real frame:

- `TermwizBackend::begin_frame` sends `TerminalCommand::Present`;
- receipt is stored;
- `frame_pending` is cleared.

For headless mode:

- no receipt is needed;
- `frame_pending` is cleared immediately.

### Visible commit

`commit_frame()` (`application/host.rs:2026-2114`) runs through `TuiEnvironment::with_host_completion()`.

The environment mutex remains held across:

1. content prepared-commit;
2. state prepared-commit;
3. content candidate finalization;
4. native History synchronization recovery if needed;
5. committed frame promotion;
6. visible structural/frame epoch updates;
7. pending/blocked queue bookkeeping.

The commit updates:

- `frame`;
- `visible_structural_revision`;
- `visible_frame_revision`;
- `committed_epoch`;
- `content_dirty` when no newer pending epoch exists.

If a newer operation arrived while the candidate presentation was in flight, `pending_epoch != candidate_epoch`; the candidate still commits, but the host remains pending for the newer work.

---

## 4.7 Component ticks and application timers

### Component ticks

`TickScheduler` (`component/tick.rs:55-305`) tracks registrations by mounted `ComponentId`.

Mount synchronization:

- mounting activates a deadline at `now + interval`;
- unmounting deactivates the registration;
- remounting resets the deadline;
- capability changes update the callback and reset only when the interval changes.

`next_deadline()` returns the earliest mounted tick deadline.

`tick_due_with_events()`:

1. snapshots all due components in mount order;
2. calls each registered callback;
3. ORs the returned dirty values;
4. records changed components;
5. schedules each next tick at `now + interval`.

It does not catch up multiple missed intervals. A delayed scheduler invocation produces one tick and schedules from the current time.

`SceneHost::tick_due()` (`scene/host.rs:963-980`) converts changed tick components into component invalidations. This is necessary because a tick callback mutating a component registry does not automatically imply a retained scene reconciliation.

### View-slot animation ticks

`MountedViewSlot` registers a generic `16ms` component tick (`application/host.rs:608-626`). The slot’s own `ViewSlotState.interval` determines whether its animation advances.

The slot behavior is:

- fewer than two animation frames → no animation;
- first tick without `last_tick` anchors the clock and reports changed;
- later tick advances only when `now - last_tick >= interval`;
- replacement frames may be staged at a cycle boundary;
- when frame index wraps to zero, pending frame arrays are promoted.

The slot mutation API does not ask TypeScript to drive every tick. TypeScript supplies semantic frames and intervals; Rust owns timer scheduling and frame selection.

### Application timers

`TimerQueue` (`application/timer.rs:21-90`) stores one-shot entries with:

- globally monotonic `TimerHandle`;
- deadline;
- insertion sequence;
- action.

Due timers are selected by `(deadline, sequence)` order.

`RunningApp::advance_ready()`:

- collects due timers to the front before ticks/actions;
- collects newly due timers to the back after each action;
- processes at most 128 actions per pass;
- clears timers when exiting.

### `nextWakeMs()`

`TuiHost::next_wake_ms()` (`application/host.rs:1242-1259`) computes the minimum of:

- application timer deadline;
- mounted component tick deadline;
- content next wakeup.

If no deadline exists, it returns `16ms`. It clamps nonzero deadline distances to at least `1ms`.

The TS polling path further clamps this to `1..16ms` (`runtime.ts:362-370`).

---

## 4.8 Event/input path

### Native event wait without a signal

`Tui.nextEvent()` (`runtime.ts:347-360`) calls `host.waitForOutput()` when no signal is provided.

The native wait loop (`application/host.rs:1510-1535`) repeatedly:

1. returns `None` if the host is exited;
2. refreshes headless time from `Instant::now()`;
3. calls `poll_terminal()`;
4. returns a queued routed output if available;
5. sleeps until the minimum next wake deadline, bounded to `1..16ms`.

### Abortable TS event wait

With an `AbortSignal`, TS uses `pollOutput()`:

1. checks closed state and signal;
2. calls `host.pollTerminal()`;
3. calls `host.nextOutput()`;
4. waits with a cancellable `setTimeout`;
5. repeats until output, closure, or abort.

The cancellation promise rejects with a `cancelled` TUI error.

### Native input pump

`TuiHost::poll_terminal()` (`application/host.rs:1471-1508`) pumps at most `INPUT_PUMP_BUDGET = 32` events per call.

It stops early if:

- the host has pending actions;
- the backend has no event;
- a terminal event source is exhausted;
- a routed action is generated.

Events are:

- key → `RunningApp::dispatch_key`;
- paste → `RunningApp::dispatch_paste`;
- resize → invalidates the running frame.

After input handling, it calls `advance_and_render()`.

The native side therefore keeps keystroke decoding, focus, paste interception, and component dispatch in Rust. TypeScript receives only generic routed output records.

---

## 4.9 Resize path

### TypeScript resize

`Tui.resize()` (`runtime.ts:716-727`) validates dimensions, drains retained execution, checks mutation state, calls `host.resize()`, and updates TS `width`/`height` only after the native call succeeds.

### Native resize

`NativeTuiHost::resize()` converts values to `u16` and calls `TuiHost::resize()` (`crates/iyon-tui-native/src/tui.rs:1007-1017`).

`TuiHost::resize()` (`application/host.rs:1391-1408`):

- rejects zero dimensions;
- updates `HeadlessSink` dimensions only in headless mode;
- invalidates the frame;
- synchronizes real time;
- advances/renders.

### Consequential uncertainty

For `HostBackend::Real`, `TuiHost::resize()` does not directly send a terminal resize command. `TermwizBackend::viewport()` returns its cached `size`, and that size is updated in `map_event()` only when a real terminal `Event::Resize` arrives (`terminal/termwiz/backend.rs:85-90`).

Therefore, statically:

- explicit `Tui.resize(width,height)` updates TS metadata;
- headless rendering uses the supplied dimensions;
- real rendering appears to continue using the backend’s last event-reported size until a terminal resize event updates it.

This may be intentional as a test/simulation API, or it may represent a real-host semantic gap. It requires execution or product/API clarification.

---

## 4.10 Close path

`Tui.close()` (`runtime.ts:822-840`) is idempotent.

Sequence:

1. return if already closed;
2. reject close during retained protocol mutation;
3. set `closed = true`;
4. close the shared History liveness token;
5. dispose retained execution and runtime resources;
6. call native `host.dispose()`;
7. aggregate all cleanup errors.

`disposeRetainedExecution()` (`runtime.ts:784-814`) intentionally tears down before native host disposal:

1. unregister TS host from wake broker;
2. cascade native content-resource disposal while host is still alive;
3. remove runtime error reporter;
4. dispose attachment bindings;
5. clear native ViewState bindings;
6. invalidate runtime resource registry entries for the host;
7. dispose owned handles, retrying handles that are temporarily still leased;
8. dispose the root builder;
9. dispose retained execution, invalidating queued microtasks;
10. close the retained root boundary;
11. clear staged/bound History references.

The comments identify the invariant: no environment work may outlive the `Tui` and target resources.

### Native `dispose`

`NativeTuiHost::dispose()` (`crates/iyon-tui-native/src/tui.rs:742-750`):

1. atomically changes `alive` from true to false;
2. aborts all native View ABI edit transactions;
3. calls `TuiHost::close()`.

Even if `TuiHost::close()` returns an error, the native wrapper’s `alive` flag is already false. Subsequent native wrapper calls return `ION_DISPOSED_HANDLE`.

### Rust `TuiHost::close()`

`TuiHost::close()` (`application/host.rs:1558-1609`) has two main paths.

If already closed:

- unregisters the host again defensively;
- returns success.

If an in-flight presentation receipt is lost:

1. marks/handles the host as closed;
2. disposes ViewStates and content;
3. restores terminal mode best-effort;
4. unregisters the host;
5. returns the original presentation error, or aggregates restore failure.

Normal close:

1. waits for an in-flight receipt;
2. replaces the application body with a spacer;
3. clears retained native views;
4. disposes ViewStates and content;
5. marks `closed = true`;
6. restores the real terminal if present;
7. unregisters the native host environment entry.

Close does **not** prepare a final frame or transfer the final logical rows to terminal scrollback.

### Rust drop behavior

`TuiHost::Drop` calls `close()` only when its `Arc<Mutex<HostInner>>` strong count is one (`application/host.rs:1662-1667`). Host-bound handles may keep the inner state alive. The final `HostInner::Drop` still disposes content and unregisters the host.

---

## 4.11 Exit path

`Tui.exit()` (`runtime.ts:843-878`) differs from close because it creates and presents a final exit frame.

Sequence:

1. return if already closed;
2. reject exit during retained protocol mutation;
3. mark History liveness closed;
4. call native `host.exit()` while content bindings still exist;
5. dispose retained execution;
6. set `closed = true`;
7. clear current Scene;
8. aggregate errors.

If native `host.exit()` fails:

- `exitFailed` is set;
- the error is retained;
- retained runtime cleanup still runs;
- native `host.dispose()` is called as a fallback;
- exit remains terminal even if restoration fails.

### Native `TuiHost::exit()`

`TuiHost::exit()` (`application/host.rs:1189-1239`) performs:

1. if already closed, unregister and return;
2. waits for any earlier presentation;
3. calls `running.host_exit()`;
4. `host_exit()` sets `exit_requested`, closes ingress, and marks the kernel dirty;
5. prepares/renders the final frame;
6. waits for the final presentation;
7. extracts non-empty final physical rows;
8. headless mode appends final rows to `HeadlessSink.history`;
9. real mode calls `position_after_final_frame()`;
10. restores the real terminal;
11. disposes ViewStates and content;
12. sets native `closed = true`;
13. unregisters the host.

`position_after_final_frame()` ends synchronized output if active and restores canonical cursor/attribute state before terminal restoration.

### Exit lifetime observation

On a successful `Tui.exit()`:

- the TS runtime becomes closed;
- the native Rust `HostInner` becomes closed and unregisters;
- the terminal worker is restored and joined;
- but `Tui.exit()` does not call `NativeTuiHost.dispose()` on the success path.

The native wrapper’s `alive` flag therefore remains true until the native wrapper is eventually dropped, although the underlying `TuiHost` is already closed and its environment membership is gone. This appears to be an intentional distinction between “terminal exit completed” and “native wrapper disposed,” but it leaves the wrapper object resident until GC or another disposal path. It is worth confirming whether the intended contract is:

- exit restores/closes the host but leaves the inert native wrapper alive; or
- exit should also mark the wrapper disposed immediately.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path matrix

| Semantic operation | Production path | Selection condition | Success boundary | Failure behavior |
|---|---|---|---|---|
| `Tui.open()` | TS `Tui.open` → N-API `NativeTuiHost::new` → `TuiHost::open_in_environment` | Always | native host constructed and initial logical frame accepted | cleanup host/Tui; aggregate cleanup failures |
| Real terminal startup | `TermwizBackend::enter` → worker `setup_terminal` | `headless !== true` | worker reports `Startup` | restore terminal and join worker |
| Headless startup | `HeadlessSink` | `headless === true` | initial frame prepared synchronously | native setup error |
| Canonical first render | retained root producer → `OwnedBuilderRoot.start` → root publication → native desired root | `typeof sceneOrBuilder === "function"` and no root yet | retained commit + native desired structure + later visible frame commit | evaluation/prepare abort preserves prior state; frame failure preserves desired root and reports |
| Canonical subsequent render | `replaceProducer` on existing root | canonical root exists | same retained scope/root transaction | producer rollback restores staged History; frame failure leaves desired revision retryable |
| Direct Scene render | `Scene.from` → `prepareRootPublication` | non-function Scene value | desired root commit + frame commit | malformed scene/attachment/History errors before mutation where possible |
| Same direct Scene render | attachment validation + dispose root builder + flush | same body and History identity | no structural publication; explicit barrier | attachment validation or barrier failure |
| Tracked State write | `StateSource.set` → scope invalidation → TS microtask → retained execution | scope read State during prior successful evaluation | retained scope commit and frame visibility | no automatic retry after evaluation/prepare failure |
| Native ViewState write | ViewState wrapper → native state patch → host wake bit → broker | host-bound ViewState | host candidate/frame commit | native validation error synchronously; runtime frame errors deferred through channel |
| Source append/replace | native Source mutation → host-grouped wake → source wake channel → environment drain | active Source subscribers | Source revision accepted first; content appears on frame commit | append remains accepted; wake failure recorded as `SOURCE_WAKE_FAILED` |
| Content Connector activate/deactivate | native control operation → wake bit → host registration pending | Connector/Port control state change | next content/frame drain | native lifecycle error synchronously |
| ViewSlot frame mutation | slot setter → component invalidation → immediate native `advance_and_render` | host-bound slot | candidate/frame commit | component/frame error; retired component remains deferred |
| ViewSlot periodic animation | mounted component tick every 16ms → slot interval check | slot has two or more frames | slot view revision + next frame | no advancement before interval; no TS per-tick calls |
| Application one-shot timer | TimerQueue deadline → action queue → update callback | scheduled timer due | update/action batch and frame | update error becomes frame preparation failure |
| Component tick | TickScheduler earliest deadline → callback → component invalidation | component mounted and tick capability active | component snapshot/frame commit | callback returns dirty or not; callback panic/error handling is constrained by callback bridge |
| Key input | terminal EventReader → `poll_terminal` → native dispatch/focus/commands | real backend event | routed output or component state/frame update | terminal input closure/error returned from poll path |
| Paste input | terminal paste event or `forwardPaste` → native intercept/dispatch | paste event | routed output/component update | dispatch error returned synchronously |
| Resize event | terminal `Event::Resize` updates backend cached size → running frame invalidation | OS terminal resize | frame prepared using new viewport | terminal event/backend error |
| Explicit resize API | TS metadata update → native host resize | caller invokes `resize` | headless dimensions update; frame invalidation | TS metadata only updates after native call succeeds |
| `Tui.flush` | retained runtime flush → TS broker explicit drain → native environment flush | caller requests read-your-writes | target pending epoch committed | stored runtime error thrown as `TuiError`; bounded failure produces retryable preparation error |
| Automatic wake | broker queueMicrotask → native `flushPendingHosts(false)` | pending edge and no latch | commit callback updates visible TS leases/root | no throw; latest error stored/reported |
| `Tui.close` | TS teardown → native dispose → Rust close | caller closes without final frame | terminal restored/host unregistered | cleanup continues and aggregates errors |
| `Tui.exit` | native final frame → final position/restore → TS teardown | caller requests terminal exit | final frame and terminal restoration | fallback native dispose after exit failure |

### 5.2 Cache miss, retry, recovery, compatibility

The source distinguishes several non-equivalent cases:

- **Retained materialization refusal** is an explicit failure, not a fallback to an older transport (`retained-dag.ts:185-197`).
- **Automatic frame failure** is retained and retry-blocked; it does not spin.
- **Explicit flush** force-retries retry-blocked hosts.
- **In-flight presentation** is polled later; it does not start a second candidate.
- **Physical synchronization uncertainty** causes the next successful frame to invoke native History synchronization recovery.
- **Host lock poisoning** blocks the host while allowing unrelated hosts in the same environment to drain.
- **Source wake failure** is a post-acceptance diagnostic and does not roll back Source bytes.
- **Native terminal worker stopped** is classified as `BACKEND_NOT_READY`, not ordinary retryable I/O.
- **Unknown native error code** is converted by TS to `INTERNAL_INVARIANT`.

No production route in the inspected retained root path silently selects the pre-T13 transport cascade. The retained DAG comments identify it as the single production structural architecture.

### 5.3 Failure masking and preservation

The implementation generally preserves the right authority:

- a failed TS evaluation preserves the last committed scope output;
- a failed root preparation preserves the previous native desired root;
- a failed native frame preparation preserves `HostInner.frame`;
- a failed presentation preserves the logical frame and candidate metadata is discarded;
- a lost presentation receipt marks physical state uncertain;
- an accepted Source mutation remains accepted even when subscriber wake fails;
- cleanup attempts continue across multiple handles and aggregate final errors.

One deliberate masking boundary is automatic error delivery: a broker microtask cannot throw to its caller, so it routes the failure through `RuntimeErrorChannel`. The error remains available to an explicit barrier.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Scheduling units

| Unit | Frequency/trigger | Owner | Work |
|---|---|---|---|
| Retained scope evaluation | State invalidation, producer replacement | TS `RetainedExecutionRuntime` | synchronous producer body and dependency capture |
| TS retained flush | one microtask per invalidation edge, or explicit | TS runtime | evaluate/prepare/commit dirty scopes |
| Native environment drain | one broker microtask per pending edge; bounded by host budget | TS broker + native environment | drain pending native hosts |
| Native frame preparation | per dirty host epoch | `HostInner` | content advance, actions, ticks, scene resolve/layout/paint |
| Terminal presentation | per prepared candidate | terminal worker | diff/full paint or synchronized scrollback output |
| Presentation poll | when receipt is pending | TS broker timer or native event wait loop | wait for backend acknowledgment |
| Component tick | per registered component deadline | native `TickScheduler` | tick callback, component invalidation |
| Application timer | one-shot deadline | native `TimerQueue` | enqueue action |
| Input pump | each `poll_terminal` call, max 32 events | native host | decode and dispatch input |
| Read-your-writes barrier | caller invokes `flush` or testing inspection | TS runtime | retained flush + native forced drain |

### 6.2 TS retained queue behavior

`RetainedExecutionRuntime` uses a level-triggered dirty queue:

- one scope appears once even under duplicate invalidations;
- queued scopes are sorted parent-before-child;
- child scopes reached inline by a dirty parent are not double-committed;
- dirty obligations are restored after evaluation/preparation abort;
- scheduled microtask tokens use a generation counter;
- explicit flush cancels the stale automatic token;
- disposal invalidates queued microtasks before dropping scopes.

This prevents:

- duplicate work from multiple State writes in one turn;
- stale auto-retry after an explicit flush failure;
- use of disposed scopes after runtime teardown.

### 6.3 Native environment queue bounds

Native `TuiEnvironment` maintains:

- `pending: VecDeque<u64>`;
- `pending_set`;
- `queued`;
- `retry_blocked`;
- `waiting_for_presentation`;
- `wake_latched`;
- `wake_epoch`.

Automatic native drains use the requested budget, normally `32` (`wake-broker.ts:62-63`; native N-API validates budgets from `1..1024`, `tui.rs:687-703`).

The TS explicit barrier makes up to 64 drain attempts. The native environment uses the pending queue and retry-blocked state to ensure persistent failure does not continuously requeue.

### 6.4 Presentation polling

If a native frame was submitted but its receipt is not ready, the native drain report sets `waiting_for_presentation`.

The TS broker schedules a `setTimeout(..., 1)` poll (`wake-broker.ts:378-389`) rather than another microtask. This is a significant anti-busy-loop mechanism.

The corresponding `tui_perf13_a.test.ts` case is explicitly named:

> `polls asynchronous presentation receipts without a microtask spin`

### 6.5 Invalidation scopes

Observed invalidation categories include:

- retained State dependency paths;
- root structural changes;
- component registry revisions;
- content dependency paths;
- theme presentation;
- ViewState geometry/presentation;
- text input and slot component revisions;
- History updates;
- Source smoothing/projection progress;
- terminal resize.

`SceneHost::invalidate_theme()` preserves layout where metrics are unaffected but invalidates paint/content-related entries (`scene/host.rs:430-469`).

`SceneHost::invalidate_root()` discards retained derived structures while preserving only dependency-local cache facts that remain valid (`scene/host.rs:793-862`).

### 6.6 Per-append, per-tick, per-frame work

#### Source append

Per Source mutation:

1. Source storage/revision update;
2. capture subscriber groups;
3. one grouped host attempt per subscribed host;
4. per-host dirty item collection;
5. host pending epoch increment;
6. environment wake;
7. later content advance/projection during frame preparation.

The source wake batch reuses scratch storage and attempts all eligible hosts even if one fails (`application/content.rs:2526-2585`).

#### Smoothing tick

Content smoothing progress is advanced in `flush_pending_frame()` by `content.advance(self.now)`. Any progressed content creates dirty content records and requires a real candidate frame so projection, measurement, viewport handling, and paint observe the new frontier (`application/host.rs:2275-2312`).

#### Frame

A candidate frame can include:

- retained-state candidate overlay;
- content projection candidate;
- SceneHost resolve/layout/paint;
- component retirement after successful reconciliation;
- state/content commit plans;
- backend submission.

The committed frame is not replaced until presentation success and native completion bookkeeping succeed.

#### Terminal presentation

Termwiz uses:

- full repaint on unknown surface state;
- screen diffing for known surfaces;
- synchronized output around native scrollback insertion;
- explicit cursor/attribute canonicalization;
- model-only `ScrollRegionUp` for its in-memory presented surface;
- ordinary CRLF output to create actual native terminal scrollback.

`TermwizPresenter::known = false` after terminal write failure, forcing a later full repaint.

### 6.7 Counters and instrumentation

TypeScript wake counters (`wake-broker.ts:65-97`):

- `pending_marks`
- `wake_latch_wins`
- `wake_already_latched`
- `microtasks_queued`
- `drains`
- `hosts_attempted`
- `frames_committed`
- `automatic_errors`
- `rearm_count`
- `explicit_barriers`
- `explicit_barrier_failures`

The broker also maintains a bounded 256-event trace when `Bun.env.PERF_RUNTIME_TRACE === "1"`.

Retained execution counters (`composition/execution.ts:256-300`):

- scope mounts/unmounts;
- body calls;
- prop skips;
- State invalidations;
- dirty enqueues and duplicates;
- no-op/changed outputs;
- flush passes;
- commit batches/aborts;
- exact View reuse/new View creation.

Retained structural counters (`transport/structural/retained-dag.ts:111-155`) measure:

- native hint hits/misses;
- NodeId promotion;
- semantic node inspection;
- child visits;
- materializer calls;
- ref words/byte payloads;
- scratch reuse;
- stale ref retries;
- host mutations.

Rust-side counters include content wake groups and frame/scene counters through `crate::perf`, with native N-API `tuiPerfReset`/`tuiPerfSnapshot` available under `perf-counters`.

No values were collected in this report.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript scheduling tests

`packages/iyon-tui/tests/tui_perf13_a.test.ts` directly protects the scheduler contract:

1. validates and leases semantic attachments during prepare;
2. rejects duplicate attachment use and restores prepare leases;
3. rejects invalid attachments before visible mutation;
4. coalesces automatic wakes and retries only at an explicit barrier;
5. preserves a Source wake failure in the failed host channel without spinning;
6. polls asynchronous presentation receipts without a microtask spin;
7. separates desired structural publication from visible frame commit.

These are highly relevant architecture tests because they distinguish:

- desired vs visible;
- automatic vs explicit scheduling;
- accepted Source data vs failed host wake;
- delayed receipt vs active microtask work.

### 7.2 Real-time loop test

`packages/iyon-tui/tests/tui_realtime.test.ts:15-33` verifies that:

- a ViewSlot animation progresses without manual clock advancement;
- an event wait drives the native runtime;
- a frame changes during real-time operation;
- close remains usable in cleanup.

The test uses `AppHarness`, which opens headless but drives the host with real wall-clock waits during `nextEvent()`.

### 7.3 Runtime/event tests

`packages/iyon-tui/tests/tui_runtime.test.ts` protects:

- native text input edits;
- output routing;
- cancellation of pending `nextEvent`;
- idempotent close.

`packages/iyon-tui/src/testing/index.ts` provides the test contract:

- `pressKey`;
- `paste`;
- `advance`;
- `screenRows`;
- `nativeHistoryRows`;
- `styleAt`;
- deterministic `now`;
- inspection barriers that flush retained/native zero-time work.

### 7.4 Native frame and lifetime tests

Relevant TS tests include:

- `tui_perf13_d.test.ts`
  - Source ownership independent from host teardown;
  - cross-host/duplicate ContentPort rejection;
  - visible Connector preservation across candidate failure;
  - stable native lifecycle error codes;
  - deferred cleanup status mapping.
- `tui_perf13_h.test.ts`
  - accepted direct-FFI revision/wake hint;
  - release of shared Source subscriptions on multi-host teardown;
  - repeated host/Connector ownership cycles;
  - invalidation of host-owned content handles after owner teardown.
- `tui_retained_scene_regressions.test.ts`
  - state dependency invalidation before structural publication;
  - theme/content refresh ordering;
  - state values through ViewSlot replacement.
- `tui_harness.test.ts`
  - native snapshots and input dispatch;
  - ViewSlot animation;
  - style and Unicode cell positions.

### 7.5 Rust tests

`crates/iyon-tui/src/component/tick_tests.rs` establishes:

- no tick before deadline;
- tick at exact deadline;
- multiple due components in mount order;
- interval changes without complete graph rescans;
- unmount deactivation;
- remount deadline reset;
- earliest-deadline selection;
- zero-interval rejection.

`crates/iyon-tui/src/terminal/termwiz/presenter.rs` tests establish:

- synchronized output begin/end balance;
- native insert failures close synchronized output;
- full presentation failures close synchronized output;
- resize cleanup closes synchronized output before geometry changes;
- idempotent synchronized-output finishing;
- exact differential rendering;
- native scroll model parity;
- attribute reset around native scrollback.

### 7.6 Observability gaps

The source has strong counters and trace hooks, but ordinary public runtime callers do not appear to receive:

- current pending queue size;
- current retry-blocked host IDs;
- whether a presentation receipt is in flight;
- last runtime error record without installing a listener;
- native environment wake epoch through the public `TuiRuntime` API;
- terminal worker state.

The explicit error listener and barrier are sufficient for normal handling, but diagnosis of a host that is waiting for presentation or retry-blocked requires test/private hooks or source-level instrumentation.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Three scheduler authorities are intentionally distinct

The repository has three different concepts of “pending”:

1. TS retained scope dirty queue.
2. TS environment broker pending host IDs.
3. Native host/environment pending epochs and queues.

They must not be collapsed:

- a TS scope can be dirty before any native structure is accepted;
- a native desired structural root can be accepted before any visible frame exists;
- a native host can be waiting for a terminal receipt while no immediate runnable work exists;
- a Source mutation can be accepted natively while TS only receives a wake hint.

The source comments consistently preserve this distinction.

### 8.2 Desired structure is not visible structure

The canonical root publication path is explicitly two-phase:

```text
TS retained commit
    → native desired structural root
    → later native frame preparation
    → backend receipt
    → visible structural/frame commit
```

`runtime.ts:218-222`, `retained-dag.ts:1705-1709`, and `application/host.rs:2026-2114` all reinforce this model.

A failed frame does not roll back the desired native structural root. The desired root remains retryable while the previous visible frame remains authoritative.

### 8.3 Environment lock spans visible promotion

`TuiEnvironment::with_host_completion()` holds the environment mutex over visible promotion and queue completion (`application/environment.rs:339-414`).

This closes a race in which:

1. a candidate frame is promoted;
2. a concurrent mutation advances the host epoch;
3. completion bookkeeping reacquires a separately poisonable environment lock;
4. the newer epoch is lost or queue capacity changes unexpectedly.

The implementation reserves queue capacity before invoking the commit closure.

### 8.4 Native and TypeScript environment identity are separate layers

TS has one `RuntimeEnvironment` per JavaScript realm (`runtime/environment.ts:39-45`).

Native N-API host environments are keyed by N-API `Env` and managed through Rust static registries (`crates/iyon-tui-native/src/tui.rs:42-146`).

In the normal single-worker/single-N-API-environment case, these align operationally. In multi-worker or unusual module-loader arrangements, they are not literally the same identity object. Source subscriptions and native content identities use native environment slots/generations, while TS attachment ownership uses JS realm tokens.

This is a valid boundary, but any future multi-realm sharing design would need explicit mapping rules.

### 8.5 Native errors are more numerous than TS stable codes

Rust can produce error codes not enumerated in `RuntimeFrameErrorCode`, including:

- `HOST_LOCK_POISONED`;
- `CONTENT_SCHEDULER_FAILED`;
- `HISTORY_TRANSFER_FAILED`;
- `HISTORY_TRANSFER_FAILED`-related diagnostics;
- content-specific failure strings.

`wake-broker.ts:524-539` maps unknown codes to `INTERNAL_INVARIANT`.

This is intentionally safer than silently treating unknown native protocol errors as ordinary retryable frame failures, but it loses the more specific code at the TS error boundary. The parent should treat this as a diagnostic-schema coupling, not as a transport fallback.

### 8.6 Source acceptance and wake delivery are deliberately decoupled

The Source implementation does not return an ordinary mutation error after accepting a revision. That prevents callers from retrying and duplicating bytes.

The tradeoff is that the caller may see a successful Source mutation while a host has not yet been woken. The later `SOURCE_WAKE_FAILED` error is the only indication. This is appropriate for accepted data but requires applications to observe runtime error delivery or explicit barriers.

### 8.7 Real resize API behavior appears asymmetric

As described in §4.9, headless resize directly changes the sink viewport, but real resize appears to invalidate without updating `TermwizBackend.size` except through terminal resize events.

This may be intentional because the real terminal is authoritative, but the TypeScript API documentation says the runtime `size` changes after successful resize. The TS `size` metadata can therefore differ temporarily from the native backend viewport.

This is the most consequential terminal scheduling uncertainty found in static inspection.

### 8.8 Exit and disposal are not symmetric

`close()` calls native `dispose()` after TS teardown. Successful `exit()` does not.

Both paths:

- invalidate History liveness;
- tear down retained resources;
- unregister the wake broker host;
- restore terminal state.

But only close necessarily flips the native N-API wrapper’s `alive` atomic. Successful exit leaves the wrapper object alive while its Rust host is closed. This is likely intentional to avoid a second close after final restoration, but it should be documented or tested explicitly.

### 8.9 Terminal worker ownership is robust against most startup failures

The worker startup path restores terminal state when:

- setup fails after opening the terminal;
- the startup receiver is abandoned;
- crossterm EventReader startup fails;
- the command channel closes.

`TermwizBackend::Drop` also restores if `restored` is false.

The main remaining risk is not terminal restoration itself, but callers retaining an inert native wrapper after successful `exit()`.

### 8.10 Generic framework boundary is maintained

The traced host action type is only:

```rust
HostOutput::Routed(RoutedOutput)
```

No agent, assistant, transcript, tool, provider, or product-status concepts enter the generic runtime. Terminal input becomes generic routed output or native component state changes. This matches the framework ownership requirements in `AGENTS.md`.

---

## 9. Open questions and coverage gaps

1. **Real resize semantics**
   - Does `Tui.resize()` intend to resize the real terminal immediately, or only update/invalidate a logical viewport?
   - Should `TermwizBackend` update its cached viewport from the explicit resize API, or should real terminal events remain the sole authority?

2. **Successful exit native-wrapper state**
   - Should `NativeTuiHost::alive` become false after successful `exit()`?
   - Is retaining an alive-but-closed wrapper until GC intentional?
   - Should `Tui.exit()` call native `dispose()` after successful terminal restoration?

3. **Late cancellation during open**
   - `Tui.open()` only checks `options.signal.aborted` before native construction.
   - Is cancellation during synchronous native startup intentionally unsupported?

4. **`nextEvent()` error normalization**
   - Most synchronous `Tui` operations normalize native failures with `asTuiError`.
   - `nextEvent()` directly awaits native operations without a visible `try/catch` normalization layer.
   - Should terminal worker/input errors surface as native errors, `TuiError`, or termination events?

5. **Native-to-TS error-code completeness**
   - Unknown native codes become `INTERNAL_INVARIANT`.
   - Should stable codes such as `HOST_LOCK_POISONED`, `CONTENT_SCHEDULER_FAILED`, and History transfer errors be added to the TS contract, or are they intentionally internal?

6. **Automatic error listener lifecycle**
   - Only one listener is retained.
   - Is replacing the previous listener intended, or should multiple observers be supported?

7. **TS/native environment alignment**
   - The TS broker is per JS realm.
   - Native environments are per N-API `Env`.
   - What guarantees exist when multiple JS realms share native Sources or when worker/module boundaries differ?

8. **Presentation receipt progress**
   - Automatic broker polling uses a 1ms timer when `waiting_for_presentation`.
   - Native event wait uses `nextWakeMs()` and a 16ms cap.
   - Is the 1ms retry floor appropriate for all terminal backends, or should the backend expose a receipt readiness hint?

9. **Persistent presentation failure**
   - The source blocks retry-spinning and leaves work retryable.
   - What public mechanism should an application use to inspect and explicitly recover a permanently failing backend besides `flush()` and `onRuntimeError()`?

10. **Close during an outstanding native async wait**
    - The Rust host’s `HostInner` closure appears to cause `wait_for_output()` to return `None`.
    - No executed validation was performed for a concurrent `nextEvent()` plus `close()`/`exit()` race.

11. **Host lock poisoning**
    - Native environment draining continues unrelated hosts after one host lock is poisoned.
    - The poisoned host remains retry-blocked and requires explicit retry, but there is no public host recovery/reset operation.

12. **Final frame and content cleanup**
    - `exit()` intentionally keeps ContentPort resources alive through final frame preparation.
    - `close()` never prepares a final frame.
    - The exact behavior desired for close with pending Source cleanup is not publicly described beyond source comments.

13. **Physical synchronization recovery**
    - `physical_sync_unknown` is cleared only after a successful candidate commit.
    - The exact History/native scrollback effects of a partial terminal write require backend-level execution tests.

14. **LOC precision**
    - Figures in §1 are static estimates, not command-generated counts.

15. **Validation status**
    - No test or benchmark command was run.
    - All behavioral claims are source-derived or supported by named tests that were inspected, not executed.

---

## 10. Evidence appendix

### 10.1 Required documents

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

### 10.2 TypeScript production files inspected

#### Runtime lifecycle and scheduler

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `Tui.open`
  - `Tui` constructor
  - `prepareRootPublication`
  - `commitHistoryBinding`
  - `stageHistoryBinding`
  - `render`
  - `renderCanonical`
  - `renderDirect`
  - `flush`
  - `nextEvent`
  - `pollOutput`
  - `resize`
  - `setTheme`
  - `disposeRetainedExecution`
  - `close`
  - `exit`
  - `commitVisibleAfterDrain`
- `packages/iyon-tui/src/runtime/wake-broker.ts`
  - `EnvironmentWakeBroker`
  - `markPending`
  - `markEnvironmentPending`
  - `flush`
  - `drainAutomatically`
  - `drain`
  - `consumeReport`
  - `schedulePresentationPoll`
  - `RuntimeHostRegistrationImpl`
  - epoch/code/error conversion helpers
- `packages/iyon-tui/src/runtime/error-channel.ts`
  - `RuntimeErrorChannel`
  - `accept`
  - `markCommitted`
  - `throwPending`
  - `reportSafely`
- `packages/iyon-tui/src/runtime/environment.ts`
  - `EnvironmentRuntime`
  - `runtimeEnvironment`
- `packages/iyon-tui/src/runtime/attachments.ts`
  - `AttachmentBindingState`
  - `prepareSemanticAttachments`
  - desired/visible/superseded lease transitions
- `packages/iyon-tui/src/runtime/access.ts`
  - runtime access registration/lookup
- `packages/iyon-tui/src/runtime/events.ts`
  - `TuiEvent`, output/termination types
- `packages/iyon-tui/src/runtime/handle-registry.ts`
  - framework handle registration/disposal
- `packages/iyon-tui/src/runtime/native-resource-registry.ts`
  - runtime resource re-export

#### Retained execution and test harness

- `packages/iyon-tui/src/composition/execution.ts`
  - `RetainedExecutionScope`
  - `RetainedExecutionRuntime`
  - `invalidate`
  - `scheduleFlush`
  - `flush`
  - `mountExistingRoot`
  - `dispose`
  - publication staging/commit/abort
- `packages/iyon-tui/src/composition/tracked-state.ts`
  - `StateSource`
  - `state`
  - read subscription/write invalidation
- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness.open`
  - testing flush/advance/inspection
  - `pressKey`, `paste`, `advance`
  - `close`, `exit`

#### Native contract and structural seam

- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeTuiHostContract`
  - `NativeHostEpochs`
  - `NativeHostDrainReport`
  - native artifact loading/version validation
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - retained root boundary comments and H3 desired publication
  - `prepareDesiredInstall`
  - `publishDesiredPrepared`
  - `resolveNativeHost`
- `packages/iyon-tui/src/api/view/retained-state.ts`
  - ViewState native wake path
- `packages/iyon-tui/src/api/content/retained.ts`
  - `nativeWake`
  - `sourceWake`
  - Source mutation methods
  - ContentPort/ContentConnector lifecycle methods

### 10.3 Native N-API files inspected

- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/tui.rs`
  - `host_environment_for_env`
  - `NativeTuiHost::new`
  - `epochs`
  - `set_desired_view_ref`
  - `flush_pending_hosts`
  - `dispose`
  - `dispose_content_resources`
  - `exit`
  - `next_wake_ms`
  - `poll_terminal`
  - `next_output`
  - `wait_for_output`
  - `resize`
  - `advance_time`
  - native History/Content/TextInput wrappers
- `crates/iyon-tui-native/Cargo.toml`
  - `iyon-tui` dependency with `native-host` feature

### 10.4 Rust application/session files inspected

- `crates/iyon-tui/src/application/app.rs`
  - `App`
  - `App::new`
  - `App::start`
- `crates/iyon-tui/src/application/kernel.rs`
  - `RunningApp`
  - `RunningApp::new`
  - `advance_ready`
  - `next_deadline`
  - input dispatch
  - host invalidation helpers
  - `host_exit`
  - `prepare_frame_with_states`
- `crates/iyon-tui/src/application/host.rs`
  - `HostInner`
  - `TuiHost`
  - `TuiHost::open_in_environment`
  - `TuiHost::set_desired_view`
  - `TuiHost::flush_pending_hosts`
  - `TuiHost::next_wake_ms`
  - `TuiHost::poll_terminal`
  - `TuiHost::wait_for_output`
  - `TuiHost::close`
  - `TuiHost::exit`
  - `HostInner::flush_pending_frame`
  - `HostInner::render`
  - `HostInner::present_frame`
  - `HostInner::commit_frame`
  - `HostInner::finish_presentation_blocking`
  - ViewSlot animation state/tick path
  - native TextInput/ScrollPane invalidation
- `crates/iyon-tui/src/application/environment.rs`
  - `TuiEnvironment`
  - `mark_host_pending`
  - `complete_host`
  - `with_host_completion`
  - `drain_pending_for`
  - retry-blocked/waiting/pending queue management
- `crates/iyon-tui/src/application/run.rs`
  - `wait_for_present_blocking`
  - `wait_for_deadline`
- `crates/iyon-tui/src/application/timer.rs`
  - `TimerQueue`
  - scheduling/cancel/pop/next deadline

### 10.5 Component, scene, content and terminal files inspected

- `crates/iyon-tui/src/component/tick.rs`
  - `TickScheduler`
  - `sync_mounts`
  - `sync_capabilities`
  - `tick_due_with_events`
  - `next_deadline`
- `crates/iyon-tui/src/scene/host.rs`
  - `tick_due`
  - `invalidate_theme`
  - `invalidate_root`
  - `render_at_with_states`
  - `next_tick_deadline`
- `crates/iyon-tui/src/application/content.rs`
  - content advancement
  - Source wake failure channel
  - `ContentMutationResult`
  - grouped Source wake delivery
- `crates/iyon-tui/src/terminal/backend.rs`
  - `TerminalBackend`
  - `PresentReceipt`
  - `TerminalWorkerStopped`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
  - `TermwizBackend::enter`
  - `begin_frame`
  - `try_next_event`
  - `restore`
  - worker command bridge
- `crates/iyon-tui/src/terminal/termwiz/worker.rs`
  - worker startup/setup
  - command loop
  - terminal restore
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
  - `TermwizPresenter`
  - full/differential present
  - native scrollback insertion
  - synchronized output cleanup
  - final-frame positioning

### 10.6 Tests indexed/inspected

- `packages/iyon-tui/tests/tui_runtime.test.ts`
- `packages/iyon-tui/tests/tui_realtime.test.ts`
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `packages/iyon-tui/tests/tui_perf13_h.test.ts`
- `packages/iyon-tui/tests/tui_handles.test.ts`
- `packages/iyon-tui/tests/tui_harness.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `crates/iyon-tui/src/component/tick_tests.rs`
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs` test module
- Additional repository test files were indexed from `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt` but not read in full.

### 10.7 Files indexed but not comprehensively read

- Most unrelated Rust presentation/layout/content/history/interaction implementation files.
- Generated ABI bodies and generated schema files, except the native contract references.
- `crates/iyon-tui/src/terminal/crossterm/*`, except calls and event mapping references.
- Full native generated wrapper bodies under `crates/iyon-tui-native/src/generated/`.
- Unrelated TS API/composition/transport modules outside the scheduling seams.
- The complete repository-wide test fixture and benchmark corpus.

### 10.8 Read-only inspection method

The investigation used repository file enumeration and literal/symbol searches over the tracked source tree. No source files, configuration files, generated artifacts, services, dependencies, or running state were modified. No tests or benchmarks were executed.