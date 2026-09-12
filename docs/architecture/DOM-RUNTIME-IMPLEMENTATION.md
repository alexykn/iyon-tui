# DOM-like runtime implementation checklist

**Scope:** T0 through T7/M2, with T5/M1 locally accepted and the T6 finite
geometry/layout foundation accepted as a separate checkpoint. React is the
only production UI authoring route. The old native View ABI/generated outputs
and ordinary Rust/native ViewState owners are deleted. Direct host rendering
and content cutover remain T6/T7 work; the private current-renderer adapter is
not a permanent architecture.
This document is the implementation ledger for IYON-DOM-LIKE-RUNTIME-HANDOFF.md;
it is not a claim that the M1/M2 migration is complete.

The T6 direct-host checkpoint below supersedes the earlier T4/M1 prose that
describes `application/legacy_scene.rs` as an active adapter. That file is no
longer part of the current source; the historical sections retain the old
tranche record for migration provenance only.

**Baseline:** branch agent/dom-occurrence-runtime, accepted React/T4 source
HEAD `79a8c90261b9c10a3255ea89c2369d4e4a1c8b77`. The handoff and this ledger
describe the accepted pre-deletion route; the current source and
generated-output checks below record the T5 implementation and validation
slice for parent review, not self-acceptance of M1.
The atlas at docs/architecture/atlas-4355c02 is historical navigation, not
the current-source authority.

## Superseding layout-validation authority

The parent architectural decision for T6/T7 is that legacy M1/T6 pixel
parity has no acceptance authority. The canonical React plus Content plane,
Taffy's Flex/Grid/intrinsic sizing semantics, and the typed provenance,
candidate, receipt, Unicode, and clipping contracts are the correctness
oracle. Terminal quantization and clipping are backend realization details.
The archived M1 captures under `/tmp/t6-m1-baseline` and any later capture
comparisons remain useful diagnostics for locating regressions, but they are
not gates, waivers, or reasons to reintroduce the deleted View allocator.
This keeps the shared React/content plane suitable for a future GPUI host;
GPUI implementation remains out of scope for this tranche.

## Progress

| Tranche | Status | Evidence |
|---|---|---|
| T0 — baseline and behavior map | **complete** | Baseline commands and route evidence below; no production source was changed while the baseline was collected. |
| T1 — finite schema and occurrence core | **accepted; committed bb0c599** | Parent source review and focused/ownership reruns accepted the typed schema/core tranche. |
| T2 — qualified native ingress/resource preparation | **accepted** | Parent reviewed the qualified ingress, generated finite payload forms, resource preparation/install path, shared controls, Source lifecycle and rejection regressions. Workspace and Bun suites passed; remaining T1 lint failures are recorded below. No renderer, React, Taffy, or old-route deletion was attempted. |
| T3 — minimal React renderer | **accepted** | Parent reviewed the React shim, speculative instances, journal/acknowledgement path, hook lifecycles, typed portals, finite properties, native resource changes and public consumer. Current-source full Bun suite: 164 passed; native UI commit tests: 12 passed. TypeScript, Biome, generated ABI, binding, ownership and formatting checks pass. Clippy completes with warnings. Acceptance is limited to the minimal desired-state renderer, not T4 frame realization or M1/M2 cutover. |
| T4 — current renderer, controls, exact frame state | **accepted** | Parent reviewed the canonical adapter, sparse resource synchronization, native controls/events, exact frame and geometry ownership, metadata-only completion, accepted History lifecycle, asynchronous physical transfer, close joining, and failure/replay barriers. Broad integration checks and the final zero-progress close correction passed; evidence and remaining migration gates are recorded below. |
| T5 — M1 TypeScript cutover/publication deletion | **parent source/design accepted; local validation passed; Linux CI pending** | React is the sole production UI route. Native deletion checkpoint `e96d0b3` removes the old View ABI/schema/generated outputs, N-API View calls/classes and ordinary Rust/native ViewState owners. The separate animation correction preserves native ticking, persistent stop, receipt ordering and retirement. |
| T6 — direct terminal Taffy integration | **direct host implementation checkpoint; broader contract/performance review remaining** | Pinned Taffy, generated finite geometry, direct Box/control/History host route, bounded content capture and receipt/control/History regressions are implemented. Archived captures may diagnose behavior, but legacy pixel parity is not an acceptance gate. Package/native-addon evidence, contract review, performance review and Linux native CI remain pending; this is not final T6 acceptance. |
| T7 — content lowering and M2 deletion | **latency-isolation implementation tranche in progress** | Shared bounded content projection, nonblocking Taffy layout, and worker-owned direct paint are implemented. Semantic-content/adapter deletion and the full receipt, close, fairness, and Linux gates remain. |

### T5 canonical React resource seam (current source)

The canonical caller setup is now `Tui.open` → `createReactRoot(tui)` →
`Tui.contentPort()`/`root.render` → a visibility barrier. `contentPort()`
rejects before root creation and no longer calls the legacy native host
`contentPort()` factory. Explicit Port and Connector facades are nominal,
caller-owned values. Their create/select/deactivate/dispose operations use the
same React `CommitCoordinator` revision stream as occurrence commits; React
unmount detaches them without disposing them, and explicit disposal is
rejected while the accepted resource graph still uses a resource.

The native status seam reports desired/confirmed Connector state and Port
mounted visibility from the current host execution owner. The native Port's
confirmed adapter identity is promoted only for touched receipt products;
superseded adapters retire after receipt-safe Source cleanup. Status snapshots
are not cached. Failed selection retains the confirmed product, successful
replacement clears it, and unmount clears visibility. React refs expose the finite
`interceptPaste(routeId)` operation, resolved atomically against the accepted
Editor and existing native paste router.

The approved M2 residue is limited to private current-renderer recipe/layout
internals, native control mechanics still used by the private adapter, and
History/content helpers that independently own behavior. None is a public
View authoring route or ordinary UI state authority. The deletion gate is T7:
direct Taffy and semantic-content realization plus receipt/History/input
witnesses must pass before deleting `application/legacy_scene.rs` and its
superseded renderer internals. Legacy pixel captures may diagnose the
replacement, but they do not authorize retaining that allocator.

### T5 canonical cutover — parent acceptance evidence

Parent review corrected duplicate root construction, lost/duplicated output
after cancellation, missing automatic diagnostics, stale Connector selection,
and native confirmed-identity/adapter-retirement defects before acceptance.
Explicit selection now changes from accepted selection records, not from an
unrelated React property update. A single FIFO output owner serves runtime and
harness callers. Native diagnostics have a bounded queue and explicitly report
overflow rather than silently losing notifications; malformed N-API diagnostic
identities fault the observer/root instead of being defaulted or truncated.

The native global-key test now asserts the required global-before-local
contract, including continued local handling of unbound keys. Its prior failure
was caused by this slice's intentional routing change, not an unrelated baseline
failure. Public tests exercise focused global routing, intercepted paste,
`forwardPaste`, stale refs, and physical History transfer on exit. The real
candidate-interruption witness remains intact.

Final normal addon after this validation pass (darwin-arm64, default N-API):

    packages/iyon-tui/native/iyon-tui-native.node
    SHA-256 64c2ac3c1541d15023413202591efdbb87810886ef3a590bdab7bd2229a30a05
    6,799,168 bytes

Validation pass checks: cargo fmt; workspace check and all-features tests (743
`iyon-tui` tests, 18 native library tests, one native sync test, 9 generator
tests, and the existing ignored History trace doctest); strict project Clippy;
generator check and regeneration; TypeScript, pinned Biome format/lint/
complexity, declaration, binding and ownership gates; canonical default staging;
packaged smoke, canonical Source content-FFI tests, and 80 Bun package/consumer
tests with 341 expectations. The
Biome and Clippy commands exit successfully while retaining their existing
warn-mode audit backlog. The instrumented React content benchmark also ran
against actual Rust counters (`ION_CONTENT_BENCH_COUNT=1000`) and the default
addon was restored afterward. Parent source/design review accepts the M1
boundary; Linux x64 execution remains an explicit CI gate, not a local result.

The final integrated workspace tests and strict Clippy gate were rerun after
the animation correction. A final test-only extension checks two subsequent
ticks after Stop and passed its focused regression. Generator, TypeScript,
Biome, declaration and binding evidence is reused from unchanged source.
The default addon, smoke and 80-test Bun evidence remains applicable because
the later receipt-test corrections did not change production source. The
final instrumented benchmark completed 1,000 appends with 10 submitted UI
records, one delivered output event, zero content registry port scans and a
confirmed visible receipt. Restoring the default addon reproduced the hash
above.

The benchmark now reads actual opt-in Rust projection/layout/paint counters,
counts submitted UI records separately, and observes native output and a visible
receipt. Parent ran it with `perf-counters`, then restored the normal addon above.
Final parent logs are `/tmp/t5-m1-final-*.log` and
`/tmp/t5-m1-parent-final-animation.log`; reused addon/Bun evidence is in
`/tmp/t5-animation-*.log`.
External migration and benchmark instructions are in
`docs/migration/REACT-RUNTIME.md`. The T4/prerequisite sections below retain their
historical validation results; they do not override this current slice status.

### T7/M2 performance and exact-traffic witness — current source evidence

This is implementation evidence for the remaining T7/M2 gates, not a claim that
T7/M2 or the complete handoff is accepted. `packages/iyon-tui/bench/react_content.ts`
now exercises the production React + `AppHarness` + occurrence + Content/native
route. It separates initial mount from post-mount mutation, records raw samples,
and reports p50/p95/p99 in milliseconds plus absolute transport bytes. The
default addon reports JavaScript acceptance/barrier timings and exact UI
traffic. An opt-in `perf-counters` stage additionally reports native-owned
`HostInner` preparation, scheduler advancement, physical submission, and frame
completion nanoseconds. The instrumented stage lane also records content
capture/refinement, the DirectDriver request/response interval, aggregate Taffy
layout time and pass count, and physical paint. The timer type, `Instant` reads,
and timer guards are compiled only under `perf-counters`; the default addon
contains no timer labels or `tuiPerf*` symbols.

