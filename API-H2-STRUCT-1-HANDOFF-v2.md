# API-H2 / STRUCT-1 — TypeScript Source Architecture Cleanup

**Repository:** `alexykn/iyon-tui`  
**Sequence:** `PERF-12 → API-H1 → API-H2 / STRUCT-1 → PERF-13`  
**Scope:** `packages/iyon-tui/src/**`  
**Intent:** physical source architecture only; **no behavior change**

> This revision keeps the original H2 ownership model, but makes the `composition/` vs `runtime/` boundary normative, gives the current bridge schema/generated ABI an explicit structural owner, avoids premature file fragmentation, and replaces inventory-first sequencing with a smaller number of atomic cuts.

---

# 0. Executive directive

H2 makes the TypeScript filesystem match the architecture already established by PERF-12 and API-H1, before PERF-13 introduces two additional native-facing planes.

```text
                         PUBLIC SEMANTICS
                              api/
                               │
                ┌──────────────┴──────────────┐
                │                             │
                ▼                             ▼
       retained semantic                live framework
          evaluation                     orchestration
        composition/                       runtime/
                │                             │
                └──────────────┬──────────────┘
                               ▼
                           transport/
                    ┌──────────┼──────────┐
                    │          │          │
               structural    state     content
                    │          │          │
                    └──────────┼──────────┘
                               ▼
                         native / ABI
                               │
                               ▼
                              Rust
```

For H2:

- `transport/structural/` is real and populated from PERF-12.
- `transport/state/` is a future PERF-13 ownership slot only.
- `transport/content/` is a future PERF-13 ownership slot only.
- H2 must not implement either future plane.

The filesystem must answer five questions clearly:

```text
api/          What concepts does the caller think about?
composition/  How does TS retain semantic View identity/reuse/execution?
runtime/      Who owns live Tui instances, orchestration, and lifetime?
transport/    How does an already-defined semantic operation cross to Rust?
testing/      What exists only for harnessing, injection, or inspection?
```

If H2 changes rendering, retained identity, lifecycle, scheduling semantics, structural ABI semantics, or equivalent-workload performance, H2 has failed.

---

# 1. Why H2 exists

The current flat source tree mixes unrelated architectural layers as peers:

```text
public semantic values
retained semantic execution
host lifecycle
raw native access
structural bridge lowering
generated ABI
testing
```

That is the actual problem—not the raw number of files.

PERF-13 would otherwise add:

```text
retained geometry state
retained presentation state
retained interaction state

ContentPort
Source
Funnel
Connector

state-plane transport
content control/data transport
```

into the same namespace.

H2 must create those ownership boundaries before PERF-13 needs them.

---

# 2. Required sequencing

```text
PERF-12
    retained structural composition + transport are proven
        ↓
API-H1
    public semantic surface is closed and cleaned
        ↓
API-H2 / STRUCT-1
    physical TS ownership is cleaned
        ↓
PERF-13
    structural DAG + retained state plane + content plane
```

H2 consumes H1 decisions. It does not reopen public naming, lifecycle, theme/style semantics, output/event semantics, testing topology, or false-alias removal unless a correctness bug is found.

If H1 leaves an awkward public API name, fix it in H1 or a separate API follow-up. Do not opportunistically rename semantic API while moving files in H2.

---

# 3. Hard invariants

## 3.1 PERF-12 is protected

H2 must preserve:

- stable `NodeId` semantics,
- retained semantic View identity,
- unchanged-subtree cutoff,
- `defineView` execution-scope reuse,
- `State<T>` subscription ownership,
- keyed and unkeyed occurrence identity,
- builder-root ownership,
- direct-render takeover,
- `ViewSlot` retained builder ownership,
- `ScrollPane` retained builder ownership,
- `PersistentSeq`,
- retained axis/grid derivations,
- retained path edits,
- multi-edit transactions,
- `NativeRef` behavior,
- caches and leases,
- cold recovery/runtime generations,
- N-API/direct-FFI structural parity.

The canonical path remains:

```text
defineView / View construction
        ↓
retained TypeScript composition
        ↓
stable semantic View DAG
        ↓
changed semantic frontier
        ↓
structural transport lowering
        ↓
Rust retained graph
```

H2 relocates modules; it does not redesign this flow.

