# Current architecture of iyon-tui

**Last source verification:** `4355c02d6853549adf32a1e038b14665ce5c6bf8` (after L1 completion).  
**Status:** maintained reference; implementation description, not a V5 design or disposition plan.  
**Companion:** [revision-specific technical inventory](atlas-4355c02/COMPREHENSIVE-REPORT.md).

This guide explains where behavior lives, how a change becomes terminal output, and which ownership boundaries matter when modifying the system. The [atlas](atlas-4355c02/README.md) preserves detailed source maps and evidence. Its [reconciliation register](atlas-4355c02/RECONCILIATION.md) corrects overbroad scout claims; its [findings register](atlas-4355c02/ISSUES.md) keeps unresolved questions visible.

Source is authoritative for **what runs**. Approved, unsuperseded PERF13/API-H/L1/PRE-V5 handoffs remain authoritative for **what was intended**. Describing a mismatch here neither approves it nor dismisses the original contract as stale documentation.

## 1. The system in one picture

iyon-tui is a generic terminal UI framework. TypeScript authors semantic values, owns retained execution and lifecycle coordination, and receives routed outputs. Rust owns native interaction, content processing, layout, painting, clocks and terminal correctness.

```text
 Caller-owned application state and policy
                    |
                    v
 @iyon/tui: semantic authoring + retained execution
 View / defineView / state / Scene / controls / Source / Funnel
                    |
         +----------+-------------+
         |          |             |
    Structure    ViewState     Source data
    generated    fixed patch  direct content
    N-API        N-API        FFI
         |          |             |
         +----------+-------------+
                    v
 Native environment and host
 retained Views, components, state, content, pending epochs
                    |
                    v
 Scene: resolve mounts -> measure -> prepare -> place -> paint
                    |
            prepared candidate
                    |
                    v
 Terminal backend worker -----> actual terminal / native scrollback
                    |
             successful receipt
                    |
                    v
 Promote captured candidate to logical visible state
```

There are two important separations in this picture:

1. **Semantic description versus physical presentation.** A View describes text, rows, grids, styles and component references. It is not a screen of cells. A Source holds authoritative content, not prepainted rows.
2. **Acceptance versus visibility.** A native root or Source mutation can be accepted before a frame is prepared, written and acknowledged. A failed write does not roll back all earlier logical changes or terminal bytes.

The framework must remain usable by a log tailer, editor, dashboard or build watcher without pretending to be an agent application. Provider/model/tool orchestration, application labels and product scene policy belong in TypeScript application contributions, not native framework APIs. The planned iyon-api/iyon-core/iyon-tui/application split is an ownership rule, not a claim that this repository implements every package in that plan.

## 2. Repository and API boundaries

| Location | Responsibility | Start here when changing |
|---|---|---|
| `packages/iyon-tui/src/api/` | Public semantic values and control/content contracts | Public authoring semantics |
| `packages/iyon-tui/src/composition/` | Retained scopes, tracked dependencies, keyed ownership, semantic reuse | Which TS bodies rerun and why |
| `packages/iyon-tui/src/runtime/` | Tui lifecycle, publication, environment wake coordination | Render/flush/open/close and automatic scheduling |
| `packages/iyon-tui/src/transport/` | Structural/state/content encoding and native sessions | Cross-language ingress and identities |
| `crates/iyon-tui-native/src/` | N-API wrappers, native structural runtime, direct content ABI | Native handle validation and wire contracts |
| `crates/iyon-tui/src/application/` | Host/environment/kernel, state/content integration | Candidate capture, scheduling, receipt promotion |
| `crates/iyon-tui/src/scene/` | Scene resolution and retained frame preparation | Mounts, invalidation, layout/paint selection |
| `crates/iyon-tui/src/presentation/` | Semantic View IR, geometry preparation, painting | Layout rules and rendering mechanics |
| `crates/iyon-tui/src/history/` | Ordered semantic scrollback, projection and native transfer | Resident/native history transitions |
| `crates/iyon-tui/src/content/`, `projection/` | Text/diff semantics, projectors, pacing, lowering | Content transformation without product policy |
| `crates/iyon-tui/src/controls/` | Native editing and scrolling controls | Key behavior and component outputs |
| `crates/iyon-tui/src/theme/`, `physical/`, `terminal/` | Semantic styling, cells, actual terminal writes | Color/Unicode/backend correctness |
| `tools/tui-abi*` | Structural schema and build-time generation | ABI evolution and generated consistency |
| Package tests, consumer fixture, Rust tests, benchmarks | Contract evidence and measurement | Verifying the actual changed route |

