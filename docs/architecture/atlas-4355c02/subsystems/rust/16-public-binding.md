# 16 — Public Binding

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Assignment: `16`, `rust/public-binding`
- Primary scope:
  - `crates/iyon-tui/src/lib.rs`
  - `crates/iyon-tui/src/binding/`
  - `crates/iyon-tui/src/id.rs`
  - `crates/iyon-tui/Cargo.toml`
  - root-level helpers otherwise not assigned to another Rust subsystem:
    - `src/scroll.rs`
    - `src/scroll_command.rs`
    - `src/text.rs`
    - `src/perf.rs`
    - `src/perf_bench.rs`
    - `src/bin/tui_perf.rs`

I read the atlas `REPORT-CONTRACT.md`, atlas `README.md`, repository `AGENTS.md`, and `PRE-V5-ARCHITECTURE-REPORT.md` for context. The report below describes current source behavior only. Historical handoff material is explicitly labeled when referenced. No V5 disposition or migration recommendation is made.

### Framework boundary

The crate is generic TUI infrastructure. Its public/native seam must remain generic and must not encode Iyon agent/application policy. The inspected scope contains no product-specific agent, assistant, provider, prompt, transcript, tool, queue, or application-status concepts.

The intended boundary is:

```text
TypeScript @iyon/tui authoring surface
              │
              ▼
      iyon-tui-native addon
              │
              ▼
  iyon_tui::binding  [unsupported Rust/native seam]
              │
              ▼
private / crate-visible iyon-tui runtime
```

Rust application authors are not intended to author Views, controls, renderers, projectors, or themes through `iyon-tui`. The crate-level documentation says the TypeScript facade is the supported authoring surface.

### Evidence status

This is a static source inspection. I did not execute Cargo tests, build commands, benchmark binaries, Bun checks, or generated-ABI checks. Existing target artifacts and historical reports were not treated as execution evidence.

The strongest current-surface evidence is:

- `crates/iyon-tui/src/lib.rs:1-117`
- `crates/iyon-tui/src/binding/mod.rs:1-104`
- `crates/iyon-tui/Cargo.toml:1-43`
- `tools/api-surface/check-binding.ts:1-155`
- native import sites under `crates/iyon-tui-native/src/`
- `crates/iyon-tui/README.md`
- `iyon-tui.md`

---

## 1. Responsibility and structure

### 1.1 Module visibility inventory

| Path | Physical LOC | Visibility from crate root | Responsibility |
|---|---:|---|---|
| `src/lib.rs` | 117 | Public crate root | Declares module visibility and crate-internal aliases; exposes `binding` and the feature-gated benchmark exception |
| `src/binding/mod.rs` | 104 | `pub mod binding` | Flat re-export-only native seam |
| `src/id.rs` | 12 | Private module | Shared monotonic nonzero-ID allocator |
| `src/scroll.rs` | 291 | `pub(crate) mod scroll`; type itself `pub` | Generic retained local viewport/controller for arbitrary semantic Views |
| `src/scroll_command.rs` | 26 | Private module; enum/functions `pub(crate)` | Maps unmodified navigation keys to scroll commands |
| `src/text.rs` | 8 | Private module | Internal convenience re-export of semantic text IR |
| `src/perf.rs` | 251 | `pub(crate)` with `perf-counters`, otherwise private | Optional counters and snapshots |
| `src/perf_bench.rs` | 557 | `pub mod`, `#[doc(hidden)]`, `perf-counters` only | Package-local benchmark oracle callable by `src/bin/tui_perf.rs` |
| `src/bin/tui_perf.rs` | 3 | Separate binary target | Calls `iyon_tui::perf_bench::run()` |
| `crates/iyon-tui/Cargo.toml` | 43 | Package manifest | Package identity, publication status, features, binary target, dependencies |

The assigned Rust source totals approximately **1,369 physical lines**:

```text
lib.rs             117
binding/mod.rs     104
id.rs               12
scroll.rs          291
scroll_command.rs  26
text.rs              8
perf.rs            251
perf_bench.rs      557
tui_perf.rs          3
                   ---
                  1369
```

This is a physical line count including comments, blank lines, and embedded tests. `scroll.rs` contains the relevant unit tests in its final approximately 69 lines. `perf.rs` contains small test-only counter-lock helpers; `perf_bench.rs` has no embedded test module. The benchmark file is production tooling rather than ordinary runtime code.

### 1.2 Crate-root layout

`lib.rs` deliberately divides the crate into three visibility classes.

#### Private modules

These are inaccessible to external Rust crates:

- `backend`
- `component`
- `geometry`
- `id`
- `physical`
- `scroll_command`
- `stream`
- `terminal`
- `text`

The module-private status is stronger than merely hiding documentation. External callers cannot name these module paths.

#### Crate-visible runtime modules

These are available to `iyon-tui` itself and to descendants but not to a separate dependent crate:

- `application`
- `content`
- `controls`
- `history`
- `interaction`
- `output`
- `presentation`
- `projection`
- `retained_state`
- `scene`
- `scroll`
- `theme`

Several of these modules contain `pub` types/functions for use by internal descendants or the binding re-export, but their crate-root ancestors prevent direct external access.

