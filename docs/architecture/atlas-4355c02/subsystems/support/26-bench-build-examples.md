# 26 — Bench/build/examples

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `26`, `support/bench-build-examples`
- Authoritative output artifact:  
  `/Users/alxknt/.pi/agent/sessions/--Users-alxknt-github-iyon-n-iyon-tui--/subagent-artifacts/outputs/b363950e-0ff9-4622-a8f0-491385679af2/subsystems/support/26-bench-build-examples.md`

I first read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `AGENTS.md`
- `PRE-V5-ARCHITECTURE-REPORT.md` in full by section/index review, including its benchmark/instrumentation, alternate-route, benchmark-integrity, and evidence requirements.

This report inventories current source and support architecture only. It does not make V5 disposition decisions.

### Scope covered

Primary inspection covered:

- Workspace and package manifests:
  - `Cargo.toml`
  - `crates/iyon-tui/Cargo.toml`
  - `crates/iyon-tui-native/Cargo.toml`
  - `tools/tui-abi-gen/Cargo.toml` was indexed but not analyzed as an implementation target because assignment 24 owns the ABI generator.
  - root `package.json`
  - `packages/iyon-tui/package.json`
  - `packages/tui-consumer-fixture/package.json`
- Native staging and package artifact loading:
  - `packages/iyon-tui/scripts/stage-native.ts`
  - `packages/iyon-tui/scripts/smoke-native.ts`
  - `packages/iyon-tui/src/transport/native/artifact.ts`
  - `packages/iyon-tui/native/.gitignore`
- Rust benchmark and instrumentation:
  - `crates/iyon-tui/src/perf.rs`
  - `crates/iyon-tui/src/perf_bench.rs`
  - relevant counter call sites
- TypeScript benchmarks and traces:
  - `packages/iyon-tui/bench/*.ts`
  - generated benchmark registry `packages/iyon-tui/bench/generated/view_abi_cases.ts`
- CI:
  - `.github/workflows/agent-fast.yml`
  - `.github/workflows/api-surface.yml`
  - `.github/workflows/ci.yml`
  - `.github/workflows/t1-bun.yml`
  - `.github/workflows/tui-typescript.yml`
- Support tooling excluding the ABI generator:
  - `tools/api-surface/check-binding.ts`
  - `tools/api-surface/check-declaration-closure.ts`
  - `tools/lint/clippy-gate.sh`
  - `tools/ownership/check.ts`
  - associated manifest/mapping paths were indexed
- Examples and reference consumers:
  - `crates/iyon-tui/examples/width_probe.rs`
  - `examples/parrot_test/README.md`
  - the `examples/parrot_test/frame_0000.txt` through `frame_0589.txt` fixture set was indexed rather than printed
  - `packages/tui-consumer-fixture/src/consumer.ts`
- Supporting architecture/build documentation:
  - `crates/iyon-tui/README.md`
  - `iyon-tui.md`
  - `docs/repository-separation/s0/README.md`
  - `docs/repository-separation/s0/test-benchmark-inventory.tsv`
  - `docs/history/perf/PERF-11v4-benchmark-report.md`
  - `reports/pre-v5-l1/post-cleanup-qualification.md`
  - `reports/pre-v5-l1/final-implementation-review.md`

### Static versus executed evidence

This was a read-only static investigation. I did **not** run:

- Cargo builds
- Rust tests
- Bun tests
- ABI generation/checks
- native staging
- native smoke
- benchmarks
- CI workflows
- `nm`
- the TTY width probe

Therefore, all observations below are source-derived or historical-document-derived. No current benchmark result is claimed as executed validation.

The checked-in JSON/JSONL benchmark artifacts contain historical execution metadata and should not be interpreted as measurements of this exact baseline unless their embedded `git_sha`, toolchain, target, and artifact hash match the baseline under investigation. Most do not.

---

## 1. Responsibility and structure

### 1.1 Support/build topology

The repository has no root-level `benches/` directory and no root-level `scripts/` directory. The effective support topology is:

```text
Cargo workspace
├── crates/iyon-tui
│   ├── hidden perf instrumentation: src/perf.rs
│   ├── hidden executable benchmark: src/perf_bench.rs
│   └── interactive example: examples/width_probe.rs
├── crates/iyon-tui-native
└── tools/tui-abi-gen                 [assignment 24; indexed/excluded here]

TypeScript workspace
├── packages/iyon-tui
│   ├── bench/
│   │   ├── current PERF-12/PERF-13 benchmark sources
│   │   ├── pre-V5/L1 trace and lane probes
│   │   ├── generated ABI benchmark registry
│   │   └── historical JSON/JSONL result artifacts
│   ├── scripts/
│   │   ├── stage-native.ts
│   │   └── smoke-native.ts
│   └── native/
│       └── ignored staged iyon-tui-native.node
└── packages/tui-consumer-fixture
    └── public-only external-consumer reference fixture

CI
└── .github/workflows/*.yml
```

### 1.2 Approximate physical LOC

Counts below are approximate physical source lines, including comments and blank lines where visible. Generated code and bulk fixture bodies are separated. No executable line counter was run.

| Area | Files | Approximate physical LOC | Production/test/generated classification | Responsibility |
|---|---|---:|---|---|
| Rust counter core | `crates/iyon-tui/src/perf.rs` | 251 | hidden production instrumentation | Counter enum, names, atomics, snapshots, reset/add/set helpers |
| Rust benchmark driver | `crates/iyon-tui/src/perf_bench.rs` | 557 | hidden benchmark support | Deterministic view/history/paint benchmark fixture generation and JSONL output |
| Rust interactive example | `crates/iyon-tui/examples/width_probe.rs` | 126 | non-CI example | Real-TTY grapheme-width comparison against termwiz |
| TypeScript benchmark sources | 8 source files under `packages/iyon-tui/bench/` | ~980 | benchmark support | PERF-12, PERF-13, L1, structural, state, content, and lane workloads |
| Generated benchmark registry | `bench/generated/view_abi_cases.ts` | 76 | generated | ABI benchmark-case metadata derived from ABI schema |
| Native package scripts | `stage-native.ts`, `smoke-native.ts` | ~144 | build/package support | Build/stage/load/qualify native addon; packaged content smoke |
| TypeScript support tooling | `check-binding.ts`, `check-declaration-closure.ts` | ~443 | repository gates | Rust binding allowlist and public declaration closure |
| Ownership gate | `tools/ownership/check.ts` | 1,547 | repository gate | Rust/TS ownership, dependency direction, public boundary, and historical-cut cleanup checks |
| Clippy gate | `tools/lint/clippy-gate.sh` | 45 | repository gate | Workspace all-target/all-feature Clippy policy |
| CI workflows | 5 workflow files | ~385 | CI configuration | Rust, Bun/N-API, direct FFI, API parity, and platform viability |
| Public consumer fixture | `packages/tui-consumer-fixture/src/consumer.ts` | 170 | reference/test consumer | Public-only third-party-shaped TUI consumer |
| Parrot fixture metadata | `examples/parrot_test/README.md` | 50 | documentation | Capture and replay contract for 590 ANSI frames |
| Parrot frame fixtures | 590 text files | approximately 590 × ~1,108 bytes | bulk fixture | ANSI animation replay; not executable code |

The `tracked-source-manifest.txt` confirms the exact support paths and records the 590 frame files as `examples/parrot_test/frame_0000.txt` through `frame_0589.txt` (`docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt:304-894`), plus the benchmark and script files (`:899-921`), tools (`:1059-1081`), and workflow paths (`:2-6`).

### 1.3 Primary and secondary responsibilities

#### `crates/iyon-tui/src/perf.rs`

