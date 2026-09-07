# 24 — Schema/codegen

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope assigned:
  - `tools/tui-abi/`
  - `tools/tui-abi-gen/`
  - generated Rust and TypeScript ABI/state outputs
  - `crates/iyon-tui-native/include/`
- Primary question: schema inputs, every generated output, consumers, checks, ownership, and ABI synchronization.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- repository `AGENTS.md`
- the complete ABI-generator source inventory and the relevant generated-file consumers.

This report maps the current implementation only. It does not make V5 migration/disposition decisions.

### Evidence status

This was a read-only static investigation. I did not modify source/configuration, install dependencies, run builds, run tests, regenerate outputs, or start services.

The report distinguishes:

- **Current source fact** — directly visible in the baseline files.
- **Static inference** — behavior inferred from generator/source relationships.
- **Historical evidence** — claims in prior reports, not independently re-executed here.
- **Unknown** — not established by this inspection.

Large generated files were indexed by headers, symbols, output line ranges, and consumer references rather than repeated line-by-line in the report. The generator source and schema were inspected in detail; large generated bodies were sampled at the seams and checked against generator declarations and generated metadata.

### Core result

The repository has one canonical structural ABI schema:

```text
tools/tui-abi/view_abi.toml
        +
packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json
        ↓
tools/tui-abi-gen
        ↓
16 generated outputs
        ↓
Rust native bridge + N-API session
TypeScript ABI/session facade + structural transport
C reference header
generated tests/bench metadata/docs
retained-state envelope packers and Rust envelope tables
```

The schema and generator embed two synchronization fingerprints into generated artifacts:

- schema BLAKE3:
  - `c697c2a2064686fae3f39ee1c120ea7da763069faf1fdaff4434b95119a65817`
- generator BLAKE3:
  - `57dcbc4070f1184a9472ecb4545b39c6c5a39d0d936e8071aeb36c0594be5010`

The generated TypeScript manifest and native runtime metadata are compared at runtime by `packages/iyon-tui/src/transport/structural/native-view-abi.ts`.

---

## 1. Responsibility and structure

### 1.1 Source and generator inventory

| Path | Language | Approximate production LOC | Tests/generated LOC | Responsibility |
|---|---|---:|---:|---|
| `tools/tui-abi/view_abi.toml` | TOML schema | ~3,674 | N/A | Canonical ABI, handle, enum, POD, function, conformance, and retained-state declarations |
| `tools/tui-abi-gen/Cargo.toml` | TOML manifest | ~40 | N/A | Private workspace generator crate and dependencies |
| `tools/tui-abi-gen/src/main.rs` | Rust | ~515 | included in source | CLI, workspace/schema resolution, rendering orchestration, output writing/checking, generator tests |
| `tools/tui-abi-gen/src/model.rs` | Rust | ~237 | included in source | Typed deserialization model for schema, diagnostics, and kind-code loading |
| `tools/tui-abi-gen/src/validate.rs` | Rust | ~711 | included in source | Schema semantic validation and bound/layout/lane/conformance checks |
| `tools/tui-abi-gen/src/render_rust.rs` | Rust | ~938 | included in source | Generated Rust types, wrappers, conformance functions, function table, N-API methods, generated Rust ABI tests |
| `tools/tui-abi-gen/src/render_typescript.rs` | Rust | ~335 | included in source | Generated TypeScript ABI declarations, calls, conformance helpers, benchmark registry, layout tests |
| `tools/tui-abi-gen/src/render_state.rs` | Rust | ~503 | included in source | Generated retained-state envelope packers and Rust mask/lane/check tables |
| `tools/tui-abi-gen/src/render_header.rs` | Rust | ~149 | included in source | Generated C ABI reference header |
| `tools/tui-abi-gen/src/render_manifest.rs` | Rust | ~300 | included in source | Generated banners, fingerprints, JSON manifest, human ABI reference, generator hash |
| `tools/tui-abi-gen/templates/*.txt` | Askama templates | small | N/A | Generated banners and C/TypeScript/reference-document preambles |
| `tools/tui-abi-gen/src/snapshots/...snap` | snapshot fixture | generated/test | generated | Snapshot of the canonical generated manifest/output rendering |

The generator implementation is approximately 3,700 Rust lines excluding the schema and snapshot. The schema itself is large because it explicitly declares every function argument, lowering, ownership policy, bound, benchmark registration, and state-property lane shape.

### 1.2 Generated output inventory

`tools/tui-abi-gen/src/main.rs` defines one authoritative `GENERATOR_OUTPUTS` list containing 16 outputs. The generated manifest repeats this list in `generated_outputs`.

#### Rust native generated outputs

1. `crates/iyon-tui-native/src/generated/view_abi_types.rs`
   - canonical pointer-free Rust ABI types/constants
   - schema/generator fingerprints
   - ABI metadata constants
   - POD structures and compile-time size/alignment assertions
   - `NativeViewAbiHeader`
   - enum representations and kind-code assertions

2. `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
   - generated validated wrapper layer
   - implementation trampolines
   - direct C ABI symbols behind `direct-ffi`
   - pointer/ref/buffer/enum/node-ID validation
   - panic conversion and result status conventions

3. `crates/iyon-tui-native/src/generated/view_abi_conformance.rs`
   - generated ABI conformance probe functions
   - scalar weighted-sum probes
   - pointer, buffer, and C-string probes
   - optional direct-FFI symbols behind `direct-ffi`

4. `crates/iyon-tui-native/src/generated/view_abi_table.rs`
   - `FunctionDescriptor`
   - static function metadata table
   - exported symbol names, family, hotness, ownership, bounds, and benchmark registration
   - `FUNCTION_COUNT`

5. `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
   - generated safe N-API methods attached to `NativeViewAbiSession`
   - typed-array, string, host-object, scalar, and conformance lowerings
   - N-API-facing camelCase method names

6. `crates/iyon-tui-native/src/generated/view_state_schema.rs`
   - generated retained-state envelope masks, IDs, lane offsets, lane lengths, and primitive encoding constants
   - generated `check_envelope` functions for geometry and presentation domains

#### Native header output

7. `crates/iyon-tui-native/include/iyon_view_abi.h`
   - generated C ABI reference header
   - C declarations for PODs, enums, ABI constants, all 60 structural functions, and 10 conformance functions
   - generated schema/generator fingerprints in the comment preamble

The sibling `crates/iyon-tui-native/include/iyon_content_abi.h` is **not** generated by `tui-abi-gen`. It is a separate handwritten content-data ABI header. It defines PERF-13 content metadata, annotation records, source mutation results, status codes, and content FFI declarations. This creates two different schema/ownership regimes under the same `include/` directory:

- generated structural/view ABI: `iyon_view_abi.h`
- handwritten content ABI: `iyon_content_abi.h`

