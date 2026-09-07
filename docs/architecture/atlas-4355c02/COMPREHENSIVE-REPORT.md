# Comprehensive architecture inventory — 4355c02

Status: **complete current-state synthesis**. All45 primary reports plus grouped follow-up46 fully read by the parent. Findings remain explicitly unresolved where evidence does not establish correctness, safety, intent or production reachability.

## 0. Scope, authority and evidence

| Field | Value |
|---|---|
| Source | `4355c02d6853549adf32a1e038b14665ce5c6bf8`, branch `main` |
| Repository | `iyon-tui`; 1,088 tracked baseline paths; original collected manifest1,082, six omissions reconciled in [coverage addendum](evidence/coverage-addendum.md) |
| Investigation | 26 local slices, 3 wiring maps, 11 behavior traces, 5 audits |
| Changes authorized | Architecture documentation only; no implementation, dependencies, new tests or AGENTS changes |
| Purpose | Current-state technical inventory for subsequent census/disposition work, not a V5 implementation plan |
| Reading evidence | [Full-text ledger](evidence/parent-reading-ledger.md), [source-check notes](evidence/parent-notes/), [provenance](evidence/report-provenance.json) |
| Execution evidence | Report test descriptions are static inspection unless explicitly marked executed; prior L1 results are historical |
| Descriptive authority | Current source determines what actually runs |
| Intended-contract authority | Approved, unsuperseded PERF13/API-H/L1/PRE-V5 handoffs; deviations require explanation, not automatic reclassification as stale docs |

This is the agent-readable inventory. The [maintained guide](../CURRENT-ARCHITECTURE.md) explains the system for humans. The 45 reports preserve exhaustive local symbol inventories, source ranges and test navigation; this document integrates their overlapping claims rather than concatenating them. [Reconciliation](RECONCILIATION.md) overrides identified report errors. [Issues](ISSUES.md) separates defects, contract deviations, evidence limitations and unresolved risks.

Path abbreviations below are exact repository prefixes: **R** = `crates/iyon-tui/src/`; **N** = `crates/iyon-tui-native/src/`; **T** = `packages/iyon-tui/src/`. Report IDs refer to the [atlas index](README.md).

## 1. Packages and supported surfaces

| Unit | Actual responsibility | Boundary |
|---|---|---|
| `crates/iyon-tui` | Canonical generic terminal runtime, semantic presentation, retained components/state/content, layout, paint, History, backend | Unpublished Rust implementation; not a supported external Rust authoring SDK |
| `crates/iyon-tui-native` | N-API host/control wrappers; generated structural ABI adapter; state decoding; direct content C ABI | Calls curated `iyon_tui::binding`; owns wire identities and structural lease tables, not application policy |
| `tools/tui-abi-gen` | Build-time TOML/schema compiler | No runtime Cargo dependency edge |
| `packages/iyon-tui` / `@iyon/tui` | Supported TypeScript semantic authoring, retained execution, lifecycle, native transport | Root, `./testing`, `./native-stage` package exports |
| `packages/tui-consumer-fixture` | Workspace public-import consumer | Not packed-artifact or independently installed package proof |

The three Cargo members are the two crates and generator. Rust `lib.rs` exposes a curated flat `binding` seam and feature-gated, doc-hidden performance tooling. Internal `pub` declarations inside private modules do not make the internal App/component/projector system a public Rust SDK. Conversely, passive types re-exported through `binding` can remain technically nameable; “unsupported authoring API” is not “no accessible Rust items.”

The governing generic-framework boundary remains mandatory. Terminal correctness, input routing, semantic layout/rendering, History, projection and native ticking belong here. Agent/provider/tool orchestration and product labels/policies do not. The planned `iyon-api` / `iyon-core` / `iyon-tui` / TypeScript-plugin split is an intended ownership contract, not a claim that those product packages exist in this repository.

### 1.1 TypeScript surface inventory

| Group | Main exposed concepts | Important qualification |
|---|---|---|
| Structure | `View`, layout builders, `Scene`, `Insets` | Immutable semantic description; not cells or native registry ownership |
| Execution | `defineView`, `state`, `View.key` | Scope identity and subscriptions; keys alone do not create independent scheduling scopes |
| Retained state | `ViewState`, geometry/presentation/style-state patches | Native occurrence overrides, distinct from tracked JS state |
| Styling | Theme, styles, selectors, semantic facts/states | Generic caller policy; metric-neutral theme changes still require repaint |
| Content runtime | Source, Funnel, Port, Connector | Source-neutral immutable funnel; Rust executes parsing/pacing/rendering |
| Content values | `TextContent`, `RawText`, `Projection`, `Smooth`, diff values/rendering | The lightweight TS text/projection classes do **not** provide full native semantic parity |
| Controls | `History`, `TextInput`; factory-created `ViewSlot`, `ScrollPane` | Slot/pane and several handle contracts are type-only root exports |
| Runtime | `Tui`, runtime contracts and routed outputs | High-level callbacks consume native outputs; no per-key JS interpreter |
| Testing | `AppHarness`, `createAppHarness` via `@iyon/tui/testing` | Not root exports; harness drives production headless host plus explicit clock/barriers |

Evidence: reports 16, 19, 25, 27–28, 30; `R/lib.rs`, `R/binding/mod.rs`, `T/index.ts`, package manifests.

## 2. Ownership graph and independent identity domains

```text
TS semantic values + tracked execution scopes
    | publication preparation; resource/attachment validation
    v
TS root boundaries, leases, weak native hints
    | generated structural N-API / state N-API / direct content FFI
    v
native environment: NativeViewRuntime + content environment registry
    | semantic View clones and host/control operations
    v
HostInner: desired roots + components + state + ports/connectors
    | candidate capture, resolution, preparation
    v
SceneHost: mount/focus graph + retained layout + paint products
    | PreparedSceneFrame + one outstanding backend receipt
    v
backend: visible model, terminal worker, irreversible native scrollback
```

| Identity | Owner and meaning | Not interchangeable with |
|---|---|---|
| JS framework HandleId | Private resource registry, owner/environment validation | NodeId, ComponentId, NativeRef |
| Execution scope identity | Parent + ordinal/type, or parent-local key group | Semantic equality; native component identity |
| TS semantic NodeId | Monotonic JS-safe semantic value identity | Rust ViewId or native lease |
| Rust ViewId | Immutable semantic View root identity; mutation creates a new ID | Occurrence/layout index |
| Native ViewRef | Runtime-local retained-reference slot with explicit lease count | JS weak hint or semantic NodeId |
| Native path/style/build/edit IDs | Separate runtime-owned tables and disjoint reserved domains where applicable | ViewRef |
| ComponentId | Process-monotonic registry identity, nonowning typed handles | Component lifetime or mount status |
| State attachment ID | Host-qualified JS-safe ID, registry-owned record | Style-state key/value bag |
| Layout node ID | Index in a prepared flat LayoutTree | Stable cross-tree component or semantic identity |
| Source identity | Environment slot/generation + Source ID/generation | Offset or revision alone |
| Source content generation | Parser lineage after replacement/clear | Source lifetime generation |
| Source revision | Accepted mutation order | Projected, submitted or visible revision |
| Port / Connector identity | Host attachment and independently paced projection selection | Source identity or width |
| HistoryUnitId | Monotonic semantic ordered-unit identity | Physical row number |
| OutputId | Typed queue/router channel identity | Component ID or caller route ID |
| Host epochs/revisions | Pending/committed work and desired/visible structure | One universal transaction number |

