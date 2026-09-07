# 31 — Structural Mutation

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: TypeScript and Rust structural update routes.
- Assignment goal: insert, remove, reorder, replace, reparent where supported; preparation/commit; rollback/failure; explicit unsupported-operation inventory.
- Investigation mode: read-only static source inspection. No repository files were edited, no dependencies were installed, and no build or test command was run in this investigation.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `AGENTS.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`, including the structural-model requirements and the later architecture-drift/fallback instructions.

The source is treated as authoritative for current behavior. Historical documents and comments are used only to explain intended contracts or implementation history.

### Scope boundaries

This report focuses on:

1. Immutable semantic `View` construction and structural derivations in TypeScript.
2. TypeScript retained materialization and native-reference ownership.
3. Generated structural ABI calls.
4. Rust retained `View` persistent sequences and native structural update functions.
5. Native runtime publication, leases, path operations, edit transactions, host installation, and failure handling.
6. Retained execution/composition commit and rollback where structural publication participates in the larger batch.

This report does **not** census content, state, layout, paint, terminal input, or the complete application runtime except where those systems are directly involved in structural publication or rollback.

### High-level result

The current implementation has two materially different categories of structural update:

1. **Production root publication path**  
   TypeScript composition produces a new immutable semantic `View`; `RetainedRootBoundary.prepareDesiredInstall()` materializes or incrementally derives a native root; the native host accepts the desired root; a later frame barrier makes it visible. This is the authoritative production structural path.

2. **Lower-level structural edit primitives**  
   The codebase also contains direct axis/grid update helpers and generated path-update ABI functions. They support axis replacement, axis insertion/removal, grid-cell replacement, and nested replacement in Rust/native code. However, the TypeScript direct rendering helpers are referenced only by tests/benchmarks in the inspected source tree, not by the main production composition/runtime path. The generated path ABI is physically present and exported by the native addon, but no TypeScript production caller was found for `viewAxisSetChildPath` or `viewGridSetCellPath`.

There is no first-class reparent operation. There is no dedicated reorder operation; reorder is expressible as an axis splice that removes an item and inserts it elsewhere. Grid row/cell insertion and deletion are not supported by the retained mutation APIs.

---

## 1. Responsibility and structure

### 1.1 Relevant module inventory

Approximate LOC are based on physical source line ranges observed during inspection, with production and tests separated where the file has an identifiable test module. These are estimates rather than compiler-generated statistics.

| Path | Language | Approx. production LOC | Approx. test LOC | Public surface | Primary responsibility | Structural relevance |
|---|---:|---:|---:|---|---|---|
| `packages/iyon-tui/src/api/view/view.ts` | TypeScript | ~1,030 | 0 | Partially public; transport helpers internal | Immutable semantic `View` authoring, node identity, axis/grid derivation helpers | Defines `axisSetChildForTransport`, `axisSpliceForTransport`, `gridSetCellForTransport`; creates fresh semantic node IDs |
| `packages/iyon-tui/src/api/view/semantic-node.ts` | TypeScript | ~620 | 0 | Internal semantic contract | Semantic node unions, derivation sidecars, persistent-sequence interfaces, attachment metadata | Defines `axisSet`, `axisSplice`, and `gridCell` derivations and wide-sequence overrides |
| `packages/iyon-tui/src/composition/persistent-seq.ts` | TypeScript | ~200 | 0 | Internal | Persistent immutable TypeScript sequence implementation | Supplies lazy structural sequence updates for wide axes/grids |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | TypeScript | ~2,140 | 0 | Internal transport boundary, selected helpers exported | Semantic-node-to-native-reference materialization, derivation fast paths, root lease protocol, prepare/commit/abort | Main production retained structural path |
| `packages/iyon-tui/src/transport/structural/native-view-abi.ts` | TypeScript | ~630 | 0 | Internal transport helpers | Generated ABI session, direct retained axis/grid helpers, path transactions, host installation | Contains direct structural edit routes; no production call sites found outside tests/benchmarks |
| `packages/iyon-tui/src/transport/structural/retained-path.ts` | TypeScript | ~300 | 0 | Internal transport helpers | Native path step representation, semantic text-layout path reconstruction, transaction lineage | Supports nested text-layout path updates; not generic structural reparenting |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts` | TypeScript generated | ~220 | 0 | Generated internal ABI wrapper | Checked wrappers for native structural functions | Exposes axis/grid/path/edit-transaction calls to transport code |
| `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts` | TypeScript generated | ~100 | 0 | Generated ABI type surface | Function-table/type declarations | Declares structural, path, and edit-transaction calls |
| `tools/tui-abi/view_abi.toml` | TOML schema | ~2,750 | 0 | Generator input | Canonical ABI schema | Classifies structural patches, path patches, and edit transactions |
| `crates/iyon-tui/src/presentation/ir.rs` | Rust | ~2,200 | ~400+ | Rust `View` public type, structural methods mostly hidden/internal | Backend-neutral immutable semantic IR and `PersistentSeq` | Implements Rust axis/grid replacement, splice, path replacement, and persistent structural storage |
| `crates/iyon-tui/src/presentation/mod.rs` | Rust | ~250 | 0 | Binding-only functions under `native-host` | Rust binding façade | Re-exports native axis/grid/path operations |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Rust | ~4,550 | ~1,700 | Native ABI implementation; generated exports | Environment-owned native cache, leases, ABI entrypoints, direct structural operations, path publication, edit transactions | Native structural mutation and prepare/commit implementation |
| `packages/iyon-tui/src/composition/execution.ts` | TypeScript | ~1,250 | 0 | Internal composition runtime | Retained execution scopes, child ownership, publication batching, commit/rollback | Decides when structural outputs are prepared and committed |
| `packages/iyon-tui/src/composition/publication.ts` | TypeScript | ~40 | 0 | Internal protocol types | `PreparedStructuralPublication` and `StructuralPublicationTarget` | Formal prepare/commit/abort seam |
| `packages/iyon-tui/src/runtime/runtime.ts` | TypeScript | ~850 | 0 | Runtime API | Canonical render, root publication target, frame draining | Connects composition output to the retained root boundary |
| `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts` | TypeScript test | 0 | ~900 | Test-only | Semantic derivation and persistent-sequence contracts | Verifies `axisSet`, `axisSplice`, `gridCell` |
| `packages/iyon-tui/tests/tui_h3_c_transport.test.ts` | TypeScript test | 0 | ~200 | Test-only | H3 retained transport behavior | Exercises wide structural replacement and host parity |
| `packages/iyon-tui/tests/tui_native_persistent_seq.test.ts` | TypeScript test | 0 | ~120 | Test-only | Direct native axis/grid retained edit helpers | Exercises replacement, insertion, removal, and grid-cell replacement |
| `packages/iyon-tui/tests/tui_native_transaction.test.ts` | TypeScript test | 0 | ~100 | Test-only | Native text-layout transaction | Verifies shared changed ancestors and host parity |
| `packages/iyon-tui/bench/perf12_t15_workload.ts` | TypeScript benchmark | ~120 | 0 | Benchmark-only | Wide axis/grid structural workload | Measures direct axis set/splice and grid-cell helper routes |

### 1.2 Primary and secondary responsibilities

#### TypeScript semantic authoring

`view.ts` owns immutable semantic construction:

- Each semantic node receives a fresh numeric `NodeId`.
- `axisSetChildForTransport` creates a new axis node whose sequence differs at one index.
- `axisSpliceForTransport` creates a new axis node whose sequence is `splice(index, removeCount, inserted...)`.
- `gridSetCellForTransport` creates a new grid node whose addressed cell points to a new child.
- Attachments from the base node are copied into the derived node.
- Derivation sidecars identify the base node and the narrow operation, allowing the transport layer to avoid rematerializing unchanged descendants.

This is semantic declaration, not in-place native mutation.

#### TypeScript retained transport

`retained-dag.ts` owns:

- Generation-scoped semantic-node-to-native-reference hints.
- Transaction-local native reference maps.
- Temporary lease tracking.
- Native materialization of new nodes.
- Incremental derivation calls for text layout, common scalar changes, axis replacement, axis splice, and grid-cell replacement.
- Stale-reference retry.
- Root publication preparation and root lease transfer.
- Desired-versus-visible root bookkeeping for the H3 frame barrier.

#### Rust semantic IR

`presentation/ir.rs` owns:

- `View` as an immutable `Arc<ViewNode>`.
- Persistent axis/grid sequences.
- Rust-side structural operations under `native-host`.
- Path traversal and replacement.
- Batch path replacement that rebuilds each changed parent once.

#### Native addon runtime

`crates/iyon-tui-native/src/tui/view_abi.rs` owns:

- Environment-local semantic cache.
- Native `ViewRef` slots and JS lease counts.
- `NodeId -> NativeRef` mappings.
- Path-store handles.
- Axis builders.
- Native edit transactions.
- Direct structural ABI implementations.
- Preparation and commit of staged structural publication.
- Host installation for host-mutating transactions.

---

## 2. Types, APIs and contracts

### 2.1 Semantic node and derivation types

`semantic-node.ts` defines the structural derivation union:

- `SemanticAxisSetDerivation` (`semantic-node.ts:486-493`)
  - `base`
  - `index`
  - optional replacement `track`
  - replacement `child`
- `SemanticAxisSpliceDerivation` (`semantic-node.ts:495-501`)
  - `base`
  - `index`
  - `removeCount`
  - inserted `(track, child)` entries
- `SemanticGridCellDerivation` (`semantic-node.ts:503-509`)
  - `base`
  - `row`
  - `column`
  - replacement `child`

The complete `SemanticDerivation` union is at `semantic-node.ts:511-516`.

Wide sequence metadata is represented by:

- `SemanticAxisSequenceOverride` (`semantic-node.ts:567-571`)
- `SemanticGridSequenceOverride` (`semantic-node.ts:573-579`)

These are weak sidecars, not mutable authoritative nodes. They point to immutable persistent sequences and preserve a link to the base semantic node.

The operation metadata used for axes is:

```text
SemanticAxisSequenceEdit =
    axisSet(index)
  | axisSplice(index, removeCount, insertedCount)