#### TypeScript generated outputs

8. `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
   - `NativeViewAbiMetadata`
   - `NativeViewAbiHandle`
   - all generated N-API method signatures
   - host contract import from `transport/native/addon.ts`

9. `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts`
   - TypeScript wrappers for the 10 conformance methods

10. `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
    - generated transport call functions
    - `NativeAbiStatusError`
    - high-bit status handling
    - cache-miss detail lookup through `viewStatusDetail()`
    - ref-result checking

11. `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json`
    - complete machine-readable ABI manifest
    - fingerprints and ABI metadata
    - handles, enums, PODs, functions, conformance declarations, retained-state rows, and output list

12. `packages/iyon-tui/src/transport/state/generated/state_envelope.ts`
    - generated TypeScript retained-state envelope structure
    - geometry/presentation masks and lane counts
    - geometry/presentation property IDs/capabilities
    - set packers and clear-mask packers
    - fixed sub-encodings for alignment, borders, colors, glyphs, attributes, and style

#### Generated tests, benchmark metadata, and documentation

13. `packages/iyon-tui/tests/generated/view_abi_layout.test.ts`
    - generated Bun test pinning:
      - schema fingerprint
      - ABI version
      - function ordering
      - conformance ordering/signatures
      - POD size/alignment
      - argument lowerings
      - Bun/result-encoding metadata

14. `packages/iyon-tui/bench/generated/view_abi_cases.ts`
    - generated benchmark registry
    - function family/hotness
    - benchmark registration name
    - scalar argument count
    - buffer presence and declared limits

15. `crates/iyon-tui-native/tests/generated_view_abi.rs`
    - generated Rust integration test harness
    - generated stub implementations for all declared semantic functions
    - wrapper delegation, function-count, ABI-version, conformance-call, and invalid-input tests

16. `docs/history/perf/PERF-11-generated-abi-reference.md`
    - generated human-readable ABI reference
    - handle/POD/enum/function/conformance tables

### 1.3 Physical size observations

Approximate physical sizes based on indexed line extents:

- `view_abi.toml`: approximately 3.7k lines.
- Generated Rust exports: approximately 5k lines.
- Generated Rust integration test harness: approximately 1.4k lines.
- Generated Rust N-API methods: approximately 430 lines.
- Generated Rust function table: approximately 866 lines.
- Generated TypeScript manifest: approximately 4.3k lines.
- Generated TypeScript state envelope: approximately 360 lines.
- Generated TypeScript ABI interface: approximately 90 lines.
- Generator source: approximately 3.7k Rust lines.

These are physical line approximations, not semantic LOC. Generated output size is dominated by explicit per-function signatures, validation, wrappers, and manifest records.

### 1.4 Primary and secondary responsibilities

The schema is not merely a C signature list. It owns:

- naming and public symbol identity;
- ABI and semantic schema versions;
- Bun compatibility constraints;
- result encoding;
- handle representation, validity range, kind, and lifetime;
- enum-to-kind-code mappings;
- POD representation and expected layout;
- every function’s family and hotness;
- ownership and borrow duration;
- thread affinity;
- allocation and host mutation declarations;
- maximum input/buffer limits;
- arity specializations;
- benchmark registration names;
- ABI argument type and lowering;
- conformance fixture operations;
- retained-state property IDs, values, nullability, clearability, capabilities, and lane shape.

The generator then translates those declarations into multiple target representations without allowing each target to invent its own ABI contract.

---

## 2. Types, APIs and contracts

### 2.1 Schema document model

`tools/tui-abi-gen/src/model.rs` defines a typed `AbiDocument` with `serde(deny_unknown_fields)`:

```rust
pub struct AbiDocument {
    pub abi: AbiMetadata,
    pub handles: Vec<HandleSpec>,
    pub enums: Vec<EnumSpec>,
    pub pods: Vec<PodSpec>,
    pub functions: Vec<FunctionSpec>,
    pub conformance: Vec<ConformanceSpec>,
    pub state_properties: Vec<StatePropertySpec>,
}
```

The `deny_unknown_fields` policy is consequential: adding an undeclared schema key fails parsing rather than being silently ignored.

#### ABI metadata

`[abi]` currently declares:

- `name = "iyon_tui_view"`
- `version = 1`
- `semantic_schema = 1`
- `minimum_bun = "1.4.0"`
- `qualified_bun = "1.4.0"`
- `result_encoding = "u32_high_bit_status"`

The validator currently requires the Bun versions and result encoding to be exactly these Tranche-1 values.

#### Handles

Current handle declarations include:

| Handle | Rust representation | TypeScript representation | Lifetime | Kind/validity |
|---|---|---|---|---|
| `RuntimePtr` | `*mut NativeViewRuntime` | `Pointer` | environment | opaque non-null pointer |
| `HostPtr` | `*mut NativeHost` | `Pointer` | host | opaque non-null pointer |
| `ViewRef` | `u32` | `number` | runtime | `1..0x7fffffff`, kind `view` |
| `PathRef` | `u32` | `number` | runtime | `1..0x7fffffff`, kind `path` |
| `StyleRef` | `u32` | `number` | runtime | `1..0x7fffffff`, kind `style` |
| `StyleAtomRef` | `u32` | `number` | runtime | `1..0x7fffffff`, kind `style_atom` |
| `BuilderRef` | `u32` | `number` | runtime | `0x7ffe0001..0x7fff0001`, kind `builder` |
| `EditTxnRef` | `u32` | `number` | runtime | `0x7fff0001..0x7fffffff`, kind `edit_txn` |

The validator requires `kind` and `valid` to be either both present or both absent.

The generated Rust wrappers validate runtime/host pointers as non-null, native refs against valid ranges and expected kinds, and buffer arguments against declared capacities/counts.

#### Enums and external kind-code input

The schema declares:

- `WrapMode`
  - `WordThenGrapheme`
  - `Grapheme`
  - `NoWrap`
- `HorizontalAlign`
  - `Start`
  - `Center`
  - `End`

The numeric values are not manually assigned in `view_abi.toml`. Each enum value references a `source_key` in:

```text
packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json
```

`model::load_kind_codes` reads that JSON separately. Validation requires each source key to resolve to an integer fitting `u32`. Rust and C outputs use those resolved values and emit compile-time/static numeric assertions where applicable.

This is a second schema input, although the TOML file is the authoritative structural declaration.

#### PODs

The current schema declares one POD:

```text
AxisChildInputV1
    track_word: u32
    child_ref:  u32
```

Declared layout:

- `repr = "C"`
- `size = 8`
- `align = 4`

