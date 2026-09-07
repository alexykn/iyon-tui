# 18 — Content/data lanes, host/controls/runtime handles, native lifecycle, callbacks/errors

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: handwritten files under `crates/iyon-tui-native/src/` outside the structural/state implementation
- Investigation mode: read-only static source inspection
- No production files, configuration, tests, or generated files were modified.
- No build, test, benchmark, or runtime suite was executed during this investigation.

The report contract, atlas README, repository `AGENTS.md`, assignment manifest, and `PRE-V5-ARCHITECTURE-REPORT.md` were inspected before source analysis. The pre-V5 document was used as historical/contextual guidance only; this report describes current source behavior and does not make V5 disposition decisions.

### Scope boundary

Assignment 17 owns the native structural/state entrypoints. This report therefore focuses on:

- Native content/data lanes
- Direct content FFI
- Native host/environment ownership
- Native controls and runtime handles
- Input/output routing and callback-like behavior
- Native lifecycle and disposal
- Native error translation
- Theme/border DTO decoding where it is part of this native boundary
- Handwritten support/probe modules in the native crate

The following were indexed but not treated as owned implementation for this assignment:

- `crates/iyon-tui-native/src/generated/**`: generated ABI bodies
- `crates/iyon-tui-native/src/tui/view_abi.rs`: structural/view ABI runtime
- `crates/iyon-tui-native/src/tui/view_state.rs`: retained state ABI runtime
- `crates/iyon-tui-native/tests/generated_view_abi.rs`: generated/structural integration tests
- Rust content/host implementation under `crates/iyon-tui/src/application/content.rs`, `host.rs`, and `environment.rs`: these were followed at relevant symbols to prove native seams, but their complete subsystem census belongs to the Rust-side assignments.

### Facts, inferences, and unknowns

- **Fact:** native N-API wrappers hold generic `iyon_tui::binding` handles such as `TuiHost`, `HostContentSource`, `HostContentPort`, `HostContentConnector`, `HostTextInput`, `HostViewSlot`, and `HostScrollPane`.
- **Fact:** high-volume Source mutation is exported through direct C ABI symbols in `content_ffi.rs`; the N-API `NativeTextSource` wrapper exposes identity, snapshots, stats, and family but not append/replace/clear/seal/truncate methods.
- **Fact:** Source mutation accepts the bytes and returns a fixed result containing the authoritative source revision, environment wake epoch, and a drain flag.
- **Fact:** native input interpretation and routing remain on the Rust side; TypeScript consumes routed output through polling or `waitForOutput`.
- **Inference:** the direct content ABI is deliberately separate from the N-API control lane to avoid repeated N-API object conversion for large payloads.
- **Unknown:** no runtime execution was performed, so this report cannot verify the behavior of the compiled addon, the dynamic library loader, or platform-specific N-API ABI details.
- **Unknown:** the complete TypeScript wake-broker and resource-registry behavior is referenced where needed but belongs primarily to assignments 21 and 23.

---

## 1. Responsibility and structure

### 1.1 Handwritten native inventory

The native handwritten source inventory is:

| File | Approx. total LOC | Approx. production LOC | Approx. test LOC | Primary responsibility |
|---|---:|---:|---:|---|
| `crates/iyon-tui-native/src/lib.rs` | 15 | 15 | 0 | Crate module wiring and intentionally small public exports |
| `crates/iyon-tui-native/src/error.rs` | 89 | 89 | 0 | Stable N-API error categories and content diagnostic translation |
| `crates/iyon-tui-native/src/sync.rs` | 8 | 8 | 0 | Native artifact/version probe |
| `crates/iyon-tui-native/src/content_ffi.rs` | 585 | 485 | 100 | Direct Source payload ABI, metadata/status records, pointer validation, panic guard |
| `crates/iyon-tui-native/src/tui.rs` | 2,017 | 1,962 | 55 | N-API host, controls, content handles, input/output routing, lifecycle |
| `crates/iyon-tui-native/src/tui/theme_dto.rs` | 597 | 440 | 157 | Typed theme and border decoding into canonical Rust theme/style records |
| **Total handwritten scope** | **3,311** | **2,999** | **312** |  |

Counting method: physical source lines from the checked-in files, with `#[cfg(test)] mod tests` regions counted separately. Generated files and structural/state modules are excluded from the table. The count is approximate in the architectural sense because some production lines are module glue, comments, and feature-gated probes.

Additional native files indexed but excluded from the production LOC total:

- `src/generated/view_abi_conformance.rs`
- `src/generated/view_abi_exports.rs`
- `src/generated/view_abi_napi.rs`
- `src/generated/view_abi_table.rs`
- `src/generated/view_abi_types.rs`
- `src/generated/view_state_schema.rs`
- `src/tui/view_abi.rs`
- `src/tui/view_state.rs`

### 1.2 `lib.rs`

`crates/iyon-tui-native/src/lib.rs:1-15` is intentionally thin:

```rust
mod content_ffi;
mod error;
mod sync;
mod tui;

pub(crate) use error::NativeError;
pub use sync::native_version;
pub use tui::tui_smoke;
```

The direct FFI symbols are exported from the private `content_ffi` module by `#[unsafe(no_mangle)]` functions; they do not need to be re-exported through the Rust module namespace.

The module boundary communicates that the native crate is a generic framework boundary, not an application/session crate. The module-level comment explicitly states that application/session bindings belong elsewhere.

### 1.3 `error.rs`

`crates/iyon-tui-native/src/error.rs:1-89` defines the stable native error category enum:

```rust
pub enum NativeError {
    InvalidInput,
    Internal,
    Cancelled,
    Closed,
}
```

The enum itself is not used as a typed error return. Its methods construct `napi::Error` values with stable Node-API status and string codes:

- `invalid_input` → `Status::InvalidArg`, `ION_INVALID_INPUT`
- `internal` → `Status::GenericFailure`, `ION_INTERNAL`
- `cancelled` → `Status::Cancelled`, `ION_CANCELLED`
- `closed` → `Status::Closing`, `ION_CLOSED`
- `coded` → arbitrary supplied status/code
- `content` → extracts a whitelisted core content code from `CODE: detail` diagnostics

`NativeError::content` is important because content-core lifecycle and validation failures are not all generic invalid input. It promotes known codes to an `ION_<CODE>` prefix, while unknown diagnostics collapse to `ION_INTERNAL`.

### 1.4 `sync.rs`

`crates/iyon-tui-native/src/sync.rs:1-8` exports `nativeVersion()` through N-API:

```rust
"iyon-tui-native/s6"
```

The TypeScript loader uses this marker to verify that the loaded native artifact matches the expected package build identity. The integration test at `crates/iyon-tui-native/tests/sync.rs:3-9` also treats the marker as a stable framework contract.

### 1.5 `content_ffi.rs`

`crates/iyon-tui-native/src/content_ffi.rs:1-585` is the direct high-volume content data lane.

Its responsibilities are:

1. Define ABI metadata and fixed result records.
2. Define numeric ABI status codes.
3. Validate direct caller pointers, lengths, alignment, and limits.
4. Resolve environment/source identities.
5. Borrow caller buffers synchronously without retaining pointers.
6. Invoke one of the safe `HostContentSource` mutations.
7. Write authoritative revision/wake output.
8. Catch panics at the FFI boundary.
9. Export the six required direct symbols.
10. Test that accepted mutation remains successful even when one host wake fails.

The file explicitly states at lines 1-6 that this is the only high-volume Source payload entrypoint and that it does not perform projection, layout, paint, or callbacks while a payload call is in progress.

### 1.6 `tui.rs`

`crates/iyon-tui-native/src/tui.rs:1-2,017` is the large N-API facade. It combines several responsibilities:

- Per-N-API-environment native runtime lookup
- Content environment registration for direct FFI
- Generic artifact/probe exports
- N-API error/liveness helpers
- Detached and host-attached `History`
- Detached and host-attached `TextInput`
- Native host construction and closure
- Host epochs and pending-host drain reports
- Theme application and border decoding
- Content Source, Port, and Connector wrappers
- View slots and scroll panes
- Key parsing and generic routed output
- Native output transport
- Color/style utility decoders
- Focused unit tests for Unicode cursor behavior and connector status shape

