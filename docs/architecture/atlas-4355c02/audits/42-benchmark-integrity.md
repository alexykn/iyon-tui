# 42 — Benchmark integrity

## 0. Baseline, scope and evidence status

**Baseline:** repository `/Users/alxknt/github/iyon-n/iyon-tui`, branch `main`, source revision `4355c02d6853549adf32a1e038b14665ce5c6bf8`.

This report covers the benchmark and instrumentation surfaces in the baseline:

- Rust benchmark binary and counter implementation.
- TypeScript benchmark runners under `packages/iyon-tui/bench/`.
- Generated ABI benchmark metadata.
- Committed benchmark result artifacts.
- CI benchmark invocations and native staging behavior.
- Route and counter assertions in adjacent TypeScript/Rust tests.
- The production retained materialization and runtime boundaries necessary to determine what the benchmarks actually execute.

The investigation was **read-only and static**. I did not rerun Rust benchmarks, TypeScript performance suites, native staging, or other costly qualification workloads. Historical result files and reports were inspected as historical evidence only; their measurements are not treated as fresh validation of the `4355c02` source tree.

Evidence categories used below:

- **Current source fact:** directly visible in the baseline source.
- **Static inference:** a consequence of current call graphs or branch logic, not runtime observation.
- **Historical evidence:** a committed report/result produced at another source revision.
- **Unknown:** not provable from the inspected source without executing an additional qualification or adding observability.