#### Public crate-root exceptions

Only two root modules are public:

1. `pub mod binding;` at `lib.rs:112-117`
2. Feature-gated, hidden `pub mod perf_bench;` at `lib.rs:28-34`

The second is explicitly documented as an executable-only tooling escape hatch. It exists because `src/bin/tui_perf.rs` is a separate crate target and cannot access a private library module.

There are no public crate-root `pub use` aliases and no public `prelude` module.

### 1.3 Crate-internal aliases

`lib.rs:52-110` creates `pub(crate) use` aliases for frequently used internal types, including:

- application: `App`, `AppCx`, test-only `AppHandle`
- components: `Component`, `ComponentCx`, `ComponentHandle`
- content/text and diff types
- `TextInput`
- `History`, `HistoryLayout`, `HistoryUnitId`
- interaction types
- output types
- test-only projection types
- `Scene`, `ScrollPane`, `Theme`
- selected style, grid, text, semantic, and projection types

These aliases are intentionally not public. They shorten internal paths but do not create authoring APIs.

The comments at `lib.rs:37-40` and `lib.rs:94-110` explicitly state that semantic text, projection, source coordinates, and retained structures remain implementation-only even where their item declarations are public enough for internal reuse or binding re-export.

### 1.4 Root helper responsibilities

#### `id.rs`

`next_nonzero_id` is the only item in this root module:

```rust
pub(crate) fn next_nonzero_id(
    counter: &AtomicU64,
    exhausted: &'static str,
) -> NonZeroU64
```

It performs a relaxed atomic checked increment. Overflow causes an explicit panic with the caller-provided exhaustion message. The resulting integer is converted to `NonZeroU64`, with a second invariant panic if the counter somehow begins at zero.

Consumers include:

- component identity allocation in `component/id.rs`
- History identity allocation in `history/model.rs` and `history/id.rs`
- output identity allocation in `output/handle.rs`

The helper does not own a namespace. Each caller supplies a different static counter.

#### `scroll.rs`

`ScrollPane` is a retained component that owns:

- a semantic `View` content handle,
- follow-end or detached scrolling mode,
- the latest allocated viewport size,
- an optional full-content extent hint.

It intentionally tracks a visual row position, not semantic content anchors. The source comments state that semantic anchoring belongs to content and History owners.

The type is declared `pub`, and its methods are mostly `pub`, but the containing module is only `pub(crate)`. Therefore it is not a public root API. Native host code reaches it through internal crate aliases and wraps it in `HostScrollPane` rather than importing it externally.

#### `scroll_command.rs`

The private command layer contains:

```rust
enum ScrollCommand {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Start,
    End,
}
```

`map_scroll_key` accepts a `KeyStroke` and returns `None` for any modifiers or unsupported key. It maps only unmodified Up, Down, PageUp, PageDown, Home, and End.

The command enum and mapper are `pub(crate)` only.

#### `text.rs`

This module is an eight-line internal shim:

```rust
pub use crate::content::text::*;
```

The wildcard re-export makes semantic text names convenient as `crate::text::*` for internal code such as `application/content.rs` and `content/text/ansi.rs`. Because `text` itself is private at the root, this does not expose a public Rust text authoring module.

#### `perf.rs`

The performance module defines a stable enum of counters and a snapshot API. It is not a normal framework authoring API.

The counter list covers:

- View construction/cloning
- N-API View cache activity
- resolver/component work
- measure/prepare/layout/paint work
- persistent-sequence allocations
- component geometry visits
- retained-state mutations/invalidation/damage
- semantic preparation and source snapshots
- content projection, registry, metric, wake, and path-index work

`Counter::COUNT` is derived from the final enum discriminant. `NAMES` is a fixed machine-readable name table.

`PerfSnapshot` stores all counter values and provides:

- `value(counter)`
- `iter()` yielding canonical JSONL order

The operations are:

- `reset`
- `inc`
- `add`
- `set`
- `snapshot`

Only `Counter`, `add`, `inc`, `reset`, and `snapshot` cross the native binding. `set` remains internal.

#### `perf_bench.rs`

This is an opt-in benchmark oracle, not ordinary runtime behavior. It:

- builds generic View fixtures,
- exercises text, row, column, grid, styled-span, and component-heavy workloads,
- compares fresh, identical-identity, shared-path, and rebuilt-equivalent routes,
- times View clone, layout, paint, and History scenarios,
- emits JSONL records,
- includes counter snapshots and a git SHA,
- reads environment variables controlling iteration counts,
- has a paint-gate-only route.

The binary target is enabled only when `perf-counters` is enabled.

---

## 2. Types, APIs and contracts

### 2.1 Intentional external surface

The crate root intentionally exposes no supported Rust authoring API. `lib.rs:1-11` says:

- the crate is an unpublished implementation crate;
- Rust applications do not author Views, controls, or renderers through it;
- the only public bridge is `binding`;
- semantic construction and retained storage remain private;
- TypeScript callers use `@iyon/tui`.

The crate manifest reinforces this with `publish = false` at `crates/iyon-tui/Cargo.toml:6-8`.