```

at `semantic-node.ts:563-565`.

### 2.2 TypeScript structural constructors

#### Axis replacement

`axisSetChildForTransport` (`view.ts:662-683`):

1. Requires the base to be a row or column.
2. Requires an integer index in range.
3. Converts the replacement child to its semantic node.
4. Preserves the current layout track if no replacement track is supplied.
5. Uses `PersistentSeq.set`.
6. Produces a new node with an `axisSet` derivation.

This is replacement, not insertion or removal.

#### Axis insertion/removal/replacement

`axisSpliceForTransport` (`view.ts:686-715`):

1. Requires row/column base.
2. Validates `index` in `[0, sequence.length]`.
3. Validates `removeCount >= 0` and does not exceed the suffix.
4. Converts inserted `View`s to semantic children.
5. Defaults omitted tracks to `{ kind: "normal" }`.
6. Uses `PersistentSeq.splice(index, removeCount, ...insertedLayout)`.
7. Produces an `axisSplice` derivation.

This single operation supports:

- insertion: `removeCount = 0`, inserted list nonempty;
- removal: inserted list empty, `removeCount > 0`;
- replacement: both removal and insertion;
- reorder: remove one or more existing logical entries and insert equivalent entries at another index, but there is no dedicated reorder API.

The implementation does not itself identify a move or preserve a separate move identity. It records only the resulting splice.

#### Grid-cell replacement

`gridSetCellForTransport` (`view.ts:718-775`):

- Requires a grid.
- Validates row.
- Resolves the logical cell coordinate to a sequence index.
- Replaces only the cell `view` while retaining span/alignment metadata.
- Produces a `gridCell` derivation.

For a wide grid, `buildWideGridNode` (`view.ts:891-933`) creates a new semantic node with a persistent flat cell sequence and preserves row offsets, row tracks, and cell-index metadata. For a narrow grid, it maps the ordinary row/cell arrays (`view.ts:749-775`).

There is no TypeScript API for adding/removing grid rows or cells.

### 2.3 Native retained references and leases

`retained-dag.ts` uses:

- `SemanticNativeHint` (`retained-dag.ts:52-56`) containing generation and native ref.
- `SEMANTIC_NATIVE` weak map (`retained-dag.ts:58-59`).
- `MaterializeTx` (`retained-dag.ts:213-340`) containing:
  - transaction-local semantic-node references;
  - recursion guard;
  - temporary leases;
  - borrowed hints;
  - stale retry count;
  - reusable axis/grid/byte buffers.

A hint is a borrowed acceleration result. It is not itself a lease. If a caller needs an owned reference, the native `viewRefForNodeId` operation acquires/promotes a counted lease.

`ensureSemanticNative` (`retained-dag.ts:1163-1219`) resolves identity in this order:

1. Same-generation weak hint.
2. Transaction-local ref.
3. NodeId promotion if `node.id <= nativeLookupCeiling`.
4. Derivation fast path.
5. Direct semantic materialization.
6. State attachment.
7. Hint installation and temporary lease recording.

The `nativeLookupCeiling` prevents newly-created nodes from causing unnecessary native lookup probes.

### 2.4 Rust persistent semantic representation

`presentation/ir.rs` describes `PersistentSeq` at lines `82-269`:

- branch factor is 32;
- leaves hold `Arc<[T]>`;
- branches hold `Arc` children and cumulative sizes;
- `set` copies the root-to-leaf path;
- `insert` copies affected persistent chunks;
- `splice` uses split/concat;
- unchanged chunks remain shared.

Rust `View` (`ir.rs:687-695`) is an immutable outer `Arc<ViewNode>`. `map_node` (`ir.rs:1012-1022`) shallow-clones the node, applies the requested update, recomputes flags, allocates a new `ViewId`, and wraps the result in a new `Arc`.

The native structural methods are hidden/internal and compiled under `feature = "native-host"`.

### 2.5 ABI contracts

The canonical ABI schema (`tools/tui-abi/view_abi.toml`) classifies:

- `view_axis_set_child` (`view_abi.toml:1213-1225`) as a structural patch, one child input, no host mutation.
- `view_axis_splice_buffer` (`view_abi.toml:1264-1276`) as a structural patch with up to 524,288 children and a 4 MiB buffer cap.
- `view_grid_set_cell` (`view_abi.toml:1326-1338`) as a structural patch.
- `view_axis_set_child_path` (`view_abi.toml:1377-1389`) as a structural path patch.
- `view_grid_set_cell_path` (`view_abi.toml:1906-1918`) as a structural path patch.
- `edit_txn_begin` (`view_abi.toml:2555-2567`).
- `edit_txn_add_text_layout` (`view_abi.toml:2586-2597`).
- `edit_txn_commit_render` (`view_abi.toml:2682-2694`) as the host-mutating transaction commit.
- `edit_txn_abort` (`view_abi.toml:2713-2725`).

The generated TypeScript wrappers use `checkedRef` (`view_calls.ts:22-28`): zero or any error-bit result becomes a `NativeAbiStatusError`; cache misses retrieve the native status-detail side channel.

Structural ABI construction functions return a `ViewRef`; they do not mutate the host. Host mutation is separate through `hostRenderRef` or `setDesiredViewRef`.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency graph

```text
TypeScript caller / component body
        │
        ▼