The required architecture documents were read first:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md` in full for benchmark-route and evidence expectations.

The framework boundary is material here: `iyon-tui` is a generic TUI framework. The benchmark fixtures use generic text, views, streams, layouts, and native handles; they do not constitute an Iyon application benchmark.

### Principal conclusion

There are several materially different benchmark paths:

1. **Current TypeScript T15 benchmark:** measures the retained semantic DAG through generated safe N-API calls and a direct, non-deferred `RetainedRootBoundary` host publication. It does **not** measure the feature-gated raw direct-FFI arm, despite the CI job name and metadata.
2. **Production `Tui.render`:** has canonical-builder and direct-scene forms, both using the retained DAG, but production uses a deferred desired-root publication followed by the environment/native host frame barrier. T15 bypasses that `Tui` runtime layer.
3. **`perf13_h_content.ts`:** measures a mixed content path: Source payload mutation uses the direct `bun:ffi` content ABI, while structural `View.content(port)` publication uses retained generated N-API and the deferred TUI host barrier.
4. **Rust `tui_perf` benchmark:** measures in-process Rust `View` construction, resolution, custom layout, and painting. It does not cross the TypeScript/native retained structural boundary and does not execute the production `Tui` runtime path.
5. **Current source contains no complete-object/cold/legacy structural fallback route.** Retained refusal, stale-reference recovery, derivation fallback, and NodeId promotion are all internal branches of the retained implementation. They do not select another architecture.
6. **Route assertions are incomplete.** Some tests assert retained counters or refusal behavior, but the main T15 runner does not assert `RootPublication.route === "retained"`, and no current counter universally distinguishes generated N-API structural calls from feature-gated raw direct-FFI calls.
7. **Committed “authoritative” T15 artifacts are historical and self-labelled.** Their source SHAs differ from the requested baseline, and the current tree does not contain the matrix orchestrator that produced the 311-case-per-arm result described by the historical report.

---

## 1. Responsibility and structure

### 1.1 Benchmark and instrumentation inventory

| Area | Path | Approximate physical LOC | Responsibility | Production path? |
|---|---|---:|---|---|
| Rust benchmark entrypoint | `crates/iyon-tui/src/bin/tui_perf.rs` | 2 | Calls `iyon_tui::perf_bench::run()` | No; benchmark tooling |
| Rust benchmark suite | `crates/iyon-tui/src/perf_bench.rs` | 557 | Rust-only view clone, layout, paint, History, and cache timing oracle | No; direct internal benchmark |
| Rust counters | `crates/iyon-tui/src/perf.rs` | 251 | Feature-gated counter enum, names, reset, increment, snapshot, test lock | Instrumentation only |
| TypeScript retained T15 case | `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts` | 122 | Constructs scenarios, runs retained boundary publication, records phase timings and retained identity counters | Partially; bypasses `Tui.render` |
| TypeScript T15 workload builder | `packages/iyon-tui/bench/perf12_t15_workload.ts` | 162 | Builds immutable View scenarios for exact identity, shared paths, derivations, wide edits, grids, and scalar patches | Fixture only |
| PERF-13 content benchmark | `packages/iyon-tui/bench/perf13_h_content.ts` | 49 | Appends many chunks to a `TextStreamSource`, flushes one frame, reports Source and wake statistics | Uses production-style headless runtime |
| L1 trace harness | `packages/iyon-tui/bench/pre-v5-l1-trace.ts` | 143 | Exercises direct retained structural publication, retained state, and content append/readback in one JSON record | Mixed production/test harness |
| L1 text-lane benchmark | `packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts` | 59 | Cold renders 300 nodes across cstring, UTF-8, and wide-span lanes | Uses headless TUI, but route assertions are weak |
| Generated ABI registry | `packages/iyon-tui/bench/generated/view_abi_cases.ts` | 76 | Generated metadata describing ABI benchmark registrations, families, hotness, and limits | Not an executable benchmark |
| ABI schema | `tools/tui-abi/view_abi.toml` | approximately 5,000+ | Canonical function, ownership, lowering, buffer, and benchmark-registration schema | Generator input |
| Generated safe-N-API calls | `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts` | generated | Calls methods on the opaque N-API `NativeViewAbiSession` | Yes, for current TS retained structural route |
| Retained materializer | `packages/iyon-tui/src/transport/structural/retained-dag.ts` | approximately 2,100+ | Identity lookup, NodeId promotion, semantic materialization, derivation, stale recovery, leases, and root boundary | Yes |
| Runtime router | `packages/iyon-tui/src/runtime/runtime.ts` | approximately 930 | Canonical/direct render selection, root boundary setup, deferred host commit, flush, lifecycle | Yes |
| Content direct ABI | `packages/iyon-tui/src/transport/content/ffi.ts` | approximately 785 | Bun `dlopen` content Source ABI, payload encoding, mutation result handling, wake request | Yes for Source payload lane |
| Native staging | `packages/iyon-tui/scripts/stage-native.ts` | 124 | Builds/stages addon, checks default/direct feature surfaces and ABI symbols | Build support |
| CI | `.github/workflows/ci.yml` | 112 | Runs tests, default T15 smoke, and nominal direct-FFI T15 smoke | Qualification driver |

The three `l13_*` files found under `packages/iyon-tui/bench/` are explicitly ignored by `.gitignore`:

- `/packages/iyon-tui/bench/l13_content_lanes.ts`
- `/packages/iyon-tui/bench/l13_source_workload.ts`
- `/packages/iyon-tui/bench/l13_view_state_workload.ts`

They were inspected for route and counter context, but they are not part of the tracked source baseline according to `.gitignore` lines 7–10 and the tracked-source manifest.

### 1.2 Benchmark families

#### Rust `tui_perf`

`crates/iyon-tui/src/perf_bench.rs` defines:

- `Workload::{TextHeavy, ColumnHeavy, RowHeavy, GridHeavy, StyledSpanHeavy, ComponentHeavy}`.
- View sizes of 20, 200, 2,000, and 10,000 nodes.
- Patterns `FRESH`, `IDENTICAL_IDENTITY`, `SHARED_PATH`, and `REBUILT_EQUIVALENT`.
- A view-clone case at 100 and 10,000 nodes.
- A paint gate for selected 2,000- and 10,000-node workloads.
- A History case with 1,000 static units and a 1,001-unit live-tail variant.

The fixture is built with the internal Rust presentation factory (`presentation::factory as vf`). Rendering calls:

- `ResolveSession::resolve_root` for component fixtures.
- `layout_resolved_scene_with_cache` or `layout_view_with_overlay_and_cache`.
- `ViewCompiler`.
- `ViewPainter.paint_tree` or `paint_tree_with_cache`.

No TypeScript object crosses the boundary. No `NativeTuiHost`, `nativeViewAbiSession`, generated N-API wrapper, retained DAG, Source, ContentPort, or environment wake broker participates.

The executable is hidden behind the `perf-counters` feature in `crates/iyon-tui/Cargo.toml` and is not invoked by the current CI workflow. `cargo test --workspace --all-features` can compile the target but does not execute its benchmark loop.

#### TypeScript T15

The T15 runner constructs a native `NativeTuiHost`, obtains a `NativeViewAbiSession`, and creates:

```ts
const boundary = new RetainedRootBoundary(session, () => host);
```

The constructor receives no `deferHostCommit` option. `RetainedRootBoundaryOptions.deferHostCommit` therefore defaults to `false` (`retained-dag.ts:1545–1548`, `1587–1600`).

Every measured render calls:

```ts
const publication = boundary.prepareInstall(view);
if (publication === undefined) throw ...
publication.commit();
```

`prepareInstall` returns a publication whose diagnostic route is statically `"retained"` (`retained-dag.ts:1679–1702`). With non-deferred mode, `publishPrepared` invokes `hostRenderRef` directly through the generated safe N-API call path (`retained-dag.ts:1950–2005`).

The runner does not instantiate `Tui`, `AppHarness`, `OwnedBuilderRoot`, `RetainedExecutionRuntime`, or the environment wake broker. Its timing therefore covers:

- JavaScript semantic scenario construction.
- Retained preparation.
- Generated safe N-API materialization calls.
- Immediate host reference rendering.
- Root lease bookkeeping.

It does not cover the production `Tui.render` deferred desired-root publication and subsequent `flushPendingHosts` frame barrier.

#### PERF-13 content benchmark

`perf13_h_content.ts`:

1. Creates `TextStreamSource` with bounded drop-oldest retention.
2. Opens `AppHarness` in headless mode.
3. Obtains a `ContentPort`.
4. Connects the Source through `TextFunnel.plain()`.
5. Activates the connector.
6. Renders `View.content(port)`.
7. Measures repeated `source.append(...)`.
8. Measures `harness.flush()`.
9. Reports Source stats and wake-broker counters.

The Source mutation path is not the structural retained N-API path. `TextStreamSource.append` calls `appendTextSource` (`api/content/retained.ts:380–384`), which calls `invokePayload` (`transport/content/ffi.ts:653–693`). `invokePayload` UTF-8 encodes in JavaScript and directly invokes `iyon_tui_source_append_utf8_v1` or `iyon_tui_source_replace_utf8_v1` from the Bun `dlopen` session (`ffi.ts:250–279`, `653–693`).

The initial and frame structural path is different:

```text
View.content(port)
  → Tui.render(scene)
  → Tui.renderDirect
  → deferred RetainedRootBoundary
  → setDesiredViewRef
  → environment wake broker
  → NativeTuiHost.flushPendingHosts
  → content projection / measure / layout / paint