The only runtime-facing external Rust namespace is therefore:

```text
iyon_tui::binding
```

It is explicitly unsupported and exists for `iyon-tui-native`.

### 2.2 Binding facade contract

`binding/mod.rs` is intentionally flat and implementation-free. Its opening comments state:

- every item is a re-export;
- the facade contains no implementation graph;
- the export set is pinned by `bun run check:tui-binding`;
- the fluent View DSL, public renderer/projector extension ecosystem, and user callbacks are excluded.

The current export groups are as follows.

#### Content and text values

Always available unless otherwise noted:

- `DiffHunk`
- `DiffLine`
- `DiffLineNumber`
- `DiffLineOffset`
- `DiffLineTermination`
- `DiffRange`
- `FormatId`
- `LanguageId`
- `SemanticTag`
- `TextOrigin`
- `TextPart`
- `TextRole`
- `TextSelector`
- `TextSpan`
- `HorizontalAlign`
- `WrapMode`
- `SmoothConfig`

Native-host-only content helper:

- `lower_diff_hunks`

Native-host-only text page:

- `NativeTextPage`

#### Grid values and operation-specific constructors

Passive types:

- `GridCellSpec`
- `GridTrack`

Operation wrappers:

- `grid_track_content`
- `grid_track_content_max`
- `grid_track_fixed`
- `grid_track_flex`
- `grid_track_flex_max`
- `grid_cell_spec_new`
- `grid_cell_spec_column_span`
- `grid_cell_spec_row_span`
- `grid_cell_spec_horizontal_align`
- `grid_cell_spec_vertical_align`

The underlying `GridTrack` constructors and `GridCellSpec` mutators are crate-private in `presentation/api/grid.rs`; the binding exposes deliberate wrapper functions rather than making the internal methods externally callable.

#### Style and geometry values

- `AnsiColor`
- `BorderEdges`
- `BorderGlyphs`
- `BorderSpec`
- `BorderStyle`
- `ColorSpec`
- `Insets`
- `OverflowIndicator`
- `StyleRef`
- `StyleSelector`
- `StyleSpec`
- `StyleStateKey`
- `StyleStateValue`
- `TextAttribute`
- `TextAttributeSpec`
- `ThemeColor`
- `ThemeKey`
- `VerticalAlign`

The binding also exposes:

- `Theme`
- native-host-only `intern_style_atom`

These are generic presentation/style values. No product theme key or application policy is encoded in this scope.

#### View handles and retained operations

Always exposed:

- `View`

Native-host-only opaque/retained values:

- `RetainedPathStep`
- `WeakView`
- `NativeCommonPatch`

Native-host-only operation wrappers:

- `view_text`
- `view_text_plain`
- `view_styled_text`
- `view_spacer`
- `view_clamp_rows`
- `view_hanging`
- `view_native_component`
- `view_native_content_host`
- `view_native_text_final`
- `view_native_grid_final`
- `view_native_axis_from_children`
- `view_native_axis_set_child`
- `view_native_axis_splice`
- `view_native_grid_set_cell`
- `view_native_replace_at_path`
- `view_native_patched`
- `view_native_container`
- `view_native_state_attachment_id`
- `view_native_state_attachment_ids`
- `view_native_state_capable`
- `view_native_with_state_attachment`
- `view_native_with_content_attachment`
- `view_downgrade`
- `view_upgrade`
- `view_try_with_text_layout_patch`
- `view_try_with_text_layout_patch_path`
- `view_try_with_text_layout_patch_path_with_nodes`
- `view_try_retained_child`
- `view_try_replace_retained_children`

The wrappers delegate into `presentation::binding`, which is itself behind the crate-visible presentation boundary.

The `View` type is externally visible through the binding but remains largely opaque. `presentation/ir.rs` shows that its useful inherent methods are `pub(crate)`. The public binding therefore supplies operation-specific functions rather than exposing a fluent public method surface.

`RetainedPathStep` is an exception in that its fields and `new` constructor are public. It is a passive path descriptor required by native retained updates.

#### Host and runtime values

Native-host-only re-exports:

- `ContentAnnotationRecord`
- `ContentDelivery`
- `ContentFamily`
- `ContentMutationResult`
- `HostContentConnector`
- `HostContentFunnel`
- `HostContentPort`
- `HostContentSource`
- `TextFunnelKind`
- `TextSourceKind`
- `TextWrapMode`
- `TuiEnvironment`
- `WakeDisposition`
- `HostCellStyle`
- `HostHistory`
- `HostScrollPane`
- `HostTextInput`
- `HostViewSlot`
- `TuiHost`
- `HostViewState`
- `GeometryAlignment`
- `ViewStateGeometryPatch`
- `ViewStateGeometryProperty`
- `ViewStateSizeMode`
- `ViewStatePresentationPatch`
- `ViewStatePresentationProperty`

Always available:

- `TextInput`
- `History`
- `HistoryLayout`
- `Key`
- `KeyStroke`
- `Modifiers`
- `Output`

These are the types needed by the native host adapter. They are not reachable from the crate root through public module paths.

#### Performance values

With `perf-counters`:

- `Counter`
- `add`
- `inc`
- `reset`
- `snapshot`

The native crate's `perf-counters` feature forwards to `iyon-tui/perf-counters`.

### 2.3 Deliberately absent authoring items

The binding does not export:

- `IntoView`
- `Renderer`
- `DiffRenderer`
- the old `prelude`
- a wholesale View factory namespace
- `Projection`
- `Projector`
- `ProjectorExt`
- `ProjectionBuilder`
- arbitrary callbacks into runtime hot paths
- public generic renderer/projector extension traits

`tools/api-surface/check-binding.ts:104-114` explicitly checks that `IntoView`, `Renderer`, and `DiffRenderer` do not return to the binding.

The absence is significant: the binding exposes enough operation-specific constructors and passive values for the native adapter, but not a supported Rust equivalent of the TypeScript semantic facade.

### 2.4 Type and ownership contracts

Important contracts visible from the inspected APIs:

- `View` values are immutable retained handles. Native update wrappers consume a base `View` and return a new `View` or a typed error.
- Weak references are explicit through `view_downgrade` and `view_upgrade`.
- Grid span setters reject zero by panicking through `NonZeroU16::new(...).expect(...)`; zero is treated as an invariant violation rather than a recoverable value.
- `NativeTextPage::span` validates bounds and UTF-8 boundaries before creating a `TextSpan`.
- `ScrollPane::new` and `set_content` assert that the content does not contain component identity. This prevents a generic local viewport from silently owning component lifecycle.
- `HistoryUnitId` and other identity values are nonzero and monotonic within process-local namespaces.
- Performance counters are observational only and do not affect runtime semantics when disabled.

---

## 3. Dependency and ownership map

### 3.1 Ownership map

```text
src/id.rs
  └── next_nonzero_id(counter)
        ├── component/id.rs        → ComponentId
        ├── history/id.rs          → HistoryUnitId
        ├── history/model.rs       → History identity
        └── output/handle.rs       → Output identity

src/scroll_command.rs
  └── map_scroll_key(KeyStroke)
        └── ScrollCommand

src/scroll.rs
  └── ScrollPane
        ├── owns semantic View handle
        ├── owns follow/detached mode
        ├── remembers viewport Size
        └── optionally remembers content extent

src/perf.rs
  ├── Counter enum / NAMES
  ├── process-global atomic values
  └── PerfSnapshot

src/perf_bench.rs
  ├── creates generic View/History fixtures
  ├── calls private layout/paint/history owners
  └── emits JSONL records

iyon_tui::binding
  └── re-exports selected types and operation wrappers
        └── iyon-tui-native
              ├── N-API adapters
              ├── direct FFI adapters
              └── generated ABI glue
```

### 3.2 Binding dependency direction

```text
iyon-tui-native
    │
    │  only `iyon_tui::binding::*`
    ▼
iyon_tui::binding
    │
    ├── presentation::binding
    ├── presentation::api
    ├── presentation::ir
    ├── application::content/environment/host/view_state
    ├── retained_state::geometry/presentation
    ├── content::diff/text
    ├── history
    ├── interaction
    ├── output
    ├── theme
    └── perf
```

The facade has no storage of its own. Ownership remains with the implementation modules:

- retained View data is owned by `presentation::ir`;
- host/content records are owned by application/content and retained-state modules;
- counters are owned by `perf`;
- style atom interning is owned by `theme`;
- identity allocation remains in the identity-owning subsystem.

### 3.3 Actual consumers

The native crate imports through the seam in:

- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/theme_dto.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`

Qualified uses also appear in those files for:

- text span construction,
- counter reset/snapshot/increment/add,
- diff lowering,
- style atom interning,
- retained View operations,
- host/content records.

The repository search found no native source use of `iyon_tui::` outside `iyon_tui::binding::...`.

### 3.4 Manifest-level dependency map

The workspace contains:

- `crates/iyon-tui`
- `crates/iyon-tui-native`
- `tools/tui-abi-gen`

`iyon-tui-native/Cargo.toml` depends on the core as:

```toml
iyon-tui = { workspace = true, features = ["native-host"] }
```

Thus the native crate always compiles the native-host portions of the binding and the core's host integration. Its optional features include:

- `perf-counters = ["iyon-tui/perf-counters"]`
- `fast-view-abi = []`
- `direct-ffi = []`

Its dev-dependency additionally enables `test-util`, but this does not restore a public core testing module.

### 3.5 Identity and lifetime

The generic `next_nonzero_id` helper gives each namespace a process-local monotonic identity. It does not recycle IDs. Exhaustion is an explicit panic.

`ScrollPane` owns its content View by value, but `View` itself is a retained shared handle. Replacing content drops the old handle according to normal Rust ownership after assigning the new handle. The pane's viewport mode survives content replacement, subject to detached-position repair.

Binding retained operations use immutable derivation semantics:

```text
base View
  ├── retained path operation
  ├── text/layout patch
  ├── child replacement
  └── attachment operation
        ▼
