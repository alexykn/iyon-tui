Yes — that split is much cleaner.

This document should do **one thing only**: establish an authoritative Bun 1.4 comparison between the completed 11v3 architecture and the real 7v2 Direct/Candidate A architecture. It should deliberately **not design the hypothetical BridgeViewNode + FFI architecture at all**.

If the result makes that architecture worth pursuing, **PERF-12 starts from zero as a full architecture tranche**: fresh research, generated ABI integration, boundary generation, lifetime model, transport design, cache integration, strings, structural edits, failure atomicity, full schema, tests, benchmark gates, etc. The implementation agent should not be able to “continue” from a sketch in this benchmark document and accidentally create a half-designed transport.

I would call this comparison **PERF-11v4** so PERF-12 remains cleanly reserved for the next architectural proposal.

# PERF-11v4 — Authoritative Bun 1.4 Comparison: PERF-7v2 Direct vs Completed PERF-11v3

**Status:** execute only after PERF-11v3 is fully implemented, tested, and committed  
**Purpose:** re-establish and fairly benchmark the real PERF-7v2 Candidate A / Direct architecture against completed PERF-11v3 under the same Bun 1.4 environment  
**Non-goal:** design or implement any new View transport architecture  
**Future work:** if justified by this result, a separately researched full PERF-12 handoff will investigate whether a 7v2-style JavaScript semantic DAG can be combined with an FFI transport

---

# 0. Scope

This tranche is a benchmark and architectural validation tranche.

It has exactly two primary candidates:

```text
Candidate A:
    PERF-7v2 Direct

Candidate B:
    completed PERF-11v3
```

The goal is to answer:

> On Bun 1.4, using the same current Rust TUI implementation and an equivalent workload, is the original PERF-7v2 Direct architecture or the completed PERF-11v3 architecture faster end-to-end?

Nothing else should be implemented.

In particular, this tranche must **not**:

```text
design a new BridgeViewNode FFI transport
add another wire protocol
add another packed representation
add another generated ABI surface
add new native builder primitives
add new cache structures
add transport-side NodeRefs
add construction-time journals
prototype "7v2 but through FFI"
```

If the benchmark suggests that such an architecture is worth exploring, stop.

The next step will be a separate, full-sized PERF-12 design and implementation handoff with the same level of rigor as PERF-11v3.

---

# 1. Why this comparison is necessary

PERF-11v3 is based on a substantially different architectural bet from PERF-7v2 Direct.

PERF-7v2 Direct used:

```text
normal immutable JS View API
        ↓
eager frozen BridgeViewNode DAG
        ↓
nodeForBridge(view)
        ↓
one N-API Object call
        ↓
Rust reads NodeId
        ↓
environment NodeId → WeakView cache
        ↓
cache hit:
    return retained Rust View immediately

cache miss:
    inspect only this previously unseen BridgeViewNode
    recursively stop at cached descendants
```

The important historical result is that JavaScript construction itself was very cheap.

For the small `IDENTICAL_IDENTITY` workload:

```text
total median:         1,209 ns
construction median:     42 ns
native median:        1,166 ns
```



For the small `SHARED_PATH` workload:

```text
total median:         38,667 ns
construction median:   2,334 ns
native median:        35,708 ns
```



Therefore the historical evidence does **not** support the assumption that constructing the JavaScript semantic DAG was inherently expensive.

Most of the retained-path cost in that benchmark came after JavaScript construction.

PERF-11v3 instead moves much more retained state and mutation knowledge into the generated native ABI path.

It may be substantially faster.

But it has never been authoritatively compared against the real Candidate A architecture under the same Bun 1.4 runtime and current Rust implementation.

PERF-11v4 exists only to obtain that answer.

---

# 2. Terminology

Use these names consistently throughout code and results.

## `direct_7v2`

The faithfully reconstructed PERF-7v2 Candidate A architecture:

```text
historical eager BridgeViewNode construction
+
current direct N-API decoder
+
current Rust TUI
+
current environment-local semantic WeakView cache
+
Bun 1.4
```

## `direct_current`

The current HEAD compatibility route:

```text
current View construction/backing architecture
+
nodeForDirectBridge
+
current direct N-API decoder
```

This is a diagnostic control only.

It is **not** a substitute for `direct_7v2`.

## `native_11v3`

The completed production PERF-11v3 architecture exactly as implemented.

No benchmark-only simplification is allowed.

---

# 3. Known repository state before completion of 11v3

At the repository state inspected while preparing this handoff, branch `perf-refactor` was at:

```text
da0280c1cd5d1df9bf60742e133b4942591611fe
```

The implementation agent must not assume that this remains HEAD.

Repeat all archaeology after PERF-11v3 is complete.

At this inspected revision, the direct native path still exists.

The current benchmark already contains a `direct` candidate which performs approximately:

```ts
bridged = nodeForDirectBridge(view)
host.render(bridged)
```



`NativeTuiHost.render(Object)` still calls:

```rust
self.host.render(decode_view(&view)?)
```

and therefore still enters the direct JavaScript-object decoder. 