```

Thus the benchmark is useful for the content lane, but its append timing is a direct content C ABI measurement, while its frame timing includes the host/content frame path. It is not a pure structural benchmark.

#### L1 trace

`pre-v5-l1-trace.ts` intentionally combines multiple route shapes:

- Structural phase uses a direct non-deferred `RetainedRootBoundary`, like T15.
- State phase uses `AppHarness` and `Tui.render` with a canonical builder.
- Content phase uses `AppHarness`, `ContentPort`, `TextStreamSource`, and a direct scene object.

It records JavaScript retained counters and optionally Rust counters if the loaded addon exports `tuiPerfSnapshot` and `tuiPerfReset`.

---

## 2. Types, APIs and contracts

### 2.1 Current route-related contracts

#### `RootPublication.route`

`RootPublication` exposes:

```ts
readonly route?: "retained";
```

in `packages/iyon-tui/src/transport/structural/retained-dag.ts:1521–1529`.

Both `prepareInstall` and `prepareDesiredInstall` construct publications with:

```ts
route: "retained"
```

at approximately lines 1684–1687 and 1717–1720.

This is a useful diagnostic contract, but the main benchmark does not consume it. `perf12_t15_authoritative_case.ts:48–52` only checks whether the publication is `undefined`; it does not assert:

```ts
publication.route === "retained"
```

Therefore the benchmark's route identity is inferred from current implementation, not enforced by the result-producing assertion.

#### Retained identity counters

`RetainedIdentityCounters` (`retained-dag.ts:111–126`) contains:

- `retained_hint_hits`
- `retained_hint_misses`
- `node_id_ref_promotion_attempts`
- `node_id_ref_promotion_hits`
- `node_id_ref_promotion_misses`
- `retained_semantic_nodes_inspected`
- `retained_children_visited`
- `direct_materializer_calls`
- `derivation_fast_path_calls`
- `ref_words_written`
- `byte_payload_bytes`
- `transport_scratch_reuses`
- `stale_ref_retries`
- `decorated_normalized_nodes`
- `host_mutations`

These counters are ordinary JavaScript object fields, not atomics. They are useful in isolated single-threaded Bun benchmark processes and tests, but they do not identify the physical transport ABI. For example, `direct_materializer_calls` means a retained materializer was invoked; it does not distinguish generated safe N-API from a raw direct C call because the materializer invokes whichever wrappers are imported by the current TypeScript module.

#### Rust `perf::Counter`

`crates/iyon-tui/src/perf.rs:15–70` declares 70 counters, including:

- Rust View construction and cloning.
- Historical `NapiViewNodesSeen`, `NapiViewCacheHits`, `NapiViewCacheMisses`, and `NapiViewStringBytesCopied`.
- Resolver, measure, layout, paint, and History counters.
- View-state invalidation and paint counters.
- Content Source/projection counters.
- Dirty propagation and candidate-preparation counters.

The `NapiView*` counters are declared but no current production increments were found in the inspected Rust source. They should not be interpreted as evidence that the current generated safe N-API route was exercised.

`perf::reset`, `perf::add`, `perf::inc`, and `perf::snapshot` are feature-gated. Without `perf-counters`, increments compile to no-ops and snapshots are zero-valued (`perf.rs:194–251`). The native addon exports `tuiPerfReset` and `tuiPerfSnapshot` only under the `perf-counters` feature (`crates/iyon-tui-native/src/tui.rs:249–263`).

There is no route counter named `retained`, `cold`, `legacy`, `direct_ffi`, `generated_safe_napi`, or `fallback` in the current Rust counter enum.

### 2.2 Generated ABI contract

The canonical schema in `tools/tui-abi/view_abi.toml` classifies each structural operation by:

- ABI family.
- Hotness.
- Ownership.
- Borrow duration.
- Thread affinity.
- Buffer/lane type.
- Maximum input counts.
- Benchmark registration.

The generated TypeScript wrappers in `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts` call methods on the opaque N-API session:

```ts
runtime.viewRenderRef(...)
runtime.hostRenderRef(...)
runtime.viewTextCreateCstring(...)
runtime.viewTextCreateUtf8(...)
runtime.viewAxisCreateBuffer(...)
runtime.viewRefForNodeId(...)
```

Examples are at `view_calls.ts:40–46`, `75–77`, `285–327`, and `210–217`.

The generated wrapper is therefore evidence of the current safe N-API route, not raw direct FFI. The generated direct symbols in the native addon are separately emitted behind `#[cfg(feature = "direct-ffi")]` and are not selected by these wrappers.

### 2.3 Direct-FFI feature contract

`crates/iyon-tui-native/Cargo.toml:12–16` defines `direct-ffi` as a feature. Native generated exports use `#[cfg(feature = "direct-ffi")]` or `#[cfg_attr(feature = "direct-ffi", unsafe(no_mangle))]`, for example:

- `view_render_ref_impl`.
- `host_render_ref_impl`.
- `view_text_create_*`.
- `view_axis_create_buffer_impl`.
- `view_ref_for_node_id_impl`.

The N-API session itself remains available through:

```rust
#[napi(js_name = "tuiViewAbiSession")]
pub fn tui_view_abi_session(...)
```

at `crates/iyon-tui-native/src/tui/view_abi.rs:1358–1415`.

The `direct-ffi` feature adds raw exported symbols and qualification probes; it does not automatically cause TypeScript to use those symbols.

---

## 3. Dependency and ownership map

### 3.1 Current TypeScript structural benchmark path

```text
perf12_t15_authoritative_case.ts
    │
    ├── makeT15Scenario()
    │       └── View semantic constructors/modifiers
    │
    └── RetainedRootBoundary.prepareInstall()
            │
            ├── nativeViewAbiSession()
            │       └── native.tuiViewAbiSession()
            │
            ├── ensureSemanticNative()
            │       ├── semantic NativeRef hint
            │       ├── transaction-local ref
            │       ├── NodeId → NativeRef promotion
            │       ├── derivation fast path
            │       └── generated view_* N-API constructors
            │
            └── publication.commit()
                    └── generated hostRenderRef()
                            └── NativeTuiHost N-API method
```

Ownership:

- The benchmark owns the host and boundary.
- `RetainedRootBoundary` owns the root lease.
- `MaterializeTx` owns temporary leases until commit/abort.
- The native runtime owns the semantic cache, `NativeRef` table, style tables, path tables, builders, and edit transactions.
- The native host owns the visible terminal frame and desired/visible frame epochs.

### 3.2 Current production `Tui.render` path

```text
Tui.render(sceneOrBuilder)
    ├── function input
    │     └── renderCanonical()
    │           ├── RetainedExecutionRuntime root scope
    │           ├── Scene.from(builder())
    │           ├── stageHistoryBinding()
    │           ├── prepareRootPublication()
    │           │     └── RetainedRootBoundary(deferHostCommit: true)
    │           ├── prepareDesiredInstall()
    │           ├── setDesiredViewRef()
    │           └── flush()
    │                 └── environment wake broker
    │                       └── NativeTuiHost.flushPendingHosts()
    │
    └── object input
          └── renderDirect()
                ├── Scene.from(scene)
                ├── attachment validation
                ├── prepareRootPublication()
                │     └── deferred RetainedRootBoundary
                ├── setDesiredViewRef()
                └── flush()
```

