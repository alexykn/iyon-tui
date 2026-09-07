# 45 — Coverage reconciliation

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Scope: repository manifest plus reconciliation of all preceding assignments 01–44.
- Investigation mode: read-only static inspection.
- No source files, configuration, generated output, lockfile, or running service was modified.
- No build, test, benchmark, package staging, native loading, or runtime command was executed in this assignment.
- Historical benchmark and test results remain historical evidence and were not treated as execution evidence for the baseline.

I read the required contract and context first:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`

The contract establishes that:

- current source proves what executes;
- only PERF-13, API-H, L1, and PRE-V5 documents have historical handoff authority;
- older documents may explain chronology but are not normative;
- handoffs are an intended-contract oracle, not proof of current execution;
- the repository is a generic terminal framework, not an Iyon application or product-plugin repository;
- no V5 disposition decisions belong in this report.

### Preceding report artifact coverage

All 44 preceding Markdown artifacts were present and reviewed as the reconciliation input. They are listed exhaustively in §10. The artifact set is complete by assignment numbering: 16 Rust subsystem reports, 2 native reports, 5 TypeScript subsystem reports, 3 support reports, 3 wiring reports, 11 trace reports, and 4 audit reports.

The atlas README explicitly states that assignment 45 reads assignments 1–44 and that the parent is the sole repository writer (`README.md:18–19`, `README.md:64–68`). The report artifacts consistently identify themselves as source-static investigations; none of the reports provides current-baseline executed validation sufficient to change that classification.

### Evidence classification used here

- **Current-source fact:** directly supported by baseline source or manifest.
- **Historical authority:** PERF-13/API-H/L1/PRE-V5 material, used only for intended-contract comparison.
- **Scout claim:** a statement made by a preceding report.
- **Reconciled finding:** a scout claim compared with another scout report and/or current source.
- **Candidate issue:** evidence-backed discrepancy or weak seam requiring later grouped evaluation; not a final acceptance or V5 disposition.
- **Coverage gap:** a claim that cannot be established from the inspected source, artifact, or non-executed state.

### High-level reconciliation result

The preceding reports cover the major architectural surfaces and identify many of their own limits. The important remaining reconciliation findings are:

1. **The artifact set is complete, but source-manifest coverage is not the same as repository-wide behavioral coverage.** The tracked manifest includes source, tests, generated outputs, fixtures, historical documents, benchmark artifacts, and replay-frame names, but it excludes ignored build/native outputs and does not itself prove that every listed file was read by an assignment.
2. **The structural production-route claim is narrower than the generated ABI surface.** Current production `Tui.render()` uses deferred retained publication; exact-root and edit-transaction helpers remain implemented and test-reachable or exported in generated ABI surfaces, but no production caller was found for the exact-root method.
3. **The content FFI report’s regex uncertainty can now be source-checked.** `ffi.ts:502` literally contains `/\\s|\\0/u`, which does not implement the error message’s stated whitespace/NUL validation. This is a concrete candidate issue, not merely an inspection-escaping uncertainty.
4. **The generated ABI manifest omits `buffer_used_of` metadata even though the canonical TOML schema and generator model use it.** This is a manifest/documentation completeness gap that can affect consumers reconstructing buffer pairings.
5. **The numeric state-kind mapping is intentionally coarser than the transport kind table, but the intent is not proven.** TypeScript semantic kinds and Rust `semantic_state_node_kind()` differ by design for some kinds, notably Diff and ContentMax. Existing reports correctly leave this as an open contract question; it should not be silently described as either a confirmed bug or a confirmed intentional design.
6. **Current execution coverage is substantially weaker than source inventory coverage.** The reports repeatedly state that tests, builds, staging, native artifact loading, terminal behavior, benchmark paths, and cross-platform behavior were not executed.
7. **The repository contains no full reference application or Iyon product plugin.** Generic fixtures and benchmark consumers must not be used as evidence that product/application routes exist.

## 1. Responsibility and structure

### Repository manifest reconciliation

The tracked source manifest at `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt` is broad and includes:

- CI and repository instructions;
- three Cargo workspace members;
- native handwritten and generated code;
- the full `crates/iyon-tui/src` tree;
- Rust tests embedded in source modules;
- generated ABI/schema files;
- TypeScript public API, composition, runtime, transport, tests, and fixtures;
- benchmark scripts and checked-in benchmark artifacts;
- historical API-H/PERF-12/PERF-13/PRE-V5 documents;
- `examples/parrot_test` replay frames;
- root package/build metadata.

The manifest is not an exhaustive runtime artifact manifest. The following are intentionally absent or outside its evidence strength:

- ignored staged native binaries under `packages/iyon-tui/native/`;
- Cargo build output under `target*`;
- any untracked or ignored generated output not represented in the tracked manifest;
- current runtime state of an installed native artifact;
- external consumers or product plugins;
- actual terminal emulator behavior;
- report artifacts under `reports/pre-v5-l1/` as source-baseline files. Those reports exist in the worktree and are referenced by assignment 44 and other audits, but they are historical/supporting records rather than tracked implementation files in the source manifest.

`find` confirmed the current package tree includes the same source/test files enumerated by the manifest, including the consumer fixture and all package tests. The manifest therefore appears structurally complete for tracked source, but it must not be interpreted as proof that every generated, ignored, historical, or runtime artifact was read or executable.

### Cargo/package ownership

Current manifests establish this dependency structure:

```text
Cargo workspace
├── crates/iyon-tui
│   └── generic Rust retained runtime, presentation, content, backend, tests
├── crates/iyon-tui-native
│   └── cdylib/rlib N-API/native facade
│       └── depends on crates/iyon-tui with native-host
└── tools/tui-abi-gen
    └── ABI schema parser/generator

TypeScript workspace
├── packages/iyon-tui
│   └── public facade, composition, runtime, transport, tests, benchmarks
└── packages/tui-consumer-fixture
    └── external-style public consumer fixture
