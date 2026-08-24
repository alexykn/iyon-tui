# PERF-8 — journaled retained graph + structural-delta packed View transport

**Status:** proposed handoff after PERF-7v2  
**Baseline repository:** `alexykn/iyon-tui`
**Baseline branch:** `perf-refactor`  
**Baseline implementation commit:** `84a7d117c777fbd5c2f0d5d072e63769be842e7c` (`test: bench 7v2`)  
**PERF-7v2 result payload:** `packages/iyon-runtime/bench/PERF-7v2-results.jsonl`  
**Scope:** benchmark first; production adoption only after the decision gate in this document

---

## 0. Executive conclusion

PERF-7v2 established that the packed path is worth continuing, but its current representation is not the end-state I would ship.

The important result is not merely that a `Uint32Array` can beat direct N-API property traversal. It is that the system already has the two prerequisites required for a substantially better retained protocol:

```text
TypeScript:
    immutable semantic View DAG
    stable semantic identity (NodeId)

Rust:
    immutable Arc-backed View DAG
    weak retained cache
```

The remaining inefficiency is that the boundary still behaves too much like a serializer:

```text
immutable JS View DAG
    ↓
walk semantic objects again at commit time
    ↓
translate fields into a temporary packed transaction
    ↓
N-API
    ↓
parse the transaction
    ↓
reconstruct Rust containers and strings
```

The optimal next design is **not a generic old-tree/new-tree diff**.

The optimal next design is:

```text
immutable semantic construction
    ↓
construction-time canonical packed metadata
    ↓
explicit lineage + persistent structural sharing
    ↓
commit compiler emits ONLY new/changed retained objects
    ↓
flat topological packed transaction
    ↓
Rust reconstructs ONLY new/changed retained objects
    ↓
old Rust objects and persistent sequence chunks are shared
```

For a one-leaf update the target is therefore not:

```text
serialize new root
    + emit REF for every stable immediate child
```

but:

```text
new leaf
+ O(log_B N) changed persistent-sequence chunks if a wide child list changed
+ changed ancestors
+ root operation
```

where `B` is a wide branch factor, nominally 32.

For exact identity the target is even smaller:

```text
one N-API call
    arguments: generation + dense root reference
    no Uint32Array transaction
    no strings
    no parser
```

This handoff calls that design **Packed V3**.

---

# 1. Why PERF-7v2 is a valid baseline but not the endpoint

The old PERF-7 was not a fair retained-transport comparison. PERF-7v2 repaired the important architecture problems:

```text
same environment ViewBridgeCache
full 53-bit NodeId
weak JS knowledge
weak native retention
cache-miss recovery
full bridge schema
reusable Uint32Array writer
root REF packet
explicit warmup
large sample counts
full workload families
```

That work is real and should be kept as the baseline.

The pushed implementation is now reconstructible at:

```text
84a7d117c777fbd5c2f0d5d072e63769be842e7c
```

Relevant files:

```text
packages/iyon-runtime/src/tui/packed.ts
crates/iyon-native/src/tui/packed.rs
packages/iyon-runtime/src/tui/ir.ts
packages/iyon-runtime/src/tui/values/view.ts
packages/iyon-runtime/bench/tui_performance.ts
packages/iyon-runtime/bench/PERF-7v2-results.jsonl
packages/iyon-runtime/tests/tui_packed.test.ts
crates/iyon-tui/src/presentation/ir.rs
```

Permanent baseline links are listed in the research/source appendix.

---

# 2. What the PERF-7v2 data says

Across the 134 direct/packed matched cases in the committed result file, the packed path has a useful but uneven profile.

Using geometric mean of **commit median** ratios by mode:

```text
mode                  packed / direct     interpretation
-------------------   ---------------     ------------------------------
COLD                       ~0.851          packed ~14.9% faster
FIRST_USE                  ~0.912          packed ~8.8% faster
REBUILT_EQUIVALENT         ~0.856          packed ~14.4% faster
SHARED_DEEP                ~0.963          packed ~3.7% faster
SHARED_PATH                ~1.001          effectively neutral
SHARED_WIDE                ~0.997          effectively neutral
IDENTICAL_IDENTITY         ~1.133          packed ~13.3% slower
```

The exact percentages are not themselves an architecture guarantee; tails are noisy and the result set contains strong outliers. The shape is what matters:

```text
new data / rebuilt data:
    packed wins

retained changed path:
    packed ≈ direct

exact identity:
    packed loses
```

That is exactly the profile expected from the current implementation.

Packed encoding itself is also now visible as a meaningful part of the remaining cold/rebuilt cost. The median `encoding_median_ns / commit_median_ns` ratio in the result set is approximately:

```text
COLD                    ~17%
REBUILT_EQUIVALENT      ~11%
FIRST_USE                ~6%
SHARED_PATH              ~3.5%
SHARED_DEEP              ~3.6%
IDENTICAL_IDENTITY        0%
```

So there are two different optimization problems:

```text
cold/rebuilt:
    remove semantic re-encoding and reduce native reconstruction cost

warm retained:
    remove constant transaction/parser/cache overhead
    and eliminate O(width) parent redefinition when it exists
```

PERF-8 must attack both.

---

# 3. One reproducibility defect must be fixed before PERF-8

The PERF-7v2 result records contain:

```text
git_sha = e5292d62c4011610850cbdc1ba4a35f296f78e4f
```

but the implementation and results were committed afterward as:

```text
84a7d117c777fbd5c2f0d5d072e63769be842e7c
```

That means the benchmark was run from a dirty working tree based on `e5292d...` and then committed.

This does not invalidate the numbers, but it means `git_sha` alone does not reconstruct the measured source.

PERF-8 benchmark output must therefore record:

```json
{
  "git_sha": "...",
  "git_dirty": false,
  "protocol_version": 2,
  "bridge_schema_version": 1,
  "native_artifact_sha256": "...",
  "benchmark_source_sha256": "..."
}
```

Authoritative runs must normally require:

```text
git_dirty == false
```

If a dirty run is intentionally allowed during development, record a deterministic patch hash and mark it non-authoritative.

---

# 4. The current Packed V2 encoder still reinterprets the semantic graph

`PackedViewEncoder.encodeNode()` currently does all of these operations at commit time:

```text
inspect node kind
split NodeId
switch over every semantic node kind
walk layout-child objects
walk grid rows and cells
walk text spans
walk Diff hunks and lines
compute decoration presence flags
compute border flags
iterate Object.entries(style.attributes)
Map lookup each attribute name
iterate style-state records
build/dedupe string table
recursively decide REF vs DEF
patch recursive record lengths
```

The reusable `Uint32Array` removed a major allocation problem, but the packed path still performs a second semantic lowering pass after construction.

That work is avoidable.

PERF-8 must change the invariant from:

```text
View construction creates a rich JS object
commit later interprets that object
```

into:

```text
View construction creates canonical semantic state
AND the cheap metadata needed to transport that state
commit does not rediscover semantics
```

---

# 5. The current recursive DEF format still over-encodes stable immediate children

The current row/column encoder is effectively:

```ts
for (const child of children) {
  write(child.kind);
  write(child.size);
  write(child.maxRows);
  encodeNode(child.child);
}
```

If a new parent has 10,000 immediate children and only child 8,721 changed, the encoder still performs approximately:

```text
10,000 layout-child iterations
9,999 REF records
1 changed child DEF
1 parent DEF
```

Stable descendants are not visited, which is good, but stable immediate sibling references are still enumerated.

That is O(width).

A real structural delta needs to eliminate this work too.

---

# 6. The current `SHARED_WIDE` benchmark does not test a wide changed parent

The current harness builds the shared-wide shape approximately as:

```text
new root
├── huge stable subtree S
└── changed leaf X
```

The new root itself has only two children.

That proves:

```text
large stable subtree cutoff works
```

It does **not** prove:

```text
10,000-child parent + one changed immediate child is sublinear
```

PERF-8 must add a different benchmark:

```text
WIDE_PARENT_ONE_EDIT

Column C0
├── child 0
├── child 1
├── ...
├── child 8721 = X0
├── ...
└── child 9999

then:

Column C1
├── exactly the same children except
└── child 8721 = X1
```

Sizes:

```text
32
256
2,048
10,000
100,000
```

If transport work grows linearly with width, Packed V3 has failed its main structural goal.

---

# 7. Rust currently has a second O(width) barrier

The Rust retained View is already Arc-backed and persistent at the View-node level.

That is excellent.

However:

```rust
ColumnView {
    children: Arc<[ColumnChild]>,
    gap: u16,
}

RowView {
    children: Arc<[RowChild]>,
    ...
}
```

A flat `Arc<[T]>` is efficient to read, but changing one element requires constructing a new flat slice.

Therefore this packet:

```text
PATCH_CHILD index=8721 new=X1
```

would not be enough by itself.

If native applies it as:

```rust
let mut children = old.children.to_vec();
children[8721] = new_child;
let children: Arc<[ColumnChild]> = children.into();
```

then the algorithm is still O(width).

The cost merely moved from TypeScript to Rust.

**PERF-8 is invalid unless the retained Rust child representation changes too.**

---

# 8. Goal of PERF-8

Answer this question:

> If the semantic View graph, its wide child sequences, and the transport itself all preserve structural sharing, how close can a Bun → Rust View mutation get to the cost of the actual semantic change?

The target asymptotic behavior is:

```text
IDENTICAL:
    O(1)

SHARED narrow path:
    O(changed View nodes)

WIDE_PARENT_ONE_EDIT:
    O(log_B N + changed View nodes)

APPEND / INSERT / REMOVE in wide list:
    O(log_B N + inserted/removed payload)

COLD:
    O(total semantic data)

REBUILT_EQUIVALENT:
    O(total semantic data)
```

with a wide branching factor `B`, initially 32.

---

# 9. PERF-8 is not a generic tree-diff project

Do **not** implement:

```text
old BridgeViewNode tree
new BridgeViewNode tree
    ↓
walk both recursively
    ↓
compute edit script
```

That algorithm starts too late.

If the application has already built a 10,000-element flat child array, then comparing it to another 10,000-element flat child array is already O(N).

If exact immutable object identity already tells us what is shared, hashes/checksums are weaker information than we already have.

The optimal design records structural sharing **when the new value is created**.

This is the same high-level lesson visible in mature retained UI systems:

```text
React Native Fabric:
    immutable structurally-shared tree
    then native tree diff → mount mutations

Flutter:
    retained element/render trees
    explicit dirty elements
    local child reconciliation
    no global whole-tree diff

RRB / persistent vectors:
    immutable sequence update
    path-copy only O(log_B N) structure
```