new View or Result<new View, error>
```

The facade does not expose mutable global View state or arbitrary callback ownership.

---

## 4. Execution paths and state transitions

### 4.1 Native text construction

Representative path:

```text
native DTO / ABI input
  → validation in iyon-tui-native
  → binding::text_span_plain / text_span_styled
  → binding::view_text / view_native_text_final
  → presentation::binding
  → presentation::factory::text_from_spans
  → retained presentation::ir::View
```

`TextSpan::plain` and `TextSpan::styled` are crate-private constructors in the core presentation API. The native seam exposes dedicated operation functions so native ingress can construct validated spans without making the core's internal constructor surface a supported Rust API.

For native length-delimited text, `NativeTextPage` retains one owned page and creates checked spans over ranges. Out-of-bounds ranges and UTF-8 sequence splits return `None`.

### 4.2 Native structural updates

Axis and grid updates use operation-specific wrappers:

```text
binding::view_native_axis_from_children
binding::view_native_axis_set_child
binding::view_native_axis_splice
binding::view_native_grid_set_cell
binding::view_native_replace_at_path
```

These delegate to retained `View` methods. The methods return `Result` where path/index/shape validation can fail. They produce derived retained values rather than mutating a published View in place.

The replacement-at-path route may return both:

```text
Result<(View, Vec<View>), String>
```

The extra returned Views are part of the native retained-update protocol and are not generic authoring children.

### 4.3 Native state/content attachment

Native host operations use:

```text
view_native_state_attachment_id
view_native_state_attachment_ids
view_native_state_capable
view_native_with_state_attachment
view_native_with_content_attachment
```

These are feature-gated by `native-host` and delegate to canonical retained View operations. Attachment identity is carried as an explicit numeric value, and attachment discovery can fail with a `String` error.

### 4.4 Weak retained handles

```text
binding::view_downgrade(&View) → WeakView
binding::view_upgrade(&WeakView) → Option<View>
```

The weak route is explicit. Upgrade failure is represented as `None`; there is no fallback to a newly constructed View.

### 4.5 ScrollPane initialization and layout

`ScrollPane::new(content)`:

1. validates that `content` has no component identity;
2. stores the content;
3. starts in `FollowEnd`;
4. has no known layout size or content extent.

Before the first layout callback, `view()` returns the content with fill width/height rules. The same fallback is used for zero-sized layout. This preserves intrinsic content visibility until the pane receives useful geometry.

On layout:

```text
on_layout_changed(Size)
  → ignore if unchanged
  → store new viewport size
  → repair detached top row
```

On content extent update:

```text
on_content_extent_changed(Size)
  → ignore if unchanged
  → store extent
  → repair detached top row
```

`content_extent` is an optimization/coordination hint supplied by native layout. If absent, `ScrollPane` measures content at the current width.

### 4.6 Scroll state transitions

The state machine is:

```text
FollowEnd
   │ scroll up / explicit start
   ▼
Detached { top_row }
   │ scroll down to max_top
   ▼
FollowEnd
```

`set_content` preserves a detached position where possible, then clamps it to the new maximum. It clears the cached extent and repairs the detached position.

`follow_end` explicitly returns to tail-following. `scroll_to_start` enters detached mode at row zero.

`scroll_up` and `scroll_down` return `bool` indicating whether state changed. Attempting to move outside the range returns `false`, except the special case of downward movement from a detached state already at the end, which transitions back to `FollowEnd` and returns `true`.

### 4.7 Key routing

The component capability registration is:

```text
cx.focusable()
cx.on_layout_changed(Self::on_layout_changed)
cx.key_commands(Self::map_command, Self::handle_command)
```

Key flow:

```text
KeyStroke
  → ScrollPane::map_command
  → scroll_command::map_scroll_key
  → ScrollCommand or None
  → ScrollPane::handle_command
  → scroll/page/start/end operation
  → InteractionResult::Consumed
```

All recognized commands return `Consumed`, even if the requested move did not change the position. Unsupported modifiers/keys do not produce a command.

### 4.8 Performance counter state transitions

When `perf-counters` is disabled:

- `reset` has no effect;
- `add`, `inc`, and `set` compile to no-op behavior;
- `snapshot` returns a zeroed `PerfSnapshot`.

When enabled, counters use process-global `AtomicU64` values with relaxed ordering.

In test builds with `perf-counters`, increments are further gated by a thread-local enable flag. `test_lock()` acquires a global mutex and enables counting for the current test thread; dropping its guard disables counting.

This prevents cross-test counter contamination but means a test must explicitly acquire the lock before expecting increments.

### 4.9 Benchmark path

```text
tui_perf binary
  → iyon_tui::perf_bench::run()
  → generic fixture construction
  → private resolve/layout/paint/history pipeline
  → perf snapshot
  → JSONL stdout
