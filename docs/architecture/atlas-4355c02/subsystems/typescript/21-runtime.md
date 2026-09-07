# 21 — TypeScript runtime

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `packages/iyon-tui/src/runtime/`
- Assignment: lifecycle, resources, attachments, wake broker, environment, errors and events.
- Parent-added atlas documentation is outside the source baseline.
- This report maps current behavior only. It does not make V5 migration, deletion, preservation, or disposition decisions.

I read:

1. `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
2. `docs/architecture/atlas-4355c02/README.md`
3. `PRE-V5-ARCHITECTURE-REPORT.md` for repository-wide context and evidence expectations
4. `AGENTS.md`, including the generic-framework ownership boundary
5. All nine production files in `packages/iyon-tui/src/runtime/`
6. The supporting native-resource implementation in `packages/iyon-tui/src/transport/native/`
7. The public framework-handle base class, testing harness, content FFI wake seam, and relevant tests

### Scope boundaries

The runtime directory is the TypeScript facade’s orchestration layer. It owns:

- `Tui` host lifecycle and public runtime operations
- root publication and desired/visible frame coordination
- host registration in the environment wake broker
- runtime error retention/reporting
- framework-handle identity and disposal delegation
- semantic attachment validation and frame leases
- test-only native access
- public runtime events

It does **not** own:

- retained execution/composition internals in `src/composition/`
- semantic `View` construction or normalization in `src/api/view/`
- native retained-DAG implementation in `src/transport/structural/`
- native host implementation in Rust/N-API
- content source/connector semantics in `src/api/content/` and native content transport
- terminal paint/layout implementation

Those modules are coupled to runtime through explicit contracts and callbacks.

### Evidence status

This is a static source inspection. I did not execute tests, builds, benchmarks, native hosts, or runtime code. Behavioral claims below are derived from source and existing tests. Existing tests are cited as architectural evidence, not as tests run during this investigation.

---

## 1. Responsibility and structure

### 1.1 Production inventory

The runtime directory contains nine TypeScript files:

| File | Physical LOC | Primary responsibility | Public surface |
|---|---:|---|---|
| `packages/iyon-tui/src/runtime/access.ts` | 27 | Weakly-held test access bridge for flushing, input injection, deterministic clock, and headless inspection | Internal; not package-root exported |
| `packages/iyon-tui/src/runtime/attachments.ts` | 313 | Semantic attachment traversal, validation, prepare leases, desired/visible binding state, revision-aware replacement | Partially public by direct internal import; not package-root exported |
| `packages/iyon-tui/src/runtime/environment.ts` | 46 | Per-realm runtime environment singleton containing resource registry and wake broker | Internal; not package-root exported |
| `packages/iyon-tui/src/runtime/error-channel.ts` | 128 | Host-scoped asynchronous error retention, deduplicated reporting, and explicit-barrier throwing | Internal; not package-root exported |
| `packages/iyon-tui/src/runtime/events.ts` | 14 | `TuiEvent`, `OutputEvent`, and `TerminateEvent` type contracts | Types re-exported from package root |
| `packages/iyon-tui/src/runtime/handle-registry.ts` | 80 | Framework handle identity allocation, registration, disposal and release delegation | Internal; used by public `FrameworkHandle` |
| `packages/iyon-tui/src/runtime/native-resource-registry.ts` | 18 | Runtime-level facade re-exporting the transport resource registry | Internal direct import point |
| `packages/iyon-tui/src/runtime/runtime.ts` | 928 | `Tui`, terminal host lifecycle, rendering, event waits, controls, history, root publication, cleanup | `Tui`, `TuiRuntime`, `TuiOpenOptions`, `TerminalMetadata` |
| `packages/iyon-tui/src/runtime/wake-broker.ts` | 551 | Environment-level edge-triggered wake scheduling, fair native drains, retry/barrier policy, presentation polling, counters/traces | Internal direct import point |

The runtime directory therefore contains approximately **2,105 physical production lines** by adding the physical file lengths above. The count includes comments, blank lines, interfaces, and documentation comments; it is not a token or executable-statement count.

Supporting resource implementation inspected outside the primary directory:

| File | Approx. physical LOC | Why runtime depends on it |
|---|---:|---|
| `packages/iyon-tui/src/transport/native/resource-registry.ts` | 480 | Actual environment-wide resource records, lifecycle states, generation, lease accounting, finalizers, ownership checks |
| `packages/iyon-tui/src/transport/native/resources.ts` | 88 | WeakMap raw-resource association and registry-backed lookups/disposal |
| `packages/iyon-tui/src/api/controls/framework-handle.ts` | 66 | Public nominal handle wrapper delegating registration and disposal into runtime |
| `packages/iyon-tui/src/transport/native/addon.ts` | relevant host contract around lines 125–165 | Native host methods consumed by `Tui` and wake broker |
| `packages/iyon-tui/src/testing/index.ts` | 154 | `AppHarness` implementation over `Tui` and `runtime/access.ts` |
| `packages/iyon-tui/src/transport/content/ffi.ts` | relevant wake/ABI sections | Content mutation result invokes environment wake scheduling |
| `packages/iyon-tui/src/api/content/retained.ts` | relevant source/port/connector sections | Content controls and source wake requests |

### 1.2 Primary versus secondary responsibilities

#### `runtime.ts`

Primary responsibilities:

- Construct and own one native `NativeTuiHostContract`.
- Register the host with the environment wake broker.
- Create one retained execution runtime per `Tui`.
- Route canonical producer rendering and direct scene rendering.
- Stage and commit `History` sideband state.
- Prepare root structural publication and semantic attachment leases.
- Expose factories for `History`, `ViewState`, `ContentPort`, `TextInput`, `ViewSlot`, and `ScrollPane`.
- Expose terminal input/output routing.
- Implement resize, theme changes, event waits, close, and exit.
- Dispose runtime-owned resources in a dependency-aware order.

Secondary responsibilities:

- Register test-only access methods.
- Enforce mutation exclusion during retained protocol passes.
- Own per-`Tui` history lifetime state.
- Own runtime error listener installation.
- Maintain the current scene and retained root boundary.

#### `attachments.ts`

Primary responsibilities:

- Traverse semantic nodes.
- Detect semantic cycles.
- Detect duplicate attachment handles.
- Resolve and lease resources through `NativeResourceRegistry`.
- Retain leases through prepared, desired, superseded, and visible frame states.
- Promote visible bindings only after native frame visibility is committed.

Secondary responsibilities:

- Normalize decimal revision numbers to canonical `bigint` strings.
- Handle row/column/grid sequence overrides during traversal.
- Remove decorated wrappers when determining attachment target node kind.

#### `wake-broker.ts`

Primary responsibilities:

- Maintain one host registration set per environment.
- Coalesce host pending marks into a microtask.
- Select a fair driver host for native environment drains.
- Keep automatic drains non-throwing.
- Preserve retryable errors for explicit barriers.
- Poll asynchronous terminal presentation receipts without microtask spinning.
- Maintain diagnostics counters and optional trace events.

Secondary responsibilities:

- Convert native string/number epochs into validated `bigint`.
- Normalize native error phases and codes.
- Remove abandoned hosts through `FinalizationRegistry`.
- Handle explicit barrier retry bounds.

#### `error-channel.ts`

Primary responsibilities:

- Store the latest frame error per host.
- Deduplicate automatic reports.
- Call a configured reporter or the platform diagnostic sink.
- Convert the retained record into a deterministic `TuiError` at an explicit barrier.
- Clear errors after successful host commits.

#### `environment.ts`

Primary responsibility:

- Provide the realm singleton containing:
  - shared `NativeResourceRegistry`
  - shared resource environment token
  - one `EnvironmentWakeBroker`

#### `handle-registry.ts`

Primary responsibilities:

- Allocate process/realm-local framework `HandleId` values.
- Connect public framework handles to native resource registrations.
- Delegate disposal through the resource registry and raw-resource map.
- Restore a resource from `disposing` to `live` if native disposal fails before release.

#### `access.ts` and `events.ts`

- `access.ts` is a test-only bridge keyed by a `WeakMap<object, RuntimeAccess>`.
- `events.ts` defines the only runtime event union:
  - `OutputEvent`
  - `TerminateEvent`

---

## 2. Types, APIs and contracts

### 2.1 Public runtime API

`TuiRuntime` in `runtime.ts` lines 59–91 exposes:

```ts
readonly size: TerminalMetadata;
nextEvent(signal?: AbortSignal): Promise<TuiEvent>;
render(scene: SceneProducer, signal?: AbortSignal): void;
flush(): void;
onRuntimeError(listener: RuntimeErrorReporter): () => void;
resize(width: number, height: number): void;
close(): void;
exit(): void;
createHistory(): HistoryContract;
viewState(): ViewStateContract;
contentPort(options?: ContentPortOptions | typeof TextContent): ContentPortContract;
createTextInput(options?: TextInputOptions): TextInputContract;
createViewSlot(initial: View): ViewSlotContract;
createScrollPane(initial: View): ScrollPaneContract;
bindKey(key: string, routeId: string, modifiers?: readonly string[]): void;
route(output: Output<string>, routeId: string): void;
interceptPaste(input: TextInputContract, routeId: string): void;
forwardPaste(text: string): void;
setTheme(theme: Theme): void;
```

The package root exports:

- `Tui`
- `TuiRuntime`
- `TuiOpenOptions`
- `TerminalMetadata`
- `TuiEvent`, `OutputEvent`, and `TerminateEvent` types
- `TuiError` helpers

The package root does **not** export `EnvironmentWakeBroker`, `RuntimeErrorChannel`, `AttachmentBindingState`, `NativeResourceRegistry`, or runtime counters. Those are internal seams or direct-test imports.

### 2.2 `TuiOpenOptions` and terminal metadata

`TuiOpenOptions` supports:

- optional width and height
- `headless`
- `AbortSignal`
- initial `Theme`

`Tui.open()`:

1. Rejects an already-aborted signal with a cancelled `TuiError`.
2. Defaults to 80×24.
3. Validates each dimension as an integer in `[1, 65535]`.
4. Loads the native `NativeTuiHost` constructor.
5. Constructs the host.
6. Constructs `Tui`.
7. Applies the initial theme if present.
8. On failure, closes a constructed `Tui` or disposes a constructed host.
9. Aggregates primary and cleanup failures if cleanup also fails.

`size` is an immutable object snapshot of the last successfully completed resize. It is not read from native on every access.

### 2.3 Events

`events.ts` defines:

```ts
interface OutputEvent {
  readonly type: "output";
  readonly routeId: string;
  readonly payload?: string;
}