Three **retention mechanisms** coexist: TS execution state; TS accepted-native knowledge/leases; native authoritative retained runtime. Three **mutation planes** coexist: structure, retained state, content. These are different classifications.

### 2.1 Strong versus weak ownership

| Owner | Strongly retains | Weak/nonowning links |
|---|---|---|
| TS scope | Committed/pending children, output, props, dependencies, semantic slots, projection target | State subscription coordination is runtime-managed |
| TS root/attachments | Prepared, desired, visible and superseded keepalive/lease roles | WeakMap native hints are never ownership |
| NativeViewRuntime | Views with outstanding JS leases; builders/edit transactions | NodeId weak View associations, unleased cache entries |
| ComponentRegistry | Boxed erased component values | Typed handles carry IDs only |
| HostInner | Runtime, scene, state records, content registry, candidate/visible products, backend | Environment registers hosts weakly |
| State candidate | Immutable Arc versions for demanded records | Wrapper weak host; disposal guarded by desired/visible/in-flight use |
| Source environment | Source records and storage | Host subscriptions weak; connector membership separately counted |
| Connector | Source membership plus parser/delivery/render caches when active | Port/host authority remains native |
| HostHistory | Strong host Arc | Unlike weak-host state/control wrappers |
| Prepared layout/cache | Measured nodes retain semantic Views; flat layout retains rendering payloads | “Flat LayoutTree” does not imply all caches are tree-free |
| Backend worker | Actual terminal and retained terminal Surface model | Host sees command/receipt channel |

No single cache is the sole owner of the semantic tree. Host roots, History, slots, components, candidate frames and temporary transactions can retain shared View graphs after a NativeRef lease is released.

Evidence: reports 02, 06, 10, 17–23, 29, 39–40; `R/application/host.rs`, `R/retained_state/registry.rs`, `N/tui/view_abi.rs`, `T/runtime/native-resource-registry.ts`.

## 3. Mutation-plane contracts and frame convergence

| Plane | Public operation | Production crossing | Native authoritative data | Does not happen at ingress |
|---|---|---|---|---|
| Structure | Construct/render semantic View; publish scope/control output | Generated structural **N-API** session, fixed/buffer/path/edit helpers as selected | Shared immutable View graph, desired host root, retained identities/leases | No JSON whole-scene fallback; acceptance is not terminal visibility |
| Retained state | Geometry/presentation/style-state patch | N-API fixed generated envelope + handwritten semantic decoder | Host ViewStateRegistry and immutable candidate versions | No JS rebuild required for native-only patch; no universal self-only repaint promise |
| Content | Source append/replace/clear/seal/truncate | **Direct content FFI**; N-API only for controls/identity/diagnostics | Environment Source registry, persistent bytes/annotations/revision | No synchronous parse/layout/paint on accepted byte mutation |

`NativeTextSource` does **not** have N-API append/replace/clear/seal payload methods. The structural `direct-ffi` build feature exposes qualification C symbols; it does not switch the TypeScript production structural transport. Feature presence, exported symbols and printed benchmark transport labels are not evidence of actual direct dispatch.

### 3.1 Commit boundaries

1. **TS evaluation/preparation:** WIP dependencies, child owners, semantic slots and publications are staged. Ordinary evaluation/preparation failure preserves previous committed scope state.
2. **Desired acceptance:** Native roots or control state can be accepted during publication. Native pending epochs advance. This can occur inside the TS commit phase rather than strictly after all JS bookkeeping.
3. **Candidate capture:** State versions, content selection/projection/frontiers, structure revision and work epoch are captured.
4. **Preparation/submission:** Scene resolution/layout/paint produces a frame; backend receives it and returns a receipt.
5. **Visible promotion:** A successful exact receipt authorizes corresponding content/state/frame visibility. A newer desired revision remains pending.

A failed receipt preserves the previous logical visible frame, not all prior mutable state or irreversible I/O. Native `host.render` sets desired state before a subsequent flush can fail; slot setters likewise mutate before rendering can fail. Therefore “old root unchanged on every error” and “all commits are one rollback transaction” are false. Mount-graph reconciliation, component retirement, terminal acceptance and TypeScript bookkeeping must each be named explicitly.

Accepted content bytes survive later wake failure. The environment reports per-host errors and still attempts healthy fanout. A failed automatic epoch is blocked to prevent spin; a newer mutation or explicit retry barrier can retry. The TS flush loop's bound of 64 covers error-free incomplete drain iterations, not 64 swallowed retries of one reported error.

Evidence: reports 01, 06, 17–18, 21–23, 29–35, 39; parent source checks in notes 17, 21, 29, 31, 39.

## 4. TypeScript semantic authoring and retained execution

### 4.1 Values and composition tables

| Mechanism | Exact behavior | Cost/limitation |
|---|---|---|
| View wrapper | Frozen wrapper; semantic node in private WeakMap; attachment/derivation sidecars | Frozen wrapper does not deep-freeze all caller-created style/theme/content values |
| Semantic replacement | Fresh semantic NodeId; unchanged child values shared | Equality and identity are distinct |
| `defineView` | Callable component token; retained scope selected by parent/type/position or key group | Recreating the token remounts; function-valued producer replacement does not necessarily replace the root scope |
| Prop comparison | Object.is, then enumerable own string keys with Object.is field values | No deep comparison; symbols/nonenumerables excluded; same mutated object can bypass change detection |
| Tracked `State<T>` | Object.is value; reads during evaluation subscribe; changed scopes queued once | Writes in component body rejected; no proxy/deep mutation tracking |
| Key group | Parent-local child ownership namespace | Key thunk state reads subscribe parent; key alone is not an execution scope |
| Scope semantic table | Ordered operation slots; staged/current values; commit truncates unused tail | Call-order identity, not compiler-generated site identity |
| Child owner | Separate WIP/current child arrays and lazy keyed maps | Evaluated-empty differs from not visited |
| Persistent sequence | Branch factor 32, path-copy localized edits | Inserted values still cost work; public composition can rebuild parent rather than invoke localized ABI edits |

