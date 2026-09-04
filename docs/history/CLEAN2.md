# PRE-V5-R0 — Architecture Rot Eradication

**Status:** Mandatory cleanup gate after PERF-13 and before the V5 architecture census  
**Repository:** finished post-PERF-13 `iyon-tui`  
**Objective:** remove architectural residue from previous generations until every semantic operation has one authoritative production path.

---

# 0. Mission

Do not begin the V5 architecture census.

Do not begin V5 implementation.

First clean the finished PERF-13 repository.

The repository has accumulated successive generations of architecture:

```text
legacy native transport
    ↓
packed transport
    ↓
direct/retained transport
    ↓
API-H3 separation
    ↓
PERF-13 three-plane runtime
```

Previous migrations have left old paths behind.

That ends here.

This tranche removes:

```text
fallback transports
cold full-object transports
legacy transports
compatibility transports
duplicate materializers
duplicate decoders
duplicate update paths
old recovery implementations
unmigrated peripheral call sites
obsolete native exports
obsolete bridge schemas
obsolete test helpers
obsolete benchmark routes
obsolete counters
obsolete migration scaffolding
```

The result must be a repository in which the architecture we believe exists is the architecture that actually executes.

---

# 1. Non-negotiable rule

For one semantic operation there is one production implementation.

Wrong:

```text
operation
    ├── retained path
    ├── cold path
    ├── compatibility path
    └── fallback path
```

Target:

```text
operation
    ↓
authoritative PERF-13 path
```

Failure means failure.

A broken authoritative path must break tests and application execution loudly.

No previous-generation implementation is allowed to make the operation succeed.

---

# 2. No preservation decisions

You are not authorized to preserve old architecture.

When you encounter:

```text
legacy
compatibility
cold decode
fallback
recovery through old transport
old addon route
parallel materializer
parallel decoder
old bridge representation
old setter
old renderer
```

the disposition is:

```text
DELETE
```

A dependency on that path does not change the disposition.

It means:

```text
consumer is unmigrated
    ↓
migrate consumer
    ↓
delete old path
```

When migration cannot be completed safely from the current evidence:

```text
STOP
    ↓
record exact blocker
    ↓
do not preserve old architecture as a solution
```

The blocker is for the architecture owner to resolve.

---

# 3. No compatibility architecture

Delete compatibility execution paths.

Version mismatch must fail explicitly.

Wrong:

```text
new ABI missing
    ↓
use old transport
```

Target:

```text
new ABI missing
    ↓
explicit incompatible-runtime error
```

Delete:

```text
older-addon fallback
legacy addon decode
compatibility renderer
compatibility materializer
compatibility transport
```

Mixed generations of TypeScript and native runtime do not receive a second architecture.

---

# 4. No semantic fallback

Delete all control flow shaped like:

```text
try new architecture
    ↓ failure/refusal
old architecture
```

Examples:

```text
retained publication fails
    → full View decode

NativeRef recovery fails
    → JSON bridge materialization

generated ABI unavailable
    → host.render(oldObject)

new state update unavailable
    → rebuild whole View

content path unavailable
    → structural replacement
```

The replacement behavior is:

```text
new architecture succeeds
```

or:

```text
new architecture fails explicitly
```

Nothing else.

---

# 5. Cold start is not a second architecture

A first materialization still exists.

It must use the same authoritative architecture as subsequent materialization.

Correct:

```text
no retained NativeRef
    ↓
authoritative retained materializer
    ↓
new NativeRef
```

Incorrect:

```text
no retained NativeRef
    ↓
serialize complete View
    ↓
legacy N-API decoder
```

Delete the second form.

Do not keep an old architecture and rename it “cold”.

---

# 6. Stale references stay inside the current architecture

A stale retained reference is a cache condition.

Handle it as:

```text
cached NativeRef
    ↓ stale
invalidate cached ref
    ↓
authoritative rematerialization
```

Delete:

```text
stale NativeRef
    ↓
lower complete semantic object
    ↓
legacy decoder
```

