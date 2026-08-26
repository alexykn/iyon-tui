Yes. I’d hand this to an implementation agent almost exactly like this. I’ve made the **“do not refactor the retained architecture while deleting transports”** constraint deliberately repetitive because that is the main failure mode here.

# PERF-12 Transport Cleanup Handoff — N-API + Direct FFI Only

## Status

Ready for implementation after completion of T15.

This tranche is a **surgical deletion and simplification pass** over `alexykn/iyon-tui@perf-refactor`.

It is **not** PERF-13.

It must not redesign the public API, View model, retained DAG, composition layer, execution scopes, layout engine, paint engine, content model, or runtime semantics.

---

# 1. Objective

Remove all obsolete View transport experiments and compatibility machinery so that the only active PERF-12 View lowerings remaining are:

```text
                  retained semantic architecture
                            │
                            │
                    canonical View ABI
                    /              \
                   /                \
              N-API               direct FFI
             canonical            second lowering
              default             feature-gated
```

Both paths must continue to exercise the **same PERF-12 retained architecture**.

The cleanup must preserve:

- semantic View DAG
- NodeId identity
- stable subtree identity and cutoff
- environment-local semantic cache
- NativeRef table
- weak cache behavior
- strong JS/root leases
- release batching
- recovery semantics
- derivation hints
- retained paths
- retained edit transactions
- `PersistentSeq`
- wide-axis/grid structural editing
- materializers
- scratch/payload lanes used by the canonical ABI
- builders
- style atom/style refs
- multi-edit behavior
- current Rust layout
- current Rust paint
- current Rust host/runtime
- canonical generated ABI
- N-API lowering
- direct FFI lowering

The T15 result establishes that these two lowerings are semantically equivalent.

Baseline supplied by T15:

```text
311 cases per arm / 622 total

Correctness mismatches:   0
Structural mismatches:    0
Phase-array mismatches:   0

N-API/direct geometric mean: 1.0421
N-API ≈ 4.2% slower

Multi-edit 2/8/32/64:
all parity checks passed

1,000,000-operation memory gate:
both converged to

204 semantic entries
204 native-ref slots
1 leased slot
203 unleased live slots
```

The stable-subtree benchmark flaw was also corrected before these results: stable subtrees now retain identity across samples.

These results are the transport decision.

Do not reopen that decision in this tranche.

---

# 2. Non-negotiable architectural invariant

## This task deletes transports. It does not change semantics.

If a piece of code belongs to the retained semantic architecture and is used by current N-API or direct FFI, **leave it alone**.

In particular, do not opportunistically rewrite:

```text
packages/iyon-tui/src/compose.ts
packages/iyon-tui/src/execution.ts
packages/iyon-tui/src/execution-context.ts
packages/iyon-tui/src/component.ts
packages/iyon-tui/src/persistent_seq.ts
packages/iyon-tui/src/values/view.ts

crates/iyon-tui/src/presentation/*
crates/iyon-tui/src/component/*
crates/iyon-tui/src/theme/*
crates/iyon-tui/src/content/*
crates/iyon-tui/src/stream/*
crates/iyon-tui/src/history/*
```

except for a **strictly mechanical removal of a now-dead transport reference** when compilation proves it is necessary.

No cleanup-driven redesign is permitted in those layers.

The TS View implementation explicitly documents that the eager immutable semantic DAG, stable NodeIds, freezing and child identity sharing are the retained model. It also explicitly says packed metadata was already removed from View construction and that remaining obsolete route machinery belongs to the PERF-12 cleanup tranche.

That semantic model is protected.

---

# 3. What remains

## 3.1 Canonical N-API path

N-API is the default View transport.

Keep the canonical `view_abi` runtime and all semantics required by it.

The current native runtime contains the important PERF-12 state directly:

- semantic `nodes` weak cache
- dense NativeRef table
- `node_refs`
- retained paths
- builders
- edit transactions
- style atoms
- style refs
- lease/release accounting
- weak-cache scavenging
- generation/recovery state