The exact traffic witness passed on current source at `665c0a8`:

| Scenario | UI calls | UI records | semantic bytes | records |
|---|---:|---:|---:|---|
| same normalized output | 0 | 0 | 0 | — |
| callback identity only | 0 | 0 | 0 | — |
| one local background change | 1 | 1 | 0 | `SetDeclared` |
| static text replacement | 1 | 1 | 7 | `ReplaceLiteral` |
| keyed move | 1 | 3 | 0 | `InsertBefore` only |
| Source append | 0 | 0 | 0 | Source stats: 7 accepted/copied UTF-8 bytes |
| native animation tick | 0 | 0 | 0 | — |
| native environment recolor | 0 | 0 | 0 | — |

The focused owning-layer regression `post-mount traffic carries only the
changed semantic lane` passed with 14 expectations. The package/consumer Bun
suite then passed with 87 tests and 395 expectations. These witnesses use no
fake renderer.

The bounded matrix uses one warmup and seven measured samples per workload,
with 16 appended lines per mutation (the count is configurable but capped at
256). It covers a 48-row stable tree leaf style change, 64-item keyed reorder,
12-level local insertion/removal, one Source at widths 20/80/160, steady and
burst Markdown with native smoothing on/off, native Editor, native Animation,
resize/theme/scroll, natural delayed-receipt mount/unmount, and shared Source
versus duplicated occurrence memory. `sourceRetainedBytes` is the native
retained Source statistic; Source accepted/copied counters are cumulative;
process RSS and JS heap are reported separately and are not native allocation
counts. With fewer than 20 samples, p99 is marked exploratory-max-adjacent and
raw samples remain authoritative.

Raw JSONL-style JSON reports from the latest runs are:

    /tmp/t7-m2-stage-profile-665c0a8.json
    /tmp/t7-m2-current-default-665c0a8.json
    /tmp/t7-m2-current-default-665c0a8-b.json
    /tmp/t7-m2-perf-baseline-latest-a.json (historical, cross-version comparison withdrawn)
    /tmp/t7-m2-perf-baseline-latest-b.json (historical, cross-version comparison withdrawn)
    /tmp/t7-m2-current-instrumented-final-c6acc54.json (historical)

The current default staged addon is darwin-arm64, SHA-256
`595fec71c4c8b8a3e044eae4d546789128623d1b02a658a1ad7df1c3be392952`.
The separate instrumented addon is SHA-256
`0deaef80e91c42d15a43ad7ee613af478b93b16424c520eb423dfe9a5c5cffe8`; it was
used only for native counters/timing and was not used as the comparison
artifact. The immutable M1 baseline remains unchanged at
`/tmp/t6-m1-baseline/iyon-tui-native.node`, SHA-256
`64c2ac3c1541d15023413202591efdbb87810886ef3a590bdab7bd2229a30a05`, source
archive SHA-256
`9a42664709cd367af0b97e9807925f5f7896f9f063a4cd9ae93800f178386d7b`, and
schema hash `b5d1fe98d102d16d7f9533ff2044e36b675ef993fda62b8866583a45b77376e0`.
The baseline addon and its source archive both load as files, but loader
compatibility is not schema or semantic compatibility. The current generated
schema is `aa92c1c46995f6daeab87f08ef493788c1b96ad6cb842a7322220675e9d1691e1`,
and its normalized descriptors differ materially from the archived schema:
current adds `SizeMode` as value kind 1 and removes the archived Dimension,
F32, Display, Direction, FlexDirection, FlexWrap, Position, AlignmentMode,
GridAutoFlow, InsetsF32, TrackList, and GridPlacement kinds. The current
occurrence property table and field encodings likewise differ. Both schemas
retain batch magic/version 1, so the old addon can load and may accept some
overlapping records; that successful process/operation result does not prove
the same structural meaning, rows, or layout contract.

The prior current-source-versus-archived-addon p95 comparison is therefore
withdrawn as invalid and is not a performance gate or waiver. The current
benchmark now rejects `ION_TUI_NATIVE_ARTIFACT` overrides so this unsafe
cross-version comparison cannot be repeated accidentally. A valid archived
comparison requires running the archived benchmark/source imports from
`/tmp/t6-m1-baseline/source` with the hash-verified archived addon, and then
comparing only workloads whose operation, content, rows, and receipt contracts
are shown equivalent. That separate archived-source rerun was not performed in
this lane. Current-source stage timings above remain valid profiling evidence;
no cross-version p95 claim is made and no Linux x64 execution is claimed.

The additional stage profile `/tmp/t7-m2-stage-profile-665c0a8.json` (current
source commit `665c0a8`) used the same production route, one warmup, seven
measured samples, and append count 16. The
median stage timings (milliseconds; `TaffyLayoutPasses` is the median count)
were:

| Workload | frame preparation | content capture | DirectDriver request | Taffy layout | Taffy passes | physical paint |
|---|---:|---:|---:|---:|---:|---:|
| stable-tree-leaf-style | 2.288 | 0.059 | 1.800 | 1.514 | 4 | 0.324 |
| wide-keyed-reorder | 6.025 | 0.547 | 4.626 | 4.487 | 4 | 0.248 |
| source-width-80 | 1.523 | 0.248 | 1.170 | 1.154 | 4 | 0.098 |
| markdown-steady-smooth-native | 15.696 | 3.918 | 11.383 | 11.067 | 64 | 0.128 |
| native-editor | 0.043 | 0.000 | 0.026 | 0.005 | 2 | 0.007 |
| resize-theme-scroll | 3.563 | 0.271 | 2.797 | 2.601 | 14 | 0.397 |

The profile confirms that the current interactive path waits on the existing
Taffy worker response and that content capture can also be material on a
content lane. Taffy and the DirectDriver interval are the dominant measured
costs; physical paint is not the source of the multi-millisecond keyed or
steady-Markdown preparation cost. Steady smooth Markdown reaches 72 Taffy
passes because the bounded workload submits 16 Source appends and native
smoothing wakes; this is an architectural latency-isolation signal, not a
reason to weaken content semantics or chase a local Markdown micro-optimization.

### T7/M2 latency-isolation design — async implementation tranche landed; full gate remains

The current route now has an asynchronous latency boundary for content
projection, Taffy layout, and direct paint. The following observations describe
the implemented ownership and the remaining synchronous preparation around it;
they do not claim that the M2 adapter-deletion gate is complete:

- `NativeTuiHost::commit_ui` accepts a desired occurrence transaction while
  holding `Arc<Mutex<HostInner>>`. It releases that guard before calling
  `render_host_after_mutation`; that helper reacquires the same guard only for
  short acceptance, capture, and worker-request transitions, then drains the
  environment queue outside the guard.
  Public input, resize, theme, and native-control mutation methods use the same
  release-then-render shape (`application/host.rs`).
- The `iyon-native-environment` thread scans the registered hosts and invokes
  `service_native_deadline_inner` and `drain_pending`. The host guard still
  owns short acceptance, capture, and candidate transitions, but
  `ContentHostRegistry::prepare_connector_projection` now captures an
  immutable `HostContentSourceSnapshot` and submits a typed task to the one
  bounded executor shared by the environment's hosts. Parser state and the
  width-independent semantic cache are retained by that executor; completion
  is installed on a later environment-queue turn.
- `SceneHost::prepare_direct_at_with_content` now calls
  `DirectDriverHandle::request_layout`/`poll_layout` and
  `request_paint`/`poll_paint`. The `iyon-tui-layout-{host_id}` worker owns
  Taffy and direct paint; the host performs only the short mount/geometry
  feedback transition and never waits on a layout or paint response.
- The `iyon-terminal` worker owns termwiz I/O and `TermwizPresenter`. Normal
  `begin_frame` only sends an ordered command and returns a oneshot receipt;
  `presenter.present` uses `presented.diff_screens` when its shadow is known.
  Normal host polling does not wait for that receipt, while explicit
  presentation barriers and close intentionally wait outside the host guard.
  The existing ordered presenter/diff shadow is the physical-output authority;
  this design does not add a second backend or a second damage model.

The implementation is one replacement ownership path: a bounded shared
executor, not one OS thread per Source or Connector, with the following state
and transitions. The remaining proof work below is still required before
claiming full T7/M2 acceptance.

#### Content executor and exact product identity

1. Move parser/execution state and width-independent semantic-cache ownership
   out of the synchronous `HostInner` preparation path into a bounded shared
   content executor. The executor keeps an ordered state record per Source
   identity and generation, and uses a fair queue or fixed worker set. A Source
   snapshot is captured quickly at the Source lock, then its immutable storage
   `Arc` pins the bytes needed by the job; no HostInner guard is held while the
   parser or projector runs.
2. Split the job keys. A semantic job is keyed by
   `(source_id, source_generation, content_generation, source_base,
   source_end/revision, sealed/head_partial, funnel kind/options and
   hyperlinks)`. It produces one immutable semantic product that can be shared
   by every requested width. A width/backend realization is separately keyed
   by `(semantic product identity, width, wrap/funnel policy, delivery
   frontier, theme/style paint facts, finalized-prefix requirement, and
   physical-row requirement)`. Multiple widths for one Source must reuse the
   semantic product and must not parse the Source again.
