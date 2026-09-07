# 17 — native structure-state

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- The atlas contract and README identify this as the post-PERF-13 current-state inventory, not a V5 design or migration exercise.
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md` and `docs/architecture/atlas-4355c02/README.md` were read first.
- `PRE-V5-ARCHITECTURE-REPORT.md` was read for context and evidence expectations. Its historical migration/disposition conclusions are not treated as current source facts.

### Primary scope

Assignment 17 covers the handwritten native structural/state entrypoints under:

```text
crates/iyon-tui-native/src/
```

The practical ownership boundary is:

| File | Assignment 17 treatment |
|---|---|
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Fully owned: handwritten structural ABI runtime, semantic publication, identities, leases, paths, builders, patching, transactions, constructors, decode/failure behavior |
| `crates/iyon-tui-native/src/tui/view_state.rs` | Fully owned: retained-state N-API wrapper lifecycle and geometry/presentation envelope decoding |
| `crates/iyon-tui-native/src/tui.rs` | Partially owned: native host lifecycle, View-ref resolution, state-wrapper creation, environment/runtime glue, and shared color helpers; content/source/control portions belong primarily to Assignment 18 |
| Generated ABI files | Indexed and followed at their call boundaries, but not treated as handwritten ownership |
| `crates/iyon-tui-native/src/tui/theme_dto.rs` | Cross-referenced only for shared color/theme decoding; theme and border DTO ownership is outside this assignment’s primary structural/state slice |
| `crates/iyon-tui-native/src/content_ffi.rs`, `error.rs`, `sync.rs` | Indexed/cross-referenced, but primarily outside this assignment |

The central distinction is that the native addon does not itself own the semantic `View` implementation. It owns the environment-scoped ABI runtime and translates wire-level inputs into canonical `iyon_tui::binding` values. The canonical retained View and state implementations remain in the Rust `iyon-tui` crate.

### Investigation method

This was a read-only static investigation. I used repository file discovery and content searches over the native sources, generated ABI boundary files, TypeScript structural callers, and the Rust retained-state host seams. No production or test source was changed, no dependencies were installed, and no build/test/benchmark suite was executed in this session.

Facts below are classified as:

- **Current source fact** — directly visible in the inspected source.
- **Static inference** — behavior reconstructed from call paths and type/storage relationships.
- **Unknown/unverified** — not established because execution or a broader owner’s source was not inspected.

---

## 1. Responsibility and structure

### 1.1 Native module structure

`crates/iyon-tui-native/src/lib.rs` declares the native crate modules:

```rust
mod content_ffi;
mod error;
mod sync;
mod tui;
```

The structural/state implementation is nested under the private `tui` module:

```rust
mod theme_dto;
mod view_abi;
mod view_state;
```

Evidence: `crates/iyon-tui-native/src/lib.rs:8-15` and `crates/iyon-tui-native/src/tui.rs:34-38`.

The native crate exports only the public smoke probe and version helper from `lib.rs`; the N-API exports are discovered through `#[napi]` declarations in the private modules.

### 1.2 Owned handwritten files and physical size

LOC counts are physical line-range estimates, not compiler/token counts. Blank lines and comments remain in the reported physical counts. Test ranges are separated at the module’s `#[cfg(test)]` boundary.

| File | Approx production LOC | Approx test LOC | Main responsibility | Assignment treatment |
|---|---:|---:|---|---|
| `crates/iyon-tui-native/src/tui/view_abi.rs` | ~4,564 | ~1,710 | Environment-scoped native View ABI, refs, semantic cache, leases, path refs, axis builders, edit transactions, structural constructors, text/style decoders | Fully owned |
| `crates/iyon-tui-native/src/tui/view_state.rs` | ~644 | ~318 | `NativeViewState` N-API wrapper, fixed envelope validation, geometry/presentation value decoding, wake conversion | Fully owned |
| `crates/iyon-tui-native/src/tui.rs` | ~1,960 total production, mixed | ~55 | Native host object and all other N-API wrappers; structural/state-relevant environment and host methods; content/control methods also reside here | Partial ownership |
| `crates/iyon-tui-native/src/generated/view_abi_exports.rs` | Generated; not counted | Generated | Checked ABI invocation wrappers, pointer/buffer/count/node-id validation, panic boundary | Indexed, not handwritten ownership |
| `crates/iyon-tui-native/src/generated/view_abi_napi.rs` | Generated; not counted | Generated | N-API methods forwarding to ABI symbols | Indexed, not handwritten ownership |
| `crates/iyon-tui-native/src/generated/view_abi_types.rs` | Generated; not counted | Generated | ABI/type constants and metadata | Indexed, not handwritten ownership |
| `crates/iyon-tui-native/src/generated/view_abi_table.rs` | Generated; not counted | Generated | Function descriptor table and ABI function metadata | Indexed, not handwritten ownership |
| `crates/iyon-tui-native/src/generated/view_state_schema.rs` | Generated; not counted | Generated | State property IDs, lane offsets, masks, nullability, and envelope shape checks | Indexed, not handwritten ownership |
| `crates/iyon-tui-native/src/tui/theme_dto.rs` | ~598 total | Included in total | Theme/border DTO decoding; shares color helper with state and structural paths | Cross-boundary only |

`view_abi.rs` has its production implementation through approximately line 4,564 and its tests beginning around line 4,565. Its test inventory is particularly important architectural evidence because it covers identity reuse, lease transfer, failed host installation, stale weak slots, path recovery, multi-edit transaction behavior, malformed buffer lanes, and alternate text encodings.

### 1.3 Primary and secondary responsibilities

#### `view_abi.rs`

Primary responsibilities:

1. Own one `NativeViewRuntime` per N-API environment.
2. Validate ABI-level pointers, refs, node IDs, enum values, buffer lengths, and packed fields.
3. Resolve semantic `NodeId` identity to native `View` values.
4. Maintain:
   - `NodeId -> WeakView` semantic cache;
   - `NodeId -> NativeRef` association;
   - native-ref slot table;
   - lease counts and strong leases;
   - interned path metadata;
   - temporary axis builders;
   - temporary edit transactions;
   - interned style atoms and styles.
5. Materialize canonical `iyon_tui::binding::View` values.
6. Publish new Views through one centralized identity/lease routine.
7. Stage and commit structural path replacements and multi-edit text transactions.
8. Surface compact status codes and a status-detail side channel.

Secondary responsibilities:

- Runtime metadata and memory diagnostics.
- Weak-cache maintenance and page reclamation.
- Direct-FFI qualification symbols.
- Text/style decoding shared by multiple constructor routes.
- N-API session creation and environment cleanup.

#### `view_state.rs`

Primary responsibilities:

1. Wrap canonical `HostViewState` as `NativeViewState`.
2. Guard wrapper use after disposal.
3. Decode fixed geometry and presentation envelopes.
4. Distinguish:
   - property absent;
   - property explicitly null;
   - property set to a value;
   - property explicitly cleared.
5. Convert decoded values into canonical `ViewStateGeometryPatch` and `ViewStatePresentationPatch`.
6. Return a primitive wake bit rather than exposing Rust effect internals.

It deliberately does **not** implement state records, effect classification, retained frame capture, layout invalidation, or presentation application. Its module documentation states that those remain in `iyon-tui`’s `retained_state` implementation (`view_state.rs:1-9`).

#### Structural/state-relevant portions of `tui.rs`

The relevant portions are:

- Environment registries and cleanup:
  - `HOST_ENVIRONMENTS`
  - `CONTENT_ENVIRONMENTS`
  - `host_environment_for_env`
  - `content_environment_for_identity`
- General wrapper validation:
  - `ensure_alive`
  - `resolve_native_view`
- Native host wrapper:
  - `NativeTuiHost`
  - `new`
  - `epochs`
  - `set_desired_view_ref`
  - `clear_view_state_bindings`
  - `flush_pending_hosts`
  - `dispose`
  - `view_state`
- Shared color and attribute decoders:
  - `color_spec_str`
  - `parse_rgb_hex`
  - `text_attribute`

Evidence: `crates/iyon-tui-native/src/tui.rs:42-146`, `:265-296`, `:602-846`, and `:1858-1961`.

`NativeHistory`, content source/port/connector, input controls, `NativeViewSlot`, and `NativeScrollPane` are in the same physical file but are primarily Assignment 18 material. They consume native View refs, so they are relevant as reverse consumers, but their own lifetime and behavior are not recounted as Assignment 17 implementation ownership.

---

## 2. Types, APIs and contracts

### 2.1 Native runtime and ABI identity types

The core runtime type is:

```rust
pub(super) struct NativeViewRuntime
```

Evidence: `view_abi.rs:357-397`.

Its identity and lifetime fields are:

```rust
magic: u32,
abi_version: u32,
semantic_version: u32,
alive: AtomicU32,
owner_thread: ThreadId,
generation: u32,
```

Its principal maps/tables are:

