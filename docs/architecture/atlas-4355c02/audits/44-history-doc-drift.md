# 44 — History-doc drift: PERF-13/API-H/L1 chronology versus the current implementation

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `44 — audits/history-doc-drift`
- Primary scope:
  - `docs/history/`
  - `reports/pre-v5-l1/`
  - current architecture documentation and referenced source
  - current History, content, host, scene, native binding, and TypeScript façade code
- No repository files were edited.
- No tests, builds, benchmarks, formatters, services, or external consumers were run during this investigation. Historical test claims below are quoted as historical evidence only.

The report contract and atlas README were read first. `PRE-V5-ARCHITECTURE-REPORT.md` was read for context and evidence expectations, without beginning its V5/census/disposition analysis.

### Evidence policy and authority hierarchy

The parent clarified that only the following document families carry relevant architectural-handoff authority for this assignment:

1. PERF-13 records;
2. API-H records;
3. L1 records under `reports/pre-v5-l1/`;
4. PRE-V5 records.

Older performance or architecture records are incidental archaeology only. They are not treated as normative and are not used to flag current source merely because the source differs from them.

Within the approved families, chronology and explicit supersession matter:

- The resolved PERF-13 handoff states that its resolved section supersedes conflicting or tentative statements in the integrated baseline body (`docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:10-14`).
- The same handoff explicitly says that PERF-13-H deleted the transitional compatibility facades and legacy History/stream paths described in the integrated baseline (`…HANDOFF-RESOLVED.md:30-34`).
- PERF-13-F is therefore a pre-G interim record, not the final content/History contract.
- PERF-13-G and PERF-13-H supersede F’s temporary coexistence statements for migrated content and legacy stream routes.
- L1 records are later implementation and qualification records. They refine the current retained implementation and record later performance/correctness work; they do not authorize V5 semantics.
- PRE-V5 documents explicitly distinguish current behavior from proposed V5 destinations. For example, the Rust-lowering handoff says that it is not a React migration, Taffy integration, GPUI migration, replacement of current History semantics, or second renderer (`docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md:8-10`).

### High-level finding

The current implementation is substantially consistent with the final PERF-13-G/H and later L1 records:

- current production streaming/content uses Source → Funnel → ContentPort → Connector → ContentHost;
- current TypeScript History exposes `layout`, `push`, `freeze`, `discardLive`, and `setLayout`;
- current native History exposes reference-based retained-view operations, not the old object-payload or stream methods;
- current Rust History remains a root-level semantic ordered flow with static/live units, provider-backed ContentHost units, width-dependent projection, and receipt-driven native scrollback;
- current content History transfer uses finalized-prefix row products and exact physical remainders rather than the pre-L1 open-row/surface-suffix approach;
- current native sink failure handling preserves irreversible physical-frontier semantics and marks synchronization unknown instead of replaying or rolling back.

The principal documentation drift is historical and chronological, not an unsuperseded current-source violation:

1. API-H1 still records the earlier `pushStream`/`sealStream` and `nativeObject()` world.
2. PERF-13-F explicitly preserves the old `HostTextStream`/History route until G.
3. PERF-13-G migrates History-backed content to ContentHost/Port/Connector.
4. PERF-13-H deletes the old stream implementation and compatibility surface.
5. L1-11/L1-12 then replace the earlier complete-surface/open-row implementation with finalized-prefix row products, cached row windows, targeted invalidation, and explicit History receipt recovery.
6. Current source matches the later state, not the earlier records.

One remaining contract-interpretation gap deserves follow-up: the resolved PERF-13 handoff describes a historical unit as capturing an immutable committed projection/snapshot descriptor at a Source revision (`…HANDOFF-RESOLVED.md:1729-1742`). Current `History` stores a ContentPort identity and the ContentHost/Connector owns Source-revision-keyed committed projection products. The behavior may satisfy the handoff through provider-owned immutable products, but the ownership of that historical snapshot is not obvious from `History` itself.

---

## 1. Responsibility and structure

### 1.1 Current implementation inventory

The current History subsystem is split into semantic, projection, and native-frontier responsibilities.

| Current path | Approximate physical size | Current responsibility |
|---|---:|---|
| `crates/iyon-tui/src/history/mod.rs` | 34 lines | Module assembly and selective exports |
| `history/boundary.rs` | 12 | Per-unit predecessor attachment policy |
| `history/error.rs` | 34 | Lifecycle/invariant errors |
| `history/id.rs` | 38 | Monotonic opaque `HistoryUnitId` allocation |
| `history/layout.rs` | 48 | Semantic padding/gap configuration |
| `history/model.rs` | 407 | Ordered units, semantic lifecycle, revisions, cache keys, attachment discovery, native metadata |
| `history/unit.rs` | 48 | Internal static/live unit and layout-key representation |
| `history/native/frontier.rs` | 104 | Physical remainder, spacing-transfer, retirement, synchronization state |
| `history/native/mod.rs` | 559 | Native prefix transfer state machine and sink handling |
| `history/projection/mod.rs` | 1,100 | Width-dependent plans, measurement, selection, retained view projection, frozen overlays |
| `history/trace.rs` | 141 | Environment-gated diagnostic tracing |
| **Recursive Rust History total** | **approximately 2,525** | Physical source lines, including comments, blank lines, and test-gated helpers |

The approximate recursive History count is also recorded in the current atlas History map (`docs/architecture/atlas-4355c02/subsystems/rust/07-history.md:79-96`). It is a physical-line count, not a compiler or production-only count.

Adjacent current seams are materially part of History behavior:

- `crates/iyon-tui/src/application/host.rs`
  - `HostHistory::layout`, `push`, `freeze`, `discard_live`
  - host-side prospective attachment validation
  - detached-to-host transfer
- `crates/iyon-tui/src/application/content.rs`
  - `HistoryAdapter`
  - ContentPort/Connector association
  - finalized-prefix preparation
  - `history_rows`
  - committed row accounting
  - unit retirement cleanup
- `crates/iyon-tui/src/scene/root.rs`
  - optional root-level History sideband
  - independent History/body resolution
  - visual merge
- `crates/iyon-tui/src/scene/host.rs`
  - native pressure draining
  - History branch refresh
  - physical synchronization recovery
- `crates/iyon-tui-native/src/tui.rs`
  - `NativeHistory`
  - N-API detached/attached lifecycle
  - retained-view reference methods
- `packages/iyon-tui/src/api/controls/history.ts`
  - TypeScript semantic handle and lifecycle façade
- `packages/iyon-tui/src/runtime/runtime.ts`
  - one-way History attachment, host ownership, root sideband publication

### 1.2 Current primary and secondary responsibilities

#### Current `History` semantic model

`crates/iyon-tui/src/history/model.rs:22-407` owns:

- ordered `VecDeque<HistoryUnit>`;
- stable History-object identity;
- semantic revision;
- separate native display-frontier revision;
- semantic `HistoryLayout`;
- per-unit width/key/height caches;
- native frontier state;
- `push`, `push_with_boundary`, `freeze`, `discard_live`, and `set_layout`;
- attachment discovery for retained state and ContentPort validation;
- synchronization-unknown state after native sink failure.

It does not own:

- terminal writes;
- terminal-backend protocol;
- generic ScrollPane offsets;
- ComponentRegistry ownership;
- ContentPort/Connector storage;
- the TypeScript handle lifecycle.

#### Current projection

`history/projection/mod.rs` owns:

- semantic unit planning;
- provider-aware resident views;
- width-dependent unit measurement;
- flow padding and gap construction;
- FollowEnd and NativeFrontier selection;
- live-tail clipping;
- retained root-column construction;
- optional physical frozen-row overlay.

This is the current source realization of the History presentation envelope. It is not the same thing as the old stream scheduler or old native stream pane described in earlier API-H/PERF-13 interim records.

#### Current native frontier

`history/native/frontier.rs` and `history/native/mod.rs` own:

- exact accepted physical-row accounting;
- irreversible front retirement;
- exact physical remainder retention after partial acknowledgements;
- top-padding and leading-gap transfer state;
- native synchronization-unknown state;
- typed transfer status;
- retirement callbacks into the ContentHost registry.

The native frontier does not write terminals itself. It consumes a `NativeHistorySink`, whose backend implementation is in `crates/iyon-tui/src/backend/native_history.rs`.

#### Current ContentHost History adapter

`application/content.rs` owns the provider side of ContentHost-backed History:

- mapping ContentPort IDs to History unit IDs;
- per-unit committed physical/content row counts;
- finalized-prefix row products;
- leading/trailing padding bookkeeping;
- committed Source-revision-keyed Connector projections;
- ContentPort/Connector cleanup on History retirement.

This is consequential for documentation drift: current History-backed content is not an independent text-stream subsystem. It is a History occurrence using the ordinary ContentPort/Connector path.

### 1.3 Chronology of the approved records

| Chronological phase | Record evidence | Meaning at that phase |
|---|---|---|
| API-H1 | `docs/history/API-H1/API-H1-V2-public-api-hygiene.md:918-934`, `:1983-2009` | Earlier API audit. Detached History was still a decision, and History had old stream attachment methods and public native-object seams. |
| API-H2 | `docs/history/API-H2/API-H2-STRUCT-1-HANDOFF-v2.md`, CUT 0–5 completions | TypeScript source-ownership cleanup. H2 preserved behavior and public names while separating API, composition, runtime, and transport. |
| API-H3-C | `docs/history/API-H3/API-H3-C-completion.md:47-59` | Structural retained path had a cold fallback during the stacked migration. This was not the final route. |
| API-H3-E | `docs/history/API-H3/API-H3-E-completion.md:126-153`, `:395-416` | Structural compatibility path was deleted. The remaining-debt text still contains stale PERF-13 content claims, discussed below. |
| PERF-13-F | `docs/history/PERF-13/PERF-13-F-completion.md:47-66` | Plain immediate content path only; old `HostTextStream`/History route deliberately remained until G. |
| PERF-13-G | `docs/history/PERF-13/PERF-13-G-completion.md:23-38`; G implementation notes `:40-54` | History-backed content migrated to ordinary ContentHost/Port/Connector; old native TextStream route removed. |
| PERF-13-H | `docs/history/PERF-13/PERF-13-H-completion.md:3-34` | Superseded stream implementation, old History stream units, scheduler/pane/transfer modules, snapshot/projector facades, and object-payload bindings removed. |
| PRE-V5-R0 cleanup | `docs/history/PRE-V5/POST-PERF13-ROT-CLEANUP.md:42-76`, `:141-153` | Structural retained route made single-path; History push/freeze retained-reference route no longer falls back to old complete-object decoding. |
| PRE-V5 Rust lowering | `docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md:8-10`, `:90-129` | Current custom History/receipt semantics are preserved; V5 component-only replacement is explicitly a future destination, not current behavior. |
| L1 early stages | `reports/pre-v5-l1/L1-00-baseline.md`, `L1-00-closure.md`, `L1-02-report.md`, `L1-04-report.md` | Baseline/correction and direct retained construction work. History projection was migrated to direct factory construction. |
| L1 delivery/lowering | `L1-09-report.md`, `L1-10-report.md`, `L1-11-report.md` | Source/semantic/parser and delivery products became persistent/cached; History switched to finalized-prefix row products and row-window painting. |
| L1 targeted commit/recovery | `L1-12-report.md`, `L1-13-checkpoint.md`, `final-implementation-review.md`, `post-cleanup-qualification.md` | Native History receipts, retirement, synchronization recovery, and targeted invalidation were hardened. Historical child records remain evidence, not automatic final acceptance. |

---

## 2. Types, APIs and contracts

### 2.1 Current Rust semantic API

The current Rust symbols are defined in:

- `crates/iyon-tui/src/history/mod.rs:18-34`
- `history/model.rs:22-407`
- `history/layout.rs:3-48`
- `history/boundary.rs:3-12`
- `history/error.rs:5-32`
- `history/id.rs:9-38`

The module is `mod history` at the crate root (`crates/iyon-tui/src/lib.rs:18-20`), and selected symbols are re-exported as `pub(crate)` (`lib.rs:72-74`). Therefore the Rust type declarations are public within the crate/runtime boundary but are not a direct external Rust authoring surface at the current baseline.

Important symbols:

```text
History
HistoryLayout
HistoryUnitId
FlowBoundary
HistoryError
```

`HistoryUnitContent` is internal:

```text
Static(View)
Live(View)
```

A View is classified as `Live` when it contains component identity. A ContentHost-bearing View without component identity is classified as `Static`, but provider projection revisions still participate in its layout-cache key (`history/model.rs:72-97`; `history/unit.rs:12-28`).

Current lifecycle contracts:

- `push` appends a static/live semantic unit and bumps semantic revision.
- `freeze` replaces a live unit with a component-free final View, invalidates that unit’s layout, and bumps semantic revision.
- `discard_live` removes only a live tail unit.
- `set_layout` invalidates all semantic layout products when the layout changes.
- native transfer is separate from semantic freezing; `freeze` does not itself write terminal rows or retire a unit.

### 2.2 Current TypeScript API

`packages/iyon-tui/src/api/controls/history.ts:9-23` exposes:

```text
layout(): HistoryLayout
push(view: View): number
freeze(unit: number, view: View): void
discardLive(unit: number): void
setLayout(layout: HistoryLayout): void
```

The concrete class is `History` (`history.ts:44-115`). Its current operation routes are:

- `push`:
  - `tryRetainedMaterializeRef(view)`;
  - explicit error if retained materialization refuses;
  - `NativeHistoryContract.pushRef(ref)`;
  - release temporary native reference.
- `freeze`:
  - same retained reference route;
  - `NativeHistoryContract.freezeRef(unit, ref)`.
- `discardLive`:
  - positive safe-integer validation;
  - `NativeHistoryContract.discardLive(unit)`.
- `setLayout`:
  - direct native control method.

No `pushStream`, `sealStream`, `nativeObject`, or stream attachment methods exist in the current TypeScript History façade.

The lifecycle comment at `history.ts:28-42` documents:

- `new History()` creates detached caller-owned storage;
- `Tui.createHistory()` creates a host-attached Tui-owned History;
- a detached History can transfer to one Tui;
- attachment is one-way and single-host;
- `freeze` and `discardLive` require attachment.

This part of the current implementation agrees with the useful portion of the API-H1 audit, while the old stream/native-object portions do not.

### 2.3 Current native binding API

`crates/iyon-tui-native/src/tui.rs:299-438` defines `NativeHistory`.

Current native methods include:

```text
new
isDetached
layout
setLayout
pushRef
freezeRef
discardLive
```

`NativeHistoryContract` in `packages/iyon-tui/src/transport/native/addon.ts:18-26` matches this shape:

```text
dispose()
layout()
setLayout(layout)
isDetached()
pushRef(viewRef)
freezeRef(unit, viewRef)
discardLive(unit)
```