interface TerminateEvent {
  readonly type: "terminate";
  readonly reason?: string;
}

type TuiEvent = OutputEvent | TerminateEvent;
```

The runtime intentionally exposes generic route IDs and payloads. There is no product-specific action interpretation.

### 2.4 Runtime error contracts

`RuntimeFrameErrorRecord` contains:

- `hostId: string`
- `attemptedEpoch: bigint`
- `desiredRevision: bigint`
- `phase: "structural" | "content" | "frame" | "backend"`
- `code`
- `retryable`
- `diagnostic`

Supported runtime codes are:

- `FRAME_PREPARATION_FAILED`
- `BACKEND_NOT_READY`
- `BACKEND_IO_FAILED`
- `SURFACE_DESYNCHRONIZED`
- `LAYOUT_DID_NOT_CONVERGE`
- `INTERNAL_INVARIANT`
- `RUNTIME_POISONED`
- `SOURCE_WAKE_FAILED`

`RuntimeErrorReporter` returns `boolean | void`. A return value of `true` suppresses the default platform reporter. It does not remove the stored error or prevent a later explicit barrier from throwing it.

`RuntimeErrorChannel` behavior:

- `accept(record)` stores only the latest record for a host.
- A record key is host ID + epoch + desired revision + phase + code.
- Repeated records with the same key are reported only once.
- A different record for the same host permits the prior key to be reported again if it reappears.
- `markCommitted(hostId)` clears the latest error and its reported key.
- `throwPending(hostId)` removes the record and throws a `TuiError("runtime", ...)` with the runtime code in `nativeCode` and bigint values stringified in context.
- Automatic reporter failures are caught and routed to `globalThis.reportError` or `console.error` without changing frame/retry state.

### 2.5 Framework handles and resource ownership

`FrameworkHandle` is the public nominal base class. It owns:

- `id: HandleId`
- `kind`
- a private disposed bit

The constructor delegates to `registerFrameworkHandle()`. `dispose()` delegates to `disposeFrameworkResource()` and only marks the wrapper disposed after disposal succeeds.

`HandleId` is a branded positive number, explicitly documented as JavaScript-local identity rather than native identity.

`FrameworkHandle` itself is a public authoring surface, but registration, disposal, lease checks, and raw native lookup are runtime/transport machinery.

### 2.6 Resource records and leases

The transport `NativeResourceRegistry` is the actual resource owner. Runtime re-exports it through `runtime/native-resource-registry.ts`.

A resource record stores:

- numeric handle identity
- resource kind
- weak references to handle and native resource
- optional owner `{ environment, host? }`
- optional accepted semantic node kinds
- environment identity
- monotonically increasing generation
- lifecycle: `"live" | "disposing" | "disposed"`
- prepared, desired, and visible lease counts

`PreparedResourceLease` has four relevant phases:

```text
prepared
   ├── commitDesired() → desired
   │                         └── commitVisible() → visible=true
   ├── abort() → released
   └── releaseDesired()
visible=true
   └── releaseVisible()
