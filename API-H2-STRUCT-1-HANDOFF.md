# API-H2 / STRUCT-1 — TypeScript Source Architecture Cleanup

**Repository:** `alexykn/iyon-tui`  
**Sequence:** `PERF-12 → API-H1 → API-H2 / STRUCT-1 → PERF-13`  
**Primary scope:** `packages/iyon-tui/src/**`  
**Nature of work:** source-layout, module-ownership, naming, import-boundary, and export-topology cleanup  
**Semantic intent:** **no behavior change**

---

## 0. Executive directive

API-H2 exists to make the TypeScript source tree reflect the architecture that already exists after PERF-12 and API-H1, before PERF-13 adds a retained mutable state plane and a retained content plane.

The current flat `packages/iyon-tui/src/` layout mixes:

- public semantic API,
- public framework-owned controls,
- TypeScript retained composition,
- runtime/lifecycle machinery,
- structural bridge/transport lowering,
- generated ABI code,
- native bindings,
- test-only access,
- implementation internals,

in one directory.

That is now an architectural liability.

H2 must reorganize the tree around five explicit ownership domains:

```text
src/
├── api/
├── composition/
├── runtime/
├── transport/
├── testing/
└── index.ts
```

The resulting filesystem should answer, at a glance:

```text
api/          what the framework means
composition/  how TypeScript retains semantic structure
runtime/      host lifecycle/control and framework-owned runtime machinery
transport/    how semantic planes cross the TS/native boundary
testing/      test-only public and internal test support
```

H2 is **not** a feature tranche. It must not redesign semantics simply because moving a file exposes an awkward implementation.

If H2 changes observable behavior, retained identity, layout semantics, transport semantics, lifecycle semantics, or performance characteristics for equivalent workloads, H2 has failed.


# 1. Why H2 exists

After PERF-12 and API-H1, the source tree is expected to contain a coherent architecture but still expose a poor physical layout.

Representative pre-H2 shape:

```text
packages/iyon-tui/src/
├── bridge-schema.json
├── child-owner.ts
├── component-facade.ts
├── component.ts
├── compose.ts
├── define-view.ts
├── errors.ts
├── execution-context.ts
├── execution.ts
├── generated/
├── handles.ts
├── history.ts
├── index.ts
├── internal-composition.ts
├── ir.ts
├── native_view_abi.ts
├── native_view_policy.ts
├── native-handles.ts
├── native.ts
├── persistent_seq.ts
├── projectors.ts
├── retained_dag.ts
├── runtime.ts
├── scene.ts
├── scroll-pane.ts
├── stream.ts
├── style-internals.ts
├── testing-access.ts
├── testing.ts
├── text-input.ts
├── tracked-state.ts
├── traits/
├── tui-execution.ts
├── tui.ts
├── types.ts
├── values/
└── view-internals.ts
```

The problem is not file count. The problem is that unrelated architectural layers sit at the same namespace level.

Examples:

- `values/view.ts` and `native_view_abi.ts` appear equally fundamental.
- `retained_dag.ts` sits beside `history.ts`.
- `generated/` sits beside public semantic modules.
- `testing.ts` sits beside runtime production modules.
- `types.ts` becomes a cross-domain dumping ground.
- `component.ts` and `component-facade.ts` are not self-explanatory.
- `runtime.ts`, `tui.ts`, and `tui-execution.ts` encode overlapping concepts without directory ownership clarifying the distinction.
- private transport records and public semantic values are physically adjacent.
- agents browsing the codebase are exposed to implementation seams before they understand the semantic framework.

H2 fixes the physical architecture before PERF-13 introduces additional concepts such as:

- retained geometry state,
- retained presentation state,
- retained interaction state,
- content ports,
- sources,
- funnels,
- connectors,
- new state transport,
- new content data transport.

Without H2, PERF-13 would amplify the current flat layout.


# 2. H2 positioning

The required sequence is:

```text
PERF-12
    retained structural transport and composition are proven
        ↓
API-H1
    public semantic surface is cleaned and closed
        ↓
API-H2 / STRUCT-1
    physical TypeScript source architecture is cleaned
        ↓
PERF-13
    structural plane + retained state plane + content plane
```

H2 must consume the decisions from API-H1 rather than second-guessing them.

In particular:

- H1 decides what is public.
- H1 decides public semantic names.
- H1 resolves accidental bridge/native leakage.
- H1 resolves control lifecycle/public construction truthfulness.
- H1 resolves output/event naming.
- H1 establishes `@iyon/tui/testing`.
- H1 removes or resolves false aliases.
- H2 moves those resulting concepts into coherent ownership domains.

