# 35 — Stream content

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: TypeScript source/content APIs and transport through native/Rust Source storage, connector lifecycle, projection/smoothing, measurement, layout/paint, sealing, truncation, compaction/retention, and release.
- Primary Rust production paths:
  - `crates/iyon-tui/src/application/content.rs`
  - `crates/iyon-tui/src/application/source_store.rs`
  - `crates/iyon-tui/src/application/environment.rs`
  - `crates/iyon-tui/src/presentation/content.rs`
  - `crates/iyon-tui/src/presentation/layout/**`
  - `crates/iyon-tui/src/presentation/paint/**`
  - `crates/iyon-tui/src/projection/**`
  - `crates/iyon-tui/src/content/text/**`
  - `crates/iyon-tui/src/stream/**`
- Primary TypeScript production paths:
  - `packages/iyon-tui/src/api/content/retained.ts`
  - `packages/iyon-tui/src/transport/content/ffi.ts`
  - `packages/iyon-tui/src/transport/content/control.ts`
  - `packages/iyon-tui/src/transport/native/addon.ts`
  - related runtime/native resource and scheduling modules.

### Scope boundary

This report follows a real current-source trace. It does not perform the V5/census/disposition analysis requested to remain outside this assignment. Terms such as “Source”, “Funnel”, “Connector”, “projection”, and “ContentPort” refer to current implementation concepts, not presumed future replacements.

The generic framework boundary is respected:

- TS APIs author content, source mutations, funnel policy, connector activation, and ownership/lifecycle operations.
- The Rust/native runtime owns retained Source data, source identity/generation, annotation validation/storage, parser/projection execution, smoothing clocks, connector activation state, measurement, layout/paint products, and frame-facing invalidation.
- No application/product-specific assistant semantics are implemented in the generic Source/Connector path.

### Evidence status

- Static source inspection only.
- No build, benchmark, runtime replay, or test suite was executed.
- Evidence below uses exact source paths and line ranges from the inspected baseline.
- Approximate LOC figures use the production-file line extents visible in source inspection; tests are listed separately where their file inventory was available. Generated/native binding files are identified but not treated as handwritten runtime ownership.

### Prior documents read for context

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The prior report establishes that current content/stream/projection machinery must be mapped from source behavior before making any architectural judgment. This report therefore describes current routes and ownership without making V5 migration decisions.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate production LOC | Test/generated LOC | Primary responsibility | Plane |
|---|---:|---:|---|---|
| `packages/iyon-tui/src/api/content/retained.ts` | ~808 | Tests elsewhere | Public Source, Funnel, ContentPort, ContentConnector APIs; TS ownership and validation | TS content/control |
| `packages/iyon-tui/src/transport/content/ffi.ts` | ~786 | Embedded tests not present in file | Bulk Source mutation ABI, annotation encoding, u64 split/join, wake/result decoding, ABI validation | TS data lane |
| `packages/iyon-tui/src/transport/content/control.ts` | ~98 | N/A | Small N-API control wrappers for Source/Port/Connector | TS control lane |
| `packages/iyon-tui/src/transport/native/addon.ts` | ~200+ | N/A | Native contract interfaces, including Source/Port/Connector methods | TS/native boundary |
| `crates/iyon-tui-native/src/content_ffi.rs` | ~480 production plus tests after that | ~100+ tests | Exported C ABI entry points and pointer/identity guards; forwards to `HostContentSource` | Native ABI |
| `crates/iyon-tui/src/application/content.rs` | ~9,600+ | Large embedded test module after production section | Source registry, Source mutation API, Port/Connector lifecycle, parser/projection execution, smoothing delivery, cache products, measure/paint/history integration | Rust content/host |
| `crates/iyon-tui/src/application/source_store.rs` | ~2,000 | ~300+ embedded tests | Persistent chunk tree, annotation index, immutable snapshots, append/seal/truncate/retention storage operations | Rust content storage |
| `crates/iyon-tui/src/projection/value.rs` | ~278 | Tests in `projection/tests.rs` and public projection tests | Validated root-coordinate `Projection<T>` and span builder/mapping | Rust semantic projection |
| `crates/iyon-tui/src/projection/smooth.rs` | ~438 | Projection tests | Stable-frontier temporal publication and tick scheduling | Rust delivery |
| `crates/iyon-tui/src/projection/validate.rs` | ~220 | Projection tests | Projection shape, relation, and transition validation | Rust semantic integrity |
| `crates/iyon-tui/src/projection/compose.rs` | Small | Projection tests | Projector composition | Rust semantic projection |
| `crates/iyon-tui/src/content/text/source.rs` | ~460 | Embedded tests | Raw page-backed domains, source-coordinate witnesses, lazy multi-page assembly, exact/derived runs | Rust text IR |
| `crates/iyon-tui/src/content/text/plain.rs`, `markdown.rs`, `diff.rs`, `ansi.rs` | Several hundred combined | Embedded/migrated tests | Funnel-specific semantic projection | Rust text IR |
| `crates/iyon-tui/src/content/text/render/**` | Several hundred combined | Embedded tests | Semantic text to presentation/layout IR | Rust semantic presentation |
| `crates/iyon-tui/src/presentation/content.rs` | ~250 | Tests in related content files | Presentation-facing ContentProvider contract; measurement, prepared tickets, paint windows, history rows | Rust presentation seam |
| `crates/iyon-tui/src/presentation/layout/**` | Thousands combined | Layout tests | Width-dependent layout and text measurement | Rust layout |
| `crates/iyon-tui/src/presentation/paint/**` | Hundreds combined | Paint/physical tests | Theme/style resolution, text geometry cache, row-range painting | Rust paint |
| `crates/iyon-tui/src/stream/coord.rs` | ~112 | Embedded tests | Opaque monotonic offsets and half-open source ranges | Rust coordinate utility |
| `crates/iyon-tui/src/stream/mod.rs` | 9 | N/A | Exports source-coordinate types only | Rust coordinate utility |

The most consequential implementation is `application/content.rs`. It is not merely a registry: it contains Source mutation orchestration, connector execution state, smoothing, semantic projection, paint-product caching, layout preparation, frame-facing ticket validation, and ContentProvider implementation.

### 1.2 Current responsibility split

The current pipeline is divided into the following conceptual layers:

1. **TS public authoring**
   - `TextStreamSource` / `TextBlockSource`
   - `TextFunnel`
   - `ContentPort`
   - `ContentConnector`

2. **TS transport**
   - Bulk Source payloads and annotations use direct FFI calls.
   - Control calls use N-API/native resource contracts.
   - Mutation results contain source revision, environment wake epoch, and a scheduling bit.

3. **Native identity resolution**
   - Environment slot/generation and Source slot/generation are checked before mutation.
   - Rust ABI entry points forward only after pointer and identity validation.

4. **Rust Source**
   - `HostContentSource` owns a registry reference and an `Arc<Mutex<ContentSourceRecord>>`.
   - `StoredSource` owns persistent content roots and scalar stamps.
   - Source snapshots clone metadata and share immutable storage.

5. **Rust Connector**
   - A connector binds one Source, one immutable Funnel specification, and one ContentPort.
   - Active Connector execution owns parser state, renderer state, and optional Smooth delivery state.
   - Inactive Connectors clear execution and derived caches.

6. **Projection**
   - Raw page-backed Source spans become `Projection<TextContent>`.
   - Funnel projectors produce semantic text content with source provenance.
   - Optional annotation rewriting applies Source annotations to semantic runs.
   - Optional Smooth publication reveals stable grapheme units over time.