```

The benchmark does not write result files. It obtains the git SHA from `GIT_SHA` or runs `git rev-parse HEAD`, falling back to `"unknown"`.

Invalid numeric benchmark environment variables panic through `.parse().expect(...)`.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to implementation path

| Semantic operation | Production path | Selection condition | Result/failure behavior |
|---|---|---|---|
| Plain/styled text View | `binding::text_span_*` → `binding::view_text*` → presentation factory | Native text ingress | Returns retained `View`; text-page range validation can return `None` |
| Grid track construction | `binding::grid_track_*` | Native grid decode | Typed value; invalid zero spans panic in span wrappers |
| Grid cell construction | `binding::grid_cell_spec_*` | Native grid decode | Typed value; span zero is rejected explicitly |
| Axis child update | `binding::view_native_axis_set_child` | Native retained update | `Result<View, String>` |
| Axis splice | `binding::view_native_axis_splice` | Native retained update | `Result<View, String>` |
| Grid cell update | `binding::view_native_grid_set_cell` | Native retained update | `Result<View, String>` |
| Path replacement | `binding::view_native_replace_at_path` | Native retained path update | `Result<(View, Vec<View>), String>` |
| Text layout patch | `binding::view_try_with_text_layout_patch*` | Native patch route | Error result for invalid target/path |
| Retained child lookup | `binding::view_try_retained_child` | Native retained inspection | `Result<View, String>` |
| Child replacement | `binding::view_try_replace_retained_children` | Native batch replacement | `Result<View, String>` |
| State attachment | `binding::view_native_with_state_attachment` | `native-host` | `Result<View, String>` |
| Content attachment | `binding::view_native_with_content_attachment` | `native-host` | `Result<View, String>` |
| Weak upgrade | `binding::view_upgrade` | Native retained weak handle | `Option<View>`; no recovery View |
| Scroll key mapping | `ScrollPane` → `map_scroll_key` | Internal component routing | `None` for unsupported key/modifiers |
| Scroll movement | `ScrollPane::scroll_up/down` | Internal component operation | `bool`; no-op at boundary |
| Content replacement in pane | `ScrollPane::set_content` | Internal retained component update | Asserts no component identity; clears extent and repairs detached position |
| ID allocation | `next_nonzero_id` | Internal namespace owner | Monotonic nonzero ID; panic on exhaustion |
| Perf instrumentation | `perf::{add, inc, snapshot}` | `perf-counters` feature | No-op/zero snapshot when disabled |
| Benchmark execution | `perf_bench::run` | Feature-gated binary | JSONL output; malformed env input panics |

### 5.2 Recovery versus fallback

Observed alternate paths are mostly intentional mode selection rather than silent recovery:

- `ScrollPane` measures content only when no host-provided extent exists. This is a cache/hint miss path, not a semantic fallback.
- `ScrollPane::view` uses the un-clipped fill-sized content before valid layout geometry exists. This is an initialization/zero-geometry recovery path.
- `view_upgrade` returns `None` when the weak target is gone. It does not fabricate state.
- Perf-disabled operations intentionally compile away. This is feature specialization, not runtime failure masking.
- `git_sha` falls back to `"unknown"` only for benchmark metadata when the git command fails.
- Benchmark environment parsing uses explicit panic, rather than silently selecting defaults for malformed values.

### 5.3 Search-scoped absence claims

Within the inspected core root and native source search:

- no native `iyon_tui::...` path bypassing `iyon_tui::binding::...` was found;
- no root `pub use` was found in `lib.rs`;
- no root public `prelude` was found;
- no public root module other than `binding` and `perf_bench` was found;
- no `IntoView`, `Renderer`, or `DiffRenderer` export was found in the binding;
- `crates/iyon-tui/tests/ui/fail` and `tests/ui/pass` are present as directories but contained no files in this worktree.

These are search results over the inspected tree, not proof about external consumers or unpublished branches.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 `ScrollPane` retained hints

`ScrollPane` has two small local caches/state hints:

1. `layout_size: Option<Size>`
2. `content_extent: Option<Size>`

Invalidation:

- `set_content` clears `content_extent`;
- changed layout size calls `repair_detached`;
- changed content extent calls `repair_detached`;
- unchanged layout/extent callbacks return early.

If the extent hint is absent, `content_height` invokes `measure_view` at `width.max(1)`. If the hint is present, it avoids remeasurement and uses the supplied height.

There is no scroll scheduler, frame cache, paint cache, or semantic anchor cache in this root helper.

### 6.2 Performance counters

Counters are global, bounded by the fixed `Counter::COUNT` array. Their keys are enum discriminants and their machine-readable names are fixed in `NAMES`.

Observed design properties:

- no dynamic counter map;
- no allocation in `inc`/`add`;
- relaxed atomic updates;
- snapshot copies all counters;
- disabled builds preserve call sites while eliminating work.

The counter surface is intentionally narrower than the internal `perf` module. `set` remains private and is not re-exported through `binding`.

### 6.3 Benchmark work

The benchmark intentionally exercises:

- View clone cost;
- fixture construction;
- retained identity reuse;
- shared-path changes;
- rebuilt equivalent trees;
- layout cache epochs;
- paint cache epochs;
- History projection and rendering;
- perf counters.

The benchmark itself is not part of the ordinary runtime scheduling path. It is feature-gated and requires the `perf-counters` feature in the manifest.

### 6.4 N/A areas

The following are not owned by this assignment:

- layout cache key design;
- paint cache invalidation;
- Scene scheduling;
- History measurement cache;
- application wake/tick scheduling;
- stream/content cache retention.

The binding exposes selected types participating in those systems, but their owners are in other assigned modules.

---

## 7. Tests, benchmarks and observability

### 7.1 Embedded ScrollPane tests

`scroll.rs:223-291` contains focused tests for:

- follow-end behavior and detachment after upward scroll;
- preserving detached position after content growth;
- resuming follow-end after reaching the tail;
- repairing the viewport after resize;
- ensuring allocation growth is not tied to the previous viewport height.

These tests directly exercise the private component owner rather than an external Rust authoring facade.

### 7.2 Binding surface checks

`tools/api-surface/check-binding.ts` provides static checks for:

1. Native seam discipline:
   - scans native Rust files for `iyon_tui::Identifier`;
   - rejects any first identifier other than `binding`.

2. Binding allowlist:
   - extracts `pub use` names from `binding/mod.rs`;
   - compares against `BLESSED_BINDING`;
   - reports added/removed names.

3. Authoring posture:
   - rejects `IntoView`, `Renderer`, and `DiffRenderer`;
   - rejects root public re-exports;
   - allows only `binding` and `perf_bench` root public modules;
   - checks `perf_bench` is hidden and feature-gated;
   - checks `publish = false`;
   - rejects a root binding wholesale re-export;
   - rejects a root `prelude`.

The package script is `check:tui-binding` in `package.json:27`.

The checker is a source-level gate, not a substitute for compiling all feature combinations. Its export parser is intentionally simple and focuses on the current re-export-only structure.

### 7.3 Benchmark observability

`perf_bench::run` emits JSONL containing:

- benchmark name;
- implementation label;
- node count;
- source bytes;
- iteration count;
- median/p95/p99 timing;
- counter object;
- git SHA.

The output is designed for external benchmark tooling and does not mutate repository files.

### 7.4 Validation not performed

I did not run:

- `bun run check:tui-binding`;
- `cargo test -p iyon-tui --lib`;
- `cargo test -p iyon-tui-native`;
- `cargo test --workspace`;
- `cargo check --all-features`;
- `cargo fmt --all -- --check`;
- benchmark binaries;
- ABI generation or generated-ABI validation.

Therefore, the report confirms source structure and static call paths but does not claim current compilation or check success.

### 7.5 Visibility of test-only contracts

`test-util` is a manifest feature, but the inspected root does not expose a public `testing` module. The feature is used in internal source locations such as application kernel/host and Markdown test hooks. The README explicitly states that the old external Rust testing facade was removed.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Public root posture is intentionally narrow

Current source and checker agree on this model:

```text
crate root:
  binding       public, unsupported native seam
  perf_bench    public only for feature-gated package binary, doc-hidden
  everything else private or crate-visible