PERF-8 should transmit explicit retained structure, not rediscover it with a post-hoc diff.

---

# 10. rsync-style rolling deltas are the wrong algorithm here

The rsync algorithm is brilliant for its problem:

```text
source and destination are on different machines
sender does not directly own the destination basis
link bandwidth/latency dominates
matching must be discovered from content
```

It therefore uses rolling checksums plus strong checksums to discover matching blocks.

Iyon has a different problem:

```text
same process
exact immutable object identity
explicit NodeId
explicit parent/child construction
native retained cache
```

Replacing exact identity with rolling hashes would add:

```text
hashing CPU
collision-handling complexity
block-boundary heuristics
extra metadata
```

while producing weaker knowledge.

Do not use rsync/content-defined chunking for the View graph.

Content-defined chunking may still be useful someday for very large opaque text/blob payloads, but not for semantic View identity.

---

# 11. Packed V3 architecture at a glance

Packed V3 introduces five cooperating pieces:

```text
1. PackedMeta
   construction-time transport metadata for each immutable semantic object

2. PackedRef
   dense 31-bit environment-local reference

3. PersistentSeq
   wide immutable sequence shared by TS and Rust semantics

4. flat V3 transaction
   definitions/patches reference objects by compact WireRef

5. native staged decoder
   resolve old refs once, build changed objects, publish weakly, mutate host once
```

Steady retained update:

```text
TS semantic operation
    ↓
new PackedMeta only for genuinely new semantic objects
    ↓
PersistentSeq path-copy where needed
    ↓
commit compiler visits changed/unpublished graph
    ↓
flat DEF / PATCH records
    ↓
N-API once
    ↓
Rust resolves retained refs
    ↓
constructs changed Arc nodes / sequence chunks only
    ↓
commit host state
```

---

# 12. Keep three identities separate

Do not overload one identifier with three different jobs.

Packed V3 has:

```text
NodeId
PackedRef
LocalDefIndex
```

They have different semantics.

## 12.1 NodeId

Existing semantic identity:

```text
range: 1 .. 2^53 - 1
lifetime: semantic JS node lifetime / process uniqueness
meaning: this immutable semantic View identity
```

NodeId remains authoritative for correctness and direct-bridge parity.

It is sent only when a semantic View is defined.

It is **not** sent on ordinary warm references.

## 12.2 PackedRef

New transport-local dense identity:

```text
range: 1 .. 0x7fff_ffff
width: 31 bits
lifetime: packed transport generation
meaning: lookup this retained packed object in native cache
```

PackedRef identifies both semantic Views and packed-internal retained objects such as sequence chunks.

PackedRef does not replace NodeId.

## 12.3 LocalDefIndex

Transaction-local definition index:

```text
range: 0 .. 0x7fff_ffff
lifetime: one packed transaction
meaning: definition N earlier/in this transaction
```

It eliminates temporary native hash lookup for new definitions.

---

# 13. `WireRef` is one u32

Use the high bit to distinguish persistent and transaction-local references.

```text
0x00000000
    INVALID

0x00000001 .. 0x7fffffff
    persistent PackedRef

0x80000000 .. 0xffffffff
    LocalDefIndex
    index = word & 0x7fffffff
```

Pseudo-code:

```ts
const LOCAL_BIT = 0x8000_0000;

function persistentRef(ref: number): number {
  assert(ref > 0 && ref < LOCAL_BIT);
  return ref;
}

function localRef(index: number): number {
  assert(index >= 0 && index < LOCAL_BIT);
  return LOCAL_BIT + index;
}
```

Do not use signed JS bitwise operations on the semantic 53-bit NodeId.

Using bitwise operations on these explicitly-u32 transport refs is fine if the result is normalized with `>>> 0`.

---

# 14. PackedRef allocation

Use one environment/runtime allocator per transport generation:

```ts
class PackedRefAllocator {
  private next = 1;

  reset(): void {
    this.next = 1;
  }

  allocate(): number {
    if (this.next >= 0x7fff_ff00) {
      throw new PackedRefGenerationExhausted();
    }
    return this.next++;
  }
}
```

A `PackedMeta` therefore stores both:

```text
ref
refGeneration
```

Before a ref is used:

```ts
function ensureRef(meta: PackedMeta): number {
  if (meta.refGeneration !== currentGeneration) {
    meta.ref = allocator.allocate();
    meta.refGeneration = currentGeneration;
    meta.publishedGeneration = 0;
  }
  return meta.ref;
}
```

Do not reuse a PackedRef within one generation.

Reserve a small range near the ceiling for future protocol sentinels.

On cache resynchronization **or** ref-space rollover:

```text
increment transport generation
reset native packed slots
reset allocator to 1
PackedMeta refs from older generations become invalid by generation tag
assign new refs lazily only to objects used in the new generation
cold-define the current root closure
```

This avoids an ABA collision with old JS Views: an old metadata object never reuses its stale numeric ref merely because the allocator restarted. `ensureRef()` gives it a current-generation ref before use.

2^31 retained-object allocations in one generation are already pathological; generation exhaustion is a correctness path, not a normal performance path.

---

# 15. Transport generation replaces WeakSet reset as the primary validity test

Current Packed V2 resets:

```ts
knownNodes = new WeakSet();
refPackets = new WeakMap();
```

V3 should make published knowledge generation-based.

Each metadata object stores:

```ts
interface PackedMeta {
  ref: number;
  refGeneration: number;
  publishedGeneration: number;
  visitEpoch: number;
  ...
}
```

The encoder owns:

```ts
currentGeneration: number;
```

A node is known native iff:

```ts
meta.publishedGeneration === currentGeneration
```

Global invalidation is therefore O(1):

```ts
currentGeneration += 1;
allocator.reset();
```

No global walk. Old metadata retains its stale `(refGeneration, ref)` pair, but `ensureRef()` reassigns a fresh numeric ref before that object can participate in the new generation.

No all-time strong Set.

No need to allocate a replacement WeakSet merely to invalidate transport knowledge.

---

# 16. Native generation semantics

Native environment state becomes approximately:

```rust
struct ViewBridgeCache {
    // Existing semantic correctness/cache index.
    nodes: HashMap<u64, WeakView>,

    // Packed acceleration state.
    packed_generation: u32,
    packed_slots: PackedSlotTable,
}
```

Normal V3 transaction:

```text
header.generation == cache.packed_generation
```

otherwise:

```text
PACKED_GENERATION_MISMATCH
```

A resynchronizing cold transaction carries:

```text
RESET_GENERATION
COLD_CLOSURE
```

and:

```text
header.generation == old_generation + 1
```

Native then:

```text
clear packed_slots
set packed_generation
process full cold closure
```

The existing semantic `NodeId -> WeakView` map does not need to be cleared merely because the packed reference generation changed.

---

# 17. Packed slots are an acceleration index, not a second retention model

Do not recreate the old mistake of giving packed transport a different semantic lifetime.

A View packed slot stores the same weak Rust View used by semantic caching:

```rust
enum PackedSlot {
    Empty,
    View {
        node_id: u64,
        weak: WeakView,
    },
    Seq {
        weak: Weak<PersistentSeqNode>,
    },
    // Future retained payload kinds if justified.
}
```

The packed slot does not keep the object alive.

The host/History/ViewSlot/etc. own strong Views in the normal way.

When the strong semantic owner disappears:

```text
WeakView can expire
PackedRef lookup becomes a cache miss
cold recovery remains correct
```

---

# 18. Prefer a paged packed slot table over a HashMap

PackedRef values are monotonic dense integers.

A hash table is unnecessary for the hot lookup.

Use a paged slot table:

```text
PAGE_SHIFT = 12
PAGE_SIZE  = 4096

page  = ref >> PAGE_SHIFT
offset = ref & (PAGE_SIZE - 1)
```

Rust shape:

```rust
struct PackedSlotTable {
    pages: Vec<Box<[PackedSlot; 4096]>>,
}
```

Allocate pages lazily.

Properties:

```text
lookup      O(1)
no hashing
predictable memory access
no per-ref allocation
cheap full-generation reset
```

Do not allocate a single `Vec<PackedSlot>` to the maximum possible ref.

---

# 19. Construction-time PackedMeta is mandatory

The commit path must not rediscover:

```text
NodeId split
kind discriminant conversion
decoration flags
style flags
attribute masks
border flags
layout child tag conversion
color tag conversion
safe integer validation
```

Do those operations when the immutable semantic value is created.

For the benchmark implementation, attach a private sidecar:

```ts
const PACKED_META = Symbol("iyon:tui:packed-meta");

interface PackedMeta {
  ref: number;
  refGeneration: number;
  readonly nodeIdLow: number;
  readonly nodeIdHigh: number;
  readonly recipe: PackedRecipe;
  readonly lineage?: PackedLineage;
  publishedGeneration: number;
  visitEpoch: number;
  localDefIndex: number;
}
```

The semantic JS object remains frozen.

`PackedMeta` is mutable transport bookkeeping and is never public.

During the benchmark, keep this sidecar behind the Packed V3 candidate so Candidate A/B controls remain reconstructible.

If V3 wins production:

```text
do not retain “rich bridge IR + permanent sidecar” forever
```

Fold the transport-friendly canonical representation into the private View IR and remove the losing bridge representation.

---

# 20. The production private TS IR should converge toward Rust's retained IR

Rust already has the cleaner retained shape:

```text
ViewNode
    common width/height/decoration/style state
    kind payload
```

TS currently represents decoration as a wrapper node:

```text
Decorated
    child
    decoration
```

and native lowers that wrapper into common Rust ViewNode fields.

Long-term packed production should not preserve that impedance mismatch.

The optimal private TS form is approximately:

```ts
interface CanonicalViewNode {
  id: number;
  kind: ViewKind;

  width: WidthRuleCode;
  height: HeightRuleCode;
  decoration: CanonicalDecoration;
  styleStates: CanonicalStyleStates;

  payload: KindPayload;
}
```

Semantic builder calls still create a new NodeId for every semantic mutation.

Unchanged kind payloads are shared.

This mirrors Rust `View::map_node()` semantics rather than reconstructing wrapper layers across N-API.

Do this only after Packed V3 correctness is proven, or behind the candidate feature while benchmarking.

---

# 21. Canonicalize styles before commit

Current commit-time code performs:

```ts
Object.entries(style.attributes)
ATTRIBUTE_BITS.get(name)
present |= bit
truth |= bit
```

That is unnecessary.

Private canonical style should store:

```ts
interface CanonicalStyle {
  readonly flags: number;
  readonly theme?: string;
  readonly foreground?: CanonicalColor;
  readonly background?: CanonicalColor;
  readonly attributePresent: number;
  readonly attributeTrue: number;
}
```