```

Evidence:

- `Cargo.toml:1–7` declares the three Rust workspace members.
- `crates/iyon-tui/Cargo.toml:6–23` states that the core crate is unpublished runtime implementation for the native binding, not the supported Rust authoring package.
- `crates/iyon-tui-native/Cargo.toml:8–20` declares `cdylib`/`rlib`, native-host dependency, and feature flags.
- `tools/tui-abi-gen/Cargo.toml:8–30` declares the generator binary and schema/rendering dependencies.
- Root `package.json:7–18` exports the TypeScript package, testing entry point, native staging script, and two workspaces.
- Root `package.json:19–36` separates ABI generation, typechecking, linting, Rust tests, staging, smoke tests, content benchmarks, and package tests.

This agrees with the preceding support and wiring reports. It contradicts any older or generic description that treats `crates/iyon-tui` as a supported public Rust UI-authoring package.

### Physical LOC evidence status

No exact LOC reconciliation was executed. The preceding reports generally use source-line extents or rounded physical estimates rather than `wc -l`; several explicitly state this limitation, including:

- `subsystems/support/24-codegen.md:1336–1338`;
- `subsystems/support/25-tests-fixtures.md:1478–1480`;
- `subsystems/support/26-bench-build-examples.md:1353–1357`;
- `audits/42-benchmark-integrity.md:969–975`;
- `audits/43-test-contracts.md:113–114`;
- `audits/44-history-doc-drift.md:1413–1419`.

The reconciliation report therefore does not invent aggregate production/test/generated LOC totals. The meaningful structure finding is ownership and evidence coverage, not an additional approximate count.

### Responsibility map from the preceding reports

| Area | Current owner | Reconciliation status |
|---|---|---|
| Generic Rust runtime/application host | `crates/iyon-tui/src/application/`, `scene/`, `backend/`, `terminal/` | Covered by assignments 01, 03, 15, 27, 34, 41 |
| Rust retained structure/state | `component/`, `scene/`, `retained_state/`, native binding | Covered by assignments 02, 03, 06, 17, 22, 29, 31, 32 |
| Rust presentation/layout/paint | `presentation/`, `physical/`, `geometry/`, theme | Covered by assignments 04, 05, 11, 14, 33, 36 |
| Rust content/projection/smoothing | `content/`, `projection/`, `stream/` coordinate types | Covered by assignments 08–10, 23, 35 |
| Rust History/scrollback | `history/`, `scroll.rs`, host/native frontier | Covered by assignments 07, 12, 26, 37, 44 |
| TypeScript authoring/composition | `packages/iyon-tui/src/api/`, `composition/` | Covered by assignments 19, 20, 30, 31–33 |
| TypeScript runtime | `packages/iyon-tui/src/runtime/` | Covered by assignments 21, 29, 34, 39, 40, 41 |
| TypeScript/native transport | `transport/abi`, `transport/native`, `transport/structural`, `transport/state`, `transport/content` | Covered by assignments 22, 23, 28, 29, 30–35, 38–41 |
| Native addon | `crates/iyon-tui-native/src/` | Covered by assignments 17, 18, 24, 27–29, 41–43 |
| ABI generator/generated output | `tools/tui-abi*`, generated Rust/TS/header files | Covered by assignments 24, 26–29, 42–43 |
| Test/fixture infrastructure | Rust tests, TS tests, consumer fixture | Covered by assignments 25, 43 |
| Benchmarks/build/examples | `bench/`, `perf*`, scripts, workflows, replay frames | Covered by assignments 26, 42 |
| Historical/document drift | `docs/history/`, L1 records, current source | Covered by assignment 44 |

No major assigned subsystem lacks a corresponding preceding report artifact. The remaining concerns are seams between owners and unexecuted behavior, not missing assignment files.

## 2. Types, APIs and contracts

### Intentional public surfaces

The current public boundary is TypeScript-oriented:

- root package exports are declared in `package.json:7–10`;
- the core Rust crate is `publish = false` and explicitly described as internal runtime implementation (`crates/iyon-tui/Cargo.toml:6–9`);
- Rust `lib.rs` exposes most runtime modules as `pub(crate)` or private;
- hidden binding exports exist for native transport but are not supported Rust authoring APIs.

This agrees across assignments 16, 19, 26, 27, 28, 41, and 43. A report describing hidden Rust binding functions as “public Rust UI API” would be inaccurate; a report describing them as nonexistent would also be inaccurate. They are compiled bridge/runtime machinery.

### Structural route versus ABI surface

Current TypeScript source confirms two distinct facts:

1. The canonical production route uses deferred retained publication:

   - `packages/iyon-tui/src/runtime/runtime.ts:242–243` calls `this.boundary!.prepareDesiredInstall(output)`.
   - `packages/iyon-tui/src/transport/structural/retained-dag.ts:1711–1713` implements `prepareDesiredInstall`.
   - `packages/iyon-tui/src/transport/structural/retained-dag.ts:2022–2035` implements `renderExact`, but this is a separate boundary method.

2. Legacy/specialized ABI functions remain generated and at least one old helper remains implemented:

   - `native-view-abi.ts:443–507` implements `tryRetainedEditTransactionRender`.
   - Generated TS functions include `editTxnBegin`, `editTxnAddTextLayout`, `editTxnCommitRender`, and `editTxnAbort`.
   - Generated native exports include path and edit-transaction symbols, gated by `direct-ffi` in the generated Rust output.

Therefore the reconciled claim is:

```text
Current production structural publication:
  retained semantic materialization → native N-API ABI → Rust View/Scene

Additional retained ABI machinery:
  path/edit-transaction helpers remain generated and test/qualification reachable

No evidence:
  that those helpers form a second complete production structural architecture