```

This is stronger than merely marking modules `#[doc(hidden)]`; most runtime modules are actually inaccessible from an external crate.

### 8.2 `pub` item declarations do not imply public crate APIs

Many implementation modules contain `pub` structs/enums/functions because:

- child modules need access;
- `binding` needs to re-export selected items;
- in-crate unit tests need the owner types;
- separate native crate linkage requires a deliberate facade.

The ancestor module visibility prevents those items from being reachable at their original paths. Examples include:

- `presentation::api::*`
- `presentation::ir::View`
- `application::host::*`
- `retained_state::*`
- `history::*`
- `theme::*`

The binding is therefore the explicit visibility bridge rather than a compatibility alias at the root.

### 8.3 Historical export-count disagreement

`iyon-tui.md:13-16` describes the current binding as having 124 operation/passive-type exports. The current checker’s `BLESSED_BINDING` list corresponds to that current expanded surface.

The historical `reports/pre-v5-l1/L1-01-report.md` describes an earlier 78-item binding stage. That report is historical and not authoritative for this baseline. The current source/checker/docs represent a later expanded export set.

### 8.4 Historical module-layout disagreement

The archived lowering handoff proposed a possible multi-file binding layout:

```text
binding/mod.rs
binding/structural.rs
binding/state.rs
binding/content.rs
binding/host.rs
```

The current implementation instead has only `binding/mod.rs`, and it is re-export-only. This is a historical proposal versus current source, not a current contradiction.

### 8.5 Public bridge versus benchmark exception

`lib.rs:5-7` calls `binding` the only public bridge, while `lib.rs:28-34` also exposes `perf_bench`. The surrounding comments qualify the latter as executable-only tooling, and `iyon-tui.md:26-32` documents the exception. The intended contract is therefore “only public runtime/native bridge,” not literally only public module under all feature configurations.

### 8.6 Binding type breadth versus unsupported authoring

The binding re-exports types such as `Theme`, `History`, `Output`, `TextInput`, `View`, and style records that may have public inherent methods. However:

- they are not reachable through ordinary crate-root paths;
- operation-specific construction helpers are used for internal/native ingress;
- the checker excludes the old authoring traits and renderer ecosystem;
- the crate is unpublished;
- the documentation explicitly labels the seam unsupported.

The current contract is therefore “narrow unsupported native integration seam,” not “zero public methods on every type crossing the seam.”

### 8.7 Framework boundary compliance