```rust
nodes: HashMap<u64, WeakView>,
slots: NativeRefTable,
node_refs: HashMap<u64, u32>,
path_nodes: HashMap<u32, PathNode>,
path_keys: HashMap<PathKey, u32>,
builders: HashMap<u32, AxisBuilder>,
edit_txns: HashMap<u32, EditTxn>,
style_atoms: HashMap<u32, String>,
styles: HashMap<u32, StyleRef>,
```

The runtime is held by:

```rust
pub(super) type ViewRuntimeHandle = Arc<NativeViewRuntime>;
```

and registered globally by environment key in:

```rust
static RUNTIME_HANDLES: OnceLock<Mutex<HashMap<usize, ViewRuntimeHandle>>>;
```

Evidence: `view_abi.rs:1327-1355`.

The `Arc` is the actual native runtime lifetime owner. N-API session objects hold an `Arc`; raw ABI calls receive a pointer derived from the `Arc`, but pointer validity is checked against the environment registry and runtime header.

### 2.2 Native View slot and lease contract

Each native View slot is:

```rust
struct NativeViewSlot {
    node_id: u64,
    weak: WeakView,
    leased: Option<View>,
    js_lease_count: u32,
    kind: NativeViewKindTag,
}
```

Evidence: `view_abi.rs:120-131`.

The slot has two separate retention mechanisms:

- `weak`: semantic cache/reference without keeping the View alive;
- `leased`: a strong `View` retained while one or more JS/native leases exist.

`js_lease_count` tracks the number of caller-owned leases. A slot can therefore be:

1. live and leased;
2. live but unleased through a weak View;
3. expired and removable;
4. invalid/stale.

The native-ref table is dense paged storage:

```rust
const NATIVE_REF_PAGE_BITS: u32 = 12;
```

A page has 4,096 slot positions. Refs are monotonic and are not recycled within a runtime generation. Pages with zero live slots are dropped, but the outer directory retains its high-water shape.

Evidence: `view_abi.rs:133-280`.

The semantic meaning is important: a `NativeRef` is not itself the semantic identity. It is an environment-local handle associated with a `NodeId`. `NodeId` is the semantic identity; `NativeRef` is the current native publication/lease handle.

### 2.3 Node ID validity

`node_id(low, high)` combines two 32-bit lanes into a `u64` and rejects:

- zero;
- a high lane above `0x001f_ffff`.

Evidence: `view_abi.rs:1685-1690`.

The high-lane cap constrains IDs to the JavaScript safe-integer range. Generated wrappers perform the same check through `generated_node_id` (`generated/view_abi_exports.rs:1324-1331`).

### 2.4 Publication identity contract

The central helper is:

```rust
fn publish_semantic_view(
    &mut self,
    node_id: u64,
    view: View,
    lease: PublicationLease,
) -> Result<u32, u32>
```

Evidence: `view_abi.rs:1083-1124`.

It is intended to be the only route that mints or re-associates a native ref for a semantic `NodeId`. Its rules are:

1. Reject zero `NodeId`.
2. Consult existing semantic identity.
3. Reject a live identity conflict where the same `NodeId` maps to a different `View`.
4. Reuse an existing `NativeRef` for the exact same live View.
5. Acquire an additional lease for `PublicationLease::Leased`.
6. Do not acquire a lease for `PublicationLease::Weak`.
7. Allocate/install a new ref for a fresh identity.
8. Store the weak semantic cache entry and node-ref association.

The two lease modes are:

```rust
pub(super) enum PublicationLease {
    Leased,
    Weak,
}
```

- `Leased`: used by ordinary generated constructors and returned to a caller.
- `Weak`: used for intermediate path publications that do not represent a standalone JS-owned root.

Evidence: `view_abi.rs:145-166` and `:1130-1135`.

### 2.5 Structural ABI entrypoints

The handwritten ABI implementation exposes the following structural groups.

#### Runtime and render

- `runtime_noop_impl`
- `view_status_detail_impl`
- `view_render_ref_impl`
- `host_render_ref_impl`
- `view_ref_for_node_id_impl`
- `view_release_many_impl`

#### State attachment

- `view_state_attach_impl`

#### Path identity

- `path_root_impl`
- `path_child_impl`

#### Text/layout patches

- `view_text_layout_patch_root_impl`
- `view_common_patch_root_impl`
- `view_text_layout_patch_path_impl`
- depth-specialized:
  - `view_text_layout_patch_path_d1_impl`
  - `view_text_layout_patch_path_d2_impl`
  - `view_text_layout_patch_path_d3_impl`
  - `view_text_layout_patch_path_d4_impl`

#### Structural constructors and updates

- `view_content_host_create_impl`
- `view_spacer_create_impl`
- `view_axis_create_buffer_impl`
- generated small row/column constructors from `define_small_axis_constructor`
- `axis_builder_begin_impl`
- `axis_builder_push_impl`
- `axis_builder_finish_impl`
- `axis_builder_abort_impl`
- `view_axis_set_child_impl`
- `view_axis_splice_buffer_impl`
- `view_axis_set_child_path_impl`
- `view_grid_set_cell_impl`
- `view_grid_create_buffer_impl`
- `view_grid_set_cell_path_impl`
- `view_hanging_create_impl`
- `view_container_create_impl`
- `view_clamp_create_impl`
- `view_component_create_impl`
- `view_decorated_create_buffer_impl`
- `view_diff_create_buffer_impl`

#### Style/text materializers

- `style_atom_create_cstring_impl`
- `style_create_bits_impl`
- `view_text_create_cstring_impl`
- `view_text_create_utf8_impl`
- `view_text_create_utf8_2_impl`
- `view_text_create_utf8_3_impl`
- `view_text_create_utf8_4_impl`
- `view_text_create_cstring_2_impl`
- `view_text_create_cstring_3_impl`
- `view_text_create_cstring_4_impl`
- `view_text_create_buffer_impl`

The corresponding generated N-API methods are in `generated/view_abi_napi.rs:9-426`; the generated descriptor table identifies these as runtime-owned ABI functions, usually in the `constructor`, `structural_patch`, `state`, or `style_atom` families.

### 2.6 State wrapper API

`NativeViewState` is declared in `view_state.rs:25-29`:

```rust
pub struct NativeViewState {
    state: HostViewState,
    alive: AtomicBool,
}
```

Intentional public N-API methods:

- `dispose`
- `stateId`
- `attachmentId`
- `validateNodeKind`
- `setGeometry`
- `clearGeometry`
- `setPresentation`
- `clearPresentation`
- `setStyleState`
- `clearStyleState`

The wrapper is not the owner of the retained state record. `HostViewState` owns only:

```rust
id: u64,
host: Weak<Mutex<HostInner>>,
```

Evidence: `crates/iyon-tui/src/application/view_state.rs:23-35`.

The host owns the actual `ViewStateRegistry`, `ViewStateRecord`s, mutable overrides, committed immutable snapshots, and binding sets.

### 2.7 State property domains

Generated state schema rows define:

#### Geometry domain

Ten properties:

1. `width`
2. `height`
3. `padding`
4. `minWidth`
5. `maxWidth`
6. `minHeight`
7. `maxHeight`
8. `gap`
9. `alignment`
10. `borderEdges`

Evidence: `generated/view_state_schema.rs:14-100`.

The geometry value lane is exactly:

- 14 `u32` words;
- zero strings.

Nullable geometry properties include min/max dimensions and border edges. The nested Rust representation distinguishes an omitted field from an explicit null:

```text
patch.min_width == None       => property absent
patch.min_width == Some(None) => property explicitly null
patch.min_width == Some(Some(x)) => value
```

#### Presentation domain

Seven properties:

1. `foreground`
2. `background`
3. `borderColor`
4. `borderStyle`
5. `borderGlyphs`
6. `textAttributes`
7. `style`

Evidence: `generated/view_state_schema.rs:147-205`.

The presentation value lane is exactly:

- 5 `u32` words;
- 14 strings.

### 2.8 Wake and effect contract

The native wrapper returns a primitive `u32` wake bit:

```rust
WAKE_SCHEDULE_ENVIRONMENT_DRAIN = 1
```

Evidence: `view_state.rs:173-180` and `generated/view_state_schema.rs:11-12`.

The canonical Rust state layer derives richer `StateEffects`, not the caller:

```rust
RESOLVE_STYLE
PAINT_SELF
PAINT_SUBTREE
DAMAGE
GEOMETRY
MEASURE_SELF
MEASURE_ANCESTORS
PLACE_SELF
PLACE_DESCENDANTS
UPDATE_CLIP
DAMAGE_OLD
DAMAGE_NEW
PROJECT_CONTENT
INTRINSIC_WIDTH
INTRINSIC_HEIGHT
```

Evidence: `crates/iyon-tui/src/retained_state/effects.rs:3-25`.

Presentation mutations produce `RESOLVE_STYLE + DAMAGE + PAINT_SELF`; style-state changes conservatively produce `PAINT_SUBTREE` because descendant selectors may observe inherited state (`effects.rs:52-66`).