The module also includes generated structural/state modules through `include!` at lines 19-31, but their generated or structural behavior is not counted as assignment-18 ownership.

### 1.7 `theme_dto.rs`

`crates/iyon-tui-native/src/tui/theme_dto.rs:1-597` is a typed input decoder rather than a second theme model.

It decodes:

- Theme colors
- Indexed ANSI colors
- Default colors
- String color forms (`theme:`, `ansi:`, `#rrggbb`, named colors)
- Sparse style objects
- Focus/state selectors
- Semantic text selectors
- Text roles and parts
- Annotation/language/origin/format selectors
- TextInput border style, edge, color, and glyph options

The module-level comment at lines 1-12 states that DTOs terminate directly in canonical Rust records and that malformed containers fail closed instead of being silently ignored.

---

## 2. Types, APIs and contracts

### 2.1 Native handle types

The handwritten N-API classes in `tui.rs` are:

| Native class | Underlying core handle/state | Main role |
|---|---|---|
| `NativeTuiOutput` | `Output<String>` | Opaque typed output value returned from TextInput submission |
| `NativeHistory` | Detached `Mutex<History>` or `HostHistory` | History authoring/control handle |
| `NativeTextInput` | Detached `Mutex<TextInput>` or `HostTextInput` | Generic text input control |
| `NativeTuiHost` | `Box<TuiHost>` | Host/environment owner and runtime facade |
| `NativeTextSource` | `HostContentSource` | Environment-owned Source identity/query handle |
| `NativeContentPort` | `HostContentPort` | Host-owned content attachment |
| `NativeContentConnector` | `HostContentConnector` | Source/Funnel/Port membership and delivery control |
| `NativeViewSlot` | `HostViewSlot` | Generic retained slot and animation control |
| `NativeScrollPane` | `HostScrollPane` | Generic scroll/content control |

Each wrapper that can be invalidated carries an `AtomicBool` named `alive`, except `NativeTuiOutput`, which is a value container.

The TypeScript contract in `packages/iyon-tui/src/transport/native/addon.ts:14-188` mirrors these classes. It intentionally keeps them private to the generic framework facade while exposing generic methods to the rest of the package.

### 2.2 Liveness contract

`tui.rs:275-284` defines `ensure_alive`, which all meaningful wrapper methods use:

```rust
if alive.load(Ordering::Acquire) {
    Ok(())
} else {
    Err(ION_DISPOSED_HANDLE)
}
```

Disposal generally uses `AcqRel`/`Release` stores. The liveness flag is an N-API wrapper guard; it is not the authoritative core lifecycle. The core handles maintain their own lifecycle and ownership checks.

This creates two layers of failure:

1. Wrapper already disposed → `ION_DISPOSED_HANDLE`
2. Wrapper live but core resource invalid/stale/retired → a core diagnostic translated by `NativeError::content`, `NativeError::internal`, or `NativeError::invalid_input`

### 2.3 `NativeTextSource`

Definition: `tui.rs:1,089-1,238`.

Constructor:

- `new(env, kind?, options?)`
- Default kind is `"stream"`.
- Accepted kinds: `"stream"` and `"block"` (`tui.rs:1,098-1,108`).
- The environment is resolved using `host_environment_for_env`.
- `TuiEnvironment::create_content_source` creates the core Source.
- Optional retention is validated and configured before returning.

Exposed methods:

- `dispose`
- `sourceId`
- `sourceGeneration`
- `environmentSlot`
- `environmentGeneration`
- `contentGeneration`
- `snapshot`
- `stats`
- `family` → always `"text"`

The Source wrapper does not expose bulk mutation methods. Those operations are intentionally routed through `content_ffi.rs` and the TypeScript direct loader.

Identity methods preserve source/environment identity values required by the direct ABI. Content and revision generations are returned as strings where JavaScript-safe integer behavior matters:

- `contentGeneration()` returns a string (`tui.rs:1,168-1,175`)
- Snapshot large integer fields are stringified (`tui.rs:1,200-1,210`)
- Stats large integer fields are stringified (`tui.rs:1,218-1,230`)

The snapshot shape includes:

```text
sourceId
sourceGeneration
contentGeneration
revision
sourceBase
sourceEnd
sealed
headPartial
text
annotations[]
```

Each annotation includes kind, flags, byte range, payload, and auxiliary lanes.

The stats shape includes:

```text
revision
sourceBase
sourceEnd
retainedBytes
retainedLines
chunkCount
sealed
headPartial
acceptedBytes
copiedBytes
droppedHeadBytes
```

### 2.4 `NativeContentPort`

Definition: `tui.rs:1,240-1,342`.

A port is created by `NativeTuiHost::content_port` at `tui.rs:849-862`.

The N-API facade currently accepts only the `"text"` family. Any other family is rejected before touching the core host:

```text
unsupported ContentPort family
```

Exposed methods:

- `dispose`
- `portId`
- `attachmentId` (currently aliases `portId`)
- `portGeneration`
- `family` → `"text"`
- `deactivate`
- `connect`
- `mounted`

`connect` validates both wrapper liveness and the Source wrapper liveness before calling `HostContentPort::connect`. It accepts:

- Funnel kind: `plain`, `markdown`, `diff`, `ansi`
- Wrap: `word`, `grapheme`, `noWrap`
- Hyperlink flag
- Immediate versus smooth delivery
- Smooth tick interval
- Spring/min/max rates

The port owns the structural attachment identity, but Source/Funnel identity remains separate. The core host comment at `crates/iyon-tui/src/application/host.rs:1,064-1,075` explicitly describes this separation.

### 2.5 `NativeContentConnector`

Definition: `tui.rs:1,344-1,431`.

A connector is the native control handle for a specific Source/Funnel/Port relationship.

Exposed methods:

- `activate`
- `deactivate`
- `dispose`
- `failNextActivation`
- `status`

`failNextActivation` is explicitly marked native/unit-only at `tui.rs:1,393-1,400`; the public TypeScript Connector intentionally does not expose it.

All control methods return a JSON object containing:

```json
{
  "schedule_environment_drain": boolean
}
```

`status()` returns:

```json
{
  "phase": "...",
  "requested": boolean,
  "visible": boolean,
  "projectedSourceRevision": "u64-or-null",
  "error": { "code": "...", "diagnostic": "..." } | null
}
```

The status helper at `tui.rs:53-72` accepts both operating and cleanup errors, but exposes one existing `error` field. Cleanup error takes precedence over operating error. This preserves the TypeScript status shape while retaining the core distinction internally.

The TypeScript wrapper uses the core status phase to finalize its wrapper after deferred disposal. Therefore the native wrapper itself intentionally remains callable long enough for status reconciliation even after logical disposal has been requested.

### 2.6 `NativeTuiHost`

Definition: `tui.rs:602-1,041`.

Constructor:

- `new(env, width?, height?, headless?)`
- Defaults: `80 × 24`, non-headless.
- Width and height must fit in `u16`.
- The environment is obtained from the Node-API `Env`.
- `TuiHost::open_in_environment` creates the actual host.
- The view ABI runtime pointer for the same Node-API environment is retained as an integer.

Main categories of methods:

#### Host state and lifecycle

- `epochs`
- `dispose`
- `disposeContentResources`
- `exit`
- `exited`
- `nextWakeMs`
- `resize`
- `advanceTime`

#### Structure/state/content attachment

- `setDesiredViewRef`
- `clearViewStateBindings`
- `flushPendingHosts`
- `setTheme`
- `setHistory`
- `history`
- `viewState`
- `contentPort`
- `textInput`
- `createViewSlotRef`
- `scrollPaneRef`

#### Input/output routing

- `bindKey`
- `route`
- `interceptPaste`
- `dispatchKey`
- `dispatchPaste`
- `forwardPaste`
- `pollTerminal`
- `nextOutput`
- `waitForOutput`

#### Headless inspection

