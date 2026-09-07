# 07 — History: units, ordering, boundaries, native/projection internals, freezing, residency, caches and scrollback

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/history/` recursively.
- Parent-added atlas documentation was treated as outside the source baseline.
- The report contract and atlas README were read before source inspection.
- `PRE-V5-ARCHITECTURE-REPORT.md` was read for context and evidence expectations. Its History-specific sections emphasize decomposing `History` into ordering, lifecycle, layout, residency, caching, viewport and native-scrollback responsibilities. This report records current implementation facts and does not make V5 disposition decisions.

### Primary source scope

The recursive History directory contains these eleven Rust files:

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

### Supporting source seams inspected

To reconstruct actual behavior, I also inspected the following adjacent consumers and providers:

- `crates/iyon-tui/src/backend/native_history.rs`
- `crates/iyon-tui/src/scene/root.rs`
- `crates/iyon-tui/src/scene/root_tests.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/kernel.rs`
- `crates/iyon-tui/src/presentation/content.rs`
- `crates/iyon-tui/src/presentation/paint/view.rs`
- `crates/iyon-tui/src/perf.rs`
- `crates/iyon-tui/src/perf_bench.rs`
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
- `crates/iyon-tui/src/terminal/termwiz/worker.rs`
- `packages/iyon-tui/src/api/controls/history.ts` for the public TypeScript-facing method set only.

The assignment manifest confirms the History recursive source list at `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt:127-137`.

### Facts, inference and validation status

- **Observed source facts** are identified by exact file/symbol references.
- **Static inferences** describe behavior implied by call chains and state transitions.
- **Historical claims** are identified as documentation claims, not current implementation facts.
- I did not run tests, builds, benchmarks, formatters, or services. The validation statements below describe test code and historical completion records, not execution during this investigation.
- No repository files were edited.

### High-level current finding

Current Rust `History` is a root-level ordered semantic model with three interacting layers:

1. **Semantic model:** ordered `HistoryUnit`s containing static or live `View`s, unit identity, boundary policy and revisioned semantic layout.
2. **Width-dependent projection:** computes resident flow geometry, visible selection, viewport anchoring, semantic-to-retained `View` structure and optional physical frozen-row overlay.
3. **Native frontier:** irreversibly transfers front prefixes to a `NativeHistorySink`, retaining exact physical remainders across partial acknowledgements and tracking native synchronization state.

The model does not itself own terminal writes. The native sink contract is backend-neutral and acknowledges exact accepted prefixes (`backend/native_history.rs:5-14`).

---

## 1. Responsibility and structure

### 1.1 Module inventory

| File | Approximate physical LOC | Responsibility | Public surface |
|---|---:|---|---|
| `history/boundary.rs` | 12 | Unit-to-predecessor flow attachment policy | `pub enum FlowBoundary` |
| `history/error.rs` | 34 | Invariant and lifecycle errors | `pub enum HistoryError` |
| `history/id.rs` | 38 | Process-wide monotonic opaque unit identity | `pub struct HistoryUnitId` |
| `history/layout.rs` | 48 | Semantic padding and inter-unit gap configuration | `pub struct HistoryLayout` |
| `history/mod.rs` | 34 | Module assembly and selective re-exports | `pub History`, `HistoryLayout`, `HistoryUnitId`, `FlowBoundary`, `HistoryError` |
| `history/model.rs` | 407 | Ordered unit storage, lifecycle mutation, semantic revisions, layout caches, content/state attachment enumeration and native-frontier metadata | `pub struct History` |
| `history/unit.rs` | 48 | Internal unit representation and dependency-keyed height cache key | crate-private |
| `history/native/frontier.rs` | 104 | Native physical-transfer state, frozen physical remainders, spacing state, synchronization marker | crate-private |
| `history/native/mod.rs` | 559 | Native prefix transfer state machine and sink/error handling | crate-private |
| `history/projection/mod.rs` | 1,100 | Width-dependent plans, flow item construction, height measurement, cached/end/front selection, semantic retained View generation and frozen physical overlay | crate-private |
| `history/trace.rs` | 141 | Environment-gated structured diagnostic tracing plus two direct tests | crate-private |
| **Total** | **≈2,525** | Physical source lines including comments/blanks and test-gated helper code | — |

The count is an approximate physical-line count based on the source line extents returned during inspection. The directory contains no generated code. Direct test-only code in the recursive directory is concentrated in `trace.rs`; projection test helpers are `#[cfg(test)]` wrappers near the top of `projection/mod.rs`. Most behavioral History tests live in adjacent `scene`, `application`, and content modules rather than inside `history/`.

### 1.2 Primary and secondary responsibilities

#### `History` semantic model

`History` owns:

- ordered `VecDeque<HistoryUnit>` storage;
- stable `History` object identity;
- semantic revision;
- separate native display-frontier revision;
- semantic `HistoryLayout`;
- per-unit width/key/height caches;
- native frontier state;
- lifecycle operations:
  - `push`;
  - `freeze`;
  - `discard_live`;
  - `set_layout`;
- attachment discovery for state and content host validation;
- recovery marker handling after uncertain native sink writes.

It does **not** own:

- terminal writes;
- terminal/backend-specific protocols;
- arbitrary user scroll offset;
- a separate stream store;
- component registry ownership;
- ContentPort/Connector storage.

#### Projection

`history/projection/mod.rs` owns the History-specific rendering envelope:

- semantic unit planning;
- provider-aware resident views;
- width-dependent height measurement;
- flow items for top padding, predecessor gaps, units and bottom padding;
- FollowEnd and NativeFrontier selection;
- flexible-height live viewport bounding;
- visible row clipping;
- retained root-column construction;
- physical frozen-row overlay generation.

Projection does not own semantic mutation or native sink acknowledgment.

#### Native frontier

`history/native/frontier.rs` and `history/native/mod.rs` own:

- native physical-row acceptance accounting;
- irreversible front retirement;
- exact partial physical remainder storage;
- spacing transfer state;
- synchronization-unknown recovery state;
- native transfer status classification;
- callbacks notifying the content provider when units retire or rows commit.

The frontier is the bridge from semantic History to terminal scrollback, but does not itself know how the backend writes terminal rows.

### 1.3 Physical structure

The source is intentionally split into semantic, projection and native modules:

```text
History
 ├── VecDeque<HistoryUnit>
 │    ├── HistoryUnitId
 │    ├── FlowBoundary
 │    ├── Static(View) | Live(View)
 │    └── RefCell<HistoryUnitLayout>
 │
 ├── HistoryLayout
 ├── semantic revision
 ├── native display revision
 └── NativeFrontier
      ├── top padding state
      ├── leading gap state
      ├── frozen static remainder
      ├── frozen ContentHost remainder
      ├── last native unit
      ├── physical row counter
      └── synchronization marker
```

Projection reads all three layers but mutates only the per-unit layout cache through `prepare_unit_layout` and `record_unit_height`.

---

## 2. Types, APIs and contracts

### 2.1 Public semantic types

#### `FlowBoundary`

`history/boundary.rs:3-12`:

```rust
pub enum FlowBoundary {
    Default,
    AttachToPrevious,
}
```

- `Default` means the framework may insert the configured predecessor gap.
- `AttachToPrevious` suppresses that gap for this unit.
- The boundary is attached to the unit, not to the predecessor.
- There is no explicit “detach from previous” or custom per-unit gap value.
- The enum is `#[non_exhaustive]`, so external exhaustive matching is intentionally prevented.

#### `HistoryError`

`history/error.rs:5-12`:

```rust
pub enum HistoryError {
    UnitNotFound { unit: HistoryUnitId },
    UnitNotLive { unit: HistoryUnitId },
    LiveMustRemainTail { unit: HistoryUnitId },
    FinalViewContainsComponent { unit: HistoryUnitId },
}
```

Contracts:

- unknown IDs are rejected;
- `freeze` and `discard_live` require a live unit;
- `discard_live` requires the live unit to be the semantic tail;
- final views passed to `freeze` may not contain component identity;
- `HistoryError` is `non_exhaustive`, `Display`, `Error`, cloneable and equality comparable.

A noteworthy asymmetry is that `LiveMustRemainTail` applies to `discard_live`, but `freeze` does not enforce that the live unit is the semantic tail. A non-tail live unit can therefore remain in the deque until explicitly frozen, while native transfer stops when it reaches that front live blocker.

#### `HistoryUnitId`

`history/id.rs:9-38`:

- wraps `NonZeroU64`;
- process-wide monotonic allocation through `NEXT_HISTORY_UNIT_ID`;
- public `value()` returns the numeric representation;
- internal `from_value` rejects zero;
- implements `Clone`, `Copy`, ordering, hashing and custom `Debug`.

The identity is stable for the unit lifetime and is also used by native/content retirement callbacks.

#### `HistoryLayout`

`history/layout.rs:3-48`:

- `Insets padding`;
- `u16 gap`;
- defaults to zero padding and zero gap;
- `new`, `from_parts`, `with_padding`, `with_gap`, `padding`, `gap`;
- fields are crate-private, so construction/configuration uses methods.

Layout is semantic configuration. It is not native frontier state, although native transfer retains physical representations of padding/gap rows once accepted.

### 2.2 `History` public methods and internal contracts

`history/model.rs:22-407` defines `History`.

Public methods:

- `new`;
- `len`;
- `is_empty`;
- `push`;
- `push_with_boundary`;
- `discard_live`;
- `freeze`;
- `layout`;
- `set_layout`;
- `with_layout`.

Internal methods include:

- unit iteration;
- state/content attachment enumeration;
- prospective replacement validation;
- semantic/native revision access;
- physical row and synchronization status;
- layout preparation and invalidation.

#### `push`

`history/model.rs:72-97`:

1. Detects whether the view contains component identity.
2. Stores component-bearing views as `HistoryUnitContent::Live`.
3. Stores all other views as `HistoryUnitContent::Static`.
4. Allocates a new `HistoryUnitId`.
5. Invalidates aggregate height cache.
6. Appends to the back of `VecDeque`.
7. Bumps semantic revision.
8. Returns the ID.

`push` currently returns `Result<HistoryUnitId, HistoryError>` but the shown implementation always returns `Ok`. The `Result` is part of the stable invariant-preserving API, while current push-time validation is limited to view classification and allocation.

A ContentHost-bearing view without component identity is classified as `Static`, not `Live`. It remains semantically mutable through its provider, with provider projection revision participating in layout-cache validity.

#### `discard_live`

`history/model.rs:99-114`:

- resolves ID;
- requires the unit to be the current final deque element;
- requires `HistoryUnitContent::Live`;
- removes the unit;
- clears aggregate height cache and stale-count state;
- bumps semantic revision.

It does not create spacing, native rows or a retirement callback. The documentation explicitly calls this a transient-tail discard.

#### `freeze`

`history/model.rs:116-127`:

- resolves ID;
- requires the current unit to be `Live`;
- rejects a final view containing component identity;
- replaces the content with `Static(final_view)`;
- invalidates that unit’s layout;
- bumps semantic revision.

Freezing changes semantic content from a component-resolved live occurrence to a caller-supplied static occurrence. It does **not** itself transfer rows to native scrollback, and it does not automatically retire a unit.

The final static view may still contain a `ContentHost` identity. This allows a live component unit to be replaced by a static provider-backed content occurrence, subject to the host-level prospective attachment checks.

#### `set_layout`

`history/model.rs:290-303`:

- no-op when equal;
- replaces `HistoryLayout`;
- invalidates every unit layout and aggregate cache;
- bumps semantic revision.

Native physical remainders already accepted before a layout change are not regenerated. This distinction matters when a partial native transfer has frozen exact rows from an earlier width/layout state.

### 2.3 Unit representation and height cache keys

`history/unit.rs:12-48` defines:

```rust
pub(super) enum HistoryUnitLayoutKey {
    Static(ViewId),
    Content { view: ViewId, projection: u64 },
    Live {
        view: ViewId,
        dependencies: Vec<(ComponentId, ComponentRevision)>,
    },
}
```

The intended contract is documented directly in the source:

- static component-free units are keyed by semantic `ViewId`;
- static ContentHost units additionally include provider projection revision;
- component-bearing live units include all reachable component revisions.

A cache entry is valid only when:

1. the content identity/dependency key matches; and
2. the offered content width matches.

`HistoryUnitLayout` stores:

- `width: Option<u16>`;
- `key: Option<HistoryUnitLayoutKey>`;
- `height: Option<usize>`.

The layout cache is interior-mutable (`RefCell`) because projection receives `&History` but records measured heights.

### 2.4 Attachment enumeration contracts

`history/model.rs:145-247` provides host-validation helpers.

#### `state_views`

Returns cloned History views when either:

- the view carries a retained-state attachment; or
- the view contains component identity.

