# API-H1-V2 — Public API Hygiene and Intent Restoration

## 0. Purpose

API-H1 is a **pre-PERF-13 public API hygiene tranche**.

Its purpose is to make the TypeScript public API accurately express the generic framework that already exists, remove extraction-era and implementation-layer drift, and make the framework boundary easier to understand for both human users and coding agents.

It is **not** the final API stabilization pass.

PERF-13 will deliberately change the ontology around:

```text
structural View DAG
retained mutable geometry/presentation state
content sources / content lanes
layout-vs-paint update paths
mandatory high-throughput content transport
```

API-H1 must therefore fix:

```text
A. accidental surface drift
B. implementation leakage
C. false or misleading abstractions
D. public contracts that disagree with actual ownership/runtime behavior
```

without prematurely designing the post-PERF-13 model.

The guiding rule is:

> **Make the public API tell the truth about the architecture that exists today. Do not make it promise the architecture that PERF-13 has not built yet.**

There is one equally important performance/architecture rule:

> **API-H1 must not weaken, bypass, duplicate, or replace PERF-12's retained TypeScript composition and semantic DAG path.**

A cleaner API that causes more View construction, destroys retained identity, bypasses `compose.ts`, changes execution ownership, or falls back to whole-subtree publication is a regression and is not acceptable.

---

# 1. Repository and integration model

Authoritative repositories:

```text
alexykn/iyon-tui
alexykn/iyon
```

Development branches:

```text
iyon-tui/api-h1
iyon/main
```

After completion:

```text
iyon-tui/main
iyon/main
```

`@iyon/tui` is the canonical framework package identity.

`iyon:tui` is an Iyon-owned virtual-module alias that re-exports `@iyon/tui`. It is application plumbing, not a second implementation or public framework identity.

Framework documentation, fixtures, examples, and generic consumers should use:

```ts
import { ... } from "@iyon/tui";
```

---

## 1.1 Application integration workflow

Do **not** treat the revision written in Iyon's checked-in normal dependency manifests as the TUI revision under test during API-H1.

Iyon has a branch-selectable persistent-worktree integration build:

```bash
bun run build:iyon -- <iyon-tui-branch>
```

During development:

```bash
bun run build:iyon -- api-h1
```

After merge:

```bash
bun run build:iyon -- main
```

The build path:

```text
requested iyon-tui branch
        │
        ▼
resolve exact remote SHA
        │
        ▼
create/reuse persistent Iyon worktree
        │
        ▼
reset worktree to current Iyon HEAD
        │
        ▼
rewrite @iyon/tui package dependencies in worktree
        │
        ▼
rewrite Rust iyon-tui dependency in worktree
        │
        ▼
bun install
cargo update -p iyon-tui
        │
        ▼
stage core native addon
stage selected TUI native addon
        │
        ▼
build Iyon
        │
        ▼
copy binary into main checkout
```

The worktree is branch-keyed and persistent, so incremental compilation state survives repeated integration builds.

### API-H1 rule

Use:

```bash
bun run build:iyon -- api-h1
```

as the real external-consumer integration gate throughout development.

Do not modify the builder merely to point Iyon at the branch under test.

Do not make updating the checked-in default TUI revision a prerequisite of API-H1.

Permanent default-pin policy is separate.

---

# 2. Critical PERF-12 non-regression invariant

This section is **normative**.

API-H1 is allowed to change public TypeScript names, types, constructors, and package exports.

It is **not allowed to change the retained execution/composition semantics underneath them**.

PERF-12 established that ordinary public TypeScript construction can participate in the retained semantic architecture without the application managing transport identity manually.

That work must survive API-H1 intact.

## 2.1 Protected TypeScript retained path

The following are protected implementation machinery:

```text
packages/iyon-tui/src/compose.ts
packages/iyon-tui/src/execution.ts
packages/iyon-tui/src/execution-context.ts
packages/iyon-tui/src/define-view.ts
packages/iyon-tui/src/tracked-state.ts
packages/iyon-tui/src/child-owner.ts
packages/iyon-tui/src/persistent_seq.ts
packages/iyon-tui/src/retained_dag.ts
packages/iyon-tui/src/native_view_abi.ts
```

and all associated retained sidecars/metadata used by `View`.

Normal public declarative code must continue to flow conceptually as:

```text
defineView / View construction
        │
        ▼
retained TS composition
        │
        ▼
stable semantic View DAG identity
        │
        ▼
changed semantic frontier only
        │
        ▼
canonical retained ABI
        │
        ▼
Rust retained graph
```

API cleanup must **not** accidentally change that into:

```text
public API
   │
   ▼
fresh raw View creation every update
   │
   ▼
full materialization
```

or:

```text
public API
   │
   ▼
direct native mutation that bypasses retained composition
```

or:

```text
public wrapper
   │
   ▼
copy/normalize entire child tree
   │
   ▼
new identities
```

unless PERF-13 explicitly introduces such a new semantic layer later.

---

## 2.2 Protected retained invariants

API-H1 must preserve all of the following:

```text
stable NodeId semantics

unchanged subtree identity preservation

semantic cutoff on retained hits

defineView execution-scope identity

State<T> subscription ownership

keyed child-owner identity

ordinary unkeyed occurrence identity

builder-root ownership

direct-render takeover semantics

builder/direct ownership transitions

ViewSlot builder ownership

ScrollPane builder ownership

animation ownership transitions

PersistentSeq wide-axis behavior

retained axis/grid derivations

retained path edits

multi-edit transactions

NativeRef hints

WeakView / semantic caches

root leases

temporary MaterializeTx leases

release batching

cold recovery behavior

environment generation/recovery

current N-API and direct-FFI semantic parity
```

---

## 2.3 Public-wrapper rule

When API-H1 introduces a cleaner public semantic wrapper, the preferred transformation is:

```text
new public semantic value
        │
        ▼
private conversion to EXISTING internal representation
        │
        ▼
existing retained composition
```

For example:

```text
ColorSpec
   │
   ▼
private ColorNode lowering
```

not:

```text
ColorSpec
   │
   ▼
new alternate View construction path
```

Similarly:

```text
StyleRef
   │
   ▼
private StyleNode lowering
   │
   ▼
existing composeStyle(...)
```

must be preferred over introducing another retained-style implementation in parallel.

---

## 2.4 No hidden eager-normalization regression

Be particularly careful when cleaning types such as:

```text
GridSpec
ChildrenBuilder
StyleRef
ColorSpec
BorderSpec
Scene
control.view()
```

A semantic wrapper must not eagerly flatten a `PersistentSeq`, materialize lazy bridge children, clone a whole subtree, or reconstruct a semantically unchanged View merely to make the public type prettier.

API boundaries must remain thin.

---

## 2.5 PERF-12 regression gate

Before API-H1 is considered complete, retained-composition regression tests must prove at minimum:

```text
unchanged defineView scopes do not re-execute

unchanged keyed children retain execution identity

single-leaf State updates do not execute clean siblings

stable subtree View identity still cuts off retained decoding

wide retained axis edits still use PersistentSeq-derived paths

multi-edit retained transactions still work

ViewSlot builder updates remain retained

ScrollPane builder updates remain retained

direct render still correctly takes ownership from builder render

builder render still correctly subscribes State dependencies

N-API/direct-FFI semantic parity remains unchanged
```

