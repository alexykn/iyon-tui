# PERF-12 - Retained DAG Direct FFI

## Persist semantic identity, not transport bytes

**Status:** final architecture and implementation handoff  
**Repository:** `alexykn/iyon`  
**Branch family:** `perf-refactor`  
**Historical semantic baseline:** PERF-7v2 Candidate A / Direct at `e5292d62c4011610850cbdc1ba4a35f296f78e4f`  
**Repository state re-audited while preparing this handoff:** `67741eb588e70ffe8ce7b08805040d0a9cc65f8c`  
**Current pinned runtime at that revision:** `bun@1.4.0`  
**Execution point:** after PERF-11v4 has produced its authoritative Bun 1.4 result and resource report  
**Candidate name:** **Retained DAG Direct FFI** (`retained_dag_ffi`)  
**Core rule:** **persist identity; transport only newly-created semantic work; never persist a second transport graph**

---

# Exact implementation tranches

PERF-12 is a full architecture experiment. The tranches below are intended to be ambitious merge-request-sized units. Do not collapse them into one implementation burst. In particular, do not begin full-schema transport work until the semantic representation, lifetime model, weak-cache cleanup, and common-node FFI path are proven.

## Tranche registry

This experiment has **16 implementation tranches**. Each tranche below names the exact document sections (`§`) it contains. Do not infer tranche boundaries from prose elsewhere in the handoff. Each tranche ends in its own commit and verification step; related tranches may share an implementation session only when every individual gate still runs independently.

| Tranche | Parent | Exact scope in this document | Required result before proceeding |
|---|---|---|---|
| **T1** | 12.0 | Evidence freeze and probes: source freeze `§82`; Bun qualification `§60`; same-image audit `§61`; memory attribution protocol `§57`–`§58`; direct-call floor probe `§83`; baseline record deliverables `§109` | `PERF-12-baseline.md`, `PERF-12-memory-attribution.jsonl`, `PERF-12-ffi-floor.jsonl` committed; 2.7 GiB classified into `§58` buckets; FFI call floor compatible with expected changed-frontier budget. If `§83` fails its decision threshold or `§105` applies, **STOP here** — no further tranches |
| **T2** | 12.2a | Shared runtime publication: one central `publish_semantic_view` helper `§24`; classify existing 11v3 ABI functions `§25`; NativeRef paged table design/decision `§52`–`§53`; page reclamation `§54`; deliverables checklist `§88` | All transports (Direct, 11v3, V4) route publication through one helper with no identity fork; existing 11v3/Direct regression suites pass unchanged; NativeRef representation chosen by measurement |
| **T3** | 12.2b | Bounded lifetime: weak-cache scavenging `§55`; maintenance counters `§56`; memory diagnostic ABI `§89`; churn acceptance gate `§59`; slot lease invariant tests `§111` | Post-maintenance weak/slot metadata = O(live + bounded slack); 1M-transient-node churn shows no linear post-GC slope; counters absent or compile-time-free in timing builds `§101` |
| **T4** | 12.1 | Semantic DAG restoration (no native change): faithful 7v2 reconstruction `§84`; JS representation `§13`; BridgeViewNode shape `§14`; sidecar inventory `§15`; hint-not-lease lifetime rule `§16`; no-FinalizationRegistry rule `§17`; semantic parity suite `§86`; construction gate `§85` | Full-schema semantic parity passes against current production Views; `retained_dag_ffi` construction ≤5% vs faithful Bun 1.4 `direct_7v2` (preferred: within noise); `nodeForBridge` is lookup-only |
| **T5** | 12.4a | Generator foundation: extend canonical ABI pipeline `§62`; MaterializerSpec model `§63`; generator validation rules `§64`; output placement `§65`; failure status detail `§74`; checked-vs-timing policy `§68`; owner-thread policy `§69` | Generator emits a one-kind vertical slice (spacer or container) end-to-end; conformance tests pass; illegal lifetime declarations fail generation |
| **T6** | 12.3 | Identity fast paths: root lease protocol `§18`; `ensureNative` core algorithm `§19`; exact-root fast path `§20`; stable subtree cutoff `§21`; runtime generation handling `§48`; BridgeNativeHint sidecar wiring `§15`/`§16`; exact-identity scaling test `§113` | Exact known root = 1 `hostRenderRef`, 0 semantic field reads, 0 buffer writes at 20/200/2k/10k node sizes; timing independent of descendant count; stale generation hints ignored correctly |
| **T7** | 12.4b | Common-node direct materializers: children-first materialization `§22`; native constructor semantic-cache-first rule `§23`; fixed-arity specialization `§32`; generated TypeScript style rules `§66`; native ownership split `§67`; cycle/work budgets `§75`; retained work budget `§50`; no-full-tree-diff rule `§51` | Fixed-size kinds materialize through monomorphic FFI; stable child cuts off before payload access; one representative SHARED_PATH retained case beats or ties `direct_7v2` total time before broader generation proceeds |
| **T8** | 12.5 | Variable-arity lanes: borrowed TypedArray transport `§29`; scratch tier policy `§30`; no-mapped-scratch rule `§31`; buffer lifetime tests `§116`; oversize → fallback routing `§50` | Variable-axis/Grid constructors use reusable synchronous `buffer`/`buffer_length`; native never retains a pointer after return; zero-length/max-length/oversize cases pass; no external ArrayBuffer machinery exists |
| **T9** | 12.6 | Retained clone/edit lanes: derivation hints `§27`–`§28`; text layout mutation `§38` | Wrap/align-only text change sends base NativeRef + NodeId + scalars, never resends payload; common scalar patch reuses base ref; hint-miss degrades cleanly to full materialization |
| **T10** | 12.7 | Wide retained edits: PersistentSeq preservation `§33`; wide sidecar exception `§34`; wide native edit path `§35`; Grid `§36`; wide benchmark gate `§96` | Replace/insert/remove/splice remain O(log₃₂ N) with counter proof at widths 2k/10k/100k; no flat materialization on the retained one-edit path |
| **T11** | 12.8 | Payload families: text paths `§37`; strings and embedded NUL `§39`; styles `§40`; Diff `§41`; streaming separation `§42`; string benchmark `§98` | Full `§39` correctness dataset passes; stable text/style payload never resent; stream bytes never enter structural construction; string path chosen by end-to-end measurement |
| **T12** | 12.9 | Transaction integrity: multi-branch DAG materialization `§43`; temporary lease transaction `§44`; host atomicity rule `§45`; stale hints `§46`; targeted one-retry recovery `§47`; recovery helper `§73`; failure injection suite `§118` | Common ancestors built once across branches; exactly one host mutation; every success/error/failure-injection path drains temporary leases; one bounded retry then authoritative fallback |
| **T13** | 12.10 + 12.11 | Router and boundaries: cold/rebuilt router and budgets `§49`; every View-bearing boundary inventory `§77`; History `§78`; Components `§79`; ViewSlot/ScrollPane `§80`; Animations `§81`; dormant-node recovery test `§114`; multi-host test `§115` | No production boundary silently routes through Direct/fallback on retained traces; dormant-node and multi-host lifetime correct; initial cold render chooses best cold path directly without wasted retained prefix |
| **T14** | 12.12 | Hardening: randomized DAG differential testing `§87`; cross-transport identity tests `§112`; fuzzing targets `§117`; full-schema coverage proof `§76`; banned-shortcut review `§107` | 100-seed differential suite, fuzz targets, and full-schema coverage green; no UAF, no retained borrowed pointer, no partial host mutation demonstrated under fault injection |
| **T15** | 12.13 | Authoritative comparison: phase visibility `§90`; structural counters `§91`; steady-state traces `§92`; benchmark matrix `§93`; large shared-subtree cutoff incl. cold-sidecar-gap case `§94`; multi-edit `§95`; cold `§97`; realistic agent trace `§99`; process isolation `§100`; statistics `§102`; result schema `§103`; adoption gates `§104`; memory gate recheck `§59` | Raw JSONL retained for every candidate; adopt/reject decided strictly by `§104` (realistic trace ≥10% over best prior candidate; no >3% credible common-case regression; cold within 5%; memory convergence). Report published regardless of outcome |
| **T16** | 12.14 | Conditional cleanup: removal candidates `§26`; complexity interpretation `§120`; code ownership end-state `§121`; rejected-architecture guards `§122`–`§124` | Executed **only after** T15 adoption plus soak; obsolete pending/recipe machinery removed or test-gated; final shape matches `§121` ownership map, not a regrown pending state machine |

## Registry rules

- **Parent mapping:** T1=12.0, T2+T3=12.2, T4=12.1, T5+T6/T7=12.3+12.4 (generator first), T8=12.5, T9=12.6, T10=12.7, T11=12.8, T12=12.9, T13=12.10+12.11, T14=12.12, T15=12.13, T16=12.14.
- **Order is mandatory:** T2/T3 precede everything that touches native state; T4 must prove semantic parity before any FFI materializer lands; T5 precedes all generated transport work; T15 decides; T16 is conditional.
- **Benchmark cost control:** tranche gates T1–T14 use the smoke profile (`§102.1`), never the full matrix; the incremental benchmark cache (`§102.2`) may skip re-running unchanged candidate arms within a tranche. The full authoritative suite runs exactly once, at T15.
- **Highest-risk boundaries are isolated:** T1 (stop-or-go), T2/T3 (shared-runtime rewrite affects every transport), T10 (wide asymptotics), T12 (failure atomicity), T13 (boundary completeness), T14 (unsafe-surface audit), and T15 (decision) each require their own commit and may not be merged together with neighbors.
- **Commit mapping:** `§108` commits 1→T1, 2→T4, 3→T2+T3, 4→T6+T7, 5→T8, 6→T9, 7→T10, 8→T11, 9→T12, 10→T13, 11→T14, 12→T15, 13→T16. Where a commit spans two tranches, run both tranches' gates separately.
- Any tranche whose gate fails triggers the corresponding stop condition in `§106`. Do not lower a gate because later work depends on it.

## Tranche implementation records (mandatory)

Every completed tranche must append an **implementation record** to this document as a new subsection under `## Tranche implementation records`, titled `### T<N> implementation record`, following the exact convention established by the PERF-11v3 records (`§2.5`–`§2.16` of `PERF-11v3-bun-1.4-zero-encode-native-view-handoff.md`). A tranche without a conforming record is not complete, regardless of whether its code passes tests.

Each record must contain, in order:

```text
1. Scope statement
   - tranche ID and parent 12.x mapping
   - exact § sections implemented
2. Commits
   - full or short SHAs with subject lines, as committed on perf-refactor
3. Review findings
   - every gap found between the local implementation and this handoff,
     and how each was corrected (11v3 records name these explicitly;
     do not silently absorb corrections into "implementation detail")
4. Implementation summary
   - what now exists, what changed structurally, what was deliberately
     NOT done yet (e.g. "production routing remains disabled until T13")
5. Provenance block
   - source revision at capture time (full SHA)
   - bun --version and bun --revision
   - rustc version and target
   - native artifact SHA-256 where a rebuilt addon is involved
   - schema BLAKE3 and generator BLAKE3 where generated ABI is involved
6. Gate evidence
   - each Required-result gate from the tranche table, with its actual
     measured result: test counts, counter values, benchmark medians,
     memory counters — raw numbers, not adjectives
   - benchmark records captured from clean working trees and committed
     under packages/iyon-runtime/bench/ with environment metadata;
     smoke-profile results marked "profile": "smoke" per §102.1
7. Status line
   - one bold terminal line: "**Tranche T<N> status: COMPLETE | PARTIAL |
     FAILED | STOPPED.**" followed by one or two sentences stating what
     is claimed and what remains for later tranches. PARTIAL/FAILED/
     STOPPED states must cite the applicable §106 stop condition if any.
```

Rules:

- Records are append-only history. Later tranches may correct factual errors in earlier records via an added errata paragraph; they may not rewrite prior claims.
- A record claiming COMPLETE must show gate evidence for **every** row of that tranche's Required-result column. Missing evidence forces PARTIAL.
- Timing numbers must state whether they come from the checked or timing build (`§68`) and must not be phase-subtracted claims (`§90`).
- The final record (T15) additionally links the adoption decision, raw JSONL artifacts, and the published report; the T16 record (if executed) documents exactly which machinery was removed or test-gated.

---

# 0. Executive decision

PERF-12 should **not** use the persistent Shared Mirror DAG as its primary architecture.

PERF-12 should also **not** make the full changed-closure fixed-record arena from the Retained Bridge Delta proposal its primary transport.

The final candidate should instead use the third option that the Delta handoff correctly identified as an important control, but promote it to the primary architecture:

```text
                     JavaScript

              immutable View value
                       |
                       v
            eager frozen BridgeViewNode
            historical 7v2 semantic shape
                       |
                       | WeakMap sidecars only
                       | NativeRef hint / derivation hint
                       v
                 ensureNative(node)
                       |
          +------------+-------------+
          |                          |
          | known NativeRef          | new semantic node
          |                          |
          v                          v
     return u32 ref          inspect this node only
     no payload read                  |
                                      | postorder children
                                      | stable child -> NativeRef, stop
                                      v
                           generated direct Bun FFI
                           per semantic constructor/edit
                                      |
                         +------------+------------+
                         |                         |
                         | scalar/fixed arity      | variable arity
                         | registers               | borrowed TypedArray
                         |                         | refs/bytes only
                         +------------+------------+
                                      |
                                      v
                              NativeViewRuntime
                                      |
                      +---------------+---------------+
                      |                               |
                      v                               v
                 NativeRef table               NodeId -> WeakView
                 fast acceleration             semantic identity
                      |                               |
                      +---------------+---------------+
                                      |
                                      v
                              retained Rust View DAG
                         PersistentSeq / retained text
                                      |
                                      v
                              hostRenderRef(root)
```

The shortest statement of the architecture is:

> **The immutable JavaScript DAG is the semantic declaration. The Rust `View` DAG is the retained native representation. A `NativeRef` is the correspondence between them. The bridge does not retain a third graph.**

For ordinary fixed-size nodes there is no packet, no record encoding, no parser, and no shared-memory allocation. Newly-created semantic nodes are materialized directly by generated engine-native Bun FFI calls.

For variable child/reference payloads, JavaScript writes only the needed `u32` refs or changed bytes into small reusable typed arrays and passes those arrays synchronously as borrowed buffers. Rust reads the same storage during the call and retains no pointer afterward.

For a retained update, work is proportional to the new semantic frontier and specialized wide-edit paths:

```text
semantic JS construction             O(changed semantic nodes)
NativeRef cutoff                     O(changed frontier)
direct FFI node materialization      O(changed frontier)
wide retained edits                  O(log_32 N + inserted refs)
stable subtree                       O(1) at its root
exact root                           O(1)
host mutation                        once
persistent transport bytes           zero
```

This design recovers the architectural reason 7v2 Candidate A was strong while keeping the transport and native-retention lessons from PERF-8 through PERF-11v3.

---

# 1. Why this is the final choice

The two supplied PERF-12 handoffs both identify the right high-level goal:

```text
cheap immutable JavaScript semantic identity
+
retained Rust semantic identity
+
no full-tree retransmission
+
no N-API property walk on the hot retained path
```

They differ in what they persist between the two semantic layers.

## 1.1 Retained Bridge Delta

The Delta design persists only:

```text
BridgeViewNode -> NativeRef sidecar
```

and uses a bounded transient arena for newly unknown nodes.

That is already much better than a full-tree packet because stable nodes are represented only by NativeRefs and the arena is not a retained graph.

Its remaining cost is that every unknown node is first encoded into a generic fixed-layout transport representation and then decoded into the Rust semantic object.

## 1.2 Shared Mirror DAG

The Shared Mirror design persists:

```text
BridgeViewNode
    -> SharedRef
    -> persistent shared record / edge blocks / payload refs
    -> NativeRef
    -> Rust View
```

This removes repeated transient record generation for the same immutable node and gives native a durable recovery source if the Rust View has disappeared.

However, once a live semantic node already has a NativeRef, Delta/direct-node designs also stop at that identity and do not resend the node. The persistent mirror's unique hot-path advantage is therefore smaller than it initially appears.

Its strongest unique capability is recovery after native retained state has disappeared while the JavaScript semantic node is still alive.

That is not enough to justify a second persistent graph in Iyon.

## 1.3 Retained DAG Direct FFI

Retained DAG Direct FFI persists exactly the useful thing:

```text
BridgeViewNode -> NativeRef hint
```

and materializes an unknown node directly into the existing native semantic graph.

It eliminates both:

```text
N-API property traversal
```

and:

```text
JS record encoding -> native record decoding
```

for common nodes.

The tradeoff is more FFI calls when a changed path contains many newly-created ancestors.

Bun 1.4 makes this tradeoff credible because normal `dlopen` / `linkSymbols` / `CFunction` calls are implemented in JavaScriptCore and hot call sites can lower to direct native calls. PERF-12.0 still measures the actual pinned runtime before committing the whole implementation.

---

# 2. Decision matrix

The following is the architecture decision, not a benchmark result.

| Property | Retained Delta | Shared Mirror DAG | Retained DAG Direct FFI |
|---|---:|---:|---:|
| Restores 7v2 eager semantic DAG | yes | yes | **yes** |
| Stable subtree sends no payload | yes | yes | **yes** |
| Exact root sends no structure | yes | yes | **yes** |
| N-API structural traversal on hot path | no | no | **no** |
| Persistent transport graph | no | **yes** | **no** |
| Generic record encode/decode | **yes** | record write/read | **no for common nodes** |
| One structural FFI call | yes | yes | no, changed-node count |
| Variable payload zero-copy | mapped arena | mapped pages | **borrowed TypedArray** |
| Memory bounded independently of historical node churn | yes if caches cleaned | requires reclamation | **yes if caches cleaned** |
| Slot/page generation/ABA protocol | NativeRef only | **SharedRef + NativeRef** | **NativeRef only** |
| Persistent edge allocator | no | **yes** | **no** |
| Finalizer-driven transport reclamation | optional | **required** | **not required** |
| Native recovery without JS semantic re-read | limited | **best** | limited / exceptional |
| Reuses current 11v3 generated direct calls | partial | partial | **maximum** |
| New ABI surface complexity | medium/high | highest | **lowest** |
| Failure atomicity | staged commit | mirror publish + host commit | **native objects may preexist; host commits once** |
| Best fit for 20-200 node real TUI | good | overbuilt | **best** |
| Best response to 2.7 GiB benchmark concern | good | weakest | **best** |

The decision is therefore:

> **Use Retained DAG Direct FFI as PERF-12. Do not implement Shared Mirror as a competing full candidate. Do not implement the Delta record VM unless a later, isolated profile proves FFI call density is the remaining blocker.**

---

# 3. Source archaeology: what 7v2 actually did

The historical semantic reference is:

```text
e5292d62c4011610850cbdc1ba4a35f296f78e4f
```

Relevant source:

```text
packages/iyon-runtime/src/tui/values/view.ts
```

At that revision, the `View` constructor immediately did the semantic work:

```ts
private constructor(node) {
    nodes.set(this, withPrivateIdentity(node))
    Object.freeze(this)
}
```

`nodeForBridge(view)` was effectively a WeakMap lookup of the already-created semantic node.

The important properties were:

```text
View construction creates final BridgeViewNode immediately
BridgeViewNode is frozen
NodeId is assigned at semantic construction
parent BridgeViewNode references child BridgeViewNodes directly
unchanged child Views reuse the exact child BridgeViewNode object
nodeForBridge does not serialize/materialize a second representation
```

The direct native decoder then:

```text
reads NodeId first
checks environment NodeId -> WeakView
returns immediately on live cache hit
only on miss reads kind/payload/children
recursively cuts off at cached descendants
```

PERF-11v4's historical measurements are important because they show that this JavaScript semantic construction was not inherently the expensive part. The supplied benchmark handoff records approximately:

```text
small IDENTICAL_IDENTITY:
    total median         1,209 ns
    construction            42 ns
    native               1,166 ns

small SHARED_PATH:
    total median        38,667 ns
    construction         2,334 ns
    native              35,708 ns
```

The exact Bun 1.4 rerun remains authoritative, but the architecture lesson is already clear:

> Do not replace a cheap semantic representation merely to optimize the boundary after it.

---

# 4. Source archaeology: what current 11v3-era code does

At the re-audited `perf-refactor` revision:

```text
67741eb588e70ffe8ce7b08805040d0a9cc65f8c
```

`packages/iyon-runtime/src/tui/values/view.ts` explicitly describes a different model:

```text
stable-shape ViewBacking
state 0 = materialized semantic node
state 1 = pending create
state 2 = pending patch
```

The source states that pending values carry the recipe needed by the generated/native route and that `BridgeViewNode` is materialized lazily for Direct compatibility.

That architecture contains valuable native transport work, but it means:

```text
direct_current != direct_7v2
```

PERF-12 must not accidentally preserve the current pending representation and claim to have restored Candidate A.

The current `native_view_abi.ts` also shows how far the native-oriented operation language has grown. It contains/generated-links concepts such as:

```text
hostRenderRef
viewRefForNodeId
viewReleaseMany
pathRoot / pathChild
root and path scalar patches
row/column fixed-arity creation
axis builder begin/push/finish/abort
axis set/splice
Grid cell edit
edit transactions
style atoms / styles
cstring and byte text creation
```

Those are useful primitives.

They should become implementation machinery behind a semantic DAG, not the semantic representation itself.

---

# 5. Current native cache invariants that must survive

At the inspected revision, `NativeViewRuntime` owns the transport-neutral semantic state.

Conceptually:

```rust
NativeViewRuntime {
    nodes: HashMap<u64, WeakView>,
    slots: HashMap<u32, NativeViewSlot>,
    node_refs: HashMap<u64, u32>,
    path_nodes: ...,
    path_keys: ...,
    builders: ...,
    edit_txns: ...,
    style_atoms: ...,
    styles: ...,
    generation: ...,
}
```

The source explicitly says the semantic cache belongs to the environment runtime, not to Direct, packed, FastShared, generated FFI, or a host.

PERF-12 keeps that ownership rule.

There must never be:

```text
Perf12SemanticCache
MirrorSemanticCache
DirectDagSemanticCache
```

The authoritative semantic identity remains:

```text
NodeId -> WeakView
```

Every transport path must converge on one publication helper.

---

# 6. New finding: the 2.7 GiB result requires cache-lifetime attribution

The reported approximately 2.7 GiB stabilized RSS during PERF-11v4 is not enough evidence to call the current architecture unusable. Large benchmark fixtures, sample retention, JavaScriptCore heap policy, JIT code, allocator high-water, retained strings, and native caches can all contribute.

However, source inspection exposes a specific mechanism that must be measured.

Current runtime maps contain weak values:

```text
nodes:     NodeId -> WeakView
slots:     NativeRef -> { WeakView, optional leased View, js_lease_count, ... }
node_refs: NodeId -> NativeRef
```

Expired entries are removed on some lookup/resolve/release paths.

A transient NodeId or NativeRef that is never touched again after its `View` dies can therefore leave map metadata behind unless another cleanup path scavenges it.

This is a plausible explanation for part of a large benchmark high-water. It is **not yet proven to be the cause**.

It matters architecturally because an agent TUI can create a very large number of unique immutable semantic nodes over hours even if only 20-200 are live at one moment.

PERF-12 therefore treats weak-cache scavenging as a prerequisite shared-runtime fix, not as an optional memory micro-optimization.

---

# 7. Memory principle

The final design has this memory model:

```text
persistent JS memory:
    live View wrappers
    live BridgeViewNode DAG
    weak sidecar metadata
    PersistentSeq sidecars only where needed

persistent native memory:
    live Rust View graph reachable from roots/leases
    WeakView semantic cache metadata, scavenged
    NativeRef slot metadata, scavenged
    retained string/style payloads already justified by semantic reuse

transport memory:
    small bounded reusable TypedArrays
    no persistent node records
    no persistent edge pages
    no SharedRef graph
```

The architecture must not have memory proportional to:

```text
all semantic nodes ever created
all transport records ever allocated
all NativeRefs ever resolved
all old root versions
```

after weak state is known to be dead and maintenance has run.

---

# 8. Why the persistent Shared Mirror is rejected

The Shared Mirror is technically coherent. The rejection is architectural, not because it cannot work.

## 8.1 It duplicates a representation whose hot role is already served by NativeRef

After warm materialization, both designs can do:

```text
BridgeViewNode identity
    -> NativeRef
    -> Rust View
```

The mirror adds:

```text
BridgeViewNode identity
    -> SharedRef
    -> persistent node/edge/payload record
    -> NativeRef
    -> Rust View
```

The persistent record is not needed for the normal retained hit.

## 8.2 Its main unique win is exceptional recovery

The mirror can rebuild a Rust View from the shared record after the semantic WeakView/native acceleration has expired.

That is useful, but normal retained updates already hold a root lease for the previous native graph until the replacement root is complete. A stable subtree shared from the previous root therefore remains strongly reachable during the update.

The recovery case is mostly:

```text
dormant JS node reintroduced later
runtime generation reset
explicit cache/lifetime disturbance
```

Those paths can afford bounded recovery through the semantic JavaScript object.

## 8.3 It creates a new lifetime universe

A correct persistent mirror needs:

```text
node pages
edge pages / size classes
slot state
slot generation
SharedRef ABA prevention
publication/sealing
release queues
FinalizationRegistry integration
page high-water accounting
payload-ref lifetime
fragmentation accounting
runtime teardown ordering
```

All of this exists only to maintain a transport representation.

## 8.4 It is the wrong direction under current memory uncertainty