This lets the host validate and synchronize retained component/state bindings across the body and History branches.

#### `content_views`

Returns cloned History views when either:

- the view carries a ContentPort identity; or
- the view contains component identity.

This makes component-bearing views content-validation candidates as well as state-validation candidates.

#### Prospective replacement methods

`state_views_with_replacement`:

- requires target unit to be live;
- rejects component-bearing replacement as a final view;
- returns the complete state-bearing view set with the prospective replacement substituted.

`content_views_with_replacement`:

- requires target unit to be live;
- permits a replacement containing content or component identity;
- does not itself reject component identity, because `freeze` performs the final component prohibition separately.

These methods let `application/host.rs` validate host attachment targets before mutating semantic History.

### 2.5 Public consumer surfaces outside Rust History

The Rust crate exports `History`, `HistoryLayout`, `HistoryUnitId`, `FlowBoundary`, and `HistoryError` through `history/mod.rs` and the crate binding module.

The inspected TypeScript API (`packages/iyon-tui/src/api/controls/history.ts:9-23`) exposes:

```text
layout()
push(view)
freeze(unit, view)
discardLive(unit)
setLayout(layout)
```

The TypeScript History handle is a native-resource wrapper. Its comments describe detached creation, one-way host attachment, and caller/host lifetime policy. Those policies are implemented in the native/application layer rather than in the recursive Rust `history/` model itself.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
History
 ├── HistoryLayout
 ├── HistoryUnitId
 ├── FlowBoundary
 ├── View
 ├── ComponentId / ComponentRevision
 ├── ContentProvider (through projection/native seams)
 ├── perf counters
 └── NativeFrontier
      ├── PhysicalRow
      └── NativeHistorySink

History projection
 ├── History semantic units/layout/cache
 ├── ResolveSession / ResolutionOverlay
 ├── View measurement/layout compiler
 ├── ContentProvider
 ├── PhysicalRow
 └── retained View factory helpers

Scene root
 ├── resolves body branch
 ├── projects History branch
 └── merges History before body

SceneHost
 ├── measures History overflow
 ├── drains NativeFrontier pressure
 ├── selects FollowEnd or NativeFrontier anchor
 └── paints semantic surface plus optional physical overlay

ContentHostRegistry
 ├── supplies HistoryContentRows
 ├── records committed row counts
 ├── strips acknowledged padding from resident View
 └── retires ContentPort/Connector after unit retirement

Native terminal backend
 └── implements NativeHistorySink
```

### 3.2 Reverse dependency map

Major reverse consumers:

- `History` is embedded in `Scene` as one optional root-level History (`scene/root.rs:24-60`).
- `App` can own one persistent root History (`application/app.rs:15-16`, `56-60`).
- `AppCx` exposes `history()` and `history_mut()` (`application/context.rs:135-144`).
- `HostHistory` exposes layout/push/freeze/discard operations (`application/host.rs:818-947`).
- `SceneHost` projects and transfers native History (`scene/host.rs:55-127`, `1059-1162`).
- `ContentHostRegistry` implements the provider callbacks needed by ContentHost-backed History (`application/content.rs:6242-6681`).
- `NativeHistorySink` is implemented by terminal backends and test sinks.
- `perf_bench.rs` directly invokes History projection for benchmark fixtures.

### 3.3 Ownership and lifetime

#### Semantic units

- Created by `History::push`.
- Owned by `History.units`.
- Removed by:
  - `discard_live` for transient live tail; or
  - `retire_front` after native transfer completes.
- No individual unit destructor/callback exists in the semantic model.
- A unit’s ID remains valid only while its unit is in the deque.

#### Native physical remainder

- Created by partial sink acceptance.
- Owned by `NativeFrontier`.
- Replaced/decremented as later sink calls accept prefixes.
- Cleared by `reset_unit_state` when the corresponding unit retires.
- Exact physical rows are retained, rather than regenerated from semantic content.

#### ContentHost attachment

- `HostHistory::push` binds a ContentPort to the new unit (`application/host.rs:848-874`).
- Native transfer calls `content.history_rows_committed`.
- On retirement, the outer adapter drains `retired_units` and calls `content.history_unit_retired` (`history/native/mod.rs:91-100`).
- `ContentHostRegistry::history_unit_retired` removes port state and associated connectors (`application/content.rs:6355-6382`).

#### Host and scene

- `Scene` owns the semantic `History`.
- `SceneHost` owns the retained resolved frame and native transfer orchestration.
- `HostHistory` holds an `Arc<Mutex<HostInner>>` handle and performs host-serialized mutations.
- The terminal backend owns physical terminal state and acknowledges rows through `NativeHistorySink`.

### 3.4 Important ownership separation

The current code keeps three forms of state distinct:

| State | Owner |
|---|---|
| Ordered semantic unit identity/content | `History` |
| Component state and mounted component forest | `ComponentRegistry` / Scene host |
| Content source/projection/connector data | `ContentHostRegistry` and provider implementation |
| Width-dependent History height | `HistoryUnitLayout` inside `History` |
| Native physical rows already accepted | `NativeFrontier` and backend terminal |
| Scroll/follow state for generic `ScrollPane` | Scroll control, not History ContentPort/Connector |
| Terminal rendering/write protocol | backend sink implementation |

This separation is reinforced by `presentation/content.rs` comments: ContentProvider supplies History rows and resident History views, while History itself remains the ordering/native-frontier owner.

---

## 4. Execution paths and state transitions

### 4.1 Semantic append path

```text
caller
  → HostHistory::push or Rust History::push
  → validate prospective state/content attachment targets
  → classify View as Static or Live
  → allocate HistoryUnitId
  → append VecDeque tail
  → invalidate height aggregate
  → bump semantic revision
  → host invalidates frame and renders
  → Scene root resolves body
  → History projection plans/measures/selects units
