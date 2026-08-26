# PERF-9 — dual-lane packed transport: `Uint32Array` structure + `Uint8Array` UTF-8 arena

**Status:** proposed handoff after PERF-8  
**Repository:** `alexykn/iyon-tui`
**Required predecessor:** PERF-8 / Packed V3 must be implemented, correctness-complete, and benchmarked first  
**Historical reference baseline:** `84a7d117c777fbd5c2f0d5d072e63769be842e7c` (`test: bench 7v2`)  
**Scope:** string/payload transport and retained string storage; all PERF-7v2 and PERF-8 structural optimizations are mandatory invariants, not optional context  
**Decision:** benchmark first; production adoption only after the decision gate in this document

---

# 0. Executive conclusion

Yes: a two-lane packed transport is worth a dedicated PERF-9 experiment.

The target is:

```text
Uint32Array
    structural records
    refs
    scalar fields
    string references
    string offset table

+

Uint8Array / Buffer
    UTF-8 bytes for strings belonging ONLY to new/changed payload
```

But PERF-9 must not be implemented as the shallow transformation:

```text
string[]
→ encode every string to bytes
→ native converts every byte range back into a separate String
```

That would remove one N-API conversion layer while retaining much of the allocation/copy cost.

The optimal design is a complete string lane:

```text
immutable TS semantic graph
    ↓
PERF-8 identity / lineage / PersistentSeq cutoff
    ↓
only genuinely new string atoms are selected
    ↓
one reusable UTF-8 writer
    ↓
StringRef → monotonic offset table + dense byte arena
    ↓
one synchronous N-API call borrowing both typed arrays
    ↓
validate structure + UTF-8 once
    ↓
copy each transmitted byte at most once into retained native storage
    ↓
new Rust semantic strings point into immutable native slabs
```

The key performance target is not merely fewer wire bytes.

It is:

```text
remove:
    JS string[] creation/repopulation
    per-string N-API value conversion
    temporary Vec<String>
    repeated per-use Rust String allocation/copy
    repeated UTF-8 validation

while preserving:
    exact-root O(1) fast path
    O(changed path) retained updates
    O(log_B N) wide child edits
    weak native retention
    cache recovery
    immutable Rust Views
    one semantic N-API mutation
```

PERF-9 calls the resulting transport **Packed V4**.

---

# 1. PERF-9 is conditional on PERF-8

Do not implement PERF-9 against the current PERF-7v2 recursive encoder and then attempt to merge the two architectures later.

PERF-9 assumes PERF-8 has already established these invariants:

```text
PackedRef
transport generation
ABA-safe ref reassignment
one-word WireRef
flat topological records
construction-time PackedMeta
lineage-aware immutable PATCH records
TS PersistentSeq
Rust PersistentSeq
O(log_B N) wide sequence edits
paged weak packed slots
same environment semantic ViewBridgeCache
staged native decoding
one cache recovery, then hard failure
specialized exact-root call
forest fast path
```

If PERF-8 has not landed, PERF-9 waits.

Reason:

```text
PERF-8 answers:
    what semantic information must cross?

PERF-9 answers:
    how should variable-length textual information cross and live natively?
```

Solving them in the opposite order risks optimizing bytes that PERF-8 later stops sending entirely.

---

# 2. PERF-7v2 optimizations are permanent constraints

PERF-9 must preserve every relevant PERF-7v2 correction:

```text
[required]
full BridgeViewNode schema parity
full 53-bit NodeId correctness
same environment-local ViewBridgeCache
weak native View retention
weak/self-healing transport knowledge
cold recovery after native weak-cache expiry
one retry exactly
host mutation only after successful decode
DAG backward-reference correctness
malformed/cyclic input rejection
reusable structural scratch
explicit warmup
raw benchmark samples
construction + encoding + native timing
commit and forced-frame separation
full workload matrix
```

The string arena is not permission to weaken any of these.

---

# 3. PERF-8 optimizations are also permanent constraints

The byte lane must disappear completely when PERF-8 says no string payload is new.

Required behavior:

```text
IDENTICAL_IDENTITY:
    exact root ref call
    no Uint32Array transaction
    no Uint8Array argument
    no UTF-8 writer call
    no byte validation

SHARED_PATH without changed text:
    byte_count = 0

numeric decoration-only PATCH:
    byte_count = 0

wide parent one child replacement:
    structure = O(log_B N)
    bytes = only changed string payload

cold/rebuilt:
    bytes = O(new textual semantic payload)
```

Any design that re-encodes stable subtree strings because it owns a global byte arena has regressed PERF-8 and is rejected.

---

# 4. Why the current string path deserves a dedicated experiment

The PERF-7v2 packed native signature is effectively:

```rust
fn render_packed(
    words: Uint32Array,
    strings: Vec<String>,
)
```

This has two important costs before semantic reconstruction is complete:

```text
JS:
    build/dedupe string[]

N-API / napi-rs:
    materialize JS strings as owned Rust Strings

packed decoder:
    look strings up again
    many consumers call .to_owned()
```

Current decoder sites include owned reconstruction for:

```text
TextSpan text
Diff line text
overflow footer prefix
style-state keys
style-state values
theme strings
border glyphs
```

A byte lane can remove the high-level array/string conversion from N-API entirely.

But the native retained representation must also be reconsidered or the savings stop halfway.

---

# 5. Research result: Arrow's variable-width layout is the right wire primitive

Apache Arrow represents variable-size binary/string values using:

```text
offsets[0..N]
+
contiguous values bytes
```

For string `j`:

```text
start  = offsets[j]
length = offsets[j + 1] - offsets[j]
```

Offsets are monotonically increasing.

This is almost exactly the primitive PERF-9 needs.

The important adaptation is:

```text
Arrow:
    column slot index → offsets

Iyon:
    StringRef → offsets
```

This lets every structural string field remain one `u32` instead of consuming:

```text
offset
length
```

for every occurrence.

Reference:

- Apache Arrow Columnar Format, Variable-size Binary Layout: https://arrow.apache.org/docs/format/Columnar.html

---

# 6. Do not copy Arrow Binary View directly

Arrow also has `BinaryView` / `Utf8View`:

```text
short strings <= 12 bytes:
    inline in a 16-byte view

long strings:
    length
    4-byte prefix
    buffer index
    offset
```

That layout is excellent for analytics workloads where strings are repeatedly compared, selected, and gathered.

It is not automatically the best wire layout for Iyon.

For Iyon:

```text
most structural fields already want a compact u32 StringRef
transaction bytes are consumed once
retained Rust storage is separate from wire storage
```

A 16-byte view descriptor per string would often be more wire metadata than:

```text
one u32 StringRef at use site
+
one amortized u32 offset
```

Borrow the lesson about contiguous data and short-string/native storage experiments, not the exact format.

Reference:

- Apache Arrow Variable-size Binary View Layout: https://arrow.apache.org/docs/format/Columnar.html

---

# 7. Research result: SBE confirms the low-latency principles

Simple Binary Encoding's design principles emphasize:

```text
copy-free codec paths
allocation-free hot paths
native scalar mapping
streaming / forward-only access
word alignment
```

PERF-9 should follow those principles where the retained semantic lifetime allows it.

The important qualification is also present in SBE's design:

```text
if data must live after message processing,
then it must be stored separately
```

That is exactly Iyon's situation.

The JS typed arrays can be borrowed without copy during the synchronous N-API call, but Rust retained Views outlive that call.

Therefore the safe baseline is:

```text
zero-copy at boundary
+
one deliberate copy into retained native storage
```

not an unsafe promise that a JS-owned buffer remains immutable forever.

Reference:

- SBE Design Principles: https://github.com/aeron-io/simple-binary-encoding/wiki/Design-Principles

---

# 8. Research result: NAPI-RS gives the correct borrowed boundary

NAPI-RS documents borrowed typed-array/slice arguments such as:

```rust
&[u8]
&[u32]
Uint8ArraySlice<'env>
```

as zero-copy and lifetime-bound to the synchronous function scope.

That means the preferred native boundary is conceptually:

```rust
#[napi]
fn commit_packed_v4(
    ...,
    words: &[u32],
    utf8: &[u8],
) -> Result<...>
```

or the equivalent `Uint*ArraySlice` types if required by the binding shape.

Do not use an owned typed-array wrapper merely for convenience if it adds a JS reference/finalizer that the synchronous decoder does not need.

Reference:

- NAPI-RS TypedArray documentation: https://napi.rs/docs/concepts/typed-array

---

# 9. Borrowed JS memory must never become retained Rust memory implicitly

The native decoder may borrow:

```text
words
utf8 bytes
```

only until the N-API method returns.

Forbidden:

```rust
struct RetainedText {
    js_ptr: *const u8,
    len: usize,
}
```

unless there is a separately proven ownership-transfer protocol.

Default invariant:

```text
before native return:
    every byte required by a retained View
    must be owned by Rust/native storage
```

This is a memory-safety boundary, not merely a performance choice.

---

# 10. External ArrayBuffers are not the PERF-9 baseline

Node-API supports external ArrayBuffers, but its own documentation notes that some non-Node runtimes may reject external buffers and that the native backing memory must remain valid until the finalizer runs.

Bun compatibility and aliasing behavior would therefore become part of the architecture.

That is unnecessary for the first serious UTF-8 arena design.

PERF-9 baseline:

```text
JS owns reusable scratch arrays
native borrows synchronously
native retains its own immutable bytes
```

A native-owned/external buffer experiment is allowed later only if profiling shows the one retained copy is dominant.

Reference:

- Node-API external ArrayBuffer documentation: https://nodejs.org/api/n-api.html

---

# 11. JavaScript string semantics are a correctness problem, not an implementation detail

JavaScript Strings are sequences of UTF-16 code units and can contain unpaired surrogates.

`TextEncoder.encodeInto()` takes a Web IDL `USVString`, not an arbitrary code-unit-preserving `DOMString`.

Web IDL converts `USVString` to Unicode scalar values.

Therefore strings such as:

```ts
"\uD800" // lone high surrogate
"\uDC00" // lone low surrogate
```

require an explicit parity test against the existing direct bridge.

Do not assume:

```text
TextEncoder UTF-8
==
Bun Buffer UTF-8
==
Bun Node-API napi_get_value_string_utf8
```

for malformed UTF-16 until the oracle proves it.

References:

- WHATWG Encoding `TextEncoder`: https://encoding.spec.whatwg.org/
- Web IDL `USVString`: https://webidl.spec.whatwg.org/
- Node-API `napi_get_value_string_utf8`: https://nodejs.org/api/n-api.html

---

# 12. PERF-9 must define the canonical string semantic domain

There are two acceptable outcomes.

## Outcome A — all paths already agree

If direct N-API, `TextEncoder`, and Bun `Buffer.write(..., "utf8")` produce equivalent semantic Rust strings for all oracle cases:

```text
adopt the fastest byte writer
```

## Outcome B — they differ on malformed UTF-16

Then do not hide the difference inside PERF-9.

Choose one explicit action:

```text
1. reject the differing writer

or

2. make a separate correctness change that defines
   the runtime's canonical string normalization and
   applies it equally to direct and packed paths
```

A performance experiment must not silently change string semantics.

---

# 13. Required Unicode oracle before performance work

Test exact semantic parity for:

```text
""
ASCII
Latin-1
2-byte UTF-8 scalar
3-byte UTF-8 scalar
4-byte UTF-8 scalar
embedded U+0000
combining marks
precomposed vs decomposed text
emoji
multiple non-BMP characters
U+10FFFF
lone high surrogate
lone low surrogate
high surrogate + ASCII
ASCII + low surrogate
valid surrogate pair
high-high-low sequences
low-high sequences
long ASCII around buffer boundaries
long Unicode around buffer boundaries
```

For each input compare:

```text
A. direct bridge
B. TextEncoder arena
C. Bun Buffer arena
```

Compare the final Rust semantic value and rendered output, not only JS byte dumps.

---

# 14. UTF-8 wire bytes must always be valid

Packed V4 defines:

```text
byte lane = valid UTF-8 only
```

Native rejects invalid byte sequences.

Do not use lossy native decoding for malformed wire input.

Reason:

```text
JS writer is expected to emit canonical UTF-8
invalid UTF-8 therefore means:
    corrupted packet
    writer bug
    malicious/malformed input
```

Silently replacing invalid bytes would make malformed transport input semantically meaningful and weaken differential correctness.

Rust baseline validation:

```rust
let text = std::str::from_utf8(&utf8[..used_bytes])?;
```

Reference:

- Rust `std::str::from_utf8`: https://doc.rust-lang.org/std/str/fn.from_utf8.html

---

# 15. Validate UTF-8 once per transaction, not once per string

Do not call:

```rust
from_utf8(range_0)
from_utf8(range_1)
from_utf8(range_2)
...
```

The byte arena is one contiguous UTF-8 concatenation.

Validate:

```rust
let all = std::str::from_utf8(bytes_used)?;
```

once.

Then each StringRef needs only:

```text
range bounds valid
start/end are UTF-8 char boundaries
```

After those invariants hold, a retained slab accessor may use an internal unchecked conversion because the slab bytes are immutable and were validated before publication.

Keep the unsafe operation inside one tiny module with a documented proof invariant.

---

# 16. Packed V4 has two lanes, not three

Do not add a separate offset typed array.

