# 45-follow-up — Focused grouped deviation evaluation

## 0. Baseline, scope, authority, and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Follow-up assignment: `docs/architecture/atlas-4355c02/evidence/grouped-followup-task.md`
- Original assignment 45 report was preserved conceptually and not overwritten.
- This is a focused multi-issue deviation evaluation, not another repository census or report-review tier.
- No source, configuration, generated file, report, lockfile, or documentation file was edited.
- No tests, builds, benchmarks, package staging, native artifact loading, or services were run.
- No dependencies or agents were added.

### Authority rule applied

The follow-up task correctly changes the evidentiary posture from “source describes actual behavior” to a comparison between:

1. **Approved unsuperseded PERF-13/API-H/L1/PRE-V5 handoffs**, which are intended architectural oracles;
2. **Current source**, which proves actual behavior;
3. **Later approved supersession**, if any;
4. **Comparative design evidence**, if the implementation may genuinely improve the original purpose.

I did not treat implementation existence as justification for divergence. I also did not treat deprecated older documents, including LAY-1, as normative.

### Parent-note and report corrections incorporated

The parent notes contain important corrections to the original scout reports. The most consequential ones incorporated here are:

- report 10/27 invented a Source N-API payload mutation route; current native `NativeTextSource` exposes identity/snapshot/stats/control only, and Source data mutation is direct content FFI;
- report 43/45 incorrectly characterized native Rust ABI tests as absent or only stub-based; `view_abi.rs` contains substantial real-runtime/generated-export `cfg(test)` coverage;
- report 43/45 incorrectly treated guarded native-addon success paths as evidence that `nativeViewAbiSession()` returns `undefined`; the addon is eagerly required and identity-checked before test bodies, so missing addon/identity errors occur earlier;
- report 03 overclaimed recursive root normalization; `layout_body` changes only the outer root;
- report 02/09 state Diff-kind concern is resolved for the current representation because Diff lowers to a Rust Column;
- report 08 overclaimed page-backed zero-copy semantic `TextRun`; exact runs copy text into fresh `Arc<str>`;
- report 11/19 theme color semantics are default-context resolution, not base-only;
- report 13 overstated event loss before route installation; queued output is routed at drain time and can be delivered if the route exists before draining;
- report 21’s runtime flush-loop retry wording requires qualification;
- report 39 confirms the stronger control mutation issue: native slot state mutates before a later host flush/render failure;
- report 14 confirms missing-ticket and physical wide-glyph issues are real evidence-backed candidates, but does not prove every route is broken;
- report 37’s native-history recovery gap is narrower than “no recovery”: current recovery clears the logical synchronization marker after a successful frame, but does not reconstruct an uncertain external terminal scrollback tape;
- report 45’s regex finding was already independently confirmed in parent note 23; it is not newly discovered in this follow-up.

### Disposition vocabulary

- **Confirmed source deviation:** current source differs from an unsuperseded handoff requirement, with no demonstrated supersession.
- **Candidate deviation:** source difference is real, but intended equivalence, caller conditions, or the handoff’s level of prescription still require proof.
- **Concrete defect candidate:** source behavior contradicts its own stated contract or produces a potentially unsafe/silent result; runtime impact remains unexecuted.
- **Resolved non-issue:** original concern is explained by source or a valid superseding design.
- **Erratum:** original report wording was inaccurate and must not be repeated.
- **Recommendation:** suggested next decision or proof step, not owner approval and not an implementation change.

---

# A. Transport, public contract, and code generation

## A1 — Missing typed `ContentDataTransport` seam

### Expected contract

The resolved PERF-13 handoff specifies a single raw content-data adapter:

- only `transport/content/ffi.ts` may import `bun:ffi` or construct raw pointers;
- public/API/runtime modules must call a typed `ContentDataTransport` interface;
- tests may inject an oracle implementation;
- production should have one adapter implementation.

Evidence:

- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:5578–5597`
- especially `:5693–5697`:
  - `ffi.ts` is the only raw adapter;
  - public/API/runtime code calls typed `ContentDataTransport`.

The purpose is not merely stylistic. The seam isolates:

- Bun-specific ABI calls;
- synchronous pointer/borrow lifetime;
- payload encoding;
- metadata validation;
- test injection;
- future runtime replacement without public API imports reaching into FFI.

### Actual source

`packages/iyon-tui/src/api/content/retained.ts` directly imports FFI functions:

- `retained.ts:9–16` imports `appendTextSource`, `clearTextSource`, `replaceTextSource`, `sealTextSource`, `truncateTextSource`, and `decodeSemanticStylePayload` from `../../transport/content/ffi.ts`;
- `retained.ts:380–405` invokes those functions directly from `TextSource` methods;
- `retained.ts:453–466` does the same for connector/source-facing methods.

The parent note confirms:

- only `ffi.ts` imports `bun:ffi`;
- there is no second raw adapter;
- no Source N-API payload mutation route exists;
- the missing typed seam remains an architectural deviation, not a justified improvement.

### Production reachability

High. `TextSource.append`, `replace`, `clear`, `seal`, `truncateHead`, and related public content APIs are public package methods. The direct import is on the ordinary content mutation path, not dead code or a test-only path.

### Supersession result

No later approved PERF-13/API-H/L1/PRE-V5 handoff was found that removes the typed adapter requirement. The source’s “one raw import” property satisfies only the weaker half of the contract; it does not satisfy the typed-call-site half.

### Comparative design analysis

The current implementation has one practical benefit: the public retained-content wrapper is thin and avoids a second TypeScript adapter abstraction. However, the costs are exactly those the handoff anticipated:

- public API code knows the FFI function names and transport shape;
- unit tests cannot substitute a typed content-data transport without reaching around the API module;
- Bun-specific behavior is part of the API module’s dependency graph;
- direct validation/encoding helpers become coupled to public object lifecycle;
- a future non-Bun runtime or native transport must either preserve the import path or rewrite public content code.

This is not equivalent to the “one adapter” contract. It is one implementation with the boundary at the wrong module.

### Recommendation

Treat A1 as a **confirmed unsuperseded architectural deviation**. The owner should choose one of:

1. restore a typed `ContentDataTransport` interface and inject the single `ffi.ts` implementation; or
2. obtain/record an approved supersession explicitly stating that direct typed-wrapper-to-FFI imports are an intentional accepted simplification.

The latter should document why test injection, runtime portability, and public/transport ownership concerns are no longer required.

### Remaining proof needed

- Confirm whether any current test or runtime module requires direct imports that would complicate injection.
- Compare `transport/content/control.ts` and `ffi.ts` interfaces to determine the smallest typed seam.
- No broad tests are required for this finding; source and handoff evidence already establish the mismatch.

---

## A2 — Explicit state fields versus the promised `PropertyDescriptor` model

### Expected contract

The resolved PERF-13 handoff describes a finite native property descriptor table:

```rust
struct PropertyDescriptor {
    id: PropertyId,
    supported_kinds: NodeKindMask,
    baseline_effects: EffectMask,
    validator: fn(&PropertyValue) -> Result<NormalizedPropertyValue, StateError>,
}
```

Evidence:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:4306–4319`
- effect flags and descriptor-driven propagation at `:4321–4324`, `:4444–4449`;
- stable `(ViewStateId, PropertyId)` coalescing at `:5845–5853`.

