# Iyon TUI Repository Separation

## Make the repository boundary the framework boundary

**Status:** proposed architecture and migration handoff  
**Current repository:** `alexykn/iyon`  
**Recommended destination:** rename the current repository to `alexykn/iyon-tui`; extract the application/kernel side into a new `alexykn/iyon` repository  
**Transport sequence:** behavior-neutral repository separation first; safe N-API migration second; retain direct FFI behind an explicit feature gate thereafter
**PERF relationship:** preserve PERF-12 T1–T13 and PERF-12 T13.1 R0–R10; keep T13.1 R6b blocked until the post-extraction transport is finalized; finish PERF-12 against the N-API default and gated direct-FFI oracle

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
| **S7** | Retain direct FFI as an explicit qualification/oracle/rollback feature; do not delete the path | N-API is the default; the default build has no FFI/pointer surface, while the gated direct-FFI build remains available and tested |
| **S8** | Resume deferred PERF work against the N-API default and gated direct-FFI oracle | T13.1 R6b, PERF-12 T14/T15, then conditional non-transport cleanup execute in order; no later S tranche removes the gated FFI path |

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

## 12.5 Dual-backend qualification and retention

Starting in S6 and continuing through all later S tranches, the extracted TUI repository must retain both lowering arms:

```text
feature-gated direct-FFI backend  # oracle / qualification / rollback only
 default generated N-API backend  # supported product path
```

This is a deliberate transport boundary, not a temporary permission to delete the direct arm. The default package and addon expose only the safe N-API contract; an explicit native/build feature enables the private direct-FFI qualification surface. Both backends consume the same semantic DAG, retained runtime, fixtures, counters, and host. No application code or public API changes between arms.

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

# 13. Tranche S7 — retain direct FFI behind an explicit feature gate

S7 and every later S tranche must **not remove the direct-FFI path**. N-API is the default supported lowering; direct FFI remains private, feature-gated, and available for qualification, oracle comparison, and rollback.

S7 may harden the split:

```text
keep the canonical ABI/schema and generated N-API bindings
keep the direct-FFI implementation behind the explicit `direct-ffi` feature
keep direct-FFI symbols absent from the default addon and public TypeScript contract
keep separate conformance/benchmark arms for both transports
keep all PERF-12/T13.1 transport-independent improvements shared
```

The default-build audit must prove:

```text
no Bun FFI import or pointer bootstrap in the default package/addon surface
no raw native pointer fields in public TypeScript contracts
N-API generated methods are the default lowering
application exports remain absent
```

The feature-build audit must prove:

```text
`direct-ffi` explicitly enables the private legacy qualification surface
its symbols are not reachable through the public @iyon/tui API
its ABI/behavior remains covered as the rollback/oracle arm
```

Do not interpret the absence of FFI symbols in the default artifact as permission to delete the feature implementation. Any future proposal to remove it requires a separate explicit transport decision after PERF-12 T15 and is outside S7 and the later repository-extraction tranches.

---

# 14. Tranche S8 — resume PERF work with both transport arms retained

Repository and transport finalization precede deferred optimization work. N-API remains the default product lowering; the feature-gated direct-FFI arm remains the private oracle/rollback comparison throughout.

Required sequence:

```text
1. T13.1 R6b
   incremental MountGraph/layout/paint frontier against the N-API default,
   with direct-FFI parity retained for qualification

2. PERF-12 T14
   randomized DAG differential tests, fuzzing, cross-transport/lifetime hardening

3. PERF-12 T15
   authoritative process-isolated comparison of N-API and the gated direct-FFI
   oracle, plus all required completed alternatives and adoption decision

4. PERF-12 T16
   conditional cleanup only for proven-dead non-transport machinery; preserve
   the direct-FFI feature path
```

T15 must measure the T13.1-adopted execution system and final N-API transport, while retaining the direct-FFI arm as the comparison/oracle. Do not publish a decision run over the pre-extraction mixed addon, and do not treat the deliberate two-arm final shape as a temporary state to be deleted.

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

## 17.5 Feature-gated direct FFI remains after extraction

**Risk:** agents accidentally make the legacy transport the default, expose its pointers publicly, or delete the rollback/oracle arm before PERF-12 is finished.

**Mitigation:** N-API is the only default package/addon lowering; the direct-FFI implementation is private and enabled only by an explicit feature; both arms share the semantic/runtime architecture and remain covered by qualification tests. Prohibit public API changes around transport details and preserve the feature through all later S tranches.

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
[ ] default addon has no raw pointer/bootstrap Bun FFI surface
[ ] direct FFI remains available only behind the explicit qualification feature
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

Perform the repository separation without changing behavior. Only after both repositories build and test independently should the standalone TUI repository make generated safe N-API the default lowering. Preserve the direct Bun FFI implementation behind an explicit private feature for qualification, oracle comparison, and rollback; do not remove it in S7 or later repository-extraction tranches. Preserve every transport-independent PERF-12 and T13.1 invariant throughout. Then finish T13.1 R6b and the remaining PERF-12 hardening/decision work against the final N-API default while retaining the gated direct-FFI arm.

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

## S1 implementation record

**1. Scope statement.** Machine-enforce framework/application ownership while both still live in one checkout: a Rust dependency-direction gate, TUI-native and application-native module purity gates, TypeScript import-direction gates for framework/runtime/application sources, and public-surface guards (snapshot plus banned-name rejection) for the TypeScript facade and the mapped Rust surface. Per direction, the checks run locally (`bun run check:ownership`); no CI wiring was added. Behavior-neutral: no production source was modified.