Those are part of the current retained implementation, not old transport baggage.

Do not simplify these merely because older transports also used the same runtime.

The central cache belongs to the environment/runtime, not to any particular lowering.

---

## 3.2 Direct FFI lowering

Keep the direct FFI implementation.

For this cleanup it remains feature-gated.

Do **not** make FFI default here and do not redesign it around future content lanes.

That belongs to PERF-13.

The native crate currently has:

```toml
direct-ffi = []
```

with comments describing the safe N-API addon as default and FFI as the qualification path.

Keep the generated/direct ABI implementation, conformance machinery, and T15-relevant N-API-vs-FFI oracle.

The generated View ABI artifacts are current infrastructure, not packed transport debris:

```text
packages/iyon-tui/src/generated/view_abi.ts
packages/iyon-tui/src/generated/view_abi_conformance.ts
packages/iyon-tui/src/generated/view_abi_manifest.json
packages/iyon-tui/src/generated/view_calls.ts
packages/iyon-tui/src/generated/view_materialize.ts
```



Do not delete generated ABI infrastructure simply because it contains low-level transport code.

---

# 4. What must be removed

There must be no executable legacy transport after this tranche.

## 4.1 Native packed / FastShared implementations

Delete:

```text
crates/iyon-tui-native/src/tui/fast_shared.rs
crates/iyon-tui-native/src/tui/packed.rs
crates/iyon-tui-native/src/tui/packed_v3.rs
crates/iyon-tui-native/src/tui/packed_v4.rs
```

These are still substantial implementations beside the canonical `view_abi.rs`.

Keep:

```text
crates/iyon-tui-native/src/tui/view_abi.rs
```

---

## 4.2 Old native feature flags

Remove the obsolete packed transport feature family from:

```text
crates/iyon-tui-native/Cargo.toml
```

Current obsolete features include:

```toml
perf-packed-transport
perf-packed-benchmark
perf-packed-timing
```

They currently pull in the old `native-shared-memory` path.

Do not remove unrelated PERF-12 instrumentation merely because its name contains `perf`.

Keep things such as `perf-counters` or timing support if they are still used by the canonical N-API/direct benchmark and diagnostics.

Keep `direct-ffi`.

---

## 4.3 Packed modules wired into `tui.rs`

Remove the conditional module declarations:

```rust
mod fast_shared;
mod packed;
mod packed_v3;
mod packed_v4;
```

and all APIs that exist solely to benchmark or inspect those transports.

Current examples include:

```text
tuiPerfV3ResetViewBridgeCache
tuiPerfV3ViewBridgeCacheSize
tuiPerfV3PackedSlotPages
tuiPerfV3ViewBridgeGeneration

tuiPerfV4ResetViewBridgeCache
tuiPerfV4ViewBridgeCacheSize
tuiPerfV4ViewBridgeGeneration
```

and the packed benchmark cfg blocks that expose them.

Do not remove ordinary canonical cache diagnostics just because old packed benchmarks happened to call them.

Classify each exported probe by **ownership**, not by proximity:

```text
canonical runtime diagnostic       → keep if still useful
packed/V3/V4/FastShared diagnostic → delete
T15 N-API/direct oracle            → keep
```

---

# 5. Clean `NativeViewRuntime` without changing its retained semantics

`NativeViewRuntime` currently contains old transport state alongside canonical PERF-12 state.

Delete only the old transport-owned fields, such as:

```rust
packed_v3
packed_v4
fast_slots
fast_sessions
```

and their initialization/accessor helpers. These are cfg-gated old benchmark state today.

Preserve the canonical fields around them.

Do not redesign `NativeViewRuntime`.

Do not change:

- NativeRef allocation strategy
- paging
- semantic publication rules
- weak cache ownership
- generation semantics
- lease behavior
- path refs
- edit transactions
- builder refs
- style refs
- maintenance/scavenging