```

This resolves the apparent conflict between:

- assignment 41’s “single retained production architecture” statement;
- assignment 43’s observation that old path/edit-transaction APIs remain in the generated ABI and are used by a test helper;
- assignment 24’s generated-function inventory.

“Single production route” is defensible only when qualified as the current ordinary production structural publication route. It must not be expanded to mean “no old symbols, no test helper, and no alternate ABI entry points exist.”

### State-kind mapping: unresolved semantic contract

The preceding TypeScript state report identified a possible Diff-kind mismatch. Source inspection confirms the relevant layers:

- `packages/iyon-tui/src/api/view/semantic-node.ts:165–179` defines semantic kinds:
  - text `0`
  - diff `1`
  - spacer `2`
  - row `3`
  - column `4`
  - grid `5`
  - hanging `6`
  - container `7`
  - clamp `8`
  - contentMax `9`
  - component `10`
  - decorated `11`
  - contentHost `12`
- `packages/iyon-tui/src/api/view/retained-state.ts:118–131` accepts nearly all semantic kinds for state resources.
- `crates/iyon-tui/src/application/view_state.rs:127–143` maps:
  - `0 → Text`
  - `1 → Column`
  - `2 → Spacer`
  - `3 → Row`
  - `4 → Column`
  - `5 → Grid`
  - `6 → Hanging`
  - `7 → Container`
  - `8 | 9 → ClampRows`
  - `10 → ComponentSlot`
  - `12 → ContentHost`
- `packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json:3–15` separately defines native transport codes beginning at 1, where `1` is text and `2` is diff.

This is not enough to call the mapping a confirmed defect because `semantic_state_node_kind()` consumes private semantic codes, not native transport codes, and deliberately collapses some semantic variants into shared capability classes. However:

- Diff is collapsed to `StateNodeKind::Column`;
- ContentMax is collapsed to `ClampRows`;
- Decorated kind `11` is absent from the mapping;
- native transport codes must not be passed to this function;
- no single generated schema appears to define the state-capability mapping.

The proper finding is a **contract/documentation and test-coverage gap**: the source proves the mapping, but does not prove why those collapses are intended or that every caller supplies semantic rather than transport codes. The later deviation review should compare the mapping against state capability requirements and tests before labeling it erroneous.

### Content annotation style validation: source-confirmed candidate issue

Assignment 23 recorded this as an uncertainty because the displayed source might have escaped formatting. A direct source check now finds:

```ts
// packages/iyon-tui/src/transport/content/ffi.ts:501–503
function validateStyleName(value: string, label: string): void {
  if (typeof value !== "string" || value.length === 0 || /\\s|\\0/u.test(value)) {
    throw contentError(...);
  }
}
```

The source contains two backslashes in each branch of the regex literal. In a JavaScript/TypeScript regex literal:

- `/\s/u` matches whitespace;
- `/\0/u` matches NUL;
- `/\\s|\\0/u` matches a literal backslash followed by `s` or a literal backslash followed by `0`.

Thus the implementation does not enforce the error message’s stated “no whitespace or NUL” rule for style names. The same function is used for style role and semantic theme-key validation (`ffi.ts:422`, `ffi.ts:490`).

This is a concrete static candidate issue. No test was executed, so the report does not claim runtime impact was observed. It should be grouped separately from the broader content ABI and annotation-validation findings.

Other validation layers do not automatically eliminate the issue:

- `packages/iyon-tui/src/api/content/text.ts:124–126` has a separate correct whitespace regex for text names;
- `ffi.ts:373–374` separately checks NUL in tag namespace/name;
- the style-name path itself still contains the incorrect literal.

### Generated ABI metadata completeness

Assignment 24 identified that `buffer_used_of` is modeled and consumed by the generator but omitted from the generated JSON manifest. Source reconciliation confirms:

- canonical schema:
  - `tools/tui-abi/view_abi.toml:1537–1540`
  - `:1599–1602`
  - `:1616–1619`
  - additional buffer pairs at `:1883–1903` and `:3383–3403`;
- generated manifest entries include `"lowering": "buffer_used"` and the argument names but no `"buffer_used_of"` field, for example:
  - `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json:1892–1896`
  - `:1946–1968`
  - `:2240–2261`
  - `:4096–4117`.

The omission is not necessarily a runtime ABI failure because generated Rust/TS bodies and canonical schema retain the pairing information. It is, however, a real generated-manifest completeness gap for any external tooling that needs to reconstruct which buffer a count belongs to.

### Public-consumer reachability

The in-tree consumer fixture is evidence of a substantial package-root path, not of all public exports. Assignment 19 explicitly states that no external product/plugin package exercises every exported content, projection, annotation, theme, or extension type. Assignment 25 likewise identifies the consumer fixture as workspace-source dependent rather than packed-package validation.

Therefore the following remain unproven:

- all package-root exports are used externally;
- every extension trait has a runtime consumer;
- the package behaves identically when packed/published;
- an Iyon application or plugin exists in this repository.

## 3. Dependency and ownership map

### Reconciled three-plane diagram

```text
TypeScript public API/composition
        │
        ▼
TypeScript semantic retained DAG
        │
        ├── structural transport / generated N-API calls
        │         │
        │         ▼
        │   native addon ABI session
        │         │
        │         ▼
        │   Rust View / Scene / retained structure
        │
        ├── state transport / generated envelope
        │         │
        │         ▼
        │   native state wrapper
        │         │
        │         ▼
        │   Rust ViewStateRegistry / candidate state overlay
        │
        └── content transport
                  ├── direct content C ABI for Source mutation
                  └── retained ContentPort attachment through structural View
                            │
                            ▼
                    Rust Source / Funnel / Connector / ContentHost
                            │
                            ▼
                    projection / semantic text / measurement / paint
                            │
                            ▼
                    Rust Scene / physical Surface
                            │
                            ▼
                    terminal backend / native History frontier
