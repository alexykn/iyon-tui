# PRE-V5-L1 — Internal Rust Runtime and Direct Three-Plane Lowering

**Repository:** `alexykn/iyon-tui`  
**Audited branch:** `main`  
**Audited commit:** `90f19f5c9057ebb41fd1f8ffc73adbab05656cbc`  
**Commit timestamp:** 2026-09-04 20:15:58 UTC  
**Commit subject:** `fix(lint): simplify combinators, asserts, and iterator collection`  
**Scope:** remove the supported Rust UI-authoring surface; lower directly into the existing retained runtime; remove avoidable copying and repeated work in all three planes; preserve the current TypeScript API and current terminal behavior.  
**Not this project:** React migration, Taffy integration, GPUI, replacement of current History semantics, or a second renderer.  
**Verification status:** source-audited design, not an implemented or benchmark-validated change.

---

## Read this first

The recommended change is **not** to replace `View` with another recursively owned graph, route everything through the existing `edit_txn_*` functions, or move the current facade wholesale into a module called `internal`.

The recommended change is:

1. Keep the canonical retained data, identities, leases, current layout algorithms, terminal backend, and transaction discipline that already work.
2. Remove the public Rust authoring contract and the construction DSL from production call paths. Construct the final retained node or property record directly, once.
3. Separate content acceptance, semantic transformation, delivery, width-dependent layout, style resolution, and visible-window painting into independently reusable products. They remain stages of the existing content architecture, not new transports or new application planes.
4. Replace whole-registry snapshots, repeated builder roots, whole-prefix copies, and whole-content repaint intermediates with changed-record preparation, immutable shared storage, and explicit borrowed views.
5. Make the eventual v5 replacement points small and named. Do not implement their future semantics prematurely.

A literal ban on every `Clone` would be counterproductive. Some current clones retain an immutable `Arc`; others copy entire strings, maps, or surfaces. This handoff distinguishes them and budgets ownership operations explicitly. **Zero avoidable deep copies is a requirement; unsafe lifetime shortcuts are not an optimization.**

The attached bridge map is useful as a list of suspicions, but several of its architectural prescriptions do not match current `main`. Section 3 corrects them before any implementation starts.

### How the implementation agent should execute this document

Read §§1–5 before changing code. Implement the tranches in §17 in dependency order. Each tranche has concrete tasks, required deletions, and stop gates. The detailed algorithms in §§6–15 are part of those tasks, not optional background reading. Use §18 as the behavior ledger and §19 as the verification procedure. Record evidence using §20.

Do not combine the public-surface removal, storage redesign, parser behavior changes, and frame-commit rewrite in one unreviewable patch. Do not preserve a previous-generation production path to make a failing tranche pass.

---

## 1. Authority, baseline, and what has actually been verified

### 1.1 Authority order

Use this order when interpreting the inputs:

1. The current request: no supported Rust authoring API, current functionality preserved, current custom terminal pipeline retained, v5 adaptability, performance and maintainability.
2. PERF-13's resolved invariants: plane ownership, desired versus visible state, resource ownership, mutation ordering, atomic frame visibility, and mandatory content data FFI.
3. The audited implementation at the pinned SHA for what current behavior and current call paths actually are.
4. The completed post-PERF13 cleanup report for the single-production-path/deletion discipline.
5. The v5 document for destination constraints and replacement seams, **not** permission to change current semantics in this project.
6. `BRIDGE-LOWERING-MAP.md` as planning input, corrected by this audit.

If code and a normative invariant disagree, add a reproducing test and identify the disagreement. Do not quietly call the current behavior correct, and do not hide a behavior change inside an optimization. A correctness repair gets an explicit, independently reviewable change and evidence.

### 1.2 Source pinning

Every repository reference in this document is pinned to the commit above. Symbol names are the durable navigation aid; line ranges describe that commit only. Before implementation:

```sh
git fetch origin main
git rev-parse origin/main
git show --no-patch --format=fuller 90f19f5c9057ebb41fd1f8ffc73adbab05656cbc
```

If `main` has advanced, inspect its diff against the audited SHA and update the affected audit entries. Do not mechanically apply stale line numbers. Do not reset or discard another person's changes.

### 1.3 Verification limits

Repository source was read through the GitHub connector, including the native ingress, retained DAG, state records, content storage/projection/lifecycle, theme pipeline, generator, and relevant TypeScript transport paths listed in Appendix A. The attached documents were available in full.

A local repository checkout could not be obtained in this environment, and Bun and Cargo were unavailable. **No test pass, benchmark result, allocation measurement, platform qualification, or LOC reduction is claimed here.** The cleanup report's historic test counts are not measurements of this proposed change. Section 19 supplies the executable gates.

This is an audit of the named paths and algorithms, not a claim to have compiled every possible feature combination or inspected every line of every repository file.

### 1.4 Attached document fingerprints

These hashes identify the actual supplied files, not the older files described inside their historical appendices:

| Input | SHA-256 |
|---|---|
| `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md` | `2b6a45793175876df4ec95d60ee1114787313b1d8e7104cc5c25f24a1a541e08` |
| `POST-PERF13-ROT-CLEANUP.md` | `cb2318cb30bcdd99e91bde511c74707fdc8e28e20696b76eb41cce927d31bd45` |
| `IYON-UI-PRELIMINARY-DESIGN-v5.md` | `cc63838eff160aba0e5fda3f02d4f0e7bee16fbd3f1b30fbb1162ebd687b41a9` |
| `BRIDGE-LOWERING-MAP.md` | `b9f547a7634b594d9d32ec324656bba6a517c10b6967cbdf36f1a3e42d7819ea` |

The most relevant document sections are PERF-13 Part I §§1–8, 10–15, 17–21; cleanup §§5–10; v5 §§4, 8–14, 18–20, 23–24, 31; and the bridge map §§2–5.

---

## 2. Scope and compatibility contract

### 2.1 What must remain compatible

Preserve current observable behavior through the TypeScript package: semantic composition and reuse, all currently expressible Views, structural derivations, retained state, styles and selectors, content families and operations, Funnel configurations, Connector lifecycle, native controls, events, viewport behavior, History transfer, headless readback, terminal presentation, errors, and disposal.

Preserve current source coordinates, annotation fields, malformed-input policies, supported Unicode behavior, and explicit size limits. Preserve the distinction between synchronous acceptance and eventual visible-frame commit. Preserve the default and feature-qualified native loading paths and fail-closed metadata checks.

Private binding signatures may change with matching generator, loader, TypeScript transport, test, and artifact updates. That is not permission to silently break a supported packaged ABI. Keep the shipped content v1 ABI stable unless an explicitly versioned replacement is necessary; a stale package/native pairing must fail before mutation.

**The supported Rust UI-authoring API is intentionally not compatible.** Do not retain its prelude, fluent construction DSL, public extension traits, authoring examples, or facade merely to avoid a breaking change.

### 2.2 Terminology: distinguish application state, structure, and retained properties

There are still three planes, not four:

| Concept | Owner in this project |
|---|---|
| TypeScript `State`, dependency tracking, composition scopes, reusable semantic View values | Existing TypeScript composition subsystem; it produces structural publications |
| Semantic View DAG, node kinds, child topology/order, structural edge contracts, attachment identities | Structural plane |
| Mutable `ViewState` geometry/presentation/style-state overrides on a physical occurrence | Retained-state plane |
| Source bytes/annotations, Funnel configuration, Connector execution/selection, Port content realization | Content plane |

Do not move the TypeScript composition dependency graph into Rust under the label “state optimization.” Do not confuse `view_common_patch_root` with `ViewState.setGeometry`. The former constructs a new immutable semantic value; the latter mutates a retained override without publishing structure. [S03], [S04], [S11], [S12]

### 2.3 Explicit non-goals

This project does not introduce React, change static TypeScript `View.text` into an implicit Source API, move existing grid/track relationships into retained properties, install Taffy, install GPUI, implement a general ECS, introduce arbitrary callback-based Funnels, or replace terminal History with component-only Surfaces.

It does not make every layout change local by assertion. The current engine may need an ancestor relayout or a larger repaint when dependency information requires it. The work must make that escalation explicit, bounded where possible, and measurable—not delete a correct escalation branch because its name contains “fallback.”

It does not preserve dead Rust authoring conveniences, obsolete complete-object decoding, removed cold-lowering oracles, or an alternate content payload route.

### 2.4 Current/v5 differences that must not be smuggled into this migration

| Topic | Preserve now | Prepare for later |
|---|---|---|
| Identity | Reusable semantic DAG plus occurrence-specific layout/state bindings | React host-occurrence lifecycle |
| Layout configuration | Current structural kind/track/grid contracts and current mutable-property subset | v5's broader geometry-state schema |
| Layout engine | Current custom algorithms and cell geometry | Taffy through a downstream layout adapter |
| Content smoothing | Current rate, seal, backlog, and visible-output behavior, characterized by tests | Semantic delivery frontier before physical projection |
| History | Current retained History and receipt-driven native scrollback behavior | Component-only Surface/residency model |
| Sources | Current typed factories and mutation semantics | Additional v5 source families/semantic-record ingress |
| Host environment | Current theme/backend observations with narrow dependency revisions | Full v5 Host Environment provider/services layer |

In particular, current content code derives smoothing weight from Source grapheme projections and then masks a physical surface. That is not yet the v5 semantic-frontier implementation. Optimize the current realization without silently changing its timing or visible prefixes; §12 gives the migration seam. [S19], [S24]

---

## 3. Corrections to the attached bridge-lowering map

These decisions supersede conflicting implementation advice in that map.

| Map assumption or proposal | Audit finding | Required decision |
|---|---|---|
| Public `View` is a temporary authoring tree that must be lowered into a different retained tree | `presentation::api::View` is the same canonical `Arc<ViewNode>` handle used internally | Keep one graph; remove authoring construction around it. Do not add another graph merely to change the type's name. |
| `view_common_patch_root` and text-layout patch functions are retained-state mutations | They receive new semantic NodeIds and produce new immutable DAG values | Keep them in the structural plane. Migrate actual `NativeViewState` methods separately. |
| Existing `edit_txn_*` can become the exclusive structural batch path | `EditTxn` contains `Vec<TextLayoutEdit>`; paths are specialized; commit currently calls `host.render` | Do not force constructors, attachment publication, or general topology edits into it. A future general batch would be a new protocol, not a routing change. |
| All structural variants besides transaction batching are second architectures | Exact-root reuse, NodeId promotion, persistent derivations, small arity constructors, and buffer constructors share the authoritative retained runtime | Preserve these optimizations unless measured evidence justifies replacing one. One semantic architecture can have several specialized encodings. |
| Retained state already crosses as word masks | Actual `NativeViewState::setGeometry/setPresentation` accepts `serde_json::Value`; clear takes property strings | Generate a real retained-state schema and direct decoder, not a cosmetic change to structural patch calls. |
| Every `Value` result is JSON serialization and a second payload architecture | `serde_json::Value` can be a dynamic N-API object representation without stringify/parse; status/control are allowed on N-API | Remove measured allocation-heavy dynamic control paths, especially per-frame drains. Do not misclassify diagnostics as a second content data path. |
| N-API may carry lifecycle only | PERF-13 also allows structural/state/control/query bindings | Keep the current split: bulk Source payloads use direct FFI; appropriately shaped control stays on N-API. |
| `pub(crate)` is enough for the bridge to call another crate's internals | Core and native are separate crates | Use a narrow unsupported cross-crate integration facade. `pub(crate)` does not cross crate boundaries. [R01] |
| All styles/colors should become packed RGB | Current semantics include named/indexed/theme colors, sparse attributes, selector facts, and inheritance | Use tagged semantic records and preserve presence/clear distinctions. RGB packing is one variant, not the entire model. |
| Interning makes `connect` an IDs-only check | Source ownership, Port ownership, family compatibility, retention restartability, membership, lifecycle, and resource generations still matter | Validate those at connection/activation boundaries even when immutable config is already validated. |
| Funnel parsing dominates content hot work | Current Funnel is a small `Copy` value; projection rehydrates full text, lowers Views, recompiles surfaces, and rebuilds stable prefixes | Prioritize repeated projection/storage work. Do not add an interner lookup to a cheap Copy config without evidence. |
| `parse_key` belongs to content | It is interaction control | Keep it out of the content migration. Preserve genuinely string-valued key input unless its own measured path merits work. |
| Generated guards prove any pointer is valid | Guards check shape, nullness, alignment, and declared bounds; pointer provenance/lifetime is a caller contract | Keep the unsafe boundary small and documented; never retain JS memory after return. |
| Old T14/direct-oracle benchmark machinery should be used | Cleanup removed obsolete runners and complete-object oracles | Use surviving authoritative fixtures, new focused tests, and pinned-output traces. Do not resurrect a deleted production architecture. |

Evidence: current `View`/`map_node` [S03], structural transaction [S04], native state [S11], FFI [S26], generator [S31], current route [S32], and cleanup §§5–10.

---

## 4. Current path audit: the work that actually needs doing

### 4.1 Structural path

```text
TypeScript semantic View / composition
  → retained-dag identity/reuse/materialization
  → generated N-API or qualified structural FFI wrappers
  → NativeViewRuntime: semantic cache + NativeRef leases
  → public builder calls around the canonical ViewNode representation
  → Scene resolution / occurrence layout / paint
```

Keep the first four ownership responsibilities. Remove redundant authoring construction in the fifth.

Concrete costs:

- `text_view_from_spans` constructs styled text, calls `.wrap`, then `.text_align`. Each current `map_node` makes another root; `map_text` also invokes `Arc::make_mut` after the root was shallow-cloned. [S03], [S05]
- `parse_and_build_decorated` repeatedly applies padding, colors, border, style, states, sizing and bounds. It creates successive semantic roots for what should be one final base record. Custom glyphs go through temporary owned strings before `BorderGlyphs` construction. [S07]
- `parse_and_build_grid` builds nested staging vectors and public `GridCellSpec`s, then a closure-based grid builder clones each already-resolved child handle. [S06]
- `native_axis_from_children` constructs intermediate vectors. `PersistentSeq::from_vec` consumes a vector but then clones items into leaf chunks. Preserve the persistent tree algorithm; fix its consuming construction. [S03]
- The native semantic reference table is already dense and paged. Replacing it with a new generic map/arena is not the primary optimization. [S04]
- Attachment discovery skips clean subtrees using flags, but duplicate state detection uses a linear search through prior targets. Preserve occurrence expansion while removing the avoidable quadratic check. [S03]
- Generated N-API CString entrypoints construct `CString`s, while their common implementation reads them back into owned text. This is a separate boundary-copy opportunity; it is not a reason to fork semantic lowering. [S05], [S31]

### 4.2 Retained-state and styling path

```text
TS typed patch
  → normalize into a dynamic object / textual color representation
  → NativeViewState parses Value
  → HostViewState locks host and state record
  → native override/effect classification
  → registry snapshots / scene overlay
  → occurrence box / layout or paint
```

Concrete costs:

- Publicly typed patches still become dynamic maps and string enum/color representations at the private boundary. [S11], [S12]
- `ViewStateRecord::snapshot` clones the override state and the `BTreeMap<String, String>` of style states. `ViewStateRegistry::snapshots` rebuilds a map by visiting every record. [S13], [S14]
- `OccurrenceBox` contains base and effective decoration/style-state values. Do not add a third copy of those fields in a new lowered node. [S15]
- `HostViewState::mutate` obtains the state ID through another record lock after the mutation lock was released. Immutable IDs can be carried without relocking. [S16]
- Theme construction and resolution use owned maps, sparse styles, selector variants, and declaration order. These are real semantics, not generic JSON fields that may be dropped. [S27], [S28]

### 4.3 Content path

```text
same-image Source FFI
  → native environment/Source lookup
  → immutable Source storage snapshots
  → raw Projection<TextContent> reconstruction
  → Plain/Markdown/Diff/ANSI + annotation rewrite
  → TextRenderer emits a View graph
  → ViewCompiler creates complete rows
  → complete Surface
  → smoothing/History suffix copies
  → ContentProvider gives layout/paint a surface
```

The same content architecture is already authoritative. The problem is repeated representation work inside it.

Concrete costs:

- FFI duplicates each eight-lane annotation record into a separate `Vec<ContentAnnotationRecord>`. [S26]
- Snapshots themselves share `Arc<SourceStorage>`, but appending while snapshots are alive can clone all `SourceStorage` metadata through `Arc::make_mut`; the storage contains deques and an annotation vector. [S17]
- Head truncation reconstructs retained chunk/index/annotation collections instead of only removing the dropped prefix. [S17]
- `source_projection` calls `to_owned()` for retained chunk text. `RawDomain` then concatenates raw spans into another `String`; prefix/suffix helpers construct more owned text. [S18], [S21]
- Markdown has valuable restart/stability logic, but cached prefixes and span vectors are copied. Reference-definition dependencies can require broader reparsing; do not replace this with a fixed-tail heuristic. [S20]
- Rendering gathers cloned semantic values, lowers fresh Views, compiles, and copies cells into a complete surface. [S18], [S23]
- Smooth ticks invalidate the complete projection cache. A new projection can repeat parsing/lowering/layout even with unchanged Source bytes. `reveal_surface` clones the surface. [S19], [S24], [S25]
- Open content computes stable rows by constructing a copied Source prefix and running another parser/render pass. [S17], [S19]
- A transferred History prefix is removed by `surface_suffix`, another surface copy. [S19], [S25]
- `ContentProvider::paint` exposes width and height but not a final row window/clip contract; it returns `Arc<Surface>`. [S22]
- Source wake fan-out groups hosts using temporary vectors and a linear host search. Connector/Port commit and tick paths scan complete registries. [S25]

### 4.4 Frame path

The current host explicitly keeps a committed frame, a candidate frame, a terminal presentation receipt, captured candidate epochs/revisions, and candidate content bindings. Keep those semantics. The host uses the internal generic application/component kernel; deleting public `App` exports does not make that kernel dead. [S29]

`SceneHost::invalidate_content` clears both layout and paint caches for the host. This is a correctness-preserving coarse invalidation, not a second renderer. Replace it only after local dependency keys and damage propagation prove equivalent. [S30]

---

## 5. Target ownership and minimal internal boundary

### 5.1 Keep the two crates; stop treating the core as a supported authoring package

Keep `crates/iyon-tui` and `crates/iyon-tui-native` separate for this project. Set the core package to `publish = false`. Remove its authoring prelude and supported API documentation. Make implementation modules private or crate-visible.

Expose exactly one deliberately unsupported cross-crate namespace, proposed name `iyon_tui::binding`. It is `pub` because the native crate must link to it, not because applications are invited to author UI with it. A hidden module alone is insufficient: its actual export set must be narrow and checked.

The facade may expose opaque retained handles, passive typed input records, prepared tokens, primitive enums, and host operations needed by the binding. It must not expose the fluent View DSL, `IntoView`, a public generic renderer/projector extension ecosystem, or arbitrary user callbacks into the hot pipeline.

Do not re-export the complete old public surface under `binding`. Do not merge crates solely to make `pub(crate)` possible. [S01], [S02], [R01]

### 5.2 Proposed module responsibilities

These are new destination names, not claims that the paths already exist:

```text
crates/iyon-tui/src/binding/
    mod.rs             narrow cross-crate facade; no implementation graph
    structural.rs      typed node construction / retained derivation inputs
    state.rs           typed patch application / lifecycle / prepared binding
    content.rs         Source ingress and content-control operations
    host.rs            desired publication, barriers, native control integration

crates/iyon-tui/src/presentation/
    ir.rs              existing canonical retained nodes and persistent edges
    node_factory.rs    final-record construction; no fluent intermediate roots
    layout/            existing custom engine, unchanged semantic contract
    paint/             current terminal painting

crates/iyon-tui/src/retained_state/
    ...                canonical overrides, effects, binding, changed-version capture

crates/iyon-tui/src/content/
    ...                canonical semantic text/projection algorithms
    storage/           shared UTF-8 pages and retained sequence/range utilities
    execution/         Connector semantic/delivery execution products
    terminal/          current-engine content lowering and row-window painting

crates/iyon-tui/src/application/
    host.rs            one host transaction coordinator
    content.rs         initially remains owner; split by responsibility, not by line count
    environment.rs     current environment wake/lifetime authority

crates/iyon-tui-native/src/
    tui/...            small N-API adapters to binding::*
    content_ffi.rs     same-image unsafe ingress and status conversion only
    generated/...      generated boundary glue only
```

Do not create all these directories as empty scaffolding. Move a cohesive implementation when its caller is migrated. A module split must delete an old owner, not duplicate one.

### 5.3 Three lowering destinations

```text
STRUCTURE: validated kind + immutable fields + resolved retained child handles
           → one canonical retained node / persistent derivation

STATE:     validated property operation + retained state identity
           → canonical sparse override record / immutable frame version

CONTENT:   validated borrowed bytes/records or immutable typed Funnel config
           → Source storage / Connector control and execution
```

Only the host frame coordinator reads products from all three together. Structural ingress does not call state/content mutation dispatchers. Resolving an attachment is not dispatching an operation to that resource.

### 5.4 Passive types that should survive when already efficient

`Insets`, wrap/alignment enums, sparse style values, validated grid track/cell values, source ranges, and the canonical retained node handle may survive internally. Removing their builder methods or public reachability is different from inventing replacement types for the same bits.

Prefer `NodeRef` as an internal name for the canonical View handle only if the rename makes ownership clearer and can be mechanical. It must remain the same representation, not a new wrapper tree around `View`. A narrower initial implementation may retain the internal name `View` while deleting its public authoring contract.

### 5.5 Allowed ownership operations

| Operation | Policy |
|---|---|
| Copy small scalar values, masks, IDs, ranges | Allowed; keep compact and typed |
| Borrow retained nodes/styles/rows inside a bounded frame scope | Preferred |
| Move newly owned text/config/edge storage into its owner | Preferred |
| Retain an immutable `Arc` for a new independent owner or asynchronous receipt | Allowed, counted at the ownership boundary |
| Clone a whole String/map/tree/surface to traverse or restyle it | Forbidden on the target hot path |
| Copy incoming borrowed FFI bytes once into native-owned storage | Required unless a real ownership-transfer ABI is explicitly designed; none is assumed here |
| Copy a changed bounded tree path/chunk page for snapshot persistence | Allowed and measured; must not scale with all retained data |
| Keep a raw pointer into a JS TypedArray after return | Forbidden |
| Replace safe sharing with lifetime-erased pointers to meet a clone counter | Forbidden |

Add separate counters for payload bytes copied, metadata items copied, retained-node constructions, shared-owner retains, and physical cells copied. Counting every `.clone()` as equivalent hides the actual work.

---

## 6. Structural lowering: construct the final retained value once

### 6.1 Construction contract

Introduce an internal final-record constructor adjacent to `ViewNodeParts`. It must accept all already-validated common fields, node-kind payload, style facts/states, and optional state/content attachment identities. The current `ViewNodeParts` omits attachments; extend it rather than applying attachment modifiers after constructing the root. [S03]

Conceptual contract—not a proposed public Rust authoring API:

```rust
// Inside the core crate. Exact field types should reuse the current IR.
struct NodeParts {
    kind: NodeKindPayload,
    common: CommonBase,
    attachments: AttachmentIds,
}

fn finish_node(parts: NodeParts) -> NodeRef {
    // Compute aggregate flags once; assign one semantic native identity;
    // move the final fields into the existing canonical node allocation.
}
```

The native binding keeps ownership of wire validation and native reference resolution. The factory keeps ownership of canonical native invariants. Neither recreates the other's complete model.

For every constructor:

1. Validate the runtime/session generation and the outer ABI envelope.
2. Perform the existing semantic NodeId lookup at the same point as today. A live identity hit must not parse payload, visit children, or construct a new node. Generated envelope guards may still run first; do not incorrectly promise that invalid pointers/capacities bypass those guards.
3. On a genuine miss, decode the used payload, validate all semantic fields and referenced resources, and prepare owned payload storage.
4. Resolve child references once into retained handles. Borrow them for inspection; retain ownership only when the new node/lease needs it.
5. Assemble final common fields and node payload without calling a public fluent modifier chain.
6. Publish through the existing semantic identity and lease authority.
7. On failure, release all temporary owned records/leases. No half-created root becomes desired or visible.

One logical node may still require a payload allocation and a root allocation. The requirement is **one final logical root**, not a misleading claim that every kind fits into one heap allocation.

### 6.2 Text constructors

**Touch:** `text_view_from_spans`, `text_view_from_owned`, `cstring_text_spans`, `utf8_text_spans`, `publish_cstring_text`, `publish_utf8_text`, fixed-arity text implementations, and the variadic text-buffer implementation in native `tui/view_abi.rs`; `presentation/api/text.rs`; the new internal node factory. [S05], [S08]

Implement as follows:

1. Keep the fixed one-to-four-span lanes and the arbitrary-span buffer lane semantically equivalent. Do not reinstate a four-span ceiling.
2. Validate the sum of span lengths with checked arithmetic against the used byte length. Validate the buffer as UTF-8 and each span boundary. Empty strings are not the same as an empty span list. Preserve embedded and trailing NUL through the length-delimited lane.
3. Reuse existing inline/page-slice text storage. For a contiguous multi-span buffer, retain one native-owned page and make spans reference checked ranges instead of allocating one String per span. Small inline spans may remain inline where that is cheaper.
4. Decode wrap and alignment before constructing `TextView`; put them in its final fields. Do not call `.wrap(...).text_align(...)` on a freshly allocated root.
5. Move the final span storage into the text payload. Do not collect an already-owned span vector into another vector merely to satisfy `IntoIterator`.
6. Preserve `StyleRef`, `StyleFacts`, cursor anchors, and text-specific rendering semantics. A passive internal `TextSpan` may remain if it is the actual consumed storage shape; the fluent `Text` wrapper must not remain on this ingress path.

For N-API string lanes, investigate the verified `String → CString → CStr → owned String` round trip in the generator and implementation. The preferred end state is a generated safe adapter that transfers its already-owned UTF-8 String into the same typed constructor that the borrowed FFI lane uses. Do not add a second semantic implementation. The borrowed lane copies once; the owned lane moves. Preserve NUL rejection on CString-contract calls and the explicit NUL-capable byte lane. Changing which lane the TS materializer chooses requires a focused benchmark, not an assumption that strings or buffers always win. [S31]

**Gates:** native root constructions per new plain text node are independent of the number of common modifiers; bytes copied after native ownership are zero; all 1/2/3/4/N-span, empty-string, Unicode, NUL, invalid-length, invalid-style-reference and cache-first cases pass.

### 6.3 Common properties and decoration

**Touch:** `view_common_patch_root_impl`, `parse_and_build_decorated`, `native_with_state_attachment`, content attachment construction, and the core common-field factory. [S03], [S04], [S07]

Decode into a stack/local candidate of the final base values. Preserve this exact observable order:

```text
existing child base
  → padding
  → background / foreground
  → border style or custom glyph replacement, then edges and color
  → sparse style overlay
  → style-state entries in their established order
  → width / height
  → min/max bounds
  → retained overrides later, at the occurrence stage
```

Do not flatten semantically distinct containers, clipping boundaries, Hanging, ClampRows, or viewport controllers. Only property-only decoration is folded into the owner occurrence's base.

A common patch of an existing immutable node creates one new root with one new native semantic identity and unchanged payload ownership. It does not mutate a root already held by a visible frame, another host, a cache, or a semantic lease.

Avoid an unconditional `Arc::make_mut` strategy on shared published nodes. Even when an input handle is moved into the function, other owners may exist. Use a deliberate fresh root assembled from shared immutable payloads. A uniqueness fast path is allowed only where native identity/cache semantics remain correct and a benchmark proves it useful.

Custom glyph parsing should validate borrowed UTF-8 slices, construct the final eight-glyph value once, and avoid the temporary `Vec<String>` followed by another construction copy. Keep existing glyph validation: never silently truncate a multi-cell grapheme to fit a one-cell border.

**Gates:** nested modifier precedence, no double border/padding inset, same attached occurrence identity, custom glyphs with every edge mask, explicit null/clear behavior through state, and malformed trailer rollback.

### 6.4 Axis and persistent sequence construction

**Touch:** `PersistentSeq::from_vec`, branch construction, `native_axis_from_children`, `native_axis_splice`, axis builder finish, and native axis materializers. [S03], [S04]

The existing persistent sequence is valuable. Keep its structural sharing, subtree lengths, aggregate flags, and path-copy edits.

Replace consuming construction that clones slices with consuming construction that moves items:

```text
owned input vector → consuming iterator
  → fill one leaf's owned item storage, up to the existing branch factor
  → compute leaf aggregate once
  → move leaf handles into parent nodes
  → compute parent sizes/aggregate once
  → continue until one root remains
```

Do not use `items.chunks(...).map(|chunk| chunk.to_vec())` when ownership of `items` is available. Do not decode `(track_word, child)` into one temporary vector and then map it into another. Decode and move directly into the final RowChild or ColumnChild representation.

Keep persistent `set` and `splice`; do not flatten a 10,000-child axis for a one-child change. When several children of one parent change in a transaction, prepare the changed leaves/branches once and publish that parent once. Preserve all per-node NodeId publication records required by the retained path.

For traversal scratch, reuse bounded stack storage or a caller-owned vector. Do not introduce recursive stack overflow risks to avoid a small allocation. A very deep valid tree must either work under current supported limits or fail explicitly before publication, never select a removed architecture.

### 6.5 Grid lowering

**Touch:** native `parse_and_build_grid`, core grid normalization/building helpers, the canonical grid payload and persistent cell sequence. [S06]

The expensive part is not `GridCellSpec` itself—it is Copy-like configuration and may be retained internally. The expensive part is staging public builder inputs and replaying them with cloned child handles.

1. Decode the framed word cursor with checked counts and full-consumption validation.
2. Preserve track decoding, row/column span validation, alignment, auto-placement, occupancy, cell ordering, implicit track behavior, and cell-coordinate lookup semantics.
3. Extract the existing placement/normalization algorithm into a private function that accepts consuming typed rows/cells or decoded iterators. **Do not reimplement CSS Grid or v5 geometry here.**
4. Build the final track storage, cell sequence, occupancy/coordinate index and flags once.
5. Move child handles into their final cells instead of iterating `&cells` and cloning them into a public closure builder.
6. For `native_grid_set_cell`, preserve placement metadata and update only the persistent child path. Do not rebuild the placement index when coordinates/spans did not change.

A malformed final cell must leave no published root. Temporary validated storage is allowed during preparation; it is not necessary to pretend all validation can occur without any scratch space.

### 6.6 Other kinds and static diff

Cover every concrete native kind: Text, Spacer, Row, Column, Grid, Hanging, Container, ClampRows, RowViewport, ContentHost, ComponentSlot. Cover semantic decorations that normalize into those kinds. New factory coverage must be exhaustive; no wildcard “unsupported” branch may silently shrink the expressible domain. [S10]

Static `View.diff` and a streaming Diff Funnel are not identical input contracts. The static path carries typed hunks, safe-integer coordinates, line numbers/offsets and termination metadata. The content path parses unified-diff text into semantic roles. Preserve both public TS features.

Refactor their reusable formatting/data algorithms internally, but do not serialize typed static hunks back into a text Source merely to claim one path. Both may use the same internal terminal primitives while retaining their own validated semantics. Remove `DiffRenderer` as a public Rust authoring requirement; keep the proven formatting algorithm behind a private lowering function.

### 6.7 Leases, cache-first behavior, and attachment discovery

Keep `NativeViewRuntime` as the structural semantic identity/lease authority. A core factory returns a retained node handle, not a new unrelated native ID graph.

`ensure_lease` should receive the already-resolved handle or a prepared lease record. Replacing it with “node ID plus words” would cause redundant resolution or decoding. A lease acquisition is an ownership operation, not node construction.

Preserve:

- exact-root reuse without semantic payload inspection;
- generation-scoped weak TS hints, not hidden per-node strong leases;
- live NodeId promotion and the existing equality/conflict contract;
- monotonic/disjoint native reference namespaces and page reclamation;
- one bounded retained stale-reference recovery, then explicit failure;
- desired, visible and in-flight resource protection;
- current cross-host/source-environment validation.