```

A visible lease may coexist with an already prepared/desired replacement. This is necessary for a failed or in-flight frame not to destroy the currently visible resource.

### 2.7 Ownership validation

`prepareResolve()` checks:

1. Record exists and is not disposed.
2. Record is not already disposing.
3. No other prepared lease exists for that handle.
4. Handle and native resource weak references remain live.
5. Resource kind matches the expected kind. `content-port` accepts registered kind `content` as a compatibility case.
6. Resource environment is exactly the target environment.
7. If resource has a host owner, the target host token matches exactly.
8. Target semantic node kind is in `acceptedNodeKinds`, when constrained.
9. Native resource’s optional `validateNodeKind()` accepts the target kind.

Failure categories distinguish disposed handles, wrong environment/host, kind mismatch, duplicate preparation, and unsupported node attachment.

### 2.8 Attachment contracts

`AttachmentRuntimeContext` supplies:

- shared `NativeResourceRegistry`
- environment token
- host token

`PreparedAttachmentSet` supplies:

- prepared leases
- `commitDesired()`
- `abort()`

`AttachmentBindingState` maintains:

- `desired`
- `visible`
- `superseded` desired revisions
- desired revision
- visible revision

Counts are exposed only as diagnostics through `desiredCount()` and `visibleCount()`.

### 2.9 Wake broker contracts

`NativeFrameHost` requires:

```ts
epochs(): NativeHostEpochs;
flushPendingHosts(budget?: number, forceRetry?: boolean): NativeHostDrainReport;
```

The native drain report contains:

- `rearm`
- `waiting_for_presentation`
- attempted count
- commit records
- error records
- wake epoch

`RuntimeHostRegistration` exposes:

- stable string ID
- token object
- native reference
- `markPending()`
- `flush()`
- `dispose()`

The registration stores a weak reference to native host. The `Tui` itself holds the strong host reference.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
package-root index.ts
        │
        ▼
runtime/runtime.ts ────────────────┐
        │                          │
        ├── runtime/environment.ts │
        │       ├── native resource registry
        │       └── wake broker
        │
        ├── runtime/error-channel.ts
        ├── runtime/attachments.ts
        ├── runtime/access.ts
        ├── runtime/events.ts
        │
        ├── composition/execution.ts
        ├── api/view/{scene,view,retained-state}.ts
        ├── api/controls/{history,text-input,view-slot,scroll-pane}.ts
        ├── api/content/retained.ts
        ├── transport/structural/retained-dag.ts
        ├── transport/structural/native-view-abi.ts
        ├── transport/structural/style-lowering.ts
        └── transport/native/addon.ts
```

```text
public FrameworkHandle
        │
        ▼
runtime/handle-registry.ts
        │
        ├── transport/native/resources.ts
        │       ├── weak raw-resource map
        │       └── runtime resource registry
        │
        └── transport/native/resource-registry.ts
                ├── environment identity
                ├── lifecycle records
                ├── lease counts
                └── finalizers
```

```text
Content Source mutation
        │
        ▼
transport/content/ffi.ts
        │  if native mutation flags schedule drain
        ▼
api/content/retained.ts::sourceWake()
        │
        ▼
runtime/environment.ts::wakeBroker.markEnvironmentPending()
        │
        ▼
EnvironmentWakeBroker → NativeFrameHost.flushPendingHosts()
```

### 3.2 Reverse ownership map

| Object | Created by | Destroyed by | Lifetime owner |
|---|---|---|---|
| Native host | `Tui.open()` | `Tui.close()` / `Tui.exit()` cleanup | `Tui` |
| `Tui` host registration | `Tui` constructor | `disposeRetainedExecution()` | `Tui`/environment broker |
| Retained execution runtime | `Tui` constructor | `disposeRetainedExecution()` | `Tui` |
| Root builder | first canonical render | replacement by direct render or close/exit | `Tui` |
| Root boundary | first direct or canonical publication | close/exit | `Tui` |
| Current scene | root publication commit | next successful desired publication or close | `Tui` |
| Attachment desired leases | root publication commit | superseding desired publication, visible replacement, close | `AttachmentBindingState` |
| Attachment visible leases | environment commit callback | next visible replacement or close | `AttachmentBindingState` |
| ViewState/ContentPort/TextInput/Slot/Pane | `Tui` factory | `disposeOwnedHandles()` or caller disposal | owning `Tui` for factory-created values |
| Caller-created detached History | caller/native factory | caller | caller, but attached History is bound once |
| Environment-owned Source | content API | caller/resource lifecycle | shared environment |
| Resource registry records | framework registration | explicit release, owner invalidation, or finalization | shared realm environment |
| Runtime error records | host error channel | commit, explicit throw, clear, or replacement | host registration |
| Wake broker host entries | environment registration | explicit registration disposal or finalizer | environment |
| Test access record | `registerRuntimeAccess()` | garbage collection of `Tui` key | test harness weak map |

### 3.3 Environment identity

There are two related but distinct singleton mechanisms:

- `runtime/environment.ts` uses `Symbol.for("iyon:tui:runtime-environment")`.
- `transport/native/resource-registry.ts` uses `Symbol.for("iyon:tui:resource-environment")` and `Symbol.for("iyon:tui:native-resource-registry")`.

`EnvironmentRuntime.token` is obtained from `runtimeResourceEnvironment()`, and `EnvironmentRuntime.resources` is obtained from `runtimeResourceRegistry()`. Thus the environment token and registry are intentionally shared across runtime and transport/native seams within the JavaScript realm/module environment.

This is a strong identity boundary. A handle registered under a different environment token is rejected even if its shape and kind otherwise match.

### 3.4 Callback ownership

The `Tui` constructor registers:

```ts
(commit) => owner.deref()?.commitVisibleAfterDrain(commit)
```

where `owner` is a `WeakRef<Tui>`. The environment broker therefore does not strongly retain `Tui` through its commit callback. The `Tui` strongly retains its host and registration, so the normal live runtime remains stable while abandoned registrations can be finalized.

---

## 4. Execution paths and state transitions

### 4.1 Runtime startup

```text
Tui.open(options)
  ├── validate cancellation and dimensions
  ├── require native NativeTuiHost
  ├── new NativeTuiHost(width, height, headless)
  ├── new Tui(host, width, height)
  │     ├── register host in environment wake broker
  │     ├── create attachment context
  │     ├── create one RetainedExecutionRuntime
  │     └── register testing access
  ├── optional setTheme()
  └── return Tui
```

The constructor eagerly creates the retained execution runtime. It does not create the root builder or structural root until the first canonical render.

If startup fails:

- a constructed `Tui` is closed;
- otherwise a constructed host is disposed;
- cleanup errors are combined with the primary failure in `AggregateError`.

### 4.2 Canonical producer render

A function passed to `render()` enters `renderCanonical()`.

#### First canonical render

1. Validate the signal.
2. Ensure the runtime is open.
3. Drain pending retained execution.
4. Reject mutation during a retained protocol pass.
5. Build a producer closure:
   - call the caller’s `SceneProducer`;
   - normalize through `Scene.from()`;
   - stage its History sideband;
   - return `scene.body`.