- `screenRows`
- `nativeHistoryRows`
- `styleAt`
- `cellXOfText`

`setDesiredViewRef` accepts a retained native View reference but does not synchronously present it. Its result reports the host ID and an edge-triggered `schedule_environment_drain` hint. The comment at `tui.rs:655-656` states that the next environment drain performs the frame transaction.

`flushPendingHosts` validates a caller budget of 1 through 1,024, defaulting to 32. It serializes host errors and commits from the core `HostDrainReport`, including attempted epoch, desired revision, phase, code, retryability, diagnostic, and wake epoch (`tui.rs:683-739`).

### 2.7 `NativeHistory`

Definition: `tui.rs:298-447`.

A detached `NativeHistory` owns a `Mutex<History>`. Once attached to a host, it stores a `HostHistory` clone and the detached state is replaced by a new empty `History`.

Exposed methods:

- `dispose`
- `isDetached`
- `layout`
- `setLayout`
- `pushRef`
- `freezeRef`
- `discardLive`

Important ownership rule:

- A detached History can be configured and pushed into locally.
- `freezeRef` and `discardLive` require a host-attached History; detached calls fail explicitly.
- `setHistory` validates the History against the host before taking its state and transferring ownership (`tui.rs:783-804`).
- A History already attached to a host cannot be attached again.

`NativeHistory::dispose` only marks the N-API wrapper dead. If attached, the underlying host History remains host-owned; this is consistent with the host being the runtime owner.

### 2.8 `NativeTextInput` and `NativeTuiOutput`

Definitions: `tui.rs:449-600`.

`NativeTextInput` supports both detached and host-attached use:

- Detached constructor creates `TextInput` in a `Mutex`.
- Host creation registers a `HostTextInput` component and wraps it.
- All reads/writes dispatch to the host handle when attached, otherwise to detached state.

Exposed methods:

- `dispose`
- `text`
- `cursorBytes`
- `setText`
- `clear`
- `setMultiline`
- `isMultiline`
- `submitted`
- `componentId`

Disposal of a host-attached input calls `HostTextInput::retire`, which is a deferred component retirement request, not immediate physical deletion (`tui.rs:468-473`).

`submitted()` returns `NativeTuiOutput`, carrying `Output<String>`. The host routes that typed output to an opaque caller route ID. The TypeScript facade caches the output channel identity; this is visible in `packages/iyon-tui/src/api/controls/text-input.ts:56-65`.

### 2.9 View slots and scroll panes

These controls are not the primary content data lane but are part of the remaining native host/control facade.

`NativeScrollPane` (`tui.rs:1,573-1,626`) exposes:

- `dispose`
- `componentId`
- `setContentRef`
- `followEnd`

`NativeViewSlot` (`tui.rs:1,566-1,856`) exposes:

- `dispose`
- `revision`
- `componentId`
- `setViewRef`
- Multiple fixed-arity and packed animation frame methods
- Immediate and cycle-boundary animation replacement
- `stopAnimation`
- `stopAnimationRef`

Both use deferred retirement. The comments at `tui.rs:1,581-1,590` and `1,629-1,638` explicitly state that disposal requests retirement and that physical reclamation waits until reconciliation proves the component unmounted.

### 2.10 Theme and border APIs

`NativeTuiHost::setTheme` delegates to `theme_dto::decode_theme` (`tui.rs:775-780`).

`NativeTuiHost::textInput` validates the border DTO before creating/registering the input, preventing a malformed border from leaving an unreachable component behind (`tui.rs:865-885`).

`theme_dto.rs` decodes caller-supplied generic theme policy into canonical framework records. It rejects:

- Unknown color object forms
- Theme references where a direct ThemeColor is required
- Unknown style attributes
- Unknown text roles/parts
- Invalid semantic IDs
- Unknown border styles or edge modes
- Invalid border glyphs

The decoder uses `BTreeMap` iteration to preserve deterministic key ordering (`theme_dto.rs:316-342`) and `intern_style_atom` for repeated theme keys (`theme_dto.rs:346-347`).

---

## 3. Dependency and ownership map

### 3.1 Native dependency direction

```text
TypeScript generic facade
    │
    ├── N-API NativeTuiHost / NativeTextSource / NativeContentPort /
    │   NativeContentConnector / NativeTextInput / NativeHistory
    │
    └── direct dlopen content FFI for Source bulk mutation
             │
             ▼
crates/iyon-tui-native
    ├── tui.rs
    │    ├── environment registries
    │    ├── N-API wrappers
    │    ├── input/output conversion
    │    └── theme/border decoding
    │
    ├── content_ffi.rs
    │    ├── fixed ABI records/statuses
    │    ├── pointer/length validation
    │    └── HostContentSource mutation calls
    │
    └── error.rs
             │
             ▼
iyon_tui::binding
    ├── TuiEnvironment
    ├── TuiHost
    ├── HostContentSource
    ├── HostContentPort
    ├── HostContentConnector
    ├── HostHistory
    ├── HostTextInput
    ├── HostViewSlot
    └── HostScrollPane
             │
             ▼
Rust application/runtime internals
    ├── content source registry and immutable source storage
    ├── host content registry
    ├── retained runtime and frame scheduler
    ├── terminal/input backend
    └── projection/layout/paint
```

### 3.2 Environment ownership

There are two process-global native registries in `tui.rs`:

```rust
static HOST_ENVIRONMENTS: OnceLock<Mutex<HashMap<usize, TuiEnvironment>>>
static CONTENT_ENVIRONMENTS: OnceLock<Mutex<HashMap<u32, TuiEnvironment>>>
```

- `HOST_ENVIRONMENTS` is keyed by the raw Node-API environment pointer (`tui.rs:42-47`).
- `CONTENT_ENVIRONMENTS` is keyed by the core environment slot (`tui.rs:43,49-51`).

`host_environment_for_env`:

1. Converts `Env::raw()` to a pointer-sized key.
2. Returns an existing cloned `TuiEnvironment` if present.
3. Otherwise creates a new `TuiEnvironment`.
4. Registers it in the content environment registry.
5. Installs an N-API environment cleanup hook.
6. Inserts it into the host environment registry.

The cleanup hook removes both the N-API environment entry and the content environment slot (`tui.rs:122-146`).

The direct FFI path does not receive an `Env`; it resolves a Source through `(environment_slot, environment_generation, source_slot, source_generation)` (`content_ffi.rs:202-213`). This allows direct bulk mutation to use the same environment-owned Source registry as N-API controls.

### 3.3 Source ownership

The core `HostContentSource` is environment-owned and backed by an `Arc<Mutex<ContentSourceRecord>>` (`crates/iyon-tui/src/application/content.rs:1,784-1,788`).

Source identity comprises:

- Source ID
- Source generation
- Environment slot
- Environment generation
- Content generation
- Mutation revision

The Source record tracks:

- Lifecycle (`Live` or `Disposed`)
- Text kind (`Stream` or `Block`)
- Immutable storage
- Retention policy
- Connector membership count
- Subscriber groups
- Accepted/copied/dropped byte accounting

Source mutation captures subscriber groups and wakes eligible hosts. A mutation can be accepted even if one subscriber host cannot be woken; the core returns the accepted revision and a drain hint while recording the wake failure (`content.rs:2,526-2,622`).

Source disposal is strict while connectors exist (`content.rs:2,637-2,669`):

```text
Source with connector_count != 0
    → SOURCE_IN_USE
Source with no memberships
    → lifecycle becomes Disposed
    → subscriber list cleared
    → registry entry removed
```

### 3.4 Port ownership

A `HostContentPort` is host-owned and carries:

- Port ID
- Port generation
- Content family
- Weak host reference
- Structural component attachment state in the host registry

The host creates it through `TuiHost::create_content_port` (`host.rs:1,064-1,075`). The port does not own Source data. Its job is to represent a host structural attachment and bind one or more Connector memberships.

The native wrapper owns only a clone of the host handle. Actual resource lifetime is controlled by the host/content registry.

### 3.5 Connector ownership

A `HostContentConnector` links:

```text
one Source + one Funnel + one ContentPort
```