The current high RSS already requires cache attribution.

Adding persistent 128-byte node records, edge blocks, free lists, page high-water, and delayed finalizer reclamation before the existing weak-cache lifetime is understood would make memory analysis harder, not easier.

## 8.5 Mature frameworks retain semantic/native objects, not a permanent wire copy

React Native Fabric keeps immutable native ShadowNodes and direct native correspondence from the JS/Fiber layer. Flutter keeps persistent Element/RenderObject state behind immutable Widgets. Qt Quick synchronizes changed QML Items into a retained scene graph.

The recurring pattern is:

```text
cheap declaration
+
persistent native/renderer state
+
stable correspondence / dirty identity
```

not:

```text
cheap declaration
+
persistent serialized mirror
+
persistent native semantic graph
```

---

# 9. Why the full Delta packet is not the primary path

Retained Bridge Delta solves the memory problem much better than Shared Mirror because its arena is temporary and bounded.

It remains the better fallback concept if direct-call amplification ever becomes the measured bottleneck.

It is not the primary PERF-12 choice because it introduces a generic encode/decode layer that Bun 1.4 may make unnecessary.

For a common node, Delta does:

```text
read JS semantic fields
write fixed record words
write reference lanes
one commit call
read fixed record words in Rust
construct Rust semantic object
```

Direct FFI does:

```text
read JS semantic fields
call generated typed constructor
construct Rust semantic object
```

For fixed arity/scalars, the second path avoids the entire temporary structural representation.

The supplied Delta handoff itself correctly requires a `bridge_direct_nodes` control and explicitly says not to assume Delta wins. PERF-12 adopts that control as the architecture because it is the smallest representation stack consistent with the required semantics.

---

# 10. Mature-framework research conclusions

## 10.1 React Native Fabric

Relevant documentation:

- <https://reactnative.dev/architecture/render-pipeline>
- <https://reactnative.dev/architecture/landing-page>
- <https://reactnative.dev/docs/the-new-architecture/using-codegen>

Useful properties:

```text
JavaScript creates declarative element state
native C++ ShadowNodes are created synchronously
fibers keep native correspondence
ShadowNodes are immutable
updates clone changed paths
unchanged nodes are structurally shared
mounting is a separate atomic stage
```

The PERF-12 lesson is:

> Keep the declaration cheap and make retained native identity directly addressable.

Do not interpret Fabric as an instruction to move Iyon's public semantic `View` construction into Rust. The relevant part is the stable JS/native correspondence and structural sharing.

## 10.2 Flutter

Reference:

- <https://docs.flutter.dev/resources/inside-flutter>

Flutter separates immutable widgets from retained Element/RenderObject structures. Dirty elements are visited directly. An identical widget object allows an immediate cutoff by object identity.

The PERF-12 lesson is:

> The immutable declaration and retained renderer representation can differ, but clean identity must terminate work before recursive traversal.

This maps naturally to:

```text
BridgeViewNode identity -> NativeRef -> retained Rust View
```

## 10.3 Qt Quick scene graph

Reference:

- <https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html>

Qt Quick's synchronization step invokes scene-graph update work on Items that changed since the previous frame, while the renderer keeps a retained scene graph.

The PERF-12 lesson is:

> Synchronize changed semantic state into retained renderer state; do not rebuild a complete transport representation every frame.

## 10.4 SBE

Reference:

- <https://github.com/aeron-io/simple-binary-encoding/wiki/Design-Principles>

SBE emphasizes copy-free access, native type mapping, allocation-free codecs, streaming access, and aligned layouts.

PERF-12 uses the physical lesson only where a variable buffer is actually needed:

```text
reusable typed arrays
native u32 layout
no intermediate copy
synchronous borrowed lifetime
```

It does not need an SBE-like record protocol for fixed semantic nodes.

## 10.5 Chromium command buffers

Reference:

- <https://chromium.googlesource.com/chromium/src.git/+/HEAD/docs/security/research/graphics/gpu_command_buffer.md>

Chromium demonstrates both the power and the cost of mutable shared-memory command protocols: synchronization, TOCTOU concerns, validation, and explicit ownership become architectural requirements.

Iyon has a much easier baseline:

```text
same process
same owner thread
synchronous FFI
one producer
one consumer
no retained pointer after call
```

PERF-12 should keep that advantage.

## 10.6 Bun FFI

Reference:

- <https://bun.com/docs/runtime/ffi>

Current Bun documentation states that normal `dlopen`, `linkSymbols`, `CFunction`, and `JSCallback` call paths are implemented in JavaScriptCore and hot sites can compile to direct native calls. It also documents `buffer` and `buffer_length` engine-native argument forms.

More importantly, this is not merely a bet on later Bun documentation: the re-audited Iyon `tools/tui-abi/view_abi.toml` already declares both `minimum_bun = "1.4.0"` and `qualified_bun = "1.4.0"`, and its existing `view_axis_create_buffer` ABI already lowers the child buffer length through `buffer_length`. PERF-12 is extending a Bun 1.4 path that the repository has already qualified.

Important constraints:

```text
napi_env / napi_value are cc()-only
buffer pointers are borrowed for the call
raw memory lifetime is the caller's responsibility
bun:ffi remains experimental
```

The repository is already explicitly pinned to Bun 1.4.0, so PERF-12 continues the existing qualification strategy rather than widening runtime support.

---

# 11. Non-negotiable invariants

PERF-12 fails if any of the following are lost.

```text
[ ] full 53-bit safe NodeId semantic identity
[ ] one environment-owned NodeId -> WeakView semantic cache
[ ] stable JS identity for unchanged semantic subtrees
[ ] semantic identity cutoff before payload/child inspection
[ ] NativeRef acceleration
[ ] exact-root fast path
[ ] weak-cache expiry correctness
[ ] bounded cache metadata after churn
[ ] PersistentSeq wide edits
[ ] no O(width) one-child update at useful wide sizes
[ ] retained strings/styles
[ ] streaming text remains specialized
[ ] current full public View schema
[ ] History/ViewSlot/ScrollPane/components/animation parity
[ ] one host mutation after complete root materialization
[ ] generated ABI source of truth
[ ] same-image runtime/bootstrap architecture
[ ] checked and timing builds remain distinct
[ ] cold/rebuilt complete fallback remains available
```

---

# 12. Identity model

PERF-12 uses two persistent identities and one transaction-local state.

## 12.1 NodeId

```text
range: 1 .. 2^53 - 1
role: semantic identity
lifetime: semantic node lifetime / never reused logically
authority: NativeViewRuntime NodeId -> WeakView cache
```

A new semantic node gets a new NodeId.

An unchanged shared node retains the same NodeId because it is the same immutable BridgeViewNode.

NodeId is never replaced by NativeRef.

## 12.2 NativeRef

```text
type: u32
role: fast environment-local acceleration handle
scope: one NativeViewRuntime generation
meaning: may resolve to an existing Rust View
```

A NativeRef is not semantic equality.

A stale NativeRef is a cache miss, not corruption.

Do not recycle NativeRef numeric values inside one runtime generation unless a future design adds explicit per-slot generations. Monotonic non-reuse avoids ABA and keeps the JS sidecar small.

## 12.3 Transaction-local state

During one `ensureNativeRoot` operation, PERF-12 uses reusable private state for:

```text
in-progress nodes / cycle guard
newly-created refs
references borrowed by variable-arity calls
temporary leases
stale-ref retry information
materialized node counter
```

No transaction-local identifier escapes the call.

There is no persistent `SharedRef`.

---

# 13. JavaScript semantic representation

The normal semantic representation returns to the historical model.

Conceptually:

```ts
const BRIDGE_NODES = new WeakMap<View, BridgeViewNode>()

class View {
  readonly kind = "view"

  private constructor(node: BridgeViewNodeDraft) {
    const semantic = withPrivateIdentity(node)
    BRIDGE_NODES.set(this, semantic)
    Object.freeze(this)
  }
}

function nodeForBridge(view: View): BridgeViewNode {
  const node = BRIDGE_NODES.get(view)
  if (node === undefined) throw ...
  return node
}
```

This code should be reconstructed mechanically from the historical SHA and adapted to the current schema.

Do not re-derive it from this prose.

---

# 14. BridgeViewNode object shape

A normal BridgeViewNode contains semantic fields only.

```text
id
schema
kind
semantic payload
semantic children
```

Do not add own properties such as:

```text
nativeRef
nativeGeneration
pathRef
wireIndex
sharedSlot
transportState
leaseCount
```

Transport data belongs in WeakMap sidecars.

This preserves the reason to expect JavaScriptCore to keep common per-kind object structures monomorphic.

---

# 15. Sidecars

The baseline sidecars are deliberately small.

```ts
interface BridgeNativeHint {
  readonly generation: number
  readonly nativeRef: number
}

const BRIDGE_NATIVE = new WeakMap<BridgeViewNode, BridgeNativeHint>()
```

Optional derivation metadata:

```ts
type BridgeDerivation =
  | TextLayoutDerivation
  | CommonScalarDerivation
  | AxisEditDerivation
  | GridEditDerivation

const BRIDGE_DERIVATION = new WeakMap<BridgeViewNode, BridgeDerivation>()
```

Wide-only sequence state:

```ts
const BRIDGE_SEQUENCE = new WeakMap<BridgeViewNode, BridgeSequenceOverride>()
const BRIDGE_GRID_SEQUENCE = new WeakMap<BridgeViewNode, BridgeGridOverride>()
```

No sidecar should be created merely to construct a plain semantic View unless the metadata is actually useful.

Construction cost is an explicit gate.

---

# 16. Critical lifetime decision: NativeRef sidecars are hints, not per-node leases

This is a major difference from both proposed PERF-12 handoffs.

`BRIDGE_NATIVE` must **not** keep one strong native `View` lease per live JavaScript BridgeViewNode.

Instead:

```text
BridgeNativeHint:
    weak acceleration only

View-bearing boundary:
    owns one root NativeRef lease

materialization transaction:
    owns temporary leases for newly-created nodes
```

Why this works:

1. The current boundary root lease strongly keeps the current Rust View graph alive.
2. During an update, the old root lease is not released until the new root has been completely materialized and installed.
3. Therefore any stable subtree reused from the old root remains native-live while its NativeRef is needed.
4. Newly-created nodes receive temporary leases while the new path is assembled.
5. After successful root installation, only the new root lease remains. Child temporary leases are released in one batch.
6. Rust graph ownership from the new root keeps its descendants alive.

This gives hot retained stability without a FinalizationRegistry per semantic node.

A dormant JS node that is no longer reachable from a native root may later contain a stale NativeRef hint. That is allowed and handled as a cache miss.

---

# 17. Why FinalizationRegistry is not baseline lifetime machinery

Finalizers are useful for cleanup hints but poor correctness clocks.

The Shared Mirror design needs finalization/release to reclaim persistent record slots.

Retained DAG Direct FFI does not create persistent transport resources per BridgeViewNode, so it does not need that dependency.

The baseline rule is:

```text
WeakMap hint disappears with JS object
NativeRef slot is weak unless a boundary/temp lease owns it
runtime scavenger removes dead slot/cache metadata
```

If a future specialized payload handle truly requires JS-lifetime ownership, use an explicit audited mechanism for that payload only.

Do not make the whole View bridge depend on GC finalizer scheduling.

---

# 18. Root lease protocol

Every View-bearing native boundary already needs a clear ownership rule.

Conceptually:

```text
boundary.previousRef = leased NativeRef for currently installed root
```

Update:

```text
1. keep previousRef leased
2. materialize next root
3. hostRenderRef(nextRef)
4. if success:
       release previousRef
       transfer nextRef temporary lease to boundary.previousRef
       release every other temporary ref
5. if failure:
       keep previousRef
       release every temporary ref
```

Close/dispose:

```text
release boundary.previousRef exactly once
```

After each successful boundary commit, also capture the private semantic NodeId allocator high-water as `boundary.nativeLookupCeiling`. This is transport metadata on the boundary/session, not on BridgeViewNode. It lets a later sidecar miss distinguish definitely-new NodeIds from older nodes that may already exist in the native semantic cache.

This protocol must be shared across:

```text
Tui root
History-held View roots where applicable
ViewSlot
ScrollPane
animation target/current View
component View boundaries
any future native View-bearing boundary
```

---

# 19. `ensureNative` algorithm

The central JS algorithm is identity-first.

Conceptual pseudocode:

```ts
function ensureNative(
  node: BridgeViewNode,
  tx: MaterializeTx,
): number {
  const hint = BRIDGE_NATIVE.get(node)
  if (hint !== undefined && hint.generation === tx.generation) {
    tx.noteBorrowedHint(node, hint.nativeRef)
    return hint.nativeRef
  }

  const local = tx.refs.get(node)
  if (local !== undefined) return local

  // A previous cold/direct/native path may have materialized this semantic
  // NodeId without ever installing a JS-side NativeRef hint. Only probe the
  // native NodeId cache for nodes that existed before the previous successful
  // boundary commit; genuinely new nodes skip this extra FFI call.
  if (node.id <= tx.nativeLookupCeiling) {
    const recovered = tryNativeRefForNodeId(node.id, tx)
    if (recovered !== undefined) {
      BRIDGE_NATIVE.set(node, {
        generation: tx.generation,
        nativeRef: recovered,
      })
      tx.refs.set(node, recovered)
      tx.temporaryLeases.push(recovered)
      return recovered
    }
  }

  if (tx.inProgress.has(node)) throw cycleError()
  if (++tx.newNodeCount > MAX_RETAINED_NEW_NODES) throw FastFallback()

  tx.inProgress.add(node)
  try {
    const ref = tryDerivation(node, tx)
      ?? generatedMaterializeNode(node, tx)

    BRIDGE_NATIVE.set(node, {
      generation: tx.generation,
      nativeRef: ref,
    })
    tx.refs.set(node, ref)
    tx.temporaryLeases.push(ref)
    return ref
  } finally {
    tx.inProgress.delete(node)
  }
}
```

Hard ordering rule:

```text
BRIDGE_NATIVE lookup
then, when eligible, NodeId -> NativeRef promotion
before
node.kind / payload inspection
before
child traversal
```

`nativeLookupCeiling` is captured from the private monotonic NodeId allocator after each successful boundary commit. Nodes created after that commit have larger NodeIds and therefore skip the NodeId-probe call; older dormant/stable nodes can recover a native ref without a semantic walk. If unrelated Views were constructed before the commit but never materialized, the probe may miss once and normal materialization continues.

This preserves Candidate A's identity-before-payload rule even when the previous root came from a cold/direct fallback that did not populate descendant sidecars.

---

# 20. Exact root fast path

Rendering the same known root should be almost trivial.

```ts
const node = nodeForBridge(next)
const hint = BRIDGE_NATIVE.get(node)

if (hint?.generation === session.generation) {
  const status = hostRenderRef(session, host, hint.nativeRef)
  if (status === OK) return hint.nativeRef
  if (status === CACHE_MISS) return recoverKnownRoot(...)
  throwStatus(status)
}
```

Required structural counters:

```text
semantic node fields read: 0
children visited: 0
TypedArray words written: 0
node constructor FFI calls: 0
host FFI calls: 1
```

Exact identity must be independent of descendant count.

---

# 21. Stable subtree cutoff

For:

```text
new root R2
+-- changed branch C2
`-- stable subtree S
```

JavaScript:

```text
ensureNative(R2)          miss
ensureNative(C2)          miss
...
ensureNative(S)           NativeRef hint hit -> stop
```

No descendant of `S` may have its semantic payload read.

Native parent construction receives only `S`'s u32 NativeRef.

The native constructor resolves that ref to the existing Rust View.

No N-API object is involved.

---

# 22. Direct FFI materialization

Generated direct materialization is children-first.

Example fixed node:

```c
uint32_t iyon_view_container_create_v2(
    NativeViewRuntime *runtime,
    uint32_t node_id_low,
    uint32_t node_id_high,
    uint32_t child_ref,
    uint32_t padding_top_right,
    uint32_t padding_bottom_left,
    uint32_t width_rule,
    uint32_t height_rule,
    uint32_t min_width,
    uint32_t max_width,
    uint32_t min_height,
    uint32_t max_height
);
```

Exact signature comes from the canonical schema and current semantic type layout.

Do not copy this sample signature blindly.

Generated TypeScript wrapper:

```ts
function materializeContainer(node: BridgeContainerNode, tx: MaterializeTx): number {
  const childRef = ensureNative(node.child, tx)
  const [lo, hi] = nodeIdPair(node.id)
  return viewContainerCreate(
    tx.symbols,
    tx.runtime,
    lo,
    hi,
    childRef,
    ...
  )
}
```

There is no generic record object.

---

# 23. Native constructor rule: semantic cache first

Every generated constructor receives the new semantic NodeId.

Native should first ask whether this NodeId already has a live semantic View.

Conceptually:

```rust
fn create_container(..., node_id: u64, child_ref: u32, ...) -> FastResult<u32> {
    if let Some(existing) = runtime.lookup_live_node(node_id) {
        return runtime.ensure_ref(node_id, existing);
    }

    let child = runtime.resolve_ref(child_ref)?;
    let view = View::container(..., child, ...)?;
    runtime.publish_semantic_view(node_id, view)
}
```

This protects cross-transport/recovery behavior.

It does not replace the JS identity cutoff; JS should not intentionally send stable nodes merely because native can rediscover them.

---

# 24. One publication helper

Refactor if necessary so all transports use one semantic publication implementation.

Conceptual:

```rust
fn publish_semantic_view(
    &mut self,
    node_id: u64,
    view: View,
    lease: LeaseMode,
) -> Result<u32, FastStatus>
```

Responsibilities:

```text
validate NodeId
reject impossible semantic identity conflict
insert/update NodeId -> WeakView
allocate NativeRef slot
associate NodeId -> NativeRef
return NativeRef
apply requested temporary/root lease count
update diagnostics
```

Direct N-API, existing generated FFI, PERF-12, and fallback recovery must not have separate identity rules.

---

# 25. Reuse current 11v3 functions before adding new ones

The current ABI already contains valuable primitives.

Examples include:

```text
hostRenderRef
viewSpacerCreate
viewTextCreateCstring / Utf8 variants
viewTextLayoutPatchRoot/path
viewCommonPatchRoot
viewRowCreate0..4
viewColumnCreate0..4
viewAxisCreateBuffer
axis builder
viewAxisSetChild
viewAxisSpliceBuffer
viewGridSetCell
style creation
viewRefForNodeId
viewReleaseMany
```

PERF-12 should classify each current function:

```text
A. semantic constructor/edit that remains useful unchanged
B. useful implementation that needs a generated signature cleanup
C. path/recipe machinery made redundant by the semantic DAG
D. benchmark/fallback only
```

Do not rewrite a native constructor merely to rename it PERF-12.

---

# 26. What should disappear after adoption

If PERF-12 wins, the production semantic layer should not retain native-oriented state merely to route common operations.

Candidates for removal or test-gating after soak include machinery whose only purpose is the current pending recipe architecture:

```text
pending create backing
pending patch backing
native scalar patch state that duplicates semantic node information
path lineage used only to compensate for missing semantic DAG identity
builder state that is only a cold construction workaround
transport-specific semantic materialization state
```

Do not remove anything until full public-API parity and performance are proven.

The cleanup tranche decides from actual usage, not this list alone.

---

# 27. Derivation hints

The semantic DAG is authoritative, but construction can cheaply record how a new immutable node was derived from an old one.

That is an optimization hint, not a second representation.

Example:

```ts
interface TextLayoutDerivation {
  readonly kind: "textLayout"
  readonly base: BridgeViewNode
  readonly wrap: number
  readonly align: number
}
```

A modifier still creates the complete new semantic BridgeViewNode eagerly.

Then it may set:

```ts
BRIDGE_DERIVATION.set(newNode, derivation)
```

`ensureNative()` tries the derivation only if:

```text
new node has no NativeRef
base node has a same-generation NativeRef
operation has an exact native retained primitive
```

Otherwise it ignores the hint and materializes from semantic fields.

---

# 28. Why derivation hints are worth keeping

They preserve important post-7v2 work without making the JS representation native-first.

Examples:

```text
Text wrap/align change:
    base NativeRef + new NodeId + wrap/align
    no text payload resend

common scalar layout/decor change:
    base NativeRef + changed scalar mask
    no unchanged child/payload resend

axis child replace:
    base NativeRef + new NodeId + index + new child NativeRef
    native PersistentSeq update

Grid cell replace:
    base NativeRef + new NodeId + coordinate + new child NativeRef
```

The architecture remains:

```text
semantic object first
optimization hint second
```

not:

```text
native recipe first
semantic object lazily reconstructed later
```

---

# 29. Variable-arity transport: borrowed TypedArrays, not mapped pages

Variable child lists need contiguous reference storage somewhere.

Use reusable JavaScript-owned typed arrays and Bun's engine-native `buffer` / `buffer_length` arguments.

Example:

```ts
const refs = session.refScratch.ensureCapacity(count)
for (let i = 0; i < count; i++) {
  refs[i] = ensureNative(children[i]!.child, tx)
}

viewAxisCreateBuffer(
  runtime,
  nodeIdLow,
  nodeIdHigh,
  axisKind,
  gap,
  refs.subarray(0, count),
)
```

Native contract:

```text
pointer valid only for synchronous call
length validated
Rust never stores pointer/slice after return
Rust copies/uses semantic child Views, not transport bytes
```

This is zero-copy at the FFI boundary: native reads the same TypedArray storage.

It is not zero-work: JavaScript must write the NativeRef words for a newly-created variable-arity parent. That work is counted as transport preparation.

---

# 30. Scratch-memory policy

Do not let one giant benchmark case permanently inflate normal-session scratch memory.

Use explicit tiers.

Suggested starting policy:

```text
ref scratch small:       1,024 u32 = 4 KiB
ref scratch medium cap:  8,192 u32 = 32 KiB
aux scratch cap:         8,192 u32 = 32 KiB
byte scratch cap:       65,536 u8  = 64 KiB
```

Rules:

```text
small scratch allocated once
medium scratch allocated only when needed
retained fast path refuses pathological cold payload above cap
large/cold input routes to existing complete fallback
no pointer retained by native
no native external ArrayBuffer/deallocator required
```

Final values are benchmark decisions.

---

# 31. Why this is preferable to native-owned mapped scratch for baseline PERF-12

Native-owned mapped memory is valid, but unnecessary when Bun already passes TypedArray backing memory directly to engine-native FFI.

Avoiding external mapped memory removes:

```text
pointer export API
toArrayBuffer lifetime
deallocator context
teardown ordering between JS mapping and Rust allocation
arena state machine
persistent native scratch allocation
```

If later profiling proves JavaScript-owned TypedArray placement materially slower than native-owned mapped storage, that is a small physical-memory experiment, not a semantic-architecture rewrite.

---

# 32. Fixed arity specialization

Keep generated fixed-arity Row/Column constructors for common arities.

Baseline:

```text
arity 0..4:
    scalar NativeRef arguments
    no ref buffer

arity >4 and <= retained cap:
    borrowed ref buffer
```

The common small TUI therefore pays no array write merely to cross the boundary.

The exact specialization limit should come from the existing 11v3 data and PERF-12 transport microbenchmarks.

---

# 33. Wide structures are a separate semantic optimization

PERF-12 must not regress the current `PersistentSeq` work.

Current JS `PersistentSeq` uses branch factor 32 and path-copying operations.

The retained wide update must therefore remain approximately:

```text
O(log_32 N)
```

for:

```text
replace
insert
remove
splice
Grid cell replacement where applicable
```

Do not force wide semantic edits through a flat historical array merely for purity.

---

# 34. Wide semantic sidecar exception

For ordinary node sizes, the BridgeViewNode itself is the complete historical-style semantic object.

For deliberately wide retained edits, PERF-12 allows one private exception:

```ts
interface BridgeSequenceOverride {
  readonly baseNode: BridgeViewNode
  readonly sequence: PersistentSeq<BridgeLayoutChild>
  readonly edit: AxisSequenceEdit
}
```

stored in:

```ts
WeakMap<BridgeViewNode, BridgeSequenceOverride>
```

The visible/frozen object shape remains stable.

The native fast path treats the sequence sidecar as authoritative for the wide edit.

Direct N-API fallback may lazily materialize an exact flat BridgeViewNode only if fallback is actually taken.

This is preferable to making every normal 20-200-node path pay persistent-sequence representation overhead.

---

# 35. Wide native edit path

For one child replacement:

```text
base axis NativeRef
new axis NodeId
index
new child NativeRef
track word if needed
```

Native:

```text
resolve base View
resolve child View
PersistentSeq::set
construct new axis View
publish new NodeId / NativeRef
```

For insert/remove/splice:

```text
base axis NativeRef
new NodeId
index
remove count
only inserted child NativeRefs
```

No old full child list crosses FFI.

No 100,000-element scan.

---

# 36. Grid

Apply the same split.

Normal small/new Grid:

```text
generated direct materializer
borrowed track/cell/ref arrays if needed
```

Retained cell edit:

```text
base Grid NativeRef
new Grid NodeId
row/column or canonical index
new child NativeRef
native persistent grid/sequence edit
```

Retained track change should use a similarly narrow operation if it is common enough to justify it.

Do not create a persistent shared Grid edge arena.

---

# 37. Text

Text is too important to hide behind a generic structural protocol.

PERF-12 reuses the best current text machinery.

Common one-span path:

```text
BridgeTextNode
    -> cstring-capable generated direct FFI
    -> retained native text
```

Exact-byte path:

```text
BridgeTextNode
    -> reusable UTF-8 scratch / buffer + byte length
    -> native retained text