If deleting the old fields causes an apparently attractive struct cleanup, resist it. That is not this task.

The intended diff should look like:

```text
NativeViewRuntime
    canonical state
    canonical state
-   PackedV3 state
-   PackedV4 state
-   FastShared state
    canonical state
    canonical state
```

not a rewrite of the runtime.

---

# 6. Remove TS packed implementations

Delete:

```text
packages/iyon-tui/src/packed.ts
packages/iyon-tui/src/packed_v3_meta.ts
```

`packed.ts` still contains the old Uint32 transaction encoder, REF/DEF protocol, cache-miss retry logic and packed counters.

`packed_v3_meta.ts` still contains packed lineage, recipes, canonicalization caches, sequence metadata and V3 publication state.

None of this should remain as an alternative executable transport.

Search the entire repository for imports or references before deleting each file and remove those call sites mechanically.

Do not replace them with a new abstraction.

---

# 7. Purge packed protocol vocabulary from the semantic bridge schema

The private semantic bridge schema currently mixes actual semantic View constants with several generations of packed protocol constants.

This must be separated by deletion.

`ir.ts` currently defines legitimate semantic bridge vocabulary such as:

```text
viewText
viewDiff
viewRow
viewColumn
layoutFlex
trackFixed
overflowEllipsis
wrapGrapheme
horizontalCenter
...
```

but the same schema type also contains the complete packed v1/V3/V4 protocol vocabulary.

Remove all schema properties belonging exclusively to:

```text
packed*
packedV3*
packedV4*
```

and remove the corresponding:

```ts
PACKED_VIEW
PACKED_V3
PACKED_V4
```

exports.

Preserve all constants used by the current semantic View DAG and canonical N-API/direct ABI.

Likewise clean:

```text
packages/iyon-tui/src/bridge-schema.json
```

which currently contains the packed protocol constants directly beside semantic View/layout constants.

After cleanup, `bridge-schema.json` should describe the semantic bridge that actually exists, not transport experiments that lost.

Do not renumber surviving semantic constants as part of this cleanup.

Do not bump schema versions merely to make the file prettier unless the current build/generator contract genuinely requires it.

Preserve existing numeric values.

---

# 8. `native-shared-memory`: treat carefully

The Rust core currently exposes:

```toml
native-shared-memory = []
```

and the packed native feature maps into it.

Do not blindly delete every occurrence of `native-shared-memory` just from its name.

First establish whether each occurrence is:

1. exclusively an implementation dependency of deleted packed/FastShared transports, or
2. still required by canonical N-API/direct PERF-12 machinery.

The target end state is that no obsolete transport remains.

However, **do not use removal of this feature as an excuse to rewrite `crates/iyon-tui` presentation/composition code.**

If a transport-only compatibility helper is embedded inside a semantic file, prefer the smallest possible mechanical deletion.

If its ownership is ambiguous and removal would require semantic refactoring, stop and report that specific dependency rather than changing the retained model.

This cleanup prioritizes semantic safety over deleting one suspiciously named helper.

---

# 9. Remove dead route compatibility stubs only when their consumer is removed

`packages/iyon-tui/src/values/view.ts` documents several always-undefined recipe readers retained solely because generated route code still expected them:

```text
nativeAxisRecipe
nativeTextRecipe
nativeSpacerRecipe
nativeScalarPatch
viewBackingState
```

The source explicitly says their removal belongs with removal of the old route code during the PERF-12 cleanup tranche.

Therefore:

1. identify the exact remaining consumer;
2. remove the obsolete route;
3. then remove its now-unused stub.

Do not reverse the dependency order and then repair compilation by changing View construction.

The eager View DAG must remain intact.

---

# 10. Benchmarks and historical evidence

Delete benchmark code whose only purpose is to execute:

```text
packed
packed V3
packed V4
FastShared
```

Do not preserve dead production code merely so historical benchmark scripts continue compiling.

Historical benchmark **results/documents** may remain as historical evidence.

