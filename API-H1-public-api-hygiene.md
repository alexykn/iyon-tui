# API-H1 — Public API Hygiene and Intent Restoration

## 0. Purpose

API-H1 is a **pre-PERF-13 public API hygiene tranche**.

Its purpose is to make the TypeScript public API accurately express the framework that already exists, remove extraction-era and implementation-layer drift, and make the boundary easier for both humans and coding agents to understand.

It is **not** the final public API stabilization pass.

PERF-13 will deliberately change the ontology around:

```text
structural View DAG
retained mutable geometry/presentation state
content sources / content lanes
layout-vs-paint update paths
high-throughput content transport
```

API-H1 must therefore clean **accidental API problems** without prematurely designing those future concepts.

The guiding rule:

> **Make the API tell the truth about the architecture that exists today. Do not make it promise the architecture that PERF-13 has not built yet.**

---

# 1. Repository and integration model

Authoritative repositories:

```text
alexykn/iyon-tui
alexykn/iyon
```

Current framework branch:

```text
iyon-tui/main
```

Current application branch:

```text
iyon/main
```

`@iyon/tui` is the canonical framework package.

`iyon:tui` is an application-owned virtual-module alias that re-exports `@iyon/tui`; it is not a second framework identity.  

## 1.1 Correct application integration workflow

Do **not** treat the TUI revision written in the normal Iyon manifests as the branch-integration target for this work.

Iyon now has a branch-selectable persistent-worktree build path:

```bash
bun run build:iyon -- <iyon-tui-branch>
```

For example:

```bash
bun run build:iyon -- api-h1
```

or after merge:

```bash
bun run build:iyon -- main
```

The build script:

```text
1. resolves refs/heads/<branch> from alexykn/iyon-tui;
2. obtains the exact remote SHA;
3. creates or reuses a persistent cached Iyon worktree for that TUI branch;
4. hard-resets the worktree to the current Iyon HEAD;
5. rewrites all @iyon/tui Git dependencies inside that worktree
   to the resolved TUI SHA;
6. rewrites the Cargo iyon-tui dependency inside that worktree
   to the same SHA;
7. runs bun install;
8. runs cargo update -p iyon-tui;
9. stages the Iyon core native addon;
10. stages the selected TUI native addon;
11. builds Iyon;
12. copies the resulting binary back to the main checkout.
```

The worktree is persistent and branch-keyed, so repeated builds retain incremental build state. 

### API-H1 rule

While developing API-H1:

```bash
bun run build:iyon -- api-h1
```

is the normal external-consumer integration gate.

Do **not** modify the build script merely to update the TUI under test.

Do **not** make changing the checked-in manifest pin a prerequisite for API-H1.

If permanent default-pin policy is to be changed later, that is a separate decision.

---

# 2. Protected architecture

API-H1 may break public TypeScript API compatibility where the existing API is clearly accidental or misleading.

It must **not** disturb PERF-12 architecture.

The following are protected:

```text
semantic View DAG
stable View / NodeId identity
retained composition
defineView
State<T>
execution scopes
dirty scheduling
NativeRef hints
WeakView / semantic caches
root leases
boundary ownership
MaterializeTx
PersistentSeq
derivation hints
retained edit paths
retained transactions
canonical View ABI
N-API lowering
feature-gated direct FFI lowering
Rust layout
Rust measurement
Rust painting
component mounting
History mechanics
TextStream internals
native input routing
focus routing
animation scheduling
```

The current repository architecture explicitly defines the TS package as the public facade over a private native seam and retained transport-independent semantic machinery. 

---

# 3. Explicit non-goals

API-H1 must **not**:

```text
remove View.text()
introduce ContentSource
introduce ContentLane
introduce the fast content FFI data plane
split content out of View yet
move paint-state changes off the DAG yet
introduce mutable retained View property handles
change Rust layout ownership
change Rust paint ownership
redefine scrolling around content surfaces
replace View.vertical/horizontal/grid
redesign the retained scheduler
rewrite the View ABI
change NodeId allocation
replace the semantic DAG
```

If a cleanup starts requiring the question:

> Is this structural state, geometry state, presentation state, or content state?

and answering that changes execution semantics, stop.

That belongs to PERF-13.

---

# API-H1A — Public Surface Audit and Closure

## H1A.1 Objective

Make the public `@iyon/tui` surface **closed, nameable, and internally consistent**.