```

Multi-span common arities may keep generated specialized calls where current 11v3 evidence justifies them.

---

# 38. Text layout mutation

A wrap/alignment-only mutation must not resend stable text.

Construction:

```text
new complete BridgeTextNode
same spans array identity where semantics permit
new NodeId
BRIDGE_DERIVATION = textLayout(base, wrap, align)
```

Native fast path:

```text
base NativeRef
new NodeId
wrap
align
```

This reuses the native text payload already retained by the base View.

If the base NativeRef is unavailable, fall back to direct semantic materialization of the new text node.

---

# 39. Strings and embedded NUL

`cstring` cannot be the only path.

Correctness suite includes:

```text
empty string
ASCII
short Unicode
emoji / non-BMP
combining sequences
embedded NUL
lone surrogate normalization behavior
U+10FFFF
256-byte text
4-KiB text
large Diff content
```

Reuse the existing exact byte-length fallback.

Do not introduce JS UTF-8 encoding for cases where current Bun/native string conversion is faster.

Choose by end-to-end benchmark.

---

# 40. Styles

Reuse the current native `StyleRef` / style-atom system.

Style identity remains a separate retained payload concern from structural View identity.

Rules:

```text
stable style object -> reuse StyleRef
stable text subtree -> NativeRef cutoff before style inspection
changed text span -> resolve only styles for changed/new payload
no style bytes in a generic structural packet
```

Do not create a second PERF-12 style cache.

---

# 41. Diff

Diff can contain much larger semantic payload than normal structural nodes.

Treat Diff like Text:

```text
structure node identity
+
specialized payload import / retained payload
```

Do not force all Diff lines/hunks through a generic View-record format.

If current native Diff construction does not have a retained payload handle, first benchmark:

```text
direct generated buffer import
N-API payload-only import
retained DiffPayloadRef
```

Only add `DiffPayloadRef` if it reduces real repeated work.

---

# 42. Streaming text

Streaming text remains outside the structural View bridge.

This separation is not a leftover of an abandoned architecture; it is one of
the confirmed wins of PERF-8 through PERF-11v3. The native streaming pipeline
(native `TextStream` append/seal, incremental stream compilation,
source-rooted offsets/revisions, frozen/live History units) produced visibly
smoother assistant-stream rendering and is transport-independent: it does not
touch the View bridge in any candidate. PERF-12 preserves it unchanged, and
the PERF-11v4 report records this as a keep-regardless outcome.

The structural DAG may contain the View/component that references a stream.

The stream's appended bytes use the existing specialized native append path.

Never route each chunk through:

```text
new BridgeViewNode
new direct node constructor
or
structural scratch buffer
```

merely for architectural uniformity.

---

# 43. Multi-branch retained updates

The immutable DAG naturally describes the union of changed branches.

Example:

```text
new root
+-- changed left
|   `-- stable leaf
+-- stable center
`-- changed right
```

`ensureNative` visits only the newly-created objects reachable from the root.

Transaction-local identity guarantees that a new shared object referenced from two places is materialized once.

Unlike the Delta packet, there is no need to emit a definition table first.

Each node becomes a native semantic object as soon as its children are available.

---

# 44. Temporary lease transaction

Direct per-node constructors return NativeRefs that must remain valid while their parents are built.

Use one materialization transaction:

```ts
interface MaterializeTx {
  refs: Map<BridgeViewNode, number>
  inProgress: Set<BridgeViewNode>
  temporaryLeases: ReusableRefList
  borrowedHints: ReusableBorrowedHintList
  nativeLookupCeiling: number
  newNodeCount: number
  retryCount: number
}
```

Rules:

```text
constructor success -> returned ref is temporarily leased
parent construction may resolve child ref safely
do not release any newly-created ref until the root is complete
on host success -> keep root lease, release all other temp refs
on any failure -> release all temp refs
```

Use `viewReleaseMany`, not one FFI release call per ref.

---

# 45. Host atomicity does not require cache-publication atomicity

This is an important simplification over a packet transaction.

Native constructors may successfully publish immutable semantic Views before a later ancestor fails.

That is acceptable because:

```text
a published View is complete and immutable
NodeId identity is valid
host state has not changed yet
orphaned Views are weakly reclaimable
future retry may reuse them
```

The atomic requirement is:

> **No View-bearing host/boundary changes until the complete new root exists.**

Therefore PERF-12 does not need to stage an entire changed closure merely to make semantic cache publication all-or-nothing.

---

# 46. Stale NativeRef hints

A `BridgeNativeHint` is allowed to become stale.

This can happen when:

```text
node is no longer in any leased native root
the Rust View expires
slot is scavenged
same JS node is reintroduced later
```

This is not a correctness error.

Native functions return `FAST_CACHE_MISS` / equivalent detail.

The common retained path should almost never see this for stable descendants of the currently installed previous root, because that root remains leased until replacement succeeds.

---

# 47. Stale-child recovery

Do not add one validation FFI call for every NativeRef hint.

Optimistically use the hint.

If a parent constructor reports a stale child ref, the generated/native status must identify the failed ref or child ordinal.

Transaction-local recovery:

```text
1. identify the BridgeViewNode corresponding to stale child
2. delete/ignore its BRIDGE_NATIVE hint
3. materialize that child from its semantic object
4. retry the parent once
```

For exact-root `hostRenderRef` miss, the caller already knows the root node and can rematerialize it.

Hard rule:

```text
one targeted retry per operation
```

If recovery still fails, route to the authoritative complete fallback.

---

# 48. Runtime generation

Every hint contains the environment generation.

```ts
if (hint.generation !== session.generation) {
  ignore hint
}
```

No old NativeRef is used after runtime recreation.

Do not attempt cross-generation remapping.

Rebuild lazily from the semantic DAG.

---

# 49. Cold and rebuilt trees

Retained DAG Direct FFI is optimized for retained identity.

It should not be forced to construct a 10,000-node completely cold graph through 10,000 FFI calls merely to avoid a fallback.

Router:

```text
no previous native root / initial cold render:
    best complete cold candidate from PERF-11v4/11v3 evidence

retained root with shared identities:
    Retained DAG Direct FFI

retained walker exceeds MAX_RETAINED_NEW_NODES:
    abort retained attempt
    use cold/bulk fallback
```

Successful direct constructors that happened before the budget was exceeded are valid semantic cache entries and may be reused by the fallback.

Do not roll them back solely because the router changed.

---

# 50. Retained work budget

Set an explicit cap for the path optimized by PERF-12.

Initial benchmark candidate:

```text
MAX_RETAINED_NEW_NODES = 256 or 512
MAX_RETAINED_DEPTH     = 256
MAX_DIRECT_AXIS_REFS   = 1,024 or benchmark-derived
```

These are not public semantic limits.

Exceeding a cap returns `FAST_FALLBACK` and invokes the complete path.

Final values come from realistic trace distributions and cold crossover benchmarks.

---

# 51. No full-tree diff

Never compare old/new semantic trees by content to discover changes.

The immutable API already encodes the answer:

```text
same BridgeViewNode identity -> same semantic node
new BridgeViewNode identity  -> new semantic node
NativeRef hint               -> already materialized candidate
BRIDGE_DERIVATION            -> optional cheaper construction path
```

No hashes.

No recursive equality.

No virtual-DOM diff layer.

---

# 52. NativeRef table: remove HashMap from the hottest handle lookup

Current source uses:

```rust
slots: HashMap<u32, NativeViewSlot>
```

PERF-12 makes NativeRef lookup central enough that a dense/paged table should be measured and likely adopted for the shared runtime.

Recommended non-reusing monotonic-ref shape:

```rust
const PAGE_BITS: u32 = 10; // also benchmark 12
const PAGE_SIZE: usize = 1 << PAGE_BITS;

struct NativeRefPage {
    slots: Box<[Option<NativeViewSlot>; PAGE_SIZE]>,
    live: u32,
}

struct NativeRefTable {
    pages: Vec<Option<Box<NativeRefPage>>>,
}
```

Lookup:

```text
page = ref >> PAGE_BITS
offset = ref & (PAGE_SIZE - 1)
vector bounds
page pointer
slot index
```

No hash on common NativeRef resolution.

---

# 53. Do not recycle NativeRef IDs inside a generation

Ref reuse saves numeric space but creates ABA requirements for JS hints.

The current ABI uses a `u32` carrier but reserves the high bit for status, so valid ViewRefs are `1..0x7fffffff`. A monotonic 31-bit ref space is still very large for one environment lifetime.

With 4,096-slot pages, the theoretical maximum directory is 524,288 page pointers, which is manageable if pages themselves are allocated lazily and empty pages can be freed.

Therefore baseline PERF-12 uses:

```text
monotonic NativeRef
no per-slot generation
no ref reuse before environment reset
```

If exhaustion can be demonstrated in a realistic long-lived process, solve it as a separate design with an explicit generation pair rather than silently reusing refs.

---

# 54. NativeRef page reclamation

A page is physical metadata storage, not semantic state.

Track `live` slot count.

When a slot is removed:

```text
page.live--
if page.live == 0:
    page entry may be dropped
```

A stale JS NativeRef into a dropped page simply returns cache miss.

The outer `Vec<Option<Page>>` can keep its high-water directory length; with large page sizes its maximum is bounded and far smaller than retaining all historical slot objects.

---

# 55. Weak semantic cache scavenging

The `NodeId -> WeakView` cache must not grow with every historical NodeId forever.

Add central maintenance.

Suggested model:

```text
fast path:
    remove expired entry when directly observed

release path:
    enqueue zero-lease refs as scavenging candidates

periodic maintenance:
    process a bounded candidate budget

threshold backstop:
    when weak-cache metadata growth since last full sweep exceeds threshold,
    perform one full expired-weak sweep outside the tiny timing path
```

The exact implementation may differ, but the invariant is mandatory:

```text
post-GC/post-maintenance metadata = O(live semantic state + bounded sweep slack)
```

not:

```text
O(all NodeIds ever created)
```

---

# 56. Suggested weak-cache maintenance counters

Expose debug/counter-build fields such as:

```text
semantic_cache_entries
semantic_cache_live
semantic_cache_expired_seen
semantic_cache_full_sweeps
semantic_cache_entries_removed
native_ref_slots
native_ref_leased_slots
native_ref_unleased_live_slots
native_ref_expired_slots
native_ref_pages
native_ref_pages_freed
node_ref_map_entries
scavenge_queue_len
scavenge_processed
```

Counters must be absent or compile-time-cheap in authoritative timing builds.

---

# 57. PERF-12.0: memory attribution protocol

Before architecture implementation, reproduce the approximately 2.7 GiB case under the frozen PERF-11v4 binary/harness.

For each benchmark block record:

```text
RSS
process heap used/total
external / ArrayBuffer bytes where available
native semantic-cache entry count
native slot count
leased slot count
node_ref count
path node/key counts
builder count
edit transaction count
style/string retained counts
raw benchmark sample storage size
fixture live count
```

Then run a forced cleanup checkpoint:

```text
release/close all benchmark roots
Bun.gc(true)
native maintenance / full weak sweep
Bun.gc(true)
record counters again
```

Bun documents `Bun.gc(true)` as synchronous GC and its benchmarking documentation recommends heap snapshots for retained-object analysis.

If JS heap remains unexpectedly high, emit a Bun/JSC heap snapshot.

If native counters remain high while live roots are low, fix runtime lifetime before proceeding.

---

# 58. Memory classification

Classify the 2.7 GiB behavior into one or more buckets.

```text
A. benchmark result/sample retention
B. benchmark fixtures intentionally still live
C. JavaScript semantic objects / pending backings
D. JSC/JIT/allocator high-water with low live heap
E. native View strong leases
F. expired NodeId WeakView map metadata
G. expired NativeRef/node_ref metadata
H. PersistentSeq structural high-water
I. retained string/style payload
J. other native allocation
```

Do not use RSS alone to decide which architecture is responsible.

---

# 59. Memory acceptance gate

Run a long churn test after PERF-12 runtime cleanup is implemented.

Example:

```text
1,000,000 transient semantic Views
retain 1 in every 10,000
live rendered tree approximately 200 nodes
periodic root replacement
periodic wide edits
periodic text replacement
```

Every 100,000 operations:

```text
close transient roots
Bun.gc(true)
native maintenance
record live counters
```

Required:

```text
semantic_cache_entries = O(live + sweep_slack)
native_ref_slots        = O(live/native-reachable + sweep_slack)
leased slots            = root-boundary leases + in-flight temp leases only
transport persistent bytes = 0
scratch bytes remain within configured caps
post-maintenance native metadata shows no linear slope with historical operations
```

RSS may remain above the live allocation total because allocators/JITs retain address space. Judge both RSS and explicit live-state counters.

---

# 60. Bun 1.4 qualification

The current repository pins:

```json
"packageManager": "bun@1.4.0",
"bun-types": "1.4.0"
```

PERF-12 must record both:

```text
bun --version
bun --revision
```

for every authoritative result.

Do not silently benchmark a newer global Bun.

The same exact runtime revision is used for:

```text
direct_7v2
native_11v3
retained_dag_ffi
```

---

# 61. Same-image rule

Keep PERF-11v3's architecture:

```text
Node-API loads iyon-native.node
Node-API owns/returns NativeViewRuntime pointer
Node-API bootstrap returns function pointers
Bun linkSymbols binds those same-image pointers
```

Do not `dlopen()` a second copy of the native library.

All paths must see the exact same:

```text
NativeViewRuntime
semantic cache
NativeRef table
styles
strings
hosts
fallback state
```

---

# 62. Generated ABI source of truth

Continue using:

```text
tools/tui-abi/view_abi.toml
tools/tui-abi-gen
```

Do not hand-maintain Rust/C/TypeScript signatures separately.

PERF-12 extends the generator for semantic direct materializers and any new status detail fields.

Do not create a second PERF-12 generator.

---

# 63. Generator model additions

The exact model should follow the current generator style, but it needs enough metadata to generate semantic constructors/edits.

Conceptually:

```rust
struct MaterializerSpec {
    name: String,
    bridge_kind: String,
    rust_builder: String,
    fields: Vec<MaterializerFieldSpec>,
    result: MaterializerResultSpec,
}

struct MaterializerFieldSpec {
    name: String,
    source: String,
    abi_type: AbiType,
    role: MaterializerFieldRole,
}

enum MaterializerFieldRole {
    NodeIdLow,
    NodeIdHigh,
    Scalar,
    ChildRef,
    RefBuffer,
    AuxBuffer,
    ByteBuffer,
    StyleRef,
    BaseRef,
}
```

Use strongly typed serde models with `deny_unknown_fields` like the existing generator.

---

# 64. Generator validation

Generation must fail for:

```text
unknown BridgeViewNode kind
missing full-schema materializer/fallback declaration
u64 field narrowed into one u32
NodeId without both low/high halves where required
child semantic field not represented by NativeRef/buffer
buffer without explicit bounded length
FFI function missing ownership/borrow duration
constructor that can retain a borrowed buffer
unsupported enum source
duplicate ABI function name
semantic constructor missing benchmark/conformance registration
```

The generator should make illegal lifetime declarations hard to express.

---

# 65. Generated outputs

Prefer extending current output families.

Possible additions:

```text
packages/iyon-runtime/src/tui/generated/view_materialize.ts
packages/iyon-runtime/src/tui/generated/view_materialize_calls.ts
crates/iyon-native/src/generated/view_materialize.rs
crates/iyon-native/include/iyon_view_materialize.h
```

If current generated files can cleanly contain these functions, do not create new files only for naming symmetry.

Keep one manifest hash/ABI handshake.

---

# 66. Generated TypeScript materializers

Per-kind functions should be monomorphic and explicit.

Good:

```ts
function materializeContainer(node: BridgeContainerNode, tx: MaterializeTx): number {
  const child = ensureNative(node.child, tx)
  const [lo, hi] = splitNodeId(node.id)
  return viewContainerCreate(
    tx.symbols,
    tx.runtime,
    lo,
    hi,
    child,
    node.paddingTopRight,
    node.paddingBottomLeft,
    node.widthRule,
    node.heightRule,
    node.minWidth,
    node.maxWidth,
    node.minHeight,
    node.maxHeight,
  )
}
```

Avoid:

```text
Object.entries
Object.keys
reflective field lists
spread-based transport objects
generic encode(kind, object)
per-node closure allocation
fresh TypedArray per node
DataView per scalar field
```

Large generated source is acceptable.

Runtime reflection is not.

---

# 67. Native implementation ownership

Generated code should own:

```text
ABI signature
argument lowering
NodeId reconstruction
bounds checks
buffer length checks
enum discriminant validation
status conversion
```

Handwritten/native semantic helpers own:

```text
constructing current iyon-tui values
PersistentSeq operations
string/style lookup
semantic cache publication
root lease semantics
fallback selection where native-owned
```

Do not generate business/semantic logic that becomes harder to review than handwritten Rust.

---

# 68. Checked vs timing path

Keep separate builds.

Checked/debug validates:

```text
all enums
all reference kinds
all ref buffer lengths
NodeId range
thread ownership
runtime generation/alive state
cycle/work budget where applicable
semantic bounds
UTF-8 / byte contracts
```

Timing still validates every memory-safety requirement.

Timing may omit expensive redundant diagnostics guaranteed by private generated producers.

Do not remove a bounds check whose only justification is a microbenchmark expectation.

---

# 69. Owner thread

PERF-12 remains synchronous on the environment owner thread.

No mutex is added around hot runtime state merely to make misuse possible.

Debug/safety builds assert owner-thread access.

Rust `View` internal thread-safety/Arc decisions are outside PERF-12 unless separately profiled.

---

# 70. No asynchronous command ring

Do not add:

```text
SharedArrayBuffer ring
Atomics producer/consumer protocol
native presenter worker
backpressure queue
fences
async root commit
```

Bun FFI call overhead must first be demonstrated as the remaining blocker after the direct retained design.

This would be a later architecture experiment.

---

# 71. No private JavaScriptCore object layout

Do not reinterpret:

```text
napi_value
JSCell pointer
Structure offsets
internal object storage
```

through undocumented JSC/Bun internals.

The pinned runtime does not make private GC/object layout a suitable Iyon ABI.

Supported hot path is typed engine-native FFI over values/handles whose layout Iyon controls.

---

# 72. Direct N-API remains an oracle and exceptional recovery path

Keep the complete Direct decoder while PERF-12 is experimental.

Uses:

```text
semantic differential oracle
rare stale-ref recovery
runtime-generation recovery if useful
cold fallback if it remains competitive
fuzz comparison
```

Do not optimize Direct during PERF-12 in a way that invalidates comparison unless the same change is clearly transport-neutral.

---

# 73. Recovery helper

If current APIs make targeted stale-ref recovery awkward, add one explicit recovery entrypoint rather than abusing host render.

Conceptual Node-API function:

```text
tuiViewDecodeRef(BridgeViewNode) -> leased NativeRef
```

It uses the existing Direct `ViewDecoder` and current `NativeViewRuntime`.

It must:

```text
not mutate host
publish through the shared semantic cache
return a root lease
not retain napi_value
```

Use it only on exceptional recovery/cold paths, never common retained hits.

For wide sequence sidecars, first materialize the exact Direct-compatible node only if this fallback is actually taken.

---

# 74. Failure status detail

Generated FFI should expose enough detail to recover a stale child without probing every ref.

Possible status detail:

```text
status.code        = FAST_CACHE_MISS
status.detail_kind = CHILD_REF
status.detail0     = offending NativeRef or child ordinal
```

For a base-ref edit:

```text
status.detail_kind = BASE_REF
```

The exact status-cell representation should extend the existing ABI convention.

Do not allocate JS `Error` objects for expected fallback statuses on the hot path.

---

# 75. Cycle handling

The public semantic API should form a DAG, but private/corrupt inputs must not recurse forever.

JS materializer has:

```text
inProgress identity set
maximum retained depth
maximum new-node work budget
```

Direct N-API recovery retains its current cycle guard.

No recursive unbounded traversal.

---

# 76. Full-schema coverage

PERF-12 cannot be selected on a benchmark subset.

At minimum cover all current View node kinds and variants including:

```text
Text
styled Text
Diff
Spacer
Row
Column
Hanging
Grid
Container
Clamp
ContentMax
Component
Decoration
```

And all relevant:

```text
layout child variants
tracks
alignment
padding
width/height rules
min/max constraints
foreground/background
text attributes
border/custom glyphs
style states
overflow/wrap
Unicode
component references
```

Every schema item is either:

```text
direct-materialized
handled by a specialized retained operation
or explicitly routed to a complete fallback
```

No silent omissions.

---

# 77. Every View-bearing boundary

Trace actual production source before implementation.

PERF-12 must integrate at every place that currently accepts or stores a View, including at least:

```text
Tui.render
History
ViewSlot
ScrollPane
animations
components
any host/control installRef path
```

Each boundary must define:

```text
current root NativeRef lease owner
replace protocol
close/dispose protocol
fallback behavior
runtime generation handling
```

No boundary may silently call Direct and erase the architecture win in realistic traces.

---

# 78. History

History is already a retained subsystem and must stay semantic-content-aware.

PERF-12 should not convert History into a repeated generic View reconstruction workload.

Rules:

```text
stable History content remains native-retained
new finalized entry imports only its new View frontier
streaming entry uses stream path
component revision invalidation remains component-scoped
width-related remeasure uses History/layout semantics, not bridge retransmission
```

Benchmark with substantial existing History, not an empty shell.

---

# 79. Components

Component nodes require stable component identity and revision semantics.

A stable component shell must cut off by NativeRef like any other stable semantic node.

Component-internal high-frequency changes should use their existing component/native revision channel rather than reconstructing the outer View DAG if that is already the architecture.

Differential tests must include components in shared subtrees.

---

# 80. ViewSlot and ScrollPane

These boundaries commonly keep a current View and replace it.

They are ideal root-lease owners.

Replace sequence:

```text
old root lease stays alive
materialize new root
install/render new root
swap stored root NativeRef
release old root lease
```

Do not release old root before new retained materialization has resolved all stable descendants.

That ordering is part of the stable-ref correctness argument.

---

# 81. Animations

Animation must not create a hidden full-tree bridge per frame.

If animation state can be represented by existing native scalar/component animation machinery, keep it there.

If a frame legitimately creates a new semantic View, it follows the same retained identity rules.

Benchmark at least one animation case to prove no fallback loop.

---

# 82. PERF-12.0 source freeze

Before coding:

```bash
git status --short
git rev-parse HEAD
git log -1 --oneline
bun --version
bun --revision
rustc --version
```

Record:

```text
final PERF-11v4 SHA
historical 7v2 SHA
PERF-12 starting SHA
Bun version/revision
Rust version/target
macOS version
CPU
```

At handoff preparation time the branch was `67741eb...`, but implementation must use the actual post-11v4 HEAD.

---

# 83. PERF-12.0 direct-call floor probe

This is not a second architecture implementation.

It is a tiny stop-before-waste test using the already-generated `runtimeNoop` and representative existing 11v3 constructor calls.

Measure:

```text
1
2
4
8
16
32
64
```

engine-native calls in one JS operation after JIT warmup.

Also measure the real call shapes for:

```text
one scalar constructor
one fixed-arity Row/Column constructor
one small ref-buffer constructor
one retained patch
```

Use the same pinned Bun revision and timing discipline as 11v4.

Decision:

```text
if direct FFI call floor alone consumes the expected retained-operation budget
for the observed changed-frontier distribution:
    STOP before full PERF-12 implementation
    reopen transport lowering design