```

### Ownership findings

The reports agree on the following ownership boundaries:

- TypeScript owns semantic authoring/composition and knowledge of desired retained publications.
- The native addon owns N-API object lifetime and bridges into Rust.
- Rust host/runtime owns accepted retained structures, state registries, content sources, scene preparation, physical presentation, and backend synchronization.
- Source content mutation uses a distinct direct C ABI lane; it is not a structural fallback.
- History semantic units and native physical frontier are distinct ownership domains.
- Content provider products are prepared and consumed by presentation; presentation does not own Source lifecycle.
- Generated ABI code is derived output; handwritten schema, content headers, and native implementations have separate ownership.

### Missing/weak reverse edges

The following reverse edges remain insufficiently proven:

1. **Exact-root production reachability**
   - `renderExactRoot()` and `RetainedRootBoundary.renderExact()` exist.
   - Source search found no production caller outside the boundary method.
   - Public runtime path uses deferred `prepareDesiredInstall`.
   - This is documented in assignments 30 and 41 and confirmed by current-source search.

2. **Generated ABI consumer completeness**
   - Generator output and generated manifest contain all function records.
   - Current package production uses only a subset of path/edit transaction functions.
   - No complete consumer matrix proves which generated functions are production, test-only, qualification-only, or intentionally retained compatibility surface.

3. **Provider-owned historical descriptors**
   - Assignment 44 identifies a potential wording mismatch between the resolved PERF-13 requirement that History capture an immutable committed projection descriptor and current code in which ContentHost/provider products hold Source-revision-keyed products.
   - The current implementation may satisfy the behavior through provider-owned immutable products.
   - Ownership language remains under-specified; no source contradiction is proven.

4. **Native artifact identity**
   - `packages/iyon-tui/src/transport/native/artifact.ts:55–59` allows `ION_TUI_NATIVE_ARTIFACT` to override the package-local artifact.
   - `stage-native.ts:91–124` validates staged symbol surfaces, but the ordinary package test command does not necessarily stage or revalidate the artifact.
   - Assignment 43 correctly treats this as a test/environment coverage gap.

5. **Cross-platform native behavior**
   - Windows staging and terminal behavior were not executed.
   - The reports identify artifact naming and platform matrix concerns but do not establish a baseline failure.

### Dependency violations or questionable couplings

The reports identify these as current coupling seams rather than proven defects:

- native `tui.rs` combines host lifecycle, controls, content wrappers, events, theme DTOs, and structural bindings;
- Rust View remains the final realization route even though TypeScript retained semantic materialization is the current authoring path;
- state invalidation traverses current retained layout/occurrence representations;
- History uses ContentHost/provider products and native physical frontier semantics;
- generated ABI declarations, handwritten TypeScript normalizers, native decoders, and Rust implementations each validate overlapping representations;
- Termwiz remains embedded in width/physical metric policy;
- paint helpers trust whole-glyph invariants that some lower-level cell writes do not independently enforce.

No preceding report proved that content reaches back into Source lifecycle from presentation code; assignment 08/14 and `presentation/content.rs` instead describe a one-way prepared-product seam.

## 4. Execution paths and state transitions

### Canonical structural lifecycle

The reconciled current path is:

```text
TS caller creates View/semantic node
    ↓
composition evaluates semantic retained tree
    ↓
retained DAG materializes semantic nodes and attachments
    ↓
generated N-API structural calls / NativeViewAbiSession
    ↓
native addon constructs Rust View values and retained references
    ↓
Rust host accepts desired root / candidate state/content bindings
    ↓
Scene preparation resolves state, content, layout and paint
    ↓
backend presents frame / native History transfer
    ↓
receipt or failure commits/discards visible state
```

The critical distinction is between:

- **desired structural publication**, which may be accepted before a frame is visible;
- **candidate frame preparation**, which can fail while preserving old visible output;
- **terminal/backend presentation**, which may leave physical synchronization unknown;
- **native History transfer**, which may partially advance irreversible physical state.

Assignment 41’s route table and assignment 29’s three-plane map agree on this separation.

### Exact-root route

Current source confirms:

- exact-root helper exists in `retained-dag.ts:1391–1400`;
- boundary method exists at `retained-dag.ts:2022–2037`;
- normal runtime route at `runtime.ts:242–243` calls deferred desired installation;
- no external production call to `renderExact()` was found.

This is not proof that the exact-root method is dead code; it may be reserved for future/qualification use. It is proof that no current production caller was established. The parent should preserve this as a reachability gap rather than infer intended use from the method’s name or comments.

### State lifecycle

Source and reports establish a layered lifecycle:

```text
TS ViewState object
    ↓
state envelope normalization/encoding
    ↓
native state wrapper
    ↓
Rust ViewStateRegistry record
    ↓
desired binding
    ↓
candidate overlay
    ↓
visible binding after frame acceptance
    ↓
in-flight receipt pinning
    ↓
retirement/disposal
```

Important coverage limits:

- state candidate and visible snapshots are separate;
- state resource disposal rejects bound/in-flight records;
- tests cover many registry invariants statically;
- no current assignment executed the full TS/native/Rust route on the baseline;
- the semantic-kind mapping remains a contract gap as described in §2.

### Content lifecycle

The content path is:

```text
TS Source API append/replace
    ↓
direct content C ABI (Bun FFI)
    ↓
Rust Source mutation and revision
    ↓
Funnel/Connector desired/visible selection
    ↓
projection and smoothing
    ↓
prepared measurement/row product
    ↓
ContentHost presentation / History transfer
    ↓