A consumer should never have to know a private source filename to describe a public argument or return value.

## H1A.2 Current problem

Public APIs currently leak private bridge/IR types.

For example, `TuiRuntime.createTextInput()` exposes:

```ts
border?: import("./ir.ts").BorderNode
```

through a public runtime interface. 

The public `View` facade similarly accepts internal things such as:

```text
ColorNode
BorderNode
GridTrackNode-derived shapes
```

while `ir.ts` itself is intended as the private semantic bridge representation.  

This is backwards.

The architecture should be:

```text
consumer
   │
   ▼
public semantic TS types
   │
   ▼
private lowering/conversion
   │
   ▼
bridge IR
   │
   ▼
retained N-API / cold N-API
```

not:

```text
consumer
   │
   ▼
bridge IR types
```

## H1A.3 Required audit

Enumerate every root export from:

```text
packages/iyon-tui/src/index.ts
```

and classify each as exactly one of:

```text
PUBLIC SEMANTIC VALUE
PUBLIC RUNTIME HANDLE
PUBLIC USER-IMPLEMENTABLE CONTRACT
PUBLIC ERROR/EVENT
TESTING API
PRIVATE BRIDGE/TRANSPORT IMPLEMENTATION
COMPATIBILITY/LEGACY
UNCLEAR — REQUIRES DECISION
```

The current root is broad and includes semantic values, native implementation classes, adapters, testing utilities, and smoke machinery together. 

## H1A.4 Public signature closure rule

Every exported function/method/class/interface must use only:

```text
language primitives
public @iyon/tui types
public callbacks/interfaces
```

No public signature may mention:

```text
./ir.ts
./native.ts
./retained_dag.ts
./native_view_abi.ts
./generated/*
Bridge*
Native* implementation types
```

unless that type has deliberately been promoted into a semantic public abstraction.

## H1A.5 Likely semantic types to expose properly

Rust already has a much cleaner semantic vocabulary:

```text
ColorSpec
ThemeColor
AnsiColor

BorderSpec
BorderStyle
BorderEdges
BorderGlyphs

StyleSpec
StyleRef
StyleSelector
StyleStateKey
StyleStateValue

OverflowIndicator

HorizontalAlign
VerticalAlign

GridTrack
GridCellSpec
GridRow
...
```

The Rust public facade exposes these rather than exposing the physical IR. 

Use this as a semantic reference, not as a requirement for mechanically identical TS syntax.

## H1A.6 Important constraint

Do **not** change the bridge schema merely because the public facade is being cleaned.

It is perfectly valid to have:

```ts
public BorderSpec
    ↓ private function
BorderNode
```

or:

```ts
public ColorSpec
    ↓ private lowering
ColorNode
```

The existing ABI should remain untouched unless an actual correctness bug makes that impossible.

## H1A.7 Deliverable

At the end of H1A:

```text
Every public signature is closed over public types.

No consumer has to import or understand ir.ts.

No native/bridge representation has been redesigned.

A surface classification record exists so later tranches
can distinguish deliberate API from historical leakage.
```

---

# API-H1B — Restore Theme / Style / Color Semantics

## H1B.1 Objective

Restore the distinction that existed in the intended framework design and still exists much more clearly on the Rust side:

```text
Theme
StyleRef
StyleSpec
ColorSpec
```

These must not be collapsed into string conventions.

## H1B.2 Current drift

Current TS permits:

```ts
Style.new().foreground("theme:truncation_footer")
Style.new().foreground(`theme:${key}`)
Style.new().theme("diff.meta")
```

Iyon actually uses all of these patterns today. 

And current `StyleSpec` literally contains:

```ts
theme(key: string): StyleSpec
```

alongside direct foreground/background modification. 

That conflates:

```text
semantic retained style identity
```

with:

```text
local sparse presentation changes
```

## H1B.3 Existing Rust model

Rust already separates them.

`StyleSpec` is a sparse local style intent containing foreground/background/attributes. 

`StyleRef` is:

```text
optional semantic ThemeKey
+
local StyleSpec override
```

with distinct constructors for:

```text
direct style
theme reference
theme + overrides
```



`ColorSpec` separately describes:

```text
theme color reference
named ANSI
indexed ANSI
RGB
```



This semantic distinction matches the intended API far better than the current TS string encoding.

## H1B.4 Required public semantics

Target conceptual model:

```text
Theme
  retained definitions and variants

ThemeKey
  semantic named identity

StyleRef
  reference to a semantic named style
  optionally carrying sparse overrides

StyleSpec
  direct sparse style patch
  no theme identity

ColorSpec
  either semantic theme-color reference
  or explicit physical/backend-neutral color

ThemeColor
  value stored in Theme color definitions

StyleSelector
  selector over focus/style-state semantics
```

## H1B.5 Required removals

From public API:

```text
StyleSpec.theme(...)
"theme:<key>" color strings
implicit parsing of theme identity from arbitrary strings
```

If the native bridge currently encodes something internally using `"theme:foo"`, it may remain as a **private lowering detail** during API-H1.

Consumers must not see it.

## H1B.6 Desired usage shape

Exact naming can be adjusted, but semantically usage should resemble:

```ts
const running = StyleRef.theme("tool.running");

const pulse = Style.new().dim();

const accent =
  Style.new().foreground(ColorSpec.theme("accent"));
```

A named style reference and a direct sparse patch must be distinguishable in the type system.

## H1B.7 Theme introspection audit

Current `Theme.style(key)` returns the current base `StyleSpec`. 

That risks making this:

```text
semantic identity → "diff.meta"
```

turn into:

```text
copied current definition → StyleSpec
```

when the caller really wanted retained semantic identity.

Audit whether `Theme.style()` is actually required publicly.

If definition inspection is legitimate, make its semantics explicit in the name/API.

Do not use it as the normal way to apply themed style.

## H1B.8 Selector naming

Current TypeScript uses:

```text
ThemeSelector
```

while Rust models the concept as:

```text
StyleSelector
```

Audit and align terminology unless there is a genuine distinction.

## H1B.9 PERF-13 boundary

Do **not** use H1B to decide how blinking/color animation/live style updates eventually bypass structural DAG mutation.

API-H1 only restores the semantic vocabulary.

PERF-13 decides the efficient retained mutation path.

## H1B.10 Gate

Public Iyon code contains zero:

```text
"theme:"
Style.new().theme(...)
```

except intentionally private bridge tests if the private wire encoding still requires them.

---

# API-H1C — Opaque Handles and Control Ownership

## H1C.1 Objective

Make public types accurately distinguish:

```text
consumer-implementable structural interfaces
```

from:

```text
framework-owned retained/native handles
```

## H1C.2 Current problem

Several public interfaces look structurally implementable, but runtime code actually assumes framework-owned internal objects and recovers them using casts.

Examples include code equivalent to:

```ts
output as unknown as { nativeObject: object }

input as unknown as { nativeHandle: object }

stream as unknown as { nativeObject(): object }
```

in runtime routing, paste interception, and History streaming.  

That means the public type contract currently says:

```text
"anything structurally compatible works"
```

while runtime semantics say:

```text
"this must be a framework-created handle with private native identity"
```

## H1C.3 Required classification

These should remain genuinely consumer-implementable where intended:

```text
Renderer
Projector
TextVisitor
TextRewriter
StreamingSource
ComponentAdapter
```

These are behavioral contracts.

These are different:

```text
History
TextInput
TextStream
ViewSlot
ScrollPane
Output<T>
```

They carry framework/native retained identity and lifecycle.

They should not be forgeable through accidental structural compatibility.

## H1C.4 Required implementation

Introduce a simple nominal/opaque boundary for framework handles.

Possible mechanisms:

```text
private class field
non-exported unique-symbol brand
opaque concrete class
internal handle registry/unwrap helper
```

Do not expose native pointers or native objects publicly.

Internal code should have typed private helpers such as conceptually:

```ts
unwrapTextInput(...)
unwrapTextStream(...)
unwrapOutput(...)
```

rather than ad hoc:

```ts
as unknown as { nativeObject... }
```

everywhere.

## H1C.5 `NativeHandleId` problem

Current `HandleBase` allocates its public `NativeHandleId` from:

```ts
let nextHandleId = 1;
```

on the TS side. 

That ID is not inherently a native identifier.

Audit whether consumers need it at all.

If they do, use semantic terminology:

```text
HandleId
ComponentHandle
...
```

If not, remove it from the root API.

Do not call a JavaScript-side identity “NativeHandleId”.

## H1C.6 Gate

No public runtime operation should need:

```ts
as unknown as { nativeHandle: ... }
```

to accept another public framework object.

Private unwrapping should be type-safe and centralized.

---