```

Do **not** automatically implement Shared Mirror or Delta after a failed probe.

The purpose is to avoid spending manpower on two full candidates.

---

# 84. PERF-12.1 implementation: faithful semantic DAG

Start from the actual historical file:

```bash
git show \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f:packages/iyon-runtime/src/tui/values/view.ts
```

Mechanically adapt:

```text
current schema
current enums
current public View API
current correctness fixes
current NodeId safe range
current component/style semantics
```

Preserve:

```text
eager BridgeViewNode construction
frozen semantic object
WeakMap View -> node
stable child object identity
lookup-only nodeForBridge
```

Do not import pending create/patch semantics into the new candidate.

---

# 85. Construction gate

Before FFI work grows, compare:

```text
direct_7v2 benchmark builder
retained_dag_ffi semantic builder only
native_11v3 current construction
```

Representative cases:

```text
plain text
styled text
3-modifier chain
20-node column
200-node column
row tracks
Grid
Diff
```

Gate:

```text
retained_dag_ffi semantic construction <= 5% slower than faithful direct_7v2
```

Preferred:

```text
within noise
```

If sidecar/derivation writes erase the 7v2 construction advantage, simplify before continuing.

---

# 86. Semantic parity tests before native transport

For every current public semantic operation, construct:

```text
faithful 7v2-style candidate View
current production View
```

and compare a transport-independent semantic snapshot.

Cover:

```text
node kind
NodeId rules
all fields
child order/identity
styles
tracks
Grid placement
Diff
Unicode
components
modifiers
wide sidecar materialization
```

No performance timing until semantic parity passes.

---

# 87. Randomized DAG differential testing

Generate deterministic random DAGs with:

```text
shared subtrees
multiple parents
wide and narrow axes
styles
Unicode strings
Grid
Diff
decorations
modifier chains
retained one-leaf changes
multiple changed branches
```

For each seed:

```text
render direct oracle
render retained_dag_ffi
compare screen/state output
```

On failure print:

```text
seed
operation sequence
semantic DAG snapshot
candidate outputs
structural counters
```

---

# 88. Native runtime cleanup must land before judging memory

Tranche 12.2 should be independently reviewable.

Required deliverables:

```text
central semantic publish/lookup helper
NativeRef table benchmark and chosen representation
weak-cache maintenance
slot/page removal
root/temp lease counters
runtime memory diagnostic snapshot
explicit maintenance hook for tests/benchmarks
```

Run the existing 11v3/Direct tests against the changed shared runtime before PERF-12 depends on it.

This change should improve all transports, not only PERF-12.

---

# 89. Memory diagnostic ABI

Add counter-build or test-only snapshot support.

Conceptual output:

```json
{
  "semantic_cache_entries": 0,
  "semantic_cache_live": 0,
  "native_ref_slots": 0,
  "native_ref_pages": 0,
  "leased_slots": 0,
  "node_ref_entries": 0,
  "path_nodes": 0,
  "path_keys": 0,
  "builders": 0,
  "edit_txns": 0,
  "style_refs": 0,
  "string_bytes": 0,
  "scavenge_queue": 0
}
```

Do not call an expensive full scan on every timing sample.

---

# 90. Transport preparation must be visible in benchmarks

Do not report:

```text
encoding = 0
```

merely because work happens in direct function argument preparation.

Phases for PERF-12:

```text
semantic_construction_ns
identity_cutoff_and_argument_prep_ns
ffi_materialization_ns
host_commit_ns
total_ns
```

For buffer calls also count:

```text
ref_words_written
aux_words_written
byte_payload_written
```

Architectural selection uses `total_ns`.

---

# 91. Structural counters

Counter build should expose at least:

```text
bridge_hint_hits
bridge_hint_misses
node_id_ref_promotion_attempts
node_id_ref_promotion_hits
node_id_ref_promotion_misses
bridge_semantic_nodes_inspected
bridge_children_visited
direct_materializer_calls
derivation_fast_path_calls
ref_words_written
byte_payload_bytes
native_ref_resolves
native_ref_cache_misses
semantic_cache_hits
semantic_cache_misses
persistent_seq_branches_cloned
persistent_seq_items_iterated
stale_ref_retries
cold_fallbacks
host_mutations
```

These prove asymptotic behavior independently of timing noise.

---

# 92. Required steady-state traces

## Exact root

```text
nodeForBridge -> lookup
BRIDGE_NATIVE -> hit
hostRenderRef(root)
return
```

## One changed leaf

```text
JS constructs new leaf + changed ancestors
stable siblings remain same BridgeViewNode identities
ensureNative(root)
    materialize new changed path postorder
    stable child -> NativeRef hint, stop
hostRenderRef(new root)
release old root + child temp leases
```

## Deep changed path

```text
one direct semantic constructor/edit per new ancestor
no work in unrelated subtrees
```

## Wide one-child edit

```text
PersistentSeq sidecar
base NativeRef + index + new child ref
O(log_32 N)
```

## Streaming text

```text
stream bytes use existing stream path
structural shell stays retained
```

---

# 93. Common-node benchmark matrix

At minimum:

```text
plain_text_column
styled_span_heavy
row_heavy
column_track_heavy
grid_heavy
decoration_heavy
diff_heavy
component_heavy
mixed_realistic
```

Sizes:

```text
~20 nodes
~200 nodes
~2,000 nodes
10,000 where sensible for cutoff/cold
```

Primary retained modes:

```text
IDENTICAL_IDENTITY
SHARED_PATH
SHARED_DEEP depths 4/16/64/128
LARGE_SHARED_SUBTREE_CUTOFF
TEXT_METADATA_PATCH
DECORATION_PATCH
REBUILT_EQUIVALENT
```

---

# 94. Large shared-subtree cutoff

Construct:

```text
new root
+-- changed small branch
`-- stable subtree of 20 / 200 / 2,000 / 10,000+ nodes
```

Required structural result:

```text
stable subtree descendants inspected = 0
```

Required timing behavior:

```text
retained cost should be effectively independent of stable subtree descendant count
```

within expected cache/JIT noise.

## Cold-fallback descendant sidecar gap

Repeat the same shape after deliberately materializing the previous root through the selected cold/direct fallback without populating descendant `BRIDGE_NATIVE` hints.

On the next one-leaf retained update, the first stable subtree boundary must do:

```text
BridgeNativeHint miss
NodeId eligible under nativeLookupCeiling
viewRefForNodeId(NodeId) -> hit
install BridgeNativeHint
stop before payload/children
```

Required:

```text
stable subtree descendants inspected = 0
one scalar NodeId-ref promotion at the stable boundary
no Direct object decode
no full subtree materialization
```

This proves the retained asymptotic does not depend on every cold path eagerly seeding JS sidecars.

---

# 95. Multi-edit benchmark

Cases:

```text
2 changed leaves same parent
8 changed leaves same branch
8 changed leaves distinct branches
32 changed leaves distinct branches
64 changed leaves
```

Report:

```text
new BridgeViewNodes
materializer calls
common ancestors materialized
NativeRef hits
ref words
native time
total time
```

Compare final candidate only against:

```text
direct_7v2
native_11v3
```

Do not require a full Shared Mirror or Delta implementation for this comparison.

---

# 96. Wide benchmark

Operations:

```text
replace one child
insert one child
remove one child
splice four children
Grid cell replacement
```

Widths:

```text
32
256
2,048
10,000
100,000
```

Required counters:

```text
PersistentSeq branches/leaves cloned
items iterated
ref words transported
semantic nodes constructed
```

Reject any one-child retained path that becomes O(width).

---

# 97. Cold benchmark

Sizes:

```text
200
2,000
10,000
```

Compare the real complete candidates available after 11v4:

```text
direct
11v3 cold/native builder
V4 if still relevant
retained_dag_ffi only below retained budget
```

The production router should not spend a large direct-call prefix before every obviously cold render.

Initial render should normally choose the known best cold path directly.

---

# 98. String benchmark

Datasets:

```text
short ASCII
short Unicode
emoji/non-BMP
embedded NUL
256-byte strings
4-KiB strings
many styled spans
Diff lines
```

Compare existing current string paths and only keep a new scratch/import path if end-to-end total wins.

Do not optimize for FFI call count at the cost of JS encoding.

---

# 99. Realistic agent-TUI trace

Use the same broad intent as PERF-11v4.

Trace contains:

```text
stable application shell
substantial existing History
assistant stream append through native stream path
periodic tool/status component updates
message finalization
new History insertion
occasional layout/decor changes
scroll/update
occasional larger structural change
```

The View bridge is measured around the real specialized stream path.

Primary question:

> Does PERF-12 reduce total application work, not merely synthetic bridge work?

---

# 100. Benchmark process isolation

Run candidates in fresh Bun processes to avoid shared cache/JIT/heap contamination.

```text
parent orchestrator
+-- child direct_7v2 / case X
+-- child native_11v3 / case X
`-- child retained_dag_ffi / case X
```

Process startup is outside measurements.

Alternate candidate order between blocks.

Use the same native artifact wherever possible.

---

# 101. Timing versus counter builds

Authoritative timing build:

```text
minimal hot-path instrumentation
same optimization/LTO settings across candidates
```

Counter/memory build:

```text
structural counters
cache diagnostics
extra invariant checks
```

Do not compare timing from a build where one candidate performs materially more atomics/counters.

---

# 102. Statistical requirements

Normal cases:

```text
warmup >= 50
measured >= 500
```

Tiny exact/FFI cases:

```text
warmup >= 10,000
measured >= 10,000
```

Reported p99:

```text
measured >= 1,000
```

Retain raw samples as JSONL.

Use:

```text
median
p95
p99 where supported
bootstrap confidence intervals
median ratios
geometric mean ratios across heterogeneous workload groups
```

Do not average unrelated absolute nanoseconds.

## 102.1 Benchmark runtime profiles: smoke vs. authoritative

The full `§102` discipline is expensive (the 11v4-scale matrix takes hours of
serial, process-isolated sampling). It is required **only once**, for the T15
authoritative comparison after the implementation is complete. Tranche gates
must not pay that cost.

Two profiles are defined:

**Smoke profile — default for every tranche gate (T1–T14):**

```text
case set:      ~20 representative cases, fixed by name in the tranche gate:
                 exact identity (small), SHARED_PATH depths 4/16/64,
                 SHARED_DEEP depth 16, one LARGE_SHARED_SUBTREE_CUTOFF,
                 TEXT_METADATA_PATCH, DECORATION_PATCH,
                 WIDE_PARENT_ONE_EDIT at width 2,048,
                 one text case (short Unicode), one diff case,
                 mixed_realistic at ~200 nodes
sampling:      adaptive; warm up >= 50, measure until the bootstrap CI
               half-width of the median drops below 5% or 500 samples,
               whichever comes first (tiny cases may batch 1,000 ops per
               timed block and record block medians)
processes:     same isolation rules as §100 (fresh child per case)
builds:        timing build only; counter builds only when a structural
               asymptote itself is the gate (then use §91 counters, not
               extra timing samples)
output:        JSONL with the §103 schema, marked "profile": "smoke"
```

Smoke results decide gate pass/fail and are retained as evidence. They are
**not** comparable across tranches as performance trends unless the candidate
and fixtures did not change.

**Authoritative profile — T15 only:**

```text
full §93 matrix (all workloads × sizes × modes), full §102 sample counts,
full §102 statistics, both candidates + all completed alternatives,
fresh native artifact, clean tree, frozen provenance per §82/§60.
```

Rules:

- Never run the authoritative profile mid-implementation "to see how it looks";
  it is the decision run and must reflect the final state.
- If a smoke gate fails in a way that suggests a real regression, fix first,
  then re-smoke. Escalate to a broader case set only when a smoke pass is
  ambiguous, not routinely.

## 102.2 Incremental benchmark cache keyed by content hashes

During T5–T14 each tranche normally changes only one candidate's code while
`direct_7v2`, `native_11v3`, fixtures, and harness definitions remain frozen.
Re-running unchanged arms of a comparison wastes hours.

Maintain a benchmark result cache keyed by:

```text
candidate implementation SHA (or content hash of its source files)
case definition SHA (workload + size + mode + fixture seed)
bun version AND bun revision
rustc version + target + native artifact SHA-256 where applicable
machine identifier
profile (smoke | authoritative)
```

Rules:

- A cached result may be reused only on an **exact key match**. Any drift in
  any key component invalidates that arm.
- Cached entries must be copied into every new raw JSONL output so each run's
  artifact is self-contained and carries its full provenance (`§103`). Mark
  reused records with `"cached": true` plus the original capture SHA/date.
- The T15 authoritative run must be computed fresh: no cached timing samples
  from smoke runs may enter the authoritative aggregate. The cache exists for
  development velocity, never for the decision evidence.
- The cache lives outside git (e.g. `target/perf12-bench-cache/`); only the
  JSONL artifacts it helped produce are committed.

---

# 103. Required result schema

Each result record should include:

```json
{
  "benchmark_version": "PERF-12",
  "candidate": "retained_dag_ffi",
  "workload": "...",
  "size": "...",
  "mode": "...",
  "git_sha": "...",
  "perf7v2_sha": "e5292d62...",
  "perf11v4_result_sha": "...",
  "bun_version": "1.4.0",
  "bun_revision": "...",
  "rustc_version": "...",
  "target": "...",
  "semantic_construction_samples_ns": [],
  "transport_prepare_samples_ns": [],
  "native_materialize_samples_ns": [],
  "host_commit_samples_ns": [],
  "samples_ns": [],
  "median_ns": 0,
  "p95_ns": 0,
  "p99_ns": 0,
  "median_ci95_ns": [0, 0]
}
```

Memory/churn results should be separate records rather than inflating tiny timing samples with expensive memory queries.

---

# 104. Adoption gates

PERF-12 replaces the current private View path only if all correctness, structural, performance, and memory gates pass.

## Correctness

```text
full schema parity
randomized DAG differential tests
NodeId identity preserved
cross-transport cache correctness
stale-ref recovery correct
runtime generation recovery correct
no partial host mutation
all temporary leases released
all View-bearing boundaries complete
Unicode/NUL/Diff parity
no UAF / retained borrowed pointer
```

## Structural

```text
exact root = zero semantic payload reads / zero buffer writes
stable subtree = cutoff before child payload inspection
retained work = changed semantic frontier
no full-tree diff
no generic structural packet
wide edit = O(log_32 N)
streaming text separate
persistent transport graph = none
```

## Construction

```text
<=5% regression vs faithful Bun 1.4 direct_7v2 construction
preferred: within noise
```

## Performance

```text
realistic retained TUI trace:
    >=10% faster than the better of direct_7v2 and native_11v3
    >=15% preferred

common retained cases:
    no credible >3% regression vs best prior path

exact identity:
    within 3% of best prior exact NativeRef/direct path

wide:
    preserve logarithmic work and beat 7v2 at useful wide sizes

cold:
    production router within 5% of best complete cold candidate
```

## Memory

```text
no persistent transport-node/edge arena
post-maintenance weak metadata converges with live state + bounded slack
leased NativeRefs correspond only to roots + in-flight temp leases
scratch memory bounded
no linear post-GC metadata slope during 1M-node churn
```

---

# 105. If PERF-11v4 says 11v3 is decisively better

PERF-11v4 already defines a category where 11v3 is a decisive realistic-trace winner.

If that result is confirmed and the 2.7 GiB memory behavior is fully explained/fixed without architectural pain, it is valid to stop at PERF-12.0 and not implement the rest.

However, if the result is a modest win, practical tie, Candidate-A win, or if 11v3's representation/memory complexity remains materially undesirable, this handoff defines the architecture to implement.

Do not lower PERF-12's adoption gate simply because implementation has begun.

---

# 106. Stop conditions during implementation

Stop and revisit the design if:

```text
direct FFI call floor consumes the retained-operation budget before semantic work
restored eager DAG construction regresses >5% without compensating total win
NativeRef hint lookup becomes a measurable dominant cost
weak-cache cleanup cannot make historical metadata bounded
one stable child routinely produces stale-ref retries in normal retained updates
full-schema direct functions devolve into a generic opcode VM
variable payload preparation reproduces PERF-10-scale JS encoding cost
wide sidecar cannot preserve correct public semantics without O(width) work
streaming bytes are accidentally routed through structural View construction
cold/rebuilt workloads frequently hit retained budget after large wasted work
root/temp lease accounting cannot be proven on every boundary
```

Do not respond to these failures by quietly layering Shared Mirror on top.

---

# 107. Banned shortcuts

Do not:

```text
put NativeRef own-properties on BridgeViewNode
keep current pending backing and call it 7v2-style
create a second semantic cache
replace NodeId semantics with NativeRef
recycle NativeRefs without ABA protection
use a persistent SharedRef graph
retain pointers into JS TypedArrays after FFI returns
retain napi_value after recovery call
use undocumented JSC object layout
flatten PersistentSeq for retained wide edits
encode stable text again
create a fresh TypedArray per node
use reflection in generated materializers
hide argument/scratch preparation outside total timing
benchmark only Tui.render while other boundaries still take Direct/fallback
make RSS alone the memory correctness metric
make FinalizationRegistry timing a correctness requirement
```

---

# 108. Suggested commit sequence

## Commit 1

```text
bench(tui): freeze PERF-12 baselines and attribute native view memory
```

Contains:

```text
source archaeology record
PERF-11v4 result import
2.7 GiB attribution tooling
Bun FFI call-floor probe
no production architecture yet
```

## Commit 2

```text
refactor(tui): restore eager immutable semantic View DAG candidate
```

Contains:

```text
historical construction adaptation
current schema parity
construction benchmarks
no native transport change yet
```

## Commit 3

```text
perf(tui): bound native semantic cache and ref metadata lifetime
```

Contains:

```text
shared runtime publication helper
weak scavenging
paged NativeRef table if benchmark gate passes
memory diagnostics
existing transport regression suite
```

## Commit 4

```text
perf(tui): materialize retained semantic nodes through direct Bun FFI
```

Contains:

```text
BridgeNativeHint
exact root
common fixed-size generated materializers
temporary lease transaction
```

## Commit 5

```text
perf(tui): lower variable retained children through borrowed FFI buffers
```

Contains:

```text
ref scratch
variable-axis/Grid materializers
caps/fallback
no persistent arena
```

## Commit 6

```text
perf(tui): preserve semantic derivations and retained scalar clones
```

Contains:

```text
text layout derivation
common scalar derivation
base-ref clone operations
stale base recovery
```

## Commit 7

```text
perf(tui): preserve logarithmic wide edits in retained DAG FFI
```

Contains:

```text
PersistentSeq sidecar
axis set/splice
Grid cell edit
100k structural-counter proof
```

## Commit 8

```text
perf(tui): complete retained text style diff and decoration materialization
```

Contains:

```text
cstring/exact-byte paths
styles
Diff
Unicode/NUL tests
stream separation
```

## Commit 9

```text
perf(tui): harden multi-branch materialization and stale-ref recovery
```

Contains:

```text
DAG dedupe
cycle/work limits
status detail
targeted one-retry recovery
failure/lease tests
```

## Commit 10

```text
perf(tui): route all View boundaries through retained semantic identity
```

Contains:

```text
History
ViewSlot
ScrollPane
animation
components
cold/rebuilt router
```

## Commit 11

```text
test(tui): complete PERF-12 differential lifetime and memory hardening
```

Contains:

```text
random DAG tests
fuzz
1M-node churn
forced-GC checkpoints
runtime teardown
cross-transport cache tests
```

## Commit 12

```text
bench(tui): complete PERF-12 retained DAG FFI decision
```

Contains:

```text
raw JSONL
performance review
memory review
adoption decision
```

## Commit 13 - only after adoption

```text
refactor(tui): remove superseded View transport recipe machinery
```

Contains only cleanup proven safe by the selected architecture.

---

# 109. Tranche 12.0 deliverables

Create:

```text
PERF-12-baseline.md
PERF-12-memory-attribution.jsonl
PERF-12-ffi-floor.jsonl
```

`PERF-12-baseline.md` records:

```text
current SHA
Bun version/revision
PERF-11v4 result category
actual current ViewBacking shape
historical 7v2 constructor/nodeForBridge shape
current NativeViewRuntime maps and lease model
current direct decoder NodeId-first behavior
current generator/runtime bootstrap
memory hypothesis results
```

Do not proceed if this archaeology contradicts a fundamental assumption in this handoff.

---

# 110. Tranche 12.2 weak-cache gate

After cache cleanup and before PERF-12 direct materialization, rerun the memory reproducer against current 11v3/Direct paths.

The result should answer:

```text
How much of the 2.7 GiB was:
    semantic/native live state?
    stale weak-cache metadata?
    benchmark harness?
    JSC/allocator high-water?
```

If a central cache-lifetime fix materially reduces memory, record it separately from PERF-12 transport wins.

Do not attribute a shared-runtime fix solely to the new candidate.

---

# 111. Native slot lease invariants

Add tests proving:

```text
new constructor returns lease count 1
child temporary lease stays live until parent/root complete
batch release drops child temp lease
root lease transfers to boundary
old root lease released only after new host install succeeds
failed host install retains old root
failed transaction releases every new temp lease
stale unleased weak slot returns CACHE_MISS
slot metadata eventually scavenged after weak expiry
```

These tests are more important than testing a FinalizationRegistry because baseline PERF-12 does not use finalizer-owned View leases.

---

# 112. Native semantic cache identity tests

Explicitly test cross-path identity:

```text
Direct materializes NodeId X
PERF-12 constructor asks for X
    -> returns/associates exact live View

PERF-12 materializes X
Direct sees X
    -> exact live WeakView hit

11v3 materializes X
PERF-12 sees X
    -> same semantic View

WeakView X expires
lookup X
    -> stale entry removed
    -> correct reconstruction
```

No transport may fork semantic identity.

---

# 113. Root exact-identity scaling test

Create identical root trees at:

```text
20
200
2,000
10,000
```

Warm root NativeRef and render the exact same root repeatedly.

Required:

```text
semantic fields read = 0
children visited = 0
materializer calls = 0
ref words written = 0
```

Timing must not scale with descendant count.

---

# 114. Dormant-node recovery test

Construct semantic node `S`.

```text
render S / acquire native state
replace all roots so S native View can expire
keep JS BridgeViewNode S strongly reachable
force native maintenance
reinsert S into a new parent
```

Expected:

```text
stale NativeRef hint may miss
one targeted rematerialization/recovery
new hint installed
correct render
no persistent mirror required
```

Run the same after runtime generation reset where test infrastructure permits.

---

# 115. Multi-host test

The same BridgeViewNode may be rendered into more than one host/boundary.

NativeRef hints are environment-local, not host-local.

Test:

```text
host A installs root R
host B installs same R
host A replaces/closes
host B still renders exact R
both close
R becomes dormant
later host C renders R and recovers if necessary
```

Root lease counts must be correct.

---

# 116. Buffer lifetime tests

For every generated buffer argument:

```text
native reads only during call
native does not store pointer
zero length correct
max allowed length correct
oversize -> FAST_FALLBACK/INVALID as specified
unaligned/wrong typed input rejected by generated JS wrapper or native validation
```

Run sanitizer builds where practical.

There should be no external native memory mapping to test in baseline PERF-12.

---

# 117. Fuzzing targets

Add fuzz/property targets for:

```text
NodeId split/recombine
invalid NativeRefs
stale refs
variable ref counts
axis/grid bounds
all enum discriminants
text byte lengths
embedded NUL
malformed Diff
cycle/depth limits
runtime generation mismatch
failure during ancestor materialization
release list duplication / invalid refs
```

The private generated producer reduces the attack surface, but native memory safety still assumes inputs can be malformed.

---

# 118. Failure injection

Test failure at every stage:

```text
child materializer fails
parent materializer fails
base-ref edit misses
buffer cap fallback
style creation fails
text import fails
hostRenderRef fails
runtime marked dead
second stale-ref retry fails
```

Assert:

```text
old host state unchanged
temporary leases drained
no borrowed pointer retained
semantic cache contains only complete immutable Views
subsequent valid render succeeds
```

---

# 119. Performance interpretation

A direct-node architecture can show more FFI calls while doing less total work.

Do not reject it because:

```text
FFI calls / update > 1
```

Likewise, do not accept it because:

```text
native constructor ns is tiny
```

The correct measure is:

```text
semantic construction
+
identity checks
+
argument/buffer preparation
+
all FFI calls
+
native semantic work
+
host mutation
```

for the real operation.

---

# 120. Complexity interpretation

PERF-12 has a second adoption criterion beyond raw speed: it should reduce semantic/transport coupling relative to 11v3.

A successful final code shape should read approximately like:

```text
View API creates semantic DAG
optional sidecar says how new node came from old node
ensureNative walks new frontier
native semantic constructor/edit creates retained View
host receives root ref
```

If the implementation instead grows back into:

```text
pending state machine
path recipe language
multi-step builder transaction language
transport-specific semantic backings
```

then PERF-12 has failed its architectural simplification goal even if one microbenchmark wins.

---

# 121. Expected code ownership after adoption

## `values/view.ts`

Owns:

```text
public View construction
semantic immutable BridgeViewNode DAG
NodeId assignment
semantic derivation hints
wide sequence sidecars
```

Does not own:

```text
wire records
native page slots
transport packet layout
```

## `native_view_abi.ts` or successor

Owns:

```text
session/bootstrap
ensureNative
NativeRef hint sidecar
materialization transaction
root lease swap
borrowed scratch buffers
fallback routing
style/text helper coordination
```

## generated ABI

Owns:

```text
monomorphic function signatures
TypeScript call wrappers
C/Rust declarations
manifest/conformance
```

## `NativeViewRuntime`

Owns:

```text
semantic weak cache
NativeRef table
leases
weak metadata scavenging
styles/strings
publication
status
```

## `iyon-tui`

Remains the semantic/native TUI implementation.

---

# 122. Explicitly rejected architecture: persistent mirror as semantic recovery cache

Do not retain Shared Mirror secretly as a recovery-only cache.

That recreates the same cost:

```text
persistent record allocation
release/reclamation
page high-water
SharedRef lifetime
edge blocks
```

for an exceptional path.

If recovery becomes frequent enough to matter, first determine why native roots/weak state are expiring during supposedly retained use.

Only reconsider a persistent mirror if measured recovery frequency and cost make it a first-order application problem.

---

# 123. Explicitly rejected architecture: generic changed-closure VM as baseline

Do not implement a 64/128-byte generic node record language solely because it gives one commit call.

The direct-call design should be given the first implementation opportunity under Bun 1.4.

A later packet/batch experiment is justified only if profiles show:

```text
changed frontier is commonly large
per-node semantic work is already tiny
FFI call dispatch itself is a meaningful share of total
```

If that happens, the semantic architecture in this handoff still survives: only the physical lowering of `generatedMaterializeNode` changes.

---

# 124. Important fallback insight

The semantic architecture and physical FFI lowering are deliberately separable.

The stable long-term API inside the runtime should be conceptually:

```text
materialize semantic node from:
    NodeId
    NativeRef children/base
    changed scalar/payload values