## 3.2 No PERF-13 implementation

Do not add:

- retained mutable geometry/presentation/interaction handles,
- content ports/sources/funnels/connectors,
- cold/buffered/hot behavior,
- state-plane mutation ABI,
- content FFI,
- Kitty/Sixel/video,
- new damage propagation,
- new layout invalidation semantics,
- property-level reactive bindings.

## 3.3 No compatibility maze

Temporary forwarding modules are acceptable during intermediate commits only. Remove them before H2 completion unless they represent a real external compatibility contract.

---

# 4. Normative domain boundaries

This section is the authority for classifying ambiguous code.

## 4.1 `api/` — semantic framework model

`api/` owns concepts that belong in user-facing framework documentation.

Examples:

```text
View
Scene
geometry/layout specs

Theme
Style
Color

text/diff/projection semantic values

History
TextInput
TextStream
ViewSlot
ScrollPane

Renderer/Projector/visitor extension contracts

TuiError
```

It answers:

> What does this framework concept mean to its caller?

It does not own `Native*`, `Bridge*`, generated ABI records, materialization, addon loading, raw handle IDs, or retained bridge bookkeeping.

## 4.2 `composition/` — retained semantic evaluation

`composition/` owns machinery that determines **semantic retained execution and identity**:

```text
defineView execution
execution scopes
State<T> dependency tracking
scope dirtiness
child occurrence ownership
View.key identity
builder-root semantic execution
retained semantic composition
PersistentSeq semantic child structure
semantic reuse/cutoff metadata
```

### Normative rule

> Composition owns retained semantic evaluation. It does not own the live Tui host or native boundary.

Composition may know semantic Views, state, scopes, child identities, builder roots, and retained semantic metadata.

Composition must not fundamentally own:

```text
Tui.open / Tui.close
terminal host lifecycle
addon loading
raw native handles
input routing
host queues
control disposal registry
```

### Definition vs orchestration

```text
composition/
    defines and performs retained semantic execution

runtime/
    creates, owns, drives, and disposes live instances of that machinery
```

If an `ExecutionRuntime` contains the retained builder algorithm, that algorithm belongs to composition. Its ownership by a Tui host and teardown belong to runtime.

**If a current file contains both, split it.**

A strong test:

> If this code could conceptually execute semantic Views against an abstract consumer without knowing there is a terminal host, it belongs to composition.

## 4.3 `runtime/` — live orchestration and lifetime

`runtime/` answers:

> Who owns this live thing, who drives it, and when does it die?

It owns:

```text
Tui host lifecycle
open/close/exit
live runtime instance ownership
event/input orchestration
control registration
framework-owned handle lifetime
creation/teardown of composition-runtime instances
host-level render ownership transitions
runtime liveness/generation where semantic rather than raw ABI
```

It does not own semantic View identity algorithms, raw bridge IR, generated ABI calls, or addon record definitions.

Architectural direction:

```text
runtime
    owns/uses
composition
```

not the other way around.

## 4.4 `transport/` — native boundary mechanics

`transport/` answers:

> How is an already-defined semantic operation represented and delivered to Rust?

Long-term:

```text
transport/
├── structural/
├── state/          # PERF-13
├── content/        # PERF-13
├── native/
└── abi/
```

It must never become a second semantic API.

## 4.5 `testing/`

Own harnesses, injection, clock advancement, and screen/cell/style inspection. Production must not depend on it.

---

# 5. Target tree

The target is deliberately coarse-grained:

```text
src/
├── api/
│   ├── view/
│   ├── presentation/
│   ├── content/
│   ├── controls/
│   ├── extensions/
│   └── errors.ts
│
├── composition/
│   └── ...
│
├── runtime/
│   └── ...
│
├── transport/
│   ├── structural/
│   │   └── ...
│   ├── state/               # PERF-13 ownership slot only
│   ├── content/             # PERF-13 ownership slot only
│   ├── native/
│   │   └── ...
│   └── abi/
│       └── structural/
│           ├── schema/
│           └── generated/
│
├── testing/
│   ├── index.ts
│   └── ...
│
└── index.ts
```

## File-granularity rule

> One file per cohesive responsibility, not one file per exported symbol.