Staleness does not authorize architecture switching.

---

# 7. Known first target: `cold-lowering.ts`

Start by re-auditing:

```text
packages/iyon-tui/src/transport/structural/cold-lowering.ts
```

Trace every reference to:

```text
lowerColdView
lowerSemanticView
BridgeViewNode
complete semantic View lowering
full-object native View decode
```

Classify every call site by operation.

Then remove the production dependency graph.

Expected work includes migration of consumers that currently depend on complete lowering.

Inspect at minimum:

```text
runtime
retained-dag
native-view-abi
ViewSlot
ScrollPane
History
animations
controls
tests
benchmarks
fixtures
native addon exports
Rust decode code
generated declarations
```

`cold-lowering.ts` is not considered cleaned up when the file disappears but equivalent lowering survives elsewhere.

Delete the architecture, not the filename.

---

# 8. Hunt for every previous-generation execution path

Search the complete repository for:

```text
cold
fallback
legacy
compat
compatibility
recover
recovery
old
slow
direct
decode
full
bridge
packed
packet
v2
v3
stale
materialize
rebuild
retry
ordinary
```

Then inspect actual control flow.

Names are not authoritative.

Search for patterns such as:

```ts
try {
    currentPath()
} catch {
    previousPath()
}
```

```ts
const result = currentPath()

if (result === undefined) {
    previousPath()
}
```

```rust
match current_path() {
    Ok(value) => value,
    Err(_) => previous_path(),
}
```

```text
current ABI absent
    → old N-API operation
```

Every such architecture switch is a cleanup target.

---

# 9. Produce a temporary route table

Before changing each area, enumerate all current ways to perform the operation.

Required operations:

```text
root publication
root replacement

structural node materialization
structural node reuse
stale-reference recovery

ViewSlot creation
ViewSlot update
ViewSlot animation
ViewSlot reset

ScrollPane creation
ScrollPane update

History push
History update
History freeze

state mutation
layout-state mutation
presentation-state mutation

content attachment
content replacement
Source append

frame publication
```

Use:

| Operation | Current production paths | Authoritative PERF-13 path | Paths to delete | Consumers to migrate |
|---|---|---|---|---|

The target for every row is:

```text
Current production paths = 1
```

---

# 10. Root publication

There must be one root publication implementation.

Delete production routes based on:

```text
full semantic View lowering
generic bridge object trees
old host.render(object)
old N-API View decoding
compatibility rendering
alternate complete materialization
```

The retained PERF-13 structural publication path becomes mandatory.

The root path must not contain:

```text
prepareDesiredInstall(...)
    ↓ returns undefined
prepareColdInstall(...)
```

where `prepareColdInstall` executes an older architecture.

Replace refusal with either:

```text
authoritative rematerialization
```

or:

```text
explicit failure
```

inside the retained architecture.

---

# 11. Retained materialization

There must be one algorithm capable of turning authoritative semantic retained state into native retained state.

Remove:

```text
retained materializer A
full decoder B
fallback materializer C
compatibility materializer D
```

until one implementation remains.

Cache presence changes work performed.

It does not change architecture.

---

# 12. ViewSlot

Audit the entire ViewSlot lifecycle:

```text
create
seed
setView
animation start
animation frame
animation stop
replacement
reset
destroy
```

Every structural View operation must use the authoritative structural path.

Delete ViewSlot APIs that accept complete bridge Views for native decoding.

Do not wrap the old API.

Replace it.

---

# 13. ScrollPane

Audit:

```text
create
seed
setContent
replace content
scroll state updates
destroy
```

Structural/content semantics must use the corresponding PERF-13 plane.

Delete complete View decode from ScrollPane.

A content update must not reconstruct structural state.

---

# 14. History

Audit every History operation:

```text
push
freeze
replace
update
stream attachment
component insertion
component replacement
removal
```

Remove old complete View transport.

History does not receive its own structural architecture.

History operations use the same authoritative retained mechanisms as the rest of the runtime.