When public `Style` methods run:

```text
validate attribute name once
set canonical masks once
```

Packed commit writes masks directly.

Direct bridge decoding, while retained for comparison, can read the same numeric masks directly.

Explicit-false semantics remain:

```text
present=0               unset
present=1,true=0        explicit false
present=1,true=1        explicit true
```

---

# 22. Canonicalize decoration and border before commit

Likewise, construct these once:

```ts
interface CanonicalDecoration {
  readonly presence: number;
  readonly paddingTop: number;
  readonly paddingRight: number;
  readonly paddingBottom: number;
  readonly paddingLeft: number;
  readonly widthRule: number;
  readonly heightRule: number;
  ...
}

interface CanonicalBorder {
  readonly presence: number;
  readonly styleTag: number;
  readonly edgeTag: number;
  readonly glyphs?: readonly [string,string,string,string,string,string,string,string];
  readonly color?: CanonicalColor;
}
```

Do not run `Object.entries()` or string-to-enum conversion in the hot commit compiler.

---

# 23. Explicit lineage is the basis of PATCH

A patch must never be inferred by comparing arbitrary old/new trees.

A patch is valid only when construction already knows the predecessor.

Example:

```ts
const b = a.padding(1);
```

The View constructor knows:

```text
base = a
change = padding
```

Store:

```ts
interface PackedLineage {
  readonly base: PackedMeta;
  readonly patch: PackedPatchRecipe;
}
```

Similarly:

```text
text.wrap(...)
text.textAlign(...)
existing axis with child replacement
existing axis gap change
style-state update
bounds update
border replacement
```

can carry exact lineage.

A newly built arbitrary tree has no lineage and receives a full DEF.

---

# 24. PATCH never mutates the old semantic View

This invariant is non-negotiable.

Wire:

```text
PATCH newRef/newNodeId FROM baseRef
```

means:

```text
construct a NEW immutable Rust View
that structurally shares unchanged payload with baseRef
```

It never means:

```text
mutate baseRef in place
```

Why:

```text
old JS View may still exist
old History item may still own Rust View
old ViewSlot frame may still own Rust View
NodeId means immutable semantic identity
```

Rust already exposes the right internal behavior conceptually through:

```rust
View::map_node(...)
View::map_text(...)
```

Packed V3 should expose a crate-private canonical patch API built on those semantics.

---

# 25. Patch chains should collapse to a useful base

Consider:

```ts
View.text("x")
  .padding(1)
  .foreground("cyan")
  .maxWidth(40)
```

Do not force native to reconstruct every transient intermediate if only the final value is reachable.

At commit time:

```text
walk lineage backward
find nearest base that is either:
    a) already published in current generation, or
    b) necessarily reachable as another local definition

compose compatible common-field patches
```

If no useful retained base exists:

```text
emit full DEF for final node
```

Never emit a long patch chain merely because the user called fluent methods.

Patch composition must be exact:

```text
last writer wins for replacement fields
attribute masks merge according to semantic rules
style-state changes preserve explicit map semantics
kind-changing operations stop patch composition
```

When composition becomes more expensive than full DEF, emit full DEF.

---

# 26. The core wide-sequence type is a persistent relaxed radix tree

Row, Column, Grid rows/cells, and eventually large text/diff payload sequences need a persistent structure.

Use a specialized RRB-like vector with branch factor 32.

Call it:

```text
PersistentSeq<T>
```

Do not depend on a generic immutable-collections framework unless benchmark data proves it matches the specialized implementation.

Target structure:

```text
Leaf
    len <= 32
    items[0..len]
    aggregate metadata

Branch
    child_count <= 32
    children[0..child_count]
    cumulative_sizes[0..child_count]
    aggregate metadata
```

Cumulative sizes allow relaxed balancing and fast indexed lookup.

---

# 27. PersistentSeq lookup

Given logical index `i`:

```text
branch:
    locate first cumulative_size > i
    subtract previous cumulative size
    descend

leaf:
    return items[i]
```

Complexity:

```text
O(log_32 N)
```

For 100,000 children the depth is tiny.

---

# 28. PersistentSeq replace/set

`set(index, value)`:

```text
1. locate path root → leaf
2. allocate one replacement leaf
3. allocate one replacement branch at each ancestor
4. reuse every sibling subtree Arc/object unchanged
5. recompute aggregate metadata only along changed path
```

Complexity:

```text
CPU            O(log_32 N)
new seq nodes  O(log_32 N)
retained data  O(log_32 N)
```

This is the structure required for a true one-child delta.

---

# 29. PersistentSeq append

Use a tail-optimized persistent vector algorithm:

```text
if rightmost leaf has capacity:
    clone only rightmost path and append

else:
    create new leaf
    propagate insertion up right spine
    split full branch if necessary
```

Amortized behavior should be near O(1) with O(log_32 N) worst-case path copying.

For bulk builder construction, do not repeatedly use persistent append; use a transient builder described below.

---

# 30. PersistentSeq insert/remove/splice

UI child lists can require insertions/removals, not only replacement.

Support:

```text
split(index)
concat(left, right)
splice(index, remove_count, inserted)
```

Use RRB-style rebalancing:

```text
split only boundary paths
reuse untouched interior subtrees
rebalance adjacent underfull nodes
keep branch occupancy within declared bounds
```

Target:

```text
split       O(log N)
concat      O(log N)
splice      O(log N + inserted payload)
```

Do not implement insertion by flattening to `Array`/`Vec` and rebuilding.

---

# 31. Transient builders prevent persistent-structure overhead during cold construction

A cold 10,000-child tree is built from scratch.

If every builder `.child()` performed persistent path-copy, cold construction would regress.

Use a transient builder:

```ts
class PersistentSeqBuilder<T> {
  // mutable private branch/tail state
  push(value: T): void;
  freeze(): PersistentSeq<T>;
}
```

Rust should have the equivalent for native construction.

The builder may mutate nodes that are private to that builder.

After `freeze()`:

```text
nodes become immutable
sharing invariants apply
builder cannot mutate frozen nodes
```

Cold construction remains O(N) with low constants.

---

# 32. PersistentSeq must cache aggregate View flags

Current Rust `ViewNode::compute_flags()` folds every child to determine facts such as:

```text
contains component slot
```

A persistent child patch would lose its asymptotic benefit if each new parent recomputed aggregate flags by scanning all children.

Every sequence node must cache associative aggregate facts.

For View children:

```rust
struct SeqAggregate {
    view_flags: ViewFlags,
}
```

Leaf aggregate:

```text
OR all item flags
```

Branch aggregate:

```text
OR all child aggregates
```

After one child replacement:

```text
recompute only O(log_32 N) sequence nodes
```

Then parent View flags are O(1) from sequence root aggregate.

This rule applies to any future aggregate retained by layout/paint.

---

# 33. Rust Row/Column must stop storing flat Arc slices

Target:

```rust
pub(crate) struct ColumnView {
    pub(crate) children: PersistentSeq<ColumnChild>,
    pub(crate) gap: u16,
}

pub(crate) struct RowView {
    pub(crate) children: PersistentSeq<RowChild>,
    pub(crate) gap: u16,
    pub(crate) vertical_align: VerticalAlign,
}
```

Layout/measure/paint code must consume:

```text
.children.len()
.children.iter()
.children.get(index)
```

without calling:

```text
to_vec()
to_array()
flatten()
collect::<Vec<_>>()
```

in the normal path.

A temporary flattening adapter is allowed only in tests during migration and must be removed before performance measurement.

---

# 34. Grid requires persistent sequences too

Grid currently has multiple flat collections:

```text
columns
rows
cells
```

Use retained sequence objects for:

```text
track vectors
row vectors
cell vectors
```

At minimum, the cell collection must support structural sharing.

For a changed cell:

```text
new cell View
+ changed cell-sequence path
+ changed row/grid metadata path
```

Do not resend or reconstruct every stable grid cell.

---

# 35. Text spans should become persistent only where it pays

Do not blindly replace every tiny `Arc<[TextSpan]>` with a tree.

Text usually has small span counts.

Use a hybrid:

```text
<= INLINE_SPAN_LIMIT:
    compact Arc/small immutable span block

> INLINE_SPAN_LIMIT or edit-heavy retained text:
    PersistentSeq<TextSpan>
```

The threshold is benchmark-selected.

Wrap/alignment changes must already be O(1) because Rust `map_text()` can share the span storage.

One-span replacement in a huge syntax-highlighted line should become sublinear if such workloads exist.

---

# 36. Diff deserves a retained payload path

PERF-7v2's `diff_heavy` workload is one of the cases where packed does not clearly win.

The current transport encodes:

```text
all hunks
all lines
all line text
all line numbers
```

and native reconstructs/renderers the Diff payload.

For optimal retained behavior:

```text
DiffSource
    PersistentSeq<DiffHunk>

DiffHunk
    PersistentSeq<DiffLine>
```

Native should cache rendered hunk Views by retained hunk identity.

A changed hunk then becomes:

```text
new/changed lines
changed hunk sequence path
changed DiffSource sequence path
re-render changed hunk only
compose stable rendered hunk Views by retained identity
```

Do not turn PERF-8 into a complete Diff-engine rewrite before the core transport works, but make this a required specialization if `diff_heavy` remains a material regression after V3 core.

---

# 37. Flat records replace recursive REF/DEF trees

Packed V2 record nesting mirrors the semantic tree recursively.

Packed V3 should be flat.

Definitions form a topologically ordered list.

Each definition references dependencies with `WireRef`.

Advantages:

```text
no recursive record nesting
no REF record per child
no record-length backpatch for nested descendants
no native active-NodeId HashSet for ordinary cycle detection
local definitions resolve by Vec index
compact child references
better decoder branch predictability
```

---

# 38. Packed V3 header

Protocol version after V2's version 1:

```text
PACKED_VIEW_PROTOCOL_VERSION = 2
```

Recommended header:

```text
word  meaning
----  --------------------------------------------------
0     magic
1     packed protocol version (=2)
2     bridge schema version
3     packed transport generation
4     flags
5     used word count
6     used byte-lane count
7     definition count
8     operation count
9     reserved = 0
10..  definition records, then operation records
```

Flags initially:

```text
RESET_GENERATION = 1 << 0
COLD_CLOSURE     = 1 << 1
HAS_BYTE_LANE    = 1 << 2
```

Unknown mandatory flag:

```text
hard error
```

Reserved words must be zero so future extensions can be detected safely.

---

# 39. Definition record envelope

Every definition starts:

```text
record_tag
record_words
persistent_ref
```