Use:

```text
Lane 1: Uint32Array
    header
    structural records
    string offset table

Lane 2: Uint8Array / Buffer
    UTF-8 bytes
```

Why:

```text
one fewer N-API argument
one fewer typed-array object
one structural bounds domain
string references remain u32
```

The offset table naturally belongs to the word lane.

---

# 17. StringRef is one u32

Define:

```text
StringRef = u32

0:
    canonical empty string

1..N:
    transaction-local non-empty UTF-8 string
```

Optional/absent semantic fields remain controlled by their existing presence bit/tag.

Do not overload `StringRef=0` to mean both:

```text
empty
absent
```

where the schema distinguishes those states.

This keeps an ordinary string-bearing field the same structural width as the current string-table index.

---

# 18. String offset table

For `N` non-empty unique string entries:

```text
offset_count = N + 1

offsets[0] = 0
offsets[i + 1] >= offsets[i]
offsets[N] = used_bytes
```

StringRef `r` where `1 <= r <= N` resolves to:

```text
start = offsets[r - 1]
end   = offsets[r]
```

Since empty is encoded as ref 0, ordinary table entries should have:

```text
end > start
```

Reject zero-length entries so canonical encoding is unique.

---

# 19. Why offset table beats `(offset,length)` at every use site

Suppose one theme key is referenced 100 times in changed definitions.

`offset,length` representation:

```text
200 structural words
```

StringRef representation:

```text
100 use-site words
+ 2 offset words for one unique string
```

The more a string is referenced, the better the amortization.

It also decouples:

```text
semantic record grammar
from
byte storage location
```

which makes future slab/dictionary experiments possible without rewriting every node record.

---

# 20. Packed V4 transaction header

If PERF-8's final V3 header differs, adapt field positions without changing these semantics.

Recommended V4 header:

```text
word  field
----  --------------------------------------------------
0     PACKED_VIEW_MAGIC
1     PACKED_PROTOCOL_VERSION = 4
2     VIEW_BRIDGE_SCHEMA_VERSION
3     transaction flags
4     transport generation
5     used_words
6     used_bytes
7     root_count
8     record_count
9     records_end_word
10    string_count
11    string_offsets_start_word
12..  records
...   string offset table
```

Required relation:

```text
HEADER_WORDS <= records_end_word <= string_offsets_start_word
string_offsets_start_word + string_count + 1 == used_words
```

If V3 already needs additional header metadata, extend the header; do not remove V3 validation merely to fit twelve words.

---

# 21. Transaction flags

Reserve explicit flags for transport semantics.

At minimum:

```text
RESET_GENERATION
HAS_UTF8
COLD_CLOSURE
```

Rules:

```text
used_bytes == 0
    ⇒ HAS_UTF8 must be 0

HAS_UTF8 == 0
    ⇒ string_count == 0
    ⇒ used_bytes == 0

COLD_CLOSURE
    ⇒ no persistent WireRefs
```

Do not infer protocol mode from accidental counts when a flag materially changes decoder semantics.

---

# 22. Exact-root path remains outside Packed V4

PERF-8's best exact-root path remains authoritative:

```ts
native.renderPackedRef(generation, packedRef)
```

It must not become:

```ts
native.renderPackedV4(emptyWords, emptyBytes)
```

The correct exact-identity trace is:

```text
JS:
    root PackedRef already published

N-API:
    generation
    root ref

Rust:
    packed slot lookup
    WeakView upgrade
    host commit
```

Required counters:

```text
utf8_encoder_calls = 0
utf8_bytes_written = 0
word_transaction_calls = 0
```

---

# 23. Forest fast path also remains byte-free when possible

For an animation/forest of already-published roots:

```text
reusable Uint32Array<PackedRef>
+ no byte lane
```

Only a mixed forest containing new definitions enters the V4 transaction compiler.

Never emit empty UTF-8 buffers as a ritual argument on the fully-known fast path if a specialized native method avoids them.

---

# 24. Empty strings cost zero byte-lane space

Canonical empty string:

```text
StringRef = 0
```

Do not append:

```text
offset x
offset x
```

for every empty value.

Do not call the UTF-8 encoder for empty strings.

This matters for payloads containing optional labels or empty text spans.

---

# 25. Embedded NUL is ordinary data

Unlike C-string-oriented formats, Packed V4 is length-delimited.

Therefore:

```text
"a\0b"
```

must encode as the UTF-8 bytes:

```text
61 00 62
```

and round-trip exactly.

Do not add trailing NUL bytes to arena entries.

FlatBuffers and Cap'n Proto use terminators for their own access conventions; Iyon does not need them because every string range has explicit boundaries.

This saves one byte per unique string and keeps `U+0000` unambiguous.

---

# 26. 32-bit arena limits are explicit

Packed V4 string offsets are `u32`.

Therefore one transaction must satisfy:

```text
used_bytes <= 0xffff_ffff
string_count < 0xffff_ffff
```

In practice Bun/JS typed-array limits will usually be smaller.

Do not rely on engine failure as protocol validation.

Fail with a clear transport error before arithmetic wraps.

A multi-gigabyte View transaction is pathological; correctness still requires a defined failure mode.

---

# 27. No offset arithmetic with signed JS bitwise operators

Byte offsets may approach the upper u32 range.

Use arithmetic-number operations with explicit range checks:

```ts
function assertU32(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new RangeError("packed byte offset must fit u32");
  }
  return value;
}
```

Do not use:

```ts
offset | 0
```

because it turns high u32 values negative.

---

# 28. JS owns one reusable byte writer

Conceptual interface:

```ts
interface Utf8Writer {
  reset(): void;
  append(value: string): { readonly start: number; readonly end: number };
  finish(): { readonly bytes: Uint8Array; readonly usedBytes: number };
  readonly capacity: number;
}
```

Requirements:

```text
geometric growth
copy used prefix only when growth occurs
no allocation per transaction after high-water warmup
no retained semantic objects stored in writer
no stable subtree traversal
```

The writer is transaction scratch, not a semantic cache.

---

# 29. PERF-9 must benchmark the JS UTF-8 writer, not assume `TextEncoder`

Two serious candidates exist in Bun.

## E1 — `TextEncoder.encodeInto`

Standard API:

```ts
encoder.encodeInto(source, destination)
```

Advantages:

```text
WHATWG-defined UTF-8 semantics
portable
writes into caller-owned memory
reports read/written progress
```

Disadvantage for an append arena:

```text
there is no destination offset parameter
```

so appending generally requires a destination view such as:

```ts
bytes.subarray(cursor)
```

or another `Uint8Array` view object.

## E2 — Bun/Node `Buffer.write`

```ts
buffer.write(source, cursor, available, "utf8")
```

Advantages:

```text
writes directly at byte offset
returns bytes written
no per-string subarray required
Buffer is Uint8Array-compatible
```

Bun documents that a partial write does not emit a partial encoded character.

PERF-9 must benchmark both.

References:

- Bun `TextEncoder.encodeInto`: https://bun.sh/reference/globals/TextEncoder/encodeInto
- Bun `Buffer.write`: https://bun.sh/reference/node/buffer/Buffer

---

# 30. Do not use `TextEncoder.encode()` in the steady path

Forbidden steady-state pattern:

```ts
const encoded = encoder.encode(value);
writer.pushBytes(encoded);
```

That allocates a new `Uint8Array` for every string.

PERF-9 exists specifically to create one reusable arena.

`encode()` may remain in oracle tests, never in the production candidate encoder.

---

# 31. Avoid an unconditional `Buffer.byteLength()` pre-pass

Naive Buffer writer:

```ts
const bytes = Buffer.byteLength(value, "utf8");
ensure(bytes);
const written = buffer.write(value, cursor, bytes, "utf8");
```

This can scan/transcode-related state twice.

Use a capacity fast path.

For JavaScript UTF-16 code units, a safe upper bound for UTF-8 scalar-value encoding is:

```text
3 * value.length bytes
```

because:

```text
BMP non-surrogate code unit <= 3 bytes
lone surrogate replacement <= 3 bytes
valid surrogate pair = 4 bytes for 2 code units
```

Therefore if:

```text
available >= 3 * value.length
```

there is definitely enough capacity for the complete string under normal UTF-8 replacement semantics.

The bound itself must be oracle-tested against the selected Bun writer semantics.

---

# 32. Recommended Bun Buffer fast path

Candidate algorithm:

```ts
append(value: string): Range {
  if (value.length === 0) return EMPTY_RANGE;

  const available = this.buffer.length - this.cursor;
  const conservativeMax = 3 * value.length;

  if (Number.isSafeInteger(conservativeMax) && available >= conservativeMax) {
    const written = this.buffer.write(
      value,
      this.cursor,
      available,
      "utf8",
    );

    const start = this.cursor;
    this.cursor += written;
    return { start, end: this.cursor };
  }

  const exact = Buffer.byteLength(value, "utf8");
  this.ensure(exact);

  const start = this.cursor;
  const written = this.buffer.write(value, this.cursor, exact, "utf8");

  if (written !== exact) {
    throw new Error("packed UTF-8 writer produced a truncated string");
  }

  this.cursor += written;
  return { start, end: this.cursor };
}
```

This is a candidate, not a decree.

Benchmark it against the TextEncoder implementation.

---

# 33. TextEncoder writer must handle partial progress correctly

If `TextEncoder.encodeInto` remains a candidate, implement its growth path according to the API's `read` / `written` contract.

Pseudo-code:

```ts
while (read < source.length) {
  let destination = this.remainingView();
  let result = encoder.encodeInto(source.substring(read), destination);

  cursor += result.written;
  read += result.read;

  if (read !== source.length) grow();
}
```

But measure the allocations created by:

```text
substring
subarray / typed-array views
result objects
```

The standard guarantees correctness, not that this particular append shape is Bun's fastest path.

The WHATWG spec itself shows a similar grow-and-resume pattern.

Reference:

- WHATWG Encoding Standard: https://encoding.spec.whatwg.org/

---

# 34. Do not write a hand-rolled JavaScript UTF-8 encoder

Rejected:

```ts
for each UTF-16 code unit:
    manually emit UTF-8
```

Reasons:

```text
surrogate correctness risk
more JS branches
less likely to beat runtime-native transcoding
larger maintenance surface
harder parity with direct N-API semantics
```

Use runtime/native encoders and benchmark their call shape.

---

# 35. `Buffer` may be the storage object even if `TextEncoder` wins

A Bun `Buffer` is compatible with typed-array byte storage.

It is valid to benchmark combinations such as:

```text
storage: Buffer
writer: Buffer.write

storage: Uint8Array
writer: TextEncoder.encodeInto

storage: Buffer
writer: TextEncoder.encodeInto on views
```

Do not conflate:

```text
backing storage type
with
UTF-8 conversion primitive
```

The N-API side should see only a `Uint8Array`-compatible borrowed byte span.

---

# 36. String selection happens after PERF-8 reachability cutoff

The transaction compiler must never do:

```text
walk all semantic strings
→ then decide which parent View records are retained
```

Correct order:

```text
start from changed/unpublished PERF-8 roots
    ↓
follow only dependencies that need DEF/PATCH publication
    ↓
visit PackedRecipe string operands in those records
    ↓
assign StringRefs
    ↓
encode those strings only
```

This preserves the fundamental retained invariant:

```text
stable semantic object
→ zero string work
```

---

# 37. String metadata belongs in construction-time PackedMeta

PERF-8 already moves semantic lowering out of commit.

Extend that idea.

Do not make the commit compiler rediscover which fields are strings by walking arbitrary semantic objects.

A recipe should already contain explicit string operands:

```ts
type PackedStringClass =
  | "bulkText"
  | "semanticKey"
  | "glyph"
  | "label";

interface PackedStringAtom {
  readonly value: string;
  readonly class: PackedStringClass;

  visitEpoch: number;
  localStringRef: number;
}
```

Example text recipe:

```ts
interface PackedTextRecipe {
  readonly kind: TEXT;
  readonly wrap: number;
  readonly align: number;
  readonly spans: readonly {
    readonly text: PackedStringAtom;
    readonly style: CanonicalStyle;
  }[];
}
```

Commit compiles known transport operands; it does not inspect public/high-level View shape.

---

# 38. Do not mutate JavaScript primitive strings

Transport sidecars belong to objects, not primitive string values.

Do not attempt:

```ts
WeakMap<string, ...>
```

WeakMap keys must be objects.

Instead semantic construction creates or associates a small private `PackedStringAtom` object where transport identity is useful.

That object remains reachable only while the semantic recipe that owns/reuses it remains reachable.

---

# 39. String identity and string equality are different optimization signals

Two cases:

```text
A:
    exact same PackedStringAtom object

B:
    different atoms containing equal JS string content
```

A gives free identity deduplication.

B requires content comparison/hash lookup if we want to avoid duplicate bytes.

PERF-9 must not silently drop current V2's content deduplication without measuring the consequence.

Current V2 uses:

```ts
Map<string, number>
```

which deduplicates equal string contents within one transaction.

That can matter heavily for:

```text
repeated theme keys
style-state keys/values
border glyphs
repeated prefixes
styled-span-heavy workloads
```

---

# 40. Benchmark string deduplication policy explicitly

Run at least these policies.

## D0 — content dedupe all

```text
identity epoch first
then Map<string, StringRef> for all non-empty values
```

