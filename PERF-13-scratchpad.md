# PERF-13 scratchpad

> Freebies and further-looking optimizations that are independent of whether
> PERF-12 selects the Direct 7v2 (N-API) path or the Retained DAG Direct FFI
> candidate. Every idea here improves *any* winner because none depends on the
> transport architecture — they target the shared runtime, the renderer caches,
> or the iyon-tui framework's hot data structures.

---

## 1. PaintCache key representation

`PaintCache` in `crates/iyon-tui/src/presentation/paint/view.rs` uses a
`HashMap<PaintKey, Arc<Surface>>` with a key that costs more to hash than the
cache saves — especially on the miss path where hashing is pure waste.

### Current key

```rust
struct PaintKey {
    view_id: ViewId,
    rect: Rect,                              // 4 × u16
    content_rect: Rect,                      // 4 × u16
    clip_rect: Rect,                         // 4 × u16
    inherited_style: PhysicalStyle,          // ~several fields
    resolved_style: PhysicalStyle,           // ~several fields
    node_context: StyleContextKey,            // Vec<(String, String)> !
    descendant_context: StyleContextKey,      // Vec<(String, String)> !
}
```

`StyleContextKey` is:

```rust
struct StyleContextKey {
    inherited_states: Vec<(String, String)>,
    local_facts: Vec<(String, String)>,
    focused: bool,
    focus_within: bool,
}
```

Every lookup hashes **all of the above**, including walking the string tuples
inside `style_state` vectors. The HashMap also means each `get` computes a full
hash and probes the table — the Rect fields alone are 12 × u16 = 192 bits of
entropy being run through SipHash per frame *per paintable node*.

### Ideas

1. **Replace with a generation-indexed slot map** keyed by `(ViewId,
   epoch_generation)` where `epoch_generation` bumps when any ancestor's
   inherited style changes. A miss then falls through to paint immediately
   without paying the hash. This avoids hashing entirely.

2. **Flatten the key** — replace the two `StyleContextKey` vectors with a
   compact bitmask of active style states (focused, focus_within, and a
   generation counter for the state/ fact stack). The string-typed
   `inherited_states` and `local_facts` are already associated with
   `StyleRef::theme(String)` — the *values* don't need to be in the paint key,
   only the *identity* of which states are active. A state-stack depth counter
   combined with a generation tag is O(1) to compare.

3. **Two-phase lookup**: use a trivial array indexed by `view_id mod N` as a
   bloom filter / fast reject before falling through to a canonical store.
   Most paint lookups miss anyway (geometry changes every frame); paying a full
   hash on a miss is the worst case.

4. **Pre-compute a compact cache key** during layout, stored on the
   `LayoutNode`. Layout already holds the resolved style context; if we assign
   a dense `PaintCacheIndex` (e.g. a simple u32 counter bumped per
   view-tree-visible change), the paint cache lookup becomes a single
   `Arc<Surface>` index compare.

### Independence from PERF-12

The paint cache lives entirely inside `iyon-tui`. Neither candidate touches it.
The improvement applies whether Views arrive via N-API objects, generated FFI
constructors, or packed records.

---

## 2. StyleContextKey string vectors

Even outside the paint key, `StyleContextKey` itself carries
`Vec<(String, String)>` for `inherited_states` and `local_facts`. This
allocates per-context and the strings are cloned when the context descends
to children.

### Ideas

1. **Intern style state keys and values** via a small arena/generation counter.
   The number of unique style-state key/value pairs in any TUI session is tiny
   (single-digit). Every node on the paint path clones or re-hashes them.

2. **Replace with a small-bitmask approach**: `StyleRef::theme(name)` is
   called with a `&str`. If the set of possible theme keys is bounded (it
   practically is — no agent creates 1000 unique style states), a dense index
   per unique key + a u64 value mask would eliminate all heap state from the
   style context.

3. **Shared reference-counted slices**: `Arc<[(String, String)]>` instead of
   `Vec<(String, String)>`. The context is built once per frame and shared
   down the tree; cloning the Arc is O(1) instead of O(state count + string
   allocations).

### Independence from PERF-12

Same argument as PaintCache: style context is `iyon-tui` framework internals.