The current `ViewDecoder` still performs the fundamental Candidate-A algorithm:

```text
read NodeId
check environment semantic cache
return immediately on live WeakView hit

otherwise:
    remove expired weak entry
    validate schema
    decode the unseen object
    recursively resolve descendants
    publish NodeId → WeakView
```



The current `NativeViewRuntime` explicitly states that the semantic cache belongs to the environment rather than to an individual transport:

```text
direct
packed
FastShared
generated paths
```

all publish through the same semantic map. 

That is the correct architecture for this comparison.

---

# 4. Important complication: current Direct is not historical Candidate A

The native direct decoder survives.

The original JavaScript construction representation does not survive unchanged.

This matters enormously.

## PERF-7v2 JavaScript representation

At the PERF-7v2 benchmark SHA:

```text
e5292d62c4011610850cbdc1ba4a35f296f78e4f
```

a `View` constructor immediately produced the final immutable `BridgeViewNode`:

```ts
private constructor(node) {
    nodes.set(this, withPrivateIdentity(node))
    Object.freeze(this)
}
```

`View.text()` immediately constructed the actual bridge text node.

`ChildrenBuilder` directly accumulated `BridgeLayoutChild` objects pointing at child bridge nodes. 

`nodeForBridge(view)` was effectively a `WeakMap` lookup:

```ts
const node = nodes.get(view)
```

with no lazy materialization step. 

## Current HEAD representation

At the inspected current revision, `View` instead stores a private stable-shape backing with states representing:

```text
materialized semantic node
pending create
pending patch
```

and the source explicitly describes `BridgeViewNode` as something materialized lazily for the cold/direct compatibility path. 

Current `nodeForBridge()` may therefore need to materialize a pending backing before returning it. 

Consequently:

> `direct_current` and `direct_7v2` are different benchmark candidates.

The former answers:

> How expensive is Direct when driven by the current 11-series JS construction model?

The latter answers:

> How expensive was the actual architectural shape we previously rejected?

Only the latter is the authoritative historical comparison.

---

# 5. Tranche order

Execute PERF-11v4 in this order:

```text
11v4.0
    Freeze completed 11v3.

11v4.1
    Archaeology and current-HEAD direct-path verification.

11v4.2
    Verify exactly what `bun run build:iyon` compiles.

11v4.3
    Re-establish a faithful benchmark-only PERF-7v2 JS semantic builder.

11v4.4
    Establish semantic and workload equivalence.

11v4.5
    Build the authoritative Bun 1.4 benchmark harness.

11v4.6
    Execute the benchmark matrix.

11v4.7
    Analyze and publish the result.

STOP.
```

There is no architecture implementation tranche after 11v4.7.

---

# 6. PERF-11v4.0 — freeze completed PERF-11v3

Before touching benchmark archaeology:

```bash
git status --short
git rev-parse HEAD
git log -1 --oneline
```

Require:

```text
working tree clean
all PERF-11v3 tests passing
PERF-11v3 implementation committed
```

Record:

```text
PERF-11v3 final SHA
```

Then verify Bun:

```bash
bun --version
bun --revision
```

The repository currently pins Bun 1.4.0 in `package.json`. 

Use the repository's actual final pinned 1.4.x version.

Do not silently use a globally newer Bun.

Store:

```text
bun version
bun revision
rustc --version
target triple
macOS version
CPU
```

in every result artifact.

---

# 7. PERF-11v4.1 — determine whether Candidate A Direct still exists

Search current HEAD:

```bash
git grep -n \
  -e 'nodeForDirectBridge' \
  -e 'nodeForBridge' \
  -- packages/iyon-runtime/src/tui
```

Then:

```bash
git grep -n \
  -e 'fn decode_view' \
  -e 'struct ViewDecoder' \
  -e 'pub fn render(&self, view: Object)' \
  -- crates/iyon-native
```

Then enumerate every direct `decode_view` call:

```bash
git grep -n 'decode_view' -- crates/iyon-native/src
```

The audit must answer separately:

```text
A. Does the N-API Object decoder still exist?

B. Does NativeTuiHost.render(Object) still enter it?

C. Does it still read NodeId before recursively decoding fields?

D. Does a live NodeId → WeakView hit stop traversal immediately?

E. Is the semantic cache still the same environment NativeViewRuntime used
   by the current retained implementation?

F. Is expired WeakView cleanup still correct?

G. Does the decoder still support the complete current BridgeViewNode schema?
```

Do not infer these from benchmark names.

Trace the source.

---

# 8. Compare historical and current source directly

Use:

```bash
git diff \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f \
  HEAD \
  -- \
  packages/iyon-runtime/src/tui/values/view.ts \
  packages/iyon-runtime/src/tui/ir.ts \
  crates/iyon-native/src/tui.rs
```

Also extract historical files:

```bash
git show \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f:packages/iyon-runtime/src/tui/values/view.ts \
  > /tmp/perf7v2-view.ts
```

```bash
git show \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f:crates/iyon-native/src/tui.rs \
  > /tmp/perf7v2-native-tui.rs
```