immutable View authoring (view.ts)
        │
        ├── semantic node + fresh NodeId
        ├── optional PersistentSeq override
        └── derivation sidecar:
              axisSet / axisSplice / gridCell / textLayout / commonScalar
        │
        ▼
RetainedRootBoundary / retained-dag.ts
        │
        ├── semantic hint / transaction-local ref
        ├── NodeId promotion if eligible
        ├── derivation fast path
        └── direct materializer for new nodes
        │
        ▼
generated TypeScript ABI calls
        │
        ▼
NativeViewRuntime
        │
        ├── NodeId → WeakView
        ├── NodeId → NativeRef
        ├── NativeRef slot / lease count
        ├── PathStore
        └── persistent Rust View
        │
        ▼
host desired-root installation
        │
        ▼
frame drain / visible-root promotion
```

### 3.2 Structural operation graph

```text
axisSetChildForTransport
        │
        ├── TS PersistentSeq.set
        ├── SemanticAxisSetDerivation
        └── retained-dag.tryDerivation
                └── viewAxisSetChild
                        └── Rust native_axis_set_child
                                └── PersistentSeq::set
                                        └── new published root

axisSpliceForTransport
        │
        ├── TS PersistentSeq.splice
        ├── SemanticAxisSpliceDerivation
        └── retained-dag.tryDerivation
                └── viewAxisSpliceBuffer
                        └── Rust native_axis_splice
                                └── PersistentSeq::splice
                                        └── new published root

gridSetCellForTransport
        │
        ├── TS PersistentSeq.set / narrow array mapping
        ├── SemanticGridCellDerivation
        └── retained-dag.tryDerivation
                └── viewGridSetCell
                        └── Rust native_grid_set_cell
                                └── PersistentSeq::set
                                        └── new published root