The supported authoring package is **`@iyon/tui`**, with root, `./testing` and `./native-stage` exports. The Rust implementation is unpublished and is not a supported external Rust authoring SDK. Its curated `iyon_tui::binding` seam serves the native adapter. Public-looking internal Rust traits do not automatically become external APIs through private module boundaries.

`AppHarness` belongs to the testing subpath, not the package root. Factory-created controls such as ViewSlot and ScrollPane are exposed as types; callers obtain them from Tui. Type-only export and private construction are not, by themselves, proof that every implementation member is hidden by the emitted declaration surface.

**Deeper evidence:** inventory§1, reports16/19/24/27/28 and the API/build audits.

## 3. Authoring is semantic; execution is retained

### 3.1 What a View means

A View is an immutable semantic wrapper. Its private node representation describes text, layout, decoration, state/content attachments or a retained component reference. Creating it does not paint cells or immediately cross the native boundary.

Changed semantic values receive new identities; unchanged children can be shared. A frozen View wrapper does not deep-freeze every caller-owned object that could have been supplied to a public helper. Semantic equality, JS object identity and native retained reference identity are different things.

Rows and columns describe tracks and children. Grid builders describe cells, spans and track policies. Styling describes semantic rules and local overrides. Rust later resolves these values for a specific viewport, component/state snapshot and theme.

### 3.2 Why a child can update without rerendering its parent

`defineView` creates a retained component token. A Tui owns one RetainedExecutionRuntime shared by its scene producer and retained control builders. Each evaluated scope records props, dependencies, child ownership, output and semantic operation slots.

```text
 parent scope
   |
   +-- ordinary semantic operations: owned by parent's evaluation
   |
   +-- child defineView scope
         reads State X
         publishes through stable native ViewSlot
                |
 State X changes +--> child dirty --> child reevaluates
                                      parent need not reevaluate
```

A nested retained scope has its own dependency subscriptions and publication target. Its native slot keeps component identity while accepting new semantic content. This is the scheduling boundary; a key alone is not.

Unkeyed children use parent/type/position identity. Keys establish a parent-local ownership namespace for children, but the key thunk itself still runs in the parent scope. Recreating the defineView token remounts rather than preserving the old component identity.

Props use shallow Object.is-based comparison over enumerable string keys. Deep mutations of the same props object are not tracked. Tracked State likewise uses Object.is; reads during evaluation subscribe, and writes inside a component body are rejected.

### 3.3 Prepare, publish, commit

Retained execution stages dependencies, child owners, semantic slots and output before publication. Ordinary evaluation/preparation failure preserves previous committed scope state and retry obligations. The State value that triggered the update is already changed; aborting render does not undo it.

Publications are prepared before commit and committed descendant-first. A pathological commit exception can partially accept native work. This mechanism is not a distributed rollback transaction across JS, native host state and terminal I/O.

Direct rendering or direct control setters can take ownership from a builder. Ordinary preparation must succeed before the old builder is released. Same-body cutoffs still perform required ownership validation and takeover; they are not permission to skip lifecycle work.

Persistent child sequences and retained semantic slots reduce reconstruction, but helpers do not make every public wide-parent update O(log N). Child construction still occurs, and available native path-edit operations are not proof that every composition call selects them.

**Deeper evidence:** inventory§4–5, reports20/21/30/31.

## 4. Three mutation paths, one frame

A useful first question for any change is: **which plane changes?**