The purpose is a centralized finite state contract for:

- property identity;
- supported node kinds;
- normalization;
- baseline effects;
- dependency propagation;
- coalescing.

### Actual source

Current source uses explicit handwritten/generated lanes rather than a visible unified `PropertyDescriptor` table:

- `packages/iyon-tui/src/api/view/retained-state.ts:118–131` defines accepted semantic node kinds;
- `packages/iyon-tui/src/transport/state/control.ts` explicitly switches over geometry and presentation field names;
- generated state schema defines fixed numeric IDs, masks, lane offsets, and strings;
- `crates/iyon-tui-native/src/tui/view_state.rs` decodes each property by generated constants;
- `crates/iyon-tui/src/retained_state/capabilities.rs:11–95` maps node kinds and validates combinations;
- `crates/iyon-tui/src/retained_state/effects.rs` classifies effects;
- `crates/iyon-tui/src/retained_state/geometry.rs` and `presentation.rs` own field-level sparse values.

The implementation does have stable property IDs and effect classification, but those responsibilities are distributed across:

```text
generated state IDs/masks/offsets
    +
TypeScript field switch/normalization
    +
native envelope decoder
    +
Rust capability mapping
    +
Rust effect classification
    +
Rust record mutation
```

No `PropertyDescriptor`-equivalent table containing all handoff fields was found.

### Production reachability

High. State mutation, clearing, attachment, and frame invalidation all use the distributed field machinery.

### Supersession result

No superseding approved handoff was found. The current generated envelope and typed field API appear to implement parts of the intended model, but no approved document says the descriptor table requirement was intentionally replaced by distributed explicit fields.

### Comparative design analysis

The current design has legitimate advantages:

- generated fixed envelopes minimize per-property dynamic dispatch;
- explicit TypeScript fields give strong public typing;
- native decoding is compact and bounded;
- Rust capability/effect code can remain strongly typed rather than dynamic.

The tradeoff is that the handoff’s central invariant is no longer represented in one obvious authoritative table. This can create drift between:

- generated property IDs;
- TypeScript accepted fields;
- native decoding;
- supported node kinds;
- effect masks;
- Rust mutation semantics.

The existing code may be semantically equivalent for current fields, but that equivalence is not machine-obvious and is not fully generated.

### Recommendation

Treat A2 as a **candidate unsuperseded deviation**, not as a confirmed behavioral bug.

Recommended owner decision:

- either introduce/derive an explicit descriptor table that unifies ID, capability, normalization, and effect metadata;
- or document and prove that the distributed generated/manual representation is a deliberate equivalent implementation of the descriptor contract.

A source-level “all current fields are covered” assertion would be useful even if no runtime table is added.

### Remaining proof needed

- Build a field-by-field matrix:
  - handoff PropertyId;
  - TS field;
  - generated lane;
  - native decoder;
  - Rust normalized field;
  - supported kinds;
  - effects;
  - clear/null semantics.
- Confirm whether any property has inconsistent support or effect classification.
- No claim is made that current state behavior is wrong solely because the central struct is absent.

---

## A3 — Generated ABI schema versus handwritten content packing

### Expected contract

The PERF-13 handoff requires one typed compact content ABI schema to generate:

- TypeScript annotation unions/adapters;
- TypeScript sidecar encoder;
- C/Rust kind constants and validation;
- Rust semantic annotation enums;
- truncation policy;
- round-trip fixtures.

Evidence:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:1615–1654`
- `:5724–5726` for typed annotation sidecar;
- `:2758–2763` requiring no silent annotation loss.

### Actual source

Structural ABI code is generated from `tools/tui-abi/view_abi.toml`, but content ABI packing is largely handwritten:

- `crates/iyon-tui-native/include/iyon_content_abi.h` is handwritten;
- `packages/iyon-tui/src/transport/content/abi.ts` contains numeric constants and lane definitions;
- `packages/iyon-tui/src/transport/content/ffi.ts` manually validates and packs annotations/styles;
- `crates/iyon-tui-native/src/content_ffi.rs` manually decodes/validates content records;
- `crates/iyon-tui/src/application/content.rs` maps native records into Rust content semantics.

Assignment 24 correctly distinguishes the generated structural header from handwritten `iyon_content_abi.h`. The parent note also confirms that Source mutation is direct FFI only, not a missing Source N-API route.

### Production reachability

High. Content append/replace annotations use the handwritten TypeScript encoder and handwritten native decoder.

### Supersession result

No approved handoff supersession was found that removes the “one schema / generated content sidecar” requirement. The current implementation may have completed the content ABI pragmatically without a generator, but that is an implementation choice, not evidence that the original requirement was withdrawn.

### Additional generated-manifest issue

The structural generated manifest omits `buffer_used_of` even though the canonical schema uses it:

- `tools/tui-abi/view_abi.toml:1537–1540`, `:1599–1602`, `:1616–1619`;
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json:1892–1968` and related entries lack the field.

This is separate from the content ABI issue but shows why schema/manifest ownership matters.

### Comparative design analysis

Handwritten content packing may be reasonable when:

- the content ABI is small and stable;
- the C header is intentionally public/hand-maintained;
- Rust and TypeScript need custom semantic validation not expressible in the current generator.

However, it weakens guarantees the handoff specifically requested:

- schema drift between TS and Rust;
- no automatic generated round-trip fixtures for every annotation field;
- manually duplicated kind/flag/payload definitions;
- manual truncation and invalid-range policy;
- no generated synchronization proof comparable to structural ABI checks.

The current tests are evidence of intended behavior but were not executed in this follow-up.

### Recommendation

Treat A3 as a **candidate unsuperseded deviation** with a concrete metadata subissue:

1. Decide whether content ABI generation is still required by the owner.
2. If yes, extend the generator or add a separate content-schema generator.
3. If no, record a deliberate exception and add source-level synchronization checks for:
   - TS constants;
   - C header;
   - Rust decoder;
   - semantic annotation types;
   - generated/reference fixtures.
4. Independently decide whether `buffer_used_of` belongs in the generated manifest.

---

## A4 — Lightweight public `TextContent`, `Projection`, and `Smooth` versus Rust semantic contracts

### Expected contract

PERF-13 describes typed Source/Funnel/Connector/ContentPort and a semantic `TextContent` family:

- `Source` is authoritative semantic data;
- `Funnel` is typed transformation/delivery policy;
- `Connector` is one retained attachment;
- `ContentPort` is a structural receiving region;
- text families use semantic projections, stability/frontier metadata, and typed delivery.

