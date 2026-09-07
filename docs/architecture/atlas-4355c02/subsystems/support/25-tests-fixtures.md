# 25 — Tests, Fixtures, and Test Infrastructure

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope assigned by the atlas: Rust/TypeScript tests, `packages/iyon-tui/src/testing/`, and `packages/tui-consumer-fixture/`.
- The atlas contract and README were read first:
  - `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - `docs/architecture/atlas-4355c02/README.md`
- `AGENTS.md` was read, including the mandatory generic-framework boundary.
- `PRE-V5-ARCHITECTURE-REPORT.md` was read for its inventory/evidence/test-contract requirements. This report does not perform V5 disposition analysis.

### Scope interpretation

This report treats “test infrastructure” broadly enough to include:

1. The TypeScript headless harness exposed through `@iyon/tui/testing`.
2. Rust unit-test-only fixtures, fake backends, test drivers, and `test-util` seams.
3. TypeScript package tests and their local fixtures.
4. The external-consumer fixture package and its public-API-only contract.
5. Native-addon integration tests and generated ABI compile/signature tests.
6. Repository-level commands and CI that establish which test suites actually run.
7. Tests that intentionally bypass production entrypoints to test lower-level routes.

The generic-framework boundary is important here. The repository’s tests and fixtures are allowed to contain generic terminal concepts such as history, streams, projections, state, slots, themes, and controls. No test fixture should encode Iyon agent/application semantics. The consumer fixture uses generic `title`, `status`, `items`, `showHint`, and `footer` concepts rather than agent/product vocabulary.

### Evidence status

This was a read-only static investigation. No source, configuration, generated file, fixture, or documentation was edited. No dependencies were installed. No test command or build suite was executed in this investigation, so this report makes no claim that tests passed.

The source tree, manifests, CI workflow, scoped testing files, and the evidence manifest were inspected. The repository-wide Rust test tree is large and includes substantial inline test modules, especially in `application/content.rs`, `scene/host.rs`, presentation, terminal, and stream/content code. Those suites were indexed and their test infrastructure was traced, but not every assertion in every large inline module was reproduced here.

### Primary observations

1. The TypeScript test boundary is intentionally split:
   - `@iyon/tui` is the public framework API.
   - `@iyon/tui/testing` is a dedicated headless/deterministic test entrypoint.
   - Most package tests exercise either the public root or internal implementation paths, depending on the contract being tested.
2. `AppHarness` is not a production application abstraction. It is test infrastructure layered over production `Tui`, using the private runtime-access registry to expose deterministic input, clock, and inspection operations.
3. `packages/tui-consumer-fixture/` is specifically designed to prove that an external consumer can use only documented public APIs and does not need composition/compiler/plugin setup.
4. Rust has no supported public authoring surface. Rust tests use `pub(crate)`, `#[cfg(test)]`, `test-util`, and in-crate reexports to test the implementation kernel.
5. Native integration tests cover two distinct contracts:
   - Generated ABI/signature/linkage behavior.
   - Stable addon smoke/version markers.
6. There are deliberate lower-level differential/oracle helpers, especially `packages/iyon-tui/tests/fixtures/native-host.ts`. These are valuable for validating native retained operations, but they do not prove the complete public `Tui` route by themselves.

---

## 1. Responsibility and structure

### 1.1 Test infrastructure inventory

| Path | Language | Role | Approximate physical size | Public surface |
|---|---|---|---:|---|
| `packages/iyon-tui/src/testing/index.ts` | TypeScript | Deterministic headless application harness | 154 lines | Exported through `@iyon/tui/testing`; test-only API |
| `packages/iyon-tui/src/runtime/access.ts` | TypeScript | Private `WeakMap` bridge from `Tui` to test inspection/control functions | 27 lines | Not exported from package root |
| `packages/iyon-tui/tests/fixtures/native-host.ts` | TypeScript | Direct retained/native host reference renderer for differential tests | 29 lines | Test-local helper |
| `packages/iyon-tui/tests/fixtures/tui_demo.ts` | TypeScript | Public-API integration scenario fixture | 47 lines | Test-local helper |
| `packages/tui-consumer-fixture/src/consumer.ts` | TypeScript | Third-party-shaped public consumer source | 170 lines | Fixture package source |
| `packages/tui-consumer-fixture/tests/consumer.test.ts` | TypeScript | Public consumer skeleton acceptance tests | 119 lines | Test-only |
| `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts` | TypeScript | Public consumer retained-composition/invalidation acceptance tests | 191 lines | Test-only |
| `packages/iyon-tui/tests/` | TypeScript | Package-level public, native, transport, semantic, runtime, ABI, differential, and regression tests | 34 tracked files including one worker helper; several thousand lines overall | Test-only |
| `crates/iyon-tui/src/**/tests*.rs` and inline `#[cfg(test)]` modules | Rust | Core implementation unit tests and test-only support | Dozens of files; large aggregate, with especially large application/content suites | Internal only |
| `crates/iyon-tui-native/tests/generated_view_abi.rs` | Rust/generated | Generated ABI conformance/linkage test target | Large generated file | Test target only |
| `crates/iyon-tui-native/tests/sync.rs` | Rust | Native addon marker/smoke contract | 10 lines | Test target only |

The evidence manifest identifies 37 standalone Rust test files:

- 2 native-crate integration tests.
- 2 application test files.
- 3 component test files.
- 9 content/text test files.
- 5 text-input test files.
- 1 interaction test file.
- 4 output test files.
- 1 physical test file.
- 5 presentation/layout test files.
- 3 projection test files.
- 2 scene test files.

This list does not include the many production source files containing inline `#[cfg(test)]` modules.

The TypeScript manifest identifies 34 files below `packages/iyon-tui/tests/`, of which 33 are test or test-support files and one is the worker helper `tui_worker_lifecycle.ts`. The consumer fixture adds two source/test files and a package manifest.

### 1.2 `AppHarness` responsibility

`packages/iyon-tui/src/testing/index.ts` contains:

- `AppHarnessContract`, an extension of `TuiRuntime`.
- `AppHarness`, wrapping a production `Tui`.
- `createAppHarness`, an alias for `AppHarness.open`.

The harness adds deterministic test operations that are intentionally absent from the ordinary runtime interface:

- `pressKey(key, modifiers?)`
- `paste(text)`
- `advance(ms)`
- `screenRows()`
- `nativeHistoryRows()`
- `styleAt(row, column)`
- `cellXOfText(row, text)`
- `exited()`
- `now()`

It also forwards production operations such as:

- `render`
- `flush`
- `nextEvent`
- `resize`
- `createHistory`
- `viewState`
- `contentPort`
- `createTextInput`
- `createViewSlot`
- `createScrollPane`
- `bindKey`
- `route`
- `interceptPaste`
- `forwardPaste`
- `setTheme`
- `close`
- `exit`

The harness therefore does not own terminal rendering, retained state, content, or scheduling policy. It owns only deterministic test control and observation around a real production `Tui`.

### 1.3 Rust test-support responsibility

Rust tests are primarily in-crate implementation tests rather than public API tests. The crate documentation in `crates/iyon-tui/src/lib.rs` explicitly describes the Rust crate as an unpublished implementation crate. Most implementation modules are `pub(crate)` or private, and test-only reexports are guarded by `#[cfg(test)]`.

Important Rust support components include:

- `crates/iyon-tui/src/application/tests.rs`
  - Fake terminal backend.
  - Headless native-history sink.
  - Presentation receipt controls.
  - Application test state/actions.
  - Application lifecycle, input, timers, history, focus, and error tests.
- `crates/iyon-tui/src/application/tests/driver.rs`
  - `TestTerminalInput`.
  - `RunError`.
  - `RuntimeError`.
  - `run_with_backend`.
  - `run_with_backend_factory`.
  - `PresentationScheduler`.
  - `TerminalSession`.
  - Test-side event/frame driving.