| Plane | Typical caller operation | Crossing | Native owner |
|---|---|---|---|
| Structure | Replace body or control View, change semantic children | Generated structural N-API | Shared immutable View graph and desired root |
| Retained state | Patch geometry/presentation/style state on an attachment | Fixed generated N-API envelope | Host ViewState registry |
| Content | Append/replace/clear/seal/truncate Source bytes and annotations | Direct content FFI | Environment Source registry |

### Structure

Materialization resolves weak hints and eligible NodeId lookups, builds missing children before parents, holds temporary native leases, then publishes the root through a retained boundary. Small axes use fixed arity; large ordinary axes use a buffer call. Specialized builder/path/edit helpers are additional capabilities, not universal production routes.

The production TS structural session is N-API. Enabling structural direct-FFI exports or printing a direct-FFI benchmark label does not change that route.

### Retained state

ViewState is native occurrence state, not tracked JS State. A patch can change effective geometry or paint without rebuilding TS semantic structure. Candidate frames capture immutable versions; newer logical versions may exist while an older candidate is still awaiting receipt.

Patches distinguish absence, explicit null/clear and actual value. No-op patches do not advance revision. Geometry changes may invalidate ancestors; presentation changes often repaint a conservative subtree. A style-state patch is not promised to touch only one physical cell.

### Content

Source mutations use the direct FFI data adapter. N-API creates and controls Source/Port/Connector handles and reads diagnostics, but NativeTextSource has no second N-API append/replace payload route.

Accepting bytes validates and stores data; it does not synchronously parse, layout or paint. Host fanout happens after unlocking the Source. A later wake failure reports a scheduling problem without pretending the accepted append was rejected.

### Frame convergence

```text
 desired structure -----+
 logical state versions +--> capture candidate --> prepare scene --> submit
 accepted Source data --+                                      |
                                                               v
                                                   await exact receipt
                                                               |
                                  +----------------------------+
                                  v
                         promote captured visible state
                         leave newer desired work pending
```

Host frames can therefore contain a captured older Source/state version while newer desired data is pending. Exact candidate identity is what makes delayed presentation coherent.

Native `host.render` and slot setters can mutate desired/control state before a subsequent flush fails. The old visible frame may remain authoritative, but the old desired root or control state need not. When adding error handling, name the boundary being preserved.

**Deeper evidence:** inventory§3/5–7, reports17/22/23/29/31/32/35.

## 5. Content is stored once, interpreted per connection

### 5.1 Source, Funnel, Port, Connector

A Source is host-neutral authoritative data within a native environment. A Funnel is immutable policy: format, wrapping, hyperlinks, smoothing and related generic choices. A ContentPort is a host receiving region. A Connector binds Source and Funnel to a Port and owns its active parser/delivery/render state.

```text
                     +-- Connector A / Funnel A --> Port A / Host A
 Source storage -----|
                     +-- Connector B / Funnel B --> Port B / Host B

 shared bytes and absolute coordinates
 independent projection, pacing, width/theme products and visibility
```

A Source can outlive ordinary host disposal while its environment remains alive. Inactive connectors discard derived execution/caches rather than becoming a second authoritative data store. Requested connector selection and visible selection are distinct across pending frames.

### 5.2 Bytes, revisions and compaction

Stream storage uses persistent UTF-8 pages. Snapshots retain Arc-backed versions; append/truncate can share unchanged pages. Absolute Source offsets do not rebase when the head is truncated.

Keep these identities separate:

- Source lifetime identity/generation;
- content generation after logical replacement/clear;
- accepted mutation revision;
- projected/delivery revision;
- submitted and visible frame state.

Sealing prevents further ordinary stream production, not every mutation: head truncation is still allowed by current source. Nor does seal mean smoothing has caught up or the terminal has acknowledged all rows.

Annotations use source-rooted ranges and explicit truncation policies. RawText/RawDomain can retain source pages, but exact semantic TextRun construction copies text into a new Arc<str> while preserving a range. “Persistent Source” does not imply a zero-copy entire text pipeline.

### 5.3 Parsing and semantic lowering

