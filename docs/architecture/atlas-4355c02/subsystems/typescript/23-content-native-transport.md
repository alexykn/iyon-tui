# 23 — Content-native transport

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary assignment: `packages/iyon-tui/src/transport/{content,native}/` and the remaining transport files
- Goal: content data/control lanes, native loading/calls, resources, failure behavior, and identification of the other transport files

The source tree was inspected read-only. No source, configuration, dependency, build artifact, or running service was changed.

The required architecture material was consulted:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
- Historical PERF-13 material, especially `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`

Historical documents are used only to identify intended contracts and source/document drift. Current implementation observations below are based on the checked-in source tree.

### Scope boundaries

This report owns the TypeScript transport layer and its immediate native bridge seams:

- `packages/iyon-tui/src/transport/content/`
- `packages/iyon-tui/src/transport/native/`
- the other transport directories are inventoried and their cross-boundary role is described, but their detailed structural/state implementation belongs primarily to assignment 22 and other scouts
- selected immediate consumers were inspected:
  - `packages/iyon-tui/src/api/content/retained.ts`
  - `packages/iyon-tui/src/api/controls/framework-handle.ts`
  - `packages/iyon-tui/src/runtime/handle-registry.ts`
  - `packages/iyon-tui/src/runtime/environment.ts`
  - `packages/iyon-tui/src/runtime/runtime.ts`
  - `packages/iyon-tui/src/runtime/attachments.ts`
  - `packages/iyon-tui/src/runtime/wake-broker.ts`
- selected native bridge/core files were inspected to prove the seam:
  - `crates/iyon-tui-native/src/tui.rs`
  - `crates/iyon-tui-native/src/content_ffi.rs`
  - `crates/iyon-tui/src/application/content.rs`
  - `crates/iyon-tui/src/application/environment.rs`
  - `crates/iyon-tui/src/application/host.rs`
  - `crates/iyon-tui/src/binding/mod.rs`

The Rust content implementation is very large and is separately covered by the Rust content/projection/stream assignments. This report therefore cites the Rust implementation where necessary to establish the transport contract, but does not attempt to replace those subsystem reports.

### Facts, inferences, and unknowns

- **Fact:** `transport/content/ffi.ts` is the only TypeScript production module importing `bun:ffi` in the inspected package source.
- **Fact:** Source bulk payload mutation uses direct FFI; Source construction, control, status, snapshots, and resource lifecycle use Node-API methods.
- **Fact:** The native addon and direct FFI loader resolve the same staged `.node` artifact through `transport/native/artifact.ts`.
- **Fact:** The direct FFI payload call has no caller-supplied sequence number. Source identity and native Source revision provide ordering.
- **Fact:** Source mutation does not perform frame work inline. The native result reports whether the JavaScript environment must schedule a drain.
- **Fact:** ContentPort and ContentConnector are host-owned; Source objects are environment-owned and can be shared across hosts.
- **Fact:** The TypeScript status surface deliberately exposes only the stable `error` field. Native cleanup failure and operating failure are merged into that field, with cleanup taking precedence.
- **Inference:** The content path is a distinct content data plane, not a structural View serialization path. ContentHost structural attachment is separate from Source payload transport.
- **Unknown:** The exact native projection/cache algorithms are implemented in the large Rust content module and are not reproduced in full here.
- **Unknown:** Whether the apparent regular-expression spelling at `ffi.ts:502` is an actual source bug or only an escaping artifact in the inspection output requires a raw-source check. The displayed source is `/\\s|\\0/u`; if those are literally doubled backslashes, whitespace/NUL validation would not behave as intended.

### Validation status

No tests or builds were run during this investigation. Tests were inspected as behavioral evidence only. Historical completion records are not treated as execution performed during this run.

---

## 1. Responsibility and structure

### 1.1 Primary transport modules

Approximate physical production LOC were estimated from the checked-in source line spans. Tests are not included in the production counts. Generated bodies are excluded from the structural/state estimates.

| Path | Language | Approx. production LOC | Approx. test LOC | Public API? | Primary responsibility | Plane |
|---|---|---:|---:|---|---|---|
| `packages/iyon-tui/src/transport/content/abi.ts` | TypeScript | 114 | 0 | Partially; constants/helpers consumed internally | Content ABI constants, status table, metadata decoder | Content/data |
| `packages/iyon-tui/src/transport/content/control.ts` | TypeScript | 98 | 0 | Internal transport facade | N-API control calls for Source, Port, Connector | Content/control |
| `packages/iyon-tui/src/transport/content/ffi.ts` | TypeScript | 785 | 0 | Internal transport facade; one decoder exported | Direct FFI payload transport, ABI handshake, encoding, validation, result decoding | Content/data |
| `packages/iyon-tui/src/transport/native/addon.ts` | TypeScript | 248 | 0 | Private contract declarations | Native addon contracts for host, Source, Port, Connector, controls, and structural operations | Mixed native boundary |
| `packages/iyon-tui/src/transport/native/artifact.ts` | TypeScript | 72 | 0 | Internal helper | Canonical platform/artifact resolution and build identity | Native loading |
| `packages/iyon-tui/src/transport/native/factories.ts` | TypeScript | 8 | 0 | Internal helper | Native History and Source constructors | Native/control |
| `packages/iyon-tui/src/transport/native/resource-registry.ts` | TypeScript | 480 | 0 | Partially; runtime facade re-exports selected types/functions | Environment-wide handle/resource registry, generation, lease, and disposal state | Resource/lifetime |
| `packages/iyon-tui/src/transport/native/resources.ts` | TypeScript | 88 | 0 | Internal helper | Handle-local lookup plus authoritative registry delegation | Resource/lifetime |

The primary assigned transport implementation is approximately **1,893 physical TypeScript lines**, excluding tests and generated code.

### 1.2 Content transport responsibilities

#### `content/abi.ts`

`abi.ts` defines the fixed direct-FFI content ABI:

- magic/version/semantic version
- metadata lane count and byte size
- annotation record lane count and byte size
- mutation result lane count and byte size
- status table version and required symbol count
- endian marker
- build/schema fingerprints
- environment-drain scheduling flag
- the complete `CONTENT_STATUS` status code table

The status table includes:

- identity/environment failures:
  - `WRONG_ENVIRONMENT`
  - `STALE_ENVIRONMENT`
  - `STALE_SOURCE`
  - `SOURCE_DISPOSED`
  - `SOURCE_IN_USE`
- payload/validation failures:
  - `INVALID_ARGUMENT`
  - `INVALID_UTF8`
  - `INVALID_RANGE`
  - `UNKNOWN_ANNOTATION_KIND`
  - `INVALID_ANNOTATION_PAYLOAD`
  - `LIMIT_EXCEEDED`
  - `PAYLOAD_TOO_LARGE`
- lifecycle/content failures:
  - `SOURCE_SEALED`
  - `SOURCE_ALREADY_SEALED`
  - `SOURCE_RETENTION_OVERFLOW`
- runtime failures:
  - `ABI_MISMATCH`
  - `RUNTIME_POISONED`
  - `INTERNAL_PANIC`
  - `INTERNAL_INVARIANT`

`decodeContentAbiMetadata()` consumes a `Uint32Array`, verifies that it contains at least 32 lanes, and returns a typed metadata record. It does not itself validate expected values; expected-value validation is in `ffi.ts`.

Evidence: `packages/iyon-tui/src/transport/content/abi.ts:1-114`.

#### `content/control.ts`

`control.ts` is deliberately thin. It owns the small N-API control calls, not payload encoding or projection:

- `createTextSource()`
- `createContentPort()`
- `deactivatePort()`
- `contentPortMounted()`
- `connectContent()`
- `activateContent()`
- `deactivateContent()`
- `disposeContentConnector()`
- `contentConnectorStatus()`

The module re-exports native contract types from `native/addon.ts` and converts the immutable TypeScript `NativeTextFunnelControl` into positional N-API parameters.

Evidence: `packages/iyon-tui/src/transport/content/control.ts:1-98`.

#### `content/ffi.ts`

`ffi.ts` owns the content bulk-data lane. Its responsibilities are substantially broader than a raw FFI call:

1. Direct FFI symbol declarations.
2. Session lifetime and metadata handshake.
3. Native Source identity extraction.
4. UTF-8 sizing and encoding.
5. Annotation validation and fixed-record encoding.
6. Semantic style payload encoding and decoding.
7. Mutation result decoding.
8. Native status-to-`TuiError` conversion.
9. Environment-drain wake dispatch.
10. Append, replace, clear, seal, and head-truncate operations.

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:1-785`.

### 1.3 Native transport modules

#### `native/addon.ts`

`addon.ts` defines private TypeScript contracts corresponding to N-API native objects:

- `NativeTuiHostContract`
- `NativeTextSourceContract`
- `NativeContentPortContract`
- `NativeContentConnectorContract`
- `NativeViewStateContract`
- `NativeHistoryContract`
- `NativeTextInputContract`
- `NativeViewSlotContract`
- `NativeScrollPaneContract`
- `NativeTuiOutputContract`
- `NativeStateWake`
- host epoch and flush result structures

The file intentionally hides the concrete addon implementation. It loads the addon exactly once at module initialization:

```text
nativeArtifact = resolveNativeArtifact(import.meta.url)
loadedNative = require(nativeArtifact.absolutePath)
loadedNative.nativeVersion() must equal nativeArtifact.packageBuildId
native = loadedNative
```

Evidence: `packages/iyon-tui/src/transport/native/addon.ts:1-248`.

#### `native/artifact.ts`

`artifact.ts` centralizes native artifact selection:

- supported targets:
  - Darwin arm64/x64
  - Linux arm64/x64
  - Windows x64
- target-specific artifact naming
- optional `ION_TUI_NATIVE_ARTIFACT` override
- staged package artifact fallback:
  - `native/iyon-tui-native.node`
- relative override resolution against repository root
- `existsSync()` and `realpathSync()` canonicalization
- shared package build identity:
  - `"iyon-tui-native/s6"`

It explicitly rejects using a raw Cargo `.dylib`/`.so` as the Node-API target. Both Node-API and direct FFI use the staged Node addon path.

Evidence: `packages/iyon-tui/src/transport/native/artifact.ts:1-72`.

#### `native/factories.ts`

`factories.ts` provides private constructors for:

- native History
- native text Source

It is intentionally not a broad native factory registry. Host/Port creation occurs through `NativeTuiHostContract.contentPort()` in `runtime/runtime.ts`.

Evidence: `packages/iyon-tui/src/transport/native/factories.ts:1-8`.

#### `native/resource-registry.ts`

This is the shared environment-level resource resolver. It is intentionally plane-neutral and does not interpret structural, state, or content semantics.

Responsibilities:

- map framework handle IDs to native resource records
- retain weak references to handle/resource wrappers
- enforce positive monotonic handle IDs
- prevent duplicate registration
- enforce environment ownership
- assign an independent monotonic resource generation
- track `live`, `disposing`, and `disposed` lifecycle
- validate attachment environment, host, expected kind, and accepted node kinds
- maintain prepared, desired, and visible lease counts
- support explicit prepare/commit/abort lease transitions
- reject disposal while resources are prepared, desired, or visible
- invalidate all resources owned by a host during host teardown
- support weak finalization of unowned wrappers
- expose aggregate lease/resource statistics

Evidence: `packages/iyon-tui/src/transport/native/resource-registry.ts:1-480`.

#### `native/resources.ts`

`resources.ts` is a two-level lookup facade:

- `nativeResources`: handle-local `WeakMap<object, object>`
- `NativeResourceRegistry`: authoritative environment-level resolver for attachment-capable handles

Opaque values without semantic `HandleId`s can use the handle-local map. Attachment-capable framework handles are registered in the shared resolver.

Evidence: `packages/iyon-tui/src/transport/native/resources.ts:1-88`.

### 1.4 Remaining transport files

The complete transport manifest is listed in `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt:968-990`.

#### Structural transport

| Path | Responsibility |
|---|---|
| `transport/structural/component-id.ts` | Resolve component identity from a framework handle/resource |
| `transport/structural/encoding.ts` | Convert semantic layout/style/content values into native scalar/word encodings |
| `transport/structural/ir.ts` | TypeScript structural/native IR and native kind/code constants |
| `transport/structural/native-view-abi.ts` | Native View ABI session, structural calls, retained native references, path/ref operations |
| `transport/structural/policy.ts` | Structural limits and direct-materialization policy constants |
| `transport/structural/retained-dag.ts` | Retained semantic-node materialization, root boundary, native reference recovery, attachment lowering |
| `transport/structural/retained-path.ts` | Native path lineage and text layout transaction representations |
| `transport/structural/style-lowering.ts` | Lower TypeScript semantic style/theme/border values into structural native DTOs |

The content transport depends on structural transport only at the attachment seam: a `ContentPort` is resolved by `retained-dag.ts` as a `contentHost` native View attachment. Bulk Source data never uses structural serialization.

#### State transport

| Path | Responsibility |
|---|---|
| `transport/state/control.ts` | Typed geometry/presentation patch normalization and generated state envelope encoding |
| `transport/state/generated/state_envelope.ts` | Generated state envelope ABI |

State and content are separate mutation planes. A ContentPort can be attached structurally while its Source payload remains in the direct content FFI lane.

#### Structural ABI support

| Path | Responsibility |
|---|---|
| `transport/abi/structural/generated/view_abi.ts` | Generated structural ABI symbols/types |
| `transport/abi/structural/generated/view_abi_conformance.ts` | Generated ABI conformance calls |
| `transport/abi/structural/generated/view_abi_manifest.json` | Generated structural ABI manifest |
| `transport/abi/structural/generated/view_calls.ts` | Generated structural call wrappers |
| `transport/abi/structural/schema/view-kind-codes.json` | Structural View-kind schema |

These are generated/ABI support rather than content transport. Content has a manually synchronized direct ABI in `content/abi.ts` and the native `content_ffi.rs` implementation.

---

## 2. Types, APIs and contracts

### 2.1 Public content authoring surface versus transport

The public semantic surface is in `api/content/retained.ts`; transport modules are private implementation seams.

Public content concepts include:

- `TextStreamSource`
- `TextBlockSource`
- `TextFunnel`
- `ContentPort`
- `ContentConnector`
- `TextSourceSnapshot`
- `TextSourceStats`
- `TextSourceMutation`
- annotation and retention options
- connector status and lifecycle phase

The transport-facing API is not exported as a package-root authoring API. `api/content/retained.ts` calls transport functions directly.

Relevant source:

- source options and mutation types: `packages/iyon-tui/src/api/content/retained.ts:37-165`
- source conversion/query helpers: `retained.ts:176-334`
- `TextStreamSource`: `retained.ts:336-407`
- `TextBlockSource`: `retained.ts:409-468`
- `TextFunnel`: `retained.ts:470-558`
- `ContentPort`: `retained.ts:560-659`
- `ContentConnector`: `retained.ts:661-797`

### 2.2 Source identity and content generation

A native Source exposes:

- `sourceId()`
- `sourceGeneration()`
- `environmentSlot()`
- `environmentGeneration()`
- `contentGeneration()`

TypeScript converts Source IDs, revisions, offsets, and generations into validated `bigint`/number values.

Source identity used by direct FFI is the four-part tuple:

```text
(environment slot,
 environment generation,
 source slot/id,
 source generation)
```

Evidence:

- native contract: `packages/iyon-tui/src/transport/native/addon.ts:60-70`
- identity extraction/cache: `packages/iyon-tui/src/transport/content/ffi.ts:290-309`
- TypeScript identity conversion: `packages/iyon-tui/src/api/content/retained.ts:219-238`, `321-334`

The native direct FFI lookup uses environment slot/generation first and then Source slot/generation:

- `crates/iyon-tui-native/src/content_ffi.rs:202-213`
- `crates/iyon-tui-native/src/tui.rs:104-119`

This means stale environment and stale Source generations fail at the boundary rather than silently targeting a newly allocated resource.

### 2.3 Mutation contracts

#### Stream Source

`TextStreamSource` supports:

- `append()`
- `appendUtf8()` compatibility alias
- `replace()`
- `replaceUtf8()` compatibility alias
- `clear()`
- `seal()`
- `truncateHead()`

All operations return:

```text
{
  revision: bigint,
  environmentWakeEpoch: bigint,
  scheduleEnvironmentDrain: boolean
}
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:63-67`, `380-407`.

#### Block Source

`TextBlockSource` supports:

- `replace()`
- `replaceUtf8()` compatibility alias
- `clear()`
- `truncateHead()`

It does not expose append.

Evidence: `packages/iyon-tui/src/api/content/retained.ts:409-468`.

#### Native source semantics

The Rust implementation confirms:

- append requires a Stream Source: `crates/iyon-tui/src/application/content.rs:2301-2384`
- replace atomically replaces Block or Stream storage and increments `content_generation`: `content.rs:2386-2424`
- clear performs a generation/revision preflight before replacing storage: `content.rs:2426-2457`
- seal is a separate operation and is a one-way Source transition: `content.rs:2460-2485`
- head truncation advances the retained head without renumbering absolute coordinates: `content.rs:2487-2509`

The current source revision is the native linearization result. There is no caller sequence token.

### 2.4 Annotation contract

The public annotation record is operation-local and byte-based:

```text
kind?
startByte?
endByte?
namespace?
name?
style?
payload?
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:49-60`.

The direct FFI encoder supports four kinds:

- `tag` = 1
- `style` = 2
- `atomic` = 3
- `point` = 4

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:34-40`.