- `crates/iyon-tui/src/perf.rs`
  - `test_lock()` and `TestLockGuard` serialize tests that inspect shared performance counters or global instrumentation.
- `#[cfg(any(test, feature = "test-util"))]` helpers in physical/text/application code.
  - These support in-tree tests and selected cross-crate tests.
  - They are not a supported Rust authoring/test API.

### 1.4 Approximate LOC method and caveat

Exact aggregate LOC was not recomputed with an execution command. The estimates below use source line ranges and the tracked-source manifest:

- Exact line extents are given for the small, central harness and fixture files.
- Rust standalone test files are counted by manifest path.
- Large inline suites are identified by their source ranges and classified separately.
- Generated ABI test source is excluded from production/test logic estimates where practical.

Approximate size buckets:

- TypeScript test infrastructure and fixtures:
  - `src/testing/index.ts`: 154 lines.
  - Consumer fixture source: 170 lines.
  - Consumer fixture tests: 310 lines.
  - Package fixture helpers: 76 lines.
  - The 33 package tests comprise several thousand lines, approximately 3,500–5,000 lines including helper code.
- Rust test source:
  - At least 37 standalone test files.
  - A substantial amount of inline test code exists in production modules.
  - `application/content.rs` contains a very large test section beginning around line 7010 and continuing beyond line 10,700.
  - `scene/host.rs`, terminal presenter/shadow code, presentation/layout, and content/render modules also contain extensive inline tests.
  - The aggregate Rust test code is plausibly in the low tens of thousands of lines, but that aggregate is not asserted as an exact count here.

---

## 2. Types, APIs and contracts

### 2.1 `AppHarnessContract`

The contract at `packages/iyon-tui/src/testing/index.ts:18-34` extends `TuiRuntime` and adds deterministic behavior.

Key invariants:

#### Headless-only construction

`AppHarness.open()` calls:

```ts
const tui = await Tui.open({ ...options, headless: true });
```

The caller may supply dimensions, theme, or signal, but `headless: true` is forced. This is a deliberate test guarantee: the harness never opens a real interactive terminal.

#### Deterministic clock

`AppHarness` maintains a private `clock` initialized to zero.

`advance(ms)` validates:

- `ms` is a safe integer.
- `ms >= 0`.
- `ms` cannot overflow the deterministic clock.

It then:

1. Flushes pending work.
2. Advances the native host clock.
3. Increments the JavaScript-side clock only after native advancement succeeds.

The comment at lines 106-107 explicitly establishes transactional semantics: a failed native advancement must not advance `now()`.

#### Deterministic input

`pressKey` and `paste`:

1. Enter `callTesting`.
2. Flush the runtime.
3. Enqueue a key or paste event through `runtimeAccess`.
4. Leave actual event interpretation to the native host/runtime.

The harness does not interpret key semantics itself. It passes key strings/modifiers to the production host.

#### Deterministic snapshot inspection

`screenRows`, `nativeHistoryRows`, `styleAt`, and `cellXOfText` all call `inspect`.

While the terminal is considered live, `inspect`:

1. Flushes the retained/native work.
2. Advances the native host by zero milliseconds.
3. Reads the requested snapshot.

After `AppHarness.exit()` sets `terminalExited`, inspection skips the additional flush/zero-time barrier because the final frame has already been committed and reopening a closed barrier would be invalid.

#### Error normalization

Most test-control operations use `callTesting`, which catches arbitrary errors and converts them through `asTuiError`.

This makes errors from native and runtime operations observable as the framework’s `TuiError` shape rather than exposing arbitrary native exceptions.

Not every forwarded operation uses the wrapper:

- `flush()` directly calls `this.tui.flush()`.
- `close()` directly calls `this.tui.close()`.
- `exit()` directly calls `this.tui.exit()`.

That difference is an observable contract and a possible consistency gap. It is not necessarily a bug: close/exit failures may intentionally preserve aggregate cleanup errors rather than normalize them, but the source does not document the asymmetry.

### 2.2 Private runtime access contract

`packages/iyon-tui/src/runtime/access.ts:1-27` defines:

```ts
type RuntimeAccess = {
  flush(): void;
  enqueue(event: RuntimeInputEvent): void;
  screenRows(): readonly string[];
  nativeHistoryRows(): readonly string[];
  styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
  cellXOfText(row: number, text: string): number | null;
  advance(milliseconds: number): void;
  exited(): boolean;
};
```

A `WeakMap<object, RuntimeAccess>` maps each production runtime owner to these operations.

- `registerRuntimeAccess(owner, access)` is called by `Tui` construction.
- `runtimeAccess(owner)` retrieves the registered access object.
- Missing registration throws `"TUI testing access is unavailable"`.

This is not a general public runtime API. It is an internal test seam consumed only by `AppHarness`. The `WeakMap` ensures the test access record does not independently keep a `Tui` alive.

### 2.3 Production registration seam

`packages/iyon-tui/src/runtime/runtime.ts:197-214` registers the access functions when `Tui` is constructed:

- `flush` forwards to `Tui.flush`.
- `enqueue` dispatches key, paste, or resize to the native host.
- `screenRows` and `nativeHistoryRows` query native snapshots.
- `styleAt` converts a nullable native style result into a runtime error if absent.
- `cellXOfText` forwards Unicode-aware cell lookup.
- `advance` forwards deterministic native time.
- `exited` queries host exit status.

This creates a clean responsibility split:

```text
AppHarness
  -> runtimeAccess(Tui)
      -> production Tui/native host
```

The harness supplies deterministic driving; the production runtime remains responsible for interpreting and applying operations.

### 2.4 Public consumer fixture API

`packages/tui-consumer-fixture/src/consumer.ts` exposes two styles of consumer source.

#### Direct public consumer

`buildConsumerBody` builds a generic vertical view from:

- `ConsumerState`
- `TextInput`
- `ViewSlot`
- `ScrollPane`

The body includes:

- Title.
- Optional hint.
- List slot.
- Composer.
- Scroll pane.
- Footer.

It uses only public `View`, `Style`, `Insets`, and control APIs.

`openConsumerSession`:

1. Opens `AppHarness` at `60x16`.
2. Creates public handles:
   - `TextInput`
   - `ViewSlot`
   - `ScrollPane`
   - `History`
3. Provides `render(state)`.
4. Provides explicit `close()` cleanup.

The session’s close method disposes handles before closing the owning harness:

```text
composer.dispose()
listSlot.dispose()
pane.dispose()
history.dispose()
tui.close()
```

This establishes explicit handle lifetime ownership in the fixture.

#### Public composition consumer

`buildScopedConsumer` uses only public:

- `defineView`
- `state`
- `View.key`
- `View`
- `Scene`
- `TuiRuntime.render`

It creates:

- `status` state.
- `items` state.
- `Header` component.
- `ItemCard` component.
- `Footer` component.
- `App` component.

It records execution counts so tests can distinguish:

- Root application execution.
- Header execution.
- Per-key card execution.

The source intentionally contains no composition setup, compiler invocation, identity discipline, plugin registration, or feature flag. It expects `Tui.render` to activate retained composition automatically.

### 2.5 Public API versus internal test API

| Surface | Intended consumer | Internal machinery? | Evidence |
|---|---|---:|---|
| `@iyon/tui` root exports | External TypeScript framework consumer | No; documented authoring/runtime surface | Root `package.json:7-10`, `src/index.ts` |
| `@iyon/tui/testing` | TypeScript tests and deterministic headless consumers | Yes; test infrastructure | Package export and `src/testing/index.ts` |
| `runtime/access.ts` | `AppHarness` only | Yes | Not exported from root/package subpath |
| `renderRetained` | Native retained differential tests | Yes | `tests/fixtures/native-host.ts:10-29` |
| Rust `pub(crate)` reexports | In-crate implementation tests and native binding | Yes | `crates/iyon-tui/src/lib.rs:13-117` |
| Rust `test-util` feature | Cross-crate/in-tree tests | Yes | `crates/iyon-tui/Cargo.toml:11-18` |
| Generated ABI test target | ABI generator/native conformance | Yes | `crates/iyon-tui-native/tests/generated_view_abi.rs` |

