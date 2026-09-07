# 10 — Source storage, coordinates, connectors, ports, snapshots and actual post-L1 ownership

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Investigation mode: read-only static inspection.
- No source files, configuration, dependencies, running services, or generated artifacts were changed.
- No tests or build commands were executed in this investigation. Historical test results cited below are explicitly identified as historical evidence from repository documents.

The assignment scope names `crates/iyon-tui/src/stream/` recursively. The most consequential discovery is that the current `stream/` directory is intentionally minimal:

```text
crates/iyon-tui/src/stream/
├── coord.rs
└── mod.rs
```

The actual post-L1 Source storage, Source mutation API, snapshots, Port/Connector registries, delivery state, and frame transaction ownership live elsewhere:

```text
crates/iyon-tui/src/stream/coord.rs
    └── opaque coordinate values only

crates/iyon-tui/src/application/content.rs
    ├── Source public handles and mutation boundary
    ├── Source registry and Source records
    ├── snapshots and statistics
    ├── Funnel values
    ├── Port/Connector records and registries
    ├── Connector-local projection, semantic, paint, and delivery state
    ├── Source subscription/wake bookkeeping
    └── ContentProvider integration

crates/iyon-tui/src/application/source_store.rs
    ├── persistent chunk/page storage
    ├── line indexing
    ├── persistent annotation index
    ├── retention/truncation primitives
    └── immutable storage snapshots

crates/iyon-tui/src/application/environment.rs
    ├── environment-owned Source registry lifetime
    └── pending-host/source-wake coordination

crates/iyon-tui/src/application/host.rs
    ├── host-owned ContentHostRegistry lifetime
    ├── candidate/frame orchestration
    └── visible commit and rollback barrier

crates/iyon-tui-native/src/content_ffi.rs
    └── direct high-volume Source data ABI

crates/iyon-tui-native/src/tui.rs
    └── N-API Source/Port/Connector control wrappers
```

This physical placement is important for the current architecture census: the `stream` namespace is a public coordinate namespace, not the owner of a streaming runtime. The current implementation follows the post-L1 content-plane ownership documented by PERF-13-H: Source storage is environment-owned, while Port and Connector state is host-owned.

### Evidence inspected

Primary current-source evidence:

- `crates/iyon-tui/src/stream/mod.rs`
- `crates/iyon-tui/src/stream/coord.rs`
- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/source_store.rs`
- `crates/iyon-tui/src/application/environment.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/projection/value.rs`
- `crates/iyon-tui/src/projection/projector.rs`
- `crates/iyon-tui/src/projection/smooth.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui/src/binding/mod.rs`
- selected TypeScript API/transport references in `packages/iyon-tui/src/api/content/retained.ts`, `packages/iyon-tui/src/index.ts`, and content transport files.

Required contextual documents:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/history/PERF-13/PERF-13-D-completion.md`
- `docs/history/PERF-13/PERF-13-E-completion.md`
- `docs/history/PERF-13/PERF-13-F-completion.md`
- `docs/history/PERF-13/PERF-13-G-completion.md`
- `docs/history/PERF-13/PERF-13-H-completion.md`
- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`
- `AGENTS.md`

### Facts, inferences and unknowns

**Facts from current source:**

- `stream/` exports `StreamOffset` and `StreamRange`; it does not define Source, Funnel, Connector, Port, snapshot, scheduler, or pane types.
- Source mutation is serialized through `Arc<Mutex<ContentSourceRecord>>`.
- Source storage is persistent and snapshot-friendly, using immutable page/chunk roots and an indexed annotation tree.
- Source identity and Source storage are environment-owned.
- Port and Connector identity/state are host-owned.
- Content projection is integrated into host measure/place/paint candidate preparation.
- Old Rust `TextStream`, `StreamSnapshot`, `StreamPane`, `StreamingSource`, `StreamRevision`, `HostTextStream`, and `NativeTextStream` symbols are absent from the current Rust source tree.
- Connector control and Source mutation are explicit operations; dropping a Connector wrapper is not itself a lifecycle operation.

**Static inferences:**

- A Source snapshot can safely outlive the Source mutex critical section because it clones an `Arc<StoredSource>` root.
- A frame candidate can use a consistent Source revision even if the Source mutates while candidate preparation is underway because `ContentHostRegistry` captures one snapshot per Source per candidate.
- The persistent storage strategy is designed to make append and head truncation proportional to changed tree paths rather than the entire retained Source, although text projection and diagnostics can still scan/materialize retained content.
- The current implementation treats raw `u64` Source revisions and offsets as the runtime representation; the public projection API wraps offsets in `StreamOffset` but does not currently define a typed `StreamRevision`.

**Unknowns/limits:**

- No runtime validation was performed for this report.
- The exact generated ABI and TypeScript transport implementation was indexed and sampled but not comprehensively audited; the native ABI entrypoints were inspected sufficiently to establish their routing and ownership.
- The current `ContentHostRegistry` is a large mixed implementation containing both content-plane logic and presentation/history adaptation. This report records that coupling but does not make a V5 disposition decision.

### Approximate LOC methodology

The counts below are approximate physical source ranges based on declaration/test boundaries visible in the inspected files, not compiler-derived counts:

| File | Approximate production LOC | Approximate test LOC | Method |
|---|---:|---:|---|
| `stream/coord.rs` | ~87 | ~26 | Test module begins at line 88 |
| `stream/mod.rs` | ~9 | 0 | Entire file is a module/barrel definition |
| `application/source_store.rs` | ~1,660 | ~700–800 | Production ends before the test module around line 1,665; tests extend beyond line 2,300 |
| `application/content.rs` | ~7,000 | ~3,700 | Production ends around line 7,009; tests begin at line 7,011 and extend past line 10,600 |
| Supporting `application/environment.rs`, `application/host.rs`, native ABI files | Not counted as assignment ownership | Not counted | Followed for seams and ownership only |

Generated code and TypeScript production/test LOC are not included in the Rust subsystem count.

---

## 1. Responsibility and structure

### 1.1 `stream/coord.rs`: coordinate value types only

`crates/iyon-tui/src/stream/coord.rs` defines two public types:

```rust
pub struct StreamOffset(pub(crate) u64);
pub struct StreamRange {
    pub(crate) start: StreamOffset,
    pub(crate) end: StreamOffset,
}
```

#### `StreamOffset`

Evidence: `stream/coord.rs:3-36`.

Responsibilities:

- Opaque coordinate within one Source root coordinate space.
- Total ordering and hashing.
- Zero value: `StreamOffset::ZERO`.
- Construction from a raw `u64`: `StreamOffset::new`.
- Conversion back to `u64`: `as_u64`.
- Checked arithmetic: `checked_add`.
- Saturating arithmetic: `saturating_add`.

The documentation explicitly states that text Sources conventionally use UTF-8 byte offsets, but arbitrary Sources could use record/event ordinals. Current projection APIs remain text/byte-specific and must use atomic or replacement boundaries for non-text coordinate systems.

The tuple field is `pub(crate)`, so external callers cannot access the raw field directly. The public constructor/accessor establish the intended API boundary.

#### `StreamRange`

Evidence: `stream/coord.rs:38-85`.

Responsibilities:

- Half-open range `[start, end)`.
- Construction with `StreamRange::new`, which asserts `start <= end`.
- Fallible construction with `try_new`.
- Accessors `start`, `end`.
- `is_empty`, `len`, and `contains_offset`.

The range itself has no Source identity. It is only valid relative to the Source root whose coordinate space supplied it. The type does not carry a Source ID, generation, or revision. That identity is carried by surrounding Source snapshots/projection envelopes.

#### What `stream/` does not own

A scoped source search found no current Rust definitions for:

- `StreamingSource`
- `TextStream`
- `StreamSnapshot`
- `StreamRevision`
- `StreamPane`
- `StreamScheduler`
- `HostTextStream`
- `NativeTextStream`

The current `stream` namespace therefore does not own mutable stream storage or stream lifecycle. Those responsibilities were moved into the generic content plane and storage implementation.

### 1.2 `application/source_store.rs`: persistent Source storage

Evidence: `application/source_store.rs:1-16`, `26-35`, `88-1663`.

`source_store.rs` owns the immutable/persistent representation behind a Source:

- validated UTF-8 input representation;
- immutable UTF-8 pages;
- chunk descriptors and persistent chunk tree;
- line/newline aggregates and line lookup;
- persistent annotation tree;
- Source base/end, seal, revision, and annotation sequence state;
- truncation/prefix operations;
- chunk-view export for zero-copy-ish frame ingestion.

It does not own:

- Source identity registry membership;
- Source lifecycle (`Live`/`Disposed`);
- host wake fanout;
- Connector membership;
- Connector projection or smoothing;
- host/theme/layout/paint state.

The storage module imports only generic style/tag types from the TUI crate:

```rust
use crate::StyleRef;
use crate::text::SemanticTag;
```

No terminal surface, layout tree, host, viewport, or backend types are imported by the persistent storage module.

### 1.3 `application/content.rs`: current content-plane implementation

Evidence: `application/content.rs:1-6`.

The module-level documentation is explicit:

> Source storage is deliberately host-independent. This module owns the PERF-13-E mutation boundary and the PERF-13-D lifecycle graph: environment-owned Sources, host-owned Ports and Connectors, desired/visible mount state, weak subscription bookkeeping, and the plain-text Connector projection.

In practice, the file owns substantially more than the original plain projection:

- Source/Funnel/Port/Connector value types;
- Source registry and Source record lifecycle;
- Source mutation validation and wake fanout;
- snapshots and statistics;
- semantic projection through plain, Markdown, diff, and ANSI projectors;
- Connector-local Smooth delivery;
- Connector-local semantic/projection/paint caches;
- candidate and committed projection state;
- ContentProvider implementation;
- History transfer adapter;
- Port/Connector mount and selection transitions;
- source membership and subscription cleanup;
- failure status and rollback handling.

This is the principal actual post-L1 owner even though the assignment directory is `stream/`.

### 1.4 Supporting owners

#### `application/environment.rs`

Evidence: `environment.rs:12-18`, `151-176`, `197-226`, `520-705`.

Owns:

- the environment-level `ContentSourceRegistry`;
- Source creation and lookup;
- environment identity/generation;
- pending-host set and wake epoch;
- deferred Source wake failures and host drain reporting.

#### `application/host.rs`

Evidence: `host.rs:1064-1084`, `1091-1120`, `1831-1855`, `1880-1934`, `2045-2102`, `2275-2285`.

Owns:

- the host-facing `create_content_port` API;
- host lifecycle and teardown;
- desired structural View publication;
- extraction/validation of ContentPort attachments;
- frame candidate creation;
- host pending/committed epochs;
- invoking `ContentHostRegistry` candidate preparation and commit;
- combining content, retained state, layout, paint, and backend receipt semantics.

#### `iyon-tui-native/src/content_ffi.rs`

Evidence: `content_ffi.rs:1-6`, `16-112`, `202-306`, `348-480`.

Owns the direct high-volume data ABI. It does not own Source state. It resolves environment/source identities and invokes `HostContentSource` operations.

#### `iyon-tui-native/src/tui.rs`

Evidence: `tui.rs:1090-1238`, `1240-1564`.

Owns N-API control wrappers and translates JavaScript values into Rust content values. The underlying Source/Port/Connector records remain owned by `iyon-tui`.

---

## 2. Types, APIs and contracts

### 2.1 Coordinate and projection contracts

`StreamOffset` and `StreamRange` are re-exported by `stream/mod.rs:7-9`.

They are consumed by:

- `projection/value.rs`;
- `projection/projector.rs`;
- `projection/validate.rs`;
- `projection/smooth.rs`;
- `application/content.rs`;
- projection tests.

`Projection<T>` stores:

```rust
source_base: StreamOffset
stable_through: StreamOffset
source_end: StreamOffset
sealed: bool
spans: Vec<ProjectionSpan<T>>
```

Evidence: `projection/value.rs:12-24`.

`ProjectionSpan<T>` pairs a `StreamRange` with zero or more projected values. A span with no values is explicit elision, not an uncovered source gap.

The builder API is public:

- `ProjectionBuilder::new`
- `emit`
- `emit_many`
- `elide`
- `finish`

The resulting projection exposes:

- `source_base`
- `stable_through`
- `source_end`
- `is_sealed`
- `spans`
- mapping/rebuild helpers.

Projection validation enforces contiguous source coverage, monotonic envelopes, and legal transitions. `StreamRange` is therefore the coordinate primitive used by both Source-derived text projection and generic incremental transformation/smoothing.

### 2.2 Content family/source/funnel values

Evidence: `application/content.rs:46-75`.

Current values:

```rust
pub enum ContentFamily {
    Text,
}