# API-H1D — Canonical Control Construction and Lifecycle

## H1D.1 Objective

Make creation semantics and ownership unambiguous.

There should not be one constructor that creates a weaker control and another factory that creates a fully integrated control under the same apparent public type.

## H1D.2 ViewSlot drift

`ViewSlot.setView(() => View)` relies on the Tui-owned shared retained execution runtime.

A slot directly constructed without that runtime explicitly rejects builder mode with:

```text
TUI_EXECUTION_BUILDER_UNSUPPORTED
```



Therefore direct construction and:

```ts
tui.createViewSlot(...)
```

do **not** have equivalent semantics.

That distinction must not be hidden behind the same apparent public abstraction.

## H1D.3 ScrollPane drift

The concrete ScrollPane implementation supports:

```ts
setContent(View | (() => View))
```

with the same retained builder ownership model. 

But the public contract currently says:

```ts
setContent(view: View)
```

only. 

Fix that mismatch.

## H1D.4 TextInput drift

The public TS class constructor accepts:

```ts
{
    multiline?: boolean;
    border?: BorderNode;
}
```

but direct construction calls:

```text
nativeTui.textInput(options?.multiline)
```

and therefore does not send the border at all. 

The Tui factory does use the host path that can provide the border.

This is exactly the kind of two-path API that API-H1 should eliminate.

## H1D.5 History audit

There is similarly a distinction between detached History construction and History obtained from a host.

Determine whether detached History is a deliberate public concept.

If yes:

```text
make detached semantics explicit
```

If no:

```text
canonicalize on Tui.createHistory()
```

Do not leave two paths merely because both currently exist.

## H1D.6 Construction policy

Default policy:

```text
Tui-created controls
    canonical for host/runtime-bound objects

direct constructor
    public only if detached construction is meaningful
    and has the same documented semantics
```

Likely:

```text
TextInput       → Tui factory
ViewSlot        → Tui factory
ScrollPane      → Tui factory

History         → audit detached semantics

TextStream      → direct construction may legitimately remain
                  because it represents an independent retained stream source
```

Do not mechanically apply this rule without auditing actual lifecycle.

## H1D.7 Lifecycle documentation

Document for each handle:

```text
who creates it
who owns native identity
whether it can outlive Tui
whether dispose() is required
whether Tui.close() disposes it
whether it can be mounted more than once
whether builder mode exists
whether direct mode replaces builder ownership
whether animation replaces builder ownership
```

This should describe actual current behavior, not invent new lifecycle machinery.

---

# API-H1E — Component and View Composition Facade

## H1E.1 Objective

Remove implementation mechanics from normal composition where a semantic API already exists.

## H1E.2 Current state

Controls already expose:

```ts
input.view()
slot.view()
pane.view()
```

The implementation constructs the component View internally.   

But normal consumer code still writes:

```ts
View.component(slot)
View.component(composer)
View.component(pane)
```

The standalone consumer fixture demonstrates this pattern. 

## H1E.3 Target

Canonical normal composition should be:

```ts
slot.view()
composer.view()
pane.view()
```

rather than requiring callers to understand component-handle lowering.

## H1E.4 `View.component()` audit

Do a repository-wide usage audit.

If the only legitimate callers are framework implementation code and control wrappers:

```text
demote View.component() from the normal public facade
```

If generic external component implementations genuinely need it:

```text
retain a carefully typed semantic component boundary
```

Do not delete the underlying component View kind.

Protected:

```text
native ComponentId
mount graph
focus identity
component retained state
ViewKind::Component
```

This is facade cleanup only.

## H1E.5 `Component` naming audit

Current TS exports a concrete `Component` class whose implementation is essentially a native ViewSlot-shaped handle, while Rust's public `Component` concept is a behavioral retained component contract.  

This deserves explicit review.

Do not let the TS root use `Component` to mean:

```text
"generic native thing that happens to occupy a component slot"
```

if the actual semantic concept is different.

Either make it represent the real semantic concept or stop exporting that implementation class under the semantic name.

---

# API-H1F — Restore Typed Output / Event Semantics

## H1F.1 Objective

Make `Output` mean one thing.

## H1F.2 Current conflict

Current TS has:

```ts
type Output = Readonly<Record<string, unknown>>
```

used by generic adapter routing. 

It also has:

```ts
interface OutputHandle<T> {
    readonly kind: "output";
    readonly payload: T;
}
```

