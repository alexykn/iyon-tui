# 27 — Rust-side crate/module wiring

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope:
  - `crates/iyon-tui`
  - `crates/iyon-tui-native`
  - `tools/tui-abi-gen`
  - ABI schema and generated Rust artifacts directly required to explain those crates
- Required context read:
  - `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - `docs/architecture/atlas-4355c02/README.md`
  - `PRE-V5-ARCHITECTURE-REPORT.md`
  - `AGENTS.md`

The source is authoritative. The Rust crate documentation explicitly describes `iyon-tui` as an unpublished implementation crate for the TypeScript framework and its in-tree native addon, not as a supported external Rust UI-authoring package (`crates/iyon-tui/src/lib.rs:1-11`; `crates/iyon-tui/Cargo.toml:6-8`).

### Evidence status

This is a static source investigation. I did not run builds, tests, benchmarks, generators, package scripts, native processes, or services. I therefore make no claim that the current source compiles or that any test passes at this baseline.

The report distinguishes:

- **Fact** — directly visible in the current source/manifests.
- **Static inference** — behavior reconstructed from call paths and ownership types.
- **Unknown** — not established without executing the relevant route or inspecting an out-of-scope consumer.

### Scope boundary

The framework boundary in `AGENTS.md` is material:

- Rust owns generic terminal mechanics, retained semantic structure/state/content, native input, layout, paint, scheduling, and generic controls.
- The native crate owns the N-API/direct-FFI boundary and native host wrappers.
- The ABI generator owns schema validation and generated-output production.
- No product-specific agent/application semantics were found or assumed in these crates.

### Approximate physical LOC

No shell LOC command was run. Estimates below are architectural sizing figures derived from source file extents and the tracked source manifest, including blank lines/comments where noted.

| Area | Approximate physical size | Method and qualification |
|---|---:|---|
| `crates/iyon-tui` production Rust | roughly 40,000–45,000 lines | Grouped source-extents estimate across application/content/presentation/scene/history/terminal and supporting modules |
| `crates/iyon-tui` tests | roughly 14,000–18,000 lines | Test modules and test-only sections included separately; exact split is uncertain without `wc` |
| `crates/iyon-tui-native` handwritten production | roughly 10,000–12,000 lines | Dominated by `tui.rs`, `tui/view_abi.rs`, `content_ffi.rs`, and state/theme DTOs |
| `crates/iyon-tui-native` tests | roughly 1,000–2,000 lines | Native unit tests and embedded test sections |
| `crates/iyon-tui-native/src/generated` | roughly 10,000–12,000 lines | Checked-in generated ABI Rust, including `view_abi_exports.rs` |
| `tools/tui-abi-gen` production | roughly 2,500–3,500 lines | Generator implementation modules |
| `tools/tui-abi-gen` tests/snapshots | roughly 500–1,000 lines plus snapshot data | Unit tests and generated snapshot fixture |
| ABI TOML/schema | roughly 5,000+ lines | `tools/tui-abi/view_abi.toml`; this is declarative schema, not Rust LOC |

The largest physical nodes in the wiring graph are:

1. `crates/iyon-tui/src/application/content.rs` — approximately 7,000 lines and the central content-source/connector/port implementation.
2. `crates/iyon-tui/src/application/host.rs` — approximately 2,600 production lines plus substantial embedded tests.
3. `crates/iyon-tui/src/scene/host.rs` — approximately 2,600 production lines plus substantial tests.
4. `crates/iyon-tui-native/src/tui/view_abi.rs` — approximately 6,000+ lines, including native runtime/ref/cache/transaction logic and tests.
5. `crates/iyon-tui-native/src/generated/view_abi_exports.rs` — approximately 5,000 lines of generated direct-FFI wrappers.

These estimates are not executable-code counts and should not be treated as precise maintenance metrics.

---

## 1. Responsibility and structure

### 1.1 Workspace-level crate graph

The workspace has exactly three Cargo members (`Cargo.toml:1-6`):

```text
workspace
├── crates/iyon-tui
├── crates/iyon-tui-native
└── tools/tui-abi-gen
```

Dependency direction:

```text
tools/tui-abi-gen
    ├── reads tools/tui-abi/view_abi.toml
    ├── reads TS kind-code schema
    └── generates checked-in Rust/TS/header/test/manifest outputs

crates/iyon-tui-native
    ├── depends on iyon-tui with feature native-host
    ├── depends on napi/napi-derive and serde
    ├── includes generated Rust ABI modules
    └── exports N-API classes/functions and optional direct-FFI symbols

crates/iyon-tui
    ├── generic retained/runtime implementation
    ├── terminal backend and native input
    ├── structural/state/content retained machinery
    └── narrow `binding` module consumed by iyon-tui-native
