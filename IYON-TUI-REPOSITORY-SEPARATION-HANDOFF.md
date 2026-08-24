# Iyon TUI Repository Separation

## Make the repository boundary the framework boundary

**Status:** proposed architecture and migration handoff  
**Current repository:** `alexykn/iyon`  
**Recommended destination:** rename the current repository to `alexykn/iyon-tui`; extract the application/kernel side into a new `alexykn/iyon` repository  
**Transport sequence:** behavior-neutral repository separation first; safe N-API migration second  
**PERF relationship:** preserve PERF-12 T1–T13 and PERF-12 T13.1 R0–R10; keep T13.1 R6b blocked until the post-extraction transport is finalized

---

# 0. Executive decision

The current repository should become the standalone **Iyon TUI framework repository**.

Do not extract the largest and most historically significant subsystem into a new repository while leaving the smaller application side behind. The path of least resistance is the reverse:

```text
current alexykn/iyon repository
        |
        | preserve its full history and PERF provenance
        v
rename to alexykn/iyon-tui
        |
        +-- keep generic TUI Rust framework
        +-- keep TUI native binding
        +-- keep TypeScript TUI facade
        +-- keep ABI generator
        +-- keep TUI tests, benches, fixtures, and PERF records
        |
        `-- remove application/kernel/plugin files in an explicit extraction commit

new alexykn/iyon repository
        |
        +-- filtered history for application-owned paths
        +-- iyon-api
        +-- iyon-core
        +-- application native binding
        +-- non-TUI runtime
        +-- SDK, CLI, plugins, providers, agents, and tools
        `-- exact dependency on the public iyon-tui package
```

This is not primarily code organization. It is an **agent-safety mechanism**.

The present filesystem presents one plausible native world containing both application and framework concepts. AI agents therefore follow valid-looking but architecturally wrong symbol paths. The new repository boundary must make those paths physically unavailable.

The final integration rule is:

```text
Iyon application code
        |
        | public TypeScript package only
        v
@iyon/tui
        |
        | private generated N-API contract
        v
iyon-tui-native
        |
        v
iyon-tui Rust framework
```

The TUI repository must never depend on or expose concepts from `iyon-core`, `iyon-api`, providers, agents, tools, model turns, approvals, transcripts, or application session policy.

The application repository may depend on the TUI repository. The reverse dependency is forbidden.

---

# 1. Why the repository boundary is necessary

## 1.1 The current filesystem contradicts the intended architecture

Today an agent sees:

```text
crates/iyon-native/src/
├── core.rs
├── credentials.rs
├── events.rs
├── model_turn.rs
├── queue.rs
├── tool_execution.rs
├── tui.rs
├── tui/
│   ├── view_abi.rs
│   └── ...
└── generated/
```

The crate is one cdylib depending on all of:

```text
iyon-api
iyon-core
iyon-tui
napi
```

The TypeScript side reinforces the same collapse. `packages/iyon-runtime/src/native.ts` defines one `NativeAddon` interface containing, side by side:

```text
KernelSession
NativeModelTurnContract
NativeToolExecutionContract
credentials
NativeTuiHost
NativeHistory
NativeTextStream
NativeViewSlot
NativeScrollPane
NativeViewAbiBootstrap
```

An agent searching for a native TUI operation therefore lands next to tool execution and kernel-session APIs. An agent searching for tool output lands next to View ABI internals. Both wrong directions are locally plausible.

The intended ownership model is instead:

```text
GENERIC FRAMEWORK                         PRODUCT

@iyon/tui                                Iyon plugins/runtime
    |                                         |
iyon-tui-native                              iyon-core-native
    |                                         |
iyon-tui                                  iyon-core / iyon-api
```

A prose rule cannot reliably defeat a contradictory filesystem. The filesystem must encode the architecture.

## 1.2 The current code is still separable

The split is timely because the actual framework remains largely clean:

- `crates/iyon-tui` has no dependency on `iyon-core` or `iyon-api`.
- The TUI portion of `crates/iyon-native` does not import `iyon-core` or `iyon-api`.
- `packages/iyon-runtime/src/tui/**` is internally cohesive; its material dependencies outside the subtree are concentrated in `../native.ts` and native contract types.
- Tool, model-turn, approval, provider, and application policy remain outside the Rust TUI implementation.

The problem is the shared funnel, not an already-corrupted framework. Split while this remains true.

## 1.3 The repository is predominantly TUI work

A rough tracked-source inventory at the review revision shows:

```text
crates/iyon-tui                         ~62k lines
TUI portion of crates/iyon-native       ~21k lines
TypeScript TUI + TUI tests              ~23k lines
tui-abi + tui-abi-gen                   ~11k lines
TUI/PERF handoff documents              ~45k lines
TUI benchmark sources/raw artifacts     dominant share of runtime/bench

application Rust:
  iyon-core + iyon-api                   ~11k lines
  legacy crates/iyon                    ~10k lines
  application part of iyon-native        ~3k lines
```

The current repository also owns the complete PERF-7 through PERF-12/T13.1 provenance chain. Rewriting or filtering that history would invalidate thousands of recorded commit references and benchmark provenance lines.

Therefore:

> Preserve the current Git repository and its commit identities as `iyon-tui`. Extract the smaller application side into a new repository with a recorded history mapping.

## 1.4 Cognitive firewall

Humans can remember that a TUI framework must not know what a tool call means. Agents follow available files, imports, symbols, and examples.

A separate repository changes the available evidence:

```text
agent working in iyon-tui can see:
  View
  Scene
  History
  TextStream
  ScrollPane
  layout
  rendering
  component revisions

agent cannot see:
  ToolExecution
  ModelTurn
  CoreEvent
  ApprovalState
  Provider
  KernelSession
  application queues
```

This is a stronger invariant than an instruction file because the wrong implementation path is absent.

