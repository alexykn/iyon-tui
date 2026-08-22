# PERF-13 scratchpad

> Freebies and further-looking optimizations that are candidates to run after
> PERF-12 concludes. **PERF-12 status: tranche 3, transport decision
> (N-API vs Direct FFI) still open; the Retained DAG direction above the
> transport layer is decided.** Items are tagged accordingly:
>
> - `[independent]` — safe to implement now, helps either transport outcome.
> - `[transport-coupled]` — value or validity depends on which transport wins;
>   defer until the PERF-12 final benchmark decides.
>
> Relevance ratings and code references below were re-verified against the
> tree on 2026-02 (see "Verification notes" at the end).

---

## 1. PaintCache key construction — `[independent]` — HIGH

**Where:** `crates/iyon-tui/src/presentation/paint/view.rs`

**Hot path:** `crates/iyon-tui/src/scene/host.rs:439–441` — every rendered frame
calls `paint_cache.begin_epoch(theme)` followed by
`ViewPainter.paint_tree_with_cache(...)`, which descends `paint_node` for every
layout node (`view.rs:157`). Per painted node:

1. `view.rs:170` — `compiler.theme.resolve_text_style(...)` runs unconditionally.
2. `view.rs:183–192` — **the full `PaintKey` is constructed unconditionally,
   *before* the `can_cache` check at `view.rs:194`.** This includes two
   `StyleContextKey::from(...)` conversions, each of which allocates two
   `Vec<(String, String)>` with a heap `String` clone per entry
   (`view.rs:26–43`).
3. Only then does `cache.surface(&key)` probe `current`, then `previous`
   (`view.rs:82–95`). A previous-generation hit additionally does
   `key.clone()` (another 4 vector allocations) before inserting into
   `current`.

So the worst cost is not SipHash over the 192 bits of rect entropy — it is
that **non-cacheable nodes pay full key construction including string
allocation for nothing**, and cache misses pay it too. The original claim
("hashing costs more than the cache saves") understated this.

### Ideas, cheapest first

1. **Guard key construction behind `can_cache`** (`view.rs:194`): move the
   `PaintKey` construction below the flag check. One-move change, removes all
   allocation for non-cacheable nodes. Do this first.
2. **Kill the `String` conversion**: the source `StyleContext`
   (`presentation/paint/theme.rs:11–16`) already stores
   `StyleStates`/`StyleFacts` whose backing store is a sorted
   `Vec<(StyleAtom, StyleAtom)>` with binary-search lookup
   (`presentation/api/style.rs:286–296`), where `StyleAtom` is
   `Static(&'static str) | Owned(String)` (`style.rs:401–404`). The key's
   `From<&StyleContext>` impl needlessly downgrades this to owned
   `(String, String)`. Key on the atoms directly (they already impl
   comparison by `as_str`) or intern them — see §2.
3. **Avoid the promotion clone** (`view.rs:87–91`): when promoting from
   `previous`, the key is cloned only to insert into `current`; restructuring
   the two-generation map so promotion moves entries would remove another
   allocation burst on stable frames.
4. Larger redesigns (generation-indexed slot map keyed by
   `(ViewId, epoch_generation)`, pre-computed dense paint index stored on
   `LayoutNode`) remain possible but require building an ancestor-style-change
   generation mechanism that does not exist yet. Not a freebie.

### Independence

Entirely inside `iyon-tui` presentation/paint. Untouched by either transport.

---

## 2. StyleContext / StyleContextKey string representation — `[independent]` — HIGH

**Where:**
- Key conversion allocating strings: `crates/iyon-tui/src/presentation/paint/view.rs:26–43`.
- Live context types: `crates/iyon-tui/src/presentation/paint/theme.rs:10–47`.
- Atom storage: `crates/iyon-tui/src/presentation/api/style.rs:222–304, 401–411`.

**Hot path:** same as §1 — `paint_node` calls
`inherited_context.enter_node(...)` per node (`view.rs:169`), which clones the
whole `StyleStates` assignment vector per node (`theme.rs:23–32`), then
`for_descendant()` clones it again (`theme.rs:35–42`). Every one of those
clones copies the `Vec<(StyleAtom, StyleAtom)>`; then §1's key conversion
re-materializes it as owned `String`s.

### Ideas

1. **Interning / static-first atoms**: most keys/values in practice come
   through `StyleStateKey::from_static` / `StyleStateValue::from_static`
   (`style.rs:225, 269`). Making the key borrow the source context instead of
   owning `String`s removes the allocations without any interning machinery.
   A self-referential key is not needed if the lookup compares against a
   freshly built key (compare-by-value, no retention), or if the cache stores
   the key alongside the surface.
2. If dynamic (owned) state values turn out to be common in profiles, add an
   intern table mapping `(key_atom, value_atom)` → dense `u32` so the paint
   key carries integers only.
3. `Arc<[(StyleAtom, StyleAtom)]>` shared slices are a weaker variant; prefer
   1–2 first.