Geometry mutations classify their own measurement, placement, intrinsic-size, clipping, and damage consequences in `retained_state/geometry.rs:280-334`.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
TypeScript structural transport
    |
    | generated ABI/N-API calls
    v
crates/iyon-tui-native/src/tui/view_abi.rs
    |
    | resolves/decodes/publishes
    v
iyon_tui::binding
    |
    +--> View / WeakView / TextSpan / StyleRef
    +--> view_native_* canonical factories
    +--> HostTui / host render
    +--> HostViewState
    v
crates/iyon-tui/src/application/host.rs
    |
    +--> retained_state::ViewStateRegistry
    +--> structural scene/layout/paint scheduling
    +--> terminal/backend or headless frame
```

State path:

```text
TypeScript ViewState API
    |
    | fixed set/null/clear masks + word/string lanes
    v
NativeViewState::set_geometry / set_presentation
    |
    | decode_*_envelope
    v
ViewStateGeometryPatch / ViewStatePresentationPatch
    |
    v
HostViewState::set_geometry / set_presentation
    |
    | host lock
    v
HostInner::mutate_view_state
    |
    v
ViewStateRegistry::mutate_record
    |
    +--> ViewStateRecord::apply_geometry/apply_presentation
    +--> immutable committed/candidate state versions
    +--> invalidation and environment wake
```

Structural root path:

```text
Semantic TS View
    |
    | ensureNative / constructor route
    v
view_*_impl
    |
    +--> runtime.ref_for_node_id (identity-first)
    +--> resolve child refs
    +--> decode payload
    +--> canonical View factory
    +--> runtime.publish(...)
    v
NativeRef lease
    |
    v
host_render_ref_impl or NativeTuiHost::set_desired_view_ref
    |
    v
TuiHost::render / set_desired_view
```

### 3.2 Create/destroy/lifetime ownership

| Object | Created by | Stored by | Destroyed/released by |
|---|---|---|---|
| `NativeViewRuntime` | `runtime_handle_for_env` on first environment use | `Arc` in `RUNTIME_HANDLES` and N-API sessions | Environment cleanup hook marks `alive = 0` and removes registry entry |
| Native View semantic cache entry | `install_semantic_view` | `NativeViewRuntime.nodes` as `WeakView` | Weak expiry, `prune_expired`, or explicit drop |
| Native View slot | `install_semantic_view` | paged `NativeRefTable` | `view_release_many`, expired weak cleanup, full sweep |
| Strong View lease | `publish_semantic_view`, `ref_for_node_id`, `acquire_lease` | `NativeViewSlot.leased` plus count | `view_release_many` / one-lease transfer paths |
| Path ref | `path_child` | `path_nodes`, `path_keys` | No normal eviction; runtime teardown |
| Axis builder | `axis_builder_begin` | `builders` | Finish, abort, or runtime cleanup |
| Edit transaction | `edit_txn_begin` | `edit_txns` | Commit removes before host render; explicit abort; host dispose aborts all |
| Style atom/style ref | `style_atom`, `style` | `style_atoms`, `styles` | No normal eviction; runtime teardown |
| `NativeViewState` wrapper | `NativeTuiHost::view_state` | N-API object containing `HostViewState` | `dispose`; host registry remains owner until detached/disposed |
| `HostViewState` record | `TuiHost::create_view_state` / host registry | `ViewStateRegistry.records` | Explicit disposal after unbound, or host teardown |

### 3.3 Host/environment ownership coupling

`NativeTuiHost::new` obtains:

1. an environment-scoped `TuiEnvironment` through `host_environment_for_env`;
2. a `TuiHost` inside a `Box`;
3. an environment-scoped View ABI runtime pointer.

Evidence: `tui.rs:611-634`.

The host wrapper therefore bridges two independently tracked native lifetimes:

- the `Box<TuiHost>` owned by `NativeTuiHost`;
- the `Arc<NativeViewRuntime>` indirectly held in the global environment registry.

The ABI runtime is environment-owned rather than host-owned. This is consequential because ABI edit transactions intentionally have no host parameter at begin time; `NativeTuiHost::dispose` explicitly calls:

```rust
view_abi::abort_all_edit_txns(self.view_runtime as *mut ...)
```

before closing the host (`tui.rs:742-750`).

### 3.4 Thread ownership

`NativeViewRuntime::valid_on_owner_thread` checks:

- ABI magic;
- ABI version;
- semantic version;
- alive flag;
- exact `ThreadId`.

Evidence: `view_abi.rs:444-450`.

`runtime_mut` rejects null and invalid/wrong-thread pointers. `runtime_from_handle` returns an N-API closing error for disposed or wrong-thread use (`view_abi.rs:1440-1452`).

This is a strict owner-thread design. The runtime maps themselves are not protected by a mutex; the global environment registry is mutex-protected, but individual runtime mutation assumes all ABI calls occur on the recorded owner thread.

---

## 4. Execution paths and state transitions

### 4.1 Native runtime initialization

The first call requiring a View ABI session invokes:

```rust
runtime_handle_for_env(&env)
```

Behavior:

1. Compute environment key from `env.raw()`.
2. Lock `RUNTIME_HANDLES`.
3. Reuse an existing `Arc` if present.
4. Otherwise create `Arc::new(NativeViewRuntime::new())`.
5. Register an environment cleanup hook.
6. Store the `Arc` in the registry.

The runtime starts with:

- generation `1`;
- `next_native_ref = 1`;
- fixed path root `PATH_ROOT_REF = 0x4000_0001`;
- empty node/ref/style/builder/transaction maps;
- alive flag set.

Evidence: `view_abi.rs:399-440` and `:1331-1355`.

The `generation` field is exposed in metadata and diagnostics, but static search of `view_abi.rs` found only initialization to `1`; no increment/reassignment is present in this file. Environment teardown is instead represented by removing the runtime registry entry and setting `alive = 0`. Whether another module changes generation is not shown by this source and remains unverified.

### 4.2 Ordinary View materialization

The ordinary constructor pattern is:

```text
ABI wrapper validates raw inputs
    ↓
implementation validates semantic node id
    ↓
runtime.ref_for_node_id(node_id)
    ├── success: return existing lease-bearing ref
    ├── cache miss: continue
    └── other error: return status
    ↓
decode payload / resolve child refs
    ↓
canonical view_native_* factory
    ↓
runtime.publish(node_id, view)
    ↓
record status and return NativeRef
```

Examples:

- text: `view_text_create_cstring_impl` (`view_abi.rs:4045-4075`);
- UTF-8: `view_text_create_utf8_impl` (`:4079-4121`);
- axis buffer: `view_axis_create_buffer_impl` (`:2609-2652`);
- spacer: `view_spacer_create_impl` (`:2321-2348`);
- container: `view_container_create_impl` (`:3616-3643`);
- decorated: `view_decorated_create_buffer_impl` (`:3504-3554`).

The canonical factory result is a complete immutable semantic `View`. Native code does not retain a mutable per-node structural object. Structural updates create a new persistent View and publish it under a new or requested semantic `NodeId`.

### 4.3 Identity-first behavior

`ref_for_node_id` provides cross-transport recovery:

1. If `node_refs` has a ref:
   - resolve it;
   - if it is already leased, increment its lease count;
   - if it is weak-only, convert it to one strong lease;
   - return the same ref.
2. If the ref is stale, remove the stale node-ref mapping.
3. Consult `nodes` for a live weak View.
4. If a live weak View exists, call ordinary leased publication to promote it.
5. Otherwise return `FAST_CACHE_MISS`.

Evidence: `view_abi.rs:1137-1164`.

This means a constructor may be called through a different transport family and still recover the same native semantic object by `NodeId`. The recovered result carries a caller-owned lease.

Every generated constructor consults this cache before consuming payload. For example, text, axis, grid, diff, decorated, container, clamp, hanging, and spacer implementations all return early when `ref_for_node_id` succeeds.

Consequential semantics: if the semantic identity is already live, malformed or stale constructor payload may be ignored because the payload is never parsed. This is intentional identity-first behavior, not an accidental compatibility fallback. It is tested in `constructor_consults_semantic_cache_before_building_perf12_s23` and related tests.

### 4.4 Semantic identity replacement

`consult_semantic_identity` distinguishes:

```rust
SameLiveWithRef(View)
SameLiveWithoutRef
Conflict
Fresh
```

Evidence: `view_abi.rs:145-157` and `:1027-1061`.

Rules:

- Existing same `NodeId` and equal live View: reuse/promote.
- Existing same `NodeId` and different live View: `FAST_INVALID`.
- Expired weak metadata: remove and treat as fresh.
- Same live View without a currently associated ref: allocate/install a ref.

`install_semantic_view` writes both maps and installs a slot:

```text
nodes[node_id] = downgrade(view)
node_refs[node_id] = reference
slots[reference] = NativeViewSlot { ... }
```

Evidence: `view_abi.rs:1064-1080`.

### 4.5 State attachment path

`view_state_attach_impl` is a structural transformation that attaches retained-state identity to an existing state-capable View:

```text
resolve base ref
    ↓