layout, paint, frame scheduling
```

The direct content ABI is a separate data lane. It is not a fallback from failed structural publication. Assignment 42 source-checked that separation; `ffi.ts:250–275` opens and validates the content library, while structural code uses the generated N-API session.

### History lifecycle

The current History model includes:

- semantic units and live/finalized state;
- ContentHost-backed units;
- projection and height products;
- native frontier and transfer status;
- native synchronization-unknown behavior;
- physical remainder retention after partial acknowledgement.

Assignment 44 reconciles older chronology:

- old `pushStream`/`sealStream` and HostTextStream routes are superseded;
- current generic `HistoryUnit` survives;
- current `stream/` contains coordinate types only;
- projection/projector algorithms remain under `projection/`, not under the old stream module.

This resolves an apparent contradiction in PERF-13-H wording: “old History stream units deleted” cannot be taken to mean all current `HistoryUnit` types were deleted.

### Failure and rollback transitions

The reports consistently distinguish:

- validation failure before mutation;
- composition abort preserving previously accepted publication;
- desired root accepted but frame preparation failing;
- backend presentation failing after logical candidate preparation;
- native sink failure with physical synchronization unknown;
- Content projection failure retaining old visible connector where possible;
- missing prepared projection ticket silently avoiding a paint operation.

No report proves an all-plane transaction that atomically rolls back semantic root, ContentHost bindings, state bindings, and physical presentation. Assignment 37 explicitly lists this as an open gap.

## 5. Alternate routes and failure semantics

### Semantic operation → implementation route reconciliation

| Semantic operation | Current primary route | Alternate/compatibility route | Reconciled status |
|---|---|---|---|
| Ordinary `Tui.render` | deferred retained DAG → `prepareDesiredInstall` | exact-root boundary method | Exact-root production reachability unproven |
| Direct scene render | direct runtime boundary and host render | test/headless boundary variants | Benchmark path does not exactly match production deferred mode |
| Structural View creation | generated N-API ABI session | generated path/edit functions | No second complete production architecture proven |
| Source append/replace | direct content C ABI | no N-API fallback found | Separate content lane by design |
| State mutation | generated state envelope → native state wrapper → Rust registry | direct internal Rust tests | Public route not executed in this investigation |
| Content projection failure | old visible connector/product may remain visible | default measurement/empty provider branches | Missing-product branches need explicit observability |
| History native transfer | NativeFrontier → sink → accepted prefix/remainder | recovery frame when synchronization unknown | No rewind after physical uncertainty |
| Rust benchmark render | in-process Rust factory/layout/paint | not TS/native end-to-end | Must not be called production TUI benchmark without qualification |
| Generated ABI test | test-local marker/no-op implementations | production native implementations in package tests | Signature/linkage coverage, not production implementation coverage |
| Consumer fixture | package-root public import | internal test helpers elsewhere | Public API evidence only for exercised fixture path |

### Candidate silent or weakly visible fallback areas

The following are repeatedly identified and source-supported:

1. `paint_window_direct` returns early for a missing/mismatched projection ticket without visibly marking physical incompleteness (`subsystems/rust/14-paint-physical.md:1267–1269`; assignment 41 also records weak observability).
2. Content measurement can fall back to default measurement when no prior visible projection exists (`audits/41-production-routes.md:1184–1195`).
3. Benchmark scripts can run in timing-only mode when counter mode is not required (`audits/42-benchmark-integrity.md:761–764`).
4. Test snapshots can pass while route selection or allocation behavior remains unproven (`subsystems/support/25-tests-fixtures.md:1504–1508`).
5. `Tui` package tests may load an artifact selected through environment override rather than the artifact just staged (`audits/43-test-contracts.md:1171–1174`).
6. Native error classification has a manually maintained content-code whitelist; a new core diagnostic could collapse to `ION_INTERNAL` if the whitelist is not updated (`subsystems/native/18-content-host-events.md:1496–1497`).

These are candidate observability and contract issues, not all confirmed runtime bugs.

### Absence claims that are source-scoped only

The preceding reports correctly limit several absence claims:

- no current product/plugin implementation in the repository;
- no production callers found for exact-root route;
- no old `pushStream`, `sealStream`, HostTextStream, or NativeTextStream route in current source;
- no active C consumer of the generated view header;
- no complete package-level graph artifact;
- no History user-scroll state;
- no full reference application under `examples/`;
- no direct production route from TypeScript extension traits into runtime composition.

These absence claims do not establish non-use by external consumers, historical branches, generated artifacts outside the inspected source, or future code.

## 6. Caches, invalidation, scheduling and performance

### Cache ownership and invalidation findings

Across the reports, cache state is split by plane:

- TypeScript semantic publication and retained identity caches;
- native resource/lease and ABI-runtime caches;
- Rust retained state candidate/visible/in-flight caches;
- layout and measurement caches;
- content Source revision, Connector selection, projection, and smoothing products;
- paint cache keyed by layout/content/theme-related revisions;
- physical presenter desired/visible shadow;
- History height/native frontier/remainder state.

The reports agree that these are not one cache. In particular:

```text
semantic identity retention
    ≠
native accepted-resource knowledge
    ≠
Rust retained state
    ≠
prepared content projection
    ≠
physical terminal state
```

### Performance evidence limitations

No current benchmark result proves the full production route because:

- Rust `tui_perf` is an in-process Rust semantic/layout/paint/History benchmark;
- T15 primarily exercises retained structural paths but does not measure every production `Tui.render` mode;
- content benchmark uses direct content FFI and reports Source/wake statistics, not full Rust projection/paint counters;
- optional counters permit timing-only runs;
- History TypeScript boundary route costs are not directly benchmarked;
- ViewSlot/ScrollPane replacement routes are test-covered but not committed benchmark cases;
- exact-root route counters are not sufficient to establish production reachability;
- historical artifacts carry different source revisions, feature sets, workload profiles, and platforms.

These limitations are documented by assignments 26 and 42 and should be preserved in any parent synthesis.

### Concrete cache/invalidations gaps requiring later grouping

1. `TextGeometryCache` is reported as keyed by `LayoutNodeId` while its product lifetime and width/tree domain are carried by surrounding preparation context rather than encoded in the key (`subsystems/rust/14-paint-physical.md:1282–1284`).
2. `has_physical_rows` may remain true after native units retire, potentially disabling a geometry shortcut indefinitely (`traces/37-history-scrollback.md:1662`).
3. Missing projection-ticket behavior has no explicit route counter or error (`audits/41-production-routes.md:1232–1234`).
4. Rust `NapiView*` counters appear declared but not used by current production paths (`audits/42-benchmark-integrity.md:763`).
5. The source manifest and scout reports identify counters, but no executed counter output at this baseline was obtained.

## 7. Tests, benchmarks and observability

### Test inventory status

The repository has extensive test source:

- Rust unit/integration tests across application, component, content, controls, interaction, layout, paint, projection, scene, state, terminal, and History;
- native generated ABI and sync tests;
- TypeScript public API, composition, transport, runtime, content, state, structural, History, benchmark, and differential tests;
- a workspace consumer fixture;
- generated ABI layout/conformance tests;
- property/fuzz-style tests;
- checked-in benchmark/replay artifacts.

Assignment 43 provides the strongest cross-repository test reconciliation and correctly warns that a generated wrapper test is not production native implementation coverage. Assignment 25 identifies test harness limitations and differential-oracle coupling.

### Tests that do not prove what their names might imply

- Generated Rust ABI tests use test-local marker/no-op implementations and prove signature/linkage shape, not production native implementation behavior (`subsystems/support/25-tests-fixtures.md:1440–1444`; `audits/43-test-contracts.md:980 onward).
- Differential tests comparing incremental retained output with a fresh retained output can share the same defect.
- Snapshot tests prove output equality, not route choice, native identity, allocation, or fallback absence.
- Rust tests can prove internal registry or layout semantics without proving staged N-API/native package behavior.
- Consumer fixture acceptance proves a public path but not a packed artifact or every public export.
- Native input validation tests may directly invoke raw native resources rather than proving public validation and native validation agree (`audits/43-test-contracts.md:978`).
- CI stages native artifacts before selected test workflows, but root `bun test` itself does not necessarily perform the staging and identity check.