## 1.5 Why not extract TUI into a fresh repository?

That direction is technically possible but inferior for this repository:

```text
extract TUI from current repo
  -> move the dominant Rust/TypeScript/native/tooling surface
  -> relocate almost all PERF tests, benches, and records
  -> either rewrite historical SHAs or leave the new repo without its natural provenance
  -> move the component with most open architectural work (R6b/T14/T15)
  -> leave the smaller application side in the repository whose history is mostly TUI
```

Renaming the current repository and extracting the application still requires a clean split of the mixed native crate and TypeScript runtime, but those splits are required in either direction. Preserving the current repository as TUI avoids adding a much larger history/provenance migration on top.

One consequence is that old application files remain visible in historical commits of `iyon-tui`. This is acceptable: current-tree search, dependencies, packages, and agent instructions define the active architecture. Do not rewrite history merely to erase historical application paths; that would destroy benchmark provenance for a cosmetic gain.

---

# 2. Non-negotiable ownership rules

## 2.1 `iyon-tui` repository ownership

The TUI repository owns generic terminal framework behavior only:

```text
terminal sessions and backends
input, paste, focus, routing, and components
View / Scene semantic values
retained execution scopes, defineView, State<T>, and batching
immutable semantic DAG and NodeId identity
History and native scrollback transfer
TextStream / StreamPane / projections / smoothing
Markdown and plain-text projection
layout, measurement, placement, paint, Unicode width
styles, themes, selectors, borders, glyphs
Diff values and rendering
TextInput, ScrollPane, ViewSlot
native TUI N-API binding
generated View ABI
headless testing and performance probes
```

It must not contain or expose:

```text
agent / assistant semantics
model or provider behavior
prompts, responses, conversations, turns, transcripts
reasoning effort
application tool names, calls, arguments, approvals, registries
steering/follow-up queues
Iyon application activity/status choreography
application renderer defaults, labels, footer policy, or theme keys
backend event reduction or application state
Iyon-specific actions or reducers
application-specific fixed scenes or layouts
```

Generic names such as `session`, `model`, `queue`, `route`, and `activity` remain valid only when their semantics are genuinely framework-generic. A lexical substring ban is not sufficient; dependency and public-surface rules are authoritative.

## 2.2 `iyon` application repository ownership

The application repository owns:

```text
iyon-api protocol/schema surface
iyon-core kernel behavior
application native binding for core/api/credentials
runtime orchestration
providers, agents, tools, approvals
Iyon application state and event reduction
product scene composition and theme choices
spinner/footer/composer/tool-card policy
CLI, SDK, plugins, packaging
```

Application code consumes the TUI through the public TypeScript package. It must not import Rust TUI internals, generated native ABI modules, bridge sidecars, NodeIds, NativeRefs, or native component registries.

## 2.3 Dependency direction

Required direction:

```text
plugins ───────> iyon-core ───────> iyon-api
   |                 |
   +-----------------+
   |
   `──────────> @iyon/tui