Validation recalculates field offsets, expected size, and maximum alignment from fixed-width primitive fields. Generated Rust emits `#[repr(C)]`, `bytemuck::Pod`, `bytemuck::Zeroable`, and `static_assertions`; generated C emits a corresponding typedef; generated TypeScript lowers arrays of the POD to `Uint32Array`.

### 2.2 Function surface

The generated table contains 60 ABI functions. The schema groups them into families including:

- runtime/diagnostic:
  - `runtime_noop`
  - `view_status_detail`
- render references:
  - `view_render_ref`
  - `host_render_ref`
- structural/state attachment:
  - `view_state_attach`
  - `view_content_host_create`
  - `view_spacer_create`
  - root scalar patches
- axis/row/column construction:
  - `view_axis_create_buffer`
  - `view_row_create_0..4`
  - `view_column_create_0..4`
- axis builders:
  - `axis_builder_begin`
  - `axis_builder_push`
  - `axis_builder_finish`
  - `axis_builder_abort`
- structural mutations:
  - child replacement/splice/grid operations
  - path-based axis/grid mutations
- buffer constructors:
  - grid, diff, decorated, text buffers
- generic constructors:
  - hanging, container, clamp, component
- lifecycle/exact lookup:
  - `view_release_many`
  - `view_ref_for_node_id`
- path operations:
  - `path_root`
  - `path_child`
- path text-layout patches:
  - generic and fixed-depth `d1..d4` forms
- edit transactions:
  - begin, add layout edit, commit/render, abort
- style atoms/styles:
  - `style_atom_create_cstring`
  - `style_create_bits`
- text constructors:
  - C-string, UTF-8, fixed-span, and buffer forms

The schema preserves specialized arity routes rather than forcing all operations through a single variadic/buffer route. Examples:

- row/column constructors have explicit 0–4 child variants;
- text constructors have explicit 1–4 span variants;
- path patches have explicit depth-specialized forms;
- `view_axis_create_buffer` declares arity specializations `0..8`;
- edit transaction layout insertion declares a maximum specialization of 16.

### 2.3 Function-level contract fields

Each function declares:

- `family`
- `hotness`
- `implementation`
- `ownership`
- `borrow_duration`
- `thread_affinity`
- `may_allocate_native_memory`
- `mutates_host_state`
- `max_buffer_bytes`
- `max_input_count`
- `arity_specializations`
- `benchmark_registration`
- return type
- ordered arguments and lowerings.

Current validation restricts:

- borrow duration to `call`;
- thread affinity to `owner_thread`;
- maximum input and buffer bounds to 16 MiB-equivalent limits;
- arity specializations to increasing order and values no greater than 16;
- variable buffers to no more than two per function;
- each variable buffer to exactly one capacity/length argument and one used-count argument;
- multi-buffer used-count pairings to explicit `buffer_used_of` declarations;
- allowed lowerings to a closed vocabulary.

Allowed ABI lowerings include:

```text
u8, u16, u32, i32, f32, f64,
node_id_pair,
native_ref,
runtime_ptr,
host_ptr,
buffer,
buffer_length,
buffer_used,
cstring_ephemeral,
pod_slice,
status_only,
native_ref_result
```

The validator cross-checks declared type and lowering. For example:

- `runtime_ptr` requires `RuntimePtr`;
- `host_ptr` requires `HostPtr`;
- `native_ref` requires a kind-bearing `u32` handle;
- `node_id_pair` requires `NodeId`;
- `pod_slice` requires a declared POD array;
- `cstring_ephemeral` requires `string`;
- `buffer_length` and `buffer_used` must refer to declared buffers;
- fixed element sizes are required to enforce byte bounds.

### 2.4 Result and failure contract

The ABI uses `u32_high_bit_status` for unsigned/reference-like results:

- high bit `0x8000_0000` indicates an error;
- zero is also rejected for reference results;
- generated Rust wrappers use distinct error literals for invalid input, buffer errors, count errors, and panic;
- generated TypeScript `view_calls.ts` converts rejected reference results to `NativeAbiStatusError`;
- `CACHE_MISS = 0x8000_0004` causes the TypeScript wrapper to call `runtime.viewStatusDetail()` and preserve native diagnostic detail.

Signed/status return types use negative status values instead of the high-bit encoding.

The generated TypeScript call layer only applies reference validation to reference-result return types. Scalar/status-returning functions return raw numeric results. This distinction is derived from `return` declarations in the schema.

### 2.5 Retained-state schema

The state schema is integrated into the same TOML document through 17 `[[state_property]]` rows.

#### Geometry domain: 10 properties

| ID | Name | Value kind | Nullable | Clearable | Capability | Words | Strings |
|---:|---|---|---:|---:|---|---:|---:|
| 0 | `width` | `size_mode` | no | yes | `node-kind` | 1 | 0 |
| 1 | `height` | `size_mode` | no | yes | `node-kind` | 1 | 0 |
| 2 | `padding` | `insets` | no | yes | `node-kind` | 4 | 0 |
| 3 | `minWidth` | `u16` | yes | yes | `node-kind` | 1 | 0 |
| 4 | `maxWidth` | `u16` | yes | yes | `node-kind` | 1 | 0 |
| 5 | `minHeight` | `u16` | yes | yes | `node-kind` | 1 | 0 |
| 6 | `maxHeight` | `u16` | yes | yes | `node-kind` | 1 | 0 |
| 7 | `gap` | `u16` | no | yes | `node-kind` | 1 | 0 |
| 8 | `alignment` | `alignment` | no | yes | `node-kind+axis` | 1 | 0 |
| 9 | `borderEdges` | `edges` | yes | yes | `node-kind` | 2 | 0 |

Geometry generated constants:

- `ALL_MASK = 0x3ff`
- `NULLABLE_MASK = 0x278`
- `CLEARABLE_MASK = 0x3ff`
- 14 word lanes
- zero string lanes

#### Presentation domain: 7 properties

| ID | Name | Value kind | Nullable | Clearable | Capability | Words | Strings |
|---:|---|---|---:|---:|---|---:|---:|
| 0 | `foreground` | `color` | yes | yes | `node-kind` | 0 | 1 |
| 1 | `background` | `color` | yes | yes | `node-kind` | 0 | 1 |
| 2 | `borderColor` | `color` | yes | yes | `node-kind` | 0 | 1 |
| 3 | `borderStyle` | `border_style` | yes | yes | `node-kind` | 1 | 0 |
| 4 | `borderGlyphs` | `glyphs` | yes | yes | `node-kind` | 0 | 8 |
| 5 | `textAttributes` | `text_attrs` | no | yes | `node-kind` | 2 | 0 |
| 6 | `style` | `style` | yes | yes | `node-kind` | 2 | 3 |

Presentation generated constants:

- `ALL_MASK = 0x7f`
- `NULLABLE_MASK = 0x5f`
- `CLEARABLE_MASK = 0x7f`
- 5 word lanes
- 14 string lanes