If existing PERF-12/T13.1 tests already cover these, keep and run them.

Do **not** rewrite such tests merely because API names changed; adapt only their public setup.

### Mandatory principle

> If API-H1 makes the public API prettier but increases semantic DAG work, scope execution, JS allocation, View reconstruction, or bridge traffic for an equivalent workload, API-H1 has failed.

Otherwise PERF-12 would have been for naught.

---

# 3. Other protected architecture

In addition to the TS retained path, API-H1 must not alter:

```text
semantic View DAG topology rules
NodeId generation
canonical View ABI
ABI function semantics
N-API lowering
feature-gated direct FFI lowering
Rust semantic cache
Rust NativeRef table
Rust lease accounting
Rust derivation behavior
Rust PersistentSeq
Rust materialization
Rust measurement
Rust layout
Rust paint
component mount graph
focus routing
input routing
History ordering/storage semantics
TextStream storage/compiler semantics
animation scheduling
terminal correctness
```

A public facade cleanup should normally terminate before these layers.

---

# 4. Explicit PERF-13 non-goals

Do not:

```text
remove View.text()

introduce ContentSource

introduce ContentLane

introduce ContentSurface

introduce mandatory FFI content transport

move text payload out of the DAG

add mutable retained geometry handles

add mutable retained presentation handles

make background/color/border updates bypass the DAG

redesign style-state mutation

split layout/paint/content revisions

redesign ScrollPane around Content attachments

add image/video/Kitty data lanes

change Rust layout ownership

change Rust paint ownership
```

Current APIs such as:

```ts
.background(...)
.foreground(...)
.border(...)
.style(...)
.styleState(...)
.padding(...)
View.text(...)
```

may not have their final post-PERF-13 execution model.

API-H1 may improve the **types and meaning** of their arguments.

It must not redesign their execution path.

---

# 5. Cross-tranche implementation rules

These apply to H1A–H1L.

## 5.1 Prefer deletion over compatibility aliases

If a name is demonstrably false or obsolete:

```text
StreamPane = TextStream
NativeViewSlot alias
TuiFailure = TuiError
pre-generic nextAction
```

prefer removing it and migrating consumers.

Do not leave permanent compatibility debris unless there is an identified external requirement.

---

## 5.2 Do not solve type errors with weaker types

Forbidden “fixes” include replacing meaningful contracts with:

```ts
any
unknown
object
Record<string, unknown>
View | object
```

The goal is a more truthful API.

---

## 5.3 Public abstraction must match runtime ownership

A framework-owned retained/native handle must not masquerade as a freely implementable structural interface.

A genuinely callback-driven extension point should remain structurally implementable.

---

## 5.4 No new generic internal escape hatch

Do not introduce:

```text
@iyon/tui/internal
@iyon/tui/unsafe
@iyon/tui/native
```

merely to relocate current leakage.

Private implementation should stay private.

Only explicit semantic subpaths such as:

```text
@iyon/tui/testing
```

are appropriate where there is a real consumer category.

---

# API-H1A — Public Surface Closure

## Objective

Make `@iyon/tui` a **closed public type system**.

Every type visible in a public declaration must itself be intentionally public and importable from an appropriate documented entrypoint.

## Required work

Audit every root export and every generated `.d.ts`.

Classify each root name as:

```text
PUBLIC SEMANTIC VALUE
PUBLIC RUNTIME HANDLE
PUBLIC USER-IMPLEMENTABLE CONTRACT
PUBLIC ERROR/EVENT
TESTING API
PRIVATE BRIDGE/TRANSPORT IMPLEMENTATION
COMPATIBILITY/LEGACY
UNCLEAR — DECISION REQUIRED
```

The Section 9 discovery inventory is the baseline for this classification.

Public signatures must not refer to:

```text
./ir.ts
./native.ts
./retained_dag.ts
./native_view_abi.ts
./generated/*
Bridge*
Native* implementation contracts
```

### Semantic facade values

Introduce proper public values/types where needed:

```text
ColorSpec
ThemeColor
AnsiColor

BorderSpec
BorderStyle
BorderEdges
BorderGlyphs

OverflowIndicator

HorizontalAlign
VerticalAlign

GridTrack
GridCell
GridRow
GridSpec
...
```

Exact TS ergonomics may differ from Rust.

Semantic meaning should not.

## Private lowering

Use private conversion functions:

```text
ColorSpec → ColorNode
BorderSpec → BorderNode
GridSpec → bridge grid records
```

Do not change `bridge-schema.json` merely to make public declarations cleaner.

## PERF-12 constraint

Do not alter View construction/identity to introduce these semantic wrappers.

The wrappers should lower into the **existing** composition machinery.

## Gate

Generate declarations and prove:

```text
no public declaration path imports ir.ts
no public declaration path imports native.ts
no public declaration path imports generated ABI modules
all externally visible types are nameable
```

---

# API-H1B — Theme / Style / Color Semantic Restoration

## Objective

Restore the intended distinction:

```text
Theme
ThemeKey
ThemeColor
ColorSpec
StyleRef
StyleSpec
StyleSelector
```

## Required semantics

### Theme

Retained semantic definitions:

```text
named colors
named styles
selector/state variants
```

### StyleRef

Semantic named-style identity:

```text
ThemeKey
+
optional local StyleSpec
```

### StyleSpec

Sparse local/direct styling only:

```text
foreground
background
text attributes
```

No semantic theme identity.

### ColorSpec

Explicitly represents:

```text
theme color reference
named ANSI color
indexed ANSI
RGB
```

## Remove public conventions

Remove:

```ts
Style.new().theme(...)
StyleSpec.theme(...)
"theme:<key>"
```

from consumer APIs.

Private bridge lowering may continue encoding theme references however the existing ABI requires.

## Critical retained-theme rule

Do not turn:

```text
StyleRef("diff.meta")
```

into:

```text
copy current Theme definition into StyleSpec
```

during View construction.

The semantic key must survive until the existing Rust theme resolver can resolve it at paint time.

Otherwise global theme swapping ceases to be retained semantic behavior.

## Theme introspection

Audit:

```ts
Theme.style(key)
Theme.color(key)
```

Distinguish:

```text
definition inspection
```

from:

```text
semantic reference construction
```

Do not let lookup APIs become the normal application mechanism for selecting themed appearance.

## PERF-12 constraint

`View.style()` and composition must continue using retained composition/derivation as before.

The new semantic classes/types are facade changes, not a second styling path.

## Gate

Consumer/public source contains no magic `theme:` strings.

Named-style identity remains intact through the View DAG.

---

# API-H1C — Opaque Framework Handles

## Objective

Separate behavioral extension contracts from framework-owned retained handles.

## Consumer-implementable

Likely:

```text
Renderer
Projector
StreamingSource
TextVisitor
TextRewriter
ComponentAdapter
```

## Framework-owned

```text
History
TextInput
TextStream
ViewSlot
ScrollPane
Output<T>
```