The runtime is one `RetainedExecutionRuntime` per Tui shared across the scene producer, projected children and slot/pane builders. Nested `defineView` scopes publish through native ViewSlots seeded with an empty spacer; parent output contains a stable component reference. This is why a descendant can update without re-evaluating its parent.

A composition helper first constructs/evaluates its child inputs, then compares the normalized operation slot. Wide axis/grid equality deliberately avoids flattening large sidecars. That is not proof that ordinary public wide-parent rerendering uses native set/splice. Existing native path/axis/grid helpers are capabilities; actual caller reachability determines production performance.

### 4.2 Publication and direct takeover

- Publication protocol carries semantic Views through prepare/commit/abort; composition itself does not import native transport.
- Prepare all affected publications before committing them descendant-first.
- Failed ordinary prepare rolls back staged dependencies/output/owners and restores dirty obligations; automatic retry behavior is not an unbounded loop.
- Pathological commit exceptions may leave partial native acceptance; publication is not a general distributed rollback transaction.
- Canonical scene producer and direct scene rendering use deferred desired installation.
- Direct scene/control takeover prepares acceptance before releasing the prior builder, preserving it on ordinary pre-acceptance failure.
- Exact same-body cutoff still performs required ownership validation and builder takeover.
- Native slots retain component identity while replacing semantic content; animation frame selection/ticking remains in Rust.

Evidence: reports 19–21, 28–31, 39; `T/composition/{compose,execution,publication,tracked-state}.ts`, `T/runtime/runtime.ts`.

## 5. Structural transport and ABI ownership

### 5.1 Materialization pipeline

```text
semantic View
 -> generation-scoped weak native hint
 -> transaction-local dedup / eligible NodeId lookup
 -> derivation/path attempt or children-first materialization
 -> temporary strong NativeRef leases
 -> root desired-install boundary
 -> release temporaries, retain required root/attachment roles
```

On an exact retained hit, the helper can avoid semantic descent. A stale hint may recover once via semantic NodeId lookup; hints do not themselves authorize use after native reclamation. Eligibility ceilings avoid pointless lookups for newly allocated JS semantic IDs. Cycles, foreign/disposed resources and invalid attachments are validated at their respective boundaries.

| Representation | Production strategy | Distinct alternate/helper path |
|---|---|---|
| Axis 0–4 children | Fixed arity generated call | Same semantic constructor underneath |
| Axis >4 | `materializeAxisNode` uses one scratch pair buffer and `viewAxisCreateBuffer` | Specialized `tryRetainedAxisCreate` uses builder begin/push/finish for larger arrays |
| Text <=4 spans, NUL-free | Fixed CString lane where eligible | CString owns per-span copied text and terminates at NUL |
| Text containing NUL / UTF-8 framing | Exact UTF-8 lane | Native structural page retains validated ranges |
| Larger text spans | Word/byte buffer framing | Not direct C invocation in production TS |
| Local derived path | Persistent path recovery/rebuild where selected | Ordinary public composition is not blanket path mutation |
| Multi-edit transaction | Bounded trie, shared ancestor rebuild, staged publication | Host desired acceptance can precede a flush error |
| Diff | Typed hunk validation and direct semantic lowering to Column | Not the text-stream permissive diff projector |

### 5.2 Native runtime tables and validation

- Runtime is environment-scoped and Arc-retained by sessions; header includes ABI/semantic version, alive/generation/thread checks.
- Native view slots are paged at 4,096 entries; each stores NodeId, WeakView, optional strongly leased View, lease count and kind.
- References are monotonic within runtime generation; released slots are not reused as generation-tagged handles.
- `refForNodeId` acquires an independent lease on each successful lookup.
- Identity-first constructor reuse occurs before handwritten semantic decoding on a live hit, but generated pointer/count/capacity guards still precede the handwritten call.
- Weak expiry permits removal; release-batch maintenance is bounded (256 candidates), with insertion-triggered full sweeps. Still-live candidates are removed from the candidate queue rather than perpetually requeued: no fixed idle reclamation guarantee follows.
- Builders own strong input Views; finish removes the builder before final validation.
- Edit transactions own a strong base, at most 256 edits, 16 MiB text, 4,096 objects and operation depth 4; path storage itself permits deeper interned paths up to its separate limit.
- One host disposal aborts environment-wide staged builders/edit transactions, not only that host's staging.
- Style/path tables have generation lifetime without ordinary per-entry eviction; page-directory high water can remain after page release.
- Memory diagnostic `string_bytes: null` means unknown, not zero.

Ordinary `runtime_mut` dereferences the supplied pointer and then checks its header; it does not first establish live registry membership. Generated non-null guards are not pointer-provenance proof. The Arc-to-`&'static mut` helper and host unsafe thread markers remain explicit safety follow-up items, not assurances derived from comments.

Evidence: reports 17, 22, 24, 29–31, 40–43; `N/tui/view_abi.rs`, `T/transport/structural/{retained-dag,native-view-abi,retained-path}.ts`.

## 6. Native retained state

| Layer | Stored data / operation |
|---|---|
| TS `ViewState` | Host-owned branded handle; attachment identity; normalizes patches and rejects duplicate clear entries |
| Generated envelope | Geometry: 10 properties, 14 words, 0 strings; presentation: 7 properties, 5 words, 14 strings; set/null/clear masks |
| Handwritten native decoder | Exact lengths, legal masks/nulls, active lanes, typed values; decode whole patch before mutation |
| Host registry | Mutable records, latest logical Arc versions, dirty IDs, desired/visible/in-flight demand sets |
| Candidate overlay | Captured epoch, demanded IDs and touched immutable versions |
| OccurrenceBox | Base/effective width, height, gap, alignment, decoration, style states, node kind, attachment |
| Scene update | Invalidate ancestors as required; presentation repaint or geometry dependency-frontier repair |

Geometry kinds are Text, Spacer, Row, Column, Grid, Hanging, Container, ClampRows, RowViewport, ContentHost and ComponentSlot. Gap applies only to Row/Column/Grid; horizontal alignment is Text-specific and vertical alignment Row-specific. A slot has no ordinary physical presentation box. Unbound records can defer kind-specific validation until mount.

Geometry patch absence preserves an override; explicit null clears it to base/default semantics. Nullable minimum/maximum bounds normalize to 0/u16::MAX. Border-edge changes can create/remove structural border allocation; presentation changes only recolor/restyle an existing border. Direct style merges differ from named-style replacement; attributes apply last.

No-op mutation does not advance revision/effects. Accepted mutation of a demanded record updates the registry's table named `committed` immediately: that table means latest immutable **logical** version, not necessarily painted/visible version. Older candidate Arcs remain pinned through the receipt boundary. Unmounted dirty records can remain dirty without frame capture. Explicit disposal rejects desired/visible/in-flight use with mounted-state error; wrapper GC is not record disposal.

