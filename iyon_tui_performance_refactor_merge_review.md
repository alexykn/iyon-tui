# Iyon TUI `perf-refactor` Remediation Handoff

**Target:** `alexykn/iyon-tui`, branch `perf-refactor`
**Purpose:** take the current refactor from “substantially faster and architecturally promising” to **correct, demonstrably incremental, reproducibly benchmarked, and merge-ready**.

This document is intentionally prescriptive. Implementation agents should treat the algorithms, invariants, tests, and acceptance gates below as requirements. Do not substitute a superficially simpler implementation because a test happens to pass.

The existing performance handoff remains the architectural source of truth. In particular, it requires stable History/stream prefixes, real-work counters, dirty-and-return native mutations, and a measured ≥15% paint threshold before PERF-10 is justified.  It explicitly prohibits walking all 1,000 static History units merely to calculate viewport overflow. 

---

# 1. Final target architecture

When this remediation is complete, a common high-frequency update should look like this:

```text
JS application changes one semantic leaf / appends one stream chunk
    │
    ├─ TS View mutation
    │      changed semantic nodes receive new NodeIds
    │      unchanged semantic nodes retain old NodeIds
    │
    ├─ N-API mutation
    │      decoder sees root NodeId
    │      unchanged subtree → WeakView hit → recursive decode stops
    │      changed path only decoded
    │
    ├─ retained native state mutation
    │      History / stream / component revision updated
    │      affected retained geometry marked dirty
    │      frame marked dirty
    │
    └─ return to JS
           NO synchronous frame preparation
           NO synchronous paint
           NO terminal presentation wait

native driver later runs
    │
    ├─ coalesces mutations
    ├─ refreshes only dirty History units
    ├─ walks only viewport-sized History region
    ├─ stream reflows only safe visual suffix
    ├─ layout cache reuses unchanged ViewIds
    ├─ optional paint cache reuses unchanged physical subtrees
    └─ presents one frame
```

The complexity target for steady-state History projection is:

```text
O(L + D + V)
```

where:

- `L` = number of genuinely Live History units that must remain mounted;
- `D` = number of dirty units whose geometry actually needs refreshing;
- `V` = number of units/flow items necessary to cover the viewport.

It must **not** be:

```text
O(total resident history)
```

with a cheap bounded loop merely appended at the end.

---

# 2. Required remediation order

Do the work in this exact order:

```text
REMED-0   repair the benchmark/instrumentation oracle and capture remediation baseline
REMED-1   fix TS retained semantic identity + Rust derived identity safety
REMED-2   separate native mutations from frame driving/presentation
REMED-3A  add O(1) resident History identity/indexing + event-driven dirty geometry
REMED-3B  replace full History planning with truly bounded projection
REMED-3C  wire native History retirement/partial transfer into retained geometry
REMED-4   harden stream correctness and benchmark multiple visual restart domains
REMED-5   rerun the PERF-10 gate correctly; keep/optimize/remove paint caching based on evidence
REMED-6   bridge/schema/cache-lifetime cleanup
REMED-7   remove or separately validate the accidentally-landed PERF-11/non-TUI work
REMED-8   complete end-to-end validation and merge gate
```

Do not start by “cleaning things up.” Do not combine the History rewrite with the paint-cache rewrite. Do not rewrite the stream engine just because it is nearby.

---

# 3. Global rules for every implementation agent

## 3.1 A passing test is not sufficient if it does not measure the forbidden work

The original handoff explicitly says counters must measure actual work. 

Therefore:

```text
BAD:
    walk all 100,000 units
    construct plans for all of them
    increment history_units_examined only during final 15-unit selector
    test says "15 examined"
    agent declares victory

GOOD:
    every resident-unit touch relevant to projection is instrumented
    warm 100k-history frame touches approximately same count as 1k frame
```

## 3.2 Never hide synchronous work behind a getter or test helper

Specifically:

```text
screenRows()
styleAt()
cellXOfText()
revision()
text()
layout()
```

must not secretly flush/render the TUI.

Snapshot getters return the **last presented frame/state**.

Explicit driver/test APIs perform explicit frame advancement.

Otherwise benchmarks and API semantics become impossible to reason about.

## 3.3 Do not use semantic hashes as correctness

Exact identity/dependency state must remain available.

For example, a Live History key remains conceptually:

```text
root ViewId
+ content width
+ exact reachable [(ComponentId, ComponentRevision), ...]
```

A hash may accompany this for lookup acceleration, but must not become the only correctness comparison.

## 3.4 Do not invent a second retained architecture

Keep:

- persistent `View`;
- current `ResolveSession` / resolution overlay;
- current `LayoutCache`;
- current `StreamModel` + `StreamRowIndex`;
- existing native History transfer machinery.

This remediation finishes those systems. It does not replace them with reconciliation trees, ECS state, generic diffing, or a second stream index.

---

# 4. REMED-0 — repair the performance oracle first

## Goal

Before touching production behavior, make it impossible for later agents to “prove” performance with invalid samples or incomplete counters.

Current problems include:

- 10k workloads defaulting to five samples;
- no proper Bun warm-up;
- COLD and REBUILT_EQUIVALENT being identical in the Bun harness;
- hard-coded `"implementation": "baseline"`;
- non-component Rust render benchmarks receiving `height` but using width-only constraints;
- stream full-presentation measurements defaulting to five iterations;
- `ViewNodesDeepCopied` existing without a meaningful work-site increment.

The current Rust and Bun harnesses show these behaviors directly.  

## Exact files

Primary:

```text
crates/iyon-tui/src/perf.rs
crates/iyon-tui/src/perf_bench.rs
packages/iyon-runtime/bench/tui_performance.ts
```

Potential native test instrumentation:

```text
crates/iyon-native/src/tui.rs
```

---

## 4.1 Add counters that reveal the currently hidden work

Add at least:

```rust
HistoryResidentUnitsTouched
HistoryDirtyUnitsRefreshed
HistoryLiveUnitsResolved
HistoryFullRebuildUnits

HostMutationsCommitted
HostFramesPrepared
HostFramesPresented

PaintCacheKeyBuilds
PaintCacheKeyOwnedStringBytes
```

### Definitions

`history_resident_units_touched`

Increment whenever the projection algorithm reads a History unit for actual per-frame decision work:

- checking/revalidating a dirty unit;
- resolving a Live unit;
- selecting a unit;
- rendering a selected unit;
- inserting an off-screen Live zero-height mount.

Do **not** increment for:

- reading aggregate `history.units.len()`;
- manipulating a set of ordinals without dereferencing the unit;
- a high-level “projection occurred” logical event.

`history_full_rebuild_units`

Increment for every unit touched by the specifically allowed full rebuild following content-width invalidation.

This separates:

```text
ordinary steady-state work
```

from:

```text
one allowed resize rebuild
```

`host_frames_prepared`

Increment directly around the real `prepare_frame`.

`host_frames_presented`

Increment only when a frame is actually handed to the backend/headless presenter.

This counter is critical for proving 100 mutations did not synchronously cause 100 frames.

---

## 4.2 Remove or make `ViewNodesDeepCopied` meaningful

Current `perf.rs` declares `ViewNodesDeepCopied`. 

Do one of these, preferably the first:

### Preferred

Delete `ViewNodesDeepCopied`.

Use structural-sharing tests and allocation benchmarks instead.

### Alternative

Define exactly what constitutes a deep copy and increment it at every work site.

Do **not** leave a permanent-zero counter.

A zero that cannot become non-zero proves nothing.

---

## 4.3 Authoritative sample policy

Separate benchmark modes:

```text
SMOKE
    useful during development
    20-30 measured iterations okay
    NEVER used for p95 acceptance

AUTHORITATIVE
    >=200 measured iterations
    used for p50/p95 decisions
    raw samples retained

P99
    >=1000 measured iterations
    otherwise p99 is explicitly informational
```

Every authoritative run must contain:

```json
{
  "benchmark_version": 2,
  "benchmark": "...",
  "implementation": "pre_remediation",
  "git_sha": "...",
  "width": 80,
  "height": 24,
  "warmup_iterations": 20,
  "iterations": 200,
  "samples_ns": [],
  "median_ns": 0,
  "p95_ns": 0,
  "p99_ns": 0,
  "p99_informational": true,
  "counters": {},
  "rustc_version": "...",
  "bun_version": "...",
  "target": "...",
  "cargo_profile": "release"
}
```

`implementation` must come from an explicit environment variable, e.g.:

```text
PERF_IMPLEMENTATION=pre_remediation
```

If it is missing in authoritative mode, **fail the benchmark**, rather than silently calling everything `baseline`.

---

## 4.4 Warm-up rules

Warm-up is not part of the sample set.

The sequence must be:

```text
construct benchmark state
warm cache/process
reset perf counters
clear measured samples
begin timed samples
```

For warm identity benchmarks:

```text
warm-up >= 10 iterations
```

For huge 10k trees, 10 is acceptable.

Do not use the first measured sample to establish the cache that the remaining samples supposedly measure.

---

## 4.5 Define the four View modes unambiguously

### COLD

Purpose:

```text
fresh semantic DAG
fresh relevant retained cache
warm process/runtime
```

Per sample:

```text
construct fresh host/cache OUTSIDE timed region
construct fresh View DAG
start timer
commit/render depending benchmark
stop timer
dispose host
```

Host construction must not be included unless explicitly benchmarking host startup.

Counter expectation:

```text
native decoder: essentially all misses
no retained semantic subtree hit from prior sample
```

### IDENTICAL_IDENTITY

One persistent host.

One exact `View` object.

Before timing:

```text
render/commit it at least 10 times
```

During timing:

```text
same View object
same root NodeId
same descendants
```

Decoder expectation:

```text
root NodeId cache hit
recursive decoder stops immediately
```

### SHARED_PATH

Construct:

```text
root
├── enormous shared subtree S
└── changed leaf X
```

Warm a version containing exactly `S`.

Every sample:

```text
new root NodeId
same exact S object / NodeId
new changed leaf
```

Counter expectation:

```text
root miss
shared child hit
changed child miss
NO recursive decode of S
```

### REBUILT_EQUIVALENT

Persistent warmed process/host.

Every sample constructs a completely new semantic tree with **equal semantics but new identities**.

Expectation:

```text
NodeIds all fresh
identity cache cannot reuse equivalent semantics
```

### Mandatory harness assertion

The benchmark code itself must fail if COLD and REBUILT_EQUIVALENT execute the exact same state/cache arrangement.

The current Bun harness literally calls `render(tree(nodes))` for both cases; that is unacceptable. 

---

## 4.6 Split mutation latency from frame latency

After REMED-2, normal host mutations must no longer render.

Therefore every native benchmark must distinguish:

### Commit latency

```text
JS semantic construction
→ nodeForBridge
→ N-API
→ Rust decode/cache
→ retained-state mutation
→ dirty flag
→ return
```

No flush.

### Frame latency

```text
commit
→ explicit deterministic frame advance
→ prepare
→ layout
→ paint
→ headless present
```

Use the already-existing explicit `advanceTime(0)` / `Tui.advance(0)` concept as the forced deterministic frame boundary.

That means the benchmark can use the same structure before and after scheduling remediation:

```ts
const started = Bun.nanoseconds();
host.render(nodeForBridge(view));
host.advanceTime(0); // explicit frame boundary
const elapsed = Bun.nanoseconds() - started;
```

Before REMED-2, `render()` may already render and `advanceTime(0)` will mostly no-op.

After REMED-2, `render()` commits and `advanceTime(0)` performs the frame.

This keeps semantic comparability.

---

## 4.7 Fix bounded render benchmarking

The 80×24 benchmark must actually constrain **both** dimensions.

Currently the non-component benchmark path accepts `height` but calls width-only layout. 

After this tranche there must not be code shaped like:

```rust
fn render_view_timed(..., width, height, ...) {
    ...
    LayoutConstraints::width_only(width)
}
```

If no existing helper represents both constraints, add one.

Then assert in benchmark/debug mode:

```rust
assert!(surface.width() <= width);
assert!(surface.height() <= height);
```

where bounded semantics require it.

---

## 4.8 Capture baseline

Once the oracle is fixed and before REMED-1:

```text
implementation = pre_remediation
git_sha = exact remediation-baseline commit
```

Capture:

```text
View modes
History 1k/10k/100k scaling fixtures
stream traces
native commit latency
native forced-frame latency
paint disabled
paint enabled
```

Generated result files are local/CI artifacts only.

Never commit them.

### REMED-0 acceptance

- authoritative runs reject insufficient samples;
- raw samples are available;
- warm-up excluded;
- COLD/IDENTICAL/SHARED/REBUILT behavior is mechanically distinct;
- 80×24 benchmark is actually bounded;
- implementation label isn't hardcoded;
- no dead “always zero” counter;
- baseline SHA recorded.

Suggested commit:

```text
test(perf): repair retained TUI performance oracle
```

---

# 5. REMED-1 — fix retained semantic identity completely

## Problem

The current TS `View` constructor correctly creates a fresh private NodeId for a new outer semantic node.

But decorated text mutation currently does this conceptually:

```ts
const child = map(existingDecorated.child);

return new View({
  ...existingDecorated,
  child,
});
```

The modified child inherits the old child's `id/schema`.

The native decoder intentionally reads NodeId first and returns an existing `WeakView` hit before inspecting payload. 

Therefore:

```text
changed child semantics
+ old child NodeId
+ old Rust View still retained
=
stale Rust semantic subtree returned
```

The current TS implementation contains exactly this decorated `mapText()` shape. 

---

## 5.1 Strengthen the types first

Current constructor accepts:

```ts
BridgeViewNode | BridgeViewNodeDraft
```

Do not continue allowing mutation helpers to pass an identity-bearing node as a draft.

Target:

```ts
private constructor(node: BridgeViewNodeDraft) {
  nodes.set(this, identifyDraft(node));
  Object.freeze(this);
}
```

Define a text draft type:

```ts
type BridgeTextNode =
  Extract<BridgeViewNode, {
    kind: typeof BRIDGE_VIEW_KIND.text;
  }>;

type BridgeTextDraft =
  Extract<BridgeViewNodeDraft, {
    kind: typeof BRIDGE_VIEW_KIND.text;
  }>;
```

Then:

```ts
private mapText(
  map: (text: BridgeTextNode) => BridgeTextDraft,
): View
```

A text mutation callback should be **incapable by API shape** of returning `id` or `schema`.

---

## 5.2 Do not use object spread for semantic mutation

Do not write:

```ts
{ ...text, wrap: ... }
```

because `text` contains identity.

Write every semantic field deliberately:

```ts
function textDraft(
  text: BridgeTextNode,
  overrides: Partial<Pick<BridgeTextDraft, "wrap" | "align">>,
): BridgeTextDraft {
  return {
    kind: BRIDGE_VIEW_KIND.text,
    spans: text.spans,
    wrap: overrides.wrap ?? text.wrap,
    align: overrides.align ?? text.align,
  };
}
```

Then:

```ts
wrap(mode: WrapMode): View {
  return this.mapText((text) => textDraft(text, {
    wrap: wrapCode(mode),
  }));
}

textAlign(align: HorizontalAlign): View {
  return this.mapText((text) => textDraft(text, {
    align: horizontalAlignCode(align),
  }));
}
```

---

## 5.3 Correct decorated mutation

Target algorithm:

```ts
private mapText(
  map: (text: BridgeTextNode) => BridgeTextDraft,
): View {
  const node = nodeForBridge(this);

  if (node.kind === BRIDGE_VIEW_KIND.text) {
    return new View(map(node));
  }

  if (
    node.kind === BRIDGE_VIEW_KIND.decorated &&
    node.child.kind === BRIDGE_VIEW_KIND.text
  ) {
    const changedChild = identifyDraft(map(node.child));

    return new View({
      kind: BRIDGE_VIEW_KIND.decorated,
      child: changedChild,
      decoration: node.decoration,
    });
  }

  return this;
}
```

Important identity result:

```text
old decorated root A
old child B

text semantic mutation

new root C
new child D
```

Both changed nodes receive new identities.

Unchanged span storage may still be shared.

---

## 5.4 Explicit identity rule

Add this comment next to the helper:

```text
Any object whose semantic payload changes MUST receive a new NodeId.
An unchanged child retained by a new parent MUST retain its NodeId.
NodeId reuse across changed semantics is a correctness bug because native
decoding stops on the first live NodeId cache hit.
```

---

## 5.5 Fix the lightweight TS row harness

Current `rows()` combines `container` and `clamp`.

Do not.

Correct:

```ts
case BRIDGE_VIEW_KIND.container:
  return rows(node.child);

case BRIDGE_VIEW_KIND.clamp:
  return rows(node.child).slice(0, node.maxRows);
```

The bridge type makes `maxRows` optional for the combined container/clamp representation. 

More importantly:

**cache-sensitive correctness tests must render through native.**

Do not prove the NodeId fix solely using `textRowsForHarness()`.

---

## 5.6 Native regression test

Add to:

```text
packages/iyon-runtime/tests/tui_values.test.ts
```

Use a shape guaranteed to put the text below a decorator before mutation:

```ts
test("decorated text alignment mutation receives fresh nested identity", () => {
  const original = View.text("x").fillWidth();

  const originalNode = nodeForBridge(original);
  expect(originalNode.kind).toBe(BRIDGE_VIEW_KIND.decorated);
  const originalChild = originalNode.child;

  const changed = original.textAlign("end");
  const changedNode = nodeForBridge(changed);

  expect(changedNode.id).not.toBe(originalNode.id);
  expect(changedNode.child.id).not.toBe(originalChild.id);

  expect(Object.isFrozen(changedNode)).toBe(true);
  expect(Object.isFrozen(changedNode.child)).toBe(true);
});
```

