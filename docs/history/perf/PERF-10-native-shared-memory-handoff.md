# PERF-10 — Native shared-memory retained fast path

**Status:** implemented prototype; counter-free timing and lifecycle checks passed; normal-workload adoption gate fails pending redesign
**Supersedes:** the earlier interpretation of PERF-10 as primarily a UTF-8/string-lane experiment
**Keeps:** PERF-7v2 optimizations, PERF-8 retained graph / persistent-sequence work, and V3 as the bulk/cold/recovery transport
**Runtime target:** pinned Bun + `bun:ffi`
**Primary goal:** make normal retained updates faster than V2 by removing generic transport work from the warm path, not by further tuning the V3 serializer.

---

## 1. Executive decision

The capped PERF-10 benchmark changes the priority.

The current V3 implementation is structurally successful but still loses on the normal warm path:

- V3 UTF-8 lane: about **21.3% slower than V2** across the capped normal matrix.
- V3 move-once string lane: about **20.1% slower than V2**.
- `SHARED_PATH` and `LARGE_SHARED_SUBTREE_CUTOFF`: roughly **30–35% slower than V2**.
- Exact identity is a genuine success:
  - UTF-8 lane: V3 about **666 ns** vs V2 about **917 ns**.
  - move-once strings: V3 about **625 ns** vs V2 about **876 ns**.
- String-lane tuning is not the limiting factor:
  - move-once strings make encoding about **17.6% faster**,
  - but total V3 latency changes by only about **0.1%**.
- The synthetic trace still favors V2:
  - UTF-8 V3 about **3.4% slower** than V2,
  - move-once V3 about **26% slower** in that trace.

Therefore:

> **The remaining problem is not tree traversal and not primarily string encoding. The remaining problem is warm-path transport, decoding, synchronization, staging, and publication overhead.**

The recommended PERF-10 architecture is:

```text
immutable JS retained graph
    |
    | PERF-8 lineage / persistent sequence knowledge
    v
small fixed-width FastOp batch
    |
    | JS writes directly into native-owned shared pages
    | no per-commit allocation
    | no typed-array copy
    | no generic record parser
    v
pinned Bun bun:ffi C ABI
    |
    | one small synchronous call
    v
thread-affine Rust FastSession
    |
    | direct retained-ref resolution
    | direct View / PersistentSeq patch construction
    | no global cache lookup
    | no mutex in the JS-thread fast path
    | no HashSet duplicate tracking
    | no generic V3 transaction staging
    v
native retained View graph
    |
    v
host.render()
```

Cold, rebuilt, large replacement, cache recovery, and any unsupported mutation continue to use Packed V3.

This is a **hybrid** architecture, not a V3 replacement.

---

## 2. PERF-8.5 vs PERF-10

The UTF-8 arena / move-once string comparison should now be considered **PERF-8.5**.

The capped result is enough to make that distinction:

- S2 is cheaper to encode.
- S2 does not materially improve total latency.
- The choice of string lane does not explain the 20–35% retained-update regression.
- V3 exact identity already wins even with either lane.

PERF-8.5 should keep the string-lane work available as evidence and fallback tuning, but PERF-10 should not spend another tranche trying to win general performance by changing `string[]` vs UTF-8 packet representation.

For the shared-memory design below, UTF-8 becomes useful again for a different reason:

> It lets JavaScript encode **directly into native-owned retained memory**, so Rust can retain the same bytes without a second transport copy.

That is architecturally different from the old S1 packet arena.

---

## 3. What must not be lost

PERF-10 must preserve all important gains and invariants from PERF-7v2 and PERF-8.

### 3.1 Preserve semantic identity

Keep:

- process/environment semantic `NodeId`,
- dense generation-scoped `PackedRef`,
- exact immutable View identity,
- lineage metadata,
- persistent-sequence identity,
- cache-miss recovery semantics.

Do not replace semantic identity with hashes.

Do not derive equality from transport addresses.

### 3.2 Preserve structural cutoff

A warm update must still scale with the changed path.

For a huge stable subtree:

```text
new root
  -> changed ancestor
     -> stable 100,000-node subtree
```

the fast path must not traverse the stable subtree.

### 3.3 Preserve PersistentSeq

Wide parent edits must remain path-copy operations.

Do not flatten a `PersistentSeq` into a normal array simply because the new native boundary is cheaper.

### 3.4 Preserve V3 bulk performance

V3 is currently very promising for:

- COLD,
- FIRST_USE,
- REBUILT_EQUIVALENT,
- large full definitions,
- cache resynchronization.

PERF-10 must not force those operations through dozens or hundreds of tiny FFI patch calls.

### 3.5 Preserve exact-identity behavior

The existing V3 exact-ref path is already clearly faster than V2.

PERF-10 may improve it further, but must not regress it into a packet or generic batch.

---

## 4. OpenTUI lessons worth copying

OpenTUI is a useful real-world precedent because its current architecture is already close to the direction PERF-10 wants:

- native core written in Zig,
- C ABI,
- Bun FFI bindings,
- stable native handles,
- native-owned render buffers,
- direct pointer/buffer arguments,
- explicit lifetime rules for retained pointers,
- zero-copy native memory exposed to JavaScript.

The important lesson is **not** “OpenTUI uses FFI, therefore FFI is always faster.”

The useful lessons are architectural.

### 4.1 Keep the C ABI explicit and boring

OpenTUI exposes a broad C ABI using fixed-width scalar types and pointers.

PERF-10 should do the same, but with a much smaller surface.

Use:

```text
u8
u16
u32
i32
u64 only when genuinely necessary
opaque handles
pointer + byte length
```

Avoid:

```text
Rust layout types
Vec<T>
String
Option<T> across FFI
callbacks on the hot path
cstring returns
exceptions crossing native
```

### 4.2 Prefer stable handles over raw object pointers

OpenTUI commonly exposes native objects as numeric handles and resolves them in native code.

That is a strong model for Iyon.

Recommended default:

```rust
type FastSessionHandle = u32;
```

with a thread-local or environment-local dense handle table.

