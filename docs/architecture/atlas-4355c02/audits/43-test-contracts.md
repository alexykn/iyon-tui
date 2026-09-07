# 43 — Test Contracts and Source Reachability

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: cross-repository tests and source reachability, specifically:
  - production contracts versus obsolete helpers and test oracles;
  - direct test bypasses around the intended TS → native → Rust route;
  - tests that can pass while the intended production route is broken;
  - silent fallback and compatibility behavior relevant to test confidence.

The repository-level contract identifies assignment 43 as `audits/test-contracts`, with the goal “Production contracts vs obsolete helpers/oracles, bypasses, tests that can succeed when intended route is broken” (`docs/architecture/atlas-4355c02/evidence/assignments.json:297-301`).

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`;
- `docs/architecture/atlas-4355c02/README.md`;
- `PRE-V5-ARCHITECTURE-REPORT.md`, including its architecture-drift addition, especially:
  - semantic-operation → implementation-path matrices;
  - silent fallback analysis;
  - authoritative-path verification;
  - tests that accidentally bless obsolete paths;
  - production reachability;
  - initial-construction versus update consistency;
  - failure semantics and route observability.

The report contract makes current source authoritative for what executes, while approved historical handoffs remain the default oracle for intended contracts. Historical claims are therefore labeled as historical and are not treated as current source evidence.

### Investigation method

This was a read-only static investigation. I:

- enumerated current Rust, native, TypeScript, fixture, and test files;
- followed imports and reverse references for native ABI helpers, retained materializers, path metadata, builders, transactions, and test fixtures;
- inspected package scripts and CI ordering;
- inspected the public package and external-consumer fixture boundaries;
- inspected the relevant production call chains in `runtime.ts`, `retained-dag.ts`, `native-view-abi.ts`, `view-slot.ts`, and `view.ts`;
- inspected test skip conditions, direct internal imports, output-only assertions, direct generated ABI calls, and fake generated-wrapper implementations.

I did **not** run Rust or Bun tests, benchmarks, staging, builds, or mutation experiments. Any execution statements below are static conclusions from source, not observed test results.

### High-confidence conclusions

1. The current production root route is intentionally retained:
   - `Tui.render()` dispatches to canonical or direct rendering (`packages/iyon-tui/src/runtime/runtime.ts:399-405`);
   - canonical rendering uses `OwnedBuilderRoot` and `RetainedRootBoundary` (`runtime.ts:451-496`);
   - root preparation invokes `ensureSemanticNative` (`retained-dag.ts:1801-1835`);
   - production retained publication is then committed through `setDesiredViewRef` and the host frame barrier (`retained-dag.ts:1919-1942`).

2. There is no evidence in the inspected current TS structural/runtime path of a complete-object or legacy cold renderer being selected as a fallback after retained refusal. The retained path explicitly fails when preparation refuses (`runtime.ts:218-222`, `retained-dag.ts:1644-1648`, `1670-1677`).

3. A substantial set of old path-oriented helpers remains in source and is directly exercised by tests and benchmarks even though it has no ordinary production caller:
   - `packages/iyon-tui/src/transport/structural/retained-path.ts`;
   - `tryRetainedAxisCreateRender`;
   - `tryRetainedAxisSetChildRender`;
   - `tryRetainedAxisSpliceRender`;
   - `tryRetainedGridSetCellRender`;
   - `tryRetainedEditTransactionRender`;
   - `textLayoutAtNativePathForTransport`;
   - `textLayoutTransactionForTransport`.

4. The shared `renderRetained` test helper is not an independent semantic oracle. It calls the same retained semantic materializer used by production:
   - helper: `packages/iyon-tui/tests/fixtures/native-host.ts:16-28`;
   - helper materialization: `tryRetainedMaterializeRef` → `ensureSemanticNative` (`packages/iyon-tui/src/transport/structural/native-view-abi.ts:159-186`);
   - production materialization: `RetainedRootBoundary.prepareFrom` → `ensureSemanticNative` (`packages/iyon-tui/src/transport/structural/retained-dag.ts:1825-1835`).

5. Several TypeScript native tests return successfully without making any assertion when the native host/session is unavailable. Those tests can therefore pass vacuously in environments where the native route is absent.

6. The native Rust integration test `generated_view_abi.rs` tests generated wrapper dispatch against test-local stub implementations, not the actual `crates/iyon-tui-native/src/tui/view_abi.rs` implementation. It is useful ABI-generator evidence, but it cannot establish that the shipped native implementation behaves correctly.

---

## 1. Responsibility and structure

### 1.1 Current test and support inventory

The current TypeScript framework package contains 33 `*.test.ts` files under `packages/iyon-tui/tests`, plus helper files:

- `packages/iyon-tui/tests/fixtures/native-host.ts`;
- `packages/iyon-tui/tests/fixtures/tui_demo.ts`;
- `packages/iyon-tui/tests/tui_worker_lifecycle.ts`.

The external-consumer package contains:

- `packages/tui-consumer-fixture/src/consumer.ts`;
- `packages/tui-consumer-fixture/tests/consumer.test.ts`;
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts`.

The current workspace test command is:

```json
"test": "bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests"
```

(`package.json:30-36`).

The framework package-local test command is:

```json
"test": "bun test tests"
```

(`packages/iyon-tui/package.json:15-21`).

CI stages the native addon before running the TypeScript tests (`.github/workflows/ci.yml:79-82`; corresponding staging/test steps also occur in the TypeScript-specific workflows). Therefore, the native-availability skips described below are not expected to trigger in the normal CI sequence if staging succeeds. They remain dangerous for local runs, alternate test invocations, missing artifacts, platform failures, and future CI changes.

### 1.2 Approximate physical scale

The following are rounded physical-scale estimates based on the current file manifest and source line ranges, not executed `wc` counts:

| Area | Approximate physical scale | Counting method and qualification |
|---|---:|---|
| TypeScript framework tests | ~4.5k–5.5k LOC | Current `packages/iyon-tui/tests/**/*.test.ts`; rounded from inspected file line ranges |
| External-consumer tests and source | ~450–550 LOC | `consumer.ts`, `consumer.test.ts`, `scoped-invalidation.test.ts` |
| Native ABI Rust integration test | ~1.6k LOC | `crates/iyon-tui-native/tests/generated_view_abi.rs`; generated wrapper stubs plus assertions |
| Native handwritten unit tests | 2 tests | `crates/iyon-tui-native/src/tui.rs:1963-2000+` |
| Core Rust unit/in-module tests | many thousands of LOC | `#[cfg(test)]` modules across `crates/iyon-tui/src`; indexed, not comprehensively re-read for this cross-repository assignment |
| Production TS transport/runtime | tens of thousands of LOC | `packages/iyon-tui/src`; indexed and key route owners read |
| Production Rust/native implementation | tens of thousands of LOC including generated ABI support | indexed from the tracked-source manifest; not LOC-counted as assignment-owned production scope |

The most consequential test-code mass for this assignment is not the total number of tests but the split between:

- public-route tests using `AppHarness` and `Tui.render`;
- direct internal transport tests using generated ABI wrappers and path helpers;
- tests that compare two paths sharing the same retained materializer;
- tests that only pin schema/wrapper shape.

### 1.3 Primary responsibilities by test layer