**2. Commits/repository SHAs.** S0 completion revision `ba33316c95cf7c45743acb0957232912757d8a77` (tagged locally `pre-iyon-tui-repository-separation`). S1 commit: this tranche's single documentation/tooling commit on top of it.

**3. Review findings and corrections.** (a) The first checker draft produced three false positives: the SDK's own generated `./tui/index.d.ts` re-export was flagged by a path regex applied to relative specifiers; locale-collated shell `sort` disagreed with JS sort ordering on the Rust snapshot; and substring matching flagged `AnsiColor::Magenta` via the `agent` inside "magenta". Corrections: relative imports are judged by resolved path only; snapshot comparison sorts both sides in-process; banned names match exact final path segments case-insensitively. (b) Negative testing exposed a real gate gap — application-side dynamic `import()` calls were not scanned; the shared specifier extractor now covers static, dynamic, and bare side-effect forms, after which injected violations are caught. (c) The ambient declarations in `virtual-modules.d.ts` belong to the same recorded S4/S5 virtual-module seam as `virtual-modules.ts`; both are exempted explicitly rather than silently. (d) AGENTS.md received only one surgical line (run the ownership gate); its existing extensive boundary prose already satisfied most of handoff §7.4.

**4. Implementation summary.** New `tools/ownership/check.ts` implements eleven checks: Rust dependency closure of `iyon-tui` excludes `iyon-core`/`iyon-api` (via `cargo metadata`); TUI-native modules (`crates/iyon-native/src/tui.rs`, `src/tui/**`, `src/generated/**`, `tests/generated_view_abi.rs`) reference no application crate; application native modules reference no TUI module; `crates/iyon-tui` sources are pure; framework TypeScript (52 files) imports nothing outside `src/tui/**` except ten recorded native-contract seams into `src/native.ts`; 108 application production files use public TUI surfaces only; runtime non-TUI sources enter through `tui/index.ts` only (19 recorded virtual-module alias seams); the TypeScript facade's 55 value + 34 type exports match the frozen S0 snapshot exactly; banned application concepts are absent from both surfaces; the mapped Rust surface (1,532 items) matches the new committed snapshot `tools/ownership/snapshots/iyon-tui-rust-surface.txt`. Wired as root script `check:ownership`. Added root `ARCHITECTURE.md` per §7.4 (generic-framework statement, layer map, ownership rules, API discipline, machine-check instruction).

**5. Provenance.** Same checkout/branch/remote as S0 (`alexykn/iyon`, branch `perf-refactor`); Bun 1.4.0+34cbb9a40; rustc/cargo 1.97.1; snapshots derived from the frozen S0 baseline artifacts at `bd503b0` evidence state. No dependency changes; no CI modifications; no remote mutation.

**6. Gate evidence with actual counts/numbers.** All eleven checks PASS on the committed tree. Negative tests prove detection: injecting an `iyon-api` dev-dependency into `crates/iyon-tui/Cargo.toml` FAILs `rust-dependency-direction`; appending an `iyon_core` reference to `src/tui.rs` FAILs `tui-native-module-purity`; adding `crate::tui` to `core.rs` FAILs `app-native-module-purity`; a framework file importing outside the framework FAILs `framework-ts-import-direction`; an application file importing `retained_dag.ts` FAILs `app-ts-public-entrypoints-only`; a dynamic `@iyon/runtime/tui/<internal>` import FAILs the same gate; tampering the TS snapshot FAILs `ts-surface-snapshot` and demonstrates banned-name drift handling; the earlier Magenta false positive (fixed) proves the Rust banned-name matcher discriminates. Every probe was reverted; final tree passes clean. `bunx tsc --noEmit` passes with the new tooling in scope.

**7. Status.** **S1 COMPLETE.** Ownership boundaries are machine-enforced in one checkout, the public-surface guards are green against frozen snapshots, and the recorded seams (native contract, virtual modules) are explicit and counted. No repository split, addon split, or transport change occurred; S2 remains the next authorized tranche.

## S2 implementation record

**1. Scope statement.** Create and verify the filtered application repository before changing the original remote; preserve the original repository and all PERF commit identities under the new `alexykn/iyon-tui` name; then assign the released `alexykn/iyon` name to the verified filtered repository. Record an explicit old→filtered SHA map and name every temporary compatibility path. Repository/hosting change only: no production behavior, addon split, package split, or transport change.

**2. Commits/repository SHAs.** Canonical source/TUI repository: pre-cutover head `55f232738aa362d8eb2c45e2e6e7e26468abe2ec`; S0 tag target `ba33316c95cf7c45743acb0957232912757d8a77` remains unchanged and is published only in `alexykn/iyon-tui`. Filtered source head before extraction metadata: `ffe17687c159aece8a3b4648e3cfcdf7119312e9` (mapping of source `55f2327…`). Application provenance commit `2f0053186e756a043d25d38a6499d99efd5bf146`; application S2 record commit `33b89aca007dd0871ce02e72bf2444681af757da`. This canonical S2 record commit lands in `alexykn/iyon-tui` after both remotes were verified.