The agent must understand the complete historical path before reproducing it.

Do not reconstruct it from the PERF-7v2 handoff prose alone.

---

# 9. Produce an archaeology record

Before implementation, create a concise record containing:

```text
final PERF-11v3 SHA:
historical PERF-7v2 SHA:

direct N-API decoder exists:
NativeTuiHost.render(Object) exists:
same environment semantic cache:
full schema:
compiled in normal build:
historical JS eager DAG unchanged:
current direct benchmark available:

faithful Candidate A can be benchmarked in current checkout:
    yes / no

second worktree required:
    yes / no
```

Expected based on the currently inspected revision:

```text
native Direct:
    yes

normal build includes Direct:
    yes

historical JS eager DAG unchanged:
    no

second complete checkout:
    probably no
```

But verify after 11v3.

---

# 10. PERF-11v4.2 — verify `bun run build:iyon`

The user-facing production build must be tested explicitly.

At the inspected revision:

```json
"build:iyon": "bun run native:stage && bun run packages/iyon-cli/build.ts"
```



and `native:stage` invokes:

```text
cargo build --release -p iyon-native
```

without extra Cargo features unless `ION_NATIVE_FEATURES` is explicitly provided. 

Repeat the proof after 11v3.

Run:

```bash
rm -f packages/iyon-runtime/native/iyon-native.node

env -u ION_NATIVE_FEATURES \
  bun run build:iyon
```

Then:

```bash
file packages/iyon-runtime/native/iyon-native.node
shasum -a 256 packages/iyon-runtime/native/iyon-native.node
```

Do not merely confirm compilation.

Execute a smoke test against the staged artifact.

---

# 11. Prove that the normal build executes Direct

Create a benchmark/test-only smoke script using a valid BridgeViewNode and:

```ts
const Host = native.NativeTuiHost
const host = new Host(80, 24, true)

host.render(bridgeNode)
```

Verify correct rendering.

Then deliberately alter:

```ts
bridgeNode.schema
```

to an invalid version.

The call must fail through the Direct decoder's schema validation.

This establishes:

```text
normal build
    ↓
staged iyon-native.node
    ↓
NativeTuiHost.render(Object)
    ↓
Direct ViewDecoder
```

Save this proof in the results notes.

---

# 12. If Direct is compiled only behind a feature

If final PERF-11v3 changes this unexpectedly, do not immediately switch to an old repository checkout.

First establish why.

If Direct can be restored as a benchmark-only entry without changing its actual lowering algorithm, do so behind an explicit feature such as:

```toml
perf-direct-baseline = []
```

The benchmark feature may expose:

```text
Direct render entry
cache reset
counter probes
```

but must not alter:

```text
ViewDecoder algorithm
semantic cache
Rust View representation
allocator behavior
renderer
```

The objective is to compare transports against the **same current Rust implementation**.

---

# 13. Why a current-checkout benchmark is preferred

The authoritative comparison should use one repository revision because otherwise the benchmark would mix transport differences with:

```text
125+ commits of Rust changes
renderer changes
IR changes
cache changes
layout changes
allocation changes
dependency changes
build changes
```

The comparison from the historical PERF-7v2 SHA to the inspected HEAD is already more than one hundred commits apart.

Therefore the desired shape is:

```text
current HEAD Rust
current HEAD renderer
current HEAD semantic cache
current HEAD build

        ↑

Candidate-specific JavaScript construction + transport
```

This isolates the architecture far better.

---

# 14. Second worktree fallback

Only use a second full checkout if the direct algorithm cannot reasonably be exercised against the current Rust implementation.

If required:

```bash
git worktree add \
  ../iyon-perf7v2-reference \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f
```

Use it primarily for archaeology.

If actual timing must be performed there, label those measurements:

```text
historical cross-revision reference
```

They must not be the primary result.

The implementation agent must explicitly explain why a current-revision Direct baseline could not be produced.

---

# 15. PERF-11v4.3 — faithfully restore the 7v2 JS construction path

This is likely the most important part of the tranche.

Do **not** mutate production `View` back to the historical architecture.

Create a benchmark-only reconstruction.

Suggested directory:

```text
packages/iyon-runtime/bench/perf7v2_direct/
    view.ts
    fixtures.ts
    adapter.ts
```

Use the actual historical source as the starting point:

```bash
git show \
  e5292d62c4011610850cbdc1ba4a35f296f78e4f:packages/iyon-runtime/src/tui/values/view.ts
```

Do not rewrite Candidate A from memory.

---

# 16. Required properties of `Perf7v2View`

The benchmark-only historical View must retain these properties:

```text
immutable View value

one stable semantic NodeId assigned during construction

final BridgeViewNode created immediately during View construction

BridgeViewNode frozen immediately

View → BridgeViewNode stored in WeakMap

unchanged descendants reused by exact JS object identity

modified semantic node gets a fresh NodeId

modified ancestors get fresh NodeIds

unchanged semantic subtrees preserve exact old BridgeViewNode references

nodeForBridge(view):
    WeakMap lookup only
```