Then the important cache test:

```ts
const host = new NativeTuiHost(10, 2, true);

host.render(nodeForBridge(original));
host.advanceTime(0);

expect(host.cellXOfText(0, "x")).toBe(0);

// The old child is retained/native-cached at this point.
host.render(nodeForBridge(changed));
host.advanceTime(0);

expect(host.cellXOfText(0, "x")).toBe(9);
```

Test:

```text
wrap()
noWrap()
textAlign(start → center → end)
```

Do not test only root IDs.

---

## 5.7 Rust `ViewFlags` derived-state safety

Current Rust `map_node()`:

```text
shallow clone
apply arbitrary mutation
new ViewId
```

but does not recompute flags. 

Immediately after the update:

```rust
update(&mut next);
next.flags = ViewNode::compute_flags(&next.kind);
next.id = next_view_id();
```

Even if current callers only mutate style/text, this prevents a later topology mutation from silently leaving `contains_component_identity()` stale.

Test:

```text
construct component-free node
internal mutation changes kind/child to contain ComponentSlot
new ViewId
new flags say contains component
```

If exposing such a mutation solely for a unit test would be ugly, add a focused private `#[cfg(test)]` helper.

---

## REMED-1 reject conditions

Reject the tranche if an agent:

- only changes the outer NodeId;
- adds a cache clear instead of fixing identity;
- disables the native NodeId cache;
- compares semantic equality on every cache hit;
- adds a special case only for `.noWrap()`;
- uses the TS helper but does not test native retained cache behavior.

### Acceptance

All changed semantic bridge nodes have new IDs.

All unchanged descendants retain IDs.

Old cached child + new decorated parent produces correct new physical output.

Suggested commit:

```text
fix(runtime): preserve retained identity across nested View mutation
```

---

# 6. REMED-2 — mutations dirty state; driver renders

This is a major architectural repair.

The original contract is explicit: normal N-API mutations dirty retained state and return.  High-frequency stream append is likewise supposed to end with “mark host dirty → return.” 

Current `HostViewSlot`, `HostScrollPane`, `HostTextInput`, History operations and stream paths call `advance_and_render()` directly.   

Meanwhile the ordinary Rust runner already contains a real presentation scheduler with an 8 ms minimum presentation interval. 

Reuse that model.

---

## 6.1 Extract presentation scheduling

Move `PresentationScheduler` into a private reusable module, for example:

```text
crates/iyon-tui/src/application/presentation.rs
```

Both:

```text
application/run.rs
application/host.rs
```

must use the same scheduling semantics.

Do not copy-paste a second scheduler implementation.

---

## 6.2 Host mutation classification

The following are **ordinary mutations** and must never prepare/paint/present a frame directly:

```text
HostViewSlot::set_view
HostViewSlot::set_animation
HostViewSlot::stop_animation

HostScrollPane::set_content
HostScrollPane::follow_end

HostTextInput::set_text
HostTextInput::clear
HostTextInput::set_border
HostTextInput::set_multiline

HostTextStream::append
HostTextStream::update
HostTextStream::seal

HostHistory::set_layout
HostHistory::push
HostHistory::freeze
HostHistory::discard_live
HostHistory::push_stream
HostHistory::seal_stream

TuiHost::render
TuiHost::set_theme
TuiHost::set_history
TuiHost::resize

direct dispatch/enqueue mutations
```

Allowed explicit frame-driving operations:

```text
TuiHost::open            initial frame
TuiHost::poll_terminal   driver iteration
TuiHost::wait_for_output driver loop
TuiHost::advance_time    explicit deterministic/test advancement
TuiHost::exit            finalization
```

`close()` should restore/shut down; it should not invent a new final frame.

`exit()` may force the documented final frame.

---

## 6.3 HostTextStream needs commit, not simply “remove render_host”

Be careful here.

Current `HostTextStream::render_host()` does important semantic work before rendering:

```text
refresh attached History stream model
invalidate frame
advance/render
```

The correct replacement is not:

```text
delete render_host()
```

It is:

```text
rename/restructure as commit_host_change()
```

Conceptually:

```rust
fn commit_host_change(&self) -> Result<()> {
    let host = ...;

    if let Some(host) = host {
        let mut inner = host.lock()?;

        if let Some(handle) = self.attached_handle()? {
            inner
                .running
                .scene_history_mut()
                .ok_or(...)?
                .refresh_stream(handle)?;
        }

        inner.running.invalidate_frame();

        // STOP HERE.
    }

    Ok(())
}
```

Do not call:

```rust
advance_ready
prepare_frame
render
present
```

inside it.

---

## 6.4 Fix History `set_layout` dirtying

Attached `HostHistory::set_layout()` currently changes the History layout but does not visibly invalidate the host in the same way push/freeze do.

Correct:

```rust
pub fn set_layout(&self, layout: HistoryLayout) -> Result<()> {
    let mut inner = self.lock_mut()?;

    inner
        .running
        .scene_history_mut()
        .ok_or(...)?
        .set_layout(layout);

    inner.running.invalidate_frame();
    Ok(())
}
```

No frame.

---

# 7. Build a correct host driver

Current `HostInner::advance_and_render()` roughly does:

```rust
let status = running.advance_ready(now)?;
if status.dirty {
    render()?;
}
```

But there is another subtle problem: `render()` prepares a frame before `present_frame()` checks whether a previous asynchronous presentation receipt is still in flight.

Do **not** clear `RunningApp::dirty` by preparing a frame that you cannot actually submit.

The generic runner avoids this by preparing only when `in_flight.is_none()`.

The native host must do the same.

---

## 7.1 Required HostInner state

Conceptually:

```rust
struct HostInner {
    running: HostRunning,
    backend: HostBackend,

    frame: PreparedSceneFrame,

    presentation: Option<PresentReceipt>,
    presentation_scheduler: PresentationScheduler,

    now: Instant,
    headless: bool,
    closed: bool,
}
```

---

## 7.2 Reap presentation before preparing another frame

Implement:

```rust
fn reap_presentation(&mut self) -> Result<bool>
```

Semantics:

```text
no presentation
    → return true

receipt completed successfully
    → remove receipt
    → scheduler.presented(now)
    → return true

receipt still pending
    → leave it in place
    → return false

receipt closed/failed
    → error
```

Do not prepare a new frame when this returns false.

---

## 7.3 Normal driver iteration

Conceptually:

```rust
fn drive_once(&mut self) -> Result<()> {
    self.sync_real_time();

    let can_present = self.reap_presentation()?;

    let status = self
        .running
        .advance_ready(self.now)
        .map_err(...)?;

    if !can_present {
        // IMPORTANT:
        // running.dirty remains true because prepare_frame was never called.
        return Ok(());
    }

    if !self
        .presentation_scheduler
        .should_present(status.dirty, self.now)
    {
        return Ok(());
    }

    self.prepare_and_begin_frame()?;
    Ok(())
}
```

For a headless backend, presentation is effectively synchronous; mark the scheduler presented immediately.

For real terminal presentation, mark it presented when the receipt resolves, matching the generic runner.

---

## 7.4 Forced deterministic frame

`advance_time(duration)` is an explicit driver/test primitive.

Its contract may be:

```text
advance host clock
process everything ready at or before that clock
force one dirty frame regardless of 8 ms presentation throttle
do not run future timers
return after headless frame or explicit real presentation completion semantics
```

Implement a helper:

```rust
fn flush_ready_frame(&mut self) -> Result<()>
```

Pseudo-algorithm:

```text
wait/reap prior presentation
loop:
    status = advance_ready(now)
    if !status.more_ready:
        break

if status.dirty:
    prepare frame
    submit frame

if explicit blocking flush on real backend:
    wait for receipt
```

This is allowed because it is an explicitly synchronous test/manual-clock primitive, not an ordinary mutation.

---

## 7.5 Snapshot getter semantics

After REMED-2:

```ts
stream.append("x");
tui.screenRows();
```

may return the previously presented frame.

That is correct.

Tests that need immediate deterministic pixels use:

```ts
stream.append("x");
tui.advance(0);
expect(tui.screenRows()...)
```

Do **not** change `screenRows()` to flush.

---

# 8. REMED-2 tests

## Test A — mutations do not prepare frames

Reset counters.

Perform:

```text
100 ViewSlot setView operations
```

without driver advancement.

Expected:

```text
host_mutations_committed = 100
host_frames_prepared = 0
host_frames_presented = 0
```

Then:

```text
advance_time(0)
```

Expected:

```text
host_frames_prepared = 1
host_frames_presented = 1
```

Repeat separately for:

```text
History.push/freeze
TextInput.setText
ScrollPane.setContent
TextStream.append
TuiHost.render
```

---

## Test B — 100 stream appends coalesce

```text
attach one TextStream
warm initial frame
reset counters

append 100 chunks
```

Before flush:

```text
frames prepared = 0
paint nodes visited = 0
```

After one flush:

```text
frames prepared = 1
```

Do not assert exact paint node count.

---

## Test C — realtime driver still animates

Keep/update the existing real-time tests that run `nextEvent/nextAction`.

The driver must still advance:

```text
ViewSlot animation
stream pacing
component ticks
```

without manual `advance(0)`.

Current real-time tests already exercise this architecture. 

---

## Test D — in-flight frame is not lost

Use a backend/test receipt deliberately held pending.

Sequence:

```text
present frame A
before A's receipt resolves:
    mutate to B
    run driver
```

Expected:

```text
B remains dirty
B is NOT prepared and forgotten
```

Resolve A.

Next driver:

```text
prepares/presents B
```

This test is mandatory.

---

## REMED-2 banned solution

After completion, a search of `application/host.rs` for:

```text
advance_and_render
```

should show no ordinary mutation call site.

If the helper remains, every call must belong to an explicitly documented driver/finalization path.

Suggested commit:

```text
fix(tui): decouple native state mutation from frame presentation
```

---

# 9. REMED-3A — build a real retained History index

This is the largest remaining algorithmic task.

Current projection starts by collecting every unit and creating a plan for every unit. 

Current `History::index_of()` also performs a linear `.position(...)`, meaning a stream append targeting the tail can itself scan the whole resident History before projection even begins. 

`next_stream_wakeup()` and `advance_streams()` likewise iterate the resident sequence.

All of this needs to become retained/indexed.

---

# 10. Resident ordinal model

Keep `VecDeque<HistoryUnit>`.

Do not replace the collection unless necessary.

Add a private contiguous resident ordinal:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ResidentOrdinal(u64);
```

History:

```rust
pub struct History {
    units: VecDeque<HistoryUnit>,

    first_ordinal: u64,
    next_ordinal: u64,

    unit_index: HashMap<HistoryUnitId, ResidentOrdinal>,

    flow: RefCell<HistoryFlowIndex>,

    layout: HistoryLayout,
    native: NativeFrontier,
}
```

Unit:

```rust
pub(crate) struct HistoryUnit {
    id: HistoryUnitId,
    ordinal: ResidentOrdinal,
    boundary: FlowBoundary,
    content: HistoryUnitContent,
    layout: RefCell<HistoryUnitLayout>,
}
```

Invariant:

```text
next_ordinal == first_ordinal + units.len()
```

and for every deque index `i`:

```text
units[i].ordinal == ResidentOrdinal(first_ordinal + i)
```

---

## 10.1 Append

```rust
ordinal = ResidentOrdinal(next_ordinal)
next_ordinal = next_ordinal.checked_add(1).expect(...)

push_back(unit)
unit_index.insert(id, ordinal)
```

---

## 10.2 Front retirement

```text
remove front unit
remove ID map entry
remove its ordinal from all retained sets
first_ordinal += 1
```

`next_ordinal` does not change.

---

## 10.3 Tail discard

Because current semantic rules only permit discard of a tail Live unit:

```text
remove tail
remove ID map entry
remove all retained-set entries
next_ordinal -= 1
```

The private positional ordinal may therefore be reused on the next append.

That is safe because:

```text
ResidentOrdinal is NOT semantic identity
ResidentOrdinal never escapes History
all references to removed tail ordinal must be removed first
```

Add debug assertions proving that.

---

## 10.4 O(1) ID lookup

Replace:

```rust
units.iter().position(...)
```

with:

```rust
fn index_of(&self, id: HistoryUnitId) -> Result<usize, HistoryError> {
    let ordinal = *self
        .unit_index
        .get(&id)
        .ok_or(HistoryError::UnitNotFound { unit: id })?;

    let relative = ordinal
        .0
        .checked_sub(self.first_ordinal)
        .ok_or(...)?;

    let index = usize::try_from(relative).map_err(...)?;

    let unit = self.units.get(index).ok_or(...)?;

    debug_assert_eq!(unit.id, id);
    debug_assert_eq!(unit.ordinal, ordinal);

    Ok(index)
}
```

This affects:

```text
freeze
discard_live
replace_live_with_stream
update_stream
refresh_stream
seal_stream
```

A high-frequency stream append after 100,000 static units must no longer search those 100,000 units for its handle.

---

# 11. Retained flow data

Target:

```rust
struct HistoryFlowIndex {
    content_width: Option<u16>,

    known_unit_height_sum: usize,

    // Number of Default-boundary gaps between resident units.
    // The gap before the first resident unit is NOT counted here.
    internal_default_gap_count: usize,

    dirty: BTreeSet<ResidentOrdinal>,
    live: BTreeSet<ResidentOrdinal>,
    streams: BTreeSet<ResidentOrdinal>,

    // History semantic rules allow at most one open stream, and it is the tail.
    open_stream: Option<ResidentOrdinal>,

    protected_band: Option<ProtectedBandIndex>,
}
```

Do not store one global `cached_total_height: Option<usize>` that must be rediscovered by walking units.

---

# 12. Define cached unit height precisely

`HistoryUnitLayout.height` means:

> current **resident semantic/presentation contribution** of that unit at `content_width`.

It is not necessarily the original entire semantic View height after some rows have transferred to native history.

Examples:

```text
ordinary static:
    full static semantic height

partially native-transferred static:
    remaining frozen row count

stream:
    frozen partial prefix rows
    +
    current retained StreamRowIndex anchor count
```

This definition is vital.

---

# 13. Dirty-unit accounting

Implement one central method.

Conceptually:

```rust
fn mark_unit_dirty(&self, ordinal: ResidentOrdinal) {
    let index = self.index_for_ordinal(ordinal);
    let mut layout = self.units[index].layout.borrow_mut();
    let mut flow = self.flow.borrow_mut();

    if !flow.dirty.insert(ordinal) {
        return; // already dirty; never subtract twice
    }

    if let Some(old_height) = layout.height.take() {
        flow.known_unit_height_sum =
            flow.known_unit_height_sum
                .checked_sub(old_height)
                .expect("...");
    }

    layout.key = None;
}
```

Then recording:

```rust
fn record_unit_height(
    &self,
    ordinal: ResidentOrdinal,
    key: HistoryUnitLayoutKey,
    height: usize,
) {
    ...
    debug_assert!(layout.height.is_none());

    layout.width = flow.content_width;
    layout.key = Some(key);
    layout.height = Some(height);

    flow.known_unit_height_sum += height;
    flow.dirty.remove(&ordinal);
}
```

No separate stale-count state.

No global invalidation because one unit changed.

---

# 14. Gap accounting

Do not build `FlowItem::Gap` for all units merely to calculate total geometry.

Maintain:

```text
internal_default_gap_count
```

For resident unit `i > 0`, an internal gap exists iff:

```text
unit.boundary == FlowBoundary::Default
```

Then:

```rust
internal_gap_height =
    internal_default_gap_count * usize::from(history.layout().gap);
```

The gap before the **first** resident unit is special because native History may own/freeze it.

Compute that O(1) from:

```text
history.native.last_native_unit
history.native.leading_gap
history.units.front().boundary
history.layout.gap
```

Similarly top padding comes directly from:

```text
Semantic → layout.padding.top
Frozen   → frozen rows length
Native   → 0
```

---

# 15. Total flow height

Once dirty set is empty:

```rust
total_flow_height =
    resident_top_padding
    + resident_leading_gap
    + flow.known_unit_height_sum
    + flow.internal_default_gap_count * layout.gap
    + layout.padding.bottom;
```

No loop.

If dirty set is not empty, refresh **only dirty ordinals**, then compute the total.

---

# 16. Layout changes

Do not invalidate unit heights merely because the vertical gap changed.

`HistoryLayout` changes split into:

### Width-affecting

Left/right padding changes effective content width.

On next projection:

```text
new content width != flow.content_width
```

Then one full rebuild is allowed.

### Non-width-affecting

Top/bottom padding or gap:

```text
unit measurement remains valid
```

Total geometry uses the new values directly.

No N-unit invalidation required.

---

# 17. Stream indexing inside History

Maintain:

```text
streams
open_stream
```

Then:

```rust
next_stream_wakeup()
```

does not call:

```rust
self.units.iter()
```

Instead:

```text
open_stream == None → None
open_stream == Some(ord) → direct ordinal→deque lookup → stream.next_wakeup()
```

Likewise `advance_streams(now)` only accesses the open stream.

When the stream changes:

```text
stream.advance/refresh
→ mark stream unit dirty
```

Do not calculate its new row height inside the mutation itself.

The frame refresh calculates that geometry using the retained stream index.

---

# 18. REMED-3B — projection algorithm

The production projection must not contain:

```rust
history.units().collect::<Vec<_>>()
```

It must not allocate:

```rust
vec![None; history.units.len()]
```

It must not construct one `UnitPlan` per resident static unit.

It must not construct one `FlowItem` per resident unit.

---

# 19. Phase 1 — width invalidation

At projection start:

```rust
let content_width = ...
```

Then:

```text
flow.content_width == None
    → establish width; all unmeasured/new units dirty