Do **not** use H2 to reopen already-settled H1 public API decisions unless moving the code exposes a concrete correctness defect.


# 3. Hard invariants

## 3.1 PERF-12 retained architecture is protected

H2 must not weaken, bypass, replace, or duplicate the retained TypeScript composition path.

Protected implementation concepts include the existing equivalents of:

```text
compose.ts
define-view.ts
execution.ts
execution-context.ts
tracked-state.ts
child-owner.ts
persistent_seq.ts
retained_dag.ts
native_view_abi.ts
```

The exact paths will change. Their semantics must not.

Protected behavior includes:

- stable `NodeId` semantics,
- unchanged subtree identity preservation,
- semantic cutoff on retained hits,
- `defineView` execution-scope identity,
- `State<T>` subscription ownership,
- keyed child-owner identity,
- unkeyed occurrence identity,
- builder-root ownership,
- direct-render takeover semantics,
- builder/direct ownership transitions,
- `ViewSlot` retained builder ownership,
- `ScrollPane` retained builder ownership,
- animation ownership transitions,
- `PersistentSeq` wide-axis behavior,
- retained axis/grid derivations,
- retained path edits,
- multi-edit transactions,
- `NativeRef` hints,
- semantic caches,
- root leases,
- temporary materialization leases,
- release batching,
- cold recovery behavior,
- runtime/environment generation behavior,
- N-API/direct-FFI structural semantic parity.

The canonical structural path remains conceptually:

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

H2 may relocate modules participating in that path. It may not redesign it.

## 3.2 No PERF-13 implementation

H2 may prepare names and directories for PERF-13, but it must not implement PERF-13.

Do not add:

- retained mutable geometry handles,
- retained mutable presentation handles,
- retained interaction-state handles,
- content ports,
- content sources,
- funnels,
- connectors,
- cold/buffered/hot standby behavior,
- state-plane mutation ABI,
- content-plane FFI data path,
- Kitty/Sixel/video support,
- new damage propagation,
- new layout invalidation semantics,
- property-level state bindings.

Empty directories or documented ownership boundaries for future `transport/state` and `transport/content` are acceptable only if repository conventions permit them. Do not add fake placeholder implementations merely to make the target tree visually complete.

## 3.3 No semantic cleanup disguised as movement

Allowed:

- move file,
- split file by existing responsibilities,
- merge files that are only accidental wrappers,
- rename private implementation symbols/files,
- update imports,
- update exports,
- relocate generated files,
- relocate codegen schema,
- delete dead forwarding modules after proving no use,
- eliminate `types.ts` by moving existing types to their owners.

Not allowed without separate justification:

- change how a View is constructed,
- change how retained identity is assigned,
- change how controls are created/disposed,
- change rendering semantics,
- change bridge representation,
- change structural ABI,
- change scheduling,
- change performance-sensitive retained algorithms,
- change current public API beyond H1-approved results.


# 4. Target source architecture

```text
packages/iyon-tui/src/
├── api/
│   ├── view/
│   ├── presentation/
│   ├── content/
│   ├── controls/
│   ├── extensions/
│   └── errors.ts
│
├── composition/
│   ├── compose.ts
│   ├── define-view.ts
│   ├── execution.ts
│   ├── execution-context.ts
│   ├── tracked-state.ts
│   ├── child-owner.ts
│   ├── persistent-seq.ts
│   └── internals.ts
│
├── runtime/
│   ├── tui.ts
│   ├── handles.ts
│   ├── interaction.ts
│   ├── events.ts
│   └── ...
│
├── transport/
│   ├── structural/
│   │   ├── ir.ts
│   │   ├── retained-dag.ts
│   │   ├── policy.ts
│   │   └── lowering.ts
│   │
│   ├── state/             # PERF-13 fills this
│   ├── content/           # PERF-13 fills this
│   ├── native/
│   │   └── ...
│   └── abi/
│       ├── schema/
│       └── generated/
│
├── testing/
│   ├── index.ts
│   ├── harness.ts
│   └── access.ts
│
└── index.ts
```

This is an ownership model, not a requirement that every directory contain exactly these filenames.

Prefer fewer meaningful files over reproducing the current fragmentation inside deeper folders.


# 5. Top-level dependency direction