@iyon/tui ─X─> iyon-core
@iyon/tui ─X─> iyon-api
@iyon/tui ─X─> plugins
```

The application may combine values from its core and the TUI in TypeScript orchestration. The TUI never combines them internally.

## 2.4 Public integration surface

The only supported application integration surface is the public TUI package:

```ts
import {
  History,
  Scene,
  ScrollPane,
  State,
  TextStream,
  Tui,
  View,
  ViewSlot,
  defineView,
  state,
} from "@iyon/tui";
```

The existing `iyon:tui` specifier may remain an Iyon application bundler alias:

```text
iyon:tui -> @iyon/tui
```

It must not be the only way third-party consumers access the framework. The standalone package must work without the Iyon application's virtual-module plugin.

---

# 3. Target repositories

## 3.1 Target `iyon-tui` repository

```text
iyon-tui/
├── AGENTS.md
├── ARCHITECTURE.md
├── Cargo.toml
├── Cargo.lock
├── package.json
├── bun.lock
├── crates/
│   ├── iyon-tui/
│   └── iyon-tui-native/
├── packages/
│   ├── iyon-tui/
│   └── tui-consumer-fixture/
├── tools/
│   ├── tui-abi/
│   ├── tui-abi-gen/
│   └── api-surface/              # TUI mapping only, if retained here
├── tests/
├── benches/
├── docs/
└── native-artifacts/
```

### `crates/iyon-tui`

Remains the generic Rust framework. It must compile and test without any application checkout.

### `crates/iyon-tui-native`

Owns only the generic N-API/native TUI bridge:

```text
NativeTuiHost
NativeHistory
NativeTextInput
NativeTextStream
NativeViewSlot
NativeScrollPane
View decoding/materialization
retained NativeRef runtime
generated View ABI
headless inspection hooks
TUI performance counters
```

It must depend on:

```text
iyon-tui
napi / napi-derive
transport-support dependencies required by the framework
```

It must not depend on `iyon-core` or `iyon-api`.

### `packages/iyon-tui`

This is the public TypeScript facade currently centered in `packages/iyon-runtime/src/tui/**`.

It owns:

```text
View, Scene, Tui
History, TextStream, ScrollPane, ViewSlot, TextInput
retained execution and tracked State<T>
styles, themes, projection, diff, Markdown
native addon loading for iyon-tui-native.node
public package exports and types
```

It must contain its own minimal native contract file. It must not import the application's `native.ts` or expose application addon contracts.

### Tests, benches, and records

The new repository retains:

```text
all TUI Rust tests
all packages/iyon-runtime tests whose subject is TUI
PERF-7 through PERF-12/T13.1 benchmark sources and raw artifacts
external TUI consumer fixture
TUI architecture and performance handoffs
layout/render/streaming documentation
```

## 3.2 Target `iyon` application repository

```text
iyon/
├── AGENTS.md
├── ARCHITECTURE.md
├── Cargo.toml
├── package.json
├── crates/
│   ├── iyon-api/
│   ├── iyon-core/
│   ├── iyon-core-native/
│   └── iyon/                     # retained only while the legacy Rust app exists
├── packages/
│   ├── iyon-runtime/             # TUI subtree removed
│   ├── iyon-sdk/
│   ├── iyon-plugins/
│   └── iyon-cli/
├── plugins/
│   ├── agents/
│   ├── app/
│   ├── providers/
│   └── tools/
└── docs/
```

### `crates/iyon-core-native`

Owns only application/kernel native bindings:

```text
KernelSession
ModelTurn
ToolExecution
application event queues
credentials
core/api serialization and async bridge
```

It must not depend on `iyon-tui`.

The legacy `crates/iyon` may depend on the external `iyon-tui` crate while it exists, but this does not authorize product policy inside `iyon-tui`.

## 3.3 Two native artifacts

Final artifacts:

```text
iyon-tui-native.node
  - generic TUI only
  - packaged with @iyon/tui

iyon-core-native.node
  - application/kernel only
  - packaged with @iyon/runtime or successor
```

Loading two different addons is valid. PERF-12's same-image invariant forbids loading two copies of the same TUI native runtime; it does not require the application kernel and TUI framework to share one binary.

No TUI handle crosses into the core addon. No core handle crosses into the TUI addon. TypeScript orchestration is the seam.

---

# 4. Current-path ownership map

| Current path | Destination | Action |
|---|---|---|
| `crates/iyon-tui/**` | `iyon-tui` | Keep in current repository |
| `crates/iyon-native/src/tui.rs` | `iyon-tui/crates/iyon-tui-native` | Move/rename |
| `crates/iyon-native/src/tui/**` | `iyon-tui/crates/iyon-tui-native` | Move/rename |
| `crates/iyon-native/src/generated/view_abi_*` | `iyon-tui/crates/iyon-tui-native` | Move |
| `crates/iyon-native/include/iyon_view_abi.h` | `iyon-tui/crates/iyon-tui-native` | Move while FFI exists |
| `crates/iyon-native/tests/generated_view_abi.rs` | `iyon-tui/crates/iyon-tui-native/tests` | Move |
| `tools/tui-abi/**` | `iyon-tui` | Keep |
| `tools/tui-abi-gen/**` | `iyon-tui` | Keep |
| `packages/iyon-runtime/src/tui/**` | `iyon-tui/packages/iyon-tui/src` | Move/rename package |
| TUI portions of `packages/iyon-runtime/src/native.ts` | `iyon-tui/packages/iyon-tui/src/native.ts` | Extract into a TUI-only contract |
| TUI tests/benches under `packages/iyon-runtime` | `iyon-tui` | Move |
| `packages/tui-consumer-fixture/**` | `iyon-tui` | Keep |
| PERF/TUI handoff documents and JSONL | `iyon-tui` | Keep with unchanged Git history |
| `crates/iyon-api/**` | new `iyon` | Extract with history |
| `crates/iyon-core/**` | new `iyon` | Extract with history |
| `crates/iyon/**` | new `iyon` | Extract with history |
| non-TUI `crates/iyon-native/src/**` | `iyon/crates/iyon-core-native` | Extract/rename |
| non-TUI `packages/iyon-runtime/**` | new `iyon` | Extract |
| `packages/iyon-sdk/**` | new `iyon` | Extract |
| `packages/iyon-plugins/**` | new `iyon` | Extract |
| `packages/iyon-cli/**` | new `iyon` | Extract |
| `plugins/**` | new `iyon` | Extract |
| application/core/API docs | new `iyon` | Extract |

Shared utility files must not be copied automatically. Each repository receives the smallest implementation its owned surface requires. In particular, do not recreate a shared `native` helper package that both repos depend on; that would reintroduce the bridge funnel under another name.

---

# 5. Migration registry

The separation is divided into independently reviewable tranches. Every tranche ends with its own commit, verification, and implementation record.

| Tranche | Scope | Required result before proceeding |
|---|---|---|
| **S0** | Freeze provenance, dependency graph, public API, tests, benchmarks, and repository-hosting metadata | Baseline artifacts committed; no open ownership ambiguity; current known failures recorded |
| **S1** | Machine-enforce framework/app ownership while still in one checkout | TUI paths cannot depend on core/api/plugins; public-surface guard green |
| **S2** | Create filtered application repository and perform remote rename/cutover | Both repositories exist with provenance mapping; no history rewrite in TUI repo |
| **S3** | Remove application Rust and app-native surface from `iyon-tui`; create `iyon-tui-native` | TUI Rust workspace builds standalone; TUI addon exports no application symbols |
| **S4** | Extract/rename TypeScript TUI package and native loader | `@iyon/tui` builds/tests standalone; external fixture imports public package only |
| **S5** | Convert `iyon` application repository to external consumer | App builds and tests against exact `@iyon/tui` and Rust dependency versions; no deep imports |
| **S6** | Move TUI native transport from unsafe Bun FFI to generated safe N-API | Differential correctness and structural gates green; performance delta measured honestly |
| **S7** | Delete obsolete unsafe/bootstrap machinery only after S6 adoption | No `bun:ffi`, pointer bootstrap, or unsafe generated export surface remains |
| **S8** | Resume deferred PERF work on final repository/transport | T13.1 R6b, PERF-12 T14/T15, then conditional T16 execute in order |

Order is mandatory. Do not combine repository extraction and transport replacement in one tranche.

---

# 6. Tranche S0 — freeze evidence and hosting state

Before moving files:

```bash
git status --short
git rev-parse HEAD
git log -1 --oneline
bun --version
bun --revision
rustc --version
cargo metadata --format-version 1
```

Record:

```text
current repository SHA
remote URL and repository name
open issues and pull requests by ownership
release/tag inventory
current package/crate graph
current NativeAddon export inventory
current public @iyon/runtime/tui surface
current known test failures
current native artifact hashes
PERF-12/T13.1 artifact provenance
```

Create a signed or annotated tag:

```text
pre-iyon-tui-repository-separation
```

Mandatory baseline checks:

```text
Rust fmt/clippy/test
TypeScript typecheck
runtime TUI tests
external consumer fixture
application plugin suite
T13.1 execution-frontier tests
PERF-12 retained identity/wide/payload tests
memory soak smoke profile
```

Known failures must be recorded exactly, not converted into a green summary.

### Hosting decision

Renaming the current repository preserves its issues, pull requests, releases, and Git commit identities in the TUI repository. Audit open issues before cutover because application-specific issues will otherwise remain attached to `iyon-tui` and may require manual recreation in the new application repository.

Do not rewrite/filter the current repository's history. PERF documents and raw artifacts contain commit SHAs whose continued meaning is required.

---

# 7. Tranche S1 — executable ownership boundaries

## 7.1 Rust dependency gate

Add a CI script based on `cargo metadata` that asserts:

```text
crates/iyon-tui dependency closure excludes iyon-core and iyon-api
TUI-native module dependency closure excludes iyon-core and iyon-api
application native modules do not import iyon-tui native ABI modules
```

After separation, the first two become structurally impossible because the crates do not exist in the TUI repository.

## 7.2 TypeScript import gate

Framework TypeScript may import only:

```text
its own package modules
its own native addon contract
standard/runtime libraries declared by @iyon/tui
```

Application code may import only the public `@iyon/tui` package entrypoints. Reject:

```text
@iyon/tui/src/*
relative paths into a checked-out TUI repository
generated/view_abi internals
retained_dag internals
NativeRef / NodeId sidecar modules
native addon implementation modules
```

Use package `exports`, TypeScript path checks, and a CI import-graph script. Do not rely on grep alone.

## 7.3 Public API guard

Maintain an API-surface snapshot for the TUI package and Rust crate. Reject application-specific exported names including, at minimum:

```text
Agent
Assistant
Provider
Prompt
ModelTurn
ToolCall
ToolExecution
Approval
Conversation
Transcript
KernelSession
Steering
ReasoningEffort
```

Generic framework uses of words such as `session`, `model`, and `queue` require semantic review rather than a blanket substring ban.

## 7.4 Agent-facing documents

The TUI repository root must contain concise, high-signal files.

### `ARCHITECTURE.md`

Lead with:

```text
This repository is a generic terminal UI framework.
It MUST NOT contain agent, model, provider, prompt, tool-call, approval,
conversation, transcript, or Iyon application policy.
If a feature requires those concepts, it belongs in the Iyon application repository.
```

Then document the public TypeScript facade, private N-API seam, Rust framework, retained execution, semantic DAG, and generic host responsibilities.

### `AGENTS.md`

Keep operational rules short enough to remain salient:

```text
- generic TUI only
- public API parity between Rust/native/TypeScript
- no application imports
- no product policy in themes/renderers/defaults
- run dependency and public-surface gates before completion
```

The application repository receives the inverse rule:

```text
Use public @iyon/tui APIs. Do not modify or import TUI internals for application behavior.
```

---

# 8. Tranche S2 — repository creation and remote cutover

## 8.1 Prepare the application repository first

Create a filtered clone under a temporary remote name, for example:

```text
alexykn/iyon-app-extract
```

Include application-owned paths from §4 plus required root build metadata. Use `git filter-repo` or an equivalent history-preserving tool.

Because filtering rewrites commit IDs, commit a provenance map:

```text
SOURCE-SHA-MAP.jsonl
  old_sha
  filtered_sha
  included_paths
  extraction_tool_version
  extraction_command
```

The TUI repository's original SHAs remain canonical for PERF evidence.

## 8.2 Remote rename sequence

Recommended sequence:

```text
1. Freeze/tag current repository.
2. Create and verify temporary filtered application repository.
3. Rename current remote alexykn/iyon -> alexykn/iyon-tui.
4. Create/rename filtered application remote -> alexykn/iyon.
5. Update local remotes, documentation links, CI secrets, badges, and package metadata.
6. Do not depend on GitHub's old-name redirect after the old name is reused.
```

## 8.3 Do not clean both repositories in one opaque operation

The cutover commit may leave each repository temporarily containing removable compatibility files, but every such file must have a named owner and removal tranche. No shared facade is allowed to become a permanent third architecture.

---

# 9. Tranche S3 — standalone Rust TUI repository

## 9.1 Create `iyon-tui-native`

Within the renamed TUI repository:

```text
move TUI-native modules out of crates/iyon-native
rename crate to iyon-tui-native
remove dependencies on iyon-core/iyon-api/keyring/application async bridge
retain only generic TUI dependencies
```

The initial S3 implementation may still contain the existing PERF-12 direct-FFI transport. S3 is behavior-neutral; safe N-API replacement belongs to S6.

## 9.2 Remove application modules from TUI repository HEAD

Delete from the TUI repository:

```text
core.rs
credentials.rs
model_turn.rs
queue.rs
tool_execution.rs
application events and value adapters
KernelSession exports
application probes
```

They already exist in the filtered application repository. Do not leave stubs or deprecated re-exports in the TUI repository.

## 9.3 Build independence gate

The TUI Rust workspace must pass from a checkout containing no application repository:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo tree -p iyon-tui-native
```

The dependency tree must contain no Iyon application crate.

## 9.4 Separate artifact identity

Rename the artifact and probes:

```text
libiyon_tui_native.*
iyon-tui-native.node
nativeVersion() -> framework-specific version
TUI load probe contains no application probe
```

No `iyon-native.node` compatibility artifact remains in the TUI repository after S5 consumer cutover.

---

# 10. Tranche S4 — standalone TypeScript TUI package

## 10.1 Package extraction

Move:

```text
packages/iyon-runtime/src/tui/**
```

into a dedicated package, for example:

```text
packages/iyon-tui/src/**
package name: @iyon/tui
```

Extract the TUI portions of `packages/iyon-runtime/src/native.ts` into a TUI-only native contract.

The current giant `NativeAddon` interface must disappear. The new TUI contract must not mention:

```text
KernelSession
ModelTurn
ToolExecution
credentials
application event queues
```

## 10.2 Virtual-module separation

The current `virtual-modules.ts` registers `iyon:api`, `iyon:core`, `iyon:tui`, and `iyon:plugins` together. Split ownership:

```text
@iyon/tui package:
  exports framework values directly
  does not know about api/core/plugins virtual modules

Iyon application bundler:
  owns iyon:api, iyon:core, iyon:plugins
  may map iyon:tui to @iyon/tui for source compatibility
```

Third-party TUI consumers must import `@iyon/tui` without installing the Iyon application plugin.

## 10.3 Native packaging

The package owns loading/staging of:

```text
iyon-tui-native.node
```

It must not know the path or type surface of `iyon-core-native.node`.

Initially preserve the current platform matrix:

```text
darwin-arm64
darwin-x64
linux-x64
linux-arm64
win32-x64
```

Prebuilt binary distribution can be a later packaging tranche. Repository separation does not require solving universal prebuild publication immediately.

## 10.4 External-consumer gate

Promote `packages/tui-consumer-fixture` into a true standalone-consumer fixture. It must:

```text
import only @iyon/tui
contain no Iyon application configuration
exercise defineView/state direct invalidation
exercise History/TextStream/ScrollPane/ViewSlot
load only iyon-tui-native.node
pass from a checkout without the application repository
```

## 10.5 Public name migration

Prefer one deliberate package rename over a long compatibility era:

```text
@iyon/runtime/tui -> @iyon/tui
```

The Iyon application may preserve `iyon:tui` as a local alias. Do not maintain two public package identities indefinitely.

---

# 11. Tranche S5 — application becomes an external consumer

## 11.1 Application dependency

Pin exact TUI revisions during the extraction period:

```text
Rust: exact git revision or exact published crate version
TypeScript: exact @iyon/tui version or exact git/package artifact
native binary: version/manifest tied to the same release
```

Do not use a floating branch dependency.

## 11.2 Application native split

In the application repository, rename the remaining native crate/artifact:

```text
crates/iyon-native -> crates/iyon-core-native
iyon-native.node   -> iyon-core-native.node
```

Its TypeScript contract contains only application/kernel exports.

It must not contain `NativeTuiHost`, View ABI bootstrap, History, TextStream, ViewSlot, or ScrollPane.

## 11.3 Consumer conversion

Replace application imports:

```text
@iyon/runtime/tui -> @iyon/tui
```

or route existing `iyon:tui` aliases directly to `@iyon/tui`.

No application file may import a generated TUI module or native addon internals.

## 11.4 Cross-repository compatibility test

The application CI matrix must test against the exact pinned TUI version. The TUI repository should additionally test one small public consumer fixture shaped like the Iyon application, but it must not import the application repository or encode application semantics.

## 11.5 Completion condition

At the end of S5:

```text
TUI repository builds/tests without application checkout
application repository builds/tests using released/pinned TUI package
TUI addon and core addon are separate
no shared NativeAddon interface exists
no application symbol appears in TUI public API
no TUI internals appear in application imports
```

Only then is the repository separation complete.

---

# 12. Tranche S6 — safe N-API transport after extraction

S6 executes entirely in the standalone TUI repository.

## 12.1 Preserve the architecture, replace only the lowering

Keep unchanged:

```text
eager immutable View -> BridgeViewNode DAG
53-bit NodeId identity
one environment-owned NodeId -> WeakView cache
BridgeNativeHint sidecars
NativeRef acceleration
identity cutoff before payload/child inspection
RetainedRootBoundary leases
MaterializeTx temporary leases
scope projections and T13.1 retained execution
tracked State<T> and dirty scheduler
PersistentSeq wide edits
text/scalar/Grid derivation hints
retained text/style/Diff payload behavior
History/TextStream specialization
ViewSlot/ScrollPane/component boundaries
cold Direct decoder as oracle/fallback
```

Replace:

```text
bun:ffi linkSymbols
raw runtime/host pointers exposed to JavaScript
same-image bootstrap pointer map
unsafe generated extern exports
buffer_length/cstring direct-FFI lowering
```

with generated N-API methods over opaque native objects and typed handles.

## 12.2 Learn from 7v2 Candidate A

The historical 7v2 N-API decoder did the important thing correctly:

```text
receive one root JS object
read NodeId first
WeakView cache hit -> return immediately
never read schema, kind, payload, or children on a hit
miss -> safe property-walk decode with schema and cycle checks
```

The new N-API lowering keeps this decoder as the complete cold/oracle route and exposes retained primitives safely for changed-frontier work.

## 12.3 Proposed native object model

JavaScript must not hold raw pointers. Prefer opaque N-API classes/objects:

```text
NativeTuiHost
NativeViewSession or environment-owned runtime handle
NativeHistory
NativeTextStream
NativeViewSlot
NativeScrollPane
```

Generated retained operations become methods or module functions taking typed values:

```text
materializeSpacer(nodeId, rows) -> NativeRef number
materializeAxis(nodeId, kind, gap, Uint32Array) -> NativeRef
patchTextLayout(nodeId, baseRef, wrap, align) -> NativeRef
axisSetChild(nodeId, baseRef, index, childRef, track) -> NativeRef
axisSplice(nodeId, baseRef, index, removeCount, insertedRefs) -> NativeRef
gridSetCell(...)
releaseMany(Uint32Array)
renderRef(NativeRef)
```

`NativeRef` remains private framework acceleration. It is not exported from the public package.

TypedArray storage may be borrowed only for the synchronous N-API call via the supported N-API typed-array API. Native code must copy/resolve semantic values before returning and never retain the pointer.

## 12.4 Generator ownership

Keep one canonical schema and generator. Add an N-API renderer rather than hand-maintaining signatures.

The generator continues to validate:

```text
NodeId halves/safe range
buffer capacity and used length
borrow duration = call
owner-thread access
reference roles
status/error mapping
full-schema materializer coverage
```

## 12.5 Temporary dual-backend qualification

During S6 only, the extracted TUI repository may build both:

```text
current direct-FFI backend
new N-API backend
```

This is a qualification mechanism, not a permanent product architecture.

Both backends consume the same semantic DAG, retained runtime, fixtures, counters, and host. A build switch selects the lowering. No application code or public API changes between arms.

## 12.6 N-API adoption gates

Correctness:

```text
full schema parity
same screen/headless output
same NodeId/NativeRef lifecycle semantics
same failure atomicity
same stale-ref one-retry behavior
same memory convergence
same stream isolation
same multi-host lifetime
```

Structural:

```text
exact root remains O(1)
stable subtree cutoff precedes payload reads
wide edits remain O(log_32 N)
semantic no-op performs zero native work
one batch/frame for multi-scope updates
no borrowed pointer survives a call
```

Performance:

```text
process-isolated FFI vs N-API smoke matrix
same build settings and fixtures
raw samples committed
per-call and end-to-end deltas reported separately
no gate reduced to justify safety preference
```

The existing ~1.7% FFI-over-7v2 smoke results are evidence for specific tiny-frontier cases, not a universal estimate. S6 must measure the actual retained N-API lowering rather than infer it.

If N-API regresses an important workload, do not silently accept or immediately return to unsafe FFI. First test whether call batching or a different N-API granularity retains safety while removing dispatch density. PERF-12 §124 deliberately keeps semantic architecture separate from physical lowering for this reason.

---

# 13. Tranche S7 — remove unsafe FFI after adoption

Only after S6 gates pass:

```text
delete bun:ffi bindings
delete NativeAbiPointers
delete runtime_ptr/host-pointer exposure
delete bootstrap function pointer table
delete unsafe generated extern exports
delete handwritten pointer-map maintenance
delete FFI-only conformance fixtures
remove Bun-FFI-only package/runtime requirements
```

Keep:

```text
canonical ABI/schema model where useful
N-API generated bindings
Direct N-API cold/oracle decoder
retained semantic runtime
all PERF-12 transport-independent improvements
```

Run a banned-surface audit:

```text
no linkSymbols or CFunction imports
no raw native pointer fields in TS contracts
no generated pub unsafe extern transport functions
no application exports in TUI addon
```

The Rust implementation may still contain isolated `unsafe` required by terminal/platform libraries or audited N-API internals. The goal is not a meaningless repository-wide zero-unsafe slogan; it is removal of the application-maintained raw FFI ABI and borrowed-pointer surface.

---

# 14. Tranche S8 — resume PERF work

Repository and transport finalization precede deferred optimization work.

Required sequence:

```text
1. T13.1 R6b
   incremental MountGraph/layout/paint frontier against final N-API commit boundary

2. PERF-12 T14
   randomized DAG differential tests, fuzzing, cross-transport/lifetime hardening

3. PERF-12 T15
   authoritative process-isolated comparison and adoption decision

4. PERF-12 T16
   conditional dead recipe/transport cleanup only after adoption
```

T15 must measure the T13.1-adopted execution system and final N-API transport. Do not publish a decision run over the pre-extraction mixed addon or temporary dual-backend state.

---

# 15. Versioning and release discipline

## 15.1 TUI versions

One release version must identify a coherent set:

```text
@iyon/tui package
Rust iyon-tui crate
iyon-tui-native addon
generated ABI/schema version
```

During extraction, pin exact revisions. After stabilization, use semantic versions and a compatibility manifest.

## 15.2 Compatibility manifest

Each package release should carry:

```json
{
  "package": "@iyon/tui",
  "version": "0.x.y",
  "nativeAddon": "iyon-tui-native",
  "nativeAbiVersion": 0,
  "semanticSchemaVersion": 0,
  "schemaHash": "...",
  "generatorHash": "...",
  "supportedTargets": []
}
```

N-API removes the pointer-table handshake but not the need to detect package/addon version skew.

## 15.3 Application upgrade flow

For a new generic primitive:

```text
1. Implement and test it in iyon-tui.
2. Release/pin a new TUI version.
3. Update the Iyon application dependency.
4. Use only the new public API in application TypeScript.
```

Do not land coordinated unpublished deep imports across repositories.

---

# 16. CI and agent-safety gates

## 16.1 TUI repository required gates

```text
standalone Rust fmt/clippy/test
standalone TypeScript typecheck/test
native addon build/load on supported targets
generated ABI freshness
public API snapshot
forbidden dependency graph
external consumer fixture
T13.1 execution-frontier tests
PERF-12 identity/wide/payload/failure tests
stream isolation test
memory soak profile
```

## 16.2 Application repository required gates

```text
exact TUI dependency version resolves
no TUI deep imports
no TUI native internals in application source
core-native addon has no TUI exports
plugins/app visual and behavior suites
standalone/CLI packaging loads both independent addons correctly
```

## 16.3 Cross-repository contract test

A small compatibility fixture should run on every TUI release candidate:

```text
install @iyon/tui into a clean temporary package
load iyon-tui-native.node
open headless Tui
render a Scene
perform direct State<T> scoped invalidation
append a TextStream
update ViewSlot and ScrollPane
close with zero leaked subscriptions/leases
```

The fixture must not clone or import the Iyon application repository.

## 16.4 Agent context reduction

Repository-local search results should reflect ownership:

```text
search ToolExecution in iyon-tui        -> no results
search NativeTuiHost in iyon            -> no implementation results
search retained_dag in iyon             -> no importable module
search Provider in iyon-tui public API  -> no results
```

Treat this as an acceptance property, not merely aesthetics.

---

# 17. Risks and mitigations

## 17.1 Current repository issues/PRs follow the rename

**Risk:** application-specific issue history remains attached to `iyon-tui`.

**Mitigation:** inventory before rename; recreate/migrate active application issues; add links to historical items. Preserve the current repository for TUI because its code and PERF history dominate.

## 17.2 Filtered application history rewrites SHAs

**Risk:** application commit references change.

**Mitigation:** commit `SOURCE-SHA-MAP.jsonl`; preserve release notes and cross-references. Do not rewrite TUI repository history.

## 17.3 Two native artifacts complicate packaging

**Risk:** staging and standalone builds must bundle two `.node` files.

**Mitigation:** each package owns one artifact and one load probe; the application build composes packages rather than one giant addon contract. Add an explicit standalone packaging test.

## 17.4 Cross-repository changes become coordinated releases

**Risk:** a new framework primitive needs a TUI release before app use.

**Mitigation:** this is intentional architectural friction. Pin exact versions and keep public contracts small. Do not defeat the boundary with local deep imports.

## 17.5 Temporary unsafe FFI remains after extraction

**Risk:** agents continue optimizing around it during S3–S5.

**Mitigation:** mark it private and transitional in `ARCHITECTURE.md`; prohibit public API changes around transport details; begin S6 immediately after consumer cutover.

## 17.6 N-API call overhead

**Risk:** many tiny materializer calls regress large changed frontiers.

**Mitigation:** process-isolated A/B; retain generator/backend boundary; batch through safe N-API if measured. Never discard retained DAG architecture because one physical lowering is suboptimal.

## 17.7 Legacy Rust application depends on iyon-tui

**Risk:** `crates/iyon` keeps a Rust-level dependency that confuses ownership.

**Mitigation:** it moves to the application repository and consumes a published/git-pinned `iyon-tui` crate. Product renderer policy remains application-owned. Retire the legacy Rust app separately if planned; do not mix that cleanup into repository extraction.

---

# 18. Banned shortcuts

Do not:

```text
leave one mixed native addon behind a renamed facade
create a permanent shared native-bindings utility repository
copy application types into the TUI repo for convenience
copy TUI internals into the application repo
expose NodeId/NativeRef/public raw handles to avoid package work
retain @iyon/runtime/tui and @iyon/tui as equal public APIs indefinitely
combine repository extraction with N-API behavior changes
rewrite the current TUI Git history
lose or regenerate authoritative PERF JSONL without provenance
run T13.1 R6b against the temporary transport
accept N-API regressions without raw measurements
return to whole-tree replay or O(width) edits to simplify the bridge
encode Iyon app policy behind generic-looking Rust names
```

---

# 19. Required implementation records

Each S0–S8 tranche appends a record to this document containing:

```text
1. Scope statement
2. Commits/repository SHAs
3. Review findings and corrections
4. Implementation summary
5. Provenance (both repositories where relevant)
6. Gate evidence with actual counts/numbers
7. Status: COMPLETE / PARTIAL / FAILED / BLOCKED
```

A tranche is not complete because files moved. It is complete only when its ownership and behavior gates pass.

For repository-cutover tranches, record:

```text
old remote/new remote names
pre-split tag
source SHA mapping artifact
package/crate versions
native artifact hashes
known issue/PR migration disposition
```

---

# 20. Final acceptance checklist

## Repository identity

```text
[ ] current Git repository renamed to iyon-tui without history rewrite
[ ] filtered application repository created as iyon
[ ] SHA mapping committed for filtered history
[ ] issues/PRs/releases audited
```

## TUI repository

```text
[ ] contains only generic TUI code and supporting tooling
[ ] crates/iyon-tui compiles standalone
[ ] crates/iyon-tui-native depends on no app crate
[ ] @iyon/tui builds and tests standalone
[ ] public package owns its native artifact
[ ] external consumer fixture requires no Iyon app setup
[ ] TUI/PERF history and artifacts remain valid
```

## Application repository

```text
[ ] owns iyon-api, iyon-core, core-native, runtime, SDK, CLI, and plugins
[ ] consumes exact public TUI versions
[ ] no deep/internal TUI imports
[ ] core-native exports no TUI handle
[ ] app behavior remains in TypeScript plugins/orchestration
```

## Native boundary

```text
[ ] two independent addon artifacts
[ ] no shared giant NativeAddon contract
[ ] N-API transport qualified after extraction
[ ] raw pointer/bootstrap Bun FFI removed after evidence
[ ] no borrowed pointer retained across calls
```

## PERF preservation

```text
[ ] eager immutable DAG preserved
[ ] exact-root and stable-subtree cutoffs preserved
[ ] wide edits remain logarithmic
[ ] derivation/payload lanes preserved
[ ] retained execution scopes and State<T> preserved
[ ] stream/history isolation preserved
[ ] failure atomicity and memory convergence preserved
[ ] R6b begins only after transport finalization
[ ] T15 authoritative run uses final repository/transport shape
```

## Agent safety

```text
[ ] TUI agents cannot discover application implementation symbols locally
[ ] application agents see public @iyon/tui APIs, not native internals
[ ] ownership constraints are machine checked
[ ] ARCHITECTURE.md and AGENTS.md state inverse repository responsibilities
[ ] wrong dependency directions fail CI
```

---

# 21. Final instruction

Preserve the current repository as the historical and technical home of the generic TUI framework. Rename it `iyon-tui`; do not rewrite its PERF provenance. Extract the smaller application/kernel/plugin side into a new `iyon` repository with filtered history and an explicit SHA map.

Make each repository own one native addon and one public responsibility. The TUI addon and TypeScript facade must contain no application concepts. The application addon must contain no TUI framework implementation. TypeScript public APIs are the only integration surface.

Perform the repository separation without changing behavior. Only after both repositories build and test independently should the standalone TUI repository replace its unsafe Bun FFI lowering with generated safe N-API bindings. Preserve every transport-independent PERF-12 and T13.1 invariant throughout. Then run T13.1 R6b and the remaining PERF-12 hardening/decision work against the final repository and transport shape.

The objective is not merely cleaner code. It is a filesystem and dependency graph that make the correct architecture the easiest path for both humans and AI agents, while making plausible-but-wrong product/framework coupling physically difficult to create.

---

# Tranche implementation records

## S0 implementation record

**1. Scope statement.** Freeze the pre-separation repository provenance, hosting state, dependency/package graph, path ownership, public Rust/TypeScript/native surfaces, test and benchmark inventory, native/ABI artifact identity, PERF-12/T13.1 provenance, mandatory check outcomes, and exact known failures. S0 changes documentation/evidence only; it does not repair baseline failures or alter application/framework behavior.

**2. Commits/repository SHAs.** Source revision tested and inventoried: `bd503b0382e34d74a38c562b9662d08c8c96f58a` (`feat: add repo split doc`, clean tree before capture). Baseline artifact commit: `165916aa1b62b1dc630d0ccd44de309be23b98d8` (`docs(split): freeze S0 separation baseline`). This implementation-record commit is canonically identified by the annotated local tag `pre-iyon-tui-repository-separation`; no commit or tag was pushed during S0.

**3. Review findings and corrections.** (a) Every one of the 1,509 source-revision tracked paths now has an explicit destination/action: 1,064 `iyon-tui`, 380 `iyon`, 64 `both-derived` seams with a named split/rewrite/minimization action, and one retirement; zero paths are unresolved. `both-derived` is explicitly not permission for an automatically copied shared facade. (b) GitHub had three branches, four active workflows, and zero tags, issues, pull requests, or releases, so no active hosting item requires migration. (c) The ignored staged addon initially hashed `df5c0ac0…`; rebuilding/staging from the source revision produced `0820e73f…`, byte-identical to the release dylib. Both observations are retained rather than conflated. (d) The checked-in Rust API parity artifacts are stale (`missing=10`, `stale=0`; manifest hash mismatch); the current source API, mappings, facade, and actual addon exports were therefore frozen independently. (e) Mandatory baseline checks are not wholly green: formatting, strict Clippy, API-surface parity, one order-dependent runtime test, one reproducible application viewport test, and a missing native-verification script are recorded exactly in `docs/repository-separation/s0/checks.md`; no failure was hidden or repaired in this tranche.

**4. Implementation summary.** Added `docs/repository-separation/s0/` containing normalized environment and hosting JSON; direct Cargo/TypeScript dependency manifests; a complete path-ownership TSV; TypeScript TUI, native addon, and Rust API snapshots; native/ABI/lock artifact hashes; hash-addressed test/benchmark and PERF registries; original PERF history identities; and the check report. The evidence commit contains 11 files and 4,902 lines. All JSON parses, the ownership registry is unique and complete against the source revision, and the evidence commit passes `git show --check`.

**5. Provenance.** Repository `alexykn/iyon`, branch `perf-refactor`, source remote `git@github.com:alexykn/iyon.git`; Bun `1.4.0+34cbb9a40`; rustc `1.97.1`; cargo `1.97.1`; macOS `26.6.2` arm64. Source-built addon SHA-256 `0820e73f68b98dfbab3d0a3ed1bd3590f08a290157433df98e2d1cc1cae493dd`; View ABI version 1, semantic schema version 1, 57 functions, schema BLAKE3 `8a6fdc06…`, generator BLAKE3 `0fb2fdc8…`. PERF artifacts and the 71-commit PERF-12/T13.1 path-history inventory remain in the original repository with unchanged commit identities. No filtered application repository exists yet; its provenance map belongs to S2.

**6. Gate evidence with actual counts/numbers.** Cargo workspace: 7 packages; TypeScript manifests including fixtures: 31; tracked tests: 226; benchmark-directory files: 65; raw PERF artifacts: 36. Public surface: 55 TUI runtime value exports, 34 TUI type exports, 34 actual addon exports, 2,331 Rust mapping records (1,532 TUI). Checks: generated TUI ABI PASS; TypeScript typecheck PASS; non-`api-surface` Rust battery 1,079 pass / 0 fail / 3 ignored; runtime TUI battery 213 pass / 1 documented order-dependent fail and 6/6 isolated control PASS; external fixture 10/10 PASS; application suite 113 pass / 1 documented reproducible fail; plugin framework 30/30 PASS; execution-frontier battery 18/18 PASS; retained identity/wide/payload battery 31/31 PASS. Memory soak PASS: 100,000 keyed cycles, 6,250 aborts, RSS 79 MiB from 20k through 100k, 64 steady subscribers, zero after disposal. Rust format reports four diffs in one file; workspace Rust tests report three API-surface failures; strict Clippy emits 105 `error:` diagnostics. T13.1 R6b remains blocked exactly as required.

**7. Status.** **S0 COMPLETE.** Baseline artifacts are committed, every current path has a migration disposition, hosting and PERF provenance are frozen, mandatory checks were executed, and all known failures are stated without a false green summary. The annotated pre-separation tag closes the local S0 freeze; S1 may begin, but no remote cutover or transport change is authorized by this record.