Effects contain richer flags than the dispatcher independently consumes. Geometry/intrinsic dependencies control layout escalation; presentation updates conservatively paint subtrees. Style-state change uses subtree paint rather than adding a separate self-paint requirement. Damage rectangles merge touching regions and promote to full above 64 rectangles or half-viewport area; the real backend currently does not consume that region metadata.

Evidence: reports 06, 17, 22, 29, 32; `R/retained_state/`, `R/application/view_state.rs`, `N/tui/view_state.rs`, `T/transport/state/`.

## 7. Source storage, content controls and native ingress

### 7.1 Control/data split

| Object | Authoritative owner | Operations / lifecycle |
|---|---|---|
| TextStreamSource | Native environment | Append, replace, clear, seal, truncate; snapshots/stats; host-neutral |
| TextBlockSource | Native environment | Whole block replacement/clear as supported; not stream append/seal |
| Funnel | Immutable TS configuration, decoded native policy | Plain/Markdown/diff/ANSI, wrapping, hyperlinks, smoothing; no Source ownership by configuration alone |
| ContentPort | Host | Structural receiving occurrence; requested/visible Connector selection |
| Connector | Host content registry | Source membership, per-connection parser/delivery/render caches; inactive derived execution discarded |
| PreparedProjectionTicket | Prepared frame | Exact port/connector/product identity and geometry selection, not “latest product at this width” |
| Source snapshot | Arc-backed immutable storage version | Can outlive subsequent append/truncate; old pages reclaim only after all owners drop |

TS content wrappers currently import the concrete FFI adapter directly. That actual route is described here; it is not declared equivalent to the handoff's typed ContentDataTransport seam. N-API creates/controls handles and retrieves diagnostics. The C data adapter uses opaque environment/Source identities, ABI/schema handshake, bounded words/bytes and structured mutation results; it is not an unvalidated JS pointer to arbitrary Rust content.

### 7.2 Accepted mutation

1. TS validates/coerces public values, encodes UTF-8 plus typed annotation lanes and splits u64 values into low/high words.
2. Direct FFI validates environment/Source identity, thread/lifetime and buffers. Payload limit is64 MiB; annotation limit is16 Ki entries.
3. Rust validates kind, sealing, UTF-8, annotation/range semantics, expected revision and retention requirements before installing mutation.
4. Persistent candidate state is prepared, or the restricted unique/in-place append fast path is used where its preconditions hold.
5. Source storage/revision/counters are installed. Lock is released before subscriber host fanout.
6. Mutation returns revision/wake information. Accepted bytes remain accepted if later host wake/scheduling fails; failures are recorded and healthy fanout still attempted.
7. TS marks environment pending; later native preparation projects/measures/paints.

Empty append can be a no-op. Logical replacement/clear changes content generation, distinguishing parser lineage from Source lifetime. Absolute retained UTF-8 offsets do not rebase on head truncation. Sealed append/replace/clear/reseal are rejected as applicable, but head truncation has no sealed guard. “Sealed” is not “all physical output already visible.”

### 7.3 Persistent storage and annotations

| Mechanism | Contract / cost |
|---|---|
| Source byte storage | Persistent tree of approximately16 KiB UTF-8 pages, right-path sharing on append |
| Snapshot | Arc storage/root ownership; cheap acquisition does not flatten text |
| Explicit snapshot text | Can materialize/copy retained text; not ordinary per-frame ingestion |
| Head truncation | UTF-8 boundary checked, page Arcs shared, absolute base advances |
| Annotation storage | Persistent interval structure keyed by start/sequence; overlap query preserves insertion ordering |
| Ordered annotation snapshot | Lazy OnceLock materialization; separate cost from byte append |
| Truncation policy | Clip style/tag ranges, drop atomic annotations when required, preserve valid point semantics; retain head-partial-line fact |
| RawDomain | Borrows a single page when possible; lazily assembles multi-page parser text |
| Exact TextRun | Fresh Arc<str> plus source range; **not** zero-copy page-backed run text |

TS style-name validation contains a bounded, source-confirmed regex error; Rust independently validates whitespace/NUL. This is cross-layer inconsistency, not proof that malformed style keys bypass native validation.

Evidence: reports08–10,18,23,29,35,46; R/application/content.rs, R/content/text/, T/api/content/retained.ts, T/transport/content/, N/content_ffi.rs.

## 8. Semantic text, projectors, smoothing and rendering

### 8.1 Native semantic IR

| Family | Values and invariants |
|---|---|
| TextContent | Raw(RawText) or structured Block root; no standalone Inline root |
| Block | Paragraph, heading1–6, quote, list, code, table, rule, raw block, container |
| List | Bullet/ordered start/style/delimiter/tightness; per-item task state |
| Table | Caption, column alignments, header rows, row/cell spans and annotations; overlap/schema/bounds validation |
| Inline | TextRun, soft/hard break, image alt, raw inline |
| Marks | Canonical sorted/deduplicated emphasis/strong/strike/underline/super/sub/small-caps/code/link; at most one link |
| Provenance | Exact, derived source-rooted range, or synthetic; exact length alone is not byte-witness validation |
| Annotations | Caller-namespaced tags/properties; bool/i64/text/text-list semantic values |
| Origin | Generic Markdown/plain/ANSI/diff/custom origin metadata, not product segment kinds |
| Visitors/rewriters | Traverse nested lists/tables/captions/images/literals; persistent no-op identity preservation |

A raw source domain is a parser region; structured boundaries prevent projectors from reading through unrelated structured values. Annotation application runs after parsing. Exact ranges split by bytes; derived ranges can be apportioned proportionally; synthetic content is not treated as a byte witness. The single-inline helper and actual vector-expanding rewrite route must not be conflated.

### 8.2 Projector implementations

| Projector | Actual behavior | Qualification |
|---|---|---|
| Plain | Raw domain to paragraphs/hard breaks/exact runs | No Markdown interpretation |
| Markdown | pulldown parser, CommonMark/GFM options, event stack, source ranges, restart/reference context | Live table stabilization is policy around open input, not a new grammar |
| Text diff | Line-oriented permissive hunk/meta/add/delete/context classification | Not typed DiffHunk validation |
| ANSI | SGR colors/attributes and OSC8 links; handles partial escape sequences | Unsupported cursor/escape controls consumed, not terminal-control passthrough |
| RewriteProjector | Transform semantic values while preserving validated projection envelope | No-op identity sharing; arbitrary rewrites must preserve source/provenance contract |
| Then | Stateful composition with input/output relation validation | Both stages advance; next wake is minimum; active Connector dispatch is not necessarily a Then chain |
| Smooth | Publish stable source spans over caller clock | Atomic spans, not partial semantic-value slicing |