Validation includes:

- maximum annotation count: 16,384
- UTF-8 boundary validation
- range ordering and bounds
- point annotations must have empty ranges
- non-point annotations must cover non-empty text
- tag annotations require namespace/name and forbid opaque payload/style
- style annotations require semantic style and forbid tag names/opaque payload
- atomic/point annotations may carry opaque `Uint8Array` payloads
- annotation payload cap: 4 MiB

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:311-407`, `578-637`.

Native mirrors the record layout and validates pointer, count, alignment, and count limits:

- `crates/iyon-tui-native/src/content_ffi.rs:183-200`
- `content_ffi.rs:215-270`

### 2.5 Semantic style payload contract

Style annotations do not send arbitrary JavaScript objects. `ffi.ts` serializes a fixed payload with:

- payload version
- flags for role/foreground/background/attributes
- attribute presence/value bitsets
- role/theme key strings
- named, indexed, RGB, and theme colors

Encoding:

- `packages/iyon-tui/src/transport/content/ffi.ts:410-455`
- `ffi.ts:458-499`

Decoding:

- `packages/iyon-tui/src/transport/content/ffi.ts:507-571`

The Source snapshot query returns raw payload bytes through N-API and TypeScript decodes style annotation payloads when `kind === 2`:

- native JSON snapshot: `crates/iyon-tui-native/src/tui.rs:1178-1211`
- TypeScript snapshot conversion: `packages/iyon-tui/src/api/content/retained.ts:276-301`

This means the semantic style itself remains represented as caller-supplied semantic data until the content snapshot decoder, rather than being prematurely converted into a terminal cell style.

### 2.6 Funnel contract

`TextFunnel` is immutable and Source-neutral:

```text
kind: "plain" | "markdown" | "diff" | "ansi"
wrap: "word" | "grapheme" | "noWrap"
hyperlinks: boolean
delivery: "immediate" | "smooth"
smooth options:
  tickIntervalMs
  spring
  minUnitsPerSecond
  maxUnitsPerSecond
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:111-137`, `470-558`.

`textFunnelNative()` lowers this to the positional N-API control contract:

- `packages/iyon-tui/src/api/content/retained.ts:546-558`
- `packages/iyon-tui/src/transport/content/control.ts:58-73`
- native parser: `crates/iyon-tui-native/src/tui.rs:1506-1563`

The native parser accepts:

- `plain`, `markdown`, `diff`, `ansi`
- `word`, `grapheme`, `noWrap`
- immediate or smooth delivery

When smooth delivery is selected, native validates finite f32-compatible rate/spring values and constructs `SmoothConfig`.

One layering observation: TypeScript validates smooth values as finite non-negative numbers but does not bound them to the native f32 range. Values can therefore pass `TextFunnel` construction and fail later during native `connect()`. This is an explicit boundary failure, not a silent fallback.

### 2.7 ContentPort and ContentConnector contracts

`ContentPort`:

- has kind `"content-port"`
- is host-owned
- owns Connector membership
- does not own Source data
- accepts only `SEMANTIC_VIEW_KIND.contentHost`
- exposes `connect()`, `deactivate()`, `mounted()`, and `connectorCount()`

Evidence: `packages/iyon-tui/src/api/content/retained.ts:560-659`.

`ContentConnector`:

- has kind `"connector"`
- references exactly one Port and one Source
- stores the native connector resource directly for control/status
- supports `activate()`, `deactivate()`, `status()`, and deferred `dispose()`
- has a JavaScript-visible intermediate disposing state
- finalizes its wrapper only when native status reaches `"disposed"`

Evidence: `packages/iyon-tui/src/api/content/retained.ts:661-797`.

Native status is intended to be:

```text
phase
requested
visible
projectedSourceRevision?
error?
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:154-165`, `732-750`.

The Rust core internally has additional cleanup fields (`cleanup_pending`, `cleanup_error`), but the N-API adapter intentionally merges cleanup failure into the existing `error` field:

- `crates/iyon-tui/src/application/content.rs:3204-3213`
- `crates/iyon-tui-native/src/tui.rs:53-72`, `1404-1430`

This preserves the existing TypeScript status shape but means TypeScript callers must inspect `error.code` to distinguish projection/activation failure from deferred Source cleanup.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency graph

```text
Public Source API
  api/content/retained.ts
      │
      ├── transport/content/ffi.ts
      │      ├── transport/content/abi.ts
      │      ├── transport/native/addon.ts
      │      ├── transport/native/artifact.ts
      │      └── runtime/environment.ts
      │
      └── transport/content/control.ts
             ├── transport/native/addon.ts
             └── transport/native/factories.ts

Public Tui runtime
  runtime/runtime.ts
      │
      ├── transport/content/control.ts
      │      └── NativeTuiHost.contentPort()
      │
      ├── runtime/attachments.ts
      │      └── transport/native/resource-registry.ts
      │
      └── transport/structural/retained-dag.ts
             └── native ContentHost structural attachment

Native addon
  transport/native/addon.ts
      │
      ├── transport/native/artifact.ts
      └── require(staged .node)

Direct content lane
  transport/content/ffi.ts
      │
      └── bun:ffi dlopen(same staged .node)
              │
              └── crates/iyon-tui-native/src/content_ffi.rs
                      │
                      └── iyon_tui::application::content
```

### 3.2 Ownership map

| Object | Created by | Native owner | JS owner | Destruction trigger |
|---|---|---|---|---|
| Native addon session | module import | Process/JS module | `addon.ts` module | Process/module lifetime |
| Direct FFI session | first content payload/metadata query | JS environment `WeakMap` session | `ffi.ts` | Environment object becoming unreachable; native library handle kept alive by session |
| Text Source | `TextStreamSource.create()` / `TextBlockSource.create()` | `TuiEnvironment` Source registry | Caller; `FrameworkHandle<"source">` | Explicit Source dispose after no Source membership/in-use rejection |
| ContentPort | `Tui.contentPort()` | Host `ContentHostRegistry` | Owning `Tui`; `FrameworkHandle<"content-port">` | Host close or explicit disposal after no attachment leases |
| ContentConnector | `ContentPort.connect()` | Host content registry | Owning Port plus caller wrapper | Deferred native disposal after removal/cleanup frame |
| Native resource record | `FrameworkHandle` construction | JS environment registry | `NativeResourceRegistry` | `release`, finalization, or host invalidation after leases drain |
| Prepared attachment lease | Structural candidate preparation | JS resource registry | Candidate/root transition | Commit desired, commit visible, abort, or finalizer |
| Environment wake | Source mutation/host pending mark | Native environment + JS wake broker | One global runtime environment | Drained/rearmed through broker |

### 3.3 Source-sharing topology

A Source is environment-owned, not host-owned. Multiple host-bound Connectors may subscribe to the same Source:

```text
                    ┌── Host A
Text Source ────────┤      └─ ContentPort A ─ ContentConnector A
(environment-owned)│
                    └── Host B
                           └─ ContentPort B ─ ContentConnector B