---

## 3. Dependency and ownership map

### 3.1 TypeScript test dependency graph

```text
packages/tui-consumer-fixture/tests/*
        │
        ├── imports ../src/consumer.ts
        │       └── imports @iyon/tui
        │               └── packages/iyon-tui/src/index.ts
        │
        └── imports @iyon/tui/testing
                └── packages/iyon-tui/src/testing/index.ts
                        ├── runtime/runtime.ts
                        └── runtime/access.ts
                                └── WeakMap registered by Tui
                                        └── native host/addon
```

Package tests have two major routes:

```text
Public/runtime tests:
  tests/*
    -> src/index.ts
    -> public APIs
    -> Tui
    -> native addon

Lower-level/native tests:
  tests/*
    -> src/transport/*
    -> src/composition/*
    -> generated ABI calls
    -> native addon or test host
```

The lower-level route is intentional for testing implementation contracts such as ABI framing, persistent sequence edits, generated calls, structural transactions, and malformed payload atomicity.

### 3.2 Native retained differential route

```text
View semantic value
    ↓
tryRetainedMaterializeRef(view)
    ↓
generated/native retained reference
    ↓
hostRenderRef(...)
    ↓
NativeTuiHostContract
    ↓
headless rows/styles
```

`renderRetained` creates a fresh ABI session, materializes a retained reference, renders it through the native host, and always releases the reference in `finally`.

The helper is an oracle/reference path for tests such as:

- Native scalar edits.
- Persistent sequence edits.
- Native builder constructors.
- Native transaction parity.
- Native string/style parity.

It is not equivalent to `Tui.render` plus runtime scheduling. A test using this helper can verify native retained rendering while bypassing application-level root publication, composition scheduling, or runtime ownership.

### 3.3 Rust test dependency graph

```text
Rust production modules
    ├── private/pub(crate) runtime types
    ├── #[cfg(test)] module imports
    ├── fake backend / fake sink / test driver
    └── assertions over internal state and prepared frames

iyon-tui-native integration tests
    ├── generated ABI test target
    │      └── generated exports/table/types/conformance
    └── sync.rs
           └── public native crate markers
```

The Rust crate’s architecture is intentionally not a public Rust UI API. Tests can access implementation internals because they are compiled inside the crate. This is materially different from the TypeScript consumer fixture, whose purpose is to prove external API reachability.

### 3.4 Ownership/lifetime map

| Object | Created by | Destroyed/disposed by | Test evidence |
|---|---|---|---|
| `Tui` | `AppHarness.open`, direct `Tui.open` | `close` or `exit` | `src/testing/index.ts:45-48`, runtime lifecycle tests |
| `AppHarness` clock | Harness constructor | Harness object lifetime | `src/testing/index.ts:37-39` |
| Runtime access registration | `Tui` constructor | WeakMap entry becomes collectible with owner | `runtime/runtime.ts:197-214`, `runtime/access.ts:17-27` |
| History handle | `Tui.createHistory` or direct public constructor in some tests | Explicit `dispose`, then host close | Consumer fixture and handle tests |
| View slot | `Tui.createViewSlot` | Explicit `dispose` or owner teardown | Consumer fixture, harness tests |
| Scroll pane | `Tui.createScrollPane` | Explicit `dispose` or owner teardown | Consumer fixture |
| Text input | `Tui.createTextInput` | Explicit `dispose` or owner teardown | Consumer fixture and harness tests |
| Native retained ref in `renderRetained` | `tryRetainedMaterializeRef` | `viewReleaseMany` in `finally` | `native-host.ts:16-29` |
| Rust fake backend | Test function | Rust scope/drop | `application/tests.rs` |
| Rust presentation scheduler | Test driver | Test driver scope | `application/tests/driver.rs` |
| Rust performance lock | `test_lock()` | `TestLockGuard::drop` | `perf.rs:149-164` |

### 3.5 Important reverse dependencies

- `Tui` owns the runtime access registration used by the test harness.
- `AppHarness` depends on runtime internals but not vice versa.
- Production runtime does not depend on test fixture source.
- The consumer fixture depends on the public package and testing subpath; it does not depend on `composition/*`, `transport/*`, generated ABI modules, or native internals.
- Native ABI generated tests depend on generated implementation files and therefore detect generator/schema drift rather than public framework behavior.
- Rust application tests depend on fake terminal backend traits and internal application kernel types, allowing tests to drive events and presentations without a real terminal.

---

## 4. Execution paths and state transitions

### 4.1 AppHarness opening

```text
AppHarness.open(options)
    ↓
Tui.open({ ...options, headless: true })
    ↓
Tui constructor
    ├── creates native host
    ├── creates retained execution runtime
    ├── registers host/runtime resources
    └── registerRuntimeAccess(this, access)
    ↓
new AppHarness(tui)
```

The test harness does not create a parallel fake runtime. It creates the actual production runtime in headless mode.

### 4.2 Initial render

`AppHarness.render(scene, signal?)`:

```text
AppHarness.render
    ↓
Tui.render(scene, signal)
    ↓
canonical or direct production render path
    ↓
AppHarness.callTesting(() => runtimeAccess(tui).advance(0))
    ↓
native zero-time drain
    ↓
visible deterministic frame
```

The extra zero-time advancement is important. A call to `render` is expected to leave the test-observable native snapshot coherent before returning.

The production runtime distinguishes direct scene values and scene producers. The testing harness accepts the same `SceneProducer` type as `TuiRuntime` and does not choose the route itself.

### 4.3 Input path

For keys:

```text
AppHarness.pressKey(key, modifiers)
    ↓
runtimeAccess(tui).flush()
    ↓
runtimeAccess(tui).enqueue({ type: "key", key, modifiers })
    ↓
Tui runtime access enqueue
    ↓
host.dispatchKey(...)
    ↓
native interaction/focus/component routing
    ↓
output/action/event delivery
```

For paste:

```text
AppHarness.paste(text)
    ↓
flush
    ↓
enqueue({ type: "paste", text })
    ↓
host.dispatchPaste(text)
    ↓
native paste interception/input path
```

The harness does not implement focus, key commands, text editing, or routing. Those remain production/native responsibilities.

### 4.4 Deterministic clock path

```text
AppHarness.advance(ms)
    ↓
validate safe integer and overflow
    ↓
runtimeAccess(tui).flush()
    ↓
runtimeAccess(tui).advance(ms)
    ↓
native host deterministic clock
    ↓
slot animation / smooth delivery / scheduled work
    ↓
clock += ms
```

Failure behavior:

- Invalid `ms` fails before native interaction.
- Native advancement failure leaves the public harness clock unchanged.
- Subsequent snapshot inspection may still surface native/runtime errors through `callTesting`.

### 4.5 Snapshot path

```text
AppHarness.screenRows()
    ↓
inspect(access)
    ├── if live:
    │      access.flush()
    │      access.advance(0)
    └── access.screenRows()
```

The same route serves:

- Screen rows.
- Native history rows.
- Cell styles.
- Unicode-aware text cell positions.

This provides a consistent observation barrier across tests.

### 4.6 Exit/destruction path

`AppHarness.exit()`:

1. Sets the harness-local `terminalExited` flag.
2. Calls production `Tui.exit()`.

Afterward, inspection avoids flush/zero-time advancement and reads the already committed final frame. `AppHarness.close()` calls `Tui.close()` but does not set `terminalExited`; this reflects the distinction between terminal exit and ordinary shutdown.