```text
                         api/
                          │
              ┌───────────┴───────────┐
              │                       │
              ▼                       ▼
        composition/               runtime/
              │                       │
              └───────────┬───────────┘
                          ▼
                      transport/
                          │
                          ▼
                         native
```

`testing/` may depend on public API and explicitly allowed internal testing seams.

Rules:

- `api/` owns semantic concepts, not bridge representation.
- `composition/` owns semantic View identity/reuse/execution.
- `runtime/` owns live host/lifecycle/control machinery.
- `transport/` owns boundary lowering and native representation.
- `testing/` owns harness, injection, and inspection support.
- production directories must not depend on `testing/`.
- avoid `shared/`, `common/`, `misc/`, or generic `utils/` as escape hatches for unclear ownership.


# 6. `api/` ownership

`api/` contains concepts framework users are expected to understand.

A useful test:

> If the concept belongs in user-facing API documentation, it probably belongs under `api/`.

A second test:

> If the file name contains `native`, `bridge`, `abi`, `materialize`, `retained`, or transport-specific terminology, it probably does not belong under `api/`.

## 6.1 `api/view/`

Own structural semantic View concepts.

Expected candidates:

```text
values/view.ts
scene.ts
values/geometry.ts
```

plus H1-resolved structural component-facing concepts.

Possible target:

```text
api/view/
├── view.ts
├── scene.ts
├── geometry.ts
├── grid.ts
├── border.ts
└── component.ts
```

Only create files justified by the post-H1 concepts. Do not manufacture layers.

Do not put native component adapters, retained DAG machinery, or bridge `*Node` records here.

## 6.2 `api/presentation/`

Own semantic styling/theming/presentation concepts.

Expected candidates:

```text
values/style.ts
values/theme.ts
values/theme-key.ts
style-internals.ts    # contents only; this file name should disappear
```

Possible H1 semantic concepts:

```text
ColorSpec
ThemeColor
AnsiColor
Style
StyleSpec
StyleRef
Theme
ThemeKey
StyleSelector
```

Private lowering helpers belong either beside their semantic owner if purely local, or in transport if they encode bridge representation.

## 6.3 `api/content/`

Establish the semantic content domain before PERF-13 without inventing PERF-13 content-plane types.

Current candidates:

```text
values/text-content.ts
values/text.ts
values/annotations.ts
values/projection.ts
values/diff.ts
values/stream-snapshot.ts
projectors.ts
```

Future PERF-13 may add `source`, `funnel`, `connector`, and `port`; H2 must not implement them.

## 6.4 `api/controls/`

Own public framework controls:

```text
History
TextInput
TextStream
ViewSlot
ScrollPane
```

Use semantic names, not implementation aliases such as `NativeViewSlot`.

## 6.5 `api/extensions/`

Own user-implementable behavioral contracts if H1 confirms they are genuine extension points.

Likely current candidates:

```text
traits/component.ts
traits/projector.ts
traits/renderer.ts
traits/streaming-source.ts
traits/text-rewriter.ts
traits/text-visitor.ts
```

Do not move adapter residue here merely because it currently sits under `traits/`.

## 6.6 `api/errors.ts`

Own public error taxonomy such as `TuiError` and public helpers. Native error decoding remains private.


# 7. `composition/` ownership

`composition/` is the TypeScript retained semantic composition engine established by T13.1/PERF-12.

It owns:

- semantic View occurrence identity,
- execution scopes,
- `State<T>` dependency tracking,
- keyed/unkeyed child ownership,
- retained semantic composition,
- persistent child sequences,
- builder-root execution semantics,
- composition-local metadata.