The TypeScript facade additionally retains wrapper references to the Source and Port so the logical membership remains represented in the JS resource graph (`retained.ts:661-695`).

Connector activation/deactivation/disposal are core state transitions. Disposal can be deferred while host reconciliation or source cleanup is pending. The status phase is authoritative for final wrapper reconciliation.

### 3.6 Host ownership

`TuiHost` owns the native retained runtime via `Arc<Mutex<HostInner>>`. `TuiHost` is explicitly marked `Send` and `Sync` in core host code because all non-Send component registry/routing access is serialized through the inner mutex (`host.rs:951-959`).

A host owns:

- Running retained application/runtime state
- Backend
- Current and candidate frames
- Presentation receipt
- Structural/state/content candidate metadata
- Content host registry
- View-state registry
- Environment membership
- Host ID and epochs
- Pending/failed frame information

`TuiHost::open_in_environment` initializes the runtime, prepares an initial frame, registers the host with the environment, assigns the host ID, and presents the initial frame (`host.rs:968-1,044`).

`TuiHost::Drop` calls `close()` only if the host is the final strong owner of the inner Arc (`host.rs:1,662-1,667`). This prevents a wrapper drop from prematurely closing a host whose child handles still hold the host inner state. Actual final reclamation occurs when the last strong owner disappears.

### 3.7 Event/output ownership

The Rust host owns:

- Key decoding
- Focus and component routing
- Paste interception
- Typed output queues
- Terminal input polling
- Tick/stream wake processing
- Rendering after input

TypeScript owns:

- Opaque route IDs
- Registration of output consumption
- Wake-broker scheduling around native drain calls
- Higher-level application interpretation of route IDs and payloads

The native bridge does not expose Rust closures to JavaScript. Rust closures capture opaque route IDs and push `RoutedOutput` into a queue.

### 3.8 Ownership/lifetime diagram

```text
Node-API Env
    │ cleanup hook
    ▼
TuiEnvironment
    ├── environment slot + generation
    ├── Source registry
    │     └── HostContentSource records
    │             └── immutable Source storage
    └── host registry / pending queue / wake epoch
            │
            ├── TuiHost
            │     ├── HostInner
            │     ├── ContentHostRegistry
            │     │     ├── HostContentPort
            │     │     └── HostContentConnector
            │     ├── component registry
            │     ├── frame/presentation state
            │     └── output queue
            │
            └── drain / retry / commit reports

N-API wrappers hold clones or boxes of these handles.
Wrapper disposal marks the wrapper dead and/or requests core retirement.
Host close cascades content/view-state cleanup and unregisters the host.
Environment cleanup removes registry lookup availability.
```

---

## 4. Execution paths and state transitions

### 4.1 Native host creation

The N-API path is:

```text
NativeTuiHost::new
    → width/height i64 → u16 validation
    → host_environment_for_env
        → HOST_ENVIRONMENTS lookup
        → create/register TuiEnvironment if absent
        → install Env cleanup hook
    → TuiHost::open_in_environment
        → initialize RunningApp/backend/history/theme
        → prepare initial frame
        → register host in environment
        → present initial frame
    → NativeTuiHost { Box<TuiHost>, alive, view_runtime }
```

Evidence:

- `tui.rs:611-634`
- `tui.rs:122-146`
- `host.rs:968-1,044`

Failure before host creation leaves no returned wrapper. If the cleanup hook cannot be installed, the newly registered content environment is removed (`tui.rs:133-146`).

### 4.2 Source creation

```text
NativeTextSource::new
    → parse kind and retention DTO
    → host_environment_for_env
    → TuiEnvironment::create_content_source
    → optional HostContentSource::configure_retention
    → NativeTextSource
```

Retention parsing (`tui.rs:1,433-1,504`) requires:

- Outer object keys only `retention`
- Retention keys only `maxBytes`, `maxLines`, `overflow`
- At least one positive bound
- Values positive and <= JavaScript safe integer max (`9_007_199_254_740_991`)
- Overflow mode exactly `drop-oldest` or `error`

If retention configuration fails, the constructor attempts Source cleanup before returning the error (`tui.rs:1,113-1,126`). If cleanup also fails, the returned error becomes an internal composite diagnostic.

### 4.3 Source append through direct FFI

The TypeScript direct loader declares the six native symbols in `packages/iyon-tui/src/transport/content/ffi.ts:74-124` and loads them via `dlopen` at lines 250-272.

The complete append path is:

```text
TextSource.append / appendUtf8
    → TypeScript UTF-8 encoding
    → annotation record/payload encoding
    → Source identity extraction
    → direct symbol iyon_tui_source_append_utf8_v1
    → content_ffi::guarded
    → output_result
    → source_for_identity
        → content_environment_for_identity
        → environment generation check
        → source registry lookup
        → source generation check
    → input_bytes / input_records validation
    → HostContentSource::append_utf8
    → immutable Source mutation
    → subscriber wake attempts
    → fixed output result
    → TypeScript finishMutation
    → requestWake if flag set
```

The FFI entrypoint is `content_ffi.rs:347-381`.

The core append operation (`content.rs:2,301-2,384`) performs:

- Source lifecycle check
- Stream-kind check
- Sealed check
- Payload size check
- UTF-8 validation
- Absolute source offset calculation
- Annotation validation
- Retention preflight
- Revision arithmetic preflight
- Immutable chunk installation
- Revision/accounting update
- Subscriber capture
- Host wake fanout

A failed host wake does not roll back accepted content. The core explicitly records the wake failure and returns a successful `ContentMutationResult` with `schedule_environment_drain = true` (`content.rs:2,537-2,622`).

The native test `content_ffi.rs:500-584` verifies this behavior:

1. Two hosts share one environment and Source.
2. One host lock is intentionally poisoned.
3. Append succeeds with revision `1`.
4. The direct FFI result includes a drain flag.
5. The healthy host still commits and renders.
6. The failed host appears in the later drain report as `SOURCE_WAKE_FAILED`.
7. A subsequent idle drain does not spin automatically.

### 4.4 Source replace, clear, seal, and head truncate

The direct ABI exports:

- `iyon_tui_source_replace_utf8_v1` (`content_ffi.rs:383-417`)
- `iyon_tui_source_clear_v1` (`content_ffi.rs:419-437`)
- `iyon_tui_source_seal_v1` (`content_ffi.rs:439-457`)
- `iyon_tui_source_head_truncate_v1` (`content_ffi.rs:459-483`)

The result format is the same for each operation:

```text
source revision low/high
environment wake epoch low/high
flags
reserved
```

The TypeScript direct lane centralizes result decoding in `finishMutation` (`ffi.ts:190-217`):

- Unknown numeric status → ABI mismatch error
- Non-OK status → named content error
- Drain flag → `requestWake()`
- Return authoritative revision and wake epoch

The Rust wrapper does not duplicate these bulk methods as N-API calls.

### 4.5 Port/Connector creation and activation

```text
NativeTuiHost::content_port("text")
    → TuiHost::create_content_port(Text)
    → HostContentPort
    → NativeContentPort

NativeContentPort::connect(source, controls)
    → wrapper liveness checks
    → parse_text_funnel_control
    → HostContentPort::connect
        → source membership acquisition
        → connector registration
    → NativeContentConnector
```

`parse_text_funnel_control` (`tui.rs:1,506-1,564`) maps caller strings to core enums:

- `plain` / `markdown` / `diff` / `ansi`
- `word` / `grapheme` / `noWrap`
- immediate or smooth delivery

Smooth values must be finite and representable as `f32`. `SmoothConfig::try_from_parts` performs the final semantic validation.

Activation:

```text
NativeContentConnector::activate
    → HostContentConnector::activate
    → core connector requested state
    → WakeDisposition JSON
```

Visibility and projection are not established merely by activation. The Port must be mounted and selected through a retained View structure, then a host drain/frame commit must occur. This is why `status()` reports both `requested` and `visible`.

### 4.6 Host desired structure and content visibility

The content lane interacts with retained structure as follows:

```text
ContentPort created by host
    ↓
ContentPort linked to Source/Funnel
    ↓
Port attached to caller-supplied View through structural content attachment
    ↓
NativeTuiHost::setDesiredViewRef
    ↓
host accepts desired structural root
    ↓
environment queue receives pending host
    ↓
flushPendingHosts / runtime wake broker
    ↓
host prepares content projection/layout/paint candidate
    ↓
presentation commit
    ↓
Port becomes visible
```

`NativeTuiHost::set_desired_view_ref` only accepts the desired structure and returns a scheduling hint (`tui.rs:655-672`). It does not claim that the content is visible.

`flush_pending_hosts` returns separate commit/error data and preserves retry-blocked hosts rather than treating a failed frame as a successful visible commit (`tui.rs:683-739`).

### 4.7 Input event path

#### Key binding

```text
NativeTuiHost::bindKey(key, modifiers, routeId)
    → parse_key
    → TuiHost::bind_key
    → host_bind_key registers Rust closure
    → closure enqueues RoutedOutput { route_id, payload: None }
```

`parse_key` (`tui.rs:1,043-1,086`) supports named keys including Enter, Escape, arrows, paging, and editing keys. Other strings must contain exactly one Unicode scalar value. Modifiers accept:

- `shift`
- `control` / `ctrl`
- `alt` / `option`
- `super` / `meta`

Unknown or malformed keys fail with `ION_INVALID_INPUT`.

#### Terminal polling

```text
NativeTuiHost::pollTerminal
    → TuiHost::poll_terminal
    → sync real time
    → bounded input pump
    → native RunningApp dispatch
    → stop consuming after routed action
    → advance_and_render
```

The host implementation comments at `host.rs:1,499-1,507` show that after a routed action the caller must reduce that action before later keystrokes can alter focus or clear the composer.

#### Manual key/paste dispatch

- `dispatchKey` calls core `dispatch_key` then `advance_and_render` (`host.rs:1,364-1,371`).
- `dispatchPaste` calls core `dispatch_paste` then renders (`host.rs:1,373-1,380`).
- `forwardPaste` invokes host-level paste forwarding and renders (`host.rs:1,382-1,389`).

#### Paste interception

```text
NativeTuiHost::interceptPaste(input, routeId)
    → require NativeTextInput.host
    → TuiHost::intercept_paste
    → output route registration associated with input
```

The wrapper rejects detached/unmounted inputs before registering the route (`tui.rs:929-938`).

#### Output consumption

- `nextOutput` pops one routed output synchronously (`tui.rs:973-978`).
- `waitForOutput` clones `TuiHost`, awaits core output, and maps it to JSON (`tui.rs:981-992`).
- Core `wait_for_output` keeps terminal input, ticks, stream wakeups, and rendering on Rust (`host.rs:1,510-1,513`).

There is no native callback invocation into TypeScript. The callback-like behavior is a Rust closure writing to a host-owned output queue, followed by explicit JS polling/awaiting.

### 4.8 TextInput output route

```text
NativeTextInput::submitted
    → HostTextInput::submitted / detached TextInput::submitted
    → NativeTuiOutput { Output<String> }

NativeTuiHost::route(output, routeId)
    → TuiHost::route_text_input_output
    → generic output route registration

Terminal submit
    → native input/component handling
    → Output<T> routed to route ID
    → nextOutput / waitForOutput
```

The TypeScript facade caches the returned output channel to preserve route identity across repeated calls.

### 4.9 Host disposal and shutdown

`NativeTuiHost::dispose` (`tui.rs:742-750`):

1. Atomically changes wrapper liveness from true to false.
2. Aborts all outstanding structural edit transactions in the per-environment view runtime.
3. Calls `TuiHost::close`.

Core `TuiHost::close`:

- Waits for a pending presentation where possible.
- Drops retained semantic root/body.
- Clears retained views.
- Disposes view states.
- Disposes all content resources.
- Marks the host closed.
- Restores the terminal for real backends.
- Unregisters the host from its environment.

Evidence: `host.rs:1,558-1,609`.