For example, the historical implementation's `View` constructor directly stored `withPrivateIdentity(node)` into a `WeakMap`. 

That behavior is the point of the candidate.

---

# 17. What must not leak into `Perf7v2View`

Do not reuse current production implementation details that did not exist in Candidate A:

```text
pending create backing
pending patch backing
native scalar patch state
native retained path lineage
generated ABI routing state
current lazy materialization
PersistentSeq metadata solely for 11v3 routing
FFI call state
native refs
transport generations
```

If such machinery is needed merely to make the benchmark compile, the candidate has been contaminated.

Use historical construction and adapt only the schema/API surface required to interoperate with current Rust.

---

# 18. Allowed compatibility adaptations

Some current schema or type definitions may have changed.

The benchmark-only copy may be mechanically adapted for:

```text
current enum discriminants
current BridgeViewNode TypeScript definitions
current required schema version
current public View feature set
new mandatory node fields
renamed imports
TypeScript compiler changes
Bun 1.4 compatibility
```

For every adaptation, classify it as:

```text
mechanical
semantic
performance-affecting
```

There should be no performance-affecting adaptation without justification.

---

# 19. Do not preserve historical bugs for benchmark purity

The target is:

> the PERF-7v2 Candidate A architecture expressed against the current correct semantic schema.

It is not:

> byte-for-byte resurrection of every old implementation mistake.

Therefore use:

```text
current full safe NodeId domain
current schema
current semantic validation
current cache correctness
current Rust core
```

while preserving the architectural property under test:

```text
eager immutable JS semantic DAG
+
Direct N-API object traversal
```

---

# 20. Candidate A must use the current native semantic cache

Do not restore any historical environment/cache mechanism if later work improved it.

The semantic ownership rule is:

```text
NodeId → WeakView
```

inside the current environment runtime.

At the inspected HEAD the runtime explicitly owns this transport-neutral cache. 

`direct_7v2` must publish through it.

This keeps:

```text
Rust identity
WeakView expiration
cache cleanup
lifetime behavior
```

equivalent between the candidates.

---

# 21. Direct decoder behavior must remain unchanged

Do not optimize Candidate A during this tranche.

No:

```text
faster property access
specialized Text decoder
specialized Row decoder
native refs
u32 handles
FFI
packed side lane
object-shape tricks
```

PERF-11v4 asks:

> How good is 7v2 Direct under Bun 1.4?

not:

> How fast could Direct become if redesigned?

Optimization belongs to later work only if the result warrants it.

---

# 22. Add `direct_current` as a diagnostic control

The current benchmark already has a direct path driven by current production `View` objects. 

Keep this candidate.

It provides a useful decomposition:

```text
direct_7v2
    historical construction
    direct transport

direct_current
    current construction
    direct transport

native_11v3
    current 11v3 construction
    generated native transport
```

This allows measurement of:

```text
effect of changing JavaScript construction architecture

vs

effect of changing the transport/native path
```

But the headline result remains:

```text
direct_7v2 vs native_11v3
```

---

# 23. PERF-11v4.4 — semantic parity before performance

Do not benchmark visually or semantically different trees.

For every benchmark fixture, produce the equivalent:

```text
Perf7v2View
production View
```

and ensure they lower to the same intended UI.

Compare at minimum:

```text
screen rows
styles where relevant
dimensions
wrap behavior
clamping
grid placement
decoration
Diff output
component placement
```

If necessary, create a semantic snapshot representation solely for test comparison.

This snapshot is not timed.

---

# 24. Full schema coverage

Parity fixtures must cover:

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

Also:

```text
all layout child variants
all alignment variants
padding
width/height rules
min/max constraints
foreground/background
text attributes
border
custom border glyphs
style states
overflow variants
Unicode text
```

Do not benchmark a subset and call Candidate A feature-complete.

---

# 25. Randomized differential testing

Before authoritative timing, create a deterministic randomized differential test.

Generate:

```text
tree shape
shared subtrees
styles
Unicode strings
rows
columns
grids
decorations
wrap modes
retained modifications
```

For each seed:

```text
build 7v2 View
build 11v3 View

render through respective candidate

compare output
```

When failure occurs, log:

```text
seed
operation sequence
semantic fixture
screen output A
screen output B
```

This benchmark reconstruction is not trustworthy without differential validation.

---

# 26. Cache identity parity test

Explicitly test:

```text
same root object twice

new root with one changed leaf

new root with one changed deep leaf

new root sharing a large unchanged subtree

rebuilt equivalent tree with entirely new identities
```

Counters must demonstrate the intended Candidate-A semantics.

Example:

```text
IDENTICAL_IDENTITY:
    second render encounters root NodeId
    live WeakView hit
    no descendant decode
```

For shared path:

```text
new root miss
new changed ancestor misses
changed leaf miss
stable subtree boundary hit
no stable descendant traversal
```

---

# 27. Cache expiry test

For Candidate A:

```text
materialize JS node
render
allow/drop native strong ownership as required
force/reset weak-cache state using benchmark helper
render same semantic JS node again
```

Verify:

```text
expired weak does not become false live hit
entry is removed
node is reconstructed correctly
cache is repopulated
```

Use the current runtime's intended lifetime semantics.

---

# 28. PERF-11v4.5 — benchmark process architecture

Do not benchmark both candidates sequentially in one shared Bun environment if they can contaminate the same semantic cache.

Use an orchestrator.

Conceptually:

```text
parent process
    |
    +-- child: direct_7v2 case X
    |
    +-- child: native_11v3 case X
```

Process startup is outside all measurements.

Each child:

```text
loads same native binary
creates fresh Bun environment
creates fresh host
owns independent semantic cache state
runs warmup
runs measured samples
writes raw JSONL
exits
```

---

# 29. Alternate candidate order

Systematic thermal/JIT drift must not always favor one candidate.

For repeated blocks:

```text
block 1:
    direct_7v2
    native_11v3

block 2:
    native_11v3
    direct_7v2

block 3:
    direct_7v2
    native_11v3

block 4:
    native_11v3
    direct_7v2
```

Randomization with a saved deterministic seed is also acceptable.

Record ordering in raw results.

---

# 30. Same native artifact wherever possible

The strongest comparison uses:

```text
one cargo build
one staged iyon-native.node
both candidates
```

because Candidate A's Direct decoder and 11v3 generated ABI can coexist.

If the completed 11v3 architecture requires benchmark-only Cargo features, ensure Candidate A runs against the exact same resulting binary.

Do not compile A with one optimization/features set and B with another.

---

# 31. Production build vs timing build

Maintain two concepts.

## Production-build proof

```bash
bun run build:iyon
```

Proves normal packaging includes the candidate pathways.

## Benchmark timing build

May enable benchmark entrypoints or reset hooks, but must not alter hot-path behavior.

Authoritative results must document exactly:

```text
Cargo features
RUSTFLAGS
profile
LTO
codegen units if customized
```

---

# 32. Instrumentation must not distort timing

Counters such as:

```text
NAPI cache hits
NAPI cache misses
nodes visited
FFI calls
native refs
string bytes
```

may materially affect tiny operations.

Therefore use:

```text
timing run:
    minimal hot-path instrumentation

counter run:
    counters enabled
```

Do not report timing from a build where one candidate performs more atomic counters than another.

---

# 33. Benchmark timing phases

Every sample must contain:

```text
construction_ns
transport_prepare_ns
native_commit_ns
total_ns
forced_frame_ns
```

The exact implementation of the phase differs between architectures.

That is acceptable.

The authoritative number is:

```text
total_ns
```

---

# 34. `direct_7v2` phase definition

Measure:

## Construction

The semantic user operation:

```text
View.text
View.vertical
modifier
shared-path reconstruction
etc.
```

using the historical benchmark-only builder.

## Transport preparation

Only:

```ts
nodeForPerf7v2Bridge(view)
```

This should normally be little more than a WeakMap lookup.

## Native commit

```ts
host.render(bridgeNode)
```

including:

```text
Bun → N-API call
N-API property access
cache lookup
cache-miss lowering
Rust semantic construction
host render mutation
```

## Total

Construction start through native commit completion.

---

# 35. `direct_current` phase definition

Use current production View creation.

Transport preparation:

```ts
nodeForDirectBridge(view)
```

This may include current compatibility materialization.

That distinction is precisely why this candidate exists.

---

# 36. `native_11v3` phase definition

Measure the completed real production API path.

Do not benchmark isolated generated FFI primitives and call that equivalent.

Construction begins at the same user-level semantic action used by `direct_7v2`.

Total ends only after the equivalent native host mutation completes.

Any:

```text
pending backing
route selection
lineage handling
FFI argument preparation
generated FFI calls
native transaction work
```

belongs inside the measured total.

---

# 37. No cost hiding

The benchmark must not classify work according to where it is convenient.

If architecture B performs preparation during construction and architecture A performs it during render:

```text
that is fine
```

because total still contains both.

Do not claim:

```text
"encoding = 0"
```

as an architectural win merely because equivalent preparation moved into construction.

Use phase breakdown only to explain the total.

---

# 38. Primary workload families

At minimum preserve the existing serious workload families:

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

The current benchmark already defines a broad matrix including these workload types. 

Use the same fixture-generation logic wherever semantics permit.

Do not create artificially favorable architecture-specific fixtures.

---

# 39. Primary sizes

Normal trees:

```text
small_view       ~20 nodes
medium_view      ~200 nodes
large_view       ~2,000 nodes
huge_view        ~10,000 nodes
```

Run `huge_view` only where benchmark duration remains sensible.

The current benchmark already follows approximately this size progression. 

---

# 40. Retention modes

Authoritative modes:

```text
COLD

FIRST_USE

IDENTICAL_IDENTITY

SHARED_PATH

SHARED_DEEP

LARGE_SHARED_SUBTREE_CUTOFF

REBUILT_EQUIVALENT
```