But concrete `NativeOutputHandle<T>` contains:

```ts
readonly payload!: T;
```

without actually holding that payload. 

It is really an opaque native channel/route identity.

Meanwhile `OutputRouter` uses `Output` to mean an arbitrary record-like emitted value. 

Three concepts have collapsed into two names.

## H1F.3 Rust reference

Rust's `Output<T>` is clear:

> an opaque typed identity for a semantic output channel.

It carries type information and identity, not a payload value. 

## H1F.4 Required TS model

Conceptually:

```text
Output<T>
    opaque typed channel identity

emitted T
    actual payload value

TuiEvent / routed output event
    delivery mechanism across runtime boundary
```

No fake `.payload`.

## H1F.5 TextInput

`TextInput.submitted()` should return an opaque typed output channel.

Something conceptually like:

```ts
submitted(): Output<string>
```

not an object pretending to contain a string it does not contain.

## H1F.6 OutputRouter

Audit whether the current TS `OutputRouter` is:

```text
a legitimate generic user-space routing abstraction
```

or:

```text
an extraction-era parallel implementation of native routing
```

If legitimate, rename its record/value concept so it does not collide with `Output<T>`.

If obsolete, remove it.

Do not keep incompatible meanings merely for compatibility.

## H1F.7 Event constructors

Also audit root exports:

```text
keyEvent
pasteEvent
resizeEvent
terminateEvent
```

The public runtime's `TuiEvent` currently represents routed output / termination, while key/paste/resize values belong to different interaction/testing paths. 

Do not expose event constructors from root merely because tests or bridge code once needed them.

Classify each into:

```text
application runtime event
component interaction event
testing injection utility
private implementation
```

and export accordingly.

---

# API-H1G — Remove False Aliases and Compatibility Scars

## H1G.1 Objective

Delete public names that actively misrepresent another concept.

## H1G.2 `StreamPane = TextStream`

Current TS exports:

```ts
export { TextStream as StreamPane };
```



This is false parity.

Rust's actual `StreamPane<S>` is a mounted semantic viewport containing:

```text
StreamModel
viewport state
layout size
row index
scroll position
follow-end state
source refreshing
sealing
semantic change repair
```



A `TextStream` is not that.

## H1G.3 Required change

Remove the TS `StreamPane` alias.

Do **not** invent another fake implementation to preserve the name.

If actual TS exposure of Rust `StreamPane` requires substantive new binding work:

```text
record it as future work
```

and leave it absent.

Do not implement the PERF-13 content plane under this ticket.

## H1G.4 `nextAction`

Current `Tui.nextAction()` explicitly describes itself as:

```text
Compatibility adapter for the pre-generic application harness.
```

The canonical generic runtime path is already:

```text
nextEvent()
TuiEvent
routeId
```



Audit repository usage.

If nothing current requires it:

```text
remove nextAction()
remove AppHarness.nextAction()
remove corresponding native compatibility operations if otherwise unused
migrate tests to nextEvent()
```

Do not replace it with a differently named alias.

## H1G.5 No-op aliases

Audit:

```ts
type TuiOperation<T> = T
type TuiFailure = TuiError
```

If these no longer encode meaningful abstraction or portability, remove them.

Do not preserve type aliases that only make current synchronous APIs look like an abstraction that no longer exists.

---

# API-H1H — Root Package Hygiene and Subpaths

## H1H.1 Objective

Make the root package read like a generic TUI framework, not a union of framework + bridge + testing + benchmark support.

## H1H.2 Current root leakage

Current root exports include:

```text
NativeOutputHandle
NativeViewSlot
NativeScrollPane

RendererAdapter
ProjectorAdapter
TextVisitorAdapter
TextRewriterAdapter
StreamingSourceAdapter
ComponentAdapterBridge

AppHarness
createAppHarness

tuiSmoke
```

alongside real semantic framework concepts. 

The repository architecture simultaneously says native contract machinery is private. 

Clean this contradiction.

## H1H.3 Root rule

The root should expose concepts an application author thinks in.

Approximately:

```text
View
Scene

Theme
ThemeKey
ThemeColor
ColorSpec
StyleRef
StyleSpec
StyleSelector

Insets
BorderSpec
Grid...
TextSelector
TextSpan

defineView
state
State

Tui
History
TextInput
TextStream
ViewSlot
ScrollPane
Output<T>

TextContent
RawText
Projection
Smooth
Diff*
MarkdownProjector
PlainTextProjector

genuine extension interfaces

TuiError
runtime events
```

