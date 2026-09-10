# DOM-like runtime implementation checklist

**Scope:** T0 through T5 TypeScript cutover. Native old View ABI/generated
outputs and ordinary Rust ViewState remain only as the explicitly approved
next-slice residue; handwritten TypeScript View/composition/structural
publication and old authoring controls are deleted.
This document is the implementation ledger for IYON-DOM-LIKE-RUNTIME-HANDOFF.md;
it is not a claim that the M1/M2 migration is complete.

**Baseline:** branch agent/dom-occurrence-runtime, accepted T1 source HEAD
bb0c599fce1a35b8e478d2868e72a851194f4297. The handoff identifies
1e935406c707ad42eb819d259f0a456f0a1129e7 as the implementation baseline;
The T0/T1 schema and occurrence core are committed at that accepted SHA;
the T2 native ingress tranche is accepted and committed at 6ae68b2.
The atlas at docs/architecture/atlas-4355c02 is historical navigation, not
the current-source authority.

## Progress

| Tranche | Status | Evidence |
|---|---|---|
| T0 — baseline and behavior map | **complete** | Baseline commands and route evidence below; no production source was changed while the baseline was collected. |
| T1 — finite schema and occurrence core | **accepted; committed bb0c599** | Parent source review and focused/ownership reruns accepted the typed schema/core tranche. |
| T2 — qualified native ingress/resource preparation | **accepted** | Parent reviewed the qualified ingress, generated finite payload forms, resource preparation/install path, shared controls, Source lifecycle and rejection regressions. Workspace and Bun suites passed; remaining T1 lint failures are recorded below. No renderer, React, Taffy, or old-route deletion was attempted. |
| T3 — minimal React renderer | **accepted** | Parent reviewed the React shim, speculative instances, journal/acknowledgement path, hook lifecycles, typed portals, finite properties, native resource changes and public consumer. Current-source full Bun suite: 164 passed; native UI commit tests: 12 passed. TypeScript, Biome, generated ABI, binding, ownership and formatting checks pass. Clippy completes with warnings. Acceptance is limited to the minimal desired-state renderer, not T4 frame realization or M1/M2 cutover. |
| T4 — current renderer, controls, exact frame state | **accepted** | Parent reviewed the canonical adapter, sparse resource synchronization, native controls/events, exact frame and geometry ownership, metadata-only completion, accepted History lifecycle, asynchronous physical transfer, close joining, and failure/replay barriers. Broad integration checks and the final zero-progress close correction passed; evidence and remaining migration gates are recorded below. |
| T5 — M1 TypeScript cutover/publication deletion | **TypeScript slice accepted; M1 incomplete** | Parent reviewed the canonical React route, explicit resource/receipt lifetimes, native diagnostics/input, output cancellation, consumer ports and physical History witness. Handwritten View/composition/structural publication and JS frame polling are deleted. Native old View ABI/generated outputs and ordinary Rust ViewState deletion plus the final M1 broad gate remain next. |
| T6 — direct terminal Taffy integration | remaining | Add the approved pinned Taffy adapter and finite Flex/Grid semantics. |
| T7 — content lowering and M2 deletion | remaining | Direct semantic-content realization; delete the temporary legacy adapter and redundant general View layout. |

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

The approved next-slice residue is limited to native old View ABI/generated
outputs, native old control mechanics still used by the private renderer, and
ordinary Rust ViewState. None is a TypeScript production route or ownership
authority.

### T5 TypeScript slice — parent acceptance evidence

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

Final normal addon (darwin-arm64, default N-API):

    packages/iyon-tui/native/iyon-tui-native.node
    SHA-256 f3198b60883af4e4eb21b6f0acb37c70948a690313045c3e86739b065907c4df
    7,725,040 bytes

Parent checks: TypeScript; declaration/binding/generated-ABI/ownership gates;
83 package/consumer tests; packaged native smoke; rustfmt; 61 native host tests;
Clippy on core/native all-targets; pinned Biome format for changed permanent
TypeScript plus lint/complexity. Biome/Clippy warning debt remains reported,
not a warning-free claim. Native correction-stage evidence of 55 content tests
and 66 N-API tests (one ignored) remains applicable to unchanged owners.
Broad Rust workspace testing waits for the native/schema/state M1 deletion gate.

The benchmark now reads actual opt-in Rust projection/layout/paint counters,
counts submitted UI records separately, and observes native output and a visible
receipt. Parent ran it with `perf-counters`, then restored the normal addon above.
Logs are `/tmp/t5-parent-*.log` and `/tmp/t5-parent-benchmark.jsonl`.
External migration and benchmark instructions are in
`docs/migration/REACT-RUNTIME.md`. The T4/prerequisite sections below retain their
historical validation results; they do not override this current slice status.

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
  the direct Taffy/content route and its parity evidence are accepted.
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