flow.content_width == content_width
    → normal steady path

flow.content_width != content_width
    → one allowed full rebuild
```

Full rebuild:

```text
clear known_unit_height_sum
mark every resident unit dirty
set content_width
history_full_rebuild_units += N
```

This is the only ordinary terminal-resize path allowed to touch the complete static resident sequence.

After the rebuild finishes, the next stable-width frame must return to bounded behavior.

---

# 20. Phase 2 — resolve Live roots only

Current projector resolves Live units because off-screen Live components must remain part of the scene's mount graph.

Preserve that.

Do **not** optimize by simply ignoring off-screen Live roots.

`ResolveSession` tracks global duplicate components, cycles, capabilities and the mount graph, so it must still observe every genuinely Live unit. 

But iterate:

```text
flow.live
```

not:

```text
all History units
```

For each live ordinal, in semantic order:

```rust
let (view, dependencies) =
    session.resolve_root_with_dependencies(unit_view)?;
```

Construct:

```rust
HistoryUnitLayoutKey::Live {
    view: unit_view.id(),
    dependencies,
}
```

Compare with the cached unit layout.

If changed:

```text
mark_unit_dirty(live ordinal)
```

Store a small frame-local structure:

```rust
struct LiveFrameEntry {
    ordinal: ResidentOrdinal,
    view: View,
    key: HistoryUnitLayoutKey,
}
```

There may be one of these for every Live unit.

That is acceptable because `L` represents actual mounted dynamic semantics.

### Do not add a component-global revision shortcut yet

A global component epoch is tempting.

Do not use one to skip Live resolution unless you also implement a fully correct cached mount-topology replay mechanism preserving:

```text
duplicate-component detection
cycle detection
MountGraph parentage
capabilities
focus scopes
resolution overlay snapshots
```

That optimization is not necessary to eliminate the static-history O(N).

---

# 21. Phase 3 — refresh dirty units

Snapshot the dirty ordinals because recording height mutates the dirty set.

For each dirty ordinal:

### Static

```text
measure static View
build Static(ViewId) key
record height
```

Static is component-free by taxonomy.

### Live

Use the already resolved `LiveFrameEntry`.

Do not resolve it again.

Measure using the current resolution overlay.

Record exact Live key and height.

### Stream

Do not invent another stream layout algorithm.

Use the existing:

```rust
stream.prepare_from(start, content_width)
```

which already routes through the retained `StreamLayoutCache` and `reindex_in_place`. 

Compute:

```text
height = frozen_prefix_rows + index.anchors.len()
```

Store the resulting stream key/height.

The existing stream row index already tracks semantic and visual restart coordinates and retains the stable row prefix. 

---

# 22. Remove another hidden scan: `stream_projection_state`

Do not retain a helper that does:

```text
history.units.iter().find(unit id)
```

when its caller already has the unit.

Change from roughly:

```rust
stream_projection_state(history, unit_id)
```

to:

```rust
stream_projection_state(history, unit)
```

and derive `semantic_base()` directly from `unit.content`.

All hot-path lookup must be positional/indexed.

---

# 23. Phase 4 — exact O(1) total

After dirty refresh:

```rust
debug_assert!(flow.dirty.is_empty());
```

Compute total using the retained sums/counts.

If this requires iterating units, the tranche is not done.

---

# 24. Phase 5 — viewport selection without N-sized arrays

Define:

```rust
struct HistorySelection {
    top_padding: Option<Selected>,
    bottom_padding: Option<Selected>,

    // Only selected gaps/units; bounded by viewport.
    gaps_before: BTreeMap<ResidentOrdinal, Selected>,
    units: BTreeMap<ResidentOrdinal, Selected>,

    remaining: usize,
}
```

This is bounded by visible flow, not resident length.

---

# 25. FollowEnd selector

Pseudo-code:

```text
remaining = viewport height

take bottom padding

index = units.len - 1

while index exists and remaining > 0:
    ordinal = unit.ordinal
    height = cached unit height

    visible = min(height, remaining)
    offset = height - visible

    record selected unit(ordinal, offset, visible)
    remaining -= visible

    if remaining == 0:
        break

    gap = gap_before(index)

    if gap > 0:
        visible_gap = min(gap, remaining)
        record selected gap_before(ordinal)
        remaining -= visible_gap

    index -= 1

if remaining > 0:
    take top padding
```

Touches only as many units as necessary to fill the viewport.

---

# 26. NativeFrontier selector

Pseudo-code:

```text
remaining = viewport height

take semantic/frozen top padding

index = 0

while index < units.len and remaining > 0:
    gap = gap_before(index)
    take visible gap

    if remaining == 0:
        break

    take front portion of unit[index]
    remaining -= visible

    index += 1

if remaining > 0:
    take bottom padding
```

Again: viewport-bounded.

The original handoff specifically requires this forward-bounded behavior. 

---

# 27. Preserve off-screen Live mounts

Current projector inserts a zero-height viewport for an unselected Live unit.

Do not accidentally remove this semantic behavior.

After selection, final View construction must merge:

```text
selected ordinals
+
all live ordinals
```

in semantic order.

For an off-screen Live ordinal:

```rust
View::row_viewport_with_height(
    resolved_live_view.clone(),
    0,
    Some(0),
)
```

For a selected Live ordinal:

```text
use selected height/offset
```

Do not push its zero-height version as well.

Complexity:

```text
O(V + L)
```

not O(N).

---

# 28. Final root construction

Do not loop over every historical `FlowItem`.

Instead merge:

```text
selected gap map
selected unit map
live ordinal sequence
top/bottom padding
```

Pseudo-order:

```text
if !native_anchored:
    spacer(slack)

selected top padding if any

for each ordinal in sorted union(selected units, live units):
    selected gap before ordinal if any

    if selected unit:
        append selected unit_view
        update rendered row
        process frozen overlay if applicable

    else:
        append zero-height Live view

selected bottom padding if any

if native_anchored:
    spacer(slack)
```

This is also where `frozen_overlay.row` must remain byte-for-byte equivalent to the current projector.

---

# 29. Flexible Live height

Current selection has special handling for Live Views with flexible height.

Preserve it.

Do not recursively inspect arbitrary static History during selection.

For selected Live units, use:

```text
resolved Live View
current resolution overlay
```

and the existing `flexible_height(...)` logic.

Optionally cache its last result in `HistoryUnitLayout` after Live resolution.

---

# 30. Protected Live + open-stream band

This is the easy place for an implementation agent to reintroduce an O(N) scan.

Current behavior protects the flow region from the first blocking Live unit through the open stream.

Do not calculate its height by scanning that whole band on every append.

Add:

```rust
struct ProtectedBandIndex {
    blocker: ResidentOrdinal,
    stream: ResidentOrdinal,

    // blocker..stream, excluding stream
    unit_height_sum: usize,