3. Preserve the current exact provenance fields rather than introducing a
   pointer-only cache key: source identity/generation, content generation,
   source frontier, immutable product identity, Connector selection, confirmed
   A versus candidate B, delivery revision, theme/style facts, and History
   finalized-prefix/physical-row requirements. A failed realization is recorded
   against its exact key and invalidated for retry; it is never returned as a
   successful zero-height or old-width product.
4. Coalesce only pending computation. Source append bytes, output events, and
   presentation receipts are accepted and counted in order and are never
   coalesced. If a newer append arrives while an older compatible prefix job is
   finishing, the older monotonic product may become the latest *ready*
   product and can be displayed while the newer job runs. Replacement,
   truncation, retired generation, Connector switch, policy/theme mismatch,
   or wrong width makes the older product incompatible; that completion is
   retained only for diagnostics or dropped after releasing its pins.
5. Bound executor work and retained data. Use a fixed maximum number of queued
   jobs/bytes, per-Source latest-desired slots, round-robin/fair worker
   scheduling, and parser checkpoints for best-effort cancellation. A full
   queue must produce an explicit pending/backpressure state, not unbounded
   allocation and not silent Source mutation loss. Cancellation must not
   interrupt unsafe parser state; completed work is filtered by its exact key.

#### Nonblocking frame request and publication

1. Keep `HostInner.frame` (confirmed A) as the sole visible scene authority.
   Accepted desired UI/Source state may request work, but a ready content
   product is not a candidate and a candidate is not confirmed visible. A
   content-completion record carries host identity, request/attempt ID, desired
   UI revision, source/product key set, viewport width/height, theme revision,
   and any History frontier identity.
2. Replace the blocking DirectDriver request/response use with a bounded,
   nonblocking handoff to the existing Taffy-owning layout thread. A layout
   request consumes immutable occurrence snapshots, control snapshots,
   matching immutable content products, and theme/style facts. The worker runs
   Taffy and returns an owned geometry/paint-input result. The host performs
   only the existing short mount-graph and geometry-feedback transition; if a
   feedback callback changes a control, it submits another bounded layout
   request. Once geometry is stable, a paint request carrying that immutable
   layout, graph/focus facts, and content products runs `paint_direct_layout` on
   the same layout thread and returns an owned `Surface`. This remains one
   general renderer, not a second occurrence tree or allocator, and React/Taffy
   semantics remain authoritative. The host must poll or receive completion
   wakes after releasing `HostInner`; it must never call `recv` while admitting
   input or servicing an interactive frame.
3. Width misses are explicit. A Taffy measurement callback may consume only a
   matching captured width product. It may not parse/project, wait, or call a
   completion callback. If the requested width has no ready realization, the
   layout request reports `NeedsProduct(width, key)` and the executor schedules
   it. The confirmed old scene and its old geometry/clip remain in force until
   a matching replacement is ready. When no confirmed scene exists or the
   backend dimensions changed, use one backend-independent bounded loading
   policy; never claim the old height valid for a new width.
4. Control-only work remains interactive. An editor, focus, animation, or
   other native control update may be accepted and rendered against the
   confirmed content product without waiting for unrelated Source projection.
   A required Taffy reflow is requested asynchronously and is published on its
   completion wake; it is not deferred until a frame-vsync tick and it does not
   block input admission. If control geometry feedback changes state, the
   existing bounded feedback/reflow loop schedules another immutable request.
5. A matching layout result enters a `Prepared` candidate state only after the
   host verifies that its desired epoch, viewport, selection, content product,
   theme, and History frontier still match. The physical worker receives that
   candidate in order. The receipt pins the candidate's immutable content
   products and Source snapshots until the actual receipt succeeds or fails.
   Successful receipt runs the existing environment-owned commit promotion;
   failure aborts B, retains A, marks physical synchronization unknown when
   required, and releases candidate pins. A stale completion can never promote
   visible state, even if its process/worker operation succeeded.
6. Completion wakes are event-driven. The content executor and layout worker
   notify the environment, which queues the host fairly. The host performs a
   short nonblocking state transition and either submits an available matching
   candidate or keeps A visible. It does not repeatedly prepare idle frames,
   and no content result is made visible merely because it is ready. Ordered
   terminal commands and receipt wakes remain the only physical publication
   barrier.

#### State and cleanup ownership to change

The implementation should remove, rather than parallelize, the current
synchronous owners in the touched path:

- `ContentHostRegistry` retains authoritative Source/Port/Connector selection,
  confirmed/candidate records, failure state, and receipt-safe cleanup, but its
  mutable parser execution and width projection work move to the one shared
  executor. `measure_content`/`prepare_connector_projection` must no longer
  perform parser/projection work from a Taffy measurement callback.
- `DirectDriverHandle::layout`'s blocking response contract and host-side
  `paint_direct_layout` call are replaced by one nonblocking request/completion
  contract on the existing layout thread. Do not add a second occurrence tree
  or manual allocator.
- `PresentationState`, the confirmed frame, environment queue fairness,
  terminal presenter shadow/diff, and typed physical receipts remain. Their
  ownership is not duplicated in the executor. The old candidate is discarded
  only through the existing abort path after a matching receipt outcome.
- Source cleanup and product pins are released only after the corresponding
  candidate abort or successful receipt promotion. Close first cancels new
  scheduling, then joins in-flight content/layout/physical work outside
  `HostInner`, and finally performs the existing authoritative cleanup. A
  completion arriving after close is rejected by host identity/generation and
  drops its owned pins.

Focused deterministic tests must hold a projection or layout job at a
test-owned barrier, then prove that an unrelated control/input desired update
can be accepted and that the confirmed scene remains observable without a
blocking `DirectDriver` receive. Releasing the job must prove that only the
latest compatible product can become B and then visible after its physical
receipt. Additional tests must cover resize/width-miss stale completions,
replacement versus append ordering, failed-measure invalidation, close-time
pin release, bounded burst queues/fairness, control updates during a content
stall, receipt loss, and the no-idle-frame rule. Existing exact UI traffic,
Source accepted/copied-byte, Unicode, History, and presenter diff contracts
remain required regressions; none may be weakened to make asynchronous work
appear complete.

The source now implements this asynchronous boundary, but this section is not
an acceptance waiver: full T7/M2 still requires the deterministic receipt,
stale-generation, close, fairness, idle-frame, and cross-backend evidence, then
deletion of the temporary legacy adapter and redundant general layout path.

### T4 handoff boundary

The React route now installs accepted UI mutations into the same native host
that owns terminal presentation. `UiResourceOwner` is the sole occurrence,
resource, and Source-membership authority; `application/legacy_scene.rs` is a
private one-way recipe projection into the existing renderer and is explicitly
scheduled for deletion at T7. Literal ContentHost occurrences and Source-backed
Connectors are adapted into the existing ContentProvider, including source
subscription/wake behavior. The terminal host drains accepted work without a
required TypeScript pump, and `whenVisible` observes the native visible
revision after that drain.

The Rust coordinator now stores one typed `PresentationState` in
`HostInner`; the superseded correlated candidate fields and receipt slot are
removed. A separate bootstrap receipt exists only for the initial physical
frame, which has no desired UI candidate. T5 still owns old-route/publication
deletion; T6 owns direct Taffy layout; T7 owns semantic content lowering and
deletion of the legacy adapter. The component-only Surface migration, explicit
physical export policy, and GPUI host remain later work under the separate
Surface gate; they are not T4 acceptance claims.

### T4 implementation and integration-gate evidence

This records the accepted T4 implementation and its verification. Acceptance
includes parent review of the resulting ownership and execution paths, not
only the delegate reports or test counts. The T4 implementation has these
ownership boundaries:

- `application/legacy_scene.rs` is the single private, one-way
  occurrence-to-current-renderer adapter allowed by the M1 migration boundary.
  It is not a public View authoring surface. Its deletion gate is T7, after
  the direct Taffy/content route and its receipt/History/input evidence are
  accepted. Archived pixel comparisons are diagnostic only.
- `application/frame.rs` owns the exact presentation products and one native
  receipt per physical submission. `PresentationState` keeps the candidate
  and receipt correlated until completion; metadata-only `NoOutput` products
  advance confirmed frame metadata without claiming a terminal write. The
  close path joins an in-flight receipt and final submission under the
  mutex/predicate contract instead of racing a second close.
- `application/environment.rs`, `application/host.rs`, and
  `application/content.rs` own native scheduling, completion wakeups, typed
  controls, Source/ContentPort projection, and deferred post-lock Source
  wakes. The `UiCommitOutput` return seam carries accepted typed work and
  deferred wakes; its `changes` field remains private. It is the only new
  `iyon_tui::binding` export added for this seam, and the binding check now
  deliberately names it in the exact blessed set (179 exports), without
  widening the binding pattern or the package's public TypeScript surface.
- React editor/control events use a bounded native admission lane and direct
  callbacks. Callback invocation occurs after the acceptance lock is released;
  admission failure is reported rather than silently dropped. Confirmed
  geometry, not desired geometry, is used by focus and visibility barriers.
- History roots and units use typed accepted state. Native History action
  handling is separate from persistent React discard props; live-tail
  restrictions and root ownership are checked during acceptance. History
  logical preparation captures an owned transfer plan under the host lock;
  terminal submission and receipt waits happen outside the UI acceptance lock.
  A failed or lost native receipt preserves the confirmed prefix, marks the
  physical/History synchronization barrier unknown, and conservatively blocks
  suffix replay until an explicit owner-level resynchronization policy exists.
- The exact History physical-export predicate accepts only a complete
  one-content adapter shape and supported transparent padding; nested metric
  dependencies are still traversed independently for layout invalidation. A
  composite or otherwise physically incomplete unit is blocked rather than
  exported as content-only rows.