pub enum TextSourceKind {
    Block,
    Stream,
}

pub enum TextFunnelKind {
    Plain,
    Markdown,
    Diff,
    Ansi,
}

pub enum ContentDelivery {
    Immediate,
    Smooth(SmoothConfig),
}

pub enum TextWrapMode {
    Word,
    Grapheme,
    NoWrap,
}
```

`ContentFamily` is currently closed to text. `TextSourceKind` distinguishes replacement-style block Sources from appendable stream Sources. `TextFunnelKind` selects semantic transformation. `ContentDelivery` controls whether delivery is immediate or Connector-local smooth.

### 2.3 Ingestion annotation contract

Evidence: `application/content.rs:77-94`, `1813-1910`; `application/source_store.rs:26-35`.

The direct data ABI and Rust Source API use this fixed 32-byte C-compatible record:

```rust
#[repr(C)]
pub struct ContentAnnotationRecord {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub aux0: u32,
    pub aux1: u32,
}
```

Compile-time assertions pin size to 32 bytes and alignment to 4 bytes.

Current closed annotation kinds:

| Kind | Constant | Retention/projection behavior |
|---|---|---|
| Semantic tag | `CONTENT_ANNOTATION_KIND_TAG = 1` | Clip ranges at retention/prefix boundaries |
| Semantic style | `CONTENT_ANNOTATION_KIND_STYLE = 2` | Clip ranges at retention/prefix boundaries |
| Atomic | `CONTENT_ANNOTATION_KIND_ATOMIC = 3` | Drop if crossing a truncation/prefix boundary |
| Point | `CONTENT_ANNOTATION_KIND_POINT = 4` | Empty range, retained by position |

Ingress validation:

- limits annotation count to `MAX_SOURCE_ANNOTATIONS` (16,384);
- limits sidecar payload to `MAX_ANNOTATION_PAYLOAD_BYTES` (4 MiB);
- rejects unknown kinds;
- rejects nonzero reserved flags/auxiliary lanes;
- requires ordered UTF-8 scalar boundaries;
- requires Point annotations to have an empty range;
- requires non-Point annotations to cover nonempty text;
- validates sidecar payload ranges;
- decodes tags and semantic styles before storage;
- converts operation-local offsets into absolute Source coordinates while the Source record is locked.

### 2.4 Snapshot API

Evidence: `application/content.rs:108-187`, `2256-2276`.

```rust
pub struct HostContentSourceSnapshot {
    pub source_id: u64,
    pub source_generation: u32,
    pub content_generation: u64,
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub sealed: bool,
    pub head_partial: bool,
    storage: Arc<StoredSource>,
}
```

Public methods:

- `text() -> String`
- `annotations() -> Vec<ContentAnnotationSnapshot>`
- `retained_bytes() -> u64`
- `retained_lines() -> u64`
- `chunk_count() -> usize`

Internal frame-facing methods:

- `chunk_views()`
- `chunks()`
- `annotations_for_projection()`
- `stable_prefix()`

The snapshot is cheap to clone because the persistent storage is held through `Arc<StoredSource>`. Calling `text()` is an explicit materialization operation and is not used by the frame path. The frame path consumes chunk views and Source annotations directly.

A snapshot captures both identity and content state:

- `source_id` identifies the Source within the environment;
- `source_generation` protects stale Source identity;
- `content_generation` changes on logical replacement/clear;
- `revision` changes on every accepted mutation;
- `source_base` advances on head retention/truncation;
- `source_end` is the absolute end coordinate;
- `sealed` and `head_partial` describe lifecycle/retained-prefix properties.

### 2.5 Source public handle

Evidence: `application/content.rs:1784-1789`, `2197-2806`.

```rust
pub struct HostContentSource {
    registry: ContentSourceRegistry,
    record: Arc<Mutex<ContentSourceRecord>>,
}
```

Identity/lifecycle methods:

- `id()`
- `generation()`
- `environment_slot()`
- `environment_generation()`
- `family()`
- `kind()`
- `content_generation()`
- `is_live()`
- `dispose()`

Data/mutation methods:

- `snapshot()`
- `stats()`
- `append_utf8(...)`
- `replace_utf8(...)`
- `clear()`
- `seal()`
- `truncate_head(...)`

Retention:

- `configure_retention(...)`

Internal membership/subscription operations:

- `acquire_connector()`
- `release_connector()`
- `subscribe(...)`
- `unsubscribe(...)`

Source disposal is explicit. `dispose()` is idempotent after disposal, but rejects while `connector_count != 0` with `SOURCE_IN_USE`. Removing a Source from the environment registry occurs only after the Source record transitions to disposed.

### 2.6 Source statistics

Evidence: `application/content.rs:492-504`, `2279-2298`.

```rust
pub struct HostContentSourceStats {
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub retained_bytes: u64,
    pub retained_lines: u64,
    pub chunk_count: usize,
    pub sealed: bool,
    pub head_partial: bool,
    pub accepted_bytes: u64,
    pub copied_bytes: u64,
    pub dropped_head_bytes: u64,
}
```

The counters distinguish:

- accepted input bytes;
- bytes copied into Source storage;
- bytes dropped from the retained head;
- current retained extent and chunk count.

These counters are exposed to N-API Source statistics and are also useful observability for determining whether the persistent storage fast path is being exercised.

### 2.7 Mutation result

Evidence: `application/content.rs:506-515`.

```rust
pub struct ContentMutationResult {
    pub revision: u64,
    pub environment_wake_epoch: u64,
    pub schedule_environment_drain: bool,
}
```

`revision` is authoritative Source mutation ordering. `environment_wake_epoch` and `schedule_environment_drain` are scheduler hints/coordination outputs, not replacement sequencing tokens.

The code explicitly avoids returning a normal mutation failure after Source bytes have already been installed. Post-acceptance host wake failures are recorded through the environment-side failure channel.

### 2.8 Funnel

Evidence: `application/content.rs:1469-1513`.

```rust
pub struct HostContentFunnel {
    pub family: ContentFamily,
    pub kind: TextFunnelKind,
    pub wrap: TextWrapMode,
    pub hyperlinks: bool,
    pub delivery: ContentDelivery,
}
```

It is an immutable Source-neutral specification. It contains no:

- Source identity;
- active execution state;
- viewport;
- host;
- projection cache;
- parser;
- smoothing clock.

`HostContentFunnel::plain` constructs an immediate plain-text Funnel with hyperlinks enabled. `HostContentFunnel::new` accepts the complete normalized configuration. `smooth_config()` converts delivery policy into an optional `SmoothConfig`.

### 2.9 Port and Connector handles

Evidence: `application/content.rs:6707-6787`, `6791-7007`.

`HostContentPort` is a host-owned mount point with:

- ID;
- generation;
- content family;
- `Arc<Mutex<PortRecord>>`;
- weak host reference.

Public methods:

- `id()`
- `generation()`
- `family()`
- `deactivate()`
- `connect(source, funnel)`
- `dispose()`
- `is_mounted()`

`HostContentConnector` is a host-owned link between exactly one Source, one Funnel, and one Port. Public methods:

- `id()`
- `generation()`
- `source_id()`
- `activate()`
- `deactivate()`
- `dispose()`
- `fail_next_activation(...)` — test/native fixture only;
- `status()`
- `visible_delivery_frontier()`
- `candidate_delivery_frontier()`
- `is_disposed()`.

The wrapper’s `Drop` implementation is intentionally a no-op. Explicit disposal or host teardown owns semantic release:

```rust
impl Drop for HostContentConnector {
    fn drop(&mut self) {
        // Explicit disposal owns semantic release.
    }
}
```

This prevents an arbitrary wrapper drop from silently mutating Source membership or host-visible state.

---

## 3. Dependency and ownership map

### 3.1 Ownership table

| Resource/concept | Created by | Authoritative owner | Destroyed/released by | Identity/lifetime |
|---|---|---|---|---|
| `StreamOffset`, `StreamRange` | callers/projection builders | value | value drop | no resource lifetime |
| Source registry | `TuiEnvironment` | environment | environment teardown | environment lifetime |
| `ContentSourceRecord` | `ContentSourceRegistry::create` | environment registry | explicit Source disposal after membership release, or environment teardown | `SourceId + source_generation` |
| `StoredSource` | Source mutation | Source record | replaced by new `Arc`, old snapshots retain old root | persistent immutable root |
| Source snapshot | `HostContentSource::snapshot` | snapshot holder / candidate | `Arc` drop | captures one Source revision |
| ContentPort | `TuiHost::create_content_port` | host `ContentHostRegistry` | explicit Port disposal or host teardown | host-owned ID/generation |
| Connector | `HostContentPort::connect` | host `ContentHostRegistry` | explicit Connector disposal, candidate finalization, or host teardown | host-owned ID/generation |
| Funnel | caller / N-API control layer | immutable value | value drop | Source-neutral |
| Source wake subscription | Connector activation/mount state | Source record grouped by host | Connector unsubscribe/disposal or Source membership release | weak host + Connector ID/generation |
| semantic projection | Connector preparation | Connector record/cache | cache eviction, deactivation, disposal | keyed by Source/content/Funnel state |
| Smooth delivery state | Connector demand | Connector record | Connector deactivation/disposal/cold transition | Connector-local |
| visible projection | host candidate commit | Connector record | replacement/unmount/disposal | committed frame lifetime |
| candidate projection | frame candidate | candidate/Connector record | candidate abort or promotion | receipt barrier |
| scroll state | ScrollPane/RowViewport | component/control layer | ScrollPane lifecycle | not Port/Connector-owned |
| History adapter state | host content registry | host/history integration | host teardown/history unit retirement | Port-to-History association |

### 3.2 Forward dependency direction

```text
TuiEnvironment
    │
    ├── EnvironmentIdentity
    ├── ContentSourceRegistry
    │       │
    │       └── ContentSourceRecord
    │               ├── Source identity/lifecycle
    │               ├── Arc<StoredSource>
    │               │       ├── persistent ChunkTree
    │               │       └── persistent AnnotationTree
    │               └── host-grouped weak subscriptions
    │
    └── live-host registry / pending-host wake broker