7. **Measurement/layout/paint**
   - Semantic text is lowered through `TextRenderer`.
   - `ViewCompiler` produces a width-dependent `LayoutTree`.
   - Physical rows are retained only when needed by History or smoothing; immediate non-History routes may defer row lowering.
   - Paint consumes a prepared ticket and paints only the selected immutable product.

8. **Scheduling**
   - Source mutations wake subscribed hosts.
   - Smooth maintains per-Connector deadlines.
   - A due Connector tick advances only delivery state, increments delivery revision when progress occurs, invalidates the candidate projection, and emits content dirtiness.

---

## 2. Types, APIs and contracts

### 2.1 TypeScript Source APIs

`packages/iyon-tui/src/api/content/retained.ts` exposes:

#### Source configuration and state types

- `ContentFamily = "text"`
- `TextSourceOptions`
- `TextRetentionPolicy`
  - `maxBytes`
  - `maxLines`
  - `overflow: "drop-oldest" | "error"`
- `TextSourceAnnotationKind`
  - `"tag"`
  - `"style"`
  - `"atomic"`
  - `"point"`
- `TextSourceAnnotation`
  - operation-local UTF-8 byte ranges
  - tag namespace/name
  - semantic style
  - opaque payload
- `TextSourceMutation`
  - `revision: bigint`
  - `environmentWakeEpoch: bigint`
  - `scheduleEnvironmentDrain: boolean`
- `TextSourceSnapshot`
  - Source identity and generation
  - content generation
  - revision
  - absolute retained range
  - seal/head-partial state
  - materialized text and annotation snapshots
- `TextSourceStats`
  - retained bytes/lines/chunk count
  - accepted/copied/dropped byte counters.

#### `TextStreamSource`

The stream Source is created by:

```ts
TextStreamSource.create(options?)
```

Public methods include:

- `sourceId()`
- `sourceGeneration()`
- `environmentSlot()`
- `environmentGeneration()`
- `contentGeneration()`
- `snapshot()`
- `stats()`
- `append(text, annotations?)`
- `appendUtf8(text, annotations?)`
- `replace(text, annotations?)`
- `replaceUtf8(text, annotations?)`
- `clear()`
- `seal()`
- `truncateHead(offset)`

Important semantics:

- `append` is only valid for a stream Source.
- `replace` creates a new content generation.
- `clear` also creates a new logical generation in Rust.
- `seal` is only valid for a stream Source.
- `truncateHead` preserves absolute coordinates; it does not renumber retained bytes.
- Every successful mutation returns a revision and wake metadata.
- The TS API validates text/annotation container shape before invoking the transport.

#### `TextBlockSource`

The block Source is created by:

```ts
TextBlockSource.create(options?)
```

It exposes:

- identity/generation accessors
- `snapshot()`
- `stats()`
- `replace`
- `replaceUtf8`
- `clear`
- `truncateHead`

It intentionally does not expose append or seal. Rust also rejects append/seal for block Sources, so this is enforced at both authoring and runtime boundaries.

### 2.2 TypeScript Funnel APIs

`TextFunnel` is immutable and Source-neutral:

- Modes:
  - `TextFunnel.plain(options?)`
  - `TextFunnel.markdown(options?)`
  - `TextFunnel.diff(options?)`
  - `TextFunnel.ansi(options?)`
- Options:
  - `wrap: "word" | "grapheme" | "noWrap"`
  - `smooth: boolean | TextSmoothOptions`
  - `hyperlinks: boolean`
- `smooth(options?)` returns a new Funnel with Connector-local native smoothing.
- `immediate()` returns a new Funnel with smoothing disabled.

The Funnel has no Source data and no active projection cache. It is a value-level policy object. Native conversion (`textFunnelNative`) maps it to:

- kind
- wrap
- hyperlinks
- smooth flag
- tick interval
- spring
- min/max units per second.

TS validation rejects unknown Funnel options and invalid smoothing ranges before control transport.

### 2.3 TypeScript Port/Connector APIs

#### `ContentPort`

`ContentPort` is host-owned and owns Connector membership but not Source bytes.

Key methods:

- `connect(source, funnel)`
- `deactivate()`
- `mounted()`
- `isMounted()`
- `connectorCount()`
- `dispose()`
- internal `syncNativeLifecycles()`
- internal `forgetConnector()`

`connect` validates:

- Source-like object is live and not disposed.
- Funnel is a `TextFunnel` with text family.
- Source native resource and Port native resource are passed to the control transport.
- The resulting Connector is retained in the Port’s TS `Set`.

#### `ContentConnector`

A Connector binds one Port, one Source, and one Funnel.

Key methods:

- `activate()`
- `deactivate()`
- `status()`
- `dispose()`
- internal `syncNativeLifecycle()`
- `attachedPort`
- `attachedSource`

Connector disposal is deliberately transactional:

1. TS marks the resource as beginning disposal.
2. Native `dispose()` requests the Rust Connector transition.
3. The TS wrapper remains observable as disposing until native status reaches `disposed`.
4. `finalizeWrapper()` removes it from Port membership and releases its framework handle.

This prevents TS wrapper destruction from preceding the Rust frame/commit boundary.

### 2.4 Rust Source APIs

`HostContentSource` in `application/content.rs` provides:

- `id()`
- `generation()`
- `content_generation()`
- `snapshot()`
- `stats()`
- `append_utf8(bytes, annotations, annotation_payload)`
- `replace_utf8(bytes, annotations, annotation_payload)`
- `clear()`
- `seal()`
- `truncate_head(offset)`
- `is_live()`
- `dispose()`

`HostContentSourceSnapshot` is cheap to clone because it shares `Arc<StoredSource>`. It exposes:

- `text()` for diagnostics/materialization
- `annotations()`
- `retained_bytes()`
- `retained_lines()`
- `chunk_count()`
- internal chunk views and annotation access for projection.

The frame path uses chunk/page views rather than calling `text()`.

### 2.5 Projection API contracts

`Projection<T>` contains:

- `source_base`
- `stable_through`
- `source_end`
- `sealed`
- ordered contiguous `ProjectionSpan<T>` values.

A span may have no values to represent explicit source elision. The builder validates complete source coverage.

Important contracts:

- Source ranges are root-coordinate half-open ranges.
- Appended spans must begin exactly at the current `source_end`.
- `stable_through` is monotonic and cannot exceed `source_end`.
- A sealed projection must have `stable_through == source_end`.
- Projection transition validation rejects source-base regressions, source-end regressions, stability regressions, changed sealed projections, and modifying already-sealed output.
- `spans_from(offset)` uses binary search over ordered spans and avoids rescanning the stable prefix.

### 2.6 Smooth API contracts

`Smooth` operates on already-projected spans and never transforms values.

- It only publishes complete input spans.
- It never splits a span.
- Pacing is based on `values.len()`, not source byte length or display width.
- Only spans through the upstream stable frontier are eligible.
- A newly observed append episode releases one immediate weighted span before paced ticks.
- A sealed input is caught up immediately and clears all pending work.
- `next_wakeup()` is a deadline, not a request to rebase the clock.
- `advance(now)` only progresses at/after the deadline and carries elapsed credit forward.

### 2.7 Presentation-facing contracts

`presentation/content.rs` defines the internal `ContentProvider` seam:

- `set_theme`
- `projection_revision`
- `layout_input_revision`
- `measure`
- `paint_window`
- `history_rows`
- History commit/clear integration.