6. Build a root publication target.
7. Start `OwnedBuilderRoot`.
8. Producer evaluation and initial materialization prepare root publication:
   - semantic attachments are resolved and leased;
   - root boundary prepares desired structural installation;
   - History binding is staged.
9. Commit desired publication:
   - commit History binding;
   - commit structural desired state;
   - read desired structural revision;
   - commit desired attachment leases;
   - mark host pending;
   - install `currentScene`.
10. Call `flush()` so the desired frame is driven through the explicit barrier.

A failed initial evaluation restores the prior staged History and leaves `rootScopeCreated` false. This keeps first render retryable.

#### Subsequent canonical render

1. Drain retained execution.
2. Replace the existing producer in the same root execution scope.
3. The retained runtime re-drives the root.
4. Root publication follows the same prepare/commit route.
5. A producer/evaluation failure restores staged History.
6. A later host/frame failure does not roll back already accepted desired state; the desired revision remains retryable.
7. `flush()` drives the host barrier.

State reads inside the producer subscribe the root execution scope. Later tracked state writes can re-render without another explicit `render()` call.

### 4.3 Direct scene render

A non-function input enters `renderDirect()`.

1. Validate signal and open state.
2. Drain retained execution.
3. Reject retained-pass mutation.
4. Normalize through `Scene.from()`.
5. Validate the semantic body before changing History sideband.
6. Validate History owner and native resource identity.
7. Compute effective History as explicit History or previously bound History.

#### Direct identity no-op

If current body and effective History are identical:

- semantic attachments are still validated;
- staged History is updated;
- the root builder is disposed;
- `flush()` is called;
- no new structural publication is prepared.

This means identity equality is a transport cutoff, not an exemption from attachment validation or runtime barriers.

#### Direct replacement

For a changed body or History:

1. Save prior staged History.
2. Stage effective History.
3. Prepare root publication.
4. Commit publication.
5. Restore prior staged History if prepare/commit throws.
6. Keep effective History staged after success.
7. Dispose any canonical root builder.
8. Call `flush()`.

### 4.4 Root publication and desired/visible separation

`prepareRootPublication()` prepares:

- semantic attachment leases;
- next `Scene` record;
- root boundary desired installation.

The returned `RootPublication.commit()`:

1. Swaps History binding before structural publication.
2. Commits the retained structural root.
3. Reads desired structural revision.
4. Commits desired attachment leases against that revision.
5. Marks the host pending.
6. Installs `currentScene`.

The returned `abort()` aborts both structural publication and semantic attachment preparation.

The native host may therefore have:

```text
desired structural revision = N
visible structural revision = N-1
pending epoch != committed epoch
```

The previous visible frame remains authoritative until the wake broker reports a native commit.

### 4.5 Visible commit

The wake broker invokes `commitVisibleAfterDrain(commit)` after a native host commit:

1. Extract visible structural revision, if provided.
2. Promote retained boundary visibility.
3. Promote attachment visibility for the same revision.
4. Call `syncNativeLifecycle()` or `syncNativeLifecycles()` on every Tui-owned handle if present.

Attachment visibility is deliberately delayed until native host visibility is confirmed.

### 4.6 History binding state

History has attach-once semantics:

- `historyOwners` is a `WeakMap<HistoryContract, Tui>`.
- A History already owned by another `Tui` is rejected.
- A different History cannot replace an already bound History on the same `Tui`.
- A detached History may be attached during root publication.
- Host-fabricated History is considered already attached and is not transferred through `setHistory`.
- `bindHistoryLifetime()` associates caller-owned attached History with the Tui’s shared liveness token.
- Closing or exiting sets that token closed.

`stageHistoryBinding()` validates the effective History on every producer pass, even when identity is unchanged, preventing a disposed native History from remaining silently referenced by a retained scene.

The owner map is never explicitly cleared by `close()`. Combined with attach-once semantics, this prevents a formerly attached History object from being silently transferred to a different Tui.

### 4.7 Mutation guard

Most public mutations use:

```text
ensureOpen()
  → retainedRuntime.flush()
  → assertNotMutating(operation)
```

`assertNotMutating()` rejects calls when:

- `protocolState.mutating` is true and not an internal publication; or
- an active retained execution scope exists.

The guard is used for runtime factories, routes, key bindings, paste forwarding, resize, theme changes, and close/exit. ViewState has a parallel callback guard.

### 4.8 Runtime controls and resource creation

#### `createHistory()`

- Drains/guards mutation.
- Obtains host-fabricated native History.
- Wraps it in a `History` handle.
- Claims the Tui owner.
- Binds the Tui lifetime.
- Adds it to `ownedHandles`.

#### `viewState()`

- Drains/guards mutation.
- Obtains native ViewState.
- Wraps it with environment and host tokens.
- Registers a pending callback.
- Registers a mutation guard.
- On wrapper construction failure, disposes the raw native resource and aggregates cleanup failure if needed.

#### `contentPort()`

- Validates options and rejects unknown keys.
- Supports only the `"text"` family.
- Creates a native host-bound ContentPort.
- Supplies environment/host owner information.
- Marks host pending after relevant mutations.
- Guards content mutation during retained passes.

#### `createTextInput()`

- Converts border input with `borderNodeFor()`.
- Creates host-bound native text input.
- Wraps and owns it.

#### `createViewSlot()` and `createScrollPane()`

- Use the shared retained execution runtime.
- Receive the same attachment context as the root.
- Are owned by the creating Tui.

#### Routing

`route()` only accepts an `Output` whose associated TextInput is:

- owned by this Tui;
- still in `ownedHandles`;
- not disposed.

`interceptPaste()` similarly requires an input owned by this Tui.

### 4.9 Event wait paths

`nextEvent(signal?)`:

- rejects an already-aborted signal;
- returns `{ type: "terminate", reason: "closed" }` if already closed;
- without a signal, awaits native `waitForOutput()`;
- with a signal, calls `pollOutput()`;
- checks cancellation again after native output arrives;
- maps native route/payload to `OutputEvent`;
- maps `null` output to `TerminateEvent`.

The abortable polling path:

```text
while (!closed):
  reject if signal aborted
  host.pollTerminal()
  output = host.nextOutput()
  return output if present
  await delay(min(max(host.nextWakeMs(), 1), 16), signal)
return null
```

This path is intentionally event-loop based and bounded to at most a 16 ms delay per iteration. A cancellation listener rejects with a cancelled TuiError and clears the timer.

### 4.10 Resize and theme

`resize()`:

1. Ensures open.
2. Validates dimensions.
3. Drains/guards mutation.
4. Calls native resize.
5. Updates `width` and `height` only after native resize succeeds.

`setTheme()`:

1. Drains/guards mutation.
2. Converts public theme to native materialized form.
3. Calls native `setTheme`.
4. Always resets retained style-reference caches in `finally`, including when native presentation fails.