**3. Review findings and corrections.** (a) `git-filter-repo` was not installed as a Git subcommand; S2 used ephemeral `uv run --with git-filter-repo` version 2.47.0, adding no repository dependency. (b) A purely application-owned path filter would leave the current mixed workspace unbuildable before S3–S5. S2 therefore retained the minimum local TUI/native/runtime/generator compatibility paths required for behavior-neutral verification and committed `APPLICATION-EXTRACTION-COMPATIBILITY.md`, assigning each canonical owner and mandatory removal tranche; this is explicitly not shared ownership. (c) The first clean-clone ownership run failed because the ignored `.node` artifact was absent, not because of source drift; the documented verification order now stages the native artifact before loading the runtime surface. (d) Reusing `alexykn/iyon` invalidates reliance on GitHub's old-name redirect. All concrete GitHub links and repository labels in canonical TUI/PERF documents were updated to `alexykn/iyon-tui`; frozen S0 capture data and this handoff's historical cutover prose remain unchanged. (e) Existing GitHub workflow failures were not used; per instruction, S2 evidence is local only and no CI configuration was added or modified.

**4. Implementation summary.** Fresh local clone filtered across all three branches with 38 included path roots. `SOURCE-SHA-MAP.jsonl` contains one record per original commit with old SHA, filtered SHA or explicit null, included paths, tool version, and exact command; `EXTRACTION-PROVENANCE.json` summarizes repository IDs and key mapped heads. The app root now has inverse `ARCHITECTURE.md`/AGENTS guidance and the compatibility-removal registry. Hosting sequence: create public `alexykn/iyon-app-extract`; push/verify `main`, `bun-refactor`, `perf-refactor`; publish the original S0 tag and S1 head; rename original repo `alexykn/iyon`→`alexykn/iyon-tui`; rename verified app repo `alexykn/iyon-app-extract`→`alexykn/iyon`; set explicit SSH remotes so no redirect is used. Original TUI repository ID `R_kgDOTw9laA` followed the rename; new application repository ID `R_kgDOUDBIpA` followed the temporary repository rename.

**5. Provenance.** Original/canonical repository `alexykn/iyon-tui`, GitHub ID `R_kgDOTw9laA`, created 2026-08-07, default `main`; application repository `alexykn/iyon`, GitHub ID `R_kgDOUDBIpA`, created 2026-08-24, default `main`. Both are public and expose `main`, `bun-refactor`, and `perf-refactor`. Original repository history was never filtered or rewritten. Extraction map: 713 unique source SHAs, 613 mapped filtered commits, 100 explicit pruned records; all old SHAs resolve in the TUI repository and every non-null destination SHA resolves in the application repository. Source tag target `ba33316…` maps to filtered historical target `2eedb6a…`, but the public pre-separation tag remains canonical only in the TUI repository.

**6. Gate evidence with actual counts/numbers.** Temporary app remote verified before either rename: three branch heads matched local filtered heads; provenance files present remotely; 0 issues / 0 pull requests / 0 releases / 0 tags. Final remote identity verification: `alexykn/iyon-tui` ID `R_kgDOTw9laA`, branches at original SHAs and annotated S0 tag; `alexykn/iyon` ID `R_kgDOUDBIpA`, filtered branches with `perf-refactor` at `33b89ac…`. App checkout local checks: frozen install PASS (one intentionally absent consumer-fixture workspace noted); native staging/load PASS; all eleven ownership gates PASS; TypeScript typecheck PASS; Rust excluding known-stale `api-surface` 1,079 pass / 0 fail / 3 ignored; plugin framework 30/30; application suite 113 pass / 1 exact S0-known viewport failure. Provenance integrity checks: 713/713 source SHAs resolve, 613/613 mapped SHAs resolve, 100 pruned entries are explicit, JSON/JSONL parse and `git diff --check` pass. TUI checkout ownership gates remain green. No CI result is claimed.

**7. Status.** **S2 COMPLETE.** The original repository now exists as `alexykn/iyon-tui` with unchanged commit/tag identities; the verified filtered application history now exists as the new `alexykn/iyon`; the explicit SHA map and compatibility-removal registry are committed; both local remotes point directly at final names. S3 may remove application Rust/native surfaces from the TUI repository, but no compatibility path may outlive its recorded S3–S5 tranche.

## S3 implementation record

**1. Scope statement.** Remove application Rust crates and application-native exports from the canonical TUI repository, split the generic bridge into `crates/iyon-tui-native`, and keep the existing direct-FFI transport behavior unchanged. S3 does not perform the safe N-API migration, TypeScript package extraction, or application consumer conversion; those remain S4–S6.

**2. Base and implementation.** The tranche starts from the S2 TUI cutover head `c01ca3ef7720532f302287b85888661de20cf543` on `perf-refactor`; implementation commit `ad7973fc4bb546cc3778385e2dbf31ed517c260d` contains the standalone split. The former `crates/iyon-native` TUI files are Git-moved into `crates/iyon-tui-native`; application-only files are removed rather than stubbed or re-exported. The TUI workspace now contains exactly three packages: `iyon-tui`, `iyon-tui-native`, and `tui-abi-gen`.

**3. Review findings and corrections.** (a) The old native crate mixed `iyon-api`/`iyon-core` bindings with the generic TUI bridge. S3 removed `api.rs`, `async_ops.rs`, `core.rs`, `credentials.rs`, `events.rs`, `handles.rs`, `model_turn.rs`, `queue.rs`, `tool_execution.rs`, `value.rs`, and their application tests; the new crate has no application dependencies or exports. (b) The application Rust crates `crates/iyon-api`, `crates/iyon-core`, and `crates/iyon` were deleted from this TUI checkout because their canonical copies are already in `alexykn/iyon`; no corresponding files were changed in the application repository. (c) The native artifact is now staged as `packages/iyon-runtime/native/iyon-tui-native.node`, built from package `iyon-tui-native`, with platform outputs `libiyon_tui_native.*`/`iyon_tui_native.dll`; `nativeVersion()` returns `iyon-tui-native/s3`. The addon retains only generic TUI symbols and the `tuiSmoke` framework probe. (d) Generated ABI output paths, manifest hashes, tests, and the ABI generator snapshot were regenerated for the new crate path. The direct-FFI ABI and schema remain unchanged; safe N-API transport work is deliberately deferred. (e) Existing owner-thread/compatibility shapes trigger current Clippy heuristics under `--all-features`; package-local lint allowances document those intentional framework/bridge shapes without changing runtime behavior. No CI configuration was modified.