validate nonzero state id
    ↓
validate View kind is state-capable
    ↓
if already attached to same state id:
    return base ref
    ↓
require node_refs[node_id] == base_ref
    ↓
canonical view_native_with_state_attachment(base, state_id)
    ↓
temporarily remove old semantic cache/node-ref association
    ↓
publish attached View under same NodeId
    ↓
release the ordinary base lease
    ↓
return new stateful ref
```

Evidence: `view_abi.rs:1780-1833`.

The implementation specifically requires the supplied `base_ref` to be the currently associated ref for the node (`:1807-1808`). This prevents attaching state to an unrelated View with the same caller-provided NodeId.

On successful replacement, the original constructor lease is consumed and the returned stateful ref owns the replacement lease (`:1818-1823`). On publication failure, the old weak/node-ref mappings are restored (`:1825-1830`).

This is a replacement of the semantic View, not mutation of the original View. State identity lives in the retained View node metadata; state values themselves remain in the host-owned state registry.

### 4.6 Root host installation

The N-API host wrapper exposes:

```rust
NativeTuiHost::set_desired_view_ref(view_ref: i64)
```

Behavior:

1. Check the host wrapper’s `alive` flag.
2. Convert the signed JS integer to positive `u32`.
3. Resolve the ref through the environment runtime.
4. Call `TuiHost::set_desired_view(view)`.
5. Return host ID and whether the environment should be drained.

Evidence: `tui.rs:655-672`.

The method intentionally accepts a retained native root without rendering immediately. The host’s next environment drain performs the frame transaction.

`NativeTuiHost::epochs` exposes:

- host ID;
- desired structural revision;
- visible structural revision;
- visible frame revision;
- pending epoch;
- committed epoch.

Evidence: `tui.rs:637-652`.

### 4.7 Path materialization

Path refs encode a persistent path through retained structure. A path key is:

```rust
struct PathKey {
    parent: u32,
    kind: u32,
    expected_view_kind: u32,
    selector: u32,
}
```

A path node stores:

```rust
struct PathNode {
    parent: u32,
    step: RetainedPathStep,
    depth: u32,
}
```

`path_child` validates:

- parent ref range and existence;
- step kind;
- expected View kind;
- step-kind/View-kind compatibility;
- maximum path depth;
- selector range.

It interns identical `(parent, kind, expected_view_kind, selector)` tuples and returns the existing path ref if found (`view_abi.rs:487-527`).

Although the general path table allows depth up to 128, structural patch entrypoints restrict actual submitted path depth to four. The fixed-arity path functions carry up to four ancestors.

### 4.8 Structural path replacement

`publish_structural_path` performs:

1. Validate path depth and node-id count.
2. Validate base and child refs.
3. Resolve path steps.
4. Resolve base and child Views.
5. Call `view_native_replace_at_path`.
6. Validate one resulting View per supplied NodeId and root placement.
7. Validate semantic identity consistency.
8. Prepare all publication refs.
9. Commit all publication entries together.

Evidence: `view_abi.rs:2547-2605`.

`prepare_staged_publication` ensures:

- all staged NodeIds are nonzero and unique;
- no existing live semantic identity conflicts;
- existing refs are reused where valid;
- stale node-ref entries are removed and replacement refs reserved;
- refs are reserved locally before host acceptance.

Evidence: `view_abi.rs:816-866`.

`commit_staged_publication` then installs the reserved entries, with only the final root receiving a strong lease. Intermediate path nodes are weak-only.

### 4.9 Text layout path patch

`publish_text_path` is similar but differs in commit behavior:

1. Resolve path and base.
2. Decode wrap/alignment.
3. Build changed path Views.
4. Validate NodeId/View pairs.
5. Publish intermediate path nodes using `publish_bulk`.
6. Publish the final root using ordinary leased `publish`.

Evidence: `view_abi.rs:1880-1943`.

Unlike edit transactions and generic structural path replacement, this helper publishes the validated path entries sequentially rather than using `prepare_staged_publication`/`commit_staged_publication`. It does not install a host root itself; the caller subsequently uses the returned root ref.

### 4.10 Axis builders

The builder route is a second structural construction mode, not a different semantic model.

```text
axis_builder_begin
    ↓
builders[builder_ref] = AxisBuilder
    ↓
axis_builder_push resolves child refs and stores owned Views
    ↓
axis_builder_finish removes builder
    ↓
validate exact expected child count and gap
    ↓
canonical axis factory
    ↓
runtime.publish(node_id, view)
```

The builder ref range is:

```text
BUILDER_REF_START = 0x7ffe_0001
BUILDER_REF_LIMIT = 0x7fff_0001
```

Evidence: `view_abi.rs:283-293`, `:550-636`.

Push statuses:

- `-1`: invalid ref/track word;
- `1`: builder or child cache miss;
- `2`: child count exceeds expected count;
- `0`: accepted.

Finish removes the builder before validating final child count and constructing the View. Therefore a failed finish does not leave the builder retained. Explicit abort removes the builder and returns `0`; unknown valid-range builder returns `1`.

### 4.11 Edit transaction lifecycle

The edit transaction route is:

```text
edit_txn_begin(base_root_ref, expected_edit_count)
    ↓
resolve and retain base View
    ↓
edit_txns[txn_ref] = EditTxn
    ↓
edit_txn_add_text_layout(...)
    ↓
validate/intern path and stage edit metadata
    ↓
edit_txn_commit_render(host, txn_ref)
    ↓
remove txn from runtime
    ↓
build edit trie
    ↓
rebuild changed leaves and shared ancestors
    ↓
prepare semantic publication
    ↓
host.render(new_root)
    ├── failure: old host root remains authoritative
    └── success: commit staged refs and return root ref
```

The transaction ref range is:

```text
EDIT_TXN_REF_START = 0x7fff_0001
EDIT_TXN_REF_LIMIT = 0x8000_0000
```

Limits:

- maximum edit count: `256`;
- maximum new text bytes: `16 MiB`;
- maximum staged objects: `4,096`;
- submitted path depth: `4`.

Evidence: `view_abi.rs:288-294`, `:638-718`.

`EditTxn` retains the base View strongly while staged:

```rust
struct EditTxn {
    base_root_ref: u32,
    base_view: View,
    expected_edit_count: u32,
    staged_text_bytes: u32,
    edits: Vec<TextLayoutEdit>,
}
```

`build_edit_trie` rejects:

- empty transactions;
- missing/invalid paths;
- wrong path depth;
- zero NodeIds;
- inconsistent shared node identities;
- a path ending at another edit or containing descendants under an edit;
- staged-object overflow.

`stage_edit_trie` applies leaf text-layout patches first and rebuilds each changed parent once, so multiple edits sharing ancestors result in one shared ancestor rebuild.

Evidence: `view_abi.rs:721-814`.

### 4.12 Retained-state mutation lifecycle

The state route is intentionally separate from structural publication:

```text
NativeViewState.setPresentation(...)
    ↓
decode_presentation_envelope(...)
    ↓
HostViewState.set_presentation(...)
    ↓
HostInner lock
    ↓
ViewStateRegistry::mutate_record
    ↓
ViewStateRecord::apply_presentation
    ↓
revision/effect update
    ↓
host.invalidate_state
    ↓