These carry identity/lifetime owned by the framework.

## Required work

Make framework handles nominal/opaque enough that arbitrary objects cannot satisfy them accidentally.

Possible implementation:

```text
private class field
non-exported unique symbol
opaque concrete class
private unwrap registry
```

Centralize native unwrapping.

Replace repeated internal patterns like:

```ts
as unknown as { nativeHandle: object }
```

with typed private helpers.

## Handle ID

Audit `NativeHandleId`.

Since it is currently allocated by a TS counter, it must not be publicly described as native identity.

Either:

```text
rename semantically
```

or:

```text
remove from consumer API
```

## PERF-12 constraint

Opaque branding must not wrap/copy/recreate View values or affect retained identity.

It should only make ownership truthful to the TS type system.

---

# API-H1D — Canonical Control Construction and Lifecycle

## Objective

Remove cases where two public creation paths produce objects with materially different capabilities under the same apparent type.

## ViewSlot

Canonical path should be Tui-owned if builder semantics depend upon the shared retained execution runtime.

Do not preserve direct construction merely for compatibility if it lacks retained builder ownership.

## ScrollPane

Align interface with actual builder support:

```ts
setContent(View | (() => View))
```

if that is the intentional supported semantic contract.

## TextInput

Resolve direct-constructor vs Tui-factory mismatch.

Current direct construction ignores `border`, while Tui-host construction applies it.

Do not leave both paths claiming equivalent semantics.

## History

Decide explicitly whether detached History is a public feature.

If retained:

```text
document detached capabilities
document attach/transfer rules
```

If not:

```text
canonicalize construction through Tui
```

## TextStream

Direct construction may be a legitimate semantic exception because it represents an independent stream source.

Audit rather than assuming.

## Lifecycle record

For every public handle, document:

```text
creator
owner
disposer
whether Tui.close() disposes it
whether it may outlive Tui
whether multiple mounts are legal
whether builder mode exists
whether direct assignment relinquishes builder ownership
whether animation relinquishes builder ownership
```

## PERF-12 constraint

Do not simplify control ownership by deleting builder semantics.

In particular:

```text
ViewSlot(() => View)
ScrollPane(() => View)
```

must continue to participate in the shared retained execution runtime exactly as before.

---

# API-H1E — Component Composition Facade

## Objective

Hide native component-placement mechanics from ordinary application composition.

Controls already have:

```ts
control.view()
```

Prefer that over:

```ts
View.component(control)
```

for normal application code.

## `View.component()` decision

Audit usages.

If only internal control wrappers need it:

```text
demote it from public semantic API
```

If external generic component implementations legitimately need it:

```text
retain a carefully typed semantic component reference
```

Do not expose `{ id, nativeComponentId? }`-shaped implementation contracts.

## `Component` naming

Current TS `Component` and Rust `Component` do not clearly denote the same abstraction.

Resolve this.

Do not use the semantic root name `Component` for a concrete native slot-like wrapper if the generic framework concept is actually a behavior/capability contract.

## Protected implementation

Do not change:

```text
ViewKind::Component
ComponentId
mount graph
focus ownership
component revisions
native mounting
```

## PERF-12 constraint

`control.view()` must remain a thin semantic projection onto the existing component View identity.

Do not construct extra wrapper DAG layers solely for API cleanliness.

---

# API-H1F — Typed Output and Event Semantics

## Objective

Make `Output` a single coherent abstraction.

## Desired model

Mirror the Rust concept:

```text
Output<T>
    opaque typed output-channel identity

T
    emitted payload

TuiEvent
    delivery event carrying routed output / termination
```

Remove fake `.payload`.

## TextInput

Conceptually:

```ts
submitted(): Output<string>
```

## OutputRouter

Audit current TS `OutputRouter`.

Determine whether it is:

```text
intentional standalone generic utility
```

or:

```text
parallel/extraction-era mechanism with no runtime role
```

Delete or rename accordingly.

Do not make it compete with native typed routing.

## Event constructors

Classify:

```text
keyEvent
pasteEvent
resizeEvent
terminateEvent
```

into:

```text
runtime public event
component event
testing injection
private utility
```

Do not export test-input constructors as application runtime events merely because they share an `Event` suffix.

## PERF-12 constraint

Routing cleanup must not change View execution scheduling, dirty draining, or root publication.

Output/event cleanup is orthogonal to retained rendering.

---

# API-H1G — False Alias and Legacy Removal

## `StreamPane`

Remove:

```ts
TextStream as StreamPane
```

Rust `StreamPane<S>` is a real viewport abstraction, not a synonym.

Do not invent a replacement TS fake.

Actual exposure can be reconsidered separately.

## `nextAction`

Audit all usage.

Production Iyon already uses generic `nextEvent()`.

If only tests/compatibility remain:

```text
migrate tests
remove Tui.nextAction()
remove AppHarness.nextAction()
remove native nextAction/waitForAction aliases if otherwise unused
```

## No-op aliases

Audit and remove where meaningless:

```ts
TuiOperation<T> = T
TuiFailure = TuiError
```

## Smoke

Root `tuiSmoke` should not be conflated with native staging/smoke validation.

Remove application-facing smoke exports while preserving build/staging verification that still serves infrastructure correctness.

---

# API-H1H — Root Package and Subpath Hygiene

## Objective

Make:

```text
@iyon/tui
```

read like a semantic TUI framework.

## Root should contain

Approximately:

```text
View
Scene
Insets
BorderSpec
Grid...
OverflowIndicator

defineView
State
state

Theme
ThemeKey
ThemeColor
ColorSpec
StyleRef
StyleSpec
StyleSelector
StyleStateKey
StyleStateValue

TextSelector
TextSpan
TextContent
RawText
Annotations

Projection
Smooth
Diff*
MarkdownProjector
PlainTextProjector

Tui
History
TextInput
TextStream
ViewSlot
ScrollPane
Output<T>

genuine extension contracts

runtime events/errors
```

## Root should not contain implementation names

Audit/remove:

```text
NativeOutputHandle
NativeViewSlot
NativeScrollPane
ComponentAdapterBridge
```

and adapter classes whose only role is internal bridge adaptation.

## Testing subpath

Create:

```text
@iyon/tui/testing
```

for:

```text
AppHarness
createAppHarness
input injection
clock advancement
screen inspection
cell/style inspection
```

Audit direct test-oriented methods currently exposed on `Tui`:

```text
enqueue
screenRows
nativeHistoryRows
styleAt
cellXOfText
advance
current
exited
```

Move or hide those that are testing-only.

Do not change their underlying native behavior as part of this move.

## No `internal` subpath

Private remains private.

---

# API-H1I — Runtime Contract Truthfulness and Correctness

## `TuiRuntime`

Remove historical optionality where every supported runtime implements the operation.

Known candidates:

```text
createHistory
createTextInput
createViewSlot
createScrollPane
interceptPaste
forwardPaste
setTheme
```

If a genuinely partial runtime exists, model an actual capability boundary rather than sprinkling optional methods everywhere.

## `size`

Fix stale size behavior after:

```ts
tui.resize(...)
```