```

### 3.3 Ownership and lifetime

| Object | Created by | Owned by | Destroyed/released by |
|---|---|---|---|
| Semantic `View` node | TS or Rust semantic authoring | Immutable caller/composition scope | Garbage collection / `Arc` drop |
| TypeScript semantic hint | `installHint` | WeakMap keyed by semantic node | Weak-key collection or explicit clear |
| Native `ViewRef` | Native runtime publication | JS lease owner plus native slot | `viewReleaseMany`, slot scavenging |
| Native semantic cache entry | Native `publish` / `publish_bulk` | Environment `NativeViewRuntime` weak cache | Weak expiry/pruning or runtime disposal |
| Root lease | `RetainedRootBoundary` | Boundary’s previous/desired/visible role | Root transfer, supersession, close |
| Temporary materialization lease | `MaterializeTx` | Current prepare transaction | `releaseAll` or `releaseAllExcept` |
| PathRef | Native runtime `PathStore` | Environment runtime | Runtime teardown; no per-operation view ownership |
| Text edit transaction | Native runtime `edit_txns` | Runtime until commit/abort | `edit_txn_commit_render`, `edit_txn_abort`, runtime cleanup |
| TypeScript execution scope | `RetainedExecutionRuntime` | Root or parent child-owner | `dispose`, deferred removal finalization |

### 3.4 Important ownership invariant

The retained root boundary keeps the old root leased until the replacement has successfully been prepared and installed. In deferred H3 mode:

- `desiredRef` can advance before the frame is visible;
- `visibleRef` remains the previous frame until the host frame barrier reports success;
- superseded desired roots remain leased while the host may still acknowledge an earlier revision;
- old visible roots are released only after visibility promotion.

This is implemented by `RetainedRootBoundary` fields (`retained-dag.ts:1572-1585`) and `commitVisible` (`retained-dag.ts:1743-1789`).

---

## 4. Execution paths and state transitions

## 4.1 Canonical production root replacement

The production TypeScript path begins at `Tui.render`:

1. `runtime.ts:451-461` creates a producer that builds a `Scene` and returns its body.
2. The first render starts an `OwnedBuilderRoot` (`runtime.ts:463-481`).
3. Subsequent canonical renders call `OwnedBuilderRoot.replaceProducer` (`runtime.ts:487-496`).
4. `RetainedExecutionRuntime` evaluates the producer synchronously (`execution.ts:627-651`).
5. The output is stored in `scope.pendingOutput`.
6. `stagePublicationsRecursive` (`execution.ts:659-683`) invokes the scope’s `StructuralPublicationTarget.preparePublication`.
7. The root target calls `runtime.prepareRootPublication` (`runtime.ts:464-467`).
8. `prepareRootPublication`:
   - prepares semantic attachments (`runtime.ts:228-235`);
   - creates the next `Scene` sideband (`runtime.ts:236-238`);
   - ensures a deferred `RetainedRootBoundary` (`runtime.ts:240-242`, `runtime.ts:551-560`);
   - calls `prepareDesiredInstall`.
9. `prepareDesiredInstall` calls `prepareFrom` (`retained-dag.ts:1711-1735`).
10. `prepareFrom`:
    - resolves/materializes the new semantic root;
    - performs stale recovery;
    - acquires/promotes a root lease where required;
    - does not install the root into the host.
11. Composition commit invokes the prepared publication (`execution.ts:723-753`).
12. The root publication commits:
    - history binding;
    - `prepared.commit()`;
    - attachment desired binding;
    - host pending marker;
    - current scene update (`runtime.ts:257-277`).
13. `publishDesiredPrepared` (`retained-dag.ts:1923-1942`) calls native `setDesiredViewRef`, drains transaction temporaries, and transfers desired-root bookkeeping.
14. The environment later drains the frame.
15. `commitVisibleAfterDrain` (`runtime.ts:770-782`) calls `boundary.commitVisible(revision)` and promotes desired attachments.

The key distinction is that semantic composition and native structural construction happen during preparation, while host-visible frame state is promoted later.

## 4.2 Semantic derivation production path

When a new semantic node has a derivation sidecar, `tryDerivation` (`retained-dag.ts:1264-1366`) attempts:

- `textLayout` → `viewTextLayoutPatchRoot`;
- `commonScalar` → `viewCommonPatchRoot`;
- `axisSet` → `viewAxisSetChild`;
- `axisSplice` → `viewAxisSpliceBuffer`;
- `gridCell` → `viewGridSetCell`.

For axis replacement (`retained-dag.ts:1301-1314`):

1. Only the replacement child is resolved/materialized.
2. The previous axis is supplied as `baseRef`.
3. The new root NodeId is supplied.
4. The native operation returns a new reference.

For axis splice (`retained-dag.ts:1315-1335`):

1. A reusable `Uint32Array` contains only `(trackWord, childRef)` pairs.
2. Only inserted children cross the FFI boundary.
3. Existing native-retained children do not cross again.
4. `removeCount` determines removal.
5. Empty inserted list is therefore a removal operation.

For grid-cell replacement (`retained-dag.ts:1335-1348`):

1. Only the replacement child is materialized.
2. The row/column coordinate identifies the cell.
3. Native persistent sequence replacement produces a new root.

`tryDerivation` increments `derivation_fast_path_calls` on success. If a generated call returns an expected native status:

- a stale base or stale child may be retried once through `recoverStaleNode`;
- if recovery cannot succeed, the function returns `undefined`;
- `ensureSemanticNative` then falls back to direct semantic materialization of the derived node.

This fallback is **within the retained architecture**, not a separate legacy transport. The file explicitly states that retained refusal is not a route selector (`retained-dag.ts:185-22` and `retained-dag.ts:1350-1366`).

## 4.3 Direct TypeScript retained structural helper path

`native-view-abi.ts` contains direct render helpers:

- `tryRetainedAxisSetChildRender` (`native-view-abi.ts:317-350`);
- `tryRetainedAxisSpliceRender` (`native-view-abi.ts:357-400`);
- `tryRetainedGridSetCellRender` (`native-view-abi.ts:403-436`).

These helpers:

1. Validate operation indexes and the previous native ref.
2. Materialize replacement/inserted children via `tryRetainedMaterializeRef`.
3. Call the generated structural ABI function.
4. Install the returned root through `hostRenderRef` or `tuiViewAbiInstallRef`.
5. Release temporary child references.
6. Release the new root if host installation fails.
7. Return `undefined` for expected native failures.

The axis splice helper uses a POD `Uint32Array` containing only track/ref pairs (`native-view-abi.ts:352-355`, `357-400`). It explicitly preserves duplicate temporary references rather than using a `Set`, because the same child can appear multiple times.

The `previous: View` parameters in these helpers are not read by the implementations; actual base validation is performed from `previousRef` inside native code. This means the helper does not independently prove that the passed semantic `previous` corresponds to the native reference. The native ref remains the effective source of truth.

### Production reachability finding

An exhaustive repository search for `tryRetainedAxisSetChildRender`, `tryRetainedAxisSpliceRender`, and `tryRetainedGridSetCellRender` found:

- their definitions in `native-view-abi.ts`;
- calls in `tui_native_persistent_seq.test.ts`;
- calls in benchmark workload code.

No production runtime or control call site was found. Therefore these helpers are present and testable but are not the ordinary `Tui.render` update route.

## 4.4 Rust direct structural operations

`presentation/ir.rs` implements:

### `native_axis_set_child` (`ir.rs:1090-1141`)

- Row and column variants are handled separately.
- Bounds are checked by `PersistentSeq::get`.
- A zero track word preserves the existing track.
- A nonzero track word is decoded.
- The replacement child is inserted into a new `RowChild` or `ColumnChild`.
- `PersistentSeq::set` creates the replacement sequence.
- `map_node` creates a fresh semantic root.
- Other kinds return an explicit error.

### `native_axis_splice` (`ir.rs:1147-1198`)

- Decodes each inserted track before splice bounds validation.
- Builds final `RowChild` or `ColumnChild` values directly.
- Validates index/remove range.
- Calls `PersistentSeq::splice`.
- Creates a new parent root through `map_node`.
- Supports insertion, removal, and replacement.
- Does not construct a flattened full child vector for the existing sequence.

### `native_grid_set_cell` (`ir.rs:1204-1230`)

- Requires a grid.
- Resolves `(row, column)` through `cell_indices`.
- Clones the existing cell metadata.
- Replaces only `cell.view`.
- Uses `PersistentSeq::set`.
- Creates a new grid root.

## 4.5 Nested replacement paths

Rust defines `RetainedPathStep` (`ir.rs:1279-1303`) as:

```text
(kind, expected_view_kind, selector)
```

The path contains selectors and expected kinds, not retained `View` objects.

Supported path steps (`ir.rs:1323-1339`, also `retained-path.ts:48-58`) include:

- container child;
- clamp child;
- row-viewport child;
- column child;
- row child;
- grid cell;
- hanging prefix;
- hanging continuation;
- hanging body.

`native_replace_at_path` (`ir.rs:1235-1267`) recursively descends the path and then applies either:

- `native_axis_set_child`, or
- `native_grid_set_cell`.

The recursion rebuilds each changed ancestor and returns the changed path views. `try_replace_retained_children` (`ir.rs:1479-1682`) batches multiple replacements for one parent:

- validates path kinds first;
- stages all replacements;
- applies them in order;
- uses persistent `set` for axis/grid parents;
- creates only one rebuilt root for the parent.

The generated native functions `view_axis_set_child_path_impl` (`view_abi.rs:3263-3308`) and `view_grid_set_cell_path_impl` (`view_abi.rs:3728-3773`) call `publish_structural_path`.

### Important boundary distinction

The nested path ABI physically exists in the generated TypeScript ABI table and native addon, but `native-view-abi.ts` does not import or call `viewAxisSetChildPath` or `viewGridSetCellPath`. Repository-wide search found only their generated declarations/wrappers and native implementation references.

Thus:

- Rust/native path replacement is implemented.
- Generated TypeScript bindings exist.
- No TypeScript production structural path route was found.
- TypeScript does have nested **text-layout** path handling through `retained-path.ts` and the edit transaction helper.

## 4.6 Text-layout edit transaction

The native edit transaction is not a general structural transaction. It batches multiple text-layout changes:

- `EditTxn` stores:
  - `base_root_ref`;
  - strong `base_view`;
  - expected edit count;
  - staged text-byte count;
  - `TextLayoutEdit` entries (`view_abi.rs:332-338`).
- `EditTrieNode` forms a shared changed-path trie (`view_abi.rs:325-330`).
- `build_edit_trie` (`view_abi.rs:721-777`) coalesces common ancestors.
- `stage_edit_trie` (`view_abi.rs:779-814`) recursively rebuilds changed leaves and parents.
- `prepare_staged_publication` (`view_abi.rs:820-866`) validates and reserves native refs without publishing them.
- `commit_staged_publication` (`view_abi.rs:871-889`) installs reserved entries after host acceptance.

TypeScript `tryRetainedEditTransactionRender` (`native-view-abi.ts:443-510`) performs:

1. `editTxnBegin`.
2. For each text edit:
   - validates depth and base NodeId;
   - interns the path;
   - packs target/ancestor NodeIds;
   - calls `editTxnAddTextLayout`.
3. Calls `editTxnCommitRender`.
4. Aborts if an add or exception fails.

The native commit function (`view_abi.rs:2220-2265`) removes the transaction, builds and stages the edit trie, prepares references, calls `host.host.render(root)`, and only then commits the staged publication.

This transaction batches text-layout path edits only. It does not accept axis splice, grid insertion, reparent, or mixed structural operation records.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation matrix

| Semantic operation | TypeScript semantic representation | Native/Rust operation | Main production reachability | Alternate/test route | Unsupported aspects |
|---|---|---|---|---|---|
| Replace root | New semantic root `View` with fresh NodeId | `ensureSemanticNative` plus root publication | **Yes**; canonical `Tui.render`, slots, panes | Direct root install/adopt helpers | No in-place mutation; replacement is immutable root publication |
| Replace axis child | `axisSet` derivation | `viewAxisSetChild` → `native_axis_set_child` → `PersistentSeq::set` | **Yes** when a derived semantic root is published through the root boundary | `tryRetainedAxisSetChildRender` in test/bench helper | No public high-level mutation method |
| Insert axis child | `axisSplice` with `removeCount = 0` | `viewAxisSpliceBuffer` → `native_axis_splice` | Semantic operation is supported by retained derivation, but ordinary production callers generally construct a new axis in composition | Direct `tryRetainedAxisSpliceRender` | Axis only; no grid insertion |
| Remove axis child | `axisSplice` with empty inserted list | Same axis splice path | Same qualification as insertion | Direct native helper tests | Axis only; no grid row/cell removal |
| Replace axis range | `axisSplice` with removal and insertion | Same axis splice path | Supported by semantic transport derivation | Direct native helper | No dedicated replace-range ABI name |
| Reorder axis children | Expressible as splice remove + insert | Same axis splice path | No dedicated production reorder operation | Could be assembled through direct helper and semantic splice | No move/reorder identity or transaction-specific operation |
| Replace grid cell | `gridCell` derivation | `viewGridSetCell` → `native_grid_set_cell` → `PersistentSeq::set` | **Yes** through derived semantic root publication | Direct `tryRetainedGridSetCellRender` | Cell replacement only |
| Replace nested child at path | Rust `native_replace_at_path`; generated path ABI | `view_axis_set_child_path` / `view_grid_set_cell_path` | Native/Rust implemented; no TS production caller found | Rust/native tests and generated ABI tests | Only existing path targets; no path splice |
| Replace nested text layout | TS `NativePathLineage` + text derivation | `view_text_layout_patch_path` or edit transaction | Low-level retained path route implemented; transaction test reachable | `tui_native_transaction.test.ts` | Text layout only, depth limited in ABI transaction lanes |
| Reparent existing child | No derivation/API | No move/reparent native operation | **No** | Caller can reconstruct a new semantic tree | Keyed scope movement is execution identity reconciliation, not physical View reparenting |
| Insert/remove grid row/cell | No semantic derivation/API | No corresponding native operation | **No** | Rebuild entire grid via `View.grid` | Not supported as a retained splice |
| Insert/remove container/hanging child | No API | Path functions only replace existing child | **No** | Rebuild enclosing semantic node | Fixed-arity structures have no structural insertion |
| Mixed structural transaction | No transaction record type | Edit transaction stores text-layout edits only | **No** | Multiple independent semantic derivations or root rebuild | No atomic mixed axis/grid/text transaction |

### 5.2 Root preparation and commit

`RootPublication` (`retained-dag.ts:1521-1530`) exposes:

```text
rootRef
route?: "retained"
commit()
abort()
```

`prepareInstall` (`retained-dag.ts:1679-1703`) guarantees:

- materialization and lease acquisition happen before commit;
- old root remains installed and leased;
- commit performs publication/bookkeeping only;
- abort drains all preparation leases.

`prepareDesiredInstall` (`retained-dag.ts:1711-1735`) is the deferred H3 form:

- preparation materializes and validates the desired root;
- commit calls `publishDesiredPrepared`;
- no host paint is performed by the boundary commit;
- `commitVisible` later promotes the desired root after the host frame succeeds.

### 5.3 TypeScript rollback and failure

Preparation failure in `prepareFrom` (`retained-dag.ts:1801-1917`):

- drains `MaterializeTx` temporary leases;
- returns `undefined` for retained refusal/cycle/stale failure;
- leaves the old installed root unchanged;
- does not invoke host publication.

`unwindPrepared` (`retained-dag.ts:2009-2019`) drains temporary leases and releases any newly acquired boundary lease that was not transferred.

At composition level:

- `stagePublicationsRecursive` treats `undefined` as `TUI_EXECUTION_PREPARE_REFUSED` (`execution.ts:673-680`);
- batch preparation failure calls `unwindStaged` and `abortBatch` (`execution.ts:497-519);
- pending outputs, pending props, semantic slot tables, child-owner WIP, and staged publications are rolled back;
- fresh never-committed execution scopes are disposed after rollback (`execution.ts:801-804`);
- deferred removed scopes are collected during promotion and finalized only after the entire commit succeeds (`execution.ts:1060-1105` and `execution.ts:522-537`).