Primary responsibility:

- hidden opt-in instrumentation for hot-path work reduction and route observation.

Secondary responsibilities:

- stable machine-readable counter names;
- test-only serialization and counter enablement;
- no-op behavior when `perf-counters` is disabled;
- native binding export of the counter type and snapshot functions through the deliberately unsupported `binding` seam.

The module is explicitly described as benchmark tooling rather than ordinary framework API (`perf.rs:1-5`).

#### `crates/iyon-tui/src/perf_bench.rs`

Primary responsibility:

- hidden `tui_perf` executable benchmark target.

Secondary responsibilities:

- deterministic fixture construction for text, row, column, grid, styled-span, and component workloads;
- benchmark route pattern comparison;
- layout/paint/history cache qualification;
- JSONL provenance output with benchmark name, implementation label, sizes, iterations, percentiles, counters, and Git SHA.

The module is not an application example. Its fixtures are generic and explicitly intended not to contain product-specific data (`perf_bench.rs:1-5`).

#### `packages/iyon-tui/bench/*.ts`

These are independent, script-style benchmark programs rather than a unified benchmark framework.

They fall into five groups:

1. **PERF-12 retained structural route**
   - `perf12_t15_authoritative_case.ts`
   - `perf12_t15_workload.ts`
2. **PERF-13 content**
   - `perf13_h_content.ts`
3. **L1/pre-V5 integrated trace**
   - `pre-v5-l1-trace.ts`
4. **L1 transport/lane and source/state qualification**
   - `pre-v5-l1-text-lanes.ts`
   - `l13_content_lanes.ts`
   - `l13_source_workload.ts`
   - `l13_view_state_workload.ts`
5. **Generated ABI benchmark metadata**
   - `generated/view_abi_cases.ts`

The benchmark scripts generally emit one JSON object to stdout and use environment variables for workload size, warmup, measured count, counter mode, provenance, and target information.

#### Native scripts

`stage-native.ts` is both build script and package qualification gate. It does more than copy Cargo output:

- determines platform/architecture;
- resolves feature list from `ION_NATIVE_FEATURES`;
- uses a feature-suffixed Cargo target directory;
- builds `iyon-tui-native` in release mode;
- verifies the platform-specific output exists;
- copies it to the package-local `.node` path;
- loads the addon;
- verifies removed native classes/methods are absent;
- verifies build ID and smoke marker;
- verifies content ABI resolves the same artifact;
- inspects exported symbols on non-Windows targets;
- checks default versus `direct-ffi` export surface.

`smoke-native.ts` is a minimal packaged-content consumer. It creates a `TextStreamSource`, connects a plain `TextFunnel`, renders `View.content(port)` through `AppHarness`, appends text, flushes, and checks screen rows for the payload.

#### Workflows

The workflows intentionally duplicate some checks:

- `ci.yml` is the broad Rust/Bun/direct-FFI pipeline.
- `agent-fast.yml` is a fast branch validation pipeline.
- `api-surface.yml` is a path-filtered parity gate.
- `t1-bun.yml` is standalone native viability across Linux x64 and macOS arm64.
- `tui-typescript.yml` is a path-filtered TypeScript surface gate.

The duplication gives multiple entry points for the same public/boundary checks, but it also means benchmark/build contract coverage is spread across several YAML files rather than represented by one canonical pipeline definition.

---

## 2. Types, APIs and contracts

### 2.1 Cargo workspace and feature contracts

Root `Cargo.toml` defines a three-member workspace:

```text
crates/iyon-tui
crates/iyon-tui-native
tools/tui-abi-gen
```

The workspace uses edition 2024, resolver 3, shared dependency versions, and workspace Clippy lint policy (`Cargo.toml:1-58`).

#### Core crate features

`crates/iyon-tui/Cargo.toml:11-23` declares:

| Feature | Meaning |
|---|---|
| `test-util` | In-tree/native test hooks; does not restore a public Rust testing facade |
| `native-host` | Enables native-host integration paths |
| `perf-counters` | Enables instrumentation and the hidden `tui_perf` executable |

The hidden executable is declared as:

```toml
[[bin]]
name = "tui_perf"
path = "src/bin/tui_perf.rs"
required-features = ["perf-counters"]
```

The core crate is `publish = false`, and its comments explicitly state that external authors use the TypeScript `@iyon/tui` surface instead of Rust authoring APIs (`crates/iyon-tui/Cargo.toml:6-18`).

#### Native crate features

`crates/iyon-tui-native/Cargo.toml:8-16` declares:

| Feature | Meaning |
|---|---|
| `perf-counters` | Propagates `iyon-tui/perf-counters` |
| `fast-view-abi` | Enables the fast-view ABI path |
| `direct-ffi` | Enables qualification-only direct ABI symbols |

There is no explicit default feature list, so ordinary builds use the default N-API surface without these optional features unless requested.

The native crate produces both `cdylib` and `rlib` (`Cargo.toml:8-10`) and depends on the core crate with `native-host` enabled (`:18-24`).

### 2.2 Rust instrumentation API

`Counter` is a `#[repr(usize)]` enum with 54 counters (`perf.rs:13-70`). The counter names are separately pinned in the `NAMES` array (`perf.rs:80-135`) and are emitted in canonical order by `PerfSnapshot::iter()` (`perf.rs:167-191`).

The current counter inventory is:

#### Structural/native/route counters

- `view_nodes_constructed_rust`
- `view_clone_calls`
- `napi_view_nodes_seen`
- `napi_view_cache_hits`
- `napi_view_cache_misses`
- `napi_view_string_bytes_copied`
- `resolver_nodes_visited`
- `component_view_calls`
- `component_capability_calls`

#### Layout and paint counters

- `measure_node_calls`
- `text_flow_measure_calls`
- `prepare_node_calls`
- `layout_nodes_emitted`
- `paint_nodes_visited`
- `paint_cells_allocated`
- `paint_cache_hits`
- `paint_cache_misses`
- `surface_cells_composited`
- `component_geometry_nodes_visited`

#### History counters

- `history_units_examined`
- `history_units_measured`
- `history_cached_height_hits`

#### Persistent sequence counters

- `persistent_seq_nodes_allocated`
- `persistent_seq_leaf_clones`
- `persistent_seq_branch_clones`

#### View-state counters

- `view_state_mutations_accepted`
- `view_state_mutations_noop`
- `view_state_presentation_invalidations`
- `view_state_style_state_invalidations`
- `view_state_incremental_paints`
- `view_state_damage_rects`
- `view_state_full_damage_repaints`
- `view_state_geometry_invalidations`
- `view_state_geometry_relayouts`
- `view_state_geometry_local_patches`
- `view_state_geometry_full_repaints`
- `view_state_dirty_propagation_nodes`

#### Content counters

- `decorated_normalized_nodes`
- `source_snapshots_acquired`
- `annotation_records_copied`
- `semantic_preparations`
- `global_cache_clears`
- `content_surface_clones`
- `content_registry_port_scans`
- `text_bytes_copied`
- `semantic_projection_rebuilds`
- `content_dirty_records_marked`
- `content_metric_evaluations`
- `content_metric_changes`
- `content_paint_propagations`
- `content_wake_groups`
- `content_due_connectors`
- `content_candidate_records_prepared`
- `content_path_index_nodes_visited`

The public-ish methods are intentionally narrow:

- `PerfSnapshot::value`
- `PerfSnapshot::iter`
- `reset`
- `inc`
- `add`
- `set`
- `snapshot`

All are hidden behind the feature in operational effect. When the feature is off, `add` and `set` consume their arguments and `snapshot` returns zeros (`perf.rs:194-251`).