Closest to current V2 wire-byte behavior.

## D1 — hybrid short/metadata content dedupe

```text
identity epoch first

if class != bulkText
    content Map
else if value.length <= threshold
    content Map
else
    identity only
```

Initial threshold candidates:

```text
16
32
64
128 UTF-16 code units
```

## D2 — identity only

```text
same PackedStringAtom dedupes
distinct equal values encode independently
```

Fastest lookup policy, potentially worst wire/memory behavior.

## D3 — all content dedupe without PackedStringAtom identity

Control that quantifies whether atom metadata itself pays off.

Choose from total cost, not bytes alone.

---

# 41. Why hybrid dedupe is likely worth testing

Large text strings are commonly unique.

For a 4 KiB unique text span, inserting it into a content Map may add hashing/table work without reducing the byte arena.

Small semantic strings are the opposite:

```text
"focused"
"selected"
"markdown.h2"
"cyan"
box-drawing glyphs
repeated Diff markers/prefixes
```

They can repeat many times, and avoiding repeated native retained copies is valuable.

Therefore the optimal policy may depend on string class/size.

Do not hard-code that conclusion without the matrix in this document.

---

# 42. StringRef assignment algorithm

Pseudo-code:

```ts
class TransactionStringTable {
  private epoch = nextEpoch();
  private content = new Map<string, number>();
  private atoms: PackedStringAtom[] = [];

  ref(atom: PackedStringAtom): number {
    if (atom.value.length === 0) return 0;

    if (atom.visitEpoch === this.epoch) {
      return atom.localStringRef;
    }

    if (this.shouldContentDedupe(atom)) {
      const existing = this.content.get(atom.value);
      if (existing !== undefined) {
        atom.visitEpoch = this.epoch;
        atom.localStringRef = existing;
        return existing;
      }
    }

    const next = this.atoms.length + 1;
    this.atoms.push(atom);

    if (this.shouldContentDedupe(atom)) {
      this.content.set(atom.value, next);
    }

    atom.visitEpoch = this.epoch;
    atom.localStringRef = next;
    return next;
  }
}
```

The exact implementation must handle epoch rollover safely, just like PERF-8 visit epochs.

---

# 43. Do not retain a global strong JavaScript string interner

Rejected production shortcut:

```ts
const allStringsEver = new Map<string, PackedStringAtom>();
```

That creates lifetime retention proportional to historical content.

Transaction-local content maps are safe because they are cleared/reused per transaction.

A bounded interner may be experimented with only for known framework/static metadata and only with explicit memory caps.

---

# 44. Static protocol vocabulary should not consume byte arena space

Anything already represented by a numeric canonical schema tag stays numeric.

Examples:

```text
node kind
wrap mode
alignment
color kind
standard ANSI color
border style
layout track type
attribute bits
```

Do not regress these back into strings because the byte lane exists.

The byte arena is for semantically arbitrary text only.

---

# 45. Theme keys remain strings unless semantics change independently

Do not hash arbitrary theme keys into lossy IDs merely for PERF-9.

A theme key is semantic application text.

Wire representation:

```text
StringRef
```

A separate symbol/interning architecture could later assign stable native IDs, but that changes retention/lifetime semantics and must be justified independently.

PERF-9 must not couple correctness to hash collision behavior.

---

# 46. Style-state key/value strings remain exact

Style state semantics currently compare textual keys/values.

Packed V4 must preserve:

```text
exact Unicode string value
empty/non-empty validity rules already enforced by semantic API
ordering/canonicalization rules from PERF-8
```

Do not hash them on the wire as the authoritative representation.

Content dedupe may reduce repeated bytes without changing semantics.

---

# 47. Border glyphs are especially important Unicode fixtures

Custom border glyphs can contain Unicode box-drawing or other one-cell strings.

PERF-9 tests must cover:

```text
ASCII glyphs
box drawing U+2500 family
non-BMP input that should fail semantic one-cell validation if currently invalid
grapheme clusters if accepted/rejected by current API
embedded NUL where semantically accepted
```

Transport parity must come after existing semantic validation, not replace it.

---

# 48. Packed V4 does not normalize Unicode

Never perform NFC/NFD normalization in the transport.

These are distinct JS strings:

```text
"é"
"e\u0301"
```

If the direct semantic path preserves the distinction, V4 preserves it too.

UTF-8 encoding is representation conversion, not semantic normalization.

---

# 49. Native V4 entrypoint must borrow both lanes synchronously

Recommended shape after PERF-8 production method names are known:

```rust
#[napi(js_name = "tuiPackedCommitV4")]
pub fn packed_commit_v4(
    &self,
    env: Env,
    words: &[u32],
    utf8: &[u8],
) -> Result<()> {
    ...
}
```

If NAPI-RS requires explicit slice wrapper types for the exact version in the repo:

```rust
Uint32ArraySlice<'_>
Uint8ArraySlice<'_>
```

is equally acceptable.

Invariant:

```text
no Vec<u32> conversion
no Vec<u8> conversion merely to enter decoder
```

---

# 50. Never accept `Vec<u8>` as the hot N-API argument without proving it is borrowed

A convenient binding such as:

```rust
utf8: Vec<u8>
```

may cause allocation/copy conversion before decoder timing begins.

The PERF-9 code review must verify the generated/binding semantics.

The intended boundary is:

```text
JS typed-array backing store
    ↓ borrowed slice
Rust decoder
```

not:

```text
JS typed array
    ↓ copy
Rust Vec
    ↓ decoder
```

---

# 51. Header validation happens before string access

Validate in this order:

```text
1. structural typed-array length
2. magic
3. protocol version
4. bridge schema version
5. flags
6. generation
7. used_words
8. used_bytes
9. record bounds
10. string_count
11. offset-table position/size
12. byte-lane length
13. UTF-8
14. offset monotonicity/boundaries
15. records / refs / semantic payloads
```

No record may obtain a string slice before the table and lane are globally validated.

---

# 52. Complete offset-table validation

Required native checks:

```rust
fn validate_offsets(
    offsets: &[u32],
    used_bytes: usize,
    all_utf8: &str,
) -> Result<()> {
    if offsets.first().copied() != Some(0) {
        return Err(...);
    }

    let mut previous = 0usize;

    for &raw in offsets {
        let current = usize::try_from(raw)?;

        if current < previous || current > used_bytes {
            return Err(...);
        }

        if !all_utf8.is_char_boundary(current) {
            return Err(...);
        }

        previous = current;
    }

    if previous != used_bytes {
        return Err(...);
    }

    Ok(())
}
```

If StringRef 0 is canonical empty and table entries are non-empty, additionally require:

```text
offsets[i + 1] > offsets[i]
```

for every table entry.

---

# 53. Full-lane UTF-8 validation plus boundary checks is stronger and cheaper

A globally valid UTF-8 byte sequence can still be sliced at an invalid code-point boundary.

Example:

```text
arena bytes contain a 3-byte scalar
malformed offset points at byte 2
```

Therefore PERF-9 needs both:

```text
from_utf8(entire lane)
+
is_char_boundary(each offset)
```

Do not infer that monotonic offsets alone make individual strings valid.

---

# 54. The byte lane must not contain unused garbage in the validated range

Writer may retain high-water capacity:

```text
capacity = 1 MiB
used_bytes = 17 KiB
```

Native operates only on:

```rust
&utf8[..used_bytes]
```

Bytes after `used_bytes` are irrelevant stale scratch and must never be validated, hashed, copied, or inspected.

Same principle as PERF-7v2 `used_words`.

---

# 55. Structural word scratch also remains reusable

PERF-9 must not undo PERF-7v2/PERF-8 structural reuse.

Steady state after warmup:

```text
word_buffer_grows = 0
utf8_buffer_grows = 0
transaction_word_allocations = 0
transaction_byte_allocations = 0
```

Growth is geometric and only copies used data.

---

# 56. The native retained string design is part of PERF-9

There are three meaningful native storage candidates.

## R0 — per-string owned control

```rust
String / Box<str>
```

Each unique transmitted string becomes its own allocation.

Useful baseline; simple but does not exploit the arena after crossing.

## R1 — one slab per transaction

```text
copy entire used byte lane once
all retained strings reference ranges in one Arc slab
```

Minimum copy/allocation count, potential severe lifetime amplification.

## R2 — paged immutable slabs

```text
copy unique strings into bounded native pages
retained strings reference page + range
large strings get dedicated pages
```

More page-management work, bounded lifetime amplification.

R2 is the recommended production target if it wins end-to-end.

---

# 57. Why native per-string `String` is not the optimal endpoint

If native receives:

```text
one contiguous validated byte lane
```

then doing:

```rust
for every StringRef:
    String::from(slice)
```

causes:

```text
one allocation per unique string
one copy per unique string
allocator metadata
fragmentation
more destructor work
```

It still avoids N-API `Vec<String>` conversion and duplicate decoder copies, so it is a useful control.

But PERF-9 should test whether one/few native byte allocations can back many strings.

---

# 58. Do not blindly replace Rust `String` with `Arc<str>`

Rust's standard library documents:

```text
From<String> for Arc<str>
    allocates a reference-counted str
    and copies the String contents
```

So changing all semantic fields to `Arc<str>` can add a copy for ordinary Rust-native construction.

PERF-9 must optimize the JS bridge without making native Rust APIs worse.

Reference:

- Rust `Arc<str>` conversions: https://doc.rust-lang.org/std/sync/struct.Arc.html

---

# 59. Recommended retained string abstraction

Introduce one private semantic string abstraction rather than changing public API types.

Approximate shape:

```rust
#[derive(Clone)]
pub(crate) enum RetainedStr {
    Static(&'static str),
    Owned(Box<str>),
    Shared(SlabSlice),
}

#[derive(Clone)]
pub(crate) struct SlabSlice {
    slab: Arc<Utf8Slab>,
    start: u32,
    len: u32,
}

pub(crate) struct Utf8Slab {
    bytes: Vec<u8>,
}
```

The exact enum layout must be measured with:

```rust
size_of::<RetainedStr>()
size_of::<String>()
size_of::<TextSpan>()
size_of::<DiffLine>()
```

Do not assume niche/layout optimization.

---

# 60. Why `Box<str>` is the native-owned variant

For an already-owned Rust `String`, converting to `Box<str>` can reuse the String allocation subject to standard implementation behavior, unlike `Arc<str>` which the standard documentation explicitly says copies.

Even if exact allocator behavior changes, `Box<str>` expresses the intended semantics:

```text
one unique owner
immutable text
```

Use it as the ordinary Rust-native representation candidate.

Do not expose `Box<str>` publicly; keep API ergonomics unchanged.

---

# 61. `RetainedStr` must behave exactly like textual value semantics

Implement/derive semantics through `as_str()`:

```text
Clone
PartialEq
Eq
Hash
Debug
Display where useful
Borrow<str>
AsRef<str>
```

Equality must be content equality.

Never make slab identity part of semantic equality.

Two equal strings in different slabs are semantically equal.

---

# 62. `RetainedStr::as_str()` safety invariant

For `Shared`:

```rust
impl SlabSlice {
    fn as_str(&self) -> &str {
        let start = self.start as usize;
        let end = start + self.len as usize;
        let bytes = &self.slab.bytes[start..end];

        // SAFETY:
        // 1. Utf8Slab is immutable after construction.
        // 2. Entire slab was assembled only from previously validated UTF-8 ranges.
        // 3. start/end were established on UTF-8 boundaries.
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }
}
```

If code cannot make these invariants mechanically obvious, use checked `from_utf8` and accept the small cost.

Unsafe is justified only to remove a validation that has already been proven once.

---

# 63. Slabs must be immutable after publication

Forbidden:

```text
Arc<Utf8Slab>
+
interior mutable Vec that later appends
```

Once any `SlabSlice` is published into a semantic View:

```text
its backing bytes never change
its length never changes
its allocation never moves
```

A builder owns mutable pages.

Freezing a page transfers it into immutable retained storage.

---

# 64. Recommended paged slab builder

Initial candidate:

```text
PAGE_TARGET = 64 KiB
LARGE_STRING_THRESHOLD = PAGE_TARGET / 2
```

Pseudo-code:

```rust
struct SlabBuilder {
    current: Vec<u8>,
    frozen: Vec<Arc<Utf8Slab>>,
}

fn retain_string(&mut self, value: &str) -> SlabSlice {
    if value.len() > LARGE_STRING_THRESHOLD {
        return dedicated_slab(value);
    }

    if self.current.capacity() - self.current.len() < value.len() {
        self.freeze_current();
        self.current = Vec::with_capacity(PAGE_TARGET);
    }

    let start = self.current.len();
    self.current.extend_from_slice(value.as_bytes());
    let len = value.len();

    // A provisional location must be resolved to its frozen Arc before
    // publishing semantic objects.
    ...
}
```

Do not publish pointers into a mutable Vec that may reallocate.

---

# 65. Simpler and safer page construction: plan first, copy second

Because the offset table already gives every string length, native can plan slab placement before copying.

Recommended staged algorithm:

```text
Pass 1:
    for each unique StringRef
        choose page index
        choose page offset
        accumulate exact page used sizes

Allocate:
    Vec<u8> for each planned page with exact/capped capacity

Pass 2:
    copy each string range exactly once into destination page

Freeze:
    convert each page builder into Arc<Utf8Slab>

Build:
    NativeStringTable[StringRef] = SlabSlice
```