### Independence

`iyon-tui` framework internals. Note §1 and §2 overlap heavily: implementing
this first shrinks the `PaintKey` problem to plain data (rects + styles +
small vectors of atoms).

---

## 3. `path_nodes` / `path_keys` pruning — `[transport-coupled]` — DEFER

**Correction to the previous revision:** these maps are **not dead code**.
The TypeScript side actively uses the path machinery in production paths:

- Rust side: `nodes`, `node_refs`, `path_nodes`, `path_keys`, `edit_txns`,
  `styles`, `style_atoms` all live on `NativeViewRuntime`
  (`crates/iyon-native/src/tui/view_abi.rs:345–354`); path allocation logic at
  `view_abi.rs:464–544`, edit transactions at `view_abi.rs:642–697`,
  `abort_all_edit_txns` at `view_abi.rs:1171`.
- TS side: `packages/iyon-runtime/src/tui/native_view_abi.ts` imports
  `pathRoot`/`pathChild` (lines 7–8), resolves them from bootstrap functions
  (lines 203–204, 289–290), and calls them via `tryNativePathScalarRender`
  (line 427) and `nativePathRefForLineage` (lines 992, 1006, 1100). Benches
  exist separately in `packages/iyon-runtime/bench/tui_abi_path.ts` and
  `tui_abi_transaction.ts`.

Whether this machinery can be pruned depends entirely on whether the FFI /
Retained DAG transport wins PERF-12 and stops calling the scalar-render and
transaction ABI exports. **Re-evaluate immediately after the PERF-12 final
benchmark.** If pruned: ~400 lines of ABI functions plus both maps plus the
TS callers go away.

---

## 4. `style_atoms` / `styles` handle tables — `[independent]` — LOW

**Where:** `crates/iyon-native/src/tui/view_abi.rs:352–353`;
monotonic allocation at `view_abi.rs:892–915` (`allocate_style_atom_ref`,
`STYLE_ATOM_REF_LIMIT` bounded).

Both are monotonic-key tables written during style creation only. Not on the
per-frame hot path. The paged-table pattern from `NativeRefTable<12>` (see §8)
is a trivial, mechanical replacement. Do opportunistically, not as dedicated
work.

---

## 5. `nodes` + `node_refs` semantic identity maps — `[independent]` — MEDIUM

**Where:** `crates/iyon-native/src/tui/view_abi.rs:345–347`
(`nodes: HashMap<u64, WeakView>`, `node_refs: HashMap<u64, u32>`).

**Hot path:** publication, not painting. `consult_semantic_identity`
(`view_abi.rs:1028–1066`) is called per published node (call site
`view_abi.rs:1103`, used by direct and staged publication). It performs:

1. `node_refs.get(node_id)` → on hit, `resolve_ref(reference)` into `nodes`
   territory, comparing the existing `View` for equality
   (`SemanticIdentityMatch::SameLiveWithRef` / `Conflict`);
2. on miss/expired, `nodes.get(node_id)` + `WeakView::upgrade` +
   full `View` equality;
3. expired-entry cleanup (`nodes.remove`).

So up to **two HashMap probes plus a `View` deep-equality per published node
per frame**. Merging into one table halves the probes; the deep equality
usually dominates anyway, so measure before investing heavily.

### Ideas

1. **Merge into `SemanticEntry { weak: WeakView, reference: Option<u32> }`**
   in a single map. Straightforward; preserves all current semantics because
   the two maps are already kept in lockstep by `install_semantic_view`
   (`view_abi.rs:1069–1079`) and the various `node_refs.remove(&node_id)`
   sites (e.g. `view_abi.rs:839–896, 1019–1075`).
2. Frontier-biased small cache or monotonic-ID dense storage are optional
   extras; NodeIds are allocated monotonically by the JS side, but the map
   also serves arbitrary lookups (`resolve_ref`, scavenge sweeps), so a hybrid
   structure must keep the fallback correct.
3. Caveat: keep the WeakView scavenging / expired-slot accounting
   (`semantic_cache_expired_seen`, `semantic_cache_entries_removed`,
   `nodes_inserted_since_full_sweep`) intact through any restructure.

### Independence

Shared runtime state consulted by every transport (`consult_semantic_identity`
is transport-agnostic).

---

## 6. LayoutCache — `[independent]` — LOW/MEDIUM (premise partly wrong)

**Where:** `crates/iyon-tui/src/presentation/layout/cache.rs`.
Keys: `MeasureKey { view, component_view, width, intent }` (cache.rs:15–21);
storage is `current_measure`/`previous_measure` plus the same pair for
`PrepareKey` (cache.rs:41–43). Epoch rollover swaps generations and clears
only the new-current generation (cache.rs:48–54) — the previous generation
survives, so the original claim "rebuilt from scratch every frame" is wrong.