The native semantic text system represents blocks, inline values, marks, origins, annotations and provenance. Plain, Markdown, ANSI and text-diff projectors turn raw domains into that IR. Markdown tracks restart/reference context and stabilizes ambiguous live tables; ANSI interprets supported style/link escapes rather than forwarding arbitrary terminal control.

TextRenderer lowers the IR into generic Views. Lists become hanging layouts, tables grids, code ordinary label/literal views, and images alt text. Structural render policy is separate from Theme.

Typed DiffHunk rendering is another intentional route: it validates structured ranges/lines and lowers directly to Column-based Views. It is not the same contract as permissive text-diff streaming.

The exported TS TextContent/Projection/Smooth helpers are substantially lighter than the Rust semantic system. For example, TS TextContent.markdown tags origin; its render method produces text rather than invoking the native Markdown pipeline. Do not assume those JS values are arbitrary user-defined native projectors.

### 5.4 Pacing without a JS frame loop

Native Smooth publishes stable spans against a caller clock, without splitting a semantic span. Active Connector delivery maintains a grapheme frontier and advances on native deadlines. Caller TypeScript supplies policy/data changes, not a per-tick render loop.

Pure delivery advancement reuses semantic work. A Source revision can still rebuild retained grapheme indexing; a subsequent frame can still acquire snapshots and prepare geometry. Stage-specific zero-work counters are not whole-frame complexity proofs.

Prepared products are keyed by source/semantic identity plus relevant width, theme and delivery dimensions. A PreparedProjectionTicket pins the selected product; paint must not replace it with the newest product at the same width. Current missing-ticket early return has a signal/reachability question recorded in the findings register.

**Deeper evidence:** inventory§7–8, reports08–10/18/35.

## 6. From scene to cells

### 6.1 Scene and component resolution

A Scene combines a body and optional History sideband. Components are retained values owned by a native registry; handles carry IDs, not component ownership. Resolution expands component Views, detects duplicate component occurrences, reconciles mounts/focus/modal state and supplies layout/tick/input capabilities.

The outer body is normalized for scene sizing. This normalization is not recursively applied to every semantic child.

Component callbacks can invalidate work during preparation. Scene convergence is bounded; it can discard a candidate and prepare again. Successful mount-graph reconciliation and successful terminal receipt are distinct boundaries, including for retirement.

### 6.2 Geometry stages

```text
 semantic View + captured component/state/content inputs
                   |
                   v
 measure(width, intent)    intrinsic requirements
                   |
                   v
 prepare(height bound)     allocate tracks and constraints
                   |
                   v
 place(origin, clip)       flat layout nodes and dependency indexes
                   |
                   v
 paint(theme, tickets)     physical cells or requested row window
```

Rows allocate width then remeasure children. Columns measure width then allocate height. Grids solve column/span requirements, remeasure cells, then solve rows. Decorations/state overrides affect effective boxes before inner geometry is prepared.

The flat LayoutTree does not mean the system has no retained semantic trees: measured nodes, caches, roots and components still retain Views. Layout indices are local to a prepared tree, unlike stable component or semantic identities.

Measure/prepare/paint caches use current/previous generations plus explicit dependency invalidation. When precise repair is unsafe, the scene falls back to broader work. Parent caches do not magically encode every descendant state revision.

Row-window painting can avoid a content-height-sized Surface. Ordered pruning requires both child top and bottom coordinates monotonic; otherwise the painter scans in z order. Full ordinary RowViewport, glyph-aware row-window and direct-content window routes differ and need route-specific parity reasoning.

### 6.3 Physical invariants

Cells distinguish transparent, painted blank, grapheme leader and continuation. Wide graphemes must remain whole and continuation geometry consistent. Width policy is extended-grapheme based and coupled to terminal lowering.

Theme-only changes can reuse text geometry while recompiling physical styles. Surface completeness means represented glyphs fit; it is not equivalent to “every cell painted.”