Benchmark a raw `*mut FastSession` variant, but only choose it if the difference is material.

A one-index lookup is usually a good trade for:

- no JS-visible dangling Rust object pointer,
- generation/ABA protection,
- deterministic teardown,
- easier debug validation.

### 4.3 Borrow transient buffers directly

OpenTUI's own FFI guidance distinguishes:

- transient synchronous pointer arguments: pass the owning typed array/view directly,
- retained pointers: keep the backing memory alive for the complete native lifetime.

PERF-10 should use the same rule.

For the baseline shared-memory design, we can do even better:

- native allocates the page,
- JS receives a zero-copy view over native memory,
- JS never owns the underlying allocation,
- native controls final lifetime.

### 4.4 Copy the NativeSpanFeed ownership pattern

OpenTUI's `NativeSpanFeed` explicitly describes itself as a **zero-copy wrapper over Zig memory**.

Its important pattern is:

```text
native owns chunk
    |
    v
JS receives ArrayBuffer alias to native pointer
    |
    v
JS uses borrowed slice
    |
    v
refcount / lifetime protocol keeps chunk alive
    |
    v
native reclaims only when consumers release
```

PERF-10 should apply the same idea in the opposite data-flow direction:

```text
native owns writable page
    |
    v
JS receives ArrayBuffer alias
    |
    v
JS writes FastOps / UTF-8 bytes directly into native memory
    |
    v
commit seals the relevant region
    |
    v
Rust reads or retains same bytes
    |
    v
page is reusable only when native lifetime rules permit
```

This is preferable to retaining a pointer into an ordinary JS-owned growable buffer.

### 4.5 Keep large render state native

OpenTUI's optimized buffers expose native buffer handles and native pointers rather than serializing the complete render buffer through JS every frame.

PERF-10 should follow the same principle:

> Once a semantic object is native-retained, future warm updates should refer to it by compact native identity and mutate through retained constructors, not resend a complete representation.

---

## 5. “Shared memory” should mean native-owned shared pages first

There are two different ideas often called shared memory.

### 5.1 JavaScript `SharedArrayBuffer`

```text
JS allocates SAB
Rust retains pointer into SAB
JS and native may access concurrently
Atomics coordinate
```

This is attractive for a true asynchronous producer/consumer ring.

It is **not** the recommended baseline.

Problems:

- native lifetime depends on a JS-owned object staying alive,
- the FFI contract becomes tightly coupled to Bun/JSC backing-store behavior,
- concurrent native and JS access introduces real memory-ordering obligations,
- Rust must not create data races with JS writes,
- page reuse becomes much harder,
- Bun's FFI is already experimental; adding a cross-language atomic-memory protocol increases the unsupported surface.

### 5.2 Native-owned memory aliased into JavaScript

```text
Rust allocates page
Bun toArrayBuffer(pointer, ...)
creates JS view over same bytes
JS writes directly into native allocation
Rust reads same bytes
```

This is the recommended PERF-10 baseline.

Advantages:

- zero transport copy,
- native controls allocation lifetime,
- no GC-owned backing pointer retained by Rust,
- JS sees a normal Uint8Array / Uint32Array,
- no per-commit buffer allocation,
- no per-commit pointer extraction if the session already knows page addresses,
- page sealing can make retained reads race-free,
- strongly precedented by OpenTUI's zero-copy native-memory views.

This is shared memory in the important performance sense: **one physical byte region is visible to JS and native code**.

It does not require ECMAScript `SharedArrayBuffer`.

---

## 6. What “zero-copy” can realistically mean

Do not set an impossible requirement.

JavaScript strings are not Rust UTF-8 strings.

A JavaScript `string` must still undergo a semantic encoding transformation before Rust can treat it as UTF-8.

The achievable goal is:

### 6.1 Numeric / structural data

Truly zero-copy at the transport boundary:

```text
JS writes u32 fields once
Rust reads those exact bytes
```

No packet copy.

No `Vec<u32>`.

No N-API conversion.

No second intermediate typed array.

### 6.2 String payload

Best possible:

```text
JS string
    |
    | TextEncoder.encodeInto
    v
native-owned UTF-8 page
    |
    | same bytes retained
    v
Rust SharedStr / RetainedStr
```

The encode itself is unavoidable.

The **post-encode copy** should be eliminated.

### 6.3 Retained graph

Persistent immutable updates still create new Rust semantic objects along the changed path.

That is not a transport copy.

Expected work:

```text
Arc clone
small ViewNode allocation
PersistentSeq path-copy
```

Those operations are the desired semantic update.

### 6.4 Terminal output

Eventually bytes must reach the terminal.

That is outside the PERF-10 transport goal.

---

## 7. Proposed native memory model

Create one `FastSession` per JS environment / runtime instance.

Conceptually:

```rust
struct FastSession {
    generation: u32,

    refs: PackedSlotTable,

    command: FastCommandPage,

    scratch: FastScratch,

    string_pages: StringPagePool,

    owner_thread: ThreadId,

    state: SessionState,
}
```

No `Arc<Mutex<FastSession>>` on the ordinary same-thread FFI path.

The session is explicitly thread-affine.

Debug builds assert thread affinity on every fast call.

---

## 8. Native-owned command page

Allocate one fixed-capacity native-owned command page at session creation.

The implemented baseline uses:

```text
256 KiB command page
128 KiB fixed-op region
128 KiB fixed metadata region
```

The exact size should be benchmarked, but the normal retained path should almost never grow it.

Layout:

```text
+---------------------------+
| FastControlV1             | 64 or 128 bytes
+---------------------------+
| FastOpV1[capacity]        |
+---------------------------+
```

Native returns:

```text
command_ptr
command_byte_len
```

once during bootstrap.

JS maps it once:

```ts
const commandBuffer = toArrayBuffer(commandPtr, 0, commandByteLength)
const commandWords = new Uint32Array(commandBuffer)
```

The view remains private inside `FastTransport`.

No per-commit:

```text
new Uint32Array
subarray()
ptr()
toArrayBuffer()
```

is required.

---

## 9. FastControlV1

Keep the control block fixed width and cache-line friendly.