| Layer | Primary responsibility | Architectural evidence strength |
|---|---|---|
| `AppHarness` tests | Public-ish TypeScript facade, headless rendering, interaction, content, state, history, scheduling | Good behavioral evidence of the TS facade and staged native addon; generally weak route identity evidence unless counters are asserted |
| External-consumer fixture | Public API-only authoring and automatic retained composition | Strongest consumer-boundary evidence; still mostly output/frontier assertions rather than native route assertions |
| `renderRetained` fixture | Direct materialize-and-host-render reference setup | Useful for host parity and low-level retained ABI experiments; not independent semantic materialization |
| Direct generated ABI tests | Wire signatures, generated wrapper argument validation, native handle calls | Good generated-wrapper evidence; not sufficient for actual native implementation behavior |
| Direct retained-path tests | Old path, transaction, builder, and persistent-sequence helpers | Evidence that legacy/internal helpers still execute; not evidence of the canonical `Tui.render` route |
| Rust core unit tests | Rust-side application, component, layout, history, stream, and control behavior | Valid unit evidence for Rust internals; independent of TS/N-API reachability |
| Native Rust generated integration test | Generator output, function order, sentinel dispatch, pointer/buffer validation | Does not exercise actual native view implementation |
| Staging script checks | Native addon surface and symbol-level packaging assertions | Strong static/package boundary check; not a runtime rendering contract |

---

## 2. Types, APIs and contracts

### 2.1 Production route contract

`TuiRuntime` documents the intended public distinction:

- `render(scene)` accepts a structural scene value;
- `render(builder)` accepts a retained scene producer;
- direct values take over the root immediately;
- producers own the retained root and subscribe to tracked state.

This contract is stated in `packages/iyon-tui/src/runtime/runtime.ts:59-90`, especially `runtime.ts:64-68`.

The canonical production route is:

```text
Tui.render(builder)
  → renderCanonical()
  → OwnedBuilderRoot.start / replaceProducer
  → RetainedRootBoundary.prepareDesiredInstall()
  → prepareFrom()
  → ensureSemanticNative()
  → generated retained ABI constructors/patches
  → setDesiredViewRef()
  → environment frame drain
  → visible host commit
```

The direct route is:

```text
Tui.render(scene)
  → renderDirect()
  → prepareRootPublication()
  → RetainedRootBoundary.prepareDesiredInstall()
  → ensureSemanticNative()
  → setDesiredViewRef()
  → environment frame drain
```

The production implementation explicitly says the retained path is the single production structural architecture and that retained refusal fails explicitly rather than selecting another transport (`packages/iyon-tui/src/transport/structural/retained-dag.ts:1-23`).

### 2.2 Public facade versus internal transport surface

The root package exports public semantic APIs from `packages/iyon-tui/src/index.ts:96-132`, including:

- `View`;
- `defineView`;
- `state`;
- `Scene`;
- `Tui`;
- `History`;
- `TextInput`;
- `ViewState`;
- `Projection`;
- `Smooth`;
- content and style primitives.

The root does not export the generated structural ABI wrappers or `retained-path.ts` functions.

However, deep imports remain available to in-repository tests and benchmarks. The following are exported from internal modules and directly imported by tests:

- `axisSetChildForTransport`, `axisSpliceForTransport`, `gridSetCellForTransport` (`packages/iyon-tui/src/api/view/view.ts:662-718`);
- `textLayoutAtNativePathForTransport`, `textLayoutTransactionForTransport` (`packages/iyon-tui/src/transport/structural/retained-path.ts:63-106`);
- generated ABI calls in `transport/abi/structural/generated/view_calls.ts`;
- retained helper functions in `transport/structural/native-view-abi.ts`.

This distinction matters: “not root-exported” is not the same as “unreachable.” The test and benchmark packages can reach these APIs directly.

### 2.3 View-slot and scroll-pane internal methods

The public `ViewSlot` interface contains:

- `setView`;
- `setAnimation`;
- `setAnimationAtCycleBoundary`;
- `stopAnimation`;
- `revision`.

(`packages/iyon-tui/src/api/controls/view-slot.ts:50-60`.)

The concrete `ViewSlot` class also exposes implementation-seam methods such as:

- `tuiViewAbiInstallRef` (`view-slot.ts:148`);
- `prepareSetView` (`view-slot.ts:244+`).

The public interface does not list those methods, but the concrete class is exported from its module and the class/type surface has historically been identified as an API-hygiene concern. The historical API-H1 document records this exact concern, including:

- `ViewSlot.prepareSetView()`;
- `ViewSlot.tuiViewAbiInstallRef()`;
- `NativeScrollPane.tuiViewAbiInstallRef()`;
- old `View` transport constructors.

(`docs/history/API-H1/API-H1-V2-public-api-hygiene.md:1949`.)

The current `tools/ownership/check.ts` contains checks for removed transport statics and internal ownership seams (`check.ts:1388-1402`), but the ordinary behavioral test suite does not contain a negative compile/API test that proves these implementation methods are inaccessible to a consumer.

### 2.4 Native host contract

The current TypeScript native host contract intentionally does **not** expose the old high-level host methods:

- no `NativeTuiHost.render`;
- no `NativeTuiHost.createViewSlot`;
- no `NativeTuiHost.scrollPane`.

Instead it exposes:

- `setDesiredViewRef`;
- `createViewSlotRef`;
- `scrollPaneRef`;
- `epochs`;
- `flushPendingHosts`;
- output/input and inspection methods.

(`packages/iyon-tui/src/transport/native/addon.ts:134-189`.)

The staging script explicitly rejects removed methods/classes:

```text
NativeHistory.push
NativeHistory.freeze
NativeHistory.pushStream
NativeHistory.sealStream
NativeTuiHost.render
NativeTuiHost.createViewSlot
NativeTuiHost.scrollPane
NativeViewSlot.setView
NativeViewSlot.setAnimation
NativeViewSlot.setAnimationAtCycleBoundary
NativeViewSlot.stopAnimation
NativeScrollPane.setContent
```

(`packages/iyon-tui/scripts/stage-native.ts:53-68`.)

This is strong evidence that the old high-level native host API was intentionally removed from the shipped addon. It does not, however, remove the lower-level path/transaction ABI exports or the TypeScript test helpers that still use them.

---

## 3. Dependency and ownership map

### 3.1 Current production ownership graph

```text
Public TS authoring
  │
  ├── @iyon/tui root exports
  │      View / Scene / Tui / state / defineView / content / controls
  │
  └── @iyon/tui/testing
         AppHarness
           │
           ▼
      Tui.render()
           │
           ├── canonical builder route
           │      OwnedBuilderRoot
           │        RetainedExecutionRuntime
           │          RetainedRootBoundary
           │
           └── direct scene route
                  RetainedRootBoundary
                    │
                    ▼
                prepareFrom()
                    │
                    ▼
              ensureSemanticNative()
                    │
                    ├── semantic identity hint
                    ├── transaction-local ref
                    ├── NodeId → NativeRef promotion
                    ├── semantic derivation patch
                    └── direct per-kind materializer
                           │
                           ▼
                  generated N-API ABI calls
                           │
                           ▼
                 NativeViewRuntime / NativeTuiHost
                           │
                           ├── desired root
                           ├── frame drain
                           ├── visible root
                           └── terminal cells/output
```

Evidence:

- `Tui` owns the host, retained runtime, environment, root boundary, and handles (`runtime.ts:108-141`);
- canonical construction starts `OwnedBuilderRoot` (`runtime.ts:451-485`);
- root preparation goes through `RetainedRootBoundary` (`runtime.ts:465-466`, `runtime.ts:535-560`);
- `prepareFrom` invokes `ensureSemanticNative` (`retained-dag.ts:1801-1835`);
- desired publication uses `setDesiredViewRef` (`retained-dag.ts:1919-1942`).

### 3.2 Test/helper dependency graph

```text
Direct low-level tests
  │
  ├── generated/view_calls.ts
  │       └── NativeViewAbiHandle methods
  │
  ├── native-view-abi.ts
  │       ├── tryRetainedMaterializeRef
  │       ├── tryRetainedAxisCreateRender
  │       ├── tryRetainedAxisSetChildRender
  │       ├── tryRetainedAxisSpliceRender
  │       ├── tryRetainedGridSetCellRender
  │       └── tryRetainedEditTransactionRender
  │
  ├── retained-path.ts
  │       ├── path lineage metadata
  │       ├── path patch construction
  │       └── transaction metadata
  │
  └── fixtures/native-host.ts
          └── materialize + hostRenderRef

Public-route tests
  │
  ├── AppHarness
  │      └── Tui.render / native runtime
  │
  └── tui-consumer-fixture
         └── @iyon/tui root + @iyon/tui/testing only
```

### 3.3 Shared ownership of the “reference” route

`renderRetained` is described as a “full-publication helper” and “same-architecture reference” (`packages/iyon-tui/tests/fixtures/native-host.ts:10-14`). Its actual implementation:

1. obtains a session;
2. calls `tryRetainedMaterializeRef(view)`;
3. calls generated `hostRenderRef`;
4. releases the temporary ref.

(`native-host.ts:16-28`.)

This means the helper does not independently decode or independently interpret semantic `View` values. Its semantic path is:

```text
renderRetained
  → tryRetainedMaterializeRef
  → MaterializeTx
  → ensureSemanticNative
  → retained-dag materializer
```

Production root preparation uses the same `ensureSemanticNative` and the same materializer map. Therefore:

- semantic corruption shared by `ensureSemanticNative` and its materializers can appear correct in parity tests;
- generated host rendering and production environment-frame rendering are not identical, so the helper still exercises a distinct host publication seam;
- the helper is useful as a host/ABI parity fixture, but should not be treated as an independent semantic oracle.

---

## 4. Execution paths and state transitions

### 4.1 Canonical production root path

`Tui.render()` chooses a route based on whether the argument is a function:

```text
function argument
  → renderCanonical
non-function scene
  → renderDirect
```

(`packages/iyon-tui/src/runtime/runtime.ts:399-405`.)

Canonical route:

1. Validate signal/open state.
2. Drain pending retained execution.
3. Build a producer that constructs a `Scene` and stages history.
4. Start or replace `OwnedBuilderRoot`.
5. The root target prepares a retained publication.
6. Commit desired structure.
7. Flush the environment host registration.
8. A later environment drain promotes desired structure to visible frame.

(`runtime.ts:451-496`.)

Direct route:

1. Validate signal/open state.
2. Normalize scene.
3. Validate semantic body and history.
4. Detect exact same scene identity and no-op where possible.
5. Otherwise prepare a retained root publication.
6. Commit it.
7. Dispose any retained builder root.
8. Flush visible frame work.

(`runtime.ts:499-548`.)

### 4.2 Retained preparation path

`RetainedRootBoundary.prepareFrom` is the central preparation route:

```text
prepareFrom(view)
  → semanticNodeOf(view)
  → MaterializeTx
  → ensureSemanticNative(node, tx)
  → stale/current-root checks
  → optional NodeId promotion
  → prepared root lease
  → RootPublication
```

(`packages/iyon-tui/src/transport/structural/retained-dag.ts:1796-1916`.)

The important failure contract is:

- a retained refusal or cycle returns no prepared publication;
- temporary leases are released;
- the old root remains installed;
- the caller surfaces an explicit failure;
- no cold or complete-object renderer is selected.

(`retained-dag.ts:1827-1835`, `1644-1648`, `1670-1677`.)

### 4.3 Retained derivation route

For semantic nodes carrying derivation metadata, `ensureSemanticNative` tries derivations before payload materialization:

```text
ensureSemanticNative
  → identity hint/local ref/NodeId promotion
  → tryDerivation
      ├── textLayout → viewTextLayoutPatchRoot
      ├── commonScalar → viewCommonPatchRoot
      ├── axisSet → viewAxisSetChild
      ├── axisSplice → viewAxisSpliceBuffer
      └── gridCell → viewGridSetCell
  → direct materializer if derivation unavailable/refused
```

(`retained-dag.ts:1148-1216`, `1252-1365`.)

The fallback from failed derivation to direct materialization is **not** an old architecture fallback. It remains inside the same retained transaction and uses semantic fields plus the same generated retained ABI. The source explicitly describes this as direct semantic materialization (`retained-dag.ts:1259-1262`).

### 4.4 Stale-reference recovery

There are two related same-architecture recovery paths:

- stale child/base recovery through `recoverStaleNode` (`retained-dag.ts:415-432`);
- exact-root host-render recovery in `renderExactRoot` (`retained-dag.ts:1380-1462`).

Both:

- invalidate the stale hint;
- retry through the retained materializer or NodeId promotion;
- consume a bounded retry budget;
- fail explicitly if retry fails.

This is not a legacy fallback, but tests should distinguish it from ordinary cache misses and direct materialization. Current counters include:

- `stale_ref_retries`;
- `node_id_ref_promotion_attempts`;
- `node_id_ref_promotion_hits`;
- `derivation_fast_path_calls`;
- `direct_materializer_calls`;
- `host_mutations`.

(`retained-dag.ts:111-148`.)

### 4.5 Slot and scroll-pane routes

Production controls use `RetainedRootBoundary` rather than the old direct render helper:

- initial slot content uses `tryRetainedMaterializeRef` to seed a native slot reference (`packages/iyon-tui/src/api/controls/view-slot.ts:24-45`);
- later direct slot replacement uses `boundary.prepareInstall(view)` (`view-slot.ts:204-225`);
- slot builder mode uses `OwnedBuilderRoot` (`view-slot.ts:157-202`);
- animations obtain retained refs for frame lists and install native animation references (`view-slot.ts:290-334`);
- `view()` composes through `composeComponent`, not through the old transport helper (`view-slot.ts:150-153`).

The initial slot/animation use of `tryRetainedMaterializeRef` is a legitimate separate boundary use: the native slot owns a strong copy while the temporary materialization lease is released. It is not evidence of a second semantic renderer.

By contrast, the `tryRetained*Render` functions in `native-view-abi.ts` are not called by these controls.

### 4.6 Rust-side alternate route

The core Rust crate still contains a complete Rust-side application/component/view route, including:

- `App`;
- `AppCx`;
- `Component`;
- `ComponentHandle`;
- `View`;
- `Scene`;
- `prepare_frame`;
- fake terminal backends.

For example, `crates/iyon-tui/src/application/tests.rs:103-111` constructs Rust `View` trees directly, and `application/tests.rs:331-350` prepares frames through the Rust application kernel.