This avoids page-pointer instability and makes total copied bytes exactly measurable.

---

# 66. Dedicated slabs for large strings

Do not let one 2 MiB text block force a 2 MiB general page whose tiny neighboring string keeps the whole page alive.

Rule candidate:

```text
if string_bytes >= LARGE_STRING_THRESHOLD:
    dedicated slab sized to that string
```

Sweep thresholds in benchmarks.

Useful candidate pairs:

```text
page 16 KiB / large 8 KiB
page 32 KiB / large 16 KiB
page 64 KiB / large 32 KiB
page 256 KiB / large 128 KiB
```

Production choice is memory/CPU evidence driven.

---

# 67. Why one transaction-wide slab is dangerous

Suppose a cold transaction transmits 10 MiB of text.

Only one 12-byte label remains reachable after later updates.

With one slab:

```text
12 live bytes
→ retain 10 MiB allocation
```

That is unacceptable lifetime amplification for a retained UI.

PERF-9 must measure:

```text
live_payload_bytes
retained_slab_bytes
amplification = retained_slab_bytes / live_payload_bytes
```

Paged slabs bound the normal small-string amplification to roughly a page-scale quantity per partially live page.

---

# 68. Page packing order matters

If strings are copied in transaction encounter order, unrelated lifetimes may share a page.

PERF-9 should test whether class-aware packing improves retention:

```text
bulk text
metadata keys
labels/glyphs
```

However do not sort by content or class if doing so adds more CPU than it saves.

Candidate policies:

```text
P0 encounter order
P1 large dedicated + remaining encounter order
P2 class buckets + size threshold
```

P1 is the recommended baseline.

---

# 69. Do not keep the transaction's native string table alive

After semantic objects are built, discard temporary lookup structures:

```text
Vec<SlabSlice> indexed by local StringRef
```

unless a semantic object owns clones of needed slices.

The transaction decoder must not retain:

```text
all strings just because they appeared in one packet
```

Only semantic graph reachability should retain native slabs through `RetainedStr` handles.

---

# 70. Slab ownership must preserve weak View cache semantics

Packed slot/cache retention remains weak at the View level.

If the last strong View containing a `RetainedStr::Shared` dies:

```text
ViewNode dies
RetainedStr handles die
Arc<Utf8Slab> count decreases
slab dies when no other live strings reference it
WeakView cache entry remains non-owning and can expire
```

Do not put strong slabs in the environment cache separately from semantic ownership.

That would recreate a lifetime leak through a different layer.

---

# 71. Static Rust strings should remain static

Framework-native constants can use:

```rust
RetainedStr::Static("...")
```

without an allocation.

Do not copy static Rust strings into slabs merely to unify representation.

The packed path creates `Shared` strings; ordinary native constructors can choose `Static` or `Owned` naturally.

---

# 72. Which Rust semantic fields should migrate

Inventory every arbitrary retained string field in the Rust View semantic path.

At minimum inspect:

```text
TextSpan.text
DiffLine text
OverflowIndicator::Footer prefix
ThemeKey
StyleStateKey
StyleStateValue
BorderGlyphs fields
Border top label if it crosses the JS bridge
any style facts/state textual atoms
any future semantic labels introduced before PERF-9 implementation
```

Do a code search at implementation time.

Do not partially migrate text while leaving high-frequency metadata on a duplicate-copy path unless benchmarks explicitly justify it.

---

# 73. Public Rust API must not become slab-aware

Do not expose:

```rust
pub fn text(RetainedStr)
```

to ordinary users.

Keep public ergonomics such as:

```rust
View::text(impl Into<String>)
StyleStateKey::new(impl Into<String>)
```

or evolve them only in a separate API simplification.

Internally convert into the retained representation.

PERF-9 is a private storage/bridge change.

---

# 74. Native Rust construction regression is a first-class guardrail

Because `RetainedStr` changes private Rust storage, benchmark pure Rust construction too.

Required controls:

```text
native View::text short string
native View::text long string
native styled text many spans
native Diff construction
native style state construction
clone retained Views
semantic equality
hash/select style state
```

Candidate B is rejected if JS bridge wins are bought by a meaningful regression in native-only hot paths.

---

# 75. `bytes::Bytes` is a reference point, not an automatic dependency

The Rust `bytes::Bytes` type demonstrates a useful model:

```text
cheap clone
O(1) slicing
shared contiguous backing memory
```

But PERF-9's retained text handles are extremely numerous and domain-specific.

Benchmark a custom compact slab slice first.

Do not add `bytes` merely because it already provides slicing if its handle size/indirection is worse for `TextSpan`-heavy layouts.

Reference:

- `bytes::Bytes`: https://docs.rs/bytes/latest/bytes/struct.Bytes.html

---

# 76. Native StringRef table should be dense

StringRef values are dense transaction-local integers.

Use:

```rust
Vec<RetainedStr>
```

or equivalent dense storage while constructing the transaction.

Do not use:

```rust
HashMap<u32, RetainedStr>
```

for ordinary lookup.

Resolution should be approximately:

```rust
if string_ref == 0 {
    empty_retained_str()
} else {
    table[(string_ref - 1) as usize].clone()
}
```

Bounds-check once per use in safe code, or validate record refs during staging and use indexed access in a later trusted stage if profiling justifies it.

---

# 77. Avoid cloning bytes when cloning StringRefs

`RetainedStr::Shared` clone must be:

```text
Arc increment
+ copy start/len scalars
```

not:

```text
allocate/copy string bytes
```

This matters because one local StringRef may appear in multiple semantic fields.

The byte arena's content deduplication only pays fully if native reuse also remains cheap.

---

# 78. String table construction can itself dedupe native retained handles

If JS emits one unique StringRef and uses it 100 times:

```text
copy bytes into slab once
construct one table handle
clone handle 100 times into semantic objects
```

Do not copy the source bytes 100 times merely because 100 fields reference the StringRef.

This is the central reason to resolve StringRefs through a native table rather than construct directly from the byte range at every record field.

---

# 79. Decoder stages should separate validation, retention, and semantic build

Recommended stages:

```text
Stage 0 — boundary borrow
    words: &[u32]
    bytes: &[u8]

Stage 1 — header/section validation

Stage 2 — UTF-8 + offset validation

Stage 3 — WireRef / record structural validation

Stage 4 — build native retained string table

Stage 5 — resolve packed refs / local refs

Stage 6 — build immutable Views/PersistentSeq chunks/PATCH results

Stage 7 — publish weak semantic + packed cache entries

Stage 8 — mutate host exactly once
```

If any stage fails before Stage 8:

```text
host state is unchanged
```

---

# 80. Cache miss recovery remains exactly PERF-8

A stale persistent `PackedRef` may fail to upgrade.

Normal path:

```text
V4 transaction
→ persistent ref miss
→ PACKED_CACHE_MISS
```

JS response:

```text
advance/reset transport generation according to PERF-8
cold-encode current closure
retry once
```

The cold retry's byte lane includes only the strings required by that cold semantic closure.

A second cache miss after a correct cold closure remains a hard protocol error.

The existence of a UTF-8 arena does not add another retry layer.

---

# 81. Cold recovery must not retain the first failed byte lane

If the first optimistic transaction fails on a stale ref:

```text
borrowed words/bytes are discarded when the call returns
staged native pages/strings from that failed attempt are dropped
no host mutation occurs
no cache publication occurs for semantically incomplete transaction state
```

Then JS reuses scratch memory for the cold retry.

Do not retain slabs from failed speculative packets.

---

# 82. Packed V4 cache publication remains transactional

Do not insert a newly built View/sequence/string-owned semantic object into persistent packed slots before the complete transaction has passed all recoverable failure points.

Recommended:

```text
staged_publications: Vec<Publication>
```

Only after all roots decode successfully:

```text
publish weak packed slots
publish/update semantic NodeId weak cache
host commit
```

If host commit itself can fail, define whether cache publication occurs immediately before or after it according to PERF-8's established atomicity contract, and test the behavior.

Strings themselves are ordinary owned dependencies; they do not need an independent global cache.

---

# 83. Do not introduce a persistent `StringRef` protocol in PERF-9

Tempting design:

```text
string once
→ assign environment-global string id
→ future transactions send id
```

Do not make this the baseline.

PERF-8 already ensures:

```text
stable semantic nodes are not retransmitted
```

Therefore a stable string inside a stable node already costs zero.

Persistent string IDs only help when equal string content appears in genuinely new semantic nodes.

That may be worth a future intern-table experiment, but it adds:

```text
lifetime synchronization
weak/strong string cache policy
ID generation/ABA
content hashing
miss recovery
memory-pressure policy
```

First measure transaction-local dedupe + slab retention.

---

# 84. Optional persistent string dictionary is a late experiment only

If PERF-9 traces show a dominant pattern such as:

```text
thousands of new Text nodes
all repeat the same theme/style-state/glyph strings
```

then benchmark a small bounded native/JS dictionary for metadata strings.

Constraints:

```text
never for arbitrary bulk text by default
bounded entries/bytes
explicit generation
collision-free textual verification
memory-pressure eviction
miss recovery
no global strong retention of application text
```

Call this PERF-9.x, not core V4.

---

# 85. Do not compress the UTF-8 arena

General compression such as:

```text
zstd
lz4
brotli
```

is wrong for the synchronous in-process hot path unless later evidence is extraordinary.

Reasons:

```text
memory bandwidth is local
payloads are often small
compression adds CPU/latency variance
retained updates already omit stable data
```

PERF-9 optimizes conversion and ownership, not network bandwidth.

---

# 86. Do not varint ordinary string offsets

The word lane is deliberately `Uint32Array` and native-friendly.

Do not replace the offset table with variable-length byte integers.

That would add:

```text
branchy decode
unaligned parsing
another byte grammar
less direct indexing
```

for small metadata savings.

SBE's native-type/word-aligned design principles are more applicable to this boundary than network-style bit packing.

---

# 87. Do not encode string length twice

With offset table:

```text
length = offsets[r] - offsets[r - 1]
```

Do not also write a length into every string-bearing record.

Exceptions require a demonstrated hot-path reason such as retained string view comparison, not convenience.

---

# 88. Do not store byte offsets in semantic PackedMeta before transaction compilation

A semantic string's byte offset depends on:

```text
which other strings are emitted in this transaction
which dedupe policy is selected
transaction encounter order
```

Construction-time metadata should store:

```text
string value / atom identity / class
```

not a stale byte offset.

`StringRef` and offsets are transaction-local compilation artifacts.

---

# 89. PERF-8 PATCH records can reference new strings directly

Example text metadata patch:

```text
PATCH_TEXT_META
    target new PackedRef
    base old PackedRef
    flags = ALIGN_CHANGED | WRAP_CHANGED
    ...
```

has no string bytes.

Example replacement of one span string, if PERF-8 supports text-local persistent spans:

```text
PATCH_TEXT_SPAN
    target new PackedRef
    base old PackedRef
    span_index
    new_string_ref
```

emits only the new span text.

The string arena must follow PATCH granularity rather than force a full text-node redefinition.

---

# 90. Decoration PATCH must not resend unchanged string metadata

Suppose:

```text
old View:
    theme = "surface"
    styleState phase = "running"
    padding = 1

new View:
    same theme
    same styleState
    padding = 2
```

PERF-9 expected:

```text
PATCH common decoration scalar
string_count = 0
used_bytes = 0
```

If the encoder emits `"surface"` or `"phase"` again, the recipe/lineage design is incomplete.

---

# 91. Rebuilt-equivalent mode intentionally retransmits strings unless deduped locally

`REBUILT_EQUIVALENT` creates entirely new semantic identities.

PERF-8 therefore cannot retain them by identity.

PERF-9 may still dedupe equal string contents inside one transaction through D0/D1 policies.

Do not add cross-transaction semantic identity merely to improve this benchmark.

That would change the experiment from transport optimization into value interning.

---

# 92. COLD mode must warm the byte writer capacity separately

PERF-8 COLD means semantic/native identity cache cold while process/JIT infrastructure is warm.

PERF-9 should extend that definition:

```text
COLD:
    process/JIT warm
    word scratch warm
    byte scratch warm
    UTF-8 writer code warm
    semantic/native identity cold
    fresh semantic NodeIds
```

FIRST_USE separately includes:

```text
byte buffer first allocation/growth
encoder first-use setup
first native slab allocation
```

Do not conflate first buffer growth with cold transport cost.

---

# 93. Add a dedicated UTF-8 writer microbenchmark suite

Before full View benchmarks, compare JS writer implementations with no N-API call.

Datasets:

```text
10,000 unique 4-16 byte ASCII strings
10,000 repeated short ASCII strings
10,000 empty strings
10,000 Latin-1-heavy strings
10,000 2/3-byte Unicode strings
10,000 emoji/non-BMP strings
1,000 ~256-byte ASCII strings
1,000 ~256-byte mixed Unicode strings
100 ~4 KiB strings
10 ~1 MiB strings
realistic styled-span strings
realistic Diff lines
```

Measure:

```text
elapsed ns
CPU user/system
bytes written
byteLength calls
buffer grows
GC/heap delta
allocations if profiler available
```

---