The authoritative public value must reflect successful resize.

## Scene

Audit:

```text
new Scene(...)
Scene.from(...)
structural Scene value
```

If the structural boundary is deliberate, retain and document it.

## Render semantics

Do not change:

```text
render(Scene)
render(() => Scene)
```

ownership semantics.

Direct rendering and retained builder rendering are intentionally different execution paths.

API-H1 may make their contracts clearer.

It must not merge them.

---

# API-H1J — Full Public Contract Parity Audit

## Mandatory audit set

Compare:

```text
TuiRuntime ↔ Tui

History contract ↔ History implementation

TextInput contract ↔ TextInput implementation

TextStream contract ↔ TextStream implementation

ViewSlot contract ↔ ViewSlot implementation

ScrollPane contract ↔ ScrollPane implementation

Theme/Style TS ↔ Rust semantic equivalents

Output TS ↔ Rust typed Output

Component TS ↔ Rust component semantics

Scene TS ↔ Rust scene semantics where relevant
```

## Known mismatches

Use Section 9 as the starting evidence:

```text
ScrollPane builder mismatch
optional TuiRuntime methods
TextInput border discrepancy
fake OutputHandle.payload
private type leakage
Component semantic mismatch
```

## Do not broaden to hide mismatch

Correct the contract.

Do not weaken it.

## PERF-12-specific parity check

Also audit whether facade changes introduce alternative raw paths around:

```text
compose*
isRetainedConstruction()
shared ExecutionRuntime
ViewSlot builder roots
ScrollPane builder roots
```

There must remain **one retained TS composition architecture**, not “old internal retained APIs” plus “new cleaner public APIs” that bypass them.

---

# API-H1K — Iyon Consumer Migration

## Integration command

During development:

```bash
bun run build:iyon -- api-h1
```

After merge:

```bash
bun run build:iyon -- main
```

## Migration areas

Known from Section 9:

```text
theme: strings
StyleSpec.theme()
View.component(control)
root testing imports
nextAction tests
optional TuiRuntime guards
control factory casts
Native* facade names
```

Migrate application code to the cleaned public API.

## Consumer boundary rule

If Iyon migration seems to require:

```text
deep @iyon/tui/src import
Bridge*
Native*
nativeHandle
nodeForBridge
retained DAG internals
```

stop.

That indicates the public framework API is still incomplete.

Fix `iyon-tui`; do not punch through the boundary.

## PERF-12 integration check

The migrated Iyon must retain the same retained behavior.

Pay particular attention to the actual application patterns using:

```text
defineView
State
View.key
ViewSlot builder functions
ScrollPane builder functions
```

A successful compile/build is necessary but not sufficient if those paths have silently become eager.

---

# API-H1L — Consumer, Performance-Semantics, and Ownership Gates

This tranche closes API-H1.

## H1L.1 Standalone external consumer

The fixture must use only documented package entrypoints.

No:

```text
Native*
Bridge*
nodeForBridge
nativeHandle
nativeObject
theme:
private IR
ABI knowledge
```

## H1L.2 Declaration closure test

Compile emitted public declarations and reject references to private source modules.

This should become a machine gate, not merely an audit observation.

At minimum catch:

```text
ir.ts
native.ts
retained_dag.ts
native_view_abi.ts
generated/
```

appearing in externally reachable declarations.

## H1L.3 Root surface gate

Review the public snapshot delta deliberately.

Expected categories:

```text
semantic types added
implementation types removed
testing symbols moved
false aliases removed
compatibility names removed
```

## H1L.4 Ownership gate

Run:

```bash
bun run check:ownership
```

Update the gate where useful so it checks API-H1's new invariants, not merely the old snapshot.

Potential new checks:

```text
no Native* root exports unless explicitly allowlisted

no Bridge* root exports

no private-module imports in emitted declarations

fixture imports only root/testing package entrypoints

no public "theme:" usage

no StyleSpec.theme

no root AppHarness

no StreamPane/TextStream alias
```

## H1L.5 PERF-12 retained-composition gate

This is mandatory.

Run the existing retained-execution/composition suite and explicitly verify:

```text
State dirty-scope cutoff

defineView scope reuse

keyed child reuse

unchanged sibling non-execution

stable semantic View identity

root builder lifecycle

direct takeover lifecycle

ViewSlot builder retention

ScrollPane builder retention

PersistentSeq retained edits

multi-edit retained transaction behavior

NativeRef/lease convergence
```

If practical, retain representative counters from before API-H1 and compare after API-H1.

The point is not microbenchmarking the renamed API.

The point is proving that the cleanup did not route equivalent public usage through more semantic work.

## H1L.6 T15 semantic oracle

API-H1 should not require reopening transport qualification, but the surviving N-API/direct oracle must still pass semantic parity:

```text
correctness
structure
phase behavior
multi-edit behavior
lifetime/memory invariants
```

A public facade rename must not alter the canonical retained semantics shared by both lowerings.

## H1L.7 Iyon integration

Final development branch gate:

```bash
bun run build:iyon -- api-h1
```

Final post-merge gate:

```bash
bun run build:iyon -- main
```

---

# 6. Recommended execution order

```text
H1A
Public Surface Audit and Closure
    │
    ▼
H1B
Theme / Style / Color Semantics
    │
    ▼
H1C
Opaque Framework Handles
    │
    ▼
H1D
Control Construction / Lifecycle
    │
    ▼
H1E
Component Composition Facade
    │
    ▼
H1F
Typed Output / Events
    │
    ▼
H1G
False Alias / Compatibility Removal
    │
    ▼
H1H
Root / Subpath Hygiene
    │
    ▼
H1I
Runtime Contract Correctness
    │
    ▼
H1J
Full Contract Parity Audit
    │
    ▼
H1K
Iyon Consumer Migration
    │
    ▼
H1L
Declaration + Ownership + PERF-12 + Integration Gates
```

Each tranche should leave `iyon-tui` internally coherent and green before the next begins.

Breaking consumer API during intermediate commits is acceptable.

Breaking retained semantic behavior is not.

---

# 7. Explicit PERF-13 follow-up ledger

Do not opportunistically solve the following:

```text
View.text() leaving structural DAG

ContentSource

ContentLane

ContentSurface

append vs replace semantics

mandatory FFI content data plane

shared/ring-buffer content transport

Kitty graphics

images/video/live frame surfaces

retained content identity

content revision

intrinsic geometry revision

paint revision

geometry-only retained node mutation

paint-only retained node mutation

background changes without DAG replacement

border color changes without DAG replacement

border width geometry mutation

blink/pulse paint mutation

focus/state paint mutation

Rust-owned consequence propagation

post-plane-split scrolling/content ownership
```

API-H1 should leave these easier to design, not pre-decide them.

---

# 8. Definition of done

API-H1 is complete when all of the following are true:

```text
PACKAGE IDENTITY
----------------
@iyon/tui is the clear canonical framework package.

PUBLIC TYPE CLOSURE
-------------------
No public declaration leaks private IR/native modules.

Every externally visible type is intentionally public and nameable.

STYLE / THEME
-------------
Theme, StyleRef, StyleSpec and ColorSpec have distinct meanings.

No public "theme:<key>" magic strings remain.

StyleSpec cannot carry semantic named-style identity.

Theme references survive as semantic identity until Rust paint resolution.

HANDLE OWNERSHIP
----------------
Framework-owned handles are opaque/truthful.

Arbitrary structural objects cannot masquerade as native-backed controls.

Internal native unwrapping is typed and centralized.

CONTROL LIFECYCLE
-----------------
Canonical construction paths have coherent capabilities.

ViewSlot/ScrollPane retained builder behavior is preserved.

TextInput construction no longer has silent border-semantic divergence.

History detached/attached behavior is deliberate and documented.

COMPONENTS
----------
Normal controls compose through semantic facade operations such as .view().

Native component-placement mechanics are not unnecessarily public.

OUTPUT
------
Output<T> means one coherent opaque typed channel identity.

No fake payload property exists on output handles.

FALSE API
---------
TextStream is not exported as StreamPane.

Dead compatibility aliases are removed.

nextAction is removed if its usage audit confirms it is compatibility-only.

ROOT HYGIENE
------------
Native implementation classes are absent from normal root exports.

Testing facilities live under @iyon/tui/testing.

No generic public internal escape hatch exists.

RUNTIME
-------
TuiRuntime reflects actual supported runtime capabilities.

Tui.size is correct after resize.

Direct vs retained render ownership remains unchanged.

EXTERNAL CONSUMERS
------------------
The standalone fixture uses no private framework knowledge.

Iyon migrates without deep imports or implementation casts.

PERF-12 NON-REGRESSION
----------------------
The TS retained composition layer is still the canonical path.

defineView/State scope reuse is unchanged.

Unchanged scopes/subtrees remain cut off.

PersistentSeq retained edits remain intact.

ViewSlot and ScrollPane builder paths remain retained.

NodeId/NativeRef/lease behavior is unchanged.

No facade wrapper causes whole-tree reconstruction or extra semantic work.

N-API and direct FFI still consume the same semantic architecture.

PERF-13
-------
No content-plane or retained mutable-property architecture has been
prematurely implemented.

INTEGRATION
-----------
bun run build:iyon -- api-h1

passes during development, and:

bun run build:iyon -- main

passes after merge.
```

The final architectural success criterion is:

> **A coding agent opening `@iyon/tui` sees one coherent semantic framework API, while the high-performance PERF-12 retained composition machinery remains invisible underneath it and behaves exactly as before.**

And the performance criterion should be stated just as strongly:

> **API-H1 is not permitted to trade away retained identity, execution cutoff, PersistentSeq behavior, NativeRef reuse, or narrow semantic publication for API cleanliness. If equivalent application code causes more DAG work after API-H1, the change is a regression even if the public API is aesthetically better.**

---

# 9. Initial codebase map and findings

- `9.1 Codebase map`
- `9.2 Root export classification record`
- `9.3 H1A — closure and declaration evidence`
- `9.4 H1B — style and color evidence`
- `9.5 H1C — handle and ownership evidence`
- `9.6 H1D — construction and lifecycle evidence`
- `9.7 H1E — composition and component evidence`
- `9.8 H1F — output and event evidence`
- `9.9 H1G — alias and compatibility evidence`
- `9.10 H1H — root/package hygiene evidence`
- `9.11 H1I/H1J — runtime correctness and parity evidence`
- `9.12 H1K — external consumer evidence`
- `9.13 H1L — fixture and gate evidence`
- `9.14 Decisions exposed by the map`
- `9.15 Protected/deferred conclusion`

> **PERF-12 non-regression note.** The discovery record also establishes that the public facade currently sits on top of a working retained TypeScript composition layer. API-H1 must treat `compose.ts`, `defineView`/`State` execution ownership, keyed child identity, ViewSlot/ScrollPane shared execution-runtime integration, `PersistentSeq`, derivation metadata, and retained materialization as existing architecture rather than cleanup targets. Public wrapper changes must lower into those paths rather than create parallel “clean” implementations. A successful API-H1 diff should therefore be concentrated in public semantic values, contracts, export topology, lifecycle wrappers, private lowering helpers, tests, and consumer migration; changes to retained composition require explicit justification and must not alter its behavior.

---

# 9. Initial codebase map and findings

This section is the discovery record for API-H1. It records the current implementation evidence before any API cleanup is applied; it does not change the requirements above. The audit baseline is the `api-h1` branch at `7f4945d`.

The audit covered the TypeScript package, the generic Rust facade/native binding, the standalone consumer fixture, and the checked-out external consumer at `/Users/alxknt/github/iyon-n/iyon`. A declaration-only TypeScript emit was also inspected so that findings are based on the generated public declaration shape, not only source imports.

## 9.1 Codebase map

| Layer | Actual paths | Responsibility observed |
| --- | --- | --- |
| Package entrypoint | `packages/iyon-tui/src/index.ts`, `packages/iyon-tui/package.json` | Root export list. The package currently exposes only `.` and `./native-stage`; there is no `./testing` subpath. |
| Public contracts | `packages/iyon-tui/src/types.ts` | Structural interfaces for runtime, controls, events, render/projector callbacks, and output. Several aliases point at implementation modules. |
| Semantic values | `packages/iyon-tui/src/values/{view,geometry,style,text,theme,text-content,annotations,projection,diff}.ts`, `scene.ts` | Immutable View construction, text/diff values, geometry, styling, theme configuration, projections, and Scene normalization. These files currently lower through `ir.ts` records. |
| Host-bound facade | `runtime.ts`, `history.ts`, `text-input.ts`, `stream.ts`, `component.ts`, `scroll-pane.ts` | TUI host lifecycle and wrappers for native History, TextInput, TextStream, ViewSlot, and ScrollPane. |
| Retained execution | `define-view.ts`, `tracked-state.ts`, `execution.ts`, `execution-context.ts`, `child-owner.ts`, `compose.ts` | `defineView`/`State` scopes, retained composition, keyed child identity, and semantic-slot reuse. This is protected PERF-12 machinery. |
| Private bridge | `ir.ts`, `native.ts`, `native_view_abi.ts`, `retained_dag.ts`, `generated/*`, `persistent_seq.ts` | Numeric bridge schema, native addon contract, retained materialization, ABI lowerings, and persistent sequences. These are implementation layers, not consumer concepts. |
| Auxiliary facade | `traits/*`, `output.ts`, `interaction.ts`, `events.ts`, `testing.ts` | Callback adapters, a separate string-keyed output queue, lightweight interaction helpers, event constructors, and headless test helpers. |
| Generic Rust semantic API | `crates/iyon-tui/src/presentation/api/*`, `output/*`, `controls/text_input/*`, `stream/pane/*`, `application/host.rs` | The semantic reference vocabulary: `ColorSpec`, `StyleRef`, `BorderSpec`, typed `Output<T>`, the real `StreamPane<S>`, host routing, and control lifecycle. |
| Native binding | `crates/iyon-tui-native/src/tui.rs`, `crates/iyon-tui-native/src/tui/view_abi.rs`, `src/generated/*` | Safe N-API control wrappers and the generated canonical View ABI. Direct FFI remains feature-gated and is outside this API cleanup. |
| External consumer oracle | `packages/tui-consumer-fixture/*`; `/Users/alxknt/github/iyon-n/iyon/plugins/app/iyon/*` | Public-only fixture and real external consumer. The Iyon virtual module `iyon:tui` re-exports `@iyon/tui` in `packages/iyon-runtime/src/virtual-modules.ts`. |