The state validator requires:

- exactly the two known domains;
- at least one property per domain;
- no more than 32 properties per domain;
- dense IDs beginning at zero;
- unique names;
- closed value-kind vocabulary;
- closed capability vocabulary;
- exact words/strings lane shape for each value kind.

This means IDs are intentionally stable semantic positions, not declaration-order conveniences. Rows are sorted by ID before generated offsets are emitted.

### 2.6 Conformance fixtures

The schema declares ten conformance fixtures:

- `u8_8`: eight `u8`, weighted sum → `u32`
- `u16_8`: eight `u16`, weighted sum → `u32`
- `u32_8`: eight `u32`, weighted sum → `u32`
- `u32_16`: sixteen `u32`, weighted sum → `u32`
- `i32_4`: four `i32`, weighted sum → `i32`
- `f32_4`: four `f32`, weighted sum → `f32`
- `f64_4`: four `f64`, weighted sum → `f64`
- `pointer`: pointer probe → `u32`
- `buffer`: buffer plus byte length probe → `u32`
- `cstring`: C-string hash probe → `u32`

These are not semantic TUI operations. They exist to exercise ABI scalar, pointer, buffer, and string conventions through generated Rust, N-API, C, TypeScript, and tests.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
view-kind-codes.json ───────┐
                            │
tools/tui-abi/view_abi.toml ─┼─> model::load + load_kind_codes
                            │
                            ▼
                    validate::validate
                            │
                            ▼
                    render_outputs
          ┌──────────────┬──┼───────────────┬────────────────┐
          ▼              ▼  ▼               ▼                ▼
    Rust types      Rust exports      Rust table      Rust conformance
          │              │              │                │
          ▼              ▼              ▼                ▼
    native Rust     handwritten      runtime ABI      ABI probes/tests
    consumers       implementations  metadata
          
          ┌────────────────┬────────────────┬──────────────┐
          ▼                ▼                ▼              ▼
     Rust N-API       C header       TS interface    TS call helpers
          │                                │              │
          ▼                                ▼              ▼
 NativeViewAbiSession                  addon.ts      structural transport
          │                                │              │
          └────────────────────────────┴──────────────┘
                           ▼
                 native-view-abi.ts

state_property rows
          ▼
  render_state::rust_schema ──> view_state_schema.rs ──> view_state.rs
  render_state::typescript_envelope
                                └─> state_envelope.ts ──> state/control.ts
```

### 3.2 Reverse consumers

#### `view_abi_types.rs`

Consumers:

- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- generated native ABI tests
- generated exports/N-API modules
- generated layout tests
- the generated C header conceptually mirrors it

Responsibilities consumed:

- ABI constants and fingerprints;
- `AxisChildInputV1`;
- generated enum values;
- result aliases;
- metadata versions.

#### `view_abi_exports.rs`

Consumers:

- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui.rs`
- generated native tests
- generated N-API methods
- feature-gated direct FFI surface

It wraps handwritten semantic implementation functions in `view_abi.rs`. The generated file does not implement View semantics itself.

#### `view_abi_napi.rs`

Consumer:

- `crates/iyon-tui-native/src/tui/view_abi.rs`

The generated N-API implementation is included inside the handwritten `NativeViewAbiSession` module. Its methods call generated validated wrappers, not separate semantic implementations.

#### `view_abi_table.rs`

Consumers:

- `view_abi.rs` metadata and diagnostics;
- generated Rust tests;
- runtime function-count metadata.

#### `view_abi_conformance.rs`

Consumers:

- `tui.rs` ABI conformance address probe;
- generated native tests;
- generated N-API conformance methods;
- generated C header;
- generated TypeScript conformance wrappers.

#### `view_abi.ts`

Consumers:

- `packages/iyon-tui/src/transport/native/addon.ts` as a type import/re-export;
- `view_calls.ts`;
- `native-view-abi.ts`;
- structural transport modules through generated calls;
- tests and fixture type-checking.

#### `view_calls.ts`

Consumers:

- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`;
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`;
- direct structural constructors/mutation paths.

The ownership checker explicitly forbids semantic API modules from importing generated ABI directly; generated ABI is intended to remain inside transport/native seams.

#### `state_envelope.ts`

Consumers:

- `packages/iyon-tui/src/transport/state/control.ts`;
- state transport callers through `geometryEnvelope`, `presentationEnvelope`, and clear-mask functions.

#### `view_state_schema.rs`

Consumers:

- `crates/iyon-tui-native/src/tui.rs` module inclusion;
- `crates/iyon-tui-native/src/tui/view_state.rs`;
- hand-written Rust state readers and validation code.

### 3.3 Ownership and lifecycle

The schema’s ownership declarations are descriptive and are turned into metadata, but actual object ownership is implemented by handwritten native/TS runtime code.

#### Environment/runtime ownership

- `RuntimePtr` belongs to the environment lifetime.
- `NativeViewAbiSession` holds a `ViewRuntimeHandle` in Rust.
- TypeScript calls `native.tuiViewAbiSession()` once and caches the resulting session in `native-view-abi.ts`.
- The TypeScript session is not intended to outlive addon teardown.
- Runtime generation is exposed through metadata and checked as a positive safe integer.

#### Native reference ownership

- `ViewRef`, `PathRef`, `StyleRef`, and `StyleAtomRef` are runtime-local `u32` handles.
- The generated wrapper validates the numerical range and kind.
- Actual reference lease, slot, semantic identity, and stale-reference behavior is implemented in handwritten `view_abi.rs`.
- `viewReleaseMany` is generated from the lifecycle function declaration and is consumed by retained DAG transaction cleanup.
- TypeScript materialization code explicitly releases temporary leases and promotes borrowed identity hints when necessary.

#### Builder and edit transaction ownership

- `BuilderRef` is runtime-owned and uses a reserved numerical range.
- `EditTxnRef` is runtime-owned and uses a distinct reserved range.
- `axis_builder_begin/push/finish/abort` and `edit_txn_begin/add/commit/abort` encode creation, mutation, commit, and abort operations.
- The generated wrappers validate handles; handwritten `view_abi.rs` owns builder/transaction implementation and lifecycle semantics.
- TypeScript `retained-dag.ts` owns higher-level transaction sequencing and cleanup.

#### Host ownership

- `HostPtr` has host lifetime.
- `host_render_ref` and `edit_txn_commit_render` are declared as host-mutating or host-taking operations.
- N-API uses an opaque typed `NativeTuiHost` object rather than exposing a raw pointer to JavaScript.
- The generated C header exposes `NativeHost *` only for the external/direct ABI representation.

### 3.4 Implementation ownership

The generator owns:

- signatures;
- wrappers;
- validation scaffolding;
- lowerings;
- metadata;
- static assertions;
- generated tests/fixtures.