## H1H.4 Native names

Names whose primary meaning is:

```text
N-API wrapper
native object wrapper
bridge adapter
ABI implementation
```

should not normally be root exports.

Specifically audit/remove:

```text
NativeOutputHandle
NativeViewSlot
NativeScrollPane
ComponentAdapterBridge
```

## H1H.5 Adapter classes

Audit:

```text
RendererAdapter
ProjectorAdapter
TextVisitorAdapter
TextRewriterAdapter
StreamingSourceAdapter
```

The interfaces may absolutely be public.

The wrapper used internally to cross an execution/native boundary does not automatically need to be.

Remove adapters from root unless an external consumer genuinely needs to instantiate them.

## H1H.6 Testing subpath

Move:

```text
AppHarness
createAppHarness
headless inspection helpers
```

to:

```ts
@iyon/tui/testing
```

The current package exports only root and `native-stage`, so add an explicit testing subpath. 

Do **not** create:

```text
@iyon/tui/internal
```

That merely turns private machinery into a semi-public dumping ground.

## H1H.7 Smoke exports

`tuiSmoke` is infrastructure verification, not application semantics.

Remove it from the normal root unless an actual external consumer contract requires it.

---

# API-H1I — Runtime Contract Truthfulness and Correctness

## H1I.1 Objective

Make `TuiRuntime` describe actual supported runtime behavior rather than historical capability uncertainty.

## H1I.2 Optional methods

Current `TuiRuntime` has optional:

```text
createHistory?
createTextInput?
createViewSlot?
createScrollPane?
interceptPaste?
forwardPaste?
setTheme?
```

even though concrete `Tui` implements these capabilities.

`AppHarness` consequently contains checks for missing methods on the concrete Tui it owns.  

Audit why that optionality exists.

If every supported Tui runtime implements an operation:

```text
make it required
```

If genuinely different runtime capability sets exist:

```text
model the smallest truthful capability interface
```

Do not construct a giant service-container architecture.

## H1I.3 `size` correctness

Current runtime stores initial:

```text
width
height
```

and `size` returns those stored fields.

`resize()` resizes the host but does not update them. 

Therefore this can be wrong:

```ts
tui.resize(120, 40);
console.log(tui.size);
```

Fix it.

Possible correct implementation:

```text
mutable width/height updated after successful resize
```

or querying authoritative host state if such API already exists.

No stale size is acceptable.

## H1I.4 Scene shape

Audit whether:

```ts
new Scene(...)
Scene.from(...)
plain SceneContract object
```

are all deliberately supported.

Current `Scene.from()` simply normalizes a structural `SceneContract` into a concrete Scene. 

If that is intentional, document the structural boundary.

Do not introduce nominal ceremony unnecessarily.

## H1I.5 Direct vs retained render ownership

Do not alter current semantics around:

```text
render(() => Scene)
render(Scene)
builder-root ownership
direct takeover
State subscriptions
root disposal
```

API-H1 may document them but must not redesign them.

---

# API-H1J — Public Contract Parity Audit

## H1J.1 Objective

Systematically compare public TS contracts with their concrete implementations and Rust semantic equivalents.

This is where API drift is caught before PERF-13 builds on top of it.

## H1J.2 Mandatory types

Audit method-by-method:

```text
TuiRuntime / Tui
History interface / History class
TextInput interface / TextInput class
TextStream interface / TextStream class
ViewSlot interface / ViewSlot class
ScrollPane interface / concrete ScrollPane
Theme / Style*
Scene
Output*
Component*
```

## H1J.3 Already known mismatch

Concrete ScrollPane:

```ts
setContent(View | (() => View))
```

Public interface:

```ts
setContent(View)
```

 

There are likely more.

## H1J.4 Do not “fix” parity by broadening types

Forbidden fixes include:

```ts
any
unknown
Record<string, unknown>
View | object
object
```

when the implementation has stronger semantics.

The goal is a truthful API, not eliminating compiler errors.

## H1J.5 Rust/TS parity rule

Rust and TS need conceptual parity where both expose the same framework abstraction.

They do not need syntactic identity.

For example:

```text
Rust builder ownership idiom
    may map to a TS closure

Rust trait
    may map to TS interface

Rust newtype
    may map to opaque branded TS value
```

The semantic concepts should still agree.