Incremental scene painting carries damage regions, but the current real backend still lowers a full Surface and diffs all cells. Local scene damage and backend output cost are separate measurement dimensions. Cell-wise clear/full viewport copy have unresolved wide-glyph reachability questions; the guide does not claim all clipping routes are proven equivalent.

**Deeper evidence:** inventory§9–10, reports03–06/14/36.

## 7. History is not one scroll buffer

History has three layers, each with a different lifetime:

```text
 Semantic units              Viewport projection          Native frontier
 ordered Views/components -> width/height/window plans -> exact physical rows
 still reflow/retheme         resident visible selection   accepted prefix irreversible
```

### Semantic units

Component-bearing units are Live. Component-free units are Static, even if a nested ContentPort still receives mutable Source data. Semantic freeze finalizes a live unit into component-free static content; it is not physical row freezing and has no general inverse. Only a live tail can be discarded through that route.

History is attached as a scene sideband. Body receives its allocated height and History occupies the remaining track above it. Live components can remain mounted even at zero visible History height.

### Projection

Height keys depend on width and unit identity/content/component revisions. FollowEnd selects the resident suffix; NativeFrontier preserves a blocked front. Nested static content/state invalidation remains a bounded proof question rather than an assumed stale-layout bug.

### Native transfer

Overflow pressure drains an exact prefix in order. A blocking live unit cannot be skipped. Content can supply finalized stable rows before Source seal, but completion and retirement need stronger proof than “some rows available.”

A sink acknowledges a prefix count. Partial acceptance freezes the unaccepted remainder at its old width/style; it is not regenerated from a newer semantic tree. An error logically accepts zero rows even though the actual terminal may have written some bytes before failing.

A later successful visible frame clears the logical unknown-synchronization marker. It cannot reconstruct uncertain native scrollback. Theme changes, layout changes and resize reflow resident semantics, not already accepted physical rows.

This distinction is essential when changing resize or retention behavior. Do not import replay/clear obligations from deprecated LAY-1 documents into the current contract.

**Deeper evidence:** inventory§11, reports07/15/37.

## 8. Input and controls stay native

Real terminal decoding produces key, paste and resize events. Native routing interprets focus, modal boundaries, component capabilities, key commands and paste handlers. TypeScript receives generic routed output events rather than keystrokes merely to reimplement native controls.

Output<T> identifies a typed channel. Route selection occurs when queued output is drained, not at emission. A queued event can route after later registration if it has not already been drained; unrouted events are dropped at drain.

| Control | What stays native | Ownership point |
|---|---|---|
| TextInput | Editing, grapheme movement, wrapping, paste, submit/change, cursor focus styling | Host-owned control plus output channels |
| ScrollPane | Extent/offset, key scrolling, layout notifications | Factory-created builder/direct content owner |
| ViewSlot | Stable component, retained View/frame array, animation deadline/frame selection | Factory-created slot; JS supplies semantic frames on state change |
| History | Live mounts, projection and native row transfer | Scene sideband with strong host lifetime |

TextInput currently emits change output for successful cursor moves as well as text edits. The intended text-only versus editor-state contract is unresolved; an existing no-op cursor test does not settle it.

Slot animation is a specialized ownership path. Native strong Views preserve frames, but current animation/stop code does not perform the ordinary TS currentView/attachment binding commit. This needs lifecycle interpretation, not a claim that frames are automatically leaked or automatically safe.

Real terminal decoding is richer than injected textual key parsing. Focus/mouse events and some injected modifier/media forms are outside current support. Supported injection is not a universal real-input parity oracle.

**Deeper evidence:** inventory§12, reports02/12/13/38/39.

## 9. Theme is policy data, not layout or parsing

Styles are sparse: absence inherits, explicit false clears an attribute, and plain clears attributes without erasing colors. Named styles and local patches are distinct.

The native cascade is:

```text
 inherited physical style
       -> framework named rule
       -> application named rule
       -> local patch
```

Selectors combine positive focus/focus-within and caller-defined state facts. Specificity/declaration ordering applies within a layer. TextSelector uses the same generic style engine through text facts; there is no second hidden text-theme policy engine.