Handwritten implementations own:

- semantic View construction;
- retained references and lease behavior;
- runtime generation;
- native identity caches;
- state readers;
- host rendering;
- actual layout/content/style behavior;
- N-API runtime/session object creation;
- content ABI implementation.

This split is visible in `render_rust::exports`: each schema function creates a generated `generated_impls::<implementation>` trampoline that calls a same-named handwritten implementation via `super::super::<implementation>`.

---

## 4. Execution paths and state transitions

### 4.1 Generator execution path

#### Generate

`cargo run -p tui-abi-gen -- generate`:

1. CLI parses `Generate`.
2. `workspace_root()` resolves the Cargo workspace through `cargo_metadata`.
3. Default input is `tools/tui-abi/view_abi.toml`.
4. Default output root is the workspace root.
5. `render_outputs()`:
   - loads TOML syntax and typed model;
   - loads `view-kind-codes.json`;
   - validates the complete document;
   - computes schema BLAKE3 from the exact TOML source;
   - computes generator BLAKE3 from included generator/template bytes;
   - renders all 16 outputs.
6. `write_outputs()` creates parent directories and writes every output.

#### Check

`cargo run -p tui-abi-gen -- check`:

1. Performs the same load/validate/hash/render path.
2. Writes expected outputs to a temporary directory.
3. Reads every actual output in `GENERATOR_OUTPUTS`.
4. Fails if an output is missing or byte-different.
5. Returns a stale-output error naming the first stale path.

This is an exact byte freshness check, not a semantic comparison.

#### PrintManifest

`PrintManifest` renders all outputs but prints only the generated JSON manifest.

#### Explain

`Explain <function>` loads the typed schema and prints:

- source span;
- family;
- hotness;
- implementation;
- ownership;
- borrow duration;
- thread affinity;
- allocation/host mutation flags;
- bounds;
- arity specializations;
- benchmark registration;
- return type;
- argument lowerings.

This is a diagnostic path for reconstructing a function’s declared ABI contract.

### 4.2 Native N-API first-use path

Static call chain:

```text
TS nativeViewAbiSession()
    ↓
native.tuiViewAbiSession()
    ↓
NativeViewAbiSession::tui_view_abi_session
    ↓
runtime_handle_for_env
    ↓
NativeViewAbiSession::metadata()
    ↓
generated_types::{ABI constants, fingerprints}
generated_table::FUNCTION_COUNT
    ↓
TS compares metadata with generated manifest
    ↓
runtime.runtimeNoop() bootstrap probe
    ↓
cached NativeViewAbiSession
```

`native-view-abi.ts` rejects the native addon if any of these differ:

- ABI name;
- ABI version;
- semantic schema version;
- schema fingerprint;
- generator fingerprint;
- transport string;
- generation validity;
- function count.

This is the strongest explicit cross-plane synchronization check in the active TypeScript path.

### 4.3 Structural call path

For an ordinary structural operation:

```text
semantic/retained TypeScript caller
    ↓
transport/structural/native-view-abi.ts
    ↓
generated view_calls.ts helper
    ↓
NativeViewAbiHandle method
    ↓
generated view_abi_napi.rs method
    ↓
generated_exports::invoke_iyon_*_v1
    ↓
generated validation
    ↓
handwritten implementation in tui/view_abi.rs
    ↓
native runtime/reference/state/host update
    ↓
numeric result or status
```

The generated TypeScript helper adds ref-result checking. `NativeAbiStatusError` retains the status and, on cache miss, retrieves native detail through `viewStatusDetail`.

### 4.4 Retained-state envelope path

```text
public/state transport patch
    ↓
transport/state/control.ts normalizer
    ↓
generated state_envelope.ts packer
    ↓
StateEnvelope { setMask, nullMask, clearMask, words, strings }
    ↓
native state transport
    ↓
handwritten view_state.rs readers
    ↓
generated view_state_schema.rs masks/offsets/check_envelope
    ↓
state mutation/invalidation/wake result
```

The generator deliberately does not own all value semantics. `render_state.rs` comments state:

- public validation and exact error shape remain in handwritten `transport/state/control.ts`;
- generated packers translate already-normalized values;
- native value semantics remain in handwritten readers;
- generated state code owns masks, IDs, offsets, lane shape, and envelope-header validation.

### 4.5 Replacement/reset/destruction

The generated ABI does not own the full lifecycle state machine, but it exposes the operations needed by handwritten ownership code:

- create constructors return runtime-owned refs;
- patch/mutation operations return replacement refs;
- path/edit transactions provide batched replacement/commit routes;
- `view_release_many` releases multiple refs;
- stale refs produce high-bit status/cache-miss results;
- runtime generation and handle validity are checked at session/bootstrap and wrapper levels.

Actual destruction, lease draining, semantic cache maintenance, weak-entry scavenging, and runtime teardown are implemented in `view_abi.rs` and retained transport code rather than generated code.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path matrix

| Semantic operation | Primary production route | Alternate route | Failure/selection semantics |
|---|---|---|---|
| Acquire ABI session | `nativeViewAbiSession` → N-API session | none observed in active framework path | Metadata mismatch throws; bootstrap `runtimeNoop` mismatch throws |
| Create/patch View | generated TypeScript call → N-API → generated Rust wrapper → handwritten implementation | feature-gated direct C ABI for qualification/bench use | invalid refs/enums/pointers/bounds return status; panics become ABI error values |
| Host render | generated `hostRenderRef` / edit commit → typed N-API host | feature-gated direct C ABI | host pointer/object and runtime validation; host mutation is declared in schema |
| Structural materialization | `retained-dag.ts` helpers using generated calls | no legacy generated transport found in active structural path | expected native statuses may be classified as refusal/cache miss; unexpected statuses propagate |
| State set patch | handwritten normalizer → generated envelope packer | none observed | public normalizer rejects invalid semantic values; generated packer encodes normalized values |
| State clear | handwritten normalizer → generated clear-mask packer | none observed | unknown property is rejected; clear mask generated from stable IDs |
| ABI conformance | generated TS/native methods | direct C conformance exports | probe-specific signatures and operation rules validated at generator load time |
| Generated freshness | `check:tui-abi` exact byte compare | CI Git diff checks | missing or byte-different generated artifact fails |

### 5.2 Validation and failure masking

Generated wrappers use explicit failure values rather than silently defaulting:

- null runtime/host pointer: invalid argument status;
- invalid native ref: invalid argument/ref status;
- invalid enum: invalid argument status;
- buffer capacity or used count overrun: buffer/count status;
- invalid C string pointer: invalid argument status;
- panics in handwritten implementations: generated panic error result;
- TypeScript reference result `0` or high-bit: `NativeAbiStatusError`;
- cache miss: `NativeAbiStatusError` plus `viewStatusDetail()` lookup.