`PreparedProjectionTicket` includes:

- `port_id`
- `connector_id`
- offered width
- projection revision
- projection identity.

Connector identity is explicitly part of the ticket. Width alone is not a safe selector because multiple Connectors can have identical widths.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
TS TextStreamSource / TextBlockSource
        │
        ├── retained.ts validation and wrapper lifecycle
        │
        ├── transport/content/ffi.ts
        │       ├── UTF-8 encoding
        │       ├── annotation encoding
        │       ├── u64 lane splitting
        │       └── wake/result decoding
        │
        └── native Source contract / content_ffi.rs
                │
                └── HostContentSource
                        ├── Source registry / identity
                        ├── ContentSourceRecord
                        │       └── Arc<StoredSource>
                        │
                        ├── subscriber wake groups
                        └── HostContentSourceSnapshot
                                │
                                └── ContentHostRegistry Connector
                                        ├── ConnectorExecution
                                        │       ├── Markdown/Diff/ANSI projector
                                        │       ├── TextRenderer
                                        │       └── ConnectorDelivery
                                        │               └── Smooth
                                        │
                                        ├── semantic projection cache
                                        ├── prepared paint cache
                                        ├── prefix proof cache
                                        ├── LayoutTree / TextGeometryCache
                                        └── HostContentProjection
                                                ├── ContentMeasurement
                                                ├── History rows
                                                └── paint ticket
```

### 3.2 Ownership

| Object | Created by | Retained by | Destroyed/released by |
|---|---|---|---|
| `TextStreamSource` / `TextBlockSource` TS wrapper | TS `create` | framework handle registry and user references | TS disposal plus native Source disposal |
| Native Source record | environment Source registry | registry, Source wrapper, snapshots, Connectors | Source `dispose` after Connector membership reaches zero |
| `StoredSource` | Source mutation | `Arc` in Source record and snapshots | Arc reference release |
| `HostContentSourceSnapshot` | Source `snapshot()` / candidate capture | projection candidate and callers | snapshot Arc release |
| `ContentPort` | host/native host | host and TS wrapper | host disposal / resource lifecycle |
| `HostContentConnector` | Port `connect` | registry, Port membership, TS wrapper | transactional Connector disposal |
| `ConnectorExecution` | Connector activation/visibility | Connector record while active | cleared when Connector inactive/hidden |
| `Projection<TextContent>` | Connector projection miss | semantic cache, paint product, projection product | cache eviction / Connector cleanup |
| `Smooth` | Connector activation for smooth Funnel | Connector execution | Connector execution clear |
| `LayoutTree` | preparation or prefix proof | prepared paint product / projection | cache eviction |
| physical rows | paint preparation when required | History/smooth projection product | cache eviction or Connector cleanup |

### 3.3 Identity and lifetime

There are several independent identities:

1. **Source handle identity**
   - environment slot + environment generation
   - Source slot + Source generation
   - checked in FFI before mutation.

2. **Content generation**
   - changes on replacement and clear.
   - parser lineage includes Source ID, Source generation, and content generation.
   - prevents parser continuation across logical document replacement even when byte coordinates happen to repeat.

3. **Source revision**
   - changes on every successful non-no-op Source mutation.
   - append, replace, clear, seal, and truncate each produce a new revision.
   - no-op empty append/annotation-free mutation returns the current revision without notifying.

4. **Connector generation**
   - identifies Connector lifecycle instances.
   - Connector membership and wake subscription are separate.

5. **Projection identity**
   - monotonic process-level `NEXT_CONTENT_PROJECTION_ID`.
   - intentionally not an allocator address, avoiding pointer ABA when caches evict/reuse products.

6. **Delivery revision**
   - Connector-local revision incremented only when Smooth progresses on a tick.
   - included in projection keys and layout-input revision semantics.

---

## 4. Execution paths and state transitions

### 4.1 Creation path

#### TS Source creation

```text
TextStreamSource.create(options)
  → validateTextSourceOptions
  → createTextSource("stream", options)
  → nativeTui.textSource
  → Native Source resource
  → FrameworkHandle registration
```

The TS constructor performs cleanup if framework registration fails: it calls native `dispose()` and aggregates cleanup errors if necessary.

#### Rust Source creation

`ContentSourceRegistry::create(kind)`:

1. Allocates Source ID/generation.
2. Creates a `ContentSourceRecord`.
3. Installs empty `StoredSource`.
4. Registers the record.
5. Returns `HostContentSource`.

Source storage starts with base/end zero, revision zero, no annotations, and unsealed state.

#### Port/Connector creation

```text
ContentPort.connect(source, funnel)
  → TS validation
  → native Port.connect(...)
  → ContentHostRegistry::connect(...)
  → Source membership increment
  → ConnectorRecord allocation
  → HostContentConnector wrapper
```

A Connector initially has:

- `requested = false`
- `visible = false`
- phase `"idle"`
- no execution
- no projection
- no active deadline
- empty derived caches.

### 4.2 Append path: TS to Rust Source

The normal append route is:

```text
TextStreamSource.append(text, annotations)
  → validateSourceMutation
  → appendTextSource(...)
  → UTF-8 byte encoding
  → encodeAnnotations(...)
  → source identity lookup
  → iyon_tui_source_append_utf8_v1
  → HostContentSource::append_utf8
  → StoredSource::apply_append / append_in_place
  → subscriber capture
  → host content dirtiness
  → environment wake result