`runtime.ts:399–405` selects canonical versus direct mode. `runtime.ts:451–497` implements canonical builder mode. `runtime.ts:499–548` implements direct scene mode. `runtime.ts:535–548` confirms direct scene publication is also passed through the root publication and flush sequence.

### 3.3 Current Source content path

```text
TextStreamSource.append()
    │
    ├── TypeScript validation
    ├── UTF-8 encoding
    ├── annotation record/payload encoding
    ├── Bun dlopen() content ABI symbols
    │     └── iyon_tui_source_append_utf8_v1
    │
    └── result decoding
          └── requestWake()
                └── environment wake broker
```

This is intentionally separate from structural `View` materialization. The content benchmark therefore cannot be used as evidence that structural View construction is or is not on the Source append path.

---

## 4. Execution paths and state transitions

### 4.1 T15 measured lifecycle

For each scenario:

1. `makeT15Scenario` creates an initial immutable semantic View.
2. The initial View is installed before timing.
3. Warmup iterations call `scenario.next(index)` and `render(...)`.
4. Retained identity counters are reset.
5. Each measured iteration:
   - Times `scenario.next(...)` as `semantic_construction`.
   - Calls `boundary.prepareInstall(next)`.
   - Fails if the retained preparation returns `undefined`.
   - Calls `publication.commit()`.
   - Records the remainder as `transportAndHost`.
6. Phase instrumentation records:
   - `transport_prepare_ns`
   - `native_materialize_ns`
   - `host_commit_ns`
7. The result reports `semantic_construction_samples_ns`, phase arrays, total samples, percentiles, retained counter deltas, and screen output.

The measured root publication is direct/non-deferred. Because no `Tui` is present, there is no canonical composition evaluation, no scene normalization, no attachment preparation, no staged History sideband, no `RetainedExecutionRuntime` root scope, and no environment frame receipt.

### 4.2 Production retained lifecycle

Current production `Tui.render` does more than T15:

- Drains pending retained execution before a new render.
- Rejects mutation while a retained protocol pass is active.
- Validates semantic attachments.
- Stages and commits History sideband state.
- Publishes the desired root through a deferred boundary.
- Marks the host pending.
- Flushes through `RuntimeHostRegistration` and the environment wake broker.
- Commits visible root bookkeeping only after the native host frame succeeds.

The production comments explicitly describe this distinction (`runtime.ts:373–389`, `217–222`, and `407–420`).

The T15 benchmark is consequently a useful retained materialization/host-reference microbenchmark, but not an end-to-end production `Tui.render` benchmark.

### 4.3 Retained identity ordering

`ensureSemanticNative` (`retained-dag.ts:1163–1219`) has a hard ordering:

1. Generation-valid semantic hint.
2. Transaction-local reference.
3. Ceiling-gated NodeId promotion.
4. Cycle check.
5. Derivation fast path.
6. Semantic materializer lookup and invocation.
7. State attachment.
8. Hint installation and temporary lease recording.

For known retained nodes, this can avoid payload inspection. For a newly-created node, the current implementation does not apply a separate architecture-selection budget. It materializes through the retained implementation, including reusable scratch buffers and native constructors.

### 4.4 Internal recovery transitions

There are several recovery-like branches, but they are not alternate benchmark architectures:

- A stale native hint may be deleted and retried once (`retained-dag.ts:416–431`).
- A stale child constructor status may trigger one child recovery and constructor retry (`retained-dag.ts:434–467`).
- A missing derivation base may fall back to direct semantic materialization **inside the same retained transaction** (`retained-dag.ts:1222–1262`).
- A missing NodeId promotion may continue to semantic inspection/materialization.
- An unsupported kind, malformed shape, cycle, oversized payload, or unrecoverable native status becomes a retained refusal.

The boundary then returns `undefined` from preparation. Production callers convert that to explicit errors, such as `TUI_ROOT_PREPARATION_FAILED`, `TUI_VIEW_SLOT_UPDATE_FAILED`, or `TUI_SCROLL_PANE_UPDATE_FAILED`. There is no complete-object decoder fallback in the current production TypeScript route.

---

## 5. Alternate routes and failure semantics

| Semantic operation | Current path | Selection condition | Possible internal fallback/recovery | Failure masking risk |
|---|---|---|---|---|
| T15 scenario render | `RetainedRootBoundary.prepareInstall` → generated safe N-API materializers → immediate generated `hostRenderRef` | Always, because runner constructs this boundary directly | Hint miss, NodeId promotion, derivation, stale retry | Route is statically retained, but runner does not assert `publication.route` |
| Production direct scene render | `Tui.renderDirect` → deferred `RetainedRootBoundary.prepareDesiredInstall` → `setDesiredViewRef` → `flushPendingHosts` | `Tui.render` argument is a non-function Scene | Same retained internal recovery; failed frame keeps previous visible root | No legacy structural fallback found |
| Production canonical render | `Tui.renderCanonical` → root execution scope → deferred root publication → broker flush | `Tui.render` argument is a function | Producer failure restores staged History; frame failure leaves desired revision retryable | T15 does not measure this scope/runtime path |
| ViewSlot replacement | ViewSlot-owned retained boundary and `prepareInstall` | Slot update or animation frame | Stale hint retry; explicit refusal | No second transport; errors are thrown |
| ScrollPane replacement | ScrollPane-owned retained boundary and `prepareInstall` | Pane content update | Stale hint retry; explicit refusal | No second transport; errors are thrown |
| History import/freeze | `tryRetainedMaterializeRef` → generated safe N-API retained materialization | Every View-bearing History operation | Hint/NodeId promotion; explicit refusal | No cold/N-API complete-object fallback in current source |
| Source append | Bun `dlopen` content ABI | Every `TextStreamSource.append` | No alternate N-API payload route found | ABI failure throws; wake result is explicit |
| Content frame | Deferred production host frame with ContentPort connector | `harness.flush()` / production flush | Native content candidate/receipt failures go through runtime error channel | Content benchmark reports frame timing but not complete route receipt details |
| Rust `tui_perf` rendering | Rust `View` factory → Rust scene/layout/paint | Benchmark binary only | Cache hits/misses within Rust layout/paint implementation | Does not measure TS/native retained transport |
| Nominal direct-FFI CI job | Same TypeScript T15 runner as default job | Only addon staging feature differs | None in TypeScript runner | **High risk of false transport comparison**: direct metadata does not select direct calls |