The main boundary finding is that the implementation layers are physically separate, but public declaration paths cross back into them. In particular, `values/view.ts`, `values/style.ts`, and `values/theme.ts` are public through `index.ts` while their declarations mention `ir.ts` types.

## 9.2 Root export classification record

The following is the complete current root export inventory from `packages/iyon-tui/src/index.ts`, including type-only exports. Each name is assigned exactly one baseline classification. “Unclear” means the name requires an API decision; it is not an approval to preserve the current shape.

| Baseline classification | Current root exports |
| --- | --- |
| **PUBLIC SEMANTIC VALUE** | `View`, `ChildrenBuilder`, `defineView`, `state`, `State`, `Insets`, `Style`, `StyleSpec`, `TextSelector`, `TextSpan`, `TextContent`, `RawText`, `Annotations`, `Projection`, `ProjectionBuilder`, `Smooth`, `DiffRange`, `DiffLine`, `DiffHunk`, `DiffRenderer`, `Theme`, `ThemeKey`, `ThemeSelector`, `PlainTextProjector`, `MarkdownProjector`, `HistoryLayout`, `ComponentCapabilities`, `RenderContext`, `StreamAnnotation`, `StreamSnapshot`, `TextStreamOptions`, `TextStreamPacing`, `TextStreamPresentation`, `TuiOpenOptions`, `Scene` |
| **PUBLIC RUNTIME HANDLE** | `NativeHandle`, `NativeHandleId`, `History`, `TextInput`, `TextStream`, `ViewSlot`, `ScrollPane`, `OutputHandle`, `Tui` |
| **PUBLIC USER-IMPLEMENTABLE CONTRACT** | `ComponentAdapter`, `ComponentContext`, `Projector`, `Renderer`, `StreamingSource`, `TextRewriter`, `TextVisitor`, `ViewComponent`, `TuiRuntime` |
| **PUBLIC ERROR/EVENT** | `TuiError`, `asTuiError`, `isTuiCancelledError`, `isTuiError`, `tuiError`, `OutputEvent`, `TuiEvent`, `KeyEvent`, `PasteEvent`, `ResizeEvent`, `TerminateEvent`, `RouteConflict` |
| **TESTING API** | `AppHarness`, `createAppHarness`, `keyEvent`, `pasteEvent`, `resizeEvent`, `terminateEvent`, `tuiSmoke` |
| **PRIVATE BRIDGE/TRANSPORT IMPLEMENTATION** | `NativeOutputHandle`, `NativeViewSlot`, `NativeScrollPane`, `RendererAdapter`, `ProjectorAdapter`, `TextVisitorAdapter`, `TextRewriterAdapter`, `StreamingSourceAdapter`, `ComponentAdapterBridge` |
| **COMPATIBILITY/LEGACY** | `StreamPane`, `TuiOperation`, `TuiFailure` |
| **UNCLEAR — REQUIRES DECISION** | `Component`, `Output`, `OutputRouter`, `FocusController`, `InteractionRouter` |

The inventory itself exposes two issues: a concrete `ViewSlot` is exported under the `NativeViewSlot` alias rather than its semantic name, and several names classified as private/legacy are still ordinary root exports. `InteractionResult`, `SceneProducer`, `TerminalMetadata`, `StreamSegmentSnapshot`, and the public grid/bridge value types are used by exported declarations but are not root-named exports.

## 9.3 H1A — closure and declaration evidence

The generated declarations confirm the closure defects:

- `types.d.ts` emits `TuiRuntime.createTextInput()` and `AppHarness.createTextInput()` with `import("./ir.ts").BorderNode`.
- `values/view.d.ts` emits `ColorNode`, `BorderNode`, `GridTrackNode`, `BridgeLayoutChild`, `GridSpec`, and `GridBuilder` through `View`, `ChildrenBuilder`, and `View.grid()`.
- `values/style.d.ts` emits `Color = ColorNode`, exposes `StyleSpec.value: StyleNode`, and makes the public constructor accept `StyleNode`.
- `values/theme.d.ts` emits `withColor(..., color: ColorNode)`, `color(): ColorNode | undefined`, and a public `materialize(): object`.
- `text-input.d.ts` exposes a public `nativeHandle?: NativeTextInputContract` constructor parameter and a `BorderNode` option; `NativeOutputHandle` exposes `nativeObject`.
- `execution.d.ts` makes `ViewComponent` public while its `View` type is imported from the private `values/view.ts` module.
- `TextSpan.value`, `TextSelector.value`, `Insets.value`, and `RawText.origin` similarly expose source-module value types that are not named at the root.

`ir.ts` explicitly labels its numeric records as private, so this is representation leakage rather than an intentional public bridge. The first cleanup can therefore add semantic wrappers and private lowering without changing `bridge-schema.json`, the retained DAG, or the ABI.

Additional public implementation surface is reachable through the root `View`/handle exports even though it is marked `@internal` in comments: `View.__rawGrid()`, `View.__composedAxis()`, the `*ForTransport()` statics, `ViewSlot.prepareSetView()`, `ViewSlot.tuiViewAbiInstallRef()`, `NativeScrollPane.tuiViewAbiInstallRef()`, and the public `nativeComponentId()`/`nativeObject()` helpers. These require an explicit root-surface decision, not a bridge redesign.

## 9.4 H1B — style and color evidence

The current TypeScript implementation is exactly the collapsed model described by H1B:

- `values/style.ts` defines `Color` as `ColorNode`, where `ColorNode` is `string | { type: "ansi"; value: number }`.
- `StyleSpec.theme(key)` writes `StyleNode.theme`, so a direct sparse style can carry named theme identity.
- `StyleSpec.foreground()` and `background()` accept arbitrary strings, including the undocumented `theme:<key>` convention.
- `Theme.withStyle()` stores `style.value`; `Theme.withColor()` stores `ColorNode`; `Theme.style()` returns a copied base `StyleSpec`; `Theme.color()` returns the private `ColorNode`.
- The only selector type is `ThemeSelector`, while Rust calls the corresponding concept `StyleSelector` and has separate `StyleStateKey`/`StyleStateValue` types.
- The root exports no `ColorSpec`, `ThemeColor`, `AnsiColor`, `StyleRef`, `StyleSelector`, `StyleStateKey`, `StyleStateValue`, `BorderSpec`, `BorderEdges`, `BorderGlyphs`, or `BorderStyle`.

The private lowering already isolates the wire convention: `retained_dag.ts` prefixes a private style key with `theme:`, and the native decoder strips that prefix in `crates/iyon-tui-native/src/tui/view_abi.rs` and `tui.rs`. This supports replacing the public type model without changing the schema.