`OwnedBuilderRoot.replaceProducer` is explicitly transactional (`execution.ts:1187-1234`): it optimistically swaps the producer and restores the prior producer if evaluation or preparation fails.

### 5.4 Native direct helper failure

The direct TypeScript axis/grid helpers perform an operation-level transaction manually:

- child materialization failure returns `undefined`;
- structural ABI failure releases any acquired new-root and child leases;
- host installation failure releases the new root and retains the previous root;
- successful install leaves the returned new-root lease to the caller;
- child temporary leases are released after the operation.

These helpers do not return a `RootPublication` object and are therefore less integrated with the composition-wide prepare-all/commit-once protocol. Their callers must perform root-lease transfer themselves.

### 5.5 Native ABI status categories

`view_abi.rs:52-55` defines:

```text
FAST_INVALID    = 0x8000_0001
FAST_CACHE_MISS = 0x8000_0004
FAST_REFUSED    = 0x8000_0005
FAST_INTERNAL   = 0x8000_0006
```

Structural functions distinguish:

- malformed input or wrong kind → `FAST_INVALID`;
- missing/stale base or child ref → `FAST_CACHE_MISS`;
- capacity/resource exhaustion → `FAST_REFUSED`;
- host/runtime failure → `FAST_INTERNAL` where applicable.