The generated wrapper’s `catch_unwind` behavior differs by feature:

- default path catches panics;
- `fast-view-abi` uses a faster path without the normal unwind wrapper.

This is declared in generated code, but whether the feature is appropriate for a production runtime is a separate build/benchmark concern.

### 5.3 Alternate transport and compatibility observations

The active TypeScript generated ABI is N-API-oriented:

- `NativeViewAbiHandle` exposes methods, not function pointers;
- `transport: "napi"` is required at runtime;
- raw pointers are not exposed to JavaScript through the safe path.

The direct C ABI remains generated and feature-gated:

```rust
#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_<function>_v1(...)
```

This is an intentional qualification/compatibility path rather than the active TypeScript route. The repository’s ownership checker verifies that `direct-ffi` remains an explicit feature and that the generated N-API lowering exists.

### 5.4 Observed route-residue risks

The following are not necessarily bugs, but they are synchronization/maintenance hazards:

1. **Generated manifest omits `buffer_used_of`.**  
   `render_manifest.rs` includes `buffer_length_of` in each argument record but does not include `buffer_used_of`. The schema and validator do use `buffer_used_of` for multi-buffer functions, and generated Rust wrappers use it to pair counts correctly. Therefore:
   - generated implementation remains correct from TOML;
   - manifest does not fully describe all buffer-pairing metadata;
   - consumers using only the manifest cannot reconstruct the complete multi-buffer contract.

2. **Two ABI schema regimes exist under `include/`.**  
   `iyon_view_abi.h` is generated from the TOML schema; `iyon_content_abi.h` is handwritten. This is architecturally understandable because structural and content ABIs are separate, but it means “native ABI synchronization” is not one uniform process.

3. **Generated files are consumed at several inclusion styles.**
   - `include!` in handwritten Rust modules;
   - `#[path] mod` in Rust tests/modules;
   - TypeScript imports;
   - JSON import;
   - C header external consumers.
   
   A stale file can therefore compile in one route while another route is not exercised unless all checks run.

4. **Generated docs are part of freshness.**  
   The generated human reference is included in the exact output set. Documentation drift is therefore intentionally a generator freshness failure, not a separate documentation check.

### 5.5 Absence claims and search scope

Within the repository-wide searches over current Rust/TypeScript source:

- no active TypeScript structural implementation was found that bypasses `NativeViewAbiHandle` for View ABI operations;
- no active TypeScript import of raw `iyon_view_abi.h` was found;
- no alternate generated structural ABI directory was found outside the listed package path;
- no generated content ABI output from `tui-abi-gen` was found;
- `iyon_content_abi.h` remains a separate handwritten header and is not in `GENERATOR_OUTPUTS`.

These absence claims are limited to current tracked source searched under this repository and do not cover external consumers or historical branches.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Generator-time cost

Generator work is offline/build-time:

- reads one TOML schema and one JSON kind-code document;
- validates all declarations;
- computes two BLAKE3 hashes;
- renders 16 outputs;
- exact-check mode writes a temporary tree and compares all files byte-for-byte.

No runtime cache or scheduling behavior is owned by the generator itself.

### 6.2 Declared hotness and benchmark metadata

Every ABI function has schema-level:

- `hotness` such as `critical`, `warm`, `rare`, or `probe`;
- `benchmark_registration`;
- maximum input/buffer bounds;
- arity specializations.

`view_abi_table.rs` preserves these in Rust. `view_abi_cases.ts` preserves a TypeScript benchmark-friendly projection:

```text
name
family
hotness
benchmarkRegistration
scalarArgs
hasBuffer
maxBufferBytes
maxInputCount
```

This avoids manually maintaining benchmark case inventories separate from the ABI schema.

### 6.3 Runtime native caches

The generated code does not implement the runtime reference cache, but generated contracts constrain it:

- native refs are bounded `u32` handles;
- runtime generation is carried through metadata;
- `view_ref_for_node_id` provides exact lookup;
- `view_release_many` provides batched release;
- cache misses use high-bit statuses and optional diagnostic detail;
- TypeScript materialization uses `PATH_REFS`, `PATH_SHAPE_REFS`, and temporary lease buffers in handwritten transport code.

`view_abi.rs` comments describe bounded maintenance/scavenging for expired weak entries. That cache and maintenance behavior is handwritten and should not be attributed to codegen.

### 6.4 Per-operation/per-frame work

Codegen exposes the operation granularity but does not itself schedule frames. Important declared routes include:

- one-call specialized row/column constructors;
- bounded buffer constructors;
- fixed-depth path patch variants;
- batched edit transactions;
- batched release (`view_release_many`);
- generated `runtime_noop` probe for dispatch-granularity measurements.

The schema’s `max_input_count` and `max_buffer_bytes` are used in generated validation to prevent unbounded FFI work. They are contract bounds, not performance measurements.

### 6.5 State-envelope cost

State envelope generation fixes lane widths by schema:

- geometry: 14 words, no strings;
- presentation: 5 words, 14 strings.

The generated TS packers allocate fixed-size arrays per envelope. The native side reads fixed offsets from generated Rust tables. This avoids JSON property-name iteration across the FFI boundary after normalization, but the allocation/per-call behavior belongs to `state/control.ts` and the handwritten native state transport.

### 6.6 N/A boundaries

The following are N/A for codegen itself:

- frame scheduling;
- tick frequency;
- paint invalidation;
- terminal output buffering;
- content projection;
- layout caching;
- stream revision caching.

The generator declares call hotness and bounds but does not own those runtime mechanisms.

---

## 7. Tests, benchmarks and observability

### 7.1 Generator unit tests

`tools/tui-abi-gen/src/main.rs` contains generator tests covering:

- canonical schema renders all declared output paths;
- generated output paths are unique;
- canonical state document includes both geometry and presentation domains;
- gapped retained-state property IDs are rejected;
- unknown state value kinds are rejected;
- invalid state lane shapes are rejected;
- unknown lowerings are rejected;
- incompatible type/lowering combinations are rejected;
- missing `buffer_used` declarations are rejected;
- unpaired buffer lengths are rejected;
- invalid conformance signatures are rejected.

The canonical render test uses an Insta snapshot of the generated manifest/output rendering.

Historical L1 reports claim passing generator tests, but I did not execute them during this read-only inspection.

### 7.2 Generated Rust tests

`crates/iyon-tui-native/tests/generated_view_abi.rs` is generated and includes:

- generated stub implementations for all 60 schema functions;
- generated conformance call tests;
- function-count stability;
- ABI-version stability;
- wrapper delegation checks;
- invalid-input rejection checks;
- status/error behavior checks;
- buffer and reference validation tests.