The T5 cutover now removes all handwritten TypeScript View/composition/
publication and old consumer routes. Remaining gates are native old View
ABI/generated-output and ordinary Rust ViewState deletion, direct Taffy layout,
semantic content lowering, and the separately scoped Surface/physical-export/
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
| crates/iyon-tui/src/retained_state | 2,006 |
| crates/iyon-tui/src/scene | 6,824 |
| packages/iyon-tui/tests | 4,297 |

The current production route remains:

    Tui.render
      -> runtime/runtime.ts
      -> OwnedBuilderRoot / RetainedExecutionRuntime
      -> RetainedRootBoundary
      -> retained-dag.ensureSemanticNative
      -> generated structural N-API
      -> NativeViewRuntime / NativeRef
      -> NativeTuiHost.setDesiredViewRef
      -> TuiHost.set_desired_view
      -> environment pending queue
      -> SceneHost layout/paint
      -> physical receipt

T1 intentionally does not route existing rendering through the new document.
The superseded owners remain until T5's cutover gate; no compatibility
reconciler was added.

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

The existing tools/tui-abi-gen now loads both view_abi.toml and ui_abi.toml. It
remains one generator and one generated-output check; no parallel schema tool
was introduced. UI outputs are:

- crates/iyon-tui/src/occurrence/generated.rs;
- packages/iyon-tui/src/transport/ui/generated/ui_schema.ts;
- packages/iyon-tui/src/transport/ui/generated/ui_abi_manifest.json;
- docs/architecture/generated/UI-ABI-REFERENCE.md.

The historical View ABI outputs remain generated during this tranche. Their
generator fingerprint and manifest output list changed because the one
generator now includes the UI schema; deleting them is a T5 operation, not
silently done in T1.

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
| old composition/structural/state tests and benchmarks | T5 | deleted as superseded View/publication-only suites; generated native ABI conformance residue remains under the next-slice gate |

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

This is deliberately a prerequisite status rather than M1 acceptance. The
legacy `Tui.render` route, public View/composition exports, native View
publication tables, root/lease machinery and ordinary ViewState remain tracked
for the full T5 cutover/deletion gate. Rust and the staged native addon are
unchanged by this slice; the accepted T4 addon provenance remains
`bb685057f27b491fbae933dc17897d32a1b880b2e602e409fcaa4ce1ec645d2e`.

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
barrier rather than ignored. The correction does not remove the legacy route,
ordinary ViewState, or the adapter; physical History export still requires a
public React consumer witness before M1 acceptance.
T4's accepted source is `ca1216335d57569a4171d10b86bcf3aad0872671`; prior
acceptance does not exempt these M1 parity requirements.
The earlier React prerequisite is committed at
`a6d70b22373eefb11e28670eff8e4576008a774c`. This alignment scope does not claim
horizontal text alignment or general Flex/Grid parity; their content/layout
ownership remains required work in the T6/T7 migration.

## Remaining proof and risks

- The occurrence document is connected to the React mutation renderer and the
  existing terminal renderer through one-way M1 adaptation. Native frame
  presentation, receipt ownership, environment scheduling, controls, typed
  events and History routing were accepted in T4. Full T5 production cutover
  and the public physical-History consumer witness remain unaccepted.
- T2 native ingress qualification and acknowledgement allocation have focused
  witnesses, including napi8 type-tag rejection for wrong wrapped classes and
  prototype spoofing, detached-buffer rejection, and local-count admission.
- T4 scheduler ownership now relies on compiler-checked `Send` data rather
  than unsafe TuiHost/TuiEnvironment markers. Erased components, queued
  output payloads, retained callbacks/routes, tick drivers and text-output
  projectors carry the required portable bounds; only the actual mutex-owned
  core crosses the native driver boundary. This tightening of the in-repo Rust
  callback/component API was accepted in T4.
- Presentation receipts retain one native oneshot receiver and register a
  queue-only weak waker. The environment has a separate external lifetime
  owner from worker queue state; receipt completion wakes all host-local
  presentation observers without a per-receipt thread or correlated result
  slot. Deterministic owner tests opt into the explicit manual environment;
  production construction uses the same testable driver loop.
- The accepted T2 correction adds sparse final binding planning, individually
  validated/coalesced literal actions, and pre-write multi-Source guard
  validation. The minimal React acceptance route was accepted in T3 and
  physical renderer integration was accepted in T4.