Confirmed external call sites needing H1B migration are `packages/tui-consumer-fixture/src/consumer.ts`, `plugins/app/iyon/src/theme.ts`, `plugins/app/iyon/src/view.ts`, `plugins/app/iyon/src/app.ts`, `packages/iyon-plugins/src/tools/support/render.ts`, and `packages/iyon-runtime/src/tools/generic.ts`. They use both `Style.new().foreground("theme:...")` and `Style.new().theme(...)`.

## 9.5 H1C — handle and ownership evidence

`NativeHandle` is structural:

```ts
interface NativeHandle {
  readonly kind: string;
  readonly id: NativeHandleId;
  readonly disposed: boolean;
  dispose(): void;
}
```

`NativeHandleId` is only a compile-time branded number. `HandleBase` allocates it from the module-local JavaScript counter `nextHandleId`, so it is not a native identifier. There is no runtime brand or central handle registry.

The runtime then recovers native identity with repeated casts:

- `runtime.ts` casts History to `{ nativeObject(): object }` during scene publication and direct rendering.
- `runtime.ts` casts `OutputHandle` to `{ nativeObject: object }` in `route()`.
- `runtime.ts` casts `TextInput` to `{ nativeHandle: object }` in `interceptPaste()`.
- `history.ts` casts `TextStream` to `{ nativeObject(): object }` in `pushStream()` and `sealStream()`.
- `testing.ts` casts the public TextInput contract to the concrete implementation before paste interception.

`History.nativeObject()` and `TextStream.nativeObject()` are public methods on root-exported classes despite comments calling them internal. `HandleBase.nativeHandle` is protected only at the TypeScript level and is an ordinary JavaScript property at runtime. `NativeOutputHandle` makes the native object public directly.

This separates the intended behavioral contracts (`Renderer`, `Projector`, `TextVisitor`, `TextRewriter`, `StreamingSource`, `ComponentAdapter`) from the framework-owned handles, but the type system does not currently enforce that separation.

## 9.6 H1D — construction and lifecycle evidence

| Public path | Actual construction/lifecycle behavior |
| --- | --- |
| `new History()` | Creates `NativeHistory` with `host: None`; detached History can push/layout/set layout and attach streams, but native `freeze`/`discardLive` reject while detached. |
| `tui.createHistory()` | Wraps `host.history()`, which is already attached to that host. Native `setHistory` only accepts a detached History and transfers it once. |
| `new TextInput(options)` | Creates an independent `NativeTextInput`. `options.multiline` is passed, but `options.border` is ignored by the constructor. |
| `tui.createTextInput(options)` | Calls the host path `host.textInput(options.multiline, options.border)`, and the native host applies the border. This is not equivalent to direct construction. |
| `tui.createViewSlot(initial)` | Creates the actual `ViewSlot` with the Tui-owned retained execution runtime; builder mode is supported. |
| `NativeViewSlot` / direct `ViewSlot` construction | The alias points to the concrete class in `component.ts`, whose constructor requires the private host contract and has no shared runtime by default; builder mode then throws `TUI_EXECUTION_BUILDER_UNSUPPORTED`. |
| `tui.createScrollPane(initial)` | Creates `NativeScrollPane` with the Tui-owned retained runtime. |
| direct `NativeScrollPane` construction | Requires the private host contract and has no shared runtime by default; builder mode is rejected for the same reason. |
| `new TextStream(options)` | Creates an independent native stream source and is the strongest candidate for deliberate direct construction. |

`Tui.close()` and `Tui.exit()` dispose the root execution state, root boundary, and host, but `Tui` has no registry or disposal loop for user-created History, TextInput, ViewSlot, ScrollPane, or TextStream handles. Current consumers therefore dispose those handles themselves. This ownership rule is real but undocumented.

The interface/class parity issues are confirmed in source: `ScrollPane.setContent(view: View)` is narrower than concrete `NativeScrollPane.setContent(viewOrBuilder: View | (() => View))`; `History.setLayout` is optional in the contract but required in the class; and `TuiRuntime` marks `createHistory`, `createTextInput`, `createViewSlot`, `createScrollPane`, `interceptPaste`, `forwardPaste`, and `setTheme` optional even though `Tui` implements all of them. Iyon consequently branches on those methods in `plugins/app/iyon/src/app.ts` and casts the factory results.

## 9.7 H1E — composition and component evidence

All four host-bound controls expose a semantic-looking `view()` method, but each implementation returns `View.component(this)`. `View.component()` itself accepts an implementation-shaped object containing `id` and optional `nativeComponentId()`, not a public semantic component contract.

Current call sites are:

- framework tests: `packages/iyon-tui/tests/tui_harness.test.ts`, `tui_runtime.test.ts`, and `tui_realtime.test.ts`;
- fixture: `packages/tui-consumer-fixture/src/consumer.ts`;
- external consumer: `plugins/app/iyon/src/app.ts` and `plugins/app/iyon/src/view.ts`.

The concrete root export named `Component` is not the Rust behavioral Component concept. Its constructor allocates a native `NativeViewSlot` around a spacer and exposes `view()`, `capabilities()`, `revision()`, and `nativeComponentId()`. Separately, `ComponentAdapter`/`ComponentAdapterBridge` are callback contracts, but no production TypeScript path mounts or invokes `ComponentAdapterBridge`; its only current use is the adapter test. The underlying component View kind and native mount graph remain protected.

## 9.8 H1F — output and event evidence

Rust's `crates/iyon-tui/src/output/handle.rs` defines `Output<T>` as an opaque, typed, copyable channel identity. Payloads are stored separately in the native output queue and delivered by `OutputRouter`.

The TypeScript side has three incompatible meanings:

- `Output` is `Readonly<Record<string, unknown>>`, used by `ComponentContext.emit()` and `InteractionResult`.
- `OutputHandle<T>` requires a `.payload: T` property.
- `NativeOutputHandle<T>` declares `readonly payload!: T` but never assigns it; it actually stores a public `nativeObject` channel wrapper.

`Tui.route()` consumes the native object, not a payload. This confirms that `OutputHandle<T>` is the wrong public contract and that the Rust `Output<T>` model is the semantic reference.

The TypeScript `OutputRouter` is a separate FIFO keyed by arbitrary strings and record values. It has no call sites in production framework code; its current behavior is exercised by `tui_traits.test.ts`. It is not the same abstraction as the native typed channel router. `ComponentAdapterBridge` also emits the record-shaped `Output`, but is not connected to native routing.

`TuiEvent` contains only `OutputEvent | TerminateEvent`. `keyEvent`, `pasteEvent`, and `resizeEvent` construct values that `nextEvent()` never returns; `Tui.enqueue()` instead accepts an inline union and is used by the headless harness. The constructors have no active call sites outside their definitions and the historical surface inventory. This makes them testing/injection candidates, while `TuiEvent`/routed output remain runtime candidates.

The native binding still exposes `nextAction`/`waitForAction`, and `NativeTuiHostContract` declares them, but the TypeScript `Tui.nextAction()` is implemented by recursively adapting `nextEvent()` and does not call either native compatibility method. The Rust host methods are explicit aliases of `next_output()`/`wait_for_output()`.