Markdown emits closed root list items independently where useful for stability, holds ambiguous live pipe tables, and can reparse reference-dependent regions. Its required restart context must be available at the retained Source base. Logical Source replacement resets parser lineage. A generic custom caller is not automatically protected by the active Source pipeline's stronger invariants.

### 8.3 Projection and pacing contracts

Native Projection<T> contains source_base, stable_through, source_end, sealed and contiguous source spans carrying zero or more values. Nonempty zero-value spans explicitly represent elision. Validation requires exact ordered coverage, valid boundaries and sealed=>stable=end. Relation/transition validators additionally describe source containment and stable-overlap preservation; they are explicit helpers, not universally invoked on every internal field mutation.

Smooth counts values per stable span, publishes one weighted span immediately at an episode start, advances by bounded credit on absolute caller time and never splits a span. Empty-value elision is free; seal catches up. Default policy is16ms with spring2 and20–800 values/sec. First clock establishment does not rebase an existing deadline.

Active Connector delivery uses a grapheme projection separate from semantic parsing. Source revision/generation/seal change rebuilds retained grapheme indexing. Pure pacing advancement can reuse parser/semantic/prepared products and move only the delivery frontier. This does **not** imply an entire subsequent host frame never acquires a Source snapshot or that every append stage is O(delta). Due selection scans active deadlines; it is not an asserted sublinear timer index.

### 8.4 Native text rendering

TextRenderer lowers semantic blocks into generic Views: paragraphs/headings to text; quotes/lists to hanging layouts; tables to grids; code to label/literal views; rules and raw values to ordinary text/containers. Images use alt text, not a terminal image protocol. Context carries complete role/part/origin/list/task/table/language/format and annotation facts.

TextRenderPolicy owns structural choices such as soft-break handling, gaps, table sizing, task markers, code labels and wrapping. Theme owns semantic style/color resolution. Parsing does not embed terminal colors. TextSelector compiles to ordinary reserved generic text-style selectors; it is not a separate cascade engine.

Direct typed DiffHunk values instead validate offsets, line numbers, range counts and terminations, then lower directly to a Column-based View representation. That explains the native state-kind mapping and is deliberately distinct from permissive text-diff projection.

### 8.5 Product keys and lifetime

| Cache/product | Key dimensions / bound |
|---|---|
| Semantic Connector cache | Source identity/generations/revision/base/end/seal/funnel/hyperlinks; excludes width/theme/delivery; two entries |
| Prepared paint/layout | Semantic key, width, theme and preparation requirements; two entries |
| Projection | Adds wrap/delivery/finalized/physical dimensions; two entries |
| Stable prefix proof | Semantic stable end and width; independently sealed prefix proof, not arbitrary cut of open rows; two entries |
| Renderer block cache | Block identity plus inherited context, retains Block/View; four entries with clear policy |
| Raw renderer cache | Page identity/range, retains page;1,024 entries |
| Edge cache | Child View IDs/gap/predecessor shape;2,048 entries |
| Semantic sequence cache | Persistent sequence/prefix reuse; four entries |

Candidate/committed/cache products are matched by exact ticket identity. Missing product or poisoned lookup returns None; direct paint currently returns without marking incomplete. This is a failure-signal concern with reachable-miss proof still open, not a newest-same-width fallback.

Evidence: reports08–10,14,35,46; R/content/text/, R/projection/, R/application/content.rs.

## 9. Scene resolution, layout and geometry

### 9.1 Frame preparation

Scene contains body plus optional History sideband. Body root fill normalization affects the outer root only. Component references expand through retained registry snapshots. Mount reconciliation establishes reachable component graph, parent/scope relationships, focus/modal eligibility, capabilities, output hooks and tick deadlines. Duplicate occurrences of a component ID are rejected rather than cloned into independent instances.

Scene preparation can converge over up to8 passes when callbacks/content/state observations invalidate earlier work. Caches advance generation once per host frame, not once per convergence attempt. A successful preparation is not yet a backend receipt. Kernel component retirement is reaped after successful graph reconciliation; its relationship to old physical-frame lifetime remains explicit in ISSUES.

### 9.2 Geometry pipeline

| Stage | Inputs | Outputs / invalidation |
|---|---|---|
| Measure | View, width constraint, sizing intent, component/state/content revisions | Intrinsic MeasuredNode with retained semantic ownership |
| Prepare | Measured result and height bound | Allocated constraints/tracks; separate cache key |
| Place | Prepared tree, absolute origin, clip, scope | Flat LayoutTree with rect/content rect/clip and parent/child indices |
| Resolve interaction/layout callbacks | Mounts, visible geometry and revisions | May require convergence/reprepare |
| Paint | Flat tree + theme/style context + content tickets | Surface/rows/incremental target |

Row measures intrinsic allocation then remeasures children at allocated widths. Column measures at available width then allocates height tracks. Grid determines column span requirements, allocates columns, remeasures cells, then solves intrinsic/spanned rows and final row heights. Tracks include content, fixed, flex and capped forms; normalization validates scalar bounds. Grid gap state applies across both axes.

Decorations and retained-state overrides compose before inner size constraints. Clamp and RowViewport have distinct measure/visible-window roles. Layout uses u16 geometry; source offsets use much wider absolute coordinates and must not be confused with visible row counts.

### 9.3 Retained indexes and local repairs

- Measure keys include View/component identity, geometry/presentation/content layout revisions, width and intent. Prepare adds height bound.
- Current/previous generation caches are temporal working-set caches, not fixed-cardinality bounds.
- Parent, component, state and content dependency indexes find affected paths. Ancestors do not implicitly encode every descendant revision, so explicit invalidation matters.
- Component subtree repair requires stable external geometry/topology; unsafe repair conditions fall back to full preparation.
- Structural work combined with state/component invalidation conservatively clears or evicts both layout and paint caches.
- Ordered row-window pruning requires both child tops and bottoms monotonic; otherwise paint scans in original z order.
- Some generic subtree metadata/cross-scope cache questions remain unresolved; do not turn conservative fallback into a blanket proof of all shapes.

### 9.4 Resize behavior

Headless resize updates sink dimensions and invalidates. Real host explicit resize validates arguments and invalidates but does not force the terminal worker to adopt those dimensions; real backend events determine actual viewport. TS cached requested metadata can therefore differ from physical size. Resident semantic content reflows using width-keyed caches; previously accepted native scrollback and frozen transfer remainder are not regenerated.

Evidence: reports03–06,30–32,36–37; R/scene/, R/presentation/layout/ and R/presentation/paint/, R/geometry/.

## 10. Physical cells, painting and terminal backend

### 10.1 Physical representation