For attachment discovery, use an explicit per-candidate `seen_attachment_id` set for uniqueness and an active-path set for cycle diagnostics. Continue expanding repeated semantic subtrees into separate occurrence uses. A global “visited semantic node” set would miss a duplicated attachment in a reused DAG subtree. Unattached subtrees can still be skipped using the existing aggregate flags.

Do not format diagnostic occurrence paths unless an error actually occurs. Store compact parent/index data or reuse the traversal stack and format both conflicting paths on the cold error path.

### 6.8 Batching decision

There is **no mandatory new generic structural batch protocol in this handoff**. First remove native construction waste and measure surviving crossing cost.

Existing specialized `edit_txn_*` remains a text-layout edit facility unless separately redesigned. Do not wire it into H3 desired publication as though its `commit_render` were an infallible desired-only commit.

A later measured batch extension must have all of the following: distinct typed opcodes, explicit temporary-reference scope, no dependency cycles, complete validation before publication, lease rollback, bounded buffers, one desired-state prepare/commit contract, and a fast no-op/exact-root path. Its decoder must call the same factories as scalar lanes. “One batch per render” alone is not a performance proof: encoding, scratch retention, changed-frontier inspection, and validation are part of the cost.

---

## 7. Retained-state lowering and changed-version capture

### 7.1 Generate the actual property protocol

**Touch:** TS `transport/state/control.ts`, native `tui/view_state.rs`, `retained_state/{geometry,presentation,capabilities,record,registry}.rs`, `application/view_state.rs`, and the ABI generator's schema/model/renderers. [S10], [S11], [S12], [S13], [S14], [S15], [S16], [S31]

The new private state decoder must terminate directly in canonical override values. Do not route it through immutable View modifiers or a generic `setProps` object.

Use a finite property schema with stable IDs, domain, native value kind, nullability, clear support, capability rule, and diagnostic name. Generate TS normalization/encoding and Rust decoding from that schema. Keep the current public TS method names and error shape.

Use compact masks for the common finite geometry/presentation fields. A property has four distinct operations:

```text
absent      leave the override unchanged
set(value)  install an override
set(null)   install semantic none, only where the property is nullable
clear       remove the override and reveal the current base
```

An acceptable concrete envelope has `set_mask`, `null_mask`, `clear_mask`, followed by schema-defined value lanes and byte ranges. Require `null_mask ⊆ set_mask`, `clear_mask ∩ set_mask = ∅`, and no unknown bits. Null legality is per-property. A no-key clear operation remains distinguishable from an explicit empty clear list. Preserve duplicate-key rejection where current API validation requires it; encoding must not erase an error by silently deduplicating.

Dynamic style-state keys remain a separate typed operation, not a free-form property map. Use immutable semantic atoms or move owned strings into state storage. Do not invent enum IDs for arbitrary application keys.

### 7.2 Validate and commit each call atomically

For a state call:

1. Decode the complete patch and validate scalar domains, finite numeric values, strings, glyphs and masks.
2. Resolve the live state/host identity and validate against the desired target kind. Account for prepared/in-flight bindings under the current controller's serialization rules.
3. Compute proposed changes in a small local candidate or change journal. Validate cross-field relationships before mutating the record.
4. Compare stored overrides and effective values with the appropriate previous values.
5. Commit the changed override entries once, advance relevant revisions, and enqueue native effects.
6. Return a primitive wake disposition. Do not allocate a one-property JSON object for every successful patch.

Do not treat “same effective value now” as permission to omit a semantically different override. Setting an override equal to today's base can matter after a future base change. Store that override correctly even if it needs no immediate paint. Conversely, a repeated identical override should not schedule work.

### 7.3 Separate immutable base from retained override and effective frame values

Keep the logical model:

```text
latest accepted immutable View base
  + retained explicit override
  + current native control/focus facts
  = effective candidate occurrence state
```

Store the base by reference to immutable node/base data where possible. Avoid copying a full base decoration and style-state map into every candidate occurrence merely to resolve an unchanged field. Small scalar geometry may be copied directly; pointer chasing should not replace a cheap Copy field without evidence.

Use compact effective values for layout/paint and retain only the semantic references necessary for later re-resolution. Clearing after remount must expose the new base. Retained states must not be keyed solely by semantic ViewId; one shared semantic View can expand into multiple physical occurrences.

### 7.4 Replace whole-registry snapshotting

The target is not `all records → clone all maps → new HashMap` per frame.

Add a host-owned dirty-state worklist, deduplicated by a generation/queued marker. Each state record publishes an immutable version only when its logical values change. A frame capture contains:

- references to unchanged committed versions already required by the visible tree;
- newly demanded versions for a structural mount/remount;
- changed versions through the captured epoch;
- binding changes prepared for that candidate.

Use **one candidate overlay over the committed version table** for this project. Reuse the existing lookup shape, make values immutable shared versions, and put only changed/newly demanded IDs in the candidate overlay; read through that overlay, then the committed table. Commit installs the touched entries. Avoid retaining an unbounded chain of overlays: each candidate is one overlay over the committed table, and commit folds touched entries into the owner table.

This is a frame-lifetime mechanism, not a second mutable state authority. Old versions survive only while a visible/in-flight frame needs them. Record how failed candidates release them.

A full initial mount may inspect all mounted state attachments. A paint-only patch on one existing attachment must not snapshot every unmounted resource in the host.

### 7.5 Lock and identity reduction

The current host lock already serializes `HostViewState::mutate`; the record is then separately locked and locked again to read its ID. First remove the redundant ID read by carrying the immutable ID in the wrapper or captured mutation result. [S16]

Do not replace all `Arc<Mutex<Record>>` objects in one speculative sweep. Audit every access first, including disposal, native ticking, prepared publication, diagnostic readback and owner teardown. When a record is proved exclusively host-serialized, store it directly in the host registry and give wrappers an ID plus weak host reference. This removes duplicated ownership/locking without changing semantic lifetime. Any record still accessed independently keeps the required synchronization until its access path is deliberately unified.

Do not add slot reuse merely because a new registry shape makes it easy. Existing monotonic IDs already avoid revival within their namespace. Any future recycling requires generations everywhere the identity crosses a boundary.

### 7.6 Effect classification and current layout integration

Keep Rust authoritative for consequences. The schema carries semantic property categories; the current layout adapter refines them into current `StateEffects` and dependency work.

Required distinctions:

- color/attributes/style changes: style resolution and damage, not measurement;
- border edge presence/padding/bounds: geometry and paint;
- border glyph/style/color with unchanged edge geometry: paint;
- gap: only supported on the current axis/grid kinds;
- horizontal alignment: current Text capability;
- vertical alignment: current Row capability;
- component indirection: not a physical state-capable box;
- style-state/focus facts: subtree presentation invalidation when selectors can observe them.

Do not transplant v5's broader layout-property classification into the current engine. Keep a narrow function from semantic state effects to current layout/cache invalidation so Taffy later replaces physical propagation, not the state protocol.

### 7.7 State acceptance gates

A one-leaf paint patch must have zero structural publications, zero new semantic nodes, zero semantic content parses, and zero measure/place work unless a separately documented dependency genuinely requires it. A geometry patch must preserve current results and damage both old and new occupied regions.

Test unmounted configuration, incompatible attach, compatible remount, duplicate attachment, desired/visible/in-flight transitions, clear after base changes, explicit null, invalid whole patches, same-value writes, focus/style-state inheritance, and disposal during pending presentation.

---

## 8. Styling and theme lowering without semantic loss

### 8.1 One internal semantic style vocabulary

Move efficient passive style records out of the authoring facade as needed; do not duplicate them into `WireStyle → PublicStyle → InternalStyle → PaintStyle`.

Use one semantic style representation with compact native atoms and tagged colors. Its data must preserve:

```text
foreground/background absent versus explicitly specified
named terminal color / indexed color / RGB / theme reference
semantic default/inherit distinctions already exposed by current behavior
attribute presence bits separate from attribute true bits
StyleRef theme key plus sparse local override
style-state and immutable style-fact predicates
text role/part/annotation/language/origin/format selectors
```

Only the host paint resolver produces terminal-realized colors/styles. Source annotations contain semantic style data, never a host-native palette/style-table index.

### 8.2 Native theme construction

Replace `lower_theme(Value)` with a generated typed theme input decoder or a typed N-API DTO whose fields terminate directly in the internal immutable theme table. The choice must reduce work rather than add an encode/decode pass for rare calls. Prefer compact word/byte records when the same schema is shared with hot style/state ingress; retain typed DTOs for genuinely rare diagnostic surfaces.

Compile selector variants in one batch. Current theme variants are ordered by predicate count and declaration order; replacing the same selector updates its declaration order. Repeated sorting after each insertion can be replaced with one final sort **only if duplicate-selector replacement and declaration ordering are exactly preserved**. [S27]

Keep all text-selector fields currently consumed by `lower_text_selector`: roles, parts, semantic annotations, language, origin, format, focus/focus-within, and application style states. A theme is not just a palette and two colors. [S28]

### 8.3 Interning policy

Intern immutable strings/semantic styles when they are reused; do not repeatedly parse `theme:...`, `ansi:...`, or `#rrggbb` in materializers. Share the environment-level semantic atom authority across the ingress paths that need it. Do not create one duplicate style vocabulary per plane.

Keep host-resolved paint caches host-owned. Two hosts may resolve the same Source semantic style differently.

Define atom lifetime explicitly. Existing generation-lifetime tables may be kept for small bounded configuration, but a stream of unique values must not create an unbounded permanent interner. Either retain atoms by immutable owner references with reclaimable indexing, or document/enforce a bounded cache policy that never evicts a still-live semantic value. Fingerprints accelerate lookup; equality resolves collisions.

The current Funnel configuration is small and Copy. Normalize/validate it once at creation or connection, then store it directly. Add Funnel interning only when repeated large configuration or measured deduplication benefit exists. Hashing/locking an interner for every cheap config is not a mandatory optimization.

### 8.4 Revision domains

Capture immutable theme data by shared ownership, not by cloning the whole theme map for each content provider pass. Use distinct revision domains for:

- semantic content and annotations;
- effective presentation/theme resolution;
- text measurement environment when it affects metrics;
- viewport window/clip.

A theme recolor must not call the Markdown/Diff/ANSI parser, rebuild semantic IR, or republish a structural View. It may repaint every styled visible occurrence until narrower theme dependency indexing is proven. A future font/scale change is different from a palette change; keep the seam capable of distinguishing them without implementing GPUI now.

### 8.5 Public-surface deletion

Delete redundant public Style/Theme convenience wrappers only after native adapters and internal consumers use canonical records. It is acceptable for an efficient internal sparse `StyleSpec` to keep its name. It is not acceptable to preserve the public Rust fluent facade and claim success because the TS bridge bypasses it.

---

## 9. Source ingress, storage, and annotations

### 9.1 Keep the mandatory same-image FFI data lane

**Touch:** native `content_ffi.rs`, Source mutation methods in `application/content.rs`, TS `transport/content/ffi.ts` only when the boundary contract changes, and the canonical ABI metadata/schema sources. [S17], [S26]

Keep Source payload mutation independent of a host. Each call validates environment slot/generation and Source slot/generation. The Node-API control module and `bun:ffi` must resolve the same staged native image and the same environment registry. Do not add a second library or a small-payload N-API mutation path.

Native must own accepted bytes before return. An ordinary borrowed TypedArray is not transferable storage. A raw pointer plus a length does not justify zero-copy retention across calls.

Keep clear, seal, replace and truncate as their existing logical operations. Never implement replacement as two accepted operations `clear(); append()`.

### 9.2 Remove the duplicate annotation-record vector

`copy_records` currently maps the fixed eight-lane FFI record array into a second vector containing the same eight fields. Delete that intermediate allocation. [S26]

The lowest-risk implementation is to pass a borrowed record view or a monomorphized exact-size iterator of small Copy record values into the canonical validation function. The native adapter may map one record's fields by value; it must not collect a second DTO vector. The final retained semantic annotations still need owned storage.

Do not `transmute` between two Rust structs just because their fields currently look alike. Either share an explicitly defined `repr(C)` record with compile-time size/alignment checks, or use the borrowed reader/iterator. The latter avoids coupling the semantic annotation type to the wire layout.

Validate count, multiplication, payload offsets/lengths, unknown flags, reserved lanes, kinds, UTF-8 ranges and payload schema before accepting the Source mutation. Preserve operation-local offsets on the wire and absolute generation-qualified offsets in storage.

### 9.3 Validate UTF-8 and indexes once per ingestion boundary

Create a private validated input view after the native boundary checks. Source storage methods receive that validated view rather than repeatedly converting the same bytes to UTF-8 in adjacent helpers. Safe Rust-only callers can construct it from `&str`; FFI callers must validate.

Combine compatible scans of incoming bytes: line-break indexing and new tail coordinates should be computed during the same bounded pass where feasible. Do not rescan all retained content to update the new line count.

Validation required for semantic correctness remains. “One scan” is not permission to omit span-boundary checks or to change CR/LF behavior. Do not normalize newline conventions as a storage optimization.

### 9.4 Source storage representation

The current `Arc<SourceStorage>` snapshot has the right ownership concept, but its mutable payload contains whole deques/vectors. Copy-on-write on that object can copy metadata proportional to the entire Source. Replace that container with a persistent chunk/index root rather than removing snapshot immutability. [S17]

**Target representation:** immutable UTF-8 pages with range views; a persistent, indexed sequence of chunk descriptors; retained line/range indexes; and a persistent annotation index. Snapshot creation retains root references and scalar stamps.

```text
Source record (mutable, serialized)
  identity / generation / revision / seal state
  current persistent storage root

Storage root (immutable)
  retained head / tail
  chunk sequence root
  line index root
  annotation index root

Chunk descriptor
  native-owned UTF-8 page
  page-local start / length
  absolute Source start
```

Use the existing 16 KiB chunk scale as the initial tuning point; do not change limits and chunk policies while changing semantics. Extract reusable persistent-sequence mechanics only if the existing sequence can be made domain-neutral without pulling View-specific aggregate flags into content. Otherwise implement a small Source-specific indexed sequence with the same bounded path-copy principle. This is not a general public collection library.

A branch stores subtree byte/line/chunk counts sufficient for indexed lookup and prefix splitting. Appending updates the right-edge path and adds new descriptors. Dropping a retained head splits/removes the prefix path. Unchanged branches remain shared.

Required complexity, excluding unavoidable eventual reclamation:

| Operation | Target |
|---|---|
| Snapshot | O(1) root retains plus scalar copy |
| Append | O(new bytes + new annotations/index work + logarithmic metadata path), not O(total retained bytes/metadata) |
| Replace | O(replacement input), fresh root, one accepted revision |
| Head truncation | Indexed prefix split plus affected boundary/annotation work, not a scan of every surviving chunk |
| Locate byte/line | Indexed lookup |
| Paint-only/theme tick | No Source storage rebuild or full-text copy |

Releasing the last large old snapshot can free many nodes/pages. Count that cost and avoid holding Source/host locks while destructing an entire detached old root when it can safely be dropped outside the critical section. Do not hide unbounded deferred reclamation behind a new unbounded queue.

### 9.5 Append and replace algorithms

**Append:**

1. Validate environment/source lifecycle, seal state, payload/annotation limits, UTF-8, ranges, and capacity arithmetic.
2. Preflight next Source revision, tail offset and all generation/counter increments that could return a normal error.
3. Build new owned payload pages and their indexes from new bytes only.
4. Compute retention consequences using the index. With overflow=`error`, reject the entire candidate.
5. Prepare the updated persistent chunk/line/annotation roots. Do not mutate a root held by a snapshot.
6. Under the Source serialization boundary, install the complete new root and revision together.
7. Capture eligible subscriber tokens, release the Source lock, and route native wakes.
8. Return an acceptance result that truthfully reports the accepted revision.

**Replace:** construct a fresh root and new content generation, validate retention and annotations, then swap once. Old snapshots remain valid. Clear follows the existing atomic empty-replacement semantics. Seal and truncate follow their existing ordering and error contracts.