    // Default-boundary gaps before units after the blocker.
    internal_default_gap_count: usize,
}
```

Do **not** store the predecessor gap before `blocker` in this aggregate because that gap may transition from semantic → frozen → native as prefix rows move into terminal history.

Calculate the blocker-predecessor gap dynamically.

Protected height:

```text
gap_before_blocker()
+
band.unit_height_sum
+
band.internal_default_gap_count * layout.gap
```

---

## 30.1 Initialize protected band

When an open stream is created/replaces the tail:

```text
find first live ordinal from flow.live
```

This is O(log L), not scanning static history.

If there is a Live blocker before the stream:

```text
initialize ProtectedBandIndex
```

It is acceptable to make **one event-driven range pass** to initialize the band.

It is not acceptable to repeat that pass for every stream append.

---

## 30.2 Height delta inside the protected band

When `record_unit_height()` changes:

```text
old_height → new_height
```

and ordinal satisfies:

```text
blocker <= ordinal < stream
```

then:

```rust
protected.unit_height_sum += new - old;
```

---

## 30.3 Blocker disappears

If the current blocker is:

```text
frozen
discarded
retired
```

find the next Live ordinal before the stream from the retained Live set.

Rebuild the protected band once.

This is event-driven and allowed.

---

# 31. REMED-3C — native transfer must update retained geometry

This cannot be deferred.

Current native History can partially transfer static rows, stream prefixes, spacing, and eventually pop the resident front. 

Every such mutation must leave `HistoryFlowIndex` correct.

---

## 31.1 Partial static transfer

If a 20-row static unit sends 7 rows to native:

```text
old effective resident height = 20
new effective resident height = 13
```

Update:

```text
known_unit_height_sum -= 7
layout.height = 13
```

Do not mark it dirty and then remeasure the original semantic View as 20 rows.

The native `FrozenStaticRemainder` becomes the source of the remaining height.

---

## 31.2 Complete front retirement

Before/while popping the front:

```text
subtract its known resident height
remove dirty state if any
remove ID → ordinal mapping
remove live set entry
remove stream set entry
clear open_stream if applicable
```

After front removal:

If the new front has `FlowBoundary::Default`, the gap before that new front was previously an **internal** gap.

It is now the leading/native-boundary gap.

Therefore:

```text
internal_default_gap_count -= 1
```

The leading gap is subsequently derived from native frontier state.

Then:

```text
first_ordinal += 1
```

Debug-assert the contiguous ordinal invariant.

---

## 31.3 Stream native release

When:

```rust
stream.release_resident_through(new_cursor)
```

changes the retained stream base/index:

```text
mark its History unit dirty
```

or update its exact effective height immediately if the required information is already available.

Preferred simple/correct path:

```text
mark dirty
next projection uses stream.prepare_from()
existing StreamLayoutCache retains the correct suffix
record exact new height
```

Do not leave the old History height in the aggregate.

---

## 31.4 Spacing transfers

Do not duplicate spacing ownership in two unsynchronized places.

Preferred:

```text
top padding and first leading gap remain derived from NativeFrontier
```

rather than stored again in the flow index.

Internal gap count only covers gaps wholly between resident units.

---

# 32. History stress tests

These are mandatory.

## H1 — 1,000 static + Live tail

```text
80×24
1000 static one-line units
1 Live tail
warm
```

Perform 100 Live changes.

After warm-up:

```text
static-prefix history_units_measured == 0
history_full_rebuild_units == 0
```

Resident touches must be bounded.

Use two assertions:

```text
touches_per_frame < 128
```

and:

```text
100k fixture touch count <= 1k fixture touch count + small constant
```

Do not rely only on wall-clock time.

---

## H2 — scale invariant

Fixtures:

```text
1,000 static + Live tail
10,000 static + Live tail
100,000 static + Live tail
```

Same viewport.

Same tail update.

Required:

```text
resident units touched does not materially depend on N
```

This test is the antidote to hidden planning walks.

---

## H3 — 100,000 static + open stream append, no render

Warm one stream at tail.

Call:

```rust
history.update_stream(handle, |stream| stream.push(chunk))
```

The mutation itself must not walk static history to find `handle`.

The new O(1) ID index is what proves this.

---

## H4 — 100,000 static + stream tail frame

After append and forced frame:

```text
stable static prefix measured = 0
resident touched bounded
stream suffix index refreshed
```

---

## H5 — NativeFrontier scaling

1k/10k/100k resident units.

Anchor at native frontier.

Same viewport.

Unit touches approximately identical.

---

## H6 — protected-band torture case

Construct:

```text
1000 static
1 Live blocker
1000 static
1 open stream
```

Warm.

Append to the stream 100 times.

Required:

```text
no 2000-unit band scan per append
protected band aggregate remains exact
```

Then freeze the Live blocker.

Ensure next Live landmark/rebuilt band is correct.

---

## H7 — resize

Warm 10,000-unit History at width 80.

Resize to 79.

Exactly one broad width rebuild is permitted.

Assert:

```text
history_full_rebuild_units >= 10,000 for rebuild frame
```

Then 100 tail updates:

```text
history_full_rebuild_units == 0
steady touch count bounded again
```

---

## H8 — native partial static transfer

Compare:

```text
before transfer total/overflow
after 1 row native
after several rows
after complete retirement
```

against physical output.

No height jumps or double-counted gaps.

---

## H9 — randomized differential projector

Before deleting the current full-scan projector, preserve it under:

```rust
#[cfg(test)]
```

as a **reference implementation**.

Run deterministic operation sequences:

```text
push static
push live
change live
freeze
discard live
push stream
append stream
seal stream
resize
native-transfer 1 row
native-transfer N rows
change HistoryLayout
```

For each state compare optimized vs reference:

```text
projected physical rows
overflow_rows
frozen physical overlay
native-anchor behavior
```

Use multiple random seeds, but keep them deterministic/reproducible.

Do not ship the reference projector in release builds.

---

# 33. Additional History hot-path audit

After implementation run a code search.

Production projection must contain no equivalent of:

```text
history.units().collect::<Vec<_>>()
Vec sized by history.units.len()
flow_items(history, all_plans)
plans.iter().all(...)
full units.iter().find(...)
full units.iter().position(...)
```

A full-unit loop is permitted only in named event-driven paths such as:

```text
full width rebuild
reference tests
one-time protected-band rebuild
debug invariant validation
```

Each such loop should have a comment saying why the O(N) operation is allowed.

Suggested commits:

```text
perf(tui): index resident History identity and dirty geometry
perf(tui): make History projection viewport bounded
fix(tui): retain exact History geometry across native transfer
```

---

# 34. REMED-4 — stream correctness and benchmark hardening

The underlying stream work is one of the better parts of the refactor.

`StreamRowIndex` already records separate:

```text
semantic_changed_from
visual_restart_from
hard_line_starts
stable anchors
```

and `reindex_in_place()` truncates/rebuilds only the damaged suffix. 

Do not replace it unless differential tests show a correctness bug.

The task is to **prove it against more than the easiest newline workload**.

---

# 35. Required stream fixtures

Current canonical chunk explicitly forces a newline at the end of each 256-byte append. 

Keep that as one fixture, not the only fixture.

## S1 — `newline_256`

```text
256-byte chunks
every chunk ends in newline
```

Expected:

```text
very small visual restart suffix
flat-ish next-chunk cost vs total transcript
```

---

## S2 — `prose_paragraph`

Generate deterministic natural prose where:

```text
chunk boundaries do NOT align with words
paragraphs are 4-16 KiB
newlines occur independently of append boundaries
```

This exercises ordinary word-wrap restart behavior.

Expected:

```text
cost depends on current affected hard-line/paragraph context
not total historical transcript size
```

---

## S3 — `long_hard_line`

No newline.

Run:

```text
64 KiB
128 KiB
512 KiB
```

This is deliberately pathological.

A safe hard-line restart may legitimately reflow the entire current hard line.

Do **not** falsify the benchmark to make this flat.

The requirement is:

```text
work proportional to the actually unstable hard line
NOT unrelated stable transcript before that line
```

Also benchmark:

```text
500 KiB stable prior lines
+ 8 KiB current hard line
```

versus:

```text
8 KiB total hard line
```

Those should be similar.

---

## S4 — Unicode boundaries

Include:

```text
é as composed
e + combining mark
emoji
emoji ZWJ sequences
CJK
multi-byte UTF-8 near append boundaries
```

Every individual JS string append must still be valid UTF-8/JS text.

Verify grapheme wrapping remains identical to full rebuild.

---

## S5 — Markdown/open semantic block

Include:

```text
open emphasis
open code fence
open list item
reference-definition prefix
paragraph that becomes a block only after later source
```

This validates semantic stability is not confused with row stability.

---

# 36. Stream differential property test

For each fixture and widths:

```text
1
2
7
20
80
121
```

after **every append**:

1. incrementally refresh existing `StreamRowIndex`;
2. construct a fresh full index from the same source;
3. compare:

```text
anchors
window output
physical rows
source coverage
stable-through semantics
```

Assert:

```rust
index.visual_restart_from >= index.indexed_from
index.visual_restart_from <= index.semantic_changed_from
```

except where an explicitly documented atomic semantic unit makes equality/ordering semantics different; if that occurs, encode the actual invariant instead of weakening the test blindly.

For ordinary text, visual restart should be at or before semantic damage because wrapping may require earlier context.

Also assert:

```text
rows before visual_restart are exactly retained/reused
```

---

# 37. Width-change stream test

Warm at width 80.

Resize to 79.

Expected:

```text
one full index build
```

Append 100 more chunks at width 79.

Expected:

```text
incremental suffix reindex resumes
no repeated full build
```

---

# 38. Native release stream test

Warm a long stream.

Release resident prefix through several anchors.

Append additional text.

Compare incremental index and physical window to fresh compile from the new semantic base.

This guards `retain_from()`.

---

# 39. Stream benchmark timings

Record separately:

### Source/model commit

```text
stream.push
model.refresh
no History projection
```

### Presentation/index

```text
append
History dirty refresh
StreamRowIndex update
bounded History selection
layout
paint
```

Do not report the first as if it represents the whole user-visible append.

The current benchmark separates these but uses only five default presentation iterations; replace that with authoritative sampling. 

### REMED-4 acceptance

- differential correctness passes every trace/width;
- newline case remains flat-ish;
- prose does not scale with old transcript;
- pathological hard line exposes its actual dependency instead of hiding it;
- native release + width change converge to full-build result.

Suggested commit:

```text
test(perf): validate stream restart and suffix reuse across text domains
```

---

# 40. REMED-5 — revalidate PERF-10 rather than assuming it was justified

The original handoff says:

```text
paint < 15% p95 dirty CPU → stop
paint >= 15% → proceed
```



The current gate is not adequate for reproducing that decision because the measured path already supplies `PaintCache`, and the non-component layout path isn't genuinely 80×24 bounded. 

---

# 41. One identical dirty-frame generator

Define something like:

```rust
enum PaintMode {
    Disabled,
    Enabled,
}
```

Then:

```rust
fn run_dirty_frame_sample(
    fixture: &Fixture,
    mode: PaintMode,
    ...
) -> Timing
```

Both modes must use:

```text
same View sequence
same LayoutCache behavior
same Theme
same 80×24 constraints
same warm-up count
same sample ordering
```

Only paint retention differs.

---

# 42. Pre-cache mode

`PaintMode::Disabled`:

```rust
ViewPainter.paint_tree(...)
```

No retained paint cache.

This is the value used for the original ≥15% decision.

---

# 43. Post-cache mode

`PaintMode::Enabled`:

```rust
PaintCache.begin_epoch(...)
ViewPainter.paint_tree_with_cache(...)
```

---

# 44. Use paired measurements

Every sample records:

```rust
struct DirtyFrameTiming {
    total_ns: u128,
    paint_ns: u128,
}
```

Sort by `total_ns`.

Let:

```text
i = nearest-rank 95th percentile index
```

The primary gate value is:

```text
paint_share_at_total_p95 =
    samples[i].paint_ns / samples[i].total_ns
