# PERF-13-F implementation notes

> This is an implementation handoff for the next context window, not a completion
> report. It records the current repository shape, the resolved PERF-13-F
> contract, and the work still required after PERF-13-A through E.

## 1. Current task and boundaries

The active user request is to re-read and implement **PERF-13-F — Connector
projection, unified convergence, and viewport integration** end to end.
PERF-13-A through E are complete and committed. PERF-13-G and PERF-13-H are
not part of this task.

PERF-13-F must make retained content visible through the existing host frame
transaction. It must not add a second renderer, second scheduler, or a new
high-volume payload path.

### Explicitly in scope for F

- plain-text Funnel and width-dependent projection;
- projection cache keys and cache lifetime;
- immutable Source snapshot acquisition during frame preparation;
- requested Connector activation inside the convergence loop;
- projection before/during measurement, followed by placement feedback;
- old-Connector/empty fallback when candidate projection fails operationally;
- atomic projection, subscription, Connector status, and frame commit;
- ContentPort allocation and clip binding;
- read-only viewport context;
- ScrollPane/RowViewport extent, clamp, and follow integration;
- scroll-only visible-window reuse;
- post-commit Connector status/error observation;
- focused plain-text visual parity and lifecycle tests/probes.

### Explicitly out of scope

- Markdown migration, complete annotation schema migration, History migration,
  and Iyon consumer migration (PERF-13-G);
- deletion of old stream/payload paths and final performance acceptance (H);
- buffered/hot inactive Connectors, arbitration, priority, preemption,
  background producer threads, or automatic activation policy;
- arbitrary JavaScript Funnel callbacks;
- a full PERF-12 benchmark suite. PERF-12 is already complete. PERF-13
  tranches may run relevant tests and focused probes/benchmarks only; the full
  PERF-12 benchmark suite takes roughly four hours and must not be run.

## 2. Authoritative handoff requirements