Do not merge small appends by mutating a published immutable page. A bounded tail optimization may mutate uniquely owned storage, or copy a bounded partial tail if measured and accounted for. It may not make old snapshots change. Initially prioritize eliminating whole-root metadata copies; add tail coalescing only with copy counters and fragmentation evidence.

### 9.6 Correctness risks to characterize before storage changes

The inspected Source methods contain checked revision/generation transitions near mutation, and wake routing can report errors after content acceptance. These are **source-inspection risks requiring tests**, not runtime failures demonstrated in this audit. [S17], [S25]

Add forced-boundary tests for counter exhaustion: no accepted text/annotation mutation may occur and then return a normal “not accepted” validation failure. Preflight fallible arithmetic before installing candidate storage.

Add a test where one subscriber host becomes unavailable/poisoned after Source acceptance. A successfully installed Source revision must not be reported as an unaccepted append that callers could retry and duplicate. Report host failure through the host/environment error channel, continue handling other eligible hosts, and keep accepted Source state authoritative. Any genuinely fatal environment condition must have an explicit result/poison contract, not an ambiguous ordinary rejection.

### 9.7 Annotation indexing and ownership

Keep semantic annotation payloads host-independent. A role/theme key is semantic data; a resolved host style index is not.

Preserve the existing ordering of overlapping annotations. An interval index ordered by start coordinate alone must not accidentally change overlay precedence. Store a stable insertion/order discriminator when needed and return overlaps in the original semantic application order.

Build/query a Source-coordinate interval index instead of scanning every annotation for every text run. For monotonic run traversal, use a sweep cursor; for arbitrary viewport/range queries, use indexed overlap lookup. Only split runs at intersecting annotation boundaries.

On truncation, continuous spans clip, atomic spans drop when incomplete, point anchors obey their specific boundary rule. Text and annotation changes commit in one Source revision. Preserve exact, derived and synthetic provenance distinctions. Do not invent precise character-to-source mappings for transformed entities/escaped text where the current IR only has a derived range.

When optimizing `RawDomain` and annotation rewriting, keep source witnesses intact. A substring borrowed from a retained page must carry its original Source range; a transformed string owns derived text with derived provenance. Semantic correctness is more important than manufacturing an “exact” zero-copy tag.

### 9.8 Limits and ABI generation

Keep current payload/annotation/projection limits until separately changed. The inspected content implementation uses a 64 MiB per-call Source payload bound, 16K annotation records, a 4 MiB sidecar payload bound, and terminal projection limits. These are distinct limits, not a single global text limit. [S17], [S26]

The content ABI constants/fingerprints are currently hand-shaped in `content_ffi.rs`; do not claim that a structural TOML regeneration automatically updates them. Make new record layouts and status mappings come from one canonical schema or explicitly checked shared constants. Include the content metadata probe, TS decoder, exported symbol list and native artifact fingerprint in the same change.

No recoverable OOM promise is introduced. Input/retention limits fail before acceptance; allocator exhaustion follows the runtime's fatal policy.

---

## 10. Content execution products and cache dependencies

### 10.1 Keep the entity model

```text
Source ───────── Connector ───────── ContentPort
                    ▲
                    │ immutable configuration
                  Funnel
```

Source owns accepted bytes/annotations and revision. Funnel owns immutable configuration. Connector owns relationship-local parser/delivery/projection state. Port owns destination binding/selection. Viewport owns scroll/follow intent. The terminal History adapter owns transfer receipts/frontiers that are specific to History.

Do not reduce objects by moving parsing into Source or Port. Do not create a ceremonial Funnel execution object in addition to Connector. Small immutable Funnel values can remain inline in the Connector record.

### 10.2 Replace the monolithic projection key with layered products

Current projection keys mix Source revision, width, delivery and theme. A miss repeats too much work. Split the product into the following stages, each with explicit keys:

| Product | Required dependencies | Must not depend on |
|---|---|---|
| Accepted Source snapshot | Source identity/generation/content generation/revision/retained range | Host, width, theme, scroll |
| Semantic result | Source stamp, semantic Funnel options, annotation semantic revision, parser restart context | Theme, viewport offset, terminal color capability, Smooth clock |
| Delivery state/view | Semantic result/frontier, delivery policy, Connector-local clock/history | Another Connector's reveal state |
| Terminal content layout model | Semantic/delivery-visible geometry, width, wrap/alignment/policy, text-measurement environment | Palette-only/theme recoloring or viewport offset |
| Resolved paint | Semantic roles/styles, inherited occurrence style/facts, theme/presentation subrevision, relevant host realization | Source reparsing |
| Window materialization | Prepared layout/paint products, row window, clip, allocation, current delivery visibility | Rebuilding offscreen semantic content |

Not every implementation needs one allocated object per row in this table. These are dependency/ownership boundaries. Fuse cheap stages where inputs and lifetimes coincide; do not fuse invalidation domains.

### 10.3 Capture a Source once per attempt

A host frame captures one immutable snapshot per Source needed by its visible or requested Connectors. Reuse that snapshot within the attempt, including convergence iterations. Two Connectors over one Source can have distinct semantic policies and delivery state while referring to the same captured Source revision.

Do not reacquire “the latest Source” independently during measure and paint. A mutation accepted during preparation may be rendered in a later frame. Its pending epoch must survive the current commit.

The captured frame is not a transaction that retroactively makes independent Sources mutate atomically. It is a consistent set of chosen immutable snapshots. Preserve native Source linearization and host epoch guarantees without inventing a global Source revision.

### 10.4 Prepared projection tickets

Introduce an internal prepared ticket returned/recorded during content preparation. It identifies the exact Connector candidate, captured Source stamp, semantic product, width/layout product, delivery view, and paint dependencies used for that attempt.

```text
prepare content under candidate constraints
  → PreparedProjectionTicket
  → measure consumes ticket metrics
  → placement derives final viewport
  → paint consumes that same ticket + final window
  → successful host commit installs its visible association
```

The ticket is not public and contains no JS pointer. It is either borrowed from candidate storage for the bounded attempt or retained by a candidate frame while a backend receipt is outstanding.

A cache entry is not automatically the visible projection. Failed candidates can leave reusable immutable derived cache entries, but they cannot advance visible selection, visible delivery frontier, subscriptions, or committed geometry. Do not confuse speculative cache residency with semantic commit.

### 10.5 Preserve transactional Connector switching

For A visible and B requested:

1. Snapshot B's Source and prepare its semantic/layout/paint products inside the same convergence loop as the rest of the frame.
2. Keep A and its visible projection/subscription available.
3. On operational B failure, store a typed failed-attempt key/status; use A under valid candidate constraints or the defined empty result when there is no A.
4. Allow unrelated valid state/structural work to commit according to the existing contract.
5. On successful backend receipt, atomically install B's selection, binding, represented Source revision, delivery-visible state and subscriptions.
6. Release A's no-longer-needed derived state after the transition, subject to explicit owners.

Latest requested selection wins before the captured boundary. Deactivation, active-Connector disposal, unmount and remount must use the same transaction discipline.

Do not continuously retry a failed key. Retry only on relevant input change, explicit retry/readiness, or the existing configured policy.

### 10.6 Cache residency and cold behavior

Inactive/unmounted Connectors retain identity/configuration/membership, not semantic projections, width caches, active Smooth timers, or Source subscriptions. Preserve the current inactive semantics before adding any v5 offscreen residency concept.

Bound per-Connector width/layout cache entries and shared immutable semantic cache residency. Eviction must not destroy a product held by a visible/in-flight frame. A Source may outlive every host; releasing a host must remove its subscriptions and Connector ownership without disposing the Source.

Optional cross-Connector semantic cache sharing is keyed by full Source and semantic-transform identity. Do not share mutable parser cursors, failure state, reveal clocks, viewport intent or selection. Implement this only after per-Connector repeated work is fixed; sharing is not required to establish correct ownership.

---

## 11. Semantic text, Markdown, ANSI, and diff

### 11.1 Reuse the existing semantic text IR

The repository already has `TextContent`, blocks/inlines, `TextRun` provenance, annotations, role/part/fact selectors, projectors and a generic text renderer. Do not build a second “v5-ready” text document beside it. [S20], [S21], [S23], [S37]

Refactor storage and ownership of that IR so unchanged blocks/runs can be shared. Use immutable block/run sequence references with stable content identities or revisions. Avoid deep `Vec<TextContent>` clones when a consumer can iterate borrowed values or retain an immutable sequence root.

Exact source text references retained native pages/ranges. Derived parser output owns its transformed text once. Synthetic markers remain synthetic. Preserve all existing validation and source-coordinate algebra, including elided syntax.

### 11.2 Remove raw rehydration before optimizing the grammar

Replace `SourceSnapshot → TextContent::raw(text.to_owned())` with a borrowed/page-backed raw input representation. `RawDomain` must not concatenate the entire retained Source on every call and then copy prefix/suffix domains again. [S18], [S21]

The current parser library needs contiguous `&str` input. That does not mean the entire Source must be recopied for every frame. Give the Connector parser a retained contiguous working buffer for the region that actually needs parsing, and preserve source-piece witnesses separately.

For an append requiring a suffix parse, append only new bytes to that working region; drop or rebase stable prefixes when their parser dependencies allow it. Borrow a contiguous page/range directly when possible. Capacity reuse does not eliminate copied-byte cost; count both capacity allocation and bytes assembled.

A grammar dependency that legitimately requires a full reparse may assemble/parse the full required domain. That is an explicit semantic restart, not an unreported cache miss. Theme changes, scroll and Smooth ticks must never cause such a restart.

### 11.3 Markdown: preserve dependency-aware restart

**Touch:** `content/text/markdown.rs`, `content/text/source.rs`, projection immutable-sequence APIs, and Connector semantic execution. [S20], [S21]

Keep the current CommonMark/GFM options and live-table policy. Preserve:

- reusable stable prefix and replaceable unstable tail;
- `required_restart_from`, reference-definition context and restart checkpoints;
- nesting/source-map validation;
- incomplete streaming input handling;
- seal finalization;
- list tightness, code fence/language, tables, links/images, annotations and source formats.

Replace cached owned prefix equality checks with revision/range identity only when Source generation and append-only invariants prove the prefix unchanged. A hash alone is not proof, and replacement must invalidate it. Retention-incompatible Markdown remains rejected unless checkpointed restart has actually been implemented and explicitly added to the feature contract.

Share immutable cached spans instead of cloning `CachedDomain` and its `Vec<TextContent>` payloads. Make `prepend_cached` and final projection assembly combine shared sequence segments or borrowed iterators. Do not deep-copy the stable prefix merely to add one unstable block.

The current stability calculation may parse a candidate prefix to establish safety. First cache/reuse its result and remove redundant source copies. Do not eliminate a correctness proof merely to reduce parser invocation counts. Replace it only with an equivalent dependency/checkpoint rule verified against the full parser across every input prefix.

### 11.4 Diff: preserve both semantic roles and line state

For the unified-diff Funnel, keep the current role classification, displayed markers, no-newline metadata, CRLF handling, malformed-input preservation, Source provenance, and host theme keys. [S38]

Move `in_hunk`, completed line segments and the unfinished line into Connector-local semantic execution. On append, resume from the affected trailing line with the correct preceding hunk state. Do not parse each incoming chunk independently; chunk boundaries are not line or hunk boundaries.

A line prefix can change meaning while incomplete. Keep it in the unstable tail until the current semantics permit stabilization. Replacing the Source or changing content generation restarts execution. Theme recolor reuses semantic diff output.

Do not unify the typed static Diff API by throwing away its numeric metadata; §6.6 governs that path.

### 11.5 ANSI: incremental state without escaping the renderer

For ANSI, retain the current parser's supported display intent and safe control consumption. Incremental execution needs the current SGR state, hyperlink state, unfinished escape/control sequence, Source ranges, and complete semantic output prefix. [S39]

On append:

1. Feed the retained incomplete suffix plus new bytes, not an independently reset ANSI state.
2. Consume SGR and permitted OSC 8 into semantic style/link intent.
3. Consume or diagnose unsafe cursor/window/device operations; never forward their bytes directly to terminal output.
4. Retain incomplete sequences for the next Source revision according to current behavior, including seal handling.
5. Emit source-witnessed runs and hard-break semantics without reparsing completed safe prefixes.

Test every split position through ESC/CSI/OSC sequences, both BEL and ST terminators, resets, extended colors, hyperlinks enabled/disabled, and ordinary multibyte UTF-8. Byte-valued control detection must not accidentally interpret a UTF-8 continuation byte as an independent C1 control. The current byte scanner warrants a characterization test here; any demonstrated safety/correctness repair must be explicit, not buried in the refactor.

Do not expand the supported ANSI command set as part of the performance tranche.

### 11.6 Annotation rewriting

The existing rewriter splits runs and overlays semantic styles/tags. Replace repeated all-annotation scans with the Source-range index from §9.7, but preserve run ordering, annotations on literal/derived/synthetic content, and the current traversal behavior.

Precompute relevant interval cuts once per run or contiguous semantic region. Borrow unaffected runs; allocate changed split records only where an annotation actually intersects. Do not mutate shared stable IR in place.

### 11.7 Semantic regression oracle

Use the same authoritative grammar/semantic implementation in fresh-state mode as the reference for incremental execution. This is not resurrection of the removed structural decoder.

For every corpus input and every supported chunk partition, compare:

```text
incremental semantic execution through prefix N
    == fresh authoritative parse of complete prefix N
```

Compare text, roles, attributes, links, Source ranges/provenance, diagnostics, stable frontier and final seal result as applicable. Compare physical output separately; matching pixels alone can hide lost links, styles or source coordinates.

---

## 12. Smoothing: remove repeated work without changing current behavior

### 12.1 Record the semantic discrepancy explicitly

The v5 destination is semantic parsing → semantic delivery frontier → physical projection. Current `application/content.rs` obtains smoothing units from Source grapheme spans, runs `Smooth`, then uses a unit count to mask a rendered surface. The generic `Smooth` itself publishes whole stable projection spans using configured weights. [S19], [S24]

This project must not silently replace that behavior with different visible Markdown prefixes or different backlog/seal timing. First characterize it with deterministic clock fixtures. There are two separate deliverables:

- **Now:** efficient execution with current externally observable delivery semantics.
- **Later v5:** a deliberately specified semantic-frontier policy, using the same Connector-owned delivery seam.

Do not put a v5 behavior flag into production merely to preserve two permanent smoothing implementations. The current policy remains the single policy until the later migration intentionally changes it.

### 12.2 Current-policy delivery index

Build the current policy's unit/provenance index when Source/semantic input changes. Reuse it on ticks. Separate ingestion of new weighted spans from advancing the native clock.

A useful internal API is:

```text
delivery.accept_input(input_stamp, stable_units, source_generation)
delivery.advance(now) → changed visible frontier / next deadline
```

`advance` must not reconstruct raw Source projections, call a parser, rebuild semantic blocks, lower a new View tree, or copy the entire content surface. Keep the current `SmoothConfig` rate calculation, first-publication behavior, tick interval, source replacement handling and seal behavior unless an explicit correctness change is approved.

Grapheme boundaries can cross Source chunks and can extend the previously unstable tail. Do not assume a fixed number of trailing bytes is enough for Unicode segmentation. Preserve the existing boundary behavior, reindex the affected tail, and retain sufficient segmentation context.

### 12.3 Painting a reveal window

Replace `reveal_surface`'s full clone with a prepared visibility description consumed by terminal painting. It may be a frontier over cached paint units/rows for the current policy. Painting iterates only units relevant to the allocated visible window and skips/clears unrevealed units using the existing glyph-safe compositor.

Keep revealed height/extent semantics identical. Width changes may require rebuilding the width-dependent unit-to-row mapping, but not reparsing Source syntax. A theme change resolves the same units with new paint style.

Separate the Connector's latest clock state from the visible frontier installed by a committed frame. Failed frame preparation or backend presentation cannot make readback claim a frontier that was never presented.

### 12.4 Scheduler integration

Use the existing native clock/scheduler. Keep a due-Connector worklist or deadline structure rather than scanning every inactive Connector each tick. Update it on selection, mount/unmount, disposal, input/seal change and new deadline. Remove entries when no work remains.

A tick with no frontier change schedules no frame. An inactive Connector has no active deadline. No new per-Source JS callback or timer architecture is allowed.