```

For a ContentHost view, `HostHistory::push` additionally binds the port to the new unit and records original insets (`application/host.rs:859-871`).

For a component view, the unit is `Live`. During projection, the live view is resolved through `ResolveSession::resolve_root_with_dependencies` (`history/projection/mod.rs:190-203`).

For a static view, projection can use the semantic view directly or ask the provider for a resident History view (`history/projection/mod.rs:172-188`).

### 4.2 Root resolution path

`Scene::with_history` stores one optional root-level History above the body (`scene/root.rs:47-60`).

`resolve_root_scene_with_anchor_and_cache_and_states_and_content`:

1. Resolves the body branch.
2. Measures body height against terminal width.
3. Assigns remaining terminal height to History.
4. Projects History with `Size::new(size.width, history_height)`.
5. Finishes the History resolve session.
6. Merges History and body in visual order.
7. Returns separate History/body scenes and History metadata (`scene/root.rs:196-260`).

The History branch is resolved before the body branch in mount order. Tests assert this at `scene/root_tests.rs:219-236`. Duplicate component identities across History and body are rejected (`scene/root_tests.rs:206-216`).

### 4.3 Projection planning

`project_into_session_with_mode` (`history/projection/mod.rs:157-527`) performs the following:

1. Reads `HistoryLayout`.
2. Computes content width as terminal width minus horizontal padding.
3. Collects ordered units.
4. Builds a `UnitPlan` for every unit.
5. Uses unit-specific cache keys:
   - `Static(ViewId)`;
   - `Content { view, projection }`;
   - `Live { view, dependencies }`.
6. Applies frozen static native remainder to the first plan if present.
7. Determines whether retained geometry can be reused.
8. Computes total flow height.
9. Determines overflow rows.
10. Selects visible items according to `HistoryViewportAnchor`.
11. Builds a retained column root.
12. Emits optional `HistoryPhysicalOverlay`.
13. Records diagnostic projection trace data.

### 4.4 Flow item ordering

`flow_items` (`history/projection/mod.rs:978-988`) creates this ordered sequence:

```text
TopPadding
[Gap(index),] Unit(index)
...
BottomPadding
```

A gap is inserted only when `has_predecessor_gap` returns true.

The gap logic is:

- `AttachToPrevious` suppresses the gap;
- non-first semantic units with `Default` receive the configured `HistoryLayout.gap`;
- the first resident unit may receive a gap after native rows have already been transferred, depending on `NativeFrontier.leading_gap`;
- a native leading gap already fully accepted contributes zero resident rows;
- a partially accepted gap contributes its frozen remainder length.

### 4.5 FollowEnd selection

Default root projection uses `HistoryViewportAnchor::FollowEnd`.

There are two FollowEnd routes:

#### Cached route

Used only when:

- no frozen static native remainder exists;
- native physical rows have not been inserted;
- every unit’s retained geometry is usable;
- there is no protected open ContentHost tail.

`select_end_following_cached` (`history/projection/mod.rs:653-715`) walks bottom-to-top:

- starts with bottom padding;
- selects complete unit heights;
- inserts predecessor gaps;
- bounds a flexible live tail to the remaining height;
- includes top padding if capacity remains.

#### Re-measuring route

`select_end_following` (`history/projection/mod.rs:794-832`) computes each item height as it walks backward. It has the same end-follow semantics but can call `ensure_height` and provider-aware measurement.

A flexible live view is clipped using `bounded_row_viewport` when it is taller than the remaining viewport (`history/projection/mod.rs:1041-1077`).

### 4.6 NativeFrontier selection

When the host has already inserted physical rows, it may request `HistoryViewportAnchor::NativeFrontier`.

The cached route, `select_native_frontier_cached` (`history/projection/mod.rs:717-770`), walks front-to-back:

- starts with top padding;
- inserts predecessor gaps;
- selects unit heights from the front;
- clips the last selected item to remaining capacity;
- appends bottom padding if capacity remains.

The non-cached route, `select_native_frontier` (`history/projection/mod.rs:834-875`), repeats the same front-pinned ordering while measuring items as needed.

The semantic projection is front-pinned because the terminal’s physical top already contains native scrollback rows. This prevents semantic rows from being placed after a screen position that the backend has already advanced.

### 4.7 Native transfer path

`SceneHost::drain_native_pressure` (`scene/host.rs:55-127`) repeatedly calls:

```text
transfer_native_prefix_with_theme_and_content(
    history,
    sink,
    width,
    remaining_overflow_rows,
    theme,
    content
)
```

The host:

- limits transfer to current History overflow;
- tracks inserted rows;
- keeps calling while progress consumes the budget;
- returns `Progress` when native state changed and the scene must be re-resolved;
- returns `Blocked` when nothing could be transferred;
- treats semantic-only retirement as progress even when zero physical rows were inserted.

This avoids a full Scene resolve for every one-row unit while preserving convergence after native geometry changes.

### 4.8 Native inner transfer ordering

`transfer_native_prefix_inner` (`history/native/mod.rs:135-237`) processes exactly one front prefix in this order:

1. idle if `max_rows == 0`, width is zero, or History is empty;
2. top padding;
3. leading gap;
4. a frozen ContentHost physical remainder;
5. a frozen static physical remainder;
6. the front semantic unit:
   - `Live`: semantic blocker;
   - static ContentHost: provider rows;
   - ordinary static view: compiled physical rows.

This ordering enforces global History order. Native transfer cannot skip a front live unit or transfer a later static unit around it.

### 4.9 Retirement

`retire_front` (`history/native/mod.rs:546-559`):

1. normalizes zero-gap/top-padding crossing;
2. pops the front semantic unit;
3. records its ID in `retired_units`;
4. updates `last_native_unit`;
5. resets unit-specific frozen/gap state.

The outer transfer adapter drains `retired_units` before interpreting the transfer result, including when a later sink operation fails. This prevents ContentHost state from leaking after semantic retirement.

### 4.10 Freeze and replacement through the host

`HostHistory::freeze` (`application/host.rs:877-918`) performs a transactional-looking sequence:

1. validates nonzero unit ID;
2. computes prospective state-bearing views;
3. validates prospective state targets;
4. computes prospective content-bearing views;
5. validates prospective content targets;
6. mutates History through `History::freeze`;
7. clears prior History-unit content binding;
8. binds a replacement ContentPort if present;
9. updates desired state/content bindings;
10. invalidates and renders.

Thus, the Rust model mutation is preceded by host attachment checks. The source code does not expose a separate “replace live view” API beyond `freeze`.

### 4.11 Discard path

`HostHistory::discard_live` (`application/host.rs:921-935`):

1. validates the numeric ID;
2. calls `History::discard_live`;
3. clears content association;
4. refreshes desired state bindings;
5. invalidates and renders.

This is a semantic deletion with no physical native transfer.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Production route | Selection/condition | Success effect | Failure or blocked effect |
|---|---|---|---|---|
| Append static view | `History::push` → `VecDeque::push_back` | View lacks component identity | New `Static` unit, semantic revision increments | Current implementation returns `Ok`; host validation may reject attachments before mutation |
| Append live view | `History::push` → `VecDeque::push_back` | View contains component identity | New `Live` unit | Same host validation path |
| Append with boundary | `push_with_boundary` | Caller supplies `Default` or `AttachToPrevious` | Boundary controls later gap insertion | No custom gap or boundary validation |
| Freeze | `HostHistory::freeze` → prospective validation → `History::freeze` | Target must be live | Replaces with static final view; invalidates unit layout | Unknown/non-live/component-bearing final view rejected |
| Discard live | `HostHistory::discard_live` → `History::discard_live` | Target must be live semantic tail | Removes unit without native rows | Non-tail or static target rejected |
| Set layout | `History::set_layout` | New layout differs | Invalidates all semantic height caches | Equal layout is a no-op |
| Project FollowEnd | `project_into_session...` | Default root anchor or explicit FollowEnd | End-aligned semantic View tree | Provider/open-tail or missing height may force uncached path |
| Project NativeFrontier | `project_into_session...` | Native pressure is blocked after physical insertion | Front-pinned semantic View tree | No separate error; resolver errors propagate |
| Transfer ordinary static | `static_rows` → `insert_prefix` | Front unit is static, no frozen remainder | Accepted physical prefix; full acceptance retires unit | Zero acknowledgment yields `SinkBlocked`; partial acceptance freezes remainder |
| Transfer ContentHost | `content.history_rows` → `transfer_content` | Front static view carries content identity | Accepted rows recorded by provider; complete transfer retires unit | Missing/incomplete provider data yields `SemanticBlocked`; partial rows freeze exact remainder |
| Transfer live unit | native inner match | Front unit is `Live` | No physical transfer | `SemanticBlocked { reason: Live }` |
| Sink failure | `NativeHistorySink::insert_history_rows` returns `Err` | Backend failure after possible write | No logical rewind | Marks synchronization unknown; subsequent transfer returns `SynchronizationUnknown` |
| Invalid sink acknowledgment | sink returns `accepted > requested` | Contract violation | No logical acceptance committed | Marks synchronization unknown and returns `InvalidAcknowledgement` |
| Synchronization recovery | host successful candidate commit path | Host has recovered physical state | Clears marker | Until cleared, transfer is refused to prevent duplicate rows |

### 5.2 Native sink contract and exact-prefix semantics

`backend/native_history.rs:5-14` defines a strong contract:

- `Ok(k)` means exactly `rows[..k]` entered native history;
- no later row entered;
- an error means zero rows were accepted for that call.

The History transfer code still defensively handles the possibility that a sink error occurred after a partial physical write. It marks synchronization unknown and **does not rewind logical frontiers**, because replaying would risk duplicate terminal rows.

### 5.3 Native transfer statuses

`history/native/mod.rs:19-36`:

```text
Progress
Idle
SinkBlocked
SemanticBlocked { unit, reason: Live | ContentHost }
```

Important distinctions:

- `Progress` includes semantic retirement with zero physical rows.
- `Idle` means no work or no applicable rows.
- `SinkBlocked` means the sink acknowledged zero rows.
- `SemanticBlocked` means History cannot safely advance because the front semantic unit is live or the ContentHost provider is unavailable/incomplete.

### 5.4 ContentHost open-tail behavior

`ContentHostRegistry::history_rows` (`application/content.rs:6242-6331`) deliberately exports only a proved stable/finalized prefix:

- it asks the connector for its finalized-prefix product;
- it uses `stable_rows`, not arbitrary open projection rows;
- it does not slice unstable open Markdown content;
- sealing alone is insufficient if Smooth delivery still has backlog;
- completion requires sealed content and stable/delivered rows reaching the sealed end.

Therefore, an open stream can expose a nonempty stable prefix while remaining `complete == false`. A zero-row, incomplete result blocks transfer; a zero-row, complete result retires the unit.

### 5.5 Failure masking and explicitness

The implementation does not silently convert native uncertainty into normal progress:

- sink errors become `NativeTransferError::Sink`;
- invalid acknowledgments become `InvalidAcknowledgement`;
- uncertain physical state becomes a persistent `SynchronizationUnknown` barrier;
- live/content blockers are surfaced as typed `NativeTransferStatus`, not treated as successful no-ops;
- missing provider rows return semantic blockage rather than fabricating rows.

One subtle compatibility-style fallback is `transfer_native_prefix_with_theme`, which supplies `EmptyContentProvider`. This is appropriate for plain/static tests and non-content History, but a ContentHost unit passed through this route will remain semantically blocked because the empty provider cannot supply rows.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Semantic revision versus native revision

`History` contains two revisions (`history/model.rs:34-40`):

- `revision`: changes when semantic units, layout or semantic content change;
- `native_revision`: changes when native display-frontier state changes.

The separation is consequential:

- native retirement can remove semantic front units and change the projected History branch;
- native transfer can also change the terminal’s physical state without changing the body branch;
- `SceneHost` retains `history_revision` and `native_history_revision` separately (`scene/host.rs:1384-1402`);
- a native-only change can refresh the History branch without rebuilding the body branch.

`bump_native_revision` uses wrapping increment, while semantic revision also wraps through `bump_revision`.

### 6.2 Per-unit layout cache

`HistoryUnitLayoutKey` is dependency-sensitive:

- width is stored separately;
- static view identity is sufficient for component-free semantic views;
- provider projection revision participates in ContentHost keys;
- all reachable component revisions participate in live keys.

`prepare_unit_layout` (`history/model.rs:305-332`):

- returns a cache hit when width and key match and height exists;
- increments `Counter::HistoryCachedHeightHits` on a hit;
- when replacing an old entry, removes the old height from a known aggregate and increments stale count;
- records the new width/key and clears height.

`record_unit_height` (`history/model.rs:335-351`):

- stores measured height;
- if a total aggregate exists, repairs the aggregate’s stale accounting;
- clears aggregate when no safe aggregate remains;
- decrements stale count as replacement heights are filled.

`cached_total_flow_height` (`history/model.rs:360-373`):

- returns the aggregate only when stale count is zero;
- otherwise scans all units;
- if every unit has a height, recomputes and stores the total;
- returns `None` when any unit remains unmeasured.

### 6.3 Cache invalidation triggers

#### Unit-level

- `freeze` invalidates the changed unit.
- A changed width/key invalidates during `prepare_unit_layout`.
- Provider projection revision changes the ContentHost key.
- Component dependency revision changes the live key.

#### Whole History

- `push` clears total and stale count.
- `discard_live` clears total and stale count.
- `set_layout` invalidates all units and clears total/stale state.
- `invalidate_all_layout` resets every `HistoryUnitLayout`.

#### Native-specific

- Native physical rows disable the retained aggregate shortcut:
  `retained_geometry = frozen_static.is_none() && !history.native.has_physical_rows()`
  (`history/projection/mod.rs:223-227`).
- A frozen static native remainder forces the first unit plan to use exact physical-row height.
- Native content remainders likewise use frozen physical rows in the native frontier and corresponding overlay path.

### 6.4 Projection work by trigger

| Trigger | Expected work |
|---|---|
| First projection | Plan every semantic unit; resolve live units; measure missing units; select viewport; build retained root |
| Static append | Semantic revision and aggregate invalidation; likely remeasure/select from the end |
| Live component revision | Live dependency key mismatch; remeasure affected live unit |
| Content projection revision | Content key mismatch at offered width; remeasure provider-backed unit |
| Width change | Width mismatch for every unit; remeasure/rebuild width-dependent rows |
| Layout padding/gap change | Invalidate all unit heights and recompute flow overhead |
| Native partial transfer | Retain exact physical remainder; re-resolve with native frontier anchor |
| Native retirement | Remove front unit; bump native revision; refresh History branch |
| Sink blocked | No semantic/native mutation; projection may paint front-pinned current state |
| Open ContentHost append | Provider revision/content dirty path; only stable prefix may transfer |
| Pure native transfer | Body branch can remain retained because native revision is distinct |

### 6.5 Measurement implementation

`view_height` (`history/projection/mod.rs:1087-1099`):

- increments `Counter::HistoryUnitsMeasured`;
- creates a fresh `LayoutCache`;
- calls `measure_view_with_overlay_and_cache_and_content`;
- returns measured height as `usize`.

The outer History cache avoids repeated unit measurement across projections, but each cache miss uses a fresh local `LayoutCache`. This means History-level reuse is the primary cross-frame optimization; the per-measurement layout compiler cache is not retained by `view_height` itself.

### 6.6 Selection work and resident geometry

Projection builds a `UnitPlan` for all units, then selects only visible flow items.

`Counter::HistoryUnitsExamined` is incremented while selecting/examining units. The source defines three History-specific counters in `perf.rs:32-36`:

- `HistoryUnitsExamined`;
- `HistoryUnitsMeasured`;
- `HistoryCachedHeightHits`.

No separate counter was observed for:

- native retirement count;
- ContentHost semantic blocks;
- frozen remainder bytes/rows;
- culling count;
- visible-unit count;
- provider transfer cache hits.

### 6.7 Diagnostic tracing

`history/trace.rs` is disabled unless `IYON_HISTORY_TRACE=1` at first call (`trace.rs:1-18`).

It emits:

- `projection` records containing terminal dimensions, anchor, native row count, last native unit, resident unit count, total flow height, overflow and slack;
- `transfer` records containing requested/accepted rows, status and physical row counters;
- `resolve_pressure` records containing resolve/layout-sync/transfer counts.

Tracing is one structured stderr line per event. The global `OnceLock` means the environment decision is process-global and cannot be changed safely after first use.

---

## 7. Tests, benchmarks and observability

### 7.1 Tests inside recursive History scope

`history/trace.rs:116-141` contains two tests:

- `tracer_does_not_panic_when_disabled`;
- `tracer_macro_does_not_panic`.

They verify no-panic behavior but intentionally do not capture stderr or exercise enabled output. The comments explain that setting the environment variable in the test would permanently initialize the process-global `OnceLock`.

There are no direct unit-test modules in `history/model.rs`, `history/native/mod.rs`, `history/native/frontier.rs`, or `history/projection/mod.rs`; projection exposes `#[cfg(test)]` helper entrypoints for adjacent test modules.