Evidence:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:3538–3545`;
- `:4691–4696`;
- `:4729–4753`;
- `:5140–5142`.

### Actual source

The public TypeScript classes are substantially lighter:

- `packages/iyon-tui/src/api/content/text-content.ts:3–30`
  - `TextContent` is a string plus origin;
  - `markdown()` only tags origin;
  - `render()` creates `View.text(this.value)`;
  - `walk()` visits one whole string;
  - `rewrite()` rewrites one whole string.
- `packages/iyon-tui/src/api/content/projection.ts:3–34`
  - spans use numeric source offsets and text;
  - no Source base/end/stability/sealed metadata;
  - validation permits gaps in some cases because it rejects only `sourceStart < expected`;
  - `Smooth` is only a nonnegative integer offset.
- Rust `crates/iyon-tui/src/projection/value.rs:6–18` retains source base, stable-through, source end, sealed state, and source spans with typed values.
- Rust projectors and smoothing carry incremental and temporal contracts not represented by those public TS classes.
- Parent note 19 explicitly warns not to describe the TS classes as full Rust parity.
- No production connector path was found that accepts an arbitrary JS `Projection`, `Projector`, or `Smooth` object as the native content pipeline.

### Production reachability

The classes are root-exported:

- `packages/iyon-tui/src/index.ts:114`, `:123`;
- `TextContent` is used as a public convenience and `TextContent` can be supplied to content-port APIs;
- standalone `Projection`, `ProjectionBuilder`, and `Smooth` appear primarily as public data/helper surfaces and test/API contracts, not as the built-in native Connector pipeline.

### Supersession result

No later approved handoff was found that clearly declares these public classes to be deliberately lightweight and non-parity. The handoff’s conceptual names are close enough that callers could reasonably infer stronger semantics than implemented.

Nevertheless, current package design could be a deliberate facade choice: public callers use `TextContent` conveniences while built-in native Funnels own the real Rust semantic pipeline.

### Comparative design analysis

The lightweight design avoids exposing Rust-specific stream revisions, stability frontiers, and semantic IR internals through TypeScript. That is consistent with the generic facade boundary and may be desirable.

The problem is contract ambiguity:

- public names resemble full semantic types;
- `TextContent.markdown()` does not parse Markdown;
- public `Projection` is not the Rust `Projection<T>`;
- public `Smooth` does not configure native smoothing;
- public extension traits are not connected to the built-in Connector pipeline.

### Recommendation

Treat A4 as a **public-contract ambiguity / candidate deviation**, not a proven runtime failure.

Recommended options:

1. Narrow names/docs and explicitly label these as authoring conveniences, not native semantic projection objects; or
2. expose adapters that make the relationship to Source/Funnel/Connector explicit; or
3. expand the public types only if arbitrary caller-owned projectors are an approved product requirement.

Do not describe these classes as full Rust semantic parity in integrated documentation.

---

## A5 — `ViewSlot` interface/class merged public type

### Actual source

`packages/iyon-tui/src/api/controls/view-slot.ts` defines both:

- `export interface ViewSlot extends ComponentHandle` at `:50–58`;
- `export class ViewSlot extends FrameworkHandle<"component"> implements ViewSlotContract` at `:101`.

The class has a private constructor (`:121–126`) and is created through `createViewSlot` (`:406–410`). The package root exports only the type:

- `packages/iyon-tui/src/index.ts:62` — `export type { ViewSlot }`.

Runtime factory ownership is through `Tui.createViewSlot`:

- `packages/iyon-tui/src/runtime/runtime.ts:643–651`.

### Production reachability

High for the interface/factory; low for the class as a root-exported runtime constructor. Consumers receive a factory-created object typed as `ViewSlot`.

### Supersession result

No handoff explicitly forbids TypeScript declaration merging. The handoff requires public retained controls and ownership semantics, but the interface/class naming choice is an implementation detail.

### Evaluation

This is mostly a **resolved non-issue / low-value API hygiene concern**:

- the runtime class is not publicly constructible;
- root export is type-only;
- factory ownership is explicit;
- the merged declaration does not expose native internals by itself.

The only concern is generated declaration readability and future API maintenance. It is not evidence of a second public implementation path or ownership leak.

### Recommendation

No deviation judgment is needed unless generated declarations or package consumers observe undesirable merged members. If cleanup is later desired, split the public contract name from the private implementation class, but do not prioritize this ahead of transport/lifetime correctness.

---

## A6 — T15 transport labels versus actual current route

### Expected benchmark contract

Historical PERF-12 T15 compared two candidate arms:

- `napi_default`;
- `direct_ffi_oracle`.

Evidence:

- `docs/history/PERF-12/PERF-12-T15-AUTHORITATIVE-REPORT.md:3–8`;
- `:23–25` says artifacts were staged separately and the direct arm remained behind `direct-ffi`;
- `:29–47` describes 311 cases per arm and N-API/direct-FFI ratios;
- `:96–101` says transport adoption remained an explicit owner decision at that historical revision.

The current approved PERF-13 handoff narrows direct FFI to a single bulk content-data adapter and identifies structural control/data as generated N-API:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:5578–5597`;
- `:3595–3604`.

### Actual current benchmark source

`packages/iyon-tui/bench/perf12_t15_authoritative_case.ts`:

- imports `nativeViewAbiSession` at `:1–2`;
- creates `RetainedRootBoundary` at `:37–42`;
- calls `boundary.prepareInstall(view)` at `:48–52`;
- outputs environment labels at `:84–88`:
  - `candidate: process.env.T15_CANDIDATE ?? "napi_default"`;
  - `transport: process.env.T15_TRANSPORT ?? "generated_safe_napi"`.

The benchmark does not select a direct structural FFI implementation based on those labels. It always executes the generated structural N-API session imported at line 2.

### Production/benchmark reachability

The benchmark is directly runnable and can produce a nominal direct label through environment variables without changing the route. This is a benchmark-integrity problem, not a production-render route.

### Supersession result

Historical T15’s two-arm comparison may have been superseded as a current structural design by PERF-13’s one production structural N-API path plus direct content-data FFI. However, no supersession makes false current benchmark labels acceptable.

### Recommendation

Treat A6 as a **confirmed benchmark-integrity deviation**:

- derive transport/candidate identity from the actual route or artifact feature;
- reject labels that do not match route evidence;
- keep historical T15 two-arm artifacts clearly revision-bound;
- do not call current `perf12_t15_authoritative_case.ts` a direct-FFI measurement;
- retain a separate content direct-FFI benchmark where that route is actually executed.

No benchmark was rerun here.

---

# B. Lifetime, transactions, and safety

## B1 — `Arc<NativeViewRuntime>` converted to `&'static mut` without `UnsafeCell`

### Expected safety contract

The native runtime is shared through `Arc`-owned storage and exposed through N-API/direct-FFI wrappers. Owner-thread checks and environment lifetime checks are expected to prevent invalid access, but they do not by themselves justify mutable aliasing.

### Actual source

`crates/iyon-tui-native/src/tui/view_abi.rs`:

- `runtime_ptr_for_env`: `:1417–1419` converts `Arc::as_ptr` to a raw mutable pointer;
- `runtime_from_handle`: `:1440–1457` returns `napi::Result<&'static mut NativeViewRuntime>`;
- conversion is `unsafe { (Arc::as_ptr(handle) as *mut NativeViewRuntime).as_mut() }`;
- validation then checks `runtime.valid_on_owner_thread()`, not an `UnsafeCell` or exclusive borrow token;
- direct-FFI `runtime_mut`: `:1672–1677` similarly returns `&'static mut NativeViewRuntime` from a raw pointer;
- many generated/direct functions use `runtime_mut`.

No `UnsafeCell<NativeViewRuntime>` wrapper or explicit runtime lock was found around this conversion.

### Production reachability

High:

- ordinary generated N-API structural calls obtain the runtime through the session handle;
- direct-FFI exports are compiled under `direct-ffi`;
- many structural operations use the mutable reference helper.

### Safety evaluation

This is a **serious safety candidate**, not a proven exploitable runtime bug from static source alone.

The following facts are insufficient by themselves to establish soundness:

- the pointer came from an `Arc`;
- owner-thread validation exists;
- N-API calls are expected to be synchronous;
- environment maps track runtime identity;
- direct FFI callers are expected to honor contracts.

A `&mut T` requires uniqueness for its lifetime. Returning it as `'static` from an `Arc` allocation creates a source-level aliasing/lifetime hazard unless an external invariant proves:

- no overlapping calls;
- no simultaneous shared references used by other code;
- no callback/reentrancy;
- no cross-thread direct-FFI access;
- no runtime access after Arc owner/lifecycle transitions;
- no stale raw pointer reuse.

The parent note correctly rejects the original claim that header checks prove arbitrary pointer validity. `runtime_is_registered` exists separately but is not used by `runtime_mut`.

### Supersession result

No handoff supersedes the requirement for safe native ownership and lifecycle behavior. PERF-13’s one-owner/typed-generational-handle principles do not justify unrestricted `'static mut` creation.

### Recommendation

Escalate B1 as a high-priority safety evaluation:

- replace the raw mutable reference pattern with an explicit interior-mutability/exclusive-access design, or
- produce a documented unsafe invariant proving serialized owner-thread execution and all raw-pointer lifetime conditions;
- audit generated and direct-FFI entrypoints together;
- distinguish N-API-safe callers from arbitrary feature-gated C/direct callers.

A repair would require implementation work and is outside this read-only task.

### Remaining proof needed

- actual N-API call/reentrancy scheduling;
- direct-FFI caller contract and whether external callers can overlap;
- runtime teardown versus outstanding session handles;
- Miri/unsafe-code audit or equivalent proof artifact;
- no test was run here.

---

## B2 — Unsafe `Send`/`Sync` host/environment versus non-`Send` callbacks and actual async wait

### Expected safety contract

Rust host/application APIs use caller-defined components, callbacks, output routes, and payloads. The generic Rust core permits non-`Send` application state in ordinary local use. Any native async boundary must not move those values across threads unsafely.

### Actual source

`crates/iyon-tui/src/application/host.rs`:

- `unsafe impl Send for TuiHost` at `:958`;
- `unsafe impl Sync for TuiHost` at `:959`;
- comments at `:955–957` claim non-Send component registries and callbacks are serialized through `inner` and no callback crosses the async boundary.

However:

- `crates/iyon-tui/src/application/environment.rs:131–135` also has unsafe `Send`/`Sync` for `TuiEnvironment`;
- generic input/output structures hold callback closures and typed outputs:
  - `application/input.rs:5–7`, `:31–33`;
  - `application/context.rs:121–127`, `:183–194`;
  - parent note 13 confirms output queues may contain non-Send payloads and route closures;
- `crates/iyon-tui-native/src/tui.rs:982–992` exposes actual N-API async `waitForOutput`;
- it clones the host and awaits `host.wait_for_output()`.

### Production reachability

High for the N-API host:

- `NativeTuiHost` is the native facade;
- `waitForOutput` is an exported async method;
- host/environment are shared through `Arc`/mutex wrappers.

### Evaluation

This is a **serious safety candidate**. Mutex serialization does not automatically make a type `Send`/`Sync` if the protected data contains thread-affine or non-Send values and the object can be moved or accessed on another executor thread.

The key unresolved question is actual N-API async task placement:

- Does N-API guarantee that the async Rust future remains on the creating JS/owner thread?
- Can `wait_for_output()` hold or move an `Arc<Mutex<HostInner>>` between threads?
- Are callbacks ever invoked while the future is polled outside the owner thread?
- Does `HostInner` include non-Send component/output state reachable during the wait?

The source comment is an assertion of intended ownership, not proof of the `unsafe impl` contract.

### Supersession result

No approved handoff was found that authorizes unsafe Send/Sync merely because a mutex surrounds the host. PERF-13 requires host/environment lifecycle and cross-plane synchronization, not thread-affinity relaxation.

### Recommendation

Treat B2 as a high-priority safety follow-up:

- prove or constrain N-API async executor/thread placement;
- avoid marking the host/environment Send/Sync unless all reachable fields satisfy the actual contract;
- isolate async wait state from non-Send component callbacks;
- document whether `waitForOutput` is owner-thread-only or can be polled on arbitrary threads.

No claim is made that the current implementation has already exhibited a cross-thread callback or payload failure.

---

## B3 — Native control mutation is accepted before later flush/render failure

### Expected contract