The `set` gauge API exists, but a repository search found no current `perf::set` or `crate::perf::set` call site. This is an instrumentation contract that is present but not currently exercised by the inspected code.

For tests with `perf-counters`, the module has:

- a global mutex;
- thread-local `TEST_COUNTERS_ENABLED`;
- `test_lock()` guard;
- automatic disablement when the guard drops (`perf.rs:140-165`).

This prevents test counter races and makes counter assertions deterministic, but it means counters in tests are not equivalent to always-on production counters.

### 2.3 Native package artifact contract

`packages/iyon-tui/src/transport/native/artifact.ts` defines:

```ts
interface NativeArtifactLocation {
  absolutePath: string;
  packageBuildId: string;
  platform: string;
  arch: string;
}
```

The canonical build identity is:

```text
iyon-tui-native/s6
```

Supported artifact names are:

| Platform/arch | Cargo/package library name |
|---|---|
| darwin-arm64 | `libiyon_tui_native.dylib` |
| darwin-x64 | `libiyon_tui_native.dylib` |
| linux-arm64 | `libiyon_tui_native.so` |
| linux-x64 | `libiyon_tui_native.so` |
| win32-x64 | `iyon_tui_native.dll` |

`resolveNativeArtifact()` does not load a raw Cargo dylib/so. It resolves the staged package-local Node addon:

```text
packages/iyon-tui/native/iyon-tui-native.node
```

An optional `ION_TUI_NATIVE_ARTIFACT` environment variable may override the location. Relative overrides are resolved against the repository root, then canonicalized with `realpathSync` (`artifact.ts:32-71`).

The package `.gitignore` ignores everything except the `.gitignore` itself. Therefore the native binary is a generated local/package artifact, not a checked-in source artifact (`packages/iyon-tui/native/.gitignore:1-2`).

### 2.4 Public reference consumer contract

`packages/tui-consumer-fixture/src/consumer.ts` is explicitly designed to look like a third-party consumer:

- imports only `@iyon/tui` and `@iyon/tui/testing`;
- imports no internal transport/runtime files;
- uses no feature flags;
- does not perform manual retained-view memoization;
- does not import plugin/application code (`consumer.ts:1-12`).

The fixture exercises:

- public `View`, `Scene`, `Style`, `Insets`;
- `History`;
- `TextInput`;
- `ViewSlot`;
- `ScrollPane`;
- `AppHarness`;
- `defineView`;
- public `state`;
- keyed view occurrences;
- ordinary declarative vertical composition.

The session lifecycle explicitly disposes controls/history and closes the harness (`consumer.ts:73-99`). The componentized half tracks execution counts for the app, header, and keyed item cards (`consumer.ts:103-170`), making it a reference consumer and an identity/invalidation diagnostic rather than a product application.

### 2.5 Generated benchmark registry contract

`bench/generated/view_abi_cases.ts` is marked generated from `tools/tui-abi/view_abi.toml` (`view_abi_cases.ts:1-3`). It registers 60-ish ABI benchmark cases covering:

- runtime/diagnostic probes;
- render/reference;
- state/content host creation;
- scalar patches;
- axis/grid/row/column constructors;
- builders;
- structural patches;
- path operations;
- edit transactions;
- style atoms;
- cstring/UTF-8/buffer text lanes;
- lifecycle release.

Each case records:

```ts
name
family
hotness
benchmarkRegistration
scalarArgs
hasBuffer
maxBufferBytes
maxInputCount
```

This file is a benchmark metadata consumer of the generated ABI schema, not a benchmark runner by itself.

---

## 3. Dependency and ownership map

### 3.1 Build/package ownership graph

```text
package.json / packages/iyon-tui/package.json
        │
        ├── native:stage / build:tui-native
        │       │
        │       ▼
        │   scripts/stage-native.ts
        │       │
        │       ├── reads ION_NATIVE_FEATURES
        │       ├── selects target${feature-suffix}/
        │       ├── cargo build --release -p iyon-tui-native
        │       ├── validates raw Cargo artifact
        │       ├── copies to packages/iyon-tui/native/iyon-tui-native.node
        │       ├── loads addon and probes version/smoke
        │       ├── checks content ABI and nm symbols
        │       └── validates default/direct-ffi surface
        │
        ▼
packages/iyon-tui/src/transport/native/artifact.ts
        │
        ├── optional ION_TUI_NATIVE_ARTIFACT
        └── staged native/iyon-tui-native.node
                │
                ▼
packages/iyon-tui/src/transport/native/addon.ts
                │
                ├── public TS runtime
                ├── content FFI
                └── structural ABI/session consumers
```

The native build owner is `stage-native.ts`; the native load owner is `artifact.ts` plus `addon.ts`. No transport chooses independently between a raw Cargo shared library and the staged Node addon.

### 3.2 Benchmark ownership graph

```text
Rust core hot paths
    ├── perf::inc/add
    └── perf::snapshot
            │
            ▼
hidden `tui_perf` binary
    ├── deterministic View fixtures
    ├── layout/paint/history execution
    └── JSONL timing + counter output

TypeScript runtime/native path
    ├── retained-dag counters
    ├── wake-broker counters
    ├── Rust native tuiPerf counters when feature available
    └── AppHarness screen readback
            │
            ▼
bench/*.ts
    ├── PERF-12 T15 authoritative structural benchmark
    ├── PERF-13-H content benchmark
    ├── L1 content/source/state qualification
    ├── pre-V5 integrated trace
    └── text transfer lane probe
```

### 3.3 Rust dependency direction

The core crate owns the implementation and counter increments. The native crate reaches the core through `iyon_tui::binding`, not through arbitrary core-root exports. `tools/api-surface/check-binding.ts` statically enforces that every native source reference to `iyon_tui::...` is through `iyon_tui::binding` (`check-binding.ts:4-12`, `68-76`).

The binding allowlist includes counter symbols:

```text
Counter
inc
add
reset
snapshot
```

alongside retained constructors, host operations, styles, geometry, text, history, and content records (`check-binding.ts:16-49`).

The support graph is therefore:

```text
native crate
    └── iyon_tui::binding
            └── private core owners + perf counters
```

The core crate itself is unpublished and deliberately does not expose a supported Rust authoring surface (`crates/iyon-tui/Cargo.toml:6-18`; `iyon-tui.md:3-31`).

### 3.4 Ownership and lifetime

#### Native artifact

- Created by Cargo release build.
- Copied by `stage-native.ts`.
- Owned physically by package-local `native/`.
- Loaded by TypeScript native transport.
- Disposed by process/module lifecycle; no repository source owns a checked-in binary.

#### Native runtime objects

The benchmark scripts instantiate native hosts/sessions and close/dispose them explicitly:

- `perf12_t15_authoritative_case.ts` creates `NativeTuiHost`, `nativeViewAbiSession`, `RetainedRootBoundary`, then closes boundary and host (`:37-42`, `:117-122`).
- `pre-v5-l1-trace.ts` follows the same structural host/session/boundary lifecycle (`:51-72`).
- Content/state benchmarks close `AppHarness`; content scripts also dispose `TextStreamSource`.

#### Counters

- Rust counters are process-global atomics.
- TS retained-DAG counters and wake-broker counters are module-global mutable objects.
- Benchmarks reset counters immediately before measured phases.
- TS benchmark scripts never expose counters as public framework authoring state; they import internal instrumentation directly or inspect feature-gated native methods.

### 3.5 Cross-plane benchmark ownership

The benchmark suite reflects the three-plane architecture:

- Structural plane: retained DAG, Native View ABI, `RetainedRootBoundary`.
- State plane: `ViewState` mutation and flush.
- Content plane: `TextStreamSource`, `TextFunnel`, `ContentPort`, connector activation, source snapshots, parser/semantic preparation, and frame flush.