For axis splice, stale child details carry the child ordinal through the status-detail side channel. TypeScript uses this to retry only the specific stale child (`retained-dag.ts:364-373`, `450-464`).

### 5.6 Native edit transaction rollback

`edit_txn_begin` validates:

- valid base `ViewRef`;
- nonzero expected edit count;
- count no greater than `MAX_EDIT_COUNT` (256);
- available edit transaction handle.

`edit_txn_add_text_layout` validates:

- edit transaction handle;
- depth no greater than four in the generated transaction lane;
- path existence and matching depth;
- maximum edit count;
- staged-object bound;
- duplicate `(path_ref, path_depth)` rejection.

`edit_txn_abort_impl` (`view_abi.rs:2268-2283`) removes the transaction and drops its strong staged `base_view`. Repeated abort reports a nonzero status but does not leave transaction state.

If native host rendering fails during `edit_txn_commit_render`:

- the edit transaction has already been removed;
- the local trie and staged `View`s drop;
- no staged publication is committed;
- the existing host root remains the host’s prior root;
- the result is recorded as `FAST_INTERNAL`.

The code comments state that staged references are reserved locally and only installed after host acceptance (`view_abi.rs:816-819`). This is the native atomicity boundary.

### 5.7 Fallback classification

The following are **recovery** rather than alternate architecture:

- stale semantic hint deletion and one NodeId retry;
- stale child/base retry in `tryDerivation`;
- direct semantic materialization after a derivation call fails;
- native weak-cache expiry and subsequent NodeId promotion.

The retained code explicitly rejects route selection after retained refusal. There is no fallback from retained materialization to an older complete-object transport in the inspected production path.

The direct TypeScript structural helpers returning `undefined` on expected status could be used by a caller to choose another route, but no production call site was found. Therefore the source tree does not currently prove a production silent fallback for these helpers.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 TypeScript semantic cache

`SEMANTIC_NATIVE` is a weak map keyed by semantic node identity:

- key lifetime follows the immutable semantic node;
- value includes native ABI generation;
- stale generations do not hit;
- hints do not carry leases;
- `clearNativeHint` explicitly removes a confirmed stale hint.

`STYLE_REF_CACHE` is separately generation/runtime scoped, but it is payload styling rather than structural topology.

### 6.2 Native semantic cache

`NativeViewRuntime` (`view_abi.rs:357-376`) holds:

- `nodes: HashMap<u64, WeakView>`;
- `slots: NativeRefTable`;
- `node_refs: HashMap<u64, u32>`;
- `path_nodes`;
- `path_keys`;
- `builders`;
- `edit_txns`.

`publish_semantic_view` distinguishes:

- leased publication for a caller-owned `ViewRef`;
- weak-only publication for intermediate path objects.

`resolve_ref` upgrades a weak view when possible. If the weak view expired:

- its `node_refs` entry is removed;
- the native slot is removed;
- expiration counters increment;
- a cache miss is returned.

### 6.3 Root invalidation

Structural updates do not mutate an existing semantic node. Every derived semantic node receives a new NodeId, so the native cache key changes for each new structural value.

The root boundary captures `viewNodeIdHighWater()` after successful desired/visible transfers (`retained-dag.ts:1632`, `2097`, `2116`). This is used to distinguish:

- old NodeIds that may already have native entries and are eligible for promotion;
- genuinely new NodeIds that should be materialized directly without a probe.

### 6.4 Persistent sequence cost

Rust `PersistentSeq`:

- `set` cost is root-to-leaf path copying;
- splice uses split, insertion-sequence construction, concat;
- unchanged chunks remain shared;
- production counters include `PersistentSeqNodesAllocated`, `PersistentSeqBranchClones`, and `PersistentSeqLeafClones`.

TypeScript wide sequence handling:

- axis wide threshold: 1,024 children (`view.ts:149-150`);
- grid wide threshold: 1,024 total cells;
- wide semantic nodes use persistent sequence overrides;
- the `children`/`rows` getters lazily flatten the sequence only if callers request the ordinary array-shaped representation.

The retained materializer itself avoids flattening wide axis data by reading the override sequence directly (`retained-dag.ts:506-565`).

### 6.5 Buffer lanes

The direct native ABI helpers use:

- reusable per-depth axis scratch in `retained-dag.ts:68-74`;
- reusable grid/diff word scratch (`retained-dag.ts:76-83`);
- reusable byte scratch (`retained-dag.ts:85-89`);
- native direct axis splice buffer containing `(track_word, child_ref)` pairs.

The standalone `native-view-abi.ts` direct splice helper still allocates a `Uint32Array` per invocation (`native-view-abi.ts:371`), whereas the root retained materializer uses transaction/environment scratch. This is another reason not to equate the direct test helper with the canonical production materializer.

### 6.6 Scheduling

Structural root changes are scheduled through retained execution and the host frame barrier:

```text
accepted state/producer mutation
    → execution dirty queue
    → synchronous evaluation
    → structural preparation
    → composition commit
    → desired-root installation
    → host frame drain
    → visible-root promotion
```

`runtime.ts:159-161` states that tracked-state writes drain on the next microtask and explicit `render()` drains pending work first.

No structural operation is applied in place during a frame. Public mutation during a retained protocol pass is rejected by `runtime.ts:439-447`.

---

## 7. Tests, benchmarks and observability

No tests were run for this report. The following is source evidence of existing tests and benchmark contracts.

### 7.1 TypeScript semantic tests

`tui_h3_a_semantic.test.ts` verifies:

- semantic derivation shape and identity:
  - `axisSet` at lines ~814 and ~844-850;
  - `axisSplice` at lines ~815 and ~851-857;
  - `gridCell` at lines ~817 and ~858-864;
- all derivation kinds are represented (`~805-807`);
- derivation sidecars do not expose transport fields such as `trackWord` or schema objects (`~799-804`);
- `PersistentSeq` satisfies the read-only semantic sequence contract without flattening at the boundary (`~867-888`).

These tests establish that semantic structural updates are immutable derivations, not mutable arrays.

### 7.2 TypeScript transport tests

`tui_native_persistent_seq.test.ts`:

- describes native wide-edit primitives at lines ~32-35;
- tests axis replacement, insertion, and removal (`~37-67`);
- tests grid-cell replacement while preserving placement (`~69-100`).

`tui_h3_c_transport.test.ts`:

- constructs a 2,000-child axis;
- applies `axisSetChildForTransport`;
- checks retained behavior and counters around the wide path (`~35-43` onward).