TuiHost / HostInner
    │
    ├── ContentHostRegistry
    │       ├── PortRecord
    │       │       ├── desired mount
    │       │       ├── visible mount
    │       │       ├── desired Connector
    │       │       └── visible Connector
    │       │
    │       ├── ConnectorRecord
    │       │       ├── HostContentSource handle
    │       │       ├── immutable HostContentFunnel
    │       │       ├── requested/visible lifecycle
    │       │       ├── ConnectorExecution
    │       │       │       ├── Markdown/Diff/ANSI parser
    │       │       │       ├── Smooth delivery
    │       │       │       └── TextRenderer
    │       │       ├── semantic/projection/paint caches
    │       │       └── committed/candidate products
    │       │
    │       ├── candidate Source snapshots
    │       ├── active Connector/deadline indexes
    │       └── HistoryTerminalAdapter
    │
    └── host frame transaction
            ├── semantic candidate
            ├── Source snapshot capture
            ├── Connector projection
            ├── measure/place
            ├── paint
            └── receipt-time commit/abort

Direct Source ABI / N-API control
    └── resolve environment + Source identity
            └── HostContentSource mutation
```

### 3.3 Reverse dependencies

- `projection` depends on `stream::{StreamOffset, StreamRange}` but does not depend on Source storage.
- `application/content.rs` depends on `stream` coordinates, `projection`, semantic text projectors/renderers, presentation ContentProvider types, and host/environment.
- `source_store.rs` is depended on by `application/content.rs`; it does not depend back on content registries.
- `host.rs` depends on `ContentHostRegistry` for content attachment validation, candidate preparation, and commit.
- `environment.rs` depends on `ContentSourceRegistry` for Source creation/lookup and wake failure draining.
- native direct FFI depends on `binding` exports and environment lookup; it does not invoke projection/layout/paint inside a payload mutation.
- structural View lowering carries a ContentPort attachment identity, but does not carry Source bytes, snapshots, Connector control, or host-native styles.

### 3.4 Important separation: Source membership versus Source wake subscription

The Source record has two distinct concepts:

1. **Connector membership** — counted by `connector_count`; retained even when a Connector is inactive/cold.
2. **Wake subscription** — a host-grouped list of active Connector ID/generation tokens used to wake only relevant hosts on Source mutation.

Evidence:

- `ContentSourceRecord.connector_count`: `application/content.rs:1566-1587`.
- `acquire_connector`/`release_connector`: `2714-2738`.
- `subscribe`/`unsubscribe`: `2741-2794`.
- Connector field comments: `2978-2985`.

This distinction allows an unmounted Port to retain its Source/Funnel/Connector relationship while avoiding parser, projection, delivery, and Source wake work.

---

## 4. Execution paths and state transitions

### 4.1 Source creation

1. `TuiEnvironment` initializes an environment-owned `ContentSourceRegistry`.
   - Evidence: `application/environment.rs:151-176`.
2. `TuiEnvironment::create_content_source(kind)` forwards to the registry.
   - Evidence: `application/environment.rs:197-226`.
3. `ContentSourceRegistry::create`:
   - increments Source ID;
   - caps Source IDs at `u32::MAX` for FFI compatibility;
   - increments Source generation;
   - creates a live `ContentSourceRecord`;
   - initializes `content_generation = 1`, `revision = 0`;
   - installs `Arc<StoredSource::empty()>`;
   - inserts the record strongly into the environment registry.
   - Evidence: `application/content.rs:1710-1780`.

A Source can therefore outlive any host, and later hosts in the same environment can reuse it. A Source cannot connect to a host from a different environment: `ContentHostRegistry::connect` compares Source and host Source registry identity and returns `WRONG_ENVIRONMENT`.

### 4.2 Append path

Primary path: `HostContentSource::append_utf8`.

Evidence: `application/content.rs:2301-2384`.

```text
caller / direct ABI / N-API
    ↓
HostContentSource::append_utf8
    ↓
Source record mutex
    ├── ensure live
    ├── require Stream kind
    ├── reject sealed
    ├── validate payload size
    ├── validate UTF-8 once
    ├── preflight absolute coordinate addition
    ├── decode/validate annotations
    ├── preflight retention
    └── install storage + revision
    ↓
release Source lock
    ↓
capture grouped host subscriptions
    ↓
wake each eligible host
    ↓
HostInner::mark_content_pending
    ↓
pending host epoch
    ↓