wake bit
```

The record mutates only after the native envelope has fully decoded. The wrapper comment states that malformed envelopes never leave a partial override (`view_state.rs:68-70`).

Canonical record behavior:

- clone current overrides;
- apply candidate patch to clone;
- return no-op if effective values are unchanged;
- validate geometry against desired/visible kind;
- replace the stored override set only after validation;
- increment logical and domain revision;
- derive effects.

Evidence: `crates/iyon-tui/src/retained_state/record.rs:71-164`.

The host registry retains mutable records in boxed storage and publishes immutable versions only when a record is demanded by desired, visible, or in-flight bindings. Unmounted state remains mutable source-of-truth without an eagerly duplicated committed snapshot (`retained_state/registry.rs:20-31`, `:84-120`).

---

## 5. Alternate routes and failure semantics

### 5.1 Status vocabulary

Handwritten ABI status constants:

```text
FAST_INVALID    = 0x8000_0001
FAST_CACHE_MISS = 0x8000_0004
FAST_REFUSED    = 0x8000_0005
FAST_INTERNAL   = 0x8000_0006
```

Host render statuses:

```text
HOST_STATUS_OK          = 0
HOST_STATUS_CACHE_MISS  = 1
HOST_STATUS_INVALID     = -1
HOST_STATUS_INTERNAL    = -3
```

Evidence: `view_abi.rs:49-72`.

The status detail side channel uses:

```text
STATUS_DETAIL_CHILD_INDEX = 0x4000_0000
STATUS_DETAIL_BASE_REF    = 0x8000_0000
```

A child cache miss records the child ordinal in the low 30 bits. A base-ref miss records the base marker. `view_status_detail_impl` returns the last recorded detail (`view_abi.rs:1692-1722`).

The TypeScript generated wrapper treats any result with the high error bit as a `NativeAbiStatusError`; it reads the detail channel only for `FAST_CACHE_MISS` (`packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts:7-27`).

### 5.2 Semantic operation to production route

| Semantic operation | Production route | Selection/alternative | Failure behavior |
|---|---|---|---|
| Create text View | `view_text_create_cstring_impl`, UTF-8 fixed routes, or buffer route | CString for NUL-terminated input; exact UTF-8 for embedded/trailing NUL; fixed arity for 1–4 spans; buffer for wider spans | Invalid UTF-8/framing/style -> `FAST_INVALID`; oversized text -> refusal/checked wrapper rejection; existing live NodeId bypasses payload |
| Create axis | small row/column constructor, `view_axis_create_buffer_impl`, or builder route | Fixed small arity, bulk buffer, or incremental builder | Invalid axis/track/gap/ref -> `FAST_INVALID`; stale child -> cache miss with child detail; over-limit -> refusal |
| Replace axis child | `view_axis_set_child_impl` | Direct persistent sequence update | Base miss gets base detail; child miss gets child detail; structural factory error -> invalid; no publication on decode/factory failure |
| Splice axis | `view_axis_splice_buffer_impl` | Buffer child lane plus persistent splice | Buffer framing/ref failures before publish; stale inserted child reports child detail |
| Replace nested path | `view_axis_set_child_path_impl`, `view_grid_set_cell_path_impl` | Interned path ref, fixed max depth 4 | Path mismatch, NodeId mismatch, wrong View kind -> invalid; stale path -> cache miss; validated structural path uses staged publication |
| Patch text layout root | `view_text_layout_patch_root_impl` | Root-only persistent patch | Existing NodeId short-circuits; invalid wrap/alignment/base -> invalid/cache miss |
| Patch text layout nested path | `view_text_layout_patch_path[_d1..d4]_impl` | Generic or depth-specialized fixed route | Path/base/identity errors fail; intermediate nodes weak-published, final root leased |
| Apply common geometry patch | `view_common_patch_root_impl` | Packed scalar mask | Masked fields decode locally before final View construction; invalid masked value -> invalid; `decoration_ref` is resolved if nonzero although not otherwise consumed |
| Attach state identity | `view_state_attach_impl` | Structural replacement of an existing ref | Base must be state-capable and currently associated with NodeId; failed replacement restores old semantic mappings |
| Create decorated View | `view_decorated_create_buffer_impl` | Words+bytes framed decoration payload | Child cache miss includes child detail; malformed decoration, style atoms, UTF-8, or framing -> invalid |
| Create diff View | `view_diff_create_buffer_impl` | Words+bytes diff payload | Existing semantic cache short-circuits parsing; malformed ranges/line metadata/UTF-8 -> invalid |
| Create content host | `view_content_host_create_impl` | Host View carrying content-port identity | Zero port ID invalid; existing NodeId short-circuits; canonical factory failure -> invalid |
| Render a native ref | `host_render_ref_impl` | Direct host render path | Invalid/disposed host -> host invalid; stale View ref -> host cache miss; host render failure -> host internal |
| Install desired root | `NativeTuiHost::set_desired_view_ref` | N-API host method, not low-level ABI render | Wrapper validation errors become `ION_INVALID_INPUT`; host content errors use `NativeError::content`; accepted operation schedules a later drain |
| Set state geometry/presentation | `NativeViewState::set_*` | Fixed envelope path only | Header/lane/value failure occurs before mutation; canonical state mutation errors become `ION_INVALID_INPUT`; wake bit returned on accepted effect |
| Clear state | `NativeViewState::clear_*` | Fixed clear envelope, optional all flag | Set/null/value lanes are prohibited; clear-all returns `None` property list; invalid clear envelope does not mutate |
| Release refs | `view_release_many_impl` | Batch release | Generated wrapper validates pointer/capacity/count; zero leases saturating behavior is implemented in the batch path; weak-expired zero-lease slots are reclaimed or queued |
| Commit multi-edit transaction | `edit_txn_commit_render_impl` | One transaction with trie rebuild and host render | Transaction removed before build; build/prepare failures publish nothing; host render failure leaves old host root and does not commit staged refs; explicit abort is available |

### 5.3 Decode alternatives

#### CString versus UTF-8

CString routes:

- `view_text_create_cstring_impl`;
- `view_text_create_cstring_2_impl`;
- `view_text_create_cstring_3_impl`;
- `view_text_create_cstring_4_impl`.

They call `CStr::from_ptr`, copy to owned UTF-8 `String`, then build styled spans.

Exact UTF-8 routes:

- `view_text_create_utf8_impl`;
- fixed span variants `_2`, `_3`, `_4`;
- `view_text_create_buffer_impl`.

They preserve embedded and trailing NUL bytes because the length is explicit. `utf8_text_spans` validates total span lengths against `used_bytes`, validates the complete UTF-8 buffer once, then creates one `NativeTextPage` and range-backed spans (`view_abi.rs:4124-4168`).

#### Fixed arity versus variadic buffer

The TypeScript materializer selects fixed arity for one through four spans and the buffer route for larger span counts. The native side implements the same semantic result using:

```rust
NativeTextPage::new(text)
```

for multi-span exact-byte materialization. The buffer route verifies exact framing:

```text
words = [span_count, style_ref, byte_length, ...]
bytes = concatenated UTF-8
```

`parse_and_build_text_buffer` rejects zero span count, incorrect word length, invalid UTF-8, unknown style refs, out-of-range spans, and any byte-length mismatch (`view_abi.rs:4442-4490`).

#### Small constructors versus bulk buffers versus builders

These are performance/transport specializations of the same persistent immutable View construction:

- fixed small row/column constructors;
- bulk axis buffer;
- builder begin/push/finish.

They do not establish separate retained identity rules. All successful routes terminate in `runtime.publish`.

#### Direct N-API versus direct FFI

The generated wrappers provide the default checked invocation route. Under `direct-ffi`, the generated exports also expose `extern "C"` symbols and `tui.rs` exposes qualification probes. The handwritten implementation remains in `view_abi.rs`; generated code supplies ABI validation and symbol forwarding.

### 5.4 Failure masking and recovery

#### Intentional identity-first masking

All major constructors consult `ref_for_node_id` before decoding payload. Therefore:

```text
live semantic NodeId + malformed alternate payload
    => existing ref returned
    => payload is not consumed/validated by the handwritten implementation