This separation is visible in `pre-v5-l1-trace.ts`, which runs three distinct phases:

1. structural publication;
2. retained state patch;
3. content append and frame.

---

## 4. Execution paths and state transitions

### 4.1 Native build and staging path

```text
package script
  → stage-native.ts
  → parse ION_NATIVE_FEATURES
  → compute target suffix and CARGO_TARGET_DIR
  → cargo build --release -p iyon-tui-native
  → locate target${suffix}/release/<platform artifact>
  → copy to packages/iyon-tui/native/iyon-tui-native.node
  → require(staged addon)
  → check nativeVersion/tuiSmoke
  → check removed classes/methods
  → check content FFI artifact identity
  → check nm symbols
  → check default/direct-ffi qualification surface
```

Important state transitions:

1. **Pre-build**
   - target directory is selected by sorted feature suffix;
   - default build uses `target/`;
   - `direct-ffi` uses `target-direct-ffi/`;
   - other combinations similarly receive suffixes.

2. **Build**
   - Cargo creates raw platform artifact in `release/`.

3. **Staging**
   - script copies raw artifact to a stable `.node` name independent of platform.

4. **Qualification**
   - addon load, marker/version probe, removed-surface probe, content path identity, and symbol checks.

5. **Consumer use**
   - all transport paths resolve the staged addon through `resolveNativeArtifact()`.

6. **Replacement**
   - re-running stage overwrites the package-local staged addon.

7. **Failure**
   - any nonzero Cargo status, missing artifact, load marker mismatch, content artifact mismatch, symbol mismatch, or feature-surface leak throws and exits nonzero.

The script does not automatically restore or clean a prior staged artifact if a new build fails. A stale prior `.node` may remain physically present after a failed subsequent invocation, although the failing process does not present it as a successful build. This is a lifecycle detail worth preserving in any future packaging automation.

### 4.2 Packaged content smoke path

```text
smoke-native.ts
  → TextStreamSource.create()
  → AppHarness.open({width:32,height:4})
  → harness.contentPort()
  → port.connect(source, TextFunnel.plain())
  → connector.activate()
  → harness.render({body: View.content(port)})
  → source.append("packaged TUI smoke\n")
  → harness.flush()
  → harness.screenRows()
  → require row containing payload
  → harness.close()
  → source.dispose()
```

The smoke validates the packaged native content route, not only addon loading. It does not exercise structural direct FFI, state mutation, input, History, or all content funnel families.

### 4.3 Rust hidden benchmark path

`perf_bench.rs` uses deterministic fixtures:

- node sizes: 20, 200, 2,000, 10,000;
- workloads:
  - `text_heavy`
  - `column_heavy`
  - `row_heavy`
  - `grid_heavy`
  - `styled_span_heavy`
  - `component_heavy`
- patterns:
  - `FRESH`
  - `IDENTICAL_IDENTITY`
  - `SHARED_PATH`
  - `REBUILT_EQUIVALENT`

The normal `run()` sequence is:

```text
view clone cases
  → 6 workloads × 4 sizes × 4 patterns
  → static History 1,000
  → live-tail History 1,001
```

This yields 96 view pattern records plus clone and history records. The optional `PERF_ONLY_PAINT_GATE` mode bypasses the normal set and runs six paint-gate cases:

- text-heavy at 2,000 and 10,000;
- column-heavy at 2,000 and 10,000;
- styled-span-heavy at 2,000 and 10,000.

Each normal record contains:

- benchmark identity;
- implementation label;
- node count;
- source bytes;
- iteration count;
- median, p95, p99;
- complete 54-counter map;
- Git SHA.

The timing boundaries differ by pattern:

- `FRESH` includes fixture construction and rendering.
- `IDENTICAL_IDENTITY` reuses a stable base view.
- `SHARED_PATH` reuses a stable subtree and changes a sibling.
- `REBUILT_EQUIVALENT` reconstructs an equivalent graph.

This is useful route contrast, but timing comparisons must not be interpreted as one uniform operation because the patterns include different amounts of construction.

### 4.4 PERF-12 T15 authoritative route

`perf12_t15_authoritative_case.ts` is explicitly route-strict:

```text
makeT15Scenario
  → RetainedRootBoundary.prepareInstall(view)
  → if undefined: throw "outside retained domain"
  → publication.commit()
  → reset identity counters
  → install phase instrumentation
  → measured scenario.next()
  → retained prepare/materialize/host commit
  → JSON with semantic and phase timings
```

The source comments state that a retained refusal is a benchmark failure, not permission to choose another transport (`perf12_t15_authoritative_case.ts:44-52`). This directly satisfies the benchmark-integrity requirement that an authoritative retained benchmark must not silently become a fallback benchmark.

The benchmark reports separate:

- semantic construction samples;
- retained transport preparation;
- native materialization;
- host commit;
- total samples;
- median/p95/p99 and bootstrap CI;
- structural identity counter delta;
- screen readback.

### 4.5 PERF-12 workload transitions

`perf12_t15_workload.ts` includes:

- exact identity: returns the same `View`;
- rebuilt equivalent: constructs a fresh equivalent tree;
- shared path: changes one branch while retaining another;
- deep shared path at configurable depths;
- large shared subtree cutoff;
- wide axis replacement/splice;
- wide grid cell replacement;
- scalar path patch;
- text metadata patch;
- decoration patch.

The workload factory’s mutable `current` variable owns the current semantic root for wide/path edits and returns each new root to the authoritative benchmark. Structural transport receives operation-specific hints such as axis index, grid coordinates, path steps, or text layout values.

### 4.6 PERF-13-H content path

`perf13_h_content.ts`:

```text
TextStreamSource.create(retention=64 KiB, drop-oldest)
  → AppHarness.open(80×24)
  → ContentPort.connect(source, TextFunnel.plain())
  → connector.activate()
  → render View.content(port)
  → append N lines
  → flush
  → source.stats()
  → wakeBrokerCounterSnapshot()
  → JSON output
```

It measures append and frame separately and reports:

- source revision;
- retained bytes;
- chunk count;
- accepted/copied bytes;
- dropped head bytes;
- wake-broker counters.

It does not require the Rust perf-counters addon and does not assert native route counters.

### 4.7 L1 content lane path

`l13_content_lanes.ts` supports:

- plain;
- Markdown;
- diff;
- ANSI.

It changes source retention behavior for Markdown:

- Markdown uses unbounded source retention because incremental parsing requires an untruncated logical prefix.
- Plain/ANSI/diff use bounded drop-oldest retention.

It optionally requires native perf counters with `CONTENT_COUNTER_MODE=required`. In required mode it asserts at least one source snapshot and semantic preparation. In timing mode, the benchmark emits `"counter_status":"unavailable"` rather than failing.

The benchmark validates semantic output through a screen probe:

- `"paragraph chunk"` for Markdown;
- `"new"` for diff;
- `"red"` for ANSI;
- `"plain chunk"` for plain.

### 4.8 L1 source lifecycle path

`l13_source_workload.ts` measures:

1. many appends;
2. source accounting;
3. retaining multiple snapshots;
4. source replacement;
5. head truncation;
6. frame flush;
7. current-versus-retained snapshot behavior.

It verifies that:

- accepted byte accounting equals expected append bytes;
- copied bytes are at least accepted bytes;
- retained snapshots remain stable after replacement/truncation;
- the current snapshot changes;
- screen output contains replacement content.

This is one of the clearest examples of an executable lifecycle contract in the benchmark suite.

### 4.9 L1 state lifecycle path