### 5.1 Direct-FFI CI mismatch

The default CI job runs:

```text
T15_TRANSPORT=generated_safe_napi
...
bun run packages/iyon-tui/bench/perf12_t15_authoritative_case.ts
```

The direct-FFI job runs:

```text
ION_NATIVE_FEATURES=direct-ffi bun run packages/iyon-tui/scripts/stage-native.ts
T15_TRANSPORT=feature_gated_direct_ffi
...
bun run packages/iyon-tui/bench/perf12_t15_authoritative_case.ts
```

The current T15 source reads `T15_TRANSPORT` only when serializing result metadata (`perf12_t15_authoritative_case.ts:84–88`). It does not branch on that value, load `bun:ffi`, call `tuiViewAbiBootstrap`, use raw symbols, or select a direct function-pointer table.

Both jobs execute:

```text
nativeViewAbiSession()
→ native.tuiViewAbiSession()
→ generated safe N-API runtime methods
```

The direct feature adds raw exports and probe methods to the addon, but the benchmark does not call them. Thus the CI “direct-ffi” T15 invocation is not a direct-FFI measurement. It is the same retained generated safe N-API benchmark against an addon compiled with extra direct symbols.

### 5.2 No complete-object fallback in current source

The retained source-level comments are unusually explicit:

- `retained-dag.ts:19–22`: retained is the single production structural architecture; refusal fails explicitly.
- `retained-dag.ts:188–191`: `RetainedRefusalError` is not a route selector.
- `retained-dag.ts:416–418`: stale recovery has no secondary transport.
- `native-view-abi.ts:151–157`: retained refusal causes explicit caller failure; no secondary transport.
- `runtime.ts:217–222`: retained refusal fails explicitly at the root boundary.

A repository search over current TypeScript source found no `nodeForBridge`, `NativeTuiHost.render(Object)`, `tryNativeMaterialize`, complete-object decoder, or old cold structural fallback entrypoint.

This is a source fact, not a runtime assertion: a future code change could reintroduce a fallback without the current benchmark detecting it unless route assertions are strengthened.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 T15 warmup and measurement boundaries

T15 performs:

- Initial render before warmup.
- Configurable warmup, default `50`.
- Counter reset after warmup.
- Configurable measured samples, default `1,000`.
- Semantic construction and transport/host phase separation.
- Bootstrap confidence interval over total samples.
- Retained identity counter snapshot after measurement.
- Screen readback after measurement.

The phase instrumentation is installed only around the measured loop (`perf12_t15_authoritative_case.ts:61–81`). It records timestamps around the retained boundary's internal preparation/materialization/host commit hooks. It cannot observe work that the runner omits, such as canonical composition evaluation or environment broker receipt handling.

### 6.2 Retained caches

Current retained structural acceleration includes:

- `SEMANTIC_NATIVE`: weak semantic-node → generation/native-ref hint.
- Transaction-local `refs`.
- Native runtime semantic cache keyed by NodeId.
- Native `NativeRef` paged slots.
- Native NodeId reference map.
- Reusable axis typed-array scratch.
- Reusable grid/diff word scratch.
- Reusable byte scratch for text/diff.
- Generation-scoped style reference sidecar.
- Native path, builder, and edit transaction tables.

These caches are visible in `retained-dag.ts:52–103` and native `view_abi.rs:133–142`, `357–396`.

T15's exact-identity and shared-path modes are designed to exercise these retained cutoffs. However, the benchmark reports retained counter deltas rather than a route-neutral cache identity record from the native host. It cannot prove that a different transport did not participate except by static source inspection.

### 6.3 Rust counter behavior

`perf.rs` uses atomics in production feature builds and a thread-local test enable gate under `cfg(test)` (`perf.rs:137–165`). The comments describe counter increments as cheap, but this is still distinct from a timing build:

- `perf-counters` builds include atomic counter operations.
- Default builds compile counter functions to no-ops.
- The Rust benchmark binary itself requires `perf-counters`.
- TypeScript qualification scripts may run with a default addon and therefore see no Rust counters.

Consequently, timing and counter profiles must not be conflated. A counter-enabled addon is a different binary/profile from the default addon.

### 6.4 Content benchmark timing boundary

`perf13_h_content.ts` resets wake-broker counters before creating the content port and performing appends, but it does not reset or read Rust `perf` counters. Its `append_ns` includes:

- JavaScript string construction.
- UTF-8 encoding.
- Annotation encoding if present (not used in this benchmark).
- Direct C ABI call.
- Result decoding.
- Wake request scheduling.

Its `frame_ns` includes the subsequent host/content flush and any frame-time semantic projection, measurement, layout, and paint. It does not expose separate Source mutation, projection, measurement, paint, or native receipt timing.

### 6.5 Rust benchmark cache semantics

The Rust benchmark deliberately reuses a `LayoutCache` across iterations and, in paint-gate cases, a `PaintCache`. This makes it a cache-sensitive in-process layout/paint benchmark:

- `cache.begin_epoch()` is called per iteration.
- Paint warmup is performed before measured samples.
- `paint_cache.begin_epoch(&Theme::default())` is called during measured iterations.
- `perf::reset()` occurs after fixture construction/warmup.

The benchmark is therefore not a cold-only measurement. Its labels `FRESH`, `IDENTICAL_IDENTITY`, `SHARED_PATH`, and `REBUILT_EQUIVALENT` describe Rust `View` fixture behavior, not TypeScript retained transport routes.

---