Do **not** pre-create `grid.ts`, `border.ts`, `selector.ts`, etc. merely because the type exists.

For example this may be enough:

```text
api/view/
├── view.ts
├── scene.ts
└── geometry.ts
```

Likewise, keep the current user-implementable trait family together initially:

```text
api/extensions/
└── traits/
    ├── component.ts
    ├── projector.ts
    ├── renderer.ts
    ├── streaming-source.ts
    ├── text-rewriter.ts
    └── text-visitor.ts
```

This gets the traits out of the root namespace without prematurely designing the post-PERF-13 extension taxonomy.


---

# 6. `api/` structure

## 6.1 `api/view/`

Own structural semantic values:

```text
View
Scene
geometry/layout specs
structural component references if H1 keeps them
```

Likely initial moves:

```text
values/view.ts
scene.ts
values/geometry.ts
```

Do not split grid/border/etc. unless post-H1 cohesion actually requires it.

## 6.2 `api/presentation/`

Own:

```text
ColorSpec / color semantics
Style
StyleSpec
StyleRef
Theme
ThemeKey
StyleSelector
```

Move semantic responsibilities from:

```text
values/style.ts
values/theme.ts
values/theme-key.ts
style-internals.ts
```

`style-internals.ts` should be decomposed, not simply renamed. Semantic concepts stay here; bridge lowering goes to transport.

## 6.3 `api/content/`

Own current semantic content concepts:

```text
text
text content
annotations
projection
diff
stream snapshots
semantic projector helpers
```

This directory intentionally exists before PERF-13 so future `Source`, `Funnel`, `Connector`, and `ContentPort` have a semantic home.

H2 must not create those types.

## 6.4 `api/controls/`

Own public controls after H1:

```text
History
TextInput
TextStream
ViewSlot
ScrollPane
```

Public semantic classes may delegate to runtime/native machinery internally. Their caller-facing concept still belongs here.

## 6.5 `api/extensions/`

Do not over-design this domain in H2.

Move the existing H1-approved public trait family as a unit under:

```text
api/extensions/traits/
```

unless H1 already collapsed or renamed it.

This is a deliberate compromise: ownership becomes clear now, while PERF-13 remains free to revise the extension taxonomy later.

## 6.6 `api/errors.ts`

Own public framework error semantics. Raw native error decoding remains private.

---

# 7. `composition/` structure

Likely composition core:

```text
compose.ts
define-view.ts
execution.ts
execution-context.ts
tracked-state.ts
child-owner.ts
persistent_seq.ts
internal-composition.ts
view-internals.ts
composition-related parts of tui-execution.ts
```

Do not assume one input file equals one output file.

### `internal-composition.ts`

Classify functions individually:

- semantic retained execution → `composition/`
- structural bridge lowering → `transport/structural/`
- live host lifecycle → `runtime/`

### `view-internals.ts`

Likewise:

- semantic identity/reuse metadata → `composition/`
- semantic View helper → `api/view/`
- bridge/materialization metadata → `transport/structural/`

### `tui-execution.ts`

This is no longer a deferred "runtime or composition?" question.

Apply the boundary:

```text
semantic execution algorithm / scope engine
    → composition/

live Tui ownership, scheduling, creation, disposal of that engine
    → runtime/
```

Split the file if both responsibilities exist.

---

# 8. `runtime/` structure

Likely current candidates:

```text
tui.ts
runtime.ts
handles.ts
parts of native-handles.ts
parts of tui-execution.ts
parts of component-facade.ts
```

Potential shape:

```text
runtime/
├── tui.ts
├── handles.ts
├── execution-host.ts
├── events.ts
├── interaction.ts
└── ...
```

Only create files that correspond to actual cohesive responsibilities.

### `runtime.ts`

A generic `runtime/runtime.ts` should not survive automatically.

Classify contents:

- host implementation → specific runtime owner
- public contract → semantic owner
- semantic retained execution → composition
- native calls → transport

The directory should reduce ambiguity, not hide it one level deeper.

---

# 9. The native-handle seam

This is the highest-risk H2 classification.

Do not pre-decide that the solution must be exactly `addon.ts`, `handles.ts`, and `contracts.ts`.

Use three ownership tests.

## 9.1 Public semantic handle

If callers use it as a framework concept:

```text
TextInput
History
ViewSlot
ScrollPane
Output<T>
```

→ `api/controls/` or the relevant semantic domain.

## 9.2 Runtime lifetime/registry state

If it owns:

```text
liveness
dispose-on-host-close
registration
runtime generation
semantic handle ownership
host/control association
```

→ `runtime/`.

## 9.3 Raw native representation/access

If it exists to:

```text
hold native IDs
unwrap raw native objects
call generated addon functions
describe native-only contracts
load the addon
```

→ `transport/native/`.

### Hard rule

> Do not let one class remain simultaneously the public semantic handle, runtime lifetime registry, and raw addon wrapper merely because the old architecture combined them.

H2 may split those existing responsibilities, but it must not change behavior.

---

# 10. Structural transport

Move the PERF-12 structural boundary as one subsystem.

Likely candidates:

```text
ir.ts
retained_dag.ts
native_view_abi.ts
native_view_policy.ts
```

Target:

```text
transport/structural/
├── ir.ts
├── retained-dag.ts
├── lowering.ts / abi.ts / materialize.ts
├── policy.ts
└── ...
```

Naming follows actual responsibility.

The key distinction is:

```text
composition/
    "is this semantic View occurrence reused?"

transport/structural/
    "how is the resulting retained semantic frontier represented to Rust?"
```

Both are retained. They are not the same subsystem.

---

# 11. Structural ABI ownership — resolved

The first handoff left schema placement open. H2 must resolve it.

The current `bridge-schema.json` and generated View ABI belong to the **structural plane**.

Place them under:

```text
transport/abi/structural/
├── schema/
│   └── bridge-schema.json
└── generated/
    ├── view_abi_conformance.ts
    ├── view_abi_manifest.json
    ├── view_abi.ts
    ├── view_calls.ts
    └── view_materialize.ts
```

Reasoning:

1. The existing schema describes the View/structural bridge.
2. PERF-13 introduces distinct state and content planes.
3. A generic `transport/abi/schema` location would imply the current View schema is the universal ABI.
4. PERF-13 must be free to add `transport/abi/state/` or `transport/abi/content/` only if those planes actually benefit from the same generated model.
5. The future content fast FFI lane may use a different ABI definition mechanism entirely.

### Generator rule

Update generator inputs/templates, staging scripts, build scripts, and CI to use the new path.

Never hand-edit generated artifacts to compensate for relocation.

---

# 12. `transport/native/`

Own physical addon access:

```text
addon loading
raw addon API
raw native handle representation
native-only contracts
```

Likely inputs:

```text
native.ts
raw portions of native-handles.ts
remaining private Native* contracts after H1
```

Do not fragment for aesthetics. If one small `native.ts` remains coherent after moving into `transport/native/`, that is preferable to three artificial files.

---

# 13. `testing/`

Move this early because ownership is unambiguous.

Target backs:

```text
@iyon/tui/testing
```

Likely:

```text
testing/
├── index.ts
├── harness.ts
├── access.ts
└── ...
```

Inputs:

```text
testing.ts
testing-access.ts
AppHarness
createAppHarness
input injection
clock advancement
screen/cell/style inspection
```

Production must not import it.

---

# 14. Root export policy

`src/index.ts` remains a curated public root.

It must not export:

```text
transport/*
composition internals
runtime internals
raw Native*
Bridge*
generated ABI
testing utilities
```

`@iyon/tui/testing` gets its own export entry.

The root barrel does not mirror the filesystem.

---

# 15. Eliminate `types.ts`

A generic root `types.ts` is an ownership smell.

Move types to their owners.

Examples:

```text
SceneProducer
    → api/view/scene.ts

Renderer
    → api/extensions/traits/renderer.ts

Projector
    → api/extensions/traits/projector.ts

content snapshot types
    → api/content/

control options
    → owning control module

runtime events
    → runtime/events.ts or the H1-approved semantic event owner
```

Do not replace it with:

```text
shared/types.ts
common/types.ts
api/types.ts
```

unless a truly cross-domain primitive exists.

---

# 16. Other ambiguous modules

## `component.ts` / `component-facade.ts`

H1 decides what `Component` means publicly.

H2 then physically separates existing responsibilities.

Possible destinations:

```text
api/view/component.ts
api/extensions/traits/component.ts
runtime/component-host.ts
transport/native/...
```

Do not preserve ambiguous peer names.

## `style-internals.ts`

Decompose by responsibility:

```text
semantic presentation
    → api/presentation/

bridge lowering
    → transport/structural/

live runtime behavior
    → runtime/
```

## `runtime.ts`

Classify its contents by §4, not its filename.

## `view-internals.ts`

Split semantic retained identity from structural transport data if both are present.

---

# 17. Import-direction rules

Machine-enforce these where practical.

## Rule 1 — no generated ABI in semantic API

Preferred:

```text
api/**  -X->  transport/abi/**
```

Public semantic types must never name generated records.

## Rule 2 — no raw native contracts in public declarations

API implementation may use a narrow private runtime/native seam, but emitted declarations stay semantic.

## Rule 3 — no production → testing

Always forbidden.

## Rule 4 — composition does not own host lifecycle

Composition must not depend on modules whose primary purpose is Tui open/close, addon loading, input routing, or control registry.

## Rule 5 — runtime may own composition instances

Runtime may import composition to create/drive retained semantic execution.

## Rule 6 — structural bridge IR has one owner

Private bridge `*Node` records belong to structural transport/ABI.

## Rule 7 — no external deep imports

Iyon and fixture consumers import only:

```text
@iyon/tui
@iyon/tui/testing
```

## Rule 8 — no architectural escape hatches

Do not introduce:

```text
shared/
common/
misc/
utils/
```

to avoid ownership decisions.

---

# 18. Barrel policy

Allowed:

```text
src/index.ts
testing/index.ts
```

Internal barrels only when they define a real subsystem seam.

Prefer direct internal imports so architecture remains visible in code review.

Do not hide cycles behind barrels.

---

# 19. Classification flow

Use this order for every symbol/file:

1. **Is this part of the user-facing framework model?**  
   → `api/`

2. **Does it determine retained semantic View identity, execution scope, dependency ownership, or reuse?**  
   → `composition/`

3. **Does it own a live Tui instance, runtime instance, event/control orchestration, or lifecycle?**  
   → `runtime/`

4. **Does it represent or cross the TS/native boundary?**  
   → `transport/`

5. **Does it exist only for harnessing/injection/inspection?**  
   → `testing/`

If one current file answers yes to multiple categories, split by responsibility.

Do not create an `internals/` dumping ground to postpone the decision.


---

# 20. Revised execution strategy

Do **not** begin H2 by demanding a complete classification spreadsheet for every file.

That creates a planning bottleneck around exactly the files whose ownership becomes clearer after easy subsystems move.

Use a **live classification ledger** during execution:

```text
current path
target path
move / split / merge / delete
reason
```

Only unresolved files in the next atomic cut need a final decision.

The restructure should happen in six substantial cuts.

---

## CUT 0 — Baseline and safety gates

Before source movement:

- capture H1 public-surface snapshot,
- run declaration closure,
- run current ownership checks,
- run unit tests,
- run retained-composition/PERF-12 gates,
- run structural N-API/direct parity,
- confirm Iyon branch build workflow.

Do not block on a full architectural inventory.

---

## CUT 1 — Skeleton + zero/low-ambiguity moves

Create the target directories.

Move as coherent units:

```text
testing.ts / testing-access.ts
    → testing/

generated/
    → transport/abi/structural/generated/

bridge-schema.json
    → transport/abi/structural/schema/

values/
    → api/view/, api/presentation/, api/content/

traits/
    → api/extensions/traits/

errors.ts
    → api/errors.ts
```

Do not split small semantic files merely because the new directories exist.

Update generator and staging paths in this cut, not later.

At the end:

- root public surface is unchanged,
- declaration closure passes,
- generator/staging works,
- tests pass.

---

## CUT 2 — Atomic retained-core cut: composition + structural transport

Move the current retained core **as one coordinated cut** because the dependencies are entangled.

Composition candidates:

```text
compose.ts
define-view.ts
execution.ts
execution-context.ts
tracked-state.ts
child-owner.ts
persistent_seq.ts
internal-composition.ts
view-internals.ts
composition-related parts of tui-execution.ts
```

Structural transport candidates:

```text
ir.ts
retained_dag.ts
native_view_abi.ts
native_view_policy.ts
transport-related portions extracted from mixed files
```

Enforce the boundary while moving:

```text
composition
    retained semantic identity / execution / reuse

transport/structural
    lowering / materialization / bridge representation
```

This cut may split mixed files but must not rewrite algorithms.

Immediately run the complete PERF-12 non-regression suite.

Do not proceed with a broken retained core.

---

## CUT 3 — Atomic live-runtime/native/control cut

Resolve the second hard seam as one coordinated cut:

```text
tui.ts
runtime.ts
handles.ts
native.ts
native-handles.ts
component.ts
component-facade.ts
remaining tui-execution.ts
history.ts
text-input.ts
stream.ts
scroll-pane.ts
```

Classify using the normative ownership rules:

```text
public semantic control
    → api/controls/

live host/lifecycle/registry
    → runtime/

raw addon/native object/ID
    → transport/native/

semantic retained execution
    → composition/
```

This is intentionally atomic because controls, live runtime ownership, and native handles currently depend on one another.

Do not force a theoretical native split before inspecting the actual post-H1 code.

The desired outcome is a clean seam, not a particular number of files.

---

## CUT 4 — Root cleanup and ambiguous-module elimination

Once major domains are physically separated, resolve residue:

```text
types.ts
style-internals.ts
projectors.ts
scene.ts if not already moved
component forwarding residue
internal-composition residue
view-internals residue
generic runtime.ts residue
```

Goals:

- delete `types.ts` by moving concepts to owners,
- remove ambiguous `*-internals.ts` modules where possible,
- remove temporary forwarding modules,
- shrink `src/index.ts`,
- ensure no implementation soup remains at root.

---

## CUT 5 — Enforcement + integration

Add or strengthen checks for:

- semantic API importing generated ABI,
- public declarations referencing transport paths,
- production importing testing,
- composition importing live host/native lifecycle modules,
- bridge/native/generated symbols exported at root,
- external deep imports,
- duplicate module state caused by path aliases,
- accidental publication of future `state/` or `content/` placeholders.

Then run:

```text
iyon-tui tests
declaration-only emit
public surface snapshot
ownership checks
native staging
codegen verification
PERF-12 retained gates
N-API/direct structural parity
external consumer fixture
Iyon build against H2 branch
```

Use the established integration workflow:

```text
bun run build:iyon -- <h2-branch>
```

Do not edit checked-in Iyon pins solely for H2 testing.

---

# 21. Directional mapping of current files

This table is a default direction, not an up-front blocking inventory.

| Current path | Default owner |
|---|---|
| `bridge-schema.json` | `transport/abi/structural/schema/` |
| `generated/*` | `transport/abi/structural/generated/` |
| `values/view.ts` | `api/view/` |
| `values/geometry.ts` | `api/view/` |
| `scene.ts` | `api/view/` |
| `values/style.ts` | `api/presentation/` |
| `values/theme.ts` | `api/presentation/` |
| `values/theme-key.ts` | `api/presentation/` |
| `values/text.ts` | `api/content/` |
| `values/text-content.ts` | `api/content/` |
| `values/annotations.ts` | `api/content/` |
| `values/projection.ts` | `api/content/` |
| `values/diff.ts` | `api/content/` |
| `values/stream-snapshot.ts` | `api/content/` |
| `traits/*` | `api/extensions/traits/` if H1 keeps them public |
| `errors.ts` | `api/errors.ts` |
| `compose.ts` | `composition/` |
| `define-view.ts` | `composition/` |
| `execution.ts` | `composition/` unless host ownership is mixed in |
| `execution-context.ts` | `composition/` |
| `tracked-state.ts` | `composition/` |
| `child-owner.ts` | `composition/` |
| `persistent_seq.ts` | `composition/persistent-seq.ts` |
| `internal-composition.ts` | split/merge under composition or structural transport |
| `view-internals.ts` | split by semantic identity vs transport role |
| `ir.ts` | `transport/structural/` |
| `retained_dag.ts` | `transport/structural/` unless semantic-only portions are discovered |
| `native_view_abi.ts` | `transport/structural/` |
| `native_view_policy.ts` | `transport/structural/policy.ts` |
| `native.ts` | `transport/native/` |
| `native-handles.ts` | split runtime-lifetime vs raw-native |
| `handles.ts` | runtime unless H1 proves otherwise |
| `tui.ts` | runtime implementation, public re-export |
| `runtime.ts` | split/classify under runtime/composition/transport |
| `tui-execution.ts` | split semantic execution vs live-host ownership |
| `history.ts` | `api/controls/` |
| `text-input.ts` | `api/controls/` |
| `stream.ts` | `api/controls/` or H1-defined semantic content owner |
| `scroll-pane.ts` | `api/controls/` |
| `component.ts` | H1-dependent split |
| `component-facade.ts` | runtime/API/native split |
| `testing.ts` | `testing/` |
| `testing-access.ts` | `testing/` |
| `types.ts` | eliminate by owner |
| `style-internals.ts` | decompose by owner |
| `projectors.ts` | `api/content/` or `api/extensions/traits/` according to semantics |