Do not preserve old History transport because History itself is scheduled for later V5 replacement.

V5 must receive a clean History implementation, not one carrying obsolete transport underneath it.

---

# 15. State plane

There is one state mutation architecture.

Delete:

```text
whole-View replacement for state changes
whole-style transport
old state object setters
old native state reconstruction
state fallback through structure
```

PERF-13 state semantics remain state semantics.

Examples:

```text
background change
    → state delta

gap change
    → state delta

focus-derived presentation change
    → state/native runtime
```

Never:

```text
state change
    → rebuild View
```

---

# 16. Content plane

There is one content mutation architecture.

Enumerate every way text/content changes today.

Delete parallel forms such as:

```text
Source append
old direct stream setter
old full text replacement path
History-specific append path
View reconstruction containing new text
full accumulated Markdown replacement through structure
```

where they perform equivalent content semantics.

The finished PERF-13 content plane becomes authoritative.

---

# 17. Plane purity

After cleanup:

```text
structure change
    → structural plane

state change
    → state plane

content change
    → content plane
```

Delete every convenience method that routes one semantic plane through another.

Search particularly for:

```text
text change → View rebuild
style change → structural publish
layout state → full structural decode
Source append → composition
History update → generic render(object)
```

The cleanup is not complete while these remain.

---

# 18. No refusal escape hatch

Search the current architecture for results such as:

```text
undefined
false
Refused
Unsupported
TooLarge
BudgetExceeded
RetryCold
Fallback
```

when returned by an authoritative path.

Trace the caller.

Delete logic where those values select an older architecture.

A current architecture must cover its own supported input domain.

A performance heuristic does not authorize previous-generation execution.

Wrong:

```text
retained path considers operation expensive
    ↓
full legacy reconstruction
```

Target:

```text
retained path performs more work
```

still using retained semantics.

---

# 19. Delete duplicate recovery implementations

Recovery does not get a separate semantic architecture.

Allowed recovery mechanisms are operations internal to the current architecture:

```text
invalidate cache
retry current operation
rematerialize with current materializer
rebuild current derived cache
```

Delete recovery based on:

```text
old transport
old decoder
old View lowering
old packed protocol
old object representation
```

---

# 20. Remove unsupported old native interfaces

After migrating TypeScript consumers, remove corresponding native entrypoints.

Inspect:

```text
N-API exports
Rust addon methods
TypeScript addon contracts
generated declarations
bridge structs
serde decode
object decoders
resource conversion
legacy native handles
```

No dead old architecture remains available merely because nothing currently calls it.

Delete the entire support graph.

---

# 21. Remove old bridge types

Trace types supporting obsolete execution paths.

Examples:

```text
BridgeViewNode
old complete View contracts
packed records
legacy structural objects
compatibility state objects
old decode-specific resource wrappers
```

Delete types whose only purpose is feeding deleted paths.

Then delete:

```text
normalizers
lowerers
validators
decoders
tests
fixtures
counters
docs
```

that exist solely for those types.

---

# 22. Remove old cache infrastructure

When deleting an execution path, find every cache supporting it.

Delete it.

Example:

```text
old semantic→bridge lowering
    ↓
old bridge cache
```

Both disappear.

Do not retain caches for nonexistent architectures.

Do not maintain two representations of equivalent authoritative data.

---

# 23. Initial creation and later update must agree

For every subsystem compare:

```text
create
first materialization
normal update
replacement
reset
recovery
destroy
```

They must belong to one architectural model.

This is not complete:

```text
create   → legacy decode
update   → retained
recovery → legacy decode
```

Migration means the lifecycle is migrated, not one method.

---

# 24. Runtime route observability during cleanup

Before deleting alternate routes, instrument enough to prove they are exercised or absent.

Track the current distinct paths.

Example temporary counters:

```text
retained publication
full-object decode
legacy render
fallback
compatibility
stale rematerialization
```

Use them to prove cleanup.

Then delete counters for deleted paths.