```

Do not divide:

```text
independently selected p95(paint)
by
independently selected p95(total)
```

because those may represent different frames.

Also report the mean paint share of the slowest 5% of frames as supplemental information.

---

# 45. Required paint workloads

At minimum:

```text
text-heavy 2k
text-heavy 10k
column-heavy 2k
column-heavy 10k
styled-span-heavy 2k
styled-span-heavy 10k
```

but all constrained to an actual 80×24 terminal.

Add:

```text
viewport-scroll dirty frame
focus-move dirty frame
theme-change dirty frame
one-leaf-change shared path
```

The goal is user-visible dirty-frame behavior, not painting 10,000 off-screen rows.

---

# 46. Keep cache only if it earns its complexity

Decision:

### If corrected pre-cache paint share <15%

PERF-10 was not justified under the intended gate.

Remove production paint retention or leave the implementation excluded from normal production compilation pending future evidence.

Do not say:

```text
"we already wrote it, so we may as well keep it"
```

### If >=15%

Then require post-cache:

```text
correctness identical
p95 total dirty-frame improvement >=5%
memory increase acceptable
```

If post-cache does not improve end-to-end dirty-frame CPU, remove it even if paint itself gets cheaper.

---

# 47. Paint key overhead

Current `StyleContextKey` allocates owned vectors of owned strings for each key, and the key is constructed before the cache lookup. 

First make key construction conditional:

```rust
let can_cache = use_cache && node.paint_cacheable;

if can_cache {
    perf::inc(Counter::PaintCacheKeyBuilds);

    let key = PaintKey::new(...);

    if let Some(surface) = cache.surface(&key) {
        return surface;
    }

    ...
}
```

Do not build a large owned `PaintKey` for a node that cannot be cached.

---

## 47.1 Avoid clone-on-promotion

Current previous-generation hit does conceptually:

```rust
current.insert(key.clone(), surface.clone())
```

If retained paint survives the gate, use `remove_entry` or equivalent:

```rust
if let Some((owned_key, surface)) = previous.remove_entry(key) {
    current.insert(owned_key, Arc::clone(&surface));
    return Some(surface);
}
```

This avoids cloning all owned key strings on promotion.

---

## 47.2 Do not weaken the correctness key to optimize allocation

Never remove:

```text
allocated rect
content rect
clip rect
inherited physical style
resolved physical style
focus
focus-within
semantic style state/facts
theme invalidation
```

just to make the cache key cheaper.

If key allocation remains significant, then investigate making semantic style state/fact storage itself cheap to share.

Do not replace exact state with a hash-only key.

---

# 48. Paint differential correctness test

For each correctness case:

```text
render with cache disabled
render same sequence with cache enabled
compare every physical cell
```

Cases:

```text
theme switch
foreground/background inheritance
focused → unfocused
focus-within
style state change
local fact change
same View at different rectangle
clip change
row viewport scroll
component update
```

Physical equality must include:

```text
grapheme
continuation flags
foreground
background
attributes
physical completeness
```

---

# 49. Paint memory gate

Measure:

```text
maximum RSS
current cache entries
previous cache entries
surface-cell count retained
```

for long transient-tree sequences.

Two-generation retention must stay bounded.

Suggested commit:

```text
perf(tui): revalidate bounded paint retention against corrected dirty-frame gate
```

If removed:

```text
perf(tui): remove unjustified retained paint cache
```

---

# 50. REMED-6 — bridge lifetime/schema hardening

## 50.1 Native WeakView cache pruning

After deeper inspection, the native cache **does have periodic pruning**:

```text
if len > 4096
and len % 256 == 0
    retain only WeakViews that still upgrade
```



So do not rewrite it as if it were unbounded.

But existing coverage proves environment cleanup, not sustained entry pruning.

Add a perf/test-only API:

```text
tuiViewBridgeEntryCount()
```

for the current environment.

Stress:

```text
decode tens of thousands of transient fresh Views
replace retained host root repeatedly
advance cache/layout generations
allow old Rust Views to drop
```

Expected:

```text
entry count repeatedly collapses after prune thresholds
does not grow linearly with all-time decoded nodes
```

Retain the existing worker environment teardown test as well. Current runtime tests already exercise environment cleanup. 

---

# 51. Replace the build-script pseudo-JSON parser

Current `build.rs` finds quoted field names in the raw JSON string and scans digits following `:`. 

This is unnecessary.

Use a typed build-time schema:

```rust
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BridgeSchema {
    schema_version: u32,
    view_text: u32,
    ...
}
```

Parse:

```rust
let schema: BridgeSchema =
    serde_json::from_str(&source)
        .unwrap_or_else(...);
```

Serde's struct deserializer also gives proper missing/duplicate/type errors.

Generate constants from the typed object.

Do not manually parse JSON.

---

# 52. Bridge schema tests

Test:

```text
repository bridge-schema.json parses
generated Rust constants match TS schema
unknown field rejected
missing field rejected
duplicate field rejected
string where number required rejected
```

The build remains the ultimate compile-time guard.

---

# 53. REMED-7 — isolate the non-TUI PERF-11 work

The branch also landed:

```text
CoreEvent native queue
nextEvents(max)
pushMany native batching
related SDK/runtime API changes
```

despite the handoff marking PERF-11 as not applicable/deferred.

This work has different cancellation/backpressure semantics and should not hitchhike on the TUI performance merge.

## Preferred action

Split or revert these changes from `perf-refactor`.

Paths include at least:

```text
crates/iyon-native/src/core.rs
crates/iyon-native/src/model_turn.rs
packages/iyon-runtime/src/modules/core.ts
packages/iyon-runtime/src/native.ts
packages/iyon-sdk/src/core.ts
related core/facade tests
```

Make it a separate change.

---

# 54. If PERF-11 must remain, specify semantics before merge

The current new `pushMany()` behavior is approximately:

```text
validate/decode all input
lock ModelTurn
apply every event
take generated events
unlock
then asynchronously deliver generated events
```

The old behavior effectively did:

```text
apply one
await its event delivery/backpressure
apply next
...
```

These are observably different under cancellation/backpressure.

If the changes cannot be split, explicitly choose one contract.

The most internally coherent new batch contract is:

```text
Input validation:
    atomic — if any input is invalid, mutate nothing

Model mutation:
    atomic with respect to the supplied batch

Delivery:
    ordered
    may be interrupted by session/turn cancellation
    cancellation does not roll back already-committed batch model state