There are no current native `push`, `freeze`, `pushStream`, or `sealStream` methods. The staging script retains absence assertions for those old names (`packages/iyon-tui/scripts/stage-native.ts:53-68`); those strings are guard vocabulary, not live methods.

### 2.4 Explicit API-H1 contradictions and supersessions

#### API-H1 old stream methods versus current API

API-H1’s later audit records:

- `history.ts` casts `TextStream` through `nativeObject()` for `pushStream()` and `sealStream()` (`API-H1-V2-public-api-hygiene.md:1983-1989`);
- detached History can “attach streams” (`:1995-1998`);
- the public History contract included old stream behavior.

Current source has none of these methods. Search scope:

```text
packages/iyon-tui/src/**
crates/iyon-tui/**
crates/iyon-tui-native/src/**
```

No production `pushStream`, `sealStream`, `HostTextStream`, `NativeTextStream`, `StreamPane`, or stream scheduler symbols were found. The only surviving `stream` Rust module is `crates/iyon-tui/src/stream/coord.rs` plus `stream/mod.rs`, containing source-rooted `StreamOffset` and `StreamRange`.

This is an explicit historical contradiction, but it is superseded by PERF-13-G/H. It is not a current implementation defect.

#### API-H1 public `nativeObject()` versus current raw-native split

API-H1 describes `History.nativeObject()` and related raw-native seams as public implementation leakage (`:1983-1989`). API-H2 CUT 5 instead says raw native/bridge/generated layers are not exposed by public declarations (`API-H2-CUT-5-completion.md:137-148`).

Current TypeScript History uses private `nativeAs`/`nativeResourceOf` transport seams and does not expose `nativeObject()`. This is a resolved API-H1 → API-H2 drift, not a current contradiction.

#### API-H1 optional `setLayout` versus current required method

API-H1 records that `History.setLayout` was optional in the interface but required in the concrete class (`:2007-2009`). Current `History` interface and class both require `setLayout` (`history.ts:9-15`, `:102-104`). This historical mismatch is resolved.

#### Detached History status

API-H1 leaves detached History as a decision (`:918-934`, `:2094-2100`). Current source makes the behavior deliberate and explicit:

- detached creation is supported;
- detached `layout`/`push`/`setLayout` are usable;
- host-only `freeze`/`discardLive` reject when detached in native implementation;
- detached transfer is one-way and single-host.

This current behavior is documented in `history.ts:28-42`, implemented in `NativeHistory::is_detached` and `take_for_host` (`tui.rs:327-342`), and enforced in `runtime.ts:280-316`.

### 2.5 Current source versus PERF-13 History contract

The resolved PERF-13 handoff says:

- History consumes the same committed content/viewport model;
- it must not create a second Source subscriber or content scheduler;
- live content is projected through the active Connector;
- historical units capture a committed projection/snapshot descriptor at a Source revision;
- History may retain immutable Source chunks/annotations or a frozen projection;
- freezing and Connector release are transactional;
- History must not receive high-volume text through the superseded payload bridge (`…HANDOFF-RESOLVED.md:1729-1742`).

Current source clearly satisfies the shared-route and no-old-bridge portions:

- `History` stores a ContentHost View/ContentPort identity;
- ContentHost uses existing Source/Funnel/Connector state;
- `application/content.rs:6242-6331` obtains finalized rows from the provider;
- no separate History Source subscriber or old text payload bridge exists;
- `history/native/mod.rs:187-225` calls `ContentProvider::history_rows`, then performs ordinary native row transfer.

The ownership detail is less explicit. Current `History` itself stores only the ContentPort-bearing View and unit ID. Source-revision-keyed immutable products are held by Connector projection/cache state (`application/content.rs:210-250`, `:328-491`, `:1022-1255`, `:2994-3010`), and committed projection revision is exposed in Connector state (`:5126-5130`, `:6178-6182`). This may satisfy the contract’s “History may retain” wording through provider-owned immutable products, but the record does not make the historical snapshot owner obvious.

---

## 3. Dependency and ownership map

### 3.1 Current forward dependency graph

```text
TypeScript caller
    │
    ├── new History()
    │       └── nativeTui.history()
    │
    ├── tui.createHistory()
    │       └── NativeTuiHost.history()
    │
    └── Scene(body, history)
            │
            ▼
TypeScript runtime
    ├── stageHistoryBinding()
    ├── prepareRootPublication()
    ├── commitHistoryBinding()
    └── Host.setHistory(detached NativeHistory)
            │
            ▼
NativeHistory
    ├── detached History state: Mutex<History>
    └── attached HostHistory
            │
            ▼
Rust HostHistory
    ├── validates prospective state/content attachments
    ├── History::push / freeze / discard_live
    └── ContentHostRegistry::set_history_unit()
            │
            ▼
Scene / root History
    ├── optional root-level History
    ├── independent History projection
    └── merge History above body
            │
            ├── ordinary static/live View projection
            ├── ContentHost provider projection
            └── native overflow pressure
                    │
                    ▼
History native frontier
    ├── finalized ContentProvider rows
    ├── static compiled rows
    ├── frozen physical remainder
    ├── exact sink acknowledgement
    └── retirement / synchronization state
            │
            ▼
NativeHistorySink
    └── backend terminal implementation
```

### 3.2 Reverse consumers

Current reverse consumers include:

- `Scene` stores one optional root-level History (`crates/iyon-tui/src/scene/root.rs:24-97`).
- `Scene` resolves and merges History separately from the body (`root.rs:196-260`, `:301-380`).
- `SceneHost` drains native History pressure and refreshes History-only revisions.
- `HostHistory` exposes serialized host operations (`application/host.rs:819-947`).
- `ContentHostRegistry` implements provider callbacks needed for ContentHost-backed History (`application/content.rs:6242-6683`).
- terminal backends implement `NativeHistorySink`.
- Rust tests and benchmark fixtures directly invoke History projection/transfer internals.
- TypeScript tests exercise History through `Tui`, `AppHarness`, and the native façade:
  - `packages/iyon-tui/tests/tui_history_prefix.test.ts`
  - `packages/iyon-tui/tests/tui_handles.test.ts`
  - `packages/iyon-tui/tests/tui_harness.test.ts`
  - `packages/iyon-tui/tests/fixtures/tui_demo.ts`

### 3.3 Ownership and lifetime

| Object/state | Created by | Owner | Release/removal |
|---|---|---|---|
| `History` semantic object | `History::new` / native host factory | `NativeHistory` while detached, then `Scene`/Host state | native disposal or host lifecycle |
| `HistoryUnit` | `History::push` | `History.units` | `discard_live` or native `retire_front` |
| `HistoryUnitId` | `HistoryUnitId::allocate` | unit identity and retirement callbacks | invalid once unit removed |
| ContentPort association | `HostHistory::push` / `freeze` | `ContentHostRegistry::HistoryAdapter` | clear on replacement/discard; retire on native unit retirement |
| Source/Connector projection | ContentHost provider | Connector record/cache and committed projection | Connector lifecycle, cache invalidation, retirement cleanup |
| Frozen physical remainder | partial sink acknowledgement | `NativeFrontier` | replaced as accepted; cleared at unit retirement |
| terminal rows | sink acknowledgement | backend physical terminal history | backend-specific; History never rolls back them |
| History attachment ownership | `runtime.ts` staging/commit | one Tui host | one-way attachment; host close invalidates caller-bound handle |

### 3.4 The important ownership boundary

Current History does not own the Source or Connector:

```text
History owns:
  semantic order, unit IDs, boundary policy, layout, native frontier

ContentHostRegistry owns:
  ContentPort/Connector association, provider projections,
  committed source/delivery products, History row extraction

Native backend owns:
  physical terminal state and sink acknowledgement

Tui runtime owns:
  TypeScript wrapper ownership, one-way host binding,
  detached/attached liveness policy
```

This agrees with final PERF-13 content-plane ownership and with the L1-12 statement that the implementation retains current native input/tick/History ownership while adding generic content/frame mechanics (`L1-12-report.md:9-13`).

---

## 4. Execution paths and state transitions

### 4.1 Detached History creation

Current TypeScript:

```text
new History()
  → nativeTui.history()
  → NativeHistory::new()
  → History::new()
  → host = None
```

References:

- `packages/iyon-tui/src/api/controls/history.ts:44-56`
- `crates/iyon-tui-native/src/tui.rs:307-325`

The detached object can perform semantic operations permitted by its native wrapper, but host-only operations are rejected while detached.

Tui-owned creation:

```text
tui.createHistory()
  → Tui::createHistory()
  → NativeTuiHost::history()
  → NativeHistory::from_host()
  → HostHistory
```

References:

- `packages/iyon-tui/src/runtime/runtime.ts:563-570`
- `crates/iyon-tui-native/src/tui.rs:439-447`

### 4.2 Attach-once publication

Current runtime stages the History sideband with the body publication:

```text
Scene producer
  → stageHistoryBinding()
  → prepareRootPublication()
  → retained structural publication
  → commitHistoryBinding()
  → NativeTuiHost.setHistory()
  → host frame pending/flush
```

References:

- `runtime.ts:224-278`
- `runtime.ts:280-316`
- `runtime.ts:455-549`

The runtime rejects a different History after one has already been bound:

```text
if boundHistory exists and history !== boundHistory:
    TUI_HISTORY_ALREADY_BOUND
```

The native layer separately rejects attempting to transfer an already attached History (`tui.rs:331-342`, `:783-804`).

The current one-way behavior therefore agrees with the resolved API-H1 decision as implemented after H2/H3 cleanup.

### 4.3 Semantic append

Current Rust route:

```text
caller
  → HostHistory::push(view)
  → prospective state/content target validation
  → History::push(view)
  → classify Static or Live
  → allocate HistoryUnitId
  → append to VecDeque tail
  → clear aggregate layout cache
  → bump semantic revision
  → bind ContentPort to unit if present
  → invalidate/render host
```

References:

- `application/host.rs:848-875`
- `history/model.rs:72-97`

For a ContentHost View, `HostHistory::push` records the unit association with the ContentPort and original padding/insets (`host.rs:859-871`).

For a component-bearing View, the History unit is `Live`; projection resolves it with component dependencies (`history/projection/mod.rs:190-203`).

For a static ContentHost View, the unit remains `Static`, but the provider projection revision participates in `HistoryUnitLayoutKey::Content` (`unit.rs:21-28`; `projection/mod.rs:172-188`).

### 4.4 Root resolution and projection

Current root resolution:

```text
Scene
  → resolve body
  → measure body
  → compute remaining History height
  → project History at (terminal width, history height)
  → finish History session
  → merge History above body
```

References:

- `scene/root.rs:196-260`
- `scene/root.rs:301-380`

Current History projection:

```text
History
  → collect ordered units
  → build UnitPlan for each unit
  → resolve live dependencies
  → obtain provider-aware static View
  → calculate Content projection key
  → reuse or measure per-unit height
  → account for frozen physical remainder/native rows
  → select FollowEnd or NativeFrontier viewport
  → build retained root column
  → optionally create physical overlay
```

References:

- `history/projection/mod.rs:157-249`
- selection functions around `:653-875`
- flow construction around `:978-1077`

The root History is an optional root-level sideband, not a nested generic composition child. This remains consistent with current `Scene` documentation (`scene/root.rs:13-22`, `:47-78`).

### 4.5 Freeze

Current host freeze sequence:

```text
HostHistory::freeze(unit, final_view)
  → validate unit ID
  → compute prospective state-bearing Views
  → validate state targets
  → compute prospective content-bearing Views
  → validate content targets
  → History::freeze()
  → clear old History-unit content association
  → bind replacement ContentPort if needed
  → update desired state/content bindings
  → invalidate/render
```

References:

- `application/host.rs:877-918`
- current atlas History map `07-history.md:719-734` as a source-map cross-check

The Rust model rejects:

- unknown unit;
- non-live unit;
- final View containing component identity.

`freeze` does not itself transfer native rows. Native transfer is a later pressure/overflow operation.

### 4.6 Discard

Current route:

```text
HostHistory::discard_live(unit)
  → validate numeric ID
  → History::discard_live()
  → remove only live tail
  → clear ContentPort association
  → refresh desired state bindings
  → invalidate/render
```

References:

- `application/host.rs:921-935`
- `history/model.rs:99-114`

No native rows are produced by discard. This is a semantic deletion.

### 4.7 Content-backed native transfer

Current host/native path:

```text
SceneHost::drain_native_pressure()
  → transfer_native_prefix_with_theme_and_content()
  → check synchronization_unknown
  → transfer_native_prefix_inner()
  → top padding / leading gap
  → frozen content remainder if present
  → frozen static remainder if present
  → inspect front semantic unit
      Live                 → SemanticBlocked
      Static ContentHost   → ContentProvider::history_rows()
      ordinary Static      → compile static physical rows
  → NativeHistorySink::insert_history_rows()
  → accept exact prefix
  → update provider committed rows
  → retire complete unit
  → drain retired unit callbacks
  → bump native revision
  → re-resolve if native display frontier changed
```

References:

- `history/native/mod.rs:62-130`
- transfer dispatch `:135-237`
- static/content transfer `:338-478`
- retirement `:546-559`
- `application/content.rs:6242-6331`, `:6334-6359`

The provider’s `history_rows` route explicitly obtains finalized-prefix rows:

- `application/content.rs:345-350` documents the finalized-prefix product;
- `:921-924` distinguishes finalized rows from the open document;
- `:1168-1197` prepares or reuses finalized products;
- `:1230-1255` applies delivery/stability limits;
- `:6260-6274` refuses to export unstable open rows and calculates complete/partial transfer state.

This is a direct implementation of the later L1-11 correction over the earlier open-row/surface-copy route.

### 4.8 Partial receipt and retirement

When the sink accepts fewer rows than requested:

- exact physical rows are retained in `FrozenStaticRemainder` or `FrozenContentRemainder`;
- the semantic History unit remains at the front;
- later transfer resumes from the exact frozen remainder;
- no regeneration from mutable semantic content is required.

When a unit becomes complete:

- `retire_front` removes it from semantic History;
- unit ID is added to the retirement list;
- the outer adapter drains retirements even if a later sink call fails;
- ContentHost provider state is removed only after retirement callback processing.