Current candidates:

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
tui-execution.ts      # classify before moving
```

Initial target:

```text
composition/
├── compose.ts
├── define-view.ts
├── execution.ts
├── execution-context.ts
├── tracked-state.ts
├── child-owner.ts
├── persistent-seq.ts
└── internals.ts
```

Do not overnest immediately.

### `internal-composition.ts` / `view-internals.ts`

Do not preserve ambiguous `internals` modules automatically.

Possible outcomes:

- merge into `compose.ts`,
- merge into a narrow `composition/internals.ts`,
- split into precisely named files,
- move transport-specific portions to `transport/structural`,
- move semantic View portions to `api/view`.

### `tui-execution.ts`

Classify by responsibility.

If it owns host/runtime execution lifecycle, it belongs under `runtime/`.

If it owns semantic builder execution and retained scopes, it belongs under `composition/`.

If mixed, split it without changing behavior.


# 8. `runtime/` ownership

`runtime/` owns the live framework host and lifecycle machinery:

```text
Tui host
runtime lifecycle
framework-owned handle lifecycle
event/input routing
control registration
host-side execution ownership
```

Current candidates:

```text
tui.ts
runtime.ts
handles.ts
native-handles.ts       # classify carefully
tui-execution.ts        # possibly
component-facade.ts     # possibly
interaction/event code
```

Potential target:

```text
runtime/
├── tui.ts
├── handles.ts
├── interaction.ts
├── events.ts
├── execution.ts
└── ...
```

## 8.1 Reconcile `tui.ts`, `runtime.ts`, `tui-execution.ts`

Do not mechanically move all three and retain ambiguity.

For each responsibility, identify whether it belongs to:

1. public semantic `Tui`,
2. runtime implementation,
3. retained composition execution,
4. raw native access.

Desired result:

- one obvious owner for public `Tui`,
- one obvious owner for Tui host/runtime implementation,
- composition execution remains under `composition/`,
- raw native operations live under `transport/`.

## 8.2 Handles

Physically separate semantic/public handles from runtime lifecycle and raw native handles.

Example:

```text
api/controls/text-input.ts
    public TextInput semantic handle

runtime/handles.ts
    common framework handle lifecycle

transport/native/handles.ts
    raw native IDs/contracts/unwrapping
```


# 9. `transport/` ownership

`transport/` contains boundary/lowering machinery, not semantic user API.

Long-term shape:

```text
transport/
├── structural/
├── state/
├── content/
├── native/
└── abi/
```

For H2:

- `structural/` is real and populated from PERF-12.
- `state/` is reserved for PERF-13.
- `content/` is reserved for PERF-13.
- `native/` owns raw addon access/native-only contracts.
- `abi/` owns schema/generated low-level protocol definitions.

The planes themselves are not transports. This directory only owns how those planes cross the boundary.

## 9.1 `transport/structural/`

Home of the current retained View/DAG bridge.

Current candidates:

```text
ir.ts
retained_dag.ts
native_view_abi.ts
native_view_policy.ts
```

Target concept:

```text
semantic retained View frontier
        ↓
structural lowering
        ↓
canonical View ABI
```

`composition/` decides semantic identity/reuse.  
`transport/structural/` lowers the retained semantic result into native structural operations.

Do not conflate these simply because both contain retained concepts.

Likely moves:

```text
retained_dag.ts        → transport/structural/retained-dag.ts
native_view_policy.ts  → transport/structural/policy.ts
ir.ts                  → transport/structural/ir.ts
```

Rename/split `native_view_abi.ts` according to actual responsibility, e.g. `lowering.ts`, `abi.ts`, or `materialize.ts`.

## 9.2 `transport/state/`

Reserved for PERF-13 retained state-plane transport.

Future examples:

```text
geometry mutation lowering
presentation mutation lowering
interaction/style-state mutation lowering
```

H2 must not implement it.

## 9.3 `transport/content/`

Reserved for PERF-13 content-plane transport.

Future examples:

```text
Source/Funnel/Connector/Port control lowering
UTF-8 append/replace data lane
fast FFI content ABI
```

H2 must not implement it.

## 9.4 `transport/native/`

Own raw addon access and implementation-only native contracts.

Likely candidates:

```text
native.ts
native-handles.ts
remaining Native* private contracts
```

Possible target:

```text
transport/native/
├── addon.ts
├── handles.ts
├── contracts.ts
└── ...
```

## 9.5 `transport/abi/`

Generated/schema code must be buried here or under the structural transport if the schema is strictly structural.

Preferred generic shape:

```text
transport/abi/
├── schema/
│   └── bridge-schema.json
└── generated/
    ├── view-abi-conformance.ts
    ├── view-abi-manifest.json
    ├── view-abi.ts
    ├── view-calls.ts
    └── view-materialize.ts