```

This is explicit in comments such as:

- `view_spacer_create_impl:2333-2339`;
- `view_common_patch_root_impl:2415-2420`;
- `view_text_create_*:4096-4102`;
- `view_grid_create_buffer_impl:3075-3080`.

This is not a secondary transport fallback. The TypeScript retained route treats an expected native status/cache miss as a controlled recovery condition, but a retained refusal ultimately fails explicitly; the source comments in `packages/iyon-tui/src/transport/structural/retained-dag.ts:19-22` state that there is no production secondary complete-object decoding path.

#### Cache miss versus invalid input

The ABI distinguishes:

- invalid shape, enum, ID, framing, or semantic conflict: `FAST_INVALID`;
- missing/stale ref/path/weak backing: `FAST_CACHE_MISS`;
- exhausted ref/path/builder/transaction or explicit resource refusal: `FAST_REFUSED`;
- host/native operation failure: `FAST_INTERNAL`.

Some implementations add child/base detail only when the underlying error is a cache miss. Structural factories generally collapse their own canonical factory errors into `FAST_INVALID`.

#### Host render failure

`host_render_ref_impl` returns `HOST_STATUS_INTERNAL` on a host render error. The edit-transaction route is stronger: it renders the candidate root before committing staged refs. A host error therefore leaves the prior host-visible root authoritative and drops the staged candidate when the function returns.

Evidence: `view_abi.rs:2220-2264`.

#### State failure atomicity

The state wrapper decodes all masks and lanes first. Canonical state records also mutate cloned override values before validation. This gives two layers of atomicity:

1. malformed wire envelope does not call host state mutation;
2. invalid geometry for the current state node kind does not replace the old stored override.

### 5.5 Generated wrapper safety boundary

Generated helpers include:

```rust
generated_nonnull
generated_nonnull_const
generated_buffer
generated_buffer_used
generated_native_ref
generated_node_id
generated_enum
```

Evidence: `generated/view_abi_exports.rs:1246-1339`.

They validate:

- null pointers;
- pointer alignment;
- byte capacity limits;
- element-size alignment;
- used count versus capacity;
- maximum count;
- native-ref range;
- NodeId safe-integer range;
- enum membership.

Generated calls also have a feature-dependent panic boundary:

- with `fast-view-abi`, `generated_catch_unwind` simply maps `Result::Err` to the error value;
- without `fast-view-abi`, it uses `std::panic::catch_unwind` and returns a panic sentinel.

Evidence: `generated/view_abi_exports.rs:1246-1259`.

The handwritten implementation assumes generated capacity/count checks for several buffer routes, as explicitly documented in `view_abi.rs:2634-2638` and `:3082-3085`. Direct calls to handwritten `*_impl` functions bypass that generated validation and are therefore only safe under the intended ABI wrapper contract.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Native-ref table

The paged table is optimized for hot ref lookup:

- page index: `reference >> 12`;
- page offset: `reference & 0xfff`;
- no hash lookup for ordinary ref resolution;
- refs are monotonic and not recycled;
- empty pages are deallocated;
- high-water directory length remains.

Evidence: `view_abi.rs:133-142`, `:176-280`.

The table tracks:

- current live slot count;
- current allocated page count;
- pages freed.

### 6.2 Semantic weak cache

`nodes: HashMap<u64, WeakView>` is the semantic cache. It is keyed by `NodeId`, not by native ref or pointer identity.

`node_refs: HashMap<u64, u32>` associates a semantic NodeId with its current NativeRef. The two structures can diverge temporarily during stale weak expiry or state-attachment replacement, but publication methods re-establish the association.

Weak cache behavior:

- live exact View: reuse;
- live different View: conflict;
- expired View: remove and republish;
- no cache entry: materialize or return cache miss depending operation.

### 6.3 Lease and release work

`release_one_lease`:

- rejects absent ref as cache miss;
- rejects zero lease count as invalid;
- decrements count;
- removes strong `leased` retention when count reaches zero.

Evidence: `view_abi.rs:1275-1287`.

`release_many`:

1. increments release batch counters;
2. runs bounded maintenance before processing the current batch;
3. decrements each ref;
4. immediately removes zero-lease slots if their weak View has expired;
5. otherwise queues zero-lease refs for later scavenging.

Evidence: `view_abi.rs:1289-1324`.

### 6.4 Weak-cache maintenance

Maintenance uses:

```text
SCAVENGE_BATCH_BUDGET = 256
FULL_SWEEP_METADATA_GROWTH_THRESHOLD = 4096
```

A bounded pass checks up to 256 queued zero-lease refs. When weak metadata growth since the previous full sweep reaches the threshold, a full expired weak sweep runs.

Evidence: `view_abi.rs:64-68`, `:1183-1225`.

Full pruning:

- removes expired semantic weak entries;
- removes corresponding unleased native refs;
- preserves leased slots even if their weak cache has expired;
- updates stale/removal counters.

Evidence: `view_abi.rs:1228-1273`.

Explicit N-API maintenance:

```rust
tui_view_abi_maintain(env, full)
```

returns cache/ref counts, queue length, processed count, and full sweep count (`view_abi.rs:1590-1605`).

### 6.5 Persistent path/style metadata

Path refs, style atom refs, and style refs are monotonic metadata allocations:

- path refs: `PATH_ROOT_REF..PATH_REF_LIMIT`;
- style atoms: `STYLE_ATOM_REF_START..STYLE_ATOM_REF_LIMIT`;
- styles: `STYLE_REF_START..STYLE_REF_LIMIT`.

The source contains no normal path/style eviction or maintenance sweep. They persist until runtime teardown. This is distinct from weak View/ref cache maintenance.

### 6.6 Text allocation behavior

Text routes intentionally distinguish allocation strategies:

- CString spans: each input is already a separate allocation; native code creates owned strings per span.
- Exact UTF-8 spans: one owned string and one `NativeTextPage` back all span ranges.
- Buffer text: validates complete framing and UTF-8 before constructing one page and range-backed spans.
- New text maximum: `MAX_NEW_TEXT_BYTES = 16 * 1024 * 1024`.

Evidence: `view_abi.rs:293-294`, `:3975-3987`, `:4124-4168`, `:4442-4490`.

### 6.7 State scheduling behavior

A successful non-noop state mutation calls `HostInner::invalidate_state`, which queues state work and returns a wake disposition. The native wrapper compresses that to one bit. State mutations do not directly increment structural desired revision; structural publication and state patch are separate lanes.

Canonical effect classification can cause:

- presentation-only paint/style work;
- geometry-dependent measure/place/project work;
- subtree repaint for dynamic style-state changes.

The host-side frame scheduler may then perform local geometry refresh or full retained work, but that implementation is outside native handwritten ownership.

### 6.8 Diagnostics

Native diagnostics include:

- semantic cache entries;
- native ref slots;
- leased slots;
- path nodes;
- builders;
- edit transactions;
- style atoms/styles;
- scavenge queue;
- stale/removal counters;
- full sweep count;
- weak expiry count;
- native-ref expired-slot count;
- current generation;
- alive flag.

`string_bytes` is intentionally returned as `null`, not zero, because the runtime does not currently track retained text/style payload bytes.

Evidence: `view_abi.rs:1612-1669`.

This is an important observability limitation: slot and semantic-entry counts are available, but retained string memory cannot be inferred from the current diagnostic snapshot.

---

## 7. Tests, benchmarks and observability

### 7.1 `view_abi.rs` tests

The handwritten ABI module contains extensive unit tests beginning around `view_abi.rs:4565`. Important tests include:

#### Identity and ref-table behavior

- `native_ref_table_maps_refs_across_pages`
- `native_ref_table_iter_matches_hashmap_semantics`
- `bulk_publication_reuses_the_environment_native_ref_table`
- `stale_unleased_weak_slot_returns_cache_miss`
- `slot_metadata_scavenged_after_weak_expiry`
- `repeated_node_id_lookups_acquire_independent_leases`

These establish page boundaries, weak expiry, semantic promotion, and independent lease counts.

#### Lease/lifetime behavior

- `new_constructor_returns_lease_count_one`
- `child_temp_lease_stays_live_until_root_completes`
- `batch_release_drops_child_temp_leases`
- `root_lease_transfers_to_boundary_after_new_install`
- `failed_transaction_releases_every_new_temp_lease`

These tests protect the distinction between temporary child leases and the final root lease.

#### Host/failure atomicity

- `failed_host_install_retains_old_root`
- `failed_transaction_releases_every_new_temp_lease`
- `stale_path_base_returns_cache_miss_then_recovers_once`

These are evidence that host installation occurs after candidate preparation and that old roots remain authoritative on failed installation.

#### Path and transaction structure

- `path_refs_are_interned_and_depth_specialization_rebuilds_only_the_path`
- `edit_transaction_builds_one_shared_ancestor_for_two_text_edits`
- `edit_transaction_abort_and_limits_leave_no_staged_state`
- `path_validation_rejects_wrong_parent_kind_and_preserves_publication`

#### Constructor and decoder alternatives

- `generated_text_string_variants_preserve_unicode_and_embedded_nul`
- `axis_buffer_rejects_count_larger_than_buffer_bytes_perf12_t8`
- `grid_create_buffer_builds_and_consults_cache_perf12_t10`
- `grid_malformed_tail_publishes_nothing_and_leaves_no_lease`
- `text_constructors_consult_semantic_cache_first_perf12_t11`
- `diff_create_buffer_builds_validates_and_consults_cache_perf12_t11`
- `t13_decorated_buffer_builds_and_validates_perf12`
- `cstring_lane_truncates_at_embedded_nul_per_contract`
- `utf8_lane_rejects_span_sum_mismatch_before_any_work`
- `utf8_lane_rejects_split_multibyte_boundary`
- `utf8_lane_preserves_embedded_and_trailing_nul`
- `utf8_lane_distinguishes_empty_string_from_empty_span_list`
- `utf8_lane_rejects_unknown_style_reference`
- `buffer_lane_rejects_bad_framing_and_split_boundaries`

### 7.2 `view_state.rs` tests

The state wrapper tests begin around `view_state.rs:645` and cover:

- stable schema IDs and lane lengths;
- full geometry decoding;
- presentation decoding with nullable fields and nested style;
- unknown header bits;
- null mask escaping the set mask;
- null on non-nullable property;
- set/clear intersection;
- short lanes;
- malformed scalar values;
- unknown alignment/edge/attribute bits;
- values outside attribute presence;
- absent and null lanes not being decoded;
- clear-all versus list versus empty clear semantics.

The tests explicitly demonstrate that fixed lanes are still required even if only one property is present, while inactive lane values are not semantically decoded.

### 7.3 Host retained-state tests as cross-boundary evidence

Although outside the native crate, `crates/iyon-tui/src/application/host.rs` tests prove the downstream state contracts consumed by the wrapper:

- `presentation_state_repaints_without_measurement_or_semantic_republication`;
- `structural_publication_invalidates_retained_state_dependency_paths`;
- `component_slot_replacement_carries_captured_state_versions`;
- `failed_frame_retains_old_state_versions_until_retry`;
- state/structural revision distinction tests around `host.rs:3497-3520`.

These show that state updates are intended to avoid structural republication and that candidate state versions remain pinned across failed frame attempts.

### 7.4 Native wrapper tests in `tui.rs`

`tui.rs` tests include:

- `native_text_input_owns_unicode_cursor_state`;
- `content_connector_status_maps_cleanup_through_the_existing_error_lane`.

The first is primarily Assignment 18 control evidence. The second is content failure behavior and is not structural/state ownership.

### 7.5 Benchmarks and counters

The native ABI implementation increments canonical `iyon_tui::binding::Counter` values under `perf-counters`, including:

- copied text bytes;
- decorated normalization;
- state invalidations;
- constructor/retained path counters in downstream modules.

The native addon exposes:

- `tuiPerfReset`;
- `tuiPerfSnapshot`;
- direct ABI probes;
- `tuiViewAbiMaintain`;
- `tuiViewRuntimeMemorySnapshot`.

The ABI source also contains an internal native-ref representation benchmark (`view_abi.rs:4681-4734`). No benchmark or test was executed for this report.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Native ABI is an adapter, not the semantic View owner

The native module imports canonical constructors and types from `iyon_tui::binding`:

```rust
View,
WeakView,
view_native_text_final,
view_native_axis_from_children,
view_native_patched,
view_native_replace_at_path,
view_native_with_state_attachment,
...
```

Evidence: `view_abi.rs:3-18`.

This means native handwritten code owns:

- wire validation;
- transport identity;
- publication;
- temporary staging;
- lease accounting.

It does not own the semantic `View` data model, layout engine, paint representation, or retained state record implementation.

### 8.2 State and structure intentionally cross at attachment identity

The structural ABI attaches a `state_id` to a state-capable View, while the state wrapper mutates the state record independently. The same state identity is therefore referenced by:

```text
View.state_attachment_id()
    ↕