**4. Implementation summary.** Root Cargo workspace membership and lockfile were reduced to the standalone TUI workspace; application-only workspace dependencies were removed. `tools/ownership/check.ts` now checks `crates/iyon-tui-native` purity and rejects an obsolete mixed native manifest. The runtime staging/loader and active TUI ABI benchmarks use the new artifact identity. The historical S0/S1 records retain their original pre-S3 paths as provenance; generated ABI documentation reflects the current path.

**5. Provenance.** Original Git/PERF history in `alexykn/iyon-tui` remains unrewritten; this tranche is an ordinary descendant of S2. Application Rust source remains available from the filtered `alexykn/iyon` repository. No transport, provider, model, tool, queue, or application behavior was moved into the TUI crate.

**6. Gate evidence with actual counts/numbers.** `cargo fmt --all -- --check` PASS; `cargo clippy --workspace --all-targets --all-features -- -D warnings` PASS; `cargo test --workspace` PASS with 888 passed / 0 failed / 3 ignored across 23 Rust test binaries; `cargo tree -p iyon-tui-native` contains only the `iyon-tui-native` root and generic `iyon-tui` framework crate among Iyon packages, with no `iyon`, `iyon-api`, or `iyon-core`; `cargo run -q -p tui-abi-gen -- check` PASS; native staging/load PASS for darwin-arm64; the loaded addon exposes 18 generic/framework exports and no `KernelSession`, `ModelTurn`, `ToolExecution`, credential, or event-queue symbols; `bun run check:ownership` passes all 11 gates; `bun run typecheck` PASS; the TUI Bun suite passes 53/53 tests with 136 expect calls. No CI result is claimed.

**7. Status.** **S3 COMPLETE.** The TUI Rust workspace is standalone, application Rust and app-native symbols are absent from the TUI repository, and the generic native bridge has the `iyon-tui-native` identity while retaining the existing direct-FFI implementation. S4 is the next authorized tranche: extract the TypeScript TUI package and its native loader without reintroducing application concepts or transport changes.

## S4 implementation record

**1. Scope statement.** Extract the TypeScript TUI implementation and native contract into the standalone `@iyon/tui` workspace package, move the TUI test/ABI fixtures with it, make the external consumer fixture depend only on the public package, and preserve the existing direct-FFI transport. Application virtual modules remain application-owned; the old runtime TUI entrypoint is an explicit S4–S5 compatibility wrapper only.

**2. Base and implementation.** The tranche starts from S3 record head `100d300e702688db12e73421d1f982c3bb775e99` on `perf-refactor`; implementation commit `98267f0614d3ba33d216c24b67476e92ba52448d` contains the extraction. `packages/iyon-runtime/src/tui/**` is now `packages/iyon-tui/src/**`; the package includes 53 TypeScript source files, the generated bridge schema/ABI outputs, a package-local `tsconfig.json`, and package-owned native staging at `packages/iyon-tui/native/iyon-tui-native.node`.

**3. Review findings and corrections.** (a) The former TUI files imported the application runtime native contract through `../native.ts`. S4 created a 288-line `NativeTuiAddon` contract in `@iyon/tui/src/native.ts` containing only generic host, history, stream, input, slot, pane, projector, and View ABI surfaces; no `NativeAddon`, `KernelSession`, `ModelTurn`, `ToolExecution`, credential, or application event-queue types remain in the package. (b) The old combined virtual-module registration remains in the application runtime; `iyon:tui` now re-exports `@iyon/tui` for compatibility, while the standalone package has no knowledge of `iyon:api`, `iyon:core`, or `iyon:plugins`. Production application/plugin imports in this checkout use the public `@iyon/tui` entrypoint; the old `@iyon/runtime/tui` export is retained only for the S5 removal tranche. (c) The ABI generator, bridge schema, generated manifests, layout test, and generated benchmark case were moved/regenerated under `packages/iyon-tui`; the native build script now reads the package-owned schema. (d) TUI tests were moved into the package, and runtime benchmarks were redirected to the package source/artifact without changing benchmark logic. (e) The consumer fixture dependency and all fixture imports now use only `@iyon/tui`; a standalone temporary checkout containing only `@iyon/tui` and the fixture passed without the application repository. (f) No safe N-API transport, core-native split, provider behavior, or CI configuration was changed.

**4. Implementation summary.** `packages/iyon-tui/package.json` owns the public package name, native staging script, package typecheck, and TUI test command. Root TypeScript paths and workspace lock metadata include `@iyon/tui`; `packages/iyon-runtime` depends on it and retains only a one-file compatibility barrel at `src/tui/index.ts`. `tools/ownership/check.ts` now scans the new framework root and adds a standalone-consumer gate. `tools/tui-abi-gen` and `tools/tui-abi/view_abi.toml` target the new package paths. The existing `iyon-tui-native.node` artifact is loaded by the package-local contract; the native transport remains Bun FFI and is still scheduled for S6.