Theme replacement crosses N-API and updates native theme state. Ordinary metric-neutral geometry can remain cached; paint and theme-dependent content products invalidate. Prepared frames retain captured theme identity until their receipt. Semantic text parsing does not need to rerun for a palette-only change.

Missing named style is no-op; missing palette color resolves terminal default. Theme::color resolves in default context, so an unconditional variant can override base; Theme::style is a base lookup.

A semantic History unit remains recolorable until physical acceptance. Exact native rows and partial transfer remainder keep their painted style. Product-specific theme keys/defaults must remain caller policy.

**Deeper evidence:** inventory§13, reports11/33.

## 10. Runtime scheduling and lifecycle

### 10.1 More than one clock or queue

The TS scope queue decides which bodies rerun. The realm wake broker coordinates native environments and pending Tui hosts. The native environment drains host epochs. A host processes bounded actions, input, component ticks and content deadlines. The backend worker owns physical writes and receipts.

These are cooperating owners, not duplicate implementations of the same loop.

Automatic failures block the failed epoch to avoid spinning. A newer mutation or explicit retry barrier can retry. Tui flush bounds incomplete progress, but a reported error is thrown immediately; it does not silently try the same error64 times.

Native waitForOutput advances/flushes its local host. It does not run the environment-wide drain that the TS broker supplies. Abort-aware event waiting has a distinct TS polling path. Deterministic harness clock advance is not the same operation as wall-clock native wait.

### 10.2 Real versus headless backend

The real backend uses a terminal worker plus an input reader. Termwiz provides output/capabilities; Crossterm input/raw-mode/paste. Frames carry asynchronous receipts, with at most one host frame outstanding. Native History insertion/restore/final positioning use ordered command/reply behavior.

Startup uses the normal terminal screen, not alternate screen. It establishes an initial known blank presenter. Known same-size frames diff; unknown/resize/error requires full-screen recovery. Damage metadata is not currently consumed by the backend lowering loop.

Headless mode uses the production host/scene mechanics with deterministic inspection. It can retain native rows for tests. Real native-history diagnostics do not read back a terminal emulator's scrollback.

Explicit headless resize changes sink size. Explicit real resize only invalidates; backend resize events establish actual viewport. Cached requested TS dimensions and physical dimensions need not be identical.

### 10.3 Close, exit and finalization

close unregisters scheduling, releases content/bindings/state/owned handles and retained execution/root state, then disposes native host resources while aggregating cleanup errors. Native close waits outstanding work and restores the terminal without necessarily presenting a final application frame.

exit performs final presentation/positioning before teardown. Wrapper liveness, inner closed state and final Arc destruction are separate. HostHistory can keep a host alive; most control/state wrappers retain it weakly.

Weak native hints do not own native Views. Finalization of TS resource bookkeeping is not equivalent to invoking every native dispose method. Strong semantic reachability is likewise not the same as an explicit-disposal veto: actual leases and native desired/visible/in-flight checks decide.

Several cleanup/safety questions remain open: static mutable runtime references derived from Arc, unsafe Send/Sync around potentially non-Send values, environment-wide staging abort on one host disposal, preparation-time retirement and retry-loop progress. They are documented risks, not silently accepted design guarantees.

**Deeper evidence:** inventory§2/14/16, reports01/18/21/34/40.

## 11. How to reason about evidence

The map found useful tests and counters, but evidence has a scope:

- Reading a test is not running it.
- A workspace consumer is not an independently packed/installed consumer.
- A headless screen assertion is not real-terminal byte/scrollback proof.
- Retained-versus-fresh comparisons can share the same rendering implementation.
- Generated wrapper tests are not independent native semantic tests; substantial real NativeViewRuntime unit tests also exist and must not be omitted.
- Missing native addon can fail eager import/identity validation before a test guard runs.
- Printed benchmark transport labels do not prove dispatch. Current T15 labels do not switch the structural N-API route.
- Feature-gated export qualification, artifact selection and performance measurement are separate checks.
- A source-local invariant gap is not a demonstrated reachable public failure.