It includes the generated Rust type, table, conformance, and exports files. The stubs return position-derived values so wrapper-to-implementation delegation can be checked without depending on the production native runtime.

### 7.3 Generated TypeScript tests

`packages/iyon-tui/tests/generated/view_abi_layout.test.ts` is generated and pins:

- schema hash;
- ABI version;
- function ordering;
- conformance ordering and signatures;
- qualified Bun version;
- result encoding;
- POD layouts;
- every function’s lowering list.

This test is intentionally schema-order-sensitive. Reordering functions or changing lowerings causes a generated test change and a test failure if outputs are not regenerated.

### 7.4 Snapshot coverage

The generator snapshot is an important architecture artifact because it verifies that the canonical schema renders the expected manifest shape. It is not a complete independent runtime ABI test; generated Rust/TypeScript tests and native runtime tests cover additional seams.

### 7.5 Benchmark metadata and observability

Generated benchmark registry data is consumed by benchmark tooling. The ABI function table also retains:

- family;
- hotness;
- allocation flag;
- host mutation flag;
- bounds;
- benchmark registration.

Runtime observability includes:

- native metadata fingerprints;
- runtime generation;
- function count;
- ABI conformance function addresses in `tui.rs`;
- `view_status_detail` for cache-miss diagnosis;
- `runtime_noop` bootstrap/dispatch probe;
- generated benchmark case registration.

### 7.6 Repository/CI checks

Current package scripts:

```json
"generate:tui-abi": "cargo run -q -p tui-abi-gen -- generate",
"check:tui-abi": "cargo run -q -p tui-abi-gen -- check"
```

CI runs:

- `bun run check:tui-abi`;
- `cargo test -p tui-abi-gen`;
- `git diff --exit-code` over generated Rust, include, generated TS, generated tests, generated benchmarks, and generated docs;
- TypeScript typecheck/lint;
- native staging/binding checks;
- ownership checks;
- package tests.

The CI path therefore protects both generator freshness and downstream compilation/behavior.

### 7.7 Ownership check coupling

`tools/ownership/check.ts` includes a `generated-napi-lowering` gate:

- generated `view_abi_napi.rs` must exist;
- generated manifest must exist;
- `direct-ffi = []` must remain present in the native crate feature declarations.

It also contains import-boundary checks preventing semantic API modules from importing generated ABI directly. This preserves the intended generic framework boundary and keeps generated ABI behind transport/native modules.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Strong synchronization seams

The most consequential cross-boundary synchronization mechanisms are:

1. **Shared schema source**
   - Rust, C, TypeScript, tests, benchmark metadata, docs, and state envelope tables are all rendered from the same TOML document.

2. **External enum source**
   - View kind numeric values come from `view-kind-codes.json`.
   - Rust/C generated enum values and TS numeric signatures are thereby linked to the same kind-code source.

3. **Embedded fingerprints**
   - Every generated output carries schema and generator fingerprints.
   - Native `view_abi.rs` metadata reads fingerprints from generated Rust constants.
   - TypeScript compares native metadata against generated manifest fingerprints.

4. **Function-count coupling**
   - Generated table declares `FUNCTION_COUNT`.
   - Native metadata exports it.
   - TypeScript compares it to `manifest.functions.length`.
   - Generated tests pin function ordering and count.

5. **Exact byte freshness**
   - `check` renders all outputs into a temporary tree and byte-compares every listed output.
   - CI additionally asserts no generated-file Git diff after checking.

6. **ABI-layout coupling**
   - POD schema fields and declared size/alignment are validated.
   - Rust emits compile-time assertions.
   - TS generated tests pin manifest size/alignment.
   - C header emits the corresponding C struct.

7. **State-lane coupling**
   - state rows produce TS packers and Rust masks/offsets.
   - generator validation checks dense IDs and exact lane shapes.
   - hand-written readers consume generated offsets and masks.

### 8.2 Ownership boundaries

The generic framework boundary is respected in the codegen scope:

- schema terms are generic View/layout/state/content-port/style/path operations;
- no agent/application/product semantics appear in `view_abi.toml`;
- generated outputs expose generic terminal framework mechanics;
- application meaning is not encoded into the ABI generator.

The presence of `view_content_host_create` is generic: the schema carries an opaque content-port identity and does not interpret application semantics.

### 8.3 Generated versus handwritten state semantics

The retained-state generator has an intentional split:

- generated:
  - property IDs;
  - mask constants;
  - null/clear masks;
  - word/string lane counts;
  - offsets;
  - low-level packer encodings;
  - envelope header checks.
- handwritten:
  - public patch normalization;
  - exact public validation error shape;
  - native property semantics;
  - node-kind and axis legality checks;
  - invalidation/wake behavior.

This is a useful ownership seam. It also means changing a state property’s semantic value encoding requires coordinated review of both schema/generator and handwritten normalizers/readers.

### 8.4 Contradiction/partial-description finding

The generated JSON manifest is intended as a machine-readable ABI description, but argument entries currently serialize:

```json
{
  "name": "...",
  "type": "...",
  "lowering": "...",
  "buffer_length_of": "..."
}
```

`buffer_used_of` is not serialized by `render_manifest.rs`, even though:

- `ArgumentSpec` models it;
- validation requires it on multi-buffer functions;
- `render_rust::validation_statements` uses it;
- the schema declares it where needed.

This is a documentation/manifest completeness gap, not necessarily a runtime correctness gap. The canonical generator input still has the information; the manifest consumer does not.

### 8.5 Historical documentation versus current source

Historical PERF/API-H2/S6 documentation references earlier package/crate names and earlier function counts. Current source uses:

- `crates/iyon-tui-native`;
- `packages/iyon-tui`;
- N-API `NativeViewAbiSession`;
- 60 structural functions in the current generated table;
- 10 conformance functions.

Historical documents are useful provenance but should not be used as current ABI authority. Current generated fingerprints, schema, manifest, and source imports are authoritative for this baseline.

### 8.6 Native include contradiction

The assignment scope includes `native include/`, but the directory contains:

- generated structural `iyon_view_abi.h`;
- handwritten content `iyon_content_abi.h`.

The generator’s output list includes only the former. Any future claim that “the native include ABI is generated” would be inaccurate unless explicitly limited to `iyon_view_abi.h`.

---

## 9. Open questions and coverage gaps

1. **Manifest completeness**
   - Should `buffer_used_of` be included in `view_abi_manifest.json` so external tooling can fully reconstruct multi-buffer pairing?
   - Current source proves the omission; product intent is not documented.

2. **Content ABI schema ownership**
   - `iyon_content_abi.h` is handwritten and outside `tui-abi-gen`.
   - It is not established whether a separate content schema/codegen system is intentionally deferred or permanently separate.

