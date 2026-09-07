# 37 — TS History consumers through Rust and terminal output

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Parent-added atlas documentation is outside the source baseline.
- Investigation was read-only. No source/configuration files were edited.
- I did not run tests, builds, benchmarks, formatters, native services, or a real terminal session. Test references below describe test source and asserted behavior, not execution during this investigation.

The report contract, atlas README, `AGENTS.md`, and `PRE-V5-ARCHITECTURE-REPORT.md` were read for boundaries and evidence expectations. The History-specific context in the pre-V5 report was treated as historical guidance only; current source is authoritative for implementation behavior.

### Primary scope

This assignment follows TypeScript `History` handles through the native N-API wrapper, Rust host and retained Scene layers, History projection/native-frontier logic, and terminal output. The emphasis is:

- insertion and replacement;
- static versus live unit classification;
- freeze/discard transitions;
- ContentHost-backed streaming History;
- visibility and viewport selection;
- native scrollback transfer;
- partial transfer and sink failure behavior;
- resize behavior;
- caching, invalidation, and frame scheduling;
- actual TypeScript consumers/examples present in this repository.

### Evidence classification

- **Source fact:** directly represented by current source symbols and paths.
- **Static inference:** behavior reconstructed from call chains and state transitions.
- **Historical/documentary claim:** stated by a document such as `LAY-1-main-screen-scrollback-and-resize.md`; not treated as source authority.
- **Observed execution:** none during this investigation.

### High-level finding

Current History is not one undifferentiated scroll widget. It consists of three cooperating layers:

1. **Semantic History model** — an ordered `VecDeque<HistoryUnit>` containing static or live `View`s, stable unit IDs, flow boundaries, semantic layout, semantic revisions, and per-unit height-cache state.
2. **History projection** — a width-dependent retained `View` projection that computes flow geometry, visibility, end-follow or native-frontier anchoring, live clipping, and an optional physical-row overlay.
3. **Native frontier** — an irreversible prefix-transfer state machine that sends final `PhysicalRow`s to a backend-neutral sink, tracks partial physical remainders, retires completed semantic units, and records physical synchronization uncertainty.

The TypeScript API exposes only a small handle surface: `layout`, `setLayout`, `push`, `freeze`, and `discardLive`. It does not expose History scrolling, arbitrary viewport offsets, `FlowBoundary`, or native transfer controls.

The most consequential current limitation is resize after native scrollback has been populated. The terminal presenter can invalidate and repaint the visible surface, but current source does not clear/replay native scrollback or regenerate already-transferred physical rows at the new width. The repository’s own `LAY-1-main-screen-scrollback-and-resize.md` explicitly identifies this as a current design problem. This is a source/document contradiction only in the sense that the research record describes the deficiency; the current implementation does not contain the proposed repair.

---

## 1. Responsibility and structure

### 1.1 Primary History module inventory

The recursive History directory contains eleven Rust source files:

```text
crates/iyon-tui/src/history/
├── boundary.rs
├── error.rs
├── id.rs
├── layout.rs
├── mod.rs
├── model.rs
├── native/
│   ├── frontier.rs
│   └── mod.rs
├── projection/
│   └── mod.rs
├── trace.rs
└── unit.rs
```

Approximate physical source LOC, including comments, blank lines, private helpers, and test-gated code where present:

| File | Approx. physical LOC | Primary responsibility | Public surface |
|---|---:|---|---|
| `history/boundary.rs` | 12 | Unit-to-predecessor flow relationship | `FlowBoundary` |
| `history/error.rs` | 34 | History invariant/lifecycle errors | `HistoryError` |
| `history/id.rs` | 38 | Process-wide opaque unit identity | `HistoryUnitId` |
| `history/layout.rs` | 48 | Semantic padding and gap configuration | `HistoryLayout` |
| `history/mod.rs` | 34 | Module assembly and selective re-exports | Public History types through crate binding |
| `history/model.rs` | 407 | Ordered units, semantic lifecycle, revisions, attachment enumeration, height caches | `History` |
| `history/unit.rs` | 48 | Internal unit representation and height-cache keys | Crate-private |
| `history/native/frontier.rs` | 104 | Native physical frontier, spacing state, frozen remainders, sync marker | Crate-private |
| `history/native/mod.rs` | 559 | Native prefix transfer state machine and sink/error handling | Crate-private |
| `history/projection/mod.rs` | ≈1,100 | Width-dependent projection, visibility, anchor selection, physical overlay | Crate-private |
| `history/trace.rs` | 141 | Environment-gated diagnostic tracing | Crate-private |
| **Total** | **≈2,525** | — | — |

The count is a physical-line approximation from source extents, not a semantic statement about implementation size. There is no generated code in the recursive History directory.

### 1.2 `History` semantic model

`crates/iyon-tui/src/history/model.rs:22-407` defines `History`.

It owns:

- ordered semantic unit storage in `VecDeque<HistoryUnit>`;
- stable History-object identity;
- semantic revision;
- separate native display-frontier revision;
- semantic `HistoryLayout`;
- per-unit width/key/height caches;
- private `NativeFrontier`;
- semantic insertion and lifecycle operations;
- state/content attachment enumeration used for host validation;
- native synchronization uncertainty state.

It does **not** own:

- terminal writes;
- a terminal-specific protocol;
- arbitrary user scroll offsets;
- a separate stream/source store;
- ContentPort or Connector storage;
- the component registry;
- the terminal’s actual native scrollback tape.

### 1.3 Projection responsibilities

`crates/iyon-tui/src/history/projection/mod.rs:157-1,099` owns the History-specific presentation envelope:

- collect ordered semantic units;
- create `UnitPlan`s;
- derive width-dependent content width;
- obtain provider-owned resident History views;
- resolve live component views and collect dependency revisions;
- compute per-unit height;
- construct top padding, gaps, units, and bottom padding as flow items;
- select visible suffix for `FollowEnd`;
- select visible prefix for `NativeFrontier`;
- bound flexible live views to the remaining viewport;
- produce an ordinary retained vertical `View`;
- produce an optional `HistoryPhysicalOverlay` for exact physical native remainders;
- report overflow rows to the host;
- emit structured projection traces.

Projection may mutate History’s interior-mutable height-cache records through `prepare_unit_layout` and `record_unit_height`, but does not semantically append, freeze, discard, or acknowledge native rows.

### 1.4 Native frontier responsibilities

`history/native/frontier.rs` and `history/native/mod.rs` own:

- transfer ordering;
- exact prefix acknowledgement;
- partial physical-row remainder retention;
- spacing rows that are semantic, frozen, or already native;
- front-unit retirement;
- native physical row counters;
- native synchronization-unknown state;
- ContentProvider callbacks for committed rows and retired units.

The native frontier is backend-neutral. It accepts a `NativeHistorySink`; it does not know whether the sink is a headless vector, a Termwiz terminal worker, or another backend.

### 1.5 Supporting consumer modules

The principal supporting production seams are:

| Path | Approx. full-file scale | History-relevant responsibility |
|---|---:|---|
| `crates/iyon-tui/src/scene/root.rs` | ≈440 | Optional root-level History above ordinary body; root projection and merge |
| `crates/iyon-tui/src/scene/host.rs` | ≈4,243 | Resolve, native-pressure drain, retained/incremental History refresh, paint |
| `crates/iyon-tui/src/application/host.rs` | ≈4,243 | N-API-facing `HostHistory`, host-owned lifecycle, headless/real backend selection |
| `crates/iyon-tui/src/application/content.rs` | ≈9,000 | ContentPort/Connector state and ContentHost History rows |
| `crates/iyon-tui/src/terminal/termwiz/backend.rs` | 166 | Termwiz backend implementation of `NativeHistorySink` |
| `crates/iyon-tui/src/terminal/termwiz/worker.rs` | ≈150 | Terminal command dispatch, including `InsertHistory` |
| `crates/iyon-tui/src/terminal/termwiz/presenter.rs` | ≈1,392 | Physical Surface diffing and native CRLF transfer |
| `crates/iyon-tui/src/terminal/termwiz/lower.rs` | ≈500 | Physical rows/surface to Termwiz `Change`s |
| `crates/iyon-tui-native/src/tui.rs` | ≈1,090 | N-API `NativeHistory`, `NativeTuiHost`, resize/readback and handle forwarding |

The supporting LOC figures are approximate full-file physical sizes and are included to identify architectural scale, not to assign ownership to this History report.

### 1.6 TypeScript History surface

`packages/iyon-tui/src/api/controls/history.ts:9-132` is a small production facade. Its public interface is:

```text
layout(): HistoryLayout
push(view: View): number
freeze(unit: number, view: View): void
discardLive(unit: number): void
setLayout(layout: HistoryLayout): void
```

The TypeScript `HistoryLayout` differs from the Rust layout shape:

```ts
interface HistoryLayout {
  readonly padding: number;
  readonly gap: number;
}
```

The N-API layer lowers this scalar `padding` to bottom-only Rust insets:

```rust
HistoryLayout::from_parts(Insets::new(0, 0, padding, 0), gap)
```

Conversely, `layout()` reports `layout.padding().bottom()`. Thus, TypeScript does not expose Rust’s full `Insets` value or left/right/top padding. The TypeScript API’s `padding` is effectively vertical bottom padding in the current binding.

---

## 2. Types, APIs and contracts

### 2.1 Public semantic Rust types

#### `FlowBoundary`

`history/boundary.rs:3-12`:

```rust
pub enum FlowBoundary {
    Default,
    AttachToPrevious,
}
```

- `Default` allows the configured predecessor gap.
- `AttachToPrevious` suppresses that gap for the unit.
- The boundary belongs to the following unit, not the predecessor.
- There is no arbitrary per-unit gap.
- The enum is `#[non_exhaustive]`.

`FlowBoundary` is used by `History::push_with_boundary`, but the repository-wide source search found no production caller other than `History::push` itself. The TypeScript facade does not expose `push_with_boundary` or `FlowBoundary`.

#### `HistoryError`

`history/error.rs:5-32`:

```rust
pub enum HistoryError {
    UnitNotFound { unit: HistoryUnitId },
    UnitNotLive { unit: HistoryUnitId },
    LiveMustRemainTail { unit: HistoryUnitId },
    FinalViewContainsComponent { unit: HistoryUnitId },
}
```

Contracts:

- unknown unit IDs are rejected;
- freeze/discard require a live unit;
- discard additionally requires the live unit to be the semantic tail;
- a frozen final view may not contain component identity.

`HistoryError` is non-exhaustive, cloneable, equality-comparable, implements `Display` and `Error`.

#### `HistoryUnitId`

`history/id.rs:9-38` wraps `NonZeroU64`.

- IDs are process-wide monotonically allocated.
- `value()` exposes the numeric value.
- `from_value` is internal and rejects zero.
- IDs are used for unit identity, native retirement callbacks, and ContentHost association.

The ID remains valid only while the semantic unit remains in the History deque. Native retirement callbacks carry the ID before associated ContentHost resources are removed.

#### `HistoryLayout`

`history/layout.rs:3-48` contains:

- `Insets padding`;
- `u16 gap`.

The Rust API provides:

- `new`;
- `from_parts`;
- `with_padding`;
- `with_gap`;
- `padding`;
- `gap`.

Layout changes are semantic and invalidate all unit height caches. Native physical rows already transferred are not regenerated when layout changes.

### 2.2 `History` API behavior

#### Creation

`History::new` (`model.rs:49-61`) creates:

- empty `VecDeque`;
- a unique History identity;
- default zero padding/gap;
- no cached total height;
- semantic and native revisions at zero;
- default NativeFrontier.

#### `push`

`History::push` delegates to `push_with_boundary(Default)` (`model.rs:72-74`).

`push_with_boundary` (`model.rs:76-97`):

1. Checks whether the `View` contains component identity.
2. Stores component-bearing views as `HistoryUnitContent::Live`.
3. Stores all other views as `HistoryUnitContent::Static`.
4. Allocates `HistoryUnitId`.
5. Clears aggregate height cache and stale-height count.
6. Appends to the back of `VecDeque`.
7. Increments semantic revision.
8. Returns the unit ID.

A ContentHost-bearing view without component identity is therefore `Static`, even though its provider-backed projection can change as stream data arrives.

The current Rust method returns `Result<HistoryUnitId, HistoryError>`, but the implementation’s push path does not currently produce a HistoryError after classification and ID allocation. Host-level attachment validation can fail before the underlying History mutation.

#### `discard_live`

`model.rs:99-114`:

1. Resolve the ID.
2. Require the unit to be the deque tail.
3. Require `HistoryUnitContent::Live`.
4. Remove it from the deque.
5. Clear aggregate height state.
6. Increment semantic revision.

This is explicitly a transient-tail operation. It:

- does not insert native rows;
- does not create a retirement callback;
- does not transfer or preserve a physical remainder.

The host wrapper separately clears any ContentHost association and refreshes desired state bindings.

#### `freeze`

`model.rs:116-127`:

1. Resolve the ID.
2. Require the existing unit to be live.
3. Reject a final view containing component identity.
4. Replace the content with `Static(final_view)`.
5. Invalidate that unit’s layout cache.
6. Increment semantic revision.

Freeze does not itself transfer rows to native scrollback and does not retire the unit. A frozen unit remains in the semantic deque until native transfer reaches and fully accepts it.

A final static view may contain a ContentPort identity. The host layer performs prospective ContentHost validation around this replacement.

There is no separate generic “replace live view” API. In current TypeScript and host usage, replacement is represented by `freeze(unit, finalView)`.

#### `set_layout`

`model.rs:290-303`:

- equal layout is a no-op;
- changed layout replaces the semantic layout;
- all unit layout caches are invalidated;
- aggregate height state is cleared;
- semantic revision increments.

Native physical rows and frozen physical remainders are not regenerated. This is correct for preserving exact already-accepted terminal rows, but it means layout changes after partial native transfer can leave physical and semantic geometry from different configurations.

### 2.3 Internal layout-cache contract

`history/unit.rs:12-48` defines:

```rust
enum HistoryUnitLayoutKey {
    Static(ViewId),
    Content { view: ViewId, projection: u64 },
    Live {
        view: ViewId,
        dependencies: Vec<(ComponentId, ComponentRevision)>,
    },
}
```

A cache entry is valid only when:

1. width matches;
2. the semantic/provider/component dependency key matches.

Key behavior:

- component-free static views use `ViewId`;
- ContentHost static views include provider projection revision;
- live views include all reachable component revisions.

`HistoryUnitLayout` stores width, key, and height inside a `RefCell`, because projection receives `&History` but records derived measurement state.

### 2.4 Attachment-enumeration contracts

`model.rs:145-247` supplies host-validation helpers.

`state_views` returns cloned History views that:

- carry retained state attachment; or
- contain component identity.

`content_views` returns cloned History views that:

- carry ContentPort identity; or
- contain component identity.

Prospective replacement methods allow host validation before semantic mutation:

- `state_views_with_replacement` requires a live target and rejects component-bearing final replacement;
- `content_views_with_replacement` requires a live target and includes replacement content/component identities but does not itself reject component identity.

This allows `HostHistory::freeze` to validate the complete body + History attachment set before replacing the live unit.

### 2.5 Native sink contract

`crates/iyon-tui/src/backend/native_history.rs:5-14`:

```rust
trait NativeHistorySink {
    type Error;

    fn insert_history_rows(
        &mut self,
        rows: &[PhysicalRow],
    ) -> Result<usize, Self::Error>;
}
```

The documented contract is:

- `Ok(k)` means exactly `rows[..k]` entered native history;
- no later row entered;
- `Err` means zero rows were accepted for that call.

The History transfer code is more defensive than the nominal contract: any sink error or invalid acknowledgement marks physical synchronization unknown, because a real backend may have partially written before reporting failure.

### 2.6 TypeScript-to-native contract

`packages/iyon-tui/src/transport/native/addon.ts:18-26`:

```ts
interface NativeHistoryContract {
  dispose(): void;
  layout(): object;
  setLayout(layout: object): void;
  isDetached(): boolean;
  pushRef(viewRef: number): number;
  freezeRef(unit: number, viewRef: number): void;
  discardLive(unit: number): void;
}
```

Important properties:

- TS does not send serialized Views for History operations.
- `push` and `freeze` materialize a retained View reference through the structural View ABI.
- The temporary native View reference is released in a `finally` block after the operation.
- Rust History retains its own `View` clone/state after accepting the operation.
- The TS facade validates unit numbers as positive safe integers before native calls for freeze/discard.
- closed host-bound Histories are rejected by `History.callHost`.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency diagram

```text
TypeScript caller
    │
    ├─ new History()
    │      │
    │      └─ nativeTui.history()
    │
    ├─ Tui.createHistory()
    │      │
    │      └─ host.history()
    │
    └─ history.push/freeze/discard/setLayout
           │
           ▼
TypeScript History facade
    │
    ├─ retained View materialization / View ABI ref
    └─ NativeHistoryContract
           │
           ▼
iyon-tui-native::NativeHistory
    │
    ├─ detached: Mutex<History>
    └─ attached: HostHistory
           │
           ▼
application::HostHistory
    │
    ├─ host attachment validation
    ├─ ContentHostRegistry bindings
    ├─ retained-state binding validation
    └─ SceneHost invalidation + render
           │
           ▼
application::TuiHost / SceneHost
    │
    ├─ root Scene with optional History + body
    ├─ resolve body
    ├─ project History
    ├─ drain native pressure
    ├─ select FollowEnd / NativeFrontier
    └─ paint PreparedSceneFrame
           │
           ├─ History semantic projection
           │      ├─ HistoryUnit
           │      ├─ ContentProvider
           │      ├─ ComponentRegistry
           │      └─ layout/measurement
           │
           └─ NativeFrontier
                  │
                  └─ NativeHistorySink
                         │
                         ├─ HeadlessSink → Vec<PhysicalRow>
                         └─ TermwizBackend
                                │
                                └─ terminal worker
                                       │
                                       └─ TermwizPresenter
                                              │
                                              └─ CRLF native scrollback transaction
```