## 7. Tests, benchmarks and observability

### 7.1 Route assertion matrix

| Suite/file | Assertion present | What it proves | What it does not prove |
|---|---|---|---|
| `bench/perf12_t15_authoritative_case.ts` | Checks native Host/session availability; checks preparation is not `undefined`; records retained counters and phases | Input was accepted by current retained boundary and produced a frame | Does not assert `publication.route`; does not prove raw direct-FFI vs generated N-API; does not measure `Tui.render` |
| `tests/fixtures/native-host.ts` | `tryRetainedMaterializeRef` must return a ref; generated `hostRenderRef` must return zero | Test reference uses retained materialization and generated host-ref call | Test helper is not production runtime; no route enum assertion |
| `tests/tui_h3_c_transport.test.ts` | `direct_materializer_calls > 0`; derivation fast path exactly one; wide children visited zero; screen output probes | Retained materialization/derivation behavior for selected shapes | Does not assert no alternate route exists; does not distinguish generated safe N-API from raw direct FFI |
| `tests/tui_perf13_b.test.ts` | `direct_materializer_calls > 0` for selected styled state case | State-bearing View reached retained materializer | Does not assert all state cases use the same route |
| `tests/tui_native_scalar.test.ts` | Compares production `Tui` screen with retained reference screen | Output parity between TUI route and retained reference | Does not instrument the production route directly |
| `tests/tui_native_builder.test.ts` | Calls retained builder helpers and compares output with retained reference | Native builder/retained parity | Does not assert production route selection |
| `tests/tui_native_transaction.test.ts` | Calls retained edit transaction and compares output | Edit transaction semantics/parity | Does not establish that production `Tui.render` selects edit transaction |
| Rust `scene/host.rs` tests | Bounds resolver/measure/paint/component counters for localized updates | Local invalidation and paint work in Rust host | Counters are zero when feature disabled; no structural TS transport route identity |
| Rust `presentation/layout/tests/mod.rs` | Paint pruning and layout cache counter assertions | Rust layout/paint pruning behavior | Not a TypeScript/native end-to-end benchmark |
| `bench/pre-v5-l1-trace.ts` | Throws on structural refusal; optional Rust counter availability | Structural/state/content trace can fail loudly on refusal | Does not assert route enum; default addon can produce `rust_counters: null` |
| Ignored `l13_content_lanes.ts` | Optional required mode asserts Source snapshots and semantic preparations | Content work occurred in the expected broad lane | Default timing mode permits absent counters; no route identity |
| Ignored `l13_source_workload.ts` | Optional required mode asserts snapshot count and semantic preparation; validates byte accounting | Source append/replace/truncate behavior and broad content work | Not tracked baseline; no structural route identity |
| Ignored `l13_view_state_workload.ts` | Required mode asserts accepted/no-op/invalidation/paint/relayout counts | State invalidation behavior in selected mounted/unmounted modes | Not tracked baseline; no physical route marker |

### 7.2 Missing observability

The current instrumentation does not provide a single route ledger capable of asserting:

```text
generated_safe_napi structural calls = N
direct_ffi structural calls = 0
cold/complete-object fallback calls = 0
retained refusal = 0
stale recovery = expected
host frame commits = N
```

The closest available data is split across:

- JavaScript retained counters.
- Rust `perf` counters.
- Native runtime memory snapshots.
- Wake-broker counters.
- Phase instrumentation.
- Result metadata supplied by environment variables.

None of these independently establishes the transport selected by the loaded addon. In particular:

- `T15_TRANSPORT` is metadata, not selection.
- `publication.route` is available but ignored by T15.
- `direct_materializer_calls` does not identify ABI lowering.
- Native memory snapshots identify retained objects/caches but not call transport.
- `tuiPerfSnapshot` is optional and absent in default builds.
- Rust `NapiView*` counters appear unused in current production source.

### 7.3 CI evidence

`.github/workflows/ci.yml` runs:

- Rust all-feature tests.
- TypeScript typecheck/lint/declaration checks.
- Native staging.
- Binding and ownership checks.
- Bun tests.
- One small default T15 smoke (`plain_text`, `shared_path`, size 20, warmup 2, measured 20).
- One nominal direct-FFI T15 smoke under `ION_NATIVE_FEATURES=direct-ffi`.

The default and direct T15 smoke commands invoke the same TypeScript source. The direct job verifies that raw direct symbols are present in the staged addon (`stage-native.ts:78–121`), but the T15 runner does not call them. Therefore the two CI T15 jobs do not form a valid generated-N-API-versus-direct-FFI benchmark comparison.

### 7.4 Historical benchmark artifacts

The committed artifacts include labels such as:

- `candidate: "napi_default"`
- `candidate: "direct_ffi_oracle"`
- `transport: "generated_safe_napi"`
- `transport: "feature_gated_direct_ffi"`

Those labels are useful provenance fields, but they are not route proof by themselves. The current source path must be checked against them.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Historical T15 matrix versus current runner

`docs/history/PERF-12/PERF-12-T15-AUTHORITATIVE-REPORT.md` describes:

- 311 cases per arm.
- 622 total records.
- `napi_default` and `direct_ffi_oracle`.
- A full heterogeneous matrix.
- A geometric mean ratio of approximately 1.0421.

The committed summary artifact `packages/iyon-tui/bench/PERF-12-T15-authoritative-019a048b7c6f.json` repeats those claims.

However:

- The result source SHA is `019a048b7c6fa8e6cc6c1e4f0e6635dc78c1b0b7`, not the requested `4355c02...`.
- The current tracked T15 runner is a single-case environment-configured script, not a 311-case orchestrator.
- The current runner always imports `nativeViewAbiSession` and generated safe N-API wrappers.
- The nominal direct-FFI CI path only changes addon build features and output metadata.

Therefore the historical 622-record result cannot be used as proof that the current baseline measured two distinct structural transports. It is historical evidence about an earlier qualification harness/source state.

### 8.2 Historical production-boundary trace versus current source

`docs/history/PERF-12/PERF-12-production-boundary-trace.md` describes an older route cascade involving:

```text
render_ref
→ scalar patch
→ path scalar
→ structural
→ edit transaction
→ text-create
→ cold
→ boundary-create
→ Direct decode fallback
```

That document also describes cold materialization and N-API fallback behavior in the pre-T13 architecture.

Current source explicitly supersedes that route shape:

- `runtime.ts:387–389` says the pre-T13 recipe cascade is gone.
- `retained-dag.ts:19–22` says retained is the single production structural architecture.
- `retained-dag.ts:188–191` says retained refusal is not a route selector.
- Current callers throw on refusal rather than selecting a complete-object decoder.

This is a genuine historical/current contradiction that should remain explicit. The historical document is not evidence that the current baseline still executes those fallbacks.

### 8.3 Direct FFI remains buildable but is not benchmark-selected

The historical handoff says the default addon should expose generated safe N-API while the direct-FFI arm remains feature-gated and buildable. Current source follows that packaging rule:

- Default staging rejects raw direct symbols.
- Direct feature staging requires raw symbols and qualification exports.
- The TypeScript structural transport still uses the N-API session abstraction.

This is consistent with the current source, but it means “direct-FFI remains buildable” must not be conflated with “current T15 benchmark measured direct FFI.”

### 8.4 Rust benchmark naming is misleading for end-to-end interpretation

`perf_bench.rs` calls its records `"baseline"` and its paint gate `"after_perf9"`, but the benchmark is not a current TypeScript/native transport comparison. It exercises Rust factories, Rust layout caches, Rust painters, and Rust History projection. A result from `tui_perf` should be described as:

```text
Rust in-process semantic/layout/paint oracle
```

rather than:

```text
production TUI render benchmark
```

unless the caller explicitly limits the claim to the Rust subsystem.

### 8.5 Content data path is correctly separate

The current Source content benchmark's use of `bun:ffi` is not evidence of a structural fallback. It is the intended content-data ABI path:

```text
Source append → direct content C ABI → wake broker → content frame
```

Structural content-host creation still uses the retained generated N-API path. Any integrated performance statement must preserve this separation.

---

## 9. Open questions and coverage gaps

1. **No current route counter proves generated safe N-API selection.** Static source proves the current T15 runner imports generated wrappers, but runtime route counters do not expose this as an assertion.
2. **No current route counter proves absence of raw direct-FFI calls.** The direct symbols are feature-gated and no TypeScript structural loader calls them, but a future change could alter this without failing T15.
3. **T15 does not assert `RootPublication.route`.** It checks only successful/non-successful preparation. The route field exists but is unused.
4. **T15 does not measure production `Tui.render` canonical-builder mode.** It omits `OwnedBuilderRoot`, tracked subscriptions, producer evaluation, History sideband, and deferred host receipt scheduling.
5. **T15 does not measure production direct-scene mode exactly.** It uses a direct boundary with immediate `hostRenderRef`, while `Tui.renderDirect` uses a deferred boundary and environment flush.
6. **No current benchmark directly measures ViewSlot replacement or ScrollPane replacement.** Their retained boundary implementations are covered by tests, but not by the committed benchmark runners.
7. **No current benchmark directly measures History `pushRef`/`freezeRef` route costs.** The Rust benchmark has History cases, but those are in-process Rust calls and not TypeScript retained-boundary imports.
8. **The committed historical direct-FFI artifacts cannot be source-checked against the requested baseline without the original orchestrator and direct-arm runner.**
9. **The default `perf13_h_content.ts` runner does not read Rust perf counters.** It reports wake counters and Source stats, so projection/paint work must not be inferred from absent fields.
10. **Optional counter modes permit timing-only results without route verification.** The ignored L1 qualification scripts fail only when `*_COUNTER_MODE=required`.
11. **Rust `NapiView*` counters appear declared but unused in current production source.** Their intended historical meaning should not be assumed from names alone.
12. **No benchmark result currently provides a complete per-operation route ledger.** A future integrity gate would need route identity and fallback counts emitted by the actual call layer, not merely metadata supplied by the runner.
13. **No runtime execution was performed during this audit.** The conclusions about actual measured paths are source-derived; historical runtime claims remain historical.

---

## 10. Evidence appendix

### 10.1 Primary current source inspected

#### Benchmark and instrumentation

- `crates/iyon-tui/src/bin/tui_perf.rs`
  - `main`
- `crates/iyon-tui/src/perf_bench.rs`
  - `Workload`
  - `ViewFixture`
  - `build_fixture`
  - `render_view_timed`
  - `run_view_clone_case`
  - `run_view_case`
  - `run_paint_gate`
  - `run_history_case`
  - `run`
- `crates/iyon-tui/src/perf.rs`
  - `Counter`
  - `PerfSnapshot`
  - `reset`
  - `add`
  - `snapshot`
- `crates/iyon-tui/Cargo.toml`
  - `perf-counters`
  - `tui_perf`
  - `required-features`
- `crates/iyon-tui-native/Cargo.toml`
  - `direct-ffi`
  - `perf-counters`
- `crates/iyon-tui-native/src/tui.rs`
  - `tui_perf_reset`
  - `tui_perf_snapshot`
  - direct-FFI qualification exports
- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - `NativeViewAbiSession`
  - `tui_view_abi_session`
  - native runtime/cache structures
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
  - direct-FFI generated exports
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
  - generated safe N-API wrappers
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
  - ABI function metadata

#### TypeScript benchmark runners

- `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts`
- `packages/iyon-tui/bench/perf12_t15_workload.ts`
- `packages/iyon-tui/bench/perf13_h_content.ts`
- `packages/iyon-tui/bench/pre-v5-l1-trace.ts`
- `packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts`
- `packages/iyon-tui/bench/generated/view_abi_cases.ts`