NativeViewState.state_id()
    ↕
ViewStateRegistry.records[id]
```

The structural resolver validates duplicate state attachment identities in the semantic View tree (`crates/iyon-tui/src/scene/resolve.rs:258-264`), while the host state registry validates state identity/kind and retains state snapshots.

This is a real cross-plane coupling, not merely a transport detail.

### 8.3 Structural publication and retained-state binding are separate transactions

`NativeTuiHost::set_desired_view_ref` accepts a complete retained root and schedules host work. `NativeViewState::set_*` changes state values and returns a wake bit. The native boundary does not combine them into one transaction.

This allows:

- a state patch to avoid desired structural revision changes;
- a structural root replacement to capture current state bindings;
- a failed host frame to retain prior visible state versions;
- a later retry to apply newer desired state.

The distinction is confirmed in Rust host tests and comments around `crates/iyon-tui/src/application/host.rs:3497-3520`.

### 8.4 Environment ownership versus host ownership

The View ABI runtime is environment-scoped, while `TuiHost` is host-scoped. A single environment may therefore own multiple hosts that use the same semantic View runtime/ref namespace.

The host wrapper stores the runtime pointer, but the `NativeViewRuntime` remains in `RUNTIME_HANDLES`, not in `NativeTuiHost`. This explains why host disposal explicitly clears runtime-scoped builders/edit transactions and why the environment cleanup hook, not host drop, marks the runtime dead.

### 8.5 State wrapper and state record locking changed ownership shape

Native `NativeViewState` contains no per-record mutex. `HostViewState` carries an immutable ID and weak host pointer; every operation upgrades the host `Weak`, locks `HostInner`, and accesses the host-owned registry.

Evidence:

- `NativeViewState`: `native/tui/view_state.rs:25-29`;
- `HostViewState`: `iyon-tui/application/view_state.rs:23-35`;
- serialized mutation: `application/view_state.rs:106-124`.

This is a deliberate ownership seam: the host owns mutable state and frame consistency, while the N-API wrapper owns only transport lifecycle.

### 8.6 Generated code is a safety boundary but not the semantic owner

The handwritten implementation relies on generated wrappers for raw pointer/capacity/count validation. Conversely, generated N-API wrappers rely on handwritten `*_impl` functions for semantic validation and publication.

The boundary is therefore:

```text
generated:
    null/alignment/capacity/count/node-id/enum/panic safety
handwritten:
    identity/ref/cache/lease/semantic decode/factory/transaction rules
```

This split must remain synchronized with `tools/tui-abi/view_abi.toml` and the generated ABI artifacts. The generated state schema explicitly states that it owns masks, offsets, and codes while handwritten readers own value semantics (`generated/view_state_schema.rs:4-9`).

### 8.7 Historical document versus current source

The historical PRE-V5 report emphasizes inventory-first investigation and warns against assuming old types must survive future redesign. That guidance is compatible with the current source, but it is not evidence that any current native structural/state type should be removed or preserved.

Current source specifically shows:

- one central semantic publication helper;
- explicit no-secondary-transport comments in the TypeScript retained route;
- staged host-atomic transaction behavior;
- distinct retained state and structural lanes.

Those are current facts. No V5 disposition is made here.

### 8.8 Potential metadata mutation before host acceptance

The staged publication documentation states that refs are reserved without exposing them and committed only after host acceptance (`view_abi.rs:816-819`). This is true for newly installed `slots` and `nodes`: they are not installed until `commit_staged_publication`.

However, `prepare_staged_publication` may remove stale `node_refs` entries while preparing replacement refs (`view_abi.rs:839-850`). If host rendering then fails, no new View slots are installed, but stale node-ref metadata may already have been cleared. The next retry can recover by rematerializing or republishing.

This is not a visible root inconsistency, but it is a narrower metadata-side effect before host acceptance worth preserving in any future transaction analysis.

### 8.9 `view_common_patch_root` has a compatibility argument

`decoration_ref` is accepted in the packed common patch ABI. The handwritten implementation:

- rejects a nonzero stale `decoration_ref` with `FAST_CACHE_MISS`;
- does not use the resolved decoration View in the resulting patch;
- treats zero as absent and valid.

Evidence: `view_abi.rs:2422-2435`.

This is an ABI compatibility surface whose current semantic payload is effectively ignored after validation.

---

## 9. Open questions and coverage gaps

1. **Runtime generation rollover:** `NativeViewRuntime.generation` is initialized to `1`, exposed in metadata, and used by external TypeScript hint logic, but no reassignment was found in `view_abi.rs`. It is not established whether generation rollover is intentionally environment-lifetime-only or implemented elsewhere.

2. **Exact native ABI function count:** `generated_table::FUNCTION_COUNT` is exposed in metadata, but this report does not reproduce the complete generated descriptor table count. The generated table is indexed, not treated as handwritten source.

3. **Direct FFI execution behavior:** The direct-FFI wrappers and panic handling were statically inspected, but no direct-FFI build or call was run. The interaction between `fast-view-abi`, `direct-ffi`, and generated panic sentinels remains execution-unverified.

4. **Raw handwritten `*_impl` safety outside generated wrappers:** Several implementations explicitly rely on generated capacity/count validation. This report did not test whether any internal caller bypasses those wrappers.

5. **Host frame transaction internals:** Native host methods expose desired/visible/pending/committed epochs, but the full frame preparation, candidate capture, layout, and paint flow belongs to the Rust `application`/`scene` layers and was not exhaustively inventoried here.

6. **State property validation after unmount:** The canonical state record validates geometry against `desired_kind.or(visible_kind)`. The precise behavior when a state is unmounted and has neither kind is visible in source but not exercised in this report.

7. **Path/style metadata bounds:** Native View/ref weak caches have explicit maintenance. Path keys, path nodes, style atoms, and style refs do not have equivalent normal eviction in this implementation. The practical lifetime and exhaustion behavior under long-running workloads was not executed.

8. **Cross-host same-environment semantics:** The runtime is environment-scoped and hosts are host-scoped. The source establishes shared ref namespace ownership, but no multi-host same-environment execution was performed.

9. **Native History/ViewSlot ownership boundary:** `NativeHistory`, `NativeViewSlot`, and `NativeScrollPane` consume structural refs and call `resolve_native_view`, but their complete lifecycle and retention behavior remain in Assignment 18’s content/host/control scope.

10. **Theme DTO parity:** The native structural/state code uses the shared string color decoder and style refs. Full object-shaped theme/border DTO parity was not treated as Assignment 17 ownership.

11. **No complete repository absence claims:** This report does not claim that no alternate route exists anywhere in the repository. It establishes that the inspected current retained production route and native handwritten implementation have the routes described above; other assignments own the broader TypeScript, generated, Rust, host, and content surfaces.

---

## 10. Evidence appendix

### 10.1 Primary inspected handwritten files

#### Fully owned

```text
crates/iyon-tui-native/src/tui/view_abi.rs
    NativeViewRuntime
    NativeViewSlot
    NativeRefTable
    SemanticIdentityMatch
    PublicationLease
    AxisBuilder
    EditTxn
    StagedPublication
    runtime_handle_for_env
    runtime_from_handle
    runtime_mut
    resolve_ref
    consult_semantic_identity
    publish_semantic_view
    publish
    publish_bulk
    ref_for_node_id
    maintain / prune_expired
    release_one_lease / release_many
    view_state_attach_impl
    path_root_impl / path_child_impl
    publish_text_path
    publish_structural_path
    validate_path_publication
    axis builder entrypoints
    edit transaction entrypoints
    common/text/path structural patch entrypoints
    axis/grid/diff/decorated/container/clamp/hanging/component constructors
    style and text constructors
    decoder helpers
    ABI status/detail helpers
    ABI unit tests