### 3.2 Reverse dependency map

Major consumers:

- `Scene` stores one optional root-level History (`scene/root.rs:24-98`).
- Rust `App` can own persistent root History (`application/app.rs` and `application/context.rs`).
- `AppCx::history` / `history_mut` expose History to Rust application code.
- `HostHistory` provides the N-API-facing attached mutation operations (`application/host.rs:818-947`).
- `SceneHost` resolves, paints, and transfers History (`scene/host.rs:55-127`, `1010-1134`).
- `ContentHostRegistry` implements `ContentProvider` History callbacks (`application/content.rs:6242-6331`, `6334-6684`).
- `NativeHistorySink` is implemented by headless/test sinks and `TermwizBackend`.
- `NativeHistory` forwards N-API operations to detached `History` state or attached `HostHistory`.
- TS `History` handles are used by public tests and the in-tree consumer fixture.

### 3.3 Ownership and lifetime

| Resource | Created by | Owner | Released/retired by |
|---|---|---|---|
| History object | TS `new History`, Tui host factory, Rust `History::new` | TS native handle or Rust Scene/App | Handle disposal, host teardown, Rust ownership drop |
| Semantic unit | `History::push` / `HostHistory::push` | `History.units` | `discard_live` or native `retire_front` |
| Unit ID | `HistoryUnitId::allocate` | Unit + callbacks | Becomes invalid after unit removal |
| Live component identity | Caller View plus ComponentRegistry | Scene/History retained graph | Component retirement/reconciliation |
| ContentPort association | `HostHistory::push` / `freeze` | ContentHostRegistry history adapter | `history_unit_retired` or explicit clear on replacement/discard |
| Per-unit measured height | Projection via `History` cache | `HistoryUnitLayout` | Width/key/layout/content revision invalidation |
| Frozen physical remainder | Partial native sink acknowledgement | `NativeFrontier` | Later prefix acknowledgement or unit retirement |
| Native terminal rows | Termwiz terminal | Terminal emulator | Terminal/scrollback lifecycle, not addressable by Iyon |
| Headless native rows | HeadlessSink | `HostBackend::Headless` | Host/backend teardown |

### 3.4 History is root-level, not an ordinary nested View

`Scene` stores:

```rust
history: Option<History>,
body: View,
```

`Scene::with_history` marks History as a root sideband. `root_view` constructs a two-child column:

1. History with a flexible track;
2. body with a content track.

History cannot be nested inside arbitrary ordinary composition through `Scene`; it is a special root-level semantic capability.

`merge_root_scene` first merges History mounts and then body mounts. Component identities must be disjoint. History therefore participates in the same retained component graph, but its projection is a separate resolution domain.

### 3.5 Ownership boundary finding

The ownership split is generally explicit:

- semantic unit order/content: `History`;
- retained component instances: `ComponentRegistry` and `SceneHost`;
- stream/source/projection state: `ContentHostRegistry`;
- physical terminal writes: backend/presenter;
- TS lifetime/attachment policy: TS runtime plus native host wrapper.

The major coupling is that History stores `View`, and live History Views resolve through the same component registry as the body. History is generic terminal framework machinery, but it is not independent of the retained View and component-runtime architecture.

---

## 4. Execution paths and state transitions

### 4.1 TypeScript detached creation and append

`History` constructor (`packages/iyon-tui/src/api/controls/history.ts:44-56`) has two paths:

#### Caller-created detached History

```text
new History()
  → nativeTui.history()
  → NativeHistory::new
  → Mutex<History::new>(), host: None
```

A detached History can:

- push;
- inspect/set layout;
- remain alive without a Tui.

The native wrapper rejects detached freeze and discard because those operations require a host.

#### Host-created History

```text
tui.createHistory()
  → Tui::createHistory()
  → host.history()
  → createHistoryHandle(native HostHistory)
  → host-bound History handle
```

`NativeTuiHost::history` returns a `NativeHistory` wrapper around the host’s existing History (`iyon-tui-native/src/tui.rs:830-836`). The host’s History is already attached; it is not transferred through `setHistory`.

### 4.2 Detached History attachment

A detached History included in a rendered `Scene` follows the TS runtime sideband path:

```text
tui.render(scene)
  → Scene.from
  → validate body and History handle
  → stage effective History
  → prepare root publication
  → commit History binding before body publication
  → nativeObj.isDetached()
  → host.setHistory(nativeObj)
  → NativeHistory::take_for_host
  → HostHistory becomes the active root History
```

Relevant TS runtime symbols:

- `runtime.ts:280-294` — `commitHistoryBinding`;
- `runtime.ts:296-317` — `stageHistoryBinding`;
- `runtime.ts:499-548` — direct render path;
- `runtime.ts:563-569` — host-owned `createHistory`.

The runtime enforces attach-once behavior:

- a History can transfer from detached to one Tui;
- it cannot transfer to another Tui after attachment;
- a different History instance cannot replace an already-bound one;
- rendering without a new History continues using the previously bound History;
- producer/prepare failures restore staged sideband state;
- a successful publication swaps the binding at commit.

`NativeTuiHost::set_history` (`iyon-tui-native/src/tui.rs:783-804`) itself:

1. rejects a History whose native host is already set;
2. locks the detached semantic state;
3. asks the host to validate History state attachments;
4. takes the semantic `History` out of the detached wrapper;
5. installs it in the host;
6. stores a host-side handle in the original native wrapper.

The transfer is intended as one-way ownership. If a later host operation fails after `take_for_host`, source does not provide a general rollback of this transfer.

### 4.3 TypeScript `push`

`History.push` (`history.ts:68-77`) does:

```text
History.callHost
  → tryRetainedMaterializeRef(view)
  → native.pushRef(ref)
  → finally releaseNativeViewRef(...)
```

A materialization refusal is explicit:

```text
HISTORY_PUSH_FAILED: View could not be materialized
```

The native View reference is temporary. Rust/native History stores its own retained View representation.

For a detached handle:

```text
NativeHistory::push_ref
  → resolve_native_view
  → NativeHistory::push_view
  → Mutex<History>::push
```

For an attached handle:

```text
NativeHistory::push_ref
  → resolve_native_view
  → NativeHistory::push_view
  → HostHistory::push
```

### 4.4 Attached `HostHistory::push`

`application/host.rs:848-874` performs the attached operation:

1. Lock `HostInner`.
2. Capture the current body.
3. Compute prospective state attachment targets for body + new History View.
4. Validate state targets.
5. Compute prospective ContentHost attachment targets.
6. Validate ContentHost targets.
7. Append the View to semantic History.
8. If the View has a ContentPort, bind it to the returned unit ID and record original padding.
9. Set desired state bindings.
10. Set desired ContentHost bindings.
11. Invalidate the frame.
12. Advance/render.
13. Return the unit ID.

This is transactional-looking but not a full rollback transaction. Validation precedes the semantic mutation, which prevents most known invalid attachment cases. However, if a later binding update or rendering step fails after `History::push`, the source does not show semantic History rollback.

### 4.5 Live classification and updates

A pushed `View` containing component identity becomes `HistoryUnitContent::Live`. A typical TS route is:

```text
slot.view()
  → View containing retained component identity
  → history.push(slot.view())
  → HistoryUnitContent::Live
```

During projection (`projection/mod.rs:190-203`):

1. `ResolveSession::resolve_root_with_dependencies(view)` resolves the component subtree.
2. The resolved View is retained in `PlannedContent::Live`.
3. All reachable component IDs/revisions form `HistoryUnitLayoutKey::Live`.
4. The height cache is reused only if width and every dependency revision still match.
5. A slot or component mutation changes its revision and invalidates the host.
6. The next projection re-resolves the live unit and remeasures it if the dependency key changed.

The live unit itself remains in semantic History until:

- it is frozen;
- it is discarded as a live tail;
- it eventually becomes a static/final unit and is later natively retired.

A live unit at the front blocks native promotion. Native transfer is deliberately not allowed to skip it in order to reach later static units.

### 4.6 Freeze/replacement through TypeScript and host

`History.freeze` (`history.ts:80-93`) follows the same retained View materialization path as push, then calls `freezeRef`.

Attached path (`application/host.rs:877-918`):