The complete normative source is:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`

Important authoritative sections:

- §1: three-plane and cold-Connector invariants;
- §§2–5: ownership, handles, epochs, wake broker, and error behavior;
- §6: candidate frame transaction and convergence;
- §§9–11: Source/Funnel/Port/Connector entities and switching;
- §14: ContentPort/viewport/ScrollPane/History ownership;
- §§17–18: text storage, projection, and measurement contracts;
- §§20–21: counters and correctness matrix;
- §22: tranche F required work and stop gates;
- §23: focused acceptance workloads;
- §24: final definition of done.

The handoff says that activation is not a post-layout phase. For one host frame,
the effective order is:

```text
capture pending epoch
→ latest desired structure
→ candidate attachment reconciliation
→ pending state/control values
→ candidate Source snapshots
→ projection at candidate width/context
→ intrinsic measurement
→ placement
→ viewport extent/window/clamp/follow resolution
→ repeat only when a projection/measurement/placement key changes
→ damage
→ candidate paint
→ backend presentation
→ one logical commit
```

A candidate Connector must never become visible merely because `activate()` was
called. The old visible Connector remains the fallback until the candidate has
successfully projected, measured, placed, and painted in the same candidate
frame.

The projection key must include, at minimum:

```text
Source identity
Source content generation
Source revision
Funnel identity/configuration fingerprint
candidate offered width / constraint key
host theme/style-resolution generation
relevant read-only viewport/materialization key
annotation/style semantic generation when separate
```

The implementation may split width-dependent/intrinsic projection from
viewport-visible materialization. This split is preferred: scrolling should not
reparse or rewrap the entire Source.

Intrinsic content extent must not depend on scroll offset. A viewport context is
read-only to projection and contains the equivalent of:

```text
offered_width
allocated_height
clip_rect
viewport_start
viewport_length
follow_end
theme_generation
```

Scroll offset, follow-end state, anchors, and user scroll intent belong to the
viewport controller (`ScrollPane`/`RowViewport`), not to ContentPort or
Connector. ContentPort owns only structural mount, allocation, and clipping.

## 3. Repository state at note creation

- Branch: `perf-13`.
- PERF-13-E commits:
  - `89a4c76 feat: implement PERF-13-E retained content data`
  - `e1c2b7a docs: clarify PERF-13 benchmark policy`
- The PERF-13-F implementation is in progress. The initial worktree slice now
  contains a generic presentation `ContentProvider`, plain Connector projection
  and cache entries, ContentHost layout/paint integration, content-aware cache
  invalidation, and ScrollPane content-extent synchronization. Continue auditing
  and hardening before calling F complete.
- The repository was clean before this notes file was created.
- The current Pi process is `openai-codex/gpt-5.6-luna`.
- `~/.pi/agent/models.json` was updated during this session so the
  `openai-codex` override for `gpt-5.6-luna` has `contextWindow: 1000000`.
  That is a Pi configuration change outside this repository.

Existing verification for A–E passed before starting this audit, including
focused Rust/Bun tests, native tests/clippy, TypeScript typecheck, package
build, ownership, declaration closure, ABI checks, direct-FFI probes, default
and direct-FFI staging, and C-header layout assertions. Do not assume F passes
those checks until rerun after implementation.

## 4. Existing Rust architecture and exact F seams

### 4.1 `crates/iyon-tui/src/application/content.rs`

This is currently the native Source/Port/Connector identity and lifecycle
module. Its module comment explicitly says projection remains a later tranche.

Existing Source pieces:

- `ContentSourceRegistry` is environment-owned;
- `HostContentSource` owns an `Arc<SourceStorage>` record;
- `SourceStorage` uses immutable `Arc<[u8]>` chunks, `VecDeque` chunks and line
  starts, absolute UTF-8 byte coordinates, semantic annotation records,
  retention, sealing, and head truncation;
- `HostContentSourceSnapshot` is immutable and cheaply cloneable because its
  storage is an `Arc`;
- `append_utf8`, `replace_utf8`, `clear`, `seal`, and `truncate_head` mutate
  atomically under the Source lock and return `ContentMutationResult`;
- Source mutation copies weak subscriber tokens, releases the Source lock, then
  marks all eligible hosts through the environment wake path;
- Source snapshots have `source_id`, `source_generation`,
  `content_generation`, `revision`, `source_base`, `source_end`, `sealed`,
  `head_partial`, and shared storage;
- `snapshot().text()` and `snapshot().annotations()` are diagnostic/materialized
  queries and must not be used by the F hot path;
- stats include accepted, copied, and dropped bytes.

Existing Funnel pieces:

```rust
pub enum TextFunnelKind { Plain }
pub enum TextWrapMode { Word, Grapheme, NoWrap }
pub struct HostContentFunnel {
    pub family: ContentFamily,
    pub kind: TextFunnelKind,
    pub wrap: TextWrapMode,
}
```

The Funnel is immutable, Source-neutral, host-neutral, and currently only
represents plain text plus wrap mode. It has no projection cache or viewport.

Existing Port/Connector pieces:

- `ContentHostRegistry` is host-owned for Ports and Connectors;
- a `PortRecord` tracks desired/visible mounted state and desired/visible
  Connector identity;
- a `ConnectorRecord` tracks Source/Funnel membership, requested/visible,
  subscription, lifecycle, phase, error, failed Source revision, and a native
  test-only activation failure hook;
- `ContentBinding { port_id, connector_id }` is the candidate/visible selection
  record;
- cold Connector membership counts as Source use, but cold Connectors do not
  subscribe or project;
- `candidate_bindings()` currently selects the requested candidate only through
  a synthetic `activation_failure` hook. A ready candidate is represented by
  its identity, but no real projection is performed;
- `commit_visible()` toggles visible Connector and Source subscription state;
- `connector_status()` currently returns
  `projected_source_revision: None` for every Connector;
- failed candidates keep the old visible Connector and retry on a later Source
  revision, remount, or explicit activation.

Important lock/lifetime rules to preserve:

- never hold the Source mutex while taking a host frame lock;
- snapshot/clone immutable Source storage briefly, release Source lock, then
  project/layout;
- do not let a cold Connector retain derived rows or an inactive delivery queue;
- candidate Connector subscriptions/leases become authoritative only at frame
  commit;
- old visible subscription/projection is released in the same logical commit
  that replaces it;
- keep `in_flight_connectors` alive across backend presentation receipts;
- no Source mutation calls callbacks, flushes, layout, or projection.

Likely F additions here:

- a generic/plain-text projection value and a cache entry owned by the active
  Connector, or a host-owned projection cache keyed by Connector;
- projected Source revision/content generation in status;
- candidate projection/measurement metadata separate from committed projection;
- methods to prepare a candidate without mutating visible Connector state;
- methods to commit/abort projection and viewport-visible state atomically;
- retry/error transitions for real projection failures distinct from validation
  errors;
- a Source snapshot byte/chunk iterator or equivalent internal view so F does
  not call `snapshot.text()` and copy the complete Source merely to inspect it.

Do not make this module own scroll state. It may carry a read-only viewport
request/result during candidate preparation, but state ownership remains with
the viewport controller.

### 4.2 `crates/iyon-tui/src/application/host.rs`

`HostInner` owns the frame transaction and currently has these relevant fields:

```text
frame                         last complete logical PreparedSceneFrame
candidate_frame               prepared but not yet visible frame
presentation                  terminal receipt
frame_pending
candidate_epoch
candidate_structural_revision
candidate_content_bindings
failed_attempt
pending_epoch / committed_epoch
view_states
content: ContentHostRegistry
```

Current flow:

1. `flush_pending_frame()` advances the application kernel and detects dirty
   work.
2. `render()` captures `pending_epoch` and desired structural revision.
3. `render()` asks `candidate_content_bindings()` for the current content
   selection.
4. It snapshots ViewState records and calls `prepare_frame(...)`.
5. `prepare_frame(...)` currently delegates to
   `RunningApp::prepare_frame_with_states(...)` with no content projection
   input.
6. A candidate Scene frame is stored and presented.
7. `commit_frame()` commits state bindings and calls
   `content.commit_visible(...)`, but only identities/status/subscriptions
   exist; no content rows are in the Scene or Surface.
8. The control-only fallback in `flush_pending_frame()` can commit content
   identities without manufacturing a surface frame. F must replace that logic
   for content changes with a real candidate frame whenever visible content or
   extent can change.

The current `HostInner::render()` is the primary integration point. Preserve:

- desired vs visible state separation;
- failure leaving `self.frame` untouched;
- backend receipts and candidate lifetime;
- candidate epoch capture; newer mutations remain pending;
- `HostAttemptError` classification and environment retry blocking;
- `content.begin_candidate`, `end_candidate`, and `abort_candidate` lifecycle.

F should make `candidate_content_bindings` carry or identify the candidate
projection transaction, not just the selected Connector ID. The host must not
commit a Connector ID whose candidate projection was not successfully painted.

The old `HostTextStream`/`HostTextPipeline` in this file is the existing
History/stream path. It uses `GenericTextStream`, `Projection`, Markdown,
`TextRenderer`, and eager `render_host()` calls. It is not the new retained
ContentPort path. F may reuse the underlying generic stream algorithms, but
must not route new Source mutations through this old host-owned scheduler or
make it a second authoritative content store.

### 4.3 `crates/iyon-tui/src/application/kernel.rs`

The kernel currently exposes:

- `host_current_state_attachment_targets()`;
- `host_current_content_attachment_targets()`;
- history variants of both target collectors;
- `host_set_body`, `host_set_history`, and retained Scene invalidation helpers.

Content target collection resolves component overlays and recursively finds
ContentPort identities before H3 publication. This is structural attachment
validation only. It does not supply ContentHost projection data to layout.

F will likely need a generic content layout/render input passed from the host
frame into the Scene/layout layer without making composition or structural
transport depend on content control transport.

### 4.4 `crates/iyon-tui/src/scene/host.rs`

`SceneHost` owns retained semantic resolution, layout, paint, component sync,
focus, and incremental retained state behavior.

Relevant current types:

```rust
struct StableScene {
    root: ResolvedRootScene,
    layout: ResolvedSceneLayout,
    history_identity: u64,
    history_revision: u64,
    native_history_revision: u64,
}