candidate frame on environment drain
```

The payload is operation-local. `base = record.storage.end()` is captured while the Source lock is held. Annotation local offsets are converted to absolute offsets using that base.

Successful append behavior:

- no input bytes and no annotations: no-op returning current revision;
- annotation-free append may use `Arc::get_mut` and `StoredSource::append_in_place`;
- shared snapshots force the persistent `apply_append` path;
- retention may replace the new storage root with a truncated root;
- Source revision is installed only after all fallible validation/storage construction succeeds;
- accepted/copied/dropped counters are updated;
- subscriptions are captured after mutation;
- wake fanout occurs after releasing the Source lock.

The Source lock is not held while host locks are acquired. This avoids lock-order inversion and permits independent hosts to be attempted.

### 4.3 Replace path

Evidence: `application/content.rs:2386-2424`.

`replace_utf8`:

- works for Block and Stream Sources;
- rejects replacement on a sealed Stream;
- validates UTF-8 and annotations;
- increments `content_generation`;
- creates a new `StoredSource::empty()` and installs the replacement;
- increments Source revision;
- preserves Source ID and Source generation;
- resets Source absolute base to zero through the new storage;
- retains old snapshots through their existing `Arc<StoredSource>` roots.

The Connector parser lineage includes Source ID, Source generation, and content generation. Therefore replacement starts a new logical parser document even if the replacement text happens to have coordinates similar to the old document.

### 4.4 Clear path

Evidence: `application/content.rs:2426-2458`.

`clear`:

- rejects clearing a sealed Stream;
- is a no-op if storage is already empty;
- preflights revision and content-generation counters before installing empty storage;
- increments `content_generation`;
- increments Source revision;
- wakes subscribers after accepting the mutation.

Clear is thus a logical replacement/reset, not merely a byte deletion.

### 4.5 Seal path

Evidence: `application/content.rs:2460-2485`; storage implementation `source_store.rs:1221-1264`.

`seal`:

- only applies to Stream Sources;
- rejects already sealed Sources;
- preflights the revision before changing the sealed flag;
- uses `StoredSource::apply_seal`;
- records `sealed = true` and `sealed_at = Some(at)`;
- can optionally store an atomic marker through the lower-level storage API, although the public Source method currently passes `None`.

The storage implementation deliberately installs the seal state before the optional marker in the copied next value, so a concurrent snapshot cannot observe a marker without the seal.

### 4.6 Head truncation path

Evidence: `application/content.rs:2487-2524`; `source_store.rs:1334-1371`.

`truncate_head(offset)`:

- requires `offset` within `[source_base, source_end]`;
- requires a UTF-8 scalar boundary;
- is a no-op at the current base;
- preflights Source revision;
- advances `source_base` without renumbering absolute coordinates;
- computes `head_partial` when the retained suffix starts in the middle of a logical line;
- updates dropped-head accounting;
- wakes subscribers.

The source coordinate space is therefore append-only in the forward direction but may have a moving retained floor. Later appends continue at the existing absolute `source_end`, not at zero.

### 4.7 Source wake fanout

Evidence: `application/content.rs:2526-2623`.

`finish_mutation`:

- receives grouped weak host references and Connector ID/generation tokens;
- attempts every host group;
- asks each host to validate that each token still represents a live Source subscription at the accepted revision;
- creates targeted `ContentDirty` records;
- marks host content work pending;
- records post-acceptance wake failures in the environment-side failure channel;
- does not convert a successfully installed Source mutation into an ordinary mutation error.

The wake path is edge-triggered scheduler coordination. Native Source revision and host epochs remain authoritative.

### 4.8 Port/Connector creation

#### Port

Evidence: `application/host.rs:1064-1075`; `application/content.rs:3346-3381`.

`TuiHost::create_content_port` obtains the host lock and delegates to `ContentHostRegistry::create_port`.

A Port starts with:

```text
lifecycle = Live
desired_mounted = false
visible_mounted = false
desired_connector = None
visible_connector = None
connector_ids = {}
```

Port IDs are allocated through a global atomic `NEXT_CONTENT_PORT_ID`, not a host-local counter. This prevents a ContentPort attachment from aliasing a Port from another host. Port generation is host-registry generation state.

#### Connector

Evidence: `application/content.rs:3383-3488`.

`HostContentPort::connect(source, funnel)` validates:

- Port is live and owned by the host;
- Source belongs to the same environment;
- Source is live;
- Markdown Funnels are not connected to a Source that has been logically truncated or uses drop-oldest retention;
- Content family matches among Port, Source, and Funnel.

After validation:

- Source `connector_count` is incremented;
- Connector ID and generation are allocated;
- Connector record is initialized as live, idle, invisible, unsubscribed;
- projection/paint/semantic/prefix caches are empty;
- no execution/parser/delivery state is created yet;
- Connector is inserted into the host Connector registry and Port membership set.

The `ConnectorExecution` is lazy. It is created when an active projection is demanded, not merely when a Connector is connected.

### 4.9 Desired/visible Port binding

Evidence: `application/content.rs:3490-3584`; `application/host.rs:1094-1120`.

When a View is published:

1. `HostInner::set_desired_view` extracts ContentPort attachment IDs from the desired semantic View.
2. `ContentHostRegistry::validate_targets` rejects:
   - duplicate ContentPort attachment;
   - stale/non-host-owned Port;
   - disposed Port.
3. `ContentHostRegistry::set_desired` scans the host’s Ports and changes desired mount state.
4. Changed Port bindings receive a monotonic binding revision.
5. Requested Connector phases are updated based on whether the Port is desired-mounted.
6. Mounted requested Connectors are subscribed for Source wakes.
7. Unmounted requested Connectors lose active deadline eligibility and, if not visible/in-flight, their wake subscription.

The desired binding is not immediately visible. The visible binding remains the previous committed binding until a successful frame receipt.

### 4.10 Candidate frame and Source snapshot capture

Evidence: `application/content.rs:3565-3584`, `3833-3849`; `application/host.rs:1880-1934`.

At candidate start:

- `begin_projection_candidate` enables Source capture;
- pending Port binding changes are copied into candidate binding sets;
- candidate Source snapshot map is cleared;
- stale candidate projections are discarded;
- pending cleanup Connector IDs are included in candidate touched state.

The first projection/key query for a Source obtains `HostContentSource::snapshot()`, then stores one snapshot keyed by Source ID in `candidate_source_snapshots`. Later queries in the same candidate reuse that snapshot.

This makes Source snapshot acquisition a candidate boundary:

```text
candidate starts
    ↓
first demand for Source S
    └── capture S at revision R
later demands for S
    └── reuse exact snapshot R
Source mutates during candidate
    └── mutation wakes next host epoch; current candidate stays coherent
```

The test `candidate_reuses_one_source_snapshot_for_revision_queries` at `application/content.rs:9655-9686` directly asserts this behavior.

### 4.11 Connector projection and semantic stages

Evidence: `application/content.rs:611-730`, `746-797`, `1021-1257`, `3934-4088`.

The Source-to-projection path is:

```text
HostContentSourceSnapshot
    ↓
source_projection
    └── chunk views → RawText page slices
    ↓
Funnel-selected projector
    ├── PlainTextProjector
    ├── MarkdownProjector
    ├── DiffProjector
    └── AnsiProjector
    ↓
SourceAnnotationRewriter
    └── semantic tags/styles applied to semantic runs
    ↓
Projection<TextContent>
    ↓
TextRenderer
    ↓