The final integration checks were run after the small T4 gate corrections (the
History transfer regression now asserts supported root padding around a
transparent one-child shell; `PendingPresentation` boxes its large frame;
the source fixture uses an escaped byte literal so Rust files remain text;
and the binding allowlist names `UiCommitOutput` deliberately):

| Command | Result | Current-source evidence |
|---|---|---|
| `cargo fmt --all -- --check` | passed | Final source is rustfmt-clean. |
| `cargo test --workspace` | passed | 762 core tests, 66 native tests, 5 ABI tests, 19 generator tests, and doctests passed; one existing ignored test remains ignored. |
| `cargo test -p iyon-tui --features native-host` | passed | 761 tests passed, 1 ignored; includes the native-host History/close/signal witnesses. |
| `bun run native:stage` | passed | Canonical darwin-arm64 default N-API addon rebuilt from the final Rust source: SHA-256 `bb685057f27b491fbae933dc17897d32a1b880b2e602e409fcaa4ce1ec645d2e`, 7,606,864 bytes. |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | passed | 188 tests, 3,587 expectations, 0 failures across 38 files against that addon. |
| `bun run typecheck` | passed | TypeScript declarations type-check. |
| `bun run lint:ts` | passed | Biome exit 0; 382 warning diagnostics and 22 informational suggestions remain visible. |
| `bun run lint:ts:complexity` | passed | Biome exit 0; 48 warning diagnostics remain visible under the configured warn-mode complexity check. |
| `bun run check:tui-abi` | passed | Generated ABI output matches. |
| `bun run check:tui-declarations` | passed | 37 reachable public declaration files remain closed over supported paths. |
| `bun run check:tui-binding` | passed | Native core imports remain on the seam; 179 exports match the exact blessed list and no authoring names leak. |
| `bun run check:ownership` | passed | Rust/TypeScript ownership and surface snapshots pass. |
| `bun run rust:clippy` | passed | The strict project gate completed with exit 0; warnings remain and are not a warning-free claim. |
| `bun run native:smoke` | passed | Packaged native content route rendered the smoke frame. |

The first broad pass exposed one stale physical-shape assertion and three
strict Clippy errors in T4 additions; those were corrected and the affected
Rust tests, format check, strict gate, rebuilt addon, full workspace Rust/Bun
tests, and smoke route were rerun. The initial broad pass's other successful
checks (TypeScript, Biome, generated ABI/declarations, binding, ownership) are
unchanged by those Rust-only corrections and are listed above with their
original logs. Durable command logs are in `/tmp/iyon-t4-broad-*.log`,
`/tmp/iyon-t4-final-*.log`, and `/tmp/iyon-t4-native-stage-final2.log`.

The T5/M1 deletion slice removes the native old View ABI/generated output,
ordinary Rust/native ViewState plane, old N-API ViewRef classes and structural
binding residue. The remaining migration gates are direct Taffy layout,
semantic content lowering and the separately scoped Surface/physical-export/
GPUI work. Uncertain physical History suffixes remain conservatively blocked
pending an explicit resynchronization owner.

### T4 parent-review correction: zero-progress History close

The current source includes a narrow correction for the final-exit History
settlement loop. `settle_history_plan_with_backend` now returns the typed
`NativeTransferOutcome` from the exact captured acknowledgement instead of
flattening it to `()`. A successful nonempty transfer with
`inserted == 0` and `NativeTransferStatus::SinkBlocked` marks the existing
`history_sink_blocked` path and prepares one front-pinned final candidate;
close does not submit the unchanged captured prefix again. The same outcome
classification is applied when close joins an already in-flight History
receipt. Zero-row semantic retirement remains `Progress`, so it is not
classified as a sink block. No synchronization-unknown marker or requested
History rows are fabricated or discarded by this correction. The existing
final-frame positioning and restoration/`CloseOperation` ownership remain
unchanged.

The production close seam is covered by
`application::host::tests::inline_exit_zero_progress_history_receipt_does_not_resubmit`:
a nonempty captured plan receives `Ok(0)` through the existing receipt hook,
exit completes after the receipt, the retained semantic prefix remains
the only prefix visible before the final frame, and the next History row is
not submitted a second time. Existing final History, in-flight receipt,
failed-receipt, and captured-row tests remain green. Parent review accepted
this correction as part of T4. The full Rust workspace evidence predates
this narrow correction; the focused native checks, strict Clippy gate,
rebuilt-addon Bun suite, and smoke checks below cover the final source.

Final correction-gate provenance after rebuilding the canonical default
N-API addon from the stabilized Rust source:

    artifact=packages/iyon-tui/native/iyon-tui-native.node
    sha256=bb685057f27b491fbae933dc17897d32a1b880b2e602e409fcaa4ce1ec645d2e
    bytes=7606864
    target=aarch64-apple-darwin
    features=default N-API (napi8 type-tag qualification enabled)

Current correction checks: `cargo fmt --all -- --check`, `cargo check -p
iyon-tui --features native-host`, the strict project Clippy gate, and the five
focused native-host History/close tests passed. The affected staged-native
React History/renderer, History-prefix, native harness, and wake-broker tests
passed (66 tests), the full Bun workspace/consumer suite passed (188 tests,
3,587 expectations), and `bun run native:smoke` passed. The prior broad
workspace Rust/ABI/declaration/binding/ownership evidence remains applicable
to unchanged surfaces; T4 parent source/design review remains required.

## Post-T2 permanent-code quality gate

After T2 is complete, review and simplify the permanent T1 occurrence, schema,
and generator code before starting T3. Prior T1 acceptance is not an
exemption: preserve behavior and contracts, and commit accepted cleanup as a
separate change. T2's qualified N-API adapter and any other temporary migration
paths retain their specific deletion gates; new long-lived ownership and
control code must remain structured at acceptance.

### T1 permanent quality cleanup status

**Accepted after parent review; committed separately from T2.** This cleanup
addresses the accepted T1 occurrence/schema/generator maintenance findings:
co-located typed creation configuration lookup, one layered style-state
operation representation, occurrence module visibility/re-export ownership,
production helpers moved before the occurrence test module, and coherent
generated-schema renderer sections. `CommitDraft` now owns interpretation and
initial snapshots, operation-family methods own their mutations, and draft
finalization and reserved installation have explicit boundaries. The parent
kept property/style/interaction finalization inside the draft that owns those
snapshots and removed the interpreter's unused record-index argument.

It preserves the accepted T1 transaction and generated ABI contracts. The
generated occurrence schema changed only its generator fingerprint; semantic
schema content did not change. No T3 work was included in this cleanup.

Verification: final delegate workspace tests and Clippy gate passed, along
with generator, binding, ownership, TypeScript and formatting checks. The
fresh delegate addon (`85b667742a454489fcace7065ad2215b16baf8170cd0074e35741ba50a6a4e3b`,
6,964,464 bytes) passed all 10 Bun ingress tests and 34 expectations. After the
parent's final ownership-only adjustment, all 30 occurrence/control tests,
native type-checking, formatting and the Clippy gate passed again. Earlier
workspace and boundary evidence is reused for unchanged behavior. Clippy now
has warnings only; the T1 hard failures recorded at T2 acceptance are resolved.

## T0 baseline provenance

The baseline addon was staged through the repository's normal script before
implementation work:

    git_sha=74ae1b967042e5cc6fa99a4faf92b25da5befc15
    artifact=packages/iyon-tui/native/iyon-tui-native.node
    artifact_sha256=b1d8700355dcd469877e5ab86fa01c017f216bb2c5d256df79a30eeb3a6c435e
    artifact_bytes=6488624
    target=aarch64-apple-darwin
    bun=1.4.0
    rustc=1.97.1 (8bab26f4f 2026-07-14)
    features=default N-API

native:stage passed and the staged path was the artifact consumed by the Bun
checks. The addon does not embed a source SHA; the hash is therefore recorded
for reproducibility, but it is not proof that a previously cached native binary
was compiled from this exact source tree. A freshly staged artifact is required
again at the T2 native-boundary gate.

### Baseline checks

All of these were run against the pre-T1 implementation route:

| Command | Result | Output summary |
|---|---|---|
| bun run native:stage | passed | Staged the default N-API addon for darwin-arm64. |
| bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests | passed | 123 passed, 0 failed, 3,332 expectations, 35 files. |
| bun run check:tui-abi | passed | Existing generated View/state outputs matched. |
| bun run typecheck | passed | TypeScript declarations checked with Bun 1.4.0. |
| bun run check:tui-declarations | passed | 37 reachable public declarations; no transport-path imports. |
| bun run check:tui-binding | passed | 124 blessed native binding exports; no authoring names. |
| bun run check:ownership | passed | Existing framework ownership, route and banned-name checks passed. |
| bun run native:smoke | passed | Packaged content route rendered packaged TUI smoke. |
| bun packages/iyon-tui/bench/perf13_h_content.ts | passed | 1,000 appends; source revision 1,000; 2 native drains/frames. |
| bun packages/iyon-tui/bench/pre-v5-l1-trace.ts | passed | Existing route trace: one structural host mutation, 50 content revisions, 2 content drains/frames. |

The existing test suite is the route witness for initial mount, local
presentation/state changes, keyed reorder, native editor/input, animation
deadlines, delayed receipts and close/failure behavior. Relevant named tests
include tui_perf13_a, tui_perf13_b, tui_perf13_d, tui_perf13_h,
tui_retained_scene_regressions, tui_runtime, tui_realtime, and the consumer
fixture tests. Their baseline results are included in the 123-test run above;
the historical trace's git_sha, Rust and artifact fields reported unknown, so
it is route evidence rather than source-qualified performance evidence.

### Baseline publication-owner inventory