Executable benchmark infrastructure should reflect the surviving decision:

```text
N-API
vs
direct FFI
```

The corrected T15 suite is the authoritative transport oracle.

Do not accidentally delete it while removing older PERF benchmark generations.

Where old scripts contain useful generic trace generation shared with T15, keep the generic trace machinery and delete only old transport arms.

---

# 11. Tests

Remove tests that test implementation details of transports that no longer exist.

Keep or migrate assertions that actually describe retained semantic invariants.

Example distinction:

```text
"packed V4 emits this opcode"
    → delete

"same NodeId with conflicting live View is rejected"
    → keep

"cold recovery preserves semantic identity"
    → keep

"released NativeRef does not leak a strong lease"
    → keep

"PersistentSeq edit preserves untouched child identity"
    → keep

"N-API and direct FFI produce identical structure"
    → keep

"V3 generation increments after reset packet"
    → delete
```

Do not delete a correctness test merely because it originated during a packed experiment if the invariant is now part of PERF-12.

---

# 12. Required repository-wide residue scan

After implementation, search the complete repository for at least:

```text
packed
packed_v3
packed_v4
PackedV3
PackedV4
PACKED_VIEW
PACKED_V3
PACKED_V4
FastShared
fast_shared
perf-packed
ION_PACKED_CACHE_MISS
native-shared-memory
```

Every remaining hit must be classified.

Allowed remaining hits:

- historical documentation clearly describing old work;
- archived benchmark result text;
- migration history where removal would destroy useful documentation.

Not allowed:

- active imports
- active feature flags
- active source modules
- active runtime fields
- generated live protocol constants
- fallback branches
- comments claiming packed/V4 is a current fallback
- tests that instantiate old transports
- package exports
- build scripts that still generate packed definitions

The active source tree must tell one architectural story.

---

# 13. Important negative requirements

Do **not**:

- introduce PERF-13 content lanes
- remove `View.text()` yet
- redesign View as layout-only yet
- redesign style/theme APIs
- implement retained mutable node properties
- change content ownership
- make direct FFI mandatory/default yet
- change NodeId semantics
- change View identity
- change composition rules
- change `PersistentSeq`
- change derivation behavior
- change retained paths
- change multi-edit semantics
- change recovery behavior
- change lease ownership
- change layout algorithms
- change paint algorithms
- change public consumer APIs
- rename large architectural concepts
- “simplify” the ABI
- generate a new ABI shape
- collapse N-API and FFI into separate semantic implementations

PERF-13 comes after this cleanup.

This tranche must leave us with a clean PERF-12 baseline on which PERF-13 can be designed.

---

# 14. Implementation strategy

Use **six bisectable clean parts**, named C1–C6, with the relevant verification gate completed before moving to the next part. Do not combine them into one large deletion.

### C1 — remove native legacy transports

Delete:

```text
crates/iyon-tui-native/src/tui/fast_shared.rs
crates/iyon-tui-native/src/tui/packed.rs
crates/iyon-tui-native/src/tui/packed_v3.rs
crates/iyon-tui-native/src/tui/packed_v4.rs
```

Remove their module declarations, packed-only Cargo features, `NativeViewRuntime` fields/helpers, old N-API benchmark endpoints, and packed cfg blocks.

Preserve canonical runtime state, `direct-ffi`, and canonical diagnostics. Verify Rust formatting/checks and the canonical N-API/direct-FFI ABI tests.

### C2 — remove TypeScript packed transports

Delete:

```text
packages/iyon-tui/src/packed.ts
packages/iyon-tui/src/packed_v3_meta.ts
```

Remove their imports, exports, counters, benchmark adapters, and stale fallbacks. Do not replace them with a new abstraction.

Verify the TypeScript typecheck and relevant tests.

### C3 — clean the bridge schema

Remove packed v1/V3/V4 constants from:

```text
packages/iyon-tui/src/bridge-schema.json
packages/iyon-tui/src/ir.ts
```