The reset is necessary because the native host may have changed theme before a later presentation operation fails.

### 4.11 Close lifecycle

`close()` is idempotent:

1. Return if already closed.
2. Reject if called during retained mutation.
3. Mark `closed = true`.
4. Mark the History lifetime closed.
5. Dispose retained execution:
   1. unregister host registration from wake broker;
   2. invoke native content-resource cascade;
   3. remove runtime error reporter;
   4. dispose attachment bindings;
   5. clear native ViewState bindings;
   6. invalidate host-owned registry resources;
   7. dispose Tui-owned handles with retry passes;
   8. dispose root builder;
   9. dispose retained execution runtime;
   10. close root boundary;
   11. clear staged/bound History references.
6. Clear current scene.
7. Dispose native host.
8. Throw one error directly or aggregate multiple errors.

The ordering intentionally prevents queued environment work from outliving the Tui and targeting resources being disposed.

### 4.12 Exit lifecycle

`exit()` has different native ordering:

1. Return if already closed.
2. Reject retained-pass mutation.
3. Mark History lifetime closed.
4. Call native `host.exit()` while desired content bindings still exist, allowing the native final exit frame.
5. Dispose retained runtime/resources after that final native frame.
6. If native exit failed, attempt native host disposal.
7. Mark closed and clear scene even when restoration failed.
8. Throw one or aggregate all collected failures.

Thus `exit()` is terminal even in the presence of restoration errors; `close()` does not prepare a final frame.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation to production-path matrix

| Semantic operation | Production path | Selection condition | Success state | Failure behavior |
|---|---|---|---|---|
| Open runtime | `Tui.open()` | Always | Native host and environment registration live | Cancel/validation/native failure; cleanup attempted and possibly aggregated |
| Render producer first time | `renderCanonical()` → `OwnedBuilderRoot.start()` | `typeof sceneOrBuilder === "function"` and no root scope | Root desired publication and retained scope installed | Evaluation/prepare failure restores staged History and keeps first render retryable |
| Render producer again | `renderCanonical()` → `replaceProducer()` | Existing root scope | Same retained root identity, new producer | Producer failure restores staged History; accepted desired frame may remain pending after later frame failure |
| Render direct changed scene | `renderDirect()` → `prepareRootPublication()` | Non-function scene and identity differs | Desired root and History committed | Previous visible frame remains native authority if prepare fails |
| Render direct same scene | Identity cutoff + attachment validation | Current body and History identical | No structural replacement; barrier still runs | Attachment validation can fail despite identity match |
| Attach state/content | `prepareSemanticAttachments()` | Semantic root reports attachments | Prepared leases then desired/visible promotion | Wrong host/environment, duplicate, disposed, unsupported, cycle, or native validator failure; all prepared leases aborted |
| Automatic frame drain | `markPending()` → queued microtask → `drain(false)` | Host mutation or native content wake | Native report consumed, commits promoted | Error retained/reported; no automatic throw; no spin when `rearm=false` |
| Explicit frame barrier | `registration.flush()` | `Tui.flush()` or testing access | Captured pending epoch reaches committed epoch | Up to 64 forced retries, then retained/throws `TuiError` |
| Source wake | `sourceWake()` → `markEnvironmentPending()` | Native content mutation flags request environment drain | One registered host marks pending | No registered host means no drain request; native source state remains governed by native contract |
| Output wait without signal | `host.waitForOutput()` | `signal === undefined` | Output or termination event | Native promise behavior |
| Output wait with signal | `pollOutput()` | AbortSignal supplied | Output or termination event | Cancellation converted to cancelled TuiError |
| Close | `disposeRetainedExecution()` → `host.dispose()` | `close()` | All reachable runtime state retired | Cleanup errors aggregated; close remains terminal |
| Exit | `host.exit()` → cleanup | `exit()` | Final terminal exit frame then resource teardown | Restoration and cleanup errors aggregated; runtime marked terminal |
| Handle disposal | `beginDisposal()` → raw native `dispose()` → registry `release()` | Public `FrameworkHandle.dispose()` | Handle/resource retired | Active leases reject disposal; native failure returns record to live |
| Resource owner teardown | `invalidateHost()` | Tui retained-runtime disposal | Host-owned records enter disposing and finalize when leases reach zero | Leases retain resource until replacement/abort/visible release |

### 5.2 Cache miss, recovery, compatibility, and masking

#### Recovery routes

- Automatic wake failures are retained rather than thrown.
- Explicit `flush()` retries with `forceRetry=true`.
- A frame can be accepted as desired while visible state remains old, enabling later retry.
- Native presentation receipts are polled by timer rather than spinning microtasks.
- Handle disposal can be retried after dependent controls release attachment leases.
- Native disposal failure invokes `cancelDisposal()` to restore resource liveness.

#### Compatibility route

`NativeResourceRegistry.prepareResolve()` accepts a registered kind of `"content"` when the expected attachment kind is `"content-port"`. This is an explicit compatibility condition, not a generic fallback.

#### Failure masking

- Automatic broker errors never throw into the microtask. They are routed to `RuntimeErrorChannel`.
- A configured runtime error listener suppresses only the default reporter when it returns `true`; it does not consume the stored failure for explicit barriers.
- A native unknown error code is mapped to `INTERNAL_INVARIANT`, preventing silent downgrade to an ordinary retry.
- `asTuiError()` parses `ION_*` prefixes from native `Error.message`; unknown or non-Error failures become runtime-category errors.
- `close()` and `exit()` deliberately continue cleanup after individual failures and aggregate outcomes.
- `Tui.open()` similarly attempts cleanup after constructor/theme failure.
- `disposeOwnedHandles()` retries handles that failed because dependent resources were still in use. If every handle fails in a pass, it stops and reports that pass’s errors.

#### Observation requiring caution

The `disposeOwnedHandles()` comment says it preserves every final error, but when an entire retry pass fails, `finalErrors` is assigned to the current `passErrors`; errors from earlier partially failed passes are not retained in that final aggregate. This may be intentional because later passes represent the final blocking state, but the implementation does not preserve every error from every pass.

### 5.3 Absence claims

The following absence claims are limited to the inspected TypeScript package source and tracked runtime files:

- No second public runtime event union was found under `packages/iyon-tui/src/runtime/`; `events.ts` has only output and terminate events.
- No package-root export for wake broker, error channel, attachment state, or resource registry was found in `src/index.ts`.
- No product-specific event/action semantics appear in runtime files.
- No direct runtime polling loop is used for automatic broker retry; presentation waiting uses `setTimeout`.
- No direct JavaScript mirror of native per-source subscriptions exists in the inspected runtime; source wakes mark a host through the environment broker.

These are not claims about uninspected Rust/native code or external packages.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Runtime-owned retained objects

The runtime keeps:

- one `RetainedExecutionRuntime` per `Tui`;
- optional root builder;
- optional root boundary;
- current scene;
- staged and bound History references;
- attachment desired/visible/superseded lease sets;
- owned handle set;
- host registration;
- one host-scoped error channel;
- environment-wide wake broker and resource registry.