Example:

```rust
#[repr(C, align(64))]
struct FastControlV1 {
    magic: u32,
    abi_version: u32,

    generation: u32,
    sequence: u32,

    op_count: u32,
    root_wire_ref: u32,

    byte_page_id: u32,
    byte_used: u32,

    flags: u32,

    status: i32,
    status_detail: u32,

    _reserved: [u32; 5],
}
```

Exact field count can change.

Important properties:

- fixed position,
- no variable header parsing,
- JS writes request fields,
- Rust writes status fields,
- `sequence` detects stale/replayed commits,
- `generation` matches retained-ref generation,
- reserved fields must be zero in debug/validation builds.

The FFI function can become:

```rust
extern "C" fn iyon_fast_commit_v1(
    session: FastSessionHandle,
) -> i32;
```

That is intentionally tiny.

All variable request data is already in the shared page.

---

## 10. FastOpV1

The warm path should not use V3's generic records.

Use one fixed-width operation representation.

Candidate:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
struct FastOpV1 {
    opcode_flags: u32,

    dst_ref: u32,
    base_ref: u32,

    node_id_lo: u32,
    node_id_hi: u32,

    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
}
```

40 bytes is acceptable if it removes parser complexity.

Do not optimize the struct down to 24 bytes until boundary cost is measured.

The important thing is:

```text
fixed offset
fixed width
direct native load
```

not a few saved words.

---

## 11. Fast op classes

PERF-10 only needs operations that cover retained lineage.

Initial opcodes:

```text
PATCH_TEXT_LAYOUT
PATCH_DECORATION
PATCH_AXIS
PATCH_GRID

SEQ_LEAF_REPLACE
SEQ_BRANCH_REPLACE

GRID_SEQ_LEAF_REPLACE
GRID_SEQ_BRANCH_REPLACE

DEF_SMALL_TEXT
DEF_SMALL_SPACER
DEF_SMALL_CONTAINER
DEF_SMALL_DECORATED

RENDER_ROOT
```

The exact set should follow the mutations emitted by the existing PERF-8 lineage system.

Do **not** reproduce every V3 full-schema decoder operation before benchmarking.

The fast lane is for common retained updates.

---

## 12. Local references without a generic decoder

A batch may need to construct:

```text
new sequence leaf
    ->
new sequence branch
    ->
new row
    ->
new decorated root
```

Do not publish each intermediate object separately.

Use topological operation order.

Reference encoding:

```text
0x00000001 .. 0x7fffffff = persistent PackedRef
0x80000000 | op_index     = result of earlier FastOp
```

Rules:

- local ref may only reference `op_index < current_op_index`,
- destination persistent refs are strictly increasing within the batch,
- no duplicate destination-ref `HashSet` required,
- no generic definition table required.

Native maintains a reused staging vector:

```rust
session.staged.clear();
```

and pushes one result per op.

Local resolution is:

```rust
staged[index]
```

Persistent resolution is:

```rust
session.refs.get(ref)
```

---

## 13. Remove HashSets from the warm path

Current V3 validation uses sets to detect duplicate refs / NodeIds.

That is correct for a generic packet.

The fast lane is internal trusted runtime ABI.

Use stronger producer invariants instead.

### Destination refs

Require:

```text
dst_ref[i] > dst_ref[i - 1]
```

This makes duplicate and reordering detection O(1).

### NodeIds

The JS NodeId allocator already creates unique immutable identities.

Fast path:

- validate nonzero,
- validate <= MAX_SAFE_INTEGER,
- optionally check only if a destination ref collides.

Debug/differential builds may keep a slow uniqueness verifier.

Production fast path must not hash every tiny batch.

---

## 14. No environment-global lookup in the hot path

The current V3 path has an environment-level cache discovery and locking story.

Keep that for the N-API / V3 fallback.

Fast path:

```text
FastSessionHandle
    ->
dense session table
    ->
&mut FastSession
```

The table is owned by the current environment/thread.

No:

```text
OnceLock
global Mutex<HashMap<env, ...>>
Arc<Mutex<...>>
```

per commit.

---

## 15. Thread-affinity contract

Baseline PERF-10 should stay synchronous and single-threaded at the bridge.

Invariant:

```text
one Bun JS environment
    ->
one owning JS thread
    ->
one FastSession
    ->