---

## 3. `path_nodes` / `path_keys` interning

Already classified in T2 as category C: *"path/recipe machinery made redundant
by the semantic DAG"*. It's still present in `NativeViewRuntime` because the
old "text layout patch path" and "edit transaction" paths have not been
removed yet (PERF-12 T16 would do that if Candidate 12 wins).

### Idea

Even if Candidate A (Direct 7v2) wins, this machinery could be pruned. The
text layout patch path was PERF-8/9-era optimization for avoiding full View
reconstruction. If a retained semantic DAG exists (either the current lazy
pending style or the full eager 7v2 style), the path interning is unnecessary
because the semantic DAG already encodes the changed frontier directly.

Removing these two `HashMap`s and the associated path-handle allocation
machinery eliminates ~400 lines of ABI functions, the per-VNode path-key
hashing, and the `path_nodes` / `path_keys` allocation bookkeeping.

### Independence from PERF-12

The path-ref ABI functions are already behind generated exports; the
TypeScript side can stop calling them regardless of which transport wins. The
hash tables are live but the text-patch-path operations are not on the
hot retained path for either candidate.

---

## 4. `style_atoms` / `styles` representation

`style_atoms: HashMap<u32, String>` and `styles: HashMap<u32, StyleRef>` are
both monotonic-allocated handle tables used during style creation. They're
bounded by unique style count, which is typically small.

### Idea

Since `style_atoms` is keyed by a monotonic `u32` (no holes), it could use the
same paged-table pattern as `NativeRefTable`. Lookup is `Vec<Option<Box<[...]>>>`
indexing instead of SipHash. The same applies to `styles`.

The win is modest (style creation is not the hot path), but the replacement is
trivial and removes two more `HashMap`s from the native bridge.

### Independence from PERF-12

`style_atoms` and `styles` are shared runtime state — every transport uses them
equally.

---

## 5. `nodes: HashMap<u64, WeakView>` in NativeViewRuntime

The semantic cache is the central NodeId→View mapping. It's consulted on every
publication, every cache hit, every path lookup.

### Idea

`u64` keys don't map as cleanly into a dense paged table as `u32` NativeRefs
do, because the NodeId space (53-bit semantic IDs) is sparse. However, a few
approaches could beat HashMap:

1. **Frontier-biased caching**: a small direct-mapped cache of the last N
   NodeId lookups (like a CPU TLB) in front of the HashMap. Most frame work
   touches only the ~200 nodes of the current frontier; a 256-entry bloom
   filter or small associative cache would catch the majority of lookups
   without touching the HashMap at all.

2. **Monotonic NodeId allocator**: if NodeIds are strictly increasing (they
   are — the JS allocator never reuses them), a `Vec<Option<WeakView>>` where
   the index is `(NodeId - 1) mod GROWTH_BLOCK` could replace the HashMap for
   recently-allocated NodeIds. Older IDs spill to a fallback structure.

3. **Combined `nodes` + `node_refs`**: since every published node has both a
   `WeakView` in `nodes` and a `u32` NativeRef in `node_refs`, these two maps
   could be merged into a single value type `SemanticEntry { weak, ref }`
   stored in one dense structure. Every lookup resolves both questions in one
   table probe instead of two.

### Independence from PERF-12

The semantic cache is shared across all transports. Direct 7v2, generated FFI,
packed — all route through `consult_semantic_identity` which reads both maps.
Improving them helps every candidate equally.

---

## 6. LayoutCache key simplicity

`LayoutCache` in `crates/iyon-tui/src/presentation/layout/cache.rs` uses
`HashMap<MeasureKey, Arc<MeasuredNode>>` where:

```rust
struct MeasureKey {
    view: ViewId,
    component_view: Option<ViewId>,
    width: u16,
    intent: WidthIntent,
}
```

The key is 4 fields plus an `Option<ViewId>`. This is not as bad as PaintKey,
but the two-generation swap pattern (swap + clear every epoch) means the
HashMap is rebuilt from scratch every frame anyway — the old generation is
simply dropped. This suggests a generation-tagged array might work: if
`ViewId` is dense enough, a `Vec<Option<(u64 epoch, Arc<MeasuredNode>)>>`
keyed by `view_id` eliminates hashing and the swap+clear allocation.