A semantic View definition then adds:

```text
node_id_low
node_id_high
... payload ...
```

Packed-internal objects such as sequence chunks do not need a semantic NodeId unless they become part of a user-visible semantic identity contract.

They still receive PackedRef.

---

# 40. Definition record tags

Initial protocol tags:

```text
DEF_VIEW_FULL
PATCH_VIEW
DEF_SEQ_LEAF_COLUMN
DEF_SEQ_LEAF_ROW
DEF_SEQ_BRANCH
DEF_GRID_TRACK_SEQ
DEF_GRID_CELL_SEQ
DEF_TEXT_SPAN_BLOCK       if hybrid text path requires it
DEF_DIFF_HUNK             if Diff specialization is enabled
DEF_DIFF_LINE_SEQ         if Diff specialization is enabled
```

Do not create one generic “blob object” whose semantics are guessed from payload.

Tags must be schema-generated and checked in TS/Rust from one canonical source.

---

# 41. Full View definitions reference children; they do not contain child definitions

Example Column:

```text
DEF_VIEW_FULL
record_words
new_packed_ref
node_id_low
node_id_high
VIEW_KIND_COLUMN
common_fields...
gap
child_sequence_wire_ref
```

That is all.

The child sequence itself is another retained definition or persistent reference.

A new parent no longer serializes every immediate child inline.

---

# 42. Sequence leaf wire format

For a Column leaf:

```text
DEF_SEQ_LEAF_COLUMN
record_words
new_packed_ref
item_count
aggregate_flags
repeated item_count times:
    track_kind_and_small_fields
    child_view_wire_ref
```

A leaf contains at most 32 items.

Row leaf is analogous.

Pack small track data into a u32 where that improves decoder simplicity:

```text
bits 0..3    track kind
bits 4..19   u16 size/max/min as applicable
remaining    reserved
```

Do not create bitfields so dense that encode/decode becomes branchier than the word savings justify.

The decision is benchmark-driven, but fixed word-aligned fields are the default.

---

# 43. Sequence branch wire format

```text
DEF_SEQ_BRANCH
record_words
new_packed_ref
height
child_count
aggregate_flags
repeated child_count times:
    cumulative_size
    child_sequence_wire_ref
```

`child_count <= 32`.

Native validates:

```text
cumulative sizes strictly increase
last size == logical subtree length
child heights valid
aggregate matches children in debug/differential mode
```

Do not trust the sender merely because it is same-process JS.

Malformed packed input must fail safely.

---

# 44. PATCH_VIEW format

```text
PATCH_VIEW
record_words
new_packed_ref
new_node_id_low
new_node_id_high
base_view_wire_ref
patch_kind
patch_mask
patch_payload...
```

`base_view_wire_ref` may be:

```text
persistent ref
or earlier LocalDefIndex
```

if the base is intentionally part of the reachable transaction.

Cold recovery is simpler if it emits only full definitions, but ordinary transactions may patch from local retained construction when profitable.

---

# 45. Common-field patch mask

At minimum:

```text
WIDTH_RULE
HEIGHT_RULE
PADDING
BOUNDS
SURFACE_BACKGROUND
BORDER
TEXT_STYLE
STYLE_STATES
```

Patch semantics are replacement semantics matching canonical Rust `ViewNode` fields.

Do not reproduce TS decorator-wrapper merge semantics in native.

TS construction has already produced the final canonical field values.

Native receives the final changed common fields and shallow-clones the base ViewNode.

---

# 46. Text patch kinds

Allow:

```text
TEXT_WRAP
TEXT_ALIGN
TEXT_CURSOR       if exposed by this bridge
TEXT_SPANS        only when span storage itself changed
```

For wrap/alignment-only patch:

```rust
base.clone().map_text(|text| {
    text.wrap = new_wrap;
    text.align = new_align;
})
```

Span Arc/PersistentSeq remains shared.

Do not retransmit text bytes for wrap/alignment changes.

---

# 47. Axis patch kinds

Column/Row patch can replace:

```text
GAP
CHILD_SEQUENCE_ROOT
VERTICAL_ALIGNMENT where applicable
```

One-child change:

```text
new child View
new O(log N) PersistentSeq path
PATCH parent with new seq root
```

Native does not enumerate stable siblings.

---

# 48. No patch may silently change kind

If:

```text
base.kind != target.kind
```

then either:

```text
emit a full definition
```

or use an explicitly schema-defined kind-changing constructor whose semantics are independently validated.

Do not let a field mask reinterpret a Text base as Column, etc.

---

# 49. Local definition order is topological

For every `LocalDefIndex` reference:

```text
referenced_index < current_definition_index
```

This gives native an extremely simple invariant:

```rust
let object = &staged[local_index];
```

No transaction-local HashMap.

No forward-reference fixups.

No cycles.

A cycle or forward local reference is malformed input and hard-fails.

---

# 50. Persistent references must belong to the current generation

A persistent `WireRef` is meaningful only under:

```text
transaction.generation == native.packed_generation
```

Native lookup:

```text
slot exists
slot object kind matches expected kind
weak upgrade succeeds
```

otherwise:

```text
PACKED_CACHE_MISS
```

Do not fall back to interpreting the numeric ref as NodeId.

---

# 51. Native decode is a staged transaction

Do not mutate host state while decoding.

Recommended phases:

```text
PHASE 1 — header and structural validation
PHASE 2 — resolve/build all definitions into strong staging objects
PHASE 3 — resolve operation roots
PHASE 4 — publish weak cache entries
PHASE 5 — mutate host exactly once
```

If phases 1–3 fail:

```text
host unchanged
packed cache publication unchanged
```

Staged objects are dropped.

---

# 52. Do not take the cache mutex for every REF

Current packed decoding can lock the shared cache around individual lookups/inserts.

V3 should batch.

A practical structure:

```text
1. validate transaction structure without cache mutation
2. identify persistent refs as records are decoded
3. acquire environment cache lock
4. upgrade required persistent objects into strong temporary handles
5. release lock
6. construct local definitions
7. reacquire cache lock once
8. publish all new weak slots + NodeId weak entries
9. release lock
10. mutate host
```

If resolving dependencies requires sequential construction, keep a small transaction resolver that can request persistent objects from a pre-resolved table.

The goal is:

```text
O(1) cache lock acquisitions per transaction
```

not per node.

---

# 53. Native packed construction must bypass ergonomic public builders

Current V2 decoding often reconstructs via APIs such as:

```rust
View::vertical(|column| { ... })
View::horizontal(|row| { ... })
DiffRenderer::render(...)
```

Those are good public APIs, not necessarily the lowest-cost decoder APIs.

Add crate-private validated constructors in `iyon-tui`, for example:

```rust
pub(crate) fn from_canonical_parts(parts: ViewNodeParts) -> View;
pub(crate) fn patch_canonical(base: &View, patch: CanonicalPatch) -> Result<View>;
pub(crate) fn column_from_persistent(...);
pub(crate) fn row_from_persistent(...);
```

The packed decoder validates wire invariants first, then calls these constructors directly.

Do not duplicate semantic rules between `iyon-native` and `iyon-tui` where a canonical core constructor can own them.

---

# 54. Preserve Rust structural sharing all the way through layout

The following is banned in the hot path:

```rust
persistent.children().collect::<Vec<_>>()
persistent.to_vec()
Arc::from(flattened)
```

Layout/measurement/paint must iterate the persistent structure directly.

A persistent transport that is flattened before layout is not a retained structural optimization.

Add counters to prove no flatten occurs.

---

# 55. Semantic equality checks must exploit pointer sharing

Packed V2 correctly detects:

```text
same NodeId resent with different semantics
```

Keep this.

But `PartialEq` over a new persistent sequence must short-circuit aggressively:

```text
if Arc::ptr_eq(root_a, root_b):
    equal

else:
    compare lengths/metadata
    recursively compare children
    pointer-equal child chunks short-circuit
```

Do not rely on a probabilistic hash as proof of semantic equality.

A cached hash may be used to reject inequality quickly, but equal hashes must not skip exact comparison unless the hash is part of a formally collision-free identity construction.

---

# 56. Exact root identity gets a dedicated native call

Current exact identity still sends a 9-word root REF packet and runs the packed transaction parser.

That is why Packed V2 can lose exact-identity latency even with zero encoding.

Add:

```ts
host.tuiPackedRenderRef(generation, packedRef)
```

Native path:

```text
ensure host alive
verify generation
lookup packed slot by dense u32
WeakView upgrade
host.render(view)
return
```

No:

```text
Uint32Array
header parse
record parse
strings parameter
NodeId split/reconstruct
transaction allocation
```

This should become the authoritative exact-root path.

---

# 57. Forest fast path

For operations such as animation that submit multiple already-known roots:

```text
small root count:
    use reusable Uint32Array of PackedRefs

large/mixed forest:
    ordinary V3 transaction
```

Do not make one N-API call per frame/root.

The invariant remains:

```text
one semantic state mutation
→ one native call
```

---

# 58. String transport must be re-benchmarked from first principles

Current V2 signature is effectively:

```rust
fn render_packed(words: Uint32Array, strings: Vec<String>)
```

That causes N-API/napi-rs to materialize the JS `string[]` as owned Rust `String`s before the decoder runs.

The decoder then commonly performs another `.to_owned()` while constructing semantic payloads.

That is an avoidable ownership/copy layer.

PERF-8 must benchmark two corrected string lanes before freezing protocol V3.

Do **not** choose based on aesthetics.

---

# 59. String lane candidate S1 — reusable UTF-8 byte arena

JS owns:

```ts
Uint8Array bytes
TextEncoder encoder
```

Use:

```ts
TextEncoder.encodeInto(source, bytes.subarray(cursor))
```

String field wire representation:

```text
byte_offset
byte_length
```

Properties:

```text
one byte buffer argument
no JS array of N string values crossing N-API
no per-string N-API value conversion
buffer reused between transactions
only changed/new definition strings encoded
```

Native:

```text
validate range
validate UTF-8
construct final owned/retained text representation once
```

Do not assume this wins; include JS UTF-8 encoding CPU in total construction+commit timing.

---

# 60. String lane candidate S2 — move-once native strings

Keep the JS string lane but remove the second ownership copy.

Options include:

```text
consume each Vec<String> entry exactly once when not deduplicated
or
store use counts and move singleton entries, clone only repeated refs
or
bind a lower-level array and construct the final Rust String directly
```

The critical invariant is:

```text
JS string → final Rust owned text
```

must not become:

```text
JS string
→ temporary Vec<String>
→ second String allocation/copy
→ final View
```

---

# 61. String-lane decision rule