pub(crate) struct PreparedSceneFrame {
    pub(crate) surface: Surface,
    pub(crate) history_overlay: Option<HistoryPhysicalOverlay>,
    pub(crate) damage: DamageRegion,
    pub(crate) state_bindings: Vec<(u64, StateNodeKind)>,
}
```

`render_at_with_states()` currently loops over History pressure, resolves a
stable Scene, and calls `paint()`. `resolve_stable_at_with_anchor()` handles
component/state/history convergence, but there is no content projection input.

The existing incremental retained-state paths must remain intact. Content
changes should not execute composition scopes or rebuild the semantic View DAG.
A content-only update should usually reuse the resolved structural Scene and
change only content-derived layout/paint products, unless content intrinsic
size legitimately propagates to ancestors.

Likely F work in this area:

- add a generic content candidate/input map to frame resolution/layout/paint;
- resolve/prepare active or activation-pending Connectors before final measure;
- preserve old resolved/painted state while preparing a candidate;
- include content projection state in `PreparedSceneFrame` or in an associated
  candidate object;
- commit content projection/viewport records only after backend success;
- ensure content failure falls back to old Connector/old projection, not an
  empty or partially updated visible frame;
- return a structured convergence failure when the defensive pass ceiling is
  exceeded.

Do not make `SceneHost` call `HostContentRegistry` directly if that would create
an ownership/dependency violation. Prefer a generic renderer/layout input
contract supplied by the host/application boundary.

### 4.5 `crates/iyon-tui/src/presentation/ir.rs`

The semantic IR already has `ViewKind::ContentHost` and a `ViewNode` content
attachment ID. `View::native_content_host(port_id)` creates it for native
fixtures. The semantic View carries a backend-neutral attachment identity and
keeps a TypeScript attachment reference on the TS side.

Do not embed Source bytes, Connector state, physical rows, viewport offset, or
host-native styles into the semantic View.

### 4.6 `crates/iyon-tui/src/presentation/layout/tree.rs`

Current layout shapes:

```rust
enum LayoutContent {
    Text { text: TextView, width_rule: WidthRule },
    Spacer { rows: u16 },
    ContentHost,
    Children,
    Clamp { overflow: OverflowIndicator },
    RowViewport { skip_rows: u16 },
}
```

`LayoutNode` currently contains View identity, occurrence box, rects, component,
children/dependencies, style, and `LayoutContent`. It indexes component roots
and retained state roots, but not content-port roots.

F replaces the former zero-content ContentHost placeholder with a generic
content-host layout value that identifies the Port and carries candidate
intrinsic metrics/clip/materialization metadata. Keep this value separate from
semantic child Views.
Possible implementation shape (choose the simplest architecture-compatible
form):

```text
LayoutContent::ContentHost { port_id, ... }
```

or a separate `content_port_id` plus generic content layout data on the node.
The port ID must remain a native preparation value, not a semantic native
pointer. Layout must be able to find the exact ContentHost occurrence for
allocation and clipping.

Preserve existing viewport coordinate behavior: `RowViewport` child geometry
is in unscrolled coordinates and incremental paint applies vertical offset.
If F adds a generic content viewport/materialization offset, use the same
signed/clipped geometry model and do not duplicate viewport ownership.

### 4.7 `crates/iyon-tui/src/presentation/layout/measure.rs`

Current `MeasuredKind::ContentHost` has no fields and `intrinsic_size()` returns
`Size::new(0, 0)`. `measure_node()` receives only View, width, overlay,
component scope, and LayoutCache. It computes decorations, retained state
geometry, then measures semantic content.

F must add a generic content measurement input. The measurement contract is:

- no active Connector: intrinsic content size is zero;
- active Connector: intrinsic size comes from the Connector projection at the
  offered width/constraints;
- fixed/fill host can update paint/viewport without ancestor measurement when
  its allocation cannot change;
- fit host propagates changed projection metrics through parent dependencies;
- projection/measurement must not depend on current scroll offset;
- candidate width changes can invalidate/rebuild width-dependent projection.

The content input should be explicit and immutable for one measure pass. Avoid
calling back into mutable Source/Connector registries from deep layout code.

### 4.8 `crates/iyon-tui/src/presentation/layout/prepare.rs` and `place.rs`

`prepare_node()` translates measured facts into bounded allocation. ContentHost
currently becomes a zero-sized leaf. `place::emit_prepared()` emits the
`LayoutTree` and computes occurrence box, rect, content rect, clip rect, style,
and children.

F must ensure ContentHost receives its allocated border/content rectangle and
its content projection is clipped to that rectangle. ContentHost remains a
leaf in the structural layout tree; its projected rows are derived content,
not semantic child Views.

Do not let the content renderer mutate layout during placement. If projection
width changes the intrinsic size, restart the unified candidate pass with a
new measure/layout result rather than committing partial geometry.

### 4.9 `crates/iyon-tui/src/presentation/paint/view.rs`

`ViewPainter::paint_node()` currently has:

```rust
LayoutContent::ContentHost => {}
```

so ContentHost paints only its background/border, if any. F must add a generic
content render input to the painter or a sibling content paint step. The
content should be painted into the ContentHost content rectangle and clipped
by the effective node/ancestor clip.

Important style issue: plain text projection must respect the effective
ContentHost inherited/host style and theme. Existing
`ViewCompiler::compile_projected_text_with_metadata_and_context()` can compile
projected text with a `StyleContext`; avoid caching physical rows with a style
context that is not in the key.

Reuse `Surface` and its wide-glyph/transparent compositing correctness. Do not
write directly to terminal output from projection or paint.

### 4.10 Existing generic stream foundations

The following modules already contain useful, tested algorithms and should be
reused/adapted rather than replaced with a giant String and repeated full
wrapping:

- `crates/iyon-tui/src/stream/source.rs` — synchronous Source contract;
- `stream/model.rs` — immutable snapshots, resident prefix, revision/base
  transition validation, stable frontiers, semantic slices;
- `stream/snapshot.rs` — snapshot validation/building;
- `stream/projected.rs` — width-independent `ProjectedText`, provenance,
  projected atoms, exact/replacement boundaries;
- `stream/compile/mod.rs` — width-specific compilation with theme;
- `stream/compile/text.rs` — wrapping and styled projected text compilation;
- `stream/compile/rows.rs` — physical rows plus semantic anchors and transfer
  metadata;
- `stream/viewport/*` — row indexes and windows;
- `stream/pane/*` — follow-end/detached viewport anchor behavior;
- `content/text/*` — generic text IR, `TextContent`, `TextRenderer`, and
  semantic text utilities.

The existing stream compiler returns `CompiledStream` rows with anchors and
transfer metadata. It currently starts from a `StreamView`/`ProjectedText`
representation and may allocate derived display strings. It is acceptable for
an active Connector projection to own derived output, but Source snapshot
creation itself must continue sharing immutable Source chunks and must not call
`HostContentSourceSnapshot::text()` for every frame.

A practical F adapter can expose Source snapshot chunks as a plain text
projection input, build/reuse a width-dependent Connector projection, and use
existing `ViewCompiler`/wrapping primitives for physical rows. Keep the raw
Source store authoritative and keep derived rows Connector-local.

## 5. Existing TypeScript API and transport seams

### 5.1 `packages/iyon-tui/src/api/content/retained.ts`

The public API already has:

- `TextStreamSource` with append/replace/clear/seal/truncate/snapshot/stats;
- `TextBlockSource` with replace/clear/truncate/snapshot/stats;
- immutable `TextFunnel.plain({ wrap })`, where wrap is `word`, `grapheme`, or
  `noWrap`;
- host-owned generic `ContentPort<TContent>`;
- `port.connect(source, funnel)` as the canonical API;
- `ContentConnector.activate()`, `.deactivate()`, `.dispose()`, `.status()`;
- status phases already anticipating F: `blocked-geometry` and
  `unsupported-backend` are in the TypeScript union;
- `ContentConnectorStatus.projectedSourceRevision?` already exists and the F
  implementation populates it only for the committed visible projection;
- `View.content(port)` is the only structural ContentHost attachment.

The TypeScript API currently has no explicit projection callback, viewport
context, or content row API. F should normally remain an internal/native
implementation change plus status/readback updates. Do not expose a callback
Funnel or physical terminal rows publicly.

`ContentPort` owns a JS connector Set for wrapper lifetime only. It does not
own Source data. `ContentConnector` keeps Source/Port wrapper references,
tracks disposing separately from final native disposal, and finalizes its JS
handle only after native status says disposed.

### 5.2 `transport/content/control.ts`

This module owns N-API control calls for Source/Port/Connector identity and
activation. It explicitly does not implement projection or payload data. Keep
that boundary. If F needs a native query or status field, add it to the control
contract only when it is a small lifecycle/query operation—not a high-volume
row payload path.

### 5.3 `transport/content/ffi.ts`

This is the only Bun direct-FFI implementation. It owns Source byte encoding,
annotation sidecar encoding, ABI metadata validation, one environment-lifetime
`dlopen`, and status mapping. F must not add frame/projection calls to this
module and must not make Source mutation call layout or flush.

### 5.4 `transport/native/addon.ts`

The current native contracts include:

```ts
NativeTextSourceContract
NativeContentPortContract
NativeContentConnectorContract
```

Connector control has activate/deactivate/dispose/status. Status can gain
projected revision/phase fields without exposing physical rows.

### 5.5 `api/controls/scroll-pane.ts` and Rust `scroll.rs`

The existing public `ScrollPane` is a focusable component with content View,
follow-end/detached state, layout size, keyboard routing, and a `RowViewport`
View projection. It owns visual-row scroll state, not ContentPort.

The Rust `ScrollPane` behavior is already tested for:

- following the end on initial content;
- detaching on scroll-up;
- preserving detached position when content grows;
- resuming follow-end at End/followEnd;
- repairing the window after resize;
- not fixing a new allocation to an old viewport height.

F must either integrate ContentHost with this existing controller or establish
a generic read-only viewport adapter. Do not put offset/follow state in
ContentPort/Connector just because ContentHost is a leaf.

## 6. Recommended implementation shape

This is a guide, not a demand for exact names. Keep module ownership generic
and avoid coupling structural transport to content control.

### 6.1 Define a generic content frame input/output boundary

Introduce a small generic presentation-facing value that can be passed into
layout and paint, for example:

```text
ContentLayoutInputs
  map ContentPort identity → candidate ContentLayoutValue

ContentLayoutValue
  port identity
  candidate active Connector identity (or empty)
  intrinsic width/height
  width-dependent projection key/fingerprint
  viewport/materialization descriptor
  derived rows/paint representation owned by the candidate
```

The presentation layer should depend on the value/trait it needs, not on
`ContentHostRegistry`, `HostInner`, or TypeScript transport modules. The
application host can build this map from `ContentHostRegistry` before invoking
Scene preparation.

If a map keyed by `u64` is used internally, keep it private to the native
preparation boundary and validate that each ID belongs to the current host.
Do not write native IDs back into semantic Views.

### 6.2 Make Source snapshot access projection-safe

`HostContentSourceSnapshot.storage` is private to `application/content.rs`.
Add an internal iterator/accessor or an adapter that lets the F projector read
immutable chunks/line indexes without materializing the full text via
`snapshot.text()`.

Potential safe properties:

- chunks are immutable `Arc<[u8]>` and remain valid while the snapshot lives;
- all chunks are valid UTF-8;
- chunk starts and Source base/end are absolute byte coordinates;
- annotation records are immutable in the snapshot;
- no Source mutex is held while consuming the adapter.

A derived projection may allocate rows/compiled text proportional to the active
projection. A Source snapshot must not duplicate the entire raw Source solely
for revision inspection, and a cache hit must not recompile.

### 6.3 Content-folder findings: reuse semantic text infrastructure

The generic content layer under `crates/iyon-tui/src/content/` is important to
F and to the post-PERF-13 v5 direction. It is not just a legacy renderer.

`content/mod.rs` defines immutable semantic content models and the generic
`Renderer` boundary. Its contract is geometry-independent: a Renderer converts
semantic content into a generic `View`; it does not receive terminal geometry,
parser state, clocks, or stream lifecycle.

`content/text/` currently provides:

- `TextContent`, a closed generic text value with `Raw(RawText)` and `Block(Block)`
  variants;
- `RawText`, immutable `Arc<str>` text with exact source-slice support;
- text blocks, paragraphs, headings, lists, quotes, code blocks, tables, breaks,
  inline content, links, marks, origins, provenance, and semantic annotations;
- `TextRun` with `Exact`, `Derived`, or `Synthetic` provenance and UTF-8-aware
  splitting;
- `PlainTextProjector`, which turns contiguous raw domains into paragraph/block
  semantics while preserving source ranges and a stable prefix;
- `MarkdownProjector` and `MarkdownOptions` for incremental/streaming semantic
  transformation;
- `TextRenderer`, which lowers semantic text to generic Views for the existing
  geometry-independent path;
- visitor/rewriter/projector utilities and validation for semantic text IR.

The current `PlainTextProjector` is a useful semantic reference, but it builds
`RawText`/`Block` values and ultimately the existing Renderer lowers to Views.
Do not call it in a way that materializes a complete Source string or rebuilds
semantic Views for every Source append. For F, reuse its newline/provenance/
validation semantics where useful, but keep Source bytes authoritative and put
width-specific derived rows/cache state on the Connector. A future optimized
adapter may expose chunk-backed semantic text without changing these public
semantic concepts.

`stream/projected.rs` and `stream/compile/*` complement `content/text/`:
`ProjectedText` is a width-independent, provenance-aware intermediate; the
compiler turns it into width-specific `PhysicalRow`s with anchors and transfer
metadata. `ViewCompiler::compile_projected_text_with_metadata_and_context()`
can resolve styles against a host/theme context. This is a good foundation for
F's plain Connector projector, provided physical rows are not cached without
including all style/context dependencies.

F should establish a clean direction for later Funnels:

```text
Source accepted bytes/semantic values
    → input decoding/adaptation
    → semantic transformation (plain/Markdown/diff/ANSI/etc.)
    → semantic delivery (immediate or future Smooth)
    → width/host/viewport projection
    → TUI layout and paint
```

Do not model Funnel and Connector as two serial mutable stages. The Funnel is
an immutable normalized specification; the Connector is the execution identity
for one `(Source, Funnel, ContentPort)` binding. A Funnel may describe input
family, semantic transform, wrap/projection settings, capability requirements,
and a fingerprint. It must not own a parser cursor, consumed revision,
smoothing frontier, width cache, or active status.

The v5 design makes the next-step implications explicit:

- `TextContent`/semantic text IR must remain backend-neutral, width-independent,
  revisioned, source-coordinate aware, incrementally replaceable, restylable,
  and free of terminal cells, GPUI glyph data, Taffy geometry, and host-native
  Style IDs;
- Markdown, unified diff, and safe ANSI should become semantic transforms whose
  roles/styles are resolved later by host/theme, not terminal byte passthrough;
- Markdown streaming needs a stable semantic prefix plus replaceable unstable
  tail, with Source seal finalizing the tail; malformed/incomplete input should
  normally remain a deterministic semantic diagnostic/fallback, not a frame
  failure;
- diff should retain semantic additions/removals/context/hunk metadata so it
  can be restyled or selected after ingestion;
- ANSI SGR/OSC 8 may become semantic style/link intent, while unsafe cursor,
  window, or control operations must be stripped/diagnosed;
- Smooth belongs after semantic parsing and before physical projection. It is a
  Connector-local Rust-clock delivery frontier, not React animation, Source
  backpressure, or raw Markdown token pacing;
- the future Connector may own accepted/semantic/visible frontiers, parser
  checkpoints, smoothing state, and width/viewport caches independently for
  each display of one shared Source;
- a future v5 Surface contains ordered component occurrences and owns scrolling,
  follow-end, anchoring, culling, and derived residency. It must not own
  Markdown/diff parsing, Source bytes, or a global smoothing policy;
- v5 replaces irreversible live/completed/frozen presentation lifetimes with
  dependency-keyed cache validity and reversible residency. Completion/seal
  affects ingestion/semantic finalization, never whether an old visible
  occurrence responds to theme, effort, Host Environment, selection, editing,
  or resize;
- v5 eventually uses React + TypeScript as the public composition model,
  Taffy for general Flexbox/Grid on terminal and GPUI, a first-class Rust Host
  Environment, and one component-only ScrollSurface instead of a privileged
  mixed text/component History renderer.

### 6.4 Smoothing interpretation for F

The v5 document confirms that the existing stream smoother should become a
**Funnel delivery-policy value**, but not that the mutable smoother itself
belongs on Funnel. The split is:

```text
Funnel specification:
    immutable delivery policy, e.g. Immediate or Smooth(config)

Connector execution:
    mutable semantic-ready/visible frontiers
    Rust clock/timer and reveal progress
    lag/catch-up state
```

The existing `HostTextPipeline` in `application/host.rs` already uses the
generic `Smooth` projector for the old `HostTextStream`/History path. That is
legacy consumer plumbing, not the retained Source/Connector implementation.
Do not copy its mutable `Smooth` instance into `HostContentFunnel`, Source, or
ContentPort. Do not make F silently migrate the old History pipeline.

PERF-13-F's handoff explicitly requires the plain-text Funnel/projection path,
not the complete Markdown/Smooth migration. F should therefore keep the current
public `TextFunnel.plain({ wrap })` immediate/plain behavior correct while
shaping the native Funnel/Connector boundary so an immutable delivery policy
can be added without changing Source ownership or projection cache keys.
The later v5/V5-F smoothing work should add the policy and Connector-local
clock/frontier state, with this stage order:

```text
Source bytes
    → plain/Markdown/diff/ANSI semantic transformation
    → Smooth delivery frontier (when selected)
    → width/host/viewport projection
    → Taffy/layout/paint
```

A future Smooth policy must not pace raw Markdown delimiters, call JavaScript
per tick, or force React updates. It must suspend when a Connector is cold and
catch up according to policy when demand resumes. F may expose an internal
`Immediate` delivery default or a placeholder delivery-policy field, but must
not invent full smoothing semantics as an unrequested F feature.

Do not implement all of v5 in F. F must make the current plain content path
correct and leave the explicit Source/Funnel/Connector/Port and semantic IR
boundaries usable for the later Markdown, diff, ANSI, Smooth, Surface, and
Taffy migrations.

### 6.5 Add a real plain-text projection

For a `HostContentFunnel::plain(wrap)` and a `HostContentSourceSnapshot`:

1. validate family/kind compatibility;
2. use Source identity, content generation, revision, and wrap mode as inputs;
3. read immutable Source chunks/annotations without holding the Source lock;
4. preserve LF/newline and UTF-8 boundary semantics;
5. build width-dependent rows/metrics using existing compiler/wrap code;
6. retain intrinsic width/height and row anchors/visible materialization data;
7. keep the result Connector-local or in a keyed host cache;
8. drop derived projection when the Connector becomes cold/inactive.

The F plain projector must be deterministic and side-effect free for fixed
inputs. It must not call JavaScript or mutate Source/viewport state.

F does not need to implement the full G annotation migration, but it must not
introduce host-native style IDs into Source data. Existing semantic annotation
records can be preserved in the projection input/metadata and ignored for
plain visual output if the current F schema has no visual interpretation;
future G must be able to resolve them per host/theme without changing Source
storage.

### 6.6 Projection caching

Use a clear key and separate committed/candidate ownership. The key should
contain all output-affecting values, at least:

```text
source_id + source_generation
content_generation
source_revision
funnel kind/wrap/config fingerprint
offered width / constraint fingerprint
theme/style generation
viewport materialization key when output rows depend on it
```

Do not include a universal host epoch if it would invalidate every cache on any
mutation. Do include host/theme/style context when physical style resolution
is done during projection.

Preferred split:

```text
TextProjectionCacheEntry
  width-dependent semantic/row metrics and anchors

VisibleWindow/Materialization
  viewport start/length, clip, follow result, paint selection
```

A scroll-only change should reuse `TextProjectionCacheEntry` and only update
visible-window materialization/paint. Width changes invalidate the width
projection. Source revision/content-generation changes invalidate Source-derived
projection. Theme/style changes invalidate only style-dependent projection or
paint data.

### 6.7 Candidate Connector preparation

Extend `ContentHostRegistry` with an operation conceptually like:

```text
prepare_candidate(port, candidate layout/viewport context)
  → Empty
  → PreparedProjection { connector, source revision, metrics, rows, ... }
  → OperationalFailure { connector, code, diagnostic, retry state }
```

The operation must not mutate the committed visible Connector/projection. It
may clear a retryable candidate error when a new Source revision, width, theme,
viewport input, or remount makes retry eligible.

For a requested switch B while A is visible:

- retain A's committed projection and subscription as fallback;
- snapshot/project B under candidate width/context;
- on success, use B for candidate measurement/paint;
- on operational failure, record B failed/requested/not-visible and use A's
  existing projection/metrics where valid;
- if no A exists, use the defined empty ContentHost projection;
- do not retain a failed candidate's derived rows as cold state;
- do not immediately retry the same failure in a microtask loop.

A Source mutation during preparation may leave the candidate representing the
captured revision. The later Source wake/epoch must make the Connector dirty
for a follow-up frame; never read half of two revisions.

### 6.8 Integrate projection with measure/place convergence

Current layout only sees `ContentHost` as zero. Change the candidate flow so
ContentHost receives the current candidate projection's intrinsic metrics.

A practical bounded loop is:

```text
candidate = current desired structural/state/control candidate
for pass in 0..MAX_CONTENT_CONVERGENCE:
    derive offered widths/constraints from current candidate layout assumptions
    prepare/reuse active/candidate Connector projections at those widths
    measure semantic tree with ContentLayoutInputs
    prepare/place the tree
    derive actual ContentHost allocations/clips and viewport extents
    if all projection keys, metrics, widths, placement, and viewport results are stable:
        stop
    update candidate inputs and continue
on ceiling:
    return structured LAYOUT_DID_NOT_CONVERGE/internal diagnostic
```

Do not put this in a separate content layout loop that commits geometry
independently. The result must feed the same Scene candidate and one final
logical frame commit.

The first pass may use a conservative offered width from parent constraints.
The next pass must reproject if actual allocated width differs. Avoid
unconditional full-tree work after dependency metadata proves that a fixed
ContentHost's intrinsic change cannot escape its allocation.

### 6.9 ContentHost measurement rules

- Empty/no active Connector: content intrinsic `0 × 0`; decoration/bounds still
  contribute.
- Fixed/fill allocation: a revision may change rows/paint/viewport without
  remeasuring ancestors when fixed constraints prevent extent escape.
- Fit allocation: changed row count/width propagates through parent dependency
  metadata and may move siblings/ancestors.
- `allocated_height` limits visible materialization, not intrinsic extent unless
  the viewport contract explicitly says so.
- Scroll offset does not affect intrinsic content extent.
- A candidate switch uses candidate metrics for candidate layout; old committed
  metrics remain visible until the complete candidate commits.

### 6.10 Paint and clip rules

Content rows must be painted inside the ContentHost content rectangle after
border/padding, then intersected with the effective ancestor/node clip. The
ContentHost background/border is still painted by the ordinary box path.

Use the existing Surface compositing APIs. Preserve transparent cells, wide
Unicode glyph continuation handling, and old/new damage. A switch with equal
geometry damages the Port rectangle; a resize/move damages old and new regions.

If physical rows are cached, the cache key must include resolved style context
or the row representation must remain semantic until paint. Do not reuse a row
compiled under one theme/context for another host.

### 6.11 Viewport integration

The connector receives a read-only viewport context. The controller applies:

- extent from the candidate projection;
- clamping when a Connector switch makes the old offset invalid;
- follow-end behavior when the controller is following the end;
- preserved detached/manual offset when not following;
- source/funnel anchor repair only if the existing viewport model supports it.

The active Connector cannot mutate viewport state during projection. A failed
candidate must leave old projection and viewport basis intact.

Use `RowViewport`/`ScrollPane` coordinate conventions: retained child rows may
be in unscrolled coordinates, while paint applies the viewport translation and
clip. Scroll-only updates should avoid width rewrap/reparse and only repaint
visible rows.

### 6.12 Commit/abort

Candidate content state must follow the existing HostInner transaction:

```text
prepare projection/layout/paint
→ backend begin/presentation receipt
→ on success:
     install candidate Surface/Scene
     swap visible Connector/projection
     update projected Source revision/status
     update viewport clamp/follow result
     release old projection/subscription
     advance committed epoch
→ on abort/failure:
     leave old Surface/Scene/Connector/viewport basis untouched
     release candidate projection/subscription/leases
     record operational or frame error
     keep retry work discoverable without spin
```

A Connector status event/observation is published only after commit. The
TypeScript `.status()` query must not report candidate B as visible before the
receipt commits.

## 7. Known traps to check explicitly

1. **Control-only shortcut:** current `flush_pending_frame()` can commit content
   identities without a rendered frame. F must not use that shortcut when a
   content revision/switch/viewport change affects visible output.
2. **Zero ContentHost metrics:** `MeasuredKind::ContentHost` currently always
   returns zero. Any implementation that only changes paint will fail fit
   layout and parent feedback.
3. **No port in LayoutContent:** painter/layout cannot locate a Port projection
   until the layout tree carries a validated ContentHost identity.
4. **Status lies:** `projected_source_revision` currently remains `None`.
5. **Old active fallback:** candidate B failure must not clear A's rows,
   subscription, status, or viewport basis.
6. **Source copy hot path:** do not call snapshot `.text()` or rebuild a giant
   `String` on every frame/cache hit.
7. **Source lock inversion:** projection/layout must run after Source lock
   release.
8. **Retry spin:** failed B must not be retried every microtask at the same
   revision/key. New revision/width/theme/viewport/remount/explicit retry is the
   eligibility boundary.
9. **Cold retention:** inactive/unmounted Connectors must not retain projection
   rows, width caches, or delivery queues.
10. **Wrong owner:** do not put scroll offset/follow state on Port/Connector.
11. **Theme leakage:** semantic Source annotations must not become a host-native
    Style ID; physical style cache keys must include theme/context.
12. **Partial visibility:** do not mutate `HostInner.frame`, committed layout,
    active Connector, or visible viewport before backend success.
13. **New Source revision during frame:** an older captured revision may commit,
    but the newer pending epoch must survive and trigger a later projection.
14. **Structural rebuild:** Source/state/control content changes must not invoke
    composition or rebuild the semantic View DAG merely to carry bytes.
15. **Projection order:** projection must happen before final measurement/placement,
    not after layout as a post-processing step.
16. **History scope:** F should not migrate Markdown/History; existing legacy
    History path may remain while F plain fixtures run.

## 8. Verification plan after implementation

Do not run the full PERF-12 benchmark suite. Run tests, focused probes, and
small targeted measurements only.

### Rust checks

Use `CARGO_BUILD_JOBS=1` for native builds. At minimum:

```bash
cargo fmt --all -- --check
CARGO_BUILD_JOBS=1 cargo check -p iyon-tui-native
CARGO_BUILD_JOBS=1 cargo clippy -p iyon-tui-native --all-targets --all-features -- -D warnings
CARGO_BUILD_JOBS=1 cargo clippy -p iyon-tui --features native-host --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo test -p iyon-tui-native --tests
CARGO_BUILD_JOBS=1 cargo test -p iyon-tui --features native-host application::content::tests --lib
```

Add no new tests unless explicitly requested by the user/project policy; use
existing test surfaces and focused ad hoc probes where possible. If modifying
existing tests is necessary for a regression, keep it narrowly scoped.

### Focused F probes

Exercise through the real staged native artifact where applicable:

- plain text with `word`, `grapheme`, and `noWrap` at stable width;
- initial activate-before-mount: waiting-for-mount, no projection/wake;
- initial mount: candidate projects before first visible frame;
- append updates active Connector without semantic View reconstruction;
- append while inactive: Source advances, no Connector projection; activation
  sees latest retained revision;
- same Source at two hosts/widths: independent projection keys and output;
- width resize/reflow: reproject before final measurement and placement;
- fit ContentHost whose content height changes parent layout;
- fixed/fill ContentHost whose row count changes only visible paint/viewport;
- ScrollPane/RowViewport scroll-only movement: no rewrap/reparse;
- follow-end append and detached/manual scroll append;
- Connector switch preserving viewport intent;
- failed candidate switch leaves old rows/status/subscription/viewport visible;
- candidate failure with no old Connector remains empty;
- Source revision after failure retries exactly once/eligibly, no spin;
- Source mutation during candidate preparation leaves later epoch pending;
- unmount/remount drops/reacquires projection and subscription only at commit;
- candidate/backend receipt failure leaves old frame and old Connector;
- candidate abort releases projection/subscription/binding resources;
- status reports committed projected Source revision only after commit;
- Unicode LF/CRLF/bare CR, combining marks, emoji ZWJ, CJK wide glyphs;
- semantic annotations stay host-independent and do not enter Source as native
  styles.

### TypeScript/package checks

After any TS changes:

```bash
bun run typecheck
bun run check:tui-declarations
bun run check:ownership
bun run check:tui-abi
bun build packages/iyon-tui/src/index.ts --outdir /tmp/iyon-tui-build --target bun
```

If the native staging script or addon contract changes, verify both:

```bash
ION_NATIVE_FEATURES=direct-ffi CARGO_BUILD_JOBS=1 bun run packages/iyon-tui/scripts/stage-native.ts
CARGO_BUILD_JOBS=1 bun run packages/iyon-tui/scripts/stage-native.ts
```

Check that the default and direct-FFI artifacts still have their intended
symbol surfaces. Direct F projection must continue using the same staged
`.node` and environment-lifetime loader; no second library is allowed.

### Existing focused suites

Run the relevant existing Bun suites, not the full benchmark suite:

```bash
bun test packages/iyon-tui/tests/tui_perf13_d.test.ts \
  packages/iyon-tui/tests/tui_harness.test.ts \
  packages/iyon-tui/tests/tui_generated_view_abi.test.ts
```

Also run any existing content/stream/scene tests affected by changed files.

## 9. Completion criteria for F

Do not mark F complete until all of these are true:

- plain Funnel output is visible in a real ContentHost;
- Source snapshots are immutable and acquired without holding Source lock during
  projection/layout;
- width-dependent projection happens before/during final measurement;
- Connector activation is part of the same convergence transaction;
- candidate B failure preserves old A or empty fallback without partial commit;
- projected Source revision/status is truthful after commit;
- width/content/layout feedback converges deterministically with a defensive
  ceiling and diagnostic;
- fixed/fill vs fit extent behavior is correct;
- scroll-only movement reuses width projection and avoids full rewrap/reparse;
- Connector switches preserve viewport intent and clamp/follow correctly;
- ContentPort supplies allocation/clip only; viewport controller owns offset/follow;
- candidate abort leaks no projection/subscription/binding;
- Source mutation during frame leaves later work pending;
- semantic structure is not rebuilt for content mutation;
- plain-text visual parity passes for ASCII and Unicode/wrap cases;
- existing A–E lifecycle, ABI, wake, ownership, and no-full-benchmark policies
  still pass.

## 10. Todo/session handoff

At the time of writing:

- Todo #46 (re-read PERF-13-F handoff): completed.
- Todo #47 (audit existing F seams): in progress.
- Todo #48 (implement PERF-13-F): pending behind #47.
- Todo #49 (verify PERF-13-F): pending behind #48.
- Todo #50 (document and commit PERF-13-F): pending behind #49.

After compaction, read this file first, then inspect the current git status and
continue with the active audit. Mark #47 complete only after the existing F
seams and implementation plan are sufficiently mapped. Start #48 only after
that. Keep exactly one todo in progress.