```

Today PERF-12 lowers that directly to generated FFI calls.

If a future Bun/runtime profile says batches are necessary, codegen could lower multiple calls into a bounded scratch transaction without changing:

```text
BridgeViewNode semantics
NativeRef sidecars
NodeId cache
wide sidecars
root lease protocol
public API
```

This is a better future-proof boundary than making packet format the architecture itself.

---

# 125. Source references used for this handoff

## Iyon

Historical 7v2 semantic construction:

- <https://github.com/alexykn/iyon/blob/e5292d62c4011610850cbdc1ba4a35f296f78e4f/packages/iyon-runtime/src/tui/values/view.ts>

Re-audited current branch revision:

- <https://github.com/alexykn/iyon/commit/67741eb588e70ffe8ce7b08805040d0a9cc65f8c>

Current semantic/View backing:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/packages/iyon-runtime/src/tui/values/view.ts>

Current generated/native JS path:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/packages/iyon-runtime/src/tui/native_view_abi.ts>

Current native View ABI/runtime:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/crates/iyon-native/src/tui/view_abi.rs>

Current Direct N-API decoder:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/crates/iyon-native/src/tui.rs>

Current persistent sequence:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/packages/iyon-runtime/src/tui/persistent_seq.ts>

Current generated ABI schema / Bun qualification:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/tools/tui-abi/view_abi.toml>

Current runtime pin:

- <https://github.com/alexykn/iyon/blob/67741eb588e70ffe8ce7b08805040d0a9cc65f8c/package.json>

## External research

React Native Fabric:

- <https://reactnative.dev/architecture/render-pipeline>
- <https://reactnative.dev/architecture/landing-page>
- <https://reactnative.dev/docs/the-new-architecture/using-codegen>

Flutter internals:

- <https://docs.flutter.dev/resources/inside-flutter>

Qt Quick retained scene graph:

- <https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html>

Bun FFI:

- <https://bun.com/docs/runtime/ffi>

Bun GC / benchmarking memory guidance:

- <https://bun.com/docs/project/benchmarking>
- <https://bun.com/reference/bun/gc>

SBE design principles:

- <https://github.com/aeron-io/simple-binary-encoding/wiki/Design-Principles>

Chromium shared-memory command-buffer caution:

- <https://chromium.googlesource.com/chromium/src.git/+/HEAD/docs/security/research/graphics/gpu_command_buffer.md>

---

# 126. Final implementation checklist

## Baseline

```text
[ ] final PERF-11v4 result imported
[ ] post-11v4 SHA frozen
[ ] Bun 1.4 exact revision frozen
[ ] historical 7v2 source re-read
[ ] current Direct decoder re-traced
[ ] current ABI/runtime re-traced
```

## Memory

```text
[ ] 2.7 GiB case reproduced
[ ] JS heap vs native vs harness classified
[ ] weak semantic-cache growth measured
[ ] NativeRef slot growth measured
[ ] central scavenging implemented if required
[ ] 1M-node churn converges after GC/maintenance
```

## Semantic JS

```text
[ ] eager BridgeViewNode normal path
[ ] nodeForBridge lookup-only normal path
[ ] stable object identity
[ ] no transport own-properties
[ ] construction <=5% regression vs 7v2
[ ] wide-only sidecar exception correct
```

## Native identity

```text
[ ] NodeId remains semantic authority
[ ] one NodeId -> WeakView cache
[ ] NativeRef is acceleration only
[ ] no NativeRef ABA
[ ] root leases explicit
[ ] child temp leases batch-released
[ ] stale refs recover once
```

## FFI

```text
[ ] generated common-node direct constructors
[ ] fixed arity scalar fast paths
[ ] variable refs use borrowed buffers
[ ] no retained buffer pointers
[ ] no persistent transport arena
[ ] generator conformance passes
```

## Modern retained optimizations

```text
[ ] text layout reuses stable payload/base
[ ] style refs reused
[ ] Diff specialized
[ ] streaming separate
[ ] native streaming pipeline (TextStream append/seal, incremental compilation,
    source-rooted coordinates, History stream units) preserved unchanged
[ ] PersistentSeq wide edits logarithmic
[ ] Grid retained edit logarithmic
[ ] exact root O(1)
[ ] stable subtree cutoff before payload
```

## Boundaries

```text
[ ] Tui root
[ ] History
[ ] ViewSlot
[ ] ScrollPane
[ ] animation
[ ] components
[ ] every other View-bearing native boundary
```

## Correctness/safety

```text
[ ] full schema parity
[ ] randomized DAG differential suite
[ ] cross-transport cache suite
[ ] dormant-node recovery
[ ] multi-host lifetime
[ ] failure injection
[ ] fuzzing
[ ] runtime teardown
[ ] no UAF
[ ] no partial host mutation
```

## Decision

```text
[ ] raw performance samples retained
[ ] phase breakdown retained
[ ] structural counters retained
[ ] memory review published
[ ] realistic TUI trace >=10% faster than better prior candidate for adoption
[ ] no common retained >3% credible regression
[ ] cold router within 5% of best complete cold path
[ ] cleanup only after soak
```

---

# 127. Final instruction to the implementation agent

**Implement PERF-12 as a retained semantic identity bridge, not as a new serialization format. Restore the real PERF-7v2 eager immutable `BridgeViewNode` DAG against the current schema. Keep `NodeId -> WeakView` in the single environment-owned `NativeViewRuntime` as semantic authority. Associate already-materialized Bridge nodes with generation-scoped NativeRef hints in WeakMap sidecars, and always test that identity before reading payload or children. Materialize only unknown semantic nodes, children first, through generated engine-native Bun FFI constructors and retained clone/edit primitives; use small reusable borrowed TypedArrays only for variable reference/byte payloads. Keep exactly one strong root NativeRef lease per View-bearing boundary plus temporary leases during a materialization transaction; do not make every sidecar a native lease and do not depend on FinalizationRegistry for normal correctness. Preserve PersistentSeq wide edits, native strings/styles, stream specialization, exact-root shortcuts, the complete fallback, and every View-bearing boundary. Before attributing the reported approximately 2.7 GiB RSS to tree size, instrument and fix any historical weak-cache/NativeRef metadata accumulation in the shared runtime. Do not implement the persistent Shared Mirror DAG as a second candidate, and do not build a generic changed-closure record VM unless a later profile proves direct Bun FFI call density is the remaining bottleneck. Select PERF-12 only on end-to-end application time, structural asymptotics, full semantic parity, and bounded long-running live memory.**

---

# Tranche implementation records

## T1 implementation record

### 1. Scope statement

```text
tranche:      T1 (PERF-12.0 evidence freeze and probes)
parent:       12.0
sections:     §82 source freeze, §60 Bun qualification, §61 same-image audit,
              §57–§58 memory attribution protocol and classification,
              §83 direct-call floor probe, §109 baseline record deliverables
```

### 2. Commits

```text
ae60ed1  bench(tui): freeze PERF-12 baselines and attribute native view memory
```

This record was appended in the immediately following documentation commit on
`perf-refactor`. No other commits belong to T1.

### 3. Review findings

```text
gap 1: the current diagnostic surface (tuiViewAbiBootstrap) does not expose
       node_refs entry counts or path_keys counts required by §57. Correction:
       recorded as explicit nulls in PERF-12-memory-attribution.jsonl with a
       pointer to Tranche 3 (§89) rather than extending the diagnostic ABI in
       T1, because T1 must not change production code.
gap 2: PersistentSeq structural high-water (§58 bucket H) has no dedicated
       counter in the frozen artifact. Recorded as a stated T1 limitation;
       classification for H is deferred to the Tranche 3 counter build.