Microbench representative distributions:

```text
10,000 unique short ASCII strings
10,000 repeated short style/theme strings
1,000 medium Unicode strings
100 large Unicode strings
mixed realistic text/span payload
Diff lines
```

Measure:

```text
JS CPU
native CPU
allocations
bytes copied
median
p95
heap/RSS
```

Choose the lane with the lowest **total construction + commit** cost and acceptable memory behavior.

If UTF-8 arena wins materially, V3 protocol uses it.

If corrected move-once `string[]` wins, do not ship UTF-8 encoding merely because it looks lower-level.

Optimal means measured end-to-end.

---

# 62. Optional string-slab retention experiment

Only if string copying remains dominant after S1/S2:

```text
JS packs all changed UTF-8 bytes contiguously
native copies the used byte lane ONCE into Arc<[u8]>
retained text fields reference validated ranges in that slab
```

This requires an internal text storage type similar to:

```rust
enum RetainedText {
    Owned(Arc<str>),
    Slab {
        bytes: Arc<[u8]>,
        start: u32,
        len: u32,
    },
}
```

`as_str()` returns the validated range.

Do not retain a JS ArrayBuffer after the N-API call unless lifetime/engine guarantees are formally proven.

The safe default is one contiguous native copy.

Memory caveat:

```text
one tiny live string can retain a whole transaction slab
```

so use slab size thresholds or compact on pressure if this path is adopted.

This is a secondary experiment, not a prerequisite for core V3.

---

# 63. Reusable structural arenas remain

Keep the successful V2 principle:

```text
reusable Uint32Array
geometric growth only
used-word count in header
```

Add the same for UTF-8 if S1 wins:

```text
reusable Uint8Array
geometric growth only
used-byte count in header
```

After warmup, steady retained commits should have:

```text
word_buffer_grows = 0
byte_buffer_grows = 0
transaction_buffer_allocations = 0
```

---

# 64. Do not make a native-owned external ArrayBuffer a prerequisite

Node-API supports external ArrayBuffers, and a native-owned shared scratch arena is theoretically attractive.

However current V2 already borrows `Uint32Array` contents synchronously; there is no evidence that ownership transfer is the dominant cost.

Bun's exact external-ArrayBuffer behavior must also be verified.

Therefore:

```text
first optimize semantic work, refs, records, native reconstruction, and strings
```

Then microbenchmark:

```text
JS-owned reusable typed arrays
vs
native-owned external scratch arrays
```

Only adopt native-owned scratch if it produces a repeatable end-to-end win.

Do not add lifetime/finalizer complexity for an unmeasured pointer-ownership optimization.

---

# 65. Bun FFI is benchmark-only unless its production status changes

Bun documents `bun:ffi` as experimental and explicitly recommends Node-API as the stable production route.

Therefore Packed V3 production must not depend on it by default.

After the stable N-API exact-ref path exists, an optional experiment may expose a C ABI function pointer and call it with `CFunction`:

```text
render_ref(host_ptr, generation, packed_ref)
```

Measure only the constant boundary overhead.

If it saves a few hundred nanoseconds but introduces an experimental production dependency, reject it unless the project explicitly chooses that risk.

Do not let microbenchmark vanity compromise transport correctness.

---

# 66. JS transaction compilation must use epoch marks, not Maps, for reachability

Packed V2 uses:

```ts
seenThisTransaction = new Map<number, BridgeViewNode>()
```

V3 metadata already exists per retained object.

Use an epoch plus an explicit temporary state:

```ts
const UNASSIGNED = 0xffff_ffff;
transactionEpoch += 1;

function begin(meta: PackedMeta): "new" | "emitted" {
  if (meta.visitEpoch !== transactionEpoch) {
    meta.visitEpoch = transactionEpoch;
    meta.localDefIndex = UNASSIGNED;
    return "new";
  }

  if (meta.localDefIndex === UNASSIGNED) {
    throw new Error("cyclic packed semantic dependency");
  }

  return "emitted";
}
```

Compilation marks a node `VISITING` before descending, compiles dependencies first, then assigns:

```ts
meta.localDefIndex = emittedDefinitionCount++;
```

A repeated DAG edge after that returns the existing LocalRef. A repeated edge while `UNASSIGNED` is a cycle and hard-fails.

This removes transaction hash-table work for DAG duplicate detection without losing cycle detection.

The metadata object is transport-private and mutable, so this does not violate semantic immutability.

---

# 67. Commit compiler algorithm

Normal changed-root transaction:

```text
compile(root):

1. if root.publishedGeneration == currentGeneration:
       use exact-ref fast path
       return

2. increment transactionEpoch

3. recursively/iteratively compile dependencies:
       if object published:
           return PersistentRef

       if object state == EMITTED this txn:
           return LocalRef(meta.localDefIndex)

       if object state == VISITING this txn:
           fail: semantic cycle

       mark VISITING

       choose:
           PATCH if a profitable valid lineage base is available
           otherwise FULL DEF

       compile all dependencies required by chosen record first

       assign localDefIndex = next emitted index
       emit flat record
       mark EMITTED

4. emit operation record referencing root WireRef

5. invoke native once

6. on success:
       mark every emitted object.publishedGeneration = currentGeneration

7. on cache miss:
       generation++
       cold-compile complete current root closure using FULL DEF only
       retry once
```

Implementation may use an explicit stack to avoid JS recursion for very deep trees.

---

# 68. PATCH profitability must be deterministic

Do not emit PATCH merely because lineage exists.

Estimate exact word/byte cost from precomputed recipes:

```text
patch_words + patch_bytes
vs
full_words + full_bytes
```

Choose PATCH only when:

```text
patch is smaller
AND
base resolution does not introduce extra required definitions
AND
native patch CPU is not known to exceed full construction
```

For common-field changes and sequence-root replacement, PATCH should usually win strongly.

For tiny Spacer/Component nodes, FULL DEF may be cheaper.

---

# 69. Cold retry contains no persistent dependencies

Cold recovery transaction is authoritative and self-contained.

Flags:

```text
RESET_GENERATION
COLD_CLOSURE
```

Rules:

```text
all reachable View/sequence objects are full-defined
all dependencies use LocalDefIndex
no PATCH requires old packed state
no persistent WireRef is allowed in the closure
```

Native receiving a persistent ref under `COLD_CLOSURE`:

```text
protocol error
```

Therefore a cache miss during cold retry is impossible by design.

If it occurs:

```text
hard fail
```

Do not retry indefinitely.

---

# 70. Weak-cache recovery remains one retry

Normal path:

```text
persistent PackedRef
→ Weak upgrade succeeds
→ continue
```

Expiry path:

```text
persistent PackedRef
→ weak expired
→ PACKED_CACHE_MISS
→ no host mutation
→ JS generation++
→ full cold closure in new generation
→ retry once
→ success
```

Second failure:

```text
hard error
```

The packed acceleration cache may disappear at any time without semantic corruption.

That property must remain true forever.

---

# 71. Cache publication and host mutation ordering

Definitions are immutable.

The safest order is:

```text
validate/decode everything
resolve operation roots
publish weak definitions
perform host mutation
```

If host mutation fails for a non-cache reason after weak publication:

```text
published immutable definitions may remain safely cached
JS does NOT mark them published because the N-API call failed
```

A later resend may full-define the same NodeId/packed object.

Native must accept semantically identical redundant definitions and reject identity changes.

No host-visible partial mutation is allowed.

---

# 72. Component handles are not PackedRefs

Component/native handles have their own identity domain.

Do not encode:

```text
component handle as PackedRef
```

Keep their full safe-integer/native-handle representation as defined by the bridge.

PackedRef only addresses retained packed transport objects.

This avoids accidental aliasing of component lifetime with semantic View lifetime.

---

# 73. Operation records make one decoder reusable for every View boundary

V3 transaction should end in one or more operation records.

Initial operation tags:

```text
RENDER
HISTORY_PUSH
HISTORY_FREEZE
VIEWSLOT_SET
VIEWSLOT_SET_ANIMATION
VIEWSLOT_STOP_ANIMATION if a View payload is relevant
SCROLLPANE_SET_CONTENT
```

Benchmark can begin with `RENDER` only.

Production migration happens only after V3 wins.

A single decoding core then serves every View-bearing N-API mutation.

Do not create transport-specific semantic differences per method.

---

# 74. Still one N-API mutation per semantic operation

Do not implement:

```text
send definitions
N-API returns handles
second N-API call performs render/history mutation
```

The transaction must remain:

```text
definitions/patches + operation
→ one N-API call
→ one atomic semantic mutation
```

This preserves the PERF-6/PERF-7 direction and avoids intermediate handle chatter.

---

# 75. Exact known-root operations may use specialized methods

The only intentional exception to “all operations use a transaction buffer” is the all-known fast path.

Example:

```text
Tui.render known root
→ tuiPackedRenderRef(generation, ref)
```

This is still one semantic N-API mutation.

For History/ViewSlot etc. add equivalent small-ref methods only if benchmarks show the generic tiny operation packet matters.

Do not explode the API into dozens of premature fast-path methods.

---

# 76. Correctness oracle remains direct semantic parity

Packed V3 must produce exactly the same semantic/physical output as direct decoding.

Before performance benchmarking:

```text
all existing PERF-7v2 differential fixtures pass
all randomized direct-vs-packed fixtures pass
all new persistent sequence/patch fixtures pass
```

Do not weaken the oracle because the wire format became more complicated.

---

# 77. Add canonical IR differential tests

If the TS private IR is normalized toward Rust shape, test the lowering independently.

For every public View operation:

```text
old bridge semantic output
new canonical semantic output
```

must match.

Especially:

```text
decoration merging
explicit false style attributes
style states
bounds
wrap/alignment
clamp/contentMax
Grid placement
component identity
Diff line numbering/termination
```

The transport benchmark must not accidentally measure a semantic simplification bug.

---

# 78. PersistentSeq unit-test matrix

Mandatory deterministic tests:

```text
empty
1 item
31 items
32 items
33 items
1,024 items
1,025 items
10,000 items
100,000 items
```

Operations:

```text
get every boundary index
append
set first/middle/last
insert first/middle/last
remove first/middle/last
split
concat
splice
iterate
clone/share
```

Compare logical result to a plain Array/Vec oracle.

---

# 79. Prove path-copy counts

Instrument sequence allocations.

For `set()` on 100,000 elements with B=32:

```text
new leaf count     ~= 1
new branch count   ~= tree depth
stable chunks      overwhelmingly pointer-identical
```

Assert an upper bound derived from tree height.

Do not merely assert output equality.

---

# 80. Prove Rust does not flatten persistent sequences