### 7.2 Scene/root behavioral tests

`scene/root_tests.rs` provides important History evidence:

- root body/history height split and terminal-width use (`:90-121`);
- History receives all space left by intrinsic body (`:125-139`);
- FollowEnd rendering keeps History above body (`:143-164`);
- body width does not narrow History (`:167-185`);
- a zero-height History branch still resolves/mounts live components (`:188-203`);
- duplicate component IDs across History/body fail (`:206-216`);
- mount order is History before body (`:219-236`);
- zero dimensions preserve semantic component validation without fake rows (`:240-264`);
- partial native static transfer creates a physical overlay contained inside the History track (`:267-301`).

The overlay test is particularly important: after one of three rows is accepted by a native sink, the remaining two rows are represented as a physical overlay inside the History track rather than being recompiled as ordinary semantic text.

### 7.3 Application/content native transfer tests

`application/content.rs` contains extensive focused tests for native History behavior. Important evidence locations include:

- sink failure marks synchronization unknown without rewinding (`:9224-9270`);
- previously acknowledged rows remain counted after a later failure (`:9277-9310`);
- exact physical row products, including styles and wide-cell geometry, are compared across partial receipts (`:9361-9588`);
- width changes after a partial transfer continue from the frozen exact remainder rather than regenerating or duplicating the prefix (`:9571-9588`);
- open and sealed plain/Markdown stream transfer paths (`:9705-10190`);
- complete sealed units retire and clean up ContentHost registry state;
- smooth delivery may require repeated transfer calls before History drains.