terminal layout tree / physical rows
```

`source_projection` uses chunk views backed by persistent Source pages and emits one `Projection` span per chunk. It does not materialize the complete Source String.

`source_grapheme_projection` is used for smoothing. It iterates Source chunks, carries the final grapheme across chunk boundaries, and emits absolute Source ranges for complete graphemes. The carry is necessary because a Unicode grapheme may span append/chunk boundaries.

`project_semantic_snapshot`:

- resets parser instances only when Source lineage changes;
- preserves parser state across compatible revisions;
- lowers Plain, Markdown, Diff, or ANSI through generic semantic text projectors;
- applies Source annotations after semantic transformation;
- does not resolve semantic styles to host-native style IDs in Source storage.

### 4.12 Candidate projection commit

Evidence: `application/content.rs:4574-4960`, `4967-5051`, `5188-5506`.

Candidate preparation captures:

- changed Port records;
- old and next Connector records;
- Source records needed for cleanup;
- candidate projection `Arc`s;
- delivery frontiers and delivery input state;
- deadlines;
- binding revisions.

The preparation function is the only point where live Port/Connector maps are resolved for the commit. The resulting `PreparedContentCommit` owns the `Arc` references needed for receipt-time commit.

Before visible mutation, commit preflights all Port, Connector, and Source locks. After that line, the intended commit body avoids ordinary fallible operations, handle lookup, registry-wide scans, and allocation growth.

At commit:

- candidate projection is copied into `committed_projection`;
- committed Source revision is updated from the prepared projection;
- committed delivery frontier is updated from the prepared candidate;
- old visible Connector is hidden and its derived state is cleared unless a newer control mutation requires preservation;
- Port `visible_mounted` and `visible_connector` are swapped;
- new Connector becomes visible;
- binding revision is removed from pending only if the desired/visible state now matches that exact revision;
- Source cleanup is attempted after logical promotion;
- cleanup failure retains Source membership/subscription for retry and exposes `cleanup_pending`.

### 4.13 Candidate abort and fallback

Evidence: `application/content.rs:5073-5118`, `5134-5170`; `application/content.rs:3894-3920`, `4261-4340`.

On candidate abort:

- candidate projections are cleared;
- in-flight Connector leases are released;
- non-visible, no-longer-requested/unmounted Connectors lose provisional subscriptions;
- disposed Connector identities can be finalized;
- candidate Source snapshots and binding overlays are discarded;
- visible bindings remain unchanged.

When a requested Connector fails projection/activation:

- the failed requested Connector records an error and failure key;
- the old visible Connector remains selected if one exists;
- the Port does not expose a gap in the visible frame;
- no-active-Connector cases remain empty;
- retries require a legitimate retry boundary, such as a newer Source revision, remount, or relevant width change;
- the same failed input is blocked from repeatedly retrying within the same candidate.

This is a recovery/fallback path, not a second content architecture: it terminates in the same ContentHostRegistry and candidate commit machinery.

### 4.14 Smooth delivery/tick path

Evidence: `application/content.rs:354-420`, `3586-3695`, `3715-3773`.

`ConnectorDelivery` owns:

- `Smooth`;
- grapheme-unit `Projection<TextContent>`;
- indexed Source generation/revision/sealed state;
- candidate delivery frontier.

On Source input:

- the delivery path regenerates grapheme units only if Source generation/revision/seal changed;
- `Smooth::project` receives the new units;
- the current frontier is retained.

On a due tick:

- `ContentHostRegistry::advance(now)` operates only on active Connector/deadline indexes;
- `Smooth::advance(now)` progresses delivery;
- no Source parser or Source storage mutation occurs;
- only the Connector’s derived candidate projection is invalidated;
- delivery revision increments;
- a targeted `DeliveryVisibility` dirty record schedules a host frame;
- projection and prepared-paint caches are not cleared merely because delivery advanced.

The active deadline index excludes cold/unmounted Connectors, so native ticks do not scan or process inactive membership.

### 4.15 History route

Evidence: `application/content.rs:2808-2950`, `6242-6384`, and tests around `8550+`, `9689+`, `9775+`.

History-backed content is no longer a separate stream storage route. The `HistoryTerminalAdapter` associates a Port with a History unit and tracks:

- History unit ID;
- insets;
- committed total rows;
- committed content rows;
- leading/trailing padding rows.

The same ContentProvider path supplies measurements and rows for ordinary ContentHost and History content. History transfer can:

- use finalized-prefix products for open streams;
- use sealed products for sealed content;
- preserve leading/trailing padding in the adapter;
- retire the History unit and dispose its associated Port after row acceptance.

Scroll offset/follow state remains owned by ScrollPane/RowViewport, not by Port or Connector.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production path matrix

| Semantic operation | Production entrypoints | Authoritative implementation | Alternate route status | Failure semantics |
|---|---|---|---|---|
| Create Source | `TuiEnvironment::create_content_source`; N-API `NativeTextSource::create` | `ContentSourceRegistry::create` | No old Source registry found | Registry lock/identity exhaustion |
| Append UTF-8 | `HostContentSource::append_utf8`; N-API `NativeTextSource::append`; direct `iyon_tui_source_append_utf8_v1` | `HostContentSource::append_utf8` → `StoredSource::apply_append`/fast path | Direct ABI and N-API converge on same Source record | Invalid UTF-8/range/payload/annotation/sealed/coordinate errors occur before installation |
| Replace | `HostContentSource::replace_utf8`; N-API replace; direct `iyon_tui_source_replace_utf8_v1` | `HostContentSource::replace_utf8` | No parallel storage path | New content generation; old snapshots remain valid |
| Clear | `HostContentSource::clear`; N-API clear; direct `iyon_tui_source_clear_v1` | `StoredSource::empty` installed through Source record | No old stream setter found | Sealed Stream/revision/content-generation errors before swap |
| Seal | `HostContentSource::seal`; N-API seal; direct `iyon_tui_source_seal_v1` | `StoredSource::apply_seal` | No separate stream finalization route found | Invalid state/already sealed/revision errors |
| Head truncate | `HostContentSource::truncate_head`; N-API truncate; direct `iyon_tui_source_head_truncate_v1` | `StoredSource::apply_truncate` | No legacy truncation store found | Range and UTF-8 boundary errors before movement |
| Snapshot | `HostContentSource::snapshot`; N-API `NativeTextSource::snapshot` | `Arc<StoredSource>` snapshot | N-API materializes diagnostic text; frame path does not | Disposed/poisoned Source errors |
| Source stats | `HostContentSource::stats`; N-API `stats` | Source record + StoredSource counters | No alternate counter source | Disposed/poisoned Source errors |
| Connect | `HostContentPort::connect`; N-API `connectContent` | `ContentHostRegistry::connect` | No Source-bound Funnel intermediate | Wrong environment, stale Port, disposed Source, retention incompatibility, family mismatch |
| Activate | `HostContentConnector::activate`; N-API activate | Host requested-state transition | No post-layout activation route | Cold waiting-for-mount; mounted candidate activation |
| Deactivate | `HostContentConnector::deactivate`; N-API deactivate | Requested/visible state transition | No hidden disposal | May preserve old visible fallback until removal frame |
| Dispose Connector | explicit Connector dispose; host teardown | candidate-aware Connector removal and Source membership release | Wrapper Drop is explicitly not a route | Visible/in-flight resource retained until safe receipt/abort |
| Mount Port | desired View attachment + `ContentHostRegistry::set_desired` | Port desired/visible reconciliation | No direct immediate visible setter | Target validation and frame barrier |
| Project content | ContentProvider `measure`/`paint_window` | Connector projection candidate preparation | No legacy `TextStream` renderer | Failed projection records status and falls back |
| Smooth tick | host clock → `ContentHostRegistry::advance` | Connector-local `Smooth` state | No TS per-tick route | Due index limits work to active Connectors |
| History transfer | History adapter/content rows | same Port/Connector provider path | Old `HostTextStream` route absent | Transfer waits for complete/finalized row product |

### 5.2 Direct ABI versus N-API control

The native direct ABI is intentionally narrow:

- `content_ffi.rs:1-6` documents it as the only high-volume Source payload entrypoint.
- It shares the environment-owned Source registry with N-API control classes.
- It performs no projection, layout, paint, or callback while handling a payload.
- It uses fixed metadata/result records and explicit status lanes.
- It resolves `(environment_slot, environment_generation, source_slot, source_generation)` before invoking the Source method.

The six required direct ABI symbols are represented by:

- `iyon_tui_perf13_abi_metadata_v1`
- `iyon_tui_source_append_utf8_v1`
- `iyon_tui_source_replace_utf8_v1`
- `iyon_tui_source_clear_v1`
- `iyon_tui_source_seal_v1`
- `iyon_tui_source_head_truncate_v1`

The two transport routes therefore differ only in ingress mechanics:

```text
N-API control:
JavaScript → NativeTextSource method → HostContentSource mutation