```

The native Source registry captures subscriber groups and attempts all eligible host wakes. A failing host must not prevent healthy hosts from receiving the same accepted Source revision.

Evidence:

- Source wake fanout: `crates/iyon-tui/src/application/content.rs:2529-2623`
- Source identity independent from host: `packages/iyon-tui/tests/tui_perf13_h.test.ts:33-49`
- multi-host Source behavior: `packages/iyon-tui/tests/tui_perf13_d.test.ts:15-43`

### 3.4 Structural attachment ownership

A `ContentPort` is not attached by the content data lane. It becomes a structural attachment when a semantic `ContentHost` View contains the Port.

`runtime/attachments.ts`:

- scans the semantic candidate
- detects `node.contentAttachment`
- resolves it as expected kind `"content-port"`
- checks environment and host
- checks accepted node kind
- rejects duplicate Port attachment in one semantic candidate
- creates a prepared resource lease

Evidence: `packages/iyon-tui/src/runtime/attachments.ts:181-245`.

`transport/structural/retained-dag.ts` then lowers the ContentHost:

1. require `contentAttachment`
2. resolve native resource with expected `"content-port"`
3. obtain the native attachment ID
4. split it into u64 words
5. call `viewContentHostCreate()`

Evidence: `packages/iyon-tui/src/transport/structural/retained-dag.ts:482-502`.

Thus the structural path is:

```text
View.content(port)
  → semantic ContentHost node
  → prepareSemanticAttachments()
  → ContentPort prepared lease
  → retained-dag.materializeContentHostNode()
  → viewContentHostCreate(node ID, port ID)
  → native desired structure
  → host frame visibility
```

This is distinct from:

```text
Source.append(text)
  → direct FFI
  → native Source storage
  → native Connector dirty state
  → environment drain
```

### 3.5 Resource registry ownership and lease identity

`NativeResourceRegistry` assigns:

- semantic `HandleId`: allocated by `runtime/handle-registry.ts`
- resource generation: allocated by the shared registry
- weak identity maps for JS wrapper and native resource
- lifecycle state
- prepared/desired/visible lease counters

`PreparedResourceLease` retains strong references to the handle and resource while a binding is prepared, desired, or visible:

- `packages/iyon-tui/src/transport/native/resource-registry.ts:74-161`

This prevents weak-wrapper finalization from reclaiming native state while a candidate or visible frame still depends on it.

---

## 4. Execution paths and state transitions

### 4.1 Source creation

```text
TextStreamSource.create(options)
  → validateTextSourceOptions()
  → createTextSource("stream", options)
  → nativeTui.textSource()
  → new NativeTextSource(kind, options)
  → host_environment_for_env()
  → TuiEnvironment.create_content_source()
  → optional configure_retention()
  → FrameworkHandle<"source"> registration
  → runtime resource registry record
```

Evidence:

- TypeScript creation: `packages/iyon-tui/src/api/content/retained.ts:336-355`, `409-428`
- native factory: `packages/iyon-tui/src/transport/native/factories.ts:4-8`
- N-API constructor/configuration: `crates/iyon-tui-native/src/tui.rs:1089-1131`
- Source resource registration: `packages/iyon-tui/src/api/controls/framework-handle.ts:18-34`, `runtime/handle-registry.ts:25-49`

Native Source options support:

- `retention.maxBytes`
- `retention.maxLines`
- `retention.overflow` = `"drop-oldest"` or `"error"`

TypeScript validates the same shape before N-API. Native validates again at its boundary:

- TypeScript: `retained.ts:176-203`
- native: `tui.rs:1433-1503`

If native retention configuration fails after Source allocation, native attempts Source cleanup and returns either the original content error or an aggregate/internal cleanup error:

- `crates/iyon-tui-native/src/tui.rs:1113-1127`

### 4.2 ContentPort creation and structural first use

```text
Tui.contentPort()
  → prepareMutation("tui.contentPort")
  → validate family == "text"
  → NativeTuiHost.contentPort("text")
  → TuiHost.create_content_port(ContentFamily::Text)
  → NativeContentPort wrapper
  → FrameworkHandle<"content-port"> registration
  → accepted node kind = SEMANTIC_VIEW_KIND.contentHost
```

Evidence:

- `packages/iyon-tui/src/runtime/runtime.ts:601-630`
- `crates/iyon-tui-native/src/tui.rs:849-862`
- `packages/iyon-tui/src/api/content/retained.ts:568-590`

The Port is not visible merely because it was created. It becomes mounted only after a semantic `View.content(port)` is structurally prepared, committed, and made visible.

### 4.3 Connector creation

```text
ContentPort.connect(source, funnel)
  → assert Port mutation allowed
  → validate Source object/kind/live state
  → validate TextFunnel/family
  → nativeResourceOf(source, "source")
  → native Port.connect(source, funnel control)
  → native parse funnel
  → HostContentPort.connect()
  → NativeContentConnector wrapper
  → FrameworkHandle<"connector"> registration
  → Port connector membership set
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:592-614`.

Native positional control parameters are:

```text
source
kind
wrap
hyperlinks
smooth
tickIntervalMs
spring
minUnitsPerSecond
maxUnitsPerSecond
```

Evidence:

- TypeScript call: `packages/iyon-tui/src/transport/content/control.ts:58-73`
- N-API method: `crates/iyon-tui-native/src/tui.rs:1293-1334`
- funnel parser: `tui.rs:1506-1563`

A Connector is initially `"idle"` or `"waiting-for-mount"` depending on activation and Port visibility. The TypeScript tests establish that:

```text
connect
  → activate
  → status.phase == "waiting-for-mount"
  → render View.content(port)
  → status.phase == "active"
```

Evidence: `packages/iyon-tui/tests/tui_perf13_d.test.ts:15-24`.

### 4.4 Source append path

The real TypeScript-to-native payload path is:

```text
TextStreamSource.append(text, annotations)
  → FrameworkHandle.call()
  → validateSourceMutation()
  → appendTextSource()
  → invokePayload()
       ├─ encodedText()
       ├─ encodeAnnotations()
       ├─ sourceIdentity()
       ├─ session()
       │    ├─ resolve nativeArtifact
       │    ├─ dlopen staged .node
       │    ├─ metadata ABI probe
       │    └─ validateMetadata()
       └─ iyon_tui_source_append_utf8_v1(...)
  → finishMutation()
       ├─ decode status
       ├─ decode fixed mutation result
       ├─ request wake if schedule flag set
       └─ return revision/wake epoch
```

Evidence:

- public call: `packages/iyon-tui/src/api/content/retained.ts:380-384`
- payload invocation: `packages/iyon-tui/src/transport/content/ffi.ts:653-692`
- public wrappers: `ffi.ts:736-773`

Native direct FFI path:

```text
iyon_tui_source_append_utf8_v1
  → guarded(catch_unwind)
  → validate output result pointer/alignment
  → source_for_identity(environment/source slots and generations)
  → validate byte pointer/length
  → validate annotation record pointer/count/alignment
  → validate annotation payload pointer/length
  → HostContentSource.append_utf8()
  → copy/adopt payload into native Source-owned chunks
  → update Source revision/storage
  → capture subscribers
  → mark eligible hosts content-pending
  → return fixed mutation result