`tui_native_transaction.test.ts`:

- has one `PERF-11.6 native edit transactions` suite;
- checks two typed text-layout path edits;
- verifies shared changed-root behavior and host parity.

### 7.3 Rust/native tests

`crates/iyon-tui-native/src/tui/view_abi.rs` contains tests including:

- `t12_stale_child_status_detail_precedes_parent_publication` (`~4740`);
- `native_axis_builders_and_small_constructors_publish_immutable_views` (`~4866`);
- `failed_transaction_releases_every_new_temp_lease` (`~5014`);
- `stale_unleased_weak_slot_returns_cache_miss` (`~5028`);
- `generated_text_and_common_patches_publish_new_node_ids` (`~5112`);
- `path_refs_are_interned_and_depth_specialization_rebuilds_only_the_path` (`~5131`);
- `stale_path_base_returns_cache_miss_then_recovers_once` (`~5163`);
- `edit_transaction_builds_one_shared_ancestor_for_two_text_edits` (`~5194`);
- `edit_transaction_abort_and_limits_leave_no_staged_state` (`~5237`);
- `generated_axis_and_grid_edits_copy_persistent_sequences` (`~5269`);
- axis buffer count validation (`~5354`);
- grid buffer/cache behavior (`~5382`);
- malformed grid tail publishing nothing (`~5469`);
- semantic-cache-first axis/grid edits (`~5709`);
- text-layout cache-first behavior (`~5798`);
- common scalar patch behavior (`~5843`);
- path validation preserving publication (`~6155`).

These tests cover the native structural operations, but they do not establish that every generated route is used by the ordinary TypeScript production runtime.

### 7.4 Counters and instrumentation

`RetainedIdentityCounters` (`retained-dag.ts:111-126`) exposes:

- hint hits/misses;
- NodeId-promotion attempts/hits/misses;
- semantic nodes inspected;
- children visited;
- direct materializer calls;
- derivation fast-path calls;
- ref words written;
- payload bytes;
- scratch reuse;
- stale retries;
- normalized decorated nodes;
- host mutations.

`RetainedPhaseInstrumentation` (`retained-dag.ts:157-182`) records:

- transport preparation time;
- native materialization time;
- host commit time.

The native runtime diagnostics (`view_abi.rs:1492-1499`, `1651-1659`) expose counts for:

- semantic cache entries;
- NodeId refs;
- path nodes and keys;
- builders;
- edit transactions;
- stale removals;
- release batches;
- expired slots.

The generated structural functions themselves do not all carry a route discriminator. A test can infer the route from counters or native diagnostics, but there is no universal production route label that says “semantic direct materialization” versus “axis derivation” for every operation.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Production composition and direct structural helpers are not the same route

The source comments sometimes describe direct helpers as retained routes for transient boundaries, but repository-wide call-site inspection found those helpers only in tests/benchmarks. The canonical production root path uses:

```text
semantic View output
    → RetainedRootBoundary.prepareDesiredInstall
    → retained-dag.ensureSemanticNative
    → derivation/direct materializer
    → desired-root publication
    → frame barrier
```

The direct helper path:

```text
previous native ref
    → tryRetained*Render
    → generated structural ABI
    → immediate host install
```

is physically implemented but not shown to be production-reachable from `Tui.render`, component projections, `ViewSlot`, or `ScrollPane`.

### 8.2 Generated path ABI is more complete than TypeScript transport integration

The schema, generated wrappers, and native implementation include:

- `view_axis_set_child_path`;
- `view_grid_set_cell_path`;
- path root/child internment;
- fixed-depth path specializations.

The TypeScript transport imports and uses the non-path structural calls, but no source call site imports `viewAxisSetChildPath` or `viewGridSetCellPath`. This is a real cross-boundary asymmetry:

- native/Rust supports nested structural replacement;
- TypeScript production transport does not currently invoke those generated path functions;
- nested TypeScript text-layout updates use separate path/transaction machinery.

### 8.3 Structural ABI patch calls do not mutate host state

The schema correctly marks `view_axis_set_child`, `view_axis_splice_buffer`, and `view_grid_set_cell` as `mutates_host_state = false`. They create/publish a new semantic native reference. Host installation is separate.

By contrast:

- `host_render_ref` mutates the host immediately;
- `edit_txn_commit_render` combines staged publication with host rendering;
- deferred H3 root publication uses `setDesiredViewRef` first and visible promotion later.

This means “structural commit” has different meanings in different routes:

- root boundary commit = desired-root installation/bookkeeping;
- direct helper completion = immediate host install;
- edit transaction commit = native staging + host render + publication commit;
- composition commit = promotion of already-prepared `RootPublication`s.

### 8.4 Rust persistent sequence is authoritative for native structural edits

Rust `native_axis_set_child`, `native_axis_splice`, and `native_grid_set_cell` directly use persistent sequences. The TypeScript wide sequence sidecar is a parallel semantic representation used to avoid flattening before FFI, but the native runtime reconstructs equivalent persistent Rust sequence updates.

The TS and Rust sequence implementations therefore share the same conceptual contract but are not the same object or cache.

### 8.5 Reparenting is not represented as a structural operation

The repository has two related but distinct concepts:

1. **View topology**: immutable parent-child semantic nodes.
2. **Execution identity topology**: retained component scopes reconciled by `View.key`.

`view.ts:251-259` says keyed invocations let moved instances keep execution scopes without re-execution. That is execution-scope reconciliation, not a native View occurrence move. There is no operation that says:

```text
detach existing child occurrence from parent A
attach same retained occurrence under parent B
```

A caller can create a new semantic root that shares an immutable child `View`, but this is reconstruction, not an atomic reparent transaction.

### 8.6 Commit-phase failure is intentionally treated as pathological

Composition comments (`execution.ts:522-537`) state that commit is infallible after preparation. The code propagates a commit-phase throw instead of entering normal rollback. This is consistent with the prepare-all/commit-once contract, but it leaves a smaller, explicitly pathological failure domain:

- ordinary validation/materialization/lease failures occur during prepare and roll back;
- teardown or unexpected commit failures are propagated and are not treated as recoverable frame aborts.

`runtime.ts:257-277` similarly assumes `prepared.commit()` does not fail after preparation. If it does, attachment rollback occurs, but previously executed sideband steps such as history binding are not a fully general compensating transaction.

### 8.7 No evidence of a legacy complete-object fallback in current production structural publication

The retained DAG module explicitly says no secondary complete-object decoding path exists in production (`retained-dag.ts:19-22`). The source searches for structural materialization and fallback terms found:

- direct semantic materializers;
- derivation recovery;
- stale NodeId promotion;
- native cache miss handling;
- no legacy structural transport invoked by `RetainedRootBoundary`.

This report therefore finds no production fallback that silently reconstructs the entire object through an older transport after retained refusal. The direct helper `undefined` returns remain a potential caller-level route-selection point, but they were not found in production call sites.

---

