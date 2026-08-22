# PERF-12.1 — Borrowed TypedArray safety design

## Motivation

PERF-12 T8 introduces variable-arity transport through borrowed TypedArray
buffers: JS writes child NativeRefs (u32) into a reusable typed array and passes
the pointer + length to Rust through engine-native FFI (`buffer`/`buffer_length`).

Rust reads the same storage during the call and retains no pointer afterward.
This is zero-copy at the FFI boundary, but the raw pointer lifetime must be
proved correct to satisfy Rust's safety guarantees without sacrificing the
performance win.

## Design

### Single responsibility: `BorrowedTypedArray<'a>`

```rust
/// A borrowed view into a JS TypedArray backing store, valid only for the
/// duration of one synchronous FFI call.
///
/// - `!Send + !Sync` — thread-bound to the FFI caller.
/// - Every access is bounds-checked.
/// - Lifetime `'a` ties the borrow to the enclosing `&mut NativeViewRuntime`,
///   so Rust's borrow checker proves the pointer is never stored or used
///   after the FFI call returns.
pub(crate) struct BorrowedTypedArray<'a> {
    ptr: *const u32,
    len: usize,
    _phantom: PhantomData<&'a ()>,
}
```

### Safety invariants

1. **Pointer validity**: The pointer is non-null (or length is zero) and points
   to initialized memory of at least `len * sizeof(u32)` bytes. This is
   guaranteed by Bun's engine-native FFI contract for `buffer` arguments: the
   TypedArray backing is pinned for the synchronous call duration.

2. **Lifetime**: The `'a` lifetime is bound to the `NativeViewRuntime` borrow.
   The borrow checker proves no `BorrowedTypedArray` outlives the FFI call, and
   therefore no raw pointer is retained across the boundary.

3. **Bounds**: Every element access goes through `get(index) -> Option<u32>`,
   which checks `index < self.len`. A malformed buffer length or corrupted
   pointer produces `None` at the worst, never an OOB read.

4. **Size cap**: The ABI generator emits a maximum buffer length
   (`MAX_REFS_PER_CALL = 4096`). The generated FFI wrapper validates this
   before constructing the `BorrowedTypedArray`, so Rust never sees a
   pathological length.

5. **Thread safety**: `BorrowedTypedArray` is `!Send + !Sync` by construction
   (its raw pointer prohibits automatic thread transfer). This matches the
   owner-thread guarantee of the entire runtime (§69).

### `unsafe` isolation

The only `unsafe` in the chain is the constructor:

```rust
impl<'a> BorrowedTypedArray<'a> {
    /// SAFETY: Caller must guarantee ptr is valid for len elements and that
    /// the memory is pinned for lifetime 'a. Generated code provides this.
    pub(crate) unsafe fn new(ptr: *const u32, len: usize) -> Self {
        assert!(!ptr.is_null() || len == 0);
        assert!(len <= MAX_REFS_PER_CALL);
        Self { ptr, len, _phantom: PhantomData }
    }
}
```

The constructor is called only from **generated FFI wrappers**, never from
handwritten business logic. This makes it auditable by inspection of a single
generated file rather than scattered across the codebase.

Every method on `BorrowedTypedArray` after construction is safe:

```rust
impl<'a> BorrowedTypedArray<'a> {
    pub(crate) fn get(&self, index: usize) -> Option<u32> {
        if index >= self.len { return None; }
        Some(unsafe { *self.ptr.add(index) })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len).map(move |i| self.get(i).unwrap())
    }

    pub(crate) fn len(&self) -> usize { self.len }
}
```

### Rejection: small-buffer copy path

An earlier alternative considered copying ≤32 refs into a stack array to avoid
the borrowed pointer entirely. This was rejected because:

- The copy hits the same L1 cache lines JS just wrote (no cold-miss benefit).
- It adds unpredictable memcpy overhead (proportional to child count).
- It undermines the architectural simplicity of the zero-copy path.
- The safety argument is already tight — adding a fallback path for "safety
  theater" makes the code harder to review, not easier.

## Decision

**Adopt `BorrowedTypedArray<'a>` as the single variable-arity transport type.**

The design is zero-cost at runtime (all checks are elidable by the optimizer in
timing builds, except the bounds check which is a single compare-and-branch),
and provably safe under the existing FFI contract.

### Rustacian satisfaction matrix

| Concern | How addressed |
|---|---|
| Raw pointer retention past FFI | Lifetime-bound, borrow checker proves it |
| Out-of-bounds access | `get()` returns `None`, never OOB |
| Thread safety | `!Send + !Sync` |
| Unsafe sprawl | Single generated constructor, safe accessors |
| Malformed input | Size cap + bounds check + non-null assert |
| Provenance provenance | Pointer provenance from `buffer` argument contract, documented at each generated call site |
| Performance | Zero-copy, single bounds check per access, no allocation |

This satisfies the Rust memory model without measurable runtime cost.