`l13_view_state_workload.ts` supports modes:

- `paint`
- `noop`
- `geometry`
- `unmounted`

It supports state target positions:

- first;
- middle;
- last.

It creates many `ViewState` values, optionally mounts them in a vertical tree, applies presentation or geometry mutations, flushes, samples mutation/flush time, and checks visibility.

Required counter assertions distinguish:

- accepted mutations versus no-ops;
- presentation invalidation;
- incremental paints;
- geometry invalidation and relayout;
- visible/offscreen physical work;
- unmounted state mutation without physical paint.

This is a strong semantic qualification harness, not merely a timer.

### 4.10 Pre-V5 integrated trace

`pre-v5-l1-trace.ts` intentionally exercises all three planes in one emitted JSON document:

- structural retained publication;
- state patch and clear;
- content append with annotations;
- screen readback;
- Rust perf counters if available;
- retained identity counters;
- wake-broker counters;
- provenance.

It uses 50 content lines and adds a tag annotation every fifth line. It is a route/provenance trace, not a long-running performance benchmark.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production/support path matrix

| Semantic operation | Current path | Alternate/selection condition | Failure behavior | Route observability |
|---|---|---|---|---|
| Build native addon | `stage-native.ts` → Cargo release | `ION_NATIVE_FEATURES` selects feature profile | nonzero exit/missing artifact throws | build log, expected path |
| Load native addon | `resolveNativeArtifact()` → staged `.node` | optional `ION_TUI_NATIVE_ARTIFACT`; otherwise package-local staged path | unsupported target or absent staged artifact throws | resolved canonical path |
| Validate default addon | stage load/probe/symbol check | no `direct-ffi` in feature list | removed classes/methods or leaked direct symbols fail staging | explicit offender list |
| Validate direct FFI addon | stage load plus qualification export/symbol checks | `ION_NATIVE_FEATURES=direct-ffi` | missing qualification export/symbol fails staging | explicit direct export list |
| Packaged plain content | `TextStreamSource` → `TextFunnel.plain` → ContentPort → AppHarness | smoke script only | missing screen probe throws | screen row probe |
| Structural retained benchmark | `RetainedRootBoundary.prepareInstall` → commit | T15 retained-domain input | refusal throws; no fallback | publication undefined check and phase samples |
| Normal structural production rendering | retained structural route plus existing production behavior | depends on semantic input/domain | benchmark source does not establish all production fallback behavior | T15 does, ordinary scripts generally do not |
| View-state presentation patch | public state → native state handle → flush | mounted/unmounted/no-op mode | benchmark assertion fails on wrong counter/visibility behavior | Rust counters and screen readback |
| Content append | source append → connector wake → flush | active connector and funnel family | source API or flush errors propagate through harness/runtime | wake counters, stats, screen probe |
| Rust benchmark | hidden `tui_perf` target | `perf-counters` feature required | invalid env values panic via `expect`; deterministic fixture assumptions use `expect` | JSONL counters and Git SHA |
| Width measurement | `width_probe` → real TTY CPR | only if stdout is a terminal | exits 1 if no TTY; CPR parse/read errors reported | printed terminal column |
| Parrot replay | shell loop over saved frames | user chooses replay loop | ordinary shell/file errors; no application harness | visual ANSI output only |

### 5.2 Silent fallback and route-integrity observations

The strongest explicit route guard is T15:

```ts
const publication = boundary.prepareInstall(view);
if (publication === undefined) {
  throw new Error("authoritative benchmark refused: input is outside the retained domain");
}
publication.commit();
```

This prevents a benchmark that claims to measure the retained route from silently measuring a legacy or compatibility route.

The other benchmark sources are less strict:

- `perf13_h_content.ts` measures through `AppHarness` and screen output but does not identify a native route.
- `l13_content_lanes.ts` and `l13_source_workload.ts` permit timing mode when native counters are unavailable. They report `counter_status: "unavailable"` rather than failing.
- `pre-v5-l1-trace.ts` feature-detects Rust counters and emits `null` when unavailable.
- `l13_view_state_workload.ts` requires counters only when `STATE_COUNTER_MODE=required`.

Thus benchmark intent is not uniform:

- T15 is route-authoritative.
- L1 qualification is counter-authoritative only in explicit `required` mode.
- ordinary timing mode is a functional/timing probe with optional observability.

### 5.3 Build fallback behavior

The artifact loader does have an explicit candidate fallback:

1. `ION_TUI_NATIVE_ARTIFACT`, if set;
2. package-local `native/iyon-tui-native.node`.

This is a location override, not an implementation-route fallback. It does not choose a raw Cargo library or alternate transport.

The stage script has a separate default/direct-FFI surface check:

- default artifacts must expose the N-API session and must not expose direct qualification exports/symbols;
- direct artifacts must expose the qualification exports and symbols.

This prevents feature leakage between profiles.

### 5.4 Historical benchmark artifacts and unsupported compatibility paths

The checked-in `packages/iyon-tui/bench/PERF-12-T15-*.jsonl` artifacts distinguish:

- `candidate: napi_default`, `transport: generated_safe_napi`;
- `candidate: direct_ffi_oracle`, `transport: feature_gated_direct_ffi`.

The artifacts are useful route evidence because their metadata records the intended candidate and transport, but they are historical records. They include older Git SHAs and native artifact hashes, so they cannot prove current baseline execution.

The historical `docs/history/perf/PERF-11v4-benchmark-report.md` explicitly discusses route diagnostics and timing runs with counters disabled (`:207-214`). It should be treated as historical methodology, not current route status.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Rust counter mechanics

The Rust counters are global atomics:

```rust
static VALUES: [AtomicU64; Counter::COUNT]
```

`reset()` stores zero with `Ordering::Relaxed`; `add()` uses `fetch_add(Ordering::Relaxed)`; `snapshot()` loads every counter with `Ordering::Relaxed` (`perf.rs:137-251`).

The intended use is measurement, not synchronization. There is no counter-based correctness synchronization contract.

Test-only behavior adds a thread-local enabled bit and global mutex, so counter assertions do not race. Outside tests, the feature is global and process-wide.

### 6.2 Rust layout/paint cache benchmark boundaries

`perf_bench.rs` creates one `LayoutCache` and begins a cache epoch for each measured iteration in view cases (`:291-350`). The paint gate adds a `PaintCache`, warms it for three iterations, begins a theme epoch each measured iteration, and reports total versus paint-only timing (`:383-450`).

Important cache/key implications:

- Layout cache retention is represented by the benchmark-owned `LayoutCache`.
- Paint cache retention is represented by the benchmark-owned `PaintCache`.
- Theme epoch is explicitly advanced in the paint gate.
- The benchmark does not expose cache eviction policy; it only reports operation counters.
- The benchmark uses stable `View` identity in `IDENTICAL_IDENTITY` and shared subtree identity in `SHARED_PATH`, so identity-based cache effects are intentional.

### 6.3 History benchmark retention

`run_history_case()` builds:

- static History with 1,000 text units;
- live-tail History with 1,000 static units plus one component unit.

The live-tail loop calls `registry.with_any_mut(handle.id(), |_| {})` before each render to force the component registry’s live path. It separately reports `history_static_1000` and `history_live_tail` (`perf_bench.rs:466-532`).

This is useful because it distinguishes static History projection from a live component tail rather than treating all History content as one route.

### 6.4 TS retained-DAG counters

`RetainedIdentityCounters` in `packages/iyon-tui/src/transport/structural/retained-dag.ts:105-127` tracks:

- retained hint hits/misses;
- NodeId promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast-path calls;
- ref words written;
- byte payload bytes;
- scratch reuse;
- stale ref retries;
- decorated normalization;
- host mutations.