## 9. Open questions and coverage gaps

1. **Why are the direct TypeScript retained axis/grid render helpers not used by production runtime code?**  
   They are implemented and benchmarked, but no production caller was found. It is unclear whether they are retained as future integration points, benchmark-only primitives, or migration residue.

2. **Are generated `viewAxisSetChildPath` and `viewGridSetCellPath` intended for future TypeScript use?**  
   Native implementations and generated wrappers exist, but no TypeScript transport caller was found.

3. **Does any external consumer call internal structural helpers?**  
   The package root does not export them, and they are marked `@internal`; this report can only establish repository-local reachability.

4. **Should reorder be treated as a first-class semantic operation?**  
   Current code can express it as `axisSplice`, but there is no operation-specific identity or route counter for a move.

5. **Should reparent preserve a native occurrence or execution identity?**  
   No current structural API answers this. Keyed scope movement preserves execution identity in the composition layer but does not define native semantic occurrence ownership.

6. **Are mixed structural/text-layout transactions required?**  
   The existing native edit transaction stores only text-layout edits. Axis/grid structural edits are separate derivations and are not atomically combined with text-layout edits in one native transaction.

7. **What should happen if root publication commit fails after history binding?**  
   The normal contract treats this as impossible/pathological. The code aborts attachment preparation, but there is no general rollback of every preceding sideband mutation.

8. **Are direct structural helpers expected to validate `previous: View` against `previousRef`?**  
   The parameter is unused in the helper bodies. Native ref resolution validates the ref, but the TS semantic object/ref correspondence is not independently checked there.

9. **How should wide-grid row metadata change under insertion/removal if such operations are added?**  
   Current wide-grid metadata includes row offsets, row tracks, and coordinate-to-flat-index maps. There is no current incremental row/cell insertion contract.

10. **No executed validation was performed in this assignment.**  
    Existing test names and assertions were inspected statically; pass/fail status for this worktree was not independently established.

---

## 10. Evidence appendix

### 10.1 Inspected production files

#### TypeScript

- `packages/iyon-tui/src/api/view/view.ts`
  - `View` static constructors;
  - `updateSemanticViewNode`;
  - `axisSetChildForTransport`;
  - `axisSpliceForTransport`;
  - `gridSetCellForTransport`;
  - wide axis/grid builders;
  - semantic sequence helpers.
- `packages/iyon-tui/src/api/view/semantic-node.ts`
  - `SemanticDerivation`;
  - `SemanticAxisSetDerivation`;
  - `SemanticAxisSpliceDerivation`;
  - `SemanticGridCellDerivation`;
  - `SemanticAxisSequenceOverride`;
  - `SemanticGridSequenceOverride`;
  - derivation/sequence sidecars.
- `packages/iyon-tui/src/composition/persistent-seq.ts`
  - TypeScript persistent sequence implementation.
- `packages/iyon-tui/src/composition/publication.ts`
  - `PreparedStructuralPublication`;
  - `StructuralPublicationTarget`.
- `packages/iyon-tui/src/composition/execution.ts`
  - `RetainedExecutionScope`;
  - `RetainedExecutionRuntime`;
  - publication staging;
  - composition commit and rollback;
  - `OwnedBuilderRoot`.
- `packages/iyon-tui/src/runtime/runtime.ts`
  - `prepareRootPublication`;
  - canonical render;
  - `RetainedRootBoundary` setup;
  - visible-root callback.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `MaterializeTx`;
  - `ensureSemanticNative`;
  - materializers;
  - `tryDerivation`;
  - `renderExactRoot`;
  - `RetainedRootBoundary`;
  - root lease transfer;
  - desired/visible commit.
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts`
  - `NativeViewAbiSession`;
  - direct axis/grid retained render helpers;
  - text-layout edit transaction;
  - path interning;
  - host/ref installation.
- `packages/iyon-tui/src/transport/structural/retained-path.ts`
  - native path types/constants;
  - text-layout path semantic reconstruction;
  - path lineage and transaction metadata.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts`
  - `checkedRef`;
  - structural/path/edit-transaction wrappers.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts`
  - generated ABI function/type declarations.

#### Rust

- `crates/iyon-tui/src/presentation/ir.rs`
  - `PersistentSeq`;
  - `View`;
  - `map_node`;
  - `native_axis_from_children`;
  - `native_axis_set_child`;
  - `native_axis_splice`;
  - `native_grid_set_cell`;
  - `native_replace_at_path`;
  - `try_replace_retained_child`;
  - `try_replace_retained_children`;
  - `RetainedPathStep`;
  - `try_retained_child`.
- `crates/iyon-tui/src/presentation/mod.rs`
  - binding-only `view_native_axis_splice`;
  - `view_native_grid_set_cell`;
  - `view_native_replace_at_path`;
  - binding re-exports.
- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - native runtime/cache/lease types;
  - `PathNode`;
  - `EditTxn`;
  - `EditTrieNode`;
  - `build_edit_trie`;
  - `stage_edit_trie`;
  - `prepare_staged_publication`;
  - `commit_staged_publication`;
  - `publish_structural_path`;
  - direct axis/grid ABI functions;
  - path ABI functions;
  - edit transaction ABI functions;
  - native tests.

#### Schema/generated support

- `tools/tui-abi/view_abi.toml`
  - structural patch declarations;
  - path patch declarations;
  - edit transaction declarations.
- Generated TS/Rust ABI files referenced by the above modules.

### 10.2 Inspected tests and benchmarks

- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_h3_c_transport.test.ts`
- `packages/iyon-tui/tests/tui_native_persistent_seq.test.ts`
- `packages/iyon-tui/tests/tui_native_transaction.test.ts`
- `packages/iyon-tui/bench/perf12_t15_workload.ts`
- Native tests embedded in `crates/iyon-tui-native/src/tui/view_abi.rs`

### 10.3 Repository-local searches performed

Searches covered the repository for:

- `axisSetChildForTransport`
- `axisSpliceForTransport`
- `gridSetCellForTransport`
- `tryRetainedAxisSetChildRender`
- `tryRetainedAxisSpliceRender`
- `tryRetainedGridSetCellRender`
- `viewAxisSetChildPath`
- `viewGridSetCellPath`
- `native_axis_set_child`
- `native_axis_splice`
- `native_grid_set_cell`
- `native_replace_at_path`
- `edit_txn_begin`
- `edit_txn_commit_render`
- `edit_txn_abort`
- `prepareInstall`
- `prepareDesiredInstall`
- `commitVisible`
- `rollback`
- `reparent`
- `reorder`
- `fallback`
- `legacy`

The absence claims in this report are limited to these repository-wide source searches and do not exclude external consumers.

### 10.4 Static versus executed evidence

- Static source inspection: completed for the files and symbols listed above.
- Test execution: not performed.
- Benchmark execution: not performed.
- Build/type-check execution: not performed.
- Runtime route counters: inspected as source definitions only; no runtime values were collected.