A PhysicalCell distinguishes transparent from painted blank, optional grapheme leader text, continuation and style. One wide grapheme occupies a leader plus same-style painted continuation cells. Width is extended-grapheme based, using the Termwiz column-width policy consistently with layout/cursor lowering. Zero-width and too-wide behavior is explicit; graphemes are not split merely to fit a clip.

Surface owns a u16-sized cell grid and physically_complete marker. Completeness means represented glyph geometry fits, not that every cell is opaque. Immutable PhysicalRows support exact native History transfer. Debug geometry validation is useful but not a universal release-mode proof.

### 10.2 Painting routes

| Route | Work / semantics |
|---|---|
| Full | Resolve inherited/named/local style, paint node/children/background/border, composite to surface |
| Row window | Build requested rows, prune vertically where ordered, preserve full semantic geometry |
| Direct ContentHost window | Use exact prepared ticket and translated clips without content-height-sized surface |
| Ordinary full RowViewport | Paint child then copy selected cells; different from glyph-aware row route |
| Incremental | Reuse last surface, reconstruct ancestor style, clear old target rectangle and repaint/composite; fallback full when target mapping invalid |
| Paint cache | View/geometry/clip/styles/context/content revisions/decor fingerprint; two generations and explicit invalidation |

Whole-glyph composition clears overwritten covering glyphs. Independent cell-wise rectangle clear and ordinary full RowViewport copying have route-specific safety questions; no universal corruption claim is justified. Damage merges touching rectangles and promotes to full above64 regions or half viewport.

### 10.3 Backend ownership and receipt

TermwizBackend is the host-side sender/worker/input-reader owner. A worker owns the real terminal and retained presenter Surface. Crossterm handles input/raw-mode/paste while Termwiz handles output/capabilities; these are complementary responsibilities, not selectable renderer fallback backends.

Startup probes terminal capabilities/size, enables raw input and bracketed paste, hides cursor, creates visible space using CRLF and paints an empty surface before starting input. No alternate screen is used. Successful startup already makes the presenter known; the first application frame need not start from an unknown screen.

Every submitted frame is fully lowered to a new Termwiz Surface, including History overlay. The current backend ignores retained DamageRegion metadata and computes whole-surface desired/diff work. Known same-size state uses diff_screens; unknown/changed-size state uses canonical reset/full paint. Worker render+flush must succeed before desired presenter state is installed and receipt sent. Host has at most one outstanding frame receipt.

### 10.4 Native rows and failure

Native History inserts clone exact rows to the worker, emit rows and CRLF overflow at the bottom, then update the retained screen model after successful write. The real output does not use ScrollRegionUp; model-only scroll operations maintain the shadow. Unix synchronized-output sequences can span native insertion and the next frame, and are closed on completion/error/resize/final positioning.

Sink Err logically accepts zero rows, but terminal bytes can already be partially written. Presenter/history synchronization becomes unknown. Full visible repaint can reestablish logical screen knowledge; it cannot reconstruct an uncertain external scrollback tape. Resize leaves native scrollback to terminal behavior.

Restore stops/joins input, restores worker state and joins worker, continuing cleanup while returning the first error. The restored flag is set before completion, so idempotence is not proof of successful retry. A startup fallible size conversion outside the main restoration path remains a cleanup-path concern, not a demonstrated broken terminal.

Evidence: reports14–15,34,36–37; R/physical/, R/presentation/paint/, R/backend/, R/terminal/.

## 11. History: semantic units, viewport projection and irreversible transfer

### 11.1 Three distinct histories

| Layer | Stored value | Mutability boundary |
|---|---|---|
| Semantic queue | Ordered static/live units, IDs, Views, layout policy and revisions | Live component units can finalize; resident semantic content still responds to theme/width |
| Projection | Width/height keys, FollowEnd or NativeFrontier plans, frozen overlay | Recomputed for viewport and dependencies |
| Native frontier | Exact painted rows, prefix acknowledgment, frozen remainder | Accepted rows are irreversible terminal history, not semantic Views to repaint |

TS History is a scene sideband, not an ordinary child View. A push containing components is Live; a component-free ContentPort unit can be Static while its Source is still open. Semantic freeze changes Live to component-free Static; it is not physical freezing and has no general inverse. Discard applies only to Live tail. Static units have no arbitrary public replace/remove route.

Detached push/layout is allowed, but native freeze/discard requires attachment. History ownership is taken for a host; validation is not general rollback of later rendering failure. Body/History attachment discovery also participates in state/content ownership.

### 11.2 Layout and cache

Body is measured/clamped first; History receives remaining space above it. Live History components remain mounted even at zero visible height. Static height keys use View identity, direct ContentHost keys include projection identity, and live keys include reachable component dependencies. Nested component-free content/state coverage remains an explicit cache-coherence question.

FollowEnd selects resident suffix. NativeFrontier preserves blocked-prefix ordering. Large flexible live content uses bounded prefix semantics rather than pretending arbitrary tail offset. Once physical rows exist, a retained geometry shortcut stays disabled via monotonic history state.

### 11.3 Drain protocol

1. Determine overflow pressure, preceding gap/padding/frozen rows and front unit.
2. Never skip a blocking live unit to transfer later content.
3. Static content transfers exact painted rows. ContentHost may offer a stable finalized prefix even while Source remains open.
4. Full retirement requires complete proof (including sealed/caught-up delivery where applicable); zero incomplete rows block, zero complete rows may retire.
5. Sink acknowledges exact prefix count. Partial success retains unaccepted rows in their original width/style.
6. Retire callbacks account accepted progress even if a later recursive drain fails.
7. Invalid acknowledgment/failure marks native synchronization unknown; do not regenerate/replay uncertain rows in the same attempt.
8. A later successful host frame clears the logical synchronization marker. It does not undo or infer unknown physical scrollback bytes.

Theme/layout/resize changes affect still-semantic resident units, not accepted physical rows or frozen partial remainder. TS History layout is a narrower scalar padding/gap facade than all internal Rust Insets/FlowBoundary capabilities. This is documented current surface, not implicit parity.

Headless nativeHistoryRows retains rows for inspection and can grow; real terminal history has no emulator readback through that diagnostic. IYON_HISTORY_TRACE=1 enables static trace instrumentation, not a correctness oracle.

Evidence: reports07,15,30,35–37; R/history/, R/scene/host.rs, R/application/host.rs.

## 12. Input, components, outputs and retained controls

### 12.1 Native routing

Terminal events decode key/paste/resize. Release events are dropped, repeats accepted; BackTab becomes shifted Tab. Focus/mouse are ignored by current driver. Injected textual key parsing is narrower than real terminal decoding.

Routing order follows the native component/mount/focus/modal machinery, capabilities and key/paste commands. Current eligible focus is preserved through reconciliation; first-eligible selection applies when replacement is necessary. There is no TS keystroke interpreter merely reproducing native controls. Native callbacks can update component state, emit typed output and enqueue generic app actions.