The counters are plain number fields deliberately incremented on already-executing paths with no allocation, atomic operation, or extra scan (`retained-dag.ts:105-110`). They are specifically intended to prove asymptotic behavior independently of timing noise.

The same module exposes phase instrumentation:

```text
transport_prepare_ns
native_materialize_ns
host_commit_ns
```

via `RetainedPhaseInstrumentation` and `setRetainedPhaseInstrumentation()` (`retained-dag.ts:157-175`).

This instrumentation is benchmark-only and is installed around T15’s measured phase.

### 6.5 Wake-broker counters

`packages/iyon-tui/src/runtime/wake-broker.ts:65-76` defines 11 counters:

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

Counters are reset and snapshotted by exported helper functions (`wake-broker.ts:102-107`). They count scheduling and error/commit behavior, not layout cost.

The explicit constants `DEFAULT_FLUSH_BUDGET = 32` and `MAX_EXPLICIT_DRAINS = 64` (`wake-broker.ts:62-64`) show that the scheduler is bounded, but the benchmark scripts do not independently vary these bounds.

### 6.6 Work per append/tick/frame/width

Observed from benchmark source:

| Event | Measured/observed work |
|---|---|
| Source append | Source mutation and byte accounting; append loop timing |
| Connector wake | Wake-broker counters; usually observed at flush rather than separately timed |
| Frame flush | `harness.flush()` timing; screen readback |
| State mutation | Mutation timing separately from flush timing |
| State no-op | Counter assertions require no invalidation/paint/geometry work |
| Width change | No current benchmark in this assignment directly varies width; width is fixed per script except the TTY probe |
| Paint cache warmup | Three warmups in Rust paint gate |
| Structural phase | T15 phase samples separate transport preparation/native materialization/host commit |
| History projection | Static versus live-tail render loops |
| Native dispatch | PERF-12 historical dispatch/transport JSONL artifacts and generated ABI registry |

### 6.7 Memory observation

`l13_content_lanes.ts`, `l13_source_workload.ts`, and `l13_view_state_workload.ts` sample process RSS:

- `rss_start_bytes`
- `rss_peak_bytes`
- `rss_end_bytes`

They do not perform allocator-level accounting.

Historical `reports/pre-v5-l1/post-cleanup-qualification.md` explicitly distinguishes RSS, allocator high-water, reachable allocations, and proven leaks (`:98-119`). It reports accepted macOS arm64 measurements but expressly does not claim cross-platform qualification (`:140-151`). Those are historical acceptance records, not measurements rerun here.

---

## 7. Tests, benchmarks and observability

### 7.1 Benchmark inventory

#### Rust

| Benchmark | Source | Route/workload | Output |
|---|---|---|---|
| View clone | `perf_bench.rs:260-288` | Persistent View clone at 100 and 10,000 nodes | JSONL timing + counters |
| View matrix | `perf_bench.rs:291-362` | 6 workload families × 4 sizes × 4 patterns | JSONL timing + counters |
| Paint gate | `perf_bench.rs:383-464` | Shared-path dirty frame and paint timing | JSONL p95/paint share + counters |
| History | `perf_bench.rs:466-534` | Static 1,000 units versus live tail | JSONL timing + counters |
| Complete runner | `perf_bench.rs:536-557` | Normal or paint-only mode | stdout JSONL |

#### TypeScript

| Script | Scope |
|---|---|
| `perf12_t15_authoritative_case.ts` | Route-strict retained structural benchmark |
| `perf12_t15_workload.ts` | Scenario factory for identity/shared/path/wide/patch workloads |
| `perf13_h_content.ts` | Content append/frame benchmark |
| `pre-v5-l1-trace.ts` | Structural/state/content integrated trace |
| `pre-v5-l1-text-lanes.ts` | CString, UTF-8, and buffer text materialization lanes |
| `l13_content_lanes.ts` | Plain/Markdown/diff/ANSI content qualification |
| `l13_source_workload.ts` | Append/replace/truncate/snapshot lifecycle |
| `l13_view_state_workload.ts` | State paint/no-op/geometry/unmounted qualification |
| `generated/view_abi_cases.ts` | Generated ABI benchmark metadata |

The benchmark source paths are listed exhaustively in the tracked-source manifest (`tracked-source-manifest.txt:912-917`).

### 7.2 Behavioral contracts asserted by benchmarks

The benchmark suite asserts more than speed:

- content screen probe succeeds;
- source accepted/copied byte accounting is consistent;
- retained snapshots remain immutable while owned;
- source replacement/truncation changes only current state;
- no-op state updates do not invalidate or paint;
- mounted visible state updates produce physical paint;
- offscreen state updates avoid physical paint;
- unmounted state updates avoid physical paint;
- geometry changes relayout exactly as expected;
- required native counters are present;
- semantic preparation and source snapshot acquisition occur;
- structural retained refusal is a benchmark failure;
- default/direct-FFI native surfaces do not leak into one another;
- removed native classes/methods remain absent;
- package content can render through the staged addon.

### 7.3 Observability limitations

1. **Optional counter modes**  
   Most TypeScript benchmarks default to timing mode and tolerate unavailable Rust counters. A timing result can therefore be valid as a functional measurement while lacking route qualification.

2. **Screen probes are coarse**  
   Screen readback verifies semantic visibility but does not prove which internal route produced the cells.

3. **T15 is the strongest route assertion**  
   It refuses unsupported retained inputs rather than selecting a fallback. The other scripts do not all enforce this.

4. **No current benchmark command is wired into normal `package.json` except `perf:content`**  
   Root and package scripts expose:
   - `perf:content`
   - native stage/smoke
   - type/lint/test/check gates  
   There is no package script for the T15 authoritative matrix, L1 state/source/content workloads, Rust `tui_perf`, or width probe.

5. **CI does not run the full benchmark suite**  
   `ci.yml` runs a small T15 plain-text shared-path smoke with 2 warmups and 20 measured samples (`ci.yml:83-90`). Direct-FFI runs the matching 20-sample smoke (`ci.yml:104-112`). It does not run the larger checked-in artifact matrices.

6. **Historical artifacts are not current acceptance evidence**  
   `reports/pre-v5-l1/final-implementation-review.md` explicitly labels benchmark fixtures and historical qualification records as retained but not automatically passing evidence (`:617-619`, `:653-658`).

### 7.4 Support gates

#### Binding gate

`check-binding.ts`:

- checks every native Rust import is through `iyon_tui::binding`;
- compares actual binding export set with a blessed set;
- rejects `IntoView`, `Renderer`, and `DiffRenderer`;
- rejects public root re-export leaks;
- requires `publish = false`;
- allows only `binding` and hidden feature-gated `perf_bench` at the core root (`check-binding.ts:68-149`).

#### Declaration closure gate

`check-declaration-closure.ts`:

- emits declarations into a temporary directory;
- traverses reachable declarations;
- rejects private implementation types;
- rejects public declarations importing transport paths;
- checks root/testing declarations exist;
- compiles a generated public-surface nameability probe;
- compiles a testing-surface probe (`check-declaration-closure.ts:160-288`).

#### Ownership gate

`tools/ownership/check.ts` is the broadest support checker. It includes 17 major gate families, including:

- Rust dependency direction;
- TS import direction;
- H2 composition/structural ownership;
- H2 runtime/native/control ownership;
- root cleanup;
- import boundaries;
- H3 composition/transport seams;
- residual architecture cleanup;
- package publication;
- N-API transport;
- external consumer fixture;
- public semantic style/handle/control/output contracts;
- PERF-13-H cleanup;
- runtime/public contract parity;
- public API surface.