---

# 22. PERF-13 architecture must remain visible during H2

H2 is immediately before PERF-13.

The source tree must encode the future three-plane split without implementing it.

## 22.1 Structural plane — already real

```text
api/view/
composition/
transport/structural/
transport/abi/structural/
```

This remains the PERF-12 structural architecture.

## 22.2 Retained state plane — PERF-13

Future semantic ownership:

```text
api/view/
    geometry vocabulary

api/presentation/
    presentation/theme/style vocabulary

runtime/
    retained handle/lifetime orchestration where required

transport/state/
    semantic state mutation crossing the TS/native boundary

transport/abi/state/
    only if a generated ABI is actually appropriate
```

H2 must not organize the code as though every geometry/presentation change will forever require structural View publication.

The long-term PERF-13 rule remains:

> If topology or semantic attachment identity did not change, the View DAG should normally not change.

Future Rust-owned retained state may include:

```text
geometry:
    padding
    gap
    bounds
    alignment
    border dimensions

presentation:
    foreground
    background
    border appearance
    semantic style state
    theme resolution

interaction:
    focus
    selected
    active
    future state-driven presentation
```

Rust—not TS—will determine whether a mutation means paint, placement, measure/layout, or structure.

H2 must merely give those concepts sane owners.

## 22.3 Content plane — PERF-13

Future semantic ownership:

```text
api/content/
    Source
    Funnel
    Connector
    ContentPort

runtime/
    lifecycle/registry where required

transport/content/
    content control + high-throughput data transport

transport/abi/content/
    only if a generated ABI is appropriate
```

The current content design direction is:

```text
Source
    ↓
Funnel
    ↓
Connector
    ↓
ContentPort
```

A port may eventually support:

```text
0..N connectors
0..1 active connector
```

PERF-13 implements only **cold** inactive connector semantics.

Future work may add:

```text
buffered
hot
priority/arbitration
capability fallback
Kitty/video/live surfaces
```

H2 must not bake in assumptions such as:

```text
View owns exactly one universal content payload

TextStream must forever be structural View IR

View ABI is the universal native boundary

all capability logic belongs in View
```

---

# 23. Why structural ABI gets its own namespace now

The current bridge schema is not the contract for all future TS/native communication.

It is the structural View contract.

That distinction matters because PERF-13 deliberately adds independent paths:

```text
STRUCTURAL
    retained View DAG

STATE
    retained geometry/presentation/interaction mutations

CONTENT
    retained content control + fast payload lane
```

Therefore the current generated code belongs at:

```text
transport/abi/structural/
```

not:

```text
transport/abi/generated/
```

as though it were universal.

This avoids a second H2-like move during PERF-13 and resists a future god-ABI.

---

# 24. Public API naming debt is not H2 work

An implementation review correctly noted that moving presentation modules does not fix any awkward H1 method naming.

That concern is valid but belongs elsewhere.

Rule:

```text
H1 semantic naming problem
    → H1 or separate API follow-up

H2
    → move the accepted semantics unchanged
```

H2 must stay reviewable as an architecture move.

---

# 25. Acceptance gates

## 25.1 Tree shape

At completion:

```text
src/
├── api/
├── composition/
├── runtime/
├── transport/
├── testing/
└── index.ts
```

Any additional root implementation file requires explicit justification.