The following line counts were collected before T1 edits with find, xargs and
wc -l; they are maintenance-burden measurements, not a deletion target:

| Existing owner | Rust/TypeScript lines |
|---|---:|
| packages/iyon-tui/src/composition | 2,904 |
| packages/iyon-tui/src/transport/structural | 3,831 |
| packages/iyon-tui/src/transport/state | 735 |
| crates/iyon-tui-native/src/tui | 7,858 |
| crates/iyon-tui/src/retained_state | deleted at M1 |
| crates/iyon-tui/src/scene | 6,824 |
| packages/iyon-tui/tests | 4,297 |

The current production route is:

    React HostConfig / CommitCoordinator
      -> commitUiV1 (qualified direct-occurrence batch)
      -> UiResourceOwner / OccurrenceDocument
      -> private LegacySceneAdapter
      -> SceneHost layout/paint
      -> environment pending queue / exact physical receipt

No immutable View-to-occurrence compatibility reconciler exists. The private
legacy adapter is one-way and scheduled for deletion at T7/M2.

## T1 schema and core

### Generated contract

tools/tui-abi/ui_abi.toml is the finite source of truth for:

- version-one header constants (0x49595549, version 1, 16-word header);
- host behavior kinds Box, ContentHost, Editor, Scroll, Animation;
- control/root/handle/ownership codes;
- all 28 operation codes and their section/operand classification;
- the 17 existing geometry/presentation properties, explicit global IDs,
  legal host kinds, normalizer/default/reset/override/inheritance metadata and
  semantic effect categories.

The existing tools/tui-abi-gen now loads only ui_abi.toml. It remains one
generator and one generated-output check; no parallel schema tool was
introduced. Current outputs are:

- crates/iyon-tui/src/occurrence/generated.rs;
- packages/iyon-tui/src/transport/ui/generated/ui_schema.ts;
- packages/iyon-tui/src/transport/ui/generated/ui_abi_manifest.json;
- docs/architecture/generated/UI-ABI-REFERENCE.md.

The old View ABI schema and every output derived from it were deleted at M1.
The UI manifest's generated-output list is the complete current set; it no
longer names native View, C header, state-envelope, structural TS, snapshot or
benchmark outputs.

After generation changes, the native addon was rebuilt and staged again for
the current route checks:

    artifact_sha256=c4ef9839ca73d9174bb2afc8d3d48011c0acab8bb2eed94fa733c768ab5203e2
    artifact_bytes=6488816
    addon_generator_blake3=4b7d6e118803b6636f8feab2f4335b793df70df560bb05062c99e020c4ed7fd6
    ui_schema_blake3=8ff1e9e2b87f0439ccdbd9f487068d190302196bd8545dd90d1cd4dc2f9daee0
    target=aarch64-apple-darwin
    features=default N-API

The staged addon passed the 38 focused route tests, the full 123-test Bun
suite, native smoke and the content/route traces. This is fresh route evidence
for the current generated View ABI; it does not claim that T1 has entered the
new UI batch through N-API.

T1 checks also passed: cargo fmt --all -- --check, cargo check --workspace
--all-features, cargo test -p iyon-tui --lib (592 passed, 1 ignored), cargo
test -p tui-abi-gen (16 passed), bun run check:tui-abi, bun run typecheck,
bun run check:ownership, and the focused occurrence tests (25 passed). The
repository's existing warning/clippy backlog remains outside this slice.

### Parent-finding correction tranche (accepted)

The follow-up review identified correctness and sparsity gaps in the first T1
slice. The parent inspected the corrected arena, topology, transaction/index
paths and generator changes and accepted this typed-core tranche. Parent reruns
passed all 25 occurrence tests and the ownership checks. The containing commit
records T0/T1; T2 and the full M1/M2 migration remain outstanding:

- prepared commits reserve each arena free-list for the exact planned recycled
  live keys (excluding generation-exhausted slots), including deep subtree and
  resource cleanup; arena reserved insertion consumes a slot through its
  per-slot free index rather than a linear free-vector search;
- tree link validation checks the touched link frontier and boundary records
  without rescanning every child of each edited parent;
- Port/Connector reverse indexes make connector dependency checks sparse, and
  resource validation checks edited resources plus the affected retired-Port
  dependencies rather than every live Port/Connector;
- portal owner edges participate in cycle walks and owner-dependent retirement;
  a sparse portal-owner reverse index validates surviving portals when an owner
  or its parentage changes and retires portal descendants with their owner;
- prepared reverse-index buckets stay in the prepared plan until apply, so a
  dropped/rejected prepare cannot publish empty authoritative buckets; touched
  buckets are cleaned after apply without a whole-index scan;
- draft portal and Connector reverse relations avoid scanning all newly created
  occurrences/resources for every owner/Port query, and generated effect-mask
  names fail explicitly if validation and rendering disagree;
- bounded regressions cover exact deep-subtree free-list preparation, dropped
  prepared-index state, sparse-created portal retirement, and portal owner
  lifetime/cycle failures. The focused occurrence suite is now 25 passed, 0
  failed.

## T2 qualified native ingress and resource preparation (accepted)

The working tree now contains the first actual native desired-state route:

- `crates/iyon-tui-native/src/tui/ui_commit.rs` decodes the generated v1
  header/sections explicitly, validates exact bounds/opcode sections/handles,
  qualifies non-shared ArrayBuffer backing and typed-array kind with napi8
  native type tags, preserves
  valid nonzero byte offsets, validates Source object class/liveness and
  environment identity, and rejects malformed/shared/detached-invalid input
  before the host core boundary;
- typed-array qualification checks the one queried typed-array span against its
  backing ArrayBuffer length, and header admission rejects local creation
  counts above 1,048,576 or counts that do not match decoded creation records
  before acknowledgement allocation;
- property decoding preserves the finite Insets, boolean Edges, ANSI/RGB
  Color, six-bit TextAttributes, nine-word Style, and sixteen-lane metadata
  BorderGlyphs representations; invalid widths, ranges, flags, and glyph
  values reject instead of silently falling back to empty values;
- acknowledgement storage is allocated as a native `Uint32Array` before
  `prepare_ui_commit`/apply; rejection batches fill the preallocated rejected
  acknowledgement without mutating accepted UI state;
- `TuiHost` owns the generic occurrence document and the new path does not call
  `setDesiredViewRef`, immutable View materialization, layout, paint, terminal
  output, or the old View ABI;
- occurrence-owned Ports/Connectors are installed through the Send-safe native
  UI resource owner while reusing existing qualified Source storage/membership
  semantics. Private literal replacement creates/reuses a private Source and
  binding identity, while Funnel changes replace that private binding;
- `crates/iyon-tui/src/occurrence/control.rs` owns closed typed Editor, Scroll,
  and Animation control state. Commands are applied sequentially to an owned
  prepared state; raw command IDs/operand vectors are not retained as a last
  command cache, and create-plus-command/replacement batches are supported.
- The existing ABI generator now emits finite control-command/config
  descriptors. `CREATE_CONTROL` and `CREATE_ROOT` decode typed metadata into
  those contracts; invalid kind/value/length combinations reject before
  acceptance instead of silently discarding configuration.
- the native UI owner keeps only qualified Source identities, explicitly
  disposes unaccepted private candidates before Source installation, and
  releases Source memberships/private Sources on accepted retirement and host
  close/drop. Cleanup errors remain visible instead of being converted into a
  successful close.
- Source replacement uses an owned prepared `StoredSource` value with checked
  UTF-8/annotation/retention/revision bounds; apply performs the guarded source
  swap rather than invoking fallible clear/append mutators.
- the new Rust core/document, native UI resource owner, and operation/result
  envelopes have compiler-checked `Send + Sync` assertions. Existing legacy
  host unsafe boundary assertions remain outside this new route and are not
  copied into occurrence/resource state or frame data;
- focused Bun boundary tests cover nonzero offsets, malformed/shared backing,
  rejected cycles, Source membership, explicit Connector/Port disposal, and
  the actual native acknowledgement. They also cover native-host close
  membership cleanup, same-batch typed control command/editor replacement,
  wrong-class/prototype-spoofed Source rejection, detached backing rejection,
  and bounded/mismatched local-creation counts.
- focused Rust tests cover repeated initial literal replacement and final
  Funnel preservation, explicit Connector/Port release, rejection after a
  changed last Source guard with the first Source untouched, and subscriber
  wake after an accepted prepared replacement.
- the decoder consumes generated opcode/header/property constants, and closed
  HistoryAction, ControlCommand, style-state layers, editor replacement, and
  annotation sidecars are typed rather than silently skipped;

The generated `PropertyId`, `ValueKind`, property descriptors, and
`ValueEncodingDescriptor` rows select the finite decoder branches and record
their bounded word/metadata layouts. The decoder keeps handwritten algorithms
for the finite semantic forms, including the bounded direct/themed Style
encoding; it does not expose a complete CSS or generic Style wire format. The
schema hash fingerprints these layout rows, and the generated Rust and
TypeScript descriptions are checked together.

Final parent-checked provenance for the freshly staged default N-API addon:

    artifact=packages/iyon-tui/native/iyon-tui-native.node
    sha256=847162f89a389b020502d4240d2e891a0a81d85d308c65f560f3a838faaefc94
    bytes=7009200
    target=aarch64-apple-darwin
    features=default N-API (napi8 type-tag qualification enabled)