1. Validate nonzero unit ID.
2. Obtain prospective state-bearing History Views with the replacement.
3. Validate state targets across body + all History state-bearing Views.
4. Obtain prospective content-bearing History Views with the replacement.
5. Validate ContentHost targets.
6. Mutate the semantic unit with `History::freeze`.
7. Clear the previous History-unit ContentHost association.
8. Bind replacement ContentPort if present.
9. Set desired state bindings.
10. Set desired ContentHost bindings.
11. Invalidate and render.

The previous live View is replaced semantically. If the replacement is plain static content, it becomes `Static`. If it has ContentPort identity but no component identity, it becomes static provider-backed content. A replacement containing component identity is rejected as a final frozen View.

Detached path:

```text
NativeHistory::freeze_ref
  → resolve view
  → NativeHistory::freeze_view
  → host == None
  → "detached history cannot freeze a unit"
```

Thus, TS documentation and native implementation agree that freeze requires an attached History.

### 4.7 Discard transition

`History.discardLive` (`history.ts:95-100`) validates a positive safe integer and calls native `discardLive`.

Attached path (`application/host.rs:921-935`):

```text
validate ID
  → History::discard_live
  → ContentHostRegistry::clear_history_unit
  → refresh desired state bindings
  → invalidate frame
  → advance/render
```

Discard is limited to the semantic tail. It does not transfer native rows, does not create a native retirement callback, and does not preserve a physical remainder.

Detached discard is rejected by `NativeHistory::discard_live` with:

```text
detached history cannot discard a unit
```

### 4.8 ContentHost-backed History and live/finalized streaming

A ContentPort-bearing static View follows a separate content path. It remains semantically static in History, but its provider projection can change as the Source/Connector changes.

`ContentProvider` (`presentation/content.rs:137-221`) defines:

- `HistoryContentRows`;
- `projection_revision`;
- `measure`;
- `paint_window`;
- `history_rows`;
- `history_rows_committed`;
- `history_view`;
- `history_unit_retired`;
- `history_transfer_blocked`.

`HistoryContentRows` contains:

- physical rows;
- `complete`;
- content-row start/end within the payload;
- leading/trailing padding counts.

`ContentHostRegistry::history_rows` (`application/content.rs:6242-6331`) deliberately uses a finalized/stable prefix rather than slicing arbitrary open projection rows:

1. Find the current visible or desired Connector.
2. Read the History adapter’s committed row count and original insets.
3. Obtain the Source snapshot and sealing state.
4. Obtain the Connector projection.
5. Read the finalized-prefix product.
6. Limit transferable content to `projection.stable_rows`.
7. Treat the payload as complete only when the Source is sealed and the stable prefix reaches the finalized end.
8. Include bottom padding only when complete.
9. Begin the payload after already committed rows.
10. Place content rows at the offered width and left inset.
11. Return exact content/padding ranges.

Consequences:

- open streaming content can export a nonempty stable prefix while remaining incomplete;
- an open unstable tail is not transferred;
- sealing alone is insufficient if smoothing/delivery still has backlog;
- a zero-row incomplete response blocks native transfer;
- a zero-row complete response allows unit retirement;
- accepted row counts are fed back to the ContentHost only after the native sink acknowledges them.

`history_view` strips decoration already transferred to native scrollback. This avoids replaying the same top/bottom padding in the resident History view while retaining the caller’s original body occurrence unchanged.

### 4.9 Root resolution and visibility

`scene/root.rs:196-260` resolves the body and History separately:

1. Resolve body branch.
2. Measure body at terminal width.
3. Clamp body height to terminal height.
4. Allocate remaining height to History.
5. Project History with that remaining height.
6. Finish the History resolution session.
7. Merge History before body.
8. Return History metadata including height and overflow rows.

The root layout (`root.rs:416-440`) has:

```text
History: flexible track, min 0
Body:    content track
```

Therefore:

- History is above the body;
- body is pinned below History;
- History receives all remaining space after intrinsic body height;
- if body consumes the whole terminal, History receives height zero;
- live History components are still resolved/mounted even when their visible track has zero height.

`scene/root_tests.rs` covers:

- body-only roots;
- remaining-height allocation;
- History above body;
- full terminal width for both branches;
- zero-height History with live components still mounted;
- duplicate component rejection across History/body;
- History-first mount ordering;
- frozen overlays remaining inside the History track.

### 4.10 `FollowEnd` selection

`HistoryViewportAnchor::FollowEnd` is the default root route.

Projection computes:

```text
content_width =
    terminal_width - left_padding - right_padding

total_flow_height =
    unit heights + top padding + gaps + bottom padding

overflow_rows =
    total_flow_height - history viewport capacity
```

There are two end-follow routes:

#### Cached route

`select_end_following_cached` (`projection/mod.rs:653-715`) is used when:

- no frozen static native remainder exists;
- no native physical rows have ever been inserted;
- all required unit heights are retained;
- no protected open ContentHost tail requires special handling.

It walks from bottom to top, selecting:

- bottom padding;
- complete units;
- predecessor gaps;
- a bounded flexible live unit when needed;
- top padding if capacity remains.

#### Re-measuring route

`select_end_following` (`projection/mod.rs:794-832`) walks backward while measuring units and provider rows as needed.

For a flexible live View whose full height exceeds remaining capacity, `unit_view` uses `bounded_row_viewport` (`projection/mod.rs:1041-1077`) so the visible prefix begins at offset zero and fills the remaining height.

For ordinary static or non-flexible content, `row_viewport` selects the suffix by offset, preserving end-follow behavior.

### 4.11 `NativeFrontier` selection

After native rows have been inserted, the terminal’s physical screen has already advanced upward. Projection must therefore front-pin the resident semantic History branch to the newly exposed top of the screen.

`select_native_frontier_cached` (`projection/mod.rs:717-770`) walks front-to-back:

- top padding;
- predecessor gaps;
- unit heights;
- partial final visible item;
- bottom padding if capacity remains.

`select_native_frontier` (`projection/mod.rs:834-875`) performs the same front-pinned selection while measuring as needed.

This is not an interactive scroll offset. It is a synchronization anchor between the semantic resident suffix and the terminal’s physical state after native scrollback insertion.

### 4.12 Native pressure loop

`SceneHost::render_at_with_states` (`scene/host.rs:1010-1134`) repeatedly:

1. obtains the current viewport;
2. resolves with `FollowEnd`;
3. checks synchronization-unknown state;
4. checks front ContentHost blocking;
5. checks `history_overflow_rows`;
6. drains native pressure;
7. re-resolves if native state changed;
8. falls back to `NativeFrontier` if no native progress is possible.

`drain_native_pressure` (`scene/host.rs:55-127`) receives an overflow budget and repeatedly invokes:

```rust
transfer_native_prefix_with_theme_and_content(
    history,
    sink,
    width,
    remaining_overflow_rows,
    theme,
    content,
)
```

Behavior:

- accepted physical rows consume the overflow budget;
- progress continues within the same drain pass;
- semantic-only retirement with zero physical rows still returns progress;
- if some rows were inserted and the next operation blocks, the host returns `Progress` so it re-resolves before painting;
- if nothing changed and the frontier is blocked, it returns `Blocked`;
- a blocked final attempt causes a `NativeFrontier`-anchored resolve.

This batching avoids one full Scene resolve per one-row static unit.

### 4.13 Native transfer ordering

`transfer_native_prefix_inner` (`history/native/mod.rs:135-237`) processes one front prefix in strict order:

```text
1. idle for zero budget, zero width, or empty History
2. top padding
3. leading gap
4. frozen ContentHost remainder
5. frozen static physical remainder
6. front semantic unit:
   a. Live → SemanticBlocked
   b. Static ContentHost → provider rows
   c. Static ordinary View → compiled rows
```

History cannot skip a live or unavailable ContentHost front unit to transfer later static units. This preserves semantic ordering.

### 4.14 Native retirement

`retire_front` (`history/native/mod.rs:546-559`):

1. normalizes zero-gap/top-padding crossing;
2. pops the semantic front unit;
3. queues its ID in `retired_units`;
4. updates `last_native_unit`;
5. resets unit-specific frozen/gap state.

The outer adapter drains `retired_units` before interpreting the transfer result (`native/mod.rs:91-100`). This is important for ContentHost lifetime: if a recursive transfer retires a unit and a later sink operation fails, the retired ContentPort is still notified and cleaned up.

### 4.15 Native physical transfer and host revision

The History model has two revisions:

- semantic `revision`;
- display-frontier `native_revision`.

Native retirement or native transfer can change the visible History branch without changing the body branch. `SceneHost::StableScene` records:

```text
history_identity
history_revision
native_history_revision
```

(`scene/host.rs:162-170`).

A native-only change can therefore:

- retain the body resolution;
- refresh only the History branch;
- update the root frame;
- avoid rebuilding unrelated body components.

`native_revision` increments when:

- rows are inserted;
- a semantic unit retires;
- a transfer reports progress, including zero-row semantic retirement.

`physical_rows_inserted` is a monotonic counter. `has_physical_rows()` remains true after the first insertion; it is a historical “native transfer has happened” bit, not a current count of resident physical rows.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production route

| Operation | Production route | Selection condition | Success effect | Failure/block effect |
|---|---|---|---|---|
| Push ordinary View | TS `History.push` → `NativeHistory.pushRef` → `HostHistory.push` → `History::push` | View lacks component identity and may lack ContentPort | Appends static unit, semantic revision increments | Materialization refusal, attachment validation, native/host errors |
| Push live View | Same route | View contains component identity | Appends live tail/unit | Same host validation; live later blocks native promotion |
| Push ContentHost View | Same route | View has ContentPort, no component identity | Appends static provider-backed unit and binds port to unit ID | Missing/invalid attachment target rejected before mutation |
| Freeze live unit | TS `freezeRef` → `HostHistory::freeze` → `History::freeze` | Existing unit is live; replacement lacks component identity | Replaces live semantic View with static final View | Unknown/non-live/component-bearing final View rejected |
| Discard live | TS `discardLive` → `HostHistory::discard_live` → `History::discard_live` | Existing unit is live and semantic tail | Removes transient tail; no native rows | Unknown/static/non-tail target rejected |
| Set layout | TS/native host → `History::set_layout` | Layout differs | Invalidates all height caches and semantic revision | Equal layout no-op |
| Project FollowEnd | Root resolve/projection | Default root route; no native-frontier fallback selected | End-aligned visible suffix | Provider/open-tail or missing height may force remeasurement |
| Project NativeFrontier | SceneHost fallback after native blockage | Native rows already inserted and no further transfer possible | Front-pinned semantic resident branch | Resolve errors propagate |
| Transfer ordinary static | `static_rows` → `insert_prefix` | Front unit static without ContentPort | Accepted rows; full acceptance retires unit | Zero accepted = `SinkBlocked`; partial accepted = exact frozen remainder |
| Transfer ContentHost | `history_rows` → `transfer_content` | Front static unit has ContentPort | Accepted rows reported to provider; complete transfer retires unit | Missing/incomplete provider rows = `SemanticBlocked`; partial = frozen remainder |
| Transfer live | Front semantic match | Front unit is live | No physical transfer | `SemanticBlocked { reason: Live }` |
| Sink failure | `NativeHistorySink::insert_history_rows` | Backend write/flush failure | No rewind; synchronization marked unknown | Subsequent transfer refuses until explicit host recovery |
| Invalid acknowledgement | Sink returns `accepted > requested` | Backend contract violation | No trusted logical acceptance | Synchronization marked unknown |
| Native recovery | Host’s next successful recovery/presentation path | Physical state is known again | Clears synchronization marker | No duplicate replay while marker remains |

### 5.2 Native transfer status distinctions

`history/native/mod.rs:19-36` defines:

```text
Progress
Idle
SinkBlocked
SemanticBlocked { unit, reason: Live | ContentHost }
```

The distinctions matter:

- `Progress` can mean physical rows inserted or semantic-only unit retirement;
- `Idle` means no applicable work;
- `SinkBlocked` means the sink accepted zero rows;
- `SemanticBlocked` means History cannot safely advance because ordering or provider completeness prevents it.

The host does not collapse semantic blockage into success. It either re-resolves after prior progress or paints front-pinned when no progress is possible.

### 5.3 Partial acknowledgement behavior

For spacing rows:

- full acceptance changes the spacing state to `Native`;
- partial acceptance stores remaining rows as `FrozenPhysicalRows`;
- zero acceptance returns `SinkBlocked`.

For ordinary static rows:

- full acceptance crosses zero spacing and retires the unit;
- partial acceptance stores a `FrozenStaticRemainder` tied to the front unit ID;
- later transfer resumes from exact remaining physical rows.

For ContentHost rows:

- accepted total/content/padding counts are reported to the ContentProvider;
- partial acceptance stores `FrozenContentRemainder`;
- ranges are shifted relative to the remaining payload;
- full acceptance clears the frozen remainder;
- retirement occurs only if the provider payload was complete.

The exact physical rows are preserved rather than regenerated. This is necessary for partial writes, but is also the reason resize can leave an old-width remainder.

### 5.4 Synchronization uncertainty

On `Sink` error or invalid acknowledgement:

```text
NativeFrontier.synchronization_unknown = true
```

`transfer_native_prefix_with_theme_and_content` then refuses later transfer with `SynchronizationUnknown`.

The source explicitly avoids:

- rewinding accepted logical rows;
- claiming the previous physical screen remains intact;
- replaying potentially duplicated rows.

This is conservative and correct for an irreversible terminal operation, but the code requires a higher-level recovery path to re-establish physical knowledge. The History layer itself does not clear the marker automatically.

`SceneHost::render_at_with_states` detects the marker before another transfer and paints the resolved frame rather than retrying the same rows. The comment at `scene/host.rs:1067-1076` states that retrying could duplicate output.

### 5.5 Detached failures

The N-API/native wrapper intentionally permits detached semantic setup but rejects host-dependent lifecycle operations:

- detached `push` and `setLayout` operate against the detached `Mutex<History>`;
- detached `freeze` rejects;
- detached `discardLive` rejects.

The TS documentation says detached History may outlive a Tui, while host-bound History becomes unavailable after host close. `History.callHost` checks the bound lifetime before invoking native operations (`history.ts:106-112`).

### 5.6 Content provider compatibility route

`transfer_native_prefix_with_theme` uses `EmptyContentProvider`. This is sufficient for ordinary static History and tests that do not mount ContentPort-backed History. If called against a ContentHost unit, it cannot provide rows and the transfer remains semantically blocked.

This is a compatibility/test route, not a silent fallback that fabricates content.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Height cache keys and invalidation

Per-unit cache state is keyed by:

```text
(width, HistoryUnitLayoutKey)
```

Key changes occur on:

- width change;
- static View identity change;
- ContentHost projection revision change;
- live component dependency revision change;
- `freeze`;
- `set_layout`.

Aggregate cache fields on History:

```text
cached_total_height: Cell<Option<usize>>
stale_cached_heights: Cell<usize>
```

`prepare_unit_layout` (`model.rs:305-332`):

- returns a hit when width/key/height match;
- increments `HistoryCachedHeightHits`;
- when replacing an existing height under a known aggregate, subtracts the old height and increments stale count;
- stores the new width/key and clears height.

`record_unit_height` (`model.rs:335-351`):

- records the measured height;
- repairs the aggregate if possible;
- clears or updates stale accounting.

`cached_total_flow_height` (`model.rs:360-373`):

- returns the aggregate only when no stale entries remain;
- otherwise scans all unit heights;
- returns `None` if any unit is unmeasured;
- stores a newly recomputed total when all heights are available.

### 6.2 Retained geometry shortcut

Projection uses:

```rust
let retained_geometry =
    frozen_static.is_none() && !history.native.has_physical_rows();
```

When true, all semantic unit heights can remain reusable and the aggregate can be used as a shortcut.

Once any native physical rows have been inserted, `has_physical_rows()` remains true permanently for that History object. This disables the retained aggregate shortcut thereafter, even if all prior units have retired. This is a notable performance/semantics distinction: the flag records historical native promotion, not whether a current frozen remainder exists.

A frozen static remainder forces the first unit plan to use the exact frozen physical row count. Native content remainders follow an analogous path.

### 6.3 Per-trigger work

| Trigger | Main work |
|---|---|
| First History projection | Plan every unit, resolve live units, measure missing heights, select viewport, build retained root |
| Static append | Clear aggregate, append unit, increment semantic revision, reproject end suffix |
| Live component revision | Dependency key changes, re-resolve/remeasure affected live unit |
| Content projection revision | Content key changes; provider measurement and History projection update |
| Width change | Width mismatch invalidates unit height keys; remeasure/reproject semantic resident content |
| `setLayout` | Invalidate all unit caches; recompute padding/gap overhead |
| Native partial transfer | Store exact physical remainder; project physical remainder/overlay |
| Native retirement | Remove front unit, notify ContentHost, bump native revision, reproject resident branch |
| Sink blocked | No transfer mutation; paint current projection using NativeFrontier if native rows already exist |
| Open stream append | Content dirty route; only finalized/stable prefix eligible for native transfer |
| Native-only promotion | Body can remain retained; History branch and root surface still refresh |

### 6.4 SceneHost retained/incremental behavior

`SceneHost::StableScene` tracks History identity, semantic revision, and native revision independently (`scene/host.rs:162-170`).

`try_incremental_stable` (`scene/host.rs:1391 onward`) rejects retained reuse when:

- terminal size changed;
- History identity changed;
- semantic History revision changed;
- body View changed;
- body component invalidation affects the body branch;
- required content or state changes cannot be handled incrementally.

A native History revision can trigger History-only refresh while retaining the body. The host maintains:

- `incremental_paint_history`;
- `history_only_refresh`;
- `history_components`;
- separate full/body/history paint flags.

A History geometry change that moves the body track sets `full_paint_pending`, because an incremental History paint must not leave old body rows at their previous positions.

### 6.5 Native pressure scheduling

The host’s pressure loop performs multiple transfer calls inside a single frame preparation, bounded by current overflow. It does not force one complete root resolve per physical row.

If physical progress occurs:

```text
native transfer
  → native revision changes
  → retained candidate preserved
  → History branch re-resolved
  → overflow recalculated
```

If no native progress is possible:

```text
blocked native transfer
  → retained candidate preserved
  → NativeFrontier projection
  → paint front-pinned resident content
```

The code traces:

- projection dimensions and anchor;
- physical rows inserted;
- last native unit;
- resident unit count;
- total/overflow/slack;
- transfer budget and accepted rows;
- pressure resolve and transfer-call counts.

### 6.6 Resize invalidation

`TuiHost::resize` (`application/host.rs:1391-1403`):

1. rejects zero dimensions;
2. updates `HeadlessSink.width/height` for headless hosts;
3. invalidates the running frame;
4. synchronizes real time;
5. advances/renders.

For a real Termwiz backend, terminal resize events are converted by `TermwizBackend::map_event` (`terminal/termwiz/backend.rs:85-90`) into `TerminalEvent::Resize`, updating the cached backend size. `TuiHost::poll_terminal` receives `Resize` and invalidates the running frame (`application/host.rs:1471-1507`).

History’s semantic width-dependent cache is invalidated naturally because the offered width changes. However, already accepted native physical rows and frozen physical remainders are not regenerated.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript History consumer tests

#### `packages/iyon-tui/tests/tui_history_prefix.test.ts`

This is the clearest public TS characterization.

Test 1, `appended units extend output without moving the settled prefix` (`:20-50`):

- creates a headless harness;
- obtains host-created History;
- pushes three static Views;
- renders body + History;
- asserts all History rows appear before the body;
- rerenders unchanged and asserts idempotence;
- pushes a fourth View;
- rerenders and asserts the original three remain an unchanged prefix.

Test 2, `freezing a live tail unit swaps only the tail` (`:52-72`):

- pushes a settled static unit;
- creates a ViewSlot;
- pushes `slot.view()` as a live unit;
- renders and observes settled + live rows;
- freezes the live unit to a static final View;
- rerenders and asserts only the tail text changed;
- disposes the slot.

These tests characterize insertion and replacement but do not exercise arbitrary History scrolling.

#### `packages/iyon-tui/tests/tui_harness.test.ts`

The headless harness test (`:7-27`) creates host History, pushes static text, renders it above body content, and asserts `nativeHistoryRows()` contains the pushed row. This tests TS → native → headless sink visibility.

#### `packages/iyon-tui/tests/fixtures/tui_demo.ts`

The in-tree demo fixture (`:1-47`) is a generic terminal test fixture, not an Iyon application. It:

- creates detached `new History()`;
- pushes one static completed row;
- creates a TextStream ContentPort;
- renders `Scene(body, history)`;
- reads screen rows and native History rows;
- seals the stream and disposes resources.

The fixture includes no explicit freeze/discard operation.

### 7.2 Rust root/visibility tests

`crates/iyon-tui/src/scene/root_tests.rs` includes:

- `history_and_body_use_remaining_height_and_terminal_width` (`:90-122`);
- `history_tracks_all_space_left_by_intrinsic_body` (`:125-139`);
- `history_follow_end_stays_above_body` (`:143-164`);
- `narrow_body_does_not_narrow_history` (`:167-185`);
- `body_exhaustion_gives_history_zero_height_but_keeps_live_mounted` (`:188-203`);
- `duplicate_component_across_history_and_body_uses_one_session` (`:206-216`);
- `root_mount_order_is_history_then_body` (`:219-236`);
- `zero_dimensions_preserve_semantic_resolution_without_fake_rows` (`:239-264`);
- `frozen_history_overlay_stays_inside_history_track_above_body` (`:267-301`).

The frozen-overlay test specifically verifies that a partial native remainder is overlaid inside the History track and cannot overwrite body rows.

### 7.3 Rust SceneHost native-pressure tests

`crates/iyon-tui/src/scene/host.rs` includes:

- `native_pressure_drains_multiple_units_before_reresolve` (`:4071 onward`) — 100 one-row static units are expected to be drained in a bounded number of resolve passes.
- `native_pressure_respects_overflow_budget` (`:4113 onward`) — transfer is limited to exactly the current overflow budget.
- `physical_progress_then_blocker_forces_reresolve_not_stale_paint` (`:4167 onward`) — static rows transferred before a live blocker must trigger a fresh resolve, followed by a NativeFrontier-anchored paint rather than stale pre-transfer output.
- `component_update_refreshes_history_branch_without_rebuilding_body` (`:3018 onward`) — live History invalidation can refresh only the History branch.
- `geometry_change_after_history_transfer_repaints_the_body` (`:3628 onward`) — native History movement can change body position and therefore requires whole-frame correctness.

### 7.4 ContentHost tests

Relevant `application/content.rs` tests include:

- finalized sealed History rows report `complete == true`;
- open-stream leading padding is preserved;
- retired History units remove their ContentPort and Connector state;
- History-bound warm Connectors retain finalized-prefix policy on binding/rebinding;
- theme changes invalidate content projection revision.

These are important because ContentHost units are semantically static in History but operationally dependent on Source/Connector revisions and finalized delivery state.

### 7.5 Termwiz presenter tests

`crates/iyon-tui/src/terminal/termwiz/presenter.rs` tests include:

- native sync wrapping multiple inserts;
- native insert failure ending synchronized output;
- native scroll model matching an independent shifted terminal model;
- native scroll not emitting explicit `ScrollRegionUp`;
- full repaint preserving existing scrollback;
- multiple native inserts matching the shadow terminal tape;
- native insertion larger than viewport chunk;
- wide glyph and style preservation;
- native History using full-screen CRLF;
- scrollback surviving a full repaint.

The independent shadow terminal (`terminal/termwiz/shadow.rs`) models:

- an in-memory screen;
- a separate scrollback tape;
- ordinary CRLF causing bottom overflow into scrollback;
- Termwiz `ScrollRegionUp` used only for the in-memory model, never sent to the real terminal.

### 7.6 Rust application tests

`crates/iyon-tui/src/application/tests.rs` includes:

- `neutral_app_composes_input_output_action_timer_and_persistent_history` (`:367 onward`), where application actions append to `cx.history_mut()`;
- `production_runtime_preserves_native_history_in_its_backend` (`:1811-1843`), which asserts native rows reach a fake backend and restoration occurs.

### 7.7 Performance counters

`crates/iyon-tui/src/perf.rs:34-36` defines:

- `HistoryUnitsExamined`;
- `HistoryUnitsMeasured`;
- `HistoryCachedHeightHits`.

`perf_bench.rs:364-530` benchmarks:

- a 1,000-unit static History;
- a 1,001-unit History with a live tail and component revision updates;
- projection, layout, and paint.

No benchmark was executed during this investigation.

### 7.8 Structured History tracing

`history/trace.rs:1-140` provides environment-gated stderr tracing.

Tracing is enabled once, on first trace call, only when:

```text
IYON_HISTORY_TRACE=1
```

Events:

```text
projection
transfer
resolve_pressure
```

Fields include:

- terminal and History dimensions;
- anchor;
- native physical row counter;
- last native unit;
- resident count;
- total/overflow/slack;
- transfer budget;
- requested/accepted rows;
- native counter before/after;
- resolve count and transfer-call count.

The tracer is silent by default and has direct no-panic tests. It is not a persistent metrics sink.

### 7.9 Validation status

No test or benchmark was run. The report intentionally does not claim runtime success. The test source provides strong route coverage for insertion, replacement, visibility, native pressure, physical overlays, and terminal tape behavior, but resize-after-native-scrollback remains principally an architectural risk/documented gap rather than an evidenced fixed behavior.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Generic framework boundary

The current History implementation is generic:

- it stores caller-supplied Views;
- it recognizes generic component identity;
- it recognizes generic ContentPort identity;
- it has no assistant/tool/conversation/product semantics;
- the TS consumer fixture uses generic text, streams, input, slots, and body Views.

The repository’s framework-boundary instructions explicitly prohibit putting Iyon application meaning into History, N-API, Rust presentation, or terminal code. No such product meaning was found in the current History source.