```

Document that if adopted.

Do not claim this preserves old per-item cancellation semantics.

---

# 55. PERF-11 tests if retained

Use event queue capacity 1.

Cases:

### Invalid middle value

```text
[A, invalid, C]
```

Expected:

```text
error
zero model mutation
zero resulting events
```

because input parsing occurs first.

### Cancellation during blocked delivery

```text
batch A/B/C committed
queue accepts first output
remaining output blocked
cancel
```

Assert exact chosen contract:

```text
model contains A/B/C
only allowed output subset delivered
terminal cancellation semantics correct
```

### Session close

Same under session close.

### Ordering

`nextEvents(max)` must preserve exact ordering relative to repeated `nextEvent()`.

### Boundaries

```text
max=0 reject
max=1
max=256
max=257 reject
```

---

# 56. REMED-8 — final validation

Do not merge after “the tests are green.”

Run four separate gates.

---

# 57. Gate A — semantic correctness

Required:

```text
cargo test -p iyon-tui
cargo test -p iyon-native
affected Bun runtime tests
full TUI semantic/runtime tests
API surface scanner
TypeScript typecheck
```

Include:

```text
new NodeId tests
History differential projector
stream differential row-index tests
paint cache differential tests if paint retained
native transfer tests
native driver/in-flight presentation tests
```

---

# 58. Gate B — complexity correctness

This gate is primarily counter-based.

For:

```text
1k
10k
100k
```

resident histories, same stable viewport/tail operation:

Required:

```text
HistoryResidentUnitsTouched approximately constant
HistoryUnitsMeasured stable prefix = 0
HistoryFullRebuildUnits = 0
```

For one resize:

```text
HistoryFullRebuildUnits = N exactly/approximately as expected
```

Afterwards:

```text
returns to constant steady-state work
```

For 100 mutations without driver:

```text
HostFramesPrepared = 0
```

Then one forced frame:

```text
HostFramesPrepared = 1
```

This gate cannot be substituted with wall-clock numbers.

---

# 59. Gate C — performance

Use authoritative benchmark mode.

Minimum:

```text
200 measured post-warmup samples
raw samples saved
exact SHA
same machine/session where possible
same release profile
same terminal size
```

Compare:

```text
pre_remediation
final
```

Primary metrics:

```text
NAPI commit p50/p95
forced dirty frame p50/p95
History tail update 1k/10k/100k
stream append source/model
stream append+presentation
paint-disabled frame
paint-enabled frame if retained
RSS
```

Do not gate on p99 unless sufficient samples were collected.

---

# 60. Gate D — long-session test

Run a synthetic session long enough to exercise retention:

```text
100k historical units
thousands of stream appends
many transient View identities
component animation/ticks
native History retirement
resizes
theme changes
```

Record periodically:

```text
RSS
View bridge entry count
layout-cache generations
paint-cache entries if retained
resident History count
StreamRowIndex anchor count
frame p95
mutation p95
```

The important shape is:

```text
memory bounded by retained working set
not total all-time identities/frames
```

---

# 61. Absolute merge-blocker checklist

The branch is **not merge-ready** unless every item below is true.

### Semantic identity

- [ ] decorated `wrap/noWrap/textAlign` produces fresh child NodeId;
- [ ] unchanged descendants retain NodeIds;
- [ ] native cached-old-child regression renders changed output;
- [ ] Rust `ViewFlags` cannot become stale after internal semantic mutation.

### Native update scheduling

- [ ] ordinary mutations do not call frame preparation;
- [ ] stream append ends in dirty-and-return;
- [ ] `screenRows()` does not secretly flush;
- [ ] explicit `advance(0)`/flush provides deterministic test frame;
- [ ] in-flight terminal presentation cannot cause a dirty frame to be lost;
- [ ] realtime nextEvent driver still advances animation/streams.

### History

- [ ] `HistoryUnitId` lookup is indexed, not linear;
- [ ] `next_stream_wakeup()` does not scan static History;
- [ ] `advance_streams()` does not scan static History;
- [ ] production projection does not collect every resident unit;
- [ ] no N-sized `plans`, `selected_units`, or `flow_items`;
- [ ] total resident geometry is an exact retained aggregate;
- [ ] dirty units alone are remeasured;
- [ ] FollowEnd selection is backward viewport-bounded;
- [ ] NativeFrontier selection is forward viewport-bounded;
- [ ] all off-screen Live components remain mounted;
- [ ] protected Live→stream band is retained, not scanned per append;
- [ ] front retirement updates the index;
- [ ] partial native static transfer updates effective height;
- [ ] stream native release invalidates/updates exact height;
- [ ] resize incurs one broad rebuild then converges.

### Streaming

- [ ] newline benchmark;
- [ ] prose benchmark;
- [ ] pathological hard-line benchmark;
- [ ] Unicode differential tests;
- [ ] width-change full-build-once test;
- [ ] native release differential test;
- [ ] incremental rows match clean full compile.

### Benchmark integrity

- [ ] no authoritative five-sample p95;
- [ ] explicit warm-up;
- [ ] raw sample distributions;
- [ ] implementation label is real;
- [ ] COLD and REBUILT are distinct;
- [ ] commit and frame latency separated;
- [ ] every supposed 80×24 workload is genuinely bounded;
- [ ] counters instrument real hidden work;
- [ ] no dead always-zero counter is used as evidence.

### Paint

- [ ] corrected cache-disabled pre-PERF10 measurement;
- [ ] paired p95 paint share;
- [ ] actual 80×24 frames;
- [ ] retain cache only if ≥15% gate is actually satisfied;
- [ ] post-cache end-to-end improvement measured;
- [ ] physical output differential tests pass;
- [ ] retention/RSS bounded;
- [ ] key allocation cost measured.

### Scope

- [ ] PERF-11 work split out, **or**
- [ ] its changed cancellation/backpressure contract is explicit and fully tested.

---

# 62. Required completion report from each implementation agent

Do not accept an agent message saying:

> “Implemented and tests pass.”

Require this template:

```text
TRANCHE:
COMMIT SHA:

FILES CHANGED:
- ...

INVARIANTS IMPLEMENTED:
- ...

OLD HOT PATH:
- exact algorithmic complexity
- exact full-scan/allocation sites removed

NEW HOT PATH:
- exact algorithmic complexity
- retained state used
- invalidation mechanism

TESTS ADDED:
- test name
- what shortcut it prevents

COMMANDS RUN:
- exact command
- exit status

COUNTERS BEFORE:
{ ... }

COUNTERS AFTER:
{ ... }

AUTHORITATIVE BENCHMARK:
- warmups:
- samples:
- git SHA:
- p50:
- p95:
- raw sample artifact:

CODE-SEARCH CHECKS:
- banned old patterns absent/present with justification

KNOWN LIMITATIONS:
- ...

WHY THIS TRANCHE SATISFIES ITS ACCEPTANCE GATE:
- ...
```

If they cannot fill in the “old hot path” and “new hot path” sections precisely, they probably do not understand what they changed.

---

# 63. Specific shortcut patterns reviewers should reject

These should cause an immediate review failure:

```rust
// "History optimized"
let units = history.units().collect::<Vec<_>>();
```

```rust
// "bounded selection"
let selected = vec![None; history.units.len()];
```

```rust
// hidden complete planning pass
let plans = history.units().map(make_plan).collect::<Vec<_>>();
select_last_12(&plans);
```

```rust
// stream handle lookup
history.units.iter().position(|u| u.id == handle.id)
```

```rust
// mutation API
inner.running.invalidate_frame();
inner.advance_and_render()?;
```

```ts
// changed semantics, inherited identity
const changed = { ...oldNode, wrap: newMode };
```

```ts
// benchmark
case "COLD":
  render(tree(n));
case "REBUILT_EQUIVALENT":
  render(tree(n));
```

```rust
// "80x24 paint test"
LayoutConstraints::width_only(80)
```

```text
5 samples
→ compute p95
→ make architecture decision
```

```text
newline after every streaming chunk
→ declare arbitrary text streaming flat
```

```text
paint cache enabled
→ call result "after_perf9 pre-cache"
```

Those are exactly the classes of shortcut this second pass must prevent.

---

# 64. Expected outcome

After this work, the refactor should preserve the improvements that are already genuine:

- persistent Rust `View` sharing;
- component snapshot retention;
- layout-cache reuse;
- direct retained NodeId→WeakView decoding;
- generic incremental `TextStream`;
- safe semantic/visual stream restart separation;
- synchronous TS API where the underlying mutation itself is synchronous.

But it will also remove the current holes:

```text
stale nested TS semantic identity
full History planning despite bounded selection counters
linear HistoryUnitId lookup
linear stream wakeup/advance discovery
mutation-triggered synchronous frames
possible dropped frame behind in-flight presentation
weak benchmark statistics
misdefined COLD/REBUILT workloads
newline-only stream evidence
incorrect/unreproducible paint gate
unreviewed non-TUI batch semantics
```

The key standard for the final branch is no longer:

> “The TUI feels much faster.”

It becomes:

```text
The TUI feels faster
AND
the semantic outputs match reference behavior
AND
changed identity is provably correct
AND
steady work counters are independent of retained-history length
AND
mutations coalesce behind the frame driver
AND
stream work is proportional to the actual damaged visual suffix
AND
the benchmark harness measures the operation it claims to measure
AND
every retained cache has a demonstrable lifetime bound
AND
every performance tranche has a reproducible acceptance result.
```

That is the point at which I would consider the performance refactor properly finished rather than merely successful-looking.