Also retain specialized semantic-edit cases where both candidates support an equivalent user operation:

```text
TEXT_METADATA_PATCH

DECORATION_PATCH
```

---

# 41. Wide modes

Wide structures are important because later work changed construction behavior substantially.

Include:

```text
WIDE_PARENT_ONE_EDIT
WIDE_PARENT_INSERT
WIDE_PARENT_REMOVE
```

Widths:

```text
32
256
2,048
10,000
100,000
```

But interpret carefully.

The historical Candidate A uses ordinary immutable arrays.

11v3 may intentionally use later persistent-sequence machinery.

That is **not unfair** in the end-to-end architectural benchmark.

The purpose is to compare the architectures as actual alternatives after 11v3.

However, also report JS construction separately so the source of any wide-case difference is explicit.

---

# 42. Shared-deep scaling

Use depths such as:

```text
4
16
64
128 where practical
```

The edit must modify the same logical leaf.

Measure scaling with:

```text
changed ancestor count
```

rather than total unrelated tree size.

---

# 43. Large shared subtree cutoff

Construct:

```text
new root
├── changed small branch
└── huge stable subtree
```

Stable subtree sizes:

```text
20
200
2,000
10,000
```

Candidate A should demonstrate that Direct's cache hit stops native traversal at the stable subtree root.

11v3 should demonstrate its own retained cutoff.

This workload is central to the comparison.

---

# 44. Exact-identity scaling

Test exact same root at:

```text
20
200
2,000
10,000
```

The correct retained design should be independent of descendant count after the root is known.

If `direct_7v2` unexpectedly scales with total descendant count, investigate before accepting the result.

Historical small Candidate A exact identity was about 1.2 µs. 

---

# 45. Text-oriented workloads

The application is a TUI with heavy text.

Do not allow synthetic container graphs to dominate the architectural decision.

Include:

```text
long_text_wrap_only

long_text_one_span_edit

large_diff_one_hunk_edit

large_decoration_only_change
```

But distinguish:

```text
View transport

from

specialized stream transport
```

---

# 46. Streaming application trace

Create a realistic trace representative of an agent TUI:

```text
stable application shell

History with substantial existing content

active streaming text

periodic text append

occasional finalization of streamed content

occasional new message/view insertion

occasional decoration/layout change

occasional tool/status component update
```

If streaming bytes already use a specialized native path shared by both candidates:

```text
do not push those bytes through Candidate A merely for symmetry
```

Both architectures should use the real common stream path.

Measure the total application operation sequence.

This answers the question that matters:

> Does changing the View bridge materially improve the actual TUI workload?

---

# 47. Synthetic trace requirements

Use a deterministic operation distribution.

Example only — derive final ratios from actual Iyon usage patterns where available:

```text
55% stream append
15% no structural View change
10% text/view replacement
8% layout metadata update
5% component/status update
4% History insertion/finalization
3% larger structural update
```

Record the exact trace schema and seed.

Run at least:

```text
1,000 measured operations per trace
```

Prefer substantially more if runtime permits.

---

# 48. Benchmark count

For normal cases:

```text
warmup >= 50
measured >= 500
```

For sub-microsecond or very small retained operations:

```text
warmup >= 10,000
measured >= 10,000
```

For meaningful p99:

```text
measured >= 1,000
```

The historical PERF-7v2 benchmark used 20 warmups and 200 measured iterations. The new comparison should increase statistical confidence rather than merely reproduce that sample count. 

---

# 49. Raw results

Every measurement run writes JSONL.

Required fields:

```json
{
  "benchmark_version": "PERF-11v4",
  "candidate": "direct_7v2",
  "workload": "...",
  "size": "...",
  "mode": "...",

  "git_sha": "...",
  "historical_candidate_sha": "...",

  "bun_version": "...",
  "bun_revision": "...",
  "rustc_version": "...",
  "target": "...",

  "warmup_iterations": 0,
  "measured_iterations": 0,

  "samples_ns": [],
  "construction_samples_ns": [],
  "transport_prepare_samples_ns": [],
  "native_samples_ns": [],

  "median_ns": 0,
  "p95_ns": 0,
  "p99_ns": 0,

  "median_ci95_ns": [0, 0],
  "p95_ci95_ns": [0, 0]
}
```

Never retain only summary statistics.

---

# 50. Resource measurements

For representative larger cases also record:

```text
CPU user
CPU system
peak RSS delta
heap delta
GC observations where reliably measurable
```

Do not overinterpret tiny heap-delta measurements.

They are secondary evidence.

---

# 51. Statistical analysis

For each matched pair:

```text
direct_7v2
native_11v3
```

calculate:

```text
median ratio

percentage difference

bootstrap CI for difference/ratio

p95 ratio
```

Example:

```text
native_11v3 / direct_7v2 = 0.74

=> 11v3 approximately 26% faster
```

Do not rely only on arithmetic means.

---

# 52. Aggregate results by mode

Produce grouped summaries:

```text
COLD
FIRST_USE
IDENTICAL_IDENTITY
SHARED_PATH
SHARED_DEEP
REBUILT_EQUIVALENT
WIDE
TEXT PATCH
DECORATION PATCH
REALISTIC TRACE
```

Use geometric means for multiplicative candidate ratios across heterogeneous workloads.

Do not average absolute nanoseconds across unrelated workloads.

---

# 53. Construction comparison is mandatory

Produce a dedicated table:

```text
workload / mode
direct_7v2 construction
direct_current construction
native_11v3 construction
```

This directly tests whether the original eager JS DAG was in fact cheap.

The result should answer:

```text
Did later pending/native-oriented JS machinery make construction faster?

Did it make construction slower?

Does the difference matter compared with native transport?
```

---

# 54. Transport comparison is mandatory

Produce another table excluding construction:

```text
transport_prepare + native commit
```

for:

```text
direct_7v2
native_11v3
```

This reveals whether the expected N-API property-traversal cost is actually the dominant Direct disadvantage under Bun 1.4.

But do not use this table to select the production architecture independently of total time.

---

# 55. Historical numbers are sanity checks, not gates

Compare reconstructed `direct_7v2` with historical PERF-7v2 results.

For example:

```text
small IDENTICAL_IDENTITY:
    historical 1,209 ns

small SHARED_PATH:
    historical 38,667 ns
```



Differences are expected because:

```text
Bun changed
Rust changed
renderer changed
machine state may differ
compiler changed
```

The relevant question is whether the **shape** remains plausible.

Example:

```text
IDENTICAL remains O(1)

SHARED_PATH construction remains small

stable subtree cutoff remains visible

Direct native miss traversal remains dominant where expected
```

If these structural properties disappear, inspect the reconstruction.

---

# 56. Correctness gate before results count

A performance result is invalid unless:

```text
all deterministic parity tests pass

all randomized differential tests pass

cache identity tests pass

cache expiry tests pass

the same current Rust renderer is used

the same Bun 1.4 version is used

the same native artifact/configuration is used
```

A faster semantically incomplete candidate loses.

---

# 57. What counts as fair

A fair comparison holds constant:

```text
machine
OS
Bun version
Bun revision
Rust compiler
Cargo profile
Rust TUI core
renderer
host dimensions
workload semantics
text contents
styles
cache initial state
warmup
sample count
process isolation
candidate ordering
```

It does **not** require both architectures to use the same internal JavaScript structures.

That would defeat the purpose.

The end-to-end benchmark compares the architectures as actual alternatives.

---

# 58. What does not count as fair

Reject results if:

```text
7v2 uses Bun 1.3.11 and 11v3 uses Bun 1.4

7v2 uses old Rust while 11v3 uses current Rust as primary comparison

7v2 is reconstructed using current pending View backing

11v3 is measured only at generated FFI primitive level

one candidate is warmed by the other candidate's semantic cache

one candidate includes construction and the other starts after construction

one candidate includes host mutation and the other stops before it

counter-heavy instrumentation exists only on one path

different native binaries are used without justification

the semantic workload differs
```

---

# 59. Expected primary result tables

## End-to-end

```text
Workload | Mode | Size | 7v2 Direct | 11v3 | Delta | Winner
```

## Construction

```text
Workload | Mode | 7v2 construction | current construction | 11v3 construction
```

## Native/transport

```text
Workload | Mode | 7v2 transport+native | 11v3 transport+native
```

## Scaling

```text
Mode | Parameter | 7v2 slope | 11v3 slope
```

## Realistic trace

```text
Candidate | median/op | p95/op | total trace CPU | memory
```

---

# 60. Required analysis questions

The final PERF-11v4 report must answer these questions explicitly.

## Q1 — Is the actual Candidate-A native decoder still present?

Answer:

```text
yes/no
```

with exact source path and entrypoint.

## Q2 — Does normal `bun run build:iyon` compile and execute it?

Answer:

```text
yes/no
```

with executed proof.

## Q3 — Is current `direct` actually historical Candidate A?

Expected likely answer:

```text
No.

The native decoder is equivalent in shape, but current View construction
uses later pending/lazy backing machinery.
```

## Q4 — Was a second checkout required?

Answer and justify.

## Q5 — What does historical-style JS construction cost under Bun 1.4?

Report by workload/mode.

## Q6 — What does completed 11v3 JS construction cost?

Report equivalently.

## Q7 — How expensive is Candidate A's N-API transport under Bun 1.4?

Separate cache-hit and cache-miss cases.

## Q8 — How expensive is 11v3's complete native transport?

Not primitive FFI calls: complete operation.

## Q9 — Which candidate wins exact identity?

## Q10 — Which candidate wins ordinary shallow retained edits?

## Q11 — Which candidate wins deep retained edits?

## Q12 — Which candidate wins large stable-subtree cutoff?

## Q13 — Which candidate wins cold construction?

## Q14 — Which candidate wins wide structural edits?

## Q15 — Which candidate wins text-heavy workloads?

## Q16 — Which candidate wins the realistic TUI trace?

This is the most important answer.

---