```

Evidence: `crates/iyon-tui-native/src/content_ffi.rs:215-306`, `348-380`; Source mutation implementation `crates/iyon-tui/src/application/content.rs:2301-2384`.

The direct FFI pointers are borrowed only for the synchronous call. No pointer is retained by native after return. The JS TypedArrays remain strongly reachable for the duration of the call through the synchronous invocation.

Historical contract evidence: `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:1896-1910`.

### 4.5 Wake and frame path after Source mutation

The mutation result contains:

```text
source_revision_lo/hi
environment_wake_epoch_lo/hi
flags
reserved0
```

Evidence:

- TypeScript decode: `packages/iyon-tui/src/transport/content/ffi.ts:65-72`, `176-218`
- native layout: `crates/iyon-tui-native/src/content_ffi.rs:96-111`

When the native flag contains `CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN`, `finishMutation()` invokes the supplied `requestWake` callback. For the public Source API, this callback is:

```text
sourceWake()
  → runtimeEnvironment().wakeBroker.markEnvironmentPending()
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:206-212`.

The environment broker then schedules one microtask and drains native pending host state. The broker is a fanout driver, not a JavaScript subscription mirror:

- `packages/iyon-tui/src/runtime/wake-broker.ts:121-186`
- automatic drain: `wake-broker.ts:275-305`
- native flush call: `wake-broker.ts:307-340`

Native Source wake fanout is responsible for determining affected hosts. The JavaScript side does not maintain a Source-to-host subscription set.

The public Source append path therefore does **not**:

- rebuild a View
- serialize a semantic View
- call structural materialization
- perform measurement or paint inline
- call N-API per affected host

### 4.6 Structural content projection path

After a ContentPort is mounted, native host content state owns projection. The Port itself owns allocation/attachment binding; native content Connector state holds Source/Funnel/projection information.

The Rust content registry maintains:

- connector requested/visible state
- source membership independently from wake subscription
- committed/candidate projections
- projected Source revision
- semantic/projection caches
- delivery revisions/frontiers
- cleanup state

Evidence:

- connector state fields: `crates/iyon-tui/src/application/content.rs:2972-3011`
- candidate state: `content.rs:3037-3048`
- connector status: `content.rs:3204-3213`
- prepare/commit: `content.rs:4574-4673`
- projection commit state: `content.rs:5125-5153`

The content status state machine is native-owned. TypeScript only requests activation/deactivation/disposal and observes status.

### 4.7 Connector activation and failure transition

```text
ContentConnector.activate()
  → assertMutationAllowed()
  → ensureControlOpen()
  → native connector.activate()
  → native content state marks requested
  → return wake disposition
  → possible "activation-pending"
  → environment drain
  → projection/activation candidate
  → either:
       active + visible
     or failed + invisible
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:712-720`.

The test-defined failure route is:

```text
first Connector active/visible
second Connector activation requested
native failNextActivation injected for second
second status = activation-pending
flush
first remains active, visible=true, requested=false
second becomes failed, visible=false, requested=true
second activate again
flush
second becomes active, visible=true, requested=true
```

Evidence: `packages/iyon-tui/tests/tui_perf13_d.test.ts:60-92`.

This establishes that candidate Connector failure does not replace the currently visible Connector. Activation failure is recoverable by a subsequent explicit activation and does not silently fall back to an alternate payload or View route.

### 4.8 Connector disposal and post-unmount lifecycle

Connector disposal is intentionally two-phase:

```text
ContentConnector.dispose()
  → beginDisposal(handle ID) in JS resource registry
  → native connector.dispose()
  → native may defer actual disposal until host frame cleanup
  → status query
  → if native phase == disposed:
       finalizeWrapper()
         → remove from Port connector set
         → releaseFrameworkHandle()
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:756-792`.

If native disposal throws, the TypeScript registry calls `cancelDisposal()` and the wrapper remains usable for a later retry:

- `retained.ts:759-768`
- registry cancellation: `packages/iyon-tui/src/transport/native/resource-registry.ts:370-384`

This is separate from ordinary `FrameworkHandle.dispose()`, which assumes a single synchronous native disposal. Connector owns an intermediate `"disposing"` state because native content cleanup may be deferred.

### 4.9 Host teardown

`Tui` closes host-owned content in this sequence:

1. unregister host from wake broker
2. invoke `host.disposeContentResources()`
3. clear runtime error reporting
4. dispose attachment bindings
5. clear ViewState bindings
6. invalidate host-owned resources in shared resource registry
7. dispose owned handles, retrying dependency-sensitive handles
8. dispose retained execution
9. close root boundary
10. dispose native host

Evidence: `packages/iyon-tui/src/runtime/runtime.ts:788-840`.

The source is intentionally not host-owned. `Tui.close()` leaves environment-owned Sources available to the caller; this is verified by the lifetime tests:

- `packages/iyon-tui/tests/tui_perf13_h.test.ts:33-49`
- `tui_perf13_h.test.ts:67-78`

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path matrix

| Semantic operation | Authoritative production path | Alternate path | Selection condition | Failure behavior |
|---|---|---|---|---|
| Create Stream/Block Source | `TextStreamSource.create()`/`TextBlockSource.create()` → N-API `NativeTextSource` | None found in TypeScript production | Constructor chosen by Source class | Native constructor/configuration error; registration cleanup attempted |
| Append Source payload | `appendTextSource()` → `bun:ffi` direct symbol `iyon_tui_source_append_utf8_v1` | No N-API payload mutation path found | Always direct FFI for payload | Status decoded; non-OK throws; accepted revision returned even if later host wake fails |
| Replace Source payload | `iyon_tui_source_replace_utf8_v1` | None found | Explicit `replace()` call | Same fixed status/result contract |
| Clear Source | Direct FFI `iyon_tui_source_clear_v1` | None found | Explicit `clear()` call | Status/result; sealed Stream rejected |
| Seal Source | Direct FFI `iyon_tui_source_seal_v1` | None found | Explicit `seal()` call | One-way Source state transition; repeated/invalid seal fails |
| Truncate Source head | Direct FFI `iyon_tui_source_head_truncate_v1` | None found | Explicit `truncateHead()` call | u64 offset validation and native range/retention errors |
| Source snapshot | N-API `NativeTextSource.snapshot()` | None found | Diagnostic/query operation | JSON/native shape converted and validated; style payload decode can raise ABI mismatch |
| Source stats | N-API `NativeTextSource.stats()` | None found | Diagnostic/query operation | Native values converted to validated bigint/number fields |
| Create ContentPort | N-API `NativeTuiHost.contentPort()` | None found | `Tui.contentPort()` | Unsupported family or host failure throws |
| Connect Source/Funnel | N-API `NativeContentPort.connect()` | None found | `ContentPort.connect()` | Invalid Source/Funnel, family, host, or native parser failure throws |
| Activate Connector | N-API `NativeContentConnector.activate()` | None found | Explicit Connector control call | Native activation may remain pending, fail in drain, or become active |
| Attach ContentPort structurally | Retained structural path → `viewContentHostCreate()` | No complete-object structural fallback in inspected production | Semantic ContentHost node materialization | Missing attachment, wrong kind, duplicate, stale resource, or native refusal fails candidate |
| Source wake scheduling | Native Source fanout sets result flag; JS calls `wakeBroker.markEnvironmentPending()` | No JS subscription mirror | Native result flag | Accepted mutation remains authoritative; wake failure is reported through host error channels |
| Connector disposal | Native deferred disposal plus status reconciliation | None found | Explicit dispose or host teardown | Resource registry blocks disposal while leased; retry/cancel path preserves liveness |
| Native artifact load | One canonical staged `.node` path via `artifact.ts` | Environment override path | `ION_TUI_NATIVE_ARTIFACT` if set and exists | Missing/unsupported artifact throws; no fallback to a different transport |
| Direct FFI ABI validation | Metadata probe and fingerprint validation | None found | First direct FFI session per environment | ABI mismatch throws runtime `TuiError` before payload call |

### 5.2 Direct FFI versus Node-API

The current split is intentional:

```text
Node-API:
  Source construction
  Source identity/query/snapshot/stats
  ContentPort creation
  Connector connect/activate/deactivate/dispose/status
  host/runtime/structural/state controls

bun:ffi:
  Source append payload
  Source replace payload
  Source clear
  Source seal
  Source head truncate
```

Historical PERF-13 says the high-volume payload lane must use direct FFI while Node-API may remain for control/query operations:

- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:1773-1779`
- same artifact: `...:1781-1814`
- content payload scope: `...:5617-5621`

The current source matches that split. No TypeScript payload fallback to N-API or an old stream bridge was found in the inspected package source.

### 5.3 ABI handshake and loader failure behavior

`ffi.ts:250-279`:

1. obtains `nativeArtifact`
2. calls `dlopen()` on the canonical absolute path
3. declares the six required symbols
4. allocates a 32-lane metadata buffer
5. invokes `iyon_tui_perf13_abi_metadata_v1`
6. maps the returned status
7. validates all metadata fields, fingerprints, reserved lanes, sizes, alignment, and pointer width
8. caches the session by runtime environment object

`validateMetadata()` checks:

- magic
- ABI version
- semantic version
- pointer width
- endian marker
- metadata/result/annotation sizes
- result/annotation alignment
- annotation lane count
- status table version
- required symbol count
- reserved fields
- build fingerprint
- schema fingerprint

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:220-248`.

Unknown metadata status is treated as ABI mismatch, not as an ordinary content error.

### 5.4 Status/error classification

`contentError()` classifies only the following as runtime-category errors:

- `ABI_MISMATCH`
- `RUNTIME_POISONED`
- `INTERNAL_PANIC`
- `INTERNAL_INVARIANT`

All other content statuses become validation-category errors:

- `packages/iyon-tui/src/transport/content/ffi.ts:138-150`

This is a useful distinction, but there is a second mapping layer in `api/errors.ts` that maps native codes to categories. The native code is preserved as `ION_<STATUS>` for direct FFI-generated errors.

The native N-API bridge uses `NativeError::content`, `NativeError::invalid_input`, and `NativeError::internal` in different places:

- Source payload errors are converted through content status mapping in `content_ffi.rs`
- Source/Port/Connector control content failures are mapped through `NativeError::content`
- host/lock failures often become internal errors
- argument parsing failures become invalid-input errors

### 5.5 Accepted mutation plus wake failure

The Source mutation result is authoritative even if a subscriber wake fails.

Rust comments and implementation explicitly state:

- Source revision is accepted before subscriber wake fanout
- every eligible host is attempted
- one failed host must not cancel healthy hosts
- wake failure is recorded on the environment/host error channel
- result still carries the accepted revision and drain hint

Evidence: `crates/iyon-tui/src/application/content.rs:2537-2623`.

The native direct FFI test verifies this behavior:

- `crates/iyon-tui-native/src/content_ffi.rs:500-?` test `accepted_wake_failure_keeps_direct_ffi_result_and_healthy_fanout`

The TypeScript-facing test verifies the accepted revision and wake hint:

- `packages/iyon-tui/tests/tui_perf13_h.test.ts:17-30`

This is not a fallback. It is a deliberate separation between:

```text
Source mutation acceptance
and
host delivery/drain success
```

### 5.6 Panic and process-fatal conditions

The direct FFI entrypoints wrap work in `catch_unwind`:

- `crates/iyon-tui-native/src/content_ffi.rs:302-306`

A panic is converted to `CONTENT_STATUS_INTERNAL_PANIC` rather than unwinding across C ABI.

The historical contract explicitly excludes out-of-memory from recoverable statuses. The current TypeScript ABI table does not advertise an OOM code. User-controlled payload/annotation limits are checked before native allocation:

- TypeScript payload cap: `ffi.ts:28-31`, `640-650`
- native payload cap: `content_ffi.rs:24-25`, `167-180`
- historical policy: `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:1945-1960`

### 5.7 Silent fallback search result

Search scope:

- all `packages/iyon-tui/src/transport/**/*.ts`
- all `packages/iyon-tui/src/api/content/**/*.ts`
- all `packages/iyon-tui/src/runtime/**/*.ts`
- native bridge symbols in `crates/iyon-tui-native/src`
- relevant Rust content entrypoints

Findings:

- no old JS-to-native text payload bridge was found
- no alternate N-API Source append/replace implementation was found
- no second Source scheduler in TypeScript was found
- no TypeScript fallback from direct FFI to N-API was found
- no complete-object structural fallback from retained materialization was found in the inspected structural module
- compatibility aliases `appendUtf8()` and `replaceUtf8()` are explicit semantic aliases, not separate implementations
- Source snapshots/stats are query paths, not payload mutation alternatives

Historical source-contract intent prohibits a second high-volume path:

- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:1976-1996`

The current TypeScript source complies within the inspected scope.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Direct FFI session cache

`ffi.ts` stores `ContentFfiSession` in:

```text
WeakMap<object, ContentFfiSession>
```

The key is `runtimeEnvironment().resources.environment`.

The session retains:

- the Bun `dlopen` library handle
- typed symbol map
- metadata copy
- canonical artifact path

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:127-136`, `250-287`.

The library is opened once per JavaScript environment, not once per Source or mutation.

### 6.2 Source identity cache

`sourceIdentities` is a `WeakMap<NativeTextSourceContract, [number, number, number, number]>`.

The identity tuple is read from the native wrapper once and reused for every direct FFI operation on that wrapper:

- `packages/iyon-tui/src/transport/content/ffi.ts:135-136`, `290-309`

This is safe only while a native Source wrapper’s identity remains immutable. Native Source generation is part of the identity and native disposal prevents further calls.

### 6.3 Content result and query allocation

Each mutation allocates:

- encoded text `Uint8Array`
- annotation records `Uint32Array`
- annotation payload `Uint8Array`
- fixed six-lane mutation result `Uint32Array`

There is no pooling visible in the current implementation.

The native side copies/adopts validated payload data into Source-owned storage before the direct call returns. Native comments explicitly distinguish borrowed FFI buffers from retained Source chunks.

Evidence:

- TypeScript allocations: `ffi.ts:578-650`, `653-733`
- native copy contract: `crates/iyon-tui/src/application/content.rs:2301-2307`

This is the main hot-path cost that the direct FFI lane removes from generic N-API object/JSON transport; it does not eliminate Source-owned copying, annotation parsing, or content storage allocation.

### 6.4 Content projection cache keys

The Rust content implementation maintains separate semantic/projection identity concepts.

`SemanticProjectionKey` includes:

- Source ID
- Source generation
- content generation
- Source revision
- Source base/end
- sealed state
- Funnel kind

Evidence: `crates/iyon-tui/src/application/content.rs:269-294`.

`TextProjectionKey` additionally includes:

- offered width
- wrap mode
- Funnel kind
- delivery revision

Evidence: `content.rs:207-216`, `1034-1042`.

The code comments state semantic IR is independent of theme, width, delivery tick, and viewport, while the rendered/projection key is width and delivery sensitive.

This is an important ownership boundary:

```text
Source mutation/replacement
  → Source/content-generation/revision invalidation

Width/wrap/Funnel change
  → projection/layout-key invalidation

Smooth delivery tick
  → delivery-frontier/projection invalidation

Theme
  → presentation realization, not Source semantic parsing