**Gates:** deterministic current-policy trace parity, independent immediate/smoothed Connectors on one Source, no parse/lower/full-surface-copy on a pure tick, zero TypeScript transport on native ticks, no timer leak on unmount/disposal, and no stale visible-frontier commit after failure.

---

## 13. Current terminal content lowering, measurement, paint, and History

### 13.1 Do not write a second general-layout engine

`TextRenderer` currently lowers semantic blocks into the same native View/layout primitives used by the custom engine. The low-risk pre-v5 destination is a **cached, private terminal content layout model built directly with internal factories**, not a new public View DSL and not a second independent Markdown layout engine. [S22], [S23]

There may still be a native derived node tree because that is the input the current compiler consumes. It is a downstream content product, not the application structural DAG and not a TypeScript structural publication. Count its construction separately. This distinction must not be used to hide rebuilding the entire content tree on every append or tick.

Replace the public `Renderer<[TextContent]>` path with an internal lowering function accepting borrowed iteration over semantic contents. Reuse unchanged block-level layout nodes. Build only changed semantic blocks/unstable tails, and use persistent edge updates for their containing sequence.

Keep policy-dependent context in cache keys. For example, the current inter-block gap depends on the preceding block and list tightness/kind. Caching a block solely by its own identity can produce the wrong leading padding after a neighboring block changes. Include the small predecessor/leading-spacing context or separate that spacing from the cached body.

Move the established formatting policies unchanged: soft breaks, block gaps, table sizing/gaps, task markers, code labels/languages, code wrapping, links/images, heading/list facts, and synthetic decorations. Do not redesign the visual language during direct-lowering work.

### 13.2 Reuse layout products at stable width

Keep a per-Connector/current-terminal layout product keyed by semantic content identity, relevant delivery geometry, width, wrap/alignment and text measurement environment. Preserve the existing compiler's intrinsic measurement and wrapping algorithms.

Avoid constructing a fresh compiler/cache universe for each theme change or Smooth tick. Reuse layout/row indexes while their dependencies match. A width change invalidates width-dependent products, not Source storage or semantic parsing.

Separate two revisions:

```text
content layout-input revision: leaf needs projection/measurement evaluation
content metric revision: projected intrinsic dimensions actually changed
```

A Source update must first mark the leaf's layout input dirty. After evaluating its current constraints, compare metrics and propagate to ancestors only when their dependency requires it. Do not reuse an old metric revision to skip the very projection that could change those metrics.

Likewise, a content paint revision can change while metrics remain identical. It still damages the ContentHost's visible region.

### 13.3 Replace full Surface return with a prepared row-window paint contract

The target `ContentProvider` interface consumes a prepared ticket and a read-only final viewport/clip description. Its paint operation writes into the host candidate surface using the current compositor, or supplies borrowed row/run iteration to that compositor. It must not require cloning a full content surface and cropping it afterward. [S22]

Conceptual shape:

```rust
// Internal interfaces; names are proposed.
struct ContentWindow {
    first_row: u64,
    row_count: u32,
    // Current terminal coordinate/clip values remain terminal-specific.
}

fn paint_prepared_content(
    ticket: &PreparedProjectionTicket,
    window: ContentWindow,
    target: &mut CandidateSurface,
    context: &TerminalPaintContext,
);
```

The actual interface must also account for content origin, horizontal clipping, allocation and inherited occurrence style/facts. Derive those from candidate placement, not from a second scroll offset stored on the Port.

Do not add a dynamic trait call per cell. Dispatch once per content leaf or prepared block, then use typed/monomorphized inner loops. Borrow cached semantic runs and resolve styles through the host cache.

This work can be staged: first cache compiled rows and eliminate full-surface copies; then materialize only the visible window. Initial full width-dependent row indexing may remain for correctness, but an ordinary scroll must not reparse or rewrap all content. The final gate forbids full offscreen surface allocation merely to show a small viewport.

### 13.4 Preserve physical rendering invariants

Use the current glyph-safe surface operations. In particular:

- clearing a cell intersecting a wide glyph must clear the whole glyph span;
- a continuation cell must never become an orphan;
- transparent content reveals the correct retained ancestor background;
- border, padding and content rectangles remain distinct;
- old and new rectangles are damaged for movement/resize/removal;
- clipping applies after the correct origin transform;
- local repaint must include siblings/ancestors when overlap or inherited style makes isolated painting unsafe.

Do not require rewriting terminal diff emission. A complete final host surface may still be diffed by the existing backend. The optimization is eliminating redundant *content-sized* surfaces and unnecessary layout/paint work, not claiming that an entire host frame can never be copied for safe in-flight ownership.

### 13.5 Scroll and follow ownership

ScrollPane/RowViewport keeps offset, follow intent, clamping and anchors. Port supplies destination identity/allocation. Connector consumes read-only viewport context.

A Connector switch must preserve viewport intent. Clamp to the new extent only in the candidate controller state that commits with the new content. Failed activation keeps the old projection and viewport basis. Manual scrolling must not be overridden by a content append unless the current follow policy says so.

For a ContentHost without a viewport ancestor, derive the default window from its own allocation/clip. Do not make every content leaf own a new scroll controller.

### 13.6 History is a terminal adapter, not a new content execution owner

Current Port records contain History-specific source/frontier/padding state. Extract those fields into a terminal History-binding adapter keyed by the existing unit/Port identity. Keep the current public History behavior and ownership until the later v5 migration.

The adapter consumes prepared/committed content products; it owns no separate Source, parser scheduler, or mutation transport. Preserve:

- only transferable stable/complete rows are offered;
- whole-grapheme and width constraints;
- content rows versus leading/trailing decoration padding;
- partial sink acceptance;
- frontier advancement only after an actual sink receipt;
- no duplicate rows after retry;
- correct retirement only after all required rows/metadata have been accepted;
- theme/style behavior for still-resident content;
- sealed/open and blocked-transfer distinctions.

Eliminate `surface_suffix` by starting row iteration at the accepted frontier. Eliminate full Source-prefix byte copies by representing the prefix as an immutable range view over shared Source storage.

Be careful with `stable_prefix()` replacement. The current code may render a finalized prefix differently from the open full semantic document. It is not valid to replace that with “take all rows before the last newline” or with an unproven slice of an open Markdown projection. Initially cache the current prefix-render policy by Source range, parser mode, width and relevant presentation inputs. Reuse its bytes/semantic products without copying the Source. Replace the prefix proof algorithm only after differential History fixtures establish equivalence.

Native scrollback already accepted by an external terminal is not a resident editable surface. This project must not promise it can recolor old physical scrollback. Still-resident History content must retain its existing correct invalidation behavior; v5 later changes the application Surface model.

---

## 14. Frame transaction, dirty work, wake routing, and errors

### 14.1 Keep desired and visible authority separate

H3 prepare validates every ordinary permanent structural/attachment error. Desired commit accepts that root and advances pending work. Frame preparation may fail later; the accepted desired root remains authoritative while the last complete visible frame remains committed. [S29], PERF-13 Part I §§3, 6

No factory optimization may install a node directly into the visible scene. No retained-state or content method may call the structural dispatcher to force a frame.

### 14.2 Candidate data must be changed-record data

The frame candidate should own:

```text
captured host epoch and desired structural revision
prepared attachment/binding changes
changed/newly demanded state versions
one captured snapshot per demanded Source
prepared Connector selection and projection tickets
candidate layout/viewport changes and damage
candidate surface/backend update
preallocated visible-commit records
```

Avoid cloning all Source/Port/Connector/state registries to create isolation. Use immutable products plus a journal/overlay for touched mutable associations. Do not journal raw references whose backing records can be reclaimed; prepared leases pin identities until commit/abort.

There is one candidate in flight under the existing host model. New operations accepted while terminal presentation is pending remain in pending state and are not accidentally folded into the older candidate's commit.

### 14.3 Convergence algorithm

For one captured attempt:

1. Select the latest desired structural root through the attempt boundary.
2. Install its prepared occurrence/attachment changes in candidate storage.
3. Capture/apply effective state and control values through that boundary.
4. Capture required Source snapshots and relevant host environment/theme data.
5. Prepare demanded content under offered constraints; record projection tickets.
6. Measure affected leaves/subtrees, compare metrics, and propagate through current dependency metadata.
7. Place affected roots and derive viewport windows/clip/follow results.
8. Re-evaluate only content/layout dependencies whose offered constraints actually changed.
9. Stop when the candidate's relevant keys and geometry are stable. Preserve the current defensive convergence ceiling and report the cause of a cycle.
10. Compute damage, paint the candidate surface, and submit the current backend update protocol.
11. After a successful receipt, install only that candidate's visible products and captured epoch.
12. Publish observations and release superseded owners outside critical commit work; newer pending work remains pending.

An operational candidate-Connector failure uses the previous committed Connector according to §10.5. A host/backend failure leaves the whole logical visible transaction uncommitted.

### 14.4 Do not let commit allocate or rediscover validity

`ContentHostRegistry::commit_visible` currently builds sets/maps and scans registries. Move its next binding/selection tables and affected-record lists into frame preparation. [S25]

At commit, apply prevalidated association changes and swap prepared ownership. Do not re-run node-kind compatibility, resolve handles, parse config, invoke user callbacks, or build an unbounded temporary collection.

Prepared capacity must remain valid while newer work is accepted. Prefer candidate-owned next visible tables or reserved touched-entry storage rather than assuming spare capacity in a shared mutable table cannot be consumed before receipt. Do not overwrite newer requested selection/state while promoting an older visible result.

Locking and record access at commit must follow the host's serialization discipline. A supposedly infallible commit cannot depend on taking an independently poisonable lock that was never accounted for during preparation.

### 14.5 Replace global content invalidation incrementally

Replace the single host-wide content dirty/cache-clear behavior with typed dirty reasons and affected Port/Connector IDs:

```text
Source/semantic input changed
Delivery visibility changed
Width/measurement environment changed
Presentation/theme changed
Viewport window changed
Selection/mount lifecycle changed
```

Map the demanded Port to the candidate/committed occurrence index. Mark the relevant leaf, then propagate metrics/placement and damage using the current layout dependency machinery. Preserve a conservative same-engine escalation when metadata is missing or the change escapes a proven boundary.

Do not globally flush layout caches on a pure theme or viewport change. Do not clear every Connector's semantic cache because one Source advanced. A full initial mount or broad resize can legitimately visit a large tree; a single fixed-allocation content change should not automatically do so.

### 14.6 Source-to-host wake routing

Keep native Source subscriptions authoritative. TS receives a wake hint, not a Source→Connector→Host fan-out graph.

Improve the current temporary linear grouping by maintaining host-grouped eligible subscriber records or collecting into reused indexed scratch keyed by host identity. Update groups on committed/requested subscription transitions. Validate weak generations/liveness when draining.

Never hold the Source mutex while acquiring a host frame lock. Do not release necessary source membership merely because a Connector is inactive; membership and wake subscription are different lifetimes.

Preserve the environment latch, fair pending-host drain, blocked-retry handling, and clear/recheck race safety. A failed attempt at the same epoch must not rearm an infinite microtask loop. A new independent mutation/readiness signal or explicit barrier can make it runnable again.

### 14.7 Error mapping

Replace success-path string construction and diagnostic-string matching with internal typed error/status variants where those paths are touched. Format diagnostics at the language boundary or on a recorded error, not on every successful operation.

Preserve separate classes:

- rejected caller input: no accepted mutation;
- structural prepare rejection: old desired and visible roots remain;
- Connector operating failure: requested failure status, previous Connector retained;
- retryable frame/backend failure: desired work remains pending;
- invariant/poison failure: explicit fatal/poison policy;
- partial physical terminal I/O: surface synchronization/recovery required.

Do not collapse everything to `INTERNAL_INVARIANT`, change typed codes by string wording, or turn a post-acceptance wake failure into a retryable payload rejection.

For per-frame drain reports, use typed N-API records or a generated compact result envelope with reusable storage. Keep rare human diagnostics/readback appropriately typed rather than forcing every string into a bespoke numeric protocol. Public readback still has to allocate returned strings when requested; that cost is not a hidden per-frame payload architecture.

### 14.8 Physical I/O and History receipts

Logical frame atomicity does not make terminal writes reversible. The existing SceneHost can perform receipt-driven native History transfer while preparing the next scene. Accepted external rows must not be emitted again if a later paint/presentation step fails. [S30]

Preserve separate accounting for irreversible sink receipts. A candidate abort must not rewind an already acknowledged History frontier as though no physical write happened. When terminal output is partially applied, mark synchronization state unknown and force the existing recovery/full repaint protocol. Do not claim the old physical screen is intact merely because the logical frame pointer did not advance.

Any attempt to reorganize History side effects into a new backend transaction must be a separately tested change; it is not an incidental consequence of replacing `Arc<Surface>`.

### 14.9 Native controls and application kernel

The native host uses `RunningApp`, component mounting, focus/action routing, TextInput, ViewSlot, ScrollPane, History, outputs and terminal timing. Public Rust authoring removal must not delete these implementations while they still power TypeScript features. [S29]

Migrate built-in view construction to the internal node factory. Keep cycle-boundary ViewSlot animation semantics, TextInput cursor/selection/paste behavior, native focus/focus-within facts, deferred component retirement, async output ownership and terminal restoration.

The eventual internal host kernel can be smaller than the public generic `App` driver, but only remove generic driver code after dependency inspection proves the native host no longer uses it. Public visibility removal can happen earlier without changing the runtime kernel.

---

## 15. v5 adaptation seams and deletion map

### 15.1 Build seams, not future backends

| Seam introduced or clarified now | Current implementation | Later replacement |
|---|---|---|
| Structural ingress → canonical retained node/attachment inputs | Current semantic DAG materializers and persistent derivations | React occurrence transaction ingress |
| State values → semantic effects | Current `ViewState` schema and Rust classifier | Broader v5 declared-base/override property schema |
| Semantic effects → layout invalidation | Current custom-engine dependency adapter | Taffy invalidation/cache integration |
| Semantic content → backend content layout | Private cached terminal node/row model | Taffy terminal measurement and GPUI text/scene lowering |
| Connector delivery → visible content selection | Characterized current Smooth policy | v5 semantic-frontier delivery |
| Content destination → viewport context | Current ScrollPane/RowViewport | Surface viewport/residency demand |
| History transfer adapter | Current receipt-driven native scrollback | Deleted/replaced during component Surface migration |
| Paint environment inputs | Current theme/terminal realization revisions | Host Environment relevant subrevisions |

Only one implementation of each current responsibility is authoritative in production. There is no permanent `old/new` runtime selector.

### 15.2 Do not freeze current terminal types into portable contracts

Cell `u16` geometry remains valid inside the current terminal engine and current private wire schema. It must not become the future cross-backend semantic content representation. Semantic text contains Source ranges/roles/structure, not terminal rows or colors already quantized for one host.

Similarly, no Taffy `Style`, GPUI object, terminal escape code, or backend pointer enters the retained semantic Source/Funnel schema. Future host-specific measurement and paint products may have different representations.

### 15.3 What is deleted now versus later

**Delete in this project:** the supported Rust UI-authoring facade/prelude; public DSL construction dependencies on the native hot path; duplicate wire-to-public-type staging where not the final storage shape; builder-created intermediate roots; duplicate annotation-record vector; repeated full-text rehydration; redundant content surface clones; per-frame whole-state snapshot copies; superseded helpers and their obsolete compatibility tests/docs.

**Keep internally now:** canonical retained nodes/edges; current custom layout and glyph/paint algorithms; semantic text/parser/provenance algorithms; required component/host kernel; current History and viewport semantics; generated conformance surface; necessary immutable ownership references.

**Delete only during later v5 migration:** the custom general-layout engine, special History model, current TypeScript View composition as canonical API, and current delivery-policy semantics when their replacements are explicitly accepted.

### 15.4 LOC policy

Track added/deleted production LOC by tranche and separately count generated code, tests, fixtures and documentation. A rename does not count as deletion. Moving a facade under `internal` does not count as removal of authoring complexity.

The high-confidence deletion opportunities are wrapper layers, chained builder helpers, duplicate staging and serialization code. Storage/index and regression-test work may add code. Do not claim a net reduction before measuring the finished diff, and do not delete correctness machinery merely to achieve a numeric LOC target.