all fast-commit calls on owner thread
```

Rust:

```rust
debug_assert_eq!(current_thread_id(), session.owner_thread);
```

Do not add `Mutex` merely to make misuse technically callable.

If a future renderer thread consumes sealed immutable pages, ownership is transferred to that thread after the synchronous FFI call.

JS must not mutate a sealed page while native may read it.

---

## 16. Native-owned UTF-8 page pool

This is the strongest zero-copy part of the design.

Create a native page pool. The current implementation uses 128 fixed 64 KiB pages so a retained mutation trace can survive page-retention amplification without reusing a page still referenced by the native graph. This is bounded (8 MiB per session) and remains subject to the compaction/retention measurements below.

Initial candidates:

```text
16 KiB
64 KiB
256 KiB
```

Recommended starting point:

```text
64 KiB
```

Each page:

```rust
struct SharedUtf8Page {
    id: u32,
    generation: u32,
    bytes: Box<[u8]>,
    used: u32,
    state: PageState,
}
```

Production implementation may use a more compact allocation.

States:

```text
FREE
WRITING
SEALED
RETAINED
```

---

## 17. Page ownership protocol

### 17.1 Acquire

JS asks for a writable page only when needed.

Prefer to allocate/map pages during initialization and reuse them.

Native marks:

```text
FREE -> WRITING
```

JS owns writes to that page.

### 17.2 Write

JS uses:

```ts
TextEncoder.encodeInto(value, writablePageView.subarray(cursor))
```

or the winning direct-write strategy.

The destination is native-owned memory.

No later packet copy occurs.

### 17.3 Seal

`iyon_fast_commit_v1()` receives the byte page id and used length through `FastControlV1`.

Native validates bounds and transitions:

```text
WRITING -> SEALED
```

After this transition, JS must not write to the page.

### 17.4 Retain

If new semantic Views reference strings from the page, native retains the page.

```text
SEALED -> RETAINED
```

### 17.5 Release

When no retained semantic value references the page:

```text
RETAINED -> FREE
```

The page may later be handed back to JS for writing.

This is the same core lifetime idea as OpenTUI pinning a native chunk until all JS consumers release it.

---

## 18. Retained string representation

To get the real zero-copy benefit, Rust semantic values cannot immediately convert every shared slice into `String`.

Introduce a private retained string abstraction.

Candidate:

```rust
#[derive(Clone)]
enum RetainedStr {
    Static(&'static str),
    Owned(Box<str>),
    Shared(SharedStr),
}

#[derive(Clone)]
struct SharedStr {
    page: Arc<SharedUtf8Page>,
    start: u32,
    len: u32,
}
```

`SharedUtf8Page` must be immutable after sealing.

`SharedStr` implements:

```text
as_str()
Deref<Target = str>
AsRef<str>
Eq
Hash
Debug
```

by text contents, not page identity.

---

## 19. UTF-8 safety

Do not scatter unchecked UTF-8 around the codebase.

At commit:

1. bounds-check every referenced byte range,
2. validate the used byte region or the unique referenced slices,
3. prove string boundaries,
4. seal page,
5. only then construct `SharedStr`.

After a page is validated and immutable, `SharedStr::as_str()` may use one tightly-contained unsafe conversion:

```rust
unsafe { std::str::from_utf8_unchecked(slice) }
```

only if the invariant is documented on `SharedUtf8Page`.

No mutation is allowed after sealing.

Malformed input must fail before host mutation or cache publication.

---

## 20. Why native-owned pages are better than retaining JS buffers

With JS-owned memory:

```text
Rust pointer validity
depends on
JS object lifetime + runtime backing-store behavior
```

With native-owned pages:

```text
JS view validity
depends on
native page lifetime
```

The second direction is easier for Iyon because Rust already owns the retained semantic graph.

It also matches OpenTUI's successful zero-copy pattern.

---

## 21. Bun bootstrap architecture

Keep N-API for bootstrap and fallback.

Recommended:

```text
load .node addon
    |
    v
create NativeTuiHost
create FastSession
    |
    v
N-API returns FastAbiV1 descriptor:
    abi version
    capabilities
    session handle
    command page pointer + length
    writable UTF-8 page pointer(s) + length(s)
    function pointers
    |
    v
bun:ffi linkSymbols / CFunction
    |
    v
map native pages once with toArrayBuffer
```

The hot path then uses `bun:ffi`.

Do not call N-API per retained update.

---

## 22. Fast ABI surface

Keep it tiny.

Candidate initial ABI:

```rust
extern "C" fn iyon_fast_commit_v1(
    session: u32,
) -> i32;

extern "C" fn iyon_fast_render_ref_v1(
    session: u32,
    generation: u32,
    packed_ref: u32,
) -> i32;

extern "C" fn iyon_fast_acquire_utf8_page_v1(
    session: u32,
    out_page: *mut FastPageInfoV1,
) -> i32;

extern "C" fn iyon_fast_release_client_page_v1(
    session: u32,
    page_id: u32,
) -> i32;
```

Potentially `acquire_utf8_page` can be avoided in the steady path by returning a small initial page pool at bootstrap.

No callback is required for the baseline.

---

## 23. Bun FFI binding

Use `linkSymbols` or `CFunction` over pointers obtained from the already-loaded native module.

Do not require a second independently loaded Rust image.

The binding should be centralized:

```text
packages/iyon-runtime/src/tui/fast_native.ts
```

No other runtime file imports `bun:ffi` directly.

Example shape:

```ts
const fast = linkSymbols({
  commit: {
    ptr: abi.commitPtr,
    args: ["u32"],
    returns: "i32",
  },

  renderRef: {
    ptr: abi.renderRefPtr,
    args: ["u32", "u32", "u32"],
    returns: "i32",
  },
})
```

The exact function-pointer retrieval mechanism may use the existing addon bootstrap.

---

## 24. Pin Bun exactly

This design deliberately depends on Bun FFI behavior and external-memory aliasing.

Therefore the runtime must be pinned exactly.

Do not use:

```text
Bun >= 1.x
bun-types: latest
```

Use:

```text
exact Bun version
exact Bun revision in benchmark metadata
matching bun types
```

CI qualification should include:

```text
bun --version
bun --revision
native build id
Fast ABI version
bridge schema version
git SHA
```

A Bun upgrade is a performance/safety qualification event, not a casual package-manager update.

---

## 25. Fast ABI handshake

Before enabling the fast path, verify:

```text
FAST_ABI_MAGIC
FAST_ABI_VERSION
FastControlV1 size
FastOpV1 size
FastOpV1 alignment
bridge schema version
native build id
supported Bun revision
endianness if relevant
pointer width
```

If anything disagrees:

```text
disable fast path
use V3/N-API fallback
```

Never attempt partial compatibility.

---

## 26. Exact identity

Current V3 exact identity is already a win.

PERF-10 should keep two candidates.

### Candidate E0 — current exact native ref

```text
bun:ffi renderRef(session, generation, packedRef)
```

Expected to beat the current N-API exact-ref call.

### Candidate E1 — JS semantic no-op

If rendering the exact same root has no required side effects:

```ts
if (root === lastCommittedRoot) return
```

This is the theoretical optimum:

```text
0 native calls
0 cache lookup
0 Weak upgrade
0 host render
```

Do not adopt E1 until semantics are explicitly tested:

- resize behavior,
- invalidation behavior,
- explicit redraw requests,
- host lifecycle events.

A forced redraw must remain available separately.

---

## 27. Routing policy

Do not attempt to make one transport optimal for everything.

Initial routing:

```text
exact root
    -> exact-ref FFI or JS no-op

supported retained patch batch
    -> FastShared V1

wide PersistentSeq path-copy batch
    -> FastShared V1

small new definitions reachable from retained lineage
    -> FastShared V1

cold root
    -> V3

cache miss
    -> V3 cold recovery

large rebuilt/full replacement
    -> V3

unsupported schema mutation
    -> V3
```

The threshold between FastShared and V3 should be measured.

Candidate threshold dimensions:

```text
op_count
new string bytes
number of full definitions
estimated V3 words
```

Do not use total tree size if changed work is already known.

---

## 28. FastShared compile path

Reuse PERF-8 metadata.

The compiler should not rediscover the semantic graph.

Input:

```text
PackedMeta lineage
PersistentSeq lineage
published generation
PackedRef metadata
```

Output:

```text
FastOpV1[]
optional shared UTF-8 slices
root WireRef
```

No generic schema walker when a lineage recipe exists.

---

## 29. Fast writer

The JS writer owns no backing allocation.

It only writes into mapped native memory.

Example abstraction:

```ts
class FastSharedWriter {
  readonly control: Uint32Array
  readonly ops: Uint32Array
  readonly bytes: Uint8Array