These tests are source evidence only; they were not run during this assignment.

### 7.4 Host/runtime tests

`application/tests.rs` includes:

- a persistent History application setup (`:367-371`);
- production runtime preservation of native History in its backend (`:1810-1814`).

`scene/host.rs` includes a focused native transfer assertion around `:4026-4032`.

### 7.5 Benchmarks and counters

`perf_bench.rs` imports `HistoryViewportAnchor` and directly renders History projection (`:364-374`). It creates large static/live History fixtures around `:475-507`.

The benchmark route exercises History projection, not necessarily terminal native transfer or ContentHost provider behavior.

Counters observed in `perf.rs`:

```text
HistoryUnitsExamined
HistoryUnitsMeasured
HistoryCachedHeightHits
```

The trace path adds richer but environment-gated event fields. No direct native transfer counter exists beyond `physical_rows_inserted` and transfer trace values.

### 7.6 Historical validation records

Historical PERF-13 completion documents report focused Rust tests and broader suites passing, including History/content routes. Those are historical claims in `docs/history/PERF-13/`, not execution evidence from this investigation. The source baseline is authoritative where historical wording differs from present code.

---

## 8. Cross-boundary findings and contradictions

### 8.1 History is generic framework machinery

The current implementation is consistent with the framework ownership boundary:

- History accepts caller-supplied `View`s;
- live/static classification is based on generic component identity;
- ContentHost behavior is provider-driven;
- no agent, assistant, transcript, tool, model or product-specific semantics occur in the History modules;
- flow boundaries and native scrollback are generic terminal mechanics.

### 8.2 Semantic History and ContentHost provider are intentionally coupled

A ContentHost occurrence crosses several boundaries:

```text
semantic View with ContentPort identity
  → History static unit
  → ContentProvider::projection_revision / history_view / history_rows
  → exact PhysicalRow payload
  → NativeHistorySink
  → ContentProvider::history_rows_committed
  → ContentProvider::history_unit_retired
```

The History model cannot independently determine whether ContentHost rows are final. It delegates that decision to the provider, which uses finalized-prefix and delivered-frontier policy.

### 8.3 Native transfer is irreversible and independent of semantic revision

The code deliberately allows:

- semantic History mutation without native transfer;
- native frontier mutation without semantic History revision;
- exact physical transfer state to persist while semantic content remains resident;
- native synchronization uncertainty without rolling back semantic units.

This is a real three-state distinction, not merely a cache optimization.

### 8.4 Documentation chronology versus current source

Historical PERF-13 documents describe the removal of old `HostTextStream`/`NativeTextStream` compatibility routes and the representation of native History stream attachments as ordinary ContentHost occurrences. Current source confirms that History ContentHost integration is through `ContentProvider`/`ContentHostRegistry` rather than a dedicated recursive History stream type.

The PRE-V5 document discusses possible future decomposition and warns not to assume `History` maps directly to a future abstraction. That is a historical/design instruction, not current source behavior. This report therefore does not classify current types as future replacements.

### 8.5 Generic scroll state is not History state

The current History projection has only two internal anchor modes:

- `FollowEnd`;
- `NativeFrontier`.

It does not expose a user-controlled arbitrary scroll offset for root History. Generic `ScrollPane`/`RowViewport` scrolling is a separate control path. The ContentHost provider does not own scroll state; it supplies intrinsic/History rows and committed-row accounting.

This distinction is important because “scrollback” in the History module primarily means irreversible native promotion plus front/end projection anchoring, not a general interactive scroll controller.

### 8.6 Frozen has multiple meanings

The code uses “frozen” for at least three different states:

1. **Semantic freeze:** `History::freeze` changes `Live(View)` to `Static(View)`.
2. **Physical frozen remainder:** `FrozenStaticRemainder` or `FrozenContentRemainder` stores exact rows not yet accepted by native sink.
3. **Frozen spacing:** `SpacingTransferState::Frozen` stores unaccepted padding/gap rows.

These meanings have different owners, triggers and reversibility:

| State | Owner | Trigger | Reversible? |
|---|---|---|---|
| Semantic live-to-static | `History` | caller invokes `freeze` | Not through a reverse API |
| Physical row remainder | `NativeFrontier` | sink accepts only a prefix | Consumed incrementally; not regenerated |
| Frozen spacing remainder | `NativeFrontier` | partial sink acceptance | Consumed incrementally |

### 8.7 Potentially surprising non-tail behavior

`discard_live` enforces live-tail status, but `freeze` does not. This is not necessarily a defect: freezing a non-tail live unit can unblock native transfer once it reaches the front. However, a non-tail live unit still blocks native transfer if earlier units retire and it eventually becomes the front.

---

## 9. Open questions and coverage gaps

1. **No direct History unit tests in `history/model.rs`:** lifecycle invariants are mostly exercised through application/scene/content integration tests. The recursive module itself has limited direct behavioral coverage.
2. **No direct test of non-tail `freeze`:** source permits it, but the inspected tests primarily exercise tail/live stream transitions.
3. **No direct test of `FlowBoundary::AttachToPrevious` in the inspected History-specific tests:** the implementation path is clear (`has_predecessor_gap`, `prepare_leading_gap`, `flow_items`), but explicit behavioral assertions should be located or added by the owning test assignment if coverage matters.
4. **No direct test of cache invalidation under every dependency type:** source keys clearly include width/provider revision/component revisions, but focused per-key invalidation assertions were not found in the inspected History test locations.
5. **No direct enabled-trace output assertion:** tracing tests verify no panic, not exact structured fields or stderr output.
6. **No direct metric for ContentHost semantic blocking:** status is available through transfer outcome and trace, but no dedicated perf counter was observed.
7. **No root History arbitrary-scroll API in Rust History:** only FollowEnd and NativeFrontier anchor modes were found. If a caller needs detached/manual scrollback of root History, the inspected source does not provide it through `History`.
8. **No independent History retention/culling state machine:** semantic units remain in `VecDeque` until discard or native retirement. Projection selection is not equivalent to semantic culling.
9. **Native sink synchronization recovery depends on host transaction:** `History::recover_native_synchronization` only clears the marker. The inspected History code does not itself verify terminal state; the surrounding host/backend recovery protocol must establish that physical state is synchronized before calling it.
10. **No executed validation in this run:** all behavioral statements are static source reconstruction and test-source evidence.

---

## 10. Evidence appendix

### 10.1 Primary History files and exact symbols

- `crates/iyon-tui/src/history/mod.rs`
  - module declarations and public/internal re-exports: `:8-34`
- `crates/iyon-tui/src/history/boundary.rs`
  - `FlowBoundary`: `:3-12`
- `crates/iyon-tui/src/history/error.rs`
  - `HistoryError`: `:5-12`
  - `Display`: `:14-29`
- `crates/iyon-tui/src/history/id.rs`
  - `HistoryUnitId`: `:9-38`
- `crates/iyon-tui/src/history/layout.rs`
  - `HistoryLayout`: `:3-48`
- `crates/iyon-tui/src/history/model.rs`
  - `History`: `:22-40`
  - construction/accessors: `:43-70`
  - append: `:72-97`
  - discard: `:99-114`
  - freeze: `:116-127`
  - attachment views: `:145-247`
  - identity/revisions/native status: `:250-288`
  - layout mutation: `:290-303`
  - cache preparation/recording: `:305-373`
  - invalidation/indexing: `:376-407`
- `crates/iyon-tui/src/history/unit.rs`
  - `HistoryUnitLayoutKey`: `:12-29`
  - `HistoryUnitLayout`: `:31-36`
  - `HistoryUnit`: `:38-48`