# 94. UTF-8 writer candidate matrix

At minimum benchmark:

```text
E1
    TextEncoder.encodeInto
    reusable Uint8Array

E2
    Buffer.write
    unconditional Buffer.byteLength

E3
    Buffer.write
    3*UTF16-length capacity fast path
    byteLength only on insufficient-capacity slow path

E4
    TextEncoder.encodeInto
    exact byteLength/growth strategy
```

Optional:

```text
E5
    Bun-specific internal/API primitive only if officially supported
```

Do not use undocumented engine internals in production.

---

# 95. Alternate writer benchmark order

Do not run:

```text
all E1
then all E2
then all E3
```

Alternate deterministically within equivalent datasets:

```text
E1 E2 E3 E4
E4 E3 E2 E1
...
```

Keep independently warmed writer objects.

This reduces thermal/JIT/background drift.

---

# 96. String dedupe microbenchmark suite

For each writer candidate, benchmark D0/D1/D2 policies on:

```text
all unique strings
all same string
10 unique repeated 1,000 times
90% unique / 10% repeated
10% unique / 90% repeated
large unique text + repeated small metadata
```

Report:

```text
Map lookups
Map inserts
identity hits
content hits
bytes avoided
JS CPU
heap
```

A policy that saves 90% of bytes but doubles JS CPU may still lose on a local in-process boundary.

---

# 97. Native storage microbenchmark suite

With a prebuilt valid `(offsets, bytes)` packet, benchmark:

```text
R0 per-string owned
R1 transaction-wide slab
R2 paged slab
```

Datasets mirror writer distributions.

Measure:

```text
validation ns
retention-build ns
alloc count if available
bytes copied
pages allocated
peak RSS
post-drop RSS/heap where meaningful
clone cost of retained string handles
string equality/hash cost
```

Separate storage cost from transport boundary cost first.

---

# 98. Page-size sweep is mandatory before freezing R2

Candidate page targets:

```text
16 KiB
32 KiB
64 KiB
256 KiB
1 MiB control
```

For each measure:

```text
allocation count
copy throughput
retention amplification
cache behavior
RSS after churn
```

Do not choose 64 KiB merely because it is conventional.

64 KiB is only the initial implementation candidate.

---

# 99. Add a slab-churn lifetime benchmark

Workload:

```text
for iteration 0..10_000:
    create transaction with many strings
    retain only a small subset in current View
    replace old root
    allow old Views to die

periodically:
    GC-capable JS point
    native cache prune point
    collect memory counters
```

Expected:

```text
native live slab bytes track live semantic text
within bounded page amplification
```

Reject:

```text
native slab memory tracks lifetime total transmitted bytes
```

---

# 100. Add a worst-case page-retention benchmark

Deliberately construct:

```text
one tiny live string per page
all other strings dead
```

This measures the upper retention-amplification behavior of the page policy.

Report:

```text
page size
live payload bytes
retained bytes
amplification
```

Do not hide worst-case fragmentation behind average RSS.

---

# 101. Add byte-lane structural retention assertions

Counters must prove:

```text
IDENTICAL:
    strings_requested = 0
    bytes_written = 0

SHARED_PATH numeric-only:
    strings_requested = 0
    bytes_written = 0

SHARED_PATH one changed 7-byte ASCII leaf:
    bytes_written ≈ 7 plus only changed metadata if any

WIDE_PARENT_ONE_EDIT 20 children:
WIDE_PARENT_ONE_EDIT 100,000 children:
    same changed text byte count
```

String work must be independent of stable subtree/sequence size.

---

# 102. Add string-heavy retained PATCH workloads

The full View matrix needs explicit string stress cases beyond generic workload names.

Add:

```text
TEXT_ONE_SPAN_REPLACE
TEXT_STYLE_ONLY
TEXT_WRAP_ONLY
TEXT_APPEND_SPAN
DIFF_ONE_LINE_REPLACE
STYLE_STATE_VALUE_CHANGE
THEME_KEY_CHANGE
BORDER_LABEL_CHANGE if bridged
BORDER_GLYPH_CHANGE
MANY_REPEATED_METADATA_KEYS
```

Each has a precise expected byte count/order-of-growth.

---

# 103. Full PERF-8 matrix remains authoritative

PERF-9 must rerun all retained/full-schema cases from PERF-8, including:

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

COLD
FIRST_USE
IDENTICAL_IDENTITY
SHARED_PATH
SHARED_WIDE
SHARED_DEEP
REBUILT_EQUIVALENT
WIDE_PARENT_ONE_EDIT
wide insert/remove/append cases
realistic trace
```

PERF-9 is invalid if it reports only string microbenchmarks.

---

# 104. Primary performance metric remains total semantic cost

Do not decide from:

```text
UTF-8 encoder ns
```

alone.

Primary:

```text
construction required by mode
+
PackedMeta work required by semantic construction
+
transaction compile
+
UTF-8 encode
+
N-API crossing
+
native validation
+
retained string storage
+
View/PersistentSeq reconstruction/patch
+
host retained commit
+
return
```

Call it:

```text
commit_ns
```

Keep component timings diagnostic.

---

# 105. Component timings for PERF-9

Record at least:

```text
construction_ns
structural_compile_ns
utf8_encode_ns
napi_entry_ns where measurable
utf8_validate_ns
native_string_retain_ns
native_view_build_ns
total_commit_ns
forced_frame_ns
```

Avoid overlapping timers that make sums misleading.

If fine-grained native timers materially perturb latency, collect them in a separate profiling run and use counters in the authoritative low-overhead run.

---

# 106. PERF-9 JS counters

Add:

```text
packed_utf8_strings_requested
packed_utf8_empty_refs
packed_utf8_identity_dedupe_hits
packed_utf8_content_dedupe_lookups
packed_utf8_content_dedupe_hits
packed_utf8_unique_strings
packed_utf8_bytes_written
packed_utf8_offset_words
packed_utf8_writer_calls
packed_utf8_byte_length_calls
packed_utf8_buffer_grows
packed_utf8_buffer_capacity_high_water
packed_utf8_unused_capacity_high_water
packed_utf8_textencoder_calls
packed_utf8_buffer_write_calls
packed_utf8_partial_write_retries
packed_utf8_bytes_avoided_by_dedupe
```

Use candidate-specific counters only where meaningful.

---

# 107. PERF-9 native counters

Add:

```text
napi_packed_utf8_bytes_borrowed
napi_packed_utf8_validation_bytes
napi_packed_utf8_validation_failures
napi_packed_utf8_string_refs_read
napi_packed_utf8_unique_strings
napi_packed_utf8_pages_planned
napi_packed_utf8_pages_allocated
napi_packed_utf8_dedicated_slabs
napi_packed_utf8_bytes_copied_to_retained
napi_packed_utf8_retained_handle_clones
napi_packed_utf8_owned_string_allocations
napi_packed_utf8_live_slab_bytes
napi_packed_utf8_live_payload_bytes
napi_packed_utf8_peak_slab_bytes
```

Do not reuse the old ambiguous `NapiPackedStringBytesCopied` counter as the only evidence.

---

# 108. Existing string-copy counter must be clarified or retired

Current packed code increments the old string-byte counter when a string-table entry is accessed.

That counter is not a literal hardware memcpy counter.

PERF-9 should either:

```text
rename it to reflect reference/read demand
```

or:

```text
retire it for V4 and use explicit counters
```

Do not claim exact byte copies from a counter that increments before `.to_owned()` or other consumer behavior.

---

# 109. Measure UTF-8 validation implementation separately

Baseline:

```rust
std::str::from_utf8
```

For large arenas only, benchmark a SIMD validator candidate such as simdutf if integration is technically reasonable.

simdutf provides optimized UTF-8 validation across x64 and ARM/NEON architectures and is used by major runtimes, including Bun according to its project documentation.

But do not add a C++ dependency to shave nanoseconds from small packets.

Decision:

```text
if validation < material threshold in realistic traces:
    keep Rust std

if large string workloads show validation dominant:
    benchmark optimized path
```

Reference:

- simdutf: https://github.com/simdutf/simdutf

---

# 110. Do not skip validation merely because JS generated the bytes

The packed native method is still an N-API boundary.

Malformed typed arrays can be supplied directly by tests or userland/native misuse.

Rust semantic safety requires:

```text
only valid UTF-8 enters RetainedStr::Shared
```

Therefore a production unsafe fast path that trusts JS bytes without validation is rejected.

Optimization must make validation cheap, not remove the trust boundary.

---

# 111. Validation can be omitted only for specialized exact-root/no-byte calls

Exact root has:

```text
no byte lane
```

so naturally:

```text
UTF-8 validation work = 0
```

Do not call an empty-lane validator on exact-root path.

For ordinary V4 packet with `used_bytes=0`, decoder may skip `from_utf8` after validating header relations.

---

# 112. Malformed V4 transaction matrix

Reject and leave host unchanged for:

```text
wrong magic
wrong V4 protocol version
wrong bridge schema version
unknown flags
generation mismatch
used_words > actual word length
used_bytes > actual byte length
records_end before header
records_end after offsets
string offset table outside word range
string_count overflow
offset[0] != 0
offsets not monotonic
offset > used_bytes
final offset != used_bytes
zero-length non-empty StringRef entry
StringRef > string_count
invalid UTF-8
string offset splitting UTF-8 scalar
record length overflow
record crossing into offset table
unknown record opcode
invalid local/persistent WireRef
cold closure containing persistent ref
PATCH kind mismatch
NodeId invalid
PackedRef invalid
PersistentSeq structural invalidity
cycle/forward-ref violation
```

One malformed string must not partially publish preceding Views.

---

# 113. Fuzz the two lanes together

Add deterministic/property fuzzing for:

```text
word header fields
record lengths
StringRefs
offset-table entries
byte contents
UTF-8 boundary positions
flags
record order
```

Important mutations:

```text
truncate words
truncate bytes
increase used_bytes only
shift one offset by +1 into a multibyte scalar
make two offsets descending
set final offset one byte short
replace one UTF-8 continuation byte
reference StringRef N+1
move offsets start into record payload
```

Assert:

```text
no panic
no UB
host unchanged on error
no cache publication on failed transaction
```

---

# 114. Direct-vs-packed full semantic differential tests remain mandatory

String transport parity must be checked through actual semantic behavior, including:

```text
rendered cells
text styles
alignment
wrapping
Diff output
border glyphs
style-state/theme resolution
clipping/overflow footer
component-containing trees
```

A byte-perfect UTF-8 arena is not sufficient if native retained storage changes semantic equality/cascade behavior.

---

# 115. Randomized tree generation must include hostile strings

Extend PERF-8 deterministic random trees with string generation classes:

```text
ASCII
Latin-1
mixed BMP
emoji/non-BMP
combining marks
embedded NUL
empty strings where schema allows
very long strings
repeated values
lone surrogates at TS boundary where schema allows raw JS strings
```

For malformed UTF-16 cases, compare the selected canonical policy explicitly.

Keep fixed seeds in CI and log failing tree + seed.

---

# 116. Memory measurement is authoritative, not optional

PERF-9 changes both sides' allocation shape.

Track:

```text
JS:
    heap used
    external/array-buffer memory if exposed
    word scratch capacity
    byte scratch capacity
    transaction-local Map high-water

Native:
    RSS
    live slab page count
    live slab allocated bytes
    live semantic payload bytes
    dedicated large slab bytes
    per-string owned allocation count for control candidate
    ViewBridgeCache entries
    packed slot pages
```

Measure after controlled churn and after old semantic roots have been released.

---

# 117. Scratch high-water memory is acceptable; historical retention is not

Acceptable:

```text
one reusable 8 MiB byte scratch remains
because one live/recent transaction needed 8 MiB
```

Potentially unacceptable:

```text
8 MiB new scratch retained per transaction
```

or:

```text
native slabs retain every historical string
```

Report scratch high-water separately from live retained semantic memory.

---

# 118. Consider shrink-on-extreme-pressure only after measuring

A reusable arena that once handled a 100 MiB pathological View may otherwise keep 100 MiB forever.

Do not add periodic shrinking to the hot path by default.

Possible policy if measurements justify it:

```text
if capacity > MAX_RETAINED_SCRATCH
and recent high-water remains below capacity / 4
for N commits:
    shrink at a cold/maintenance boundary
```

This belongs after baseline performance measurements.

The common path should not branch on GC/memory policy every append.

---

# 119. Arena growth policy

Initial policy:

```ts
nextCapacity = max(
  MIN_BYTE_CAPACITY,
  currentCapacity * 2,
  requiredUsedBytes,
)
```

For very large buffers, a 2x growth policy may create excessive temporary memory.

Benchmark a tiered policy:

```text
< 1 MiB:
    2x

1 MiB .. 16 MiB:
    1.5x or exact-rounded

> 16 MiB:
    round required size to large page quantum