```

#### TS data-plane behavior

`ffi.ts`:

- Computes UTF-8 byte length before allocating the payload.
- Rejects payloads beyond `64 MiB`.
- Encodes up to `16 Ki` annotations.
- Converts local annotation ranges to fixed ABI records.
- Validates scalar boundaries in the JS payload before FFI.
- Splits u64 offsets/revisions into low/high 32-bit lanes.
- Checks ABI metadata and schema/build fingerprints once per runtime environment.
- Decodes mutation result lanes.
- Calls `requestWake()` if the returned scheduling bit is set.

#### Native/Rust data-plane behavior

`content_ffi.rs` exports:

- `iyon_tui_source_append_utf8_v1`
- `iyon_tui_source_replace_utf8_v1`
- `iyon_tui_source_clear_v1`
- `iyon_tui_source_seal_v1`
- `iyon_tui_source_head_truncate_v1`

The append entrypoint forwards to `HostContentSource::append_utf8`.

Rust append validation and mutation order:

1. Lock Source record.
2. Verify Source is live.
3. Verify kind is `Stream`.
4. Reject sealed Source.
5. Validate payload size and UTF-8 once.
6. Capture current absolute `base = storage.end()`.
7. Check `base + input.len()` for u64 exhaustion.
8. Decode/validate annotations relative to this append.
9. Return unchanged revision for an empty payload with no annotations.
10. Check retention preflight.
11. Preflight revision increment.
12. Use in-place append only when:
    - annotations are empty, and
    - retention will not require truncation.
13. Otherwise build a persistent candidate with `StoredSource::apply_append`.
14. Apply retention if required.
15. Swap the `Arc<StoredSource>` only after all fallible checks succeed.
16. Increment accepted/copied/drop accounting.
17. Capture subscriber groups.
18. Release the Source lock.
19. Fan out host wake groups.

The source comments explicitly identify failure atomicity: a failed validation must not leave partial bytes, annotations, revision, or counters installed.

### 4.3 Source storage append and persistent structure

`source_store.rs` implements:

- immutable UTF-8 pages
- chunk range views
- persistent chunk tree
- persistent annotation treap/index
- derived line-entry queries.

#### Chunk append

`ChunkTree::append_in_place`:

- chunks appends into approximately 16 KiB UTF-8-safe pages;
- avoids splitting inside a UTF-8 scalar;
- copies through uniquely owned right-edge nodes where possible;
- uses `Arc::make_mut` for shared nodes;
- copies only the right-edge path if snapshots retain old roots;
- splits full leaves/branches as necessary.

`ChunkTree::appended` clones the tree value and appends, preserving old snapshots.

#### Annotation append

`AnnotationTree` is keyed by `(start_byte, seqno)`:

- index order supports overlap lookup;
- sequence number preserves application order;
- overlap results are sorted by `seqno` before semantic application;
- append batches merge into the existing tree;
- ordered annotation vectors are materialized lazily through `OnceLock`.

#### Snapshot behavior

Snapshots retain the old `Arc<StoredSource>` roots. Appending or truncating a live Source therefore does not mutate an already-acquired snapshot. The old and new snapshots can share page Arcs and tree subtrees.

### 4.4 Replacement and clear

#### Replace

`HostContentSource::replace_utf8`:

- is valid for both Block and Stream Sources;
- rejects replacement on a sealed Stream Source;
- validates the new payload and annotations;
- preflights revision and content-generation counters;
- builds a fresh `StoredSource::empty()` plus the replacement payload;
- applies retention;
- swaps the new storage;
- increments `content_generation`;
- records accounting;
- wakes subscribers.

Replacement creates a new logical document. Connector parser lineage is consequently invalidated even if the new retained range has the same coordinates.

#### Clear

`HostContentSource::clear`:

- rejects clear on a sealed Stream Source;
- detects empty/no-op state and returns the existing revision;
- preflights revision and content-generation counters;
- installs `StoredSource::empty()`;
- increments content generation and revision;
- wakes subscribers.

The Connector execution path resets parser state when it observes a new `ContentLineage`. Renderer policy remains reusable; parser state does not.

### 4.5 Sealing

```text
TextStreamSource.seal()
  → sealTextSource
  → iyon_tui_source_seal_v1
  → HostContentSource::seal
  → StoredSource::apply_seal
  → Source revision increment
  → subscriber wake
```

Rust rules:

- only Stream Sources can seal;
- sealing an already sealed Source returns `SOURCE_ALREADY_SEALED`;
- revision arithmetic is preflighted before flipping the flag;
- the stored sealed marker and optional marker annotation are ordered so a concurrent snapshot does not observe a marker without the seal;
- Source `sealed = true`, `sealed_at = end`.

Sealing affects projection and smoothing:

- a `Projection` with `sealed = true` must have stable frontier equal to source end;
- `Smooth::update` catches up to sealed input and clears pending temporal work;
- a smoothed Connector may still have a frame/candidate backlog until the host commits the resulting projection, but the Smooth state itself no longer schedules further ticks.

### 4.6 Head truncation

```text
TextStreamSource.truncateHead(offset)
  → truncateTextSource
  → iyon_tui_source_head_truncate_v1
  → HostContentSource::truncate_head
  → StoredSource::apply_truncate
  → chunk/annotation persistent split
  → revision increment
  → subscriber wake
```

Validation:

- `offset` must be within `[storage.base, storage.end]`;
- `offset` must be a UTF-8 scalar boundary;
- truncating at the current base is a no-op;
- revision arithmetic is preflighted before storage replacement.

Storage behavior:

- chunk roots split at the absolute offset;
- page bytes are shared when a split is inside a page;
- the new Source base becomes the truncation offset;
- absolute coordinates after the head remain unchanged;
- `head_partial` records whether the retained head begins inside a logical line;
- annotation records are clipped, dropped, or preserved according to annotation kind:
  - tag/style: clip straddling records;
  - atomic: drop when the original range cannot be represented truthfully;
  - point: retain only points at/after the new base;
  - unknown kinds fail closed.

Truncation invalidates semantic cache keys because Source base/end and Source revision participate in the keys. Parser lineage itself does not necessarily change merely due to head truncation if Source/content generation remains the same, but parser/projection validation sees the changed root range.

### 4.7 Raw Source projection

`source_projection(snapshot)` creates a `Projection<TextContent>` whose spans are raw page-backed `TextContent::Raw` values:

- source base = snapshot base;
- stable frontier = snapshot end;
- source end = snapshot end;
- sealed follows snapshot;
- each `ChunkView` becomes one contiguous source span;
- `RawText::from_page_slice` retains the backing page Arc and range instead of copying.

`source_grapheme_projection(snapshot)` is used for smoothing:

- iterates chunk views;
- carries the final grapheme across chunk/append boundaries;
- emits complete grapheme spans;
- retains only the final temporary grapheme between chunks;
- preserves absolute UTF-8 byte coordinates;
- avoids materializing the entire Source.

### 4.8 Funnel-specific semantic projection

`project_semantic_snapshot`:

1. Resets parser state when `ContentLineage` changes.
2. Builds raw Source projection.
3. Selects one projector:
   - `PlainTextProjector`
   - Connector-local `MarkdownProjector`
   - Connector-local `DiffProjector`
   - Connector-local `AnsiProjector`
4. Applies Source annotations through `SourceAnnotationRewriter` if present.

Parser instances are retained inside active Connector execution so incremental append behavior can reuse parser state. Replacement/clear resets only parser state associated with the old lineage.

`content/text/source.rs` preserves source provenance:

- one-page raw spans can stay page-backed;
- multi-page domains retain `RawPiece` witnesses;
- parser working text is lazily assembled only when required;
- exact runs preserve source-coordinate witnesses;
- derived runs map local parser ranges back to root Source ranges;
- `RawDomain` supports suffix/prefix extraction without forcing full-source materialization.

### 4.9 Smooth path

Smooth is Connector-local and optional:

```text
Source append/change
  → snapshot
  → source_grapheme_projection
  → ConnectorDelivery::accept_input
  → Smooth::project
  → stable/pending state
  → frame scheduler deadline
  → ContentHostRegistry::advance(now)
  → Smooth::advance(now)
  → delivery frontier/revision update
  → candidate projection invalidation
  → ContentDirty::DeliveryVisibility
  → next host frame
```

#### On new input

`ConnectorDelivery::accept_input` rebuilds the grapheme projection only when any of these change:

- Source generation
- Source revision
- sealed state.

It then calls `Smooth::project`, stores the units, and records the published frontier.

#### On a tick

`ContentHostRegistry::advance`:

- considers only Connectors in the active deadline index;
- counts due Connectors (`ContentDueConnectors`);
- calls `delivery.advance(now)`;
- updates the next deadline;
- if progress occurred:
  - copies the candidate delivery frontier;
  - increments `delivery_revision`;
  - clears only `candidate_projection`;
  - preserves `projection_cache` and `prepared_paint_cache`;
  - emits `ContentDirtyReason::DeliveryVisibility`.

This is a significant post-L1 cache semantic: a pure smoothing tick does not reparse the Source, rebuild semantic IR, or discard prepared width-dependent products. It changes the visible reveal frontier and forces a new Connector projection product.

### 4.10 Measurement and layout

`ContentHostRegistry::measure_content` is invoked by the presentation layer through `ContentProvider::measure`.

High-level route:

```text
measure(port_id, offered_width, width_rule)
  → inspect desired/visible Port association
  → prepare desired Connector candidate
  → prepare_connector_projection
  → projection_measurement / cached product
  → fit-width refinement
  → History committed-row adjustment
  → ContentMeasurement