# 61. Final architectural classification

At the end, classify the evidence into one of four categories.

## A — 11v3 decisive

Example threshold:

```text
11v3 >= 15% faster on the realistic trace

and

no serious common-mode regression
```

Conclusion:

```text
11v3 has clearly displaced Candidate A.
```

Do not begin PERF-12 automatically.

Only pursue further architecture research if there is another compelling reason.

---

## B — 11v3 modest winner

Example:

```text
5–15% faster realistic trace
```

Conclusion:

```text
11v3 wins, but Candidate A remains architecturally interesting.
```

This is a strong reason to consider a separately designed PERF-12.

---

## C — practical tie

Example:

```text
difference <5%
```

Conclusion:

```text
the much simpler Candidate-A semantic construction model remains competitive.
```

A full PERF-12 investigation is warranted.

---

## D — Candidate A wins

If `direct_7v2` wins realistic end-to-end workloads:

```text
do not dismiss the result because isolated 11v3 FFI calls are faster.
```

The architecture benchmark outranks primitive microbenchmarks.

A separately researched PERF-12 becomes high priority.

---

# 62. Do not design PERF-12 in the result document

If category B, C, or D suggests further investigation, the final paragraph should say only:

> The result justifies a separate PERF-12 investigation into whether the advantageous parts of Candidate A can be combined with a lower-overhead native boundary. PERF-12 must be designed independently from this benchmark and is outside the scope of PERF-11v4.

Do not include:

```text
wire format
FFI function signatures
cache protocol
NativeRef scheme
generated ABI changes
string lanes
transaction design
PersistentSeq transport
```

Those require dedicated research.

---

# 63. Why PERF-12 must be separate

A real Candidate-A-derived FFI architecture would need deliberate design across at least:

```text
Bun 1.4 FFI characteristics

generated ABI source of truth

ABI code generation

Rust/C/TypeScript type agreement

host/runtime handle ownership

NodeId identity

native stable references

WeakView lifetime

cache expiry

failure recovery

atomicity

strings

Text

Diff

Styles

Decoration

Row/Column

Grid

PersistentSeq

History

ViewSlot

ScrollPane

animation

components

cross-transport compatibility

full-schema differential testing

fuzzing

benchmarking
```

That cannot responsibly be expressed as an appendix to this comparison.

PERF-11v4 must provide evidence.

PERF-12, if commissioned, will provide architecture.

---

# 64. Suggested commits

## Commit 1

```text
bench(tui): audit PERF-7v2 direct baseline on Bun 1.4
```

Contains:

```text
source audit
build proof
no benchmark architecture yet
```

## Commit 2

```text
bench(tui): restore faithful PERF-7v2 JS direct candidate
```

Contains:

```text
benchmark-only eager View builder
semantic parity tests
```

## Commit 3

```text
bench(tui): add isolated 7v2 versus 11v3 harness
```

Contains:

```text
process isolation
candidate selection
raw result output
phase timing
```

## Commit 4

```text
bench(tui): complete Bun 1.4 retained architecture comparison
```

Contains:

```text
final workload matrix
results
analysis
decision document
```

No production architecture changes should be necessary in this tranche other than minimal benchmark exposure if the final 11v3 state makes that unavoidable.

---

# 65. Completion criteria

PERF-11v4 is complete only when:

```text
[ ] completed 11v3 SHA is frozen

[ ] Bun 1.4 exact version/revision is recorded

[ ] current Direct native path has been traced

[ ] `bun run build:iyon` has been proven to execute Direct

[ ] historical PERF-7v2 View construction has been inspected from source

[ ] benchmark-only faithful eager 7v2 View builder exists

[ ] current direct compatibility candidate remains separately measurable

[ ] semantic parity passes

[ ] randomized differential testing passes

[ ] cache identity/expiry behavior passes

[ ] candidates run in isolated cache environments

[ ] same native artifact is used where possible

[ ] workloads cover cold, exact identity, retained path, deep, shared cutoff,
    rebuilt equivalent, wide, text, decorations and realistic application trace

[ ] >=500 normal measured samples

[ ] >=1,000 samples for reported p99

[ ] tiny exact paths receive much larger sample counts

[ ] raw samples are retained

[ ] construction and transport/native phases are reported separately

[ ] total end-to-end is the architectural decision metric

[ ] final result explicitly classifies A/B/C/D above

[ ] no new transport architecture has been designed or implemented
```

---

# 66. Final instruction to the implementation agent

**After PERF-11v3 is complete, do not begin another transport optimization. First determine from the actual source whether the PERF-7v2 Direct decoder remains in current HEAD and in the normal `bun run build:iyon` artifact, faithfully reconstruct the historical eager `BridgeViewNode` JavaScript construction model as a benchmark-only candidate over the current Rust semantic cache, and perform an isolated, semantics-equivalent, end-to-end Bun 1.4 comparison against the complete production 11v3 path. Stop after publishing that result. Any attempt to combine the 7v2 JavaScript DAG with FFI belongs to a separately researched and fully specified PERF-12.**