- T3's React/reconciler contract remains pinned and isolated; native frame
  realization is now driven by the environment's native scheduler and receipt
  notifications rather than a TS frame loop.
- Each Iyon React root owns its pinned reconciler instance. This prevents a
  rejected commit's scheduled work from contaminating a later root; the
  original-order focused suite includes a faulted duplicate-token root followed
  by a healthy new host/root mount, update and unmount.
- Same-host portal roots use their typed owner correspondence: same-owner
  reorder does not enter the ordinary child list, mixed ordinary/portal
  placement skips portal anchors, owner transfer retires/recreates the typed
  root, and root retirement plus cross-host rejection are covered by focused
  tests. Current terminal presentation ordering is now covered by the T4
  confirmed-frame path; parent review remains the acceptance gate.
- Lazy Port/Connector tokens are immutable and occurrence materialization is
  commit-only. Port and Connector hook owners are independent; resources
  survive occurrence retirement and consumer gaps until their real owner
  cleanup, preserve identity during live transfer, and dispose in dependency
  order through the coordinator. Caller-owned public Source resources remain
  outside this renderer's disposal path. Focused tests cover split-owner
  lifetime, committed absent-consumer replacement, same-instance and
  independent A/B/A selection, source/literal/lazy transitions, transfer,
  blocked dependency release, cross-root rejection, and StrictMode cleanup;
  parent source review accepted the bounded cross-host/portal ownership contract.
- React literal Content is intentionally plain-only in T3: strings, numbers,
  and bigints are supported, while legacy `TextContent`/`RawText` wrappers and
  Markdown/diff/ANSI/annotation lowering are rejected explicitly rather than
  silently discarded. Broader content families and semantic span lowering
  remain bounded T4/T7 work.
- Native UI close now has Open/Closing/Closed state, drops the occurrence
  document only after all resource cleanup succeeds, rejects body-handle and
  commit ingress after close, and retains failed cleanup ownership for retry.
  JS root close likewise avoids a fallback second close and clears accepted JS
  references only after native cleanup succeeds.
- The renderer now has both failure and interruption evidence: an abandoned
  render confirms HostConfig candidate creation followed by a render error and
  zero native creation calls, while the T5 prerequisite witness observes real
  candidate creation before native commit and then exercises replacement,
  yielded unmount, and yielded close. A controlled discrete-priority negative
  experiment fails the witness when the same workload is forced synchronous;
  it is recorded outside the repository rather than retained as a production
  test or scheduler selector.
- Public refs publish supported overrides/clear operations through the same
  coordinator. Focus and visible geometry use confirmed frame metadata and
  native control ownership; unsupported non-control occurrences reject
  explicitly rather than fabricating geometry from desired state.
- React presentation-capable props now accept finite NUL-free `styleStates`
  records, and occurrence refs publish layer-1 style-state overrides through
  the same UI commit coordinator. Declared updates remain masked by active
  overrides and clearing reveals the newest declaration. Focused Rust/native
  and React boundary tests cover border/style precedence, theme selection,
  override masking/clear, no-op style-state rerenders, supported row alignment,
  and explicit unsupported alignment errors. This remains a T5 prerequisite
  correction, not M1 publication/deletion acceptance.
- Parent's final neutral-axis correction rebuilt the canonical darwin-arm64
  default N-API addon: `packages/iyon-tui/native/iyon-tui-native.node`, SHA-256
  `d11862d1a4f2f9625286c9fd1aafb5a42959f0a247c5bada45379ce43f8b98eb`,
  7,609,376 bytes. The public alignment witness compares neutral Box output
  with the default, checks bottom-aligned Row output, and rejects horizontal
  center through the production/native boundary. The expected pre-fix failures
  are in `/tmp/iyon-t5-parent-alignment-prefix-failure.log` and
  `/tmp/iyon-t5-parent-neutral-negative.log`.
- Final current-source checks passed 58 React/consumer/demo tests with 271
  expectations, 21 legacy-adapter Rust tests, native-host check and Clippy,
  rustfmt, TypeScript, and pinned Biome. Logs are
  `/tmp/iyon-t5-parent-parity-*.log`. The correction pass's 28 occurrence-commit
  tests, declaration, binding and ownership checks are reused for unchanged
  paths. Full workspace Rust/Bun validation remains the final M1 gate, not a
  claim of this bounded prerequisite acceptance.
- Existing strict lint debt is recorded, not swept: baseline architecture
  checks passed, while broad warning/clippy cleanup remains outside this slice.
- The generated old View ABI remains intentionally present until M1. Its
  continued presence is a tracked migration remainder, not an alternate new
  route.