### Observable gaps

The following remain unsupported by executed evidence:

- current baseline test pass status;
- current native artifact loadability;
- route selection counters for all production operations;
- exact-root production reachability;
- direct-FFI versus N-API selection at runtime for every lane;
- complete per-operation fallback ledger;
- real terminal resize/rewrap behavior;
- Windows staging and terminal behavior;
- cross-platform C ABI pointer/layout behavior;
- native scrollback readback on real Termwiz backends;
- all lifecycle combinations involving host, History, Port, Connector, control handles, and finalizers;
- full all-plane rollback after late frame/presentation errors.

### Benchmark route caveat

The phrase “benchmark passed” is insufficient without at least:

- source SHA;
- artifact hash;
- feature profile;
- platform/architecture;
- workload;
- counter mode/status;
- route/candidate metadata;
- whether the run was historical or current.

Assignments 26, 42, and 44 agree that committed benchmark JSON/JSONL files are historical evidence and must not be silently promoted to current-baseline qualification.

## 8. Cross-boundary findings and contradictions

### 8.1 “Single route” versus retained ABI compatibility surface

There is no contradiction once terminology is narrowed:

- current production structural publication is retained semantic materialization through generated N-API;
- path/edit transaction functions remain generated and one helper remains implemented;
- generated direct-FFI symbols are feature-gated;
- no complete-object fallback route was found.

The parent should avoid both overstatements:

- incorrect: “all old ABI paths are gone”;
- incorrect: “the old ABI is a second complete production architecture”;
- supported: “one ordinary retained production structural route exists, while compatibility/qualification/test ABI functions remain.”

Evidence: `runtime.ts:242–243`; `retained-dag.ts:1391–1400`, `:1711–1713`, `:2022–2037`; `native-view-abi.ts:443–507`; generated ABI files.

### 8.2 Stream module deletion versus projection machinery survival

Assignment 44 notes that current `crates/iyon-tui/src/stream/` contains only `coord.rs` and `mod.rs`, while current projection traits and smoothing remain. Source confirms:

- `lib.rs:40` exposes `projection` as `pub(crate)`;
- `lib.rs:46` keeps `stream` private;
- `stream/mod.rs:1–4` describes only source-rooted coordinate values;
- `projection/projector.rs` still defines `Projector`;
- `projection/smooth.rs` implements `Smooth`;
- content text projectors remain under `content/text`.

Thus “old stream implementation deleted” and “generic projection machinery survives” are compatible. It would be inaccurate to conclude that all stream/projection algorithms disappeared merely because the old stream directory was reduced.

### 8.3 History immutable descriptor ownership

Assignment 44 identifies a wording gap:

- historical handoff language can be read as requiring History itself to capture immutable committed projection descriptors;
- current source stores ContentPort-bearing History views/units;
- provider/Connector products carry Source-revision-keyed immutable products and finalized prefixes.

Source behavior may satisfy the intended immutability/lifetime property without History owning the product directly. The unresolved question is ownership wording, not a proven current behavior failure.

### 8.4 State semantic versus native transport codes

The state-kind mapping is a real cross-boundary seam:

```text
TypeScript semantic kind codes: 0..12
TypeScript native transport codes: 1..13
Rust application/view_state.rs: semantic_state_node_kind()
Rust native/view_state.rs: forwards u32 unchanged
```

The current source keeps the layers separate, but there is no generated shared state-capability schema equivalent to `view-kind-codes.json`. This makes accidental transport-code/semantic-code confusion possible. Existing tests should be source-checked for explicit semantic code inputs, and any future change should preserve that distinction.

### 8.5 Content regex validation

The source-confirmed `/\\s|\\0/u` issue is not mentioned as a resolved issue by the other reports. Assignment 23 left it uncertain; assignment 41’s route report treats content validation broadly but does not flag this exact line. This is a newly reconciled candidate issue and should be retained for grouped deviation evaluation.

### 8.6 Generated manifest/schema disagreement

Assignment 24’s `buffer_used_of` finding is source-confirmed. It is not a generated ABI runtime contradiction because:

- schema and generator model retain the pairing;
- generated function implementations can still be correct;
- manifest consumers lose metadata.

It is a generated-output/documentation completeness issue and should not be reported as an ABI wire incompatibility without executed evidence.

### 8.7 Generic framework boundary

Across all reports, no current generic framework source was shown to encode Iyon-specific agent/application semantics. The fixture uses generic concepts such as consumer, item, status, header, and footer, but no product/plugin implementation is present. This is an important negative finding: the repository maps terminal mechanics and retained UI framework responsibilities, not a complete Iyon application.

### 8.8 Physical whole-glyph invariant

Assignment 14 identifies a documentation/source tension:

- `physical/glyph.rs` describes a whole-glyph safety invariant;
- lower-level clear, border, and cell-copy helpers do not independently enforce whole-glyph boundaries;
- current layout/caller invariants may make canonical paths safe;
- no source proof covers every direct helper combination.

This remains a candidate physical-safety coverage issue, not a confirmed rendering defect.

## 9. Open questions and coverage gaps

### Repository and artifact gaps

1. Does the tracked-source manifest intentionally exclude `reports/pre-v5-l1/`, or should the historical report set have a separate manifest/provenance record?
2. Are all generated outputs outside the ABI generator’s `GENERATOR_OUTPUTS` list intentionally excluded?
3. Which ignored native artifact, if any, corresponds to the baseline source revision?
4. Does package test execution always load the artifact staged by the current workflow?
5. Are environment overrides such as `ION_TUI_NATIVE_ARTIFACT` allowed in CI qualification?
6. Is there any external consumer or product plugin not represented in this repository?

### Route and ABI gaps

7. Is `RetainedRootBoundary.renderExact()` intentionally reserved, or is its lack of production callers route drift?
8. Are path/edit-transaction ABI functions supported compatibility contracts, test-only APIs, or migration residue?
9. Is the generated function manifest expected to expose `buffer_used_of`?
10. Is direct FFI content transport intentionally the only direct lane, with no N-API retry?
11. Does any runtime counter prove generated safe-N-API selection for the structural route?
12. Does any runtime counter prove absence of direct structural FFI calls?
13. Does `RootPublication.route` have an active consumer or assertion?
14. Does the default content benchmark measure only wake/Source work, or is projection/paint measurement expected?