This accepts only the T2 ingress/resource-preparation boundary. Editor
replacement and closed ControlCommand now enter the typed Send-safe UI state;
parser/layout/output and renderer/frame realization remain explicit later
seams. Typed editor/scroll/animation control identities and command/editor
state are prepared without entering the renderer; full native control
input/tick lifecycle remains a later controls tranche. The T3 renderer below
consumes the accepted UI seam without claiming frame realization.
Source replacement and membership now use one sparse guard-scoped multi-Source
transaction: the final binding map computes each membership delta once, all
ordered Source guards, revision/liveness/count checks, and prepared storage
allocation complete before any Source write, and accepted storage replacement
captures subscriber wakes before writes and schedules them after UI acceptance
without invoking parsing, layout, or output. Concrete Editor/Scroll/Animation
controller state is prepared in the core occurrence module, while Source and
private-resource cleanup remains explicit on retirement and host close/drop.

### T2 acceptance verification

- `cargo test --workspace --all-features` passed on the final delegate source,
  including 19 generator tests.
- TypeScript checking, the focused Biome lint command, generated-output checks,
  the 177-export binding check, and ownership checks passed. Biome emitted
  informational suggestions, not errors.
- The freshly staged delegate addon passed the full Bun suite: 133 tests,
  3,366 expectations, no failures.
- Parent removed a T2-introduced unused `SmoothConfig` import and repeated
  editor operand checks already covered by the generated command descriptor.
  Formatting, native type-checking and the three focused control tests passed
  afterward. The parent then rebuilt/staged the addon recorded above and reran
  the actual Bun ingress suite: 10 tests, 34 expectations, no failures.
- The Clippy gate was run and failed. Contrary to the initial delegate
  attribution, the unused host import was introduced in T2 and is now removed.
  The other hard failures—unused occurrence re-exports and production items
  after the occurrence commit test module—were verified in accepted T1 source.
  They are assigned to the immediately following T1 quality cleanup, not
  reported as a passing gate. Existing non-fatal warning debt remains visible.

Full-suite evidence remains applicable to the parent changes above; focused
checks cover their affected behavior. No full migration, native frame-driver,
React, renderer or Taffy acceptance is implied.

## T3 minimal React renderer (accepted)

The tranche based on `17f116c` contains the first React mutation route plus
the ownership/lifecycle corrections described below. Parent source review and
current-source verification accept this bounded T3 implementation, not an
M1/M2 cutover:

- `packages/iyon-tui/src/react/host-config.ts` is the isolated React
  19.2.8/reconciler 0.33.0 HostConfig shim. It declares mutation mode,
  disables persistence/hydration, uses non-null host context, installs the
  current update-priority/scheduling hooks, and keeps React's internal Fiber
  handle opaque;
- `packages/iyon-tui/src/react/instance.ts` creates pure JS HostInstance and
  HostTextInstance candidates. It normalizes the finite generated-schema
  property inventory (including colors, Insets, edges, attributes and direct
  or themed styles), callback presence, finite Box/Row/Column/Grid layout
  kind, literal text, Source/Funnel and editor/control declarations before
  native mutation. The accepted React Port surface is the qualified lazy
  token, not the old ContentPort attachment identity;
- `tools/tui-abi-gen/src/render_ui.rs` emits generated property descriptor
  lookup, finite value-key/equality helpers, value-encoding lookup, and
  `uiEncodePropertyValue` packing used by the React path. Regenerated outputs
  retain all pre-existing T2 wire IDs and value forms. T3 adds the finite
  LayoutMode/layout property required for distinct Row/Column/Grid desired
  state, and refreshes the generator fingerprint and corresponding snapshot.
  The generated Style packer has one owned color implementation, explicit
  attribute-mask contracts, and no speculative zero fallbacks;
- `packages/iyon-tui/src/react/commit.ts` owns one per-commit journal. It
  traverses each newly materialized subtree once, collapses overlapping
  candidate roots, assigns local creation ordinals, and encodes generated
  schema records/forms and sidecars. It calls `commitUiV1` once for nonempty
  native work, assigns acknowledgement handles, then promotes accepted
  snapshots and ownership links. Candidate creation, abandoned render and
  callback identity-only updates do not allocate native resources or send
  semantic UI work. Anchored insertion uses the anchor's actual previous
  sibling, including prepend/first-node cases; keyed placement uses React's
  ordered mutation callbacks with no second list reconciler or immutable View
  translation. Initial anchor lookup memoizes skipped candidate/portal runs
  and stops at the next accepted ordinary sibling; it does not scan a parent's
  clean child list. Resource encoding receives its already-assigned node ordinal
  directly. Sidecar copying does not spread payload bytes into function arguments,
  with a one-million-character literal regression. Accepted token maps are
  changed only after acknowledgement;
  Port and Connector hook owners are independent, explicit resources retain one
  qualified handle across consumer gaps/transfers, and dependency-aware cleanup
  disposes Connector before Port through this same coordinator. Deleted-instance
  cleanup only detaches the consumer. A committed connector-hook dependency
  replacement releases the old token after acknowledgement, even when its
  consumer is absent; an inactive live Connector is not disposed by another
  consumer's selection. Blocked Port cleanup waits for the dependent Connector
  notification instead of re-scanning every render. Accepted selection
  bookkeeping follows acknowledged initial handles, Port replacement, subtree
  detachment, hook disposal and close. Occurrence-owned detached resources
  lose their selection with retirement; caller-owned explicit Ports retain
  their selected Connector under the accepted T2 contract until React
  explicitly deselects or disposes them. A rejected commit leaves accepted
  token identity and native membership untouched;
- `packages/iyon-tui/src/react/root.ts` exposes `createReactRoot` for a
  public `TuiRuntime`/`AppHarness`, a Promise that resolves on accepted UI
  desired state, one root authority per host, same-host portals, explicit
  root cleanup, unmount/close, and separate
  `whenVisible`/`whenContentVisible` barriers. Native environment scheduling,
  receipt notification and exact confirmed revisions settle those barriers
  without a JavaScript frame pump; they do not pretend acceptance is terminal
  output;
- `packages/iyon-tui/src/react/components.ts` adds Box/Row/Column/Grid,
  Content/Text, Editor/Scroll/Animation conveniences. Text and raw JSX text
  lower to ordinary childless ContentHost occurrences; no structural Text or
  Hanging kind was introduced. Controls create only the existing typed
  Editor/Scroll/Animation state. `hooks.ts` adds immutable JS-only lazy
  Port/Connector binding tokens, which materialize only when a committed
  Content occurrence uses the binding. Port and Connector hooks have separate
  lifecycle owners bound to the coordinator only after acknowledgement;
  cleanup is microtask-coalesced for StrictMode replay, stale superseded
  tokens are revisited after their memoized consumer detaches, and owner
  release failures fault the root rather than escaping a microtask. Component
  conveniences normalize finite props during render as well as at the
  HostConfig/native boundary. Token owner/coordinator state remains internal,
  presentation values are copied through the semantic finite validators, and
  React literals are the plain string/number/bigint path only;
- generated finite equality keys use length-delimited canonical fields rather
  than separator-joined glyph/theme strings. Public occurrence refs expose the
  supported override/clear publication commands through the coordinator. The
  React host shim uses the installed reconciler runtime priority constants and
  defaults NoEventPriority to DefaultEventPriority for ordinary state updates;
- the private native host seam now provides a qualified body occurrence
  handle without exposing it through the package root. `Tui` and `AppHarness`
  carry the association privately, so the consumer fixture uses only
  `@iyon/tui`, `@iyon/tui/testing` and `@iyon/tui/react` imports;
- `packages/tui-consumer-fixture/src/react-consumer.ts` and its acceptance
  witness exercise the public consumer path while all existing old production
  routes remain in place for T5 disposition.

The published artifact provenance was checked before this renderer run:

    react=19.2.8 (registry dist integrity sha512-PWaYA1L/q9u2u7xYQi+Y3L3Yfnie7XyLeaJICV1MGD6LprsBxcAqGjYyr0eY3p+QdsA+x/Irkt4Qif8D63+Sbw==)
    react-reconciler=0.33.0 (registry dist integrity sha512-KetWRytFv1epdpJc3J4G75I4WrplZE5jOL7Yq0p34+OVOKF4Se7WrdIdVC45XsSSmUTlht2FM/fM1FZb1mfQeA==)
    @types/react=19.2.8
    @types/react-reconciler=0.33.0
    installed manifests: packages/iyon-tui/node_modules/{react,react-reconciler}/package.json

The installed `@types/react-reconciler` HostConfig declaration was read rather
than copied from an older renderer tutorial; the runtime factory was also
inspected because its 0.33.0 object does not expose the stale declaration's
`flushSync` helper. T3 uses the installed `updateContainerSync` and
`flushSyncFromReconciler` methods. The resolved React, reconciler, scheduler,
React type and `csstype` entries are locked in `bun.lock`.

The native boundary was rebuilt and staged after the final generated outputs,
private body-handle method and closed UI-state lifecycle used by the renderer:

    artifact=packages/iyon-tui/native/iyon-tui-native.node
    sha256=ed29f621ef2df91174fac1d466d5c66756341777279f773561a08871f9b49f9d
    bytes=6980224
    target=aarch64-apple-darwin
    features=default N-API (napi8 type-tag qualification enabled)

The final lifecycle-correction checks and exact current counts are recorded in
the durable handoff artifact. They include staged-native React/consumer tests
covering committed dependency replacement with an absent consumer, stale
memoized-consumer detachment, ordinary useState plus layout/passive effect
scheduling, same-instance and independent A/B/A selection, inactive-owner
cleanup, blocked Port dependency release, implicit source/literal/Port
transitions, typed portal-root reorder/retirement/owner transfer,
hook-owner detach/restore/cleanup under StrictMode, cross-root authority
rejection, token disposal/transfer, ref publication, fault cleanup retry and
the native UI commit lifecycle tests. `cargo fmt`, `cargo check`, TypeScript,
Biome and the generator check pass on the current source. After the final parent
anchor correction, the full Bun workspace pass reports 164 tests, 0 failures
and 3502 expectations across 38 files. Parent reran the native UI commit tests
(12 passed), formatting, generator, binding and ownership checks. The unchanged
Rust source also retains the delegate's occurrence (26 passed), generator
(19 passed) and workspace/all-features check evidence. Parent Clippy with
`-W clippy::cognitive_complexity` completed without errors; warnings remain,
including documentation/style warnings in touched Rust files. This is not a
warning-free lint claim. The staged addon above is unchanged by the final
TypeScript-only corrections.