**Hot path:** `scene/host.rs:298` and `:373` call `layout_cache.begin_epoch()`
per frame; lookups from `presentation/layout/measure.rs:218`
(`store_measured`) and `presentation/layout/prepare.rs:59–66`
(`prepared`/`store_prepared`).

### Correction to the proposed fix

A `Vec` indexed by `view_id` does not type-check: `MeasureKey` includes
`width`, `intent`, and `Option<ViewId>` `component_view`, so there are many
entries per view per generation (one per measured width/intent combination).
Any dense replacement needs per-view buckets of (width, intent) entries, which
is more machinery than the HashMap it replaces. Only pursue if profiling shows
measure/prep hashing matters relative to measurement itself.

---

## 7. `RUNTIME_HANDLES` / feature-gated struct fields — DROPPED (wrong premise)

**Where:** `RUNTIME_HANDLES` at
`crates/iyon-native/src/tui/view_abi.rs:1349–1392`.

Two corrections:

- `packed_v3`, `packed_v4`, `fast_slots`, `fast_sessions` are behind
  `#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]`
  (`view_abi.rs:374–381` and matching initializers at `view_abi.rs:427–433`).
  They compile out of production builds entirely — there is no dead padding.
- The global registry mutex is acquired once per bootstrap and per
  pointer→handle resolution; negligible.

No action. Kept here only so the idea isn't re-proposed.

---

## 8. `NativeRefTable` pattern spread — `[independent]` — LOW

T2 replaced `HashMap<u32, NativeViewSlot>` with the paged
`NativeRefTable<12>`. Mechanical application targets (all in
`crates/iyon-native/src/tui/view_abi.rs`): `style_atoms` (:352), `styles`
(:353), and post-PERF-12 leftovers `path_nodes` (:348), `path_keys` (:349),
`edit_txns` (:351) — the latter three only if §3 concludes they survive at
all. Cleanup-tier work.

---

## 9. Perf-counter vs timing build discipline — procedural note, no code change

Counters are correctly feature-gated
(`crates/iyon-tui/src/perf.rs:8, 209`;
`crates/iyon-native/Cargo.toml:11–14`: `perf-counters` ⊂ `perf-packed-timing`
which enables `iyon-tui/perf-timing`). Inline `perf::inc` sites compile out in
timing builds. No change needed — just remember: **decision benchmarks run on
the timing build, counter builds are for ratios/hit-rates only.**

Suggest folding this rule into the PERF-12 tranche-3 benchmark protocol rather
than tracking it here.

---

## 10. Host raw-pointer validation at the FFI boundary — `[transport-coupled]` — MEDIUM (if FFI wins)

**Where:** `host_render_ref_impl`
(`crates/iyon-native/src/tui/view_abi.rs:1666–1695`): validates `runtime` via
`runtime_mut` (magic/thread-id check) but takes `host: *mut NativeHost` and
only checks null + `host.alive` (`:1677–1680`). Same pattern at
`view_abi.rs:2094`. The host object itself is `NativeTuiHost`
(`crates/iyon-native/src/tui.rs:645–651`, `alive: AtomicBool` there vs the
FFI-facing `NativeHost.alive: AtomicU32` at `view_abi.rs:339`).

A dangling host pointer reads freed memory before any check fires — genuine UB,
unlike the runtime path which has magic/version/alive defense-in-depth.

### Idea

Add a `u32 magic` (+ optionally a generation) at offset 0 of the FFI-facing
host struct and validate it alongside `alive`, mirroring the runtime's
`ABI_MAGIC`/`ABI_VERSION`/`owner_thread` pattern
(`view_abi.rs:370–381, 445–450`).

**Timing:** only relevant if the FFI transport wins PERF-12 (these exports
are the FFI entry surface). Cheap enough to fold into the winner-integration
work rather than doing speculatively now.

---

## Priority order (revised)

| # | Item | Status | Rating |
|---|------|--------|--------|
| 1 | Guard `PaintKey` construction behind `can_cache` (`view.rs:183–194`) | independent | HIGH, near-trivial |
| 2 | De-stringify `StyleContextKey` (atoms/borrowing, §2.1) | independent | HIGH |
| 3 | Paint-cache promotion without key clone (`view.rs:87–91`) | independent | MEDIUM |
| 4 | Merge `nodes`+`node_refs` into one semantic table (§5.1) | independent | MEDIUM |
| 5 | Host pointer magic validation (§10) | transport-coupled (FFI) | MEDIUM |
| 6 | Prune path/edit-txn machinery (§3) | transport-coupled | decide after PERF-12 |
| 7 | Paged tables for `style_atoms`/`styles` (§4, §8) | independent | LOW, opportunistic |
| 8 | LayoutCache dense structure (§6) | independent | LOW, profile-gated |
| — | `RUNTIME_HANDLES` / cfg-field padding (§7) | dropped | wrong premise |

Items 1–4 are safe to start during tranche 3 (they touch only `iyon-tui`
presentation internals and publication-time native tables, not the transport
surface being benchmarked). Items 5–6 wait for the PERF-12 verdict.