```

Measurement selects a desired Connector if the Port is desired-mounted. If candidate preparation fails and a visible Connector exists, the old visible Connector can be prepared as a rollback product. This preserves the old visible state instead of replacing it with a failed candidate.

`ContentMeasurement` reports:

- intrinsic size
- physical completeness
- projection revision
- metric revision
- paint revision
- Connector identity
- projection identity.

Width is part of the projection and prepared-paint keys. A width change therefore misses width-dependent products but can reuse the semantic cache.

#### Content projection preparation

`project_text_snapshot` performs:

1. Projection-key construction.
2. Empty Source handling.
3. Coarse row/line bound calculation.
4. terminal row/line limit rejection.
5. semantic cache lookup/build.
6. prepared-paint cache lookup/build.
7. semantic-to-layout lowering.
8. optional physical row compilation.
9. optional finalized-prefix proof.
10. optional Smooth delivery reveal calculation.
11. construction of a fresh `HostContentProjection` with a monotonic identity.

The coarse `projected_bounds` pass counts bytes/newlines without first lowering the entire semantic IR. It rejects products exceeding the `u16::MAX` row/line constraints.

### 4.11 Paint path

Painting is intentionally ticket-based:

```text
prepared measurement/layout
  → PreparedProjectionTicket
  → ContentProvider::paint_window
  → paint_window_direct
  → projection_for_ticket
  → candidate/committed/cache identity matching
  → retained rows OR deferred row-range lowering
  → clip/cut/history-row adjustment
  → physical cell compositing into Surface