**5. Provenance.** Source files were Git-moved from the S2/S3 TUI checkout; no application repository history was rewritten. The application repository remains unchanged by S4 and still owns its pre-S5 runtime/native compatibility copy. The package's public API export names remain equal to the frozen S0 TypeScript surface; no application-specific names were added.

**6. Gate evidence with actual counts/numbers.** `bun install` PASS; `bun run packages/iyon-tui/scripts/stage-native.ts` PASS for darwin-arm64; `bun run check:tui-abi` PASS; `cargo test -p tui-abi-gen` 27/27 PASS; `bun run check:ownership` passes 12/12 gates; root `bun run typecheck` PASS; package-local `bun run typecheck` PASS; package plus fixture tests pass 66/66 with 180 expect calls across 19 test files (17 package, 2 fixture); standalone copied checkout fixture tests pass 10/10 with 35 expect calls across 2 files; the fixture manifest has exactly one dependency, `@iyon/tui`, and its source imports no application package or virtual module. No CI result is claimed and no CI file was modified.

**7. Status.** **S4 COMPLETE.** `@iyon/tui` is a standalone TypeScript/native-loader package with package-owned ABI artifacts and tests; the external consumer fixture works without the application repository; application code uses the public package in this checkout; and the direct-FFI implementation remains unchanged. S5 is next: convert `alexykn/iyon` into an exact external consumer and split its remaining application-native addon.

## S5 implementation record

**1. Scope statement.** Convert `alexykn/iyon` from the S2–S4 local compatibility checkout into an external consumer of the standalone TUI repository. Pin exact Rust and TypeScript revisions, remove local generic TUI source/tests/benches/ABI tooling, split the remaining native addon into the application-only `iyon-core-native`, and preserve application behavior and the direct-FFI TUI transport.

**2. Commits/repository SHAs.** TUI S5 cleanup/public package commit: `e322f10dff490c1423d988982c0782c22774f85d`. Application implementation commit: `947ce35b01117efa93e60ac93a1f621db8f3baf2`; clean-checkout TypeScript path fix: `03b4d99aec23bf34427fa7ea34eb43390a8e7e0e`; application S5 evidence record: `2cefdda18d57c792722c5c6f50687c336ec9345d`. Rust and TypeScript both pin TUI revision `e322f10dff490c1423d988982c0782c22774f85d`.

**3. Review findings and corrections.** (a) The application still had the S2 local `crates/iyon-tui` checkout, TUI subtree under `packages/iyon-runtime`, TUI benchmarks/ABI tests, and generated TUI ABI tooling. All were deleted from the application repository; authoritative copies remain in `alexykn/iyon-tui`. (b) The mixed `crates/iyon-native` addon contained generic View ABI/History/TextStream/ViewSlot/ScrollPane exports beside kernel/session/tool/credential exports. It is now `crates/iyon-core-native`; all TUI modules, generated ABI files, header, TUI tests, and TUI artifact paths are gone. (c) `packages/iyon-runtime/src/native.ts` is now the application-only `NativeCoreAddon` contract and loads `iyon-core-native.node`; no shared `NativeAddon` name remains. (d) Application/plugin imports use public `@iyon/tui`; `iyon:tui` is only an application-owned alias that re-exports that package. Deep imports into TUI source/ABI/native internals were removed, including tool-renderer tests that previously inspected `nodeForBridge`; they now assert public View results. (e) The TUI root is now a generic external package workspace with a package-owned `iyon-tui-native-stage` bin. The application explicitly stages that external addon before tests/builds and stages its own core addon separately. (f) The TUI checkout removed the remaining current application packages/plugins/runtime compatibility surface and app-only API tooling; its ownership checker now checks only the generic framework and external fixture. No CI file was modified.

**4. Implementation summary.** `alexykn/iyon` has no `crates/iyon-tui`, `packages/iyon-runtime/src/tui`, TUI benches/tests, `tools/tui-abi`, `tools/tui-abi-gen`, or local generic ABI/native contract. Its root Cargo/Bun manifests and lockfiles contain exact external pins; its application ownership checker enforces five S5 gates. `@iyon/tui` loads `iyon-tui-native.node` from its package-owned path, while `@iyon/runtime` loads only `iyon-core-native.node`. The TUI root package exposes the public facade and native-stage command for Git consumers. Direct FFI remains unchanged and safe N-API migration remains S6.

**5. Provenance.** The original TUI repository history remains canonical and unrewritten. The application retains S2 extraction provenance (`SOURCE-SHA-MAP.jsonl`, `EXTRACTION-PROVENANCE.json`, and `S2-APPLICATION-EXTRACTION-RECORD.md`); the normative handoff is retained only here. The application keeps its product-owned legacy `crates/iyon/src/tui/**` surface while it exists, but it no longer vendors the generic `iyon-tui` crate. Both independent native artifacts are staged from their owning repositories/packages; no shared native bridge was recreated.

**6. Gate evidence with actual counts/numbers.** Final TUI checkout: `cargo fmt --all -- --check` PASS; strict workspace Clippy PASS; Rust workspace 888 passed / 0 failed / 3 ignored across 23 test targets; ABI check PASS; ownership 9/9 PASS; root and package TypeScript typechecks PASS; TUI Bun tests 66/66 with 180 expect calls. Final application checkout: `bun install --ignore-scripts` resolves the exact Git package; external TUI and core-native staging/load PASS for darwin-arm64; typecheck PASS; ownership 5/5 PASS; API surface reachable 799 / mapped 799 / missing 0 / stale 0; Rust workspace 209 tests pass across 25 targets; focused application native/runtime tests 23/23 and plugin/tool/public-TUI tests 53/53 pass; full Bun suite 280/281 passes with the unchanged S0-known `production_successful_ls_is_green_finished` `row must fit in u16` failure. A fresh application clone at `03b4d99` passes install, both staging commands, ownership, typecheck, standalone build, and 7/7 external-consumer tests with 14 expect calls. No CI result is claimed.