Potential uncertainty: if a production path causes the native host to report exited without the caller invoking `AppHarness.exit()`, `terminalExited` remains false. The harness would still attempt its live inspection barrier. The source does not document whether that state can occur through the public test API.

### 4.7 External fixture direct rendering

`openConsumerSession().render(state)`:

```text
render(state)
    ↓
buildConsumerBody(state, handles)
    ↓
new Scene(body, history)
    ↓
tui.render(scene)
```

This is a direct scene path. The fixture’s first suite tests rendering and handle updates without requiring retained component composition.

### 4.8 External fixture scoped composition rendering

`buildScopedConsumer(tui).renderApp()`:

```text
tui.render(() => new Scene(App({})))
    ↓
Tui canonical producer path
    ↓
retained execution root
    ↓
App / Header / ItemCard / Footer scopes
    ↓
semantic View tree
    ↓
native retained publication
```

State writes do not call `renderApp` again. The test expects tracked state writes to enqueue and drain the appropriate scope automatically.

### 4.9 Scoped state transition contract

The test at `scoped-invalidation.test.ts:40-70` establishes:

```text
Initial render:
  App=1, Header=1, Card(a)=1, Card(b)=1

status.set("running"):
  App unchanged
  Header increments
  Card(a), Card(b) unchanged
```

This is a route assertion, not merely a screen assertion. It proves both:

- The changed header text becomes visible.
- The execution frontier excludes unaffected parent/sibling scopes.

The 1,000-sibling test at lines 72-114 verifies the same frontier property at scale. It mutates one `state` object and expects only one child body to rerun, including after a geometry-changing text update.

### 4.10 Keyed reorder transition

The consumer test at `scoped-invalidation.test.ts:116-153` establishes:

```text
Initial:
  App = 1
  Card(a) = 1
  Card(b) = 1

Reverse keyed items:
  App increments
  Card(a), Card(b) do not rerun

Change only item b's label:
  App increments
  Card(b) increments
  Card(a) remains unchanged
```

The test proves the public `View.key` contract:

- Keys preserve per-item identity across reorder.
- Shallow-equal props allow bodies to be skipped.
- A changed keyed item reruns only that item’s body.

### 4.11 Slot ownership transition

The test at `scoped-invalidation.test.ts:155-191` exercises:

```text
builder-owned slot
    ↓
tracked label changes
    ↓
builder update visible
    ↓
direct View installed
    ↓
builder ownership disposed
    ↓
stale tracked label changes
    ↓
no ghost overwrite
```

This is a particularly consequential ownership contract. `ViewSlot` accepts either a builder or direct view. Installing the direct value must invalidate/dispose the previous builder’s ability to publish later.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production/test path matrix

| Semantic operation | Primary tested route | Alternate/lower-level route | Failure/recovery behavior |
|---|---|---|---|
| Render a scene | `Tui.render` via `AppHarness` or public fixture | Direct native retained host via `renderRetained` | Retained refusal and publication errors are explicit |
| Update tracked state | Public `state().set()` through canonical `Tui.render` producer | Direct internal composition tests | Scheduler drains affected scopes; errors are surfaced through runtime channels |
| Update a slot | Public `ViewSlot.setView` | Internal native handle/transport tests | Latest value should win; stale builder must not publish |
| Advance animation/smoothing | `AppHarness.advance(ms)` | Real-time runtime test with wall-clock sleep | Deterministic route validates clock; real-time route validates production timer behavior |
| Input key | `AppHarness.pressKey` → native dispatch | Rust `App`/interaction tests or direct host tests | Native routing determines consumed/ignored behavior |
| Paste | `AppHarness.paste` → native dispatch | Rust fake terminal/application tests | Interceptor and focused component semantics are tested |
| Inspect screen | `screenRows` after flush/zero-time barrier | Fake backend frame inspection or native host rows | Snapshot may fail if native style/cell state is unavailable |
| Inspect styles | `styleAt` | Native ABI and paint tests | Null native style becomes a runtime error |
| Inspect Unicode position | `cellXOfText` | Physical cell/glyph tests | Uses terminal-cell, not JavaScript string-index semantics |
| History output | `nativeHistoryRows` and public `History` | Rust history/application tests | History sideband is tested independently from body |
| ABI operation | Generated TS calls / native generated integration test | Direct internal transport tests | Invalid payloads should fail atomically; generated signatures are pinned |
| Native retained edit | `Tui` retained path | `renderRetained` reference helper | Reference helper catches incremental parity but can bypass high-level root publication |
| Native cleanup | `close`, `exit`, explicit handle disposal | Rust lifetime and native resource tests | Stale/cross-host/duplicate handles must error explicitly |

### 5.2 Intentional alternate routes

The repository contains legitimate alternate testing routes rather than one universal test path:

1. **Public behavior route**
   - Tests root exports and `AppHarness`.
   - Best evidence for external TypeScript consumer contracts.
2. **Composition internals route**
   - Tests execution scopes, semantic identity, memoization, publication, and child ownership.
   - Uses internal `src/composition/*` imports.
3. **Transport/ABI route**
   - Tests generated calls, retained node framing, path edits, and native transaction behavior.
   - Uses internal transport modules and generated calls.
4. **Native-host differential route**
   - Creates a fresh retained rendering reference.
   - Compares incremental/optimized updates against a fresh retained result.
5. **Rust kernel route**
   - Tests application, layout, scene, content, interaction, and terminal implementation directly with private types and fake backends.
6. **Generated conformance route**
   - Tests generated Rust/TypeScript ABI files against checked-in schema/manifests.

These routes should not be conflated. A public fixture test passing does not prove every internal transport operation; a generated ABI test passing does not prove the public runtime route; a lower-level differential test passing can still miss a shared bug in both compared implementations.

### 5.3 Failure semantics in the harness

Observed explicit failures include:

- Invalid deterministic clock advancement:
  - `tuiError("validation", "clock advancement must keep the deterministic clock within safe integer range")`.
- Missing runtime test access:
  - `"TUI testing access is unavailable"`.
- Native style lookup with no style:
  - `tuiError("runtime", "native cell style is unavailable")`.
- Arbitrary runtime/native errors:
  - Normalized using `asTuiError` in `callTesting`.
- Native retained materialization refusal:
  - `renderRetained` throws `"retained test materialization refused the view"`.
- Host render failure:
  - `renderRetained` throws `"native host-ref render failed"`.

The harness does not silently convert failed `advance` calls into successful clock changes. It also does not hide retained materialization refusal.

### 5.4 Failure semantics in Rust test drivers

`application/tests/driver.rs` makes test-side runtime errors explicit through:

- `RuntimeError`.
- `RunError<ApplicationError>`.
- Error source preservation and downcasting.

`application/tests.rs:59-73` verifies that:

- A wrapped top-level runtime cause remains available through `Error::source`.
- An `OutputDispatchError` can still be downcast through the source chain.

The fake backend can independently fail:

- Event acquisition.
- Viewport lookup.
- Drawing.
- Terminal restore.
- Presentation completion.

It can also delay presentation receipts to exercise asynchronous presentation polling and scheduling.

### 5.5 Potential masking and oracle limitations

#### `renderRetained` reference limitations

`renderRetained` is deliberately a low-level helper. It does not:

- Exercise `Tui.render`.
- Exercise the runtime retained scheduler.
- Exercise root desired/visible publication separation.
- Exercise history sideband binding.
- Exercise public handle ownership.

It is appropriate for native retained parity, not as the sole acceptance route for public runtime behavior.

#### Screen-only tests

Several tests assert only row text or style snapshots. Such tests prove user-visible effects but do not necessarily prove:

- Which execution scope reran.
- Whether a structural publication occurred.
- Whether cache reuse occurred.
- Whether the intended route was selected.

The scoped invalidation tests are stronger because they combine counters with screen assertions.

#### Direct internal imports

Many package tests import files such as:

- `src/transport/native/addon.ts`
- `src/transport/structural/native-view-abi.ts`
- `src/composition/execution.ts`
- Generated ABI calls.
- Internal semantic-node helpers.

These are necessary implementation tests, but they are not external API evidence. The consumer fixture is the repository’s stronger public API contract.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Harness scheduling

The harness uses production scheduling rather than reimplementing it.

- `render()` invokes `Tui.render` and then a native zero-time advancement.
- `screenRows()` and related inspection methods flush and zero-advance before reading.
- `advance(ms)` flushes before moving the deterministic native clock.
- Tracked state updates are expected to drain via microtasks in canonical runtime tests.

This yields a consistent test observation rule:

> Tests can read a coherent frame after a harness operation without manually knowing the native barrier sequence.

### 6.2 Microtask draining in the consumer fixture

`scoped-invalidation.test.ts:32-37` defines:

```ts
async function drain(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
```

The comments document the scheduling contract:

- `scheduleFlush` queues a microtask.
- The first hop runs the flush.
- The second hop resumes the test after the flush has completed.

This is an explicit test dependency on the retained composition scheduler’s asynchronous batching behavior.

### 6.3 Execution frontier counters

The consumer fixture maintains counters in caller-owned closures:

- `appCount`.
- `headerCount`.
- `cardCounts`.

The counters are not production instrumentation. They are test observability attached to public component bodies. This is useful because the desired contract is not just “screen eventually changes”; it is “only the dependency-reading scope reruns.”

The 1,000-sibling case is a performance/architecture test:

- Initial evaluation calls all 1,000 child scopes once.
- A write to one state value reruns only child 500.
- A geometry-changing update still reruns only child 500.
- The test explicitly delegates layout/paint propagation to native differential tests, avoiding duplicate assertions.

### 6.4 Rust performance serialization

`crates/iyon-tui/src/perf.rs` provides a test lock. Presentation layout tests, scene/host tests, and application/content tests acquire it around counter-sensitive sections.

This indicates that some test-observed counters or global instrumentation are shared and require serialization. The lock is test infrastructure, not runtime scheduling policy.

### 6.5 Generated and ABI performance tests

The TypeScript package includes tests for:

- Scalar retained operations.
- Persistent sequence operations.
- Native builders.
- Native strings/style atoms.
- Native transactions.
- Differential retained rendering.
- Malformed-boundary properties.

These suites often compare screen snapshots against a fresh/reference render and assert identity or path behavior. They are intended to guard fast retained routes and zero-copy/low-allocation ABI contracts.

### 6.6 Cache invalidation evidence

The following test groups exercise invalidation/cache behavior:

- `tui_semantic_cache_ownership.test.ts`
  - Semantic sequence cache ownership.
  - Repeated Markdown replacement.
  - Parser reset across source content generations.
- `tui_retained_scene_regressions.test.ts`
  - State dependency invalidation before structural publication.
  - Full-paint theme obligations during content refresh.
  - Captured state values through slot replacement.
- `tui_perf13_a.test.ts`
  - Attachment leases.
  - Coalesced wakes.
  - Desired structural publication versus visible frame commit.
- `tui_perf13_b.test.ts`
  - Presentation overrides without republishing structure.
  - State attachment identity and remount behavior.
- `tui_perf13_d.test.ts` and `tui_perf13_h.test.ts`
  - Content ownership, source/connector identity, and teardown.

### 6.7 N/A boundaries

The test harness itself does not own production caches. It exposes snapshots after barriers. Cache keying, retained-node identity, semantic sequence retention, state revisioning, layout cache behavior, and native resource lifetime remain production responsibilities tested by separate suites.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript package test index

The following files are tracked under `packages/iyon-tui/tests/`.

#### Public/runtime/harness and contract tests

- `tui_harness.test.ts`
  - Native snapshots.
  - Mounted-host key dispatch.
  - Slot animation.
  - Native history rows.
  - Unicode cell positions.
  - Cell styles.
- `tui_runtime.test.ts`
  - Local editing remains native.
  - Submit crosses as routed output.
  - Cancellation and idempotent close.
- `tui_surface_contract.test.ts`
  - Generic `Scene` plus optional native history.
  - Native state behind handles.
  - Explicit callback contracts.
- `tui_demo.test.ts`
  - Public composer/content/focus/scene integration via `tests/fixtures/tui_demo.ts`.
- `tui_handles.test.ts`
  - Synchronous native mutations.
  - Text input native state.
  - Source revisions/sealed-state errors.
  - Shared history/component handles.
- `tui_realtime.test.ts`
  - Real-time host driving and slot tick without manual clock advancement.

#### Semantic/content/text tests

- `tui_semantic_pipeline.test.ts`
  - Origins through projection/rewrite.
  - Source-span validation.
  - Diff rendering.
  - Semantic style lookup.
- `tui_values.test.ts`
  - Immutable semantic values.
  - Nested composition.
  - Semantic diff nodes.
  - Malformed buffer validation.
  - Cache stop-before-payload-read.
  - Unicode and module re-evaluation identity.
  - Worker teardown.
- `tui_text_lanes.test.ts`
  - Fixed/buffer/UTF-8 lane equivalence.
  - Styled spans.
  - NUL, empty, trailing newline, Unicode.
- `tui_ansi_scanner.test.ts`
  - Split ANSI escape sequences.
  - SGR style intent.
  - Unsafe sequence consumption.
  - UTF-8 continuation handling.
- `tui_smooth_delivery.test.ts`
  - Deterministic clock smoothing.
  - Sealing partial delivery.
  - Repeated one-millisecond advances.
- `tui_history_prefix.test.ts`
  - History append and settled prefix stability.
  - Live tail freeze behavior.
- `tui_semantic_cache_ownership.test.ts`
  - Semantic sequence cache behavior and parser reset.

#### Retained state and runtime invalidation

- `tui_state_envelope.test.ts`
  - Geometry and presentation state patches.
  - Clear/reveal-base behavior.
  - Validation and atomicity.
- `tui_retained_scene_regressions.test.ts`
  - Dependency paths.
  - Theme full-paint obligations.
  - Slot replacement state capture.
- `tui_perf13_a.test.ts`
  - Semantic attachment validation and leasing.
  - Duplicate attachment rejection.
  - Automatic wake coalescing.
  - Failed host channel behavior.
  - Presentation receipt polling.
  - Desired versus visible root publication.
- `tui_perf13_b.test.ts`
  - Retained presentation state.
  - State attachment lifecycle.
  - Style-state selector cascade.
  - Cross-host/duplicate/disposed state errors.
  - Atomic patch rejection.
- `tui_perf13_d.test.ts`
  - Content identities.
  - Cross-host/duplicate content attachment.
  - Candidate failure preserving visible connector.
  - Lifecycle error codes and deferred cleanup.
- `tui_perf13_h.test.ts`
  - Direct-FFI accepted revision/wake hint.
  - Multi-host source subscription cleanup.
  - Repeated host/connector ownership cycles.
  - Invalidating host-owned content handles.

#### Native/ABI/transport tests

- `tui_generated_view_abi.test.ts`
  - Generated N-API session.
  - Stable opaque handles.
  - Native refs, spacer creation, text metadata patching.
- `generated/view_abi_layout.test.ts`
  - Pinned schema hash.
  - ABI version.
  - Function ordering.
  - Conformance signatures.
  - POD sizes/alignment.
  - Bun and result-encoding metadata.
- `tui_native_scalar.test.ts`
  - O(1) identity.
  - Generated FFI text layout.
  - Depth-specialized path edits.
  - Root common-field patches.
- `tui_native_persistent_seq.test.ts`
  - Native axis replace/insert/remove.
  - Grid cell path copying.
  - Wide-host parity.
- `tui_native_builder.test.ts`
  - Generated scalar constructors.
  - Native builder axis and text parity.