---

## 16. Repository change map

Paths below are current owners unless explicitly marked **new**. Use the symbol as the primary anchor and read its complete callers before editing.

| Owner/path | Concrete changes | Must preserve |
|---|---|---|
| `crates/iyon-tui/src/lib.rs`, core `Cargo.toml`, `ARCHITECTURE.md` | Remove authoring exports/prelude and publication promise; introduce narrow binding facade; amend obsolete Rust/TS parity rule | Framework/app separation; TS package contract |
| `crates/iyon-tui/src/presentation/ir.rs` | Final node assembly including attachments; consuming sequence construction; single-root derivations; attachment uniqueness work | Native identity, subtree flags, persistent sharing, all kinds |
| `presentation/api/{text,style}.rs` | Extract/pass through passive records; remove production DSL use; shared text ownership | Inline/page-slice text, sparse styles, glyph semantics |
| `presentation/api` composition/grid helpers | Extract actual normalization algorithms, then delete unneeded public construction wrappers | Track/cell placement, Hanging/Clamp/container semantics |
| **new** `presentation/node_factory.rs` and `binding/*` | Typed direct construction/operations | One canonical runtime representation |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | Text/grid/axis/decorated direct lowering; lease handling; keep cache-first behavior | Reference namespaces, errors, staging/abort, expressiveness |
| `crates/iyon-tui-native/src/tui/view_state.rs` | Replace Value/property-string parsing with generated state records | Exact public method behavior, patch atomicity |
| `retained_state/{record,registry,occurrence,capabilities,geometry,presentation,effects}.rs` | Changed-version capture, base/effective ownership, native effect mapping | Clear/null/remount/lifecycle, capability exhaustiveness |
| `application/view_state.rs` | Remove repeated ID locks; unify host-serialized access where proved | Lock order, stale/disposed behavior |
| `theme/mod.rs`, native `tui.rs` theme helpers | Direct immutable theme assembly; tagged styles; typed selectors; batched variant ordering | Every selector field, specificity/declaration order |
| `crates/iyon-tui-native/src/content_ffi.rs` | Borrow/iterate annotation records; typed errors; truthful acceptance; shared schema checks | Exported content v1 layout, same image, pointer lifetime |
| `application/content.rs` | Separate storage/execution/terminal adapter responsibilities; tickets; dirty worklists; prepared commit records | Source/Port/Connector ownership and all lifecycle transitions |
| `content/text/source.rs`, raw/provenance records | Page/range-backed input; retained parser working region; no prefix/suffix deep copies | Exact/derived/synthetic witnesses |
| `content/text/markdown.rs` | Immutable cache sharing and dependency-keyed incremental work | Current parser options and stability/restart correctness |
| `content/text/{ansi,diff}.rs` | Connector-local incremental context and reusable semantic prefix | Safe controls, line/role/link/provenance semantics |
| `content/text/render/{mod,block,inline,identity,policy}.rs` | Replace authoring calls with internal factories; cache block lowering; borrow contents | Current formatting and context-sensitive spacing |
| `projection/{value,smooth,validate,projector}.rs` | Borrow/share projection segments; separate delivery ingest/tick where needed | Projection algebra, span/frontier validation, current rate policy |
| `presentation/content.rs` | Prepared content ticket/metrics and row-window paint boundary; isolate History hooks | Measurement/paint consistency, no Source/control mutation |
| `scene/host.rs`, `application/host.rs`, current layout/paint callers | Targeted content/state invalidation; candidate overlays; preallocated commit; borrowed paint | Convergence, in-flight epochs, receipts, damage, controls |
| `packages/iyon-tui/src/transport/state/control.ts` | Schema normalization directly to compact records | Public TS typing, validation, undefined/null/clear |
| `transport/structural/{retained-dag,native-view-abi,encoding}.ts` | Only required encoder/factory-contract changes; preserve identity/reuse path | H3 ownership, scratch lifetime, current optimized lanes |
| `transport/native/{addon,resources,resource-registry}.ts` | Update private contracts; keep shared resolver authority | Framework handles, prepared/desired/visible leases |
| `transport/content/ffi.ts`, content control, runtime wake owner | Matching private contract/results and native wake hints | No Source payload in structural/state lanes; no JS subscription graph |
| `tools/tui-abi/view_abi.toml`, `tools/tui-abi-gen/src/*` | Schema/model/template changes; regenerate all outputs | Checked bounds, hashes, feature surfaces, conformance |
| `tools/ownership/check.ts`, API surface tooling and fixtures | Replace obsolete Rust facade snapshot with narrow integration allowlist and deletion guards | Do not weaken TS/public/plane-purity checks |
| `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts`, `perf13_h_content.ts` | Focused workload/counter additions | Authoritative route assertion, no deleted decoder/oracle |

This table does not authorize deleting a file because its directory contains `api`. Trace remaining internal use first. Some passive types and established algorithms currently live under that directory and must move before the wrappers disappear.

---

## 17. Implementation tranches — execute in this order

A tranche is not complete because it compiles. It is complete when its behavior, ownership, route and focused performance gates pass and its superseded production path is deleted. Each completion report uses §20.

### L1-00 — Freeze behavior, route, and cost baselines

**Depends on:** nothing.  
**Owners:** tests, focused benchmarks, ownership gate, source census.  
**Goal:** make it impossible to confuse fewer wrappers with preserved functionality or measured speed.

Steps:

1. Pin the starting SHA and record Bun/Rust versions, target, build features, native artifact path/hash and terminal/headless mode.
2. Run the current verification commands in §19 before changing implementation. Record baseline failures precisely; do not pre-authorize the cleanup report's old environmental exceptions.
3. Record the actual Rust external export set and every native import from the core crate. Separate authoring APIs, passive values, host/runtime integration and test-only hooks.
4. Record TS public declarations and the consumer-fixture results. These are the compatibility surface that remains frozen.
5. Add a route-preserving trace harness covering structure, state, content and native time. Use the existing headless/native host, not a mock renderer.
6. Add counters for deep copied bytes/metadata/cells, node construction, state snapshot visits, Source snapshot acquisition, semantic parse work, content lowering, row layout, global cache clears and registry scans. Reuse the existing perf reporting mechanism; do not log strings from hot loops.
7. Capture baseline traces and focused workload results from §19. Store inputs and expected semantic/physical outputs with provenance.
8. Add characterization fixtures for the identified risks: state/structural patch distinction, current Smooth prefixes/timing/seal, stable History-prefix output, Source post-acceptance wake failures, counter-exhaustion atomicity and UTF-8/ANSI scanner boundaries.

**Delete:** no production algorithm yet.  
**Stop:** the baseline route, source commit, artifact and expected outputs are reproducible. Every claimed defect is either reproduced or clearly marked an unconfirmed inspection risk.

### L1-01 — Establish the unsupported internal binding contract

**Depends on:** L1-00.  
**Owners:** core manifest/lib, new `binding` facade, native imports, ownership docs/tooling.

Steps:

1. Set the core crate's `publish = false` and document that Rust is runtime implementation, not a supported authoring package.
2. Add the narrow `binding` namespace from §5. Export only the current runtime operations/passive types actually required by native callers. Do not export `prelude` or blanket-export old modules.
3. Change native imports to this seam without changing implementation behavior yet.
4. Add an explicit allowlist/check for the binding surface and a forbidden list for public authoring exposure. Keep TS declaration and framework-purity checks intact.
5. Amend `ARCHITECTURE.md` where it requires public Rust/TS authoring parity. Replace that with TS public compatibility plus internal ABI/runtime conformance.
6. Create a checked migration ledger for remaining internal uses of fluent constructors. The ledger names each production owner; it is not a permanent compatibility allowance.
7. Keep Rust unit tests inside the core module tree where they need private access. Plan migration of external integration tests to the binding or transport fixture, not a permanent public authoring test facade.

**Delete:** public authoring support promises and newly unnecessary re-exports as their native callers move. Final DSL deletion occurs in L1-13.  
**Stop:** the native addon and current tests work through the new seam; no additional representation or runtime registry has been introduced.

### L1-02 — Final node/common-field construction

**Depends on:** L1-01.  
**Owners:** `presentation/ir.rs`, new node factory, native common/decorated/attachment constructors.

Steps:

1. Extend final node parts to include attachments and all common fields.
2. Introduce one final-node construction function using the existing canonical allocation/identity machinery.
3. Replace common property chains with local candidate field assembly followed by one final root.
4. Replace decoration construction in the exact precedence order of §6.3, including custom glyphs and sparse style overlays.
5. Construct state/content-attached nodes directly; avoid allocating a root and immediately replacing it just to attach an ID.
6. Preserve subtree aggregate flags and semantic equality behavior. Add tests showing child flags/attachment presence remain accurate after a derivation.
7. Replace quadratic attachment duplicate lookup with candidate uniqueness tracking while retaining occurrence expansion and cycle detection.
8. Delete migrated modifier helper calls from these ingress implementations.

**Stop:** all decoration/base/attachment goldens and failure cases pass; a new decorated node no longer creates one intermediate root per field; exact-root/NodeId hits do no payload work.

### L1-03 — Text storage and direct text constructors

**Depends on:** L1-02.  
**Owners:** native text helpers, internal text storage/factory, generated string adapter where needed.

Steps:

1. Add direct final TextView construction with wrap/alignment initialized before root allocation.
2. Preserve small inline text and share a native page across length-delimited spans.
3. Remove per-span String rehydration where a checked page range is sufficient.
4. Cover one-to-four fixed lanes and the N-span buffer lane with the same factory and validation semantics.
5. Benchmark the current N-API CString round trip separately. Replace its owned-string recycling only through generated adapters to the same semantic factory.
6. Add byte-copy counters and lifetime tests retaining old roots after subsequent construction.
7. Delete `text_view_from_spans`' authoring chain and redundant text wrapper helpers once their callers are migrated.

**Stop:** text/span/NUL/Unicode/invalid-input parity; no copy of already native-owned text merely to apply layout metadata; no loss of the small-text fast case.

### L1-04 — Persistent axis/grid and remaining node kinds

**Depends on:** L1-03.  
**Owners:** persistent sequence, axis/grid normalizers and native materializers, static diff lowering, built-in node constructors.

Steps:

1. Make consuming sequence construction move items and branch handles rather than clone slices.
2. Make axis decoding construct final child records in one pass; preserve staged axis builder limits and cleanup.
3. Extract the current grid placement/normalization algorithm from the public builder; move parsed cells into final storage.
4. Preserve native axis set/splice and grid set-cell path sharing; avoid flattening complete child sequences.
5. Make the specialized edit trie build each changed parent once where feasible, while preserving all required rewritten NodeId records and its current transaction semantics.
6. Migrate Hanging, Container, ClampRows, RowViewport, Spacer, ComponentSlot and ContentHost construction to the same internal factory.
7. Migrate static Diff formatting without losing numeric hunk/line/termination metadata.
8. Update built-in Rust producers to the direct factory as encountered; keep a complete kind/constructor coverage table.
9. Delete closure-based builder replay from native materializers; do not delete the specialized wire lanes or add universal batching.

**Stop:** every currently expressible TS View renders through the retained path; incremental edits equal fresh retained publication; large axes/grids do not regress to full-copy edits; malformed tail data leaves no publication/lease leak.

### L1-05 — Typed retained-state ingress

**Depends on:** L1-02; run after L1-04 for a simple linear delivery sequence.  
**Owners:** state schema, generator, TS state transport, native `view_state.rs`.

Steps:

1. Encode every current geometry/presentation field in the finite property schema, including nullability and clear semantics.
2. Generate TS encoders and native decoders with complete mask/value/range validation.
3. Replace Value-based geometry/presentation calls and property-string clear lists at the private boundary.
4. Preserve public TS validation errors, including unknown keys, duplicate clear keys, `undefined` omission and invalid glyphs.
5. Return a primitive/compact wake result on success.
6. Keep dynamic style-state key/value operations typed and separate.
7. Verify actual state operations never enter structural `view_*_patch` functions.
8. Delete superseded dynamic parsers/transport object construction after parity tests pass.

**Stop:** state mutation is direct to overrides; no semantic View construction or structural transport; exact null/clear/base/remount behavior; no partial invalid patch.

### L1-06 — Changed state versions and host-owned access

**Depends on:** L1-05.  
**Owners:** state record/registry/occurrence, host overlay capture, native wrapper access.

Steps:

1. Add dirty-state deduplication and immutable changed versions.
2. Replace full registry snapshot creation with one candidate overlay over the committed version table; adapt consumers to lookup through it without materializing a full map.
3. Capture newly attached/remounted states even when they were not recently mutated.
4. Store immutable base references where that removes repeated maps/decoration copies; keep cheap scalar fields direct.
5. Remove redundant ID locking and repeated validation passes.
6. Audit record access serialization. Move proved host-only records behind the host owner; retain required locks for the others until their access is unified.
7. Prepare binding changes before commit and preserve in-flight pins.
8. Remove the old whole-registry snapshot path from production.

**Stop:** one paint-state mutation does not visit/snapshot unrelated unmounted states; failed frame and delayed receipt retain correct old versions; clear after masked base update works; no stale identity revival or disposal race.

### L1-07 — Semantic style/theme normalization

**Depends on:** L1-05 and the direct node factory.  
**Owners:** style records/atoms, Theme, native theme helpers, TS private style encoders.

Steps:

1. Define/share the tagged semantic color and sparse attribute representation.
2. Replace repeated textual color parsing on hot structural/state materialization with validated typed values or semantic atom references.
3. Construct immutable theme tables directly, preserving duplicate selector updates and declaration order.
4. Generate/validate all current text-selector fields, not only roles/colors.
5. Share immutable theme data into frame/content contexts rather than clone its maps.
6. Split palette/presentation revisions from semantic and measurement dependencies.
7. Define interner ownership/reclamation and test unique-value growth.
8. Delete superseded theme/style JSON helper chains only after all live callers, including TextInput border and state style ingress, are migrated.

**Stop:** full style/selector/theme parity across two hosts; theme changes do not reparse semantic content; no geometry change from a presentation-only theme; no unbounded interner leak.

### L1-08 — Source FFI and persistent storage

**Depends on:** L1-00 baseline, L1-07 shared style vocabulary where annotations use it.  
**Owners:** content FFI, Source storage/mutation/annotations, content ABI metadata.

Steps:

1. Remove the duplicate annotation DTO vector via a borrowed reader/iterator.
2. Introduce validated input views and eliminate repeated adjacent UTF-8/line scans where possible.
3. Add persistent chunk/index roots and page-range slices; keep old snapshots immutable.
4. Implement append/replace/truncate algorithms from §9, including preflight counter arithmetic.
5. Preserve annotation order, truncation policies and source witnesses with indexed range access.
6. Keep current limits and strict seal/clear behavior.
7. Fix any reproduced post-acceptance error ambiguity as an explicit correctness commit.
8. Update shared metadata/schema checks and test the real same-image symbols.
9. Delete the old whole-deque/vector COW mutation implementation once the persistent path passes.

**Stop:** retained old snapshots plus repeated tiny append do not trigger full surviving-metadata copies; no Source byte copy on snapshot; retention and annotations are atomic; rejected operations leave all authoritative state unchanged; real FFI lifetime tests pass.

### L1-09 — Semantic execution and parser reuse

**Depends on:** L1-08.  
**Owners:** Source raw projection adapter, semantic IR/projection sequence storage, RawDomain, Markdown/Diff/ANSI execution.

Steps:

1. Replace raw chunk `to_owned()` reconstruction with native page/range-backed input.
2. Make semantic projection assembly share unchanged sequences and borrow values for traversal.
3. Give parsers retained working regions and explicit Source stamps/restart context.
4. Remove deep Markdown cached-prefix/span reconstruction while preserving all reference/stability logic.
5. Add incremental diff line/hunk state and ANSI escape/style/link state.
6. Index annotation rewriting; preserve exact/derived/synthetic provenance.
7. Test every prefix/chunk partition against fresh authoritative semantic execution.
8. Separate semantic cache keys from theme, width, delivery tick and viewport.
9. Delete obsolete whole-Source raw/domain rehydration helpers from active execution paths.