**7. Status.** **S5 COMPLETE.** `alexykn/iyon` builds/tests against exact external TUI package and Rust revisions; the core and TUI addons are independent; no local generic TUI compatibility path remains; no shared giant native contract exists; and the clean external-consumer checkout passes. S6 is next: qualify the safe generated N-API lowering in the standalone TUI repository.

## S6 implementation record

**1. Scope statement.** Replace the default TUI View-bridge lowering from Bun direct FFI/bootstrap pointers to generated safe N-API methods after repository separation. Preserve the existing PERF-12 semantic/native architecture unchanged: eager immutable `BridgeViewNode` identity, one environment-owned `NodeId -> WeakView` cache, paged `NativeRef` table and scavenging, generation-scoped hints, `ensureNative` cutoff/promotion, root and temporary leases, derivation hints, `PersistentSeq` wide edits, text/style/Diff lanes, stream specialization, stale-ref recovery, and all retained View-bearing boundary routing. S6 changes only the physical call lowering; it does not introduce a packet VM, shared mirror graph, or generic changed-closure serialization.

**2. Commits/repository SHAs.** Generated safe transport implementation: `9058fc7fce3be30ef32042b2e4cf245cbca3c464`. Safe-boundary ownership gate and isolated transport benchmark: `d15c2e19b9dd385e7cb885959cdf0c46ab831ecc`; N-API dispatch-granularity probe: `618ade10dcc2eb5f5f8e7dee63fabe3a52a2c94b`; final benchmark provenance refresh: `c2557fe3e4dc72934fa27d1d0925d75da2bb277b`. Final TUI `perf-refactor` head: `c2557fe3e4dc72934fa27d1d0925d75da2bb277b`.

**3. Review findings and corrections.** (a) The pre-S6 generated TypeScript ABI linked 57 function pointers from `tuiViewAbiBootstrap` through `bun:ffi`; the active generated contract now exposes `NativeViewAbiHandle`, an opaque N-API class session, and generated method wrappers for all 57 semantic functions plus the 10 conformance probes. (b) The Rust methods call the existing generated, bounds-checked semantic implementations rather than duplicating or weakening PERF-12 behavior. `NativeViewAbiSession` owns an environment `Arc<NativeViewRuntime>`; JavaScript receives no runtime address, host address, function-pointer table, `NativeAbiPointers`, or `linkSymbols` surface. (c) Host-mutating operations accept the opaque `NativeTuiHost` N-API object. Buffer/pod operations accept typed arrays and compute capacity only for the synchronous call; CString inputs are copied into call-local storage; no borrowed pointer is retained after return. (d) The initial generated N-API conformance methods used `f32`, which napi 3 does not accept as a JS input conversion; the safe generated signature uses `f64` at the boundary and casts to the existing Rust `f32` conformance implementation. (e) The existing C ABI wrappers remain private Rust implementation/qualification machinery, while bootstrap, host-pointer, and pointer-probe JS exports are behind the explicit `direct-ffi` Cargo feature. The default staged addon exposes the N-API session and no legacy pointer/bootstrap probes. (f) A dispatch-granularity probe demonstrates that safe batching can reduce N-API dispatch density, but no generic operation-record VM was added: that remains a separate future lowering experiment under §123. No semantic algorithm, stream path, provider/application behavior, or CI file changed.

**4. Implementation summary.** `tools/tui-abi-gen` now emits `view_abi_napi.rs`, pointer-free TypeScript ABI/session types, typed-array/string N-API calls, and N-API conformance wrappers from the same canonical `tools/tui-abi/view_abi.toml`. `crates/iyon-tui-native` adds the opaque `NativeViewAbiSession`, `tuiViewAbiSession()`, generated methods, a feature-gated direct qualification surface, and `iyon-tui-native/s6`. `packages/iyon-tui/src/native_view_abi.ts` and `retained_dag.ts` retain the same transaction, hint, derivation, lease, cutoff, and recovery algorithms while passing opaque handles instead of pointers. The package version is `0.1.0-s6.0`; the public `@iyon/tui` value/type surface remains 55/34. `tools/ownership/check.ts` adds safe-N-API and generated-lowering gates. `packages/iyon-tui/bench/` contains the process-isolated transport comparison and dispatch probe raw JSONL. S7 unsafe-surface deletion is deliberately not included in S6.

**5. Provenance.** Source/final evidence head: `c2557fe3e4dc72934fa27d1d0925d75da2bb277b` on `alexykn/iyon-tui`, `perf-refactor`; direct baseline arm: clean checkout `e2b929944e51d5d3b163bcd81c66f2544b43f17a`. Bun `1.4.0`, revision `1.4.0+34cbb9a40b4bd1bd767d134a7065e66c2432a676`; rustc `1.97.1 (8bab26f4f 2026-07-14)`, target `aarch64-apple-darwin`. Safe N-API addon SHA-256: `07854db16e33d2bf826ac89834a69ec5acf8ae7a605e2a3c71b3ecdd4099f295`; direct comparison addon SHA-256: `ffba93c2d8a16288977b35b609b6032f934e0ed9ad332acf0e70295b151e14a7`. Semantic schema BLAKE3: `5e7332e72b071e87f451f9710dd21d6d9f707277281abe50f2583dc3509c1745`; generator BLAKE3: `7b78d1762bb7d796d3536e7f77c50ec7913f999797661d7d49e684fe74048568`.