This route is valid Rust-internal unit coverage, but it is not evidence that the TypeScript facade, N-API binding, native addon, or shipped canonical TS route is reachable. The crate manifest explicitly says the core crate is runtime implementation for the in-tree native binding rather than a supported external Rust authoring package (`crates/iyon-tui/Cargo.toml:6-18`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation → route matrix

| Semantic operation | Current intended production path | Other reachable path(s) | Selection condition | Fallback/masking risk | Evidence |
|---|---|---|---|---|---|
| Root scene publication | `Tui.render` → `RetainedRootBoundary.prepareDesiredInstall` → desired root → frame drain | Direct internal helper `renderRetained`; Rust `App.prepare_frame` | Test-only helper or Rust unit test | Helper bypasses root boundary; Rust route bypasses TS/native | `runtime.ts:399-548`; `native-host.ts:16-28`; `application/tests.rs:331-350` |
| Initial semantic materialization | `ensureSemanticNative` and `MATERIALIZERS` | `tryRetainedMaterializeRef`, which calls the same materializer | Initial slot/history/animation boundary or test fixture | No independent oracle; shared materializer failures can be shared | `retained-dag.ts:1153-1216`; `native-view-abi.ts:159-186` |
| Exact-root visible render | Environment frame drain / host desired-root protocol | `renderExactRoot`; direct `hostRenderRef` in tests | Warm root optimization or test helper | Output-only parity can miss semantic errors shared earlier | `retained-dag.ts:1380-1462`; `native-host.ts:23-24` |
| Scalar text/layout derivation | `tryDerivation` → `viewTextLayoutPatchRoot` | `textLayoutAtNativePathForTransport` + old path ABI | Production semantic derivation versus test/benchmark explicit path metadata | Old path tests can pass independently of canonical root routing | `retained-dag.ts:1264-1281`; `retained-path.ts:63-106` |
| Axis replacement/splice | `tryDerivation` → `viewAxisSetChild`/`viewAxisSpliceBuffer` | `tryRetainedAxisSetChildRender`, `tryRetainedAxisSpliceRender`; `axisSetChildForTransport` test/bench construction | Explicit internal helper or semantic derivation | Direct helper bypasses root boundary and uses host install path directly | `retained-dag.ts:1301-1334`; `native-view-abi.ts:317-400` |
| Grid cell replacement | `tryDerivation` → `viewGridSetCell` | `tryRetainedGridSetCellRender`; `gridSetCellForTransport` test/bench construction | Explicit internal helper or semantic derivation | Same direct-helper bypass | `retained-dag.ts:1335-1347`; `native-view-abi.ts:403-436` |
| Multiple text edits | No current ordinary production caller found for old edit transaction helper | `tryRetainedEditTransactionRender` | Direct test invocation | Tests exercise old transaction route, not `Tui.render` | `native-view-abi.ts:443-510`; `tui_native_transaction.test.ts:23-65` |
| Wide-axis construction | Current direct materializer uses generated fixed constructors or buffer/builder constructors | `tryRetainedAxisCreateRender`; direct builder test | Test-only direct native route | Tests compare direct helper to same materializer/reference | `retained-dag.ts:515-567`; `native-view-abi.ts:194-250`; `tui_native_builder.test.ts:19-50` |
| Content append/replace | Content port/source/connector → native content lane → host content node | No complete alternate renderer found | Content API operations | Public tests assert visible text but generally do not assert route counters | `tui_semantic_cache_ownership.test.ts:13-97`; content source/port APIs |
| State mutation | Public `ViewState` / state attachment → native state lane → retained invalidation/frame | Direct raw native resource mutation in malformed-input subprocess | Boundary validation tests | Raw native tests bypass public validation by design | `tui_native_input_validation.test.ts:33-125` |
| External composition | Public `defineView`/`state`/`View.key` → `Tui.render` | None in consumer fixture source | Consumer imports only root/testing entrypoints | Native route itself is not directly identified by consumer tests | `packages/tui-consumer-fixture/src/consumer.ts:1-17`, `126-169` |
| Generated ABI wrapper dispatch | Generated TS/Rust wrappers | Test-local stub implementations | ABI integration test | Actual native implementation may be broken while wrappers pass | `crates/iyon-tui-native/tests/generated_view_abi.rs:19-58`, `1411-1600` |

### 5.2 Obsolete or test-only helper reachability

#### `retained-path.ts`

The source search found no production import of:

- `textLayoutAtNativePathForTransport`;
- `textLayoutTransactionForTransport`;
- `nativePathLineage`;
- `nativePathChildLineage`;
- `attachNativePathLineage`;
- `nativeTextLayoutTransaction`.

Consumers found were:

- `packages/iyon-tui/tests/tui_native_scalar.test.ts:45-78`;
- `packages/iyon-tui/tests/tui_native_transaction.test.ts:44-61`;
- `packages/iyon-tui/bench/perf12_t15_workload.ts:8-12`, `111-127`;
- historical documentation.

The file itself calls its constructors `@internal` and describes them as native retained path/transaction metadata (`retained-path.ts:63-106`). It is therefore test/benchmark reachable but not ordinary production reachable.

#### `tryRetained*Render` helpers

The following functions are defined in `native-view-abi.ts` but have no production source consumers outside their own helper relationships:

- `tryRetainedAxisCreateRender` (`native-view-abi.ts:228-250`);
- `tryRetainedAxisSetChildRender` (`native-view-abi.ts:317-350`);
- `tryRetainedAxisSpliceRender` (`native-view-abi.ts:357-400`);
- `tryRetainedGridSetCellRender` (`native-view-abi.ts:403-436`);
- `tryRetainedEditTransactionRender` (`native-view-abi.ts:443-510`).

They are directly exercised by:

- `tui_native_builder.test.ts`;
- `tui_native_persistent_seq.test.ts`;
- `tui_native_transaction.test.ts`.

Their design is an older direct “construct and install” route:

```text
resolve/acquire child refs
  → generated structural patch/builder
  → installNativeRef(host, ...)
  → return native ref
```

That is structurally different from the current root publication route:

```text
semantic View
  → RetainedRootBoundary.prepareInstall / prepareDesiredInstall
  → commit publication
  → desired/visible root protocol
```

These helpers are therefore high-priority migration residue candidates for test/benchmark classification. They may remain as deliberate low-level ABI contracts only if the ABI is still intentionally supported; current source does not show an ordinary production caller.

#### `tryRetainedMaterializeRef`

This helper is different. It has production consumers in:

- `api/controls/view-slot.ts`;
- `api/controls/scroll-pane.ts`;
- `api/controls/history.ts`.

It obtains a temporary retained lease and explicitly fails on refusal (`native-view-abi.ts:159-186`). It is a legitimate retained boundary adapter, not an obsolete renderer.

#### Internal semantic derivation constructors

`axisSetChildForTransport`, `axisSpliceForTransport`, and `gridSetCellForTransport` are internal semantic constructors in `api/view/view.ts:662-718`. They are used by tests and the T15 benchmark, but no ordinary production source call was found.

They are not equivalent to the old native direct-render helpers: they construct semantic values carrying derivation metadata, and `retained-dag.ts` consumes those derivations in the current production route. Their current status is:

- semantically connected to the retained production path;
- not root-exported;
- directly reachable by deep imports from tests/benchmarks;
- potentially unnecessary as an external authoring seam, but not provably removable without replacing their semantic derivation construction role.

### 5.3 Generated ABI path residue

The generated ABI still contains path and transaction families:

- `view_axis_set_child_path`;
- `view_grid_set_cell_path`;
- `view_text_layout_patch_path`;
- `view_text_layout_patch_path_d1`;
- `view_text_layout_patch_path_d2`;
- `view_text_layout_patch_path_d3`;
- `view_text_layout_patch_path_d4`;
- `edit_txn_begin`;
- `edit_txn_add_text_layout`;
- `edit_txn_commit_render`;
- `edit_txn_abort`.

The manifest pins these functions (`packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json:1720-1842`, `2396-3272`).

The source search found:

- generated TypeScript declarations/wrappers;
- generated native N-API methods;
- native metadata table entries;
- old `tryRetainedEditTransactionRender` use of `edit_txn_*`;
- no current TypeScript call sites for the path patch functions themselves.

In particular, `viewAxisSetChildPath`, `viewGridSetCellPath`, and `viewTextLayoutPatchPath*` appear to be generated/exported ABI residue with no current TS caller. The generated manifest test proves that they remain named and ordered; it does not prove that production uses them.

### 5.4 Silent fallback findings

No complete-object/cold rendering fallback was found in the inspected current TypeScript production route.

The source contains several `undefined`-returning helpers, but their meanings differ:

1. **Legitimate retained refusal**
   - `prepareFrom` returns `undefined`;
   - caller retains the old root and throws/fails explicitly.
   - Evidence: `retained-dag.ts:1644-1648`, `1670-1677`, `1827-1835`.

2. **Same-architecture derivation fallback**
   - derivation failure falls back to direct semantic materialization through `MATERIALIZERS`.
   - Evidence: `retained-dag.ts:1201-1210`, `1259-1262`.
   - This does not select a complete-object or compatibility renderer.

3. **Same-architecture stale-reference recovery**
   - stale hints are invalidated and retried once.
   - Evidence: `retained-dag.ts:415-432`, `1350-1365`, `1387-1459`.

4. **Boundary adapter refusal**
   - `tryRetainedMaterializeRef` returns `undefined` for retained refusal/cycle;
   - slot/history callers turn that into explicit initialization/update errors.
   - Evidence: `native-view-abi.ts:159-186`; `view-slot.ts:37-45`, `210-225`.

The important remaining risk is not a discovered silent fallback in current production but test ambiguity: direct helper tests and shared-materializer parity tests can succeed without exercising the canonical root publication path.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Route counters

`retained-dag.ts` exposes `retainedIdentityCounterSnapshot()` with counters for:

- retained hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- retained children visited;
- direct materializer calls;
- derivation fast-path calls;
- reference words written;
- byte payload bytes;
- scratch reuse;
- stale-reference retries;
- decorated normalization;
- host mutations.

(`packages/iyon-tui/src/transport/structural/retained-dag.ts:111-148`.)

Tests explicitly reset/read these counters in:

- `tui_h3_c_transport.test.ts:23-50`, `73-134`;
- `tui_perf13_b.test.ts:54-68`.

The strongest route assertion found is the wide-axis test:

```text
reset counters
→ Tui.render(base)
→ Tui.render(next)
→ derivation_fast_path_calls === 1
→ retained_children_visited === 0
```

(`tui_h3_c_transport.test.ts:35-54`.)

This verifies a specific retained derivation optimization for one internal semantic constructor. It does not prove that all public semantic operations use the intended route.

### 6.2 Counter limitations

The tests generally do not assert:

- that the expected generated ABI function family was called;
- that no obsolete helper was called;
- that no fallback or direct materializer path was selected when a derivation was expected;
- that the final host frame was committed through the desired/visible epoch protocol rather than direct host rendering;
- that native and TS route counters agree across all boundary types.

For example, most text/style/content tests only assert screen rows or cell style. A wrong implementation that produces the same screen output through a different path could pass.

The counter names also mix semantic materialization and route observability. `direct_materializer_calls > 0` proves that a direct materializer ran, but it does not prove that no other route also ran. The tests would be stronger if route identity were represented as an explicit per-operation event or assertion token rather than inferred from aggregate counters.

### 6.3 Test fixture cache interactions

`renderRetained` can populate semantic/native hints before the tested operation:

```text
renderRetained(reference, view)
  → tryRetainedMaterializeRef(view)
  → install semantic/native hint
  → hostRenderRef
```

When a later test invokes a production or direct route with the same semantic objects, it may exercise hint promotion/hit behavior that would not occur in a clean application lifecycle.

This is particularly relevant to:

- `tui_native_scalar.test.ts`;
- `tui_native_builder.test.ts`;
- `tui_native_persistent_seq.test.ts`;
- `tui_native_transaction.test.ts`;
- `tui_native_strings.test.ts`;
- `tui_generated_view_abi.test.ts`.

The tests do create separate hosts, but the module-level semantic/native sidecar state can span those hosts. `native-view-abi.ts` caches the ABI session globally (`native-view-abi.ts:98-128`), and retained-dag semantic hints are sidecar state. Tests therefore validate environment/session behavior, but not necessarily a fresh process-independent route.

### 6.4 Scheduling and microtask assertions

The external-consumer scoped tests include explicit two-microtask draining:

```text
scheduleFlush queues M1
await queues continuation after M1
```

(`packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:32-37`.)

Those tests verify:

- initial scope execution;
- state-read frontier invalidation;
- keyed reorder identity;
- direct/builder ownership transitions.

They are strong composition/scheduling evidence, but do not assert native route identity. A broken native publication path could only be detected where the screen assertion depends on visible output; no counter connects the observed result to the intended retained materializer.

---

## 7. Tests, benchmarks and observability

### 7.1 Public consumer fixture

The consumer fixture is the best evidence that a real external-shaped consumer can use the public API without internal setup.

`packages/tui-consumer-fixture/src/consumer.ts` states:

- import only documented public `@iyon/tui` or `@iyon/tui/testing`;
- no internal runtime imports;
- no composition/compiler/plugin setup;
- no feature flags;
- no manual memoization or identity discipline.

(`consumer.ts:1-12`.)

Its imports comply:

```ts
import { Insets, Scene, Style, View } from "@iyon/tui";
import { AppHarness } from "@iyon/tui/testing";
```

(`consumer.ts:15-17`.)

The later scoped consumer also uses public `defineView`, `state`, `View.key`, `View`, and `TuiRuntime.render` (`consumer.ts:103-169`).

The tests cover:

- public surface/chrome rendering;
- exact semantic no-op;
- text update;
- conditional branch insertion/removal;
- repeated `ViewSlot` updates;
- repeated `ScrollPane` updates;
- state frontier execution;
- 1,000 sibling scopes;
- keyed reorder identity;
- builder → direct ownership transition;
- stale builder inertness.

(`consumer.test.ts:33-118`; `scoped-invalidation.test.ts:39-191`.)

These tests protect the external API and composition contract. They do not prove the native structural implementation route beyond whatever visible screen assertions require.

### 7.2 Tests directly blessing obsolete/internal routes

#### `tui_native_scalar.test.ts`

Imports:

- `textLayoutAtNativePathForTransport`;
- `renderRetained`;
- internal retained ABI support.

(`tui_native_scalar.test.ts:3-13`.)

It directly constructs path metadata and compares an `AppHarness` render with `renderRetained`. All four tests return immediately if `NativeTuiHost` is unavailable (`tui_native_scalar.test.ts:19-20`, `40-42`, `62-64`, `90-92`).

The path constructor under test is not the current ordinary public `Tui.render` authoring contract. This suite should be classified as legacy/internal path ABI characterization unless the path ABI is intentionally retained.

#### `tui_native_builder.test.ts`

The first test directly calls `tryRetainedAxisCreateRender` (`tui_native_builder.test.ts:19-50`). This bypasses the current root publication route.

The second test does use `AppHarness`, but it compares output against `renderRetained` (`tui_native_builder.test.ts:53-72`). Both tests skip if the native host/session is unavailable (`tui_native_builder.test.ts:20-21`, `53-55`).

#### `tui_native_persistent_seq.test.ts`

The file explicitly states that it is testing native retained wide-edit primitives:

- `view_axis_set_child`;
- `view_axis_splice_buffer`;
- `view_grid_set_cell`.

(`tui_native_persistent_seq.test.ts:30-35`.)

It then calls:

- `tryRetainedAxisSetChildRender`;
- `tryRetainedAxisSpliceRender`;
- `tryRetainedGridSetCellRender`.

(`tui_native_persistent_seq.test.ts:47-50`, `94-97`.)

These are direct helper routes and not current production `Tui.render`/`RetainedRootBoundary` updates.

Both tests return without assertions when `NativeTuiHost` is unavailable (`tui_native_persistent_seq.test.ts:37-38`, `69-70`).

#### `tui_native_transaction.test.ts`

This is the clearest old-route test:

- imports `tryRetainedEditTransactionRender`;
- manually builds `NativePathLineage`;
- manually provides `nodeIds`, wrap, and alignment;
- directly calls the transaction helper;
- compares the result to `renderRetained`.

(`tui_native_transaction.test.ts:3-18`, `23-65`.)

It returns if the host/session is unavailable (`tui_native_transaction.test.ts:24-25`) and can also return if `nativeViewRefForNodeId(base)` is unavailable after setup (`tui_native_transaction.test.ts:42-44`).

No current production source call to `tryRetainedEditTransactionRender` was found.

#### `tui_generated_view_abi.test.ts`

This test directly invokes generated ABI calls:

- `hostRenderRef`;
- `viewRenderRef`;
- `viewSpacerCreate`;
- `viewReleaseMany`;
- patch functions.

(`tui_generated_view_abi.test.ts:1-6`, `13-115`.)

Several cases return without assertion if the host/session is unavailable (`tui_generated_view_abi.test.ts:15`, `28-30`, `47`, `58-60`).

The manifest test pins a 60-function ABI and function names/order (`tui_generated_view_abi.test.ts:13-24`; `generated/view_abi_layout.test.ts:7-107`). This verifies ABI shape, not current production route usage.

#### `tui_values.test.ts`

The file mixes:

- public semantic values;
- direct generated `viewTextCreateBuffer`;
- direct `viewRenderRef`;
- direct `tryRetainedMaterializeRef`;
- direct host construction;
- worker/session lifecycle.

(`tui_values.test.ts:3-15`.)

The malformed-buffer and cache tests return if the ABI session is unavailable (`tui_values.test.ts:57-60`, `87-90`). They are valid low-level boundary tests but do not validate the public semantic route.

#### `tui_t14_fuzz_property.test.ts`

The malformed payload test and cache test both return if `nativeViewAbiSession()` is unavailable (`tui_t14_fuzz_property.test.ts:40-43`, `93-95`). The suite directly invokes generated `viewTextCreateBuffer` and `viewRenderRef` (`tui_t14_fuzz_property.test.ts:40-90`).

This is useful ABI hardening evidence, but not a public-route test.

#### `tui_native_strings.test.ts`

The Unicode/style test skips when either host or session is unavailable (`tui_native_strings.test.ts:14-18`). It uses `renderRetained` as the comparison route (`tui_native_strings.test.ts:34-39`).

The second test uses `AppHarness` but also skips if the session is unavailable (`tui_native_strings.test.ts:47-55`).

### 7.3 Tests with vacuous native success

The following current TS cases can pass by returning from the test body when native prerequisites are absent:

| Test file | Skip condition | Evidence |
|---|---|---|
| `tui_native_scalar.test.ts` | `Host === undefined` in all four tests | `:19-20`, `:40-42`, `:62-64`, `:90-92` |
| `tui_native_builder.test.ts` | host/session absent in both tests | `:19-21`, `:53-55` |
| `tui_native_persistent_seq.test.ts` | host absent in both tests | `:37-38`, `:69-70` |
| `tui_native_transaction.test.ts` | host/session absent; base ref absent | `:23-25`, `:42-44` |
| `tui_native_strings.test.ts` | host/session absent | `:14-18`, `:47-49` |
| `tui_generated_view_abi.test.ts` | session/host absent in several tests | `:13-15`, `:27-30`, `:45-48`, `:57-60` |
| `tui_values.test.ts` | session absent in malformed/cache tests | `:57-60`, `:87-90` |
| `tui_t14_fuzz_property.test.ts` | session absent in both tests | `:40-43`, `:93-95` |

The production harness itself does not silently fallback when the native class is absent: `Tui.open` calls `requireNativeClass` (`runtime.ts:319-340`), and `requireNativeClass` throws on missing native class (`packages/iyon-tui/src/transport/native/addon.ts:237-246`). The vacuity exists in the tests’ skip guards, not in the production runtime.

### 7.4 Tests with weak failure specificity

Some low-level tests treat any thrown exception as sufficient:

- malformed native input uses `catch { caught = true }` in the subprocess (`tui_native_input_validation.test.ts:45-70`, `81-88`, `102-121`);
- malformed framing catches all errors (`tui_t14_fuzz_property.test.ts:53-70`);
- some native lifecycle tests use bare `.toThrow()` (`tui_generated_view_abi.test.ts:54`; `tui_perf13_h.test.ts:73-74`).

This is acceptable for panic/no-partial-mutation regression tests when the contract is “must reject and preserve state,” but it weakens diagnosis and can allow the wrong validation layer to satisfy the test. In particular, the malformed input subprocess directly invokes raw native resources (`tui_native_input_validation.test.ts:40-70`) rather than proving that the public API’s validation and native validation agree.

### 7.5 Rust generated-wrapper test is not implementation coverage

`crates/iyon-tui-native/tests/generated_view_abi.rs` defines test-only `NativeViewRuntime` and `NativeHost` types (`generated_view_abi.rs:4-17`) and test-local exported implementations returning sentinel values (`generated_view_abi.rs:26-58` and throughout the generated stub block).

The main wrapper test is named `generated_wrappers_reject_invalid_inputs_and_delegate` (`generated_view_abi.rs:1410-1413`) and invokes the generated wrapper dispatch functions, asserting sentinel values (`generated_view_abi.rs:1414-1600`).

This proves:

- generated function names/signatures;
- pointer/buffer precondition handling;
- dispatch to the expected symbol;
- invalid pointer/buffer return behavior.

It does **not** prove:

- `NativeViewRuntime` behavior in `crates/iyon-tui-native/src/tui/view_abi.rs`;
- actual native object lifetime;
- host rendering;
- native retained sequence behavior;
- native state/content attachment behavior.

### 7.6 Native handwritten unit-test gap

The native handwritten `tui.rs` contains only two local tests in the inspected region:

1. Unicode cursor state for `NativeTextInput`;
2. status mapping for cleanup errors.

(`crates/iyon-tui-native/src/tui.rs:1963-2000+`.)

There is no equivalent native Rust unit coverage for:

- `NativeTuiHost` desired/visible root behavior;
- native root epochs;
- native view materialization;
- native host rendering;
- slot/scroll-pane retained ref installation;
- native state patch application;
- direct native content/source lifecycle.

Those behaviors are primarily tested through staged Bun/N-API integration. If staging is skipped, broken, or replaced by a fake addon, the Rust test suite alone cannot detect their failure.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Strongest contradiction: “single production route” versus old direct test routes

Current production comments repeatedly state:

- retained semantic materialization is the single production structural architecture;
- there is no secondary complete-object decoding path;
- refusal fails explicitly.

(`retained-dag.ts:19-23`, `runtime.ts:218-222`.)

At the same time, the repository retains and tests direct route helpers that:

- accept a host object;
- materialize or patch native refs directly;
- install those refs with `installNativeRef`;
- return a native ref to the test.

(`native-view-abi.ts:228-250`, `317-350`, `357-400`, `403-436`, `443-510`.)

This is not necessarily a production contradiction because source search found no ordinary production callers. It is a test/benchmark architecture contradiction: current test artifacts continue to treat the older direct helper route as a first-class retained behavior.

### 8.2 “Same-architecture reference” is not independent

The fixture comment calls `renderRetained` an authoritative retained reference (`native-host.ts:10-14`), but it shares the production semantic materializer. The two routes differ mainly after semantic materialization:

```text
production:
  ensureSemanticNative
    → desired root publication
    → environment frame drain

fixture:
  ensureSemanticNative
    → hostRenderRef
```

The helper therefore cannot detect a shared bug in:

- semantic node traversal;
- semantic derivation interpretation;
- style/decoration lowering;
- generated constructor selection;
- semantic child materialization;
- text payload encoding.

It can detect some differences in host publication/rendering and native ref ownership.

### 8.3 External-consumer tests do not prove native route identity

The external fixture is correctly public-only and therefore valuable. However:

- `consumer.test.ts` checks screen text and public handle behavior;
- `scoped-invalidation.test.ts` checks scope execution counters and visible header output;
- neither asserts a retained/native route counter;
- neither asserts generated ABI call identity;
- neither tests a deliberate retained-route refusal.

This is not a defect in the consumer fixture’s purpose: a real consumer should not know internal route counters. It is a coverage gap that must be covered by a separate framework-owned route-observability test, not by adding internal imports to the external fixture.

### 8.4 Historical test inventory is stale relative to current source paths

`docs/repository-separation/s0/test-benchmark-inventory.tsv` contains historical paths such as:

- `crates/iyon-native/tests/generated_view_abi.rs`;
- `packages/iyon-runtime/tests/...`.

Current source paths are:

- `crates/iyon-tui-native/tests/generated_view_abi.rs`;
- `packages/iyon-tui/tests/...`.

Examples of the historical entries are visible at `test-benchmark-inventory.tsv:70-73`, `192-235`, and `240-241`.

This document should not be used as current source reachability evidence without path reconciliation. The current `tracked-source-manifest.txt` and actual source tree are the relevant baseline evidence.

### 8.5 Rust unit tests are valid but do not cross the repository boundary

Rust tests in `crates/iyon-tui/src/application/tests.rs` and other `#[cfg(test)]` modules exercise Rust internals directly. They may pass while:

- TypeScript imports are broken;
- the native addon cannot load;
- generated N-API bindings are mismatched;
- the TypeScript retained root route is bypassed or broken.

This is expected for unit tests, but integrated reporting must not combine Rust unit success with TS/native route success.

### 8.6 Staging script is a stronger surface check than test suite

`stage-native.ts` performs runtime addon inspection and rejects:

- removed classes/methods;
- missing content ABI symbols;
- leaked direct-FFI symbols in default artifacts;
- missing default N-API session;
- missing direct-FFI qualification exports when requested.

(`stage-native.ts:49-121`.)

This is valuable packaging evidence, but it is not included automatically by the root `test` command. CI explicitly stages first; local users invoking only `bun test` may observe either stale artifact behavior or vacuous test skips depending on the environment.

---

## 9. Open questions and coverage gaps

1. **Are the old path and edit-transaction ABI functions intentionally supported ABI contracts, or are they migration residue?**
   - Current TS production has no callers for the path patch family.
   - `edit_txn_*` is reached through the old `tryRetainedEditTransactionRender` helper used by one test.
   - The generated manifest still pins all functions.

2. **Should direct helper tests remain as low-level ABI tests, or should they be rewritten around `Tui.render` with route instrumentation?**
   - Their current output parity is useful but does not verify the canonical root publication route.

3. **Is `renderRetained` intended as a temporary oracle or permanent test fixture?**
   - Its comment presents it as authoritative.
   - Its semantic materializer is shared with production, so it cannot be a fully independent oracle.
   - If retained, its scope should be explicitly limited to host/ABI parity and native lifetime.

4. **Does the public declaration surface expose concrete `ViewSlot`/`ScrollPane` implementation methods despite the narrower interfaces?**
   - Historical API-H1 evidence identifies this as a concern.
   - Current runtime ownership checks inspect many seams, but no test was found that attempts to compile a consumer against forbidden methods and expects failure.

5. **What is the required behavior when the native artifact/session is absent?**
   - Production `Tui.open` fails explicitly.
   - Many low-level tests return successfully.
   - A test policy decision is needed: skip, fail, or explicitly mark unavailable prerequisites.

6. **Are native Rust behavior tests intentionally delegated entirely to Bun/N-API?**
   - Native handwritten unit tests do not exercise host rendering or retained ABI implementation.
   - Generated Rust tests use stubs.
   - This leaves the staged integration sequence as the primary native implementation oracle.

7. **Are aggregate retained counters sufficient route observability?**
   - They identify some retained activity but do not identify every generated operation or detect old helper invocation.
   - No universal “this operation used this architecture” assertion exists.

8. **Does the current test suite intentionally cover both direct `Scene` values and canonical builder producers?**
   - Yes, but coverage is uneven:
     - consumer scoped tests emphasize canonical builders;
     - many native low-level tests use direct scenes or direct helpers;
     - route-specific failure tests are not symmetric across both forms.

9. **Can any test pass after replacing the intended retained path with a semantically equivalent direct renderer?**
   - Yes for output-only tests and shared-materializer parity tests.
   - No current source evidence shows such a fallback exists, but the tests would not reliably detect it.

10. **Do current CI workflows guarantee that the staged addon is the artifact loaded by every test?**
    - The staging script verifies artifact identity and content ABI metadata.
    - The test command itself does not perform that check.
    - Environment overrides such as `ION_TUI_NATIVE_ARTIFACT` can alter the loaded artifact.

---

## 10. Evidence appendix

### 10.1 Inspected current production files

#### TypeScript runtime and structural transport

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`;
  - `Tui`;
  - `render`;
  - `renderCanonical`;
  - `renderDirect`;
  - `prepareRootPublication`;
  - `RetainedExecutionRuntime` setup;
  - `registerRuntimeAccess`.

- `packages/iyon-tui/src/runtime/access.ts`
  - `registerRuntimeAccess`;
  - `runtimeAccess`.

- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness`;
  - deterministic clock;
  - headless inspection;
  - input injection.

- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `ensureNative`;
  - `ensureSemanticNative`;
  - `tryDerivation`;
  - `recoverStaleNode`;
  - `materializeWithRecovery`;
  - `renderExactRoot`;
  - `acquireKnownRoot`;
  - `RetainedRootBoundary`;
  - `prepareFrom`;
  - `prepareInstall`;
  - `prepareDesiredInstall`;
  - root lease/desired/visible transitions;
  - route counters.

- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `nativeViewAbiSession`;
  - `nativeViewRefForNodeId`;
  - `tryRetainedMaterializeRef`;
  - direct axis/grid/edit helper families;
  - path lineage;
  - `releaseNativeViewRef`.

- `packages/iyon-tui/src/transport/structural/retained-path.ts`
  - path step/lineage types;
  - path patch constructors;
  - transaction constructors;
  - path sidecars.

- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - generated wrapper families and status conversion.

- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
  - generated native ABI interface and path/edit function declarations.

- `packages/iyon-tui/src/api/view/view.ts`
  - semantic node construction;
  - `axisSetChildForTransport`;
  - `axisSpliceForTransport`;
  - `gridSetCellForTransport`;
  - semantic derivation construction;
  - attachment sidecars.

- `packages/iyon-tui/src/api/controls/view-slot.ts`
  - public `ViewSlot` interface;
  - retained slot boundary;
  - direct/builder ownership;
  - animation refs;
  - implementation seam methods.

- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - retained scroll-pane boundary and content installation.

- `packages/iyon-tui/src/api/controls/history.ts`
  - history view materialization and temporary retained leases.

- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeTuiHostContract`;
  - removed high-level host methods;
  - `requireNativeClass`;
  - native artifact loading.

- `packages/iyon-tui/src/index.ts`
  - root public exports.

- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - semantic identity and derivation sidecar APIs.

#### TypeScript tests and fixtures

- `packages/iyon-tui/tests/fixtures/native-host.ts`
- `packages/iyon-tui/tests/fixtures/tui_demo.ts`
- `packages/iyon-tui/tests/tui_ansi_scanner.test.ts`
- `packages/iyon-tui/tests/tui_demo.test.ts`
- `packages/iyon-tui/tests/tui_generated_view_abi.test.ts`
- `packages/iyon-tui/tests/generated/view_abi_layout.test.ts`
- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_h3_b_composition.test.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_handles.test.ts`
- `packages/iyon-tui/tests/tui_harness.test.ts`
- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
- `packages/iyon-tui/tests/tui_native_input_validation.test.ts`
- `packages/iyon-tui/tests/tui_native_persistent_seq.test.ts`
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
- `packages/iyon-tui/tests/tui_native_strings.test.ts`
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
- `packages/iyon-tui/tests/tui_perf13_b.test.ts`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `packages/iyon-tui/tests/tui_perf13_h.test.ts`
- `packages/iyon-tui/tests/tui_realtime.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `packages/iyon-tui/tests/tui_runtime.test.ts`
- `packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts`
- `packages/iyon-tui/tests/tui_semantic_pipeline.test.ts`
- `packages/iyon-tui/tests/tui_smooth_delivery.test.ts`
- `packages/iyon-tui/tests/tui_state_envelope.test.ts`
- `packages/iyon-tui/tests/tui_surface_contract.test.ts`
- `packages/iyon-tui/tests/tui_t14_differential.test.ts`
- `packages/iyon-tui/tests/tui_t14_fuzz_property.test.ts`
- `packages/iyon-tui/tests/tui_text_lanes.test.ts`
- `packages/iyon-tui/tests/tui_traits.test.ts`
- `packages/iyon-tui/tests/tui_values.test.ts`
- `packages/iyon-tui/tests/tui_worker_lifecycle.ts`

#### External-consumer fixture

- `packages/tui-consumer-fixture/package.json`
- `packages/tui-consumer-fixture/src/consumer.ts`
- `packages/tui-consumer-fixture/tests/consumer.test.ts`
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts`

#### Native/Rust source and tests

- `crates/iyon-tui-native/Cargo.toml`
- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/tests/generated_view_abi.rs`
- `crates/iyon-tui-native/tests/sync.rs`
- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui/src/application/tests.rs`
- Rust source files indexed using the tracked source manifest and `#[cfg(test)]` reference search.

#### Build and boundary checks

- `package.json`
- `packages/iyon-tui/package.json`
- `Cargo.toml`
- `packages/iyon-tui/scripts/stage-native.ts`
- `tools/ownership/check.ts`
- `tools/api-surface/check-declaration-closure.ts`
- `tools/api-surface/check-binding.ts`
- `.github/workflows/ci.yml`
- `.github/workflows/agent-fast.yml`
- `.github/workflows/tui-typescript.yml`
- `.github/workflows/t1-bun.yml`

#### Historical/context files

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/history/API-H1/API-H1-V2-public-api-hygiene.md`
- `docs/repository-separation/s0/test-benchmark-inventory.tsv`

### 10.2 Exact high-value symbols and line references

| Finding | Evidence |
|---|---|
| Public `Tui.render` chooses canonical/direct route | `packages/iyon-tui/src/runtime/runtime.ts:399-405` |
| Canonical builder root lifecycle | `runtime.ts:451-496` |
| Direct scene lifecycle | `runtime.ts:499-548` |
| Retained path declared single production architecture | `packages/iyon-tui/src/transport/structural/retained-dag.ts:1-23` |
| Retained identity-first entrypoint | `retained-dag.ts:1148-1216` |
| Derivation fast path | `retained-dag.ts:1252-1365` |
| Same-architecture stale recovery | `retained-dag.ts:415-432`, `1350-1365`, `1380-1462` |
| Root desired/visible lease protocol | `retained-dag.ts:1488-1507`, `1919-1942` |
| Direct old helper family | `packages/iyon-tui/src/transport/structural/native-view-abi.ts:228-510` |
| Old path metadata constructors | `packages/iyon-tui/src/transport/structural/retained-path.ts:63-134` |
| Shared test helper | `packages/iyon-tui/tests/fixtures/native-host.ts:10-28` |
| Public consumer import boundary | `packages/tui-consumer-fixture/src/consumer.ts:1-17` |
| Consumer composition route | `consumer.ts:126-169` |
| Consumer scope tests | `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts:39-191` |
| Native host removed methods | `packages/iyon-tui/scripts/stage-native.ts:53-68` |
| Native addon contract | `packages/iyon-tui/src/transport/native/addon.ts:134-189` |
| Native session availability behavior | `packages/iyon-tui/src/transport/structural/native-view-abi.ts:98-128` |
| Native generated wrapper stubs | `crates/iyon-tui-native/tests/generated_view_abi.rs:19-58` |
| Native generated wrapper test | `generated_view_abi.rs:1410-1600` |
| Native handwritten test count/gap | `crates/iyon-tui-native/src/tui.rs:1963-2000+` |
| Rust crate internal-runtime status | `crates/iyon-tui/Cargo.toml:6-18` |
| Historical API-H1 internal-surface concern | `docs/history/API-H1/API-H1-V2-public-api-hygiene.md:1949` |

### 10.3 Search scope and absence claims

The absence claims in this report are scoped to:

- current repository source under `packages/iyon-tui/src`;
- current repository tests and benchmarks under `packages/iyon-tui`;
- current native/Rust source and tests under `crates/iyon-tui-native` and relevant `crates/iyon-tui` paths;
- direct symbol/reference searches for the named old helper families.

I did not claim that no historical branch, generated artifact, external consumer, or future package uses these helpers. Historical documentation and the stale repository-separation inventory contain older names and paths, so those were treated as historical references unless current source references were found.

### 10.4 Validation status

No tests, builds, staging commands, or benchmarks were executed in this investigation. The report is based on static source and manifest evidence only.