The resolved PERF-13 handoff says H3 commit is an infallible authoritative state transition and that failed candidate work leaves old desired/publication state authoritative:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:307`;
- `:311–` describes prevalidated/preallocated commit;
- `:4558–4563` distinguishes caller validation, candidate validation, transient preparation, and invariant failure;
- `:684–688` defines poisoning scope.

### Actual source: `HostViewSlot::set_view`

`crates/iyon-tui/src/application/host.rs:239–253`:

1. locks slot state;
2. sets `state.view = view`;
3. clears frames/pending frames;
4. resets frame index/tick;
5. increments revision;
6. releases the slot lock;
7. calls `invalidate_host()`.

`invalidate_host()` eventually calls `inner.advance_and_render()` (`host.rs:437–461` and analogous paths). A later render/prepare/backend error can therefore occur after the slot’s native/application state has already changed.

The parent note 39 confirms this is not merely hypothetical report wording:

- Rust state mutates before `invalidate_host()->advance_and_render`;
- TypeScript boundary rollback releases acquired leases/preserves old TS bookkeeping;
- native slot mutation is not undone.

### Actual TS bookkeeping

`packages/iyon-tui/src/api/controls/view-slot.ts` maintains:

- `currentView`;
- boundary/root ownership;
- attachment bindings;
- native slot refs.

The setter’s transaction logic can preserve or promote TS-side values only around structural publication, while the native host slot has already accepted its new `View`.

### Production reachability

High. `ViewSlot.setView`, animation, TextInput/ScrollPane host control wrappers, and direct native host control methods are ordinary public/native routes.

### Evaluation

This is a **confirmed source-level acceptance/visible-state divergence**, with likely deviation from the intended all-plane transaction semantics.

The distinction is important:

- native slot logical state changes;
- TS retained root/attachment bookkeeping may still refer to the old root;
- host frame/terminal visibility may remain old;
- a later retry must reconcile the divergence;
- the current implementation does not prove atomic rollback of all domains.

This does not necessarily mean the next frame is permanently corrupted; later retained publication may recover. It does mean “failed render preserves old control state” is too strong.

### Supersession result

No later approved handoff was found that permits native control state to mutate before an infallible/rollback-safe publication barrier. The source comments in current code describe ownership intent but do not supersede the handoff.

### Recommendation

Treat B3 as a high-priority transaction deviation:

- either move mutation behind a prepared/committed host transaction;
- or explicitly split “desired logical control state” from “accepted/visible native slot state” and provide recovery/rollback semantics;
- ensure TS attachment/root bookkeeping and native slot state use the same accepted publication result.

Do not claim that current old-frame preservation equals old-root/state rollback.

---

## B4 — Component retirement follows mount-graph preparation, not backend receipt

### Expected contract

Current comments state that retirement waits until a successful mount-graph reconciliation proves a component is unmounted. That protects future candidate preparation from destroying a component still referenced by the committed graph.

Source:

- `crates/iyon-tui/src/application/kernel.rs:53–58`;
- `:113–127`;
- `:128–143`;
- `:696–701`.

### Actual transition

`RunningApp::prepare_frame` returns a successful prepared scene and then:

- `kernel.rs:696–699` calls `reap_retired_components()`;
- this can remove component registry entries before backend receipt/presentation completes.

The host-level frame transaction later distinguishes candidate/receipt visibility, as documented in parent note 03 and reports 01/02/41. If the backend receipt fails after successful preparation:

- the logical/committed mount graph may remain old;
- physical visible frame may remain old;
- retired component object may already have been removed because the candidate mount graph no longer contained it.

### Production reachability

High for native host rendering with deferred backend receipt. Component retirement is public through host controls and can coincide with frame/backend failure.

### Evaluation

This is a **candidate lifetime/transaction deviation**:

- the mount graph is sufficient to decide semantic reachability for the next candidate;
- it is not necessarily sufficient to decide destruction safety for the still-visible committed frame if backend receipt has not succeeded;
- the handoff’s “visible frame” and “accepted candidate” distinctions require careful interpretation.

The code comments call preparation success enough. Parent notes correctly identify the unresolved seam rather than claiming unconditional use-after-free.

### Supersession result

No approved handoff was found explicitly allowing physical component destruction before backend receipt. The current source’s preparation-based policy may be an accepted design if component lifetime is defined by logical accepted scene rather than terminal visibility, but this tradeoff is not established.

### Recommendation

Treat B4 as a substantive lifecycle issue requiring owner decision:

- retain retired components until backend receipt, or
- formally define preparation/mount-graph acceptance as the ownership boundary and prove old visible frames cannot invoke retired component callbacks;
- include failure/retry semantics in the contract.

---

## B5 — Animation/stop paths bypass committed TypeScript attachment binding/root ownership

### Actual source

`packages/iyon-tui/src/api/controls/view-slot.ts`:

- `setAnimationWithRefs`: `:290–326`
  - validates each frame with `prepareAttachmentsForView(...).abort()`;
  - materializes refs;
  - calls native `setAnimationRef*`;
  - releases temporary refs;
  - does not commit attachment binding replacement;
  - does not update `currentView` or boundary root.
- `stopAnimation`: `:365–378`
  - validates with `prepareAttachmentsForView(...).abort()`;
  - materializes a ref;
  - calls native `stopAnimationRef`;
  - releases temporary ref;
  - does not commit TS attachment bindings or root ownership.
- Parent note 39 confirms static `currentView` and boundary state diverge from native animation/current frame.

Native Rust host animation mutates the slot’s current view/frame state:

- `host.rs:283–372`;
- `stop_animation` delegates to `set_view` at `:374–375`;
- tick advances `state.view` and revision at `:386–417`.

### Production reachability

High. `Tui.createViewSlot` is a public runtime factory:

- `runtime.ts:643–651`;
- testing wrapper exposes it at `testing/index.ts:19–24`, `:69–70`;
- root public type exposes animation methods in `view-slot.ts:50–58`.

### Evaluation

This is a **candidate public-control ownership deviation**. Animation is intentionally a native scheduler specialization, so it need not rebuild the full semantic root on every tick. However, the current route allows:

- native current frame to differ from the TS semantic current view;
- attachment-bearing animation frames to be validated but not registered as committed bindings;
- stop animation to install a new native view while TS root/attachments still refer to the old view;
- native strong references to preserve frames whose TS owner has no corresponding attachment lease.

This may be intentional if animation frames are treated as ephemeral native presentation variants under one semantic slot. The source does not make that ownership contract explicit.

### Supersession result

No later approved handoff was found that explicitly exempts animation/stop from attachment ownership. PERF-13 requires backend-neutral attachment handles and one native owner per effective property. Current source should not be assumed compliant solely because animation is specialized.

### Recommendation

Treat B5 as an unresolved but reachable lifecycle issue:

- define whether animation frames are semantic retained roots or native physical variants;
- if semantic, commit attachment/root transitions transactionally;
- if physical variants, ensure every frame’s state/content resources are owned by the slot/native runtime and cannot outlive Tui/environment disposal;
- document why temporary `prepare...abort()` is sufficient.

---

## B6 — Environment-wide staged transaction abort on one host disposal

### Actual source

`crates/iyon-tui-native/src/tui/view_abi.rs:1459–1467` documents and calls `abort_all_edit_txns` on host disposal. The comment explicitly says:

- builders/edit transactions are runtime-scoped;
- ABI begin calls have no host argument;
- clearing all uncommitted sets is the conservative lifecycle boundary.

Parent note 17 confirms this is shared by all hosts in one environment, not host-local.

### Expected ownership principles

PERF-13 says host disposal tears down that host’s retained resources while Sources can survive ordinary host disposal when the environment remains alive:

- `PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md:252–258`.

An environment-wide transaction registry may be acceptable if staged transactions are intentionally environment-owned, but the handoff’s host ownership language makes cross-host abort behavior surprising.

### Production reachability

Conditional but real:

- multiple hosts can share an environment;
- any host disposal can invoke the cleanup;
- staged builder/edit transactions can exist in another host’s native ABI session.

### Evaluation

This is a **candidate cross-host lifetime deviation**:

- conservative cleanup prevents staged native views from retaining dead environment resources;
- but it can abort an unrelated host’s in-progress transaction;
- TypeScript caller sees a failure/stale transaction with no host-local cause;
- no explicit cross-host notification or error category was found.

### Recommendation

Either:

- make transaction ownership host-specific by adding host identity to transaction records; or
- explicitly define environment-wide transaction abort as an environment poisoning/teardown rule and expose a typed cross-host invalidation error.

No owner approval is implied.

---

## B7 — Host `Send`/`Sync`, raw runtime aliases, and cross-host cleanup are related but distinct

The parent notes correctly group B1, B2, and B6, but they should not be collapsed:

- B1 is raw pointer alias/lifetime safety;
- B2 is thread-affinity and unsafe auto-trait safety;
- B6 is logical transaction ownership and cross-host semantics.

A fix to one does not resolve the others.

---

# C. Content, layout, and physical behavior

## C1 — History height cache keys and nested content/state dependencies

### Actual cache contract

`crates/iyon-tui/src/history/unit.rs:12–35` defines:

- `Static(ViewId)`;
- `Content { view: ViewId, projection: u64 }`;
- `Live { view: ViewId, dependencies: ... }`.

`crates/iyon-tui/src/history/projection/mod.rs:172–200` chooses:

- direct `view.content_attachment_id()` → `Content` key;
- otherwise `Static(view.id())` for static units;
- component-bearing units → `Live` with reachable component dependencies.

`crates/iyon-tui/src/presentation/ir.rs:1038–1040` and `:1881–1914` show aggregate flags propagate nested content/state identity through containers, rows, columns, grids, clamps, and viewports.

### Candidate concern

A static History unit containing a nested ContentHost can satisfy `view.contains_content_identity()` while its root `content_attachment_id()` is `None`. The projection code then selects `HistoryUnitLayoutKey::Static(view.id())`, not a key containing all nested ContentPort projection revisions.

A similarly nested retained-state attachment can be present through flags/walkers while the static History key does not include state revision.

### Reachability

Potentially reachable where:

- a History unit is static/component-free at the root;
- its descendants include ContentHost or ViewState;
- nested Source/content/state mutation changes intrinsic height;
- History projection reuses the cached root height.

### Mitigating source

SceneHost maintains retained content dependency indexes and invalidates layout/paint cache view IDs:

- `crates/iyon-tui/src/scene/host.rs:357–428`;
- `:803–826`;
- `:1561–1574`.

History has separate projection/height ownership and is conservatively refreshed in some paths. The parent note 37 says History keys include content projection/live component dependencies, but it does not establish nested static content/state coverage for every shape.

### Evaluation

This is a **substantive candidate cache-key issue**, not a confirmed stale-render bug. The direct-attachment key is robust for direct ContentHost units; nested identity coverage needs source tracing through all History callers and invalidation routes.

### Recommendation

Build a source-only dependency matrix for:

- direct ContentHost History unit;
- container/row/column/grid descendant ContentHost;
- nested State attachment;
- component-bearing Live unit;
- History-only refresh versus root/full refresh.

If nested descendants are invalidated through guaranteed History revision or full cache clearing, document that as the equivalence. Otherwise, expand the History key or dependency propagation.

---

## C2 — Markdown/manual truncation after connect

### Actual source

Current Source API exposes `truncateHead`:

- `packages/iyon-tui/src/api/content/retained.ts:404–405`;
- direct FFI implements `truncateTextSource`.
- Parent note 10 confirms Rust Source truncation:
  - `application/content.rs:2461–2526`;
  - truncation has no sealed guard;
  - truncation operates on valid retained UTF-8 offsets.
- Connector creation validates family/retention and Markdown policy, but this is not a general “connected Markdown cannot truncate” guard.

### Expected semantics

PERF-13 defines Source content generation/absolute UTF-8 coordinates and allows Source replacement/truncation semantics as part of the Source model. It does not establish that mounting a Markdown Connector permanently forbids manual Source truncation.

### Evaluation

This is **not a confirmed defect**. It is a contract question:

- Markdown projection may preserve parser lineage and restart context after source-base changes;
- truncation can require reparse or restart context;
- current code has explicit `InsufficientRestartContext` handling;
- a user-controlled truncation may be valid if the new Source snapshot satisfies the projector’s source-base contract.

The parent note correctly warns not to infer that truncation is blocked merely because a Connector is mounted.

### Recommendation

Document/verify one of:

1. truncation is supported after Markdown connect, with parser restart/reprojection semantics;
2. truncation is allowed only under retention conditions;
3. truncation is rejected while an incompatible Connector is active.

No current source or approved handoff proves which policy is intended. Do not classify this as a deviation without that policy.

---

## C3 — Wide-cell clearing, full RowViewport copying, and row-path differences

### Source paths

`crates/iyon-tui/src/presentation/paint/view.rs` contains distinct routes:

- ordinary row-window paths use `composite_clipped` around `:362–417` and `:727–730`;
- ordinary full RowViewport child painting uses a full child surface and cell-copy/composite behavior around `:499–551`, `:1051–1124`;
- incremental repaint clears rectangles with cell-wise writes around `:871–886` and `:1357–1370`;
- direct ContentHost RowViewport uses a prepared-ticket window and translated clips around `:1127–1255`;
- tests compare nested RowViewport row output to full output at `:1848–1955`.

The parent note 14 narrows the concern:

- row paths use glyph-aware clipped composition;
- ordinary full RowViewport copies child cells and may truncate a wide glyph when child width exceeds destination width;
- cell-wise `clear_rect_with_background` can overwrite a wide glyph/continuation pair;
- no claim is justified that every route is broken.

### Expected contract

`physical/glyph.rs` documents a whole-glyph safety invariant. Physical surface/copy/clear routes should preserve valid grapheme geometry.

### Reachability

Potentially reachable with:

- wide Unicode graphemes crossing a viewport or clear rectangle boundary;
- full ordinary RowViewport (not ContentHost direct window);
- incremental repaint of a region intersecting a prior wide glyph;
- custom borders or direct provider surface writes.

Canonical layout may constrain many normal cases, and `PhysicalRow::validate_cell_geometry` catches some invalid output in tests/debug assertions, but it does not prove all write sequences safe.

### Evaluation

This is a **concrete physical-safety candidate**, with route-specific uncertainty:

- full RowViewport cell copying is not equivalent to row-path glyph-aware clipping;
- clear rectangle logic is cell-wise;
- custom border setter writes physical cells rather than validating glyph width at the setter;
- current tests cover some geometry and RowViewport parity, but no execution occurred in this follow-up.

### Recommendation

Prioritize source-level reachability and focused tests later:

- wide glyph at left/right viewport boundaries;
- full RowViewport with child wider than destination;
- incremental clear intersecting prior wide glyph;
- border glyph width and continuation cells;
- compare full, row-window, direct-content, and incremental paths.

Do not generalize this to all paint paths.

---

## C4 — Missing prepared projection ticket silently skips painting

### Actual source

`crates/iyon-tui/src/application/content.rs:4355–4373`:

- `paint_window_direct` receives the `PreparedProjectionTicket`;
- `projection_for_ticket(ticket)` returns `None` if connector/product is absent or identity does not match;
- `let Some(projection) = ... else { return; }`;
- no `target.physically_complete = false` is set in that branch.

By contrast, an existing incomplete product sets `physically_complete = false` at `:4371–4373`, and later text-geometry lock failure also marks incomplete at `:4409–4411`.

`projection_for_ticket` performs identity matching at `:4518–4530`.

### Expected contract

The prepared ticket pins the product selected during frame preparation. A mismatch should not silently yield a visually incomplete frame; the handoff’s failure taxonomy distinguishes cache miss/recovery from invariant failure and requires explicit handling for important failures.

### Production reachability

Conditional but meaningful:

- a ticket can become stale if Connector switch, Source append, delivery tick, theme change, or cache eviction occurs while a frame is pending;
- code comments explicitly anticipate such concurrent/newer products at `:4364–4367`;
- tests exercise ticket retention, but no runtime execution was performed.

The source intentionally refuses to substitute a newer product. That part is correct. The missing signal is the concern.

### Evaluation

This is a **confirmed silent failure-path candidate**:

- the code intentionally does not paint when the prepared identity is unavailable;
- it does not mark the destination incomplete or emit an error/counter at the early return;
- stale/evicted product behavior can therefore appear as an unchanged/blank region while frame-level completeness remains true.

It is not a fallback bug; refusing substitution is the correct transactional behavior. The issue is observability/failure marking.

### Recommendation

At minimum, distinguish:

- expected cache miss that forces candidate retry;
- invalid ticket/invariant failure;
- connector disposal or environment teardown;
- transient product eviction.

The parent should require an explicit incomplete/error/counter path before treating the silent return as acceptable.

---

## C5 — State patch dependency bits, y-sorted reuse, and cache scope

### State dependency behavior

`crates/iyon-tui/src/scene/host.rs` explicitly documents why ancestor cache keys cannot simply be reused after descendant state mutation:

- `:803–806` evicts changed state dependency paths when structural and state work coincide;
- `:1344–1350` clears both layout and paint caches when state invalidation combines with structural/component changes;
- `:1561–1574` invalidates targeted state paths or clears both caches when mapping is unavailable.

Parent note 03 confirms the design:

- cache entries are node-local;
- parent entries do not carry every descendant state revision;
- broad invalidation is intentional conservative safety.

### Evaluation

The original concern that state dependency bits necessarily produce stale output is **mostly resolved as a non-issue for correctness**:

- invalidation is explicitly conservative;
- layout and paint cache entries are invalidated on state paths;
- full cache clearing occurs when precise paths cannot be established.

The remaining cost is performance and scope:

- broad clears can reduce cache reuse;
- state/structural coalescing may cause more work than a descriptor/dependency graph would;
- no measured baseline counters were run here.

### y-sort/paint reuse

The scene host retains layout/paint caches and incremental component paths, while ViewPainter derives y-sorted children from the retained layout. No source evidence found that y-sort reuse itself causes stale ordering; geometry/topology invalidation falls back to full layout/paint as needed.

### Recommendation

Classify as **resolved correctness concern with an optimization follow-up**. Do not report a stale-cache bug absent a reachable key/invalidation omission. Preserve the performance tradeoff for later measurement.

---

## C6 — Content style-name regex validator

### Source-confirmed behavior

`packages/iyon-tui/src/transport/content/ffi.ts:501–503` contains:

```ts
/\\s|\\0/u
```

The parent note 23 independently evaluated the literal:

- whitespace/tab/NUL are not rejected;
- literal backslash-`s` and backslash-`0` are rejected;
- Rust `application/content.rs:2010–2027` correctly rejects Unicode whitespace/NUL at the payload boundary and accepts backslashes.

### Evaluation

This is a **confirmed TypeScript validator defect candidate**, though native Rust validation provides a second boundary check for direct calls reaching Rust.

Consequences:

- TypeScript can accept a style role/theme key containing whitespace or NUL that it claims to reject;
- TypeScript can reject legal names containing the literal sequence `\s` or `\0`;
- native Rust may reject/accept differently, producing cross-boundary parity inconsistency;
- public error category/message is misleading.

### Recommendation

Treat as a concrete issue separate from the broader content transport deviation:

- align TS regex with the intended validator;
- add source-level/generated parity proof later;
- do not claim the native layer makes the TypeScript contract correct.

No edit or test was made here.

---

## C7 — TextInput successful cursor movement emits change output

### Actual source

`crates/iyon-tui/src/controls/text_input/command.rs:82–115` uses one `changed` boolean for both edits and cursor movement:

- `MoveLeft`, `MoveRight`, word movement, line movement all return booleans;
- `if changed` calls `input.emit_change(cx)`.

`crates/iyon-tui/src/controls/text_input/buffer.rs` confirms successful cursor movement returns `true`:

- `move_right`: `:164–170`;
- `move_left`: `:155–161`;
- line/word movement similarly return true when position changes.

`output.rs:33–49` converts every emitted change into an output payload containing text and cursor position.

### Contract/documentation mismatch

Parent note 12 states:

- output documentation implies user text mutation only;
- actual successful cursor movement also emits change output;
- existing test named `programmatic_mutation_and_cursor_movement_emit_no_change` performs `MoveRight` on empty input, so it only tests a no-op movement.

### Production reachability

High for native TextInput interaction and any registered change output route.

### Evaluation

This is a **confirmed source-contract discrepancy**, with intent unresolved:

- If the output contract means “editor state changed,” cursor movement output is reasonable.
- If it means “text changed,” current behavior is too broad.
- The payload includes cursor position, making cursor movement potentially intentional, but documentation and test naming do not establish that.

### Recommendation

Owner should decide and document whether TextInput change output means:

1. text-buffer mutation only; or
2. any observable editing-state change, including cursor movement.

Then align `changed` classification, public docs, output names, and tests. Do not dismiss this as a stale comment without deciding the intended contract.

---

# Resolved non-issues and false alarms

## R1 — No Source N-API payload mutation route

The original report 10/27 route diagram was wrong. Current native `NativeTextSource` implementation only exposes construction, identity, environment/content generation, snapshots, stats, family, and disposal. It does not expose append/replace/clear/seal/truncate payload mutation methods.

Evidence:

- `crates/iyon-tui-native/src/tui.rs:1089–1238` as independently read by the parent;
- parent notes 10 and 27;
- actual mutation path is `ffi.ts` direct content FFI.

Do not synthesize a second Source payload ingress.

## R2 — Diff state-kind mapping is not currently proven wrong

The mapping `semantic kind 1 → StateNodeKind::Column` is explained by Diff lowering:

- `crates/iyon-tui-native/src/tui/view_abi.rs` Diff parsing lowers through `binding::lower_diff_hunks`;
- `crates/iyon-tui/src/content/diff/render.rs:1–30` lowers Diff to a Column;
- `application/view_state.rs:127–139` therefore matches the lowered native representation.

The remaining issue is documentation/representation coupling, not a confirmed capability bug.

## R3 — Native Rust ABI tests are not absent

`crates/iyon-tui-native/src/tui/view_abi.rs` has substantial `cfg(test)` tests that instantiate real `NativeViewRuntime` and invoke generated exports. Generated wrapper tests are not the only native coverage.

The correct distinction is:

- generated stub tests prove generated signatures/linkage;
- real `view_abi.rs` tests prove substantial native runtime behavior;
- no tests were executed in this follow-up.

## R4 — Missing-addon “guarded success” claim is wrong

Native package tests eagerly require/load the addon and validate identity before the guarded test body. Therefore:

- a missing addon or wrong artifact can throw before the test body;
- `nativeViewAbiSession()` does not generally return `undefined` merely because the addon is absent;
- guards inside test bodies should not be described as proving absent-addon success.

## R5 — Root normalization was overclaimed

`layout_body` normalization changes only the outer root through the nonrecursive `map_node` path. It does not recursively normalize every visited node.

Evidence:

- parent note 03;
- `crates/iyon-tui/src/scene/root.rs:1–100`;
- `crates/iyon-tui/src/presentation/ir.rs:1001–1040`.

## R6 — Event route installation timing

Queued outputs are routed when drained, not irreversibly discarded at emission if no route exists at that moment. An output queued before route registration can be delivered if the route exists before drain. Only already-drained unrouted output is lost.

## R7 — Theme color is not simply base-only

Parent note 11 confirms:

- `Theme::color()` delegates default-context color resolution;
- an empty selector variant can override the base;
- `Theme::style()` is the genuinely base-only path.

## R8 — History native recovery is partial, not absent

Current successful-frame recovery clears logical synchronization uncertainty:

- `application/host.rs:2091–2094`;
- kernel/History/NativeFrontier chain.

It does not reconstruct an uncertain external terminal scrollback tape. Resize/rewrap and physical remainder policy remain open, but “no recovery exists” is wrong.

## R9 — Exact-root is implemented but production reachability remains unproven

This is not a false alarm in either direction:

- implementation exists;
- no ordinary production caller was established;
- do not call it dead or canonical solely from source comments.

---

# Original-report errata

The following original report statements should not be repeated in the final atlas:

| Original area | Erratum |
|---|---|
| Reports 10/27 | Invented Source N-API mutation route; actual Source payload ingress is direct content FFI |
| Reports 43/45 | Claimed absent native Rust ABI implementation tests; `view_abi.rs` has extensive real-runtime unit tests |
| Reports 43/45 | Treated guarded native test success as evidence `nativeViewAbiSession()` returns undefined on absent addon; eager addon require/identity checks happen first |
| Report 03 | Claimed root normalization affected every visited node; it affects only the outer root |
| Reports 02/09/22 | Diff-kind mapping is not a confirmed mismatch; Diff lowers to Column |
| Report 08 | Exact semantic runs are not zero-copy page-backed strings; `TextRun::exact` copies into fresh `Arc<str>` |
| Report 08 | `InsufficientRestartContext` wording in one early trace was inverted; later failure description is correct |
| Report 11/19 | `Theme::color()` is default-context resolution, not base-only |
| Report 13 | “Event emitted before route installation is lost” is too broad; routing happens at drain |
| Report 21 | “64 retries” requires qualification; the flush loop can abort immediately on a pending host error |
| Report 39 | Old native root preservation on install failure is too strong; native slot state mutates before later render failure |
| Report 45 | Regex issue was already independently confirmed by parent note 23; not newly discovered here |
| Report 45 | Source-manifest appendix extent should not be treated as canonical total path count; parent inventory reports a different canonical count |
| Report 45 | Generic framework-boundary negative finding is source-static; do not present ownership checks as executed unless run |

---

# Recommended grouped priority

## Highest priority

1. **B1 — Raw `Arc` runtime to `'static mut` aliasing**
2. **B2 — Unsafe host/environment `Send`/`Sync` with actual async N-API wait**
3. **B3 — Native control state accepted before later render/flush failure**
4. **B4 — Retirement before backend receipt**
5. **B6 — Environment-wide staged transaction abort**
6. **A1 — Missing typed `ContentDataTransport` seam**
7. **A6 — T15 route/transport labels not derived from actual route**
8. **C4 — Missing prepared ticket silently skips paint**
9. **C6 — Literal style-name regex defect**