### State/content/layout gaps

15. Is `semantic_state_node_kind(1) → Column` an intentional capability collapse for Diff?
16. Is `semantic_state_node_kind(9) → ClampRows` intentional for ContentMax?
17. Why is semantic kind 11 (Decorated) absent from `semantic_state_node_kind()`?
18. Can a transport code ever reach Rust state validation accidentally?
19. Does the literal style-name regex enforce the intended whitespace/NUL contract? Static source says no; runtime test confirmation remains outstanding.
20. Are style-name validation and API-level text-name validation intentionally separate?
21. Is `TextGeometryCache` lifetime always bounded by the prepared layout product?
22. Does missing projection-ticket handling need an explicit error/counter?
23. Are default measurements on missing/poisoned content records acceptable?

### History/backend gaps

24. What precisely did PERF-13-H mean by “old History stream units”?
25. Who owns the immutable historical projection descriptor required by the resolved PERF-13 contract?
26. Does native scrollback recover/replay after terminal resize?
27. Does `has_physical_rows` intentionally remain true after all native units retire?
28. Is History user scrolling intentionally delegated entirely to terminal-native scrollback?
29. Is the current synchronization-unknown recovery path complete for real terminal sinks?
30. Can all semantic/state/content changes be rolled back after late frame or presentation failure?

### Validation gaps

31. What is the actual current baseline test/build/typecheck result?
32. What is the actual current baseline native staging result?
33. What is the actual current baseline benchmark route/counter result?
34. What happens under Windows staging and terminal execution?
35. What happens under real Termwiz resize/rewrap conditions?
36. What happens when host wrappers, History, ContentPort, Connector, and controls outlive the host in all combinations?
37. Do raw C callers outside TypeScript satisfy pointer/length contracts?
38. Are native content error whitelists synchronized automatically with core diagnostics?

No item above is a V5 disposition decision. These are evidence and contract questions for later grouped evaluation.

## 10. Evidence appendix