### 8.2 TypeScript History is a sideband, not ordinary View composition

Although the public TS `Scene` contains `{ body, history }`, the History object is not lowered as an ordinary nested public View:

```text
Scene.body
    → structural View publication

Scene.history
    → TS runtime sideband
    → native host History binding
    → root-level Rust History projection
```

This is consequential for any future changes to composition: body structural identity and History identity/revision are tracked independently.

### 8.3 History and ContentHost coupling

History itself does not own stream bytes, projection products, or Connector lifetime, but ContentHost must understand History-specific state:

- unit ID association;
- committed physical row counts;
- stable/finalized prefix extraction;
- transferred padding;
- ContentPort retirement.

The coupling is explicit through `ContentProvider`, not hidden behind a product-specific abstraction. The source-level owner split is sound, but History promotion cannot be understood without the ContentHost contract.

### 8.4 History and retained component coupling

Live History units resolve through the same ComponentRegistry as body components. This yields useful generic behavior:

- live cards/slots can update without replacing History order;
- component revisions participate in History height-cache keys;
- History/body duplicate identities are rejected;
- mount ordering is History first, body second.

It also means deleting or changing retained component resolution would directly affect live History semantics.

### 8.5 Native frontier versus terminal presenter

The native frontier is backend-neutral and works in terms of `PhysicalRow` and exact prefix acknowledgements. Termwiz is responsible for converting those rows into terminal operations:

```text
PhysicalRow
  → row_changes
  → terminal cursor positioning / attributes / text
  → CRLF overflow
  → native terminal scrollback
```

This keeps terminal protocol details out of History, but it also means History cannot repair rows once they are outside the visible terminal Surface.

### 8.6 Native scrollback is append-only from Iyon’s point of view

Current presenter design deliberately avoids explicit scroll-region commands on the real terminal. `native_transaction` (`presenter.rs:184-213`) instead:

1. writes each physical row at a screen row;
2. positions the cursor at the bottom;
3. emits `\r\n` repeatedly;
4. moves back to the newly exposed row;
5. clears the tail;
6. restores canonical attributes/cursor state.

The terminal’s own full-screen CRLF overflow creates native scrollback.

`apply_native_scroll_model` (`presenter.rs:215-236`) uses `Change::ScrollRegionUp` only on the in-memory Termwiz `Surface` shadow. The source comment explicitly says this must never be emitted to the real terminal.

### 8.7 Resize contradiction/risk

The current code path is:

```text
resize event or TuiHost::resize
  → update viewport dimensions
  → invalidate frame
  → presenter.resize
  → Surface.resize
  → presenter.known = false
  → full repaint of desired visible frame
```

`TermwizPresenter::resize` (`presenter.rs:36-42`) marks only the visible presenter model unknown. `present` (`presenter.rs:60-87`) then full-repaints the visible Surface when unknown.

The native scrollback is not cleared or replayed. `LAY-1-main-screen-scrollback-and-resize.md:9-26` identifies the consequence:

- rows already in native scrollback are not addressable;
- terminals may rewrap old rows differently at a new width;
- new History rows are generated at the new width;
- the resulting scrollback can mix geometries;
- the visible shadow model covers only the screen, not native scrollback.

The same issue applies to exact physical remainders:

- partial native transfers store `PhysicalRow`s rendered at the old width;
- after resize, those rows remain in `FrozenStaticRemainder` or `FrozenContentRemainder`;
- later transfer can send old-width rows after the new-width resize.

Current source therefore has a clear semantic/physical geometry hazard after resize. The research document proposes clearing/replaying native scrollback and regenerating rows, but this report makes no migration or V5 disposition decision.

### 8.8 Layout-change analogue

`History::set_layout` invalidates semantic caches but does not regenerate already-frozen physical rows. A gap or padding change after partial native transfer can therefore leave:

- old physical spacing in the native remainder;
- new semantic spacing in the resident projection.

The native frontier’s exact-remainder policy is internally consistent for irreversible transfer, but semantic layout changes are not globally retroactive.

### 8.9 No History-specific interactive scrolling

The current History API does not have:

- `scrollBy`;
- `scrollTo`;
- an arbitrary scroll offset;
- scroll position readback;
- a History-specific keyboard command;
- a History-specific mouse route.

`ScrollPane` is a separate generic control. The consumer fixture creates a `ScrollPane` for body content, but that is not History scrolling. History uses only:

```text
FollowEnd
NativeFrontier
```

The terminal emulator may let a user inspect native scrollback independently, but Iyon does not model or control that position.

### 8.10 Public API mismatch

Rust’s internal/public-in-crate surface includes:

- `FlowBoundary`;
- `HistoryUnitId`;
- `HistoryError`;
- `History::push_with_boundary`;
- full `HistoryLayout` Insets.

The public TypeScript/native surface exposes only:

- numeric unit IDs;
- scalar padding/gap;
- push/freeze/discard/layout.

This is not necessarily a bug: `crates/iyon-tui/src/history` is crate-private and the public native binding intentionally narrows the surface. However, any caller expecting boundary control or horizontal/top padding cannot obtain it through the TS facade.

### 8.11 “Freeze” has two distinct meanings

The code uses “frozen” for at least two different mechanisms:

1. **Semantic freeze:** `History::freeze` converts a live `View` into a static final `View`.
2. **Physical remainder freeze:** `FrozenPhysicalRows` stores exact unaccepted rows after a partial native sink acknowledgement.

They have different owners and semantics:

- semantic freeze is caller-requested and reversible only by replacing/removing the unit through other operations;
- physical remainder freezing is an optimization/consistency mechanism caused by partial native acceptance;
- physical remainder freezing does not mean the semantic unit is complete;
- ContentHost remainders can remain incomplete even when physical rows are being transferred.

These concepts should not be conflated when reconstructing lifecycle behavior.

---

## 9. Open questions and coverage gaps

1. **Resize recovery is unresolved in current source.** There is no current source path proving that native scrollback is cleared and replayed after resize. The visible Surface is repainted, but old native rows remain outside the presenter’s addressable model.
2. **Real terminal behavior was not executed.** Termwiz shadow tests model CRLF and scrollback, but emulator-specific resize/rewrap behavior was not observed.
3. **Native scrollback readback is unavailable for real hosts.** `HostHistory` and `NativeTuiHost` expose `nativeHistoryRows()` only for `HostBackend::Headless`; real `TermwizBackend` returns an empty vector (`application/host.rs:1546-1555`).
4. **There is no History user-scroll state.** It remains unclear whether future product requirements expect terminal-native scrollback only, a framework-owned scroll surface, or both. Current source implements only native terminal promotion plus end/front projection anchors.
5. **No explicit semantic replacement API exists.** `freeze` is the sole public replacement route and requires a live unit.
6. **No complete rollback transaction spans semantic mutation, ContentHost binding, state binding, and frame presentation.** Validation precedes mutation, but later errors do not visibly restore all earlier changes.
7. **Native synchronization recovery owner is only partially visible in this scope.** The History layer marks uncertainty and SceneHost avoids replay; a higher-level successful frame path must clear it. The exact full recovery lifecycle should be source-checked before changes.
8. **The `has_physical_rows` lifetime policy may over-constrain cache reuse.** It remains true after all native units retire. This may be intentional to preserve native-frontier anchoring but can cause retained geometry shortcuts to remain disabled indefinitely.
9. **ContentHost row completeness depends on stable/finalized delivery.** The source correctly avoids unstable open tails, but the exact latency/fairness tradeoff for smoothing backlog is not represented in the History API.
10. **TypeScript does not expose `FlowBoundary` or full Insets.** It is unknown whether this narrowing is intentional permanent API policy or simply the current binding subset.
11. **The in-tree generic fixtures are not a full application example.** `packages/iyon-tui/tests/fixtures/tui_demo.ts` and `packages/tui-consumer-fixture/src/consumer.ts` are test/consumer fixtures. No Iyon product plugin implementation was treated as present in this repository scope.
12. **No test was executed during this investigation.** All test findings are source-based.

---

## 10. Evidence appendix

### 10.1 Primary source paths and symbols

#### History semantic model

- `crates/iyon-tui/src/history/mod.rs`
  - module assembly and re-exports;
  - `History`, `HistoryLayout`, `HistoryUnitId`, `FlowBoundary`, `HistoryError` exports.
- `crates/iyon-tui/src/history/model.rs`
  - `History`;
  - `History::new`;
  - `push`;
  - `push_with_boundary`;
  - `discard_live`;
  - `freeze`;
  - `state_views`;
  - `content_views`;
  - prospective replacement helpers;
  - semantic/native revisions;
  - cache preparation/invalidation.
- `crates/iyon-tui/src/history/unit.rs`
  - `HistoryUnit`;
  - `HistoryUnitContent`;
  - `HistoryUnitLayoutKey`;
  - `HistoryUnitLayout`.
