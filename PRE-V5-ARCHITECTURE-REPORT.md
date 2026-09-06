# Handoff — Post-PERF-13 Repository Architecture Census Before Iyon UI V5

## 0. Mission

PERF-13 has just reached its stable reference implementation.

Before beginning the `iyon-ui` V5 migration, perform a **complete architecture census of the current repository**.

This is **not** a refactor task.

This is **not** an implementation task.

This is **not** an invitation to immediately map old classes onto new classes.

Your job is to determine, from the actual source tree, exactly what exists after PERF-13:

- what each subsystem owns;
- which concepts overlap;
- which abstractions are public authoring surfaces versus internal runtime machinery;
- which runtime responsibilities are genuinely necessary;
- which responsibilities only exist because of the current View/composition/layout architecture;
- which existing content/stream/projection machinery V5 should adapt rather than reinvent;
- what depends on what;
- what can disappear wholesale;
- what must survive;
- what must be rewritten because its useful responsibility is entangled with obsolete architecture;
- what hidden coupling could make an apparently simple deletion dangerous;
- what migration ordering constraints follow from the real code.

The result must be detailed enough that a later architecture handoff can design the V5 migration from **evidence**, not assumptions.

The main danger we are avoiding is this:

```text
old abstraction
    ↓
make it private
    ↓
adapt React into old abstraction
    ↓
adapt old abstraction into Taffy/new runtime
```

when the correct answer may instead be:

```text
React host occurrence
    ↓
new retained runtime representation
    ↓
Taffy / content / backend

old abstraction: deleted
```

Do **not** assume an old type must survive merely because some responsibility currently passes through it.

---

# 1. Required source material

Read in full before drawing conclusions:

1. The final PERF-13 handoff/specification.
2. The actual finished PERF-13 implementation.
3. `IYON-UI-PRELIMINARY-DESIGN-v5.md`.
4. Any implementation records / tranche completion records for PERF-13.
5. Relevant API-H1/API-H2/API-H3 architecture handoffs if they explain current package boundaries.
6. Repository-level architecture docs, `AGENTS.md`, package READMEs, crate docs, generated-schema docs, benchmark docs, and migration records that describe why current seams exist.

The source code is authoritative when documentation and implementation disagree.

Record disagreements explicitly.

Do not silently reconcile them.

---

# 2. Core rule: inventory first, V5 mapping second

For every subsystem, first answer:

```text
WHAT EXISTS NOW?
```

Only after that answer:

```text
WHAT DOES V5 PROBABLY DO WITH IT?
```

Do not begin from V5 nouns and search for similarly named current nouns.

For example, do not assume:

```text
current View == future occurrence
current Projector == future Funnel
current StreamPane == future Connector
current History == future ScrollSurface
current component registry == React HostConfig equivalent
```

Those mappings may be partly right, completely wrong, or only true for one responsibility inside the old abstraction.

Decompose responsibilities first.

---

# 3. Repository-wide census

Produce a complete top-level inventory of the repository after PERF-13.

Cover at least:

```text
Rust crates
TypeScript packages
native addons
generated ABI/schema code
code generators
build tooling
benchmarks
examples
reference application
tests
fixtures
replay traces
docs
scripts
feature flags
migration/debug-only code
```

For every significant package/crate/module, record:

| Field | Required |
|---|---|
| Path | exact path |
| Language | Rust / TS / generated / etc. |
| Approx production LOC | excluding tests/generated where practical |
| Approx test LOC | separately |
| Public API? | yes/no/partially |
| Primary responsibility | concise |
| Secondary responsibilities | all significant ones |
| Depends on | architectural dependencies |
| Depended on by | major reverse dependencies |
| Runtime lifetime owner | who creates/destroys it |
| Plane | structural/state/content/host/backend/mixed/not applicable |
| Hot path? | yes/no/conditional |
| PERF-13 critical? | yes/no |
| V5 disposition candidate | preserve/adapt/rewrite/delete/unknown |
| Evidence | paths + symbols |

Do not optimize for pretty prose yet.

Build the map.

---

# 4. Produce an actual dependency map

We need more than directory names.

Trace real dependency direction.

At minimum produce maps for:

```text
TypeScript package/module dependencies

Rust crate/module dependencies

TS → native boundary dependencies

retained structural runtime dependencies

retained state runtime dependencies

content runtime dependencies

layout dependencies

paint/output dependencies

interaction/input dependencies

History dependencies

application/runtime dependencies
```

Where useful, generate graphs automatically from:

```text
Cargo metadata
module imports
TS imports
package manifests
native-addon exports
generated ABI references
symbol/reference search
```

But verify generated graphs against the source.

The report must identify dependency violations such as:

```text
content depends on presentation API
layout depends on History
bridge schema depends on public View builders
terminal backend reaches directly into composition
state invalidation assumes current layout representation
```

These hidden edges matter more than folder names.

---

# 5. Public API census

Enumerate **every intentional public authoring surface** after PERF-13.

## Rust

Find and classify:

```text
pub exports
preludes
View constructors/builders
Scene APIs
Component APIs
History APIs
controls
layout authoring APIs
style/theme APIs
content authoring APIs
stream/projector APIs
application APIs
backend APIs
testing APIs
```

For each public Rust API concept, answer:

1. Who currently consumes it?
2. Is it used internally as well as externally?
3. Does its implementation exist mainly to support Rust-side UI authoring?
4. Does it also own runtime behavior that survives removal of Rust authoring?
5. Could that surviving runtime behavior exist independently?
6. How much code becomes unnecessary if the public authoring API disappears?
7. What tests exist only to validate the public authoring surface?

Important:

> Do not classify public Rust UI types as “make private” by default.

V5 has no public native Rust UI authoring API.

Ask whether the abstraction itself has any reason to continue existing.

## TypeScript

Likewise enumerate:

```text
current View API
defineView
State<T>
composition helpers
execution scopes
component-like abstractions
style/layout APIs
content APIs
runtime APIs
transport APIs
testing APIs
package-root exports
```

Record which parts React replaces versus which parts are transport/runtime utilities that React will still require.

---

# 6. Current structural model

Map the finished PERF-13 structural plane exactly.

Determine:

```text
native identity types
TS identity types
generation/lease semantics
node/occurrence ownership
parent-child topology
attachment ownership
root ownership
resource lifetime
prepare/commit behavior
rollback/failure behavior
cache identity
structural transaction types
bridge representations
```

Trace one complete lifecycle:

```text
application creates semantic UI thing
    ↓
TS composition
    ↓
structural representation
    ↓
bridge operation
    ↓
Rust allocation
    ↓
retained identity
    ↓
mount/parenting
    ↓
frame
    ↓
unmount/destruction
```

Name every significant type/function involved.

Then separately identify which parts are:

```text
fundamental retained-runtime machinery
current custom composition machinery
current View representation
custom layout-oriented structure
bridge-only representation
migration/debug infrastructure
```

This distinction will decide whether V5 preserves, adapts, or deletes each layer.

---

# 7. Current View / presentation architecture

This section is critical.

Map the current Rust `View` / presentation system in enough detail to answer:

> If we deleted the entire current View authoring and custom general-layout architecture, which genuinely necessary runtime responsibilities would be lost?

Enumerate:

```text
View types / variants / kind system
composition API
layout declarations
text/content attachment
style declarations
semantic IDs
state references
resource references
measurement
layout
resolved geometry
painting
damage
cache state
theme resolution
interaction metadata
backend-specific data
```

For every responsibility, identify where it truly belongs in V5.

Do not treat `View` as one indivisible thing.

Produce a table:

| Current responsibility inside/around View | Needed after V5? | Future owner candidate | Can old implementation survive independently? |
|---|---:|---|---|

Particularly identify cases where the correct result is:

```text
responsibility survives
current abstraction does not
```

That is likely to occur frequently.

---

# 8. Custom layout engine census

Map the existing terminal general-layout implementation comprehensively.

Find all code involved in:

```text
Row
Column
Grid
constraints
measure
placement
intrinsic sizing
child dependency propagation
dirty propagation
layout cache keys
resolved geometry
rounding
overflow
padding/margin/gap
min/max
text measurement integration
layout-triggered damage
layout tests
layout benchmark counters
debug visualization
legacy compatibility paths
```

Distinguish:

### A. General layout machinery expected to die with Taffy

from:

### B. Terminal-specific leaf measurement that must survive

from:

### C. Generic geometry/runtime machinery still needed after Taffy

from:

### D. Paint/damage code that merely consumes geometry and should survive

from:

### E. Controls with specialized internal spatial behavior that are not general layout

Do not produce the simplistic statement:

```text
presentation/ = delete
```

unless the source proves that.

Produce a responsibility-level deletion map.

Also identify every place outside the obvious layout directory that assumes details of the custom allocator.

Those are migration hazards.

---

# 9. Taffy insertion-point analysis

Without implementing Taffy yet, identify the cleanest places where Taffy could replace current general layout.

We need to know:

1. What retained object should correspond to a Taffy node?
2. Which current structure owns mutable layout properties?
3. Where are intrinsic leaf measurements requested?
4. Where is resolved geometry consumed?
5. Where does layout invalidation originate?
6. Which current dirty flags become redundant?
7. Which dirty flags are unrelated to general layout and must stay?
8. Whether a `TaffyTree` mirror would naturally fit current ownership.
9. Whether low-level Taffy traits over runtime-owned storage would eliminate more translation.
10. What existing abstraction would become a useless intermediate representation if retained.

Do not choose the Taffy integration strategy yet.

Give us the evidence needed to choose it.

---

# 10. TypeScript composition/runtime census

Map the complete current TypeScript composition path.

Specifically enumerate the responsibilities of:

```text
defineView
View builders
State<T>
tracked state
execution scopes
composition context
child ownership
identity
normalization
memoization
scheduling
runtime lifecycle
transport retention
state diffing
content attachment
event registration
bridge batching
```

For every responsibility answer:

```text
React/Fiber replaces this
three-plane TS runtime still needs this
Rust runtime owns this
this becomes unnecessary entirely
unknown
```

This section must make it possible to remove the current composition system without accidentally removing useful transport-side retention.

Remember:

```text
React/Fiber retention
!=
Iyon TS knowledge of what Rust has accepted
!=
Rust retained runtime state
```

Find the actual current code corresponding to each role.

---

# 11. Content architecture census

This section must be especially careful because much of V5's supposed “new” content architecture already exists.

Inventory in detail:

```text
content/
projection/
stream/
annotations
semantic text IR
Markdown
plain text
diff
ANSI if present
provenance/origin
style representation
projector composition
smoothing
snapshots
stream models
stream revisions
resident/projected forms
compile stages
viewport-specific projection
StreamPane or equivalents
source traits/types
renderers
validation
```

For each concept record:

```text
input type
output type
mutable state
width dependent?
viewport dependent?
backend dependent?
Source-owned?
consumer-owned?
shared?
cacheable?
incremental?
stream aware?
lifetime owner?
```

Then identify how current machinery likely decomposes into future conceptual roles:

```text
Source
Funnel
Connector
ContentPort
semantic IR
delivery policy
projection cache
backend projection
```

But use **many-to-many mapping**, not rename assumptions.

Example:

| Existing type/subsystem | Responsibilities today | Candidate V5 roles |
|---|---|---|
| `X` | source storage + projection cache + viewport state | Source + part of Connector |
| `Y` | immutable transform + stateful smoothing | Funnel spec + Connector state |
| ... | ... | ... |

Explicitly answer:

> What content machinery can be retained almost unchanged after removing the old View/layout/composition architecture?

And:

> What existing content code is accidentally coupled to old presentation/history types and therefore needs extraction?

---