```

`paint_window_direct` does not select “the newest same-width projection”. It verifies:

- Connector ID
- projection identity
- offered width
- projection revision.

The projection may come from:

1. current candidate projection;
2. current committed projection;
3. Connector projection cache.

If no product matches, paint does nothing rather than painting an unrelated/newer product.

For products with retained rows:

- History committed rows are skipped.
- Window offsets are bounded by visible row count.
- Physical completeness is propagated to the target surface.
- reveal cuts limit the final row’s columns and suppress later rows.

For immediate non-History products without retained rows:

- the projection retains `LayoutTree`, `TextGeometryCache`, and theme;
- `ViewPainter::paint_tree_row_range_with_text_cache` generates only the requested row range;
- theme is taken from the immutable projection product, not the Connector’s newer theme.

### 4.12 History/finalized-prefix path

Open content can expose a finalized prefix separate from the open projection:

- `snapshot.stable_prefix()` produces an immutable sealed-range proof candidate.
- `prove_finalized_prefix` projects and compiles that prefix separately.
- `PrefixProofCache` retains a small two-entry working set.
- Markdown or other open syntax may render a stable prefix differently from the still-open trailing block.
- For smooth products, finalized rows are transferred only as the corresponding open rows become fully revealed.
- A sealed Source can still have a partial sink-visible backlog if delivery/History commit has not caught up.

This is not the same as Source sealing. Source sealing ends future mutation and makes the projection stable; finalized-prefix proof is a presentation/History policy for safely reusing rows from an open document.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Operation | Production route | Selection/conditions | Failure behavior |
|---|---|---|---|
| Create stream Source | `TextStreamSource.create` → `createTextSource("stream")` → Source registry | TS environment must be live | Native registration/disposal errors surface through wrapper |
| Create block Source | `TextBlockSource.create` → `createTextSource("block")` | Replacement-style Source | Same |
| Append | TS FFI → native C ABI → `HostContentSource::append_utf8` | Stream and unsealed only | Invalid UTF-8/range/retention/sealed/source identity reject atomically |
| Replace | TS FFI → native C ABI → `HostContentSource::replace_utf8` | Block or unsealed Stream | New content generation; old snapshots remain valid |
| Clear | TS FFI → Source clear | Block or unsealed Stream | No-op if already empty; otherwise new content generation |
| Seal | TS FFI → Source seal | Stream and unsealed | Already sealed is explicit error |
| Head truncate | TS FFI → Source truncate | Offset within retained UTF-8 boundaries | Out-of-range/non-boundary rejects without revision/storage change |
| Immediate projection | Connector prepare → semantic cache → layout/paint | Funnel delivery immediate | Projection failure is recorded; visible Connector may remain |
| Smooth projection | Connector Delivery → Smooth | Funnel delivery smooth | Tick failures remove poisoned Connector from due index and return scheduler error |
| Measure | `ContentProvider::measure` → `measure_content` | desired-mounted Port | Candidate failure can roll back to visible Connector |
| Paint | `paint_window` → ticket validation → rows/deferred lowering | Prepared ticket must still match | Stale/mismatched ticket is ignored |
| Connector activation | TS control → Rust request state → next candidate/frame | Connector control open | Activation failures are candidate failures, not immediate visible replacement |
| Connector dispose | TS begin disposal → Rust transactional disposal → status poll | May remain `"disposing"` | Wrapper finalizes only after native `"disposed"` |
| Source release | Source `dispose` after Connector membership zero | Source cannot be disposed while connected | `SOURCE_IN_USE` otherwise |
| Derived cache eviction | cache capacity and Connector inactivity | Small bounded working sets | Recompute from authoritative Source/snapshot |

### 5.2 Mutation atomicity and wake failures

The Source mutation methods separate acceptance from wake delivery:

- Storage/revision are installed under the Source lock only after preflight succeeds.
- Subscriber weak references are captured after acceptance.
- Wake groups are attempted after releasing the Source lock.
- A failed wake does not make the accepted append look like a failed mutation.
- Remaining subscriber groups are still attempted.
- Wake failure is recorded for later environment drain reporting.
- The mutation result still reports the accepted revision.

This prevents direct FFI callers from retrying a successful append and duplicating bytes merely because a host wake failed.

### 5.3 Projection failures and visible-state recovery

`ContentProjectionFailureKind` distinguishes:

- `LIMIT_EXCEEDED`
- `RETENTION_INCOMPATIBLE`
- `PROJECTION_FAILED`

On desired Connector projection failure:

1. Record the failure against the exact projection key.
2. Preserve the old visible Connector if present.
3. Attempt rollback measurement/projection using the visible Connector.
4. Keep requested/visible state separate.
5. Retry when a later Source revision or control change makes the failure key stale.

This is a cache-miss/recovery path, not silent fallback to arbitrary newest content.

### 5.4 Poisoned locks

The source and connector paths explicitly handle poisoned locks:

- Source lock poisoning rejects subsequent ordinary Source operations.
- Post-acceptance wake cleanup poisoning is reported as an accepted mutation plus wake failure.
- A poisoned Connector encountered during delivery advance is removed from active deadline indexes to prevent infinite retries.
- The scheduler returns a typed error rather than repeatedly attempting the same poisoned Connector.

### 5.5 Stale identities and stale tickets

Stale Source identity is checked by environment and Source generation. Stale TS wrappers are rejected before data operations.

Paint uses product identity and revision rather than pointer identity. This prevents:

- an old frame ticket from painting a newer Source append;
- a Connector switch from painting the old Connector’s same-width product;
- cache eviction/reallocation pointer ABA;
- a theme change from replacing a ticket’s captured palette.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Source storage retention and compaction

There is no public operation named `compact` in the inspected Source API. Compaction is implemented as persistent structural sharing and retention/truncation:

- `ChunkTree` append copies only the right-edge path as needed.
- `ChunkTree::split_at` shares page Arcs and re-describes only boundary views.
- `AnnotationTree::truncate_head` splits indexed roots and reconstructs only surviving boundary/path records.
- Dropped roots become unreachable and are reclaimed by Arc reference release.
- `apply_retention` invokes head truncation when configured byte/line limits require it.
- No global source text rewrite is needed for ordinary append/truncate.

Thus “compact” is currently implicit in storage operations and retention, not an independently exposed API.

### 6.2 Source/cache keys

#### Semantic projection key

`SemanticProjectionKey` includes:

- Source ID
- Source generation
- content generation
- Source revision
- Source base
- Source end
- sealed state
- Funnel kind
- hyperlinks

It intentionally excludes:

- theme
- width
- delivery tick/frontier
- viewport.

Consequences:

- theme-only changes reuse semantic IR;
- width changes reuse semantic IR;
- delivery ticks reuse semantic IR;
- append/replacement/truncate/source seal changes generally miss because revision/range/sealed state changes;
- replacement/clear cannot collide with an old parser document because content generation participates.

#### Text projection key

`TextProjectionKey` includes:

- Source identity/generations
- Source revision
- width
- wrap
- Funnel kind
- delivery revision
- theme revision
- finalized-prefix need
- physical-row need.

It produces:

- projection revision
- metric revision
- layout-input revision
- paint revision.

#### Prepared paint key

`PreparedPaintKey` includes:

- semantic key
- theme revision
- width
- finalized-prefix need
- physical-row need.

This allows a theme-only repaint to reuse semantic IR and width-dependent layout while regenerating theme-resolved physical rows.

#### Prefix proof key

`PrefixProofKey` includes:

- semantic key
- stable prefix source end
- width.

It retains a separate proof/layout/text-geometry product for finalized open prefixes.

### 6.3 Cache bounds and retention

Current capacities:

```text
CONTENT_CACHE_CAPACITY        = 2
CONTENT_PREFIX_CACHE_CAPACITY = 2
```

Per active Connector:

- projection cache: bounded to two `HostContentProjection` products;
- prepared-paint cache: bounded to two products;
- semantic cache: bounded to two semantic projections;
- prefix proof cache: bounded to two proofs.

Inactive Connectors clear:

- committed/candidate projection;
- projection cache;
- prepared-paint cache;
- semantic cache;
- prefix proof cache;
- execution/parser/delivery;
- delivery frontiers and revisions.

The Source remains authoritative while inactive. Re-activation reconstructs derived products from a Source snapshot.

### 6.4 Append work

Per successful append, the following work occurs:

1. TS validates and UTF-8-encodes text.
2. TS validates/encodes annotations.
3. Native ABI copies/borrows the incoming buffers synchronously.
4. Rust validates UTF-8 once.
5. Rust counts newlines as part of the validation pass.
6. Rust performs coordinate/revision/retention preflight.
7. Storage appends pages/tree nodes.
8. Annotation batch is inserted if present.
9. Source revision/accounting updates.
10. Subscriber wake fanout.

What does **not** occur directly on append:

- no immediate parser run under the Source lock;
- no mandatory semantic projection;
- no mandatory layout or paint;
- no source-wide string materialization in the frame route.

Projection occurs later when a host candidate measures/prepares a visible Connector.

### 6.5 Per-snapshot work

`HostContentSource::snapshot`:

- locks the Source record;
- increments the `SourceSnapshotsAcquired` counter;
- clones the storage Arc and scalar metadata;
- does not copy Source text.

Diagnostic `snapshot.text()` materializes the entire retained text and is explicitly not used on the frame path.

### 6.6 Per-projection work

On a projection cache miss:

- raw Source chunks are converted to raw page-backed spans;
- the selected semantic Funnel parser runs;
- annotation rewriting may run;
- semantic IR is lowered through `TextRenderer`;
- layout tree is built for the offered width;
- text geometry cache is populated;
- physical rows are compiled only when required;
- finalized-prefix proof may run for History.

On a semantic cache hit:

- parser/projector work is skipped;
- layout/paint may still miss on width/theme/finalized-prefix/row requirements.

### 6.7 Per-tick work

On each native smoothing tick:

- due Connector IDs are selected from `active_deadlines`;
- `Smooth::advance(now)` runs only for due Connectors;
- no Source snapshot is acquired;
- no raw Source projection is rebuilt;
- no parser runs;
- no semantic cache is cleared;
- no prepared-paint cache is cleared;
- `delivery_revision` increments only if visible delivery progressed;
- candidate projection is cleared;
- a single `ContentDirtyReason::DeliveryVisibility` record is emitted.

The next deadline is kept in `active_deadlines`; inactive/not-requested Connectors are removed from it.

### 6.8 Per-width-change work

A width change:

- changes `TextProjectionKey.width`;
- changes `PreparedPaintKey.width`;
- invalidates/rebuilds width-dependent layout and physical rows;
- retains/reuses semantic IR because width is absent from `SemanticProjectionKey`;
- can reuse `TextGeometryCache` only when a matching prepared product/layout exists.

The coarse `projected_bounds` function also runs for the new offered width and can reject impossible products before full semantic compilation.

### 6.9 Per-frame work

During a frame candidate:

- candidate Source snapshots are captured once per Source ID where possible;
- Connector candidates are prepared;
- measurement and layout use candidate projection products;
- `PreparedContentCommit` captures immutable Arc products and record references;
- receipt-time commit promotes captured products without rebuilding or registry scanning.

During paint:

- the ticket chooses a specific Connector/product identity;
- retained rows are sliced, or deferred rows are generated for only the requested row range;
- physical cells are composited into the target Surface.

### 6.10 Observed counters and instrumentation

`perf.rs` defines content-related counters including:

- `SourceSnapshotsAcquired`
- `AnnotationRecordsCopied`
- `SemanticPreparations`
- `ContentPaintPropagations`
- `ContentWakeGroups`
- `ContentDueConnectors`
- `ContentCandidateRecordsPrepared`
- `ContentPathIndexNodesVisited`
- semantic/projection rebuild counters visible in content code.

The inspected code increments:

- `SemanticProjectionRebuilds` on semantic cache misses;
- `ContentWakeGroups` for Source wake fanout;
- `ContentDueConnectors` for due Smooth Connectors;
- `ContentCandidateRecordsPrepared` for candidate table preparation.

---

## 7. Tests, benchmarks and observability

### 7.1 Behavioral test coverage observed

Relevant test files include:

- `crates/iyon-tui/src/application/content.rs` embedded tests
- `crates/iyon-tui/src/application/source_store.rs` embedded tests
- `crates/iyon-tui/src/projection/tests.rs`
- `crates/iyon-tui/src/projection/tests/projection_public.rs`
- `crates/iyon-tui/src/projection/migrated_tests.rs`
- `crates/iyon-tui/src/content/text/tests/**`
- `crates/iyon-tui-native/src/content_ffi.rs` embedded tests
- `crates/iyon-tui-native/tests/generated_view_abi.rs`
- `packages/iyon-tui/src/testing/**` and content-related package tests by manifest/index.

The test source demonstrates contracts for:

- u64 coordinate exhaustion;
- append/truncate absolute-coordinate continuity;
- persistent snapshot immutability;
- page sharing on partial truncation;
- cross-page UTF-8 boundary handling;
- chunk-edge newline behavior;
- annotation truncation policy;
- projection coverage and transition validation;
- Smooth deterministic deadlines;
- Smooth sealed identity;
- incremental vs one-shot projector equivalence;
- candidate commit rollback;
- prepared ticket identity safety;
- theme changes not repainting stale ticket products;
- direct FFI wake-failure fanout.

### 7.2 Tests as architecture evidence

The Source storage tests confirm that:

- old snapshots stay immutable after append/truncate;
- page Arcs are shared through partial truncation;
- retained absolute coordinates remain contiguous after truncate + append;
- line entries are derived correctly across page/chunk boundaries.

Projection tests confirm that:

- source coverage is explicit;
- stable prefixes cannot be altered;
- sealed projections cannot be mutated;
- Smooth output is a prefix of stable input and catches up on seal;
- incremental Smooth/projector output matches one-shot output.

Content tests confirm that:

- a newer desired Connector does not overwrite an older in-flight candidate plan;
- candidate record tables are receipt-owned and do not grow during delayed commits;
- paint uses the ticket’s old product after a newer theme/candidate appears;
- source wake failures do not suppress healthy subscriber wake fanout.

### 7.3 Observability gaps

Static inspection identifies several areas where runtime visibility is limited:

- Cache hit/miss counters for every cache tier are not uniformly exposed.
- Projection identities and ticket mismatch reasons are internal; paint silently returns on a missing/mismatched product.
- Source snapshots expose text and metadata but not persistent tree sharing statistics.
- Smooth exposes status/deadlines through Connector internal state but TS only sees projected Source revision and phase/error, not delivery frontier.
- Connector status exposes `projectedSourceRevision`, but not delivery revision or projection identity.
- Retention does expose accepted/copied/dropped byte counters, which is useful for append/compaction accounting.
- No public “compact” operation or compaction report exists.

No tests were run for this report.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Bulk data and control lanes are separate

The current boundary is intentionally split:

- Source append/replace/clear/seal/truncate use direct bulk FFI in `transport/content/ffi.ts`.
- Port/Connector creation and lifecycle use N-API control methods in `transport/content/control.ts`.
- The control lane never owns Source payload bytes.
- The data lane does not create or destroy Connector membership.

This separation is reflected both in comments and actual call structure.

### 8.2 TS and Rust validation are intentionally duplicated

TS validates:

- JS object shapes;
- annotation kind/payload combinations;
- UTF-8 scalar boundaries in encoded payloads;
- numeric width and u64 lane bounds;
- Funnel/smoothing option types.

Rust validates again:

- native identity/generation;
- UTF-8;
- absolute range arithmetic;
- Source kind/sealed state;
- retention;
- revision/content-generation arithmetic;
- annotation semantics.

This is not a contradiction. TS provides ergonomic early errors; Rust remains authoritative for Source state and atomic mutation.

### 8.3 Snapshot text API versus frame path

`TextSourceSnapshot.text` is materializing and intentionally diagnostic. The frame path uses:

- `ChunkView`
- `RawText::from_page_slice`
- `RawDomain`
- persistent Source roots.

A caller can request full text through the public snapshot API, but doing so is not equivalent to what the renderer does during ordinary frames.

### 8.4 Source sealing versus finalized prefix

These are distinct:

- Source sealing is a mutation/lifecycle transition.
- Finalized-prefix proof is a projection/History optimization and semantic correctness policy for open content.

An open Source can produce a finalized prefix product. A sealed Source can still have an incomplete visible smoothed/History receipt until delivery catches up.

### 8.5 Projection cache invalidation intentionally differs by tier

A Source append changes semantic keys and therefore normally causes semantic rebuild. A Smooth tick changes delivery revision but deliberately leaves semantic and prepared-paint caches intact. A theme change clears Connector projection and candidate products but semantic keys remain theme-independent; the next preparation can reuse semantic IR.

This tiered behavior is consequential and should not be collapsed into “content dirty means all caches invalid”.

### 8.6 Candidate versus committed ownership

Candidate preparation captures:

- source snapshots;
- connector records;
- projection Arc products;
- delivery frontier;
- control revision.

Receipt-time commit does not resolve a newer Connector or rebuild a product. This prevents a race in which a newer append/theme/Connector switch changes the live candidate while an earlier backend receipt is still outstanding.

### 8.7 Source retention and presentation caches are independent

Source retention drops old bytes structurally and changes Source base/revision. Presentation caches retain derived products only by bounded keys. A presentation cache eviction does not affect Source retention; a Source truncate invalidates derived products through changed keys and source range.

### 8.8 Potentially consequential semantic detail: sealed Smooth output

`Smooth::update` catches up immediately when input is sealed, but `HostContentProjection` can still expose partial visible rows in History/smooth terms because the host’s committed rows and delivery receipt are separate. The source is semantically sealed while the frame may still paint only the candidate/committed visible prefix. This is intentional according to comments and tests, but callers should not equate `sealed == fully painted`.

---

## 9. Open questions and coverage gaps

1. **No public compact API**
   - Search of Source APIs found append, replace, clear, seal, truncate, snapshot, stats, and dispose, but no independently callable `compact`.
   - Current compaction is implicit through persistent-tree truncation and Arc reclamation.
   - It is unknown whether future product code expects an explicit compaction acknowledgment.

2. **No public delivery-frontier API**
   - TS Connector status exposes projected Source revision but not the current Smooth published frontier or delivery revision.
   - A caller cannot directly distinguish “Source accepted through revision N” from “Connector has revealed/painted through source offset X” except through frame behavior.

3. **No direct runtime cache-hit report**
   - Source stats report storage/accounting counters, but semantic/prepared-paint/projection cache hit rates are internal.
   - Perf counters provide rebuild/prepare counts but do not expose all cache tiers symmetrically.

4. **No executed validation in this report**
   - The report is static. Exact runtime behavior under malformed ABI pointers, poisoned locks, timer timing, and actual terminal backends remains dependent on the existing test suite.

5. **Content projection ownership remains concentrated**
   - `application/content.rs` currently contains Source-facing lifecycle, Connector execution, projection, smoothing integration, measurement, paint, History adaptation, and candidate commit logic.
   - This is an observed structure, not a disposition judgment.

6. **Annotation semantic policy is closed**
   - The current annotation kinds are tag/style/atomic/point.
   - Unknown kinds fail closed or are dropped according to current storage/projection policy.
   - No extensible plugin registration path was found.

7. **Source snapshot materialization cost is caller-controlled**
   - `snapshot.text()` can materialize the entire retained Source.
   - The frame route avoids it, but external callers can still request it repeatedly; there is no separate budget or streaming snapshot iterator in the public TS API.

8. **Retention-line semantics need product-level confirmation**
   - Rust tracks `retained_lines`, `head_partial`, and persistent newline entries.
   - The exact interpretation of `maxLines` around a partial retained head should be verified by the retention tests before relying on it as a UI-level policy.

9. **Backend independence of semantic IR**
   - Current semantic projection and provenance structures are generic Rust content types, but lowering immediately enters the terminal-oriented `TextRenderer`/layout/physical-row path.
   - This report records the route; it does not determine how much of the semantic IR is reusable by a non-terminal backend.

---

## 10. Evidence appendix

### 10.1 Exact paths and symbols inspected

#### TypeScript public APIs

- `packages/iyon-tui/src/api/content/retained.ts`
  - `TextRetentionPolicy`
  - `TextSourceOptions`
  - `TextSourceAnnotation`
  - `TextSourceMutation`
  - `TextSourceSnapshot`
  - `TextSourceStats`
  - `TextStreamSource`
  - `TextBlockSource`
  - `TextFunnel`
  - `ContentPort`
  - `ContentConnector`
  - `createContentPort`

#### TypeScript data transport

- `packages/iyon-tui/src/transport/content/ffi.ts`
  - `ContentFfiSymbols`
  - `MutationResult`
  - `finishMutation`
  - `sourceIdentity`
  - `encodeAnnotations`
  - `decodeSemanticStylePayload`
  - `appendTextSource`
  - `replaceTextSource`
  - `clearTextSource`
  - `sealTextSource`
  - `truncateTextSource`
  - `contentFfiMetadata`

#### TypeScript control transport

- `packages/iyon-tui/src/transport/content/control.ts`
  - `NativeTextFunnelControl`
  - `createTextSource`
  - `createContentPort`
  - `connectContent`
  - `activateContent`
  - `deactivateContent`
  - `disposeContentConnector`
  - `contentConnectorStatus`

- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeTextSourceContract`
  - `NativeContentConnectorContract`
  - `NativeContentPortContract`
  - `NativeStateWake`
  - `NativeTuiHostContract`

#### Rust Source/storage

- `crates/iyon-tui/src/application/content.rs`
  - `HostContentSourceSnapshot`
  - `TextProjectionKey`
  - `SemanticProjectionKey`
  - `ConnectorDelivery`
  - `ConnectorExecution`
  - `HostContentProjection`
  - `PreparedPaintKey`
  - `PreparedPaintProduct`
  - `PrefixProof`
  - `HostContentSourceStats`
  - `ContentMutationResult`
  - `source_projection`
  - `source_grapheme_projection`
  - `project_semantic_snapshot`
  - `prove_finalized_prefix`
  - `project_text_snapshot`
  - `HostContentSource::append_utf8`
  - `HostContentSource::replace_utf8`
  - `HostContentSource::clear`
  - `HostContentSource::seal`
  - `HostContentSource::truncate_head`
  - `HostContentSource::finish_mutation`
  - `ContentHostRegistry::advance`
  - `ContentHostRegistry::measure_content`
  - `ContentHostRegistry::paint_window_direct`
  - `ContentHostRegistry::projection_for_ticket`
  - `ContentProvider for ContentHostRegistry`

- `crates/iyon-tui/src/application/source_store.rs`
  - `ValidatedInput`
  - `ChunkTree`
  - `ChunkTree::append_in_place`
  - `ChunkTree::split_at`
  - `ChunkTree::truncated_head`
  - `AnnotationTree`
  - `AnnotationTree::truncate_head`
  - `StoredSource`
  - `StoredSource::apply_append`
  - `StoredSource::append_in_place`
  - `StoredSource::apply_seal`
  - `StoredSource::apply_truncate`
  - `StoredSource::stable_prefix`

#### Rust projection/stream

- `crates/iyon-tui/src/stream/coord.rs`
  - `StreamOffset`
  - `StreamRange`

- `crates/iyon-tui/src/projection/value.rs`
  - `Projection`
  - `ProjectionSpan`
  - `ProjectionBuilder`
  - `Projection::append_span`
  - `Projection::append_span_many`
  - `Projection::set_envelope`
  - `Projection::spans_from`
  - `Projection::map`
  - `Projection::map_ref`

- `crates/iyon-tui/src/projection/smooth.rs`
  - `SmoothConfig`
  - `Smooth`
  - `Smooth::ensure_clock`
  - `Smooth::advance`
  - `Smooth::rebuild_pending`
  - `Smooth::update`
  - `Smooth::project_incremental`

- `crates/iyon-tui/src/projection/validate.rs`
  - `validate_projection`
  - `validate_projection_relation`
  - `validate_projection_transition`
  - `ProjectionValidationError`
  - `ProjectionRelationError`
  - `ProjectionTransitionError`

- `crates/iyon-tui/src/content/text/source.rs`
  - `RawDomain`
  - `RawPiece`
  - `RawDomain::from_spans`
  - `RawDomain::text`
  - `RawDomain::prefix`
  - `RawDomain::suffix`
  - `RawDomain::exact_runs`
  - `RawDomain::derived_run`

#### Rust presentation/paint

- `crates/iyon-tui/src/presentation/content.rs`
  - `ContentDirtyReason`
  - `ContentMeasurement`
  - `PreparedProjectionTicket`
  - `HistoryContentRows`
  - `ContentProvider`

- `crates/iyon-tui/src/presentation/paint/mod.rs`
  - `TextGeometryCache`
  - `ViewPainter`
  - `PaintCache`

- `crates/iyon-tui/src/presentation/layout/**`
  - `ViewCompiler`
  - `LayoutTree`
  - `compile_tree_with_text_cache`
  - width/layout measurement and row generation paths.

#### Native ABI

- `crates/iyon-tui-native/src/content_ffi.rs`
  - `iyon_tui_source_append_utf8_v1`
  - `iyon_tui_source_replace_utf8_v1`
  - `iyon_tui_source_clear_v1`
  - `iyon_tui_source_seal_v1`
  - `iyon_tui_source_head_truncate_v1`
  - ABI metadata/result structures
  - direct-FFI wake-failure tests.

### 10.2 Inspected-file manifest grouped by concern

#### Contract and context

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

#### TS source authoring/transport

- `packages/iyon-tui/src/api/content/retained.ts`
- `packages/iyon-tui/src/transport/content/ffi.ts`
- `packages/iyon-tui/src/transport/content/control.ts`
- `packages/iyon-tui/src/transport/native/addon.ts`
- `packages/iyon-tui/src/transport/native/resources.ts`
- `packages/iyon-tui/src/runtime/environment.ts`
- `packages/iyon-tui/src/runtime/handle-registry.ts`

#### Rust content/source

- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/source_store.rs`
- `crates/iyon-tui/src/application/environment.rs`
- `crates/iyon-tui/src/application/host.rs`

#### Rust projection/content/text

- `crates/iyon-tui/src/stream/mod.rs`
- `crates/iyon-tui/src/stream/coord.rs`
- `crates/iyon-tui/src/projection/mod.rs`
- `crates/iyon-tui/src/projection/value.rs`
- `crates/iyon-tui/src/projection/smooth.rs`
- `crates/iyon-tui/src/projection/validate.rs`
- `crates/iyon-tui/src/projection/compose.rs`
- `crates/iyon-tui/src/content/text/source.rs`
- `crates/iyon-tui/src/content/text/plain.rs`
- `crates/iyon-tui/src/content/text/markdown.rs`
- `crates/iyon-tui/src/content/text/diff.rs`
- `crates/iyon-tui/src/content/text/ansi.rs`
- `crates/iyon-tui/src/content/text/content.rs`
- `crates/iyon-tui/src/content/text/render/**`

#### Rust presentation

- `crates/iyon-tui/src/presentation/content.rs`
- `crates/iyon-tui/src/presentation/layout/**`
- `crates/iyon-tui/src/presentation/paint/**`
- `crates/iyon-tui/src/physical/**`

#### Native ABI

- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/generated/**`

### 10.3 Files indexed but not comprehensively read

The repository-wide file manifest includes many unrelated Rust/TS modules. For this assignment, unrelated application, input, control, backend, and structural modules were indexed to identify seams but not exhaustively traced. Content-related tests were sampled by symbol/search scope; no test suite was run.

### 10.4 LOC methodology

- Production LOC figures are approximate source line extents for the inspected modules, excluding or separating obvious embedded test sections where identifiable.
- Generated ABI files are listed separately and are not counted as handwritten content runtime.
- No repository-editing or shell line-count operation was performed in this read-only investigation.
- Therefore LOC figures should be treated as architecture-scale estimates, not exact census counts.