- `crates/iyon-tui/src/history/boundary.rs`
  - `FlowBoundary`.
- `crates/iyon-tui/src/history/error.rs`
  - `HistoryError`.
- `crates/iyon-tui/src/history/id.rs`
  - `HistoryUnitId`.
- `crates/iyon-tui/src/history/layout.rs`
  - `HistoryLayout`.

#### Native frontier

- `crates/iyon-tui/src/history/native/frontier.rs`
  - `NativeFrontier`;
  - `SpacingTransferState`;
  - `FrozenPhysicalRows`;
  - `FrozenStaticRemainder`;
  - `FrozenContentRemainder`.
- `crates/iyon-tui/src/history/native/mod.rs`
  - `NativeTransferStatus`;
  - `NativeTransferError`;
  - `transfer_native_prefix_with_theme_and_content`;
  - `transfer_native_prefix_inner`;
  - `insert_prefix`;
  - spacing/static/content transfer;
  - frozen remainder transfer;
  - `cross_zero_spacing`;
  - `retire_front`.

#### History projection

- `crates/iyon-tui/src/history/projection/mod.rs`
  - `HistoryProjectionParts`;
  - `HistoryPhysicalOverlay`;
  - `HistoryViewportAnchor`;
  - `project_into_session_with_mode`;
  - `protected_content_tail_bounds`;
  - `select_end_following_cached`;
  - `select_end_following`;
  - `select_native_frontier_cached`;
  - `select_native_frontier`;
  - `frozen_visible_rows`;
  - `resident_top_padding`;
  - `resident_gap`;
  - `flow_items`;
  - `ensure_height`;
  - `unit_view`;
  - `view_height`.

#### Native sink and Scene root

- `crates/iyon-tui/src/backend/native_history.rs`
  - `NativeHistorySink`.
- `crates/iyon-tui/src/scene/root.rs`
  - `Scene`;
  - `Scene::with_history`;
  - `ResolvedRootScene`;
  - `resolve_root_scene_with_anchor_and_cache_and_states_and_content`;
  - `merge_root_scene`;
  - `root_view`.
- `crates/iyon-tui/src/scene/host.rs`
  - `NativePressure`;
  - `drain_native_pressure`;
  - `PreparedSceneFrame`;
  - `StableScene`;
  - `SceneHost::render_at_with_states`;
  - `resolve_stable_at_with_anchor`;
  - `try_incremental_stable`;
  - History-only refresh;
  - native pressure tests.

#### Host and ContentHost

- `crates/iyon-tui/src/application/host.rs`
  - `HeadlessSink`;
  - `HostBackend`;
  - `HostInner`;
  - `HostHistory`;
  - `HostHistory::push`;
  - `HostHistory::freeze`;
  - `HostHistory::discard_live`;
  - `TuiHost::open_in_environment`;
  - `TuiHost::set_history`;
  - `TuiHost::resize`;
  - `TuiHost::screen_rows`;
  - `TuiHost::native_history_rows`;
  - terminal event handling.
- `crates/iyon-tui/src/application/content.rs`
  - `ContentHostRegistry::set_history_unit`;
  - `history_rows`;
  - `history_rows_committed`;
  - `clear_history_unit`;
  - `history_unit_retired`;
  - `ContentProvider` implementation;
  - `history_view`;
  - `history_transfer_blocked`.
- `crates/iyon-tui/src/presentation/content.rs`
  - `HistoryContentRows`;
  - `ContentProvider` History methods.

#### Termwiz output

- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
  - `TermwizBackend`;
  - `map_event`;
  - `NativeHistorySink` implementation;
  - `TerminalBackend` implementation.
- `crates/iyon-tui/src/terminal/termwiz/worker.rs`
  - `TerminalCommand::InsertHistory`;
  - command dispatch to presenter.
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
  - `TermwizPresenter`;
  - `resize`;
  - `present`;
  - `insert_history`;
  - `native_transaction`;
  - `apply_native_scroll_model`;
  - sync-output and failure handling.
- `crates/iyon-tui/src/terminal/termwiz/lower.rs`
  - `desired_surface`;
  - overlay compositing;
  - `row_changes`;
  - physical style lowering.
- `crates/iyon-tui/src/terminal/termwiz/shadow.rs`
  - independent screen/scrollback test oracle.

#### N-API and TypeScript

- `crates/iyon-tui-native/src/tui.rs`
  - `NativeHistory`;
  - `NativeHistory::new`;
  - `dispose`;
  - `is_detached`;
  - `take_for_host`;
  - `layout`;
  - `set_layout`;
  - `push_ref`;
  - `freeze_ref`;
  - `discard_live`;
  - `from_host`;
  - `NativeTuiHost::set_history`;
  - `NativeTuiHost::history`;
  - `NativeTuiHost::native_history_rows`;
  - `NativeTuiHost::resize`.
- `packages/iyon-tui/src/api/controls/history.ts`
  - public `History` interface/class;
  - TS lifecycle comments;
  - `History.push`;
  - `History.freeze`;
  - `History.discardLive`;
  - `History.setLayout`;
  - `History.callHost`;
  - `createHistoryHandle`;
  - `bindHistoryLifetime`.
- `packages/iyon-tui/src/transport/native/addon.ts`
  - `NativeHistoryContract`.
- `packages/iyon-tui/src/runtime/runtime.ts`
  - `commitHistoryBinding`;
  - `stageHistoryBinding`;
  - `renderDirect`;
  - `createHistory`.
- `packages/iyon-tui/src/api/view/scene.ts`
  - `SceneContract.history`;
  - `Scene.history`.
- `packages/iyon-tui/src/testing/index.ts`
  - `AppHarness.createHistory`;
  - `AppHarness.nativeHistoryRows`.

### 10.2 TypeScript consumer/example manifest

- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
  - append prefix characterization;
  - live-tail freeze characterization.
- `packages/iyon-tui/tests/tui_harness.test.ts`
  - host History and headless native row readback.
- `packages/iyon-tui/tests/fixtures/tui_demo.ts`
  - generic History + ContentPort stream fixture.
- `packages/tui-consumer-fixture/src/consumer.ts`
  - public consumer session creates a Tui-owned History;
  - no History push/freeze/discard operation in the shown fixture body.
- `packages/iyon-tui/src/index.ts`
  - exports TypeScript `History` and `HistoryLayout`.

No Iyon-specific application/plugin History source was treated as present. The consumer fixture is generic and uses only public framework APIs.

### 10.3 Test and benchmark manifest

- `crates/iyon-tui/src/scene/root_tests.rs`
  - root History/body geometry, ordering, mount, live zero-height, duplicate IDs, physical overlay.
- `crates/iyon-tui/src/scene/host.rs`
  - native pressure batching/budget/blocker tests;
  - History-only refresh and body repaint tests.
- `crates/iyon-tui/src/application/tests.rs`
  - persistent Rust History;
  - production/fake-backend native History.
- `crates/iyon-tui/src/application/content.rs`
  - finalized prefix, open-stream padding, ContentPort retirement, projection invalidation.
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
  - native CRLF, shadow scroll model, native failure, styles, wide glyphs, full repaint.
- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
  - TS insertion and freeze.
- `packages/iyon-tui/tests/tui_harness.test.ts`
  - TS native History readback.
- `crates/iyon-tui/src/perf.rs`
  - History counters.
- `crates/iyon-tui/src/perf_bench.rs`
  - static 1,000-unit and live-tail History benchmark routes.
- `crates/iyon-tui/src/history/trace.rs`
  - History trace events and no-panic tests.

### 10.4 Historical/documentary sources inspected

- `REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `LAY-1-main-screen-scrollback-and-resize.md`

`LAY-1-main-screen-scrollback-and-resize.md` is particularly relevant to the resize finding. It explicitly characterizes the current presenter as maintaining a visible-screen shadow while native scrollback is outside the addressable model, and it records replay/clear behavior as an evaluation proposal rather than current implementation.

### 10.5 Files indexed but not comprehensively read

The repository contains a large `examples/parrot_test/` replay-frame set. It was indexed to establish that examples/replays exist, but individual frame files were not read because they do not provide History implementation behavior.

Large unrelated portions of:

- `application/content.rs`;
- `application/host.rs`;
- `scene/host.rs`;
- `iyon-tui-native/src/tui.rs`;
- `PRE-V5-ARCHITECTURE-REPORT.md`;

were not repeated line-by-line when outside the History/terminal/resize call paths. Relevant symbols and source ranges listed above were inspected.

### 10.6 LOC methodology

LOC figures are approximate physical source-line counts including comments and blank lines, derived from source file extents visible during inspection. History recursive totals include private helpers and test-gated wrappers. Supporting file figures are approximate full-file sizes and are not claimed as exact production/test separations. No generated code is present in the recursive History directory.