**Stop:** pure theme/scroll/tick causes zero parser input work; ordinary append cost follows new/unstable input instead of stable prefix size; grammar-required broad restarts are explicit and correct; unsafe ANSI never escapes.

### L1-10 — Delivery state without per-tick reprocessing

**Depends on:** L1-09.  
**Owners:** Connector execution, `projection/smooth.rs`, native deadline integration.

Steps:

1. Pin current delivery behavior with deterministic trace fixtures.
2. Separate delivery input acceptance/indexing from clock advancement.
3. Retain the current unit policy and rate/seal semantics; do not substitute v5 defaults.
4. Cache raw-unit-to-prepared-paint visibility metadata for the current policy.
5. Make ticks change only the Connector delivery candidate/frontier and necessary native dirty state.
6. Maintain due-Connector scheduling without scanning inactive registries.
7. Preserve visible-frontier commit separately from latest execution progress.
8. Delete tick-triggered semantic cache clearing and repeated grapheme-projection construction.

**Stop:** current traces match; native ticks perform no JS transport, parser work, fresh content View lowering or whole-surface clone; cold/disposed timers disappear; two Connectors remain independent.

### L1-11 — Cached terminal content model, window paint and History adapter

**Depends on:** L1-04, L1-07, L1-09, L1-10.  
**Owners:** TextRenderer internals, ContentProvider, current compiler/paint integration, History binding.

Steps:

1. Replace public authoring calls in semantic text rendering with internal factory calls over borrowed semantic iteration.
2. Cache unchanged block lowering with correct predecessor/spacing context.
3. Retain width-dependent layout/row indexes and separate metric from paint revisions.
4. Add prepared projection tickets shared by measure and paint within the attempt.
5. Add the final viewport/clip-aware paint contract and remove full content-surface return/crop requirements.
6. Replace Smooth masking and History suffix copying with visibility/row-window iteration.
7. Extract History-only Port fields/hooks into the terminal adapter without changing public History behavior.
8. Replace copied stable Source prefixes with range views and cached, equivalent prefix proof/render products.
9. Test wide glyphs, clipping, backgrounds, padding, viewport transitions and partial History receipts.
10. Delete the superseded `render_semantic_surface`/surface-copy route once the new path is authoritative. Reusable compiler algorithms may survive behind renamed private functions.

**Stop:** small viewports do not allocate whole offscreen content surfaces; scroll reuses semantic/wrap work; style updates re-resolve paint only; History emits no duplicate/lost rows through failure/retry; no alternate renderer is left behind.

### L1-12 — Targeted frame work, prepared commit and wake/report cleanup

**Depends on:** L1-06 and L1-11.  
**Owners:** SceneHost, HostInner, content registry, environment wake integration, private report contracts.

Steps:

1. Replace global content invalidation with affected-ID dirty reasons and occurrence lookup.
2. Integrate changed content metrics with current layout dependency propagation and convergence.
3. Capture one Source snapshot per demanded Source per attempt and reuse tickets through paint.
4. Prepare next visible binding/selection tables and touched-record lists before backend submission.
5. Make visible commit swaps/assignments only; preserve newer pending operations while a receipt is in flight.
6. Replace quadratic temporary wake grouping and unnecessary registry scans with maintained/reused indexes.
7. Replace per-frame dynamic report assembly where measured with typed/reusable records; keep structured error observability.
8. Preserve no-spin retry blocking, fair drain and lost-wake protection.
9. Inject failures at each phase, including delayed/partial backend receipts and History transfers.
10. Delete superseded whole-registry/whole-cache paths where targeted handling is now proven. Keep explicit same-engine escalation where required.

**Stop:** local content/state updates do not globally clear unrelated caches; all desired/visible/in-flight semantics pass; no lost wake, duplicate acceptance, stale binding, premature disposal or partial logical commit.

### L1-13 — Eradicate authoring residue and accept the result

**Depends on:** every prior tranche.  
**Owners:** whole-repository caller census, exports, documentation, tests, package artifacts, focused acceptance.

Steps:

1. Re-run the complete import/call census. Migrate every remaining production fluent View/Text/Grid/Renderer authoring call to the internal constructors/functions.
2. Remove public authoring exports, prelude, `IntoView` exposure, obsolete Rust extension facade and authoring-only helpers. Keep genuinely necessary internal algorithms/passive data, not compatibility wrappers.
3. Migrate built-in controls and kernel constructors without altering their lifecycle/input/timing semantics. Delete unused standalone Rust application-driver code only after native-host independence is proved.
4. Move necessary private algorithm tests into module tests and transport tests. Remove obsolete public Rust API snapshots; replace them with a narrow binding allowlist and negative export tests.
5. Delete temporary route counters/scaffolding that were only needed for migration, retaining useful performance and invariant counters.
6. Assert absence of old complete-object decode/oracle paths and newly superseded helper paths. Do not count absence regexes themselves as production callers.
7. Run all §19 gates on produced artifacts for supported targets and feature profiles.
8. Compare every focused workload against L1-00, including memory/lifetime slopes and worst-case inputs. Investigate any repeatable regression; do not average unrelated wins over it.
9. Publish the final ownership diagram, surviving passive type rationale, source/ABI versions, benchmark data and production LOC accounting.
10. Check the definition of done in §21 line by line.

**Stop:** one supported TS UI model, one internal Rust runtime, no public Rust authoring contract, one authoritative path per operation, no unexplained behavioral or measured performance regression, and no “temporary” production route left for v5 to inherit.

---

## 18. Mandatory behavior and failure matrix

Use this as a checked ledger. A feature is not preserved merely because its simplest example still renders.

### 18.1 Structural/composition

| Case | Required assertion |
|---|---|
| Exact same root | No semantic field reads, child visits, payload encoding or new native nodes beyond existing authoritative route semantics |
| Live NodeId reuse | Existing cache-first behavior; no payload decoding on the implementation hit path |
| One changed path | Unchanged subtrees retained; changed ancestors rebuilt only as needed |
| Large axis | No old size/depth heuristic selects another architecture |
| Grid tracks/spans/alignment | Layout and cell lookup identical; persistent child replacement does not change placement |
| 1–4 and N styled text spans | Same text/style/width output; no restored four-span restriction |
| Empty text and embedded/trailing NUL | Valid cases preserved; CString and length-delimited contracts remain distinct |
| Custom glyph decoration | All eight glyphs, edge masks, color and precedence preserved |
| Real wrappers | Container, clamp, Hanging, viewport, component and clipping boundaries retain behavior |
| Attached shared DAG subtree used twice | Duplicate occurrence attachment rejected even when semantic node identity is shared |
| Wrong host/environment/stale handle | Deterministic prepare failure; no desired/visible mutation |
| H3 B accepted then frame failure then C accepted | Desired C survives; visible A remains until successful C frame; B can be superseded safely |
| Lease/ref release and slot/reference exhaustion | No revival, premature reclamation or leaked temporary lease |

### 18.2 State and presentation

Test every schema field on every supported and unsupported node class. Include complete-invalid patches, partial field presence, `undefined`, explicit null, clear-one, clear-all, explicit empty clear list and duplicate clear arguments.

Required compound scenarios:

```text
base A → set override A → republish base B → clear override → B is revealed
state unmounted → configure → mount → remount compatible kind → clear
new desired kind + old visible kind during delayed receipt
paint-only patch + simultaneous Source append
border absent → introduced → custom glyph recolor → removed
focus/focus-within change under selector-dependent descendants
```

Assert structural publication counts, state version counts, measure/place counts, semantic parse counts, damage and final pixels. Include old resident History/body occurrences, not only the newest node.

### 18.3 Source/storage/FFI

Exercise block and stream operations independently: append where supported, replace, clear, seal, repeated seal, mutation after seal, truncate, explicit disposal, owner death, and Source survival after all hosts are gone.

Test UTF-8 boundaries, CR/LF/CRLF, NUL, emoji/ZWJ/combining sequences, wide glyphs, zero-length pointer cases, null/non-null buffer rules, invalid counts and alignment, reserved lanes, malformed payload offsets, unknown annotation kinds, operation-local range conversion, and wrong environment/Source generation.

Retention tests must hold old snapshots live while appending/truncating/replacing. Verify old text and annotations remain unchanged, new head/tail/generation are correct, all limits are atomic, partial-head semantics are preserved, and memory releases when snapshots disappear.

Force revision/generation/tail arithmetic near its limit. On normal rejection: bytes, annotations, revision, retained range, membership and wake state must not partially change.

### 18.4 Content transforms and delivery

| Feature | Cases |
|---|---|
| Plain | All supported wrap modes/alignment, width 0/1/narrow/wide, trailing breaks, annotations |
| Markdown | Every input prefix; delayed emphasis/link/reference definitions; fenced code; tables; lists and tightness; images/fallback labels; seal; source replacement; retention incompatibility |
| Diff Funnel | Headers, hunks, additions/deletions/context, no-newline marker, CRLF, malformed input, partial final line, theme recolor |
| Static Diff View | Large safe-integer coordinates, offsets, termination flags and formatting, independent of textual Funnel parsing |
| ANSI | Every sequence split boundary; SGR resets/colors/attributes; OSC 8 on/off; BEL/ST; unsafe controls; malformed/unfinished sequences; non-ASCII text |
| Annotations | Exact/derived/synthetic runs; overlapping order; semantic tags/styles; continuous/atomic/point truncation; two hosts/themes |
| Smooth | Current default/configured rate traces; first mount/backlog; burst/slow producer; seal; replace/clear; shared Source with independent policies; failed/delayed frame; inactive timer removal |

Compare semantic content, metadata, diagnostics and physical output. A prefix oracle is the same authoritative parser in fresh-state mode, not an old structural transport.

### 18.5 Port/Connector/viewport lifecycle

Test: connect before mount; activate before mount; no implicit first activation; requested A→B→C before flush; successful switch; failed switch with A visible; retry after relevant Source/width/theme input; deactivate; active Connector disposal; unmount while visible; remount at a different width; Port disposal while mounted/in use; Source disposal with inactive membership; host disposal with candidate work in flight.

Assert requested and visible identities separately. Assert Source subscription eligibility, represented Source revision and timer/projection residency. Manual scroll/follow intent must survive switches. Inactive resources may retain membership but must not do derived work.

### 18.6 Frame and backend failure injection

Inject failure at these distinct boundaries:

```text
structural prepare, before desired acceptance
state patch validation, before acceptance
Source validation, before acceptance
Connector semantic preparation
Connector width/measurement preparation
candidate layout/convergence
candidate painting
backend not-ready before submission
backend submission / partial physical write
delayed backend receipt while newer work is accepted
History sink accepts zero / some / all offered rows
visible commit invariant guard
host/environment teardown with pending work
```

For each case, assert desired revision, visible revision, captured/committed epoch, visible state versions, selected Connector, Source revision, subscription set, retained leases, History frontier and eventual retry behavior. Count emitted terminal/History rows so a visually similar final screen cannot hide duplicate output.

### 18.7 Native control and package compatibility

Preserve TextInput editing/cursor/paste/output routing; focus traversal and focus styling; ViewSlot set/stop/animation/cycle-boundary replacement; ScrollPane scrolling/content replacement; History push/freeze/transfer; async output waiting; headless screen/style/coordinate readback; resize; native clock; clean terminal restoration.

Run the consumer fixture from the produced package/native artifact. Public TypeScript extension contracts remain available even though the supported Rust authoring facade is removed. Do not replace declaration closure checks with a narrow unit test that misses exported types.

### 18.8 Performance assertions independent of timing

These are target assertions, not measured results:

- No structural work for a retained-state or Source mutation.
- No Source payload copy for a snapshot, theme change, scroll or delivery tick.
- No full stable semantic prefix clone for an ordinary tail append.
- No semantic parser invocation on pure theme/scroll/tick.
- No whole-state-registry snapshot for a local state patch.
- No projection/parser/timer work for inactive Connectors.
- No full offscreen content Surface allocation for a small visible window.
- No host-wide layout/paint cache clear for an isolated content change after targeted dependency handling is established.
- No per-field chain of native root constructions for one final node.
- No wholesale child sequence copy for a local persistent edit.
- No swallowed refusal or route switch to deleted architecture.

A legitimate grammar restart, broad structural replacement or full resize must be labeled and counted, not made invisible to satisfy these assertions.

---

## 19. Verification commands and performance acceptance

### 19.1 Run the repository's actual commands

Run from the repository root. These commands are grounded in the checked-in scripts; they were not executed in this audit. [S35]

```sh
# Confirm the environment and baseline.
git status --short
git rev-parse HEAD
bun --version
rustc --version --verbose
cargo --version

# Generate after schema/template changes, then prove checked-in output matches.
bun run generate:tui-abi
bun run check:tui-abi
cargo test -p tui-abi-gen

# Rust correctness and static gates.
cargo fmt --all -- --check
sh tools/lint/clippy-gate.sh
cargo test --workspace
cargo test --workspace --all-features

# TypeScript/public surface and ownership.
bun run typecheck
bun run lint:ts
bun run check:tui-declarations
bun run check:ownership

# Build/stage the actual default addon before TS integration tests.
bun run native:stage
bun run native:smoke
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
```

Do not use `git diff --exit-code` against the entire dirty implementation worktree before committing and call the intended changes a failure. For generation determinism, commit the intended generated outputs or compare them to a saved first-generation snapshot, then rerun generation and verify that the **second generation produces no change**. Keep schema, template, generated outputs, manifest/header/reference and conformance changes together.

Run qualification-specific artifact profiles through the repository's real staging/CI mechanism. The default safe N-API structural addon, the feature-qualified structural FFI surface, and the mandatory default content FFI are different coverage obligations. `--all-features` unit tests alone do not prove a packaged addon loads correctly.

If a baseline environmental declaration/lint failure exists, reproduce it on the unmodified audited revision in the same environment and record the exact diagnostic. Do not broadly ignore a class of errors or lower a gate to make the new code pass.

### 19.2 Surviving authoritative structural smoke benchmark

The existing benchmark asserts retained publication and reports phase samples/counters. [S36]

```sh
mkdir -p reports/pre-v5-l1

T15_GIT_SHA="$(git rev-parse HEAD)" \
T15_RUSTC_VERSION="$(rustc --version)" \
T15_WORKLOAD=plain_text \
T15_MODE=shared_path \
T15_SIZE=20 \
T15_WARMUP=2 \
T15_MEASURED=20 \
bun run packages/iyon-tui/bench/perf12_t15_authoritative_case.ts \
  > reports/pre-v5-l1/t15-route-smoke.json

bun run perf:content
```

Twenty measurements are a route smoke test, not performance acceptance. Use repeated fresh processes and the benchmark's longer warmup/measurement settings for comparison. Record the native artifact SHA, profile and target in the report. `T15_TRANSPORT` is report metadata in the inspected case file; setting it does not magically select another implementation.

Do not run the deleted differential runner or reconstruct its oracle. Do not run the four-hour full PERF-12 suite as a per-tranche gate.

### 19.3 Focused workload set

Add small, explicit cases where current benchmarks do not cover the target. The following are **proposed new workloads**, not claims that these names already exist:

| Group | Workload and scaling axis | Primary evidence |
|---|---|---|
| Construction | New text/decorated nodes with 0/1/many modifiers; 1/4/N spans | Native root/payload allocations; copied bytes; crossing vs construction time |
| Persistent structure | 2K/10K children; one set/splice; shared ancestor multi-edit; grids | Unchanged child visits, metadata copies, branch/path cost |
| State | One paint patch among 2K/10K mounted or unmounted states; geometry patch; no-op | Snapshot visits, locks, layout/paint counts, damage |
| Source | Tiny through large appends; retained old snapshots; replacement; head truncation | Payload and metadata copies, lock duration, retained/peak memory |
| Semantic text | Plain/Markdown/diff/ANSI append; difficult grammar restart | Parsed bytes, stable prefix reuse, domain assembly bytes, semantic allocations |
| Styling | Theme/selector change on existing content in two hosts | Parser/lowering count zero; paint/cache work; no Source revision |
| Delivery | Deterministic native ticks with no new bytes; bursts; seal | Parser/lowering/copy count zero; tick time; visible lag; deadlines |
| Viewport | Scroll through large content with small allocation; width resize | Rewrap/parse count, materialized rows/cells, memory |
| Lifecycle | Many inactive Connectors; repeated mount/switch/dispose; Source across hosts | Registry scans, timer/subscription counts, retained memory |
| Failure | Repeated candidate failures, delayed/partial receipts | Old-product retention bound, leak recovery, duplicate-output count |