### 10.1 Required context and contract files

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`
- `docs/architecture/atlas-4355c02/evidence/report-provenance.json`
- `docs/architecture/atlas-4355c02/evidence/parent-reading-ledger.md`
- `AGENTS.md`

### 10.2 Complete preceding report artifact manifest

#### Rust subsystem reports

- `docs/architecture/atlas-4355c02/subsystems/rust/01-application.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/02-components.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/03-scene.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/04-view-presentation.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/05-layout-geometry.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/06-retained-state.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/07-history.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/08-semantic-content.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/09-projection-smoothing.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/10-stream-l1.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/11-theme.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/12-controls-scroll.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/13-interaction-output.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/14-paint-physical.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/15-terminal-backend.md`
- `docs/architecture/atlas-4355c02/subsystems/rust/16-public-binding.md`

#### Native reports

- `docs/architecture/atlas-4355c02/subsystems/native/17-structure-state.md`
- `docs/architecture/atlas-4355c02/subsystems/native/18-content-host-events.md`

#### TypeScript reports

- `docs/architecture/atlas-4355c02/subsystems/typescript/19-public-api.md`
- `docs/architecture/atlas-4355c02/subsystems/typescript/20-composition.md`
- `docs/architecture/atlas-4355c02/subsystems/typescript/21-runtime.md`
- `docs/architecture/atlas-4355c02/subsystems/typescript/22-structure-state-transport.md`
- `docs/architecture/atlas-4355c02/subsystems/typescript/23-content-native-transport.md`

#### Support reports

- `docs/architecture/atlas-4355c02/subsystems/support/24-codegen.md`
- `docs/architecture/atlas-4355c02/subsystems/support/25-tests-fixtures.md`
- `docs/architecture/atlas-4355c02/subsystems/support/26-bench-build-examples.md`

#### Wiring reports

- `docs/architecture/atlas-4355c02/wiring/27-rust-side.md`
- `docs/architecture/atlas-4355c02/wiring/28-typescript-side.md`
- `docs/architecture/atlas-4355c02/wiring/29-three-planes.md`

#### Trace reports

- `docs/architecture/atlas-4355c02/traces/30-composition-root.md`
- `docs/architecture/atlas-4355c02/traces/31-structural-mutation.md`
- `docs/architecture/atlas-4355c02/traces/32-state-mutation.md`
- `docs/architecture/atlas-4355c02/traces/33-themes-styles.md`
- `docs/architecture/atlas-4355c02/traces/34-runtime-scheduling.md`
- `docs/architecture/atlas-4355c02/traces/35-stream-content.md`
- `docs/architecture/atlas-4355c02/traces/36-layout-resize.md`
- `docs/architecture/atlas-4355c02/traces/37-history-scrollback.md`
- `docs/architecture/atlas-4355c02/traces/38-input-callbacks.md`
- `docs/architecture/atlas-4355c02/traces/39-slots-controls-animation.md`
- `docs/architecture/atlas-4355c02/traces/40-lifetime-caches.md`

#### Audit reports

- `docs/architecture/atlas-4355c02/audits/41-production-routes.md`
- `docs/architecture/atlas-4355c02/audits/42-benchmark-integrity.md`
- `docs/architecture/atlas-4355c02/audits/43-test-contracts.md`
- `docs/architecture/atlas-4355c02/audits/44-history-doc-drift.md`

### 10.3 Current source paths and symbols source-checked

#### Manifests and package ownership

- `Cargo.toml:1–7` — Rust workspace members.
- `crates/iyon-tui/Cargo.toml:6–23` — unpublished internal core crate and perf feature.
- `crates/iyon-tui-native/Cargo.toml:8–20` — native crate, `cdylib`, `direct-ffi`, native-host dependency.
- `tools/tui-abi-gen/Cargo.toml:8–30` — generator binary and dependencies.
- `package.json:7–18` — package exports and workspaces.
- `package.json:19–36` — build/test/check/staging scripts.
- `docs/architecture/atlas-4355c02/evidence/tracked-source-manifest.txt:1–1031` — tracked source inventory.

#### Structural route

- `packages/iyon-tui/src/runtime/runtime.ts:240–243` — deferred desired publication.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1391–1400` — `renderExactRoot`.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:1711–1713` — `prepareDesiredInstall`.
- `packages/iyon-tui/src/transport/structural/retained-dag.ts:2022–2037` — boundary `renderExact`.
- `packages/iyon-tui/src/transport/structural/native-view-abi.ts:443–507` — edit-transaction helper.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts:63–68` — generated path/edit declarations.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts:255–273` — generated edit-transaction wrappers.
- `crates/iyon-tui-native/src/generated/view_abi_exports.rs:3461–4183` — generated path/edit direct exports under `direct-ffi`.

#### Content validation and FFI

- `packages/iyon-tui/src/transport/content/ffi.ts:250–275` — direct content FFI session.
- `packages/iyon-tui/src/transport/content/ffi.ts:311–404` — annotation validation.
- `packages/iyon-tui/src/transport/content/ffi.ts:410–503` — semantic style encoding and `validateStyleName`.
- `packages/iyon-tui/src/api/content/text.ts:124–126` — separate correct text-name whitespace validation.
- `packages/iyon-tui/src/transport/content/ffi.ts:373–374` — separate tag NUL validation.

#### State-kind contract

- `packages/iyon-tui/src/api/view/semantic-node.ts:165–179` — semantic kind codes.
- `packages/iyon-tui/src/api/view/retained-state.ts:118–131` — accepted state-node semantic kinds.
- `crates/iyon-tui/src/application/view_state.rs:45–54` — state-kind validation entrypoint.
- `crates/iyon-tui/src/application/view_state.rs:127–143` — semantic-to-capability mapping.
- `crates/iyon-tui/src/retained_state/capabilities.rs:11–42` — capability enum and ViewKind mapping.
- `packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json:3–15` — separate native transport codes.
- `packages/iyon-tui/src/transport/structural/ir.ts:47–64` — native transport kind table.

#### Generated metadata

- `tools/tui-abi/view_abi.toml:1537–1540`, `:1599–1602`, `:1616–1619`, `:1883–1903`, `:3383–3403` — `buffer_used_of` schema fields.
- `packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json:1892–1968`, `:2240–2261`, `:4096–4117` — generated manifest entries lacking `buffer_used_of`.

#### Rust visibility and surviving projection machinery

- `crates/iyon-tui/src/lib.rs:13–49` — module visibility, private stream, crate-visible projection.
- `crates/iyon-tui/src/lib.rs:77–109` — internal reexports.
- `crates/iyon-tui/src/stream/mod.rs:1–4` — coordinate-only stream module.
- `crates/iyon-tui/src/projection/projector.rs:12–52` — surviving generic Projector machinery.
- `crates/iyon-tui/src/projection/smooth.rs:422–` — Smooth projector implementation.

### 10.4 Historical/preceding-report evidence used for reconciliation

- `audits/41-production-routes.md:1085–1226` — production route, exact-root, direct-FFI, hidden binding, content fallback, and frame-failure findings.
- `audits/41-production-routes.md:1226–1265` — route reachability and observability gaps.
- `audits/42-benchmark-integrity.md:663–747` — T15/current runner and direct content lane distinctions.
- `audits/42-benchmark-integrity.md:751–765` — benchmark coverage gaps and non-execution.
- `audits/43-test-contracts.md:1024–1124` — old test routes, generated tests, staging/artifact limitations.
- `audits/43-test-contracts.md:1128–1174` — explicit test/source-reachability gaps.
- `audits/44-history-doc-drift.md:1104–1120` — authority-aware chronology and contradiction register.
- `audits/44-history-doc-drift.md:1202–1226` — History/doc ownership and execution gaps.
- `subsystems/support/24-codegen.md:1282–1310` — generated manifest and native-header findings.
- `subsystems/support/24-codegen.md:1314–1348` — generator coverage gaps.
- `subsystems/support/25-tests-fixtures.md:1474–1509` — fixture/test coverage limits.
- `subsystems/support/26-bench-build-examples.md:1351–1384` — benchmark/build/reference-app gaps.
- `subsystems/typescript/23-content-native-transport.md:1636–` — content transport gaps including prior regex uncertainty.
- `subsystems/rust/14-paint-physical.md:1232–1288` — whole-glyph, projection-ticket, cache, and damage gaps.
- `traces/37-history-scrollback.md:1653–1666` — History resize, scrollback, recovery, and execution gaps.
- `wiring/29-three-planes.md:1527–1532` — cross-plane files not fully read by that assignment.
- `wiring/29-three-planes.md:1809–` — no executed validation.
- `subsystems/native/18-content-host-events.md:1487–1503` — native ABI/lifecycle/error whitelist gaps.

### 10.5 Files indexed but not independently validated

The preceding reports collectively identify these classes of files as indexed, sampled, or outside their primary ownership rather than comprehensively read:

- ignored native artifacts and Cargo target directories;
- large generated ABI bodies;
- checked-in benchmark JSON/JSONL records;
- replay-frame files under `examples/parrot_test`;
- unrelated Rust/TypeScript test fixtures;
- terminal escape-generation details beyond route selection;
- native platform-specific build outputs;
- external consumers;
- historical machine-readable L1 artifacts;
- broad cross-plane helpers outside each assignment’s primary ownership.

These limitations are not evidence of unused code. They are evidence that source inventory and route-focused reading were intentionally scoped.

### 10.6 Commands/search methods used

Read-only repository inspection used:

- file listing and glob enumeration of the atlas and repository;
- source-manifest inspection;
- targeted symbol/reference searches for route names, generated ABI functions, semantic/native kind codes, content validation, workspace members, and benchmark/staging surfaces;
- cross-report searches for open questions, indexed/not-read declarations, contradiction registers, and validation status.

No command was used to mutate files, install dependencies, run tests, build native artifacts, stage binaries, execute benchmarks, or start services.

### 10.7 Final evidence status

This report reconciles source and report evidence. It does not claim:

- that tests passed;
- that benchmarks passed;
- that generated files are fully synchronized beyond the inspected fields;
- that the native artifact is loadable;
- that exact-root is production-reachable;
- that all public APIs have external consumers;
- that all fallback paths are safe;
- that any candidate issue is already an accepted deviation;
- that any V5 deletion or migration decision has been made.