Preserve all surviving semantic numeric values. Regenerate only artifacts mechanically derived from the surviving canonical schema/ABI, then run generated ABI and conformance checks.

### C4 — remove dead compatibility routes and stubs

Trace consumers first. Remove only obsolete routes proven unreachable after C1–C3, then remove their now-unused always-undefined recipe helpers:

```text
nativeAxisRecipe
nativeTextRecipe
nativeSpacerRecipe
nativeScalarPatch
viewBackingState
```

Do not modify retained View construction or semantic DAG behavior. Verify the affected TypeScript and Rust consumers.

### C5 — clean benchmarks and tests

Delete executable benchmark arms and implementation-specific tests for:

```text
packed
packed V3
packed V4
FastShared
```

Preserve retained semantic invariant tests, memory/lifetime regressions, and the T15 N-API/direct-FFI oracle. Keep generic trace generation when it is shared with T15.

### C6 — remove active residue and validate the final tree

Remove stale active comments, package exports, build references, and current-architecture documentation that still presents deleted transports as viable. Keep historical results and migration evidence where useful.

Run the complete residue scan and all final verification gates, including both default N-API and `direct-ffi`, generated ABI/conformance checks, consumer fixtures, and the canonical memory/lifetime and parity regressions.

If actual dependency structure requires a different boundary, preserve the same principle: **delete outward from obsolete transports; do not refactor inward into the retained semantic architecture.**

---

# 15. Verification gates

At minimum, the final tree must pass the project's normal:

```text
Rust formatting
Rust compile/check
Rust tests
TypeScript typecheck
TypeScript tests
generated ABI/conformance checks
consumer fixture
```

Also run both surviving lowerings:

```text
default N-API
direct-ffi feature
```

and confirm the canonical ABI has not accidentally diverged.

The T15 correctness/oracle suite should still report parity between N-API and direct FFI.

Re-running the entire expensive authoritative benchmark distribution is not required merely to prove deletion unless the normal workflow demands it; the critical requirement is that the surviving paths still pass correctness/structural/phase parity.

Run the memory/lifetime gate or its canonical regression equivalent if available.

No regression is acceptable in:

```text
semantic entries
NativeRef lifetime
leased-slot lifetime
weak-cache convergence
multi-edit parity
```

---

# 16. Stop conditions

Stop instead of improvising if deletion appears to require any of the following:

- changing DAG shape;
- changing View construction;
- changing child identity rules;
- changing `PersistentSeq`;
- changing retained execution semantics;
- changing public composition behavior;
- changing layout;
- changing paint;
- replacing a deleted transport with a newly invented fallback;
- changing canonical ABI semantics merely to make cleanup easier.

At that point produce a dependency trace explaining:

```text
obsolete item
    ↓
unexpected surviving dependency
    ↓
why N-API/direct currently needs it
```

Do not solve that architectural question inside the cleanup tranche without review.

---

# 17. Definition of done

The cleanup is complete when:

```text
ACTIVE VIEW LOWERINGS

1. N-API
2. direct FFI
```

and nothing else.

The retained architecture beneath them remains the same.

Repository-wide active source contains no:

```text
packed
packed V3
packed V4
FastShared
shared-memory transport experiment
packed fallback
packed benchmark feature
packed protocol schema
```

The canonical ABI remains the shared boundary.

N-API remains the default PERF-12 lowering.

Direct FFI remains the second implementation of the same ABI.

Both still resolve into the same:

```text
semantic DAG
identity/cache model
leases
NativeRefs
derivations
PersistentSeq
retained paths
edit transactions
materialization
Rust host
layout
paint
```

No semantic or structural redesign is part of this task.

The resulting codebase should make it difficult for a future human or coding agent to infer that packed/V3/V4/FastShared are still viable architectural options.

There should be exactly one current PERF-12 architecture and exactly two physical lowerings of it.

This intentionally stops before the content-lane / retained-property work; that can start as PERF-13 from a codebase where the old transport archaeology is gone.