```

### 6.5 Scheduling work frequency

| Work | Frequency/trigger |
|---|---|
| Native artifact resolution | Native module import |
| FFI `dlopen`/metadata probe | First content FFI call per JS environment |
| Source identity extraction | First payload call per native Source wrapper |
| UTF-8 encoding | Every append/replace payload |
| Annotation encoding | Every append/replace payload |
| Native Source storage update | Every accepted append/replace/clear/seal/truncate |
| Native subscriber capture | Every accepted mutation with subscribers |
| JS wake broker mark | Only when native mutation result requests environment drain |
| JS microtask scheduling | Edge-triggered; one queued driver while latched/draining |
| Host content drain | Native environment pending-host drain |
| Projection recompile | Source/Funnel/width/delivery invalidation as determined by native content registry |
| Smooth progression | Native clock/tick path, not TypeScript per-tick payload calls |
| Structural ContentPort materialization | Semantic candidate materialization only |
| ContentPort visibility | Native frame commit/drain |
| Source snapshot/stats | Explicit caller query; not frame hot path |

### 6.6 Resource lease costs and bounds

The resource registry has no configured global record bound visible in `resource-registry.ts`. It relies on:

- monotonically increasing IDs
- explicit release
- host invalidation
- weak finalizers
- lease count drainage

Prepared leases are bounded by the number of attachments in the candidate tree. Duplicate attachment detection is per candidate and keyed by handle ID.

The registry can retain a disposed/disposing record while outstanding leases exist. This is necessary for safe replacement/visibility transitions but means a leaked visible/desired lease can retain native resources.

Evidence:

- lease counters and finalizer: `resource-registry.ts:49-68`
- prepare and lease creation: `resource-registry.ts:272-337`
- disposal blocking: `resource-registry.ts:340-367`
- host invalidation/finalization: `resource-registry.ts:386-460`

### 6.7 Performance observability

The assigned transport modules expose no direct counters of their own, apart from returning revision/wake metadata and content stats. Observability is supplied elsewhere:

- wake broker counters/traces: `packages/iyon-tui/src/runtime/wake-broker.ts:65-119`
- native content stats: `TextSourceStats`, `NativeTextSource.stats()`
- Rust content perf counters in the application/content module
- historical PERF-13 tests and benchmark records

The direct FFI path itself does not expose a “which route ran” counter to TypeScript. Route integrity is inferred from source imports/symbol use and tests.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript behavioral tests inspected

#### `packages/iyon-tui/tests/tui_perf13_h.test.ts`

Covers:

- accepted direct-FFI revision and environment wake hint
- rendered Source update after `tui.flush()`
- eight-host shared Source fanout
- Source survives host teardown
- repeated host/Connector ownership cycles
- host-owned Port/Connector handles become invalid after host teardown
- Source remains caller-owned after host teardown

Evidence: `tui_perf13_h.test.ts:1-79`.

#### `packages/iyon-tui/tests/tui_perf13_d.test.ts`

Covers:

- Source ownership independent from host ownership
- Source `SOURCE_IN_USE` while Connector membership remains
- Source reuse on another host after first host cleanup
- cross-host ContentPort attachment rejection
- duplicate ContentPort attachment rejection
- visible Connector preserved across candidate activation failure
- failed Connector can be explicitly retried
- stable native lifecycle codes:
  - `ION_SOURCE_IN_USE`
  - `ION_PORT_MOUNTED`
- deferred cleanup error remains in the supported `error` field without adding `cleanup_pending` or `cleanup_error` fields

Evidence: `tui_perf13_d.test.ts:1-171`.

#### `packages/iyon-tui/tests/tui_handles.test.ts`

Search evidence shows tests for:

- synchronous native mutations not allocating Promise wrappers
- Source revision preservation
- sealed-state errors

#### `packages/iyon-tui/tests/tui_ansi_scanner.test.ts`

Search evidence shows:

- Source retention configuration
- ANSI Funnel route
- hyperlink behavior
- UTF-8 continuation-byte handling
- ANSI scanner output

#### `packages/iyon-tui/tests/tui_smooth_delivery.test.ts`

Search evidence shows:

- smooth Funnel control path
- explicit `tickIntervalMs`, minimum, and maximum rates
- host-driven time advancement

#### `packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts`

Search evidence shows:

- Block and Stream Source use
- Markdown/ANSI/Diff Funnel selection
- Source replacement/clear behavior
- semantic cache ownership and isolation

### 7.2 Native content tests inspected

`crates/iyon-tui-native/src/content_ffi.rs` contains direct FFI tests, including:

- accepted Source mutation result despite a failed host wake
- healthy-host fanout after failed-host wake failure
- direct FFI status/result behavior

The core Rust content file contains extensive tests for:

- Source revision and content-generation overflow
- retention behavior
- Source wake failure and healthy fanout
- Connector cleanup
- projection candidates
- smoothing frontiers
- cache identity
- host/Source lifetime

Search evidence includes:

- `crates/iyon-tui/src/application/content.rs:7180-7194`
- `content.rs:7334-7343`
- `content.rs:7398-7408`
- `content.rs:7680-7683`
- `content.rs:7797-7812`
- `content.rs:7848-7904`
- `content.rs:7958-8084`
- `content.rs:8340-8440`

No claim is made that these tests ran in this investigation.

### 7.3 Missing observability

The following are not directly observable through the current TypeScript content API:

- whether the Source mutation was delivered to zero, one, or many native hosts
- which individual host wake failed, except through runtime error reporting
- whether projection was a semantic cache hit or miss
- whether a Connector activation failed during projection, native attachment, or cleanup except through a generic `error.code`/diagnostic
- whether direct FFI session reuse occurred, except by source inspection
- whether a content mutation caused a new projection, smoothing tick, or only a wake
- whether a status error is operating failure versus cleanup failure without interpreting code/diagnostic conventions

The native status shape intentionally keeps only one error field, so any future route diagnostics would need to preserve that compatibility contract or add an explicit new status schema.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Current Source/API versus historical `ContentDataTransport` requirement

Historical PERF-13 text says:

> Only `transport/content/ffi.ts` may import `bun:ffi` or construct raw pointers. Public/API/runtime modules call a typed `ContentDataTransport` interface.

Evidence: `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:5691-5697`.

Current source does not contain a `ContentDataTransport` abstraction in the inspected TypeScript package. Instead:

```text
api/content/retained.ts
  directly imports:
    appendTextSource
    clearTextSource
    replaceTextSource
    sealTextSource
    truncateTextSource
    decodeSemanticStylePayload
  from transport/content/ffi.ts
```

Evidence: `packages/iyon-tui/src/api/content/retained.ts:1-32`.

This is a source/document discrepancy. The current implementation still honors the more important one-adapter rule—`ffi.ts` is the only direct FFI module—but the typed interface indirection described historically is not present.

### 8.2 Content API depends on presentation types at the transport boundary

`ffi.ts` imports:

- `TextSourceAnnotation`, `SemanticTextStyle`
- `TextAttribute`
- `ColorSpec`

Evidence: `packages/iyon-tui/src/transport/content/ffi.ts:3-7`.

This is caller-supplied semantic styling, not Iyon product policy. It is compatible with the framework boundary because the values are generic text/style concepts. Nevertheless, it means content payload encoding is coupled to the TypeScript presentation/style type definitions.

### 8.3 ContentPort is both content-plane and structural-plane

ContentPort is named and authored as a content object, but it is also a structural attachment:

```text
ContentPort
  → FrameworkHandle<"content-port">
  → accepted node kind contentHost
  → semantic attachment lease
  → structural viewContentHostCreate(port ID)
```

At the same time, the Port owns native content registry membership and selected Connector state.

This is not necessarily incorrect, but it is a consequential coupling:

- structural replacement controls whether the Port is mounted/visible
- content control calls activate/deactivate Connector state
- content Source mutation wakes hosts without rebuilding structure
- Port disposal can be blocked by structural attachment leases

### 8.4 Status shape intentionally hides native cleanup fields

Rust `ContentConnectorStatus` has:

```text
error
cleanup_pending
cleanup_error
```

Evidence: `crates/iyon-tui/src/application/content.rs:3204-3213`.

The N-API bridge emits only:

```text
phase
requested
visible
projectedSourceRevision
error
```

with cleanup error taking precedence:

- `crates/iyon-tui-native/src/tui.rs:53-72`
- `tui.rs:1404-1430`

The TypeScript test explicitly asserts that `cleanup_pending` and `cleanup_error` are absent:

- `packages/iyon-tui/tests/tui_perf13_d.test.ts:124-170`

This preserves the existing public status shape but loses structured distinction between operation and cleanup failure. Callers must use error codes and diagnostics.

### 8.5 One native artifact, two loading mechanisms

The source follows the historical same-artifact rule:

- `addon.ts` uses `require(nativeArtifact.absolutePath)`
- `ffi.ts` uses `dlopen(artifact.absolutePath, symbols)`
- `artifact.ts` is the shared resolver

Evidence:

- `packages/iyon-tui/src/transport/native/addon.ts:236-248`
- `packages/iyon-tui/src/transport/content/ffi.ts:250-287`
- `packages/iyon-tui/src/transport/native/artifact.ts:32-71`

This avoids a second native runtime/artifact, but it creates a packaging requirement: the staged `.node` must expose both its Node-API initialization surface and the direct C symbols.

### 8.6 Direct FFI is synchronous while host presentation is deferred

The Source payload operation itself is synchronous and returns a mutation result immediately. Presentation remains deferred:

```text
direct FFI Source mutation
  → native Source revision/storage update
  → native host pending mark
  → JS wake broker
  → native environment drain
  → candidate projection/commit
  → visible frame