Output<T> is a typed channel identity; erased queued payloads are checked by corresponding router. Route lookup occurs at drain, so a pre-registration queued event can still route if registered before drain. Already-drained unrouted output is lost. Queues are not generically backpressured, and native generic payload/callback types need not be Send merely because their wrapper holds a mutex.

### 12.2 Control table

| Control | Native mechanics | TS ownership / caveat |
|---|---|---|
| TextInput | UTF-8/grapheme cursor/edit commands, wrapping, paste, submit/change output, focus cursor style | Host-owned wrapper/output projectors; no per-key JS editing; successful cursor moves also currently emit change |
| ScrollPane | Retained content, u16 extent/offset, native key scrolling and layout notification | Factory-created; builder/direct mode uses shared runtime; freshness comes from layout callbacks |
| ViewSlot | Stable component identity, retained View, frames, revision and tick deadline | Factory-created; direct/builder takeover plus root boundary; animation/stop bypass ordinary TS currentView/attachment commit path |
| History | Semantic units, live mount participation and native prefix transfer | Strong host-retaining wrapper, unlike weak-host state/control wrappers |

Slot frame arrays are caller-supplied semantic Views; Rust chooses frames on native ticks. Public animation does not send every tick over N-API. Generic slot capability can register an outer16ms tick even for static/one-frame state; actual idle cost remains measurement work. Cycle-boundary update and stop are specialized paths, not ordinary scope rerendering.

Mount handles are nonowning IDs; registry owns component boxes. Poisoned mounted controls can degrade to spacer/ignored interaction while direct setter operations report error; do not describe this as universally surfaced failure.

Evidence: reports01–02,12–13,18,21,28,34,38–39; R/component/, R/interaction/, R/output/, R/controls/, T/api/controls/.

## 13. Themes and style-state propagation

| Stage | Contract |
|---|---|
| StyleSpec | Sparse fg/bg and explicit optional attribute booleans; plain clears attributes, not colors |
| StyleRef | Optional named theme identity plus local patch |
| Selector | Positive focus/focus-within and caller key/value predicates; canonical replacement by key |
| Theme entry | Base plus variants sorted by specificity/declaration order; exact-selector replacement gets new order |
| Native cascade | Inherited physical style -> framework named rule -> application named rule -> local patch |
| Color reference | Application palette resolution then framework; explicit terminal Default counts as a resolved value |
| Text facts | Self-only role/part/origin/etc facts; descendants retain caller states/inherited style but clear self facts |

Missing style is no-op; missing color resolves physical default. Specificity compares within a layer, not across framework/application precedence. Theme::color resolves in default context, including unconditional variants; Theme::style is base lookup. Text selectors use the same style machinery.

TS sends theme DTO over N-API; native decodes/batches then replaces active Arc theme and invalidates paint/content products. Semantic parser IR and ordinary geometry remain reusable when theme is metric-neutral. Prepared content retains captured theme through receipt. TS StyleRef cache reset occurs in finally after native invocation; existing semantic refs still encode named policy, not baked terminal colors.

Atom interning is a bounded2,048-entry best-effort table; eviction does not invalidate live Arc strings. Resolver creation clones application theme and constructs framework defaults; no universal compiled-theme cache is claimed. History semantic freeze remains recolorable until physical acceptance; native rows do not retroactively change palette.

Evidence: reports11,19,22,33; R/theme/, R/presentation/paint/theme.rs, R/content/text/style.rs.

## 14. Scheduling, runtime barriers, errors and disposal

### 14.1 Independent schedulers

| Authority | Work owned | Fairness/barrier |
|---|---|---|
| RetainedExecutionRuntime | JS dirty scopes/dependencies/publications | Deduplicated queue/microtask generation; explicit flush cancels obsolete scheduling |
| JS realm wake broker | Native environment pending/wake/error coordination across Tui instances | Edge latch/pending generation, timer-based presentation wait; no busy microtask receipt spin |
| Native environment | Host pending/committed epochs, content fanout and drains | Bounded drain32; failed automatic epoch blocked until newer work/explicit retry |
| Native host | Input/actions/timers/component ticks/content deadlines/frame preparation | Action budget128; missed component intervals produce one callback, not catch-up storm |
| Terminal worker | Actual writes/diff/receipts | One host frame in flight; command order and exact receipt |
| Headless test clock | Explicit deterministic advance/barrier | Not identical to wall-clock native wait |

Automatic State write changes the JS value before scope reevaluation. Failed preparation restores committed dependencies/output and dirty obligations but does not roll back the source State value. Equal write is no new invalidation; failed work is not automatically retried forever.

Canonical Tui render drains TS work, validates producer/direct ownership, accepts desired roots and drives the environment barrier. Force-retry flush is bounded64 error-free incomplete iterations but throws reported pending errors immediately. Native waitForOutput drives local poll/advance/flush, **not** environment-wide draining. TS broker supplies that broader scheduling. Abort-aware nextEvent uses TS polling/waits capped around16ms; native no-signal wait has its own path. Input pumping is bounded32 and can stop at routed action.

### 14.2 Failure taxonomy

- Public validation errors, native ABI/decode errors, stale/foreign/disposed identity errors, preparation failures and terminal receipt failures are separate domains.
- Async source acceptance followed by wake failure is not a rejected append.
- Single onError listener can consume errors; otherwise reportError/console path reports them.
- Some native diagnostic codes normalize to INTERNAL_INVARIANT at the TS surface while preserving message/context.
- A thrown callback/render does not roll back previously consumed input/actions/output, accepted source data or terminal bytes.
- Poisoning, exact-ticket misses and cleanup partial progress have narrower policies recorded in ISSUES rather than invented universal exception handling.

### 14.3 Teardown

close marks TS liveness closed, unregisters broker, disposes content/bindings/state/resources/owned handles, releases builder/execution/boundary state and disposes native host, aggregating cleanup failures. Native close waits pending receipt, clears roots/components/content, restores terminal and unregisters; it does not necessarily present a final application frame.

exit presents final content/positions cursor before resource teardown. Wrapper alive flag and inner closed state are distinct. Drop behavior depends on remaining Arcs; HostHistory can retain a host after its primary wrapper goes away.

TS resource registry finalization retires bookkeeping and is not a blanket native.dispose call. Semantic reachability does not veto explicit disposal by itself; actual desired/visible/prepared leases and native checks matter. Cleanup's retry loop compares pass number to a shrinking pending array: algorithmic concern is confirmed, but production pre-cleanup may collapse the dependency chain, so an actual leaked public handle is not asserted.

Evidence: reports01,18,21,23,34,40–41; T/runtime/, T/runtime/{handle-registry,native-resource-registry}.ts, R/application/{environment,host,kernel}.rs.