The gate ends with a single all-pass message or exits nonzero (`check.ts:1417-1547`).

### 7.5 CI verification matrix

| Workflow | Rust | TypeScript/ABI | Native | Benchmarks |
|---|---|---|---|---|
| `agent-fast.yml` | fmt, check all features, core lib tests | ABI, typecheck, declarations, binding, ownership | default stage and content smoke | none |
| `api-surface.yml` | fmt, check all features, ABI-generator tests | ABI/type/declaration/binding/ownership | default stage | none |
| `ci.yml` | fmt, conditional Clippy, all-feature workspace tests | install, ABI drift, type/lint/declarations/binding/ownership/tests | default stage | T15 20-sample default + direct smoke |
| `t1-bun.yml` | workspace check all features, fmt | ABI drift, type/declaration/binding/ownership/tests | default and direct stage | T15 default/direct smoke |
| `tui-typescript.yml` | Rust fmt component setup | ABI/type/lint/declarations/binding/ownership/tests | default stage | none |

The main CI path detects Rust-impacting changes for Clippy on pull requests by checking:

```text
Cargo.toml
Cargo.lock
clippy.toml
crates
tools/tui-abi-gen
tools/lint
```

(`ci.yml:23-48`). This means changes to benchmark scripts alone do not trigger the Rust Clippy job, although TypeScript workflows may still trigger based on their path filters.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Current package names versus historical documents

Current source uses:

```text
@iyon/tui
packages/iyon-tui
```

Several historical documents and the S0 inventory still refer to:

```text
@iyon/runtime/tui
packages/iyon-runtime
```

Examples:

- `docs/repository-separation/s0/README.md:29-37` describes 7 Cargo packages, 31 TypeScript manifests, `@iyon/runtime/tui`, and 65 files in a runtime benchmark directory.
- `docs/repository-separation/s0/test-benchmark-inventory.tsv` lists many `packages/iyon-runtime/bench/...` paths.
- `docs/history/perf/PERF-11v4-benchmark-report.md` refers to `packages/iyon-runtime/bench/PERF-11v4-route-diagnostics.json` (`:207-214`).
- Current tracked source places those retained benchmark artifacts under `packages/iyon-tui/bench/`.

These documents are historical migration records, not current source manifests. The current tracked-source manifest is authoritative for current path existence.

### 8.2 Historical artifact SHAs differ from current baseline

Checked-in benchmark artifacts embed older source SHAs, for example:

- PERF-12-S6 artifacts use `618ade...`;
- R6b artifacts use `04b984...`;
- T15 artifacts use `019a048...`, `e1fdd...`, `701b...`, `7acdc...`, or other historical commits;
- PERF-13-H historical report uses a captured run rather than the current source baseline.

They are valuable as preserved evidence but cannot be silently treated as current `4355c02` benchmark output.

### 8.3 Build command mismatch

The root `justfile` defines:

```text
release: cargo build --release
build: cargo build
```

This builds the default workspace package set according to Cargo’s normal behavior, but it does not encode the package-specific native staging contract. The actual package artifact process is `bun run native:stage`, which invokes:

```text
cargo build --release -p iyon-tui-native
```

and copies/qualifies the resulting artifact.

Therefore:

- `just release` is a generic Rust workspace build helper;
- `native:stage` is the authoritative packaged-addon build;
- a successful `just release` does not prove a loadable `packages/iyon-tui/native/iyon-tui-native.node` exists.

### 8.4 CI platform support versus local qualification

`artifact.ts` declares five platform/architecture mappings, and `t1-bun.yml` tests only:

- Linux x64;
- macOS arm64.

Historical qualification documentation explicitly says Linux x64, Linux arm64, Darwin x64, and Windows x64 were not locally qualified in the accepted macOS delivery (`post-cleanup-qualification.md:140-151`). This is not necessarily a defect—the CI matrix can provide remote coverage—but no current run was executed during this investigation.

### 8.5 Root support surface versus framework boundary

`iyon-tui.md` and `crates/iyon-tui/README.md` consistently document:

- unpublished Rust implementation crate;
- TypeScript as the supported authoring API;
- `binding` as an unsupported native seam;
- hidden `perf_bench` as benchmark-only;
- no public Rust testing facade.

This agrees with the current manifests and binding checker. The benchmark executable is an intentional root visibility exception, not evidence of a supported Rust authoring surface.

### 8.6 Benchmark route enforcement is uneven

The repository has a strong route-integrity contract in T15, but optional observability elsewhere:

- authoritative T15 refuses retained-route misses;
- L1 scripts can run without native counters in timing mode;
- PERF-13-H uses content functionality and wake counters but does not prove every native path;
- Rust benchmark output always includes a counter map, but with counters disabled that map is all zeros.

A future benchmark reader must inspect `counter_mode`, `counter_status`, `transport`, and `candidate` before comparing numbers.

### 8.7 Generic framework boundary is respected by current examples

No inspected benchmark/example file introduces Iyon product/application concepts into generic framework code. The examples are:

- generic terminal width probing;
- generic ANSI frame replay;
- generic public TUI consumer;
- generic retained/content/state benchmark workloads.

The assignment boundary therefore remains visible: application meaning is absent from the generic benchmark and example layer.

---

## 9. Open questions and coverage gaps

1. **Current exact physical LOC**  
   This report uses approximate physical counts. A dedicated line-count command was not run.

2. **Current benchmark execution status**  
   No benchmark was run against `4355c02`. Existing JSON/JSONL artifacts have historical provenance.

3. **Current generated ABI synchronization**  
   ABI generator implementation and generated-source completeness belong primarily to assignment 24. This report only confirms that generated benchmark metadata exists and is checked by CI drift commands.

4. **Actual current native binary contents**  
   The package-local native artifact is ignored/generated. This inspection did not load or symbol-inspect the binary.

5. **Cross-platform staging behavior**  
   Platform mappings exist for Darwin x64/arm64, Linux x64/arm64, and Windows x64, but only Linux x64 and macOS arm64 appear in the explicit T1 CI matrix.

6. **Artifact cleanup after failed staging**  
   `stage-native.ts` overwrites the staged addon only after successful build and validation steps. It does not explicitly delete an older staged addon before beginning a new build. The behavior of consumers after a failed stage with a stale artifact still present is not covered by the inspected script.

7. **Raw Cargo artifact naming under Windows**  
   The script’s `nativeArtifactName()` table uses `iyon_tui_native.dll` for Windows, while the package-local final loader expects `.node`. The script copies/renames the result, but Windows-specific native staging was not executed.

8. **Benchmark count and artifact line counts**  
   The checked-in JSONL files were indexed by name and metadata samples, not fully parsed for every record count. Bulk output is intentionally not repeated here.

9. **Full benchmark route coverage**  
   T15 route strictness is explicit. Equivalent route assertions for all ordinary runtime benchmark scripts are not established by this assignment.

10. **Width-probe portability**  
    `width_probe.rs` depends on `stty`, CPR responses, and a real terminal. It intentionally is not a CI test. Behavior under Windows terminals and multiplexers was not investigated.

11. **Reference application absence**  
    No full example application or product/reference app exists under `examples/`. The closest consumer is the public-only `packages/tui-consumer-fixture` package.

12. **Historical report reconciliation**  
    Several historical documents preserve old package names and old source paths. The current source manifest provides a path authority, but no automated stale-document checker was found in this assignment’s scope.

13. **No explicit benchmark package command for every source**  
    `package.json` exposes `perf:content`, but not named scripts for T15, source workload, state workload, lane workload, trace, or Rust `tui_perf`.

14. **No benchmark threshold gate in CI**  
    CI checks that one small T15 smoke completes, but it does not compare latency/counter thresholds or enforce a performance budget.