#### Structural/runtime/content seams

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `Tui.render`
  - `renderCanonical`
  - `renderDirect`
  - `prepareRootPublication`
  - `flush`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `RetainedIdentityCounters`
  - `RetainedRefusalError`
  - `MaterializeTx`
  - `ensureNative`
  - `ensureSemanticNative`
  - `materializeWithRecovery`
  - `RetainedRootBoundary`
  - `prepareInstall`
  - `prepareDesiredInstall`
  - `publishPrepared`
  - `renderExact`
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `nativeViewAbiSession`
  - `tryRetainedMaterializeRef`
  - retained edit/build helpers
  - `installNativeRef`
  - `resolveNativeHost`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - generated safe N-API call wrappers
- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeViewAbiHandle`
  - `NativeTuiHostContract`
  - `NativeTuiAddon`
- `packages/iyon-tui/src/transport/content/ffi.ts`
  - `openSession`
  - `invokePayload`
  - `invokeNoPayload`
  - `appendTextSource`
  - `replaceTextSource`
- `packages/iyon-tui/src/api/content/retained.ts`
  - `TextStreamSource.append`
  - `TextStreamSource.replace`
- `packages/iyon-tui/src/runtime/wake-broker.ts`
  - `WakeBrokerCounters`
  - `markPending`
  - `flush`
  - `drain`
  - `consumeReport`
- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness.render`
  - `AppHarness.flush`
- `packages/iyon-tui/scripts/stage-native.ts`
  - native build feature selection
  - direct/default symbol checks
  - direct qualification export checks
- `packages/iyon-tui/package.json`
  - `perf:content`
- root `package.json`
  - `perf:content`
  - test/build scripts
- `.github/workflows/ci.yml`
  - default T15 smoke
  - direct-FFI staging and nominal direct T15 smoke
- `.gitignore`
  - ignored `l13_*` qualification files

#### Route-focused tests

- `packages/iyon-tui/tests/fixtures/native-host.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_perf13_a.test.ts`
- `packages/iyon-tui/tests/tui_perf13_b.test.ts`
- `packages/iyon-tui/tests/tui_perf13_d.test.ts`
- `packages/iyon-tui/tests/tui_perf13_h.test.ts`
- `packages/iyon-tui/tests/tui_native_scalar.test.ts`
- `packages/iyon-tui/tests/tui_native_builder.test.ts`
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
- `packages/iyon-tui/tests/tui_native_persistent_seq.test.ts`
- `packages/iyon-tui/tests/tui_generated_view_abi.test.ts`
- `packages/iyon-tui/tests/tui_values.test.ts`
- `packages/iyon-tui/tests/tui_native_strings.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/presentation/layout/measure.rs`
- `crates/iyon-tui/src/presentation/layout/tests/mod.rs`
- `crates/iyon-tui/src/presentation/paint/view.rs`
- `crates/iyon-tui/src/presentation/ir.rs`
- `crates/iyon-tui/src/application/host.rs`

### 10.2 Schema/build/CI evidence inspected

- `tools/tui-abi/view_abi.toml`
- `tools/tui-abi-gen/src/main.rs`
- `tools/tui-abi-gen/src/render_typescript.rs`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json`
- `packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`

### 10.3 Committed benchmark artifacts indexed

The following result files under `packages/iyon-tui/bench/` were indexed and their provenance/route fields inspected; they were not rerun:

- `PERF-12-T13.1-R6b-frontier.jsonl`
- `PERF-12-T15-authoritative-019a048b7c6f.json`
- `PERF-12-T15-authoritative-019a048b7c6f.jsonl`
- `PERF-12-T15-authoritative-e1fdd93a1a20.json`
- `PERF-12-T15-authoritative-e1fdd93a1a20.jsonl`
- `PERF-12-T15-memory-3a76f5069246.jsonl`
- `PERF-12-T15-memory-3d32b5163962.jsonl`
- `PERF-12-T15-multi-edit-701b68055782.jsonl`
- `PERF-12-T15-multi-edit-7acdc10375e9.jsonl`
- `PERF-12-T15-realistic-6efb3d9216e7.jsonl`
- `PERF-12-T15-realistic-80707ce7d9af.jsonl`
- `PERF-12-s6-napi-dispatch.jsonl`
- `PERF-12-s6-napi-transport.jsonl`

Historical report files inspected:

- `docs/history/PERF-12/PERF-12-T15-AUTHORITATIVE-REPORT.md`
- `docs/history/PERF-12/PERF-12-T15-TRANSPORT-PARITY.md`
- `docs/history/PERF-12/PERF-12-production-boundary-trace.md`
- `docs/history/PERF-12/PERF-12-retained-dag-direct-ffi-handoff.md`
- `docs/history/PERF-12/README.md`
- `docs/history/perf/PERF-11v4-benchmark-report.md`
- `reports/pre-v5-l1/final-implementation-review.md`
- `reports/pre-v5-l1/L1-13-checkpoint.md`
- `reports/pre-v5-l1/perf13-h-content.json`
- `reports/pre-v5-l1/t15-route-smoke.json`
- `reports/pre-v5-l1/trace-default.json`
- `reports/pre-v5-l1/trace-fixed.json`
- `reports/pre-v5-l1/trace-perf-counters.json`

### 10.4 Files merely indexed or excluded from baseline claims

- Ignored local qualification files:
  - `packages/iyon-tui/bench/l13_content_lanes.ts`
  - `packages/iyon-tui/bench/l13_source_workload.ts`
  - `packages/iyon-tui/bench/l13_view_state_workload.ts`
- Build outputs and staged native artifacts under:
  - `target/`
  - `target-*`
  - `packages/iyon-tui/native/`
- Historical `reports/pre-v5-l1/` result JSON and logs were not treated as baseline execution evidence.

### 10.5 LOC methodology

Approximate LOC figures are physical source lines including comments and blank lines, based on the line endpoints exposed by source inspection. Generated files, JSONL artifacts, and historical reports are classified separately rather than counted as production implementation LOC. The benchmark scope is cross-cutting, so no single production/test LOC total represents ownership of the framework; files are listed by benchmark or instrumentation responsibility.

### 10.6 Validation status

No benchmark, test, build, staging, or performance command was executed for this audit. All current-path conclusions are from static source inspection. Historical pass counts and timings remain historical claims tied to the source SHAs embedded in their artifacts.