Separate Source acceptance timing from end-to-end visible-frame latency. Separate initial parse/first layout from steady-state reuse. Report work by phase so a faster boundary cannot conceal a slower renderer.

### 19.4 Acceptance policy

Correctness, route, identity and lifetime gates are exact. Performance acceptance requires both deterministic work/copy counters and repeatable measurements on the same target/profile/corpus.

Report p50, p95, p99, maximum/outliers, allocations, copied bytes, retained memory and peak memory. Compare each workload, not one average across unrelated cases. Use multiple fresh-process runs, alternating baseline/candidate order where practical. Include cold startup and steady-state costs separately.

There is no blanket permission in this handoff for “up to N% slower.” A repeatable regression is a failed acceptance item until understood and removed, or explicitly approved as a separate tradeoff by the project owner. A noisy sample is not evidence of either a win or a regression.

“Maximally performant” is the optimization objective, not a theorem established by renaming types. Do not claim success before the workload evidence exists. A new interner, virtual dispatch layer, general batch, or extra arena must justify its absolute and scaling costs.

### 19.5 Platform, safety and memory qualification

Use every target currently supported by the package/staging/CI matrix; this project must not silently narrow that matrix. Validate same-image N-API/content FFI loading, metadata mismatch rejection, GC/TypedArray lifetime, teardown, and conformance from produced artifacts, not only Rust unit functions.

Exercise default and qualified feature profiles. In particular, inspect panic handling under `fast-view-abi`; the generator changes catch behavior by feature. Test panic/abort policy in subprocesses where necessary and ensure no unwind crosses a C boundary. Do not disable validation or panic containment merely to improve a microbenchmark. [S31]

Use memory/sanitizer/model tools supported by the project/toolchain where practical, but do not invent a passing Miri/TSan/loom result or require a tool to support the Node-API runtime when it cannot. Pure storage/state/scheduler models should be testable independently of the addon.

---

## 20. Required completion report for each tranche

Commit a short report with this exact information:

```text
Tranche:
Baseline SHA:
Result SHA:
Native artifact hash / target / profile / features:
Bun and Rust versions:

Changed authoritative path:
Deleted path/helpers:
Surviving passive types and why they are direct storage:
Any temporary scaffold and its deletion tranche:

Behavior fixtures exercised:
Failure/rollback fixtures exercised:
Ownership/lifetime evidence:
Route/copy/work counters:
Focused timing and memory results, with raw report paths:

Generation/public-declaration/ownership gates:
Platform qualification:
Baseline failures reproduced, if any:
New failures or deviations:

Production LOC added/deleted:
Generated LOC added/deleted:
Test/documentation LOC added/deleted:

Decision: accepted / blocked
Reason for any block:
```

Do not mark a tranche accepted while its implementation is still reached only by an unused test hook. Tests must drive the authoritative production route. Do not remove a regression test because its old builder-based setup no longer compiles; migrate the setup to the current transport/factory and retain the behavioral assertion.

---

## 21. Definition of done

The project is complete only when every statement is true:

### Runtime and public contract

- The core crate is not a supported Rust UI-authoring package.
- The old public authoring facade/prelude and unnecessary builder/extension APIs are gone.
- Native code uses the narrow internal binding seam; it does not import a renamed copy of the old facade.
- TypeScript public functionality and declarations remain compatible.
- Current terminal layout, controls, History and backend behavior are preserved.
- There is one canonical retained node representation and one content architecture.

### Direct lowering and retained state

- New nodes are assembled directly from validated fields; common modifiers do not create chains of intermediate roots.
- Text/axis/grid lowering consumes owned data and retains unchanged payloads/edges without avoidable deep copies.
- Exact-root reuse, semantic NodeId promotion, leases and persistent derivations remain effective.
- Actual retained-state methods use the generated typed protocol and do not publish structure.
- State capture is proportional to changed/newly demanded versions, not every registered resource.
- Null, clear, masked base changes, remount and in-flight lifetime semantics are exact.

### Content and paint

- Source FFI copies borrowed payload into native ownership once and does no projection inline.
- Snapshot/append/truncation avoid whole-surviving-content and whole-metadata copying.
- Annotation fields, order, truncation and provenance survive unchanged.
- Semantic parser results are independent of theme, scroll and clock ticks.
- Markdown restart correctness and diff/ANSI streaming semantics are preserved.
- Smooth has Connector-local input/frontier/clock state and no per-tick reparsing or whole-surface cloning.
- Measure and paint consume the same captured prepared content product.
- Viewport paint does not allocate a full offscreen content surface.
- History uses shared prepared products and receipt-driven frontiers, not copied Source prefixes/surface suffixes.

### Transactions and performance

- Desired acceptance remains distinct from visible commit.
- Ordinary hard errors are rejected before authoritative acceptance.
- Failed candidates do not advance visible bindings, Connector selection or presentation state.
- Newer pending work survives an older in-flight frame commit.
- Partial physical output and History receipts recover without duplicate emission.
- Source fan-out, native timers and automatic drains have no lost-wake/spin/lifetime regressions.
- Same-image ABI/package checks pass for supported targets/profiles.
- Focused behavior, work/copy, timing and memory evidence supports acceptance per workload.
- Deleted architecture and migration scaffolding are absent from production.
- The future v5 replacement seams are documented, with no Taffy/GPUI/React rewrite hidden in this project.

The final result should be smaller because duplicated authoring and staging layers are gone, and faster because native work follows changed data and explicit dependencies. Neither result is established until the final diff and measurements demonstrate it.


---

## Appendix A — Pinned source evidence and inspection scope

All `S` references resolve to the audited commit, not moving `main`. These references support the **current-state findings**; proposed new paths, representations, protocols and performance targets are design recommendations in this handoff. A linked file is not a claim that every line of that file was read or that its tests were executed. The narrower inspection scopes below are intentional.

The supplied document fingerprints and authority order are in §1. Their historical audits, reported test counts and future design statements do not replace current source evidence.

| Reference | Pinned source | Evidence / inspection scope |
|---|---|---|
| [S01] | `crates/iyon-tui/src/lib.rs` | Core exports, prelude, public authoring versus host integration. |
| [S02] | `crates/iyon-tui/Cargo.toml` | Core package/features; publication policy and native-host integration. |
| [S03] | `crates/iyon-tui/src/presentation/ir.rs` | Inspected ranges 1–260 and 650–1220: persistent sequence construction, canonical View/ViewNode, common fields, map_node, native axis/grid/path operations. |
| [S04] | `crates/iyon-tui-native/src/tui/view_abi.rs` | Inspected declarations/reference runtime, edit transaction implementation, cache-first constructors and structural patches; see also the narrower entries below. |
| [S05] | `crates/iyon-tui-native/src/tui/view_abi.rs` | Text helpers and CString/UTF-8 ingress; text_view_from_spans and repeated fluent layout modifiers. |
| [S06] | `crates/iyon-tui-native/src/tui/view_abi.rs` | Persistent axis/grid edits and parse_and_build_grid; two-phase framing and builder replay. |
| [S07] | `crates/iyon-tui-native/src/tui/view_abi.rs` | parse_and_build_decorated; masks, custom-glyph trailer and exact modifier precedence. |
| [S08] | `crates/iyon-tui/src/presentation/api/text.rs` | Inline/page-slice/owned TextStorage, TextSpan, and Text authoring wrapper construction. |
| [S10] | `crates/iyon-tui/src/retained_state/capabilities.rs` | Concrete state-capability kinds and geometry validation. |
| [S11] | `crates/iyon-tui-native/src/tui/view_state.rs` | Actual retained-state N-API ingress: dynamic patches, string clear lists, wake object and validators. |
| [S12] | `packages/iyon-tui/src/transport/state/control.ts` | TS patch normalization, null/clear behavior and textual color/style encoding. |
| [S13] | `crates/iyon-tui/src/retained_state/record.rs` | Record snapshot cloning, override mutation, revisions, no-op and lifecycle logic. |
| [S14] | `crates/iyon-tui/src/retained_state/registry.rs` | Whole-registry snapshot map, target checks, desired/visible/in-flight bindings and monotonic IDs. |
| [S15] | `crates/iyon-tui/src/retained_state/occurrence.rs` | OccurrenceBox base/effective fields and state application. |
| [S16] | `crates/iyon-tui/src/application/view_state.rs` | Host/record locking, ID readback, mutation wake and disposal integration. |
| [S17] | `crates/iyon-tui/src/application/content.rs` | Source records, storage/snapshots, mutation/retention and annotations; inspected contiguous source ranges through line 3410. |
| [S18] | `crates/iyon-tui/src/application/content.rs` | Source-to-raw projection, semantic rendering and complete content surface preparation. |
| [S19] | `crates/iyon-tui/src/application/content.rs` | Surface reveal, stable-prefix preparation and semantic annotation rewrite helpers. |
| [S20] | `crates/iyon-tui/src/content/text/markdown.rs` | Markdown projector state, stable-prefix checks, reference-context restart, cache copying and composition. |
| [S21] | `crates/iyon-tui/src/content/text/source.rs` | RawDomain concatenation, source witnesses, owned prefix/suffix construction and exact runs. |
| [S22] | `crates/iyon-tui/src/presentation/content.rs` | ContentProvider measurement/paint contract and History-specific hooks. |
| [S23] | `crates/iyon-tui/src/content/text/render/mod.rs` | TextRenderer to View lowering and context-sensitive block/list spacing. |
| [S24] | `crates/iyon-tui/src/projection/smooth.rs` | Current SmoothConfig, span-weighted delivery, clock credit and input pending reconstruction. |
| [S25] | `crates/iyon-tui/src/application/content.rs` | Inspected lifecycle/control/projection/tick/commit and subscriber paths, particularly lines 1990–3410. |
| [S26] | `crates/iyon-tui-native/src/content_ffi.rs` | Content ABI records/statuses, pointer guards, copy_records, Source identity resolution and mutation adapter. |
| [S27] | `crates/iyon-tui/src/theme/mod.rs` | Theme entries, selector replacement, specificity/declaration ordering and resolution. |
| [S28] | `crates/iyon-tui-native/src/tui.rs` | lower_theme, sparse style decoding, complete text selector dimensions and enum mappings. |
| [S29] | `crates/iyon-tui/src/application/host.rs` | Inspected HostRunning/HostInner and native ViewSlot integration, committed/candidate fields, in-flight receipt/epoch metadata. Not a complete audit of every host method. |
| [S30] | `crates/iyon-tui/src/scene/host.rs` | Inspected SceneHost/PreparedSceneFrame declarations, History pressure helper, invalidate_content and local geometry refresh. Not a complete audit of all scene paths. |
| [S31] | `tools/tui-abi-gen/src/render_rust.rs` | Generated PODs/guards, feature-dependent unwind boundary and N-API CString conversion. |
| [S32] | `packages/iyon-tui/src/transport/structural/retained-dag.ts` | Authoritative route design, weak semantic hints, transport scratch, counters and MaterializeTx ownership. |
| [S33] | `packages/iyon-tui/src/transport/native/resources.ts` | Plane-neutral registration/lookup facade and the underlying resource registry authority. |
| [S34] | `tools/ownership/check.ts` | Existing ownership/dependency/public-contract gate structure; extend rather than disable it. |
| [S35] | `package.json` | Repository commands, native stage/smoke, TypeScript/Rust checks and focused content benchmark script. |
| [S36] | `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts` | Surviving authoritative retained-route benchmark; workload variables and reported route metadata. |
| [S37] | `crates/iyon-tui/src/content/text/mod.rs` | Existing canonical semantic text modules/types/projectors and render/visitor ownership. |
| [S38] | `crates/iyon-tui/src/content/text/diff.rs` | Unified diff roles, in-hunk state, marker provenance and line handling. |
| [S39] | `crates/iyon-tui/src/content/text/ansi.rs` | ANSI options/state, safe display intent, raw-domain traversal, escape/CSI scanning and styled exact runs. |
| [S40] | `crates/iyon-tui/src/presentation/api/style.rs` | Sparse StyleSpec and attribute presence, Insets, named colors and owned semantic style atoms. |
| [S41] | `crates/iyon-tui-native/Cargo.toml` | Native crate boundary, dependencies and features. |
| [S42] | `Cargo.toml` | Workspace members and shared dependency configuration. |
| [S43] | `ARCHITECTURE.md` | Repository ownership/publication contract and current Rust/TypeScript API framing. |
| [S44] | `crates/iyon-tui/src/presentation/api/mod.rs` | Authoring facade and re-export of the canonical IR View, rather than a second View representation. |
| [S45] | `crates/iyon-tui/src/presentation/mod.rs` | Existing private presentation/layout/paint/content module boundaries. |
| [S46] | `crates/iyon-tui/src/projection/mod.rs` | Projection algebra, validation, composition and temporal publication boundary. |
| [S47] | `crates/iyon-tui/src/application/mod.rs` | Public application exports and internal runtime modules. |
| [S48] | `crates/iyon-tui/src/retained_state/mod.rs` | State module ownership, public passive patch types versus private records/effects. |

### Rust visibility rule

[R01] is the official Rust Reference, **Visibility and Privacy**. It supports the specific language rule that `pub(crate)` visibility is confined to the current crate. It does not prescribe this project's module layout or establish performance claims.

### Required follow-through before editing

Read the complete implementation and callers of each touched symbol in the checked-out baseline. In particular, inspect the entire grid normalizer, text renderer's block/inline helpers, frame preparation/receipt methods, resource registry, and platform staging scripts before altering them. This handoff names their responsibilities and gates; it does not replace local call-site verification. Update the evidence ledger when additional source inspection changes a proposed boundary.

[S01]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/lib.rs
[S02]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/Cargo.toml
[S03]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/ir.rs
[S04]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui/view_abi.rs
[S05]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui/view_abi.rs#L3910-L4185
[S06]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui/view_abi.rs#L2870-L3150
[S07]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui/view_abi.rs#L3230-L3525
[S08]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/api/text.rs#L1-L270
[S10]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/retained_state/capabilities.rs
[S11]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui/view_state.rs#L1-L310
[S12]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/packages/iyon-tui/src/transport/state/control.ts
[S13]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/retained_state/record.rs
[S14]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/retained_state/registry.rs
[S15]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/retained_state/occurrence.rs
[S16]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/view_state.rs
[S17]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/content.rs
[S18]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/content.rs#L570-L845
[S19]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/content.rs#L845-L1125
[S20]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/markdown.rs#L1-L575
[S21]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/source.rs#L1-L260
[S22]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/content.rs
[S23]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/render/mod.rs
[S24]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/projection/smooth.rs#L1-L300
[S25]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/content.rs
[S26]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/content_ffi.rs#L1-L325
[S27]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/theme/mod.rs
[S28]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/src/tui.rs#L1780-L2070
[S29]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/host.rs
[S30]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/scene/host.rs
[S31]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/tools/tui-abi-gen/src/render_rust.rs#L1-L210
[S32]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/packages/iyon-tui/src/transport/structural/retained-dag.ts#L1-L280
[S33]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/packages/iyon-tui/src/transport/native/resources.ts
[S34]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/tools/ownership/check.ts#L1-L235
[S35]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/package.json
[S36]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/packages/iyon-tui/bench/perf12_t15_authoritative_case.ts
[S37]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/mod.rs
[S38]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/diff.rs
[S39]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/content/text/ansi.rs#L1-L250
[S40]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/api/style.rs#L1-L270
[S41]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui-native/Cargo.toml
[S42]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/Cargo.toml
[S43]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/ARCHITECTURE.md
[S44]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/api/mod.rs
[S45]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/presentation/mod.rs
[S46]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/projection/mod.rs
[S47]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/application/mod.rs
[S48]: https://github.com/alexykn/iyon-tui/blob/90f19f5c9057ebb41fd1f8ffc73adbab05656cbc/crates/iyon-tui/src/retained_state/mod.rs
[R01]: https://doc.rust-lang.org/reference/visibility-and-privacy.html
