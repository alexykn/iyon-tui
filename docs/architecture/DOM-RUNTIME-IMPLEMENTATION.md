# DOM-like runtime implementation checklist

**Scope:** T0 through T3. This document is the implementation ledger for
IYON-DOM-LIKE-RUNTIME-HANDOFF.md; it is not a claim that the M1/M2 migration
is complete.

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
| T4 — current renderer, controls, exact frame state | remaining | One-way native adapter, native scheduling, controls/content/History integration, exact receipt state. |
| T5 — M1 cutover/publication deletion | remaining | React-only production frontend; delete old composition, View publication, leases, paths and ordinary ViewState after consumer gates. |
| T6 — direct terminal Taffy integration | remaining | Add the approved pinned Taffy adapter and finite Flex/Grid semantics. |
| T7 — content lowering and M2 deletion | remaining | Direct semantic-content realization; delete the temporary legacy adapter and redundant general View layout. |

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
  `whenVisible`/`whenContentVisible` barriers. Those barriers reject with
  `T3_FRAME_BARRIER_UNREALIZED` until T4 installs frame scheduling/presentation;
  they do not pretend acceptance is terminal output;
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

T3 is accepted. T4 owns terminal frame
realization, scheduling, content/control integration and exact presentation
barriers; T5 owns canonical production cutover and deletion of old composition
and View publication.

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
| packages/tui-consumer-fixture/src/consumer.ts | T3 | legacy route retained until T5; React port is in `src/react-consumer.ts` using only documented entrypoints |
| packages/tui-consumer-fixture/src/react-consumer.ts | T3 | accepted public React consumer |
| packages/tui-consumer-fixture/tests/consumer.test.ts | T3/T5 | legacy behavior tests retained; React acceptance is in `tests/react-consumer.test.ts` |
| packages/tui-consumer-fixture/tests/react-consumer.test.ts | T3 | accepted public React occurrence-root witness |
| packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts | T3/T5 | pending replacement with occurrence delta/invalidation witness |
| packages/iyon-tui/scripts/smoke-native.ts | T2/T3/T5 | pending new production bridge smoke route |
| packages/iyon-tui/tests/fixtures/tui_demo.ts | T3/T5 | old fixture retained until production cutover; React route covered by `tui_react_renderer.test.ts` |
| controls (TextInput, ScrollPane, ViewSlot) | T4 | native mechanics retained; composition-only slots are not ported in T1 |
| content Source/Funnel/Port/Connector | T2/T4 | existing direct Source data lane retained; no payload fallback added |
| History/native scrollback | T4/T8 | current physical behavior retained until Surface migration |
| old composition/structural/state tests and benchmarks | T5 | baseline-only in this tranche; replace/delete at cutover, never duplicate in a fake runtime |

## Remaining proof and risks

- The occurrence document is now connected to the bounded T3 React mutation
  renderer and qualified native acceptance seam. Native frame presentation,
  scheduling receipts and the terminal renderer remain T4 work.
- T2 native ingress qualification and acknowledgement allocation have focused
  witnesses, including napi8 type-tag rejection for wrong wrapped classes and
  prototype spoofing, detached-buffer rejection, and local-count admission.
- The existing TuiHost/TuiEnvironment Send/Sync assertions and erased callback
  payloads remain untouched; moving the document to a mutex is not a
  soundness fix.
- The accepted T2 correction adds sparse final binding planning, individually
  validated/coalesced literal actions, and pre-write multi-Source guard
  validation. The minimal React acceptance route is accepted in T3; physical
  renderer integration remains the T4 acceptance gate.
- T3's React/reconciler contract is pinned and isolated; native frame and
  presentation realization are intentionally still unrealized until T4.
- Each Iyon React root owns its pinned reconciler instance. This prevents a
  rejected commit's scheduled work from contaminating a later root; the
  original-order focused suite includes a faulted duplicate-token root followed
  by a healthy new host/root mount, update and unmount.
- Same-host portal roots use their typed owner correspondence: same-owner
  reorder does not enter the ordinary child list, mixed ordinary/portal
  placement skips portal anchors, owner transfer retires/recreates the typed
  root, and root retirement plus cross-host rejection are covered by focused
  tests. Presentation ordering remains a later T4 concern.
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
- The abandoned-render witness now confirms actual HostConfig candidate
  creation followed by a render error and zero native creation calls. A true
  concurrent scheduler interruption remains a separate witness.
- Public refs publish supported overrides/clear operations through the same
  coordinator. Focus remains an explicit T4 interaction-executor error, and
  visible geometry rejects with `T3_GEOMETRY_UNREALIZED`; neither is fabricated
  from desired state.
- Existing strict lint debt is recorded, not swept: baseline architecture
  checks passed, while broad warning/clippy cleanup remains outside this slice.
- The generated old View ABI remains intentionally present until M1. Its
  continued presence is a tracked migration remainder, not an alternate new
  route.