- `tui_native_transaction.test.ts`
  - Shared changed root over typed text edits.
  - Host parity.
- `tui_native_strings.test.ts`
  - Unicode, styled spans, embedded NUL.
  - Native style atoms.
- `tui_native_input_validation.test.ts`
  - Malformed RGB/color values.
  - Atomic state patches.
- `tui_t14_differential.test.ts`
  - 100 deterministic DAG seeds comparing retained and fresh renders.
- `tui_t14_fuzz_property.test.ts`
  - Malformed payloads and invalid refs do not partially mutate host.
  - Live node cache behavior.
- `tui_h3_a_semantic.test.ts`
  - Semantic node coverage for current View families.
  - Field preservation.
  - Backend-neutral normalization.
  - Caller-value snapshot/freezing.
  - Semantic derivations and persistent sequences.
  - No structural/native dependency in semantic foundation.
- `tui_h3_b_composition.test.ts`
  - Semantic-authoritative View construction.
  - Memoization.
  - Changed-frontier reuse.
  - Component semantic identity.
- `tui_h3_c_transport.test.ts`
  - Semantic retained materialization.
  - Lazy wide semantic sequences.
  - Component handle lowering.
  - NUL-bearing text.
  - Styled text and custom border paths.

#### Trait/adapter/worker tests

- `tui_traits.test.ts`
  - JS-thread renderer, rewriter, and component adapters.
  - Promise ownership.
  - No borrowed native value passed to component callback.
- `tui_worker_lifecycle.ts`
  - Worker-side native ABI session/teardown helper used by `tui_values.test.ts`.

### 7.2 TypeScript fixture tests

#### `consumer.test.ts`

`packages/tui-consumer-fixture/tests/consumer.test.ts` is deliberately described as an external-consumer skeleton.

It tests:

1. Public imports resolve and chrome renders.
2. Identical semantic state re-rendering leaves screen content stable.
3. A single footer text change replaces the old footer.
4. Conditional hint branches appear/disappear/reappear.
5. Repeated `ViewSlot` updates show the latest content only.
6. Repeated `ScrollPane` updates remain visible after `followEnd`.

All tests use `try/finally` and call `session.close()`.

#### `scoped-invalidation.test.ts`

This is the stronger retained-composition acceptance suite. It verifies:

1. Narrow state-write execution frontier.
2. 1,000-sibling isolation.
3. Keyed reorder identity preservation and body skipping.
4. Key-specific changed props.
5. Builder-to-direct slot ownership transition.
6. Stale builder cannot ghost-overwrite a direct value.

The test imports public `defineView`, `Scene`, `state`, `View`, and `AppHarness`. No internal composition or transport module is imported.

### 7.3 Rust test index by subsystem

#### Application/kernel

- `crates/iyon-tui/src/application/tests.rs`
- `crates/iyon-tui/src/application/tests/driver.rs`

Contracts include:

- Runtime error source preservation.
- Theme copy-on-write/deferred mutation.
- Input/output/timer/history composition.
- Focus and component registration.
- Paste interception.
- Application update/render lifecycle.
- Delayed presentation receipts.
- Backend event, viewport, draw, and restore errors.
- Native history sink integration.

#### Component

- `component/mount_tests.rs`
- `component/tests.rs`
- `component/tick_tests.rs`

Contracts include:

- Typed handle identity.
- Registry isolation and stale-handle rejection.
- Revision changes only after successful mutable access.
- Stable component identity.
- Descendant replacement and mount ordering.
- Tick scheduling and failure semantics.

#### Content/text

- `content/text/migrated_tests.rs`
- `content/text/render/tests.rs`
- `content/text/tests/document_public.rs`
- `content/text/tests/markdown_composition.rs`
- `content/text/tests/markdown_hardening.rs`
- `content/text/tests/markdown_incremental.rs`
- `content/text/tests/markdown_smoke.rs`
- `content/text/tests/pulldown_characterization.rs`
- `content/text/tests/text_origin.rs`

Inline test modules additionally exist in text blocks, ANSI, diff, origin, source, style, render, and related modules.

Contracts include:

- Semantic text structure.
- Markdown composition and incremental parsing.
- ANSI handling.
- Origin/provenance.
- Projection/rendering.
- Text validation and error behavior.

#### Text input

- `controls/text_input/tests/buffer.rs`
- `controls/text_input/tests/command.rs`
- `controls/text_input/tests/mod.rs`
- `controls/text_input/tests/output.rs`
- `controls/text_input/tests/presentation.rs`

Contracts include:

- Unicode-safe cursor movement.
- Grapheme/character boundary handling.
- Word deletion and kill/yank operations.
- Command mapping.
- Output routing.
- Presentation cursor/reversal styles.
- Wrapped/multiline layout behavior.

#### Interaction/output

- `interaction/tests/mod.rs`
- `output/tests/event.rs`
- `output/tests/handle.rs`
- `output/tests/mod.rs`
- `output/tests/router.rs`

Contracts include:

- Focus traversal.
- Modal restoration.
- Event routing and consumption.
- Typed output handles.
- Route conflict/type mismatch behavior.
- Queue draining.

#### Physical/layout/paint/terminal

- `physical/tests.rs`
- `presentation/layout/tests/flow.rs`
- `presentation/layout/tests/grid.rs`
- `presentation/layout/tests/mod.rs`
- `presentation/layout/tests/style.rs`
- `presentation/layout/tests/text.rs`

Inline tests also occur in:

- `physical/text_metrics.rs`
- `presentation/wrap.rs`
- `presentation/api/grid.rs`
- `presentation/api/style.rs`
- `presentation/api/text.rs`
- `presentation/paint/decoration.rs`
- `presentation/paint/theme.rs`
- `presentation/paint/view.rs`
- `terminal/termwiz/lower.rs`
- `terminal/termwiz/presenter.rs`
- `terminal/termwiz/shadow.rs`
- `terminal/crossterm/key.rs`

Contracts include:

- Cell/glyph geometry.
- Wide/combining character correctness.
- Layout flow/grid/style/text.
- Theme resolution.
- Paint damage.
- Terminal lowering and shadow buffers.
- Diff/presenter synchronization.
- Cleanup/restoration.

#### Projection/scene/history

- `projection/migrated_tests.rs`
- `projection/tests.rs`
- `projection/tests/p3c_ergonomics.rs`
- `projection/tests/projection_public.rs`
- `scene/root_tests.rs`
- `scene/tests.rs`

Inline tests and test modules also appear in retained state and history support.

Contracts include:

- Projection composition and validation.
- Smoothing and wakeup timing.
- Public projection ergonomics.
- Scene resolution.
- Root replacement.
- Layout/paint invalidation.
- History and native scrollback interaction.

### 7.4 Native crate tests

#### Generated ABI test

`crates/iyon-tui-native/tests/generated_view_abi.rs` is generated from `tools/tui-abi/view_abi.toml`. Its header pins:

- Schema hash.
- Generator hash.

It includes generated tables/types/conformance/exports and defines no-op or marker implementations for generated symbols. This is primarily a compile/link/signature test target for generated ABI shape, not a complete semantic rendering test.

#### Sync test

`crates/iyon-tui-native/tests/sync.rs:3-10` asserts:

- `native_version() == "iyon-tui-native/s6"`
- `tui_smoke() == "iyon-tui/t1"`

This is a tiny cross-crate smoke/version contract.

### 7.5 CI and command observability

Root `package.json` establishes:

- `test`: `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests`
- `rust:test`: `cargo test --workspace`
- `typecheck`: `bunx tsc --noEmit`
- `check:tui-abi`
- `check:tui-declarations`
- `check:tui-binding`
- `check:ownership`

`tsconfig.json:13-16` includes both:

```text
packages/iyon-tui/**/*.ts
packages/tui-consumer-fixture/**/*.ts
```

The CI workflow adds significant validation:

- Rust:
  - `cargo fmt --all -- --check`
  - clippy gate for Rust-impacting changes
  - `cargo test --workspace --all-features`
- TypeScript/native:
  - frozen Bun install
  - ABI generator check
  - `cargo test -p tui-abi-gen`
  - generated-file `git diff --exit-code`
  - TypeScript typecheck/lint
  - declaration closure check
  - native staging
  - binding check
  - ownership check
  - `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests`
- Direct-FFI CI:
  - Native staging with `ION_NATIVE_FEATURES=direct-ffi`
  - authoritative benchmark route.

The test command alone does not stage the native addon. CI explicitly stages it before running Bun tests.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Production/test helper distinction

The repository has a clear but layered distinction:

#### Production

- `Tui`.
- `TuiRuntime`.
- Native host and addon.
- Retained execution runtime.
- Runtime access registration mechanism itself.
- Public controls, content, scene, state, and composition APIs.

#### Test infrastructure

- `AppHarness`.
- `runtimeAccess` consumer.
- Native host reference renderer.
- Rust fake backends and headless sinks.
- Rust test drivers and performance locks.
- Consumer fixture and test counters.

`runtime/access.ts` is production source code in the package, but its only observed consumer is the testing harness. Its existence supports testability without putting snapshot/clock operations on the ordinary public `TuiRuntime` API.

### 8.2 Public TypeScript API versus private implementation imports

The consumer fixture obeys the public boundary:

- `@iyon/tui`
- `@iyon/tui/testing`

The package’s own tests intentionally do not all obey that boundary. Internal imports are used to test:

- Transport framing.
- ABI generated calls.
- Semantic-node normalization.
- Composition execution.
- Native resource ownership.
- Retained DAG internals.

This is appropriate as long as those tests are not mistaken for external-consumer evidence.

### 8.3 Test fixture source itself uses testing API

`packages/tui-consumer-fixture/src/consumer.ts` imports `AppHarness` from `@iyon/tui/testing`. This is deliberate and documented. The fixture simulates a third-party consumer of the framework’s testing entrypoint, not a production application running on a real terminal.

The source still remains public-API-only relative to the package boundaries. It does not import internal source paths.

### 8.4 Rust crate documentation versus historical expectations

`crates/iyon-tui/src/lib.rs` says:

- Rust is an unpublished implementation crate.
- Rust applications do not author Views, controls, or renderers through this crate.
- TypeScript callers use `@iyon/tui`.
- Native input, clocks, History, layout, painting, and retained host behavior remain Rust-owned.

The Rust test suites nevertheless exercise Rust `App`, `Component`, `View`, `History`, and presentation types directly. This is not a contradiction: tests are in-crate and validate implementation machinery, not a supported external Rust authoring contract.

### 8.5 Generated ABI test versus production ABI behavior

The generated Rust ABI test target uses marker/no-op implementations and generated includes. It verifies generated signatures and linkage shape. It is not equivalent to `iyon-tui-native`’s production implementations.

The TypeScript generated ABI tests are more behavior-oriented when they load a staged native addon, but the manifest/layout test remains a schema pinning test.

### 8.6 Differential oracle coupling

`renderRetained` intentionally uses the authoritative retained path as a fresh reference. This is useful for incremental mutation parity but introduces a limitation:

```text
incremental retained route
       compared against
fresh retained route
```

If both share a semantic conversion or native host defect, differential equality can still hold. Public `Tui` integration tests and independent semantic/physical tests are needed to complement it.

### 8.7 Generic framework boundary

The fixture’s `ConsumerState` uses generic display concepts. No production or test code in the assigned fixture scope was found to encode the prohibited Iyon-specific concepts listed in `AGENTS.md`.

The tests use terms such as “consumer,” “status,” “header,” “footer,” and “item,” but these are generic UI fixture terms, not agent/application policy.

### 8.8 Historical contract versus current source

Comments in the consumer fixture refer to PERF-12 T13.1 and R9 requirements. Current source is authoritative for actual behavior:

- The fixture now includes automatic retained composition acceptance tests.
- The tests prove public `defineView`, `state`, `View.key`, and no setup.
- Any historical “skeleton” description should not be read as evidence that only the baseline direct path exists.

---

## 9. Open questions and coverage gaps

1. **No tests were executed in this investigation.**
   - Runtime behavior, native staging, and test pass/fail state remain unverified here.
2. **Exact aggregate LOC was not recomputed.**
   - Small files have exact line extents.
   - Large Rust/TS aggregate estimates are approximate.
3. **Harness exit state synchronization is not fully documented.**
   - `terminalExited` is set only by `AppHarness.exit()`.
   - `exited()` queries the native host independently.
   - It is unclear whether native exit can become true without `AppHarness.exit()` and whether inspection remains safe in that case.
4. **Error normalization is asymmetric.**
   - `pressKey`, `paste`, `advance`, inspection, and `exited` use `callTesting`.
   - `flush`, `close`, and `exit` do not.
   - The intended distinction between normalized runtime errors and raw lifecycle/aggregate errors is not documented in the testing API.
5. **No standalone fixture package script exists.**
   - `packages/tui-consumer-fixture/package.json` contains metadata and a workspace dependency but no local `test` or `typecheck` script.
   - Root scripts and CI include the fixture instead.
6. **No separate public-API compile-only consumer test was found.**
   - TypeScript compilation includes the fixture.
   - Bun acceptance tests import and execute it.
   - There is no distinct “compile fixture against package declarations only” target visible in the scoped manifests.
7. **The external consumer fixture depends on the workspace source package.**
   - This is expected for a monorepo fixture.
   - It does not independently validate a packed/published package artifact.
8. **`tests/ui/pass` and `tests/ui/fail` directories exist but contain no indexed Rust test files.**
   - No active compile-fail/UI test suite was found under those directories in the tracked source manifest.
9. **Generated ABI test provenance is machine-generated.**
   - The generated header pins schema and generator hashes.
   - A full source-to-generated verification depends on the separate ABI generator check.
10. **Some tests observe only snapshots.**
    - Snapshot equality proves rendered output, not necessarily route selection or allocation behavior.
    - Route-sensitive tests should be preferred where the contract is about retained identity, changed frontiers, or publication barriers.
11. **The test suite has several implementation-only paths.**
    - Future API changes must distinguish breakage in external contracts from expected breakage in internal transport/ABI tests.
12. **Rust `test-util` feature scope requires explicit review when changing helpers.**
    - `test-util` is enabled in native dev-dependencies but not shipped artifacts.
    - Its exports and behavior should remain clearly test-only.

---

## 10. Evidence appendix

### 10.1 Contract and context documents read

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

### 10.2 Central test infrastructure files inspected

- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarnessContract`
  - `AppHarness`
  - `createAppHarness`
- `packages/iyon-tui/src/runtime/access.ts`
  - `RuntimeInputEvent`
  - `RuntimeAccess`
  - `registerRuntimeAccess`
  - `runtimeAccess`
- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`
  - `Tui`
  - runtime access registration at lines 197-214
  - canonical/direct render paths
  - close/exit paths
- `packages/iyon-tui/src/index.ts`
  - public root exports
- `packages/iyon-tui/package.json`
  - package exports, test/typecheck scripts
- `package.json`
  - workspace test/typecheck/ABI/binding/ownership scripts
- `tsconfig.json`
  - package and fixture inclusion
- `.github/workflows/ci.yml`
  - Rust, Bun/native, generated ABI, consumer fixture, and direct-FFI validation
- `justfile`
  - Rust test/check commands

### 10.3 TypeScript package fixture files inspected

- `packages/iyon-tui/tests/fixtures/native-host.ts`
  - `renderRetained`
- `packages/iyon-tui/tests/fixtures/tui_demo.ts`
  - `runTuiDemo`