Add counters:

```text
persistent_seq_flatten_calls
persistent_seq_nodes_allocated
persistent_seq_leaf_clones
persistent_seq_branch_clones
persistent_seq_items_iterated_during_patch
```

For one-child replacement:

```text
persistent_seq_flatten_calls == 0
items_iterated_during_patch = O(log N), not N
```

Layout may later iterate N children because the frame genuinely needs layout; the **commit transport/reconstruction phase** must not.

Separate those timings.

---

# 81. Packed V3 JS counters

Add:

```text
packed_v3_compile_objects_visited
packed_v3_full_view_defs
packed_v3_patch_view_defs
packed_v3_seq_leaf_defs
packed_v3_seq_branch_defs
packed_v3_persistent_refs
packed_v3_local_refs
packed_v3_words_used
packed_v3_bytes_used
packed_v3_word_buffer_grows
packed_v3_byte_buffer_grows
packed_v3_exact_ref_fast_hits
packed_v3_lineage_steps
packed_v3_patch_chains_collapsed
packed_v3_cache_resyncs
packed_v3_cold_retries
packed_v3_string_count
packed_v3_utf8_bytes
```

If S2 wins, substitute the appropriate string counters.

---

# 82. Native V3 counters

Add:

```text
napi_v3_transactions
napi_v3_exact_ref_calls
napi_v3_persistent_ref_upgrades
napi_v3_persistent_ref_misses
napi_v3_local_ref_resolves
napi_v3_full_views_built
napi_v3_views_patched
napi_v3_seq_nodes_built
napi_v3_seq_nodes_reused
napi_v3_cache_lock_acquisitions
napi_v3_cache_publications
napi_v3_words_read
napi_v3_bytes_read
napi_v3_utf8_validations
napi_v3_host_mutations
```

The benchmark result must explain **why** a candidate wins.

---

# 83. Exact-identity acceptance test

Warm one root.

Run 100,000 exact submissions.

Required steady trace:

```text
TS:
    root meta published
    exact-ref fast path

wire:
    none

native:
    one generation check
    one PackedRef slot lookup
    one WeakView upgrade
    host dirty/commit
```

Required counters after warmup:

```text
full defs                  0
patch defs                 0
words used                 0
bytes used                 0
transaction buffer grows   0
exact ref fast hits        == operation count
```

Packed V3 exact commit median must be statistically neutral or faster than direct.

---

# 84. Narrow shared-path acceptance test

Shape:

```text
R0
└── A0
    └── B0
        ├── stable S
        └── X0

R1
└── A1
    └── B1
        ├── same S
        └── X1
```

Expected compile work:

```text
new X
changed B/A/R
stable S persistent ref
```

No descendant of S visited.

No stable subtree strings encoded.

---

# 85. Wide-parent one-edit acceptance test

Warm a 100,000-child Column.

Replace exactly one child through the retained sequence API.

Required transport structural work:

```text
1 new child View
~O(log_32 100000) seq chunks
1 parent patch/full definition
```

Required invariance:

```text
encoder object visits
wire words
native objects built
```

must remain approximately flat as width scales:

```text
2,048
10,000
100,000
```

The values may grow with tree height, not linearly with child count.

---

# 86. Wide-parent insertion/removal acceptance test

Warm 100,000 children.

Perform:

```text
insert one at 10%
insert one at 50%
insert one at 90%
remove one at same positions
splice 8 items
```

Assert:

```text
no full flat child-copy
no full child REF list emitted
no full Rust child Vec allocation
```

Measure sequence rebalance counts.

---

# 87. Decoration patch acceptance test

Warm:

```text
Text("x").padding(1).foreground("cyan")
```

Then change only:

```text
maxWidth
```

Expected:

```text
PATCH_VIEW
base ref
MAX_WIDTH field only
```

No text bytes.

No span traversal.

No child View reconstruction.

---

# 88. Text-local patch acceptance test

Warm a Text View with a large span payload.

Change only:

```text
wrap
alignment
```

Expected:

```text
small PATCH_VIEW
same native span storage Arc/persistent root
```

Assert pointer sharing in Rust.

Transport cost must be independent of text byte length.

---

# 89. Weak-cache expiry acceptance test

Procedure:

```text
1. packed-render root A
2. JS metadata marks generation G published
3. expire/reset native packed weak cache
4. exact-render A again
```

Expected:

```text
exact ref call
→ cache miss

JS:
    generation G+1
    allocator reset
    lazily assign new-generation refs to current closure
    one full cold closure

native:
    reset packed slots
    reconstruct current A closure
    commit host once
```

Assert:

```text
one recovery
one host mutation
no infinite retry
```

---

# 90. Cold-closure protocol test

Construct a tree containing:

```text
DAG duplicate children
PersistentSeq branches
Text
Grid
Decoration
Diff
Component
```

Cold transaction must contain:

```text
all required definitions exactly once
only LocalDefIndex dependencies
no persistent refs
```

Native must succeed from an empty packed slot table.

Any persistent dependency under `COLD_CLOSURE` is a test failure.

---

# 91. Malformed V3 tests

Reject:

```text
bad magic
bad protocol version
bad schema version
bad generation
used_words out of range
used_bytes out of range
unknown record tag
record length underflow/overflow
duplicate persistent ref in same txn
persistent ref 0
local ref forward reference
local ref index out of range
wrong object kind for ref
invalid NodeId high bits
NodeId 0
invalid u16 fields
invalid UTF-8 range
invalid UTF-8 bytes
invalid PersistentSeq cumulative sizes
invalid PersistentSeq height
branch > 32 children
leaf > 32 items
PATCH kind mismatch
PATCH invalid field mask
cold closure containing persistent ref
```

Host state must remain unchanged after every rejected transaction.

---

# 92. NodeId tests remain mandatory

Continue exact cases:

```text
1
2^32 - 1
2^32
2^32 + 1
2^53 - 1
```

Reject:

```text
0
negative
fractional
2^53
NaN
Infinity
```

PackedRef width optimizations must never regress semantic NodeId correctness.

---

# 93. PackedRef generation tests

Test:

```text
ref 1
ref 0x7fff_fffe
wrong generation
reset generation
same numeric ref reused in new generation
old generation access
```

An old `(generation, ref)` pair must never resolve to a new semantic object.

That is the ABA invariant.

---

# 94. Memory/lifetime tests

Prove:

```text
PackedMeta does not strongly retain dead Views globally
packed native slots are weak
PersistentSeq sharing does not create cycles
scratch capacity follows high-water mark, not lifetime object count
transport generation reset releases old packed slot pages/references
```

Track:

```text
JS heap after GC-capable checkpoints
RSS
native ViewBridgeCache semantic entries
packed slot pages
live upgraded packed slots
PersistentSeq node counts
word scratch capacity
byte scratch capacity
```

---

# 95. Do not use FinalizationRegistry for correctness

GC finalizers may be used for diagnostics or opportunistic cleanup.

They must not be required for:

```text
PackedRef correctness
cache invalidation
host correctness
sequence correctness
```

Weak native expiry + generation recovery is the correctness mechanism.

---

# 96. Benchmark candidates

PERF-8 authoritative run should include three candidates:

```text
A = direct baseline
B = PERF-7v2 packed
C = Packed V3
```

A/B should be rerun from a clean reproducible baseline under the updated harness.

C includes all construction-time metadata costs.

Do not compare:

```text
A construction without metadata
vs
C commit only after hiding metadata construction outside the timer
```

Primary decision uses total required semantic construction + commit.

---

# 97. Benchmark metrics

Record separately:

```text
construction_ns
packed_metadata_ns       if separable without perturbation
transaction_compile_ns
napi_native_ns
commit_ns
forced_frame_ns
```

Primary transport metric:

```text
construction required by mode
+ transaction compile
+ N-API/native retained reconstruction
+ host dirty/commit
```

Call it:

```text
total_commit_ns
```

Forced frame remains secondary:

```text
total commit + advance/layout/paint
```

---

# 98. Benchmark mode matrix

Retain:

```text
COLD
FIRST_USE
IDENTICAL_IDENTITY
SHARED_PATH
SHARED_DEEP
REBUILT_EQUIVALENT
```

Replace ambiguous naming around wide sharing with two explicit modes:

```text
LARGE_SHARED_SUBTREE_CUTOFF
WIDE_PARENT_ONE_EDIT
```

Add:

```text
WIDE_PARENT_INSERT
WIDE_PARENT_REMOVE
TEXT_METADATA_PATCH
DECORATION_PATCH
```

---

# 99. Workload matrix

Retain full-schema families:

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

Add targeted retained workloads:

```text
wide_column_one_edit
wide_row_one_edit
wide_grid_cell_edit
long_text_wrap_only
long_text_one_span_edit
large_diff_one_hunk_edit
large_decoration_only_change
```

---

# 100. Realistic trace is mandatory this time

A mode matrix alone is not a production decision.

Add a declared synthetic trace if production telemetry is still unavailable.

Example only:

```text
1 initial cold tree
then 10,000 operations:

65% narrow retained-path updates
15% exact identity
8% decoration/text metadata patches
5% wide-list small edits
5% rebuilt small sections
2% large replacements
```

The exact mix must be clearly labeled synthetic.

If real instrumentation can provide update-shape frequencies, use it instead.

Report:

```text
total elapsed commit time
total JS CPU
total native CPU
allocations/resyncs
p50/p95 per operation class
```

---

# 101. Add update-shape telemetry hooks before guessing too much

A short diagnostic build can count production-style operations without recording content:

```text
root identity same?
changed path depth
immediate changed-parent width
number of changed siblings
text byte sizes
span counts
diff hunk counts
grid cell counts
```

No user text needs to be recorded.

If available, this tells us whether persistent wide sequences are a huge production win or an insurance mechanism for pathological trees.

Do not block core V3 on telemetry, but use it to prioritize specialization work.

---

# 102. Warmup and samples

Authoritative:

```text
warmup >= 50 for tiny hot paths
measured >= 1,000 for IDENTICAL and tiny PATCH cases
measured >= 200 for expensive large cases
```

p99:

```text
>= 1,000 observations
```

Otherwise mark p99 informational.

Tiny sub-microsecond/microsecond paths need more samples than PERF-7v2 because scheduler noise becomes a large percentage.

---

# 103. Candidate order

Continue deterministic alternation:

```text
A B C
C B A
B C A
A C B
...
```

Each candidate gets independent warmed state.

No candidate may populate another candidate's semantic/packed cache.

Use explicit environment/cache reset isolation where needed.

---

# 104. Statistical reporting

Keep raw samples.

Report:

```text
median
p95
p99 when authoritative
bootstrap CI for median/p95
relative change with CI
```

For paired alternating samples, also compute paired bootstrap deltas where practical.

Do not call a 2% change meaningful when CI spans both sides of zero.

---

# 105. Tail latency matters more after V3

V2 sometimes shows packed p95 spikes despite good medians.

V3 must explain and reduce variance.

For each outlier bucket, retain:

```text
buffer growth?
GC?
cache resync?
native cache prune?
PersistentSeq rebalance?
string arena growth?
component registration?
OS scheduling?
```

Add counters before concluding tails are “noise.”

---

# 106. Cache pruning must not create periodic latency cliffs

Current semantic cache prunes periodically based on map size.

Packed V3 paged slots should avoid an O(total refs) sweep on an arbitrary hot commit.

Options:

```text
incremental page-local weak cleanup
lazy clear on failed upgrade
bounded maintenance budget per commit
full cleanup outside hot path
```

Do not run a large global `retain()` sweep every N operations in the latency-critical path.

Keep semantic NodeId cache cleanup similarly bounded if profiling identifies it as a tail source.

---

# 107. Protocol code generation

Extend the canonical bridge schema generator to emit:

```text
record tags
operation tags
patch kinds
patch masks
header flags
WireRef masks
PersistentSeq constants
field discriminants
```

Do not hand-copy numeric values into TS and Rust.

The schema source should be machine-parsed JSON/JSON5 or another canonical format.

The serde-based build-script cleanup already made the correct move here.

---

# 108. Prefer generated decoder tables only where they improve clarity/perf

A giant generic reflection decoder is not the goal.

The hot decoder should remain explicit and branch-predictable.

Code generation may create:

```text
constants
small field readers
validation tables
```

but do not turn a handful of View kinds into a generic runtime schema interpreter.

SBE's useful lesson is fixed schema + direct buffer access, not “adopt another framework.”

---

# 109. Do not adopt FlatBuffers/Cap'n Proto/SBE as the View protocol

Their design principles are relevant:

```text
in-place buffer access
native scalar representation
avoid intermediate objects
allocation-free/flyweight decoding
word-aligned predictable layouts
```

But Iyon has unusually strong domain-specific information:

```text
semantic NodeId
weak retained View cache
structural sharing
custom View kinds
patch lineage
persistent sequence refs
one-process N-API boundary
```

A custom protocol can exploit these better than a general serializer.

Borrow the principles, not the entire stack.

---

# 110. Production decision gate

Packed V3 may replace the current private transport only after all correctness gates pass and the following performance conditions hold.

## Exact identity

```text
no statistically credible regression vs direct
```

Target:

```text
Packed V3 <= direct median and p95 within noise
```

## Narrow retained updates

```text
no material regression vs best of direct/V2
```

Target:

```text
>= 5% improvement over V2 where transaction compilation remains visible
or statistical neutrality if already dominated by host work
```

## Wide one-edit

Required algorithmic proof:

```text
structural work scales O(log N), not O(N)
```

and large-width latency must materially beat V2.

## Cold/rebuilt

Do not sacrifice the reason packed exists.

Target:

```text
retain or improve V2's large-tree advantage
```

A >10% regression from V2 on large cold/rebuilt requires a compelling trace-level reason.

## Realistic trace

Primary production gate:

```text
Packed V3 total commit time >= 10% better than V2
AND
>= 10% better than direct
```

Strong candidate:

```text
>= 15% trace improvement
```

If trace improvement is <5% after all this complexity:

```text
reject V3 complexity
keep V2/direct winner
```

---

# 111. Memory decision gate

Reject V3 if performance comes from unbounded retention.

Required:

```text
packed slot table growth bounded by generation/high-water policy
weak Views actually expire
PersistentSeq nodes are released when semantic graphs die
scratch arenas bounded by high-water transaction size
string slabs do not accumulate indefinitely
```

A single reusable high-water scratch allocation is acceptable.

One retained entry for every View ever created is not.

---

# 112. Implementation sequence

Do not ask one agent to implement the entire architecture in one commit.

Use these tranches.

## PERF-8.0 — freeze/reproduce the baseline

```text
rerun A/B from clean 84a7d11 baseline
fix benchmark source fingerprinting
add WIDE_PARENT_ONE_EDIT control benchmark
add exact-ref sample count increase
no V3 implementation yet
```

Expected result:

```text
prove V2 O(width) behavior on an actually wide changed parent
reconfirm A/B baseline
```

## PERF-8.1 — dense refs + flat protocol + exact-ref fast path

```text
PackedRef + generation
paged native packed slots
WireRef local/persistent encoding
flat topological definitions
specialized exact-root call
staged native decode
batched cache publication
no PersistentSeq yet
```

This isolates constant/protocol improvements.

## PERF-8.2 — construction-time canonical metadata

```text
PackedMeta created with immutable semantic node
precompute NodeId split
precompute style/decoration/border tags
transaction epoch marks
lineage metadata
PATCH common fields
PATCH text metadata
remove commit-time semantic Object.entries/Map work
```

Benchmark construction + commit, not commit alone.

## PERF-8.3 — PersistentSeq end to end

```text
TS persistent/transient sequence
Rust PersistentSeq
Column/Row migration
aggregate flags
sequence wire records
axis PATCH
wide edit/insert/remove tests
layout consumes PersistentSeq directly
```

This is the tranche that earns real structural-delta semantics.

## PERF-8.4 — Grid and retained payload specialization

```text
Grid persistent collections
large Text span path if justified
Diff retained hunk/line path if still a regression
```

Prioritize with benchmark/telemetry.

## PERF-8.5 — string lane shootout

```text
S1 reusable UTF-8 arena
S2 corrected move-once string lane
choose measured winner
optional slab-retention experiment only if still dominant
```

Do not silently mix string architecture changes into the sequence benchmark.

## PERF-8.6 — authoritative benchmark and decision

```text
full workload/mode matrix
wide structural matrix
realistic trace
memory/lifetime run
tail analysis
write evidence-backed decision
```

## PERF-8.7 — only if V3 wins

```text
migrate all View-bearing N-API boundaries
make V3 the private production path
remove benchmark-only V2 path
remove direct production bridge after differential soak period
remove candidate flags
```

---

# 113. Suggested commits

```text
bench(tui): establish PERF-8 packed delta baseline
feat(runtime): add dense retained packed references
feat(native): decode flat packed graph transactions
perf(runtime): precompute packed View metadata at construction
feat(tui): add persistent retained child sequences
perf(native): patch retained Views without flattening children
perf(runtime): optimize packed string transport
bench(tui): complete PERF-8 retained delta decision
```

Only if it wins:

```text
perf(tui): adopt retained packed graph transport
```

---

# 114. Production migration inventory

If V3 wins, search the repository again at implementation time.

At minimum inventory:

```text
Tui.render
History.push
History.freeze
ViewSlot initial value
ViewSlot.setView
ViewSlot.setAnimation
ViewSlot.stopAnimation
ScrollPane initial content
ScrollPane.setContent
```

and every call site passing:

```text
BridgeViewNode
nodeForBridge()
View-bearing Object
```

Do not migrate only `render()`.

---

# 115. Keep differential direct decoding temporarily, not permanently

After V3 production adoption:

```text
direct decoder may remain behind test feature
```

for randomized parity testing.

After sufficient soak:

```text
remove it from production binary/path
```

Do not maintain two production View transports indefinitely.

The correctness surface is too large.

---

# 116. Banned shortcut: post-hoc JSON/object diff

Reject:

```text
JSON.stringify(old)
JSON.stringify(new)
diff objects
```

Reject any generic recursive property diff.

It destroys the purpose of identity-preserving retained construction.

---

# 117. Banned shortcut: hash every subtree to discover sharing

Exact object/PackedRef identity already exists.

Do not compute content hashes merely to rediscover stable subtrees.

Hashes are allowed as diagnostics or fast inequality checks, never as a replacement for retained identity.

---

# 118. Banned shortcut: PATCH a flat Vec

Reject:

```rust
old.children.to_vec()
replace one
into Arc<[T]>
```

while claiming O(changes) transport.

That is O(width) native reconstruction.

---

# 119. Banned shortcut: persistent sequence that layout immediately flattens

Reject:

```rust
let children = seq.iter().cloned().collect::<Vec<_>>();
layout(children)
```

in the normal retained path.

The retained representation must be consumed directly.

---

# 120. Banned shortcut: PATCH without explicit lineage

Reject:

```text
new node “looks similar” to old node
→ assume old node is base
```

PATCH requires exact construction lineage or an explicit persistent sequence operation.

No heuristics.

---

# 121. Banned shortcut: strong native cache to avoid recovery

Do not turn:

```text
PackedRef -> WeakView
```

into:

```text
PackedRef -> permanently strong View
```

just to make cache misses disappear.

Weak expiry + cold generation resync is the correct model.

---

# 122. Banned shortcut: hide metadata work outside benchmark timing

If V3 creates PackedMeta, persistent sequence chunks, or encoded UTF-8 during View construction, that CPU belongs to the candidate.

Primary benchmark includes it.

Moving work earlier is valuable only if total work falls or overlaps useful construction work.

---

# 123. Banned shortcut: one native call per View constructor

Do not solve encoding by doing:

```text
View.text()
→ native call

View.vertical()
→ native call

padding()
→ native call
```

The whole point of packed transport is batching the boundary.

One construction call per node would exchange serializer cost for boundary-call cost.

---

# 124. Banned shortcut: permanent Bun FFI dependency without explicit decision

`bun:ffi` is currently experimental.

Do not quietly replace stable Node-API with FFI because one microbenchmark looks good.

Any such production choice requires a separate explicit risk/compatibility decision.

---

# 125. Banned shortcut: benchmark only tiny two-child shared roots

A retained delta architecture must prove:

```text
wide parent
one edit
constant/logarithmic structural work
```

The old `shared subtree + changed leaf` shape remains useful but is insufficient.

---

# 126. Expected steady-state trace — exact identity

```text
TS:
root PackedMeta
publishedGeneration == currentGeneration

call:
tuiPackedRenderRef(generation, root.ref)

Rust:
check generation
paged slot lookup
WeakView upgrade
host.render(view)
dirty
return
```

No words.

No bytes.

No parser.

No NodeId.

---

# 127. Expected steady-state trace — narrow changed path

```text
R1
└── A1
    ├── stable S
    └── X1
```

Assume S published, R/A/X new.

Compile:

```text
DEF X1                local 0
PATCH/DEF A1          local 1
    child stable S    persistent ref
    child X1          local 0
PATCH/DEF R1          local 2
operation RENDER      local 2
```