T3 and T4 are accepted. T4 owns terminal frame realization, native scheduling,
content/control integration and exact presentation barriers; T5 owns canonical
production cutover and deletion of old composition and View publication.

### Occurrence ownership and transaction contract

crates/iyon-tui/src/occurrence/ contains:

- arena.rs: host-qualified handles, one-based slot indexes, generation
  validation, nonrecycling host namespace allocation, free-slot reuse and
  generation exhaustion slot burning;
- tree.rs: intrusive parent/first/last/previous/next/child-count links,
  sequential insert_before, exact detach, subtree retirement, cycle, anchor,
  root and final-orphan validation;
- properties.rs: finite typed property values, declared/override layers,
  explicit Unset versus Null, semantic value-kind validation and
  effective-value comparison;
- control.rs: closed typed Editor/Scroll/Animation controller state and
  sequential command/replacement transitions used by the native UI owner;
- commit.rs: typed UI operations, local creation ordinals, sparse tree and
  resource overlays, final Port/Control owner indexes, whole-batch rejection,
  reserved apply, and the exact eight-word acknowledgement header followed
  by four words per created handle.

OccurrenceDocument::prepare_ui_commit performs decoding-equivalent typed
validation, sparse overlay interpretation and reservation without publishing
logical records. apply_prepared_ui_commit installs the prepared records and
returns the acknowledgement allocated by preflight. It does not render, parse,
write to a terminal, invoke callbacks or use N-API. This remains the T1 owning
boundary beneath the qualified T2 native decoder.

The core does not import React, N-API, Termwiz, GPUI, Taffy, application policy
or the existing immutable View transport. Existing Source payload mutation and
the old renderer are untouched.

### Focused T1 witnesses

The core tests cover:

- initial local creation/ordered sibling links and acknowledgement layout;
- self-anchor no-op and wrong-parent anchor rejection;
- dense local creation ordinals and old/new parent frontier invalidation;
- sequential cycle rejection after a valid earlier property edit;
- descendant detachment/rescue before parent subtree retirement;
- generation increment, stale-handle rejection and generation-exhaustion
  burning;
- wrong-host and wrong-kind handles;
- declared/override masking where clearing an override reveals the newest base;
- occurrence-owned resource owner-index atomicity and retirement cleanup;
- occurrence-owned resource owner requirements, Connector disposal ordering,
  deferred explicit-Connector disposal, and all-Connector cleanup when a Port
  detaches;
- empty style-state value rejection without partial apply;
- true no-op property commits not advancing the accepted UI revision.

The generator tests cover output-path uniqueness, complete UI inventory and
rejection of duplicate property IDs or unknown effects. Decoder malformed-byte
fixtures, Bun ArrayBuffer qualification and native resource preparation are
deliberately T2 tests.

## Consumer and migration checklist

This is the port/deletion ledger required by the handoff. No consumer was
silently kept on a test-only compatibility runtime.

| Consumer/route | Planned tranche | Status |
|---|---|---|
| packages/tui-consumer-fixture/src/react-consumer.ts | T5 | sole public consumer route; exercises editor callbacks/focus, keyed Scroll, Animation, Source/Port/Connector and typed HistoryUnit refs |
| packages/tui-consumer-fixture/tests/react-consumer.test.ts | T5 | public React acceptance plus editor, keyed list, control, content lifecycle, and History identity witnesses |
| packages/iyon-tui/scripts/smoke-native.ts | T5 | packaged React root/content smoke route |
| controls (Editor, Scroll, Animation) | T5 | finite React occurrence controls; old TextInput/ScrollPane/ViewSlot authoring facades deleted |
| content Source/Funnel/Port/Connector | T2/T4 | existing direct Source data lane retained; no payload fallback added |
| History/native scrollback | T4 / Surface gate | current physical behavior retained until the component-only Surface migration and explicit physical-export policy |
| old composition/structural/state tests and benchmarks | T5 | deleted as superseded View/publication-only suites; current occurrence, content, control, History, receipt and failure witnesses remain |

### T5 prerequisite slice (not M1 acceptance)

> Historical note: this subsection records the prerequisite review evidence
> collected before the T5 cutover. The current T5 row and consumer ledger
> above supersede its old-route retention statements.

Parent acceptance covers the following prerequisite contracts and evidence:

- `createReactRoot` passes the pinned reconciler's named `ConcurrentRoot`
  constant. In the installed `react-reconciler@0.33.0` development runtime,
  `FiberRootNode` unconditionally assigns `this.tag = 1` (line 16774) and
  `createFiberRoot` sets the root fiber's concurrent mode through its reused
  local variable `tag = 1` (line 16836),
  regardless of the argument. The argument therefore records pinned API intent;
  it is not evidence that this change enables concurrency. The public `render`
  method continues to use `updateContainerSync` and retains its accepted-commit
  Promise contract; no root mode selector or second render API was added.
- The React renderer test starts a real `startTransition` that builds 2,048
  keyed Content candidates with lazy Port/Connector tokens. It observes HostConfig
  candidate creation while the native commit count remains unchanged, then
  replaces the work through ordinary `root.render`. The replacement is
  accepted, the abandoned candidate set produces no native commit/resource
  records, no committed batch contains `createConnector`, and candidate hook
  effect/ref counters remain zero. A second transition is superseded by public
  `unmount`, and a third yielded transition is superseded directly by
  `root.close`, followed by a scheduler turn checking for late effects or native
  calls. A healthy render between unmount and close proves the unmount path
  leaves the root usable. The witness is based on candidate-before-native-commit observation,
  not a causal claim about the root tag.
- The negative control is a controlled temporary experiment, not a permanent
  test: on the same interruption fixture and 2,048-candidate workload,
  `startTransition(() => setInterrupted(true))` was temporarily replaced with
  `withNativeEventPriority("discrete", () => setInterrupted(true))`. The
  unchanged `waitForCandidateYield` then failed promptly with `native commit
  overtook candidate-yield observation` (exit 1), the source was restored byte
  for byte, and the positive witness passed again. The exact modification,
  failure output, restoration hash and comparison are recorded in
  `/tmp/iyon-t5-react-interruption-negative-control.log`.
- The external-shaped React fixture and `tui_demo.ts` use only documented
  package entrypoints for React components, refs, Source/content hooks and
  History. The consumer tests observe editor input/submit callbacks after
  public ref focus, keyed list replacement, native Scroll/Animation output,
  Source revision/content output, Connector-backed Content, and stable typed
  HistoryUnit identity. The smaller demo observes composer screen output,
  direct Source-backed Content, editor callbacks after ref focus, and a typed
  HistoryUnit identity. `ConsumerState` is now a passive type module that does
  not import the legacy View route.

This subsection is retained as historical prerequisite evidence. Its old-route
statements do not describe the current source: the legacy `Tui.render` route,
public View/composition exports, native View publication tables, root/lease
machinery and ordinary ViewState have now been removed. The current addon must
be staged from the current source before final package evidence.

Slice evidence: the focused React/consumer/demo command passed 56 tests and
256 expectations, and the corrected interruption witness passed five
consecutive isolated runs. The controlled discrete-priority negative run
failed as expected (exit 1) and is logged outside the repository. The permanent
negative-control test was removed rather than retained as workload/priority
confounding evidence. The previous full Bun evidence (192 tests and 3,619
expectations) remains reusable for unaffected paths; this correction changed
the Rust adapter and native artifact, so the current focused
React/consumer/demo, typecheck, declarations, Biome, ownership, and native
alignment checks are the applicable evidence. The staged T4 addon above is
historical prerequisite provenance, not the artifact used by those alignment
checks.
Parent's final source check reran the 56 focused tests, TypeScript, and Biome
after removing duplicate demo submit bookkeeping and adding a post-close
scheduler turn. Biome's two import-order findings were fixed; the interruption
test then passed again. Logs are `/tmp/iyon-t5-parent-*.log`.

The bounded T5 presentation/state parity correction is accepted after parent
source/design review of the occurrence owner, adapter, and public boundary.
Border fields are composed into one
effective specification, direct foreground/text attributes overlay a named
style base, effective declared/override style states are snapshotted and
lowered into the existing theme selector, and Box-row vertical alignment uses
the existing `row_specs` path. Neutral/default axes are accepted as identity
layout, while only non-neutral Box-row vertical axes are mapped; unsupported
non-neutral axes are reported at the frame
barrier rather than ignored. The correction is now applied on the direct
occurrence route. The private adapter remains only as M2 renderer residue; the
ordinary ViewState plane and old publication route are deleted. Physical
History export still requires a public React consumer witness before the
separate Surface gate.
T4's accepted source is `ca1216335d57569a4171d10b86bcf3aad0872671`; this
historical alignment record does not impose an M1 pixel-parity requirement on
the replacement React/Taffy route.
The earlier React prerequisite is committed at
`a6d70b22373eefb11e28670eff8e4576008a774c`. This alignment scope does not claim
horizontal text alignment or general Flex/Grid parity; their content/layout
ownership remains required work in the T6/T7 migration.

## Remaining proof and risks