15. **Counters are not schema-versioned independently**  
    Rust counter names are stable in source, but no separate counter-schema version is emitted by `perf_bench.rs` or the TypeScript qualification scripts. Consumers must infer compatibility from Git SHA/source version.

---

## 10. Evidence appendix

### 10.1 Primary source paths inspected

#### Mandatory/context

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `AGENTS.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

#### Manifests and package/build

- `Cargo.toml`
- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui-native/Cargo.toml`
- `package.json`
- `packages/iyon-tui/package.json`
- `packages/tui-consumer-fixture/package.json`
- `justfile`

#### Native/package artifact

- `packages/iyon-tui/scripts/stage-native.ts`
- `packages/iyon-tui/scripts/smoke-native.ts`
- `packages/iyon-tui/src/transport/native/artifact.ts`
- `packages/iyon-tui/native/.gitignore`
- `crates/iyon-tui-native/src/tui.rs` feature/export search
- `crates/iyon-tui-native/src/tui/view_abi.rs` feature/export search
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs` feature search
- `crates/iyon-tui-native/src/generated/view_abi_conformance.rs` feature search

#### Rust instrumentation and benchmark

- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/perf_bench.rs`
- `crates/iyon-tui/examples/width_probe.rs`
- counter call-site search across `crates/iyon-tui/src/**/*.rs`

#### TypeScript benchmarks

- `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts`
- `packages/iyon-tui/bench/perf12_t15_workload.ts`
- `packages/iyon-tui/bench/perf13_h_content.ts`
- `packages/iyon-tui/bench/pre-v5-l1-trace.ts`
- `packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts`
- `packages/iyon-tui/bench/l13_content_lanes.ts`
- `packages/iyon-tui/bench/l13_source_workload.ts`
- `packages/iyon-tui/bench/l13_view_state_workload.ts`
- `packages/iyon-tui/bench/generated/view_abi_cases.ts`

#### TypeScript instrumentation

- `packages/iyon-tui/src/runtime/wake-broker.ts`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`

#### CI

- `.github/workflows/agent-fast.yml`
- `.github/workflows/api-surface.yml`
- `.github/workflows/ci.yml`
- `.github/workflows/t1-bun.yml`
- `.github/workflows/tui-typescript.yml`

#### Support tooling, excluding ABI generator

- `tools/api-surface/check-binding.ts`
- `tools/api-surface/check-declaration-closure.ts`
- `tools/api-surface/mappings/iyon-tui.toml` indexed
- `tools/bun-revision.txt` indexed
- `tools/lint/clippy-gate.sh`
- `tools/ownership/check.ts`
- `tools/ownership/snapshots/iyon-tui-rust-surface.txt` indexed

#### Reference consumers/examples

- `packages/tui-consumer-fixture/src/consumer.ts`
- `packages/tui-consumer-fixture/tests/consumer.test.ts` indexed
- `packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts` indexed
- `examples/parrot_test/README.md`
- `examples/parrot_test/frame_0000.txt` through `frame_0589.txt` indexed, not printed

#### Support documentation

- `crates/iyon-tui/README.md`
- `iyon-tui.md`
- `docs/repository-separation/s0/README.md`
- `docs/repository-separation/s0/test-benchmark-inventory.tsv`
- `docs/history/perf/PERF-11v4-benchmark-report.md`
- `reports/pre-v5-l1/final-implementation-review.md`
- `reports/pre-v5-l1/post-cleanup-qualification.md`

### 10.2 Bulk replay fixture index

`examples/parrot_test/` contains exactly 590 frame files:

```text
frame_0000.txt
frame_0001.txt
...
frame_0589.txt
```

The README states:

- source: live `parrot.live` ANSI stream;
- capture command used a 5-second timeout and 50,000-byte cap;
- delimiter: `ESC [2J ESC [3J ESC [H`;
- each frame is approximately 1,108 bytes;
- replay uses a shell loop with a 0.04-second sleep.

No frame contents are reproduced here, per the assignment instruction to index bulk replay fixtures instead of printing them.

### 10.3 Checked-in benchmark artifact index

Under `packages/iyon-tui/bench/`:

```text
PERF-12-s6-napi-dispatch.jsonl
PERF-12-s6-napi-transport.jsonl
PERF-12-T13.1-R6b-frontier.jsonl
PERF-12-T15-authoritative-019a048b7c6f.json
PERF-12-T15-authoritative-019a048b7c6f.jsonl
PERF-12-T15-authoritative-e1fdd93a1a20.json
PERF-12-T15-authoritative-e1fdd93a1a20.jsonl
PERF-12-T15-memory-3a76f5069246.jsonl
PERF-12-T15-memory-3d32b5163962.jsonl
PERF-12-T15-multi-edit-701b68055782.jsonl
PERF-12-T15-multi-edit-7acdc10375e9.jsonl
PERF-12-T15-realistic-6efb3d9216e7.jsonl
PERF-12-T15-realistic-80707ce7d9af.jsonl
```

Metadata samples confirm that these include historical:

- benchmark version;
- profile;
- candidate;
- transport;
- workload/mode/size;
- source Git SHA;
- Bun version/revision;
- Rust version;
- target;
- native artifact hash;
- warmup/measured counts;
- timing and structural/memory samples.

The JSON summary artifacts report `status: "complete"` and `recommendation: "owner_decision_required"` in at least the inspected T15 summary. They remain historical evidence, not current baseline validation.

### 10.4 Historical-only files indexed but not treated as current validation

- `docs/repository-separation/s0/test-benchmark-inventory.tsv`
- `docs/history/perf/PERF-11v4-benchmark-report.md`
- `reports/pre-v5-l1/perf13-h-content.json`
- `reports/pre-v5-l1/t15-route-smoke.json`
- `reports/pre-v5-l1/trace-default.json`
- `reports/pre-v5-l1/trace-fixed.json`
- `reports/pre-v5-l1/trace-perf-counters.json`
- `reports/pre-v5-l1/post-cleanup-qualification-evidence.json`
- `reports/pre-v5-l1/post-cleanup-qualification.md`
- `reports/pre-v5-l1/final-implementation-review.md`

These records are useful for provenance, methodology, and known acceptance boundaries. They contain source revisions and/or package names that do not necessarily match `4355c02`.

### 10.5 Files indexed but not read in depth because outside assignment ownership

- `tools/tui-abi-gen/Cargo.toml`
- `tools/tui-abi-gen/src/main.rs`
- `tools/tui-abi-gen/src/model.rs`
- `tools/tui-abi-gen/src/render_header.rs`
- `tools/tui-abi-gen/src/render_manifest.rs`
- `tools/tui-abi-gen/src/render_rust.rs`
- `tools/tui-abi-gen/src/render_state.rs`
- `tools/tui-abi-gen/src/render_typescript.rs`
- `tools/tui-abi-gen/src/validate.rs`
- `tools/tui-abi-gen/src/snapshots/*`
- `tools/tui-abi-gen/templates/*`
- `tools/tui-abi/view_abi.toml`
- generated Rust/native ABI bodies owned by assignment 24
- the complete Rust/native/TypeScript test trees owned by assignment 25 and the subsystem scouts

### 10.6 LOC methodology

- Source LOC estimates use the physical line ranges visible during static inspection.
- Generated files are reported separately.
- Bulk replay fixtures are counted by filename range and README declaration rather than reproduced.
- Historical JSON/JSONL output is indexed by filename and sampled metadata, not counted as current production or test LOC.
- No claim is made that approximate counts equal `cloc`/`tokei` output.
- No build, benchmark, or test execution was performed for this report.