```

Generated ABI implementation must never again appear as a peer of semantic public API files.


# 10. Generated code and schema rules

Generated files are immutable outputs.

H2 must:

1. move the generator output location,
2. update generator configuration/templates,
3. regenerate,
4. verify semantics are unchanged aside from path/import changes,
5. update staging/build scripts,
6. update ownership checks,
7. remove temporary forwarding modules before completion unless a real compatibility contract requires them.

Do not manually edit generated output to fix path breakage.

Fix the generator or source schema.

## `bridge-schema.json`

Classify by actual generator ownership.

If strictly structural:

```text
transport/structural/schema/bridge-schema.json
```

If it is a cross-plane ABI schema intended to grow with PERF-13:

```text
transport/abi/schema/bridge-schema.json
```

Inspect generator inputs, Rust generation use, staging scripts, and CI before choosing.


# 11. `testing/` ownership

After H1, public testing utilities should be exported from:

```text
@iyon/tui/testing
```

Mirror that physically:

```text
testing/
├── index.ts
├── harness.ts
├── access.ts
├── events.ts
└── inspection.ts
```

Only create files justified by actual responsibilities.

Current candidates:

```text
testing.ts
testing-access.ts
AppHarness
createAppHarness
input injection
clock advancement
screen/style/cell inspection
```

Production code must never import from `testing/`.

Use deliberately tiny internal test seams rather than broad production exports.


# 12. Root exports and removal of dumping-ground modules

## 12.1 `src/index.ts`

Remain the curated semantic root export map.

It must not export:

- transport internals,
- generated ABI,
- raw native contracts,
- retained transport internals,
- private composition machinery,
- testing utilities.

Testing is a separate package export.

## 12.2 Eliminate `types.ts`

H2 should aim to delete root `types.ts`.

Move types to their semantic owners.

Examples:

```text
TuiEvent / output events
    → runtime/events.ts or H1-approved owner

SceneProducer
    → api/view/scene.ts

Renderer
    → api/extensions/renderer.ts

Projector
    → api/extensions/projector.ts

content snapshot types
    → api/content/

control options
    → owning control module