- The occurrence document is connected to the React mutation renderer and the
  existing terminal renderer through one private one-way adapter. The adapter,
  current View IR and general Scene/layout helpers remain explicit M2 residue;
  T7 deletes them after direct Taffy/content realization and
  receipt/History/input evidence. Archived pixel comparisons are diagnostic
  only and do not authorize retaining the adapter.
- The old native View ABI, generated C/Rust/TypeScript outputs, old N-API
  ViewRef classes, state envelope and ordinary Rust/native ViewState registry
  are absent from the current source. The current generator emits only the
  direct UI schema and four listed outputs. Generator, binding, ownership and
  declaration checks assert this absence rather than accepting an old/new
  selector.
- The current source passed the broad Rust, generator, TypeScript, package,
  smoke, Source content-FFI, and benchmark checks listed above. Source FFI is
  part of the canonical addon and is qualified by unconditional symbol checks
  during staging plus `content_ffi::tests`; there is no second direct-FFI UI
  route or feature artifact. The canonical artifact is the default N-API build
  above.
- Only the macOS arm64 toolchain/target is installed locally. The CI matrix
  includes Linux x64 and macOS arm64; Linux x64 remains a CI gate and is not
  claimed from this macOS run. Direct Taffy/content latency isolation is now
  implemented, while the T7 semantic-content lowering, adapter deletion,
  receipt/close/fairness evidence, Surface and GPUI gates remain deferred.
  Parent source/design acceptance covers M1, not those later migration
  boundaries or unexecuted Linux validation.

### T6 foundation checkpoint — parent accepted

The finite schema and derived layout adapter are accepted as a T6 foundation
checkpoint after parent source/design review and local integration checks.
**T6 itself is not accepted:** ordinary Box/control/History frames still use
the M1 route. The next T6 slice must wire the direct occurrence renderer,
preserve exact frame/content ownership, compare baseline output, and remove
the temporary unsupported-geometry guard. T7 then replaces content View
lowering and deletes the remaining old general rendering machinery.

The accepted foundation contains:

- Taffy `=0.12.2`, with default features disabled and only `std`,
  `taffy_tree`, `flexbox`, `grid`, and `content_size`. Its high-level tree
  remains private to `presentation/taffy.rs`.
- Manifest-owned finite geometry descriptors, enum names/tags, property
  domains and generated codecs/reference outputs. Existing property IDs and
  fit/fill conveniences remain stable. Public normalization owns structured
  inputs, canonicalizes f32 values/signed zero, and compares geometry fields
  semantically. Native ingress independently validates wire data.
- Finite `TrackMinBound`/`TrackMaxBound` types for intrinsic, fixed,
  percentage and flex/minmax tracks. A flex minimum or recursive minmax
  cannot survive native decoding as a renderer fallback.
- One generation-qualified `NodeKey -> LayoutEntry` map. Taffy owns styles
  and topology; the entry owns only backend identity, current visibility/
  participation facts and the last installed child-list revision. Preparation
  creates each new style once and avoids per-node searches over other new
  nodes. Unchanged parent revisions and equal styles do not dirty layout.
- Two-phase edge replacement, including retiring subtrees. Old child edges
  are severed before final surviving child lists are installed; explicit
  retired members are removed in postorder without repeatedly scanning a
  shrinking sibling list. Canonical document validation is not duplicated.
- Independent renderer-hidden, semantic display-none and control
  participation facts. Hidden/inactive occurrences retain their identities.
- Pure captured-leaf measurement requests with separate known dimensions
  and definite/min-content/max-content available constraints. Measurement
  invalidation marks Taffy dirty; product revisions remain upstream-owned.
  Failed measurements cannot leave cached placeholder geometry that becomes
  a successful retry.
- Disabled Taffy rounding, checked accumulated absolute-edge quantization,
  and inner-content-box widths for floor-width wrapping. Zero-width content
  remains distinct from width one.
- An explicit M1 guard for newly authored geometry that cannot yet be
  rendered. Unset/reset values do not trigger that guard. Its deletion gate
  is the next direct-rendering slice within T6, not T7.

Parent validation:

| Check | Result |
|---|---|
| Rust formatting and workspace all-features check | passed |
| Workspace all-features tests | passed: 756 core tests, 19 native library tests, one native synchronization test, nine generator tests; existing ignored tests remain ignored |
| Actual adapter regressions | passed: 13 tests covering sparse topology/revisions, retirement/rescue, hiding/participation, measurement invalidation/failure, inner content width and quantization |
| Strict repository Clippy gate | passed; repository warn-mode audit findings remain; the initial new `let_and_return` error was corrected rather than suppressed |
| Generated ABI consistency | passed |
| TypeScript, changed-file Biome, binding, declaration and ownership gates | passed; two existing binding-check non-null-assertion warnings remain |
| Fresh default native staging and packaged smoke | passed on macOS ARM64 |
| All package/consumer Bun tests | passed: 84 tests, 372 expectations |

Logs are `/tmp/t6-foundation-final-*.log` and
`/tmp/t6-parent-final-adapter.log`. The workspace tests preceded a
semantics-preserving removal of a redundant Rust return binding; that evidence
is reused. Strict Clippy, native staging/smoke and the Bun suite ran afterward.
The TS checks and Bun suite include the final normalization simplification.

Current default addon:

    packages/iyon-tui/native/iyon-tui-native.node
    SHA-256 fbdc88acfe2ba556f53d656238663e8adaa750e8e571f54969d260c2d1850c24
    6,806,496 bytes; darwin-arm64, default N-API

The initial fanout failure was a regression in this slice, not a baseline
failure: the first temporary guard rejected new property IDs even when their
effective value was Unset. The archived M1 baseline passed the exact test.
Parent review identified the cause, the guard was corrected, and the final
19-test native suite passed. No failure is waived as unrelated.

### T6 direct occurrence host cutover — implementation checkpoint

The production terminal host now synchronizes the accepted occurrence
snapshots directly into the derived Taffy adapter. Ordinary Box layout,
including Grid, no longer calls the legacy occurrence-to-View adapter or its
unsupported-geometry guard. The direct candidate retains one
generation-qualified occurrence geometry map and builds a disposable paint
tree from those resolved rectangles. ContentHost leaves capture each content
product before layout, keep Taffy measurement callbacks pure, and perform at
most one bounded final-width measurement pass. The content key accepts real
zero-width constraints; no width-one or maximum-width sentinel is introduced.

Native control registration and event routing remain the existing concrete
Editor/Scroll/Animation mechanics. Their View is projected only inside the
Taffy-assigned control box, while occurrence geometry, focus visibility and
animation participation remain occurrence-keyed. Taffy is thread-affine in the
pinned release, so each host owns a dedicated renderer-driver command boundary
and keeps its persistent derived tree on that driver thread rather than adding
an unsafe Send assertion to HostInner.

History roots use the same occurrence snapshot synchronization, direct Taffy
root stacking, and occurrence-keyed painting as the body. Physical prefix
acknowledgement continues through the existing native transfer owner using a
narrow transparent content-shell descriptor; the exact candidate rows and
receipt ownership remain unchanged. FollowEnd/NativeFrontier placement and
bounded transfer receipts remain part of the terminal History contract;
component-only Surface export is separately scheduled. No View fallback is
used for ordinary History layout. The old `legacy_scene.rs` ordinary adapter and
its stale recipe tests were removed rather than retained as a fallback. The
remaining local View projection is limited to native control pixels and the
History physical transfer descriptor and is deleted/rewritten at the T7
semantic-content gate.

Focused current-source evidence for this checkpoint includes the direct
occurrence row/grid geometry test, the native host Grid/content/geometry test,
the all-features host receipt/control/History tests, and the all-features
content tests. The full package/native-addon and contract checks remain
parent-owned final gates; archived baseline captures are diagnostics only and
are not acceptance gates. Linux execution is not claimed locally.

Separate matching M1 source/addon captures are preserved under
`/tmp/t6-m1-baseline`: 18 integral UI/control fixtures and 27 content
fixtures at multiple widths, including complete cell styles. These are
comparison inputs for diagnosis, not acceptance criteria or evidence that the
replacement must reproduce the old allocator. Linux native execution,
fractional paint/clip behavior and the T6/T7 performance gates remain pending.

#### Selected terminal layout semantics

Parent review selected the declared Iyon Flex/Grid behavior rather than
reconstructing the old allocator through sibling-specific shrink factors.
Ordinary Row/Column children use `flexShrink: 1` unless explicitly overridden.
The Body uses cross-axis stretch: an auto-width Box occupies the available
width, and its ref reports that outer allocation rather than the width of its
fitted text. Explicit dimensions and alignment remain authoritative.

Each retained root has one stable, derived Taffy constraint boundary. Its
tracks use `minmax(0, 1fr)`: the finite terminal constraint must not be widened
by an automatic min-content track minimum before text is measured. Intrinsic
and definite passes use the same occurrence topology. There is no overflow
subtree clone, sibling-position allocation override, or fractional compatibility
shrink constant. Content paint uses the exact width-specific captured product;
explicit/Fill widths are refined to the resolved logical content-box width.

The canonical React plus Content plane and Taffy's declared Flex/Grid and
intrinsic-sizing behavior are the layout oracle. Terminal quantization and
clipping are backend realization details; source Unicode segmentation,
provenance, candidate/receipt ownership, and physical safety remain explicit
contracts. The archived M1 comparison captures can locate changes but cannot
require old rows/styles or authorize a legacy allocator. This decision keeps
the shared React/content plane suitable for a future GPUI host; GPUI remains
out of scope. The native root regression requires the final hard line to
remain visible after wrapping an unbreakable line in a narrow terminal.

These decisions do not claim completion of the T7 content-lowering/deletion,
performance, Linux, or final workspace acceptance gates.