- `packages/iyon-tui/tests/tui_harness.test.ts`
- `packages/iyon-tui/tests/tui_demo.test.ts`
- `packages/iyon-tui/tests/tui_runtime.test.ts`
- `packages/iyon-tui/tests/tui_surface_contract.test.ts`
- `packages/iyon-tui/tests/tui_handles.test.ts`
- `packages/iyon-tui/tests/tui_realtime.test.ts`
- `packages/iyon-tui/tests/tui_values.test.ts`
- `packages/iyon-tui/tests/tui_semantic_pipeline.test.ts`
- `packages/iyon-tui/tests/tui_text_lanes.test.ts`
- `packages/iyon-tui/tests/tui_ansi_scanner.test.ts`
- `packages/iyon-tui/tests/tui_smooth_delivery.test.ts`
- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
- `packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts`
- `packages/iyon-tui/tests/tui_state_envelope.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
- `packages/iyon-tui/tests/tui_perf13_b.test.ts`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `packages/iyon-tui/tests/tui_perf13_h.test.ts`
- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
- `packages/iyon-tui/tests/tui_native_input_validation.test.ts`
- `packages/iyon-tui/tests/tui_native_persistent_seq.test.ts`
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
- `packages/iyon-tui/tests/tui_native_strings.test.ts`
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
- `packages/iyon-tui/tests/tui_generated_view_abi.test.ts`
- `packages/iyon-tui/tests/generated/view_abi_layout.test.ts`
- `packages/iyon-tui/tests/tui_t14_differential.test.ts`
- `packages/iyon-tui/tests/tui_t14_fuzz_property.test.ts`
- `packages/iyon-tui/tests/tui_traits.test.ts`
- `packages/iyon-tui/tests/tui_worker_lifecycle.ts`

### 10.4 External-consumer fixture files inspected

- `packages/tui-consumer-fixture/package.json`
- `packages/tui-consumer-fixture/src/consumer.ts`
  - `ConsumerState`
  - `consumerFooterText`
  - `buildConsumerBody`
  - `buildItemRows`
  - `ConsumerSession`
  - `openConsumerSession`
  - `ScopedEntry`
  - `ScopedConsumer`
  - `buildScopedConsumer`
- `packages/tui-consumer-fixture/tests/consumer.test.ts`
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts`

### 10.5 Native test files inspected

- `crates/iyon-tui-native/Cargo.toml`
- `crates/iyon-tui-native/tests/generated_view_abi.rs`
- `crates/iyon-tui-native/tests/sync.rs`

### 10.6 Rust test-support files inspected or indexed

- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/application/tests.rs`
- `crates/iyon-tui/src/application/tests/driver.rs`
- `crates/iyon-tui/src/application/tests/`
- `crates/iyon-tui/src/component/mount_tests.rs`
- `crates/iyon-tui/src/component/tests.rs`
- `crates/iyon-tui/src/component/tick_tests.rs`
- `crates/iyon-tui/src/content/text/migrated_tests.rs`
- `crates/iyon-tui/src/content/text/render/tests.rs`
- `crates/iyon-tui/src/content/text/tests/document_public.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_composition.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_hardening.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_incremental.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_smoke.rs`
- `crates/iyon-tui/src/content/text/tests/pulldown_characterization.rs`
- `crates/iyon-tui/src/content/text/tests/text_origin.rs`
- `crates/iyon-tui/src/controls/text_input/tests/buffer.rs`
- `crates/iyon-tui/src/controls/text_input/tests/command.rs`
- `crates/iyon-tui/src/controls/text_input/tests/mod.rs`
- `crates/iyon-tui/src/controls/text_input/tests/output.rs`
- `crates/iyon-tui/src/controls/text_input/tests/presentation.rs`
- `crates/iyon-tui/src/interaction/tests/mod.rs`
- `crates/iyon-tui/src/output/tests/event.rs`
- `crates/iyon-tui/src/output/tests/handle.rs`
- `crates/iyon-tui/src/output/tests/mod.rs`
- `crates/iyon-tui/src/output/tests/router.rs`
- `crates/iyon-tui/src/physical/tests.rs`
- `crates/iyon-tui/src/presentation/layout/tests/flow.rs`
- `crates/iyon-tui/src/presentation/layout/tests/grid.rs`
- `crates/iyon-tui/src/presentation/layout/tests/mod.rs`
- `crates/iyon-tui/src/presentation/layout/tests/style.rs`
- `crates/iyon-tui/src/presentation/layout/tests/text.rs`
- `crates/iyon-tui/src/projection/migrated_tests.rs`
- `crates/iyon-tui/src/projection/tests.rs`
- `crates/iyon-tui/src/projection/tests/p3c_ergonomics.rs`
- `crates/iyon-tui/src/projection/tests/projection_public.rs`
- `crates/iyon-tui/src/scene/root_tests.rs`
- `crates/iyon-tui/src/scene/tests.rs`

Inline test-bearing production modules indexed include:

- `application/content.rs`
- `application/environment.rs`
- `application/handle.rs`
- `application/host.rs`
- `application/kernel.rs`
- `application/mod.rs`
- `application/source_store.rs`
- `component/mount.rs`
- `component/mod.rs`
- `component/registry.rs`
- `component/revision.rs`
- `component/tick.rs`
- `content/diff/model.rs`
- `content/diff/render.rs`
- `content/text/ansi.rs`
- `content/text/block.rs`
- `content/text/diff.rs`
- `content/text/mod.rs`
- `content/text/origin.rs`
- `content/text/render/mod.rs`
- `content/text/render/source_format.rs`
- `content/text/source.rs`
- `content/text/style.rs`
- `controls/text_input/buffer.rs`
- `controls/text_input/mod.rs`
- `history/mod.rs`
- `history/projection/mod.rs`
- `history/trace.rs`
- `interaction/mod.rs`
- `output/event.rs`
- `output/mod.rs`
- `physical/mod.rs`
- `physical/text_metrics.rs`
- `presentation/api/grid.rs`
- `presentation/api/style.rs`
- `presentation/api/text.rs`
- `presentation/layout/cache.rs`
- `presentation/layout/grid.rs`
- `presentation/layout/measure.rs`
- `presentation/layout/mod.rs`
- `presentation/layout/place.rs`
- `presentation/layout/prepare.rs`
- `presentation/paint/decoration.rs`
- `presentation/paint/mod.rs`
- `presentation/paint/theme.rs`
- `presentation/paint/view.rs`
- `presentation/wrap.rs`
- `projection/mod.rs`
- `retained_state/capabilities.rs`
- `retained_state/capture.rs`
- `retained_state/damage.rs`
- `retained_state/presentation.rs`
- `retained_state/record.rs`
- `retained_state/registry.rs`
- `scene/host.rs`
- `scene/mod.rs`
- `scene/resolve.rs`
- `terminal/crossterm/key.rs`
- `terminal/termwiz/lower.rs`
- `terminal/termwiz/mod.rs`
- `terminal/termwiz/presenter.rs`
- `terminal/termwiz/shadow.rs`

### 10.7 Files indexed but not deeply read

The following were identified through the tracked source manifest and repository search but were not all read line-by-line in this assignment:

- Every large Rust production module containing inline tests.
- Every Rust standalone test file listed above.
- All generated ABI Rust source included by `generated_view_abi.rs`.
- All TypeScript package test bodies not directly needed to establish test infrastructure contracts.
- Benchmark data and replay traces outside the assigned tests/fixtures scope.
- Other atlas scout reports and pending parent-authored integrated documents.

### 10.8 Static commands/searches used

Read-only repository searches were used to:

- Enumerate scoped files.
- Search test declarations and `#[cfg(test)]` modules.
- Locate runtime access registration.
- Locate package scripts and CI test commands.
- Locate fixture imports and internal bypasses.
- Inspect the tracked-source manifest for exhaustive path indexing.

No test, build, lint, dependency installation, or service mutation was performed.