Direct data:
TypedArray/pointer → C ABI identity lookup → HostContentSource mutation
```

They do not maintain separate Source stores or duplicate wake schedulers.

### 5.3 Failure masking and recovery

The implementation distinguishes failures at several levels:

1. **Ingress rejection before Source installation**
   - invalid UTF-8;
   - malformed annotation;
   - payload too large;
   - coordinate exhaustion;
   - revision/content-generation exhaustion;
   - sealed Source;
   - invalid truncation boundary.

2. **Accepted Source mutation but wake failure**
   - Source revision and bytes remain accepted;
   - each host is still attempted;
   - failure is recorded in an environment-owned channel;
   - the mutation result remains successful.

3. **Projection/activation failure**
   - Connector records a structured error and failure key;
   - old visible Connector remains selected;
   - candidate projection is aborted;
   - retry is blocked for the same failed input until a valid retry boundary.

4. **Commit-preparation lock/poison failure**
   - visible state is not swapped;
   - prepared candidate remains available for retry when possible;
   - no partial visible mutation is performed.

5. **Post-promotion Source cleanup failure**
   - logical visible promotion remains committed;
   - Source membership/subscription stays retained;
   - Connector reports `cleanup_pending`;
   - cleanup is retried on a later candidate.

6. **Host/environment teardown**
   - host teardown forcibly disposes host-owned Ports/Connectors;
   - Source records remain environment-owned until environment teardown or explicit Source disposal;
   - detached Connector status/frontier records may remain readable after removal from the live host registry.

### 5.4 Absence claims

Within the current Rust source scope, searches found no production implementation for the former:

- `TextStream` renderer;
- `HostTextStream`;
- `NativeTextStream`;
- stream scheduler/pane/transfer modules;
- old stream snapshot/projector facades;
- old lifecycle request shims.

This is supported by PERF-13-H documentation and current source search. The current canonical route is:

```text
Source → Funnel → ContentPort → Connector → ContentHost
```

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Persistent Source storage

Evidence: `application/source_store.rs:1-16`, `88-100`, `223-226`, `705-721`, `1077-1100`.

Storage constants:

```rust
SOURCE_CHUNK_BYTES = 16 * 1024
MAX_SOURCE_PAYLOAD_BYTES = 64 * 1024 * 1024
MAX_SOURCE_ANNOTATIONS = 16 * 1024
MAX_ANNOTATION_PAYLOAD_BYTES = 4 * 1024 * 1024
LEAF_DESCS = 16
BRANCH_CHILDREN = 16
```

#### UTF-8 pages and chunk descriptors

Each `ChunkDesc` stores:

- `Arc<str>` page;
- page-local start and length;
- absolute Source start;
- precomputed relative newline offsets.

Pages are split at UTF-8 scalar boundaries. Splitting a descriptor shares the page `Arc` and only creates new range metadata.

#### Persistent ChunkTree

The chunk tree:

- uses leaves containing up to 16 descriptors;
- uses branches containing up to 16 children;
- stores aggregate byte and newline counts;
- supports right-edge append with `Arc::make_mut`;
- copies only shared right-edge nodes when snapshots retain old roots;
- splits by absolute coordinate for head truncation;
- supports line-entry lookup without a global line-start vector.

The line contract is:

- entry `0` is the Source range base;
- entry `i > 0` is the position immediately after the `(i-1)`th newline.

#### Persistent AnnotationTree

The annotation tree is a persistent deterministic treap:

- key: `(start_byte, seqno)`;
- `seqno`: acceptance order;
- `max_end`: subtree maximum end coordinate for overlap pruning;
- `size`: subtree count;
- deterministic priority derived from key;
- path copying on insert/split/merge.

Overlap queries are indexed by coordinate and then sorted by sequence number so overlapping annotations retain original semantic acceptance order.

### 6.2 Source storage mutation invalidation

Every operation that changes storage creates or installs a new logical storage value, except the validated annotation-free append fast path that mutates a unique `StoredSource` in place.

Invalidated/updated storage state:

- append:
  - chunk root;
  - annotation root if annotations are present;
  - ordered-annotation cache if annotation content changes;
  - end coordinate;
  - revision;
  - retention-derived base/head state.
- replace/clear:
  - entire `StoredSource` root;
  - content generation;
  - revision.
- seal:
  - seal flag and `sealed_at`;
  - optional annotation root.
- head truncate:
  - chunk root;
  - annotation root;
  - base/head-partial state;
  - ordered-annotation cache.

Old snapshots remain immutable and valid because they own old persistent roots.

### 6.3 Connector semantic/projection caches

Evidence: `application/content.rs:208-324`, `328-352`, `2994-3008`.

Semantic projection cache:

```rust
type SemanticProjectionCache =
    VecDeque<(SemanticProjectionKey, Arc<Projection<TextContent>>)>;