**6. Gate evidence with actual counts/numbers.** `cargo fmt --all -- --check` PASS; strict workspace Clippy PASS; `cargo test --workspace` PASS with 888 passed / 0 failed / 3 ignored; `cargo test -p iyon-tui-native` PASS with 37 library tests, 5 generated ABI integration tests, 1 sync test, and 1 ignored representation benchmark; `cargo test -p tui-abi-gen` 27/27 PASS; generator freshness check PASS; root and package TypeScript typechecks PASS; ownership checks 11/11 PASS; package plus standalone fixture tests 66/66 with 184 expect calls. The default addon load probe reports `nativeVersion() = iyon-tui-native/s6`, N-API metadata `function_count = 57`, `generation = 1`, and `transport = napi`; top-level bootstrap/pointer probes are absent. `cargo check -p iyon-tui-native --features direct-ffi` also passes for isolated legacy qualification.

The raw smoke comparison in `packages/iyon-tui/bench/PERF-12-s6-napi-transport.jsonl` uses four fresh Bun processes per candidate, 50 warmup operations and 200 measured operations per case, with the same Bun/Rust/fixtures and retained route (`retained = 200`, `fallback = 0`) in every record. Safe N-API versus the clean S5 direct-FFI baseline medians are: `shared_path@20` 46,917 ns vs 41,375 ns (+13.4%); `shared_path@200` 56,791 ns vs 54,792 ns (+3.6%); `text_layout@20` 40,000 ns vs 39,916 ns (+0.2%); `text_layout@200` 39,958 ns vs 39,833 ns (+0.3%). These are measured deltas, not a claim that N-API is universally faster; the small shared-path dispatch case remains an observed regression. The separate safe batching probe (`PERF-12-s6-napi-dispatch.jsonl`, 20 rounds × 10,000 calls) measures 141.06 ns per individual N-API call versus 10.11 ns per operation inside one safe native batch (~14x dispatch-density reduction). No batching result is silently promoted into a generic production VM, and no S7 cleanup/adoption decision is claimed from this smoke profile.

**7. Status.** **S6 COMPLETE.** The standalone TUI now defaults to generated safe N-API over opaque native handles, while every transport-independent PERF-12/T13 algorithm and invariant remains active and green. Differential correctness, structural safety, typed-buffer lifetime, ownership, generator, and independent-build gates pass; performance deltas and the remaining dispatch-density regression are recorded honestly. Direct FFI is intentionally retained behind the explicit qualification feature for rollback/oracle use; S7 and later S tranches must preserve that gated path while PERF-12 is finished.

## S7 implementation record

**1. Scope statement.** Replace the former S7 deletion plan with an explicit retention policy: keep generated safe N-API as the default package/addon lowering, keep the legacy direct-FFI exports behind `iyon-tui-native`'s `direct-ffi` feature, and harden staging/load checks so feature artifacts cannot leak into the default artifact. S7 does not remove Bun FFI, bootstrap, or legacy qualification symbols; it removes only accidental default exposure and ambiguous feature-artifact reuse.

**2. Commits/repository SHAs.** Feature-gated staging hardening: `2077510d6e7edc4d5f630967f4af73402ed1c2cb` (`fix(tui): isolate feature-gated native staging`). Policy/documentation record: this commit. The direct feature remains declared in `crates/iyon-tui-native/Cargo.toml` as `direct-ffi = []`; default and feature builds use separate Cargo target roots (`target/` and `target-direct-ffi/`).

**3. Review findings and corrections.** (a) A direct-feature build followed by a default staging invocation could reuse the same Cargo release artifact and accidentally stage legacy exports into the default addon. `packages/iyon-tui/scripts/stage-native.ts` now selects a feature-specific target root and validates the loaded export surface. (b) The default probe rejects the legacy bootstrap/perf qualification exports and requires `tuiViewAbiSession`; the direct-feature probe requires the legacy qualification exports. (c) `tuiViewAbiDecodeRef` is intentionally not treated as a raw direct-FFI export: it is the safe synchronous N-API cold/oracle decoder retained by the default path. (d) No public TypeScript API, semantic/runtime algorithm, PERF-12/T13.1 optimization, or application behavior changed.

**4. Implementation summary.** S7 now has a permanent two-arm transport shape: generated N-API is the supported default; direct FFI is private and explicitly feature-gated for qualification, oracle comparison, and rollback. Feature-aware staging cannot silently cross-contaminate artifacts. Later S tranches must retain this gate and may clean only proven-dead non-transport machinery.

**5. Provenance.** Bun `1.4.0`, revision `34cbb9a40b4bd1bd767d134a7065e66c2432a676`; rustc `1.97.1 (8bab26f4f 2026-07-14)`; target `aarch64-apple-darwin`. Default artifact load passed with N-API session and no bootstrap/perf qualification exports. `ION_NATIVE_FEATURES=direct-ffi` artifact load passed with `tuiViewAbiBootstrap`, `tuiPerfAbiProbe`, and `tuiPerfAbiConformanceProbe`; the safe `tuiViewAbiDecodeRef` decoder is present in both arms by design.

**6. Gate evidence with actual counts/numbers.** `cargo test --workspace --features direct-ffi` PASS; focused `cargo test -p iyon-tui-native --features direct-ffi` PASS with 43 passed / 0 failed / 1 ignored; feature-aware default/direct/default staging sequence PASS; root TypeScript typecheck PASS; ownership checks 11/11 PASS; default N-API surface load PASS. The staging script's feature-specific target-root and export checks are now committed. PERF-12 T14/T15 and T13.1 R6b remain later work; S7 does not claim the PERF-12 adoption decision.