Do not leave permanent architecture vocabulary describing implementations that no longer exist.

---

# 25. Benchmarks must assert architecture

Every PERF benchmark must prove that it executed the intended route.

Required pattern:

```text
reset counters

run benchmark

assert authoritative route count matches expectation
assert legacy route count == 0
assert fallback route count == 0
assert compatibility route count == 0
assert full-decode route count == 0
```

A benchmark that reaches correct output through the wrong route fails.

A benchmark that cannot identify its route is incomplete.

---

# 26. Tests must expose architecture failure

For tests intended to validate PERF-13 mechanisms, final pixels/output alone are insufficient.

Tests must fail when the authoritative mechanism is broken.

Required invariant:

```text
authoritative path broken
    ↓
test failure
```

Forbidden:

```text
authoritative path broken
    ↓
alternate implementation succeeds
    ↓
test passes
```

Add route assertions where necessary.

---

# 27. Do not retain old implementations as production oracles

Correctness oracles belong in explicit test tooling.

Move any required differential implementation out of production reach.

Production code cannot import or invoke it.

It cannot be selected dynamically.

It cannot be used for recovery.

It cannot be packaged as a runtime fallback.

After relevant differential validation is complete, delete the oracle too.

---

# 28. No migration scaffolding without an active migration

Search for old:

```text
feature flags
route selectors
environment toggles
compat switches
dual-path configuration
fallback knobs
debug switches selecting old engines/transports
```

Delete them.

PERF-13 is finished.

Its previous migration scaffolding is no longer architecture.

---

# 29. Comments do not preserve code

Comments such as:

```text
for older addon
compatibility
safe fallback
recovery path
temporary
cold route
legacy support
```

carry zero architectural authority.

Verify the code.

Delete stale comments together with stale implementations.

Do not interpret a comment as permission to preserve a second architecture.

---

# 30. Tests do not preserve code

A test depending on an obsolete implementation is not justification for retaining that implementation.

Classify tests:

```text
semantic contract test
architecture-path test
obsolete implementation test
```

Delete obsolete implementation tests.

Rewrite semantic tests against the authoritative implementation.

---

# 31. Benchmarks do not preserve code

A benchmark referencing old architecture does not preserve that architecture.

Delete benchmarks for obsolete implementations after their historical purpose is complete.

Retain benchmark results/reports where useful.

The executable old architecture itself does not survive merely to reproduce history.

---

# 32. Public exports do not preserve code

An exported API from a previous architecture is not a compatibility commitment.

This project intentionally removes old architecture rather than carrying deprecation layers.

Delete obsolete exports and migrate in-repository consumers.

Do not add aliases.

Do not add deprecated wrappers.

Do not add adapters.

Do not add shims.

---

# 33. No wrappers over old architecture

Forbidden:

```text
new API
    ↓
adapter
    ↓
old implementation
```

Forbidden:

```text
authoritativeRetainedMaterialize()
    ↓
lowerColdView()
```

Forbidden:

```text
new state API
    ↓
old View setter
```

Forbidden:

```text
new content API
    ↓
old History text mutation
```

Replacement means replacement.

---

# 34. Recursive deletion

For every deleted path, continue following dependencies downward until the whole obsolete support graph disappears.

Example:

```text
cold-lowering.ts
    ↓
BridgeViewNode
    ↓
native bridge decoder
    ↓
N-API method
    ↓
Rust conversion types
    ↓
counters
    ↓
tests
    ↓
bench helpers
```

Delete all of it after consumers migrate.

Do not leave architectural fossils.

---

# 35. Detect never-migrated subsystems

Explicitly search peripheral systems for previous-generation behavior:

```text
ViewSlot
ScrollPane
History
animation
error UI
empty-state rendering
loading UI
controls
test utilities
benchmark utilities
debug tools
initial seed operations
reset operations
recovery paths
```

Central-path migration does not count as repository migration.

Every production subsystem must use the current architecture.

---

# 36. Detect architecture inconsistencies

During the work maintain:

```text
ROT-001
ROT-002
ROT-003
...
```

Create an entry whenever two parts of the repository disagree about how the same semantic operation works.

Examples:

```text
root uses generated retained ABI
ViewSlot uses complete N-API View decode
```

```text
content append uses Source
History append rebuilds View
```

```text
normal materialization is retained
stale recovery uses old bridge
```

```text
spec says state plane
control routes change through structural render
```

Every entry must end as:

```text
FIXED
```

The tranche cannot close with an inconsistency classified as “accepted compatibility”.

---

# 37. Block instead of preserve

When deleting an old path reveals a dependency that cannot immediately migrate:

Do not restore the old path.

Do not create an adapter.

Do not create a compatibility layer.

Do not weaken the cleanup rule.

Record:

```text
BLOCKER R0-B###
```

with:

```text
exact consumer
exact obsolete dependency
why migration cannot be completed
what information/decision is missing
```

Stop that cleanup chain there.

Continue independent cleanup work.

The architecture owner resolves the blocker.

This preserves uncertainty as visible uncertainty instead of embedding it permanently into code.

---

# 38. Required cleanup tranches

Execute as stacked changes.

## R0.1 — Route discovery and temporary observability

Identify every production implementation for every semantic operation.

Add temporary counters/assertions necessary to prove route selection.

**Gate:**

```text
all duplicate structural/state/content execution routes are known
```

---

## R0.2 — Remove automatic fallback

Delete every automatic switch from the current architecture into an older architecture.

Do this before migrating all remaining direct old callers.

Old callers remain explicitly old for this short tranche.

The current path no longer hides its own failure.

**Gate:**

```text
authoritative path failure is visible
```

---

## R0.3 — Migrate structural consumers

Migrate all production callers still using obsolete structural transport:

```text
root
ViewSlot
ScrollPane
History
animations
controls
seed/reset/recovery paths
```

**Gate:**

```text
one production structural publication architecture
```

---

## R0.4 — Delete obsolete structural architecture

Delete:

```text
cold lowering
complete bridge lowering
legacy View decode
compatibility materializers
old host.render structural path
old bridge contracts
old structural native exports
related support graph
```

**Gate:**

```text
old structural architecture cannot execute in production
```

---

## R0.5 — State residue

Delete all alternative state mutation architectures.

Migrate whole-object/full-View state mutations to state deltas.

**Gate:**

```text
one production state mutation architecture
```

---

## R0.6 — Content residue

Delete duplicate content mutation paths.

Migrate content changes to the PERF-13 content system.

**Gate:**

```text
one production content mutation architecture
```

---

## R0.7 — Native/API cleanup

Remove obsolete native exports, bridge structs, generated declarations, wrappers, shims, feature flags, and compatibility code.

**Gate:**

```text
no old execution architecture remains callable
```

---

## R0.8 — Test and benchmark hardening

Make modern-path tests and benchmarks assert route integrity.

Delete obsolete architecture tests and benchmarks.

**Gate:**

```text
breaking the authoritative route makes relevant tests/benchmarks fail
```

---

## R0.9 — Recursive dead-code eradication

Repository-wide reference search.

Delete stranded support machinery and vocabulary.

**Gate:**

```text
no unexplained legacy/cold/fallback/compat execution concept remains
```

---

## R0.10 — Final route audit

Rebuild the operation table.

Every production semantic operation must show exactly one implementation.

**Gate:**

```text
STRUCTURE = one authoritative path
STATE     = one authoritative path
CONTENT   = one authoritative path
```

---

# 39. Mandatory validation scenarios

Prove at minimum:

### Structural first materialization

```text
no cached ref
    ↓
authoritative retained materialization
```

### Structural reuse

```text
existing valid ref
    ↓
retained reuse
```

### Stale ref

```text
stale ref
    ↓
invalidate
    ↓
authoritative retained rematerialization
```

### Invalid structural data

```text
validation failure
    ↓
explicit failure
```

No fallback.

### ViewSlot