## 9.9 H1G — alias and compatibility evidence

- `types.ts` declares `type StreamPane = TextStream`, and `stream.ts` re-exports `TextStream as StreamPane`. Rust `crates/iyon-tui/src/stream/pane/mod.rs` is a real generic mounted viewport with model, row index, viewport mode, refresh, seal, and scrolling; it is not a TextStream.
- `nextAction()` remains in `Tui`, `AppHarness`, and the `AppHarness` contract. It is used by framework tests and by Iyon public/smoke tests, but the external Iyon application loop in `plugins/app/iyon/src/app.ts` already uses `nextEvent()`. No Iyon production source call to `nextAction()` was found.
- `TuiOperation<T> = T` is used throughout the public contracts only to disguise synchronous returns. `TuiFailure = TuiError` has no active source use beyond its root export and historical inventory.
- `tuiSmoke` is a root constant while the staging script also probes the native `tuiSmoke()` symbol. Root removal should decouple the package-load probe from the application-facing export rather than remove the native staging check accidentally.

## 9.10 H1H — root/package hygiene evidence

`packages/iyon-tui/package.json` exports only:

```json
{
  ".": "./src/index.ts",
  "./native-stage": "./scripts/stage-native.ts"
}
```

Consequently `AppHarness`, deterministic input/clock helpers, screen rows, cell/style inspection, and `createAppHarness` are currently root exports. There is no documented `@iyon/tui/testing` entrypoint.

The root also exports the native/bridge-adapter names listed in the classification table. The adapter classes are only used by `packages/iyon-tui/tests/tui_traits.test.ts` (plus their definitions and root exports). `FocusController` and `InteractionRouter` have no active framework or external consumer call sites beyond their definitions. `OutputRouter` has the one adapter test noted above.

The concrete `Tui` root class additionally exposes test/inspection and injection operations not present in `TuiRuntime`: `enqueue`, `screenRows`, `nativeHistoryRows`, `styleAt`, `cellXOfText`, `exited`, `advance`, and `current`. These need classification alongside `AppHarness`; moving them must not alter host or retained behavior.

## 9.11 H1I/H1J — runtime correctness and parity evidence

- `Tui` stores `private readonly width` and `height` at open time. `resize()` updates the native host but never updates those fields, so `tui.size` remains stale after a successful resize. `AppHarness` separately updates its own mutable `options`, which can hide this defect in harness-only checks. The native host currently has no public size query.
- `Scene.from(value)` intentionally normalizes a structural `{ body, history? }` value into the concrete `Scene`; this is a deliberate structural boundary candidate, not an accidental duplicate constructor until usage is reviewed.
- Direct and builder render paths are distinct and protected: `Tui.render(Scene)` performs direct takeover and disposes the root builder; `Tui.render(() => Scene)` owns the retained root and subscribes State reads. API-H1 must not merge these execution paths.
- The declaration audit confirms the known ScrollPane mismatch, optional runtime methods, TextInput border mismatch, fake OutputHandle payload, and private type paths. `Tui`/`AppHarness` implementation methods otherwise satisfy the current structural interfaces, which is why ordinary typechecking does not expose the semantic drift.

## 9.12 H1K — external consumer evidence

The external integration path is implemented as documented: Iyon `package.json` maps `build:iyon` to `packages/iyon-cli/build-perf-refactor.ts`; that script resolves the requested `alexykn/iyon-tui` branch SHA, rewrites package and Cargo dependencies in a persistent branch-keyed worktree, stages both native addons, and builds the consumer.

The current external consumer demonstrates the migration pressure:

- `plugins/app/iyon/src/app.ts` starts with `new History()` and `new TextInput({ multiline: true })`, then disposes and replaces them with `tui.createHistory()`/`tui.createTextInput()` when a host is available.
- The same file and `plugins/app/iyon/src/view.ts` use `View.component(...)` for controls and slots instead of the already available `.view()` methods.
- `plugins/app/iyon/src/theme.ts`, `plugins/app/iyon/src/app.ts`, and tool rendering helpers use both magic `theme:` strings and `StyleSpec.theme()`.
- The app checks optional `TuiRuntime` capabilities and uses casts around factory results, directly reflecting H1D/H1J contract uncertainty.
- Iyon tests import `createAppHarness` from the root and use `nextAction()`, so H1G/H1H changes require a coordinated test-subpath/event migration.

`iyon:tui` itself is not a second framework identity: `packages/iyon-runtime/src/virtual-modules.ts` defines it as `export * from "@iyon/tui"`. No separate alias implementation should be introduced.

## 9.13 H1L — fixture and gate evidence

The standalone fixture passes the existing package-boundary rule: `packages/tui-consumer-fixture/package.json` depends only on `@iyon/tui`, and fixture source imports only that package (plus relative fixture files). It nevertheless contains two API-H1 acceptance failures by design: `consumer.ts` uses `Style.new().foreground("theme:...")` and repeatedly uses `View.component(...)`.

The current public surface test is not a closure test. `packages/iyon-tui/tests/tui_surface_contract.test.ts` checks a structural Scene object and a minimal `ComponentAdapter`, using `{ kind: "view" } as View`; it does not import or name `BorderSpec`, `ColorSpec`, `SceneProducer`, `InteractionResult`, or any other type exposed through a public signature. The root typecheck also includes only this workspace and the fixture, so private declaration paths can remain unnoticed.

`tools/ownership/check.ts` validates import direction, safe N-API boundary, fixture dependencies, root export-name snapshot, and banned root names. It does not currently validate public declaration closure, testing-subpath placement, semantic style types, nominal handles, or class/interface parity. The snapshot is therefore a useful inventory baseline, not proof that the current root is hygienic.

## 9.14 Decisions exposed by the map

The audit leaves these concrete decisions for the implementation tranches:

1. Whether detached `History` is a supported public value, and if so how its detached/attached capabilities are named and documented.
2. Whether direct TextInput construction remains public after fixing the border mismatch, or whether host-bound construction becomes canonical.
3. Whether `Component` means a behavioral consumer contract or should be removed/renamed because the current class is a native slot wrapper.
4. Whether `OutputRouter`/`FocusController`/`InteractionRouter` are generic public utilities or test/extraction residue.
5. Which concrete `Tui` inspection/injection methods belong under `@iyon/tui/testing`.
6. Whether `nextAction` is retained only through the migration window and then removed together with the unused native aliases.
7. Which semantic public names and types replace the current bridge-facing values, while keeping private `theme:` lowering and the existing ABI intact.

## 9.15 Protected/deferred conclusion

The confirmed API-H1 defects are concentrated in the TypeScript facade, its declaration surface, control ownership wrappers, and external consumers. The audit found no reason to modify `retained_dag.ts`, `native_view_abi.ts`, generated ABI functions, NodeId/NativeRef allocation, PersistentSeq, Rust layout, Rust painting, or the two PERF-12 transport lowerings. Those remain protected while H1A–H1L clean the names and boundaries around them.

No implementation API changes have been made as part of this discovery record; the next work should proceed in the documented H1A → H1L order, using these paths and call sites as the acceptance map.
