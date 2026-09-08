# DOM-like runtime implementation checklist

**Scope:** T0 and T1 only. This document is the implementation ledger for
IYON-DOM-LIKE-RUNTIME-HANDOFF.md; it is not a claim that the M1/M2 migration
is complete.

**Baseline:** branch agent/dom-occurrence-runtime, source HEAD
74ae1b967042e5cc6fa99a4faf92b25da5befc15. The handoff identifies
1e935406c707ad42eb819d259f0a456f0a1129e7 as the implementation baseline;
HEAD contains only the approved handoff documentation after that baseline.
The atlas at docs/architecture/atlas-4355c02 is historical navigation, not
the current-source authority.

## Progress

| Tranche | Status | Evidence |
|---|---|---|
| T0 — baseline and behavior map | **complete** | Baseline commands and route evidence below; no production source was changed while the baseline was collected. |
| T1 — finite schema and occurrence core | **accepted by parent** | tools/tui-abi/ui_abi.toml, generated schema outputs, crates/iyon-tui/src/occurrence/, focused Rust/generator tests, and the parent-finding corrections below. Acceptance is limited to the typed core; no renderer, React, N-API UI decoder, Taffy, or old-route deletion was attempted. |
| T2 — qualified native ingress/resource preparation | remaining | Add commitUiV1 only after the typed core contract is reviewed; qualify ArrayBuffer ownership and native object identity. |
| T3 — minimal React renderer | remaining | React mutation host config and JS-only candidates; no second reconciler or immutable-View compatibility path. |
| T4 — current renderer, controls, exact frame state | remaining | One-way native adapter, native scheduling, controls/content/History integration, exact receipt state. |
| T5 — M1 cutover/publication deletion | remaining | React-only production frontend; delete old composition, View publication, leases, paths and ordinary ViewState after consumer gates. |
| T6 — direct terminal Taffy integration | remaining | Add the approved pinned Taffy adapter and finite Flex/Grid semantics. |
| T7 — content lowering and M2 deletion | remaining | Direct semantic-content realization; delete the temporary legacy adapter and redundant general View layout. |

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
- commit.rs: typed UI operations, local creation ordinals, sparse tree and
  resource overlays, final Port/Control owner indexes, whole-batch rejection,
  reserved apply, and the exact eight-word acknowledgement header followed
  by four words per created handle.

OccurrenceDocument::prepare_ui_commit performs decoding-equivalent typed
validation, sparse overlay interpretation and reservation without publishing
logical records. apply_prepared_ui_commit installs the prepared records and
returns the acknowledgement allocated by preflight. It does not render, parse,
write to a terminal, invoke callbacks or use N-API. This is the T1 owning
boundary; T2 will add the qualified native decoder around it.

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
| packages/tui-consumer-fixture/src/consumer.ts | T3 | pending React port using only documented @iyon/tui entrypoints |
| packages/tui-consumer-fixture/tests/consumer.test.ts | T3/T5 | pending production occurrence-root witness |
| packages/tui-consumer-fixture/tests/scoped-invalidation.test.ts | T3/T5 | pending replacement with occurrence delta/invalidation witness |
| packages/iyon-tui/scripts/smoke-native.ts | T2/T3/T5 | pending new production bridge smoke route |
| packages/iyon-tui/tests/fixtures/tui_demo.ts | T3/T5 | pending React fixture |
| controls (TextInput, ScrollPane, ViewSlot) | T4 | native mechanics retained; composition-only slots are not ported in T1 |
| content Source/Funnel/Port/Connector | T2/T4 | existing direct Source data lane retained; no payload fallback added |
| History/native scrollback | T4/T8 | current physical behavior retained until Surface migration |
| old composition/structural/state tests and benchmarks | T5 | baseline-only in this tranche; replace/delete at cutover, never duplicate in a fake runtime |

## Remaining proof and risks

- The new document is not connected to a renderer or native addon yet.
- T2 must qualify non-shared, attached ArrayBuffers, detached buffers and
  nonzero offsets on Bun 1.4.0, then prove acknowledgement allocation before
  apply.
- The existing TuiHost/TuiEnvironment Send/Sync assertions and erased callback
  payloads remain untouched; moving the document to a mutex is not a
  soundness fix.
- T2 must split resource preparation/install from output/frame preparation.
- T3 must pin and isolate the approved React/reconciler contract; no React
  dependency was added in T1.
- Existing strict lint debt is recorded, not swept: baseline architecture
  checks passed, while broad warning/clippy cleanup remains outside this slice.
- The generated old View ABI remains intentionally present until M1. Its
  continued presence is a tracked migration remainder, not an alternate new
  route.