- `crates/iyon-tui/src/history/native/frontier.rs`
  - frozen physical rows: `:5-20`
  - spacing state: `:23-34`
  - frozen static/content remainders: `:36-52`
  - `NativeFrontier`: `:54-69`
  - synchronization/row/reset helpers: `:72-104`
- `crates/iyon-tui/src/history/native/mod.rs`
  - transfer statuses/outcome/errors: `:19-50`
  - transfer adapters: `:52-132`
  - transfer inner state machine: `:135-237`
  - spacing/static/content transfer: `:251-476`
  - static compilation: `:507-520`
  - spacing crossing/retirement: `:522-559`
- `crates/iyon-tui/src/history/projection/mod.rs`
  - plans and flow items: `:36-83`
  - host/test projection entrypoints: `:85-163`
  - complete projection pipeline: `:164-527`
  - protected open-content-tail logic: `:529-592`
  - item height/selection: `:595-888`
  - flexible and frozen rows: `:890-940`
  - gap/flow item construction: `:951-988`
  - measurement and retained unit view generation: `:990-1099`
- `crates/iyon-tui/src/history/trace.rs`
  - environment gate/macro: `:1-43`
  - projection trace: `:45-80`
  - transfer/pressure traces: `:82-114`
  - tests: `:116-141`

### 10.2 Supporting source files and exact symbols

- `crates/iyon-tui/src/backend/native_history.rs`
  - `NativeHistorySink`: `:5-14`
- `crates/iyon-tui/src/scene/root.rs`
  - `Scene` History ownership: `:24-99`
  - `ResolvedRootScene`: `:118-136`
  - root resolution: `:196-260`
  - root merge and mount ordering: `:301-376`
- `crates/iyon-tui/src/scene/host.rs`
  - `drain_native_pressure`: `:47-127`
  - retained `StableScene` History revisions: `:162-170`
  - host resolve/projection route: `:1331-1388`
  - History revision comparison: `:1393-1404`
  - native-frontier conditional refresh: `:1701-1707`
- `crates/iyon-tui/src/application/host.rs`
  - `HostHistory`: `:818-947`
  - push: `:848-875`
  - freeze: `:877-918`
  - discard: `:921-935`
  - History validation/replacement: `:1331-1361`
- `crates/iyon-tui/src/application/content.rs`
  - `HistoryPortState`: `:2808-2816`
  - `HistoryTerminalAdapter`: `:2818-2950`
  - History row provider: `:6242-6332`
  - committed rows/retirement: `:6334-6382`
  - ContentProvider History implementation: `:6572-6683`
  - projection revision including History adapter state: `:6590-6613`
- `crates/iyon-tui/src/presentation/content.rs`
  - `HistoryContentRows` and provider contracts: `:137-218`
- `crates/iyon-tui/src/scene/root_tests.rs`
  - History/root dimensions and selection: `:90-185`
  - live mounts, duplicate components and order: `:188-236`
  - zero dimensions and physical overlay: `:240-301`
- `crates/iyon-tui/src/perf.rs`
  - History counters: `:32-36`
- `crates/iyon-tui/src/perf_bench.rs`
  - History render benchmark route: `:364-374`
  - large History fixtures: `:466-507`
- `crates/iyon-tui/src/application/content.rs`
  - native sink failure/recovery tests: `:9224-9310`
  - exact row/partial/resize tests: `:9361-9588`
  - stream transfer/retirement tests: `:9705-10190`
- `packages/iyon-tui/src/api/controls/history.ts`
  - public History interface: `:9-23`
  - detached/attached lifetime documentation: `:28-43`
  - native wrapper construction: `:44-56`
  - layout and push forwarding: `:58-77`

### 10.3 Historical/context documents inspected or indexed

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `AGENTS.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
  - History autopsy guidance: section 14, approximately `:702-760`
  - residency/caching/freezing guidance: section 15, approximately `:764-811`
  - streaming trace requirements: section 12, approximately `:623-658`
- `docs/history/PERF-13/PERF-13-G-completion.md`
  - current-history/content-host integration claims
- `docs/history/PERF-13/PERF-13-G-implementation-notes.md`
  - History/content provider seam and stable-prefix policy
- `docs/history/PERF-13/PERF-13-H-completion.md`
  - removal of old stream/History compatibility surfaces
- `docs/history/PERF-13/PERF-13-F-implementation-notes.md`
  - historical distinction between ContentPort/Connector and viewport ownership
- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`
  - source/native/content ownership and viewport-boundary statements

Historical documents were used to identify chronology and potential documentation drift. Current source was treated as authoritative.

### 10.4 Inspected-file manifest

#### Recursive primary scope

```text
crates/iyon-tui/src/history/boundary.rs
crates/iyon-tui/src/history/error.rs
crates/iyon-tui/src/history/id.rs
crates/iyon-tui/src/history/layout.rs
crates/iyon-tui/src/history/mod.rs
crates/iyon-tui/src/history/model.rs
crates/iyon-tui/src/history/native/frontier.rs
crates/iyon-tui/src/history/native/mod.rs
crates/iyon-tui/src/history/projection/mod.rs
crates/iyon-tui/src/history/trace.rs
crates/iyon-tui/src/history/unit.rs
```

#### Supporting Rust source

```text
crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/application/context.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/application/kernel.rs
crates/iyon-tui/src/application/tests.rs
crates/iyon-tui/src/backend/native_history.rs
crates/iyon-tui/src/perf.rs
crates/iyon-tui/src/perf_bench.rs
crates/iyon-tui/src/presentation/content.rs
crates/iyon-tui/src/presentation/paint/view.rs
crates/iyon-tui/src/scene/host.rs
crates/iyon-tui/src/scene/root.rs
crates/iyon-tui/src/scene/root_tests.rs
crates/iyon-tui/src/terminal/backend.rs
crates/iyon-tui/src/terminal/termwiz/backend.rs
crates/iyon-tui/src/terminal/termwiz/presenter.rs
crates/iyon-tui/src/terminal/termwiz/worker.rs
```

#### Supporting TypeScript source

```text
packages/iyon-tui/src/api/controls/history.ts
```

#### Indexed but not comprehensively read

The repository-wide tracked-source manifest was indexed to verify assignment coverage. Other Rust modules, generated code, native addon implementation, broad TypeScript transport, fixtures, and unrelated architecture reports were not treated as part of this assignment’s primary evidence unless a direct History seam was required.