```

Do not optimize this before measuring peak working set.

---

# 120. Growth copies only the used prefix

When JS scratch grows:

```ts
next.set(current.subarray(0, usedBytes));
```

or equivalent Buffer copy.

Do not copy stale capacity after `usedBytes`.

Counter:

```text
packed_utf8_growth_bytes_copied
```

FIRST_USE reports this cost; warmed steady-state should not.

---

# 121. No arena zero-fill requirement for unused capacity

If using `Buffer.allocUnsafe()` for scratch, unused bytes are never exposed semantically because native receives/uses only:

```text
used_bytes
```

Security/correctness invariant:

```text
native MUST ignore bytes >= used_bytes
logs/dumps MUST NOT serialize scratch capacity beyond used_bytes
```

If any API accidentally passes full capacity as meaningful length, `allocUnsafe` turns that bug into data exposure.

Tests must therefore assert the boundary uses the explicit used count.

---

# 122. Do not slice the byte typed array merely to communicate used length if header already carries it

Avoid per-commit:

```ts
bytes.subarray(0, usedBytes)
```

solely for N-API.

Prefer:

```text
pass stable reusable typed-array object/backing view
header.used_bytes tells native active prefix
```

provided NAPI-RS/Bun binding accepts the full capacity and native strictly bounds to `used_bytes`.

This avoids one JS view object per commit.

Benchmark both if binding behavior differs.

---

# 123. Same principle applies to word lane

If PERF-8 currently creates:

```ts
words.subarray(0, usedWords)
```

per transaction, PERF-9 should include a control for passing the full reusable array plus `used_words`.

But this is a secondary constant-factor experiment.

Do not entangle it with UTF-8 correctness.

---

# 124. The two lanes must share one transaction lifetime

Do not asynchronously retain or consume one lane after the other.

One semantic operation:

```text
compile word + byte prefixes
    ↓
one synchronous native call
    ↓
native fully consumes/retains required data
    ↓
return
    ↓
JS may immediately reuse both scratch buffers
```

This permits aggressive scratch reuse without ownership handshakes.

---

# 125. One semantic operation is still one native call

Do not implement:

```text
send UTF-8 arena
→ get native string handles
→ send structural packet containing handles
```

That restores an intermediate-object boundary and two N-API calls.

Correct:

```text
commitPackedV4(words, utf8)
```

Native stages both structure and strings internally.

---

# 126. No per-string N-API calls

Reject:

```ts
for each changed string:
    native.internString(value)
```

Even if native returns compact IDs.

PERF-9's purpose is to batch variable payload into one contiguous boundary crossing.

---

# 127. No JS `Blob`, `ArrayBufferSink`, or stream abstraction in the hot path

Bun provides many binary-writing APIs.

They solve broader streaming/file/network problems.

The View bridge knows:

```text
synchronous commit
bounded in-memory packet
reusable scratch
```

Use the simplest direct byte writer that benchmarks best.

Avoid generic sink abstraction overhead unless profiling proves it is optimized away.

---

# 128. Native packed decoder remains custom

Do not replace the retained protocol with FlatBuffers, Cap'n Proto, Arrow, or SBE wholesale.

Research lessons are useful:

```text
Arrow:
    offsets + contiguous variable data
    view/slab ideas

SBE:
    native scalars
    streaming order
    avoid intermediate copies/allocations

FlatBuffers:
    contiguous byte-vector strings
    offset references

Cap'n Proto:
    structured pointer-based wire design
```

But Iyon's defining semantics are domain-specific:

```text
weak retained identity
PackedRef generations
semantic NodeId
immutable PATCH lineage
PersistentSeq path sharing
host mutation atomicity
```

A custom protocol is justified.

---

# 129. FlatBuffers string termination is intentionally not copied

FlatBuffers stores strings as byte vectors with a trailing NUL for accessor convenience.

Iyon's native retained representation exposes Rust `&str`, which is already length-aware.

Therefore V4 uses:

```text
no terminator
```

This is both smaller and more natural for Rust.

Reference:

- FlatBuffers Internals: https://flatbuffers.dev/internals/

---

# 130. String table order must be deterministic for reproducibility

Given identical semantic transaction, candidate configuration, and traversal order:

```text
StringRef assignment
byte arena
word packet
```

should be deterministic.

Do not use unordered native/JS hash-map iteration to emit table entries.

The content-dedupe Map is lookup-only; first semantic encounter determines the canonical StringRef.

This makes packet snapshots, fuzz reduction, and benchmark diagnosis easier.

---

# 131. Add packet snapshot tests

For small fixtures assert exact V4 words and bytes.

Examples:

```text
single Text("abc")
two Text nodes sharing equal content
empty string
styled Text with repeated theme key
Diff line
Unicode text
embedded NUL
```

Snapshot:

```text
header
record words
StringRefs
offset table
hex byte arena
```

These tests lock protocol grammar independently of renderer behavior.

---

# 132. Packet snapshots do not replace semantic differential tests

Exact bytes only prove encoder stability.

A stable bug is still a bug.

Keep both:

```text
wire snapshots
+
direct-vs-packed semantic/render parity
```

---

# 133. Version V4 explicitly

Do not reuse V3 protocol version with an implicit interpretation of the second argument.

Packed V4 changes:

```text
string field meaning
header sections
native function signature
validation rules
retained string path
```

Increment packed protocol version.

Bridge semantic schema version only changes if semantic View grammar changes independently.

Transport version and semantic schema version remain distinct.

---

# 134. Shared canonical constants remain generated

Extend the canonical schema/config mechanism for:

```text
PACKED_PROTOCOL_VERSION_V4
V4 header indexes if generated constants are useful
V4 flags
string-ref reserved values
section tags if any
```

Do not hand-copy unrelated magic integers into TS and Rust.

If header offsets are compile-time constants local to each implementation, tests must still cross-check versioned wire snapshots.

---

# 135. Feature gating

Keep PERF-9 behind a benchmark/experimental feature until decision.

Suggested feature:

```text
perf-packed-utf8-benchmark
```

If PERF-8 has a generic packed candidate feature, nest or extend it cleanly rather than creating incompatible feature combinations.

Do not make V4 production-facing before the final decision tranche.

---

# 136. Baseline candidates

The authoritative PERF-9 comparison should include:

```text
A — direct structured bridge

B — winning PERF-8 Packed V3 with its selected pre-PERF-9 string lane

C — Packed V4 byte arena + per-string native owned storage (R0)

D — Packed V4 byte arena + paged retained slabs (R2)
```

Optionally retain:

```text
E — V4 transaction-wide slab (R1)
```

as a diagnostic upper bound on allocation/copy reduction.

The production question is primarily:

```text
B vs D
```

A remains an architectural control.

---

# 137. Do not compare against an obsolete V2 packed implementation and call it a PERF-9 win

PERF-9 must preserve the best preceding packed architecture.

If V4 beats PERF-7v2 but loses to PERF-8 V3, it is not a win.

Report exact predecessor SHA and configuration.

---

# 138. Benchmark source reproducibility rules remain strict

Every result record must include:

```json
{
  "benchmark_version": "PERF-9",
  "git_sha": "...",
  "git_dirty": false,
  "git_diff_sha256": null,
  "native_artifact_sha256": "...",
  "benchmark_source_sha256": "...",
  "packed_protocol_version": 4,
  "bridge_schema_version": 1,
  "utf8_writer": "...",
  "string_dedupe_policy": "...",
  "native_string_storage": "...",
  "slab_page_bytes": 65536
}
```

Authoritative runs require clean source unless explicitly marked otherwise.

---

# 139. Raw samples remain mandatory

For every authoritative case retain:

```text
commit samples
forced-frame samples
construction samples
structural compile samples
UTF-8 encode samples
native samples
CPU
memory/counters
```

Do not publish only medians.

---

# 140. Sample counts

Use at least:

```text
warmup >= 20
measured >= 200
```

For authoritative p99:

```text
measured >= 1000
```

Otherwise mark p99 informational.

String writer microbenchmarks can use many more iterations because individual operations are short.

---

# 141. Confidence intervals

Retain bootstrap confidence intervals for:

```text
median
p95
```

and report pairwise relative change.

Do not call a 2% improvement meaningful when confidence/noise is larger than the effect.

---

# 142. Candidate ordering

Within one benchmark case deterministically alternate candidate order.

Example:

```text
A B C D
D C B A
B D A C
C A D B
```

Use independent retained state where required.

No candidate may populate another candidate's semantic/packed caches.

---

# 143. CPU attribution

Report:

```text
JS/Bun user CPU
system CPU
native component timings/counters
```

The byte arena moves work from N-API string conversion into explicit JS UTF-8 transcoding.

A lower native time with higher total CPU is not automatically a win.

---

# 144. Why the byte arena can still win despite JS UTF-8 encoding

The hypothesis is not that UTF-8 conversion disappears.

The hypothesis is:

```text
current path:
    N separate JS string values cross N-API
    runtime converts/materializes N Rust Strings
    decoder may allocate/copy again per semantic use

V4:
    JS performs one explicit contiguous encoding phase
    two typed-array args cross N-API
    native borrows both
    validates byte lane once
    copies each unique transmitted byte once into retained storage
```

The byte arena wins only if batching/ownership savings exceed explicit JS encoder CPU.

That is exactly what PERF-9 measures.

---

# 145. Expected exact identity trace

```text
TS:
    exact same semantic root R

PERF-8 metadata:
    R has live PackedRef in current generation

Call:
    renderPackedRef(generation, ref)

Rust:
    paged slot lookup
    WeakView upgrade
    host commit
```

Expected PERF-9 counters:

```text
strings_requested = 0
unique_strings = 0
bytes_written = 0
utf8_writer_calls = 0
utf8_validation_bytes = 0
slab_allocations = 0
```

If PERF-9 touches the byte writer here, reject the implementation.

---

# 146. Expected narrow shared text update trace

Initial:

```text
R0
├── stable S
└── Text("old")
```

Update:

```text
R1
├── stable S
└── Text("new")
```

PERF-8 structural work:

```text
new/patch Text
new/patch R1
REF S
```

PERF-9 string work:

```text
StringRef 1 = "new"
offsets = [0, 3]
bytes = 6e 65 77
```

No bytes from `S`.

No bytes for `"old"`.

---

# 147. Expected numeric-only update trace

```text
R0 = Text("same").padding(1)
R1 = Text("same").padding(2)
```

With valid PERF-8 lineage:

```text
PATCH padding only
```

PERF-9:

```text
string_count = 0
used_bytes = 0
```

The unchanged text must not be re-encoded merely because root identity changed.

---

# 148. Expected wide one-edit trace

```text
Column C0 with 100,000 children
child 87,213 = Text("old")

Column C1 differs only there:
child 87,213 = Text("new")
```

Target:

```text
TS structural work:
    O(log_32 100000) PersistentSeq chunks
    changed Text
    changed parent ancestry

wire:
    O(log_32 N) structural records
    one changed string

byte lane:
    3 bytes for "new"

Rust:
    O(log_32 N) sequence nodes
    one retained string insertion
```

Width must not affect byte count.

---

# 149. Expected rebuilt-equivalent text trace

If 10,000 newly-created Text nodes contain equal values:

```text
identity cannot retain them
```

Dedupe policies differ intentionally.

D0 content dedupe:

```text
one unique byte entry if all contents equal
10,000 StringRef uses
```

D2 identity-only:

```text
10,000 byte entries
```

The benchmark decides whether content dedupe CPU is worth the retained/wire savings.

---

# 150. Expected weak-cache recovery trace

```text
JS semantic root R alive
PackedRef marked current-generation
native WeakView expired
```

First call:

```text
exact root ref
→ PACKED_CACHE_MISS
```

JS:

```text
PERF-8 generation resync
compile cold closure
collect only closure strings
encode UTF-8 bytes
```

Native:

```text
validate V4
retain bytes
reconstruct closure
publish weak refs
host commit exactly once
```

No first-attempt slab survives.

---

# 151. Production decision gate

Packed V4 enters production only if all correctness gates pass and the measured result satisfies the following.

## Mandatory no-regression gates

```text
IDENTICAL:
    still uses specialized ref call
    zero byte work
    no material regression vs PERF-8

SHARED numeric/no-string:
    zero byte work
    no material regression vs PERF-8

WIDE_PARENT_ONE_EDIT:
    PERF-8 sublinear structural scaling preserved

memory:
    no lifetime accumulation
    bounded slab retention amplification

native-only Rust View construction:
    no material regression
```

## Performance gate

Use total commit time over the realistic trace as primary.

Suggested interpretation:

```text
< 3% trace improvement:
    reject V4 production complexity

3% to < 8%:
    adopt only if memory/allocation behavior also materially improves
    and implementation complexity remains contained

>= 8% trace improvement:
    strong production candidate