```

This preserves the distinction between content acceptance and frame visibility. Tests depend on explicit `tui.flush()` to observe rendered output.

### 8.7 Potential static validation concern in `ffi.ts`

The inspected display for `packages/iyon-tui/src/transport/content/ffi.ts:501-504` is:

```ts
if (typeof value !== "string" || value.length === 0 || /\\s|\\0/u.test(value)) {
```

The error message says the function rejects whitespace or NUL. If the source literally contains two backslashes in the regular expression, it would match a backslash followed by `s` or `0`, not whitespace or NUL. The inspection output may have escaped the source representation, so this is not classified as a confirmed defect. It should be source-checked before relying on this validation.

The analogous public text validator in `packages/iyon-tui/src/api/content/text.ts:124-127` displays `/\s/u`, which makes the difference worth verifying.

### 8.8 Smooth-option validation is split across layers

TypeScript allows finite non-negative numbers in `deliveryFor()`:

- `packages/iyon-tui/src/api/content/retained.ts:520-543`

Native later constrains smooth values to f32-compatible ranges and validates `SmoothConfig`:

- `crates/iyon-tui-native/src/tui.rs:1537-1563`

Therefore a Funnel can be successfully constructed in TypeScript but fail at Connector creation. This is an explicit cross-boundary validation failure, not a silent fallback.

---

## 9. Open questions and coverage gaps

1. **Typed transport interface:** Was the historical `ContentDataTransport` interface intentionally removed, or is the current direct import from `api/content/retained.ts` migration residue?
2. **Regex spelling:** Does `ffi.ts:502` literally contain doubled backslashes, or did the inspection representation escape them?
3. **Staged artifact availability:** `addon.ts` eagerly requires the native artifact during module import. The current static inspection did not execute package loading, so artifact availability in the baseline environment was not validated.
4. **Direct symbol visibility:** The same `.node` artifact must expose Node-API initialization and six direct symbols. The source declares this requirement, but no load probe was run here.
5. **Cross-realm behavior:** The registry/environment is keyed through `Symbol.for` values on `globalThis`, while native environment lookup is keyed by N-API `Env`. Behavior across multiple JavaScript realms/workers is not established by this report.
6. **Weak finalizer timing:** Resource registry finalization is nondeterministic. The source protects outstanding leases, but exact reclamation timing was not experimentally observed.
7. **Source mutation concurrency:** The historical contract says native Source mutex/revision is the linearization order. The current code was not exercised concurrently in this investigation.
8. **Native projection details:** Full projection, smoothing, viewport, and cache behavior resides in `crates/iyon-tui/src/application/content.rs`; this report cites keys and state fields but does not replace the detailed Rust content/projection reports.
9. **Host close versus pending connector cleanup:** The TypeScript close order is explicit, but no runtime test was run here to observe all combinations of pending Connector cleanup, host invalidation, and wrapper finalization.
10. **Status-error precedence:** Native cleanup errors take precedence over operating errors in the emitted `error` field. The exact cases where both coexist and how long an old operating diagnostic remains are native-core questions.
11. **Source retention accounting:** TypeScript exposes native `acceptedBytes`, `copiedBytes`, and `droppedHeadBytes`, but no TypeScript-level performance interpretation or allocation/RSS correlation is present.
12. **Other transport LOC:** Structural and state files were inventoried and sampled for their content attachment edges, but their exact physical LOC were not measured with an executed line-count command in this read-only inspection.

No unsupported claim is made that any transport file is unused or removable.

---

## 10. Evidence appendix

### 10.1 Primary assigned production files read

```text
packages/iyon-tui/src/transport/content/abi.ts
packages/iyon-tui/src/transport/content/control.ts
packages/iyon-tui/src/transport/content/ffi.ts

packages/iyon-tui/src/transport/native/addon.ts
packages/iyon-tui/src/transport/native/artifact.ts
packages/iyon-tui/src/transport/native/factories.ts
packages/iyon-tui/src/transport/native/resource-registry.ts
packages/iyon-tui/src/transport/native/resources.ts
```

### 10.2 Immediate TypeScript consumers read

```text
packages/iyon-tui/src/api/content/retained.ts
packages/iyon-tui/src/api/content/text.ts
packages/iyon-tui/src/api/errors.ts
packages/iyon-tui/src/api/controls/framework-handle.ts

packages/iyon-tui/src/runtime/environment.ts
packages/iyon-tui/src/runtime/handle-registry.ts
packages/iyon-tui/src/runtime/native-resource-registry.ts
packages/iyon-tui/src/runtime/runtime.ts
packages/iyon-tui/src/runtime/attachments.ts
packages/iyon-tui/src/runtime/wake-broker.ts
```

### 10.3 Remaining transport files indexed

```text
packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts
packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json
packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts
packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json

packages/iyon-tui/src/transport/state/control.ts
packages/iyon-tui/src/transport/state/generated/state_envelope.ts

packages/iyon-tui/src/transport/structural/component-id.ts
packages/iyon-tui/src/transport/structural/encoding.ts
packages/iyon-tui/src/transport/structural/ir.ts
packages/iyon-tui/src/transport/structural/native-view-abi.ts
packages/iyon-tui/src/transport/structural/policy.ts
packages/iyon-tui/src/transport/structural/retained-dag.ts
packages/iyon-tui/src/transport/structural/retained-path.ts
packages/iyon-tui/src/transport/structural/style-lowering.ts
```

### 10.4 Native bridge/core files sampled or read for seam verification

```text
crates/iyon-tui-native/src/content_ffi.rs
crates/iyon-tui-native/src/tui.rs
crates/iyon-tui-native/src/error.rs
crates/iyon-tui-native/src/lib.rs

crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/application/environment.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/binding/mod.rs
```

### 10.5 Tests inspected

```text
packages/iyon-tui/tests/tui_perf13_d.test.ts
packages/iyon-tui/tests/tui_perf13_h.test.ts
packages/iyon-tui/tests/tui_handles.test.ts
packages/iyon-tui/tests/tui_ansi_scanner.test.ts
packages/iyon-tui/tests/tui_smooth_delivery.test.ts
packages/iyon-tui/tests/tui_semantic_cache_ownership.test.ts
packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts
packages/iyon-tui/tests/tui_h3_a_semantic.test.ts
```

### 10.6 Historical contract files consulted

```text
PRE-V5-ARCHITECTURE-REPORT.md
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/evidence/assignments.json
docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt
docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md
```

### 10.7 Key symbol/path references

```text
packages/iyon-tui/src/transport/content/abi.ts
  CONTENT_STATUS
  decodeContentAbiMetadata
  CONTENT_ABI_* constants

packages/iyon-tui/src/transport/content/control.ts
  createTextSource
  createContentPort
  connectContent
  activateContent
  deactivateContent
  disposeContentConnector
  contentConnectorStatus

packages/iyon-tui/src/transport/content/ffi.ts
  ContentFfiSymbols
  ContentFfiSession
  validateMetadata
  openSession
  sourceIdentity
  encodeAnnotations
  encodeSemanticStyle
  decodeSemanticStylePayload
  invokePayload
  invokeNoPayload
  finishMutation
  appendTextSource
  replaceTextSource
  clearTextSource
  sealTextSource
  truncateTextSource

packages/iyon-tui/src/transport/native/addon.ts
  NativeTextSourceContract
  NativeContentPortContract
  NativeContentConnectorContract
  NativeTuiHostContract
  NativeTuiAddon
  nativeArtifact
  native
  requireNativeClass

packages/iyon-tui/src/transport/native/artifact.ts
  NATIVE_PACKAGE_BUILD_ID
  nativeArtifactName
  resolveNativeArtifact

packages/iyon-tui/src/transport/native/resource-registry.ts
  PreparedResourceLease
  NativeResourceRegistry
  prepareResolve
  beginDisposal
  cancelDisposal
  invalidateHost
  runtimeResourceEnvironment
  runtimeResourceRegistry

packages/iyon-tui/src/transport/native/resources.ts
  registerNativeResource
  nativeResourceForHandleId
  nativeResourceOf
  releaseNativeResource
  disposeNativeResource

packages/iyon-tui/src/runtime/runtime.ts
  Tui.contentPort
  disposeRetainedExecution
  commitVisibleAfterDrain
  close
  exit

packages/iyon-tui/src/runtime/attachments.ts
  prepareSemanticAttachments
  PreparedAttachmentSet
  addAttachment

packages/iyon-tui/src/transport/structural/retained-dag.ts
  materializeContentHostNode
  viewContentHostCreate

crates/iyon-tui-native/src/content_ffi.rs
  source_for_identity
  run_payload_mutation
  run_no_payload_mutation
  guarded
  iyon_tui_perf13_abi_metadata_v1
  iyon_tui_source_append_utf8_v1
  iyon_tui_source_replace_utf8_v1
  iyon_tui_source_clear_v1
  iyon_tui_source_seal_v1
  iyon_tui_source_head_truncate_v1

crates/iyon-tui-native/src/tui.rs
  host_environment_for_env
  NativeTextSource
  NativeContentPort
  NativeContentConnector
  parse_text_source_options
  parse_text_funnel_control
  content_connector_status_value

crates/iyon-tui/src/application/content.rs
  HostContentSource
  HostContentSourceSnapshot
  HostContentSourceStats
  ContentMutationResult
  HostContentFunnel
  ContentConnectorStatus
  ContentSourceRegistry
  ContentHostRegistry
  append_utf8
  replace_utf8
  clear
  seal
  truncate_head
  prepare_content_commit
```

### 10.8 LOC methodology

- Assigned TypeScript counts are approximate physical source spans based on the checked-in line labels shown by source inspection.
- Tests are reported separately and were not included in production counts.
- Generated ABI/schema bodies are not included in production counts.
- No build, test, benchmark, or runtime execution was performed.
- Historical test/completion claims are labeled as historical evidence and are not current-run validation.