## Medium priority

10. **A2 — Distributed state fields versus unsuperseded descriptor-table contract**
11. **A3 — Handwritten content ABI packing versus generated schema requirement**
12. **B5 — Animation/stop ownership and attachment divergence**
13. **C1 — Nested History content/state cache-key coverage**
14. **C3 — Wide-glyph safety in full RowViewport/clear routes**
15. **C7 — TextInput cursor movement output contract**

## Lower priority / documentation or design clarification

16. **A4 — Lightweight public content helper semantics versus Rust semantic names**
17. **A5 — `ViewSlot` interface/class declaration merging**
18. **C2 — Markdown truncation after connect**
19. **C5 — Conservative state/cache invalidation and y-sort reuse**

---

# Evidence appendix

## Required assignment and parent-note files

- `docs/architecture/atlas-4355c02/evidence/grouped-followup-task.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/01-application.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/02-components.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/03-scene.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/06-retained-state.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/08-semantic-content.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/09-projection-smoothing.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/10-stream-l1.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/11-theme.md`
- `docs/architecture/atlas-4355c02/evidence/12-controls-scroll.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/13-interaction-output.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/14-paint-physical.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/17-native-structure-state.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/18-native-content-host.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/19-ts-public-api.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/21-runtime.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/22-ts-structure-state.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/23-content-native-transport.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/24-codegen.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/27-rust-wiring.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/28-ts-wiring.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/29-three-planes.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/30-composition-root.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/31-structural-mutation.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/32-state-mutation.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/33-themes-styles.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/35-stream-content.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/37-history-scrollback.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/39-slots-controls-animation.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/40-lifetime-caches.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/41-production-routes.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/42-benchmark-integrity.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/43-test-contracts.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/44-history-doc-drift.md`
- `docs/architecture/atlas-4355c02/evidence/parent-notes/45-coverage-reconciliation.md`