  begin(): void
  emit(op: FastOp): LocalWireRef
  writeUtf8(value: string): SharedStringRef
  commit(root: WireRef): FastStatus
}
```

No steady-state heap allocation should occur for a small patch.

Avoid:

```text
Array.map()
object spread in inner compiler
temporary tuples
new subarray per field
```

Use indexed writes.

---

## 30. Native FastSession staging

We still need atomic publication semantics.

Use reusable staging storage.

Example:

```rust
struct FastScratch {
    values: Vec<StagedFastValue>,
    publications: Vec<FastPublication>,
}
```

Created once.

Per commit:

```rust
values.clear();
publications.clear();
```

Reserve enough for normal small batches.

No per-commit HashSet.

No duplicated `definitions` + `staged_refs` vectors.

---

## 31. Cache publication

Do not publish destination refs while later ops can still fail.

Sequence:

```text
read fixed control
validate session/generation/counts
decode/apply FastOps into scratch
validate root
seal referenced byte page
publish all destination refs
host.render(root)
mark commit success
```

If any stage fails:

```text
scratch dropped/cleared
no destination refs published
host unchanged
```

For cache miss:

```text
return FAST_CACHE_MISS
JS invalidates transport generation
V3 cold closure retry once
```

Preserve PERF-8 recovery behavior.

---

## 32. Packed slot table

The current V3 paged slot table remains useful.

Fast path should access the live table directly from its thread-affine session.

Do not snapshot it.

That removes one cost that currently grows with slot-table high-water state.

Reads:

```text
session.refs.view(ref)
session.refs.sequence(ref)
```

Writes are staged and applied at publication.

---

## 33. PersistentSeq fast operations

Wide-parent behavior is one of the major reasons PERF-8 exists.

FastShared must support sequence path-copy directly.

The JS lineage already knows which sequence nodes are new.

Emit:

```text
SEQ_LEAF_REPLACE
SEQ_BRANCH_REPLACE
```

for only the changed sequence path.

Rust constructs retained sequence nodes directly from:

```text
existing persistent child refs
new local child refs
aggregate flags
sizes
```

Do not rebuild the full child vector.

---

## 34. Large string policy

A single very large string should not strand a mostly-empty shared page forever.

Initial rule:

```text
normal page: 64 KiB
large threshold: 32 KiB
```

If encoded string > threshold:

- allocate a dedicated native page sized for that payload,
- seal it,
- retain it independently,
- return it to pool/free when no semantic value references it.

Benchmark threshold/page geometry later.

---

## 35. Page-retention amplification

Zero-copy slabs can trade copying for memory retention.

Worst case:

```text
64 KiB page
one 8-byte string remains live
page cannot be reused
```

Therefore benchmark:

```text
live_payload_bytes
live_page_bytes
retention_amplification = live_page_bytes / live_payload_bytes
```

Required stress:

```text
10,000 iterations
one tiny surviving string per page
```

If amplification is unacceptable:

- smaller pages,
- mixed page classes,
- dedicated pages for long-lived metadata,
- optional copy-out of tiny survivors after a threshold.

Zero-copy should not mean unbounded retained memory.

---

## 36. Optional compaction escape hatch

A page may become mostly dead.

PERF-10 may allow a later compaction policy:

```text
if page live ratio < X%
and surviving bytes < Y
and page age > Z
    copy survivors once into compact native owned storage
    release shared page
```

This is intentionally not “zero copy forever.”

It is a bounded memory optimization.

Do not implement before measuring retention amplification.

---

## 37. True SharedArrayBuffer ring — optional PERF-10.x

Only consider a JS `SharedArrayBuffer` + Atomics ring if synchronous FastShared still leaves significant boundary cost.

Architecture:

```text
JS producer
    |
    | writes fixed FastOps
    | Atomics.store(write_index, release)
    v
SharedArrayBuffer ring
    |
    v
native consumer thread
    | acquire load
    v
apply patches / render
```

Potential benefit:

```text
no FFI call per commit
```

Potential cost:

- asynchronous semantics,
- backpressure,
- queue overflow,
- lifecycle races,
- renderer ordering,
- `screenRows()` / measurement synchronization,
- JS/native memory-model assumptions,
- difficult teardown.

Do not make this baseline merely because “shared memory sounds faster.”

The first FastShared design already gives zero-copy transport with one extremely small FFI call.

Benchmark that first.

---

## 38. Why one FFI call may be enough

The current regression is tens of percent, but the retained operations themselves are very small.

Current V3 pays for:

- generic packet encoding,
- generic field decoder,
- cache snapshot,
- multiple lock operations,
- HashSets,
- generic validation,
- staging duplication,
- publication bookkeeping,
- per-word instrumentation in counter builds.

FastShared pays for:

```text
write ~2–20 fixed ops
one direct FFI call
direct refs
direct patch constructors
publish
render
```

The remaining FFI call may be a tiny fraction of total warm work.

Do not introduce asynchronous rendering until this is measured.

---

## 39. Instrumentation correction

The current performance-counter build uses atomic counters in very hot decoder locations.

That must not be used for authoritative timing.

Create two benchmark artifacts.

### Timing build

Compile out inner-loop structural counters.

Allowed coarse counters:

```text
commit count
cache miss count
fallback count
page grow count
```

Only if they do not sit inside per-word/per-op loops.

### Counter build

Enable full counters.

Use a small number of iterations.

Use it to prove structural behavior, not latency.

Never compare V2 vs V3 vs FastShared timing with one candidate doing substantially more atomic counter traffic.

---

## 40. FastShared counters

JS diagnostic counters:

```text
fast_commits
fast_exact_ref_calls
fast_ops_emitted
fast_local_refs
fast_persistent_refs
fast_v3_fallbacks
fast_cache_misses

fast_utf8_strings
fast_utf8_bytes
fast_page_acquires
fast_page_seals
fast_page_reuses
fast_large_pages
```

Native diagnostic counters:

```text
fast_transactions
fast_ops_read
fast_refs_resolved
fast_seq_nodes_built
fast_views_built
fast_publications

fast_pages_retained
fast_pages_released
fast_live_page_bytes
fast_live_payload_bytes

fast_status_cache_miss
fast_status_invalid
```

These are diagnostic builds only where inner-loop cost is nontrivial.

---

## 41. Required correctness tests

### ABI

- struct sizes match JS constants,
- field offsets match,
- wrong ABI version disables fast path,
- wrong Bun revision disables fast path when strict pin is enabled.

### Session

- stale handle rejected,
- destroyed session rejected,
- wrong-thread use fails in debug build,
- generation mismatch returns cache miss.

### Fast op validation

- local forward reference rejected,
- local out-of-range reference rejected,
- invalid persistent ref returns cache miss,
- nonmonotonic destination refs rejected,
- invalid opcode rejected,
- invalid mask rejected,
- op count over capacity rejected.

### UTF-8 pages

- empty,
- ASCII,
- 2-byte,
- 3-byte,
- 4-byte scalar,
- emoji,
- combining sequences,
- embedded NUL,
- U+10FFFF,
- lone high surrogate,
- lone low surrogate,
- page-boundary encoding,
- invalid range rejected,
- page write after seal prevented by JS state machine.

### Atomic publication

Every malformed/failed commit must leave:

```text
host unchanged
generation unchanged unless explicit resync
published refs unchanged
page ownership recoverable
```

---

## 42. Differential oracle

For every FastShared mutation supported in production:

```text
direct
V2
V3
FastShared
```

must produce semantically equivalent Rust View/render output.

Run randomized mutation traces.

On mismatch, dump:

```text
seed
initial tree
mutation sequence
FastOps
V3 packet
native result summary
```

This is essential because the fast path intentionally removes some generic validation.

---

## 43. Unsafe-code boundary

Unsafe must remain tiny.

Acceptable categories:

### External page alias

Creating a slice over a native-owned allocation already represented by the session.

### Validated fixed-op slice

If the op region is stored as raw bytes:

```rust
unsafe {
    std::slice::from_raw_parts(
        ptr.cast::<FastOpV1>(),
        op_count,
    )
}
```

only after alignment and length validation.

### Validated retained UTF-8

`from_utf8_unchecked` only inside `SharedStr::as_str()` after the page invariant has been established.

Do not use:

```text
get_unchecked everywhere
raw pointer graph traversal
unchecked enum transmute
unchecked persistent ref indexing
```

The unsafe surface should be reviewable on one screen.

---

## 44. Panic and error policy

No Rust panic may cross the C ABI.

Every exported fast function:

```rust
extern "C" fn ...
```

returns a small status code.

Candidate statuses:

```text
0  FAST_OK
1  FAST_CACHE_MISS
2  FAST_BAD_SESSION
3  FAST_BAD_GENERATION
4  FAST_BAD_BATCH
5  FAST_PAGE_STATE
6  FAST_UNSUPPORTED
7  FAST_INTERNAL
```

Detailed debug diagnostics may be written into `FastControlV1.status_detail`.

Production JS should not allocate an `Error` object for expected cache-miss fallback unless needed.

---

## 45. Recommended benchmark sequence under 1800 seconds

Do not run one monolithic benchmark.

Run separate decision experiments.

### PERF-10.0 — clean timing baseline

Purpose:

- quantify how much counter instrumentation distorted V3.

Matrix:

```text
candidates:
  V2
  V3

sizes:
  20
  200

modes:
  SHARED_PATH
  LARGE_SHARED_SUBTREE_CUTOFF
  SHARED_DEEP
  IDENTICAL_IDENTITY

workloads:
  plain_text_column
  styled_span_heavy
  row_heavy
```

Timing build only.

Samples:

```text
warmup 50
measured 500
```

---

## 46. PERF-10.1 — boundary microbenchmark

Candidates:

```text
empty N-API
empty bun:ffi

exact-ref N-API
exact-ref bun:ffi

one scalar patch bun:ffi
one FastShared page commit

3-op batch
8-op batch
16-op batch
```

Use large iteration counts because operations are tiny.

This answers:

- whether `bun:ffi` itself matters,
- whether shared-page commit beats pointer arguments,
- whether handle lookup matters.

Also compare:

```text
u32 session handle
raw session pointer
```

If raw pointer is not materially faster, use the handle.

---

## 47. PERF-10.2 — retained warm path

Matrix:

```text
sizes:
  20
  200

modes:
  SHARED_PATH
  SHARED_DEEP
  LARGE_SHARED_SUBTREE_CUTOFF
  TEXT_METADATA_PATCH
  DECORATION_PATCH

candidates:
  V2
  V3
  FastShared
```

Primary gate:

> FastShared must beat V2, not merely beat V3.

---

## 48. PERF-10.3 — wide parent

This is mandatory before production.

Workloads:

```text
WIDE_PARENT_ONE_EDIT
```

Widths:

```text
2,000
10,000
100,000
```

Candidate workload classes:

```text
row
column
grid cells if supported
```

Measure:

```text
ops emitted
PersistentSeq nodes built
items iterated
total commit
native commit
```

Required structural behavior:

```text
O(log_32 N)
```

not O(N).

---

## 49. PERF-10.4 — shared UTF-8 retention

Compare:

```text
V3 UTF-8 packet + native copy
FastShared UTF-8 retained page
```

Workloads:

```text
one span replace
one diff line replace
large text append
many tiny metadata strings
many repeated short strings
large unique text
```

Metrics:

```text
JS encode time
native retain time
bytes copied after JS encode
live payload bytes
live page bytes
page reuse
p95 / p99
```

The target for FastShared is:

```text
post-encode transport copies = 0
```

for retained shared strings.

---

## 50. PERF-10.5 — cold/bulk guardrail

Confirm V3 remains the correct bulk path.

Sizes:

```text
200
2,000
10,000
```

Modes:

```text
COLD
FIRST_USE
REBUILT_EQUIVALENT
```

Candidates:

```text
V2
V3
FastShared only if routing would choose it
hybrid router
```

The hybrid router should choose V3 for these unless measurements prove otherwise.

---

## 51. PERF-10.6 — realistic synthetic trace

Use one authoritative trace.

Suggested initial mix:

```text
20% exact identity
55% narrow retained path
10% deep retained path
5% wide one-edit
8% rebuilt equivalent
2% cold/recovery
```

The exact percentages should reflect real Iyon workloads if production telemetry or representative traces exist.

Run:

```text
V2
V3
HybridFastShared
```

Do not include abandoned string-lane candidates in the final decision trace.

---

## 52. Acceptance gates

### Mandatory correctness

- all differential tests pass,
- cache recovery remains one cold retry,
- no UAF under teardown stress,
- no page reuse before native release,
- no cross-thread mutable aliasing,
- no malformed fast batch partially mutates host.

### Structural

- exact identity does O(1) work,
- stable subtree cutoff remains path-only,
- wide one edit remains O(log N),
- no hidden full sequence flatten.

### Performance

Normal small/medium retained cases:

```text
FastShared median must beat V2
```

Target:

```text
>= 10% faster than V2
```

Strong result:

```text
>= 20% faster than V2
```

Synthetic trace:

```text
minimum adoption target: >= 10% faster than V2
preferred target: >= 15%
strong target: >= 25%
```

Exact identity:

```text
no regression vs current V3 exact-ref
```

Cold/bulk:

```text
hybrid no worse than V3 by >5%
```

Memory:

```text
bounded page retention
no monotonic page leak
```

---

## 53. Stop conditions

Reject or redesign FastShared if:

- it remains slower than V2 on `SHARED_PATH`,
- shared pages only save copies but fixed native bookkeeping still dominates,
- retained shared strings produce unacceptable memory amplification,
- Bun external-memory lifetime behavior cannot be made deterministic under the pinned runtime,
- wide edits accidentally flatten,
- raw pointer lifetime cannot be audited,
- background-thread shared memory is required just to break even.

If synchronous FastShared cannot beat V2, then the next architectural question is not another serializer.

It is:

> Should semantic View construction itself move into the native retained graph earlier?

That would be a larger native-backed View architecture.

---

## 54. Implementation tranches

### 9.0 — measurement cleanup

- build timing-native artifact with hot atomic counters compiled out,
- rerun reduced V2/V3 retained matrix,
- preserve old counter build for structural proof.

Suggested commit:

```text
bench(tui): separate timing and structural-counter builds
```

### 9.1 — FFI boundary proof

- pin Bun candidate version/revision,
- expose one empty C function,
- expose exact-ref C function,
- benchmark N-API vs `bun:ffi`,
- benchmark u32 handle vs raw pointer.

Suggested commit:

```text
bench(tui): qualify pinned Bun FFI fast boundary
```

### 9.2 — native-owned command page

- create `FastSession`,
- allocate command page,
- return pointer/capacity at bootstrap,
- map once with `toArrayBuffer`,
- implement `FastControlV1`,
- implement fixed `FastOpV1`,
- no strings yet.

Suggested commit:

```text
feat(tui): add native-owned shared fast-command page
```

### 9.3 — scalar retained patches

Implement:

```text
text layout
decoration
simple container/root path
```

No sequence edits yet.

Suggested commit:

```text
perf(tui): apply retained scalar patches through shared FFI
```

### 9.4 — PersistentSeq fast path

- sequence leaf op,
- sequence branch op,
- local refs,
- wide one-edit benchmark.

Suggested commit:

```text
perf(tui): patch persistent sequences through shared fast ops
```

### 9.5 — zero-copy UTF-8 retained pages

- native-owned byte-page pool,
- JS direct encode into page,
- seal/retain/release protocol,
- `RetainedStr::Shared`,
- text/diff first,
- then metadata/glyph paths as justified.

Suggested commit:

```text
perf(tui): retain UTF-8 directly from native shared pages
```

### 9.6 — hybrid router

- exact identity,
- FastShared retained patch,
- V3 cold/bulk/fallback,
- thresholds,
- one cold retry.

Suggested commit:

```text
perf(tui): route retained updates through native fast transport
```

### 9.7 — authoritative benchmark

- wide,
- retained,
- cold guardrail,
- realistic trace,
- memory churn,
- p95/p99.

Suggested commit:

```text
bench(tui): complete PERF-10 shared-memory decision
```

### 9.x — optional async ring

Only if synchronous FFI remains a measured bottleneck:

```text
SharedArrayBuffer
Atomics
native consumer thread
```

Do not implement speculatively.

---

## 55. Files likely to change

Runtime:

```text
packages/iyon-runtime/src/tui/
  fast_shared.ts
  packed_v3.ts
  packed_v3_meta.ts
  persistent_seq.ts
  values/view.ts
```

Native bridge:

```text
crates/iyon-native/src/tui.rs
crates/iyon-native/src/tui/fast_shared.rs
crates/iyon-native/src/tui/packed_v3.rs
```

Retained semantic storage:

```text
crates/iyon-tui/src/presentation/ir.rs
crates/iyon-tui/src/presentation/api/text.rs
style/theme/border string holders as required
```

Benchmarks:

```text
packages/iyon-runtime/bench/tui_performance.ts
packages/iyon-runtime/bench/PERF-10-results.jsonl
packages/iyon-runtime/tests/tui_fast_shared.test.ts
```

Exact filenames may vary; do not reorganize the project merely to match this document.

---

## 56. Banned shortcuts

Do not:

- replace V3 with FastShared for every workload,
- put V3 bytes in a `SharedArrayBuffer` and call that the solution,
- retain a pointer into a growable JS buffer,
- mutate a sealed retained page,
- flatten PersistentSeq before native commit,
- create one FFI call per ancestor when a batch can represent the whole path,
- expose dozens of tiny C functions for every View property,
- use JS callbacks on the commit hot path,
- return Rust-owned heap pointers without a lifetime protocol,
- use a global strong string interner,
- copy shared UTF-8 into `String` immediately and still call the lane zero-copy,
- benchmark with per-word atomic counters enabled,
- decide based only on FFI microbenchmarks,
- decide based only on encoder time,
- accept a warm-path regression because large COLD is fast.

---

## 57. Recommended production shape

The intended endpoint is:

```text
                           ┌──────────────────────────┐
                           │      JS semantic IR      │
                           │ immutable + PERF-8 meta  │
                           └────────────┬─────────────┘
                                        │
                         exact identity │
                      ┌─────────────────┴───────────────────┐
                      │                                     │
                      v                                     v
               zero-op / renderRef                  retained lineage
                                                            │
                                                            v
                                                native-owned shared page
                                                FastOpV1 + UTF-8 slices
                                                            │
                                                            v
                                                     bun:ffi commit
                                                            │
                                                            v
                                              thread-affine FastSession
                                                            │
                                         ┌──────────────────┴───────────────┐
                                         │                                  │
                                         v                                  v
                                  retained View                         cache miss /
                                  + PersistentSeq                       unsupported
                                         │                                  │
                                         v                                  v
                                   host.render()                       Packed V3
                                                                            │
                                                                            v
                                                                      cold recovery
```

This keeps the best properties of both designs:

**PERF-8 / V3**
- robust bulk transport,
- reconstruction performance,
- cache recovery,
- full schema,
- retained structural identity.

**PERF-10 FastShared**
- no generic packet for normal retained changes,
- no transport copy for numeric ops,
- zero-copy post-encode UTF-8 retention,
- no hot-path mutex,
- no global cache lookup,
- no slot snapshot,
- no generic HashSet validation,
- one tiny FFI crossing,
- bounded unsafe.

---

## 58. Final recommendation

The implemented baseline is **pinned Bun + `bun:ffi` + native-owned shared pages**. Keep it behind the PERF-10 benchmark feature until the authoritative production decision is reviewed.

More specifically:

```text
Bun:
  exact version/revision pin

N-API:
  bootstrap
  ownership/lifetime
  V3 fallback
  debug/recovery

bun:ffi:
  exact-ref hot path
  one shared-page commit call
  page-management calls outside the inner path

memory:
  native-owned
  mapped into JS once
  JS writes directly into native pages
  immutable after seal
  native retains pages by Arc/refcount

protocol:
  fixed-width FastOpV1
  topological local refs
  no generic record decoder

strings:
  encode directly into native-owned UTF-8 page
  retain same bytes as SharedStr
  no second transport copy

V3:
  cold
  rebuilt
  large bulk
  unsupported
  cache recovery

SharedArrayBuffer + Atomics:
  optional later experiment only if one synchronous FFI call is still measurable
```

The key distinction is:

> **Use shared memory to remove transport ownership and copying, while using the direct C ABI to remove generic decoding.**

Either one alone is incomplete.

Putting V3 in shared memory would still leave the V3 decoder overhead.

Using FFI with per-call temporary arrays would still leave avoidable buffer construction and retained-string copies.

The combination is what targets the measured problem at its source.

---

## 59. Research notes / source appendix

Primary sources consulted for this handoff:

### Bun FFI

Bun FFI documentation:

https://bun.sh/docs/runtime/ffi

Important relevant points:

- `bun:ffi` is still documented as experimental.
- it supports C ABI functions from Rust/Zig/C/etc.
- `CFunction` / `linkSymbols` can call known function pointers, including pointers obtained from an already-loaded Node-API module.
- typed arrays can be passed as pointer/buffer arguments.
- `ptr()` exposes typed-array addresses.
- `toArrayBuffer()` exposes native memory as an ArrayBuffer.
- Bun documents deallocator hooks for native memory exposed to JavaScript.
- Bun explicitly places memory-lifetime responsibility on the caller.

### OpenTUI

Repository:

https://github.com/anomalyco/opentui

FFI abstraction:

https://github.com/anomalyco/opentui/blob/main/packages/core/src/platform/ffi.ts

Native ABI / handles:

https://github.com/anomalyco/opentui/blob/main/packages/core/src/zig/lib.zig

NativeSpanFeed zero-copy wrapper:

https://github.com/anomalyco/opentui/blob/main/packages/core/src/NativeSpanFeed.ts

Renderer documentation:

https://github.com/anomalyco/opentui/blob/main/packages/web/src/content/docs/core-concepts/renderer.mdx

OpenTUI engineering guidance:

https://github.com/anomalyco/opentui/blob/main/AGENTS.md

Relevant OpenTUI lessons:

- direct C ABI is a first-class architecture,
- native handles are preferable to leaking raw pointers through the public JS layer,
- transient arrays are borrowed synchronously,
- retained pointers require explicit backing-lifetime ownership,
- native memory can be exposed to JS as zero-copy ArrayBuffer views,
- NativeSpanFeed uses explicit pin/refcount behavior so native chunks remain alive while JS uses them,
- substantial render/storage state remains native.

OpenTUI roadmap:

https://github.com/anomalyco/opentui/issues/821

The roadmap includes moving more incremental text work into native code rather than repeatedly replacing full styled text, which is directionally consistent with PERF-10's retained-update goal.

---

## 60. One-sentence handoff to the implementer

**Do not optimize the V3 packet again: preserve V3 for bulk/recovery, but make the normal retained path write fixed FastOps and UTF-8 directly into native-owned shared pages and commit them through one pinned-Bun FFI call into the existing PERF-8 retained graph.**