No inspected symbol in this scope gives generic framework types Iyon-specific meaning. The counter names and binding records describe generic View, content, style, host, History, and measurement mechanics.

---

## 9. Open questions and coverage gaps

1. **Feature compilation was not rerun.**  
   Source inspection shows matching gates, but default, `native-host`, `perf-counters`, `test-util`, all-features, and native configurations were not compiled during this investigation.

2. **Public method surface of every binding type was not exhaustively enumerated.**  
   The binding export names are known, but some re-exported types such as `Theme`, `History`, `TextInput`, `StyleSpec`, and `Output` may have public inherent methods. Their external usability is constrained by the unsupported/unpublished contract, but a full rustdoc/API extraction would be required to enumerate every callable method.

3. **No external consumer inventory exists by design.**  
   Repository search found the native crate as the intended consumer. It cannot prove that no out-of-tree Rust consumer depends on `iyon_tui::binding`.

4. **The binding checker is source-text based.**  
   It checks current `pub use` exports and simple native path patterns. It does not type-check arbitrary aliases, macro-generated paths, or all possible Rust visibility forms. The current source is re-export-only, so this limitation does not correspond to an observed violation.

5. **Generated ABI coupling was not traced in full.**  
   Native/generated ABI users were indexed, but the ABI generator, generated files, and TypeScript transport are outside this assignment’s primary scope.

6. **Counter semantics under concurrent benchmark/native use were not executed.**  
   The source uses relaxed atomics and fixed arrays. Actual benchmark/native counter values and reset races were not observed.

7. **ScrollPane component-identity assertion coverage is local only.**  
   The source asserts that content cannot contain component identity, but no separate test in the inspected root helper explicitly exercises the panic path.

8. **The empty `tests/ui` directories may reflect cleanup rather than an intentionally documented permanent state.**  
   The crate README says the old external authoring/trybuild contract was removed, but no current standalone UI test files were present to verify that removal.

---

## 10. Evidence appendix

### 10.1 Fully inspected assigned production files

- `crates/iyon-tui/src/lib.rs`
  - module declarations and visibility
  - crate-internal aliases
  - public binding declaration
  - hidden benchmark exception
- `crates/iyon-tui/src/binding/mod.rs`
  - complete re-export facade
  - all `native-host` and `perf-counters` gates
- `crates/iyon-tui/src/id.rs`
  - complete allocator helper
- `crates/iyon-tui/src/scroll.rs`
  - complete ScrollPane state, routing, layout, and embedded tests
- `crates/iyon-tui/src/scroll_command.rs`
  - complete key-to-command mapper
- `crates/iyon-tui/src/text.rs`
  - complete internal text re-export shim
- `crates/iyon-tui/src/perf.rs`
  - complete counter enum, snapshot, and instrumentation operations
- `crates/iyon-tui/src/perf_bench.rs`
  - complete benchmark helper and JSONL runner
- `crates/iyon-tui/src/bin/tui_perf.rs`
  - complete binary entrypoint
- `crates/iyon-tui/Cargo.toml`
  - complete package manifest and feature definitions

### 10.2 Supporting source inspected for public-seam proof

- `Cargo.toml`
  - workspace members, package defaults, dependencies, lints
- `crates/iyon-tui-native/Cargo.toml`
  - feature forwarding and core dependency configuration
- `crates/iyon-tui/README.md`
  - Rust publication/authoring contract and binding boundary
- `iyon-tui.md`
  - root visibility and binding enforcement description
- `tools/api-surface/check-binding.ts`
  - native seam scanner, binding allowlist, root posture checks
- `package.json`
  - `check:tui-binding` script
- `crates/iyon-tui/src/presentation/mod.rs`
  - private presentation boundary and operation-specific binding implementation module
- `crates/iyon-tui/src/presentation/api/grid.rs`
  - private grid constructors and binding-relevant passive records
- `crates/iyon-tui/src/presentation/ir.rs`
  - `View`, `RetainedPathStep`, `WeakView`, and native retained method visibility

### 10.3 Consumer paths indexed

- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/theme_dto.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`

The native source search found `iyon_tui::binding` imports and qualified paths only; no native `iyon_tui::` access outside the binding namespace was found.

### 10.4 Historical/context documents inspected

- `AGENTS.md`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md`
- `reports/pre-v5-l1/L1-01-report.md`

Historical reports were used only to identify intended boundary rationale and to label count/layout discrepancies. Current source remains authoritative.

### 10.5 Indexed but not fully read for this assignment

The following are owned by other assignments and were not treated as part of this report’s implementation scope:

- application runtime internals beyond binding-relevant symbols;
- component registry/graph internals;
- content and semantic text implementation;
- History implementation;
- projection implementation;
- retained-state implementation;
- Scene/layout/paint implementation;
- terminal/backend implementation;
- native generated ABI bodies;
- TypeScript public/runtime facade.

### 10.6 Static inspection methods

Evidence came from:

- repository file inventory;
- exact source searches with line numbers;
- complete reads of assigned root files;
- visibility and feature-gate inspection;
- binding export and native-consumer path searches;
- manifest and package-script inspection.

No source edits, dependency installation, process startup, build, benchmark, or test execution was performed.