## 25.2 Public surface

- H1 root API unchanged.
- `@iyon/tui/testing` separate.
- no transport/native/generated types in declarations.
- no deep imports required by Iyon.

## 25.3 Composition/runtime seam

- retained semantic execution does not own Tui host lifecycle.
- runtime owns live composition-engine instances.
- raw native access is not pulled into composition because of historical colocation.

## 25.4 Transport seam

- structural lowering under `transport/structural/`.
- structural schema/generated ABI under `transport/abi/structural/`.
- raw addon access under `transport/native/`.
- future state/content ownership slots remain non-public and unimplemented.

## 25.5 PERF-12

All retained identity, reuse, lifetime, and parity tests pass.

## 25.6 Tooling

- generator emits to new location,
- staging/build scripts use new paths,
- declarations pass,
- package tests pass,
- fixture passes,
- Iyon branch build passes,
- ownership checks enforce the new architecture.

---

# 26. Performance gate

H2 should compile to equivalent work.

Representative workloads must show no meaningful increase in:

```text
scope executions
View constructions
NodeId creation
structural materialization
bridge traffic
native calls
retained DAG work
```

Investigate any regression for:

```text
duplicate module singletons
path-alias duplication
new hot-path wrappers
changed initialization ordering
broken cache identity
semantic object reconstruction
generator/staging mistakes
```

Do not accept a regression as "just a refactor."

---

# 27. Non-goals checklist

H2 must not accidentally:

- [ ] redesign `View`,
- [ ] change `State<T>` scheduling,
- [ ] change `defineView` identity,
- [ ] change keyed/unkeyed child identity,
- [ ] change `ViewSlot` builder retention,
- [ ] change `ScrollPane` builder retention,
- [ ] change direct-render takeover,
- [ ] change H1 theme semantics,
- [ ] rename H1 public API opportunistically,
- [ ] change control lifecycle,
- [ ] change structural bridge semantics,
- [ ] change structural ABI records,
- [ ] implement retained mutable geometry,
- [ ] implement retained mutable presentation,
- [ ] implement retained interaction state,
- [ ] implement Source/Funnel/Connector/ContentPort,
- [ ] implement cold/buffered/hot behavior,
- [ ] implement content FFI,
- [ ] implement Kitty/Sixel/video,
- [ ] build a content scheduler,
- [ ] retain forwarding modules as a permanent second architecture.

---

# 28. Completion report required from the implementation agent

The final H2 report must contain:

1. final `src/` tree;
2. deleted root modules;
3. files split because they crossed ownership boundaries;
4. final `composition/` vs `runtime/` boundary;
5. final public-handle / runtime-lifetime / raw-native split;
6. structural schema/generated ABI relocation confirmation;
7. ownership/import checks added;
8. declaration closure result;
9. PERF-12 non-regression result;
10. structural N-API/direct parity result;
11. Iyon integration result;
12. remaining debt explicitly deferred to PERF-13 or another API tranche.

"Imports updated and tests pass" is not a sufficient completion report.

H2 is an architecture task; the report must show that ownership improved.

---

# 29. Final directive

Preserve the core H2 model:

```text
api/
    what the framework means

composition/
    how semantic retained identity/reuse works

runtime/
    how live framework instances are owned and orchestrated

transport/
    how the three semantic planes cross to Rust

testing/
    how tests inject and inspect
```

Execute it pragmatically:

```text
baseline
    ↓
easy subtree moves
    ↓
composition + structural transport atomic cut
    ↓
runtime + native + controls atomic cut
    ↓
root cleanup
    ↓
enforcement + integration
```

Do not perfectly classify the entire repository before moving anything.

Do not create dozens of tiny files merely because directories now exist.

Do not leave the structural bridge schema in a generic universal ABI bucket.

Do not let composition own Tui host lifecycle.

Do not let runtime become the next dumping ground.

Keep PERF-13's architecture visible:

```text
           STRUCTURAL DAG
                 │
       ┌─────────┴─────────┐
       │                   │
 RETAINED STATE        CONTENT
       │                   │
       └─────────┬─────────┘
                 ▼
               Rust
```

H2 succeeds when PERF-13 can add the state and content planes into obvious existing ownership boundaries **without reorganizing the TypeScript tree again first**.