These are retention/lifetime structures rather than memoization caches.

### 6.2 Weak maps and finalizers

Weak structures include:

- `accesses` in `access.ts`, keyed by Tui owner;
- `historyOwners`, keyed by History object;
- `handleIds`, keyed by public framework handle;
- raw `nativeResources`, keyed by framework handle;
- registry handle/resource lookup maps;
- resource and lease `FinalizationRegistry` callbacks;
- wake-broker registration finalizer for abandoned native hosts;
- weak native host reference per registration;
- weak runtime owner captured by Tui commit callback.

The raw resource itself remains strongly held by `PreparedResourceLease` while prepared, desired, or visible. This prevents garbage collection from invalidating an in-flight native attachment.

### 6.3 Attachment retention bounds

Attachment superseded revisions are retained so native frames in flight can still refer to their resources. On visible promotion:

- target revision is selected;
- prior visible leases are released;
- superseded revisions at or below the committed revision are released.

With an undefined revision, all superseded state is immediately released and desired state is made visible. The source does not define a numeric bound on the number of superseded revisions; practical retention depends on how many desired revisions are published before native visibility commits.

### 6.4 Wake scheduling

The wake broker uses:

- one pending set of host IDs;
- one edge-triggered microtask latch;
- a pending-generation counter to detect new marks during native calls;
- a fair cursor for selecting drivers;
- a default native drain budget of 32;
- a maximum of 64 attempts for explicit barriers;
- one presentation poll timer;
- no microtask retry when native reports `waiting_for_presentation`.

Important scheduling semantics:

1. Multiple marks before the microtask are coalesced.
2. Marks during a drain are recorded through `pendingGeneration`.
3. `rearm=false` is authoritative for automatic failure handling; unchanged pending epochs do not cause a busy loop.
4. `waiting_for_presentation=true` yields to the event loop for one millisecond before trying again.
5. Explicit barriers cancel a queued automatic microtask and force native retries.
6. Native can drain multiple hosts in one call; JavaScript does not mirror every native subscription.

### 6.5 Counters and traces

`WakeBrokerCounters` tracks:

- pending marks
- wake latch wins
- already-latched marks
- queued microtasks
- drains
- attempted hosts
- committed frames
- automatic errors
- rearm count
- explicit barriers
- explicit barrier failures

Counters are module-global and resettable. `WakeTraceEvent` records pending, drain, commit, error, and rearm events. Trace storage is capped at 256 records and is enabled only when `Bun.env.PERF_RUNTIME_TRACE === "1"`.

These are directly importable internal diagnostics, not package-root API.

### 6.6 Per-operation and per-frame work

| Work | Frequency |
|---|---|
| Semantic attachment traversal | Per root/slot/pane prepare that has attachments |
| Duplicate attachment map lookup | Per attachment occurrence |
| Resource lease promotion | Per desired or visible binding transition |
| Retained root publication | Per accepted structural replacement |
| Wake mark | Per host mutation/requested source wake |
| Automatic native drain | Per coalesced microtask |
| Native visible callback | Per native frame commit |
| Owned handle lifecycle sync | Per visible commit, for every owned handle with sync method |
| Runtime error report | Once per distinct host/epoch/revision/phase/code key |
| Style reference cache reset | Every `setTheme()` call, including failure |
| Event polling | Per abortable `nextEvent()` call while no output is available |
| Resource statistics iteration | Per explicit `stats()` call |

The runtime itself has no width-keyed layout cache; width-dependent layout/cache work belongs to retained structural/native transport and layout modules.

### 6.7 Invalidation triggers

- Structural desired publication marks a host pending.
- Content mutation can call `markEnvironmentPending()`.
- ViewState and ContentPort mutations receive callbacks that mark the owning host pending.
- Native visible commit promotes desired/visible bindings.
- Theme changes clear style-reference caches.
- Host close invalidates host-owned registry resources.
- Handle disposal is blocked while any lease remains.
- Successful commit clears the latest runtime error for that host.

---

## 7. Tests, benchmarks and observability

### 7.1 Relevant existing tests

The principal runtime-seam test file is:

- `packages/iyon-tui/tests/tui_perf13_a.test.ts` — 358 physical lines

It directly imports:

- `NativeResourceRegistry`
- `runtimeResourceEnvironment`
- `runtimeResourceRegistry`
- `AttachmentBindingState`
- `prepareSemanticAttachments`
- `RuntimeErrorChannel`
- `EnvironmentWakeBroker`
- native structural boundary helpers

The test coverage documents these contracts:

1. **Attachment preparation and leasing**
   - prepared → desired → visible counts
   - wrong-host rejection
   - unsupported node-kind rejection
   - release after binding disposal
   - retired ID rejection

2. **Duplicate attachment rejection**
   - shared semantic ordinary subtrees are allowed;
   - duplicate attachment use is rejected;
   - failed prepare restores prepared lease count to zero.

3. **H3 prepare atomicity**
   - invalid duplicate attachment in a replacement render throws;
   - previously visible screen rows remain unchanged.

4. **Wake coalescing and explicit retry**
   - two `markPending()` calls produce one automatic native call;
   - automatic retryable failure is retained and does not spin;
   - explicit `flush()` forces the second call and commits.

5. **Content/source wake failure**
   - `SOURCE_WAKE_FAILED` retains `phase: "content"`;
   - a subsequent new pending mark causes another attempt.

6. **Asynchronous presentation receipt**
   - `waiting_for_presentation` does not busy-loop microtasks;
   - timer polling later retries and commits.

7. **Desired versus visible publication**
   - desired structural revision advances before visible frame revision;
   - old screen remains visible while second desired state is pending;
   - visible screen changes only after native drain and `commitVisible()`.

The runtime-specific event/control test is:

- `packages/iyon-tui/tests/tui_runtime.test.ts` — 31 physical lines

It covers:

- native local TextInput editing;
- routed output event delivery;
- cancellation of pending `nextEvent()`;
- idempotent `close()`.

The testing harness is:

- `packages/iyon-tui/src/testing/index.ts`

`AppHarness`:

- always opens `Tui` headless;
- routes deterministic input through `runtimeAccess`;
- flushes before input and inspection;
- uses `advance(0)` to drain deterministic native work;
- preserves read-only inspection after exit;
- converts thrown failures with `asTuiError`.

Other test files call runtime through `AppHarness`, but they were indexed rather than exhaustively read for this assignment.

### 7.2 Observability surfaces

Available internal observability includes:

- `NativeResourceRegistry.stats()`
  - live resources
  - disposing resources
  - prepared leases
  - desired leases
  - visible leases
- wake broker counters
- optional 256-entry wake trace
- host epoch snapshots through native `epochs()`
- host error channel `latestFor(hostId)`
- deterministic `AppHarness` screen/style/history inspection
- native `Tui` error records surfaced at explicit `flush()`