```

```text
crates/iyon-tui-native/src/tui/view_state.rs
    NativeViewState
    wake_value
    decode_geometry_envelope
    decode_geometry_clear
    decode_presentation_envelope
    decode_presentation_clear
    read_size_mode
    read_alignment
    read_border_edges
    read_border_style
    read_border_glyphs
    read_text_attributes
    read_style
    state decoder tests
```

#### Partially owned

```text
crates/iyon-tui-native/src/tui.rs
    HOST_ENVIRONMENTS / CONTENT_ENVIRONMENTS
    host_environment_for_env
    content_environment_for_identity
    ensure_alive
    resolve_native_view
    NativeTuiHost::new
    NativeTuiHost::epochs
    NativeTuiHost::set_desired_view_ref
    NativeTuiHost::clear_view_state_bindings
    NativeTuiHost::flush_pending_hosts
    NativeTuiHost::dispose
    NativeTuiHost::view_state
    shared color_spec_str / parse_rgb_hex / text_attribute
```

### 10.2 Generated files indexed at seams

```text
crates/iyon-tui-native/src/generated/view_abi_exports.rs
    generated_catch_unwind
    generated_nonnull / generated_nonnull_const
    generated_buffer
    generated_buffer_used
    generated_native_ref
    generated_node_id
    generated_enum
    invoke_* wrappers
    direct-FFI exports
```

```text
crates/iyon-tui-native/src/generated/view_abi_napi.rs
    NativeViewAbiSession N-API methods forwarding to ABI symbols
```

```text
crates/iyon-tui-native/src/generated/view_abi_types.rs
    ABI_NAME
    ABI_VERSION
    SEMANTIC_SCHEMA_VERSION
    generated ABI constants/types
```

```text
crates/iyon-tui-native/src/generated/view_abi_table.rs
    FunctionDescriptor table
    FUNCTION_COUNT
    family/ownership/hotness metadata
```

```text
crates/iyon-tui-native/src/generated/view_state_schema.rs
    geometry property IDs/masks/offsets/codes/check_envelope
    presentation property IDs/masks/offsets/codes/check_envelope
```

These generated files were not counted as handwritten Assignment 17 implementation.

### 10.3 Rust retained-state seams followed

```text
crates/iyon-tui/src/application/view_state.rs
    HostViewState
    state_id
    validate_node_kind
    set_geometry / clear_geometry
    set_presentation / clear_presentation
    set_style_state / clear_style_state
    dispose
    mutate
```

```text
crates/iyon-tui/src/retained_state/record.rs
    ViewStateRecord
    ViewStateLifecycle
    snapshot
    apply_geometry / clear_geometry
    apply_presentation / clear_presentation
    set_style_state / clear_style_state
```

```text
crates/iyon-tui/src/retained_state/registry.rs
    ViewStateRegistry
    PreparedStateCommit
    create
    mutate_record
    capture_candidate
    validate_targets
    set_desired / set_visible
    prepare_visible / prepare_candidate
    commit_visible_prepared
    dispose / clear_bindings / dispose_all
```

```text
crates/iyon-tui/src/retained_state/effects.rs
    StateEffects
    presentation_effects
```

```text
crates/iyon-tui/src/retained_state/geometry.rs
    GeometryOverrides
    geometry property effect classification
```

```text
crates/iyon-tui/src/application/host.rs
    HostInner view-state registry ownership
    create_view_state
    mutate_view_state
    validate_view_state_kind
    dispose_view_state
    clear_state_bindings
    frame/state invalidation tests
```

```text
crates/iyon-tui/src/scene/resolve.rs
    duplicate state attachment validation
```

### 10.4 TypeScript reverse consumers followed

```text
packages/iyon-tui/src/transport/structural/retained-dag.ts
    semantic NativeRef hints
    ensureNative identity-first ordering
    NativeRef promotion
    retained root boundary
    edit transaction caller
    no-secondary-transport refusal semantics
```

```text
packages/iyon-tui/src/transport/structural/native-view-abi.ts
    nativeViewAbiSession
    viewRefForNodeId
    viewReleaseMany
    editTxnBegin / Add / Commit / Abort
    hostRenderRef
    native host resolution
```

```text
packages/iyon-tui/src/api/view/retained-state.ts
    ViewStateContract
    geometry/presentation envelope packing
    state mutation calls
```

```text
packages/iyon-tui/src/runtime/runtime.ts
    viewState() public runtime surface
```

```text
packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts
    NativeAbiStatusError
    status high-bit checks
    status-detail reads on cache miss
```

### 10.5 Relevant current tests indexed

```text
crates/iyon-tui-native/src/tui/view_abi.rs
    native_ref_table_maps_refs_across_pages
    native_ref_table_iter_matches_hashmap_semantics
    generated_spacer_publish_lookup_and_release_share_the_semantic_cache
    bulk_publication_reuses_the_environment_native_ref_table
    generated_text_string_variants_preserve_unicode_and_embedded_nul
    native_axis_builders_and_small_constructors_publish_immutable_views
    new_constructor_returns_lease_count_one
    child_temp_lease_stays_live_until_root_completes
    batch_release_drops_child_temp_leases
    root_lease_transfers_to_boundary_after_new_install
    failed_host_install_retains_old_root
    failed_transaction_releases_every_new_temp_lease
    stale_unleased_weak_slot_returns_cache_miss
    slot_metadata_scavenged_after_weak_expiry
    repeated_node_id_lookups_acquire_independent_leases
    generated_text_and_common_patches_publish_new_node_ids
    path_refs_are_interned_and_depth_specialization_rebuilds_only_the_path
    stale_path_base_returns_cache_miss_then_recovers_once
    edit_transaction_builds_one_shared_ancestor_for_two_text_edits
    edit_transaction_abort_and_limits_leave_no_staged_state
    grid_malformed_tail_publishes_nothing_and_leaves_no_lease
    text_constructors_consult_semantic_cache_first_perf12_t11
    diff_create_buffer_builds_validates_and_consults_cache_perf12_t11
    path_validation_rejects_wrong_parent_kind_and_preserves_publication
    cstring_lane_truncates_at_embedded_nul_per_contract
    utf8_lane_rejects_span_sum_mismatch_before_any_work
    utf8_lane_rejects_split_multibyte_boundary
    utf8_lane_preserves_embedded_and_trailing_nul
    utf8_lane_distinguishes_empty_string_from_empty_span_list
    utf8_lane_rejects_unknown_style_reference
    buffer_lane_rejects_bad_framing_and_split_boundaries
```

```text
crates/iyon-tui-native/src/tui/view_state.rs
    schema_ids_are_stable_per_domain
    decodes_full_geometry_envelope
    decodes_presentation_envelope_with_null_and_style
    rejects_envelope_header_violations
    rejects_unknown_value_bits_and_values_outside_presence
    absent_and_null_value_lanes_are_not_decoded
    clear_distinguishes_all_from_list_and_empty
```

### 10.6 Documentation and contract files

```text
docs/architecture/atlas-4355c02/REPORT-CONTRACT.md
docs/architecture/atlas-4355c02/README.md
docs/architecture/atlas-4355c02/evidence/assignments.json
AGENTS.md
PRE-V5-ARCHITECTURE-REPORT.md
```

### 10.7 Files merely indexed or outside primary ownership

```text
crates/iyon-tui-native/src/content_ffi.rs
crates/iyon-tui-native/src/error.rs
crates/iyon-tui-native/src/sync.rs
crates/iyon-tui-native/src/tui/theme_dto.rs
crates/iyon-tui-native/src/generated/view_abi_conformance.rs
crates/iyon-tui-native/src/lib.rs
```

The native `error.rs` mapping was cross-referenced because `tui.rs` converts host/content/state failures into N-API errors. Its principal ownership is outside the structural/state implementation.

### 10.8 LOC methodology

- Physical line estimates use the observed source line ranges returned by repository content inspection.
- Production/test boundaries use the first `#[cfg(test)]` module in each file.
- Generated files are intentionally excluded from handwritten production/test LOC.
- `tui.rs` is reported as a mixed file; its total physical size includes content/control/resource wrappers primarily belonging to Assignment 18.
- No test, benchmark, build, or runtime execution was performed for this report.