## 15. Build, generation, tests, benchmarks and observability

### 15.1 Generation and artifact path

tools/tui-abi/view_abi.toml plus view-kind-codes.json and generator sources produce structural C/Rust/TS ABI definitions, validation wrappers, IDs/offsets, manifests and wrapper fixtures. The generator is build-time tooling. Handwritten native structural implementation and content packing/decoding remain separate correctness responsibilities.

Generated schema_hash/generator_hash do not include every separate input: view-kind-codes.json is excluded from those hashes. Freshness checks cover generated output consistency more broadly than runtime fingerprint equality alone. Manifest omits buffer_used_of and carries some enum references rather than expanded values. Content C header/TS encoder/Rust decoder are handwritten despite the handoff generation requirement.

Native staging builds/selects artifact then copies before all load/marker/content/nm qualification completes. A post-copy qualification error can leave an unqualified new artifact. A nonexistent explicit override can fall back to staged addon; an existing invalid selected override fails. Record actual selected artifact path, not only requested environment labels.

### 15.2 Evidence classes

| Check family | What it establishes | What it does not establish |
|---|---|---|
| Rust unit tests | Internal behavior, layout/projection/state/history/backend contracts; substantial real NativeViewRuntime generated-export tests | Fresh success without running; installed package behavior |
| Generated stubs | Signature/framing/linkage agreement | Independent native semantic oracle |
| TS package tests | Public/internal authoring, native integration, counters and headless screens depending on test | Universal absent-addon skip; eager require/identity can fail first |
| Consumer fixture | Workspace root/testing public imports and scoped behavior | Separately packed/installed package qualification |
| Declaration closure / binding gate | Public declaration reachability/nameability and curated Rust boundary checks | Semantic parity or absence of every merged implementation member |
| Ownership scanner | Configured generic/product-policy boundary patterns | Full semantic ownership proof |
| Recording/ShadowTerminal tests | Backend byte/change/screen/scrollback/failure scenarios, independent terminal model where implemented | Real terminal emulator behavior across platforms |
| Retained-versus-fresh oracle | Cross-route consistency within shared implementation | Entirely independent renderer truth |
| Benchmark counters | Instrumented path/stage work | Uninstrumented costs or printed transport identity truth |

All primary scout test discussions are static. This mapping's executed checks are listed in evidence/final-validation.md; no broad test suite or benchmark was rerun.

### 15.3 Benchmark integrity

Current T15 case imports nativeViewAbiSession and RetainedRootBoundary for both labels. T15_CANDIDATE/T15_TRANSPORT affect printed metadata, not actual structural dispatch. Feature-gated C export qualification is not a direct-call benchmark. Historical two-arm artifacts are revision-specific and must remain so.

Some microbenchmarks use nondeferred boundaries unlike canonical Tui desired-install scheduling. Rust fresh/rebuilt-equivalent fixture modes can execute identical arms. Unseeded bootstrap, optional unknown identity fields, literal isolation metadata and observed rather than asserted screens limit inference. No baseline4355 latency/allocation regression is asserted here.

### 15.4 Diagnostic dimensions

Track JS body/props/dirty/flush/abort/semantic reuse; native ABI calls/leases/builders/edits/weak lookup; state geometry/paint/damage; Source pages/bytes/revisions/annotations; parser/projection/delivery/cache work; host pending/committed epochs and failures; layout/paint nodes/cache/cell allocation; native History accepted/remainder rows. Many counters are feature/test gated and some memory fields unknown. Backend bytes/diff/flush latency are not universally instrumented.

Evidence: reports16–18,24–26,40–46; tools/, package scripts/tests/bench, Rust cfg(test)/performance features.

## 16. Cross-cutting cache and invalidation inventory

| Owner | Reuse trigger | Invalidation / release | Bound qualification |
|---|---|---|---|
| Scope semantic slots | Same normalized operation at same scope ordinal | Changed op, unused tail, scope disposal | Live scope call count |
| Persistent child sequences | Shared branches on localized edit | Old roots dropped | Sharing, not global eviction |
| JS weak native hints | Same semantic object/runtime generation | Weak GC/generation mismatch/native miss | Nonowning |
| Native view table | NodeId/Ref live semantic Weak or lease | Release + weak expiry + maintenance/lookup/sweep | Paged high water; no idle collection guarantee |
| Native styles/paths | Interned semantic identity | Runtime generation teardown | No ordinary entry eviction |
| State logical versions | Same effective values | Patch revision/candidate receipt/disposal | Old Arc candidates pin prior version |
| Measure/prepare/paint | Keys described above | Dependency path invalidation/full clear/two generations | Temporal, not cardinality |
| Text geometry | Same prepared layout identity/width | Prepared-tree owner lifetime | Style-independent |
| Connector caches | Exact semantic/prepared/projection keys | Source/theme/width/delivery/selection; deactivate | Mostly two-entry products |
| Markdown parser state | Same lineage/stable prefix/references | Restart/full reparse/deactivate | Not all internal vectors fixed-size |
| Theme atoms | Same string | FIFO eviction while live Arc survives |2,048 intern table entries |
| History heights | Unit/width/content/live dependency key | History revision/width/dependency refresh | Nested dependencies remain a proof question |
| Frozen History remainder | Partial prefix acknowledgment | Remaining rows accepted/discarded via lifecycle | Exact old-width/style rows |
| Backend Surface | Known prior screen | Successful desired install; unknown on failure/resize | Viewport-sized, not damage-only |
| Headless rows/output queues | Diagnostic/queued history | Explicit lifecycle/drain | Can grow; no blanket backpressure |

## 17. Reading map and completion boundary

Use the [atlas index](README.md) for all45 primary slices/traces/audits and [follow-up46](audits/46-grouped-deviation-followup.md). Their local inventories and exact source ranges are part of this deliverable. For each subsystem, this synthesis supplies integrated ownership, route and failure semantics; local reports supply exhaustive symbol/test navigation. Apply [RECONCILIATION](RECONCILIATION.md) before reusing an original claim.

Recommended reading orders:
- Architecture author: maintained guide -> sections1–3 -> relevant behavior trace -> local implementation report.
- Performance investigator: sections5,8–10,14–16 -> actual production route41 -> benchmark audit42; verify dispatch before counters.
- Lifecycle investigator: sections2–3,7,11–14 -> trace40 -> ISSUES B/C and follow-up46 parent qualifications.
- Future census/V5 work: manifest + assignments + report index + this inventory + findings register. This map does not classify every symbol for removal/move/keep.

Completion means all46 reports fully read, substantive conflicts reconciled, source snapshot/provenance retained and two distinct documentation views delivered. It does not mean all open safety/performance/design questions have been repaired or proven. No production source, dependency, test or AGENTS change was made.