```

Additional rule:

```text
if realistic trace is neutral but string-heavy COLD/REBUILT/full-schema
cases improve >= 15% with no retained-path regression,
V4 may still be adopted if those workloads are operationally important
and memory behavior improves.
```

Do not decide from UTF-8 microbenchmarks alone.

---

# 152. Allocation/memory can break a close latency tie

If B and V4 differ by only a few percent in latency, prefer V4 only if it also proves a meaningful reduction in:

```text
N-API string conversions
native string allocations
bytes copied
heap churn
RSS under long-session workloads
tail variance
```

Conversely, reject a small median win that causes retained slab amplification or p95/p99 instability.

---

# 153. Production migration only after decision

Keep V4 experimental through the authoritative run.

If it loses:

```text
remove V4 candidate implementation
retain result document + focused regression/oracle tests where useful
```

If it wins:

```text
separate production commit
migrate every Packed V3 View-bearing operation to V4 string lane
remove old string[] lane after differential soak
```

Do not carry two string transports permanently.

---

# 154. Every View-bearing operation inherits V4

If PERF-8 productionized all View boundaries, PERF-9 must preserve that inventory.

At minimum re-check:

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

plus every new View-bearing boundary present at implementation time.

Forest transactions use one shared offset table and byte arena across roots.

---

# 155. Forest transaction string dedupe is global within that transaction

If animation frames share newly-created equal metadata strings:

```text
one transaction-local StringRef table
```

covers all roots.

Do not create one byte arena/string table per frame.

PERF-8 intra-transaction graph sharing and PERF-9 string dedupe must compose.

---

# 156. Strings in PersistentSeq internal nodes do not exist

PersistentSeq structural nodes should contain only child/item refs and aggregate metadata.

Do not attach string payload to sequence chunks unless the semantic sequence item itself owns it.

This keeps:

```text
sequence PATCH
```

orthogonal to the UTF-8 lane.

---

# 157. Text span persistence is a possible PERF-9 amplifier, not a prerequisite

If PERF-8.4 introduces persistent span sequences, PERF-9 benefits directly:

```text
one span text changed
→ only that string enters arena
```

If Text still redefines a flat span collection, V4 may need to encode every string in a changed Text definition.

Do not hide this distinction.

Report text structural granularity in PERF-9 results.

A span-persistence redesign belongs to PERF-8/structural semantics, not the byte-lane core.

---

# 158. Diff persistence likewise stays a structural concern

Large Diff payloads may dominate string bytes.

If PERF-8 retains hunk/line structure:

```text
one changed Diff line
→ one changed line string
```

If not:

```text
changed Diff node
→ all lines may be new payload
```

PERF-9 optimizes how those selected strings cross; it does not fake structural retention with string content hashing.

---

# 159. Optional content hashing is not semantic identity

Do not treat equal UTF-8 bytes as proof that two semantic nodes are the same View.

String content dedupe means only:

```text
these textual payload bytes are equal
```

It does not merge:

```text
NodeId
View identity
style identity
component identity
sequence identity
```

Keep dedupe scope narrow.

---

# 160. Native slab dedupe across equal StringRefs is automatic; cross-transaction dedupe is not

Within one transaction:

```text
one StringRef
→ one retained source entry
```

Across transactions, a new equal string normally gets new native retained storage if its semantic node is genuinely new.

That is intentional in baseline V4.

Do not sneak in a global Rust string interner without measuring lifetime and lookup cost.

---

# 161. If a native string interner is later tested, it must be weak/bounded

Requirements:

```text
bounded bytes/entries or weak ownership
content hash + exact byte equality
no unbounded historical retention
memory-pressure behavior
clear generation/lifetime semantics
threading/lock cost measured
```

The default V4 has no persistent string interner.

---

# 162. Consider small-string inline native storage only as a later storage candidate

Arrow BinaryView demonstrates the value of inlining very small strings.

A future Rust candidate could use:

```rust
enum RetainedStr {
    Inline { len: u8, bytes: [u8; N] },
    Owned(Box<str>),
    Shared(SlabSlice),
    Static(&'static str),
}
```

But this can inflate every `RetainedStr` handle and therefore every TextSpan.

Do not implement it before `size_of` + workload data shows short strings dominate enough to justify the wider enum.

PERF-9 baseline uses slab/owned/static.

---

# 163. Native UTF-8 slab can preserve strings without per-string validation

After whole-lane validation and boundary checks, each source range is known valid.

When copying into native pages:

```text
concatenating individually valid complete ranges
```

produces valid UTF-8 page content.

Record destination boundaries at each copy.

This establishes the slab invariant without running `from_utf8` on every page again.

Test the invariant with debug assertions in development if useful.

---

# 164. Never concatenate ranges without preserving boundaries

If page planner splits a string because a page is nearly full, then one `SlabSlice` would span pages.

Baseline R2 forbids that.

Rule:

```text
one retained string belongs entirely to one slab page
```

If it does not fit:

```text
new page
or dedicated large slab
```

This keeps `SlabSlice` two scalars + one Arc and makes `as_str()` contiguous.

---

# 165. Page waste is measured explicitly

For each page:

```text
allocated capacity
used bytes
live referenced bytes
```

Aggregate:

```text
packing waste = allocated - initially used
lifetime waste = retained allocated - currently live payload
```

These are different problems.

A page size can pack efficiently initially and still retain poorly after churn.

---

# 166. Do not use `Arc<Vec<u8>>` with mutable API surface casually

Preferred internal slab:

```rust
struct Utf8Slab {
    bytes: Box<[u8]> // if conversion/copy behavior is acceptable
}
```

or:

```rust
struct Utf8Slab {
    bytes: Vec<u8>
}
```

with no mutable methods exposed after construction.

Choosing `Box<[u8]>` may shrink capacity but can involve allocation/copy depending on capacity/layout.

Benchmark exact conversion behavior.

The semantic invariant is immutability; representation is measured.

---

# 167. Exact-capacity page allocation can avoid post-freeze shrink copies

Because the planner knows page used sizes before copying, allocate each page at its planned final length/capacity.

Example:

```rust
let mut page = Vec::with_capacity(planned_len);
...
debug_assert_eq!(page.len(), planned_len);
```

Then keep the Vec in `Utf8Slab` without `shrink_to_fit()`.

This avoids a freeze-time allocator/copy operation.

---

# 168. Plan table can be compact and temporary

Native planner needs roughly:

```rust
struct PlannedString {
    page_index: u32,
    offset: u32,
    len: u32,
}
```

for each unique StringRef.

This table is transaction-local.

After semantic build:

```text
planned locations drop
native StringRef table drops
only RetainedStr handles in semantic objects survive
```

Measure its allocation cost against simpler one-pass R0.

---

# 169. Optional no-copy transaction-wide slab is a diagnostic ceiling

R1 can show the theoretical benefit of minimizing native copy/allocation:

```text
copy full used byte lane once
all ranges reference it
```

If R1 is not materially faster than paged R2:

```text
choose R2 for safer lifetime behavior
```

If R1 is dramatically faster, investigate why page planning/copying is costly before accepting unbounded retention amplification.

---

# 170. External zero-copy retained arena requires an ownership-transfer protocol

The only way to retain JS/native shared bytes safely without copying is to prove:

```text
after commit succeeds:
    JS can no longer mutate the backing memory
    JS cannot reuse it as scratch
    Rust owns/lifetimes it until last retained string dies
```

That implies something like:

```text
native-owned page
exposed temporarily to JS
JS writes
page sealed
JS mutable alias detached/revoked
Rust retains page
```

Do not attempt this with ordinary reusable `Uint8Array` aliases.

One accidental later write would violate Rust `&str` immutability and can become undefined behavior.

---

# 171. External sealed pages are PERF-9.x only

Only investigate if all are true:

```text
V4 wins enough to continue
native retained-copy cost is dominant
Bun supports required external buffer behavior robustly
alias revocation/detachment is testable
lifetime finalization is correct
memory savings justify complexity
```

The safe copied-slab design is the production baseline.

---

# 172. Do not depend on Bun FFI for V4

Bun FFI may offer lower raw call overhead, but PERF-9 primarily targets variable data marshalling/retention.

Keep stable N-API as production baseline.

If PERF-8 already has an FFI boundary experiment, re-run it only after V4 is stable.

Do not combine:

```text
new UTF-8 wire
new native string storage
new FFI boundary
```

in one decision.

---

# 173. Required implementation sequence

Do not hand this whole document to one implementation agent as one commit.

Use the following tranches.

## PERF-9.0 — freeze the winning PERF-8 baseline

```text
record exact PERF-8 production/candidate SHA
rerun key PERF-8 structural counters
verify exact-root fast path
verify WIDE_PARENT_ONE_EDIT scaling
verify weak-cache recovery
verify full-schema parity
capture baseline string metrics
no V4 code yet
```

Gate:

```text
PERF-8 invariants are reproducible
```

## PERF-9.1 — Unicode/string semantic oracle

```text
add lone-surrogate/NUL/non-BMP fixtures
compare direct N-API conversion
compare TextEncoder
compare Buffer.write
lock canonical behavior
no performance decision yet
```

Gate:

```text
selected byte writers are semantically equivalent to required bridge behavior
```

## PERF-9.2 — V4 dual-lane wire, R0 storage

```text
protocol version 4
used_bytes header
StringRef
word-lane offset table
borrow &[u8]
whole-lane UTF-8 validation
offset boundary validation
per-string owned native storage control
full malformed-packet tests
```

Do not change retained Rust string types broadly yet if R0 can be integrated narrowly for the candidate.

## PERF-9.3 — JS UTF-8 writer + dedupe shootout

```text
E1/E2/E3/E4 writer candidates
D0/D1/D2/D3 dedupe candidates
microbenchmark distributions
choose one writer and one dedupe policy
```

Freeze the measured winner before native storage work.

## PERF-9.4 — retained string abstraction + paged slabs

```text
RetainedStr
SlabSlice
Utf8Slab
page planner
large-string dedicated slabs
migrate relevant private Rust semantic fields
native-only regression benchmark
lifetime/churn tests
```

Compare R0/R1/R2.

## PERF-9.5 — validation/page tuning

```text
page-size sweep
large-string threshold sweep
std::str::from_utf8 baseline
optional simdutf benchmark if validation dominates
scratch growth policy
```

Do not add dependencies without measured benefit.

## PERF-9.6 — full authoritative benchmark

```text
all PERF-8 workloads/modes
string-specific retained workloads
realistic trace
raw samples
confidence intervals
memory/lifetime
p95/p99
CPU
write final decision
```

## PERF-9.7 — productionize only if V4 wins

```text
make selected V4 lane normal packed transport
migrate every View-bearing boundary
remove old string[] packed lane
retain differential direct bridge temporarily
remove PERF-9 candidate flags
```

## PERF-9.x — optional only after production decision

```text
persistent metadata string dictionary
small-string inline native representation
sealed native-owned external pages
Bun FFI call boundary
```

Each requires its own evidence.

---

# 174. Suggested commits

```text
test(tui): lock packed UTF-8 string semantics
feat(runtime): add dual-lane packed UTF-8 transaction
perf(runtime): batch packed strings into reusable byte arena
perf(native): retain packed strings in immutable UTF-8 slabs
test(tui): prove packed UTF-8 lifetime and malformed-input safety
bench(tui): tune packed UTF-8 writer and slab geometry
bench(tui): complete PERF-9 UTF-8 arena decision
```

Only if it wins:

```text
perf(tui): adopt dual-lane retained packed transport
```

---

# 175. Mandatory acceptance checklist

An implementation agent is not allowed to mark PERF-9 complete until all of these pass:

```text
[ ] PERF-8 winning baseline SHA is frozen and clean
[ ] exact-root path still bypasses transaction arrays
[ ] forest known-root path still emits no byte lane
[ ] PackedRef generation/ABA invariants unchanged
[ ] semantic NodeId remains full safe integer
[ ] same environment ViewBridgeCache remains authoritative
[ ] weak native View retention unchanged
[ ] one cache recovery then hard failure unchanged
[ ] cold closure contains no persistent refs
[ ] PersistentSeq TS/Rust path remains O(log N)
[ ] WIDE_PARENT_ONE_EDIT structural counters remain sublinear
[ ] immutable PATCH semantics unchanged
[ ] one semantic mutation remains one native call

[ ] packed protocol version incremented to V4
[ ] word lane contains string offset table
[ ] byte lane contains UTF-8 active prefix only
[ ] StringRef is one u32
[ ] StringRef 0 is canonical empty only
[ ] absent vs empty semantics remain distinct
[ ] no NUL terminators added
[ ] embedded NUL round-trips
[ ] used_words and used_bytes validated
[ ] offset[0] == 0
[ ] offsets monotonic
[ ] final offset == used_bytes
[ ] every offset is UTF-8 boundary
[ ] invalid UTF-8 rejected
[ ] StringRef out of range rejected
[ ] malformed string data leaves host unchanged

[ ] direct/TextEncoder/Buffer Unicode oracle completed
[ ] lone high surrogate tested
[ ] lone low surrogate tested
[ ] valid surrogate pair tested
[ ] non-BMP tested
[ ] U+10FFFF tested
[ ] combining marks tested
[ ] composed/decomposed distinction preserved
[ ] writer selected only after parity

[ ] hot N-API byte argument is borrowed, not Vec<u8> conversion
[ ] hot N-API word argument remains borrowed
[ ] JS backing memory is never retained after call
[ ] native copies/internalizes every retained byte before return

[ ] TextEncoder.encode() not used in steady path
[ ] UTF-8 writer uses reusable scratch
[ ] byte buffer growth is geometric/tuned
[ ] steady warmed byte buffer grows = 0
[ ] no per-string typed-array allocation in winning writer, or measured cost accepted
[ ] Buffer.write/TextEncoder candidates benchmarked
[ ] byteLength pre-pass strategy benchmarked
[ ] no hand-written JS UTF-8 codec

[ ] string identity dedupe tested
[ ] content dedupe policies tested
[ ] current V2/V3 content-dedupe benefit not silently lost
[ ] no unbounded global JS string interner
[ ] stable semantic subtree strings are never visited/re-encoded
[ ] numeric-only PATCH writes zero string bytes
[ ] wide one-edit byte count independent of width

[ ] R0 per-string native owned control benchmarked
[ ] R1 transaction slab control benchmarked
[ ] R2 paged slab benchmarked
[ ] native retained string abstraction preserves exact equality/hash semantics
[ ] native Rust-only construction benchmarked
[ ] String -> Arc<str> copy not introduced blindly
[ ] shared string clone does not copy bytes
[ ] one StringRef copies source bytes at most once into retained storage
[ ] large strings can use dedicated slabs
[ ] one retained string never spans slab pages
[ ] slab pages immutable after publication
[ ] no strong slab cache independent of semantic View ownership
[ ] transaction string table drops after build
[ ] page-size sweep completed
[ ] retention amplification measured
[ ] churn test shows no lifetime accumulation
[ ] worst-case one-live-string-per-page measured

[ ] whole-lane UTF-8 validation measured
[ ] std::str::from_utf8 remains baseline unless optimized validator wins materially
[ ] unsafe from_utf8_unchecked, if used, is isolated behind documented invariant
[ ] invalid bytes never enter RetainedStr

[ ] all PERF-8 full-schema differential fixtures pass
[ ] randomized differential trees pass
[ ] hostile string randomized fixtures pass
[ ] malformed two-lane fuzzing passes
[ ] wire snapshot tests pass

[ ] warmup >= 20
[ ] authoritative measured >= 200
[ ] p99 only authoritative at >= 1000
[ ] raw samples retained
[ ] bootstrap median/p95 intervals reported
[ ] candidates alternated
[ ] total construction+commit is primary metric
[ ] forced-frame remains separately reported
[ ] JS CPU and native CPU reported
[ ] JS heap/RSS/native slab memory reported
[ ] realistic trace run
[ ] final 3%/8%/15% decision rule explicitly applied
```

---

# 176. Banned shortcuts

Reject the tranche immediately if an implementation does any equivalent of the following.

## 176.1 Re-encode all strings every commit

```text
root changes
→ walk entire retained tree's strings
→ rebuild byte arena
```

This destroys PERF-8 retention.

## 176.2 Re-encode stable subtree strings merely to dedupe them

```text
scan stable tree to discover duplicate text
```

Identity cutoff wins over content-dedupe curiosity.

## 176.3 Per-string `TextEncoder.encode()`

```ts
for (const value of strings) {
  chunks.push(encoder.encode(value));
}
```

Rejected steady-state allocation pattern.

## 176.4 `string[]` plus UTF-8 arena

```text
keep old N-API string[]
and also build bytes
```

unless used as a temporary benchmark oracle, never candidate hot path.

## 176.5 `Vec<u8>` hot binding that copies typed-array input

If generated binding semantics copy before decoder, fix the signature.

## 176.6 Per-string native N-API calls

```text
internString()
internString()
internString()
commit()
```

Rejected.

## 176.7 Per-use native String allocation

```text
same StringRef appears 20 times
→ allocate/copy it 20 times
```

Rejected.

## 176.8 Global strong string cache

```rust
HashMap<String, Arc<str>> // lifetime = environment forever
```

without bounded/weak policy.

Rejected.

## 176.9 One giant retained transaction slab as production without lifetime proof

R1 is a control, not automatic winner.

## 176.10 Retaining JS typed-array pointer as Rust `&str`

Memory-safety violation unless a separately proven ownership-transfer mechanism exists.

## 176.11 Skip UTF-8 validation because JS wrote the packet

Rejected trust-boundary weakening.

## 176.12 Lossy native UTF-8 decode

Invalid packet must fail, not normalize silently.

## 176.13 Unicode normalization

No NFC/NFD transformation in transport.

## 176.14 Hash-only string semantics

Never use hash equality as authoritative text equality.

## 176.15 Regress exact-root path to empty packet parsing

Exact root remains specialized.

## 176.16 Flatten PersistentSeq because V4 code is easier that way

PERF-8 structural sharing is non-negotiable.

## 176.17 One candidate's cache warms another

Benchmark invalid.

## 176.18 Decide from encoder microbenchmark alone

Total commit + realistic trace governs production.

---

# 177. Required final PERF-9 result document

The implementation agent must report:

```text
Exact candidate SHAs
Clean/dirty status
Native artifact hash
Benchmark source hash
Bun version
Rust version
target/profile

Protocol:
    Packed version
    bridge schema version
    header layout
    StringRef rules
    offset-table layout
    byte writer selected
    dedupe policy selected
    retained string storage selected
    slab page size
    large-string threshold

Correctness:
    direct parity status
    lone-surrogate policy/result
    randomized seeds
    malformed packet/fuzz status
    weak-cache recovery
    NodeId width
    PackedRef generation/ABA
    PersistentSeq oracle

Per workload/mode/candidate:
    construction median/p95
    structural compile median/p95
    UTF-8 encode median/p95
    native median/p95
    total commit median/p95
    forced-frame median/p95
    p99 where authoritative
    confidence intervals
    relative change

JS string counters:
    requested strings
    identity hits
    content lookups/hits
    unique strings
    bytes written
    bytes avoided
    byteLength calls
    writer calls
    buffer growth
    scratch high-water

Native string counters:
    bytes borrowed
    validation bytes/time
    StringRefs resolved
    retained source strings
    bytes copied to retained storage
    pages allocated
    dedicated slabs
    handle clones
    owned allocations

Structural counters:
    Views visited
    full defs
    patches
    sequence leaf/branch defs
    persistent refs
    local refs
    words

Memory:
    JS heap
    RSS
    word scratch high-water
    byte scratch high-water
    native live slab allocated bytes
    native live string payload bytes
    retention amplification
    slab/page peak
    ViewBridgeCache size
    packed slot pages

Realistic trace:
    operation distribution
    total direct time/CPU
    total PERF-8 time/CPU
    total V4 time/CPU
    total bytes transmitted
    total retained copy bytes

Final decision:
    reject / adopt V4
    exact decision threshold satisfied
```

No statement such as:

> "UTF-8 arena was faster."

is acceptable without showing:

```text
what writer won
what dedupe policy won
what native ownership strategy won
that exact/no-string updates remained byte-free
that PERF-8 structural scaling remained intact
that native-only construction did not regress
that retained memory remained bounded
```

---

# 178. Research-derived design lessons

The external research supports several concrete decisions.

## WHATWG Encoding / Web IDL

Lesson:

```text
UTF-8 encoding has defined scalar-value semantics
TextEncoder accepts USVString
malformed UTF-16/lone surrogates therefore require explicit parity tests
```

Applied to PERF-9:

```text
Unicode oracle before performance adoption
no silent semantic drift
```

Sources:

- https://encoding.spec.whatwg.org/
- https://webidl.spec.whatwg.org/

## Bun Buffer / TextEncoder

Lesson:

```text
TextEncoder.encodeInto writes to caller-owned Uint8Array
Buffer.write can write a UTF-8 string at a byte offset
partial Buffer writes avoid partial encoded characters
```

Applied:

```text
benchmark offset-native Buffer writer vs standard TextEncoder writer
```

Sources:

- https://bun.sh/reference/globals/TextEncoder/encodeInto
- https://bun.sh/reference/node/buffer/Buffer

## Bun Node-API implementation

Current Bun source implements `napi_get_value_string_utf8` through its JavaScriptCore string view and Bun UTF conversion helpers.

Lesson:

```text
direct bridge already pays runtime string-to-UTF8 conversion per N-API string extraction
```

Applied:

```text
PERF-9 explicitly batches that conversion before crossing
and verifies exact semantics rather than assuming equivalence
```

Source:

- https://github.com/oven-sh/bun/blob/cfa9f8e15b4252a08c483711d835dfe56a8b21ab/src/jsc/bindings/napi.cpp

## NAPI-RS

Lesson:

```text
borrowed typed-array/slice parameters can be zero-copy
lifetime is bounded to the function call
```

Applied:

```text
borrow word/byte lanes synchronously
copy/internalize retained bytes before return
```

Source:

- https://napi.rs/docs/concepts/typed-array

## Node-API external buffers

Lesson:

```text
external memory has explicit lifetime/finalizer requirements
runtime compatibility is not universal
```

Applied:

```text
external/native-owned scratch is not baseline
```

Source:

- https://nodejs.org/api/n-api.html

## Apache Arrow

Lesson:

```text
variable-width data is efficiently represented as monotonic offsets + contiguous bytes
view formats can inline small strings or reference multiple buffers
```

Applied:

```text
StringRef + offset table + byte arena
paged retained native storage experiment
```

Source:

- https://arrow.apache.org/docs/format/Columnar.html

## SBE

Lesson:

```text
low-latency codecs minimize intermediate copying/allocation
prefer native scalar/streaming access
retained data must be copied out of transient message memory
```

Applied:

```text
u32 structural lane
forward validation/build
borrow boundary
one deliberate retained copy
```

Source:

- https://github.com/aeron-io/simple-binary-encoding/wiki/Design-Principles

## FlatBuffers

Lesson:

```text
strings are encoded as UTF-8 byte vectors and referred to by offsets
```

Applied:

```text
separate textual bytes from structural records
```

Not copied:

```text
NUL termination
backwards builder
```

because Iyon has different access semantics.

Source:

- https://flatbuffers.dev/internals/

## Rust UTF-8

Lesson:

```text
&str requires valid UTF-8
from_utf8 validates
from_utf8_unchecked may skip validation only when invariant is already proven
```

Applied:

```text
validate arena once
check string boundaries
isolate any unchecked retained accessor
```

Source:

- https://doc.rust-lang.org/std/str/fn.from_utf8.html

## Rust Arc<str>

Lesson:

```text
String -> Arc<str> allocates and copies
```

Applied:

```text
do not globally switch ordinary Rust-native Strings to Arc<str>
use a private owned/shared retained representation
```

Source:

- https://doc.rust-lang.org/std/sync/struct.Arc.html

## simdutf

Lesson:

```text
very fast SIMD UTF-8 validation exists on x64 and ARM
```

Applied:

```text
benchmark only if validation is a measured bottleneck
```

Source:

- https://github.com/simdutf/simdutf

---

# 179. Concrete target architecture after PERF-9

If PERF-8 and PERF-9 both win, the private runtime boundary should look like this:

```text
                     TYPESCRIPT / BUN

public View builders
        │
        ▼
canonical immutable semantic View graph
        │
        ├── NodeId
        ├── PackedMeta
        ├── lineage
        ├── PackedStringAtom operands
        └── PersistentSeq
        │
        ▼
changed/unpublished closure compiler
        │
        ├───────────────┐
        │               │
        ▼               ▼
reusable           reusable
Uint32Array        UTF-8 byte arena
        │               │
        │     StringRef ─┘
        │     offsets stored in word lane
        │
        └───────┬───────┘
                │
                │ one synchronous N-API mutation
                ▼
                         RUST

borrow &[u32] + &[u8]
        │
        ▼
validate header/records/UTF-8/offsets
        │
        ├── resolve weak PackedRefs
        ├── build local PersistentSeq path copies
        ├── build immutable View PATCH results
        └── copy unique changed string bytes once
                into immutable retained slabs
        │
        ▼
Arc-backed retained semantic View graph
        │
        ▼
weak semantic cache + weak packed slot acceleration
        │
        ▼
atomic host semantic mutation
```

And the special hot path remains:

```text
same root identity
    ↓
renderPackedRef(generation, ref)
    ↓
no word transaction
no byte transaction
no parser
no UTF-8
```

---

# 180. The algorithmic invariant to remember

PERF-7v2 introduced:

```text
stable View identity
→ do not serialize descendants
```

PERF-8 introduces:

```text
stable sequence identity
→ do not enumerate stable siblings

lineage
→ do not resend unchanged fields inside changed Views

exact PackedRef
→ bypass packet parser entirely
```

PERF-9 must add only:

```text
new textual payload
→ encode once into dense UTF-8 lane

one transaction-local textual value
→ one StringRef

one unique transmitted UTF-8 range
→ at most one native retained byte copy

stable textual payload
→ zero byte work
```

That composition is the goal.

---

# 181. Bottom line

`Uint32Array structure + Uint8Array UTF-8 arena` is worth PERF-9.

The strongest reason is not wire compactness.

It is that the current high-level string boundary makes the runtime repeatedly cross an object-oriented conversion path for data that is ultimately required by Rust as bytes/owned UTF-8 anyway.

The optimal candidate should therefore be:

```text
Packed V4

PERF-8 retained/delta graph semantics
+
one-u32 StringRef fields
+
Arrow-style monotonic offset table in the word lane
+
one reusable Bun UTF-8 byte arena
+
zero-copy synchronous typed-array borrowing at N-API
+
one whole-lane UTF-8 validation
+
transaction-local string dedupe chosen by benchmark
+
paged immutable native UTF-8 slabs
+
private RetainedStr handles
```

Do not settle for:

```text
string[] → Uint8Array
```

as the architecture.

That is only the first half of the optimization.

The complete optimization is:

```text
remove redundant JS/native marshalling
+
remove redundant per-use native string ownership work
+
preserve all retained identity cutoffs
+
keep lifetime and Unicode semantics exact
```

If V4 cannot beat the winning PERF-8 path on total realistic commit cost after including JS UTF-8 work, reject it cleanly.

If it wins while keeping exact/shared updates byte-free and memory bounded, it is the transport I would ship.