3. **External C consumers**
   - No active in-repository C consumer of `iyon_view_abi.h` was found.
   - External consumers cannot be assessed from this repository.

4. **Generated output completeness outside `GENERATOR_OUTPUTS`**
   - The 16 listed outputs are authoritative for this generator.
   - It is not established whether downstream packaging copies or transforms generated files outside these paths.

5. **Runtime behavior**
   - No tests/builds were executed in this assignment.
   - Runtime N-API metadata compatibility, direct-FFI qualification, and generated wrapper behavior are source-inferred here and historically reported as tested elsewhere.

6. **Generated line counts**
   - Approximate physical sizes were inferred from indexed line extents and symbol locations.
   - Exact `wc -l` counts were not executed.

7. **Feature policy**
   - `fast-view-abi` changes panic handling/performance behavior in generated exports.
   - The generator’s declaration and native feature policy are visible; the acceptance criteria for enabling this feature in deployment are outside this scope.

8. **Kind-code schema lifecycle**
   - The TOML references `view-kind-codes.json`, but the ownership/versioning policy for that JSON was not found in the scoped generator source.
   - Numeric enum stability therefore depends on a second tracked schema input whose change policy should remain explicit.

9. **Output consumers of generated reference docs**
   - The human reference is generated and freshness-checked, but no active runtime consumer exists.
   - It is a review/documentation artifact rather than an execution input.

---

## 10. Evidence appendix

### 10.1 Primary source paths inspected

#### Contract and context

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`

#### Schema/codegen source

- `tools/tui-abi/view_abi.toml`
- `tools/tui-abi-gen/Cargo.toml`
- `tools/tui-abi-gen/src/main.rs`
- `tools/tui-abi-gen/src/model.rs`
- `tools/tui-abi-gen/src/validate.rs`
- `tools/tui-abi-gen/src/render_rust.rs`
- `tools/tui-abi-gen/src/render_typescript.rs`
- `tools/tui-abi-gen/src/render_header.rs`
- `tools/tui-abi-gen/src/render_manifest.rs`
- `tools/tui-abi-gen/src/render_state.rs`
- `tools/tui-abi-gen/templates/generated_banner.txt`
- `tools/tui-abi-gen/templates/generated_typescript_bindings_header.txt`
- `tools/tui-abi-gen/templates/generated_typescript_calls_header.txt`
- `tools/tui-abi-gen/templates/generated_c_header_preamble.txt`
- `tools/tui-abi-gen/templates/generated_reference_header.txt`
- `tools/tui-abi-gen/src/snapshots/tui_abi_gen__tests__canonical_schema_renders_all_tranche_one_outputs.snap`
- `packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json`

#### Generated Rust

- `crates/iyon-tui-native/src/generated/view_abi_types.rs`
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs`
- `crates/iyon-tui-native/src/generated/view_abi_conformance.rs`
- `crates/iyon-tui-native/src/generated/view_abi_table.rs`
- `crates/iyon-tui-native/src/generated/view_abi_napi.rs`
- `crates/iyon-tui-native/src/generated/view_state_schema.rs`

#### Generated TypeScript

- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json`
- `packages/iyon-tui/src/transport/state/generated/state_envelope.ts`

#### Generated tests/bench/docs

- `crates/iyon-tui-native/tests/generated_view_abi.rs`
- `packages/iyon-tui/tests/generated/view_abi_layout.test.ts`
- `packages/iyon-tui/bench/generated/view_abi_cases.ts`
- `docs/history/perf/PERF-11-generated-abi-reference.md`

#### Native consumers and headers

- `crates/iyon-tui-native/include/iyon_view_abi.h`
- `crates/iyon-tui-native/include/iyon_content_abi.h`
- `crates/iyon-tui-native/src/tui.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `crates/iyon-tui-native/src/tui/view_state.rs`
- `crates/iyon-tui-native/Cargo.toml`
- `packages/iyon-tui/src/transport/native/addon.ts`
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
- `packages/iyon-tui/src/transport/state/control.ts`
- `tools/ownership/check.ts`
- `package.json`
- `.github/workflows/ci.yml`
- `.github/workflows/agent-fast.yml`
- `.github/workflows/tui-typescript.yml`
- `.github/workflows/api-surface.yml`
- `.github/workflows/t1-bun.yml`

### 10.2 Key symbol references

- `main.rs`
  - `DEFAULT_SCHEMA`
  - `KIND_CODES_SCHEMA`
  - `GENERATOR_OUTPUTS`
  - `render_outputs`
  - `write_outputs`
  - `check_outputs`
  - `PrintManifest`
  - `Explain`
- `model.rs`
  - `AbiDocument`
  - `AbiMetadata`
  - `HandleSpec`
  - `EnumSpec`
  - `PodSpec`
  - `FunctionSpec`
  - `ArgumentSpec`
  - `ConformanceSpec`
  - `StatePropertySpec`
  - `load`
  - `load_kind_codes`
- `validate.rs`
  - `validate`
  - `validate_state_properties`
  - `validate_conformance`
  - `validate_enum`
  - `validate_type`
  - `validate_lowering`
  - `validate_pod`
- `render_rust.rs`
  - `types`
  - `exports`
  - `napi_methods`
  - `conformance`
  - `table`
  - `layout_tests`
  - `validation_statements`
- `render_typescript.rs`
  - `abi_bindings`
  - `conformance_bindings`
  - `calls`
  - `benchmark_registry`
  - `layout_test`
- `render_state.rs`
  - `typescript_envelope`
  - `rust_schema`
  - `ts_set_packer`
  - `ts_clear_packer`
  - `rust_check_envelope`
- `render_manifest.rs`
  - `generator_hash`
  - `banner`
  - `manifest`
  - `human_reference`
- native runtime:
  - `NativeViewAbiSession`
  - `metadata`
  - `tui_view_abi_session`
- TypeScript runtime:
  - `nativeViewAbiSession`
  - metadata/fingerprint/function-count compatibility checks
  - `NativeAbiStatusError`
  - `checkedRef`

### 10.3 Commands/scripts identified but not executed

- `bun run generate:tui-abi`
- `bun run check:tui-abi`
- `cargo test -p tui-abi-gen`
- `cargo test --workspace --all-features`
- `bun run typecheck`
- `bun run check:ownership`
- `bun run check:tui-binding`
- CI `git diff --exit-code` generated-output checks

### 10.4 Files indexed but not exhaustively repeated

Generated files with many repetitive per-function records were indexed by headers, fingerprints, symbol names, line ranges, and consumer references:

- `view_abi_exports.rs`
- `generated_view_abi.rs`
- `view_abi_napi.rs`
- `view_abi_manifest.json`
- the generator snapshot

The repeated records are mechanically rendered from the schema; the report captures the generator rules and representative/current output structure rather than duplicating every generated signature.