```

Do not replace it with `shared/types.ts`, `common/types.ts`, or another dumping ground.

## 12.3 Resolve `component.ts` / `component-facade.ts`

H1 decides the semantic component model. H2 must encode that decision physically.

Possible split:

```text
api/view/component.ts
api/extensions/component.ts
runtime/component-host.ts
transport/native/component.ts
```

Do not preserve ambiguous peer naming.

## 12.4 Resolve `style-internals.ts`

This file name should disappear.

Move each responsibility to:

- `api/presentation/`,
- its owning semantic module,
- `transport/structural/`,
- `transport/abi/`,
- runtime,

as appropriate.

## 12.5 Resolve `native-handles.ts`

Expected split:

```text
runtime/handles.ts
transport/native/handles.ts
```

No public declaration may name the raw native handle types.


# 13. Naming conventions

Normalize handwritten source to kebab-case unless a stronger repository convention exists.

Examples:

```text
persistent_seq.ts      → persistent-seq.ts
retained_dag.ts        → retained-dag.ts
native_view_policy.ts  → policy.ts under transport/structural
```

Directory ownership should let filenames become shorter and more precise.

Prefer:

```text
transport/structural/policy.ts
```

over:

```text
transport/structural/native-view-policy.ts
```

Generated file naming may retain generator conventions if changing it adds noise without architectural value.


# 14. Import rules

H2 should add or strengthen machine-enforced ownership rules.

## Rule A — semantic API cannot import generated ABI directly

Preferred:

```text
src/api/**  -X->  src/transport/abi/generated/**
```

Target is zero direct imports.

## Rule B — semantic API should not import raw native contracts directly

Prefer mediation through runtime/private semantic boundaries.

Any exception must be explicit and narrow.

## Rule C — production cannot import testing

Always forbidden.

## Rule D — composition cannot depend on host/runtime details unnecessarily

The retained composition engine must remain about semantic identity and execution, not Tui host implementation.

## Rule E — structural transport owns bridge IR

No `api/**` module should import private bridge `*Node` records.

## Rule F — no external deep imports

Iyon and the consumer fixture import only:

```text
@iyon/tui
@iyon/tui/testing
```

No source-layout path is a public contract.

## Rule G — avoid internal barrel proliferation

Keep:

```text
src/index.ts
testing/index.ts
```

as intentional barrels.

Prefer direct internal imports elsewhere so dependency ownership remains visible.


# 15. H2 phased implementation plan

## H2A — Inventory and classification

Before broad moves, create a complete table for every current source file:

```text
current path
semantic owner
target path
public/private/generated
dependencies
dependents
move/split/merge/delete
```

Required classifications:

```text
api/view
api/presentation
api/content
api/controls
api/extensions
composition
runtime
transport/structural
transport/native
transport/abi
testing
delete/merge
```

Resolve every `unclear` item before H2B.

## H2B — Move H1-stabilized public semantic modules

Move:

```text
api/view
api/presentation
api/content
api/controls
api/extensions
api/errors.ts
```

Keep root exports stable.

Run declaration/public-surface gates.

## H2C — Move composition subsystem

Move protected T13.1/PERF-12 composition machinery into `composition/`.

No algorithmic refactor.

Split only where necessary to establish the runtime/composition boundary.

Immediately run retained-composition tests.

## H2D — Reconcile runtime ownership

Resolve and classify:

```text
tui.ts
runtime.ts
tui-execution.ts
handles.ts
native-handles.ts
component-facade.ts
```

Move host/lifecycle code to runtime, raw native contracts to transport/native, semantic composition logic to composition.

## H2E — Move structural transport

Move current PERF-12 structural transport into:

```text
transport/structural/
```

Update imports only.

Do not change structural ABI semantics.

Run structural/T15 parity tests.

## H2F — Move native seam, schema, and generated ABI

Move:

```text
native access      → transport/native/
generated output   → transport/abi/generated/
schema             → chosen transport schema owner
```

Update generators, staging scripts, build scripts, package scripts, CI, and ownership checks.

Regenerate; do not hand-edit generated output.

## H2G — Move testing

Create the source backing for:

```text
@iyon/tui/testing
```

Move harness/injection/inspection support.

Ensure root `@iyon/tui` contains no test surface.

## H2H — Eliminate ambiguous root files

Resolve/delete or precisely rename:

```text
types.ts
style-internals.ts
component-facade.ts
internal-composition.ts
view-internals.ts
runtime.ts
```

Goal: no miscellaneous implementation architecture remains at `src/` root.

## H2I — Enforce ownership

Add machine checks for:

- no semantic API → generated ABI imports,
- no public declarations referencing transport paths,
- no production → testing imports,
- no external deep imports,
- no bridge/native/generated root exports,
- no accidental public future PERF-13 placeholders.

## H2J — Consumer/build validation

Run:

```text
iyon-tui tests
declaration-only emit
public-surface snapshot
ownership checks
external consumer fixture
native staging
PERF-12 semantic parity/oracle tests
Iyon integration build
```

For Iyon:

```text
bun run build:iyon -- <h2-branch>
```

Do not modify checked-in Iyon dependency pins merely to test H2.

## H2K — Final source-tree audit

The final root should be essentially:

```text
api/
composition/
runtime/
transport/
testing/
index.ts
```

Any other root implementation file requires explicit justification.


# 16. Proposed current-file mapping

This is prescriptive in direction but must be checked against the post-H1 source.

| Current path | Target ownership | Likely target |
|---|---|---|
| `bridge-schema.json` | transport ABI/schema | `transport/abi/schema/bridge-schema.json` or structural schema |
| `child-owner.ts` | composition | `composition/child-owner.ts` |
| `component-facade.ts` | split/classify | runtime or API semantic owner |
| `component.ts` | H1-dependent | view/control/extension split |
| `compose.ts` | composition | `composition/compose.ts` |
| `define-view.ts` | composition | `composition/define-view.ts` |
| `errors.ts` | public API | `api/errors.ts` |
| `execution-context.ts` | composition | `composition/execution-context.ts` |
| `execution.ts` | composition | `composition/execution.ts` |
| `generated/*` | generated ABI | `transport/abi/generated/*` |
| `handles.ts` | runtime | `runtime/handles.ts` |
| `history.ts` | public control | `api/controls/history.ts` |
| `index.ts` | public root | `index.ts` |
| `internal-composition.ts` | composition/split | merge/split under `composition/` |
| `ir.ts` | structural transport | `transport/structural/ir.ts` |
| `native_view_abi.ts` | structural transport | `transport/structural/lowering.ts` / `abi.ts` |
| `native_view_policy.ts` | structural transport | `transport/structural/policy.ts` |
| `native-handles.ts` | native/runtime split | `transport/native/handles.ts` + runtime part if needed |
| `native.ts` | raw native seam | `transport/native/addon.ts` or `native.ts` |
| `persistent_seq.ts` | composition | `composition/persistent-seq.ts` |
| `projectors.ts` | content/extension | post-H1 semantic owner |
| `retained_dag.ts` | structural transport | `transport/structural/retained-dag.ts` |
| `runtime.ts` | runtime/split | specific module(s) under `runtime/` |
| `scene.ts` | view API | `api/view/scene.ts` |
| `scroll-pane.ts` | public control | `api/controls/scroll-pane.ts` |
| `stream.ts` | public control/content classify | likely `api/controls/text-stream.ts` |
| `style-internals.ts` | split | presentation or transport owner |
| `testing-access.ts` | testing | `testing/access.ts` |
| `testing.ts` | testing | split into `testing/index.ts`, `harness.ts`, etc. |
| `text-input.ts` | public control | `api/controls/text-input.ts` |
| `tracked-state.ts` | composition | `composition/tracked-state.ts` |
| `traits/component.ts` | extension if public | `api/extensions/component.ts` |
| `traits/projector.ts` | extension | `api/extensions/projector.ts` |
| `traits/renderer.ts` | extension | `api/extensions/renderer.ts` |
| `traits/streaming-source.ts` | extension | `api/extensions/streaming-source.ts` |
| `traits/text-rewriter.ts` | extension | `api/extensions/text-rewriter.ts` |
| `traits/text-visitor.ts` | extension | `api/extensions/text-visitor.ts` |
| `tui-execution.ts` | classify/split | `composition/` and/or `runtime/` |
| `tui.ts` | public/runtime | likely `runtime/tui.ts`, re-exported publicly |
| `types.ts` | eliminate | move types to owners |
| `values/annotations.ts` | content API | `api/content/annotations.ts` |
| `values/diff.ts` | content API | `api/content/diff.ts` |
| `values/geometry.ts` | view API | `api/view/geometry.ts` |
| `values/projection.ts` | content API | `api/content/projection.ts` |
| `values/stream-snapshot.ts` | content API | `api/content/stream-snapshot.ts` |
| `values/style.ts` | presentation API | `api/presentation/style.ts` |
| `values/text-content.ts` | content API | `api/content/text-content.ts` |
| `values/text.ts` | content API | `api/content/text.ts` |
| `values/theme-key.ts` | presentation API | `api/presentation/theme-key.ts` |
| `values/theme.ts` | presentation API | `api/presentation/theme.ts` |
| `values/view.ts` | view API | `api/view/view.ts` |
| `view-internals.ts` | split/classify | composition or structural transport |


# 17. PERF-13 preparation requirements

H2 must leave obvious homes for the three PERF-13 planes without implementing them.

```text
STRUCTURAL PLANE

api/view/
composition/
transport/structural/


RETAINED STATE PLANE — PERF-13

api/presentation/          semantic vocabulary
runtime/...                retained handle/lifecycle machinery where needed
transport/state/           mutation boundary


CONTENT PLANE — PERF-13

api/content/
runtime/...                retained content lifecycle where needed
transport/content/         control/data boundary
```

Important:

> The planes are semantic architectural domains. `transport/*` is only how each plane crosses the native boundary.

Do not put all future state/content implementation into transport.

H2 must not encode assumptions that make these future concepts difficult:

- singular structural DAG with non-structural retained mutations,
- retained geometry state,
- retained presentation state,
- retained interaction/style state,
- a structural content-port attachment,
- multiple connectors per port,
- zero or one active connector,
- cold connector semantics,
- future buffered/hot standby,
- Source sharing across connectors,
- Funnel capability negotiation,
- backend capability negotiation,
- separate content FFI data plane,
- separate structural/state/content transport.

Avoid future-hostile assumptions such as:

```text
View owns exactly one content payload
View ABI is the universal native boundary
all runtime state belongs in structural IR
all native operations belong in native_view_abi.ts
```


# 18. H2 completion gates

## Source architecture

- `src/` root is reduced to architectural directories and root export files.
- public semantic concepts live under `api/`.
- retained semantic composition lives under `composition/`.
- Tui host/lifecycle machinery lives under `runtime/`.
- structural native lowering lives under `transport/structural/`.
- raw native access lives under `transport/native/`.
- generated ABI is buried under `transport/abi/generated/`.
- test-only code lives under `testing/`.
- `types.ts` is gone or has an exceptional documented reason.
- ambiguous root `*-internals.ts` files are gone or precisely relocated/renamed.

## Public surface

- root public API is unchanged from H1 except separately approved cleanup.
- `@iyon/tui/testing` remains the only testing subpath.
- no generated/native/bridge/retained transport type leaks into `.d.ts`.
- no deep import is necessary for Iyon.

## Import ownership

- production does not depend on testing.
- semantic API does not directly depend on generated ABI.
- bridge IR does not leak into public semantic API.
- composition does not depend on app code.
- structural transport does not own user-facing semantic abstractions.

## PERF-12 behavior

- structural retained identity tests pass.
- dirty-scope cutoff remains unchanged.
- keyed/unkeyed identity remains unchanged.
- `ViewSlot` builder retention remains unchanged.
- `ScrollPane` builder retention remains unchanged.
- `PersistentSeq` behavior remains unchanged.
- multi-edit transactions remain unchanged.
- lease/NativeRef convergence remains unchanged.
- N-API/direct-FFI structural parity remains unchanged.

## Build/tooling

- generators emit to new locations.
- native staging uses new paths.
- declarations build.
- package builds.
- tests pass.
- consumer fixture passes.
- Iyon builds against the H2 branch.
- ownership checks enforce the new architecture.


# 19. Performance gate

H2 is intended to be operationally neutral.

Representative retained workloads before and after H2 should show no meaningful increase in:

```text
scope executions
semantic View constructions
NodeId creation
structural materialization
bridge traffic
native calls
retained DAG work
```

Moving modules should compile away.

If performance changes materially, investigate before acceptance.

Likely accidental causes:

- duplicate singleton/module state after a split,
- path alias creating duplicate module instances,
- new wrappers allocating on hot paths,
- changed initialization ordering,
- semantic facade reconstruction,
- cache identity changes,
- staging/build path mistakes.


# 20. Review guidance for coding agents

When deciding where a symbol belongs, ask in order:

1. **Is this part of the user-facing mental model?**  
   → `api/`

2. **Does it determine semantic View identity/reuse/execution?**  
   → `composition/`

3. **Does it own live Tui host/lifecycle/control behavior?**  
   → `runtime/`

4. **Does it encode/lower/cross the TypeScript/native boundary?**  
   → `transport/`

5. **Is it exclusively for harnesses, injection, inspection, or tests?**  
   → `testing/`

If the answer is two categories at once, the current file probably mixes responsibilities and should be split.

Do not resolve uncertainty by creating:

```text
shared/
common/
utils/
misc/
internals/
```

as new dumping grounds.


# 21. Expected result

The final TypeScript tree should visually communicate the architecture without prior repository knowledge.

A developer or coding agent should infer:

```text
api/
    framework concepts and public contracts

composition/
    retained TypeScript semantic View composition

runtime/
    live Tui runtime and framework-owned lifecycle

transport/
    native-boundary implementation
    ├── structural
    ├── state      future PERF-13
    ├── content    future PERF-13
    ├── native
    └── abi

testing/
    test-only surface
```

The cleanup should make PERF-13 obvious rather than forcing PERF-13 to first rediscover source ownership.


# 22. Non-goals checklist

Before accepting any H2 change, verify it is **not** accidentally:

- [ ] redesigning `View`,
- [ ] changing structural identity semantics,
- [ ] changing `State<T>` scheduling,
- [ ] changing builder ownership,
- [ ] changing `ScrollPane` behavior,
- [ ] changing `ViewSlot` behavior,
- [ ] changing H1 theme semantics,
- [ ] changing H1 control lifecycle semantics,
- [ ] changing structural ABI representation,
- [ ] replacing PERF-12 retained DAG behavior,
- [ ] implementing retained mutable geometry,
- [ ] implementing retained mutable presentation,
- [ ] implementing retained interaction state,
- [ ] implementing content ports/sources/funnels/connectors,
- [ ] implementing cold/buffered/hot connector behavior,
- [ ] implementing content FFI,
- [ ] implementing Kitty/video,
- [ ] preserving obsolete modules solely as permanent compatibility aliases.

Temporary forwarding modules are acceptable during staged migration but should be removed before H2 completes unless a real external compatibility contract requires them.


# 23. Final directive

API-H2 should leave `iyon-tui` with a TypeScript filesystem that matches its architecture.

Do not optimize algorithms.

Do not invent PERF-13.

Do not rewrite working retained machinery.

Do not preserve accidental file boundaries simply because they already exist.

The objective is:

> **Make architectural ownership physically obvious, enforce dependency direction, bury implementation transport details, and give PERF-13 clean places to add its retained state and content planes without contaminating the structural View system.**

The desired sequence remains:

```text
PERF-12
    efficient retained structural semantics

API-H1
    clean, truthful public semantic API

API-H2 / STRUCT-1
    clean physical TypeScript architecture

PERF-13
    structural DAG
    + retained mutable state plane
    + retained content plane
```

H2 succeeds when PERF-13 can add its new plane-specific modules without returning `packages/iyon-tui/src/` to a flat cross-layer namespace.