Native builds only local 0..2.

Stable S is upgraded, not decoded.

---

# 128. Expected steady-state trace — 100,000-child one-edit

Existing:

```text
Column C0
    child sequence Q0
```

Replace child 87,210.

PersistentSeq depth approximately 4 for branch factor 32.

New structure:

```text
X1
Q leaf L1
Q branch B1
Q branch B2
Q branch B3
Q root  Q1
C1
```

Transaction approximately:

```text
DEF X1
DEF_SEQ_LEAF L1
DEF_SEQ_BRANCH B1
DEF_SEQ_BRANCH B2
DEF_SEQ_BRANCH B3
PATCH C1 FROM C0: child_seq = Q1
RENDER C1
```

Stable ~99,999 child references are not emitted.

Stable sequence chunks are not rebuilt.

---

# 129. Expected trace — wide insertion

Insert 1 child at index 50,000.

RRB-style sequence performs:

```text
split/rebalance only boundary path/chunks
reuse untouched left/right interior subtrees
```

Wire contains only:

```text
new child
new/rebalanced sequence chunks
new parent patch
```

No 100,001-entry child list packet.

---

# 130. Expected trace — wrap-only Text update

Existing:

```text
T0 spans = 1 MB logical text/span payload
wrap = word
```

New:

```text
T1 spans = same retained span storage
wrap = no-wrap
```

Wire:

```text
PATCH T1 FROM T0
    TEXT_WRAP = NO_WRAP
```

No text bytes.

Native:

```text
clone ViewNode shallowly
Arc-share Text spans
change wrap
new View identity
```

Cost independent of 1 MB text size.

---

# 131. Expected trace — weak-cache resurrection

```text
JS still owns semantic root R
PackedMeta says generation G published
native weak R expired

exact-ref call:
    ref R

native:
    miss

JS:
    generation = G + 1
    reset PackedRef allocator
    assign new-generation refs while compiling R closure
    cold compile complete R closure

wire:
    RESET_GENERATION
    COLD_CLOSURE
    all definitions local

native:
    clear packed slots
    rebuild closure
    publish weak slots
    render once

JS:
    mark emitted metas generation G+1
```

Correctness is restored without strong all-time retention.

---

# 132. Expected trace — cold 10,000-node tree

Cold remains O(total input), but should be cheaper than V2 because:

```text
canonical metadata already computed during construction
flat records
one-word child refs
no nested REF records
no per-node cache locks
no recursive record boundary backpatching
fewer JS semantic property conversions
corrected string ownership
native canonical constructors
```

Packed V3 must preserve V2's cold advantage.

---

# 133. Research rationale

This design borrows principles, not implementations.

## React Native Fabric

Useful ideas:

```text
immutable structurally shared retained tree
changed path cloning
native retained tree comparison/mutation stage
direct native architecture to reduce serialization
```

Important difference:

```text
Iyon is optimizing the JS → native semantic boundary itself
```

so a native Fabric-style diff cannot replace transport unless native already receives enough retained structure to construct the new snapshot.

Packed V3 sends that retained structure directly.

## Flutter

Useful idea:

```text
explicit dirty/reused retained state beats global diffing
```

Flutter specifically avoids a whole-tree diff and reconciles locally.

Packed V3 likewise uses explicit lineage and persistent sequence operations instead of scanning arbitrary trees.

## RRB Trees

Useful idea:

```text
wide immutable vectors
structural sharing
fast indexed update
split/concat/insert without rebuilding the whole vector
```

This is the algorithmic basis for removing O(width) child-array rebuilds.

## SBE / Cap'n Proto / similar zero-copy codecs

Useful principles:

```text
fixed schema
native scalar layout
word-aligned predictable records
no unnecessary intermediate representation
allocation/copy avoidance
```

Packed V3 remains custom because retained identity/patch semantics are domain-specific.

## rsync

Useful contrast:

```text
content matching is needed when exact retained identity is unavailable
```

Iyon already has exact identity, so checksum-based block discovery is unnecessary overhead for the semantic View graph.

---

# 134. Required final PERF-8 result document

The implementation agent must report:

```text
Exact candidate SHAs
Clean/dirty status and artifact hashes
Protocol version
Bridge schema version
PersistentSeq branch factor and thresholds
String lane selected and why

Correctness:
    direct parity
    randomized seeds
    PersistentSeq oracle status
    weak-cache recovery
    NodeId width
    PackedRef generation/ABA tests
    malformed transaction tests

Per mode/workload/candidate:
    construction median/p95
    compile median/p95
    native median/p95
    total commit median/p95
    p99 where authoritative
    confidence intervals
    relative deltas

Structural counters:
    objects visited
    full defs
    patches
    seq leaf/branch defs
    persistent refs
    local refs
    words
    bytes
    buffer grows

Native counters:
    ref upgrades/misses
    views built/patched
    seq nodes built/reused
    cache lock acquisitions
    cache publications

Wide edit scaling:
    width 2k/10k/100k
    structural work counts
    latency scaling

Memory:
    JS heap
    RSS
    packed slot pages
    native semantic cache
    PersistentSeq live nodes
    scratch high-water

Synthetic/real trace:
    total elapsed commit time
    total JS CPU
    total native CPU
    operation-class distribution

Final decision:
    direct / V2 / V3
    exact decision rule satisfied or failed
```

No summary sentence such as:

> “Delta transport is faster.”

is acceptable without proving that:

```text
wide one-edit work is sublinear
exact root no longer parses a packet
native does not flatten retained child sequences
construction metadata cost is included
weak-cache expiry remains recoverable
```

---

# 135. Mandatory acceptance checklist

```text
[ ] baseline A/B rerun from clean source
[ ] result records identify exact measured source
[ ] WIDE_PARENT_ONE_EDIT added
[ ] PackedRef is dense <=31-bit and distinct from NodeId
[ ] generation prevents PackedRef ABA
[ ] exact-root path sends no transaction buffer
[ ] PackedMeta is built during semantic construction
[ ] primary benchmark includes PackedMeta construction cost
[ ] commit no longer computes style/decoration semantic masks
[ ] flat topological V3 wire implemented
[ ] child dependencies are one-word WireRef
[ ] local refs cannot point forward
[ ] native decode stages before host mutation
[ ] cache locks O(1) per transaction, not per ref
[ ] semantic cache remains weak
[ ] packed slots remain weak
[ ] one cache-miss recovery, then hard fail
[ ] cold closure contains no persistent refs
[ ] TS PersistentSeq implemented
[ ] Rust PersistentSeq implemented
[ ] Column no longer requires flat Arc<[ColumnChild]> in V3 path
[ ] Row no longer requires flat Arc<[RowChild]> in V3 path
[ ] persistent sequence update is O(log N)
[ ] cached View flags update O(log N), not O(N)
[ ] layout consumes PersistentSeq without flattening
[ ] wide edit structural counters flat/logarithmic vs width
[ ] insertion/removal do not rebuild full child list
[ ] PATCH creates new immutable View, never mutates base
[ ] wrap/alignment patch does not retransmit spans
[ ] decoration-only patch does not retransmit child payload
[ ] string S1/S2 benchmark completed
[ ] no duplicate JS→temporary Rust→final Rust string copy in chosen lane
[ ] full-schema differential tests pass
[ ] randomized differential tests pass
[ ] NodeId >2^32 and MAX_SAFE pass
[ ] PackedRef generation tests pass
[ ] malformed V3 transactions leave host unchanged
[ ] exact identity >=1,000 authoritative samples
[ ] full workload matrix run
[ ] realistic trace run
[ ] memory/lifetime run
[ ] tail-latency outliers explained by counters where possible
```

---

# 136. Bottom line

PERF-7v2 optimized **how a tree is serialized**.

PERF-8 should optimize **whether the unchanged tree is serialized or rebuilt at all**.

The architectural endpoint should be:

```text
TS immutable retained graph
        ║
        ║ exact identity / lineage
        ║
        ▼
compact new-object journal
+ persistent-sequence path copies
+ immutable View patches
        ║
        ║ one packed native mutation
        ▼
Rust immutable retained graph
```

The important algorithmic shift is this:

```text
V2:
    identity cuts off descendants

V3:
    identity cuts off descendants
    AND persistent sequence identity cuts off stable siblings
    AND lineage avoids resending unchanged payload inside a changed View
    AND exact root bypasses the transaction format entirely
```

That is the design I would pursue if the goal is the fastest technically coherent packed transport rather than the smallest next patch.

---

# Source appendix

## Iyon baseline

- PERF-7v2 baseline commit:  
  https://github.com/alexykn/iyon-tui/commit/84a7d117c777fbd5c2f0d5d072e63769be842e7c
- Packed V2 TypeScript encoder:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/src/tui/packed.ts
- Packed V2 Rust decoder:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/crates/iyon-native/src/tui/packed.rs
- TypeScript retained bridge IR:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/src/tui/ir.ts
- TypeScript View construction:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/src/tui/values/view.ts
- Rust retained View IR:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/crates/iyon-tui/src/presentation/ir.rs
- PERF-7v2 harness:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/bench/tui_performance.ts
- PERF-7v2 raw samples:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/bench/PERF-7v2-results.jsonl
- PERF-7v2 packed tests:  
  https://github.com/alexykn/iyon-tui/blob/84a7d117c777fbd5c2f0d5d072e63769be842e7c/packages/iyon-runtime/tests/tui_packed.test.ts

## External research

- React Native — Render, Commit, and Mount:  
  https://reactnative.dev/architecture/render-pipeline
- React Native — New Architecture / direct JS-native communication:  
  https://reactnative.dev/blog/2024/10/23/the-new-architecture-is-here
- React Native — Cross Platform Implementation / mount mutations over JNI:  
  https://reactnative.dev/architecture/xplat-implementation
- Flutter — Inside Flutter, sublinear build and local reconciliation:  
  https://docs.flutter.dev/resources/inside-flutter
- Bagwell & Rompf — RRB-Trees: Efficient Immutable Vectors:  
  https://infoscience.epfl.ch/record/169879/files/RMTrees.pdf
- Tridgell & Mackerras — The rsync algorithm:  
  https://rsync.samba.org/tech_report/
- Simple Binary Encoding — Design Principles:  
  https://github.com/aeron-io/simple-binary-encoding/wiki/Design-Principles
- Cap'n Proto — Encoding specification:  
  https://capnproto.org/encoding.html
- Node-API documentation:  
  https://nodejs.org/api/n-api.html
- Bun FFI documentation and production warning:  
  https://bun.sh/docs/runtime/ffi
- Bun Node-API documentation:  
  https://bun.sh/docs/runtime/node-api