### Independence from PERF-12

LayoutCache is `iyon-tui` framework code.

---

## 7. `RUNTIME_HANDLES: OnceLock<Mutex<HashMap<usize, ViewRuntimeHandle>>>`

The global N-API environment→runtime handle registry. Every `tuiViewAbiBootstrap`
and every FFI call that resolves `runtime_mut` from a pointer touches the global
lock.

### Idea

This is small (one lock acquisition per bootstrap + per N-API call), but the
feature-gated `packed_v3`, `packed_v4`, `fast_slots`, and `fast_sessions`
HashMap entries in `NativeViewRuntime` also sit behind feature gates that
aren't part of the normal build. If those are never enabled in the production
build, their unused fields in `NativeViewRuntime::new()` are dead struct
padding.

Not a big win, but a tidy one.

---

## 8. `NativeRefTable` — already done in T2, but the pattern can spread

T2 replaced `HashMap<u32, NativeViewSlot>` with `NativeRefTable<12>`. The
same construction (monotonic key → paged dense table) applies to all the
handle-based tables in the native bridge.

Potential candidates already listed above: `style_atoms`, `styles`,
`path_nodes` (if kept), `edit_txns` (if kept), `builders` (if kept).

---

## 9. Perf-counter build vs timing build discipline

The PERF-10 handoff §39 noted that the counter build uses atomic increments
on the hot decoder path and must not be used for authoritative timing. This
is already documented but the codebase still has:

```rust
#[cfg(feature = "perf-counters")]
iyon_tui::perf::inc(iyon_tui::perf::Counter::$counter);
```

embedded inline in hot functions. Any future perf work that needs clean timing
must ensure the timing build compiles out all counters. The `cfg` gates already
exist, so this is just a reminder to *use* the timing build for decision
benchmarks.

---

## 10. Use-after-free / stale-pointer safety in the FFI boundary

Both PERF-12 candidates share the same raw-pointer FFI pattern:

```rust
unsafe extern "Rust" fn view_spacer_create_impl(
    runtime: *mut NativeViewRuntime,
    ...
)
```

`runtime_mut` does an `unsafe { pointer.as_mut() }` + thread-ID check. If the
runtime is deallocated and the pointer is dangling, `as_mut()` returns `None`
and we get a graceful `FAST_INVALID`. That's fine.

But the `host: *mut NativeHost` in `host_render_ref_impl` and
`edit_txn_commit_render_impl` is only checked via `host.alive` — there's no
guarantee the host pointer still points to a valid allocation. A stale host
pointer would read garbage `alive` and potentially write into freed memory.

### Idea

Add a host-side magic + generation similar to the runtime's `magic`/`abi_version`/
`alive` pattern. A 4-byte `u32` magic at offset 0 of `NativeTuiHost` would
let `host_render_ref_impl` sanity-check the pointer before dereferencing
`host.alive`.

This is a correctness hardening, not a performance optimization, but every
transport uses the same host-render path.

---

## Summary: what to scratch first

Priority order for a hypothetical PERF-13 (always-on freebies):

1. **PaintCache key flattening** — most visible per-frame savings on the
   `iyon-tui` side, no ABI changes needed.
2. **StyleContextKey → interned bitmask** — eliminates String clones and
   Vec allocations per node, cleans up the paint key.
3. **Merge `nodes` + `node_refs`** into one dense structure — every
   publication pays two `HashMap` lookups; a combined table halves that.
4. **Prune `path_nodes`/`path_keys`** — if unused by the chosen winner,
   delete ~400 lines and two maps.
5. **Paged table for `style_atoms`/`styles`** — trivial replacement, removes
   two more HashMaps, predictable cost.
6. **Pre-computed paint cache index on `LayoutNode`** — eliminates paint
   cache hash entirely, replaces with a simple slot index comparison.
7. **Host pointer magic validation** — safety hardening.

None of these require knowing whether PERF-12 picks Direct 7v2 or Retained DAG
Direct FFI. All of them improve the smoothness the Muppet feels.

Happy scratching! 🐱