# 12. Streaming and smoothing trace

Trace a real assistant-text streaming append end-to-end after PERF-13.

Start at the TS/application append call and follow it through:

```text
producer
source
transport/data lane
Rust Source
stream model
projector
semantic text/Markdown
smoothing
projection
measurement
layout invalidation
paint
frame scheduling
```

Record exact symbols and transitions.

For each stage state:

```text
runs once
runs per append
runs per smoothing tick
runs per width change
runs per frame
runs only when dirty
```

This is necessary to preserve the good part of the current content architecture while deleting unrelated machinery.

---

# 13. Semantic text IR census

Document the existing semantic text representation precisely.

Enumerate:

```text
document/block/inline types
style representation
annotations
provenance
origins
links
code blocks
Markdown semantics
diff semantics
ANSI semantics if present
selection hooks
theme/style resolution
width-dependent state
backend-dependent state
incremental parser state
```

Answer:

1. Is the IR truly backend-neutral?
2. Where does semantic styling become terminal physical styling?
3. Can colors/theme/effort be changed after parsing without reparsing source?
4. Which pieces currently collapse semantic data too early?
5. Which caches depend on theme versus content versus width?
6. Can the same IR naturally serve GPUI?
7. What would have to change for it to do so?

Do not redesign it yet.

Map reality.

---

# 14. History subsystem autopsy

Treat current `History` as a bundle of responsibilities, not one feature.

Enumerate everything it currently owns:

```text
semantic item ordering
text storage
component storage
streaming
live/completed/frozen lifecycle
scroll position
follow-end
viewport
row realization
residency
culling
measurement caches
layout caches
paint snapshots
selection
tool units
user messages
assistant messages
theme resolution
state propagation
editing
replacement
freeze
native scrollback behavior
```

Then classify each responsibility:

```text
application/conversation responsibility
generic Surface responsibility
content responsibility
layout responsibility
cache responsibility
backend responsibility
obsolete workaround
```

Produce a decomposition such as:

| Current History responsibility | Future likely owner | Current code reusable? |
|---|---|---|
| child ordering | React/app | no/partial |
| viewport clipping | Surface | yes/partial |
| frozen presentation snapshot | nobody | delete |
| ... | ... | ... |

Explicitly investigate the current `freeze` semantics and document exactly what becomes immutable and what remains reactive.

Use the current user-message-border/theme/effort bug as a concrete trace if it still exists.

The report must tell us what code can disappear when History becomes ordinary retained child occurrences inside a Surface.

---

# 15. Residency, caching, culling, and freezing

Inventory every current concept related to:

```text
live
completed
frozen
resident
cold
materialized
cached
visible
culled
dirty
snapshot
prepared
committed
```

These words may currently refer to very different things.

For each state machine record:

```text
owner
transition triggers
reversible?
semantic or optimization?
what memory/resources are retained?
what derived state is retained?
what invalidates it?
what becomes stale?
```

Then identify duplicated concepts.

We specifically need to know what current machinery can collapse into:

```text
semantic retained identity
+
reversible physical residency
+
dependency-keyed derived caches
```

and what cannot.

---

# 16. Component / Scene / application-kernel census

Map current Rust:

```text
Component
ComponentHandle
ComponentCx
slots
registries
Scene
SceneHost
application/App/AppCx
root ownership
mount lifecycle
scheduler integration
```

For each, determine whether its purpose is:

```text
UI authoring
retained native lifecycle
application process ownership
frame scheduling
backend ownership
input/focus routing
resource registry
```

This is crucial.

React may eliminate the component-authoring role while some root runtime/scheduling responsibilities still need a new owner.

Do not preserve `Component` merely because one useful scheduler call currently lives inside `ComponentCx`.

Extract responsibilities conceptually.

---

# 17. State-plane census

Map PERF-13's final retained state implementation.

Enumerate:

```text
declared/base state
overrides
effective state
PropertyId
PropertyDescriptor
effect classification
state storage
revisioning
dirty classification
layout properties
presentation properties
interaction-derived state
theme-dependent state
native control state
prepare/commit
validation
```

Then determine which pieces are independent of the current View/layout API.

Particularly separate:

```text
semantic state representation
general-layout invalidation implementation
presentation invalidation
interaction state
backend realization
```

We expect Taffy to delete significant general-layout dirty/dependency machinery but not the state plane itself.

Find the exact seam.

---

# 18. Theme / style / presentation census

Map all current mechanisms for:

```text
theme tokens
StyleSelector
state selectors
effort-dependent styles
focus/hover/active styles
foreground/background/borders
Markdown semantic styles
diff styles
terminal color realization
style caching
theme revisions
```

Determine:

```text
what is semantic
what is application theme policy
what is native presentation resolution
what is terminal-specific
what is bound to current View construction
what can react to theme changes without rebuilding structure
```

We need this to avoid carrying the current presentation API into V5 simply because useful semantic style logic lives there.

---

# 19. Interaction/input/control census

Map:

```text
keyboard
mouse/pointer
focus
hover
selection
paste
scroll
capture if present
TextInput
controls
callback routing
event subscriptions
terminal protocol activation
```

Trace one event end-to-end.

Classify ownership:

```text
backend decoding
native semantic event
hit testing
focus/control state
subscription presence
Rust→JS transport
JS callback registry
application callback
```

Identify where callbacks/closures currently live.

Identify what would have to change under React.

Also record any input code entangled with current `Component`/`Scene`/`View` types.

---

# 20. Terminal backend census

Map the terminal-specific pipeline from retained runtime to bytes written to the terminal.

Cover:

```text
terminal initialization
Termwiz integration
capability handling
surface/cell representation
text measurement
layout input/output
paint
damage
buffer diffing
escape generation
flush
cursor
selection
scrolling
scrollback
mouse
focus
paste
resize
cleanup/restoration
```

Identify:

```text
portable runtime logic
terminal backend logic
custom-layout-only logic
content-specific logic
History-specific logic
```