```

Capacity: `CONTENT_CACHE_CAPACITY = 2`.

Semantic key includes:

- Source ID;
- Source generation;
- content generation;
- Source revision;
- Source base/end;
- sealed state;
- Funnel kind;
- hyperlink setting.

Theme and width are deliberately excluded, so theme-only recolors reuse semantic IR and do not reparse.

Width/projection key includes:

- Source identity/generation/content generation;
- Source revision;
- offered width;
- wrap;
- Funnel kind;
- Connector delivery revision;
- host theme revision;
- finalized-prefix requirement;
- physical-row requirement.

Projection cache capacity is also 2. Candidate and committed projections are separate.

Prefix proof cache capacity: `CONTENT_PREFIX_CACHE_CAPACITY = 2`.

Prepared paint cache is Connector-local and reused across delivery ticks. Immediate non-History content can defer physical row lowering to the prepared-ticket window; Smooth and History products retain physical rows because delivery/scrollback requires them.

### 6.4 Theme and width invalidation

- Source semantic data and annotations are width-independent and theme-independent.
- Theme changes increment host theme revision and invalidate/rebuild presentation/paint products.
- Semantic parser products remain reusable on theme-only changes.
- Width changes produce a distinct projection key.
- The width offered to a Connector is normalized with `width.max(1)`.
- Layout measurement keys include Connector identity, exact offered width, projection readiness, requested/visible/mount/error state, and the relevant projection key. This prevents a stale layout cache product from being committed when its matching paint/projection product no longer exists.

### 6.5 Candidate Source snapshot cache

`candidate_source_snapshots` is:

```rust
RefCell<HashMap<u64, HostContentSourceSnapshot>>
```

It is:

- attempt-local;
- keyed by Source ID;
- lazily populated;
- cleared at candidate start/end/abort;
- not a second Source authority.

This cache is particularly important because `connector_projection_key` is callable from read-only ContentProvider-facing paths while the registry itself is mutably preparing a candidate. The `RefCell` preserves one candidate-wide snapshot without exposing a mutable Source copy.

### 6.6 Smooth scheduling

Active Smooth work is indexed by:

```rust
active_deadlines: HashMap<u64, Instant>
active_connectors: HashSet<u64>
```

Scratch vectors are reused for active and due IDs.

Cold Connectors are removed from the active indexes when:

- the Port is unmounted;
- the Connector is not visible/requested;
- delivery has no pending work;
- the Connector is disposed.

Native ticks therefore operate on active due Connector IDs rather than scanning every Connector in the host registry.

A pure delivery tick:

- advances Connector-local Smooth state;
- updates delivery frontier/revision;
- clears only candidate projection;
- preserves semantic/projection/prepared paint caches;
- emits targeted content dirty work.

### 6.7 Source wake cost

`capture_subscribers`:

- retains only groups with a live weak host and nonempty tokens;
- reuses `subscriber_wake_scratch`;
- copies token lists into a detached wake batch;
- allows Source lock release before host lock acquisition.

The environment wake broker drains all affected hosts. JavaScript does not maintain a subscription mirror and does not receive one payload call per host.

### 6.8 Retention policy

Source retention is configured at Source creation-time/early lifecycle through:

```rust
configure_retention(max_bytes, max_lines, drop_oldest)
```

Rules:

- at least one positive byte or line limit is required;
- changing retention while Connectors exist is rejected;
- `drop_oldest = false` rejects a mutation that would exceed the retention limit;
- `drop_oldest = true` advances the retained head;
- byte retention chooses a line boundary when possible, otherwise a UTF-8 boundary;
- retention preserves absolute coordinates;
- tag/style ranges can clip;
- atomic ranges crossing the retained floor drop;
- Point annotations survive according to point-position rules.

Markdown Funnel connection is rejected when Source history has been logically truncated or uses drop-oldest retention, because Markdown parsing requires the logical document start.

### 6.9 Performance counters observed in source

The following counters are referenced in the current implementation:

- `SourceSnapshotsAcquired`
- `ContentWakeGroups`
- `SemanticProjectionRebuilds`
- `ContentRegistryPortScans`
- `ContentDueConnectors`
- `ContentCandidateRecordsPrepared`

These provide visibility into:

- Source snapshot acquisition;
- grouped host wake fanout;
- semantic parser rebuilds;
- host Port registry scans;
- due Smooth Connector work;
- candidate record preparation volume.

No counter values were observed at runtime during this investigation.

---

## 7. Tests, benchmarks and observability

### 7.1 Coordinate tests

`crates/iyon-tui/src/stream/coord.rs:88-113` contains:

- `stream_offset_checked_add_reports_exhaustion_at_the_limit`.

It pins:

- ordinary checked addition;
- exact `u64::MAX` boundary;
- overflow rejection;
- saturating addition behavior.

The test comment explicitly connects `None` to the append-path `INVALID_RANGE: Source coordinate exhausted` error and emphasizes mutation atomicity.

### 7.2 Storage tests

`crates/iyon-tui/src/application/source_store.rs` contains focused tests for:

- empty tree line base behavior;
- single-chunk roundtrip;
- large append spanning pages and branches;
- truncate-then-append with absolute coordinates;
- stacked truncates and appends;
- newline at chunk edge;
- multibyte characters split across chunks;
- retention boundary at a multibyte scalar;
- partial truncation sharing a page with old snapshots;
- lazy ordered annotation materialization;
- overlap precedence;
- annotation sequence overflow atomicity;
- descriptor count semantics;
- annotation acceptance order;
- stored-record truncation policy;
- atomic versus clipping annotation behavior for semantic prefixes;
- byte-window retention;
- zero-length annotations at the retention floor;
- seal range validation;
- failed append leaving storage untouched;
- persistent treap invariants under randomized operations.

Representative evidence:

- `source_store.rs:1719-1790`
- `source_store.rs:1793-1939`
- `source_store.rs:1939-2196`
- `source_store.rs:2219-2346`

These tests establish that persistent storage is not merely an optimization: absolute coordinate continuity, UTF-8 boundary correctness, snapshot immutability, and annotation semantics are treated as behavioral contracts.

### 7.3 Source lifecycle/mutation tests

`application/content.rs` tests include:

- append revision exhaustion without installation;
- annotation sequence exhaustion without accounting;
- drop-oldest retention after a multibyte chunk boundary;
- replace revision exhaustion preserving old storage;
- clear revision exhaustion preserving storage;
- clear content-generation exhaustion preserving storage;
- seal revision exhaustion without setting sealed;
- truncate revision exhaustion without moving the head;
- Source wake subscription scoping across hosts;
- post-acceptance wake failure reporting;
- stale wake failure removal after membership ends;
- persistent Source sharing across repeated appends;
- Source disposal blocked by inactive Connector membership.

Representative evidence:

- `content.rs:7049-7252`
- `content.rs:7258-7513`

### 7.4 Port/Connector lifecycle tests

Tests cover:

- globally non-aliasing ContentPort IDs across hosts;
- wrong-environment Source connection;
- activation candidate failure and rollback;
- failed Connector retry after remount;
- mounting an unactivated Connector without endless requeue;
- Connector status after native host drop;
- detached Connector status/frontier retention;
- in-flight Connector identity protection;
- newer disposal overriding an older hidden plan;
- exactly-once Source membership release;
- post-promotion Source cleanup failure and retry.

Representative evidence:

- `content.rs:7539-8093`.

### 7.5 Candidate transaction tests

Tests cover:

- newer requested selection surviving an older prepared plan;
- many prepared records preserving newer pending binding without receipt growth;
- poisoned Connector/Port/Source records rejecting commit before visible swap;
- prepared candidate retaining its captured projection when a newer delivery tick changes mutable Connector state;
- one Source snapshot reused across multiple key/revision queries;
- candidate delivery frontier versus committed visible frontier.

Representative evidence:

- `content.rs:8093-8550`
- `content.rs:9603-9686`.

### 7.6 Projection/delivery/History tests

Tests cover:

- History binding requiring a finalized-prefix product;
- sealed History row completeness;
- leading padding preservation for open streams;
- History unit retirement and Port disposal;
- theme invalidation of content measurement;
- prepared paint separation for immediate versus smooth row demands;
- deferred tickets painting with their captured theme;
- theme recolor without semantic reparsing;
- native sink failure without rewinding History;
- finalized-prefix rows matching sealed baseline;
- partial/zero receipt preservation across resize;
- prepared ticket not selecting newer same-width projection;
- static and unsealed Markdown History transfer;
- Smooth History matching finalized rows across ticks/receipts;
- delivery frontier parity and monotonicity;
- independent delivery for two Connectors sharing one Source;
- native ticks causing zero parser/surface clones;
- visible frontier being distinct from execution progress;
- cold/disposed Connector deadline cleanup.

Representative evidence:

- `content.rs:8550-10647`.

### 7.7 Historical verification evidence

Historical PERF-13 documents report successful checks, but these were not run as part of this investigation:

- PERF-13-F completion reports `cargo test -p iyon-tui --features native-host --lib — 754 passed, 1 ignored`.
- PERF-13-G completion reports `752 passed` for a later content tranche and focused content probes.
- PERF-13-H completion reports workspace checks, `95 passed` TypeScript fixture tests, direct FFI probes, and content performance probes.

These are historical claims from `docs/history/PERF-13/*`, not observations from the current report run.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Physical `stream/` ownership is narrower than conceptual documentation

Current source:

- `stream/` owns only coordinate types.
- Source storage is in `application/source_store.rs`.
- Source/Port/Connector lifecycle and projection are in `application/content.rs`.

This agrees with PERF-13-H’s finalization statement that old stream facades and schedulers were deleted while `StreamOffset`/`StreamRange` survived as source-rooted coordinate types.

However, `AGENTS.md:93` lists broader conceptual names such as `StreamingSource`, `TextStream`, `StreamPane`, `StreamSnapshot`, and `StreamRevision` under generic framework responsibilities. A current-source search does not find those Rust symbols. The current implementation should be treated as authoritative: those names are historical/conceptual documentation vocabulary, not current Rust module ownership.

### 8.2 `ContentHostRegistry` is a large mixed seam

`application/content.rs` intentionally combines:

- Source-independent Source lifecycle and registry logic;
- host-owned Port/Connector state;
- generic projection and smoothing;
- `TextRenderer` lowering;
- layout tree creation;
- paint cache preparation;
- History row transfer.

This is not a second Source authority: `StoredSource` remains the Source data owner. It is nevertheless a consequential coupling because the file reaches into:

- `crate::presentation::layout`;
- `crate::presentation::paint`;
- `crate::presentation::ContentProvider`;
- `crate::History` adapter semantics;
- `crate::projection`;
- semantic text projectors/renderers.

The current code explicitly tries to preserve the conceptual boundary by exposing `ContentProvider` to presentation while preventing presentation from reaching into Source/Port/Connector lifecycle. The reverse direction remains intentionally coupled in the current host implementation.

### 8.3 Source storage keeps semantic style/tag data, not host-native style IDs

`StoredSource` contains `SourceAnnotation` values with optional `SemanticTag`, `StyleRef`, and semantic payload data. It does not contain terminal cell styles, physical rows, layout geometry, or backend-specific IDs.

The source-side style representation is therefore semantic/generic. Host theme and presentation resolution occur later during Connector projection/paint preparation.

### 8.4 Snapshot materialization is split between diagnostics and frame path

There are two materially different snapshot consumers:

1. Frame path:
   - clones `Arc<StoredSource>`;
   - consumes chunk views and indexed annotations;
   - avoids materializing the entire text String.

2. Diagnostic/N-API path:
   - invokes `snapshot.text()`;
   - copies annotation payloads;
   - serializes full text and metadata into a JavaScript object.

This distinction is documented in `HostContentSourceSnapshot` and visible in `NativeTextSource::snapshot` at `tui.rs:1179-1211`.

### 8.5 IDs and generations are not all scoped identically

- Source IDs are allocated by an environment-owned Source registry and capped to `u32::MAX` for the direct ABI.
- Source generation is `u32`.
- Source content generation is `u64`.
- Source revision is `u64`.
- ContentPort IDs use a global atomic counter to prevent cross-host aliasing.
- ContentPort and Connector generations are host-registry generation values.
- Connector IDs are allocated by the host ContentHostRegistry, while N-API/resource-registry ownership provides the external host-bound handle surface.

The direct ABI must therefore carry both environment identity/generation and Source identity/generation. A Source ID alone is not a globally sufficient FFI identity.

### 8.6 Desired versus visible state is essential, not incidental

Both Port and Connector records maintain desired and visible state:

```text
Port:
    desired_mounted
    visible_mounted
    desired_connector
    visible_connector

Connector:
    requested
    visible
    candidate_projection
    committed_projection
```

This duality allows:

- a new desired Connector to fail without disturbing the old visible Connector;
- a Port to remain structurally mounted while Connector projection is pending;
- in-flight receipt candidates to retain exact identity/product ownership;
- newer control operations to supersede older prepared plans without resurrecting stale state.

### 8.7 No separate scroll ownership in content

PERF-13-F and current source agree that ContentPort/Connector do not own scroll offset or follow-end state. Content integration reports extent/rows, while ScrollPane/RowViewport owns scroll position and follow behavior.

The History adapter tracks committed row accounting and padding, but not the user’s scroll offset.

### 8.8 Former legacy stream path is absent from production Rust

Current Rust source does not contain `HostTextStream`, `NativeTextStream`, `TextStream`, or stream scheduler/pane modules. PERF-13-G/H records their removal. Existing algorithms that remain—persistent storage, projections, smoothing, and source-rooted coordinates—are integrated through the canonical content-plane path rather than exposed as a second public/runtime stream architecture.

---

## 9. Open questions and coverage gaps

1. **Typed revision identity**
   - Current Source revisions are raw `u64` fields.
   - `StreamOffset` and `StreamRange` are typed, but there is no current `StreamRevision` type.
   - It is unknown whether a later architecture should introduce a typed revision wrapper or retain raw revisions at the Source/content seam.

2. **Non-text Source families**
   - `ContentFamily` currently has only `Text`.
   - `StreamOffset` documentation permits record/event ordinals, but no current non-text implementation exists.
   - The storage implementation is UTF-8-specific.

3. **Exact runtime complexity**
   - The persistent tree design indicates path-copy behavior, but no benchmark was run here.
   - Projection, semantic parser, annotation rewriter, and row compilation may still scan all retained chunks for a changed Source revision.
   - The current counters expose rebuilds and snapshot acquisition but do not by themselves prove end-to-end incremental complexity.

4. **Source snapshot and Source mutation concurrency**
   - Candidate snapshots intentionally remain stable during a frame even if Source mutates concurrently.
   - The exact host-drain ordering between a Source mutation and a candidate already in flight was source-inspected but not dynamically exercised here.

5. **Cleanup retry progress**
   - The implementation preserves membership after post-promotion cleanup failure and schedules retry through later candidates.
   - The exact behavior under permanently poisoned Source records is represented by tests and explicit error channels, but was not executed during this report.

6. **Generated ABI completeness**
   - Direct ABI Source data entrypoints and fixed records were inspected.
   - The full generated-schema provenance and all TypeScript transport implementation details were not audited line-by-line in this assignment.

7. **History transfer decomposition**
   - Current History row behavior is mediated by `HistoryTerminalAdapter` inside `application/content.rs`.
   - The adapter’s row/padding contract is clear, but its eventual conceptual decomposition from Connector/ContentHost is outside this assignment’s V5-decision boundary.

8. **Current framework documentation drift**
   - `AGENTS.md` lists conceptual stream symbols no longer present in current Rust.
   - PERF-13-H is the more specific and current historical record of removal.
   - No attempt was made to rewrite documentation.

9. **No executed validation**
   - All behavioral observations are from source and historical test definitions/results.
   - This report does not claim current tests pass at the specified baseline based on a fresh run.

---

## 10. Evidence appendix

### 10.1 Exact current-source symbols and ranges

#### Coordinate module

- `crates/iyon-tui/src/stream/mod.rs:1-9`
  - module documentation;
  - `mod coord`;
  - `pub use coord::{StreamOffset, StreamRange}`.
- `crates/iyon-tui/src/stream/coord.rs:1-36`
  - `StreamOffset`;
  - checked/saturating coordinate arithmetic.
- `crates/iyon-tui/src/stream/coord.rs:38-85`
  - `StreamRange`;
  - range construction/query methods.
- `crates/iyon-tui/src/stream/coord.rs:88-113`
  - coordinate overflow test.

#### Persistent storage

- `crates/iyon-tui/src/application/source_store.rs:1-16`
  - persistent storage design and line-entry contract.
- `source_store.rs:26-38`
  - constants and annotation kinds.
- `source_store.rs:40-86`
  - `ValidatedInput`.
- `source_store.rs:88-150`
  - `ChunkDesc`.
- `source_store.rs:153-221`
  - `ChunkNode`.
- `source_store.rs:223-704`
  - `ChunkTree`.
- `source_store.rs:705-1011`
  - `SourceAnnotation`, `AnnotationNode`, `AnnotationTree`.
- `source_store.rs:1013-1075`
  - `ValidatedAnnotation`, `ContentError`, sequence preflight.
- `source_store.rs:1077-1663`
  - `StoredSource` and storage operations.
- `source_store.rs:1639-1663`
  - `StoredOverlap`.
- `source_store.rs:1667+`
  - storage behavioral tests.

#### Source/public content API

- `crates/iyon-tui/src/application/content.rs:46-75`
  - Content family/source/funnel/delivery/wrap enums.
- `content.rs:77-106`
  - ABI annotation record and diagnostic annotation snapshot.
- `content.rs:108-187`
  - `HostContentSourceSnapshot`.
- `content.rs:191-324`
  - Source lineage, projection keys, semantic cache.
- `content.rs:328-488`
  - `HostContentProjection`, `ConnectorDelivery`, `ConnectorExecution`.
- `content.rs:492-515`
  - Source stats and mutation result.
- `content.rs:611-730`
  - Source and grapheme projection.
- `content.rs:746-797`
  - semantic-to-layout compilation.
- `content.rs:1021-1257`
  - width/theme/delivery-aware content projection.
- `content.rs:1258-1462`
  - Source annotation rewriting and coordinate mapping.
- `content.rs:1464-1513`
  - ContentPort global ID and `HostContentFunnel`.
- `content.rs:1516-1666`
  - lifecycle records, Source subscriptions, Source registry structures.
- `content.rs:1668-1782`
  - `ContentSourceRegistry`.
- `content.rs:1784-2806`
  - `HostContentSource` and Source mutation/subscription APIs.
- `content.rs:2808-2950`
  - History adapter.
- `content.rs:2952-3276`
  - Port/Connector records, prepared candidate records, registry fields.
- `content.rs:3278-3584`
  - ContentHostRegistry construction, Port/Connector creation, desired bindings, candidate start.
- `content.rs:3586-3773`
  - delivery advancement and deadline management.
- `content.rs:3800-4243`
  - Source snapshot capture, projection keys, cache lookup, projection preparation.
- `content.rs:4261-4340`
  - Content measurement and failed-candidate selection.
- `content.rs:4545-4960`
  - projection ticket/commit preparation.
- `content.rs:4967-5118`
  - activation candidate, prepared candidate lease, abort cleanup.
- `content.rs:5188-5506`
  - receipt-time commit.
- `content.rs:5537-6151`
  - activation/deactivation/disposal and Connector/Port cleanup.
- `content.rs:6167-6683`
  - status and `ContentProvider`.
- `content.rs:6707-7007`
  - public Port/Connector handles and explicit disposal semantics.
- `content.rs:7011-10647`
  - Source, storage, Connector, candidate, projection, delivery, and History tests.

#### Host/environment seams

- `crates/iyon-tui/src/application/environment.rs:151-226`
  - environment Source registry ownership, Source create/lookup.
- `environment.rs:520-705`
  - Source wake failure drain and host pending processing.
- `crates/iyon-tui/src/application/host.rs:1064-1149`
  - host Port creation, desired View publication, pending host drain.
- `host.rs:1831-1934`
  - content dirty and candidate preparation.
- `host.rs:2045-2102`
  - candidate visible commit.
- `host.rs:2171-2184`
  - candidate abort.
- `host.rs:2275-2285`
  - delivery advancement and content dirty scheduling.

#### Projection seams

- `crates/iyon-tui/src/projection/value.rs:1-279`
  - `Projection`, `ProjectionSpan`, `ProjectionBuilder`, source envelopes.
- `crates/iyon-tui/src/projection/projector.rs:1+`
  - Projector restart coordinate contract.
- `crates/iyon-tui/src/projection/smooth.rs:8-415`
  - Source-rooted Smooth state/frontiers and incremental output.

#### Native boundaries

- `crates/iyon-tui-native/src/content_ffi.rs:1-6`
  - direct Source data ABI ownership statement.
- `content_ffi.rs:16-112`
  - ABI constants, status lanes, metadata/result records.
- `content_ffi.rs:167-306`
  - pointer/input validation and Source identity resolution.
- `content_ffi.rs:348-480`
  - append, replace, clear, seal, and head-truncate symbols.
- `crates/iyon-tui-native/src/tui.rs:849-862`
  - N-API ContentPort creation.
- `tui.rs:1090-1238`
  - N-API Source creation, identity, snapshot, and statistics.
- `tui.rs:1240-1564`
  - N-API Port/Connector and Funnel control.
- `crates/iyon-tui/src/binding/mod.rs:73-80`
  - Rust content types re-exported to the native binding crate.

### 10.2 Historical source/document evidence

- `docs/history/PERF-13/PERF-13-D-completion.md`
  - environment-owned Source registry;
  - host-owned Port/Connector registries;
  - cold activation;
  - transactional lifecycle.
- `docs/history/PERF-13/PERF-13-E-completion.md`
  - retained UTF-8 Source storage;
  - immutable chunks;
  - absolute coordinates;
  - snapshots, retention, sealing, truncation;
  - direct ABI and wake fanout.
- `docs/history/PERF-13/PERF-13-F-completion.md`
  - ContentProvider boundary;
  - Connector-local width-dependent caches;
  - candidate/committed projections;
  - ScrollPane ownership of scroll state;
  - content-aware invalidation.
- `docs/history/PERF-13/PERF-13-G-completion.md`
  - Funnel modes;
  - Connector-local Smooth;
  - semantic annotations;
  - History integration;
  - removal of `NativeTextStream`/`HostTextStream`.
- `docs/history/PERF-13/PERF-13-H-completion.md`
  - final route `Source -> Funnel -> ContentPort -> Connector -> ContentHost`;
  - deletion of old stream facades/schedulers/panes;
  - retained source-rooted coordinates;
  - explicit ownership and teardown.
- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`
  - environment versus host ownership;
  - identity table;
  - desired/visible binding semantics;
  - cold Connector invariants;
  - viewport ownership;
  - Source revision linearization.

### 10.3 Commands/search methods used

Read-only repository inspection used file listing, file discovery, and content searches equivalent to:

- locating the report contract, README, AGENTS, pre-V5 report, and PERF-13 records;
- enumerating `crates/iyon-tui/src/stream/**`;
- searching Source/Funnel/Connector/Port/snapshot symbols across Rust;
- searching old stream facade names for current-source absence;
- locating Source mutation and ABI entrypoints;
- extracting declaration/test symbol ranges and line references.

No shell mutation, build, test, dependency installation, or service operation was performed.

### 10.4 Files indexed but not comprehensively read

The following were used only as supporting seam evidence rather than assignment-owned exhaustive reads:

- most TypeScript content transport and runtime files;
- generated ABI bodies and generated schema artifacts;
- the full native N-API implementation outside Source/Port/Connector sections;
- unrelated Rust presentation/layout/paint/history modules;
- unrelated repository-level benchmarks and fixtures.

The assignment-owned `stream/` files were fully inspected. The supporting `application/content.rs` and `application/source_store.rs` were investigated through their complete declaration/test inventories and targeted source extraction around every Source/storage/Port/Connector/projection seam; no claim is made that every unrelated presentation helper in those large files was line-by-line re-reviewed.