Create/update/animation/reset all use authoritative route.

### ScrollPane

Create/update all use authoritative route.

### History

Relevant structural/content operations use the correct PERF-13 planes.

### State

State mutation produces no structural fallback.

### Content

Source append produces:

```text
React/composition = 0
structure         = 0
state             = 0
content           = payload
```

### Native ABI mismatch

Fails explicitly.

### Benchmark route

Wrong route causes benchmark failure.

---

# 40. Required repository-wide searches at completion

Search production source for all vocabulary used by removed architectures.

At minimum:

```text
cold
fallback
legacy
compat
compatibility
packed
old bridge
full decode
lowerColdView
lowerSemanticView
BridgeViewNode
```

Every remaining occurrence must be inspected.

Historical documentation is allowed to describe history.

Production code must not contain unexplained execution residue.

---

# 41. Required final report

Produce:

```text
POST-PERF13-ROT-CLEANUP.md
```

This is not the architecture census.

Required contents:

## 0. Result

State whether cleanup passed.

## 1. Rot discovered

| ID | Area | Conflicting paths | Result |
|---|---|---|---|

Every result is:

```text
FIXED
```

or:

```text
BLOCKED
```

No “retained for compatibility”.

## 2. Fallbacks deleted

List all automatic architecture fallbacks removed.

## 3. Unmigrated consumers migrated

List exact subsystems/call sites.

## 4. Architecture deleted

Exact files/modules/types/native exports.

Include approximate production LOC removed.

## 5. Authoritative route table

One row per semantic operation.

Every production operation has one path.

## 6. Tests

Show route-integrity guarantees.

## 7. Benchmarks

Show that intended paths are asserted.

## 8. Temporary observability removed

List counters/debug machinery removed after proving cleanup.

## 9. Blockers

Exact unresolved cleanup blockers.

Any blocker prevents the architecture census.

## 10. Final verification

Commands and results.

---

# 42. Exit criteria

The architecture census does not begin until all of these are true:

1. Root structural publication has one production implementation.
2. Structural node materialization has one implementation.
3. Stale-reference recovery remains inside that implementation.
4. No structural fallback switches to previous-generation decode.
5. ViewSlot uses the authoritative structural architecture.
6. ScrollPane uses the authoritative architecture.
7. History uses PERF-13 plane semantics rather than old full-View transport.
8. Animation/update/reset paths use the same architecture as normal updates.
9. Unsupported native versions fail explicitly.
10. Complete old semantic-View bridge lowering is gone from production.
11. Old full-object native structural decode is gone.
12. State mutations use one state architecture.
13. Content mutations use one content architecture.
14. State/content changes never escape through structural reconstruction.
15. Old native exports are removed.
16. Old TS contracts are removed.
17. Old Rust decoder/support code is removed.
18. Old bridge schemas are removed.
19. Old compatibility wrappers are removed.
20. Old feature switches are removed.
21. Old fallback counters are removed after validation.
22. Tests assert architectural routes where architecture matters.
23. Benchmarks assert architectural routes.
24. Breaking the retained path produces failure.
25. No old path can make that failure disappear.
26. Every `ROT-*` item is `FIXED`.
27. There are no cleanup blockers.

Only then:

```text
PERF-13
    ↓
ROT-FREE PERF-13 BASELINE
    ↓
ARCHITECTURE CENSUS
    ↓
V5 MIGRATION DESIGN
```

---

# 43. Final rule

Do not ask:

> “Is there a reason to keep this old path?”

Ask:

> “What still depends on this old path, and how do I migrate that dependency so the old path can be deleted?”

Do not turn uncertainty into compatibility code.

Do not turn failure into fallback.

Do not turn previous architecture into recovery.

Do not leave two ways to do the same thing.

At the end of R0, the repository must be boring in the best possible way:

```text
one structural architecture
one state architecture
one content architecture

one semantic operation
    ↓
one authoritative route
```

Then we can finally inspect what PERF-13 actually is and design V5 against something real.