```

There is no Cargo dependency from `iyon-tui` to `iyon-tui-native`; the dependency is deliberately one-way from native binding to core. The `binding` module is a source-level cross-crate seam, not a separate Cargo package.

### 1.2 `iyon-tui` crate root and visibility

`crates/iyon-tui/src/lib.rs` declares the core module graph:

| Module | Visibility | Main role |
|---|---|---|
| `application` | `pub(crate)` | Runtime/application kernel, native host, environment, source registry |
| `backend` | private | Native History/scrollback sink |
| `component` | private | Component identity, registry, mount graph, capabilities, ticks |
| `content` | `pub(crate)` | Generic text/diff/content model and renderers |
| `controls` | `pub(crate)` | Generic native controls, currently `TextInput` |
| `geometry` | private | Rectangles, points, sizes, constraints |
| `id` | private | Monotonic ID allocation |
| `interaction` | `pub(crate)` | Keys, focus, routing, component interaction |
| `output` | `pub(crate)` | Typed output handles, routes, event contexts |
| `perf` | private or `pub(crate)` under `perf-counters` | Counter storage and optional exports |
| `physical` | private | Terminal cells, rows, glyphs, surfaces, text metrics |
| `presentation` | `pub(crate)` | Semantic `View`, style API, layout, painting |
| `projection` | `pub(crate)` | Stream projections and smoothing |
| `retained_state` | `pub(crate)` | Sparse state records, effects, geometry/presentation patches |
| `scene` | `pub(crate)` | Scene resolution, retained host, prepared frames |
| `scroll` | `pub(crate)` | Generic scroll pane |
| `stream` | private | Source-rooted offsets and ranges |
| `terminal` | private | Crossterm/Termwiz integration and terminal presentation |
| `text` | private re-export alias | Semantic text IR re-export |
| `theme` | `pub(crate)` | Generic theme/style policy and style atom interning |
| `binding` | public | Deliberately narrow native-binding re-export seam |

The root comments make the intended boundary explicit: semantic construction and retained storage remain private, while `binding` exports only operation-specific native seam types/functions (`lib.rs:1-11`, `lib.rs:112-117`).

The crate root also re-exports many internal symbols as `pub(crate)` short names for use by the kernel and in-crate tests (`lib.rs:55-110`). Those re-exports are not external Rust APIs.

### 1.3 Core subsystem inventory

#### Application/runtime cluster

| Path | Responsibility | Principal reverse consumers |
|---|---|---|
| `application/app.rs` | Generic `App` lifecycle and user-state shell | `application::kernel`, `application::run`, native host |
| `application/context.rs` | `AppCx`, registration/invalidation/timer/output capabilities | app init/update, tests |
| `application/handle.rs` | `AppHandle` control surface | application driver/tests |
| `application/kernel.rs` | Running app state machine; ingress, output, timers, scene host, content/state synchronization | `application::host`, app driver |
| `application/run.rs` | Terminal/app execution loop | application entry paths |
| `application/timer.rs` | Timer registration/deadline handling | kernel |
| `application/input.rs` | Input routing into components | kernel/host |
| `application/host.rs` | `TuiHost`, native wrappers, frame publication, host snapshots, N-API-facing host operations | `binding`, `iyon-tui-native::tui` |
| `application/environment.rs` | Shared environment identity, host registry, wake queue/latch, content-source registry | `host`, `content`, native direct FFI |
| `application/content.rs` | Source storage, projections, connector/port lifecycle, content commit candidates | environment, host, scene, native N-API/direct FFI |
| `application/source_store.rs` | Source/content storage helpers and source identity | application content |
| `application/view_state.rs` | Host wrapper around retained state records | host, binding, native state N-API |

`application/environment.rs` is especially important architecturally. `TuiEnvironment` contains an `Arc<Mutex<EnvironmentInner>>`; `EnvironmentInner` owns host weak references, pending host IDs, queue/latch state, wake epoch, and `ContentSourceRegistry` (`environment.rs:121-177`). It does **not** own semantic `View` trees, retained state, or terminal paint (`environment.rs:1-5`).

#### Structural/presentation cluster

| Path | Responsibility | Principal reverse consumers |
|---|---|---|
| `presentation/ir.rs` | Immutable semantic `View` tree, `ViewKind`, persistent sequences, retained identity/path data | factories, layout, scene, history, components, native ABI |
| `presentation/api/*` | Style, text, grid, and common patch value vocabulary | IR, binding, native ABI |
| `presentation/factory.rs` | Internal constructors/lowering helpers for semantic `View` trees | renderers, controls, host |
| `presentation/layout/*` | Measure/prepare/place/cache/layout tree | scene, paint, controls, native host |
| `presentation/paint/*` | Theme/style resolution and conversion of semantic layout to physical rows | scene/terminal |
| `presentation/content.rs` | Content providers and measurement integration | layout, application content |
| `presentation/wrap.rs` | Unicode/grapheme-aware text wrapping and metrics | paint, text controls |
| `physical/*` | Physical cells, glyph spans, rows, surfaces, text metrics | paint, terminal, native history |
| `geometry/*` | Generic geometric values and constraints | layout, retained state, scene |

The `View` representation is an immutable semantic graph. `presentation/ir.rs` states that construction APIs lower into private owned nodes and that the IR contains no terminal/backend state (`ir.rs:1-4`). `ViewId` is a process-local semantic identity used as a cache/retention key, not as public value semantics (`ir.rs:30-45`). `PersistentSeq` uses `Arc`-backed immutable chunks and copies only the root-to-leaf update path (`ir.rs:82-106`).

#### Retained state and scene cluster

| Path | Responsibility | Principal reverse consumers |
|---|---|---|
| `retained_state/capabilities.rs` | Node-kind/state capability rules | state registry, native state bridge |
| `retained_state/effects.rs` | Geometry/presentation effect classification | state record/registry, scene |
| `retained_state/geometry.rs` | Geometry overrides and effective geometry | measure/layout/scene |
| `retained_state/presentation.rs` | Presentation/style overrides and snapshots | paint/scene/native state |
| `retained_state/record.rs` | One retained state record lifecycle and mutations | registry |
| `retained_state/registry.rs` | Stable state identity, candidate capture/prepare/commit, disposal | scene host, application host |
| `retained_state/occurrence.rs` | State-attached occurrence geometry | layout/paint |
| `retained_state/capture.rs` | Candidate overlays and state-frame capture | host/registry |
| `retained_state/damage.rs` | Damage-region union/touch logic | scene host |
| `scene/resolve.rs` | Semantic `View` traversal into resolved scene data | scene host |
| `scene/resolved.rs` | Resolved nodes and overlay/index structures | layout/paint/interaction |
| `scene/layout.rs` | Scene-to-layout integration | scene host |
| `scene/root.rs` | Root/body/history composition | scene host/application |
| `scene/host.rs` | Candidate preparation, retained commit, incremental invalidation, mount/tick/input/paint coordination | `application::host`, kernel |
| `scene/host.rs` | Durable owner of last committed scene graph and derived frame state | application host |

The scene host is the central coordinator of the core graph. It is not the owner of component instances, state records, or content sources individually; it owns their committed scene reachability and derived frame products.

#### Content/projection/history cluster

| Path | Responsibility | Principal reverse consumers |
|---|---|---|
| `content/text/*` | Semantic text IR, annotations, provenance/origin, Markdown/plain/ANSI/diff projectors, visitors/rewriters, text renderers | application content, presentation factory/layout |
| `content/diff/*` | Diff model and View lowering | binding, text/render paths |
| `content/render.rs` | Generic renderer trait surface | text/diff renderers |
| `projection/*` | Projection values, composition (`Then`), validation, projectors, smoothing | content source/connectors, text projectors, history projection |
| `stream/*` | Source-rooted offsets/ranges | projections, content source |
| `history/*` | Ordered semantic units, live/frozen transitions, layout, native scrollback frontier, history projection | scene root/host, application host, native wrappers |
| `backend/native_history.rs` | Physical History rows/scrollback sink interface | scene/application host |
| `scroll.rs` | Scroll pane component wrapper and follow-end semantics | application host, controls |

`History` owns semantic unit order and semantic layout. Its source fields include unit order, semantic identity, layout cache/revision, separate native display revision, and a native frontier (`history/model.rs:22-40`). It stores `View` values, but native scrollback durability is mediated through the host/native sink rather than being owned solely by the semantic `History`.

#### Interaction/component/output cluster

| Path | Responsibility | Principal reverse consumers |
|---|---|---|
| `component/id.rs` | Component ID allocation | registry/graph |
| `component/registry.rs` | Sole owner of erased component instances and revisioned snapshots | scene resolution/host |
| `component/graph.rs` | Mount graph topology and traversal | mount/tick/scene |
| `component/mount.rs` | Mount reconciliation and lifecycle callbacks | scene host |
| `component/capability.rs` | Component-facing capability context | interaction/component |
| `component/tick.rs` | Tick registrations and deadlines | scene host/application kernel |
| `component/slot.rs` | Present but effectively empty in this baseline | no significant direct implementation path |
| `interaction/*` | Key model, focus state, capability routing, interaction results | terminal/application host/component |
| `output/*` | Caller-defined output handles, route registry, event contexts | components/application kernel/native host |
| `controls/text_input/*` | Unicode-safe buffer/cursor/editing plus component output/presentation | application host, interaction, presentation |

The component registry is the actual owner of erased component values. Handles are identity tokens and do not own component memory. Scene resolution creates derived snapshots/overlays, while mount and tick systems consume them. This creates an intentional split:

```text
ComponentRegistry owns component object memory
SceneHost owns committed semantic reachability/mount graph
MountedComponents owns lifecycle membership
TickScheduler owns due-registration state
```

### 1.4 Native crate inventory

#### Handwritten native files

| Path | Responsibility |
|---|---|
| `src/lib.rs` | Native crate module root; exports `native_version` and `tui_smoke` |
| `src/error.rs` | N-API error/status translation (`NativeError`) |
| `src/sync.rs` | Native version probe |
| `src/tui.rs` | N-API classes/functions, environment registries, host wrappers, high-level history/text-input/content/slot/scroll APIs |
| `src/content_ffi.rs` | PERF-13 direct content-data ABI; borrowed payload validation and source mutation calls |
| `src/tui/theme_dto.rs` | Theme DTO decode and native-to-core theme conversion |
| `src/tui/view_state.rs` | N-API retained state handle and patch operations |
| `src/tui/view_abi.rs` | Native View runtime, semantic cache, lease/ref tables, path refs, builders, edit transactions, structural publication, N-API ABI session |
| `src/generated/*` | Generated Rust types, direct-FFI exports, N-API methods, ABI table, conformance probes, state schema |

`crates/iyon-tui-native/src/lib.rs:8-15` shows that `content_ffi`, `error`, `sync`, and `tui` are private implementation modules; only `native_version` and `tui_smoke` are directly re-exported at the crate root. Most N-API exports are discovered through `#[napi]` on types/functions in private modules.

#### Native N-API wrapper classes

`src/tui.rs` defines these principal wrapper types:

- `NativeHistory` (`tui.rs:299-447`)
- `NativeTextInput` (`tui.rs:450-593`)
- `NativeTuiHost` (`tui.rs:603-1040`)
- `NativeTextSource` (`tui.rs:1090-1234`)
- `NativeContentPort` (`tui.rs:1241-1340`)
- `NativeContentConnector` (`tui.rs:1345-1438`)
- `NativeViewSlot` (`tui.rs:1567-1850`)
- `NativeScrollPane` (`tui.rs:1574-1629`)
- `NativeTuiOutput` (`tui.rs:271-284`)

The wrappers are not independent implementations of the runtime. They hold locks, handles, native identity values, and optionally a host-owned core wrapper. Most operations delegate into `iyon_tui::binding`.

### 1.5 ABI-generator inventory

`tools/tui-abi-gen/src/main.rs:1-7` declares these implementation modules:

| Module | Responsibility |
|---|---|
| `model.rs` | Deserialize ABI TOML and kind-code schema into typed model |
| `validate.rs` | Validate handles, enums, PODs, function lowerings, buffer relationships, state property layout |
| `render_header.rs` | C header output |
| `render_manifest.rs` | Generator/schema metadata, manifest, human-readable reference |
| `render_rust.rs` | Rust types, direct exports, N-API methods, ABI table, conformance and layout tests |
| `render_state.rs` | Rust state schema and TypeScript state envelope generation |
| `render_typescript.rs` | TS ABI bindings, conformance, calls, benchmarks, layout tests |

The generator declares 16 output paths (`main.rs:25-42`). They include:

- Rust generated ABI types/exports/conformance/table/N-API methods
- C header
- TypeScript structural ABI bindings/conformance/calls/manifest
- TS layout tests and benchmark registry
- Rust native ABI tests
- documentation/reference
- Rust and TypeScript retained-state schema/envelope

The generator is therefore a build-time schema compiler, not a runtime dependency of either crate.

---

## 2. Types, APIs and contracts

### 2.1 Intentional Rust-facing API

The core crate has no supported external Rust authoring API in this baseline:

- `iyon-tui` is `publish = false` (`crates/iyon-tui/Cargo.toml:6-8`).
- Most root modules are private or `pub(crate)` (`lib.rs:13-49`).
- `binding` is public only because `iyon-tui-native` must link to it (`lib.rs:112-117`).

The binding surface is a curated set of re-exports (`binding/mod.rs:1-104`). It is organized into:

1. **Structural lane**
   - `View`
   - structural constructors/patches
   - text/grid/track specs
   - `NativeCommonPatch`
   - retained paths and weak views
2. **State lane**
   - geometry and presentation patch/property types
   - `HostViewState`
3. **Content lane**
   - `HostContentSource`
   - source/port/connector/funnel/delivery types
   - `SmoothConfig`
   - semantic text/diff types
4. **Host lane**
   - `TuiEnvironment`
   - `TuiHost`
   - `HostHistory`
   - `HostTextInput`
   - `HostViewSlot`
   - `HostScrollPane`
   - `Output`
   - `Key`, `KeyStroke`, `Modifiers`
5. **Measurement/diagnostic lane**
   - optional performance counter operations
   - style atom interning

The comments explicitly prohibit growing this seam into a fluent View DSL, public renderer ecosystem, or callback route into the hot pipeline (`binding/mod.rs:1-13`).

### 2.2 Core ownership types

#### `View`

`View` is the central structural value:

- Public within the binding seam but backed by private semantic IR.
- Cloneable through an outer `Arc`/immutable-node representation.
- Carries process-local `ViewId` identity.
- Stores semantic flags for component slots, state attachments, and content attachments (`presentation/ir.rs:30-80`).
- Uses persistent immutable sequences for wide child collections (`presentation/ir.rs:82-106`).
- Contains no terminal/backend state (`presentation/ir.rs:1-4`).

Its identity is used by native semantic caches and core layout/retention logic, but identity is not part of user-facing semantic equality.

#### Components

The component model is internally generic:

- `Component: 'static`
- `ComponentHandle<C>` is a typed, non-owning reference token.
- `ComponentRegistry` owns erased component objects.
- `ComponentRevision` tracks mutable access/invalidation.
- `MountGraph`, `MountedComponents`, and `TickScheduler` derive lifecycle/scheduling state.

The component registry is not thread-safe application storage in the abstract; its trait-erased callback/state model is runtime-thread-oriented. The native host serializes access through host locks.

#### Retained state

The retained-state API separates:

- stable state identity
- geometry overrides
- presentation overrides
- style-state overrides
- effect classification
- candidate capture
- prepare/commit
- disposal

`NativeViewState` wraps a host-owned state identity and delegates geometry/presentation/style operations to `HostViewState` (`tui/view_state.rs:26-156`). State handles are not owners of the underlying record; the core `ViewStateRegistry` is.

#### Content

The content bridge distinguishes:

- `HostContentSource`
- `HostContentPort`
- `HostContentConnector`
- `HostContentFunnel`
- `ContentFamily`
- `TextSourceKind`
- `TextFunnelKind`
- `ContentDelivery`
- `ContentMutationResult`
- source/connector status and snapshots

`HostContentSource` owns source-side content storage and revisions. The source registry owns lookup by `(source_id, generation)` within an environment. Ports and connectors refer to source/content identities and are host-owned resources, not independent copies of source data.

#### Environment

`TuiEnvironment` contains:

- process-local environment slot/generation identity
- `Arc<Mutex<EnvironmentInner>>`
- weak references to hosts
- pending host queues/sets
- wake-latch and wake epoch
- shared source registry

The identity is explicit even though generation is currently initialized to `1`; source direct FFI validates both slot and generation (`environment.rs:97-118`, `environment.rs:121-177`).

### 2.3 Native API contracts

#### Host lifecycle

`NativeTuiHost` exposes:

- construction/opening
- desired-view publication
- pending-host flush
- theme/history/state/content resource creation
- text input/view slots/scroll panes
- key/paste routing
- terminal polling
- resize/time advancement
- output polling
- screen/history snapshots
- dispose/exit

The host holds a host pointer representation stable enough for generated ABI calls. `view_abi.rs:35-43` explicitly states that the generated ABI uses an opaque `NativeTuiHost` allocation rather than the movable inner `TuiHost` value.

#### Handle disposal

Native wrappers consistently use an `AtomicBool` or equivalent disposed state:

- `NativeHistory` uses `alive: AtomicBool` (`tui.rs:299-315`).
- `NativeTextInput` uses `alive` and retires its host component on disposal (`tui.rs:450-473`).
- `ensure_alive` returns a coded `ION_DISPOSED_HANDLE` error (`tui.rs:275-284`).
- View slots/scroll panes retire host components rather than immediately forcing registry destruction.

This means wrapper disposal and core object destruction are intentionally separate. A disposed wrapper can leave a host-owned component/source/state object alive until the committed scene and registry no longer retain it.

#### Direct content ABI

`content_ffi.rs` defines fixed-layout records:

- `IyonTuiPerf13AbiMetadataV1`, size 128, alignment 4
- `IyonTuiSourceMutationResultV1`, size 24, alignment 4

The source mutation result carries:

- source revision low/high words
- environment wake epoch low/high words
- schedule-drain flag

(`content_ffi.rs:72-110`).

Payload pointers are borrowed only for the synchronous call. The code checks null/alignment/length limits before forming slices, and explicitly does not retain the pointers (`content_ffi.rs:154-180`).

---

## 3. Dependency and ownership map

### 3.1 Forward module graph

The most consequential forward edges are:

```text
application::host
    ├── application::kernel
    ├── application::environment
    ├── application::content
    ├── application::view_state
    ├── component
    ├── interaction
    ├── output
    ├── history
    ├── retained_state
    ├── scene
    ├── presentation
    ├── physical
    ├── backend
    └── terminal

application::environment
    ├── application::content
    └── application::host (weak host references)

application::content
    ├── content::text
    ├── content::diff
    ├── projection
    ├── stream
    ├── presentation::ContentProvider
    ├── physical rows/measurement
    ├── history/native adapter
    └── environment wake scheduling

scene::host
    ├── scene::resolve/layout/root/resolved
    ├── retained_state
    ├── component
    ├── interaction
    ├── history
    ├── presentation
    ├── terminal
    └── application::content/environment

scene::resolve
    ├── presentation::View / ViewKind
    ├── component registry
    ├── retained-state overlay
    ├── content attachment lookup
    └── history/body composition

presentation::layout
    ├── presentation::ir
    ├── geometry
    ├── physical
    ├── retained-state effective geometry
    └── scene::ResolutionOverlay

presentation::paint
    ├── presentation::ir
    ├── retained-state snapshots
    ├── theme/style
    ├── physical surface
    └── content providers

content::text renderers
    ├── content::text IR
    ├── projection
    ├── stream coordinates
    ├── presentation factory/IR
    ├── theme/style
    ├── layout
    └── physical text metrics
```

The graph contains intentional module-level cycles:

- `presentation::layout` reads scene resolution overlays.
- `scene` creates and consumes presentation-derived structures.
- `application::content` implements presentation content-provider contracts.
- `history` stores `View` values and its projection/host paths refer to scene/component/presentation products.
- `component` capability data is consumed by interaction and scene resolution.

These are not Cargo cycles; they are intra-crate module dependency cycles resolved by Rust module privacy and type-level contracts.

### 3.2 Cross-crate graph

```text
TypeScript package transport
    │
    ├── generated N-API methods
    │       packages/.../generated/view_abi.ts
    │       packages/.../generated/view_calls.ts
    │
    ├── N-API runtime classes
    │       crates/iyon-tui-native/src/tui.rs
    │       crates/iyon-tui-native/src/tui/view_state.rs
    │       crates/iyon-tui-native/src/tui/view_abi.rs
    │
    ├── generated structural ABI
    │       generated/view_abi_napi.rs
    │       generated/view_abi_exports.rs
    │       generated/view_abi_table.rs
    │
    └── core binding seam
            crates/iyon-tui/src/binding/mod.rs
                    │
                    ├── presentation/View/IR/factory
                    ├── retained_state
                    ├── application/TuiHost
                    ├── application/content
                    ├── environment
                    ├── history
                    ├── controls
                    ├── interaction/output
                    └── theme/perf

Direct content payload lane:
TypeScript TypedArray / direct FFI
    → crates/iyon-tui-native/src/content_ffi.rs
    → TuiEnvironment identity lookup
    → ContentSourceRegistry
    → HostContentSource mutation
    → wake epoch/latch
    → environment drain
    → TuiHost/HostInner
    → ContentHostRegistry prepare/commit
    → SceneHost
    → layout/paint/terminal
```

### 3.3 Reverse dependency map

The most important reverse edges are:

| Core owner | Major reverse dependents |
|---|---|
| `presentation::View` | binding, history, components, scene, layout, paint, content renderers, controls, native ABI |
| `application::host::TuiHost` | binding, native `tui.rs`, generated `host_render_ref`/edit transaction methods |
| `application::content::HostContentSource` | environment, host, content FFI, N-API `NativeTextSource` |
| `application::content::ContentHostRegistry` | host frame preparation/commit, scene content providers, connector/port wrappers |
| `retained_state::ViewStateRegistry` | host, scene resolution, layout keys, paint, native `NativeViewState` |
| `scene::SceneHost` | application kernel/host, native host flush, terminal backend |
| `component::ComponentRegistry` | scene resolution, mount/tick, interaction/focus, host wrappers |
| `History` | application host, scene root/host, native `NativeHistory`, native scrollback backend |
| `physical::Surface/PhysicalRow` | presentation paint, terminal presenter, native history sink, host snapshots |
| `binding` | only `iyon-tui-native` in the intended production graph plus in-crate tests |
| generated ABI output | native `view_abi.rs`, N-API module inclusion, TS transport, native tests/headers |

### 3.4 Ownership and destruction map

```text
TuiEnvironment
    owns Arc<Mutex<EnvironmentInner>>
    ├── owns ContentSourceRegistry
    │       └── owns HostContentSource entries
    ├── stores Weak<Mutex<HostInner>> host references
    └── owns pending/wake bookkeeping

NativeTuiHost
    owns N-API wrapper
    └── owns or references Arc<Mutex<HostInner>>
            ├── owns TuiHost/runtime state
            ├── owns SceneHost
            ├── owns ComponentRegistry
            ├── owns ViewStateRegistry
            ├── owns ContentHostRegistry/ports/connectors
            ├── owns terminal/backend state
            └── owns committed frame/products

NativeViewRuntime
    owns semantic View/ref/path/style/builder/edit tables
    ├── owns strong leases for native references
    ├── stores weak semantic cache entries
    ├── owns path/builder/transaction identities
    └── publishes View values into Host/TuiHost

History
    owns ordered semantic units and semantic unit IDs
    └── delegates native scrollback durability to host/native sink

ComponentRegistry
    owns erased component instances
    └── handles are non-owning typed references

ViewStateRegistry
    owns retained state records
    └── NativeViewState handles are non-owning identity wrappers

HostContentPort/Connector
    refer to host-owned content state
    └── disposal/deactivation changes reachability and delivery, not source ownership
```

### 3.5 Identity and lifetime classes

There are multiple unrelated identity systems:

| Identity | Owner | Scope | Reuse/generation |
|---|---|---|---|
| `ViewId` | core semantic IR | process-local semantic node | monotonic, non-reused until exhaustion |
| Native `ViewRef` | `NativeViewRuntime` | environment/runtime | validated positive u32, lease-counted |
| `PathRef` | `NativeViewRuntime` | environment/runtime | interned path table |
| `StyleRef`/`StyleAtomRef` | `NativeViewRuntime` | runtime | runtime table |
| `ComponentId` | core component allocator | process/runtime | monotonic |
| state ID | `ViewStateRegistry`/host identity scheme | host/environment | stable retained state identity |
| content source ID/generation | environment source registry | environment | explicit source generation |
| environment slot/generation | `TuiEnvironment` | process/runtime | explicit identity; current generation starts at 1 |
| host ID | environment host registry | environment/process | bounded u64 allocation |
| History identity/unit ID | core History | process/runtime | separate History-object identity and unit identity |

A major architectural consequence is that the native `ViewRef` is not the same thing as the core `ViewId`. The native runtime maps native references to owned/weak semantic `View` values and uses semantic identity to consult caches (`view_abi.rs:962-1137`).

---

## 4. Execution paths and state transitions

### 4.1 Structural View construction and frame publication

Representative path:

```text
TS semantic authoring
    ↓
generated N-API method or direct ABI method
    ↓
NativeViewRuntime constructor/patch
    ↓
decode/validate enum, ref, buffer, node identity
    ↓
semantic cache lookup
    ├── hit: acquire a lease on cached View
    └── miss: resolve children, build new View, publish semantic identity
    ↓
Native ViewRef returned to TypeScript
    ↓
NativeTuiHost.set_desired_view_ref
    ↓
resolve ViewRef from NativeViewRuntime
    ↓
TuiHost.set_desired_view
    ↓
App/Host desired body revision
    ↓
environment pending-host queue
    ↓
HostInner flush
    ↓
SceneHost candidate resolution
    ↓
state/content/component attachment validation
    ↓
layout/measure/place
    ↓
paint to PhysicalSurface/PhysicalRows
    ↓
terminal presenter or headless snapshot
```

The generated ABI methods are thin schema-derived wrappers. Their substantive implementation is in `view_abi.rs` and core binding functions. Generated exports validate pointers/refs/buffers and then call implementation functions in the enclosing `generated_exports` module (`view_abi_exports.rs:1248-1356` onward).

### 4.2 Native View-runtime publication

`NativeViewRuntime` has several stages:

1. Resolve a reference from the native ref table.
2. Consult semantic identity/cache.
3. Resolve child refs and validate expected kinds.
4. Build or patch a core `View`.
5. Stage publication if the operation is transactional.
6. Install/publish semantic identity and lease.
7. Return `ViewRef` status/result.

The internal methods establish this graph:

- ref allocation and lease handling: `view_abi.rs:962-1027`
- semantic identity consultation: `view_abi.rs:1027-1066`
- semantic publication: `view_abi.rs:1066-1137`
- release/maintenance: `view_abi.rs:1166-1290`
- staged publication/edit transactions: `view_abi.rs:638-891`

The runtime can preserve an old root when host installation fails. This is a consequential ownership boundary: a failed candidate does not automatically destroy the last visible/committed semantic root.

### 4.3 State mutation path

Representative state path:

```text
TypeScript retained-state handle
    ↓
NativeViewState.set_geometry /
set_presentation / set_style_state
    ↓
binding::HostViewState
    ↓
ViewStateRegistry mutation
    ↓
effect classification
    ├── geometry/intrinsic effect
    ├── presentation/paint effect
    └── style-state effect
    ↓
host desired/visible state revision
    ↓
environment wake or host-local pending work
    ↓
SceneHost state candidate capture
    ↓
layout/paint incremental or full path
    ↓
state record commit
```

`NativeViewState` itself does not render or maintain a scene. It converts N-API values into binding-layer state mutations and returns errors for invalid/disposed handles (`tui/view_state.rs:26-156`).

The retained-state registry owns candidate/committed state records. The scene host is responsible for making state identity reachable through the current retained graph and for coordinating candidate state with structural/content candidates.

### 4.4 Content append path

The high-volume direct content path is:

```text
TypedArray/pointer payload
    ↓
iyon_tui_source_append_utf8_v1
    ↓
output buffer validation
payload length/null/alignment/UTF-8/annotation validation
    ↓
content_environment_for_identity(slot, generation)
    ↓
environment source registry lookup(source_id, generation)
    ↓
HostContentSource.append_utf8
    ↓
source revision increment
    ↓
environment wake epoch/latch
    ↓
fixed ABI mutation result
    ↓
separate environment drain
    ↓
ContentHostRegistry / connector projection advancement
    ↓
prepared content candidate
    ↓
HostInner / SceneHost commit
    ↓
measurement/layout/paint/terminal
```

`content_ffi.rs:1-6` explicitly says the direct content lane shares the environment-owned source registry with N-API controls and does not perform projection, layout, paint, or callbacks while the payload call is in progress.

This gives the direct payload call a deliberately narrow lifetime:

- input memory is borrowed for the synchronous call only;
- source storage owns copied/retained content after mutation;
- wake scheduling is represented by an epoch/result flag;
- rendering is deferred to the environment/host drain.

### 4.5 Content control/N-API path

The N-API `NativeTextSource` path is lower volume and object-oriented:

```text
NativeTextSource.new
    ↓
host_environment_for_env
    ↓
TuiEnvironment::new or existing per-Env environment
    ↓
EnvironmentInner::content_sources.create
    ↓
HostContentSource wrapper returned

NativeTextSource.append/replace/clear/seal
    ↓
HostContentSource mutation
    ↓
ContentMutationResult
    ↓
JSON/status conversion
```

The environment registry is keyed by `napi::Env` raw address in `HOST_ENVIRONMENTS` and by environment slot in `CONTENT_ENVIRONMENTS` (`tui.rs:42-50`). An N-API environment cleanup hook removes both host and content registry entries (`tui.rs:122-146`).

This is an important reverse/lifetime edge: native cleanup is not merely wrapper disposal. It tears down the process-local environment registration and invalidates future source identity lookups.

### 4.6 History lifecycle

Representative path:

```text
NativeHistory.new
    ↓
detached core History::new
    ↓
NativeHistory.pushRef
    ↓
resolve ViewRef from NativeViewRuntime
    ↓
History::push
    ├── View with component identity → Live unit
    └── otherwise → Static unit
    ↓
Host attachment
    ├── detached operations update local Mutex<History>
    └── attached operations delegate to HostHistory
    ↓
freeze/discardLive
    ↓
History semantic revision + invalidated unit layout
    ↓
host/history projection/native frontier update
```

`History::push_with_boundary` decides whether the unit is live or static based on component identity (`history/model.rs:72-96`). `freeze` rejects a final view that still contains component identity (`history/model.rs:116-127`). A detached history cannot freeze/discard through host operations (`tui.rs:405-436`).

This produces two modes:

- **Detached mode**: `NativeHistory.state: Mutex<History>` owns semantic history locally.
- **Attached mode**: `NativeHistory.host: HostHistory` delegates into the host’s history integration; the detached state is replaced/transferred via `take_for_host` (`tui.rs:331-342`).

### 4.7 Component slot/animation lifecycle

Native view slots are not a separate scheduler implementation. `HostViewSlot` wraps a regular retained component and uses the core component registry/scheduler.

Path:

```text
NativeViewSlot.new
    ↓
TuiHost.create_view_slot
    ↓
HostViewSlot::new(View)
    ↓
component registry registration
    ↓
ViewKind::ComponentSlot
    ↓
Scene resolution/mount graph
    ↓
component tick registration
    ↓
set_view/set_animation
    ↓
component revision + slot state update
    ↓
tick-driven frame selection
    ↓
scene invalidation/paint
```

The wrapper’s `dispose` retires the component/slot, but component memory remains governed by registry ownership and committed scene reachability.

### 4.8 Input/output path

Representative generic input route:

```text
terminal backend event
    ↓
TerminalEvent / key decoding
    ↓
KeyStroke
    ↓
TuiHost.dispatch_key / application input
    ↓
SceneHost focused component/capability lookup
    ↓
interaction::routing
    ↓
component key/paste capability
    ↓
ComponentCx mutation and/or Output<T>
    ↓
OutputRouter
    ↓
application kernel action queue
    ↓
host output queue / NativeTuiOutput
```

The generic host deliberately emits caller-defined routed outputs. It does not expose terminal events as product/application semantics (`application/host.rs:1-5` and `RoutedOutput` around `host.rs:40-45`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path matrix

| Semantic operation | Primary production path | Alternate route | Selection condition | Failure/recovery behavior |
|---|---|---|---|---|
| Create structural View | generated N-API → `NativeViewRuntime` → binding constructor | direct-FFI generated export | `direct-ffi` feature selects C ABI exports | invalid enum/ref/buffer returns ABI status; no publication on failed validation |
| Patch structural View | `view_abi` patch implementation | staged edit transaction | path depth/operation shape and generated specialization | transaction abort releases temporary leases and leaves prior root |
| Publish host View | `NativeTuiHost.set_desired_view_ref` → `TuiHost` | Rust-internal `set_desired_view(View)` | N-API ref route versus core host route | candidate failure retains previous committed root/frame |
| Append content | direct `content_ffi` source mutation | N-API `NativeTextSource.append` | high-volume direct lane versus control lane | fixed status/result; wake/drain deferred |
| Drain content | `TuiEnvironment::drain_pending` → hosts | explicit `NativeTuiHost.flush_pending_hosts` | environment-wide versus host-local invocation | retry/block/waiting-for-presentation represented in drain report |
| Create source | environment content registry | host convenience wrapper | environment identity available | stale/wrong environment/source errors |
| Mutate state | `NativeViewState` → binding → state registry | core host state APIs | N-API versus internal path | node-kind/capability validation; effect-specific invalidation |
| Attach History | detached local `Mutex<History>` | host `HostHistory` | before/after host attachment | detached freeze/discard rejected; host attachment transfers state |
| View cache lookup | semantic identity/ref table | rebuild on miss | weak/lease entry valid or expired | cache miss triggers reconstruction; stale weak metadata eventually scavenged |
| Terminal output | `SceneHost` → `TerminalBackend` | headless physical snapshot | host `headless` mode | terminal errors are host frame failures; previous committed frame remains authoritative |
| Performance probe | feature-gated counters/ABI probes | no-op/absent symbols | `perf-counters`/`direct-ffi` | featureless builds do not expose these diagnostics |

### 5.2 Cache miss versus failure

The native View runtime distinguishes several conditions:

- **Valid cache miss**: a weak semantic cache entry is absent/expired; operation reconstructs and republishes.
- **Stale native reference**: reference cannot be resolved; operation returns an invalid/cache-miss status and does not silently substitute another View.
- **Invalid base reference**: patch/edit cannot proceed; staged work is aborted.
- **Invalid child reference/kind**: child-specific status detail can identify the failing ordinal.
- **Host installation failure**: the old host root remains committed; newly staged leases/publications are cleaned up.
- **Transaction abort**: temporary builder/edit state and temporary leases are released.
- **Runtime/environment poisoning**: explicit internal/native errors are returned; not masked as cache misses.

The existence of separate status details is visible in `view_abi.rs:49-72` and helper functions around `view_abi.rs:1692-1724`.

### 5.3 Content failure semantics

The direct content ABI maps diagnostics into a fixed status table (`content_ffi.rs:113-138`). Notable categories include:

- invalid argument
- ABI mismatch
- wrong/stale environment
- stale/disposed source
- source in use/sealed
- invalid UTF-8/range
- invalid annotation kind/payload
- payload/retention limits
- runtime poison
- internal invariant/panic

The direct call writes a fixed result record even for many accepted mutation outcomes. Projection/rendering is not attempted inline, which prevents payload mutation from recursively entering layout/paint/callback routes.

`NativeTui` status JSON has an additional compatibility behavior: cleanup errors take precedence over older operating diagnostics when mapping connector status (`tui.rs:53-72`). The underlying Rust status keeps causes distinct, but the TypeScript-facing shape reuses one error lane.

### 5.4 Host frame failures

`TuiEnvironment` stores retry-blocked and waiting-for-presentation host sets (`environment.rs:144-155`). `HostDrainReport` carries:

- whether the wake latch should rearm
- whether presentation is still pending
- attempted host count
- successful commits
- structured host frame errors
- wake epoch

This means “accepted source mutation” and “visible frame committed” are separate states. A source may have a new revision while its host is retry-blocked or waiting for a terminal/presentation boundary.

### 5.5 Absence claims and search scope

Within the inspected Rust/native/tool scope:

- No product-specific agent/assistant/tool/approval state was found.
- No second core Cargo implementation of the native host was found.
- No independent native slot scheduler was found; slots use components.
- No separate native retained View tree was found outside `NativeViewRuntime`; native runtime values eventually become core `View` values.
- No source FFI lifetime extending beyond a synchronous direct-content call was found.
- No direct content FFI call performs layout/paint/callback work inline.

These are bounded absence claims over the source files listed in the evidence appendix, not claims about external packages or future branches.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Native structural caches

`NativeViewRuntime` maintains several distinct tables:

- native reference table
- semantic identity/cache map
- leases/reference counts
- path references
- axis builders
- edit transactions
- style atom/style references
- bounded weak-cache metadata

Relevant methods are grouped in `view_abi.rs:399-891` and `view_abi.rs:891-1290`.

Important properties:

- `ViewRef` is a lease-bearing runtime token.
- Child temporary leases are retained until parent/root publication completes.
- Release-many exists for batched cleanup.
- Weak semantic entries may expire while retained metadata is scavenged later.
- Maintenance is bounded rather than always a full sweep; constants include `SCAVENGE_BATCH_BUDGET` and a metadata-growth threshold (`view_abi.rs:64-68`).
- Persistent sequences avoid copying every unchanged child during axis/grid updates.

The core `ViewId` and native semantic cache identity are related through publication, but they are not interchangeable IDs.

### 6.2 Layout and paint caches

The core presentation path has:

- layout measurement cache
- prepared-node cache
- layout tree/index structures
- text geometry/wrapping caches
- state revision participation in layout keys
- theme/style resolution products
- physical surface rows/cells

The relevant dependency direction is:

```text
View identity + geometry constraints + state revision + content/layout properties
    → measure/prepared/layout cache
    → resolved geometry
    → paint/style/text geometry
    → PhysicalSurface
```

State revisions are therefore not purely semantic metadata; geometry-related state participates in layout cache invalidation, while presentation/style state can take a narrower paint path.

### 6.3 Content caches and projection scheduling

`application/content.rs` contains source revision, projection lineage, projected content, connector/frontier state, source cleanup, and prepared commit machinery.

Core lifecycle methods include:

- `prepare_content_commit` (`content.rs:4673`)
- candidate begin/end/abort (`content.rs:5046-5076`)
- `commit_prepared` (`content.rs:5195`)
- connector advancement (`content.rs:3589`)
- connector deadlines (`content.rs:3698-3702`)
- connector delivery frontiers (`content.rs:3776-3788`)
- source mutation methods (`content.rs:2303-2673`)
- source disposal/retention (`content.rs:2630-2797`)
- registry disposal (`content.rs:5844`)
- provider implementation (`content.rs:6572`)

The expected invalidation pattern is:

```text
source revision changes
    → projection/content candidate becomes dirty
    → source/connector frontier advances
    → environment wake scheduled
    → host drains candidate
    → content commit either succeeds or is aborted
    → layout/paint only sees committed content
```

### 6.4 Environment and host scheduling

`TuiEnvironment` uses:

- pending queue
- pending set
- queued set
- retry-blocked set
- waiting-for-presentation set
- edge-trigger wake latch
- monotonic wake epoch

The queue stores host IDs, not host state; host state remains authoritative behind weak host references (`environment.rs:121-177`). This avoids duplicating host state in the environment and means host cleanup must remove queue/set membership.

The component tick scheduler is separate from environment wake scheduling:

```text
Component tick:
    due component callback
    → component revision/dirty result
    → SceneHost invalidation

Environment wake:
    content/state/host mutation
    → pending host ID
    → host drain/flush
    → frame candidate/commit
```

Native view-slot animation also uses the generic component scheduler rather than an independent environment tick loop.

### 6.5 ABI bounds and hot-path metadata

The ABI schema records per-function:

- family
- hotness
- ownership
- borrow duration
- thread affinity
- allocation possibility
- host mutation
- maximum buffer bytes
- maximum input count
- arity specializations
- benchmark registration
- return type

For example, `view_render_ref` and `host_render_ref` are marked critical in `view_abi.toml:149-204`; axis buffer creation has a 4 MiB buffer cap and 524,288 input cap (`view_abi.toml:461-516`).

These values are not merely documentation: `tui-abi-gen` validates and propagates them into generated Rust/TS/header/manifest outputs.

### 6.6 Performance counters

Performance counters are compiled conditionally:

- core `perf` is `pub(crate)` under `perf-counters`
- native `perf-counters` enables `iyon-tui/perf-counters`
- core `binding` re-exports `Counter`, `add`, `inc`, `reset`, and `snapshot` only under that feature (`binding/mod.rs:96-99`)
- native exposes `tuiPerfReset` and `tuiPerfSnapshot` only under `perf-counters` (`tui.rs:249-263`)
- the package-local `tui_perf` binary requires core `perf-counters` (`crates/iyon-tui/Cargo.toml:20-22`)

No counters were executed in this investigation.

---

## 7. Tests, benchmarks and observability

### 7.1 Core tests

The tracked manifest includes tests for:

- application driver/host/kernel lifecycle
- component registry, mount graph, ticks, capabilities
- content text/document/Markdown/ANSI/diff/provenance
- text-input editing/cursor/output/presentation
- history/scene/root/interaction
- layout flow/grid/style/text
- projection composition/validation/smoothing
- retained state records/registry/damage/capabilities
- physical glyph/row/surface/text metrics
- terminal lower/presenter/shadow behavior
- presentation IR and paint
- native host behavior embedded in application/scene/content modules

Tests are behavioral evidence of contracts but were not run.

### 7.2 Native tests

Important native test locations include:

- `tui.rs:1963-1978` — wrapper/status tests
- `content_ffi.rs:486-501` — direct content FFI acceptance/wake behavior
- `tui/view_state.rs:645` onward — state bridge tests
- `tui/theme_dto.rs:441` onward — theme DTO tests
- `tui/view_abi.rs:4554` onward — runtime/cache/lease/publication/path/edit/buffer/text ABI tests

The View ABI tests explicitly cover:

- native reference table behavior
- semantic cache reuse
- lease counts and root lease transfer
- failed host installation retaining the old root
- failed transaction cleanup
- stale weak cache recovery
- repeated node-ID lookups acquiring independent leases
- path interning and depth-specialized rebuilds
- edit transaction shared ancestry
- malformed axis/grid/text buffers
- semantic-cache-first constructors
- wrong parent/path kinds
- C-string embedded-NUL behavior
- UTF-8 span-boundary and framing validation

This test inventory confirms that the runtime’s primary architecture is not “N-API wrapper calls directly construct and render”; it is lease-managed retained publication with transactional failure handling.

### 7.3 Generator tests

`tools/tui-abi-gen/src/main.rs:309-485` includes tests for:

- canonical schema rendering all outputs
- generated output-path uniqueness
- gapped retained-state property IDs
- unknown state value kinds
- invalid state lane shapes
- state envelope coverage of geometry/presentation lanes
- unknown/incompatible lowerings
- missing buffer-used relationships

The generator’s canonical snapshot test checks that the schema yields the expected manifest and all declared outputs (`main.rs:313-324`).

### 7.4 Observability surfaces

Observed diagnostic/inspection surfaces include:

- `HostEpochs`
- `HostDrainReport`
- `HostFrameError`
- `HostCommit`
- connector status JSON
- source snapshot/stats
- host screen rows/history rows
- cell style lookup
- environment count
- ABI metadata probes
- ABI status-detail side channel
- optional performance counters
- native runtime memory snapshot/maintenance probes

Gaps:

- No single end-to-end counter spans source mutation → projection → layout → paint → terminal write.
- Environment wake epochs expose scheduling state but not complete per-host queue history.
- Native View cache metrics are available through diagnostic/runtime snapshots but were not observed at execution time.
- Direct FFI and N-API routes have different observability shapes; direct content returns fixed records, while N-API returns JS/JSON objects.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Crate-level private versus item-level public

The core crate contains many `pub` item declarations inside `pub(crate)` modules and exposes a public `binding` module. This can look like a public Rust authoring surface if read only at item visibility level.

The stronger boundary is:

```text
Cargo publish = false
module mostly private/pub(crate)
binding = explicitly unsupported native seam
TypeScript facade = intended external authoring boundary
```

Evidence:

- `crates/iyon-tui/Cargo.toml:6-8`
- `crates/iyon-tui/src/lib.rs:1-11`
- `crates/iyon-tui/src/lib.rs:112-117`
- `crates/iyon-tui/src/binding/mod.rs:1-13`

### 8.2 `binding` is broad in symbol count but narrow in architectural purpose

`binding` re-exports a large set of passive types and runtime operations, including View construction, state, content, history, host, controls, and output. It is nevertheless intentionally one-way and operation-specific:

```text
native crate → binding → core runtime
```

There is no callback registration from core into arbitrary TypeScript in the hot structural/content path. Outputs are caller-defined routed values, not product semantics.

### 8.3 Environment owns scheduling, not rendering

`TuiEnvironment` refers to hosts through weak pointers and queues host IDs. It shares source storage and wake state but does not own semantic Views, retained state, or physical paint (`environment.rs:121-177`).

This is a consequential seam:

```text
content mutation can be accepted by Environment/Source
without immediate layout/paint
```

Any consumer treating the mutation result as equivalent to visible presentation would be crossing the wrong lifetime boundary.

### 8.4 Host wrappers versus core registry ownership

Native wrappers are disposable handles, not universal owners:

- `NativeTextInput.dispose` retires a host component.
- `NativeViewSlot.dispose` retires a slot.
- `NativeHistory` can transfer detached state into a host.
- `NativeContentPort`/`NativeContentConnector` control attachment/delivery but do not own the source storage.
- `NativeViewState` refers to a registry-owned retained state record.

Destruction is therefore often deferred until both wrapper reachability and committed scene reachability are gone.

### 8.5 Content module placement versus presentation dependency

`application/content.rs` is semantically generic and serves as a host/runtime content registry, but it depends on:

- semantic text/projectors
- stream coordinates
- presentation content-provider contracts
- physical rows/measurement
- history-native adapters

This is not a product-specific violation. It is a current wiring fact: content delivery and projection are upstream of layout but still expose physical/host commit products to integrate with the existing retained renderer.

### 8.6 Direct-FFI feature naming versus unconditional content FFI

The native crate declares a `direct-ffi` feature. Generated structural direct exports and ABI probes are gated by `#[cfg(feature = "direct-ffi")]` (`generated/view_abi_exports.rs`, `tui/view_abi.rs`, `tui.rs`).

However, `content_ffi.rs` contains its direct `extern "C"` source mutation exports without an observed `#[cfg(feature = "direct-ffi")]` gate (`content_ffi.rs:309-460`). Thus “direct FFI disabled” does not necessarily mean all direct FFI symbols disappear. The feature gates the generated structural/direct ABI lane and probes, while PERF-13-E content symbols appear to be part of the native artifact independently.

This is a concrete configuration distinction that should not be collapsed into one generic “direct FFI” statement.

### 8.7 Generated structural ABI versus handwritten host ABI

The structural View ABI is generated from `tools/tui-abi/view_abi.toml` and emitted into Rust/TS/header/table/test/reference outputs. The high-level N-API host/content/history classes are handwritten in `tui.rs`.

The two layers meet through:

```text
generated N-API structural method
    → NativeViewRuntime
    → core binding
```

and:

```text
handwritten N-API host/content wrapper
    → core binding host/content operation
```

The generator does not generate the entire native runtime. It generates wire/lowering/validation wrappers and schema-driven state/ABI surfaces; semantic implementation remains handwritten.

### 8.8 Stateful three-plane commit coordination

Structural View, retained state, and content each have candidate/commit concepts. `TuiHost`/`HostInner` coordinates them into visible frame publication.

The important coupling is not a shared data structure but a shared commit boundary:

```text
desired structural revision
    + desired retained-state bindings/patches
    + content projection candidate
    → host frame preparation
    → scene/layout/paint
    → visible commit
```

Failure in one candidate can preserve the previous visible frame while source/state revisions remain newer than visible revisions.

### 8.9 Current View identity is used in several roles

`ViewId`/semantic identity participates in:

- core retained semantic identity
- layout/measurement cache keys
- native semantic cache reuse
- weak-cache expiry/scavenging
- structural update publication
- content/state attachment identity propagation

The same identity should not be interpreted as ownership. A cached semantic identity can be weak/expired; a native lease is a separate resource; a host’s committed root is another ownership boundary.

---

## 9. Open questions and coverage gaps

1. **Exact generated-output feature matrix**  
   Static inspection establishes many `direct-ffi`, `fast-view-abi`, and `perf-counters` gates, but the complete artifact symbol matrix was not built under every feature combination.

2. **Native package build defaults**  
   The Cargo manifest has no explicit `[features] default`; the effective package build profile is controlled by package scripts/CI outside this assignment’s execution. The source-level gates are known, but default artifact contents were not executed.

3. **Cross-crate ABI symbol retention**  
   `content_ffi.rs` appears unconditionally compiled, while generated structural exports are direct-FFI gated. The exact linker/export behavior for `cdylib` under each feature set remains unverified.

4. **Environment cleanup timing**  
   The cleanup hook removes environment registries, but actual N-API environment teardown timing was not observed. Whether pending asynchronous drains can race with cleanup requires runtime execution.

5. **Host commit ordering under simultaneous structural/state/content mutations**  
   Source shows candidate preparation/commit machinery, but no executed trace was performed combining all three mutation lanes in one wake cycle.

6. **Terminal backend route selection**  
   Both Crossterm and Termwiz modules are present. This report identifies the module edges but does not establish which backend is selected by every supported runtime/build mode.

7. **Performance counter completeness**  
   Counters are feature-gated and exposed through binding/native probes. No runtime snapshot was captured, so counter names, increments, and route coverage remain source-only.

8. **External consumer reachability**  
   The in-tree TypeScript package and native crate are visible, but this report does not assume consumers outside the repository. Product/plugin packages mentioned in architecture guidance were not treated as present source consumers.

9. **Exact physical LOC**  
   Estimates are approximate source-extents calculations. A reproducible count should be generated separately if exact numbers are required.

10. **Potential stale generated artifacts**  
    `tui-abi-gen` has a `check` command designed to detect stale outputs, but it was not executed. The report therefore describes the checked-in generated files, not their verified equality with current schema output.

11. **History native transfer semantics under host disposal**  
    Detached-to-host transfer is visible, but the complete sequence for host disposal, retained History reachability, and native scrollback retirement needs an executed lifecycle trace.

12. **Thread-affinity validation**  
    The ABI schema records `owner_thread` for functions, and core environment/host use locks/unsafe markers. Static source establishes intended ownership, but no multithreaded runtime validation was performed.

---

## 10. Evidence appendix

### 10.1 Primary baseline and contract files

- `Cargo.toml`
- `Cargo.lock`
- `AGENTS.md`
- `crates/iyon-tui/Cargo.toml`
- `crates/iyon-tui-native/Cargo.toml`
- `tools/tui-abi-gen/Cargo.toml`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

### 10.2 Core crate root and runtime files inspected

- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/application/mod.rs`
- `crates/iyon-tui/src/application/app.rs`
- `crates/iyon-tui/src/application/context.rs`
- `crates/iyon-tui/src/application/environment.rs`
- `crates/iyon-tui/src/application/handle.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/application/input.rs`
- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/application/run.rs`
- `crates/iyon-tui/src/application/source_store.rs`
- `crates/iyon-tui/src/application/timer.rs`
- `crates/iyon-tui/src/application/view_state.rs`
- `crates/iyon-tui/src/binding/mod.rs`
- `crates/iyon-tui/src/backend/mod.rs`
- `crates/iyon-tui/src/backend/native_history.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/perf_bench.rs`

### 10.3 Core structural/presentation/state files inspected

- `crates/iyon-tui/src/presentation/mod.rs`
- `crates/iyon-tui/src/presentation/ir.rs`
- `crates/iyon-tui/src/presentation/factory.rs`
- `crates/iyon-tui/src/presentation/content.rs`
- `crates/iyon-tui/src/presentation/api/mod.rs`
- `crates/iyon-tui/src/presentation/api/grid.rs`
- `crates/iyon-tui/src/presentation/api/style.rs`
- `crates/iyon-tui/src/presentation/api/text.rs`
- `crates/iyon-tui/src/presentation/api/view.rs`
- `crates/iyon-tui/src/presentation/layout/mod.rs`
- `crates/iyon-tui/src/presentation/layout/cache.rs`
- `crates/iyon-tui/src/presentation/layout/engine.rs`
- `crates/iyon-tui/src/presentation/layout/grid.rs`
- `crates/iyon-tui/src/presentation/layout/measure.rs`
- `crates/iyon-tui/src/presentation/layout/place.rs`
- `crates/iyon-tui/src/presentation/layout/prepare.rs`
- `crates/iyon-tui/src/presentation/layout/tracks.rs`
- `crates/iyon-tui/src/presentation/layout/tree.rs`
- `crates/iyon-tui/src/presentation/paint/mod.rs`
- `crates/iyon-tui/src/presentation/paint/decoration.rs`
- `crates/iyon-tui/src/presentation/paint/text.rs`
- `crates/iyon-tui/src/presentation/paint/theme.rs`
- `crates/iyon-tui/src/presentation/paint/view.rs`
- `crates/iyon-tui/src/presentation/wrap.rs`
- `crates/iyon-tui/src/retained_state/mod.rs`
- `crates/iyon-tui/src/retained_state/capabilities.rs`
- `crates/iyon-tui/src/retained_state/capture.rs`
- `crates/iyon-tui/src/retained_state/damage.rs`
- `crates/iyon-tui/src/retained_state/effects.rs`
- `crates/iyon-tui/src/retained_state/geometry.rs`
- `crates/iyon-tui/src/retained_state/occurrence.rs`
- `crates/iyon-tui/src/retained_state/presentation.rs`
- `crates/iyon-tui/src/retained_state/record.rs`
- `crates/iyon-tui/src/retained_state/registry.rs`

### 10.4 Core graph/interaction/history/content files inspected

- `crates/iyon-tui/src/scene/mod.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/scene/layout.rs`
- `crates/iyon-tui/src/scene/resolve.rs`
- `crates/iyon-tui/src/scene/resolved.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/component/mod.rs`
- `crates/iyon-tui/src/component/capability.rs`
- `crates/iyon-tui/src/component/graph.rs`
- `crates/iyon-tui/src/component/id.rs`
- `crates/iyon-tui/src/component/mount.rs`
- `crates/iyon-tui/src/component/registry.rs`
- `crates/iyon-tui/src/component/revision.rs`
- `crates/iyon-tui/src/component/slot.rs`
- `crates/iyon-tui/src/component/tick.rs`
- `crates/iyon-tui/src/interaction/mod.rs`
- `crates/iyon-tui/src/interaction/command.rs`
- `crates/iyon-tui/src/interaction/focus.rs`
- `crates/iyon-tui/src/interaction/key.rs`
- `crates/iyon-tui/src/interaction/result.rs`
- `crates/iyon-tui/src/interaction/routing.rs`
- `crates/iyon-tui/src/output/mod.rs`
- `crates/iyon-tui/src/output/event.rs`
- `crates/iyon-tui/src/output/handle.rs`
- `crates/iyon-tui/src/output/router.rs`
- `crates/iyon-tui/src/history/mod.rs`
- `crates/iyon-tui/src/history/model.rs`
- `crates/iyon-tui/src/history/unit.rs`
- `crates/iyon-tui/src/history/layout.rs`
- `crates/iyon-tui/src/history/native/mod.rs`
- `crates/iyon-tui/src/history/native/frontier.rs`
- `crates/iyon-tui/src/history/projection/mod.rs`
- `crates/iyon-tui/src/scroll.rs`
- `crates/iyon-tui/src/content/mod.rs`
- `crates/iyon-tui/src/content/render.rs`
- `crates/iyon-tui/src/content/diff/mod.rs`
- `crates/iyon-tui/src/content/diff/model.rs`
- `crates/iyon-tui/src/content/diff/render.rs`
- `crates/iyon-tui/src/content/text/mod.rs`
- all production files under `crates/iyon-tui/src/content/text/`
- `crates/iyon-tui/src/projection/mod.rs`
- `crates/iyon-tui/src/projection/compose.rs`
- `crates/iyon-tui/src/projection/projector.rs`
- `crates/iyon-tui/src/projection/smooth.rs`
- `crates/iyon-tui/src/projection/validate.rs`
- `crates/iyon-tui/src/projection/value.rs`
- `crates/iyon-tui/src/stream/mod.rs`
- `crates/iyon-tui/src/stream/coord.rs`
- `crates/iyon-tui/src/controls/mod.rs`
- all production files under `crates/iyon-tui/src/controls/text_input/`

### 10.5 Core physical/terminal/theme files inspected

- `crates/iyon-tui/src/physical/mod.rs`
- `crates/iyon-tui/src/physical/glyph.rs`
- `crates/iyon-tui/src/physical/row.rs`
- `crates/iyon-tui/src/physical/style.rs`
- `crates/iyon-tui/src/physical/surface.rs`
- `crates/iyon-tui/src/physical/text_metrics.rs`
- `crates/iyon-tui/src/geometry/mod.rs`
- `crates/iyon-tui/src/geometry/constraints.rs`
- `crates/iyon-tui/src/geometry/point.rs`
- `crates/iyon-tui/src/geometry/rect.rs`
- `crates/iyon-tui/src/geometry/size.rs`
- `crates/iyon-tui/src/terminal/mod.rs`
- `crates/iyon-tui/src/terminal/backend.rs`
- `crates/iyon-tui/src/terminal/crossterm/mod.rs`
- `crates/iyon-tui/src/terminal/crossterm/key.rs`
- `crates/iyon-tui/src/terminal/termwiz/mod.rs`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
- `crates/iyon-tui/src/terminal/termwiz/lower.rs`
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
- `crates/iyon-tui/src/terminal/termwiz/shadow.rs`
- `crates/iyon-tui/src/terminal/termwiz/worker.rs`
- `crates/iyon-tui/src/theme/mod.rs`
- `crates/iyon-tui/src/theme/atoms.rs`
- `crates/iyon-tui/src/theme/batch.rs`
- `crates/iyon-tui/src/theme/framework.rs`

### 10.6 Native crate files inspected

- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/error.rs`
- `crates/iyon-tui-native/src/sync.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui/theme_dto.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`
- `crates/iyon-tui-native/src/generated/view_abi_conformance.rs`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
- `crates/iyon-tui-native/src/generated/view_state_schema.rs`

The generated bodies were indexed and inspected for module inclusion, symbol families, feature gates, and call shape. They were not manually re-described function by function because the schema and generator are the authoritative source of their repeated structure.

### 10.7 Tool crate and schema files inspected

- `tools/tui-abi-gen/src/main.rs`
- `tools/tui-abi-gen/src/model.rs`
- `tools/tui-abi-gen/src/validate.rs`
- `tools/tui-abi-gen/src/render_header.rs`
- `tools/tui-abi-gen/src/render_manifest.rs`
- `tools/tui-abi-gen/src/render_rust.rs`
- `tools/tui-abi-gen/src/render_state.rs`
- `tools/tui-abi-gen/src/render_typescript.rs`
- `tools/tui-abi/view_abi.toml`

The generator output list is explicitly enumerated in `tools/tui-abi-gen/src/main.rs:25-42`; the schema model fields and lifetime/ownership/function metadata are defined in `tools/tui-abi-gen/src/model.rs:46-193`.

### 10.8 Files indexed but not comprehensively read

- TypeScript package implementation outside the ABI paths.
- Native generated TypeScript/header outputs not directly needed to establish Rust-side ownership.
- Examples, replay traces, and package fixtures.
- CI/package scripts except where manifests were needed to understand crate features.
- Historical reports beyond the required PRE-V5 context and architecture subsystem references.
- External consumers/plugins not present in this repository.

### 10.9 Static inspection commands/searches

The investigation used repository file listing and source-content searches equivalent to:

- enumerate files under `crates/iyon-tui/src`
- enumerate files under `crates/iyon-tui-native/src`
- enumerate files under `tools/tui-abi-gen/src`
- inspect Cargo manifests
- inspect module declarations and `pub use` boundaries
- search `use crate::`, `use super::`, `#[cfg(feature = ...)]`, `pub struct`, `pub fn`, `extern "C"`, and generated-output lists
- inspect the tracked source manifest at `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`

No command executed a build, generator, test, benchmark, package script, or runtime process.