Structural ABI generation does not cover the handwritten content ABI. Runtime fingerprints do not include every separate generator input. Native staging copies before all qualification completes. Record actual artifact path, revision, features and route when interpreting failures or benchmark numbers.

The mapping's executed checks are in [final validation evidence](atlas-4355c02/evidence/final-validation.md). No broad test suite or benchmark result is implied by the architectural description.

## 12. Open issues that matter to an architecture change

Use the [full findings register](atlas-4355c02/ISSUES.md) for IDs, source references and next proof. The main groups are:

1. **Native soundness:** exclusive mutable access/lifetime and unsafe thread-transfer assumptions need proof beyond affinity comments or mutex serialization.
2. **Transaction/lifetime boundaries:** desired acceptance versus visible receipt, slot bookkeeping, animation attachments, cross-host staged abort and retirement.
3. **Unsuperseded handoff differences:** concrete content adapter imports, distributed state descriptor guarantees, handwritten content ABI, and lightweight public semantic helper contracts. A simpler implementation may be defensible, but needs explicit comparison/approval.
4. **Concrete bounded discrepancies:** TS style-name regex and false benchmark-route evidence; TextInput cursor-change semantics require an intent decision.
5. **Reachable correctness proofs:** nested History dependencies, wide-cell clear/full viewport parity, and missing-ticket signaling.
6. **Measurement/observability:** conservative invalidation, whole-surface backend lowering, Source-revision grapheme work, cache high water and exact artifact/counter provenance.

No item is an approved V5 move/remove/keep decision. Do not silently “fix” a public behavior while changing unrelated architecture. Preserve generic Rust/TS ownership and ask for the approvals required by repository instructions.

## 13. Keeping this guide current

Update this guide in the same change when responsibilities, APIs, transport routes, lifetimes, invalidation, error behavior or observability change. Keep the revision-specific atlas intact; it is a historical record, not a file to edit until it appears to describe newer code.

### Change checklist

- [ ] State the last verified source revision and the changed behavior.
- [ ] Identify caller policy versus generic native framework mechanics.
- [ ] Update the repository/API map and any newly public types/subpaths.
- [ ] Trace the real producer-to-consumer route; identify intentional alternate/helper/test paths.
- [ ] Name every relevant identity and strong/weak owner.
- [ ] Describe desired, prepared, submitted and visible boundaries separately.
- [ ] Account for pending frames, disposal, replacement, cancellation and failed cleanup.
- [ ] Update cache keys, dependency invalidation, bounds and reclamation triggers.
- [ ] Keep source-rooted offsets/revisions distinct from viewport coordinates.
- [ ] Cover headless and actual terminal/scrollback effects where relevant.
- [ ] Record exact tests/checks executed and evidence limits.
- [ ] Compare approved intended handoffs; link explicit supersession or accepted improvement.
- [ ] Run the relevant public-boundary/declaration/ownership checks; do not infer parity from naming.
- [ ] Link the deeper source/tests and retain removal conditions for temporary paths.

### Navigation by question

| Question | This guide | Detailed atlas |
|---|---|---|
| Why did this body rerun? | §3 |20 composition,30 root flow |
| Why was native structure rebuilt? | §4 |17/22 transport,31 structural mutation |
| Why did a style patch reflow/repaint siblings? | §4/6/9 |06 state,32 mutation,33 theme |
| Why did append not immediately become visible? | §4/5/10 |10 storage,35 streaming,34 scheduling |
| Why does history retain old width/color? | §7 |07 history,37 trace,15 backend |
| Who owns a slot/Source/state record at teardown? | §5/8/10 |39 controls,40 lifetimes,46 follow-up |
| Does a benchmark measure production dispatch? | §11 |41 routes,42 benchmark integrity |
| What still needs proof before redesign? | §12 |ISSUES, RECONCILIATION,46 parent assessment |

For a reproducible mapping procedure and the actual execution/recovery record, see the [architecture mapping runbook](ARCHITECTURE-MAPPING-RUNBOOK.md).