---

# API-H1K — Iyon External Consumer Migration

## H1K.1 Objective

Use real Iyon as the external consumer proving the cleaned API is usable.

Do not migrate Iyon by reaching into TUI internals.

## H1K.2 Development workflow

Create/use a framework branch such as:

```text
api-h1
```

Then from Iyon:

```bash
bun run build:iyon -- api-h1
```

The persistent-worktree build resolves the branch's current SHA and rewrites both TS and Cargo dependencies inside the cached integration worktree automatically. 

That is the authoritative integration route during development.

After merge:

```bash
bun run build:iyon -- main
```

must work against the resulting `iyon-tui/main`.

## H1K.3 Known Iyon migrations

### Theme/style

Current:

```ts
Style.new().foreground("theme:truncation_footer")
Style.new().foreground(`theme:${key}`)
Style.new().theme("diff.meta")
```



Migrate to explicit semantic APIs from H1B.

### Components

Where canonicalized:

```ts
View.component(composer)
View.component(slot)
View.component(pane)
```

becomes:

```ts
composer.view()
slot.view()
pane.view()
```

### Testing

Move test harness imports to:

```ts
@iyon/tui/testing
```

### Events

If `nextAction` is removed:

```text
nextAction
    ↓
nextEvent + generic routed-output semantics
```

### Native names

Iyon must not substitute its own casts or deep imports when `Native*` facade exports disappear.

If public API migration becomes impossible without an internal TUI import, that reveals an API-H1 defect that should be fixed in `iyon-tui`.

---

# API-H1L — Consumer Fixture and Regression Gates

## H1L.1 Standalone consumer fixture

The existing fixture is explicitly designed to use only public `@iyon/tui` APIs and no internal framework setup. 

Update it into the API-H1 acceptance oracle.

It should demonstrate:

```text
canonical package import
public semantic styling
explicit theme references
retained State/defineView
normal View composition
canonical control creation
control.view() composition
History
TextStream where relevant
runtime output routing
runtime events
cleanup/disposal
```

## H1L.2 Fixture prohibitions

Fixture source must contain no:

```text
deep TUI imports
Native*
Bridge*
nodeForBridge
nativeObject
nativeHandle
as unknown as internal-handle
"theme:"
private IR types
ABI knowledge
```

## H1L.3 Type-surface gate

Add or maintain a compile-only external consumer test that ensures every type visible in typical public signatures is importable from documented package entrypoints.

This should catch regressions such as:

```text
public method accepts BorderNode
but BorderNode is private
```

## H1L.4 Ownership gate

Existing architecture/ownership gates remain mandatory:

```bash
bun run check:ownership
```

Public surface snapshot changes must be deliberate.

Do not mechanically regenerate snapshots before reviewing the delta.

The snapshot delta itself should be treated as an API-H1 artifact:

```text
removed accidental exports
added proper semantic exports
moved testing exports
renamed false concepts
```

---

# 4. Recommended execution order

API-H1 should be implemented sequentially:

```text
H1A  Public Surface Audit and Closure
 ↓
H1B  Theme / Style / Color Semantics
 ↓
H1C  Opaque Handles
 ↓
H1D  Control Construction and Lifecycle
 ↓
H1E  Component / View Composition Facade
 ↓
H1F  Output / Event Semantics
 ↓
H1G  False Aliases and Compatibility Removal
 ↓
H1H  Root Package Hygiene / Testing Subpath
 ↓
H1I  Runtime Contract Correctness
 ↓
H1J  Full Contract Parity Audit
 ↓
H1K  Iyon Consumer Migration
 ↓
H1L  Consumer / Ownership / Integration Gates
```

Each sub-tranche should ideally leave `iyon-tui` internally green before moving on.

Iyon may temporarily fail to compile between deliberate breaking API commits, but the full API-H1 branch must not be considered complete until:

```bash
bun run build:iyon -- api-h1
```

passes from the actual Iyon repository.

---

# 5. Areas explicitly deferred to PERF-13

The implementation agent should create explicit follow-up notes rather than “fixing” these during API-H1:

```text
View.text() as content-vs-layout category error

text payload leaving structural DAG

ContentSource / ContentLane / Content handles

append-vs-replace content transport

fast mandatory content FFI data path

Kitty graphics / frame/video sources

retained content identity and revisions

content intrinsic geometry revision

paint-only retained node mutations

geometry-only retained node mutations

background/border/focus/blink mutation without DAG changes

theme/state mutation directly in Rust

layout-state vs presentation-state revision model

post-plane-split ScrollPane/content attachment model
```