gap 3: the wide/100000 block retains 239.5 MiB JS heap after gc(true). No
       PERF-12 gate depends on it and the §57 escalation rule ("if JS heap
       remains unexpectedly high, emit a heap snapshot") was evaluated against
       the GiB-scale RSS question, which native counters already answer; the
       residual is noted in PERF-12-baseline.md §8 instead of chased further.
```

### 4. Implementation summary

What now exists:

```text
PERF-12-baseline.md                      archaeology + freeze + attribution + probe decision record
packages/iyon-runtime/bench/perf12_memory_attribution.ts   isolated-block parent harness
packages/iyon-runtime/bench/perf12_memory_child.ts         phase-instrumented child runner
packages/iyon-runtime/bench/perf12_ffi_floor.ts            §83 call-floor probe
packages/iyon-runtime/bench/PERF-12-memory-attribution.jsonl
packages/iyon-runtime/bench/PERF-12-ffi-floor.jsonl
```

Structurally nothing changed in production code: the shared runtime, Direct
decoder, generated ABI, and View backing are byte-for-byte the frozen
PERF-11v4 state. Deliberately not done yet: all architecture work (semantic
DAG restoration, publication helper, NativeRef table, scavenging, direct
materializers, routing) belongs to T2+; production routing remains untouched.

### 5. Provenance block

```text
source revision at capture: 3d156a78e1577b4b2d491cf393b08956cf2aa7f5 (clean tree)
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
native artifact SHA-256:    81a1682d90f3b0be14fb0bb5cd07007c6e1a6b2a9c09158ff2e45be8aff54a9e
schema BLAKE3:              f7b30e32493e2e95f86541401308e5db64103bd8a7e694cbecbfe851040025d3
generator BLAKE3:           20435cb0e211e543dd671e6c86669cf3f205c8e77c5070f47f4d181a4a9d3c71
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Deliverables committed:
   PERF-12-baseline.md, PERF-12-memory-attribution.jsonl,
   PERF-12-ffi-floor.jsonl committed in ae60ed1. PASS.

2. 2.7 GiB classified into §58 buckets:
   six blocks reproduced against the frozen artifact (peaks 38 MiB – 6,613 MiB;
   the SHARED_DEEP 10k block reproduced the 11v4 6.2–6.5 GiB peak at 6.6 GiB).
   After forced cleanup checkpoints every block shows:
     leased_slots = 0, native_ref_slots = 0,
     semantic_cache_entries -> 0 (wide block: 100,069 -> 0; trace: 4,297 -> 1 live)
   while RSS remains 1,279–4,315 MiB with 3.5–15.3 MiB JS heap
   (wide block 2,451 MiB RSS / 239.5 MiB heap).
   Classification: dominant buckets D/J (JSC/JIT/allocator address-space
   high-water); A negligible (~8–16 KiB sample storage); B/C MiB-scale;
   E zero; F transient but sweep-convergent (no linear post-sweep slope);
   G ≈ 0 on the Direct path; H not instrumented (stated limitation);
   I zero in all blocks. PASS.

3. FFI call floor compatible with expected changed-frontier budget:
   measured on the pinned revision, 30 warmup + 50 measured blocks x 1,000 ops
   per timed block (timing build, release artifact):
     noop chain x1        8 ns/op          (x64: 107 ns ~ 1.7 ns/call)
     generated dispatch   10 ns/call
     scalar constructor   246 ns
     fixed-arity create2  478 ns
     ref-buffer create4   693 ns
     retained patches     429–436 ns
   Projected frontier cost at the worst common shape vs frozen PERF-11v4
   direct_7v2 total medians:
     frontier 8   -> 5.5 µs   = 11.5% of 48.4 µs    OK
     frontier 32  -> 22.2 µs  = 10.5% of 211.0 µs   OK
     frontier 128 -> 88.7 µs  =  0.09% of 101.6 ms  OK
     frontier 200 -> 138.6 µs = 11.7% of 1.18 ms    OK
   Decision threshold (≤25% of budget, noop < 100 ns/call): met with ≥8x
   headroom. §83 decision: GO. PASS.

4. Stop conditions:
   §105 does not apply — PERF-11v4 category D (Candidate A wins the realistic
   trace 1.977x), so continuing is mandated. §83 returned GO. No stop.
```

### 7. Status line

**Tranche T1 status: COMPLETE.** Baselines are frozen, the reported large-RSS
behavior is attributed (allocator/JIT high-water over transient-but-sweep-
convergent weak-cache metadata, not live semantic state), and the direct-call
floor clears its decision threshold with headroom; T2/T3 may proceed. The
bucket-F mechanism confirms Tranche 3 weak-cache scavenging remains a mandatory
shared-runtime prerequisite before memory gates can be judged.

## T2 implementation record

### 1. Scope statement

```text
tranche:      T2 (PERF-12.2a shared runtime publication)
parent:       12.2 (T2+T3 share commit 3 of §108; this tranche is the
              publication/representation half)
sections:     §24 one central publish_semantic_view helper,
              §25 classify existing 11v3 ABI functions,
              §52–§53 NativeRef paged table design and decision,
              §54 page reclamation,
              §88 deliverables checklist (shared with T3; the weak-cache
              maintenance item belongs to T3)
```

### 2. Commits

```text
2da3813  perf(tui): unify semantic publication and adopt paged NativeRef table
```

This record was appended in the immediately following documentation commit on
`perf-refactor`.

### 3. Review findings

```text
finding 1: the pre-T2 runtime had four near-duplicate identity-rule
       implementations: `publish` (leased), `publish_bulk` (weak-only), the
       staged publication prepare/commit pair, and blind weak inserts from the
       Direct N-API decoder (crates/iyon-native/src/tui.rs) plus the packed
       decoder (tui/packed.rs). Correction: all now funnel through shared rules
       on NativeViewRuntime (`publish_semantic_view` +
       `consult_semantic_identity` + `install_semantic_view` for ref-producing
       transports; `record_decoded_semantic_view` for decode-style
       transports). The only remaining direct cache accesses are the admin
       hooks tuiPerfResetViewBridgeCache / size probes, which are not
       publications.

finding 2: `bun test packages/iyon-runtime/tests` (full directory) shows one
       failing test — perf11v4 "reconstructs correctly after the weak cache
       expires" asserts global live_weak_upgrades == 0, but tui_demo,
       tui_native_persistent_seq, and tui_native_transaction legitimately leave
       live Views in the shared per-environment runtime. Bisected pairwise and
       confirmed byte-identical on pre-change HEAD: this is pre-existing
       cross-file interference, not a T2 regression. The suite passes in
       isolation (the mode used by the PERF-11v4 report).

finding 3: `cargo test --workspace` shows three pre-existing api-surface
       failures (generated manifest drift, missing TS declaration for
       View::native_axis_from_children, checked-in artifact freshness) that are
       byte-identical on pre-change HEAD. They concern iyon-tui/generated
       surface state untouched by T2.

finding 4: clippy parity was verified against HEAD (227 warnings before,
       228 after, no new lint classes in view_abi.rs beyond one moved
       pre-existing manual_is_multiple_of pattern); rustfmt applied.
```

### 4. Implementation summary

What now exists:

```text
NativeViewRuntime::publish_semantic_view(node_id, view, PublicationLease)
    single identity funnel: validate NodeId -> consult semantic identity ->
    reject conflicts (FAST_INVALID) -> reuse live NativeRef (re-acquiring or
    not per lease mode) or allocate fresh -> install slot/cache/ref entries
publish / publish_bulk          thin delegations (Leased / Weak)
commit_staged_publication       installs entries via install_semantic_view
NativeRefTable<PAGE_BITS=12>    dense paged slots storage replacing
    HashMap<u32, NativeViewSlot>; monotonic non-reused refs (§53); pages carry
    live counts and are dropped at zero (§54) while the directory keeps its
    high-water length; stale refs into dropped pages are plain cache misses
record_decoded_semantic_view    shared weak-cache insertion rules for Direct
    and packed decoders, including the single size-based retain cleanup
PERF-12-nativeref-table.jsonl   representation benchmark artifact
```

Deliberately NOT done yet: weak-cache scavenging/maintenance counters and the
memory diagnostic ABI extension belong to T3; no PERF-12 JS-side transport
(BridgeNativeHint, ensureNative, materializers) exists yet; production routing
is unchanged.

§25 classification of all 49 generated ABI functions:

```text
A. semantic constructor/edit, remains useful unchanged (32):
   host_render_ref, view_render_ref, view_spacer_create,
   view_text_layout_patch_root, view_common_patch_root,
   view_axis_create_buffer, view_row_create_0..4, view_column_create_0..4,
   view_axis_set_child, view_axis_splice_buffer, view_grid_set_cell,
   view_release_many, view_ref_for_node_id, style_atom_create_cstring,
   style_create_bits, view_text_create_cstring(_2..4),
   view_text_create_utf8(_2..4)
B. useful implementation needing later cleanup (4):
   edit_txn_begin/add_text_layout/commit_render/abort — superseded by the
   planned JS-side MaterializeTx (§44); retained until boundary routing lands
C. path/recipe machinery made redundant by the semantic DAG (9):
   path_root, path_child, view_axis_set_child_path, view_grid_set_cell_path,
   view_text_layout_patch_path(_d1..d4)
D. benchmark/fallback only (4):
   runtime_noop, axis_builder_begin/push/finish/abort
```

No function was rewritten merely to rename it PERF-12 (§25 rule).

### 5. Provenance block

```text
source revision at capture: 2da38138a0f0af249e890bfe588438f70229e279
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      69319658d0570761fe13e994cc5f2e60b054f1bdf81936d58d1c5d3b3931e448
schema BLAKE3:              f7b30e32493e2e95f86541401308e5db64103bd8a7e694cbecbfe851040025d3 (unchanged; no ABI schema change)
generator BLAKE3:           20435cb0e211e543dd671e6c86669cf3f205c8e77c5070f47f4d181a4a9d3c71 (unchanged)
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. All transports route publication through one helper, no identity fork:
   ref-producing paths (generated constructors, patches, path publications,
   staged commits) all reach publish_semantic_view/install_semantic_view;
   decode paths (Direct N-API, packed) share record_decoded_semantic_view.
   Verified by code inspection plus the unchanged behavioral suites below:
   cross-transport identity tests in tui_generated_view_abi (4 pass) and
   tui_packed (8 pass) exercise Direct-vs-generated and packed-vs-runtime
   sharing through the same environment cache.

2. Existing 11v3/Direct regression suites pass unchanged:
     perf11v4_direct.test.ts            7 pass / 0 fail
     tui_generated_view_abi             4 pass / 0 fail
     tui_native_builder                 2 pass / 0 fail
     tui_native_scalar                  4 pass / 0 fail
     tui_native_strings                 2 pass / 0 fail
     tui_native_transaction             1 pass / 0 fail
     tui_native_persistent_seq          2 pass / 0 fail
     tui_packed                         8 pass / 0 fail
     tui_runtime                        2 pass / 0 fail
     tui_values                         8 pass / 0 fail
     tui_fast_shared                   10 pass / 0 fail
     tui_demo/handles/harness/pipeline/surface/traits   all green
     bun14_ffi_probe                    JIT noop 4.97 ns, conformance pass
     cargo test -p iyon-native         22 unit tests pass (+2 new table tests)
   Full-directory bun run carries the one pre-existing interference failure
   documented under Review findings; identical before and after T2.
   cargo workspace failures are the three pre-existing api-surface drifts,
   also identical before and after.

3. NativeRef representation chosen by measurement:
   PERF-12-nativeref-table.jsonl (release build, 8,192 live slots,
   4,000,000 lookups, checksum-verified):
     hash_map      22.66 ns/lookup (run 1), 17.65 ns (run 2)
     paged 10-bit   5.23 ns / 5.24 ns
     paged 12-bit   5.20 ns / 5.22 ns
   Adopted: paged table, PAGE_BITS = 12 (~3.4–4.3x faster than HashMap;
   page size decided by §53's smaller worst-case directory since 10 vs 12
   lookup cost is statistically identical). Empty-page reclamation covered by
   native_ref_table_maps_refs_across_pages unit test.
   Post-adoption timing sanity: PERF-12 ffi floor probe re-run shows no
   regression (worst common shape median 693 ns -> 561 ns, within noise);
   diagnostics smoke block still converges to zero expired metadata.
```

### 7. Status line

**Tranche T2 status: COMPLETE.** One shared semantic publication funnel and one
decode-cache rule set now serve every transport, and the NativeRef table is a
measured paged representation; all pre-existing behavior suites pass unchanged
with the two documented pre-existing full-suite failures unrelated to this
tranche. Weak-cache scavenging, maintenance counters, and the churn acceptance
gate remain for T3.

## T3 implementation record

### 1. Scope statement

```text
tranche:      T3 (PERF-12.2b bounded lifetime)
parent:       12.2 (shares §108 commit 3 with T2)
sections:     §55 weak-cache scavenging, §56 maintenance counters,
              §89 memory diagnostic ABI, §59 churn acceptance gate,
              §111 slot lease invariant tests, §110 weak-cache gate rerun
```

### 2. Commits

```text
c43f830  perf(tui): bound native semantic cache and ref metadata lifetime
```

This record was appended in the immediately following documentation commit on
`perf-refactor`.

### 3. Review findings

```text
finding 1: the first churn-gate run FAILED (11,373 entries, linear slope) and
       the leak was in the gate harness itself, not the runtime: each root
       replacement dropped the old boundary lease but never released the
       generation's final wide-edit chain column lease, leaking one column
       plus 512 children per generation (~1,036 entries per checkpoint,
       matching the observed slope exactly). Correction: the §18 boundary
       protocol in the harness now releases every lease the previous
       generation held. After the fix the gate passes with flat checkpoints.
       Recorded because the gate did exactly its job: it refuses to pass
       while any lease leaks.

finding 2: scavenge candidates enqueued by a release batch are processed by
       the NEXT maintenance pass, not the same one. This is deliberate: a
       View that expires after its release is only then reclaimable, and
       processing-before-enqueue gives continuous churn (the common case)
       one-pass reclamation latency while keeping the release hot path free
       of same-batch scans.

finding 3: `string_bytes` from §89's conceptual snapshot is reported as null:
       retained text payload byte accounting does not exist in the current
       runtime and adding it would require touching every text publication
       path. Deferred rather than reporting a misleading zero.

finding 4: §56 counters are implemented as plain u64 field increments that
       are always compiled in ("compile-time-cheap" arm of §56/§101) instead
       of feature-gated fields, matching the existing stale_removals/
       release_batches precedent. No scans or atomics were added to hot
       paths; the full sweep remains amortized by the 4096-growth threshold.

finding 5: the pre-existing full-directory bun interference failure and the
       three api-surface workspace drifts documented in the T2 record remain
       unchanged (verified identical behavior; not touched by T3).
```

### 4. Implementation summary

What now exists:

```text
NativeViewRuntime maintenance core (§55):
    scavenge_queue (zero-lease candidate refs enqueued on release)
    maintain_bounded(): drains up to 256 candidates per call, reclaiming
        slots whose View expired after release; threshold backstop runs
        prune_expired() when weak-cache growth since the last sweep
        exceeds 4096 insertions
    maintain(full) / tuiViewAbiMaintain(full) explicit hook for
        tests/benchmarks (§88 deliverable)
Counters (§56): semantic_cache_expired_seen, semantic_cache_full_sweeps,
    semantic_cache_entries_removed, native_ref_expired_slots_removed,
    native_ref_pages, native_ref_pages_freed, node_ref_map_entries,
    scavenge_queue_len, scavenge_processed, nodes_inserted_since_full_sweep
Diagnostic ABI (§89): bootstrap diagnostics extended with the counters above;
    new tuiViewRuntimeMemorySnapshot(count_live) returns the §89 snapshot
    shape (expensive live-count scans only on request); native.ts addon
    interface extended
Lease invariant tests (§111): eight new Rust tests
Churn gate (§59): packages/iyon-runtime/bench/perf12_churn.ts +
    PERF-12-churn.jsonl; §110 rerun in PERF-12-memory-attribution-t3.jsonl
```

Deliberately NOT done yet: no PERF-12 JS transport exists; production routing
unchanged; retained text payload byte accounting deferred (finding 3).

### 5. Provenance block

```text
source revision at capture: c43f8307e441cfa126997ab25461dfe16cd4b3c0
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      ab62b3d58c59a83bad7274518f68190c0511299026845fc7dd39fd8a13ccf2b2
schema BLAKE3:              f7b30e32493e2e95f86541401308e5db64103bd8a7e694cbecbfe851040025d3 (unchanged)
generator BLAKE3:           20435cb0e211e543dd671e6c86669cf3f205c8e77c5070f47f4d181a4a9d3c71 (unchanged)
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Post-maintenance weak/slot metadata = O(live + bounded slack):
   unit test slot_metadata_scavenged_after_weak_expiry demonstrates the full
   model end to end: slots reclaimed inline/bounded-pass, expired weak-cache
   entries held as bounded slack (65 entries incl. one pending) until the
   full sweep returns them to exactly 0. Churn gate final state after 1M
   transients: semantic_cache_entries 1525 vs live floor 812 (slack 713,
   bound 1024); native_ref_slots 1525 == entries; node_ref_entries 0.

2. 1M-transient-node churn shows no linear post-GC slope:
   PERF-12-churn.jsonl (1,000,000 transient views, 100 retained at
   1/10,000, ~200-node live rendered tree re-installed via hostRenderRef,
   root replacement every 50k ops, wide edits every 500 ops on a 512-child
   live column, text churn throughout; 10 checkpoints at 100k cadence plus
   final):
     steady-state entries min/max: 1435/1525 (delta 90 over 1M ops)
     slope: 1e-4 entries/op (bound: 1024 absolute slack)
     leased slots: 102 = 100 retained + 1 root + 1 edit-base exactly
     wall time 1184 ms
   PASS. The pre-fix failing run (11373 entries, linear) is retained history
   in the record as evidence the gate discriminates.

3. Counters absent or compile-time-free in timing builds:
   all §56 counters are single u64 adds on already-executing paths; no hot
   path performs scans; full sweeps amortize one O(live+slack) pass per
   4096 insertions; the expensive live-count scans exist only behind the
   explicit count_live flag of tuiViewRuntimeMemorySnapshot. Verified by
   inspection; the ffi floor probe re-run in T2 (worst shape 561 ns) shows
   no timing-path instrumentation cost.

§111 slot lease invariants (new tests, all passing):
   new constructor returns lease count 1        new_constructor_returns_lease_count_one
   child temp lease live until root completes   child_temp_lease_stays_live_until_root_completes
   batch release drops child temp leases        batch_release_drops_child_temp_leases
   root lease transfers to boundary             root_lease_transfers_to_boundary_after_new_install
   failed host install retains old root         failed_host_install_retains_old_root
   failed transaction drains every temp lease   failed_transaction_releases_every_new_temp_lease
   stale unleased weak slot -> CACHE_MISS       stale_unleased_weak_slot_returns_cache_miss
   metadata scavenged after weak expiry         slot_metadata_scavenged_after_weak_expiry

§110 weak-cache gate rerun (post-T2/T3, artifact ab62b3d5...):
   PERF-12-memory-attribution-t3.jsonl: all six blocks converge to
   cache=0/slots=0/leased=0 after the forced-cleanup checkpoint; RSS classes
   unchanged from T1 (allocator high-water dominant). The shared-runtime
   cache-lifetime fix therefore does not materially move the 2.7 GiB-class
   RSS figures (they were never live-state), and is recorded separately from
   any future PERF-12 transport win as §110 requires.

Regression suites (unchanged behavior):
   perf11v4_direct 7/7, generated_view_abi 4, native_builder 2, native_scalar
   4, native_strings 2, native_transaction 1, native_persistent_seq 2,
   packed 8, runtime 2, values 8, fast_shared 10 — all green; cargo
   iyon-native 30/30 (+8 new); bun14_ffi_probe conformance pass; rustfmt
   clean; clippy parity with HEAD maintained; tsc clean.
```

### 7. Status line

**Tranche T3 status: COMPLETE.** The shared runtime now bounds weak-cache and
slot metadata automatically (bounded candidate passes plus a threshold-backstop
full sweep), exposes the §56/§89 counter and snapshot surface, and passes the
§59 1M-transient churn gate with O(live + bounded slack) post-maintenance
metadata and zero slope. T2+T3 together complete §108 commit 3's scope; T4
(faithful semantic DAG restoration) may proceed.

Errata (added by the post-T1..T3 specification review): the gate-evidence list
above omitted two explicit mappings that nonetheless hold. First, the §111
scenario "old root lease released only after new host install succeeds" is
covered jointly by `root_lease_transfers_to_boundary_after_new_install`
(success path: old root slot removed only after the successful install) and
`failed_host_install_retains_old_root` (failure path). Second, the remaining
§59 required lines: transport persistent bytes are structurally zero (no
transport node/edge arena exists anywhere in the runtime — verified by
inspection), and scratch stays within configured caps because the churn gate
allocates exactly two fixed buffers (a reused 2 KiB child-ref scratch sized
for the 512-child wide column and one display scratch per generation), far
below the §30 tier caps.

## T4 implementation record

### 1. Scope statement

```text
tranche:      T4 (PERF-12.1 semantic DAG restoration)
parent:       12.1
sections:     §84 faithful 7v2 reconstruction, §13 JS representation,
              §14 BridgeViewNode shape, §15 sidecar inventory,
              §16 hint-not-lease lifetime rule, §17 no-FinalizationRegistry
              rule, §86 semantic parity, §85 construction gate
scope extension (user-directed): production now USES the eager DAG
              (values/view.ts converted; not a bench-local candidate), the
              measured 7v2 construction speed gains are fully recovered,
              and all transports other than direct_7v2 and PERF-12 were
              declared ruled out by the user, so their JS-side machinery was
              removed ahead of the conditional cleanup tranche with this
              record as the documenting commit.
```

### 2. Commits

```text
b224cc3  refactor(tui): restore eager immutable semantic View DAG
```

This record was appended in the immediately following documentation commit.

### 3. Review findings

```text
finding 1 (pre-implementation challenge): before implementing, the handoff
       premise "eager 7v2 construction is cheap" was challenged against the
       pending-backing production model at the user's request. Measured on
       the pinned runtime (PERF-12-construction-challenge.jsonl, pre-T4):
       render-ready production construction was 0.22x-0.50x of the faithful
       7v2 reference across nine cases (i.e., eager was 2-4.5x faster). The
       premise holds; T4 proceeded. Verdict recorded here because it is the
       empirical basis for promoting the candidate to production.

finding 2: production had drifted from 7v2 in one visible way - modifier
       styles on plain text were pushed down into text spans instead of
       wrapping in a decorated node. The pushdown was re-evaluated under the
       eager model and dropped: one modifier costs 1,145 ns via span
       pushdown vs 661 ns via the faithful decorated wrapper, and the two
       are render-equivalent. Faithful 7v2 semantics win; if span-merging
       ever measures as a net win end-to-end it belongs as a §27 derivation
       hint (T9), not as a semantic-shape change.

finding 3 (user ruling): packed V3/V4, fast-shared, and every other
       transport candidate are definitively ruled out. Their JS-side
       machinery was removed: values/view.ts no longer calls
       registerPackedMeta at construction and lost the *ForPackedTransport
       statics and sequence-override decode shims; src/tui/packed_v3.ts,
       packed_v4.ts, fast_shared.ts, their three test suites, and the
       historical tui_decision/tui_performance/perf11v4 comparison bench
       scripts were deleted (the bench:tui-decision package script went with
       them). The native packed decoders in iyon-native are untouched - T4
       remains a no-native-change tranche; they are simply unreachable from
       the TS layer and become cleanup-tranche material.

finding 4: the recipe reader functions (nativeAxisRecipe, nativeTextRecipe,
       nativeSpacerRecipe, nativeScalarPatch, nativeStructuralEdit,
       viewBackingState) remain exported as documented always-undefined
       stubs so native_view_abi.ts's dead route code still compiles. Under
       the eager DAG those routes never fire; renders land on the Direct
       decode path, which PERF-11v4 category D already showed is faster on
       realistic traces. Route-code removal happens with the cleanup tranche
       or when T6+ reroutes rendering onto retained-DAG FFI.

finding 5: tests asserting ruled-out behavior were adapted or removed per
       user direction rather than kept passing artificially:
         perf11v4_direct.test.ts   rewritten as the single-arm Direct-
                                   decoder correctness suite (schema render,
                                   schema-validation rejection, NodeId cache
                                   identity, retained modes, weak-cache
                                   expiry reconstruction, 100-seed random
                                   coverage); the two-arm differential
                                   against the historical candidate module
                                   collapsed because production now IS the
                                   candidate.
         tui_native_persistent_seq.test.ts  adapted: edited views are built
                                   eagerly through the public API; the native
                                   wide-edit primitives it exercises
                                   (view_axis_set_child / splice_buffer /
                                   view_grid_set_cell) are PERF-12 §35/T10
                   surface and remain covered.
         tui_native_strings.test.ts  adapted: the pre-materialization call
                                   through the dead native single-text
                                   builder route was removed; Unicode/NUL/
                                   surrogate parity is verified through the
                                   production router vs the Direct oracle.
         tui_packed_v3/v4/tui_fast_shared tests removed with their modules.
```

### 4. Implementation summary

What now exists:

```text
values/view.ts (production): eager frozen BridgeViewNode per View;
    monotonic 53-bit-safe NodeId at semantic construction; parent nodes
    reference child nodes directly; unchanged child Views share the exact
    child node object; nodeForBridge = WeakMap lookup + throw; WeakMap
    sidecars only for nativePath lineage/transaction metadata; no pending
    state machine, no packed metadata, no FinalizationRegistry
retained-path/transaction statics kept (pure semantic transforms):
    View.textLayoutAtNativePathForTransport,
    View.textLayoutTransactionForTransport + patchBridgeTextPath machinery
removed from production surface: pending backings, makeBacking,
    materializeBacking, packed statics, nodeForDirectBridge,
    registerPackedMeta coupling; deleted modules: packed_v3.ts, packed_v4.ts,
    fast_shared.ts (+3 test files, +6 historical bench scripts incl. the
    perf11v4 comparison harness whose artifacts were already frozen)
perf12_view_7v2.ts stays as the independent faithful-reference module used
    by the construction gate
```

Deliberately NOT done yet: no PERF-12 FFI materializers/hints exist (T6+);
render routing still walks the dead recipe routes before Direct fallback
(harmless, removed later); native packed decoders untouched.

### 5. Provenance block

```text
source revision at capture: b224cc3fd0942062317d1c2d6dccd27322bd8525
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
native artifact SHA-256:    ab62b3d58c59a83bad7274518f68190c0511299026845fc7dd39fd8a13ccf2b2
                            (unchanged from T3 - no native change in T4)
schema BLAKE3:              f7b30e32493e2e95f86541401308e5db64103bd8a7e694cbecbfe851040025d3 (unchanged)
generator BLAKE3:           20435cb0e211e543dd671e6c86669cf3f205c8e77c5070f47f4d181a4a9d3c71 (unchanged)
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Full-schema semantic parity against current production Views:
   parity is now structural identity (production is the eager DAG), and
   render correctness is covered by: perf11v4_direct.test.ts 6/6 (full-
   schema render, schema-validation rejection, identity/cache-hit, five
   retained modes, weak-cache expiry reconstruction, 100 randomized seeds),
   tui_values 8/8, tui_semantic_pipeline 3/3, tui_harness 4/4,
   synthetic-agent 1/1, cancellation 1/1 - all green post-conversion.
   The pre-T4 two-arm differential suite historically proved current-vs-
   7v2 equivalence over the same fixtures; that comparison is now identity.

2. retained_dag_ffi semantic construction <=5% vs faithful Bun 1.4 direct_7v2:
   PERF-12-construction-challenge.jsonl (post-conversion run; 30 warmup +
   50 measured rounds x 1,000 ops/block, order-rotated, gc between blocks;
   v7_ready/prod_ready medians):
     plain_text            329ns vs 340ns     ratio 1.026
     styled_text_3spans    662ns vs 658ns     ratio 1.007
     modifier_chain_3     2056ns vs 2011ns    ratio 1.022
     column_20           10540ns vs 10745ns   ratio 0.981
     column_200         140596ns vs 139380ns  ratio 1.009
     row_tracks_mixed     1649ns vs 1542ns    ratio 1.069
     grid_3x3             6584ns vs 6485ns    ratio 1.015
     diff_10_lines        1343ns vs 1334ns    ratio 1.007
     agent_message      17954ns vs 17971ns    ratio 0.999
   Production is within noise of the faithful reference on every case
   (largest deviation 6.9%, in production's favor). PASS ("within noise"
   preferred target met in both directions).

   Speed gains recovered vs the pre-T4 pending model (prod_ready medians,
   pre-T4 -> post-T4):
     plain_text          1166ns ->  340ns   (3.4x)
     styled_text_3spans  2481ns ->  658ns   (3.8x)
     modifier_chain_3    4833ns -> 2011ns   (2.4x)
     column_20          37620ns -> 10745ns  (3.5x)
     column_200        480123ns -> 139380ns (3.4x)
     agent_message      60421ns -> 17971ns  (3.4x)

3. nodeForBridge is lookup-only:
   implementation is `nodes.get(view)` + throw; verified by inspection and
   by the identity assertion in perf11v4_direct (same frozen object returned).

§13-§17 property checklist: no transport own-properties on BridgeViewNode
   (nodes carry id/schema/kind/payload/children only); sidecars are typed
   WeakMaps created lazily and never during plain construction; NativeRef
   hints do not exist yet (T6) and nothing takes per-node leases;
   no FinalizationRegistry anywhere in the module.
Regression battery post-change: 18 bun suites green (56 tests), cargo
   iyon-native 30/30, rustfmt clean, tsc clean, memory-attribution pipeline
   smoke-passed against the slimmed fixtures.
```

Errata (added by the post-T4/T5 end-to-end review; no code changes required,
all gates re-verified green at the review revision):

```text
eratum 1: the §85 case list above does not consistently follow its own
   "v7_ready/prod_ready" column header. The plain_text row lists production
   first (329 ns prod_ready, ~340 ns v7_ready) while column_20 lists v7
   first (10,540 ns v7_ready, 10,745 ns prod_ready). The authoritative
   figures are the committed v7_ready_over_prod_ready_median_ratio values in
   PERF-12-construction-challenge.jsonl (range 0.981-1.069; worst-case
   production regression 1.9% at column_20, inside the <=5% gate; several
   cases show production faster).
eratum 2: the JSONL summary line's git_sha records 741ac6a because the
   post-conversion construction-gate run was captured on that base commit
   with the values/view.ts conversion applied but not yet committed. The
   case rows are the post-conversion measurements cited above (prod ≈ v7),
   not the pre-conversion 0.22x-0.50x challenge run, which the diff in
   b224cc3 replaced.
review note: `bun test` across suite directories reports one failure -
   perf11v4_direct "reconstructs correctly after the weak cache expires"
   asserts live_weak_upgrades == 0 while tui_demo, tui_native_persistent_seq,
   and tui_native_transaction legitimately leave live Views in the shared
   per-environment runtime. This is the same pre-existing cross-file
   interference bisected and documented in the T2 record; the suite passes
   in isolation and was left unchanged.
```

### 7. Status line

**Tranche T4 status: COMPLETE.** Production now constructs the faithful
7v2 eager immutable semantic DAG with lookup-only nodeForBridge, matches the
faithful reference within noise on the §85 construction gate, and recovers
the full 2.4-3.8x construction speedup over the pending model; ruled-out
transport machinery was removed from the JS layer with this record as its
documenting commit. Remaining for later tranches: BridgeNativeHint sidecar
wiring and FFI materialization (T6/T7), wide-edit sidecars (T10), and final
removal of the dead route code (cleanup tranche).

## T5 implementation record

### 1. Scope statement

```text
tranche:      T5 (PERF-12.4a generator foundation)
parent:       12.4 (T5 precedes all generated transport work)
sections:     §62 extend canonical ABI pipeline, §63 MaterializerSpec model,
              §64 generator validation rules, §65 output placement,
              §74 failure status detail, §68 checked-vs-timing policy,
              §69 owner-thread policy
vertical slice: the spacer kind, emitted end-to-end from the canonical
              schema into generated TypeScript consumed against the shared
              runtime through a real host
```

### 2. Commits

```text
df37cec  feat(generator): emit semantic materializer vertical slice
```

This record was appended in the immediately following documentation commit.

### 3. Review findings

```text
finding 1: the §63 model stores roles as validated strings plus a parsed
       MaterializerFieldRole enum rather than deserializing the enum
       directly, so serde diagnostics name the exact illegal role string
       before rejection. deny_unknown_fields is enforced on every
       materializer type as §63 requires.

finding 2: reference lowerings (child_ref/style_ref/base_ref) and buffer
       lowerings (ref_buffer/aux_buffer/byte_buffer) are fully modeled and
       validated (buffer bounds are mandatory) but their RENDERING panics
       generation with a message naming the owning tranche (T6/T7/T8).
       Rationale: emitting a materializer that calls an ensureNative that
       does not exist yet would generate dead or misleading code; the model
       is complete, the emitted surface is truthful. The T5 slice needs
       neither.

finding 3: §74 status detail is implemented as the convention layer -
       status_detail ("none" | "child_ref" | "base_ref") is a validated
       declaration per materializer, decodeMaterializeStatus() is generated
       shared surface, and each materializer exports its detail kind
       constant. Native builders do not emit detail cells yet (no child-ref
       constructor exists in the ABI); the first child-bearing materializer
       (T7) wires the native side of this convention.

finding 4: schema BLAKE3 and generator BLAKE3 changed legitimately in this
       tranche (view_abi.toml gained the materializer section; generator
       sources gained the renderer and validation rules). New pinned values:
       schema 2b797eccd4c6c803a51937b1344f29c27e6289ae5b4765a0a76bf082cb201f
       be, generator 581e146de3ee31e0ceb7b1292ca9a5ca487fb0ada2aa235857505a5
       5520467fa. The addon was rebuilt and restaged so its embedded
       handshake matches (SHA below).

finding 5: tui_generated_view_abi's function-count assertion still holds:
       materializers are declarations over existing ABI functions, not new
       [[function]] entries - the spacer builder reuses view_spacer_create
       exactly as §25 (reuse before adding) requires.
```

### 4. Implementation summary

What now exists:

```text
tools/tui-abi/view_abi.toml        [[materializer]] declaration block
tools/tui-abi-gen/src/model.rs     MaterializerSpec / MaterializerFieldSpec /
                                   MaterializerResultSpec / role enum
tools/tui-abi-gen/src/validate.rs  §64 rule set + 9 dedicated failure tests +
                                   slice declaration test (17 total)
tools/tui-abi-gen/renderers        view_materialize.ts output + manifest
                                   materializers section
generated outputs                  packages/iyon-runtime/src/tui/generated/
                                   view_materialize.ts (first-class, enforced
                                   fresh by tui-abi-gen check)
packages/iyon-runtime/tests/view_materialize.test.ts
                                   end-to-end conformance for the slice
```

Deliberately NOT done yet: no ensureNative/BridgeNativeHint (T6), no
per-kind full-schema coverage beyond the vertical slice (T7+ extends the
same [[materializer]] blocks), no buffer transport (T8), no native status
detail cells (first needed by T7).

### 5. Provenance block

```text
source revision at capture: df37cecaf3695578aefa94d8ad294dd9ce8557cf
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      c5d4acf061a92ef859cf34c094e029fd4d85afeed3be82d204e6891a53c10652
schema BLAKE3:              2b797eccd4c6c803a51937b1344f29c27e6289ae5b4765a0a76bf082cb201fbe
generator BLAKE3:           581e146de3ee31e0ceb7b1292ca9a5ca487fb0ada2aa235857505a55520467fa
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Generator emits a one-kind vertical slice end-to-end:
   view_abi.toml declares materializer "spacer" -> view_spacer_create;
   cargo run -p tui-abi-gen -- generate emits
   packages/iyon-runtime/src/tui/generated/view_materialize.ts;
   view_materialize.test.ts drives it end-to-end: minted ref > 0,
   NodeId halves match the shared split convention, §23 semantic-cache-first
   consultation (viewRefForNodeId returns the SAME ref), hostRenderRef
   renders blank rows through a real NativeTuiHost, release drains cleanly.
   PASS (3/3 tests).

2. Conformance tests pass:
   tui-abi-gen 17/17 (incl. snapshot freshness), iyon-native 30 lib +
   all generated suites green, bun battery 19 suites green including
   tui_generated_view_abi (function_count 49 unchanged), perf11v4_direct
   6/6, tui_values 8/8; tui-abi-gen check enforces every generated file is
   byte-fresh. PASS.

3. Illegal lifetime declarations fail generation:
   ten dedicated validation tests prove rejection of: unknown bridge kind,
   missing node_id_high half (§64 u64-narrowing rule), unknown field role,
   unbounded buffer field, borrow_duration != call (§107 retained-pointer
   ban), thread_affinity != owner_thread (§69), duplicate materializer name,
   empty benchmark registration, unknown builder function. PASS.
```

### 7. Status line

**Tranche T5 status: COMPLETE.** The canonical generator now models,
validates, and emits semantic materializers with the spacer kind working
end-to-end through the shared runtime; illegal lifetime declarations fail
generation with named rules. Full-schema materializers land by extending
[[materializer]] blocks in T7 once BridgeNativeHint and MaterializeTx
wiring exist (T6).

## T6 implementation record

### 1. Scope statement

```text
tranche:      T6 (PERF-12.3 identity fast paths)
parent:       12.3
sections:     §18 root lease protocol, §19 ensureNative core algorithm,
              §20 exact-root fast path, §21 stable subtree cutoff,
              §48 runtime generation handling, §15/§16 BridgeNativeHint
              sidecar wiring, §113 exact-identity scaling test
supporting:   §44 MaterializeTx (temporary lease transaction),
              §49/§50 fast-fallback routing surface + retained budgets,
              §47 single targeted retry, §75 cycle/depth guards,
              §91 structural counters (T6-relevant subset)
```

### 2. Commits

```text
4c91589  feat(tui): add retained-DAG identity fast paths and root lease protocol
```

This record was appended in the immediately following documentation commit on
`perf-refactor`. The committed benchmark artifact's summary `git_sha` records
`ebad4f5` (the implementation revision captured after the code was committed
but before the artifact was re-captured and the commit amended); the tree
content at `ebad4f5` and `4c91589` is identical except for the artifact file
itself. Stated here explicitly to avoid repeating the provenance imprecision
corrected by the T4 errata.

### 3. Review findings

```text
finding 1: the §18 protocol requires the boundary to OWN exactly one lease on
       its root, but a root resolved through a valid BridgeNativeHint is a
       borrowed ref whose lease belongs to another owner (e.g. a second host
       rendering the same semantic root, §115). Blindly transferring a
       borrowed ref into boundary.previousRef would release a foreign lease
       at close/replace. Correction: install() detects borrowed-hint roots
       (ref not in tx.temporaryLeases and different from previousRef) and
       acquires the boundary's own lease by NodeId before installation;
       re-installing the boundary's own current root reuses its existing
       lease instead of double-acquiring.

finding 2: viewRenderRef only resolves; it does not acquire a lease. Lease
       acquisition on an existing NodeId goes through viewRefForNodeId
       (ref_for_node_id -> acquire/ensure lease). All boundary lease
       acquisitions use the latter.

finding 3: §91 bridge_hint_hits initially counted only ensureNative hits.
       The §20 exact-root path consumes hints directly, so its hits (and the
       §47 recovery promotion) were invisible to the counters that prove the
       gate. Correction: renderExactRoot increments bridge_hint_hits,
       node_id_ref_promotion_attempts/hits, and stale_ref_retries on the
       same conventions as ensureNative.

finding 4: tuiViewRuntimeMemorySnapshot reports leased_slots only when
       count_live=true; an initial test draft read zeros and mis-attributed
       them. Tests now request the live scan. Because the Direct-decoded
       host already holds one lease on an adopted slot, the exactly-once
       close proof isolates the boundary's own lease by materializing a
       rootless spacer (tx temp lease A + boundary lease B on one slot),
       draining A, and asserting leased_slots +1/-1 around the boundary
       lifecycle.

finding 5: the full-directory bun interference failure documented in the T2
       record (perf11v4_direct "reconstructs correctly after the weak cache
       expires" vs suites legitimately leaving live Views) is unchanged by
       T6; the T6 suite passes in isolation and in the full run.
```

### 4. Implementation summary

What now exists:

```text
src/tui/retained_dag.ts
    BridgeNativeHint {generation, nativeRef} in WeakMap<BridgeViewNode,...>
    (§15/§16: weak acceleration, never a per-node lease)
    MaterializeTx (§44): refs map, inProgress set, temporaryLeases batched
        through one viewReleaseMany call, borrowedHints list, newNodeCount,
        depth; releaseAll()/releaseAllExcept(keepRef)
    ensureNative (§19): hard ordering hint -> tx-local -> ceiling-gated
        NodeId->NativeRef promotion -> generated materializer dispatch ->
        child traversal; cycle guard; §50 budgets; unknown kinds raise
        RetainedFastFallbackError for complete-cold-path routing (§49)
    renderExactRoot (§20): one hostRenderRef on a generation-valid hint;
        FAST_CACHE_MISS triggers the single targeted §47 retry (drop hint,
        re-promote, retry once)
    RetainedRootBoundary (§18): adopt/install/renderExact/close; previousRef
        leased across replacement, temp leases drain in one batch, failure
        keeps the old root, nativeLookupCeiling captured from the private
        NodeId high-water after every successful commit
    §91 counter subset + snapshot/reset (plain field increments,
        compile-time-cheap arm of §56/§68/§101)
native_view_policy.ts: MAX_RETAINED_NEW_NODES = 512, MAX_RETAINED_DEPTH = 256
values/view.ts: viewNodeIdHighWater() export for the §18 ceiling capture
bench/perf12_t6_exact_identity.ts + PERF-12-t6-exact-identity.jsonl
tests/perf12_t6_identity.test.ts (nine conformance/scaling tests)
```

Deliberately NOT done yet: production routing is unchanged — every boundary
still renders through the Direct decode path until T13 wires §18 boundaries
in; full-schema materializers are T7 (only the generator's spacer slice is
registered, so composite retained installs fall back by design); derivation
hints (§27) are T9; native status-detail cells (§74 native side) arrive with
the first child-bearing materializer in T7; borrowedHints consumption for
stale-child recovery is exercised by T12.

### 5. Provenance block

```text
source revision at capture: ebad4f5 (tree identical to committed 4c91589
                            except the artifact file; see Commits note)
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
native artifact SHA-256:    c5d4acf061a92ef859cf34c094e029fd4d85afeed3be82d204e6891a53c10652
                            (unchanged from T5 - no native change in T6)
schema BLAKE3:              2b797eccd4c6c803a51937b1344f29c27e6289ae5b4765a0a76bf082cb201fbe (unchanged)
generator BLAKE3:           581e146de3ee31e0ceb7b1292ca9a5ca487fb0ada2aa235857505a55520467fa (unchanged)
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Exact known root = 1 hostRenderRef, 0 semantic field reads, 0 buffer
   writes at 20/200/2k/10k node sizes:
   PERF-12-t6-exact-identity.jsonl (smoke profile, timing build, fresh
   process, 50 warmup blocks x 1,000 ops, 100 measured blocks per size,
   bootstrap CI half-width < 5% met at 100 blocks everywhere):
     size     median_ns   p95_ns   p99_ns   host FFI calls / renders
     20           87        131      136      100,000 / 100,000
     200          53         58       97      100,000 / 100,000
     2,000        53         76       94      100,000 / 100,000
     10,000       55         65       81      100,000 / 100,000
   Structural counters over every measured render at every size:
     bridge_hint_hits = renders; bridge_hint_misses = 0;
     bridge_semantic_nodes_inspected = 0; bridge_children_visited = 0;
     direct_materializer_calls = 0; node_id_ref_promotion_attempts = 0;
     ref_words_written = 0; byte_payload_bytes = 0; stale_ref_retries = 0;
     cold_fallbacks = 0. The benchmark aborts on any violation, so PASS is
     structural, not sampled.

2. Timing independent of descendant count:
   median ratio 10,000/20 = 0.626 (second capture; first capture 0.616) —
   the largest tree renders no slower than the smallest; the native host
   short-circuits an unchanged body ref, so each exact render is one
   engine-native call plus identity comparisons. PASS.

3. Stale generation hints ignored correctly:
   perf12_t6_identity.test.ts "§48: stale generation hints are ignored and
   re-derived": a hint forced to generation+10,000 is skipped, the NodeId
   promotion recovers the identical NativeRef, and the re-installed hint
   carries the current generation. PASS.

§113 exact-identity scaling test:
   "§113 structural proof" runs the full counter assertion set at
   20/200/2,000/10,000 in-suite (100 exact renders per size, all deltas
   zero except host_mutations = renders). PASS.

Section coverage:
   §18  RetainedRootBoundary; tests: failed-install lease draining,
        exactly-once close (leased_slots +1/-1 around the boundary
        lifecycle), §115-lite shared-root boundaries, dormant re-adopt.
   §19  ensureNative ordering tests: ceiling gating skips the probe for
        post-commit NodeIds (promotion_attempts +0, materializer_calls +1,
        §23 cache-first consult returns the same ref); §21 hinted subtree
        cutoff (hint hit -> ref returned with 0 field reads, 0 children,
        0 leases taken, borrowedHints +1).
   §20  renderExactRoot + §47 single-retry CACHE_MISS recovery test
        (sabotaged hint recovers once, re-renders correctly, counter
        stale_ref_retries increments by exactly the one retry).

Regression battery: perf12_t6_identity 9/9; tests directory 72 pass /
1 fail (the T2-documented pre-existing cross-file interference, identical
before and after); test+tests 121 pass / 1 fail; tsc clean; tui-abi-gen
check byte-fresh; tui-abi-gen 17/17; cargo iyon-native 30 lib + generated
suites green; cargo fmt clean. No native change, so the T5 addon artifact
remains authoritative.
```

### 7. Status line

**Tranche T6 status: COMPLETE.** The identity layer — generation-scoped
NativeRef hints, ceiling-gated NodeId promotion, exact-root fast path with
one-retry recovery, and the §18 boundary lease protocol — is implemented,
structurally proven at 20/200/2k/10k with flat ~53–87 ns exact renders, and
covered by nine conformance tests; production boundaries still route through
Direct until T13, full-schema materializers land in T7, and derivation hints
in T9.

## T7 implementation record

### 1. Scope statement

```text
tranche:      T7 (PERF-12.4b common-node direct materializers)
parent:       12.4
sections:     §22 children-first materialization, §23 native constructor
              semantic-cache-first rule, §32 fixed-arity specialization,
              §66 generated TypeScript style rules, §67 native ownership
              split, §75 cycle/work budgets, §50 retained work budget,
              §51 no-full-tree-diff rule
supporting:   §63/§64 generator model + validation extension,
              §65 generated output placement, §25 reuse-before-adding
```

### 2. Commits

```text
36a1856  feat(tui): materialize common nodes through monomorphic generated FFI
```

This record was appended in the immediately following documentation commit.
The committed benchmark artifact's summary `git_sha` records `50549dc` (the
implementation commit before the artifact was re-captured and the commit
amended; tree content identical to `36a1856` except the artifact file), per
the provenance convention documented in the T6 record.

### 3. Review findings

```text
finding 1: T5's record (finding 3) anticipated that "the first child-bearing
       materializer (T7) wires the native side of the §74 status-detail
       convention." The tranche registry for T7 does not list §74, and the
       registry is authoritative over prose. Native status-detail cells are
       therefore NOT wired in T7; row/column materializers are declared with
       status_detail = "none" (matching actual native behavior) and the
       child-ref detail cells land with §47 recovery in T12. Recorded as a
       correction of the T5 forward expectation, not a gap against the gate.

finding 2: the first SHARED_PATH test draft expected a bridge_hint_hits on
       the stable subtree boundary, but after a Direct-decoded previous root
       the descendants carry no JS hints (§94 cold-sidecar gap). The correct
       resolution is one ceiling-gated NodeId->NativeRef promotion at the
       stable boundary - which is exactly what the implementation does and
       what the test now asserts (promotion_attempts +1, promotion_hits +1,
       hint_hits +0). This also exercises the §94 cold-fallback sidecar-gap
       requirement early.

finding 3: the generated axis lowering initially imported only each
       materializer's rust_builder; family builders (arity 1..=4) were
       emitted but not imported. Correction: the builder import set now
       unions every fixed_arity_axis family member.

finding 4: the generated MaterializeTx interface ({symbols, runtime}) was
       too narrow once axis lowerings recurse into ensureNative, which
       requires full transaction state. Correction: when any axis
       materializer exists, the generated module imports and re-exports the
       runtime's MaterializeTx type instead of declaring its own; the T5
       spacer conformance test was updated to construct the real transaction.

finding 5: an initial §23 test probe released the node's only lease before
       consulting, so the semantic View had expired and the consult
       correctly missed. The probe now holds the transaction lease while
       verifying cache-first behavior - documenting that expiry-after-
       release remains the intended lifetime model (§16), not a bug.
```

### 4. Implementation summary

What now exists:

```text
tools/tui-abi/view_abi.toml
    [[materializer]] blocks for row and column over the EXISTING
    view_row_create_0..4 / view_column_create_0..4 family (§25 reuse before
    adding); no new ABI functions (function_count stays 49)
tools/tui-abi-gen
    MaterializerFixedArityAxisSpec (validated shape: axis kinds only,
    family 1..=8 declared ViewRefResult builders with matching lifetime
    policy, exactly node-id pair + gap fields); TypeScript renderer emits
    monomorphic per-kind dispatchers switching on children.length, lowering
    layoutTrackWord + ensureNative(child) per slot (children-first, §22),
    throwing RetainedFastFallbackError beyond the specialization arity
    (§32/§49); 7 new validation tests (24 total)
crates/iyon-native/src/tui/view_abi.rs
    §23 semantic-cache-first consult at the top of view_spacer_create,
    create_small_axis (row/column arity family), and view_axis_create_buffer:
    a live NodeId returns its ref before payload/child inspection (+ unit
    test including stale-child recovery through the consult)
packages/iyon-runtime/src/tui/retained_dag.ts
    row/column/spacer dispatcher registration; children-visited counting;
    expected-native-status-to-RetainedFastFallbackError conversion
tests/perf12_t7_materialize.test.ts (six conformance tests)
bench/perf12_t7_shared_path.ts + PERF-12-t7-shared-path.jsonl
addon rebuilt and restaged for the new schema hash:
    e36d356ded172088d1631ff2301d2d205a031f0841f37bfdc04d338636fc0c94
```

Deliberately NOT done yet: production routing unchanged until T13; variable
ref-buffer lanes are T8 (column arity >4 falls back cleanly today);
derivation hints are T9; text/style/Diff payloads are T11; container/clamp/
hanging/grid/decorated/component have no constructors and route to fallback
(§76 allows explicit fallback routing; their materializers or retained ops
land with T8/T10/T11/T13); native status-detail cells are T12 (finding 1).

### 5. Provenance block

```text
source revision at capture: 50549dc (tree identical to committed 36a1856
                            except the artifact file; see Commits note)
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      e36d356ded172088d1631ff2301d2d205a031f0841f37bfdc04d338636fc0c94
schema BLAKE3:              ac76addefd7312010e808174c6d163abfeadd798561f55f67e731e202ac20740
generator BLAKE3:           2d8ad3919e8133be4109ee23dc629f20fd29abbe708113532f25015bb77a5881
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Fixed-size kinds materialize through monomorphic FFI:
   perf12_t7_materialize.test.ts "§32: fixed arities 0..=4 materialize and
   render like the Direct oracle": rows and columns at arities 0..=4,
   installed through RetainedRootBoundary.install and rendered via
   hostRenderRef, produce screen output identical to the Direct-decode
   oracle across both axis orientations. Generated dispatchers contain a
   switch per kind - no reflection, no per-node closures, no fresh
   TypedArrays (§66). PASS.

2. Stable child cuts off before payload access:
   SHARED_PATH test: previous root = changed branch + stable 200-node
   subtree rendered via Direct and adopted; next generation rebuilds only
   the branch. Counters across install(): direct_materializer_calls +2
   (new root column + new leaf only), node_id_ref_promotion_attempts/hits
   +1/+1 (one identity resolution at the stable boundary through the
   ceiling-gated NodeId promotion, since Direct decode seeded no JS hints -
   §94 shape), bridge_children_visited +2 (the new root's two layout slots,
   never S's descendants), host_mutations +1. Render parity with a fresh
   Direct decode asserted. PASS.

3. One representative SHARED_PATH retained case beats or ties direct_7v2
   total time:
   PERF-12-t7-shared-path.jsonl (smoke profile, timing build, fresh
   process, alternating-arm sampling, 60 warmup blocks x 20 ops, CI
   half-width < 3% met at 80 blocks per arm):
     direct_7v2        median 189,429 ns/op   p95 198,402 ns
     retained_dag_ffi  median 186,173 ns/op   p95 196,679 ns
     ratio retained/direct = 0.9828 -> BEATS.
   Both arms pay identical JS construction (~3 fresh nodes) and identical
   host repaint of the 201-node tree; the differential is the changed-
   frontier transport (three monomorphic FFI constructors + identity cutoff
   vs N-API property walk). Total wall time decides per §90/§119. PASS.

Section coverage evidence:
   §22  children-first: generated dispatchers evaluate ensureNative per
        child slot left-to-right before the parent constructor call;
        dedupe test proves a shared child referenced from two parents is
        materialized exactly once per transaction (4 distinct nodes ->
        4 materializer calls despite 6 edge references).
   §23  Rust unit test constructor_consults_semantic_cache_before_building_
        perf12_s23 plus the JS stale-child probe: re-requesting a live
        NodeId through viewColumnCreate2 with two stale child refs returns
        the cached ref without resolving them.
   §32  arity sweep test; >4 falls back cleanly with the old root intact
        (ref-buffer lane deferred to T8).
   §75  budgets exercised: nested-column tree exceeding
        MAX_RETAINED_NEW_NODES falls back via the T6 guards; lease audit
        shows temporary leases fully drained and the old root still
        leased/rendering.
   §50  same test; budget constants unchanged (512 new nodes / depth 256).
   §51  no-full-tree-diff is structural: resolution touches only hinted,
        promoted, or newly constructed nodes - the SHARED_PATH counters
        show zero inspection of the 200-node stable subtree's payload.

Regression battery: perf12_t7_materialize 6/6; view_materialize (T5 slice)
and perf12_t6_identity suites green; tests directory 78 pass / 1 fail (the
T2-documented pre-existing cross-file interference, unchanged); tsc clean;
tui-abi-gen check byte-fresh; tui-abi-gen 24/24; cargo iyon-native 31 lib
tests green (+§23 test); cargo fmt clean; clippy warning profile identical
to pre-T7 baseline (sorted counts diff empty).
```

### 7. Status line

**Tranche T7 status: COMPLETE.** Spacer plus row/column arities 0..=4 now
materialize through monomorphic generated FFI with children-first ordering,
native cache-first construction, working-budget fallbacks, and a SHARED_PATH
gate win (0.9828x of direct_7v2 total time); broader generation continues in
T8 with borrowed ref-buffer lanes, then derivations (T9), wide edits (T10),
payload families (T11), recovery hardening (T12), and boundary routing (T13).

Errata (added by the post-T7/T8 in-depth review): no code or gate changes
were required for T7. The review re-ran every gate green at the current
revision and additionally confirmed that the borrowed-buffer lane added in
T8 inherits count-vs-capacity validation from the generated export layer,
so the dispatcher transport path satisfies the §68 memory-safety rule
without per-call-site checks.

## T8 implementation record

### 1. Scope statement

```text
tranche:      T8 (PERF-12.5 variable-arity lanes)
parent:       12.5
sections:     §29 borrowed TypedArray transport, §30 scratch tier policy,
              §31 no-mapped-scratch rule, §116 buffer lifetime tests,
              §50 oversize -> fallback routing
supporting:   §63/§64 generator model + validation extension,
              §66/67 generated/native ownership split, §90 transport
              preparation visibility (ref_words_written)
```

### 2. Commits

```text
cb4843a  feat(tui): lower variable retained children through borrowed FFI buffers
```

This record was appended in the immediately following documentation commit.

### 3. Review findings

```text
finding 1: the first dispatcher emission duplicated the retained-cap check
       in generated code AND in MaterializeTx.axisRefScratch; the duplicate
       fired first and the §91 cold_fallbacks counter never recorded the
       refusal (caught by the oversize test). Correction: axisRefScratch is
       the single enforcement point - it refuses arities above the cap,
       counts the fallback, and sizes/returns the scratch; the generated
       code only fills and transports.
finding 2: AXIS_REF_SCRATCH was initially a WeakMap keyed by runtime
       Pointer; WeakMap keys must be objects, so every lookup threw.
       Correction: plain Map keyed by the numeric pointer - at most one
       live entry per environment.
finding 3: the emitted buffer call passed `scratch` twice (mirroring the
       raw symbol's ptr+buffer_length pair); the generated viewAxisCreateBuffer
       wrapper already duplicates the array internally, so the extra argument
       was a tsc-visible arity error. Correction: wrapper call takes eight
       arguments.
finding 4: the T7 suite's "arity >4 falls back cleanly" test became obsolete:
       since T8 those arities ride the borrowed-buffer lane. The test was
       updated to assert the lane (7-child column = 14 ref words) rather
       than the fallback; the true oversize fallback moved to the T8 suite
       where the §30/§50 cap lives.
finding 5: Grid construction has NO native constructor in the current ABI;
       the tranche gate's "Variable-axis/Grid constructors" therefore binds
       the variable-axis lane concretely, and new-Grid materialization
       arrives with the §36 Grid work (T10) riding this same transport lane.
       Recorded explicitly so the gate wording is not read as a gap.
```

### 4. Implementation summary

What now exists:

```text
tools/tui-abi-gen
    MaterializerFixedArityAxisSpec.buffer_builder: validated optional
        borrowed-buffer lane (declared function, ViewRefResult, matching
        owner-thread/call-borrow lifetime policy, not duplicating family
        builders); renderer emits the default-arm scratch loop + one
        viewAxisCreateBuffer call per kind (axis_kind literal from the
        bridge kind); MAX_DIRECT_AXIS_REFS imported into the generated
        module; 3 new validation tests (27 total)
packages/iyon-runtime/src/tui/retained_dag.ts
    MaterializeTx.axisRefScratch: §30 small-tier reusable scratch - one
        environment-level Uint32Array sized exactly for the retained cap,
        allocated once per runtime, reused across transactions; refusal
        above MAX_DIRECT_AXIS_REFS counted via cold_fallbacks
    MaterializeTx.noteRefWords: feeds ref_words_written (§90 - transport
        preparation is counted, never hidden)
native_view_policy.ts: MAX_DIRECT_AXIS_REFS = 1_024 children (initial
        §50 candidate; final values from realistic traces at T15)
tests/perf12_t8_buffers.test.ts: six §116 lifetime tests
addon rebuilt and restaged for the new schema/generator hashes:
    32d21a13f1a5c1c37a3c41a4b7e4738cda41c8f6ee3fbdd83a7c2333fe32666c
```

§31 compliance is structural: there is no pointer-export API, no external
ArrayBuffer/deallocator context, no arena state machine, and no persistent
native scratch anywhere in the runtime or JS layer - the scratch is a plain
JS-owned Uint32Array whose storage native reads only during the synchronous
call (`resolve_axis_children` collects owned child Views before returning).

Deliberately NOT done yet: medium/byte scratch tiers arrive with T11 byte
payloads (the small tier covers the entire axis-ref surface);
new-Grid materialization arrives with §36/T10 on this lane (finding 5);
production routing unchanged until T13; sanitizer builds skipped (no ASan/
UBSan harness wired on the darwin host - noted as a stated limitation).

### 5. Provenance block

```text
source revision at capture: d733020 (tree identical to committed cb4843a
                            after snap.new cleanup; see Commits note)
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      32d21a13f1a5c1c37a3c41a4b7e4738cda41c8f6ee3fbdd83a7c2333fe32666c
schema BLAKE3:              fd2399c70ce82d2b29ee40a4f69864e452568325cb1d83360f72a8b4248ed73d
generator BLAKE3:           e6237f38757724691b7b739064c158573fc0f1dcd63ab16d537a85039e8d155a
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Variable-axis constructors use reusable synchronous buffer/buffer_length:
   perf12_t7_materialize "arity beyond the fixed-arity family uses the T8
   borrowed-buffer lane": a 7-child column installs through the default arm
   with ref_words_written = 14 (7 track words + 7 child refs). The variable
   suite exercises arities 5/6/17/100 with mixed layout-child kinds
   (normal/fixed/flex/contentMax) against Direct-decode render parity.
   Grid: no native constructor exists; lands with §36/T10 on this lane
   (finding 5). PASS for the axis surface; Grid documented.

2. Native never retains a pointer after return:
   structural inspection (resolve_axis_children collects owned Views inside
   the call) plus two behavioral tests: alternating boundaries reuse the
   single environment scratch across six installs of distinct trees with
   correct renders each time, and fully rewritten shared storage backs two
   successive raw buffer calls yielding distinct correct constructions.
   PASS.

3. Zero-length/max-length/oversize cases pass:
   zero-length raw buffer call mints a valid empty axis that participates
   in the semantic cache; exactly-at-cap transport moves 2,048 ref words
   through ONE call with only the root node materialized (children resolved
   by identity); one-over-the-cap refuses before any FFI - cold_fallbacks
   increments by exactly 1, nothing publishes, old root stays leased and
   rendering. PASS.

4. No external ArrayBuffer machinery exists:
   inspection - no pointer export API, no deallocator context, no external
   memory registration anywhere in iyon-native or the TS layer; the scratch
   is a JS-owned Uint32Array. Wrong-typed input (non-TypedArray) is
   rejected by bun:ffi before any native code runs. PASS.

Section coverage evidence:
   §29  borrowed transport as above; pairs laid out as (track_word,
        child_ref) matching AxisChildInputV1 (8 bytes, align 4).
   §30  small tier allocated once, sized for the retained cap, reused
        across transactions/boundaries; above-cap refusal routes the
        complete cold path. Medium/byte tiers deferred to T11 payloads.
   §31  structurally absent machinery (above).
   §116 six dedicated tests (zero/max/oversize/reuse/wrong-type/no-
        retention).
   §50  MAX_DIRECT_AXIS_REFS = 1,024 enforced at the single refusal point
        with fallback accounting; budgets unchanged otherwise.

Regression battery: tests directory 84 pass / 1 fail (the T2-documented
pre-existing cross-file interference, unchanged); T7/T6/T5 suites green;
tsc clean; tui-abi-gen check byte-fresh; tui-abi-gen 27/27; cargo
iyon-native 31 lib tests green; cargo fmt clean; clippy warning profile
unchanged from pre-T8 baseline.
```

### 7. Status line

**Tranche T8 status: COMPLETE.** Variable-axis children now transport
through the reusable synchronous borrowed-buffer lane with visible ref-word
accounting, single-point cap enforcement, and proven buffer lifetimes
(zero/max/oversize/no-retention), with no mapped-scratch machinery anywhere;
Grid construction and medium/byte tiers follow in T10/T11, production
routing in T13.

Errata (added by the post-T7/T8 in-depth review):

```text
finding R1: AXIS_REF_SCRATCH was an unbounded Map keyed by runtime pointer;
       every environment reset leaked one 8 KiB entry (reachable only via
       test resets today, but still a leak). Correction: single-slot
       storage holding the latest runtime's scratch - at most one live
       NativeViewRuntime exists per environment, so nothing accumulates.
finding R2: the review verified where count-vs-capacity validation for
       view_axis_create_buffer actually lives: the generated export layer
       (generated_buffer_used) rejects used_child_count entries that do
       not fit inside children_capacity_bytes before the implementation
       dereferences anything - satisfying the §68 rule that timing
       builds still validate memory-safety requirements. A dedicated Rust
       unit test (axis_buffer_rejects_count_larger_than_buffer_bytes_
       perf12_t8) now proves the rejection and that a matching count on
       the same buffer shape still validates, strengthening the §116
       gate evidence recorded above.
```

## T9 implementation record

### 1. Scope statement

```text
tranche:      T9 (PERF-12.6 retained clone/edit lanes)
parent:       12.6
sections:     §27 derivation hints, §28 why derivations are kept,
              §38 text layout mutation
supporting:   §19 ensureNative ordering (tryDerivation step),
              §23 native cache-first rule extended to patch impls,
              §25 reuse-before-adding (no new ABI functions),
              §91 derivation_fast_path_calls counter
```

### 2. Commits

```text
ecbe221  feat(tui): preserve semantic derivations and retained scalar clones
```

This record was appended in the immediately following documentation commit on
`perf-refactor`. The committed benchmark artifact's summary `git_sha` records
`ecbe221` (the artifact was re-captured after the implementation commit; tree
content identical except the artifact file), per the provenance convention
documented in the T6 record.

### 3. Review findings

```text
finding 1: the legacy view_common_patch_root surface resolved decoration_ref
       unconditionally and the GENERATED EXPORT LAYER validated it as a
       non-zero ViewRef, so the function could never be called with the
       absent-marker 0 - the dead 11v3 route dodged this by passing the base
       ref twice. Correction: decoration_ref is consumed by no mask branch,
       so the canonical schema now lowers it as plain u32; the impl resolves
       it only when non-zero. Schema BLAKE3 changed legitimately (below).

finding 2: both patch implementations lacked the §23 semantic-cache-first
       consult that T7 added to constructors. Correction: a live NodeId now
       returns its cached ref before the base is resolved or scalars are
       inspected, on both view_text_layout_patch_root and
       view_common_patch_root (+ Rust unit tests covering the consult and
       the stale-base error path).

finding 3: the first benchmark draft derived every generation from the same
       ORIGINAL text node and fell back after one generation. Root cause is
       a native lifetime fact, now documented: modifier operations allocate
       a new root Arc<ViewNode> (persistent-value semantics), so a node's
       weak cache entry dies once the tree holding its exact handle is
       released. Correction: generations chain - each generation derives
       from the PREVIOUS generation's text node, whose handle lives inside
       the still-leased previous root. This is exactly the §18 guarantee
       the architecture pairs with §27/§38; deriving from a dormant
       original degrades cleanly to full materialization instead.

finding 4: scalar-only decorations over INLINE-kind bases (Spacer) have the
       same exposure amplified: the padded node's construction already drops
       the inner spacer's Arc even while the tree lives, so such bases are
       unresolvable after ANY decode. Tests therefore use Text bases (Arc'd
       inside ViewKind::Text and held by the parent column), which is also
       the realistic agent-TUI shape. Spacer-based scalar derivations remain
       correct - they simply fall back when the base is not resolvable,
       which §27 permits ("otherwise it ignores the hint").

finding 5: the first test suite draft asserted direct_materializer_calls +1
       for the wrap-only generation but each `View.vertical([...])` builds a
       FRESH spacer leaf (new identity), so the true delta was 2. The counter
       semantics were right; the expectation was fixed.
```

### 4. Implementation summary

What now exists:

```text
src/tui/ir.ts
    BridgeTextLayoutDerivation / BridgeCommonScalarDerivation /
    BridgeDerivation types, BRIDGE_DERIVATION WeakMap sidecar, set/peek
    accessors (§15: hints die with their semantic node)
values/view.ts
    textLayoutPatch attaches TextLayoutDerivation {base, wrap, align} to
        the derived text node in BOTH cases (plain text; decorated wrapper
        with patched inner text) - final codes recorded, spans array
        identity preserved by the spread (§38 construction contract)
    decorate() attaches CommonScalarDerivation when the merged decoration is
        scalar-only (padding/width/height/min/max, empty style/colors/
        border/styleStates, mask != 0); mask bits mirror the native
        PATCH_* constants; padding packs as top|right<<16 /
        bottom|left<<16 exactly as the native impl unpacks
retained_dag.ts
    tryDerivation(node, tx): runs after identity resolution and before
        materializer dispatch (§19 hard ordering); resolves the base's
        same-generation NativeRef from its hint or, for pre-commit NodeIds,
        one ceiling-gated promotion whose acquired lease joins the tx
        temporaries; calls the EXISTING generated wrappers
        viewTextLayoutPatchRoot / viewCommonPatchRoot (§25 - zero new
        ABI functions, function_count stays 49); any expected native
        failure status ignores the hint (which stays attached for later
        re-adoption) and materializes from semantic fields (§27/§38);
        success counted as derivation_fast_path_calls (§91)
crates/iyon-native/src/tui/view_abi.rs
    §23 consult on both patch impls; decoration_ref resolution only when
    non-zero; two new unit tests
tools/tui-abi/view_abi.toml
    decoration_ref lowered as u32 (schema change; regenerated)
tests/perf12_t9_derivation.test.ts   five conformance tests
bench/perf12_t9_text_metadata_patch.ts + PERF-12-t9-text-metadata-patch.jsonl
addon rebuilt and restaged for the new schema hash:
    d48b8acbe92472b3d22727bc9018467b0ebcb626cafd6fdaf365ed6f76ec63b6
```

Deliberately NOT done yet: AxisEditDerivation/GridEditDerivation land with
their native retained edit primitives in T10 (§33-§36); production
routing unchanged until T13; decorated-node materializers are T11/T13, so a
scalar derivation under a STILL-decorated parent falls back until then.

### 5. Provenance block

```text
source revision at capture: ecbe2215adf9468c3dd76a4321bda339eaa74515
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      d48b8acbe92472b3d22727bc9018467b0ebcb626cafd6fdaf365ed6f76ec63b6
schema BLAKE3:              ec82466e117642ffc4009bd11199b7a24aa37f3476065fa34e8732d070dda2d4
generator BLAKE3:           e6237f38757724691b7b739064c158573fc0f1dcd63ab16d537a85039e8d155a
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Wrap/align-only text change sends base NativeRef + NodeId + scalars,
   never resends payload:
   perf12_t9_derivation.test.ts "§38: wrap-only text change clones the
   retained payload" - install(next) over a Direct-decoded previous root:
   derivation_fast_path_calls +1, byte_payload_bytes +0, cold_fallbacks +0,
   exactly one ceiling-gated base promotion (attempts/hits +1/+1, the
   §94 cold-sidecar shape), render parity with the Direct oracle.
   The align-only variant passes identically. The smoke benchmark's measured
   window proves the same structurally at scale: 1,600 consecutive retained
   installs rode the derivation lane with fallbacks=0 and
   byte_payload_bytes=0 (every violation aborts the run). PASS.

2. Common scalar patch reuses base ref:
   perf12_t9_derivation.test.ts "§27/§28: scalar-only decoration
   patch reuses the shared base ref" - an unmodified shared text base inside
   the leased root is patched through view_common_patch_root across TWO
   consecutive generations (padding scalars, then a width rule), each riding
   derivation_fast_path_calls +1 with byte_payload_bytes +0 and Direct-oracle
   render parity; generation 2 resolves the base from its hint (no extra
   promotion). PASS.

3. Hint-miss degrades cleanly to full materialization:
   three independent proofs -
     a) mixed decorations stay unhinted (style-bearing decoration attaches
        nothing) -> install returns undefined, every temporary lease drains
        (leased_slots back to the old root only), old root keeps rendering;
     b) raw-symbol proof: stale/unresolvable base refs return error statuses
        (never crash) for BOTH patch primitives, and tryDerivation converts
        expected statuses into "ignore hint";
     c) dormant-base fallback: a derivation whose base weak entry expired
        misses the ceiling-gated promotion and falls through to normal
        materialization/fallback routing (finding 3 scenario).
   PASS.

§91 counter coverage: derivation_fast_path_calls incremented only on
successful fast paths; misses are visible through materializer/fallback
behavior rather than a new counter (the §91 list has no miss cell for
derivations; adding one would extend the agreed surface mid-tranche).

Regression battery: perf12_t9_derivation 5/5; tests directory 89 pass /
1 fail (the T2-documented pre-existing cross-file interference, unchanged);
test+tests 138 pass / 1 fail; tsc clean; tui-abi-gen check byte-fresh;
tui-abi-gen 27/27; cargo iyon-native 34 lib tests green (+2 new); cargo fmt
clean; clippy warning profile identical to the pre-T9 baseline.
```

### 7. Status line

**Tranche T9 status: COMPLETE.** Derivation hints now let wrap/align-only
text changes and scalar-only decoration changes clone retained native state
through the existing patch primitives (base ref + NodeId + scalars, zero
payload bytes, SHARED_PATH-style gate win at 0.9821x of direct_7v2 total
time on TEXT_METADATA_PATCH) with clean degradation on every hint-miss path;
axis/grid edit derivations land with their native primitives in T10, payload
families in T11, recovery hardening in T12, and boundary routing in T13.

## T10 implementation record

### 1. Scope statement

```text
tranche:      T10 (PERF-12.7 wide retained edits)
parent:       12.7
sections:     §33 PersistentSeq preservation, §34 wide sidecar exception,
              §35 wide native edit path, §36 Grid, §96 wide benchmark gate
supporting:   §28 AxisEdit/GridEdit derivation hints, §29/§30 borrowed
              transport and scratch policy, §50 retained caps, §91 structural
              counters, §23 native cache-first rule
```

### 2. Commits

```text
b63a64c  perf(tui): preserve logarithmic wide edits in retained DAG FFI
```

This record is appended in the immediately following documentation commit on
`perf-refactor`. The §96 artifact summary records `git_sha = 6e4787f` because
its clean-tree capture ran immediately before the T10 implementation commit;
the benchmark source and all implementation files were then committed in
`b63a64c`, with no semantic drift between capture and commit. This is the
same explicit pre-amend/pre-commit provenance convention used by T6–T9.

### 3. Review findings

```text
finding 1: a first design draft converted a flat base axis to PersistentSeq
       inside the edit constructor. That would make the first 100k edit
       O(width), violating §33 and the user's wide-performance requirement.
       Correction: row/column construction above the 1,024-wide threshold
       seeds a PersistentSeq sidecar once; every subsequent set/insert/remove/
       splice is path-copying only. Derived nodes carry a lazy frozen `children`
       accessor; the retained path never asks for the flat array.

finding 2: the first Grid cell implementation copied the addressed row's
       complete cell array. For a 100k-cell row this silently regrew a flat
       wide edit. Correction: wide Grids seed a BRIDGE_GRID_SEQUENCE sidecar
       with PersistentSeq cells plus row offsets/tracks; gridSetCell performs
       one logarithmic set and uses a lazy rows accessor. Narrow Grids retain
       the eager semantic shape. The wide-grid conformance test proves the
       sidecar path has bounded sequence counters and zero retained child
       traversal.

finding 3: T8 documented that no native Grid constructor existed. The T10
       §36 scope requires new-Grid construction on the same borrowed lane, so
       a canonical `view_grid_create_buffer` function was added rather than
       deferring Grid again. It uses one bounded u32 buffer with explicit
       track/row/cell framing, exact-consumption validation, packed spans and
       alignments, and owned native Grid construction; no persistent buffer
       pointer survives the call.

finding 4: the generated ABI manifest/function count changed from 49 to 50,
       but the handwritten bootstrap pointer map initially omitted the new
       `viewGridCreateBuffer` pointer. Correction: the same-image bootstrap
       map, NativeAbiPointers surface, function-name list, generated outputs,
       conformance fixture, and pinned function-count assertion now all agree.

finding 5: inserting the new ABI function shifted the generated conformance
       stub return values, while two fixed expected values in the generator's
       Rust fixture remained at their T9 values. Correction: the canonical
       Rust renderer now emits the shifted expectations (127 and 0x11c), the
       snapshot was refreshed, and `tui-abi-gen check` is byte-fresh.

finding 6: the native Grid parser initially narrowed packed track amounts to
       u16 without rejecting high bits. Correction: raw amounts above u16::MAX
       are rejected before construction, and zero cell spans/invalid alignment
       codes are rejected as well; generated buffer length/count checks remain
       active before the unsafe implementation.
```

### 4. Implementation summary

What now exists:

```text
packages/iyon-runtime/src/tui/persistent_seq.ts
    §91 counters for nodes_cloned, branches_cloned, items_iterated; mutation
    paths instrumented without scans/allocations outside their existing
    path-copy work

packages/iyon-runtime/src/tui/ir.ts
    AxisSet/AxisSplice/GridCell derivation hints; BridgeSequenceOverride and
    BridgeGridSequenceOverride WeakMap sidecars; PersistentSeq remains the
    authoritative wide semantic storage

packages/iyon-runtime/src/tui/values/view.ts
    axisSetChildForTransport, axisSpliceForTransport, and
    gridSetCellForTransport; wide axis/grid initial seeding above 1,024;
    frozen lazy accessors preserve exact BridgeViewNode shape and only flatten
    on Direct/fallback access; normal narrow View construction is unchanged

packages/iyon-runtime/src/tui/retained_dag.ts
    axis set/splice and Grid cell derivations resolve only replacement or
    inserted child nodes, then call existing native retained primitives;
    viewAxisSetChild/viewAxisSpliceBuffer/viewGridSetCell carry no old child
    list; reusable bounded grid word scratch; new Grid materializer uses one
    generated borrowed u32 buffer and native exact parser; §91 counters remain
    visible

canonical ABI/native
    view_grid_create_buffer added to view_abi.toml and all generated Rust/C/
    TypeScript/manifest/conformance outputs; NativeViewRuntime parses packed
    Grid tracks, row metadata, cell refs/spans/alignments and publishes via
    the shared semantic cache; §23 cache-first consults cover Grid creation,
    axis set/splice, and Grid cell edits

tests/perf12_t10_wide.test.ts
    seven tests covering axis replacement, insert/remove/splice parity,
    2k/10k/100k sequence counters, new Grid construction, wide Grid cell
    replacement, larger Grid construction, and over-cap fallback
bench/perf12_t10_wide_edits.ts + PERF-12-t10-wide-edits.jsonl
    §96 smoke profile, clean isolated arm runs, retained/direct total timing,
    structural counter records, full provenance
```

Deliberately NOT done yet: production boundary routing remains T13; payload
families remain T11; Grid/axis edits use this T10 native lane but do not add
an application-specific production API — the `ForTransport` constructors are
private retained-candidate machinery, matching the existing T9 transport
constructor pattern.

### 5. Provenance block

```text
source revision at capture: b63a64c3be04701273ef4e79beb852aff56b0846
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      c47c40f39493b620478fd5ace608fff08a3479f4c71143fd6bce547af075818e
schema BLAKE3:              7c7f9480cf8950965436de870da6d9a135bc346bd1e78aa74cb702874f0cf498
generator BLAKE3:           5fa933f670b4b38bdf04e8e5b6635342d3a75e6781cceb14283f73c575d4ed4a
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Replace/insert/remove/splice remain O(log₃₂ N) at widths 2k/10k/100k:
   perf12_t10_wide.test.ts performs set at all three widths. Per one edit:
     width 2,000:   nodes_cloned=3, branches_cloned=2, items_iterated=32
     width 10,000:  nodes_cloned=3, branches_cloned=2, items_iterated=32
     width 100,000: nodes_cloned=4, branches_cloned=3, items_iterated=32
   generous assertions are <=10 nodes, <=8 branches, <=64 items at every
   width. A lazy-access assertion proves no sequence items are iterated while
   constructing the retained node; flat children are only produced when
   explicitly accessed. Insert/remove/splice render-parity tests use width
   2,000 and all pass.

2. No flat materialization on the retained one-edit path:
   all retained wide cases report bridge_children_visited=0. Final §96
   retained records (300 measured operations per case) report:
     axis_set@2k:     derivations=300, materializers=300, words=0,
                      seq nodes=900, items=9,600
     axis_set@10k:    derivations=300, materializers=300, words=0,
                      seq nodes=900, items=9,600
     axis_set@100k:   derivations=300, materializers=300, words=0,
                      seq nodes=1,200, items=9,600
     axis_insert@2k:  derivations=300, materializers=300, words=600
     axis_remove@2k:  derivations=300, materializers=0, words=0
     splice4@2k:      derivations=300, materializers=1,200, words=2,400
   The only materializers are newly inserted/replacement children; no old
   sequence is inspected or transported. PASS.

3. Grid §36:
   new Grid construction: 2-cell and 400-cell grids pass render/installation
   tests through one borrowed word buffer; a 22,000-cell grid exceeds the
   65,536-word retained scratch cap and cleanly falls back while the old root
   remains installed. Retained wide Grid cell replacement at 2,000 cells
   clones bounded sequence work, reports derivation +1, direct child
   materializer +1, bridge_children_visited +0, and matches Direct output.
   Native Grid parsing tests cover track kinds, gaps, spans, alignments,
   malformed/truncated buffers, and §23 cache-first re-request. PASS.

4. §96 smoke timing (total operation time, including host commit; profile
   smoke, 20 warmup rounds + 30 measured rounds, 10 operations/block):
     mode             width       retained/direct median ratio
     axis_set         2,000       0.5443
     axis_set        10,000       0.5575
     axis_set       100,000       0.5337
     axis_insert      2,000       0.4999
     axis_remove      2,000       0.4928
     axis_splice4     2,000       0.0411
   Retained is faster in every measured case. The 100k totals include the
   unavoidable host repaint/layout of a 100k-node view; the structural
   counters separately prove the edit itself remains logarithmic and sends
   no old child sequence.

§23 native cache-first tests: 2 new Rust tests cover Grid construction and
axis/Grid edit re-requests with stale base/child arguments; all return the
live NodeId ref before consuming payload/children.

Regression battery: perf12_t10_wide 7/7; full runtime test run 145 pass /
1 fail (the T2-documented pre-existing cross-file interference in the weak-
cache expiry test); tsc clean; tui-abi-gen check fresh and 27/27; cargo
iyon-native 36 lib tests plus all generated suites green; cargo fmt clean;
clippy clean.
```

### 7. Status line

**Tranche T10 status: COMPLETE.** Wide row/column replacement and splice
operations now preserve PersistentSeq logarithmic work, transport only new
child refs through retained native edits, and keep flat semantic children
lazy; Grid creation uses the bounded borrowed buffer lane and Grid cell edits
use the retained native PersistentSeq path. §96 smoke timing beats direct_7v2
at every tested width and operation; payload families remain T11, recovery
hardening T12, and production boundary routing T13.

Errata (added by the post-Tranche-10 implementation review):

```text
finding R1: `wrapFrozenBridgeNode` bypassed the View constructor and therefore
       omitted the public frozen View `kind` field on wide axis/Grid derived
       wrappers. Correction in b36e493 (`fix(tui): correct T10 retained wide
       edit semantics`): the wrapper installs the same enumerable `kind:
       "view"` own property before freezing; T10 tests now assert it.

finding R2: Grid retained edits initially treated the source-row cell array
       index as the native column coordinate. That diverges for column/row
       spans and could target no cell or the wrong cell. Correction: the JS
       wide sidecar now records the native placement map while seeding its
       PersistentSeq; narrow edits compute the same placement map on demand.
       A span-aware render-parity test covers the corrected coordinate rule.

finding R3: the packed Grid lane silently discarded marker-track payload bits
       and the JS encoder could truncate out-of-range track amounts through
       bitwise coercion. Correction: the JS lane rejects non-u16 amounts via
       the retained fallback, and native parsing rejects nonzero amounts on
       content/flex marker words before construction; the native conformance
       test covers malformed marker words.

finding R4: the original T10 smoke artifact omitted §96 replacement widths 32
       and 256 and did not include p99/median bootstrap-CI fields. The smoke
       harness and committed `PERF-12-t10-wide-edits.jsonl` were refreshed at
       b36e493 with replacement widths 32/256/2,000/10,000/100,000, p99 and
       median_ci95_ns, and the existing 2,000-width insert/remove/splice
       records. The additional structural test now proves set/insert/remove/
       splice bounds at 2,000/10,000/100,000. Refreshed artifact provenance:
       Bun 1.4.0 revision 34cbb9a40, rustc 1.97.1 aarch64-apple-darwin,
       native SHA-256 6caffbd5772ca43aacd88519b0162afc1ce22aea49a4664628a83b762743046f.
```

The review corrections preserve the T10 status; no T11+ scope was pulled
forward.

## T12 implementation record

### 1. Scope statement

```text
tranche:      T12 (PERF-12.9 transaction integrity)
parent:       12.9
sections:     §43 multi-branch DAG materialization, §44 temporary lease
              transaction, §45 host atomicity, §46 stale hints, §47 targeted
              one-retry recovery, §73 authoritative recovery helper,
              §118 failure injection suite
```

### 2. Commits

```text
15ae34c  perf(tui): harden multi-branch materialization and stale-ref recovery
```

The generated ABI reference, schema outputs, native addon-facing status
surface, retained transaction implementation, smoke harness, and T12 tests
are all included in this commit. The raw smoke artifact was captured after
this commit and is included in the following documentation commit.

### 3. Review findings

```text
finding 1: T6/T7 already deduplicated identical BridgeViewNode objects in a
       transaction-local Map, but there was no T12-specific proof spanning
       multiple changed branches. Correction: the T12 suite constructs a
       shared child referenced by both branches and asserts exactly two
       materializer calls (one parent + one child), with render parity.

finding 2: the native ABI status cell already had an unused detail word, but
       generated TypeScript discarded it by throwing an untyped Error. That
       made §47 unable to identify a stale child without probing every ref.
       Correction: the canonical ABI now emits `view_status_detail`; native
       constructors/edit primitives record child ordinals or a base-ref kind;
       generated wrappers throw NativeAbiStatusError carrying status/detail.

finding 3: retained materializer failures were previously converted directly
       to FastFallback, so a stale child hint could not receive the required
       one targeted retry. Correction: ensureNative maps the native detail to
       the corresponding semantic child, invalidates only that hint, uses the
       retained NodeId path or §73 Direct recovery helper, and retries the
       parent once. A transaction counter prevents a second retry.

finding 4: the existing exact-root recovery acquired a NodeId lease but did
       not release that temporary acquisition after the host retry. Correction:
       the recovery path now releases exactly that extra lease in a finally
       block; the boundary's durable root lease remains untouched.

finding 5: releaseAllExcept previously removed every equal ref from the temp
       list. Correction: it transfers exactly one lease occurrence and batch-
       releases the rest, preserving correct accounting if a future path
       acquires the same ref more than once.

finding 6: no persistent transaction record, second semantic cache, or
       borrowed-buffer retention was introduced. The exceptional §73 helper
       decodes synchronously, publishes through the shared publication funnel,
       returns one lease, and retains no napi_value.
```

### 4. Implementation summary

What now exists:

```text
§43/§44  MaterializeTx.refs deduplicates shared semantic nodes; temporary
         NativeRef leases remain private to one transaction and transfer only
         the completed root to RetainedRootBoundary. All failure paths use one
         batch release; root transfer removes exactly one lease occurrence.
§45     RetainedRootBoundary installs/render-commits only after complete root
         materialization. Host failure leaves previousRef and host state in
         place; temporary and newly-acquired boundary leases are drained.
§46/47  generated NativeAbiStatusError exposes FAST_CACHE_MISS detail;
         constructors and edits identify stale child ordinals/base refs;
         ensureNative and derivation edits perform one targeted retry, then
         return the complete fallback without looping.
§73     `tuiViewAbiDecodeRef` is an exceptional synchronous N-API helper that
         decodes one BridgeViewNode, publishes with the shared semantic cache,
         and returns a leased NativeRef. It is used only when a stale node has
         no retained materializer (the T12 dormant text-child test).
§74     `view_status_detail` is generated through the canonical ABI and reads
         the runtime's status detail side channel only on an error path.
§118   T12 transaction suite covers shared-branch dedupe, child/base stale
         recovery, dormant Direct recovery, unsupported-child failure, a
         second-stale-child bounded retry, host failure, old-host preservation,
         and temporary lease invariants. Native unit coverage proves malformed
         parent publication returns child detail before publishing a root.
```

Deliberately NOT done yet: production boundary routing remains T13; T11
payload-family materializers remain outside this tranche; no persistent mirror,
changed-closure VM, asynchronous command ring, or per-node native lease was
added. Existing T8 buffer-cap fallback and T11 payload work remain the
authoritative owners of their respective specialized failure paths.

### 5. Provenance block

```text
source revision at capture: 15ae34c6f07ac0db8529e8c2ca3d0e83912c88e1
bun --version:              1.4.0
bun --revision:             1.4.0+34cbb9a40
rustc:                      1.97.1 (8bab26f4f 2026-07-14), target aarch64-apple-darwin
rebuilt addon SHA-256:      0e8f48d641c6b38b6fdfa204b0c3da1c9f0c8e1dfb4a1d32c87e665ef6918d5b
schema BLAKE3:              3f4ebadaf333fb067cc4ffbde6266b7177216a3fa210cbd25e04992c5ae13332
generator BLAKE3:           4eb8b57027886c4f8812e667ad51e61f3d6fcbdc4dbd0e1bc935b2aae8f6b29c
macOS 26.5.2, Apple M1 Pro
```

### 6. Gate evidence

Tranche table "Required result" rows:

```text
1. Common ancestors built once across branches:
   perf12_t12_transaction.test.ts §43 passes. A View.horizontal with one
   shared spacer in both child positions installs through one transaction;
   direct_materializer_calls = 2 (one row + one spacer), bridge_children_visited
   = 2, host_mutations = 1, and the retained screen equals the Direct oracle.
   PASS.

2. Exactly one host mutation after complete materialization:
   the same §43 case reports host_mutations = 1; all other T12 successful
   installs in the smoke run report one host mutation per operation (50/50).
   The boundary commits only after ensureNative returns the complete root.
   PASS.

3. Temporary leases drain on success, error, and failure injection:
   §45/§118 child failure keeps leased_slots exactly at the old-root count,
   preserves the old screen, and subsequently accepts a valid install;
   second-stale-child failure keeps the old screen and is followed by a
   successful exact-root render; disposed-host failure leaves the lease count
   unchanged. The eight JS T12 tests pass with 33 assertions; native test
   `t12_stale_child_status_detail_precedes_parent_publication` passes and
   observes zero published slots after the failed parent. Existing §111 Rust
   lease tests remain green. PASS.

4. One bounded stale-ref retry then authoritative fallback:
   child recovery: stale_ref_retries delta = 1 and render parity PASS;
   derivation-base recovery: stale_ref_retries delta = 1 and text parity PASS;
   dormant text child: stale_ref_retries delta = 1 and §73 Direct decode
   recovery/render parity PASS; two stale children: install returns undefined,
   stale_ref_retries delta = 1, old host remains unchanged, and no second
   retry occurs. Native detail cells report child ordinal 0/1 as appropriate;
   the raw ABI test observed detail kind `0x40000000` and ordinal `0`.
   PASS.

5. Host atomicity:
   failed child materialization and disposed-host render both leave the
   previous boundary root installed; no host mutation is recorded on either
   failure. The existing Rust failed_host_install_retains_old_root test and
   the T12 JS failure cases pass. PASS.

6. Smoke-profile raw evidence (timing build, total operation time including
   host commit, 20 warmup + 50 measured operations, fresh staged addon):
   PERF-12-t12-transaction.jsonl records multi_branch_shared_child with
   median 13,979 ns, p95 47,959 ns, p99 58,083 ns, samples retained, and
   structural counters bridge_hint_hits=50, direct_materializer_calls=100,
   bridge_children_visited=100, stale_ref_retries=0, cold_fallbacks=0,
   host_mutations=50. This is integrity evidence, not an adoption decision.
```

Regression/verification evidence:

```text
T6–T10 plus T12 TypeScript suites: 42 pass / 0 fail / 732 expect calls
T12 suite alone:                    8 pass / 0 fail / 33 expect calls
iyon-native lib tests:              37 pass / 0 fail / 1 ignored
ABI generator freshness check:      pass
TypeScript typecheck:               pass
cargo fmt --check:                  pass
native addon staged:                pass; SHA recorded above
```

### 7. Status line

**Tranche T12 status: COMPLETE.** Multi-branch retained materialization now
has one transaction-local identity/lease protocol, atomic host installation,
status-directed stale-child/base recovery with one bounded retry, and an
exceptional shared-cache Direct recovery helper; §118 success and failure
paths are covered by passing JS/native tests. T13 still owns production
boundary routing and T11 still owns payload-family materializers.

Errata (post-record generated-ABI gate):

```text
finding R7: adding the generated `view_status_detail` function increased the
       canonical ABI function count from 50 to 51. The existing PERF-11
       vertical-slice assertion still expected 50 and failed despite fresh
       generated outputs. Correction in 4f9a9b8 (`test(tui): update generated
       ABI count for T12 status detail`): the assertion now pins 51; the
       generated ABI layout/conformance tests and vertical slice are green.

finding R8: the first committed T12 smoke artifact used 20 warmup/50 measured
       operations and omitted the §103 phase/CI fields. Correction in 47c26b0
       (`bench(tui): align T12 smoke output with result schema`): the harness
       now uses 50 warmup/500 measured operations, records the required
       candidate/provenance/phase fields, p99, and a bootstrap median CI. The
       raw JSONL was refreshed at that revision. The authoritative integrity
       record is now median 5,729 ns, p95 19,834 ns, p99 46,333 ns,
       median_ci95_ns [5,354, 6,209], with host_mutations=500,
       direct_materializer_calls=1,000, bridge_children_visited=1,000,
       stale_ref_retries=0, and cold_fallbacks=0.

finding R9: the initial §43 regression used a shared leaf, which proves
       transaction identity deduplication but is weaker than the handoff's
       multi-branch wording. Correction in 9896b5d (`test(tui): cover shared
       retained branch deduplication`): the test now shares a complete
       retained branch (row + leaf) between both root branches and asserts
       exactly three materializers (outer row + shared row + shared leaf).

finding R10: generated `checkedRef` queried the status-detail side channel
       for every error result, while some legacy/native invalid-result paths
       intentionally return without writing detail. A later invalid call could
       therefore observe stale child/base detail. Correction: the canonical
       TypeScript generator now reads detail only for `FAST_CACHE_MISS` and
       supplies zero for other statuses; generated outputs were refreshed and
       the addon was restaged. New generator BLAKE3 is
       362fc984b6d1270399baf0880b9c80fc3e4e939c943938952eca14ee7c12e74e,
       and the restaged addon SHA-256 is
       96cf11783d18f6e42b64977748829139df1063cf4aab623a35c970bf22d657d6.
```