## Authoritative handoffs/source documents

- `docs/history/PERF-13/PERF-13-THREE-PLANE-RUNTIME-HANDOFF-RESOLVED.md`
  - `:252–258` host/environment disposal ownership;
  - `:307–311` H3 commit/old-root authority;
  - `:4306–4319` PropertyDescriptor contract;
  - `:4444–4449` descriptor-driven propagation;
  - `:4558–4563` failure classes;
  - `:4691–4696` Source/Funnel/Connector/ContentPort roles;
  - `:5578–5597` transport split;
  - `:5693–5697` one typed ContentDataTransport adapter;
  - `:5724–5726` typed annotation sidecar;
  - `:5845–5853` state coalescing.
- `docs/history/PERF-12/PERF-12-T15-AUTHORITATIVE-REPORT.md`
  - `:3–8` historical transport candidates;
  - `:23–25` historical arm staging;
  - `:29–47` historical 311-per-arm comparison;
  - `:96–101` historical decision status.

## Current source paths

- `packages/iyon-tui/src/api/content/retained.ts:9–16`, `:380–405`, `:453–466`
- `packages/iyon-tui/src/transport/content/ffi.ts:501–503`
- `packages/iyon-tui/src/api/content/text-content.ts:3–30`
- `packages/iyon-tui/src/api/content/projection.ts:3–34`
- `packages/iyon-tui/src/api/controls/view-slot.ts:50–64`, `:101–145`, `:284–378`, `:406–410`
- `packages/iyon-tui/src/runtime/runtime.ts:643–651`
- `packages/iyon-tui/src/index.ts:62–63`, `:114–130`
- `packages/iyon-tui/src/api/view/retained-state.ts:118–131`
- `packages/iyon-tui/src/transport/state/control.ts`
- `packages/iyon-tui/bench/perf12_t15_authoritative_case.ts:1–17`, `:37–53`, `:84–116`
- `crates/iyon-tui-native/src/tui/view_abi.rs:1417–1449`, `:1459–1467`, `:1672–1677`
- `crates/iyon-tui-native/src/tui.rs:603–607`, `:982–992`
- `crates/iyon-tui/src/application/environment.rs:131–135`
- `crates/iyon-tui/src/application/host.rs:239–253`, `:283–375`, `:420–461`, `:955–959`
- `crates/iyon-tui/src/application/kernel.rs:53–58`, `:113–143`, `:370–377`, `:696–701`
- `crates/iyon-tui/src/history/unit.rs:12–35`
- `crates/iyon-tui/src/history/projection/mod.rs:172–200`
- `crates/iyon-tui/src/history/model.rs:306–374`
- `crates/iyon-tui/src/presentation/ir.rs:1038–1040`, `:1881–1914`
- `crates/iyon-tui/src/presentation/paint/view.rs:362–417`, `:499–551`, `:727–730`, `:871–886`, `:1051–1124`, `:1127–1255`, `:1357–1370`, `:1848–1955`
- `crates/iyon-tui/src/application/content.rs:4355–4373`, `:4518–4530`, `:6625–6635`
- `crates/iyon-tui/src/scene/host.rs:357–428`, `:803–826`, `:1344–1370`, `:1561–1574`
- `crates/iyon-tui/src/controls/text_input/command.rs:82–115`
- `crates/iyon-tui/src/controls/text_input/buffer.rs:155–170`
- `crates/iyon-tui/src/controls/text_input/output.rs:33–49`

## Validation status

This follow-up is static evidence only. No claim in this report means that:

- the baseline test suite passed;
- the native addon loaded;
- T15 or another benchmark ran;
- an unsafe execution path was observed;
- any candidate issue has a demonstrated user-visible failure in a running process.

The strongest conclusions are source/handoff mismatches and source-confirmed contract discrepancies. The parent retains final synthesis, grouping, owner judgment, and any decision to classify or remediate deviations.