We want to know how much of the terminal backend survives after View + custom layout + History disappear.

---

# 21. Native-addon and ABI census

Document the final PERF-13 TypeScript↔Rust boundary completely.

For every exported native operation classify:

```text
structural
state
content control
content bulk data
event
host/backend
debug/testing
legacy
```

Record:

```text
entrypoint
wire representation
ownership
allocation
copy behavior
batching
transaction participation
failure semantics
generated schema source
call sites
```

Also enumerate every old/legacy transport path that still exists after PERF-13, if any.

Identify which ABI pieces are tied specifically to the old View representation versus generic retained occurrences/state/content.

---

# 22. Generated schema/codegen census

Map all code generation.

For each generator:

```text
input schema
generated TS
generated Rust
generated ABI constants
generated tests
consumers
```

Determine whether it currently knows about:

```text
View kinds
layout kinds
state properties
content properties
effect classes
wire representations
backend capabilities
```

This matters because V5 should extend useful schema generation rather than hand-build parallel systems.

Also identify generated code that will become obsolete when old APIs disappear.

---

# 23. Tests as architecture evidence

Do not treat tests merely as verification.

Use them to discover behavioral contracts.

Classify test suites by what architecture they protect:

```text
public Rust API
TS composition
three-plane invariants
identity/lifetime
layout
text measurement
content streaming
projection
smoothing
History
state overrides
theme
interaction
terminal rendering
ABI
benchmarks
failure semantics
```

For every major deletion candidate, identify:

```text
tests that should disappear because the feature disappears
tests whose behavior must be preserved through the replacement
tests that can become differential migration tests
```

This will be needed to design safe tranches.

---

# 24. Benchmarks and instrumentation census

Enumerate all counters and benchmark suites.

For each record:

```text
what it measures
what implementation assumption it contains
whether it remains useful in V5
whether it becomes an oracle during migration
whether it should eventually be deleted
```

Particularly locate instrumentation for:

```text
structural traffic
state traffic
content traffic
layout invalidation
measure/place
content projection
smoothing
paint/damage
frames
JS/native calls
cache hits/misses
History
```

The later V5 migration needs to know which existing measurements can prove that simplification did not regress hot paths.

---

# 25. Cross-cutting ownership matrix

Produce one consolidated ownership matrix.

Rows should be responsibilities such as:

```text
component composition
host occurrence identity
parent/child topology
layout declaration
layout computation
intrinsic text measurement
semantic text parsing
stream storage
smooth delivery
width projection
presentation resolution
theme lookup
scroll position
follow-end
residency
culling
paint
damage
input decoding
focus
event subscriptions
callbacks
host capabilities
backend services
frame scheduling
```

Columns:

```text
Current TS owner
Current Rust owner
Current backend owner
Current primary types
V5 intended owner
Migration action
```

This matrix is one of the most important outputs.

---

# 26. Candidate deletion map

After the factual inventory, produce a **candidate deletion map**.

Use these categories:

## Delete wholesale

The abstraction and its responsibilities are fully replaced.

## Delete after extracting surviving responsibilities

Most of the abstraction is obsolete, but one or more genuine runtime responsibilities need a new home first.

## Adapt in place

The abstraction already models a V5 concept well and should mostly survive.

## Rewrite behind stable semantics

The semantic responsibility survives, but implementation/ownership must materially change.

## Preserve

The subsystem is already aligned with V5.

## Unclear

Evidence is insufficient.

For every candidate deletion, include:

```text
exact directories/files
approx LOC
what behavior disappears
what surviving behavior must move first
reverse dependencies
tests affected
migration prerequisites
```

Do **not** optimize for maximizing deleted LOC.

Architecture is primary.

LOC is supporting evidence.

---

# 27. “If we delete X, what breaks?” analysis

For these major candidates, explicitly simulate deletion:

```text
current Rust View/presentation authoring model
custom terminal general-layout engine
Rust Component system
Rust Scene system
special History renderer
LIVE/COMPLETED/FROZEN lifecycle
current TS defineView/composition system
current Rust controls API
old theme/style authoring facade
migration-only PERF-13 layout dirty machinery
```

For each answer:

1. Compile-time dependents.
2. Runtime responsibilities lost.
3. Tests lost.
4. Reusable submodules trapped inside it.
5. Required replacement before deletion.
6. Things that turn out not to need replacement at all.

This exercise should expose the real migration graph.

---

# 28. Hidden coupling report

Make a dedicated list of surprising/undesirable couplings.

Examples:

```text
content parser imports presentation View
History owns layout cache
theme selector requires View reconstruction
text measurement depends on public layout builder
terminal input depends on Scene authoring API
stream projection assumes History row model
Component registry owns unrelated frame scheduler
```

For each coupling record severity:

```text
low
migration nuisance
major migration constraint
architectural blocker
```

These are likely to determine the tranche order.

---

# 29. V5 net-code hypothesis

Only after the architecture census, estimate the expected code movement.

Produce rough totals:

```text
production Rust LOC likely deleted
production Rust LOC likely retained
production Rust LOC likely heavily rewritten

production TS LOC likely deleted
production TS LOC likely retained
production TS LOC likely heavily rewritten

test LOC likely deleted
test LOC useful as migration oracle

known new implementation areas
```

Do not pretend this is exact.

More importantly, explain **why** code goes away.

We expect the V5 migration to be roughly code-neutral or net-negative even while adding capability because it removes duplicated architecture:

```text
custom composition → React
custom general layout → Taffy
special mixed History → Surface + ordinary children
public Rust UI authoring → deleted
freeze lifecycle → cache validity/residency
```

But verify that expectation against the finished repository rather than assuming it.

GPUI and Host Environment are genuine additions; much of content/stream/projection is expected to be adaptation of existing machinery.

---

# 30. Migration dependency graph

Do **not** write the final V5 tranche plan yet.

Instead produce constraints of the form:

```text
A must precede B because ...
C and D can be done independently because ...
E must remain as oracle until F passes ...
G can be deleted immediately after H because no other dependency remains ...
```

We want a partial order, not yet a project plan.

Especially identify seams around:

```text
React introduction
TS composition deletion
Rust public API deletion
content adaptation
History decomposition
Surface introduction
Taffy introduction
legacy-layout deletion
state dirty-machinery deletion
Host Environment
GPUI
```

---

# 31. Required final report structure

Deliver one document:

```text
POST-PERF13-ARCHITECTURE-CENSUS.md
```

Use this exact broad structure:

## 0. Executive summary

Two to five pages maximum.

State:

- what the current architecture actually is;
- biggest obsolete architectural masses;
- strongest surviving kernels;
- biggest hidden couplings;
- likely V5 deletion opportunities;
- main migration risks;
- areas where V5 assumptions were wrong.

## 1. Repository map

Packages, crates, modules, LOC, purposes.

## 2. Dependency architecture

Actual forward/reverse dependencies.

## 3. Public API surfaces

Rust and TypeScript.

## 4. End-to-end current runtime

One complete diagram from application semantic intent to terminal output.

## 5. Structural plane

## 6. State plane

## 7. Content plane

## 8. View / presentation system

## 9. Layout engine

## 10. Content / projection / stream / semantic text

## 11. History / scrolling / residency / caching

## 12. Component / Scene / application kernel

## 13. Theme / presentation

## 14. Input / interaction / controls

## 15. Terminal backend

## 16. Native ABI / generated schemas

## 17. Tests / benchmarks / observability

## 18. Cross-cutting ownership matrix

## 19. Hidden coupling report

## 20. Candidate preserve/adapt/rewrite/delete map

## 21. “Delete X” impact analyses

## 22. Migration dependency graph

## 23. Approximate code-size implications

## 24. Open questions requiring V5 design decisions

## 25. Evidence appendix

Paths, key symbols, generated graphs, commands, counts.

---

# 32. Required diagrams

Include at least these diagrams.

### Current end-to-end architecture

```text
application
→ TS API/composition
→ plane runtime
→ native ABI
→ Rust retained runtime
→ current layout/content/presentation
→ terminal backend
```

Use the real stages/names.

### Current ownership graph

Show:

```text
structure
state
content
layout
presentation
history
interaction
backend
```

and actual ownership edges.

### Content append path

One real streamed assistant token/chunk.

### Current layout path

One real Box/Row/Grid from declaration to resolved terminal cells.

### Current History path

One user/assistant/tool component entering history and eventually becoming cold/frozen/resident.

### Candidate V5 deletion boundary

Show actual existing subsystems divided into:

```text
likely survives
likely adapts
likely deleted
genuinely new V5
```

This final diagram is a conclusion, not the starting assumption.

---

# 33. Evidence standard

Every architectural claim should point to evidence.

Prefer:

```text
path/to/file.rs :: TypeName
path/to/file.ts :: functionName
specific module
specific test
specific generated schema
specific benchmark
```

Do not drown the main report in line-by-line citations, but make conclusions auditable.

Use exact symbols.

When saying:

> “History owns presentation snapshots”

show which types/functions prove that.

When saying:

> “View is only an authoring facade”

prove whether that is actually true.

When saying:

> “Projector already maps naturally to Funnel”

show its state/lifetime/input/output and note mismatches.

---

# 34. Things you MUST NOT do

Do not modify production code.

Do not begin the V5 migration.

Do not create React components.

Do not integrate Taffy.

Do not create GPUI code.

Do not rename current abstractions to V5 names.

Do not assume directory names equal architectural boundaries.

Do not assume all code in `presentation/` dies.

Do not assume all code in `content/` survives.

Do not assume `View` becomes an internal/private IR.

Do not assume `Scene` becomes a private runtime root.

Do not assume `Component` becomes a private occurrence.

Do not assume `History` becomes `ScrollSurface`.

Do not propose compatibility layers merely to keep old APIs alive.

Do not preserve an abstraction solely because other code currently depends on it.

Do not optimize for minimal migration diff.

The goal is the cleanest final architecture.

Migration scaffolding is temporary.

---

# 35. Things you SHOULD aggressively investigate

Look for abstractions that only exist because another abstraction exists.

Examples:

```text
A exists to construct B
B exists to feed C
C is being replaced

→ perhaps A + B + C all disappear
```

Look for duplicated ownership:

```text
React will own component identity
but current Component also owns identity
```

Ask what part is duplicate and what part is genuinely native.

Look for accidental translation chains:

```text
React props
→ old View style
→ old layout state
→ Taffy style
```

The desired architecture should usually be closer to:

```text
React prop
→ Iyon semantic property
→ retained native state
→ Taffy
```

Likewise:

```text
Source
→ old StreamPane
→ old History row
→ View text
→ renderer
```

may need to become:

```text
Source
→ Funnel/Connector
→ ContentPort occurrence
→ measure/paint
```

But prove this from the current implementation.

---

# 36. Questions the report must let us answer

At the end, another architect should be able to answer all of these without reopening the whole repository:

1. If the Rust public UI API vanished tomorrow, exactly what code becomes dead?
2. Which pieces of `View` represent actual runtime semantics and which only support current authoring/layout?
3. Can those surviving semantics be represented directly on retained occurrences without keeping `View`?
4. What exact code constitutes the custom general-layout engine?
5. What terminal measurement/paint machinery is independent of that engine?
6. What PERF-13 dirty/dependency logic becomes redundant under Taffy?
7. What state-plane machinery remains essential?
8. What does React genuinely replace?
9. What TS retention still exists beneath React?
10. What does the current Rust Component system do besides composition?
11. What does Scene do besides exposing a Rust application API?
12. What does History own today?
13. Which History responsibilities belong to React, Surface, content, caching, or terminal backend?
14. What does “freeze” actually freeze?
15. Which current cache/residency mechanisms already approximate V5?
16. How does a stream append currently travel through the system?
17. Which existing types already implement parts of Source/Funnel/Connector/Port?
18. Which content structures are width-independent?
19. Which projection state is width/viewport dependent?
20. Where does smoothing live and who owns its clock/state?
21. How reusable is current Markdown/diff/text IR across terminal and GPUI?
22. What semantic presentation information survives parsing?
23. Which code assumes terminal physical cells too early?
24. Where could Host Environment integrate without becoming a god object?
25. What terminal capability handling already exists?
26. What current event/callback model survives React?
27. Which generated schemas can be extended rather than replaced?
28. What existing benchmarks can serve as migration oracles?
29. What hidden coupling dictates migration order?
30. Approximately how much code should V5 delete versus add?
31. Which V5 concepts require genuinely new machinery?
32. Which apparently “new” V5 concepts are mostly existing machinery with corrected ownership?
33. What is the shortest clean dependency path from React host occurrence to Taffy?
34. What obsolete abstraction would remain if we naïvely “made the Rust API private” instead of deleting it?
35. What must be true before each major old subsystem can be deleted?

If the report cannot answer these, it is not complete.

---

# 37. Final mindset

The purpose of this work is not to prove the V5 design correct.

It is to give us enough factual understanding to **correct V5 where necessary before implementation**.

PERF-13 is valuable because it establishes the correct three-plane retained semantics.

That does not mean its complete implementation shape is sacred.

Likewise, years/months of work in an abstraction are not evidence that the abstraction belongs in the final system.

Classify code by responsibility, not sunk cost.

The central question throughout the census is:

> **If we were implementing the V5 destination from scratch today, knowing everything PERF-13 taught us, which parts of the finished PERF-13 implementation would we deliberately choose to keep?**

Everything else is a migration problem, not an architectural requirement.

# ADDITION — Architecture Drift, Parallel Paths, Fallbacks, and Migration Residue Audit

This is a **mandatory major section** of the census.

One of the purposes of this audit is to determine whether repeated architectural migrations have accumulated multiple ways of accomplishing the same semantic operation.

Do not assume that because a newer architecture exists, all production paths actually use it.

Actively search for:

```text
new path + old path

hot path + cold path

retained path + complete-decode path

generated ABI + legacy N-API object decode

new transport + compatibility transport

new cache + legacy cache

new state system + old mutation route

new content attachment model + older direct View mutation

new structural publication + old host.render(...)

normal path + "recovery" path that reconstructs everything

fast path + fallback that silently makes correctness appear intact
```

The key question is:

> **For every semantic operation, how many distinct production execution paths can currently perform it?**

If the answer is greater than one, document why.

---

## A. Build a semantic-operation → implementation-path matrix

Do not only inventory functions.

Inventory **operations**.

At minimum investigate:

```text
publish root structure
materialize a retained node
replace a root
recover a stale native reference

create/update a ViewSlot
create/update a ScrollPane
push/update/freeze History content

apply retained state
apply layout state
apply presentation state

create Source
append Source data
attach content
project content
smooth content

render a frame
measure text
perform layout
paint terminal output
```

For each operation produce:

| Semantic operation | Path A | Path B | Path C | Selection condition | Fallback? | Production reachable? | Intended final path? |
|---|---|---|---|---|---|---|---|

For example, if reality resembles:

```text
publish root

Path A:
    retained semantic node
    → generated retained ABI
    → native ref

Path B:
    semantic node
    → complete bridge object
    → N-API decode
    → native object
```

that must be recorded explicitly.

Do not hide this as an implementation detail.

---

## B. Hunt for silent fallbacks

Search specifically for code with semantics like:

```text
try fast/new path
if unavailable:
    use old path

try retained materialization
if refused:
    decode complete object

try cached/native reference
if stale:
    reconstruct through legacy bridge

if generated ABI unavailable:
    use compatibility renderer

catch:
    rebuild everything
```

Search names and control flow involving terms such as:

```text
fallback
cold
legacy
compat
compatibility
recover
recovery
retry
stale
slow
direct
decode
materialize
rebuild
full
ordinary
old
v2
v3
packed
bridge
napi
ref
```

Do not rely only on names.

Inspect control flow.

A fallback may be hidden behind an innocent helper.

---

## C. Distinguish legitimate alternate modes from architectural residue

Multiple paths are not automatically wrong.

Classify every parallel path as one of:

### 1. Legitimate distinct semantic mode

Example:

```text
terminal backend
vs
GPUI backend
```

Different physical realization is intentional.

### 2. Temporary migration oracle

Example:

```text
legacy layout
vs
Taffy layout during differential migration
```

Allowed only while explicitly serving migration validation.

Must have a deletion gate.

### 3. Required recovery mechanism

Only classify something here if recovery **cannot conceal failure of the authoritative implementation**.

Explain why the recovery path is necessary.

### 4. Compatibility path

Identify exactly what old artifact/client/version requires it.

If there is no supported compatibility requirement, mark it as likely residue.

### 5. Performance specialization

Two implementations may be justified if semantics are identical and the slower one is deliberately invoked rather than silently masking failure.

Document how equivalence is tested.

### 6. Architectural residue / incomplete migration

An older implementation remains reachable because not all call sites were migrated.

This is a high-priority cleanup candidate.

### 7. Dangerous silent fallback

A newer authoritative path can fail, refuse, or be bypassed and an older implementation silently makes the operation succeed.

This is particularly dangerous because:

```text
new architecture can be broken
    ↓
fallback succeeds
    ↓
tests/application appear correct
    ↓
nobody knows new architecture is broken
```

Flag these prominently.

---

# D. Authoritative-path audit

For every important subsystem answer:

> **What is the authoritative implementation?**

Then prove that production execution actually uses it.

Examples:

```text
What is the authoritative structural publication path?

What is the authoritative state mutation path?

What is the authoritative content append path?

What is the authoritative root materialization path?

What is the authoritative ViewSlot update path?

What is the authoritative History update path?

What is the authoritative terminal layout path?
```

Then answer:

```text
Can the authoritative path currently be bypassed?

Can it return "not handled" and cause another path to execute?

Can failure be converted into success through reconstruction?

Can an older native API perform the same semantic operation?

Do some components exclusively use the old route?

Do tests exercise the authoritative route or merely final output?
```

Do not accept comments claiming a path is authoritative.

Trace call sites.

---

# E. "Does the hot path actually have to work?" test

For every performance-critical/new retained path, determine whether the system can still function when that path is intentionally disabled or broken.

Conceptually:

```text
break Path A deliberately

Does application correctness fail loudly?

or

Does Path B silently take over?
```

Do not modify production source solely for this census unless an existing test/feature mechanism supports it.

Static tracing may be sufficient.

But the report must answer whether a fallback exists that could conceal breakage.

This is especially important for:

```text
retained structural publication
native ref reuse
state fast paths
content incremental paths
projection caches
layout caches
direct data lanes
generated ABI paths
```

If a benchmark claims to measure Path A but Path B can execute during the same benchmark, flag the benchmark as untrustworthy until route observability is proven.

---

# F. Benchmark route integrity

For every important benchmark, determine:

```text
Which exact production path does it execute?

Can fallback occur?

Does the benchmark assert which route ran?

Are route counters available?

Could the benchmark report healthy latency while measuring the wrong implementation?
```

A benchmark of a retained/hot architecture MUST NOT silently become a benchmark of a cold/compatibility implementation.

Prefer a requirement like:

```text
expected route counter:
    retained = N
    cold     = 0
    legacy   = 0

otherwise:
    benchmark fails
```

Record where current benchmarks do and do not enforce this.

---

# G. Tests that accidentally bless obsolete paths

Search tests for direct construction/use of:

```text
legacy bridge objects
cold lowering
complete JSON/native decode
old host.render APIs
deprecated constructors
compatibility helpers
old mutation methods
```

Determine whether tests are:

```text
testing a deliberately retained compatibility contract
using an old helper merely for convenience
using old behavior as a correctness oracle
accidentally keeping dead architecture alive
```

An old implementation serving as a temporary oracle is acceptable.

An old implementation serving as ordinary test infrastructure indefinitely is dangerous because new code becomes dependent on it.

Record this distinction.

---

# H. Migration completeness by subsystem

For every major architectural migration already performed, report whether it is actually complete.

Use statuses:

```text
COMPLETE
    one production implementation remains

DUAL BY DESIGN
    two paths intentionally exist with documented distinct semantics

TEMPORARY DUAL
    migration/oracle path remains with explicit deletion condition

PARTIAL
    newer architecture exists but some production operations still use old path

FALLBACK-MASKED
    newer path is preferred but old path silently takes over on failure/refusal

LEGACY-DOMINANT
    new architecture exists, but significant production behavior still primarily uses old implementation

UNKNOWN
    evidence insufficient
```

Apply this to at least:

```text
structural publication
native materialization
state mutation
content transport
ViewSlot
ScrollPane
History
root rendering
layout
streaming
projection
smoothing
ABI transport
```

---

# I. Compatibility claim verification

Whenever code says or implies:

```text
older addon support
compatibility
legacy caller
safe fallback
old protocol support
```

answer:

1. Is that compatibility still a supported product requirement?
2. Which version/artifact needs it?
3. Is that version still buildable/distributed?
4. Is the path exercised by current production?
5. Is there a documented removal condition?

If nobody can identify a supported consumer, classify the path as **unjustified compatibility residue**.

Do not preserve dead architecture because a comment says “compatibility”.

---

# J. Production reachability

For every suspicious old path determine whether it is:

```text
dead code

test-only

benchmark-only

debug-only

feature-flagged

compatibility-only

fallback-only

ordinary production reachable

primary production path
```

This distinction must be based on actual call graphs and configuration, not filenames.

A file named `cold-*` may still be on the primary production path.

A file named `legacy-*` may still be required.

A file with no alarming name may contain the actual fallback.

---

# K. Inconsistency register

Create a dedicated:

```text
ARCHITECTURAL INCONSISTENCY REGISTER
```

Do not limit it to known issues.

Whenever two parts of the repository appear to disagree about the architecture, record it.

Examples:

```text
spec says generated retained ABI is authoritative
but subsystem X still uses full object N-API decode

spec says structural mutations use one transaction path
but control Y performs direct native mutation

spec says content bypasses structure
but component Z rebuilds a View for text append

spec says state mutations are property deltas
but path Q retransmits whole state object

new retained cache exists
but old cache is still maintained in parallel

two different source-of-truth representations exist

two different ownership models exist for equivalent resources

one subsystem treats object X as semantic identity
another treats object Y as identity

some code assumes one lifecycle while another subsystem assumes another

comments/specification describe one architecture
call graph demonstrates another
```

Each entry must contain:

| Field | Meaning |
|---|---|
| ID | `INC-001`, etc. |
| Area | structural/content/layout/etc. |
| Expected architecture | according to current design |
| Actual implementation | what code does |
| Evidence | exact paths/symbols |
| Why inconsistent | explanation |
| Production impact | correctness/perf/complexity/etc. |
| Masks failure? | yes/no |
| V5 relevance | delete/adapt/blocker/etc. |
| Recommended disposition | investigate/delete/consolidate/etc. |

Do not silently decide which side is correct.

Surface the inconsistency.

---

# L. Architecture duplication register

Separate from inconsistencies, enumerate places where two systems solve substantially the same problem.

Examples:

```text
two structural materializers

two complete native decoders

two ways to update a retained view

two layout dependency systems

two caching systems for equivalent results

two stream projection paths

two history storage paths

two APIs owning the same lifecycle

two definitions of semantic identity

two ways to schedule equivalent frames
```