The runtime error path is intentionally split:

```text
automatic drain
  → error channel accept/report
  → no throw

explicit flush
  → error channel throwPending
  → synchronous TuiError
```

### 7.3 Missing or limited visibility

- Wake counters and trace snapshots are not package-root exported.
- Attachment desired/visible counts are only available through direct internal binding object.
- Resource registry statistics are direct internal imports.
- No public API exposes pending desired versus visible root revisions; native epoch access is through internal host contracts/test surfaces.
- The inspected tests do not cover every close/exit aggregation combination.
- No runtime test was found for the observed `disposeOwnedHandles()` error-aggregation nuance.
- No test execution was performed during this report.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Runtime is the join point of all three planes

The runtime joins:

```text
TypeScript authoring/composition
        │
        ├── structural retained root
        ├── state/control attachments
        ├── content ports and source wakes
        └── native host/backend frame lifecycle
```

The runtime does not implement the semantic planes itself, but it determines when each plane is accepted, made desired, made visible, invalidated, or destroyed.

### 8.2 Desired structure and visible frame are intentionally separate

`Tui`, `AttachmentBindingState`, and `RetainedRootBoundary` all implement a two-phase lifecycle:

```text
prepare
  → desired commit
      → native host pending
          → native frame drain
              → visible commit
```

This coupling is consequential:

- disposing an attachment immediately after desired commit can be invalid;
- visible leases must outlive desired replacement;
- native host failure must not rewrite the visible frame;
- runtime cleanup must retire wake registrations before resource owners.

### 8.3 Resource ownership crosses runtime and transport boundaries

The runtime facade (`runtime/native-resource-registry.ts`) re-exports transport implementation. The public `FrameworkHandle` imports runtime handle registration, while runtime handle registration imports transport resource storage.

This is a deliberate ownership seam, but it is not a one-way dependency:

```text
api FrameworkHandle
  → runtime handle-registry
  → transport resources
  → transport resource-registry

runtime attachments
  → runtime resource facade
  → transport resource-registry
```

The resource registry remains plane-neutral and does not know state/content/component dispatch semantics.

### 8.4 Content wakes enter through the shared environment

The content FFI decodes native mutation results. If the native flags indicate `CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN`, the content API invokes:

```text
runtimeEnvironment().wakeBroker.markEnvironmentPending()
```

The broker then marks one registered host as pending. JavaScript does not retain a separate source-to-host subscription list. Native environment state is treated as authoritative.

### 8.5 Generic-framework boundary is respected

The runtime uses generic concepts:

- terminal
- output
- route
- source
- content
- history
- theme
- component
- state
- event

No Iyon-agent/application policy was found in the inspected runtime files. Route IDs and output payloads are opaque caller-supplied values. Runtime errors do not encode product semantics.

### 8.6 Native contract is a major seam

The runtime depends on native methods including:

- `dispose`
- `exit`
- `history`
- `viewState`
- `contentPort`
- `disposeContentResources`
- `textInput`
- `setTheme`
- `setHistory`
- `bindKey`
- `route`
- `interceptPaste`
- `dispatchKey`
- `dispatchPaste`
- `pollTerminal`
- `nextOutput`
- `waitForOutput`
- `nextWakeMs`
- `epochs`
- `clearViewStateBindings`
- `flushPendingHosts`

The TypeScript report can establish how these are called and how results are interpreted, but not the native implementation’s internal rollback, I/O, presentation receipt, or poison-state semantics. Those remain native-side evidence gaps.

### 8.7 Error-code handling has two conventions

There are two distinct error conventions:

1. Native/API errors use `ION_*` message prefixes and are converted by `asTuiError()`.
2. Runtime frame records use unprefixed codes such as `FRAME_PREPARATION_FAILED` and are directly placed in `TuiError.nativeCode`.

This is consistent within each path but means consumers should not assume all `TuiError.nativeCode` values use the `ION_*` prefix.

### 8.8 History attachment is stronger than ordinary scene replacement

Ordinary structural scene replacement is allowed. History replacement is not symmetric:

- a History can transition detached → attached;
- a different History cannot replace an already bound History;
- attached History remains associated with the original Tui owner;
- close/exit invalidates host-bound liveness.

This is a semantic ownership contract, not merely a cache or optimization.

### 8.9 Potential error-cleanup contradiction

`disposeRetainedExecution()` comments say scope projections, attachment bindings, builder roots, and runtime resources are retired before the host so queued work cannot outlive the Tui. The implementation does unregister the host first, but still invokes `host.disposeContentResources()` before invalidating the registry and disposing individual wrappers. That ordering is intentional for content-plane cascade, but native and registry ownership must remain consistent during the interval. The source tests should be treated as the behavioral authority for this interaction.

---

## 9. Open questions and coverage gaps

1. **Native frame semantics**
   - What exact native conditions produce `rearm`, `waiting_for_presentation`, and each frame error code?
   - How does native `flushPendingHosts()` choose and process hosts internally?

2. **Exit frame guarantees**
   - Does native `host.exit()` always synchronously present its final frame, or can the TypeScript teardown race an asynchronous presentation receipt?

3. **Content cascade**
   - What precisely does `disposeContentResources()` dispose?
   - How does it interact with individual ContentPort and ContentConnector wrapper disposal?

4. **Resource finalization**
   - Are weak finalizers expected to retire all abandoned handles in production, or are they only a safety net?
   - What native resource remains alive when a JavaScript wrapper dies while visible leases remain?

5. **History lifetime**
   - `historyOwners` is not explicitly cleared at Tui close. Is permanent attach-once ownership intended for the lifetime of the object, or should a closed Tui be able to release ownership without violating native History semantics?

6. **Owned-handle retry diagnostics**
   - Is retaining only the final all-failed disposal pass intentional?
   - Should earlier errors from partial failed passes be preserved for diagnosis?

7. **Attachment graph traversal**
   - The active-node set detects cycles along the current traversal stack. Ordinary DAG reuse is legal, but the exact intended semantics for a shared subtree containing distinct attachment identities are not documented beyond the duplicate-handle rejection test.

8. **Revision backlog**
   - There is no explicit upper bound on superseded attachment revisions while native presentation is delayed. Does native guarantee a bounded number of in-flight structural revisions?

9. **Environment singleton scope**
   - The code describes one environment per JavaScript realm/module instance but uses `Symbol.for`, which can span separately loaded module copies in the same realm. The intended behavior across duplicate package copies is not documented.

10. **Error listener lifecycle**
    - `Tui.onRuntimeError()` supports one listener, replacing any prior listener. Is single-listener ownership intentional, or should multiple subscribers be supported?

11. **Testing access**
    - `runtime/access.ts` is internal but provides substantial native inspection and event injection. The exact contract between production `Tui` and `AppHarness` is not represented in public types.

12. **Native resource kind compatibility**
    - `prepareResolve()` accepts `"content"` for expected `"content-port"`, but the broader resource-kind compatibility policy is not documented in the runtime files.