This agrees with L1-11’s partial/zero receipt evidence (`L1-11-report.md:48-60`) and L1-12’s explicit retirement/failure fixes (`L1-12-report.md:56-63`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to current production route

| Semantic operation | Current production path | Selection condition | Success result | Failure/blocked result |
|---|---|---|---|---|
| Append ordinary View | `HostHistory::push` → `History::push` | View has no component identity | Static unit appended | Attachment validation may reject before mutation |
| Append component View | same | View contains component identity | Live unit appended | Attachment validation may reject before mutation |
| Append ContentHost View | same plus `set_history_unit` | View carries ContentPort identity | Static provider-backed unit | Missing/invalid host attachment rejects before mutation |
| Freeze live unit | Host prospective validation → `History::freeze` | Unit is live and final View has no component identity | Static final View | Unknown/non-live/component-bearing final View rejected |
| Discard live | `HostHistory::discard_live` → `History::discard_live` | Unit is live semantic tail | Unit removed | Non-tail/static/unknown unit rejected |
| Project History | `project_into_session...` | FollowEnd or NativeFrontier anchor | Retained View plus optional physical overlay | Resolver errors propagate |
| Transfer ordinary static | `static_rows` → `insert_prefix` | Front is static and no ContentHost identity | Exact accepted physical prefix | SinkBlocked, sink error, invalid acknowledgement |
| Transfer ContentHost | `history_rows` → `transfer_content` | Front static View carries ContentPort | Finalized/stable provider rows transferred | SemanticBlocked if provider cannot prove rows |
| Transfer live unit | native transfer dispatch | Front unit is Live | No transfer | `SemanticBlocked { reason: Live }` |
| Synchronization recovery | successful host recovery candidate | Prior physical state may be unknown | Marker cleared after recovery | Normal transfer blocked while marker remains |
| Detached native `freeze`/`discard` | Native wrapper checks `host` | Detached native History | Rejects | Explicit “detached history cannot…” native error |

### 5.2 Current alternate wrappers are not old architectures

There are three native transfer entrypoints:

- `transfer_native_prefix` (`#[cfg(test)]`);
- `transfer_native_prefix_with_theme`;
- `transfer_native_prefix_with_theme_and_content`.

The first is test-only. The second supplies `EmptyContentProvider` and is useful for ordinary static History tests. The third is the host route that can transfer ContentHost-backed rows.

This is not a silent previous-generation fallback:

- a ContentHost unit sent through the empty provider route remains blocked;
- no old stream scheduler or object-payload implementation is selected;
- the current route returns typed `SemanticBlocked` rather than fabricating rows;
- structural retained-reference refusal is explicit after PRE-V5-R0 cleanup.

This distinction matters because PERF-13-H’s “no second object-payload fallback” language concerns the deleted structural/native compatibility architecture, not a test convenience that deliberately supplies an empty ContentProvider.

### 5.3 Failure semantics

Current failure classifications are explicit:

- `HistoryError::UnitNotFound`
- `HistoryError::UnitNotLive`
- `HistoryError::LiveMustRemainTail`
- `HistoryError::FinalViewContainsComponent`
- `NativeTransferStatus::SinkBlocked`
- `NativeTransferStatus::SemanticBlocked`
- `NativeTransferError::Sink`
- `NativeTransferError::InvalidAcknowledgement`
- `NativeTransferError::SynchronizationUnknown`

A native sink failure does not rewind logical History:

- `NativeFrontier.synchronization_unknown` is set;
- transfer is refused until a successful recovery frame clears the marker;
- accepted physical rows are never replayed speculatively.

This is directly recorded in source (`history/native/frontier.rs:55-60`, `:73-79`) and in L1-12 (`L1-12-report.md:58-63`).

### 5.4 Search-based absence claim

The following current production scopes were searched for deleted stream/fallback vocabulary:

```text
crates/iyon-tui/src/**
crates/iyon-tui-native/src/**
packages/iyon-tui/src/**
packages/iyon-tui/scripts/**
```

No live production definitions or calls were found for:

```text
pushStream
sealStream
HostTextStream
NativeTextStream
StreamPane
stream scheduler
stream transfer module
old History stream attachment
tryNativeMaterialize
prepareColdInstall
cold-lowering.ts
NativeHistory.push
NativeHistory.freeze
```

The deleted words remain in some tests, documentation, and absence assertions. Those occurrences are not production execution paths.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 History semantic and native revisions

`History` has two revisions (`history/model.rs:34-40`):

- semantic `revision`;
- native display-frontier `native_revision`.

Semantic revision changes for:

- push;
- freeze;
- discard;
- layout changes.

Native revision changes when:

- physical rows are accepted;
- a semantic unit retires from native transfer;
- a zero-row semantic retirement makes the display frontier change.

The separation allows `SceneHost` to refresh the History branch without rebuilding an unchanged body branch. This is reflected in `scene/host.rs`’s separate History/body revision bookkeeping and in L1-12’s statement that native display revision advancement was corrected to once per successful transfer operation (`L1-12-report.md:58-61`).

### 6.2 Per-unit layout cache keys

`HistoryUnitLayoutKey` (`history/unit.rs:12-28`) has three forms:

```text
Static(ViewId)

Content {
    view: ViewId,
    projection: u64,
}

Live {
    view: ViewId,
    dependencies: Vec<(ComponentId, ComponentRevision)>,
}
```

Cache validity requires both:

1. matching width;
2. matching semantic/provider/component dependency key.

Consequences:

- static component-free units reuse height by View identity;
- ContentHost units invalidate when provider projection changes;
- live units invalidate when reachable component revisions change;
- width changes invalidate all affected unit heights;
- `HistoryCachedHeightHits` records successful unit-height cache reuse.

`cached_total_flow_height` can use the aggregate only when no stale unit heights remain; otherwise it scans unit heights and repairs the aggregate.

### 6.3 Content projection cache keys

Current `application/content.rs` keys content products using Source and delivery identity:

- Source ID;
- Source generation;
- Source revision;
- width;
- wrap mode;
- Funnel kind;
- delivery revision;
- theme revision;
- whether finalized-prefix rows are needed;
- whether physical rows are needed.

References:

- `content.rs:210-250`
- `:1022-1045`
- `:2994-3010`

This prevents a History unit from reusing an ordinary open-content product when finalized-prefix semantics are required. L1-11 explicitly records that sealed/open semantic cache aliasing was prevented by the sealed/finalized key component (`L1-11-report.md:57-60`).

### 6.4 Finalized-prefix product versus old open-row path

The PRE-V5 Rust-lowering handoff describes an earlier route:

```text
TextRenderer
  → View graph
  → complete rows
  → complete Surface
  → smoothing/History suffix copies
  → ContentProvider Surface
```

It specifically records:

- complete-surface compilation;
- `reveal_surface` whole-surface clones;
- copied Source prefixes for open content;
- `surface_suffix` copies for transferred History prefixes.

References:

- `PRE-V5-RUST-LOWERING-HANDOFF.md:215-235`

That description is explicitly contradicted by later L1-11 source/record state:

- L1-11 says History no longer uses an open-row slice;
- finalized-prefix row products are the sole transferable open/sealed source;
- production ViewCompiler no longer returns a content-sized offscreen Surface;
- the old reveal helper remains only as a test differential helper.

References:

- `reports/pre-v5-l1/L1-11-report.md:19-45`
- `:48-75`
- `:98-106`

This is a real chronology/document contradiction, but the later L1 implementation record supersedes the earlier pre-L1 performance description.

### 6.5 Smooth delivery work

L1-10 records the transition from per-tick reprocessing to cached delivery:

- `ConnectorDelivery` separates Source acceptance from clock advancement;
- grapheme units are cached and binary-searched;
- `VisibilityIndex` applies only the revealed row window;
- prepared paint products are reused;
- due connectors are indexed by active deadlines;
- candidate delivery frontier is separate from committed visible frontier;
- pure ticks perform zero TypeScript transport, parser/diff/Markdown/ANSI work, fresh content View lowering, and whole-surface copying.

References:

- `L1-10-report.md:7-37`
- tests/evidence `:39-64`

Current source retains these products in `application/content.rs`; current History consumes the finalized/stable row product rather than rerunning an old stream path.

### 6.6 Native pressure and transfer work

Current native transfer is bounded by:

- available History overflow rows;
- terminal width;
- current front unit;
- exact sink acknowledgement.

The host repeatedly transfers while progress consumes budget. A semantic zero-row retirement still counts as progress, ensuring the scene refreshes after a unit disappears even if no physical row entered the sink.

L1-12 explicitly records:

- reusable retirement list instead of before/after whole-History set scanning;
- maintained unit-to-Port lookup instead of scanning every Port on retirement;
- synchronization unknown after partial/error sink behavior;
- no History frontier rollback.

References:

- `L1-12-report.md:56-63`
- `:104-123`

### 6.7 No execution validation claimed here

The current investigation did not run the counters or benchmarks. Historical counter and timing records are retained only as provenance:

- L1-00 closure’s `trace-perf-counters.json` records History/content counters at SHA `36efe16…`;
- L1-11 records pruning and row-product counters;
- `post-cleanup-qualification.md` records macOS arm64 timing and memory qualification;
- these are not measurements performed against the current investigation process.

---

## 7. Tests, benchmarks and observability

### 7.1 Current behavioral tests found

Current History-focused or History-adjacent tests include:

- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
  - static History prefix ordering;
  - body pinned after History;
  - live unit then freeze;
  - History output preservation after append.
- `packages/iyon-tui/tests/tui_handles.test.ts`
  - detached History creation and disposal;
  - retained View push;
  - host attachment behavior.
- `packages/iyon-tui/tests/tui_harness.test.ts`
  - host-created History and native row readback.
- `crates/iyon-tui/src/scene/root_tests.rs`
  - History/body mount order;
  - duplicate component identity rejection across History/body.
- `crates/iyon-tui/src/application/content.rs` History tests:
  - finalized-prefix correctness;
  - complete/open transfer;
  - leading/trailing padding;
  - partial/zero receipt;
  - Smooth transfer;
  - retirement cleanup;
  - synchronization failure fixtures.
- `crates/iyon-tui/src/history/trace.rs`
  - trace helper non-panicking behavior.

### 7.2 Current observability

`history/trace.rs:1-141` provides environment-gated structured diagnostics:

```text
IYON_HISTORY_TRACE=1
```

Events include:

- `projection`;
- `transfer`;
- `resolve_pressure`.

Fields include:

- frame sequence;
- terminal/body/History dimensions;
- anchor;
- physical rows inserted;
- last native unit;
- resident unit count;
- total flow height;
- overflow rows;
- slack;
- transfer requested/accepted/status;
- resolve/pass/transfer-call counts.

Counters include at least:

- `HistoryCachedHeightHits`;
- `HistoryUnitsMeasured`;
- content semantic/projection/paint counters;
- native transfer and physical-row counters.

### 7.3 Historical test-evidence status

Historical records must not be conflated:

- `L1-00-baseline.md:13-24` records the pre-fix baseline;
- `L1-00-closure.md:3-22` says the fixed SHA supersedes that behavior baseline;
- `L1-11-report.md:48-67` records a 648-test library run and row/History differential fixtures;
- `L1-12-report.md:135-164` records later 671-test and workspace checks;
- `L1-13-checkpoint.md:7-9` explicitly says it was not final L1-13 acceptance;
- `final-implementation-review.md:52-56` says historical child reports are not substitutes for final qualification;
- `post-cleanup-qualification.md:180-182` says the historical L1-13 draft was not relabeled as passing.

Therefore, claims such as “98 tests,” “105 tests,” “113 tests,” “648 tests,” “671 tests,” “719 tests,” and “123 Bun tests” are all provenance-specific records, not competing current test totals.

### 7.4 Benchmark route integrity

Approved records distinguish route proof from timing:

- PERF-13-H says direct-FFI and content benchmarks were run, but the full PERF-12 benchmark suite was intentionally not run (`PERF-13-H-completion.md:36-52`).
- L1-13 records incomplete performance acceptance and an unwaived small-append regression (`L1-13-checkpoint.md:47-67`).
- final qualification accepts macOS arm64 use with explicit platform and workload limits (`post-cleanup-qualification.md:14-23`, `:121-152`).

No current-source contradiction should be inferred from benchmark count differences without comparing SHA, artifact hash, workload, and feature profile.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Contradiction register

| ID | Historical/document claim | Current source/document evidence | Status and interpretation |
|---|---|---|---|
| HD-01 | API-H1 documents `pushStream`, `sealStream`, `TextStream.nativeObject`, and History stream attachment (`API-H1:1983-1998`). | Current `History` has only `layout`, `push`, `freeze`, `discardLive`, `setLayout`; native contract has only reference methods (`history.ts:9-15`; `addon.ts:18-26`). No production old stream symbols found. | Explicit API-H1 → PERF-13-G/H supersession. Not a current defect. |
| HD-02 | PERF-13-F says old `HostTextStream`/History pipeline remains as pre-G compatibility path (`PERF-13-F:64-66`). | PERF-13-G migrates History content to ContentHost/Port/Connector; H deletes old route; current source has no old route. | Intentional interim chronology. F is not final. |
| HD-03 | PERF-13-G implementation notes say generic Rust stream algorithms remain available as framework semantic algorithms/tests (`PERF-13-G-implementation-notes:51-54`). | Current `crates/iyon-tui/src/stream/` contains only coordinate types (`coord.rs`, `mod.rs`); old stream model/projector/transfer/pane files are absent. | Documentation overstates what survived after H/L1 cleanup, or describes an earlier G tree. H explicitly deletes the superseded implementation. |
| HD-04 | PERF-13-H says old History stream units and scheduler/pane/transfer modules were deleted (`PERF-13-H:5-16`). | Current source still has `HistoryUnit`, `history/native`, and `history/projection`. | Lexical ambiguity, not proven semantic contradiction: current `HistoryUnit` is the generic static/live semantic unit, while H likely means the old stream-specific units. The H record does not name deleted files/types precisely enough to prove identity. |
| HD-05 | API-H3-E remaining-debt section says content/source transport, stream handoff, and `pushStream`/`sealStream` redesign remain deferred (`API-H3-E:395-405`). | PERF-13-G/H subsequently claim those capabilities were migrated/deleted; current source has Source/Funnel/Port/Connector and no push/seal History methods. | Explicit stale H3-E tail, superseded by PERF-13-G/H. |
| HD-06 | H3-C says `cold-lowering.ts` remains a complete derived fallback (`API-H3-C:47-59`). | H3-E deletes the structural install fallback; PRE-V5-R0 deletes cold-lowering/complete-object decode; current file is absent. | Structural migration chronology, not a History route defect. |
| HD-07 | PRE-V5 Rust handoff describes complete Surface creation, smoothing/History suffix copies, copied Source prefixes, and `surface_suffix` (`PRE-V5-RUST-LOWERING-HANDOFF:215-235`). | L1-11 says the old open-row History slice and content-sized production Surface are no longer authoritative; current content uses finalized-prefix row products and exact physical remainders. | Explicit pre-L1 → L1-11 implementation drift. Later L1 record supersedes earlier cost/path description. |
| HD-08 | PRE-V5 Rust handoff maps current History transfer toward deletion/replacement during a future component-only Surface migration (`:1211-1217`). | Current source retains root-level History, native scrollback, and receipt-driven transfer. | Not a current mismatch. The same handoff explicitly labels this as a V5 destination and says current History semantics are not being replaced (`:8-10`, `:115-129`). |
| HD-09 | Resolved PERF-13 contract says historical units capture an immutable committed projection/snapshot descriptor at a Source revision (`…HANDOFF-RESOLVED.md:1735-1740`). | `History` stores a ContentPort-bearing View/unit ID; Source revision and committed projection are held in Connector/provider products (`application/content.rs:210-250`, `:2994-3010`, `:5126-5130`). | Potential ownership/documentation gap. Current behavior may satisfy the contract via provider-owned immutable Arc products, but the historical snapshot owner is not evident in `History` itself. Follow-up needed; do not declare a deviation without clarifying the intended ownership wording. |
| HD-10 | PRE-V5 Rust handoff says current History/receipt semantics must be preserved (`:90-94`, `:1184-1188`). | L1-12 says current History residency/receipt semantics are retained and then documents synchronization-unknown/retirement fixes (`L1-12:9-13`, `:56-63`). | Agreement; no drift. |
| HD-11 | POST-PERF13 cleanup says the structural path is one authoritative route and History push/freeze use retained references (`POST-PERF13:10-18`, `:141-153`). | Current TS push/freeze use `tryRetainedMaterializeRef` and `pushRef`/`freezeRef`; no old structural fallback exists. | Agreement. |
| HD-12 | API-H1 says detached History’s status needed a decision (`API-H1:918-934`). | Current TS/native/runtime explicitly implement detached creation, attach-once transfer, host-only freeze/discard, and single-host ownership (`history.ts:28-42`; `runtime.ts:280-316`; `tui.rs:327-342`). | Resolved decision, no drift. |
| HD-13 | API-H1 records optional `History.setLayout` and public raw native leakage (`API-H1:1989-2009`). | Current interface requires `setLayout`, and raw native methods are transport-private (`history.ts:9-15`, `:102-125`). | Resolved by H2/H3 boundary work. |
| HD-14 | L1-13 checkpoint records itself as not final acceptance (`L1-13:7-9`). | Final review and post-cleanup qualification preserve that distinction (`final-implementation-review:52-56`; `post-cleanup-qualification:180-182`). | Agreement; do not promote checkpoint claims to final evidence. |
| HD-15 | Current architecture README says the atlas is a source snapshot and final living architecture documents are pending (`docs/architecture/README.md:3-10`, `:18-21`). | `POST-PERF13-ROT-CLEANUP.md:18-19` says the architecture census is “UNGATED.” | Status-word ambiguity/documentation drift. “Ungated” appears to refer to the cleanup’s own exit gate, while the current architecture README says the later census deliverables remain pending. Parent should preserve both meanings explicitly rather than treating one as a final census acceptance claim. |

### 8.2 Most consequential contradiction: old complete-surface History path

The clearest implementation/documentation transition is HD-07.

The PRE-V5 Rust-lowering handoff describes a path in which:

```text
complete rendered Surface
  → smoothing/History suffix copies
  → ContentProvider surface
```

It also describes open content as copied Source-prefix parsing and transferred History as `surface_suffix` copying.

L1-11 explicitly records that this is no longer the authoritative route:

- finalized-prefix row products replace open-row slicing;
- production ViewCompiler no longer produces a content-sized offscreen Surface;
- row-window painting replaces full-surface allocation in the production path;
- exact partial receipts are compared against finalized baselines.

Current source confirms the later state:

- `application/content.rs:345-350` stores a finalized-prefix product;
- `:921-924` distinguishes finalized rows from open rows;
- `:1168-1197` creates/reuses finalized products;
- `:6260-6274` exports only stable/finalized rows;
- `history/native/mod.rs:365-478` transfers provider rows and retains exact physical remainders.

This is not a V5 redesign. It is a completed pre-V5/L1 implementation transition.

### 8.3 History provider revision versus immutable historical descriptor

The resolved PERF-13 handoff is clear that History should not create a second Source subscriber/scheduler, but it also describes capture of an immutable historical descriptor. Current implementation instead keeps the ContentPort association and obtains current provider rows through the Connector projection:

```text
History unit
  → ContentPort ID
  → selected Connector
  → Source-revision-keyed committed projection
  → finalized/stable rows
  → native transfer
```

Evidence that current provider products are revision-keyed and immutable/shared:

- `TextProjectionKey` includes Source identity/generation/revision (`application/content.rs:210-250`);
- `HostContentProjection` and finalized products are `Arc`-owned (`:328-350`, `:1022-1255`);
- Connector tracks candidate versus committed projection (`:2994-3010`);
- committed projection revision is installed only on promotion (`:5126-5130`);
- History row export uses finalized/stable products, not mutable open rows (`:6260-6274`).

The current behavior may therefore satisfy the contract with ContentHost/provider ownership rather than History-object ownership. The unresolved point is documentation precision: the handoff says “History captures” and “History may retain,” while current code makes the provider the obvious owner. This should remain an open question, not an assumed contradiction.

### 8.4 V5 must not be treated as current source

Several approved PRE-V5/PERF-13 records mention future V5 behavior:

- component-only Surface/residency;
- semantic delivery frontier;
- Taffy/downstream layout adapter;
- deleting/replacing the current History transfer adapter;
- broader occurrence/viewport abstractions.

These are future destination statements. Current source remains:

- custom terminal layout;
- root-level History semantic flow;
- native receipt-driven scrollback;
- ContentHost provider rows;
- current TypeScript History façade.

No current-source finding in this report treats V5 proposals as requirements or labels current History retention as a defect merely because a V5 table says it may eventually disappear.

---

## 9. Open questions and coverage gaps

1. **What exactly did PERF-13-H mean by “old History stream units”?**  
   Current `HistoryUnit` survives and is central to the semantic model. H does not identify the deleted files/types. The current source proves old stream scheduler/pane/transfer paths are absent, but not whether the phrase was intended to include or exclude the surviving generic `HistoryUnit`.

2. **Who owns the immutable historical descriptor required by resolved PERF-13 §14.5?**  
   Current `History` stores a ContentPort-bearing View and unit identity. The provider owns Source-revision-keyed committed projections and finalized-prefix products. The implementation may be compliant, but the handoff language should be clarified to say whether provider-owned immutable products satisfy “History captures.”

3. **Does a ContentHost-backed History unit remain dynamically connected until all stable rows transfer?**  
   Current `history_rows` uses the active Connector/provider and tracks committed row counts while open/Smooth content advances. This is supported by G and L1 tests, but differs from a naïve reading of “historical” as immediately frozen. The source behavior is clear; the historical prose is not.

4. **Is `History::push`’s always-`Ok` result intentional API stability or an unfinished validation contract?**  
   `History::push` currently returns `Result<HistoryUnitId, HistoryError>` but the model implementation returns `Ok` after classification/allocation. Host-level attachment validation can fail before the mutation. This is documented in the current atlas map but not resolved by the historical records.

5. **Rust public-surface terminology needs precision.**  
   Current `history/mod.rs` declares public types, but the crate root exposes the module as `pub(crate) mod history` and re-exports selected types as `pub(crate)`. TypeScript is the supported external API. Documentation calling these symbols simply “public Rust API” risks reviving the obsolete Rust authoring contract that L1-13 removed.

6. **Historical test/benchmark totals are not directly comparable.**  
   Every record needs SHA, artifact hash, feature profile, platform, and workload before comparison. L1-00 baseline/fix, L1-11, L1-12, L1-13 checkpoint, final review, and post-cleanup qualification intentionally describe different snapshots.

7. **No runtime execution was performed for this report.**  
   Current behavior findings are static source reconstruction. Historical tests and benchmark outputs were not rerun.

8. **The current atlas History map is parent-added documentation outside the source baseline.**  
   `docs/architecture/atlas-4355c02/subsystems/rust/07-history.md` is useful as a cross-check and source manifest, but current source—not that map—is authoritative for behavior.

---

## 10. Evidence appendix

### 10.1 Required contract/context files read

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/README.md`
- `docs/history/README.md`

### 10.2 Approved historical authority files read for chronology

#### API-H

- `docs/history/API-H1/API-H1-V2-public-api-hygiene.md`
- `docs/history/API-H2/API-H2-STRUCT-1-HANDOFF-v2.md`
- `docs/history/API-H2/API-H2-CUT-0-baseline.md`
- `docs/history/API-H2/API-H2-CUT-1-completion.md`
- `docs/history/API-H2/API-H2-CUT-2-completion.md`
- `docs/history/API-H2/API-H2-CUT-3-completion.md`
- `docs/history/API-H2/API-H2-CUT-4-completion.md`
- `docs/history/API-H2/API-H2-CUT-5-completion.md`
- `docs/history/API-H2/API-H2-CUT-0-5-audit.md`
- `docs/history/API-H3/API-H3-A-completion.md`
- `docs/history/API-H3/API-H3-B-completion.md`
- `docs/history/API-H3/API-H3-C-completion.md`
- `docs/history/API-H3/API-H3-D-completion.md`
- `docs/history/API-H3/API-H3-E-completion.md`
- `docs/history/API-H3/API-H3-COMPOSITION-TRANSPORT-SEAM-HANDOFF.md`

#### PERF-13

- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`
- `docs/history/PERF-13/PERF-13-A-completion.md`
- `docs/history/PERF-13/PERF-13-B-completion.md`
- `docs/history/PERF-13/PERF-13-C-completion.md`
- `docs/history/PERF-13/PERF-13-D-completion.md`
- `docs/history/PERF-13/PERF-13-E-completion.md`
- `docs/history/PERF-13/PERF-13-F-completion.md`
- `docs/history/PERF-13/PERF-13-F-implementation-notes.md`
- `docs/history/PERF-13/PERF-13-G-completion.md`
- `docs/history/PERF-13/PERF-13-G-implementation-notes.md`
- `docs/history/PERF-13/PERF-13-H-completion.md`

#### PRE-V5

- `docs/history/PRE-V5/POST-PERF13-ROT-CLEANUP.md`
- `docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md`

### 10.3 L1 records read

- `reports/pre-v5-l1/L1-00-baseline.md`
- `reports/pre-v5-l1/L1-00b-tranche-report.md`
- `reports/pre-v5-l1/L1-00c-tranche-report.md`
- `reports/pre-v5-l1/L1-00d-fix-note.md`
- `reports/pre-v5-l1/L1-00-closure.md`
- `reports/pre-v5-l1/L1-01-ledger.md`
- `reports/pre-v5-l1/L1-01-report.md`
- `reports/pre-v5-l1/L1-02-report.md`
- `reports/pre-v5-l1/L1-03-report.md`
- `reports/pre-v5-l1/L1-04-report.md`
- `reports/pre-v5-l1/L1-05-report.md`
- `reports/pre-v5-l1/L1-06-report.md`
- `reports/pre-v5-l1/L1-07-report.md`
- `reports/pre-v5-l1/L1-08-report.md`
- `reports/pre-v5-l1/L1-09-report.md`
- `reports/pre-v5-l1/L1-10-report.md`
- `reports/pre-v5-l1/L1-11-report.md`
- `reports/pre-v5-l1/L1-12-report.md`
- `reports/pre-v5-l1/L1-13-checkpoint.md`
- `reports/pre-v5-l1/final-implementation-review.md`
- `reports/pre-v5-l1/post-cleanup-qualification.md`

Machine-readable L1 artifacts were indexed for provenance and counter names but not treated as narrative authority:

- `reports/pre-v5-l1/trace-default.json`
- `reports/pre-v5-l1/trace-fixed.json`
- `reports/pre-v5-l1/trace-perf-counters.json`
- `reports/pre-v5-l1/t15-route-smoke.json`
- `reports/pre-v5-l1/perf13-h-content.json`
- `reports/pre-v5-l1/post-cleanup-qualification-evidence.json`

### 10.4 Current source files inspected

#### History

- `crates/iyon-tui/src/history/mod.rs`
- `crates/iyon-tui/src/history/boundary.rs`
- `crates/iyon-tui/src/history/error.rs`
- `crates/iyon-tui/src/history/id.rs`
- `crates/iyon-tui/src/history/layout.rs`
- `crates/iyon-tui/src/history/model.rs`
- `crates/iyon-tui/src/history/unit.rs`
- `crates/iyon-tui/src/history/native/frontier.rs`
- `crates/iyon-tui/src/history/native/mod.rs`
- `crates/iyon-tui/src/history/projection/mod.rs`
- `crates/iyon-tui/src/history/trace.rs`

#### Host/content/scene/backend seams

- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/scene/root_tests.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/backend/native_history.rs`
- `crates/iyon-tui/src/presentation/content.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/stream/mod.rs`
- `crates/iyon-tui/src/stream/coord.rs`
- `crates/iyon-tui/src/lib.rs`

#### Native and TypeScript seams

- `crates/iyon-tui-native/src/tui.rs`
- `packages/iyon-tui/src/api/controls/history.ts`
- `packages/iyon-tui/src/transport/native/addon.ts`
- `packages/iyon-tui/src/transport/native/factories.ts`
- `packages/iyon-tui/src/runtime/runtime.ts`
- `packages/iyon-tui/scripts/stage-native.ts`
- `packages/iyon-tui/src/api/view/scene.ts`
- `packages/iyon-tui/src/testing/index.ts`

#### Current tests

- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
- `packages/iyon-tui/tests/tui_handles.test.ts`
- `packages/iyon-tui/tests/tui_harness.test.ts`
- `packages/iyon-tui/tests/fixtures/tui_demo.ts`
- `crates/iyon-tui/src/scene/root_tests.rs`
- History/content tests embedded in `history/trace.rs` and `application/content.rs`

### 10.5 Current atlas cross-check

- `docs/architecture/atlas-4355c02/subsystems/rust/07-history.md`

This parent-added document was not treated as source authority. It was used as a cross-check for the recursive History inventory, current source manifest, and already-recorded source paths.

### 10.6 Investigation searches

Read-only repository searches covered:

```text
History
history
pushStream
sealStream
HostTextStream
NativeTextStream
StreamPane
stream scheduler
stream transfer
nativeObject
pushRef
freezeRef
discardLive
history_rows
history_rows_committed
history_unit_retired
finalized_prefix
stable_rows
synchronization_unknown
transfer_native_prefix
HistoryUnit
HistoryLayout
FlowBoundary
HistoryError
```

Search scopes were:

```text
crates/iyon-tui/src/**
crates/iyon-tui-native/src/**
packages/iyon-tui/src/**
packages/iyon-tui/scripts/**
packages/iyon-tui/tests/**
docs/history/**
reports/pre-v5-l1/**
docs/architecture/**
```

### 10.7 LOC methodology

- Current recursive History LOC is the approximate physical count recorded by the current atlas map: source line extents including comments, blanks, and test-gated helper code.
- No generated code is included in the History recursive count.
- Cross-cutting adjacent files were not assigned artificial ownership LOC because `application/content.rs`, `scene/host.rs`, `application/host.rs`, and native/TypeScript runtime files have substantial non-History responsibilities.
- Historical report LOC and machine-readable artifact sizes were not used as production-size evidence.
- No runtime validation was executed for this report.