For every duplication answer:

```text
Why do both exist?

Which came first?

Was one introduced as migration scaffolding?

Does one still have unique consumers?

Can one fail over to the other?

Which is measured?

Which is tested?

Which is intended to survive?
```

---

# M. "Never migrated" subsystem audit

Repeated architectural rewrites create another failure mode:

> The central path migrated, but peripheral abstractions continued using the old architecture indefinitely.

Actively search for these.

Candidate clues:

```text
ViewSlot
ScrollPane
History
animations
controls
testing helpers
benchmarks
debug utilities
less-common render paths
error rendering
empty/error states
initial seed paths
replacement/update methods
recovery paths
```

For every major abstraction answer:

> Does this subsystem participate in the same modern structure/state/content architecture as the main root path?

If not, mark:

```text
MIGRATION GAP
```

and document the old mechanism it still uses.

---

# N. Initial construction vs update consistency

Check whether create/seed/update paths use different architectures.

For example:

```text
create:
    cold object decode

update:
    retained reference mutation
```

or the opposite.

Check separately:

```text
creation
first materialization
ordinary update
replacement
animation update
reset
unmount
recovery
```

A subsystem is not fully migrated merely because its common update path is modern.

---

# O. Failure semantics audit

For every fallback/recovery path answer:

```text
What exact failure triggers it?

Programmer error?

Stale cache?

Unsupported native artifact?

Budget exhaustion?

Allocation failure?

Validation failure?

Transport absence?

Old ABI?

Normal cache miss?
```

These are not equivalent.

Particularly flag:

```text
validation failure → alternate implementation succeeds

invariant violation → reconstruct whole object and continue

new-path inability → silently use old architecture
```

Those mechanisms may turn architectural bugs into invisible slow paths.

The report should recommend which failures should instead be:

```text
fatal invariant error

explicit unsupported-version error

observable cold start

explicit retry

ordinary cache miss
```

Do not implement the change during this census.

---

# P. Route observability

Determine whether the current runtime can tell us which path executed.

Inventory counters/logging for:

```text
retained
cold
fallback
legacy
direct
compatibility
full decode
cache recovery
stale recovery
```

If multiple routes exist without route observability, classify this as a problem itself.

For every multi-path operation we eventually want the ability to assert:

```text
THIS operation used THIS architecture.
```

Without that, correctness and performance testing are ambiguous.

---

# Q. Add these questions to the mandatory final-answer list

The final report must additionally let us answer:

36. How many production structural publication paths exist?
37. How many complete semantic-View → native materialization paths exist?
38. Which production operations still use legacy/cold/full-object decoding?
39. Which subsystems never migrated to the newest retained architecture?
40. Where can a new/hot path silently fall back to an older implementation?
41. Which fallbacks can mask correctness bugs?
42. Which fallbacks can invalidate performance measurements?
43. Which benchmarks assert that the intended path actually ran?
44. Which tests still depend on obsolete implementations as helpers/oracles?
45. Which compatibility paths have an identifiable supported consumer?
46. Which compatibility paths exist only because nobody deleted them?
47. Where do creation, update, replacement, and recovery use different architectures?
48. Which subsystem migrations are COMPLETE, PARTIAL, FALLBACK-MASKED, or LEGACY-DOMINANT?
49. Where do comments/specification and actual control flow disagree?
50. Which duplicate systems can be deleted before V5 even begins?
51. Which duplicate systems must survive temporarily as migration or correctness oracles?
52. What runtime observability is missing to prove that the intended architecture is executing?
53. If every legacy/cold fallback were disabled, what currently stops working?
54. If the retained/hot implementation were deliberately broken, what operations would still appear to work?
55. Are any "performance wins" currently capable of silently measuring another path?

---

# R. Add a top-level report section

The final `POST-PERF13-ARCHITECTURE-CENSUS.md` must contain near the front:

## Architecture Drift and Migration Completeness

This section must summarize:

```text
authoritative paths
parallel paths
cold paths
legacy paths
compatibility paths
fallback paths
recovery paths
migration gaps
duplicate implementations
route observability
```

Provide a table:

| Area | Intended authoritative path | Other production path(s) | Status | Silent fallback? | Cleanup urgency |
|---|---|---|---|---|---|

Then provide:

### Critical inconsistencies

The highest-risk cases where the repository contradicts its intended architecture.

### Silent fallback hazards

Anything capable of hiding failure of the intended implementation.

### Never-migrated subsystems

Peripheral systems that still execute a previous-generation architecture.

### Benchmark integrity hazards

Cases where performance tests cannot guarantee which implementation they measured.

### Cleanup possible before V5

Residue that has no legitimate reason to survive even until the V5 migration.

---

# S. Strong rule for the census

The existence of a fallback is not evidence that the system is robust.

Sometimes the opposite is true.

A fallback is architecturally dangerous when:

```text
it implements the same semantics through an older architecture

AND

it activates automatically when the intended architecture fails

AND

the caller cannot tell that it happened
```

That creates:

```text
broken new path
    ↓
successful old fallback
    ↓
green tests
    ↓
apparently healthy application
    ↓
incorrect benchmark conclusions
    ↓
obsolete architecture never dies
```

Treat that pattern as a major finding.

---

# T. Specific example to keep in mind — but independently verify

A pattern already observed during current investigation resembles:

```text
semantic View
    ├── retained/native-reference materialization
    │
    └── complete cold bridge lowering
            ↓
        N-API full decode
```

with possible usage as:

```text
preferred retained route
    ↓ failure/refusal
cold/full decode route
```

and some peripheral systems potentially still invoking the cold/full-decode mechanism directly.

Do **not** assume the exact details remain true after PERF-13.

Re-audit them from the final repository.

The purpose of mentioning this example is to demonstrate the class of problem we are looking for:

> A migration is not complete merely because the new path exists.

It is complete when the old path is either deliberately isolated for a documented temporary purpose or gone.