Likewise current `View` modifiers such as:

```ts
.background(...)
.foreground(...)
.border(...)
.styleState(...)
.padding(...)
```

may not represent the final efficient mutation architecture.

API-H1 should ensure they have sane semantic types, but **must not make their current DAG-lowering behavior a new architectural commitment**.

---

# 6. Important conceptual distinction for the implementation agent

There are three different reasons an API can look ugly:

```text
A. accidental surface drift
B. implementation leakage
C. underlying architecture not yet split correctly
```

API-H1 fixes **A and B**.

Examples:

```text
"theme:foo"
    → accidental API encoding
    → FIX NOW

NativeScrollPane in root exports
    → implementation leakage
    → FIX NOW

StreamPane = TextStream
    → false semantic alias
    → FIX NOW

BorderNode in public signature
    → private representation leakage
    → FIX NOW

fake OutputHandle.payload
    → lying public contract
    → FIX NOW

View.background() creates a new semantic DAG value
    → probably wrong post-PERF-13 architecture
    → DO NOT SOLVE HERE

View.text() contains large text payload
    → known plane problem
    → DO NOT SOLVE HERE
```

That distinction is crucial.

---

# 7. Final expected public shape

Not a frozen exact export list, but after API-H1 the root should conceptually look approximately like:

```text
@iyon/tui

STRUCTURE / COMPOSITION
  View
  Scene
  Insets
  BorderSpec
  Grid...
  OverflowIndicator
  alignment / wrapping semantic values

RETAINED EXECUTION
  defineView
  state
  State

PRESENTATION
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

SEMANTIC CONTENT / RENDERERS
  TextContent
  RawText
  Annotations
  Projection
  Smooth
  Diff*
  MarkdownProjector
  PlainTextProjector
  genuine renderer/projector extension contracts

RUNTIME
  Tui
  History
  TextInput
  TextStream
  ViewSlot
  ScrollPane
  Output<T>

EVENT / ERROR
  TuiEvent
  routed output event
  TuiError
  related semantic error helpers
```

Separately:

```text
@iyon/tui/testing

  AppHarness
  createAppHarness
  deterministic clock/input injection
  screen inspection
  style/cell inspection
```

And not publicly surfaced merely because implementation uses them:

```text
NativeOutputHandle
NativeViewSlot
NativeScrollPane
ComponentAdapterBridge
native objects
bridge nodes
NodeIds
NativeRefs
retained path metadata
generated ABI functions
transport session
tuiSmoke
benchmark counters
```

---

# 8. Definition of done

API-H1 is complete only when:

```text
@iyon/tui is the unambiguous framework package identity.

The public API is closed over public semantic types.

No public signature leaks private IR/native modules.

Theme identity is not encoded through StyleSpec.

No public "theme:<key>" magic strings remain.

StyleRef and StyleSpec have distinct meanings.

ColorSpec expresses semantic theme references explicitly.

Framework-owned handles cannot be forged through accidental
structural typing.

Internal handle access is typed and centralized rather than
recovered through repeated `as unknown as ...`.

Host-bound controls have clear construction and lifecycle rules.

Public control interfaces match their implementations.

Normal component composition does not unnecessarily expose
native component lowering.

Output<T> means one coherent typed channel concept.

No fake payload property exists on channel identities.

TextStream is no longer exported as a fake StreamPane.

Obsolete pre-generic nextAction compatibility is removed if
the usage audit confirms it is dead.

Native implementation classes are absent from the normal root.

Testing utilities live under @iyon/tui/testing.

Tui.size remains correct after resize.

The standalone consumer fixture uses no private knowledge.

The public-surface snapshot delta is deliberate and reviewed.

PERF-12 DAG / retained execution / N-API / FFI / layout /
paint architecture has not been changed.

PERF-13 concepts have not been prematurely implemented.

Iyon builds successfully against the API-H1 branch through:

    bun run build:iyon -- api-h1

and, after merge, successfully against:

    bun run build:iyon -- main
```

The most important success criterion is that after API-H1 a coding agent opening `@iyon/tui` should see **one coherent semantic framework API**, rather than having to infer which names are real architecture, which are N-API plumbing, which are extraction leftovers, and which are placeholders for concepts that do not actually exist.