13. **Runtime test coverage**
    - Existing tests strongly cover PERF-13-A attachment/wake behavior, but there is less direct evidence for:
      - open cleanup aggregation;
      - exit failure cleanup;
      - repeated close after partial cleanup failure;
      - History ownership after close;
      - all combinations of desired/visible attachment replacement and failed native commits.

14. **No executed validation**
    - This report did not run tests or native code, so all runtime claims remain static/source-based.

---

## 10. Evidence appendix

### 10.1 Primary production manifest — exhaustively read

```text
packages/iyon-tui/src/runtime/access.ts
packages/iyon-tui/src/runtime/attachments.ts
packages/iyon-tui/src/runtime/environment.ts
packages/iyon-tui/src/runtime/error-channel.ts
packages/iyon-tui/src/runtime/events.ts
packages/iyon-tui/src/runtime/handle-registry.ts
packages/iyon-tui/src/runtime/native-resource-registry.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/runtime/wake-broker.ts
```

These nine paths are also listed at lines 958–966 of:

```text
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt
```

### 10.2 Supporting production files inspected

```text
packages/iyon-tui/src/transport/native/resource-registry.ts
packages/iyon-tui/src/transport/native/resources.ts
packages/iyon-tui/src/transport/native/addon.ts
packages/iyon-tui/src/api/errors.ts
packages/iyon-tui/src/api/controls/framework-handle.ts
packages/iyon-tui/src/api/content/retained.ts
packages/iyon-tui/src/transport/content/ffi.ts
packages/iyon-tui/src/testing/index.ts
packages/iyon-tui/src/index.ts
```

Supporting source was read for the runtime seams described in this report. Large unrelated sections of content and native transport were not treated as part of runtime LOC ownership.

### 10.3 Relevant tests inspected

```text
packages/iyon-tui/tests/tui_perf13_a.test.ts
packages/iyon-tui/tests/tui_runtime.test.ts
```

Other test files were indexed to identify runtime callers but not read exhaustively:

```text
packages/iyon-tui/tests/generated/view_abi_layout.test.ts
packages/iyon-tui/tests/tui_ansi_scanner.test.ts
packages/iyon-tui/tests/tui_demo.test.ts
packages/iyon-tui/tests/tui_generated_view_abi.test.ts
packages/iyon-tui/tests/tui_h3_a_semantic.test.ts
packages/iyon-tui/tests/tui_h3_b_composition.test.ts
packages/iyon-tui/tests/tui_h3_c_transport.test.ts
packages/iyon-tui/tests/tui_handles.test.ts
packages/iyon-tui/tests/tui_harness.test.ts
packages/iyon-tui/tests/tui_history_prefix.test.ts
packages/iyon-tui/tests/tui_native_builder.test.ts
packages/iyon-tui/tests/tui_native_input_validation.test.ts
packages/iyon-tui/tests/tui_native_persistent_seq.test.ts
packages/iyon-tui/tests/tui_native_scalar.test.ts
packages/iyon-tui/tests/tui_native_strings.test.ts
packages/iyon-tui/tests/tui_native_transaction.test.ts
packages/iyon-tui/tests/tui_perf13_b.test.ts
packages/iyon-tui/tests/tui_perf13_d.test.ts
packages/iyon-tui/tests/tui_perf13_h.test.ts
packages/iyon-tui/tests/tui_realtime.test.ts
packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts
packages/iyon-tui/tests/tui_semantic_pipeline.test.ts
packages/iyon-tui/tests/tui_smooth_delivery.test.ts
packages/iyon-tui/tests/tui_state_envelope.test.ts
packages/iyon-tui/tests/tui_surface_contract.test.ts
packages/iyon-tui/tests/tui_t14_differential.test.ts
packages/iyon-tui/tests/tui_t14_fuzz_property.test.ts
packages/iyon-tui/tests/tui_text_lanes.test.ts
packages/iyon-tui/tests/tui_traits.test.ts
packages/iyon-tui/tests/tui_values.test.ts
```

### 10.4 Key symbols

#### Runtime lifecycle

```text
Tui
Tui.open
Tui.render
Tui.renderCanonical
Tui.renderDirect
Tui.flush
Tui.close
Tui.exit
Tui.resize
Tui.setTheme
Tui.nextEvent
Tui.prepareRootPublication
Tui.commitHistoryBinding
Tui.stageHistoryBinding
Tui.disposeRetainedExecution
Tui.disposeOwnedHandles
Tui.commitVisibleAfterDrain
```

#### Attachments

```text
AttachmentRuntimeContext
PreparedAttachmentSet
AttachmentBindingState
AttachmentBindingState.commitDesired
AttachmentBindingState.commitVisible
AttachmentBindingState.dispose
prepareAttachmentsForView
prepareSemanticAttachments
validateSemanticAttachments
PreparedAttachmentSetImpl
```

#### Resources

```text
registerFrameworkHandle
handleIdOf
releaseFrameworkHandle
disposeFrameworkResource
NativeResourceRegistry
PreparedResourceLease
runtimeResourceEnvironment
runtimeResourceRegistry
registerNativeResource
nativeResourceOf
nativeResourceForHandleId
disposeNativeResource
releaseNativeResource
```

#### Wake broker

```text
EnvironmentWakeBroker
RuntimeHostRegistration
EnvironmentWakeBroker.register
EnvironmentWakeBroker.markPending
EnvironmentWakeBroker.markEnvironmentPending
EnvironmentWakeBroker.flush
EnvironmentWakeBroker.drainAutomatically
EnvironmentWakeBroker.drain
EnvironmentWakeBroker.consumeReport
EnvironmentWakeBroker.schedulePresentationPoll
wakeBrokerCounterSnapshot
resetWakeBrokerCounters
wakeTraceSnapshot
```

#### Errors/events/testing

```text
RuntimeErrorChannel
RuntimeFrameErrorRecord
RuntimeFrameErrorCode
RuntimeErrorReporter
RuntimeInputEvent
RuntimeAccess
registerRuntimeAccess
runtimeAccess
OutputEvent
TerminateEvent
TuiEvent
```

### 10.5 Approximate LOC methodology

- Counts are physical source lines based on file line positions from the inspected source.
- Counts include comments, blank lines, interfaces, and type declarations.
- Generated files were excluded from runtime production totals.
- Tests were counted separately where directly inspected:
  - `tui_perf13_a.test.ts`: approximately 358 physical lines
  - `tui_runtime.test.ts`: approximately 31 physical lines
- No executable statement, branch, coverage, or benchmark count was inferred.
- No tests were run during this investigation.

### 10.6 Commands/tools represented by the inspection

The evidence was obtained through read-only repository file listing and content search over:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- tracked source manifest
- runtime production files
- supporting transport/API files
- relevant test files

No source or configuration files were edited, no dependencies were installed, no services were started, and no external agents were launched.