**7. Status.** **S7 COMPLETE.** The direct-FFI path is retained behind an explicit feature, the default addon is N-API-only for raw pointer/bootstrap qualification symbols, and artifact staging is isolated and audited. S8 proceeds with T13.1 R6b and the remaining PERF-12 hardening/authoritative comparison; no later S tranche may delete the gated FFI path.

## S8 implementation record

**1. Scope statement.** Correct the S6–S7 transport-boundary review findings and resume the authorized S8 PERF-12 work without changing the public `@iyon/tui` surface. S8 corrects the R6b counter gate, makes the default native artifact free of stable direct-FFI qualification symbols, adds a current finalized-runtime direct-FFI oracle arm for T15, and records the tranche in this handoff. T15 remains the authoritative adoption decision; T16 remains blocked.

**2. Commits/repository SHAs.** R6b counter-gate correction, generated transport-surface isolation, and the current direct-FFI oracle harness landed in `f67cba545382ccbfba6de29445a2baf3d58e998f` (`fix(tui): complete S8 transport corrections`). This S8 record is the accompanying documentation correction. The direct feature remains `direct-ffi = []`; no CI or shared native-addon contract was changed.

**3. Review findings and corrections.** (a) The R6b focused perf-counter test previously asserted `PaintCacheHits >= 999`, although incremental painting intentionally patches the retained surface directly and bypasses the full-tree paint cache. The gate now asserts bounded local paint work (`PaintNodesVisited`, zero full-tree cache hits, bounded misses/compositing) alongside the existing resolver/measurement bounds. (b) Generated ABI C wrappers, conformance wrappers, implementation symbols, and ABI probes are now stable-exported only under `direct-ffi`; default N-API methods call private generated Rust invocations. Staging validates the native symbol surface as well as JS exports. (c) `packages/iyon-tui/bench/direct_ffi/**` supplies a process-isolated direct oracle built against the finalized retained materialization/lease logic and the current native host; `perf12_t15_case.ts` selects either generated safe N-API or the feature-gated direct arm through `T15_TRANSPORT`. (d) Both benchmark arms expose §90 phase samples for transport preparation, native materialization, and host commit. The authoritative full §93/§102 matrix and adoption decision are intentionally still pending.

**4. Implementation summary.** The canonical ABI generator emits private N-API invocations plus `direct-ffi`-gated C wrappers from the same schema. The default binary has no `_iyon_abi_probe_*`, `_iyon_abi_conformance_*`, or `_iyon_*_v1` direct qualification symbols; the direct artifact retains them and the bootstrap/perf qualification exports. The current oracle uses the same workload definitions, retained identity/lease/frontier semantics, native host commit, structural counters, and phase instrumentation as the N-API case runner; only the physical lowering differs. The public package and ownership boundary remain application-free.

**5. Provenance.** Source revision at correction: `f67cba545382ccbfba6de29445a2baf3d58e998f`; schema BLAKE3: `5e7332e72b071e87f451f9710dd21d6d9f707277281abe50f2583dc3509c1745`; regenerated ABI generator BLAKE3: `3ec191da668808743dbd8fc0c89380de5015a70d86dcacd7482e00571ef11232`. Bun `1.4.0` / revision `34cbb9a40b4bd1bd767d134a7065e66c2432a676`; rustc `1.97.1 (8bab26f4f 2026-07-14)`; target `aarch64-apple-darwin`.

**6. Gate evidence with actual counts/numbers.** The corrected R6b perf-counter test passes in isolation; default and direct native crate tests pass 37/37 with 1 ignored plus 5 generated ABI and 1 sync integration test; `cargo test -p tui-abi-gen` passes 27/27; generator freshness, typecheck, fmt, and direct/default staging gates pass. Default `nm -gU` contains no direct qualification names; direct staging contains `iyon_abi_probe_noop` and `iyon_runtime_noop_v1`. A small isolated N-API/direct oracle run produces matching structural deltas and non-empty prepare/materialize/commit phase arrays for both arms. No T15 adoption or T16 cleanup is claimed from this smoke evidence.

**7. Status.** **S8 PARTIAL — T15 remains STOPPED.** R6b, default-symbol isolation, current direct-oracle lowering, phase visibility, and the missing S8 record are corrected. The full authoritative T15 matrix, memory recheck, correctness/structural review, and adoption decision must still run before T16 can begin; the gated direct-FFI implementation must remain available.

## S8/T15 completion erratum

The prior S8 status is historical. The authoritative T15 comparison has now run from committed source. The final common matrix contains 311 cases per arm / 622 records with zero correctness or structural mismatches and zero phase-array length mismatches. The §95 multi-edit, §96 wide/path, generic §99 trace, and §59 one-million-operation memory gates also completed for both generated safe N-API and feature-gated direct FFI. Raw artifacts and the report are recorded in `docs/history/PERF-12/PERF-12-T15-AUTHORITATIVE-REPORT.md` and the `packages/iyon-tui/bench/PERF-12-T15-*` JSONL files.

T15 is **COMPLETE as a technical comparison** and publishes a recommendation, not an automatic transport decision. Both arms remain buildable and retained until the repository owner explicitly decides. S8 remains **PARTIAL** pending that owner decision and any separately authorized T16 non-transport cleanup; no direct-FFI deletion or T16 cleanup was performed.