`disposeContentResources` is a narrower explicit cascade that invalidates host-owned content resources while leaving the host alive (`tui.rs:753-758`, `host.rs:1,077-1,083`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production path matrix

| Semantic operation | Primary native path | Alternate path | Selection condition | Failure/recovery semantics |
|---|---|---|---|---|
| Create text Source | `NativeTextSource::new` → `TuiEnvironment::create_content_source` | None observed in native facade | N-API control operation | Invalid kind/options fail before or during Source creation; retention setup attempts cleanup |
| Append UTF-8 Source data | Direct `iyon_tui_source_append_utf8_v1` | None in N-API wrapper | High-volume bulk data lane | Accepted Source revision is authoritative even if a host wake fails; later drain reports wake failure |
| Replace UTF-8 Source data | Direct `iyon_tui_source_replace_utf8_v1` | None in N-API wrapper | Direct content FFI | Atomic core replacement; sealed/invalid/retention failures are returned without accepting mutation |
| Clear Source | Direct `iyon_tui_source_clear_v1` | None in N-API wrapper | Direct content FFI | Core lifecycle/sealed checks; revision and drain result returned |
| Seal Source | Direct `iyon_tui_source_seal_v1` | None in N-API wrapper | Direct content FFI | Already-sealed and lifecycle failures use numeric statuses |
| Truncate Source head | Direct `iyon_tui_source_head_truncate_v1` | None in N-API wrapper | Direct content FFI | Absolute coordinates preserved; retention and range failures returned |
| Connect Source/Funnel/Port | `NativeContentPort::connect` | TypeScript control helper only delegates | N-API control lane | Family/funnel/wrap/smooth validation; core membership errors are content-coded |
| Activate Connector | `NativeContentConnector::activate` | TypeScript `activateContent` delegate | N-API control lane | Returns wake hint; actual visibility waits for mount/frame commit |
| Deactivate Connector | `NativeContentConnector::deactivate` | TypeScript `deactivateContent` delegate | N-API control lane | Returns wake hint; status tracks requested/visible transitions |
| Dispose Connector | `NativeContentConnector::dispose` | Host `disposeContentResources` / host close | Logical connector teardown versus owner cascade | Core may defer cleanup; wrapper status must be polled until `disposed` |
| Set desired retained root | `NativeTuiHost::setDesiredViewRef` | Core/internal `render` is not exposed through this wrapper | Native retained View reference available | Accepts desired state only; environment drain performs transaction |
| Drain pending hosts | `NativeTuiHost::flushPendingHosts` | Runtime wake broker calls same method | Explicit barrier or automatic runtime drain | Budget constrained; retry-blocked hosts only retried with `forceRetry` |
| Terminal key handling | `pollTerminal` / `waitForOutput` | `dispatchKey` | Real terminal versus deterministic/manual event injection | Native Rust routing; output queue remains authoritative |
| Paste handling | `pollTerminal`, `dispatchPaste`, `interceptPaste`, `forwardPaste` | None observed | Input source/control routing | Input errors remain native; route IDs are opaque |
| TextInput submit | `NativeTextInput::submitted` + `NativeTuiHost::route` | None observed | Typed output channel | Output value is returned to native host and then routed by opaque ID |
| Theme update | `setTheme` → `decode_theme` | None observed | N-API theme object | Typed decode fails closed; canonical Theme assembled only after all DTOs validate |
| TextInput border | `textInput` → predecode border → create input → apply border | None observed | Optional border object | Decode before registration; post-registration application failure retires input |
| Host shutdown | `dispose` → `close` | `exit` for semantic terminal exit | Wrapper disposal versus application exit | Core closes/restores/unregisters; terminal restore failures are surfaced except recognized shutdown conditions |
| Native artifact check | `nativeVersion` | `tuiSmoke` probe | Loader/integration test | Build identity mismatch is a load-time failure |

### 5.2 Cache miss versus recovery versus compatibility

The native wrapper layer does not implement a projection cache. Core content and host registries do.

Observed classes of alternate behavior:

- **Recovery:** environment drain retry after accepted Source wake failure.
- **Recovery:** explicit `forceRetry` on `flushPendingHosts` for retry-blocked hosts.
- **Deferred lifecycle:** Connector disposal remains in a `disposing`/cleanup state until core membership cleanup succeeds.
- **Compatibility:** `NativeContentPort::attachmentId` aliases `portId` to satisfy the structural attachment contract.
- **Compatibility:** `NativeContentConnector` keeps the status shape's single `error` field while cleanup and operating errors remain separate inside the core.
- **Feature qualification:** direct structural ABI probes are behind the `direct-ffi` feature; content ABI symbols are part of the shipped/default native artifact according to `Cargo.toml:11-16`.
- **Potentially dangerous fallback:** unknown diagnostics in `status_for_diagnostic` become `CONTENT_STATUS_INTERNAL_INVARIANT`; unknown content codes in `NativeError::content` become `ION_INTERNAL`. These are explicit collapse paths, not silent successful fallbacks, but they can reduce diagnostic specificity.

### 5.3 Failure masking and ordering

The native facade generally validates before core registration:

- Source kind/options before Source setup
- Content family before Port creation
- Border DTO before TextInput registration
- Key parsing before route registration
- View references before host slot/pane creation

The strongest failure-semantics rule is Source mutation acceptance:

```text
Source storage/revision accepted
    ≠
all subscriber host wakes succeeded
```

The core deliberately does not convert post-acceptance wake failure into a mutation failure, because retrying the whole mutation could duplicate content. Native direct FFI preserves the accepted revision and returns a drain hint.

### 5.4 Absence claims

Within the inspected native handwritten files:

- No Rust-to-JavaScript callback closure is exposed.
- No product-specific event/action enum exists.
- No application-specific Source meaning exists.
- No N-API Source bulk mutation methods exist; direct FFI is the production path.
- No direct native projection/layout/paint work occurs inside `content_ffi.rs`.
- No pointer is intentionally retained after a direct FFI call returns.
- No native wrapper method silently returns a successful default for a disposed wrapper; `ensure_alive` fails explicitly.

These absence claims are limited to the inspected native handwritten scope and selected core seams, not the entire repository.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Native-side retained identity and caches

Native handwritten code maintains two environment lookup registries:

- `HOST_ENVIRONMENTS`: Node-API Env pointer → `TuiEnvironment`
- `CONTENT_ENVIRONMENTS`: environment slot → `TuiEnvironment`

They are initialized lazily with `OnceLock` and guarded by `Mutex`.

Retention bounds:

- Environment maps are retained until N-API environment cleanup or explicit test removal.
- Source storage and content records are owned by the core environment registry.
- Native wrappers retain Arc/Box handles but do not duplicate Source text for mutation.
- `NativeTextSource::snapshot` intentionally materializes a JSON text copy for inspection; this is not the frame path.
- Source snapshots in core retain immutable Arc-backed storage, enabling old snapshots to survive replacement.

No explicit size bound is present on the process-global environment map itself beyond lifecycle cleanup. Its boundedness depends on N-API environment cleanup and eventual source/host handle release.

### 6.2 Source mutation cost and copy behavior

The direct content ABI borrows caller buffers synchronously:

- `input_bytes` returns a borrowed slice.
- `input_records` returns a borrowed fixed-record slice.
- The core Source copies validated bytes into immutable chunks before releasing the Source lock.
- No pointer is retained after the call.

Limits in `content_ffi.rs`:

- Source payload: 64 MiB
- Annotation payload: 4 MiB
- Annotation records: 16,384
- Annotation record: 8 `u32` lanes / 32 bytes
- Mutation result: 24 bytes
- Metadata: 128 bytes

The core tracks:

- `accepted_bytes`
- `copied_bytes`
- `dropped_head_bytes`
- retained bytes/lines/chunk count

These are returned through `NativeTextSource::stats`.

### 6.3 Invalidation and scheduling

Source mutation:

```text
Source mutation
    → revision increments
    → matching subscribers captured
    → host content marked pending
    → environment wake latch/epoch updated
    → direct result flags schedule_environment_drain
    → TypeScript wake broker schedules host drain
```

A failed subscriber wake sets the drain hint and records a per-host error rather than cancelling healthy subscribers.

Host structure mutation:

```text
setDesiredViewRef
    → desired structural revision changes
    → environment pending-host queue
    → flush_pending_hosts
    → candidate prepare/commit
    → visible structural/frame epoch changes
```

Host drain budget is caller supplied and bounded from 1 through 1,024. Default is 32.

### 6.4 Per-append, per-tick, per-frame work

| Operation | Observed native behavior |
|---|---|
| Per Source append | UTF-8/annotation validation, core immutable chunk installation, revision/accounting, subscriber wake fanout |
| Per Source snapshot query | Core snapshot acquisition and JSON materialization of text/annotations |
| Per ContentPort connect | Funnel DTO lowering and core connector membership registration |
| Per Connector activate/deactivate | Core control transition and wake disposition |
| Per smoothing tick | Native host/core scheduler owns tick and projection delivery; TypeScript does not drive each tick |
| Per host drain | Fair queue drain up to budget, candidate frame preparation/commit, structured errors |
| Per frame | Core layout/paint/backend work; native wrapper only serializes selected results |
| Per event | Native parse/dispatch, render advancement, output queue insertion |
| Per `nextOutput` | Queue pop and JSON serialization |
| Per `waitForOutput` | Async native loop; TypeScript does not own event polling |

### 6.5 Theme/style cache considerations

`theme_dto.rs` batches Theme construction:

- `BTreeMap` gives deterministic key order.
- Variants are accumulated and passed to `Theme::assemble_batched`.
- Repeated style atoms are interned through `intern_style_atom`.

The native decoder does not retain a DTO cache. Core Theme/style systems own canonical records and any style resolution/caching beyond the DTO boundary.

### 6.6 Performance observability

Feature-gated methods in `tui.rs:249-263` expose core performance counter reset/snapshot:

- `tuiPerfReset`
- `tuiPerfSnapshot`

Feature-gated direct ABI qualification probes are at `tui.rs:161-247`. They report function addresses and generated conformance function addresses; they are not production content mutation paths.

The native content ABI metadata is a compatibility/performance contract rather than a runtime performance counter. It reports ABI layout, pointer width, endianness marker, record sizes/alignment, symbol count, and fingerprints.

---

## 7. Tests, benchmarks and observability

### 7.1 Native handwritten tests

#### Direct FFI wake failure fanout

`crates/iyon-tui-native/src/content_ffi.rs:500-584`:

`accepted_wake_failure_keeps_direct_ffi_result_and_healthy_fanout`

Protects:

- Shared environment/source identity
- Direct append result success
- Source revision authority
- Wake drain flag despite one failed host
- Healthy host still receives and renders content
- Failed host error appears in drain report
- No automatic retry spin after the failed wake

This is the most important assignment-specific behavioral test.

#### TextInput Unicode cursor ownership

`tui.rs:1,967-1,975`:

`native_text_input_owns_unicode_cursor_state`

Protects:

- Detached TextInput stores Unicode text
- Cursor position is measured in UTF-8 bytes
- Disposal makes later access fail

#### Connector status error lane

`tui.rs:1,977-2,015`:

`content_connector_status_maps_cleanup_through_the_existing_error_lane`

Protects:

- Cleanup error is exposed through the existing `error` field
- Cleanup error takes precedence over operating error
- No separate `cleanupPending` or `cleanupError` fields are introduced
- Operating error remains visible when no cleanup error exists

#### Theme/border DTO tests

`theme_dto.rs:441-597` protects:

- Sequential theme construction parity
- Strict rejection of malformed color objects
- Strict rejection of unknown attributes, roles, and parts
- Empty text selector semantics
- Border style/edge/color/glyph lowering parity
- Invalid border style/edge rejection

### 7.2 Native integration tests

`crates/iyon-tui-native/tests/sync.rs:1-9` verifies:

- `native_version() == "iyon-tui-native/s6"`
- `tui_smoke() == "iyon-tui/t1"`

`crates/iyon-tui-native/tests/generated_view_abi.rs` was indexed but is structural/generated coverage and not analyzed as assignment-18 ownership.

### 7.3 Rust core tests followed as seam evidence

Selected core tests in `crates/iyon-tui/src/application/content.rs` prove contracts consumed by the native boundary, including:

- Source retention and truncation
- Mutation atomicity
- Accepted revision despite wake failure
- Connector mount/activation
- Projection failures
- Theme/width invalidation
- Prepared content ticket selection
- Deferred cleanup/retry behavior

Examples visible in the inspected source include:

- `content.rs:7,333-7,344`: accepted mutation and wake failure reporting
- `content.rs:7,655-7,?`: unactivated connector does not requeue forever
- `content.rs:8,095-8,?`: content commit/revision tests
- `content.rs:8,765-8,?`: theme invalidation
- `content.rs:9,603-9,?`: prepared ticket cannot select a newer projection incorrectly

Exact full test inventory belongs to the Rust content assignment.

### 7.4 TypeScript route evidence

The TypeScript transport contracts were inspected to verify native consumers:

- `packages/iyon-tui/src/transport/native/addon.ts:48-97`: Source/Port/Connector/TextInput contracts
- `packages/iyon-tui/src/transport/native/addon.ts:134-188`: Host contract, epochs, drain reports, event methods
- `packages/iyon-tui/src/transport/content/control.ts:36-98`: control delegates
- `packages/iyon-tui/src/transport/content/ffi.ts:190-217,250-272,658-744`: direct FFI loading/result handling
- `packages/iyon-tui/src/api/content/retained.ts:560-810`: wrapper ownership, deferred connector disposal, status reconciliation

### 7.5 Validation status

No tests or builds were executed in this read-only investigation. All test statements above are source-based behavioral evidence, not observed execution results.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Generic framework boundary is maintained

The native files expose generic concepts only:

- Source
- Port
- Connector
- Funnel
- Host
- History
- TextInput
- Output
- Key/paste routing
- Theme/style
- View/slot/pane

No Iyon agent/application concepts were found in the inspected native handwritten scope. Route IDs are opaque caller values; the native layer does not interpret their product meaning.

This matches the ownership boundary in `AGENTS.md:77-128`.

### 8.2 Content data and content control are genuinely separate lanes

The source tree implements two distinct transport contracts:

```text
Control lane:
    N-API constructors and small control calls
    Source identity/snapshot/stats
    Port/Connector activation/deactivation/status

Data lane:
    direct C ABI
    borrowed UTF-8/annotation buffers
    fixed output record
    numeric statuses
    no projection/layout/paint/callback during mutation
```

This is not merely a naming split. The N-API Source class contains no payload mutation methods, while `content_ffi.rs` contains all five mutation operations and metadata.

### 8.3 Accepted mutation versus wake/commit is explicit

The native bridge preserves a crucial distinction:

```text
Source revision accepted
    → direct call returns success
    → drain flag may be true
    → host wake failures are reported later
    → visible frame commit is a separate event
```

The direct FFI test and TypeScript `finishMutation` implementation both preserve this distinction. This prevents retrying an accepted append merely because one host wake failed.

### 8.4 Deferred disposal crosses all three planes

Disposal is not uniform:

- N-API wrapper can become unusable immediately.
- Core connector/source/port membership may remain until host reconciliation.
- Component retirement may wait until the retained runtime proves unmount.
- Host close is the owner cascade that invalidates content resources and unregisters the host.

This is reflected in:

- `NativeContentConnector.status`
- TypeScript `ContentConnector.syncNativeLifecycle`
- `NativeViewSlot`/`NativeScrollPane` deferred-retirement comments
- `TuiHost::close` content disposal

### 8.5 NativeError translation loses some diagnostic specificity

`NativeError::content` only recognizes the whitelist in `error.rs:55-89`. Not every possible core/native content diagnostic is in that list. For example, status/cleanup-oriented diagnostics such as `SOURCE_CLEANUP_PENDING`, `SOURCE_WAKE_FAILED`, and some ABI/panic categories are not promoted by this helper.

Consequences:

- Content operation failures known to the whitelist become `ION_<CODE>`.
- Unknown or newly introduced `CODE: detail` diagnostics become `ION_INTERNAL`.
- Connector `status()` avoids this collapse by serializing core status code/diagnostic directly.
- Direct FFI has its own numeric status map and separately maps unknown diagnostics to `CONTENT_STATUS_INTERNAL_INVARIANT`.

This is an observed translation asymmetry, not a recommendation.

### 8.6 Connector wrapper liveness is intentionally distinct from logical disposal

`NativeContentConnector::dispose` delegates disposal but does not set its wrapper `alive` flag false on success (`tui.rs:1,376-1,390`). The TypeScript wrapper then queries status and finalizes itself once the core reports phase `"disposed"` (`retained.ts:756-775`).

Therefore:

```text
native wrapper alive
    ≠
core connector logically active
```

This is consistent with deferred disposal/status reconciliation, but it differs from Source and Port wrappers, which set `alive = false` after successful core disposal.

### 8.7 `NativeHistory::dispose` does not own host teardown

Attached `NativeHistory` disposal only marks the wrapper dead. It does not retire the host History because the host owns the actual retained History. The host close path remains responsible for eventual cleanup.

### 8.8 Host environment cleanup and stale direct FFI handles

The direct FFI identity includes environment slot and generation. `content_environment_for_identity` rejects:

- Missing environment slot
- Mismatched environment generation

This prevents a stale direct Source handle from silently resolving to a newly created environment that reused a slot. The content environment map is removed by the N-API environment cleanup hook.

### 8.9 Pointer contracts are safe only under the stated caller contract

`content_ffi.rs` validates:

- Null pointers
- Alignment
- Maximum lengths/counts
- Output pointer alignment
- Metadata output size
- Synchronous lifetime assumptions

However, a raw C caller must still ensure that a non-null pointer actually references the number of bytes/records declared. The ABI cannot determine the allocation size from a raw pointer. The checked-in TypeScript direct loader satisfies this contract by passing live TypedArrays for the duration of synchronous calls.

### 8.10 Native module is a broad facade rather than narrow submodules

`tui.rs` contains host lifecycle, controls, content wrappers, event routing, theme helpers, and tests in one handwritten file. This creates a visible coupling point:

```text
NativeTuiHost
    ↔ content controls
    ↔ structural View references
    ↔ state/view transaction aborts
    ↔ theme DTO
    ↔ input/output routing
```

This is current structure, not a V5 judgment. The coupling is consequential because disposal and environment identity cross content, structural, state, and event responsibilities.

---

## 9. Open questions and coverage gaps

1. **Compiled ABI validation:** The direct content ABI metadata and fingerprints were statically inspected, but no compiled artifact was loaded or probed during this investigation.
2. **Platform-specific pointer behavior:** The code asserts fixed Rust sizes/alignment, but cross-platform C compiler layout was not executed.
3. **N-API cleanup timing:** Source/environment registry behavior was inspected statically; the timing of `Env` cleanup hooks relative to JavaScript finalizers was not observed.
4. **Async wait cancellation:** `NativeTuiHost::wait_for_output` checks liveness before awaiting and clones the host. The behavior if another thread disposes the host while the await is pending was not executed.
5. **Host drop with surviving child handles:** Core `TuiHost::Drop` only closes when its Arc is the final strong owner. The eventual behavior of all combinations of host wrapper, History, Port, Connector, and control handles should be confirmed with lifecycle tests.
6. **Output queue bounds:** `next_output` pops from the host queue, but no explicit queue bound is visible in the native facade. Queue retention/backpressure belongs to core host/runtime code and was not fully audited here.
7. **Status phase transitions:** The native wrapper serializes core status but does not define the phase state machine. Complete phase transition guarantees belong to the Rust content assignment and TypeScript retained-content assignment.
8. **Error whitelist evolution:** There is no generated linkage between the core content diagnostic universe and `error.rs::is_content_code`. A future core diagnostic can collapse to `ION_INTERNAL` unless the whitelist is updated.
9. **Raw C caller misuse:** The FFI pointer contract assumes truthful lengths/counts. The TypeScript loader is trusted, but no standalone malformed raw-C caller test was inspected.
10. **Direct FFI symbol discoverability:** The symbols are exported from a private Rust module using `no_mangle`, but no platform linker/export inspection was performed.
11. **Theme DTO unknown-field behavior:** `serde` DTOs reject malformed leaf shapes, but the exact behavior for every unknown top-level/theme object field was not exhaustively enumerated.
12. **Callback terminology:** The native layer has Rust closures for route registration, but no direct JS callback. Whether product-facing documentation calls these “callbacks” or “routed outputs” should remain explicit to avoid implying cross-language closure ownership.
13. **NativeTuiOutput construction:** The Rust struct is marked `#[napi]` and returned from `submitted`; the TypeScript contract also declares an optional constructor. The intended external constructibility of this class was not separately verified.
14. **Source mutation batching:** The current direct ABI handles one mutation per call. No native-side multi-append batch operation was found in the inspected scope.

---

## 10. Evidence appendix

### 10.1 Primary inspected files

#### Native handwritten production

- `crates/iyon-tui-native/src/lib.rs`
- `crates/iyon-tui-native/src/error.rs`
- `crates/iyon-tui-native/src/sync.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/tui/theme_dto.rs`

#### Native configuration and ABI header

- `crates/iyon-tui-native/Cargo.toml`
- `crates/iyon-tui-native/include/iyon_content_abi.h`

#### Native tests

- `crates/iyon-tui-native/tests/sync.rs`
- `crates/iyon-tui-native/tests/generated_view_abi.rs` — indexed, structural/generated, not analyzed as assignment ownership

#### Core seam files

- `crates/iyon-tui/src/application/content.rs` — selected Source/Port/Connector/wake/lifecycle symbols
- `crates/iyon-tui/src/application/host.rs` — selected host creation, drain, input/output, close, and ownership symbols
- `crates/iyon-tui/src/application/environment.rs` — selected environment Source creation and pending-host drain symbols

#### TypeScript consumers

- `packages/iyon-tui/src/transport/native/addon.ts`
- `packages/iyon-tui/src/transport/native/factories.ts`
- `packages/iyon-tui/src/transport/content/control.ts`
- `packages/iyon-tui/src/transport/content/ffi.ts`
- `packages/iyon-tui/src/transport/content/abi.ts`
- `packages/iyon-tui/src/api/content/retained.ts`
- `packages/iyon-tui/src/api/controls/text-input.ts`
- `packages/iyon-tui/src/runtime/wake-broker.ts`

#### Repository instructions/context

- `AGENTS.md`
- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `PRE-V5-ARCHITECTURE-REPORT.md`

### 10.2 Exact symbol evidence

#### Direct content ABI

- `content_ffi.rs:16-46` — ABI constants/status table
- `content_ffi.rs:72-110` — metadata/result structs and compile-time size/alignment assertions
- `content_ffi.rs:113-152` — diagnostic/result conversion
- `content_ffi.rs:154-198` — output/input pointer validation
- `content_ffi.rs:200-213` — environment/source identity lookup
- `content_ffi.rs:215-306` — shared mutation helpers and panic guard
- `content_ffi.rs:309-344` — metadata symbol
- `content_ffi.rs:347-483` — append/replace/clear/seal/truncate symbols
- `content_ffi.rs:486-585` — direct FFI wake-failure test

#### Native environment/host facade

- `tui.rs:42-146` — environment registries and N-API cleanup hook
- `tui.rs:149-267` — smoke/probe/perf exports
- `tui.rs:275-296` — liveness and View-ref resolution
- `tui.rs:298-447` — `NativeHistory`
- `tui.rs:449-600` — `NativeTextInput`/`NativeTuiOutput`
- `tui.rs:602-1,041` — `NativeTuiHost`
- `tui.rs:1,043-1,086` — key/modifier parser
- `tui.rs:1,089-1,238` — `NativeTextSource`
- `tui.rs:1,240-1,342` — `NativeContentPort`
- `tui.rs:1,344-1,431` — `NativeContentConnector`
- `tui.rs:1,433-1,564` — Source retention and Funnel control parsing
- `tui.rs:1,566-1,856` — scroll pane and view slot wrappers
- `tui.rs:1,858-1,960` — scalar/style/color helpers
- `tui.rs:1,963-2,015` — native unit tests

#### Error conversion

- `error.rs:3-52` — NativeError categories and constructors
- `error.rs:55-89` — content diagnostic whitelist

#### Theme/border decoding

- `theme_dto.rs:1-12` — typed decoder contract
- `theme_dto.rs:31-114` — color/style DTOs
- `theme_dto.rs:117-234` — selectors and semantic text selectors
- `theme_dto.rs:236-286` — role/part validation
- `theme_dto.rs:288-357` — Theme DTO assembly
- `theme_dto.rs:360-438` — Border DTO lowering
- `theme_dto.rs:441-597` — theme/border behavioral tests

#### Core ownership/lifecycle seams

- `application/content.rs:508-517` — mutation result with revision/wake fields
- `application/content.rs:1,517-1,578` — Source lifecycle/record
- `application/content.rs:1,784-1,809` — Source ownership and validation helpers
- `application/content.rs:2,301-2,523` — Source mutations
- `application/content.rs:2,526-2,622` — wake fanout and accepted-mutation semantics
- `application/content.rs:2,637-2,711` — Source disposal and retention configuration
- `application/content.rs:6,707-6,?` — HostContentPort/Connector definitions and methods
- `application/host.rs:176-185` — HostInner drop cleanup/unregister
- `application/host.rs:951-1,044` — TuiHost creation/registration/initial presentation
- `application/host.rs:1,064-1,148` — content Port creation, resource disposal, epochs, desired structure, drain
- `application/host.rs:1,151-1,297` — controls and route registration
- `application/host.rs:1,364-1,389` — dispatch key/paste/forward paste
- `application/host.rs:1,411-1,513` — output queue, terminal state, wait loop
- `application/host.rs:1,558-1,609` — host close/shutdown
- `application/host.rs:1,662-1,667` — TuiHost Drop ownership rule
- `application/environment.rs:212-218` — environment Source creation
- `application/environment.rs:504-511` — pending-host drain

#### TypeScript native/content consumers

- `transport/native/addon.ts:48-97` — Source/Port/Connector/TextInput contracts
- `transport/native/addon.ts:134-188` — Host/epoch/drain/event contracts
- `transport/content/control.ts:36-98` — content control delegates
- `transport/content/ffi.ts:74-124` — direct ABI symbol declarations
- `transport/content/ffi.ts:190-217` — mutation result/status conversion
- `transport/content/ffi.ts:250-272` — dynamic loading and metadata validation
- `transport/content/ffi.ts:658-744` — direct append/replace/clear/seal/truncate calls
- `api/content/retained.ts:560-649` — Port ownership and Connector membership
- `api/content/retained.ts:661-810` — Connector deferred disposal/status lifecycle
- `api/controls/text-input.ts:56-65` — submitted output channel caching

### 10.3 Files indexed but not read as assignment-18 implementation

- `crates/iyon-tui-native/src/generated/view_abi_conformance.rs`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
- `crates/iyon-tui-native/src/generated/view_state_schema.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`
- `crates/iyon-tui-native/tests/generated_view_abi.rs`
- Bulk generated TypeScript ABI bodies
- Unrelated repository-wide tests, benchmarks, examples, and fixtures

### 10.4 LOC methodology

Physical LOC were counted from the checked-in handwritten native files returned by the repository manifest. Test regions were separated by the `#[cfg(test)]` modules:

- `content_ffi.rs`: lines 1-485 production; 486-585 tests
- `tui.rs`: lines 1-1,962 production; 1,963-2,017 tests
- `theme_dto.rs`: lines 1-440 production; 441-597 tests
- `error.rs`, `sync.rs`, and `lib.rs`: all production
- Generated, structural, and state files excluded from assignment totals

No generated code was attributed to the handwritten native implementation.